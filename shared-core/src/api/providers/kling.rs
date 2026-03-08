use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use super::{VideoGeneratorAdapter, VideoGenerationConfig, GenerationTask, TaskStatus};

pub struct KlingAdapter {
    api_key: String,
    client: Client,
    base_url: String,
}

impl KlingAdapter {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: Client::new(),
            base_url: "https://api.klingai.com/v1".to_string(),
        }
    }
}

#[async_trait]
impl VideoGeneratorAdapter for KlingAdapter {
    fn name(&self) -> &str {
        "kling"
    }

    async fn text_to_video(
        &self,
        prompt: &str,
        negative_prompt: Option<&str>,
        config: VideoGenerationConfig,
    ) -> Result<GenerationTask> {
        let request = KlingRequest {
            prompt: prompt.to_string(),
            negative_prompt: negative_prompt.map(|s| s.to_string()),
            width: config.width,
            height: config.height,
            duration: config.duration_seconds,
            cfg_scale: config.cfg_scale,
            mode: "std".to_string(),
        };

        let response: KlingTaskResponse = self.client
            .post(format!("{}/videos/text2video", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request)
            .send()
            .await?
            .json()
            .await?;

        Ok(GenerationTask {
            task_id: response.task_id,
            status: TaskStatus::Pending,
            progress: 0.0,
        })
    }

    async fn image_to_video(
        &self,
        image_data: &[u8],
        prompt: &str,
        config: VideoGenerationConfig,
    ) -> Result<GenerationTask> {
        // TODO: Implement with multipart upload
        let _ = (image_data, prompt, config);
        anyhow::bail!("Not implemented")
    }

    async fn get_task_status(&self, task_id: &str) -> Result<TaskStatus> {
        let response: KlingStatusResponse = self.client
            .get(format!("{}/videos/text2video/{}", self.base_url, task_id))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await?
            .json()
            .await?;

        let status = match response.task_status.as_str() {
            "submitted" => TaskStatus::Pending,
            "processing" => TaskStatus::Processing,
            "succeed" => TaskStatus::Completed,
            "failed" => TaskStatus::Failed,
            _ => TaskStatus::Pending,
        };

        Ok(status)
    }

    async fn cancel_task(&self, _task_id: &str) -> Result<()> {
        anyhow::bail!("Kling API does not support task cancellation")
    }

    async fn download_result(&self, task_id: &str) -> Result<Vec<u8>> {
        let response: KlingStatusResponse = self.client
            .get(format!("{}/videos/text2video/{}", self.base_url, task_id))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await?
            .json()
            .await?;

        let video_url = response.task_result
            .and_then(|r| r.videos)
            .and_then(|v| v.into_iter().next())
            .map(|v| v.url)
            .ok_or_else(|| anyhow::anyhow!("No video URL in response"))?;

        let video_data = self.client
            .get(&video_url)
            .send()
            .await?
            .bytes()
            .await?;

        Ok(video_data.to_vec())
    }
}

#[derive(Serialize)]
struct KlingRequest {
    prompt: String,
    negative_prompt: Option<String>,
    width: u32,
    height: u32,
    duration: f32,
    cfg_scale: f32,
    mode: String,
}

#[derive(Deserialize)]
struct KlingTaskResponse {
    task_id: String,
}

#[derive(Deserialize)]
struct KlingStatusResponse {
    task_status: String,
    task_result: Option<KlingResult>,
}

#[derive(Deserialize)]
struct KlingResult {
    videos: Option<Vec<KlingVideo>>,
}

#[derive(Deserialize)]
struct KlingVideo {
    url: String,
}
