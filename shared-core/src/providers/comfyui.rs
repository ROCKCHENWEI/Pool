use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use uuid::Uuid;

use crate::db::RuntimeRepository;
use crate::models::AssetRecord;
use crate::models::{ProviderConfig, ProviderKind, RuntimeEvent, RuntimeEventLevel, TaskStatus};
use crate::providers::{
    ProviderAdapter, ProviderHealth, ProviderJob, ProviderRequest, ProviderVerification,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComfyUiProviderOptions {
    pub endpoint: String,
    pub client_id: String,
}

impl Default for ComfyUiProviderOptions {
    fn default() -> Self {
        Self {
            endpoint: "http://127.0.0.1:8188".to_string(),
            client_id: format!("pool-{}", Uuid::new_v4()),
        }
    }
}

impl ComfyUiProviderOptions {
    pub fn from_env() -> Self {
        Self {
            endpoint: std::env::var("POOL_COMFYUI_ENDPOINT")
                .or_else(|_| std::env::var("COMFYUI_ENDPOINT"))
                .unwrap_or_else(|_| "http://127.0.0.1:8188".to_string()),
            client_id: std::env::var("POOL_COMFYUI_CLIENT_ID")
                .unwrap_or_else(|_| format!("pool-{}", Uuid::new_v4())),
        }
    }
}

pub struct ComfyUiProvider {
    config: ProviderConfig,
    options: ComfyUiProviderOptions,
    client: Client,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ComfyUiProgressEvent {
    pub prompt_id: String,
    pub event_type: String,
    pub node_id: Option<String>,
    pub value: Option<u64>,
    pub max: Option<u64>,
    pub status: TaskStatus,
    pub message: String,
}

impl ComfyUiProvider {
    pub fn new(options: ComfyUiProviderOptions) -> Self {
        Self {
            config: ProviderConfig {
                id: "comfyui".to_string(),
                display_name: "ComfyUI".to_string(),
                kind: ProviderKind::AiImage,
                endpoint: options.endpoint.clone(),
                auth_env_key: None,
                output_contract: "ComfyUI workflow JSON submitted over HTTP; outputs downloaded from /view into local output_dir".to_string(),
                high_cost: false,
            },
            options,
            client: Client::new(),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.options.endpoint.trim_end_matches('/'), path)
    }

    pub async fn stream_progress_events<F>(&self, job: &ProviderJob, mut on_event: F) -> Result<()>
    where
        F: FnMut(RuntimeEvent) -> Result<()>,
    {
        let prompt_id = prompt_id(job)?;
        let ws_url = job
            .metadata_json
            .as_ref()
            .and_then(|metadata| metadata.get("websocket_url"))
            .and_then(Value::as_str)
            .map(ToString::to_string)
            .unwrap_or_else(|| websocket_url(&self.options.endpoint, &self.options.client_id));
        let project_slug = job
            .metadata_json
            .as_ref()
            .and_then(|metadata| metadata.get("project_slug"))
            .and_then(Value::as_str)
            .unwrap_or("comfyui")
            .to_string();

        let (mut socket, _) = connect_async(&ws_url)
            .await
            .with_context(|| format!("connect ComfyUI websocket {ws_url}"))?;

        while let Some(message) = socket.next().await {
            let message = message.context("read ComfyUI websocket message")?;
            let text = match message {
                Message::Text(text) => text,
                Message::Binary(_) | Message::Ping(_) | Message::Pong(_) | Message::Frame(_) => {
                    continue;
                }
                Message::Close(_) => break,
            };
            let Some(progress) = parse_progress_message(&text, Some(&prompt_id))? else {
                continue;
            };
            let finished = matches!(progress.status, TaskStatus::Succeeded | TaskStatus::Failed);
            on_event(progress.to_runtime_event(&project_slug))?;
            if finished {
                break;
            }
        }

        Ok(())
    }

    pub async fn download_and_index(
        &self,
        job: &ProviderJob,
        repository: &RuntimeRepository,
        source_node_id: Option<&str>,
    ) -> Result<Vec<AssetRecord>> {
        let local_paths = self.download(job).await?;
        self.index_downloaded_paths(job, repository, source_node_id, &local_paths)
    }

    pub fn index_downloaded_paths(
        &self,
        job: &ProviderJob,
        repository: &RuntimeRepository,
        source_node_id: Option<&str>,
        local_paths: &[String],
    ) -> Result<Vec<AssetRecord>> {
        let project_slug =
            metadata_string(job, "project_slug").unwrap_or_else(|| "comfyui".to_string());
        let provider_url = metadata_string(job, "history_url");
        repository.index_local_outputs(
            &project_slug,
            source_node_id,
            provider_url.as_deref(),
            local_paths,
        )
    }
}

#[async_trait]
impl ProviderAdapter for ComfyUiProvider {
    fn config(&self) -> &ProviderConfig {
        &self.config
    }

    async fn health(&self) -> Result<ProviderHealth> {
        let response = self
            .client
            .get(self.url("/system_stats"))
            .send()
            .await
            .context("request ComfyUI /system_stats")?;

        let status = response.status();
        let ok = status.is_success();
        let message = if ok {
            "ComfyUI HTTP API ready".to_string()
        } else {
            format!("ComfyUI health returned HTTP {status}")
        };

        Ok(ProviderHealth {
            provider_id: self.config.id.clone(),
            status: if ok { "ready" } else { "unhealthy" }.to_string(),
            message,
        })
    }

    async fn submit(&self, request: ProviderRequest) -> Result<ProviderJob> {
        fs::create_dir_all(&request.output_dir)
            .with_context(|| format!("create ComfyUI output dir {}", request.output_dir))?;

        let body = build_prompt_payload(&request.prompt, &self.options.client_id)?;
        let response: Value = self
            .client
            .post(self.url("/prompt"))
            .json(&body)
            .send()
            .await
            .context("submit ComfyUI prompt")?
            .error_for_status()
            .context("ComfyUI /prompt returned error")?
            .json()
            .await
            .context("decode ComfyUI /prompt response")?;

        let prompt_id = response
            .get("prompt_id")
            .and_then(Value::as_str)
            .context("ComfyUI /prompt response missing prompt_id")?
            .to_string();
        let metadata_path =
            Path::new(&request.output_dir).join(format!(".comfyui-{prompt_id}-request.json"));
        let metadata = json!({
            "provider_id": self.config.id,
            "endpoint": self.options.endpoint,
            "client_id": self.options.client_id,
            "prompt_id": prompt_id,
            "project_slug": request.project_slug,
            "output_dir": request.output_dir,
            "history_url": self.url(&format!("/history/{prompt_id}")),
            "websocket_url": websocket_url(&self.options.endpoint, &self.options.client_id),
            "request": body,
            "response": response,
        });
        fs::write(
            &metadata_path,
            serde_json::to_string_pretty(&metadata)
                .context("serialize ComfyUI request metadata")?,
        )
        .with_context(|| format!("write ComfyUI metadata {}", metadata_path.display()))?;

        Ok(ProviderJob {
            provider_id: self.config.id.clone(),
            external_job_id: Some(prompt_id),
            status: TaskStatus::Running,
            request_metadata_path: metadata_path.to_string_lossy().to_string(),
            expected_outputs: Vec::new(),
            metadata_json: Some(metadata),
        })
    }

    async fn poll(&self, job: &ProviderJob) -> Result<TaskStatus> {
        let prompt_id = prompt_id(job)?;
        let history = self.fetch_history(&prompt_id).await?;
        let entry = history.get(&prompt_id);
        let completed = entry
            .and_then(|value| value.pointer("/status/completed"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let status_str = entry
            .and_then(|value| value.pointer("/status/status_str"))
            .and_then(Value::as_str)
            .unwrap_or_default();

        if status_str.eq_ignore_ascii_case("error") || status_str.eq_ignore_ascii_case("failed") {
            Ok(TaskStatus::Failed)
        } else if completed || !collect_output_files(&history, &prompt_id).is_empty() {
            Ok(TaskStatus::Succeeded)
        } else {
            Ok(TaskStatus::Running)
        }
    }

    async fn download(&self, job: &ProviderJob) -> Result<Vec<String>> {
        let prompt_id = prompt_id(job)?;
        let history = self.fetch_history(&prompt_id).await?;
        let output_dir = output_dir(job)?;
        fs::create_dir_all(&output_dir)
            .with_context(|| format!("create ComfyUI download dir {}", output_dir.display()))?;

        let mut local_paths = Vec::new();
        for file in collect_output_files(&history, &prompt_id) {
            let response = self
                .client
                .get(self.url("/view"))
                .query(&[
                    ("filename", file.filename.as_str()),
                    ("subfolder", file.subfolder.as_str()),
                    ("type", file.kind.as_str()),
                ])
                .send()
                .await
                .with_context(|| format!("download ComfyUI output {}", file.filename))?
                .error_for_status()
                .with_context(|| format!("ComfyUI /view returned error for {}", file.filename))?;
            let bytes = response
                .bytes()
                .await
                .with_context(|| format!("read ComfyUI output {}", file.filename))?;
            let local_path = output_dir.join(format!(
                "{}-{}",
                local_paths.len() + 1,
                sanitize_filename(&file.filename)
            ));
            fs::write(&local_path, bytes)
                .with_context(|| format!("write ComfyUI output {}", local_path.display()))?;
            local_paths.push(local_path.to_string_lossy().to_string());
        }

        Ok(local_paths)
    }

    async fn verify(&self, job: &ProviderJob) -> Result<ProviderVerification> {
        let paths = if job.expected_outputs.is_empty() {
            Vec::new()
        } else {
            job.expected_outputs.clone()
        };
        let missing: Vec<_> = paths
            .iter()
            .filter(|path| !Path::new(path.as_str()).exists())
            .cloned()
            .collect();

        Ok(ProviderVerification {
            ok: missing.is_empty(),
            local_paths: paths,
            message: if missing.is_empty() {
                "ComfyUI local output paths verified".to_string()
            } else {
                format!("missing ComfyUI local outputs: {}", missing.join(", "))
            },
        })
    }
}

impl ComfyUiProgressEvent {
    pub fn to_runtime_event(&self, project_slug: &str) -> RuntimeEvent {
        let level = match self.status {
            TaskStatus::Failed => RuntimeEventLevel::Error,
            TaskStatus::Succeeded => RuntimeEventLevel::Ok,
            _ => RuntimeEventLevel::Info,
        };
        RuntimeEvent::new(project_slug, level, self.message.clone())
    }
}

pub fn parse_progress_message(
    message: &str,
    expected_prompt_id: Option<&str>,
) -> Result<Option<ComfyUiProgressEvent>> {
    let value: Value = serde_json::from_str(message).context("decode ComfyUI websocket JSON")?;
    let Some(event_type) = value.get("type").and_then(Value::as_str) else {
        return Ok(None);
    };
    let data = value.get("data").unwrap_or(&Value::Null);
    let prompt_id = data
        .get("prompt_id")
        .and_then(Value::as_str)
        .or(expected_prompt_id)
        .unwrap_or_default()
        .to_string();

    if let Some(expected) = expected_prompt_id {
        if !prompt_id.is_empty() && prompt_id != expected {
            return Ok(None);
        }
    }

    let node_id = data
        .get("node")
        .and_then(Value::as_str)
        .filter(|node| !node.is_empty())
        .map(ToString::to_string);
    let value_num = data.get("value").and_then(Value::as_u64);
    let max = data.get("max").and_then(Value::as_u64);
    let status = match event_type {
        "execution_error" => TaskStatus::Failed,
        "executed" => TaskStatus::Succeeded,
        "executing" if data.get("node").is_some_and(Value::is_null) => TaskStatus::Succeeded,
        _ => TaskStatus::Running,
    };
    let message = match event_type {
        "execution_start" => format!("ComfyUI execution started: {prompt_id}"),
        "progress" => format!(
            "ComfyUI progress{}: {}/{}",
            node_id
                .as_ref()
                .map(|node| format!(" node {node}"))
                .unwrap_or_default(),
            value_num.unwrap_or(0),
            max.unwrap_or(0)
        ),
        "executing" => match &node_id {
            Some(node) => format!("ComfyUI executing node {node}"),
            None => format!("ComfyUI execution completed: {prompt_id}"),
        },
        "executed" => match &node_id {
            Some(node) => format!("ComfyUI executed node {node}"),
            None => format!("ComfyUI executed workflow: {prompt_id}"),
        },
        "execution_error" => format!("ComfyUI execution failed: {prompt_id}"),
        other => format!("ComfyUI event {other}: {prompt_id}"),
    };

    Ok(Some(ComfyUiProgressEvent {
        prompt_id,
        event_type: event_type.to_string(),
        node_id,
        value: value_num,
        max,
        status,
        message,
    }))
}

impl ComfyUiProvider {
    async fn fetch_history(&self, prompt_id: &str) -> Result<Value> {
        self.client
            .get(self.url(&format!("/history/{prompt_id}")))
            .send()
            .await
            .with_context(|| format!("request ComfyUI history for {prompt_id}"))?
            .error_for_status()
            .with_context(|| format!("ComfyUI /history returned error for {prompt_id}"))?
            .json()
            .await
            .with_context(|| format!("decode ComfyUI history for {prompt_id}"))
    }
}

fn build_prompt_payload(prompt: &str, client_id: &str) -> Result<Value> {
    let value: Value = serde_json::from_str(prompt)
        .context("ComfyUI ProviderRequest.prompt must be workflow JSON")?;
    if value.get("prompt").is_some() {
        let mut body = value;
        if body.get("client_id").is_none() {
            body["client_id"] = Value::String(client_id.to_string());
        }
        Ok(body)
    } else if value.is_object() {
        Ok(json!({
            "prompt": value,
            "client_id": client_id,
        }))
    } else {
        bail!("ComfyUI workflow JSON must be an object")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ComfyOutputFile {
    filename: String,
    subfolder: String,
    kind: String,
}

fn collect_output_files(history: &Value, prompt_id: &str) -> Vec<ComfyOutputFile> {
    let mut files = Vec::new();
    let Some(outputs) = history
        .get(prompt_id)
        .and_then(|value| value.get("outputs"))
        .and_then(Value::as_object)
    else {
        return files;
    };

    for node_output in outputs.values() {
        for key in ["images", "gifs", "videos"] {
            let Some(items) = node_output.get(key).and_then(Value::as_array) else {
                continue;
            };
            for item in items {
                let Some(filename) = item.get("filename").and_then(Value::as_str) else {
                    continue;
                };
                files.push(ComfyOutputFile {
                    filename: filename.to_string(),
                    subfolder: item
                        .get("subfolder")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    kind: item
                        .get("type")
                        .and_then(Value::as_str)
                        .unwrap_or("output")
                        .to_string(),
                });
            }
        }
    }

    files
}

fn prompt_id(job: &ProviderJob) -> Result<String> {
    job.external_job_id
        .clone()
        .context("ComfyUI job missing prompt_id")
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
        .context("ComfyUI job missing output_dir metadata")
}

fn metadata_string(job: &ProviderJob, key: &str) -> Option<String> {
    job.metadata_json
        .as_ref()
        .and_then(|metadata| metadata.get(key))
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn websocket_url(endpoint: &str, client_id: &str) -> String {
    let endpoint = endpoint.trim_end_matches('/');
    let ws_base = if let Some(rest) = endpoint.strip_prefix("https://") {
        format!("wss://{rest}")
    } else if let Some(rest) = endpoint.strip_prefix("http://") {
        format!("ws://{rest}")
    } else {
        endpoint.to_string()
    };
    format!("{ws_base}/ws?clientId={client_id}")
}

fn sanitize_filename(filename: &str) -> String {
    filename
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-') {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_raw_workflow_json_for_prompt_endpoint() {
        let body =
            build_prompt_payload(r#"{"1":{"class_type":"SaveImage"}}"#, "pool-test").unwrap();

        assert!(body.get("prompt").is_some());
        assert_eq!(body["client_id"], "pool-test");
    }

    #[test]
    fn preserves_existing_prompt_body_and_adds_client_id() {
        let body = build_prompt_payload(
            r#"{"prompt":{"1":{"class_type":"SaveImage"}}}"#,
            "pool-test",
        )
        .unwrap();

        assert_eq!(body["client_id"], "pool-test");
        assert!(body["prompt"].get("1").is_some());
    }

    #[test]
    fn collects_images_and_videos_from_history() {
        let history = json!({
            "abc": {
                "outputs": {
                    "9": {
                        "images": [{
                            "filename": "ComfyUI_00001_.png",
                            "subfolder": "",
                            "type": "output"
                        }],
                        "videos": [{
                            "filename": "preview.mp4",
                            "subfolder": "clips",
                            "type": "output"
                        }]
                    }
                }
            }
        });

        let files = collect_output_files(&history, "abc");
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].filename, "ComfyUI_00001_.png");
        assert_eq!(files[1].subfolder, "clips");
    }

    #[test]
    fn builds_websocket_url_from_http_endpoint() {
        assert_eq!(
            websocket_url("http://127.0.0.1:8188", "pool-test"),
            "ws://127.0.0.1:8188/ws?clientId=pool-test"
        );
    }

    #[test]
    fn parses_progress_websocket_message() {
        let event = parse_progress_message(
            r#"{"type":"progress","data":{"prompt_id":"abc","node":"7","value":3,"max":10}}"#,
            Some("abc"),
        )
        .unwrap()
        .unwrap();

        assert_eq!(event.prompt_id, "abc");
        assert_eq!(event.node_id.as_deref(), Some("7"));
        assert_eq!(event.value, Some(3));
        assert_eq!(event.max, Some(10));
        assert_eq!(event.status, TaskStatus::Running);
    }

    #[test]
    fn ignores_progress_for_different_prompt_id() {
        let event = parse_progress_message(
            r#"{"type":"progress","data":{"prompt_id":"other","node":"7","value":3,"max":10}}"#,
            Some("abc"),
        )
        .unwrap();

        assert!(event.is_none());
    }

    #[test]
    fn maps_completion_websocket_message_to_ok_runtime_event() {
        let event = parse_progress_message(
            r#"{"type":"executing","data":{"prompt_id":"abc","node":null}}"#,
            Some("abc"),
        )
        .unwrap()
        .unwrap();
        let runtime_event = event.to_runtime_event("demo");

        assert_eq!(event.status, TaskStatus::Succeeded);
        assert_eq!(runtime_event.project_slug, "demo");
        assert_eq!(runtime_event.level, RuntimeEventLevel::Ok);
    }

    #[test]
    fn indexes_downloaded_paths_from_job_metadata() {
        let repository = RuntimeRepository::in_memory().unwrap();
        repository.migrate().unwrap();
        let provider = ComfyUiProvider::new(ComfyUiProviderOptions::default());
        let job = ProviderJob {
            provider_id: "comfyui".to_string(),
            external_job_id: Some("abc".to_string()),
            status: TaskStatus::Succeeded,
            request_metadata_path: "target/comfyui/.comfyui-abc-request.json".to_string(),
            expected_outputs: Vec::new(),
            metadata_json: Some(json!({
                "project_slug": "demo",
                "history_url": "http://127.0.0.1:8188/history/abc"
            })),
        };
        let paths = vec![
            "worlds/demo/output/1-ComfyUI_00001_.png".to_string(),
            "worlds/demo/output/2-preview.mp4".to_string(),
        ];

        let assets = provider
            .index_downloaded_paths(&job, &repository, Some("node-comfyui"), &paths)
            .unwrap();

        assert_eq!(assets.len(), 2);
        assert_eq!(assets[0].project_slug, "demo");
        assert_eq!(assets[0].asset_type, "image");
        assert_eq!(
            assets[0].provider_url.as_deref(),
            Some("http://127.0.0.1:8188/history/abc")
        );
        assert_eq!(repository.stats().unwrap().assets, 2);
    }
}
