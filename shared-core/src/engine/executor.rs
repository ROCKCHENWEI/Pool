use anyhow::Result;

use crate::models::Workflow;

use super::NodeEngine;

pub struct WorkflowExecutor {
    engine: NodeEngine,
}

impl WorkflowExecutor {
    pub fn new(workflow: &Workflow) -> Self {
        let mut engine = NodeEngine::new();

        for node in &workflow.nodes {
            engine.add_node(node.clone());
        }

        for conn in &workflow.connections {
            engine.add_connection(conn.clone());
        }

        Self { engine }
    }

    pub fn validate(&self) -> Result<Vec<String>> {
        self.engine.topological_sort()
    }

    pub async fn execute(&self) -> Result<()> {
        let execution_order = self.validate()?;

        for node_id in execution_order {
            self.execute_node(&node_id).await?;
        }

        Ok(())
    }

    async fn execute_node(&self, _node_id: &str) -> Result<()> {
        // TODO: Implement node execution logic
        Ok(())
    }
}
