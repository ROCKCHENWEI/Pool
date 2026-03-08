use anyhow::{Context, Result};
use reqwest::Client;
use serde_json::Value;
use std::collections::HashMap;

pub struct ComfyUIClient {
    base_url: String,
    client: Client,
}

impl ComfyUIClient {
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            client: Client::new(),
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub async fn get_system_stats(&self) -> Result<Value> {
        let url = format!("{}/system_stats", self.base_url);
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to connect to ComfyUI")?
            .json::<Value>()
            .await?;

        Ok(response)
    }

    pub async fn queue_prompt(&self, workflow: &HashMap<String, Value>) -> Result<String> {
        let url = format!("{}/prompt", self.base_url);
        let mut payload = serde_json::Map::new();
        payload.insert("prompt".to_string(), serde_json::to_value(workflow)?);

        let response: Value = self
            .client
            .post(&url)
            .json(&payload)
            .send()
            .await?
            .json()
            .await?;

        let prompt_id = response["prompt_id"]
            .as_str()
            .context("No prompt_id in response")?
            .to_string();

        Ok(prompt_id)
    }

    pub async fn get_history(&self, prompt_id: &str) -> Result<Value> {
        let url = format!("{}/history/{}", self.base_url, prompt_id);
        let response = self
            .client
            .get(&url)
            .send()
            .await?
            .json::<Value>()
            .await?;

        Ok(response)
    }

    pub async fn get_image(
        &self,
        filename: &str,
        subfolder: &str,
        r#type: &str,
    ) -> Result<Vec<u8>> {
        let url = format!("{}/view", self.base_url);
        let response = self
            .client
            .get(url)
            .query(&[("filename", filename), ("subfolder", subfolder), ("type", r#type)])
            .send()
            .await?
            .bytes()
            .await?;

        Ok(response.to_vec())
    }
}
