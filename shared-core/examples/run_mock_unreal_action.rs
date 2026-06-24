use anyhow::Result;
use pool_core::{
    ControlPriority, MockUnrealAdapter, RuntimeRepository, RuntimeTask, SoftwareActionKind,
    SoftwareActionRunner, SoftwareControlAction,
};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<()> {
    let output_dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/mock-unreal-runner"));
    std::fs::create_dir_all(&output_dir)?;

    let db_path = output_dir.join("pool-runtime.sqlite");
    let repository = RuntimeRepository::open(&db_path)?;
    repository.migrate()?;

    let runner = SoftwareActionRunner::new(&repository);
    let adapter = MockUnrealAdapter::new();
    let mut task = RuntimeTask::new("demo", "Unreal scene assembly");
    task.node_id = Some("node-unreal".to_string());

    let report = runner.run(
        &adapter,
        task,
        SoftwareControlAction {
            adapter_id: "unreal".to_string(),
            action_kind: SoftwareActionKind::CreateScene,
            priority: ControlPriority::ApiMcp,
            payload_json: serde_json::json!({
                "project": "demo",
                "level": "demo_content_burst",
                "assets": ["worlds/demo/output/1-world.glb"],
                "camera": "hero_orbit"
            }),
            requires_confirmation: false,
        },
    )?;

    println!("db={}", db_path.display());
    println!("status={:?}", report.status);
    println!("action_id={}", report.action_id);
    println!(
        "result={}",
        report
            .result
            .as_ref()
            .map(|result| result.message.as_str())
            .unwrap_or("none")
    );
    println!(
        "stats=tasks:{},software_actions:{},events:{}",
        repository.table_count("tasks")?,
        repository.table_count("software_actions")?,
        repository.table_count("workflow_events")?
    );

    Ok(())
}
