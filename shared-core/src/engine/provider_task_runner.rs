use anyhow::Result;
use serde::Serialize;

use crate::db::RuntimeRepository;
use crate::models::{AssetRecord, RuntimeEvent, RuntimeEventLevel, RuntimeTask, TaskStatus};
use crate::providers::{ProviderAdapter, ProviderJob, ProviderRequest};

#[derive(Debug, Clone, Serialize)]
pub struct ProviderTaskRunReport {
    pub task_id: String,
    pub provider_id: String,
    pub status: TaskStatus,
    pub job: Option<ProviderJob>,
    pub assets: Vec<AssetRecord>,
}

pub struct ProviderTaskRunner<'a> {
    repository: &'a RuntimeRepository,
}

impl<'a> ProviderTaskRunner<'a> {
    pub fn new(repository: &'a RuntimeRepository) -> Self {
        Self { repository }
    }

    pub async fn run<A>(
        &self,
        adapter: &A,
        mut task: RuntimeTask,
        request: ProviderRequest,
    ) -> Result<ProviderTaskRunReport>
    where
        A: ProviderAdapter,
    {
        self.repository.insert_task(&task)?;

        if task.requires_approval && task.status == TaskStatus::WaitingApproval {
            self.repository.insert_event(&RuntimeEvent::new(
                task.project_slug.clone(),
                RuntimeEventLevel::Warn,
                format!("task waiting for approval: {}", task.title),
            ))?;
            return Ok(ProviderTaskRunReport {
                task_id: task.id,
                provider_id: adapter.config().id.clone(),
                status: TaskStatus::WaitingApproval,
                job: None,
                assets: Vec::new(),
            });
        }

        task.status = TaskStatus::Running;
        self.repository
            .update_task_status(&task.id, TaskStatus::Running)?;
        self.repository.insert_event(&RuntimeEvent::new(
            task.project_slug.clone(),
            RuntimeEventLevel::Info,
            format!("provider task started: {}", task.title),
        ))?;

        let health = adapter.health().await?;
        self.repository.insert_event(&RuntimeEvent::new(
            task.project_slug.clone(),
            RuntimeEventLevel::Info,
            format!("provider health {}: {}", health.provider_id, health.status),
        ))?;

        let job = adapter.submit(request).await?;
        self.repository.update_task_request_metadata_path(
            &task.id,
            Some(job.request_metadata_path.as_str()),
        )?;
        self.repository.insert_event(&RuntimeEvent::new(
            task.project_slug.clone(),
            RuntimeEventLevel::Info,
            format!(
                "provider job submitted: {}",
                job.external_job_id.as_deref().unwrap_or("local")
            ),
        ))?;

        let status = adapter.poll(&job).await?;
        self.repository
            .update_task_status(&task.id, status.clone())?;

        let mut assets = Vec::new();
        if status == TaskStatus::Succeeded {
            let local_paths = adapter.download(&job).await?;
            assets = self.repository.index_local_outputs(
                &task.project_slug,
                task.node_id.as_deref(),
                provider_url(&job).as_deref(),
                &local_paths,
            )?;
            self.repository.insert_event(&RuntimeEvent::new(
                task.project_slug.clone(),
                RuntimeEventLevel::Ok,
                format!("provider task succeeded: {} assets indexed", assets.len()),
            ))?;
        } else if status == TaskStatus::Failed {
            self.repository.insert_event(&RuntimeEvent::new(
                task.project_slug.clone(),
                RuntimeEventLevel::Error,
                format!("provider task failed: {}", task.title),
            ))?;
        } else {
            self.repository.insert_event(&RuntimeEvent::new(
                task.project_slug.clone(),
                RuntimeEventLevel::Info,
                format!("provider task pending: {:?}", status),
            ))?;
        }

        Ok(ProviderTaskRunReport {
            task_id: task.id,
            provider_id: adapter.config().id.clone(),
            status,
            job: Some(job),
            assets,
        })
    }

    pub fn record_progress_event(&self, event: RuntimeEvent) -> Result<()> {
        self.repository.insert_event(&event)
    }

    pub fn record_progress_events(
        &self,
        events: impl IntoIterator<Item = RuntimeEvent>,
    ) -> Result<()> {
        for event in events {
            self.record_progress_event(event)?;
        }
        Ok(())
    }
}

fn provider_url(job: &ProviderJob) -> Option<String> {
    job.metadata_json
        .as_ref()
        .and_then(|metadata| metadata.get("history_url"))
        .and_then(serde_json::Value::as_str)
        .map(ToString::to_string)
        .or_else(|| {
            job.external_job_id
                .as_ref()
                .map(|id| format!("provider-job://{id}"))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::RuntimeRepository;
    use crate::models::RuntimeTask;
    use crate::providers::{Mock3dgsProvider, ProviderRequest};

    #[tokio::test]
    async fn blocks_provider_task_waiting_for_approval() {
        let repository = RuntimeRepository::in_memory().unwrap();
        repository.migrate().unwrap();
        let runner = ProviderTaskRunner::new(&repository);
        let provider = Mock3dgsProvider::new("mock-3dgs", "Mock 3DGS");
        let task = RuntimeTask::new("demo", "3DGS high cost").with_approval_gate(true);

        let report = runner
            .run(
                &provider,
                task,
                ProviderRequest {
                    project_slug: "demo".to_string(),
                    prompt: "make world".to_string(),
                    input_paths: vec!["source/plate.png".to_string()],
                    output_dir: "worlds/demo/output".to_string(),
                    require_approval: true,
                },
            )
            .await
            .unwrap();

        assert_eq!(report.status, TaskStatus::WaitingApproval);
        assert!(report.job.is_none());
        assert_eq!(repository.stats().unwrap().tasks, 1);
        assert_eq!(repository.stats().unwrap().events, 1);
    }

    #[tokio::test]
    async fn runs_mock_provider_and_indexes_outputs() {
        let repository = RuntimeRepository::in_memory().unwrap();
        repository.migrate().unwrap();
        let runner = ProviderTaskRunner::new(&repository);
        let provider = Mock3dgsProvider::new("mock-3dgs", "Mock 3DGS");
        let mut task = RuntimeTask::new("demo", "3DGS approved");
        task.node_id = Some("node-3dgs".to_string());

        let report = runner
            .run(
                &provider,
                task,
                ProviderRequest {
                    project_slug: "demo".to_string(),
                    prompt: "make world".to_string(),
                    input_paths: vec!["source/plate.png".to_string()],
                    output_dir: "worlds/demo/output".to_string(),
                    require_approval: false,
                },
            )
            .await
            .unwrap();

        assert_eq!(report.status, TaskStatus::Succeeded);
        assert_eq!(report.assets.len(), 3);
        assert_eq!(repository.stats().unwrap().assets, 3);
    }

    #[test]
    fn records_external_progress_events_through_runner() {
        let repository = RuntimeRepository::in_memory().unwrap();
        repository.migrate().unwrap();
        let runner = ProviderTaskRunner::new(&repository);

        runner
            .record_progress_event(RuntimeEvent::new(
                "demo",
                RuntimeEventLevel::Info,
                "ComfyUI progress node 7: 3/10",
            ))
            .unwrap();

        assert_eq!(repository.stats().unwrap().events, 1);
    }
}
