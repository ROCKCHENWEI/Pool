use anyhow::{Context, Result};
use pool_core::{
    build_default_content_burst_plan, materialize_project_envelope, runtime_prd_readiness_resource,
    spawn_provider_gateway_mock, RuntimeHttpConfig, RuntimeHttpServer, RuntimeRepository,
};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProviderFamily {
    Media,
    OpenAiImage,
    ThreeDgs,
}

#[derive(Debug, Clone, Copy)]
struct ProviderEvidenceTarget {
    provider_id: &'static str,
    family: ProviderFamily,
}

const TARGETS: &[ProviderEvidenceTarget] = &[
    ProviderEvidenceTarget {
        provider_id: "midjourney",
        family: ProviderFamily::Media,
    },
    ProviderEvidenceTarget {
        provider_id: "openai-image-2",
        family: ProviderFamily::OpenAiImage,
    },
    ProviderEvidenceTarget {
        provider_id: "nano-banana-pro",
        family: ProviderFamily::Media,
    },
    ProviderEvidenceTarget {
        provider_id: "suno",
        family: ProviderFamily::Media,
    },
    ProviderEvidenceTarget {
        provider_id: "worldlabs-marble",
        family: ProviderFamily::ThreeDgs,
    },
    ProviderEvidenceTarget {
        provider_id: "tripo-splat",
        family: ProviderFamily::ThreeDgs,
    },
    ProviderEvidenceTarget {
        provider_id: "sam-3d",
        family: ProviderFamily::ThreeDgs,
    },
    ProviderEvidenceTarget {
        provider_id: "spark-3dgs",
        family: ProviderFamily::ThreeDgs,
    },
    ProviderEvidenceTarget {
        provider_id: "qunhe-3d",
        family: ProviderFamily::ThreeDgs,
    },
];

fn main() -> Result<()> {
    let options = MatrixOptions::from_args(std::env::args().skip(1));
    std::fs::create_dir_all(&options.output_root)
        .with_context(|| format!("create output root {}", options.output_root.display()))?;
    let production_attestation = if options.production_upstream {
        Some(options.production_attestation()?)
    } else {
        None
    };

    let mock_endpoint = if options.configured_only {
        None
    } else if options.media_endpoint.is_none() || options.three_dgs_endpoint.is_none() {
        Some(spawn_provider_gateway_mock(96)?)
    } else {
        None
    };
    let media_endpoint = options
        .media_endpoint
        .clone()
        .or_else(|| mock_endpoint.clone());
    let three_dgs_endpoint = options
        .three_dgs_endpoint
        .clone()
        .or_else(|| mock_endpoint.clone());
    let openai_api_key_ready = options.openai_api_key_ready();

    let db_path = options.output_root.join("pool-runtime.sqlite");
    let repository = RuntimeRepository::open(&db_path)?;
    repository.migrate()?;
    if repository.stats()?.projects == 0 {
        let plan = build_default_content_burst_plan("demo", "Pool provider evidence matrix");
        repository.persist_plan(&plan)?;
        materialize_project_envelope(&options.output_root, &plan)?;
    }
    drop(repository);

    let server = RuntimeHttpServer::new(
        RuntimeHttpConfig::new(&db_path)
            .with_project_slug("demo")
            .with_bind_addr("127.0.0.1:4788"),
    );

    let mut succeeded = 0_usize;
    let mut failed = 0_usize;
    let mut skipped = 0_usize;
    let mut production_evidence_items = Vec::new();
    println!("db={}", db_path.display());
    println!("output_root={}", options.output_root.display());
    println!(
        "mock_endpoint={}",
        mock_endpoint.as_deref().unwrap_or("none")
    );
    if let Some(attestation) = production_attestation {
        println!("production_attestation={attestation}");
    }

    for target in TARGETS {
        let endpoint = match target.family {
            ProviderFamily::Media => media_endpoint.as_deref(),
            ProviderFamily::OpenAiImage => options.openai_endpoint.as_deref(),
            ProviderFamily::ThreeDgs => three_dgs_endpoint.as_deref(),
        };
        if target.family == ProviderFamily::OpenAiImage && !openai_api_key_ready {
            skipped += 1;
            println!(
                "provider={} family={:?} status=skipped reason=missing_openai_api_key",
                target.provider_id, target.family
            );
            continue;
        }
        let Some(endpoint) = endpoint else {
            skipped += 1;
            println!(
                "provider={} family={:?} status=skipped reason=missing_endpoint",
                target.provider_id, target.family
            );
            continue;
        };

        let evidence_mode = match target.family {
            ProviderFamily::OpenAiImage => "native_api",
            _ if mock_endpoint.as_deref() == Some(endpoint) => "local_mock_gateway",
            _ => "configured_gateway",
        };
        let production_upstream = options.production_upstream
            && matches!(evidence_mode, "configured_gateway" | "native_api");
        let output_dir = options
            .output_root
            .join("worlds/demo/output/provider-evidence")
            .join(target.provider_id);
        let body = json!({
            "project_slug": "demo",
            "provider_id": target.provider_id,
            "execution_mode": match target.family {
                ProviderFamily::OpenAiImage => "adapter",
                _ => "gateway",
            },
            "endpoint": endpoint,
            "api_key": if target.family == ProviderFamily::OpenAiImage {
                options.openai_api_key.clone()
            } else {
                None
            },
            "task_title": format!("{} provider evidence", target.provider_id),
            "prompt": evidence_prompt(target),
            "input_paths": if target.family == ProviderFamily::OpenAiImage {
                json!([])
            } else {
                json!(["worlds/demo/source/0-reference.png"])
            },
            "output_dir": output_dir.to_string_lossy(),
            "requires_approval": false,
            "evidence_json": {
                "source": "run_provider_evidence_matrix",
                "family": match target.family {
                    ProviderFamily::Media => "ai_media",
                    ProviderFamily::OpenAiImage => "ai_image",
                    ProviderFamily::ThreeDgs => "3dgs",
                },
                "evidence_mode": evidence_mode,
                "production_upstream": production_upstream,
                "local_mock_gateway": evidence_mode == "local_mock_gateway",
                "production_attestation": production_attestation,
            }
        });
        let response =
            server.handle_request_with_body("POST", "/api/provider-runs", &body.to_string())?;
        let value: Value = serde_json::from_str(&response.body)
            .with_context(|| format!("parse provider response for {}", target.provider_id))?;
        let status = value
            .pointer("/report/status")
            .and_then(Value::as_str)
            .or_else(|| value.get("error").and_then(Value::as_str))
            .unwrap_or("unknown");
        let assets = value
            .pointer("/report/assets")
            .and_then(Value::as_array)
            .map_or(0, Vec::len);
        let provider_request_id = value
            .get("provider_request_id")
            .and_then(Value::as_str)
            .unwrap_or("none");

        if response.status_code < 400 && status == "Succeeded" {
            succeeded += 1;
            if production_upstream {
                production_evidence_items.push(provider_production_evidence_item(
                    target,
                    endpoint,
                    &body,
                    &value,
                    &output_dir,
                    production_attestation.expect("production attestation is required"),
                )?);
            }
        } else {
            failed += 1;
        }
        println!(
            "provider={} family={:?} http={} status={} assets={} provider_request_id={} evidence_mode={}",
            target.provider_id,
            target.family,
            response.status_code,
            status,
            assets,
            provider_request_id,
            evidence_mode
        );
    }

    let evidence_bundle_path = options.evidence_bundle_path();
    let evidence_bundle = json!({
        "source": "run_provider_evidence_matrix",
        "project_slug": "demo",
        "providers": production_evidence_items,
        "software_actions": [],
        "desktop_vision": [],
    });
    write_json_file(&evidence_bundle_path, &evidence_bundle)?;
    println!(
        "provider_production_evidence_bundle={} providers={}",
        evidence_bundle_path.display(),
        evidence_bundle
            .get("providers")
            .and_then(Value::as_array)
            .map_or(0, Vec::len)
    );

    let repository = RuntimeRepository::open(&db_path)?;
    repository.migrate()?;
    let snapshot = repository.snapshot(Some("demo"))?;
    let readiness = runtime_prd_readiness_resource(&snapshot)?;
    println!(
        "summary=succeeded:{succeeded},failed:{failed},skipped:{skipped},provider_requests:{}",
        snapshot.provider_requests.len()
    );
    println!("prd_summary={}", readiness["summary"]);
    if let Some(ai_requirement) = readiness
        .get("requirements")
        .and_then(Value::as_array)
        .and_then(|items| {
            items.iter().find(|item| {
                item.get("id").and_then(Value::as_str) == Some("ai_media_and_3dgs_providers")
            })
        })
    {
        println!("ai_media_and_3dgs_status={}", ai_requirement["status"]);
        println!(
            "ai_media_and_3dgs_evidence={}",
            ai_requirement["evidence"]["provider_evidence"]
        );
    }

    Ok(())
}

#[derive(Debug)]
struct MatrixOptions {
    output_root: PathBuf,
    media_endpoint: Option<String>,
    openai_endpoint: Option<String>,
    openai_api_key: Option<String>,
    three_dgs_endpoint: Option<String>,
    configured_only: bool,
    production_upstream: bool,
    production_attestation: Option<String>,
    evidence_bundle_path: Option<PathBuf>,
}

impl MatrixOptions {
    fn from_args(args: impl IntoIterator<Item = String>) -> Self {
        let mut output_root = PathBuf::from("target/provider-evidence-matrix");
        let mut media_endpoint = std::env::var("POOL_MEDIA_GATEWAY_ENDPOINT").ok();
        let mut openai_endpoint = std::env::var("POOL_OPENAI_ENDPOINT")
            .ok()
            .or_else(|| Some("https://api.openai.com/v1".to_string()));
        let mut openai_api_key = std::env::var("OPENAI_API_KEY").ok();
        let mut three_dgs_endpoint = std::env::var("POOL_3DGS_GATEWAY_ENDPOINT").ok();
        let mut configured_only = false;
        let mut production_upstream = false;
        let mut production_attestation = std::env::var("POOL_PROVIDER_PRODUCTION_ATTESTATION").ok();
        let mut evidence_bundle_path = std::env::var("POOL_PROVIDER_EVIDENCE_BUNDLE")
            .ok()
            .map(PathBuf::from);

        for arg in args {
            if let Some(value) = arg.strip_prefix("--media-endpoint=") {
                media_endpoint = Some(value.to_string());
            } else if let Some(value) = arg.strip_prefix("--3dgs-endpoint=") {
                three_dgs_endpoint = Some(value.to_string());
            } else if let Some(value) = arg.strip_prefix("--openai-endpoint=") {
                openai_endpoint = Some(value.to_string());
            } else if let Some(value) = arg.strip_prefix("--openai-api-key=") {
                openai_api_key = Some(value.to_string());
            } else if let Some(value) = arg.strip_prefix("--endpoint=") {
                media_endpoint = Some(value.to_string());
                three_dgs_endpoint = Some(value.to_string());
            } else if arg == "--configured-only" {
                configured_only = true;
            } else if arg == "--production-upstream" {
                production_upstream = true;
            } else if let Some(value) = arg.strip_prefix("--production-attestation=") {
                production_attestation = Some(value.to_string());
            } else if let Some(value) = arg.strip_prefix("--evidence-bundle=") {
                evidence_bundle_path = Some(PathBuf::from(value));
            } else if !arg.trim().is_empty() {
                output_root = PathBuf::from(arg);
            }
        }

        Self {
            output_root,
            media_endpoint,
            openai_endpoint,
            openai_api_key,
            three_dgs_endpoint,
            configured_only,
            production_upstream,
            production_attestation,
            evidence_bundle_path,
        }
    }

    fn openai_api_key_ready(&self) -> bool {
        self.openai_api_key
            .as_deref()
            .map(str::trim)
            .is_some_and(|value| !value.is_empty())
            || std::env::var("OPENAI_API_KEY")
                .ok()
                .map(|value| !value.trim().is_empty())
                .unwrap_or(false)
    }

    fn evidence_bundle_path(&self) -> PathBuf {
        self.evidence_bundle_path.clone().unwrap_or_else(|| {
            self.output_root
                .join("worlds/demo/output/control/production-evidence/provider-production-evidence-bundle.json")
        })
    }

    fn production_attestation(&self) -> Result<&str> {
        let attestation = self
            .production_attestation
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .context(
                "--production-upstream requires --production-attestation=<real-worker-attestation> or POOL_PROVIDER_PRODUCTION_ATTESTATION",
            )?;
        if !is_valid_production_attestation(attestation) {
            anyhow::bail!(
                "provider production attestation must identify a real upstream worker/SDK run and must not use placeholder, todo, dummy, fake, or mock text"
            );
        }
        Ok(attestation)
    }
}

fn evidence_prompt(target: &ProviderEvidenceTarget) -> String {
    match target.family {
        ProviderFamily::Media => format!(
            "Pool provider evidence run for {}. Generate one local media output for audit.",
            target.provider_id
        ),
        ProviderFamily::OpenAiImage => json!({
            "prompt": "Pool provider evidence run for OpenAI image-2. Generate one local audit image.",
            "size": "1024x1024",
            "quality": "medium",
            "output_format": "png"
        })
        .to_string(),
        ProviderFamily::ThreeDgs => format!(
            "Pool provider evidence run for {}. Convert reference input to image-blaster indexed 3DGS outputs.",
            target.provider_id
        ),
    }
}

fn provider_production_evidence_item(
    target: &ProviderEvidenceTarget,
    endpoint: &str,
    request_body: &Value,
    response: &Value,
    output_dir: &Path,
    production_attestation: &str,
) -> Result<Value> {
    let local_artifacts = provider_response_artifacts(response);
    let (artifacts, missing_artifacts): (Vec<_>, Vec<_>) = local_artifacts
        .into_iter()
        .partition(|path| Path::new(path.as_str()).exists());
    if artifacts.is_empty() {
        anyhow::bail!(
            "production evidence for {} requires at least one local artifact path",
            target.provider_id
        );
    }
    if !missing_artifacts.is_empty() {
        anyhow::bail!(
            "production evidence for {} references missing local artifacts: {}",
            target.provider_id,
            missing_artifacts.join(", ")
        );
    }
    let metadata_path = output_dir.join("provider-production-metadata.json");
    write_json_file(
        &metadata_path,
        &json!({
            "kind": "pool_provider_production_evidence_metadata",
            "provider_id": target.provider_id,
            "endpoint": endpoint,
            "request": request_body,
            "response": response,
            "production_attestation": production_attestation,
            "artifact_policy": {
                "local_files_authoritative": true,
                "provider_urls_are_provenance": true
            }
        }),
    )?;

    Ok(json!({
        "provider_id": target.provider_id,
        "external_job_id": external_job_id(response, target.provider_id),
        "endpoint": endpoint,
        "family": match target.family {
            ProviderFamily::Media => "ai_media",
            ProviderFamily::OpenAiImage => "ai_image",
            ProviderFamily::ThreeDgs => "3dgs",
        },
        "task_title": format!("{} production upstream evidence", target.provider_id),
        "metadata_path": metadata_path.to_string_lossy(),
        "artifacts": artifacts,
        "evidence_json": {
            "source": "run_provider_evidence_matrix",
            "evidence_mode": "production_upstream",
            "production_upstream": true,
            "local_mock_gateway": false,
            "configured_gateway": true,
            "production_attestation": production_attestation,
            "provider_request_id": response.get("provider_request_id").and_then(Value::as_str),
        },
        "response_json": response,
    }))
}

fn provider_response_artifacts(response: &Value) -> Vec<String> {
    let mut artifacts = Vec::new();
    collect_string_array(response.pointer("/report/assets"), &mut artifacts);
    collect_string_array(response.pointer("/assets"), &mut artifacts);
    collect_string_array(response.pointer("/report/artifacts"), &mut artifacts);
    collect_string_array(response.pointer("/artifacts"), &mut artifacts);
    collect_output_paths(response.pointer("/snapshot/assets"), &mut artifacts);
    artifacts.sort();
    artifacts.dedup();
    artifacts
        .into_iter()
        .filter(|path| is_local_artifact_path(path))
        .collect()
}

fn collect_string_array(value: Option<&Value>, output: &mut Vec<String>) {
    if let Some(values) = value.and_then(Value::as_array) {
        output.extend(values.iter().filter_map(|value| {
            match value {
                Value::String(path) => Some(path.clone()),
                Value::Object(object) => object
                    .get("local_path")
                    .or_else(|| object.get("path"))
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
                _ => None,
            }
        }));
    }
}

fn collect_output_paths(value: Option<&Value>, output: &mut Vec<String>) {
    if let Some(values) = value.and_then(Value::as_array) {
        output.extend(values.iter().filter_map(|value| {
            value
                .get("local_path")
                .and_then(Value::as_str)
                .map(ToString::to_string)
        }));
    }
}

fn external_job_id(response: &Value, provider_id: &str) -> String {
    response
        .pointer("/report/job_id")
        .or_else(|| response.pointer("/report/provider_job/job_id"))
        .or_else(|| response.pointer("/job_id"))
        .or_else(|| response.pointer("/provider_job_id"))
        .or_else(|| response.pointer("/provider_request_id"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(|| {
            format!(
                "{}-{}",
                provider_id,
                response
                    .get("provider_request_id")
                    .and_then(Value::as_str)
                    .unwrap_or("production-evidence")
            )
        })
}

fn is_local_artifact_path(path: &str) -> bool {
    let trimmed = path.trim();
    !trimmed.is_empty()
        && !trimmed.starts_with("http://")
        && !trimmed.starts_with("https://")
        && !trimmed.starts_with("s3://")
}

fn is_valid_production_attestation(value: &str) -> bool {
    let trimmed = value.trim();
    let lowered = trimmed.to_ascii_lowercase();
    trimmed.len() >= 8
        && ![
            "replace-with",
            "placeholder",
            "todo",
            "dummy",
            "fake",
            "mock",
        ]
        .iter()
        .any(|blocked| lowered.contains(blocked))
}

fn write_json_file(path: &Path, value: &Value) -> Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    let body = serde_json::to_string_pretty(value)?;
    std::fs::write(path, body).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_bundle_collects_local_provider_artifacts() {
        let response = json!({
            "provider_request_id": "request-1",
            "report": {
                "job_id": "vendor-job-1",
                "assets": [
                    "worlds/demo/output/provider-evidence/midjourney/1-image.png",
                    {"local_path": "worlds/demo/output/provider-evidence/midjourney/2-metadata.json"},
                    "https://example.com/remote.png"
                ]
            }
        });

        assert_eq!(
            provider_response_artifacts(&response),
            vec![
                "worlds/demo/output/provider-evidence/midjourney/1-image.png".to_string(),
                "worlds/demo/output/provider-evidence/midjourney/2-metadata.json".to_string(),
            ]
        );
        assert_eq!(external_job_id(&response, "midjourney"), "vendor-job-1");
    }

    #[test]
    fn production_evidence_item_requires_existing_local_artifacts() {
        let temp_dir = unique_temp_dir("provider-existing-artifacts");
        std::fs::create_dir_all(&temp_dir).unwrap();
        let artifact_path = temp_dir.join("1-image.png");
        std::fs::write(&artifact_path, b"image").unwrap();
        let output_dir = temp_dir.join("metadata");

        let response = json!({
            "provider_request_id": "request-1",
            "report": {
                "job_id": "vendor-job-1",
                "assets": [
                    artifact_path.to_string_lossy(),
                    "https://example.com/provenance.png"
                ]
            }
        });
        let item = provider_production_evidence_item(
            &ProviderEvidenceTarget {
                provider_id: "midjourney",
                family: ProviderFamily::Media,
            },
            "http://127.0.0.1:8788",
            &json!({"prompt":"demo"}),
            &response,
            &output_dir,
            "real-vendor-sdk-worker-2026-06-17",
        )
        .unwrap();

        assert_eq!(item["artifacts"].as_array().unwrap().len(), 1);
        assert!(Path::new(item["metadata_path"].as_str().unwrap()).exists());
        assert_eq!(
            item["evidence_json"]["production_attestation"],
            "real-vendor-sdk-worker-2026-06-17"
        );
        std::fs::remove_dir_all(temp_dir).ok();
    }

    #[test]
    fn production_evidence_item_rejects_missing_local_artifacts() {
        let temp_dir = unique_temp_dir("provider-missing-artifacts");
        let missing_path = temp_dir.join("missing.png");
        let response = json!({
            "provider_request_id": "request-1",
            "report": {
                "job_id": "vendor-job-1",
                "assets": [missing_path.to_string_lossy()]
            }
        });
        let error = provider_production_evidence_item(
            &ProviderEvidenceTarget {
                provider_id: "midjourney",
                family: ProviderFamily::Media,
            },
            "http://127.0.0.1:8788",
            &json!({"prompt":"demo"}),
            &response,
            &temp_dir.join("metadata"),
            "real-vendor-sdk-worker-2026-06-17",
        )
        .unwrap_err()
        .to_string();

        assert!(error.contains("requires at least one local artifact path"));
    }

    #[test]
    fn matrix_options_accepts_evidence_bundle_path() {
        let options = MatrixOptions::from_args([
            "target/provider-evidence".to_string(),
            "--production-upstream".to_string(),
            "--production-attestation=real-vendor-sdk-worker-2026-06-17".to_string(),
            "--evidence-bundle=target/provider-evidence/bundle.json".to_string(),
        ]);

        assert!(options.production_upstream);
        assert_eq!(
            options.production_attestation().unwrap(),
            "real-vendor-sdk-worker-2026-06-17"
        );
        assert_eq!(
            options.evidence_bundle_path(),
            PathBuf::from("target/provider-evidence/bundle.json")
        );
    }

    #[test]
    fn production_upstream_requires_real_attestation() {
        let options = MatrixOptions::from_args([
            "target/provider-evidence".to_string(),
            "--production-upstream".to_string(),
            "--production-attestation=mock gateway".to_string(),
        ]);

        assert!(options
            .production_attestation()
            .unwrap_err()
            .to_string()
            .contains("provider production attestation"));
    }

    fn unique_temp_dir(slug: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "pool-{slug}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }
}
