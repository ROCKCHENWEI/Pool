use std::collections::{BTreeMap, VecDeque};
use thiserror::Error;

use crate::models::{Workflow, WorkflowConnection, WorkflowNode};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum NodeEngineError {
    #[error("connection references missing node: {0}")]
    MissingNode(String),
    #[error("workflow contains a cycle")]
    CycleDetected,
}

pub struct NodeEngine;

impl NodeEngine {
    pub fn execution_order(workflow: &Workflow) -> Result<Vec<WorkflowNode>, NodeEngineError> {
        let mut incoming: BTreeMap<String, usize> = workflow
            .nodes
            .keys()
            .map(|node_id| (node_id.clone(), 0))
            .collect();
        let mut outgoing: BTreeMap<String, Vec<&WorkflowConnection>> = BTreeMap::new();

        for connection in &workflow.connections {
            if !workflow.nodes.contains_key(&connection.from_node_id) {
                return Err(NodeEngineError::MissingNode(
                    connection.from_node_id.clone(),
                ));
            }
            if !workflow.nodes.contains_key(&connection.to_node_id) {
                return Err(NodeEngineError::MissingNode(connection.to_node_id.clone()));
            }
            *incoming.entry(connection.to_node_id.clone()).or_insert(0) += 1;
            outgoing
                .entry(connection.from_node_id.clone())
                .or_default()
                .push(connection);
        }

        let mut ready: VecDeque<String> = incoming
            .iter()
            .filter_map(|(node_id, count)| (*count == 0).then(|| node_id.clone()))
            .collect();
        let mut ordered = Vec::with_capacity(workflow.nodes.len());

        while let Some(node_id) = ready.pop_front() {
            if let Some(node) = workflow.nodes.get(&node_id) {
                ordered.push(node.clone());
            }

            for connection in outgoing.get(&node_id).into_iter().flatten() {
                let count = incoming
                    .get_mut(&connection.to_node_id)
                    .expect("connection target was validated");
                *count -= 1;
                if *count == 0 {
                    ready.push_back(connection.to_node_id.clone());
                }
            }
        }

        if ordered.len() != workflow.nodes.len() {
            return Err(NodeEngineError::CycleDetected);
        }

        Ok(ordered)
    }

    pub fn validate_acyclic(workflow: &Workflow) -> Result<(), NodeEngineError> {
        Self::execution_order(workflow).map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ConnectionKind, NodeType, WorkflowNode};

    #[test]
    fn sorts_workflow_topology() {
        let mut workflow = Workflow::new("demo", "content burst");
        let input = workflow.add_node(WorkflowNode::new("input", NodeType::Input));
        let gs = workflow.add_node(WorkflowNode::new("3dgs", NodeType::ThreeDgs));
        let unreal = workflow.add_node(WorkflowNode::new("unreal", NodeType::Unreal));

        workflow.connect(&input, &gs, ConnectionKind::AssetFlow, "source to 3DGS");
        workflow.connect(&gs, &unreal, ConnectionKind::ControlFlow, "import assets");

        let titles: Vec<_> = NodeEngine::execution_order(&workflow)
            .unwrap()
            .into_iter()
            .map(|node| node.title)
            .collect();

        assert_eq!(titles, vec!["input", "3dgs", "unreal"]);
    }

    #[test]
    fn detects_cycles() {
        let mut workflow = Workflow::new("demo", "cycle");
        let a = workflow.add_node(WorkflowNode::new("a", NodeType::Input));
        let b = workflow.add_node(WorkflowNode::new("b", NodeType::Agent));

        workflow.connect(&a, &b, ConnectionKind::AssetFlow, "a to b");
        workflow.connect(&b, &a, ConnectionKind::FeedbackLoop, "b to a");

        assert_eq!(
            NodeEngine::validate_acyclic(&workflow),
            Err(NodeEngineError::CycleDetected)
        );
    }
}
