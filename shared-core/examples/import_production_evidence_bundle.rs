use anyhow::{bail, Context, Result};
use pool_core::{
    build_default_content_burst_plan, materialize_project_envelope, RuntimeHttpConfig,
    RuntimeHttpResponse, RuntimeHttpServer, RuntimeRepository,
};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn main() -> Result<()> {
    let output_root = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/production-evidence-import-smoke"));
    std::fs::create_dir_all(&output_root)
        .with_context(|| format!("create output root {}", output_root.display()))?;

    let db_path = output_root.join("pool-runtime.sqlite");
    let repository = RuntimeRepository::open(&db_path)?;
    repository.migrate()?;
    if repository.stats()?.projects == 0 {
        let plan = build_default_content_burst_plan("demo", "Pool production evidence import");
        repository.persist_plan(&plan)?;
        materialize_project_envelope(&output_root, &plan)?;
    }
    drop(repository);

    let mut bundle: Value = serde_json::from_str(include_str!(
        "../../docs/examples/production-evidence-bundle.example.json"
    ))
    .context("parse production evidence example bundle")?;
    bundle["project_slug"] = json!("demo");
    bundle["source"] = json!("production-evidence-import-smoke");
    materialize_bundle_files(&output_root, &mut bundle)?;

    let bundle_path = output_root.join("production-evidence-import.bundle.json");
    std::fs::write(&bundle_path, serde_json::to_string_pretty(&bundle)?)
        .with_context(|| format!("write bundle {}", bundle_path.display()))?;
    let body = serde_json::to_string(&bundle)?;

    let server = RuntimeHttpServer::new(
        RuntimeHttpConfig::new(&db_path)
            .with_project_slug("demo")
            .with_bind_addr("127.0.0.1:4788"),
    );
    seed_prd_baseline_evidence(&server, &output_root)?;

    let validation = parse_response(
        "validate production evidence",
        200,
        server.handle_request_with_body("POST", "/api/production-evidence/validate", &body)?,
    )?;
    ensure_true(
        &validation,
        "/artifact_files/complete",
        "validation artifact_files.complete",
    )?;
    ensure_true(
        &validation,
        "/coverage/complete",
        "validation coverage.complete",
    )?;
    if validation.pointer("/writes").and_then(Value::as_i64) != Some(0) {
        bail!("validation wrote to runtime: {}", validation["writes"]);
    }

    let imported = parse_response(
        "import production evidence",
        201,
        server.handle_request_with_body("POST", "/api/production-evidence", &body)?,
    )?;
    ensure_true(
        &imported,
        "/artifact_files/complete",
        "import artifact_files.complete",
    )?;
    ensure_true(&imported, "/coverage/complete", "import coverage.complete")?;
    let overall_status = imported
        .pointer("/prd_readiness/overall_status")
        .and_then(Value::as_str)
        .unwrap_or("missing");
    if overall_status != "ready" {
        bail!(
            "expected PRD readiness ready after full evidence import, got {overall_status}: {}",
            imported["prd_readiness"]
        );
    }
    if imported
        .pointer("/prd_readiness/summary/ready")
        .and_then(Value::as_u64)
        != Some(10)
    {
        bail!(
            "expected all 10 PRD requirements ready, got {}",
            imported["prd_readiness"]["summary"]
        );
    }

    println!("db={}", db_path.display());
    println!("output_root={}", output_root.display());
    println!("bundle_path={}", bundle_path.display());
    println!(
        "validation=writes:{},coverage_complete:{},artifact_files_complete:{}",
        validation["writes"],
        validation["coverage"]["complete"],
        validation["artifact_files"]["complete"]
    );
    println!(
        "imported=providers:{},software_actions:{},desktop_vision:{}",
        imported["summary"]["providers"],
        imported["summary"]["software_actions"],
        imported["summary"]["desktop_vision"]
    );
    println!("artifact_files={}", imported["artifact_files"]);
    println!("coverage={}", imported["coverage"]);
    println!("prd_summary={}", imported["prd_readiness"]["summary"]);
    println!(
        "overall_status={}",
        imported["prd_readiness"]["overall_status"]
    );

    Ok(())
}

fn seed_prd_baseline_evidence(server: &RuntimeHttpServer, output_root: &Path) -> Result<()> {
    let control_dir = output_root.join("worlds/demo/output/control/agent-sessions");
    parse_response(
        "stage Hermes session",
        201,
        server.handle_request_with_body(
            "POST",
            "/api/agent-sessions",
            &json!({
                "kind": "hermes",
                "project_slug": "demo",
                "control_dir": path_string(&control_dir),
                "title": "Production evidence import smoke",
                "instruction": "Prepare Pool production evidence import handoff and verify PRD readiness.",
                "allowed_tools": ["mcp", "pool-cli", "runtime"],
                "requires_confirmation": false
            })
            .to_string(),
        )?,
    )?;

    let output_dir = output_root.join("worlds/demo/output");
    parse_response(
        "create output package",
        201,
        server.handle_request_with_body(
            "POST",
            "/api/output-packages",
            &json!({
                "project_slug": "demo",
                "node_id": "outputs",
                "output_dir": path_string(&output_dir),
                "title": "Production evidence import output package",
                "source_assets": ["worlds/demo/output/production-evidence/providers/worldlabs-marble/1-world.glb"],
                "duration_ms": 12000
            })
            .to_string(),
        )?,
    )?;

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
        parse_response(
            &format!("record {target} output result"),
            201,
            server.handle_request_with_body(
                "POST",
                "/api/output-packages/results",
                &json!({
                    "project_slug": "demo",
                    "node_id": "outputs",
                    "target": target,
                    "status": "succeeded",
                    "runtime": runtime,
                    "adapter_id": adapter_id,
                    "message": format!("{target} production evidence smoke result"),
                    "artifacts": [artifact],
                    "metrics": [{"label": "evidence", "value": "production-import-smoke"}],
                    "verification": {
                        "source": "import_production_evidence_bundle",
                        "fixture": true
                    }
                })
                .to_string(),
            )?,
        )?;
    }

    Ok(())
}

fn materialize_bundle_files(output_root: &Path, bundle: &mut Value) -> Result<()> {
    if let Some(providers) = bundle.get_mut("providers").and_then(Value::as_array_mut) {
        for provider in providers {
            let provider_id = provider
                .get("provider_id")
                .and_then(Value::as_str)
                .context("provider_id")?;
            let external_job_id = provider
                .get("external_job_id")
                .and_then(Value::as_str)
                .context("external_job_id")?;
            let original_artifact = provider
                .get("artifacts")
                .and_then(Value::as_array)
                .and_then(|items| items.first())
                .and_then(Value::as_str)
                .unwrap_or("1-provider-artifact.bin");
            let artifact_path = output_root
                .join("worlds/demo/output/production-evidence/providers")
                .join(provider_id)
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
                .join(provider_id)
                .join("request-metadata.json");
            write_json_file(
                &metadata_path,
                &json!({
                    "schema": "pool.production_provider_metadata.v1",
                    "provider_id": provider_id,
                    "external_job_id": external_job_id,
                    "source": "import_production_evidence_bundle",
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
                    "source": "import_production_evidence_bundle",
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
