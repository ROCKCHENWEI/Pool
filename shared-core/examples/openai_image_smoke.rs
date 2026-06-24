use anyhow::{Context, Result};
use pool_core::{
    OpenAiImageProvider, OpenAiImageProviderOptions, ProviderAdapter, ProviderRequest,
    ProviderTaskRunner, RuntimeRepository, RuntimeTask, TaskStatus,
};
use serde_json::{json, Value};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<()> {
    let request_path = std::env::args().nth(1).map(PathBuf::from);
    let output_dir = std::env::args()
        .nth(2)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/openai-image-smoke"));
    std::fs::create_dir_all(&output_dir)?;

    let provider = OpenAiImageProvider::new(OpenAiImageProviderOptions::from_env());
    let health = provider.health().await?;
    println!("provider={}", health.provider_id);
    println!("health={}", health.status);
    println!("message={}", health.message);

    let Some(request_path) = request_path else {
        println!("submit=skipped");
        println!(
            "usage=cargo run -p pool-core --example openai_image_smoke -- request.json target/openai-image-smoke"
        );
        return Ok(());
    };

    if health.status != "ready" {
        println!("submit=skipped_missing_auth");
        println!("set OPENAI_API_KEY before submitting a real OpenAI image task");
        return Ok(());
    }

    let prompt = std::fs::read_to_string(&request_path)
        .with_context(|| format!("read OpenAI image request {}", request_path.display()))?;
    let db_path = output_dir.join("pool-runtime.sqlite");
    let repository = RuntimeRepository::open(&db_path)?;
    repository.migrate()?;

    let runner = ProviderTaskRunner::new(&repository);
    let mut task = RuntimeTask::new("openai-image-demo", "OpenAI image generation");
    task.node_id = Some("node-openai-image".to_string());

    let report = runner
        .run(
            &provider,
            task,
            ProviderRequest {
                project_slug: "openai-image-demo".to_string(),
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
            .context("POOL_PROVIDER_PRODUCTION_ATTESTATION is required when writing OpenAI image production evidence")?;
        let bundle = provider_production_evidence_bundle(
            "openai-image-demo",
            "openai_image_smoke",
            provider.config().id.as_str(),
            provider.config().endpoint.as_str(),
            "ai_image",
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
            "task_title": format!("{provider_id} production API run"),
            "metadata_path": job.request_metadata_path,
            "artifacts": artifacts,
            "evidence_json": {
                "source": source,
                "production_upstream": true,
                "local_mock_gateway": false,
                "production_attestation": production_attestation,
                "provider_task_id": report.task_id,
                "request_metadata_path": job.request_metadata_path,
                "output_contract": "openai-images-local-files"
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

#[cfg(test)]
mod tests {
    use super::*;
    use pool_core::{AssetRecord, ProviderJob, ProviderTaskRunReport};

    #[test]
    fn builds_openai_provider_production_evidence_bundle() {
        let mut asset = AssetRecord::local(
            "openai-image-demo",
            "1-openai-image.png",
            "target/openai-image-smoke/1-openai-image.png",
        );
        asset.source_node_id = Some("node-openai-image".to_string());
        let report = ProviderTaskRunReport {
            task_id: "task-openai-image".to_string(),
            provider_id: "openai-image-2".to_string(),
            status: TaskStatus::Succeeded,
            job: Some(ProviderJob {
                provider_id: "openai-image-2".to_string(),
                external_job_id: Some("req-openai-image-1".to_string()),
                status: TaskStatus::Succeeded,
                request_metadata_path: "target/openai-image-smoke/.openai-image-req-request.json"
                    .to_string(),
                expected_outputs: vec!["target/openai-image-smoke/1-openai-image.png".to_string()],
                metadata_json: None,
            }),
            assets: vec![asset],
        };

        let bundle = provider_production_evidence_bundle(
            "openai-image-demo",
            "openai_image_smoke",
            "openai-image-2",
            "https://api.openai.com/v1/images/generations",
            "ai_image",
            "real-openai-image-run-1",
            &report,
        )
        .unwrap();

        assert_eq!(bundle["project_slug"], "openai-image-demo");
        assert_eq!(bundle["providers"][0]["provider_id"], "openai-image-2");
        assert_eq!(
            bundle["providers"][0]["external_job_id"],
            "req-openai-image-1"
        );
        assert_eq!(
            bundle["providers"][0]["production_attestation"],
            "real-openai-image-run-1"
        );
        assert_eq!(
            bundle["providers"][0]["artifacts"][0],
            "target/openai-image-smoke/1-openai-image.png"
        );
        assert_eq!(
            bundle["providers"][0]["evidence_json"]["production_upstream"],
            true
        );
    }

    #[test]
    fn rejects_openai_provider_production_evidence_without_success() {
        let report = ProviderTaskRunReport {
            task_id: "task-openai-image".to_string(),
            provider_id: "openai-image-2".to_string(),
            status: TaskStatus::Failed,
            job: None,
            assets: Vec::new(),
        };

        let error = provider_production_evidence_bundle(
            "openai-image-demo",
            "openai_image_smoke",
            "openai-image-2",
            "https://api.openai.com/v1/images/generations",
            "ai_image",
            "real-openai-image-run-1",
            &report,
        )
        .unwrap_err();

        assert!(error
            .to_string()
            .contains("requires a succeeded provider task"));
    }
}
