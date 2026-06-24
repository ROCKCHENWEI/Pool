use anyhow::{bail, Result};
use serde::Serialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::db::RuntimeRepository;
use crate::engine::{
    OutputDeliverableResultReport, OutputDeliverableResultRequest, OutputManifestMetric,
    OutputPackageRunner,
};
use crate::models::{RuntimeEvent, RuntimeEventLevel, RuntimeTask, TaskStatus};

use super::{SoftwareActionResult, SoftwareAdapter, SoftwareControlAction};

#[derive(Debug, Clone, Serialize)]
pub struct SoftwareActionRunReport {
    pub task_id: String,
    pub action_id: String,
    pub adapter_id: String,
    pub status: TaskStatus,
    pub result: Option<SoftwareActionResult>,
    pub output_result: Option<OutputDeliverableResultReport>,
    pub output_result_error: Option<String>,
}

pub struct SoftwareActionRunner<'a> {
    repository: &'a RuntimeRepository,
}

impl<'a> SoftwareActionRunner<'a> {
    pub fn new(repository: &'a RuntimeRepository) -> Self {
        Self { repository }
    }

    pub fn run<A>(
        &self,
        adapter: &A,
        mut task: RuntimeTask,
        action: SoftwareControlAction,
    ) -> Result<SoftwareActionRunReport>
    where
        A: SoftwareAdapter,
    {
        if action.adapter_id != adapter.config().id {
            bail!(
                "software action adapter mismatch: action={} adapter={}",
                action.adapter_id,
                adapter.config().id
            );
        }

        let action_id = Uuid::new_v4().to_string();
        task.provider_id = Some(action.adapter_id.clone());

        if action.requires_confirmation || task.requires_approval {
            task.status = TaskStatus::WaitingApproval;
            self.repository.insert_task(&task)?;
            self.repository.insert_software_action(
                &action_id,
                Some(&task.id),
                &action,
                Some(&waiting_for_confirmation_result(&action)),
            )?;
            self.repository.insert_event(&RuntimeEvent::new(
                task.project_slug.clone(),
                RuntimeEventLevel::Warn,
                format!(
                    "software action waiting for approval: {} {:?}",
                    action.adapter_id, action.action_kind
                ),
            ))?;
            return Ok(SoftwareActionRunReport {
                task_id: task.id,
                action_id,
                adapter_id: action.adapter_id,
                status: TaskStatus::WaitingApproval,
                result: None,
                output_result: None,
                output_result_error: None,
            });
        }

        task.status = TaskStatus::Running;
        self.repository.insert_task(&task)?;
        self.repository.insert_event(&RuntimeEvent::new(
            task.project_slug.clone(),
            RuntimeEventLevel::Info,
            format!(
                "software action started: {} {:?}",
                action.adapter_id, action.action_kind
            ),
        ))?;

        let health = adapter.health()?;
        self.repository.insert_event(&RuntimeEvent::new(
            task.project_slug.clone(),
            if health.ok {
                RuntimeEventLevel::Info
            } else {
                RuntimeEventLevel::Error
            },
            format!("software health {}: {}", health.adapter_id, health.message),
        ))?;
        if !health.ok {
            self.repository
                .update_task_status(&task.id, TaskStatus::Failed)?;
            self.repository.insert_software_action(
                &action_id,
                Some(&task.id),
                &action,
                Some(&health),
            )?;
            return Ok(SoftwareActionRunReport {
                task_id: task.id,
                action_id,
                adapter_id: action.adapter_id,
                status: TaskStatus::Failed,
                result: Some(health),
                output_result: None,
                output_result_error: None,
            });
        }

        let result = adapter.execute(action.clone())?;
        let status = if result.ok {
            TaskStatus::Succeeded
        } else {
            TaskStatus::Failed
        };
        self.repository
            .update_task_status(&task.id, status.clone())?;
        self.repository.insert_software_action(
            &action_id,
            Some(&task.id),
            &action,
            Some(&result),
        )?;
        self.repository.insert_event(&RuntimeEvent::new(
            task.project_slug.clone(),
            if result.ok {
                RuntimeEventLevel::Ok
            } else {
                RuntimeEventLevel::Error
            },
            format!(
                "software action finished: {} {:?} ok={}",
                result.adapter_id, result.action_kind, result.ok
            ),
        ))?;
        let (output_result, output_result_error) = if result.ok {
            self.record_output_result_from_software_action(&task, &action_id, &action, &result)?
        } else {
            (None, None)
        };

        Ok(SoftwareActionRunReport {
            task_id: task.id,
            action_id,
            adapter_id: action.adapter_id,
            status,
            result: Some(result),
            output_result,
            output_result_error,
        })
    }

    fn record_output_result_from_software_action(
        &self,
        task: &RuntimeTask,
        action_id: &str,
        action: &SoftwareControlAction,
        result: &SoftwareActionResult,
    ) -> Result<(Option<OutputDeliverableResultReport>, Option<String>)> {
        let Some(request) = output_result_request_from_payload(task, action_id, action, result)
        else {
            return Ok((None, None));
        };

        match OutputPackageRunner::new(self.repository).record_result(request) {
            Ok(report) => {
                self.repository.insert_event(&RuntimeEvent::new(
                    task.project_slug.clone(),
                    RuntimeEventLevel::Ok,
                    format!(
                        "software action linked output result: {} -> {}",
                        action.adapter_id, report.target
                    ),
                ))?;
                Ok((Some(report), None))
            }
            Err(error) => {
                let message = format!("software action output result link failed: {error}");
                self.repository.insert_event(&RuntimeEvent::new(
                    task.project_slug.clone(),
                    RuntimeEventLevel::Warn,
                    message.clone(),
                ))?;
                Ok((None, Some(message)))
            }
        }
    }
}

fn waiting_for_confirmation_result(action: &SoftwareControlAction) -> SoftwareActionResult {
    SoftwareActionResult {
        adapter_id: action.adapter_id.clone(),
        action_kind: action.action_kind.clone(),
        priority: action.priority.clone(),
        ok: false,
        message: "waiting for human confirmation before executing software action".to_string(),
        artifacts: Vec::new(),
    }
}

fn output_result_request_from_payload(
    task: &RuntimeTask,
    action_id: &str,
    action: &SoftwareControlAction,
    result: &SoftwareActionResult,
) -> Option<OutputDeliverableResultRequest> {
    let value = action
        .payload_json
        .get("pool_output_result")
        .or_else(|| action.payload_json.get("output_result"))?;
    let target = value
        .get("target")
        .and_then(Value::as_str)
        .or_else(|| value.as_str())?
        .trim();
    if target.is_empty() {
        return None;
    }

    Some(OutputDeliverableResultRequest {
        project_slug: task.project_slug.clone(),
        node_id: task.node_id.clone(),
        target: target.to_string(),
        local_path: string_field(value, "local_path"),
        status: string_field(value, "status").unwrap_or_else(|| "succeeded".to_string()),
        runtime: string_field(value, "runtime").or_else(|| Some(result.adapter_id.clone())),
        adapter_id: string_field(value, "adapter_id").or_else(|| Some(result.adapter_id.clone())),
        software_action_id: Some(action_id.to_string()),
        message: string_field(value, "message").or_else(|| Some(result.message.clone())),
        artifacts: string_array_field(value, "artifacts")
            .unwrap_or_else(|| result.artifacts.clone()),
        metrics: output_result_metrics(value),
        verification: value
            .get("verification")
            .cloned()
            .or_else(|| Some(default_output_result_verification(action, result))),
    })
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn string_array_field(value: &Value, key: &str) -> Option<Vec<String>> {
    let values = value.get(key)?.as_array()?;
    Some(
        values
            .iter()
            .filter_map(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .collect(),
    )
}

fn output_result_metrics(value: &Value) -> Vec<OutputManifestMetric> {
    let Some(metrics) = value.get("metrics") else {
        return Vec::new();
    };
    if let Ok(metrics) = serde_json::from_value::<Vec<OutputManifestMetric>>(metrics.clone()) {
        return metrics;
    }
    metrics
        .as_object()
        .map(|object| {
            object
                .iter()
                .map(|(label, value)| OutputManifestMetric {
                    label: label.clone(),
                    value: value
                        .as_str()
                        .map(str::to_string)
                        .unwrap_or_else(|| value.to_string()),
                })
                .collect()
        })
        .unwrap_or_default()
}

fn default_output_result_verification(
    action: &SoftwareControlAction,
    result: &SoftwareActionResult,
) -> Value {
    json!({
        "source": "software_action_runner",
        "adapter_id": action.adapter_id,
        "action_kind": action.action_kind,
        "priority": action.priority,
        "ok": result.ok,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::control::{ControlPriority, MockUnrealAdapter, SoftwareActionKind};
    use crate::db::RuntimeRepository;
    use crate::engine::{OutputPackageRequest, OutputPackageRunner};
    use std::fs;

    #[test]
    fn runs_mock_unreal_action_and_records_audit_rows() {
        let repository = RuntimeRepository::in_memory().unwrap();
        repository.migrate().unwrap();
        let runner = SoftwareActionRunner::new(&repository);
        let adapter = MockUnrealAdapter::new();
        let task = RuntimeTask::new("demo", "Unreal scene assembly");

        let report = runner
            .run(
                &adapter,
                task,
                SoftwareControlAction {
                    adapter_id: "unreal".to_string(),
                    action_kind: SoftwareActionKind::CreateScene,
                    priority: ControlPriority::ApiMcp,
                    payload_json: serde_json::json!({"level":"demo"}),
                    requires_confirmation: false,
                },
            )
            .unwrap();

        assert_eq!(report.status, TaskStatus::Succeeded);
        assert!(report.result.unwrap().ok);
        assert_eq!(repository.table_count("software_actions").unwrap(), 1);
        assert_eq!(repository.table_count("workflow_events").unwrap(), 3);
    }

    #[test]
    fn links_successful_software_action_to_output_manifest() {
        let root =
            std::env::temp_dir().join(format!("pool-software-output-{}", uuid::Uuid::new_v4()));
        let repository = RuntimeRepository::in_memory().unwrap();
        repository.migrate().unwrap();
        let output_runner = OutputPackageRunner::new(&repository);
        let output_report = output_runner
            .run(OutputPackageRequest {
                project_slug: "demo".to_string(),
                node_id: Some("outputs".to_string()),
                output_dir: root.to_string_lossy().to_string(),
                title: "Deliver demo".to_string(),
                source_assets: vec!["worlds/demo/output/1-world.glb".to_string()],
                duration_ms: 8_000,
            })
            .unwrap();
        let runner = SoftwareActionRunner::new(&repository);
        let adapter = MockUnrealAdapter::new();
        let mut task = RuntimeTask::new("demo", "Unreal viewport verification");
        task.node_id = Some("outputs".to_string());

        let report = runner
            .run(
                &adapter,
                task,
                SoftwareControlAction {
                    adapter_id: "unreal".to_string(),
                    action_kind: SoftwareActionKind::RunViewport,
                    priority: ControlPriority::ApiMcp,
                    payload_json: serde_json::json!({
                        "level": "demo_content_burst",
                        "pool_output_result": {
                            "target": "game",
                            "local_path": output_report.local_paths[1],
                            "runtime": "Unreal",
                            "adapter_id": "unreal",
                            "message": "play-in-editor viewport verified",
                            "artifacts": ["unreal://level/demo_content_burst"],
                            "metrics": { "fps": 60 }
                        }
                    }),
                    requires_confirmation: false,
                },
            )
            .unwrap();

        let output_result = report.output_result.unwrap();
        assert_eq!(report.status, TaskStatus::Succeeded);
        assert!(report.output_result_error.is_none());
        assert_eq!(output_result.target, "game");
        assert_eq!(
            output_result.manifest["execution_result"]["software_action_id"],
            report.action_id
        );
        assert_eq!(
            output_result.manifest["execution_result"]["message"],
            "play-in-editor viewport verified"
        );
        assert!(output_result.catalog.deliverables[1]
            .metrics
            .iter()
            .any(|metric| metric.label == "execution" && metric.value == "succeeded"));
        assert_eq!(repository.table_count("software_actions").unwrap(), 1);
        assert_eq!(repository.table_count("tasks").unwrap(), 3);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn blocks_software_action_waiting_for_confirmation() {
        let repository = RuntimeRepository::in_memory().unwrap();
        repository.migrate().unwrap();
        let runner = SoftwareActionRunner::new(&repository);
        let adapter = MockUnrealAdapter::new();
        let task = RuntimeTask::new("demo", "Unreal render");

        let report = runner
            .run(
                &adapter,
                task,
                SoftwareControlAction {
                    adapter_id: "unreal".to_string(),
                    action_kind: SoftwareActionKind::Render,
                    priority: ControlPriority::ApiMcp,
                    payload_json: serde_json::json!({"sequence":"main"}),
                    requires_confirmation: true,
                },
            )
            .unwrap();

        assert_eq!(report.status, TaskStatus::WaitingApproval);
        assert!(report.result.is_none());
        assert_eq!(repository.table_count("software_actions").unwrap(), 1);
        assert_eq!(repository.table_count("workflow_events").unwrap(), 1);
    }

    #[test]
    fn rejects_adapter_mismatch() {
        let repository = RuntimeRepository::in_memory().unwrap();
        repository.migrate().unwrap();
        let runner = SoftwareActionRunner::new(&repository);
        let adapter = MockUnrealAdapter::new();
        let task = RuntimeTask::new("demo", "wrong adapter");

        let result = runner.run(
            &adapter,
            task,
            SoftwareControlAction {
                adapter_id: "blender".to_string(),
                action_kind: SoftwareActionKind::CreateScene,
                priority: ControlPriority::ApiMcp,
                payload_json: serde_json::json!({}),
                requires_confirmation: false,
            },
        );

        assert!(result.is_err());
        assert_eq!(repository.table_count("software_actions").unwrap(), 0);
    }
}
