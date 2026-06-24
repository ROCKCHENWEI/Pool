use anyhow::{Context, Result};
use pool_core::{
    ProviderAdapter, ProviderRequest, ProviderTaskRunner, RuntimeRepository, RuntimeTask,
    TaskStatus, ThreeDgsGatewayOptions, ThreeDgsGatewayProvider,
};
use serde_json::{json, Value};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<()> {
    let request_path = std::env::args().nth(1).map(PathBuf::from);
    let output_dir = std::env::args()
        .nth(2)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/three-dgs-gateway-smoke"));
    let provider_id = std::env::args()
        .nth(3)
        .unwrap_or_else(|| "worldlabs-marble".to_string());
    std::fs::create_dir_all(&output_dir)?;

    let provider = ThreeDgsGatewayProvider::new(ThreeDgsGatewayOptions::from_env(
        provider_id.clone(),
        provider_id.clone(),
        None,
    ));
    let health = provider.health().await?;
    println!("provider={}", health.provider_id);
    println!("health={}", health.status);
    println!("message={}", health.message);

    let Some(request_path) = request_path else {
        println!("submit=skipped");
        println!(
            "usage=cargo run -p pool-core --example three_dgs_gateway_smoke -- request.json target/three-dgs-gateway-smoke worldlabs-marble"
        );
        return Ok(());
    };

    let prompt = std::fs::read_to_string(&request_path)
        .with_context(|| format!("read 3DGS gateway request {}", request_path.display()))?;
    let db_path = output_dir.join("pool-runtime.sqlite");
    let repository = RuntimeRepository::open(&db_path)?;
    repository.migrate()?;

    let runner = ProviderTaskRunner::new(&repository);
    let mut task = RuntimeTask::new("three-dgs-demo", "3DGS gateway conversion");
    task.node_id = Some("node-3dgs".to_string());
    let report = runner
        .run(
            &provider,
            task,
            ProviderRequest {
                project_slug: "three-dgs-demo".to_string(),
                prompt,
                input_paths: vec!["worlds/three-dgs-demo/source/plate.png".to_string()],
                output_dir: output_dir.to_string_lossy().to_string(),
                require_approval: false,
            },
        )
        .await?;

    println!("db={}", db_path.display());
    println!("status={:?}", report.status);
    println!("assets_indexed={}", report.assets.len());
    println!(
        "stats=tasks:{},assets:{},events:{}",
        repository.table_count("tasks")?,
        repository.table_count("assets")?,
        repository.table_count("workflow_events")?
    );
    if let Some(bundle_path) = provider_evidence_bundle_path(&output_dir) {
        let attestation = std::env::var("POOL_PROVIDER_PRODUCTION_ATTESTATION")
            .context("POOL_PROVIDER_PRODUCTION_ATTESTATION is required when writing 3DGS production evidence")?;
        let bundle = provider_production_evidence_bundle(
            "three-dgs-demo",
            "three_dgs_gateway_smoke",
            &provider_id,
            &provider.config().endpoint,
            "3dgs",
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
                "output_contract": "image-blaster-indexed-files"
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
