use anyhow::Result;
use pool_core::{
    Mock3dgsProvider, ProviderRequest, ProviderTaskRunner, RuntimeRepository, RuntimeTask,
};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<()> {
    let output_dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/mock-3dgs-runner"));
    std::fs::create_dir_all(&output_dir)?;

    let db_path = output_dir.join("pool-runtime.sqlite");
    let repository = RuntimeRepository::open(&db_path)?;
    repository.migrate()?;

    let runner = ProviderTaskRunner::new(&repository);
    let provider = Mock3dgsProvider::new("mock-3dgs", "Mock 3DGS");
    let mut task = RuntimeTask::new("demo", "mock 3DGS asset package");
    task.node_id = Some("node-3dgs".to_string());

    let report = runner
        .run(
            &provider,
            task,
            ProviderRequest {
                project_slug: "demo".to_string(),
                prompt: "mock world".to_string(),
                input_paths: vec!["worlds/demo/source/plate.png".to_string()],
                output_dir: output_dir.to_string_lossy().to_string(),
                require_approval: false,
            },
        )
        .await?;

    let stats = repository.stats()?;
    println!("db={}", db_path.display());
    println!("status={:?}", report.status);
    println!("assets_indexed={}", report.assets.len());
    println!(
        "stats=tasks:{},assets:{},events:{}",
        stats.tasks, stats.assets, stats.events
    );

    Ok(())
}
