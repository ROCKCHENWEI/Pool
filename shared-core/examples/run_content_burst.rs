use anyhow::{Context, Result};
use pool_core::{ContentBurstRunRequest, ContentBurstRunner, RuntimeRepository};
use std::path::PathBuf;

fn main() -> Result<()> {
    let output_root = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/content-burst-runner"));
    std::fs::create_dir_all(&output_root)
        .with_context(|| format!("create output root {}", output_root.display()))?;

    let db_path = output_root.join("pool-runtime.sqlite");
    let repository = RuntimeRepository::open(&db_path)?;
    repository.migrate()?;

    let runner = ContentBurstRunner::new(&repository);
    let report = runner.run(ContentBurstRunRequest {
        project_slug: "demo".to_string(),
        output_root: output_root.to_string_lossy().to_string(),
        title: "Pool local content burst".to_string(),
        prompt: "turn a reference plate into video, game and interactive art outputs".to_string(),
        source_inputs: vec!["worlds/demo/source/0-reference.png".to_string()],
        duration_ms: 12_000,
        ..ContentBurstRunRequest::new("demo", output_root.to_string_lossy().to_string())
    })?;

    let stats = repository.stats()?;
    println!("db={}", db_path.display());
    println!("workflow_id={}", report.workflow_id);
    println!(
        "agent_status={:?}",
        report.agent_report.as_ref().map(|report| &report.status)
    );
    println!("provider_status={:?}", report.provider_report.status);
    println!("software_status={:?}", report.software_report.status);
    println!("output_status={:?}", report.output_report.status);
    println!("assets_indexed={}", report.assets_indexed);
    println!(
        "stats=projects:{},tasks:{},assets:{},events:{},agent_sessions:{}",
        stats.projects,
        stats.tasks,
        stats.assets,
        stats.events,
        repository.table_count("agent_sessions")?
    );

    Ok(())
}
