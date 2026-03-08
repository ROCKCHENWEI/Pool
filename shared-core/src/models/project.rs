use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::Shot;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub shots: Vec<Shot>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Project {
    pub fn new(name: String) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            name,
            description: None,
            shots: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }

    pub fn add_shot(&mut self, shot: Shot) {
        self.shots.push(shot);
        self.updated_at = Utc::now();
    }
}
