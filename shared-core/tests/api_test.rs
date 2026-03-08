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
    ) -> Result<GenerationTask, anyhow::Error> {
        Ok(GenerationTask {
            task_id: format!("task_{}", prompt.len()),
            status: TaskStatus::Pending,
            progress: 0.0,
        })
    }
}

#[tokio::test]
async fn test_mock_provider() {
    let provider = MockProvider;
    let task = provider.text_to_video("test prompt", None, VideoGenerationConfig::default()).await.unwrap();
    assert_eq!(task.task_id, "task_11");
}
