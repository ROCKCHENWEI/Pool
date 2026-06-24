use serde::Serialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use crate::db::{AssetSnapshot, RuntimeSnapshot};

#[derive(Debug, Clone, Serialize)]
pub struct ProductionEvidenceHandoffPackageCatalog {
    pub kind: String,
    pub project_filter: Option<String>,
    pub generated_at: String,
    pub summary: ProductionEvidenceHandoffPackageCatalogSummary,
    pub packages: Vec<ProductionEvidenceHandoffPackageSummary>,
    pub policy: ProductionEvidenceHandoffPackagePolicy,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProductionEvidenceHandoffPackageCatalogSummary {
    pub package_count: usize,
    pub indexed_files: usize,
    pub ready_packages: usize,
    pub item_files: usize,
    pub runner_packages: usize,
    pub local_file_failures: Vec<String>,
    pub latest_asset_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProductionEvidenceHandoffPackageSummary {
    pub package_id: String,
    pub project_slug: String,
    pub package_dir: String,
    pub status: String,
    pub local_files: Vec<String>,
    pub local_file_failures: Vec<String>,
    pub request_path: Option<String>,
    pub requirements_path: Option<String>,
    pub tasks_path: Option<String>,
    pub handoff_path: Option<String>,
    pub run_plan_path: Option<String>,
    pub bundle_path: Option<String>,
    pub manifest_path: Option<String>,
    pub runner_script_path: Option<String>,
    pub runner_preflight_path: Option<String>,
    pub snapshot_path: Option<String>,
    pub manifest_found: bool,
    pub item_count: usize,
    pub item_file_count: usize,
    pub missing_total: usize,
    pub output_root: Option<String>,
    pub summary: Value,
    pub commands: Value,
    pub operator_checklist: Value,
    pub provider_gateway_worker_start_commands: Value,
    pub software_bridge_worker_start_commands: Value,
    pub source_node_ids: Vec<String>,
    pub latest_asset_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProductionEvidenceHandoffPackagePolicy {
    pub local_files_authoritative: bool,
    pub provider_urls_are_provenance: bool,
    pub expected_files: Vec<String>,
    pub item_templates_are_scaffolds: bool,
}

pub fn production_evidence_handoff_package_catalog_resource(
    snapshot: &RuntimeSnapshot,
) -> ProductionEvidenceHandoffPackageCatalog {
    let mut grouped_assets: BTreeMap<String, Vec<&AssetSnapshot>> = BTreeMap::new();
    for asset in snapshot.assets.iter().filter(|asset| {
        asset.provider_url.as_deref() == Some("pool-production-evidence://handoff-package")
            || production_evidence_handoff_dir_from_path(&asset.local_path).is_some()
    }) {
        if let Some(package_dir) = production_evidence_handoff_dir_from_path(&asset.local_path) {
            grouped_assets.entry(package_dir).or_default().push(asset);
        }
    }

    let mut packages = grouped_assets
        .into_iter()
        .map(|(package_dir, assets)| {
            production_evidence_handoff_package_summary(&package_dir, assets)
        })
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
    let item_files = packages.iter().map(|package| package.item_file_count).sum();
    let runner_packages = packages
        .iter()
        .filter(|package| package.runner_script_path.is_some())
        .count();
    let latest_asset_at = packages
        .iter()
        .filter_map(|package| package.latest_asset_at.clone())
        .max();

    ProductionEvidenceHandoffPackageCatalog {
        kind: "pool_production_evidence_handoff_packages".to_string(),
        project_filter: snapshot.project_filter.clone(),
        generated_at: snapshot.generated_at.clone(),
        summary: ProductionEvidenceHandoffPackageCatalogSummary {
            package_count: packages.len(),
            indexed_files,
            ready_packages,
            item_files,
            runner_packages,
            local_file_failures,
            latest_asset_at,
        },
        packages,
        policy: ProductionEvidenceHandoffPackagePolicy {
            local_files_authoritative: true,
            provider_urls_are_provenance: true,
            expected_files: production_evidence_handoff_package_expected_files()
                .iter()
                .map(|file| file.to_string())
                .collect(),
            item_templates_are_scaffolds: true,
        },
    }
}

fn production_evidence_handoff_package_summary(
    package_dir: &str,
    mut assets: Vec<&AssetSnapshot>,
) -> ProductionEvidenceHandoffPackageSummary {
    assets.sort_by(|left, right| right.created_at.cmp(&left.created_at));
    let mut seen_paths = BTreeSet::new();
    let mut unique_assets = Vec::new();
    for asset in assets {
        if seen_paths.insert(asset.local_path.clone()) {
            unique_assets.push(asset);
        }
    }
    unique_assets.sort_by(|left, right| {
        production_evidence_handoff_package_file_rank(&left.local_path)
            .cmp(&production_evidence_handoff_package_file_rank(
                &right.local_path,
            ))
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
    let request_path = production_evidence_handoff_path_with_file(
        &local_files,
        ".1-production-evidence-handoff-package-request.json",
    );
    let requirements_path = production_evidence_handoff_path_with_file(
        &local_files,
        "1-production-evidence-requirements.json",
    );
    let tasks_path = production_evidence_handoff_path_with_file(
        &local_files,
        "2-production-evidence-tasks.json",
    );
    let handoff_path = production_evidence_handoff_path_with_file(
        &local_files,
        "3-production-evidence-handoff.json",
    );
    let run_plan_path = production_evidence_handoff_path_with_file(
        &local_files,
        "4-production-evidence-run-plan.json",
    );
    let bundle_path = production_evidence_handoff_path_with_file(
        &local_files,
        "5-production-evidence-bundle.json",
    );
    let manifest_path = production_evidence_handoff_path_with_file(
        &local_files,
        "6-production-evidence-package-manifest.json",
    );
    let runner_script_path =
        production_evidence_handoff_path_with_file(&local_files, "7-production-evidence-runner.sh");
    let runner_preflight_path = production_evidence_handoff_path_with_file(
        &local_files,
        "8-production-evidence-runner-preflight.json",
    );
    let snapshot_path =
        production_evidence_handoff_path_with_file(&local_files, "9-runtime-snapshot.json");
    let manifest = manifest_path.as_deref().and_then(read_json_file);
    let runner_preflight = runner_preflight_path.as_deref().and_then(read_json_file);
    let manifest_found = manifest.is_some();
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
    let item_count = manifest
        .as_ref()
        .and_then(|manifest| manifest["items"].as_array())
        .map(Vec::len)
        .unwrap_or(0);
    let item_file_count = local_files
        .iter()
        .filter(|path| path.contains("/items/") && path.ends_with("-item.json"))
        .count();
    let missing_total = summary["missing_total"].as_u64().unwrap_or(0) as usize;
    let output_root = manifest
        .as_ref()
        .and_then(|manifest| manifest["output_root"].as_str())
        .map(|value| value.to_string());
    let provider_gateway_worker_start_commands = runner_preflight
        .as_ref()
        .map(|preflight| preflight["environment"]["provider_gateway_worker_start_commands"].clone())
        .unwrap_or_else(|| json!([]));
    let software_bridge_worker_start_commands = runner_preflight
        .as_ref()
        .map(|preflight| preflight["environment"]["software_bridge_worker_start_commands"].clone())
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
    } else if manifest_found && runner_script_path.is_some() && runner_preflight_path.is_some() {
        "ready"
    } else if manifest_found {
        "manifest_only"
    } else {
        "indexed"
    };
    let project_slug = unique_assets
        .first()
        .map(|asset| asset.project_slug.clone())
        .unwrap_or_default();

    ProductionEvidenceHandoffPackageSummary {
        package_id: format!(
            "production-evidence-handoff-package:{}",
            manifest_path.as_deref().unwrap_or(package_dir)
        ),
        project_slug,
        package_dir: package_dir.to_string(),
        status: status.to_string(),
        local_files,
        local_file_failures,
        request_path,
        requirements_path,
        tasks_path,
        handoff_path,
        run_plan_path,
        bundle_path,
        manifest_path,
        runner_script_path,
        runner_preflight_path,
        snapshot_path,
        manifest_found,
        item_count,
        item_file_count,
        missing_total,
        output_root,
        summary,
        commands,
        operator_checklist,
        provider_gateway_worker_start_commands,
        software_bridge_worker_start_commands,
        source_node_ids,
        latest_asset_at,
    }
}

fn production_evidence_handoff_dir_from_path(path: &str) -> Option<String> {
    let parent = Path::new(path).parent()?;
    let normalized = parent.to_string_lossy().replace('\\', "/");
    if normalized.ends_with("/control/production-evidence/items")
        || normalized == "control/production-evidence/items"
    {
        let package_dir = Path::new(&normalized)
            .parent()
            .map(|parent| parent.to_string_lossy().replace('\\', "/"))?;
        return Some(package_dir);
    }
    if normalized == "control/production-evidence"
        || normalized.ends_with("/control/production-evidence")
    {
        Some(normalized)
    } else {
        None
    }
}

fn production_evidence_handoff_path_with_file(paths: &[String], file_name: &str) -> Option<String> {
    paths
        .iter()
        .find(|path| Path::new(path).file_name().and_then(|name| name.to_str()) == Some(file_name))
        .cloned()
}

fn production_evidence_handoff_package_file_rank(path: &str) -> usize {
    let file_name = Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    production_evidence_handoff_package_expected_files()
        .iter()
        .position(|expected| *expected == file_name)
        .unwrap_or_else(|| {
            if path.contains("/items/") {
                100
            } else {
                usize::MAX
            }
        })
}

fn production_evidence_handoff_package_expected_files() -> &'static [&'static str] {
    &[
        ".1-production-evidence-handoff-package-request.json",
        "1-production-evidence-requirements.json",
        "2-production-evidence-tasks.json",
        "3-production-evidence-handoff.json",
        "4-production-evidence-run-plan.json",
        "5-production-evidence-bundle.json",
        "6-production-evidence-package-manifest.json",
        "7-production-evidence-runner.sh",
        "8-production-evidence-runner-preflight.json",
        "9-runtime-snapshot.json",
    ]
}

fn read_json_file(path: &str) -> Option<Value> {
    let content = fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}
