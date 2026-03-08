use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workflow {
    pub id: String,
    pub shot_id: String,
    pub name: String,
    pub nodes: Vec<Node>,
    pub connections: Vec<Connection>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    pub node_type: NodeType,
    pub position: (f32, f32),
    pub params: HashMap<String, NodeParam>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeType {
    TextPrompt,
    VISCCore,
    SuperResolution,
    HDR,
    ColorGrade,
    Subtitle,
    Output,
    ComfyUI,
    APIProvider,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeParam {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Array(Vec<NodeParam>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Connection {
    pub from_node: String,
    pub from_slot: i32,
    pub to_node: String,
    pub to_slot: i32,
}

impl Workflow {
    pub fn new(name: String, shot_id: String) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            shot_id,
            name,
            nodes: Vec::new(),
            connections: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }
}
