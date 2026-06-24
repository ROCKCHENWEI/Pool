use anyhow::{bail, Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use std::future::Future;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::models::SoftwareAdapterConfig;

mod agent_session_runner;
mod hermes_bridge_worker;
mod software_action_runner;
mod software_api_bridge_worker;
mod unreal_bridge_worker;

pub use agent_session_runner::{
    AgentCliExecutionOptions, AgentCliExecutionReport, AgentSessionExecutionChannel,
    AgentSessionExecutionReport, AgentSessionKind, AgentSessionRunReport, AgentSessionRunner,
    HermesExecutionOptions,
};
pub use hermes_bridge_worker::{
    spawn_hermes_mcp_bridge_worker, HermesMcpBridgeWorker, HermesMcpBridgeWorkerOptions,
    HermesMcpBridgeWorkerResponse,
};
pub use software_action_runner::{SoftwareActionRunReport, SoftwareActionRunner};
pub use software_api_bridge_worker::{
    spawn_software_api_bridge_worker, SoftwareApiBridgeWorker, SoftwareApiBridgeWorkerOptions,
    SoftwareApiBridgeWorkerResponse,
};
pub use unreal_bridge_worker::{
    spawn_unreal_mcp_bridge_worker, UnrealMcpBridgeWorker, UnrealMcpBridgeWorkerOptions,
    UnrealMcpBridgeWorkerResponse,
};

#[derive(Debug, Clone, Default)]
pub struct SoftwareAdapterRegistry {
    configs: BTreeMap<String, SoftwareAdapterConfig>,
}

impl SoftwareAdapterRegistry {
    pub fn new(configs: impl IntoIterator<Item = SoftwareAdapterConfig>) -> Self {
        Self {
            configs: configs
                .into_iter()
                .map(|config| (config.id.clone(), config))
                .collect(),
        }
    }

    pub fn defaults() -> Self {
        Self::new(default_software_adapters())
    }

    pub fn get(&self, id: &str) -> Option<&SoftwareAdapterConfig> {
        self.configs.get(id)
    }

    pub fn ids(&self) -> Vec<&str> {
        self.configs.keys().map(String::as_str).collect()
    }

    pub fn configs(&self) -> Vec<&SoftwareAdapterConfig> {
        self.configs.values().collect()
    }

    pub fn control_priority_chain() -> Vec<ControlPriority> {
        vec![
            ControlPriority::ApiMcp,
            ControlPriority::SkillsCli,
            ControlPriority::DesktopRecognition,
            ControlPriority::HumanTakeover,
        ]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ControlPriority {
    ApiMcp,
    SkillsCli,
    DesktopRecognition,
    HumanTakeover,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SoftwareActionKind {
    HealthCheck,
    OpenProject,
    ImportAsset,
    CreateScene,
    RunViewport,
    Render,
    Transcode,
    ExportBuild,
    ExecuteCli,
    DesktopClick,
    DesktopHotkey,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoftwareControlAction {
    pub adapter_id: String,
    pub action_kind: SoftwareActionKind,
    pub priority: ControlPriority,
    pub payload_json: serde_json::Value,
    pub requires_confirmation: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoftwareActionResult {
    pub adapter_id: String,
    pub action_kind: SoftwareActionKind,
    pub priority: ControlPriority,
    pub ok: bool,
    pub message: String,
    pub artifacts: Vec<String>,
}

pub trait SoftwareAdapter: Send + Sync {
    fn config(&self) -> &SoftwareAdapterConfig;
    fn health(&self) -> Result<SoftwareActionResult>;
    fn execute(&self, action: SoftwareControlAction) -> Result<SoftwareActionResult>;
}

pub struct MockUnrealAdapter {
    config: SoftwareAdapterConfig,
}

pub struct UnrealMcpAdapter {
    config: SoftwareAdapterConfig,
    options: UnrealMcpAdapterOptions,
    client: Client,
}

pub struct HermesMcpAdapter {
    config: SoftwareAdapterConfig,
    options: HermesMcpAdapterOptions,
    client: Client,
}

pub struct GenericSoftwareApiAdapter {
    config: SoftwareAdapterConfig,
    options: GenericSoftwareApiOptions,
    client: Client,
}

#[derive(Debug, Clone, Default)]
pub struct UnrealMcpAdapterOptions {
    pub endpoint: String,
    pub health_path: String,
    pub action_path: String,
    pub auth_token: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct HermesMcpAdapterOptions {
    pub endpoint: String,
    pub health_path: String,
    pub action_path: String,
    pub auth_token: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct GenericSoftwareApiOptions {
    pub endpoint: String,
    pub health_path: String,
    pub action_path: String,
    pub auth_token: Option<String>,
}

impl MockUnrealAdapter {
    pub fn new() -> Self {
        Self {
            config: adapter(
                "unreal",
                "Unreal",
                &["api/mcp", "skills/cli", "desktop-recognition"],
                1,
                true,
            ),
        }
    }
}

impl UnrealMcpAdapter {
    pub fn new(options: UnrealMcpAdapterOptions) -> Self {
        Self {
            config: adapter(
                "unreal",
                "Unreal",
                &["api/mcp", "skills/cli", "desktop-recognition"],
                1,
                true,
            ),
            options,
            client: Client::new(),
        }
    }

    pub fn from_env() -> Self {
        Self::new(UnrealMcpAdapterOptions::from_env())
    }

    pub fn from_action(action: &SoftwareControlAction) -> Self {
        Self::new(UnrealMcpAdapterOptions::from_env().with_action_payload(&action.payload_json))
    }

    pub fn is_configured(&self) -> bool {
        !self.options.endpoint.trim().is_empty()
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.options.endpoint.trim_end_matches('/'), path)
    }
}

impl HermesMcpAdapter {
    pub fn new(options: HermesMcpAdapterOptions) -> Self {
        Self {
            config: adapter(
                "hermes",
                "Hermes",
                &["agent-api", "mcp", "skills/cli"],
                11,
                false,
            ),
            options,
            client: Client::new(),
        }
    }

    pub fn from_env() -> Self {
        Self::new(HermesMcpAdapterOptions::from_env())
    }

    pub fn from_action(action: &SoftwareControlAction, auth_token: Option<String>) -> Self {
        Self::new(
            HermesMcpAdapterOptions::from_env()
                .with_action_payload(&action.payload_json)
                .with_auth_token(auth_token),
        )
    }

    pub fn is_configured(&self) -> bool {
        !self.options.endpoint.trim().is_empty()
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.options.endpoint.trim_end_matches('/'), path)
    }
}

impl GenericSoftwareApiAdapter {
    pub fn new(config: SoftwareAdapterConfig, options: GenericSoftwareApiOptions) -> Self {
        Self {
            config,
            options,
            client: Client::new(),
        }
    }

    pub fn from_action(config: SoftwareAdapterConfig, action: &SoftwareControlAction) -> Self {
        Self::new(
            config,
            GenericSoftwareApiOptions::default().with_action_payload(&action.payload_json),
        )
    }

    pub fn is_configured(&self) -> bool {
        !self.options.endpoint.trim().is_empty()
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.options.endpoint.trim_end_matches('/'), path)
    }
}

impl UnrealMcpAdapterOptions {
    pub fn from_env() -> Self {
        Self {
            endpoint: std::env::var("POOL_UNREAL_MCP_ENDPOINT").unwrap_or_default(),
            health_path: std::env::var("POOL_UNREAL_MCP_HEALTH_PATH")
                .unwrap_or_else(|_| "/health".to_string()),
            action_path: std::env::var("POOL_UNREAL_MCP_ACTION_PATH")
                .unwrap_or_else(|_| "/mcp".to_string()),
            auth_token: std::env::var("POOL_UNREAL_MCP_TOKEN").ok(),
        }
    }

    fn with_action_payload(mut self, payload: &Value) -> Self {
        if let Some(endpoint) = payload_string(payload, "endpoint")
            .or_else(|| payload_string(payload, "mcp_endpoint"))
            .or_else(|| payload_string(payload, "unreal_mcp_endpoint"))
        {
            self.endpoint = endpoint;
        }
        if let Some(health_path) = payload_string(payload, "health_path") {
            self.health_path = ensure_leading_slash(&health_path);
        }
        if let Some(action_path) =
            payload_string(payload, "action_path").or_else(|| payload_string(payload, "mcp_path"))
        {
            self.action_path = ensure_leading_slash(&action_path);
        }
        if let Some(auth_token) = payload_string(payload, "auth_token")
            .or_else(|| payload_string(payload, "bearer_token"))
        {
            self.auth_token = Some(auth_token);
        }
        self
    }
}

impl HermesMcpAdapterOptions {
    pub fn from_env() -> Self {
        Self {
            endpoint: std::env::var("POOL_HERMES_MCP_ENDPOINT")
                .or_else(|_| std::env::var("POOL_HERMES_ENDPOINT"))
                .unwrap_or_default(),
            health_path: std::env::var("POOL_HERMES_MCP_HEALTH_PATH")
                .unwrap_or_else(|_| "/health".to_string()),
            action_path: std::env::var("POOL_HERMES_MCP_ACTION_PATH")
                .unwrap_or_else(|_| "/mcp".to_string()),
            auth_token: std::env::var("POOL_HERMES_MCP_TOKEN")
                .or_else(|_| std::env::var("POOL_HERMES_TOKEN"))
                .ok(),
        }
    }

    fn with_action_payload(mut self, payload: &Value) -> Self {
        if let Some(endpoint) = payload_string(payload, "endpoint")
            .or_else(|| payload_string(payload, "mcp_endpoint"))
            .or_else(|| payload_string(payload, "hermes_endpoint"))
            .or_else(|| payload_string(payload, "hermes_mcp_endpoint"))
        {
            self.endpoint = endpoint;
        }
        if let Some(health_path) = payload_string(payload, "health_path") {
            self.health_path = ensure_leading_slash(&health_path);
        }
        if let Some(action_path) =
            payload_string(payload, "action_path").or_else(|| payload_string(payload, "mcp_path"))
        {
            self.action_path = ensure_leading_slash(&action_path);
        }
        if let Some(auth_token) = payload_string(payload, "auth_token")
            .or_else(|| payload_string(payload, "bearer_token"))
        {
            self.auth_token = Some(auth_token);
        }
        self
    }

    fn with_auth_token(mut self, auth_token: Option<String>) -> Self {
        if auth_token.is_some() {
            self.auth_token = auth_token;
        }
        self
    }
}

impl GenericSoftwareApiOptions {
    fn with_action_payload(mut self, payload: &Value) -> Self {
        if let Some(endpoint) = payload_string_any(
            payload,
            &[
                "endpoint",
                "mcp_endpoint",
                "api_endpoint",
                "software_endpoint",
                "control_endpoint",
            ],
        ) {
            self.endpoint = endpoint;
        }
        if let Some(health_path) = payload_string(payload, "health_path") {
            self.health_path = ensure_leading_slash(&health_path);
        } else if self.health_path.is_empty() {
            self.health_path = "/health".to_string();
        }
        if let Some(action_path) =
            payload_string(payload, "action_path").or_else(|| payload_string(payload, "mcp_path"))
        {
            self.action_path = ensure_leading_slash(&action_path);
        } else if self.action_path.is_empty() {
            self.action_path = "/mcp".to_string();
        }
        if let Some(auth_token) = payload_string(payload, "auth_token")
            .or_else(|| payload_string(payload, "bearer_token"))
        {
            self.auth_token = Some(auth_token);
        }
        self
    }
}

pub struct CommandSoftwareAdapter {
    config: SoftwareAdapterConfig,
}

pub struct DesktopRecognitionAdapter {
    config: SoftwareAdapterConfig,
}

impl CommandSoftwareAdapter {
    pub fn new(config: SoftwareAdapterConfig) -> Self {
        Self { config }
    }
}

impl DesktopRecognitionAdapter {
    pub fn new(config: SoftwareAdapterConfig) -> Self {
        Self { config }
    }
}

pub fn desktop_recognition_contract_resource() -> Value {
    let software_targets = SoftwareAdapterRegistry::defaults()
        .configs()
        .into_iter()
        .filter(|config| {
            config
                .control_modes
                .iter()
                .any(|surface| surface == "desktop-recognition")
        })
        .map(|config| {
            json!({
                "adapter_id": &config.id,
                "display_name": &config.display_name,
                "control_modes": &config.control_modes,
                "supports_desktop_recognition": true,
            })
        })
        .collect::<Vec<_>>();

    json!({
        "kind": "pool_desktop_recognition_contract",
        "version": 1,
        "summary": {
            "control_priority": "API/MCP > Skills/CLI > Desktop Recognition > Human Takeover",
            "software_targets": software_targets.len(),
            "request_contract": "desktop-recognition-control-request",
            "controller_result_contract": "desktop-recognition-result-callback",
        },
        "policy": {
            "api_mcp_and_cli_are_preferred": true,
            "desktop_recognition_is_fallback": true,
            "local_request_file_is_authoritative": true,
            "controller_must_return_evidence": true,
            "secrets_stay_server_side": true,
        },
        "queue": {
            "read_requests": {
                "http": "GET /api/desktop-recognition/requests",
                "mcp": "pool://desktop-recognition",
                "cli": "pool-cli desktop-requests"
            },
            "claim_or_dry_run": {
                "http": "POST /api/desktop-recognition/run-next",
                "cli": "pool-cli desktop-run-next --controller-id <id> --status <status>"
            },
            "result_callback": {
                "http": "POST /api/desktop-recognition/results",
                "mcp_tool": "pool_desktop_result",
                "cli": "pool-cli desktop-result <software-action-id> --status <status>"
            }
        },
        "controller_execution": {
            "default_mode": "dry_run",
            "dry_run": {
                "http": "POST /api/desktop-recognition/run-next",
                "cli": "pool-cli desktop-run-next --controller-id <id> --status <status>",
                "description": "Validates queue/result callback without touching the desktop."
            },
            "applescript": {
                "example": "cargo run -p pool-core --example run_desktop_recognition_controller -- http://127.0.0.1:4788 --project=demo --mode=applescript --vision-trace=worlds/demo/output/control/desktop-recognition/trace.json",
                "osascript_env": "POOL_DESKTOP_OSASCRIPT",
                "vision_trace_env": "POOL_DESKTOP_VISION_TRACE",
                "supported_primitives": ["activate target_window", "click explicit coordinates", "click visual target resolved from trace", "hotkey / keys", "type text"],
                "requires": ["macOS Accessibility permission", "explicit target_window plus coordinates, hotkey, text, or vision trace hit"],
                "vision_trace_contract": {
                    "accepted_roots": ["detections", "targets", "items", "elements", "ocr", "array root"],
                    "label_fields": ["label", "text", "name", "id"],
                    "position_fields": ["center", "point", "position", "coordinates", "bounds", "bbox", "box"],
                    "bounds_fields": ["x/y/width/height", "left/top/right/bottom", "x1/y1/x2/y2", "[x,y,width,height] or [x1,y1,x2,y2]"],
                    "matching": "case-insensitive exact match first, substring match fallback"
                },
                "boundary": "AppleScript mode executes deterministic desktop primitives and can consume external vision/OCR trace files; the visual model itself is still external."
            }
        },
        "request_file": {
            "output_contract": "desktop-recognition-control-request",
            "required_fields": [
                "id",
                "adapter_id",
                "action_kind",
                "priority",
                "status",
                "instruction",
                "target_window",
                "visual_targets",
                "pool_desktop_action",
                "desktop_payload",
                "payload",
                "created_at"
            ],
            "status": "queued_for_desktop_recognition",
            "path_source": "software_action.artifacts[] entry ending in desktop-recognition-<id>.json"
        },
        "controller_input": {
            "pool_desktop_action": {
                "profile_id": "stable desktop operation profile",
                "desktop_tool": "suggested controller tool family",
                "expected_artifacts": ["desktop_request", "screen_trace"]
            },
            "desktop_payload": {
                "tool": "desktop controller command name",
                "operation": "operation semantic id",
                "instruction": "human-readable instruction",
                "target_window": "preferred app/window title",
                "visual_targets": "visual labels, nodes, buttons, panels, or timeline targets",
                "arguments": "operation-specific payload",
                "handoff": "stage, expected artifacts, and control priority"
            }
        },
        "result_callback": {
            "statuses": [
                "queued_for_desktop_recognition",
                "running",
                "succeeded",
                "failed",
                "retryable",
                "cancelled"
            ],
            "accepted_fields": [
                "software_action_id",
                "task_id",
                "status",
                "message",
                "artifacts",
                "screen_trace_path",
                "result",
                "verification"
            ],
            "task_status_mapping": {
                "succeeded": "Succeeded",
                "failed": "Failed",
                "retryable": "Retryable",
                "cancelled": "Cancelled",
                "running": "Running",
                "queued_for_desktop_recognition": "Running"
            }
        },
        "action_profiles": desktop_action_contracts(),
        "software_targets": software_targets,
    })
}

pub fn software_control_contracts_resource(adapter_id: Option<&str>) -> Result<Value> {
    if let Some(adapter_id) = adapter_id {
        let Some(contract) = software_control_contract(adapter_id) else {
            bail!("unknown software control contract: {adapter_id}");
        };
        return Ok(contract);
    }

    let contracts = SoftwareAdapterRegistry::defaults()
        .configs()
        .into_iter()
        .map(|config| software_control_contract_for_config(config))
        .collect::<Vec<_>>();

    Ok(json!({
        "kind": "pool_software_control_contracts",
        "version": 1,
        "summary": {
            "software_adapters": contracts.len(),
            "control_priority": "API/MCP > Skills/CLI > Desktop Recognition > Human Takeover",
            "runtime_action_endpoint": "/api/software-actions",
            "runtime_health_endpoint": "/api/software-health",
        },
        "policy": {
            "api_mcp_and_cli_are_preferred": true,
            "desktop_recognition_is_fallback": true,
            "human_takeover_is_last_resort": true,
            "software_actions_are_audited": true,
            "secrets_stay_server_side": true,
        },
        "contracts": contracts,
    }))
}

pub fn software_control_contract(adapter_id: &str) -> Option<Value> {
    let adapter_id = canonical_software_adapter_id(adapter_id);
    let registry = SoftwareAdapterRegistry::defaults();
    let config = registry.get(&adapter_id)?;
    Some(software_control_contract_for_config(config))
}

fn software_control_contract_for_config(config: &SoftwareAdapterConfig) -> Value {
    json!({
        "kind": "pool_software_control_contract",
        "version": 1,
        "adapter_id": &config.id,
        "display_name": &config.display_name,
        "priority": config.priority,
        "control_modes": &config.control_modes,
        "desktop_fallback": config.desktop_fallback,
        "runtime_health": {
            "method": "POST",
            "path": "/api/software-health",
            "body": {
                "adapter_id": &config.id,
                "priority": preferred_control_priority(config),
                "payload_json": software_health_payload_template(config),
            }
        },
        "runtime_action": {
            "method": "POST",
            "path": "/api/software-actions",
            "body": {
                "project_slug": "demo",
                "adapter_id": &config.id,
                "task_title": format!("{} software action", config.display_name),
                "action_kind": default_action_kind(config),
                "priority": preferred_control_priority(config),
                "payload_json": software_action_payload_template(config),
                "requires_confirmation": false,
            }
        },
        "output_result_bridge": {
            "payload_key": "pool_output_result",
            "description": "When present and the software action succeeds, SoftwareActionRunner records the result into the matching output manifest through /api/output-packages/results.",
            "target_values": ["video", "game", "interactive_art"],
            "body_fields": {
                "target": "video | game | interactive_art",
                "local_path": "optional deliverable manifest path; omitted means resolve from catalog",
                "status": "optional, defaults to succeeded",
                "runtime": "optional runtime display name",
                "adapter_id": "optional adapter id; defaults to software action adapter",
                "message": "optional result message",
                "artifacts": ["optional artifact URI or local path"],
                "metrics": [{"label": "fps", "value": "60"}],
                "verification": {"source": "software adapter or controller evidence"}
            }
        },
        "control_priority_chain": SoftwareAdapterRegistry::control_priority_chain(),
        "control_routes": software_control_routes(config),
        "supported_action_kinds": software_action_kind_contracts(config),
        "conformance_runbook": software_conformance_runbook(config),
        "fallback": {
            "desktop_recognition_contract": if config.desktop_fallback {
                Some("pool://desktop-recognition-contract")
            } else {
                None
            },
            "human_takeover": {
                "status": "waiting_approval",
                "reason": "adapter has no configured executable route or requires explicit operator confirmation",
            }
        },
    })
}

fn software_conformance_runbook(config: &SoftwareAdapterConfig) -> Value {
    let adapter_id = config.id.as_str();
    let worker = software_conformance_worker_command(adapter_id);
    let endpoint = software_conformance_endpoint(adapter_id);
    let endpoint_env = software_production_endpoint_env(adapter_id);
    let artifacts_env = software_production_artifacts_env(adapter_id);
    let attestation_env = software_production_attestation_env(adapter_id);
    let action_kind = default_action_kind(config);
    let smoke_artifact = format!("worlds/demo/output/{adapter_id}-software-smoke.json");
    let smoke_payload = format!(
        r#"{{"mcp_path":"/mcp","instruction":"Pool {adapter_id} bridge conformance smoke","artifacts":["{smoke_artifact}"]}}"#
    );
    let matrix_output = "target/software-evidence-matrix";
    let evidence_bundle =
        "target/software-evidence-matrix/software-production-evidence-bundle.json";
    let local_bridge_command = format!("{worker} --once --output-root worlds/demo/output");
    let real_upstream_command = format!(
        "{worker} --bind {} --output-root worlds/demo/output --upstream <real-plugin-or-gateway-url>",
        software_conformance_bind(adapter_id)
    );
    let health_command =
        format!("pool-cli --project demo software-health {adapter_id} --endpoint {endpoint}");
    let action_command = format!(
        "pool-cli --project demo run-software {adapter_id} --action-kind {action_kind} --priority ApiMcp --endpoint {endpoint} --payload-json '{smoke_payload}' --no-confirmation"
    );
    let production_command = format!(
        "{endpoint_env}={endpoint} {artifacts_env}={smoke_artifact} {attestation_env}=<real-software-run-id> pool-cli --project demo production-evidence-software-matrix {matrix_output} --production-software --software-endpoint-env {adapter_id}={endpoint_env} --software-artifacts-env {adapter_id}={artifacts_env} --software-attestation-env {adapter_id}={attestation_env} --evidence-bundle={evidence_bundle}"
    );
    let validate_command = format!(
        "pool-cli --project demo validate-production-evidence {evidence_bundle} && pool-cli --project demo import-production-evidence {evidence_bundle}"
    );

    json!({
        "purpose": "Verify this software adapter from local bridge smoke to real upstream evidence import.",
        "control_priority": "API/MCP > Skills/CLI > Desktop Recognition > Human Takeover",
        "adapter_id": adapter_id,
        "bridge_worker": {
            "local_command": worker,
            "endpoint": endpoint,
            "endpoint_env": endpoint_env,
            "artifacts_env": artifacts_env,
            "production_attestation_env": attestation_env,
        },
        "phases": [
            {
                "id": "local_bridge_baseline",
                "goal": "Run the local bridge worker dry-run/self-check and create local request/response files.",
                "command": local_bridge_command,
            },
            {
                "id": "real_upstream_bridge",
                "goal": "Run the same Pool wrapper as a forwarder to a real plugin, MCP server, SDK worker, or software gateway.",
                "command": real_upstream_command,
            },
            {
                "id": "software_health",
                "goal": "Verify Runtime can reach the software bridge through /api/software-health.",
                "command": health_command,
            },
            {
                "id": "software_action_smoke",
                "goal": "Submit a minimal audited software action and require a local artifact path in the result.",
                "command": action_command,
            },
            {
                "id": "production_matrix",
                "goal": "Convert a real upstream run into production software evidence; missing endpoint/artifact/attestation must fail.",
                "command": production_command,
            },
            {
                "id": "validate_and_import",
                "goal": "Validate the bundle, then import it into the runtime ledger only after template ids are replaced.",
                "command": validate_command,
            }
        ],
        "pass_conditions": [
            "local bridge worker returns ok=true and writes local request/response artifacts",
            "real upstream bridge is backed by a real plugin/gateway/SDK worker, not dry-run or echo output",
            "software action result includes local file artifacts; remote URLs remain provenance only",
            "production matrix includes endpoint, local artifacts, and non-placeholder production attestation",
            "validate/import rejects template ids, missing local files, leaked secrets, and URI-only artifacts"
        ],
        "mcp_tools": [
            "pool_software_contracts",
            "pool_run_software",
            "pool_worker_self_checks"
        ],
    })
}

fn software_conformance_worker_command(adapter_id: &str) -> String {
    match adapter_id {
        "unreal" => "pool-cli unreal-mcp-bridge-worker".to_string(),
        "hermes" => "pool-cli hermes-mcp-bridge-worker".to_string(),
        _ => format!("pool-cli software-api-bridge-worker {adapter_id}"),
    }
}

fn software_conformance_bind(adapter_id: &str) -> &'static str {
    match adapter_id {
        "unreal" => "127.0.0.1:8790",
        "hermes" => "127.0.0.1:8792",
        _ => "127.0.0.1:8793",
    }
}

fn software_conformance_endpoint(adapter_id: &str) -> String {
    format!("http://{}", software_conformance_bind(adapter_id))
}

fn software_production_endpoint_env(adapter_id: &str) -> String {
    match adapter_id {
        "unreal" => "POOL_UNREAL_MCP_ENDPOINT".to_string(),
        "hermes" => "POOL_HERMES_MCP_ENDPOINT".to_string(),
        _ => format!("POOL_{}_ENDPOINT", software_env_token(adapter_id)),
    }
}

fn software_production_artifacts_env(adapter_id: &str) -> String {
    format!("POOL_{}_ARTIFACTS", software_env_token(adapter_id))
}

fn software_production_attestation_env(adapter_id: &str) -> String {
    format!(
        "POOL_{}_PRODUCTION_ATTESTATION",
        software_env_token(adapter_id)
    )
}

fn canonical_software_adapter_id(adapter_id: &str) -> String {
    match adapter_id.trim().to_ascii_lowercase().as_str() {
        "davinci" | "davinci-resolve" | "da-vinci-resolve" => "resolve".to_string(),
        "touch-designer" => "touchdesigner".to_string(),
        "mad-mapper" => "madmapper".to_string(),
        "mocap" | "motion-capture" | "motion-database" => "motion-db".to_string(),
        "editing" | "editor" | "cutting" => "editing-suite".to_string(),
        value => value.to_string(),
    }
}

fn preferred_control_priority(config: &SoftwareAdapterConfig) -> &'static str {
    if config
        .control_modes
        .iter()
        .any(|mode| matches!(mode.as_str(), "api/mcp" | "agent-api" | "mcp" | "http-api"))
    {
        "ApiMcp"
    } else if config.control_modes.iter().any(|mode| {
        matches!(
            mode.as_str(),
            "skills/cli" | "python-api" | "scripting-api" | "editor-api" | "osc" | "api"
        )
    }) {
        "SkillsCli"
    } else if config.desktop_fallback {
        "DesktopRecognition"
    } else {
        "HumanTakeover"
    }
}

fn default_action_kind(config: &SoftwareAdapterConfig) -> &'static str {
    match config.id.as_str() {
        "unreal" | "unity" => "CreateScene",
        "resolve" | "editing-suite" => "Transcode",
        "touchdesigner" | "madmapper" => "RunViewport",
        "comfyui" | "blender" | "nuke" | "motion-db" => "ExecuteCli",
        "hermes" => "RunViewport",
        _ => "HealthCheck",
    }
}

fn software_health_payload_template(config: &SoftwareAdapterConfig) -> Value {
    match config.id.as_str() {
        "unreal" => json!({
            "endpoint": "http://127.0.0.1:<unreal-mcp-port>",
            "health_path": "/health",
        }),
        "hermes" => json!({
            "endpoint": "http://127.0.0.1:<hermes-port>",
            "health_path": "/health",
        }),
        _ => json!({
            "endpoint": format!("http://127.0.0.1:<{}-api-port>", config.id),
            "health_path": "/health",
        }),
    }
}

fn software_action_payload_template(config: &SoftwareAdapterConfig) -> Value {
    match config.id.as_str() {
        "unreal" => json!({
            "endpoint": "http://127.0.0.1:<unreal-mcp-port>",
            "mcp_path": "/mcp",
            "level": "pool_content_burst",
            "asset_paths": ["worlds/demo/output/1-object.glb"],
        }),
        "hermes" => json!({
            "endpoint": "http://127.0.0.1:<hermes-port>",
            "mcp_path": "/mcp",
            "instruction": "Coordinate Pool software control for the selected workflow node.",
            "target_adapter": "unreal",
            "target_action_kind": "CreateScene",
        }),
        "blender" | "nuke" | "motion-db" | "editing-suite" => json!({
            "endpoint": format!("http://127.0.0.1:<{}-api-port>", config.id),
            "mcp_path": "/mcp",
            "command": "/bin/echo pool-software-action",
            "allowed_commands": ["/bin/echo", "echo"],
            "working_dir": ".",
        }),
        "resolve" => json!({
            "endpoint": "http://127.0.0.1:<resolve-api-port>",
            "mcp_path": "/mcp",
            "instruction": "Open timeline, import generated media, and prepare a preview render.",
            "target_window": "DaVinci Resolve",
            "visual_targets": ["Media Pool", "Timeline", "Deliver"],
        }),
        "touchdesigner" => json!({
            "endpoint": "http://127.0.0.1:<touchdesigner-api-port>",
            "mcp_path": "/mcp",
            "instruction": "Open perform mode and trigger the selected cue.",
            "target_window": "TouchDesigner",
            "visual_targets": ["Perform", "Cue"],
        }),
        "madmapper" => json!({
            "endpoint": "http://127.0.0.1:<madmapper-api-port>",
            "mcp_path": "/mcp",
            "instruction": "Load mapped media and verify output surface playback.",
            "target_window": "MadMapper",
            "visual_targets": ["Media", "Surface", "Output"],
        }),
        "unity" => json!({
            "endpoint": "http://127.0.0.1:<unity-api-port>",
            "mcp_path": "/mcp",
            "instruction": "Import generated assets and run the selected scene.",
            "target_window": "Unity",
            "visual_targets": ["Project", "Scene", "Play"],
        }),
        _ => json!({}),
    }
}

fn software_control_routes(config: &SoftwareAdapterConfig) -> Vec<Value> {
    let mut routes = Vec::new();

    if config.id != "unreal" && config.id != "hermes" {
        routes.push(generic_api_mcp_control_route(config));
    }
    if config
        .control_modes
        .iter()
        .any(|mode| matches!(mode.as_str(), "api/mcp" | "agent-api" | "mcp" | "http-api"))
    {
        routes.push(api_mcp_control_route(config));
    }
    if config.control_modes.iter().any(|mode| {
        matches!(
            mode.as_str(),
            "skills/cli" | "python-api" | "scripting-api" | "editor-api" | "osc" | "api"
        )
    }) {
        routes.push(skills_cli_control_route(config));
    }
    if config.desktop_fallback {
        routes.push(json!({
            "priority": "DesktopRecognition",
            "status": "fallback_available",
            "contract": "pool://desktop-recognition-contract",
            "http_contract": "/api/desktop-recognition/contract",
            "runtime_action_priority": "DesktopRecognition",
        }));
    }
    routes.push(json!({
        "priority": "HumanTakeover",
        "status": "last_resort",
        "runtime_status": "waiting_approval",
    }));

    routes
}

fn api_mcp_control_route(config: &SoftwareAdapterConfig) -> Value {
    match config.id.as_str() {
        "unreal" => json!({
            "priority": "ApiMcp",
            "adapter_kind": "unreal_mcp",
            "endpoint_env": "POOL_UNREAL_MCP_ENDPOINT",
            "token_env": "POOL_UNREAL_MCP_TOKEN",
            "health_path_env": "POOL_UNREAL_MCP_HEALTH_PATH",
            "action_path_env": "POOL_UNREAL_MCP_ACTION_PATH",
            "request_wrapper": "pool_unreal_action + mcp_payload",
            "action_profiles": unreal_action_contracts(),
        }),
        "hermes" => json!({
            "priority": "ApiMcp",
            "adapter_kind": "hermes_mcp",
            "endpoint_env": "POOL_HERMES_MCP_ENDPOINT",
            "token_env": "POOL_HERMES_MCP_TOKEN",
            "health_path_env": "POOL_HERMES_MCP_HEALTH_PATH",
            "action_path_env": "POOL_HERMES_MCP_ACTION_PATH",
            "request_wrapper": "pool_hermes_action + mcp_payload",
            "action_profiles": hermes_action_contracts(),
        }),
        "comfyui" => json!({
            "priority": "ApiMcp",
            "adapter_kind": "comfyui_provider_bridge",
            "note": "ComfyUI is usually executed through /api/provider-runs; /api/software-actions can still audit software-level control.",
        }),
        _ => json!({
            "priority": "ApiMcp",
            "adapter_kind": "external_gateway_or_local_api",
            "note": "Provide endpoint details through payload_json when a local gateway is available.",
        }),
    }
}

fn generic_api_mcp_control_route(config: &SoftwareAdapterConfig) -> Value {
    json!({
        "priority": "ApiMcp",
        "adapter_kind": "generic_software_api_mcp",
        "status": "available_when_endpoint_is_configured",
        "endpoint_env": software_endpoint_env_names(&config.id),
        "local_worker": {
            "cli": format!("pool-cli software-api-bridge-worker {} --bind 127.0.0.1:8793 --output-root worlds/demo/output", config.id),
            "endpoint_env": format!("POOL_{}_ENDPOINT=http://127.0.0.1:8793", software_env_token(&config.id)),
            "dry_run": "validates pool_software_action + mcp_payload and writes request/response JSON files",
            "forwarder": "add --upstream <url> to proxy the same Pool wrapper to a real software plugin or gateway",
        },
        "token_payload_fields": ["auth_token", "bearer_token"],
        "health": {
            "method": "GET",
            "default_path": "/health",
            "override_payload_field": "health_path",
        },
        "action": {
            "method": "POST",
            "default_path": "/mcp",
            "override_payload_fields": ["action_path", "mcp_path"],
            "request_wrapper": "pool_software_action + mcp_payload",
            "result_contract": {
                "ok": "boolean, defaults to true on 2xx when omitted",
                "message": "optional status or message string",
                "artifacts": "array of local artifact paths or software result URIs",
                "artifact_path": "optional single local artifact path",
                "output_path": "optional single local output path",
                "report_path": "optional local report path",
            }
        },
    })
}

fn skills_cli_control_route(config: &SoftwareAdapterConfig) -> Value {
    json!({
        "priority": "SkillsCli",
        "adapter_kind": "command_software_adapter",
        "runtime_action_kind": "ExecuteCli",
        "required_payload_fields": ["command", "allowed_commands"],
        "optional_payload_fields": ["working_dir", "timeout_ms", "max_output_bytes", "artifacts"],
        "safety": {
            "allowed_commands_required": true,
            "shell_is_not_used": true,
        },
        "suggested_modes": &config.control_modes,
    })
}

fn software_endpoint_env_names(adapter_id: &str) -> Vec<String> {
    let token = software_env_token(adapter_id);
    let mut names = vec![
        format!("POOL_SOFTWARE_{token}_ENDPOINT"),
        format!("POOL_{token}_ENDPOINT"),
    ];
    names.extend(
        software_alias_env_tokens(adapter_id)
            .into_iter()
            .flat_map(|alias| {
                [
                    format!("POOL_SOFTWARE_{alias}_ENDPOINT"),
                    format!("POOL_{alias}_ENDPOINT"),
                ]
            }),
    );
    dedup_env_names(names)
}

fn software_alias_env_tokens(adapter_id: &str) -> Vec<String> {
    match adapter_id {
        "resolve" => vec!["DAVINCI_RESOLVE".to_string()],
        "motion-db" => vec!["MOTION_DB".to_string(), "MOCAP_DB".to_string()],
        "editing-suite" => vec!["EDITING_SUITE".to_string(), "EDITOR".to_string()],
        "touchdesigner" => vec!["TOUCH_DESIGNER".to_string()],
        _ => Vec::new(),
    }
}

fn software_env_token(adapter_id: &str) -> String {
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

fn dedup_env_names(names: Vec<String>) -> Vec<String> {
    let mut deduped = Vec::new();
    for name in names {
        if !deduped.contains(&name) {
            deduped.push(name);
        }
    }
    deduped
}

fn software_action_kind_contracts(config: &SoftwareAdapterConfig) -> Vec<Value> {
    let mut actions = match config.id.as_str() {
        "unreal" => unreal_action_contracts(),
        "hermes" => hermes_action_contracts(),
        "blender" | "nuke" | "motion-db" | "editing-suite" | "comfyui" => {
            vec![command_action_contract()]
        }
        "resolve" => vec![
            generic_action_contract(SoftwareActionKind::OpenProject, "open_project"),
            generic_action_contract(SoftwareActionKind::ImportAsset, "import_asset"),
            generic_action_contract(SoftwareActionKind::Transcode, "transcode_timeline"),
            generic_action_contract(SoftwareActionKind::Render, "render_timeline"),
        ],
        "unity" => vec![
            generic_action_contract(SoftwareActionKind::OpenProject, "open_project"),
            generic_action_contract(SoftwareActionKind::ImportAsset, "import_asset"),
            generic_action_contract(SoftwareActionKind::CreateScene, "create_scene"),
            generic_action_contract(SoftwareActionKind::RunViewport, "run_play_mode"),
            generic_action_contract(SoftwareActionKind::ExportBuild, "export_build"),
        ],
        "touchdesigner" | "madmapper" => vec![
            generic_action_contract(SoftwareActionKind::OpenProject, "open_project"),
            generic_action_contract(SoftwareActionKind::ImportAsset, "import_media"),
            generic_action_contract(SoftwareActionKind::RunViewport, "run_preview_or_cue"),
            generic_action_contract(SoftwareActionKind::Render, "record_output"),
        ],
        _ => vec![generic_action_contract(
            SoftwareActionKind::HealthCheck,
            "health_check",
        )],
    };

    if config.desktop_fallback {
        actions.extend(desktop_action_contracts());
    }
    actions
}

fn command_action_contract() -> Value {
    json!({
        "action_kind": SoftwareActionKind::ExecuteCli,
        "operation": "execute_cli",
        "priority": "SkillsCli",
        "required_payload_fields": ["command", "allowed_commands"],
        "output_contract": "software-command-audit",
    })
}

fn generic_action_contract(action_kind: SoftwareActionKind, operation: &str) -> Value {
    json!({
        "action_kind": action_kind,
        "operation": operation,
        "priority": "ApiMcp_or_SkillsCli_or_DesktopRecognition",
        "output_contract": "software-action-audit",
    })
}

impl SoftwareAdapter for CommandSoftwareAdapter {
    fn config(&self) -> &SoftwareAdapterConfig {
        &self.config
    }

    fn health(&self) -> Result<SoftwareActionResult> {
        Ok(SoftwareActionResult {
            adapter_id: self.config.id.clone(),
            action_kind: SoftwareActionKind::HealthCheck,
            priority: ControlPriority::SkillsCli,
            ok: true,
            message: "command software adapter ready".to_string(),
            artifacts: Vec::new(),
        })
    }

    fn execute(&self, action: SoftwareControlAction) -> Result<SoftwareActionResult> {
        let command = command_payload(&action)?;
        let execution = execute_command_payload(&command);
        let mut artifacts = command.artifacts.clone();
        artifacts.push(format!(
            "software-command://{}/{}",
            action.adapter_id,
            command_name(
                execution
                    .argv
                    .first()
                    .map(String::as_str)
                    .unwrap_or_default()
            )
        ));
        Ok(SoftwareActionResult {
            adapter_id: action.adapter_id,
            action_kind: action.action_kind,
            priority: action.priority,
            ok: execution.ok,
            message: execution.message(),
            artifacts,
        })
    }
}

impl SoftwareAdapter for DesktopRecognitionAdapter {
    fn config(&self) -> &SoftwareAdapterConfig {
        &self.config
    }

    fn health(&self) -> Result<SoftwareActionResult> {
        Ok(SoftwareActionResult {
            adapter_id: self.config.id.clone(),
            action_kind: SoftwareActionKind::HealthCheck,
            priority: ControlPriority::DesktopRecognition,
            ok: true,
            message: "desktop recognition request adapter ready".to_string(),
            artifacts: Vec::new(),
        })
    }

    fn execute(&self, action: SoftwareControlAction) -> Result<SoftwareActionResult> {
        let request = desktop_recognition_request(&action)?;
        fs::create_dir_all(&request.control_dir).with_context(|| {
            format!(
                "create desktop recognition control dir {}",
                request.control_dir
            )
        })?;
        let request_path = Path::new(&request.control_dir)
            .join(format!("desktop-recognition-{}.json", request.id));
        let body = json!({
            "id": request.id,
            "adapter_id": action.adapter_id.clone(),
            "action_kind": action.action_kind.clone(),
            "priority": action.priority.clone(),
            "status": "queued_for_desktop_recognition",
            "instruction": request.instruction,
            "target_window": request.target_window,
            "visual_targets": request.visual_targets,
            "pool_desktop_action": desktop_action_metadata(&action),
            "desktop_payload": desktop_payload(&action, &request),
            "payload": action.payload_json.clone(),
            "created_at": chrono::Utc::now(),
        });
        fs::write(
            &request_path,
            serde_json::to_string_pretty(&body).context("serialize desktop recognition request")?,
        )
        .with_context(|| {
            format!(
                "write desktop recognition request {}",
                request_path.display()
            )
        })?;

        let mut artifacts = request.artifacts;
        artifacts.push(request_path.to_string_lossy().to_string());
        artifacts.push(format!(
            "desktop-recognition://{}/{}",
            self.config.id, request.id
        ));

        Ok(SoftwareActionResult {
            adapter_id: self.config.id.clone(),
            action_kind: action.action_kind,
            priority: action.priority,
            ok: true,
            message: format!(
                "desktop recognition request staged: {}",
                request_path.display()
            ),
            artifacts,
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
struct CommandSoftwarePayload {
    command: String,
    #[serde(default)]
    allowed_commands: Vec<String>,
    working_dir: Option<String>,
    #[serde(default = "default_command_timeout_ms")]
    timeout_ms: u64,
    #[serde(default = "default_command_max_output_bytes")]
    max_output_bytes: usize,
    #[serde(default)]
    artifacts: Vec<String>,
}

struct DesktopRecognitionRequest {
    id: String,
    control_dir: String,
    instruction: String,
    target_window: Option<String>,
    visual_targets: Vec<String>,
    artifacts: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
struct DesktopActionProfile {
    profile_id: &'static str,
    operation: &'static str,
    desktop_tool: &'static str,
    stage: &'static str,
    expected_artifacts: &'static [&'static str],
}

const DESKTOP_REQUEST_ARTIFACTS: &[&str] = &["desktop_request", "screen_trace"];
const DESKTOP_OUTPUT_ARTIFACTS: &[&str] = &["desktop_request", "output_file", "screen_trace"];

struct CommandSoftwareExecution {
    ok: bool,
    attempted: bool,
    allowed: bool,
    argv: Vec<String>,
    exit_code: Option<i32>,
    stdout: String,
    stderr: String,
    error: Option<String>,
}

impl CommandSoftwareExecution {
    fn message(&self) -> String {
        if let Some(error) = &self.error {
            return error.clone();
        }
        format!(
            "command attempted={} allowed={} exit_code={:?} stdout={} stderr={}",
            self.attempted,
            self.allowed,
            self.exit_code,
            compact_output(&self.stdout),
            compact_output(&self.stderr)
        )
    }
}

fn desktop_action_profile(action_kind: &SoftwareActionKind) -> DesktopActionProfile {
    match action_kind {
        SoftwareActionKind::OpenProject => DesktopActionProfile {
            profile_id: "desktop-open-project",
            operation: "open_project",
            desktop_tool: "desktop.open_project",
            stage: "project_bootstrap",
            expected_artifacts: DESKTOP_REQUEST_ARTIFACTS,
        },
        SoftwareActionKind::ImportAsset => DesktopActionProfile {
            profile_id: "desktop-import-asset",
            operation: "import_asset",
            desktop_tool: "desktop.import_asset",
            stage: "asset_ingest",
            expected_artifacts: DESKTOP_REQUEST_ARTIFACTS,
        },
        SoftwareActionKind::CreateScene => DesktopActionProfile {
            profile_id: "desktop-create-scene",
            operation: "create_scene",
            desktop_tool: "desktop.create_scene",
            stage: "scene_assembly",
            expected_artifacts: DESKTOP_REQUEST_ARTIFACTS,
        },
        SoftwareActionKind::RunViewport => DesktopActionProfile {
            profile_id: "desktop-run-preview",
            operation: "run_preview",
            desktop_tool: "desktop.run_preview",
            stage: "interactive_preview",
            expected_artifacts: DESKTOP_REQUEST_ARTIFACTS,
        },
        SoftwareActionKind::Render => DesktopActionProfile {
            profile_id: "desktop-render",
            operation: "render_output",
            desktop_tool: "desktop.render_output",
            stage: "video_output",
            expected_artifacts: DESKTOP_OUTPUT_ARTIFACTS,
        },
        SoftwareActionKind::Transcode => DesktopActionProfile {
            profile_id: "desktop-transcode",
            operation: "transcode_output",
            desktop_tool: "desktop.transcode_output",
            stage: "delivery_output",
            expected_artifacts: DESKTOP_OUTPUT_ARTIFACTS,
        },
        SoftwareActionKind::ExportBuild => DesktopActionProfile {
            profile_id: "desktop-export-build",
            operation: "export_build",
            desktop_tool: "desktop.export_build",
            stage: "game_output",
            expected_artifacts: DESKTOP_OUTPUT_ARTIFACTS,
        },
        SoftwareActionKind::DesktopClick => DesktopActionProfile {
            profile_id: "desktop-click",
            operation: "click_target",
            desktop_tool: "desktop.click",
            stage: "desktop_interaction",
            expected_artifacts: DESKTOP_REQUEST_ARTIFACTS,
        },
        SoftwareActionKind::DesktopHotkey => DesktopActionProfile {
            profile_id: "desktop-hotkey",
            operation: "send_hotkey",
            desktop_tool: "desktop.hotkey",
            stage: "desktop_interaction",
            expected_artifacts: DESKTOP_REQUEST_ARTIFACTS,
        },
        _ => DesktopActionProfile {
            profile_id: "desktop-generic-control",
            operation: "generic_control",
            desktop_tool: "desktop.execute",
            stage: "desktop_control",
            expected_artifacts: DESKTOP_REQUEST_ARTIFACTS,
        },
    }
}

fn desktop_action_contracts() -> Vec<Value> {
    [
        SoftwareActionKind::OpenProject,
        SoftwareActionKind::ImportAsset,
        SoftwareActionKind::CreateScene,
        SoftwareActionKind::RunViewport,
        SoftwareActionKind::Render,
        SoftwareActionKind::Transcode,
        SoftwareActionKind::ExportBuild,
        SoftwareActionKind::DesktopClick,
        SoftwareActionKind::DesktopHotkey,
    ]
    .into_iter()
    .map(|action_kind| {
        let profile = desktop_action_profile(&action_kind);
        json!({
            "action_kind": action_kind,
            "profile_id": profile.profile_id,
            "operation": profile.operation,
            "desktop_tool": profile.desktop_tool,
            "stage": profile.stage,
            "expected_artifacts": profile.expected_artifacts,
        })
    })
    .collect()
}

fn desktop_action_metadata(action: &SoftwareControlAction) -> Value {
    let profile = desktop_action_profile(&action.action_kind);
    json!({
        "profile_id": profile.profile_id,
        "operation": profile.operation,
        "desktop_tool": profile.desktop_tool,
        "stage": profile.stage,
        "adapter_id": &action.adapter_id,
        "control_priority": &action.priority,
        "target_window": desktop_target_window(&action.adapter_id, &action.payload_json),
        "expected_artifacts": profile.expected_artifacts,
        "output_contract": "desktop-recognition-control-request",
    })
}

fn desktop_payload(action: &SoftwareControlAction, request: &DesktopRecognitionRequest) -> Value {
    if let Some(custom) = action.payload_json.get("desktop_payload") {
        return custom.clone();
    }
    let profile = desktop_action_profile(&action.action_kind);
    json!({
        "tool": profile.desktop_tool,
        "operation": profile.operation,
        "instruction": &request.instruction,
        "target_window": &request.target_window,
        "visual_targets": &request.visual_targets,
        "arguments": desktop_arguments(action, request),
        "handoff": {
            "stage": profile.stage,
            "expected_artifacts": profile.expected_artifacts,
            "control_priority": &action.priority,
        }
    })
}

fn desktop_arguments(action: &SoftwareControlAction, request: &DesktopRecognitionRequest) -> Value {
    match action.action_kind {
        SoftwareActionKind::DesktopClick => json!({
            "click_target": payload_string_any(&action.payload_json, &["click_target", "button", "label"]),
            "coordinates": action.payload_json.get("coordinates").cloned(),
            "visual_targets": &request.visual_targets,
        }),
        SoftwareActionKind::DesktopHotkey => json!({
            "hotkey": payload_string_any(&action.payload_json, &["hotkey", "shortcut"]),
            "keys": payload_string_array_any(&action.payload_json, &["keys", "hotkeys"]),
            "target_window": &request.target_window,
        }),
        SoftwareActionKind::RunViewport => json!({
            "mode": payload_string_any(&action.payload_json, &["mode", "play_mode", "perform_mode"])
                .unwrap_or_else(|| "perform_or_preview".to_string()),
            "cue": payload_string_any(&action.payload_json, &["cue", "scene", "level"]),
            "visual_targets": &request.visual_targets,
        }),
        SoftwareActionKind::Render
        | SoftwareActionKind::Transcode
        | SoftwareActionKind::ExportBuild => json!({
            "output_dir": payload_string_any(&action.payload_json, &["output_dir", "render_dir", "build_dir"]),
            "preset": payload_string_any(&action.payload_json, &["preset", "render_preset", "export_preset"]),
            "visual_targets": &request.visual_targets,
        }),
        _ => json!({
            "payload": &action.payload_json,
            "visual_targets": &request.visual_targets,
        }),
    }
}

fn desktop_target_window(adapter_id: &str, payload: &Value) -> Option<String> {
    payload_string_any(payload, &["target_window", "window", "app_window"]).or_else(|| {
        match adapter_id {
            "resolve" => Some("DaVinci Resolve".to_string()),
            "touchdesigner" => Some("TouchDesigner".to_string()),
            "madmapper" => Some("MadMapper".to_string()),
            "blender" => Some("Blender".to_string()),
            "unity" => Some("Unity".to_string()),
            "unreal" => Some("Unreal Editor".to_string()),
            "nuke" => Some("Nuke".to_string()),
            "editing-suite" => Some("Editing Suite".to_string()),
            _ => None,
        }
    })
}

fn command_payload(action: &SoftwareControlAction) -> Result<CommandSoftwarePayload> {
    if action.action_kind != SoftwareActionKind::ExecuteCli {
        bail!(
            "command software adapter requires ExecuteCli action, got {:?}",
            action.action_kind
        );
    }
    let payload: CommandSoftwarePayload =
        serde_json::from_value(action.payload_json.clone()).context("parse command payload")?;
    if payload.command.trim().is_empty() {
        bail!("command software payload requires command");
    }
    if payload.allowed_commands.is_empty() {
        bail!("command software payload requires allowed_commands");
    }
    Ok(payload)
}

fn desktop_recognition_request(
    action: &SoftwareControlAction,
) -> Result<DesktopRecognitionRequest> {
    let control_dir = payload_string(&action.payload_json, "control_dir")
        .unwrap_or_else(|| "target/pool-desktop-recognition".to_string());
    let instruction = payload_string(&action.payload_json, "instruction").unwrap_or_else(|| {
        format!(
            "Use desktop recognition to perform {:?} in {}",
            action.action_kind, action.adapter_id
        )
    });
    let visual_targets = action
        .payload_json
        .get("visual_targets")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default();
    let artifacts = action
        .payload_json
        .get("artifacts")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default();

    Ok(DesktopRecognitionRequest {
        id: uuid::Uuid::new_v4().to_string(),
        control_dir,
        instruction,
        target_window: desktop_target_window(&action.adapter_id, &action.payload_json),
        visual_targets,
        artifacts,
    })
}

fn execute_command_payload(payload: &CommandSoftwarePayload) -> CommandSoftwareExecution {
    let argv = match parse_command_line(&payload.command) {
        Ok(argv) => argv,
        Err(error) => {
            return CommandSoftwareExecution {
                ok: false,
                attempted: false,
                allowed: false,
                argv: Vec::new(),
                exit_code: None,
                stdout: String::new(),
                stderr: String::new(),
                error: Some(error),
            };
        }
    };
    if argv.is_empty() {
        return CommandSoftwareExecution {
            ok: false,
            attempted: false,
            allowed: false,
            argv,
            exit_code: None,
            stdout: String::new(),
            stderr: String::new(),
            error: Some("software command is empty".to_string()),
        };
    }
    if !command_allowed(&argv[0], &payload.allowed_commands) {
        return CommandSoftwareExecution {
            ok: false,
            attempted: false,
            allowed: false,
            argv,
            exit_code: None,
            stdout: String::new(),
            stderr: String::new(),
            error: Some("software command is not in allowlist".to_string()),
        };
    }

    let mut process = Command::new(&argv[0]);
    process.args(&argv[1..]);
    process.stdout(Stdio::piped());
    process.stderr(Stdio::piped());
    if let Some(working_dir) = &payload.working_dir {
        process.current_dir(working_dir);
    }

    let mut child = match process.spawn() {
        Ok(child) => child,
        Err(error) => {
            return CommandSoftwareExecution {
                ok: false,
                attempted: true,
                allowed: true,
                argv,
                exit_code: None,
                stdout: String::new(),
                stderr: String::new(),
                error: Some(format!("failed to spawn software command: {error}")),
            };
        }
    };
    let started = Instant::now();
    let timeout = Duration::from_millis(payload.timeout_ms.max(1));
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                return match child.wait_with_output() {
                    Ok(output) => CommandSoftwareExecution {
                        ok: output.status.success(),
                        attempted: true,
                        allowed: true,
                        argv,
                        exit_code: output.status.code(),
                        stdout: truncate_utf8(&output.stdout, payload.max_output_bytes),
                        stderr: truncate_utf8(&output.stderr, payload.max_output_bytes),
                        error: None,
                    },
                    Err(error) => CommandSoftwareExecution {
                        ok: false,
                        attempted: true,
                        allowed: true,
                        argv,
                        exit_code: None,
                        stdout: String::new(),
                        stderr: String::new(),
                        error: Some(format!(
                            "failed to collect software command output: {error}"
                        )),
                    },
                };
            }
            Ok(None) => {
                if started.elapsed() >= timeout {
                    let _ = child.kill();
                    return match child.wait_with_output() {
                        Ok(output) => CommandSoftwareExecution {
                            ok: false,
                            attempted: true,
                            allowed: true,
                            argv,
                            exit_code: output.status.code(),
                            stdout: truncate_utf8(&output.stdout, payload.max_output_bytes),
                            stderr: truncate_utf8(&output.stderr, payload.max_output_bytes),
                            error: Some(format!(
                                "software command timed out after {} ms",
                                payload.timeout_ms
                            )),
                        },
                        Err(error) => CommandSoftwareExecution {
                            ok: false,
                            attempted: true,
                            allowed: true,
                            argv,
                            exit_code: None,
                            stdout: String::new(),
                            stderr: String::new(),
                            error: Some(format!(
                                "software command timed out and output collection failed: {error}"
                            )),
                        },
                    };
                }
                thread::sleep(Duration::from_millis(10));
            }
            Err(error) => {
                let _ = child.kill();
                return CommandSoftwareExecution {
                    ok: false,
                    attempted: true,
                    allowed: true,
                    argv,
                    exit_code: None,
                    stdout: String::new(),
                    stderr: String::new(),
                    error: Some(format!("failed to wait for software command: {error}")),
                };
            }
        }
    }
}

impl Default for MockUnrealAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl SoftwareAdapter for MockUnrealAdapter {
    fn config(&self) -> &SoftwareAdapterConfig {
        &self.config
    }

    fn health(&self) -> Result<SoftwareActionResult> {
        Ok(SoftwareActionResult {
            adapter_id: self.config.id.clone(),
            action_kind: SoftwareActionKind::HealthCheck,
            priority: ControlPriority::ApiMcp,
            ok: true,
            message: "mock Unreal adapter ready".to_string(),
            artifacts: Vec::new(),
        })
    }

    fn execute(&self, action: SoftwareControlAction) -> Result<SoftwareActionResult> {
        Ok(SoftwareActionResult {
            adapter_id: self.config.id.clone(),
            action_kind: action.action_kind,
            priority: action.priority,
            ok: true,
            message: "mock Unreal action accepted".to_string(),
            artifacts: vec!["unreal://mock/viewport".to_string()],
        })
    }
}

impl SoftwareAdapter for UnrealMcpAdapter {
    fn config(&self) -> &SoftwareAdapterConfig {
        &self.config
    }

    fn health(&self) -> Result<SoftwareActionResult> {
        if !self.is_configured() {
            return Ok(SoftwareActionResult {
                adapter_id: self.config.id.clone(),
                action_kind: SoftwareActionKind::HealthCheck,
                priority: ControlPriority::ApiMcp,
                ok: false,
                message: "Unreal MCP endpoint is not configured".to_string(),
                artifacts: Vec::new(),
            });
        }

        let result = match run_async_http(self.health_request()) {
            Ok(result) => result,
            Err(error) => UnrealMcpHttpResult {
                ok: false,
                message: format!("Unreal MCP health failed: {error}"),
                artifacts: Vec::new(),
            },
        };
        Ok(SoftwareActionResult {
            adapter_id: self.config.id.clone(),
            action_kind: SoftwareActionKind::HealthCheck,
            priority: ControlPriority::ApiMcp,
            ok: result.ok,
            message: result.message,
            artifacts: result.artifacts,
        })
    }

    fn execute(&self, action: SoftwareControlAction) -> Result<SoftwareActionResult> {
        if !self.is_configured() {
            return Ok(SoftwareActionResult {
                adapter_id: self.config.id.clone(),
                action_kind: action.action_kind,
                priority: action.priority,
                ok: false,
                message: "Unreal MCP endpoint is not configured".to_string(),
                artifacts: Vec::new(),
            });
        }

        let priority = action.priority.clone();
        let action_kind = action.action_kind.clone();
        let result = match run_async_http(self.execute_request(&action)) {
            Ok(result) => result,
            Err(error) => UnrealMcpHttpResult {
                ok: false,
                message: format!("Unreal MCP action failed: {error}"),
                artifacts: Vec::new(),
            },
        };
        Ok(SoftwareActionResult {
            adapter_id: self.config.id.clone(),
            action_kind,
            priority,
            ok: result.ok,
            message: result.message,
            artifacts: result.artifacts,
        })
    }
}

impl SoftwareAdapter for HermesMcpAdapter {
    fn config(&self) -> &SoftwareAdapterConfig {
        &self.config
    }

    fn health(&self) -> Result<SoftwareActionResult> {
        if !self.is_configured() {
            return Ok(SoftwareActionResult {
                adapter_id: self.config.id.clone(),
                action_kind: SoftwareActionKind::HealthCheck,
                priority: ControlPriority::ApiMcp,
                ok: false,
                message: "Hermes MCP endpoint is not configured".to_string(),
                artifacts: Vec::new(),
            });
        }

        let result = match run_async_http(self.health_request()) {
            Ok(result) => result,
            Err(error) => HermesMcpHttpResult {
                ok: false,
                message: format!("Hermes MCP health failed: {error}"),
                artifacts: Vec::new(),
            },
        };
        Ok(SoftwareActionResult {
            adapter_id: self.config.id.clone(),
            action_kind: SoftwareActionKind::HealthCheck,
            priority: ControlPriority::ApiMcp,
            ok: result.ok,
            message: result.message,
            artifacts: result.artifacts,
        })
    }

    fn execute(&self, action: SoftwareControlAction) -> Result<SoftwareActionResult> {
        if !self.is_configured() {
            return Ok(SoftwareActionResult {
                adapter_id: self.config.id.clone(),
                action_kind: action.action_kind,
                priority: action.priority,
                ok: false,
                message: "Hermes MCP endpoint is not configured".to_string(),
                artifacts: Vec::new(),
            });
        }

        let priority = action.priority.clone();
        let action_kind = action.action_kind.clone();
        let result = match run_async_http(self.execute_request(&action)) {
            Ok(result) => result,
            Err(error) => HermesMcpHttpResult {
                ok: false,
                message: format!("Hermes MCP action failed: {error}"),
                artifacts: Vec::new(),
            },
        };
        Ok(SoftwareActionResult {
            adapter_id: self.config.id.clone(),
            action_kind,
            priority,
            ok: result.ok,
            message: result.message,
            artifacts: result.artifacts,
        })
    }
}

impl SoftwareAdapter for GenericSoftwareApiAdapter {
    fn config(&self) -> &SoftwareAdapterConfig {
        &self.config
    }

    fn health(&self) -> Result<SoftwareActionResult> {
        if !self.is_configured() {
            return Ok(SoftwareActionResult {
                adapter_id: self.config.id.clone(),
                action_kind: SoftwareActionKind::HealthCheck,
                priority: ControlPriority::ApiMcp,
                ok: false,
                message: format!("{} API/MCP endpoint is not configured", self.config.id),
                artifacts: Vec::new(),
            });
        }

        let result = match run_async_http(self.health_request()) {
            Ok(result) => result,
            Err(error) => GenericSoftwareApiHttpResult {
                ok: false,
                message: format!("{} API/MCP health failed: {error}", self.config.id),
                artifacts: Vec::new(),
            },
        };
        Ok(SoftwareActionResult {
            adapter_id: self.config.id.clone(),
            action_kind: SoftwareActionKind::HealthCheck,
            priority: ControlPriority::ApiMcp,
            ok: result.ok,
            message: result.message,
            artifacts: result.artifacts,
        })
    }

    fn execute(&self, action: SoftwareControlAction) -> Result<SoftwareActionResult> {
        if !self.is_configured() {
            return Ok(SoftwareActionResult {
                adapter_id: self.config.id.clone(),
                action_kind: action.action_kind,
                priority: action.priority,
                ok: false,
                message: format!("{} API/MCP endpoint is not configured", self.config.id),
                artifacts: Vec::new(),
            });
        }

        let priority = action.priority.clone();
        let action_kind = action.action_kind.clone();
        let result = match run_async_http(self.execute_request(&action)) {
            Ok(result) => result,
            Err(error) => GenericSoftwareApiHttpResult {
                ok: false,
                message: format!("{} API/MCP action failed: {error}", self.config.id),
                artifacts: Vec::new(),
            },
        };
        Ok(SoftwareActionResult {
            adapter_id: self.config.id.clone(),
            action_kind,
            priority,
            ok: result.ok,
            message: result.message,
            artifacts: result.artifacts,
        })
    }
}

impl UnrealMcpAdapter {
    async fn health_request(&self) -> Result<UnrealMcpHttpResult> {
        let mut builder = self.client.get(self.url(&self.options.health_path));
        if let Some(token) = &self.options.auth_token {
            builder = builder.bearer_auth(token);
        }
        let response = builder.send().await.context("call Unreal MCP health")?;
        response_to_unreal_result(response, "Unreal MCP health").await
    }

    async fn execute_request(&self, action: &SoftwareControlAction) -> Result<UnrealMcpHttpResult> {
        let body = unreal_mcp_request_body(action);
        let mut builder = self
            .client
            .post(self.url(&self.options.action_path))
            .json(&body);
        if let Some(token) = &self.options.auth_token {
            builder = builder.bearer_auth(token);
        }
        let response = builder.send().await.context("call Unreal MCP action")?;
        response_to_unreal_result(response, "Unreal MCP action").await
    }
}

impl HermesMcpAdapter {
    async fn health_request(&self) -> Result<HermesMcpHttpResult> {
        let mut builder = self.client.get(self.url(&self.options.health_path));
        if let Some(token) = &self.options.auth_token {
            builder = builder.bearer_auth(token);
        }
        let response = builder.send().await.context("call Hermes MCP health")?;
        response_to_hermes_result(response, "Hermes MCP health").await
    }

    async fn execute_request(&self, action: &SoftwareControlAction) -> Result<HermesMcpHttpResult> {
        let body = hermes_mcp_request_body(action);
        let mut builder = self
            .client
            .post(self.url(&self.options.action_path))
            .json(&body);
        if let Some(token) = &self.options.auth_token {
            builder = builder.bearer_auth(token);
        }
        let response = builder.send().await.context("call Hermes MCP action")?;
        response_to_hermes_result(response, "Hermes MCP action").await
    }
}

impl GenericSoftwareApiAdapter {
    async fn health_request(&self) -> Result<GenericSoftwareApiHttpResult> {
        let mut builder = self.client.get(self.url(&self.options.health_path));
        if let Some(token) = &self.options.auth_token {
            builder = builder.bearer_auth(token);
        }
        let response = builder
            .send()
            .await
            .with_context(|| format!("call {} API/MCP health", self.config.id))?;
        response_to_generic_software_result(response, &format!("{} API/MCP health", self.config.id))
            .await
    }

    async fn execute_request(
        &self,
        action: &SoftwareControlAction,
    ) -> Result<GenericSoftwareApiHttpResult> {
        let body = generic_software_api_request_body(&self.config, action);
        let mut builder = self
            .client
            .post(self.url(&self.options.action_path))
            .json(&body);
        if let Some(token) = &self.options.auth_token {
            builder = builder.bearer_auth(token);
        }
        let response = builder
            .send()
            .await
            .with_context(|| format!("call {} API/MCP action", self.config.id))?;
        response_to_generic_software_result(response, &format!("{} API/MCP action", self.config.id))
            .await
    }
}

#[derive(Debug, Clone, Copy)]
struct UnrealActionProfile {
    profile_id: &'static str,
    operation: &'static str,
    mcp_tool: &'static str,
    stage: &'static str,
    expected_artifacts: &'static [&'static str],
}

const UNREAL_LEVEL_ARTIFACTS: &[&str] = &["level", "viewport"];
const UNREAL_ASSET_ARTIFACTS: &[&str] = &["asset", "import_report"];
const UNREAL_RENDER_ARTIFACTS: &[&str] = &["sequence", "render_output"];
const UNREAL_BUILD_ARTIFACTS: &[&str] = &["build", "manifest"];
const UNREAL_PROJECT_ARTIFACTS: &[&str] = &["project"];

fn unreal_action_profile(action_kind: &SoftwareActionKind) -> UnrealActionProfile {
    match action_kind {
        SoftwareActionKind::OpenProject => UnrealActionProfile {
            profile_id: "unreal-open-project",
            operation: "open_project",
            mcp_tool: "unreal.open_project",
            stage: "project_bootstrap",
            expected_artifacts: UNREAL_PROJECT_ARTIFACTS,
        },
        SoftwareActionKind::ImportAsset => UnrealActionProfile {
            profile_id: "unreal-import-asset",
            operation: "import_asset",
            mcp_tool: "unreal.import_asset",
            stage: "asset_ingest",
            expected_artifacts: UNREAL_ASSET_ARTIFACTS,
        },
        SoftwareActionKind::CreateScene => UnrealActionProfile {
            profile_id: "unreal-create-scene",
            operation: "create_scene",
            mcp_tool: "unreal.create_scene",
            stage: "scene_assembly",
            expected_artifacts: UNREAL_LEVEL_ARTIFACTS,
        },
        SoftwareActionKind::RunViewport => UnrealActionProfile {
            profile_id: "unreal-run-viewport",
            operation: "run_viewport",
            mcp_tool: "unreal.run_viewport",
            stage: "interactive_preview",
            expected_artifacts: UNREAL_LEVEL_ARTIFACTS,
        },
        SoftwareActionKind::Render => UnrealActionProfile {
            profile_id: "unreal-render",
            operation: "render_sequence",
            mcp_tool: "unreal.render_sequence",
            stage: "video_output",
            expected_artifacts: UNREAL_RENDER_ARTIFACTS,
        },
        SoftwareActionKind::ExportBuild => UnrealActionProfile {
            profile_id: "unreal-export-build",
            operation: "export_build",
            mcp_tool: "unreal.export_build",
            stage: "game_output",
            expected_artifacts: UNREAL_BUILD_ARTIFACTS,
        },
        SoftwareActionKind::Transcode => UnrealActionProfile {
            profile_id: "unreal-transcode",
            operation: "transcode_media",
            mcp_tool: "unreal.transcode_media",
            stage: "delivery_output",
            expected_artifacts: UNREAL_RENDER_ARTIFACTS,
        },
        SoftwareActionKind::HealthCheck => UnrealActionProfile {
            profile_id: "unreal-health-check",
            operation: "health_check",
            mcp_tool: "unreal.health",
            stage: "adapter_health",
            expected_artifacts: &[],
        },
        _ => UnrealActionProfile {
            profile_id: "unreal-generic-action",
            operation: "generic_action",
            mcp_tool: "unreal.execute_action",
            stage: "generic_control",
            expected_artifacts: &[],
        },
    }
}

fn unreal_action_contracts() -> Vec<Value> {
    [
        SoftwareActionKind::OpenProject,
        SoftwareActionKind::ImportAsset,
        SoftwareActionKind::CreateScene,
        SoftwareActionKind::RunViewport,
        SoftwareActionKind::Render,
        SoftwareActionKind::ExportBuild,
        SoftwareActionKind::Transcode,
        SoftwareActionKind::HealthCheck,
    ]
    .into_iter()
    .map(|action_kind| {
        let profile = unreal_action_profile(&action_kind);
        json!({
            "action_kind": action_kind,
            "profile_id": profile.profile_id,
            "operation": profile.operation,
            "mcp_tool": profile.mcp_tool,
            "stage": profile.stage,
            "expected_artifacts": profile.expected_artifacts,
            "output_contract": "unreal-mcp-action-result",
        })
    })
    .collect()
}

pub fn unreal_mcp_bridge_contract_resource() -> Value {
    json!({
        "kind": "pool_unreal_mcp_bridge_contract",
        "version": 1,
        "adapter_id": "unreal",
        "service": "pool-unreal-mcp-bridge",
        "purpose": "Contract for Unreal plugin/gateway implementers that receive Pool software actions and execute Unreal Editor operations.",
        "status": "bridge_contract_ready_plugin_execution_pending",
        "control_priority": "API/MCP",
        "pool_runtime_routes": {
            "contract_http": "/api/unreal-mcp-bridge",
            "contract_mcp": "pool://unreal-mcp-bridge",
            "software_contract_http": "/api/software-contracts?adapter_id=unreal",
            "software_contract_mcp": "pool://software-contracts/unreal",
            "action_submit": "/api/software-actions",
            "health_submit": "/api/software-health",
        },
        "environment": {
            "endpoint": "POOL_UNREAL_MCP_ENDPOINT",
            "auth_token": "POOL_UNREAL_MCP_TOKEN",
            "health_path": "POOL_UNREAL_MCP_HEALTH_PATH",
            "action_path": "POOL_UNREAL_MCP_ACTION_PATH",
        },
        "transport": {
            "default_health": {
                "method": "GET",
                "path": "/health",
                "expected_response": {
                    "ok": true,
                    "message": "unreal-health-ok"
                }
            },
            "default_action": {
                "method": "POST",
                "path": "/mcp",
                "content_type": "application/json"
            },
            "auth": {
                "header": "Authorization: Bearer <POOL_UNREAL_MCP_TOKEN>",
                "optional_for_localhost": true
            }
        },
        "request_contract": {
            "root_required_fields": [
                "adapter_id",
                "action_kind",
                "priority",
                "payload",
                "pool_unreal_action",
                "mcp_payload"
            ],
            "pool_unreal_action": {
                "description": "Pool-normalized metadata for audit, PRD readiness, and expected output handling.",
                "required_fields": [
                    "profile_id",
                    "operation",
                    "mcp_tool",
                    "stage",
                    "expected_artifacts",
                    "output_contract"
                ]
            },
            "mcp_payload": {
                "description": "Unreal-facing tool invocation. A plugin may implement MCP directly or provide an HTTP gateway that accepts this shape.",
                "required_fields": ["tool", "operation", "arguments", "handoff"]
            }
        },
        "action_profiles": unreal_action_contracts(),
        "tool_contracts": unreal_action_contracts()
            .into_iter()
            .map(|profile| json!({
                "tool": profile["mcp_tool"].clone(),
                "operation": profile["operation"].clone(),
                "profile_id": profile["profile_id"].clone(),
                "stage": profile["stage"].clone(),
                "required_input": unreal_bridge_required_input(
                    profile["operation"].as_str().unwrap_or_default()
                ),
                "expected_artifacts": profile["expected_artifacts"].clone(),
            }))
            .collect::<Vec<_>>(),
        "response_contract": {
            "accepted_success_fields": ["ok", "success"],
            "accepted_status_fields": ["status", "state"],
            "message_field": "message",
            "artifacts_field": "artifacts",
            "artifact_policy": "Return local file paths, unreal:// engine URIs, or Pool output paths. Remote URLs are provenance only and must be materialized before frontend loading.",
            "minimum_success_response": {
                "ok": true,
                "message": "unreal-action-ok",
                "artifacts": ["unreal://level/demo"]
            }
        },
        "operator_checks": [
            "Unreal project is open or open_project tool can open the target .uproject.",
            "Generated media/3D/3DGS assets referenced by asset_paths exist as local files.",
            "Plugin logs the received pool_unreal_action.profile_id and mcp_payload.tool for audit.",
            "CreateScene imports assets, creates or updates a level, sets camera and lighting, and returns a level/viewport artifact.",
            "Render writes a local sequence output path before reporting success.",
            "ExportBuild writes a local build or manifest path before reporting success."
        ],
        "local_worker": {
            "service": "pool-unreal-mcp-bridge",
            "cli": "pool-cli unreal-mcp-bridge-worker --bind 127.0.0.1:8790 --output-root worlds/demo/output",
            "routes": {
                "health": "GET /health",
                "action": "POST /mcp"
            },
            "modes": {
                "dry_run": "Validates Pool wrapper and writes local request/response audit files without launching Unreal.",
                "forwarder": "Set --upstream to validate/log the Pool body before forwarding to a real Unreal plugin/gateway endpoint."
            },
            "endpoint_env": "POOL_UNREAL_MCP_ENDPOINT=http://127.0.0.1:8790"
        },
        "fallback": {
            "desktop_recognition_contract": "pool://desktop-recognition-contract",
            "human_takeover_status": "waiting_approval"
        }
    })
}

fn unreal_bridge_required_input(operation: &str) -> Value {
    match operation {
        "open_project" => json!(["project_file"]),
        "import_asset" => json!(["asset_paths", "destination"]),
        "create_scene" => json!(["level", "asset_paths", "camera", "lighting", "world_origin"]),
        "run_viewport" => json!(["level", "camera", "play_mode"]),
        "render_sequence" => json!(["sequence", "output_dir", "preset"]),
        "export_build" => json!(["target_platform", "output_dir", "configuration"]),
        "transcode_media" => json!(["input_path", "output_path", "preset"]),
        "health_check" => json!([]),
        _ => json!(["arguments"]),
    }
}

fn unreal_mcp_request_body(action: &SoftwareControlAction) -> Value {
    let profile = unreal_action_profile(&action.action_kind);
    let custom_mcp_payload = action.payload_json.get("mcp_payload").cloned();
    json!({
        "adapter_id": action.adapter_id.clone(),
        "action_kind": action.action_kind.clone(),
        "priority": action.priority.clone(),
        "payload": action.payload_json.clone(),
        "requires_confirmation": action.requires_confirmation,
        "pool_unreal_action": unreal_action_metadata(&profile, action),
        "mcp_payload": custom_mcp_payload.unwrap_or_else(|| default_unreal_mcp_payload(&profile, action)),
    })
}

fn unreal_action_metadata(profile: &UnrealActionProfile, action: &SoftwareControlAction) -> Value {
    json!({
        "profile_id": profile.profile_id,
        "operation": profile.operation,
        "mcp_tool": profile.mcp_tool,
        "stage": profile.stage,
        "control_priority": action.priority.clone(),
        "expected_artifacts": profile.expected_artifacts,
        "output_contract": "unreal-mcp-action-result",
    })
}

fn default_unreal_mcp_payload(
    profile: &UnrealActionProfile,
    action: &SoftwareControlAction,
) -> Value {
    json!({
        "tool": profile.mcp_tool,
        "operation": profile.operation,
        "arguments": unreal_mcp_arguments(profile, &action.payload_json),
        "handoff": {
            "stage": profile.stage,
            "expected_artifacts": profile.expected_artifacts,
            "control_priority": action.priority.clone(),
        }
    })
}

fn unreal_mcp_arguments(profile: &UnrealActionProfile, payload: &Value) -> Value {
    match profile.operation {
        "open_project" => json!({
            "project_file": payload_string_any(payload, &["project_file", "uproject_path", "path"]),
        }),
        "import_asset" => json!({
            "asset_paths": payload_string_array_any(payload, &["asset_paths", "assets", "input_paths"]),
            "destination": payload_string_any(payload, &["destination", "content_path"])
                .unwrap_or_else(|| "/Game/Pool/Imported".to_string()),
            "replace_existing": payload_bool(payload, "replace_existing").unwrap_or(false),
        }),
        "create_scene" => json!({
            "level": payload_string_any(payload, &["level", "level_name", "scene"])
                .unwrap_or_else(|| "pool_content_burst".to_string()),
            "asset_paths": payload_string_array_any(payload, &["asset_paths", "assets", "input_paths"]),
            "camera": payload_string_any(payload, &["camera", "camera_preset"])
                .unwrap_or_else(|| "hero_orbit".to_string()),
            "lighting": payload_string_any(payload, &["lighting", "lighting_preset"])
                .unwrap_or_else(|| "cinematic_day".to_string()),
            "world_origin": payload.get("world_origin").cloned().unwrap_or_else(|| json!([0, 0, 0])),
        }),
        "run_viewport" => json!({
            "level": payload_string_any(payload, &["level", "level_name", "scene"]),
            "camera": payload_string_any(payload, &["camera", "camera_preset"]),
            "play_mode": payload_string_any(payload, &["play_mode", "mode"])
                .unwrap_or_else(|| "simulate".to_string()),
        }),
        "render_sequence" => json!({
            "sequence": payload_string_any(payload, &["sequence", "sequencer", "timeline"])
                .unwrap_or_else(|| "main".to_string()),
            "output_dir": payload_string_any(payload, &["output_dir", "render_dir"])
                .unwrap_or_else(|| "worlds/demo/output/renders".to_string()),
            "preset": payload_string_any(payload, &["preset", "render_preset"])
                .unwrap_or_else(|| "preview_1080p".to_string()),
        }),
        "export_build" => json!({
            "target_platform": payload_string_any(payload, &["target_platform", "platform"])
                .unwrap_or_else(|| "Mac".to_string()),
            "output_dir": payload_string_any(payload, &["output_dir", "build_dir"])
                .unwrap_or_else(|| "worlds/demo/output/builds".to_string()),
            "configuration": payload_string_any(payload, &["configuration", "config"])
                .unwrap_or_else(|| "Development".to_string()),
        }),
        "transcode_media" => json!({
            "input_path": payload_string_any(payload, &["input_path", "source"]),
            "output_path": payload_string_any(payload, &["output_path", "target"]),
            "preset": payload_string_any(payload, &["preset", "transcode_preset"])
                .unwrap_or_else(|| "h264_preview".to_string()),
        }),
        _ => payload.clone(),
    }
}

#[derive(Debug, Clone, Copy)]
struct HermesActionProfile {
    profile_id: &'static str,
    operation: &'static str,
    mcp_tool: &'static str,
    stage: &'static str,
    expected_artifacts: &'static [&'static str],
}

const HERMES_SESSION_ARTIFACTS: &[&str] = &["session", "transcript", "task_plan"];
const HERMES_EXECUTION_ARTIFACTS: &[&str] = &["session", "execution_report", "transcript"];

fn hermes_action_profile(action_kind: &SoftwareActionKind) -> HermesActionProfile {
    match action_kind {
        SoftwareActionKind::HealthCheck => HermesActionProfile {
            profile_id: "hermes-health-check",
            operation: "health_check",
            mcp_tool: "hermes.health",
            stage: "adapter_health",
            expected_artifacts: &[],
        },
        SoftwareActionKind::OpenProject => HermesActionProfile {
            profile_id: "hermes-open-project",
            operation: "open_project_context",
            mcp_tool: "hermes.open_project",
            stage: "project_context",
            expected_artifacts: HERMES_SESSION_ARTIFACTS,
        },
        SoftwareActionKind::ImportAsset | SoftwareActionKind::CreateScene => HermesActionProfile {
            profile_id: "hermes-coordinate-software",
            operation: "coordinate_software_action",
            mcp_tool: "hermes.coordinate",
            stage: "agent_orchestration",
            expected_artifacts: HERMES_SESSION_ARTIFACTS,
        },
        SoftwareActionKind::RunViewport => HermesActionProfile {
            profile_id: "hermes-run-preview",
            operation: "run_preview_control",
            mcp_tool: "hermes.run_preview",
            stage: "interactive_preview",
            expected_artifacts: HERMES_EXECUTION_ARTIFACTS,
        },
        SoftwareActionKind::Render
        | SoftwareActionKind::Transcode
        | SoftwareActionKind::ExportBuild => HermesActionProfile {
            profile_id: "hermes-output-control",
            operation: "coordinate_output_action",
            mcp_tool: "hermes.output_control",
            stage: "output_orchestration",
            expected_artifacts: HERMES_EXECUTION_ARTIFACTS,
        },
        _ => HermesActionProfile {
            profile_id: "hermes-generic-control",
            operation: "generic_control",
            mcp_tool: "hermes.execute",
            stage: "agent_control",
            expected_artifacts: HERMES_SESSION_ARTIFACTS,
        },
    }
}

fn hermes_action_contracts() -> Vec<Value> {
    [
        SoftwareActionKind::HealthCheck,
        SoftwareActionKind::OpenProject,
        SoftwareActionKind::ImportAsset,
        SoftwareActionKind::CreateScene,
        SoftwareActionKind::RunViewport,
        SoftwareActionKind::Render,
        SoftwareActionKind::Transcode,
        SoftwareActionKind::ExportBuild,
    ]
    .into_iter()
    .map(|action_kind| {
        let profile = hermes_action_profile(&action_kind);
        json!({
            "action_kind": action_kind,
            "profile_id": profile.profile_id,
            "operation": profile.operation,
            "mcp_tool": profile.mcp_tool,
            "stage": profile.stage,
            "expected_artifacts": profile.expected_artifacts,
            "output_contract": "hermes-mcp-action-result",
        })
    })
    .collect()
}

fn hermes_mcp_request_body(action: &SoftwareControlAction) -> Value {
    let profile = hermes_action_profile(&action.action_kind);
    let custom_mcp_payload = action.payload_json.get("mcp_payload").cloned();
    json!({
        "adapter_id": action.adapter_id.clone(),
        "action_kind": action.action_kind.clone(),
        "priority": action.priority.clone(),
        "payload": action.payload_json.clone(),
        "requires_confirmation": action.requires_confirmation,
        "pool_hermes_action": hermes_action_metadata(&profile, action),
        "mcp_payload": custom_mcp_payload.unwrap_or_else(|| default_hermes_mcp_payload(&profile, action)),
    })
}

fn hermes_action_metadata(profile: &HermesActionProfile, action: &SoftwareControlAction) -> Value {
    json!({
        "profile_id": profile.profile_id,
        "operation": profile.operation,
        "mcp_tool": profile.mcp_tool,
        "stage": profile.stage,
        "control_priority": action.priority.clone(),
        "expected_artifacts": profile.expected_artifacts,
        "output_contract": "hermes-mcp-action-result",
    })
}

fn default_hermes_mcp_payload(
    profile: &HermesActionProfile,
    action: &SoftwareControlAction,
) -> Value {
    json!({
        "tool": profile.mcp_tool,
        "operation": profile.operation,
        "arguments": hermes_mcp_arguments(profile, action),
        "handoff": {
            "stage": profile.stage,
            "expected_artifacts": profile.expected_artifacts,
            "control_priority": action.priority.clone(),
        }
    })
}

fn hermes_mcp_arguments(profile: &HermesActionProfile, action: &SoftwareControlAction) -> Value {
    let payload = &action.payload_json;
    json!({
        "project_slug": payload_string_any(payload, &["project_slug", "project"])
            .unwrap_or_else(|| "demo".to_string()),
        "instruction": payload_string_any(payload, &["instruction", "prompt", "command"])
            .unwrap_or_else(|| default_hermes_instruction(profile, action)),
        "allowed_tools": payload_string_array_any(payload, &["allowed_tools", "tools"]),
        "target_adapter": payload_string_any(payload, &["target_adapter", "software", "tool"]),
        "target_action_kind": payload_string_any(payload, &["target_action_kind", "target_action"])
            .unwrap_or_else(|| format!("{:?}", action.action_kind)),
        "context": payload.get("context").cloned().unwrap_or_else(|| json!({})),
        "payload": payload.clone(),
    })
}

fn generic_software_api_request_body(
    config: &SoftwareAdapterConfig,
    action: &SoftwareControlAction,
) -> Value {
    let custom_mcp_payload = action.payload_json.get("mcp_payload").cloned();
    json!({
        "adapter_id": action.adapter_id.clone(),
        "action_kind": action.action_kind.clone(),
        "priority": action.priority.clone(),
        "payload": action.payload_json.clone(),
        "requires_confirmation": action.requires_confirmation,
        "pool_software_action": {
            "profile_id": format!("{}-generic-api-mcp", config.id),
            "operation": generic_software_operation(&action.action_kind),
            "stage": "software_api_mcp_control",
            "control_priority": action.priority.clone(),
            "expected_artifacts": payload_string_array_any(&action.payload_json, &["expected_artifacts", "artifacts"]),
            "output_contract": "pool-generic-software-api-result",
        },
        "mcp_payload": custom_mcp_payload.unwrap_or_else(|| json!({
            "tool": format!("{}.execute", config.id),
            "operation": generic_software_operation(&action.action_kind),
            "arguments": action.payload_json.clone(),
            "handoff": {
                "stage": "software_api_mcp_control",
                "control_priority": action.priority.clone(),
            }
        })),
    })
}

fn generic_software_operation(action_kind: &SoftwareActionKind) -> &'static str {
    match action_kind {
        SoftwareActionKind::HealthCheck => "health_check",
        SoftwareActionKind::OpenProject => "open_project",
        SoftwareActionKind::ImportAsset => "import_asset",
        SoftwareActionKind::CreateScene => "create_scene",
        SoftwareActionKind::RunViewport => "run_viewport",
        SoftwareActionKind::Render => "render",
        SoftwareActionKind::Transcode => "transcode",
        SoftwareActionKind::ExportBuild => "export_build",
        SoftwareActionKind::ExecuteCli => "execute_cli",
        SoftwareActionKind::DesktopClick => "desktop_click",
        SoftwareActionKind::DesktopHotkey => "desktop_hotkey",
    }
}

fn default_hermes_instruction(
    profile: &HermesActionProfile,
    action: &SoftwareControlAction,
) -> String {
    format!(
        "Hermes should {} for {:?} with {:?} priority",
        profile.operation, action.action_kind, action.priority
    )
}

struct UnrealMcpHttpResult {
    ok: bool,
    message: String,
    artifacts: Vec<String>,
}

struct HermesMcpHttpResult {
    ok: bool,
    message: String,
    artifacts: Vec<String>,
}

struct GenericSoftwareApiHttpResult {
    ok: bool,
    message: String,
    artifacts: Vec<String>,
}

async fn response_to_unreal_result(
    response: reqwest::Response,
    context_label: &str,
) -> Result<UnrealMcpHttpResult> {
    let status = response.status();
    let text = response
        .text()
        .await
        .with_context(|| format!("read {context_label} response"))?;
    let value: Option<Value> = serde_json::from_str(&text).ok();
    let ok = status.is_success() && json_bool(value.as_ref(), "ok").unwrap_or(true);
    let message = value
        .as_ref()
        .and_then(|value| {
            payload_string(value, "message").or_else(|| payload_string(value, "status"))
        })
        .unwrap_or_else(|| {
            if text.trim().is_empty() {
                format!("{context_label} returned HTTP {}", status.as_u16())
            } else {
                format!(
                    "{context_label} returned HTTP {}: {}",
                    status.as_u16(),
                    compact_output(&text)
                )
            }
        });
    let artifacts = value
        .as_ref()
        .map(collect_unreal_artifacts)
        .unwrap_or_default();

    Ok(UnrealMcpHttpResult {
        ok,
        message,
        artifacts,
    })
}

async fn response_to_hermes_result(
    response: reqwest::Response,
    context_label: &str,
) -> Result<HermesMcpHttpResult> {
    let status = response.status();
    let text = response
        .text()
        .await
        .with_context(|| format!("read {context_label} response"))?;
    let value: Option<Value> = serde_json::from_str(&text).ok();
    let ok = status.is_success() && json_bool(value.as_ref(), "ok").unwrap_or(true);
    let message = value
        .as_ref()
        .and_then(|value| {
            payload_string(value, "message")
                .or_else(|| payload_string(value, "status"))
                .or_else(|| payload_string(value, "summary"))
        })
        .unwrap_or_else(|| {
            if text.trim().is_empty() {
                format!("{context_label} returned HTTP {}", status.as_u16())
            } else {
                format!(
                    "{context_label} returned HTTP {}: {}",
                    status.as_u16(),
                    compact_output(&text)
                )
            }
        });
    let artifacts = value
        .as_ref()
        .map(collect_hermes_artifacts)
        .unwrap_or_default();

    Ok(HermesMcpHttpResult {
        ok,
        message,
        artifacts,
    })
}

async fn response_to_generic_software_result(
    response: reqwest::Response,
    context_label: &str,
) -> Result<GenericSoftwareApiHttpResult> {
    let status = response.status();
    let text = response
        .text()
        .await
        .with_context(|| format!("read {context_label} response"))?;
    let value: Option<Value> = serde_json::from_str(&text).ok();
    let ok = status.is_success() && json_bool(value.as_ref(), "ok").unwrap_or(true);
    let message = value
        .as_ref()
        .and_then(|value| {
            payload_string(value, "message")
                .or_else(|| payload_string(value, "status"))
                .or_else(|| payload_string(value, "summary"))
        })
        .unwrap_or_else(|| {
            if text.trim().is_empty() {
                format!("{context_label} returned HTTP {}", status.as_u16())
            } else {
                format!(
                    "{context_label} returned HTTP {}: {}",
                    status.as_u16(),
                    compact_output(&text)
                )
            }
        });
    let artifacts = value
        .as_ref()
        .map(collect_generic_software_artifacts)
        .unwrap_or_default();

    Ok(GenericSoftwareApiHttpResult {
        ok,
        message,
        artifacts,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HermesCommand {
    pub endpoint: String,
    pub project_slug: String,
    pub instruction: String,
    pub allowed_tools: Vec<String>,
    pub requires_confirmation: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCliCommand {
    pub id: String,
    pub title: String,
    pub command: String,
    pub tools: Vec<String>,
    pub token_budget: Option<u64>,
}

pub fn default_software_adapters() -> Vec<SoftwareAdapterConfig> {
    vec![
        adapter(
            "unreal",
            "Unreal",
            &["api/mcp", "skills/cli", "desktop-recognition"],
            1,
            true,
        ),
        adapter(
            "blender",
            "Blender",
            &["python-api", "skills/cli", "desktop-recognition"],
            2,
            true,
        ),
        adapter(
            "comfyui",
            "ComfyUI",
            &["http-api", "websocket", "skills/cli"],
            3,
            false,
        ),
        adapter(
            "resolve",
            "DaVinci Resolve",
            &["scripting-api", "desktop-recognition"],
            4,
            true,
        ),
        adapter(
            "unity",
            "Unity",
            &["editor-api", "skills/cli", "desktop-recognition"],
            5,
            true,
        ),
        adapter(
            "touchdesigner",
            "TouchDesigner",
            &["python-api", "osc", "desktop-recognition"],
            6,
            true,
        ),
        adapter(
            "madmapper",
            "MadMapper",
            &["osc", "desktop-recognition"],
            7,
            true,
        ),
        adapter(
            "nuke",
            "Nuke",
            &["python-api", "skills/cli", "desktop-recognition"],
            8,
            true,
        ),
        adapter(
            "motion-db",
            "Motion Capture Database",
            &["api", "skills/cli"],
            9,
            false,
        ),
        adapter(
            "editing-suite",
            "Editing Suite",
            &["skills/cli", "desktop-recognition"],
            10,
            true,
        ),
        adapter(
            "hermes",
            "Hermes",
            &["agent-api", "mcp", "skills/cli"],
            11,
            false,
        ),
    ]
}

fn adapter(
    id: &str,
    display_name: &str,
    control_modes: &[&str],
    priority: u8,
    desktop_fallback: bool,
) -> SoftwareAdapterConfig {
    SoftwareAdapterConfig {
        id: id.to_string(),
        display_name: display_name.to_string(),
        control_modes: control_modes.iter().map(|mode| mode.to_string()).collect(),
        priority,
        desktop_fallback,
    }
}

fn default_command_timeout_ms() -> u64 {
    30_000
}

fn default_command_max_output_bytes() -> usize {
    16 * 1024
}

fn run_async_http<F, T>(future: F) -> Result<T>
where
    F: Future<Output = Result<T>>,
{
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("create software adapter HTTP runtime")?
        .block_on(future)
}

fn payload_string(payload: &Value, key: &str) -> Option<String> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn payload_string_any(payload: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| payload_string(payload, key))
}

fn payload_string_array_any(payload: &Value, keys: &[&str]) -> Vec<String> {
    keys.iter()
        .find_map(|key| {
            payload.get(*key).and_then(Value::as_array).map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(ToString::to_string)
                    .collect()
            })
        })
        .unwrap_or_default()
}

fn payload_bool(payload: &Value, key: &str) -> Option<bool> {
    payload.get(key).and_then(Value::as_bool)
}

fn json_bool(payload: Option<&Value>, key: &str) -> Option<bool> {
    payload?.get(key).and_then(Value::as_bool)
}

fn ensure_leading_slash(path: &str) -> String {
    if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    }
}

fn collect_unreal_artifacts(value: &Value) -> Vec<String> {
    let mut artifacts = Vec::new();
    for key in ["artifact", "viewport_url", "level_url", "sequence_url"] {
        if let Some(artifact) = payload_string(value, key) {
            artifacts.push(artifact);
        }
    }
    if let Some(items) = value.get("artifacts").and_then(Value::as_array) {
        artifacts.extend(
            items
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string),
        );
    }
    artifacts
}

fn collect_hermes_artifacts(value: &Value) -> Vec<String> {
    let mut artifacts = Vec::new();
    for key in [
        "artifact",
        "session_url",
        "session_id",
        "transcript_path",
        "transcript_url",
        "control_path",
        "task_plan_url",
    ] {
        if let Some(artifact) = payload_string(value, key) {
            artifacts.push(artifact);
        }
    }
    if let Some(items) = value.get("artifacts").and_then(Value::as_array) {
        artifacts.extend(
            items
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string),
        );
    }
    artifacts
}

fn collect_generic_software_artifacts(value: &Value) -> Vec<String> {
    let mut artifacts = Vec::new();
    for key in [
        "artifact",
        "artifact_path",
        "output_path",
        "result_path",
        "report_path",
        "url",
    ] {
        if let Some(artifact) = payload_string(value, key) {
            artifacts.push(artifact);
        }
    }
    if let Some(items) = value.get("artifacts").and_then(Value::as_array) {
        artifacts.extend(
            items
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string),
        );
    }
    artifacts
}

fn parse_command_line(command: &str) -> std::result::Result<Vec<String>, String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut chars = command.chars().peekable();
    let mut quote: Option<char> = None;
    while let Some(ch) = chars.next() {
        match (quote, ch) {
            (None, '"') | (None, '\'') => quote = Some(ch),
            (Some(active), ch) if ch == active => quote = None,
            (None, ch) if ch.is_whitespace() => {
                if !current.is_empty() {
                    args.push(std::mem::take(&mut current));
                }
            }
            (_, '\\') => {
                if let Some(next) = chars.next() {
                    current.push(next);
                } else {
                    current.push('\\');
                }
            }
            (_, ch) => current.push(ch),
        }
    }
    if let Some(active) = quote {
        return Err(format!("unterminated quote in software command: {active}"));
    }
    if !current.is_empty() {
        args.push(current);
    }
    Ok(args)
}

fn command_allowed(binary: &str, allowed_commands: &[String]) -> bool {
    let name = command_name(binary);
    allowed_commands
        .iter()
        .any(|allowed| allowed == binary || allowed == &name)
}

fn command_name(binary: &str) -> String {
    Path::new(binary)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(binary)
        .to_string()
}

fn truncate_utf8(bytes: &[u8], max_bytes: usize) -> String {
    let text = String::from_utf8_lossy(bytes);
    if text.len() <= max_bytes {
        return text.to_string();
    }
    let mut end = max_bytes;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...[truncated]", &text[..end])
}

fn compact_output(text: &str) -> String {
    let text = text.trim().replace('\n', "\\n");
    if text.len() > 160 {
        let mut end = 160;
        while end > 0 && !text.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}...[truncated]", &text[..end])
    } else {
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_registry_contains_target_software_stack() {
        let registry = SoftwareAdapterRegistry::defaults();

        assert!(registry.get("unreal").is_some());
        assert!(registry.get("blender").is_some());
        assert!(registry.get("resolve").is_some());
        assert!(registry.get("touchdesigner").is_some());
        assert!(registry.get("hermes").is_some());
        assert_eq!(
            SoftwareAdapterRegistry::control_priority_chain(),
            vec![
                ControlPriority::ApiMcp,
                ControlPriority::SkillsCli,
                ControlPriority::DesktopRecognition,
                ControlPriority::HumanTakeover
            ]
        );
        assert!(registry
            .configs()
            .iter()
            .any(|config| config.id == "touchdesigner"));
    }

    #[test]
    fn mock_unreal_adapter_executes_control_action() {
        let adapter = MockUnrealAdapter::new();
        assert!(adapter.health().unwrap().ok);

        let result = adapter
            .execute(SoftwareControlAction {
                adapter_id: "unreal".to_string(),
                action_kind: SoftwareActionKind::CreateScene,
                priority: ControlPriority::ApiMcp,
                payload_json: serde_json::json!({ "level": "demo" }),
                requires_confirmation: false,
            })
            .unwrap();

        assert!(result.ok);
        assert_eq!(result.action_kind, SoftwareActionKind::CreateScene);
        assert_eq!(result.artifacts, vec!["unreal://mock/viewport"]);
    }

    #[test]
    fn unreal_mcp_adapter_reports_missing_endpoint() {
        let adapter = UnrealMcpAdapter::new(UnrealMcpAdapterOptions::default());
        let health = adapter.health().unwrap();

        assert!(!health.ok);
        assert!(health.message.contains("not configured"));
    }

    #[test]
    fn unreal_mcp_request_body_maps_create_scene_schema() {
        let action = SoftwareControlAction {
            adapter_id: "unreal".to_string(),
            action_kind: SoftwareActionKind::CreateScene,
            priority: ControlPriority::ApiMcp,
            payload_json: serde_json::json!({
                "level": "demo_level",
                "assets": ["worlds/demo/output/1-world.glb"],
                "camera": "hero_orbit"
            }),
            requires_confirmation: false,
        };

        let body = unreal_mcp_request_body(&action);

        assert_eq!(
            body["pool_unreal_action"]["profile_id"],
            "unreal-create-scene"
        );
        assert_eq!(
            body["pool_unreal_action"]["mcp_tool"],
            "unreal.create_scene"
        );
        assert_eq!(body["mcp_payload"]["tool"], "unreal.create_scene");
        assert_eq!(body["mcp_payload"]["arguments"]["level"], "demo_level");
        assert_eq!(
            body["mcp_payload"]["arguments"]["asset_paths"][0],
            "worlds/demo/output/1-world.glb"
        );
        assert_eq!(
            body["mcp_payload"]["arguments"]["lighting"],
            "cinematic_day"
        );
    }

    #[test]
    fn unreal_mcp_request_body_preserves_custom_mcp_payload() {
        let action = SoftwareControlAction {
            adapter_id: "unreal".to_string(),
            action_kind: SoftwareActionKind::Render,
            priority: ControlPriority::ApiMcp,
            payload_json: serde_json::json!({
                "sequence": "main",
                "mcp_payload": {
                    "tool": "custom.unreal.render",
                    "arguments": { "preset": "offline" }
                }
            }),
            requires_confirmation: false,
        };

        let body = unreal_mcp_request_body(&action);

        assert_eq!(body["pool_unreal_action"]["profile_id"], "unreal-render");
        assert_eq!(body["mcp_payload"]["tool"], "custom.unreal.render");
        assert_eq!(body["mcp_payload"]["arguments"]["preset"], "offline");
    }

    #[test]
    fn unreal_mcp_bridge_contract_exposes_plugin_tool_schema() {
        let contract = unreal_mcp_bridge_contract_resource();

        assert_eq!(contract["kind"], "pool_unreal_mcp_bridge_contract");
        assert_eq!(
            contract["pool_runtime_routes"]["action_submit"],
            "/api/software-actions"
        );
        assert_eq!(contract["transport"]["default_action"]["path"], "/mcp");
        assert!(contract["tool_contracts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|tool| tool["tool"] == "unreal.create_scene"
                && tool["required_input"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|field| field == "asset_paths")));
        assert_eq!(
            contract["response_contract"]["artifacts_field"],
            "artifacts"
        );
        assert_eq!(
            contract["local_worker"]["endpoint_env"],
            "POOL_UNREAL_MCP_ENDPOINT=http://127.0.0.1:8790"
        );
    }

    #[test]
    fn unreal_mcp_adapter_can_execute_against_bridge_worker() {
        let output_root = std::env::temp_dir().join(format!(
            "pool-unreal-adapter-bridge-{}",
            uuid::Uuid::new_v4().simple()
        ));
        let endpoint = spawn_unreal_mcp_bridge_worker(&output_root, 2).unwrap();
        let adapter = UnrealMcpAdapter::new(UnrealMcpAdapterOptions {
            endpoint,
            health_path: "/health".to_string(),
            action_path: "/mcp".to_string(),
            auth_token: None,
        });
        let health = adapter.health().unwrap();
        assert!(health.ok);

        let result = adapter
            .execute(SoftwareControlAction {
                adapter_id: "unreal".to_string(),
                action_kind: SoftwareActionKind::CreateScene,
                priority: ControlPriority::ApiMcp,
                payload_json: serde_json::json!({
                    "level": "demo_level",
                    "assets": ["worlds/demo/output/1-world.glb"]
                }),
                requires_confirmation: false,
            })
            .unwrap();

        assert!(result.ok);
        assert!(result.message.contains("unreal-bridge-dry-run"));
        assert!(result
            .artifacts
            .iter()
            .any(|artifact| artifact.starts_with("unreal://bridge/create_scene/")));
        assert!(
            std::fs::read_dir(output_root.join("control/unreal-mcp-bridge"))
                .unwrap()
                .count()
                >= 2
        );
    }

    #[test]
    fn hermes_mcp_request_body_maps_software_control_schema() {
        let action = SoftwareControlAction {
            adapter_id: "hermes".to_string(),
            action_kind: SoftwareActionKind::CreateScene,
            priority: ControlPriority::ApiMcp,
            payload_json: serde_json::json!({
                "project_slug": "demo",
                "instruction": "coordinate Unreal scene assembly",
                "allowed_tools": ["unreal", "filesystem"],
                "target_adapter": "unreal"
            }),
            requires_confirmation: false,
        };

        let body = hermes_mcp_request_body(&action);

        assert_eq!(
            body["pool_hermes_action"]["profile_id"],
            "hermes-coordinate-software"
        );
        assert_eq!(body["pool_hermes_action"]["mcp_tool"], "hermes.coordinate");
        assert_eq!(body["mcp_payload"]["tool"], "hermes.coordinate");
        assert_eq!(
            body["mcp_payload"]["arguments"]["instruction"],
            "coordinate Unreal scene assembly"
        );
        assert_eq!(
            body["mcp_payload"]["arguments"]["allowed_tools"][0],
            "unreal"
        );
    }

    #[test]
    fn hermes_mcp_request_body_preserves_custom_mcp_payload() {
        let action = SoftwareControlAction {
            adapter_id: "hermes".to_string(),
            action_kind: SoftwareActionKind::Render,
            priority: ControlPriority::ApiMcp,
            payload_json: serde_json::json!({
                "mcp_payload": {
                    "tool": "custom.hermes.review",
                    "arguments": { "mode": "dry-run" }
                }
            }),
            requires_confirmation: false,
        };

        let body = hermes_mcp_request_body(&action);

        assert_eq!(
            body["pool_hermes_action"]["profile_id"],
            "hermes-output-control"
        );
        assert_eq!(body["mcp_payload"]["tool"], "custom.hermes.review");
        assert_eq!(body["mcp_payload"]["arguments"]["mode"], "dry-run");
    }

    #[test]
    fn hermes_mcp_adapter_can_execute_against_bridge_worker() {
        let output_root = std::env::temp_dir().join(format!(
            "pool-hermes-adapter-bridge-{}",
            uuid::Uuid::new_v4().simple()
        ));
        let endpoint = spawn_hermes_mcp_bridge_worker(&output_root, 2).unwrap();
        let adapter = HermesMcpAdapter::new(HermesMcpAdapterOptions {
            endpoint,
            health_path: "/health".to_string(),
            action_path: "/mcp".to_string(),
            auth_token: None,
        });
        let health = adapter.health().unwrap();
        assert!(health.ok);

        let result = adapter
            .execute(SoftwareControlAction {
                adapter_id: "hermes".to_string(),
                action_kind: SoftwareActionKind::CreateScene,
                priority: ControlPriority::ApiMcp,
                payload_json: serde_json::json!({
                    "project_slug": "demo",
                    "instruction": "coordinate Unreal scene assembly",
                    "target_adapter": "unreal",
                    "target_action_kind": "CreateScene"
                }),
                requires_confirmation: false,
            })
            .unwrap();

        assert!(result.ok);
        assert!(result.message.contains("hermes-bridge-dry-run"));
        assert!(result
            .artifacts
            .iter()
            .any(|artifact| artifact.starts_with("hermes://bridge/coordinate_software_action/")));
        assert!(
            std::fs::read_dir(output_root.join("control/hermes-mcp-bridge"))
                .unwrap()
                .count()
                >= 2
        );
    }

    #[test]
    fn command_software_adapter_executes_allowed_command() {
        let config = adapter(
            "blender",
            "Blender",
            &["python-api", "skills/cli", "desktop-recognition"],
            2,
            true,
        );
        let adapter = CommandSoftwareAdapter::new(config);

        let result = adapter
            .execute(SoftwareControlAction {
                adapter_id: "blender".to_string(),
                action_kind: SoftwareActionKind::ExecuteCli,
                priority: ControlPriority::SkillsCli,
                payload_json: serde_json::json!({
                    "command": "/bin/echo blender-cli-ok",
                    "allowed_commands": ["/bin/echo", "echo"],
                    "timeout_ms": 2000,
                    "max_output_bytes": 1024
                }),
                requires_confirmation: false,
            })
            .unwrap();

        assert!(result.ok);
        assert!(result.message.contains("blender-cli-ok"));
        assert!(result
            .artifacts
            .iter()
            .any(|artifact| artifact.contains("software-command://blender/echo")));
    }

    #[test]
    fn command_software_adapter_denies_unlisted_command() {
        let config = adapter("nuke", "Nuke", &["python-api", "skills/cli"], 8, true);
        let adapter = CommandSoftwareAdapter::new(config);

        let result = adapter
            .execute(SoftwareControlAction {
                adapter_id: "nuke".to_string(),
                action_kind: SoftwareActionKind::ExecuteCli,
                priority: ControlPriority::SkillsCli,
                payload_json: serde_json::json!({
                    "command": "/bin/echo blocked",
                    "allowed_commands": ["nuke"],
                    "timeout_ms": 2000
                }),
                requires_confirmation: false,
            })
            .unwrap();

        assert!(!result.ok);
        assert!(result.message.contains("not in allowlist"));
    }

    #[test]
    fn generic_software_api_adapter_can_execute_against_bridge_worker() {
        let output_root = std::env::temp_dir().join(format!(
            "pool-software-api-adapter-bridge-{}",
            uuid::Uuid::new_v4().simple()
        ));
        let endpoint = spawn_software_api_bridge_worker("resolve", &output_root, 2).unwrap();
        let registry = SoftwareAdapterRegistry::defaults();
        let config = registry.get("resolve").unwrap().clone();
        let action = SoftwareControlAction {
            adapter_id: "resolve".to_string(),
            action_kind: SoftwareActionKind::CreateScene,
            priority: ControlPriority::ApiMcp,
            payload_json: serde_json::json!({
                "endpoint": endpoint,
                "project_slug": "demo",
                "artifacts": ["worlds/demo/output/production/resolve/1-edit.mov"]
            }),
            requires_confirmation: false,
        };
        let adapter = GenericSoftwareApiAdapter::from_action(config, &action);
        let health = adapter.health().unwrap();
        assert!(health.ok);

        let result = adapter.execute(action).unwrap();

        assert!(result.ok);
        assert!(result
            .message
            .contains("resolve-software-api-bridge-dry-run"));
        assert!(result
            .artifacts
            .iter()
            .any(|artifact| artifact.starts_with("software-api://resolve/create_scene/")));
        assert!(
            std::fs::read_dir(output_root.join("control/software-api-bridge/resolve"))
                .unwrap()
                .count()
                >= 2
        );
    }

    #[test]
    fn desktop_recognition_adapter_stages_control_request_file() {
        let control_dir =
            std::env::temp_dir().join(format!("pool-desktop-recognition-{}", uuid::Uuid::new_v4()));
        let config = adapter(
            "madmapper",
            "MadMapper",
            &["osc", "desktop-recognition"],
            7,
            true,
        );
        let adapter = DesktopRecognitionAdapter::new(config);

        let result = adapter
            .execute(SoftwareControlAction {
                adapter_id: "madmapper".to_string(),
                action_kind: SoftwareActionKind::RunViewport,
                priority: ControlPriority::DesktopRecognition,
                payload_json: serde_json::json!({
                    "control_dir": control_dir,
                    "instruction": "open cue list and trigger scene 1",
                    "target_window": "MadMapper",
                    "visual_targets": ["Cue 1", "Output"]
                }),
                requires_confirmation: false,
            })
            .unwrap();

        assert!(result.ok);
        let request_path = result
            .artifacts
            .iter()
            .find(|artifact| artifact.ends_with(".json"))
            .unwrap();
        assert!(Path::new(request_path).exists());
        let body = fs::read_to_string(request_path).unwrap();
        let body: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(body["status"], "queued_for_desktop_recognition");
        assert_eq!(
            body["pool_desktop_action"]["profile_id"],
            "desktop-run-preview"
        );
        assert_eq!(body["pool_desktop_action"]["target_window"], "MadMapper");
        assert_eq!(body["desktop_payload"]["tool"], "desktop.run_preview");
        assert_eq!(
            body["desktop_payload"]["arguments"]["visual_targets"][0],
            "Cue 1"
        );

        fs::remove_dir_all(control_dir).unwrap();
    }
}
