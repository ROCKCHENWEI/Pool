use anyhow::{Context, Result};
use pool_core::{
    build_default_content_burst_plan, materialize_project_envelope, runtime_prd_readiness_resource,
    RuntimeHttpConfig, RuntimeHttpServer, RuntimeRepository,
};
use serde_json::{json, Value};
use std::path::PathBuf;

fn main() -> Result<()> {
    let output_root = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/desktop-vision-trace-smoke"));
    std::fs::create_dir_all(&output_root)
        .with_context(|| format!("create output root {}", output_root.display()))?;

    let db_path = output_root.join("pool-runtime.sqlite");
    let repository = RuntimeRepository::open(&db_path)?;
    repository.migrate()?;
    if repository.stats()?.projects == 0 {
        let plan = build_default_content_burst_plan("demo", "Pool desktop vision trace smoke");
        repository.persist_plan(&plan)?;
        materialize_project_envelope(&output_root, &plan)?;
    }
    drop(repository);

    let server = RuntimeHttpServer::new(
        RuntimeHttpConfig::new(&db_path)
            .with_project_slug("demo")
            .with_bind_addr("127.0.0.1:4788"),
    );
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
            "task_title": "TouchDesigner desktop vision trace smoke",
            "payload_json": {
                "control_dir": control_dir,
                "instruction": "find TouchDesigner perform mode and trigger cue 1",
                "target_window": "TouchDesigner",
                "visual_targets": ["Perform", "Cue 1", "Output"]
            },
            "requires_confirmation": false,
            "evidence_json": {
                "source": "run_desktop_vision_trace_smoke",
                "control_profile": "desktop_recognition",
                "local_trace_smoke": true,
                "external_visual_model": false
            }
        })
        .to_string(),
    )?;
    let action_value: Value = serde_json::from_str(&action_response.body)
        .context("parse desktop software action response")?;
    let action_id = action_value
        .pointer("/report/action_id")
        .and_then(Value::as_str)
        .context("desktop software action id")?;
    let task_id = action_value
        .pointer("/report/task_id")
        .and_then(Value::as_str)
        .context("desktop software task id")?;

    let trace_path = control_dir.join("1-touchdesigner-vision-trace.json");
    let trace = json!({
        "schema": "pool.desktop_vision_trace.v1",
        "source": "run_desktop_vision_trace_smoke",
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
            "message": "desktop vision trace smoke resolved TouchDesigner cue target",
            "artifacts": [trace_path],
            "screen_trace_path": trace_path,
            "verification": {
                "local_first": true,
                "trace_schema": "pool.desktop_vision_trace.v1"
            },
            "result": {
                "controller": "run_desktop_vision_trace_smoke",
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
    let result_value: Value = serde_json::from_str(&result_response.body)
        .context("parse desktop result callback response")?;

    let repository = RuntimeRepository::open(&db_path)?;
    repository.migrate()?;
    let snapshot = repository.snapshot(Some("demo"))?;
    let readiness = runtime_prd_readiness_resource(&snapshot)?;

    println!("db={}", db_path.display());
    println!("output_root={}", output_root.display());
    println!("software_action_id={action_id}");
    println!("task_id={task_id}");
    println!("trace_path={}", trace_path.display());
    println!(
        "result_status={}",
        result_value["software_action"]["verification"]["desktop_recognition_status"]
    );
    println!("prd_summary={}", readiness["summary"]);
    if let Some(requirement) = readiness
        .get("requirements")
        .and_then(Value::as_array)
        .and_then(|items| {
            items
                .iter()
                .find(|item| item.get("id").and_then(Value::as_str) == Some("production_hardening"))
        })
    {
        println!("production_hardening_status={}", requirement["status"]);
        println!(
            "desktop_vision_evidence={}",
            requirement["evidence"]["desktop_vision_evidence"]
        );
        println!("production_hardening_gaps={}", requirement["gaps"]);
    }

    Ok(())
}
