use anyhow::{Context, Result};
use pool_core::{
    build_default_content_burst_plan, materialize_project_envelope, RuntimeRepository,
};
use std::path::PathBuf;

fn main() -> Result<()> {
    let output_dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/runtime-snapshot-smoke"));
    std::fs::create_dir_all(&output_dir)?;

    let db_path = output_dir.join("pool-runtime.sqlite");
    let snapshot_path = output_dir.join("runtime-snapshot.json");
    let repository = RuntimeRepository::open(&db_path)?;
    repository.migrate()?;

    let plan = build_default_content_burst_plan("demo", "Pool runtime snapshot smoke");
    repository.persist_plan(&plan)?;
    materialize_project_envelope(&output_dir, &plan)?;

    let snapshot = repository.snapshot(Some("demo"))?;
    std::fs::write(
        &snapshot_path,
        serde_json::to_string_pretty(&snapshot).context("serialize runtime snapshot")?,
    )
    .with_context(|| format!("write runtime snapshot {}", snapshot_path.display()))?;

    println!("db={}", db_path.display());
    println!("snapshot={}", snapshot_path.display());
    println!(
        "stats=projects:{},workflows:{},tasks:{},node_states:{},waiting_approval:{}",
        snapshot.stats.projects,
        snapshot.stats.workflows,
        snapshot.stats.tasks,
        snapshot.node_states.len(),
        snapshot.stats.waiting_approval
    );

    Ok(())
}
