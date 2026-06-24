use anyhow::{bail, Context, Result};
use pool_core::{
    runtime_core_architecture_readiness_resource, runtime_prd_readiness_resource,
    spawn_provider_gateway_mock, ContentBurstAgentMode, ContentBurstProviderMode,
    ContentBurstRunRequest, ContentBurstRunner, ContentBurstSoftwareMode,
    OutputDeliverableResultRequest, OutputManifestMetric, OutputPackageRunner, RuntimeHttpConfig,
    RuntimeHttpResponse, RuntimeHttpServer, RuntimeRepository,
};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProviderFamily {
    Media,
    ThreeDgs,
}

#[derive(Debug, Clone, Copy)]
struct ProviderEvidenceTarget {
    provider_id: &'static str,
    family: ProviderFamily,
}

const PROVIDER_TARGETS: &[ProviderEvidenceTarget] = &[
    ProviderEvidenceTarget {
        provider_id: "midjourney",
        family: ProviderFamily::Media,
    },
    ProviderEvidenceTarget {
        provider_id: "nano-banana-pro",
        family: ProviderFamily::Media,
    },
    ProviderEvidenceTarget {
        provider_id: "suno",
        family: ProviderFamily::Media,
    },
    ProviderEvidenceTarget {
        provider_id: "worldlabs-marble",
        family: ProviderFamily::ThreeDgs,
    },
    ProviderEvidenceTarget {
        provider_id: "tripo-splat",
        family: ProviderFamily::ThreeDgs,
    },
    ProviderEvidenceTarget {
        provider_id: "sam-3d",
        family: ProviderFamily::ThreeDgs,
    },
    ProviderEvidenceTarget {
        provider_id: "spark-3dgs",
        family: ProviderFamily::ThreeDgs,
    },
    ProviderEvidenceTarget {
        provider_id: "qunhe-3d",
        family: ProviderFamily::ThreeDgs,
    },
];

const SOFTWARE_TARGETS: &[&str] = &[
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

fn main() -> Result<()> {
    let options = SmokeOptions::from_args()?;
    let output_root = options.output_root;
    std::fs::create_dir_all(&output_root)
        .with_context(|| format!("create output root {}", output_root.display()))?;

    let db_path = output_root.join("pool-runtime.sqlite");
    let repository = RuntimeRepository::open(&db_path)?;
    repository.migrate()?;

    let run_report = ContentBurstRunner::new(&repository).run(ContentBurstRunRequest {
        project_slug: "demo".to_string(),
        output_root: output_root.to_string_lossy().to_string(),
        title: "Pool PRD readiness local smoke".to_string(),
        prompt: "verify local creative input to 3DGS to Unreal to three output targets".to_string(),
        source_inputs: vec!["worlds/demo/source/0-reference.png".to_string()],
        duration_ms: 12_000,
        agent_mode: ContentBurstAgentMode::Stage,
        three_dgs_mode: ContentBurstProviderMode::Mock,
        unreal_mode: ContentBurstSoftwareMode::Mock,
        ..ContentBurstRunRequest::new("demo", output_root.to_string_lossy().to_string())
    })?;

    let output_runner = OutputPackageRunner::new(&repository);
    for target in ["video", "game", "interactive_art"] {
        let local_path = run_report
            .output_report
            .manifests
            .iter()
            .find(|manifest| manifest.target == target)
            .map(|manifest| manifest.local_path.clone());
        let (runtime, adapter_id, artifacts, metrics) =
            output_result_evidence(target, &run_report.software_report.action_id);
        output_runner.record_result(OutputDeliverableResultRequest {
            project_slug: "demo".to_string(),
            node_id: Some("outputs".to_string()),
            target: target.to_string(),
            local_path,
            status: "succeeded".to_string(),
            runtime: Some(runtime.to_string()),
            adapter_id: Some(adapter_id.to_string()),
            software_action_id: Some(run_report.software_report.action_id.clone()),
            message: Some(format!("{target} local PRD smoke verified")),
            artifacts,
            metrics,
            verification: Some(json!({
                "source": "run_prd_readiness_smoke",
                "local_first": true,
                "note": "Local smoke proves Pool runtime bookkeeping and handoff manifests, not vendor production execution.",
            })),
        })?;
    }
    drop(output_runner);
    drop(repository);

    let mock_endpoint = spawn_provider_gateway_mock(128)?;
    let server = RuntimeHttpServer::new(
        RuntimeHttpConfig::new(&db_path)
            .with_project_slug("demo")
            .with_bind_addr("127.0.0.1:4788"),
    );
    let provider_evidence =
        run_provider_local_evidence(&server, &output_root, mock_endpoint.as_str())?;
    let software_evidence = run_software_local_evidence(&server)?;
    let desktop_evidence = run_desktop_trace_evidence(&server, &output_root)?;
    let production_closeout = if options.with_production_evidence {
        Some(run_production_evidence_closeout(&server, &output_root)?)
    } else {
        None
    };

    let repository = RuntimeRepository::open(&db_path)?;
    repository.migrate()?;
    let snapshot = repository.snapshot(Some("demo"))?;
    drop(repository);
    let core_architecture = runtime_core_architecture_readiness_resource(&snapshot)?;
    ensure_true(
        &core_architecture,
        "/architecture_gate/ready_for_core_architecture",
        "core architecture ready_for_core_architecture",
    )?;
    let core_gate = parse_response(
        "core architecture gate",
        200,
        server.handle_path("/api/core-architecture-gate?project=demo&require_ready=true")?,
    )?;
    ensure_true(
        &core_gate,
        "/architecture_gate/ready_for_core_architecture",
        "core architecture hard gate",
    )?;
    let core_package = parse_response(
        "core architecture package",
        201,
        server.handle_request_with_body(
            "POST",
            "/api/core-architecture-package",
            &json!({
                "project_slug": "demo",
                "node_id": "agent",
                "title": "Pool core architecture local proof package",
                "output_dir": output_root.join("worlds/demo/output"),
                "source": "run_prd_readiness_smoke",
                "include_snapshot": true
            })
            .to_string(),
        )?,
    )?;
    ensure_true(
        &core_package,
        "/report/ready_for_core_architecture",
        "core architecture package ready_for_core_architecture",
    )?;
    let readiness = runtime_prd_readiness_resource(&snapshot)?;
    if options.with_production_evidence
        && readiness.pointer("/summary/ready").and_then(Value::as_u64) != Some(10)
    {
        bail!(
            "expected production evidence mode to make all PRD requirements ready, got {}",
            readiness["summary"]
        );
    }

    println!("db={}", db_path.display());
    println!("output_root={}", output_root.display());
    println!(
        "production_evidence_mode={}",
        options.with_production_evidence
    );
    println!("workflow_id={}", run_report.workflow_id);
    println!("agent_mode={:?}", run_report.agent_mode);
    println!("three_dgs_mode={:?}", run_report.three_dgs_mode);
    println!("unreal_mode={:?}", run_report.unreal_mode);
    println!("provider_status={:?}", run_report.provider_report.status);
    println!("software_status={:?}", run_report.software_report.status);
    println!("output_status={:?}", run_report.output_report.status);
    println!(
        "software_action_id={}",
        run_report.software_report.action_id
    );
    println!(
        "deliverables={}",
        run_report.output_report.local_paths.join(",")
    );
    println!(
        "provider_evidence=succeeded:{},failed:{}",
        provider_evidence.succeeded, provider_evidence.failed
    );
    println!(
        "software_evidence=succeeded:{},failed:{}",
        software_evidence.succeeded, software_evidence.failed
    );
    println!(
        "desktop_trace_evidence=succeeded:{},failed:{}",
        desktop_evidence.succeeded, desktop_evidence.failed
    );
    if let Some(summary) = production_closeout {
        println!(
            "production_closeout=writes:{},ready_for_import:{},input_bundles:{},imported_providers:{},imported_software:{},imported_desktop_vision:{}",
            summary.writes,
            summary.ready_for_import,
            summary.input_bundles,
            summary.imported_providers,
            summary.imported_software_actions,
            summary.imported_desktop_vision
        );
    }
    println!("prd_summary={}", readiness["summary"]);
    println!("core_architecture_summary={}", core_architecture["summary"]);
    println!(
        "core_architecture_gate=status:{},ready_for_core_architecture:{}",
        core_architecture["architecture_gate"]["status"],
        core_architecture["architecture_gate"]["ready_for_core_architecture"]
    );
    println!(
        "core_architecture_package={}",
        core_package["report"]["manifest_path"]
    );
    println!(
        "completion_gate=status:{},ready_for_completion:{}",
        readiness["completion_gate"]["status"],
        readiness["completion_gate"]["ready_for_completion"]
    );
    print_requirement_statuses(&readiness);

    Ok(())
}

#[derive(Debug)]
struct SmokeOptions {
    output_root: PathBuf,
    with_production_evidence: bool,
}

impl SmokeOptions {
    fn from_args() -> Result<Self> {
        let mut output_root = None;
        let mut with_production_evidence = false;

        for arg in std::env::args().skip(1) {
            match arg.as_str() {
                "--with-production-evidence" | "--production-evidence" => {
                    with_production_evidence = true;
                }
                "--help" | "-h" => {
                    println!(
                        "Usage: run_prd_readiness_smoke [--with-production-evidence] [output-root]"
                    );
                    std::process::exit(0);
                }
                value if value.starts_with('-') => bail!("unknown option: {value}"),
                value => {
                    if output_root.is_some() {
                        bail!("run_prd_readiness_smoke accepts at most one output-root");
                    }
                    output_root = Some(PathBuf::from(value));
                }
            }
        }

        Ok(Self {
            output_root: output_root
                .unwrap_or_else(|| PathBuf::from("target/prd-readiness-runner")),
            with_production_evidence,
        })
    }
}

#[derive(Debug, Default)]
struct LocalEvidenceSummary {
    succeeded: usize,
    failed: usize,
}

fn run_provider_local_evidence(
    server: &RuntimeHttpServer,
    output_root: &std::path::Path,
    endpoint: &str,
) -> Result<LocalEvidenceSummary> {
    let mut summary = LocalEvidenceSummary::default();
    for target in PROVIDER_TARGETS {
        let output_dir = output_root
            .join("worlds/demo/output/provider-evidence")
            .join(target.provider_id);
        let body = json!({
            "project_slug": "demo",
            "provider_id": target.provider_id,
            "execution_mode": "gateway",
            "endpoint": endpoint,
            "task_title": format!("{} PRD local evidence", target.provider_id),
            "prompt": provider_evidence_prompt(target),
            "input_paths": ["worlds/demo/source/0-reference.png"],
            "output_dir": output_dir.to_string_lossy(),
            "requires_approval": false,
            "evidence_json": {
                "source": "run_prd_readiness_smoke",
                "family": match target.family {
                    ProviderFamily::Media => "ai_media",
                    ProviderFamily::ThreeDgs => "3dgs",
                },
                "evidence_mode": "local_mock_gateway",
                "production_upstream": false,
                "local_mock_gateway": true
            }
        });
        let response =
            server.handle_request_with_body("POST", "/api/provider-runs", &body.to_string())?;
        let value: Value = serde_json::from_str(&response.body).with_context(|| {
            format!(
                "parse provider evidence response for {}",
                target.provider_id
            )
        })?;
        let status = value
            .pointer("/report/status")
            .and_then(Value::as_str)
            .or_else(|| value.get("error").and_then(Value::as_str))
            .unwrap_or("unknown");
        if response.status_code < 400 && status == "Succeeded" {
            summary.succeeded += 1;
        } else {
            summary.failed += 1;
        }
    }
    Ok(summary)
}

fn provider_evidence_prompt(target: &ProviderEvidenceTarget) -> String {
    match target.family {
        ProviderFamily::Media => format!(
            "Pool PRD local evidence run for {}. Generate one local media output for audit.",
            target.provider_id
        ),
        ProviderFamily::ThreeDgs => format!(
            "Pool PRD local evidence run for {}. Convert reference input to image-blaster indexed 3DGS outputs.",
            target.provider_id
        ),
    }
}

fn run_software_local_evidence(server: &RuntimeHttpServer) -> Result<LocalEvidenceSummary> {
    let mut summary = LocalEvidenceSummary::default();
    for adapter_id in SOFTWARE_TARGETS {
        let body = software_action_body(adapter_id);
        let response =
            server.handle_request_with_body("POST", "/api/software-actions", &body.to_string())?;
        let value: Value = serde_json::from_str(&response.body)
            .with_context(|| format!("parse software evidence response for {adapter_id}"))?;
        let status = value
            .pointer("/report/status")
            .and_then(Value::as_str)
            .or_else(|| value.get("error").and_then(Value::as_str))
            .unwrap_or("unknown");
        if response.status_code < 400 && status == "Succeeded" {
            summary.succeeded += 1;
        } else {
            summary.failed += 1;
        }
    }
    Ok(summary)
}

fn software_action_body(adapter_id: &str) -> Value {
    if adapter_id == "unreal" {
        return json!({
            "project_slug": "demo",
            "adapter_id": "unreal",
            "action_kind": "CreateScene",
            "priority": "ApiMcp",
            "task_title": "unreal PRD local evidence",
            "payload_json": {
                "level": "demo_prd_local_evidence",
                "assets": ["worlds/demo/output/1-world.glb"]
            },
            "requires_confirmation": false,
            "evidence_json": software_evidence_json(adapter_id, "api_mcp", true),
        });
    }

    json!({
        "project_slug": "demo",
        "adapter_id": adapter_id,
        "action_kind": "ExecuteCli",
        "priority": "SkillsCli",
        "task_title": format!("{adapter_id} PRD local evidence"),
        "payload_json": {
            "command": format!("/bin/echo pool-prd-local-software-evidence-{adapter_id}"),
            "allowed_commands": ["/bin/echo", "echo"],
            "timeout_ms": 2000,
            "max_output_bytes": 2048,
            "artifacts": [format!("software-evidence://{adapter_id}/prd-local-cli")]
        },
        "requires_confirmation": false,
        "evidence_json": software_evidence_json(adapter_id, "skills_cli", false),
    })
}

fn software_evidence_json(
    adapter_id: &str,
    control_profile: &str,
    local_mock_software: bool,
) -> Value {
    json!({
        "source": "run_prd_readiness_smoke",
        "adapter_id": adapter_id,
        "control_profile": control_profile,
        "evidence_mode": "local_control_profile",
        "production_software": false,
        "local_mock_software": local_mock_software,
    })
}

fn run_desktop_trace_evidence(
    server: &RuntimeHttpServer,
    output_root: &std::path::Path,
) -> Result<LocalEvidenceSummary> {
    let mut summary = LocalEvidenceSummary::default();
    let control_dir = output_root.join("worlds/demo/output/control/desktop-recognition");
    std::fs::create_dir_all(&control_dir)
        .with_context(|| format!("create control dir {}", control_dir.display()))?;

    let action_response = server.handle_request_with_body(
        "POST",
        "/api/software-actions",
        &json!({
            "project_slug": "demo",
            "node_id": "interactive_art",
            "adapter_id": "touchdesigner",
            "action_kind": "RunViewport",
            "priority": "DesktopRecognition",
            "task_title": "TouchDesigner PRD desktop vision trace evidence",
            "payload_json": {
                "control_dir": control_dir,
                "instruction": "find TouchDesigner perform mode and trigger cue 1",
                "target_window": "TouchDesigner",
                "visual_targets": ["Perform", "Cue 1", "Output"]
            },
            "requires_confirmation": false,
            "evidence_json": {
                "source": "run_prd_readiness_smoke",
                "control_profile": "desktop_recognition",
                "local_trace_smoke": true,
                "external_visual_model": false
            }
        })
        .to_string(),
    )?;
    let action_value: Value = serde_json::from_str(&action_response.body)
        .context("parse desktop software action response")?;
    let Some(action_id) = action_value
        .pointer("/report/action_id")
        .and_then(Value::as_str)
    else {
        summary.failed += 1;
        return Ok(summary);
    };
    let Some(task_id) = action_value
        .pointer("/report/task_id")
        .and_then(Value::as_str)
    else {
        summary.failed += 1;
        return Ok(summary);
    };

    let trace_path = control_dir.join("1-touchdesigner-prd-vision-trace.json");
    let trace = json!({
        "schema": "pool.desktop_vision_trace.v1",
        "source": "run_prd_readiness_smoke",
        "external_visual_model": false,
        "screen": {
            "target_window": "TouchDesigner",
            "width": 1440,
            "height": 900
        },
        "detections": [
            {"label": "Perform", "center": {"x": 220, "y": 84}, "confidence": 0.97},
            {"label": "Cue 1", "center": {"x": 512, "y": 420}, "confidence": 0.94},
            {"label": "Output", "bbox": {"x": 980, "y": 220, "width": 180, "height": 120}, "confidence": 0.91}
        ]
    });
    std::fs::write(&trace_path, serde_json::to_string_pretty(&trace)?)
        .with_context(|| format!("write trace {}", trace_path.display()))?;

    let result_response = server.handle_request_with_body(
        "POST",
        "/api/desktop-recognition/results",
        &json!({
            "software_action_id": action_id,
            "task_id": task_id,
            "status": "succeeded",
            "message": "PRD desktop vision trace evidence resolved TouchDesigner cue target",
            "artifacts": [trace_path],
            "screen_trace_path": trace_path,
            "verification": {
                "local_first": true,
                "trace_schema": "pool.desktop_vision_trace.v1"
            },
            "result": {
                "controller": "run_prd_readiness_smoke",
                "mode": "vision_trace",
                "vision_trace_path": trace_path,
                "resolved_target": {
                    "label": "Cue 1",
                    "x": 512,
                    "y": 420
                },
                "external_visual_model": false
            }
        })
        .to_string(),
    )?;
    if result_response.status_code < 400 {
        summary.succeeded += 1;
    } else {
        summary.failed += 1;
    }
    Ok(summary)
}

#[derive(Debug)]
struct ProductionCloseoutSummary {
    writes: i64,
    ready_for_import: bool,
    input_bundles: u64,
    imported_providers: u64,
    imported_software_actions: u64,
    imported_desktop_vision: u64,
}

fn run_production_evidence_closeout(
    server: &RuntimeHttpServer,
    output_root: &Path,
) -> Result<ProductionCloseoutSummary> {
    let mut bundle: Value = serde_json::from_str(include_str!(
        "../../docs/examples/production-evidence-bundle.example.json"
    ))
    .context("parse production evidence example bundle")?;
    bundle["project_slug"] = json!("demo");
    bundle["source"] = json!("run-prd-readiness-smoke-production-evidence");
    materialize_production_bundle_files(output_root, &mut bundle)?;

    let provider_bundle = json!({
        "project_slug": "demo",
        "source": "prd-readiness-provider-closeout",
        "providers": bundle.get("providers").cloned().unwrap_or_else(|| json!([])),
    });
    let software_bundle = json!({
        "project_slug": "demo",
        "source": "prd-readiness-software-closeout",
        "software_actions": bundle.get("software_actions").cloned().unwrap_or_else(|| json!([])),
    });
    let desktop_bundle = json!({
        "project_slug": "demo",
        "source": "prd-readiness-desktop-closeout",
        "desktop_vision": bundle.get("desktop_vision").cloned().unwrap_or_else(|| json!([])),
    });

    let evidence_dir = output_root.join("worlds/demo/output/control/production-evidence");
    write_json_file(
        &evidence_dir.join("provider-production-evidence.bundle.json"),
        &provider_bundle,
    )?;
    write_json_file(
        &evidence_dir.join("software-production-evidence.bundle.json"),
        &software_bundle,
    )?;
    write_json_file(
        &evidence_dir.join("desktop-vision-production-evidence.bundle.json"),
        &desktop_bundle,
    )?;

    let validate_body = json!({
        "project_slug": "demo",
        "source": "run-prd-readiness-smoke-production-evidence",
        "bundles": [provider_bundle, software_bundle, desktop_bundle],
    })
    .to_string();
    let closeout = parse_response(
        "closeout production evidence",
        200,
        server.handle_request_with_body(
            "POST",
            "/api/production-evidence/closeout",
            &validate_body,
        )?,
    )?;
    ensure_true(
        &closeout,
        "/validation/artifact_files/complete",
        "production closeout artifact_files.complete",
    )?;
    ensure_true(
        &closeout,
        "/validation/coverage/complete",
        "production closeout coverage.complete",
    )?;
    ensure_true(
        &closeout,
        "/ready_for_import",
        "production closeout ready_for_import",
    )?;
    let writes = closeout
        .pointer("/writes")
        .and_then(Value::as_i64)
        .unwrap_or(-1);
    if writes != 0 {
        bail!("production closeout preflight wrote to runtime: {writes}");
    }

    let import_body = json!({
        "project_slug": "demo",
        "source": "run-prd-readiness-smoke-production-evidence",
        "import": true,
        "bundles": [closeout["merge"]["bundle"].clone()],
    })
    .to_string();
    let imported = parse_response(
        "closeout production evidence import",
        201,
        server.handle_request_with_body(
            "POST",
            "/api/production-evidence/closeout",
            &import_body,
        )?,
    )?;
    ensure_true(
        &imported,
        "/import/artifact_files/complete",
        "production closeout import artifact_files.complete",
    )?;
    ensure_true(
        &imported,
        "/import/coverage/complete",
        "production closeout import coverage.complete",
    )?;

    Ok(ProductionCloseoutSummary {
        writes,
        ready_for_import: closeout
            .pointer("/ready_for_import")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        input_bundles: closeout
            .pointer("/merge/summary/input_bundles")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        imported_providers: imported
            .pointer("/import/summary/providers")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        imported_software_actions: imported
            .pointer("/import/summary/software_actions")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        imported_desktop_vision: imported
            .pointer("/import/summary/desktop_vision")
            .and_then(Value::as_u64)
            .unwrap_or(0),
    })
}

fn materialize_production_bundle_files(output_root: &Path, bundle: &mut Value) -> Result<()> {
    if let Some(providers) = bundle.get_mut("providers").and_then(Value::as_array_mut) {
        for provider in providers {
            let provider_id = provider
                .get("provider_id")
                .and_then(Value::as_str)
                .context("provider_id")?
                .to_string();
            let external_job_id = provider
                .get("external_job_id")
                .and_then(Value::as_str)
                .context("external_job_id")?
                .to_string();
            let original_artifact = provider
                .get("artifacts")
                .and_then(Value::as_array)
                .and_then(|items| items.first())
                .and_then(Value::as_str)
                .unwrap_or("1-provider-artifact.bin");
            let artifact_path = output_root
                .join("worlds/demo/output/production-evidence/providers")
                .join(&provider_id)
                .join(file_name_or_default(
                    original_artifact,
                    "1-provider-artifact.bin",
                ));
            write_text_file(
                &artifact_path,
                &format!("fixture artifact for {provider_id} {external_job_id}\n"),
            )?;
            let metadata_path = output_root
                .join("worlds/demo/output/production-evidence/providers")
                .join(&provider_id)
                .join("request-metadata.json");
            write_json_file(
                &metadata_path,
                &json!({
                    "schema": "pool.production_provider_metadata.v1",
                    "provider_id": provider_id,
                    "external_job_id": external_job_id,
                    "source": "run_prd_readiness_smoke",
                    "fixture": true,
                    "replace_with": "real external worker request/response metadata"
                }),
            )?;
            provider["artifacts"] = json!([path_string(&artifact_path)]);
            provider["metadata_path"] = json!(path_string(&metadata_path));
        }
    }

    if let Some(software_actions) = bundle
        .get_mut("software_actions")
        .and_then(Value::as_array_mut)
    {
        for item in software_actions {
            let adapter_id = item
                .get("adapter_id")
                .and_then(Value::as_str)
                .context("adapter_id")?
                .to_string();
            if let Some(artifacts) = item.get_mut("artifacts").and_then(Value::as_array_mut) {
                for artifact in artifacts {
                    let Some(path) = artifact.as_str() else {
                        continue;
                    };
                    if path.contains("://") || path.starts_with("pool://") {
                        continue;
                    }
                    let materialized = output_root.join(path);
                    write_text_file(
                        &materialized,
                        &format!("fixture software artifact for {adapter_id}\n"),
                    )?;
                    *artifact = json!(path_string(&materialized));
                }
            }
        }
    }

    if let Some(desktop_vision) = bundle
        .get_mut("desktop_vision")
        .and_then(Value::as_array_mut)
    {
        for item in desktop_vision {
            let external_action_id = item
                .get("external_action_id")
                .and_then(Value::as_str)
                .context("desktop external_action_id")?;
            let trace_path = output_root
                .join("worlds/demo/output/production-evidence/desktop-vision")
                .join("1-touchdesigner-trace.json");
            write_json_file(
                &trace_path,
                &json!({
                    "schema": "pool.desktop_vision_trace.v1",
                    "source": "run_prd_readiness_smoke",
                    "external_visual_model": true,
                    "external_action_id": external_action_id,
                    "detections": [
                        {"label": "Perform", "confidence": 0.98},
                        {"label": "Cue 1", "confidence": 0.96},
                        {"label": "Output", "confidence": 0.94}
                    ]
                }),
            )?;
            item["trace_path"] = json!(path_string(&trace_path));
            item["artifacts"] = json!([path_string(&trace_path)]);
        }
    }

    Ok(())
}

fn parse_response(
    label: &str,
    expected_status: u16,
    response: RuntimeHttpResponse,
) -> Result<Value> {
    let value: Value = serde_json::from_str(&response.body)
        .with_context(|| format!("parse {label} response body"))?;
    if response.status_code != expected_status {
        bail!(
            "{label} returned HTTP {}, expected {expected_status}: {}",
            response.status_code,
            value
        );
    }
    Ok(value)
}

fn ensure_true(value: &Value, pointer: &str, label: &str) -> Result<()> {
    if value.pointer(pointer).and_then(Value::as_bool) != Some(true) {
        bail!(
            "{label} is not true: {}",
            value.pointer(pointer).unwrap_or(&Value::Null)
        );
    }
    Ok(())
}

fn write_text_file(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create dir {}", parent.display()))?;
    }
    std::fs::write(path, content).with_context(|| format!("write {}", path.display()))
}

fn write_json_file(path: &Path, value: &Value) -> Result<()> {
    write_text_file(path, &serde_json::to_string_pretty(value)?)
}

fn file_name_or_default(path: &str, default: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(default)
        .to_string()
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn output_result_evidence(
    target: &str,
    software_action_id: &str,
) -> (
    &'static str,
    &'static str,
    Vec<String>,
    Vec<OutputManifestMetric>,
) {
    match target {
        "video" => (
            "Pool timeline/FFmpeg handoff smoke",
            "editing-suite",
            vec!["pool://deliverable/video".to_string()],
            vec![
                metric("timeline", "verified"),
                metric("transcode", "handoff"),
            ],
        ),
        "game" => (
            "Unreal mock viewport",
            "unreal",
            vec![
                "unreal://mock/viewport".to_string(),
                format!("pool://software-actions/{software_action_id}"),
            ],
            vec![metric("viewport", "mock_verified"), metric("fps", "60")],
        ),
        _ => (
            "TouchDesigner/MadMapper cue handoff smoke",
            "touchdesigner",
            vec!["pool://deliverable/interactive_art".to_string()],
            vec![
                metric("cue_graph", "verified"),
                metric("interfaces", "osc,midi,dmx"),
            ],
        ),
    }
}

fn metric(label: &str, value: &str) -> OutputManifestMetric {
    OutputManifestMetric {
        label: label.to_string(),
        value: value.to_string(),
    }
}

fn print_requirement_statuses(readiness: &Value) {
    let Some(requirements) = readiness.get("requirements").and_then(Value::as_array) else {
        return;
    };

    for requirement in requirements {
        let id = requirement
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let status = requirement
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let gaps = requirement
            .get("gaps")
            .and_then(Value::as_array)
            .map_or(0, Vec::len);
        println!("requirement={id} status={status} gaps={gaps}");
    }
}
