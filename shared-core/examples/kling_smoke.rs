use anyhow::{Context, Result};
use pool_core::{
    KlingProvider, KlingProviderOptions, ProviderAdapter, ProviderRequest, ProviderTaskRunner,
    RuntimeRepository, RuntimeTask,
};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<()> {
    let request_path = std::env::args().nth(1).map(PathBuf::from);
    let output_dir = std::env::args()
        .nth(2)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/kling-smoke"));
    std::fs::create_dir_all(&output_dir)?;

    let provider = KlingProvider::new(KlingProviderOptions::from_env());
    let health = provider.health().await?;
    println!("provider={}", health.provider_id);
    println!("health={}", health.status);
    println!("message={}", health.message);

    let Some(request_path) = request_path else {
        println!("submit=skipped");
        println!(
            "usage=cargo run -p pool-core --example kling_smoke -- request.json target/kling-smoke"
        );
        return Ok(());
    };

    if health.status != "ready" {
        println!("submit=skipped_missing_auth");
        println!("set POOL_KLING_API_KEY or POOL_KLING_ACCESS_KEY/POOL_KLING_SECRET_KEY");
        return Ok(());
    }

    let prompt = std::fs::read_to_string(&request_path)
        .with_context(|| format!("read Kling request {}", request_path.display()))?;
    let db_path = output_dir.join("pool-runtime.sqlite");
    let repository = RuntimeRepository::open(&db_path)?;
    repository.migrate()?;

    let runner = ProviderTaskRunner::new(&repository);
    let mut task = RuntimeTask::new("kling-demo", "Kling video generation");
    task.node_id = Some("node-kling-video".to_string());

    let report = runner
        .run(
            &provider,
            task,
            ProviderRequest {
                project_slug: "kling-demo".to_string(),
                prompt,
                input_paths: Vec::new(),
                output_dir: output_dir.to_string_lossy().to_string(),
                require_approval: false,
            },
        )
        .await?;

    let stats = repository.stats()?;
    println!("db={}", db_path.display());
    println!("status={:?}", report.status);
    println!("job={:?}", report.job.and_then(|job| job.external_job_id));
    println!("assets_indexed={}", report.assets.len());
    println!(
        "stats=tasks:{},assets:{},events:{}",
        stats.tasks, stats.assets, stats.events
    );

    Ok(())
}
