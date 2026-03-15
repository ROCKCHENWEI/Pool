use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};

pub struct FeishuBot {
    app_id: String,
    app_secret: String,
    client: Client,
    base_url: String,
}

impl FeishuBot {
    pub fn new(app_id: String, app_secret: String) -> Self {
        Self { app_id, app_secret, client: Client::new(), base_url: "https://open.feishu.cn/open-apis".to_string() }
    }

    pub fn app_id(&self) -> &str { &self.app_id }

    pub async fn send_message(&self, _chat_id: &str, _content: &str) -> Result<String> {
        Ok(format!("msg_{}", _chat_id))
    }
}
