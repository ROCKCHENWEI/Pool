use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use base64::{engine::general_purpose, Engine as _};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

use crate::models::{ProviderConfig, ProviderKind, TaskStatus};
use crate::providers::{
    ProviderAdapter, ProviderHealth, ProviderJob, ProviderRequest, ProviderVerification,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum KlingAuth {
    BearerToken(String),
    AccessSecret {
        access_key: String,
        secret_key: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KlingProviderOptions {
    pub endpoint: String,
    pub auth: Option<KlingAuth>,
}

impl Default for KlingProviderOptions {
    fn default() -> Self {
        Self {
            endpoint: "https://api.klingapi.com".to_string(),
            auth: None,
        }
    }
}

impl KlingProviderOptions {
    pub fn from_env() -> Self {
        let endpoint = std::env::var("POOL_KLING_ENDPOINT")
            .unwrap_or_else(|_| "https://api.klingapi.com".to_string());
        let auth = std::env::var("POOL_KLING_API_KEY")
            .ok()
            .map(KlingAuth::BearerToken)
            .or_else(|| {
                let access_key = std::env::var("POOL_KLING_ACCESS_KEY").ok()?;
                let secret_key = std::env::var("POOL_KLING_SECRET_KEY").ok()?;
                Some(KlingAuth::AccessSecret {
                    access_key,
                    secret_key,
                })
            });
        Self { endpoint, auth }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KlingVideoRequest {
    #[serde(default = "default_kling_model")]
    pub model: String,
    pub prompt: String,
    pub negative_prompt: Option<String>,
    pub duration: Option<u64>,
    pub aspect_ratio: Option<String>,
    pub mode: Option<String>,
    pub image: Option<String>,
    pub image_url: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, Value>,
}

fn default_kling_model() -> String {
    "kling-v2.6-std".to_string()
}

pub struct KlingProvider {
    config: ProviderConfig,
    options: KlingProviderOptions,
    client: Client,
}

impl KlingProvider {
    pub fn new(options: KlingProviderOptions) -> Self {
        Self {
            config: ProviderConfig {
                id: "kling".to_string(),
                display_name: "Kling".to_string(),
                kind: ProviderKind::AiVideo,
                endpoint: options.endpoint.clone(),
                auth_env_key: Some(
                    "POOL_KLING_API_KEY or POOL_KLING_ACCESS_KEY/POOL_KLING_SECRET_KEY".to_string(),
                ),
                output_contract:
                    "Kling async video task; generated video URL downloaded into local output_dir"
                        .to_string(),
                high_cost: true,
            },
            options,
            client: Client::new(),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.options.endpoint.trim_end_matches('/'), path)
    }

    fn auth_header(&self) -> Result<String> {
        match self.options.auth.as_ref().context(
            "Kling auth missing: set POOL_KLING_API_KEY or POOL_KLING_ACCESS_KEY/POOL_KLING_SECRET_KEY",
        )? {
            KlingAuth::BearerToken(token) => Ok(format!("Bearer {token}")),
            KlingAuth::AccessSecret {
                access_key,
                secret_key,
            } => Ok(format!("Bearer {}", generate_kling_jwt(access_key, secret_key)?)),
        }
    }
}

#[async_trait]
impl ProviderAdapter for KlingProvider {
    fn config(&self) -> &ProviderConfig {
        &self.config
    }

    async fn health(&self) -> Result<ProviderHealth> {
        Ok(ProviderHealth {
            provider_id: self.config.id.clone(),
            status: if self.options.auth.is_some() {
                "ready"
            } else {
                "missing_auth"
            }
            .to_string(),
            message: "Kling adapter configured; network health is checked on submit/poll"
                .to_string(),
        })
    }

    async fn submit(&self, request: ProviderRequest) -> Result<ProviderJob> {
        fs::create_dir_all(&request.output_dir)
            .with_context(|| format!("create Kling output dir {}", request.output_dir))?;
        let video_request = parse_video_request(&request.prompt, &request.input_paths)?;
        let endpoint_path = if video_request.image.is_some() || video_request.image_url.is_some() {
            "/v1/videos/image2video"
        } else {
            "/v1/videos/text2video"
        };
        let body = to_request_body(video_request)?;
        let metadata_request = metadata_request_body(&body, &request.input_paths);
        let response: Value = self
            .client
            .post(self.url(endpoint_path))
            .header("Authorization", self.auth_header()?)
            .json(&body)
            .send()
            .await
            .context("submit Kling video task")?
            .error_for_status()
            .context("Kling submit returned error")?
            .json()
            .await
            .context("decode Kling submit response")?;

        let task_id = extract_task_id(&response).context("Kling response missing task_id")?;
        let metadata_path =
            Path::new(&request.output_dir).join(format!(".kling-{task_id}-request.json"));
        let metadata = json!({
            "provider_id": self.config.id,
            "endpoint": self.options.endpoint,
            "task_id": task_id,
            "project_slug": request.project_slug,
            "output_dir": request.output_dir,
            "status_url": self.url(&format!("/v1/videos/{task_id}")),
            "request": metadata_request,
            "response": response,
        });
        fs::write(
            &metadata_path,
            serde_json::to_string_pretty(&metadata).context("serialize Kling request metadata")?,
        )
        .with_context(|| format!("write Kling metadata {}", metadata_path.display()))?;

        Ok(ProviderJob {
            provider_id: self.config.id.clone(),
            external_job_id: Some(task_id),
            status: TaskStatus::Running,
            request_metadata_path: metadata_path.to_string_lossy().to_string(),
            expected_outputs: Vec::new(),
            metadata_json: Some(metadata),
        })
    }

    async fn poll(&self, job: &ProviderJob) -> Result<TaskStatus> {
        let task_id = task_id(job)?;
        let status: Value = self
            .client
            .get(self.url(&format!("/v1/videos/{task_id}")))
            .header("Authorization", self.auth_header()?)
            .send()
            .await
            .with_context(|| format!("poll Kling task {task_id}"))?
            .error_for_status()
            .with_context(|| format!("Kling poll returned error for {task_id}"))?
            .json()
            .await
            .with_context(|| format!("decode Kling poll response for {task_id}"))?;

        Ok(map_kling_status(&status))
    }

    async fn download(&self, job: &ProviderJob) -> Result<Vec<String>> {
        let task_id = task_id(job)?;
        let status: Value = self
            .client
            .get(self.url(&format!("/v1/videos/{task_id}")))
            .header("Authorization", self.auth_header()?)
            .send()
            .await
            .with_context(|| format!("fetch Kling result {task_id}"))?
            .error_for_status()
            .with_context(|| format!("Kling result returned error for {task_id}"))?
            .json()
            .await
            .with_context(|| format!("decode Kling result for {task_id}"))?;
        let output_dir = output_dir(job)?;
        download_urls(&self.client, &output_dir, collect_video_urls(&status)).await
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
                "Kling local output paths verified".to_string()
            } else {
                format!("missing Kling local outputs: {}", missing.join(", "))
            },
        })
    }
}

fn parse_video_request(prompt: &str, input_paths: &[String]) -> Result<KlingVideoRequest> {
    let mut request: KlingVideoRequest =
        serde_json::from_str(prompt).context("Kling ProviderRequest.prompt must be JSON")?;
    if request.model.trim().is_empty() {
        request.model = "kling-v2.6-std".to_string();
    }
    if request.prompt.trim().is_empty() {
        bail!("Kling prompt cannot be empty");
    }
    apply_input_paths_to_kling_request(&mut request, input_paths)?;
    Ok(request)
}

fn apply_input_paths_to_kling_request(
    request: &mut KlingVideoRequest,
    input_paths: &[String],
) -> Result<()> {
    if input_paths.is_empty() || request.image.is_some() || request.image_url.is_some() {
        return Ok(());
    }
    let image_path = input_paths
        .first()
        .context("Kling input_paths unexpectedly empty")?;
    validate_local_image_path(image_path)?;
    request.image = Some(local_image_data_url(image_path)?);
    Ok(())
}

fn validate_local_image_path(path: &str) -> Result<()> {
    if path.trim().is_empty() {
        bail!("Kling image2video input path cannot be empty");
    }
    if path.contains("://") {
        bail!("Kling image2video input must be a local file path, not a URL");
    }
    if !Path::new(path).is_file() {
        bail!("Kling image2video input file does not exist: {path}");
    }
    Ok(())
}

fn local_image_data_url(path: &str) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("read Kling image2video input {path}"))?;
    Ok(format!(
        "data:{};base64,{}",
        image_mime_type(path),
        general_purpose::STANDARD.encode(bytes)
    ))
}

fn image_mime_type(path: &str) -> &'static str {
    match Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        _ => "image/png",
    }
}

fn to_request_body(request: KlingVideoRequest) -> Result<Value> {
    serde_json::to_value(request).context("serialize Kling request body")
}

fn metadata_request_body(body: &Value, input_paths: &[String]) -> Value {
    let mut metadata = body.clone();
    let image_is_data_url = metadata
        .get("image")
        .and_then(Value::as_str)
        .is_some_and(|image| image.starts_with("data:"));
    if image_is_data_url {
        if let Some(object) = metadata.as_object_mut() {
            object.insert("image".to_string(), json!("local_image_data_url_redacted"));
            object.insert("local_input_paths".to_string(), json!(input_paths));
        }
    }
    metadata
}

fn extract_task_id(response: &Value) -> Option<String> {
    response
        .get("task_id")
        .or_else(|| response.get("id"))
        .or_else(|| response.pointer("/data/task_id"))
        .or_else(|| response.pointer("/data/id"))
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn map_kling_status(response: &Value) -> TaskStatus {
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

fn collect_video_urls(response: &Value) -> Vec<String> {
    let mut urls = Vec::new();
    for pointer in ["/video_url", "/url", "/data/video_url", "/data/url"] {
        if let Some(url) = response.pointer(pointer).and_then(Value::as_str) {
            urls.push(url.to_string());
        }
    }
    for pointer in ["/videos", "/data/videos", "/outputs", "/data/outputs"] {
        let Some(items) = response.pointer(pointer).and_then(Value::as_array) else {
            continue;
        };
        for item in items {
            if let Some(url) = item
                .get("url")
                .or_else(|| item.get("video_url"))
                .and_then(Value::as_str)
            {
                urls.push(url.to_string());
            }
        }
    }
    urls.sort();
    urls.dedup();
    urls
}

async fn download_urls(
    client: &Client,
    output_dir: &Path,
    urls: Vec<String>,
) -> Result<Vec<String>> {
    fs::create_dir_all(output_dir)
        .with_context(|| format!("create Kling download dir {}", output_dir.display()))?;
    let mut local_paths = Vec::new();
    for (index, url) in urls.iter().enumerate() {
        let bytes = client
            .get(url)
            .send()
            .await
            .with_context(|| format!("download Kling output {url}"))?
            .error_for_status()
            .with_context(|| format!("Kling output download returned error for {url}"))?
            .bytes()
            .await
            .with_context(|| format!("read Kling output {url}"))?;
        let local_path = output_dir.join(format!("{}-kling-output.mp4", index + 1));
        fs::write(&local_path, bytes)
            .with_context(|| format!("write Kling output {}", local_path.display()))?;
        local_paths.push(local_path.to_string_lossy().to_string());
    }
    Ok(local_paths)
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
        .context("Kling job missing output_dir metadata")
}

fn task_id(job: &ProviderJob) -> Result<String> {
    job.external_job_id
        .clone()
        .context("Kling job missing task_id")
}

fn generate_kling_jwt(access_key: &str, secret_key: &str) -> Result<String> {
    #[derive(Serialize)]
    struct Claims<'a> {
        iss: &'a str,
        exp: usize,
        nbf: usize,
    }
    let now = chrono::Utc::now().timestamp() as usize;
    let claims = Claims {
        iss: access_key,
        exp: now + 30 * 60,
        nbf: now.saturating_sub(5),
    };
    encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(secret_key.as_bytes()),
    )
    .context("generate Kling JWT")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_text_to_video_request() {
        let request =
            parse_video_request(r#"{"prompt":"cinematic robot","duration":5}"#, &[]).unwrap();

        assert_eq!(request.model, "kling-v2.6-std");
        assert_eq!(request.prompt, "cinematic robot");
        assert!(request.image_url.is_none());
    }

    #[test]
    fn maps_image_request_to_image_endpoint_signal() {
        let request = parse_video_request(
            r#"{"model":"kling-v2.6-std","prompt":"animate","image_url":"https://example.com/a.png"}"#,
            &[],
        )
        .unwrap();

        assert!(request.image_url.is_some());
    }

    #[test]
    fn maps_provider_input_path_to_image2video_data_url_and_redacts_metadata() {
        let root = std::env::temp_dir().join(format!("pool-kling-input-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let image_path = root.join("concept.png");
        fs::write(&image_path, b"png-bytes").unwrap();
        let input_paths = vec![image_path.to_string_lossy().to_string()];
        let request =
            parse_video_request(r#"{"prompt":"animate this concept"}"#, &input_paths).unwrap();

        assert!(request
            .image
            .as_deref()
            .unwrap()
            .starts_with("data:image/png;base64,"));
        let body = to_request_body(request).unwrap();
        let metadata = metadata_request_body(&body, &input_paths);

        assert_eq!(metadata["image"], "local_image_data_url_redacted");
        assert_eq!(
            metadata["local_input_paths"][0].as_str(),
            Some(image_path.to_string_lossy().as_ref())
        );
        assert!(metadata.to_string().find("png-bytes").is_none());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_remote_input_path_before_submit() {
        let input_paths = vec!["https://example.com/concept.png".to_string()];
        let error = parse_video_request(r#"{"prompt":"animate"}"#, &input_paths).unwrap_err();

        assert!(error.to_string().contains("local file path"));
    }

    #[test]
    fn extracts_task_id_from_common_shapes() {
        assert_eq!(
            extract_task_id(&json!({"task_id":"abc"})).as_deref(),
            Some("abc")
        );
        assert_eq!(
            extract_task_id(&json!({"data":{"task_id":"def"}})).as_deref(),
            Some("def")
        );
    }

    #[test]
    fn maps_kling_status_values() {
        assert_eq!(
            map_kling_status(&json!({"status":"completed"})),
            TaskStatus::Succeeded
        );
        assert_eq!(
            map_kling_status(&json!({"data":{"status":"failed"}})),
            TaskStatus::Failed
        );
        assert_eq!(
            map_kling_status(&json!({"status":"processing"})),
            TaskStatus::Running
        );
    }

    #[test]
    fn collects_video_urls_from_result_shapes() {
        let urls = collect_video_urls(&json!({
            "video_url": "https://cdn.example/a.mp4",
            "data": {
                "videos": [{ "url": "https://cdn.example/b.mp4" }]
            }
        }));

        assert_eq!(urls.len(), 2);
        assert!(urls.contains(&"https://cdn.example/a.mp4".to_string()));
        assert!(urls.contains(&"https://cdn.example/b.mp4".to_string()));
    }

    #[test]
    fn generates_jwt_for_access_secret_auth() {
        let token = generate_kling_jwt("access", "secret").unwrap();
        assert_eq!(token.split('.').count(), 3);
    }
}
