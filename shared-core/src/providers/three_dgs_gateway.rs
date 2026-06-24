use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::fs;
use std::path::{Path, PathBuf};

use crate::models::{ProviderConfig, ProviderKind, TaskStatus};
use crate::providers::local_inputs::{local_input_manifest, reject_remote_input_paths};
use crate::providers::{
    ProviderAdapter, ProviderHealth, ProviderJob, ProviderRequest, ProviderVerification,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreeDgsGatewayOptions {
    pub provider_id: String,
    pub display_name: String,
    pub endpoint: String,
    pub api_key: Option<String>,
    pub auth_env_key: Option<String>,
    pub submit_path: String,
    pub poll_path_template: String,
    pub output_slug: String,
    pub asset_index: u32,
}

impl ThreeDgsGatewayOptions {
    pub fn new(provider_id: impl Into<String>, display_name: impl Into<String>) -> Self {
        let provider_id = provider_id.into();
        let profile = gateway_profile_for_provider(&provider_id);
        Self {
            provider_id,
            display_name: display_name.into(),
            endpoint: "http://127.0.0.1:8787".to_string(),
            api_key: None,
            auth_env_key: None,
            submit_path: profile.submit_path.to_string(),
            poll_path_template: profile.poll_path_template.to_string(),
            output_slug: profile.default_output_slug.to_string(),
            asset_index: 1,
        }
    }

    pub fn from_env(
        provider_id: impl Into<String>,
        display_name: impl Into<String>,
        auth_env_key: Option<&str>,
    ) -> Self {
        let provider_id = provider_id.into();
        let profile = gateway_profile_for_provider(&provider_id);
        let env_prefix = provider_env_prefix(&provider_id);
        let endpoint = std::env::var(format!("POOL_{env_prefix}_ENDPOINT"))
            .or_else(|_| std::env::var("POOL_3DGS_GATEWAY_ENDPOINT"))
            .unwrap_or_else(|_| "http://127.0.0.1:8787".to_string());
        let api_key = auth_env_key
            .and_then(|key| std::env::var(key).ok())
            .or_else(|| std::env::var(format!("POOL_{env_prefix}_API_KEY")).ok())
            .or_else(|| std::env::var("POOL_3DGS_GATEWAY_API_KEY").ok());
        Self {
            provider_id,
            display_name: display_name.into(),
            endpoint,
            api_key,
            auth_env_key: auth_env_key.map(ToString::to_string),
            submit_path: std::env::var(format!("POOL_{env_prefix}_SUBMIT_PATH"))
                .unwrap_or_else(|_| profile.submit_path.to_string()),
            poll_path_template: std::env::var(format!("POOL_{env_prefix}_POLL_PATH"))
                .unwrap_or_else(|_| profile.poll_path_template.to_string()),
            output_slug: std::env::var(format!("POOL_{env_prefix}_OUTPUT_SLUG"))
                .unwrap_or_else(|_| profile.default_output_slug.to_string()),
            asset_index: std::env::var(format!("POOL_{env_prefix}_ASSET_INDEX"))
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(1),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreeDgsGatewayRequest {
    pub prompt: String,
    #[serde(default)]
    pub input_paths: Vec<String>,
    pub output_slug: Option<String>,
    pub expected_outputs: Option<Vec<String>>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

pub struct ThreeDgsGatewayProvider {
    config: ProviderConfig,
    options: ThreeDgsGatewayOptions,
    profile: ThreeDgsGatewayProfile,
    client: Client,
}

impl ThreeDgsGatewayProvider {
    pub fn new(options: ThreeDgsGatewayOptions) -> Self {
        let profile = gateway_profile_for_provider(&options.provider_id);
        Self {
            config: ProviderConfig {
                id: options.provider_id.clone(),
                display_name: options.display_name.clone(),
                kind: ProviderKind::ThreeDgs,
                endpoint: options.endpoint.clone(),
                auth_env_key: options.auth_env_key.clone(),
                output_contract:
                    "HTTP 3DGS gateway profile; outputs downloaded as image-blaster indexed local files"
                        .to_string(),
                high_cost: true,
            },
            options,
            profile,
            client: Client::new(),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.options.endpoint.trim_end_matches('/'), path)
    }
}

#[async_trait]
impl ProviderAdapter for ThreeDgsGatewayProvider {
    fn config(&self) -> &ProviderConfig {
        &self.config
    }

    async fn health(&self) -> Result<ProviderHealth> {
        Ok(ProviderHealth {
            provider_id: self.config.id.clone(),
            status: if self.options.endpoint.trim().is_empty() {
                "missing_endpoint"
            } else {
                "ready"
            }
            .to_string(),
            message: "3DGS gateway configured; network health is checked on submit/poll"
                .to_string(),
        })
    }

    async fn submit(&self, request: ProviderRequest) -> Result<ProviderJob> {
        fs::create_dir_all(&request.output_dir)
            .with_context(|| format!("create 3DGS output dir {}", request.output_dir))?;

        let gateway_request = parse_gateway_request(&request)?;
        let output_slug = gateway_request
            .output_slug
            .clone()
            .unwrap_or_else(|| self.options.output_slug.clone());
        let expected_outputs = gateway_request
            .expected_outputs
            .clone()
            .unwrap_or_else(|| default_expected_outputs(&request.output_dir, &output_slug));
        let body = to_request_body(
            &request,
            &gateway_request,
            &output_slug,
            &expected_outputs,
            &self.config.id,
            &self.profile,
        )?;
        let mut builder = self
            .client
            .post(self.url(&self.options.submit_path))
            .json(&body);
        if let Some(api_key) = &self.options.api_key {
            builder = builder.bearer_auth(api_key);
        }
        let response: Value = builder
            .send()
            .await
            .context("submit 3DGS gateway job")?
            .error_for_status()
            .context("3DGS gateway submit returned error")?
            .json()
            .await
            .context("decode 3DGS gateway submit response")?;
        let job_id = extract_job_id(&response).context("3DGS gateway response missing job_id")?;
        let metadata_path = Path::new(&request.output_dir).join(format!(
            ".{}-{}__{}-request.json",
            self.options.asset_index, output_slug, self.config.id
        ));
        let metadata = json!({
            "provider_id": self.config.id,
            "endpoint": self.options.endpoint,
            "job_id": job_id,
            "project_slug": request.project_slug,
            "output_dir": request.output_dir,
            "output_slug": output_slug,
            "asset_index": self.options.asset_index,
            "poll_url": self.url(&self.options.poll_path_template.replace("{job_id}", &job_id)),
            "request": body,
            "response": response,
            "expected_outputs": expected_outputs,
        });
        fs::write(
            &metadata_path,
            serde_json::to_string_pretty(&metadata)
                .context("serialize 3DGS gateway request metadata")?,
        )
        .with_context(|| format!("write 3DGS metadata {}", metadata_path.display()))?;

        Ok(ProviderJob {
            provider_id: self.config.id.clone(),
            external_job_id: Some(job_id),
            status: TaskStatus::Running,
            request_metadata_path: metadata_path.to_string_lossy().to_string(),
            expected_outputs,
            metadata_json: Some(metadata),
        })
    }

    async fn poll(&self, job: &ProviderJob) -> Result<TaskStatus> {
        let job_id = job_id(job)?;
        let poll_path = self.options.poll_path_template.replace("{job_id}", &job_id);
        let mut builder = self.client.get(self.url(&poll_path));
        if let Some(api_key) = &self.options.api_key {
            builder = builder.bearer_auth(api_key);
        }
        let response: Value = builder
            .send()
            .await
            .with_context(|| format!("poll 3DGS gateway job {job_id}"))?
            .error_for_status()
            .with_context(|| format!("3DGS gateway poll returned error for {job_id}"))?
            .json()
            .await
            .with_context(|| format!("decode 3DGS gateway poll response for {job_id}"))?;

        Ok(map_gateway_status(&response))
    }

    async fn download(&self, job: &ProviderJob) -> Result<Vec<String>> {
        let metadata = job
            .metadata_json
            .as_ref()
            .context("3DGS gateway job missing metadata")?;
        let output_dir = output_dir(job)?;
        let output_slug = metadata
            .get("output_slug")
            .and_then(Value::as_str)
            .unwrap_or("world");
        let asset_index = metadata
            .get("asset_index")
            .and_then(Value::as_u64)
            .unwrap_or(1) as u32;
        let poll_result = fetch_result_json(&self.client, &self.options, job).await?;
        let outputs = collect_gateway_outputs(&poll_result)
            .or_else(|| collect_gateway_outputs(metadata))
            .unwrap_or_default();
        if outputs.is_empty() {
            return Ok(job.expected_outputs.clone());
        }
        download_gateway_outputs(
            &self.client,
            &output_dir,
            output_slug,
            asset_index,
            &outputs,
            &self.options.api_key,
        )
        .await
    }

    async fn verify(&self, job: &ProviderJob) -> Result<ProviderVerification> {
        let missing: Vec<_> = job
            .expected_outputs
            .iter()
            .filter(|path| !Path::new(path.as_str()).exists())
            .cloned()
            .collect();
        Ok(ProviderVerification {
            ok: missing.is_empty(),
            local_paths: job.expected_outputs.clone(),
            message: if missing.is_empty() {
                "3DGS indexed local output paths verified".to_string()
            } else {
                format!("missing 3DGS local outputs: {}", missing.join(", "))
            },
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ThreeDgsGatewayProfile {
    provider_id: &'static str,
    profile_id: &'static str,
    pipeline: &'static str,
    task_type: &'static str,
    asset_scope: &'static str,
    submit_path: &'static str,
    poll_path_template: &'static str,
    default_output_slug: &'static str,
    expected_output_kinds: &'static [&'static str],
}

const GENERIC_EXPECTED_OUTPUTS: &[&str] = &["metadata", "mesh", "gaussian_splat"];
const WORLDLABS_EXPECTED_OUTPUTS: &[&str] =
    &["world_metadata", "navigable_scene", "gaussian_splat"];
const TRIPO_EXPECTED_OUTPUTS: &[&str] = &["object_mesh", "gaussian_splat", "preview"];
const SAM3D_EXPECTED_OUTPUTS: &[&str] = &["segmentation_mask", "object_mesh", "metadata"];
const SPARK_EXPECTED_OUTPUTS: &[&str] = &["scene_splat", "mesh_proxy", "preview"];
const QUNHE_EXPECTED_OUTPUTS: &[&str] = &["scene_package", "materials", "metadata"];

fn gateway_profile_for_provider(provider_id: &str) -> ThreeDgsGatewayProfile {
    match provider_id {
        "worldlabs-marble" | "world-labs-marble" | "marble" => ThreeDgsGatewayProfile {
            provider_id: "worldlabs-marble",
            profile_id: "worldlabs-marble",
            pipeline: "image_or_text_to_world",
            task_type: "marble_world_generation",
            asset_scope: "scene",
            submit_path: "/v1/3dgs/jobs",
            poll_path_template: "/v1/3dgs/jobs/{job_id}",
            default_output_slug: "world",
            expected_output_kinds: WORLDLABS_EXPECTED_OUTPUTS,
        },
        "tripo-splat" | "triposplat" => ThreeDgsGatewayProfile {
            provider_id: "tripo-splat",
            profile_id: "triposplat",
            pipeline: "image_to_object_splat",
            task_type: "tripo_splat_reconstruction",
            asset_scope: "object",
            submit_path: "/v1/3dgs/triposplat/jobs",
            poll_path_template: "/v1/3dgs/triposplat/jobs/{job_id}",
            default_output_slug: "object",
            expected_output_kinds: TRIPO_EXPECTED_OUTPUTS,
        },
        "sam-3d" | "sam3d" => ThreeDgsGatewayProfile {
            provider_id: "sam-3d",
            profile_id: "sam-3d",
            pipeline: "segment_then_reconstruct_object",
            task_type: "sam_3d_object_reconstruction",
            asset_scope: "object",
            submit_path: "/v1/3dgs/sam-3d/jobs",
            poll_path_template: "/v1/3dgs/sam-3d/jobs/{job_id}",
            default_output_slug: "object",
            expected_output_kinds: SAM3D_EXPECTED_OUTPUTS,
        },
        "spark-3dgs" | "spark" => ThreeDgsGatewayProfile {
            provider_id: "spark-3dgs",
            profile_id: "spark-3dgs",
            pipeline: "multi_view_scene_reconstruction",
            task_type: "spark_3dgs_scene_reconstruction",
            asset_scope: "scene",
            submit_path: "/v1/3dgs/spark/jobs",
            poll_path_template: "/v1/3dgs/spark/jobs/{job_id}",
            default_output_slug: "scene",
            expected_output_kinds: SPARK_EXPECTED_OUTPUTS,
        },
        "qunhe-3d" | "qunhe" => ThreeDgsGatewayProfile {
            provider_id: "qunhe-3d",
            profile_id: "qunhe-3d",
            pipeline: "space_scene_reconstruction",
            task_type: "qunhe_scene_package",
            asset_scope: "scene",
            submit_path: "/v1/3dgs/qunhe/jobs",
            poll_path_template: "/v1/3dgs/qunhe/jobs/{job_id}",
            default_output_slug: "scene",
            expected_output_kinds: QUNHE_EXPECTED_OUTPUTS,
        },
        _ => ThreeDgsGatewayProfile {
            provider_id: "generic-3dgs",
            profile_id: "generic-3dgs",
            pipeline: "generic_3dgs_generation",
            task_type: "generic_3dgs_job",
            asset_scope: "scene",
            submit_path: "/v1/3dgs/jobs",
            poll_path_template: "/v1/3dgs/jobs/{job_id}",
            default_output_slug: "world",
            expected_output_kinds: GENERIC_EXPECTED_OUTPUTS,
        },
    }
}

fn profile_metadata(
    profile: &ThreeDgsGatewayProfile,
    output_slug: &str,
    expected_outputs: &[String],
) -> Value {
    json!({
        "profile_id": profile.profile_id,
        "provider_id": profile.provider_id,
        "pipeline": profile.pipeline,
        "task_type": profile.task_type,
        "asset_scope": profile.asset_scope,
        "output_contract": "image-blaster-indexed-files",
        "output_slug": output_slug,
        "expected_output_kinds": profile.expected_output_kinds,
        "expected_outputs": expected_outputs,
    })
}

fn profile_provider_payload(
    profile: &ThreeDgsGatewayProfile,
    gateway_request: &ThreeDgsGatewayRequest,
    output_slug: &str,
    expected_outputs: &[String],
) -> Value {
    match profile.profile_id {
        "worldlabs-marble" => json!({
            "service": "worldlabs-marble",
            "mode": "marble_world",
            "prompt": &gateway_request.prompt,
            "inputs": {
                "source_paths": &gateway_request.input_paths,
                "accepted_types": ["image", "video", "text_reference"]
            },
            "outputs": {
                "slug": output_slug,
                "contract": "image-blaster-indexed-files",
                "requested": expected_outputs,
                "kinds": profile.expected_output_kinds
            },
            "handoff": {
                "preferred_engine": "unreal",
                "scene_role": "world"
            }
        }),
        "triposplat" => json!({
            "service": "tripo-splat",
            "mode": "image_to_splat_object",
            "prompt": &gateway_request.prompt,
            "inputs": {
                "reference_paths": &gateway_request.input_paths,
                "asset_scope": "object"
            },
            "outputs": {
                "slug": output_slug,
                "contract": "image-blaster-indexed-files",
                "requested": expected_outputs,
                "kinds": profile.expected_output_kinds
            },
            "handoff": {
                "preferred_formats": ["spz", "glb", "preview"],
                "scene_role": "placeable_object"
            }
        }),
        "sam-3d" => json!({
            "service": "sam-3d",
            "mode": "segment_then_reconstruct",
            "prompt": &gateway_request.prompt,
            "inputs": {
                "reference_paths": &gateway_request.input_paths,
                "segmentation_required": true
            },
            "outputs": {
                "slug": output_slug,
                "contract": "image-blaster-indexed-files",
                "requested": expected_outputs,
                "kinds": profile.expected_output_kinds
            },
            "handoff": {
                "include_masks": true,
                "scene_role": "isolated_object"
            }
        }),
        "spark-3dgs" => json!({
            "service": "spark-3dgs",
            "mode": "scene_3dgs_reconstruction",
            "prompt": &gateway_request.prompt,
            "inputs": {
                "reference_paths": &gateway_request.input_paths,
                "asset_scope": "scene"
            },
            "outputs": {
                "slug": output_slug,
                "contract": "image-blaster-indexed-files",
                "requested": expected_outputs,
                "kinds": profile.expected_output_kinds
            },
            "handoff": {
                "preferred_formats": ["spz", "glb", "json"],
                "scene_role": "reconstructable_scene"
            }
        }),
        "qunhe-3d" => json!({
            "service": "qunhe-3d",
            "mode": "space_scene_package",
            "prompt": &gateway_request.prompt,
            "inputs": {
                "reference_paths": &gateway_request.input_paths,
                "asset_scope": "architectural_scene"
            },
            "outputs": {
                "slug": output_slug,
                "contract": "image-blaster-indexed-files",
                "requested": expected_outputs,
                "kinds": profile.expected_output_kinds
            },
            "handoff": {
                "include_materials": true,
                "scene_role": "assembled_environment"
            }
        }),
        _ => json!({
            "service": "generic-3dgs",
            "mode": "generic_3dgs_job",
            "prompt": &gateway_request.prompt,
            "inputs": {
                "reference_paths": &gateway_request.input_paths
            },
            "outputs": {
                "slug": output_slug,
                "contract": "image-blaster-indexed-files",
                "requested": expected_outputs,
                "kinds": profile.expected_output_kinds
            }
        }),
    }
}

fn parse_gateway_request(request: &ProviderRequest) -> Result<ThreeDgsGatewayRequest> {
    let trimmed = request.prompt.trim();
    let mut gateway_request: ThreeDgsGatewayRequest = if trimmed.starts_with('{') {
        serde_json::from_str(trimmed).context("3DGS ProviderRequest.prompt must be JSON")?
    } else {
        ThreeDgsGatewayRequest {
            prompt: trimmed.to_string(),
            input_paths: request.input_paths.clone(),
            output_slug: None,
            expected_outputs: None,
            extra: Map::new(),
        }
    };
    if gateway_request.prompt.trim().is_empty() {
        bail!("3DGS prompt cannot be empty");
    }
    if gateway_request.input_paths.is_empty() {
        gateway_request.input_paths = request.input_paths.clone();
    }
    reject_remote_input_paths(&gateway_request.input_paths, "3DGS gateway")?;
    Ok(gateway_request)
}

fn to_request_body(
    request: &ProviderRequest,
    gateway_request: &ThreeDgsGatewayRequest,
    output_slug: &str,
    expected_outputs: &[String],
    provider_id: &str,
    profile: &ThreeDgsGatewayProfile,
) -> Result<Value> {
    let mut body =
        serde_json::to_value(gateway_request).context("serialize 3DGS gateway request body")?;
    let Value::Object(ref mut map) = body else {
        bail!("3DGS gateway request body must be object");
    };
    map.insert(
        "project_slug".to_string(),
        Value::String(request.project_slug.clone()),
    );
    map.insert(
        "provider_id".to_string(),
        Value::String(provider_id.to_string()),
    );
    map.insert(
        "output_contract".to_string(),
        Value::String("image-blaster-indexed-files".to_string()),
    );
    map.insert(
        "output_slug".to_string(),
        Value::String(output_slug.to_string()),
    );
    map.insert(
        "expected_outputs".to_string(),
        serde_json::to_value(expected_outputs).context("serialize expected 3DGS outputs")?,
    );
    map.insert(
        "pool_gateway_profile".to_string(),
        profile_metadata(profile, output_slug, expected_outputs),
    );
    let input_manifest = local_input_manifest(&gateway_request.input_paths, "3DGS gateway")?;
    if input_manifest
        .as_array()
        .is_some_and(|entries| !entries.is_empty())
    {
        map.insert("local_input_manifest".to_string(), input_manifest);
    }
    map.entry("provider_payload".to_string())
        .or_insert_with(|| {
            profile_provider_payload(profile, gateway_request, output_slug, expected_outputs)
        });
    Ok(body)
}

fn extract_job_id(response: &Value) -> Option<String> {
    response
        .get("job_id")
        .or_else(|| response.get("task_id"))
        .or_else(|| response.get("id"))
        .or_else(|| response.pointer("/data/job_id"))
        .or_else(|| response.pointer("/data/task_id"))
        .or_else(|| response.pointer("/data/id"))
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn map_gateway_status(response: &Value) -> TaskStatus {
    let status = response
        .get("status")
        .or_else(|| response.pointer("/data/status"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    match status.as_str() {
        "completed" | "complete" | "succeeded" | "success" => TaskStatus::Succeeded,
        "failed" | "error" | "canceled" | "cancelled" => TaskStatus::Failed,
        "queued" | "pending" => TaskStatus::Queued,
        _ => TaskStatus::Running,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GatewayOutput {
    name: Option<String>,
    url: String,
}

fn collect_gateway_outputs(response: &Value) -> Option<Vec<GatewayOutput>> {
    let mut outputs = Vec::new();
    for pointer in [
        "/outputs",
        "/data/outputs",
        "/assets",
        "/data/assets",
        "/files",
    ] {
        let Some(items) = response.pointer(pointer).and_then(Value::as_array) else {
            continue;
        };
        for item in items {
            if let Some(url) = item
                .get("url")
                .or_else(|| item.get("download_url"))
                .and_then(Value::as_str)
            {
                outputs.push(GatewayOutput {
                    name: item
                        .get("name")
                        .or_else(|| item.get("filename"))
                        .and_then(Value::as_str)
                        .map(ToString::to_string),
                    url: url.to_string(),
                });
            }
        }
    }
    if let Some(url) = response
        .get("output_url")
        .or_else(|| response.pointer("/data/output_url"))
        .and_then(Value::as_str)
    {
        outputs.push(GatewayOutput {
            name: None,
            url: url.to_string(),
        });
    }
    if outputs.is_empty() {
        None
    } else {
        Some(outputs)
    }
}

async fn fetch_result_json(
    client: &Client,
    options: &ThreeDgsGatewayOptions,
    job: &ProviderJob,
) -> Result<Value> {
    let job_id = job_id(job)?;
    let poll_path = options.poll_path_template.replace("{job_id}", &job_id);
    let mut builder = client.get(format!(
        "{}{}",
        options.endpoint.trim_end_matches('/'),
        poll_path
    ));
    if let Some(api_key) = &options.api_key {
        builder = builder.bearer_auth(api_key);
    }
    builder
        .send()
        .await
        .with_context(|| format!("fetch 3DGS gateway result {job_id}"))?
        .error_for_status()
        .with_context(|| format!("3DGS gateway result returned error for {job_id}"))?
        .json()
        .await
        .with_context(|| format!("decode 3DGS gateway result {job_id}"))
}

async fn download_gateway_outputs(
    client: &Client,
    output_dir: &Path,
    output_slug: &str,
    asset_index: u32,
    outputs: &[GatewayOutput],
    api_key: &Option<String>,
) -> Result<Vec<String>> {
    fs::create_dir_all(output_dir)
        .with_context(|| format!("create 3DGS download dir {}", output_dir.display()))?;
    let mut paths = Vec::new();
    for (offset, output) in outputs.iter().enumerate() {
        let mut builder = client.get(&output.url);
        if let Some(api_key) = api_key {
            builder = builder.bearer_auth(api_key);
        }
        let bytes = builder
            .send()
            .await
            .with_context(|| format!("download 3DGS output {}", output.url))?
            .error_for_status()
            .with_context(|| format!("3DGS output download returned error for {}", output.url))?
            .bytes()
            .await
            .with_context(|| format!("read 3DGS output {}", output.url))?;
        let index = asset_index + offset as u32;
        let local_path = output_dir.join(indexed_output_file_name(index, output_slug, output));
        fs::write(&local_path, bytes)
            .with_context(|| format!("write 3DGS output {}", local_path.display()))?;
        paths.push(local_path.to_string_lossy().to_string());
    }
    Ok(paths)
}

fn default_expected_outputs(output_dir: &str, output_slug: &str) -> Vec<String> {
    vec![
        format!("{output_dir}/1-{output_slug}.json"),
        format!("{output_dir}/1-{output_slug}.glb"),
        format!("{output_dir}/1-{output_slug}-full_res.spz"),
    ]
}

fn output_dir(job: &ProviderJob) -> Result<PathBuf> {
    if let Some(path) = job
        .metadata_json
        .as_ref()
        .and_then(|metadata| metadata.get("output_dir"))
        .and_then(Value::as_str)
    {
        return Ok(PathBuf::from(path));
    }
    Path::new(&job.request_metadata_path)
        .parent()
        .map(Path::to_path_buf)
        .context("3DGS job missing output_dir metadata")
}

fn job_id(job: &ProviderJob) -> Result<String> {
    job.external_job_id
        .clone()
        .context("3DGS job missing job_id")
}

fn provider_env_prefix(provider_id: &str) -> String {
    provider_id
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect()
}

pub fn three_dgs_gateway_contract(provider_id: &str) -> Value {
    let profile = gateway_profile_for_provider(provider_id);
    let output_slug = profile.default_output_slug;
    let expected_outputs = default_expected_outputs("worlds/demo/output", output_slug);
    let sample_request = ProviderRequest {
        project_slug: "demo".to_string(),
        prompt: format!("Convert references into {} 3DGS asset", profile.asset_scope),
        input_paths: vec!["worlds/demo/source/0-reference.png".to_string()],
        output_dir: "worlds/demo/output".to_string(),
        require_approval: true,
    };
    let sample_gateway_request = ThreeDgsGatewayRequest {
        prompt: sample_request.prompt.clone(),
        input_paths: sample_request.input_paths.clone(),
        output_slug: Some(output_slug.to_string()),
        expected_outputs: Some(expected_outputs.clone()),
        extra: Map::new(),
    };
    let gateway_submit_body = to_request_body(
        &sample_request,
        &sample_gateway_request,
        output_slug,
        &expected_outputs,
        profile.provider_id,
        &profile,
    )
    .unwrap_or_else(|_| json!({}));

    json!({
        "provider_id": profile.provider_id,
        "adapter_kind": "three_dgs_http_gateway",
        "gateway_family": "3dgs",
        "profile": {
            "profile_id": profile.profile_id,
            "pipeline": profile.pipeline,
            "task_type": profile.task_type,
            "asset_scope": profile.asset_scope,
            "default_output_slug": profile.default_output_slug,
            "expected_output_kinds": profile.expected_output_kinds,
        },
        "environment": {
            "endpoint": format!("POOL_{}_ENDPOINT or POOL_3DGS_GATEWAY_ENDPOINT", provider_env_prefix(profile.provider_id)),
            "api_key": format!("POOL_{}_API_KEY or POOL_3DGS_GATEWAY_API_KEY", provider_env_prefix(profile.provider_id)),
            "submit_path": format!("POOL_{}_SUBMIT_PATH", provider_env_prefix(profile.provider_id)),
            "poll_path": format!("POOL_{}_POLL_PATH", provider_env_prefix(profile.provider_id)),
        },
        "runtime_provider_run": {
            "method": "POST",
            "path": "/api/provider-runs",
            "body": {
                "project_slug": "demo",
                "provider_id": profile.provider_id,
                "execution_mode": "gateway",
                "endpoint": "http://127.0.0.1:8787",
                "prompt": sample_gateway_request.prompt,
                "input_paths": sample_gateway_request.input_paths,
                "output_dir": sample_request.output_dir,
                "requires_approval": true,
            }
        },
        "gateway_submit": {
            "method": "POST",
            "path": profile.submit_path,
            "auth": "optional bearer token",
            "body": gateway_submit_body,
        },
        "gateway_poll": {
            "method": "GET",
            "path_template": profile.poll_path_template,
            "status_fields": ["status", "data.status"],
            "success_statuses": ["completed", "complete", "succeeded", "success"],
            "failure_statuses": ["failed", "error", "canceled", "cancelled"],
            "queued_statuses": ["queued", "pending"],
        },
        "gateway_response": {
            "job_id_fields": ["job_id", "task_id", "id", "data.job_id", "data.task_id", "data.id"],
            "output_fields": ["outputs", "data.outputs", "assets", "data.assets", "result.outputs", "data.result.outputs"],
            "output_item_fields": ["url", "download_url", "uri", "file_url", "name"],
        },
        "local_output_policy": {
            "local_files_authoritative": true,
            "provider_urls_are_provenance": true,
            "output_contract": "image-blaster-indexed-files",
            "metadata_file": ".<asset-index>-<output-slug>__<provider-id>-request.json",
            "indexed_outputs": ["N-<slug>.json", "N-<slug>.glb", "N-<slug>-full_res.spz"],
        },
    })
}

fn file_extension(path_or_url: &str) -> Option<&str> {
    path_or_url
        .split('?')
        .next()
        .and_then(|path| path.rsplit_once('.'))
        .map(|(_, extension)| extension)
        .filter(|extension| {
            extension
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        })
}

fn indexed_output_file_name(index: u32, output_slug: &str, output: &GatewayOutput) -> String {
    let extension = output
        .name
        .as_deref()
        .and_then(file_extension)
        .or_else(|| file_extension(&output.url))
        .unwrap_or("bin");
    let suffix = output
        .name
        .as_deref()
        .and_then(indexed_name_suffix)
        .unwrap_or_default();
    format!("{index}-{output_slug}{suffix}.{extension}")
}

fn indexed_name_suffix(file_name: &str) -> Option<&'static str> {
    let stem = file_name
        .split('?')
        .next()
        .and_then(|name| name.rsplit_once('.'))
        .map(|(stem, _)| stem)
        .unwrap_or(file_name)
        .to_ascii_lowercase();
    if stem.contains("full_res") || stem.contains("full-res") {
        Some("-full_res")
    } else if stem.contains("preview") {
        Some("-preview")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_prompt_request() {
        let request = ProviderRequest {
            project_slug: "demo".to_string(),
            prompt: "convert concept plate to 3DGS".to_string(),
            input_paths: vec!["worlds/demo/source/plate.png".to_string()],
            output_dir: "worlds/demo/output".to_string(),
            require_approval: false,
        };

        let parsed = parse_gateway_request(&request).unwrap();

        assert_eq!(parsed.prompt, "convert concept plate to 3DGS");
        assert_eq!(parsed.input_paths, vec!["worlds/demo/source/plate.png"]);
    }

    #[test]
    fn json_request_can_override_output_slug_and_expected_outputs() {
        let request = ProviderRequest {
            project_slug: "demo".to_string(),
            prompt: r#"{"prompt":"make scene","output_slug":"stage","expected_outputs":["out/1-stage.glb"],"quality":"high"}"#.to_string(),
            input_paths: Vec::new(),
            output_dir: "out".to_string(),
            require_approval: false,
        };

        let parsed = parse_gateway_request(&request).unwrap();
        let body = to_request_body(
            &request,
            &parsed,
            parsed.output_slug.as_deref().unwrap(),
            parsed.expected_outputs.as_deref().unwrap(),
            "worldlabs-marble",
            &gateway_profile_for_provider("worldlabs-marble"),
        )
        .unwrap();

        assert_eq!(body["output_slug"], "stage");
        assert_eq!(body["quality"], "high");
        assert_eq!(body["output_contract"], "image-blaster-indexed-files");
    }

    #[test]
    fn provider_profiles_select_default_gateway_paths() {
        let worldlabs = ThreeDgsGatewayOptions::new("worldlabs-marble", "World Labs Marble");
        let tripo = ThreeDgsGatewayOptions::new("tripo-splat", "TripoSplat");
        let sam = ThreeDgsGatewayOptions::new("sam-3d", "SAM-3D");
        let spark = ThreeDgsGatewayOptions::new("spark-3dgs", "Spark 3DGS");
        let qunhe = ThreeDgsGatewayOptions::new("qunhe-3d", "Qunhe 3D");

        assert_eq!(worldlabs.submit_path, "/v1/3dgs/jobs");
        assert_eq!(worldlabs.output_slug, "world");
        assert_eq!(tripo.submit_path, "/v1/3dgs/triposplat/jobs");
        assert_eq!(
            tripo.poll_path_template,
            "/v1/3dgs/triposplat/jobs/{job_id}"
        );
        assert_eq!(tripo.output_slug, "object");
        assert_eq!(sam.submit_path, "/v1/3dgs/sam-3d/jobs");
        assert_eq!(spark.submit_path, "/v1/3dgs/spark/jobs");
        assert_eq!(qunhe.submit_path, "/v1/3dgs/qunhe/jobs");
    }

    #[test]
    fn builds_triposplat_profile_payload() {
        let request = ProviderRequest {
            project_slug: "demo".to_string(),
            prompt: "make object splat".to_string(),
            input_paths: vec!["worlds/demo/source/prop.png".to_string()],
            output_dir: "worlds/demo/output".to_string(),
            require_approval: false,
        };
        let parsed = parse_gateway_request(&request).unwrap();
        let profile = gateway_profile_for_provider("tripo-splat");
        let expected_outputs = default_expected_outputs(&request.output_dir, "object");
        let body = to_request_body(
            &request,
            &parsed,
            "object",
            &expected_outputs,
            "tripo-splat",
            &profile,
        )
        .unwrap();

        assert_eq!(body["provider_id"], "tripo-splat");
        assert_eq!(body["pool_gateway_profile"]["profile_id"], "triposplat");
        assert_eq!(body["pool_gateway_profile"]["asset_scope"], "object");
        assert_eq!(body["provider_payload"]["mode"], "image_to_splat_object");
        assert_eq!(
            body["provider_payload"]["handoff"]["scene_role"],
            "placeable_object"
        );
    }

    #[test]
    fn preserves_custom_provider_payload_from_request() {
        let request = ProviderRequest {
            project_slug: "demo".to_string(),
            prompt: r#"{"prompt":"make object","provider_payload":{"custom":true}}"#.to_string(),
            input_paths: vec!["worlds/demo/source/prop.png".to_string()],
            output_dir: "worlds/demo/output".to_string(),
            require_approval: false,
        };
        let parsed = parse_gateway_request(&request).unwrap();
        let profile = gateway_profile_for_provider("sam-3d");
        let body = to_request_body(
            &request,
            &parsed,
            "object",
            &default_expected_outputs(&request.output_dir, "object"),
            "sam-3d",
            &profile,
        )
        .unwrap();

        assert_eq!(body["pool_gateway_profile"]["profile_id"], "sam-3d");
        assert_eq!(body["provider_payload"]["custom"], true);
    }

    #[test]
    fn adds_local_input_manifest_to_3dgs_gateway_body() {
        let root =
            std::env::temp_dir().join(format!("pool-3dgs-input-manifest-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let input_path = root.join("plate.png");
        std::fs::write(&input_path, b"3dgs-ref").unwrap();
        let request = ProviderRequest {
            project_slug: "demo".to_string(),
            prompt: "make object splat".to_string(),
            input_paths: vec![input_path.to_string_lossy().to_string()],
            output_dir: "worlds/demo/output".to_string(),
            require_approval: false,
        };
        let parsed = parse_gateway_request(&request).unwrap();
        let profile = gateway_profile_for_provider("tripo-splat");
        let body = to_request_body(
            &request,
            &parsed,
            "object",
            &default_expected_outputs(&request.output_dir, "object"),
            "tripo-splat",
            &profile,
        )
        .unwrap();

        assert_eq!(body["local_input_manifest"][0]["exists"], true);
        assert_eq!(body["local_input_manifest"][0]["mime_type"], "image/png");
        assert_eq!(body["local_input_manifest"][0]["bytes"], 8);
        assert!(body.to_string().find("3dgs-ref").is_none());

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_remote_3dgs_input_paths() {
        let request = ProviderRequest {
            project_slug: "demo".to_string(),
            prompt: "make object splat".to_string(),
            input_paths: vec!["https://example.com/plate.png".to_string()],
            output_dir: "worlds/demo/output".to_string(),
            require_approval: false,
        };

        let error = parse_gateway_request(&request).unwrap_err();

        assert!(error.to_string().contains("local file paths"));
    }

    #[test]
    fn extracts_job_id_from_common_shapes() {
        assert_eq!(
            extract_job_id(&json!({"job_id":"abc"})).as_deref(),
            Some("abc")
        );
        assert_eq!(
            extract_job_id(&json!({"data":{"task_id":"def"}})).as_deref(),
            Some("def")
        );
    }

    #[test]
    fn maps_gateway_status_values() {
        assert_eq!(
            map_gateway_status(&json!({"status":"completed"})),
            TaskStatus::Succeeded
        );
        assert_eq!(
            map_gateway_status(&json!({"data":{"status":"failed"}})),
            TaskStatus::Failed
        );
        assert_eq!(
            map_gateway_status(&json!({"status":"queued"})),
            TaskStatus::Queued
        );
    }

    #[test]
    fn collects_outputs_from_gateway_shapes() {
        let outputs = collect_gateway_outputs(&json!({
            "data": {
                "outputs": [
                    {"name":"world.glb","url":"https://cdn.example/world.glb"},
                    {"filename":"world.spz","download_url":"https://cdn.example/world.spz"}
                ]
            }
        }))
        .unwrap();

        assert_eq!(outputs.len(), 2);
        assert_eq!(outputs[0].name.as_deref(), Some("world.glb"));
        assert_eq!(outputs[1].url, "https://cdn.example/world.spz");
    }

    #[test]
    fn builds_default_indexed_outputs() {
        assert_eq!(
            default_expected_outputs("worlds/demo/output", "world"),
            vec![
                "worlds/demo/output/1-world.json",
                "worlds/demo/output/1-world.glb",
                "worlds/demo/output/1-world-full_res.spz"
            ]
        );
    }

    #[test]
    fn provider_env_prefix_is_stable() {
        assert_eq!(provider_env_prefix("worldlabs-marble"), "WORLDLABS_MARBLE");
        assert_eq!(provider_env_prefix("sam-3d"), "SAM_3D");
    }

    #[test]
    fn preserves_indexed_full_res_suffix() {
        let output = GatewayOutput {
            name: Some("world-full_res.spz".to_string()),
            url: "https://cdn.example/world-full_res.spz".to_string(),
        };

        assert_eq!(
            indexed_output_file_name(1, "world", &output),
            "1-world-full_res.spz"
        );
    }
}
