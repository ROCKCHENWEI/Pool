use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use super::{VideoGeneratorAdapter, VideoGenerationConfig, GenerationTask, TaskStatus};

pub struct HailuoAdapter {
    api_key: String,
    client: Client,
    base_url: String,
}

impl HailuoAdapter {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: Client::new(),
            base_url: "https://api.minimax.chat/v1".to_string(),
        }
    }
}

#[async_trait]
impl VideoGeneratorAdapter for HailuoAdapter {
    fn name(&self) -> &str { "hailuo" }

    async fn text_to_video(&self, _prompt: &str, _negative_prompt: Option<&str>, _config: VideoGenerationConfig) -> Result<GenerationTask> {
        Ok(GenerationTask { task_id: format!("hailuo_{}", uuid::Uuid::new_v4()), status: TaskStatus::Pending, progress: 0.0 })
    }

    async fn image_to_video(&self, _image_data: &[u8], _prompt: &str, _config: VideoGenerationConfig) -> Result<GenerationTask> {
        Ok(GenerationTask { task_id: format!("hailuo_{}", uuid::Uuid::new_v4()), status: TaskStatus::Pending, progress: 0.0 })
    }

    async fn get_task_status(&self, _task_id: &str) -> Result<TaskStatus> {
        Ok(TaskStatus::Pending)
    }

    async fn cancel_task(&self, _task_id: &str) -> Result<()> {
        anyhow::bail!("Hailuo does not support cancellation")
    }

    async fn download_result(&self, _task_id: &str) -> Result<Vec<u8>> {
        anyhow::bail!("Not ready")
    }
}
