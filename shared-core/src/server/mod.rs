use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha1::{Digest as Sha1Digest, Sha1};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;
use uuid::Uuid;

use crate::db::{ApiKeySnapshot, AssetSnapshot, ProviderRequestSnapshot, TaskSnapshot};
use crate::desktop_recognition_contract_resource;
use crate::unreal_mcp_bridge_contract_resource;
use crate::{
    output_package_catalog_resource, pool_mcp_prompt_definitions, pool_mcp_prompt_get_result,
    pool_mcp_prompt_http_path, production_evidence_run_plan_resource,
    production_evidence_tasks_resource, provider_contracts_resource,
    provider_gateway_worker_contract, runtime_adapter_catalog_resource, runtime_budget_resource,
    runtime_execution_plan_resource, runtime_graph_resource,
    runtime_handoff_package_catalog_resource, runtime_handoff_resource,
    runtime_integration_readiness_resource, runtime_node_context_index_resource,
    runtime_node_context_resource, runtime_prd_completion_gate_resource,
    runtime_prd_readiness_resource, runtime_preflight_resource,
    runtime_production_evidence_requirements_resource, runtime_workflow_context_index_resource,
    runtime_workflow_context_resource, AgentCliCommand, AgentCliExecutionOptions,
    AgentSessionRunner, ComfyUiProvider, ComfyUiProviderOptions, CommandSoftwareAdapter,
    ContentBurstAgentMode, ContentBurstProviderMode, ContentBurstRunRequest, ContentBurstRunner,
    ContentBurstSoftwareMode, ControlPriority, DesktopRecognitionAdapter, GenericHttpMediaOptions,
    GenericHttpMediaProvider, GenericSoftwareApiAdapter, HermesCommand, HermesExecutionOptions,
    HermesMcpAdapter, KlingAuth, KlingProvider, KlingProviderOptions, McpServer, Mock3dgsProvider,
    MockUnrealAdapter, OpenAiImageProvider, OpenAiImageProviderOptions,
    OutputDeliverableResultRequest, OutputManifestMetric, OutputPackageRequest,
    OutputPackageRunner, ProviderAdapter, ProviderKind, ProviderRegistry, ProviderRequest,
    ProviderRequestRecord, ProviderTaskRunner, RuntimeEvent, RuntimeEventLevel,
    RuntimeHandoffPackageRequest, RuntimeHandoffPackageRunner, RuntimeRepository, RuntimeSnapshot,
    RuntimeTask, SoftwareActionKind, SoftwareActionRecord, SoftwareActionResult,
    SoftwareActionRunner, SoftwareActionSnapshot, SoftwareAdapter, SoftwareAdapterRegistry,
    SoftwareControlAction, TaskStatus, ThreeDgsGatewayOptions, ThreeDgsGatewayProvider,
    UnrealMcpAdapter,
};
use crate::{software_control_contract, software_control_contracts_resource};

const PROVIDER_AUTO_APPROVAL_COST_TOKENS: u64 = 6_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeHttpConfig {
    pub db_path: PathBuf,
    pub project_slug: Option<String>,
    pub bind_addr: String,
}

impl RuntimeHttpConfig {
    pub fn new(db_path: impl Into<PathBuf>) -> Self {
        Self {
            db_path: db_path.into(),
            project_slug: None,
            bind_addr: "127.0.0.1:4788".to_string(),
        }
    }

    pub fn with_project_slug(mut self, project_slug: impl Into<String>) -> Self {
        self.project_slug = Some(project_slug.into());
        self
    }

    pub fn with_bind_addr(mut self, bind_addr: impl Into<String>) -> Self {
        self.bind_addr = bind_addr.into();
        self
    }
}

#[derive(Debug, Clone)]
pub struct RuntimeHttpServer {
    config: RuntimeHttpConfig,
}

impl RuntimeHttpServer {
    pub fn new(config: RuntimeHttpConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &RuntimeHttpConfig {
        &self.config
    }

    pub fn handle_path(&self, path_and_query: &str) -> Result<RuntimeHttpResponse> {
        self.handle_request("GET", path_and_query)
    }

    pub fn handle_request(
        &self,
        method: &str,
        path_and_query: &str,
    ) -> Result<RuntimeHttpResponse> {
        self.handle_request_with_body(method, path_and_query, "")
    }

    pub fn handle_request_with_body(
        &self,
        method: &str,
        path_and_query: &str,
        body: &str,
    ) -> Result<RuntimeHttpResponse> {
        if method.eq_ignore_ascii_case("OPTIONS") {
            return Ok(RuntimeHttpResponse::empty(204));
        }

        let request = RuntimeHttpRequest::parse(path_and_query)?;

        match (method, request.path.as_str()) {
            ("GET", "/api/health") => self.health_response(&request),
            ("GET", "/api/runtime-registry") => self.runtime_registry_response(),
            ("GET", "/api/discovery") | ("GET", "/.well-known/pool-runtime.json") => {
                self.discovery_response(&request)
            }
            ("GET", "/api/snapshot") => self.snapshot_response(&request),
            ("GET", "/api/projects") => self.projects_response(&request),
            ("GET", "/api/events") => self.events_response(&request),
            ("GET", "/api/events/stream") => self.events_stream_response(&request),
            ("GET", "/api/events/ws") => self.events_websocket_response(&request),
            ("GET", "/api/provider-requests/metadata") => {
                self.provider_request_metadata_response(&request)
            }
            ("GET", "/api/agent-sessions/transcript") => {
                self.agent_session_transcript_response(&request)
            }
            ("GET", "/api/agent-sessions/stream") => self.agent_session_stream_response(&request),
            ("GET", "/api/agent-sessions/ws") => self.agent_session_websocket_response(&request),
            ("GET", "/api/resources") => self.resources_response(&request),
            ("GET", "/api/prompts") => self.prompts_response(&request),
            ("GET", "/api/mcp") => self.mcp_response(&request),
            ("GET", "/api/runtime-graph") => self.runtime_graph_response(&request),
            ("GET", "/api/runtime-execution-plan") => {
                self.runtime_execution_plan_response(&request)
            }
            ("POST", "/api/runtime-execution-plan/run-next") => {
                self.runtime_execution_plan_run_next_response(&request, body)
            }
            ("GET", "/api/runtime-budget") => self.runtime_budget_response(&request),
            ("GET", "/api/runtime-preflight") => self.runtime_preflight_response(&request),
            ("GET", "/api/runtime-handoff") => self.runtime_handoff_response(&request),
            ("GET", "/api/prd-readiness") => self.prd_readiness_response(&request),
            ("GET", "/api/prd-completion-gate") => self.prd_completion_gate_response(&request),
            ("POST", "/api/prd-completion-package") => self.prd_completion_package_response(body),
            ("GET", "/api/production-evidence/requirements") => {
                self.production_evidence_requirements_response(&request)
            }
            ("GET", "/api/production-evidence/tasks") => {
                self.production_evidence_tasks_response(&request)
            }
            ("POST", "/api/production-evidence/tasks/claim") => {
                self.production_evidence_task_claim_response(body)
            }
            ("GET", "/api/production-evidence/run-plan") => {
                self.production_evidence_run_plan_response(&request)
            }
            ("GET", "/api/production-evidence/handoff") => {
                self.production_evidence_handoff_response(&request)
            }
            ("POST", "/api/production-evidence/handoff-packages") => {
                self.production_evidence_handoff_package_response(body)
            }
            ("GET", "/api/workflow-context") => self.workflow_context_response(&request),
            ("GET", "/api/node-context") => self.node_context_response(&request),
            ("GET", "/api/api-keys") => self.api_keys_response(&request),
            ("GET", "/api/adapters") => self.adapters_response(),
            ("GET", "/api/integration-readiness") => self.integration_readiness_response(&request),
            ("GET", "/api/provider-contracts") => self.provider_contracts_response(&request),
            ("GET", "/api/provider-gateway-worker") => self.provider_gateway_worker_response(),
            ("POST", "/api/provider-conformance-packages") => {
                self.provider_conformance_package_response(body)
            }
            ("POST", "/api/integration-conformance-packages") => {
                self.integration_conformance_package_response(body)
            }
            ("GET", "/api/software-contracts") => self.software_contracts_response(&request),
            ("POST", "/api/software-conformance-packages") => {
                self.software_conformance_package_response(body)
            }
            ("GET", "/api/unreal-mcp-bridge") => self.unreal_mcp_bridge_response(),
            ("GET", "/api/output-packages") => self.output_packages_response(&request),
            ("GET", "/api/handoff-packages") => self.handoff_packages_response(&request),
            ("GET", "/api/production-evidence/template") => {
                self.production_evidence_template_response(&request)
            }
            ("GET", "/api/production-evidence/item-template") => {
                self.production_evidence_item_template_response(&request)
            }
            ("GET", "/api/production-evidence/item-from-ledger") => {
                self.production_evidence_item_from_ledger_response(&request)
            }
            ("GET", "/api/production-evidence/bundle-from-ledger") => {
                self.production_evidence_bundle_from_ledger_response(&request)
            }
            ("POST", "/api/adapter-health") => self.adapter_health_response(body),
            ("POST", "/api/provider-health") => self.provider_health_response(body),
            ("POST", "/api/nodes/run") => self.node_run_response(body),
            ("POST", "/api/tasks") => self.create_task_response(body),
            ("POST", "/api/api-keys") => self.upsert_api_key_response(body),
            ("POST", "/api/workflow-runs") => self.workflow_run_response(body),
            ("POST", "/api/provider-runs") => self.provider_run_response(body),
            ("POST", "/api/production-evidence/validate") => {
                self.production_evidence_validate_response(body)
            }
            ("POST", "/api/production-evidence/merge") => {
                self.production_evidence_merge_response(body)
            }
            ("POST", "/api/production-evidence/closeout") => {
                self.production_evidence_closeout_response(body)
            }
            ("POST", "/api/production-evidence/items/validate") => {
                self.production_evidence_item_validate_response(body)
            }
            ("POST", "/api/production-evidence/items") => {
                self.production_evidence_item_response(body)
            }
            ("POST", "/api/production-evidence") => self.production_evidence_response(body),
            ("POST", "/api/output-packages") => self.output_package_response(body),
            ("POST", "/api/output-packages/results") => self.output_package_result_response(body),
            ("POST", "/api/handoff-packages") => self.handoff_package_response(body),
            ("POST", "/api/agent-sessions") => self.agent_session_response(body),
            ("POST", "/api/agent-conformance-packages") => {
                self.agent_conformance_package_response(body)
            }
            ("POST", "/api/tasks/approve") => self.approve_task_response(&request),
            ("POST", "/api/tasks/cancel") => self.cancel_task_response(&request),
            ("POST", "/api/tasks/retry") => self.retry_task_response(&request),
            ("POST", "/api/software-health") => self.software_health_response(body),
            ("POST", "/api/software-actions") => self.software_action_response(body),
            ("GET", "/api/desktop-recognition/requests") => {
                self.desktop_recognition_requests_response(&request)
            }
            ("GET", "/api/desktop-recognition/contract") => {
                self.desktop_recognition_contract_response()
            }
            ("POST", "/api/desktop-recognition/run-next") => {
                self.desktop_recognition_run_next_response(&request, body)
            }
            ("POST", "/api/desktop-recognition/results") => {
                self.desktop_recognition_result_response(body)
            }
            (
                _,
                "/api/health"
                | "/api/runtime-registry"
                | "/api/discovery"
                | "/.well-known/pool-runtime.json"
                | "/api/snapshot"
                | "/api/projects"
                | "/api/events"
                | "/api/events/stream"
                | "/api/events/ws"
                | "/api/provider-requests/metadata"
                | "/api/agent-sessions/transcript"
                | "/api/agent-sessions/stream"
                | "/api/agent-sessions/ws"
                | "/api/resources"
                | "/api/prompts"
                | "/api/mcp"
                | "/api/runtime-graph"
                | "/api/runtime-execution-plan"
                | "/api/runtime-execution-plan/run-next"
                | "/api/runtime-budget"
                | "/api/runtime-preflight"
                | "/api/runtime-handoff"
                | "/api/prd-readiness"
                | "/api/prd-completion-gate"
                | "/api/prd-completion-package"
                | "/api/production-evidence/requirements"
                | "/api/production-evidence/tasks"
                | "/api/production-evidence/tasks/claim"
                | "/api/production-evidence/run-plan"
                | "/api/production-evidence/handoff"
                | "/api/production-evidence/handoff-packages"
                | "/api/workflow-context"
                | "/api/node-context"
                | "/api/api-keys"
                | "/api/adapters"
                | "/api/integration-readiness"
                | "/api/provider-contracts"
                | "/api/provider-gateway-worker"
                | "/api/provider-conformance-packages"
                | "/api/integration-conformance-packages"
                | "/api/software-contracts"
                | "/api/software-conformance-packages"
                | "/api/unreal-mcp-bridge"
                | "/api/adapter-health"
                | "/api/provider-health"
                | "/api/nodes/run"
                | "/api/tasks"
                | "/api/workflow-runs"
                | "/api/provider-runs"
                | "/api/production-evidence/item-template"
                | "/api/production-evidence/item-from-ledger"
                | "/api/production-evidence/bundle-from-ledger"
                | "/api/production-evidence/validate"
                | "/api/production-evidence/items/validate"
                | "/api/production-evidence/items"
                | "/api/production-evidence"
                | "/api/output-packages"
                | "/api/output-packages/results"
                | "/api/handoff-packages"
                | "/api/agent-sessions"
                | "/api/agent-conformance-packages"
                | "/api/tasks/approve"
                | "/api/tasks/cancel"
                | "/api/tasks/retry"
                | "/api/software-health"
                | "/api/software-actions"
                | "/api/desktop-recognition/requests"
                | "/api/desktop-recognition/contract"
                | "/api/desktop-recognition/run-next"
                | "/api/desktop-recognition/results",
            ) => RuntimeHttpResponse::json(
                405,
                json!({
                    "error": "method_not_allowed",
                    "method": method,
                    "path": request.path,
                }),
            ),
            _ => Ok(RuntimeHttpResponse::json(
                404,
                json!({
                    "error": "not_found",
                    "path": request.path,
                }),
            )?),
        }
    }

    pub fn serve_blocking(&self) -> Result<()> {
        let listener = TcpListener::bind(&self.config.bind_addr)
            .with_context(|| format!("bind Pool runtime HTTP server {}", self.config.bind_addr))?;

        for stream in listener.incoming() {
            let stream = stream.context("accept Pool runtime HTTP connection")?;
            let server = self.clone();
            thread::spawn(move || {
                if let Err(error) = server.handle_tcp_connection(stream) {
                    eprintln!("Pool runtime HTTP connection error: {error}");
                }
            });
        }

        Ok(())
    }

    pub fn runtime_registry_value(&self) -> Value {
        let base_url = self.runtime_base_url();
        json!({
            "kind": "pool_runtime_registry",
            "service": "pool-runtime",
            "version": "1",
            "generated_at": chrono::Utc::now().to_rfc3339(),
            "base_url": base_url,
            "runtime_url": base_url,
            "runtime_endpoint": base_url,
            "discovery_url": format!("{}/api/discovery", base_url),
            "well_known_url": format!("{}/.well-known/pool-runtime.json", base_url),
            "bind_addr": self.config.bind_addr,
            "db_path": self.config.db_path.to_string_lossy(),
            "project_slug": self.config.project_slug.clone(),
            "project_filter": self.config.project_slug.clone(),
            "endpoints": runtime_discovery_endpoints(),
            "local_files_authoritative": true,
            "provider_urls_are_provenance": true,
        })
    }

    pub fn write_runtime_registry(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("create runtime registry dir {}", parent.display()))?;
        }
        fs::write(
            path,
            serde_json::to_string_pretty(&self.runtime_registry_value())
                .context("serialize Pool runtime registry")?,
        )
        .with_context(|| format!("write Pool runtime registry {}", path.display()))
    }

    fn handle_tcp_connection(&self, mut stream: TcpStream) -> Result<()> {
        let response = match read_http_request(&mut stream) {
            Ok(request) => {
                let parsed = RuntimeHttpRequest::parse(&request.path)?;
                if request.method == "GET" && parsed.path == "/api/events/stream" {
                    return self.stream_events_to_connection(&mut stream, parsed);
                }
                if request.method == "GET" && parsed.path == "/api/events/ws" {
                    return self.stream_events_websocket_to_connection(
                        &mut stream,
                        parsed,
                        &request.headers,
                    );
                }
                if request.method == "GET" && parsed.path == "/api/agent-sessions/stream" {
                    return self.stream_agent_session_to_connection(&mut stream, parsed);
                }
                if request.method == "GET" && parsed.path == "/api/agent-sessions/ws" {
                    return self.stream_agent_session_websocket_to_connection(
                        &mut stream,
                        parsed,
                        &request.headers,
                    );
                }
                self.handle_request_with_body(&request.method, &request.path, &request.body)
                    .unwrap_or_else(RuntimeHttpResponse::from_error)
            }
            Err(error) => RuntimeHttpResponse::from_error(error),
        };
        stream
            .write_all(response.to_http_bytes().as_bytes())
            .context("write Pool runtime HTTP response")?;
        Ok(())
    }

    fn health_response(&self, request: &RuntimeHttpRequest) -> Result<RuntimeHttpResponse> {
        let snapshot = self.load_snapshot_for_request(request)?;
        RuntimeHttpResponse::json(
            200,
            json!({
                "status": "ready",
                "runtime": "local",
                "project_filter": snapshot.project_filter,
                "stats": snapshot.stats,
            }),
        )
    }

    fn runtime_registry_response(&self) -> Result<RuntimeHttpResponse> {
        RuntimeHttpResponse::json(200, self.runtime_registry_value())
    }

    fn discovery_response(&self, request: &RuntimeHttpRequest) -> Result<RuntimeHttpResponse> {
        let snapshot = self.load_snapshot_for_request(request)?;
        RuntimeHttpResponse::json(
            200,
            json!({
                "service": "pool-runtime",
                "version": snapshot.version,
                "base_url": self.runtime_base_url(),
                "bind_addr": self.config.bind_addr,
                "project_filter": snapshot.project_filter,
                "stats": snapshot.stats,
                "capabilities": {
                    "runtime_http": true,
                    "runtime_registry": true,
                    "sqlite_snapshot": true,
                    "project_registry": true,
                    "event_stream": true,
                    "event_stream_transport": "sse+websocket",
                    "event_stream_transports": ["websocket", "sse", "polling"],
                    "event_websocket": true,
                    "mcp_resources": true,
                    "mcp_tools": true,
                    "mcp_prompts": true,
                    "agent_runbooks": true,
                    "provider_adapters": true,
                    "provider_contracts": true,
                    "provider_gateway_worker": true,
                    "provider_conformance_packages": true,
                    "integration_conformance_packages": true,
                    "integration_readiness": true,
                    "provider_request_metadata": true,
                    "production_evidence_tasks": true,
                    "production_evidence_task_claim": true,
                    "production_evidence_run_plan": true,
                    "production_evidence_handoff": true,
                    "production_evidence_handoff_packages": true,
                    "production_evidence_template": true,
                    "production_evidence_item_template": true,
                    "production_evidence_item_from_ledger": true,
                    "production_evidence_bundle_from_ledger": true,
                    "production_evidence_validate": true,
                    "production_evidence_merge": true,
                    "production_evidence_closeout": true,
                    "production_evidence_item_validate": true,
                    "production_evidence_items": true,
                    "production_evidence_import": true,
                    "software_adapters": true,
                    "software_contracts": true,
                    "software_conformance_packages": true,
                    "unreal_mcp_bridge_contract": true,
                    "agent_sessions": true,
                    "agent_conformance_packages": true,
                    "agent_session_transcripts": true,
                    "agent_session_stream": true,
                    "agent_session_websocket": true,
                    "agent_session_stream_transports": ["websocket", "sse"],
                    "desktop_recognition_handoff": true,
                    "desktop_recognition_contract": true,
                    "api_key_management": true,
                    "runtime_budget": true,
                    "runtime_preflight": true,
                    "runtime_execution_plan": true,
                    "runtime_execution_plan_run_next": true,
                    "runtime_handoff": true,
                    "prd_readiness": true,
                    "prd_completion_gate": true,
                    "prd_completion_package": true,
                    "output_package_catalog": true,
                    "output_package_results": true,
                    "handoff_packages": true,
                    "handoff_package_catalog": true,
                    "local_files_authoritative": true,
                    "provider_urls_are_provenance": true,
                },
                "endpoints": runtime_discovery_endpoints(),
                "mcp_resources": mcp_resource_discovery(),
                "mcp_tools": mcp_tool_discovery(),
                "mcp_prompts": mcp_prompt_discovery(),
                "projects": snapshot.projects,
            }),
        )
    }

    fn provider_run_response(&self, body: &str) -> Result<RuntimeHttpResponse> {
        let request = match serde_json::from_str::<CreateProviderRunRequest>(body) {
            Ok(request) => request,
            Err(error) => {
                return RuntimeHttpResponse::json(
                    400,
                    json!({
                        "error": "invalid_provider_run_request",
                        "message": error.to_string(),
                    }),
                );
            }
        };

        if request.provider_id.trim().is_empty() {
            return RuntimeHttpResponse::json(
                400,
                json!({
                    "error": "missing_provider_id",
                    "expected": "JSON body with provider_id",
                }),
            );
        }

        let provider_id = canonical_provider_id(&request.provider_id);
        let execution_mode = request
            .execution_mode
            .unwrap_or(ProviderRunExecutionMode::Auto);
        let dispatch_options = ProviderRunDispatchOptions {
            endpoint: request.endpoint.clone(),
            api_key: request.api_key.clone(),
        };

        if execution_mode == ProviderRunExecutionMode::Mock && !is_three_dgs_provider(&provider_id)
        {
            return RuntimeHttpResponse::json(
                400,
                json!({
                    "error": "provider_mock_not_available",
                    "provider_id": request.provider_id,
                    "message": "mock execution is only available for 3DGS providers",
                }),
            );
        }

        let inputs = self.provider_run_inputs(&request, &provider_id, execution_mode);

        self.dispatch_provider_run(&provider_id, execution_mode, dispatch_options, inputs)
    }

    fn provider_health_response(&self, body: &str) -> Result<RuntimeHttpResponse> {
        let request = match serde_json::from_str::<CheckProviderHealthRequest>(body) {
            Ok(request) => request,
            Err(error) => {
                return RuntimeHttpResponse::json(
                    400,
                    json!({
                        "error": "invalid_provider_health_request",
                        "message": error.to_string(),
                    }),
                );
            }
        };

        if request.provider_id.trim().is_empty() {
            return RuntimeHttpResponse::json(
                400,
                json!({
                    "error": "missing_provider_id",
                    "expected": "JSON body with provider_id",
                }),
            );
        }

        let provider_id = canonical_provider_id(&request.provider_id);
        let execution_mode = request
            .execution_mode
            .unwrap_or(ProviderRunExecutionMode::Auto);
        let dispatch_options = ProviderRunDispatchOptions {
            endpoint: request.endpoint.clone(),
            api_key: request.api_key.clone(),
        };

        if execution_mode == ProviderRunExecutionMode::Mock && !is_three_dgs_provider(&provider_id)
        {
            return RuntimeHttpResponse::json(
                400,
                json!({
                    "error": "provider_mock_not_available",
                    "provider_id": request.provider_id,
                    "message": "mock health is only available for 3DGS providers",
                }),
            );
        }

        self.dispatch_provider_health(&provider_id, execution_mode, dispatch_options)
    }

    fn node_run_response(&self, body: &str) -> Result<RuntimeHttpResponse> {
        let request = match serde_json::from_str::<RunNodeRequest>(body) {
            Ok(request) => request,
            Err(error) => {
                return RuntimeHttpResponse::json(
                    400,
                    json!({
                        "error": "invalid_node_run_request",
                        "message": error.to_string(),
                    }),
                );
            }
        };

        if request.node_id.trim().is_empty() {
            return RuntimeHttpResponse::json(
                400,
                json!({
                    "error": "missing_node_id",
                    "expected": "JSON body with node_id",
                }),
            );
        }

        let project_slug = request
            .project_slug
            .clone()
            .or_else(|| self.config.project_slug.clone())
            .unwrap_or_else(|| "demo".to_string());
        let repository = RuntimeRepository::open(&self.config.db_path)?;
        repository.migrate()?;
        let snapshot = repository.snapshot(Some(&project_slug))?;
        let Some(node) = runtime_node_from_snapshot(&snapshot, &request.node_id, &project_slug)
        else {
            return RuntimeHttpResponse::json(
                404,
                json!({
                    "error": "node_not_found",
                    "node_id": request.node_id,
                    "project_slug": project_slug,
                }),
            );
        };
        let control_context = node_control_context_from_snapshot(&snapshot, &request.node_id);
        drop(repository);

        if is_output_node_type(&node.node_type) {
            return self.output_package_response(
                &json!({
                    "project_slug": node.project_slug,
                    "node_id": node.id,
                    "title": format!("Run node: {}", node.title),
                    "source_assets": request.input_paths.clone().unwrap_or_default(),
                    "duration_ms": request.duration_ms.unwrap_or(12_000),
                    "output_dir": request.output_dir,
                    "control_context": control_context,
                })
                .to_string(),
            );
        }

        if is_agent_node_type(&node.node_type) || node.provider_id.as_deref() == Some("hermes") {
            let instruction = request
                .prompt
                .clone()
                .unwrap_or_else(|| format!("Run workflow node {}: {}", node.id, node.title));
            return self.agent_session_response(
                &json!({
                    "kind": "hermes",
                    "project_slug": node.project_slug,
                    "title": format!("Run node: {}", node.title),
                    "instruction": format!("{instruction}\n\nNode control_context:\n{}", control_context),
                    "allowed_tools": ["api", "mcp", "skills", "cli", "desktop"],
                    "requires_confirmation": node.requires_approval,
                })
                .to_string(),
            );
        }

        if let Some(provider_id) = node.provider_id.clone() {
            let provider_id = canonical_provider_id(&provider_id);
            let provider_run_request = CreateProviderRunRequest {
                project_slug: Some(node.project_slug.clone()),
                node_id: Some(node.id.clone()),
                task_title: Some(format!("Run node: {}", node.title)),
                provider_id,
                execution_mode: request.execution_mode,
                endpoint: request.endpoint.clone(),
                api_key: request.api_key.clone(),
                prompt: request
                    .prompt
                    .clone()
                    .or_else(|| node_parameter_string(&node.parameters, "prompt"))
                    .or_else(|| Some(format!("Run workflow node: {}", node.title))),
                input_paths: request
                    .input_paths
                    .clone()
                    .or_else(|| node_parameter_string_array(&node.parameters, "input_paths")),
                output_dir: request
                    .output_dir
                    .clone()
                    .or_else(|| Some(format!("worlds/{}/output", node.project_slug))),
                cost_estimate_tokens: (node.cost_estimate_tokens > 0)
                    .then_some(node.cost_estimate_tokens),
                requires_approval: node.requires_approval.then_some(true),
                evidence_json: None,
            };
            let provider_id = canonical_provider_id(&provider_run_request.provider_id);
            let execution_mode = provider_run_request
                .execution_mode
                .unwrap_or(ProviderRunExecutionMode::Auto);
            let dispatch_options = ProviderRunDispatchOptions {
                endpoint: provider_run_request.endpoint.clone(),
                api_key: provider_run_request.api_key.clone(),
            };
            let mut inputs =
                self.provider_run_inputs(&provider_run_request, &provider_id, execution_mode);
            inputs.control_context = Some(control_context);
            return self.dispatch_provider_run(
                &provider_id,
                execution_mode,
                dispatch_options,
                inputs,
            );
        }

        if let Some(adapter_id) = node.software_adapter_id.clone() {
            let payload_json = json!({
                "node_id": node.id,
                "node_type": node.node_type,
                "title": node.title,
                "parameters": node.parameters,
                "endpoint": request.endpoint,
                "requested_from": "runtime-node-run",
                "control_context": control_context,
            });
            return self.software_action_response(
                &json!({
                    "project_slug": node.project_slug,
                    "node_id": node.id,
                    "task_title": format!("Run node: {}", node.title),
                    "adapter_id": adapter_id,
                    "action_kind": software_action_kind_for_node(&node.node_type, &adapter_id),
                    "priority": "ApiMcp",
                    "payload_json": payload_json,
                    "requires_confirmation": node.requires_approval,
                })
                .to_string(),
            );
        }

        self.create_task_response(
            &json!({
                "project_slug": node.project_slug,
                "node_id": node.id,
                "title": format!("Run node: {}", node.title),
                "cost_estimate_tokens": node.cost_estimate_tokens,
                "requires_approval": node.requires_approval,
            })
            .to_string(),
        )
    }

    fn dispatch_provider_run(
        &self,
        provider_id: &str,
        execution_mode: ProviderRunExecutionMode,
        dispatch: ProviderRunDispatchOptions,
        inputs: ProviderRunInputs,
    ) -> Result<RuntimeHttpResponse> {
        if provider_id == "mock-3dgs" || should_use_mock_3dgs(&provider_id, execution_mode) {
            let provider = Mock3dgsProvider::new(
                provider_id.to_string(),
                display_name_for_provider(provider_id),
            );
            return self.run_provider_adapter_response(provider, inputs);
        }

        match provider_id {
            "comfyui" => {
                let mut options = ComfyUiProviderOptions::from_env();
                if let Some(endpoint) = dispatch.endpoint.clone() {
                    options.endpoint = endpoint;
                }
                self.run_provider_adapter_response(ComfyUiProvider::new(options), inputs)
            }
            "kling" => {
                let mut options = KlingProviderOptions::from_env();
                if let Some(endpoint) = dispatch.endpoint.clone() {
                    options.endpoint = endpoint;
                }
                if let Some(api_key) = dispatch
                    .api_key
                    .clone()
                    .or(self.stored_api_key(provider_id, "provider")?)
                {
                    options.auth = Some(KlingAuth::BearerToken(api_key));
                }
                if options.auth.is_none() {
                    return provider_not_configured_response(
                        &provider_id,
                        "Kling auth missing: set POOL_KLING_API_KEY or POOL_KLING_ACCESS_KEY/POOL_KLING_SECRET_KEY",
                    );
                }
                self.run_provider_adapter_response(KlingProvider::new(options), inputs)
            }
            "openai-image-2" => {
                let mut options = OpenAiImageProviderOptions::from_env();
                if let Some(endpoint) = dispatch.endpoint.clone() {
                    options.endpoint = endpoint;
                }
                if let Some(api_key) = dispatch
                    .api_key
                    .clone()
                    .or(self.stored_api_key(provider_id, "provider")?)
                {
                    options.api_key = Some(api_key);
                }
                if options.api_key.is_none() {
                    return provider_not_configured_response(
                        &provider_id,
                        "OpenAI image auth missing: set OPENAI_API_KEY",
                    );
                }
                self.run_provider_adapter_response(OpenAiImageProvider::new(options), inputs)
            }
            provider_id if is_http_media_provider(provider_id) => {
                let Some((display_name, kind, auth_env_key, output_extension, high_cost)) =
                    http_media_provider_defaults(provider_id)
                else {
                    return RuntimeHttpResponse::json(
                        501,
                        json!({
                            "error": "provider_not_executable",
                            "provider_id": provider_id,
                        }),
                    );
                };
                let mut options = GenericHttpMediaOptions::from_env(
                    provider_id.to_string(),
                    display_name,
                    kind,
                    auth_env_key,
                    output_extension,
                    high_cost,
                );
                if let Some(endpoint) = dispatch.endpoint.clone() {
                    options.endpoint = endpoint;
                }
                if options.endpoint.trim().is_empty() {
                    return provider_not_configured_response(
                        provider_id,
                        &format!(
                            "{provider_id} media gateway missing: pass endpoint in /api/provider-runs or set POOL_{}_ENDPOINT / POOL_MEDIA_GATEWAY_ENDPOINT",
                            provider_env_prefix(provider_id)
                        ),
                    );
                }
                if let Some(api_key) = dispatch
                    .api_key
                    .clone()
                    .or(self.stored_api_key(provider_id, "provider")?)
                {
                    options.api_key = Some(api_key);
                }
                self.run_provider_adapter_response(GenericHttpMediaProvider::new(options), inputs)
            }
            provider_id if is_three_dgs_provider(provider_id) => {
                let mut options = ThreeDgsGatewayOptions::from_env(
                    provider_id.to_string(),
                    display_name_for_provider(provider_id),
                    None,
                );
                if let Some(endpoint) = dispatch.endpoint.clone() {
                    options.endpoint = endpoint;
                }
                if let Some(api_key) = dispatch
                    .api_key
                    .clone()
                    .or(self.stored_api_key(provider_id, "provider")?)
                {
                    options.api_key = Some(api_key);
                }
                self.run_provider_adapter_response(ThreeDgsGatewayProvider::new(options), inputs)
            }
            _ => RuntimeHttpResponse::json(
                501,
                json!({
                    "error": "provider_not_executable",
                    "provider_id": provider_id,
                    "message": "Runtime HTTP provider run does not have an executable adapter for this provider yet; use /api/tasks to stage it.",
                    "supported_adapters": [
                        "comfyui",
                        "kling",
                        "openai-image-2",
                        "midjourney",
                        "nano-banana-pro",
                        "suno",
                        "worldlabs-marble",
                        "tripo-splat",
                        "sam-3d",
                        "spark-3dgs",
                        "qunhe-3d"
                    ],
                }),
            ),
        }
    }

    fn dispatch_provider_health(
        &self,
        provider_id: &str,
        execution_mode: ProviderRunExecutionMode,
        dispatch: ProviderRunDispatchOptions,
    ) -> Result<RuntimeHttpResponse> {
        if provider_id == "mock-3dgs" || should_use_mock_3dgs(provider_id, execution_mode) {
            let provider = Mock3dgsProvider::new(
                provider_id.to_string(),
                display_name_for_provider(provider_id),
            );
            return self.run_provider_health_response(provider, execution_mode, "mock");
        }

        match provider_id {
            "comfyui" => {
                let mut options = ComfyUiProviderOptions::from_env();
                if let Some(endpoint) = dispatch.endpoint.clone() {
                    options.endpoint = endpoint;
                }
                self.run_provider_health_response(
                    ComfyUiProvider::new(options),
                    execution_mode,
                    "adapter",
                )
            }
            "kling" => {
                let mut options = KlingProviderOptions::from_env();
                if let Some(endpoint) = dispatch.endpoint.clone() {
                    options.endpoint = endpoint;
                }
                if let Some(api_key) = dispatch
                    .api_key
                    .clone()
                    .or(self.stored_api_key(provider_id, "provider")?)
                {
                    options.auth = Some(KlingAuth::BearerToken(api_key));
                }
                self.run_provider_health_response(
                    KlingProvider::new(options),
                    execution_mode,
                    "adapter",
                )
            }
            "openai-image-2" => {
                let mut options = OpenAiImageProviderOptions::from_env();
                if let Some(endpoint) = dispatch.endpoint.clone() {
                    options.endpoint = endpoint;
                }
                if let Some(api_key) = dispatch
                    .api_key
                    .clone()
                    .or(self.stored_api_key(provider_id, "provider")?)
                {
                    options.api_key = Some(api_key);
                }
                self.run_provider_health_response(
                    OpenAiImageProvider::new(options),
                    execution_mode,
                    "adapter",
                )
            }
            provider_id if is_http_media_provider(provider_id) => {
                let Some((display_name, kind, auth_env_key, output_extension, high_cost)) =
                    http_media_provider_defaults(provider_id)
                else {
                    return provider_not_executable_response(provider_id);
                };
                let mut options = GenericHttpMediaOptions::from_env(
                    provider_id.to_string(),
                    display_name,
                    kind,
                    auth_env_key,
                    output_extension,
                    high_cost,
                );
                if let Some(endpoint) = dispatch.endpoint.clone() {
                    options.endpoint = endpoint;
                }
                if let Some(api_key) = dispatch
                    .api_key
                    .clone()
                    .or(self.stored_api_key(provider_id, "provider")?)
                {
                    options.api_key = Some(api_key);
                }
                self.run_provider_health_response(
                    GenericHttpMediaProvider::new(options),
                    execution_mode,
                    "gateway",
                )
            }
            provider_id if is_three_dgs_provider(provider_id) => {
                let mut options = ThreeDgsGatewayOptions::from_env(
                    provider_id.to_string(),
                    display_name_for_provider(provider_id),
                    None,
                );
                if let Some(endpoint) = dispatch.endpoint.clone() {
                    options.endpoint = endpoint;
                }
                if let Some(api_key) = dispatch
                    .api_key
                    .clone()
                    .or(self.stored_api_key(provider_id, "provider")?)
                {
                    options.api_key = Some(api_key);
                }
                self.run_provider_health_response(
                    ThreeDgsGatewayProvider::new(options),
                    execution_mode,
                    "gateway",
                )
            }
            _ => provider_not_executable_response(provider_id),
        }
    }

    fn run_provider_health_response<A>(
        &self,
        adapter: A,
        execution_mode: ProviderRunExecutionMode,
        adapter_mode: &str,
    ) -> Result<RuntimeHttpResponse>
    where
        A: ProviderAdapter,
    {
        let config = adapter.config().clone();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .context("create provider health tokio runtime")?;

        match runtime.block_on(adapter.health()) {
            Ok(health) => RuntimeHttpResponse::json(
                200,
                json!({
                    "provider_id": config.id,
                    "display_name": config.display_name,
                    "kind": config.kind,
                    "endpoint": config.endpoint,
                    "execution_mode": execution_mode,
                    "adapter_mode": adapter_mode,
                    "health": health,
                }),
            ),
            Err(error) => RuntimeHttpResponse::json(
                502,
                json!({
                    "error": "provider_health_failed",
                    "provider_id": config.id,
                    "display_name": config.display_name,
                    "kind": config.kind,
                    "endpoint": config.endpoint,
                    "execution_mode": execution_mode,
                    "adapter_mode": adapter_mode,
                    "message": error.to_string(),
                }),
            ),
        }
    }

    fn stored_api_key(&self, provider_id: &str, service_type: &str) -> Result<Option<String>> {
        let repository = RuntimeRepository::open(&self.config.db_path)?;
        repository.migrate()?;
        repository.api_key_secret(provider_id, service_type)
    }

    fn provider_run_inputs(
        &self,
        request: &CreateProviderRunRequest,
        provider_id: &str,
        execution_mode: ProviderRunExecutionMode,
    ) -> ProviderRunInputs {
        let project_slug = request
            .project_slug
            .clone()
            .or_else(|| self.config.project_slug.clone())
            .unwrap_or_else(|| "demo".to_string());
        let title = request
            .task_title
            .clone()
            .unwrap_or_else(|| format!("{provider_id} provider run"));
        let requires_approval = request.requires_approval.unwrap_or(false);
        let mut task = RuntimeTask::new(project_slug.clone(), title);
        task.node_id = request.node_id.clone();
        task.provider_id = Some(provider_id.to_string());
        task.cost_estimate_tokens = request.cost_estimate_tokens.unwrap_or(0);
        task.requires_approval = requires_approval;
        task.status = if requires_approval {
            TaskStatus::WaitingApproval
        } else {
            TaskStatus::Ready
        };

        let provider_request = ProviderRequest {
            project_slug: project_slug.clone(),
            prompt: request
                .prompt
                .clone()
                .unwrap_or_else(|| default_prompt_for_provider(provider_id)),
            input_paths: request.input_paths.clone().unwrap_or_default(),
            output_dir: request
                .output_dir
                .clone()
                .unwrap_or_else(|| format!("worlds/{project_slug}/output")),
            require_approval: requires_approval,
        };

        ProviderRunInputs {
            task,
            request: provider_request,
            execution_mode,
            endpoint: request.endpoint.clone(),
            control_context: None,
            evidence_json: request.evidence_json.clone(),
            inline_api_key_provided: request
                .api_key
                .as_ref()
                .is_some_and(|api_key| !api_key.trim().is_empty()),
            cost_estimate_explicit: request.cost_estimate_tokens.is_some(),
            requires_approval_explicit: request.requires_approval.is_some(),
            resume_provider_request_id: None,
            retry_of_provider_request_id: None,
        }
    }

    fn run_provider_adapter_response<A>(
        &self,
        adapter: A,
        mut inputs: ProviderRunInputs,
    ) -> Result<RuntimeHttpResponse>
    where
        A: ProviderAdapter,
    {
        let repository = RuntimeRepository::open(&self.config.db_path)?;
        repository.migrate()?;
        let runner = ProviderTaskRunner::new(&repository);

        let estimated_cost = if inputs.cost_estimate_explicit {
            inputs.task.cost_estimate_tokens
        } else {
            adapter.estimate_cost_tokens(&inputs.request)
        };
        if !inputs.cost_estimate_explicit {
            inputs.task.cost_estimate_tokens = estimated_cost;
        }
        if !inputs.requires_approval_explicit
            && adapter.config().high_cost
            && estimated_cost >= PROVIDER_AUTO_APPROVAL_COST_TOKENS
        {
            inputs.task.requires_approval = true;
            inputs.task.status = TaskStatus::WaitingApproval;
            inputs.request.require_approval = true;
        }

        let approval_handoff_path =
            if inputs.task.requires_approval && inputs.task.status == TaskStatus::WaitingApproval {
                let path = provider_approval_handoff_path(&adapter.config().id, &inputs);
                inputs.task.request_metadata_path = Some(path.clone());
                Some(path)
            } else {
                None
            };

        let task_id = inputs.task.id.clone();
        let project_slug = inputs.task.project_slug.clone();
        let provider_id = adapter.config().id.clone();
        let ledger_json = provider_run_ledger_json(&provider_id, &inputs);
        if approval_handoff_path.is_some() {
            write_provider_approval_handoff(&provider_id, &inputs, &ledger_json)?;
        }
        let resume_provider_request_id = inputs.resume_provider_request_id.clone();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .context("create provider run tokio runtime")?;

        match runtime.block_on(runner.run(&adapter, inputs.task, inputs.request)) {
            Ok(report) => {
                let provider_request_id = match resume_provider_request_id.clone() {
                    Some(request_id) => Some(request_id),
                    None => Some(
                        repository
                            .insert_provider_request(
                                &task_id,
                                &provider_id,
                                &ledger_json,
                                approval_handoff_path.as_deref(),
                            )?
                            .id,
                    ),
                };
                if let Some(request_id) = provider_request_id.as_deref() {
                    let response_json =
                        serde_json::to_value(&report).context("serialize provider run report")?;
                    let metadata_path = report
                        .job
                        .as_ref()
                        .map(|job| job.request_metadata_path.as_str());
                    repository.update_provider_request_response(
                        request_id,
                        &response_json,
                        metadata_path,
                    )?;
                }
                let task = repository.task_snapshot(&report.task_id)?;
                let snapshot = repository.snapshot(self.config.project_slug.as_deref())?;

                RuntimeHttpResponse::json(
                    200,
                    json!({
                        "report": report,
                        "provider_request_id": provider_request_id,
                        "task": task,
                        "snapshot": snapshot,
                    }),
                )
            }
            Err(error) => {
                let message = error.to_string();
                let provider_request_id = match resume_provider_request_id.clone() {
                    Some(request_id) => Some(request_id),
                    None => Some(
                        repository
                            .insert_provider_request(
                                &task_id,
                                &provider_id,
                                &ledger_json,
                                approval_handoff_path.as_deref(),
                            )?
                            .id,
                    ),
                };
                if let Some(request_id) = provider_request_id.as_deref() {
                    let response_json = json!({
                        "status": "Failed",
                        "error": message,
                    });
                    let _ = repository.update_provider_request_response(
                        request_id,
                        &response_json,
                        None,
                    );
                }
                let _ = repository.update_task_status(&task_id, TaskStatus::Failed);
                let _ = repository.insert_event(&RuntimeEvent::new(
                    project_slug,
                    RuntimeEventLevel::Error,
                    format!("provider run failed: {message}"),
                ));
                let task = repository.task_snapshot(&task_id).ok();
                let snapshot = repository.snapshot(self.config.project_slug.as_deref())?;

                RuntimeHttpResponse::json(
                    502,
                    json!({
                        "error": "provider_run_failed",
                        "provider_id": provider_id,
                        "message": message,
                        "provider_request_id": provider_request_id,
                        "task": task,
                        "snapshot": snapshot,
                    }),
                )
            }
        }
    }

    fn workflow_run_response(&self, body: &str) -> Result<RuntimeHttpResponse> {
        let request = match serde_json::from_str::<CreateWorkflowRunRequest>(body) {
            Ok(request) => request,
            Err(error) => {
                return RuntimeHttpResponse::json(
                    400,
                    json!({
                        "error": "invalid_workflow_run_request",
                        "message": error.to_string(),
                    }),
                );
            }
        };
        let project_slug = request
            .project_slug
            .clone()
            .or_else(|| self.config.project_slug.clone())
            .unwrap_or_else(|| "demo".to_string());
        let output_root = request
            .output_root
            .unwrap_or_else(|| self.default_output_root().to_string_lossy().to_string());
        let title = request
            .title
            .filter(|title| !title.trim().is_empty())
            .unwrap_or_else(|| "Pool content burst run".to_string());
        let prompt = request
            .prompt
            .filter(|prompt| !prompt.trim().is_empty())
            .unwrap_or_else(|| "generate a local content burst package".to_string());

        let repository = RuntimeRepository::open(&self.config.db_path)?;
        repository.migrate()?;
        let runner = ContentBurstRunner::new(&repository);
        let report = runner.run(ContentBurstRunRequest {
            project_slug,
            output_root,
            title,
            prompt,
            source_inputs: request.source_inputs.unwrap_or_default(),
            duration_ms: request.duration_ms.unwrap_or(12_000),
            three_dgs_mode: request.three_dgs_mode.unwrap_or_default(),
            three_dgs_provider_id: request.three_dgs_provider_id,
            three_dgs_endpoint: request.three_dgs_endpoint,
            three_dgs_api_key: request.three_dgs_api_key,
            unreal_mode: request.unreal_mode.unwrap_or_default(),
            unreal_endpoint: request.unreal_endpoint,
            unreal_auth_token: request.unreal_auth_token,
            agent_mode: request.agent_mode.unwrap_or_default(),
            hermes_endpoint: request.hermes_endpoint,
            hermes_auth_token: request.hermes_auth_token,
            agent_requires_confirmation: request.agent_requires_confirmation.unwrap_or(false),
        })?;
        let snapshot = repository.snapshot(self.config.project_slug.as_deref())?;

        RuntimeHttpResponse::json(
            201,
            json!({
                "report": report,
                "snapshot": snapshot,
            }),
        )
    }

    fn output_package_response(&self, body: &str) -> Result<RuntimeHttpResponse> {
        let request = match serde_json::from_str::<CreateOutputPackageRequest>(body) {
            Ok(request) => request,
            Err(error) => {
                return RuntimeHttpResponse::json(
                    400,
                    json!({
                        "error": "invalid_output_package_request",
                        "message": error.to_string(),
                    }),
                );
            }
        };
        let project_slug = request
            .project_slug
            .clone()
            .or_else(|| self.config.project_slug.clone())
            .unwrap_or_else(|| "demo".to_string());
        let output_dir = request.output_dir.unwrap_or_else(|| {
            self.default_output_dir(&project_slug)
                .to_string_lossy()
                .to_string()
        });
        let title = request
            .title
            .filter(|title| !title.trim().is_empty())
            .unwrap_or_else(|| "Pool output package".to_string());

        let repository = RuntimeRepository::open(&self.config.db_path)?;
        repository.migrate()?;
        let runner = OutputPackageRunner::new(&repository);
        let report = runner.run(OutputPackageRequest {
            project_slug,
            node_id: request.node_id,
            output_dir,
            title,
            source_assets: request.source_assets.unwrap_or_default(),
            duration_ms: request.duration_ms.unwrap_or(12_000),
        })?;
        let task = repository.task_snapshot(&report.task_id)?;
        let snapshot = repository.snapshot(self.config.project_slug.as_deref())?;

        RuntimeHttpResponse::json(
            201,
            json!({
                "report": report,
                "task": task,
                "snapshot": snapshot,
            }),
        )
    }

    fn output_packages_response(
        &self,
        request: &RuntimeHttpRequest,
    ) -> Result<RuntimeHttpResponse> {
        let snapshot = self.load_snapshot_for_request(request)?;
        RuntimeHttpResponse::json(200, output_package_catalog_resource(&snapshot))
    }

    fn handoff_packages_response(
        &self,
        request: &RuntimeHttpRequest,
    ) -> Result<RuntimeHttpResponse> {
        let snapshot = self.load_snapshot_for_request(request)?;
        RuntimeHttpResponse::json(200, runtime_handoff_package_catalog_resource(&snapshot))
    }

    fn output_package_result_response(&self, body: &str) -> Result<RuntimeHttpResponse> {
        let request = match serde_json::from_str::<RecordOutputPackageResultRequest>(body) {
            Ok(request) => request,
            Err(error) => {
                return RuntimeHttpResponse::json(
                    400,
                    json!({
                        "error": "invalid_output_package_result_request",
                        "message": error.to_string(),
                    }),
                );
            }
        };
        if request.target.trim().is_empty() {
            return RuntimeHttpResponse::json(
                400,
                json!({
                    "error": "missing_target",
                    "expected": "JSON body with target: video | game | interactive_art",
                }),
            );
        }
        if request.status.trim().is_empty() {
            return RuntimeHttpResponse::json(
                400,
                json!({
                    "error": "missing_status",
                    "expected": "JSON body with status",
                }),
            );
        }

        let project_slug = request
            .project_slug
            .clone()
            .or_else(|| self.config.project_slug.clone())
            .unwrap_or_else(|| "demo".to_string());
        let repository = RuntimeRepository::open(&self.config.db_path)?;
        repository.migrate()?;
        let runner = OutputPackageRunner::new(&repository);
        let report = runner.record_result(OutputDeliverableResultRequest {
            project_slug,
            node_id: request.node_id,
            target: request.target,
            local_path: request.local_path,
            status: request.status,
            runtime: request.runtime,
            adapter_id: request.adapter_id,
            software_action_id: request.software_action_id,
            message: request.message,
            artifacts: request.artifacts.unwrap_or_default(),
            metrics: request.metrics.unwrap_or_default(),
            verification: request.verification,
        })?;
        let task = repository.task_snapshot(&report.task_id)?;
        let snapshot = repository.snapshot(self.config.project_slug.as_deref())?;

        RuntimeHttpResponse::json(
            201,
            json!({
                "report": report,
                "task": task,
                "snapshot": snapshot,
            }),
        )
    }

    fn handoff_package_response(&self, body: &str) -> Result<RuntimeHttpResponse> {
        let request = match serde_json::from_str::<CreateHandoffPackageRequest>(body) {
            Ok(request) => request,
            Err(error) => {
                return RuntimeHttpResponse::json(
                    400,
                    json!({
                        "error": "invalid_handoff_package_request",
                        "message": error.to_string(),
                    }),
                );
            }
        };
        let project_slug = request
            .project_slug
            .clone()
            .or_else(|| self.config.project_slug.clone())
            .unwrap_or_else(|| "demo".to_string());
        let output_dir = request.output_dir.unwrap_or_else(|| {
            self.default_output_dir(&project_slug)
                .to_string_lossy()
                .to_string()
        });
        let title = request
            .title
            .filter(|title| !title.trim().is_empty())
            .unwrap_or_else(|| "Pool runtime handoff package".to_string());

        let repository = RuntimeRepository::open(&self.config.db_path)?;
        repository.migrate()?;
        let runner = RuntimeHandoffPackageRunner::new(&repository);
        let report = runner.run(RuntimeHandoffPackageRequest {
            project_slug,
            node_id: request.node_id,
            output_dir,
            title,
            include_snapshot: request.include_snapshot.unwrap_or(false),
        })?;
        let task = repository.task_snapshot(&report.task_id)?;
        let snapshot = repository.snapshot(self.config.project_slug.as_deref())?;

        RuntimeHttpResponse::json(
            201,
            json!({
                "report": report,
                "task": task,
                "snapshot": snapshot,
            }),
        )
    }

    fn software_conformance_package_response(&self, body: &str) -> Result<RuntimeHttpResponse> {
        let request = match serde_json::from_str::<CreateSoftwareConformancePackageRequest>(body) {
            Ok(request) => request,
            Err(error) => {
                return RuntimeHttpResponse::json(
                    400,
                    json!({
                        "error": "invalid_software_conformance_package_request",
                        "message": error.to_string(),
                    }),
                );
            }
        };
        let adapter_id = request.adapter_id.trim().to_string();
        if adapter_id.is_empty() {
            return RuntimeHttpResponse::json(
                400,
                json!({
                    "error": "missing_adapter_id",
                    "message": "software conformance package requires adapter_id",
                }),
            );
        }
        let Some(contract) = software_control_contract(&adapter_id) else {
            return RuntimeHttpResponse::json(
                404,
                json!({
                    "error": "software_contract_not_found",
                    "adapter_id": adapter_id,
                }),
            );
        };
        let canonical_adapter_id = contract["adapter_id"]
            .as_str()
            .unwrap_or(adapter_id.as_str())
            .to_string();
        let project_slug = request
            .project_slug
            .clone()
            .or_else(|| self.config.project_slug.clone())
            .unwrap_or_else(|| "demo".to_string());
        let output_dir = request.output_dir.unwrap_or_else(|| {
            self.default_output_dir(&project_slug)
                .to_string_lossy()
                .to_string()
        });
        let title = request
            .title
            .filter(|title| !title.trim().is_empty())
            .unwrap_or_else(|| {
                format!("Pool software conformance package: {canonical_adapter_id}")
            });

        let repository = RuntimeRepository::open(&self.config.db_path)?;
        repository.migrate()?;
        let mut task = RuntimeTask::new(project_slug.clone(), title.clone());
        task.node_id = request.node_id.clone();
        task.provider_id = Some("software-conformance-package".to_string());
        task.status = TaskStatus::Running;
        task.cost_estimate_tokens = 120;
        repository.insert_task(&task)?;
        repository.insert_event(&RuntimeEvent::new(
            project_slug.clone(),
            RuntimeEventLevel::Info,
            format!("software conformance package started: {canonical_adapter_id}"),
        ))?;

        let package_dir = Path::new(&output_dir)
            .join("control")
            .join("software-conformance")
            .join(safe_package_segment(&canonical_adapter_id));
        let report = write_software_conformance_package(
            &package_dir,
            &project_slug,
            request.node_id.as_deref(),
            &canonical_adapter_id,
            &title,
            &contract,
        )?;
        let local_paths = report["local_paths"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        let assets = repository.index_local_outputs(
            &project_slug,
            request.node_id.as_deref(),
            Some(&format!(
                "pool-software-conformance://{canonical_adapter_id}"
            )),
            &local_paths,
        )?;
        repository.update_task_status(&task.id, TaskStatus::Succeeded)?;
        repository.insert_event(&RuntimeEvent::new(
            project_slug.clone(),
            RuntimeEventLevel::Ok,
            format!(
                "software conformance package succeeded: {canonical_adapter_id}, {} files",
                assets.len()
            ),
        ))?;
        let task = repository.task_snapshot(&task.id)?;
        let snapshot = repository.snapshot(self.config.project_slug.as_deref())?;

        RuntimeHttpResponse::json(
            201,
            json!({
                "report": report,
                "assets": assets,
                "task": task,
                "snapshot": snapshot,
            }),
        )
    }

    fn software_action_response(&self, body: &str) -> Result<RuntimeHttpResponse> {
        let request = match serde_json::from_str::<CreateSoftwareActionRequest>(body) {
            Ok(request) => request,
            Err(error) => {
                return RuntimeHttpResponse::json(
                    400,
                    json!({
                        "error": "invalid_software_action_request",
                        "message": error.to_string(),
                    }),
                );
            }
        };

        if request.adapter_id.trim().is_empty() {
            return RuntimeHttpResponse::json(
                400,
                json!({
                    "error": "missing_adapter_id",
                    "expected": "JSON body with adapter_id",
                }),
            );
        }

        let project_slug = request
            .project_slug
            .clone()
            .or_else(|| self.config.project_slug.clone())
            .unwrap_or_else(|| "demo".to_string());
        let mut task = RuntimeTask::new(
            project_slug.clone(),
            request
                .task_title
                .clone()
                .unwrap_or_else(|| format!("{} software action", request.adapter_id)),
        );
        task.node_id = request.node_id.clone();
        task.provider_id = Some(request.adapter_id.clone());
        task.requires_approval = request.requires_confirmation.unwrap_or(false);
        let mut payload_json = request.payload_json.unwrap_or_else(|| json!({}));
        if let Some(evidence_json) = request.evidence_json.clone() {
            merge_payload_child_object(&mut payload_json, "evidence", evidence_json);
        }
        let action = SoftwareControlAction {
            adapter_id: request.adapter_id.clone(),
            action_kind: request
                .action_kind
                .unwrap_or(SoftwareActionKind::HealthCheck),
            priority: request.priority.unwrap_or(ControlPriority::ApiMcp),
            payload_json,
            requires_confirmation: request.requires_confirmation.unwrap_or(false),
        };

        let repository = RuntimeRepository::open(&self.config.db_path)?;
        repository.migrate()?;

        self.dispatch_software_action_response(&repository, &project_slug, task, action)
    }

    fn production_evidence_template_response(
        &self,
        request: &RuntimeHttpRequest,
    ) -> Result<RuntimeHttpResponse> {
        let project_slug = request
            .query
            .get("project")
            .or_else(|| request.query.get("project_slug"))
            .cloned()
            .or_else(|| self.config.project_slug.clone())
            .unwrap_or_else(|| "demo".to_string());
        if project_slug == "*" {
            return RuntimeHttpResponse::json(
                400,
                json!({
                    "error": "production_evidence_template_requires_project",
                    "expected": "Use a concrete project slug, not *",
                }),
            );
        }
        let output_root = request
            .query
            .get("output_root")
            .or_else(|| request.query.get("output-root"))
            .map(String::as_str)
            .filter(|value| !value.trim().is_empty());
        let source = request
            .query
            .get("source")
            .map(String::as_str)
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("replace-with-production-evidence-source");
        let missing_only = request
            .query
            .get("missing_only")
            .or_else(|| request.query.get("missing-only"))
            .is_some_and(|value| query_bool(value));
        let scope = if missing_only {
            let snapshot = self.load_snapshot_for_request(request)?;
            ProductionEvidenceTemplateScope::missing_only(&snapshot)?
        } else {
            ProductionEvidenceTemplateScope::full()
        };

        RuntimeHttpResponse::json(
            200,
            production_evidence_template_value(&project_slug, output_root, source, scope),
        )
    }

    fn production_evidence_item_template_response(
        &self,
        request: &RuntimeHttpRequest,
    ) -> Result<RuntimeHttpResponse> {
        let project_slug = request
            .query
            .get("project")
            .or_else(|| request.query.get("project_slug"))
            .cloned()
            .or_else(|| self.config.project_slug.clone())
            .unwrap_or_else(|| "demo".to_string());
        if project_slug == "*" {
            return RuntimeHttpResponse::json(
                400,
                json!({
                    "error": "production_evidence_item_template_requires_project",
                    "expected": "Use a concrete project slug, not *",
                }),
            );
        }
        let output_root = request
            .query
            .get("output_root")
            .or_else(|| request.query.get("output-root"))
            .map(String::as_str)
            .filter(|value| !value.trim().is_empty());
        let source = request
            .query
            .get("source")
            .map(String::as_str)
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("replace-with-production-evidence-source");
        let (kind, target_id, task_id) = match production_evidence_item_selector(request) {
            Ok(selector) => selector,
            Err(error) => {
                return RuntimeHttpResponse::json(
                    400,
                    json!({
                        "error": "invalid_production_evidence_item_template_request",
                        "message": error.to_string(),
                    }),
                );
            }
        };

        RuntimeHttpResponse::json(
            200,
            production_evidence_item_template_value(
                &project_slug,
                output_root,
                source,
                &kind,
                &target_id,
                task_id.as_deref(),
            )?,
        )
    }

    fn production_evidence_item_from_ledger_response(
        &self,
        request: &RuntimeHttpRequest,
    ) -> Result<RuntimeHttpResponse> {
        let project_slug = request
            .query
            .get("project")
            .or_else(|| request.query.get("project_slug"))
            .cloned()
            .or_else(|| self.config.project_slug.clone())
            .unwrap_or_else(|| "demo".to_string());
        if project_slug == "*" {
            return RuntimeHttpResponse::json(
                400,
                json!({
                    "error": "production_evidence_item_from_ledger_requires_project",
                    "expected": "Use a concrete project slug, not *",
                }),
            );
        }
        let source = request
            .query
            .get("source")
            .map(String::as_str)
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("runtime-ledger");
        let snapshot = self.load_snapshot_for_request(request)?;
        let derived = if let Some(provider_request_id) = request
            .query
            .get("provider_request_id")
            .or_else(|| request.query.get("provider-request-id"))
            .map(String::as_str)
            .filter(|value| !value.trim().is_empty())
        {
            production_evidence_item_from_provider_ledger(
                &project_slug,
                source,
                &snapshot,
                provider_request_id,
            )
        } else if let Some(desktop_vision_action_id) = request
            .query
            .get("desktop_vision_action_id")
            .or_else(|| request.query.get("desktop-vision-action-id"))
            .map(String::as_str)
            .filter(|value| !value.trim().is_empty())
        {
            production_evidence_item_from_desktop_vision_ledger(
                &project_slug,
                source,
                &snapshot,
                desktop_vision_action_id,
            )
        } else if let Some(software_action_id) = request
            .query
            .get("software_action_id")
            .or_else(|| request.query.get("software-action-id"))
            .map(String::as_str)
            .filter(|value| !value.trim().is_empty())
        {
            production_evidence_item_from_software_ledger(
                &project_slug,
                source,
                &snapshot,
                software_action_id,
            )
        } else {
            return RuntimeHttpResponse::json(
                400,
                json!({
                    "error": "missing_production_evidence_ledger_id",
                    "expected": "provider_request_id, software_action_id, or desktop_vision_action_id query parameter",
                }),
            );
        };

        match derived {
            Ok(value) => RuntimeHttpResponse::json(200, value),
            Err(error) => RuntimeHttpResponse::json(
                404,
                json!({
                    "error": "production_evidence_ledger_item_not_found",
                    "message": error.to_string(),
                }),
            ),
        }
    }

    fn production_evidence_bundle_from_ledger_response(
        &self,
        request: &RuntimeHttpRequest,
    ) -> Result<RuntimeHttpResponse> {
        let project_slug = request
            .query
            .get("project")
            .or_else(|| request.query.get("project_slug"))
            .cloned()
            .or_else(|| self.config.project_slug.clone())
            .unwrap_or_else(|| "demo".to_string());
        if project_slug == "*" {
            return RuntimeHttpResponse::json(
                400,
                json!({
                    "error": "production_evidence_bundle_from_ledger_requires_project",
                    "expected": "Use a concrete project slug, not *",
                }),
            );
        }
        let source = request
            .query
            .get("source")
            .map(String::as_str)
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("runtime-ledger");
        let include_incomplete = request
            .query
            .get("include_incomplete")
            .or_else(|| request.query.get("include-incomplete"))
            .is_some_and(|value| query_bool(value));
        let snapshot = self.load_snapshot_for_request(request)?;

        RuntimeHttpResponse::json(
            200,
            production_evidence_bundle_from_ledger_value(
                &project_slug,
                source,
                &snapshot,
                include_incomplete,
            )?,
        )
    }

    fn production_evidence_response(&self, body: &str) -> Result<RuntimeHttpResponse> {
        let request = match serde_json::from_str::<ImportProductionEvidenceRequest>(body) {
            Ok(request) => request,
            Err(error) => {
                return RuntimeHttpResponse::json(
                    400,
                    json!({
                        "error": "invalid_production_evidence_request",
                        "message": error.to_string(),
                    }),
                );
            }
        };
        let project_slug = request
            .project_slug
            .clone()
            .or_else(|| self.config.project_slug.clone())
            .unwrap_or_else(|| "demo".to_string());
        let source = request
            .source
            .clone()
            .unwrap_or_else(|| "production_evidence_bundle".to_string());
        let (provider_count, software_count, desktop_count) =
            match validate_production_evidence_bundle(&request) {
                Ok(counts) => counts,
                Err(error) => {
                    return RuntimeHttpResponse::json(
                        400,
                        json!({
                            "error": "invalid_production_evidence_item",
                            "message": error.to_string(),
                        }),
                    );
                }
            };

        if provider_count == 0 && software_count == 0 && desktop_count == 0 {
            return RuntimeHttpResponse::json(
                400,
                json!({
                    "error": "empty_production_evidence_bundle",
                    "expected": "providers, software_actions, or desktop_vision evidence arrays",
                }),
            );
        }

        let artifact_file_report = production_evidence_artifact_file_report(&request);
        if !artifact_file_report["complete"].as_bool().unwrap_or(false) {
            return RuntimeHttpResponse::json(
                400,
                json!({
                    "error": "missing_production_artifact_files",
                    "message": "provider artifacts/metadata, software artifacts, and desktop vision trace/artifact files must exist locally before production evidence import",
                    "writes": 0,
                    "artifact_files": artifact_file_report,
                }),
            );
        }

        let coverage = production_evidence_coverage(&request);
        let imported_at = chrono::Utc::now().to_rfc3339();
        let repository = RuntimeRepository::open(&self.config.db_path)?;
        repository.migrate()?;

        let mut provider_reports = Vec::with_capacity(provider_count);
        for item in request.providers.unwrap_or_default() {
            let report = match self.import_provider_production_evidence(
                &repository,
                &project_slug,
                &source,
                &imported_at,
                item,
            ) {
                Ok(report) => report,
                Err(error) => {
                    return RuntimeHttpResponse::json(
                        400,
                        json!({
                            "error": "invalid_production_evidence_item",
                            "message": error.to_string(),
                        }),
                    );
                }
            };
            provider_reports.push(report);
        }

        let mut software_reports = Vec::with_capacity(software_count);
        for item in request.software_actions.unwrap_or_default() {
            let report = match self.import_software_production_evidence(
                &repository,
                &project_slug,
                &source,
                &imported_at,
                item,
            ) {
                Ok(report) => report,
                Err(error) => {
                    return RuntimeHttpResponse::json(
                        400,
                        json!({
                            "error": "invalid_production_evidence_item",
                            "message": error.to_string(),
                        }),
                    );
                }
            };
            software_reports.push(report);
        }

        let mut desktop_reports = Vec::with_capacity(desktop_count);
        for item in request.desktop_vision.unwrap_or_default() {
            let report = match self.import_desktop_vision_production_evidence(
                &repository,
                &project_slug,
                &source,
                &imported_at,
                item,
            ) {
                Ok(report) => report,
                Err(error) => {
                    return RuntimeHttpResponse::json(
                        400,
                        json!({
                            "error": "invalid_production_evidence_item",
                            "message": error.to_string(),
                        }),
                    );
                }
            };
            desktop_reports.push(report);
        }

        repository.insert_event(&RuntimeEvent::new(
            project_slug.clone(),
            RuntimeEventLevel::Ok,
            format!(
                "imported production evidence bundle: providers={}, software_actions={}, desktop_vision={}",
                provider_reports.len(),
                software_reports.len(),
                desktop_reports.len()
            ),
        ))?;
        let snapshot = repository.snapshot(self.config.project_slug.as_deref())?;
        let prd_readiness = runtime_prd_readiness_resource(&snapshot)?;

        RuntimeHttpResponse::json(
            201,
            json!({
                "kind": "pool_production_evidence_import",
                "project_slug": project_slug,
                "source": source,
                "imported_at": imported_at,
                "summary": {
                    "providers": provider_reports.len(),
                    "software_actions": software_reports.len(),
                    "desktop_vision": desktop_reports.len(),
                },
                "artifact_files": artifact_file_report,
                "coverage": coverage,
                "providers": provider_reports,
                "software_actions": software_reports,
                "desktop_vision": desktop_reports,
                "prd_readiness": prd_readiness,
                "snapshot": snapshot,
            }),
        )
    }

    fn production_evidence_item_response(&self, body: &str) -> Result<RuntimeHttpResponse> {
        let request = match serde_json::from_str::<SubmitProductionEvidenceItemRequest>(body) {
            Ok(request) => request,
            Err(error) => {
                return RuntimeHttpResponse::json(
                    400,
                    json!({
                        "error": "invalid_production_evidence_item_request",
                        "message": error.to_string(),
                    }),
                );
            }
        };
        let request = match request.into_import_request() {
            Ok(request) => request,
            Err(error) => {
                return RuntimeHttpResponse::json(
                    400,
                    json!({
                        "error": "invalid_production_evidence_item_request",
                        "message": error.to_string(),
                    }),
                );
            }
        };
        let body = serde_json::to_string(&request)?;
        self.production_evidence_response(&body)
    }

    fn production_evidence_item_validate_response(
        &self,
        body: &str,
    ) -> Result<RuntimeHttpResponse> {
        let item = match serde_json::from_str::<SubmitProductionEvidenceItemRequest>(body) {
            Ok(request) => request,
            Err(error) => {
                return RuntimeHttpResponse::json(
                    400,
                    json!({
                        "error": "invalid_production_evidence_item_request",
                        "message": error.to_string(),
                        "writes": 0,
                    }),
                );
            }
        };
        let request = match item.into_import_request() {
            Ok(request) => request,
            Err(error) => {
                return RuntimeHttpResponse::json(
                    400,
                    json!({
                        "error": "invalid_production_evidence_item_request",
                        "message": error.to_string(),
                        "writes": 0,
                    }),
                );
            }
        };
        let project_slug = self.production_evidence_project_slug(&request);
        let source = request
            .source
            .clone()
            .unwrap_or_else(|| "production_evidence_item".to_string());
        let validation =
            match production_evidence_validation_value(&request, &project_slug, &source) {
                Ok(value) => value,
                Err(value) => return RuntimeHttpResponse::json(400, value),
            };

        RuntimeHttpResponse::json(
            200,
            json!({
                "kind": "pool_production_evidence_item_validation",
                "valid": true,
                "writes": 0,
                "project_slug": project_slug,
                "source": source,
                "validation": validation,
                "commands": {
                    "submit": format!("pool-cli --project {project_slug} submit-production-evidence-item <item.json>"),
                    "validate_bundle": format!("pool-cli --project {project_slug} validate-production-evidence <bundle.json>"),
                    "readiness": format!("pool-cli --project {project_slug} prd-readiness"),
                },
                "http": {
                    "submit": "POST /api/production-evidence/items",
                    "validate_item": "POST /api/production-evidence/items/validate",
                    "validate_bundle": "POST /api/production-evidence/validate",
                },
                "mcp": {
                    "validate_item_tool": "pool_validate_production_evidence_item",
                    "submit_tool": "pool_submit_production_evidence_item",
                }
            }),
        )
    }

    fn production_evidence_validate_response(&self, body: &str) -> Result<RuntimeHttpResponse> {
        let request = match serde_json::from_str::<ImportProductionEvidenceRequest>(body) {
            Ok(request) => request,
            Err(error) => {
                return RuntimeHttpResponse::json(
                    400,
                    json!({
                        "error": "invalid_production_evidence_request",
                        "message": error.to_string(),
                    }),
                );
            }
        };
        let project_slug = self.production_evidence_project_slug(&request);
        let source = request
            .source
            .clone()
            .unwrap_or_else(|| "production_evidence_bundle".to_string());
        let value = match production_evidence_validation_value(&request, &project_slug, &source) {
            Ok(value) => value,
            Err(value) => return RuntimeHttpResponse::json(400, value),
        };

        RuntimeHttpResponse::json(200, value)
    }

    fn production_evidence_project_slug(
        &self,
        request: &ImportProductionEvidenceRequest,
    ) -> String {
        request
            .project_slug
            .clone()
            .or_else(|| self.config.project_slug.clone())
            .unwrap_or_else(|| "demo".to_string())
    }

    fn production_evidence_merge_response(&self, body: &str) -> Result<RuntimeHttpResponse> {
        let request = match serde_json::from_str::<MergeProductionEvidenceRequest>(body) {
            Ok(request) => request,
            Err(error) => {
                return RuntimeHttpResponse::json(
                    400,
                    json!({
                        "error": "invalid_production_evidence_merge_request",
                        "message": error.to_string(),
                        "writes": 0,
                    }),
                );
            }
        };
        if request.bundles.is_empty() {
            return RuntimeHttpResponse::json(
                400,
                json!({
                    "error": "production_evidence_merge_requires_bundles",
                    "message": "production evidence merge requires at least one bundle",
                    "writes": 0,
                }),
            );
        }
        let project_slug = request
            .project_slug
            .clone()
            .or_else(|| self.config.project_slug.clone())
            .map(|slug| slug.trim().to_string())
            .filter(|slug| !slug.is_empty() && slug != "*")
            .unwrap_or_else(|| "demo".to_string());
        let response = match merge_production_evidence_requests(
            &project_slug,
            request.source.as_deref(),
            request.bundles,
        ) {
            Ok(response) => response,
            Err(error) => {
                return RuntimeHttpResponse::json(
                    400,
                    json!({
                        "error": "invalid_production_evidence_merge_request",
                        "message": error.to_string(),
                        "writes": 0,
                    }),
                );
            }
        };
        RuntimeHttpResponse::json(200, response)
    }

    fn production_evidence_closeout_response(&self, body: &str) -> Result<RuntimeHttpResponse> {
        let request = match serde_json::from_str::<CloseoutProductionEvidenceRequest>(body) {
            Ok(request) => request,
            Err(error) => {
                return RuntimeHttpResponse::json(
                    400,
                    json!({
                        "error": "invalid_production_evidence_closeout_request",
                        "message": error.to_string(),
                        "writes": 0,
                    }),
                );
            }
        };
        if request.bundles.is_empty() {
            return RuntimeHttpResponse::json(
                400,
                json!({
                    "error": "production_evidence_closeout_requires_bundles",
                    "message": "production evidence closeout requires at least one bundle",
                    "writes": 0,
                }),
            );
        }
        let project_slug = request
            .project_slug
            .clone()
            .or_else(|| self.config.project_slug.clone())
            .map(|slug| slug.trim().to_string())
            .filter(|slug| !slug.is_empty() && slug != "*")
            .unwrap_or_else(|| "demo".to_string());
        let source = request
            .source
            .clone()
            .map(|source| source.trim().to_string())
            .filter(|source| !source.is_empty());
        let merge_response = match merge_production_evidence_requests(
            &project_slug,
            source.as_deref(),
            request.bundles,
        ) {
            Ok(response) => response,
            Err(error) => {
                return RuntimeHttpResponse::json(
                    400,
                    json!({
                        "error": "invalid_production_evidence_closeout_request",
                        "message": error.to_string(),
                        "writes": 0,
                    }),
                );
            }
        };
        let merged_request = match serde_json::from_value::<ImportProductionEvidenceRequest>(
            merge_response["bundle"].clone(),
        ) {
            Ok(request) => request,
            Err(error) => {
                return RuntimeHttpResponse::json(
                    400,
                    json!({
                        "error": "invalid_production_evidence_closeout_request",
                        "message": error.to_string(),
                        "writes": 0,
                    }),
                );
            }
        };
        let (provider_count, software_count, desktop_count) =
            match validate_production_evidence_bundle(&merged_request) {
                Ok(counts) => counts,
                Err(error) => {
                    return RuntimeHttpResponse::json(
                        400,
                        json!({
                            "error": "invalid_production_evidence_closeout_item",
                            "message": error.to_string(),
                            "writes": 0,
                        }),
                    );
                }
            };

        if provider_count == 0 && software_count == 0 && desktop_count == 0 {
            return RuntimeHttpResponse::json(
                400,
                json!({
                    "error": "empty_production_evidence_closeout_bundle",
                    "expected": "providers, software_actions, or desktop_vision evidence arrays",
                    "writes": 0,
                }),
            );
        }

        let coverage = production_evidence_coverage(&merged_request);
        let artifact_files = production_evidence_artifact_file_report(&merged_request);
        let ready_for_import = coverage["complete"].as_bool().unwrap_or(false)
            && artifact_files["complete"].as_bool().unwrap_or(false);
        let validation = json!({
            "kind": "pool_production_evidence_validation",
            "valid": true,
            "writes": 0,
            "project_slug": project_slug.clone(),
            "source": merged_request.source.clone(),
            "summary": {
                "providers": provider_count,
                "software_actions": software_count,
                "desktop_vision": desktop_count,
            },
            "coverage": coverage,
            "artifact_files": artifact_files,
            "providers": provider_production_evidence_validation_rows(&merged_request),
            "software_actions": software_production_evidence_validation_rows(&merged_request),
            "desktop_vision": desktop_vision_production_evidence_validation_rows(&merged_request),
        });

        if !request.import.unwrap_or(false) {
            return RuntimeHttpResponse::json(
                200,
                json!({
                    "kind": "pool_production_evidence_closeout",
                    "project_slug": project_slug.clone(),
                    "source": merged_request.source.clone(),
                    "mode": "validate",
                    "writes": 0,
                    "ready_for_import": ready_for_import,
                    "merge": merge_response,
                    "validation": validation,
                    "commands": {
                        "closeout": format!("pool-cli --project {project_slug} closeout-production-evidence --output <merged-bundle.json> <bundle-a.json> <bundle-b.json>..."),
                        "validate": format!("pool-cli --project {project_slug} validate-production-evidence <merged-bundle.json>"),
                        "import": format!("pool-cli --project {project_slug} import-production-evidence <merged-bundle.json>"),
                        "readiness": format!("pool-cli --project {project_slug} prd-readiness"),
                        "completion_gate": format!("pool-cli --project {project_slug} prd-completion-gate --require-complete"),
                        "completion_package": format!("pool-cli --project {project_slug} prd-completion-package --output-dir worlds/{project_slug}/output --include-snapshot"),
                    },
                }),
            );
        }

        let import_body = serde_json::to_string(&merged_request)?;
        let import_response = self.production_evidence_response(&import_body)?;
        let import_status_code = import_response.status_code;
        let import_body = import_response.body;
        let import_value = serde_json::from_str::<Value>(&import_body)
            .unwrap_or_else(|_| json!({ "body": import_body }));
        if import_status_code >= 400 {
            return RuntimeHttpResponse::json(
                import_status_code,
                json!({
                    "kind": "pool_production_evidence_closeout",
                    "project_slug": project_slug.clone(),
                    "source": merged_request.source.clone(),
                    "mode": "import",
                    "writes": 0,
                    "ready_for_import": ready_for_import,
                    "merge": merge_response,
                    "validation": validation,
                    "import": import_value,
                    "error": "production_evidence_closeout_import_failed",
                }),
            );
        }
        let imported_writes = import_value
            .get("summary")
            .map(production_evidence_count_summary_total)
            .unwrap_or(0);
        let completion_gate = import_value
            .pointer("/prd_readiness/completion_gate")
            .cloned()
            .unwrap_or_else(|| json!({}));
        let ready_for_completion = completion_gate
            .get("ready_for_completion")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let prd_overall_status = import_value
            .pointer("/prd_readiness/overall_status")
            .cloned()
            .unwrap_or_else(|| json!("unknown"));
        let prd_summary = import_value
            .pointer("/prd_readiness/summary")
            .cloned()
            .unwrap_or_else(|| json!({}));
        let completion_package = if let Some(package_request) = request.completion_package.as_ref()
        {
            if ready_for_completion {
                let package_body = json!({
                    "project_slug": project_slug.clone(),
                    "node_id": package_request.node_id.clone(),
                    "title": package_request.title.clone().unwrap_or_else(|| "Pool PRD completion package".to_string()),
                    "output_dir": package_request.output_dir.clone(),
                    "source": package_request.source.clone().unwrap_or_else(|| "production-evidence-closeout".to_string()),
                    "include_snapshot": package_request.include_snapshot.unwrap_or(true),
                });
                let response = self.prd_completion_package_response(&package_body.to_string())?;
                let status_code = response.status_code;
                let body = response.body;
                let value = serde_json::from_str::<Value>(&body)
                    .unwrap_or_else(|_| json!({ "body": body }));
                json!({
                    "requested": true,
                    "written": status_code < 400,
                    "status_code": status_code,
                    "response": value,
                })
            } else {
                json!({
                    "requested": true,
                    "written": false,
                    "skipped": true,
                    "reason": "completion_gate_incomplete",
                    "completion_gate": completion_gate,
                })
            }
        } else {
            json!({
                "requested": false,
                "written": false,
            })
        };

        RuntimeHttpResponse::json(
            201,
            json!({
                "kind": "pool_production_evidence_closeout",
                "project_slug": project_slug.clone(),
                "source": merged_request.source,
                "mode": "import",
                "writes": imported_writes,
                "ready_for_import": ready_for_import,
                "ready_for_completion": ready_for_completion,
                "prd_overall_status": prd_overall_status,
                "prd_summary": prd_summary,
                "completion_gate": completion_gate,
                "completion_package": completion_package,
                "merge": merge_response,
                "validation": validation,
                "import": import_value,
                "commands": {
                    "closeout": format!("pool-cli --project {project_slug} closeout-production-evidence --output <merged-bundle.json> <bundle-a.json> <bundle-b.json>..."),
                    "validate": format!("pool-cli --project {project_slug} validate-production-evidence <merged-bundle.json>"),
                    "import": format!("pool-cli --project {project_slug} closeout-production-evidence --import <merged-bundle.json>"),
                    "readiness": format!("pool-cli --project {project_slug} prd-readiness"),
                    "completion_gate": format!("pool-cli --project {project_slug} prd-completion-gate --require-complete"),
                    "completion_package": format!("pool-cli --project {project_slug} prd-completion-package --output-dir worlds/{project_slug}/output --include-snapshot"),
                },
            }),
        )
    }

    fn import_provider_production_evidence(
        &self,
        repository: &RuntimeRepository,
        project_slug: &str,
        source: &str,
        imported_at: &str,
        item: ProviderProductionEvidenceItem,
    ) -> Result<Value> {
        let input_provider_id = required_non_empty(item.provider_id, "providers[].provider_id")?;
        let provider_id = canonical_provider_id(&input_provider_id.trim().to_ascii_lowercase());
        let external_job_id =
            required_production_identifier(item.external_job_id, "providers[].external_job_id")?;
        let production_attestation = required_provider_production_attestation(
            item.production_attestation.as_deref(),
            item.evidence_json.as_ref(),
            "providers[].production_attestation",
        )?;
        let artifacts = required_local_artifact_vec(item.artifacts, "providers[].artifacts")?;
        let mut task = RuntimeTask::new(
            project_slug.to_string(),
            item.task_title
                .unwrap_or_else(|| format!("{provider_id} production evidence import")),
        );
        task.node_id = item.node_id.clone();
        task.provider_id = Some(provider_id.clone());
        task.status = TaskStatus::Succeeded;
        repository.insert_task(&task)?;

        let mut evidence = json!({
            "source": source,
            "evidence_mode": "production_upstream",
            "production_upstream": true,
            "local_mock_gateway": false,
            "input_provider_id": input_provider_id,
            "provider_id": provider_id,
            "external_job_id": external_job_id,
            "family": item.family,
            "production_attestation": production_attestation,
            "imported_at": imported_at,
        });
        if let Some(extra) = item.evidence_json {
            merge_json_object(&mut evidence, extra);
        }
        json_object_insert(&mut evidence, "production_upstream", json!(true));
        json_object_insert(&mut evidence, "local_mock_gateway", json!(false));
        json_object_insert(
            &mut evidence,
            "production_attestation",
            json!(production_attestation),
        );

        let request_json = json!({
            "provider_id": provider_id,
            "execution_mode": "gateway",
            "endpoint": item.endpoint,
            "artifacts": artifacts,
            "evidence": evidence,
            "imported_at": imported_at,
            "provider_request": {
                "source": "production_evidence_import",
                "input_provider_id": input_provider_id,
            }
        });
        let record = repository.insert_provider_request(
            &task.id,
            &provider_id,
            &request_json,
            item.metadata_path.as_deref(),
        )?;
        let mut response_json = json!({
            "status": "Succeeded",
            "ok": true,
            "evidence_import": true,
            "external_job_id": external_job_id,
            "artifacts": artifacts,
            "imported_at": imported_at,
        });
        if let Some(extra) = item.response_json {
            merge_json_object(&mut response_json, extra);
        }
        json_object_insert(&mut response_json, "status", json!("Succeeded"));
        json_object_insert(&mut response_json, "ok", json!(true));
        repository.update_provider_request_response(
            &record.id,
            &response_json,
            item.metadata_path.as_deref(),
        )?;
        repository.insert_event(&RuntimeEvent::new(
            project_slug.to_string(),
            RuntimeEventLevel::Ok,
            format!("imported provider production evidence: {provider_id}"),
        ))?;

        Ok(json!({
            "task_id": task.id,
            "provider_request_id": record.id,
            "provider_id": provider_id,
            "input_provider_id": input_provider_id,
            "external_job_id": external_job_id,
            "artifacts": artifacts,
        }))
    }

    fn import_software_production_evidence(
        &self,
        repository: &RuntimeRepository,
        project_slug: &str,
        source: &str,
        imported_at: &str,
        item: SoftwareProductionEvidenceItem,
    ) -> Result<Value> {
        let input_adapter_id =
            required_non_empty(item.adapter_id, "software_actions[].adapter_id")?;
        let adapter_id = canonical_production_software_adapter_id(&input_adapter_id);
        let external_action_id = required_production_identifier(
            item.external_action_id,
            "software_actions[].external_action_id",
        )?;
        let production_attestation = required_production_attestation(
            item.production_attestation.as_deref(),
            item.evidence_json.as_ref(),
            "software_actions[].production_attestation",
            "a real software plugin, API, CLI, MCP, or desktop-control run",
        )?;
        let artifacts = item.artifacts.unwrap_or_default();
        if artifacts.is_empty() && item.verification_json.is_none() {
            bail!("software_actions[] requires artifacts or verification_json");
        }
        let action_kind = item.action_kind.unwrap_or(SoftwareActionKind::HealthCheck);
        let priority = item.priority.unwrap_or(ControlPriority::ApiMcp);
        let mut task = RuntimeTask::new(
            project_slug.to_string(),
            item.task_title
                .unwrap_or_else(|| format!("{adapter_id} production software evidence import")),
        );
        task.node_id = item.node_id.clone();
        task.provider_id = Some(adapter_id.clone());
        task.status = TaskStatus::Succeeded;
        repository.insert_task(&task)?;

        let mut evidence = json!({
            "source": source,
            "evidence_mode": "production_software",
            "production_software": true,
            "local_mock_software": false,
            "input_adapter_id": input_adapter_id,
            "adapter_id": adapter_id,
            "control_profile": item.control_profile.unwrap_or_else(|| software_control_profile_name(&priority)),
            "external_action_id": external_action_id,
            "production_attestation": production_attestation,
            "imported_at": imported_at,
        });
        if let Some(extra) = item.evidence_json {
            merge_json_object(&mut evidence, extra);
        }
        json_object_insert(&mut evidence, "production_software", json!(true));
        json_object_insert(&mut evidence, "local_mock_software", json!(false));
        json_object_insert(
            &mut evidence,
            "production_attestation",
            json!(production_attestation),
        );

        let action = SoftwareControlAction {
            adapter_id: adapter_id.clone(),
            action_kind: action_kind.clone(),
            priority: priority.clone(),
            payload_json: json!({
                "evidence": evidence,
                "artifacts": artifacts,
                "external_action_id": external_action_id,
                "production_attestation": production_attestation,
                "imported_at": imported_at,
            }),
            requires_confirmation: false,
        };
        let action_id = Uuid::new_v4().to_string();
        let result = SoftwareActionResult {
            adapter_id: adapter_id.clone(),
            action_kind,
            priority,
            ok: true,
            message: "production software evidence imported".to_string(),
            artifacts: artifacts.clone(),
        };
        repository.insert_software_action(&action_id, Some(&task.id), &action, Some(&result))?;
        if let Some(extra_verification) = item.verification_json {
            let mut verification = json!(result);
            merge_json_object(&mut verification, extra_verification);
            json_object_insert(&mut verification, "ok", json!(true));
            repository.update_software_action_verification(&action_id, verification)?;
        }
        repository.insert_event(&RuntimeEvent::new(
            project_slug.to_string(),
            RuntimeEventLevel::Ok,
            format!("imported software production evidence: {adapter_id}"),
        ))?;

        Ok(json!({
            "task_id": task.id,
            "software_action_id": action_id,
            "adapter_id": adapter_id,
            "input_adapter_id": input_adapter_id,
            "external_action_id": external_action_id,
            "artifacts": artifacts,
        }))
    }

    fn import_desktop_vision_production_evidence(
        &self,
        repository: &RuntimeRepository,
        project_slug: &str,
        source: &str,
        imported_at: &str,
        item: DesktopVisionProductionEvidenceItem,
    ) -> Result<Value> {
        let adapter_id = item
            .adapter_id
            .unwrap_or_else(|| "touchdesigner".to_string());
        let external_action_id = required_production_identifier(
            item.external_action_id,
            "desktop_vision[].external_action_id",
        )?;
        let controller_id =
            required_production_identifier(item.controller_id, "desktop_vision[].controller_id")?;
        let production_attestation = required_production_attestation(
            item.production_attestation.as_deref(),
            item.evidence_json.as_ref(),
            "desktop_vision[].production_attestation",
            "a real external visual/OCR/screen model controller run",
        )?;
        let trace_path =
            required_local_artifact_path(item.trace_path, "desktop_vision[].trace_path")?;
        let mut artifacts = item.artifacts.unwrap_or_default();
        if !artifacts.iter().any(|artifact| artifact == &trace_path) {
            artifacts.push(trace_path.clone());
        }
        let mut task = RuntimeTask::new(
            project_slug.to_string(),
            item.task_title
                .unwrap_or_else(|| format!("{adapter_id} external vision evidence import")),
        );
        task.node_id = item.node_id.clone();
        task.provider_id = Some(adapter_id.clone());
        task.status = TaskStatus::Succeeded;
        repository.insert_task(&task)?;

        let mut evidence = json!({
            "source": source,
            "control_profile": "desktop_recognition",
            "external_visual_model": true,
            "local_trace_smoke": false,
            "external_action_id": external_action_id,
            "controller_id": controller_id,
            "production_attestation": production_attestation,
            "vision_trace_path": trace_path,
            "visual_model": item.visual_model.clone().unwrap_or_else(|| "external".to_string()),
            "imported_at": imported_at,
        });
        if let Some(extra) = item.evidence_json {
            merge_json_object(&mut evidence, extra);
        }
        json_object_insert(&mut evidence, "external_visual_model", json!(true));
        json_object_insert(&mut evidence, "local_trace_smoke", json!(false));
        json_object_insert(
            &mut evidence,
            "production_attestation",
            json!(production_attestation),
        );

        let action = SoftwareControlAction {
            adapter_id: adapter_id.clone(),
            action_kind: SoftwareActionKind::RunViewport,
            priority: ControlPriority::DesktopRecognition,
            payload_json: json!({
                "evidence": evidence,
                "artifacts": artifacts,
                "external_action_id": external_action_id,
                "controller_id": controller_id,
                "production_attestation": production_attestation,
                "screen_trace_path": trace_path,
                "imported_at": imported_at,
            }),
            requires_confirmation: false,
        };
        let action_id = Uuid::new_v4().to_string();
        repository.insert_software_action(&action_id, Some(&task.id), &action, None)?;
        let mut verification = json!({
            "ok": true,
            "status": "succeeded",
            "desktop_recognition_status": "succeeded",
            "external_visual_model": true,
            "screen_trace_path": trace_path,
            "artifacts": artifacts,
            "controller_result": {
                "controller": controller_id,
                "external_action_id": external_action_id,
                "production_attestation": production_attestation,
                "external_visual_model": true,
                "visual_model": item.visual_model.unwrap_or_else(|| "external".to_string()),
                "vision_trace_path": trace_path,
                "imported_at": imported_at,
            },
            "updated_at": imported_at,
        });
        if let Some(extra_verification) = item.verification_json {
            merge_json_object(&mut verification, extra_verification);
        }
        json_object_insert(&mut verification, "ok", json!(true));
        json_object_insert(
            &mut verification,
            "desktop_recognition_status",
            json!("succeeded"),
        );
        json_object_insert(&mut verification, "external_visual_model", json!(true));
        repository.update_software_action_verification(&action_id, verification)?;
        repository.insert_event(&RuntimeEvent::new(
            project_slug.to_string(),
            RuntimeEventLevel::Ok,
            format!("imported desktop vision production evidence: {adapter_id}"),
        ))?;

        Ok(json!({
            "task_id": task.id,
            "software_action_id": action_id,
            "adapter_id": adapter_id,
            "external_action_id": external_action_id,
            "controller_id": controller_id,
            "trace_path": trace_path,
        }))
    }

    fn dispatch_software_action_response(
        &self,
        repository: &RuntimeRepository,
        project_slug: &str,
        mut task: RuntimeTask,
        action: SoftwareControlAction,
    ) -> Result<RuntimeHttpResponse> {
        if action.adapter_id == "unreal" {
            let runner = SoftwareActionRunner::new(repository);
            let adapter = UnrealMcpAdapter::from_action(&action);
            let report = if adapter.is_configured() {
                runner.run(&adapter, task, action)?
            } else {
                runner.run(&MockUnrealAdapter::new(), task, action)?
            };
            let task = repository.task_snapshot(&report.task_id)?;
            let snapshot = repository.snapshot(self.config.project_slug.as_deref())?;

            return RuntimeHttpResponse::json(
                200,
                json!({
                    "report": report,
                    "task": task,
                    "snapshot": snapshot,
                }),
            );
        }
        if action.adapter_id == "hermes" {
            let runner = SoftwareActionRunner::new(repository);
            let adapter =
                HermesMcpAdapter::from_action(&action, agent_auth_token(repository, "hermes")?);
            if adapter.is_configured() {
                let report = runner.run(&adapter, task, action)?;
                let task = repository.task_snapshot(&report.task_id)?;
                let snapshot = repository.snapshot(self.config.project_slug.as_deref())?;

                return RuntimeHttpResponse::json(
                    200,
                    json!({
                        "report": report,
                        "task": task,
                        "snapshot": snapshot,
                    }),
                );
            }
        }
        if action.priority == ControlPriority::ApiMcp {
            let registry = SoftwareAdapterRegistry::defaults();
            if let Some(config) = registry.get(&action.adapter_id).cloned() {
                let adapter = GenericSoftwareApiAdapter::from_action(config, &action);
                if adapter.is_configured() {
                    let runner = SoftwareActionRunner::new(repository);
                    let report = runner.run(&adapter, task, action)?;
                    let task = repository.task_snapshot(&report.task_id)?;
                    let snapshot = repository.snapshot(self.config.project_slug.as_deref())?;

                    return RuntimeHttpResponse::json(
                        200,
                        json!({
                            "report": report,
                            "task": task,
                            "snapshot": snapshot,
                        }),
                    );
                }
            }
        }
        if action.action_kind == SoftwareActionKind::ExecuteCli {
            let registry = SoftwareAdapterRegistry::defaults();
            if let Some(config) = registry.get(&action.adapter_id).cloned() {
                let runner = SoftwareActionRunner::new(repository);
                let report = runner.run(&CommandSoftwareAdapter::new(config), task, action)?;
                let task = repository.task_snapshot(&report.task_id)?;
                let snapshot = repository.snapshot(self.config.project_slug.as_deref())?;

                return RuntimeHttpResponse::json(
                    200,
                    json!({
                        "report": report,
                        "task": task,
                        "snapshot": snapshot,
                    }),
                );
            }
        }
        if should_run_desktop_recognition(&action) {
            let registry = SoftwareAdapterRegistry::defaults();
            if let Some(config) = registry.get(&action.adapter_id).cloned() {
                let action = action_with_default_control_dir(
                    action,
                    self.default_control_dir(project_slug)
                        .join("desktop-recognition"),
                );
                let runner = SoftwareActionRunner::new(repository);
                let report = runner.run(&DesktopRecognitionAdapter::new(config), task, action)?;
                let task = repository.task_snapshot(&report.task_id)?;
                let snapshot = repository.snapshot(self.config.project_slug.as_deref())?;

                return RuntimeHttpResponse::json(
                    200,
                    json!({
                        "report": report,
                        "task": task,
                        "snapshot": snapshot,
                    }),
                );
            }
        }

        task.status = TaskStatus::WaitingApproval;
        task.requires_approval = true;
        repository.insert_task(&task)?;
        let action_id = Uuid::new_v4().to_string();
        let result = SoftwareActionResult {
            adapter_id: action.adapter_id.clone(),
            action_kind: action.action_kind.clone(),
            priority: action.priority.clone(),
            ok: false,
            message: "software adapter is not executable yet; queued for human takeover"
                .to_string(),
            artifacts: Vec::new(),
        };
        repository.insert_software_action(&action_id, Some(&task.id), &action, Some(&result))?;
        repository.insert_event(&RuntimeEvent::new(
            project_slug.to_string(),
            RuntimeEventLevel::Warn,
            format!(
                "software action queued for human takeover: {} {:?}",
                action.adapter_id, action.action_kind
            ),
        ))?;
        let task = repository.task_snapshot(&task.id)?;
        let snapshot = repository.snapshot(self.config.project_slug.as_deref())?;

        RuntimeHttpResponse::json(
            202,
            json!({
                "report": {
                    "task_id": task.id,
                    "action_id": action_id,
                    "adapter_id": action.adapter_id,
                    "status": task.status,
                    "result": result,
                },
                "task": task,
                "snapshot": snapshot,
            }),
        )
    }

    fn desktop_recognition_requests_response(
        &self,
        request: &RuntimeHttpRequest,
    ) -> Result<RuntimeHttpResponse> {
        let snapshot = self.load_snapshot_for_request(request)?;
        let requests: Vec<Value> = snapshot
            .software_actions
            .iter()
            .filter_map(desktop_recognition_request_value)
            .collect();
        let count = requests.len();

        RuntimeHttpResponse::json(
            200,
            json!({
                "requests": requests,
                "count": count,
                "project_filter": snapshot.project_filter,
                "stats": snapshot.stats,
            }),
        )
    }

    fn desktop_recognition_contract_response(&self) -> Result<RuntimeHttpResponse> {
        RuntimeHttpResponse::json(200, desktop_recognition_contract_resource())
    }

    fn desktop_recognition_run_next_response(
        &self,
        request: &RuntimeHttpRequest,
        body: &str,
    ) -> Result<RuntimeHttpResponse> {
        let run_request = if body.trim().is_empty() {
            DesktopRecognitionRunNextRequest::default()
        } else {
            match serde_json::from_str::<DesktopRecognitionRunNextRequest>(body) {
                Ok(request) => request,
                Err(error) => {
                    return RuntimeHttpResponse::json(
                        400,
                        json!({
                            "error": "invalid_desktop_recognition_run_next_request",
                            "message": error.to_string(),
                        }),
                    );
                }
            }
        };
        let status = run_request
            .status
            .as_deref()
            .unwrap_or("succeeded")
            .to_string();
        if normalize_desktop_recognition_status(&status).is_none() {
            return RuntimeHttpResponse::json(
                400,
                json!({
                    "error": "invalid_desktop_recognition_status",
                    "expected": "succeeded, failed, cancelled, retryable, running, or queued_for_desktop_recognition",
                }),
            );
        }
        let controller_id = run_request
            .controller_id
            .clone()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "pool-runtime-desktop-controller".to_string());
        let snapshot = self.load_snapshot_for_request(request)?;
        let requests = snapshot
            .software_actions
            .iter()
            .filter_map(desktop_recognition_request_value)
            .collect::<Vec<_>>();
        let queued_count = requests.len();
        let mut callbacks = Vec::new();
        let mut skipped = Vec::new();

        for request_value in requests.iter().take(run_request.limit.unwrap_or(1)) {
            let Some(software_action_id) = json_value_string(request_value, "software_action_id")
            else {
                skipped.push(json!({
                    "reason": "missing_software_action_id",
                    "request": request_value,
                }));
                continue;
            };
            let result_body = desktop_run_next_result_body(
                request_value,
                &software_action_id,
                &status,
                &controller_id,
                run_request.message.as_deref(),
                run_request.artifacts.as_deref().unwrap_or(&[]),
                run_request.screen_trace_path.as_deref(),
            );
            let callback_response =
                self.desktop_recognition_result_response(&result_body.to_string())?;
            let response_body = serde_json::from_str::<Value>(&callback_response.body)
                .unwrap_or_else(|_| json!({ "raw": callback_response.body }));
            callbacks.push(json!({
                "software_action_id": software_action_id,
                "status_code": callback_response.status_code,
                "response": response_body,
            }));
        }

        RuntimeHttpResponse::json(
            200,
            json!({
                "controller": controller_id,
                "mode": "dry_run",
                "requested_status": status,
                "queued_count": queued_count,
                "processed_count": callbacks.len(),
                "skipped": skipped,
                "callbacks": callbacks,
            }),
        )
    }

    fn desktop_recognition_result_response(&self, body: &str) -> Result<RuntimeHttpResponse> {
        let request = match serde_json::from_str::<DesktopRecognitionResultRequest>(body) {
            Ok(request) => request,
            Err(error) => {
                return RuntimeHttpResponse::json(
                    400,
                    json!({
                        "error": "invalid_desktop_recognition_result_request",
                        "message": error.to_string(),
                    }),
                );
            }
        };
        let action_id = request
            .software_action_id
            .clone()
            .or_else(|| request.action_id.clone())
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let Some(action_id) = action_id else {
            return RuntimeHttpResponse::json(
                400,
                json!({
                    "error": "missing_software_action_id",
                    "expected": "JSON body with software_action_id or action_id",
                }),
            );
        };
        let status = request
            .status
            .as_deref()
            .and_then(normalize_desktop_recognition_status);
        let Some(status) = status else {
            return RuntimeHttpResponse::json(
                400,
                json!({
                    "error": "invalid_desktop_recognition_status",
                    "expected": "succeeded, failed, cancelled, retryable, running, or queued_for_desktop_recognition",
                }),
            );
        };

        let repository = RuntimeRepository::open(&self.config.db_path)?;
        repository.migrate()?;
        let all_snapshot = repository.snapshot(None)?;
        let Some(action) = all_snapshot
            .software_actions
            .iter()
            .find(|action| action.id == action_id)
            .cloned()
        else {
            return RuntimeHttpResponse::json(
                404,
                json!({
                    "error": "software_action_not_found",
                    "software_action_id": action_id,
                }),
            );
        };

        if !is_desktop_recognition_action(&action) {
            return RuntimeHttpResponse::json(
                409,
                json!({
                    "error": "software_action_is_not_desktop_recognition",
                    "software_action_id": action.id,
                    "adapter_id": action.adapter_id,
                    "action_kind": action.action_kind,
                }),
            );
        }

        let request_task_id = request
            .task_id
            .clone()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        if let (Some(action_task_id), Some(request_task_id)) =
            (action.task_id.as_ref(), request_task_id.as_ref())
        {
            if action_task_id != request_task_id {
                return RuntimeHttpResponse::json(
                    409,
                    json!({
                        "error": "desktop_recognition_task_mismatch",
                        "software_action_id": action.id,
                        "action_task_id": action_task_id,
                        "request_task_id": request_task_id,
                    }),
                );
            }
        }
        let task_id = request_task_id.or_else(|| action.task_id.clone());
        let task_before_update = task_id
            .as_deref()
            .and_then(|task_id| repository.task_snapshot(task_id).ok());
        let project_slug = task_before_update
            .as_ref()
            .map(|task| task.project_slug.clone())
            .or_else(|| self.config.project_slug.clone())
            .unwrap_or_else(|| "demo".to_string());

        let mut verification = action.verification.clone().unwrap_or_else(|| json!({}));
        if !verification.is_object() {
            verification = json!({ "previous_verification": verification });
        }
        if let Some(extra) = request.verification.clone() {
            merge_json_object(&mut verification, extra);
        }
        if let Some(result) = request.result.clone() {
            json_object_insert(&mut verification, "controller_result", result);
        }
        if let Some(message) = request.message.clone() {
            json_object_insert(&mut verification, "message", json!(message));
        }
        if let Some(artifacts) = request.artifacts.clone() {
            json_object_insert(&mut verification, "artifacts", json!(artifacts));
        }
        if let Some(screen_trace_path) = request.screen_trace_path.clone() {
            json_object_insert(
                &mut verification,
                "screen_trace_path",
                json!(screen_trace_path),
            );
        }
        json_object_insert(&mut verification, "status", json!(status));
        json_object_insert(
            &mut verification,
            "desktop_recognition_status",
            json!(status),
        );
        json_object_insert(
            &mut verification,
            "ok",
            json!(status == "succeeded" || status == "running"),
        );
        json_object_insert(
            &mut verification,
            "updated_at",
            json!(chrono::Utc::now().to_rfc3339()),
        );

        repository.update_software_action_verification(&action.id, verification)?;
        let task = if let Some(task_id) = task_id.as_deref() {
            repository.update_task_status(task_id, task_status_for_desktop_recognition(status))?;
            Some(repository.task_snapshot(task_id)?)
        } else {
            None
        };
        repository.insert_event(&RuntimeEvent::new(
            project_slug,
            event_level_for_desktop_recognition(status),
            format!(
                "desktop recognition result: action={} status={}",
                action.id, status
            ),
        ))?;

        let snapshot = repository.snapshot(self.config.project_slug.as_deref())?;
        let updated_action = repository
            .snapshot(None)?
            .software_actions
            .into_iter()
            .find(|action| action.id == action_id);

        RuntimeHttpResponse::json(
            200,
            json!({
                "software_action": updated_action,
                "task": task,
                "snapshot": snapshot,
            }),
        )
    }

    fn software_health_response(&self, body: &str) -> Result<RuntimeHttpResponse> {
        let request = match serde_json::from_str::<CheckSoftwareHealthRequest>(body) {
            Ok(request) => request,
            Err(error) => {
                return RuntimeHttpResponse::json(
                    400,
                    json!({
                        "error": "invalid_software_health_request",
                        "message": error.to_string(),
                    }),
                );
            }
        };

        if request.adapter_id.trim().is_empty() {
            return RuntimeHttpResponse::json(
                400,
                json!({
                    "error": "missing_adapter_id",
                    "expected": "JSON body with adapter_id",
                }),
            );
        }

        let action = SoftwareControlAction {
            adapter_id: request.adapter_id.clone(),
            action_kind: SoftwareActionKind::HealthCheck,
            priority: request.priority.unwrap_or(ControlPriority::ApiMcp),
            payload_json: request.payload_json.unwrap_or_else(|| json!({})),
            requires_confirmation: false,
        };
        self.dispatch_software_health(action)
    }

    fn dispatch_software_health(
        &self,
        action: SoftwareControlAction,
    ) -> Result<RuntimeHttpResponse> {
        if action.adapter_id == "unreal" {
            let adapter = UnrealMcpAdapter::from_action(&action);
            if adapter.is_configured() {
                return self.run_software_health_response(&adapter, "mcp");
            }
            return self.run_software_health_response(&MockUnrealAdapter::new(), "mock");
        }

        if action.adapter_id == "hermes" {
            let repository = RuntimeRepository::open(&self.config.db_path)?;
            repository.migrate()?;
            let adapter =
                HermesMcpAdapter::from_action(&action, agent_auth_token(&repository, "hermes")?);
            return self.run_software_health_response(&adapter, "mcp");
        }

        let registry = SoftwareAdapterRegistry::defaults();
        let Some(config) = registry.get(&action.adapter_id).cloned() else {
            return RuntimeHttpResponse::json(
                501,
                json!({
                    "error": "software_adapter_not_registered",
                    "adapter_id": action.adapter_id,
                    "message": "Runtime HTTP does not have this software adapter in the registry.",
                }),
            );
        };

        if action.priority == ControlPriority::ApiMcp {
            let adapter = GenericSoftwareApiAdapter::from_action(config.clone(), &action);
            if adapter.is_configured() {
                return self.run_software_health_response(&adapter, "api_mcp");
            }
        }

        if action.priority == ControlPriority::DesktopRecognition {
            return self
                .run_software_health_response(&DesktopRecognitionAdapter::new(config), "desktop");
        }

        if action.priority == ControlPriority::HumanTakeover {
            let result = SoftwareActionResult {
                adapter_id: action.adapter_id,
                action_kind: SoftwareActionKind::HealthCheck,
                priority: ControlPriority::HumanTakeover,
                ok: false,
                message: "software adapter requires human takeover before execution".to_string(),
                artifacts: Vec::new(),
            };
            return RuntimeHttpResponse::json(
                200,
                json!({
                    "adapter_id": result.adapter_id,
                    "adapter_mode": "human_takeover",
                    "health": result,
                }),
            );
        }

        self.run_software_health_response(&CommandSoftwareAdapter::new(config), "cli")
    }

    fn run_software_health_response<A>(
        &self,
        adapter: &A,
        adapter_mode: &str,
    ) -> Result<RuntimeHttpResponse>
    where
        A: SoftwareAdapter,
    {
        let config = adapter.config().clone();
        let result = adapter.health()?;
        RuntimeHttpResponse::json(
            200,
            json!({
                "adapter_id": config.id,
                "display_name": config.display_name,
                "control_modes": config.control_modes,
                "priority": config.priority,
                "desktop_fallback": config.desktop_fallback,
                "adapter_mode": adapter_mode,
                "health": result,
            }),
        )
    }

    fn agent_session_response(&self, body: &str) -> Result<RuntimeHttpResponse> {
        let request = match serde_json::from_str::<CreateAgentSessionRequest>(body) {
            Ok(request) => request,
            Err(error) => {
                return RuntimeHttpResponse::json(
                    400,
                    json!({
                        "error": "invalid_agent_session_request",
                        "message": error.to_string(),
                    }),
                );
            }
        };
        let project_slug = request
            .project_slug
            .clone()
            .or_else(|| self.config.project_slug.clone())
            .unwrap_or_else(|| "demo".to_string());
        let control_dir = request
            .control_dir
            .clone()
            .map(PathBuf::from)
            .unwrap_or_else(|| self.default_control_dir(&project_slug));

        let repository = RuntimeRepository::open(&self.config.db_path)?;
        repository.migrate()?;
        let runner = AgentSessionRunner::new(&repository);
        let report = match request.kind {
            AgentSessionRequestKind::Hermes => {
                let instruction = request.instruction.unwrap_or_default();
                if instruction.trim().is_empty() {
                    return RuntimeHttpResponse::json(
                        400,
                        json!({
                            "error": "missing_instruction",
                            "expected": "Hermes request requires instruction",
                        }),
                    );
                }
                let command = HermesCommand {
                    endpoint: request
                        .endpoint
                        .unwrap_or_else(|| "http://127.0.0.1:8787/hermes".to_string()),
                    project_slug: project_slug.clone(),
                    instruction,
                    allowed_tools: request.allowed_tools.unwrap_or_else(default_agent_tools),
                    requires_confirmation: request.requires_confirmation.unwrap_or(false),
                };
                if request.execute.unwrap_or(false) {
                    runner.run_hermes_command(
                        command,
                        &control_dir,
                        HermesExecutionOptions {
                            auth_token: agent_auth_token(&repository, "hermes")?,
                            max_response_bytes: request.max_output_bytes.unwrap_or(16 * 1024),
                            timeout_ms: request.timeout_ms.unwrap_or(30_000),
                        },
                    )?
                } else {
                    runner.stage_hermes_command(command, &control_dir)?
                }
            }
            AgentSessionRequestKind::AgentCli => {
                let command = request.command.unwrap_or_default();
                if command.trim().is_empty() {
                    return RuntimeHttpResponse::json(
                        400,
                        json!({
                            "error": "missing_command",
                            "expected": "Agent CLI request requires command",
                        }),
                    );
                }
                let command = AgentCliCommand {
                    id: request
                        .command_id
                        .unwrap_or_else(|| "agent-cli".to_string()),
                    title: request
                        .title
                        .unwrap_or_else(|| "Agent CLI session".to_string()),
                    command,
                    tools: request.tools.unwrap_or_else(default_agent_tools),
                    token_budget: request.token_budget,
                };
                if request.execute.unwrap_or(false) {
                    runner.run_cli_command(
                        project_slug.clone(),
                        command,
                        &control_dir,
                        AgentCliExecutionOptions {
                            allowed_commands: request
                                .allowed_commands
                                .unwrap_or_else(default_agent_cli_allowlist),
                            working_dir: request.working_dir.map(PathBuf::from),
                            max_output_bytes: request.max_output_bytes.unwrap_or(16 * 1024),
                            timeout_ms: request.timeout_ms.unwrap_or(30_000),
                        },
                    )?
                } else {
                    runner.stage_cli_command(project_slug.clone(), command, &control_dir)?
                }
            }
        };
        let task = repository.task_snapshot(&report.task_id)?;
        let snapshot = repository.snapshot(self.config.project_slug.as_deref())?;

        RuntimeHttpResponse::json(
            201,
            json!({
                "report": report,
                "task": task,
                "snapshot": snapshot,
            }),
        )
    }

    fn agent_conformance_package_response(&self, body: &str) -> Result<RuntimeHttpResponse> {
        let request = match serde_json::from_str::<CreateAgentConformancePackageRequest>(body) {
            Ok(request) => request,
            Err(error) => {
                return RuntimeHttpResponse::json(
                    400,
                    json!({
                        "error": "invalid_agent_conformance_package_request",
                        "message": error.to_string(),
                    }),
                );
            }
        };
        let kind = match normalize_agent_conformance_kind(request.kind.as_deref()) {
            Ok(kind) => kind,
            Err(error) => {
                return RuntimeHttpResponse::json(
                    400,
                    json!({
                        "error": "invalid_agent_conformance_kind",
                        "message": error.to_string(),
                    }),
                );
            }
        };
        let project_slug = request
            .project_slug
            .clone()
            .or_else(|| self.config.project_slug.clone())
            .unwrap_or_else(|| "demo".to_string());
        let output_dir = request.output_dir.unwrap_or_else(|| {
            self.default_output_dir(&project_slug)
                .to_string_lossy()
                .to_string()
        });
        let title = request
            .title
            .filter(|title| !title.trim().is_empty())
            .unwrap_or_else(|| format!("Pool Agent/Hermes conformance package: {kind}"));

        let repository = RuntimeRepository::open(&self.config.db_path)?;
        repository.migrate()?;
        let mut task = RuntimeTask::new(project_slug.clone(), title.clone());
        task.node_id = request.node_id.clone();
        task.provider_id = Some("agent-conformance-package".to_string());
        task.status = TaskStatus::Running;
        task.cost_estimate_tokens = 110;
        repository.insert_task(&task)?;
        repository.insert_event(&RuntimeEvent::new(
            project_slug.clone(),
            RuntimeEventLevel::Info,
            format!("agent conformance package started: {kind}"),
        ))?;

        let package_dir = Path::new(&output_dir)
            .join("control")
            .join("agent-conformance")
            .join(safe_package_segment(&kind));
        let report = write_agent_conformance_package(
            &package_dir,
            &project_slug,
            request.node_id.as_deref(),
            &kind,
            &title,
        )?;
        let local_paths = report["local_paths"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        let assets = repository.index_local_outputs(
            &project_slug,
            request.node_id.as_deref(),
            Some(&format!("pool-agent-conformance://{kind}")),
            &local_paths,
        )?;
        repository.update_task_status(&task.id, TaskStatus::Succeeded)?;
        repository.insert_event(&RuntimeEvent::new(
            project_slug.clone(),
            RuntimeEventLevel::Ok,
            format!(
                "agent conformance package succeeded: {kind}, {} files",
                assets.len()
            ),
        ))?;
        let task = repository.task_snapshot(&task.id)?;
        let snapshot = repository.snapshot(self.config.project_slug.as_deref())?;

        RuntimeHttpResponse::json(
            201,
            json!({
                "report": report,
                "assets": assets,
                "task": task,
                "snapshot": snapshot,
            }),
        )
    }

    fn create_task_response(&self, body: &str) -> Result<RuntimeHttpResponse> {
        let request = match serde_json::from_str::<CreateTaskRequest>(body) {
            Ok(request) => request,
            Err(error) => {
                return RuntimeHttpResponse::json(
                    400,
                    json!({
                        "error": "invalid_task_request",
                        "message": error.to_string(),
                    }),
                );
            }
        };

        if request.title.trim().is_empty() {
            return RuntimeHttpResponse::json(
                400,
                json!({
                    "error": "missing_title",
                    "expected": "JSON body with a non-empty title",
                }),
            );
        }

        let project_slug = request
            .project_slug
            .clone()
            .or_else(|| self.config.project_slug.clone())
            .unwrap_or_else(|| "demo".to_string());
        let mut task = RuntimeTask::new(project_slug.clone(), request.title.trim().to_string());
        task.node_id = request.node_id.clone();
        task.provider_id = request.provider_id.clone();
        task.cost_estimate_tokens = request.cost_estimate_tokens.unwrap_or_default();
        task.requires_approval = request.requires_approval.unwrap_or(false);
        task.status = if task.requires_approval {
            TaskStatus::WaitingApproval
        } else {
            TaskStatus::Ready
        };

        let repository = RuntimeRepository::open(&self.config.db_path)?;
        repository.migrate()?;
        repository.insert_task(&task)?;
        repository.insert_event(&RuntimeEvent::new(
            project_slug,
            RuntimeEventLevel::Info,
            format!("created task: {}", task.title),
        ))?;
        let task = repository.task_snapshot(&task.id)?;
        let snapshot = repository.snapshot(self.config.project_slug.as_deref())?;

        RuntimeHttpResponse::json(
            201,
            json!({
                "task": task,
                "snapshot": snapshot,
            }),
        )
    }

    fn api_keys_response(&self, request: &RuntimeHttpRequest) -> Result<RuntimeHttpResponse> {
        let rotation_days = match api_key_rotation_days_for_request(request) {
            Ok(rotation_days) => rotation_days,
            Err(error) => {
                return RuntimeHttpResponse::json(
                    400,
                    json!({
                        "error": "invalid_api_key_audit_request",
                        "message": error.to_string(),
                    }),
                );
            }
        };
        let repository = RuntimeRepository::open(&self.config.db_path)?;
        repository.migrate()?;
        let api_keys = repository.api_key_snapshots()?;
        let audit = api_key_audit_value(&api_keys, rotation_days);
        RuntimeHttpResponse::json(
            200,
            json!({
                "api_keys": api_keys,
                "audit": audit,
            }),
        )
    }

    fn upsert_api_key_response(&self, body: &str) -> Result<RuntimeHttpResponse> {
        let request = match serde_json::from_str::<CreateApiKeyRequest>(body) {
            Ok(request) => request,
            Err(error) => {
                return RuntimeHttpResponse::json(
                    400,
                    json!({
                        "error": "invalid_api_key_request",
                        "message": error.to_string(),
                    }),
                );
            }
        };
        let provider = request
            .provider_id
            .or(request.provider)
            .map(|provider| canonical_provider_id(&provider))
            .unwrap_or_default();
        let service_type = request
            .service_type
            .unwrap_or_else(|| "provider".to_string());
        if provider.trim().is_empty() {
            return RuntimeHttpResponse::json(
                400,
                json!({
                    "error": "missing_provider_id",
                    "expected": "JSON body with provider_id",
                }),
            );
        }
        if request.api_key.trim().is_empty() {
            return RuntimeHttpResponse::json(
                400,
                json!({
                    "error": "missing_api_key",
                    "expected": "JSON body with non-empty api_key",
                }),
            );
        }

        let repository = RuntimeRepository::open(&self.config.db_path)?;
        repository.migrate()?;
        let api_key = repository.upsert_api_key(
            &provider,
            &service_type,
            &request.api_key,
            request.metadata.unwrap_or_else(|| json!({})),
        )?;
        let project_slug = request
            .project_slug
            .or_else(|| self.config.project_slug.clone())
            .unwrap_or_else(|| "runtime".to_string());
        repository.insert_event(&RuntimeEvent::new(
            project_slug,
            RuntimeEventLevel::Info,
            format!("api key updated: {provider}/{service_type}"),
        ))?;
        let snapshot = repository.snapshot(self.config.project_slug.as_deref())?;

        RuntimeHttpResponse::json(
            201,
            json!({
                "api_key": api_key,
                "api_keys": repository.api_key_snapshots()?,
                "snapshot": snapshot,
            }),
        )
    }

    fn snapshot_response(&self, request: &RuntimeHttpRequest) -> Result<RuntimeHttpResponse> {
        RuntimeHttpResponse::json(200, self.load_snapshot_for_request(request)?)
    }

    fn projects_response(&self, request: &RuntimeHttpRequest) -> Result<RuntimeHttpResponse> {
        let repository = RuntimeRepository::open(&self.config.db_path)?;
        repository.migrate()?;
        let active_project_filter = match project_filter_for_request(request) {
            Some(ProjectFilterOverride::All) => None,
            Some(ProjectFilterOverride::Slug(slug)) => Some(slug.to_string()),
            None => self.config.project_slug.clone(),
        };
        let snapshot = repository.snapshot(None)?;

        RuntimeHttpResponse::json(
            200,
            json!({
                "project_filter": active_project_filter,
                "count": snapshot.projects.len(),
                "projects": snapshot.projects,
            }),
        )
    }

    fn events_response(&self, request: &RuntimeHttpRequest) -> Result<RuntimeHttpResponse> {
        let (project_filter, latest_event_id, events) = self.event_slice_for_request(request)?;

        RuntimeHttpResponse::json(
            200,
            json!({
                "project_filter": project_filter,
                "latest_event_id": latest_event_id,
                "count": events.len(),
                "events": events,
            }),
        )
    }

    fn events_stream_response(&self, request: &RuntimeHttpRequest) -> Result<RuntimeHttpResponse> {
        let (_, latest_event_id, events) = self.event_slice_for_request(request)?;
        let mut body = String::from(": pool-runtime-events\n");
        if let Some(latest_event_id) = latest_event_id {
            body.push_str(&sse_cursor_frame(&latest_event_id));
        }
        for event in events {
            body.push_str(&sse_event_frame(&event)?);
        }

        Ok(RuntimeHttpResponse::text(
            200,
            "text/event-stream; charset=utf-8",
            body,
        ))
    }

    fn events_websocket_response(
        &self,
        request: &RuntimeHttpRequest,
    ) -> Result<RuntimeHttpResponse> {
        let (project_filter, latest_event_id, _) = self.event_slice_for_request(request)?;
        RuntimeHttpResponse::json(
            426,
            json!({
                "error": "websocket_upgrade_required",
                "message": "Use a WebSocket client with Connection: Upgrade and Sec-WebSocket-Key.",
                "path": "/api/events/ws",
                "project_filter": project_filter,
                "latest_event_id": latest_event_id,
                "fallbacks": {
                    "sse": "/api/events/stream",
                    "polling": "/api/events",
                },
            }),
        )
    }

    fn provider_request_metadata_response(
        &self,
        request: &RuntimeHttpRequest,
    ) -> Result<RuntimeHttpResponse> {
        let Some(provider_request_id) = request
            .query
            .get("provider_request_id")
            .or_else(|| request.query.get("request_id"))
            .map(String::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            return RuntimeHttpResponse::json(
                400,
                json!({
                    "error": "missing_provider_request_id",
                    "expected": "GET /api/provider-requests/metadata?provider_request_id=<provider-request-id>",
                }),
            );
        };

        let snapshot = self.load_snapshot_for_request(request)?;
        let Some(provider_request) = snapshot
            .provider_requests
            .iter()
            .find(|request| request.id == provider_request_id)
        else {
            return RuntimeHttpResponse::json(
                404,
                json!({
                    "error": "provider_request_not_found",
                    "provider_request_id": provider_request_id,
                }),
            );
        };
        let Some(metadata_path) = provider_request
            .metadata_path
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            return RuntimeHttpResponse::json(
                404,
                json!({
                    "error": "provider_request_metadata_missing",
                    "provider_request_id": provider_request_id,
                }),
            );
        };
        let metadata_text = match fs::read_to_string(metadata_path) {
            Ok(text) => text,
            Err(error) => {
                return RuntimeHttpResponse::json(
                    404,
                    json!({
                        "error": "provider_request_metadata_unreadable",
                        "provider_request_id": provider_request_id,
                        "metadata_path": metadata_path,
                        "message": error.to_string(),
                    }),
                );
            }
        };
        let metadata_json = serde_json::from_str::<Value>(&metadata_text).ok();
        let metadata_text_value = if metadata_json.is_some() {
            Value::Null
        } else {
            Value::String(metadata_text.clone())
        };

        RuntimeHttpResponse::json(
            200,
            json!({
                "provider_request_id": provider_request.id,
                "task_id": provider_request.task_id,
                "project_slug": provider_request.project_slug,
                "provider_id": provider_request.provider_id,
                "metadata_path": metadata_path,
                "bytes": metadata_text.len(),
                "metadata": metadata_json.unwrap_or(Value::Null),
                "metadata_text": metadata_text_value,
            }),
        )
    }

    fn agent_session_transcript_response(
        &self,
        request: &RuntimeHttpRequest,
    ) -> Result<RuntimeHttpResponse> {
        let Some(session_id) = request
            .query
            .get("session_id")
            .map(String::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            return RuntimeHttpResponse::json(
                400,
                json!({
                    "error": "missing_session_id",
                    "expected": "GET /api/agent-sessions/transcript?session_id=<agent-session-id>",
                }),
            );
        };

        let snapshot = self.load_snapshot_for_request(request)?;
        let Some(session) = snapshot
            .agent_sessions
            .iter()
            .find(|session| session.id == session_id)
        else {
            return RuntimeHttpResponse::json(
                404,
                json!({
                    "error": "agent_session_not_found",
                    "session_id": session_id,
                }),
            );
        };
        let Some(transcript_path) = session
            .transcript_path
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            return RuntimeHttpResponse::json(
                404,
                json!({
                    "error": "agent_session_transcript_missing",
                    "session_id": session_id,
                }),
            );
        };
        let transcript_text = match std::fs::read_to_string(transcript_path) {
            Ok(text) => text,
            Err(error) => {
                return RuntimeHttpResponse::json(
                    404,
                    json!({
                        "error": "agent_session_transcript_unreadable",
                        "session_id": session_id,
                        "transcript_path": transcript_path,
                        "message": error.to_string(),
                    }),
                );
            }
        };
        let transcript_json = serde_json::from_str::<Value>(&transcript_text).ok();
        let transcript_text_value = if transcript_json.is_some() {
            Value::Null
        } else {
            Value::String(transcript_text.clone())
        };

        RuntimeHttpResponse::json(
            200,
            json!({
                "session_id": session.id,
                "project_slug": session.project_slug,
                "tools": session.tools.clone(),
                "token_budget": session.token_budget,
                "token_used": session.token_used,
                "transcript_path": transcript_path,
                "bytes": transcript_text.len(),
                "transcript": transcript_json.unwrap_or(Value::Null),
                "transcript_text": transcript_text_value,
            }),
        )
    }

    fn agent_session_stream_response(
        &self,
        request: &RuntimeHttpRequest,
    ) -> Result<RuntimeHttpResponse> {
        let transcript_response = self.agent_session_transcript_response(request)?;
        if transcript_response.status_code >= 400 {
            return Ok(transcript_response);
        }
        let transcript_payload: Value =
            serde_json::from_str(&transcript_response.body).context("parse transcript payload")?;
        let (session_id, latest_event_id, events) =
            self.agent_session_event_slice_for_request(request)?;
        let mut body = format!(": pool-agent-session {}\n\n", sse_line(&session_id));
        if let Some(latest_event_id) = latest_event_id {
            body.push_str(&sse_cursor_frame(&latest_event_id));
        }
        body.push_str(&sse_json_frame(
            "agent-transcript",
            None,
            &transcript_payload,
        )?);
        for event in events {
            body.push_str(&sse_event_frame(&event)?);
        }

        Ok(RuntimeHttpResponse::text(
            200,
            "text/event-stream; charset=utf-8",
            body,
        ))
    }

    fn agent_session_websocket_response(
        &self,
        request: &RuntimeHttpRequest,
    ) -> Result<RuntimeHttpResponse> {
        let transcript_response = self.agent_session_transcript_response(request)?;
        if transcript_response.status_code >= 400 {
            return Ok(transcript_response);
        }
        let session_id = agent_session_id_for_request(request)?.to_string();
        RuntimeHttpResponse::json(
            426,
            json!({
                "error": "websocket_upgrade_required",
                "message": "Use a WebSocket client with Connection: Upgrade and Sec-WebSocket-Key.",
                "path": "/api/agent-sessions/ws",
                "session_id": session_id,
                "fallback": format!("/api/agent-sessions/stream?session_id={}", percent_encode_query_value(&session_id)),
            }),
        )
    }

    fn stream_agent_session_to_connection(
        &self,
        stream: &mut TcpStream,
        request: RuntimeHttpRequest,
    ) -> Result<()> {
        self.stream_agent_session_to_writer(stream, request, None)
    }

    fn stream_agent_session_to_writer<W: Write>(
        &self,
        writer: &mut W,
        mut request: RuntimeHttpRequest,
        max_ticks: Option<usize>,
    ) -> Result<()> {
        let transcript_response = self.agent_session_transcript_response(&request)?;
        if transcript_response.status_code >= 400 {
            writer
                .write_all(transcript_response.to_http_bytes().as_bytes())
                .context("write Agent session stream error response")?;
            return Ok(());
        }
        let transcript_payload: Value =
            serde_json::from_str(&transcript_response.body).context("parse transcript payload")?;
        let session_id = agent_session_id_for_request(&request)?.to_string();

        writer
            .write_all(sse_response_headers().as_bytes())
            .context("write Agent session SSE response headers")?;
        writer
            .write_all(format!(": pool-agent-session {}\n\n", sse_line(&session_id)).as_bytes())
            .context("write Agent session SSE prelude")?;
        writer
            .write_all(sse_json_frame("agent-transcript", None, &transcript_payload)?.as_bytes())
            .context("write Agent transcript SSE frame")?;
        writer.flush().context("flush Agent session SSE prelude")?;

        let mut ticks = 0_usize;
        let mut last_event_id = request
            .query
            .get("last_event_id")
            .or_else(|| request.query.get("after_id"))
            .cloned();
        let poll_ms = request
            .query
            .get("poll_ms")
            .and_then(|value| value.parse::<u64>().ok())
            .map(|value| value.clamp(500, 30_000))
            .unwrap_or(2_000);
        let mut idle_ticks = 0_u64;

        loop {
            if let Some(cursor) = &last_event_id {
                request
                    .query
                    .insert("last_event_id".to_string(), cursor.clone());
            } else {
                request.query.remove("last_event_id");
            }

            let (_, _, events) = self.agent_session_event_slice_for_request(&request)?;
            if events.is_empty() {
                idle_ticks += 1;
                if idle_ticks % 8 == 0 {
                    writer
                        .write_all(b": heartbeat\n\n")
                        .context("write Agent session SSE heartbeat")?;
                    writer
                        .flush()
                        .context("flush Agent session SSE heartbeat")?;
                }
            } else {
                idle_ticks = 0;
                for event in events.iter().rev() {
                    writer
                        .write_all(sse_event_frame(event)?.as_bytes())
                        .context("write Agent session runtime event")?;
                    last_event_id = Some(event.id.clone());
                }
                writer
                    .flush()
                    .context("flush Agent session runtime events")?;
            }

            ticks += 1;
            if max_ticks.is_some_and(|max_ticks| ticks >= max_ticks) {
                return Ok(());
            }

            thread::sleep(Duration::from_millis(poll_ms));
        }
    }

    fn stream_agent_session_websocket_to_connection(
        &self,
        stream: &mut TcpStream,
        request: RuntimeHttpRequest,
        headers: &BTreeMap<String, String>,
    ) -> Result<()> {
        let Some(sec_websocket_key) = websocket_header(headers, "sec-websocket-key") else {
            return write_websocket_upgrade_error(
                stream,
                "missing Sec-WebSocket-Key",
                "/api/agent-sessions/ws",
            );
        };
        if !websocket_header_contains(headers, "upgrade", "websocket")
            || !websocket_header_contains(headers, "connection", "upgrade")
        {
            return write_websocket_upgrade_error(
                stream,
                "missing WebSocket Upgrade/Connection headers",
                "/api/agent-sessions/ws",
            );
        }
        if websocket_header(headers, "sec-websocket-version") != Some("13") {
            return write_websocket_upgrade_error(
                stream,
                "unsupported WebSocket version",
                "/api/agent-sessions/ws",
            );
        }

        let protocol = websocket_requested_protocol(headers);
        stream
            .write_all(websocket_response_headers(sec_websocket_key, protocol).as_bytes())
            .context("write Agent session WebSocket upgrade response")?;
        stream
            .flush()
            .context("flush Agent session WebSocket upgrade response")?;
        self.stream_agent_session_websocket_to_writer(stream, request, None)
    }

    fn stream_agent_session_websocket_to_writer<W: Write>(
        &self,
        writer: &mut W,
        mut request: RuntimeHttpRequest,
        max_ticks: Option<usize>,
    ) -> Result<()> {
        let transcript_response = self.agent_session_transcript_response(&request)?;
        if transcript_response.status_code >= 400 {
            let payload = serde_json::from_str::<Value>(&transcript_response.body)
                .unwrap_or_else(|_| json!({ "error": transcript_response.body }));
            write_websocket_json_frame(
                writer,
                &json!({
                    "type": "agent-session-error",
                    "status_code": transcript_response.status_code,
                    "error": payload,
                }),
            )
            .context("write Agent session WebSocket error frame")?;
            writer
                .flush()
                .context("flush Agent session WebSocket error frame")?;
            return Ok(());
        }
        let transcript_payload: Value =
            serde_json::from_str(&transcript_response.body).context("parse transcript payload")?;
        let session_id = agent_session_id_for_request(&request)?.to_string();
        let (_, latest_event_id, events) = self.agent_session_event_slice_for_request(&request)?;

        write_websocket_json_frame(
            writer,
            &json!({
                "type": "agent-session",
                "transport": "websocket",
                "session_id": session_id,
                "latest_event_id": latest_event_id,
                "transcript": transcript_payload,
                "event_count": events.len(),
            }),
        )
        .context("write Agent session WebSocket transcript frame")?;

        let mut last_event_id = latest_event_id.or_else(|| {
            request
                .query
                .get("last_event_id")
                .or_else(|| request.query.get("after_id"))
                .cloned()
        });
        for event in events.iter().rev() {
            write_websocket_json_frame(
                writer,
                &json!({
                    "type": "runtime-event",
                    "session_id": session_id,
                    "event": event,
                }),
            )
            .context("write initial Agent session WebSocket runtime event")?;
            last_event_id = Some(event.id.clone());
        }
        writer
            .flush()
            .context("flush initial Agent session WebSocket frames")?;

        let mut ticks = 0_usize;
        let poll_ms = request
            .query
            .get("poll_ms")
            .and_then(|value| value.parse::<u64>().ok())
            .map(|value| value.clamp(500, 30_000))
            .unwrap_or(2_000);
        let mut idle_ticks = 0_u64;

        loop {
            if let Some(cursor) = &last_event_id {
                request
                    .query
                    .insert("last_event_id".to_string(), cursor.clone());
            } else {
                request.query.remove("last_event_id");
            }

            let (_, _, events) = self.agent_session_event_slice_for_request(&request)?;
            if events.is_empty() {
                idle_ticks += 1;
                if idle_ticks % 8 == 0 {
                    write_websocket_json_frame(
                        writer,
                        &json!({
                            "type": "heartbeat",
                            "session_id": session_id,
                            "generated_at": chrono::Utc::now().to_rfc3339(),
                            "last_event_id": last_event_id,
                        }),
                    )
                    .context("write Agent session WebSocket heartbeat")?;
                    writer
                        .flush()
                        .context("flush Agent session WebSocket heartbeat")?;
                }
            } else {
                idle_ticks = 0;
                for event in events.iter().rev() {
                    write_websocket_json_frame(
                        writer,
                        &json!({
                            "type": "runtime-event",
                            "session_id": session_id,
                            "event": event,
                        }),
                    )
                    .context("write Agent session WebSocket runtime event")?;
                    last_event_id = Some(event.id.clone());
                }
                writer
                    .flush()
                    .context("flush Agent session WebSocket runtime events")?;
            }

            ticks += 1;
            if max_ticks.is_some_and(|max_ticks| ticks >= max_ticks) {
                return Ok(());
            }

            thread::sleep(Duration::from_millis(poll_ms));
        }
    }

    fn stream_events_to_connection(
        &self,
        stream: &mut TcpStream,
        request: RuntimeHttpRequest,
    ) -> Result<()> {
        self.stream_events_to_writer(stream, request, None)
    }

    fn stream_events_to_writer<W: Write>(
        &self,
        writer: &mut W,
        mut request: RuntimeHttpRequest,
        max_ticks: Option<usize>,
    ) -> Result<()> {
        writer
            .write_all(sse_response_headers().as_bytes())
            .context("write SSE response headers")?;
        writer
            .write_all(b": pool-runtime-events\n\n")
            .context("write SSE prelude")?;
        writer.flush().context("flush SSE prelude")?;

        let mut ticks = 0_usize;
        let mut last_event_id = request
            .query
            .get("last_event_id")
            .or_else(|| request.query.get("after_id"))
            .cloned();
        let poll_ms = request
            .query
            .get("poll_ms")
            .and_then(|value| value.parse::<u64>().ok())
            .map(|value| value.clamp(500, 30_000))
            .unwrap_or(2_000);
        let mut idle_ticks = 0_u64;

        loop {
            if let Some(cursor) = &last_event_id {
                request
                    .query
                    .insert("last_event_id".to_string(), cursor.clone());
            } else {
                request.query.remove("last_event_id");
            }

            let (_, _, events) = self.event_slice_for_request(&request)?;
            if events.is_empty() {
                idle_ticks += 1;
                if idle_ticks % 8 == 0 {
                    writer
                        .write_all(b": heartbeat\n\n")
                        .context("write SSE heartbeat")?;
                    writer.flush().context("flush SSE heartbeat")?;
                }
            } else {
                idle_ticks = 0;
                for event in events.iter().rev() {
                    writer
                        .write_all(sse_event_frame(event)?.as_bytes())
                        .context("write SSE runtime event")?;
                    last_event_id = Some(event.id.clone());
                }
                writer.flush().context("flush SSE runtime events")?;
            }

            ticks += 1;
            if max_ticks.is_some_and(|max_ticks| ticks >= max_ticks) {
                return Ok(());
            }

            thread::sleep(Duration::from_millis(poll_ms));
        }
    }

    fn stream_events_websocket_to_connection(
        &self,
        stream: &mut TcpStream,
        request: RuntimeHttpRequest,
        headers: &BTreeMap<String, String>,
    ) -> Result<()> {
        let Some(sec_websocket_key) = websocket_header(headers, "sec-websocket-key") else {
            return write_websocket_upgrade_error(
                stream,
                "missing Sec-WebSocket-Key",
                "/api/events/ws",
            );
        };
        if !websocket_header_contains(headers, "upgrade", "websocket")
            || !websocket_header_contains(headers, "connection", "upgrade")
        {
            return write_websocket_upgrade_error(
                stream,
                "missing WebSocket Upgrade/Connection headers",
                "/api/events/ws",
            );
        }
        if websocket_header(headers, "sec-websocket-version") != Some("13") {
            return write_websocket_upgrade_error(
                stream,
                "unsupported WebSocket version",
                "/api/events/ws",
            );
        }

        let protocol = websocket_requested_protocol(headers);
        stream
            .write_all(websocket_response_headers(sec_websocket_key, protocol).as_bytes())
            .context("write WebSocket upgrade response")?;
        stream.flush().context("flush WebSocket upgrade response")?;
        self.stream_events_websocket_to_writer(stream, request, None)
    }

    fn stream_events_websocket_to_writer<W: Write>(
        &self,
        writer: &mut W,
        mut request: RuntimeHttpRequest,
        max_ticks: Option<usize>,
    ) -> Result<()> {
        let (project_filter, latest_event_id, events) = self.event_slice_for_request(&request)?;
        write_websocket_json_frame(
            writer,
            &json!({
                "type": "pool-runtime-events",
                "transport": "websocket",
                "project_filter": project_filter,
                "latest_event_id": latest_event_id,
                "count": events.len(),
            }),
        )
        .context("write WebSocket prelude frame")?;

        let mut last_event_id = latest_event_id.or_else(|| {
            request
                .query
                .get("last_event_id")
                .or_else(|| request.query.get("after_id"))
                .cloned()
        });
        for event in events.iter().rev() {
            write_websocket_json_frame(
                writer,
                &json!({
                    "type": "runtime-event",
                    "event": event,
                }),
            )
            .context("write initial WebSocket runtime event")?;
            last_event_id = Some(event.id.clone());
        }
        writer.flush().context("flush initial WebSocket frames")?;

        let mut ticks = 0_usize;
        let poll_ms = request
            .query
            .get("poll_ms")
            .and_then(|value| value.parse::<u64>().ok())
            .map(|value| value.clamp(500, 30_000))
            .unwrap_or(2_000);
        let mut idle_ticks = 0_u64;

        loop {
            if let Some(cursor) = &last_event_id {
                request
                    .query
                    .insert("last_event_id".to_string(), cursor.clone());
            } else {
                request.query.remove("last_event_id");
            }

            let (_, _, events) = self.event_slice_for_request(&request)?;
            if events.is_empty() {
                idle_ticks += 1;
                if idle_ticks % 8 == 0 {
                    write_websocket_json_frame(
                        writer,
                        &json!({
                            "type": "heartbeat",
                            "generated_at": chrono::Utc::now().to_rfc3339(),
                            "last_event_id": last_event_id,
                        }),
                    )
                    .context("write WebSocket heartbeat")?;
                    writer.flush().context("flush WebSocket heartbeat")?;
                }
            } else {
                idle_ticks = 0;
                for event in events.iter().rev() {
                    write_websocket_json_frame(
                        writer,
                        &json!({
                            "type": "runtime-event",
                            "event": event,
                        }),
                    )
                    .context("write WebSocket runtime event")?;
                    last_event_id = Some(event.id.clone());
                }
                writer.flush().context("flush WebSocket runtime events")?;
            }

            ticks += 1;
            if max_ticks.is_some_and(|max_ticks| ticks >= max_ticks) {
                return Ok(());
            }

            thread::sleep(Duration::from_millis(poll_ms));
        }
    }

    fn event_slice_for_request(
        &self,
        request: &RuntimeHttpRequest,
    ) -> Result<(Option<String>, Option<String>, Vec<crate::EventSnapshot>)> {
        let snapshot = self.load_snapshot_for_request(request)?;
        let limit = request
            .query
            .get("limit")
            .and_then(|value| value.parse::<usize>().ok())
            .map(|value| value.clamp(1, 200))
            .unwrap_or(50);
        let after_id = request
            .query
            .get("after_id")
            .or_else(|| request.query.get("last_event_id"))
            .map(String::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let events = match after_id {
            Some(after_id) => snapshot
                .events
                .iter()
                .take_while(|event| event.id != after_id)
                .take(limit)
                .cloned()
                .collect::<Vec<_>>(),
            None => snapshot
                .events
                .iter()
                .take(limit)
                .cloned()
                .collect::<Vec<_>>(),
        };
        let latest_event_id = snapshot.events.first().map(|event| event.id.clone());

        Ok((snapshot.project_filter, latest_event_id, events))
    }

    fn agent_session_event_slice_for_request(
        &self,
        request: &RuntimeHttpRequest,
    ) -> Result<(String, Option<String>, Vec<crate::EventSnapshot>)> {
        let session_id = agent_session_id_for_request(request)?.to_string();
        let snapshot = self.load_snapshot_for_request(request)?;
        let limit = request
            .query
            .get("limit")
            .and_then(|value| value.parse::<usize>().ok())
            .map(|value| value.clamp(1, 200))
            .unwrap_or(50);
        let after_id = request
            .query
            .get("after_id")
            .or_else(|| request.query.get("last_event_id"))
            .map(String::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let matching_events = snapshot
            .events
            .iter()
            .filter(|event| event.message.contains(&session_id))
            .cloned()
            .collect::<Vec<_>>();
        let events = match after_id {
            Some(after_id) => matching_events
                .iter()
                .take_while(|event| event.id != after_id)
                .take(limit)
                .cloned()
                .collect::<Vec<_>>(),
            None => matching_events.into_iter().take(limit).collect::<Vec<_>>(),
        };
        let latest_event_id = events.first().map(|event| event.id.clone());

        Ok((session_id, latest_event_id, events))
    }

    fn resources_response(&self, request: &RuntimeHttpRequest) -> Result<RuntimeHttpResponse> {
        let snapshot = self.load_snapshot_for_request(request)?;
        let server = McpServer::from_snapshot(snapshot);
        RuntimeHttpResponse::json(200, json!({ "resources": server.list_resources() }))
    }

    fn prompts_response(&self, request: &RuntimeHttpRequest) -> Result<RuntimeHttpResponse> {
        let Some(name) = request
            .query
            .get("name")
            .or_else(|| request.query.get("prompt"))
            .filter(|name| !name.trim().is_empty())
        else {
            return RuntimeHttpResponse::json(
                200,
                json!({ "prompts": pool_mcp_prompt_definitions() }),
            );
        };

        let mut arguments = serde_json::Map::new();
        for (key, value) in &request.query {
            if key == "name" || key == "prompt" {
                continue;
            }
            arguments.insert(key.clone(), Value::String(value.clone()));
        }

        match pool_mcp_prompt_get_result(json!({
            "name": name,
            "arguments": arguments,
        })) {
            Ok(prompt) => RuntimeHttpResponse::json(200, prompt),
            Err(error) => RuntimeHttpResponse::json(
                400,
                json!({
                    "error": "invalid_prompt_request",
                    "name": name,
                    "message": error.to_string(),
                }),
            ),
        }
    }

    fn runtime_graph_response(&self, request: &RuntimeHttpRequest) -> Result<RuntimeHttpResponse> {
        let snapshot = self.load_snapshot_for_request(request)?;
        RuntimeHttpResponse::json(200, runtime_graph_resource(&snapshot)?)
    }

    fn runtime_execution_plan_response(
        &self,
        request: &RuntimeHttpRequest,
    ) -> Result<RuntimeHttpResponse> {
        let snapshot = self.load_snapshot_for_request(request)?;
        RuntimeHttpResponse::json(200, runtime_execution_plan_resource(&snapshot)?)
    }

    fn runtime_execution_plan_run_next_response(
        &self,
        http_request: &RuntimeHttpRequest,
        body: &str,
    ) -> Result<RuntimeHttpResponse> {
        let run_request = if body.trim().is_empty() {
            RuntimeExecutionPlanRunNextRequest::default()
        } else {
            match serde_json::from_str::<RuntimeExecutionPlanRunNextRequest>(body) {
                Ok(request) => request,
                Err(error) => {
                    return RuntimeHttpResponse::json(
                        400,
                        json!({
                            "error": "invalid_runtime_execution_plan_run_next_request",
                            "message": error.to_string(),
                        }),
                    );
                }
            }
        };
        let project_slug = run_request
            .project_slug
            .clone()
            .or_else(|| http_request.query.get("project").cloned())
            .or_else(|| self.config.project_slug.clone())
            .unwrap_or_else(|| "demo".to_string());
        if project_slug.trim().is_empty() || project_slug == "*" {
            return RuntimeHttpResponse::json(
                400,
                json!({
                    "error": "runtime_execution_plan_run_next_requires_project",
                    "message": "run-next needs a concrete project slug",
                }),
            );
        }

        let repository = RuntimeRepository::open(&self.config.db_path)?;
        repository.migrate()?;
        let snapshot = repository.snapshot(Some(&project_slug))?;
        let plan = runtime_execution_plan_resource(&snapshot)?;
        let Some(selected_step) = runtime_execution_plan_selected_step(&plan, &run_request) else {
            return RuntimeHttpResponse::json(
                404,
                json!({
                    "error": "runtime_execution_plan_step_not_found",
                    "project_slug": project_slug,
                    "node_id": run_request.node_id,
                    "task_id": run_request.task_id,
                }),
            );
        };
        let action = selected_step
            .get("control")
            .and_then(|control| control.get("recommended_action"))
            .cloned()
            .unwrap_or_else(|| json!({}));
        let action_kind = action
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or("run_node");

        if !run_request.execute.unwrap_or(false) {
            return RuntimeHttpResponse::json(
                200,
                json!({
                    "mode": "preview",
                    "executed": false,
                    "project_slug": project_slug,
                    "selected_step": selected_step,
                    "action": action,
                    "message": "set execute:true to dispatch this runtime execution plan step",
                }),
            );
        }

        let response = match action_kind {
            "approve_task" | "approval" => {
                if !run_request.allow_approval.unwrap_or(false) {
                    return RuntimeHttpResponse::json(
                        409,
                        json!({
                            "error": "runtime_execution_plan_approval_requires_explicit_allow",
                            "message": "approval steps require allow_approval:true",
                            "selected_step": selected_step,
                            "action": action,
                        }),
                    );
                }
                let task_id = action
                    .get("arguments")
                    .and_then(|arguments| arguments.get("task_id"))
                    .and_then(Value::as_str)
                    .or_else(|| {
                        selected_step
                            .get("gate")
                            .and_then(|gate| gate.get("task_id"))
                            .and_then(Value::as_str)
                    });
                let Some(task_id) = task_id else {
                    return RuntimeHttpResponse::json(
                        409,
                        json!({
                            "error": "runtime_execution_plan_action_missing_task_id",
                            "selected_step": selected_step,
                            "action": action,
                        }),
                    );
                };
                self.handle_request(
                    "POST",
                    &format!(
                        "/api/tasks/approve?task_id={}&project={}",
                        percent_encode_query_value(task_id),
                        percent_encode_query_value(&project_slug)
                    ),
                )?
            }
            "retry_task" => {
                let task_id = action
                    .get("arguments")
                    .and_then(|arguments| arguments.get("task_id"))
                    .and_then(Value::as_str)
                    .or_else(|| {
                        selected_step
                            .get("evidence")
                            .and_then(|evidence| evidence.get("latest_task"))
                            .and_then(|task| task.get("id"))
                            .and_then(Value::as_str)
                    });
                let Some(task_id) = task_id else {
                    return RuntimeHttpResponse::json(
                        409,
                        json!({
                            "error": "runtime_execution_plan_action_missing_task_id",
                            "selected_step": selected_step,
                            "action": action,
                        }),
                    );
                };
                self.handle_request(
                    "POST",
                    &format!(
                        "/api/tasks/retry?task_id={}&project={}",
                        percent_encode_query_value(task_id),
                        percent_encode_query_value(&project_slug)
                    ),
                )?
            }
            kind if kind.starts_with("run_") => {
                let node_id = action
                    .get("arguments")
                    .and_then(|arguments| arguments.get("node_id"))
                    .and_then(Value::as_str)
                    .or_else(|| selected_step.get("node_id").and_then(Value::as_str));
                let Some(node_id) = node_id else {
                    return RuntimeHttpResponse::json(
                        409,
                        json!({
                            "error": "runtime_execution_plan_action_missing_node_id",
                            "selected_step": selected_step,
                            "action": action,
                        }),
                    );
                };
                self.node_run_response(
                    &json!({
                        "project_slug": project_slug.clone(),
                        "node_id": node_id,
                        "execution_mode": run_request.execution_mode,
                        "endpoint": run_request.endpoint,
                        "api_key": run_request.api_key,
                        "prompt": run_request.prompt,
                        "input_paths": run_request.input_paths,
                        "output_dir": run_request.output_dir,
                        "duration_ms": run_request.duration_ms,
                    })
                    .to_string(),
                )?
            }
            _ => {
                return RuntimeHttpResponse::json(
                    409,
                    json!({
                        "error": "runtime_execution_plan_action_not_executable",
                        "action_kind": action_kind,
                        "selected_step": selected_step,
                        "action": action,
                    }),
                );
            }
        };

        let executed_body =
            serde_json::from_str::<Value>(&response.body).unwrap_or_else(|_| json!(response.body));
        RuntimeHttpResponse::json(
            response.status_code,
            json!({
                "mode": "executed",
                "executed": true,
                "project_slug": project_slug,
                "selected_step": selected_step,
                "action": action,
                "dispatch_status": response.status_code,
                "dispatch": executed_body,
            }),
        )
    }

    fn runtime_budget_response(&self, request: &RuntimeHttpRequest) -> Result<RuntimeHttpResponse> {
        let snapshot = self.load_snapshot_for_request(request)?;
        RuntimeHttpResponse::json(200, runtime_budget_resource(&snapshot))
    }

    fn runtime_preflight_response(
        &self,
        request: &RuntimeHttpRequest,
    ) -> Result<RuntimeHttpResponse> {
        let snapshot = self.load_snapshot_for_request(request)?;
        RuntimeHttpResponse::json(200, runtime_preflight_resource(&snapshot)?)
    }

    fn runtime_handoff_response(
        &self,
        request: &RuntimeHttpRequest,
    ) -> Result<RuntimeHttpResponse> {
        let snapshot = self.load_snapshot_for_request(request)?;
        RuntimeHttpResponse::json(200, runtime_handoff_resource(&snapshot)?)
    }

    fn prd_readiness_response(&self, request: &RuntimeHttpRequest) -> Result<RuntimeHttpResponse> {
        let snapshot = self.load_snapshot_for_request(request)?;
        RuntimeHttpResponse::json(200, runtime_prd_readiness_resource(&snapshot)?)
    }

    fn prd_completion_gate_response(
        &self,
        request: &RuntimeHttpRequest,
    ) -> Result<RuntimeHttpResponse> {
        let snapshot = self.load_snapshot_for_request(request)?;
        let gate = runtime_prd_completion_gate_resource(&snapshot)?;
        let require_complete = request
            .query
            .get("require_complete")
            .or_else(|| request.query.get("require-complete"))
            .or_else(|| request.query.get("fail_if_incomplete"))
            .or_else(|| request.query.get("fail-if-incomplete"))
            .is_some_and(|value| query_bool(value));
        let ready_for_completion = gate
            .pointer("/completion_gate/ready_for_completion")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        if require_complete && !ready_for_completion {
            return RuntimeHttpResponse::json(
                428,
                json!({
                    "error": "prd_completion_gate_incomplete",
                    "message": "PRD completion gate is not satisfied by the current Runtime snapshot.",
                    "completion_gate": gate.get("completion_gate").cloned().unwrap_or_else(|| json!({})),
                    "prd_summary": gate.get("summary").cloned().unwrap_or_else(|| json!({})),
                    "overall_status": gate.get("overall_status").cloned().unwrap_or_else(|| json!("unknown")),
                    "project_filter": gate.get("project_filter").cloned().unwrap_or(Value::Null),
                }),
            );
        }

        RuntimeHttpResponse::json(200, gate)
    }

    fn prd_completion_package_response(&self, body: &str) -> Result<RuntimeHttpResponse> {
        let request = match serde_json::from_str::<CreatePrdCompletionPackageRequest>(body) {
            Ok(request) => request,
            Err(error) => {
                return RuntimeHttpResponse::json(
                    400,
                    json!({
                        "error": "invalid_prd_completion_package_request",
                        "message": error.to_string(),
                    }),
                );
            }
        };
        let project_slug = request
            .project_slug
            .clone()
            .or_else(|| self.config.project_slug.clone())
            .unwrap_or_else(|| "demo".to_string());
        if project_slug == "*" {
            return RuntimeHttpResponse::json(
                400,
                json!({
                    "error": "prd_completion_package_requires_project",
                    "expected": "Use a concrete project slug, not *",
                }),
            );
        }
        let output_dir = request.output_dir.clone().unwrap_or_else(|| {
            self.default_output_dir(&project_slug)
                .to_string_lossy()
                .to_string()
        });
        let source = request
            .source
            .clone()
            .filter(|source| !source.trim().is_empty())
            .unwrap_or_else(|| "prd-completion-package".to_string());
        let title = request
            .title
            .clone()
            .filter(|title| !title.trim().is_empty())
            .unwrap_or_else(|| "Pool PRD completion package".to_string());

        let repository = RuntimeRepository::open(&self.config.db_path)?;
        repository.migrate()?;
        let snapshot = repository.snapshot(Some(&project_slug))?;
        let mut task = RuntimeTask::new(project_slug.clone(), title.clone());
        task.node_id = request.node_id.clone();
        task.provider_id = Some("prd-completion-package".to_string());
        task.cost_estimate_tokens = 120;
        task.status = TaskStatus::Running;
        repository.insert_task(&task)?;
        repository.insert_event(&RuntimeEvent::new(
            project_slug.clone(),
            RuntimeEventLevel::Info,
            format!("PRD completion package started: {title}"),
        ))?;

        let package_dir = Path::new(&output_dir)
            .join("control")
            .join("prd-completion");
        let report = write_prd_completion_package(
            &package_dir,
            &project_slug,
            request.node_id.as_deref(),
            &title,
            &source,
            request.include_snapshot.unwrap_or(true),
            &snapshot,
        )?;
        let local_paths = json_string_array_at(&report, "local_paths");
        let assets = repository.index_local_outputs(
            &project_slug,
            request.node_id.as_deref(),
            Some("pool-prd://completion-package"),
            &local_paths,
        )?;
        repository.update_task_status(&task.id, TaskStatus::Succeeded)?;
        repository.insert_event(&RuntimeEvent::new(
            project_slug.clone(),
            RuntimeEventLevel::Ok,
            format!("PRD completion package succeeded: {} files", assets.len()),
        ))?;
        let task = repository.task_snapshot(&task.id)?;
        let snapshot = repository.snapshot(self.config.project_slug.as_deref())?;

        RuntimeHttpResponse::json(
            201,
            json!({
                "kind": "pool_prd_completion_package",
                "report": report,
                "task": task,
                "assets": assets,
                "snapshot": snapshot,
            }),
        )
    }

    fn production_evidence_requirements_response(
        &self,
        request: &RuntimeHttpRequest,
    ) -> Result<RuntimeHttpResponse> {
        let snapshot = self.load_snapshot_for_request(request)?;
        RuntimeHttpResponse::json(
            200,
            runtime_production_evidence_requirements_resource(&snapshot)?,
        )
    }

    fn production_evidence_tasks_response(
        &self,
        request: &RuntimeHttpRequest,
    ) -> Result<RuntimeHttpResponse> {
        let snapshot = self.load_snapshot_for_request(request)?;
        let project_slug = snapshot
            .project_filter
            .as_deref()
            .filter(|project_slug| *project_slug != "*")
            .unwrap_or("<slug>");

        RuntimeHttpResponse::json(
            200,
            production_evidence_tasks_resource(project_slug, &snapshot)?,
        )
    }

    fn production_evidence_task_claim_response(&self, body: &str) -> Result<RuntimeHttpResponse> {
        let request = match serde_json::from_str::<ClaimProductionEvidenceTaskRequest>(body) {
            Ok(request) => request,
            Err(error) => {
                return RuntimeHttpResponse::json(
                    400,
                    json!({
                        "error": "invalid_production_evidence_task_claim_request",
                        "message": error.to_string(),
                    }),
                );
            }
        };
        let project_slug = request
            .project_slug
            .clone()
            .or_else(|| self.config.project_slug.clone())
            .unwrap_or_else(|| "demo".to_string());
        if project_slug == "*" {
            return RuntimeHttpResponse::json(
                400,
                json!({
                    "error": "production_evidence_task_claim_requires_project",
                    "message": "production evidence task claim requires a concrete project slug",
                }),
            );
        }
        let task_id = request.task_id.trim();
        if task_id.is_empty() {
            return RuntimeHttpResponse::json(
                400,
                json!({
                    "error": "invalid_production_evidence_task_claim_request",
                    "message": "task_id cannot be empty",
                }),
            );
        }

        let repository = RuntimeRepository::open(&self.config.db_path)?;
        repository.migrate()?;
        let snapshot = repository.snapshot(Some(project_slug.as_str()))?;
        let task_queue = production_evidence_tasks_resource(&project_slug, &snapshot)?;
        let evidence_task =
            match task_queue
                .get("tasks")
                .and_then(Value::as_array)
                .and_then(|tasks| {
                    tasks
                        .iter()
                        .find(|task| task.get("id").and_then(Value::as_str) == Some(task_id))
                }) {
                Some(task) => task.clone(),
                None => {
                    return RuntimeHttpResponse::json(
                        404,
                        json!({
                            "error": "production_evidence_task_not_found",
                            "task_id": task_id,
                            "message": "task_id is not currently missing for this project",
                        }),
                    );
                }
            };
        let (kind, target_id) = match production_evidence_selector_from_task_id(task_id) {
            Ok(selector) => selector,
            Err(error) => {
                return RuntimeHttpResponse::json(
                    400,
                    json!({
                        "error": "invalid_production_evidence_task_claim_request",
                        "message": error.to_string(),
                    }),
                );
            }
        };
        let output_root = request
            .output_root
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
            .unwrap_or_else(|| format!("worlds/{project_slug}/output"));
        let source = request
            .source
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("production_evidence_task_claim");
        let item_template = production_evidence_item_template_value(
            &project_slug,
            Some(output_root.as_str()),
            source,
            &kind,
            &target_id,
            Some(task_id),
        )?;

        let mut runtime_task = RuntimeTask::new(
            project_slug.clone(),
            format!(
                "Claim production evidence: {}",
                evidence_task
                    .get("title")
                    .and_then(Value::as_str)
                    .unwrap_or(task_id)
            ),
        );
        runtime_task.status = TaskStatus::Running;
        runtime_task.provider_id = if kind == "provider" {
            Some(target_id.clone())
        } else {
            None
        };

        let claim_dir = PathBuf::from(&output_root)
            .join("control")
            .join("production-evidence")
            .join("claims");
        fs::create_dir_all(&claim_dir).with_context(|| {
            format!(
                "create production evidence claim dir {}",
                claim_dir.display()
            )
        })?;
        let claim_path = claim_dir.join(format!(
            "{}-claim.json",
            production_evidence_file_slug(task_id)
        ));
        runtime_task.request_metadata_path = Some(path_string_lossy(&claim_path));
        let claim = json!({
            "kind": "pool_production_evidence_task_claim",
            "version": 1,
            "project_slug": project_slug,
            "task_id": task_id,
            "runtime_task_id": runtime_task.id,
            "claimed_at": chrono::Utc::now().to_rfc3339(),
            "assignee": request.assignee,
            "role": request.role,
            "source": source,
            "output_root": output_root,
            "selector": {
                "kind": kind,
                "target_id": target_id,
            },
            "evidence_task": evidence_task,
            "item_template": item_template,
            "commands": {
                "validate_item": format!("pool-cli --project {} validate-production-evidence-item <item.json>", runtime_task.project_slug),
                "submit_item": format!("pool-cli --project {} submit-production-evidence-item <item.json>", runtime_task.project_slug),
                "readiness": format!("pool-cli --project {} prd-readiness", runtime_task.project_slug),
            },
            "http": {
                "validate_item": "POST /api/production-evidence/items/validate",
                "submit_item": "POST /api/production-evidence/items",
                "tasks": format!("GET /api/production-evidence/tasks?project={}", runtime_task.project_slug),
            },
            "mcp": {
                "validate_item_tool": "pool_validate_production_evidence_item",
                "submit_tool": "pool_submit_production_evidence_item",
            }
        });
        write_server_json_file(&claim_path, &claim)?;
        repository.insert_task(&runtime_task)?;
        repository.insert_event(&RuntimeEvent::new(
            runtime_task.project_slug.clone(),
            RuntimeEventLevel::Info,
            format!("claimed production evidence task: {task_id}"),
        ))?;
        let task = repository.task_snapshot(&runtime_task.id)?;
        let snapshot = repository.snapshot(Some(runtime_task.project_slug.as_str()))?;

        RuntimeHttpResponse::json(
            201,
            json!({
                "kind": "pool_production_evidence_task_claim",
                "project_slug": runtime_task.project_slug,
                "task_id": task_id,
                "runtime_task": task,
                "claim_path": path_string_lossy(&claim_path),
                "claim": claim,
                "snapshot": snapshot,
            }),
        )
    }

    fn production_evidence_run_plan_response(
        &self,
        request: &RuntimeHttpRequest,
    ) -> Result<RuntimeHttpResponse> {
        let snapshot = self.load_snapshot_for_request(request)?;
        let project_slug = request
            .query
            .get("project")
            .or_else(|| request.query.get("project_slug"))
            .cloned()
            .or_else(|| snapshot.project_filter.clone())
            .or_else(|| self.config.project_slug.clone())
            .unwrap_or_else(|| "demo".to_string());
        if project_slug == "*" {
            return RuntimeHttpResponse::json(
                400,
                json!({
                    "error": "production_evidence_run_plan_requires_project",
                    "expected": "Use a concrete project slug, not *",
                }),
            );
        }
        let output_root = request
            .query
            .get("output_root")
            .or_else(|| request.query.get("output-root"))
            .map(String::as_str)
            .filter(|value| !value.trim().is_empty());
        let source = request
            .query
            .get("source")
            .map(String::as_str)
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("production-evidence-run-plan");

        RuntimeHttpResponse::json(
            200,
            production_evidence_run_plan_value(&project_slug, output_root, source, &snapshot)?,
        )
    }

    fn production_evidence_handoff_response(
        &self,
        request: &RuntimeHttpRequest,
    ) -> Result<RuntimeHttpResponse> {
        let project_slug = request
            .query
            .get("project")
            .or_else(|| request.query.get("project_slug"))
            .cloned()
            .or_else(|| self.config.project_slug.clone())
            .unwrap_or_else(|| "demo".to_string());
        if project_slug == "*" {
            return RuntimeHttpResponse::json(
                400,
                json!({
                    "error": "production_evidence_handoff_requires_project",
                    "expected": "Use a concrete project slug, not *",
                }),
            );
        }
        let output_root = request
            .query
            .get("output_root")
            .or_else(|| request.query.get("output-root"))
            .map(String::as_str)
            .filter(|value| !value.trim().is_empty());
        let source = request
            .query
            .get("source")
            .map(String::as_str)
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("production-evidence-handoff");
        let snapshot = self.load_snapshot_for_request(request)?;

        RuntimeHttpResponse::json(
            200,
            production_evidence_handoff_value(&project_slug, output_root, source, &snapshot)?,
        )
    }

    fn production_evidence_handoff_package_response(
        &self,
        body: &str,
    ) -> Result<RuntimeHttpResponse> {
        let request =
            match serde_json::from_str::<CreateProductionEvidenceHandoffPackageRequest>(body) {
                Ok(request) => request,
                Err(error) => {
                    return RuntimeHttpResponse::json(
                        400,
                        json!({
                            "error": "invalid_production_evidence_handoff_package_request",
                            "message": error.to_string(),
                        }),
                    );
                }
            };
        let project_slug = request
            .project_slug
            .clone()
            .or_else(|| self.config.project_slug.clone())
            .unwrap_or_else(|| "demo".to_string());
        if project_slug == "*" {
            return RuntimeHttpResponse::json(
                400,
                json!({
                    "error": "production_evidence_handoff_package_requires_project",
                    "expected": "Use a concrete project slug, not *",
                }),
            );
        }
        let output_dir = request.output_dir.clone().unwrap_or_else(|| {
            self.default_output_dir(&project_slug)
                .to_string_lossy()
                .to_string()
        });
        let output_root = request.output_root.clone().unwrap_or_else(|| {
            Path::new(&output_dir)
                .join("production-evidence")
                .to_string_lossy()
                .to_string()
        });
        let source = request
            .source
            .clone()
            .filter(|source| !source.trim().is_empty())
            .unwrap_or_else(|| "production-evidence-handoff-package".to_string());
        let title = request
            .title
            .clone()
            .filter(|title| !title.trim().is_empty())
            .unwrap_or_else(|| "Pool production evidence handoff package".to_string());

        let repository = RuntimeRepository::open(&self.config.db_path)?;
        repository.migrate()?;
        let snapshot = repository.snapshot(Some(&project_slug))?;
        let mut task = RuntimeTask::new(project_slug.clone(), title.clone());
        task.node_id = request.node_id.clone();
        task.provider_id = Some("production-evidence-handoff-package".to_string());
        task.cost_estimate_tokens = 240;
        task.status = TaskStatus::Running;
        repository.insert_task(&task)?;
        repository.insert_event(&RuntimeEvent::new(
            project_slug.clone(),
            RuntimeEventLevel::Info,
            format!("production evidence handoff package started: {title}"),
        ))?;

        let package_dir = Path::new(&output_dir)
            .join("control")
            .join("production-evidence");
        let report = write_production_evidence_handoff_package(
            &package_dir,
            &project_slug,
            request.node_id.as_deref(),
            &title,
            &output_root,
            &source,
            request.include_items.unwrap_or(true),
            request.include_snapshot.unwrap_or(false),
            &snapshot,
        )?;
        let local_paths = json_string_array_at(&report, "local_paths");
        let assets = repository.index_local_outputs(
            &project_slug,
            request.node_id.as_deref(),
            Some("pool-production-evidence://handoff-package"),
            &local_paths,
        )?;
        repository.update_task_status(&task.id, TaskStatus::Succeeded)?;
        repository.insert_event(&RuntimeEvent::new(
            project_slug.clone(),
            RuntimeEventLevel::Ok,
            format!(
                "production evidence handoff package succeeded: {} files",
                assets.len()
            ),
        ))?;
        let task = repository.task_snapshot(&task.id)?;
        let snapshot = repository.snapshot(self.config.project_slug.as_deref())?;

        RuntimeHttpResponse::json(
            201,
            json!({
                "kind": "pool_production_evidence_handoff_package",
                "report": report,
                "task": task,
                "assets": assets,
                "snapshot": snapshot,
            }),
        )
    }

    fn workflow_context_response(
        &self,
        request: &RuntimeHttpRequest,
    ) -> Result<RuntimeHttpResponse> {
        let snapshot = self.load_snapshot_for_request(request)?;
        let Some(workflow_id) = request
            .query
            .get("workflow_id")
            .or_else(|| request.query.get("workflow"))
            .filter(|workflow_id| !workflow_id.trim().is_empty())
        else {
            return RuntimeHttpResponse::json(
                200,
                runtime_workflow_context_index_resource(&snapshot)?,
            );
        };

        match runtime_workflow_context_resource(&snapshot, workflow_id) {
            Ok(value) => RuntimeHttpResponse::json(200, value),
            Err(error) => RuntimeHttpResponse::json(
                404,
                json!({
                    "error": "workflow_context_not_found",
                    "workflow_id": workflow_id,
                    "message": error.to_string(),
                }),
            ),
        }
    }

    fn node_context_response(&self, request: &RuntimeHttpRequest) -> Result<RuntimeHttpResponse> {
        let snapshot = self.load_snapshot_for_request(request)?;
        let Some(node_id) = request
            .query
            .get("node_id")
            .or_else(|| request.query.get("node"))
            .filter(|node_id| !node_id.trim().is_empty())
        else {
            return RuntimeHttpResponse::json(200, runtime_node_context_index_resource(&snapshot)?);
        };

        match runtime_node_context_resource(&snapshot, node_id) {
            Ok(value) => RuntimeHttpResponse::json(200, value),
            Err(error) => RuntimeHttpResponse::json(
                404,
                json!({
                    "error": "node_context_not_found",
                    "node_id": node_id,
                    "message": error.to_string(),
                }),
            ),
        }
    }

    fn adapters_response(&self) -> Result<RuntimeHttpResponse> {
        RuntimeHttpResponse::json(200, runtime_adapter_catalog_resource())
    }

    fn integration_readiness_response(
        &self,
        request: &RuntimeHttpRequest,
    ) -> Result<RuntimeHttpResponse> {
        let snapshot = self.load_snapshot_for_request(request)?;
        RuntimeHttpResponse::json(200, runtime_integration_readiness_resource(&snapshot))
    }

    fn provider_gateway_worker_response(&self) -> Result<RuntimeHttpResponse> {
        RuntimeHttpResponse::json(200, provider_gateway_worker_contract())
    }

    fn provider_contracts_response(
        &self,
        request: &RuntimeHttpRequest,
    ) -> Result<RuntimeHttpResponse> {
        let provider_id = request
            .query
            .get("provider_id")
            .or_else(|| request.query.get("provider"))
            .map(String::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty());
        match provider_contracts_resource(provider_id) {
            Ok(value) => RuntimeHttpResponse::json(200, value),
            Err(error) => RuntimeHttpResponse::json(
                404,
                json!({
                    "error": "provider_contract_not_found",
                    "provider_id": provider_id,
                    "message": error.to_string(),
                }),
            ),
        }
    }

    fn provider_conformance_package_response(&self, body: &str) -> Result<RuntimeHttpResponse> {
        let request = match serde_json::from_str::<CreateProviderConformancePackageRequest>(body) {
            Ok(request) => request,
            Err(error) => {
                return RuntimeHttpResponse::json(
                    400,
                    json!({
                        "error": "invalid_provider_conformance_package_request",
                        "message": error.to_string(),
                    }),
                );
            }
        };
        let provider_id = request.provider_id.trim().to_string();
        if provider_id.is_empty() {
            return RuntimeHttpResponse::json(
                400,
                json!({
                    "error": "missing_provider_id",
                    "message": "provider conformance package requires provider_id",
                }),
            );
        }
        let contract = match provider_contracts_resource(Some(&provider_id)) {
            Ok(contract) => contract,
            Err(error) => {
                return RuntimeHttpResponse::json(
                    404,
                    json!({
                        "error": "provider_contract_not_found",
                        "provider_id": provider_id,
                        "message": error.to_string(),
                    }),
                );
            }
        };
        let canonical_provider_id = contract["provider_id"]
            .as_str()
            .unwrap_or(provider_id.as_str())
            .to_string();
        let gateway_contract = provider_gateway_worker_contract();
        let project_slug = request
            .project_slug
            .clone()
            .or_else(|| self.config.project_slug.clone())
            .unwrap_or_else(|| "demo".to_string());
        let output_dir = request.output_dir.unwrap_or_else(|| {
            self.default_output_dir(&project_slug)
                .to_string_lossy()
                .to_string()
        });
        let title = request
            .title
            .filter(|title| !title.trim().is_empty())
            .unwrap_or_else(|| {
                format!("Pool provider conformance package: {canonical_provider_id}")
            });

        let repository = RuntimeRepository::open(&self.config.db_path)?;
        repository.migrate()?;
        let mut task = RuntimeTask::new(project_slug.clone(), title.clone());
        task.node_id = request.node_id.clone();
        task.provider_id = Some("provider-conformance-package".to_string());
        task.status = TaskStatus::Running;
        task.cost_estimate_tokens = 140;
        repository.insert_task(&task)?;
        repository.insert_event(&RuntimeEvent::new(
            project_slug.clone(),
            RuntimeEventLevel::Info,
            format!("provider conformance package started: {canonical_provider_id}"),
        ))?;

        let package_dir = Path::new(&output_dir)
            .join("control")
            .join("provider-conformance")
            .join(safe_package_segment(&canonical_provider_id));
        let report = write_provider_conformance_package(
            &package_dir,
            &project_slug,
            request.node_id.as_deref(),
            &canonical_provider_id,
            &title,
            &contract,
            &gateway_contract,
        )?;
        let local_paths = report["local_paths"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        let assets = repository.index_local_outputs(
            &project_slug,
            request.node_id.as_deref(),
            Some(&format!(
                "pool-provider-conformance://{canonical_provider_id}"
            )),
            &local_paths,
        )?;
        repository.update_task_status(&task.id, TaskStatus::Succeeded)?;
        repository.insert_event(&RuntimeEvent::new(
            project_slug.clone(),
            RuntimeEventLevel::Ok,
            format!(
                "provider conformance package succeeded: {canonical_provider_id}, {} files",
                assets.len()
            ),
        ))?;
        let task = repository.task_snapshot(&task.id)?;
        let snapshot = repository.snapshot(self.config.project_slug.as_deref())?;

        RuntimeHttpResponse::json(
            201,
            json!({
                "report": report,
                "assets": assets,
                "task": task,
                "snapshot": snapshot,
            }),
        )
    }

    fn integration_conformance_package_response(&self, body: &str) -> Result<RuntimeHttpResponse> {
        let request = match serde_json::from_str::<CreateIntegrationConformancePackageRequest>(body)
        {
            Ok(request) => request,
            Err(error) => {
                return RuntimeHttpResponse::json(
                    400,
                    json!({
                        "error": "invalid_integration_conformance_package_request",
                        "message": error.to_string(),
                    }),
                );
            }
        };
        let include_providers = request.include_providers.unwrap_or(true);
        let include_software = request.include_software.unwrap_or(true);
        let include_agent = request.include_agent.unwrap_or(true);
        let providers = if include_providers {
            conformance_request_items(request.providers, REQUIRED_PRODUCTION_PROVIDER_EVIDENCE)
        } else {
            Vec::new()
        };
        let software_adapters = if include_software {
            conformance_request_items(
                request.software_adapters,
                REQUIRED_PRODUCTION_SOFTWARE_EVIDENCE,
            )
        } else {
            Vec::new()
        };
        let agent_kind = if include_agent {
            match normalize_agent_conformance_kind(request.agent_kind.as_deref()) {
                Ok(kind) => Some(kind),
                Err(error) => {
                    return RuntimeHttpResponse::json(
                        400,
                        json!({
                            "error": "invalid_agent_conformance_kind",
                            "message": error.to_string(),
                        }),
                    );
                }
            }
        } else {
            None
        };
        if providers.is_empty() && software_adapters.is_empty() && agent_kind.is_none() {
            return RuntimeHttpResponse::json(
                400,
                json!({
                    "error": "empty_integration_conformance_package",
                    "message": "integration conformance package needs at least one provider, software adapter, or Agent/Hermes package",
                }),
            );
        }

        let project_slug = request
            .project_slug
            .clone()
            .or_else(|| self.config.project_slug.clone())
            .unwrap_or_else(|| "demo".to_string());
        let output_dir = request.output_dir.unwrap_or_else(|| {
            self.default_output_dir(&project_slug)
                .to_string_lossy()
                .to_string()
        });
        let title = request
            .title
            .filter(|title| !title.trim().is_empty())
            .unwrap_or_else(|| "Pool integration conformance package".to_string());

        let repository = RuntimeRepository::open(&self.config.db_path)?;
        repository.migrate()?;
        let mut task = RuntimeTask::new(project_slug.clone(), title.clone());
        task.node_id = request.node_id.clone();
        task.provider_id = Some("integration-conformance-package".to_string());
        task.status = TaskStatus::Running;
        task.cost_estimate_tokens = providers.len() as u64 * 140
            + software_adapters.len() as u64 * 120
            + if agent_kind.is_some() { 110 } else { 0 };
        repository.insert_task(&task)?;
        repository.insert_event(&RuntimeEvent::new(
            project_slug.clone(),
            RuntimeEventLevel::Info,
            format!(
                "integration conformance package started: {} providers, {} software, agent={}",
                providers.len(),
                software_adapters.len(),
                agent_kind.as_deref().unwrap_or("disabled")
            ),
        ))?;

        let package_dir = Path::new(&output_dir)
            .join("control")
            .join("integration-conformance");
        let report = write_integration_conformance_package(
            &package_dir,
            &project_slug,
            request.node_id.as_deref(),
            &title,
            &providers,
            &software_adapters,
            agent_kind.as_deref(),
        )?;
        let local_paths = report["local_paths"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        let assets = repository.index_local_outputs(
            &project_slug,
            request.node_id.as_deref(),
            Some("pool-integration-conformance://all"),
            &local_paths,
        )?;
        repository.update_task_status(&task.id, TaskStatus::Succeeded)?;
        repository.insert_event(&RuntimeEvent::new(
            project_slug.clone(),
            RuntimeEventLevel::Ok,
            format!(
                "integration conformance package succeeded: {} files",
                assets.len()
            ),
        ))?;
        let task = repository.task_snapshot(&task.id)?;
        let snapshot = repository.snapshot(self.config.project_slug.as_deref())?;

        RuntimeHttpResponse::json(
            201,
            json!({
                "report": report,
                "assets": assets,
                "task": task,
                "snapshot": snapshot,
            }),
        )
    }

    fn software_contracts_response(
        &self,
        request: &RuntimeHttpRequest,
    ) -> Result<RuntimeHttpResponse> {
        let adapter_id = request
            .query
            .get("adapter_id")
            .or_else(|| request.query.get("adapter"))
            .or_else(|| request.query.get("software"))
            .map(String::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty());
        match software_control_contracts_resource(adapter_id) {
            Ok(value) => RuntimeHttpResponse::json(200, value),
            Err(error) => RuntimeHttpResponse::json(
                404,
                json!({
                    "error": "software_contract_not_found",
                    "adapter_id": adapter_id,
                    "message": error.to_string(),
                }),
            ),
        }
    }

    fn unreal_mcp_bridge_response(&self) -> Result<RuntimeHttpResponse> {
        RuntimeHttpResponse::json(200, unreal_mcp_bridge_contract_resource())
    }

    fn adapter_health_response(&self, body: &str) -> Result<RuntimeHttpResponse> {
        let request = if body.trim().is_empty() {
            CheckAdapterHealthRequest::default()
        } else {
            match serde_json::from_str::<CheckAdapterHealthRequest>(body) {
                Ok(request) => request,
                Err(error) => {
                    return RuntimeHttpResponse::json(
                        400,
                        json!({
                            "error": "invalid_adapter_health_request",
                            "message": error.to_string(),
                        }),
                    );
                }
            }
        };

        let provider_checks = if request.include_providers.unwrap_or(true) {
            request
                .providers
                .unwrap_or_else(default_provider_health_checks)
        } else {
            Vec::new()
        };
        let software_checks = if request.include_software.unwrap_or(true) {
            request
                .software_adapters
                .unwrap_or_else(default_software_health_checks)
        } else {
            Vec::new()
        };

        let mut providers = Vec::new();
        for check in provider_checks {
            if check.provider_id.trim().is_empty() {
                providers.push(json!({
                    "provider_id": check.provider_id,
                    "status_code": 400,
                    "ok": false,
                    "error": "missing_provider_id",
                }));
                continue;
            }

            let provider_id = canonical_provider_id(&check.provider_id);
            let execution_mode = check
                .execution_mode
                .unwrap_or_else(|| default_provider_health_mode(&provider_id));
            let dispatch_options = ProviderRunDispatchOptions {
                endpoint: check.endpoint.clone(),
                api_key: check.api_key.clone(),
            };
            let response =
                self.dispatch_provider_health(&provider_id, execution_mode, dispatch_options)?;
            providers.push(health_item_value(response)?);
        }

        let mut software_adapters = Vec::new();
        for check in software_checks {
            if check.adapter_id.trim().is_empty() {
                software_adapters.push(json!({
                    "adapter_id": check.adapter_id,
                    "status_code": 400,
                    "ok": false,
                    "error": "missing_adapter_id",
                }));
                continue;
            }

            let action = SoftwareControlAction {
                adapter_id: check.adapter_id,
                action_kind: SoftwareActionKind::HealthCheck,
                priority: check.priority.unwrap_or(ControlPriority::ApiMcp),
                payload_json: check.payload_json.unwrap_or_else(|| json!({})),
                requires_confirmation: false,
            };
            let response = self.dispatch_software_health(action)?;
            software_adapters.push(health_item_value(response)?);
        }

        let providers_ready = providers
            .iter()
            .filter(|item| provider_health_item_ready(item))
            .count();
        let software_ready = software_adapters
            .iter()
            .filter(|item| software_health_item_ready(item))
            .count();
        let failed = providers.len() + software_adapters.len() - providers_ready - software_ready;

        RuntimeHttpResponse::json(
            200,
            json!({
                "providers": providers,
                "software_adapters": software_adapters,
                "summary": {
                    "providers_total": providers.len(),
                    "providers_ready": providers_ready,
                    "software_total": software_adapters.len(),
                    "software_ready": software_ready,
                    "failed": failed,
                },
            }),
        )
    }

    fn mcp_response(&self, request: &RuntimeHttpRequest) -> Result<RuntimeHttpResponse> {
        let Some(uri) = request.query.get("uri") else {
            return RuntimeHttpResponse::json(
                400,
                json!({
                    "error": "missing_uri",
                    "expected": "/api/mcp?uri=pool://tasks",
                }),
            );
        };

        let snapshot = self.load_snapshot_for_request(request)?;
        let server = McpServer::from_snapshot(snapshot);
        let payload = server.read_resource(uri)?;

        Ok(RuntimeHttpResponse {
            status_code: 200,
            content_type: "application/json; charset=utf-8".to_string(),
            body: payload,
        })
    }

    fn approve_task_response(&self, request: &RuntimeHttpRequest) -> Result<RuntimeHttpResponse> {
        let Some(task_id) = request.query.get("task_id") else {
            return RuntimeHttpResponse::json(
                400,
                json!({
                    "error": "missing_task_id",
                    "expected": "/api/tasks/approve?task_id=<task-id>",
                }),
            );
        };

        let repository = RuntimeRepository::open(&self.config.db_path)?;
        repository.migrate()?;
        let task = match repository.approve_task(task_id) {
            Ok(task) => task,
            Err(error) => {
                return RuntimeHttpResponse::json(
                    409,
                    json!({
                        "error": "approval_rejected",
                        "message": error.to_string(),
                    }),
                );
            }
        };
        if let Some(provider_request) = repository.latest_provider_request(task_id)? {
            drop(repository);
            return self.resume_provider_request_response(provider_request, true);
        }
        if let Some(software_action) = repository.latest_software_action(task_id)? {
            let action: SoftwareControlAction =
                serde_json::from_value(software_action.command_json.clone())
                    .context("parse software action command ledger")?;
            if action.requires_confirmation {
                drop(repository);
                return self.resume_software_action_response(task_id, software_action, true);
            }
        }
        if let Some(response) = self.resume_agent_session_task_response(
            &repository,
            task_id,
            task.provider_id.as_deref(),
            task.request_metadata_path.as_deref(),
            "approval",
        )? {
            return Ok(response);
        }
        let snapshot = repository.snapshot(self.config.project_slug.as_deref())?;

        RuntimeHttpResponse::json(
            200,
            json!({
                "task": task,
                "snapshot": snapshot,
            }),
        )
    }

    fn cancel_task_response(&self, request: &RuntimeHttpRequest) -> Result<RuntimeHttpResponse> {
        let Some(task_id) = request.query.get("task_id") else {
            return RuntimeHttpResponse::json(
                400,
                json!({
                    "error": "missing_task_id",
                    "expected": "/api/tasks/cancel?task_id=<task-id>",
                }),
            );
        };

        let repository = RuntimeRepository::open(&self.config.db_path)?;
        repository.migrate()?;
        let current = match repository.task_snapshot(task_id) {
            Ok(task) => task,
            Err(error) => {
                return RuntimeHttpResponse::json(
                    404,
                    json!({
                        "error": "task_not_found",
                        "message": error.to_string(),
                    }),
                );
            }
        };

        if current.status == "Succeeded" {
            return RuntimeHttpResponse::json(
                409,
                json!({
                    "error": "task_cancel_rejected",
                    "message": "succeeded tasks cannot be cancelled",
                    "task": current,
                }),
            );
        }

        repository.update_task_status(task_id, TaskStatus::Cancelled)?;
        repository.insert_event(&RuntimeEvent::new(
            current.project_slug.clone(),
            RuntimeEventLevel::Warn,
            format!("cancelled task: {}", current.title),
        ))?;
        let task = repository.task_snapshot(task_id)?;
        let snapshot = repository.snapshot(self.config.project_slug.as_deref())?;

        RuntimeHttpResponse::json(
            200,
            json!({
                "task": task,
                "snapshot": snapshot,
            }),
        )
    }

    fn retry_task_response(&self, request: &RuntimeHttpRequest) -> Result<RuntimeHttpResponse> {
        let Some(task_id) = request.query.get("task_id") else {
            return RuntimeHttpResponse::json(
                400,
                json!({
                    "error": "missing_task_id",
                    "expected": "/api/tasks/retry?task_id=<task-id>",
                }),
            );
        };

        let repository = RuntimeRepository::open(&self.config.db_path)?;
        repository.migrate()?;
        let current = match repository.task_snapshot(task_id) {
            Ok(task) => task,
            Err(error) => {
                return RuntimeHttpResponse::json(
                    404,
                    json!({
                        "error": "task_not_found",
                        "message": error.to_string(),
                    }),
                );
            }
        };

        if !matches!(
            current.status.as_str(),
            "Failed" | "Retryable" | "Cancelled"
        ) {
            return RuntimeHttpResponse::json(
                409,
                json!({
                    "error": "task_retry_rejected",
                    "message": "only failed, retryable, or cancelled tasks can be retried",
                    "task": current,
                }),
            );
        }

        repository.update_task_status(task_id, TaskStatus::Ready)?;
        repository.insert_event(&RuntimeEvent::new(
            current.project_slug.clone(),
            RuntimeEventLevel::Info,
            format!("retried task: {}", current.title),
        ))?;
        if let Some(provider_request) = repository.latest_provider_request(task_id)? {
            drop(repository);
            return self.resume_provider_request_response(provider_request, false);
        }
        if let Some(software_action) = repository.latest_software_action(task_id)? {
            drop(repository);
            return self.resume_software_action_response(task_id, software_action, false);
        }
        if let Some(response) = self.resume_agent_session_task_response(
            &repository,
            task_id,
            current.provider_id.as_deref(),
            current.request_metadata_path.as_deref(),
            "retry",
        )? {
            return Ok(response);
        }
        let task = repository.task_snapshot(task_id)?;
        let snapshot = repository.snapshot(self.config.project_slug.as_deref())?;

        RuntimeHttpResponse::json(
            200,
            json!({
                "task": task,
                "snapshot": snapshot,
            }),
        )
    }

    fn resume_provider_request_response(
        &self,
        provider_request: ProviderRequestRecord,
        reuse_provider_request: bool,
    ) -> Result<RuntimeHttpResponse> {
        let provider_id = provider_request.provider_id.clone();
        let execution_mode = provider_request
            .request_json
            .get("execution_mode")
            .cloned()
            .map(serde_json::from_value::<ProviderRunExecutionMode>)
            .transpose()
            .context("parse provider run execution mode from ledger")?
            .unwrap_or(ProviderRunExecutionMode::Auto);
        let endpoint = provider_request
            .request_json
            .get("endpoint")
            .and_then(Value::as_str)
            .filter(|endpoint| !endpoint.trim().is_empty())
            .map(ToString::to_string);
        let mut request = provider_request
            .request_json
            .get("provider_request")
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("provider request ledger missing provider_request"))?;
        if let Some(object) = request.as_object_mut() {
            object.insert("require_approval".to_string(), Value::Bool(false));
        }
        let request: ProviderRequest =
            serde_json::from_value(request).context("parse provider request ledger")?;

        let repository = RuntimeRepository::open(&self.config.db_path)?;
        repository.migrate()?;
        let mut task = repository.runtime_task(&provider_request.task_id)?;
        task.status = TaskStatus::Ready;
        task.provider_id = Some(provider_id.clone());
        drop(repository);
        let provider_request_id = provider_request.id.clone();

        let inputs = ProviderRunInputs {
            task,
            request,
            execution_mode,
            endpoint: endpoint.clone(),
            control_context: provider_request
                .request_json
                .get("control_context")
                .cloned(),
            evidence_json: provider_request.request_json.get("evidence").cloned(),
            inline_api_key_provided: false,
            cost_estimate_explicit: true,
            requires_approval_explicit: true,
            resume_provider_request_id: reuse_provider_request
                .then_some(provider_request_id.clone()),
            retry_of_provider_request_id: (!reuse_provider_request).then_some(provider_request_id),
        };
        let dispatch_options = ProviderRunDispatchOptions {
            endpoint,
            api_key: None,
        };

        self.dispatch_provider_run(&provider_id, execution_mode, dispatch_options, inputs)
    }

    fn resume_software_action_response(
        &self,
        task_id: &str,
        software_action: SoftwareActionRecord,
        clear_confirmation: bool,
    ) -> Result<RuntimeHttpResponse> {
        let mut action: SoftwareControlAction =
            serde_json::from_value(software_action.command_json.clone())
                .context("parse software action command ledger")?;
        if clear_confirmation {
            action.requires_confirmation = false;
        }

        let repository = RuntimeRepository::open(&self.config.db_path)?;
        repository.migrate()?;
        let mut task = repository.runtime_task(task_id)?;
        task.status = TaskStatus::Ready;
        task.requires_approval = false;
        task.provider_id = Some(action.adapter_id.clone());
        let project_slug = task.project_slug.clone();

        self.dispatch_software_action_response(&repository, &project_slug, task, action)
    }

    fn resume_agent_session_task_response(
        &self,
        repository: &RuntimeRepository,
        task_id: &str,
        provider_id: Option<&str>,
        transcript_path: Option<&str>,
        resume_reason: &str,
    ) -> Result<Option<RuntimeHttpResponse>> {
        if !matches!(provider_id, Some("hermes" | "agent-cli")) {
            return Ok(None);
        }
        let Some(transcript_path) = transcript_path else {
            return Ok(None);
        };
        let runner = AgentSessionRunner::new(repository);
        let Some(report) = runner.resume_transcript_execution(
            task_id,
            transcript_path,
            HermesExecutionOptions {
                auth_token: agent_auth_token(repository, "hermes")?,
                ..HermesExecutionOptions::default()
            },
            resume_reason,
        )?
        else {
            return Ok(None);
        };
        let task = repository.task_snapshot(&report.task_id)?;
        let snapshot = repository.snapshot(self.config.project_slug.as_deref())?;

        RuntimeHttpResponse::json(
            200,
            json!({
                "report": report,
                "task": task,
                "snapshot": snapshot,
            }),
        )
        .map(Some)
    }

    fn load_snapshot_for_request(
        &self,
        request: &RuntimeHttpRequest,
    ) -> Result<crate::RuntimeSnapshot> {
        let repository = RuntimeRepository::open(&self.config.db_path)?;
        repository.migrate()?;
        let project_filter = match project_filter_for_request(request) {
            Some(ProjectFilterOverride::All) => None,
            Some(ProjectFilterOverride::Slug(slug)) => Some(slug),
            None => self.config.project_slug.as_deref(),
        };
        repository.snapshot(project_filter)
    }

    fn default_control_dir(&self, project_slug: &str) -> PathBuf {
        self.config
            .db_path
            .parent()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
            .join("worlds")
            .join(project_slug)
            .join("output")
            .join("control")
    }

    fn default_output_dir(&self, project_slug: &str) -> PathBuf {
        self.default_output_root()
            .join("worlds")
            .join(project_slug)
            .join("output")
    }

    fn default_output_root(&self) -> PathBuf {
        self.config
            .db_path
            .parent()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
    }

    fn runtime_base_url(&self) -> String {
        format!("http://{}", self.config.bind_addr)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeHttpResponse {
    pub status_code: u16,
    pub content_type: String,
    pub body: String,
}

impl RuntimeHttpResponse {
    pub fn empty(status_code: u16) -> Self {
        Self {
            status_code,
            content_type: "text/plain; charset=utf-8".to_string(),
            body: String::new(),
        }
    }

    pub fn json(status_code: u16, value: impl Serialize) -> Result<Self> {
        Ok(Self {
            status_code,
            content_type: "application/json; charset=utf-8".to_string(),
            body: serde_json::to_string_pretty(&value)?,
        })
    }

    pub fn text(
        status_code: u16,
        content_type: impl Into<String>,
        body: impl Into<String>,
    ) -> Self {
        Self {
            status_code,
            content_type: content_type.into(),
            body: body.into(),
        }
    }

    pub fn from_error(error: anyhow::Error) -> Self {
        Self::json(
            500,
            json!({
                "error": "runtime_http_error",
                "message": error.to_string(),
            }),
        )
        .unwrap_or_else(|_| Self {
            status_code: 500,
            content_type: "application/json; charset=utf-8".to_string(),
            body: "{\"error\":\"runtime_http_error\"}".to_string(),
        })
    }

    pub fn to_http_bytes(&self) -> String {
        format!(
            "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type\r\nConnection: close\r\n\r\n{}",
            self.status_code,
            status_text(self.status_code),
            self.content_type,
            self.body.as_bytes().len(),
            self.body
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimeHttpRequestLine {
    method: String,
    path: String,
    headers: BTreeMap<String, String>,
    body: String,
}

#[derive(Debug, Clone, Deserialize)]
struct CreateTaskRequest {
    project_slug: Option<String>,
    node_id: Option<String>,
    title: String,
    provider_id: Option<String>,
    cost_estimate_tokens: Option<u64>,
    requires_approval: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
struct CreateProviderRunRequest {
    project_slug: Option<String>,
    node_id: Option<String>,
    task_title: Option<String>,
    provider_id: String,
    execution_mode: Option<ProviderRunExecutionMode>,
    endpoint: Option<String>,
    api_key: Option<String>,
    prompt: Option<String>,
    input_paths: Option<Vec<String>>,
    output_dir: Option<String>,
    cost_estimate_tokens: Option<u64>,
    requires_approval: Option<bool>,
    evidence_json: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
struct CheckProviderHealthRequest {
    provider_id: String,
    execution_mode: Option<ProviderRunExecutionMode>,
    endpoint: Option<String>,
    api_key: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct CheckAdapterHealthRequest {
    providers: Option<Vec<AdapterProviderHealthCheck>>,
    software_adapters: Option<Vec<AdapterSoftwareHealthCheck>>,
    include_providers: Option<bool>,
    include_software: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
struct AdapterProviderHealthCheck {
    provider_id: String,
    execution_mode: Option<ProviderRunExecutionMode>,
    endpoint: Option<String>,
    api_key: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct AdapterSoftwareHealthCheck {
    adapter_id: String,
    priority: Option<ControlPriority>,
    payload_json: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
struct RunNodeRequest {
    project_slug: Option<String>,
    node_id: String,
    prompt: Option<String>,
    execution_mode: Option<ProviderRunExecutionMode>,
    endpoint: Option<String>,
    api_key: Option<String>,
    input_paths: Option<Vec<String>>,
    output_dir: Option<String>,
    duration_ms: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
struct CreateWorkflowRunRequest {
    project_slug: Option<String>,
    output_root: Option<String>,
    title: Option<String>,
    prompt: Option<String>,
    source_inputs: Option<Vec<String>>,
    duration_ms: Option<u64>,
    three_dgs_mode: Option<ContentBurstProviderMode>,
    three_dgs_provider_id: Option<String>,
    three_dgs_endpoint: Option<String>,
    three_dgs_api_key: Option<String>,
    unreal_mode: Option<ContentBurstSoftwareMode>,
    unreal_endpoint: Option<String>,
    unreal_auth_token: Option<String>,
    agent_mode: Option<ContentBurstAgentMode>,
    hermes_endpoint: Option<String>,
    hermes_auth_token: Option<String>,
    agent_requires_confirmation: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
struct CreateOutputPackageRequest {
    project_slug: Option<String>,
    node_id: Option<String>,
    output_dir: Option<String>,
    title: Option<String>,
    source_assets: Option<Vec<String>>,
    duration_ms: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
struct RecordOutputPackageResultRequest {
    project_slug: Option<String>,
    node_id: Option<String>,
    target: String,
    local_path: Option<String>,
    status: String,
    runtime: Option<String>,
    adapter_id: Option<String>,
    software_action_id: Option<String>,
    message: Option<String>,
    artifacts: Option<Vec<String>>,
    metrics: Option<Vec<OutputManifestMetric>>,
    verification: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
struct CreateHandoffPackageRequest {
    project_slug: Option<String>,
    node_id: Option<String>,
    output_dir: Option<String>,
    title: Option<String>,
    include_snapshot: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
struct CreateProviderConformancePackageRequest {
    project_slug: Option<String>,
    node_id: Option<String>,
    provider_id: String,
    output_dir: Option<String>,
    title: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct CreateIntegrationConformancePackageRequest {
    project_slug: Option<String>,
    node_id: Option<String>,
    output_dir: Option<String>,
    title: Option<String>,
    providers: Option<Vec<String>>,
    software_adapters: Option<Vec<String>>,
    agent_kind: Option<String>,
    include_providers: Option<bool>,
    include_software: Option<bool>,
    include_agent: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
struct CreateSoftwareConformancePackageRequest {
    project_slug: Option<String>,
    node_id: Option<String>,
    adapter_id: String,
    output_dir: Option<String>,
    title: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct CreateProductionEvidenceHandoffPackageRequest {
    project_slug: Option<String>,
    node_id: Option<String>,
    output_dir: Option<String>,
    output_root: Option<String>,
    source: Option<String>,
    title: Option<String>,
    include_items: Option<bool>,
    include_snapshot: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
struct ClaimProductionEvidenceTaskRequest {
    project_slug: Option<String>,
    task_id: String,
    assignee: Option<String>,
    role: Option<String>,
    output_root: Option<String>,
    source: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct CreatePrdCompletionPackageRequest {
    project_slug: Option<String>,
    node_id: Option<String>,
    output_dir: Option<String>,
    title: Option<String>,
    source: Option<String>,
    include_snapshot: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
struct CreateApiKeyRequest {
    project_slug: Option<String>,
    provider_id: Option<String>,
    provider: Option<String>,
    service_type: Option<String>,
    api_key: String,
    metadata: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
struct CreateAgentSessionRequest {
    kind: AgentSessionRequestKind,
    project_slug: Option<String>,
    control_dir: Option<String>,
    endpoint: Option<String>,
    instruction: Option<String>,
    allowed_tools: Option<Vec<String>>,
    requires_confirmation: Option<bool>,
    command_id: Option<String>,
    title: Option<String>,
    command: Option<String>,
    tools: Option<Vec<String>>,
    token_budget: Option<u64>,
    execute: Option<bool>,
    allowed_commands: Option<Vec<String>>,
    working_dir: Option<String>,
    max_output_bytes: Option<usize>,
    timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
struct CreateAgentConformancePackageRequest {
    project_slug: Option<String>,
    node_id: Option<String>,
    kind: Option<String>,
    output_dir: Option<String>,
    title: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum AgentSessionRequestKind {
    Hermes,
    AgentCli,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum ProviderRunExecutionMode {
    Auto,
    Mock,
    Adapter,
    Gateway,
}

struct ProviderRunInputs {
    task: RuntimeTask,
    request: ProviderRequest,
    execution_mode: ProviderRunExecutionMode,
    endpoint: Option<String>,
    control_context: Option<Value>,
    evidence_json: Option<Value>,
    inline_api_key_provided: bool,
    cost_estimate_explicit: bool,
    requires_approval_explicit: bool,
    resume_provider_request_id: Option<String>,
    retry_of_provider_request_id: Option<String>,
}

#[derive(Debug, Clone)]
struct ProviderRunDispatchOptions {
    endpoint: Option<String>,
    api_key: Option<String>,
}

#[derive(Debug, Clone)]
struct RuntimeWorkflowNodeRef {
    id: String,
    project_slug: String,
    title: String,
    node_type: String,
    provider_id: Option<String>,
    software_adapter_id: Option<String>,
    requires_approval: bool,
    cost_estimate_tokens: u64,
    parameters: Value,
}

#[derive(Debug, Clone, Deserialize)]
struct CreateSoftwareActionRequest {
    project_slug: Option<String>,
    node_id: Option<String>,
    task_title: Option<String>,
    adapter_id: String,
    action_kind: Option<SoftwareActionKind>,
    priority: Option<ControlPriority>,
    payload_json: Option<serde_json::Value>,
    evidence_json: Option<serde_json::Value>,
    requires_confirmation: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct ImportProductionEvidenceRequest {
    project_slug: Option<String>,
    source: Option<String>,
    providers: Option<Vec<ProviderProductionEvidenceItem>>,
    software_actions: Option<Vec<SoftwareProductionEvidenceItem>>,
    desktop_vision: Option<Vec<DesktopVisionProductionEvidenceItem>>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct MergeProductionEvidenceRequest {
    project_slug: Option<String>,
    source: Option<String>,
    bundles: Vec<ImportProductionEvidenceRequest>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct CloseoutProductionEvidenceRequest {
    project_slug: Option<String>,
    source: Option<String>,
    import: Option<bool>,
    bundles: Vec<ImportProductionEvidenceRequest>,
    completion_package: Option<CloseoutCompletionPackageRequest>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct CloseoutCompletionPackageRequest {
    node_id: Option<String>,
    title: Option<String>,
    output_dir: Option<String>,
    source: Option<String>,
    include_snapshot: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
struct SubmitProductionEvidenceItemRequest {
    project_slug: Option<String>,
    source: Option<String>,
    kind: String,
    provider: Option<ProviderProductionEvidenceItem>,
    software_action: Option<SoftwareProductionEvidenceItem>,
    desktop_vision: Option<DesktopVisionProductionEvidenceItem>,
}

impl SubmitProductionEvidenceItemRequest {
    fn into_import_request(self) -> Result<ImportProductionEvidenceRequest> {
        let kind = self.kind.trim().to_ascii_lowercase();
        let mut request = ImportProductionEvidenceRequest {
            project_slug: self.project_slug,
            source: self.source,
            providers: None,
            software_actions: None,
            desktop_vision: None,
        };
        match kind.as_str() {
            "provider" | "provider_production_upstream" | "providers" => {
                let item = self
                    .provider
                    .context("production evidence item kind provider requires provider")?;
                request.providers = Some(vec![item]);
            }
            "software" | "software_action" | "software_production" => {
                let item = self.software_action.context(
                    "production evidence item kind software_action requires software_action",
                )?;
                request.software_actions = Some(vec![item]);
            }
            "desktop_vision" | "desktop" | "vision" => {
                let item = self.desktop_vision.context(
                    "production evidence item kind desktop_vision requires desktop_vision",
                )?;
                request.desktop_vision = Some(vec![item]);
            }
            _ => bail!(
                "production evidence item kind must be provider, software_action, or desktop_vision"
            ),
        }
        Ok(request)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProviderProductionEvidenceItem {
    provider_id: String,
    external_job_id: String,
    endpoint: Option<String>,
    family: Option<String>,
    production_attestation: Option<String>,
    node_id: Option<String>,
    task_title: Option<String>,
    metadata_path: Option<String>,
    artifacts: Option<Vec<String>>,
    evidence_json: Option<Value>,
    response_json: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SoftwareProductionEvidenceItem {
    adapter_id: String,
    external_action_id: String,
    production_attestation: Option<String>,
    action_kind: Option<SoftwareActionKind>,
    priority: Option<ControlPriority>,
    control_profile: Option<String>,
    node_id: Option<String>,
    task_title: Option<String>,
    artifacts: Option<Vec<String>>,
    evidence_json: Option<Value>,
    verification_json: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DesktopVisionProductionEvidenceItem {
    adapter_id: Option<String>,
    external_action_id: String,
    controller_id: String,
    production_attestation: Option<String>,
    trace_path: String,
    visual_model: Option<String>,
    node_id: Option<String>,
    task_title: Option<String>,
    artifacts: Option<Vec<String>>,
    evidence_json: Option<Value>,
    verification_json: Option<Value>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct RuntimeExecutionPlanRunNextRequest {
    project_slug: Option<String>,
    node_id: Option<String>,
    task_id: Option<String>,
    execute: Option<bool>,
    allow_approval: Option<bool>,
    execution_mode: Option<ProviderRunExecutionMode>,
    endpoint: Option<String>,
    api_key: Option<String>,
    prompt: Option<String>,
    input_paths: Option<Vec<String>>,
    output_dir: Option<String>,
    duration_ms: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
struct CheckSoftwareHealthRequest {
    adapter_id: String,
    priority: Option<ControlPriority>,
    payload_json: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
struct DesktopRecognitionResultRequest {
    software_action_id: Option<String>,
    action_id: Option<String>,
    task_id: Option<String>,
    status: Option<String>,
    message: Option<String>,
    artifacts: Option<Vec<String>>,
    screen_trace_path: Option<String>,
    result: Option<Value>,
    verification: Option<Value>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct DesktopRecognitionRunNextRequest {
    status: Option<String>,
    message: Option<String>,
    controller_id: Option<String>,
    limit: Option<usize>,
    artifacts: Option<Vec<String>>,
    screen_trace_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct RuntimeHttpRequest {
    path: String,
    query: BTreeMap<String, String>,
}

impl RuntimeHttpRequest {
    fn parse(path_and_query: &str) -> Result<Self> {
        let (raw_path, raw_query) = path_and_query
            .split_once('?')
            .map(|(path, query)| (path, Some(query)))
            .unwrap_or((path_and_query, None));

        let path = percent_decode(raw_path)?;
        let mut query = BTreeMap::new();
        if let Some(raw_query) = raw_query {
            for pair in raw_query.split('&').filter(|pair| !pair.is_empty()) {
                let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
                query.insert(percent_decode(key)?, percent_decode(value)?);
            }
        }

        Ok(Self { path, query })
    }
}

fn read_http_request(stream: &mut impl Read) -> Result<RuntimeHttpRequestLine> {
    let mut buffer = [0_u8; 8192];
    let bytes_read = stream.read(&mut buffer).context("read HTTP request")?;
    let mut request_bytes = buffer[..bytes_read].to_vec();
    let mut headers_end = find_headers_end(&request_bytes);
    let mut content_length = headers_end
        .and_then(|end| parse_content_length(&request_bytes[..end]))
        .unwrap_or(0);

    while headers_end.is_none() || request_body_len(&request_bytes, headers_end) < content_length {
        let mut chunk = [0_u8; 8192];
        let read = stream.read(&mut chunk).context("read HTTP request body")?;
        if read == 0 {
            break;
        }
        request_bytes.extend_from_slice(&chunk[..read]);
        headers_end = find_headers_end(&request_bytes);
        content_length = headers_end
            .and_then(|end| parse_content_length(&request_bytes[..end]))
            .unwrap_or(content_length);
    }

    let request = std::str::from_utf8(&request_bytes).context("parse HTTP request bytes")?;
    let first_line = request.lines().next().unwrap_or_default();
    let mut parts = first_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let path = parts.next().unwrap_or_default();

    if method.is_empty() {
        bail!("missing HTTP request method");
    }
    if !matches!(method, "GET" | "POST" | "OPTIONS") {
        bail!("unsupported HTTP method: {method}");
    }
    if path.is_empty() {
        bail!("missing HTTP request path");
    }

    Ok(RuntimeHttpRequestLine {
        method: method.to_string(),
        path: path.to_string(),
        headers: headers_end
            .map(|end| parse_http_headers(&request_bytes[..end]))
            .unwrap_or_default(),
        body: extract_body(&request_bytes, headers_end).unwrap_or_default(),
    })
}

fn find_headers_end(bytes: &[u8]) -> Option<usize> {
    bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|index| index + 4)
}

fn parse_content_length(headers: &[u8]) -> Option<usize> {
    let headers = std::str::from_utf8(headers).ok()?;
    headers.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        if name.eq_ignore_ascii_case("content-length") {
            value.trim().parse::<usize>().ok()
        } else {
            None
        }
    })
}

fn parse_http_headers(headers: &[u8]) -> BTreeMap<String, String> {
    let Ok(headers) = std::str::from_utf8(headers) else {
        return BTreeMap::new();
    };
    headers
        .lines()
        .skip(1)
        .filter_map(|line| {
            let (name, value) = line.split_once(':')?;
            Some((name.trim().to_ascii_lowercase(), value.trim().to_string()))
        })
        .collect()
}

fn request_body_len(bytes: &[u8], headers_end: Option<usize>) -> usize {
    headers_end
        .map(|end| bytes.len().saturating_sub(end))
        .unwrap_or_default()
}

fn extract_body(bytes: &[u8], headers_end: Option<usize>) -> Option<String> {
    let start = headers_end?;
    String::from_utf8(bytes[start..].to_vec()).ok()
}

fn percent_decode(input: &str) -> Result<String> {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars();

    while let Some(char) = chars.next() {
        if char == '%' {
            let high = chars.next().context("incomplete percent escape")?;
            let low = chars.next().context("incomplete percent escape")?;
            let high = high.to_digit(16).context("invalid percent escape")?;
            let low = low.to_digit(16).context("invalid percent escape")?;
            let value = ((high << 4) | low) as u8;
            output.push(value as char);
        } else if char == '+' {
            output.push(' ');
        } else {
            output.push(char);
        }
    }

    Ok(output)
}

fn status_text(status_code: u16) -> &'static str {
    match status_code {
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        204 => "No Content",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        426 => "Upgrade Required",
        501 => "Not Implemented",
        502 => "Bad Gateway",
        500 => "Internal Server Error",
        _ => "OK",
    }
}

fn canonical_provider_id(provider_id: &str) -> String {
    provider_aliases()
        .get(provider_id)
        .copied()
        .unwrap_or(provider_id)
        .to_string()
}

fn provider_aliases() -> BTreeMap<&'static str, &'static str> {
    BTreeMap::from([
        ("world-labs-marble", "worldlabs-marble"),
        ("triposplat", "tripo-splat"),
        ("spark", "spark-3dgs"),
        ("qunhe", "qunhe-3d"),
        ("openai", "openai-image-2"),
        ("openai-image", "openai-image-2"),
        ("image-2", "openai-image-2"),
        ("mj", "midjourney"),
        ("nano-banana", "nano-banana-pro"),
        ("nanobanana", "nano-banana-pro"),
        ("nanobananapro", "nano-banana-pro"),
    ])
}

fn default_provider_health_checks() -> Vec<AdapterProviderHealthCheck> {
    ProviderRegistry::defaults()
        .configs()
        .into_iter()
        .map(|config| AdapterProviderHealthCheck {
            provider_id: config.id.clone(),
            execution_mode: Some(default_provider_health_mode(&config.id)),
            endpoint: None,
            api_key: None,
        })
        .collect()
}

fn default_software_health_checks() -> Vec<AdapterSoftwareHealthCheck> {
    SoftwareAdapterRegistry::defaults()
        .configs()
        .into_iter()
        .map(|config| AdapterSoftwareHealthCheck {
            adapter_id: config.id.clone(),
            priority: Some(ControlPriority::ApiMcp),
            payload_json: None,
        })
        .collect()
}

fn default_provider_health_mode(provider_id: &str) -> ProviderRunExecutionMode {
    if is_three_dgs_provider(provider_id) {
        ProviderRunExecutionMode::Auto
    } else {
        ProviderRunExecutionMode::Adapter
    }
}

fn health_item_value(response: RuntimeHttpResponse) -> Result<Value> {
    let status_code = response.status_code;
    let mut value: Value = serde_json::from_str(&response.body).context("parse health response")?;
    if let Value::Object(map) = &mut value {
        map.insert("status_code".to_string(), json!(status_code));
        map.insert("http_ok".to_string(), json!(status_code < 400));
    }
    Ok(value)
}

fn provider_health_item_ready(value: &Value) -> bool {
    value
        .get("status_code")
        .and_then(Value::as_u64)
        .is_some_and(|status| status < 400)
        && value
            .pointer("/health/status")
            .and_then(Value::as_str)
            .is_some_and(|status| status.eq_ignore_ascii_case("ready"))
}

fn software_health_item_ready(value: &Value) -> bool {
    value
        .get("status_code")
        .and_then(Value::as_u64)
        .is_some_and(|status| status < 400)
        && value
            .pointer("/health/ok")
            .and_then(Value::as_bool)
            .unwrap_or(false)
}

fn is_three_dgs_provider(provider_id: &str) -> bool {
    matches!(
        provider_id,
        "mock-3dgs" | "worldlabs-marble" | "tripo-splat" | "sam-3d" | "spark-3dgs" | "qunhe-3d"
    )
}

fn is_http_media_provider(provider_id: &str) -> bool {
    matches!(provider_id, "midjourney" | "nano-banana-pro" | "suno")
}

fn http_media_provider_defaults(
    provider_id: &str,
) -> Option<(
    &'static str,
    ProviderKind,
    Option<&'static str>,
    &'static str,
    bool,
)> {
    match provider_id {
        "midjourney" => Some((
            "Midjourney",
            ProviderKind::AiImage,
            Some("POOL_MIDJOURNEY_API_KEY"),
            "png",
            false,
        )),
        "nano-banana-pro" => Some((
            "Nano Banana Pro",
            ProviderKind::AiImage,
            Some("POOL_NANO_BANANA_PRO_KEY"),
            "png",
            false,
        )),
        "suno" => Some((
            "Suno",
            ProviderKind::Audio,
            Some("POOL_SUNO_API_KEY"),
            "mp3",
            false,
        )),
        _ => None,
    }
}

fn should_use_mock_3dgs(provider_id: &str, execution_mode: ProviderRunExecutionMode) -> bool {
    if !is_three_dgs_provider(provider_id) {
        return false;
    }
    match execution_mode {
        ProviderRunExecutionMode::Mock => true,
        ProviderRunExecutionMode::Adapter | ProviderRunExecutionMode::Gateway => false,
        ProviderRunExecutionMode::Auto => !three_dgs_gateway_configured(provider_id),
    }
}

fn three_dgs_gateway_configured(provider_id: &str) -> bool {
    let prefix = provider_env_prefix(provider_id);
    env_has_value("POOL_3DGS_GATEWAY_ENDPOINT") || env_has_value(&format!("POOL_{prefix}_ENDPOINT"))
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

fn env_has_value(key: &str) -> bool {
    env::var(key).is_ok_and(|value| !value.trim().is_empty())
}

fn display_name_for_provider(provider_id: &str) -> String {
    match provider_id {
        "mock-3dgs" => "Mock 3DGS",
        "worldlabs-marble" => "World Labs Marble",
        "tripo-splat" => "TripoSplat",
        "sam-3d" => "SAM-3D",
        "spark-3dgs" => "Spark 3DGS",
        "qunhe-3d" => "Qunhe 3D",
        "midjourney" => "Midjourney",
        "nano-banana-pro" => "Nano Banana Pro",
        "suno" => "Suno",
        other => other,
    }
    .to_string()
}

fn runtime_node_from_snapshot(
    snapshot: &RuntimeSnapshot,
    node_id: &str,
    project_slug: &str,
) -> Option<RuntimeWorkflowNodeRef> {
    snapshot.workflows.iter().find_map(|workflow| {
        let nodes = workflow.nodes.as_object()?;
        let node = nodes.get(node_id)?;
        Some(RuntimeWorkflowNodeRef {
            id: node_value_string(node, "id").unwrap_or_else(|| node_id.to_string()),
            project_slug: project_slug.to_string(),
            title: node_value_string(node, "title").unwrap_or_else(|| node_id.to_string()),
            node_type: node_value_string(node, "node_type")
                .unwrap_or_else(|| "Runtime".to_string()),
            provider_id: node_value_string(node, "provider_id"),
            software_adapter_id: node_value_string(node, "software_adapter_id"),
            requires_approval: node
                .get("requires_approval")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            cost_estimate_tokens: node
                .get("cost_estimate_tokens")
                .and_then(Value::as_u64)
                .unwrap_or_default(),
            parameters: node.get("parameters").cloned().unwrap_or_else(|| json!({})),
        })
    })
}

fn node_control_context_from_snapshot(snapshot: &RuntimeSnapshot, node_id: &str) -> Value {
    runtime_node_context_resource(snapshot, node_id)
        .ok()
        .and_then(|value| value.get("control_context").cloned())
        .unwrap_or_else(|| json!({}))
}

fn node_value_string(node: &Value, key: &str) -> Option<String> {
    node.get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string)
}

fn json_value_string(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string)
}

fn json_string_array_at(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(ToString::to_string)
        .collect()
}

fn query_bool(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "y" | "on"
    )
}

fn runtime_execution_plan_selected_step(
    plan: &Value,
    request: &RuntimeExecutionPlanRunNextRequest,
) -> Option<Value> {
    let all_steps = plan
        .get("workflows")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|workflow| workflow.get("steps"))
        .filter_map(Value::as_array)
        .flatten()
        .cloned()
        .collect::<Vec<_>>();

    if let Some(node_id) = request
        .node_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return all_steps
            .into_iter()
            .find(|step| step.get("node_id").and_then(Value::as_str) == Some(node_id));
    }
    if let Some(task_id) = request
        .task_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return all_steps.into_iter().find(|step| {
            step.get("evidence")
                .and_then(|evidence| evidence.get("latest_task"))
                .and_then(|task| task.get("id"))
                .and_then(Value::as_str)
                == Some(task_id)
                || step
                    .get("gate")
                    .and_then(|gate| gate.get("task_id"))
                    .and_then(Value::as_str)
                    == Some(task_id)
        });
    }

    plan.get("next_steps")
        .and_then(Value::as_array)
        .and_then(|steps| steps.first())
        .cloned()
}

fn node_parameter_string(parameters: &Value, key: &str) -> Option<String> {
    parameters
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string)
}

fn node_parameter_string_array(parameters: &Value, key: &str) -> Option<Vec<String>> {
    let values = parameters.get(key)?.as_array()?;
    let strings = values
        .iter()
        .filter_map(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    (!strings.is_empty()).then_some(strings)
}

fn is_agent_node_type(node_type: &str) -> bool {
    matches!(node_type, "Agent" | "AgentCli" | "Hermes")
}

fn is_output_node_type(node_type: &str) -> bool {
    matches!(
        node_type,
        "VideoOutput" | "GameOutput" | "InteractiveOutput"
    )
}

fn software_action_kind_for_node(node_type: &str, adapter_id: &str) -> &'static str {
    match (node_type, adapter_id) {
        ("Unreal", _) => "CreateScene",
        ("GameOutput", _) => "ExportBuild",
        ("VideoOutput", "resolve" | "editing-suite") => "Transcode",
        ("VideoOutput", _) => "Render",
        ("InteractiveOutput", _) => "RunViewport",
        ("Blender", _) | (_, "blender") => "ImportAsset",
        ("Nuke", _) | (_, "nuke") => "Render",
        ("TouchDesigner", _) | ("MadMapper", _) => "RunViewport",
        ("MotionCaptureDb", _) | (_, "motion-db") => "ImportAsset",
        _ => "HealthCheck",
    }
}

fn default_prompt_for_provider(provider_id: &str) -> String {
    match provider_id {
        "kling" => {
            r#"{"prompt":"generate a cinematic content burst preview","duration":5,"aspect_ratio":"16:9"}"#.to_string()
        }
        "openai-image-2" => {
            "generate a concept plate for a video game interactive art scene".to_string()
        }
        "midjourney" | "nano-banana-pro" => {
            "generate a production concept image for a video game interactive art scene".to_string()
        }
        "suno" => {
            r#"{"prompt":"generate a short electronic cue for interactive art","output_slug":"suno-cue","output_extension":"mp3"}"#.to_string()
        }
        "comfyui" => "{}".to_string(),
        _ => "generate local 3DGS world package".to_string(),
    }
}

fn provider_run_ledger_json(provider_id: &str, inputs: &ProviderRunInputs) -> Value {
    json!({
        "provider_id": provider_id,
        "execution_mode": inputs.execution_mode,
        "endpoint": inputs.endpoint.clone(),
        "control_context": inputs.control_context.clone(),
        "evidence": inputs.evidence_json.clone(),
        "attempt": {
            "kind": if inputs.retry_of_provider_request_id.is_some() {
                "retry"
            } else if inputs.resume_provider_request_id.is_some() {
                "resume_existing"
            } else {
                "initial"
            },
            "retry_of_provider_request_id": inputs.retry_of_provider_request_id.clone(),
            "resume_provider_request_id": inputs.resume_provider_request_id.clone(),
        },
        "credential_source": if inputs.inline_api_key_provided {
            "inline_request_not_persisted"
        } else {
            "runtime_api_key_or_environment"
        },
        "task": {
            "id": inputs.task.id.clone(),
            "project_slug": inputs.task.project_slug.clone(),
            "node_id": inputs.task.node_id.clone(),
            "title": inputs.task.title.clone(),
            "cost_estimate_tokens": inputs.task.cost_estimate_tokens,
            "requires_approval": inputs.task.requires_approval,
            "status": inputs.task.status.clone(),
            "request_metadata_path": inputs.task.request_metadata_path.clone(),
        },
        "provider_request": inputs.request.clone(),
    })
}

fn provider_approval_handoff_path(provider_id: &str, inputs: &ProviderRunInputs) -> String {
    Path::new(&inputs.request.output_dir)
        .join(format!(
            ".0-provider-approval__{}-request.json",
            provider_metadata_slug(provider_id)
        ))
        .to_string_lossy()
        .to_string()
}

fn write_provider_approval_handoff(
    provider_id: &str,
    inputs: &ProviderRunInputs,
    ledger_json: &Value,
) -> Result<()> {
    let metadata_path = provider_approval_handoff_path(provider_id, inputs);
    let metadata_path = Path::new(&metadata_path);
    if let Some(parent) = metadata_path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!("create provider approval handoff dir {}", parent.display())
        })?;
    }
    let payload = json!({
        "kind": "pool_provider_approval_handoff",
        "status": "waiting_approval",
        "provider_id": provider_id,
        "project_slug": inputs.task.project_slug,
        "task_id": inputs.task.id,
        "task_title": inputs.task.title,
        "node_id": inputs.task.node_id,
        "cost_estimate_tokens": inputs.task.cost_estimate_tokens,
        "requires_approval": inputs.task.requires_approval,
        "output_dir": inputs.request.output_dir,
        "control_context": inputs.control_context,
        "provider_request": inputs.request,
        "ledger": ledger_json,
        "operator_next_actions": [
            "Review prompt, inputs, output_dir, provider_id, execution_mode, credential_source and cost_estimate_tokens.",
            "Approve with POST /api/tasks/approve?task_id=<task-id> or pool-cli approve-task <task-id>.",
            "Do not call the external provider before approval; this file is a local handoff package for Agent/Hermes/gateway inspection."
        ]
    });

    fs::write(
        metadata_path,
        serde_json::to_string_pretty(&payload).context("serialize provider approval handoff")?,
    )
    .with_context(|| {
        format!(
            "write provider approval handoff {}",
            metadata_path.display()
        )
    })?;
    Ok(())
}

fn provider_metadata_slug(value: &str) -> String {
    let slug = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    if slug.is_empty() {
        "provider".to_string()
    } else {
        slug
    }
}

fn provider_not_configured_response(
    provider_id: &str,
    message: &str,
) -> Result<RuntimeHttpResponse> {
    RuntimeHttpResponse::json(
        409,
        json!({
            "error": "provider_not_configured",
            "provider_id": provider_id,
            "message": message,
        }),
    )
}

fn provider_not_executable_response(provider_id: &str) -> Result<RuntimeHttpResponse> {
    RuntimeHttpResponse::json(
        501,
        json!({
            "error": "provider_not_executable",
            "provider_id": provider_id,
            "message": "Runtime HTTP does not have an executable adapter for this provider yet; use /api/tasks to stage it.",
            "supported_adapters": [
                "comfyui",
                "kling",
                "openai-image-2",
                "midjourney",
                "nano-banana-pro",
                "suno",
                "worldlabs-marble",
                "tripo-splat",
                "sam-3d",
                "spark-3dgs",
                "qunhe-3d"
            ],
        }),
    )
}

fn runtime_discovery_endpoints() -> Value {
    let entries = [
        ("discovery", "/api/discovery"),
        ("runtime_registry", "/api/runtime-registry"),
        ("well_known", "/.well-known/pool-runtime.json"),
        ("health", "/api/health"),
        ("snapshot", "/api/snapshot"),
        ("projects", "/api/projects"),
        ("events", "/api/events"),
        ("events_stream", "/api/events/stream"),
        ("events_websocket", "/api/events/ws"),
        ("resources", "/api/resources"),
        ("prompts", "/api/prompts"),
        ("prompt", "/api/prompts?name=<prompt-name>"),
        ("mcp", "/api/mcp?uri=pool://tasks"),
        ("runtime_graph", "/api/runtime-graph"),
        ("runtime_execution_plan", "/api/runtime-execution-plan"),
        (
            "runtime_execution_plan_run_next",
            "/api/runtime-execution-plan/run-next",
        ),
        ("runtime_budget", "/api/runtime-budget"),
        ("runtime_preflight", "/api/runtime-preflight"),
        ("runtime_handoff", "/api/runtime-handoff"),
        ("prd_readiness", "/api/prd-readiness"),
        ("prd_completion_gate", "/api/prd-completion-gate"),
        ("prd_completion_package", "/api/prd-completion-package"),
        (
            "production_evidence_requirements",
            "/api/production-evidence/requirements",
        ),
        (
            "production_evidence_tasks",
            "/api/production-evidence/tasks",
        ),
        (
            "production_evidence_task_claim",
            "/api/production-evidence/tasks/claim",
        ),
        (
            "production_evidence_run_plan",
            "/api/production-evidence/run-plan",
        ),
        (
            "production_evidence_handoff",
            "/api/production-evidence/handoff",
        ),
        (
            "workflow_context",
            "/api/workflow-context?workflow_id=<workflow-id>",
        ),
        ("node_context", "/api/node-context?node_id=<node-id>"),
        ("mcp_tasks", "/api/mcp?uri=pool://tasks"),
        (
            "mcp_runtime_execution_plan",
            "/api/mcp?uri=pool://runtime-execution-plan",
        ),
        ("mcp_runtime_budget", "/api/mcp?uri=pool://runtime-budget"),
        (
            "mcp_runtime_preflight",
            "/api/mcp?uri=pool://runtime-preflight",
        ),
        ("mcp_runtime_handoff", "/api/mcp?uri=pool://runtime-handoff"),
        (
            "mcp_runtime_handoff_packages",
            "/api/mcp?uri=pool://runtime-handoff-packages",
        ),
        ("mcp_prd_readiness", "/api/mcp?uri=pool://prd-readiness"),
        (
            "mcp_prd_completion_gate",
            "/api/mcp?uri=pool://prd-completion-gate",
        ),
        (
            "mcp_production_evidence_requirements",
            "/api/mcp?uri=pool://production-evidence-requirements",
        ),
        (
            "mcp_production_evidence_tasks",
            "/api/mcp?uri=pool://production-evidence-tasks",
        ),
        (
            "mcp_production_evidence_run_plan",
            "/api/mcp?uri=pool://production-evidence-run-plan",
        ),
        (
            "mcp_production_evidence_handoff",
            "/api/mcp?uri=pool://production-evidence-handoff",
        ),
        (
            "mcp_production_evidence_item_template",
            "/api/mcp?uri=pool://production-evidence-item-template/<task-id>",
        ),
        ("mcp_output_packages", "/api/mcp?uri=pool://output-packages"),
        ("mcp_adapters", "/api/mcp?uri=pool://adapters"),
        ("integration_readiness", "/api/integration-readiness"),
        (
            "mcp_integration_readiness",
            "/api/mcp?uri=pool://integration-readiness",
        ),
        (
            "mcp_workflow_context",
            "/api/mcp?uri=pool://workflow/<workflow-id>",
        ),
        ("mcp_runtime_graph", "/api/mcp?uri=pool://runtime-graph"),
        (
            "mcp_node_context",
            "/api/mcp?uri=pool://node-context/<node-id>",
        ),
        (
            "mcp_provider_requests",
            "/api/mcp?uri=pool://provider-requests",
        ),
        (
            "mcp_software_actions",
            "/api/mcp?uri=pool://software-actions",
        ),
        (
            "mcp_software_contracts",
            "/api/mcp?uri=pool://software-contracts",
        ),
        (
            "mcp_desktop_recognition",
            "/api/mcp?uri=pool://desktop-recognition",
        ),
        (
            "mcp_desktop_recognition_contract",
            "/api/mcp?uri=pool://desktop-recognition-contract",
        ),
        ("mcp_agent_sessions", "/api/mcp?uri=pool://agent-sessions"),
        ("api_keys", "/api/api-keys"),
        ("adapters", "/api/adapters"),
        (
            "provider_contracts",
            "/api/provider-contracts?provider_id=<provider-id>",
        ),
        ("provider_gateway_worker", "/api/provider-gateway-worker"),
        (
            "mcp_provider_gateway_worker",
            "/api/mcp?uri=pool://provider-gateway-worker",
        ),
        (
            "provider_conformance_packages",
            "/api/provider-conformance-packages",
        ),
        (
            "integration_conformance_packages",
            "/api/integration-conformance-packages",
        ),
        (
            "software_contracts",
            "/api/software-contracts?adapter_id=<adapter-id>",
        ),
        (
            "software_conformance_packages",
            "/api/software-conformance-packages",
        ),
        ("unreal_mcp_bridge", "/api/unreal-mcp-bridge"),
        (
            "mcp_unreal_mcp_bridge",
            "/api/mcp?uri=pool://unreal-mcp-bridge",
        ),
        ("adapter_health", "/api/adapter-health"),
        ("provider_health", "/api/provider-health"),
        ("software_health", "/api/software-health"),
        ("nodes_run", "/api/nodes/run"),
        ("tasks", "/api/tasks"),
        ("workflow_runs", "/api/workflow-runs"),
        ("provider_runs", "/api/provider-runs"),
        (
            "production_evidence_template",
            "/api/production-evidence/template",
        ),
        (
            "production_evidence_item_template",
            "/api/production-evidence/item-template?kind=<kind>&target_id=<target-id>",
        ),
        (
            "production_evidence_item_from_ledger",
            "/api/production-evidence/item-from-ledger?provider_request_id=<provider-request-id>",
        ),
        (
            "production_evidence_bundle_from_ledger",
            "/api/production-evidence/bundle-from-ledger",
        ),
        (
            "production_evidence_handoff_packages",
            "/api/production-evidence/handoff-packages",
        ),
        (
            "production_evidence_validate",
            "/api/production-evidence/validate",
        ),
        (
            "production_evidence_item_validate",
            "/api/production-evidence/items/validate",
        ),
        (
            "production_evidence_merge",
            "/api/production-evidence/merge",
        ),
        (
            "production_evidence_closeout",
            "/api/production-evidence/closeout",
        ),
        (
            "production_evidence_items",
            "/api/production-evidence/items",
        ),
        ("production_evidence", "/api/production-evidence"),
        (
            "provider_request_metadata",
            "/api/provider-requests/metadata?provider_request_id=<provider-request-id>",
        ),
        ("output_packages", "/api/output-packages"),
        ("output_package_results", "/api/output-packages/results"),
        ("handoff_packages", "/api/handoff-packages"),
        (
            "agent_conformance_packages",
            "/api/agent-conformance-packages",
        ),
        ("agent_sessions", "/api/agent-sessions"),
        (
            "agent_session_transcript",
            "/api/agent-sessions/transcript?session_id=<agent-session-id>",
        ),
        (
            "agent_session_stream",
            "/api/agent-sessions/stream?session_id=<agent-session-id>",
        ),
        (
            "agent_session_websocket",
            "/api/agent-sessions/ws?session_id=<agent-session-id>",
        ),
        ("tasks_approve", "/api/tasks/approve?task_id=<task-id>"),
        ("tasks_cancel", "/api/tasks/cancel?task_id=<task-id>"),
        ("tasks_retry", "/api/tasks/retry?task_id=<task-id>"),
        ("software_actions", "/api/software-actions"),
        (
            "desktop_recognition_requests",
            "/api/desktop-recognition/requests",
        ),
        (
            "desktop_recognition_contract",
            "/api/desktop-recognition/contract",
        ),
        (
            "desktop_recognition_run_next",
            "/api/desktop-recognition/run-next",
        ),
        (
            "desktop_recognition_results",
            "/api/desktop-recognition/results",
        ),
    ];
    Value::Object(
        entries
            .into_iter()
            .map(|(key, value)| (key.to_string(), Value::String(value.to_string())))
            .collect(),
    )
}

fn mcp_resource_discovery() -> Value {
    let server = McpServer::new();
    let resources = server
        .list_resources()
        .iter()
        .map(|resource| {
            json!({
                "uri": resource.uri.clone(),
                "name": resource.name.clone(),
                "description": resource.description.clone(),
                "http_path": mcp_resource_http_path(&resource.uri),
            })
        })
        .collect::<Vec<_>>();

    Value::Array(resources)
}

fn mcp_prompt_discovery() -> Value {
    Value::Array(
        pool_mcp_prompt_definitions()
            .into_iter()
            .map(|prompt| {
                let mut object = prompt.as_object().cloned().unwrap_or_default();
                if let Some(name) = object.get("name").and_then(Value::as_str) {
                    object.insert(
                        "http_path".to_string(),
                        json!(pool_mcp_prompt_http_path(name)),
                    );
                }
                Value::Object(object)
            })
            .collect::<Vec<_>>(),
    )
}

fn mcp_tool_discovery() -> Value {
    let tools = [
        (
            "pool_status",
            "Read Pool runtime health.",
            "read",
            json!({}),
        ),
        (
            "pool_snapshot",
            "Read the sanitized RuntimeSnapshot.",
            "read",
            json!({}),
        ),
        (
            "pool_adapters",
            "Read Provider and software adapter catalog.",
            "read",
            json!({}),
        ),
        (
            "pool_integration_readiness",
            "Read snapshot-backed Provider, software, and Agent/Hermes integration readiness matrix.",
            "read",
            json!({
                "project_slug": "<project>"
            }),
        ),
        (
            "pool_provider_gateway_worker",
            "Read the Provider gateway worker contract.",
            "read",
            json!({}),
        ),
        (
            "pool_provider_conformance_package",
            "Write a local Provider conformance package with provider contract, gateway worker contract, runbook, preflight, and runner script.",
            "write",
            json!({
                "project_slug": "<project>",
                "provider_id": "worldlabs-marble",
                "output_dir": "worlds/<project>/output"
            }),
        ),
        (
            "pool_integration_conformance_package",
            "Write a local integration conformance package for Provider, software, and Agent/Hermes adapters.",
            "write",
            json!({
                "project_slug": "<project>",
                "output_dir": "worlds/<project>/output",
                "providers": ["worldlabs-marble"],
                "software_adapters": ["resolve"],
                "agent_kind": "all"
            }),
        ),
        (
            "pool_worker_self_checks",
            "Run local Provider gateway, SDK worker, Unreal, Hermes, and software bridge self-checks.",
            "local_smoke",
            json!({
                "output_root": "target/pool-worker-self-checks",
                "software_adapter": "resolve"
            }),
        ),
        (
            "pool_unreal_mcp_bridge",
            "Read the Unreal MCP bridge contract.",
            "read",
            json!({}),
        ),
        (
            "pool_runtime_execution_plan",
            "Read ordered executable workflow steps.",
            "read",
            json!({}),
        ),
        (
            "pool_runtime_execution_plan_run_next",
            "Preview or dispatch the next runtime execution-plan step.",
            "write",
            json!({ "execute": false }),
        ),
        (
            "pool_handoff_package",
            "Write a local runtime handoff package with runbook, preflight, graph, integration readiness, worker self-check script, manifest, and optional snapshot.",
            "write",
            json!({
                "project_slug": "<project>",
                "node_id": "agent",
                "output_dir": "worlds/<project>/output",
                "include_snapshot": true
            }),
        ),
        (
            "pool_software_conformance_package",
            "Write a local software adapter conformance package with contract, runbook, preflight, and runner script.",
            "write",
            json!({
                "project_slug": "<project>",
                "adapter_id": "resolve",
                "output_dir": "worlds/<project>/output"
            }),
        ),
        (
            "pool_run_provider",
            "Run a Provider task such as AI media or 3DGS.",
            "write",
            json!({ "provider_id": "world-labs-marble", "execution_mode": "mock" }),
        ),
        (
            "pool_run_software",
            "Run or stage an external software control action.",
            "write",
            json!({ "adapter_id": "blender", "action_kind": "ExecuteCli" }),
        ),
        (
            "pool_agent_session",
            "Stage or execute Hermes / Agent CLI control sessions.",
            "write",
            json!({ "kind": "agent_cli" }),
        ),
        (
            "pool_agent_conformance_package",
            "Write a local Agent/Hermes conformance package with session contract, runbook, preflight, and runner script.",
            "write",
            json!({
                "project_slug": "<project>",
                "kind": "all",
                "output_dir": "worlds/<project>/output"
            }),
        ),
        (
            "pool_production_evidence_tasks",
            "Read missing production evidence tasks.",
            "read",
            json!({}),
        ),
        (
            "pool_production_evidence_task_claim",
            "Claim one missing production evidence task.",
            "write",
            json!({ "task_id": "<task-id>" }),
        ),
        (
            "pool_closeout_production_evidence",
            "Merge, validate, and optionally import production evidence bundles.",
            "write",
            json!({ "bundles": [], "import": false }),
        ),
        (
            "pool_desktop_run_next",
            "Dry-run queued desktop recognition requests.",
            "write",
            json!({ "controller_id": "local-vision-dry-run", "limit": 1 }),
        ),
    ];

    Value::Array(
        tools
            .into_iter()
            .map(|(name, description, category, example_arguments)| {
                json!({
                    "name": name,
                    "description": description,
                    "category": category,
                    "transport": "mcp_stdio",
                    "method": "tools/call",
                    "example_arguments": example_arguments,
                    "stdio_command": "pool-cli --project <project> serve-mcp",
                })
            })
            .collect::<Vec<_>>(),
    )
}

fn mcp_resource_http_path(uri: &str) -> String {
    format!("/api/mcp?uri={}", percent_encode_query_value(uri))
}

fn percent_encode_query_value(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

fn sse_line(value: &str) -> String {
    value.replace('\r', " ").replace('\n', " ")
}

fn sse_event_frame(event: &crate::EventSnapshot) -> Result<String> {
    Ok(format!(
        "id: {}\nevent: runtime-event\ndata: {}\n\n",
        sse_line(&event.id),
        sse_line(&serde_json::to_string(event)?)
    ))
}

fn sse_json_frame(event_name: &str, id: Option<&str>, payload: &Value) -> Result<String> {
    let id = id
        .map(|id| format!("id: {}\n", sse_line(id)))
        .unwrap_or_default();
    Ok(format!(
        "{}event: {}\ndata: {}\n\n",
        id,
        sse_line(event_name),
        sse_line(&serde_json::to_string(payload)?)
    ))
}

fn sse_cursor_frame(event_id: &str) -> String {
    format!("event: cursor\ndata: {}\n\n", sse_line(event_id))
}

fn sse_response_headers() -> String {
    format!(
        "HTTP/1.1 200 {}\r\nContent-Type: text/event-stream; charset=utf-8\r\nCache-Control: no-cache\r\nAccess-Control-Allow-Origin: *\r\nConnection: keep-alive\r\nX-Accel-Buffering: no\r\n\r\n",
        status_text(200)
    )
}

fn websocket_header<'a>(headers: &'a BTreeMap<String, String>, name: &str) -> Option<&'a str> {
    headers
        .get(&name.to_ascii_lowercase())
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn websocket_header_contains(
    headers: &BTreeMap<String, String>,
    name: &str,
    expected: &str,
) -> bool {
    websocket_header(headers, name).is_some_and(|value| {
        value
            .split(',')
            .any(|part| part.trim().eq_ignore_ascii_case(expected))
    })
}

fn websocket_requested_protocol(headers: &BTreeMap<String, String>) -> Option<&'static str> {
    let protocols = websocket_header(headers, "sec-websocket-protocol")?;
    protocols
        .split(',')
        .any(|protocol| protocol.trim() == "pool.events.v1")
        .then_some("pool.events.v1")
}

fn websocket_accept_key(sec_websocket_key: &str) -> String {
    let mut hasher = Sha1::new();
    hasher.update(sec_websocket_key.trim().as_bytes());
    hasher.update(b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11");
    general_purpose::STANDARD.encode(Sha1Digest::finalize(hasher))
}

fn websocket_response_headers(sec_websocket_key: &str, protocol: Option<&str>) -> String {
    let protocol_header = protocol
        .map(|protocol| format!("Sec-WebSocket-Protocol: {protocol}\r\n"))
        .unwrap_or_default();
    format!(
        "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {}\r\n{}Access-Control-Allow-Origin: *\r\n\r\n",
        websocket_accept_key(sec_websocket_key),
        protocol_header,
    )
}

fn write_websocket_upgrade_error(writer: &mut impl Write, message: &str, path: &str) -> Result<()> {
    let response = RuntimeHttpResponse::json(
        426,
        json!({
            "error": "websocket_upgrade_required",
            "message": message,
            "path": path,
        }),
    )?;
    writer
        .write_all(response.to_http_bytes().as_bytes())
        .context("write WebSocket upgrade error response")
}

fn write_websocket_json_frame(writer: &mut impl Write, payload: &Value) -> Result<()> {
    let payload = serde_json::to_string(payload).context("serialize WebSocket JSON payload")?;
    write_websocket_text_frame(writer, payload.as_bytes())
}

fn write_websocket_text_frame(writer: &mut impl Write, payload: &[u8]) -> Result<()> {
    let mut header = vec![0x81];
    match payload.len() {
        len if len < 126 => header.push(len as u8),
        len if len <= u16::MAX as usize => {
            header.push(126);
            header.extend_from_slice(&(len as u16).to_be_bytes());
        }
        len => {
            header.push(127);
            header.extend_from_slice(&(len as u64).to_be_bytes());
        }
    }
    writer
        .write_all(&header)
        .context("write WebSocket text frame header")?;
    writer
        .write_all(payload)
        .context("write WebSocket text frame payload")
}

fn default_agent_tools() -> Vec<String> {
    ["api", "mcp", "skills", "cli"]
        .into_iter()
        .map(ToString::to_string)
        .collect()
}

fn default_agent_cli_allowlist() -> Vec<String> {
    vec!["pool-cli".to_string()]
}

fn should_run_desktop_recognition(action: &SoftwareControlAction) -> bool {
    action.priority == ControlPriority::DesktopRecognition
        || matches!(
            action.action_kind,
            SoftwareActionKind::DesktopClick | SoftwareActionKind::DesktopHotkey
        )
}

fn action_with_default_control_dir(
    mut action: SoftwareControlAction,
    control_dir: PathBuf,
) -> SoftwareControlAction {
    let mut payload = match action.payload_json {
        Value::Object(map) => map,
        other => {
            let mut map = serde_json::Map::new();
            map.insert("value".to_string(), other);
            map
        }
    };
    payload
        .entry("control_dir".to_string())
        .or_insert_with(|| Value::String(control_dir.to_string_lossy().to_string()));
    action.payload_json = Value::Object(payload);
    action
}

fn agent_auth_token(repository: &RuntimeRepository, provider: &str) -> Result<Option<String>> {
    if let Some(secret) = repository.api_key_secret(provider, "agent")? {
        return Ok(Some(secret));
    }
    repository.api_key_secret(provider, "provider")
}

enum ProjectFilterOverride<'a> {
    All,
    Slug(&'a str),
}

fn project_filter_for_request(request: &RuntimeHttpRequest) -> Option<ProjectFilterOverride<'_>> {
    let value = request
        .query
        .get("project_slug")
        .or_else(|| request.query.get("project"))
        .map(String::as_str)
        .map(str::trim)?;
    if value.is_empty() || value == "*" || value.eq_ignore_ascii_case("all") {
        Some(ProjectFilterOverride::All)
    } else {
        Some(ProjectFilterOverride::Slug(value))
    }
}

fn agent_session_id_for_request(request: &RuntimeHttpRequest) -> Result<&str> {
    request
        .query
        .get("session_id")
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .context("agent session stream requires session_id")
}

fn api_key_rotation_days_for_request(request: &RuntimeHttpRequest) -> Result<u64> {
    let Some(value) = request
        .query
        .get("rotation_days")
        .or_else(|| request.query.get("rotation_interval_days"))
    else {
        return Ok(90);
    };
    let value = value.trim();
    if value.is_empty() {
        return Ok(90);
    }
    let days = value
        .parse::<u64>()
        .context("rotation_days must be an unsigned integer")?;
    if days > 3650 {
        bail!("rotation_days must be 3650 or less");
    }
    Ok(days)
}

fn api_key_audit_value(api_keys: &[ApiKeySnapshot], default_rotation_days: u64) -> Value {
    let items = api_keys
        .iter()
        .map(|api_key| api_key_audit_item(api_key, default_rotation_days))
        .collect::<Vec<_>>();
    let rotation_due = items
        .iter()
        .filter(|item| {
            item.get("rotation_due")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
        .count();
    let unencrypted = items
        .iter()
        .filter(|item| {
            !item
                .get("encrypted")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
        .count();
    let configured = api_keys.iter().filter(|key| key.configured).count();
    json!({
        "kind": "pool_api_key_audit",
        "default_rotation_days": default_rotation_days,
        "total": api_keys.len(),
        "configured": configured,
        "rotation_due": rotation_due,
        "unencrypted": unencrypted,
        "items": items,
    })
}

fn api_key_audit_item(api_key: &ApiKeySnapshot, default_rotation_days: u64) -> Value {
    let metadata = &api_key.metadata;
    let rotation_days = metadata
        .get("rotation_days")
        .and_then(Value::as_u64)
        .unwrap_or(default_rotation_days);
    let age_days = api_key_age_days(&api_key.updated_at);
    let rotation_due = age_days
        .map(|days| days >= rotation_days)
        .unwrap_or(api_key.configured);
    json!({
        "provider": api_key.provider.clone(),
        "service_type": api_key.service_type.clone(),
        "configured": api_key.configured,
        "key_hint": api_key.key_hint.clone(),
        "source": metadata.get("source").and_then(Value::as_str),
        "env": metadata.get("env").and_then(Value::as_str),
        "owner": metadata.get("owner").and_then(Value::as_str),
        "storage": metadata.pointer("/credential/storage").and_then(Value::as_str),
        "backend": metadata.pointer("/credential/backend").and_then(Value::as_str),
        "encrypted": metadata.pointer("/credential/encrypted").and_then(Value::as_bool).unwrap_or(false),
        "created_at": api_key.created_at.clone(),
        "updated_at": api_key.updated_at.clone(),
        "age_days": age_days,
        "rotation_days": rotation_days,
        "rotation_due": rotation_due,
    })
}

fn api_key_age_days(updated_at: &str) -> Option<u64> {
    let updated_at = chrono::DateTime::parse_from_rfc3339(updated_at).ok()?;
    let age = chrono::Utc::now().signed_duration_since(updated_at.with_timezone(&chrono::Utc));
    Some(age.num_days().max(0) as u64)
}

fn desktop_recognition_request_value(action: &SoftwareActionSnapshot) -> Option<Value> {
    if !is_open_desktop_recognition_request(action) {
        return None;
    }

    let request_path = desktop_request_path(action);
    let request_file =
        request_path
            .as_deref()
            .and_then(|path| match std::fs::read_to_string(path) {
                Ok(raw) => serde_json::from_str::<Value>(&raw).ok(),
                Err(_) => None,
            });
    let pool_desktop_action = request_file
        .as_ref()
        .and_then(|value| value.get("pool_desktop_action"))
        .cloned();
    let desktop_payload = request_file
        .as_ref()
        .and_then(|value| value.get("desktop_payload"))
        .cloned();
    let request_status = request_file
        .as_ref()
        .and_then(|value| value.get("status"))
        .and_then(Value::as_str)
        .or_else(|| desktop_recognition_status(action))
        .unwrap_or("queued_for_desktop_recognition");

    Some(json!({
        "software_action_id": action.id.clone(),
        "task_id": action.task_id.clone(),
        "adapter_id": action.adapter_id.clone(),
        "action_kind": action.action_kind.clone(),
        "status": request_status,
        "desktop_request_path": request_path,
        "request_file_available": request_file.is_some(),
        "pool_desktop_action": pool_desktop_action,
        "desktop_payload": desktop_payload,
        "command": action.command.clone(),
        "verification": action.verification.clone(),
        "created_at": action.created_at.clone(),
    }))
}

fn desktop_run_next_result_body(
    request: &Value,
    software_action_id: &str,
    status: &str,
    controller_id: &str,
    message: Option<&str>,
    extra_artifacts: &[String],
    screen_trace_path: Option<&str>,
) -> Value {
    let mut artifacts = extra_artifacts.to_vec();
    if let Some(request_path) = json_value_string(request, "desktop_request_path") {
        artifacts.push(request_path);
    }
    artifacts.sort();
    artifacts.dedup();

    let action_kind =
        json_value_string(request, "action_kind").unwrap_or_else(|| "unknown".to_string());
    let message = message.map(ToString::to_string).unwrap_or_else(|| {
        format!("pool runtime desktop controller dry-run: {status} {action_kind}")
    });
    let mut body = serde_json::Map::new();
    body.insert("software_action_id".to_string(), json!(software_action_id));
    if let Some(task_id) = json_value_string(request, "task_id") {
        body.insert("task_id".to_string(), json!(task_id));
    }
    body.insert("status".to_string(), json!(status));
    body.insert("message".to_string(), json!(message));
    if !artifacts.is_empty() {
        body.insert("artifacts".to_string(), json!(artifacts));
    }
    if let Some(screen_trace_path) = screen_trace_path {
        body.insert("screen_trace_path".to_string(), json!(screen_trace_path));
    }
    body.insert(
        "result".to_string(),
        json!({
            "controller": controller_id,
            "mode": "dry_run",
            "software_action_id": software_action_id,
            "adapter_id": request.get("adapter_id").cloned(),
            "action_kind": request.get("action_kind").cloned(),
            "desktop_request_path": request.get("desktop_request_path").cloned(),
            "pool_desktop_action": request.get("pool_desktop_action").cloned(),
            "desktop_payload": request.get("desktop_payload").cloned(),
            "request_file_available": request.get("request_file_available").cloned(),
        }),
    );
    body.insert(
        "verification".to_string(),
        json!({
            "controller": controller_id,
            "mode": "dry_run",
        }),
    );
    Value::Object(body)
}

fn is_open_desktop_recognition_request(action: &SoftwareActionSnapshot) -> bool {
    if !is_desktop_recognition_action(action) {
        return false;
    }
    match desktop_recognition_status(action).and_then(normalize_desktop_recognition_status) {
        Some("queued_for_desktop_recognition") | None => true,
        Some("retryable") => true,
        Some(_) => false,
    }
}

fn is_desktop_recognition_action(action: &SoftwareActionSnapshot) -> bool {
    let priority = action
        .command
        .get("priority")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let command_action_kind = action
        .command
        .get("action_kind")
        .and_then(Value::as_str)
        .unwrap_or(action.action_kind.as_str());
    priority == "DesktopRecognition"
        || matches!(command_action_kind, "DesktopClick" | "DesktopHotkey")
        || desktop_request_path(action).is_some()
        || desktop_recognition_artifacts(action)
            .iter()
            .any(|artifact| artifact.starts_with("desktop-recognition://"))
}

fn desktop_recognition_status(action: &SoftwareActionSnapshot) -> Option<&str> {
    action
        .verification
        .as_ref()
        .and_then(|value| {
            value
                .get("desktop_recognition_status")
                .or_else(|| value.get("status"))
        })
        .and_then(Value::as_str)
}

fn desktop_request_path(action: &SoftwareActionSnapshot) -> Option<String> {
    desktop_recognition_artifacts(action)
        .into_iter()
        .find(|artifact| artifact.ends_with(".json") && artifact.contains("desktop-recognition"))
}

fn desktop_recognition_artifacts(action: &SoftwareActionSnapshot) -> Vec<String> {
    action
        .verification
        .as_ref()
        .and_then(|value| value.get("artifacts"))
        .and_then(Value::as_array)
        .map(|artifacts| {
            artifacts
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn normalize_desktop_recognition_status(status: &str) -> Option<&'static str> {
    match status.trim().to_ascii_lowercase().as_str() {
        "queued" | "queued_for_desktop_recognition" | "pending" => {
            Some("queued_for_desktop_recognition")
        }
        "running" | "in_progress" | "claimed" | "working" => Some("running"),
        "succeeded" | "success" | "ok" | "completed" | "complete" => Some("succeeded"),
        "failed" | "failure" | "error" => Some("failed"),
        "retryable" | "retry" => Some("retryable"),
        "cancelled" | "canceled" => Some("cancelled"),
        _ => None,
    }
}

fn task_status_for_desktop_recognition(status: &str) -> TaskStatus {
    match status {
        "succeeded" => TaskStatus::Succeeded,
        "failed" => TaskStatus::Failed,
        "retryable" => TaskStatus::Retryable,
        "cancelled" => TaskStatus::Cancelled,
        "running" | "queued_for_desktop_recognition" => TaskStatus::Running,
        _ => TaskStatus::Retryable,
    }
}

fn event_level_for_desktop_recognition(status: &str) -> RuntimeEventLevel {
    match status {
        "succeeded" => RuntimeEventLevel::Ok,
        "failed" => RuntimeEventLevel::Error,
        "retryable" | "cancelled" => RuntimeEventLevel::Warn,
        _ => RuntimeEventLevel::Info,
    }
}

fn production_evidence_validation_value(
    request: &ImportProductionEvidenceRequest,
    project_slug: &str,
    source: &str,
) -> std::result::Result<Value, Value> {
    let (provider_count, software_count, desktop_count) =
        match validate_production_evidence_bundle(request) {
            Ok(counts) => counts,
            Err(error) => {
                return Err(json!({
                        "error": "invalid_production_evidence_item",
                        "message": error.to_string(),
                        "writes": 0,
                }));
            }
        };

    if provider_count == 0 && software_count == 0 && desktop_count == 0 {
        return Err(json!({
                "error": "empty_production_evidence_bundle",
                "expected": "providers, software_actions, or desktop_vision evidence arrays",
                "writes": 0,
        }));
    }

    Ok(json!({
        "kind": "pool_production_evidence_validation",
        "valid": true,
        "writes": 0,
        "project_slug": project_slug,
        "source": source,
        "summary": {
            "providers": provider_count,
            "software_actions": software_count,
            "desktop_vision": desktop_count,
        },
        "coverage": production_evidence_coverage(request),
        "artifact_files": production_evidence_artifact_file_report(request),
        "providers": provider_production_evidence_validation_rows(request),
        "software_actions": software_production_evidence_validation_rows(request),
        "desktop_vision": desktop_vision_production_evidence_validation_rows(request),
    }))
}

fn merge_production_evidence_requests(
    project_slug: &str,
    source: Option<&str>,
    bundles: Vec<ImportProductionEvidenceRequest>,
) -> Result<Value> {
    let source = source
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("runtime_production_evidence_merge")
        .to_string();
    let mut providers = Vec::new();
    let mut software_actions = Vec::new();
    let mut desktop_vision = Vec::new();
    let mut input_summaries = Vec::new();
    let input_bundle_count = bundles.len();

    for (index, bundle) in bundles.into_iter().enumerate() {
        let bundle_project_slug = bundle
            .project_slug
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty() && *value != "*")
            .map(ToString::to_string);
        if let Some(bundle_project_slug) = bundle_project_slug.as_deref() {
            if bundle_project_slug != project_slug {
                bail!(
                    "conflicting project_slug in production evidence bundle {}: expected {}, got {}",
                    index + 1,
                    project_slug,
                    bundle_project_slug
                );
            }
        }
        let bundle_source = bundle.source.clone();
        let provider_items = bundle.providers.unwrap_or_default();
        let software_items = bundle.software_actions.unwrap_or_default();
        let desktop_items = bundle.desktop_vision.unwrap_or_default();
        let provider_count = provider_items.len();
        let software_count = software_items.len();
        let desktop_count = desktop_items.len();

        providers.extend(provider_items);
        software_actions.extend(software_items);
        desktop_vision.extend(desktop_items);
        input_summaries.push(json!({
            "index": index + 1,
            "source": bundle_source,
            "project_slug": bundle_project_slug,
            "providers": provider_count,
            "software_actions": software_count,
            "desktop_vision": desktop_count,
        }));
    }

    let bundle = json!({
        "project_slug": project_slug,
        "source": source.clone(),
        "providers": providers,
        "software_actions": software_actions,
        "desktop_vision": desktop_vision,
        "merge": {
            "input_count": input_bundle_count,
            "inputs": input_summaries,
        },
    });
    let summary = json!({
        "input_bundles": input_bundle_count,
        "providers": bundle
            .get("providers")
            .and_then(Value::as_array)
            .map_or(0, Vec::len),
        "software_actions": bundle
            .get("software_actions")
            .and_then(Value::as_array)
            .map_or(0, Vec::len),
        "desktop_vision": bundle
            .get("desktop_vision")
            .and_then(Value::as_array)
            .map_or(0, Vec::len),
    });
    let input_summaries = bundle
        .pointer("/merge/inputs")
        .cloned()
        .unwrap_or_else(|| json!([]));

    Ok(json!({
        "kind": "pool_production_evidence_merge",
        "project_slug": project_slug,
        "source": source,
        "writes": 0,
        "summary": summary,
        "input_summaries": input_summaries,
        "bundle": bundle,
        "commands": {
            "closeout": format!("pool-cli --project {project_slug} closeout-production-evidence --output <merged-bundle.json> <bundle-a.json> <bundle-b.json>..."),
            "validate": format!("pool-cli --project {project_slug} validate-production-evidence <merged-bundle.json>"),
            "import": format!("pool-cli --project {project_slug} import-production-evidence <merged-bundle.json>"),
            "readiness": format!("pool-cli --project {project_slug} prd-readiness"),
        },
    }))
}

fn production_evidence_count_summary_total(summary: &Value) -> usize {
    ["providers", "software_actions", "desktop_vision"]
        .into_iter()
        .filter_map(|key| summary.get(key).and_then(Value::as_u64))
        .map(|value| value as usize)
        .sum()
}

fn validate_production_evidence_bundle(
    request: &ImportProductionEvidenceRequest,
) -> Result<(usize, usize, usize)> {
    let providers = request.providers.as_deref().unwrap_or(&[]);
    let software_actions = request.software_actions.as_deref().unwrap_or(&[]);
    let desktop_vision = request.desktop_vision.as_deref().unwrap_or(&[]);

    for (index, item) in providers.iter().enumerate() {
        validate_provider_production_evidence_item(item, index)?;
    }
    for (index, item) in software_actions.iter().enumerate() {
        validate_software_production_evidence_item(item, index)?;
    }
    for (index, item) in desktop_vision.iter().enumerate() {
        validate_desktop_vision_production_evidence_item(item, index)?;
    }

    Ok((
        providers.len(),
        software_actions.len(),
        desktop_vision.len(),
    ))
}

fn validate_provider_production_evidence_item(
    item: &ProviderProductionEvidenceItem,
    index: usize,
) -> Result<()> {
    required_non_empty(
        item.provider_id.clone(),
        &production_evidence_field("providers", index, "provider_id"),
    )?;
    required_production_identifier(
        item.external_job_id.clone(),
        &production_evidence_field("providers", index, "external_job_id"),
    )?;
    required_provider_production_attestation(
        item.production_attestation.as_deref(),
        item.evidence_json.as_ref(),
        &production_evidence_field("providers", index, "production_attestation"),
    )?;
    required_local_artifact_vec(
        item.artifacts.clone(),
        &production_evidence_field("providers", index, "artifacts"),
    )?;
    if let Some(metadata_path) = &item.metadata_path {
        required_local_artifact_path(
            metadata_path.clone(),
            &production_evidence_field("providers", index, "metadata_path"),
        )?;
    }
    Ok(())
}

fn validate_software_production_evidence_item(
    item: &SoftwareProductionEvidenceItem,
    index: usize,
) -> Result<()> {
    required_non_empty(
        item.adapter_id.clone(),
        &production_evidence_field("software_actions", index, "adapter_id"),
    )?;
    required_production_identifier(
        item.external_action_id.clone(),
        &production_evidence_field("software_actions", index, "external_action_id"),
    )?;
    required_production_attestation(
        item.production_attestation.as_deref(),
        item.evidence_json.as_ref(),
        &production_evidence_field("software_actions", index, "production_attestation"),
        "a real software plugin, API, CLI, MCP, or desktop-control run",
    )?;
    if item
        .artifacts
        .as_ref()
        .map(|artifacts| artifacts.is_empty())
        .unwrap_or(true)
        && item.verification_json.is_none()
    {
        bail!(
            "{} requires artifacts or verification_json",
            production_evidence_field("software_actions", index, "")
        );
    }
    Ok(())
}

fn validate_desktop_vision_production_evidence_item(
    item: &DesktopVisionProductionEvidenceItem,
    index: usize,
) -> Result<()> {
    required_production_identifier(
        item.external_action_id.clone(),
        &production_evidence_field("desktop_vision", index, "external_action_id"),
    )?;
    required_production_identifier(
        item.controller_id.clone(),
        &production_evidence_field("desktop_vision", index, "controller_id"),
    )?;
    required_production_attestation(
        item.production_attestation.as_deref(),
        item.evidence_json.as_ref(),
        &production_evidence_field("desktop_vision", index, "production_attestation"),
        "a real external visual/OCR/screen model controller run",
    )?;
    required_local_artifact_path(
        item.trace_path.clone(),
        &production_evidence_field("desktop_vision", index, "trace_path"),
    )?;
    if let Some(artifacts) = &item.artifacts {
        for artifact in artifacts {
            required_local_artifact_path(
                artifact.clone(),
                &production_evidence_field("desktop_vision", index, "artifacts"),
            )?;
        }
    }
    if !desktop_vision_external_visual_model(item) {
        bail!(
            "{} must explicitly identify an external visual model; set visual_model:\"external\" or evidence_json.external_visual_model:true",
            production_evidence_field("desktop_vision", index, "visual_model")
        );
    }
    Ok(())
}

fn production_evidence_field(collection: &str, index: usize, field: &str) -> String {
    if field.is_empty() {
        format!("{collection}[{index}]")
    } else {
        format!("{collection}[{index}].{field}")
    }
}

const REQUIRED_PRODUCTION_PROVIDER_EVIDENCE: &[&str] = &[
    "midjourney",
    "openai-image-2",
    "nano-banana-pro",
    "suno",
    "worldlabs-marble",
    "tripo-splat",
    "sam-3d",
    "spark-3dgs",
    "qunhe-3d",
];

const REQUIRED_PRODUCTION_SOFTWARE_EVIDENCE: &[&str] = &[
    "unreal",
    "blender",
    "comfyui",
    "resolve",
    "unity",
    "touchdesigner",
    "madmapper",
    "nuke",
    "motion-db",
    "editing-suite",
    "hermes",
];

#[derive(Debug, Clone)]
struct ProductionEvidenceTemplateScope {
    mode: &'static str,
    providers: Vec<String>,
    software: Vec<String>,
    include_desktop_vision: bool,
}

impl ProductionEvidenceTemplateScope {
    fn full() -> Self {
        Self {
            mode: "full",
            providers: REQUIRED_PRODUCTION_PROVIDER_EVIDENCE
                .iter()
                .map(|value| (*value).to_string())
                .collect(),
            software: REQUIRED_PRODUCTION_SOFTWARE_EVIDENCE
                .iter()
                .map(|value| (*value).to_string())
                .collect(),
            include_desktop_vision: true,
        }
    }

    fn missing_only(snapshot: &RuntimeSnapshot) -> Result<Self> {
        let requirements = runtime_production_evidence_requirements_resource(snapshot)?;
        let summary = requirements
            .get("summary")
            .cloned()
            .unwrap_or_else(|| json!({}));
        Ok(Self {
            mode: "missing_only",
            providers: json_string_array_at(
                &summary,
                "missing_provider_production_upstream_success",
            ),
            software: json_string_array_at(&summary, "missing_software_production_success"),
            include_desktop_vision: !json_string_array_at(&summary, "missing_desktop_vision")
                .is_empty(),
        })
    }
}

fn production_evidence_handoff_value(
    project_slug: &str,
    output_root: Option<&str>,
    source: &str,
    snapshot: &RuntimeSnapshot,
) -> Result<Value> {
    let default_output_root = format!("worlds/{project_slug}/output/production-evidence");
    let output_root = output_root.unwrap_or(default_output_root.as_str());
    let requirements = runtime_production_evidence_requirements_resource(snapshot)?;
    let template = production_evidence_template_value(
        project_slug,
        Some(output_root),
        source,
        ProductionEvidenceTemplateScope::missing_only(snapshot)?,
    );
    let missing_total = requirements
        .pointer("/summary/missing_total")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let evidence_task_count = requirements
        .pointer("/evidence_tasks/summary/total")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let bundle = template.get("bundle").cloned().unwrap_or_else(|| json!({}));

    Ok(json!({
        "kind": "pool_production_evidence_handoff",
        "version": 1,
        "project_slug": project_slug,
        "generated_at": snapshot.generated_at,
        "overall_status": requirements.get("overall_status").cloned().unwrap_or_else(|| json!("partial")),
        "ready_for_import": false,
        "reason": "This handoff is a missing-only production evidence package. Replace placeholders with real upstream/software/visual-controller evidence, validate, then import.",
        "output_root": output_root,
        "summary": {
            "missing_total": missing_total,
            "evidence_tasks": evidence_task_count,
            "provider_tasks": requirements.pointer("/evidence_tasks/summary/provider_tasks").cloned().unwrap_or_else(|| json!(0)),
            "software_tasks": requirements.pointer("/evidence_tasks/summary/software_tasks").cloned().unwrap_or_else(|| json!(0)),
            "desktop_vision_tasks": requirements.pointer("/evidence_tasks/summary/desktop_vision_tasks").cloned().unwrap_or_else(|| json!(0)),
        },
        "requirements": requirements,
        "missing_only_template": template,
        "bundle": bundle,
        "commands": {
            "requirements": format!("pool-cli --project {project_slug} production-evidence-requirements"),
            "template": format!("pool-cli --project {project_slug} production-evidence-template --missing-only --output-root {output_root} <bundle.json>"),
            "merge": format!("pool-cli --project {project_slug} merge-production-evidence <combined-bundle.json> <bundle-a.json> <bundle-b.json>..."),
            "closeout": format!("pool-cli --project {project_slug} closeout-production-evidence --output <merged-bundle.json> <bundle-a.json> <bundle-b.json>..."),
            "validate": format!("pool-cli --project {project_slug} validate-production-evidence <bundle.json>"),
            "import": format!("pool-cli --project {project_slug} import-production-evidence <bundle.json>"),
            "submit_item": format!("pool-cli --project {project_slug} submit-production-evidence-item <item.json>"),
            "readiness": format!("pool-cli --project {project_slug} prd-readiness"),
        },
        "http": {
            "requirements": format!("GET /api/production-evidence/requirements?project={project_slug}"),
            "handoff": format!("GET /api/production-evidence/handoff?project={project_slug}"),
            "template": format!("GET /api/production-evidence/template?project={project_slug}&missing_only=true"),
            "closeout": "POST /api/production-evidence/closeout",
            "validate": "POST /api/production-evidence/validate",
            "import": "POST /api/production-evidence",
            "submit_item": "POST /api/production-evidence/items",
        },
        "mcp": {
            "resources": ["pool://production-evidence-requirements", "pool://prd-readiness"],
            "tools": [
                "pool_production_evidence_requirements",
                "pool_production_evidence_handoff",
                "pool_production_evidence_template",
                "pool_closeout_production_evidence",
                "pool_validate_production_evidence",
                "pool_import_production_evidence",
                "pool_submit_production_evidence_item"
            ],
        },
        "operator_checklist": [
            "Assign each evidence_tasks.tasks item to a Provider worker, software operator, or visual-controller operator.",
            "Write every provider artifact and metadata_path to local files before validation.",
            "Use real external_job_id, external_action_id, production_attestation, controller_id, and external visual model trace values; placeholders are rejected.",
            "Use submit-production-evidence-item or pool_submit_production_evidence_item for per-task evidence handoff; use bundle import when closing a full batch.",
            "Use merge-production-evidence when Provider, software, and desktop vision evidence were produced by separate runners.",
            "Use closeout-production-evidence to merge and validate multi-runner bundles in one writes:0 preflight.",
            "Run closeout-production-evidence or validate-production-evidence first and confirm writes:0 plus artifact_files.complete:true.",
            "Import only after the evidence was produced by real upstream services, real software control, or a real visual controller."
        ],
        "next_actions": if missing_total == 0 {
            vec![format!("Run pool-cli --project {project_slug} prd-readiness and archive this handoff as production evidence is already complete.")]
        } else {
            vec![
                format!("Write the bundle field to a JSON file after replacing placeholders."),
                format!("If evidence was produced as multiple bundles, merge them with pool-cli --project {project_slug} merge-production-evidence <combined-bundle.json> <bundle-a.json> <bundle-b.json>..."),
                format!("For final batch preflight, run pool-cli --project {project_slug} closeout-production-evidence --output <merged-bundle.json> <bundle-a.json> <bundle-b.json>..."),
                format!("Run pool-cli --project {project_slug} validate-production-evidence <bundle.json>."),
                format!("Run pool-cli --project {project_slug} import-production-evidence <bundle.json> after validation passes."),
            ]
        },
    }))
}

fn production_evidence_run_plan_value(
    project_slug: &str,
    output_root: Option<&str>,
    source: &str,
    snapshot: &RuntimeSnapshot,
) -> Result<Value> {
    production_evidence_run_plan_resource(project_slug, output_root, source, snapshot)
}

fn write_production_evidence_handoff_package(
    package_dir: &Path,
    project_slug: &str,
    node_id: Option<&str>,
    title: &str,
    output_root: &str,
    source: &str,
    include_items: bool,
    include_snapshot: bool,
    snapshot: &RuntimeSnapshot,
) -> Result<Value> {
    let created_at = chrono::Utc::now().to_rfc3339();
    let requirements = runtime_production_evidence_requirements_resource(snapshot)?;
    let tasks = production_evidence_tasks_resource(project_slug, snapshot)?;
    let handoff =
        production_evidence_handoff_value(project_slug, Some(output_root), source, snapshot)?;
    let bundle = handoff.get("bundle").cloned().unwrap_or_else(|| json!({}));

    fs::create_dir_all(package_dir).with_context(|| {
        format!(
            "create production evidence package dir {}",
            package_dir.display()
        )
    })?;
    let items_dir = package_dir.join("items");
    if include_items {
        fs::create_dir_all(&items_dir).with_context(|| {
            format!(
                "create production evidence package items dir {}",
                items_dir.display()
            )
        })?;
    }

    let request_path = package_dir.join(".1-production-evidence-handoff-package-request.json");
    let requirements_path = package_dir.join("1-production-evidence-requirements.json");
    let tasks_path = package_dir.join("2-production-evidence-tasks.json");
    let handoff_path = package_dir.join("3-production-evidence-handoff.json");
    let run_plan_path = package_dir.join("4-production-evidence-run-plan.json");
    let bundle_path = package_dir.join("5-production-evidence-bundle.json");
    let manifest_path = package_dir.join("6-production-evidence-package-manifest.json");
    let runner_script_path = package_dir.join("7-production-evidence-runner.sh");
    let runner_preflight_path = package_dir.join("8-production-evidence-runner-preflight.json");
    let snapshot_path = include_snapshot.then(|| package_dir.join("9-runtime-snapshot.json"));

    let request_value = json!({
        "kind": "pool_production_evidence_handoff_package_request",
        "project_slug": project_slug,
        "node_id": node_id,
        "title": title,
        "output_root": output_root,
        "source": source,
        "include_items": include_items,
        "include_snapshot": include_snapshot,
        "created_at": created_at,
        "local_files_authoritative": true,
        "provider_urls_are_provenance": true,
        "submit_endpoint": "/api/production-evidence/items",
    });
    write_server_json_file(&request_path, &request_value)?;
    write_server_json_file(&requirements_path, &requirements)?;
    write_server_json_file(&tasks_path, &tasks)?;
    write_server_json_file(&handoff_path, &handoff)?;
    let run_plan =
        production_evidence_run_plan_value(project_slug, Some(output_root), source, snapshot)?;
    write_server_json_file(&run_plan_path, &run_plan)?;
    write_server_json_file(&bundle_path, &bundle)?;
    let runner_script = production_evidence_runner_script(project_slug, output_root, &run_plan);
    write_server_text_file(&runner_script_path, &runner_script, true)?;
    let runner_preflight =
        production_evidence_runner_preflight(project_slug, output_root, &run_plan);
    write_server_json_file(&runner_preflight_path, &runner_preflight)?;
    if let Some(snapshot_path) = &snapshot_path {
        write_server_json_file(snapshot_path, &serde_json::to_value(snapshot)?)?;
    }

    let mut item_entries = Vec::new();
    let mut local_paths = vec![
        path_string_lossy(&request_path),
        path_string_lossy(&requirements_path),
        path_string_lossy(&tasks_path),
        path_string_lossy(&handoff_path),
        path_string_lossy(&run_plan_path),
        path_string_lossy(&bundle_path),
        path_string_lossy(&runner_script_path),
        path_string_lossy(&runner_preflight_path),
    ];
    if include_items {
        for (index, task) in tasks
            .get("tasks")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .enumerate()
        {
            let Some(task_id) = task.get("id").and_then(Value::as_str) else {
                continue;
            };
            let (kind, target_id) = production_evidence_selector_from_task_id(task_id)?;
            let item_template = production_evidence_item_template_value(
                project_slug,
                Some(output_root),
                source,
                &kind,
                &target_id,
                Some(task_id),
            )?;
            let item = item_template
                .get("item")
                .cloned()
                .unwrap_or_else(|| json!({}));
            let slug = production_evidence_file_slug(&format!("{kind}-{target_id}"));
            let item_path = items_dir.join(format!("{}-{}-item.json", index + 1, slug));
            let item_template_path =
                items_dir.join(format!(".{}-{}-item-template.json", index + 1, slug));
            write_server_json_file(&item_path, &item)?;
            write_server_json_file(&item_template_path, &item_template)?;
            local_paths.push(path_string_lossy(&item_path));
            local_paths.push(path_string_lossy(&item_template_path));
            item_entries.push(json!({
                "task_id": task_id,
                "kind": kind,
                "target_id": target_id,
                "bundle_path": task.get("bundle_path").cloned().unwrap_or_else(|| json!(null)),
                "preferred_control_profile": task.get("preferred_control_profile").cloned().unwrap_or_else(|| json!(null)),
                "bridge_worker": task.get("bridge_worker").cloned().unwrap_or_else(|| json!(null)),
                "item_path": path_string_lossy(&item_path),
                "template_path": path_string_lossy(&item_template_path),
                "submit_command": format!("pool-cli --project {project_slug} submit-production-evidence-item {}", path_string_lossy(&item_path)),
            }));
        }
    }
    if let Some(snapshot_path) = &snapshot_path {
        local_paths.push(path_string_lossy(snapshot_path));
    }

    let manifest = json!({
        "kind": "pool_production_evidence_handoff_package_manifest",
        "version": 1,
        "project_slug": project_slug,
        "node_id": node_id,
        "title": title,
        "created_at": created_at,
        "output_root": output_root,
        "source": source,
        "summary": {
            "local_files": local_paths.len() + 1,
            "item_templates": item_entries.len(),
            "missing_total": requirements.pointer("/summary/missing_total").cloned().unwrap_or_else(|| json!(0)),
        },
        "paths": {
            "request": path_string_lossy(&request_path),
            "requirements": path_string_lossy(&requirements_path),
            "tasks": path_string_lossy(&tasks_path),
            "handoff": path_string_lossy(&handoff_path),
            "run_plan": path_string_lossy(&run_plan_path),
            "bundle": path_string_lossy(&bundle_path),
            "runner_script": path_string_lossy(&runner_script_path),
            "runner_preflight": path_string_lossy(&runner_preflight_path),
            "manifest": path_string_lossy(&manifest_path),
            "snapshot": snapshot_path.as_ref().map(|path| path_string_lossy(path)),
        },
        "items": item_entries,
        "commands": {
            "tasks": format!("pool-cli --project {project_slug} production-evidence-tasks"),
            "run_plan": format!("pool-cli --project {project_slug} production-evidence-run-plan --output-root {output_root} {}", path_string_lossy(&run_plan_path)),
            "runner_script": path_string_lossy(&runner_script_path),
            "runner_preflight": format!("{} --preflight", path_string_lossy(&runner_script_path)),
            "closeout_bundle": format!("pool-cli --project {project_slug} closeout-production-evidence {}", path_string_lossy(&bundle_path)),
            "closeout_import": format!("pool-cli --project {project_slug} closeout-production-evidence --import {}", path_string_lossy(&bundle_path)),
            "validate_bundle": format!("pool-cli --project {project_slug} validate-production-evidence {}", path_string_lossy(&bundle_path)),
            "import_bundle": format!("pool-cli --project {project_slug} import-production-evidence {}", path_string_lossy(&bundle_path)),
            "readiness": format!("pool-cli --project {project_slug} prd-readiness"),
        },
        "operator_checklist": [
            "Run 7-production-evidence-runner.sh --preflight and inspect 8-production-evidence-runner-preflight.json before spending external Provider/software budget.",
            "Review and optionally execute 7-production-evidence-runner.sh to run provider/software/desktop evidence phases and closeout preflight.",
            "Assign each items/*-item.json file to the matching provider worker, software operator, or visual-controller operator.",
            "Read 4-production-evidence-run-plan.json first to follow the Provider/software/desktop vision execution order.",
            "Replace placeholder external ids with real upstream ids and write every artifact/metadata/trace path before submit.",
            "Use each item submit_command for incremental callbacks, or closeout/validate/import 5-production-evidence-bundle.json when closing the full batch.",
            "Do not mark production_upstream, production_software, or external_visual_model true unless the evidence came from the real external system."
        ],
    });
    write_server_json_file(&manifest_path, &manifest)?;
    local_paths.push(path_string_lossy(&manifest_path));

    Ok(json!({
        "status": "Succeeded",
        "project_slug": project_slug,
        "node_id": node_id,
        "title": title,
        "output_root": output_root,
        "source": source,
        "package_dir": path_string_lossy(package_dir),
        "request_path": path_string_lossy(&request_path),
        "requirements_path": path_string_lossy(&requirements_path),
        "tasks_path": path_string_lossy(&tasks_path),
        "handoff_path": path_string_lossy(&handoff_path),
        "run_plan_path": path_string_lossy(&run_plan_path),
        "bundle_path": path_string_lossy(&bundle_path),
        "runner_script_path": path_string_lossy(&runner_script_path),
        "runner_preflight_path": path_string_lossy(&runner_preflight_path),
        "provider_gateway_worker_start_commands": runner_preflight.pointer("/environment/provider_gateway_worker_start_commands").cloned().unwrap_or_else(|| json!([])),
        "software_bridge_worker": runner_preflight.pointer("/environment/software_bridge_worker").cloned().unwrap_or_else(|| json!(null)),
        "software_bridge_worker_start_commands": runner_preflight.pointer("/environment/software_bridge_worker_start_commands").cloned().unwrap_or_else(|| json!([])),
        "manifest_path": path_string_lossy(&manifest_path),
        "snapshot_path": snapshot_path.as_ref().map(|path| path_string_lossy(path)),
        "item_count": item_entries.len(),
        "items": item_entries,
        "local_paths": local_paths.clone(),
    }))
}

fn write_software_conformance_package(
    package_dir: &Path,
    project_slug: &str,
    node_id: Option<&str>,
    adapter_id: &str,
    title: &str,
    contract: &Value,
) -> Result<Value> {
    let created_at = chrono::Utc::now().to_rfc3339();
    let runbook = contract
        .get("conformance_runbook")
        .cloned()
        .unwrap_or_else(|| json!({ "phases": [] }));
    fs::create_dir_all(package_dir).with_context(|| {
        format!(
            "create software conformance package dir {}",
            package_dir.display()
        )
    })?;

    let request_path = package_dir.join(".1-software-conformance-package-request.json");
    let contract_path = package_dir.join("1-software-control-contract.json");
    let runbook_path = package_dir.join("2-software-conformance-runbook.json");
    let preflight_path = package_dir.join("3-software-conformance-preflight.json");
    let runner_script_path = package_dir.join("4-software-conformance-runner.sh");
    let manifest_path = package_dir.join("5-software-conformance-package-manifest.json");

    let request_value = json!({
        "kind": "pool_software_conformance_package_request",
        "project_slug": project_slug,
        "node_id": node_id,
        "adapter_id": adapter_id,
        "title": title,
        "created_at": created_at,
        "resources": [
            format!("pool://software-contracts/{adapter_id}"),
            "pool://software-contracts",
            "pool://software-actions",
            "pool://production-evidence-run-plan"
        ],
        "local_files_authoritative": true,
        "provider_urls_are_provenance": true,
    });
    let runbook_value = with_server_package_metadata(
        runbook.clone(),
        "software_conformance_runbook",
        project_slug,
        &created_at,
    );
    let contract_value = with_server_package_metadata(
        contract.clone(),
        "software_control_contract",
        project_slug,
        &created_at,
    );
    let runner_script = software_conformance_runner_script(project_slug, adapter_id, &runbook);
    let preflight = software_conformance_preflight(project_slug, adapter_id, &runbook);

    write_server_json_file(&request_path, &request_value)?;
    write_server_json_file(&contract_path, &contract_value)?;
    write_server_json_file(&runbook_path, &runbook_value)?;
    write_server_json_file(&preflight_path, &preflight)?;
    write_server_text_file(&runner_script_path, &runner_script, true)?;

    let local_paths = vec![
        path_string_lossy(&request_path),
        path_string_lossy(&contract_path),
        path_string_lossy(&runbook_path),
        path_string_lossy(&preflight_path),
        path_string_lossy(&runner_script_path),
        path_string_lossy(&manifest_path),
    ];
    let manifest = json!({
        "kind": "pool_software_conformance_package_manifest",
        "project_slug": project_slug,
        "node_id": node_id,
        "adapter_id": adapter_id,
        "title": title,
        "created_at": created_at,
        "paths": {
            "request": path_string_lossy(&request_path),
            "contract": path_string_lossy(&contract_path),
            "runbook": path_string_lossy(&runbook_path),
            "preflight": path_string_lossy(&preflight_path),
            "runner_script": path_string_lossy(&runner_script_path),
            "manifest": path_string_lossy(&manifest_path),
        },
        "commands": {
            "preflight": format!("{} --preflight", path_string_lossy(&runner_script_path)),
            "local_baseline": format!("{} local", path_string_lossy(&runner_script_path)),
            "run": format!("{} run", path_string_lossy(&runner_script_path)),
            "contract": format!("pool-cli --project {project_slug} software-contracts {adapter_id}")
        },
        "local_paths": local_paths,
        "next_actions": [
            "Run 4-software-conformance-runner.sh --preflight before using real software budget.",
            "Run 4-software-conformance-runner.sh local to verify the local bridge worker self-check.",
            "Set the upstream endpoint, production attestation, and local artifact env vars before running full conformance."
        ]
    });
    write_server_json_file(&manifest_path, &manifest)?;

    Ok(json!({
        "kind": "pool_software_conformance_package_report",
        "project_slug": project_slug,
        "node_id": node_id,
        "adapter_id": adapter_id,
        "title": title,
        "package_dir": path_string_lossy(package_dir),
        "local_paths": local_paths,
        "paths": manifest["paths"].clone(),
        "commands": manifest["commands"].clone(),
        "preflight": preflight,
    }))
}

fn write_provider_conformance_package(
    package_dir: &Path,
    project_slug: &str,
    node_id: Option<&str>,
    provider_id: &str,
    title: &str,
    contract: &Value,
    gateway_contract: &Value,
) -> Result<Value> {
    let created_at = chrono::Utc::now().to_rfc3339();
    let runbook = provider_conformance_runbook(project_slug, provider_id, contract);
    fs::create_dir_all(package_dir).with_context(|| {
        format!(
            "create provider conformance package dir {}",
            package_dir.display()
        )
    })?;

    let request_path = package_dir.join(".1-provider-conformance-package-request.json");
    let contract_path = package_dir.join("1-provider-contract.json");
    let gateway_contract_path = package_dir.join("2-provider-gateway-worker-contract.json");
    let runbook_path = package_dir.join("3-provider-conformance-runbook.json");
    let preflight_path = package_dir.join("4-provider-conformance-preflight.json");
    let runner_script_path = package_dir.join("5-provider-conformance-runner.sh");
    let manifest_path = package_dir.join("6-provider-conformance-package-manifest.json");

    let request_value = json!({
        "kind": "pool_provider_conformance_package_request",
        "project_slug": project_slug,
        "node_id": node_id,
        "provider_id": provider_id,
        "title": title,
        "created_at": created_at,
        "resources": [
            format!("pool://provider-contracts/{provider_id}"),
            "pool://provider-contracts",
            "pool://provider-gateway-worker",
            "pool://production-evidence-run-plan"
        ],
        "local_files_authoritative": true,
        "provider_urls_are_provenance": true,
        "high_cost_requires_approval": contract
            .get("high_cost")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    });
    let contract_value = with_server_package_metadata(
        contract.clone(),
        "provider_contract",
        project_slug,
        &created_at,
    );
    let gateway_contract_value = with_server_package_metadata(
        gateway_contract.clone(),
        "provider_gateway_worker_contract",
        project_slug,
        &created_at,
    );
    let runbook_value = with_server_package_metadata(
        runbook.clone(),
        "provider_conformance_runbook",
        project_slug,
        &created_at,
    );
    let runner_script = provider_conformance_runner_script(project_slug, provider_id, &runbook);
    let preflight = provider_conformance_preflight(project_slug, provider_id, &runbook);

    write_server_json_file(&request_path, &request_value)?;
    write_server_json_file(&contract_path, &contract_value)?;
    write_server_json_file(&gateway_contract_path, &gateway_contract_value)?;
    write_server_json_file(&runbook_path, &runbook_value)?;
    write_server_json_file(&preflight_path, &preflight)?;
    write_server_text_file(&runner_script_path, &runner_script, true)?;

    let local_paths = vec![
        path_string_lossy(&request_path),
        path_string_lossy(&contract_path),
        path_string_lossy(&gateway_contract_path),
        path_string_lossy(&runbook_path),
        path_string_lossy(&preflight_path),
        path_string_lossy(&runner_script_path),
        path_string_lossy(&manifest_path),
    ];
    let manifest = json!({
        "kind": "pool_provider_conformance_package_manifest",
        "project_slug": project_slug,
        "node_id": node_id,
        "provider_id": provider_id,
        "title": title,
        "created_at": created_at,
        "paths": {
            "request": path_string_lossy(&request_path),
            "contract": path_string_lossy(&contract_path),
            "gateway_worker_contract": path_string_lossy(&gateway_contract_path),
            "runbook": path_string_lossy(&runbook_path),
            "preflight": path_string_lossy(&preflight_path),
            "runner_script": path_string_lossy(&runner_script_path),
            "manifest": path_string_lossy(&manifest_path),
        },
        "commands": {
            "preflight": format!("{} --preflight", path_string_lossy(&runner_script_path)),
            "local_baseline": format!("{} local", path_string_lossy(&runner_script_path)),
            "run": format!("{} run", path_string_lossy(&runner_script_path)),
            "contract": format!("pool-cli --project {project_slug} provider-contracts {provider_id}"),
            "gateway_worker_contract": "pool-cli provider-gateway-worker-contract"
        },
        "local_paths": local_paths,
        "next_actions": [
            "Run 5-provider-conformance-runner.sh --preflight before using real Provider budget.",
            "Run 5-provider-conformance-runner.sh local to verify the Pool gateway worker baseline.",
            "Set endpoint, upstream, API key, and production attestation env vars before full conformance.",
            "Import production evidence only after outputs have been downloaded to local files."
        ]
    });
    write_server_json_file(&manifest_path, &manifest)?;

    Ok(json!({
        "kind": "pool_provider_conformance_package_report",
        "project_slug": project_slug,
        "node_id": node_id,
        "provider_id": provider_id,
        "title": title,
        "package_dir": path_string_lossy(package_dir),
        "local_paths": local_paths,
        "paths": manifest["paths"].clone(),
        "commands": manifest["commands"].clone(),
        "preflight": preflight,
    }))
}

fn write_agent_conformance_package(
    package_dir: &Path,
    project_slug: &str,
    node_id: Option<&str>,
    kind: &str,
    title: &str,
) -> Result<Value> {
    let created_at = chrono::Utc::now().to_rfc3339();
    let contract = agent_session_control_contract(project_slug);
    let runbook = agent_conformance_runbook(project_slug, kind);
    fs::create_dir_all(package_dir).with_context(|| {
        format!(
            "create agent conformance package dir {}",
            package_dir.display()
        )
    })?;

    let request_path = package_dir.join(".1-agent-conformance-package-request.json");
    let contract_path = package_dir.join("1-agent-session-contract.json");
    let runbook_path = package_dir.join("2-agent-conformance-runbook.json");
    let preflight_path = package_dir.join("3-agent-conformance-preflight.json");
    let runner_script_path = package_dir.join("4-agent-conformance-runner.sh");
    let manifest_path = package_dir.join("5-agent-conformance-package-manifest.json");

    let request_value = json!({
        "kind": "pool_agent_conformance_package_request",
        "project_slug": project_slug,
        "node_id": node_id,
        "session_kind": kind,
        "title": title,
        "created_at": created_at,
        "resources": [
            "pool://agent-sessions",
            "pool://runtime-handoff",
            "pool://runtime-execution-plan",
            "pool://workflow",
            "pool://tasks"
        ],
        "local_files_authoritative": true,
        "provider_urls_are_provenance": true,
        "secrets_stay_server_side": true,
    });
    let contract_value = with_server_package_metadata(
        contract.clone(),
        "agent_session_control_contract",
        project_slug,
        &created_at,
    );
    let runbook_value = with_server_package_metadata(
        runbook.clone(),
        "agent_conformance_runbook",
        project_slug,
        &created_at,
    );
    let preflight = agent_conformance_preflight(project_slug, kind, &runbook);
    let runner_script = agent_conformance_runner_script(project_slug, kind, &runbook);

    write_server_json_file(&request_path, &request_value)?;
    write_server_json_file(&contract_path, &contract_value)?;
    write_server_json_file(&runbook_path, &runbook_value)?;
    write_server_json_file(&preflight_path, &preflight)?;
    write_server_text_file(&runner_script_path, &runner_script, true)?;

    let local_paths = vec![
        path_string_lossy(&request_path),
        path_string_lossy(&contract_path),
        path_string_lossy(&runbook_path),
        path_string_lossy(&preflight_path),
        path_string_lossy(&runner_script_path),
        path_string_lossy(&manifest_path),
    ];
    let manifest = json!({
        "kind": "pool_agent_conformance_package_manifest",
        "project_slug": project_slug,
        "node_id": node_id,
        "session_kind": kind,
        "title": title,
        "created_at": created_at,
        "paths": {
            "request": path_string_lossy(&request_path),
            "contract": path_string_lossy(&contract_path),
            "runbook": path_string_lossy(&runbook_path),
            "preflight": path_string_lossy(&preflight_path),
            "runner_script": path_string_lossy(&runner_script_path),
            "manifest": path_string_lossy(&manifest_path),
        },
        "commands": {
            "preflight": format!("{} --preflight", path_string_lossy(&runner_script_path)),
            "local_baseline": format!("{} local", path_string_lossy(&runner_script_path)),
            "run": format!("{} run", path_string_lossy(&runner_script_path)),
            "stage_hermes": format!("pool-cli --project {project_slug} agent-session hermes --instruction \"inspect workflow context and coordinate Unreal handoff\" --allowed-tool api --allowed-tool mcp --allowed-tool unreal"),
            "stage_agent_cli": format!("pool-cli --project {project_slug} agent-session agent-cli --command-id workflow-context --title \"Inspect workflow context\" --command \"pool-cli --project {project_slug} workflow-context\" --tool cli --tool mcp --token-budget 74000")
        },
        "local_paths": local_paths,
        "next_actions": [
            "Run 4-agent-conformance-runner.sh --preflight before executing real Hermes HTTP.",
            "Run 4-agent-conformance-runner.sh local to verify Hermes bridge dry-run and Agent CLI allowlist execution.",
            "Set POOL_HERMES_ENDPOINT before full Hermes HTTP conformance.",
            "Use pool-cli agent-transcript and agent-stream with returned session ids to verify transcript and stream handoff."
        ]
    });
    write_server_json_file(&manifest_path, &manifest)?;

    Ok(json!({
        "kind": "pool_agent_conformance_package_report",
        "project_slug": project_slug,
        "node_id": node_id,
        "session_kind": kind,
        "title": title,
        "package_dir": path_string_lossy(package_dir),
        "local_paths": local_paths,
        "paths": manifest["paths"].clone(),
        "commands": manifest["commands"].clone(),
        "preflight": preflight,
    }))
}

fn write_integration_conformance_package(
    package_dir: &Path,
    project_slug: &str,
    node_id: Option<&str>,
    title: &str,
    providers: &[String],
    software_adapters: &[String],
    agent_kind: Option<&str>,
) -> Result<Value> {
    let created_at = chrono::Utc::now().to_rfc3339();
    fs::create_dir_all(package_dir).with_context(|| {
        format!(
            "create integration conformance package dir {}",
            package_dir.display()
        )
    })?;

    let request_path = package_dir.join(".1-integration-conformance-package-request.json");
    let runbook_path = package_dir.join("1-integration-conformance-runbook.json");
    let runner_script_path = package_dir.join("2-integration-conformance-runner.sh");
    let manifest_path = package_dir.join("3-integration-conformance-package-manifest.json");

    let gateway_contract = provider_gateway_worker_contract();
    let mut provider_reports = Vec::new();
    let mut software_reports = Vec::new();
    let mut local_paths = vec![
        path_string_lossy(&request_path),
        path_string_lossy(&runbook_path),
        path_string_lossy(&runner_script_path),
        path_string_lossy(&manifest_path),
    ];

    for provider_id in providers {
        let contract = provider_contracts_resource(Some(provider_id))
            .with_context(|| format!("read provider conformance contract for {provider_id}"))?;
        let canonical_provider_id = contract["provider_id"]
            .as_str()
            .unwrap_or(provider_id.as_str())
            .to_string();
        let provider_dir = package_dir
            .join("providers")
            .join(safe_package_segment(&canonical_provider_id));
        let report = write_provider_conformance_package(
            &provider_dir,
            project_slug,
            node_id,
            &canonical_provider_id,
            &format!("Pool provider conformance package: {canonical_provider_id}"),
            &contract,
            &gateway_contract,
        )?;
        extend_local_paths_from_report(&mut local_paths, &report);
        provider_reports.push(report);
    }

    for adapter_id in software_adapters {
        let contract = software_control_contract(adapter_id)
            .with_context(|| format!("read software conformance contract for {adapter_id}"))?;
        let canonical_adapter_id = contract["adapter_id"]
            .as_str()
            .unwrap_or(adapter_id.as_str())
            .to_string();
        let software_dir = package_dir
            .join("software")
            .join(safe_package_segment(&canonical_adapter_id));
        let report = write_software_conformance_package(
            &software_dir,
            project_slug,
            node_id,
            &canonical_adapter_id,
            &format!("Pool software conformance package: {canonical_adapter_id}"),
            &contract,
        )?;
        extend_local_paths_from_report(&mut local_paths, &report);
        software_reports.push(report);
    }

    let agent_report = if let Some(kind) = agent_kind {
        let agent_dir = package_dir.join("agent").join(safe_package_segment(kind));
        let report = write_agent_conformance_package(
            &agent_dir,
            project_slug,
            node_id,
            kind,
            &format!("Pool Agent/Hermes conformance package: {kind}"),
        )?;
        extend_local_paths_from_report(&mut local_paths, &report);
        Some(report)
    } else {
        None
    };

    let request_value = json!({
        "kind": "pool_integration_conformance_package_request",
        "project_slug": project_slug,
        "node_id": node_id,
        "title": title,
        "created_at": created_at,
        "providers": providers,
        "software_adapters": software_adapters,
        "agent_kind": agent_kind,
        "local_files_authoritative": true,
        "provider_urls_are_provenance": true,
        "secrets_stay_server_side": true,
    });
    let runbook = integration_conformance_runbook(
        project_slug,
        providers,
        software_adapters,
        agent_kind,
        &provider_reports,
        &software_reports,
        agent_report.as_ref(),
    );
    let runbook_value = with_server_package_metadata(
        runbook,
        "integration_conformance_runbook",
        project_slug,
        &created_at,
    );
    let runner_script = integration_conformance_runner_script(
        &provider_reports,
        &software_reports,
        agent_report.as_ref(),
    );

    write_server_json_file(&request_path, &request_value)?;
    write_server_json_file(&runbook_path, &runbook_value)?;
    write_server_text_file(&runner_script_path, &runner_script, true)?;

    let manifest = json!({
        "kind": "pool_integration_conformance_package_manifest",
        "project_slug": project_slug,
        "node_id": node_id,
        "title": title,
        "created_at": created_at,
        "summary": {
            "providers": provider_reports.len(),
            "software_adapters": software_reports.len(),
            "agent": agent_report.is_some(),
            "local_files": local_paths.len(),
        },
        "paths": {
            "request": path_string_lossy(&request_path),
            "runbook": path_string_lossy(&runbook_path),
            "runner_script": path_string_lossy(&runner_script_path),
            "manifest": path_string_lossy(&manifest_path),
        },
        "commands": {
            "preflight": format!("{} --preflight", path_string_lossy(&runner_script_path)),
            "local_baseline": format!("{} local", path_string_lossy(&runner_script_path)),
            "run": format!("{} run", path_string_lossy(&runner_script_path)),
        },
        "packages": {
            "providers": provider_reports,
            "software_adapters": software_reports,
            "agent": agent_report,
        },
        "local_paths": local_paths,
    });
    write_server_json_file(&manifest_path, &manifest)?;

    Ok(json!({
        "kind": "pool_integration_conformance_package_report",
        "project_slug": project_slug,
        "node_id": node_id,
        "title": title,
        "package_dir": path_string_lossy(package_dir),
        "summary": manifest["summary"].clone(),
        "local_paths": local_paths,
        "paths": manifest["paths"].clone(),
        "commands": manifest["commands"].clone(),
    }))
}

fn integration_conformance_runbook(
    project_slug: &str,
    providers: &[String],
    software_adapters: &[String],
    agent_kind: Option<&str>,
    provider_reports: &[Value],
    software_reports: &[Value],
    agent_report: Option<&Value>,
) -> Value {
    json!({
        "kind": "pool_integration_conformance_runbook",
        "project_slug": project_slug,
        "purpose": "One local handoff package for validating Pool's AI/3DGS Providers, external software adapters, and Agent/Hermes control boundary.",
        "scope": {
            "providers": providers,
            "software_adapters": software_adapters,
            "agent_kind": agent_kind,
        },
        "team_lanes": [
            {
                "role": "tech_lead",
                "owns": "Run top-level preflight, assign failed package sections, and verify all local manifests are indexed."
            },
            {
                "role": "provider_worker",
                "owns": "AI media and 3DGS provider SDK/HTTP workers, API keys, endpoints, output downloads, and provider production evidence."
            },
            {
                "role": "engine_operator",
                "owns": "Unreal, Blender, Resolve, TouchDesigner, MadMapper, Unity, Nuke, motion database, and editing-suite bridge validation."
            },
            {
                "role": "agent_operator",
                "owns": "Hermes endpoint, Agent CLI allowlist, transcript/stream readback, and approval/retry resume."
            }
        ],
        "execution_order": [
            "Run 2-integration-conformance-runner.sh --preflight.",
            "Run 2-integration-conformance-runner.sh local for local bridge/gateway/allowlist smoke.",
            "Fix each blocked child package by following its own runbook.",
            "After real endpoints and attestations are configured, run child package scripts in run mode by lane.",
            "Use production-evidence-handoff-package for final production evidence closeout."
        ],
        "child_packages": {
            "providers": provider_reports.iter().map(integration_child_package_summary).collect::<Vec<_>>(),
            "software_adapters": software_reports.iter().map(integration_child_package_summary).collect::<Vec<_>>(),
            "agent": agent_report.map(integration_child_package_summary),
        },
        "pass_conditions": [
            "Every required child package has request, contract, runbook, preflight, runner, and manifest files.",
            "Provider package runner local baselines pass before real vendor SDK/HTTP workers are attached.",
            "Software package runner local baselines pass before real app bridge endpoints or allowlisted CLI commands are attached.",
            "Agent/Hermes package records transcript and stream evidence for staged and executed sessions.",
            "All child package files are indexed as local assets; remote URLs remain provenance only."
        ],
        "next_runtime_steps": [
            "pool-cli production-evidence-handoff-package --output-dir worlds/demo/output --output-root worlds/demo/output/production-evidence --include-snapshot",
            "pool-cli prd-readiness",
            "pool-cli prd-completion-gate --require-complete"
        ]
    })
}

fn integration_child_package_summary(report: &Value) -> Value {
    json!({
        "kind": report["kind"].clone(),
        "provider_id": report.get("provider_id").cloned(),
        "adapter_id": report.get("adapter_id").cloned(),
        "session_kind": report.get("session_kind").cloned(),
        "package_dir": report["package_dir"].clone(),
        "runner_script": report["paths"]["runner_script"].clone(),
        "manifest": report["paths"]["manifest"].clone(),
        "preflight": report["commands"]["preflight"].clone(),
    })
}

fn integration_conformance_runner_script(
    provider_reports: &[Value],
    software_reports: &[Value],
    agent_report: Option<&Value>,
) -> String {
    let mut runner_paths = Vec::new();
    for report in provider_reports.iter().chain(software_reports.iter()) {
        if let Some(path) = report["paths"]["runner_script"].as_str() {
            runner_paths.push(path.to_string());
        }
    }
    if let Some(report) = agent_report {
        if let Some(path) = report["paths"]["runner_script"].as_str() {
            runner_paths.push(path.to_string());
        }
    }
    let runner_array = runner_paths
        .iter()
        .map(|path| format!("  {}", shell_single_quote(path)))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"#!/usr/bin/env bash
set -euo pipefail

RUNNER_MODE="${{1:-preflight}}"
RUNNERS=(
{runner_array}
)

case "$RUNNER_MODE" in
  preflight|--preflight)
    RUNNER_ARG="--preflight"
    ;;
  local|local-baseline)
    RUNNER_ARG="local"
    ;;
  run)
    RUNNER_ARG="run"
    ;;
  *)
    echo "Usage: $0 [preflight|--preflight|local|run]"
    exit 64
    ;;
esac

FAILED=0
for runner in "${{RUNNERS[@]}}"; do
  echo
  echo "+ $runner $RUNNER_ARG"
  if ! "$runner" "$RUNNER_ARG"; then
    FAILED=1
  fi
done

exit "$FAILED"
"#
    )
}

fn extend_local_paths_from_report(local_paths: &mut Vec<String>, report: &Value) {
    if let Some(paths) = report["local_paths"].as_array() {
        local_paths.extend(
            paths
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string),
        );
    }
}

fn agent_session_control_contract(project_slug: &str) -> Value {
    json!({
        "kind": "pool_agent_session_control_contract",
        "version": 1,
        "project_slug": project_slug,
        "purpose": "Auditable Hermes embedded control and Agent CLI execution boundary for Pool runtime tasks.",
        "runtime_routes": {
            "create_session": {
                "method": "POST",
                "path": "/api/agent-sessions",
                "body_kinds": ["hermes", "agent_cli"]
            },
            "transcript": {
                "method": "GET",
                "path": "/api/agent-sessions/transcript?session_id=<agent-session-id>"
            },
            "stream": {
                "method": "GET",
                "path": "/api/agent-sessions/stream?session_id=<agent-session-id>"
            },
            "websocket": {
                "method": "GET",
                "path": "/api/agent-sessions/ws?session_id=<agent-session-id>"
            },
            "approval_resume": {
                "approve": "/api/tasks/approve?task_id=<task-id>",
                "retry": "/api/tasks/retry?task_id=<task-id>"
            }
        },
        "session_kinds": {
            "hermes": {
                "required": ["instruction"],
                "optional": ["endpoint", "allowed_tools", "requires_confirmation", "execute", "timeout_ms", "max_output_bytes"],
                "execution_channel": "hermes_http",
                "default_endpoint": "http://127.0.0.1:8787/hermes",
                "auth": "Runtime may attach server-side Bearer token from hermes/agent or hermes/provider API key records."
            },
            "agent_cli": {
                "required": ["command"],
                "optional": ["command_id", "title", "tools", "token_budget", "execute", "allowed_commands", "working_dir", "timeout_ms", "max_output_bytes"],
                "execution_channel": "agent_cli",
                "security": "Command is parsed and executed without shell only when the binary matches allowed_commands."
            }
        },
        "mcp_tools": [
            "pool_agent_session",
            "pool_agent_transcript",
            "pool_agent_stream",
            "pool_runtime_handoff",
            "pool_runtime_execution_plan"
        ],
        "cli": {
            "stage_hermes": format!("pool-cli --project {project_slug} agent-session hermes --instruction \"inspect workflow context\" --allowed-tool api --allowed-tool mcp"),
            "execute_hermes": format!("pool-cli --project {project_slug} agent-session hermes --endpoint $POOL_HERMES_ENDPOINT --instruction \"inspect workflow context\" --execute --timeout-ms 2000"),
            "stage_agent_cli": format!("pool-cli --project {project_slug} agent-session agent-cli --command-id workflow-context --title \"Inspect workflow context\" --command \"pool-cli --project {project_slug} workflow-context\" --tool cli --tool mcp --token-budget 74000"),
            "execute_agent_cli": format!("pool-cli --project {project_slug} agent-session agent-cli --command-id echo --title \"Agent CLI allowlist smoke\" --command \"/bin/echo pool-agent-ok\" --tool cli --execute --allowed-command /bin/echo --allowed-command echo --timeout-ms 2000")
        },
        "policy": {
            "local_files_authoritative": true,
            "transcript_required": true,
            "agent_cli_shell": false,
            "interactive_tui": false,
            "secrets_stay_server_side": true,
            "approval_resume_uses_original_task": true
        }
    })
}

fn agent_conformance_runbook(project_slug: &str, kind: &str) -> Value {
    json!({
        "purpose": "Prove Hermes embedded control and Agent CLI sessions can be staged, executed, streamed, and resumed through Pool's auditable runtime boundary.",
        "session_kind": kind,
        "endpoint_env": "POOL_HERMES_ENDPOINT",
        "phases": [
            {
                "id": "context_readiness",
                "command": format!("pool-cli --project {project_slug} runtime-handoff && pool-cli --project {project_slug} runtime-execution-plan"),
                "proves": ["Agent can read current runtime state", "execution plan and handoff are available before acting"],
                "requires_hermes_endpoint": false
            },
            {
                "id": "local_hermes_bridge_baseline",
                "command": "pool-cli hermes-mcp-bridge-worker --once --output-root target/agent-conformance/hermes-bridge",
                "proves": ["pool_hermes_action wrapper", "mcp_payload validation", "local request/response audit files"],
                "requires_hermes_endpoint": false
            },
            {
                "id": "stage_hermes",
                "command": format!("pool-cli --project {project_slug} agent-session hermes --instruction \"inspect workflow context and coordinate Unreal handoff\" --allowed-tool api --allowed-tool mcp --allowed-tool unreal"),
                "proves": ["Hermes session staging", "task and transcript creation", "allowed tool recording"],
                "requires_hermes_endpoint": false
            },
            {
                "id": "execute_hermes_http",
                "command": format!("pool-cli --project {project_slug} agent-session hermes --endpoint <hermes-endpoint> --instruction \"inspect workflow context and coordinate Unreal handoff\" --allowed-tool api --allowed-tool mcp --allowed-tool unreal --execute --timeout-ms 2000"),
                "proves": ["Hermes HTTP execution", "server-side auth boundary", "execution result appended to transcript"],
                "requires_hermes_endpoint": true
            },
            {
                "id": "stage_agent_cli",
                "command": format!("pool-cli --project {project_slug} agent-session agent-cli --command-id workflow-context --title \"Inspect workflow context\" --command \"pool-cli --project {project_slug} workflow-context\" --tool cli --tool mcp --token-budget 74000"),
                "proves": ["Agent CLI session staging", "token budget recording", "transcript creation"],
                "requires_hermes_endpoint": false
            },
            {
                "id": "execute_agent_cli_allowlist",
                "command": format!("pool-cli --project {project_slug} agent-session agent-cli --command-id echo --title \"Agent CLI allowlist smoke\" --command \"/bin/echo pool-agent-ok\" --tool cli --execute --allowed-command /bin/echo --allowed-command echo --timeout-ms 2000"),
                "proves": ["non-shell CLI execution", "allowed command enforcement", "stdout/stderr/exit code transcript"],
                "requires_hermes_endpoint": false
            },
            {
                "id": "transcript_and_stream",
                "command": format!("pool-cli --project {project_slug} agent-transcript <agent-session-id> && pool-cli --project {project_slug} agent-stream <agent-session-id> --limit 24"),
                "proves": ["transcript readback", "SSE stream slice", "handoff to Web/Hermes/Agent readers"],
                "requires_hermes_endpoint": false
            },
            {
                "id": "approval_resume",
                "command": "pool-cli --project demo approve-task <waiting-agent-task-id> || pool-cli --project demo retry-task <failed-agent-task-id>",
                "proves": ["waiting approval task release", "execution_request resume", "same task lineage"],
                "requires_hermes_endpoint": false
            }
        ],
        "pass_conditions": [
            "Every staged or executed Agent session writes a local transcript path.",
            "Hermes HTTP execution uses Runtime-side endpoint/auth handling, not browser direct provider calls.",
            "Agent CLI execution uses an explicit allowlist and does not invoke a shell.",
            "Transcript, SSE stream, and WebSocket fallback expose the same session id and task lineage.",
            "Approval or retry resumes from the transcript execution_request when execution was blocked."
        ],
        "failure_conditions": [
            "Agent CLI command requires pipes, redirection, a shell, or an interactive TUI.",
            "Hermes endpoint is called from browser code instead of Runtime HTTP.",
            "Secrets are written into transcripts, request JSON, or frontend state.",
            "A waiting approval Agent task is resumed as a different task instead of preserving lineage."
        ]
    })
}

fn agent_conformance_preflight(project_slug: &str, kind: &str, runbook: &Value) -> Value {
    json!({
        "kind": "pool_agent_conformance_preflight",
        "project_slug": project_slug,
        "session_kind": kind,
        "runner": "4-agent-conformance-runner.sh",
        "required_env": {
            "hermes_endpoint": "POOL_HERMES_ENDPOINT",
            "pool_cli_cmd": "POOL_CLI_CMD optional override"
        },
        "phases": runbook.get("phases").cloned().unwrap_or_else(|| json!([])),
        "pass_conditions": runbook.get("pass_conditions").cloned().unwrap_or_else(|| json!([])),
        "failure_conditions": [
            "POOL_CLI_CMD falls back to cargo but cargo is unavailable",
            "POOL_HERMES_ENDPOINT is missing before full run mode",
            "Agent CLI smoke command is not allowlisted",
            "transcript path is not written as a local file"
        ],
        "preflight_contract": {
            "local_mode": "runner local executes Hermes bridge baseline and Agent CLI allowlist smoke without a real Hermes endpoint",
            "run_mode": "runner run executes all conformance phases after env preflight passes",
            "truth_source": "pool://agent-sessions"
        }
    })
}

fn agent_conformance_runner_script(project_slug: &str, kind: &str, runbook: &Value) -> String {
    let local_bridge_command =
        production_evidence_phase_command(runbook, "local_hermes_bridge_baseline").unwrap_or_else(
            || {
                "pool-cli hermes-mcp-bridge-worker --once --output-root target/agent-conformance/hermes-bridge"
                    .to_string()
            },
        );
    let context_command = production_evidence_phase_command(runbook, "context_readiness")
        .unwrap_or_else(|| format!("pool-cli --project {project_slug} runtime-handoff"));
    let stage_hermes_command = production_evidence_phase_command(runbook, "stage_hermes")
        .unwrap_or_else(|| {
            format!("pool-cli --project {project_slug} agent-session hermes --instruction \"inspect workflow context\" --allowed-tool api --allowed-tool mcp")
        });
    let execute_hermes_command =
        production_evidence_phase_command(runbook, "execute_hermes_http").unwrap_or_else(|| {
            format!("pool-cli --project {project_slug} agent-session hermes --endpoint <hermes-endpoint> --instruction \"inspect workflow context\" --execute --timeout-ms 2000")
        });
    let stage_cli_command = production_evidence_phase_command(runbook, "stage_agent_cli")
        .unwrap_or_else(|| {
            format!("pool-cli --project {project_slug} agent-session agent-cli --command-id workflow-context --title \"Inspect workflow context\" --command \"pool-cli --project {project_slug} workflow-context\" --tool cli --tool mcp --token-budget 74000")
        });
    let execute_cli_command =
        production_evidence_phase_command(runbook, "execute_agent_cli_allowlist").unwrap_or_else(
            || {
                format!("pool-cli --project {project_slug} agent-session agent-cli --command-id echo --title \"Agent CLI allowlist smoke\" --command \"/bin/echo pool-agent-ok\" --tool cli --execute --allowed-command /bin/echo --allowed-command echo --timeout-ms 2000")
            },
        );

    format!(
        r#"#!/usr/bin/env bash
set -euo pipefail

PROJECT={project_slug}
SESSION_KIND={kind}
HERMES_ENDPOINT_ENV=POOL_HERMES_ENDPOINT
CONTEXT_CMD={context_command}
LOCAL_BRIDGE_CMD={local_bridge_command}
STAGE_HERMES_CMD={stage_hermes_command}
EXECUTE_HERMES_CMD={execute_hermes_command}
STAGE_CLI_CMD={stage_cli_command}
EXECUTE_CLI_CMD={execute_cli_command}
RUNNER_MODE="${{1:-preflight}}"
POOL_CLI_CMD="${{POOL_CLI_CMD:-}}"

run_cmd() {{
  echo
  echo "+ $*"
  eval "$*"
}}

rewrite_pool_cli_cmd() {{
  local command="$1"
  if [[ "$command" == pool-cli\ * ]]; then
    printf '%s %s' "$POOL_CLI_CMD" "${{command#pool-cli }}"
  else
    printf '%s' "$command"
  fi
}}

if [[ -z "$POOL_CLI_CMD" ]]; then
  if command -v pool-cli >/dev/null 2>&1; then
    POOL_CLI_CMD="pool-cli"
  else
    POOL_CLI_CMD="cargo run -q -p pool-cli --"
  fi
fi

CONTEXT_CMD="$(rewrite_pool_cli_cmd "$CONTEXT_CMD")"
LOCAL_BRIDGE_CMD="$(rewrite_pool_cli_cmd "$LOCAL_BRIDGE_CMD")"
STAGE_HERMES_CMD="$(rewrite_pool_cli_cmd "$STAGE_HERMES_CMD")"
EXECUTE_HERMES_CMD="$(rewrite_pool_cli_cmd "$EXECUTE_HERMES_CMD")"
STAGE_CLI_CMD="$(rewrite_pool_cli_cmd "$STAGE_CLI_CMD")"
EXECUTE_CLI_CMD="$(rewrite_pool_cli_cmd "$EXECUTE_CLI_CMD")"

runner_preflight() {{
  local missing=0
  echo "Pool Agent/Hermes conformance preflight"
  echo "project=$PROJECT"
  echo "session_kind=$SESSION_KIND"
  echo "pool_cli_cmd=$POOL_CLI_CMD"

  if [[ "$POOL_CLI_CMD" == cargo\ * ]] && ! command -v cargo >/dev/null 2>&1; then
    echo "MISSING cargo command on PATH for POOL_CLI_CMD cargo fallback"
    missing=1
  fi
  if [[ -z "${{!HERMES_ENDPOINT_ENV:-}}" ]]; then
    echo "MISSING Hermes endpoint env for run mode: $HERMES_ENDPOINT_ENV"
    missing=1
  fi

  if [[ "$missing" == "0" ]]; then
    echo "preflight_status=ready"
  else
    echo "preflight_status=blocked"
  fi
  return "$missing"
}}

if [[ "$RUNNER_MODE" == "--preflight" || "$RUNNER_MODE" == "preflight" ]]; then
  runner_preflight
  exit $?
fi

if [[ "$RUNNER_MODE" == "local" || "$RUNNER_MODE" == "local-baseline" ]]; then
  run_cmd "$LOCAL_BRIDGE_CMD"
  run_cmd "$EXECUTE_CLI_CMD"
  exit 0
fi

if [[ "$RUNNER_MODE" != "run" ]]; then
  echo "Usage: $0 [preflight|--preflight|local|run]"
  exit 64
fi

runner_preflight
HERMES_ENDPOINT="${{!HERMES_ENDPOINT_ENV}}"
EXECUTE_HERMES_CMD="${{EXECUTE_HERMES_CMD//<hermes-endpoint>/$HERMES_ENDPOINT}}"

run_cmd "$CONTEXT_CMD"
run_cmd "$LOCAL_BRIDGE_CMD"
run_cmd "$STAGE_HERMES_CMD"
run_cmd "$EXECUTE_HERMES_CMD"
run_cmd "$STAGE_CLI_CMD"
run_cmd "$EXECUTE_CLI_CMD"

echo
echo "Read returned session ids with:"
echo "  $POOL_CLI_CMD --project $PROJECT agent-transcript <agent-session-id>"
echo "  $POOL_CLI_CMD --project $PROJECT agent-stream <agent-session-id> --limit 24"
"#,
        project_slug = shell_single_quote(project_slug),
        kind = shell_single_quote(kind),
        context_command = shell_single_quote(&context_command),
        local_bridge_command = shell_single_quote(&local_bridge_command),
        stage_hermes_command = shell_single_quote(&stage_hermes_command),
        execute_hermes_command = shell_single_quote(&execute_hermes_command),
        stage_cli_command = shell_single_quote(&stage_cli_command),
        execute_cli_command = shell_single_quote(&execute_cli_command),
    )
}

fn conformance_request_items(items: Option<Vec<String>>, defaults: &[&str]) -> Vec<String> {
    items
        .unwrap_or_else(|| defaults.iter().map(|item| (*item).to_string()).collect())
        .into_iter()
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .collect()
}

fn normalize_agent_conformance_kind(kind: Option<&str>) -> Result<String> {
    let normalized = kind.unwrap_or("all").trim().to_ascii_lowercase();
    match normalized.as_str() {
        "" | "all" | "agent" | "agent-hermes" | "hermes-agent" => Ok("all".to_string()),
        "hermes" => Ok("hermes".to_string()),
        "agent_cli" | "agent-cli" | "cli" => Ok("agent-cli".to_string()),
        value => bail!("unknown agent conformance kind: {value}"),
    }
}

fn provider_conformance_runbook(project_slug: &str, provider_id: &str, contract: &Value) -> Value {
    let family = provider_conformance_family(contract);
    let endpoint_env = provider_conformance_endpoint_env(provider_id, &family);
    let upstream_envs = provider_conformance_upstream_envs(provider_id, &family);
    let upstream_display = upstream_envs.join(" or ");
    let api_key_env = format!("POOL_{}_API_KEY", provider_env_token(provider_id));
    let attestation_env = provider_conformance_attestation_env(provider_id);
    let execution_mode = if family == "native" {
        "adapter"
    } else {
        "gateway"
    };
    let smoke_input = "worlds/demo/source/0-reference.png";
    let smoke_output = "worlds/demo/output";
    let smoke_prompt = format!("Pool conformance smoke for {provider_id}");
    let real_upstream_command = if family == "native" {
        format!(
            "{endpoint_env}=<real-provider-endpoint> pool-cli --project {project_slug} provider-health {provider_id} --execution-mode adapter --api-key-env {api_key_env}"
        )
    } else {
        format!(
            "pool-cli provider-gateway-worker --bind 127.0.0.1:8788 --provider-upstream {provider_id}=<real-worker-or-sdk-url> --provider-api-key-env {provider_id}={api_key_env}"
        )
    };
    let health_command = format!(
        "{endpoint_env}=http://127.0.0.1:8788 pool-cli --project {project_slug} provider-health {provider_id} --execution-mode {execution_mode} --endpoint http://127.0.0.1:8788 --api-key-env {api_key_env}"
    );
    let smoke_command = format!(
        "{endpoint_env}=http://127.0.0.1:8788 pool-cli --project {project_slug} run-provider {provider_id} --execution-mode {execution_mode} --endpoint http://127.0.0.1:8788 --api-key-env {api_key_env} --prompt '{}' --input {smoke_input} --output-dir {smoke_output} --no-approval",
        smoke_prompt.replace('\'', "'\\''")
    );
    let production_command = format!(
        "{endpoint_env}=http://127.0.0.1:8788 {attestation_env}=<real-provider-run-id> pool-cli --project {project_slug} production-evidence-provider-matrix target/provider-evidence-matrix --production-upstream --provider-endpoint-env {provider_id}={endpoint_env} --provider-api-key-env {provider_id}={api_key_env} --provider-attestation-env {provider_id}={attestation_env} --evidence-bundle=target/provider-evidence-matrix/provider-production-evidence-bundle.json"
    );

    json!({
        "purpose": format!("Prove {provider_id} satisfies Pool Provider adapter and local file contracts before it is used as production evidence."),
        "provider_id": provider_id,
        "gateway_family": family,
        "endpoint_env": endpoint_env,
        "upstream_envs": upstream_envs,
        "api_key_env": api_key_env,
        "production_attestation_env": attestation_env,
        "upstream_env_display": upstream_display,
        "phases": [
            {
                "id": "local_gateway_baseline",
                "command": "pool-cli provider-gateway-worker --once",
                "proves": ["Pool gateway routes", "template translation", "Pool-compatible submit/poll normalization"],
                "production_evidence": false
            },
            {
                "id": "real_upstream_worker",
                "command": real_upstream_command,
                "proves": ["real upstream reachability", "server-side bearer forwarding", "provider URL provenance boundary"],
                "production_evidence": false
            },
            {
                "id": "provider_health",
                "command": health_command,
                "proves": ["Provider adapter health path", "endpoint env wiring", "credential lookup without browser secret exposure"],
                "production_evidence": false
            },
            {
                "id": "provider_smoke",
                "command": smoke_command,
                "proves": ["submit", "poll", "download to local asset", "metadata ledger"],
                "production_evidence": false
            },
            {
                "id": "production_matrix",
                "command": production_command,
                "proves": ["per-provider production evidence", "non-mock upstream attestation", "local artifact existence", "production metadata paths"],
                "production_evidence": true
            },
            {
                "id": "validate_and_import",
                "command": "pool-cli --project demo validate-production-evidence target/provider-evidence-matrix/provider-production-evidence-bundle.json && pool-cli --project demo import-production-evidence target/provider-evidence-matrix/provider-production-evidence-bundle.json",
                "proves": ["template ids rejected before import", "remote URLs remain provenance only", "PRD readiness can consume imported evidence"],
                "production_evidence": true
            }
        ],
        "pass_conditions": [
            "Every upstream response contains a job id or equivalent id field.",
            "Poll returns a completed/succeeded status before Pool downloads outputs.",
            "All outputs are downloadable by Pool into local files; remote URLs are not used as frontend truth sources.",
            "local_input_manifest is consumed by the real upstream worker when input media is required.",
            "Production evidence uses a real non-placeholder attestation and local artifact paths."
        ],
        "failure_conditions": [
            "Provider evidence is produced by provider_gateway_mock_server or provider_sdk_worker_template without a real upstream run.",
            "Upstream only returns a remote URL that Pool cannot download.",
            "API keys are echoed into response metadata or production evidence bundles.",
            "Input references are remote URLs instead of local paths or local_input_manifest entries."
        ]
    })
}

fn provider_conformance_preflight(project_slug: &str, provider_id: &str, runbook: &Value) -> Value {
    let endpoint_env = runbook
        .get("endpoint_env")
        .and_then(Value::as_str)
        .unwrap_or("POOL_PROVIDER_ENDPOINT");
    let upstream_envs = runbook
        .get("upstream_envs")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let api_key_env = runbook
        .get("api_key_env")
        .and_then(Value::as_str)
        .unwrap_or("POOL_PROVIDER_API_KEY");
    let attestation_env = runbook
        .get("production_attestation_env")
        .and_then(Value::as_str)
        .unwrap_or("POOL_PROVIDER_PRODUCTION_ATTESTATION");

    json!({
        "kind": "pool_provider_conformance_preflight",
        "project_slug": project_slug,
        "provider_id": provider_id,
        "runner": "5-provider-conformance-runner.sh",
        "required_env": {
            "endpoint": endpoint_env,
            "upstream": upstream_envs,
            "api_key": api_key_env,
            "production_attestation": attestation_env,
        },
        "phases": runbook.get("phases").cloned().unwrap_or_else(|| json!([])),
        "pass_conditions": runbook.get("pass_conditions").cloned().unwrap_or_else(|| json!([])),
        "failure_conditions": [
            "missing endpoint env for provider health/smoke",
            "missing upstream endpoint env for real_upstream_worker",
            "missing production attestation env for production_matrix",
            "production bundle references remote URLs as truth instead of local files",
            "POOL_CLI_CMD falls back to cargo but cargo is unavailable"
        ],
        "preflight_contract": {
            "local_mode": "runner local executes only local_gateway_baseline",
            "run_mode": "runner run executes all conformance phases after env preflight passes",
            "truth_source": format!("pool://provider-contracts/{provider_id}")
        }
    })
}

fn provider_conformance_runner_script(
    project_slug: &str,
    provider_id: &str,
    runbook: &Value,
) -> String {
    let endpoint_env = runbook
        .get("endpoint_env")
        .and_then(Value::as_str)
        .unwrap_or("POOL_PROVIDER_ENDPOINT");
    let upstream_envs = runbook
        .get("upstream_envs")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let upstream_env_args = shell_env_args(&upstream_envs);
    let upstream_env_display = upstream_envs.join(" or ");
    let api_key_env = runbook
        .get("api_key_env")
        .and_then(Value::as_str)
        .unwrap_or("POOL_PROVIDER_API_KEY");
    let attestation_env = runbook
        .get("production_attestation_env")
        .and_then(Value::as_str)
        .unwrap_or("POOL_PROVIDER_PRODUCTION_ATTESTATION");
    let local_gateway_command =
        production_evidence_phase_command(runbook, "local_gateway_baseline")
            .unwrap_or_else(|| "pool-cli provider-gateway-worker --once".to_string());
    let real_upstream_command = production_evidence_phase_command(runbook, "real_upstream_worker")
        .unwrap_or_else(|| {
            format!("pool-cli provider-gateway-worker --bind 127.0.0.1:8788 --provider-upstream {provider_id}=<real-worker-or-sdk-url>")
        });
    let health_command = production_evidence_phase_command(runbook, "provider_health")
        .unwrap_or_else(|| {
            format!("pool-cli --project {project_slug} provider-health {provider_id}")
        });
    let smoke_command = production_evidence_phase_command(runbook, "provider_smoke")
        .unwrap_or_else(|| format!("pool-cli --project {project_slug} run-provider {provider_id}"));
    let production_command = production_evidence_phase_command(runbook, "production_matrix")
        .unwrap_or_else(|| {
            format!(
                "pool-cli --project {project_slug} production-evidence-provider-matrix target/provider-evidence-matrix --production-upstream"
            )
        });
    let validate_command = production_evidence_phase_command(runbook, "validate_and_import")
        .unwrap_or_else(|| "pool-cli validate-production-evidence <bundle.json>".to_string());

    format!(
        r#"#!/usr/bin/env bash
set -euo pipefail

PROJECT={project_slug}
PROVIDER={provider_id}
ENDPOINT_ENV={endpoint_env}
API_KEY_ENV={api_key_env}
ATTESTATION_ENV={attestation_env}
LOCAL_GATEWAY_CMD={local_gateway_command}
REAL_UPSTREAM_CMD={real_upstream_command}
HEALTH_CMD={health_command}
SMOKE_CMD={smoke_command}
PRODUCTION_CMD={production_command}
VALIDATE_CMD={validate_command}
RUNNER_MODE="${{1:-preflight}}"
POOL_CLI_CMD="${{POOL_CLI_CMD:-}}"

run_cmd() {{
  echo
  echo "+ $*"
  eval "$*"
}}

rewrite_pool_cli_cmd() {{
  local command="$1"
  if [[ "$command" == pool-cli\ * ]]; then
    printf '%s %s' "$POOL_CLI_CMD" "${{command#pool-cli }}"
  else
    printf '%s' "$command"
  fi
}}

first_env_value() {{
  local name
  for name in "$@"; do
    if [[ -n "${{!name:-}}" ]]; then
      printf '%s' "${{!name}}"
      return 0
    fi
  done
  return 1
}}

check_env_value() {{
  local label="$1"
  local name="$2"
  if [[ -z "${{!name:-}}" ]]; then
    echo "MISSING $label env: $name"
    missing=1
  fi
}}

if [[ -z "$POOL_CLI_CMD" ]]; then
  if command -v pool-cli >/dev/null 2>&1; then
    POOL_CLI_CMD="pool-cli"
  else
    POOL_CLI_CMD="cargo run -q -p pool-cli --"
  fi
fi

LOCAL_GATEWAY_CMD="$(rewrite_pool_cli_cmd "$LOCAL_GATEWAY_CMD")"
REAL_UPSTREAM_CMD="$(rewrite_pool_cli_cmd "$REAL_UPSTREAM_CMD")"
HEALTH_CMD="$(rewrite_pool_cli_cmd "$HEALTH_CMD")"
SMOKE_CMD="$(rewrite_pool_cli_cmd "$SMOKE_CMD")"
PRODUCTION_CMD="$(rewrite_pool_cli_cmd "$PRODUCTION_CMD")"
VALIDATE_CMD="$(rewrite_pool_cli_cmd "$VALIDATE_CMD")"

runner_preflight() {{
  local missing=0
  echo "Pool provider conformance preflight"
  echo "project=$PROJECT"
  echo "provider=$PROVIDER"
  echo "pool_cli_cmd=$POOL_CLI_CMD"
  echo "upstream_env={upstream_env_display}"

  if [[ "$POOL_CLI_CMD" == cargo\ * ]] && ! command -v cargo >/dev/null 2>&1; then
    echo "MISSING cargo command on PATH for POOL_CLI_CMD cargo fallback"
    missing=1
  fi
  check_env_value "endpoint" "$ENDPOINT_ENV"
  check_env_value "api key" "$API_KEY_ENV"
  check_env_value "production attestation" "$ATTESTATION_ENV"
  first_env_value {upstream_env_args} >/dev/null || {{
    echo "MISSING upstream endpoint env: {upstream_env_display}"
    missing=1
  }}

  if [[ "$missing" == "0" ]]; then
    echo "preflight_status=ready"
  else
    echo "preflight_status=blocked"
  fi
  return "$missing"
}}

if [[ "$RUNNER_MODE" == "--preflight" || "$RUNNER_MODE" == "preflight" ]]; then
  runner_preflight
  exit $?
fi

if [[ "$RUNNER_MODE" == "local" || "$RUNNER_MODE" == "local-baseline" ]]; then
  run_cmd "$LOCAL_GATEWAY_CMD"
  exit 0
fi

if [[ "$RUNNER_MODE" != "run" ]]; then
  echo "Usage: $0 [preflight|--preflight|local|run]"
  exit 64
fi

runner_preflight
UPSTREAM_VALUE="$(first_env_value {upstream_env_args})"
REAL_UPSTREAM_CMD="${{REAL_UPSTREAM_CMD//<real-worker-or-sdk-url>/$UPSTREAM_VALUE}}"

run_cmd "$LOCAL_GATEWAY_CMD"
run_cmd "$REAL_UPSTREAM_CMD"
run_cmd "$HEALTH_CMD"
run_cmd "$SMOKE_CMD"
run_cmd "$PRODUCTION_CMD"
run_cmd "$VALIDATE_CMD"
"#,
        project_slug = shell_single_quote(project_slug),
        provider_id = shell_single_quote(provider_id),
        endpoint_env = shell_single_quote(endpoint_env),
        api_key_env = shell_single_quote(api_key_env),
        attestation_env = shell_single_quote(attestation_env),
        upstream_env_args = upstream_env_args,
        upstream_env_display = upstream_env_display,
        local_gateway_command = shell_single_quote(&local_gateway_command),
        real_upstream_command = shell_single_quote(&real_upstream_command),
        health_command = shell_single_quote(&health_command),
        smoke_command = shell_single_quote(&smoke_command),
        production_command = shell_single_quote(&production_command),
        validate_command = shell_single_quote(&validate_command),
    )
}

fn provider_conformance_family(contract: &Value) -> String {
    contract
        .get("gateway_family")
        .and_then(Value::as_str)
        .filter(|family| !family.is_empty())
        .unwrap_or("native")
        .to_string()
}

fn provider_conformance_endpoint_env(provider_id: &str, family: &str) -> String {
    match family {
        "ai_media" => "POOL_MEDIA_GATEWAY_ENDPOINT".to_string(),
        "3dgs" => "POOL_3DGS_GATEWAY_ENDPOINT".to_string(),
        _ => format!("POOL_{}_ENDPOINT", provider_env_token(provider_id)),
    }
}

fn provider_conformance_upstream_envs(provider_id: &str, family: &str) -> Vec<String> {
    let token = provider_env_token(provider_id);
    let mut envs = vec![format!("POOL_{token}_UPSTREAM_ENDPOINT")];
    match family {
        "ai_media" => envs.push("POOL_MEDIA_GATEWAY_UPSTREAM_ENDPOINT".to_string()),
        "3dgs" => envs.push("POOL_3DGS_GATEWAY_UPSTREAM_ENDPOINT".to_string()),
        _ => envs.push(format!("POOL_{token}_ENDPOINT")),
    }
    envs.push("POOL_PROVIDER_GATEWAY_UPSTREAM".to_string());
    dedup_strings(envs)
}

fn provider_conformance_attestation_env(provider_id: &str) -> String {
    format!(
        "POOL_PROVIDER_PRODUCTION_ATTESTATION_{}",
        provider_env_token(provider_id)
    )
}

fn provider_env_token(provider_id: &str) -> String {
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

fn software_conformance_preflight(project_slug: &str, adapter_id: &str, runbook: &Value) -> Value {
    let bridge = runbook
        .get("bridge_worker")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let endpoint_env = bridge
        .get("endpoint_env")
        .and_then(Value::as_str)
        .unwrap_or("POOL_SOFTWARE_ENDPOINT");
    let artifacts_env = bridge
        .get("artifacts_env")
        .and_then(Value::as_str)
        .unwrap_or("POOL_SOFTWARE_ARTIFACTS");
    let attestation_env = bridge
        .get("production_attestation_env")
        .and_then(Value::as_str)
        .unwrap_or("POOL_SOFTWARE_PRODUCTION_ATTESTATION");
    let upstream_env = software_conformance_upstream_envs(adapter_id, endpoint_env);

    json!({
        "kind": "pool_software_conformance_preflight",
        "project_slug": project_slug,
        "adapter_id": adapter_id,
        "runner": "4-software-conformance-runner.sh",
        "required_env": {
            "endpoint": endpoint_env,
            "artifacts": artifacts_env,
            "production_attestation": attestation_env,
            "upstream": upstream_env,
        },
        "phases": runbook.get("phases").cloned().unwrap_or_else(|| json!([])),
        "pass_conditions": runbook.get("pass_conditions").cloned().unwrap_or_else(|| json!([])),
        "failure_conditions": [
            "missing upstream endpoint env for real_upstream_bridge",
            "missing production endpoint/artifacts/attestation env for production_matrix",
            "artifact env contains URI-only paths instead of local files",
            "POOL_CLI_CMD falls back to cargo but cargo is unavailable"
        ],
        "preflight_contract": {
            "local_mode": "runner local executes only local_bridge_baseline",
            "run_mode": "runner run executes all conformance phases after env preflight passes",
            "truth_source": format!("pool://software-contracts/{adapter_id}")
        }
    })
}

fn software_conformance_runner_script(
    project_slug: &str,
    adapter_id: &str,
    runbook: &Value,
) -> String {
    let bridge = runbook
        .get("bridge_worker")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let endpoint_env = bridge
        .get("endpoint_env")
        .and_then(Value::as_str)
        .unwrap_or("POOL_SOFTWARE_ENDPOINT");
    let artifacts_env = bridge
        .get("artifacts_env")
        .and_then(Value::as_str)
        .unwrap_or("POOL_SOFTWARE_ARTIFACTS");
    let attestation_env = bridge
        .get("production_attestation_env")
        .and_then(Value::as_str)
        .unwrap_or("POOL_SOFTWARE_PRODUCTION_ATTESTATION");
    let upstream_envs = software_conformance_upstream_envs(adapter_id, endpoint_env);
    let upstream_env_args = shell_env_args(&upstream_envs);
    let upstream_env_display = upstream_envs.join(" or ");
    let local_bridge_command = production_evidence_phase_command(runbook, "local_bridge_baseline")
        .unwrap_or_else(|| {
            format!(
                "pool-cli software-api-bridge-worker {adapter_id} --once --output-root worlds/demo/output"
            )
        });
    let real_upstream_command = production_evidence_phase_command(runbook, "real_upstream_bridge")
        .unwrap_or_else(|| {
            format!("pool-cli software-api-bridge-worker {adapter_id} --bind 127.0.0.1:8793 --output-root worlds/demo/output --upstream <real-plugin-or-gateway-url>")
        });
    let health_command = production_evidence_phase_command(runbook, "software_health")
        .unwrap_or_else(|| {
            format!("pool-cli --project {project_slug} software-health {adapter_id}")
        });
    let action_command = production_evidence_phase_command(runbook, "software_action_smoke")
        .unwrap_or_else(|| {
            format!("pool-cli --project {project_slug} run-software {adapter_id} --no-confirmation")
        });
    let production_command = production_evidence_phase_command(runbook, "production_matrix")
        .unwrap_or_else(|| {
            format!(
                "pool-cli --project {project_slug} production-evidence-software-matrix target/software-evidence-matrix --production-software"
            )
        });
    let validate_command = production_evidence_phase_command(runbook, "validate_and_import")
        .unwrap_or_else(|| "pool-cli validate-production-evidence <bundle.json>".to_string());

    format!(
        r#"#!/usr/bin/env bash
set -euo pipefail

PROJECT={project_slug}
ADAPTER={adapter_id}
ENDPOINT_ENV={endpoint_env}
ARTIFACTS_ENV={artifacts_env}
ATTESTATION_ENV={attestation_env}
LOCAL_BRIDGE_CMD={local_bridge_command}
REAL_UPSTREAM_CMD={real_upstream_command}
HEALTH_CMD={health_command}
ACTION_CMD={action_command}
PRODUCTION_CMD={production_command}
VALIDATE_CMD={validate_command}
RUNNER_MODE="${{1:-preflight}}"
POOL_CLI_CMD="${{POOL_CLI_CMD:-}}"

run_cmd() {{
  echo
  echo "+ $*"
  eval "$*"
}}

rewrite_pool_cli_cmd() {{
  local command="$1"
  if [[ "$command" == pool-cli\ * ]]; then
    printf '%s %s' "$POOL_CLI_CMD" "${{command#pool-cli }}"
  else
    printf '%s' "$command"
  fi
}}

first_env_value() {{
  local name
  for name in "$@"; do
    if [[ -n "${{!name:-}}" ]]; then
      printf '%s' "${{!name}}"
      return 0
    fi
  done
  return 1
}}

check_env_value() {{
  local label="$1"
  local name="$2"
  if [[ -z "${{!name:-}}" ]]; then
    echo "MISSING $label env: $name"
    missing=1
  fi
}}

check_artifacts() {{
  local value="${{!ARTIFACTS_ENV:-}}"
  local artifact
  if [[ -z "$value" ]]; then
    echo "MISSING artifact env: $ARTIFACTS_ENV"
    missing=1
    return
  fi
  IFS=',' read -ra artifact_paths <<< "$value"
  for artifact in "${{artifact_paths[@]}}"; do
    if [[ "$artifact" == *"://"* ]]; then
      echo "INVALID artifact path in $ARTIFACTS_ENV: $artifact"
      echo "Software conformance artifacts must be local file paths."
      missing=1
    fi
  done
}}

if [[ -z "$POOL_CLI_CMD" ]]; then
  if command -v pool-cli >/dev/null 2>&1; then
    POOL_CLI_CMD="pool-cli"
  else
    POOL_CLI_CMD="cargo run -q -p pool-cli --"
  fi
fi

LOCAL_BRIDGE_CMD="$(rewrite_pool_cli_cmd "$LOCAL_BRIDGE_CMD")"
REAL_UPSTREAM_CMD="$(rewrite_pool_cli_cmd "$REAL_UPSTREAM_CMD")"
HEALTH_CMD="$(rewrite_pool_cli_cmd "$HEALTH_CMD")"
ACTION_CMD="$(rewrite_pool_cli_cmd "$ACTION_CMD")"
PRODUCTION_CMD="$(rewrite_pool_cli_cmd "$PRODUCTION_CMD")"
VALIDATE_CMD="$(rewrite_pool_cli_cmd "$VALIDATE_CMD")"

runner_preflight() {{
  local missing=0
  echo "Pool software conformance preflight"
  echo "project=$PROJECT"
  echo "adapter=$ADAPTER"
  echo "pool_cli_cmd=$POOL_CLI_CMD"
  echo "upstream_env={upstream_env_display}"

  if [[ "$POOL_CLI_CMD" == cargo\ * ]] && ! command -v cargo >/dev/null 2>&1; then
    echo "MISSING cargo command on PATH for POOL_CLI_CMD cargo fallback"
    missing=1
  fi
  first_env_value {upstream_env_args} >/dev/null || {{
    echo "MISSING upstream endpoint env: {upstream_env_display}"
    missing=1
  }}
  check_env_value "endpoint" "$ENDPOINT_ENV"
  check_env_value "production attestation" "$ATTESTATION_ENV"
  check_artifacts

  if [[ "$missing" == "0" ]]; then
    echo "preflight_status=ready"
  else
    echo "preflight_status=blocked"
  fi
  return "$missing"
}}

if [[ "$RUNNER_MODE" == "--preflight" || "$RUNNER_MODE" == "preflight" ]]; then
  runner_preflight
  exit $?
fi

if [[ "$RUNNER_MODE" == "local" || "$RUNNER_MODE" == "local-baseline" ]]; then
  run_cmd "$LOCAL_BRIDGE_CMD"
  exit 0
fi

if [[ "$RUNNER_MODE" != "run" ]]; then
  echo "Usage: $0 [preflight|--preflight|local|run]"
  exit 64
fi

runner_preflight
UPSTREAM_VALUE="$(first_env_value {upstream_env_args})"
REAL_UPSTREAM_CMD="${{REAL_UPSTREAM_CMD//<real-plugin-or-gateway-url>/$UPSTREAM_VALUE}}"

run_cmd "$LOCAL_BRIDGE_CMD"
run_cmd "$REAL_UPSTREAM_CMD"
run_cmd "$HEALTH_CMD"
run_cmd "$ACTION_CMD"
run_cmd "$PRODUCTION_CMD"
run_cmd "$VALIDATE_CMD"
"#,
        project_slug = shell_single_quote(project_slug),
        adapter_id = shell_single_quote(adapter_id),
        endpoint_env = shell_single_quote(endpoint_env),
        artifacts_env = shell_single_quote(artifacts_env),
        attestation_env = shell_single_quote(attestation_env),
        upstream_env_args = upstream_env_args,
        upstream_env_display = upstream_env_display,
        local_bridge_command = shell_single_quote(&local_bridge_command),
        real_upstream_command = shell_single_quote(&real_upstream_command),
        health_command = shell_single_quote(&health_command),
        action_command = shell_single_quote(&action_command),
        production_command = shell_single_quote(&production_command),
        validate_command = shell_single_quote(&validate_command),
    )
}

fn software_conformance_upstream_envs(adapter_id: &str, endpoint_env: &str) -> Vec<String> {
    let token = software_adapter_env_token(adapter_id);
    let mut envs = vec![format!("POOL_SOFTWARE_{token}_UPSTREAM_ENDPOINT")];
    if !endpoint_env.is_empty() {
        envs.push(endpoint_env.replace("_ENDPOINT", "_UPSTREAM_ENDPOINT"));
    }
    envs.push(format!("POOL_{token}_UPSTREAM_ENDPOINT"));
    dedup_strings(envs)
}

fn software_adapter_env_token(adapter_id: &str) -> String {
    adapter_id
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

fn shell_env_args(envs: &[String]) -> String {
    envs.iter()
        .map(|env| shell_single_quote(env))
        .collect::<Vec<_>>()
        .join(" ")
}

fn safe_package_segment(value: &str) -> String {
    let segment = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    if segment.is_empty() {
        "package".to_string()
    } else {
        segment
    }
}

fn dedup_strings(values: Vec<String>) -> Vec<String> {
    let mut deduped = Vec::new();
    for value in values {
        if !value.is_empty() && !deduped.contains(&value) {
            deduped.push(value);
        }
    }
    deduped
}

fn with_server_package_metadata(
    mut value: Value,
    package_kind: &str,
    project_slug: &str,
    created_at: &str,
) -> Value {
    let metadata = json!({
        "package_kind": package_kind,
        "project_slug": project_slug,
        "created_at": created_at,
        "local_files_authoritative": true,
        "provider_urls_are_provenance": true,
    });
    match &mut value {
        Value::Object(object) => {
            object.insert("package_metadata".to_string(), metadata);
            value
        }
        _ => json!({
            "package_metadata": metadata,
            "value": value,
        }),
    }
}

fn production_evidence_provider_matrix_command(
    project_slug: &str,
    output_root: &str,
    provider_bundle: &str,
) -> String {
    let provider_env_flags = production_evidence_provider_matrix_env_flags();
    format!(
        "pool-cli --project {project_slug} production-evidence-provider-matrix {output_root}/provider-evidence-matrix --production-upstream --media-endpoint=<shared-media-gateway> --3dgs-endpoint=<shared-3dgs-gateway> {provider_env_flags} --openai-api-key-env OPENAI_API_KEY --evidence-bundle={provider_bundle}"
    )
}

fn production_evidence_provider_matrix_env_flags() -> String {
    REQUIRED_PRODUCTION_PROVIDER_EVIDENCE
        .iter()
        .flat_map(|provider_id| {
            let env_key = production_evidence_provider_env_key(provider_id);
            [
                format!("--provider-endpoint-env {provider_id}=POOL_PROVIDER_ENDPOINT_{env_key}"),
                format!("--provider-api-key-env {provider_id}=POOL_PROVIDER_API_KEY_{env_key}"),
                format!("--provider-attestation-env {provider_id}=POOL_PROVIDER_PRODUCTION_ATTESTATION_{env_key}"),
            ]
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn production_evidence_provider_env_key(provider_id: &str) -> String {
    provider_id.replace('-', "_").to_ascii_uppercase()
}

fn production_evidence_software_matrix_command(
    project_slug: &str,
    output_root: &str,
    software_bundle: &str,
) -> String {
    format!(
        "pool-cli --project {project_slug} production-evidence-software-matrix {output_root}/software-evidence-matrix --production-software --software-endpoint-env unreal=POOL_UNREAL_MCP_ENDPOINT --software-endpoint-env blender=POOL_BLENDER_ENDPOINT --software-endpoint-env comfyui=POOL_COMFYUI_ENDPOINT --software-endpoint-env resolve=POOL_RESOLVE_ENDPOINT --software-endpoint-env unity=POOL_UNITY_ENDPOINT --software-endpoint-env touchdesigner=POOL_TOUCHDESIGNER_ENDPOINT --software-endpoint-env madmapper=POOL_MADMAPPER_ENDPOINT --software-endpoint-env nuke=POOL_NUKE_ENDPOINT --software-endpoint-env motion-db=POOL_MOTION_DB_ENDPOINT --software-endpoint-env editing-suite=POOL_EDITING_SUITE_ENDPOINT --software-endpoint-env hermes=POOL_HERMES_MCP_ENDPOINT --software-command-env blender=POOL_BLENDER_COMMAND --software-command-env comfyui=POOL_COMFYUI_COMMAND --software-command-env resolve=POOL_RESOLVE_COMMAND --software-command-env unity=POOL_UNITY_COMMAND --software-command-env touchdesigner=POOL_TOUCHDESIGNER_COMMAND --software-command-env madmapper=POOL_MADMAPPER_COMMAND --software-command-env nuke=POOL_NUKE_COMMAND --software-command-env motion-db=POOL_MOTION_DB_COMMAND --software-command-env editing-suite=POOL_EDITING_SUITE_COMMAND --software-command-env hermes=POOL_HERMES_COMMAND --software-artifacts-env unreal=POOL_UNREAL_ARTIFACTS --software-artifacts-env blender=POOL_BLENDER_ARTIFACTS --software-artifacts-env comfyui=POOL_COMFYUI_ARTIFACTS --software-artifacts-env resolve=POOL_RESOLVE_ARTIFACTS --software-artifacts-env unity=POOL_UNITY_ARTIFACTS --software-artifacts-env touchdesigner=POOL_TOUCHDESIGNER_ARTIFACTS --software-artifacts-env madmapper=POOL_MADMAPPER_ARTIFACTS --software-artifacts-env nuke=POOL_NUKE_ARTIFACTS --software-artifacts-env motion-db=POOL_MOTION_DB_ARTIFACTS --software-artifacts-env editing-suite=POOL_EDITING_SUITE_ARTIFACTS --software-artifacts-env hermes=POOL_HERMES_ARTIFACTS --software-attestation-env unreal=POOL_UNREAL_PRODUCTION_ATTESTATION --software-attestation-env blender=POOL_BLENDER_PRODUCTION_ATTESTATION --software-attestation-env comfyui=POOL_COMFYUI_PRODUCTION_ATTESTATION --software-attestation-env resolve=POOL_RESOLVE_PRODUCTION_ATTESTATION --software-attestation-env unity=POOL_UNITY_PRODUCTION_ATTESTATION --software-attestation-env touchdesigner=POOL_TOUCHDESIGNER_PRODUCTION_ATTESTATION --software-attestation-env madmapper=POOL_MADMAPPER_PRODUCTION_ATTESTATION --software-attestation-env nuke=POOL_NUKE_PRODUCTION_ATTESTATION --software-attestation-env motion-db=POOL_MOTION_DB_PRODUCTION_ATTESTATION --software-attestation-env editing-suite=POOL_EDITING_SUITE_PRODUCTION_ATTESTATION --software-attestation-env hermes=POOL_HERMES_PRODUCTION_ATTESTATION --evidence-bundle={software_bundle}"
    )
}

fn production_evidence_desktop_vision_command(
    project_slug: &str,
    output_root: &str,
    desktop_bundle: &str,
) -> String {
    format!(
        "pool-cli --project {project_slug} production-evidence-desktop-vision {output_root}/desktop-vision --production-vision --trace-env POOL_DESKTOP_VISION_TRACE --controller-id-env POOL_DESKTOP_VISION_CONTROLLER_ID --external-action-id-env POOL_DESKTOP_VISION_EXTERNAL_ACTION_ID --production-attestation-env POOL_DESKTOP_VISION_PRODUCTION_ATTESTATION --evidence-bundle={desktop_bundle}"
    )
}

fn production_evidence_runner_script(
    project_slug: &str,
    output_root: &str,
    run_plan: &Value,
) -> String {
    let provider_command = production_evidence_phase_command(run_plan, "provider_evidence_matrix")
        .unwrap_or_else(|| {
            production_evidence_provider_matrix_command(
                project_slug,
                output_root,
                &format!("{output_root}/provider-production-evidence-bundle.json"),
            )
        });
    let software_command = production_evidence_phase_command(run_plan, "software_evidence_matrix")
        .unwrap_or_else(|| {
            production_evidence_software_matrix_command(
                project_slug,
                output_root,
                &format!("{output_root}/software-production-evidence-bundle.json"),
            )
        });
    let desktop_command = production_evidence_phase_command(run_plan, "desktop_vision_evidence")
        .unwrap_or_else(|| {
            production_evidence_desktop_vision_command(
                project_slug,
                output_root,
                &format!("{output_root}/desktop-vision-production-evidence-bundle.json"),
            )
        });
    let merge_command = production_evidence_phase_command(run_plan, "merge_bundles")
        .or_else(|| production_evidence_phase_command(run_plan, "merge"))
        .unwrap_or_else(|| format!("pool-cli --project {project_slug} merge-production-evidence {output_root}/combined-production-evidence-bundle.json {output_root}/provider-production-evidence-bundle.json {output_root}/software-production-evidence-bundle.json {output_root}/desktop-vision-production-evidence-bundle.json"));
    let closeout_preflight_command = production_evidence_phase_command(run_plan, "closeout_preflight")
        .unwrap_or_else(|| format!("pool-cli --project {project_slug} closeout-production-evidence --output {output_root}/combined-production-evidence-bundle.json {output_root}/combined-production-evidence-bundle.json"));
    let closeout_import_command = production_evidence_phase_command(run_plan, "closeout_import")
        .unwrap_or_else(|| format!("pool-cli --project {project_slug} closeout-production-evidence --import {output_root}/combined-production-evidence-bundle.json"));
    let completion_command = run_plan
        .pointer("/commands/completion_gate")
        .and_then(Value::as_str)
        .unwrap_or("pool-cli prd-completion-gate --require-complete");
    let desktop_bundle_path = run_plan
        .pointer("/paths/desktop_vision_bundle")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .unwrap_or_else(|| format!("{output_root}/desktop-vision-production-evidence-bundle.json"));
    let provider_gateway_worker_commands = production_evidence_phase_value(
        run_plan,
        "provider_evidence_matrix",
        "provider_gateway_worker_start_commands",
    )
    .unwrap_or_else(|| default_production_evidence_provider_gateway_worker_commands(output_root));
    let provider_gateway_worker_hint =
        production_evidence_provider_gateway_shell_hint(&provider_gateway_worker_commands);
    let software_bridge_worker = production_evidence_phase_value(
        run_plan,
        "software_evidence_matrix",
        "generic_api_bridge_worker",
    )
    .unwrap_or_else(default_production_evidence_software_bridge_worker);
    let software_bridge_worker_hint =
        production_evidence_bridge_worker_shell_hint(output_root, &software_bridge_worker);

    format!(
        r#"#!/usr/bin/env bash
set -euo pipefail

echo "Pool production evidence runner"
echo "project={project_slug}"
echo "output_root={output_root}"

run_cmd() {{
  echo
  echo "+ $*"
  eval "$*"
}}

rewrite_pool_cli_cmd() {{
  local command="$1"
  if [[ "$command" == pool-cli\ * ]]; then
    printf '%s %s' "$POOL_CLI_CMD" "${{command#pool-cli }}"
  else
    printf '%s' "$command"
  fi
}}

has_any_env() {{
  local name
  for name in "$@"; do
    if [[ -n "${{!name:-}}" ]]; then
      return 0
    fi
  done
  return 1
}}

check_software_env() {{
  local label="$1"
  shift
  if ! has_any_env "$@"; then
    echo "MISSING production software config for $label: $*"
    missing=1
  fi
}}

check_provider_attestation() {{
  local label="$1"
  shift
  if ! has_any_env "$@" POOL_PROVIDER_PRODUCTION_ATTESTATION; then
    echo "MISSING provider production attestation for $label: $* or POOL_PROVIDER_PRODUCTION_ATTESTATION"
    missing=1
  fi
}}

check_provider_endpoint() {{
  local label="$1"
  local shared_env="$2"
  shift 2
  if ! has_any_env "$shared_env" "$@"; then
    echo "MISSING provider endpoint for $label: $shared_env or $*"
    missing=1
  fi
}}

check_provider_api_key() {{
  local label="$1"
  shift
  if ! has_any_env "$@"; then
    echo "MISSING provider API key for $label: $*"
    missing=1
  fi
}}

check_software_artifacts() {{
  local label="$1"
  shift
  local found=0
  local name value artifact
  for name in "$@"; do
    value="${{!name:-}}"
    if [[ -n "$value" ]]; then
      found=1
      IFS=',' read -ra artifact_paths <<< "$value"
      for artifact in "${{artifact_paths[@]}}"; do
        if [[ "$artifact" == *"://"* ]]; then
          echo "INVALID production software artifact path for $label in $name: $artifact"
          echo "Software production artifacts must be local file paths, not URLs or custom URIs."
          missing=1
        fi
      done
    fi
  done
  if [[ "$found" == "0" ]]; then
    echo "MISSING production software artifact config for $label: $*"
    missing=1
  fi
}}

PROVIDER_CMD={provider_command}
SOFTWARE_CMD={software_command}
DESKTOP_CMD={desktop_command}
DESKTOP_BUNDLE_PATH={desktop_bundle_path}
MERGE_CMD={merge_command}
CLOSEOUT_PREFLIGHT_CMD={closeout_preflight_command}
CLOSEOUT_IMPORT_CMD={closeout_import_command}
COMPLETION_GATE_CMD={completion_command}
RUNNER_MODE="${{1:-run}}"
POOL_CLI_CMD="${{POOL_CLI_CMD:-}}"

if [[ -z "$POOL_CLI_CMD" ]]; then
  if command -v pool-cli >/dev/null 2>&1; then
    POOL_CLI_CMD="pool-cli"
  else
    POOL_CLI_CMD="cargo run -q -p pool-cli --"
  fi
fi

MERGE_CMD="$(rewrite_pool_cli_cmd "$MERGE_CMD")"
PROVIDER_CMD="$(rewrite_pool_cli_cmd "$PROVIDER_CMD")"
SOFTWARE_CMD="$(rewrite_pool_cli_cmd "$SOFTWARE_CMD")"
DESKTOP_CMD="$(rewrite_pool_cli_cmd "$DESKTOP_CMD")"
CLOSEOUT_PREFLIGHT_CMD="$(rewrite_pool_cli_cmd "$CLOSEOUT_PREFLIGHT_CMD")"
CLOSEOUT_IMPORT_CMD="$(rewrite_pool_cli_cmd "$CLOSEOUT_IMPORT_CMD")"
COMPLETION_GATE_CMD="$(rewrite_pool_cli_cmd "$COMPLETION_GATE_CMD")"

runner_preflight() {{
  local missing=0
  echo "Pool production evidence runner preflight"
  echo "project={project_slug}"
  echo "output_root={output_root}"
  echo "pool_cli_cmd=$POOL_CLI_CMD"
{provider_gateway_worker_hint}
{software_bridge_worker_hint}

  if [[ "$POOL_CLI_CMD" == cargo\ * ]] && ! command -v cargo >/dev/null 2>&1; then
    echo "MISSING cargo command on PATH for POOL_CLI_CMD cargo fallback"
    missing=1
  fi
  if [[ "$POOL_CLI_CMD" != "pool-cli" ]]; then
    echo "INFO pool-cli command fallback active: $POOL_CLI_CMD"
  fi

  check_provider_endpoint "midjourney" POOL_MEDIA_GATEWAY_ENDPOINT POOL_PROVIDER_ENDPOINT_MIDJOURNEY POOL_MIDJOURNEY_ENDPOINT
  check_provider_endpoint "nano-banana-pro" POOL_MEDIA_GATEWAY_ENDPOINT POOL_PROVIDER_ENDPOINT_NANO_BANANA_PRO POOL_NANO_BANANA_PRO_ENDPOINT
  check_provider_endpoint "suno" POOL_MEDIA_GATEWAY_ENDPOINT POOL_PROVIDER_ENDPOINT_SUNO POOL_SUNO_ENDPOINT
  check_provider_endpoint "worldlabs-marble" POOL_3DGS_GATEWAY_ENDPOINT POOL_PROVIDER_ENDPOINT_WORLDLABS_MARBLE POOL_WORLDLABS_MARBLE_ENDPOINT
  check_provider_endpoint "tripo-splat" POOL_3DGS_GATEWAY_ENDPOINT POOL_PROVIDER_ENDPOINT_TRIPO_SPLAT POOL_TRIPO_SPLAT_ENDPOINT
  check_provider_endpoint "sam-3d" POOL_3DGS_GATEWAY_ENDPOINT POOL_PROVIDER_ENDPOINT_SAM_3D POOL_SAM_3D_ENDPOINT
  check_provider_endpoint "spark-3dgs" POOL_3DGS_GATEWAY_ENDPOINT POOL_PROVIDER_ENDPOINT_SPARK_3DGS POOL_SPARK_3DGS_ENDPOINT
  check_provider_endpoint "qunhe-3d" POOL_3DGS_GATEWAY_ENDPOINT POOL_PROVIDER_ENDPOINT_QUNHE_3D POOL_QUNHE_3D_ENDPOINT
  check_provider_api_key "openai-image-2" OPENAI_API_KEY POOL_PROVIDER_API_KEY_OPENAI_IMAGE_2 POOL_OPENAI_IMAGE_2_API_KEY
  if [[ "$PROVIDER_CMD" == *"--production-upstream"* ]]; then
    check_provider_attestation "midjourney" POOL_PROVIDER_PRODUCTION_ATTESTATION_MIDJOURNEY POOL_MIDJOURNEY_PRODUCTION_ATTESTATION
    check_provider_attestation "openai-image-2" POOL_PROVIDER_PRODUCTION_ATTESTATION_OPENAI_IMAGE_2 POOL_OPENAI_IMAGE_2_PRODUCTION_ATTESTATION
    check_provider_attestation "nano-banana-pro" POOL_PROVIDER_PRODUCTION_ATTESTATION_NANO_BANANA_PRO POOL_NANO_BANANA_PRO_PRODUCTION_ATTESTATION
    check_provider_attestation "suno" POOL_PROVIDER_PRODUCTION_ATTESTATION_SUNO POOL_SUNO_PRODUCTION_ATTESTATION
    check_provider_attestation "worldlabs-marble" POOL_PROVIDER_PRODUCTION_ATTESTATION_WORLDLABS_MARBLE POOL_WORLDLABS_MARBLE_PRODUCTION_ATTESTATION
    check_provider_attestation "tripo-splat" POOL_PROVIDER_PRODUCTION_ATTESTATION_TRIPO_SPLAT POOL_TRIPO_SPLAT_PRODUCTION_ATTESTATION
    check_provider_attestation "sam-3d" POOL_PROVIDER_PRODUCTION_ATTESTATION_SAM_3D POOL_SAM_3D_PRODUCTION_ATTESTATION
    check_provider_attestation "spark-3dgs" POOL_PROVIDER_PRODUCTION_ATTESTATION_SPARK_3DGS POOL_SPARK_3DGS_PRODUCTION_ATTESTATION
    check_provider_attestation "qunhe-3d" POOL_PROVIDER_PRODUCTION_ATTESTATION_QUNHE_3D POOL_QUNHE_3D_PRODUCTION_ATTESTATION
  fi
  check_software_env "unreal attestation" POOL_SOFTWARE_UNREAL_PRODUCTION_ATTESTATION POOL_UNREAL_PRODUCTION_ATTESTATION POOL_SOFTWARE_PRODUCTION_ATTESTATION
  check_software_env "blender attestation" POOL_SOFTWARE_BLENDER_PRODUCTION_ATTESTATION POOL_BLENDER_PRODUCTION_ATTESTATION POOL_SOFTWARE_PRODUCTION_ATTESTATION
  check_software_env "comfyui attestation" POOL_SOFTWARE_COMFYUI_PRODUCTION_ATTESTATION POOL_COMFYUI_PRODUCTION_ATTESTATION POOL_SOFTWARE_PRODUCTION_ATTESTATION
  check_software_env "resolve attestation" POOL_SOFTWARE_RESOLVE_PRODUCTION_ATTESTATION POOL_RESOLVE_PRODUCTION_ATTESTATION POOL_SOFTWARE_DAVINCI_RESOLVE_PRODUCTION_ATTESTATION POOL_DAVINCI_RESOLVE_PRODUCTION_ATTESTATION POOL_SOFTWARE_PRODUCTION_ATTESTATION
  check_software_env "unity attestation" POOL_SOFTWARE_UNITY_PRODUCTION_ATTESTATION POOL_UNITY_PRODUCTION_ATTESTATION POOL_SOFTWARE_PRODUCTION_ATTESTATION
  check_software_env "touchdesigner attestation" POOL_SOFTWARE_TOUCHDESIGNER_PRODUCTION_ATTESTATION POOL_TOUCHDESIGNER_PRODUCTION_ATTESTATION POOL_SOFTWARE_TOUCH_DESIGNER_PRODUCTION_ATTESTATION POOL_TOUCH_DESIGNER_PRODUCTION_ATTESTATION POOL_SOFTWARE_PRODUCTION_ATTESTATION
  check_software_env "madmapper attestation" POOL_SOFTWARE_MADMAPPER_PRODUCTION_ATTESTATION POOL_MADMAPPER_PRODUCTION_ATTESTATION POOL_SOFTWARE_PRODUCTION_ATTESTATION
  check_software_env "nuke attestation" POOL_SOFTWARE_NUKE_PRODUCTION_ATTESTATION POOL_NUKE_PRODUCTION_ATTESTATION POOL_SOFTWARE_PRODUCTION_ATTESTATION
  check_software_env "motion-db attestation" POOL_SOFTWARE_MOTION_DB_PRODUCTION_ATTESTATION POOL_MOTION_DB_PRODUCTION_ATTESTATION POOL_SOFTWARE_MOCAP_DB_PRODUCTION_ATTESTATION POOL_MOCAP_DB_PRODUCTION_ATTESTATION POOL_SOFTWARE_PRODUCTION_ATTESTATION
  check_software_env "editing-suite attestation" POOL_SOFTWARE_EDITING_SUITE_PRODUCTION_ATTESTATION POOL_EDITING_SUITE_PRODUCTION_ATTESTATION POOL_SOFTWARE_EDITOR_PRODUCTION_ATTESTATION POOL_EDITOR_PRODUCTION_ATTESTATION POOL_SOFTWARE_PRODUCTION_ATTESTATION
  check_software_env "hermes attestation" POOL_SOFTWARE_HERMES_PRODUCTION_ATTESTATION POOL_HERMES_PRODUCTION_ATTESTATION POOL_SOFTWARE_PRODUCTION_ATTESTATION
  if [[ "${{POOL_RUN_DESKTOP_VISION:-0}}" == "1" && -z "${{POOL_DESKTOP_VISION_TRACE:-}}" ]]; then
    echo "MISSING POOL_DESKTOP_VISION_TRACE while POOL_RUN_DESKTOP_VISION=1"
    missing=1
  fi
  if [[ "${{POOL_RUN_DESKTOP_VISION:-0}}" == "1" && -z "${{POOL_DESKTOP_VISION_EXTERNAL_ACTION_ID:-}}" ]]; then
    echo "MISSING POOL_DESKTOP_VISION_EXTERNAL_ACTION_ID while POOL_RUN_DESKTOP_VISION=1"
    missing=1
  fi
  if [[ "${{POOL_RUN_DESKTOP_VISION:-0}}" == "1" && -z "${{POOL_DESKTOP_VISION_PRODUCTION_ATTESTATION:-}}" ]]; then
    echo "MISSING POOL_DESKTOP_VISION_PRODUCTION_ATTESTATION while POOL_RUN_DESKTOP_VISION=1"
    missing=1
  fi
  check_software_env "unreal" POOL_UNREAL_MCP_ENDPOINT POOL_SOFTWARE_UNREAL_ENDPOINT POOL_UNREAL_ENDPOINT
  check_software_env "blender" POOL_SOFTWARE_BLENDER_ENDPOINT POOL_BLENDER_ENDPOINT POOL_SOFTWARE_BLENDER_COMMAND POOL_BLENDER_COMMAND
  check_software_env "comfyui" POOL_SOFTWARE_COMFYUI_ENDPOINT POOL_COMFYUI_ENDPOINT POOL_SOFTWARE_COMFYUI_COMMAND POOL_COMFYUI_COMMAND
  check_software_env "resolve" POOL_SOFTWARE_RESOLVE_ENDPOINT POOL_RESOLVE_ENDPOINT POOL_SOFTWARE_DAVINCI_RESOLVE_ENDPOINT POOL_DAVINCI_RESOLVE_ENDPOINT POOL_SOFTWARE_RESOLVE_COMMAND POOL_RESOLVE_COMMAND POOL_SOFTWARE_DAVINCI_RESOLVE_COMMAND POOL_DAVINCI_RESOLVE_COMMAND
  check_software_env "unity" POOL_SOFTWARE_UNITY_ENDPOINT POOL_UNITY_ENDPOINT POOL_SOFTWARE_UNITY_COMMAND POOL_UNITY_COMMAND
  check_software_env "touchdesigner" POOL_SOFTWARE_TOUCHDESIGNER_ENDPOINT POOL_TOUCHDESIGNER_ENDPOINT POOL_SOFTWARE_TOUCH_DESIGNER_ENDPOINT POOL_TOUCH_DESIGNER_ENDPOINT POOL_SOFTWARE_TOUCHDESIGNER_COMMAND POOL_TOUCHDESIGNER_COMMAND POOL_SOFTWARE_TOUCH_DESIGNER_COMMAND POOL_TOUCH_DESIGNER_COMMAND
  check_software_env "madmapper" POOL_SOFTWARE_MADMAPPER_ENDPOINT POOL_MADMAPPER_ENDPOINT POOL_SOFTWARE_MADMAPPER_COMMAND POOL_MADMAPPER_COMMAND
  check_software_env "nuke" POOL_SOFTWARE_NUKE_ENDPOINT POOL_NUKE_ENDPOINT POOL_SOFTWARE_NUKE_COMMAND POOL_NUKE_COMMAND
  check_software_env "motion-db" POOL_SOFTWARE_MOTION_DB_ENDPOINT POOL_MOTION_DB_ENDPOINT POOL_SOFTWARE_MOCAP_DB_ENDPOINT POOL_MOCAP_DB_ENDPOINT POOL_SOFTWARE_MOTION_DB_COMMAND POOL_MOTION_DB_COMMAND POOL_SOFTWARE_MOCAP_DB_COMMAND POOL_MOCAP_DB_COMMAND
  check_software_env "editing-suite" POOL_SOFTWARE_EDITING_SUITE_ENDPOINT POOL_EDITING_SUITE_ENDPOINT POOL_SOFTWARE_EDITOR_ENDPOINT POOL_EDITOR_ENDPOINT POOL_SOFTWARE_EDITING_SUITE_COMMAND POOL_EDITING_SUITE_COMMAND POOL_SOFTWARE_EDITOR_COMMAND POOL_EDITOR_COMMAND
  check_software_env "hermes" POOL_HERMES_MCP_ENDPOINT POOL_HERMES_ENDPOINT POOL_SOFTWARE_HERMES_ENDPOINT POOL_HERMES_COMMAND POOL_SOFTWARE_HERMES_COMMAND
  check_software_artifacts "unreal" POOL_SOFTWARE_UNREAL_ARTIFACTS POOL_UNREAL_ARTIFACTS
  check_software_artifacts "blender" POOL_SOFTWARE_BLENDER_ARTIFACTS POOL_BLENDER_ARTIFACTS
  check_software_artifacts "comfyui" POOL_SOFTWARE_COMFYUI_ARTIFACTS POOL_COMFYUI_ARTIFACTS
  check_software_artifacts "resolve" POOL_SOFTWARE_RESOLVE_ARTIFACTS POOL_RESOLVE_ARTIFACTS POOL_SOFTWARE_DAVINCI_RESOLVE_ARTIFACTS POOL_DAVINCI_RESOLVE_ARTIFACTS
  check_software_artifacts "unity" POOL_SOFTWARE_UNITY_ARTIFACTS POOL_UNITY_ARTIFACTS
  check_software_artifacts "touchdesigner" POOL_SOFTWARE_TOUCHDESIGNER_ARTIFACTS POOL_TOUCHDESIGNER_ARTIFACTS POOL_SOFTWARE_TOUCH_DESIGNER_ARTIFACTS POOL_TOUCH_DESIGNER_ARTIFACTS
  check_software_artifacts "madmapper" POOL_SOFTWARE_MADMAPPER_ARTIFACTS POOL_MADMAPPER_ARTIFACTS
  check_software_artifacts "nuke" POOL_SOFTWARE_NUKE_ARTIFACTS POOL_NUKE_ARTIFACTS
  check_software_artifacts "motion-db" POOL_SOFTWARE_MOTION_DB_ARTIFACTS POOL_MOTION_DB_ARTIFACTS POOL_SOFTWARE_MOCAP_DB_ARTIFACTS POOL_MOCAP_DB_ARTIFACTS
  check_software_artifacts "editing-suite" POOL_SOFTWARE_EDITING_SUITE_ARTIFACTS POOL_EDITING_SUITE_ARTIFACTS POOL_SOFTWARE_EDITOR_ARTIFACTS POOL_EDITOR_ARTIFACTS
  check_software_artifacts "hermes" POOL_SOFTWARE_HERMES_ARTIFACTS POOL_HERMES_ARTIFACTS

  if [[ "$missing" == "0" ]]; then
    echo "preflight_status=ready"
  else
    echo "preflight_status=blocked"
  fi
  return "$missing"
}}

if [[ "$RUNNER_MODE" == "--preflight" || "$RUNNER_MODE" == "preflight" ]]; then
  runner_preflight
  exit $?
fi

if [[ "$RUNNER_MODE" != "run" ]]; then
  echo "Usage: $0 [run|--preflight|preflight]"
  exit 64
fi

runner_preflight

if [[ "$PROVIDER_CMD" == *"<real-media-gateway>"* || "$PROVIDER_CMD" == *"<shared-media-gateway>"* || "$PROVIDER_CMD" == *"<real-3dgs-gateway>"* || "$PROVIDER_CMD" == *"<shared-3dgs-gateway>"* ]]; then
  MEDIA_GATEWAY_VALUE="${{POOL_MEDIA_GATEWAY_ENDPOINT:-http://127.0.0.1:0/pool-unused-media-gateway}}"
  THREE_DGS_GATEWAY_VALUE="${{POOL_3DGS_GATEWAY_ENDPOINT:-http://127.0.0.1:0/pool-unused-3dgs-gateway}}"
  PROVIDER_CMD="${{PROVIDER_CMD//<real-media-gateway>/${{MEDIA_GATEWAY_VALUE}}}}"
  PROVIDER_CMD="${{PROVIDER_CMD//<shared-media-gateway>/${{MEDIA_GATEWAY_VALUE}}}}"
  PROVIDER_CMD="${{PROVIDER_CMD//<real-3dgs-gateway>/${{THREE_DGS_GATEWAY_VALUE}}}}"
  PROVIDER_CMD="${{PROVIDER_CMD//<shared-3dgs-gateway>/${{THREE_DGS_GATEWAY_VALUE}}}}"
fi

run_cmd "$PROVIDER_CMD"
run_cmd "$SOFTWARE_CMD"

if [[ "${{POOL_RUN_DESKTOP_VISION:-0}}" == "1" ]]; then
  if [[ -z "${{POOL_DESKTOP_VISION_TRACE:-}}" || -z "${{POOL_DESKTOP_VISION_EXTERNAL_ACTION_ID:-}}" || -z "${{POOL_DESKTOP_VISION_PRODUCTION_ATTESTATION:-}}" ]]; then
    echo "POOL_RUN_DESKTOP_VISION=1 requires POOL_DESKTOP_VISION_TRACE, POOL_DESKTOP_VISION_EXTERNAL_ACTION_ID, and POOL_DESKTOP_VISION_PRODUCTION_ATTESTATION"
    exit 2
  fi
  DESKTOP_CMD="${{DESKTOP_CMD//<real-vision-trace>/${{POOL_DESKTOP_VISION_TRACE}}}}"
  DESKTOP_CMD="${{DESKTOP_CMD//<real-vision-action-id>/${{POOL_DESKTOP_VISION_EXTERNAL_ACTION_ID}}}}"
  run_cmd "$DESKTOP_CMD"
else
  echo
  echo "Skipping desktop vision evidence. Set POOL_RUN_DESKTOP_VISION=1, POOL_DESKTOP_VISION_TRACE, POOL_DESKTOP_VISION_EXTERNAL_ACTION_ID, and POOL_DESKTOP_VISION_PRODUCTION_ATTESTATION to enable it."
  if [[ ! -f "$DESKTOP_BUNDLE_PATH" ]]; then
    mkdir -p "$(dirname "$DESKTOP_BUNDLE_PATH")"
    cat > "$DESKTOP_BUNDLE_PATH" <<JSON
{{"project_slug":"{project_slug}","source":"production-evidence-runner-desktop-skipped","providers":[],"software_actions":[],"desktop_vision":[]}}
JSON
  fi
fi

run_cmd "$MERGE_CMD"
run_cmd "$CLOSEOUT_PREFLIGHT_CMD"

if [[ "${{POOL_IMPORT_PRODUCTION_EVIDENCE:-0}}" == "1" ]]; then
  run_cmd "$CLOSEOUT_IMPORT_CMD"
  run_cmd "$COMPLETION_GATE_CMD"
else
  echo
  echo "Preflight complete. Set POOL_IMPORT_PRODUCTION_EVIDENCE=1 to import after reviewing the merged bundle."
fi
"#,
        project_slug = project_slug,
        output_root = output_root,
        provider_command = shell_single_quote(&provider_command),
        software_command = shell_single_quote(&software_command),
        desktop_command = shell_single_quote(&desktop_command),
        desktop_bundle_path = shell_single_quote(&desktop_bundle_path),
        merge_command = shell_single_quote(&merge_command),
        closeout_preflight_command = shell_single_quote(&closeout_preflight_command),
        closeout_import_command = shell_single_quote(&closeout_import_command),
        completion_command = shell_single_quote(completion_command),
        provider_gateway_worker_hint = provider_gateway_worker_hint,
        software_bridge_worker_hint = software_bridge_worker_hint,
    )
}

fn production_evidence_runner_preflight(
    project_slug: &str,
    output_root: &str,
    run_plan: &Value,
) -> Value {
    let provider_command = production_evidence_phase_command(run_plan, "provider_evidence_matrix")
        .unwrap_or_else(|| {
            production_evidence_provider_matrix_command(
                project_slug,
                output_root,
                &format!("{output_root}/provider-production-evidence-bundle.json"),
            )
        });
    let software_command = production_evidence_phase_command(run_plan, "software_evidence_matrix")
        .unwrap_or_else(|| {
            production_evidence_software_matrix_command(
                project_slug,
                output_root,
                &format!("{output_root}/software-production-evidence-bundle.json"),
            )
        });
    let desktop_command = production_evidence_phase_command(run_plan, "desktop_vision_evidence")
        .unwrap_or_else(|| {
            production_evidence_desktop_vision_command(
                project_slug,
                output_root,
                &format!("{output_root}/desktop-vision-production-evidence-bundle.json"),
            )
        });
    let merge_command = production_evidence_phase_command(run_plan, "merge_bundles")
        .or_else(|| production_evidence_phase_command(run_plan, "merge"))
        .unwrap_or_else(|| format!("pool-cli --project {project_slug} merge-production-evidence {output_root}/combined-production-evidence-bundle.json {output_root}/provider-production-evidence-bundle.json {output_root}/software-production-evidence-bundle.json {output_root}/desktop-vision-production-evidence-bundle.json"));
    let closeout_preflight_command = production_evidence_phase_command(run_plan, "closeout_preflight")
        .unwrap_or_else(|| format!("pool-cli --project {project_slug} closeout-production-evidence --output {output_root}/merged-production-evidence-bundle.json {output_root}/combined-production-evidence-bundle.json"));
    let closeout_import_command = production_evidence_phase_command(run_plan, "closeout_import")
        .unwrap_or_else(|| format!("pool-cli --project {project_slug} closeout-production-evidence --import {output_root}/merged-production-evidence-bundle.json"));
    let completion_command = run_plan
        .pointer("/commands/completion_gate")
        .and_then(Value::as_str)
        .unwrap_or("pool-cli prd-completion-gate --require-complete");
    let provider_bundle = production_evidence_run_plan_path(
        run_plan,
        "provider_bundle",
        &format!("{output_root}/provider-production-evidence-bundle.json"),
    );
    let software_bundle = production_evidence_run_plan_path(
        run_plan,
        "software_bundle",
        &format!("{output_root}/software-production-evidence-bundle.json"),
    );
    let desktop_bundle = production_evidence_run_plan_path(
        run_plan,
        "desktop_vision_bundle",
        &format!("{output_root}/desktop-vision-production-evidence-bundle.json"),
    );
    let combined_bundle = production_evidence_run_plan_path(
        run_plan,
        "combined_bundle",
        &format!("{output_root}/combined-production-evidence-bundle.json"),
    );
    let merged_bundle = production_evidence_run_plan_path(
        run_plan,
        "merged_bundle",
        &format!("{output_root}/merged-production-evidence-bundle.json"),
    );
    let provider_gateway_worker_commands = production_evidence_phase_value(
        run_plan,
        "provider_evidence_matrix",
        "provider_gateway_worker_start_commands",
    )
    .unwrap_or_else(|| default_production_evidence_provider_gateway_worker_commands(output_root));
    let software_bridge_worker = production_evidence_phase_value(
        run_plan,
        "software_evidence_matrix",
        "generic_api_bridge_worker",
    )
    .unwrap_or_else(default_production_evidence_software_bridge_worker);
    let software_bridge_worker_commands =
        production_evidence_bridge_worker_commands(output_root, &software_bridge_worker);

    json!({
        "kind": "pool_production_evidence_runner_preflight",
        "version": 1,
        "project_slug": project_slug,
        "output_root": output_root,
        "local_files_authoritative": true,
        "provider_urls_are_provenance": true,
        "runner_modes": {
            "preflight": "7-production-evidence-runner.sh --preflight",
            "run": "7-production-evidence-runner.sh",
            "run_with_import": "POOL_IMPORT_PRODUCTION_EVIDENCE=1 7-production-evidence-runner.sh",
        },
        "environment": {
            "required_for_provider": [
                "POOL_MEDIA_GATEWAY_ENDPOINT or per-media Provider endpoint envs for Midjourney, Nano Banana Pro, and Suno",
                "POOL_3DGS_GATEWAY_ENDPOINT or per-3DGS Provider endpoint envs for World Labs Marble, TripoSplat, SAM-3D, Spark, and Qunhe",
                "OPENAI_API_KEY or POOL_PROVIDER_API_KEY_OPENAI_IMAGE_2 / POOL_OPENAI_IMAGE_2_API_KEY for OpenAI image-2",
                "POOL_PROVIDER_PRODUCTION_ATTESTATION or every required per-provider POOL_PROVIDER_PRODUCTION_ATTESTATION_<PROVIDER> / POOL_<PROVIDER>_PRODUCTION_ATTESTATION"
            ],
            "required_for_desktop_vision_when_enabled": [
                "POOL_RUN_DESKTOP_VISION=1",
                "POOL_DESKTOP_VISION_TRACE",
                "POOL_DESKTOP_VISION_EXTERNAL_ACTION_ID",
                "POOL_DESKTOP_VISION_PRODUCTION_ATTESTATION"
            ],
            "required_for_software": [
                "POOL_UNREAL_MCP_ENDPOINT or POOL_SOFTWARE_UNREAL_ENDPOINT or POOL_UNREAL_ENDPOINT",
                "POOL_SOFTWARE_BLENDER_ENDPOINT or POOL_BLENDER_ENDPOINT or POOL_SOFTWARE_BLENDER_COMMAND or POOL_BLENDER_COMMAND",
                "POOL_SOFTWARE_COMFYUI_ENDPOINT or POOL_COMFYUI_ENDPOINT or POOL_SOFTWARE_COMFYUI_COMMAND or POOL_COMFYUI_COMMAND",
                "POOL_SOFTWARE_RESOLVE_ENDPOINT or POOL_RESOLVE_ENDPOINT or POOL_SOFTWARE_DAVINCI_RESOLVE_ENDPOINT or POOL_DAVINCI_RESOLVE_ENDPOINT or POOL_SOFTWARE_RESOLVE_COMMAND or POOL_RESOLVE_COMMAND or POOL_SOFTWARE_DAVINCI_RESOLVE_COMMAND or POOL_DAVINCI_RESOLVE_COMMAND",
                "POOL_SOFTWARE_UNITY_ENDPOINT or POOL_UNITY_ENDPOINT or POOL_SOFTWARE_UNITY_COMMAND or POOL_UNITY_COMMAND",
                "POOL_SOFTWARE_TOUCHDESIGNER_ENDPOINT or POOL_TOUCHDESIGNER_ENDPOINT or POOL_SOFTWARE_TOUCH_DESIGNER_ENDPOINT or POOL_TOUCH_DESIGNER_ENDPOINT or POOL_SOFTWARE_TOUCHDESIGNER_COMMAND or POOL_TOUCHDESIGNER_COMMAND or POOL_SOFTWARE_TOUCH_DESIGNER_COMMAND or POOL_TOUCH_DESIGNER_COMMAND",
                "POOL_SOFTWARE_MADMAPPER_ENDPOINT or POOL_MADMAPPER_ENDPOINT or POOL_SOFTWARE_MADMAPPER_COMMAND or POOL_MADMAPPER_COMMAND",
                "POOL_SOFTWARE_NUKE_ENDPOINT or POOL_NUKE_ENDPOINT or POOL_SOFTWARE_NUKE_COMMAND or POOL_NUKE_COMMAND",
                "POOL_SOFTWARE_MOTION_DB_ENDPOINT or POOL_MOTION_DB_ENDPOINT or POOL_SOFTWARE_MOCAP_DB_ENDPOINT or POOL_MOCAP_DB_ENDPOINT or POOL_SOFTWARE_MOTION_DB_COMMAND or POOL_MOTION_DB_COMMAND or POOL_SOFTWARE_MOCAP_DB_COMMAND or POOL_MOCAP_DB_COMMAND",
                "POOL_SOFTWARE_EDITING_SUITE_ENDPOINT or POOL_EDITING_SUITE_ENDPOINT or POOL_SOFTWARE_EDITOR_ENDPOINT or POOL_EDITOR_ENDPOINT or POOL_SOFTWARE_EDITING_SUITE_COMMAND or POOL_EDITING_SUITE_COMMAND or POOL_SOFTWARE_EDITOR_COMMAND or POOL_EDITOR_COMMAND",
                "POOL_HERMES_MCP_ENDPOINT or POOL_HERMES_ENDPOINT or POOL_SOFTWARE_HERMES_ENDPOINT or POOL_HERMES_COMMAND or POOL_SOFTWARE_HERMES_COMMAND"
            ],
            "required_for_software_attestation": [
                "POOL_SOFTWARE_PRODUCTION_ATTESTATION or per-adapter POOL_SOFTWARE_<ADAPTER>_PRODUCTION_ATTESTATION / POOL_<ADAPTER>_PRODUCTION_ATTESTATION"
            ],
            "required_for_software_artifacts": [
                "POOL_SOFTWARE_UNREAL_ARTIFACTS or POOL_UNREAL_ARTIFACTS",
                "POOL_SOFTWARE_BLENDER_ARTIFACTS or POOL_BLENDER_ARTIFACTS",
                "POOL_SOFTWARE_COMFYUI_ARTIFACTS or POOL_COMFYUI_ARTIFACTS",
                "POOL_SOFTWARE_RESOLVE_ARTIFACTS or POOL_RESOLVE_ARTIFACTS or POOL_SOFTWARE_DAVINCI_RESOLVE_ARTIFACTS or POOL_DAVINCI_RESOLVE_ARTIFACTS",
                "POOL_SOFTWARE_UNITY_ARTIFACTS or POOL_UNITY_ARTIFACTS",
                "POOL_SOFTWARE_TOUCHDESIGNER_ARTIFACTS or POOL_TOUCHDESIGNER_ARTIFACTS or POOL_SOFTWARE_TOUCH_DESIGNER_ARTIFACTS or POOL_TOUCH_DESIGNER_ARTIFACTS",
                "POOL_SOFTWARE_MADMAPPER_ARTIFACTS or POOL_MADMAPPER_ARTIFACTS",
                "POOL_SOFTWARE_NUKE_ARTIFACTS or POOL_NUKE_ARTIFACTS",
                "POOL_SOFTWARE_MOTION_DB_ARTIFACTS or POOL_MOTION_DB_ARTIFACTS or POOL_SOFTWARE_MOCAP_DB_ARTIFACTS or POOL_MOCAP_DB_ARTIFACTS",
                "POOL_SOFTWARE_EDITING_SUITE_ARTIFACTS or POOL_EDITING_SUITE_ARTIFACTS or POOL_SOFTWARE_EDITOR_ARTIFACTS or POOL_EDITOR_ARTIFACTS",
                "POOL_SOFTWARE_HERMES_ARTIFACTS or POOL_HERMES_ARTIFACTS"
            ],
            "optional_gates": [
                "POOL_CLI_CMD=<custom pool-cli invocation>",
                "POOL_IMPORT_PRODUCTION_EVIDENCE=1"
            ],
            "command_path_warnings": [
                "cargo is required only when the runner uses the cargo run -q -p pool-cli -- fallback or POOL_CLI_CMD starts with cargo",
                "pool-cli on PATH is preferred; runner falls back to cargo run -q -p pool-cli -- unless POOL_CLI_CMD overrides it"
            ],
            "provider_gateway_worker_start_commands": provider_gateway_worker_commands.clone(),
            "software_bridge_worker": software_bridge_worker.clone(),
            "software_bridge_worker_start_commands": software_bridge_worker_commands.clone()
        },
        "phases": [
            {
                "id": "provider_evidence_matrix",
                "command": provider_command,
                "required_env": [
                    "POOL_MEDIA_GATEWAY_ENDPOINT or every required AI media Provider endpoint env",
                    "POOL_3DGS_GATEWAY_ENDPOINT or every required 3DGS Provider endpoint env",
                    "OPENAI_API_KEY or provider-specific OpenAI image-2 API key env",
                    "POOL_PROVIDER_PRODUCTION_ATTESTATION or every required per-provider POOL_PROVIDER_PRODUCTION_ATTESTATION_<PROVIDER> / POOL_<PROVIDER>_PRODUCTION_ATTESTATION"
                ],
                "expected_outputs": [provider_bundle],
                "blocks_without_env": true,
                "provider_gateway_worker_start_commands": provider_gateway_worker_commands,
                "operator_note": "Shared POOL_MEDIA_GATEWAY_ENDPOINT / POOL_3DGS_GATEWAY_ENDPOINT can cover Provider families, or every required Provider can define its own POOL_PROVIDER_ENDPOINT_<PROVIDER> / POOL_<PROVIDER>_ENDPOINT. POOL_PROVIDER_PRODUCTION_ATTESTATION covers the full required provider matrix; without it, every required Provider must define its own production attestation. Placeholder, fake, mock, dummy, or todo text is rejected by the provider matrix runner."
            },
            {
                "id": "software_evidence_matrix",
                "command": software_command,
                "required_env": [
                "POOL_UNREAL_MCP_ENDPOINT",
                "POOL_SOFTWARE_PRODUCTION_ATTESTATION or per-adapter POOL_SOFTWARE_<ADAPTER>_PRODUCTION_ATTESTATION / POOL_<ADAPTER>_PRODUCTION_ATTESTATION",
                    "POOL_*_ENDPOINT backed by a real plugin/gateway or pool-cli software-api-bridge-worker, or POOL_*_COMMAND for Blender, ComfyUI, Resolve, Unity, TouchDesigner, MadMapper, Nuke, motion database, editing suite, and Hermes",
                "POOL_*_ARTIFACTS local file paths for every required software adapter"
                ],
                "expected_outputs": [software_bundle],
                "blocks_without_env": true,
                "generic_api_bridge_worker": software_bridge_worker,
                "bridge_worker_start_commands": software_bridge_worker_commands,
                "operator_note": "This phase only emits production evidence items for adapters with explicit real endpoint/command env, a production_attestation env, and local artifact env. Generic endpoint envs may point at pool-cli software-api-bridge-worker <adapter-id> when that worker forwards to a real plugin or gateway. Missing adapters fail instead of using local echo, mock profiles, or URI artifacts."
            },
            {
                "id": "desktop_vision_evidence",
                "command": desktop_command,
                "required_env": ["POOL_RUN_DESKTOP_VISION=1", "POOL_DESKTOP_VISION_TRACE", "POOL_DESKTOP_VISION_EXTERNAL_ACTION_ID", "POOL_DESKTOP_VISION_PRODUCTION_ATTESTATION"],
                "expected_outputs": [desktop_bundle],
                "optional_by_default": true,
                "skipped_bundle_source": "production-evidence-runner-desktop-skipped",
                "operator_note": "POOL_DESKTOP_VISION_TRACE must point to a local trace file produced by a real external visual/OCR/screen controller, and POOL_DESKTOP_VISION_PRODUCTION_ATTESTATION must identify that real run. Missing trace/action id/attestation leaves desktop_vision[] empty."
            },
            {
                "id": "merge_bundles",
                "command": merge_command,
                "required_inputs": [provider_bundle, software_bundle, desktop_bundle],
                "expected_outputs": [combined_bundle]
            },
            {
                "id": "closeout_preflight",
                "command": closeout_preflight_command,
                "required_inputs": [combined_bundle],
                "expected_outputs": [merged_bundle],
                "expected_writes": 0
            },
            {
                "id": "closeout_import",
                "command": closeout_import_command,
                "required_gate": "POOL_IMPORT_PRODUCTION_EVIDENCE=1",
                "required_inputs": [merged_bundle],
                "expected_writes": "provider_requests/software_actions/workflow_events"
            },
            {
                "id": "completion_gate",
                "command": completion_command,
                "required_gate": "POOL_IMPORT_PRODUCTION_EVIDENCE=1"
            }
        ],
        "preflight_contract": {
            "pass_condition": "runner --preflight exits 0 after required Provider endpoints, Provider production attestation, software endpoint/command env, software production attestation, software artifact env with local paths, desktop endpoint when enabled, and cargo for local fallback commands are present",
            "failure_condition": "runner --preflight exits non-zero before any external Provider/software budget is spent",
            "truth_source": "4-production-evidence-run-plan.json commands plus this generated preflight contract"
        }
    })
}

fn production_evidence_run_plan_path(run_plan: &Value, key: &str, fallback: &str) -> String {
    run_plan
        .pointer(&format!("/paths/{key}"))
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .unwrap_or_else(|| fallback.to_string())
}

fn production_evidence_phase_command(run_plan: &Value, phase_id: &str) -> Option<String> {
    run_plan
        .get("phases")
        .and_then(Value::as_array)?
        .iter()
        .find(|phase| phase.get("id").and_then(Value::as_str) == Some(phase_id))?
        .get("command")
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn default_production_evidence_provider_gateway_worker_commands(output_root: &str) -> Value {
    json!([
        {
            "family": "ai_media",
            "applies_to": ["midjourney", "nano-banana-pro", "suno"],
            "endpoint_env": "POOL_MEDIA_GATEWAY_ENDPOINT",
            "endpoint_assignment": "POOL_MEDIA_GATEWAY_ENDPOINT=http://127.0.0.1:<port>",
            "upstream_env": "POOL_MEDIA_GATEWAY_UPSTREAM_ENDPOINT",
            "api_key_env": "POOL_MEDIA_GATEWAY_API_KEY",
            "cli": format!("pool-cli provider-gateway-worker --bind 127.0.0.1:<port> --upstream $POOL_MEDIA_GATEWAY_UPSTREAM_ENDPOINT --api-key-env POOL_MEDIA_GATEWAY_API_KEY"),
            "output_root": output_root,
            "production_rule": "The worker is production-valid only when --upstream or --provider-upstream routes to real AI media vendor workers, official SDK wrappers, or approved gateway services; provider URLs remain provenance and local downloaded files are authoritative."
        },
        {
            "family": "3dgs",
            "applies_to": ["worldlabs-marble", "tripo-splat", "sam-3d", "spark-3dgs", "qunhe-3d"],
            "endpoint_env": "POOL_3DGS_GATEWAY_ENDPOINT",
            "endpoint_assignment": "POOL_3DGS_GATEWAY_ENDPOINT=http://127.0.0.1:<port>",
            "upstream_env": "POOL_3DGS_GATEWAY_UPSTREAM_ENDPOINT",
            "api_key_env": "POOL_3DGS_GATEWAY_API_KEY",
            "cli": format!("pool-cli provider-gateway-worker --bind 127.0.0.1:<port> --upstream $POOL_3DGS_GATEWAY_UPSTREAM_ENDPOINT --api-key-env POOL_3DGS_GATEWAY_API_KEY"),
            "output_root": output_root,
            "production_rule": "The worker is production-valid only when --upstream or --provider-upstream routes to real 3DGS vendor workers, official SDK wrappers, or approved gateway services; indexed local 3DGS assets are the loading source of truth."
        }
    ])
}

fn production_evidence_provider_gateway_shell_hint(commands: &Value) -> String {
    let Some(command_values) = commands.as_array() else {
        return String::new();
    };
    if command_values.is_empty() {
        return String::new();
    }

    let mut lines = vec![
        "  echo \"INFO provider gateway workers can cover AI media and 3DGS provider families.\"".to_string(),
        "  echo \"INFO provider workers are production-valid only when --upstream routes to real vendor workers or SDK wrappers.\"".to_string(),
    ];
    for command in command_values {
        let family = command
            .get("family")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let endpoint_assignment = command
            .get("endpoint_assignment")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let upstream_env = command
            .get("upstream_env")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let cli = command
            .get("cli")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let endpoint_assignment = shell_double_quote_echo_fragment(endpoint_assignment);
        let upstream_env = shell_double_quote_echo_fragment(upstream_env);
        let cli = shell_double_quote_echo_fragment(cli);
        lines.push(format!(
            "  echo \"INFO provider gateway {family}: {endpoint_assignment}; upstream={upstream_env}; {cli}\""
        ));
    }
    lines.join("\n")
}

fn default_production_evidence_software_bridge_worker() -> Value {
    json!({
        "applies_to": ["blender", "comfyui", "resolve", "unity", "nuke", "motion-db", "editing-suite"],
        "cli_template": "pool-cli software-api-bridge-worker <adapter-id> --bind 127.0.0.1:<port> --output-root worlds/<project>/output --upstream <real-plugin-or-gateway-url>",
        "endpoint_env_template": "POOL_<ADAPTER>_ENDPOINT=http://127.0.0.1:<port>",
        "operator_note": "Use the local bridge worker only as an audit/forwarder in production evidence mode; the upstream behind --upstream must be a real software plugin, API, MCP service, or gateway."
    })
}

fn production_evidence_bridge_worker_commands(output_root: &str, bridge_worker: &Value) -> Value {
    let commands = bridge_worker
        .get("applies_to")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::trim)
        .filter(|adapter_id| !adapter_id.is_empty())
        .map(|adapter_id| {
            let env_key = production_evidence_adapter_env_key(adapter_id);
            let endpoint_env = format!("POOL_{env_key}_ENDPOINT");
            let upstream_env = format!("POOL_{env_key}_UPSTREAM_ENDPOINT");
            json!({
                "adapter_id": adapter_id,
                "endpoint_env": endpoint_env,
                "endpoint_assignment": format!("POOL_{env_key}_ENDPOINT=http://127.0.0.1:<port>"),
                "upstream_env": upstream_env,
                "cli": format!("pool-cli software-api-bridge-worker {adapter_id} --bind 127.0.0.1:<port> --output-root {output_root} --upstream ${upstream_env}"),
                "production_rule": "The bridge worker is production-valid only when --upstream points to a real software plugin, API, MCP service, or gateway, and local artifacts are written through POOL_*_ARTIFACTS."
            })
        })
        .collect::<Vec<_>>();
    json!(commands)
}

fn production_evidence_bridge_worker_shell_hint(
    output_root: &str,
    bridge_worker: &Value,
) -> String {
    let commands = production_evidence_bridge_worker_commands(output_root, bridge_worker);
    let Some(command_values) = commands.as_array() else {
        return String::new();
    };
    if command_values.is_empty() {
        return String::new();
    }

    let mut lines = vec![
        "  echo \"INFO generic software API/MCP bridge workers can cover adapters without native MCP/API endpoints.\"".to_string(),
        "  echo \"INFO bridge workers are production-valid only when --upstream points to a real software plugin/gateway.\"".to_string(),
    ];
    for command in command_values {
        let adapter_id = command
            .get("adapter_id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let endpoint_assignment = command
            .get("endpoint_assignment")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let cli = command
            .get("cli")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let endpoint_assignment = shell_double_quote_echo_fragment(endpoint_assignment);
        let cli = shell_double_quote_echo_fragment(cli);
        lines.push(format!(
            "  echo \"INFO bridge worker {adapter_id}: {endpoint_assignment}; {cli}\""
        ));
    }
    lines.join("\n")
}

fn production_evidence_adapter_env_key(adapter_id: &str) -> String {
    adapter_id
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

fn shell_double_quote_echo_fragment(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('$', "\\$")
        .replace('`', "\\`")
}

fn production_evidence_phase_value(run_plan: &Value, phase_id: &str, key: &str) -> Option<Value> {
    run_plan
        .get("phases")
        .and_then(Value::as_array)?
        .iter()
        .find(|phase| phase.get("id").and_then(Value::as_str) == Some(phase_id))?
        .get(key)
        .cloned()
}

fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', r#"'\''"#))
}

fn write_prd_completion_package(
    package_dir: &Path,
    project_slug: &str,
    node_id: Option<&str>,
    title: &str,
    source: &str,
    include_snapshot: bool,
    snapshot: &RuntimeSnapshot,
) -> Result<Value> {
    let created_at = chrono::Utc::now().to_rfc3339();
    let readiness = runtime_prd_readiness_resource(snapshot)?;
    let completion_gate = runtime_prd_completion_gate_resource(snapshot)?;
    let production_requirements = runtime_production_evidence_requirements_resource(snapshot)?;
    let ready_for_completion = completion_gate
        .pointer("/completion_gate/ready_for_completion")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let status = completion_gate
        .pointer("/completion_gate/status")
        .and_then(Value::as_str)
        .unwrap_or("unknown");

    fs::create_dir_all(package_dir).with_context(|| {
        format!(
            "create PRD completion package dir {}",
            package_dir.display()
        )
    })?;

    let request_path = package_dir.join(".1-prd-completion-package-request.json");
    let readiness_path = package_dir.join("1-prd-readiness.json");
    let completion_gate_path = package_dir.join("2-prd-completion-gate.json");
    let production_requirements_path = package_dir.join("3-production-evidence-requirements.json");
    let manifest_path = package_dir.join("4-prd-completion-package-manifest.json");
    let snapshot_path = include_snapshot.then(|| package_dir.join("5-runtime-snapshot.json"));

    let request_value = json!({
        "kind": "pool_prd_completion_package_request",
        "project_slug": project_slug,
        "node_id": node_id,
        "title": title,
        "source": source,
        "include_snapshot": include_snapshot,
        "created_at": created_at,
        "local_files_authoritative": true,
        "provider_urls_are_provenance": true,
    });
    write_server_json_file(&request_path, &request_value)?;
    write_server_json_file(&readiness_path, &readiness)?;
    write_server_json_file(&completion_gate_path, &completion_gate)?;
    write_server_json_file(&production_requirements_path, &production_requirements)?;
    if let Some(snapshot_path) = &snapshot_path {
        write_server_json_file(snapshot_path, &serde_json::to_value(snapshot)?)?;
    }

    let mut local_paths = vec![
        path_string_lossy(&request_path),
        path_string_lossy(&readiness_path),
        path_string_lossy(&completion_gate_path),
        path_string_lossy(&production_requirements_path),
    ];
    if let Some(snapshot_path) = &snapshot_path {
        local_paths.push(path_string_lossy(snapshot_path));
    }

    let manifest = json!({
        "kind": "pool_prd_completion_package_manifest",
        "version": 1,
        "project_slug": project_slug,
        "node_id": node_id,
        "title": title,
        "created_at": created_at,
        "source": source,
        "status": status,
        "ready_for_completion": ready_for_completion,
        "summary": {
            "readiness": readiness.get("summary").cloned().unwrap_or_else(|| json!({})),
            "completion_gate": completion_gate.get("completion_gate").cloned().unwrap_or_else(|| json!({})),
            "production_evidence": production_requirements.get("summary").cloned().unwrap_or_else(|| json!({})),
        },
        "paths": {
            "request": path_string_lossy(&request_path),
            "readiness": path_string_lossy(&readiness_path),
            "completion_gate": path_string_lossy(&completion_gate_path),
            "production_evidence_requirements": path_string_lossy(&production_requirements_path),
            "manifest": path_string_lossy(&manifest_path),
            "snapshot": snapshot_path.as_ref().map(|path| path_string_lossy(path)),
        },
        "commands": {
            "readiness": format!("pool-cli --project {project_slug} prd-readiness"),
            "completion_gate": format!("pool-cli --project {project_slug} prd-completion-gate --require-complete"),
            "production_evidence_requirements": format!("pool-cli --project {project_slug} production-evidence-requirements"),
            "production_evidence_handoff_package": format!("pool-cli --project {project_slug} production-evidence-handoff-package --output-dir worlds/{project_slug}/output --output-root worlds/{project_slug}/output/production-evidence --include-snapshot"),
            "closeout_preflight": format!("pool-cli --project {project_slug} closeout-production-evidence --output <merged-bundle.json> <provider-bundle.json> <software-bundle.json> <desktop-vision-bundle.json>"),
            "closeout_import": format!("pool-cli --project {project_slug} closeout-production-evidence --import <merged-bundle.json>"),
            "completion_package": format!("pool-cli --project {project_slug} prd-completion-package --output-dir worlds/{project_slug}/output --include-snapshot"),
        },
        "operator_checklist": [
            "Archive this package with the production evidence bundle used for final PRD review.",
            "If ready_for_completion is false, use incomplete_requirements and production evidence requirements before claiming completion.",
            "If ready_for_completion is true, keep the local files in this package as the completion proof source."
        ],
    });
    write_server_json_file(&manifest_path, &manifest)?;
    local_paths.push(path_string_lossy(&manifest_path));

    Ok(json!({
        "status": "Succeeded",
        "project_slug": project_slug,
        "node_id": node_id,
        "title": title,
        "source": source,
        "package_dir": path_string_lossy(package_dir),
        "request_path": path_string_lossy(&request_path),
        "readiness_path": path_string_lossy(&readiness_path),
        "completion_gate_path": path_string_lossy(&completion_gate_path),
        "production_evidence_requirements_path": path_string_lossy(&production_requirements_path),
        "manifest_path": path_string_lossy(&manifest_path),
        "snapshot_path": snapshot_path.as_ref().map(|path| path_string_lossy(path)),
        "ready_for_completion": ready_for_completion,
        "completion_status": status,
        "local_paths": local_paths,
    }))
}

fn write_server_json_file(path: &Path, value: &Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create parent dir {}", parent.display()))?;
    }
    fs::write(
        path,
        serde_json::to_string_pretty(value).context("serialize runtime server json file")?,
    )
    .with_context(|| format!("write runtime server json file {}", path.display()))
}

fn write_server_text_file(path: &Path, body: &str, executable: bool) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create parent dir {}", parent.display()))?;
    }
    fs::write(path, body)
        .with_context(|| format!("write runtime server text file {}", path.display()))?;
    if executable {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = fs::metadata(path)
                .with_context(|| format!("read permissions for {}", path.display()))?
                .permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(path, permissions)
                .with_context(|| format!("set executable bit on {}", path.display()))?;
        }
    }
    Ok(())
}

fn path_string_lossy(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

fn production_evidence_file_slug(value: &str) -> String {
    let mut slug = String::new();
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            slug.push(character.to_ascii_lowercase());
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }
    slug.trim_matches('-').to_string()
}

fn production_evidence_template_value(
    project_slug: &str,
    output_root: Option<&str>,
    source: &str,
    scope: ProductionEvidenceTemplateScope,
) -> Value {
    let providers = scope
        .providers
        .iter()
        .map(|provider_id| {
            let artifact_path = production_template_path(
                output_root,
                &format!(
                    "worlds/{project_slug}/output/production/{provider_id}/{}",
                    production_template_provider_artifact_name(provider_id)
                ),
            );
            let metadata_path = production_template_path(
                output_root,
                &format!(
                    "worlds/{project_slug}/output/production/{provider_id}/request-metadata.json"
                ),
            );
            json!({
                "provider_id": provider_id,
                "external_job_id": format!("replace-with-real-{provider_id}-job-id"),
                "endpoint": format!("https://worker.example.com/{provider_id}"),
                "family": production_template_provider_family(provider_id),
                "production_attestation": format!("replace-with-real-{provider_id}-worker-attestation"),
                "metadata_path": metadata_path,
                "artifacts": [artifact_path],
                "evidence_json": {
                    "source": source,
                    "replace_with": "real provider worker evidence",
                    "production_upstream": true,
                    "local_mock_gateway": false,
                    "production_attestation": format!("replace-with-real-{provider_id}-worker-attestation")
                }
            })
        })
        .collect::<Vec<_>>();
    let software_actions = scope
        .software
        .iter()
        .map(|adapter_id| {
            let (action_kind, priority, control_profile, artifact) =
                production_template_software_profile(adapter_id, project_slug, output_root);
            json!({
                "adapter_id": adapter_id,
                "external_action_id": format!("replace-with-real-{adapter_id}-action-id"),
                "production_attestation": format!("replace-with-real-{adapter_id}-software-run-attestation"),
                "action_kind": action_kind,
                "priority": priority,
                "control_profile": control_profile,
                "artifacts": [artifact],
                "evidence_json": {
                    "source": source,
                    "replace_with": "real software plugin/API/MCP/CLI evidence",
                    "production_software": true,
                    "local_mock_software": false,
                    "production_attestation": format!("replace-with-real-{adapter_id}-software-run-attestation")
                }
            })
        })
        .collect::<Vec<_>>();
    let desktop_trace_path = production_template_path(
        output_root,
        &format!(
            "worlds/{project_slug}/output/production/desktop-vision/1-touchdesigner-trace.json"
        ),
    );
    let desktop_vision = if scope.include_desktop_vision {
        vec![json!({
            "adapter_id": "touchdesigner",
            "external_action_id": "replace-with-real-desktop-vision-action-id",
            "controller_id": "replace-with-real-vision-controller-id",
            "production_attestation": "replace-with-real-desktop-vision-controller-attestation",
            "trace_path": desktop_trace_path,
            "visual_model": "external",
            "artifacts": [desktop_trace_path],
            "evidence_json": {
                "source": source,
                "replace_with": "real desktop visual model trace",
                "external_visual_model": true,
                "local_trace_smoke": false,
                "production_attestation": "replace-with-real-desktop-vision-controller-attestation"
            }
        })]
    } else {
        Vec::new()
    };
    let bundle = json!({
        "project_slug": project_slug,
        "source": source,
        "providers": providers,
        "software_actions": software_actions,
        "desktop_vision": desktop_vision,
    });
    let output_root_display = output_root.unwrap_or(".");

    json!({
        "kind": "pool_production_evidence_bundle_template",
        "version": 1,
        "project_slug": project_slug,
        "ready_for_import": false,
        "reason": "Template identifiers and fixture paths must be replaced with real external job/action/controller ids and local files before import.",
        "scope": {
            "mode": scope.mode,
            "missing_only": scope.mode == "missing_only",
            "provider_targets": scope.providers.clone(),
            "software_targets": scope.software.clone(),
            "desktop_vision_target": scope.include_desktop_vision,
        },
        "output_root": output_root_display,
        "bundle": bundle,
        "artifact_plan": {
            "providers": production_template_provider_artifact_plan(project_slug, output_root, &scope.providers),
            "desktop_vision": if scope.include_desktop_vision { vec![json!({
                "adapter_id": "touchdesigner",
                "trace_path": production_template_path(
                    output_root,
                    &format!("worlds/{project_slug}/output/production/desktop-vision/1-touchdesigner-trace.json"),
                ),
                "required": true,
            })] } else { Vec::<Value>::new() },
        },
        "operator_checklist": [
            "Replace every external_job_id, production_attestation, external_action_id, and controller_id with real upstream ids or run attestations.",
            "Download provider artifacts and request metadata to the exact local paths in providers[].artifacts and providers[].metadata_path.",
            "Write a desktop vision trace with visual_model:\"external\" or evidence_json.external_visual_model:true.",
            "Run closeout-production-evidence or validate-production-evidence before import-production-evidence; validation must report writes:0 and artifact_files.complete:true.",
            "Import only after the evidence was produced by real provider workers, software plugins/API/MCP/CLI, or a real visual controller."
        ],
        "commands": {
            "closeout": format!("pool-cli --project {project_slug} closeout-production-evidence --output <merged-bundle.json> <bundle-a.json> <bundle-b.json>..."),
            "validate": format!("pool-cli --project {project_slug} validate-production-evidence <bundle.json>"),
            "import": format!("pool-cli --project {project_slug} import-production-evidence <bundle.json>"),
        }
    })
}

fn production_evidence_item_selector(
    request: &RuntimeHttpRequest,
) -> Result<(String, String, Option<String>)> {
    if let Some(task_id) = request
        .query
        .get("task_id")
        .or_else(|| request.query.get("task-id"))
        .map(String::as_str)
        .filter(|value| !value.trim().is_empty())
    {
        let (kind, target_id) = production_evidence_selector_from_task_id(task_id)?;
        return Ok((kind, target_id, Some(task_id.to_string())));
    }

    let kind = request
        .query
        .get("kind")
        .map(String::as_str)
        .filter(|value| !value.trim().is_empty())
        .context("production evidence item template requires kind or task_id")?;
    let kind = normalize_production_evidence_item_kind(kind)?;
    let target_id = request
        .query
        .get("target_id")
        .or_else(|| request.query.get("target-id"))
        .or_else(|| request.query.get("provider_id"))
        .or_else(|| request.query.get("provider-id"))
        .or_else(|| request.query.get("adapter_id"))
        .or_else(|| request.query.get("adapter-id"))
        .or_else(|| request.query.get("missing_id"))
        .or_else(|| request.query.get("missing-id"))
        .map(String::as_str)
        .filter(|value| !value.trim().is_empty())
        .context("production evidence item template requires target_id")?;

    Ok((kind, target_id.to_string(), None))
}

fn production_evidence_selector_from_task_id(task_id: &str) -> Result<(String, String)> {
    if let Some(rest) = task_id.strip_prefix("provider:") {
        let target_id = rest
            .split(':')
            .next()
            .filter(|value| !value.trim().is_empty())
            .context("provider production evidence task_id requires target id")?;
        return Ok(("provider".to_string(), target_id.to_string()));
    }
    if let Some(rest) = task_id.strip_prefix("software:") {
        let target_id = rest
            .split(':')
            .next()
            .filter(|value| !value.trim().is_empty())
            .context("software production evidence task_id requires target id")?;
        return Ok(("software_action".to_string(), target_id.to_string()));
    }
    if let Some(rest) = task_id.strip_prefix("desktop_vision:") {
        let target_id = rest
            .split(':')
            .next()
            .filter(|value| !value.trim().is_empty())
            .context("desktop vision production evidence task_id requires target id")?;
        return Ok(("desktop_vision".to_string(), target_id.to_string()));
    }
    bail!("task_id must start with provider:, software:, or desktop_vision:")
}

fn normalize_production_evidence_item_kind(kind: &str) -> Result<String> {
    match kind.trim().to_ascii_lowercase().as_str() {
        "provider" | "providers" | "provider_production_upstream" => Ok("provider".to_string()),
        "software" | "software_action" | "software_actions" | "software_production" => {
            Ok("software_action".to_string())
        }
        "desktop" | "desktop_vision" | "vision" => Ok("desktop_vision".to_string()),
        _ => bail!(
            "production evidence item kind must be provider, software_action, or desktop_vision"
        ),
    }
}

fn production_evidence_item_task_id(kind: &str, target_id: &str) -> String {
    match kind {
        "provider" => format!("provider:{target_id}:production_upstream"),
        "software_action" => format!("software:{target_id}:production_software"),
        "desktop_vision" => format!("desktop_vision:{target_id}"),
        _ => format!("{kind}:{target_id}"),
    }
}

fn production_evidence_item_template_value(
    project_slug: &str,
    output_root: Option<&str>,
    source: &str,
    kind: &str,
    target_id: &str,
    task_id: Option<&str>,
) -> Result<Value> {
    let kind = normalize_production_evidence_item_kind(kind)?;
    let target_id = target_id.trim();
    if target_id.is_empty() {
        bail!("production evidence item template target_id cannot be empty");
    }
    let task_id = task_id
        .map(ToString::to_string)
        .unwrap_or_else(|| production_evidence_item_task_id(&kind, target_id));
    let item = match kind.as_str() {
        "provider" => {
            let artifact_path = production_template_path(
                output_root,
                &format!(
                    "worlds/{project_slug}/output/production/{target_id}/{}",
                    production_template_provider_artifact_name(target_id)
                ),
            );
            let metadata_path = production_template_path(
                output_root,
                &format!(
                    "worlds/{project_slug}/output/production/{target_id}/request-metadata.json"
                ),
            );
            json!({
                "project_slug": project_slug,
                "source": source,
                "kind": "provider",
                "provider": {
                    "provider_id": target_id,
                    "external_job_id": format!("replace-with-real-{target_id}-job-id"),
                    "endpoint": format!("https://worker.example.com/{target_id}"),
                    "family": production_template_provider_family(target_id),
                    "production_attestation": format!("replace-with-real-{target_id}-worker-attestation"),
                    "metadata_path": metadata_path,
                    "artifacts": [artifact_path],
                    "evidence_json": {
                        "source": source,
                        "task_id": task_id,
                        "replace_with": "real provider worker evidence",
                        "production_upstream": true,
                        "local_mock_gateway": false,
                        "production_attestation": format!("replace-with-real-{target_id}-worker-attestation")
                    }
                }
            })
        }
        "software_action" => {
            let (action_kind, priority, control_profile, artifact) =
                production_template_software_profile(target_id, project_slug, output_root);
            json!({
                "project_slug": project_slug,
                "source": source,
                "kind": "software_action",
                "software_action": {
                    "adapter_id": target_id,
                    "external_action_id": format!("replace-with-real-{target_id}-action-id"),
                    "production_attestation": format!("replace-with-real-{target_id}-software-run-attestation"),
                    "action_kind": action_kind,
                    "priority": priority,
                    "control_profile": control_profile,
                    "artifacts": [artifact],
                    "evidence_json": {
                        "source": source,
                        "task_id": task_id,
                        "replace_with": "real software plugin/API/MCP/CLI evidence",
                        "production_software": true,
                        "local_mock_software": false,
                        "production_attestation": format!("replace-with-real-{target_id}-software-run-attestation")
                    }
                }
            })
        }
        "desktop_vision" => {
            let trace_path = production_template_path(
                output_root,
                &format!(
                    "worlds/{project_slug}/output/production/desktop-vision/{target_id}-trace.json"
                ),
            );
            json!({
                "project_slug": project_slug,
                "source": source,
                "kind": "desktop_vision",
                "desktop_vision": {
                    "adapter_id": "touchdesigner",
                    "external_action_id": format!("replace-with-real-{target_id}-action-id"),
                    "controller_id": "replace-with-real-vision-controller-id",
                    "production_attestation": format!("replace-with-real-{target_id}-controller-attestation"),
                    "trace_path": trace_path,
                    "visual_model": "external",
                    "artifacts": [trace_path],
                    "evidence_json": {
                        "source": source,
                        "task_id": task_id,
                        "target_id": target_id,
                        "replace_with": "real desktop visual model trace",
                        "external_visual_model": true,
                        "local_trace_smoke": false,
                        "production_attestation": format!("replace-with-real-{target_id}-controller-attestation")
                    }
                }
            })
        }
        _ => bail!(
            "production evidence item kind must be provider, software_action, or desktop_vision"
        ),
    };

    Ok(json!({
        "kind": "pool_production_evidence_item_template",
        "version": 1,
        "project_slug": project_slug,
        "ready_for_import": false,
        "reason": "Template identifiers and fixture paths must be replaced with real external job/action/controller ids and local files before submit.",
        "selector": {
            "task_id": task_id,
            "kind": kind,
            "target_id": target_id,
        },
        "output_root": output_root.unwrap_or("."),
        "item": item,
        "commands": {
            "submit": format!("pool-cli --project {project_slug} submit-production-evidence-item <item.json>"),
            "tasks": format!("pool-cli --project {project_slug} production-evidence-tasks"),
            "readiness": format!("pool-cli --project {project_slug} prd-readiness"),
        },
        "http": {
            "tasks": format!("GET /api/production-evidence/tasks?project={project_slug}"),
            "item_template": format!("GET /api/production-evidence/item-template?project={project_slug}&task_id={}", task_id),
            "submit_item": "POST /api/production-evidence/items",
        },
        "mcp": {
            "tasks_tool": "pool_production_evidence_tasks",
            "item_template_tool": "pool_production_evidence_item_template",
            "submit_tool": "pool_submit_production_evidence_item",
        },
        "operator_checklist": [
            "Replace external_job_id, production_attestation, external_action_id, or controller_id with the real upstream identifier or run attestation.",
            "Write every artifact, metadata_path, or trace_path in the item to local files before submit.",
            "Keep production_upstream/production_software/external_visual_model true only for real external evidence.",
            "Submit the item with submit-production-evidence-item after local files exist."
        ],
    }))
}

fn production_evidence_item_from_provider_ledger(
    project_slug: &str,
    source: &str,
    snapshot: &RuntimeSnapshot,
    provider_request_id: &str,
) -> Result<Value> {
    let provider_request = snapshot
        .provider_requests
        .iter()
        .find(|request| request.id == provider_request_id)
        .with_context(|| format!("provider request not found: {provider_request_id}"))?;
    let task = snapshot
        .tasks
        .iter()
        .find(|task| task.id == provider_request.task_id);
    let provider_id = provider_request.provider_id.clone();
    let artifacts = provider_ledger_artifacts(provider_request, task, &snapshot.assets);
    let external_job_id = provider_request
        .response
        .as_ref()
        .and_then(|response| string_at_pointer(response, "/job/external_job_id"))
        .or_else(|| {
            provider_request
                .response
                .as_ref()
                .and_then(|response| string_at_pointer(response, "/external_job_id"))
        })
        .or_else(|| {
            provider_request
                .response
                .as_ref()
                .and_then(|response| string_at_pointer(response, "/job/id"))
        })
        .unwrap_or_else(|| format!("replace-with-real-{provider_id}-job-id"));
    let metadata_path = provider_request.metadata_path.clone().or_else(|| {
        provider_request
            .response
            .as_ref()
            .and_then(|response| string_at_pointer(response, "/job/request_metadata_path"))
    });
    let mut evidence = provider_request
        .request
        .get("evidence")
        .cloned()
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}));
    json_object_insert(&mut evidence, "source", json!(source));
    json_object_insert(
        &mut evidence,
        "provider_request_id",
        json!(provider_request.id),
    );
    json_object_insert(&mut evidence, "task_id", json!(provider_request.task_id));
    if evidence.get("production_upstream").is_none() {
        json_object_insert(&mut evidence, "production_upstream", json!(false));
    }
    if evidence.get("local_mock_gateway").is_none() {
        json_object_insert(&mut evidence, "local_mock_gateway", json!(true));
    }

    let item = json!({
        "project_slug": project_slug,
        "source": source,
        "kind": "provider",
        "provider": {
            "provider_id": provider_id,
            "external_job_id": external_job_id,
            "endpoint": provider_request.request.get("endpoint").and_then(Value::as_str),
            "family": production_template_provider_family(&provider_id),
            "production_attestation": evidence
                .get("production_attestation")
                .and_then(Value::as_str),
            "node_id": task.and_then(|task| task.node_id.clone()),
            "task_title": task.map(|task| task.title.clone()),
            "metadata_path": metadata_path,
            "artifacts": artifacts,
            "evidence_json": evidence,
            "response_json": provider_request.response,
        }
    });
    production_evidence_item_from_ledger_value(
        project_slug,
        source,
        "provider_request",
        provider_request_id,
        item,
    )
}

fn production_evidence_item_from_software_ledger(
    project_slug: &str,
    source: &str,
    snapshot: &RuntimeSnapshot,
    software_action_id: &str,
) -> Result<Value> {
    let software_action = snapshot
        .software_actions
        .iter()
        .find(|action| action.id == software_action_id)
        .with_context(|| format!("software action not found: {software_action_id}"))?;
    let task = software_action
        .task_id
        .as_ref()
        .and_then(|task_id| snapshot.tasks.iter().find(|task| task.id == *task_id));
    let adapter_id = canonical_production_software_adapter_id(&software_action.adapter_id);
    let artifacts = software_action
        .verification
        .as_ref()
        .map(|verification| json_string_array_at(verification, "artifacts"))
        .unwrap_or_default();
    let external_action_id = software_action
        .verification
        .as_ref()
        .and_then(|verification| string_at_pointer(verification, "/external_action_id"))
        .or_else(|| {
            software_action
                .verification
                .as_ref()
                .and_then(|verification| string_at_pointer(verification, "/action_id"))
        })
        .or_else(|| string_at_pointer(&software_action.command, "/payload_json/external_action_id"))
        .or_else(|| {
            string_at_pointer(
                &software_action.command,
                "/payload_json/evidence/external_action_id",
            )
        })
        .unwrap_or_else(|| format!("replace-with-real-{adapter_id}-action-id"));
    let mut evidence = software_action
        .command
        .pointer("/payload_json/evidence")
        .cloned()
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}));
    json_object_insert(&mut evidence, "source", json!(source));
    json_object_insert(
        &mut evidence,
        "software_action_id",
        json!(software_action.id),
    );
    if let Some(task_id) = software_action.task_id.as_ref() {
        json_object_insert(&mut evidence, "task_id", json!(task_id));
    }
    if evidence.get("production_software").is_none() {
        json_object_insert(&mut evidence, "production_software", json!(false));
    }
    if evidence.get("local_mock_software").is_none() {
        json_object_insert(&mut evidence, "local_mock_software", json!(true));
    }
    let production_attestation = evidence
        .get("production_attestation")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);

    let item = json!({
        "project_slug": project_slug,
        "source": source,
        "kind": "software_action",
        "software_action": {
            "adapter_id": adapter_id,
            "external_action_id": external_action_id,
            "production_attestation": production_attestation,
            "action_kind": software_action.action_kind,
            "priority": software_action.command.get("priority").and_then(Value::as_str),
            "control_profile": software_action.command.get("priority").and_then(Value::as_str),
            "node_id": task.and_then(|task| task.node_id.clone()),
            "task_title": task.map(|task| task.title.clone()),
            "artifacts": artifacts,
            "evidence_json": evidence,
            "verification_json": software_action.verification,
        }
    });
    production_evidence_item_from_ledger_value(
        project_slug,
        source,
        "software_action",
        software_action_id,
        item,
    )
}

fn production_evidence_item_from_desktop_vision_ledger(
    project_slug: &str,
    source: &str,
    snapshot: &RuntimeSnapshot,
    software_action_id: &str,
) -> Result<Value> {
    let software_action = snapshot
        .software_actions
        .iter()
        .find(|action| action.id == software_action_id)
        .with_context(|| {
            format!("desktop vision software action not found: {software_action_id}")
        })?;
    if !is_desktop_recognition_action(software_action) {
        bail!("software action is not a desktop recognition action: {software_action_id}");
    }
    let verification = software_action
        .verification
        .as_ref()
        .context("desktop vision action has no verification result")?;
    let task = software_action
        .task_id
        .as_ref()
        .and_then(|task_id| snapshot.tasks.iter().find(|task| task.id == *task_id));
    let adapter_id = canonical_production_software_adapter_id(&software_action.adapter_id);
    let artifacts = desktop_vision_ledger_artifacts(verification);
    let external_action_id = string_at_pointer(verification, "/external_action_id")
        .or_else(|| string_at_pointer(verification, "/action_id"))
        .or_else(|| string_at_pointer(verification, "/controller_result/external_action_id"))
        .unwrap_or_else(|| format!("replace-with-real-{adapter_id}-desktop-vision-action-id"));
    let controller_id = string_at_pointer(verification, "/controller_id")
        .or_else(|| string_at_pointer(verification, "/controller"))
        .or_else(|| string_at_pointer(verification, "/controller_result/controller_id"))
        .or_else(|| string_at_pointer(verification, "/controller_result/controller"))
        .unwrap_or_else(|| "replace-with-real-vision-controller-id".to_string());
    let trace_path = string_at_pointer(verification, "/screen_trace_path")
        .or_else(|| string_at_pointer(verification, "/trace_path"))
        .or_else(|| string_at_pointer(verification, "/controller_result/screen_trace_path"))
        .or_else(|| string_at_pointer(verification, "/controller_result/trace_path"))
        .unwrap_or_else(|| {
            format!("worlds/{project_slug}/output/production/desktop-vision/{software_action_id}-trace.json")
        });
    let external_visual_model = production_evidence_json_external_visual_model(Some(verification));
    let visual_model = if external_visual_model {
        "external"
    } else {
        "replace-with-external-visual-model"
    };
    let mut evidence = software_action
        .command
        .pointer("/payload_json/evidence")
        .cloned()
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}));
    json_object_insert(&mut evidence, "source", json!(source));
    json_object_insert(
        &mut evidence,
        "desktop_vision_action_id",
        json!(software_action.id),
    );
    if let Some(task_id) = software_action.task_id.as_ref() {
        json_object_insert(&mut evidence, "task_id", json!(task_id));
    }
    if evidence.get("external_visual_model").is_none() {
        json_object_insert(
            &mut evidence,
            "external_visual_model",
            json!(external_visual_model),
        );
    }
    if evidence.get("local_trace_smoke").is_none() {
        json_object_insert(
            &mut evidence,
            "local_trace_smoke",
            json!(!external_visual_model),
        );
    }
    let production_attestation = evidence
        .get("production_attestation")
        .and_then(Value::as_str)
        .or_else(|| {
            verification
                .get("production_attestation")
                .and_then(Value::as_str)
        })
        .or_else(|| {
            verification
                .pointer("/controller_result/production_attestation")
                .and_then(Value::as_str)
        })
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);

    let item = json!({
        "project_slug": project_slug,
        "source": source,
        "kind": "desktop_vision",
        "desktop_vision": {
            "adapter_id": adapter_id,
            "external_action_id": external_action_id,
            "controller_id": controller_id,
            "production_attestation": production_attestation,
            "trace_path": trace_path,
            "visual_model": visual_model,
            "node_id": task.and_then(|task| task.node_id.clone()),
            "task_title": task.map(|task| task.title.clone()),
            "artifacts": artifacts,
            "evidence_json": evidence,
            "verification_json": verification,
        }
    });
    production_evidence_item_from_ledger_value(
        project_slug,
        source,
        "desktop_vision_action",
        software_action_id,
        item,
    )
}

fn production_evidence_bundle_from_ledger_value(
    project_slug: &str,
    source: &str,
    snapshot: &RuntimeSnapshot,
    include_incomplete: bool,
) -> Result<Value> {
    let mut bundle = ImportProductionEvidenceRequest {
        project_slug: Some(project_slug.to_string()),
        source: Some(source.to_string()),
        providers: Some(Vec::new()),
        software_actions: Some(Vec::new()),
        desktop_vision: Some(Vec::new()),
    };
    let mut ready_items = Vec::new();
    let mut incomplete_items = Vec::new();
    let mut incomplete_count = 0usize;

    for provider_request in &snapshot.provider_requests {
        production_evidence_collect_ledger_wrapper(
            production_evidence_item_from_provider_ledger(
                project_slug,
                source,
                snapshot,
                &provider_request.id,
            ),
            "provider_request",
            &provider_request.id,
            &mut bundle,
            &mut ready_items,
            &mut incomplete_items,
            &mut incomplete_count,
            include_incomplete,
        )?;
    }

    for software_action in &snapshot.software_actions {
        if is_desktop_recognition_action(software_action) {
            production_evidence_collect_ledger_wrapper(
                production_evidence_item_from_desktop_vision_ledger(
                    project_slug,
                    source,
                    snapshot,
                    &software_action.id,
                ),
                "desktop_vision_action",
                &software_action.id,
                &mut bundle,
                &mut ready_items,
                &mut incomplete_items,
                &mut incomplete_count,
                include_incomplete,
            )?;
        } else {
            production_evidence_collect_ledger_wrapper(
                production_evidence_item_from_software_ledger(
                    project_slug,
                    source,
                    snapshot,
                    &software_action.id,
                ),
                "software_action",
                &software_action.id,
                &mut bundle,
                &mut ready_items,
                &mut incomplete_items,
                &mut incomplete_count,
                include_incomplete,
            )?;
        }
    }

    let provider_count = bundle.providers.as_ref().map(Vec::len).unwrap_or_default();
    let software_count = bundle
        .software_actions
        .as_ref()
        .map(Vec::len)
        .unwrap_or_default();
    let desktop_count = bundle
        .desktop_vision
        .as_ref()
        .map(Vec::len)
        .unwrap_or_default();
    let ready_count = provider_count + software_count + desktop_count;
    let summary = json!({
        "providers": provider_count,
        "software_actions": software_count,
        "desktop_vision": desktop_count,
        "ready_items": ready_count,
        "incomplete_items": incomplete_count,
        "ledger_candidates": snapshot.provider_requests.len() + snapshot.software_actions.len(),
    });
    let artifact_files = production_evidence_artifact_file_report(&bundle);
    let validation = if ready_count == 0 {
        json!({
            "valid": false,
            "message": "no ready ledger evidence items",
            "summary": summary,
            "artifact_files": artifact_files,
        })
    } else {
        match validate_production_evidence_bundle(&bundle) {
            Ok((providers, software_actions, desktop_vision)) => json!({
                "valid": true,
                "message": "ledger-derived production evidence bundle passes schema validation",
                "summary": {
                    "providers": providers,
                    "software_actions": software_actions,
                    "desktop_vision": desktop_vision,
                },
                "artifact_files": artifact_files,
            }),
            Err(error) => json!({
                "valid": false,
                "message": error.to_string(),
                "summary": summary,
                "artifact_files": artifact_files,
            }),
        }
    };
    let ready_for_import = ready_count > 0
        && validation
            .get("valid")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        && validation
            .get("artifact_files")
            .and_then(|report| report.get("complete"))
            .and_then(Value::as_bool)
            .unwrap_or(false);

    Ok(json!({
        "kind": "pool_production_evidence_bundle_from_ledger",
        "version": 1,
        "project_slug": project_slug,
        "source": source,
        "ready_for_import": ready_for_import,
        "include_incomplete": include_incomplete,
        "summary": summary,
        "bundle": bundle,
        "items": ready_items,
        "incomplete_items": incomplete_items,
        "validation": validation,
        "commands": {
            "write_bundle": format!("pool-cli --project {project_slug} production-evidence-bundle-from-ledger <bundle.json>"),
            "validate": format!("pool-cli --project {project_slug} validate-production-evidence <bundle.json>"),
            "import": format!("pool-cli --project {project_slug} import-production-evidence <bundle.json>"),
            "closeout": format!("pool-cli --project {project_slug} closeout-production-evidence --import <bundle.json>"),
            "readiness": format!("pool-cli --project {project_slug} prd-readiness"),
        },
        "operator_checklist": [
            "Review incomplete_items before import; local mock and dry-run desktop traces stay out of the bundle.",
            "Only import after ready_for_import is true and validation.artifact_files.complete is true.",
            "Use closeout-production-evidence when merging this ledger bundle with external worker bundles."
        ],
    }))
}

fn production_evidence_collect_ledger_wrapper(
    wrapper: Result<Value>,
    ledger_kind: &str,
    ledger_id: &str,
    bundle: &mut ImportProductionEvidenceRequest,
    ready_items: &mut Vec<Value>,
    incomplete_items: &mut Vec<Value>,
    incomplete_count: &mut usize,
    include_incomplete: bool,
) -> Result<()> {
    let wrapper = match wrapper {
        Ok(wrapper) => wrapper,
        Err(error) => {
            *incomplete_count += 1;
            if include_incomplete {
                incomplete_items.push(json!({
                    "kind": "pool_production_evidence_item_from_ledger",
                    "version": 1,
                    "ready_for_import": false,
                    "ledger": {
                        "kind": ledger_kind,
                        "id": ledger_id,
                    },
                    "error": error.to_string(),
                }));
            }
            return Ok(());
        }
    };

    let ready = wrapper
        .get("ready_for_import")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if !ready {
        *incomplete_count += 1;
        if include_incomplete {
            incomplete_items.push(wrapper);
        }
        return Ok(());
    }

    let item = wrapper
        .get("item")
        .cloned()
        .context("ledger-derived production evidence wrapper missing item")?;
    match production_evidence_import_request_from_item_value(item) {
        Ok(import_request) => {
            append_production_evidence_import_request(bundle, import_request);
            ready_items.push(wrapper);
        }
        Err(error) => {
            *incomplete_count += 1;
            if include_incomplete {
                let mut failed = wrapper;
                failed["ready_for_import"] = json!(false);
                failed["error"] = json!(error.to_string());
                incomplete_items.push(failed);
            }
        }
    }

    Ok(())
}

fn production_evidence_import_request_from_item_value(
    item: Value,
) -> Result<ImportProductionEvidenceRequest> {
    serde_json::from_value::<SubmitProductionEvidenceItemRequest>(item)
        .context("parse ledger-derived production evidence item")?
        .into_import_request()
}

fn append_production_evidence_import_request(
    bundle: &mut ImportProductionEvidenceRequest,
    request: ImportProductionEvidenceRequest,
) {
    if let Some(providers) = request.providers {
        bundle
            .providers
            .get_or_insert_with(Vec::new)
            .extend(providers);
    }
    if let Some(software_actions) = request.software_actions {
        bundle
            .software_actions
            .get_or_insert_with(Vec::new)
            .extend(software_actions);
    }
    if let Some(desktop_vision) = request.desktop_vision {
        bundle
            .desktop_vision
            .get_or_insert_with(Vec::new)
            .extend(desktop_vision);
    }
}

fn production_evidence_item_from_ledger_value(
    project_slug: &str,
    source: &str,
    ledger_kind: &str,
    ledger_id: &str,
    item: Value,
) -> Result<Value> {
    let validation = production_evidence_item_validation_report(&item);
    let ready_for_import = validation
        .get("valid")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        && validation
            .get("artifact_files")
            .and_then(|report| report.get("complete"))
            .and_then(Value::as_bool)
            .unwrap_or(false)
        && validation
            .get("production_flags")
            .and_then(|report| report.get("complete"))
            .and_then(Value::as_bool)
            .unwrap_or(false);

    Ok(json!({
        "kind": "pool_production_evidence_item_from_ledger",
        "version": 1,
        "project_slug": project_slug,
        "source": source,
        "ready_for_import": ready_for_import,
        "ledger": {
            "kind": ledger_kind,
            "id": ledger_id,
        },
        "item": item,
        "validation": validation,
        "commands": {
            "submit": format!("pool-cli --project {project_slug} submit-production-evidence-item <item.json>"),
            "validate_bundle": format!("pool-cli --project {project_slug} validate-production-evidence <bundle.json>"),
            "readiness": format!("pool-cli --project {project_slug} prd-readiness"),
        },
        "operator_checklist": [
            "Confirm the ledger run came from a real external Provider, software control path, or external desktop vision controller, not a local mock.",
            "Replace any replace-with-real identifier before submit.",
            "Ensure Provider metadata, Provider artifacts, software artifacts, desktop trace, and desktop artifacts are existing local files.",
            "Submit only after validation.valid, artifact_files.complete, and production_flags.complete are true."
        ],
    }))
}

fn production_evidence_item_validation_report(item: &Value) -> Value {
    let production_flags = production_evidence_item_production_flag_report(item);
    let submit_request =
        match serde_json::from_value::<SubmitProductionEvidenceItemRequest>(item.clone()) {
            Ok(request) => request,
            Err(error) => {
                return json!({
                    "valid": false,
                    "message": error.to_string(),
                    "artifact_files": {
                        "complete": false,
                        "checked": 0,
                        "missing": [],
                        "checks": [],
                    },
                    "production_flags": production_flags,
                });
            }
        };
    let import_request = match submit_request.into_import_request() {
        Ok(request) => request,
        Err(error) => {
            return json!({
            "valid": false,
            "message": error.to_string(),
            "artifact_files": {
                "complete": false,
                "checked": 0,
                    "missing": [],
                    "checks": [],
                },
                "production_flags": production_flags,
            });
        }
    };
    let artifact_files = production_evidence_artifact_file_report(&import_request);
    match validate_production_evidence_bundle(&import_request) {
        Ok((providers, software_actions, desktop_vision)) => json!({
            "valid": true,
            "message": "item passes production evidence schema validation",
            "summary": {
                "providers": providers,
                "software_actions": software_actions,
                "desktop_vision": desktop_vision,
            },
            "artifact_files": artifact_files,
            "production_flags": production_flags,
        }),
        Err(error) => json!({
            "valid": false,
            "message": error.to_string(),
            "artifact_files": artifact_files,
            "production_flags": production_flags,
        }),
    }
}

fn production_evidence_item_production_flag_report(item: &Value) -> Value {
    match item.get("kind").and_then(Value::as_str).unwrap_or_default() {
        "provider" => {
            let evidence = item
                .pointer("/provider/evidence_json")
                .cloned()
                .unwrap_or_else(|| json!({}));
            let production_upstream = evidence
                .get("production_upstream")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let local_mock_gateway = evidence
                .get("local_mock_gateway")
                .and_then(Value::as_bool)
                .unwrap_or(true);
            let production_attestation = item
                .pointer("/provider/production_attestation")
                .and_then(Value::as_str)
                .or_else(|| {
                    evidence
                        .get("production_attestation")
                        .and_then(Value::as_str)
                })
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .is_some();
            json!({
                "complete": production_upstream && !local_mock_gateway && production_attestation,
                "required": [
                    "provider.evidence_json.production_upstream:true",
                    "provider.evidence_json.local_mock_gateway:false",
                    "provider.production_attestation or provider.evidence_json.production_attestation"
                ],
                "actual": {
                    "production_upstream": production_upstream,
                    "local_mock_gateway": local_mock_gateway,
                    "production_attestation": production_attestation,
                },
            })
        }
        "software_action" => {
            let evidence = item
                .pointer("/software_action/evidence_json")
                .cloned()
                .unwrap_or_else(|| json!({}));
            let production_software = evidence
                .get("production_software")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let local_mock_software = evidence
                .get("local_mock_software")
                .and_then(Value::as_bool)
                .unwrap_or(true);
            let production_attestation = item
                .pointer("/software_action/production_attestation")
                .and_then(Value::as_str)
                .or_else(|| {
                    evidence
                        .get("production_attestation")
                        .and_then(Value::as_str)
                })
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .is_some();
            json!({
                "complete": production_software && !local_mock_software && production_attestation,
                "required": [
                    "software_action.evidence_json.production_software:true",
                    "software_action.evidence_json.local_mock_software:false",
                    "software_action.production_attestation or software_action.evidence_json.production_attestation"
                ],
                "actual": {
                    "production_software": production_software,
                    "local_mock_software": local_mock_software,
                    "production_attestation": production_attestation,
                },
            })
        }
        "desktop_vision" => {
            let evidence = item
                .pointer("/desktop_vision/evidence_json")
                .cloned()
                .unwrap_or_else(|| json!({}));
            let verification = item
                .pointer("/desktop_vision/verification_json")
                .cloned()
                .unwrap_or_else(|| json!({}));
            let item_value = item
                .get("desktop_vision")
                .cloned()
                .unwrap_or_else(|| json!({}));
            let external_visual_model =
                production_evidence_json_external_visual_model(Some(&evidence))
                    || production_evidence_json_external_visual_model(Some(&verification))
                    || item_value
                        .get("visual_model")
                        .and_then(Value::as_str)
                        .is_some_and(|value| value.eq_ignore_ascii_case("external"));
            let local_trace_smoke = evidence
                .get("local_trace_smoke")
                .and_then(Value::as_bool)
                .unwrap_or(!external_visual_model);
            let production_attestation = item
                .pointer("/desktop_vision/production_attestation")
                .and_then(Value::as_str)
                .or_else(|| {
                    evidence
                        .get("production_attestation")
                        .and_then(Value::as_str)
                })
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .is_some();
            json!({
                "complete": external_visual_model && !local_trace_smoke && production_attestation,
                "required": [
                    "desktop_vision.visual_model:external or evidence_json.external_visual_model:true",
                    "desktop_vision.evidence_json.local_trace_smoke:false",
                    "desktop_vision.production_attestation or desktop_vision.evidence_json.production_attestation"
                ],
                "actual": {
                    "external_visual_model": external_visual_model,
                    "local_trace_smoke": local_trace_smoke,
                    "production_attestation": production_attestation,
                },
            })
        }
        _ => json!({
            "complete": false,
            "required": ["kind:provider, kind:software_action, or kind:desktop_vision"],
            "actual": {},
        }),
    }
}

fn desktop_vision_ledger_artifacts(verification: &Value) -> Vec<String> {
    let mut artifacts = json_string_array_at(verification, "artifacts");
    if let Some(trace_path) = string_at_pointer(verification, "/screen_trace_path")
        .or_else(|| string_at_pointer(verification, "/trace_path"))
        .or_else(|| string_at_pointer(verification, "/controller_result/screen_trace_path"))
        .or_else(|| string_at_pointer(verification, "/controller_result/trace_path"))
    {
        artifacts.push(trace_path);
    }
    artifacts.sort();
    artifacts.dedup();
    artifacts
}

fn provider_ledger_artifacts(
    provider_request: &ProviderRequestSnapshot,
    task: Option<&TaskSnapshot>,
    assets: &[AssetSnapshot],
) -> Vec<String> {
    provider_request
        .response
        .as_ref()
        .map(|response| {
            response
                .get("assets")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|asset| string_at_pointer(asset, "/local_path"))
                .collect::<Vec<_>>()
        })
        .filter(|paths| !paths.is_empty())
        .or_else(|| {
            provider_request
                .response
                .as_ref()
                .map(|response| string_array_at_pointer(response, "/artifacts"))
                .filter(|paths| !paths.is_empty())
        })
        .or_else(|| {
            provider_request
                .response
                .as_ref()
                .map(|response| string_array_at_pointer(response, "/job/expected_outputs"))
                .filter(|paths| !paths.is_empty())
        })
        .or_else(|| {
            task.and_then(|task| {
                task.node_id.as_ref().map(|node_id| {
                    assets
                        .iter()
                        .filter(|asset| asset.source_node_id.as_deref() == Some(node_id))
                        .map(|asset| asset.local_path.clone())
                        .collect::<Vec<_>>()
                })
            })
            .filter(|paths| !paths.is_empty())
        })
        .unwrap_or_default()
}

fn string_at_pointer(value: &Value, pointer: &str) -> Option<String> {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string)
}

fn string_array_at_pointer(value: &Value, pointer: &str) -> Vec<String> {
    value
        .pointer(pointer)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string)
        .collect()
}

fn production_template_provider_artifact_plan(
    project_slug: &str,
    output_root: Option<&str>,
    provider_ids: &[String],
) -> Vec<Value> {
    provider_ids
        .iter()
        .map(|provider_id| {
            json!({
                "provider_id": provider_id,
                "artifact_path": production_template_path(
                    output_root,
                    &format!(
                        "worlds/{project_slug}/output/production/{provider_id}/{}",
                        production_template_provider_artifact_name(provider_id)
                    ),
                ),
                "metadata_path": production_template_path(
                    output_root,
                    &format!("worlds/{project_slug}/output/production/{provider_id}/request-metadata.json"),
                ),
                "family": production_template_provider_family(provider_id),
            })
        })
        .collect()
}

fn production_template_provider_family(provider_id: &str) -> &'static str {
    match provider_id {
        "openai-image-2" => "ai_image",
        "midjourney" | "nano-banana-pro" | "suno" => "ai_media",
        _ => "3dgs",
    }
}

fn production_template_provider_artifact_name(provider_id: &str) -> &'static str {
    match provider_id {
        "midjourney" => "1-midjourney.png",
        "openai-image-2" => "1-openai-image.png",
        "nano-banana-pro" => "1-nano.png",
        "suno" => "1-cue.mp3",
        "worldlabs-marble" => "1-world.glb",
        "tripo-splat" => "1-object.glb",
        "sam-3d" => "1-mask-object.glb",
        "spark-3dgs" => "1-scene.glb",
        "qunhe-3d" => "1-layout.glb",
        _ => "1-provider-artifact.bin",
    }
}

fn production_template_software_profile(
    adapter_id: &str,
    project_slug: &str,
    output_root: Option<&str>,
) -> (&'static str, &'static str, &'static str, String) {
    match adapter_id {
        "unreal" => (
            "CreateScene",
            "ApiMcp",
            "api_mcp",
            production_template_path(
                output_root,
                &format!("worlds/{project_slug}/output/production/unreal/1-level.umap"),
            ),
        ),
        "unity" => (
            "ExportBuild",
            "ApiMcp",
            "api_mcp",
            production_template_path(
                output_root,
                &format!("worlds/{project_slug}/output/production/unity/1-build.zip"),
            ),
        ),
        "blender" => (
            "ExecuteCli",
            "ApiMcp",
            "api_mcp",
            production_template_path(
                output_root,
                &format!("worlds/{project_slug}/output/production/blender/1-cleanup.blend"),
            ),
        ),
        "comfyui" => (
            "ExecuteCli",
            "ApiMcp",
            "api_mcp",
            production_template_path(
                output_root,
                &format!("worlds/{project_slug}/output/production/comfyui/1-image.png"),
            ),
        ),
        "touchdesigner" => (
            "RunViewport",
            "DesktopRecognition",
            "desktop_recognition",
            production_template_path(
                output_root,
                &format!("worlds/{project_slug}/output/production/touchdesigner/1-performance.toe"),
            ),
        ),
        "madmapper" => (
            "RunViewport",
            "DesktopRecognition",
            "desktop_recognition",
            production_template_path(
                output_root,
                &format!("worlds/{project_slug}/output/production/madmapper/1-cues.mad"),
            ),
        ),
        "resolve" => (
            "Transcode",
            "ApiMcp",
            "api_mcp",
            production_template_path(
                output_root,
                &format!("worlds/{project_slug}/output/production/resolve/1-master.mov"),
            ),
        ),
        "nuke" => (
            "Render",
            "ApiMcp",
            "api_mcp",
            production_template_path(
                output_root,
                &format!("worlds/{project_slug}/output/production/nuke/1-comp.exr"),
            ),
        ),
        "motion-db" => (
            "ImportAsset",
            "ApiMcp",
            "api_mcp",
            production_template_path(
                output_root,
                &format!("worlds/{project_slug}/output/production/motion-db/1-take.fbx"),
            ),
        ),
        "editing-suite" => (
            "Transcode",
            "ApiMcp",
            "api_mcp",
            production_template_path(
                output_root,
                &format!("worlds/{project_slug}/output/production/editing-suite/1-delivery.mp4"),
            ),
        ),
        "hermes" => (
            "CreateScene",
            "ApiMcp",
            "api_mcp",
            production_template_path(
                output_root,
                &format!("worlds/{project_slug}/output/production/hermes/1-session.json"),
            ),
        ),
        _ => (
            "ExecuteCli",
            "SkillsCli",
            "skills_cli",
            production_template_path(
                output_root,
                &format!("worlds/{project_slug}/output/production/{adapter_id}/1-artifact.bin"),
            ),
        ),
    }
}

fn production_template_path(output_root: Option<&str>, relative_path: &str) -> String {
    let Some(root) = output_root.map(str::trim).filter(|root| !root.is_empty()) else {
        return relative_path.to_string();
    };
    Path::new(root)
        .join(relative_path)
        .to_string_lossy()
        .into_owned()
}

fn production_evidence_coverage(request: &ImportProductionEvidenceRequest) -> Value {
    let provider_ids = request
        .providers
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .map(|item| canonical_provider_id(&item.provider_id.trim().to_ascii_lowercase()))
        .collect::<BTreeSet<_>>();
    let software_ids = request
        .software_actions
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .map(|item| canonical_production_software_adapter_id(&item.adapter_id))
        .collect::<BTreeSet<_>>();
    let desktop_vision_count = request.desktop_vision.as_ref().map(Vec::len).unwrap_or(0);
    let external_visual_model_count = request
        .desktop_vision
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .filter(|item| {
            desktop_vision_external_visual_model(item)
                && production_attestation_present(
                    item.production_attestation.as_deref(),
                    item.evidence_json.as_ref(),
                )
        })
        .count();
    let external_visual_model_ready = external_visual_model_count > 0;

    let provider_missing =
        missing_required_evidence(REQUIRED_PRODUCTION_PROVIDER_EVIDENCE, &provider_ids);
    let software_missing =
        missing_required_evidence(REQUIRED_PRODUCTION_SOFTWARE_EVIDENCE, &software_ids);
    let provider_complete = provider_missing.is_empty();
    let software_complete = software_missing.is_empty();
    let complete = provider_complete && software_complete && external_visual_model_ready;

    json!({
        "complete": complete,
        "would_satisfy_prd_production_evidence": complete,
        "providers": {
            "required": REQUIRED_PRODUCTION_PROVIDER_EVIDENCE,
            "provided": provider_ids.iter().cloned().collect::<Vec<_>>(),
            "covered": REQUIRED_PRODUCTION_PROVIDER_EVIDENCE.len() - provider_missing.len(),
            "missing": provider_missing,
            "complete": provider_complete,
        },
        "software_actions": {
            "required": REQUIRED_PRODUCTION_SOFTWARE_EVIDENCE,
            "provided": software_ids.iter().cloned().collect::<Vec<_>>(),
            "covered": REQUIRED_PRODUCTION_SOFTWARE_EVIDENCE.len() - software_missing.len(),
            "missing": software_missing,
            "complete": software_complete,
        },
        "desktop_vision": {
            "required": ["external_visual_model", "production_attestation"],
            "provided": desktop_vision_count,
            "external_visual_model_count": external_visual_model_count,
            "external_visual_model": external_visual_model_ready,
            "missing": if external_visual_model_ready {
                Vec::<String>::new()
            } else {
                vec!["external_visual_model".to_string(), "production_attestation".to_string()]
            },
            "complete": external_visual_model_ready,
        }
    })
}

fn missing_required_evidence(required: &[&str], provided: &BTreeSet<String>) -> Vec<String> {
    required
        .iter()
        .filter(|required_id| !provided.contains(&(*required_id).to_string()))
        .map(|required_id| (*required_id).to_string())
        .collect()
}

fn canonical_production_software_adapter_id(adapter_id: &str) -> String {
    match adapter_id.trim().to_ascii_lowercase().as_str() {
        "davinci" | "davinci-resolve" | "da-vinci-resolve" => "resolve".to_string(),
        "touch-designer" => "touchdesigner".to_string(),
        "mad-mapper" => "madmapper".to_string(),
        "mocap" | "motion-capture" | "motion-database" => "motion-db".to_string(),
        "editing" | "editor" | "cutting" => "editing-suite".to_string(),
        value => value.to_string(),
    }
}

fn desktop_vision_external_visual_model(item: &DesktopVisionProductionEvidenceItem) -> bool {
    item.visual_model
        .as_deref()
        .map(|value| value.trim().to_ascii_lowercase())
        .is_some_and(|value| {
            matches!(
                value.as_str(),
                "external"
                    | "external_visual_model"
                    | "external-visual-model"
                    | "external_vision"
                    | "external-vision"
            ) || value.starts_with("external:")
        })
        || production_evidence_json_external_visual_model(item.evidence_json.as_ref())
        || production_evidence_json_external_visual_model(item.verification_json.as_ref())
}

fn production_evidence_json_external_visual_model(value: Option<&Value>) -> bool {
    value.is_some_and(|value| {
        value
            .get("external_visual_model")
            .and_then(Value::as_bool)
            .unwrap_or(false)
            || value
                .pointer("/controller_result/external_visual_model")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            || value
                .pointer("/evidence/external_visual_model")
                .and_then(Value::as_bool)
                .unwrap_or(false)
    })
}

fn production_attestation_present(top_level: Option<&str>, evidence_json: Option<&Value>) -> bool {
    top_level
        .or_else(|| {
            evidence_json
                .and_then(|evidence| evidence.get("production_attestation"))
                .and_then(Value::as_str)
        })
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some()
}

fn provider_production_evidence_validation_rows(
    request: &ImportProductionEvidenceRequest,
) -> Vec<Value> {
    request
        .providers
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .map(|item| {
            let input_provider_id = item.provider_id.trim();
            let provider_id = canonical_provider_id(&input_provider_id.to_ascii_lowercase());
            json!({
                "provider_id": provider_id,
                "input_provider_id": input_provider_id,
                "external_job_id": item.external_job_id.trim(),
                "family": item.family.as_deref(),
                "endpoint": item.endpoint.as_deref(),
                "production_attestation": item.production_attestation.as_deref().or_else(|| {
                    item.evidence_json
                        .as_ref()
                        .and_then(|evidence| evidence.get("production_attestation"))
                        .and_then(Value::as_str)
                }),
                "artifacts": item.artifacts.as_ref().map(Vec::len).unwrap_or(0),
                "metadata_path": item.metadata_path.as_deref(),
                "production_upstream": true,
                "writes_on_validate": 0,
            })
        })
        .collect()
}

fn software_production_evidence_validation_rows(
    request: &ImportProductionEvidenceRequest,
) -> Vec<Value> {
    request
        .software_actions
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .map(|item| {
            let input_adapter_id = item.adapter_id.trim();
            let adapter_id = canonical_production_software_adapter_id(input_adapter_id);
            json!({
                "adapter_id": adapter_id,
                "input_adapter_id": input_adapter_id,
                "external_action_id": item.external_action_id.trim(),
                "production_attestation": item.production_attestation.as_deref().or_else(|| {
                    item.evidence_json
                        .as_ref()
                        .and_then(|evidence| evidence.get("production_attestation"))
                        .and_then(Value::as_str)
                }),
                "action_kind": item.action_kind.as_ref().map(|kind| format!("{kind:?}")),
                "priority": item.priority.as_ref().map(|priority| format!("{priority:?}")),
                "control_profile": item.control_profile.as_deref(),
                "artifacts": item.artifacts.as_ref().map(Vec::len).unwrap_or(0),
                "has_verification_json": item.verification_json.is_some(),
                "production_software": true,
                "writes_on_validate": 0,
            })
        })
        .collect()
}

fn desktop_vision_production_evidence_validation_rows(
    request: &ImportProductionEvidenceRequest,
) -> Vec<Value> {
    request
        .desktop_vision
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .map(|item| {
            json!({
                "adapter_id": item.adapter_id.as_deref().unwrap_or("touchdesigner"),
                "external_action_id": item.external_action_id.trim(),
                "controller_id": item.controller_id.trim(),
                "production_attestation": item.production_attestation.as_deref().or_else(|| {
                    item.evidence_json
                        .as_ref()
                        .and_then(|evidence| evidence.get("production_attestation"))
                        .and_then(Value::as_str)
                }),
                "trace_path": item.trace_path.trim(),
                "visual_model": item.visual_model.as_deref().unwrap_or("unspecified"),
                "artifacts": item.artifacts.as_ref().map(Vec::len).unwrap_or(0),
                "external_visual_model": desktop_vision_external_visual_model(item),
                "writes_on_validate": 0,
            })
        })
        .collect()
}

fn production_evidence_artifact_file_report(request: &ImportProductionEvidenceRequest) -> Value {
    let mut checks = Vec::new();
    let mut missing = Vec::new();

    for (index, item) in request
        .providers
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .enumerate()
    {
        for artifact in item.artifacts.as_deref().unwrap_or(&[]) {
            let path = artifact.trim();
            if path.is_empty() {
                continue;
            }
            let exists = Path::new(path).exists();
            if !exists {
                missing.push(path.to_string());
            }
            checks.push(json!({
                "kind": "provider_artifact",
                "field": production_evidence_field("providers", index, "artifacts"),
                "path": path,
                "exists": exists,
            }));
        }
        if let Some(metadata_path) = item.metadata_path.as_deref() {
            let path = metadata_path.trim();
            if !path.is_empty() {
                let exists = Path::new(path).exists();
                if !exists {
                    missing.push(path.to_string());
                }
                checks.push(json!({
                    "kind": "provider_metadata",
                    "field": production_evidence_field("providers", index, "metadata_path"),
                    "path": path,
                    "exists": exists,
                }));
            }
        }
    }

    for (index, item) in request
        .desktop_vision
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .enumerate()
    {
        let trace_path = item.trace_path.trim();
        if !trace_path.is_empty() {
            let exists = Path::new(trace_path).exists();
            if !exists {
                missing.push(trace_path.to_string());
            }
            checks.push(json!({
                "kind": "desktop_vision_trace",
                "field": production_evidence_field("desktop_vision", index, "trace_path"),
                "path": trace_path,
                "exists": exists,
            }));
        }
        for artifact in item.artifacts.as_deref().unwrap_or(&[]) {
            let path = artifact.trim();
            if path.is_empty() {
                continue;
            }
            let exists = Path::new(path).exists();
            if !exists {
                missing.push(path.to_string());
            }
            checks.push(json!({
                "kind": "desktop_vision_artifact",
                "field": production_evidence_field("desktop_vision", index, "artifacts"),
                "path": path,
                "exists": exists,
            }));
        }
    }

    for (index, item) in request
        .software_actions
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .enumerate()
    {
        let mut artifact_count = 0usize;
        for artifact in item.artifacts.as_deref().unwrap_or(&[]) {
            let path = artifact.trim();
            if path.is_empty() {
                continue;
            }
            artifact_count += 1;
            let exists = Path::new(path).exists();
            if !exists {
                missing.push(path.to_string());
            }
            checks.push(json!({
                "kind": "software_action_artifact",
                "field": production_evidence_field("software_actions", index, "artifacts"),
                "path": path,
                "exists": exists,
            }));
        }
        if artifact_count == 0 {
            let field = production_evidence_field("software_actions", index, "artifacts");
            missing.push(field.clone());
            checks.push(json!({
                "kind": "software_action_artifact",
                "field": field,
                "path": "",
                "exists": false,
            }));
        }
    }

    missing.sort();
    missing.dedup();
    json!({
        "complete": missing.is_empty(),
        "checked": checks.len(),
        "missing": missing,
        "checks": checks,
    })
}

fn required_non_empty(value: String, field: &str) -> Result<String> {
    let value = value.trim().to_string();
    if value.is_empty() {
        bail!("{field} must not be empty");
    }
    Ok(value)
}

fn required_production_identifier(value: String, field: &str) -> Result<String> {
    let value = required_non_empty(value, field)?;
    let lower = value.to_ascii_lowercase();
    let markers = [
        "replace-with",
        "placeholder",
        "todo",
        "dummy",
        "fake",
        "sample-",
        "example-",
        "template-",
        "web-prod",
    ];
    if markers.iter().any(|marker| lower.contains(marker)) {
        bail!("{field} must be a real production identifier, not a template placeholder");
    }
    Ok(value)
}

fn required_provider_production_attestation(
    top_level: Option<&str>,
    evidence_json: Option<&Value>,
    field: &str,
) -> Result<String> {
    required_production_attestation(
        top_level,
        evidence_json,
        field,
        "a real upstream provider worker or SDK run",
    )
}

fn required_production_attestation(
    top_level: Option<&str>,
    evidence_json: Option<&Value>,
    field: &str,
    subject: &str,
) -> Result<String> {
    let value = top_level
        .or_else(|| {
            evidence_json
                .and_then(|evidence| evidence.get("production_attestation"))
                .and_then(Value::as_str)
        })
        .unwrap_or_default()
        .trim()
        .to_string();
    let value = required_non_empty(value, field)?;
    let lower = value.to_ascii_lowercase();
    let markers = [
        "replace-with",
        "placeholder",
        "todo",
        "dummy",
        "fake",
        "mock",
        "sample-",
        "example-",
        "template-",
    ];
    if markers.iter().any(|marker| lower.contains(marker)) {
        bail!("{field} must identify {subject}, not a template placeholder");
    }
    Ok(value)
}

fn required_non_empty_vec(value: Option<Vec<String>>, field: &str) -> Result<Vec<String>> {
    let values = value
        .unwrap_or_default()
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    if values.is_empty() {
        bail!("{field} must contain at least one value");
    }
    Ok(values)
}

fn required_local_artifact_vec(value: Option<Vec<String>>, field: &str) -> Result<Vec<String>> {
    let values = required_non_empty_vec(value, field)?;
    for value in &values {
        required_local_artifact_path(value.clone(), field)?;
    }
    Ok(values)
}

fn required_local_artifact_path(value: String, field: &str) -> Result<String> {
    let value = required_non_empty(value, field)?;
    let lower = value.to_ascii_lowercase();
    if lower.contains("://") {
        bail!(
            "{field} must be a local file path; provider URLs and runtime URIs are provenance only"
        );
    }
    Ok(value)
}

fn software_control_profile_name(priority: &ControlPriority) -> String {
    match priority {
        ControlPriority::ApiMcp => "api_mcp",
        ControlPriority::SkillsCli => "skills_cli",
        ControlPriority::DesktopRecognition => "desktop_recognition",
        ControlPriority::HumanTakeover => "human_takeover",
    }
    .to_string()
}

fn merge_json_object(target: &mut Value, source: Value) {
    if !target.is_object() {
        *target = json!({});
    }
    if let (Some(target), Value::Object(source)) = (target.as_object_mut(), source) {
        for (key, value) in source {
            target.insert(key, value);
        }
    }
}

fn merge_payload_child_object(target: &mut Value, key: &str, source: Value) {
    if !source.is_object() {
        return;
    }
    if !target.is_object() {
        *target = json!({});
    }
    if let Some(object) = target.as_object_mut() {
        let mut child = object.remove(key).unwrap_or_else(|| json!({}));
        merge_json_object(&mut child, source);
        object.insert(key.to_string(), child);
    }
}

fn json_object_insert(target: &mut Value, key: &str, value: Value) {
    if !target.is_object() {
        *target = json!({});
    }
    if let Some(object) = target.as_object_mut() {
        object.insert(key.to_string(), value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        build_default_content_burst_plan, spawn_provider_gateway_mock, NodeType, RuntimeRepository,
    };
    use std::io::Write;
    use std::thread;
    use uuid::Uuid;

    #[test]
    fn returns_snapshot_and_mcp_resources_from_sqlite() {
        let db_path = temp_db_path("runtime-http-snapshot");
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        let plan = build_default_content_burst_plan("demo", "Pool HTTP demo");
        repository.persist_plan(&plan).unwrap();
        drop(repository);

        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));

        let snapshot = server.handle_path("/api/snapshot").unwrap();
        assert_eq!(snapshot.status_code, 200);
        assert!(snapshot.body.contains("\"project_filter\": \"demo\""));

        let mcp = server
            .handle_path("/api/mcp?uri=pool%3A%2F%2Ftasks")
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&mcp.body).unwrap();
        assert_eq!(
            value["tasks"].as_array().unwrap().len(),
            plan.workflow.nodes.len()
        );
        assert_eq!(value["approval_gates"].as_array().unwrap().len(), 1);

        let workflow_index = server.handle_path("/api/workflow-context").unwrap();
        let workflow_index_value: serde_json::Value =
            serde_json::from_str(&workflow_index.body).unwrap();
        assert_eq!(workflow_index.status_code, 200);
        assert_eq!(workflow_index_value["summary"]["workflows"], 1);
        assert_eq!(
            workflow_index_value["workflows"][0]["workflow_id"],
            plan.workflow.id
        );

        let workflow_context = server
            .handle_path(&format!(
                "/api/workflow-context?workflow_id={}",
                plan.workflow.id
            ))
            .unwrap();
        let workflow_context_value: serde_json::Value =
            serde_json::from_str(&workflow_context.body).unwrap();
        assert_eq!(workflow_context.status_code, 200);
        assert_eq!(workflow_context_value["workflow_id"], plan.workflow.id);
        assert_eq!(
            workflow_context_value["summary"]["tasks"],
            plan.workflow.nodes.len()
        );

        let workflow_uri = format!("pool://workflow/{}", plan.workflow.id);
        let workflow_mcp = server
            .handle_path(&format!(
                "/api/mcp?uri={}",
                percent_encode_query_value(&workflow_uri)
            ))
            .unwrap();
        let workflow_value: serde_json::Value = serde_json::from_str(&workflow_mcp.body).unwrap();
        assert_eq!(workflow_mcp.status_code, 200);
        assert_eq!(workflow_value["workflow_id"], plan.workflow.id);
        assert_eq!(
            workflow_value["summary"]["tasks"],
            plan.workflow.nodes.len()
        );
        assert_eq!(
            workflow_value["graph"]["summary"]["edges"],
            plan.workflow.connections.len()
        );

        let missing_workflow = server
            .handle_path("/api/workflow-context?workflow_id=missing-workflow")
            .unwrap();
        let missing_workflow_value: serde_json::Value =
            serde_json::from_str(&missing_workflow.body).unwrap();
        assert_eq!(missing_workflow.status_code, 404);
        assert_eq!(
            missing_workflow_value["error"],
            "workflow_context_not_found"
        );
    }

    #[test]
    fn get_snapshot_and_health_accept_project_query_override() {
        let db_path = temp_db_path("runtime-http-project-query");
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        let demo_plan = build_default_content_burst_plan("demo", "Pool demo");
        let alt_plan = build_default_content_burst_plan("alt", "Pool alt");
        repository.persist_plan(&demo_plan).unwrap();
        repository.persist_plan(&alt_plan).unwrap();
        drop(repository);

        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let alt_snapshot = server.handle_path("/api/snapshot?project=alt").unwrap();
        let alt_value: serde_json::Value = serde_json::from_str(&alt_snapshot.body).unwrap();

        assert_eq!(alt_snapshot.status_code, 200);
        assert_eq!(alt_value["project_filter"], "alt");
        assert_eq!(alt_value["projects"].as_array().unwrap().len(), 1);
        assert_eq!(alt_value["projects"][0]["slug"], "alt");

        let all_health = server.handle_path("/api/health?project=*").unwrap();
        let all_value: serde_json::Value = serde_json::from_str(&all_health.body).unwrap();

        assert_eq!(all_health.status_code, 200);
        assert!(all_value["project_filter"].is_null());
        assert_eq!(all_value["stats"]["projects"], 2);
    }

    #[test]
    fn post_prd_completion_package_writes_proof_files() {
        let db_path = temp_db_path("runtime-http-prd-completion-package");
        let output_dir = temp_control_dir("runtime-http-prd-completion-package-output");
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        repository
            .persist_plan(&build_default_content_burst_plan(
                "demo",
                "Pool PRD completion package test",
            ))
            .unwrap();
        drop(repository);

        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let body = json!({
            "project_slug": "demo",
            "node_id": "agent",
            "title": "PRD completion proof package",
            "output_dir": output_dir.to_string_lossy(),
            "source": "server-test",
            "include_snapshot": true
        });
        let response = server
            .handle_request_with_body("POST", "/api/prd-completion-package", &body.to_string())
            .unwrap();
        let value: Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 201);
        assert_eq!(value["kind"], "pool_prd_completion_package");
        assert_eq!(value["report"]["ready_for_completion"], false);
        assert!(PathBuf::from(value["report"]["readiness_path"].as_str().unwrap()).exists());
        assert!(PathBuf::from(value["report"]["completion_gate_path"].as_str().unwrap()).exists());
        assert!(PathBuf::from(
            value["report"]["production_evidence_requirements_path"]
                .as_str()
                .unwrap()
        )
        .exists());
        assert!(PathBuf::from(value["report"]["snapshot_path"].as_str().unwrap()).exists());
        assert!(value["assets"].as_array().unwrap().len() >= 5);
        assert_eq!(value["task"]["status"], "Succeeded");

        let _ = fs::remove_dir_all(output_dir);
    }

    #[test]
    fn returns_resource_list_and_health() {
        let db_path = temp_db_path("runtime-http-health");
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        let plan = build_default_content_burst_plan("demo", "Pool HTTP demo");
        repository.persist_plan(&plan).unwrap();
        drop(repository);

        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));

        let health = server.handle_path("/api/health").unwrap();
        assert_eq!(health.status_code, 200);
        assert!(health.body.contains("\"status\": \"ready\""));

        let resources = server.handle_path("/api/resources").unwrap();
        assert_eq!(resources.status_code, 200);
        assert!(resources.body.contains("pool://snapshot"));

        let graph = server.handle_path("/api/runtime-graph").unwrap();
        let graph_value: serde_json::Value = serde_json::from_str(&graph.body).unwrap();
        assert_eq!(graph.status_code, 200);
        assert_eq!(graph_value["summary"]["nodes"], plan.workflow.nodes.len());
        assert!(graph_value["workflows"][0]["edges"]
            .as_array()
            .unwrap()
            .iter()
            .any(|edge| edge["label"] == "cost gate + localize"));

        let three_dgs_node = plan
            .workflow
            .nodes
            .values()
            .find(|node| node.node_type == NodeType::ThreeDgs)
            .unwrap();
        let node_index = server.handle_path("/api/node-context").unwrap();
        let node_index_value: serde_json::Value = serde_json::from_str(&node_index.body).unwrap();
        assert_eq!(node_index.status_code, 200);
        assert_eq!(
            node_index_value["summary"]["nodes"],
            plan.workflow.nodes.len()
        );

        let node_context = server
            .handle_path(&format!("/api/node-context?node_id={}", three_dgs_node.id))
            .unwrap();
        let node_context_value: serde_json::Value =
            serde_json::from_str(&node_context.body).unwrap();
        assert_eq!(node_context.status_code, 200);
        assert_eq!(node_context_value["node"]["task_type"], "3dgs");
        assert_eq!(node_context_value["summary"]["tasks"], 1);
        assert_eq!(node_context_value["summary"]["blocked_by_approval"], true);

        let prd_readiness = server
            .handle_path("/api/prd-readiness?project=demo")
            .unwrap();
        let prd_readiness_value: serde_json::Value =
            serde_json::from_str(&prd_readiness.body).unwrap();
        assert_eq!(prd_readiness.status_code, 200);
        assert_eq!(prd_readiness_value["kind"], "pool_prd_readiness");
        assert_eq!(prd_readiness_value["summary"]["total"], 10);
        assert!(prd_readiness_value["requirements"]
            .as_array()
            .unwrap()
            .iter()
            .any(|requirement| requirement["id"] == "external_software_control"));

        let prd_completion_gate = server
            .handle_path("/api/prd-completion-gate?project=demo")
            .unwrap();
        let prd_completion_gate_value: serde_json::Value =
            serde_json::from_str(&prd_completion_gate.body).unwrap();
        assert_eq!(prd_completion_gate.status_code, 200);
        assert_eq!(
            prd_completion_gate_value["kind"],
            "pool_prd_completion_gate"
        );
        assert_eq!(
            prd_completion_gate_value["completion_gate"]["ready_for_completion"],
            false
        );

        let prd_completion_gate_required = server
            .handle_path("/api/prd-completion-gate?project=demo&require_complete=true")
            .unwrap();
        let prd_completion_gate_required_value: serde_json::Value =
            serde_json::from_str(&prd_completion_gate_required.body).unwrap();
        assert_eq!(prd_completion_gate_required.status_code, 428);
        assert_eq!(
            prd_completion_gate_required_value["error"],
            "prd_completion_gate_incomplete"
        );

        let prd_completion_gate_resource = server
            .handle_path("/api/mcp?uri=pool%3A%2F%2Fprd-completion-gate&project=demo")
            .unwrap();
        let prd_completion_gate_resource_value: serde_json::Value =
            serde_json::from_str(&prd_completion_gate_resource.body).unwrap();
        assert_eq!(prd_completion_gate_resource.status_code, 200);
        assert_eq!(
            prd_completion_gate_resource_value["completion_gate"]["status"],
            "incomplete"
        );

        let production_requirements = server
            .handle_path("/api/production-evidence/requirements?project=demo")
            .unwrap();
        let production_requirements_value: serde_json::Value =
            serde_json::from_str(&production_requirements.body).unwrap();
        assert_eq!(production_requirements.status_code, 200);
        assert_eq!(
            production_requirements_value["kind"],
            "pool_production_evidence_requirements"
        );
        assert!(production_requirements_value["required_software"]
            .as_array()
            .unwrap()
            .iter()
            .any(|software| software["adapter_id"] == "unreal"
                && software["preferred_control_profile"] == "api_mcp"));
        assert!(
            production_requirements_value["evidence_tasks"]["summary"]["total"]
                .as_u64()
                .unwrap()
                >= 1
        );
        assert!(production_requirements_value["evidence_tasks"]["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|task| task["kind"] == "provider_production_upstream"
                && task["target_id"] == "midjourney"
                && task["bundle_path"] == "providers[]"));
        assert_eq!(
            production_requirements_value["commands"]["closeout"],
            "pool-cli --project <slug> closeout-production-evidence --output <merged-bundle.json> <bundle-a.json> <bundle-b.json>..."
        );

        let production_tasks = server
            .handle_path("/api/production-evidence/tasks?project=demo")
            .unwrap();
        let production_tasks_value: serde_json::Value =
            serde_json::from_str(&production_tasks.body).unwrap();
        assert_eq!(production_tasks.status_code, 200);
        assert_eq!(
            production_tasks_value["kind"],
            "pool_production_evidence_tasks"
        );
        assert!(
            production_tasks_value["summary"]["provider_tasks"]
                .as_u64()
                .unwrap()
                >= 1
        );
        assert_eq!(
            production_tasks_value["commands"]["submit_item"],
            "pool-cli --project demo submit-production-evidence-item <item.json>"
        );
        assert!(production_tasks_value["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .any(
                |task| task["id"] == "provider:midjourney:production_upstream"
                    && task["mcp"]["submit_tool"] == "pool_submit_production_evidence_item"
            ));

        let claim_root = temp_control_dir("runtime-http-production-evidence-claim");
        let claim_body = json!({
            "project_slug": "demo",
            "task_id": "provider:midjourney:production_upstream",
            "assignee": "provider-worker-1",
            "role": "provider_worker",
            "output_root": claim_root.to_string_lossy(),
            "source": "server-test-claim"
        });
        let claim_response = server
            .handle_request_with_body(
                "POST",
                "/api/production-evidence/tasks/claim",
                &claim_body.to_string(),
            )
            .unwrap();
        let claim_value: serde_json::Value = serde_json::from_str(&claim_response.body).unwrap();
        assert_eq!(claim_response.status_code, 201);
        assert_eq!(claim_value["kind"], "pool_production_evidence_task_claim");
        assert_eq!(
            claim_value["task_id"],
            "provider:midjourney:production_upstream"
        );
        assert_eq!(claim_value["runtime_task"]["status"], "Running");
        assert_eq!(claim_value["runtime_task"]["provider_id"], "midjourney");
        assert_eq!(
            claim_value["claim"]["item_template"]["item"]["provider"]["provider_id"],
            "midjourney"
        );
        assert!(PathBuf::from(claim_value["claim_path"].as_str().unwrap()).exists());
        assert!(claim_value["snapshot"]["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|task| task["id"] == claim_value["runtime_task"]["id"]));
        let _ = fs::remove_dir_all(claim_root);

        let production_run_plan = server
            .handle_path(
                "/api/production-evidence/run-plan?project=demo&output_root=target/prod&source=agent-run-plan",
            )
            .unwrap();
        let production_run_plan_value: serde_json::Value =
            serde_json::from_str(&production_run_plan.body).unwrap();
        assert_eq!(production_run_plan.status_code, 200);
        assert_eq!(
            production_run_plan_value["kind"],
            "pool_production_evidence_run_plan"
        );
        assert_eq!(production_run_plan_value["project_slug"], "demo");
        assert_eq!(production_run_plan_value["source"], "agent-run-plan");
        assert_eq!(
            production_run_plan_value["status"],
            "needs_real_production_evidence"
        );
        assert_eq!(
            production_run_plan_value["paths"]["combined_bundle"],
            "target/prod/combined-production-evidence-bundle.json"
        );
        assert!(production_run_plan_value["phases"]
            .as_array()
            .unwrap()
            .iter()
            .any(|phase| phase["id"] == "provider_evidence_matrix"
                && phase["command"]
                    .as_str()
                    .unwrap_or_default()
                    .contains("--production-upstream")
                && phase["command"].as_str().unwrap_or_default().contains(
                    "--provider-endpoint-env sam-3d=POOL_PROVIDER_ENDPOINT_SAM_3D"
                )
                && phase["command"].as_str().unwrap_or_default().contains(
                    "--provider-api-key-env qunhe-3d=POOL_PROVIDER_API_KEY_QUNHE_3D"
                )
                && phase["command"].as_str().unwrap_or_default().contains(
                    "--provider-attestation-env worldlabs-marble=POOL_PROVIDER_PRODUCTION_ATTESTATION_WORLDLABS_MARBLE"
                )
                && phase["command"].as_str().unwrap_or_default().contains(
                    "--evidence-bundle=target/prod/provider-production-evidence-bundle.json"
                )));
        assert!(production_run_plan_value["phases"]
            .as_array()
            .unwrap()
            .iter()
            .any(|phase| phase["id"] == "software_evidence_matrix"
                && phase["command"]
                    .as_str()
                    .unwrap_or_default()
                    .contains("--production-software")));
        assert!(production_run_plan_value["phases"]
            .as_array()
            .unwrap()
            .iter()
            .any(|phase| phase["id"] == "desktop_vision_evidence"
                && phase["command"]
                    .as_str()
                    .unwrap_or_default()
                    .contains("production-evidence-desktop-vision")));
        assert_eq!(
            production_run_plan_value["commands"]["completion_gate"],
            "pool-cli --project demo prd-completion-gate --require-complete"
        );

        let production_item_template = server
            .handle_path(
                "/api/production-evidence/item-template?project=demo&task_id=provider:midjourney:production_upstream&output_root=target/prod&source=external-worker",
            )
            .unwrap();
        let production_item_template_value: serde_json::Value =
            serde_json::from_str(&production_item_template.body).unwrap();
        assert_eq!(production_item_template.status_code, 200);
        assert_eq!(
            production_item_template_value["kind"],
            "pool_production_evidence_item_template"
        );
        assert_eq!(
            production_item_template_value["selector"]["task_id"],
            "provider:midjourney:production_upstream"
        );
        assert_eq!(production_item_template_value["item"]["kind"], "provider");
        assert_eq!(
            production_item_template_value["item"]["provider"]["provider_id"],
            "midjourney"
        );
        assert!(
            production_item_template_value["item"]["provider"]["metadata_path"]
                .as_str()
                .unwrap()
                .starts_with("target/prod/worlds/demo/output/production/midjourney/")
        );

        let invalid_item_template = server
            .handle_path("/api/production-evidence/item-template?project=demo")
            .unwrap();
        let invalid_item_template_value: serde_json::Value =
            serde_json::from_str(&invalid_item_template.body).unwrap();
        assert_eq!(invalid_item_template.status_code, 400);
        assert_eq!(
            invalid_item_template_value["error"],
            "invalid_production_evidence_item_template_request"
        );

        let production_handoff = server
            .handle_path("/api/production-evidence/handoff?project=demo")
            .unwrap();
        let production_handoff_value: serde_json::Value =
            serde_json::from_str(&production_handoff.body).unwrap();
        assert_eq!(production_handoff.status_code, 200);
        assert_eq!(
            production_handoff_value["kind"],
            "pool_production_evidence_handoff"
        );
        assert_eq!(
            production_handoff_value["missing_only_template"]["scope"]["missing_only"],
            true
        );
        assert_eq!(
            production_handoff_value["commands"]["validate"],
            "pool-cli --project demo validate-production-evidence <bundle.json>"
        );
        assert_eq!(
            production_handoff_value["commands"]["merge"],
            "pool-cli --project demo merge-production-evidence <combined-bundle.json> <bundle-a.json> <bundle-b.json>..."
        );
        assert_eq!(
            production_handoff_value["commands"]["closeout"],
            "pool-cli --project demo closeout-production-evidence --output <merged-bundle.json> <bundle-a.json> <bundle-b.json>..."
        );
        assert_eq!(
            production_handoff_value["commands"]["submit_item"],
            "pool-cli --project demo submit-production-evidence-item <item.json>"
        );
        assert!(production_handoff_value["bundle"]["providers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|provider| provider["provider_id"] == "midjourney"));

        let missing_node = server
            .handle_path("/api/node-context?node_id=missing-node")
            .unwrap();
        let missing_node_value: serde_json::Value =
            serde_json::from_str(&missing_node.body).unwrap();
        assert_eq!(missing_node.status_code, 404);
        assert_eq!(missing_node_value["error"], "node_context_not_found");
    }

    #[test]
    fn returns_runtime_discovery_descriptor() {
        let db_path = temp_db_path("runtime-http-discovery");
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        let plan = build_default_content_burst_plan("demo", "Pool HTTP demo");
        repository.persist_plan(&plan).unwrap();
        drop(repository);

        let server = RuntimeHttpServer::new(
            RuntimeHttpConfig::new(&db_path)
                .with_project_slug("demo")
                .with_bind_addr("127.0.0.1:4799"),
        );

        let response = server.handle_path("/api/discovery").unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 200);
        assert_eq!(value["service"], "pool-runtime");
        assert_eq!(value["base_url"], "http://127.0.0.1:4799");
        assert_eq!(value["project_filter"], "demo");
        assert_eq!(value["capabilities"]["runtime_registry"], true);
        assert_eq!(value["capabilities"]["event_stream"], true);
        assert_eq!(
            value["capabilities"]["event_stream_transport"],
            "sse+websocket"
        );
        assert_eq!(value["capabilities"]["event_websocket"], true);
        assert!(value["capabilities"]["event_stream_transports"]
            .as_array()
            .unwrap()
            .iter()
            .any(|transport| transport == "websocket"));
        assert_eq!(value["capabilities"]["mcp_tools"], true);
        assert_eq!(value["capabilities"]["provider_contracts"], true);
        assert_eq!(value["capabilities"]["provider_gateway_worker"], true);
        assert_eq!(value["capabilities"]["provider_conformance_packages"], true);
        assert_eq!(
            value["capabilities"]["integration_conformance_packages"],
            true
        );
        assert_eq!(value["capabilities"]["integration_readiness"], true);
        assert_eq!(value["capabilities"]["provider_request_metadata"], true);
        assert_eq!(value["capabilities"]["software_contracts"], true);
        assert_eq!(value["capabilities"]["software_conformance_packages"], true);
        assert_eq!(value["capabilities"]["agent_conformance_packages"], true);
        assert_eq!(value["capabilities"]["agent_session_transcripts"], true);
        assert_eq!(value["capabilities"]["agent_session_stream"], true);
        assert_eq!(value["capabilities"]["agent_session_websocket"], true);
        assert!(value["capabilities"]["agent_session_stream_transports"]
            .as_array()
            .unwrap()
            .iter()
            .any(|transport| transport == "websocket"));
        assert_eq!(
            value["endpoints"]["runtime_registry"],
            "/api/runtime-registry"
        );
        assert_eq!(value["endpoints"]["events_stream"], "/api/events/stream");
        assert_eq!(value["endpoints"]["events_websocket"], "/api/events/ws");
        assert_eq!(
            value["endpoints"]["provider_contracts"],
            "/api/provider-contracts?provider_id=<provider-id>"
        );
        assert_eq!(
            value["endpoints"]["provider_gateway_worker"],
            "/api/provider-gateway-worker"
        );
        assert_eq!(
            value["endpoints"]["mcp_provider_gateway_worker"],
            "/api/mcp?uri=pool://provider-gateway-worker"
        );
        assert_eq!(
            value["endpoints"]["provider_conformance_packages"],
            "/api/provider-conformance-packages"
        );
        assert_eq!(
            value["endpoints"]["integration_conformance_packages"],
            "/api/integration-conformance-packages"
        );
        assert_eq!(
            value["endpoints"]["integration_readiness"],
            "/api/integration-readiness"
        );
        assert_eq!(
            value["endpoints"]["mcp_integration_readiness"],
            "/api/mcp?uri=pool://integration-readiness"
        );
        assert_eq!(
            value["endpoints"]["software_contracts"],
            "/api/software-contracts?adapter_id=<adapter-id>"
        );
        assert_eq!(
            value["endpoints"]["provider_request_metadata"],
            "/api/provider-requests/metadata?provider_request_id=<provider-request-id>"
        );
        assert_eq!(
            value["endpoints"]["agent_session_transcript"],
            "/api/agent-sessions/transcript?session_id=<agent-session-id>"
        );
        assert_eq!(
            value["endpoints"]["agent_conformance_packages"],
            "/api/agent-conformance-packages"
        );
        assert_eq!(
            value["endpoints"]["agent_session_stream"],
            "/api/agent-sessions/stream?session_id=<agent-session-id>"
        );
        assert_eq!(
            value["endpoints"]["agent_session_websocket"],
            "/api/agent-sessions/ws?session_id=<agent-session-id>"
        );
        let mcp_tools = value["mcp_tools"].as_array().unwrap();
        assert!(mcp_tools
            .iter()
            .any(|tool| tool["name"] == "pool_worker_self_checks"
                && tool["transport"] == "mcp_stdio"
                && tool["example_arguments"]["software_adapter"] == "resolve"));
        assert!(mcp_tools
            .iter()
            .any(|tool| tool["name"] == "pool_handoff_package"
                && tool["transport"] == "mcp_stdio"
                && tool["category"] == "write"
                && tool["example_arguments"]["include_snapshot"] == true));
        assert!(mcp_tools
            .iter()
            .any(|tool| tool["name"] == "pool_software_conformance_package"
                && tool["transport"] == "mcp_stdio"
                && tool["category"] == "write"
                && tool["example_arguments"]["adapter_id"] == "resolve"));
        assert!(mcp_tools
            .iter()
            .any(|tool| tool["name"] == "pool_provider_conformance_package"
                && tool["transport"] == "mcp_stdio"
                && tool["category"] == "write"
                && tool["example_arguments"]["provider_id"] == "worldlabs-marble"));
        assert!(mcp_tools.iter().any(|tool| tool["name"]
            == "pool_integration_conformance_package"
            && tool["transport"] == "mcp_stdio"
            && tool["category"] == "write"
            && tool["example_arguments"]["agent_kind"] == "all"));
        assert!(mcp_tools
            .iter()
            .any(|tool| tool["name"] == "pool_integration_readiness"
                && tool["transport"] == "mcp_stdio"
                && tool["category"] == "read"));
        assert!(mcp_tools
            .iter()
            .any(|tool| tool["name"] == "pool_agent_conformance_package"
                && tool["transport"] == "mcp_stdio"
                && tool["category"] == "write"
                && tool["example_arguments"]["kind"] == "all"));
        assert_eq!(
            value["endpoints"]["node_context"],
            "/api/node-context?node_id=<node-id>"
        );
        assert_eq!(value["endpoints"]["prompts"], "/api/prompts");
        assert_eq!(
            value["endpoints"]["prompt"],
            "/api/prompts?name=<prompt-name>"
        );
        assert_eq!(value["capabilities"]["mcp_prompts"], true);
        assert_eq!(value["capabilities"]["runtime_execution_plan"], true);
        assert_eq!(
            value["capabilities"]["runtime_execution_plan_run_next"],
            true
        );
        assert_eq!(value["capabilities"]["runtime_handoff"], true);
        assert_eq!(value["capabilities"]["prd_readiness"], true);
        assert_eq!(value["capabilities"]["production_evidence_tasks"], true);
        assert_eq!(
            value["capabilities"]["production_evidence_task_claim"],
            true
        );
        assert_eq!(value["capabilities"]["production_evidence_handoff"], true);
        assert_eq!(
            value["capabilities"]["production_evidence_item_template"],
            true
        );
        assert_eq!(value["capabilities"]["production_evidence_merge"], true);
        assert_eq!(value["capabilities"]["production_evidence_closeout"], true);
        assert_eq!(
            value["capabilities"]["production_evidence_item_validate"],
            true
        );
        assert_eq!(value["capabilities"]["production_evidence_items"], true);
        assert_eq!(value["capabilities"]["handoff_packages"], true);
        assert_eq!(value["capabilities"]["handoff_package_catalog"], true);
        assert_eq!(
            value["endpoints"]["mcp_desktop_recognition"],
            "/api/mcp?uri=pool://desktop-recognition"
        );
        assert_eq!(
            value["endpoints"]["mcp_software_contracts"],
            "/api/mcp?uri=pool://software-contracts"
        );
        assert_eq!(
            value["endpoints"]["software_conformance_packages"],
            "/api/software-conformance-packages"
        );
        assert_eq!(value["capabilities"]["desktop_recognition_contract"], true);
        assert_eq!(
            value["endpoints"]["mcp_desktop_recognition_contract"],
            "/api/mcp?uri=pool://desktop-recognition-contract"
        );
        assert_eq!(
            value["endpoints"]["desktop_recognition_contract"],
            "/api/desktop-recognition/contract"
        );
        assert_eq!(
            value["endpoints"]["desktop_recognition_run_next"],
            "/api/desktop-recognition/run-next"
        );
        assert_eq!(value["endpoints"]["runtime_graph"], "/api/runtime-graph");
        assert_eq!(
            value["endpoints"]["runtime_execution_plan"],
            "/api/runtime-execution-plan"
        );
        assert_eq!(
            value["endpoints"]["runtime_execution_plan_run_next"],
            "/api/runtime-execution-plan/run-next"
        );
        assert_eq!(
            value["endpoints"]["runtime_handoff"],
            "/api/runtime-handoff"
        );
        assert_eq!(
            value["endpoints"]["mcp_runtime_handoff_packages"],
            "/api/mcp?uri=pool://runtime-handoff-packages"
        );
        assert_eq!(value["endpoints"]["prd_readiness"], "/api/prd-readiness");
        assert_eq!(
            value["endpoints"]["production_evidence_requirements"],
            "/api/production-evidence/requirements"
        );
        assert_eq!(
            value["endpoints"]["production_evidence_tasks"],
            "/api/production-evidence/tasks"
        );
        assert_eq!(
            value["endpoints"]["production_evidence_task_claim"],
            "/api/production-evidence/tasks/claim"
        );
        assert_eq!(value["capabilities"]["production_evidence_run_plan"], true);
        assert_eq!(
            value["endpoints"]["production_evidence_run_plan"],
            "/api/production-evidence/run-plan"
        );
        assert_eq!(
            value["endpoints"]["mcp_production_evidence_tasks"],
            "/api/mcp?uri=pool://production-evidence-tasks"
        );
        assert_eq!(
            value["endpoints"]["mcp_production_evidence_run_plan"],
            "/api/mcp?uri=pool://production-evidence-run-plan"
        );
        assert_eq!(
            value["endpoints"]["mcp_production_evidence_handoff"],
            "/api/mcp?uri=pool://production-evidence-handoff"
        );
        assert_eq!(
            value["endpoints"]["mcp_production_evidence_item_template"],
            "/api/mcp?uri=pool://production-evidence-item-template/<task-id>"
        );
        assert_eq!(
            value["endpoints"]["production_evidence_handoff"],
            "/api/production-evidence/handoff"
        );
        assert_eq!(
            value["endpoints"]["production_evidence_item_template"],
            "/api/production-evidence/item-template?kind=<kind>&target_id=<target-id>"
        );
        assert_eq!(
            value["endpoints"]["production_evidence_merge"],
            "/api/production-evidence/merge"
        );
        assert_eq!(
            value["endpoints"]["production_evidence_closeout"],
            "/api/production-evidence/closeout"
        );
        assert_eq!(
            value["endpoints"]["production_evidence_item_validate"],
            "/api/production-evidence/items/validate"
        );
        assert_eq!(
            value["endpoints"]["production_evidence_items"],
            "/api/production-evidence/items"
        );
        assert_eq!(
            value["endpoints"]["handoff_packages"],
            "/api/handoff-packages"
        );
        assert_eq!(
            value["endpoints"]["workflow_context"],
            "/api/workflow-context?workflow_id=<workflow-id>"
        );
        assert_eq!(
            value["endpoints"]["mcp_runtime_graph"],
            "/api/mcp?uri=pool://runtime-graph"
        );
        assert_eq!(
            value["endpoints"]["mcp_runtime_execution_plan"],
            "/api/mcp?uri=pool://runtime-execution-plan"
        );
        assert_eq!(
            value["endpoints"]["mcp_adapters"],
            "/api/mcp?uri=pool://adapters"
        );
        assert_eq!(
            value["endpoints"]["mcp_workflow_context"],
            "/api/mcp?uri=pool://workflow/<workflow-id>"
        );
        assert_eq!(
            value["endpoints"]["mcp_node_context"],
            "/api/mcp?uri=pool://node-context/<node-id>"
        );
        assert!(value["mcp_resources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|resource| resource["uri"] == "pool://desktop-recognition"
                && resource["http_path"] == "/api/mcp?uri=pool%3A%2F%2Fdesktop-recognition"));
        assert!(value["mcp_resources"]
            .as_array()
            .unwrap()
            .iter()
            .any(
                |resource| resource["uri"] == "pool://desktop-recognition-contract"
                    && resource["http_path"]
                        == "/api/mcp?uri=pool%3A%2F%2Fdesktop-recognition-contract"
            ));
        assert!(value["mcp_resources"]
            .as_array()
            .unwrap()
            .iter()
            .any(
                |resource| resource["uri"] == "pool://provider-gateway-worker"
                    && resource["http_path"] == "/api/mcp?uri=pool%3A%2F%2Fprovider-gateway-worker"
            ));
        assert!(value["mcp_resources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|resource| resource["uri"] == "pool://software-contracts"
                && resource["http_path"] == "/api/mcp?uri=pool%3A%2F%2Fsoftware-contracts"));
        assert!(value["mcp_resources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|resource| resource["uri"] == "pool://runtime-graph"
                && resource["http_path"] == "/api/mcp?uri=pool%3A%2F%2Fruntime-graph"));
        assert!(value["mcp_resources"]
            .as_array()
            .unwrap()
            .iter()
            .any(
                |resource| resource["uri"] == "pool://runtime-execution-plan"
                    && resource["http_path"] == "/api/mcp?uri=pool%3A%2F%2Fruntime-execution-plan"
            ));
        assert!(value["mcp_resources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|resource| resource["uri"] == "pool://runtime-handoff"
                && resource["http_path"] == "/api/mcp?uri=pool%3A%2F%2Fruntime-handoff"));
        assert!(value["mcp_resources"]
            .as_array()
            .unwrap()
            .iter()
            .any(
                |resource| resource["uri"] == "pool://runtime-handoff-packages"
                    && resource["http_path"]
                        == "/api/mcp?uri=pool%3A%2F%2Fruntime-handoff-packages"
            ));
        assert!(value["mcp_resources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|resource| resource["uri"] == "pool://prd-readiness"
                && resource["http_path"] == "/api/mcp?uri=pool%3A%2F%2Fprd-readiness"));
        assert!(value["mcp_resources"]
            .as_array()
            .unwrap()
            .iter()
            .any(
                |resource| resource["uri"] == "pool://production-evidence-requirements"
                    && resource["http_path"]
                        == "/api/mcp?uri=pool%3A%2F%2Fproduction-evidence-requirements"
            ));
        assert!(value["mcp_resources"]
            .as_array()
            .unwrap()
            .iter()
            .any(
                |resource| resource["uri"] == "pool://production-evidence-tasks"
                    && resource["http_path"]
                        == "/api/mcp?uri=pool%3A%2F%2Fproduction-evidence-tasks"
            ));
        assert!(value["mcp_resources"]
            .as_array()
            .unwrap()
            .iter()
            .any(
                |resource| resource["uri"] == "pool://production-evidence-run-plan"
                    && resource["http_path"]
                        == "/api/mcp?uri=pool%3A%2F%2Fproduction-evidence-run-plan"
            ));
        assert!(value["mcp_resources"]
            .as_array()
            .unwrap()
            .iter()
            .any(
                |resource| resource["uri"] == "pool://production-evidence-handoff"
                    && resource["http_path"]
                        == "/api/mcp?uri=pool%3A%2F%2Fproduction-evidence-handoff"
            ));
        assert!(value["mcp_resources"]
            .as_array()
            .unwrap()
            .iter()
            .any(
                |resource| resource["uri"] == "pool://production-evidence-item-template"
                    && resource["http_path"]
                        == "/api/mcp?uri=pool%3A%2F%2Fproduction-evidence-item-template"
            ));
        assert!(value["mcp_resources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|resource| resource["uri"] == "pool://adapters"
                && resource["http_path"] == "/api/mcp?uri=pool%3A%2F%2Fadapters"));
        assert!(value["mcp_resources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|resource| resource["uri"] == "pool://integration-readiness"
                && resource["http_path"] == "/api/mcp?uri=pool%3A%2F%2Fintegration-readiness"));
        assert!(value["mcp_resources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|resource| resource["uri"] == "pool://node-context"
                && resource["http_path"] == "/api/mcp?uri=pool%3A%2F%2Fnode-context"));
        assert!(value["mcp_prompts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|prompt| prompt["name"] == "pool_software_handoff"
                && prompt["http_path"] == "/api/prompts?name=pool_software_handoff"));
        assert_eq!(value["projects"][0]["slug"], "demo");

        let well_known = server
            .handle_path("/.well-known/pool-runtime.json")
            .unwrap();
        let wrong_method = server.handle_request("POST", "/api/discovery").unwrap();
        assert_eq!(well_known.status_code, 200);
        assert_eq!(wrong_method.status_code, 405);
    }

    #[test]
    fn returns_and_writes_runtime_registry() {
        let db_path = temp_db_path("runtime-http-registry");
        let server = RuntimeHttpServer::new(
            RuntimeHttpConfig::new(&db_path)
                .with_project_slug("demo")
                .with_bind_addr("127.0.0.1:4877"),
        );

        let response = server.handle_path("/api/runtime-registry").unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 200);
        assert_eq!(value["kind"], "pool_runtime_registry");
        assert_eq!(value["base_url"], "http://127.0.0.1:4877");
        assert_eq!(value["runtime_endpoint"], "http://127.0.0.1:4877");
        assert_eq!(value["project_slug"], "demo");
        assert_eq!(value["endpoints"]["discovery"], "/api/discovery");
        assert_eq!(
            value["endpoints"]["runtime_registry"],
            "/api/runtime-registry"
        );

        let registry_path = std::env::temp_dir()
            .join(format!("pool-runtime-registry-{}", Uuid::new_v4()))
            .join("runtime-registry.json");
        server.write_runtime_registry(&registry_path).unwrap();
        let file_value: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(registry_path).unwrap()).unwrap();

        assert_eq!(file_value["service"], "pool-runtime");
        assert_eq!(file_value["base_url"], "http://127.0.0.1:4877");
        assert_eq!(
            file_value["well_known_url"],
            "http://127.0.0.1:4877/.well-known/pool-runtime.json"
        );

        let wrong_method = server
            .handle_request("POST", "/api/runtime-registry")
            .unwrap();
        assert_eq!(wrong_method.status_code, 405);
    }

    #[test]
    fn returns_runtime_prompt_registry_and_prompt_text() {
        let db_path = temp_db_path("runtime-http-prompts");
        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));

        let list = server.handle_path("/api/prompts").unwrap();
        let list_value: serde_json::Value = serde_json::from_str(&list.body).unwrap();

        assert_eq!(list.status_code, 200);
        assert!(list_value["prompts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|prompt| prompt["name"] == "pool_content_burst_runbook"));

        let prompt = server
            .handle_path(
                "/api/prompts?name=pool_software_handoff&project_slug=demo&adapter_id=blender&action_kind=ExecuteCli",
            )
            .unwrap();
        let prompt_value: serde_json::Value = serde_json::from_str(&prompt.body).unwrap();

        assert_eq!(prompt.status_code, 200);
        assert_eq!(
            prompt_value["description"],
            "Pool external software control handoff prompt"
        );
        assert!(prompt_value["messages"][0]["content"]["text"]
            .as_str()
            .unwrap_or_default()
            .contains("Adapter: blender"));

        let invalid = server
            .handle_path("/api/prompts?name=pool_software_handoff")
            .unwrap();
        assert_eq!(invalid.status_code, 400);

        let wrong_method = server.handle_request("POST", "/api/prompts").unwrap();
        assert_eq!(wrong_method.status_code, 405);
    }

    #[test]
    fn returns_runtime_adapter_registry() {
        let db_path = temp_db_path("runtime-http-adapters");
        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));

        let response = server.handle_path("/api/adapters").unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 200);
        assert!(value["providers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|provider| provider["id"] == "openai-image-2"));
        assert!(value["software_adapters"]
            .as_array()
            .unwrap()
            .iter()
            .any(|adapter| adapter["id"] == "unreal"));
        assert_eq!(value["control_priority_chain"][0], "ApiMcp");
        assert_eq!(
            value["provider_aliases"]["world-labs-marble"],
            "worldlabs-marble"
        );
        assert_eq!(value["policy"]["local_files_authoritative"], true);

        let mcp_response = server
            .handle_path("/api/mcp?uri=pool%3A%2F%2Fadapters")
            .unwrap();
        let mcp_value: serde_json::Value = serde_json::from_str(&mcp_response.body).unwrap();

        assert_eq!(mcp_response.status_code, 200);
        assert_eq!(mcp_value["provider_aliases"]["triposplat"], "tripo-splat");
        assert_eq!(mcp_value["control_priority_chain"][0], "ApiMcp");

        let wrong_method = server.handle_request("POST", "/api/adapters").unwrap();
        assert_eq!(wrong_method.status_code, 405);
    }

    #[test]
    fn returns_integration_readiness_matrix() {
        let db_path = temp_db_path("runtime-http-integration-readiness");
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        let plan = build_default_content_burst_plan("demo", "Pool readiness demo");
        repository.persist_plan(&plan).unwrap();
        drop(repository);

        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let response = server.handle_path("/api/integration-readiness").unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 200);
        assert_eq!(value["kind"], "pool_integration_readiness");
        assert_eq!(value["project_filter"], "demo");
        assert!(value["summary"]["providers"].as_u64().unwrap() >= 9);
        assert!(value["summary"]["software_adapters"].as_u64().unwrap() >= 11);
        assert_eq!(value["summary"]["lanes"], 5);
        assert!(value["summary"]["actions"].as_u64().unwrap() > 0);
        assert!(value["lanes"].as_array().unwrap().iter().any(|lane| {
            lane["lane"] == "orchestration" && lane["title"] == "制片 / Agent 编排"
        }));
        assert!(value["run_plan"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["action"]["command"]
                .as_str()
                .unwrap_or_default()
                .contains("set-api-key")));
        assert!(value["providers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|provider| {
                provider["provider_id"] == "worldlabs-marble"
                    && provider["lane"] == "spatial_engine"
                    && provider["commands"]["conformance_package"]
                        .as_str()
                        .unwrap_or_default()
                        .contains("provider-conformance-package")
            }));
        assert!(value["software_adapters"]
            .as_array()
            .unwrap()
            .iter()
            .any(|adapter| adapter["adapter_id"] == "unreal"
                && adapter["commands"]["conformance_package"]
                    .as_str()
                    .unwrap_or_default()
                    .contains("software-conformance-package")));
        assert_eq!(value["agent"]["status"], "needs_execution");
        assert!(value["commands"]["integration_conformance_package"]
            .as_str()
            .unwrap_or_default()
            .contains("integration-conformance-package"));

        let mcp_response = server
            .handle_path("/api/mcp?uri=pool%3A%2F%2Fintegration-readiness")
            .unwrap();
        let mcp_value: serde_json::Value = serde_json::from_str(&mcp_response.body).unwrap();

        assert_eq!(mcp_response.status_code, 200);
        assert_eq!(mcp_value["kind"], "pool_integration_readiness");
        assert_eq!(
            mcp_value["summary"]["providers"],
            value["summary"]["providers"]
        );

        let wrong_method = server
            .handle_request("POST", "/api/integration-readiness")
            .unwrap();
        assert_eq!(wrong_method.status_code, 405);
    }

    #[test]
    fn returns_provider_contracts_over_http_and_mcp() {
        let db_path = temp_db_path("runtime-http-provider-contracts");
        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));

        let list = server.handle_path("/api/provider-contracts").unwrap();
        let list_value: serde_json::Value = serde_json::from_str(&list.body).unwrap();
        assert_eq!(list.status_code, 200);
        assert!(list_value["contracts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|contract| contract["provider_id"] == "midjourney"));

        let tripo = server
            .handle_path("/api/provider-contracts?provider_id=triposplat")
            .unwrap();
        let tripo_value: serde_json::Value = serde_json::from_str(&tripo.body).unwrap();
        assert_eq!(tripo.status_code, 200);
        assert_eq!(tripo_value["provider_id"], "tripo-splat");
        assert_eq!(tripo_value["adapter_kind"], "three_dgs_http_gateway");
        assert_eq!(
            tripo_value["gateway_submit"]["body"]["pool_gateway_profile"]["profile_id"],
            "triposplat"
        );

        let mcp = server
            .handle_path("/api/mcp?uri=pool%3A%2F%2Fprovider-contracts%2Fmidjourney")
            .unwrap();
        let mcp_value: serde_json::Value = serde_json::from_str(&mcp.body).unwrap();
        assert_eq!(mcp.status_code, 200);
        assert_eq!(mcp_value["provider_id"], "midjourney");
        assert_eq!(mcp_value["adapter_kind"], "generic_http_media_gateway");

        let missing = server
            .handle_path("/api/provider-contracts?provider_id=missing")
            .unwrap();
        assert_eq!(missing.status_code, 404);

        let wrong_method = server
            .handle_request("POST", "/api/provider-contracts")
            .unwrap();
        assert_eq!(wrong_method.status_code, 405);
    }

    #[test]
    fn returns_provider_gateway_worker_contract_over_http_and_mcp() {
        let db_path = temp_db_path("runtime-http-provider-gateway-worker");
        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));

        let response = server.handle_path("/api/provider-gateway-worker").unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();
        assert_eq!(response.status_code, 200);
        assert_eq!(value["kind"], "pool_provider_gateway_worker_contract");
        assert_eq!(value["service"], "pool-provider-gateway-worker");
        assert_eq!(
            value["routes"]["three_dgs_submit"]["path"],
            "/v1/3dgs/<provider>/jobs"
        );
        assert_eq!(
            value["sdk_worker_template"]["routes"]["output_download"]["path"],
            "/outputs/<job-id>/<file>"
        );
        assert_eq!(
            value["sdk_worker_template"]["audit"]["production_evidence_allowed"],
            false
        );
        assert!(value["conformance_runbook"]["phases"]
            .as_array()
            .unwrap()
            .iter()
            .any(|phase| phase["id"] == "production_matrix"
                && phase["command"]
                    .as_str()
                    .unwrap_or_default()
                    .contains("production-evidence-provider-matrix")));

        let mcp = server
            .handle_path("/api/mcp?uri=pool%3A%2F%2Fprovider-gateway-worker")
            .unwrap();
        let mcp_value: serde_json::Value = serde_json::from_str(&mcp.body).unwrap();
        assert_eq!(mcp.status_code, 200);
        assert_eq!(mcp_value["kind"], "pool_provider_gateway_worker_contract");
        assert!(mcp_value["cli"]["primary"]
            .as_str()
            .unwrap()
            .contains("pool-cli provider-gateway-worker"));
        assert!(mcp_value["sdk_worker_template"]["self_check"]
            .as_str()
            .unwrap()
            .contains("provider-sdk-worker-template"));
        assert!(mcp_value["conformance_runbook"]["pass_conditions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|condition| condition
                .as_str()
                .unwrap_or_default()
                .contains("local files")));

        let wrong_method = server
            .handle_request("POST", "/api/provider-gateway-worker")
            .unwrap();
        assert_eq!(wrong_method.status_code, 405);
    }

    #[test]
    fn returns_software_contracts_over_http_and_mcp() {
        let db_path = temp_db_path("runtime-http-software-contracts");
        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));

        let list = server.handle_path("/api/software-contracts").unwrap();
        let list_value: serde_json::Value = serde_json::from_str(&list.body).unwrap();
        assert_eq!(list.status_code, 200);
        assert!(list_value["contracts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|contract| contract["adapter_id"] == "unreal"));
        assert_eq!(
            list_value["summary"]["control_priority"],
            "API/MCP > Skills/CLI > Desktop Recognition > Human Takeover"
        );

        let unreal = server
            .handle_path("/api/software-contracts?adapter_id=unreal")
            .unwrap();
        let unreal_value: serde_json::Value = serde_json::from_str(&unreal.body).unwrap();
        assert_eq!(unreal.status_code, 200);
        assert_eq!(unreal_value["adapter_id"], "unreal");
        assert_eq!(
            unreal_value["runtime_action"]["path"],
            "/api/software-actions"
        );
        assert_eq!(
            unreal_value["control_routes"][0]["adapter_kind"],
            "unreal_mcp"
        );
        assert_eq!(
            unreal_value["fallback"]["desktop_recognition_contract"],
            "pool://desktop-recognition-contract"
        );

        let resolve = server
            .handle_path("/api/software-contracts?adapter_id=resolve")
            .unwrap();
        let resolve_value: serde_json::Value = serde_json::from_str(&resolve.body).unwrap();
        assert_eq!(resolve.status_code, 200);
        assert_eq!(resolve_value["adapter_id"], "resolve");
        assert!(resolve_value["control_routes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|route| route["adapter_kind"] == "generic_software_api_mcp"
                && route["action"]["request_wrapper"] == "pool_software_action + mcp_payload"));
        assert!(resolve_value["control_routes"][0]["local_worker"]["cli"]
            .as_str()
            .unwrap_or_default()
            .contains("pool-cli software-api-bridge-worker resolve"));
        assert!(resolve_value["control_routes"][0]["endpoint_env"]
            .as_array()
            .unwrap()
            .iter()
            .any(|env| env == "POOL_RESOLVE_ENDPOINT"));
        assert!(resolve_value["conformance_runbook"]["phases"]
            .as_array()
            .unwrap()
            .iter()
            .any(|phase| phase["id"] == "production_matrix"
                && phase["command"]
                    .as_str()
                    .unwrap_or_default()
                    .contains("production-evidence-software-matrix")));
        assert!(resolve_value["conformance_runbook"]["phases"]
            .as_array()
            .unwrap()
            .iter()
            .any(|phase| phase["id"] == "real_upstream_bridge"
                && phase["command"]
                    .as_str()
                    .unwrap_or_default()
                    .contains("software-api-bridge-worker resolve")));

        let mcp = server
            .handle_path("/api/mcp?uri=pool%3A%2F%2Fsoftware-contracts%2Fresolve")
            .unwrap();
        let mcp_value: serde_json::Value = serde_json::from_str(&mcp.body).unwrap();
        assert_eq!(mcp.status_code, 200);
        assert_eq!(mcp_value["adapter_id"], "resolve");
        assert_eq!(mcp_value["runtime_health"]["path"], "/api/software-health");
        assert_eq!(
            mcp_value["control_routes"][0]["adapter_kind"],
            "generic_software_api_mcp"
        );
        assert!(mcp_value["conformance_runbook"]["pass_conditions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|condition| condition
                .as_str()
                .unwrap_or_default()
                .contains("local file artifacts")));

        let missing = server
            .handle_path("/api/software-contracts?adapter_id=missing")
            .unwrap();
        assert_eq!(missing.status_code, 404);

        let wrong_method = server
            .handle_request("POST", "/api/software-contracts")
            .unwrap();
        assert_eq!(wrong_method.status_code, 405);
    }

    #[test]
    fn returns_unreal_mcp_bridge_contract_over_http_and_mcp() {
        let db_path = temp_db_path("runtime-http-unreal-mcp-bridge");
        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));

        let response = server.handle_path("/api/unreal-mcp-bridge").unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();
        assert_eq!(response.status_code, 200);
        assert_eq!(value["kind"], "pool_unreal_mcp_bridge_contract");
        assert_eq!(
            value["pool_runtime_routes"]["action_submit"],
            "/api/software-actions"
        );
        assert!(value["tool_contracts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|tool| tool["tool"] == "unreal.create_scene"));

        let discovery = server.handle_path("/api/discovery").unwrap();
        let discovery_value: serde_json::Value = serde_json::from_str(&discovery.body).unwrap();
        assert_eq!(
            discovery_value["capabilities"]["unreal_mcp_bridge_contract"],
            true
        );
        assert_eq!(
            discovery_value["endpoints"]["unreal_mcp_bridge"],
            "/api/unreal-mcp-bridge"
        );
        assert!(discovery_value["mcp_resources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|resource| resource["uri"] == "pool://unreal-mcp-bridge"));

        let mcp = server
            .handle_path("/api/mcp?uri=pool%3A%2F%2Funreal-mcp-bridge")
            .unwrap();
        let mcp_value: serde_json::Value = serde_json::from_str(&mcp.body).unwrap();
        assert_eq!(mcp.status_code, 200);
        assert_eq!(mcp_value["transport"]["default_action"]["path"], "/mcp");

        let wrong_method = server
            .handle_request("POST", "/api/unreal-mcp-bridge")
            .unwrap();
        assert_eq!(wrong_method.status_code, 405);
    }

    #[test]
    fn get_projects_returns_all_runtime_projects() {
        let db_path = temp_db_path("runtime-http-projects");
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        let demo_plan = build_default_content_burst_plan("demo", "Pool demo");
        let alt_plan = build_default_content_burst_plan("alt", "Pool alt");
        repository.persist_plan(&demo_plan).unwrap();
        repository.persist_plan(&alt_plan).unwrap();
        drop(repository);

        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let response = server.handle_path("/api/projects").unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();
        let slugs = value["projects"]
            .as_array()
            .unwrap()
            .iter()
            .map(|project| project["slug"].as_str().unwrap())
            .collect::<Vec<_>>();

        assert_eq!(response.status_code, 200);
        assert_eq!(value["project_filter"], "demo");
        assert_eq!(value["count"], 2);
        assert!(slugs.contains(&"demo"));
        assert!(slugs.contains(&"alt"));

        let all = server.handle_path("/api/projects?project=*").unwrap();
        let all_value: serde_json::Value = serde_json::from_str(&all.body).unwrap();
        assert!(all_value["project_filter"].is_null());

        let wrong_method = server.handle_request("POST", "/api/projects").unwrap();
        assert_eq!(wrong_method.status_code, 405);
    }

    #[test]
    fn get_events_supports_limit_and_after_id() {
        let db_path = temp_db_path("runtime-http-events");
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        let mut first = RuntimeEvent::new("demo", RuntimeEventLevel::Info, "first event");
        first.created_at = chrono::Utc::now() - chrono::Duration::seconds(2);
        let mut second = RuntimeEvent::new("demo", RuntimeEventLevel::Ok, "second event");
        second.created_at = chrono::Utc::now() - chrono::Duration::seconds(1);
        let mut third = RuntimeEvent::new("demo", RuntimeEventLevel::Warn, "third event");
        third.created_at = chrono::Utc::now();
        repository.insert_event(&first).unwrap();
        repository.insert_event(&second).unwrap();
        repository.insert_event(&third).unwrap();
        drop(repository);

        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let latest = server.handle_request("GET", "/api/events?limit=2").unwrap();
        let latest_value: serde_json::Value = serde_json::from_str(&latest.body).unwrap();

        assert_eq!(latest.status_code, 200);
        assert_eq!(latest_value["count"], 2);
        assert_eq!(latest_value["events"][0]["message"], "third event");
        assert_eq!(latest_value["events"][1]["message"], "second event");
        assert_eq!(latest_value["latest_event_id"], third.id);

        let delta = server
            .handle_request(
                "GET",
                &format!("/api/events?after_id={}&limit=10", second.id),
            )
            .unwrap();
        let delta_value: serde_json::Value = serde_json::from_str(&delta.body).unwrap();

        assert_eq!(delta.status_code, 200);
        assert_eq!(delta_value["count"], 1);
        assert_eq!(delta_value["events"][0]["message"], "third event");

        let wrong_method = server.handle_request("POST", "/api/events").unwrap();
        assert_eq!(wrong_method.status_code, 405);
    }

    #[test]
    fn get_events_stream_returns_sse_frames() {
        let db_path = temp_db_path("runtime-http-events-stream");
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        let mut first = RuntimeEvent::new("demo", RuntimeEventLevel::Info, "first event");
        first.created_at = chrono::Utc::now() - chrono::Duration::seconds(1);
        let mut second = RuntimeEvent::new("demo", RuntimeEventLevel::Ok, "second event");
        second.created_at = chrono::Utc::now();
        repository.insert_event(&first).unwrap();
        repository.insert_event(&second).unwrap();
        drop(repository);

        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let response = server
            .handle_request(
                "GET",
                &format!("/api/events/stream?last_event_id={}&limit=10", first.id),
            )
            .unwrap();

        assert_eq!(response.status_code, 200);
        assert_eq!(response.content_type, "text/event-stream; charset=utf-8");
        assert!(response.body.contains(": pool-runtime-events"));
        assert!(response.body.contains("event: cursor"));
        assert!(response.body.contains(&format!("data: {}", second.id)));
        assert!(response.body.contains(&format!("id: {}", second.id)));
        assert!(response.body.contains("event: runtime-event"));
        assert!(response.body.contains("\"message\":\"second event\""));
        assert!(!response.body.contains("\"message\":\"first event\""));

        let wrong_method = server.handle_request("POST", "/api/events/stream").unwrap();
        assert_eq!(wrong_method.status_code, 405);
    }

    #[test]
    fn get_events_websocket_requires_upgrade_and_streams_frames() {
        let db_path = temp_db_path("runtime-http-events-websocket");
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        let mut first = RuntimeEvent::new("demo", RuntimeEventLevel::Info, "first event");
        first.created_at = chrono::Utc::now() - chrono::Duration::seconds(1);
        let mut second = RuntimeEvent::new("demo", RuntimeEventLevel::Ok, "second event");
        second.created_at = chrono::Utc::now();
        repository.insert_event(&first).unwrap();
        repository.insert_event(&second).unwrap();
        drop(repository);

        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let no_upgrade = server.handle_request("GET", "/api/events/ws").unwrap();
        let no_upgrade_value: serde_json::Value = serde_json::from_str(&no_upgrade.body).unwrap();

        assert_eq!(no_upgrade.status_code, 426);
        assert_eq!(no_upgrade_value["error"], "websocket_upgrade_required");
        assert_eq!(
            websocket_accept_key("dGhlIHNhbXBsZSBub25jZQ=="),
            "s3pPLMBiTxaQ9kYGzzhZRbK+xOo="
        );

        let request = RuntimeHttpRequest::parse(&format!(
            "/api/events/ws?last_event_id={}&poll_ms=500",
            first.id
        ))
        .unwrap();
        let mut buffer = Vec::new();

        server
            .stream_events_websocket_to_writer(&mut buffer, request, Some(1))
            .unwrap();
        let frames = websocket_text_frames(&buffer);
        let prelude: serde_json::Value = serde_json::from_str(&frames[0]).unwrap();
        let event_frame: serde_json::Value = serde_json::from_str(&frames[1]).unwrap();

        assert_eq!(prelude["type"], "pool-runtime-events");
        assert_eq!(prelude["transport"], "websocket");
        assert_eq!(prelude["latest_event_id"], second.id);
        assert_eq!(event_frame["type"], "runtime-event");
        assert_eq!(event_frame["event"]["id"], second.id);
        assert_eq!(event_frame["event"]["message"], "second event");
        assert!(!frames.join("\n").contains("first event"));

        let wrong_method = server.handle_request("POST", "/api/events/ws").unwrap();
        assert_eq!(wrong_method.status_code, 405);
    }

    #[test]
    fn sse_stream_writer_keeps_connection_open_and_emits_delta() {
        let db_path = temp_db_path("runtime-http-events-stream-writer");
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        let mut first = RuntimeEvent::new("demo", RuntimeEventLevel::Info, "first event");
        first.created_at = chrono::Utc::now() - chrono::Duration::seconds(1);
        let mut second = RuntimeEvent::new("demo", RuntimeEventLevel::Ok, "second event");
        second.created_at = chrono::Utc::now();
        repository.insert_event(&first).unwrap();
        repository.insert_event(&second).unwrap();
        drop(repository);

        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let request = RuntimeHttpRequest::parse(&format!(
            "/api/events/stream?last_event_id={}&poll_ms=500",
            first.id
        ))
        .unwrap();
        let mut buffer = Vec::new();

        server
            .stream_events_to_writer(&mut buffer, request, Some(1))
            .unwrap();
        let output = String::from_utf8(buffer).unwrap();

        assert!(output.starts_with("HTTP/1.1 200 OK"));
        assert!(output.contains("Content-Type: text/event-stream; charset=utf-8"));
        assert!(output.contains("Connection: keep-alive"));
        assert!(output.contains(": pool-runtime-events"));
        assert!(output.contains(&format!("id: {}", second.id)));
        assert!(output.contains("\"message\":\"second event\""));
        assert!(!output.contains("\"message\":\"first event\""));
    }

    #[test]
    fn post_adapter_health_checks_requested_provider_and_software() {
        let resolve_endpoint = spawn_fake_generic_software_api_server("resolve");
        let db_path = temp_db_path("runtime-http-adapter-health");
        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));

        let response = server
            .handle_request_with_body(
                "POST",
                "/api/adapter-health",
                r#"{
                    "providers": [
                        {"provider_id":"worldlabs-marble","execution_mode":"mock"},
                        {"provider_id":""}
                    ],
                    "software_adapters": [
                        {"adapter_id":"unreal","priority":"ApiMcp"},
                        {"adapter_id":"resolve","priority":"ApiMcp","payload_json":{"endpoint":"__RESOLVE_ENDPOINT__"}},
                        {"adapter_id":""}
                    ]
                }"#
                .replace("__RESOLVE_ENDPOINT__", &resolve_endpoint)
                .as_str(),
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 200);
        assert_eq!(value["summary"]["providers_total"], 2);
        assert_eq!(value["summary"]["providers_ready"], 1);
        assert_eq!(value["summary"]["software_total"], 3);
        assert_eq!(value["summary"]["software_ready"], 2);
        assert_eq!(value["summary"]["failed"], 2);
        assert_eq!(value["providers"][0]["provider_id"], "worldlabs-marble");
        assert_eq!(value["providers"][0]["health"]["status"], "ready");
        assert_eq!(value["providers"][1]["error"], "missing_provider_id");
        assert_eq!(value["software_adapters"][0]["adapter_id"], "unreal");
        assert_eq!(value["software_adapters"][0]["health"]["ok"], true);
        assert_eq!(value["software_adapters"][1]["adapter_id"], "resolve");
        assert_eq!(value["software_adapters"][1]["adapter_mode"], "api_mcp");
        assert_eq!(
            value["software_adapters"][1]["health"]["message"],
            "generic-software-health-ok"
        );
        assert_eq!(value["software_adapters"][2]["error"], "missing_adapter_id");

        let wrong_method = server.handle_request("GET", "/api/adapter-health").unwrap();
        assert_eq!(wrong_method.status_code, 405);
    }

    #[test]
    fn post_adapter_health_rejects_invalid_json() {
        let db_path = temp_db_path("runtime-http-adapter-health-invalid");
        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));

        let response = server
            .handle_request_with_body("POST", "/api/adapter-health", "{")
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 400);
        assert_eq!(value["error"], "invalid_adapter_health_request");
    }

    #[test]
    fn post_provider_health_checks_adapter_without_creating_tasks() {
        let db_path = temp_db_path("runtime-http-provider-health");
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        drop(repository);

        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let response = server
            .handle_request_with_body(
                "POST",
                "/api/provider-health",
                r#"{
                    "provider_id":"openai-image-2",
                    "api_key":"sk-test"
                }"#,
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 200);
        assert_eq!(value["provider_id"], "openai-image-2");
        assert_eq!(value["adapter_mode"], "adapter");
        assert_eq!(value["health"]["status"], "ready");

        let repository = RuntimeRepository::open(&db_path).unwrap();
        assert_eq!(repository.table_count("tasks").unwrap(), 0);
        assert_eq!(repository.table_count("provider_requests").unwrap(), 0);
    }

    #[test]
    fn post_provider_health_reports_missing_gateway_endpoint() {
        let db_path = temp_db_path("runtime-http-provider-health-missing-endpoint");
        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));

        let response = server
            .handle_request_with_body(
                "POST",
                "/api/provider-health",
                r#"{
                    "provider_id":"nano-banana-pro",
                    "endpoint":""
                }"#,
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 200);
        assert_eq!(value["provider_id"], "nano-banana-pro");
        assert_eq!(value["adapter_mode"], "gateway");
        assert_eq!(value["health"]["status"], "missing_endpoint");

        let bad_mock = server
            .handle_request_with_body(
                "POST",
                "/api/provider-health",
                r#"{
                    "provider_id":"openai-image-2",
                    "execution_mode":"mock"
                }"#,
            )
            .unwrap();
        assert_eq!(bad_mock.status_code, 400);
    }

    #[test]
    fn post_approve_task_releases_approval_gate() {
        let db_path = temp_db_path("runtime-http-approve");
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        let plan = build_default_content_burst_plan("demo", "Pool HTTP demo");
        repository.persist_plan(&plan).unwrap();
        let task_id = repository
            .snapshot(Some("demo"))
            .unwrap()
            .tasks
            .into_iter()
            .find(|task| task.status == "WaitingApproval")
            .unwrap()
            .id;
        drop(repository);

        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let response = server
            .handle_request("POST", &format!("/api/tasks/approve?task_id={task_id}"))
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 200);
        assert_eq!(value["task"]["status"], "Ready");
        assert_eq!(value["snapshot"]["stats"]["waiting_approval"], 0);
    }

    #[test]
    fn post_cancel_task_marks_task_cancelled_and_writes_event() {
        let db_path = temp_db_path("runtime-http-cancel-task");
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        let task = RuntimeTask::new("demo", "Cancel me");
        let task_id = task.id.clone();
        repository.insert_task(&task).unwrap();
        drop(repository);

        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let response = server
            .handle_request("POST", &format!("/api/tasks/cancel?task_id={task_id}"))
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 200);
        assert_eq!(value["task"]["status"], "Cancelled");
        assert!(value["snapshot"]["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|event| event["message"] == "cancelled task: Cancel me"));
    }

    #[test]
    fn post_retry_task_restores_cancelled_task_to_ready() {
        let db_path = temp_db_path("runtime-http-retry-task");
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        let mut task = RuntimeTask::new("demo", "Retry me");
        task.status = TaskStatus::Cancelled;
        let task_id = task.id.clone();
        repository.insert_task(&task).unwrap();
        drop(repository);

        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let response = server
            .handle_request("POST", &format!("/api/tasks/retry?task_id={task_id}"))
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 200);
        assert_eq!(value["task"]["status"], "Ready");
        assert!(value["snapshot"]["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|event| event["message"] == "retried task: Retry me"));

        let rejected = server
            .handle_request("POST", &format!("/api/tasks/retry?task_id={task_id}"))
            .unwrap();
        assert_eq!(rejected.status_code, 409);
    }

    #[test]
    fn post_create_task_inserts_runtime_task_and_event() {
        let db_path = temp_db_path("runtime-http-create-task");
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        let plan = build_default_content_burst_plan("demo", "Pool HTTP demo");
        repository.persist_plan(&plan).unwrap();
        drop(repository);

        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let response = server
            .handle_request_with_body(
                "POST",
                "/api/tasks",
                r#"{
                    "title":"World Labs Marble 生成任务",
                    "node_id":"node-3dgs",
                    "provider_id":"worldlabs-marble",
                    "cost_estimate_tokens":9000,
                    "requires_approval":true
                }"#,
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 201);
        assert_eq!(value["task"]["status"], "WaitingApproval");
        assert_eq!(value["task"]["provider_id"], "worldlabs-marble");
        assert_eq!(value["snapshot"]["stats"]["tasks"], 10);
        assert_eq!(value["snapshot"]["stats"]["waiting_approval"], 2);
        assert!(value["snapshot"]["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|event| event["message"] == "created task: World Labs Marble 生成任务"));
    }

    #[test]
    fn create_task_validates_json_body_and_title() {
        let db_path = temp_db_path("runtime-http-create-errors");
        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));

        let invalid_json = server
            .handle_request_with_body("POST", "/api/tasks", "{")
            .unwrap();
        assert_eq!(invalid_json.status_code, 400);

        let missing_title = server
            .handle_request_with_body("POST", "/api/tasks", r#"{"title":"   "}"#)
            .unwrap();
        assert_eq!(missing_title.status_code, 400);

        let wrong_method = server.handle_path("/api/tasks").unwrap();
        assert_eq!(wrong_method.status_code, 405);
    }

    #[test]
    fn post_api_key_upserts_sanitized_runtime_secret() {
        let db_path = temp_db_path("runtime-http-api-key");
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        let plan = build_default_content_burst_plan("demo", "Pool HTTP demo");
        repository.persist_plan(&plan).unwrap();
        drop(repository);

        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let response = server
            .handle_request_with_body(
                "POST",
                "/api/api-keys",
                r#"{
                    "provider_id":"openai-image-2",
                    "service_type":"provider",
                    "api_key":"sk-runtime-secret",
                    "metadata":{"env":"OPENAI_API_KEY"}
                }"#,
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 201);
        assert_eq!(value["api_key"]["provider"], "openai-image-2");
        assert_eq!(value["api_key"]["key_hint"], "...cret");
        assert_eq!(value["snapshot"]["stats"]["api_keys"], 1);
        assert!(!response.body.contains("sk-runtime-secret"));

        let keys = server.handle_path("/api/api-keys").unwrap();
        let keys_value: serde_json::Value = serde_json::from_str(&keys.body).unwrap();
        assert_eq!(keys.status_code, 200);
        assert!(keys.body.contains("openai-image-2"));
        assert_eq!(keys_value["audit"]["kind"], "pool_api_key_audit");
        assert_eq!(keys_value["audit"]["total"], 1);
        assert_eq!(keys_value["audit"]["configured"], 1);
        assert_eq!(
            keys_value["audit"]["items"][0]["provider"],
            "openai-image-2"
        );
        assert_eq!(keys_value["audit"]["items"][0]["env"], "OPENAI_API_KEY");
        assert_eq!(keys_value["audit"]["items"][0]["key_hint"], "...cret");
        assert!(!keys.body.contains("sk-runtime-secret"));

        let budget = server.handle_path("/api/runtime-budget").unwrap();
        let budget_value: serde_json::Value = serde_json::from_str(&budget.body).unwrap();
        assert_eq!(budget.status_code, 200);
        assert_eq!(budget_value["summary"]["configured_api_keys"], 1);
        assert_eq!(
            budget_value["summary"]["waiting_approval_estimated_tokens"],
            9_000
        );
        assert!(budget_value["provider_credentials"]
            .as_array()
            .unwrap()
            .iter()
            .any(|provider| provider["provider_id"] == "openai-image-2"
                && provider["configured"] == true
                && provider["key_hint"] == "...cret"));
        assert!(!budget.body.contains("sk-runtime-secret"));

        let preflight = server.handle_path("/api/runtime-preflight").unwrap();
        let preflight_value: serde_json::Value = serde_json::from_str(&preflight.body).unwrap();
        assert_eq!(preflight.status_code, 200);
        assert_eq!(preflight_value["ready"], false);
        assert!(preflight_value["next_actions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|action| action["kind"] == "approval"
                && action["command"]
                    .as_str()
                    .unwrap_or_default()
                    .contains("approve-task")));
        assert!(preflight_value["next_actions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|action| action["kind"] == "local_worker_self_check"
                && action["mcp_tool"] == "pool_worker_self_checks"
                && action["command"]
                    .as_str()
                    .unwrap_or_default()
                    .contains("worker-self-checks")));
        assert!(!preflight.body.contains("sk-runtime-secret"));

        let execution_plan = server.handle_path("/api/runtime-execution-plan").unwrap();
        let execution_plan_value: serde_json::Value =
            serde_json::from_str(&execution_plan.body).unwrap();
        assert_eq!(execution_plan.status_code, 200);
        assert_eq!(execution_plan_value["kind"], "pool_runtime_execution_plan");
        assert!(execution_plan_value["next_steps"]
            .as_array()
            .unwrap()
            .iter()
            .any(|step| step["phase"] == "waiting_approval" && step["gate"]["kind"] == "approval"));
        assert!(execution_plan_value["workflows"][0]["steps"]
            .as_array()
            .unwrap()
            .iter()
            .any(|step| step["contracts"]
                .as_array()
                .unwrap()
                .iter()
                .any(|contract| contract["mcp_uri"] == "pool://software-contracts/unreal")));
        assert!(!execution_plan.body.contains("sk-runtime-secret"));

        let execution_plan_mcp = server
            .handle_path("/api/mcp?uri=pool%3A%2F%2Fruntime-execution-plan")
            .unwrap();
        let execution_plan_mcp_value: serde_json::Value =
            serde_json::from_str(&execution_plan_mcp.body).unwrap();
        assert_eq!(execution_plan_mcp.status_code, 200);
        assert_eq!(
            execution_plan_mcp_value["kind"],
            "pool_runtime_execution_plan"
        );

        let handoff = server.handle_path("/api/runtime-handoff").unwrap();
        let handoff_value: serde_json::Value = serde_json::from_str(&handoff.body).unwrap();
        assert_eq!(handoff.status_code, 200);
        assert_eq!(handoff_value["ready"], false);
        assert!(handoff_value["commands"]
            .as_array()
            .unwrap()
            .iter()
            .any(|command| command["command"]
                .as_str()
                .unwrap_or_default()
                .contains("approve-task")));
        assert!(handoff_value["lanes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|lane| lane["id"] == "local_worker_smoke"
                && lane["actions"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|action| action["mcp_tool"] == "pool_worker_self_checks")));
        assert!(handoff_value["lanes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|lane| lane["id"] == "handoff_package"
                && lane["actions"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|action| action["mcp_tool"] == "pool_handoff_package")));
        assert!(!handoff.body.contains("sk-runtime-secret"));
    }

    #[test]
    fn api_keys_response_reports_rotation_audit_without_secret() {
        let db_path = temp_db_path("runtime-http-api-key-audit");
        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let response = server
            .handle_request_with_body(
                "POST",
                "/api/api-keys",
                r#"{
                    "provider_id":"suno",
                    "service_type":"provider",
                    "api_key":"suno-runtime-secret",
                    "metadata":{
                        "source":"env",
                        "env":"POOL_SUNO_API_KEY",
                        "owner":"creative-tech",
                        "rotation_days":0
                    }
                }"#,
            )
            .unwrap();
        assert_eq!(response.status_code, 201);

        let audit = server
            .handle_path("/api/api-keys?rotation_days=90")
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&audit.body).unwrap();

        assert_eq!(audit.status_code, 200);
        assert_eq!(value["audit"]["total"], 1);
        assert_eq!(value["audit"]["rotation_due"], 1);
        assert_eq!(value["audit"]["unencrypted"], 1);
        assert_eq!(value["audit"]["items"][0]["provider"], "suno");
        assert_eq!(value["audit"]["items"][0]["owner"], "creative-tech");
        assert_eq!(value["audit"]["items"][0]["rotation_days"], 0);
        assert_eq!(value["audit"]["items"][0]["rotation_due"], true);
        assert_eq!(value["audit"]["items"][0]["backend"], "sqlite");
        assert!(!audit.body.contains("suno-runtime-secret"));

        let invalid = server
            .handle_path("/api/api-keys?rotation_days=never")
            .unwrap();
        let invalid_value: serde_json::Value = serde_json::from_str(&invalid.body).unwrap();
        assert_eq!(invalid.status_code, 400);
        assert_eq!(invalid_value["error"], "invalid_api_key_audit_request");
    }

    #[test]
    fn post_runtime_execution_plan_run_next_previews_selected_step() {
        let db_path = temp_db_path("runtime-http-execution-plan-run-next-preview");
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        let plan = build_default_content_burst_plan("demo", "Pool run-next preview");
        let node_id = plan
            .workflow
            .nodes
            .values()
            .find(|node| node.node_type == NodeType::AiImage)
            .unwrap()
            .id
            .clone();
        repository.persist_plan(&plan).unwrap();
        drop(repository);

        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let response = server
            .handle_request_with_body(
                "POST",
                "/api/runtime-execution-plan/run-next",
                &json!({ "node_id": node_id }).to_string(),
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 200);
        assert_eq!(value["mode"], "preview");
        assert_eq!(value["executed"], false);
        assert_eq!(value["selected_step"]["node_id"], node_id);
        assert_eq!(value["action"]["mcp_tool"], "pool_run_node");

        let snapshot = server.handle_path("/api/snapshot").unwrap();
        let snapshot_value: serde_json::Value = serde_json::from_str(&snapshot.body).unwrap();
        assert_eq!(snapshot_value["stats"]["tasks"], plan.workflow.nodes.len());
    }

    #[test]
    fn post_runtime_execution_plan_run_next_requires_explicit_approval_allow() {
        let db_path = temp_db_path("runtime-http-execution-plan-run-next-approval");
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        let plan = build_default_content_burst_plan("demo", "Pool run-next approval");
        let node_id = plan
            .workflow
            .nodes
            .values()
            .find(|node| node.node_type == NodeType::ThreeDgs)
            .unwrap()
            .id
            .clone();
        repository.persist_plan(&plan).unwrap();
        drop(repository);

        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let rejected = server
            .handle_request_with_body(
                "POST",
                "/api/runtime-execution-plan/run-next",
                &json!({ "node_id": node_id, "execute": true }).to_string(),
            )
            .unwrap();
        let rejected_value: serde_json::Value = serde_json::from_str(&rejected.body).unwrap();
        assert_eq!(rejected.status_code, 409);
        assert_eq!(
            rejected_value["error"],
            "runtime_execution_plan_approval_requires_explicit_allow"
        );

        let approved = server
            .handle_request_with_body(
                "POST",
                "/api/runtime-execution-plan/run-next",
                &json!({
                    "node_id": node_id,
                    "execute": true,
                    "allow_approval": true
                })
                .to_string(),
            )
            .unwrap();
        let approved_value: serde_json::Value = serde_json::from_str(&approved.body).unwrap();

        assert_eq!(approved.status_code, 200);
        assert_eq!(approved_value["mode"], "executed");
        assert_eq!(approved_value["executed"], true);
        assert_eq!(approved_value["dispatch"]["task"]["status"], "Ready");
        assert_eq!(
            approved_value["dispatch"]["snapshot"]["stats"]["waiting_approval"],
            0
        );
    }

    #[test]
    fn post_agent_sessions_stages_hermes_and_cli() {
        let db_path = temp_db_path("runtime-http-agent-sessions");
        let control_dir = temp_control_dir("runtime-http-agent-sessions");
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        let plan = build_default_content_burst_plan("demo", "Pool HTTP demo");
        repository.persist_plan(&plan).unwrap();
        drop(repository);

        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let hermes = server
            .handle_request_with_body(
                "POST",
                "/api/agent-sessions",
                &format!(
                    r#"{{
                    "kind":"hermes",
                    "project_slug":"demo",
                    "endpoint":"http://127.0.0.1:8787/hermes",
                    "instruction":"inspect Unreal assembly and report import order",
                    "allowed_tools":["mcp","unreal","sqlite"],
                    "requires_confirmation":false,
                    "control_dir":"{}"
                }}"#,
                    control_dir.display()
                ),
            )
            .unwrap();
        let hermes_value: serde_json::Value = serde_json::from_str(&hermes.body).unwrap();

        assert_eq!(hermes.status_code, 201);
        assert_eq!(hermes_value["report"]["kind"], "Hermes");
        assert_eq!(hermes_value["task"]["provider_id"], "hermes");
        assert_eq!(hermes_value["snapshot"]["stats"]["agent_sessions"], 1);
        assert!(
            std::path::Path::new(hermes_value["report"]["transcript_path"].as_str().unwrap())
                .exists()
        );
        let transcript = server
            .handle_path(&format!(
                "/api/agent-sessions/transcript?session_id={}",
                hermes_value["report"]["session_id"].as_str().unwrap()
            ))
            .unwrap();
        let transcript_value: serde_json::Value = serde_json::from_str(&transcript.body).unwrap();
        assert_eq!(transcript.status_code, 200);
        assert_eq!(
            transcript_value["session_id"],
            hermes_value["report"]["session_id"]
        );
        assert_eq!(
            transcript_value["transcript"]["command"]["instruction"],
            "inspect Unreal assembly and report import order"
        );
        assert_eq!(transcript_value["transcript_text"], serde_json::Value::Null);
        let stream = server
            .handle_path(&format!(
                "/api/agent-sessions/stream?session_id={}&limit=10",
                hermes_value["report"]["session_id"].as_str().unwrap()
            ))
            .unwrap();
        assert_eq!(stream.status_code, 200);
        assert_eq!(stream.content_type, "text/event-stream; charset=utf-8");
        assert!(stream.body.contains(": pool-agent-session"));
        assert!(stream.body.contains("event: agent-transcript"));
        assert!(stream.body.contains("event: runtime-event"));
        assert!(stream.body.contains("Hermes command staged"));
        assert!(stream
            .body
            .contains(hermes_value["report"]["session_id"].as_str().unwrap()));
        let stream_request = RuntimeHttpRequest::parse(&format!(
            "/api/agent-sessions/stream?session_id={}&poll_ms=500",
            hermes_value["report"]["session_id"].as_str().unwrap()
        ))
        .unwrap();
        let mut stream_buffer = Vec::new();
        server
            .stream_agent_session_to_writer(&mut stream_buffer, stream_request, Some(1))
            .unwrap();
        let stream_output = String::from_utf8(stream_buffer).unwrap();
        assert!(stream_output.starts_with("HTTP/1.1 200 OK"));
        assert!(stream_output.contains("Content-Type: text/event-stream; charset=utf-8"));
        assert!(stream_output.contains("Connection: keep-alive"));
        assert!(stream_output.contains("event: agent-transcript"));
        assert!(stream_output.contains("event: runtime-event"));

        let ws_no_upgrade = server
            .handle_path(&format!(
                "/api/agent-sessions/ws?session_id={}",
                hermes_value["report"]["session_id"].as_str().unwrap()
            ))
            .unwrap();
        let ws_no_upgrade_value: serde_json::Value =
            serde_json::from_str(&ws_no_upgrade.body).unwrap();
        assert_eq!(ws_no_upgrade.status_code, 426);
        assert_eq!(ws_no_upgrade_value["path"], "/api/agent-sessions/ws");

        let ws_request = RuntimeHttpRequest::parse(&format!(
            "/api/agent-sessions/ws?session_id={}&poll_ms=500",
            hermes_value["report"]["session_id"].as_str().unwrap()
        ))
        .unwrap();
        let mut ws_buffer = Vec::new();
        server
            .stream_agent_session_websocket_to_writer(&mut ws_buffer, ws_request, Some(1))
            .unwrap();
        let ws_frames = websocket_text_frames(&ws_buffer);
        let ws_transcript: serde_json::Value = serde_json::from_str(&ws_frames[0]).unwrap();
        let ws_event: serde_json::Value = serde_json::from_str(&ws_frames[1]).unwrap();
        assert_eq!(ws_transcript["type"], "agent-session");
        assert_eq!(ws_transcript["transport"], "websocket");
        assert_eq!(
            ws_transcript["session_id"],
            hermes_value["report"]["session_id"]
        );
        assert_eq!(
            ws_transcript["transcript"]["transcript"]["command"]["instruction"],
            "inspect Unreal assembly and report import order"
        );
        assert_eq!(ws_event["type"], "runtime-event");
        assert_eq!(ws_event["session_id"], hermes_value["report"]["session_id"]);
        assert!(ws_event["event"]["message"]
            .as_str()
            .unwrap()
            .contains("Hermes command staged"));

        let missing_session_id = server
            .handle_path("/api/agent-sessions/transcript")
            .unwrap();
        let missing_session_id_value: serde_json::Value =
            serde_json::from_str(&missing_session_id.body).unwrap();
        assert_eq!(missing_session_id.status_code, 400);
        assert_eq!(missing_session_id_value["error"], "missing_session_id");

        let cli = server
            .handle_request_with_body(
                "POST",
                "/api/agent-sessions",
                &format!(
                    r#"{{
                    "kind":"agent_cli",
                    "project_slug":"demo",
                    "command_id":"node-context",
                    "title":"Inspect runtime nodes",
                    "command":"pool-cli --project demo node-context",
                    "tools":["sqlite","filesystem"],
                    "token_budget":4000,
                    "control_dir":"{}"
                }}"#,
                    control_dir.display()
                ),
            )
            .unwrap();
        let cli_value: serde_json::Value = serde_json::from_str(&cli.body).unwrap();

        assert_eq!(cli.status_code, 201);
        assert_eq!(cli_value["report"]["kind"], "AgentCli");
        assert_eq!(cli_value["task"]["provider_id"], "agent-cli");
        assert_eq!(cli_value["snapshot"]["stats"]["agent_sessions"], 2);

        let executed = server
            .handle_request_with_body(
                "POST",
                "/api/agent-sessions",
                &format!(
                    r#"{{
                    "kind":"agent_cli",
                    "project_slug":"demo",
                    "command_id":"echo",
                    "title":"Execute allowed command",
                    "command":"/bin/echo runtime-agent-ok",
                    "tools":["cli"],
                    "token_budget":4000,
                    "execute":true,
                    "allowed_commands":["/bin/echo","echo"],
                    "control_dir":"{}"
                }}"#,
                    control_dir.display()
                ),
            )
            .unwrap();
        let executed_value: serde_json::Value = serde_json::from_str(&executed.body).unwrap();

        assert_eq!(executed.status_code, 201);
        assert_eq!(executed_value["report"]["kind"], "AgentCli");
        assert_eq!(executed_value["report"]["status"], "Succeeded");
        assert_eq!(executed_value["report"]["execution"]["exit_code"], 0);
        assert!(executed_value["report"]["execution"]["stdout"]
            .as_str()
            .unwrap()
            .contains("runtime-agent-ok"));
        assert_eq!(executed_value["snapshot"]["stats"]["agent_sessions"], 3);
    }

    #[test]
    fn post_agent_sessions_validates_required_payload() {
        let db_path = temp_db_path("runtime-http-agent-session-errors");
        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));

        let missing_instruction = server
            .handle_request_with_body(
                "POST",
                "/api/agent-sessions",
                r#"{"kind":"hermes","instruction":" "}"#,
            )
            .unwrap();
        assert_eq!(missing_instruction.status_code, 400);

        let missing_command = server
            .handle_request_with_body(
                "POST",
                "/api/agent-sessions",
                r#"{"kind":"agent_cli","command":" "}"#,
            )
            .unwrap();
        assert_eq!(missing_command.status_code, 400);
    }

    #[test]
    fn post_agent_sessions_executes_hermes_http() {
        let endpoint = spawn_fake_hermes_server();
        let db_path = temp_db_path("runtime-http-hermes-exec");
        let control_dir = temp_control_dir("runtime-http-hermes-exec");
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        let plan = build_default_content_burst_plan("demo", "Pool HTTP demo");
        repository.persist_plan(&plan).unwrap();
        drop(repository);

        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let response = server
            .handle_request_with_body(
                "POST",
                "/api/agent-sessions",
                &format!(
                    r#"{{
                    "kind":"hermes",
                    "project_slug":"demo",
                    "endpoint":"{endpoint}/hermes",
                    "instruction":"inspect Unreal import queue",
                    "allowed_tools":["api","mcp","unreal"],
                    "requires_confirmation":false,
                    "execute":true,
                    "timeout_ms":2000,
                    "control_dir":"{}"
                }}"#,
                    control_dir.display()
                ),
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 201);
        assert_eq!(value["report"]["kind"], "Hermes");
        assert_eq!(value["report"]["status"], "Succeeded");
        assert_eq!(value["task"]["status"], "Succeeded");
        assert_eq!(value["report"]["execution"]["status_code"], 200);
        assert!(value["report"]["execution"]["response_body"]
            .as_str()
            .unwrap()
            .contains("hermes-ok"));
        assert_eq!(value["snapshot"]["stats"]["agent_sessions"], 1);
    }

    #[test]
    fn approve_task_resumes_agent_cli_execution_request() {
        let db_path = temp_db_path("runtime-http-agent-cli-approve-resume");
        let control_dir = temp_control_dir("runtime-http-agent-cli-approve-resume");
        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));

        let staged = server
            .handle_request_with_body(
                "POST",
                "/api/agent-sessions",
                &format!(
                    r#"{{
                    "kind":"agent_cli",
                    "project_slug":"demo",
                    "command_id":"echo",
                    "title":"Approved Agent CLI echo",
                    "command":"/bin/echo approved-agent-ok",
                    "tools":["cli"],
                    "token_budget":1,
                    "execute":true,
                    "allowed_commands":["/bin/echo","echo"],
                    "timeout_ms":2000,
                    "max_output_bytes":1024,
                    "control_dir":"{}"
                }}"#,
                    control_dir.display()
                ),
            )
            .unwrap();
        let staged_value: serde_json::Value = serde_json::from_str(&staged.body).unwrap();
        let task_id = staged_value["task"]["id"].as_str().unwrap().to_string();
        let transcript_path = staged_value["report"]["transcript_path"]
            .as_str()
            .unwrap()
            .to_string();

        assert_eq!(staged.status_code, 201);
        assert_eq!(staged_value["report"]["kind"], "AgentCli");
        assert_eq!(staged_value["report"]["status"], "WaitingApproval");
        assert_eq!(staged_value["task"]["status"], "WaitingApproval");
        assert!(std::fs::read_to_string(&transcript_path)
            .unwrap()
            .contains("\"execution_request\""));

        let approved = server
            .handle_request("POST", &format!("/api/tasks/approve?task_id={task_id}"))
            .unwrap();
        let approved_value: serde_json::Value = serde_json::from_str(&approved.body).unwrap();

        assert_eq!(approved.status_code, 200);
        assert_eq!(approved_value["report"]["kind"], "AgentCli");
        assert_eq!(approved_value["report"]["status"], "Succeeded");
        assert_eq!(
            approved_value["task"]["id"].as_str(),
            Some(task_id.as_str())
        );
        assert_eq!(approved_value["task"]["status"], "Succeeded");
        assert!(approved_value["report"]["execution"]["stdout"]
            .as_str()
            .unwrap()
            .contains("approved-agent-ok"));
        assert_eq!(approved_value["snapshot"]["stats"]["waiting_approval"], 0);
        let transcript = std::fs::read_to_string(&transcript_path).unwrap();
        assert!(transcript.contains("\"resume_reason\": \"approval\""));
        assert!(transcript.contains("approved-agent-ok"));
    }

    #[test]
    fn approve_task_without_execution_request_only_releases_agent_session() {
        let db_path = temp_db_path("runtime-http-agent-cli-approve-stage-only");
        let control_dir = temp_control_dir("runtime-http-agent-cli-approve-stage-only");
        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));

        let staged = server
            .handle_request_with_body(
                "POST",
                "/api/agent-sessions",
                &format!(
                    r#"{{
                    "kind":"agent_cli",
                    "project_slug":"demo",
                    "command_id":"echo",
                    "title":"Stage only Agent CLI echo",
                    "command":"/bin/echo should-not-run",
                    "tools":["cli"],
                    "token_budget":1,
                    "control_dir":"{}"
                }}"#,
                    control_dir.display()
                ),
            )
            .unwrap();
        let staged_value: serde_json::Value = serde_json::from_str(&staged.body).unwrap();
        let task_id = staged_value["task"]["id"].as_str().unwrap().to_string();
        let transcript_path = staged_value["report"]["transcript_path"]
            .as_str()
            .unwrap()
            .to_string();

        assert_eq!(staged.status_code, 201);
        assert_eq!(staged_value["report"]["status"], "WaitingApproval");
        assert!(!std::fs::read_to_string(&transcript_path)
            .unwrap()
            .contains("\"execution_request\""));

        let approved = server
            .handle_request("POST", &format!("/api/tasks/approve?task_id={task_id}"))
            .unwrap();
        let approved_value: serde_json::Value = serde_json::from_str(&approved.body).unwrap();

        assert_eq!(approved.status_code, 200);
        assert!(approved_value.get("report").is_none());
        assert_eq!(approved_value["task"]["status"], "Ready");
        assert_eq!(approved_value["snapshot"]["stats"]["waiting_approval"], 0);
        assert!(!std::fs::read_to_string(&transcript_path)
            .unwrap()
            .contains("resume_reason"));
    }

    #[test]
    fn post_provider_run_executes_mock_3dgs_and_indexes_assets() {
        let db_path = temp_db_path("runtime-http-provider-run");
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        let plan = build_default_content_burst_plan("demo", "Pool HTTP demo");
        repository.persist_plan(&plan).unwrap();
        drop(repository);

        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let response = server
            .handle_request_with_body(
                "POST",
                "/api/provider-runs",
                r#"{
                    "provider_id":"world-labs-marble",
                    "task_title":"World Labs Marble local run",
                    "node_id":"node-3dgs",
                    "prompt":"generate neon bazaar world",
                    "input_paths":["worlds/demo/source/0-plate.png"],
                    "output_dir":"worlds/demo/output",
                    "requires_approval":false
                }"#,
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 200);
        assert_eq!(value["report"]["status"], "Succeeded");
        assert_eq!(value["report"]["provider_id"], "worldlabs-marble");
        assert_eq!(value["report"]["assets"].as_array().unwrap().len(), 3);
        assert_eq!(value["snapshot"]["stats"]["assets"], 3);
        assert!(value["snapshot"]["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|event| event["message"]
                .as_str()
                .unwrap_or_default()
                .contains("provider task succeeded")));
    }

    #[test]
    fn get_production_evidence_item_from_provider_ledger_builds_submit_item() {
        let db_path = temp_db_path("runtime-http-provider-ledger-evidence-item");
        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let response = server
            .handle_request_with_body(
                "POST",
                "/api/provider-runs",
                r#"{
                    "provider_id":"world-labs-marble",
                    "task_title":"World Labs Marble local ledger run",
                    "node_id":"node-3dgs",
                    "prompt":"generate ledger world",
                    "output_dir":"target/runtime-http-provider-ledger-evidence-item/worlds/demo/output",
                    "requires_approval":false
                }"#,
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();
        let provider_request_id = value["provider_request_id"].as_str().unwrap();

        let evidence = server
            .handle_path(&format!(
                "/api/production-evidence/item-from-ledger?provider_request_id={provider_request_id}&source=ledger-test"
            ))
            .unwrap();
        let evidence_value: serde_json::Value = serde_json::from_str(&evidence.body).unwrap();

        assert_eq!(evidence.status_code, 200);
        assert_eq!(
            evidence_value["kind"],
            "pool_production_evidence_item_from_ledger"
        );
        assert_eq!(evidence_value["item"]["kind"], "provider");
        assert_eq!(
            evidence_value["item"]["provider"]["provider_id"],
            "worldlabs-marble"
        );
        assert_eq!(
            evidence_value["item"]["provider"]["artifacts"]
                .as_array()
                .unwrap()
                .len(),
            3
        );
        assert_eq!(evidence_value["validation"]["valid"], false);
        assert!(evidence_value["validation"]["message"]
            .as_str()
            .unwrap()
            .contains("production_attestation"));
        assert_eq!(
            evidence_value["validation"]["artifact_files"]["complete"],
            false
        );
        assert_eq!(
            evidence_value["validation"]["production_flags"]["complete"],
            false
        );
        assert_eq!(evidence_value["ready_for_import"], false);
    }

    #[test]
    fn post_production_evidence_imports_provider_software_and_desktop_flags() {
        let db_path = temp_db_path("runtime-http-production-evidence");
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        let plan = build_default_content_burst_plan("demo", "Pool production evidence demo");
        repository.persist_plan(&plan).unwrap();
        drop(repository);

        let artifact_root = temp_control_dir("runtime-http-production-evidence-artifacts");
        let provider_artifact = write_temp_artifact(&artifact_root, "provider/marble.glb");
        let software_artifact = write_temp_artifact(&artifact_root, "software/unreal-level.json");
        let trace_path = write_temp_artifact(&artifact_root, "desktop/vision-trace.json");
        let body = json!({
            "project_slug": "demo",
            "source": "server-test",
            "providers": [{
                "provider_id": "worldlabs-marble",
                "external_job_id": "marble-real-1",
                "endpoint": "https://worker.example.test/worldlabs-marble",
                "family": "3dgs",
                "production_attestation": "worldlabs-marble-worker-prod-run-1",
                "artifacts": [provider_artifact],
            }],
            "software_actions": [{
                "adapter_id": "unreal",
                "external_action_id": "unreal-real-1",
                "production_attestation": "unreal-software-run-1",
                "action_kind": "CreateScene",
                "priority": "ApiMcp",
                "control_profile": "api_mcp",
                "artifacts": [software_artifact],
            }],
            "desktop_vision": [{
                "adapter_id": "touchdesigner",
                "external_action_id": "vision-real-1",
                "controller_id": "external-vision",
                "production_attestation": "external-vision-controller-run-1",
                "trace_path": trace_path,
                "visual_model": "external",
            }],
        })
        .to_string();
        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let response = server
            .handle_request_with_body("POST", "/api/production-evidence", &body)
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 201);
        assert_eq!(value["artifact_files"]["complete"], true);
        assert_eq!(value["summary"]["providers"], 1);
        assert_eq!(value["summary"]["software_actions"], 1);
        assert_eq!(value["summary"]["desktop_vision"], 1);
        assert_eq!(value["coverage"]["complete"], false);
        assert_eq!(value["coverage"]["providers"]["covered"], 1);
        assert_eq!(value["coverage"]["software_actions"]["covered"], 1);
        assert!(value["coverage"]["providers"]["missing"]
            .as_array()
            .unwrap()
            .iter()
            .any(|provider| provider == "midjourney"));
        let provider = value["prd_readiness"]["requirements"]
            .as_array()
            .unwrap()
            .iter()
            .find(|requirement| requirement["id"] == "ai_media_and_3dgs_providers")
            .unwrap();
        assert!(provider["evidence"]["provider_evidence"]["providers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["provider_id"] == "worldlabs-marble"
                && row["production_upstream_success"] == true));
        let software = value["prd_readiness"]["requirements"]
            .as_array()
            .unwrap()
            .iter()
            .find(|requirement| requirement["id"] == "external_software_control")
            .unwrap();
        assert!(
            software["evidence"]["software_evidence"]["adapters"]
                .as_array()
                .unwrap()
                .iter()
                .any(|row| row["adapter_id"] == "unreal"
                    && row["production_software_success"] == true)
        );
        let hardening = value["prd_readiness"]["requirements"]
            .as_array()
            .unwrap()
            .iter()
            .find(|requirement| requirement["id"] == "production_hardening")
            .unwrap();
        assert_eq!(
            hardening["evidence"]["desktop_vision_evidence"]["summary"]
                ["external_visual_model_ready"],
            true
        );
    }

    #[test]
    fn get_production_evidence_bundle_from_ledger_builds_ready_bundle() {
        let db_path = temp_db_path("runtime-http-ledger-evidence-bundle");
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        let plan = build_default_content_burst_plan("demo", "Pool ledger evidence bundle");
        repository.persist_plan(&plan).unwrap();
        drop(repository);

        let artifact_root = temp_control_dir("runtime-http-ledger-evidence-bundle-artifacts");
        let provider_artifact = write_temp_artifact(&artifact_root, "provider/marble.glb");
        let software_artifact = write_temp_artifact(&artifact_root, "software/unreal-level.json");
        let trace_path = write_temp_artifact(&artifact_root, "desktop/vision-trace.json");
        let body = json!({
            "project_slug": "demo",
            "source": "server-test",
            "providers": [{
                "provider_id": "worldlabs-marble",
                "external_job_id": "marble-ledger-real-1",
                "endpoint": "https://worker.example.test/worldlabs-marble",
                "family": "3dgs",
                "production_attestation": "worldlabs-marble-worker-ledger-run-1",
                "artifacts": [provider_artifact],
            }],
            "software_actions": [{
                "adapter_id": "unreal",
                "external_action_id": "unreal-ledger-real-1",
                "production_attestation": "unreal-software-ledger-run-1",
                "action_kind": "CreateScene",
                "priority": "ApiMcp",
                "control_profile": "api_mcp",
                "artifacts": [software_artifact],
            }],
            "desktop_vision": [{
                "adapter_id": "touchdesigner",
                "external_action_id": "vision-ledger-real-1",
                "controller_id": "external-vision",
                "production_attestation": "external-vision-controller-ledger-run-1",
                "trace_path": trace_path,
                "visual_model": "external",
            }],
        })
        .to_string();
        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let import_response = server
            .handle_request_with_body("POST", "/api/production-evidence", &body)
            .unwrap();
        assert_eq!(import_response.status_code, 201);

        let response = server
            .handle_path(
                "/api/production-evidence/bundle-from-ledger?source=ledger-bundle-test&include_incomplete=true",
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 200);
        assert_eq!(value["kind"], "pool_production_evidence_bundle_from_ledger");
        assert_eq!(value["ready_for_import"], true);
        assert_eq!(value["summary"]["providers"], 1);
        assert_eq!(value["summary"]["software_actions"], 1);
        assert_eq!(value["summary"]["desktop_vision"], 1);
        assert_eq!(value["summary"]["ready_items"], 3);
        assert_eq!(value["summary"]["incomplete_items"], 0);
        assert_eq!(
            value["bundle"]["providers"][0]["external_job_id"],
            "marble-ledger-real-1"
        );
        assert_eq!(
            value["bundle"]["software_actions"][0]["external_action_id"],
            "unreal-ledger-real-1"
        );
        assert_eq!(
            value["bundle"]["desktop_vision"][0]["external_action_id"],
            "vision-ledger-real-1"
        );
        assert_eq!(value["validation"]["valid"], true);
        assert_eq!(value["validation"]["artifact_files"]["complete"], true);
        assert_eq!(value["items"].as_array().unwrap().len(), 3);
        assert_eq!(value["incomplete_items"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn post_production_evidence_item_imports_single_provider_item() {
        let db_path = temp_db_path("runtime-http-production-evidence-item");
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        let plan = build_default_content_burst_plan("demo", "Pool production evidence item");
        repository.persist_plan(&plan).unwrap();
        drop(repository);

        let artifact_root = temp_control_dir("runtime-http-production-evidence-item-artifacts");
        let provider_artifact = write_temp_artifact(&artifact_root, "provider/midjourney.png");
        let provider_metadata =
            write_temp_artifact(&artifact_root, "provider/midjourney-request.json");
        let body = json!({
            "project_slug": "demo",
            "source": "single-item-test",
            "kind": "provider",
            "provider": {
                "provider_id": "midjourney",
                "external_job_id": "mj-real-single-1",
                "production_attestation": "midjourney-worker-single-run-1",
                "metadata_path": provider_metadata,
                "artifacts": [provider_artifact],
                "evidence_json": {
                    "operator": "provider-worker"
                }
            }
        })
        .to_string();
        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let response = server
            .handle_request_with_body("POST", "/api/production-evidence/items", &body)
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 201);
        assert_eq!(value["kind"], "pool_production_evidence_import");
        assert_eq!(value["artifact_files"]["complete"], true);
        assert_eq!(value["summary"]["providers"], 1);
        assert_eq!(value["summary"]["software_actions"], 0);
        assert_eq!(value["summary"]["desktop_vision"], 0);
        assert_eq!(value["providers"][0]["provider_id"], "midjourney");
        assert_eq!(value["coverage"]["providers"]["covered"], 1);
        let provider = value["prd_readiness"]["requirements"]
            .as_array()
            .unwrap()
            .iter()
            .find(|requirement| requirement["id"] == "ai_media_and_3dgs_providers")
            .unwrap();
        assert!(provider["evidence"]["provider_evidence"]["providers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["provider_id"] == "midjourney"
                && row["production_upstream_success"] == true));
    }

    #[test]
    fn post_production_evidence_item_validate_reports_without_writes() {
        let db_path = temp_db_path("runtime-http-production-evidence-item-validate");
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        let plan =
            build_default_content_burst_plan("demo", "Pool production evidence item validate");
        repository.persist_plan(&plan).unwrap();
        assert_eq!(repository.table_count("provider_requests").unwrap(), 0);
        drop(repository);

        let artifact_root = temp_control_dir("runtime-http-production-evidence-item-validate");
        let provider_artifact = write_temp_artifact(&artifact_root, "provider/midjourney.png");
        let provider_metadata =
            write_temp_artifact(&artifact_root, "provider/midjourney-request.json");
        let body = json!({
            "project_slug": "demo",
            "source": "single-item-validate-test",
            "kind": "provider",
            "provider": {
                "provider_id": "midjourney",
                "external_job_id": "mj-real-single-validate-1",
                "production_attestation": "midjourney-worker-single-validate-run-1",
                "metadata_path": provider_metadata,
                "artifacts": [provider_artifact],
                "evidence_json": {
                    "production_upstream": true,
                    "local_mock_gateway": false
                }
            }
        })
        .to_string();
        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let response = server
            .handle_request_with_body("POST", "/api/production-evidence/items/validate", &body)
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 200);
        assert_eq!(value["kind"], "pool_production_evidence_item_validation");
        assert_eq!(value["writes"], 0);
        assert_eq!(
            value["validation"]["kind"],
            "pool_production_evidence_validation"
        );
        assert_eq!(value["validation"]["valid"], true);
        assert_eq!(value["validation"]["summary"]["providers"], 1);
        assert_eq!(value["validation"]["artifact_files"]["complete"], true);
        assert_eq!(
            value["mcp"]["validate_item_tool"],
            "pool_validate_production_evidence_item"
        );

        let repository = RuntimeRepository::open(&db_path).unwrap();
        assert_eq!(repository.table_count("provider_requests").unwrap(), 0);
    }

    #[test]
    fn post_production_evidence_item_rejects_missing_item() {
        let db_path = temp_db_path("runtime-http-production-evidence-item-invalid");
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        let plan = build_default_content_burst_plan("demo", "Pool production evidence item");
        repository.persist_plan(&plan).unwrap();
        drop(repository);

        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let response = server
            .handle_request_with_body(
                "POST",
                "/api/production-evidence/items",
                r#"{"project_slug":"demo","kind":"provider"}"#,
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 400);
        assert_eq!(value["error"], "invalid_production_evidence_item_request");
        assert!(value["message"]
            .as_str()
            .unwrap_or_default()
            .contains("requires provider"));
    }

    #[test]
    fn post_production_evidence_import_canonicalizes_aliases_for_prd_readiness() {
        let db_path = temp_db_path("runtime-http-production-evidence-aliases");
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        let plan = build_default_content_burst_plan("demo", "Pool production evidence aliases");
        repository.persist_plan(&plan).unwrap();
        drop(repository);

        let artifact_root = temp_control_dir("runtime-http-production-evidence-alias-artifacts");
        let provider_artifact = write_temp_artifact(&artifact_root, "provider/marble.glb");
        let software_artifact = write_temp_artifact(&artifact_root, "software/resolve-master.mov");
        let body = json!({
            "project_slug": "demo",
            "source": "server-test",
            "providers": [{
                "provider_id": "world-labs-marble",
                "external_job_id": "marble-real-alias-1",
                "endpoint": "https://worker.example.test/worldlabs-marble",
                "family": "3dgs",
                "production_attestation": "worldlabs-marble-worker-alias-run-1",
                "artifacts": [provider_artifact],
            }],
            "software_actions": [{
                "adapter_id": "davinci-resolve",
                "external_action_id": "resolve-real-alias-1",
                "production_attestation": "resolve-software-alias-run-1",
                "action_kind": "Render",
                "priority": "ApiMcp",
                "control_profile": "api_mcp",
                "artifacts": [software_artifact],
            }],
        })
        .to_string();
        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let response = server
            .handle_request_with_body("POST", "/api/production-evidence", &body)
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 201);
        assert_eq!(value["providers"][0]["provider_id"], "worldlabs-marble");
        assert_eq!(
            value["providers"][0]["input_provider_id"],
            "world-labs-marble"
        );
        assert_eq!(value["software_actions"][0]["adapter_id"], "resolve");
        assert_eq!(
            value["software_actions"][0]["input_adapter_id"],
            "davinci-resolve"
        );

        let provider = value["prd_readiness"]["requirements"]
            .as_array()
            .unwrap()
            .iter()
            .find(|requirement| requirement["id"] == "ai_media_and_3dgs_providers")
            .unwrap();
        assert!(provider["evidence"]["provider_evidence"]["providers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["provider_id"] == "worldlabs-marble"
                && row["production_upstream_success"] == true));
        let software = value["prd_readiness"]["requirements"]
            .as_array()
            .unwrap()
            .iter()
            .find(|requirement| requirement["id"] == "external_software_control")
            .unwrap();
        assert!(software["evidence"]["software_evidence"]["adapters"]
            .as_array()
            .unwrap()
            .iter()
            .any(
                |row| row["adapter_id"] == "resolve" && row["production_software_success"] == true
            ));
    }

    #[test]
    fn post_production_evidence_import_reports_complete_prd_coverage_for_example() {
        let db_path = temp_db_path("runtime-http-production-evidence-import-complete");
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        let plan = build_default_content_burst_plan("demo", "Pool production evidence demo");
        repository.persist_plan(&plan).unwrap();
        drop(repository);

        let body = production_evidence_example_with_temp_files(
            "runtime-http-production-evidence-import-complete-artifacts",
        );
        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        seed_prd_readiness_baseline_evidence(
            &server,
            "runtime-http-production-evidence-import-complete-baseline",
        );
        let response = server
            .handle_request_with_body("POST", "/api/production-evidence", &body)
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 201);
        assert_eq!(value["kind"], "pool_production_evidence_import");
        assert_eq!(value["summary"]["providers"], 9);
        assert_eq!(value["summary"]["software_actions"], 11);
        assert_eq!(value["summary"]["desktop_vision"], 1);
        assert_eq!(value["artifact_files"]["complete"], true);
        assert!(value["artifact_files"]["checks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|check| check["kind"] == "provider_metadata"));
        assert_eq!(value["coverage"]["complete"], true);
        assert_eq!(
            value["coverage"]["would_satisfy_prd_production_evidence"],
            true
        );
        assert_eq!(value["coverage"]["providers"]["covered"], 9);
        assert_eq!(value["coverage"]["software_actions"]["covered"], 11);
        assert_eq!(value["coverage"]["desktop_vision"]["complete"], true);
        assert_eq!(
            value["coverage"]["desktop_vision"]["external_visual_model_count"],
            1
        );
        assert!(value["coverage"]["providers"]["missing"]
            .as_array()
            .unwrap()
            .is_empty());
        assert!(value["coverage"]["software_actions"]["missing"]
            .as_array()
            .unwrap()
            .is_empty());
        assert_eq!(value["prd_readiness"]["overall_status"], "ready");
        assert_eq!(value["prd_readiness"]["summary"]["ready"], 10);
        assert_eq!(value["prd_readiness"]["summary"]["partial"], 0);
        assert_eq!(value["prd_readiness"]["summary"]["blocked"], 0);
        assert_eq!(
            value["prd_readiness"]["completion_gate"]["status"],
            "complete"
        );
        assert_eq!(
            value["prd_readiness"]["completion_gate"]["ready_for_completion"],
            true
        );
        assert_eq!(
            value["prd_readiness"]["completion_gate"]["completion_is_proven_by_current_snapshot"],
            true
        );
        assert!(
            value["prd_readiness"]["completion_gate"]["incomplete_requirements"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert!(value["prd_readiness"]["requirements"]
            .as_array()
            .unwrap()
            .iter()
            .all(|requirement| requirement["status"] == "ready"));
    }

    #[test]
    fn get_production_evidence_template_returns_full_scaffold() {
        let db_path = temp_db_path("runtime-http-production-evidence-template");
        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let response = server
            .handle_path("/api/production-evidence/template?output_root=target/prod")
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 200);
        assert_eq!(value["kind"], "pool_production_evidence_bundle_template");
        assert_eq!(value["project_slug"], "demo");
        assert_eq!(value["ready_for_import"], false);
        assert_eq!(value["scope"]["mode"], "full");
        assert_eq!(value["scope"]["missing_only"], false);
        assert_eq!(value["bundle"]["providers"].as_array().unwrap().len(), 9);
        assert_eq!(
            value["bundle"]["software_actions"]
                .as_array()
                .unwrap()
                .len(),
            11
        );
        assert_eq!(
            value["bundle"]["desktop_vision"].as_array().unwrap().len(),
            1
        );
        assert!(value["bundle"]["providers"][0]["metadata_path"]
            .as_str()
            .unwrap()
            .starts_with("target/prod/worlds/demo/output/production/"));
        let software_actions = value["bundle"]["software_actions"].as_array().unwrap();
        for adapter_id in [
            "blender",
            "comfyui",
            "resolve",
            "nuke",
            "motion-db",
            "editing-suite",
        ] {
            let action = software_actions
                .iter()
                .find(|action| action["adapter_id"] == adapter_id)
                .unwrap_or_else(|| panic!("missing software template for {adapter_id}"));
            assert_eq!(action["priority"], "ApiMcp");
            assert_eq!(action["control_profile"], "api_mcp");
        }
        let touchdesigner = software_actions
            .iter()
            .find(|action| action["adapter_id"] == "touchdesigner")
            .expect("missing touchdesigner software template");
        assert_eq!(touchdesigner["priority"], "DesktopRecognition");
        assert_eq!(touchdesigner["control_profile"], "desktop_recognition");
        assert!(value["operator_checklist"].as_array().unwrap().len() >= 5);
        assert_eq!(
            value["commands"]["validate"],
            "pool-cli --project demo validate-production-evidence <bundle.json>"
        );

        let rejected = server
            .handle_request_with_body(
                "POST",
                "/api/production-evidence/validate",
                &serde_json::to_string(&value["bundle"]).unwrap(),
            )
            .unwrap();
        let rejected_value: serde_json::Value = serde_json::from_str(&rejected.body).unwrap();
        assert_eq!(rejected.status_code, 400);
        assert_eq!(rejected_value["error"], "invalid_production_evidence_item");
    }

    #[test]
    fn get_production_evidence_template_can_return_missing_only_scaffold() {
        let db_path = temp_db_path("runtime-http-production-evidence-template-missing-only");
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        let plan = build_default_content_burst_plan("demo", "Pool production evidence template");
        repository.persist_plan(&plan).unwrap();
        drop(repository);

        let artifact_root =
            temp_control_dir("runtime-http-production-evidence-template-missing-only");
        fs::create_dir_all(&artifact_root).unwrap();
        let provider_artifact = artifact_root.join("midjourney.png");
        let provider_metadata = artifact_root.join("midjourney-request.json");
        fs::write(&provider_artifact, "provider").unwrap();
        fs::write(&provider_metadata, "{}").unwrap();
        let software_artifact = write_temp_artifact(&artifact_root, "software/unreal-level.json");
        let body = json!({
            "source": "missing-only-template-test",
            "providers": [{
                "provider_id": "midjourney",
                "external_job_id": "mj-real-job",
                "production_attestation": "midjourney-worker-missing-only-run-1",
                "metadata_path": provider_metadata,
                "artifacts": [provider_artifact],
                "evidence_json": {
                    "production_upstream": true,
                    "local_mock_gateway": false
                }
            }],
            "software_actions": [{
                "adapter_id": "unreal",
                "external_action_id": "unreal-real-action",
                "production_attestation": "unreal-software-missing-only-run-1",
                "action_kind": "CreateScene",
                "priority": "ApiMcp",
                "artifacts": [software_artifact],
                "verification_json": { "ok": true },
                "evidence_json": {
                    "production_software": true,
                    "local_mock_software": false
                }
            }]
        })
        .to_string();
        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let import = server
            .handle_request_with_body("POST", "/api/production-evidence", &body)
            .unwrap();
        assert_eq!(import.status_code, 201);

        let response = server
            .handle_path("/api/production-evidence/template?missing_only=true&project=demo")
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 200);
        assert_eq!(value["scope"]["mode"], "missing_only");
        assert_eq!(value["scope"]["missing_only"], true);
        assert_eq!(value["bundle"]["providers"].as_array().unwrap().len(), 8);
        assert_eq!(
            value["bundle"]["software_actions"]
                .as_array()
                .unwrap()
                .len(),
            10
        );
        assert_eq!(
            value["bundle"]["desktop_vision"].as_array().unwrap().len(),
            1
        );
        assert!(!value["bundle"]["providers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|provider| provider["provider_id"] == "midjourney"));
        assert!(!value["bundle"]["software_actions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|action| action["adapter_id"] == "unreal"));
    }

    #[test]
    fn post_production_evidence_validate_reports_counts_without_writes() {
        let db_path = temp_db_path("runtime-http-production-evidence-validate");
        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let response = server
            .handle_request_with_body(
                "POST",
                "/api/production-evidence/validate",
                r#"{
                    "project_slug":"demo",
                    "source":"server-test",
                    "providers":[
                        {
                            "provider_id":"world-labs-marble",
                            "external_job_id":"marble-real-1",
                            "endpoint":"https://worker.example.test/worldlabs-marble",
                            "family":"3dgs",
                            "production_attestation":"worldlabs-marble-worker-validate-run-1",
                            "artifacts":["worlds/demo/output/production/marble.glb"]
                        }
                    ],
                    "software_actions":[
                        {
                            "adapter_id":"davinci-resolve",
                            "external_action_id":"unreal-real-1",
                            "production_attestation":"resolve-software-validate-run-1",
                            "action_kind":"CreateScene",
                            "priority":"ApiMcp",
                            "control_profile":"api_mcp",
                            "artifacts":["unreal://project/demo/level/real"]
                        }
                    ],
                    "desktop_vision":[
                        {
                            "adapter_id":"touchdesigner",
                            "external_action_id":"vision-real-1",
                            "controller_id":"external-vision",
                            "production_attestation":"external-vision-controller-validate-run-1",
                            "trace_path":"worlds/demo/output/production/vision-trace.json",
                            "visual_model":"external"
                        }
                    ]
                }"#,
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 200);
        assert_eq!(value["kind"], "pool_production_evidence_validation");
        assert_eq!(value["valid"], true);
        assert_eq!(value["writes"], 0);
        assert_eq!(value["summary"]["providers"], 1);
        assert_eq!(value["summary"]["software_actions"], 1);
        assert_eq!(value["summary"]["desktop_vision"], 1);
        assert_eq!(value["coverage"]["complete"], false);
        assert_eq!(value["artifact_files"]["complete"], false);
        assert!(value["artifact_files"]["missing"]
            .as_array()
            .unwrap()
            .iter()
            .any(|path| path == "worlds/demo/output/production/marble.glb"));
        assert_eq!(value["coverage"]["providers"]["complete"], false);
        assert!(value["coverage"]["providers"]["missing"]
            .as_array()
            .unwrap()
            .iter()
            .any(|provider| provider == "midjourney"));
        assert_eq!(value["coverage"]["desktop_vision"]["complete"], true);
        assert_eq!(
            value["coverage"]["desktop_vision"]["external_visual_model_count"],
            1
        );
        assert_eq!(value["providers"][0]["provider_id"], "worldlabs-marble");
        assert_eq!(
            value["providers"][0]["input_provider_id"],
            "world-labs-marble"
        );
        assert_eq!(value["software_actions"][0]["adapter_id"], "resolve");
        assert_eq!(
            value["software_actions"][0]["input_adapter_id"],
            "davinci-resolve"
        );
        assert_eq!(
            value["desktop_vision"][0]["controller_id"],
            "external-vision"
        );

        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        assert_eq!(repository.table_count("tasks").unwrap(), 0);
        assert_eq!(repository.table_count("provider_requests").unwrap(), 0);
        assert_eq!(repository.table_count("software_actions").unwrap(), 0);
        assert_eq!(repository.table_count("workflow_events").unwrap(), 0);
    }

    #[test]
    fn post_production_evidence_merge_combines_bundle_arrays_without_writes() {
        let db_path = temp_db_path("runtime-http-production-evidence-merge");
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        drop(repository);

        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let body = json!({
            "project_slug": "demo",
            "source": "agent-closeout",
            "bundles": [
                {
                    "project_slug": "demo",
                    "source": "provider-worker",
                    "providers": [{
                        "provider_id": "worldlabs-marble",
                        "external_job_id": "marble-real-merge-1",
                        "production_attestation": "worldlabs-marble-worker-merge-run-1",
                        "artifacts": ["worlds/demo/output/production/marble.glb"]
                    }]
                },
                {
                    "project_slug": "demo",
                    "source": "software-operator",
                    "software_actions": [{
                        "adapter_id": "unreal",
                        "external_action_id": "unreal-real-merge-1",
                        "production_attestation": "unreal-software-merge-run-1",
                        "action_kind": "CreateScene",
                        "priority": "ApiMcp"
                    }]
                },
                {
                    "project_slug": "demo",
                    "source": "vision-controller",
                    "desktop_vision": [{
                        "adapter_id": "touchdesigner",
                        "external_action_id": "vision-real-merge-1",
                        "controller_id": "external-vision",
                        "production_attestation": "external-vision-controller-merge-run-1",
                        "trace_path": "worlds/demo/output/production/vision-trace.json",
                        "visual_model": "external"
                    }]
                }
            ]
        })
        .to_string();
        let response = server
            .handle_request_with_body("POST", "/api/production-evidence/merge", &body)
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 200);
        assert_eq!(value["kind"], "pool_production_evidence_merge");
        assert_eq!(value["writes"], 0);
        assert_eq!(value["project_slug"], "demo");
        assert_eq!(value["source"], "agent-closeout");
        assert_eq!(value["summary"]["input_bundles"], 3);
        assert_eq!(value["summary"]["providers"], 1);
        assert_eq!(value["summary"]["software_actions"], 1);
        assert_eq!(value["summary"]["desktop_vision"], 1);
        assert_eq!(value["input_summaries"].as_array().unwrap().len(), 3);
        assert_eq!(value["bundle"]["providers"].as_array().unwrap().len(), 1);
        assert_eq!(
            value["bundle"]["software_actions"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            value["bundle"]["desktop_vision"].as_array().unwrap().len(),
            1
        );
        assert!(value["commands"]["closeout"]
            .as_str()
            .unwrap()
            .contains("closeout-production-evidence"));
        assert!(value["commands"]["validate"]
            .as_str()
            .unwrap()
            .contains("validate-production-evidence"));
        assert!(value["commands"]["import"]
            .as_str()
            .unwrap()
            .contains("import-production-evidence"));

        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        assert_eq!(repository.table_count("tasks").unwrap(), 0);
        assert_eq!(repository.table_count("provider_requests").unwrap(), 0);
        assert_eq!(repository.table_count("software_actions").unwrap(), 0);
        assert_eq!(repository.table_count("workflow_events").unwrap(), 0);
    }

    #[test]
    fn post_production_evidence_merge_rejects_project_conflict_without_writes() {
        let db_path = temp_db_path("runtime-http-production-evidence-merge-conflict");
        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let response = server
            .handle_request_with_body(
                "POST",
                "/api/production-evidence/merge",
                r#"{
                    "project_slug":"demo",
                    "bundles":[
                        {
                            "project_slug":"other",
                            "providers":[
                                {
                                    "provider_id":"worldlabs-marble",
                                    "external_job_id":"marble-real-merge-1",
                                    "artifacts":["worlds/demo/output/production/marble.glb"]
                                }
                            ]
                        }
                    ]
                }"#,
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 400);
        assert_eq!(value["error"], "invalid_production_evidence_merge_request");
        assert_eq!(value["writes"], 0);
        assert!(value["message"]
            .as_str()
            .unwrap_or_default()
            .contains("conflicting project_slug"));

        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        assert_eq!(repository.table_count("tasks").unwrap(), 0);
        assert_eq!(repository.table_count("provider_requests").unwrap(), 0);
        assert_eq!(repository.table_count("software_actions").unwrap(), 0);
        assert_eq!(repository.table_count("workflow_events").unwrap(), 0);
    }

    #[test]
    fn post_production_evidence_closeout_validates_merged_bundles_without_writes() {
        let db_path = temp_db_path("runtime-http-production-evidence-closeout");
        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let body = json!({
            "project_slug": "demo",
            "source": "agent-closeout",
            "bundles": [
                {
                    "project_slug": "demo",
                    "source": "provider-worker",
                    "providers": [{
                        "provider_id": "worldlabs-marble",
                        "external_job_id": "marble-real-closeout-1",
                        "production_attestation": "worldlabs-marble-worker-closeout-run-1",
                        "artifacts": ["worlds/demo/output/production/marble.glb"]
                    }]
                },
                {
                    "project_slug": "demo",
                    "source": "software-operator",
                    "software_actions": [{
                        "adapter_id": "unreal",
                        "external_action_id": "unreal-real-closeout-1",
                        "production_attestation": "unreal-software-closeout-run-1",
                        "action_kind": "CreateScene",
                        "priority": "ApiMcp",
                        "verification_json": { "ok": true }
                    }]
                },
                {
                    "project_slug": "demo",
                    "source": "vision-controller",
                    "desktop_vision": [{
                        "adapter_id": "touchdesigner",
                        "external_action_id": "vision-real-closeout-1",
                        "controller_id": "external-vision",
                        "production_attestation": "external-vision-controller-closeout-run-1",
                        "trace_path": "worlds/demo/output/production/vision-trace.json",
                        "visual_model": "external"
                    }]
                }
            ]
        })
        .to_string();
        let response = server
            .handle_request_with_body("POST", "/api/production-evidence/closeout", &body)
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 200);
        assert_eq!(value["kind"], "pool_production_evidence_closeout");
        assert_eq!(value["mode"], "validate");
        assert_eq!(value["writes"], 0);
        assert_eq!(value["ready_for_import"], false);
        assert_eq!(value["merge"]["summary"]["input_bundles"], 3);
        assert_eq!(value["merge"]["summary"]["providers"], 1);
        assert_eq!(value["merge"]["summary"]["software_actions"], 1);
        assert_eq!(value["merge"]["summary"]["desktop_vision"], 1);
        assert_eq!(value["validation"]["summary"]["providers"], 1);
        assert_eq!(value["validation"]["summary"]["software_actions"], 1);
        assert_eq!(value["validation"]["summary"]["desktop_vision"], 1);
        assert_eq!(value["validation"]["artifact_files"]["complete"], false);
        assert!(value["commands"]["closeout"]
            .as_str()
            .unwrap()
            .contains("closeout-production-evidence"));
        assert!(value["commands"]["import"]
            .as_str()
            .unwrap()
            .contains("import-production-evidence"));
        assert!(value["commands"]["validate"]
            .as_str()
            .unwrap()
            .contains("validate-production-evidence"));

        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        assert_eq!(repository.table_count("tasks").unwrap(), 0);
        assert_eq!(repository.table_count("provider_requests").unwrap(), 0);
        assert_eq!(repository.table_count("software_actions").unwrap(), 0);
        assert_eq!(repository.table_count("workflow_events").unwrap(), 0);
    }

    #[test]
    fn post_production_evidence_closeout_rejects_project_conflict_without_writes() {
        let db_path = temp_db_path("runtime-http-production-evidence-closeout-conflict");
        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let response = server
            .handle_request_with_body(
                "POST",
                "/api/production-evidence/closeout",
                r#"{
                    "project_slug":"demo",
                    "bundles":[
                        {
                            "project_slug":"other",
                            "providers":[
                                {
                                    "provider_id":"worldlabs-marble",
                                    "external_job_id":"marble-real-closeout-1",
                                    "artifacts":["worlds/demo/output/production/marble.glb"]
                                }
                            ]
                        }
                    ]
                }"#,
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 400);
        assert_eq!(
            value["error"],
            "invalid_production_evidence_closeout_request"
        );
        assert_eq!(value["writes"], 0);
        assert!(value["message"]
            .as_str()
            .unwrap_or_default()
            .contains("conflicting project_slug"));

        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        assert_eq!(repository.table_count("tasks").unwrap(), 0);
        assert_eq!(repository.table_count("provider_requests").unwrap(), 0);
        assert_eq!(repository.table_count("software_actions").unwrap(), 0);
        assert_eq!(repository.table_count("workflow_events").unwrap(), 0);
    }

    #[test]
    fn post_production_evidence_closeout_import_reuses_artifact_file_gate() {
        let db_path = temp_db_path("runtime-http-production-evidence-closeout-import-gate");
        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let response = server
            .handle_request_with_body(
                "POST",
                "/api/production-evidence/closeout",
                r#"{
                    "project_slug":"demo",
                    "source":"agent-closeout",
                    "import":true,
                    "bundles":[
                        {
                            "project_slug":"demo",
                            "providers":[
                                {
                                    "provider_id":"worldlabs-marble",
                                    "external_job_id":"marble-real-closeout-2",
                                    "production_attestation":"worldlabs-marble-worker-closeout-run-2",
                                    "artifacts":["worlds/demo/output/production/missing-marble.glb"]
                                }
                            ]
                        }
                    ]
                }"#,
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 400);
        assert_eq!(value["kind"], "pool_production_evidence_closeout");
        assert_eq!(value["mode"], "import");
        assert_eq!(value["writes"], 0);
        assert_eq!(value["error"], "production_evidence_closeout_import_failed");
        assert_eq!(
            value["import"]["error"],
            "missing_production_artifact_files"
        );
        assert_eq!(value["validation"]["artifact_files"]["complete"], false);

        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        assert_eq!(repository.table_count("tasks").unwrap(), 0);
        assert_eq!(repository.table_count("provider_requests").unwrap(), 0);
        assert_eq!(repository.table_count("software_actions").unwrap(), 0);
        assert_eq!(repository.table_count("workflow_events").unwrap(), 0);
    }

    #[test]
    fn post_production_evidence_closeout_import_reports_completion_gate() {
        let db_path = temp_db_path("runtime-http-production-evidence-closeout-import-complete");
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        let plan = build_default_content_burst_plan("demo", "Pool production closeout complete");
        repository.persist_plan(&plan).unwrap();
        drop(repository);

        let bundle: Value = serde_json::from_str(&production_evidence_example_with_temp_files(
            "runtime-http-production-evidence-closeout-import-complete-artifacts",
        ))
        .unwrap();
        let completion_output_dir =
            temp_control_dir("runtime-http-production-evidence-closeout-completion-package");
        let completion_output_dir_string = completion_output_dir.to_string_lossy().to_string();
        let body = json!({
            "project_slug": "demo",
            "source": "agent-closeout-complete",
            "import": true,
            "completion_package": {
                "node_id": "agent",
                "title": "Closeout PRD proof",
                "output_dir": completion_output_dir_string,
                "source": "closeout-test",
                "include_snapshot": true
            },
            "bundles": [bundle]
        });
        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        seed_prd_readiness_baseline_evidence(
            &server,
            "runtime-http-production-evidence-closeout-import-complete-baseline",
        );
        let response = server
            .handle_request_with_body(
                "POST",
                "/api/production-evidence/closeout",
                &body.to_string(),
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 201);
        assert_eq!(value["kind"], "pool_production_evidence_closeout");
        assert_eq!(value["mode"], "import");
        assert_eq!(value["writes"], 21);
        assert_eq!(value["ready_for_import"], true);
        assert_eq!(value["ready_for_completion"], true);
        assert_eq!(value["prd_overall_status"], "ready");
        assert_eq!(value["prd_summary"]["ready"], 10);
        assert_eq!(value["completion_gate"]["status"], "complete");
        assert_eq!(value["completion_gate"]["ready_for_completion"], true);
        assert_eq!(
            value["completion_gate"]["completion_is_proven_by_current_snapshot"],
            true
        );
        assert!(value["completion_gate"]["incomplete_requirements"]
            .as_array()
            .unwrap()
            .is_empty());
        assert_eq!(
            value["commands"]["completion_gate"],
            "pool-cli --project demo prd-completion-gate --require-complete"
        );
        assert_eq!(
            value["commands"]["completion_package"],
            "pool-cli --project demo prd-completion-package --output-dir worlds/demo/output --include-snapshot"
        );
        assert_eq!(
            value["import"]["prd_readiness"]["completion_gate"]["ready_for_completion"],
            true
        );
        assert_eq!(value["completion_package"]["requested"], true);
        assert_eq!(value["completion_package"]["written"], true);
        assert_eq!(value["completion_package"]["status_code"], 201);
        assert_eq!(
            value["completion_package"]["response"]["kind"],
            "pool_prd_completion_package"
        );
        assert_eq!(
            value["completion_package"]["response"]["report"]["ready_for_completion"],
            true
        );
        assert_eq!(
            value["completion_package"]["response"]["task"]["status"],
            "Succeeded"
        );
        assert!(PathBuf::from(
            value["completion_package"]["response"]["report"]["manifest_path"]
                .as_str()
                .unwrap()
        )
        .exists());

        let _ = fs::remove_dir_all(completion_output_dir);
    }

    #[test]
    fn post_production_evidence_validate_reports_complete_prd_coverage_for_example() {
        let db_path = temp_db_path("runtime-http-production-evidence-validate-complete");
        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let response = server
            .handle_request_with_body(
                "POST",
                "/api/production-evidence/validate",
                include_str!("../../../docs/examples/production-evidence-bundle.example.json"),
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 200);
        assert_eq!(value["kind"], "pool_production_evidence_validation");
        assert_eq!(value["writes"], 0);
        assert_eq!(value["summary"]["providers"], 9);
        assert_eq!(value["summary"]["software_actions"], 11);
        assert_eq!(value["summary"]["desktop_vision"], 1);
        assert_eq!(value["coverage"]["complete"], true);
        assert_eq!(value["artifact_files"]["complete"], false);
        assert_eq!(
            value["coverage"]["would_satisfy_prd_production_evidence"],
            true
        );
        assert_eq!(value["coverage"]["providers"]["covered"], 9);
        assert_eq!(value["coverage"]["software_actions"]["covered"], 11);
        assert_eq!(value["coverage"]["desktop_vision"]["complete"], true);
        assert_eq!(
            value["coverage"]["desktop_vision"]["external_visual_model_count"],
            1
        );
        assert!(value["coverage"]["providers"]["missing"]
            .as_array()
            .unwrap()
            .is_empty());
        assert!(value["coverage"]["software_actions"]["missing"]
            .as_array()
            .unwrap()
            .is_empty());

        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        assert_eq!(repository.table_count("tasks").unwrap(), 0);
        assert_eq!(repository.table_count("provider_requests").unwrap(), 0);
        assert_eq!(repository.table_count("software_actions").unwrap(), 0);
    }

    #[test]
    fn post_production_evidence_validate_rejects_local_desktop_vision_without_writes() {
        let db_path = temp_db_path("runtime-http-production-evidence-validate-local-vision");
        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let response = server
            .handle_request_with_body(
                "POST",
                "/api/production-evidence/validate",
                r#"{
                    "project_slug":"demo",
                    "source":"server-test",
                    "desktop_vision":[
                        {
                            "adapter_id":"touchdesigner",
                            "external_action_id":"vision-real-1",
                            "controller_id":"local-vision-dry-run",
                            "production_attestation":"external-vision-controller-local-model-reject-run-1",
                            "trace_path":"worlds/demo/output/production/vision-trace.json",
                            "visual_model":"local-trace"
                        }
                    ]
                }"#,
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 400);
        assert_eq!(value["error"], "invalid_production_evidence_item");
        assert_eq!(value["writes"], 0);
        assert!(value["message"]
            .as_str()
            .unwrap_or_default()
            .contains("external visual model"));

        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        assert_eq!(repository.table_count("tasks").unwrap(), 0);
        assert_eq!(repository.table_count("provider_requests").unwrap(), 0);
        assert_eq!(repository.table_count("software_actions").unwrap(), 0);
        assert_eq!(repository.table_count("workflow_events").unwrap(), 0);
    }

    #[test]
    fn post_production_evidence_validate_rejects_invalid_bundle_without_writes() {
        let db_path = temp_db_path("runtime-http-production-evidence-validate-invalid");
        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let response = server
            .handle_request_with_body(
                "POST",
                "/api/production-evidence/validate",
                r#"{
                    "project_slug":"demo",
                    "source":"server-test",
                    "providers":[
                        {
                            "provider_id":"worldlabs-marble",
                            "external_job_id":"marble-real-1",
                            "production_attestation":"worldlabs-marble-worker-invalid-run-1",
                            "artifacts":["https://provider.example.test/marble.glb"]
                        }
                    ]
                }"#,
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 400);
        assert_eq!(value["error"], "invalid_production_evidence_item");
        assert_eq!(value["writes"], 0);

        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        assert_eq!(repository.table_count("tasks").unwrap(), 0);
        assert_eq!(repository.table_count("provider_requests").unwrap(), 0);
    }

    #[test]
    fn post_production_evidence_rejects_missing_local_artifact_files_without_writes() {
        let db_path = temp_db_path("runtime-http-production-evidence-missing-files");
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        let plan =
            build_default_content_burst_plan("demo", "Pool production evidence missing files");
        repository.persist_plan(&plan).unwrap();
        let tasks_before = repository.table_count("tasks").unwrap();
        let provider_requests_before = repository.table_count("provider_requests").unwrap();
        let software_actions_before = repository.table_count("software_actions").unwrap();
        let workflow_events_before = repository.table_count("workflow_events").unwrap();
        drop(repository);

        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let response = server
            .handle_request_with_body(
                "POST",
                "/api/production-evidence",
                r#"{
                    "project_slug":"demo",
                    "source":"server-test",
                    "providers":[
                        {
                            "provider_id":"worldlabs-marble",
                            "external_job_id":"marble-real-missing-file",
                            "production_attestation":"worldlabs-marble-worker-missing-file-run-1",
                            "artifacts":["target/missing-production-artifact/marble.glb"]
                        }
                    ]
                }"#,
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 400);
        assert_eq!(value["error"], "missing_production_artifact_files");
        assert_eq!(value["writes"], 0);
        assert_eq!(value["artifact_files"]["complete"], false);
        assert!(value["artifact_files"]["missing"]
            .as_array()
            .unwrap()
            .iter()
            .any(|path| path == "target/missing-production-artifact/marble.glb"));

        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        assert_eq!(repository.table_count("tasks").unwrap(), tasks_before);
        assert_eq!(
            repository.table_count("provider_requests").unwrap(),
            provider_requests_before
        );
        assert_eq!(
            repository.table_count("software_actions").unwrap(),
            software_actions_before
        );
        assert_eq!(
            repository.table_count("workflow_events").unwrap(),
            workflow_events_before
        );
    }

    #[test]
    fn post_production_evidence_rejects_missing_provider_metadata_without_writes() {
        let db_path = temp_db_path("runtime-http-production-evidence-missing-provider-metadata");
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        let plan = build_default_content_burst_plan(
            "demo",
            "Pool production evidence missing provider metadata",
        );
        repository.persist_plan(&plan).unwrap();
        let tasks_before = repository.table_count("tasks").unwrap();
        let provider_requests_before = repository.table_count("provider_requests").unwrap();
        let software_actions_before = repository.table_count("software_actions").unwrap();
        let workflow_events_before = repository.table_count("workflow_events").unwrap();
        drop(repository);

        let artifact_root =
            temp_control_dir("runtime-http-production-evidence-missing-provider-metadata");
        let provider_artifact = write_temp_artifact(&artifact_root, "provider/marble.glb");
        let missing_metadata = artifact_root
            .join("provider/.marble-request.json")
            .to_string_lossy()
            .into_owned();
        let body = json!({
            "project_slug": "demo",
            "source": "server-test",
            "providers": [{
                "provider_id": "worldlabs-marble",
                "external_job_id": "marble-real-missing-metadata",
                "production_attestation": "worldlabs-marble-worker-missing-metadata-run-1",
                "artifacts": [provider_artifact],
                "metadata_path": missing_metadata,
            }],
        })
        .to_string();
        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let response = server
            .handle_request_with_body("POST", "/api/production-evidence", &body)
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 400);
        assert_eq!(value["error"], "missing_production_artifact_files");
        assert_eq!(value["writes"], 0);
        assert_eq!(value["artifact_files"]["complete"], false);
        assert!(value["artifact_files"]["checks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|check| check["kind"] == "provider_metadata"
                && check["path"] == missing_metadata
                && check["exists"] == false));

        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        assert_eq!(repository.table_count("tasks").unwrap(), tasks_before);
        assert_eq!(
            repository.table_count("provider_requests").unwrap(),
            provider_requests_before
        );
        assert_eq!(
            repository.table_count("software_actions").unwrap(),
            software_actions_before
        );
        assert_eq!(
            repository.table_count("workflow_events").unwrap(),
            workflow_events_before
        );
    }

    #[test]
    fn post_production_evidence_rejects_missing_desktop_artifact_files_without_writes() {
        let db_path = temp_db_path("runtime-http-production-evidence-missing-desktop-artifacts");
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        let plan = build_default_content_burst_plan(
            "demo",
            "Pool production evidence missing desktop artifacts",
        );
        repository.persist_plan(&plan).unwrap();
        let tasks_before = repository.table_count("tasks").unwrap();
        let provider_requests_before = repository.table_count("provider_requests").unwrap();
        let software_actions_before = repository.table_count("software_actions").unwrap();
        let workflow_events_before = repository.table_count("workflow_events").unwrap();
        drop(repository);

        let artifact_root =
            temp_control_dir("runtime-http-production-evidence-missing-desktop-artifacts");
        let trace_path = write_temp_artifact(&artifact_root, "desktop/vision-trace.json");
        let missing_artifact = artifact_root
            .join("desktop/missing-capture.json")
            .to_string_lossy()
            .into_owned();
        let body = json!({
            "project_slug": "demo",
            "source": "server-test",
            "desktop_vision": [{
                "adapter_id": "touchdesigner",
                "external_action_id": "vision-real-missing-artifact",
                "controller_id": "external-vision",
                "production_attestation": "external-vision-controller-missing-artifact-run-1",
                "trace_path": trace_path,
                "visual_model": "external",
                "artifacts": [missing_artifact],
            }],
        })
        .to_string();
        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let response = server
            .handle_request_with_body("POST", "/api/production-evidence", &body)
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 400);
        assert_eq!(value["error"], "missing_production_artifact_files");
        assert_eq!(value["writes"], 0);
        assert_eq!(value["artifact_files"]["complete"], false);
        assert!(value["artifact_files"]["checks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|check| check["kind"] == "desktop_vision_artifact"
                && check["path"] == missing_artifact
                && check["exists"] == false));

        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        assert_eq!(repository.table_count("tasks").unwrap(), tasks_before);
        assert_eq!(
            repository.table_count("provider_requests").unwrap(),
            provider_requests_before
        );
        assert_eq!(
            repository.table_count("software_actions").unwrap(),
            software_actions_before
        );
        assert_eq!(
            repository.table_count("workflow_events").unwrap(),
            workflow_events_before
        );
    }

    #[test]
    fn post_production_evidence_rejects_template_identifiers() {
        let db_path = temp_db_path("runtime-http-production-evidence-placeholder");
        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let response = server
            .handle_request_with_body(
                "POST",
                "/api/production-evidence",
                r#"{
                    "project_slug":"demo",
                    "source":"server-test",
                    "providers":[
                        {
                            "provider_id":"worldlabs-marble",
                            "external_job_id":"replace-with-real-provider-job-id",
                            "artifacts":["worlds/demo/output/production/marble.glb"]
                        }
                    ]
                }"#,
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 400);
        assert_eq!(value["error"], "invalid_production_evidence_item");
        assert!(value["message"]
            .as_str()
            .unwrap_or_default()
            .contains("template placeholder"));
    }

    #[test]
    fn post_production_evidence_validate_rejects_web_template_identifiers() {
        let db_path = temp_db_path("runtime-http-production-evidence-web-template");
        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let response = server
            .handle_request_with_body(
                "POST",
                "/api/production-evidence/validate",
                r#"{
                    "project_slug":"demo",
                    "source":"server-test",
                    "providers":[
                        {
                            "provider_id":"worldlabs-marble",
                            "external_job_id":"web-prod-worldlabs-marble-job-001",
                            "artifacts":["worlds/demo/output/production/marble.glb"]
                        }
                    ]
                }"#,
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 400);
        assert_eq!(value["error"], "invalid_production_evidence_item");
        assert_eq!(value["writes"], 0);
        assert!(value["message"]
            .as_str()
            .unwrap_or_default()
            .contains("template placeholder"));
    }

    #[test]
    fn post_production_evidence_rejects_missing_provider_attestation() {
        let db_path = temp_db_path("runtime-http-production-evidence-missing-attestation");
        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let response = server
            .handle_request_with_body(
                "POST",
                "/api/production-evidence/validate",
                r#"{
                    "project_slug":"demo",
                    "source":"server-test",
                    "providers":[
                        {
                            "provider_id":"worldlabs-marble",
                            "external_job_id":"marble-real-no-attestation-1",
                            "artifacts":["worlds/demo/output/production/marble.glb"],
                            "evidence_json":{
                                "production_upstream":true,
                                "local_mock_gateway":false
                            }
                        }
                    ]
                }"#,
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 400);
        assert_eq!(value["error"], "invalid_production_evidence_item");
        assert_eq!(value["writes"], 0);
        assert!(value["message"]
            .as_str()
            .unwrap_or_default()
            .contains("production_attestation"));
    }

    #[test]
    fn post_production_evidence_rejects_missing_software_attestation() {
        let db_path = temp_db_path("runtime-http-production-evidence-missing-software-attestation");
        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let response = server
            .handle_request_with_body(
                "POST",
                "/api/production-evidence/validate",
                r#"{
                    "project_slug":"demo",
                    "source":"server-test",
                    "software_actions":[
                        {
                            "adapter_id":"unreal",
                            "external_action_id":"unreal-real-no-attestation-1",
                            "verification_json":{"ok":true},
                            "evidence_json":{
                                "production_software":true,
                                "local_mock_software":false
                            }
                        }
                    ]
                }"#,
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 400);
        assert_eq!(value["error"], "invalid_production_evidence_item");
        assert_eq!(value["writes"], 0);
        assert!(value["message"]
            .as_str()
            .unwrap_or_default()
            .contains("software_actions[0].production_attestation"));
    }

    #[test]
    fn post_production_evidence_rejects_missing_desktop_vision_attestation() {
        let db_path = temp_db_path("runtime-http-production-evidence-missing-desktop-attestation");
        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let response = server
            .handle_request_with_body(
                "POST",
                "/api/production-evidence/validate",
                r#"{
                    "project_slug":"demo",
                    "source":"server-test",
                    "desktop_vision":[
                        {
                            "adapter_id":"touchdesigner",
                            "external_action_id":"vision-real-no-attestation-1",
                            "controller_id":"external-vision-controller",
                            "trace_path":"worlds/demo/output/production/vision-trace.json",
                            "visual_model":"external",
                            "evidence_json":{
                                "external_visual_model":true,
                                "local_trace_smoke":false
                            }
                        }
                    ]
                }"#,
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 400);
        assert_eq!(value["error"], "invalid_production_evidence_item");
        assert_eq!(value["writes"], 0);
        assert!(value["message"]
            .as_str()
            .unwrap_or_default()
            .contains("desktop_vision[0].production_attestation"));
    }

    #[test]
    fn post_production_evidence_rejects_remote_provider_artifacts() {
        let db_path = temp_db_path("runtime-http-production-evidence-remote-artifact");
        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let response = server
            .handle_request_with_body(
                "POST",
                "/api/production-evidence",
                r#"{
                    "project_slug":"demo",
                    "source":"server-test",
                    "providers":[
                        {
                            "provider_id":"worldlabs-marble",
                            "external_job_id":"marble-real-1",
                            "production_attestation":"worldlabs-marble-worker-remote-artifact-run-1",
                            "artifacts":["https://provider.example.test/marble.glb"]
                        }
                    ]
                }"#,
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 400);
        assert_eq!(value["error"], "invalid_production_evidence_item");
        assert!(value["message"]
            .as_str()
            .unwrap_or_default()
            .contains("local file path"));
    }

    #[test]
    fn post_production_evidence_prevalidates_bundle_without_partial_writes() {
        let db_path = temp_db_path("runtime-http-production-evidence-prevalidate");
        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let response = server
            .handle_request_with_body(
                "POST",
                "/api/production-evidence",
                r#"{
                    "project_slug":"demo",
                    "source":"server-test",
                    "providers":[
                        {
                            "provider_id":"midjourney",
                            "external_job_id":"mj-real-1",
                            "production_attestation":"midjourney-worker-prevalidate-run-1",
                            "artifacts":["worlds/demo/output/production/midjourney.png"]
                        },
                        {
                            "provider_id":"worldlabs-marble",
                            "external_job_id":"replace-with-real-provider-job-id",
                            "artifacts":["worlds/demo/output/production/marble.glb"]
                        }
                    ]
                }"#,
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 400);
        assert_eq!(value["error"], "invalid_production_evidence_item");
        assert!(value["message"]
            .as_str()
            .unwrap_or_default()
            .contains("providers[1].external_job_id"));

        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        assert_eq!(repository.table_count("tasks").unwrap(), 0);
        assert_eq!(repository.table_count("provider_requests").unwrap(), 0);
        assert_eq!(repository.table_count("workflow_events").unwrap(), 0);
    }

    #[test]
    fn post_workflow_run_executes_local_content_burst_chain() {
        let db_path = temp_db_path("runtime-http-workflow-run");
        let output_root =
            std::env::temp_dir().join(format!("runtime-http-workflow-run-{}", Uuid::new_v4()));
        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let response = server
            .handle_request_with_body(
                "POST",
                "/api/workflow-runs",
                &format!(
                    r#"{{
                    "project_slug":"demo",
                    "output_root":"{}",
                    "title":"Runtime local content burst",
                    "prompt":"generate a content burst world",
                    "source_inputs":["worlds/demo/source/0-reference.png"],
                    "duration_ms":9000
                }}"#,
                    output_root.display()
                ),
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 201);
        assert_eq!(value["report"]["agent_mode"], "stage");
        assert_eq!(value["report"]["agent_report"]["status"], "Ready");
        assert_eq!(value["report"]["provider_report"]["status"], "Succeeded");
        assert_eq!(value["report"]["software_report"]["status"], "Succeeded");
        assert_eq!(value["report"]["output_report"]["status"], "Succeeded");
        assert_eq!(value["report"]["assets_indexed"], 6);
        assert_eq!(value["snapshot"]["stats"]["projects"], 1);
        assert_eq!(value["snapshot"]["stats"]["agent_sessions"], 1);
        assert_eq!(value["snapshot"]["stats"]["assets"], 6);
        assert!(output_root
            .join("worlds/demo/output/deliverables/1-video-timeline.json")
            .exists());

        std::fs::remove_dir_all(output_root).unwrap();
    }

    #[test]
    fn post_workflow_run_accepts_gateway_and_unreal_mcp_modes() {
        let gateway = spawn_fake_3dgs_gateway();
        let unreal = spawn_fake_unreal_mcp_server();
        let db_path = temp_db_path("runtime-http-workflow-run-real-adapters");
        let output_root = std::env::temp_dir().join(format!(
            "runtime-http-workflow-run-real-adapters-{}",
            Uuid::new_v4()
        ));
        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let response = server
            .handle_request_with_body(
                "POST",
                "/api/workflow-runs",
                &format!(
                    r#"{{
                    "project_slug":"demo",
                    "output_root":"{}",
                    "title":"Runtime gateway content burst",
                    "prompt":"generate a content burst world",
                    "source_inputs":["worlds/demo/source/0-reference.png"],
                    "duration_ms":9000,
                    "three_dgs_mode":"gateway",
                    "three_dgs_endpoint":"{}",
                    "unreal_mode":"unreal_mcp",
                    "unreal_endpoint":"{}"
                }}"#,
                    output_root.display(),
                    gateway,
                    unreal
                ),
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 201);
        assert_eq!(value["report"]["three_dgs_mode"], "gateway");
        assert_eq!(value["report"]["unreal_mode"], "unreal_mcp");
        assert_eq!(
            value["report"]["provider_report"]["assets"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            value["report"]["software_report"]["result"]["artifacts"][0],
            "unreal://level/demo"
        );
        assert_eq!(value["report"]["assets_indexed"], 4);

        std::fs::remove_dir_all(output_root).unwrap();
    }

    #[test]
    fn post_output_package_creates_deliverable_manifests() {
        let db_path = temp_db_path("runtime-http-output-package");
        let output_dir =
            std::env::temp_dir().join(format!("runtime-http-output-package-{}", Uuid::new_v4()));
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        let plan = build_default_content_burst_plan("demo", "Pool HTTP demo");
        repository.persist_plan(&plan).unwrap();
        drop(repository);

        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let response = server
            .handle_request_with_body(
                "POST",
                "/api/output-packages",
                &format!(
                    r#"{{
                    "project_slug":"demo",
                    "node_id":"outputs",
                    "output_dir":"{}",
                    "title":"Runtime output package",
                    "source_assets":["worlds/demo/output/1-world.glb"],
                    "duration_ms":8000
                }}"#,
                    output_dir.display()
                ),
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 201);
        assert_eq!(value["report"]["status"], "Succeeded");
        assert_eq!(value["report"]["assets"].as_array().unwrap().len(), 3);
        assert_eq!(value["report"]["manifests"].as_array().unwrap().len(), 3);
        assert_eq!(value["report"]["manifests"][0]["target"], "video");
        assert_eq!(value["task"]["provider_id"], "output-package");
        assert_eq!(value["task"]["node_id"], "outputs");
        assert_eq!(value["snapshot"]["stats"]["assets"], 3);
        assert!(output_dir
            .join("deliverables/1-video-timeline.json")
            .exists());
        assert!(output_dir.join("deliverables/2-game-build.json").exists());
        assert!(output_dir
            .join("deliverables/3-interactive-cues.json")
            .exists());

        let catalog = server.handle_path("/api/output-packages").unwrap();
        let catalog_value: serde_json::Value = serde_json::from_str(&catalog.body).unwrap();
        assert_eq!(catalog.status_code, 200);
        assert_eq!(catalog_value["summary"]["ready_targets"], 3);
        assert_eq!(catalog_value["deliverables"][0]["target"], "video");
        assert_eq!(catalog_value["deliverables"][0]["manifest_found"], true);
        assert_eq!(
            catalog_value["deliverables"][2]["control_routes"][0],
            "touchdesigner"
        );

        let output_packages_mcp = server
            .handle_path("/api/mcp?uri=pool%3A%2F%2Foutput-packages")
            .unwrap();
        let output_packages_mcp_value: serde_json::Value =
            serde_json::from_str(&output_packages_mcp.body).unwrap();
        assert_eq!(output_packages_mcp.status_code, 200);
        assert_eq!(output_packages_mcp_value["summary"]["ready_targets"], 3);

        std::fs::remove_dir_all(output_dir).unwrap();
    }

    #[test]
    fn post_output_package_result_updates_deliverable_manifest() {
        let db_path = temp_db_path("runtime-http-output-package-result");
        let output_dir = std::env::temp_dir().join(format!(
            "runtime-http-output-package-result-{}",
            Uuid::new_v4()
        ));
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        let plan = build_default_content_burst_plan("demo", "Pool HTTP demo");
        repository.persist_plan(&plan).unwrap();
        drop(repository);

        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        server
            .handle_request_with_body(
                "POST",
                "/api/output-packages",
                &format!(
                    r#"{{
                    "project_slug":"demo",
                    "node_id":"outputs",
                    "output_dir":"{}",
                    "title":"Runtime output package",
                    "source_assets":["worlds/demo/output/1-world.glb"],
                    "duration_ms":8000
                }}"#,
                    output_dir.display()
                ),
            )
            .unwrap();
        let response = server
            .handle_request_with_body(
                "POST",
                "/api/output-packages/results",
                r#"{
                    "project_slug":"demo",
                    "node_id":"outputs",
                    "target":"game",
                    "status":"succeeded",
                    "runtime":"Unreal",
                    "adapter_id":"unreal",
                    "software_action_id":"action-unreal",
                    "message":"play-in-editor viewport verified",
                    "artifacts":["unreal://level/demo_content_burst"],
                    "metrics":[{"label":"fps","value":"60"}],
                    "verification":{"viewport":"play_in_editor"}
                }"#,
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();
        let game_manifest: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(output_dir.join("deliverables/2-game-build.json")).unwrap(),
        )
        .unwrap();

        assert_eq!(response.status_code, 201);
        assert_eq!(value["report"]["status"], "Succeeded");
        assert_eq!(value["task"]["provider_id"], "output-package-result");
        assert_eq!(value["task"]["node_id"], "outputs");
        assert_eq!(value["snapshot"]["stats"]["tasks"], 11);
        assert_eq!(
            game_manifest["execution_result"]["message"],
            "play-in-editor viewport verified"
        );
        assert_eq!(
            game_manifest["execution_history"].as_array().unwrap().len(),
            1
        );
        assert!(value["report"]["catalog"]["deliverables"][1]["metrics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|metric| metric["label"] == "execution" && metric["value"] == "succeeded"));

        std::fs::remove_dir_all(output_dir).unwrap();
    }

    #[test]
    fn post_handoff_package_creates_runtime_handoff_files() {
        let db_path = temp_db_path("runtime-http-handoff-package");
        let output_dir =
            std::env::temp_dir().join(format!("runtime-http-handoff-package-{}", Uuid::new_v4()));
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        let plan = build_default_content_burst_plan("demo", "Pool HTTP demo");
        repository.persist_plan(&plan).unwrap();
        drop(repository);

        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let response = server
            .handle_request_with_body(
                "POST",
                "/api/handoff-packages",
                &format!(
                    r#"{{
                    "project_slug":"demo",
                    "node_id":"agent",
                    "output_dir":"{}",
                    "title":"Runtime handoff package",
                    "include_snapshot":true
                }}"#,
                    output_dir.display()
                ),
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 201);
        assert_eq!(value["report"]["status"], "Succeeded");
        assert_eq!(value["report"]["assets"].as_array().unwrap().len(), 9);
        assert_eq!(value["task"]["provider_id"], "runtime-handoff-package");
        assert_eq!(value["task"]["node_id"], "agent");
        assert_eq!(value["snapshot"]["stats"]["assets"], 9);
        assert!(output_dir
            .join("control/handoff/.1-runtime-handoff-request.json")
            .exists());
        assert!(output_dir
            .join("control/handoff/1-runtime-handoff.json")
            .exists());
        assert!(output_dir
            .join("control/handoff/2-runtime-preflight.json")
            .exists());
        assert!(output_dir
            .join("control/handoff/3-runtime-graph.json")
            .exists());
        assert!(output_dir
            .join("control/handoff/5-worker-self-checks.sh")
            .exists());
        assert!(output_dir
            .join("control/handoff/6-worker-self-checks-preflight.json")
            .exists());
        assert!(output_dir
            .join("control/handoff/7-integration-readiness.json")
            .exists());
        assert!(output_dir
            .join("control/handoff/8-runtime-handoff-package-manifest.json")
            .exists());
        assert!(output_dir
            .join("control/handoff/4-runtime-snapshot.json")
            .exists());
        assert!(value["report"]["worker_self_checks_path"]
            .as_str()
            .unwrap_or_default()
            .ends_with("5-worker-self-checks.sh"));
        assert!(value["report"]["worker_self_checks_preflight_path"]
            .as_str()
            .unwrap_or_default()
            .ends_with("6-worker-self-checks-preflight.json"));
        assert!(value["report"]["integration_readiness_path"]
            .as_str()
            .unwrap_or_default()
            .ends_with("7-integration-readiness.json"));
        assert!(value["report"]["manifest_path"]
            .as_str()
            .unwrap_or_default()
            .ends_with("8-runtime-handoff-package-manifest.json"));
        assert!(
            value["report"]["operator_checklist"]
                .as_array()
                .unwrap()
                .len()
                >= 6
        );
        assert!(value["report"]["operator_checklist"]
            .as_array()
            .unwrap()
            .iter()
            .any(|step| step["owner"] == "ai_3dgs_td"
                && step["command"] == "pool-cli --project demo integration-readiness"));
        assert!(value["report"]["agent_entrypoint"]["first_file"]
            .as_str()
            .unwrap_or_default()
            .ends_with("8-runtime-handoff-package-manifest.json"));
        assert!(value["report"]["mcp_resources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|resource| resource.as_str() == Some("pool://integration-readiness")));
        let catalog_response = server.handle_path("/api/handoff-packages").unwrap();
        let catalog: serde_json::Value = serde_json::from_str(&catalog_response.body).unwrap();
        assert_eq!(catalog_response.status_code, 200);
        assert_eq!(catalog["summary"]["package_count"], 1);
        assert_eq!(catalog["summary"]["ready_packages"], 1);
        assert_eq!(
            catalog["packages"][0]["manifest_path"],
            value["report"]["manifest_path"]
        );
        assert!(catalog["packages"][0]["operator_checklist"]
            .as_array()
            .unwrap()
            .iter()
            .any(|step| step["owner"] == "ai_3dgs_td"));
        assert!(catalog["packages"][0]["agent_entrypoint"]["mcp_stdio"]
            .as_str()
            .unwrap_or_default()
            .contains("serve-mcp"));
        assert!(catalog["packages"][0]["mcp_resources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|resource| resource.as_str() == Some("pool://integration-readiness")));

        std::fs::remove_dir_all(output_dir).unwrap();
    }

    #[test]
    fn post_software_conformance_package_creates_adapter_runbook_files() {
        let db_path = temp_db_path("runtime-http-software-conformance-package");
        let output_dir = std::env::temp_dir().join(format!(
            "runtime-http-software-conformance-package-{}",
            Uuid::new_v4()
        ));
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        let plan = build_default_content_burst_plan("demo", "Pool HTTP demo");
        repository.persist_plan(&plan).unwrap();
        drop(repository);

        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let response = server
            .handle_request_with_body(
                "POST",
                "/api/software-conformance-packages",
                &format!(
                    r#"{{
                    "project_slug":"demo",
                    "node_id":"resolve-node",
                    "adapter_id":"resolve",
                    "output_dir":"{}",
                    "title":"Resolve conformance package"
                }}"#,
                    output_dir.display()
                ),
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 201);
        assert_eq!(value["report"]["adapter_id"], "resolve");
        assert_eq!(value["task"]["provider_id"], "software-conformance-package");
        assert_eq!(value["task"]["status"], "Succeeded");
        assert_eq!(value["task"]["node_id"], "resolve-node");
        assert_eq!(value["snapshot"]["stats"]["assets"], 6);
        assert!(output_dir
            .join(
                "control/software-conformance/resolve/.1-software-conformance-package-request.json"
            )
            .exists());
        assert!(output_dir
            .join("control/software-conformance/resolve/1-software-control-contract.json")
            .exists());
        assert!(output_dir
            .join("control/software-conformance/resolve/2-software-conformance-runbook.json")
            .exists());
        assert!(output_dir
            .join("control/software-conformance/resolve/3-software-conformance-preflight.json")
            .exists());
        let runner_path = output_dir
            .join("control/software-conformance/resolve/4-software-conformance-runner.sh");
        assert!(runner_path.exists());
        let runner = std::fs::read_to_string(runner_path).unwrap();
        assert!(runner.contains("software-api-bridge-worker resolve"));
        assert!(runner.contains("production-evidence-software-matrix"));
        assert!(runner.contains("POOL_RESOLVE_UPSTREAM_ENDPOINT"));
        assert!(output_dir
            .join(
                "control/software-conformance/resolve/5-software-conformance-package-manifest.json"
            )
            .exists());

        std::fs::remove_dir_all(output_dir).unwrap();
    }

    #[test]
    fn post_provider_conformance_package_creates_provider_runbook_files() {
        let db_path = temp_db_path("runtime-http-provider-conformance-package");
        let output_dir = std::env::temp_dir().join(format!(
            "runtime-http-provider-conformance-package-{}",
            Uuid::new_v4()
        ));
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        let plan = build_default_content_burst_plan("demo", "Pool HTTP demo");
        repository.persist_plan(&plan).unwrap();
        drop(repository);

        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let response = server
            .handle_request_with_body(
                "POST",
                "/api/provider-conformance-packages",
                &format!(
                    r#"{{
                    "project_slug":"demo",
                    "node_id":"provider-node",
                    "provider_id":"world-labs-marble",
                    "output_dir":"{}",
                    "title":"Marble provider conformance package"
                }}"#,
                    output_dir.display()
                ),
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 201);
        assert_eq!(value["report"]["provider_id"], "worldlabs-marble");
        assert_eq!(value["task"]["provider_id"], "provider-conformance-package");
        assert_eq!(value["task"]["status"], "Succeeded");
        assert_eq!(value["task"]["node_id"], "provider-node");
        assert_eq!(value["snapshot"]["stats"]["assets"], 7);
        assert!(output_dir
            .join(
                "control/provider-conformance/worldlabs-marble/.1-provider-conformance-package-request.json"
            )
            .exists());
        assert!(output_dir
            .join("control/provider-conformance/worldlabs-marble/1-provider-contract.json")
            .exists());
        assert!(output_dir
            .join(
                "control/provider-conformance/worldlabs-marble/2-provider-gateway-worker-contract.json"
            )
            .exists());
        assert!(output_dir
            .join(
                "control/provider-conformance/worldlabs-marble/3-provider-conformance-runbook.json"
            )
            .exists());
        assert!(output_dir
            .join(
                "control/provider-conformance/worldlabs-marble/4-provider-conformance-preflight.json"
            )
            .exists());
        let runner_path = output_dir
            .join("control/provider-conformance/worldlabs-marble/5-provider-conformance-runner.sh");
        assert!(runner_path.exists());
        let runner = std::fs::read_to_string(runner_path).unwrap();
        assert!(runner.contains("provider-gateway-worker --once"));
        assert!(runner.contains("production-evidence-provider-matrix"));
        assert!(runner.contains("POOL_WORLDLABS_MARBLE_UPSTREAM_ENDPOINT"));
        assert!(output_dir
            .join(
                "control/provider-conformance/worldlabs-marble/6-provider-conformance-package-manifest.json"
            )
            .exists());

        std::fs::remove_dir_all(output_dir).unwrap();
    }

    #[test]
    fn post_agent_conformance_package_creates_session_runbook_files() {
        let db_path = temp_db_path("runtime-http-agent-conformance-package");
        let output_dir = std::env::temp_dir().join(format!(
            "runtime-http-agent-conformance-package-{}",
            Uuid::new_v4()
        ));
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        let plan = build_default_content_burst_plan("demo", "Pool HTTP demo");
        repository.persist_plan(&plan).unwrap();
        drop(repository);

        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let response = server
            .handle_request_with_body(
                "POST",
                "/api/agent-conformance-packages",
                &format!(
                    r#"{{
                    "project_slug":"demo",
                    "node_id":"agent-node",
                    "kind":"all",
                    "output_dir":"{}",
                    "title":"Agent Hermes conformance package"
                }}"#,
                    output_dir.display()
                ),
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 201);
        assert_eq!(value["report"]["session_kind"], "all");
        assert_eq!(value["task"]["provider_id"], "agent-conformance-package");
        assert_eq!(value["task"]["status"], "Succeeded");
        assert_eq!(value["task"]["node_id"], "agent-node");
        assert_eq!(value["snapshot"]["stats"]["assets"], 6);
        assert!(output_dir
            .join("control/agent-conformance/all/.1-agent-conformance-package-request.json")
            .exists());
        assert!(output_dir
            .join("control/agent-conformance/all/1-agent-session-contract.json")
            .exists());
        assert!(output_dir
            .join("control/agent-conformance/all/2-agent-conformance-runbook.json")
            .exists());
        assert!(output_dir
            .join("control/agent-conformance/all/3-agent-conformance-preflight.json")
            .exists());
        let runner_path =
            output_dir.join("control/agent-conformance/all/4-agent-conformance-runner.sh");
        assert!(runner_path.exists());
        let runner = std::fs::read_to_string(runner_path).unwrap();
        assert!(runner.contains("hermes-mcp-bridge-worker --once"));
        assert!(runner.contains("agent-session hermes"));
        assert!(runner.contains("agent-session agent-cli"));
        assert!(runner.contains("POOL_HERMES_ENDPOINT"));
        assert!(output_dir
            .join("control/agent-conformance/all/5-agent-conformance-package-manifest.json")
            .exists());

        std::fs::remove_dir_all(output_dir).unwrap();
    }

    #[test]
    fn post_integration_conformance_package_creates_team_handoff_files() {
        let db_path = temp_db_path("runtime-http-integration-conformance-package");
        let output_dir = std::env::temp_dir().join(format!(
            "runtime-http-integration-conformance-package-{}",
            Uuid::new_v4()
        ));
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        let plan = build_default_content_burst_plan("demo", "Pool HTTP demo");
        repository.persist_plan(&plan).unwrap();
        drop(repository);

        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let body = json!({
            "project_slug": "demo",
            "node_id": "agent-node",
            "output_dir": output_dir.to_string_lossy(),
            "title": "Integration conformance package",
            "providers": ["worldlabs-marble"],
            "software_adapters": ["resolve"],
            "agent_kind": "all"
        });
        let response = server
            .handle_request_with_body(
                "POST",
                "/api/integration-conformance-packages",
                &body.to_string(),
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 201);
        assert_eq!(
            value["report"]["kind"],
            "pool_integration_conformance_package_report"
        );
        assert_eq!(value["report"]["summary"]["providers"], 1);
        assert_eq!(value["report"]["summary"]["software_adapters"], 1);
        assert_eq!(value["report"]["summary"]["agent"], true);
        assert_eq!(
            value["task"]["provider_id"],
            "integration-conformance-package"
        );
        assert_eq!(value["task"]["status"], "Succeeded");
        assert_eq!(value["task"]["node_id"], "agent-node");
        assert_eq!(value["snapshot"]["stats"]["assets"], 23);
        assert!(output_dir
            .join("control/integration-conformance/.1-integration-conformance-package-request.json")
            .exists());
        assert!(output_dir
            .join("control/integration-conformance/1-integration-conformance-runbook.json")
            .exists());
        let runner_path =
            output_dir.join("control/integration-conformance/2-integration-conformance-runner.sh");
        assert!(runner_path.exists());
        let runner = std::fs::read_to_string(runner_path).unwrap();
        assert!(runner.contains("providers/worldlabs-marble/5-provider-conformance-runner.sh"));
        assert!(runner.contains("software/resolve/4-software-conformance-runner.sh"));
        assert!(runner.contains("agent/all/4-agent-conformance-runner.sh"));
        assert!(output_dir
            .join("control/integration-conformance/3-integration-conformance-package-manifest.json")
            .exists());
        assert!(output_dir
            .join(
                "control/integration-conformance/providers/worldlabs-marble/6-provider-conformance-package-manifest.json"
            )
            .exists());
        assert!(output_dir
            .join("control/integration-conformance/software/resolve/5-software-conformance-package-manifest.json")
            .exists());
        assert!(output_dir
            .join(
                "control/integration-conformance/agent/all/5-agent-conformance-package-manifest.json"
            )
            .exists());

        std::fs::remove_dir_all(output_dir).unwrap();
    }

    #[test]
    fn post_production_evidence_handoff_package_creates_item_files() {
        let db_path = temp_db_path("runtime-http-production-evidence-handoff-package");
        let output_dir = std::env::temp_dir().join(format!(
            "runtime-http-production-evidence-handoff-package-{}",
            Uuid::new_v4()
        ));
        let output_root = output_dir.join("production-artifacts");
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        let plan = build_default_content_burst_plan("demo", "Pool HTTP demo");
        repository.persist_plan(&plan).unwrap();
        drop(repository);

        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let response = server
            .handle_request_with_body(
                "POST",
                "/api/production-evidence/handoff-packages",
                &format!(
                    r#"{{
                    "project_slug":"demo",
                    "node_id":"agent",
                    "output_dir":"{}",
                    "output_root":"{}",
                    "source":"external-worker-handoff",
                    "title":"Production evidence handoff package",
                    "include_snapshot":true
                }}"#,
                    output_dir.display(),
                    output_root.display()
                ),
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 201);
        assert_eq!(value["kind"], "pool_production_evidence_handoff_package");
        assert_eq!(value["report"]["status"], "Succeeded");
        assert!(value["report"]["item_count"].as_u64().unwrap() >= 20);
        assert_eq!(
            value["task"]["provider_id"],
            "production-evidence-handoff-package"
        );
        assert_eq!(value["task"]["node_id"], "agent");
        assert!(value["assets"].as_array().unwrap().len() > 20);
        assert!(output_dir
            .join("control/production-evidence/.1-production-evidence-handoff-package-request.json")
            .exists());
        assert!(output_dir
            .join("control/production-evidence/1-production-evidence-requirements.json")
            .exists());
        assert!(output_dir
            .join("control/production-evidence/2-production-evidence-tasks.json")
            .exists());
        assert!(output_dir
            .join("control/production-evidence/3-production-evidence-handoff.json")
            .exists());
        assert!(output_dir
            .join("control/production-evidence/4-production-evidence-run-plan.json")
            .exists());
        assert!(output_dir
            .join("control/production-evidence/5-production-evidence-bundle.json")
            .exists());
        assert!(output_dir
            .join("control/production-evidence/6-production-evidence-package-manifest.json")
            .exists());
        assert!(output_dir
            .join("control/production-evidence/7-production-evidence-runner.sh")
            .exists());
        assert!(output_dir
            .join("control/production-evidence/8-production-evidence-runner-preflight.json")
            .exists());
        assert!(output_dir
            .join("control/production-evidence/9-runtime-snapshot.json")
            .exists());
        assert!(output_dir
            .join("control/production-evidence/items/1-provider-midjourney-item.json")
            .exists());
        assert!(output_dir
            .join("control/production-evidence/items/.1-provider-midjourney-item-template.json")
            .exists());
        let manifest_path = output_dir
            .join("control/production-evidence/6-production-evidence-package-manifest.json");
        let manifest_value: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(manifest_path).unwrap()).unwrap();
        assert!(value["report"]["run_plan_path"]
            .as_str()
            .unwrap()
            .ends_with("4-production-evidence-run-plan.json"));
        assert_eq!(
            manifest_value["paths"]["run_plan"]
                .as_str()
                .unwrap()
                .ends_with("4-production-evidence-run-plan.json"),
            true
        );
        assert_eq!(
            manifest_value["paths"]["runner_script"]
                .as_str()
                .unwrap()
                .ends_with("7-production-evidence-runner.sh"),
            true
        );
        assert_eq!(
            manifest_value["paths"]["runner_preflight"]
                .as_str()
                .unwrap()
                .ends_with("8-production-evidence-runner-preflight.json"),
            true
        );
        assert!(manifest_value["commands"]["run_plan"]
            .as_str()
            .unwrap()
            .contains("production-evidence-run-plan"));
        assert!(manifest_value["commands"]["runner_script"]
            .as_str()
            .unwrap()
            .contains("7-production-evidence-runner.sh"));
        assert!(manifest_value["commands"]["runner_preflight"]
            .as_str()
            .unwrap()
            .contains("--preflight"));
        assert!(manifest_value["commands"]["closeout_bundle"]
            .as_str()
            .unwrap()
            .contains("closeout-production-evidence"));
        assert!(manifest_value["commands"]["closeout_import"]
            .as_str()
            .unwrap()
            .contains("--import"));
        let resolve_manifest_item = manifest_value["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["task_id"] == "software:resolve:production_software")
            .expect("resolve manifest item");
        assert_eq!(
            resolve_manifest_item["preferred_control_profile"],
            "api_mcp"
        );
        assert_eq!(resolve_manifest_item["bundle_path"], "software_actions[]");
        assert!(resolve_manifest_item["bridge_worker"]["available"]
            .as_bool()
            .unwrap());
        assert_eq!(
            resolve_manifest_item["bridge_worker"]["endpoint_env"],
            "POOL_RESOLVE_ENDPOINT"
        );
        assert!(resolve_manifest_item["bridge_worker"]["cli_template"]
            .as_str()
            .unwrap()
            .contains("software-api-bridge-worker resolve"));
        let runner_script = std::fs::read_to_string(
            output_dir.join("control/production-evidence/7-production-evidence-runner.sh"),
        )
        .unwrap();
        assert!(runner_script.contains("production-evidence-provider-matrix"));
        assert!(runner_script.contains("production-evidence-software-matrix"));
        assert!(runner_script.contains("POOL_MEDIA_GATEWAY_ENDPOINT"));
        assert!(runner_script.contains("POOL_3DGS_GATEWAY_ENDPOINT"));
        assert!(runner_script.contains("POOL_PROVIDER_PRODUCTION_ATTESTATION"));
        assert!(runner_script.contains("POOL_SOFTWARE_PRODUCTION_ATTESTATION"));
        assert!(runner_script.contains("POOL_RUN_DESKTOP_VISION"));
        assert!(runner_script.contains("POOL_IMPORT_PRODUCTION_EVIDENCE"));
        assert!(runner_script.contains("check_software_env \"blender\""));
        assert!(runner_script.contains("check_software_artifacts \"blender\""));
        assert!(runner_script.contains("--software-endpoint-env blender=POOL_BLENDER_ENDPOINT"));
        assert!(runner_script.contains("POOL_BLENDER_ENDPOINT"));
        assert!(runner_script.contains("POOL_BLENDER_COMMAND"));
        assert!(runner_script.contains("--software-endpoint-env resolve=POOL_RESOLVE_ENDPOINT"));
        assert!(runner_script.contains("POOL_RESOLVE_ENDPOINT"));
        assert!(runner_script.contains("--software-endpoint-env nuke=POOL_NUKE_ENDPOINT"));
        assert!(runner_script.contains("POOL_NUKE_ENDPOINT"));
        assert!(runner_script.contains("POOL_BLENDER_ARTIFACTS"));
        assert!(runner_script.contains("POOL_UNREAL_MCP_ENDPOINT"));
        assert!(runner_script.contains("POOL_UNREAL_ARTIFACTS"));
        assert!(runner_script.contains("INVALID production software artifact path"));
        assert!(runner_script.contains("POOL_CLI_CMD"));
        assert!(runner_script.contains("cargo run -q -p pool-cli --"));
        assert!(runner_script.contains("rewrite_pool_cli_cmd"));
        assert!(
            runner_script.contains("PROVIDER_CMD=\"$(rewrite_pool_cli_cmd \"$PROVIDER_CMD\")\"")
        );
        assert!(runner_script.contains("check_provider_endpoint \"sam-3d\""));
        assert!(runner_script.contains("check_provider_api_key \"openai-image-2\""));
        assert!(runner_script.contains("pool-unused-media-gateway"));
        assert!(runner_script.contains("pool-unused-3dgs-gateway"));
        assert!(runner_script.contains("POOL_PROVIDER_ENDPOINT_SAM_3D"));
        assert!(runner_script.contains("POOL_PROVIDER_API_KEY_QUNHE_3D"));
        assert!(runner_script.contains("POOL_PROVIDER_PRODUCTION_ATTESTATION_WORLDLABS_MARBLE"));
        assert!(runner_script.contains("check_provider_attestation \"tripo-splat\""));
        assert!(runner_script.contains("POOL_PROVIDER_PRODUCTION_ATTESTATION_TRIPO_SPLAT"));
        assert!(runner_script.contains("POOL_TRIPO_SPLAT_PRODUCTION_ATTESTATION"));
        assert!(runner_script.contains("POOL_CLI_CMD\" == cargo\\ *"));
        assert!(
            runner_script.contains("MISSING cargo command on PATH for POOL_CLI_CMD cargo fallback")
        );
        assert!(runner_script.contains("runner_preflight"));
        assert!(runner_script.contains("preflight_status=ready"));
        assert!(runner_script.contains("INFO bridge worker resolve"));
        assert!(runner_script.contains("POOL_RESOLVE_ENDPOINT=http://127.0.0.1:<port>"));
        assert!(runner_script.contains("POOL_RESOLVE_UPSTREAM_ENDPOINT"));
        assert!(runner_script.contains("software-api-bridge-worker resolve"));
        assert!(runner_script.contains("INFO provider gateway 3dgs"));
        assert!(runner_script.contains("POOL_3DGS_GATEWAY_UPSTREAM_ENDPOINT"));
        assert!(runner_script.contains("provider-gateway-worker"));
        assert!(runner_script.contains("Usage: $0 [run|--preflight|preflight]"));
        assert!(runner_script.contains("production-evidence-runner-desktop-skipped"));
        let runner_preflight: serde_json::Value =
            serde_json::from_str(
                &std::fs::read_to_string(output_dir.join(
                    "control/production-evidence/8-production-evidence-runner-preflight.json",
                ))
                .unwrap(),
            )
            .unwrap();
        assert_eq!(
            runner_preflight["kind"],
            "pool_production_evidence_runner_preflight"
        );
        assert!(runner_preflight["environment"]["required_for_provider"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item
                .as_str()
                .unwrap_or_default()
                .contains("POOL_MEDIA_GATEWAY_ENDPOINT or per-media")));
        assert!(runner_preflight["environment"]["required_for_provider"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item
                .as_str()
                .unwrap_or_default()
                .contains("POOL_3DGS_GATEWAY_ENDPOINT or per-3DGS")));
        assert!(runner_preflight["environment"]["required_for_provider"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item.as_str().unwrap_or_default().contains("OPENAI_API_KEY")));
        assert!(runner_preflight["environment"]["required_for_provider"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item
                .as_str()
                .unwrap_or_default()
                .contains("POOL_PROVIDER_PRODUCTION_ATTESTATION")));
        assert_eq!(
            runner_preflight["environment"]["optional_gates"][0],
            "POOL_CLI_CMD=<custom pool-cli invocation>"
        );
        assert!(runner_preflight["environment"]["command_path_warnings"][0]
            .as_str()
            .unwrap()
            .contains("cargo is required only"));
        assert!(runner_preflight["environment"]["command_path_warnings"][1]
            .as_str()
            .unwrap()
            .contains("pool-cli on PATH is preferred"));
        assert!(runner_preflight["environment"]["required_for_software"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item
                .as_str()
                .unwrap_or_default()
                .contains("POOL_SOFTWARE_BLENDER_ENDPOINT")));
        assert!(runner_preflight["environment"]["required_for_software"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item
                .as_str()
                .unwrap_or_default()
                .contains("POOL_SOFTWARE_BLENDER_COMMAND")));
        assert!(
            runner_preflight["environment"]["required_for_software_artifacts"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item
                    .as_str()
                    .unwrap_or_default()
                    .contains("POOL_SOFTWARE_BLENDER_ARTIFACTS"))
        );
        assert!(runner_preflight["phases"][1]["required_env"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item.as_str().unwrap_or_default().contains("ARTIFACTS")));
        assert!(runner_preflight["phases"][1]["required_env"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item
                .as_str()
                .unwrap_or_default()
                .contains("software-api-bridge-worker")));
        assert!(
            runner_preflight["environment"]["software_bridge_worker"]["applies_to"]
                .as_array()
                .unwrap()
                .iter()
                .any(|adapter| adapter.as_str() == Some("resolve"))
        );
        assert!(
            runner_preflight["environment"]["software_bridge_worker"]["cli_template"]
                .as_str()
                .unwrap()
                .contains("software-api-bridge-worker")
        );
        assert!(runner_preflight["phases"][1]["generic_api_bridge_worker"]
            ["endpoint_env_template"]
            .as_str()
            .unwrap()
            .contains("POOL_<ADAPTER>_ENDPOINT"));
        let resolve_bridge_command = runner_preflight["phases"][1]["bridge_worker_start_commands"]
            .as_array()
            .unwrap()
            .iter()
            .find(|command| command["adapter_id"] == "resolve")
            .expect("resolve bridge worker start command");
        assert_eq!(
            resolve_bridge_command["endpoint_env"],
            "POOL_RESOLVE_ENDPOINT"
        );
        assert_eq!(
            resolve_bridge_command["upstream_env"],
            "POOL_RESOLVE_UPSTREAM_ENDPOINT"
        );
        assert!(resolve_bridge_command["cli"]
            .as_str()
            .unwrap()
            .contains("software-api-bridge-worker resolve"));
        assert!(
            runner_preflight["environment"]["software_bridge_worker_start_commands"]
                .as_array()
                .unwrap()
                .iter()
                .any(|command| command["adapter_id"] == "resolve")
        );
        let three_dgs_provider_gateway_command = runner_preflight["environment"]
            ["provider_gateway_worker_start_commands"]
            .as_array()
            .unwrap()
            .iter()
            .find(|command| command["family"] == "3dgs")
            .expect("3dgs provider gateway worker start command");
        assert_eq!(
            three_dgs_provider_gateway_command["endpoint_env"],
            "POOL_3DGS_GATEWAY_ENDPOINT"
        );
        assert_eq!(
            three_dgs_provider_gateway_command["upstream_env"],
            "POOL_3DGS_GATEWAY_UPSTREAM_ENDPOINT"
        );
        assert!(three_dgs_provider_gateway_command["cli"]
            .as_str()
            .unwrap()
            .contains("provider-gateway-worker"));
        assert!(
            runner_preflight["phases"][0]["provider_gateway_worker_start_commands"]
                .as_array()
                .unwrap()
                .iter()
                .any(|command| command["family"] == "ai_media")
        );
        assert!(runner_preflight["phases"][1]["operator_note"]
            .as_str()
            .unwrap()
            .contains("local artifact env"));
        assert!(runner_preflight["phases"][1]["operator_note"]
            .as_str()
            .unwrap()
            .contains("software-api-bridge-worker"));
        assert_eq!(
            runner_preflight["phases"][0]["id"],
            "provider_evidence_matrix"
        );
        assert!(runner_preflight["phases"][0]["operator_note"]
            .as_str()
            .unwrap()
            .contains("every required Provider"));
        assert_eq!(
            runner_preflight["phases"][2]["skipped_bundle_source"],
            "production-evidence-runner-desktop-skipped"
        );
        assert!(value["report"]["runner_script_path"]
            .as_str()
            .unwrap()
            .ends_with("7-production-evidence-runner.sh"));
        assert!(value["report"]["runner_preflight_path"]
            .as_str()
            .unwrap()
            .ends_with("8-production-evidence-runner-preflight.json"));
        assert!(value["report"]["software_bridge_worker_start_commands"]
            .as_array()
            .unwrap()
            .iter()
            .any(|command| command["adapter_id"] == "resolve"
                && command["endpoint_env"] == "POOL_RESOLVE_ENDPOINT"
                && command["cli"]
                    .as_str()
                    .unwrap_or_default()
                    .contains("software-api-bridge-worker resolve")));
        assert!(value["report"]["provider_gateway_worker_start_commands"]
            .as_array()
            .unwrap()
            .iter()
            .any(|command| command["family"] == "ai_media"
                && command["endpoint_env"] == "POOL_MEDIA_GATEWAY_ENDPOINT"
                && command["cli"]
                    .as_str()
                    .unwrap_or_default()
                    .contains("provider-gateway-worker")));

        std::fs::remove_dir_all(output_dir).unwrap();
    }

    #[test]
    fn post_provider_run_respects_approval_gate() {
        let db_path = temp_db_path("runtime-http-provider-run-approval");
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        let plan = build_default_content_burst_plan("demo", "Pool HTTP demo");
        repository.persist_plan(&plan).unwrap();
        drop(repository);

        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let response = server
            .handle_request_with_body(
                "POST",
                "/api/provider-runs",
                r#"{
                    "provider_id":"tripo-splat",
                    "task_title":"TripoSplat gated run",
                    "output_dir":"target/runtime-http-provider-run-approval/worlds/demo/output",
                    "requires_approval":true
                }"#,
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 200);
        assert_eq!(value["report"]["status"], "WaitingApproval");
        assert_eq!(value["report"]["assets"].as_array().unwrap().len(), 0);
        assert_eq!(value["snapshot"]["stats"]["waiting_approval"], 2);
        assert_eq!(value["snapshot"]["stats"]["provider_requests"], 1);
        let metadata_path = value["task"]["request_metadata_path"].as_str().unwrap();
        assert!(metadata_path.ends_with(".0-provider-approval__tripo-splat-request.json"));
        assert!(Path::new(metadata_path).exists());
    }

    #[test]
    fn post_provider_run_auto_estimates_high_cost_provider_and_waits_for_approval() {
        let db_path = temp_db_path("runtime-http-provider-run-auto-approval");
        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let response = server
            .handle_request_with_body(
                "POST",
                "/api/provider-runs",
                r#"{
                    "provider_id":"world-labs-marble",
                    "execution_mode":"mock",
                    "task_title":"World Labs Marble auto approval run",
                    "output_dir":"target/runtime-http-provider-run-auto-approval/worlds/demo/output"
                }"#,
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 200);
        assert_eq!(value["report"]["status"], "WaitingApproval");
        assert!(value["provider_request_id"].as_str().is_some());
        assert_eq!(value["task"]["cost_estimate_tokens"], 9_000);
        assert_eq!(value["task"]["requires_approval"], true);
        assert_eq!(value["snapshot"]["stats"]["provider_requests"], 1);
        assert_eq!(value["snapshot"]["stats"]["waiting_approval"], 1);
        assert_eq!(
            value["snapshot"]["stats"]["waiting_approval_estimated_tokens"],
            9_000
        );
        let metadata_path = value["task"]["request_metadata_path"].as_str().unwrap();
        assert!(
            metadata_path.ends_with(".0-provider-approval__worldlabs-marble-request.json"),
            "{metadata_path}"
        );
        assert!(Path::new(metadata_path).exists());
        assert_eq!(
            value["snapshot"]["provider_requests"][0]["metadata_path"],
            metadata_path
        );
        let handoff: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(metadata_path).unwrap()).unwrap();
        assert_eq!(handoff["kind"], "pool_provider_approval_handoff");
        assert_eq!(handoff["status"], "waiting_approval");
        assert_eq!(handoff["provider_id"], "worldlabs-marble");
        assert_eq!(handoff["provider_request"]["require_approval"], true);
        assert_eq!(
            handoff["ledger"]["task"]["request_metadata_path"],
            metadata_path
        );
        let metadata_response = server
            .handle_path(&format!(
                "/api/provider-requests/metadata?provider_request_id={}",
                value["provider_request_id"].as_str().unwrap()
            ))
            .unwrap();
        let metadata_value: serde_json::Value =
            serde_json::from_str(&metadata_response.body).unwrap();
        assert_eq!(metadata_response.status_code, 200);
        assert_eq!(metadata_value["provider_id"], "worldlabs-marble");
        assert_eq!(metadata_value["metadata_path"], metadata_path);
        assert_eq!(
            metadata_value["metadata"]["kind"],
            "pool_provider_approval_handoff"
        );
    }

    #[test]
    fn post_approve_provider_run_resumes_from_provider_request_ledger() {
        let db_path = temp_db_path("runtime-http-provider-run-approve-resume");
        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let queued = server
            .handle_request_with_body(
                "POST",
                "/api/provider-runs",
                r#"{
                    "provider_id":"world-labs-marble",
                    "execution_mode":"mock",
                    "task_title":"World Labs Marble approve resume run",
                    "output_dir":"target/runtime-http-provider-run-approve-resume/worlds/demo/output"
                }"#,
            )
            .unwrap();
        let queued_value: serde_json::Value = serde_json::from_str(&queued.body).unwrap();
        let task_id = queued_value["task"]["id"].as_str().unwrap().to_string();
        let provider_request_id = queued_value["provider_request_id"]
            .as_str()
            .unwrap()
            .to_string();

        assert_eq!(queued.status_code, 200);
        assert_eq!(queued_value["report"]["status"], "WaitingApproval");
        assert_eq!(queued_value["snapshot"]["stats"]["provider_requests"], 1);
        assert_eq!(queued_value["snapshot"]["stats"]["waiting_approval"], 1);

        let approved = server
            .handle_request("POST", &format!("/api/tasks/approve?task_id={task_id}"))
            .unwrap();
        let approved_value: serde_json::Value = serde_json::from_str(&approved.body).unwrap();

        assert_eq!(approved.status_code, 200);
        assert_eq!(approved_value["report"]["status"], "Succeeded");
        assert_eq!(
            approved_value["provider_request_id"].as_str(),
            Some(provider_request_id.as_str())
        );
        assert_eq!(
            approved_value["task"]["id"].as_str(),
            Some(task_id.as_str())
        );
        assert_eq!(approved_value["task"]["status"], "Succeeded");
        assert_eq!(
            approved_value["task"]["request_metadata_path"],
            "target/runtime-http-provider-run-approve-resume/worlds/demo/output/.1-world-request.json"
        );
        assert_eq!(
            approved_value["report"]["assets"].as_array().unwrap().len(),
            3
        );
        assert_eq!(approved_value["snapshot"]["stats"]["waiting_approval"], 0);
        assert_eq!(approved_value["snapshot"]["stats"]["assets"], 3);
        assert_eq!(approved_value["snapshot"]["stats"]["provider_requests"], 1);

        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        let request_record = repository
            .latest_provider_request(&task_id)
            .unwrap()
            .expect("provider request ledger");
        assert_eq!(request_record.id, provider_request_id);
        assert_eq!(request_record.provider_id, "worldlabs-marble");
        assert_eq!(
            request_record.response_json.unwrap()["status"],
            serde_json::json!("Succeeded")
        );
        assert_eq!(repository.table_count("provider_requests").unwrap(), 1);
    }

    #[test]
    fn post_retry_provider_run_appends_provider_request_attempt() {
        let gateway = spawn_fake_failed_3dgs_gateway();
        let db_path = temp_db_path("runtime-http-provider-run-retry-ledger");
        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let first = server
            .handle_request_with_body(
                "POST",
                "/api/provider-runs",
                &format!(
                    r#"{{
                    "provider_id":"world-labs-marble",
                    "execution_mode":"gateway",
                    "endpoint":"{gateway}",
                    "task_title":"World Labs Marble failed run",
                    "prompt":"attempt a gateway conversion",
                    "output_dir":"target/runtime-http-provider-run-retry-ledger/worlds/demo/output",
                    "requires_approval":false
                }}"#
                ),
            )
            .unwrap();
        let first_value: serde_json::Value = serde_json::from_str(&first.body).unwrap();
        let task_id = first_value["task"]["id"].as_str().unwrap().to_string();
        let first_provider_request_id = first_value["provider_request_id"]
            .as_str()
            .unwrap()
            .to_string();

        assert_eq!(first.status_code, 200);
        assert_eq!(first_value["report"]["status"], "Failed");
        assert_eq!(first_value["snapshot"]["stats"]["provider_requests"], 1);
        assert_eq!(first_value["task"]["status"], "Failed");

        let retry = server
            .handle_request("POST", &format!("/api/tasks/retry?task_id={task_id}"))
            .unwrap();
        let retry_value: serde_json::Value = serde_json::from_str(&retry.body).unwrap();
        let retry_provider_request_id = retry_value["provider_request_id"]
            .as_str()
            .unwrap()
            .to_string();

        assert_eq!(retry.status_code, 200);
        assert_eq!(retry_value["report"]["status"], "Failed");
        assert_eq!(retry_value["task"]["id"].as_str(), Some(task_id.as_str()));
        assert_ne!(retry_provider_request_id, first_provider_request_id);
        assert_eq!(retry_value["snapshot"]["stats"]["provider_requests"], 2);

        let provider_requests = retry_value["snapshot"]["provider_requests"]
            .as_array()
            .unwrap();
        assert_eq!(
            provider_requests
                .iter()
                .filter(|request| request["task_id"] == task_id)
                .count(),
            2
        );
        let retry_request = provider_requests
            .iter()
            .find(|request| request["id"] == retry_provider_request_id)
            .expect("retry provider request");
        assert_eq!(retry_request["request"]["attempt"]["kind"], "retry");
        assert_eq!(
            retry_request["request"]["attempt"]["retry_of_provider_request_id"],
            first_provider_request_id
        );
    }

    #[test]
    fn post_provider_run_uses_saved_openai_key() {
        let endpoint = spawn_fake_openai_image_server();
        let db_path = temp_db_path("runtime-http-provider-run-openai-key");
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        repository
            .upsert_api_key(
                "openai-image-2",
                "provider",
                "sk-runtime-secret",
                serde_json::json!({"env":"OPENAI_API_KEY"}),
            )
            .unwrap();
        drop(repository);

        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let response = server
            .handle_request_with_body(
                "POST",
                "/api/provider-runs",
                &format!(
                    r#"{{
                    "provider_id":"openai-image-2",
                    "execution_mode":"adapter",
                    "endpoint":"{endpoint}/v1",
                    "task_title":"OpenAI saved key run",
                    "prompt":"generate saved key image",
                    "output_dir":"target/runtime-http-openai-key/output",
                    "requires_approval":false
                }}"#
                ),
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 200);
        assert_eq!(value["report"]["status"], "Succeeded");
        assert_eq!(value["report"]["provider_id"], "openai-image-2");
        assert_eq!(value["report"]["assets"].as_array().unwrap().len(), 1);
        assert_eq!(value["snapshot"]["stats"]["api_keys"], 1);
        assert!(!response.body.contains("sk-runtime-secret"));
    }

    #[test]
    fn post_provider_run_submits_openai_image_edit_with_local_input() {
        let endpoint = spawn_fake_openai_image_server();
        let db_path = temp_db_path("runtime-http-provider-run-openai-edit");
        let input_dir = temp_control_dir("runtime-http-provider-run-openai-edit-input");
        std::fs::create_dir_all(&input_dir).unwrap();
        let output_dir = temp_control_dir("runtime-http-provider-run-openai-edit-output");
        std::fs::create_dir_all(&output_dir).unwrap();
        let image_path = input_dir.join("input.png");
        std::fs::write(&image_path, b"edit-input").unwrap();
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        repository
            .upsert_api_key(
                "openai-image-2",
                "provider",
                "sk-runtime-secret",
                serde_json::json!({"env":"OPENAI_API_KEY"}),
            )
            .unwrap();
        drop(repository);

        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let prompt = serde_json::to_string(&serde_json::json!({
            "operation": "edit",
            "prompt": "replace backdrop",
            "image": image_path.to_string_lossy(),
            "output_format": "png"
        }))
        .unwrap();
        let response = server
            .handle_request_with_body(
                "POST",
                "/api/provider-runs",
                &format!(
                    r#"{{
                    "provider_id":"openai-image-2",
                    "execution_mode":"adapter",
                    "endpoint":"{endpoint}/v1",
                    "task_title":"OpenAI edit run",
                    "prompt":{},
                    "output_dir":{},
                    "requires_approval":false
                }}"#,
                    serde_json::to_string(&prompt).unwrap(),
                    serde_json::to_string(&output_dir.to_string_lossy()).unwrap()
                ),
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 200);
        assert_eq!(value["report"]["status"], "Succeeded");
        assert_eq!(value["report"]["provider_id"], "openai-image-2");
        assert_eq!(value["report"]["assets"].as_array().unwrap().len(), 1);
        let metadata_path = value["report"]["job"]["request_metadata_path"]
            .as_str()
            .unwrap();
        let metadata: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(metadata_path).unwrap()).unwrap();
        assert_eq!(metadata["operation"], "edit");
        assert_eq!(metadata["endpoint_path"], "/images/edits");
        assert_eq!(
            metadata["request"]["image_paths"][0].as_str(),
            Some(image_path.to_string_lossy().as_ref())
        );
        assert!(metadata["request"].get("image").is_none());
        assert!(!response.body.contains("sk-runtime-secret"));

        let input_paths_output_dir =
            temp_control_dir("runtime-http-provider-run-openai-edit-input-paths-output");
        std::fs::create_dir_all(&input_paths_output_dir).unwrap();
        let input_paths_endpoint = spawn_fake_openai_image_server();
        let input_paths_response = server
            .handle_request_with_body(
                "POST",
                "/api/provider-runs",
                &format!(
                    r#"{{
                    "provider_id":"openai-image-2",
                    "execution_mode":"adapter",
                    "endpoint":"{input_paths_endpoint}/v1",
                    "task_title":"OpenAI edit run from input_paths",
                    "prompt":"extend this source image",
                    "input_paths":[{}],
                    "output_dir":{},
                    "requires_approval":false
                }}"#,
                    serde_json::to_string(&image_path.to_string_lossy()).unwrap(),
                    serde_json::to_string(&input_paths_output_dir.to_string_lossy()).unwrap()
                ),
            )
            .unwrap();
        let input_paths_value: serde_json::Value =
            serde_json::from_str(&input_paths_response.body).unwrap();
        assert_eq!(input_paths_response.status_code, 200);
        assert_eq!(input_paths_value["report"]["status"], "Succeeded");
        let input_paths_metadata_path = input_paths_value["report"]["job"]["request_metadata_path"]
            .as_str()
            .unwrap();
        let input_paths_metadata: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(input_paths_metadata_path).unwrap())
                .unwrap();
        assert_eq!(input_paths_metadata["operation"], "edit");
        assert_eq!(input_paths_metadata["endpoint_path"], "/images/edits");
        assert_eq!(
            input_paths_metadata["request"]["image_paths"][0].as_str(),
            Some(image_path.to_string_lossy().as_ref())
        );
        assert!(input_paths_metadata["request"]
            .get("input_images")
            .is_none());
        let _ = std::fs::remove_dir_all(input_dir);
        let _ = std::fs::remove_dir_all(output_dir);
        let _ = std::fs::remove_dir_all(input_paths_output_dir);
    }

    #[test]
    fn post_provider_run_submits_kling_image2video_with_local_input_path() {
        let endpoint = spawn_fake_kling_video_server();
        let db_path = temp_db_path("runtime-http-provider-run-kling-input-path");
        let input_dir = temp_control_dir("runtime-http-provider-run-kling-input-path");
        std::fs::create_dir_all(&input_dir).unwrap();
        let output_dir = temp_control_dir("runtime-http-provider-run-kling-output");
        std::fs::create_dir_all(&output_dir).unwrap();
        let image_path = input_dir.join("concept.png");
        std::fs::write(&image_path, b"kling-image-input").unwrap();
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        repository
            .upsert_api_key(
                "kling",
                "provider",
                "kling-runtime-secret",
                serde_json::json!({"env":"POOL_KLING_API_KEY"}),
            )
            .unwrap();
        drop(repository);

        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let response = server
            .handle_request_with_body(
                "POST",
                "/api/provider-runs",
                &format!(
                    r#"{{
                    "provider_id":"kling",
                    "execution_mode":"adapter",
                    "endpoint":"{endpoint}",
                    "task_title":"Kling image2video input path run",
                    "prompt":"{{\"prompt\":\"animate this local concept\",\"duration\":5}}",
                    "input_paths":[{}],
                    "output_dir":{},
                    "requires_approval":false
                }}"#,
                    serde_json::to_string(&image_path.to_string_lossy()).unwrap(),
                    serde_json::to_string(&output_dir.to_string_lossy()).unwrap()
                ),
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 200);
        assert_eq!(value["report"]["status"], "Succeeded");
        assert_eq!(value["report"]["provider_id"], "kling");
        assert_eq!(value["report"]["assets"].as_array().unwrap().len(), 1);
        let metadata_path = value["report"]["job"]["request_metadata_path"]
            .as_str()
            .unwrap();
        let metadata: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(metadata_path).unwrap()).unwrap();
        assert_eq!(
            metadata["request"]["image"],
            "local_image_data_url_redacted"
        );
        assert_eq!(
            metadata["request"]["local_input_paths"][0].as_str(),
            Some(image_path.to_string_lossy().as_ref())
        );
        assert!(!metadata.to_string().contains("a2xpbmctaW1hZ2UtaW5wdXQ"));
        assert!(!response.body.contains("kling-runtime-secret"));
        let _ = std::fs::remove_dir_all(input_dir);
        let _ = std::fs::remove_dir_all(output_dir);
    }

    #[test]
    fn post_provider_run_executes_three_dgs_gateway_adapter_when_requested() {
        let gateway = spawn_provider_gateway_mock(6).unwrap();
        let db_path = temp_db_path("runtime-http-provider-run-gateway");
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        let plan = build_default_content_burst_plan("demo", "Pool HTTP demo");
        repository.persist_plan(&plan).unwrap();
        drop(repository);

        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let response = server
            .handle_request_with_body(
                "POST",
                "/api/provider-runs",
                &format!(
                    r#"{{
                    "provider_id":"world-labs-marble",
                    "execution_mode":"gateway",
                    "endpoint":"{gateway}",
                    "task_title":"World Labs Marble gateway run",
                    "prompt":"convert concept plate into 3DGS world",
                    "output_dir":"target/runtime-http-provider-run-gateway/worlds/demo/output",
                    "requires_approval":false
                }}"#
                ),
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 200);
        assert_eq!(value["report"]["status"], "Succeeded");
        assert_eq!(value["report"]["provider_id"], "worldlabs-marble");
        assert_eq!(value["report"]["assets"].as_array().unwrap().len(), 3);
        assert_eq!(
            value["report"]["assets"][0]["local_path"],
            "target/runtime-http-provider-run-gateway/worlds/demo/output/1-world.json"
        );
        assert_eq!(
            value["report"]["assets"][2]["local_path"],
            "target/runtime-http-provider-run-gateway/worlds/demo/output/3-world-full_res.spz"
        );
        assert_eq!(value["snapshot"]["stats"]["assets"], 3);
    }

    #[test]
    fn post_provider_run_executes_generic_media_gateway_for_nano_banana() {
        let gateway = spawn_provider_gateway_mock(4).unwrap();
        let db_path = temp_db_path("runtime-http-provider-run-media-gateway");
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        let plan = build_default_content_burst_plan("demo", "Pool HTTP demo");
        repository.persist_plan(&plan).unwrap();
        drop(repository);

        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let response = server
            .handle_request_with_body(
                "POST",
                "/api/provider-runs",
                &format!(
                    r#"{{
                    "provider_id":"nanobananapro",
                    "execution_mode":"gateway",
                    "endpoint":"{gateway}",
                    "task_title":"Nano Banana Pro gateway run",
                    "prompt":"{{\"prompt\":\"generate hero plate\",\"output_slug\":\"nano\",\"output_extension\":\"png\"}}",
                    "output_dir":"target/runtime-http-provider-run-media-gateway/worlds/demo/output",
                    "requires_approval":false
                }}"#
                ),
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 200);
        assert_eq!(value["report"]["status"], "Succeeded");
        assert_eq!(value["report"]["provider_id"], "nano-banana-pro");
        assert_eq!(value["report"]["assets"].as_array().unwrap().len(), 1);
        assert_eq!(
            value["report"]["assets"][0]["local_path"],
            "target/runtime-http-provider-run-media-gateway/worlds/demo/output/1-nano-nano-mock.png"
        );
        assert_eq!(value["snapshot"]["stats"]["assets"], 1);
    }

    #[test]
    fn provider_run_reports_missing_real_provider_auth() {
        let db_path = temp_db_path("runtime-http-provider-run-unsupported");
        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));

        let response = server
            .handle_request_with_body(
                "POST",
                "/api/provider-runs",
                r#"{"provider_id":"openai-image-2","task_title":"image run"}"#,
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 409);
        assert_eq!(value["error"], "provider_not_configured");
    }

    #[test]
    fn provider_run_rejects_unsupported_provider() {
        let db_path = temp_db_path("runtime-http-provider-run-unsupported");
        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));

        let response = server
            .handle_request_with_body(
                "POST",
                "/api/provider-runs",
                r#"{"provider_id":"unknown-ai","task_title":"unknown run"}"#,
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 501);
        assert_eq!(value["error"], "provider_not_executable");
    }

    #[test]
    fn post_node_run_dispatches_provider_node_from_workflow() {
        let db_path = temp_db_path("runtime-http-node-run-provider");
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        let plan = build_default_content_burst_plan("demo", "Pool HTTP demo");
        let node_id = plan
            .workflow
            .nodes
            .iter()
            .find(|(_, node)| node.node_type == NodeType::ThreeDgs)
            .map(|(id, _)| id.clone())
            .unwrap();
        repository.persist_plan(&plan).unwrap();
        drop(repository);

        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let response = server
            .handle_request_with_body(
                "POST",
                "/api/nodes/run",
                &format!(r#"{{"project_slug":"demo","node_id":"{node_id}"}}"#),
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 200);
        assert_eq!(value["report"]["provider_id"], "worldlabs-marble");
        assert_eq!(value["report"]["status"], "WaitingApproval");
        assert_eq!(value["task"]["node_id"], node_id);
        assert_eq!(value["snapshot"]["stats"]["provider_requests"], 1);
        assert_eq!(
            value["snapshot"]["provider_requests"][0]["request"]["control_context"]["provider"]
                ["id"],
            "worldlabs-marble"
        );
        assert_eq!(
            value["snapshot"]["provider_requests"][0]["request"]["control_context"]["mcp_tools"][1]
                ["name"],
            "pool_run_provider"
        );
    }

    #[test]
    fn post_node_run_dispatches_software_node_from_workflow() {
        let db_path = temp_db_path("runtime-http-node-run-software");
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        let plan = build_default_content_burst_plan("demo", "Pool HTTP demo");
        let node_id = plan
            .workflow
            .nodes
            .iter()
            .find(|(_, node)| node.node_type == NodeType::Unreal)
            .map(|(id, _)| id.clone())
            .unwrap();
        repository.persist_plan(&plan).unwrap();
        drop(repository);

        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let response = server
            .handle_request_with_body(
                "POST",
                "/api/nodes/run",
                &format!(r#"{{"project_slug":"demo","node_id":"{node_id}"}}"#),
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 200);
        assert_eq!(value["report"]["adapter_id"], "unreal");
        assert_eq!(value["report"]["status"], "Succeeded");
        assert_eq!(value["task"]["node_id"], node_id);
        assert_eq!(value["snapshot"]["stats"]["software_actions"], 1);
        assert_eq!(
            value["snapshot"]["software_actions"][0]["command"]["payload_json"]["control_context"]
                ["software_adapter"]["id"],
            "unreal"
        );
        assert_eq!(
            value["snapshot"]["software_actions"][0]["command"]["payload_json"]["control_context"]
                ["mcp_tools"][1]["name"],
            "pool_run_software"
        );
    }

    #[test]
    fn post_software_health_checks_adapter_without_creating_audit_rows() {
        let db_path = temp_db_path("runtime-http-software-health");
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        drop(repository);

        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let response = server
            .handle_request_with_body(
                "POST",
                "/api/software-health",
                r#"{
                    "adapter_id":"blender",
                    "priority":"SkillsCli"
                }"#,
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 200);
        assert_eq!(value["adapter_id"], "blender");
        assert_eq!(value["adapter_mode"], "cli");
        assert_eq!(value["health"]["ok"], true);

        let repository = RuntimeRepository::open(&db_path).unwrap();
        assert_eq!(repository.table_count("tasks").unwrap(), 0);
        assert_eq!(repository.table_count("software_actions").unwrap(), 0);
    }

    #[test]
    fn post_software_health_runs_generic_api_mcp_when_endpoint_is_configured() {
        let endpoint = spawn_fake_generic_software_api_server("resolve");
        let db_path = temp_db_path("runtime-http-software-health-generic-api");
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        drop(repository);

        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let response = server
            .handle_request_with_body(
                "POST",
                "/api/software-health",
                &format!(
                    r#"{{
                    "adapter_id":"resolve",
                    "priority":"ApiMcp",
                    "payload_json":{{"endpoint":"{endpoint}"}}
                }}"#
                ),
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 200);
        assert_eq!(value["adapter_id"], "resolve");
        assert_eq!(value["adapter_mode"], "api_mcp");
        assert_eq!(value["health"]["ok"], true);
        assert_eq!(value["health"]["message"], "generic-software-health-ok");

        let repository = RuntimeRepository::open(&db_path).unwrap();
        assert_eq!(repository.table_count("tasks").unwrap(), 0);
        assert_eq!(repository.table_count("software_actions").unwrap(), 0);
    }

    #[test]
    fn post_software_health_reports_unreal_mock_and_hermes_missing_endpoint() {
        let db_path = temp_db_path("runtime-http-software-health-modes");
        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));

        let unreal = server
            .handle_request_with_body(
                "POST",
                "/api/software-health",
                r#"{"adapter_id":"unreal","priority":"ApiMcp"}"#,
            )
            .unwrap();
        let unreal_value: serde_json::Value = serde_json::from_str(&unreal.body).unwrap();

        assert_eq!(unreal.status_code, 200);
        assert_eq!(unreal_value["adapter_mode"], "mock");
        assert_eq!(unreal_value["health"]["ok"], true);

        let hermes = server
            .handle_request_with_body(
                "POST",
                "/api/software-health",
                r#"{"adapter_id":"hermes","priority":"ApiMcp"}"#,
            )
            .unwrap();
        let hermes_value: serde_json::Value = serde_json::from_str(&hermes.body).unwrap();

        assert_eq!(hermes.status_code, 200);
        assert_eq!(hermes_value["adapter_mode"], "mcp");
        assert_eq!(hermes_value["health"]["ok"], false);
        assert!(hermes_value["health"]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("endpoint is not configured"));

        let invalid = server
            .handle_request_with_body("POST", "/api/software-health", r#"{"adapter_id":" "}"#)
            .unwrap();
        assert_eq!(invalid.status_code, 400);
    }

    #[test]
    fn post_software_action_runs_mock_unreal_and_records_audit() {
        let db_path = temp_db_path("runtime-http-software-action");
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        let plan = build_default_content_burst_plan("demo", "Pool HTTP demo");
        repository.persist_plan(&plan).unwrap();
        drop(repository);

        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let response = server
            .handle_request_with_body(
                "POST",
                "/api/software-actions",
                r#"{
                    "adapter_id":"unreal",
                    "action_kind":"CreateScene",
                    "priority":"ApiMcp",
                    "task_title":"Unreal scene assembly",
                    "payload_json":{"level":"demo"}
                }"#,
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 200);
        assert_eq!(value["report"]["status"], "Succeeded");
        assert_eq!(value["report"]["adapter_id"], "unreal");
        assert_eq!(value["snapshot"]["stats"]["software_actions"], 1);
        assert!(value["snapshot"]["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|event| event["message"]
                .as_str()
                .unwrap_or_default()
                .contains("software action finished")));
    }

    #[test]
    fn get_production_evidence_item_from_software_ledger_builds_submit_item() {
        let db_path = temp_db_path("runtime-http-software-ledger-evidence-item");
        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let response = server
            .handle_request_with_body(
                "POST",
                "/api/software-actions",
                r#"{
                    "adapter_id":"unreal",
                    "action_kind":"CreateScene",
                    "priority":"ApiMcp",
                    "task_title":"Unreal scene assembly ledger",
                    "payload_json":{"level":"demo"}
                }"#,
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();
        let software_action_id = value["snapshot"]["software_actions"][0]["id"]
            .as_str()
            .unwrap();

        let evidence = server
            .handle_path(&format!(
                "/api/production-evidence/item-from-ledger?software_action_id={software_action_id}&source=ledger-test"
            ))
            .unwrap();
        let evidence_value: serde_json::Value = serde_json::from_str(&evidence.body).unwrap();

        assert_eq!(evidence.status_code, 200);
        assert_eq!(evidence_value["item"]["kind"], "software_action");
        assert_eq!(
            evidence_value["item"]["software_action"]["adapter_id"],
            "unreal"
        );
        assert_eq!(
            evidence_value["item"]["software_action"]["artifacts"][0],
            "unreal://mock/viewport"
        );
        assert_eq!(evidence_value["validation"]["valid"], false);
        assert!(evidence_value["validation"]["message"]
            .as_str()
            .unwrap()
            .contains("template placeholder"));
        assert_eq!(
            evidence_value["validation"]["production_flags"]["complete"],
            false
        );
        assert_eq!(evidence_value["ready_for_import"], false);
    }

    #[test]
    fn post_software_action_runs_unreal_mcp_when_endpoint_is_configured() {
        let endpoint = spawn_fake_unreal_mcp_server();
        let db_path = temp_db_path("runtime-http-software-action-unreal-mcp");
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        let plan = build_default_content_burst_plan("demo", "Pool HTTP demo");
        repository.persist_plan(&plan).unwrap();
        drop(repository);

        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let response = server
            .handle_request_with_body(
                "POST",
                "/api/software-actions",
                &format!(
                    r#"{{
                    "adapter_id":"unreal",
                    "action_kind":"CreateScene",
                    "priority":"ApiMcp",
                    "task_title":"Unreal MCP scene assembly",
                    "payload_json":{{
                        "endpoint":"{endpoint}",
                        "level":"demo",
                        "assets":["worlds/demo/output/1-world.glb"]
                    }}
                }}"#
                ),
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 200);
        assert_eq!(value["report"]["status"], "Succeeded");
        assert_eq!(value["report"]["adapter_id"], "unreal");
        assert_eq!(value["report"]["result"]["message"], "unreal-action-ok");
        assert_eq!(
            value["report"]["result"]["artifacts"][0],
            "unreal://level/demo"
        );
        assert_eq!(value["snapshot"]["stats"]["software_actions"], 1);
    }

    #[test]
    fn post_software_action_runs_hermes_mcp_when_endpoint_is_configured() {
        let endpoint = spawn_fake_hermes_mcp_server();
        let db_path = temp_db_path("runtime-http-software-action-hermes-mcp");
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        let plan = build_default_content_burst_plan("demo", "Pool HTTP demo");
        repository.persist_plan(&plan).unwrap();
        drop(repository);

        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let response = server
            .handle_request_with_body(
                "POST",
                "/api/software-actions",
                &format!(
                    r#"{{
                    "adapter_id":"hermes",
                    "action_kind":"CreateScene",
                    "priority":"ApiMcp",
                    "task_title":"Hermes MCP orchestration",
                    "payload_json":{{
                        "endpoint":"{endpoint}",
                        "project_slug":"demo",
                        "instruction":"coordinate Unreal scene assembly",
                        "allowed_tools":["unreal","filesystem"],
                        "target_adapter":"unreal"
                    }}
                }}"#
                ),
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 200);
        assert_eq!(value["report"]["status"], "Succeeded");
        assert_eq!(value["report"]["adapter_id"], "hermes");
        assert_eq!(value["report"]["result"]["message"], "hermes-mcp-ok");
        assert!(value["report"]["result"]["artifacts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|artifact| artifact == "hermes://session/1"));
        assert_eq!(value["snapshot"]["stats"]["software_actions"], 1);
    }

    #[test]
    fn post_software_action_runs_generic_api_mcp_when_endpoint_is_configured() {
        let endpoint = spawn_fake_generic_software_api_server("resolve");
        let db_path = temp_db_path("runtime-http-software-action-generic-api");
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        let plan = build_default_content_burst_plan("demo", "Pool HTTP demo");
        repository.persist_plan(&plan).unwrap();
        drop(repository);

        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let response = server
            .handle_request_with_body(
                "POST",
                "/api/software-actions",
                &format!(
                    r#"{{
                    "adapter_id":"resolve",
                    "action_kind":"CreateScene",
                    "priority":"ApiMcp",
                    "task_title":"Resolve API production evidence",
                    "payload_json":{{
                        "endpoint":"{endpoint}",
                        "project_slug":"demo",
                        "artifacts":["worlds/demo/output/production/resolve/1-edit.mov"]
                    }}
                }}"#
                ),
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 200);
        assert_eq!(value["report"]["status"], "Succeeded");
        assert_eq!(value["report"]["adapter_id"], "resolve");
        assert_eq!(value["report"]["result"]["message"], "resolve-api-ok");
        assert_eq!(
            value["report"]["result"]["artifacts"][0],
            "worlds/demo/output/production/resolve/1-edit.mov"
        );
        assert_eq!(value["snapshot"]["stats"]["software_actions"], 1);
    }

    #[test]
    fn post_software_action_runs_command_adapter_for_blender() {
        let db_path = temp_db_path("runtime-http-software-action-command");
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        let plan = build_default_content_burst_plan("demo", "Pool HTTP demo");
        repository.persist_plan(&plan).unwrap();
        drop(repository);

        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let response = server
            .handle_request_with_body(
                "POST",
                "/api/software-actions",
                r#"{
                    "adapter_id":"blender",
                    "action_kind":"ExecuteCli",
                    "priority":"SkillsCli",
                    "task_title":"Blender CLI smoke",
                    "payload_json":{
                        "command":"/bin/echo blender-runtime-ok",
                        "allowed_commands":["/bin/echo","echo"],
                        "timeout_ms":2000,
                        "max_output_bytes":1024,
                        "artifacts":["blender://script/smoke"]
                    }
                }"#,
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 200);
        assert_eq!(value["report"]["status"], "Succeeded");
        assert_eq!(value["report"]["adapter_id"], "blender");
        assert!(value["report"]["result"]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("blender-runtime-ok"));
        assert_eq!(value["snapshot"]["stats"]["software_actions"], 1);
    }

    #[test]
    fn approve_task_resumes_confirmed_software_action() {
        let db_path = temp_db_path("runtime-http-software-action-approve-resume");
        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));

        let staged = server
            .handle_request_with_body(
                "POST",
                "/api/software-actions",
                r#"{
                    "project_slug":"demo",
                    "adapter_id":"blender",
                    "action_kind":"ExecuteCli",
                    "priority":"SkillsCli",
                    "task_title":"Confirm Blender CLI smoke",
                    "requires_confirmation":true,
                    "payload_json":{
                        "command":"/bin/echo approved-runtime-ok",
                        "allowed_commands":["/bin/echo","echo"],
                        "timeout_ms":2000,
                        "max_output_bytes":1024
                    }
                }"#,
            )
            .unwrap();
        let staged_value: serde_json::Value = serde_json::from_str(&staged.body).unwrap();
        let task_id = staged_value["task"]["id"].as_str().unwrap().to_string();

        assert_eq!(staged.status_code, 200);
        assert_eq!(staged_value["report"]["status"], "WaitingApproval");
        assert_eq!(staged_value["task"]["status"], "WaitingApproval");
        assert_eq!(staged_value["snapshot"]["stats"]["software_actions"], 1);
        assert_eq!(staged_value["snapshot"]["stats"]["waiting_approval"], 1);

        let approved = server
            .handle_request("POST", &format!("/api/tasks/approve?task_id={task_id}"))
            .unwrap();
        let approved_value: serde_json::Value = serde_json::from_str(&approved.body).unwrap();

        assert_eq!(approved.status_code, 200);
        assert_eq!(approved_value["report"]["status"], "Succeeded");
        assert_eq!(
            approved_value["task"]["id"].as_str(),
            Some(task_id.as_str())
        );
        assert_eq!(approved_value["task"]["status"], "Succeeded");
        assert!(approved_value["report"]["result"]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("approved-runtime-ok"));
        assert_eq!(approved_value["snapshot"]["stats"]["software_actions"], 2);
        assert_eq!(approved_value["snapshot"]["stats"]["waiting_approval"], 0);
    }

    #[test]
    fn post_software_action_stages_desktop_recognition_request() {
        let db_path = temp_db_path("runtime-http-software-action-desktop-recognition");
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        let plan = build_default_content_burst_plan("demo", "Pool HTTP demo");
        repository.persist_plan(&plan).unwrap();
        drop(repository);

        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let response = server
            .handle_request_with_body(
                "POST",
                "/api/software-actions",
                r#"{
                    "adapter_id":"touchdesigner",
                    "action_kind":"RunViewport",
                    "priority":"DesktopRecognition",
                    "task_title":"TouchDesigner cue",
                    "payload_json":{
                        "scene":"demo",
                        "instruction":"find the TouchDesigner perform window and trigger cue 1",
                        "target_window":"TouchDesigner",
                        "visual_targets":["Perform","Cue 1"]
                    }
                }"#,
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 200);
        assert_eq!(value["task"]["status"], "Succeeded");
        assert_eq!(value["snapshot"]["stats"]["software_actions"], 1);
        let request_path = value["report"]["result"]["artifacts"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|artifact| artifact.as_str())
            .find(|artifact| artifact.ends_with(".json"))
            .unwrap();
        assert!(std::path::Path::new(request_path).exists());
        let request_body: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(request_path).unwrap()).unwrap();
        assert_eq!(
            request_body["pool_desktop_action"]["profile_id"],
            "desktop-run-preview"
        );
        assert_eq!(
            request_body["desktop_payload"]["tool"],
            "desktop.run_preview"
        );
        assert_eq!(
            request_body["desktop_payload"]["target_window"],
            "TouchDesigner"
        );

        let queue_response = server
            .handle_request("GET", "/api/desktop-recognition/requests")
            .unwrap();
        let queue_value: serde_json::Value = serde_json::from_str(&queue_response.body).unwrap();

        assert_eq!(queue_response.status_code, 200);
        assert_eq!(queue_value["count"], 1);
        assert_eq!(
            queue_value["requests"][0]["pool_desktop_action"]["profile_id"],
            "desktop-run-preview"
        );
        assert_eq!(
            queue_value["requests"][0]["desktop_payload"]["tool"],
            "desktop.run_preview"
        );
    }

    #[test]
    fn desktop_recognition_result_callback_updates_action_and_task() {
        let db_path = temp_db_path("runtime-http-desktop-recognition-result");
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        let plan = build_default_content_burst_plan("demo", "Pool HTTP demo");
        repository.persist_plan(&plan).unwrap();
        drop(repository);

        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let stage_response = server
            .handle_request_with_body(
                "POST",
                "/api/software-actions",
                r#"{
                    "adapter_id":"touchdesigner",
                    "action_kind":"RunViewport",
                    "priority":"DesktopRecognition",
                    "task_title":"TouchDesigner cue",
                    "payload_json":{
                        "instruction":"trigger cue 1",
                        "target_window":"TouchDesigner",
                        "visual_targets":["Perform","Cue 1"]
                    }
                }"#,
            )
            .unwrap();
        let stage_value: serde_json::Value = serde_json::from_str(&stage_response.body).unwrap();
        let action_id = stage_value["report"]["action_id"].as_str().unwrap();
        let task_id = stage_value["report"]["task_id"].as_str().unwrap();

        let result_response = server
            .handle_request_with_body(
                "POST",
                "/api/desktop-recognition/results",
                &format!(
                    r#"{{
                    "software_action_id":"{action_id}",
                    "task_id":"{task_id}",
                    "status":"failed",
                    "message":"TouchDesigner window was not visible",
                    "screen_trace_path":"worlds/demo/output/control/desktop-recognition/trace.json",
                    "artifacts":["worlds/demo/output/control/desktop-recognition/trace.json"],
                    "result":{{"controller":"desktop-vision","attempts":1}}
                }}"#
                ),
            )
            .unwrap();
        let result_value: serde_json::Value = serde_json::from_str(&result_response.body).unwrap();

        assert_eq!(result_response.status_code, 200);
        assert_eq!(result_value["task"]["status"], "Failed");
        assert_eq!(
            result_value["software_action"]["verification"]["desktop_recognition_status"],
            "failed"
        );
        assert_eq!(
            result_value["software_action"]["verification"]["controller_result"]["controller"],
            "desktop-vision"
        );
        assert!(result_value["snapshot"]["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|event| event["message"]
                .as_str()
                .unwrap_or_default()
                .contains("desktop recognition result")));

        let queue_response = server
            .handle_request("GET", "/api/desktop-recognition/requests")
            .unwrap();
        let queue_value: serde_json::Value = serde_json::from_str(&queue_response.body).unwrap();

        assert_eq!(queue_value["count"], 0);
    }

    #[test]
    fn get_production_evidence_item_from_desktop_vision_ledger_builds_submit_item() {
        let db_path = temp_db_path("runtime-http-desktop-ledger-evidence-item");
        let artifact_root = temp_control_dir("runtime-http-desktop-ledger-evidence-item");
        let trace_path = write_temp_artifact(&artifact_root, "desktop/external-vision-trace.json");
        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let stage_response = server
            .handle_request_with_body(
                "POST",
                "/api/software-actions",
                r#"{
                    "adapter_id":"touchdesigner",
                    "action_kind":"RunViewport",
                    "priority":"DesktopRecognition",
                    "task_title":"TouchDesigner external vision cue",
                    "payload_json":{
                        "instruction":"trigger cue 1",
                        "target_window":"TouchDesigner",
                        "visual_targets":["Perform","Cue 1"]
                    }
                }"#,
            )
            .unwrap();
        let stage_value: serde_json::Value = serde_json::from_str(&stage_response.body).unwrap();
        let action_id = stage_value["report"]["action_id"].as_str().unwrap();
        let task_id = stage_value["report"]["task_id"].as_str().unwrap();
        let result_response = server
            .handle_request_with_body(
                "POST",
                "/api/desktop-recognition/results",
                &json!({
                    "software_action_id": action_id,
                    "task_id": task_id,
                    "status": "succeeded",
                    "message": "external visual controller completed",
                    "screen_trace_path": trace_path,
                    "artifacts": [trace_path],
                    "result": {
                        "controller": "external-vision-controller",
                        "production_attestation": "external-vision-controller-ledger-item-run-1",
                        "external_visual_model": true
                    },
                    "verification": {
                        "controller_id": "external-vision-controller",
                        "external_action_id": "vision-action-1",
                        "production_attestation": "external-vision-controller-ledger-item-run-1",
                        "external_visual_model": true,
                        "local_trace_smoke": false
                    }
                })
                .to_string(),
            )
            .unwrap();
        assert_eq!(result_response.status_code, 200);

        let evidence = server
            .handle_path(&format!(
                "/api/production-evidence/item-from-ledger?desktop_vision_action_id={action_id}&source=ledger-test"
            ))
            .unwrap();
        let evidence_value: serde_json::Value = serde_json::from_str(&evidence.body).unwrap();

        assert_eq!(evidence.status_code, 200);
        assert_eq!(evidence_value["item"]["kind"], "desktop_vision");
        assert_eq!(
            evidence_value["item"]["desktop_vision"]["adapter_id"],
            "touchdesigner"
        );
        assert_eq!(
            evidence_value["item"]["desktop_vision"]["external_action_id"],
            "vision-action-1"
        );
        assert_eq!(
            evidence_value["item"]["desktop_vision"]["controller_id"],
            "external-vision-controller"
        );
        assert_eq!(
            evidence_value["item"]["desktop_vision"]["production_attestation"],
            "external-vision-controller-ledger-item-run-1"
        );
        assert_eq!(
            evidence_value["item"]["desktop_vision"]["visual_model"],
            "external"
        );
        assert_eq!(evidence_value["validation"]["valid"], true);
        assert_eq!(
            evidence_value["validation"]["artifact_files"]["complete"],
            true
        );
        assert_eq!(
            evidence_value["validation"]["production_flags"]["complete"],
            true
        );
        assert_eq!(evidence_value["ready_for_import"], true);
    }

    #[test]
    fn desktop_recognition_contract_endpoint_exposes_controller_protocol() {
        let db_path = temp_db_path("runtime-http-desktop-recognition-contract");
        let server = RuntimeHttpServer::new(
            RuntimeHttpConfig::new(db_path)
                .with_project_slug("demo")
                .with_bind_addr("127.0.0.1:4811"),
        );

        let response = server
            .handle_request("GET", "/api/desktop-recognition/contract")
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 200);
        assert_eq!(value["kind"], "pool_desktop_recognition_contract");
        assert_eq!(
            value["queue"]["result_callback"]["http"],
            "POST /api/desktop-recognition/results"
        );
        assert!(value["software_targets"]
            .as_array()
            .unwrap()
            .iter()
            .any(|target| target["adapter_id"] == "touchdesigner"));
    }

    #[test]
    fn desktop_recognition_run_next_dispatches_queued_request() {
        let db_path = temp_db_path("runtime-http-desktop-recognition-run-next");
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        let plan = build_default_content_burst_plan("demo", "Pool HTTP demo");
        repository.persist_plan(&plan).unwrap();
        drop(repository);

        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let stage_response = server
            .handle_request_with_body(
                "POST",
                "/api/software-actions",
                r#"{
                    "adapter_id":"touchdesigner",
                    "action_kind":"RunViewport",
                    "priority":"DesktopRecognition",
                    "task_title":"TouchDesigner runtime run-next cue",
                    "payload_json":{
                        "instruction":"trigger cue 1",
                        "target_window":"TouchDesigner",
                        "visual_targets":["Perform","Cue 1"]
                    }
                }"#,
            )
            .unwrap();
        let stage_value: serde_json::Value = serde_json::from_str(&stage_response.body).unwrap();

        assert_eq!(stage_response.status_code, 200);
        assert_eq!(stage_value["snapshot"]["stats"]["software_actions"], 1);

        let run_response = server
            .handle_request_with_body(
                "POST",
                "/api/desktop-recognition/run-next",
                r#"{
                    "controller_id":"runtime-dry-run",
                    "status":"succeeded",
                    "message":"runtime dry-run completed",
                    "artifacts":["screen-trace.json"],
                    "screen_trace_path":"screen-trace.json"
                }"#,
            )
            .unwrap();
        let run_value: serde_json::Value = serde_json::from_str(&run_response.body).unwrap();

        assert_eq!(run_response.status_code, 200);
        assert_eq!(run_value["controller"], "runtime-dry-run");
        assert_eq!(run_value["processed_count"], 1);
        assert_eq!(
            run_value["callbacks"][0]["response"]["task"]["status"],
            "Succeeded"
        );
        assert_eq!(
            run_value["callbacks"][0]["response"]["software_action"]["verification"]
                ["controller_result"]["mode"],
            "dry_run"
        );
        assert_eq!(
            run_value["callbacks"][0]["response"]["software_action"]["verification"]
                ["controller_result"]["controller"],
            "runtime-dry-run"
        );

        let queue_response = server
            .handle_request("GET", "/api/desktop-recognition/requests")
            .unwrap();
        let queue_value: serde_json::Value = serde_json::from_str(&queue_response.body).unwrap();

        assert_eq!(queue_value["count"], 0);
    }

    #[test]
    fn retry_task_resumes_failed_software_action_from_ledger() {
        let db_path = temp_db_path("runtime-http-software-action-retry-resume");
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        let plan = build_default_content_burst_plan("demo", "Pool HTTP demo");
        repository.persist_plan(&plan).unwrap();
        drop(repository);

        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let stage_response = server
            .handle_request_with_body(
                "POST",
                "/api/software-actions",
                r#"{
                    "adapter_id":"touchdesigner",
                    "action_kind":"RunViewport",
                    "priority":"DesktopRecognition",
                    "task_title":"TouchDesigner retry cue",
                    "payload_json":{
                        "instruction":"trigger cue 2",
                        "target_window":"TouchDesigner",
                        "visual_targets":["Perform","Cue 2"]
                    }
                }"#,
            )
            .unwrap();
        let stage_value: serde_json::Value = serde_json::from_str(&stage_response.body).unwrap();
        let action_id = stage_value["report"]["action_id"].as_str().unwrap();
        let task_id = stage_value["report"]["task_id"]
            .as_str()
            .unwrap()
            .to_string();

        let result_response = server
            .handle_request_with_body(
                "POST",
                "/api/desktop-recognition/results",
                &format!(
                    r#"{{
                    "software_action_id":"{action_id}",
                    "task_id":"{task_id}",
                    "status":"failed",
                    "message":"TouchDesigner cue target was hidden"
                }}"#
                ),
            )
            .unwrap();
        let result_value: serde_json::Value = serde_json::from_str(&result_response.body).unwrap();

        assert_eq!(result_response.status_code, 200);
        assert_eq!(result_value["task"]["status"], "Failed");

        let retry_response = server
            .handle_request("POST", &format!("/api/tasks/retry?task_id={task_id}"))
            .unwrap();
        let retry_value: serde_json::Value = serde_json::from_str(&retry_response.body).unwrap();

        assert_eq!(retry_response.status_code, 200);
        assert_eq!(retry_value["report"]["status"], "Succeeded");
        assert_eq!(retry_value["task"]["id"].as_str(), Some(task_id.as_str()));
        assert_eq!(retry_value["snapshot"]["stats"]["software_actions"], 2);

        let queue_response = server
            .handle_request("GET", "/api/desktop-recognition/requests")
            .unwrap();
        let queue_value: serde_json::Value = serde_json::from_str(&queue_response.body).unwrap();

        assert_eq!(queue_response.status_code, 200);
        assert_eq!(queue_value["count"], 1);
        assert_ne!(
            queue_value["requests"][0]["software_action_id"].as_str(),
            Some(action_id)
        );
        assert_eq!(
            queue_value["requests"][0]["task_id"].as_str(),
            Some(task_id.as_str())
        );
    }

    #[test]
    fn post_software_action_queues_unsupported_adapter_for_human_takeover() {
        let db_path = temp_db_path("runtime-http-software-action-fallback");
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        let plan = build_default_content_burst_plan("demo", "Pool HTTP demo");
        repository.persist_plan(&plan).unwrap();
        drop(repository);

        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));
        let response = server
            .handle_request_with_body(
                "POST",
                "/api/software-actions",
                r#"{
                    "adapter_id":"unknown-live-tool",
                    "action_kind":"RunViewport",
                    "priority":"HumanTakeover",
                    "task_title":"Unknown live tool cue",
                    "payload_json":{"scene":"demo"}
                }"#,
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 202);
        assert_eq!(value["task"]["status"], "WaitingApproval");
        assert_eq!(value["snapshot"]["stats"]["software_actions"], 1);
        assert_eq!(value["snapshot"]["stats"]["waiting_approval"], 2);
        assert!(value["report"]["result"]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("human takeover"));
    }

    #[test]
    fn software_action_validates_json_body_and_adapter_id() {
        let db_path = temp_db_path("runtime-http-software-action-errors");
        let server =
            RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path).with_project_slug("demo"));

        let invalid_json = server
            .handle_request_with_body("POST", "/api/software-actions", "{")
            .unwrap();
        assert_eq!(invalid_json.status_code, 400);

        let missing_adapter = server
            .handle_request_with_body("POST", "/api/software-actions", r#"{"adapter_id":" "}"#)
            .unwrap();
        assert_eq!(missing_adapter.status_code, 400);
    }

    #[test]
    fn approve_task_requires_post_and_task_id() {
        let db_path = temp_db_path("runtime-http-approve-errors");
        let server = RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path));

        let wrong_method = server.handle_path("/api/tasks/approve").unwrap();
        assert_eq!(wrong_method.status_code, 405);

        let missing_id = server.handle_request("POST", "/api/tasks/approve").unwrap();
        assert_eq!(missing_id.status_code, 400);
    }

    #[test]
    fn handles_missing_and_unknown_routes() {
        let db_path = temp_db_path("runtime-http-routes");
        let server = RuntimeHttpServer::new(RuntimeHttpConfig::new(&db_path));

        let missing_uri = server.handle_path("/api/mcp").unwrap();
        assert_eq!(missing_uri.status_code, 400);

        let unknown = server.handle_path("/unknown").unwrap();
        assert_eq!(unknown.status_code, 404);
    }

    #[test]
    fn decodes_percent_encoded_query_values() {
        let request = RuntimeHttpRequest::parse("/api/mcp?uri=pool%3A%2F%2Ftasks").unwrap();

        assert_eq!(request.path, "/api/mcp");
        assert_eq!(
            request.query.get("uri").map(String::as_str),
            Some("pool://tasks")
        );
    }

    #[test]
    fn reads_http_request_body() {
        let raw = b"POST /api/tasks HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: 17\r\n\r\n{\"title\":\"task\"}";
        let mut cursor = std::io::Cursor::new(raw.as_slice());

        let request = read_http_request(&mut cursor).unwrap();

        assert_eq!(request.method, "POST");
        assert_eq!(request.path, "/api/tasks");
        assert_eq!(
            request.headers.get("content-type").map(String::as_str),
            Some("application/json")
        );
        assert_eq!(request.body, "{\"title\":\"task\"}");
    }

    fn spawn_fake_3dgs_gateway() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let base_url = format!("http://{addr}");
        let response_base_url = base_url.clone();

        thread::spawn(move || {
            for _ in 0..4 {
                let (mut stream, _) = listener.accept().unwrap();
                let request = read_http_request(&mut stream).unwrap();
                let (content_type, body) = match request.path.as_str() {
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
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                stream.write_all(response.as_bytes()).unwrap();
            }
        });

        base_url
    }

    fn spawn_fake_failed_3dgs_gateway() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let base_url = format!("http://{addr}");

        thread::spawn(move || {
            let mut submit_count = 0_u8;
            for _ in 0..4 {
                let (mut stream, _) = listener.accept().unwrap();
                let request = read_http_request(&mut stream).unwrap();
                let body = match request.path.as_str() {
                    "/v1/3dgs/jobs" => {
                        submit_count += 1;
                        format!(r#"{{"job_id":"failed-job-{submit_count}"}}"#)
                    }
                    "/v1/3dgs/jobs/failed-job-1" | "/v1/3dgs/jobs/failed-job-2" => {
                        r#"{"status":"failed"}"#.to_string()
                    }
                    _ => r#"{"status":"not_found"}"#.to_string(),
                };
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                stream.write_all(response.as_bytes()).unwrap();
            }
        });

        base_url
    }

    fn spawn_fake_openai_image_server() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let base_url = format!("http://{addr}");

        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let request = read_http_request(&mut stream).unwrap();
            let body = match request.path.as_str() {
                "/v1/images/generations" => {
                    r#"{"created":1,"data":[{"b64_json":"ZmFrZS1pbWFnZQ=="}],"usage":{"total_tokens":12}}"#
                }
                "/v1/images/edits" => {
                    r#"{"created":1,"data":[{"b64_json":"ZmFrZS1lZGl0"}],"usage":{"total_tokens":18}}"#
                }
                _ => r#"{"error":"not_found"}"#,
            };
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nx-request-id: req-test\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes()).unwrap();
        });

        base_url
    }

    fn spawn_fake_kling_video_server() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let base_url = format!("http://{addr}");
        let response_base_url = base_url.clone();

        thread::spawn(move || {
            for _ in 0..4 {
                let (mut stream, _) = listener.accept().unwrap();
                let request = read_http_request(&mut stream).unwrap();
                let (content_type, body) = match request.path.as_str() {
                    "/v1/videos/image2video" if request.body.contains("data:image/png;base64,") => {
                        (
                            "application/json",
                            r#"{"task_id":"kling-job-1"}"#.to_string(),
                        )
                    }
                    "/v1/videos/kling-job-1" => (
                        "application/json",
                        format!(
                            r#"{{"status":"completed","video_url":"{response_base_url}/files/kling.mp4"}}"#
                        ),
                    ),
                    "/files/kling.mp4" => ("video/mp4", "fake-kling-video".to_string()),
                    _ => ("application/json", r#"{"error":"not_found"}"#.to_string()),
                };
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                stream.write_all(response.as_bytes()).unwrap();
            }
        });

        base_url
    }

    fn spawn_fake_unreal_mcp_server() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let base_url = format!("http://{addr}");

        thread::spawn(move || {
            for _ in 0..2 {
                let (mut stream, _) = listener.accept().unwrap();
                let request = read_http_request(&mut stream).unwrap();
                let body = match request.path.as_str() {
                    "/health" => r#"{"ok":true,"message":"unreal-health-ok"}"#,
                    "/mcp"
                        if request.body.contains("CreateScene")
                            && request.body.contains("pool_unreal_action")
                            && request.body.contains("unreal.create_scene") =>
                    {
                        r#"{"ok":true,"message":"unreal-action-ok","artifacts":["unreal://level/demo"]}"#
                    }
                    _ => r#"{"ok":false,"message":"unreal-bad-request"}"#,
                };
                let status_line = if body.contains("\"ok\":true") {
                    "HTTP/1.1 200 OK"
                } else {
                    "HTTP/1.1 400 Bad Request"
                };
                let response = format!(
                    "{status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                stream.write_all(response.as_bytes()).unwrap();
            }
        });

        base_url
    }

    fn spawn_fake_hermes_mcp_server() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let base_url = format!("http://{addr}");

        thread::spawn(move || {
            for _ in 0..2 {
                let (mut stream, _) = listener.accept().unwrap();
                let request = read_http_request(&mut stream).unwrap();
                let body = match request.path.as_str() {
                    "/health" => r#"{"ok":true,"message":"hermes-health-ok"}"#,
                    "/mcp"
                        if request.body.contains("pool_hermes_action")
                            && request.body.contains("hermes.coordinate")
                            && request.body.contains("coordinate Unreal scene assembly") =>
                    {
                        r#"{"ok":true,"message":"hermes-mcp-ok","artifacts":["hermes://session/1"],"session_id":"session-1"}"#
                    }
                    _ => r#"{"ok":false,"message":"hermes-bad-request"}"#,
                };
                let status_line = if body.contains("\"ok\":true") {
                    "HTTP/1.1 200 OK"
                } else {
                    "HTTP/1.1 400 Bad Request"
                };
                let response = format!(
                    "{status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                stream.write_all(response.as_bytes()).unwrap();
            }
        });

        base_url
    }

    fn spawn_fake_generic_software_api_server(adapter_id: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let base_url = format!("http://{addr}");

        thread::spawn(move || {
            for _ in 0..2 {
                let (mut stream, _) = listener.accept().unwrap();
                let request = read_http_request(&mut stream).unwrap();
                let body = match request.path.as_str() {
                    "/health" => r#"{"ok":true,"message":"generic-software-health-ok"}"#,
                    "/mcp"
                        if request.body.contains(adapter_id)
                            && request.body.contains("pool_software_action")
                            && request.body.contains("generic-api-mcp") =>
                    {
                        r#"{"ok":true,"message":"resolve-api-ok","artifacts":["worlds/demo/output/production/resolve/1-edit.mov"]}"#
                    }
                    _ => r#"{"ok":false,"message":"generic-software-bad-request"}"#,
                };
                let status_line = if body.contains("\"ok\":true") {
                    "HTTP/1.1 200 OK"
                } else {
                    "HTTP/1.1 400 Bad Request"
                };
                let response = format!(
                    "{status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                stream.write_all(response.as_bytes()).unwrap();
            }
        });

        base_url
    }

    fn spawn_fake_hermes_server() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let base_url = format!("http://{addr}");

        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let request = read_http_request(&mut stream).unwrap();
            let body = if request.path == "/hermes" && request.body.contains("inspect Unreal") {
                r#"{"status":"hermes-ok","action_id":"act-1"}"#
            } else {
                r#"{"status":"bad-request"}"#
            };
            let status_line = if body.contains("hermes-ok") {
                "HTTP/1.1 200 OK"
            } else {
                "HTTP/1.1 400 Bad Request"
            };
            let response = format!(
                "{status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes()).unwrap();
        });

        base_url
    }

    fn temp_db_path(prefix: &str) -> PathBuf {
        std::env::temp_dir().join(format!("{prefix}-{}.sqlite", Uuid::new_v4()))
    }

    fn write_temp_artifact(root: &Path, relative_path: &str) -> String {
        let path = root.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, b"pool-production-artifact").unwrap();
        path.to_string_lossy().into_owned()
    }

    fn production_evidence_example_with_temp_files(prefix: &str) -> String {
        let root = temp_control_dir(prefix);
        let mut value: Value = serde_json::from_str(include_str!(
            "../../../docs/examples/production-evidence-bundle.example.json"
        ))
        .unwrap();

        if let Some(providers) = value.get_mut("providers").and_then(Value::as_array_mut) {
            for (index, provider) in providers.iter_mut().enumerate() {
                let path = write_temp_artifact(&root, &format!("providers/{index}.artifact"));
                let metadata_path =
                    write_temp_artifact(&root, &format!("providers/{index}.metadata.json"));
                provider["artifacts"] = json!([path]);
                provider["metadata_path"] = json!(metadata_path);
            }
        }

        if let Some(desktop_vision) = value
            .get_mut("desktop_vision")
            .and_then(Value::as_array_mut)
        {
            for (index, item) in desktop_vision.iter_mut().enumerate() {
                let path = write_temp_artifact(&root, &format!("desktop-vision/{index}.json"));
                item["trace_path"] = json!(path);
                item["artifacts"] = json!([path]);
            }
        }

        if let Some(software_actions) = value
            .get_mut("software_actions")
            .and_then(Value::as_array_mut)
        {
            for (index, item) in software_actions.iter_mut().enumerate() {
                let path = write_temp_artifact(&root, &format!("software/{index}.artifact"));
                item["artifacts"] = json!([path]);
            }
        }

        serde_json::to_string(&value).unwrap()
    }

    fn seed_prd_readiness_baseline_evidence(server: &RuntimeHttpServer, prefix: &str) {
        let root = temp_control_dir(prefix);
        let control_dir = root.join("worlds/demo/output/control/agent-sessions");
        let response = server
            .handle_request_with_body(
                "POST",
                "/api/agent-sessions",
                &json!({
                    "kind": "hermes",
                    "project_slug": "demo",
                    "control_dir": control_dir.to_string_lossy(),
                    "title": "Production evidence import test",
                    "instruction": "Prepare Pool production evidence import handoff and verify PRD readiness.",
                    "allowed_tools": ["mcp", "pool-cli", "runtime"],
                    "requires_confirmation": false
                })
                .to_string(),
            )
            .unwrap();
        assert_eq!(response.status_code, 201);

        let output_dir = root.join("worlds/demo/output");
        let response = server
            .handle_request_with_body(
                "POST",
                "/api/output-packages",
                &json!({
                    "project_slug": "demo",
                    "node_id": "outputs",
                    "output_dir": output_dir.to_string_lossy(),
                    "title": "Production evidence import output package",
                    "source_assets": ["worlds/demo/output/production/worldlabs-marble/1-world.glb"],
                    "duration_ms": 12000
                })
                .to_string(),
            )
            .unwrap();
        assert_eq!(response.status_code, 201);

        for (target, runtime, adapter_id, artifact) in [
            (
                "video",
                "DaVinci Resolve",
                "resolve",
                "worlds/demo/output/production/resolve/1-master.mov",
            ),
            (
                "game",
                "Unreal",
                "unreal",
                "unreal://project/demo/level/PoolProductionEvidence",
            ),
            (
                "interactive_art",
                "TouchDesigner",
                "touchdesigner",
                "touchdesigner://project/demo/perform",
            ),
        ] {
            let response = server
                .handle_request_with_body(
                    "POST",
                    "/api/output-packages/results",
                    &json!({
                        "project_slug": "demo",
                        "node_id": "outputs",
                        "target": target,
                        "status": "succeeded",
                        "runtime": runtime,
                        "adapter_id": adapter_id,
                        "message": format!("{target} production evidence test result"),
                        "artifacts": [artifact],
                        "metrics": [{"label": "evidence", "value": "production-import-test"}],
                        "verification": {
                            "source": "post_production_evidence_import_reports_complete_prd_coverage_for_example",
                            "fixture": true
                        }
                    })
                    .to_string(),
                )
                .unwrap();
            assert_eq!(response.status_code, 201);
        }
    }

    fn websocket_text_frames(bytes: &[u8]) -> Vec<String> {
        let mut frames = Vec::new();
        let mut offset = 0_usize;

        while offset < bytes.len() {
            assert_eq!(bytes[offset], 0x81);
            let second = bytes[offset + 1];
            assert_eq!(second & 0x80, 0);
            offset += 2;

            let mut len = (second & 0x7f) as usize;
            if len == 126 {
                len = u16::from_be_bytes([bytes[offset], bytes[offset + 1]]) as usize;
                offset += 2;
            } else if len == 127 {
                len = u64::from_be_bytes([
                    bytes[offset],
                    bytes[offset + 1],
                    bytes[offset + 2],
                    bytes[offset + 3],
                    bytes[offset + 4],
                    bytes[offset + 5],
                    bytes[offset + 6],
                    bytes[offset + 7],
                ]) as usize;
                offset += 8;
            }

            let payload = bytes[offset..offset + len].to_vec();
            frames.push(String::from_utf8(payload).unwrap());
            offset += len;
        }

        frames
    }

    fn temp_control_dir(prefix: &str) -> PathBuf {
        std::env::temp_dir().join(format!("{prefix}-{}-control", Uuid::new_v4()))
    }
}
