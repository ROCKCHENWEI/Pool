use anyhow::Result;

#[derive(Debug, Clone, PartialEq)]
pub enum LlmProvider {
    Zhipu,
    Minimax,
    Kimi,
    Claude,
}

pub struct LlmBar {
    provider: LlmProvider,
    api_key: Option<String>,
}

impl LlmBar {
    pub fn new(provider: LlmProvider) -> Self {
        Self { provider, api_key: None }
    }

    pub fn provider(&self) -> LlmProvider { self.provider.clone() }

    pub fn set_api_key(&mut self, api_key: String) { self.api_key = Some(api_key); }

    pub async fn enhance_prompt(&self, prompt: &str) -> Result<String> {
        Ok(format!("Enhanced: {}", prompt))
    }
}
