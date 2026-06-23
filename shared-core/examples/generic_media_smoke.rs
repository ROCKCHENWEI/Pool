use anyhow::{Context, Result};
use pool_core::{
    GenericHttpMediaOptions, GenericHttpMediaProvider, ProviderAdapter, ProviderKind,
    ProviderRequest, ProviderTaskRunner, RuntimeRepository, RuntimeTask, TaskStatus,
};
use serde_json::{json, Value};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<()> {
    let provider_id = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "nano-banana-pro".to_string());
    let request_path = std::env::args().nth(2).map(PathBuf::from);
    let output_dir = std::env::args()
        .nth(3)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/generic-media-smoke"));
    std::fs::create_dir_all(&output_dir)?;

    let (display_name, kind, auth_env_key, output_extension, high_cost) =
        defaults_for_provider(&provider_id);
    let provider = GenericHttpMediaProvider::new(GenericHttpMediaOptions::from_env(
        provider_id.clone(),
        display_name,
        kind,
        auth_env_key,
        output_extension,
        high_cost,
    ));
    let health = provider.health().await?;
    println!("provider={}", health.provider_id);
    println!("health={}", health.status);
    println!("message={}", health.message);

    let Some(request_path) = request_path else {
        println!("submit=skipped");
        println!(
            "usage=cargo run -p pool-core --example generic_media_smoke -- nano-banana-pro request.json target/generic-media-smoke"
        );
        return Ok(());
    };

    if health.status != "ready" {
        println!("submit=skipped_missing_endpoint");
        println!(
            "set POOL_{}_ENDPOINT or POOL_MEDIA_GATEWAY_ENDPOINT before submitting a media task",
            provider_id.replace('-', "_").to_ascii_uppercase()
        );
        return Ok(());
    }

    let prompt = std::fs::read_to_string(&request_path)
        .with_context(|| format!("read generic media request {}", request_path.display()))?;
    let db_path = output_dir.join("pool-runtime.sqlite");
    let repository = RuntimeRepository::open(&db_path)?;
    repository.migrate()?;

    let runner = ProviderTaskRunner::new(&repository);
    let mut task = RuntimeTask::new("generic-media-demo", format!("{provider_id} media run"));
    task.node_id = Some(format!("node-{provider_id}"));

    let report = runner
        .run(
            &provider,
            task,
            ProviderRequest {
                project_slug: "generic-media-demo".to_string(),
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
    println!(
        "job={:?}",
        report
            .job
            .as_ref()
            .and_then(|job| job.external_job_id.clone())
    );
    println!("assets_indexed={}", report.assets.len());
    println!(
        "stats=tasks:{},assets:{},events:{}",
        stats.tasks, stats.assets, stats.events
    );
    if let Some(bundle_path) = provider_evidence_bundle_path(&output_dir) {
        let attestation = std::env::var("POOL_PROVIDER_PRODUCTION_ATTESTATION")
            .context("POOL_PROVIDER_PRODUCTION_ATTESTATION is required when writing provider production evidence")?;
        let bundle = provider_production_evidence_bundle(
            "generic-media-demo",
            "generic_media_smoke",
            &provider_id,
            &provider.config().endpoint,
            "ai_media",
            &attestation,
            &report,
        )?;
        if let Some(parent) = bundle_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&bundle_path, serde_json::to_string_pretty(&bundle)?)?;
        println!("production_evidence_bundle={}", bundle_path.display());
    }

    Ok(())
}

fn provider_evidence_bundle_path(output_dir: &std::path::Path) -> Option<PathBuf> {
    std::env::var("POOL_PROVIDER_EVIDENCE_BUNDLE")
        .or_else(|_| std::env::var("POOL_PRODUCTION_EVIDENCE_BUNDLE"))
        .ok()
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var("POOL_PROVIDER_PRODUCTION_ATTESTATION")
                .ok()
                .map(|_| output_dir.join("provider-production-evidence-bundle.json"))
        })
}

fn provider_production_evidence_bundle(
    project_slug: &str,
    source: &str,
    provider_id: &str,
    endpoint: &str,
    family: &str,
    production_attestation: &str,
    report: &pool_core::ProviderTaskRunReport,
) -> Result<Value> {
    if report.status != TaskStatus::Succeeded {
        anyhow::bail!("provider production evidence requires a succeeded provider task");
    }
    let job = report
        .job
        .as_ref()
        .context("provider production evidence requires provider job metadata")?;
    let external_job_id = job
        .external_job_id
        .as_deref()
        .context("provider production evidence requires external_job_id")?;
    let artifacts = report
        .assets
        .iter()
        .map(|asset| asset.local_path.clone())
        .collect::<Vec<_>>();
    if artifacts.is_empty() {
        anyhow::bail!("provider production evidence requires indexed local artifacts");
    }
    Ok(json!({
        "project_slug": project_slug,
        "source": source,
        "providers": [{
            "provider_id": provider_id,
            "external_job_id": external_job_id,
            "endpoint": endpoint,
            "family": family,
            "production_attestation": production_attestation,
            "node_id": report.assets.first().and_then(|asset| asset.source_node_id.clone()),
            "task_title": format!("{provider_id} production gateway run"),
            "metadata_path": job.request_metadata_path,
            "artifacts": artifacts,
            "evidence_json": {
                "source": source,
                "production_upstream": true,
                "local_mock_gateway": false,
                "production_attestation": production_attestation,
                "provider_task_id": report.task_id,
                "gateway_metadata_path": job.request_metadata_path,
                "output_contract": "local-provider-files"
            },
            "response_json": {
                "status": "Succeeded",
                "ok": true,
                "external_job_id": external_job_id,
                "provider_job_status": format!("{:?}", job.status)
            }
        }],
        "software_actions": [],
        "desktop_vision": []
    }))
}

fn defaults_for_provider(
    provider_id: &str,
) -> (
    &'static str,
    ProviderKind,
    Option<&'static str>,
    &'static str,
    bool,
) {
    match provider_id {
        "midjourney" => (
            "Midjourney",
            ProviderKind::AiImage,
            Some("POOL_MIDJOURNEY_API_KEY"),
            "png",
            false,
        ),
        "suno" => (
            "Suno",
            ProviderKind::Audio,
            Some("POOL_SUNO_API_KEY"),
            "mp3",
            false,
        ),
        _ => (
            "Nano Banana Pro",
            ProviderKind::AiImage,
            Some("POOL_NANO_BANANA_PRO_KEY"),
            "png",
            false,
        ),
    }
}
