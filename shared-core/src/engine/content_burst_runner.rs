use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::{Path, PathBuf};

use crate::assets::materialize_project_envelope;
use crate::control::{
    AgentSessionRunReport, AgentSessionRunner, ControlPriority, HermesCommand,
    HermesExecutionOptions, MockUnrealAdapter, SoftwareActionKind, SoftwareActionRunReport,
    SoftwareActionRunner, SoftwareControlAction, UnrealMcpAdapter,
};
use crate::db::RuntimeRepository;
use crate::models::{NodeType, RuntimeEvent, RuntimeEventLevel, RuntimeTask, TaskStatus};
use crate::providers::{
    Mock3dgsProvider, ProviderAdapter, ProviderRequest, ThreeDgsGatewayOptions,
    ThreeDgsGatewayProvider,
};

use super::{
    build_default_content_burst_plan, OutputPackageRequest, OutputPackageRunReport,
    OutputPackageRunner, PoolRuntimePlan, ProviderTaskRunReport, ProviderTaskRunner,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentBurstRunRequest {
    pub project_slug: String,
    pub output_root: String,
    pub title: String,
    pub prompt: String,
    pub source_inputs: Vec<String>,
    pub duration_ms: u64,
    #[serde(default)]
    pub three_dgs_mode: ContentBurstProviderMode,
    pub three_dgs_provider_id: Option<String>,
    pub three_dgs_endpoint: Option<String>,
    pub three_dgs_api_key: Option<String>,
    #[serde(default)]
    pub unreal_mode: ContentBurstSoftwareMode,
    pub unreal_endpoint: Option<String>,
    pub unreal_auth_token: Option<String>,
    #[serde(default)]
    pub agent_mode: ContentBurstAgentMode,
    pub hermes_endpoint: Option<String>,
    pub hermes_auth_token: Option<String>,
    #[serde(default)]
    pub agent_requires_confirmation: bool,
}

impl ContentBurstRunRequest {
    pub fn new(project_slug: impl Into<String>, output_root: impl Into<String>) -> Self {
        Self {
            project_slug: project_slug.into(),
            output_root: output_root.into(),
            title: "Pool content burst run".to_string(),
            prompt: "generate a local content burst package".to_string(),
            source_inputs: vec!["worlds/demo/source/0-reference.png".to_string()],
            duration_ms: 12_000,
            three_dgs_mode: ContentBurstProviderMode::Auto,
            three_dgs_provider_id: None,
            three_dgs_endpoint: None,
            three_dgs_api_key: None,
            unreal_mode: ContentBurstSoftwareMode::Auto,
            unreal_endpoint: None,
            unreal_auth_token: None,
            agent_mode: ContentBurstAgentMode::Stage,
            hermes_endpoint: None,
            hermes_auth_token: None,
            agent_requires_confirmation: false,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContentBurstAgentMode {
    Stage,
    Skip,
    HermesHttp,
}

impl Default for ContentBurstAgentMode {
    fn default() -> Self {
        Self::Stage
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContentBurstProviderMode {
    Auto,
    Mock,
    Gateway,
}

impl Default for ContentBurstProviderMode {
    fn default() -> Self {
        Self::Auto
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContentBurstSoftwareMode {
    Auto,
    Mock,
    UnrealMcp,
}

impl Default for ContentBurstSoftwareMode {
    fn default() -> Self {
        Self::Auto
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ContentBurstRunReport {
    pub project_slug: String,
    pub workflow_id: String,
    pub envelope_root: String,
    pub agent_mode: ContentBurstAgentMode,
    pub three_dgs_mode: ContentBurstProviderMode,
    pub unreal_mode: ContentBurstSoftwareMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_report: Option<AgentSessionRunReport>,
    pub provider_report: ProviderTaskRunReport,
    pub software_report: SoftwareActionRunReport,
    pub output_report: OutputPackageRunReport,
    pub assets_indexed: usize,
}

pub struct ContentBurstRunner<'a> {
    repository: &'a RuntimeRepository,
}

impl<'a> ContentBurstRunner<'a> {
    pub fn new(repository: &'a RuntimeRepository) -> Self {
        Self { repository }
    }

    pub fn run(&self, request: ContentBurstRunRequest) -> Result<ContentBurstRunReport> {
        let plan = build_default_content_burst_plan(&request.project_slug, &request.title);
        self.ensure_project_plan(&plan)?;

        let envelope = materialize_project_envelope(&request.output_root, &plan)?;
        self.repository.insert_event(&RuntimeEvent::new(
            request.project_slug.clone(),
            RuntimeEventLevel::Info,
            format!("content burst run started: {}", request.title),
        ))?;

        let (agent_report, agent_mode) = self.run_agent_decision(
            &plan,
            &request,
            &Path::new(&envelope.output_dir).join("control"),
        )?;
        let (provider_report, three_dgs_mode) =
            self.run_3dgs(&plan, &request, &envelope.output_dir)?;
        let (software_report, unreal_mode) =
            self.run_unreal_assembly(&plan, &request, &provider_report)?;
        let output_report = self.run_output_package(
            &plan,
            &request,
            &envelope.output_dir,
            &provider_report,
            &software_report,
        )?;

        let assets_indexed = provider_report.assets.len() + output_report.assets.len();
        self.repository.insert_event(&RuntimeEvent::new(
            request.project_slug.clone(),
            RuntimeEventLevel::Ok,
            format!("content burst run finished: {assets_indexed} assets indexed"),
        ))?;

        Ok(ContentBurstRunReport {
            project_slug: request.project_slug,
            workflow_id: plan.workflow.id,
            envelope_root: envelope.root,
            agent_mode,
            three_dgs_mode,
            unreal_mode,
            agent_report,
            provider_report,
            software_report,
            output_report,
            assets_indexed,
        })
    }

    fn ensure_project_plan(&self, plan: &PoolRuntimePlan) -> Result<()> {
        if self
            .repository
            .snapshot(Some(&plan.project.slug))?
            .stats
            .projects
            == 0
        {
            self.repository.persist_plan(plan)?;
        }
        Ok(())
    }

    fn run_agent_decision(
        &self,
        plan: &PoolRuntimePlan,
        request: &ContentBurstRunRequest,
        control_dir: &Path,
    ) -> Result<(Option<AgentSessionRunReport>, ContentBurstAgentMode)> {
        if request.agent_mode == ContentBurstAgentMode::Skip {
            self.repository.insert_event(&RuntimeEvent::new(
                request.project_slug.clone(),
                RuntimeEventLevel::Info,
                "content burst agent decision skipped",
            ))?;
            return Ok((None, ContentBurstAgentMode::Skip));
        }

        let command = HermesCommand {
            endpoint: non_empty(&request.hermes_endpoint).unwrap_or_default(),
            project_slug: request.project_slug.clone(),
            instruction: build_agent_decision_instruction(plan, request),
            allowed_tools: vec![
                "api".to_string(),
                "mcp".to_string(),
                "sqlite".to_string(),
                "filesystem".to_string(),
                "3dgs-gateway".to_string(),
                "unreal".to_string(),
                "output-package".to_string(),
                "human-takeover".to_string(),
            ],
            requires_confirmation: request.agent_requires_confirmation,
        };
        let runner = AgentSessionRunner::new(self.repository);
        let report = match request.agent_mode {
            ContentBurstAgentMode::Stage => runner.stage_hermes_command(command, control_dir)?,
            ContentBurstAgentMode::HermesHttp => runner.run_hermes_command(
                command,
                control_dir,
                HermesExecutionOptions {
                    auth_token: non_empty(&request.hermes_auth_token),
                    ..HermesExecutionOptions::default()
                },
            )?,
            ContentBurstAgentMode::Skip => unreachable!(),
        };
        if let Some(agent_node_id) = node_id_for(plan, NodeType::Agent) {
            self.repository
                .update_task_node_id(&report.task_id, Some(&agent_node_id))?;
        }

        Ok((Some(report), request.agent_mode))
    }

    fn run_3dgs(
        &self,
        plan: &PoolRuntimePlan,
        request: &ContentBurstRunRequest,
        output_dir: &str,
    ) -> Result<(ProviderTaskRunReport, ContentBurstProviderMode)> {
        let gateway_available = three_dgs_gateway_configured(request);
        match request.three_dgs_mode {
            ContentBurstProviderMode::Mock => Ok((
                self.run_mock_3dgs(plan, request, output_dir)?,
                ContentBurstProviderMode::Mock,
            )),
            ContentBurstProviderMode::Gateway => Ok((
                self.run_gateway_3dgs(plan, request, output_dir)?,
                ContentBurstProviderMode::Gateway,
            )),
            ContentBurstProviderMode::Auto if gateway_available => {
                match self.run_gateway_3dgs(plan, request, output_dir) {
                    Ok(report) if report.status == TaskStatus::Succeeded => {
                        Ok((report, ContentBurstProviderMode::Gateway))
                    }
                    Ok(report) => {
                        self.repository.insert_event(&RuntimeEvent::new(
                            request.project_slug.clone(),
                            RuntimeEventLevel::Warn,
                            format!(
                                "3DGS gateway returned {:?}; falling back to mock 3DGS",
                                report.status
                            ),
                        ))?;
                        Ok((
                            self.run_mock_3dgs(plan, request, output_dir)?,
                            ContentBurstProviderMode::Mock,
                        ))
                    }
                    Err(error) => {
                        self.repository.insert_event(&RuntimeEvent::new(
                            request.project_slug.clone(),
                            RuntimeEventLevel::Warn,
                            format!("3DGS gateway failed; falling back to mock 3DGS: {error}"),
                        ))?;
                        Ok((
                            self.run_mock_3dgs(plan, request, output_dir)?,
                            ContentBurstProviderMode::Mock,
                        ))
                    }
                }
            }
            ContentBurstProviderMode::Auto => Ok((
                self.run_mock_3dgs(plan, request, output_dir)?,
                ContentBurstProviderMode::Mock,
            )),
        }
    }

    fn run_mock_3dgs(
        &self,
        plan: &PoolRuntimePlan,
        request: &ContentBurstRunRequest,
        output_dir: &str,
    ) -> Result<ProviderTaskRunReport> {
        let provider = Mock3dgsProvider::new("mock-3dgs", "Mock 3DGS");
        let mut task = RuntimeTask::new(request.project_slug.clone(), "Local 3DGS asset package");
        task.node_id = node_id_for(plan, NodeType::ThreeDgs);
        task.provider_id = Some("mock-3dgs".to_string());
        task.cost_estimate_tokens = 0;
        task.status = TaskStatus::Ready;

        self.run_provider_adapter(
            &provider,
            task,
            ProviderRequest {
                project_slug: request.project_slug.clone(),
                prompt: request.prompt.clone(),
                input_paths: request.source_inputs.clone(),
                output_dir: output_dir.to_string(),
                require_approval: false,
            },
        )
    }

    fn run_gateway_3dgs(
        &self,
        plan: &PoolRuntimePlan,
        request: &ContentBurstRunRequest,
        output_dir: &str,
    ) -> Result<ProviderTaskRunReport> {
        let provider_id = request
            .three_dgs_provider_id
            .clone()
            .unwrap_or_else(|| "worldlabs-marble".to_string());
        let mut options = ThreeDgsGatewayOptions::from_env(
            provider_id.clone(),
            three_dgs_display_name(&provider_id),
            None,
        );
        if let Some(endpoint) = non_empty(&request.three_dgs_endpoint) {
            options.endpoint = endpoint;
        }
        if let Some(api_key) = non_empty(&request.three_dgs_api_key) {
            options.api_key = Some(api_key);
        }

        let provider = ThreeDgsGatewayProvider::new(options);
        let mut task = RuntimeTask::new(
            request.project_slug.clone(),
            format!("{} 3DGS gateway asset package", provider_id),
        );
        task.node_id = node_id_for(plan, NodeType::ThreeDgs);
        task.provider_id = Some(provider_id);
        task.cost_estimate_tokens = 9_000;
        task.status = TaskStatus::Ready;

        self.run_provider_adapter(
            &provider,
            task,
            ProviderRequest {
                project_slug: request.project_slug.clone(),
                prompt: request.prompt.clone(),
                input_paths: request.source_inputs.clone(),
                output_dir: output_dir.to_string(),
                require_approval: false,
            },
        )
    }

    fn run_provider_adapter<A>(
        &self,
        adapter: &A,
        task: RuntimeTask,
        request: ProviderRequest,
    ) -> Result<ProviderTaskRunReport>
    where
        A: ProviderAdapter,
    {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?
            .block_on(ProviderTaskRunner::new(self.repository).run(adapter, task, request))
    }

    fn run_unreal_assembly(
        &self,
        plan: &PoolRuntimePlan,
        request: &ContentBurstRunRequest,
        provider_report: &ProviderTaskRunReport,
    ) -> Result<(SoftwareActionRunReport, ContentBurstSoftwareMode)> {
        let mcp_available = unreal_mcp_configured(request);
        match request.unreal_mode {
            ContentBurstSoftwareMode::Mock => Ok((
                self.run_mock_unreal_assembly(plan, provider_report)?,
                ContentBurstSoftwareMode::Mock,
            )),
            ContentBurstSoftwareMode::UnrealMcp => Ok((
                self.run_unreal_mcp_assembly(plan, request, provider_report)?,
                ContentBurstSoftwareMode::UnrealMcp,
            )),
            ContentBurstSoftwareMode::Auto if mcp_available => {
                let report = self.run_unreal_mcp_assembly(plan, request, provider_report)?;
                if report.status == TaskStatus::Succeeded {
                    Ok((report, ContentBurstSoftwareMode::UnrealMcp))
                } else {
                    self.repository.insert_event(&RuntimeEvent::new(
                        plan.project.slug.clone(),
                        RuntimeEventLevel::Warn,
                        format!(
                            "Unreal MCP returned {:?}; falling back to mock Unreal",
                            report.status
                        ),
                    ))?;
                    Ok((
                        self.run_mock_unreal_assembly(plan, provider_report)?,
                        ContentBurstSoftwareMode::Mock,
                    ))
                }
            }
            ContentBurstSoftwareMode::Auto => Ok((
                self.run_mock_unreal_assembly(plan, provider_report)?,
                ContentBurstSoftwareMode::Mock,
            )),
        }
    }

    fn run_mock_unreal_assembly(
        &self,
        plan: &PoolRuntimePlan,
        provider_report: &ProviderTaskRunReport,
    ) -> Result<SoftwareActionRunReport> {
        let runner = SoftwareActionRunner::new(self.repository);
        let adapter = MockUnrealAdapter::new();
        let mut task = RuntimeTask::new(plan.project.slug.clone(), "Unreal local assembly");
        task.node_id = node_id_for(plan, NodeType::Unreal);
        task.provider_id = Some("unreal".to_string());

        runner.run(
            &adapter,
            task,
            SoftwareControlAction {
                adapter_id: "unreal".to_string(),
                action_kind: SoftwareActionKind::CreateScene,
                priority: ControlPriority::ApiMcp,
                payload_json: json!({
                    "level": "demo_content_burst",
                    "assets": provider_report
                        .assets
                        .iter()
                        .map(|asset| asset.local_path.clone())
                        .collect::<Vec<_>>(),
                    "requested_from": "content-burst-runner",
                }),
                requires_confirmation: false,
            },
        )
    }

    fn run_unreal_mcp_assembly(
        &self,
        plan: &PoolRuntimePlan,
        request: &ContentBurstRunRequest,
        provider_report: &ProviderTaskRunReport,
    ) -> Result<SoftwareActionRunReport> {
        let runner = SoftwareActionRunner::new(self.repository);
        let mut task = RuntimeTask::new(plan.project.slug.clone(), "Unreal MCP assembly");
        task.node_id = node_id_for(plan, NodeType::Unreal);
        task.provider_id = Some("unreal".to_string());

        let mut payload = json!({
            "level": "demo_content_burst",
            "assets": provider_report
                .assets
                .iter()
                .map(|asset| asset.local_path.clone())
                .collect::<Vec<_>>(),
            "requested_from": "content-burst-runner",
        });
        if let Some(endpoint) = non_empty(&request.unreal_endpoint) {
            payload["endpoint"] = json!(endpoint);
        }
        if let Some(auth_token) = non_empty(&request.unreal_auth_token) {
            payload["auth_token"] = json!(auth_token);
        }

        let action = SoftwareControlAction {
            adapter_id: "unreal".to_string(),
            action_kind: SoftwareActionKind::CreateScene,
            priority: ControlPriority::ApiMcp,
            payload_json: payload,
            requires_confirmation: false,
        };
        let adapter = UnrealMcpAdapter::from_action(&action);
        runner.run(&adapter, task, action)
    }

    fn run_output_package(
        &self,
        plan: &PoolRuntimePlan,
        request: &ContentBurstRunRequest,
        output_dir: &str,
        provider_report: &ProviderTaskRunReport,
        software_report: &SoftwareActionRunReport,
    ) -> Result<OutputPackageRunReport> {
        let mut source_assets = provider_report
            .assets
            .iter()
            .map(|asset| asset.local_path.clone())
            .collect::<Vec<_>>();
        if let Some(result) = &software_report.result {
            source_assets.extend(result.artifacts.clone());
        }

        OutputPackageRunner::new(self.repository).run(OutputPackageRequest {
            project_slug: request.project_slug.clone(),
            node_id: node_id_for(plan, NodeType::VideoOutput),
            output_dir: output_dir.to_string(),
            title: "三类输出交付包".to_string(),
            source_assets,
            duration_ms: request.duration_ms,
        })
    }
}

fn node_id_for(plan: &PoolRuntimePlan, node_type: NodeType) -> Option<String> {
    plan.workflow
        .nodes
        .values()
        .find(|node| node.node_type == node_type)
        .map(|node| node.id.clone())
}

pub fn default_content_burst_output_root(db_path: impl AsRef<Path>) -> PathBuf {
    db_path
        .as_ref()
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn three_dgs_gateway_configured(request: &ContentBurstRunRequest) -> bool {
    if non_empty(&request.three_dgs_endpoint).is_some() {
        return true;
    }
    let provider_id = request
        .three_dgs_provider_id
        .as_deref()
        .unwrap_or("worldlabs-marble");
    let prefix = provider_env_prefix(provider_id);
    env_has_value("POOL_3DGS_GATEWAY_ENDPOINT") || env_has_value(&format!("POOL_{prefix}_ENDPOINT"))
}

fn unreal_mcp_configured(request: &ContentBurstRunRequest) -> bool {
    non_empty(&request.unreal_endpoint).is_some() || env_has_value("POOL_UNREAL_MCP_ENDPOINT")
}

fn non_empty(value: &Option<String>) -> Option<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn env_has_value(key: &str) -> bool {
    std::env::var(key).is_ok_and(|value| !value.trim().is_empty())
}

fn provider_env_prefix(provider_id: &str) -> String {
    provider_id
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect()
}

fn build_agent_decision_instruction(
    plan: &PoolRuntimePlan,
    request: &ContentBurstRunRequest,
) -> String {
    let three_dgs_provider = request
        .three_dgs_provider_id
        .as_deref()
        .unwrap_or("worldlabs-marble");
    format!(
        "Plan a Pool content burst execution for project `{}`. \
Input title: `{}`. Prompt: `{}`. Source inputs: {:?}. \
3DGS mode: {:?}, provider: `{}`, gateway configured: {}. \
Unreal mode: {:?}, MCP configured: {}. \
Keep local files authoritative, use provider URLs only as provenance, \
and recommend human takeover if API/MCP and CLI control both fail. \
Workflow id: {}.",
        request.project_slug,
        request.title,
        request.prompt,
        request.source_inputs,
        request.three_dgs_mode,
        three_dgs_provider,
        three_dgs_gateway_configured(request),
        request.unreal_mode,
        unreal_mcp_configured(request),
        plan.workflow.id
    )
}

fn three_dgs_display_name(provider_id: &str) -> String {
    match provider_id {
        "worldlabs-marble" => "World Labs Marble",
        "tripo-splat" => "TripoSplat",
        "sam-3d" => "SAM-3D",
        "spark-3dgs" => "Spark 3DGS",
        "qunhe-3d" => "Qunhe 3D",
        other => other,
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::thread;

    #[test]
    fn runs_local_content_burst_chain() {
        let root =
            std::env::temp_dir().join(format!("pool-content-burst-{}", uuid::Uuid::new_v4()));
        let repository = RuntimeRepository::in_memory().unwrap();
        repository.migrate().unwrap();
        let runner = ContentBurstRunner::new(&repository);

        let report = runner
            .run(ContentBurstRunRequest {
                project_slug: "demo".to_string(),
                output_root: root.to_string_lossy().to_string(),
                title: "Pool local content burst".to_string(),
                prompt: "generate a local world".to_string(),
                source_inputs: vec!["worlds/demo/source/0-reference.png".to_string()],
                duration_ms: 12_000,
                ..ContentBurstRunRequest::new("demo", root.to_string_lossy().to_string())
            })
            .unwrap();

        assert_eq!(report.provider_report.status, TaskStatus::Succeeded);
        assert_eq!(report.agent_mode, ContentBurstAgentMode::Stage);
        let agent_report = report.agent_report.as_ref().unwrap();
        assert_eq!(agent_report.status, TaskStatus::Ready);
        assert!(Path::new(&agent_report.transcript_path).exists());
        assert_eq!(report.three_dgs_mode, ContentBurstProviderMode::Mock);
        assert_eq!(report.unreal_mode, ContentBurstSoftwareMode::Mock);
        assert_eq!(report.software_report.status, TaskStatus::Succeeded);
        assert_eq!(report.output_report.status, TaskStatus::Succeeded);
        assert_eq!(report.assets_indexed, 6);
        assert_eq!(repository.table_count("projects").unwrap(), 1);
        assert_eq!(repository.table_count("agent_sessions").unwrap(), 1);
        assert_eq!(repository.table_count("assets").unwrap(), 6);
        assert_eq!(repository.table_count("software_actions").unwrap(), 1);
        let task_node_id = repository
            .task_snapshot(&agent_report.task_id)
            .unwrap()
            .node_id
            .unwrap();
        let snapshot = repository.snapshot(Some("demo")).unwrap();
        let node_type = snapshot.workflows[0]
            .nodes
            .get(&task_node_id)
            .unwrap()
            .get("node_type")
            .unwrap()
            .as_str()
            .unwrap();
        assert_eq!(node_type, "Agent");
        assert!(root
            .join("worlds/demo/output/deliverables/1-video-timeline.json")
            .exists());

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn can_skip_agent_decision() {
        let root = std::env::temp_dir().join(format!(
            "pool-content-burst-no-agent-{}",
            uuid::Uuid::new_v4()
        ));
        let repository = RuntimeRepository::in_memory().unwrap();
        repository.migrate().unwrap();
        let runner = ContentBurstRunner::new(&repository);

        let report = runner
            .run(ContentBurstRunRequest {
                project_slug: "demo".to_string(),
                output_root: root.to_string_lossy().to_string(),
                title: "Pool no-agent content burst".to_string(),
                prompt: "generate a local world".to_string(),
                source_inputs: vec!["worlds/demo/source/0-reference.png".to_string()],
                duration_ms: 12_000,
                agent_mode: ContentBurstAgentMode::Skip,
                ..ContentBurstRunRequest::new("demo", root.to_string_lossy().to_string())
            })
            .unwrap();

        assert_eq!(report.agent_mode, ContentBurstAgentMode::Skip);
        assert!(report.agent_report.is_none());
        assert_eq!(repository.table_count("agent_sessions").unwrap(), 0);

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn can_execute_hermes_http_decision_when_requested() {
        let endpoint = spawn_fake_hermes_http();
        let root = std::env::temp_dir().join(format!(
            "pool-content-burst-hermes-{}",
            uuid::Uuid::new_v4()
        ));
        let repository = RuntimeRepository::in_memory().unwrap();
        repository.migrate().unwrap();
        let runner = ContentBurstRunner::new(&repository);

        let report = runner
            .run(ContentBurstRunRequest {
                project_slug: "demo".to_string(),
                output_root: root.to_string_lossy().to_string(),
                title: "Pool Hermes content burst".to_string(),
                prompt: "generate a local world".to_string(),
                source_inputs: vec!["worlds/demo/source/0-reference.png".to_string()],
                duration_ms: 12_000,
                agent_mode: ContentBurstAgentMode::HermesHttp,
                hermes_endpoint: Some(endpoint),
                ..ContentBurstRunRequest::new("demo", root.to_string_lossy().to_string())
            })
            .unwrap();

        let agent_report = report.agent_report.as_ref().unwrap();
        assert_eq!(report.agent_mode, ContentBurstAgentMode::HermesHttp);
        let execution = agent_report.execution.as_ref().unwrap();
        assert_eq!(
            agent_report.status,
            TaskStatus::Succeeded,
            "Hermes execution failed: {execution:?}"
        );
        assert!(execution.ok);
        assert_eq!(repository.table_count("agent_sessions").unwrap(), 1);

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn persists_requested_project_when_other_projects_exist() {
        let root = std::env::temp_dir().join(format!(
            "pool-content-burst-project-{}",
            uuid::Uuid::new_v4()
        ));
        let repository = RuntimeRepository::in_memory().unwrap();
        repository.migrate().unwrap();
        let other = build_default_content_burst_plan("other", "Other project");
        repository.persist_plan(&other).unwrap();
        let runner = ContentBurstRunner::new(&repository);

        runner
            .run(ContentBurstRunRequest {
                project_slug: "demo".to_string(),
                output_root: root.to_string_lossy().to_string(),
                title: "Pool local content burst".to_string(),
                prompt: "generate a local world".to_string(),
                source_inputs: vec!["worlds/demo/source/0-reference.png".to_string()],
                duration_ms: 12_000,
                ..ContentBurstRunRequest::new("demo", root.to_string_lossy().to_string())
            })
            .unwrap();

        assert_eq!(repository.snapshot(None).unwrap().stats.projects, 2);
        assert_eq!(repository.snapshot(Some("demo")).unwrap().stats.projects, 1);

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn can_run_three_dgs_gateway_when_requested() {
        let gateway = spawn_fake_3dgs_gateway();
        let root = std::env::temp_dir().join(format!(
            "pool-content-burst-gateway-{}",
            uuid::Uuid::new_v4()
        ));
        let repository = RuntimeRepository::in_memory().unwrap();
        repository.migrate().unwrap();
        let runner = ContentBurstRunner::new(&repository);

        let report = runner
            .run(ContentBurstRunRequest {
                project_slug: "demo".to_string(),
                output_root: root.to_string_lossy().to_string(),
                title: "Pool gateway content burst".to_string(),
                prompt: "generate a gateway world".to_string(),
                source_inputs: vec!["worlds/demo/source/0-reference.png".to_string()],
                duration_ms: 12_000,
                three_dgs_mode: ContentBurstProviderMode::Gateway,
                three_dgs_endpoint: Some(gateway),
                ..ContentBurstRunRequest::new("demo", root.to_string_lossy().to_string())
            })
            .unwrap();

        assert_eq!(report.three_dgs_mode, ContentBurstProviderMode::Gateway);
        assert_eq!(report.provider_report.provider_id, "worldlabs-marble");
        assert_eq!(report.provider_report.assets.len(), 1);
        assert_eq!(report.assets_indexed, 4);
        assert!(root.join("worlds/demo/output/1-world.glb").exists());

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn can_run_unreal_mcp_when_requested() {
        let endpoint = spawn_fake_unreal_mcp();
        let root = std::env::temp_dir().join(format!(
            "pool-content-burst-unreal-{}",
            uuid::Uuid::new_v4()
        ));
        let repository = RuntimeRepository::in_memory().unwrap();
        repository.migrate().unwrap();
        let runner = ContentBurstRunner::new(&repository);

        let report = runner
            .run(ContentBurstRunRequest {
                project_slug: "demo".to_string(),
                output_root: root.to_string_lossy().to_string(),
                title: "Pool Unreal MCP content burst".to_string(),
                prompt: "generate a local world".to_string(),
                source_inputs: vec!["worlds/demo/source/0-reference.png".to_string()],
                duration_ms: 12_000,
                unreal_mode: ContentBurstSoftwareMode::UnrealMcp,
                unreal_endpoint: Some(endpoint),
                ..ContentBurstRunRequest::new("demo", root.to_string_lossy().to_string())
            })
            .unwrap();

        assert_eq!(report.unreal_mode, ContentBurstSoftwareMode::UnrealMcp);
        assert_eq!(report.software_report.status, TaskStatus::Succeeded);
        assert!(report
            .software_report
            .result
            .as_ref()
            .unwrap()
            .artifacts
            .iter()
            .any(|artifact| artifact == "unreal://level/demo"));

        std::fs::remove_dir_all(root).unwrap();
    }

    fn spawn_fake_3dgs_gateway() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let base_url = format!("http://{addr}");
        let response_base_url = base_url.clone();

        thread::spawn(move || {
            for _ in 0..4 {
                let (mut stream, _) = listener.accept().unwrap();
                let request = read_http_request(&mut stream);
                let path = request_path(&request);
                let (content_type, body) = match path.as_str() {
                    "/v1/3dgs/jobs" => ("application/json", r#"{"job_id":"job-1"}"#.to_string()),
                    "/v1/3dgs/jobs/job-1" => (
                        "application/json",
                        format!(
                            r#"{{
                                "status":"completed",
                                "outputs":[
                                    {{"name":"world.glb","url":"{response_base_url}/files/world.glb"}}
                                ]
                            }}"#
                        ),
                    ),
                    "/files/world.glb" => ("model/gltf-binary", "fake-glb".to_string()),
                    _ => ("application/json", r#"{"status":"not_found"}"#.to_string()),
                };
                write_http_response(&mut stream, content_type, &body);
            }
        });

        base_url
    }

    fn spawn_fake_unreal_mcp() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let base_url = format!("http://{addr}");

        thread::spawn(move || {
            for _ in 0..2 {
                let (mut stream, _) = listener.accept().unwrap();
                let request = read_http_request(&mut stream);
                let path = request_path(&request);
                let body = match path.as_str() {
                    "/health" => r#"{"ok":true,"message":"unreal-health-ok"}"#,
                    "/mcp" => {
                        r#"{"ok":true,"message":"unreal-action-ok","artifacts":["unreal://level/demo"]}"#
                    }
                    _ => r#"{"ok":false,"message":"not-found"}"#,
                };
                write_http_response(&mut stream, "application/json", body);
            }
        });

        base_url
    }

    fn spawn_fake_hermes_http() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let base_url = format!("http://{addr}");

        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let _request = read_http_request(&mut stream);
            let body = r#"{"ok":true,"message":"hermes decision accepted"}"#;
            write_http_response(&mut stream, "application/json", body);
        });

        base_url
    }

    fn read_http_request(stream: &mut TcpStream) -> String {
        let mut request = Vec::new();
        let mut buffer = [0_u8; 1024];
        let mut sent_continue = false;
        loop {
            let bytes = stream.read(&mut buffer).unwrap_or_default();
            if bytes == 0 {
                break;
            }
            request.extend_from_slice(&buffer[..bytes]);
            let text = String::from_utf8_lossy(&request);
            if let Some(header_end) = text.find("\r\n\r\n") {
                let headers = &text[..header_end];
                if !sent_continue && http_expects_continue(headers) {
                    let _ = stream.write_all(b"HTTP/1.1 100 Continue\r\n\r\n");
                    sent_continue = true;
                }
                let body_start = header_end + 4;
                if http_transfer_chunked(headers) {
                    if text[body_start..].contains("\r\n0\r\n\r\n") {
                        break;
                    }
                    continue;
                }
                let content_length = http_content_length(headers);
                if request.len() >= body_start + content_length {
                    break;
                }
            }
        }
        String::from_utf8_lossy(&request).to_string()
    }

    fn http_expects_continue(headers: &str) -> bool {
        headers.lines().any(|line| {
            let Some((name, value)) = line.split_once(':') else {
                return false;
            };
            name.eq_ignore_ascii_case("expect")
                && value
                    .split(',')
                    .any(|part| part.trim().eq_ignore_ascii_case("100-continue"))
        })
    }

    fn http_transfer_chunked(headers: &str) -> bool {
        headers.lines().any(|line| {
            let Some((name, value)) = line.split_once(':') else {
                return false;
            };
            name.eq_ignore_ascii_case("transfer-encoding")
                && value
                    .split(',')
                    .any(|part| part.trim().eq_ignore_ascii_case("chunked"))
        })
    }

    fn http_content_length(headers: &str) -> usize {
        headers
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                if name.eq_ignore_ascii_case("content-length") {
                    value.trim().parse().ok()
                } else {
                    None
                }
            })
            .unwrap_or(0)
    }

    fn request_path(request: &str) -> String {
        request
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .unwrap_or("/")
            .to_string()
    }

    fn write_http_response(stream: &mut impl Write, content_type: &str, body: &str) {
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        stream.write_all(response.as_bytes()).unwrap();
    }
}
