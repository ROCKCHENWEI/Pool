use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use crate::db::{AssetSnapshot, RuntimeRepository, RuntimeSnapshot};
use crate::models::{AssetRecord, RuntimeEvent, RuntimeEventLevel, RuntimeTask, TaskStatus};
use crate::openclaw::{
    runtime_graph_resource, runtime_handoff_resource, runtime_integration_readiness_resource,
    runtime_preflight_resource,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeHandoffPackageRequest {
    pub project_slug: String,
    pub node_id: Option<String>,
    pub output_dir: String,
    pub title: String,
    pub include_snapshot: bool,
}

impl RuntimeHandoffPackageRequest {
    pub fn new(project_slug: impl Into<String>, output_dir: impl Into<String>) -> Self {
        Self {
            project_slug: project_slug.into(),
            node_id: None,
            output_dir: output_dir.into(),
            title: "Pool runtime handoff package".to_string(),
            include_snapshot: false,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeHandoffPackageRunReport {
    pub task_id: String,
    pub status: TaskStatus,
    pub local_paths: Vec<String>,
    pub request_path: String,
    pub handoff_path: String,
    pub preflight_path: String,
    pub graph_path: String,
    pub worker_self_checks_path: String,
    pub worker_self_checks_preflight_path: String,
    pub integration_readiness_path: String,
    pub manifest_path: String,
    pub operator_checklist: Value,
    pub agent_entrypoint: Value,
    pub mcp_resources: Value,
    pub snapshot_path: Option<String>,
    pub assets: Vec<AssetRecord>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeHandoffPackageCatalog {
    pub kind: String,
    pub project_filter: Option<String>,
    pub generated_at: String,
    pub summary: RuntimeHandoffPackageCatalogSummary,
    pub packages: Vec<RuntimeHandoffPackageSummary>,
    pub policy: RuntimeHandoffPackagePolicy,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeHandoffPackageCatalogSummary {
    pub package_count: usize,
    pub indexed_files: usize,
    pub ready_packages: usize,
    pub local_file_failures: Vec<String>,
    pub latest_asset_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeHandoffPackageSummary {
    pub package_id: String,
    pub project_slug: String,
    pub handoff_dir: String,
    pub status: String,
    pub local_files: Vec<String>,
    pub local_file_failures: Vec<String>,
    pub request_path: Option<String>,
    pub handoff_path: Option<String>,
    pub preflight_path: Option<String>,
    pub graph_path: Option<String>,
    pub worker_self_checks_path: Option<String>,
    pub worker_self_checks_preflight_path: Option<String>,
    pub integration_readiness_path: Option<String>,
    pub manifest_path: Option<String>,
    pub snapshot_path: Option<String>,
    pub manifest_found: bool,
    pub operator_checklist: Value,
    pub agent_entrypoint: Value,
    pub mcp_resources: Value,
    pub source_node_ids: Vec<String>,
    pub latest_asset_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeHandoffPackagePolicy {
    pub local_files_authoritative: bool,
    pub provider_urls_are_provenance: bool,
    pub expected_files: Vec<String>,
}

pub struct RuntimeHandoffPackageRunner<'a> {
    repository: &'a RuntimeRepository,
}

impl<'a> RuntimeHandoffPackageRunner<'a> {
    pub fn new(repository: &'a RuntimeRepository) -> Self {
        Self { repository }
    }

    pub fn run(
        &self,
        request: RuntimeHandoffPackageRequest,
    ) -> Result<RuntimeHandoffPackageRunReport> {
        let snapshot = self.repository.snapshot(Some(&request.project_slug))?;
        let mut task = RuntimeTask::new(request.project_slug.clone(), request.title.clone());
        task.node_id = request.node_id.clone();
        task.provider_id = Some("runtime-handoff-package".to_string());
        task.cost_estimate_tokens = 200;
        task.status = TaskStatus::Running;
        self.repository.insert_task(&task)?;
        self.repository.insert_event(&RuntimeEvent::new(
            request.project_slug.clone(),
            RuntimeEventLevel::Info,
            format!("runtime handoff package started: {}", request.title),
        ))?;

        let handoff_dir = Path::new(&request.output_dir)
            .join("control")
            .join("handoff");
        fs::create_dir_all(&handoff_dir)
            .with_context(|| format!("create handoff dir {}", handoff_dir.display()))?;

        let package = write_handoff_package(&handoff_dir, &request, &snapshot)?;
        let assets = self.repository.index_local_outputs(
            &request.project_slug,
            request.node_id.as_deref(),
            Some("pool-runtime-handoff://package"),
            &package.local_paths,
        )?;
        self.repository
            .update_task_status(&task.id, TaskStatus::Succeeded)?;
        self.repository.insert_event(&RuntimeEvent::new(
            request.project_slug.clone(),
            RuntimeEventLevel::Ok,
            format!("runtime handoff package succeeded: {} files", assets.len()),
        ))?;

        Ok(RuntimeHandoffPackageRunReport {
            task_id: task.id,
            status: TaskStatus::Succeeded,
            local_paths: package.local_paths,
            request_path: package.request_path,
            handoff_path: package.handoff_path,
            preflight_path: package.preflight_path,
            graph_path: package.graph_path,
            worker_self_checks_path: package.worker_self_checks_path,
            worker_self_checks_preflight_path: package.worker_self_checks_preflight_path,
            integration_readiness_path: package.integration_readiness_path,
            manifest_path: package.manifest_path,
            operator_checklist: package.operator_checklist,
            agent_entrypoint: package.agent_entrypoint,
            mcp_resources: package.mcp_resources,
            snapshot_path: package.snapshot_path,
            assets,
        })
    }
}

pub fn runtime_handoff_package_catalog_resource(
    snapshot: &RuntimeSnapshot,
) -> RuntimeHandoffPackageCatalog {
    let mut grouped_assets: BTreeMap<String, Vec<&AssetSnapshot>> = BTreeMap::new();
    for asset in snapshot.assets.iter().filter(|asset| {
        asset.provider_url.as_deref() == Some("pool-runtime-handoff://package")
            || handoff_dir_from_path(&asset.local_path).is_some()
    }) {
        if let Some(handoff_dir) = handoff_dir_from_path(&asset.local_path) {
            grouped_assets.entry(handoff_dir).or_default().push(asset);
        }
    }

    let mut packages = grouped_assets
        .into_iter()
        .map(|(handoff_dir, assets)| runtime_handoff_package_summary(&handoff_dir, assets))
        .collect::<Vec<_>>();
    packages.sort_by(|left, right| right.latest_asset_at.cmp(&left.latest_asset_at));

    let local_file_failures = packages
        .iter()
        .flat_map(|package| package.local_file_failures.clone())
        .collect::<Vec<_>>();
    let ready_packages = packages
        .iter()
        .filter(|package| package.status == "ready")
        .count();
    let indexed_files = packages
        .iter()
        .map(|package| package.local_files.len())
        .sum();
    let latest_asset_at = packages
        .iter()
        .filter_map(|package| package.latest_asset_at.clone())
        .max();

    RuntimeHandoffPackageCatalog {
        kind: "pool_runtime_handoff_packages".to_string(),
        project_filter: snapshot.project_filter.clone(),
        generated_at: snapshot.generated_at.clone(),
        summary: RuntimeHandoffPackageCatalogSummary {
            package_count: packages.len(),
            indexed_files,
            ready_packages,
            local_file_failures,
            latest_asset_at,
        },
        packages,
        policy: RuntimeHandoffPackagePolicy {
            local_files_authoritative: true,
            provider_urls_are_provenance: true,
            expected_files: handoff_package_expected_files()
                .iter()
                .map(|file| file.to_string())
                .collect(),
        },
    }
}

#[derive(Debug, Clone)]
struct WrittenHandoffPackage {
    local_paths: Vec<String>,
    request_path: String,
    handoff_path: String,
    preflight_path: String,
    graph_path: String,
    worker_self_checks_path: String,
    worker_self_checks_preflight_path: String,
    integration_readiness_path: String,
    manifest_path: String,
    operator_checklist: Value,
    agent_entrypoint: Value,
    mcp_resources: Value,
    snapshot_path: Option<String>,
}

fn write_handoff_package(
    handoff_dir: &Path,
    request: &RuntimeHandoffPackageRequest,
    snapshot: &crate::db::RuntimeSnapshot,
) -> Result<WrittenHandoffPackage> {
    let created_at = Utc::now().to_rfc3339();
    let request_path = handoff_dir.join(".1-runtime-handoff-request.json");
    let handoff_path = handoff_dir.join("1-runtime-handoff.json");
    let preflight_path = handoff_dir.join("2-runtime-preflight.json");
    let graph_path = handoff_dir.join("3-runtime-graph.json");
    let worker_self_checks_path = handoff_dir.join("5-worker-self-checks.sh");
    let worker_self_checks_preflight_path = handoff_dir.join("6-worker-self-checks-preflight.json");
    let integration_readiness_path = handoff_dir.join("7-integration-readiness.json");
    let manifest_path = handoff_dir.join("8-runtime-handoff-package-manifest.json");
    let snapshot_path = request
        .include_snapshot
        .then(|| handoff_dir.join("4-runtime-snapshot.json"));

    write_json(
        &request_path,
        &json!({
            "kind": "runtime_handoff_package_request",
            "project_slug": request.project_slug,
            "node_id": request.node_id,
            "title": request.title,
            "include_snapshot": request.include_snapshot,
            "created_at": created_at,
            "resources": [
                "pool://runtime-handoff",
                "pool://runtime-preflight",
                "pool://runtime-graph",
                "pool://integration-readiness",
                "pool://provider-gateway-worker",
                "pool://software-contracts",
                "pool://desktop-recognition"
            ],
            "worker_self_checks": {
                "script": "5-worker-self-checks.sh",
                "preflight": "6-worker-self-checks-preflight.json",
                "mcp_tool": "pool_worker_self_checks"
            },
            "manifest": "8-runtime-handoff-package-manifest.json",
            "local_files_authoritative": true,
            "provider_urls_are_provenance": true
        }),
    )?;

    let handoff = with_package_metadata(
        runtime_handoff_resource(snapshot)?,
        "runtime_handoff",
        &request.project_slug,
        &created_at,
    );
    let preflight = with_package_metadata(
        runtime_preflight_resource(snapshot)?,
        "runtime_preflight",
        &request.project_slug,
        &created_at,
    );
    let graph = with_package_metadata(
        runtime_graph_resource(snapshot)?,
        "runtime_graph",
        &request.project_slug,
        &created_at,
    );
    let integration_readiness = with_package_metadata(
        runtime_integration_readiness_resource(snapshot),
        "runtime_integration_readiness",
        &request.project_slug,
        &created_at,
    );

    write_json(&handoff_path, &handoff)?;
    write_json(&preflight_path, &preflight)?;
    write_json(&graph_path, &graph)?;
    write_json(&integration_readiness_path, &integration_readiness)?;
    let worker_action = preflight
        .get("next_actions")
        .and_then(Value::as_array)
        .and_then(|actions| {
            actions
                .iter()
                .find(|action| action.get("kind").and_then(Value::as_str) == Some("local_worker_self_check"))
        })
        .cloned()
        .unwrap_or_else(|| {
            json!({
                "kind": "local_worker_self_check",
                "title": "Run local Provider/Hermes/software worker self-checks",
                "command": "pool-cli worker-self-checks --output-root target/pool-worker-self-checks --software-adapter resolve",
                "mcp_tool": "pool_worker_self_checks",
                "mcp_arguments": {
                    "output_root": "target/pool-worker-self-checks",
                    "software_adapter": "resolve"
                },
                "optional": true
            })
        });
    write_worker_self_checks_preflight(
        &worker_self_checks_preflight_path,
        &request.project_slug,
        &created_at,
        &worker_action,
    )?;
    write_worker_self_checks_script(&worker_self_checks_path)?;
    if let Some(snapshot_path) = &snapshot_path {
        write_json(snapshot_path, &serde_json::to_value(snapshot)?)?;
    }

    let request_path = path_string_ref(&request_path);
    let handoff_path = path_string_ref(&handoff_path);
    let preflight_path = path_string_ref(&preflight_path);
    let graph_path = path_string_ref(&graph_path);
    let worker_self_checks_path = path_string_ref(&worker_self_checks_path);
    let worker_self_checks_preflight_path = path_string_ref(&worker_self_checks_preflight_path);
    let integration_readiness_path = path_string_ref(&integration_readiness_path);
    let manifest_path_string = path_string_ref(&manifest_path);
    let snapshot_path = snapshot_path.as_ref().map(|path| path_string_ref(path));

    let manifest = runtime_handoff_package_manifest(
        request,
        &created_at,
        &request_path,
        &handoff_path,
        &preflight_path,
        &graph_path,
        &worker_self_checks_path,
        &worker_self_checks_preflight_path,
        &integration_readiness_path,
        &manifest_path_string,
        snapshot_path.as_deref(),
    );
    let operator_checklist = manifest["operator_checklist"].clone();
    let agent_entrypoint = manifest["agent_entrypoint"].clone();
    let mcp_resources = manifest["mcp_resources"].clone();
    write_json(&manifest_path, &manifest)?;

    let mut local_paths = vec![
        request_path.clone(),
        handoff_path.clone(),
        preflight_path.clone(),
        graph_path.clone(),
        worker_self_checks_path.clone(),
        worker_self_checks_preflight_path.clone(),
        integration_readiness_path.clone(),
        manifest_path_string.clone(),
    ];
    if let Some(path) = &snapshot_path {
        local_paths.push(path.clone());
    }

    Ok(WrittenHandoffPackage {
        local_paths,
        request_path,
        handoff_path,
        preflight_path,
        graph_path,
        worker_self_checks_path,
        worker_self_checks_preflight_path,
        integration_readiness_path,
        manifest_path: manifest_path_string,
        operator_checklist,
        agent_entrypoint,
        mcp_resources,
        snapshot_path,
    })
}

fn runtime_handoff_package_manifest(
    request: &RuntimeHandoffPackageRequest,
    created_at: &str,
    request_path: &str,
    handoff_path: &str,
    preflight_path: &str,
    graph_path: &str,
    worker_self_checks_path: &str,
    worker_self_checks_preflight_path: &str,
    integration_readiness_path: &str,
    manifest_path: &str,
    snapshot_path: Option<&str>,
) -> Value {
    let runtime_preflight_command = format!(
        "pool-cli --project {} runtime-preflight",
        request.project_slug
    );
    let runtime_handoff_command = format!(
        "pool-cli --project {} runtime-handoff",
        request.project_slug
    );
    let integration_readiness_command = format!(
        "pool-cli --project {} integration-readiness",
        request.project_slug
    );
    let serve_mcp_command = format!("pool-cli --project {} serve-mcp", request.project_slug);
    let mut read_order = vec![
        json!({
            "step": 1,
            "role": "operator_entry",
            "path": handoff_path,
            "purpose": "Read the Agent/Hermes/human runbook and team lanes first."
        }),
        json!({
            "step": 2,
            "role": "preflight_gate",
            "path": preflight_path,
            "purpose": "Inspect blocking approvals, credentials, failed tasks, desktop handoffs, and local worker smoke action."
        }),
        json!({
            "step": 3,
            "role": "runtime_graph",
            "path": graph_path,
            "purpose": "Inspect node topology, task types, asset flow, control flow, approvals, and feedback loops."
        }),
        json!({
            "step": 4,
            "role": "integration_readiness",
            "path": integration_readiness_path,
            "purpose": "Inspect Provider, software adapter, and Agent/Hermes readiness lanes and next-action run plan."
        }),
        json!({
            "step": 5,
            "role": "local_worker_smoke",
            "path": worker_self_checks_preflight_path,
            "script": worker_self_checks_path,
            "purpose": "Run the local bridge self-check before assigning tasks to external Provider/software workers."
        }),
    ];
    if let Some(snapshot_path) = snapshot_path {
        read_order.push(json!({
            "step": 6,
            "role": "offline_snapshot",
            "path": snapshot_path,
            "purpose": "Use the bundled snapshot only when the live runtime is unavailable."
        }));
    }
    let operator_checklist = vec![
        json!({
            "step": 1,
            "owner": "agent_operator",
            "action": "Open the offline handoff manifest, then read the runtime handoff runbook.",
            "path": handoff_path,
            "verify": "Team lanes, commands, approval gates, desktop handoffs, and runnable nodes are visible before any external action."
        }),
        json!({
            "step": 2,
            "owner": "creative_director",
            "action": "Resolve blocking approval gates and high-cost generation decisions.",
            "path": preflight_path,
            "command": runtime_preflight_command,
            "verify": "No high-cost 3DGS or software-control task is executed before the required approval is explicit."
        }),
        json!({
            "step": 3,
            "owner": "agent_operator",
            "action": "Run local worker bridge self-checks before assigning work to Provider, Hermes, Unreal, or software bridge workers.",
            "path": worker_self_checks_preflight_path,
            "command": "./5-worker-self-checks.sh",
            "verify": "The worker self-check report exists and failed bridge checks are assigned before production execution."
        }),
        json!({
            "step": 4,
            "owner": "ai_3dgs_td",
            "action": "Use integration readiness run plan to configure or execute AI media, 3DGS, and Agent/Hermes adapters.",
            "path": integration_readiness_path,
            "command": integration_readiness_command,
            "verify": "Provider, software, and Agent/Hermes rows move from needs_configuration/needs_execution toward ready."
        }),
        json!({
            "step": 5,
            "owner": "engine_integrator",
            "action": "Use the runtime graph to confirm node topology, asset flow, control flow, and Unreal/software dispatch boundaries.",
            "path": graph_path,
            "verify": "Each runnable or blocked node has a clear upstream asset/control dependency and a responsible lane."
        }),
        json!({
            "step": 6,
            "owner": "agent_operator",
            "action": "Expose the same context through MCP when Hermes or another Agent CLI controls the handoff.",
            "command": serve_mcp_command,
            "verify": "MCP resources and tools are available before Agent automation mutates runtime state."
        }),
    ];

    json!({
        "kind": "pool_runtime_handoff_package_manifest",
        "project_slug": request.project_slug,
        "node_id": request.node_id,
        "title": request.title,
        "created_at": created_at,
        "paths": {
            "request": request_path,
            "handoff": handoff_path,
            "preflight": preflight_path,
            "runtime_graph": graph_path,
            "runtime_snapshot": snapshot_path,
            "worker_self_checks": worker_self_checks_path,
            "worker_self_checks_preflight": worker_self_checks_preflight_path,
            "integration_readiness": integration_readiness_path,
            "manifest": manifest_path
        },
        "read_order": read_order,
        "operator_checklist": operator_checklist,
        "commands": {
            "worker_self_checks": "./5-worker-self-checks.sh",
            "runtime_preflight": runtime_preflight_command,
            "runtime_handoff": runtime_handoff_command,
            "integration_readiness": integration_readiness_command,
            "serve_mcp": serve_mcp_command
        },
        "agent_entrypoint": {
            "first_file": manifest_path,
            "primary_runbook": handoff_path,
            "readiness": integration_readiness_path,
            "worker_smoke": worker_self_checks_path,
            "mcp_stdio": format!("pool-cli --project {} serve-mcp", request.project_slug)
        },
        "mcp_tools": {
            "worker_self_checks": "pool_worker_self_checks",
            "handoff_package": "pool_handoff_package",
            "integration_readiness": "pool_integration_readiness"
        },
        "mcp_resources": [
            "pool://runtime-handoff",
            "pool://runtime-preflight",
            "pool://runtime-graph",
            "pool://integration-readiness",
            "pool://workflow",
            "pool://tasks",
            "pool://assets"
        ],
        "human_takeover": {
            "first_file": manifest_path,
            "approval_source": preflight_path,
            "readiness_source": integration_readiness_path,
            "worker_smoke_script": worker_self_checks_path
        },
        "policies": {
            "local_files_authoritative": true,
            "provider_urls_are_provenance": true,
            "high_cost_requires_approval": true,
            "control_priority": "API/MCP > Skills/CLI > Desktop Recognition > Human Takeover"
        }
    })
}

fn write_worker_self_checks_preflight(
    path: &Path,
    project_slug: &str,
    created_at: &str,
    worker_action: &Value,
) -> Result<()> {
    write_json(
        path,
        &json!({
            "kind": "pool_runtime_handoff_worker_self_checks_preflight",
            "project_slug": project_slug,
            "created_at": created_at,
            "action": worker_action,
            "script": "5-worker-self-checks.sh",
            "mcp_tool": "pool_worker_self_checks",
            "purpose": "Run before handing runtime tasks to external AI media/3DGS workers, Hermes MCP, Unreal MCP, or generic software API bridge workers.",
            "outputs": {
                "default_output_root": "target/pool-worker-self-checks",
                "report_kind": "pool_worker_self_checks"
            },
            "local_files_authoritative": true,
            "production_note": "This is a local bridge smoke gate. Production evidence still requires real upstream Provider/software/desktop controller attestations."
        }),
    )
}

fn write_worker_self_checks_script(path: &Path) -> Result<()> {
    let script = r#"#!/usr/bin/env bash
set -euo pipefail

OUTPUT_ROOT="${POOL_WORKER_SELF_CHECKS_OUTPUT_ROOT:-target/pool-worker-self-checks}"
SOFTWARE_ADAPTER="${POOL_WORKER_SELF_CHECKS_SOFTWARE_ADAPTER:-resolve}"

if command -v pool-cli >/dev/null 2>&1; then
  exec pool-cli worker-self-checks --output-root "$OUTPUT_ROOT" --software-adapter "$SOFTWARE_ADAPTER"
fi

exec cargo run -q -p pool-cli -- worker-self-checks --output-root "$OUTPUT_ROOT" --software-adapter "$SOFTWARE_ADAPTER"
"#;
    fs::write(path, script)
        .with_context(|| format!("write handoff worker self-checks script {}", path.display()))?;
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
    Ok(())
}

fn with_package_metadata(
    mut value: Value,
    kind: &str,
    project_slug: &str,
    created_at: &str,
) -> Value {
    if let Some(object) = value.as_object_mut() {
        object.insert("package_kind".to_string(), json!(kind));
        object.insert("package_project_slug".to_string(), json!(project_slug));
        object.insert("package_created_at".to_string(), json!(created_at));
    }
    value
}

fn runtime_handoff_package_summary(
    handoff_dir: &str,
    mut assets: Vec<&AssetSnapshot>,
) -> RuntimeHandoffPackageSummary {
    assets.sort_by(|left, right| right.created_at.cmp(&left.created_at));
    let mut seen_paths = BTreeSet::new();
    let mut unique_assets = Vec::new();
    for asset in assets {
        if seen_paths.insert(asset.local_path.clone()) {
            unique_assets.push(asset);
        }
    }
    unique_assets.sort_by(|left, right| {
        handoff_package_file_rank(&left.local_path)
            .cmp(&handoff_package_file_rank(&right.local_path))
            .then_with(|| left.local_path.cmp(&right.local_path))
    });

    let local_files = unique_assets
        .iter()
        .map(|asset| asset.local_path.clone())
        .collect::<Vec<_>>();
    let local_file_failures = local_files
        .iter()
        .filter(|path| !Path::new(path).is_file())
        .cloned()
        .collect::<Vec<_>>();
    let request_path = handoff_path_with_file(&local_files, ".1-runtime-handoff-request.json");
    let handoff_path = handoff_path_with_file(&local_files, "1-runtime-handoff.json");
    let preflight_path = handoff_path_with_file(&local_files, "2-runtime-preflight.json");
    let graph_path = handoff_path_with_file(&local_files, "3-runtime-graph.json");
    let snapshot_path = handoff_path_with_file(&local_files, "4-runtime-snapshot.json");
    let worker_self_checks_path = handoff_path_with_file(&local_files, "5-worker-self-checks.sh");
    let worker_self_checks_preflight_path =
        handoff_path_with_file(&local_files, "6-worker-self-checks-preflight.json");
    let integration_readiness_path =
        handoff_path_with_file(&local_files, "7-integration-readiness.json");
    let manifest_path =
        handoff_path_with_file(&local_files, "8-runtime-handoff-package-manifest.json");
    let manifest = manifest_path.as_deref().and_then(read_json_file);
    let manifest_found = manifest.is_some();
    let operator_checklist = manifest
        .as_ref()
        .map(|manifest| manifest["operator_checklist"].clone())
        .unwrap_or_else(|| json!([]));
    let agent_entrypoint = manifest
        .as_ref()
        .map(|manifest| manifest["agent_entrypoint"].clone())
        .unwrap_or_else(|| json!({}));
    let mcp_resources = manifest
        .as_ref()
        .map(|manifest| manifest["mcp_resources"].clone())
        .unwrap_or_else(|| json!([]));
    let source_node_ids = unique_assets
        .iter()
        .filter_map(|asset| asset.source_node_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let latest_asset_at = unique_assets
        .iter()
        .map(|asset| asset.created_at.clone())
        .max();
    let status = if !local_file_failures.is_empty() {
        "indexed_missing_file"
    } else if manifest_found {
        "ready"
    } else {
        "indexed"
    };
    let project_slug = unique_assets
        .first()
        .map(|asset| asset.project_slug.clone())
        .unwrap_or_default();

    RuntimeHandoffPackageSummary {
        package_id: format!(
            "runtime-handoff-package:{}",
            manifest_path.as_deref().unwrap_or(handoff_dir)
        ),
        project_slug,
        handoff_dir: handoff_dir.to_string(),
        status: status.to_string(),
        local_files,
        local_file_failures,
        request_path,
        handoff_path,
        preflight_path,
        graph_path,
        worker_self_checks_path,
        worker_self_checks_preflight_path,
        integration_readiness_path,
        manifest_path,
        snapshot_path,
        manifest_found,
        operator_checklist,
        agent_entrypoint,
        mcp_resources,
        source_node_ids,
        latest_asset_at,
    }
}

fn handoff_dir_from_path(path: &str) -> Option<String> {
    let parent = Path::new(path).parent()?;
    let normalized = parent.to_string_lossy().replace('\\', "/");
    if normalized == "control/handoff" || normalized.ends_with("/control/handoff") {
        Some(normalized)
    } else {
        None
    }
}

fn handoff_path_with_file(paths: &[String], file_name: &str) -> Option<String> {
    paths
        .iter()
        .find(|path| Path::new(path).file_name().and_then(|name| name.to_str()) == Some(file_name))
        .cloned()
}

fn handoff_package_file_rank(path: &str) -> usize {
    let file_name = Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    handoff_package_expected_files()
        .iter()
        .position(|expected| *expected == file_name)
        .unwrap_or(usize::MAX)
}

fn handoff_package_expected_files() -> &'static [&'static str] {
    &[
        ".1-runtime-handoff-request.json",
        "1-runtime-handoff.json",
        "2-runtime-preflight.json",
        "3-runtime-graph.json",
        "4-runtime-snapshot.json",
        "5-worker-self-checks.sh",
        "6-worker-self-checks-preflight.json",
        "7-integration-readiness.json",
        "8-runtime-handoff-package-manifest.json",
    ]
}

fn read_json_file(path: &str) -> Option<Value> {
    fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
}

fn write_json(path: &Path, value: &Value) -> Result<()> {
    fs::write(
        path,
        serde_json::to_string_pretty(value).context("serialize handoff package file")?,
    )
    .with_context(|| format!("write handoff package file {}", path.display()))
}

fn path_string_ref(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::build_default_content_burst_plan;
    use uuid::Uuid;

    #[test]
    fn writes_runtime_handoff_package_files_and_assets() {
        let repository = RuntimeRepository::in_memory().unwrap();
        repository.migrate().unwrap();
        let plan = build_default_content_burst_plan("demo", "Pool demo");
        repository.persist_plan(&plan).unwrap();
        let output_dir =
            std::env::temp_dir().join(format!("pool-handoff-package-{}", Uuid::new_v4()));

        let report = RuntimeHandoffPackageRunner::new(&repository)
            .run(RuntimeHandoffPackageRequest {
                project_slug: "demo".to_string(),
                node_id: Some("agent".to_string()),
                output_dir: output_dir.to_string_lossy().to_string(),
                title: "Runtime handoff package".to_string(),
                include_snapshot: true,
            })
            .unwrap();

        assert_eq!(report.status, TaskStatus::Succeeded);
        assert_eq!(report.local_paths.len(), 9);
        assert_eq!(report.assets.len(), 9);
        assert!(Path::new(&report.request_path).exists());
        assert!(Path::new(&report.handoff_path).exists());
        assert!(Path::new(&report.preflight_path).exists());
        assert!(Path::new(&report.graph_path).exists());
        assert!(Path::new(&report.worker_self_checks_path).exists());
        assert!(Path::new(&report.worker_self_checks_preflight_path).exists());
        assert!(Path::new(&report.integration_readiness_path).exists());
        assert!(Path::new(&report.manifest_path).exists());
        assert!(Path::new(report.snapshot_path.as_ref().unwrap()).exists());
        assert!(fs::read_to_string(&report.worker_self_checks_path)
            .unwrap()
            .contains("worker-self-checks"));
        let worker_preflight: Value = serde_json::from_str(
            &fs::read_to_string(&report.worker_self_checks_preflight_path).unwrap(),
        )
        .unwrap();
        assert_eq!(
            worker_preflight["kind"],
            "pool_runtime_handoff_worker_self_checks_preflight"
        );
        assert_eq!(
            worker_preflight["action"]["mcp_tool"],
            "pool_worker_self_checks"
        );
        let integration_readiness: Value =
            serde_json::from_str(&fs::read_to_string(&report.integration_readiness_path).unwrap())
                .unwrap();
        assert_eq!(
            integration_readiness["package_kind"],
            "runtime_integration_readiness"
        );
        assert_eq!(integration_readiness["kind"], "pool_integration_readiness");
        assert!(
            integration_readiness["summary"]["total"]
                .as_u64()
                .unwrap_or_default()
                > 0
        );
        let manifest: Value =
            serde_json::from_str(&fs::read_to_string(&report.manifest_path).unwrap()).unwrap();
        assert_eq!(manifest["kind"], "pool_runtime_handoff_package_manifest");
        assert_eq!(
            manifest["paths"]["integration_readiness"],
            report.integration_readiness_path
        );
        assert!(manifest["read_order"]
            .as_array()
            .unwrap()
            .iter()
            .any(|step| step["role"] == "integration_readiness"));
        assert_eq!(
            manifest["commands"]["integration_readiness"],
            "pool-cli --project demo integration-readiness"
        );
        assert_eq!(
            manifest["agent_entrypoint"]["first_file"],
            report.manifest_path
        );
        assert_eq!(report.agent_entrypoint["first_file"], report.manifest_path);
        assert!(manifest["operator_checklist"]
            .as_array()
            .unwrap()
            .iter()
            .any(|step| step["owner"] == "creative_director"
                && step["command"] == "pool-cli --project demo runtime-preflight"));
        assert!(report
            .operator_checklist
            .as_array()
            .unwrap()
            .iter()
            .any(|step| step["owner"] == "ai_3dgs_td"
                && step["command"] == "pool-cli --project demo integration-readiness"));
        assert!(manifest["mcp_resources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|resource| resource.as_str() == Some("pool://integration-readiness")));
        assert!(report
            .mcp_resources
            .as_array()
            .unwrap()
            .iter()
            .any(|resource| resource.as_str() == Some("pool://runtime-handoff")));
        let snapshot = repository.snapshot(Some("demo")).unwrap();
        let catalog = runtime_handoff_package_catalog_resource(&snapshot);
        assert_eq!(catalog.summary.package_count, 1);
        assert_eq!(catalog.summary.ready_packages, 1);
        assert_eq!(catalog.summary.indexed_files, 9);
        let package = &catalog.packages[0];
        assert_eq!(package.status, "ready");
        assert_eq!(
            package.manifest_path.as_deref(),
            Some(report.manifest_path.as_str())
        );
        assert_eq!(package.agent_entrypoint["first_file"], report.manifest_path);
        assert!(package
            .operator_checklist
            .as_array()
            .unwrap()
            .iter()
            .any(|step| step["owner"] == "ai_3dgs_td"));
        assert!(package
            .mcp_resources
            .as_array()
            .unwrap()
            .iter()
            .any(|resource| resource.as_str() == Some("pool://integration-readiness")));
        let handoff: Value =
            serde_json::from_str(&fs::read_to_string(&report.handoff_path).unwrap()).unwrap();
        assert_eq!(handoff["package_kind"], "runtime_handoff");
        assert!(handoff["commands"]
            .as_array()
            .unwrap()
            .iter()
            .any(|command| command["command"]
                .as_str()
                .unwrap_or_default()
                .contains("approve-task")));
        let task = repository.task_snapshot(&report.task_id).unwrap();
        assert_eq!(task.provider_id.as_deref(), Some("runtime-handoff-package"));
        assert_eq!(repository.snapshot(Some("demo")).unwrap().stats.assets, 9);

        fs::remove_dir_all(output_dir).unwrap();
    }
}
