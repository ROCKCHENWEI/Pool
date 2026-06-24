use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use base64::{engine::general_purpose, Engine as _};
use reqwest::{multipart, Client};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::fs;
use std::path::Path;
use uuid::Uuid;

use crate::models::{ProviderConfig, ProviderKind, TaskStatus};
use crate::providers::{
    ProviderAdapter, ProviderHealth, ProviderJob, ProviderRequest, ProviderVerification,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiImageProviderOptions {
    pub endpoint: String,
    pub api_key: Option<String>,
    pub organization_id: Option<String>,
    pub project_id: Option<String>,
    pub default_model: String,
}

impl Default for OpenAiImageProviderOptions {
    fn default() -> Self {
        Self {
            endpoint: "https://api.openai.com/v1".to_string(),
            api_key: None,
            organization_id: None,
            project_id: None,
            default_model: "gpt-image-2".to_string(),
        }
    }
}

impl OpenAiImageProviderOptions {
    pub fn from_env() -> Self {
        Self {
            endpoint: std::env::var("POOL_OPENAI_ENDPOINT")
                .unwrap_or_else(|_| "https://api.openai.com/v1".to_string()),
            api_key: std::env::var("OPENAI_API_KEY").ok(),
            organization_id: std::env::var("OPENAI_ORG_ID")
                .ok()
                .or_else(|| std::env::var("OPENAI_ORGANIZATION").ok()),
            project_id: std::env::var("OPENAI_PROJECT_ID").ok(),
            default_model: std::env::var("POOL_OPENAI_IMAGE_MODEL")
                .unwrap_or_else(|_| "gpt-image-2".to_string()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiImageRequest {
    #[serde(default)]
    pub model: String,
    pub prompt: String,
    pub n: Option<u64>,
    pub size: Option<String>,
    pub quality: Option<String>,
    pub background: Option<String>,
    pub output_format: Option<String>,
    pub output_compression: Option<u8>,
    pub moderation: Option<String>,
    pub response_format: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OpenAiImageOperation {
    Generate,
    Edit,
}

pub struct OpenAiImageProvider {
    config: ProviderConfig,
    options: OpenAiImageProviderOptions,
    client: Client,
}

impl OpenAiImageProvider {
    pub fn new(options: OpenAiImageProviderOptions) -> Self {
        Self {
            config: ProviderConfig {
                id: "openai-image-2".to_string(),
                display_name: "OpenAI image-2".to_string(),
                kind: ProviderKind::AiImage,
                endpoint: format!(
                    "{}/images/generations",
                    options.endpoint.trim_end_matches('/')
                ),
                auth_env_key: Some("OPENAI_API_KEY".to_string()),
                output_contract: "OpenAI Images API response saved as local indexed image files"
                    .to_string(),
                high_cost: false,
            },
            options,
            client: Client::new(),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.options.endpoint.trim_end_matches('/'), path)
    }
}

#[async_trait]
impl ProviderAdapter for OpenAiImageProvider {
    fn config(&self) -> &ProviderConfig {
        &self.config
    }

    async fn health(&self) -> Result<ProviderHealth> {
        Ok(ProviderHealth {
            provider_id: self.config.id.clone(),
            status: if self.options.api_key.is_some() {
                "ready"
            } else {
                "missing_auth"
            }
            .to_string(),
            message: "OpenAI image adapter configured; network health is checked on submit"
                .to_string(),
        })
    }

    async fn submit(&self, request: ProviderRequest) -> Result<ProviderJob> {
        fs::create_dir_all(&request.output_dir)
            .with_context(|| format!("create OpenAI image output dir {}", request.output_dir))?;

        let image_request = parse_image_request(
            &request.prompt,
            &self.options.default_model,
            &request.input_paths,
        )?;
        let operation = image_request_operation(&image_request)?;
        let output_format = image_request
            .output_format
            .as_deref()
            .unwrap_or("png")
            .to_string();
        let api_key = self
            .options
            .api_key
            .as_deref()
            .context("OpenAI auth missing: set OPENAI_API_KEY")?;

        let (operation_name, endpoint_path, request_body, response) = match operation {
            OpenAiImageOperation::Generate => {
                let body = to_request_body(image_request)?;
                let response = self
                    .client
                    .post(self.url("/images/generations"))
                    .bearer_auth(api_key)
                    .header("Content-Type", "application/json")
                    .headers(optional_openai_headers(
                        self.options.organization_id.as_deref(),
                        self.options.project_id.as_deref(),
                    )?)
                    .json(&body)
                    .send()
                    .await
                    .context("submit OpenAI image generation")?
                    .error_for_status()
                    .context("OpenAI image generation returned error")?;
                (
                    "generate".to_string(),
                    "/images/generations".to_string(),
                    body,
                    response,
                )
            }
            OpenAiImageOperation::Edit => {
                let (form, body) = to_edit_multipart_form(&image_request).await?;
                let response = self
                    .client
                    .post(self.url("/images/edits"))
                    .bearer_auth(api_key)
                    .headers(optional_openai_headers(
                        self.options.organization_id.as_deref(),
                        self.options.project_id.as_deref(),
                    )?)
                    .multipart(form)
                    .send()
                    .await
                    .context("submit OpenAI image edit")?
                    .error_for_status()
                    .context("OpenAI image edit returned error")?;
                (
                    "edit".to_string(),
                    "/images/edits".to_string(),
                    body,
                    response,
                )
            }
        };

        let request_id = response
            .headers()
            .get("x-request-id")
            .and_then(|value| value.to_str().ok())
            .map(ToString::to_string)
            .unwrap_or_else(|| format!("openai-image-{}", Uuid::new_v4()));
        let response_json: Value = response
            .json()
            .await
            .context("decode OpenAI image generation response")?;
        let output_dir = Path::new(&request.output_dir);
        let local_paths =
            save_openai_outputs(&self.client, output_dir, &response_json, &output_format).await?;
        let metadata_path = output_dir.join(format!(".openai-image-{request_id}-request.json"));
        let metadata = json!({
            "provider_id": self.config.id,
            "endpoint": self.options.endpoint,
            "endpoint_path": endpoint_path,
            "operation": operation_name,
            "request_id": request_id,
            "project_slug": request.project_slug,
            "output_dir": request.output_dir,
            "request": request_body,
            "response_summary": summarize_response(&response_json),
            "local_paths": local_paths,
        });
        fs::write(
            &metadata_path,
            serde_json::to_string_pretty(&metadata)
                .context("serialize OpenAI image request metadata")?,
        )
        .with_context(|| {
            format!(
                "write OpenAI image metadata {}",
                metadata_path.to_string_lossy()
            )
        })?;

        Ok(ProviderJob {
            provider_id: self.config.id.clone(),
            external_job_id: Some(request_id),
            status: TaskStatus::Succeeded,
            request_metadata_path: metadata_path.to_string_lossy().to_string(),
            expected_outputs: local_paths,
            metadata_json: Some(metadata),
        })
    }

    async fn poll(&self, job: &ProviderJob) -> Result<TaskStatus> {
        Ok(job.status.clone())
    }

    async fn download(&self, job: &ProviderJob) -> Result<Vec<String>> {
        Ok(local_paths(job))
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
                "OpenAI image local output paths verified".to_string()
            } else {
                format!("missing OpenAI image local outputs: {}", missing.join(", "))
            },
        })
    }
}

fn optional_openai_headers(
    organization_id: Option<&str>,
    project_id: Option<&str>,
) -> Result<reqwest::header::HeaderMap> {
    let mut headers = reqwest::header::HeaderMap::new();
    if let Some(organization_id) = organization_id {
        headers.insert("OpenAI-Organization", organization_id.parse()?);
    }
    if let Some(project_id) = project_id {
        headers.insert("OpenAI-Project", project_id.parse()?);
    }
    Ok(headers)
}

fn parse_image_request(
    prompt: &str,
    default_model: &str,
    input_paths: &[String],
) -> Result<OpenAiImageRequest> {
    let trimmed = prompt.trim();
    let mut request: OpenAiImageRequest = if trimmed.starts_with('{') {
        serde_json::from_str(trimmed).context("OpenAI image ProviderRequest.prompt must be JSON")?
    } else {
        OpenAiImageRequest {
            model: String::new(),
            prompt: trimmed.to_string(),
            n: None,
            size: None,
            quality: None,
            background: None,
            output_format: None,
            output_compression: None,
            moderation: None,
            response_format: None,
            extra: Map::new(),
        }
    };
    if request.model.trim().is_empty() {
        request.model = default_model.to_string();
    }
    if request.prompt.trim().is_empty() {
        bail!("OpenAI image prompt cannot be empty");
    }
    apply_input_paths_to_image_request(&mut request, input_paths);
    Ok(request)
}

fn apply_input_paths_to_image_request(request: &mut OpenAiImageRequest, input_paths: &[String]) {
    if input_paths.is_empty() || request.extra.contains_key("image") {
        return;
    }
    if request.extra.contains_key("images") || request.extra.contains_key("input_images") {
        return;
    }
    let explicit_generate = request
        .extra
        .get("operation")
        .and_then(Value::as_str)
        .is_some_and(|operation| {
            matches!(operation, "generate" | "generation" | "images.generate")
        });
    if explicit_generate {
        return;
    }
    request
        .extra
        .insert("input_images".to_string(), json!(input_paths));
}

fn image_request_operation(request: &OpenAiImageRequest) -> Result<OpenAiImageOperation> {
    if let Some(operation) = request.extra.get("operation").and_then(Value::as_str) {
        return match operation {
            "generate" | "generation" | "images.generate" => Ok(OpenAiImageOperation::Generate),
            "edit" | "edits" | "images.edit" => Ok(OpenAiImageOperation::Edit),
            _ => bail!("unsupported OpenAI image operation: {operation}"),
        };
    }
    if request.extra.contains_key("image")
        || request.extra.contains_key("images")
        || request.extra.contains_key("input_images")
        || request.extra.contains_key("mask")
    {
        return Ok(OpenAiImageOperation::Edit);
    }
    Ok(OpenAiImageOperation::Generate)
}

fn to_request_body(request: OpenAiImageRequest) -> Result<Value> {
    serde_json::to_value(request).context("serialize OpenAI image request body")
}

async fn to_edit_multipart_form(request: &OpenAiImageRequest) -> Result<(multipart::Form, Value)> {
    let image_paths = edit_image_paths(request)?;
    let mask_path = edit_mask_path(request)?;
    let mut body = to_request_body(request.clone())?;
    remove_edit_file_fields(&mut body);

    let mut form = multipart::Form::new();
    let object = body
        .as_object()
        .context("OpenAI image edit request body must be an object")?;
    for (key, value) in object {
        if value.is_null() {
            continue;
        }
        form = form.text(key.clone(), multipart_field_value(value)?);
    }
    for image_path in &image_paths {
        form = form.part(
            "image",
            multipart_file_part("image", image_path)
                .with_context(|| format!("attach OpenAI edit image {image_path}"))?,
        );
    }
    if let Some(mask_path) = &mask_path {
        form = form.part(
            "mask",
            multipart_file_part("mask", mask_path)
                .with_context(|| format!("attach OpenAI edit mask {mask_path}"))?,
        );
    }

    json_object_insert(&mut body, "operation", json!("edit"));
    json_object_insert(&mut body, "image_paths", json!(image_paths));
    if let Some(mask_path) = mask_path {
        json_object_insert(&mut body, "mask_path", json!(mask_path));
    }
    Ok((form, body))
}

fn remove_edit_file_fields(body: &mut Value) {
    if let Some(object) = body.as_object_mut() {
        object.remove("operation");
        object.remove("image");
        object.remove("images");
        object.remove("input_images");
        object.remove("mask");
    }
}

fn edit_image_paths(request: &OpenAiImageRequest) -> Result<Vec<String>> {
    let mut paths = Vec::new();
    for key in ["image", "images", "input_images"] {
        if let Some(value) = request.extra.get(key) {
            collect_local_file_paths(value, key, &mut paths)?;
        }
    }
    paths.sort();
    paths.dedup();
    if paths.is_empty() {
        bail!("OpenAI image edit requires image, images, or input_images local file path(s)");
    }
    Ok(paths)
}

fn edit_mask_path(request: &OpenAiImageRequest) -> Result<Option<String>> {
    let Some(value) = request.extra.get("mask") else {
        return Ok(None);
    };
    let mut paths = Vec::new();
    collect_local_file_paths(value, "mask", &mut paths)?;
    if paths.len() > 1 {
        bail!("OpenAI image edit mask must be a single local file path");
    }
    Ok(paths.pop())
}

fn collect_local_file_paths(value: &Value, field: &str, paths: &mut Vec<String>) -> Result<()> {
    match value {
        Value::String(path) => {
            validate_local_file_path(field, path)?;
            paths.push(path.to_string());
        }
        Value::Array(items) => {
            for item in items {
                collect_local_file_paths(item, field, paths)?;
            }
        }
        _ => bail!("OpenAI image edit {field} must be a local file path string or array"),
    }
    Ok(())
}

fn validate_local_file_path(field: &str, path: &str) -> Result<()> {
    if path.trim().is_empty() {
        bail!("OpenAI image edit {field} path cannot be empty");
    }
    if path.contains("://") {
        bail!("OpenAI image edit {field} must be a local file path, not a URL");
    }
    if !Path::new(path).is_file() {
        bail!("OpenAI image edit {field} file does not exist: {path}");
    }
    Ok(())
}

fn multipart_field_value(value: &Value) -> Result<String> {
    Ok(match value {
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        Value::Bool(flag) => flag.to_string(),
        Value::Array(_) | Value::Object(_) => serde_json::to_string(value)?,
        Value::Null => String::new(),
    })
}

fn multipart_file_part(field: &str, path: &str) -> Result<multipart::Part> {
    let bytes = fs::read(path).with_context(|| format!("read OpenAI image edit {field} {path}"))?;
    let file_name = Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(field)
        .to_string();
    Ok(multipart::Part::bytes(bytes).file_name(file_name))
}

fn json_object_insert(body: &mut Value, key: &str, value: Value) {
    if let Some(object) = body.as_object_mut() {
        object.insert(key.to_string(), value);
    }
}

async fn save_openai_outputs(
    client: &Client,
    output_dir: &Path,
    response: &Value,
    output_format: &str,
) -> Result<Vec<String>> {
    let items = response
        .get("data")
        .and_then(Value::as_array)
        .context("OpenAI image response missing data[]")?;
    let extension = normalize_image_extension(output_format);
    let mut paths = Vec::new();
    for (index, item) in items.iter().enumerate() {
        let bytes = if let Some(encoded) = item.get("b64_json").and_then(Value::as_str) {
            decode_base64_image(encoded)?
        } else if let Some(url) = item.get("url").and_then(Value::as_str) {
            client
                .get(url)
                .send()
                .await
                .with_context(|| format!("download OpenAI image output {url}"))?
                .error_for_status()
                .with_context(|| format!("OpenAI image output download returned error for {url}"))?
                .bytes()
                .await
                .with_context(|| format!("read OpenAI image output {url}"))?
                .to_vec()
        } else {
            continue;
        };
        let local_path = output_dir.join(format!("{}-openai-image.{extension}", index + 1));
        fs::write(&local_path, bytes)
            .with_context(|| format!("write OpenAI image output {}", local_path.display()))?;
        paths.push(local_path.to_string_lossy().to_string());
    }
    if paths.is_empty() {
        bail!("OpenAI image response did not contain b64_json or url outputs");
    }
    Ok(paths)
}

fn decode_base64_image(encoded: &str) -> Result<Vec<u8>> {
    let payload = encoded
        .split_once(',')
        .map(|(_, payload)| payload)
        .unwrap_or(encoded);
    general_purpose::STANDARD
        .decode(payload)
        .context("decode OpenAI b64_json image")
}

fn normalize_image_extension(output_format: &str) -> &'static str {
    match output_format.to_ascii_lowercase().as_str() {
        "jpeg" | "jpg" => "jpg",
        "webp" => "webp",
        _ => "png",
    }
}

fn summarize_response(response: &Value) -> Value {
    let outputs = response
        .get("data")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| {
                    json!({
                        "has_b64_json": item.get("b64_json").is_some(),
                        "url": item.get("url").and_then(Value::as_str),
                        "revised_prompt": item.get("revised_prompt").and_then(Value::as_str),
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    json!({
        "created": response.get("created"),
        "usage": response.get("usage"),
        "outputs": outputs,
    })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_prompt_with_default_model() {
        let request = parse_image_request("matte painting of a stage", "gpt-image-2", &[]).unwrap();

        assert_eq!(request.model, "gpt-image-2");
        assert_eq!(request.prompt, "matte painting of a stage");
    }

    #[test]
    fn parses_json_request_and_preserves_extra_fields() {
        let request = parse_image_request(
            r#"{"prompt":"poster","model":"gpt-image-2","size":"1024x1024","style_ref":"neon"}"#,
            "gpt-image-2",
            &[],
        )
        .unwrap();
        let body = to_request_body(request).unwrap();

        assert_eq!(body["model"], "gpt-image-2");
        assert_eq!(body["style_ref"], "neon");
    }

    #[test]
    fn detects_edit_operation_from_explicit_operation_or_image_field() {
        let explicit = parse_image_request(
            r#"{"operation":"edit","prompt":"extend the set","image":"target/input.png"}"#,
            "gpt-image-2",
            &[],
        )
        .unwrap();
        assert_eq!(
            image_request_operation(&explicit).unwrap(),
            OpenAiImageOperation::Edit
        );

        let inferred = parse_image_request(
            r#"{"prompt":"extend the set","images":["target/a.png","target/b.png"]}"#,
            "gpt-image-2",
            &[],
        )
        .unwrap();
        assert_eq!(
            image_request_operation(&inferred).unwrap(),
            OpenAiImageOperation::Edit
        );
    }

    #[test]
    fn maps_provider_input_paths_to_edit_images_unless_generate_is_explicit() {
        let input_paths = vec!["worlds/demo/source/0-reference.png".to_string()];
        let inferred =
            parse_image_request("extend this reference", "gpt-image-2", &input_paths).unwrap();

        assert_eq!(
            image_request_operation(&inferred).unwrap(),
            OpenAiImageOperation::Edit
        );
        assert_eq!(inferred.extra["input_images"][0], input_paths[0]);

        let explicit_generate = parse_image_request(
            r#"{"operation":"generate","prompt":"new image"}"#,
            "gpt-image-2",
            &input_paths,
        )
        .unwrap();

        assert_eq!(
            image_request_operation(&explicit_generate).unwrap(),
            OpenAiImageOperation::Generate
        );
        assert!(explicit_generate.extra.get("input_images").is_none());
    }

    #[tokio::test]
    async fn builds_edit_multipart_metadata_without_embedding_image_bytes() {
        let root = std::env::temp_dir().join(format!("pool-openai-edit-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let image_path = root.join("input.png");
        let mask_path = root.join("mask.png");
        fs::write(&image_path, b"image-bytes").unwrap();
        fs::write(&mask_path, b"mask-bytes").unwrap();
        let request = parse_image_request(
            &json!({
                "operation": "edit",
                "model": "gpt-image-2",
                "prompt": "replace the backdrop",
                "image": image_path.to_string_lossy(),
                "mask": mask_path.to_string_lossy(),
                "size": "1024x1024",
                "output_format": "png"
            })
            .to_string(),
            "gpt-image-2",
            &[],
        )
        .unwrap();

        let (_form, body) = to_edit_multipart_form(&request).await.unwrap();

        assert_eq!(body["operation"], "edit");
        assert_eq!(body["prompt"], "replace the backdrop");
        assert_eq!(
            body["image_paths"][0].as_str(),
            Some(image_path.to_string_lossy().as_ref())
        );
        assert_eq!(
            body["mask_path"].as_str(),
            Some(mask_path.to_string_lossy().as_ref())
        );
        assert!(body.get("image").is_none());
        assert!(body.get("mask").is_none());
        assert!(body.to_string().find("image-bytes").is_none());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_edit_url_inputs_before_submit() {
        let request = parse_image_request(
            r#"{"operation":"edit","prompt":"replace the backdrop","image":"https://example.com/input.png"}"#,
            "gpt-image-2",
            &[],
        )
        .unwrap();

        let error = edit_image_paths(&request).unwrap_err();

        assert!(error.to_string().contains("local file path"));
    }

    #[test]
    fn rejects_empty_prompt() {
        assert!(parse_image_request("   ", "gpt-image-2", &[]).is_err());
    }

    #[test]
    fn decodes_raw_and_data_url_base64() {
        let raw = general_purpose::STANDARD.encode("png-bytes");
        assert_eq!(decode_base64_image(&raw).unwrap(), b"png-bytes");
        assert_eq!(
            decode_base64_image(&format!("data:image/png;base64,{raw}")).unwrap(),
            b"png-bytes"
        );
    }

    #[test]
    fn normalizes_image_extensions() {
        assert_eq!(normalize_image_extension("jpeg"), "jpg");
        assert_eq!(normalize_image_extension("webp"), "webp");
        assert_eq!(normalize_image_extension("png"), "png");
        assert_eq!(normalize_image_extension("unknown"), "png");
    }

    #[test]
    fn summarizes_response_without_persisting_base64_payload() {
        let summary = summarize_response(&json!({
            "created": 1,
            "data": [{
                "b64_json": "abc",
                "revised_prompt": "better poster"
            }],
            "usage": { "total_tokens": 42 }
        }));

        assert_eq!(summary["outputs"][0]["has_b64_json"], true);
        assert!(summary.to_string().find("abc").is_none());
        assert_eq!(summary["usage"]["total_tokens"], 42);
    }

    #[test]
    fn pulls_local_paths_from_job_metadata() {
        let job = ProviderJob {
            provider_id: "openai-image-2".to_string(),
            external_job_id: Some("req".to_string()),
            status: TaskStatus::Succeeded,
            request_metadata_path: ".openai-image-req-request.json".to_string(),
            expected_outputs: Vec::new(),
            metadata_json: Some(json!({"local_paths":["out/1-openai-image.png"]})),
        };

        assert_eq!(local_paths(&job), vec!["out/1-openai-image.png"]);
    }
}
