//! Ollama Adapter
//!
//! Provides integration with Ollama for local LLM inference
//! to support prompt enhancement and text generation features.

use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use tokio_stream::StreamExt;

/// Ollama adapter for local LLM inference
pub struct OllamaAdapter {
    client: Client,
    base_url: String,
}

impl OllamaAdapter {
    /// Create a new adapter with the given base URL
    pub fn new(base_url: &str) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(300))
            .build()
            .unwrap_or_default();

        let base_url = base_url.trim_end_matches('/').to_string();

        Self { client, base_url }
    }

    /// Check if Ollama is running
    pub async fn health_check(&self) -> Result<bool> {
        let url = format!("{}/api/tags", self.base_url);
        let response = self.client.get(&url).send().await?;
        Ok(response.status().is_success())
    }

    /// Get list of available models
    pub async fn list_models(&self) -> Result<Vec<OllamaModel>> {
        let url = format!("{}/api/tags", self.base_url);
        let response = self.client.get(&url).send().await?;

        let result: TagsResponse = response.json().await?;
        Ok(result.models)
    }

    /// Get model information
    pub async fn get_model_info(&self, model_name: &str) -> Result<ModelInfo> {
        let url = format!("{}/api/show", self.base_url);
        let payload = serde_json::json!({ "name": model_name });

        let response = self.client.post(&url).json(&payload).send().await?;

        if response.status().is_success() {
            let info: ModelInfo = response.json().await?;
            Ok(info)
        } else {
            Err(anyhow!("Failed to get model info"))
        }
    }

    /// Generate completion (non-streaming)
    pub async fn generate(&self, request: GenerateRequest) -> Result<GenerateResponse> {
        let url = format!("{}/api/generate", self.base_url);
        let response = self.client.post(&url).json(&request).send().await?;

        if response.status().is_success() {
            let result: GenerateResponse = response.json().await?;
            Ok(result)
        } else {
            let error_text = response.text().await?;
            Err(anyhow!("Generate failed: {}", error_text))
        }
    }

    /// Generate chat completion (non-streaming)
    pub async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        let url = format!("{}/api/chat", self.base_url);

        // Request non-streaming response
        let mut request = request;
        request.stream = Some(false);

        let response = self.client.post(&url).json(&request).send().await?;

        if response.status().is_success() {
            let result: ChatResponse = response.json().await?;
            Ok(result)
        } else {
            let error_text = response.text().await?;
            Err(anyhow!("Chat failed: {}", error_text))
        }
    }

    /// Generate embeddings
    pub async fn embeddings(&self, request: EmbeddingRequest) -> Result<EmbeddingResponse> {
        let url = format!("{}/api/embeddings", self.base_url);
        let response = self.client.post(&url).json(&request).send().await?;

        if response.status().is_success() {
            let result: EmbeddingResponse = response.json().await?;
            Ok(result)
        } else {
            let error_text = response.text().await?;
            Err(anyhow!("Embeddings failed: {}", error_text))
        }
    }

    /// Pull a model
    pub async fn pull_model(&self, model_name: &str, stream: bool) -> Result<()> {
        let url = format!("{}/api/pull", self.base_url);
        let payload = serde_json::json!({
            "name": model_name,
            "stream": stream
        });

        let response = self.client.post(&url).json(&payload).send().await?;

        if response.status().is_success() {
            Ok(())
        } else {
            let error_text = response.text().await?;
            Err(anyhow!("Pull failed: {}", error_text))
        }
    }

    /// Delete a model
    pub async fn delete_model(&self, model_name: &str) -> Result<()> {
        let url = format!("{}/api/delete", self.base_url);
        let payload = serde_json::json!({ "name": model_name });

        let response = self.client.delete(&url).json(&payload).send().await?;

        if response.status().is_success() {
            Ok(())
        } else {
            let error_text = response.text().await?;
            Err(anyhow!("Delete failed: {}", error_text))
        }
    }

    /// Create a model from a Modelfile
    pub async fn create_model(&self, name: &str, modelfile: &str) -> Result<()> {
        let url = format!("{}/api/create", self.base_url);
        let payload = serde_json::json!({
            "name": name,
            "modelfile": modelfile,
            "stream": false
        });

        let response = self.client.post(&url).json(&payload).send().await?;

        if response.status().is_success() {
            Ok(())
        } else {
            let error_text = response.text().await?;
            Err(anyhow!("Create failed: {}", error_text))
        }
    }

    /// Enhance a prompt using an LLM
    pub async fn enhance_prompt(
        &self,
        model: &str,
        base_prompt: &str,
        style: Option<&str>,
    ) -> Result<String> {
        let style_hint = style.unwrap_or("detailed and vivid");

        let system_prompt = format!(
            "You are a prompt enhancement assistant. Take the user's basic prompt and \
             enhance it to be more {}, descriptive, and suitable for AI image generation. \
             Return ONLY the enhanced prompt without any explanations or quotes.",
            style_hint
        );

        let request = ChatRequest {
            model: model.to_string(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: format!(
                    "Enhance this prompt for AI image generation: {}",
                    base_prompt
                ),
                images: None,
            }],
            stream: Some(false),
            format: None,
            options: Some(HashMap::from([
                ("temperature".to_string(), serde_json::json!(0.7)),
                ("top_p".to_string(), serde_json::json!(0.9)),
            ])),
            template: None,
        };

        let response = self.chat(request).await?;
        Ok(response.message.content.trim().to_string())
    }
}

// ============================================================================
// Data Structures
// ============================================================================

/// Model list response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagsResponse {
    pub models: Vec<OllamaModel>,
}

/// Ollama model information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaModel {
    pub name: String,
    pub modified_at: Option<String>,
    pub size: Option<i64>,
    pub digest: Option<String>,
    pub details: Option<ModelDetails>,
}

/// Model details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelDetails {
    pub format: Option<String>,
    pub family: Option<String>,
    pub parameter_size: Option<String>,
    pub quantization_level: Option<String>,
}

/// Detailed model information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub license: Option<String>,
    pub modelfile: Option<String>,
    pub parameters: Option<String>,
    pub template: Option<String>,
    pub details: Option<ModelDetails>,
    pub model_info: Option<HashMap<String, serde_json::Value>>,
}

/// Generate request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateRequest {
    pub model: String,
    pub prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub images: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<HashMap<String, serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<Vec<i32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keep_alive: Option<String>,
}

/// Generate response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateResponse {
    pub model: String,
    pub created_at: Option<String>,
    pub response: String,
    pub done: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<Vec<i32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_duration: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub load_duration: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_eval_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_eval_duration: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eval_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eval_duration: Option<i64>,
}

/// Chat request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<HashMap<String, serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
}

/// Chat message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub images: Option<Vec<String>>,
}

/// Chat response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub model: String,
    pub created_at: String,
    pub message: ChatMessage,
    pub done: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_duration: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub load_duration: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_eval_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_eval_duration: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eval_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eval_duration: Option<i64>,
}

/// Embedding request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingRequest {
    pub model: String,
    pub prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<HashMap<String, serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keep_alive: Option<String>,
}

/// Embedding response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingResponse {
    pub embedding: Vec<f32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_creation() {
        let adapter = OllamaAdapter::new("http://localhost:11434");
        assert_eq!(adapter.base_url, "http://localhost:11434");
    }

    #[test]
    fn test_generate_request_serialization() {
        let request = GenerateRequest {
            model: "llama2".to_string(),
            prompt: "Hello".to_string(),
            images: None,
            format: None,
            options: None,
            system: None,
            template: None,
            context: None,
            stream: Some(false),
            raw: None,
            keep_alive: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("llama2"));
        assert!(json.contains("Hello"));
    }

    #[test]
    fn test_chat_request_serialization() {
        let request = ChatRequest {
            model: "llama2".to_string(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: "Hello".to_string(),
                images: None,
            }],
            stream: Some(false),
            format: None,
            options: None,
            template: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("user"));
        assert!(json.contains("Hello"));
    }
}
