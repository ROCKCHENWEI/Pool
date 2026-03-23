mod kling;
mod vidu;
mod hailuo;
mod seedance;
mod automatic1111;
mod ollama;

pub use kling::KlingAdapter;
pub use vidu::ViduAdapter;
pub use hailuo::HailuoAdapter;
pub use seedance::SeedanceAdapter;
pub use automatic1111::{Automatic1111Adapter, Txt2ImgRequest};
pub use ollama::{OllamaAdapter, ChatRequest, ChatMessage};

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[async_trait::async_trait]
pub trait VideoGeneratorAdapter {
    fn name(&self) -> &str;

    async fn text_to_video(
        &self,
        prompt: &str,
        negative_prompt: Option<&str>,
        config: VideoGenerationConfig,
    ) -> Result<GenerationTask>;

    async fn image_to_video(
        &self,
        image_data: &[u8],
        prompt: &str,
        config: VideoGenerationConfig,
    ) -> Result<GenerationTask>;

    async fn get_task_status(&self, task_id: &str) -> Result<TaskStatus>;

    async fn cancel_task(&self, task_id: &str) -> Result<()>;

    async fn download_result(&self, task_id: &str) -> Result<Vec<u8>>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoGenerationConfig {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub duration_seconds: f32,
    pub seed: Option<i64>,
    pub cfg_scale: f32,
    pub steps: u32,
}

impl Default for VideoGenerationConfig {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            fps: 24,
            duration_seconds: 4.0,
            seed: None,
            cfg_scale: 7.0,
            steps: 30,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationTask {
    pub task_id: String,
    pub status: TaskStatus,
    pub progress: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaskStatus {
    Pending,
    Processing,
    Completed,
    Failed,
    Cancelled,
}
