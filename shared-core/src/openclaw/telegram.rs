use anyhow::Result;
use reqwest::Client;

pub struct TelegramBot {
    token: String,
    client: Client,
    base_url: String,
}

impl TelegramBot {
    pub fn new(token: String) -> Self {
        Self { token, client: Client::new(), base_url: "https://api.telegram.org".to_string() }
    }

    pub fn token(&self) -> &str { &self.token }

    pub async fn send_message(&self, _chat_id: i64, _text: &str) -> Result<i32> {
        Ok(1)
    }
}
