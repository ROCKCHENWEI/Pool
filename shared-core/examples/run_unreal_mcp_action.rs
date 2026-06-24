use anyhow::Result;
use pool_core::{
    ControlPriority, RuntimeRepository, RuntimeTask, SoftwareActionKind, SoftwareActionRunner,
    SoftwareAdapter, SoftwareControlAction, UnrealMcpAdapter,
};
use std::path::PathBuf;

fn main() -> Result<()> {
    let endpoint = std::env::args()
        .nth(1)
        .or_else(|| std::env::var("POOL_UNREAL_MCP_ENDPOINT").ok());
    let output_dir = std::env::args()
        .nth(2)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/unreal-mcp-runner"));
    std::fs::create_dir_all(&output_dir)?;

    let payload_json = serde_json::json!({
        "endpoint": endpoint,
        "project": "demo",
        "level": "demo_content_burst",
        "assets": ["worlds/demo/output/1-world.glb"],
        "camera": "hero_orbit"
    });
    let action = SoftwareControlAction {
        adapter_id: "unreal".to_string(),
        action_kind: SoftwareActionKind::CreateScene,
        priority: ControlPriority::ApiMcp,
        payload_json,
        requires_confirmation: false,
    };
    let adapter = UnrealMcpAdapter::from_action(&action);
    let health = adapter.health()?;

    println!("health_ok={}", health.ok);
    println!("health={}", health.message);
    if !adapter.is_configured() {
        println!("submit=skipped_missing_endpoint");
        println!(
            "usage=cargo run -p pool-core --example run_unreal_mcp_action -- http://127.0.0.1:8787 target/unreal-mcp-runner"
        );
        return Ok(());
    }
    if !health.ok {
        println!("submit=skipped_unhealthy");
        return Ok(());
    }

    let db_path = output_dir.join("pool-runtime.sqlite");
    let repository = RuntimeRepository::open(&db_path)?;
    repository.migrate()?;

    let runner = SoftwareActionRunner::new(&repository);
    let mut task = RuntimeTask::new("demo", "Unreal MCP scene assembly");
    task.node_id = Some("node-unreal".to_string());
    let report = runner.run(&adapter, task, action)?;

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
