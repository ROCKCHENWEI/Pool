use anyhow::{bail, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

use crate::models::{RuntimeEvent, RuntimeEventLevel, RuntimeTask, TaskStatus};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskQueueSnapshot {
    pub queued: Vec<RuntimeTask>,
    pub events: Vec<RuntimeEvent>,
}

#[derive(Debug, Default)]
pub struct TaskQueue {
    queued: VecDeque<RuntimeTask>,
    events: Vec<RuntimeEvent>,
}

impl TaskQueue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, mut task: RuntimeTask) {
        if task.status == TaskStatus::Queued && task.requires_approval {
            task.status = TaskStatus::WaitingApproval;
        }
        self.events.push(RuntimeEvent::new(
            task.project_slug.clone(),
            RuntimeEventLevel::Info,
            format!("queued task: {}", task.title),
        ));
        self.queued.push_back(task);
    }

    pub fn approve(&mut self, task_id: &str) -> Result<()> {
        let task = self
            .queued
            .iter_mut()
            .find(|task| task.id == task_id)
            .ok_or_else(|| anyhow::anyhow!("task not found: {task_id}"))?;

        if task.status != TaskStatus::WaitingApproval {
            bail!("task is not waiting for approval");
        }

        task.status = TaskStatus::Ready;
        task.updated_at = Utc::now();
        self.events.push(RuntimeEvent::new(
            task.project_slug.clone(),
            RuntimeEventLevel::Ok,
            format!("approved task: {}", task.title),
        ));
        Ok(())
    }

    pub fn next_ready(&mut self) -> Option<RuntimeTask> {
        let index = self
            .queued
            .iter()
            .position(|task| matches!(task.status, TaskStatus::Queued | TaskStatus::Ready))?;
        let mut task = self.queued.remove(index)?;
        task.status = TaskStatus::Running;
        task.updated_at = Utc::now();
        Some(task)
    }

    pub fn snapshot(&self) -> TaskQueueSnapshot {
        TaskQueueSnapshot {
            queued: self.queued.iter().cloned().collect(),
            events: self.events.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn approval_gate_blocks_task_until_approved() {
        let mut queue = TaskQueue::new();
        let task = RuntimeTask::new("demo", "expensive 3DGS").with_approval_gate(true);
        let task_id = task.id.clone();
        queue.push(task);

        assert!(queue.next_ready().is_none());
        queue.approve(&task_id).unwrap();
        assert_eq!(queue.next_ready().unwrap().status, TaskStatus::Running);
    }
}
