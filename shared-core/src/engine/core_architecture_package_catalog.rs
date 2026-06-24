use serde::Serialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use crate::db::{AssetSnapshot, RuntimeSnapshot};

#[derive(Debug, Clone, Serialize)]
pub struct CoreArchitecturePackageCatalog {
    pub kind: String,
    pub project_filter: Option<String>,
    pub generated_at: String,
    pub summary: CoreArchitecturePackageCatalogSummary,
    pub packages: Vec<CoreArchitecturePackageSummary>,
    pub policy: CoreArchitecturePackagePolicy,
}

#[derive(Debug, Clone, Serialize)]
pub struct CoreArchitecturePackageCatalogSummary {
    pub package_count: usize,
    pub indexed_files: usize,
    pub ready_packages: usize,
    pub architecture_ready_packages: usize,
    pub local_file_failures: Vec<String>,
    pub latest_asset_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CoreArchitecturePackageSummary {
    pub package_id: String,
    pub project_slug: String,
    pub package_dir: String,
    pub status: String,
    pub ready_for_core_architecture: bool,
    pub architecture_status: Option<String>,
    pub local_files: Vec<String>,
    pub local_file_failures: Vec<String>,
    pub request_path: Option<String>,
    pub readiness_path: Option<String>,
    pub core_architecture_gate_path: Option<String>,
    pub runtime_graph_path: Option<String>,
    pub runtime_execution_plan_path: Option<String>,
    pub runtime_handoff_path: Option<String>,
    pub output_packages_path: Option<String>,
    pub strict_prd_completion_gate_path: Option<String>,
    pub manifest_path: Option<String>,
    pub snapshot_path: Option<String>,
    pub manifest_found: bool,
    pub summary: Value,
    pub commands: Value,
    pub operator_checklist: Value,
    pub mcp_resources: Value,
    pub source_node_ids: Vec<String>,
    pub latest_asset_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CoreArchitecturePackagePolicy {
    pub local_files_authoritative: bool,
    pub provider_urls_are_provenance: bool,
    pub expected_files: Vec<String>,
    pub production_evidence_is_out_of_scope: bool,
}

pub fn core_architecture_package_catalog_resource(
    snapshot: &RuntimeSnapshot,
) -> CoreArchitecturePackageCatalog {
    let mut grouped_assets: BTreeMap<String, Vec<&AssetSnapshot>> = BTreeMap::new();
    for asset in snapshot.assets.iter().filter(|asset| {
        asset.provider_url.as_deref() == Some("pool-core://architecture-package")
            || core_architecture_dir_from_path(&asset.local_path).is_some()
    }) {
        if let Some(package_dir) = core_architecture_dir_from_path(&asset.local_path) {
            grouped_assets.entry(package_dir).or_default().push(asset);
        }
    }

    let mut packages = grouped_assets
        .into_iter()
        .map(|(package_dir, assets)| core_architecture_package_summary(&package_dir, assets))
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
    let architecture_ready_packages = packages
        .iter()
        .filter(|package| package.ready_for_core_architecture)
        .count();
    let indexed_files = packages
        .iter()
        .map(|package| package.local_files.len())
        .sum();
    let latest_asset_at = packages
        .iter()
        .filter_map(|package| package.latest_asset_at.clone())
        .max();

    CoreArchitecturePackageCatalog {
        kind: "pool_core_architecture_packages".to_string(),
        project_filter: snapshot.project_filter.clone(),
        generated_at: snapshot.generated_at.clone(),
        summary: CoreArchitecturePackageCatalogSummary {
            package_count: packages.len(),
            indexed_files,
            ready_packages,
            architecture_ready_packages,
            local_file_failures,
            latest_asset_at,
        },
        packages,
        policy: CoreArchitecturePackagePolicy {
            local_files_authoritative: true,
            provider_urls_are_provenance: true,
            expected_files: core_architecture_package_expected_files()
                .iter()
                .map(|file| file.to_string())
                .collect(),
            production_evidence_is_out_of_scope: true,
        },
    }
}

fn core_architecture_package_summary(
    package_dir: &str,
    mut assets: Vec<&AssetSnapshot>,
) -> CoreArchitecturePackageSummary {
    assets.sort_by(|left, right| right.created_at.cmp(&left.created_at));
    let mut seen_paths = BTreeSet::new();
    let mut unique_assets = Vec::new();
    for asset in assets {
        if seen_paths.insert(asset.local_path.clone()) {
            unique_assets.push(asset);
        }
    }
    unique_assets.sort_by(|left, right| {
        core_architecture_package_file_rank(&left.local_path)
            .cmp(&core_architecture_package_file_rank(&right.local_path))
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
    let request_path =
        core_architecture_path_with_file(&local_files, ".1-core-architecture-package-request.json");
    let readiness_path =
        core_architecture_path_with_file(&local_files, "1-core-architecture-readiness.json");
    let core_architecture_gate_path =
        core_architecture_path_with_file(&local_files, "2-core-architecture-gate.json");
    let runtime_graph_path = core_architecture_path_with_file(&local_files, "3-runtime-graph.json");
    let runtime_execution_plan_path =
        core_architecture_path_with_file(&local_files, "4-runtime-execution-plan.json");
    let runtime_handoff_path =
        core_architecture_path_with_file(&local_files, "5-runtime-handoff.json");
    let output_packages_path =
        core_architecture_path_with_file(&local_files, "6-output-packages.json");
    let strict_prd_completion_gate_path =
        core_architecture_path_with_file(&local_files, "7-strict-prd-completion-gate.json");
    let manifest_path =
        core_architecture_path_with_file(&local_files, "8-core-architecture-package-manifest.json");
    let snapshot_path = core_architecture_path_with_file(&local_files, "9-runtime-snapshot.json");
    let manifest = manifest_path.as_deref().and_then(read_json_file);
    let manifest_found = manifest.is_some();
    let ready_for_core_architecture = manifest
        .as_ref()
        .and_then(|manifest| manifest["ready_for_core_architecture"].as_bool())
        .unwrap_or(false);
    let architecture_status = manifest
        .as_ref()
        .and_then(|manifest| manifest["architecture_status"].as_str())
        .map(|status| status.to_string());
    let summary = manifest
        .as_ref()
        .map(|manifest| manifest["summary"].clone())
        .unwrap_or_else(|| json!({}));
    let commands = manifest
        .as_ref()
        .map(|manifest| manifest["commands"].clone())
        .unwrap_or_else(|| json!({}));
    let operator_checklist = manifest
        .as_ref()
        .map(|manifest| manifest["operator_checklist"].clone())
        .unwrap_or_else(|| json!([]));
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

    CoreArchitecturePackageSummary {
        package_id: format!(
            "core-architecture-package:{}",
            manifest_path.as_deref().unwrap_or(package_dir)
        ),
        project_slug,
        package_dir: package_dir.to_string(),
        status: status.to_string(),
        ready_for_core_architecture,
        architecture_status,
        local_files,
        local_file_failures,
        request_path,
        readiness_path,
        core_architecture_gate_path,
        runtime_graph_path,
        runtime_execution_plan_path,
        runtime_handoff_path,
        output_packages_path,
        strict_prd_completion_gate_path,
        manifest_path,
        snapshot_path,
        manifest_found,
        summary,
        commands,
        operator_checklist,
        mcp_resources,
        source_node_ids,
        latest_asset_at,
    }
}

fn core_architecture_dir_from_path(path: &str) -> Option<String> {
    let parent = Path::new(path).parent()?;
    let normalized = parent.to_string_lossy().replace('\\', "/");
    if normalized == "control/core-architecture"
        || normalized.ends_with("/control/core-architecture")
    {
        Some(normalized)
    } else {
        None
    }
}

fn core_architecture_path_with_file(paths: &[String], file_name: &str) -> Option<String> {
    paths
        .iter()
        .find(|path| Path::new(path).file_name().and_then(|name| name.to_str()) == Some(file_name))
        .cloned()
}

fn core_architecture_package_file_rank(path: &str) -> usize {
    let file_name = Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    core_architecture_package_expected_files()
        .iter()
        .position(|expected| *expected == file_name)
        .unwrap_or(usize::MAX)
}

fn core_architecture_package_expected_files() -> &'static [&'static str] {
    &[
        ".1-core-architecture-package-request.json",
        "1-core-architecture-readiness.json",
        "2-core-architecture-gate.json",
        "3-runtime-graph.json",
        "4-runtime-execution-plan.json",
        "5-runtime-handoff.json",
        "6-output-packages.json",
        "7-strict-prd-completion-gate.json",
        "8-core-architecture-package-manifest.json",
        "9-runtime-snapshot.json",
    ]
}

fn read_json_file(path: &str) -> Option<Value> {
    fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
}
