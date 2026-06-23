use anyhow::Result;
use pool_core::{
    ComfyUiProvider, ComfyUiProviderOptions, ProviderAdapter, ProviderRequest, ProviderTaskRunner,
    RuntimeRepository,
};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<()> {
    let endpoint = std::env::var("POOL_COMFYUI_ENDPOINT")
        .unwrap_or_else(|_| "http://127.0.0.1:8188".to_string());
    let provider = ComfyUiProvider::new(ComfyUiProviderOptions {
        endpoint,
        ..Default::default()
    });

    let health = provider.health().await?;
    println!(
        "provider={},status={},message={}",
        health.provider_id, health.status, health.message
    );

    let Some(workflow_path) = std::env::args().nth(1) else {
        println!("submit=skipped");
        return Ok(());
    };

    let workflow_json = std::fs::read_to_string(&workflow_path)?;
    let output_dir = std::env::args()
        .nth(2)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/comfyui-smoke"));
    std::fs::create_dir_all(&output_dir)?;

    let job = provider
        .submit(ProviderRequest {
            project_slug: "comfyui-smoke".to_string(),
            prompt: workflow_json,
            input_paths: Vec::new(),
            output_dir: output_dir.to_string_lossy().to_string(),
            require_approval: false,
        })
        .await?;

    println!(
        "prompt_id={}",
        job.external_job_id.as_deref().unwrap_or_default()
    );
    println!("metadata={}", job.request_metadata_path);

    if let Some(events_db) = std::env::args().nth(3) {
        let repository = RuntimeRepository::open(&events_db)?;
        repository.migrate()?;
        let runner = ProviderTaskRunner::new(&repository);
        provider
            .stream_progress_events(&job, |event| {
                println!(
                    "event={},level={:?},message={}",
                    event.project_slug, event.level, event.message
                );
                runner.record_progress_event(event)
            })
            .await?;
        if std::env::args().nth(4).is_some() {
            let assets = provider
                .download_and_index(&job, &repository, Some("comfyui-smoke"))
                .await?;
            println!("assets_indexed={}", assets.len());
        }
        println!("events_db={events_db}");
    }

    Ok(())
}
