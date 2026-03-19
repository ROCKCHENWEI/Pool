use pool_core::api::{VideoGeneratorAdapter, VideoGenerationConfig, GenerationTask, TaskStatus};

struct MockProvider;

#[async_trait::async_trait]
impl VideoGeneratorAdapter for MockProvider {
    fn name(&self) -> &str {
        "mock"
    }

    async fn text_to_video(
        &self,
        prompt: &str,
        _negative_prompt: Option<&str>,
        _config: VideoGenerationConfig,
    ) -> anyhow::Result<GenerationTask> {
        Ok(GenerationTask {
            task_id: format!("task_{}", prompt.len()),
            status: TaskStatus::Pending,
            progress: 0.0,
        })
    }

    async fn image_to_video(
        &self,
        _image_data: &[u8],
        _prompt: &str,
        _config: VideoGenerationConfig,
    ) -> anyhow::Result<GenerationTask> {
        Ok(GenerationTask {
            task_id: "img_task".to_string(),
            status: TaskStatus::Pending,
            progress: 0.0,
        })
    }

    async fn get_task_status(&self, _task_id: &str) -> anyhow::Result<TaskStatus> {
        Ok(TaskStatus::Pending)
    }

    async fn cancel_task(&self, _task_id: &str) -> anyhow::Result<()> {
        Ok(())
    }

    async fn download_result(&self, _task_id: &str) -> anyhow::Result<Vec<u8>> {
        Ok(vec![])
    }
}

#[tokio::test]
async fn test_mock_provider() {
    let provider = MockProvider;
    let task = provider.text_to_video("test prompt", None, VideoGenerationConfig::default()).await.unwrap();
    assert_eq!(task.task_id, "task_11");
}
