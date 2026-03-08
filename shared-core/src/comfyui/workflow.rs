use crate::models::{Node, NodeParam, NodeType, Workflow};
use serde_json::{json, Value};
use std::collections::HashMap;

pub struct WorkflowTranslator {
    node_counter: u32,
}

impl WorkflowTranslator {
    pub fn new() -> Self {
        Self { node_counter: 0 }
    }

    pub fn translate(&mut self, workflow: &Workflow) -> HashMap<String, Value> {
        let mut comfy_workflow = HashMap::new();

        for node in &workflow.nodes {
            let comfy_node = self.translate_node(node);
            let node_id = self.node_counter.to_string();
            self.node_counter += 1;
            comfy_workflow.insert(node_id, comfy_node);
        }

        comfy_workflow
    }

    fn translate_node(&self, node: &Node) -> Value {
        match node.node_type {
            NodeType::TextPrompt => self.translate_text_prompt(node),
            NodeType::VISCCore => self.translate_visc(node),
            NodeType::SuperResolution => self.translate_sr(node),
            NodeType::Output => self.translate_output(node),
            _ => json!({ "class_type": "unknown" }),
        }
    }

    fn translate_text_prompt(&self, node: &Node) -> Value {
        let prompt = node
            .params
            .get("prompt")
            .and_then(|p| match p {
                NodeParam::String(s) => Some(s.clone()),
                _ => None,
            })
            .unwrap_or_default();

        json!({
            "class_type": "CLIPTextEncode",
            "inputs": {
                "text": prompt,
                "clip": ["2", 1]
            }
        })
    }

    fn translate_visc(&self, node: &Node) -> Value {
        json!({
            "class_type": "KSampler",
            "inputs": {
                "seed": node.params.get("seed").and_then(|p| match p {
                    NodeParam::Integer(i) => Some(*i),
                    _ => None,
                }).unwrap_or(0),
                "steps": 30,
                "cfg": 7.0,
                "sampler_name": "dpmpp_2m",
                "scheduler": "karras",
                "denoise": 1.0,
                "model": ["2", 0],
                "positive": ["1", 0],
                "negative": ["3", 0],
                "latent_image": ["4", 0]
            }
        })
    }

    fn translate_sr(&self, _node: &Node) -> Value {
        json!({
            "class_type": "ImageScale",
            "inputs": {
                "upscale_method": "nearest-exact",
                "width": 2048,
                "height": 2048,
                "image": ["5", 0]
            }
        })
    }

    fn translate_output(&self, _node: &Node) -> Value {
        json!({
            "class_type": "SaveImage",
            "inputs": {
                "filename_prefix": "Pool Output",
                "images": ["6", 0]
            }
        })
    }
}

impl Default for WorkflowTranslator {
    fn default() -> Self {
        Self::new()
    }
}
