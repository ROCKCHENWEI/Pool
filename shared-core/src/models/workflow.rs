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
    // ComfyUI 节点类型
    ComfyUITextEncode,
    ComfyUIKSampler,
    ComfyUIVAEDecode,
    ComfyUISaveImage,
    ComfyUILoadCheckpoint,
    ComfyUIEmptyLatentImage,
    ComfyUIClipVisionEncode,
    ComfyUIControlNetApply,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeParam {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Array(Vec<NodeParam>),
}

impl NodeParam {
    /// Get as integer, returns None if not an Integer variant
    pub fn as_integer(&self) -> Option<i64> {
        match self {
            NodeParam::Integer(i) => Some(*i),
            _ => None,
        }
    }

    /// Get as float, returns None if not a Float variant
    pub fn as_float(&self) -> Option<f64> {
        match self {
            NodeParam::Float(f) => Some(*f),
            NodeParam::Integer(i) => Some(*i as f64),
            _ => None,
        }
    }

    /// Get as string reference, returns None if not a String variant
    pub fn as_str(&self) -> Option<&str> {
        match self {
            NodeParam::String(s) => Some(s),
            _ => None,
        }
    }

    /// Get as boolean, returns None if not a Boolean variant
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            NodeParam::Boolean(b) => Some(*b),
            _ => None,
        }
    }
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
