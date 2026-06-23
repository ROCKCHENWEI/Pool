use anyhow::Result;
use pool_core::{
    build_default_content_burst_plan, materialize_project_envelope, RuntimeRepository,
};
use std::path::PathBuf;

fn main() -> Result<()> {
    let output_dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/pool-runtime-smoke"));
    std::fs::create_dir_all(&output_dir)?;

    let db_path = output_dir.join("pool-runtime.sqlite");
    let repository = RuntimeRepository::open(&db_path)?;
    repository.migrate()?;

    let plan = build_default_content_burst_plan("demo", "Pool runtime smoke");
    let stats = repository.persist_plan(&plan)?;
    let manifest = materialize_project_envelope(&output_dir, &plan)?;

    println!("db={}", db_path.display());
    println!(
        "stats=projects:{},shots:{},workflows:{},tasks:{},events:{}",
        stats.projects, stats.shots, stats.workflows, stats.tasks, stats.events
    );
    println!("envelope={}", manifest.root);

    Ok(())
}
