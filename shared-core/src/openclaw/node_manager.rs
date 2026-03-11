use anyhow::{Result, bail};
use std::collections::HashMap;
use crate::models::{Node, Workflow};

pub struct NodeManager {
    nodes: HashMap<String, Node>,
    workflows: HashMap<String, Workflow>,
}

impl NodeManager {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            workflows: HashMap::new(),
        }
    }

    pub fn create_node(&mut self, node: Node) -> Result<()> {
        if self.nodes.contains_key(&node.id) {
            bail!("Node with id {} already exists", node.id);
        }
        self.nodes.insert(node.id.clone(), node);
        Ok(())
    }

    pub fn get_node(&self, id: &str) -> Option<&Node> {
        self.nodes.get(id)
    }

    pub fn update_node(&mut self, node: Node) -> Result<()> {
        if !self.nodes.contains_key(&node.id) {
            bail!("Node with id {} not found", node.id);
        }
        self.nodes.insert(node.id.clone(), node);
        Ok(())
    }

    pub fn delete_node(&mut self, id: &str) -> Result<()> {
        if self.nodes.remove(id).is_none() {
            bail!("Node with id {} not found", id);
        }
        Ok(())
    }

    pub fn list_nodes(&self) -> Vec<&Node> {
        self.nodes.values().collect()
    }

    pub fn create_workflow(&mut self, workflow: Workflow) -> Result<()> {
        if self.workflows.contains_key(&workflow.id) {
            bail!("Workflow with id {} already exists", workflow.id);
        }
        self.workflows.insert(workflow.id.clone(), workflow);
        Ok(())
    }

    pub fn get_workflow(&self, id: &str) -> Option<&Workflow> {
        self.workflows.get(id)
    }
}

impl Default for NodeManager {
    fn default() -> Self {
        Self::new()
    }
}
