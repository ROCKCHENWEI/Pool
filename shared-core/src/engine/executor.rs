//! Workflow Executor
//!
//! Executes workflows by coordinating with ComfyUI server and other providers.

use anyhow::{Context, Result};
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use serde::{Deserialize, Serialize};

use crate::comfyui::ComfyUIClient;
use crate::models::{Workflow, ComfyUIConfig};

use super::NodeEngine;

/// Workflow execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    /// Workflow ID
    pub workflow_id: String,
    /// Execution status
    pub status: ExecutionStatus,
    /// Output file paths (images, videos)
    pub output_files: Vec<String>,
    /// Error message if failed
    pub error: Option<String>,
    /// Execution time in seconds
    pub duration_secs: f64,
}

/// Execution status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ExecutionStatus {
    Pending,
    Running,
    Success,
    Failed,
    Cancelled,
}

/// Progress update for workflow execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowProgress {
    /// Workflow ID
    pub workflow_id: String,
    /// Current node being executed
    pub current_node: Option<String>,
    /// Overall progress (0.0 - 1.0)
    pub progress: f32,
    /// Status message
    pub message: String,
    /// Execution status
    pub status: ExecutionStatus,
}

pub struct WorkflowExecutor {
    engine: NodeEngine,
    workflow_id: String,
    comfyui_client: Arc<RwLock<Option<ComfyUIClient>>>,
    comfyui_config: Arc<RwLock<ComfyUIConfig>>,
    progress_sender: broadcast::Sender<WorkflowProgress>,
    result_sender: broadcast::Sender<ExecutionResult>,
}

impl WorkflowExecutor {
    pub fn new(workflow: &Workflow) -> Self {
        let mut engine = NodeEngine::new();

        for node in &workflow.nodes {
            engine.add_node(node.clone());
        }

        for conn in &workflow.connections {
            engine.add_connection(conn.clone());
        }

        let (progress_sender, _) = broadcast::channel(100);
        let (result_sender, _) = broadcast::channel(50);

        Self {
            engine,
            workflow_id: workflow.id.clone(),
            comfyui_client: Arc::new(RwLock::new(None)),
            comfyui_config: Arc::new(RwLock::new(ComfyUIConfig::default())),
            progress_sender,
            result_sender,
        }
    }

    /// Create executor with ComfyUI configuration
    pub fn with_comfyui(workflow: &Workflow, config: ComfyUIConfig) -> Self {
        let mut executor = Self::new(workflow);
        let mut current_config = executor.comfyui_config.try_write().unwrap();
        *current_config = config;
        executor
    }

    /// Set ComfyUI configuration
    pub async fn set_comfyui_config(&self, config: ComfyUIConfig) {
        let mut current_config = self.comfyui_config.write().await;
        *current_config = config;
    }

    /// Connect to ComfyUI server
    pub async fn connect_comfyui(&self) -> Result<()> {
        let config = self.comfyui_config.read().await;
        let client = ComfyUIClient::new(&config.server_url);

        // Test connection
        client.get_system_stats().await
            .context("Failed to connect to ComfyUI server")?;

        let mut current_client = self.comfyui_client.write().await;
        *current_client = Some(client);

        Ok(())
    }

    /// Check if connected to ComfyUI
    pub async fn is_connected(&self) -> bool {
        self.comfyui_client.read().await.is_some()
    }

    /// Subscribe to progress updates
    pub fn subscribe_progress(&self) -> broadcast::Receiver<WorkflowProgress> {
        self.progress_sender.subscribe()
    }

    /// Subscribe to execution results
    pub fn subscribe_results(&self) -> broadcast::Receiver<ExecutionResult> {
        self.result_sender.subscribe()
    }

    pub fn validate(&self) -> Result<Vec<String>> {
        self.engine.topological_sort()
    }

    /// Execute the workflow
    pub async fn execute(&self) -> Result<ExecutionResult> {
        let start_time = std::time::Instant::now();

        // Send initial progress
        let _ = self.progress_sender.send(WorkflowProgress {
            workflow_id: self.workflow_id.clone(),
            current_node: None,
            progress: 0.0,
            message: "Starting workflow execution...".to_string(),
            status: ExecutionStatus::Pending,
        });

        let execution_order = self.validate()?;

        // Check if we have ComfyUI connection
        let has_comfyui = self.comfyui_client.read().await.is_some();

        if has_comfyui {
            self.execute_with_comfyui(&execution_order, start_time).await
        } else {
            self.execute_locally(&execution_order, start_time).await
        }
    }

    /// Execute workflow using ComfyUI server
    async fn execute_with_comfyui(
        &self,
        _execution_order: &[String],
        start_time: std::time::Instant,
    ) -> Result<ExecutionResult> {
        let client_guard = self.comfyui_client.read().await;
        let client = client_guard.as_ref()
            .context("ComfyUI client not connected")?;

        // Send progress update
        let _ = self.progress_sender.send(WorkflowProgress {
            workflow_id: self.workflow_id.clone(),
            current_node: None,
            progress: 0.1,
            message: "Submitting workflow to ComfyUI...".to_string(),
            status: ExecutionStatus::Running,
        });

        // Convert workflow to ComfyUI format and submit
        let workflow_json = self.engine.to_comfyui_workflow();
        let prompt_id = client.submit_workflow(&workflow_json).await
            .context("Failed to submit workflow to ComfyUI")?;

        // Wait for completion
        let mut progress: f32 = 0.1;
        let mut output_files = Vec::new();

        loop {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;

            let history = client.get_history(&prompt_id).await?;

            if let Some(outputs) = history.get("outputs").and_then(|o| o.as_object()) {
                if !outputs.is_empty() {
                    // Extract output files
                    for (_, output) in outputs {
                        if let Some(images) = output.get("images").and_then(|i| i.as_array()) {
                            for image in images {
                                if let Some(filename) = image.get("filename").and_then(|f| f.as_str()) {
                                    output_files.push(filename.to_string());
                                }
                            }
                        }
                    }
                    break;
                }
            }

            progress = (progress + 0.05_f32).min(0.95_f32);
            let _ = self.progress_sender.send(WorkflowProgress {
                workflow_id: self.workflow_id.clone(),
                current_node: None,
                progress,
                message: "Executing workflow...".to_string(),
                status: ExecutionStatus::Running,
            });
        }

        let duration = start_time.elapsed().as_secs_f64();

        let _ = self.progress_sender.send(WorkflowProgress {
            workflow_id: self.workflow_id.clone(),
            current_node: None,
            progress: 1.0,
            message: "Execution complete!".to_string(),
            status: ExecutionStatus::Success,
        });

        let result = ExecutionResult {
            workflow_id: self.workflow_id.clone(),
            status: ExecutionStatus::Success,
            output_files,
            error: None,
            duration_secs: duration,
        };

        let _ = self.result_sender.send(result.clone());
        Ok(result)
    }

    /// Execute workflow locally (without ComfyUI)
    async fn execute_locally(
        &self,
        execution_order: &[String],
        start_time: std::time::Instant,
    ) -> Result<ExecutionResult> {
        let total_nodes = execution_order.len();

        for (i, node_id) in execution_order.iter().enumerate() {
            let progress = (i as f32 + 0.5) / total_nodes as f32;
            let _ = self.progress_sender.send(WorkflowProgress {
                workflow_id: self.workflow_id.clone(),
                current_node: Some(node_id.clone()),
                progress,
                message: format!("Executing node: {}", node_id),
                status: ExecutionStatus::Running,
            });

            self.execute_node(node_id).await?;
        }

        let duration = start_time.elapsed().as_secs_f64();

        let _ = self.progress_sender.send(WorkflowProgress {
            workflow_id: self.workflow_id.clone(),
            current_node: None,
            progress: 1.0,
            message: "Execution complete!".to_string(),
            status: ExecutionStatus::Success,
        });

        let result = ExecutionResult {
            workflow_id: self.workflow_id.clone(),
            status: ExecutionStatus::Success,
            output_files: vec![],
            error: None,
            duration_secs: duration,
        };

        let _ = self.result_sender.send(result.clone());
        Ok(result)
    }

    async fn execute_node(&self, _node_id: &str) -> Result<()> {
        // TODO: Implement node execution logic
        Ok(())
    }
}

/// Global workflow execution manager
pub struct WorkflowExecutionManager {
    comfyui_config: Arc<RwLock<ComfyUIConfig>>,
}

impl WorkflowExecutionManager {
    pub fn new() -> Self {
        Self {
            comfyui_config: Arc::new(RwLock::new(ComfyUIConfig::default())),
        }
    }

    pub async fn set_comfyui_config(&self, config: ComfyUIConfig) {
        let mut current = self.comfyui_config.write().await;
        *current = config;
    }

    pub async fn get_comfyui_config(&self) -> ComfyUIConfig {
        self.comfyui_config.read().await.clone()
    }

    pub fn create_executor(&self, workflow: &Workflow) -> WorkflowExecutor {
        let config = self.comfyui_config.try_read().unwrap();
        WorkflowExecutor::with_comfyui(workflow, config.clone())
    }
}

impl Default for WorkflowExecutionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Node, NodeType, Connection};

    fn create_test_workflow() -> Workflow {
        Workflow {
            id: "test-workflow".to_string(),
            shot_id: "test-shot".to_string(),
            name: "Test Workflow".to_string(),
            nodes: vec![],
            connections: vec![],
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn test_workflow_executor_creation() {
        let workflow = create_test_workflow();
        let executor = WorkflowExecutor::new(&workflow);
        assert!(executor.validate().is_ok());
    }

    #[test]
    fn test_execution_manager() {
        let manager = WorkflowExecutionManager::new();
        let workflow = create_test_workflow();
        let _executor = manager.create_executor(&workflow);
    }

    #[tokio::test]
    async fn test_progress_subscribe() {
        let workflow = create_test_workflow();
        let executor = WorkflowExecutor::new(&workflow);
        let mut receiver = executor.subscribe_progress();

        // Should be able to receive progress updates
        // In a real test, we would execute and check the receiver
    }
}
