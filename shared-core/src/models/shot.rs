use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::Workflow;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Shot {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub position: i32,
    pub duration: f64,
    pub workflow: Option<Workflow>,
    pub status: ShotStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ShotStatus {
    Idle,
    Pending,
    Processing,
    Completed,
    Failed,
}

impl Shot {
    pub fn new(name: String) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            project_id: String::new(),
            name,
            position: 0,
            duration: 0.0,
            workflow: None,
            status: ShotStatus::Idle,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn with_project(mut self, project_id: String) -> Self {
        self.project_id = project_id;
        self
    }
}
