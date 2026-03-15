use anyhow::Result;
use std::collections::HashMap;
use super::providers::{VideoGeneratorAdapter, VideoGenerationConfig, GenerationTask};

pub struct ApiGateway {
    providers: HashMap<String, Box<dyn VideoGeneratorAdapter + Send + Sync>>,
}

impl ApiGateway {
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
        }
    }

    pub fn register_provider(&mut self, provider: Box<dyn VideoGeneratorAdapter + Send + Sync>) {
        self.providers.insert(provider.name().to_string(), provider);
    }

    pub fn get_provider(&self, name: &str) -> Option<&(dyn VideoGeneratorAdapter + Send + Sync)> {
        self.providers.get(name).map(|p| p.as_ref())
    }

    pub async fn text_to_video(
        &self,
        provider_name: &str,
        prompt: &str,
        negative_prompt: Option<&str>,
        config: VideoGenerationConfig,
    ) -> Result<GenerationTask> {
        let provider = self.providers.get(provider_name)
            .ok_or_else(|| anyhow::anyhow!("Provider not found: {}", provider_name))?;

        provider.text_to_video(prompt, negative_prompt, config).await
    }
}

impl Default for ApiGateway {
    fn default() -> Self {
        Self::new()
    }
}
