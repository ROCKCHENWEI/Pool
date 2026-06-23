use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

use crate::db::RuntimeRepository;
use crate::db::{AssetSnapshot, RuntimeSnapshot};
use crate::models::{AssetRecord, RuntimeEvent, RuntimeEventLevel, RuntimeTask, TaskStatus};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputPackageRequest {
    pub project_slug: String,
    pub node_id: Option<String>,
    pub output_dir: String,
    pub title: String,
    pub source_assets: Vec<String>,
    pub duration_ms: u64,
}

impl OutputPackageRequest {
    pub fn new(project_slug: impl Into<String>, output_dir: impl Into<String>) -> Self {
        Self {
            project_slug: project_slug.into(),
            node_id: None,
            output_dir: output_dir.into(),
            title: "Pool output package".to_string(),
            source_assets: Vec::new(),
            duration_ms: 12_000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputDeliverableResultRequest {
    pub project_slug: String,
    pub node_id: Option<String>,
    pub target: String,
    pub local_path: Option<String>,
    pub status: String,
    pub runtime: Option<String>,
    pub adapter_id: Option<String>,
    pub software_action_id: Option<String>,
    pub message: Option<String>,
    pub artifacts: Vec<String>,
    pub metrics: Vec<OutputManifestMetric>,
    pub verification: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OutputDeliverableResultReport {
    pub task_id: String,
    pub status: TaskStatus,
    pub target: String,
    pub local_path: String,
    pub manifest: Value,
    pub catalog: OutputPackageCatalog,
}

#[derive(Debug, Clone, Serialize)]
pub struct OutputPackageRunReport {
    pub task_id: String,
    pub status: TaskStatus,
    pub local_paths: Vec<String>,
    pub manifests: Vec<OutputManifestSummary>,
    pub assets: Vec<AssetRecord>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OutputManifestSummary {
    pub target: String,
    pub title: String,
    pub local_path: String,
    pub primary_runtime: String,
    pub metrics: Vec<OutputManifestMetric>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputManifestMetric {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct OutputPackageCatalog {
    pub kind: String,
    pub project_filter: Option<String>,
    pub generated_at: String,
    pub summary: OutputPackageCatalogSummary,
    pub deliverables: Vec<OutputDeliverableSummary>,
    pub policy: OutputPackagePolicy,
}

#[derive(Debug, Clone, Serialize)]
pub struct OutputPackageCatalogSummary {
    pub total_targets: usize,
    pub indexed_targets: usize,
    pub ready_targets: usize,
    pub missing_targets: Vec<String>,
    pub local_file_failures: Vec<String>,
    pub latest_asset_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OutputDeliverableSummary {
    pub target: String,
    pub title: String,
    pub expected_file: String,
    pub primary_runtime: String,
    pub status: String,
    pub local_path: Option<String>,
    pub asset_id: Option<String>,
    pub asset_name: Option<String>,
    pub asset_status: Option<String>,
    pub source_node_id: Option<String>,
    pub provider_url: Option<String>,
    pub file_found: bool,
    pub manifest_found: bool,
    pub metrics: Vec<OutputManifestMetric>,
    pub preview_contract: Value,
    pub control_routes: Vec<String>,
    pub next_action: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct OutputPackagePolicy {
    pub local_files_authoritative: bool,
    pub provider_urls_are_provenance: bool,
    pub expected_targets: Vec<String>,
}

pub struct OutputPackageRunner<'a> {
    repository: &'a RuntimeRepository,
}

impl<'a> OutputPackageRunner<'a> {
    pub fn new(repository: &'a RuntimeRepository) -> Self {
        Self { repository }
    }

    pub fn run(&self, request: OutputPackageRequest) -> Result<OutputPackageRunReport> {
        let mut task = RuntimeTask::new(request.project_slug.clone(), request.title.clone());
        task.node_id = request.node_id.clone();
        task.provider_id = Some("output-package".to_string());
        task.cost_estimate_tokens = 600;
        task.status = TaskStatus::Running;
        self.repository.insert_task(&task)?;
        self.repository.insert_event(&RuntimeEvent::new(
            request.project_slug.clone(),
            RuntimeEventLevel::Info,
            format!("output package started: {}", request.title),
        ))?;

        let deliverables_dir = Path::new(&request.output_dir).join("deliverables");
        fs::create_dir_all(&deliverables_dir)
            .with_context(|| format!("create deliverables dir {}", deliverables_dir.display()))?;

        let package = write_output_manifests(&deliverables_dir, &request)?;
        let assets = self.repository.index_local_outputs(
            &request.project_slug,
            None,
            Some("pool-output-package://deliverables"),
            &package.local_paths,
        )?;
        self.repository
            .update_task_status(&task.id, TaskStatus::Succeeded)?;
        self.repository.insert_event(&RuntimeEvent::new(
            request.project_slug.clone(),
            RuntimeEventLevel::Ok,
            format!("output package succeeded: {} deliverables", assets.len()),
        ))?;

        Ok(OutputPackageRunReport {
            task_id: task.id,
            status: TaskStatus::Succeeded,
            local_paths: package.local_paths,
            manifests: package.manifests,
            assets,
        })
    }

    pub fn record_result(
        &self,
        request: OutputDeliverableResultRequest,
    ) -> Result<OutputDeliverableResultReport> {
        let target = request.target.trim();
        let Some(descriptor) = output_descriptors()
            .into_iter()
            .find(|descriptor| descriptor.target == target)
        else {
            bail!("unknown output deliverable target: {target}");
        };
        if request.status.trim().is_empty() {
            bail!("output deliverable result status is required");
        }

        let snapshot = self.repository.snapshot(Some(&request.project_slug))?;
        let deliverable = output_deliverable_from_snapshot(&snapshot, &descriptor);
        let local_path = request
            .local_path
            .clone()
            .or(deliverable.local_path.clone())
            .with_context(|| format!("output deliverable not indexed for target {target}"))?;
        let manifest_path = Path::new(&local_path);
        if !manifest_path.is_file() {
            bail!("output deliverable manifest is missing: {}", local_path);
        }
        let mut manifest = read_manifest_json(&local_path)
            .with_context(|| format!("read output deliverable manifest {}", local_path))?;
        let status = task_status_from_output_result(&request.status);
        let result_value = json!({
            "target": target,
            "status": request.status,
            "runtime": request.runtime,
            "adapter_id": request.adapter_id,
            "software_action_id": request.software_action_id,
            "message": request.message,
            "artifacts": request.artifacts,
            "metrics": request.metrics,
            "verification": request.verification,
            "updated_at": chrono::Utc::now().to_rfc3339(),
        });
        merge_execution_result(&mut manifest, result_value)?;
        write_json(manifest_path, &manifest)?;

        let mut task = RuntimeTask::new(
            request.project_slug.clone(),
            format!("Output result: {}", descriptor.title),
        );
        task.node_id = request.node_id.clone().or(deliverable.source_node_id);
        task.provider_id = Some("output-package-result".to_string());
        task.cost_estimate_tokens = 100;
        task.request_metadata_path = Some(local_path.clone());
        task.status = status.clone();
        self.repository.insert_task(&task)?;
        self.repository.insert_event(&RuntimeEvent::new(
            request.project_slug.clone(),
            event_level_for_output_result(&status),
            format!("output result recorded: {target} -> {}", request.status),
        ))?;

        let refreshed = self.repository.snapshot(Some(&request.project_slug))?;
        let catalog = output_package_catalog_resource(&refreshed);

        Ok(OutputDeliverableResultReport {
            task_id: task.id,
            status,
            target: target.to_string(),
            local_path,
            manifest,
            catalog,
        })
    }
}

pub fn output_package_catalog_resource(snapshot: &RuntimeSnapshot) -> OutputPackageCatalog {
    let descriptors = output_descriptors();
    let deliverables = descriptors
        .iter()
        .map(|descriptor| output_deliverable_from_snapshot(snapshot, descriptor))
        .collect::<Vec<_>>();
    let ready_targets = deliverables
        .iter()
        .filter(|deliverable| deliverable.status == "ready")
        .count();
    let indexed_targets = deliverables
        .iter()
        .filter(|deliverable| deliverable.asset_id.is_some())
        .count();
    let missing_targets = deliverables
        .iter()
        .filter(|deliverable| deliverable.asset_id.is_none())
        .map(|deliverable| deliverable.target.clone())
        .collect::<Vec<_>>();
    let local_file_failures = deliverables
        .iter()
        .filter(|deliverable| deliverable.asset_id.is_some() && !deliverable.file_found)
        .filter_map(|deliverable| deliverable.local_path.clone())
        .collect::<Vec<_>>();
    let latest_asset_at = deliverables
        .iter()
        .filter_map(|deliverable| {
            deliverable
                .asset_id
                .as_ref()
                .and_then(|_| {
                    snapshot
                        .assets
                        .iter()
                        .find(|asset| Some(&asset.id) == deliverable.asset_id.as_ref())
                })
                .map(|asset| asset.created_at.clone())
        })
        .max();

    OutputPackageCatalog {
        kind: "pool_output_packages".to_string(),
        project_filter: snapshot.project_filter.clone(),
        generated_at: snapshot.generated_at.clone(),
        summary: OutputPackageCatalogSummary {
            total_targets: descriptors.len(),
            indexed_targets,
            ready_targets,
            missing_targets,
            local_file_failures,
            latest_asset_at,
        },
        deliverables,
        policy: OutputPackagePolicy {
            local_files_authoritative: true,
            provider_urls_are_provenance: true,
            expected_targets: descriptors
                .iter()
                .map(|descriptor| descriptor.target.to_string())
                .collect(),
        },
    }
}

#[derive(Debug, Clone)]
struct WrittenOutputPackage {
    local_paths: Vec<String>,
    manifests: Vec<OutputManifestSummary>,
}

#[derive(Debug, Clone)]
struct OutputDescriptor {
    target: &'static str,
    title: &'static str,
    expected_file: &'static str,
    primary_runtime: &'static str,
    next_action: &'static str,
    preview_contract: Value,
    control_routes: Vec<&'static str>,
}

fn write_output_manifests(
    deliverables_dir: &Path,
    request: &OutputPackageRequest,
) -> Result<WrittenOutputPackage> {
    let video = deliverables_dir.join("1-video-timeline.json");
    let game = deliverables_dir.join("2-game-build.json");
    let interactive = deliverables_dir.join("3-interactive-cues.json");
    let source_asset_count = request.source_assets.len();
    let duration_seconds = format!("{:.1}s", request.duration_ms as f64 / 1000.0);

    write_json(
        &video,
        &json!({
            "target": "video",
            "project_slug": request.project_slug,
            "timeline": {
                "duration_ms": request.duration_ms,
                "fps": 24,
                "tracks": [
                    {
                        "name": "camera",
                        "clips": [
                            {
                                "start_ms": 0,
                                "duration_ms": request.duration_ms,
                                "source": "unreal://sequencer/main"
                            }
                        ]
                    },
                    {
                        "name": "assets",
                        "clips": request.source_assets
                    }
                ]
            },
            "transcode": {
                "container": "mp4",
                "codec": "h264",
                "resolution": "1920x1080"
            }
        }),
    )?;
    write_json(
        &game,
        &json!({
            "target": "game",
            "project_slug": request.project_slug,
            "engine": "unreal",
            "level": "demo_content_burst",
            "viewport": {
                "mode": "play_in_editor",
                "start_camera": "hero_orbit"
            },
            "assets": request.source_assets,
            "build": {
                "platforms": ["macos", "windows"],
                "configuration": "development"
            }
        }),
    )?;
    write_json(
        &interactive,
        &json!({
            "target": "interactive_art",
            "project_slug": request.project_slug,
            "runtime": "touchdesigner/madmapper",
            "cue_graph": [
                {
                    "id": "cue-1",
                    "time_ms": 0,
                    "visual": "unreal://viewport/feed",
                    "audio": "suno://cue/main",
                    "controls": ["osc:/pool/cue/1", "dmx:universe-1"]
                }
            ],
            "device_interfaces": ["osc", "midi", "dmx"],
            "assets": request.source_assets
        }),
    )?;

    let video_path = path_string(video);
    let game_path = path_string(game);
    let interactive_path = path_string(interactive);

    Ok(WrittenOutputPackage {
        local_paths: vec![
            video_path.clone(),
            game_path.clone(),
            interactive_path.clone(),
        ],
        manifests: vec![
            OutputManifestSummary {
                target: "video".to_string(),
                title: "时间线与转码".to_string(),
                local_path: video_path,
                primary_runtime: "DaVinci Resolve / FFmpeg".to_string(),
                metrics: vec![
                    metric("duration", &duration_seconds),
                    metric("fps", "24"),
                    metric("tracks", "2"),
                    metric("transcode", "mp4 h264 1920x1080"),
                ],
            },
            OutputManifestSummary {
                target: "game".to_string(),
                title: "运行原型".to_string(),
                local_path: game_path,
                primary_runtime: "Unreal".to_string(),
                metrics: vec![
                    metric("level", "demo_content_burst"),
                    metric("viewport", "play_in_editor"),
                    metric("platforms", "macos, windows"),
                    metric("assets", &source_asset_count.to_string()),
                ],
            },
            OutputManifestSummary {
                target: "interactive_art".to_string(),
                title: "节点与现场控制".to_string(),
                local_path: interactive_path,
                primary_runtime: "TouchDesigner / MadMapper".to_string(),
                metrics: vec![
                    metric("cues", "1"),
                    metric("interfaces", "osc, midi, dmx"),
                    metric("visual", "unreal viewport feed"),
                    metric("assets", &source_asset_count.to_string()),
                ],
            },
        ],
    })
}

fn metric(label: &str, value: &str) -> OutputManifestMetric {
    OutputManifestMetric {
        label: label.to_string(),
        value: value.to_string(),
    }
}

fn merge_execution_result(manifest: &mut Value, result: Value) -> Result<()> {
    let Some(object) = manifest.as_object_mut() else {
        bail!("output deliverable manifest must be a JSON object");
    };
    object.insert("execution_result".to_string(), result.clone());
    match object
        .get_mut("execution_history")
        .and_then(Value::as_array_mut)
    {
        Some(history) => history.push(result),
        None => {
            object.insert("execution_history".to_string(), json!([result]));
        }
    }
    Ok(())
}

fn task_status_from_output_result(status: &str) -> TaskStatus {
    match status.trim().to_ascii_lowercase().as_str() {
        "succeeded" | "success" | "ok" | "completed" | "complete" | "ready" => {
            TaskStatus::Succeeded
        }
        "running" | "in_progress" | "processing" => TaskStatus::Running,
        "failed" | "failure" | "error" => TaskStatus::Failed,
        "retryable" | "retry" => TaskStatus::Retryable,
        "cancelled" | "canceled" => TaskStatus::Cancelled,
        _ => TaskStatus::Retryable,
    }
}

fn event_level_for_output_result(status: &TaskStatus) -> RuntimeEventLevel {
    match status {
        TaskStatus::Succeeded => RuntimeEventLevel::Ok,
        TaskStatus::Failed | TaskStatus::Retryable | TaskStatus::Cancelled => {
            RuntimeEventLevel::Warn
        }
        _ => RuntimeEventLevel::Info,
    }
}

fn output_descriptors() -> Vec<OutputDescriptor> {
    vec![
        OutputDescriptor {
            target: "video",
            title: "时间线与转码",
            expected_file: "1-video-timeline.json",
            primary_runtime: "DaVinci Resolve / FFmpeg",
            next_action: "open timeline manifest in Resolve or transcode with FFmpeg",
            preview_contract: json!({
                "surface": "timeline",
                "requires": ["timeline.duration_ms", "timeline.tracks", "transcode"],
                "handoff": "resolve-or-ffmpeg",
            }),
            control_routes: vec!["resolve", "editing-software", "ffmpeg-cli"],
        },
        OutputDescriptor {
            target: "game",
            title: "运行原型",
            expected_file: "2-game-build.json",
            primary_runtime: "Unreal",
            next_action: "open runtime viewport and build configured Unreal level",
            preview_contract: json!({
                "surface": "runtime_viewport",
                "requires": ["engine", "level", "viewport", "build.platforms"],
                "handoff": "unreal-mcp",
            }),
            control_routes: vec!["unreal", "unity-future"],
        },
        OutputDescriptor {
            target: "interactive_art",
            title: "节点与现场控制",
            expected_file: "3-interactive-cues.json",
            primary_runtime: "TouchDesigner / MadMapper",
            next_action: "load cue graph and map OSC/MIDI/DMX device routes",
            preview_contract: json!({
                "surface": "cue_graph",
                "requires": ["cue_graph", "device_interfaces"],
                "handoff": "touchdesigner-or-madmapper",
            }),
            control_routes: vec!["touchdesigner", "madmapper", "osc", "midi", "dmx"],
        },
    ]
}

fn output_deliverable_from_snapshot(
    snapshot: &RuntimeSnapshot,
    descriptor: &OutputDescriptor,
) -> OutputDeliverableSummary {
    let asset = snapshot
        .assets
        .iter()
        .find(|asset| output_target_from_path(&asset.local_path) == Some(descriptor.target));
    let local_path = asset.map(|asset| asset.local_path.clone());
    let file_found = local_path
        .as_deref()
        .is_some_and(|path| Path::new(path).is_file());
    let manifest = local_path.as_deref().and_then(read_manifest_json);
    let manifest_found = manifest.is_some();
    let status = output_deliverable_status(asset, file_found);
    let metrics = manifest
        .as_ref()
        .map(|manifest| manifest_metrics(descriptor.target, manifest))
        .filter(|metrics| !metrics.is_empty())
        .unwrap_or_else(|| fallback_deliverable_metrics(asset, descriptor));

    OutputDeliverableSummary {
        target: descriptor.target.to_string(),
        title: descriptor.title.to_string(),
        expected_file: descriptor.expected_file.to_string(),
        primary_runtime: descriptor.primary_runtime.to_string(),
        status,
        local_path,
        asset_id: asset.map(|asset| asset.id.clone()),
        asset_name: asset.map(|asset| asset.name.clone()),
        asset_status: asset.map(|asset| asset.status.clone()),
        source_node_id: asset.and_then(|asset| asset.source_node_id.clone()),
        provider_url: asset.and_then(|asset| asset.provider_url.clone()),
        file_found,
        manifest_found,
        metrics,
        preview_contract: descriptor.preview_contract.clone(),
        control_routes: descriptor
            .control_routes
            .iter()
            .map(|route| route.to_string())
            .collect(),
        next_action: descriptor.next_action.to_string(),
    }
}

fn output_target_from_path(path: &str) -> Option<&'static str> {
    if path.contains("1-video-timeline.json") || path.contains("video-timeline") {
        Some("video")
    } else if path.contains("2-game-build.json") || path.contains("game-build") {
        Some("game")
    } else if path.contains("3-interactive-cues.json") || path.contains("interactive-cues") {
        Some("interactive_art")
    } else {
        None
    }
}

fn output_deliverable_status(asset: Option<&AssetSnapshot>, file_found: bool) -> String {
    match (asset, file_found) {
        (None, _) => "missing".to_string(),
        (Some(_), true) => "ready".to_string(),
        (Some(_), false) => "indexed_missing_file".to_string(),
    }
}

fn read_manifest_json(path: &str) -> Option<Value> {
    fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
}

fn manifest_metrics(target: &str, manifest: &Value) -> Vec<OutputManifestMetric> {
    let mut metrics = match target {
        "video" => video_manifest_metrics(manifest),
        "game" => game_manifest_metrics(manifest),
        "interactive_art" => interactive_manifest_metrics(manifest),
        _ => Vec::new(),
    };
    metrics.extend(execution_result_metrics(manifest));
    metrics
}

fn video_manifest_metrics(manifest: &Value) -> Vec<OutputManifestMetric> {
    let duration = manifest
        .pointer("/timeline/duration_ms")
        .and_then(Value::as_u64)
        .map(|duration_ms| format!("{:.1}s", duration_ms as f64 / 1000.0));
    let fps = manifest.pointer("/timeline/fps").and_then(Value::as_u64);
    let tracks = manifest
        .pointer("/timeline/tracks")
        .and_then(Value::as_array)
        .map(Vec::len);
    let transcode = [
        manifest
            .pointer("/transcode/container")
            .and_then(Value::as_str),
        manifest.pointer("/transcode/codec").and_then(Value::as_str),
        manifest
            .pointer("/transcode/resolution")
            .and_then(Value::as_str),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" ");

    optional_metrics([
        duration.map(|value| metric("duration", &value)),
        fps.map(|value| metric("fps", &value.to_string())),
        tracks.map(|value| metric("tracks", &value.to_string())),
        (!transcode.is_empty()).then(|| metric("transcode", &transcode)),
    ])
}

fn game_manifest_metrics(manifest: &Value) -> Vec<OutputManifestMetric> {
    let platforms = manifest
        .pointer("/build/platforms")
        .and_then(Value::as_array)
        .map(|values| join_string_array(values))
        .filter(|value| !value.is_empty());

    optional_metrics([
        manifest
            .get("engine")
            .and_then(Value::as_str)
            .map(|value| metric("engine", value)),
        manifest
            .get("level")
            .and_then(Value::as_str)
            .map(|value| metric("level", value)),
        manifest
            .pointer("/viewport/mode")
            .and_then(Value::as_str)
            .map(|value| metric("viewport", value)),
        platforms.map(|value| metric("platforms", &value)),
    ])
}

fn interactive_manifest_metrics(manifest: &Value) -> Vec<OutputManifestMetric> {
    let cues = manifest
        .get("cue_graph")
        .and_then(Value::as_array)
        .map(Vec::len);
    let interfaces = manifest
        .get("device_interfaces")
        .and_then(Value::as_array)
        .map(|values| join_string_array(values))
        .filter(|value| !value.is_empty());
    let controls = manifest
        .pointer("/cue_graph/0/controls")
        .and_then(Value::as_array)
        .map(|values| join_string_array(values))
        .filter(|value| !value.is_empty());

    optional_metrics([
        manifest
            .get("runtime")
            .and_then(Value::as_str)
            .map(|value| metric("runtime", value)),
        cues.map(|value| metric("cues", &value.to_string())),
        interfaces.map(|value| metric("interfaces", &value)),
        controls.map(|value| metric("routes", &value)),
    ])
}

fn execution_result_metrics(manifest: &Value) -> Vec<OutputManifestMetric> {
    let Some(result) = manifest.get("execution_result") else {
        return Vec::new();
    };
    let artifact_count = result
        .get("artifacts")
        .and_then(Value::as_array)
        .map(Vec::len);
    optional_metrics([
        result
            .get("status")
            .and_then(Value::as_str)
            .map(|value| metric("execution", value)),
        result
            .get("runtime")
            .and_then(Value::as_str)
            .map(|value| metric("runtime_result", value)),
        result
            .get("adapter_id")
            .and_then(Value::as_str)
            .map(|value| metric("adapter", value)),
        artifact_count.map(|value| metric("artifacts", &value.to_string())),
        result
            .get("message")
            .and_then(Value::as_str)
            .map(|value| metric("message", value)),
    ])
}

fn fallback_deliverable_metrics(
    asset: Option<&AssetSnapshot>,
    descriptor: &OutputDescriptor,
) -> Vec<OutputManifestMetric> {
    match asset {
        Some(asset) => vec![
            metric("status", &asset.status),
            metric("asset", &asset.name),
        ],
        None => vec![
            metric("expected", descriptor.expected_file),
            metric("status", "not generated"),
        ],
    }
}

fn optional_metrics<const N: usize>(
    metrics: [Option<OutputManifestMetric>; N],
) -> Vec<OutputManifestMetric> {
    metrics.into_iter().flatten().collect()
}

fn join_string_array(values: &[Value]) -> String {
    values
        .iter()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>()
        .join(", ")
}

fn write_json(path: &Path, value: &serde_json::Value) -> Result<()> {
    fs::write(
        path,
        serde_json::to_string_pretty(value).context("serialize output manifest")?,
    )
    .with_context(|| format!("write output manifest {}", path.display()))
}

fn path_string(path: PathBuf) -> String {
    path.to_string_lossy().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_three_output_manifests() {
        let root =
            std::env::temp_dir().join(format!("pool-output-package-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(root.join("deliverables")).unwrap();
        let request = OutputPackageRequest {
            project_slug: "demo".to_string(),
            node_id: None,
            output_dir: root.to_string_lossy().to_string(),
            title: "Deliver demo".to_string(),
            source_assets: vec!["worlds/demo/output/1-world.glb".to_string()],
            duration_ms: 8_000,
        };

        let package = write_output_manifests(&root.join("deliverables"), &request).unwrap();
        let paths = package.local_paths;

        assert_eq!(paths.len(), 3);
        assert_eq!(package.manifests.len(), 3);
        assert_eq!(package.manifests[0].target, "video");
        assert_eq!(package.manifests[1].target, "game");
        assert_eq!(package.manifests[2].target, "interactive_art");
        assert!(paths[0].ends_with("1-video-timeline.json"));
        assert!(paths[1].ends_with("2-game-build.json"));
        assert!(paths[2].ends_with("3-interactive-cues.json"));
        assert!(Path::new(&paths[0]).exists());
        assert!(fs::read_to_string(&paths[0])
            .unwrap()
            .contains("\"target\": \"video\""));
        assert!(fs::read_to_string(&paths[1])
            .unwrap()
            .contains("\"target\": \"game\""));
        assert!(fs::read_to_string(&paths[2])
            .unwrap()
            .contains("\"target\": \"interactive_art\""));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn runner_indexes_output_manifests_and_events() {
        let root =
            std::env::temp_dir().join(format!("pool-output-runner-{}", uuid::Uuid::new_v4()));
        let repository = RuntimeRepository::in_memory().unwrap();
        repository.migrate().unwrap();
        let runner = OutputPackageRunner::new(&repository);

        let report = runner
            .run(OutputPackageRequest {
                project_slug: "demo".to_string(),
                node_id: Some("outputs".to_string()),
                output_dir: root.to_string_lossy().to_string(),
                title: "Deliver demo".to_string(),
                source_assets: vec!["worlds/demo/output/1-world.glb".to_string()],
                duration_ms: 8_000,
            })
            .unwrap();

        assert_eq!(report.status, TaskStatus::Succeeded);
        assert_eq!(report.assets.len(), 3);
        assert_eq!(report.manifests.len(), 3);
        assert_eq!(
            report.manifests[0].primary_runtime,
            "DaVinci Resolve / FFmpeg"
        );
        assert_eq!(
            repository.task_snapshot(&report.task_id).unwrap().node_id,
            Some("outputs".to_string())
        );
        assert_eq!(repository.table_count("tasks").unwrap(), 1);
        assert_eq!(repository.table_count("assets").unwrap(), 3);
        assert_eq!(repository.table_count("workflow_events").unwrap(), 2);

        let snapshot = repository.snapshot(Some("demo")).unwrap();
        let catalog = output_package_catalog_resource(&snapshot);
        assert_eq!(catalog.summary.total_targets, 3);
        assert_eq!(catalog.summary.ready_targets, 3);
        assert_eq!(catalog.deliverables[0].target, "video");
        assert!(catalog.deliverables[0].manifest_found);
        assert!(catalog.deliverables[0]
            .metrics
            .iter()
            .any(|metric| metric.label == "transcode"));

        fs::remove_file(&report.local_paths[0]).unwrap();
        let stale_catalog = output_package_catalog_resource(&snapshot);
        assert_eq!(stale_catalog.summary.ready_targets, 2);
        assert_eq!(stale_catalog.deliverables[0].status, "indexed_missing_file");
        assert_eq!(stale_catalog.summary.local_file_failures.len(), 1);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn runner_records_output_execution_result_into_manifest() {
        let root =
            std::env::temp_dir().join(format!("pool-output-result-{}", uuid::Uuid::new_v4()));
        let repository = RuntimeRepository::in_memory().unwrap();
        repository.migrate().unwrap();
        let runner = OutputPackageRunner::new(&repository);

        let report = runner
            .run(OutputPackageRequest {
                project_slug: "demo".to_string(),
                node_id: Some("outputs".to_string()),
                output_dir: root.to_string_lossy().to_string(),
                title: "Deliver demo".to_string(),
                source_assets: vec!["worlds/demo/output/1-world.glb".to_string()],
                duration_ms: 8_000,
            })
            .unwrap();
        let result = runner
            .record_result(OutputDeliverableResultRequest {
                project_slug: "demo".to_string(),
                node_id: Some("outputs".to_string()),
                target: "video".to_string(),
                local_path: None,
                status: "succeeded".to_string(),
                runtime: Some("DaVinci Resolve".to_string()),
                adapter_id: Some("resolve".to_string()),
                software_action_id: Some("action-resolve".to_string()),
                message: Some("timeline rendered".to_string()),
                artifacts: vec!["worlds/demo/output/final.mp4".to_string()],
                metrics: vec![metric("frames", "192")],
                verification: Some(json!({ "checksum": "abc" })),
            })
            .unwrap();

        assert_eq!(result.status, TaskStatus::Succeeded);
        assert_eq!(result.target, "video");
        assert_eq!(result.local_path, report.local_paths[0]);
        assert_eq!(result.manifest["execution_result"]["adapter_id"], "resolve");
        assert_eq!(
            result.manifest["execution_history"][0]["message"],
            "timeline rendered"
        );
        assert!(result.catalog.deliverables[0]
            .metrics
            .iter()
            .any(|metric| metric.label == "execution" && metric.value == "succeeded"));
        assert_eq!(
            repository
                .task_snapshot(&result.task_id)
                .unwrap()
                .provider_id,
            Some("output-package-result".to_string())
        );
        assert_eq!(repository.table_count("tasks").unwrap(), 2);
        assert_eq!(repository.table_count("workflow_events").unwrap(), 3);

        fs::remove_dir_all(root).unwrap();
    }
}
