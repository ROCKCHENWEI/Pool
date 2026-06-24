use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use base64::{engine::general_purpose, Engine as _};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::models::{ProviderConfig, ProviderKind, TaskStatus};
use crate::providers::local_inputs::{local_input_manifest, reject_remote_input_paths};
use crate::providers::{
    ProviderAdapter, ProviderHealth, ProviderJob, ProviderRequest, ProviderVerification,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenericHttpMediaOptions {
    pub provider_id: String,
    pub display_name: String,
    pub kind: ProviderKind,
    pub endpoint: String,
    pub api_key: Option<String>,
    pub auth_env_key: Option<String>,
    pub submit_path: String,
    pub poll_path_template: String,
    pub output_slug: String,
    pub output_extension: String,
    pub asset_index: u32,
    pub high_cost: bool,
}

impl GenericHttpMediaOptions {
    pub fn new(
        provider_id: impl Into<String>,
        display_name: impl Into<String>,
        kind: ProviderKind,
    ) -> Self {
        let provider_id = provider_id.into();
        let profile = media_profile_for_provider(&provider_id);
        Self {
            display_name: display_name.into(),
            kind,
            endpoint: String::new(),
            api_key: None,
            auth_env_key: None,
            submit_path: profile.submit_path.to_string(),
            poll_path_template: profile.poll_path_template.to_string(),
            output_slug: profile.default_output_slug.to_string(),
            output_extension: profile.default_output_extension.to_string(),
            asset_index: 1,
            high_cost: false,
            provider_id,
        }
    }

    pub fn from_env(
        provider_id: impl Into<String>,
        display_name: impl Into<String>,
        kind: ProviderKind,
        auth_env_key: Option<&str>,
        output_extension: impl Into<String>,
        high_cost: bool,
    ) -> Self {
        let provider_id = provider_id.into();
        let profile = media_profile_for_provider(&provider_id);
        let env_prefix = provider_env_prefix(&provider_id);
        let endpoint = std::env::var(format!("POOL_{env_prefix}_ENDPOINT"))
            .or_else(|_| std::env::var("POOL_MEDIA_GATEWAY_ENDPOINT"))
            .unwrap_or_default();
        let api_key = auth_env_key
            .and_then(|key| std::env::var(key).ok())
            .or_else(|| std::env::var(format!("POOL_{env_prefix}_API_KEY")).ok())
            .or_else(|| std::env::var("POOL_MEDIA_GATEWAY_API_KEY").ok());
        Self {
            provider_id,
            display_name: display_name.into(),
            kind,
            endpoint,
            api_key,
            auth_env_key: auth_env_key.map(ToString::to_string),
            submit_path: std::env::var(format!("POOL_{env_prefix}_SUBMIT_PATH"))
                .unwrap_or_else(|_| profile.submit_path.to_string()),
            poll_path_template: std::env::var(format!("POOL_{env_prefix}_POLL_PATH"))
                .unwrap_or_else(|_| profile.poll_path_template.to_string()),
            output_slug: std::env::var(format!("POOL_{env_prefix}_OUTPUT_SLUG"))
                .unwrap_or_else(|_| profile.default_output_slug.to_string()),
            output_extension: output_extension.into(),
            asset_index: std::env::var(format!("POOL_{env_prefix}_ASSET_INDEX"))
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(1),
            high_cost,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenericHttpMediaRequest {
    pub prompt: String,
    #[serde(default)]
    pub input_paths: Vec<String>,
    pub output_slug: Option<String>,
    pub output_extension: Option<String>,
    pub expected_outputs: Option<Vec<String>>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

pub struct GenericHttpMediaProvider {
    config: ProviderConfig,
    options: GenericHttpMediaOptions,
    profile: GenericHttpMediaProfile,
    client: Client,
}

impl GenericHttpMediaProvider {
    pub fn new(options: GenericHttpMediaOptions) -> Self {
        let profile = media_profile_for_provider(&options.provider_id);
        Self {
            config: ProviderConfig {
                id: options.provider_id.clone(),
                display_name: options.display_name.clone(),
                kind: options.kind.clone(),
                endpoint: options.endpoint.clone(),
                auth_env_key: options.auth_env_key.clone(),
                output_contract:
                    "HTTP media gateway profile; provider URLs are downloaded as local files"
                        .to_string(),
                high_cost: options.high_cost,
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
impl ProviderAdapter for GenericHttpMediaProvider {
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
            message: "generic HTTP media gateway configured; network health is checked on submit"
                .to_string(),
        })
    }

    async fn submit(&self, request: ProviderRequest) -> Result<ProviderJob> {
        fs::create_dir_all(&request.output_dir)
            .with_context(|| format!("create media output dir {}", request.output_dir))?;

        let media_request = parse_media_request(&request, &self.options.output_extension)?;
        let output_slug = media_request
            .output_slug
            .clone()
            .unwrap_or_else(|| self.options.output_slug.clone());
        let output_extension = media_request
            .output_extension
            .clone()
            .unwrap_or_else(|| self.options.output_extension.clone());
        let expected_outputs = media_request.expected_outputs.clone().unwrap_or_default();
        let body = to_request_body(
            &request,
            &media_request,
            &output_slug,
            &output_extension,
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
            .context("submit generic media gateway job")?
            .error_for_status()
            .context("generic media gateway submit returned error")?
            .json()
            .await
            .context("decode generic media gateway submit response")?;

        let outputs = collect_media_outputs(&response);
        let status = if !outputs.is_empty() {
            TaskStatus::Succeeded
        } else {
            map_media_status(&response)
        };
        let job_id = extract_job_id(&response)
            .unwrap_or_else(|| format!("{}-{}", self.config.id, Uuid::new_v4()));
        let output_dir = Path::new(&request.output_dir);
        let local_paths = if status == TaskStatus::Succeeded && !outputs.is_empty() {
            download_media_outputs(
                &self.client,
                output_dir,
                &output_slug,
                &self.config.id,
                self.options.asset_index,
                &output_extension,
                &outputs,
                &self.options.api_key,
            )
            .await?
        } else {
            Vec::new()
        };
        let metadata_path = output_dir.join(format!(
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
            "output_extension": output_extension,
            "asset_index": self.options.asset_index,
            "poll_url": self.url(&self.options.poll_path_template.replace("{job_id}", &job_id)),
            "request": body,
            "response_summary": summarize_media_response(&response),
            "expected_outputs": expected_outputs,
            "local_paths": local_paths,
        });
        fs::write(
            &metadata_path,
            serde_json::to_string_pretty(&metadata)
                .context("serialize generic media request metadata")?,
        )
        .with_context(|| format!("write media metadata {}", metadata_path.display()))?;

        Ok(ProviderJob {
            provider_id: self.config.id.clone(),
            external_job_id: Some(job_id),
            status,
            request_metadata_path: metadata_path.to_string_lossy().to_string(),
            expected_outputs: if local_paths.is_empty() {
                expected_outputs
            } else {
                local_paths
            },
            metadata_json: Some(metadata),
        })
    }

    async fn poll(&self, job: &ProviderJob) -> Result<TaskStatus> {
        if job.status == TaskStatus::Succeeded || job.status == TaskStatus::Failed {
            return Ok(job.status.clone());
        }

        let response = fetch_result_json(&self.client, &self.options, job).await?;
        Ok(map_media_status(&response))
    }

    async fn download(&self, job: &ProviderJob) -> Result<Vec<String>> {
        let existing = local_paths(job);
        if !existing.is_empty() {
            return Ok(existing);
        }

        let output_dir = output_dir(job)?;
        let output_slug =
            metadata_string(job, "output_slug").unwrap_or_else(|| "media".to_string());
        let output_extension = metadata_string(job, "output_extension")
            .unwrap_or_else(|| self.options.output_extension.clone());
        let asset_index = job
            .metadata_json
            .as_ref()
            .and_then(|metadata| metadata.get("asset_index"))
            .and_then(Value::as_u64)
            .unwrap_or(1) as u32;
        let result = fetch_result_json(&self.client, &self.options, job).await?;
        let outputs = collect_media_outputs(&result);
        if outputs.is_empty() {
            return Ok(job.expected_outputs.clone());
        }
        download_media_outputs(
            &self.client,
            &output_dir,
            &output_slug,
            &self.config.id,
            asset_index,
            &output_extension,
            &outputs,
            &self.options.api_key,
        )
        .await
    }

    async fn verify(&self, job: &ProviderJob) -> Result<ProviderVerification> {
        let paths = local_paths(job);
        let missing: Vec<_> = paths
            .iter()
            .filter(|path| !Path::new(path.as_str()).exists())
            .cloned()
            .collect();
        Ok(ProviderVerification {
            ok: missing.is_empty() && !paths.is_empty(),
            local_paths: paths,
            message: if missing.is_empty() {
                "generic media local output paths verified".to_string()
            } else {
                format!(
                    "missing generic media local outputs: {}",
                    missing.join(", ")
                )
            },
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GenericHttpMediaProfile {
    provider_id: &'static str,
    profile_id: &'static str,
    modality: &'static str,
    pipeline: &'static str,
    task_type: &'static str,
    media_role: &'static str,
    submit_path: &'static str,
    poll_path_template: &'static str,
    default_output_slug: &'static str,
    default_output_extension: &'static str,
    expected_output_kinds: &'static [&'static str],
}

const MIDJOURNEY_EXPECTED_OUTPUTS: &[&str] = &["image_grid", "upscaled_image", "metadata"];
const NANO_BANANA_EXPECTED_OUTPUTS: &[&str] = &["image", "edit_mask", "metadata"];
const SUNO_EXPECTED_OUTPUTS: &[&str] = &["audio_master", "stems", "lyrics_metadata"];
const GENERIC_MEDIA_EXPECTED_OUTPUTS: &[&str] = &["media_file", "metadata"];

fn media_profile_for_provider(provider_id: &str) -> GenericHttpMediaProfile {
    match provider_id {
        "midjourney" | "mj" => GenericHttpMediaProfile {
            provider_id: "midjourney",
            profile_id: "midjourney",
            modality: "image",
            pipeline: "prompt_to_image",
            task_type: "midjourney_imagine",
            media_role: "concept_plate",
            submit_path: "/v1/media/jobs",
            poll_path_template: "/v1/media/jobs/{job_id}",
            default_output_slug: "midjourney",
            default_output_extension: "png",
            expected_output_kinds: MIDJOURNEY_EXPECTED_OUTPUTS,
        },
        "nano-banana-pro" | "nano-banana" | "nanobanana" | "nanobananapro" => {
            GenericHttpMediaProfile {
                provider_id: "nano-banana-pro",
                profile_id: "nano-banana-pro",
                modality: "image",
                pipeline: "reference_guided_image_generation",
                task_type: "nano_banana_pro_image",
                media_role: "reference_plate",
                submit_path: "/v1/media/jobs",
                poll_path_template: "/v1/media/jobs/{job_id}",
                default_output_slug: "nano",
                default_output_extension: "png",
                expected_output_kinds: NANO_BANANA_EXPECTED_OUTPUTS,
            }
        }
        "suno" => GenericHttpMediaProfile {
            provider_id: "suno",
            profile_id: "suno",
            modality: "audio",
            pipeline: "prompt_to_music",
            task_type: "suno_music_generation",
            media_role: "soundtrack_or_cue",
            submit_path: "/v1/media/jobs",
            poll_path_template: "/v1/media/jobs/{job_id}",
            default_output_slug: "suno-cue",
            default_output_extension: "mp3",
            expected_output_kinds: SUNO_EXPECTED_OUTPUTS,
        },
        _ => GenericHttpMediaProfile {
            provider_id: "generic-media",
            profile_id: "generic-media",
            modality: "media",
            pipeline: "generic_media_generation",
            task_type: "generic_media_job",
            media_role: "media_asset",
            submit_path: "/v1/media/jobs",
            poll_path_template: "/v1/media/jobs/{job_id}",
            default_output_slug: "media",
            default_output_extension: "bin",
            expected_output_kinds: GENERIC_MEDIA_EXPECTED_OUTPUTS,
        },
    }
}

fn profile_metadata(
    profile: &GenericHttpMediaProfile,
    output_slug: &str,
    output_extension: &str,
    expected_outputs: &[String],
) -> Value {
    json!({
        "profile_id": profile.profile_id,
        "provider_id": profile.provider_id,
        "modality": profile.modality,
        "pipeline": profile.pipeline,
        "task_type": profile.task_type,
        "media_role": profile.media_role,
        "output_contract": "local-media-files",
        "output_slug": output_slug,
        "output_extension": output_extension,
        "expected_output_kinds": profile.expected_output_kinds,
        "expected_outputs": expected_outputs,
    })
}

fn profile_provider_payload(
    profile: &GenericHttpMediaProfile,
    media_request: &GenericHttpMediaRequest,
    output_slug: &str,
    output_extension: &str,
    expected_outputs: &[String],
) -> Value {
    match profile.profile_id {
        "midjourney" => json!({
            "service": "midjourney",
            "mode": "imagine",
            "prompt": &media_request.prompt,
            "inputs": {
                "reference_paths": &media_request.input_paths,
                "accepted_types": ["image", "text_reference"]
            },
            "outputs": {
                "slug": output_slug,
                "extension": output_extension,
                "contract": "local-media-files",
                "requested": expected_outputs,
                "kinds": profile.expected_output_kinds
            },
            "handoff": {
                "media_role": "concept_plate",
                "next_stage": "2d_to_3d_or_video"
            }
        }),
        "nano-banana-pro" => json!({
            "service": "nano-banana-pro",
            "mode": "reference_guided_image",
            "prompt": &media_request.prompt,
            "inputs": {
                "reference_paths": &media_request.input_paths,
                "accepted_types": ["image", "mask", "text_reference"]
            },
            "outputs": {
                "slug": output_slug,
                "extension": output_extension,
                "contract": "local-media-files",
                "requested": expected_outputs,
                "kinds": profile.expected_output_kinds
            },
            "handoff": {
                "media_role": "reference_plate",
                "next_stage": "comfyui_or_3dgs"
            }
        }),
        "suno" => json!({
            "service": "suno",
            "mode": "music_or_cue",
            "prompt": &media_request.prompt,
            "inputs": {
                "reference_paths": &media_request.input_paths,
                "accepted_types": ["text_reference", "audio_reference"]
            },
            "outputs": {
                "slug": output_slug,
                "extension": output_extension,
                "contract": "local-media-files",
                "requested": expected_outputs,
                "kinds": profile.expected_output_kinds
            },
            "handoff": {
                "media_role": "soundtrack_or_cue",
                "next_stage": "timeline_or_interactive_cue"
            }
        }),
        _ => json!({
            "service": "generic-media",
            "mode": "generic_media_job",
            "prompt": &media_request.prompt,
            "inputs": {
                "reference_paths": &media_request.input_paths
            },
            "outputs": {
                "slug": output_slug,
                "extension": output_extension,
                "contract": "local-media-files",
                "requested": expected_outputs,
                "kinds": profile.expected_output_kinds
            }
        }),
    }
}

fn parse_media_request(
    request: &ProviderRequest,
    default_extension: &str,
) -> Result<GenericHttpMediaRequest> {
    let trimmed = request.prompt.trim();
    let mut media_request: GenericHttpMediaRequest = if trimmed.starts_with('{') {
        serde_json::from_str(trimmed).context("media ProviderRequest.prompt must be JSON")?
    } else {
        GenericHttpMediaRequest {
            prompt: trimmed.to_string(),
            input_paths: request.input_paths.clone(),
            output_slug: None,
            output_extension: Some(default_extension.to_string()),
            expected_outputs: None,
            extra: Map::new(),
        }
    };
    if media_request.prompt.trim().is_empty() {
        bail!("media prompt cannot be empty");
    }
    if media_request.input_paths.is_empty() {
        media_request.input_paths = request.input_paths.clone();
    }
    reject_remote_input_paths(&media_request.input_paths, "media gateway")?;
    if media_request.output_extension.is_none() {
        media_request.output_extension = Some(default_extension.to_string());
    }
    Ok(media_request)
}

fn to_request_body(
    request: &ProviderRequest,
    media_request: &GenericHttpMediaRequest,
    output_slug: &str,
    output_extension: &str,
    expected_outputs: &[String],
    provider_id: &str,
    profile: &GenericHttpMediaProfile,
) -> Result<Value> {
    let mut body =
        serde_json::to_value(media_request).context("serialize generic media request body")?;
    let Value::Object(ref mut map) = body else {
        bail!("generic media request body must be object");
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
        Value::String("local-media-files".to_string()),
    );
    map.insert(
        "output_slug".to_string(),
        Value::String(output_slug.to_string()),
    );
    map.insert(
        "output_extension".to_string(),
        Value::String(output_extension.to_string()),
    );
    map.insert(
        "pool_media_profile".to_string(),
        profile_metadata(profile, output_slug, output_extension, expected_outputs),
    );
    let input_manifest = local_input_manifest(&media_request.input_paths, "media gateway")?;
    if input_manifest
        .as_array()
        .is_some_and(|entries| !entries.is_empty())
    {
        map.insert("local_input_manifest".to_string(), input_manifest);
    }
    map.entry("provider_payload".to_string())
        .or_insert_with(|| {
            profile_provider_payload(
                profile,
                media_request,
                output_slug,
                output_extension,
                expected_outputs,
            )
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

fn map_media_status(response: &Value) -> TaskStatus {
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
struct MediaOutput {
    name: Option<String>,
    url: Option<String>,
    base64_payload: Option<String>,
    extension: Option<String>,
    local_path: Option<String>,
}

fn collect_media_outputs(response: &Value) -> Vec<MediaOutput> {
    let mut outputs = Vec::new();
    for pointer in [
        "/outputs",
        "/data/outputs",
        "/assets",
        "/data/assets",
        "/files",
        "/data/files",
        "/images",
        "/data/images",
        "/audios",
        "/data/audios",
    ] {
        let Some(items) = response.pointer(pointer).and_then(Value::as_array) else {
            continue;
        };
        for item in items {
            collect_media_output_item(item, &mut outputs);
        }
    }

    for key in [
        "output_url",
        "image_url",
        "audio_url",
        "video_url",
        "file_url",
        "download_url",
    ] {
        if let Some(url) = response.get(key).and_then(Value::as_str) {
            outputs.push(output_from_string(url));
        }
        if let Some(url) = response
            .pointer(&format!("/data/{key}"))
            .and_then(Value::as_str)
        {
            outputs.push(output_from_string(url));
        }
    }

    outputs
}

fn collect_media_output_item(item: &Value, outputs: &mut Vec<MediaOutput>) {
    if let Some(value) = item.as_str() {
        outputs.push(output_from_string(value));
        return;
    }
    let Some(object) = item.as_object() else {
        return;
    };

    let url = [
        "url",
        "download_url",
        "image_url",
        "audio_url",
        "video_url",
        "file_url",
        "uri",
    ]
    .iter()
    .find_map(|key| object.get(*key).and_then(Value::as_str))
    .map(ToString::to_string);
    let base64_payload = ["b64_json", "base64", "data"]
        .iter()
        .find_map(|key| object.get(*key).and_then(Value::as_str))
        .map(ToString::to_string);
    let local_path = ["local_path", "path"]
        .iter()
        .find_map(|key| object.get(*key).and_then(Value::as_str))
        .map(ToString::to_string);

    if url.is_none() && base64_payload.is_none() && local_path.is_none() {
        return;
    }

    outputs.push(MediaOutput {
        name: object
            .get("name")
            .or_else(|| object.get("filename"))
            .and_then(Value::as_str)
            .map(ToString::to_string),
        url,
        base64_payload,
        extension: object
            .get("extension")
            .or_else(|| object.get("format"))
            .and_then(Value::as_str)
            .map(normalize_extension),
        local_path,
    });
}

fn output_from_string(value: &str) -> MediaOutput {
    if value.starts_with("data:") {
        return MediaOutput {
            name: None,
            url: None,
            base64_payload: Some(value.to_string()),
            extension: data_url_extension(value),
            local_path: None,
        };
    }

    MediaOutput {
        name: None,
        url: Some(value.to_string()),
        base64_payload: None,
        extension: file_extension(value).map(ToString::to_string),
        local_path: None,
    }
}

async fn fetch_result_json(
    client: &Client,
    options: &GenericHttpMediaOptions,
    job: &ProviderJob,
) -> Result<Value> {
    let job_id = job
        .external_job_id
        .clone()
        .context("generic media job missing job_id")?;
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
        .with_context(|| format!("fetch generic media result {job_id}"))?
        .error_for_status()
        .with_context(|| format!("generic media result returned error for {job_id}"))?
        .json()
        .await
        .with_context(|| format!("decode generic media result {job_id}"))
}

async fn download_media_outputs(
    client: &Client,
    output_dir: &Path,
    output_slug: &str,
    provider_id: &str,
    asset_index: u32,
    default_extension: &str,
    outputs: &[MediaOutput],
    api_key: &Option<String>,
) -> Result<Vec<String>> {
    fs::create_dir_all(output_dir)
        .with_context(|| format!("create media download dir {}", output_dir.display()))?;
    let mut paths = Vec::new();
    for (offset, output) in outputs.iter().enumerate() {
        if let Some(local_path) = &output.local_path {
            if Path::new(local_path).exists() {
                paths.push(local_path.clone());
                continue;
            }
        }

        let bytes = if let Some(encoded) = &output.base64_payload {
            decode_base64_payload(encoded)?
        } else if let Some(url) = &output.url {
            let mut builder = client.get(url);
            if let Some(api_key) = api_key {
                builder = builder.bearer_auth(api_key);
            }
            builder
                .send()
                .await
                .with_context(|| format!("download media output {url}"))?
                .error_for_status()
                .with_context(|| format!("media output download returned error for {url}"))?
                .bytes()
                .await
                .with_context(|| format!("read media output {url}"))?
                .to_vec()
        } else {
            continue;
        };

        let index = asset_index + offset as u32;
        let local_path = output_dir.join(media_output_file_name(
            index,
            output_slug,
            provider_id,
            default_extension,
            output,
        ));
        fs::write(&local_path, bytes)
            .with_context(|| format!("write media output {}", local_path.display()))?;
        paths.push(local_path.to_string_lossy().to_string());
    }
    Ok(paths)
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
        .context("generic media job missing output_dir metadata")
}

fn metadata_string(job: &ProviderJob, key: &str) -> Option<String> {
    job.metadata_json
        .as_ref()
        .and_then(|metadata| metadata.get(key))
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn local_paths(job: &ProviderJob) -> Vec<String> {
    if !job.expected_outputs.is_empty() {
        return job.expected_outputs.clone();
    }
    job.metadata_json
        .as_ref()
        .and_then(|metadata| metadata.get("local_paths"))
        .and_then(Value::as_array)
        .map(|paths| {
            paths
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn media_output_file_name(
    index: u32,
    output_slug: &str,
    provider_id: &str,
    default_extension: &str,
    output: &MediaOutput,
) -> String {
    let extension = output
        .extension
        .clone()
        .or_else(|| {
            output
                .name
                .as_deref()
                .and_then(file_extension)
                .map(ToString::to_string)
        })
        .or_else(|| {
            output
                .url
                .as_deref()
                .and_then(file_extension)
                .map(ToString::to_string)
        })
        .unwrap_or_else(|| normalize_extension(default_extension));
    let suffix = output
        .name
        .as_deref()
        .and_then(|name| name.split('?').next())
        .and_then(|name| name.rsplit_once('.').map(|(stem, _)| stem).or(Some(name)))
        .map(sanitize_stem)
        .filter(|stem| !stem.is_empty())
        .unwrap_or_else(|| provider_id.replace('-', "_"));
    format!("{index}-{output_slug}-{suffix}.{extension}")
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

fn normalize_extension(value: &str) -> String {
    match value
        .trim()
        .trim_start_matches('.')
        .to_ascii_lowercase()
        .as_str()
    {
        "jpeg" => "jpg".to_string(),
        "mpeg" => "mp3".to_string(),
        "" => "bin".to_string(),
        other => other.to_string(),
    }
}

fn data_url_extension(value: &str) -> Option<String> {
    let media_type = value.strip_prefix("data:")?.split_once(';')?.0;
    let extension = match media_type {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/webp" => "webp",
        "audio/mpeg" => "mp3",
        "audio/wav" => "wav",
        "video/mp4" => "mp4",
        _ => return None,
    };
    Some(extension.to_string())
}

fn decode_base64_payload(encoded: &str) -> Result<Vec<u8>> {
    let payload = encoded
        .split_once(',')
        .map(|(_, payload)| payload)
        .unwrap_or(encoded);
    general_purpose::STANDARD
        .decode(payload)
        .context("decode generic media base64 output")
}

fn sanitize_stem(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

fn summarize_media_response(response: &Value) -> Value {
    let outputs = collect_media_outputs(response)
        .into_iter()
        .map(|output| {
            json!({
                "name": output.name,
                "url": output.url,
                "has_base64": output.base64_payload.is_some(),
                "extension": output.extension,
                "local_path": output.local_path,
            })
        })
        .collect::<Vec<_>>();
    json!({
        "job_id": extract_job_id(response),
        "status": response.get("status").or_else(|| response.pointer("/data/status")),
        "outputs": outputs,
    })
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

pub fn generic_http_media_contract(provider_id: &str) -> Value {
    let profile = media_profile_for_provider(provider_id);
    let output_slug = profile.default_output_slug;
    let output_extension = profile.default_output_extension;
    let expected_outputs = Vec::<String>::new();
    let sample_request = ProviderRequest {
        project_slug: "demo".to_string(),
        prompt: format!("Generate {} for Pool content burst", profile.media_role),
        input_paths: vec!["worlds/demo/source/0-reference.png".to_string()],
        output_dir: "worlds/demo/output".to_string(),
        require_approval: false,
    };
    let sample_media_request = GenericHttpMediaRequest {
        prompt: sample_request.prompt.clone(),
        input_paths: sample_request.input_paths.clone(),
        output_slug: Some(output_slug.to_string()),
        output_extension: Some(output_extension.to_string()),
        expected_outputs: Some(expected_outputs.clone()),
        extra: Map::new(),
    };
    let gateway_submit_body = to_request_body(
        &sample_request,
        &sample_media_request,
        output_slug,
        output_extension,
        &expected_outputs,
        profile.provider_id,
        &profile,
    )
    .unwrap_or_else(|_| json!({}));

    json!({
        "provider_id": profile.provider_id,
        "adapter_kind": "generic_http_media_gateway",
        "gateway_family": "ai_media",
        "profile": {
            "profile_id": profile.profile_id,
            "modality": profile.modality,
            "pipeline": profile.pipeline,
            "task_type": profile.task_type,
            "media_role": profile.media_role,
            "default_output_slug": profile.default_output_slug,
            "default_output_extension": profile.default_output_extension,
            "expected_output_kinds": profile.expected_output_kinds,
        },
        "environment": {
            "endpoint": format!("POOL_{}_ENDPOINT or POOL_MEDIA_GATEWAY_ENDPOINT", provider_env_prefix(profile.provider_id)),
            "api_key": format!("POOL_{}_API_KEY or POOL_MEDIA_GATEWAY_API_KEY", provider_env_prefix(profile.provider_id)),
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
                "prompt": sample_media_request.prompt,
                "input_paths": sample_media_request.input_paths,
                "output_dir": sample_request.output_dir,
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
            "output_item_fields": ["url", "download_url", "uri", "file_url", "base64", "b64_json", "local_path", "name", "extension"],
        },
        "local_output_policy": {
            "local_files_authoritative": true,
            "provider_urls_are_provenance": true,
            "metadata_file": ".<asset-index>-<output-slug>__<provider-id>-request.json",
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_prompt_with_default_extension() {
        let request = ProviderRequest {
            project_slug: "demo".to_string(),
            prompt: "make key art".to_string(),
            input_paths: vec!["worlds/demo/source/ref.png".to_string()],
            output_dir: "worlds/demo/output".to_string(),
            require_approval: false,
        };

        let parsed = parse_media_request(&request, "png").unwrap();

        assert_eq!(parsed.prompt, "make key art");
        assert_eq!(parsed.output_extension.as_deref(), Some("png"));
        assert_eq!(parsed.input_paths, vec!["worlds/demo/source/ref.png"]);
    }

    #[test]
    fn json_request_preserves_extra_fields_and_output_contract() {
        let request = ProviderRequest {
            project_slug: "demo".to_string(),
            prompt: r#"{"prompt":"song cue","output_slug":"cue","style":"ambient"}"#.to_string(),
            input_paths: Vec::new(),
            output_dir: "out".to_string(),
            require_approval: false,
        };

        let parsed = parse_media_request(&request, "mp3").unwrap();
        let body = to_request_body(
            &request,
            &parsed,
            "cue",
            "mp3",
            &[],
            "suno",
            &media_profile_for_provider("suno"),
        )
        .unwrap();

        assert_eq!(body["prompt"], "song cue");
        assert_eq!(body["style"], "ambient");
        assert_eq!(body["output_contract"], "local-media-files");
        assert_eq!(body["pool_media_profile"]["profile_id"], "suno");
        assert_eq!(body["provider_payload"]["mode"], "music_or_cue");
    }

    #[test]
    fn media_profiles_select_default_output_contracts() {
        let midjourney =
            GenericHttpMediaOptions::new("midjourney", "Midjourney", ProviderKind::AiImage);
        let nano = GenericHttpMediaOptions::new(
            "nano-banana-pro",
            "Nano Banana Pro",
            ProviderKind::AiImage,
        );
        let suno = GenericHttpMediaOptions::new("suno", "Suno", ProviderKind::Audio);

        assert_eq!(midjourney.output_slug, "midjourney");
        assert_eq!(midjourney.output_extension, "png");
        assert_eq!(nano.output_slug, "nano");
        assert_eq!(nano.output_extension, "png");
        assert_eq!(suno.output_slug, "suno-cue");
        assert_eq!(suno.output_extension, "mp3");
        assert_eq!(suno.submit_path, "/v1/media/jobs");
    }

    #[test]
    fn builds_nano_banana_profile_payload() {
        let request = ProviderRequest {
            project_slug: "demo".to_string(),
            prompt: "generate hero plate".to_string(),
            input_paths: vec!["worlds/demo/source/ref.png".to_string()],
            output_dir: "worlds/demo/output".to_string(),
            require_approval: false,
        };
        let parsed = parse_media_request(&request, "png").unwrap();
        let profile = media_profile_for_provider("nano-banana-pro");
        let body = to_request_body(
            &request,
            &parsed,
            "nano",
            "png",
            &[],
            "nano-banana-pro",
            &profile,
        )
        .unwrap();

        assert_eq!(body["provider_id"], "nano-banana-pro");
        assert_eq!(
            body["pool_media_profile"]["pipeline"],
            "reference_guided_image_generation"
        );
        assert_eq!(body["provider_payload"]["mode"], "reference_guided_image");
        assert_eq!(
            body["provider_payload"]["handoff"]["next_stage"],
            "comfyui_or_3dgs"
        );
    }

    #[test]
    fn preserves_custom_media_provider_payload() {
        let request = ProviderRequest {
            project_slug: "demo".to_string(),
            prompt: r#"{"prompt":"make grid","provider_payload":{"vendor_mode":"relax"}}"#
                .to_string(),
            input_paths: Vec::new(),
            output_dir: "worlds/demo/output".to_string(),
            require_approval: false,
        };
        let parsed = parse_media_request(&request, "png").unwrap();
        let profile = media_profile_for_provider("midjourney");
        let body = to_request_body(
            &request,
            &parsed,
            "midjourney",
            "png",
            &[],
            "midjourney",
            &profile,
        )
        .unwrap();

        assert_eq!(body["pool_media_profile"]["profile_id"], "midjourney");
        assert_eq!(body["provider_payload"]["vendor_mode"], "relax");
    }

    #[test]
    fn adds_local_input_manifest_to_media_gateway_body() {
        let root = std::env::temp_dir().join(format!(
            "pool-media-input-manifest-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let input_path = root.join("ref.png");
        std::fs::write(&input_path, b"media-ref").unwrap();
        let request = ProviderRequest {
            project_slug: "demo".to_string(),
            prompt: "make key art".to_string(),
            input_paths: vec![input_path.to_string_lossy().to_string()],
            output_dir: "worlds/demo/output".to_string(),
            require_approval: false,
        };

        let parsed = parse_media_request(&request, "png").unwrap();
        let body = to_request_body(
            &request,
            &parsed,
            "nano",
            "png",
            &[],
            "nano-banana-pro",
            &media_profile_for_provider("nano-banana-pro"),
        )
        .unwrap();

        assert_eq!(body["local_input_manifest"][0]["exists"], true);
        assert_eq!(body["local_input_manifest"][0]["mime_type"], "image/png");
        assert_eq!(body["local_input_manifest"][0]["bytes"], 9);
        assert!(body.to_string().find("media-ref").is_none());

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_remote_media_input_paths() {
        let request = ProviderRequest {
            project_slug: "demo".to_string(),
            prompt: "make key art".to_string(),
            input_paths: vec!["https://example.com/ref.png".to_string()],
            output_dir: "worlds/demo/output".to_string(),
            require_approval: false,
        };

        let error = parse_media_request(&request, "png").unwrap_err();

        assert!(error.to_string().contains("local file paths"));
    }

    #[test]
    fn collects_common_output_shapes() {
        let outputs = collect_media_outputs(&json!({
            "status":"completed",
            "outputs":[
                {"name":"hero.png","url":"http://localhost/hero.png"},
                {"filename":"cue.mp3","base64":"ZmFrZQ=="}
            ],
            "data":{"audio_url":"http://localhost/cue.mp3"}
        }));

        assert_eq!(outputs.len(), 3);
        assert_eq!(outputs[0].name.as_deref(), Some("hero.png"));
        assert!(outputs[1].base64_payload.is_some());
        assert_eq!(outputs[2].url.as_deref(), Some("http://localhost/cue.mp3"));
    }

    #[test]
    fn maps_status_and_extracts_job_id() {
        assert_eq!(
            extract_job_id(&json!({"data":{"task_id":"job-1"}})).as_deref(),
            Some("job-1")
        );
        assert_eq!(
            map_media_status(&json!({"status":"completed"})),
            TaskStatus::Succeeded
        );
        assert_eq!(
            map_media_status(&json!({"data":{"status":"failed"}})),
            TaskStatus::Failed
        );
    }

    #[test]
    fn decodes_data_url_and_sanitizes_file_name() {
        let output = output_from_string("data:image/png;base64,ZmFrZS1pbWFnZQ==");
        assert_eq!(output.extension.as_deref(), Some("png"));
        assert_eq!(
            decode_base64_payload(output.base64_payload.as_deref().unwrap()).unwrap(),
            b"fake-image"
        );
        assert_eq!(
            media_output_file_name(1, "nano", "nano-banana-pro", "png", &output),
            "1-nano-nano_banana_pro.png"
        );
    }

    #[test]
    fn summarizes_response_without_raw_base64_payload() {
        let summary = summarize_media_response(&json!({
            "job_id":"job-1",
            "outputs":[{"name":"hero.png","base64":"ZmFrZS1pbWFnZQ=="}]
        }));

        assert_eq!(summary["job_id"], "job-1");
        assert_eq!(summary["outputs"][0]["has_base64"], true);
        assert!(summary.to_string().find("ZmFrZS1pbWFnZQ==").is_none());
    }
}
