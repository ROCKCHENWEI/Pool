//! Batch Processing Module
//!
//! Provides utilities for batch import, export, and processing of images and workflows.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::{Mutex, RwLock};
use tokio::sync::Semaphore;

/// Batch task type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BatchTaskType {
    TextToImage,
    ImageToImage,
    Upscale,
    Inpainting,
    VideoGeneration,
    StyleTransfer,
    BatchProcessing,
}

/// Batch task priority
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum BatchTaskPriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Urgent = 3,
}

/// Batch task status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BatchTaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

/// Batch task definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchTask {
    pub id: String,
    pub name: String,
    pub task_type: BatchTaskType,
    pub priority: BatchTaskPriority,
    pub status: BatchTaskStatus,
    pub params: serde_json::Value,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub error: Option<String>,
    pub progress: f32,
    pub result: Option<serde_json::Value>,
}

impl BatchTask {
    /// Create a new batch task
    pub fn new(id: String, name: String, task_type: BatchTaskType) -> Self {
        Self {
            id,
            name,
            task_type,
            priority: BatchTaskPriority::Normal,
            status: BatchTaskStatus::Pending,
            params: serde_json::json!({}),
            created_at: chrono::Utc::now(),
            started_at: None,
            completed_at: None,
            error: None,
            progress: 0.0,
            result: None,
        }
    }

    /// Set priority
    pub fn with_priority(mut self, priority: BatchTaskPriority) -> Self {
        self.priority = priority;
        self
    }

    /// Set parameters
    pub fn with_params(mut self, params: serde_json::Value) -> Self {
        self.params = params;
        self
    }
}

/// Batch queue statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchQueueStats {
    pub total_tasks: usize,
    pub pending_count: usize,
    pub running_count: usize,
    pub completed_count: usize,
    pub failed_count: usize,
}

/// Batch queue for managing task execution
pub struct BatchQueue {
    tasks: Arc<RwLock<HashMap<String, BatchTask>>>,
    pending: Arc<Mutex<Vec<String>>>,
    running: Arc<Mutex<HashMap<String, ()>>>,
    max_concurrent: usize,
    semaphore: Arc<Semaphore>,
}

impl BatchQueue {
    /// Create a new batch queue
    pub fn new(max_concurrent: usize) -> Self {
        Self {
            tasks: Arc::new(RwLock::new(HashMap::new())),
            pending: Arc::new(Mutex::new(Vec::new())),
            running: Arc::new(Mutex::new(HashMap::new())),
            max_concurrent,
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
        }
    }

    /// Add a task to the queue
    pub fn add_task(&self, task: BatchTask) -> Result<()> {
        let task_id = task.id.clone();
        let priority = task.priority;

        {
            let mut tasks = self.tasks.write();
            tasks.insert(task_id.clone(), task);
        }

        // Add to pending queue based on priority
        let mut pending = self.pending.lock();
        let position = pending.iter().position(|t_id| {
            let tasks = self.tasks.read();
            let existing = tasks.get(t_id);
            if let Some(existing) = existing {
                existing.priority < priority
            } else {
                false
            }
        });

        let insert_pos = position.unwrap_or(pending.len());
        pending.insert(insert_pos, task_id);
        Ok(())
    }

    /// Get next task to process
    pub fn get_next_task(&self) -> Option<BatchTask> {
        let mut pending = self.pending.lock();
        if pending.is_empty() {
            return None;
        }

        let task_id = pending.remove(0); // Remove first element
        drop(pending); // Release the lock

        let tasks = self.tasks.read();
        let task = tasks.get(&task_id).cloned();

        if let Some(mut task) = task {
            task.status = BatchTaskStatus::Running;
            task.started_at = Some(chrono::Utc::now());

            // Mark as running
            {
                let mut running = self.running.lock();
                running.insert(task_id.clone(), ());
            }

            // Update task in map
            {
                let mut tasks = self.tasks.write();
                tasks.insert(task_id, task.clone());
            }

            return Some(task);
        }

        None
    }

    /// Update task progress
    pub fn update_progress(&self, task_id: &str, progress: f32) -> Result<()> {
        let mut tasks = self.tasks.write();
        if let Some(task) = tasks.get_mut(task_id) {
            task.progress = progress.clamp(0.0, 100.0);
        }
        Ok(())
    }

    /// Complete a task
    pub fn complete_task(&self, task_id: &str, result: Option<serde_json::Value>) -> Result<()> {
        {
            let mut tasks = self.tasks.write();
            if let Some(task) = tasks.get_mut(task_id) {
                task.status = BatchTaskStatus::Completed;
                task.completed_at = Some(chrono::Utc::now());
                task.progress = 100.0;
                task.result = result;
            }
        }

        // Remove from running
        {
            let mut running = self.running.lock();
            running.remove(task_id);
        }

        // Release semaphore
        self.semaphore.add_permits(1);

        Ok(())
    }

    /// Fail a task
    pub fn fail_task(&self, task_id: &str, error: String) -> Result<()> {
        {
            let mut tasks = self.tasks.write();
            if let Some(task) = tasks.get_mut(task_id) {
                task.status = BatchTaskStatus::Failed;
                task.completed_at = Some(chrono::Utc::now());
                task.error = Some(error);
            }
        }

        // Remove from running
        {
            let mut running = self.running.lock();
            running.remove(task_id);
        }

        // Release semaphore
        self.semaphore.add_permits(1);

        Ok(())
    }

    /// Cancel a task
    pub fn cancel_task(&self, task_id: &str) -> Result<()> {
        let was_running = {
            let mut tasks = self.tasks.write();
            let was_running = if let Some(task) = tasks.get(task_id) {
                task.status == BatchTaskStatus::Running
            } else {
                false
            };

            if let Some(task) = tasks.get_mut(task_id) {
                task.status = BatchTaskStatus::Cancelled;
                task.completed_at = Some(chrono::Utc::now());
            }
            was_running
        };

        // Remove from pending if there
        {
            let mut pending = self.pending.lock();
            pending.retain(|id| id != task_id);
        }

        // Remove from running if there
        {
            let mut running = self.running.lock();
            running.remove(task_id);
        }

        // Release semaphore if was running
        if was_running {
            self.semaphore.add_permits(1);
        }

        Ok(())
    }

    /// Get task by ID
    pub fn get_task(&self, task_id: &str) -> Option<BatchTask> {
        let tasks = self.tasks.read();
        tasks.get(task_id).cloned()
    }

    /// Get all tasks
    pub fn get_all_tasks(&self) -> Vec<BatchTask> {
        let tasks = self.tasks.read();
        tasks.values().cloned().collect()
    }

    /// Get tasks by status
    pub fn get_tasks_by_status(&self, status: BatchTaskStatus) -> Vec<BatchTask> {
        let tasks = self.tasks.read();
        tasks.values()
            .filter(|t| t.status == status)
            .cloned()
            .collect()
    }

    /// Get queue statistics
    pub fn get_stats(&self) -> BatchQueueStats {
        let tasks = self.tasks.read();
        let pending = self.pending.lock();
        let running = self.running.lock();

        BatchQueueStats {
            total_tasks: tasks.len(),
            pending_count: pending.len(),
            running_count: running.len(),
            completed_count: tasks.values().filter(|t| t.status == BatchTaskStatus::Completed).count(),
            failed_count: tasks.values().filter(|t| t.status == BatchTaskStatus::Failed).count(),
        }
    }

    /// Clear completed tasks
    pub fn clear_completed(&self) -> usize {
        let mut tasks = self.tasks.write();
        let before = tasks.len();
        tasks.retain(|_, t| t.status != BatchTaskStatus::Completed);
        before - tasks.len()
    }

    /// Clear failed tasks
    pub fn clear_failed(&self) -> usize {
        let mut tasks = self.tasks.write();
        let before = tasks.len();
        tasks.retain(|_, t| t.status != BatchTaskStatus::Failed);
        before - tasks.len()
    }
}

