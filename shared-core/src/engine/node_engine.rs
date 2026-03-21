use anyhow::{bail, Result};
use std::collections::{HashMap, VecDeque};

use crate::models::{Connection, Node, NodeType};

pub struct NodeEngine {
    nodes: HashMap<String, Node>,
    connections: Vec<Connection>,
    adjacency: HashMap<String, Vec<String>>,
}

impl NodeEngine {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            connections: Vec::new(),
            adjacency: HashMap::new(),
        }
    }

    pub fn add_node(&mut self, node: Node) {
        let node_id = node.id.clone();
        self.nodes.insert(node_id.clone(), node);
        self.adjacency.insert(node_id, Vec::new());
    }

    pub fn add_connection(&mut self, connection: Connection) {
        self.connections.push(connection.clone());
        if let Some(adj) = self.adjacency.get_mut(&connection.from_node) {
            adj.push(connection.to_node);
        }
    }

    /// Kahn's algorithm for topological sort
    pub fn topological_sort(&self) -> Result<Vec<String>> {
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        let mut result = Vec::new();
        let mut queue = VecDeque::new();

        // Initialize in-degrees
        for node_id in self.nodes.keys() {
            in_degree.insert(node_id.clone(), 0);
        }

        // Calculate in-degrees
        for conn in &self.connections {
            *in_degree.get_mut(&conn.to_node).unwrap() += 1;
        }

        // Find nodes with no incoming edges
        for (node_id, &degree) in &in_degree {
            if degree == 0 {
                queue.push_back(node_id.clone());
            }
        }

        // Process queue
        while let Some(node_id) = queue.pop_front() {
            result.push(node_id.clone());

            if let Some(neighbors) = self.adjacency.get(&node_id) {
                for neighbor in neighbors {
                    let degree = in_degree.get_mut(neighbor).unwrap();
                    *degree -= 1;
                    if *degree == 0 {
                        queue.push_back(neighbor.clone());
                    }
                }
            }
        }

        // Check for cycle
        if result.len() != self.nodes.len() {
            bail!("Cycle detected in node graph");
        }

        Ok(result)
    }

    pub fn get_node(&self, id: &str) -> Option<&Node> {
        self.nodes.get(id)
    }

    pub fn get_connections(&self) -> &[Connection] {
        &self.connections
    }

    /// Convert the workflow to ComfyUI format
    pub fn to_comfyui_workflow(&self) -> HashMap<String, serde_json::Value> {
        let mut workflow = HashMap::new();

        for (index, node) in self.nodes.values().enumerate() {
            let node_json = self.node_to_comfyui_json(node, index);
            workflow.insert(index.to_string(), node_json);
        }

        workflow
    }

    /// Convert a single node to ComfyUI JSON format
    fn node_to_comfyui_json(&self, node: &Node, index: usize) -> serde_json::Value {
        use crate::models::NodeType;
        use serde_json::json;

        let class_type = match &node.node_type {
            NodeType::ComfyUITextEncode => "CLIPTextEncode",
            NodeType::ComfyUIKSampler => "KSampler",
            NodeType::ComfyUIVAEDecode => "VAEDecode",
            NodeType::ComfyUISaveImage => "SaveImage",
            NodeType::ComfyUILoadCheckpoint => "CheckpointLoaderSimple",
            NodeType::ComfyUIEmptyLatentImage => "EmptyLatentImage",
            NodeType::ComfyUIClipVisionEncode => "CLIPVisionEncode",
            NodeType::ComfyUIControlNetApply => "ControlNetApply",
            _ => "Unknown",
        };

        let mut inputs = serde_json::Map::new();

        // Add parameters based on node type
        match &node.node_type {
            NodeType::ComfyUITextEncode => {
                if let Some(text) = node.params.get("text") {
                    if let Some(s) = text.as_str() {
                        inputs.insert("text".to_string(), json!(s));
                    }
                }
            }
            NodeType::ComfyUIKSampler => {
                let seed = node.params.get("seed").and_then(|v| v.as_integer()).unwrap_or(0);
                let steps = node.params.get("steps").and_then(|v| v.as_integer()).unwrap_or(20);
                let cfg = node.params.get("cfg").and_then(|v| v.as_float()).unwrap_or(7.0);
                inputs.insert("seed".to_string(), json!(seed));
                inputs.insert("steps".to_string(), json!(steps));
                inputs.insert("cfg".to_string(), json!(cfg));
                inputs.insert("sampler_name".to_string(), json!("euler"));
                inputs.insert("scheduler".to_string(), json!("normal"));
                inputs.insert("denoise".to_string(), json!(1.0));
            }
            NodeType::ComfyUISaveImage => {
                inputs.insert("filename_prefix".to_string(), json!("PoolOutput"));
            }
            NodeType::ComfyUILoadCheckpoint => {
                if let Some(ckpt) = node.params.get("checkpoint") {
                    if let Some(s) = ckpt.as_str() {
                        inputs.insert("ckpt_name".to_string(), json!(s));
                    }
                }
            }
            NodeType::ComfyUIEmptyLatentImage => {
                let width = node.params.get("width").and_then(|v| v.as_integer()).unwrap_or(512);
                let height = node.params.get("height").and_then(|v| v.as_integer()).unwrap_or(512);
                inputs.insert("width".to_string(), json!(width));
                inputs.insert("height".to_string(), json!(height));
                inputs.insert("batch_size".to_string(), json!(1));
            }
            _ => {}
        }

        json!({
            "class_type": class_type,
            "inputs": inputs
        })
    }
}

impl Default for NodeEngine {
    fn default() -> Self {
        Self::new()
    }
}
