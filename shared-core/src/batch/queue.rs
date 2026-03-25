//! Batch Queue for managing task execution
//!
//! This module provides priority-based task queue with support for canceling tasks.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::{Mutex, RwLock};
use tokio::sync::Semaphore;
use serde::{Deserialize, Serialize};

use super::{BatchTask, BatchTaskPriority, BatchTaskStatus, BatchQueueStats};

/// Priority entry for ordering tasks in the queue
pub struct PriorityEntry {
    pub task_id: String,
    pub priority: BatchTaskPriority,
}

impl Ord for PriorityEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        self.priority.cmp(&other.priority)
    }
}

impl PartialOrd for PriorityEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for PriorityEntry {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority && self.task_id == other.task_id
    }
}

impl Eq for PriorityEntry {}

/// Internal batch queue implementation
pub struct BatchQueueInner {
    tasks: Arc<RwLock<HashMap<String, BatchTask>>>,
    pending: Arc<Mutex<Vec<PriorityEntry>>>,
    running: Arc<Mutex<HashMap<String, ()>>>,
    max_concurrent: usize,
    semaphore: Arc<Semaphore>,
}

impl BatchQueueInner {
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

    /// Add a task to the queue with priority
    pub fn add_with_priority(&self, task: BatchTask) -> anyhow::Result<()> {
        let task_id = task.id.clone();
        let priority = task.priority;

        // Add to tasks map
        {
            let mut tasks = self.tasks.write();
            tasks.insert(task_id.clone(), task);
        }

        // Add to pending queue in priority order
        let entry = PriorityEntry { task_id: task_id.clone(), priority };
        let mut pending = self.pending.lock();

        // Find insertion point (higher priority first)
        let pos = pending.iter().position(|e| e.priority < priority).unwrap_or(pending.len());
        pending.insert(pos, entry);

        Ok(())
    }

    /// Get next highest priority task
    pub fn pop_next(&self) -> Option<BatchTask> {
        let mut pending = self.pending.lock();
        if pending.is_empty() {
            return None;
        }

        // Get highest priority entry
        let entry = pending.remove(0)?;
        drop(pending);

        // Get the task
        let tasks = self.tasks.read();
        let task = tasks.get(&entry.task_id).cloned()?;
        drop(tasks);

        Some(task)
    }

    /// Mark task as running
    pub fn mark_running(&self, task_id: &str) {
        let mut running = self.running.lock();
        running.insert(task_id.to_string(), ());
    }

    /// Remove task from running
    pub fn remove_running(&self, task_id: &str) {
        let mut running = self.running.lock();
        running.remove(task_id);
    }

    /// Get semaphore permit count
    pub fn available_permits(&self) -> usize {
        self.semaphore.available_permits()
    }

    /// Acquire a permit
    pub async fn acquire(&self) -> tokio::sync::SemaphorePermit<'_> {
        self.semaphore.acquire().await.expect("semaphore closed")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_priority_entry_ordering() {
        let low = PriorityEntry { task_id: "1".into(), priority: BatchTaskPriority::Low };
        let normal = PriorityEntry { task_id: "2".into(), priority: BatchTaskPriority::Normal };
        let high = PriorityEntry { task_id: "3".into(), priority: BatchTaskPriority::High };

        assert!(high > normal);
        assert!(normal > low);
    }
}
