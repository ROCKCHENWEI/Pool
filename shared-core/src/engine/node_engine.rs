use anyhow::{bail, Result};
use std::collections::{HashMap, VecDeque};

use crate::models::{Connection, Node};

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
        self.nodes.insert(node.id.clone(), node);
        self.adjacency.insert(node.id.clone(), Vec::new());
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
}

impl Default for NodeEngine {
    fn default() -> Self {
        Self::new()
    }
}
