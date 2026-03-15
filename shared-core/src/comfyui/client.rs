use anyhow::{Context, Result};
use reqwest::Client;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::websocket::{ComfyUIWebSocket, ExecutionUpdate, ExecutionStatus, ProgressUpdate};

pub struct ComfyUIClient {
    base_url: String,
    client: Client,
    websocket: Arc<RwLock<Option<ComfyUIWebSocket>>>,
}

impl ComfyUIClient {
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            client: Client::new(),
            websocket: Arc::new(RwLock::new(None)),
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub async fn get_system_stats(&self) -> Result<Value> {
        let url = format!("{}/system_stats", self.base_url);
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to connect to ComfyUI")?
            .json::<Value>()
            .await?;

        Ok(response)
    }

    pub async fn queue_prompt(&self, workflow: &HashMap<String, Value>) -> Result<String> {
        let url = format!("{}/prompt", self.base_url);
        let mut payload = serde_json::Map::new();
        payload.insert("prompt".to_string(), serde_json::to_value(workflow)?);

        let response: Value = self
            .client
            .post(&url)
            .json(&payload)
            .send()
            .await?
            .json()
            .await?;

        let prompt_id = response["prompt_id"]
            .as_str()
            .context("No prompt_id in response")?
            .to_string();

        Ok(prompt_id)
    }

    pub async fn get_history(&self, prompt_id: &str) -> Result<Value> {
        let url = format!("{}/history/{}", self.base_url, prompt_id);
        let response = self
            .client
            .get(&url)
            .send()
            .await?
            .json::<Value>()
            .await?;

        Ok(response)
    }

    pub async fn get_image(
        &self,
        filename: &str,
        subfolder: &str,
        r#type: &str,
    ) -> Result<Vec<u8>> {
        let url = format!("{}/view", self.base_url);
        let response = self
            .client
            .get(url)
            .query(&[("filename", filename), ("subfolder", subfolder), ("type", r#type)])
            .send()
            .await?
            .bytes()
            .await?;

        Ok(response.to_vec())
    }

    /// Get or create WebSocket client for real-time updates
    pub async fn get_websocket(&self) -> Result<Arc<ComfyUIWebSocket>> {
        let mut ws_guard = self.websocket.write().await;

        if ws_guard.is_none() {
            let ws = ComfyUIWebSocket::new(&self.base_url);
            ws.connect().await.context("Failed to connect WebSocket")?;
            *ws_guard = Some(ws);
        }

        Ok(Arc::new(ws_guard.as_ref().unwrap().clone()))
    }

    /// Subscribe to progress updates for all executions
    pub async fn subscribe_progress(&self) -> Result<tokio::sync::broadcast::Receiver<ProgressUpdate>> {
        let ws = self.get_websocket().await?;
        Ok(ws.subscribe_progress())
    }

    /// Subscribe to execution status updates
    pub async fn subscribe_execution(&self) -> Result<tokio::sync::broadcast::Receiver<ExecutionUpdate>> {
        let ws = self.get_websocket().await?;
        Ok(ws.subscribe_execution())
    }

    /// Queue a prompt and wait for completion with real-time progress
    pub async fn queue_and_wait(
        &self,
        workflow: &HashMap<String, Value>,
        timeout_secs: u64,
    ) -> Result<Value> {
        // First, ensure WebSocket is connected
        let ws = self.get_websocket().await?;
        let mut exec_receiver = ws.subscribe_execution();

        // Queue the prompt
        let prompt_id = self.queue_prompt(workflow).await?;
        tracing::info!("Queued prompt: {}", prompt_id);

        // Wait for completion
        let prompt_id_clone = prompt_id.clone();
        let _result = tokio::time::timeout(
            std::time::Duration::from_secs(timeout_secs),
            async move {
                loop {
                    match exec_receiver.recv().await {
                        Ok(update) => {
                            if update.prompt_id == prompt_id_clone {
                                match update.status {
                                    ExecutionStatus::Success => {
                                        tracing::info!("Prompt {} completed successfully", prompt_id_clone);
                                        return Ok(());
                                    }
                                    ExecutionStatus::Failed => {
                                        return Err(anyhow::anyhow!(
                                            "Execution failed: {}",
                                            update.message.unwrap_or_default()
                                        ));
                                    }
                                    ExecutionStatus::Running => {
                                        tracing::debug!(
                                            "Prompt {} progress: {:.1}%",
                                            prompt_id_clone,
                                            update.progress * 100.0
                                        );
                                    }
                                    ExecutionStatus::Pending => {
                                        tracing::debug!("Prompt {} pending in queue", prompt_id_clone);
                                    }
                                }
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                            return Err(anyhow::anyhow!("WebSocket channel closed"));
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                            tracing::warn!("Receiver lagged by {} messages", n);
                        }
                    }
                }
            },
        )
        .await
        .context("Timeout waiting for prompt completion")??;

        // Get the final result from history
        let history = self.get_history(&prompt_id).await?;
        Ok(history)
    }

    /// Disconnect WebSocket connection
    pub async fn disconnect(&self) {
        let mut ws_guard = self.websocket.write().await;
        *ws_guard = None;
        tracing::info!("Disconnected ComfyUI WebSocket");
    }
}
