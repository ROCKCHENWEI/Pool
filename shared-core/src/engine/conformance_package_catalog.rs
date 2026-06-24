use serde::Serialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::db::{AssetSnapshot, RuntimeSnapshot};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConformancePackageKind {
    Provider,
    Software,
    Agent,
    Integration,
}

impl ConformancePackageKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Provider => "provider",
            Self::Software => "software",
            Self::Agent => "agent",
            Self::Integration => "integration",
        }
    }

    pub fn catalog_kind(self) -> &'static str {
        match self {
            Self::Provider => "pool_provider_conformance_packages",
            Self::Software => "pool_software_conformance_packages",
            Self::Agent => "pool_agent_conformance_packages",
            Self::Integration => "pool_integration_conformance_packages",
        }
    }

    fn provider_url_prefix(self) -> &'static str {
        match self {
            Self::Provider => "pool-provider-conformance://",
            Self::Software => "pool-software-conformance://",
            Self::Agent => "pool-agent-conformance://",
            Self::Integration => "pool-integration-conformance://",
        }
    }

    fn manifest_file(self) -> &'static str {
        match self {
            Self::Provider => "6-provider-conformance-package-manifest.json",
            Self::Software => "5-software-conformance-package-manifest.json",
            Self::Agent => "5-agent-conformance-package-manifest.json",
            Self::Integration => "3-integration-conformance-package-manifest.json",
        }
    }

    fn runner_file(self) -> &'static str {
        match self {
            Self::Provider => "5-provider-conformance-runner.sh",
            Self::Software => "4-software-conformance-runner.sh",
            Self::Agent => "4-agent-conformance-runner.sh",
            Self::Integration => "2-integration-conformance-runner.sh",
        }
    }

    fn preflight_file(self) -> Option<&'static str> {
        match self {
            Self::Provider => Some("4-provider-conformance-preflight.json"),
            Self::Software => Some("3-software-conformance-preflight.json"),
            Self::Agent => Some("3-agent-conformance-preflight.json"),
            Self::Integration => None,
        }
    }

    fn request_file(self) -> &'static str {
        match self {
            Self::Provider => ".1-provider-conformance-package-request.json",
            Self::Software => ".1-software-conformance-package-request.json",
            Self::Agent => ".1-agent-conformance-package-request.json",
            Self::Integration => ".1-integration-conformance-package-request.json",
        }
    }

    fn runbook_file(self) -> &'static str {
        match self {
            Self::Provider => "3-provider-conformance-runbook.json",
            Self::Software => "2-software-conformance-runbook.json",
            Self::Agent => "2-agent-conformance-runbook.json",
            Self::Integration => "1-integration-conformance-runbook.json",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ConformancePackageCatalog {
    pub kind: String,
    pub package_kind: ConformancePackageKind,
    pub project_filter: Option<String>,
    pub generated_at: String,
    pub summary: ConformancePackageCatalogSummary,
    pub packages: Vec<ConformancePackageSummary>,
    pub policy: ConformancePackagePolicy,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConformancePackageCatalogSummary {
    pub package_count: usize,
    pub indexed_files: usize,
    pub ready_packages: usize,
    pub runner_packages: usize,
    pub local_file_failures: Vec<String>,
    pub latest_asset_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConformancePackageSummary {
    pub package_id: String,
    pub package_kind: ConformancePackageKind,
    pub target_id: Option<String>,
    pub project_slug: String,
    pub package_dir: String,
    pub status: String,
    pub local_files: Vec<String>,
    pub local_file_failures: Vec<String>,
    pub request_path: Option<String>,
    pub contract_path: Option<String>,
    pub gateway_worker_contract_path: Option<String>,
    pub runbook_path: Option<String>,
    pub preflight_path: Option<String>,
    pub runner_script_path: Option<String>,
    pub manifest_path: Option<String>,
    pub manifest_found: bool,
    pub title: Option<String>,
    pub paths: Value,
    pub commands: Value,
    pub next_actions: Value,
    pub summary: Value,
    pub packages: Value,
    pub source_node_ids: Vec<String>,
    pub latest_asset_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConformancePackagePolicy {
    pub local_files_authoritative: bool,
    pub provider_urls_are_provenance: bool,
    pub secrets_stay_server_side: bool,
    pub expected_files: Vec<String>,
}

pub fn conformance_package_catalog_resource(
    snapshot: &RuntimeSnapshot,
    package_kind: ConformancePackageKind,
) -> ConformancePackageCatalog {
    let mut grouped_assets: BTreeMap<String, Vec<&AssetSnapshot>> = BTreeMap::new();
    for asset in snapshot.assets.iter().filter(|asset| {
        asset
            .provider_url
            .as_deref()
            .map(|url| url.starts_with(package_kind.provider_url_prefix()))
            .unwrap_or(false)
            || conformance_package_dir_from_path(&asset.local_path, package_kind).is_some()
    }) {
        if let Some(package_dir) = conformance_package_dir_from_asset(asset, package_kind) {
            grouped_assets.entry(package_dir).or_default().push(asset);
        }
    }

    let mut packages = grouped_assets
        .into_iter()
        .map(|(package_dir, assets)| {
            conformance_package_summary(package_kind, &package_dir, assets)
        })
        .collect::<Vec<_>>();
    packages.sort_by(|left, right| {
        right
            .latest_asset_at
            .cmp(&left.latest_asset_at)
            .then_with(|| left.package_dir.cmp(&right.package_dir))
    });

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
    let runner_packages = packages
        .iter()
        .filter(|package| package.runner_script_path.is_some())
        .count();
    let latest_asset_at = packages
        .iter()
        .filter_map(|package| package.latest_asset_at.clone())
        .max();

    ConformancePackageCatalog {
        kind: package_kind.catalog_kind().to_string(),
        package_kind,
        project_filter: snapshot.project_filter.clone(),
        generated_at: snapshot.generated_at.clone(),
        summary: ConformancePackageCatalogSummary {
            package_count: packages.len(),
            indexed_files,
            ready_packages,
            runner_packages,
            local_file_failures,
            latest_asset_at,
        },
        packages,
        policy: ConformancePackagePolicy {
            local_files_authoritative: true,
            provider_urls_are_provenance: true,
            secrets_stay_server_side: true,
            expected_files: conformance_expected_files(package_kind),
        },
    }
}

fn conformance_package_summary(
    package_kind: ConformancePackageKind,
    package_dir: &str,
    mut assets: Vec<&AssetSnapshot>,
) -> ConformancePackageSummary {
    assets.sort_by(|left, right| right.created_at.cmp(&left.created_at));
    let mut seen_paths = BTreeSet::new();
    let mut unique_assets = Vec::new();
    for asset in assets {
        if conformance_package_dir_from_asset(asset, package_kind).as_deref() == Some(package_dir)
            && seen_paths.insert(asset.local_path.clone())
        {
            unique_assets.push(asset);
        }
    }
    unique_assets.sort_by(|left, right| {
        conformance_file_rank(&left.local_path, package_kind)
            .cmp(&conformance_file_rank(&right.local_path, package_kind))
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
    let request_path = conformance_path_with_file(&local_files, package_kind.request_file());
    let contract_path = conformance_contract_path(&local_files, package_kind);
    let gateway_worker_contract_path =
        conformance_path_with_file(&local_files, "2-provider-gateway-worker-contract.json");
    let runbook_path = conformance_path_with_file(&local_files, package_kind.runbook_file());
    let preflight_path = package_kind
        .preflight_file()
        .and_then(|file| conformance_path_with_file(&local_files, file));
    let runner_script_path = conformance_path_with_file(&local_files, package_kind.runner_file());
    let manifest_path = conformance_path_with_file(&local_files, package_kind.manifest_file());
    let manifest = manifest_path.as_deref().and_then(read_json_file);
    let manifest_found = manifest.is_some();
    let title = manifest
        .as_ref()
        .and_then(|manifest| manifest["title"].as_str())
        .map(ToString::to_string);
    let paths = manifest
        .as_ref()
        .map(|manifest| manifest["paths"].clone())
        .unwrap_or_else(|| json!({}));
    let commands = manifest
        .as_ref()
        .map(|manifest| manifest["commands"].clone())
        .unwrap_or_else(|| json!({}));
    let next_actions = manifest
        .as_ref()
        .map(|manifest| manifest["next_actions"].clone())
        .unwrap_or_else(|| json!([]));
    let summary = manifest
        .as_ref()
        .map(|manifest| manifest["summary"].clone())
        .unwrap_or_else(|| json!({}));
    let packages = manifest
        .as_ref()
        .map(|manifest| manifest["packages"].clone())
        .unwrap_or_else(|| json!({}));
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
    let project_slug = unique_assets
        .first()
        .map(|asset| asset.project_slug.clone())
        .unwrap_or_default();
    let status = if !local_file_failures.is_empty() {
        "indexed_missing_file"
    } else if manifest_found
        && runner_script_path.is_some()
        && (package_kind.preflight_file().is_none() || preflight_path.is_some())
    {
        "ready"
    } else if manifest_found {
        "manifest_only"
    } else {
        "indexed"
    };

    ConformancePackageSummary {
        package_id: format!(
            "{}-conformance-package:{}",
            package_kind.as_str(),
            manifest_path.as_deref().unwrap_or(package_dir)
        ),
        package_kind,
        target_id: conformance_target_id(package_dir, package_kind),
        project_slug,
        package_dir: package_dir.to_string(),
        status: status.to_string(),
        local_files,
        local_file_failures,
        request_path,
        contract_path,
        gateway_worker_contract_path,
        runbook_path,
        preflight_path,
        runner_script_path,
        manifest_path,
        manifest_found,
        title,
        paths,
        commands,
        next_actions,
        summary,
        packages,
        source_node_ids,
        latest_asset_at,
    }
}

fn conformance_package_dir_from_asset(
    asset: &AssetSnapshot,
    package_kind: ConformancePackageKind,
) -> Option<String> {
    if asset
        .provider_url
        .as_deref()
        .map(|url| url.starts_with(package_kind.provider_url_prefix()))
        .unwrap_or(false)
    {
        return conformance_package_dir_from_provider_url(&asset.local_path, package_kind)
            .or_else(|| conformance_package_dir_from_path(&asset.local_path, package_kind));
    }
    conformance_package_dir_from_path(&asset.local_path, package_kind)
}

fn conformance_package_dir_from_provider_url(
    path: &str,
    package_kind: ConformancePackageKind,
) -> Option<String> {
    let parent = Path::new(path).parent()?;
    let normalized = parent.to_string_lossy().replace('\\', "/");
    match package_kind {
        ConformancePackageKind::Integration => {
            conformance_root_dir(&normalized, "integration-conformance")
        }
        _ => Some(normalized),
    }
}

fn conformance_package_dir_from_path(
    path: &str,
    package_kind: ConformancePackageKind,
) -> Option<String> {
    let parent = Path::new(path).parent()?;
    let normalized = parent.to_string_lossy().replace('\\', "/");
    match package_kind {
        ConformancePackageKind::Provider => conformance_nested_dir(
            &normalized,
            &["provider-conformance", "integration-conformance/providers"],
        ),
        ConformancePackageKind::Software => conformance_nested_dir(
            &normalized,
            &["software-conformance", "integration-conformance/software"],
        ),
        ConformancePackageKind::Agent => conformance_nested_dir(
            &normalized,
            &["agent-conformance", "integration-conformance/agent"],
        ),
        ConformancePackageKind::Integration => {
            conformance_root_dir(&normalized, "integration-conformance")
        }
    }
}

fn conformance_nested_dir(parent: &str, markers: &[&str]) -> Option<String> {
    for marker in markers {
        let needle = format!("/control/{marker}/");
        if parent.contains(&needle) {
            return Some(parent.to_string());
        }
        let relative = format!("control/{marker}/");
        if parent.starts_with(&relative) {
            return Some(parent.to_string());
        }
    }
    None
}

fn conformance_root_dir(parent: &str, marker: &str) -> Option<String> {
    let needle = format!("/control/{marker}");
    if let Some(index) = parent.find(&needle) {
        let end = index + needle.len();
        return Some(parent[..end].to_string());
    }
    let relative = format!("control/{marker}");
    if parent == relative || parent.starts_with(&(relative.clone() + "/")) {
        return Some(relative);
    }
    None
}

fn conformance_target_id(
    package_dir: &str,
    package_kind: ConformancePackageKind,
) -> Option<String> {
    if package_kind == ConformancePackageKind::Integration {
        return None;
    }
    Path::new(package_dir)
        .file_name()
        .and_then(|name| name.to_str())
        .map(ToString::to_string)
}

fn conformance_contract_path(
    paths: &[String],
    package_kind: ConformancePackageKind,
) -> Option<String> {
    match package_kind {
        ConformancePackageKind::Provider => {
            conformance_path_with_file(paths, "1-provider-contract.json")
        }
        ConformancePackageKind::Software => {
            conformance_path_with_file(paths, "1-software-control-contract.json")
        }
        ConformancePackageKind::Agent => {
            conformance_path_with_file(paths, "1-agent-session-contract.json")
        }
        ConformancePackageKind::Integration => None,
    }
}

fn conformance_path_with_file(paths: &[String], file_name: &str) -> Option<String> {
    paths
        .iter()
        .find(|path| Path::new(path).file_name().and_then(|name| name.to_str()) == Some(file_name))
        .cloned()
}

fn conformance_file_rank(path: &str, package_kind: ConformancePackageKind) -> usize {
    let file_name = Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    conformance_expected_files(package_kind)
        .iter()
        .position(|expected| expected == file_name)
        .unwrap_or(usize::MAX)
}

fn conformance_expected_files(package_kind: ConformancePackageKind) -> Vec<String> {
    let files = match package_kind {
        ConformancePackageKind::Provider => vec![
            ".1-provider-conformance-package-request.json",
            "1-provider-contract.json",
            "2-provider-gateway-worker-contract.json",
            "3-provider-conformance-runbook.json",
            "4-provider-conformance-preflight.json",
            "5-provider-conformance-runner.sh",
            "6-provider-conformance-package-manifest.json",
        ],
        ConformancePackageKind::Software => vec![
            ".1-software-conformance-package-request.json",
            "1-software-control-contract.json",
            "2-software-conformance-runbook.json",
            "3-software-conformance-preflight.json",
            "4-software-conformance-runner.sh",
            "5-software-conformance-package-manifest.json",
        ],
        ConformancePackageKind::Agent => vec![
            ".1-agent-conformance-package-request.json",
            "1-agent-session-contract.json",
            "2-agent-conformance-runbook.json",
            "3-agent-conformance-preflight.json",
            "4-agent-conformance-runner.sh",
            "5-agent-conformance-package-manifest.json",
        ],
        ConformancePackageKind::Integration => vec![
            ".1-integration-conformance-package-request.json",
            "1-integration-conformance-runbook.json",
            "2-integration-conformance-runner.sh",
            "3-integration-conformance-package-manifest.json",
        ],
    };
    files.into_iter().map(ToString::to_string).collect()
}

fn read_json_file(path: &str) -> Option<Value> {
    let content = fs::read_to_string(PathBuf::from(path)).ok()?;
    serde_json::from_str(&content).ok()
}
