use anyhow::{Context, Result};
use pool_core::{
    build_default_content_burst_plan, materialize_project_envelope, OutputPackageRequest,
    OutputPackageRunner, RuntimeRepository,
};
use std::path::PathBuf;

fn main() -> Result<()> {
    let output_dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/output-package-runner"));
    std::fs::create_dir_all(&output_dir)
        .with_context(|| format!("create output dir {}", output_dir.display()))?;

    let db_path = output_dir.join("pool-runtime.sqlite");
    let repository = RuntimeRepository::open(&db_path)?;
    repository.migrate()?;

    let plan = build_default_content_burst_plan("demo", "Pool output package smoke");
    repository.persist_plan(&plan)?;
    let envelope = materialize_project_envelope(&output_dir, &plan)?;

    let runner = OutputPackageRunner::new(&repository);
    let report = runner.run(OutputPackageRequest {
        project_slug: "demo".to_string(),
        node_id: Some("outputs".to_string()),
        output_dir: envelope.output_dir,
        title: "三类输出交付包".to_string(),
        source_assets: vec![
            "worlds/demo/output/1-world.glb".to_string(),
            "worlds/demo/output/2-world-full_res.spz".to_string(),
        ],
        duration_ms: 12_000,
    })?;

    let stats = repository.stats()?;
    println!("db={}", db_path.display());
    println!("status={:?}", report.status);
    println!("deliverables={}", report.local_paths.join(","));
    println!("assets_indexed={}", report.assets.len());
    println!(
        "stats=projects:{},tasks:{},assets:{},events:{}",
        stats.projects, stats.tasks, stats.assets, stats.events
    );

    Ok(())
}
