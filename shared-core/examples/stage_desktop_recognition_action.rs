use anyhow::Result;
use pool_core::{
    ControlPriority, DesktopRecognitionAdapter, RuntimeRepository, RuntimeTask, SoftwareActionKind,
    SoftwareActionRunner, SoftwareAdapterRegistry, SoftwareControlAction,
};
use std::path::PathBuf;

fn main() -> Result<()> {
    let output_dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/desktop-recognition-runner"));
    std::fs::create_dir_all(&output_dir)?;

    let db_path = output_dir.join("pool-runtime.sqlite");
    let control_dir = output_dir.join("worlds/demo/output/control/desktop-recognition");
    let repository = RuntimeRepository::open(&db_path)?;
    repository.migrate()?;

    let registry = SoftwareAdapterRegistry::defaults();
    let config = registry
        .get("touchdesigner")
        .cloned()
        .expect("touchdesigner adapter config");
    let adapter = DesktopRecognitionAdapter::new(config);
    let runner = SoftwareActionRunner::new(&repository);
    let mut task = RuntimeTask::new("demo", "TouchDesigner desktop recognition cue");
    task.node_id = Some("node-interactive".to_string());

    let report = runner.run(
        &adapter,
        task,
        SoftwareControlAction {
            adapter_id: "touchdesigner".to_string(),
            action_kind: SoftwareActionKind::RunViewport,
            priority: ControlPriority::DesktopRecognition,
            payload_json: serde_json::json!({
                "control_dir": control_dir,
                "instruction": "find TouchDesigner perform mode and trigger cue 1",
                "target_window": "TouchDesigner",
                "visual_targets": ["Perform", "Cue 1", "Output"]
            }),
            requires_confirmation: false,
        },
    )?;

    println!("db={}", db_path.display());
    println!("status={:?}", report.status);
    println!("action_id={}", report.action_id);
    println!(
        "artifacts={}",
        report
            .result
            .as_ref()
            .map(|result| result.artifacts.join(","))
            .unwrap_or_default()
    );
    println!(
        "stats=tasks:{},software_actions:{},events:{}",
        repository.table_count("tasks")?,
        repository.table_count("software_actions")?,
        repository.table_count("workflow_events")?
    );

    Ok(())
}
