use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResource {
    pub uri: String,
    pub name: String,
    pub description: String,
}

pub struct McpServer {
    resources: Vec<McpResource>,
}

impl McpServer {
    pub fn new() -> Self {
        Self {
            resources: vec![
                McpResource { uri: "pool://status".to_string(), name: "Pool Status".to_string(), description: "Current status".to_string() },
                McpResource { uri: "pool://shots".to_string(), name: "Shots".to_string(), description: "Shot list".to_string() },
            ],
        }
    }

    pub fn list_resources(&self) -> &[McpResource] { &self.resources }

    pub fn read_resource(&self, uri: &str) -> Result<String> {
        match uri {
            "pool://status" => Ok(r#"{"status":"idle"}"#.to_string()),
            _ => bail!("Unknown resource: {}", uri),
        }
    }
}

impl Default for McpServer {
    fn default() -> Self { Self::new() }
}
