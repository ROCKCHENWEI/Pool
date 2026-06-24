use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Deserialize)]
struct DesktopRecognitionQueue {
    #[serde(default)]
    requests: Vec<DesktopRecognitionRequest>,
}

#[derive(Debug, Deserialize)]
struct DesktopRecognitionRequest {
    software_action_id: String,
    task_id: Option<String>,
    adapter_id: Option<String>,
    action_kind: Option<String>,
    desktop_request_path: Option<String>,
    pool_desktop_action: Option<Value>,
    desktop_payload: Option<Value>,
}

#[derive(Debug)]
struct RequestExecution {
    status: String,
    message: String,
    artifacts: Vec<String>,
    result: Value,
}

#[derive(Debug, Serialize)]
struct DesktopRecognitionResult<'a> {
    software_action_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    task_id: Option<&'a str>,
    status: &'a str,
    message: String,
    artifacts: Vec<String>,
    result: Value,
}

#[tokio::main]
async fn main() -> Result<()> {
    let options = ControllerOptions::from_args(std::env::args().skip(1));
    let client = reqwest::Client::new();
    let queue_url = format!(
        "{}/api/desktop-recognition/requests{}",
        options.runtime_base_url, options.project_query
    );
    let queue: DesktopRecognitionQueue = client
        .get(&queue_url)
        .send()
        .await
        .with_context(|| format!("GET {queue_url}"))?
        .error_for_status()
        .with_context(|| format!("desktop recognition queue failed: {queue_url}"))?
        .json()
        .await
        .context("parse desktop recognition queue")?;

    println!("runtime={}", options.runtime_base_url);
    println!("queued_requests={}", queue.requests.len());

    if queue.requests.is_empty() {
        write_desktop_vision_evidence_bundle(&options, &[])?;
        return Ok(());
    }

    let result_url = format!(
        "{}/api/desktop-recognition/results",
        options.runtime_base_url
    );
    let mut production_evidence_items = Vec::new();
    for request in queue.requests.iter().take(options.limit) {
        let execution = execute_request(request, &options, &client).await;
        let body = DesktopRecognitionResult {
            software_action_id: &request.software_action_id,
            task_id: request.task_id.as_deref(),
            status: &execution.status,
            message: execution.message,
            artifacts: execution.artifacts,
            result: execution.result,
        };
        let value: Value = client
            .post(&result_url)
            .json(&body)
            .send()
            .await
            .with_context(|| format!("POST {result_url}"))?
            .error_for_status()
            .with_context(|| {
                format!(
                    "desktop recognition result callback failed: {}",
                    request.software_action_id
                )
            })?
            .json()
            .await
            .context("parse desktop recognition result response")?;
        let task_status = value
            .get("task")
            .and_then(|task| task.get("status"))
            .and_then(Value::as_str)
            .unwrap_or("none");
        println!(
            "completed action={} status={} task_status={}",
            request.software_action_id, body.status, task_status
        );
        if let Some(item) =
            desktop_vision_production_evidence_item(request, &options, &body, &value)?
        {
            production_evidence_items.push(item);
        }
    }
    write_desktop_vision_evidence_bundle(&options, &production_evidence_items)?;
    println!(
        "desktop_vision_production_evidence_bundle={} desktop_vision={}",
        options.evidence_bundle_path(),
        production_evidence_items.len()
    );

    Ok(())
}

#[derive(Debug)]
struct ControllerOptions {
    runtime_base_url: String,
    project_query: String,
    status: String,
    controller_id: String,
    limit: usize,
    mode: ControllerMode,
    osascript_path: String,
    vision_trace_path: Option<String>,
    vision_trace_output_path: Option<String>,
    vision_endpoint: Option<String>,
    vision_api_key: Option<String>,
    production_attestation: Option<String>,
    evidence_bundle_path: Option<String>,
    project_slug: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ControllerMode {
    DryRun,
    AppleScript,
    VisionHttp,
}

impl ControllerMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::DryRun => "dry_run",
            Self::AppleScript => "applescript",
            Self::VisionHttp => "vision_http",
        }
    }
}

impl ControllerOptions {
    fn from_args(args: impl IntoIterator<Item = String>) -> Self {
        let mut runtime_base_url = "http://127.0.0.1:4788".to_string();
        let mut project = Some("demo".to_string());
        let mut status = "succeeded".to_string();
        let mut controller_id = "desktop-controller-smoke".to_string();
        let mut limit = usize::MAX;
        let mut mode = ControllerMode::DryRun;
        let mut osascript_path =
            std::env::var("POOL_DESKTOP_OSASCRIPT").unwrap_or_else(|_| "osascript".to_string());
        let mut vision_trace_path = std::env::var("POOL_DESKTOP_VISION_TRACE").ok();
        let mut vision_trace_output_path = std::env::var("POOL_DESKTOP_VISION_TRACE_OUTPUT").ok();
        let mut vision_endpoint = std::env::var("POOL_DESKTOP_VISION_ENDPOINT").ok();
        let mut vision_api_key = std::env::var("POOL_DESKTOP_VISION_API_KEY").ok();
        let mut production_attestation =
            std::env::var("POOL_DESKTOP_VISION_PRODUCTION_ATTESTATION").ok();
        let mut evidence_bundle_path = std::env::var("POOL_DESKTOP_VISION_EVIDENCE_BUNDLE").ok();

        for arg in args {
            if let Some(value) = arg.strip_prefix("--project=") {
                project = Some(value.to_string());
            } else if arg == "--all-projects" {
                project = Some("*".to_string());
            } else if let Some(value) = arg.strip_prefix("--status=") {
                status = value.to_string();
            } else if let Some(value) = arg.strip_prefix("--controller-id=") {
                controller_id = value.to_string();
            } else if let Some(value) = arg.strip_prefix("--limit=") {
                limit = value.parse().unwrap_or(usize::MAX);
            } else if let Some(value) = arg.strip_prefix("--mode=") {
                mode = match value {
                    "applescript" | "apple-script" | "execute" => ControllerMode::AppleScript,
                    "vision-http" | "vision_http" | "vision" => ControllerMode::VisionHttp,
                    _ => ControllerMode::DryRun,
                };
            } else if arg == "--execute" {
                mode = ControllerMode::AppleScript;
            } else if arg == "--vision-http" {
                mode = ControllerMode::VisionHttp;
            } else if let Some(value) = arg.strip_prefix("--osascript=") {
                osascript_path = value.to_string();
            } else if let Some(value) = arg.strip_prefix("--vision-trace=") {
                vision_trace_path = Some(value.to_string());
            } else if let Some(value) = arg.strip_prefix("--vision-trace-output=") {
                vision_trace_output_path = Some(value.to_string());
            } else if let Some(value) = arg.strip_prefix("--vision-endpoint=") {
                vision_endpoint = Some(value.to_string());
            } else if let Some(value) = arg.strip_prefix("--vision-api-key=") {
                vision_api_key = Some(value.to_string());
            } else if let Some(value) = arg.strip_prefix("--vision-api-key-env=") {
                vision_api_key = std::env::var(value).ok();
            } else if let Some(value) = arg.strip_prefix("--production-attestation=") {
                production_attestation = Some(value.to_string());
            } else if let Some(value) = arg.strip_prefix("--production-attestation-env=") {
                production_attestation = std::env::var(value).ok();
            } else if let Some(value) = arg.strip_prefix("--evidence-bundle=") {
                evidence_bundle_path = Some(value.to_string());
            } else if !arg.trim().is_empty() {
                runtime_base_url = normalize_runtime_base_url(&arg);
            }
        }

        let project_slug = project
            .as_ref()
            .filter(|project| project.as_str() != "*")
            .filter(|project| !project.trim().is_empty())
            .cloned()
            .unwrap_or_else(|| "demo".to_string());
        let project_query = project
            .filter(|project| !project.trim().is_empty())
            .map(|project| format!("?project={}", url_query_value(&project)))
            .unwrap_or_default();

        Self {
            runtime_base_url: normalize_runtime_base_url(&runtime_base_url),
            project_query,
            status,
            controller_id,
            limit,
            mode,
            osascript_path,
            vision_trace_path,
            vision_trace_output_path,
            vision_endpoint,
            vision_api_key,
            production_attestation,
            evidence_bundle_path,
            project_slug,
        }
    }

    fn evidence_bundle_path(&self) -> String {
        self.evidence_bundle_path.clone().unwrap_or_else(|| {
            "target/pool-desktop-vision-evidence/desktop-vision-production-evidence-bundle.json"
                .to_string()
        })
    }
}

async fn execute_request(
    request: &DesktopRecognitionRequest,
    options: &ControllerOptions,
    client: &reqwest::Client,
) -> RequestExecution {
    match options.mode {
        ControllerMode::DryRun => dry_run_execution(request, options),
        ControllerMode::AppleScript => applescript_execution(request, options),
        ControllerMode::VisionHttp => vision_http_execution(request, options, client).await,
    }
}

fn dry_run_execution(
    request: &DesktopRecognitionRequest,
    options: &ControllerOptions,
) -> RequestExecution {
    RequestExecution {
        status: options.status.clone(),
        message: format!(
            "desktop recognition controller dry-run: {} {:?}",
            options.status, request.action_kind
        ),
        artifacts: request_artifacts(request, options),
        result: json!({
            "controller": options.controller_id,
            "mode": options.mode.as_str(),
            "adapter_id": request.adapter_id,
            "action_kind": request.action_kind,
            "vision_trace_path": options.vision_trace_path,
            "pool_desktop_action": request.pool_desktop_action,
            "desktop_payload": request.desktop_payload,
        }),
    }
}

fn applescript_execution(
    request: &DesktopRecognitionRequest,
    options: &ControllerOptions,
) -> RequestExecution {
    let vision_trace = match load_vision_trace(options) {
        Ok(trace) => trace,
        Err(error) => {
            return RequestExecution {
                status: "failed".to_string(),
                message: format!("AppleScript controller could not read vision trace: {error}"),
                artifacts: request_artifacts(request, options),
                result: json!({
                    "controller": options.controller_id,
                    "mode": options.mode.as_str(),
                    "adapter_id": request.adapter_id,
                    "action_kind": request.action_kind,
                    "vision_trace_path": options.vision_trace_path,
                    "pool_desktop_action": request.pool_desktop_action,
                    "desktop_payload": request.desktop_payload,
                    "error": "invalid_vision_trace",
                }),
            };
        }
    };
    let plan = build_applescript_plan(request, vision_trace.as_ref());
    if !plan.has_action {
        return RequestExecution {
            status: "failed".to_string(),
            message: "AppleScript controller found no executable desktop primitive; provide target_window plus coordinates, hotkey, text, or a vision trace resolving visual_targets.".to_string(),
            artifacts: request_artifacts(request, options),
            result: json!({
                "controller": options.controller_id,
                "mode": options.mode.as_str(),
                "adapter_id": request.adapter_id,
                "action_kind": request.action_kind,
                "vision_trace_path": options.vision_trace_path,
                "pool_desktop_action": request.pool_desktop_action,
                "desktop_payload": request.desktop_payload,
                "planned_steps": plan.steps.iter().map(AppleScriptStep::summary).collect::<Vec<_>>(),
                "error": "no_executable_desktop_primitive",
            }),
        };
    }

    let mut reports = Vec::new();
    for step in &plan.steps {
        let report = run_applescript_step(&options.osascript_path, step);
        let failed = report
            .get("status")
            .and_then(Value::as_str)
            .is_some_and(|status| status != "succeeded");
        reports.push(report);
        if failed {
            return RequestExecution {
                status: "failed".to_string(),
                message: format!(
                    "AppleScript controller failed while running {}.",
                    step.operation
                ),
                artifacts: request_artifacts(request, options),
                result: json!({
                    "controller": options.controller_id,
                    "mode": options.mode.as_str(),
                    "adapter_id": request.adapter_id,
                    "action_kind": request.action_kind,
                    "osascript_path": options.osascript_path,
                    "vision_trace_path": options.vision_trace_path,
                    "pool_desktop_action": request.pool_desktop_action,
                    "desktop_payload": request.desktop_payload,
                    "steps": reports,
                }),
            };
        }
    }

    RequestExecution {
        status: options.status.clone(),
        message: format!(
            "AppleScript controller completed {} desktop step(s).",
            reports.len()
        ),
        artifacts: request_artifacts(request, options),
        result: json!({
            "controller": options.controller_id,
            "mode": options.mode.as_str(),
            "adapter_id": request.adapter_id,
            "action_kind": request.action_kind,
            "osascript_path": options.osascript_path,
            "vision_trace_path": options.vision_trace_path,
            "pool_desktop_action": request.pool_desktop_action,
            "desktop_payload": request.desktop_payload,
            "steps": reports,
        }),
    }
}

async fn vision_http_execution(
    request: &DesktopRecognitionRequest,
    options: &ControllerOptions,
    client: &reqwest::Client,
) -> RequestExecution {
    let Some(endpoint) = options
        .vision_endpoint
        .as_deref()
        .filter(|endpoint| !endpoint.trim().is_empty())
    else {
        return RequestExecution {
            status: "failed".to_string(),
            message:
                "vision-http controller requires --vision-endpoint or POOL_DESKTOP_VISION_ENDPOINT."
                    .to_string(),
            artifacts: request_artifacts(request, options),
            result: json!({
                "controller": options.controller_id,
                "mode": options.mode.as_str(),
                "adapter_id": request.adapter_id,
                "action_kind": request.action_kind,
                "pool_desktop_action": request.pool_desktop_action,
                "desktop_payload": request.desktop_payload,
                "external_visual_model": false,
                "error": "missing_vision_endpoint",
            }),
        };
    };

    let trace_path = vision_trace_output_path(request, options);
    let payload = vision_http_request_payload(request, options, endpoint, &trace_path);
    let mut request_builder = client.post(endpoint).json(&payload);
    if let Some(api_key) = options
        .vision_api_key
        .as_deref()
        .filter(|api_key| !api_key.trim().is_empty())
    {
        request_builder = request_builder.bearer_auth(api_key);
    }

    let response = match request_builder.send().await {
        Ok(response) => response,
        Err(error) => {
            return RequestExecution {
                status: "failed".to_string(),
                message: format!(
                    "vision-http controller could not call external vision service: {error}"
                ),
                artifacts: request_artifacts(request, options),
                result: json!({
                    "controller": options.controller_id,
                    "mode": options.mode.as_str(),
                    "adapter_id": request.adapter_id,
                    "action_kind": request.action_kind,
                    "vision_endpoint": endpoint,
                    "vision_trace_path": trace_path,
                    "pool_desktop_action": request.pool_desktop_action,
                    "desktop_payload": request.desktop_payload,
                    "external_visual_model": false,
                    "error": "vision_http_request_failed",
                }),
            };
        }
    };

    let response = match response.error_for_status() {
        Ok(response) => response,
        Err(error) => {
            return RequestExecution {
                status: "failed".to_string(),
                message: format!("vision-http controller received an error response: {error}"),
                artifacts: request_artifacts(request, options),
                result: json!({
                    "controller": options.controller_id,
                    "mode": options.mode.as_str(),
                    "adapter_id": request.adapter_id,
                    "action_kind": request.action_kind,
                    "vision_endpoint": endpoint,
                    "vision_trace_path": trace_path,
                    "pool_desktop_action": request.pool_desktop_action,
                    "desktop_payload": request.desktop_payload,
                    "external_visual_model": false,
                    "error": "vision_http_status_failed",
                }),
            };
        }
    };

    let response_json = match response.json::<Value>().await {
        Ok(value) => value,
        Err(error) => {
            return RequestExecution {
                status: "failed".to_string(),
                message: format!(
                    "vision-http controller could not parse vision JSON response: {error}"
                ),
                artifacts: request_artifacts(request, options),
                result: json!({
                    "controller": options.controller_id,
                    "mode": options.mode.as_str(),
                    "adapter_id": request.adapter_id,
                    "action_kind": request.action_kind,
                    "vision_endpoint": endpoint,
                    "vision_trace_path": trace_path,
                    "pool_desktop_action": request.pool_desktop_action,
                    "desktop_payload": request.desktop_payload,
                    "external_visual_model": false,
                    "error": "invalid_vision_http_response",
                }),
            };
        }
    };

    let trace = normalize_vision_http_trace(request, options, endpoint, &trace_path, response_json);
    if let Err(error) = write_json_file(&trace_path, &trace) {
        return RequestExecution {
            status: "failed".to_string(),
            message: format!("vision-http controller could not write vision trace: {error}"),
            artifacts: request_artifacts(request, options),
            result: json!({
                "controller": options.controller_id,
                "mode": options.mode.as_str(),
                "adapter_id": request.adapter_id,
                "action_kind": request.action_kind,
                "vision_endpoint": endpoint,
                "vision_trace_path": trace_path,
                "pool_desktop_action": request.pool_desktop_action,
                "desktop_payload": request.desktop_payload,
                "external_visual_model": false,
                "error": "write_vision_trace_failed",
            }),
        };
    }

    let detections = vision_detection_count(&trace);
    let mut artifacts = request_artifacts(request, options);
    push_unique_artifact(&mut artifacts, trace_path.clone());
    for artifact in vision_response_artifacts(&trace) {
        push_unique_artifact(&mut artifacts, artifact);
    }

    RequestExecution {
        status: options.status.clone(),
        message: format!(
            "vision-http controller wrote external visual model trace with {detections} detection(s)."
        ),
        artifacts,
        result: json!({
            "controller": options.controller_id,
            "mode": options.mode.as_str(),
            "adapter_id": request.adapter_id,
            "action_kind": request.action_kind,
            "vision_endpoint": endpoint,
            "vision_trace_path": trace_path,
            "production_attestation": options.production_attestation.as_deref(),
            "external_visual_model": true,
            "pool_desktop_action": request.pool_desktop_action,
            "desktop_payload": request.desktop_payload,
            "detections": detections,
            "controller_result": {
                "external_visual_model": true,
                "production_attestation": options.production_attestation.as_deref(),
                "vision_trace_path": trace_path,
                "detections": detections,
            },
        }),
    }
}

fn request_artifacts(
    request: &DesktopRecognitionRequest,
    options: &ControllerOptions,
) -> Vec<String> {
    request
        .desktop_request_path
        .iter()
        .chain(options.vision_trace_path.iter())
        .cloned()
        .collect()
}

fn push_unique_artifact(artifacts: &mut Vec<String>, artifact: String) {
    if !artifact.trim().is_empty() && !artifacts.iter().any(|existing| existing == &artifact) {
        artifacts.push(artifact);
    }
}

fn load_vision_trace(options: &ControllerOptions) -> Result<Option<Value>> {
    let Some(path) = options
        .vision_trace_path
        .as_ref()
        .filter(|path| !path.trim().is_empty())
    else {
        return Ok(None);
    };
    let body = std::fs::read_to_string(path).with_context(|| format!("read {path}"))?;
    let trace = serde_json::from_str(&body).with_context(|| format!("parse {path}"))?;
    Ok(Some(trace))
}

fn vision_http_request_payload(
    request: &DesktopRecognitionRequest,
    options: &ControllerOptions,
    endpoint: &str,
    trace_path: &str,
) -> Value {
    let desktop_payload = request.desktop_payload.as_ref();
    let pool_desktop_action = request.pool_desktop_action.as_ref();
    json!({
        "kind": "pool_desktop_vision_request",
        "version": 1,
        "controller_id": options.controller_id,
        "software_action_id": request.software_action_id,
        "task_id": request.task_id,
        "adapter_id": request.adapter_id,
        "action_kind": request.action_kind,
        "desktop_request_path": request.desktop_request_path,
        "pool_desktop_action": request.pool_desktop_action,
        "desktop_payload": request.desktop_payload,
        "target_window": target_window(desktop_payload, pool_desktop_action),
        "visual_targets": desktop_payload.map(desktop_visual_targets).unwrap_or_default(),
        "requested_trace_path": trace_path,
        "vision_endpoint": endpoint,
        "expected_response": {
            "detections": "array of OCR/UI detections with label/text/name/id plus center, point, position, coordinates, bounds, bbox, or box",
            "artifacts": "optional local screenshot/capture/artifact paths",
            "screenshot_path": "optional local screenshot path"
        }
    })
}

fn normalize_vision_http_trace(
    request: &DesktopRecognitionRequest,
    options: &ControllerOptions,
    endpoint: &str,
    trace_path: &str,
    response: Value,
) -> Value {
    let detections = vision_response_detections(&response);
    json!({
        "schema": "pool.desktop_vision_trace.v1",
        "kind": "pool_desktop_vision_trace",
        "version": 1,
        "source": "external_vision_http",
        "external_visual_model": true,
        "controller_id": options.controller_id,
        "production_attestation": options.production_attestation.as_deref(),
        "software_action_id": request.software_action_id,
        "task_id": request.task_id,
        "adapter_id": request.adapter_id,
        "action_kind": request.action_kind,
        "desktop_request_path": request.desktop_request_path,
        "pool_desktop_action": request.pool_desktop_action,
        "desktop_payload": request.desktop_payload,
        "target_window": target_window(
            request.desktop_payload.as_ref(),
            request.pool_desktop_action.as_ref()
        ),
        "visual_targets": request
            .desktop_payload
            .as_ref()
            .map(desktop_visual_targets)
            .unwrap_or_default(),
        "vision_endpoint": endpoint,
        "vision_trace_path": trace_path,
        "detections": detections,
        "raw_response": response,
    })
}

fn vision_response_detections(response: &Value) -> Vec<Value> {
    if let Some(values) = response.as_array() {
        return values.clone();
    }
    ["detections", "targets", "items", "elements", "ocr"]
        .into_iter()
        .find_map(|key| response.get(key)?.as_array().cloned())
        .unwrap_or_default()
}

fn vision_response_artifacts(trace: &Value) -> Vec<String> {
    let raw_response = trace.get("raw_response").unwrap_or(trace);
    let mut artifacts = Vec::new();
    for key in [
        "screenshot_path",
        "image_path",
        "capture_path",
        "trace_path",
    ] {
        if let Some(value) = value_string_path(Some(raw_response), &[key]) {
            push_unique_artifact(&mut artifacts, value);
        }
    }
    for key in ["artifacts", "files"] {
        if let Some(values) = value_string_array_path(Some(raw_response), &[key]) {
            for value in values {
                push_unique_artifact(&mut artifacts, value);
            }
        }
    }
    artifacts
}

fn vision_detection_count(trace: &Value) -> usize {
    vision_trace_detections(trace).map_or(0, |detections| detections.len())
}

fn vision_trace_output_path(
    request: &DesktopRecognitionRequest,
    options: &ControllerOptions,
) -> String {
    if let Some(path) = options
        .vision_trace_output_path
        .as_ref()
        .filter(|path| !path.trim().is_empty())
        .or_else(|| {
            options
                .vision_trace_path
                .as_ref()
                .filter(|path| !path.trim().is_empty())
        })
    {
        return path.to_string();
    }

    let file_name = format!(
        "{}-external-vision-trace.json",
        sanitize_file_component(&request.software_action_id)
    );
    request
        .desktop_request_path
        .as_deref()
        .and_then(|path| {
            Path::new(path)
                .parent()
                .map(|parent| parent.join(&file_name))
        })
        .unwrap_or_else(|| PathBuf::from("target/pool-desktop-vision-trace").join(file_name))
        .to_string_lossy()
        .into_owned()
}

fn write_json_file(path: &str, value: &Value) -> Result<()> {
    let path = Path::new(path);
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

fn write_desktop_vision_evidence_bundle(
    options: &ControllerOptions,
    desktop_vision: &[Value],
) -> Result<()> {
    write_json_file(
        &options.evidence_bundle_path(),
        &json!({
            "source": "run_desktop_recognition_controller",
            "project_slug": options.project_slug,
            "providers": [],
            "software_actions": [],
            "desktop_vision": desktop_vision,
        }),
    )
}

fn desktop_vision_production_evidence_item(
    request: &DesktopRecognitionRequest,
    options: &ControllerOptions,
    result: &DesktopRecognitionResult<'_>,
    callback_response: &Value,
) -> Result<Option<Value>> {
    if result.status != "succeeded" || !result_external_visual_model(&result.result) {
        return Ok(None);
    }
    let Some(production_attestation) = options
        .production_attestation
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };
    let trace_path = result_trace_path(&result.result).with_context(|| {
        format!(
            "external visual model result for {} missing vision_trace_path",
            request.software_action_id
        )
    })?;
    if !Path::new(&trace_path).exists() {
        anyhow::bail!(
            "external visual model trace for {} does not exist: {}",
            request.software_action_id,
            trace_path
        );
    }
    let mut artifacts = existing_local_artifacts(&result.artifacts);
    push_unique_artifact(&mut artifacts, trace_path.clone());

    Ok(Some(json!({
        "adapter_id": request.adapter_id,
        "external_action_id": request.software_action_id,
        "controller_id": options.controller_id,
        "production_attestation": production_attestation,
        "trace_path": trace_path,
        "visual_model": "external",
        "task_title": format!("{} desktop vision production evidence", request.adapter_id.as_deref().unwrap_or("desktop")),
        "artifacts": artifacts,
        "evidence_json": {
            "source": "run_desktop_recognition_controller",
            "mode": options.mode.as_str(),
            "external_visual_model": true,
            "production_attestation": production_attestation,
            "software_action_id": request.software_action_id,
            "task_id": request.task_id,
            "adapter_id": request.adapter_id,
        },
        "verification_json": {
            "external_visual_model": true,
            "production_attestation": production_attestation,
            "controller_result": result.result,
            "callback_response": callback_response,
        },
    })))
}

fn result_external_visual_model(result: &Value) -> bool {
    result
        .get("external_visual_model")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || result
            .pointer("/controller_result/external_visual_model")
            .and_then(Value::as_bool)
            .unwrap_or(false)
}

fn result_trace_path(result: &Value) -> Option<String> {
    result
        .get("vision_trace_path")
        .and_then(Value::as_str)
        .or_else(|| {
            result
                .pointer("/controller_result/vision_trace_path")
                .and_then(Value::as_str)
        })
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string)
}

fn existing_local_artifacts(artifacts: &[String]) -> Vec<String> {
    let mut values = Vec::new();
    for artifact in artifacts {
        if is_local_artifact_path(artifact) && Path::new(artifact).exists() {
            push_unique_artifact(&mut values, artifact.clone());
        }
    }
    values
}

fn is_local_artifact_path(path: &str) -> bool {
    let value = path.trim().to_ascii_lowercase();
    !value.is_empty()
        && !value.starts_with("http://")
        && !value.starts_with("https://")
        && !value.contains("://")
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AppleScriptStep {
    operation: String,
    description: String,
    script: String,
}

impl AppleScriptStep {
    fn summary(&self) -> Value {
        json!({
            "operation": self.operation,
            "description": self.description,
            "script": self.script,
        })
    }
}

#[derive(Debug, Default)]
struct AppleScriptPlan {
    steps: Vec<AppleScriptStep>,
    has_action: bool,
}

fn build_applescript_plan(
    request: &DesktopRecognitionRequest,
    vision_trace: Option<&Value>,
) -> AppleScriptPlan {
    let payload = request.desktop_payload.as_ref();
    let action = request.pool_desktop_action.as_ref();
    let mut plan = AppleScriptPlan::default();

    if let Some(target) = target_window(payload, action) {
        plan.steps.push(AppleScriptStep {
            operation: "activate_application".to_string(),
            description: format!("activate {target}"),
            script: format!(
                "tell application {} to activate",
                applescript_string(&target)
            ),
        });
        if is_open_project_action(request) {
            plan.has_action = true;
        }
    }

    if let Some((x, y, source)) = desktop_coordinates(payload, vision_trace) {
        plan.steps.push(AppleScriptStep {
            operation: "click".to_string(),
            description: match source {
                Some(source) => format!("click visual target {source} at {x},{y}"),
                None => format!("click at {x},{y}"),
            },
            script: format!("tell application \"System Events\" to click at {{{x}, {y}}}"),
        });
        plan.has_action = true;
    }

    if let Some(hotkey) = desktop_hotkey(payload) {
        plan.steps.push(AppleScriptStep {
            operation: "hotkey".to_string(),
            description: hotkey.description(),
            script: hotkey.script(),
        });
        plan.has_action = true;
    }

    if let Some(text) = desktop_text(payload) {
        plan.steps.push(AppleScriptStep {
            operation: "type_text".to_string(),
            description: "type provided text".to_string(),
            script: format!(
                "tell application \"System Events\" to keystroke {}",
                applescript_string(&text)
            ),
        });
        plan.has_action = true;
    }

    plan
}

fn run_applescript_step(osascript_path: &str, step: &AppleScriptStep) -> Value {
    match Command::new(osascript_path)
        .arg("-e")
        .arg(&step.script)
        .output()
    {
        Ok(output) => json!({
            "operation": step.operation,
            "description": step.description,
            "script": step.script,
            "status": if output.status.success() { "succeeded" } else { "failed" },
            "exit_code": output.status.code(),
            "stdout": String::from_utf8_lossy(&output.stdout).trim(),
            "stderr": String::from_utf8_lossy(&output.stderr).trim(),
        }),
        Err(error) => json!({
            "operation": step.operation,
            "description": step.description,
            "script": step.script,
            "status": "failed",
            "error": error.to_string(),
        }),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Hotkey {
    key: String,
    modifiers: Vec<String>,
}

impl Hotkey {
    fn description(&self) -> String {
        if self.modifiers.is_empty() {
            return format!("press {}", self.key);
        }
        format!("press {}+{}", self.modifiers.join("+"), self.key)
    }

    fn script(&self) -> String {
        let Some(key_code) = special_key_code(&self.key) else {
            return if self.modifiers.is_empty() {
                format!(
                    "tell application \"System Events\" to keystroke {}",
                    applescript_string(&self.key)
                )
            } else {
                format!(
                    "tell application \"System Events\" to keystroke {} using {}",
                    applescript_string(&self.key),
                    applescript_modifier_list(&self.modifiers)
                )
            };
        };

        if self.modifiers.is_empty() {
            format!("tell application \"System Events\" to key code {key_code}")
        } else {
            format!(
                "tell application \"System Events\" to key code {key_code} using {}",
                applescript_modifier_list(&self.modifiers)
            )
        }
    }
}

fn is_open_project_action(request: &DesktopRecognitionRequest) -> bool {
    request.action_kind.as_deref() == Some("OpenProject")
        || value_string_path(request.desktop_payload.as_ref(), &["operation"])
            .is_some_and(|operation| operation == "open_project")
        || value_string_path(request.pool_desktop_action.as_ref(), &["operation"])
            .is_some_and(|operation| operation == "open_project")
}

fn target_window(payload: Option<&Value>, action: Option<&Value>) -> Option<String> {
    value_string_path(payload, &["target_window"])
        .or_else(|| value_string_path(payload, &["arguments", "target_window"]))
        .or_else(|| value_string_path(action, &["target_window"]))
}

fn desktop_coordinates(
    payload: Option<&Value>,
    vision_trace: Option<&Value>,
) -> Option<(i64, i64, Option<String>)> {
    let payload = payload?;
    value_coordinates_path(payload, &["arguments", "coordinates"])
        .or_else(|| value_coordinates_path(payload, &["arguments", "click"]))
        .or_else(|| value_coordinates_path(payload, &["arguments", "position"]))
        .or_else(|| value_coordinates_path(payload, &["coordinates"]))
        .or_else(|| value_coordinates_path(payload, &["click"]))
        .or_else(|| {
            let args = payload.get("arguments")?;
            let x = value_i64_path(Some(args), &["x"])?;
            let y = value_i64_path(Some(args), &["y"])?;
            Some((x, y))
        })
        .or_else(|| {
            let x = value_i64_path(Some(payload), &["x"])?;
            let y = value_i64_path(Some(payload), &["y"])?;
            Some((x, y))
        })
        .map(|(x, y)| (x, y, None))
        .or_else(|| {
            let visual_targets = desktop_visual_targets(payload);
            let target = visual_targets.first()?;
            let hit = visual_trace_coordinates(vision_trace?, target)?;
            Some((hit.x, hit.y, Some(hit.description())))
        })
}

fn desktop_hotkey(payload: Option<&Value>) -> Option<Hotkey> {
    let payload = payload?;
    value_string_path(Some(payload), &["arguments", "hotkey"])
        .or_else(|| value_string_path(Some(payload), &["arguments", "shortcut"]))
        .or_else(|| value_string_path(Some(payload), &["hotkey"]))
        .or_else(|| value_string_path(Some(payload), &["shortcut"]))
        .and_then(|value| parse_hotkey(&value))
        .or_else(|| {
            value_string_array_path(Some(payload), &["arguments", "keys"])
                .or_else(|| value_string_array_path(Some(payload), &["keys"]))
                .and_then(parse_hotkey_parts)
        })
}

fn desktop_text(payload: Option<&Value>) -> Option<String> {
    value_string_path(payload, &["arguments", "text"])
        .or_else(|| value_string_path(payload, &["arguments", "type_text"]))
        .or_else(|| value_string_path(payload, &["arguments", "input_text"]))
        .or_else(|| value_string_path(payload, &["text"]))
        .or_else(|| value_string_path(payload, &["type_text"]))
        .or_else(|| value_string_path(payload, &["input_text"]))
}

fn desktop_visual_targets(payload: &Value) -> Vec<String> {
    value_string_array_path(Some(payload), &["arguments", "visual_targets"])
        .or_else(|| value_string_array_path(Some(payload), &["visual_targets"]))
        .unwrap_or_else(|| {
            value_string_path(Some(payload), &["arguments", "click_target"])
                .or_else(|| value_string_path(Some(payload), &["click_target"]))
                .into_iter()
                .collect()
        })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct VisionTraceHit {
    label: String,
    x: i64,
    y: i64,
}

impl VisionTraceHit {
    fn description(&self) -> String {
        format!("{} ({},{})", self.label, self.x, self.y)
    }
}

fn visual_trace_coordinates(trace: &Value, target: &str) -> Option<VisionTraceHit> {
    let detections = vision_trace_detections(trace)?;
    detections
        .iter()
        .filter_map(|detection| vision_detection_hit(detection, target))
        .max_by_key(|hit| (vision_label_score(&hit.label, target), hit.label.len()))
}

fn vision_trace_detections(trace: &Value) -> Option<Vec<&Value>> {
    if let Some(values) = trace.as_array() {
        return Some(values.iter().collect());
    }
    ["detections", "targets", "items", "elements", "ocr"]
        .into_iter()
        .find_map(|key| {
            trace
                .get(key)?
                .as_array()
                .map(|values| values.iter().collect())
        })
}

fn vision_detection_hit(detection: &Value, target: &str) -> Option<VisionTraceHit> {
    let label = value_string_path(Some(detection), &["label"])
        .or_else(|| value_string_path(Some(detection), &["text"]))
        .or_else(|| value_string_path(Some(detection), &["name"]))
        .or_else(|| value_string_path(Some(detection), &["id"]))?;
    if vision_label_score(&label, target) == 0 {
        return None;
    }
    let (x, y) = value_coordinates_path(detection, &["center"])
        .or_else(|| value_coordinates_path(detection, &["point"]))
        .or_else(|| value_coordinates_path(detection, &["position"]))
        .or_else(|| value_coordinates_path(detection, &["coordinates"]))
        .or_else(|| value_bounds_center_path(detection, &["bounds"]))
        .or_else(|| value_bounds_center_path(detection, &["bbox"]))
        .or_else(|| value_bounds_center_path(detection, &["box"]))?;
    Some(VisionTraceHit { label, x, y })
}

fn vision_label_score(label: &str, target: &str) -> usize {
    let label = normalize_vision_label(label);
    let target = normalize_vision_label(target);
    if label.is_empty() || target.is_empty() {
        0
    } else if label == target {
        3
    } else if label.contains(&target) || target.contains(&label) {
        2
    } else {
        0
    }
}

fn normalize_vision_label(value: &str) -> String {
    value
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>()
        .to_ascii_lowercase()
}

fn parse_hotkey(value: &str) -> Option<Hotkey> {
    parse_hotkey_parts(
        value
            .split('+')
            .map(|part| part.trim().to_string())
            .collect::<Vec<_>>(),
    )
}

fn parse_hotkey_parts(parts: Vec<String>) -> Option<Hotkey> {
    let mut key = None;
    let mut modifiers = Vec::new();

    for part in parts.into_iter().filter(|part| !part.trim().is_empty()) {
        if is_modifier(&part) {
            modifiers.push(normalize_modifier(&part).to_string());
        } else {
            key = Some(part);
        }
    }

    key.map(|key| Hotkey { key, modifiers })
}

fn is_modifier(value: &str) -> bool {
    matches!(
        value.to_ascii_lowercase().as_str(),
        "cmd" | "command" | "meta" | "shift" | "option" | "alt" | "control" | "ctrl"
    )
}

fn normalize_modifier(value: &str) -> &'static str {
    match value.to_ascii_lowercase().as_str() {
        "cmd" | "command" | "meta" => "command",
        "option" | "alt" => "option",
        "control" | "ctrl" => "control",
        _ => "shift",
    }
}

fn applescript_modifier_list(modifiers: &[String]) -> String {
    if modifiers.is_empty() {
        return "{}".to_string();
    }
    let values = modifiers
        .iter()
        .map(|modifier| format!("{} down", normalize_modifier(modifier)))
        .collect::<Vec<_>>()
        .join(", ");
    format!("{{{values}}}")
}

fn special_key_code(value: &str) -> Option<i64> {
    match value.to_ascii_lowercase().as_str() {
        "return" | "enter" => Some(36),
        "tab" => Some(48),
        "escape" | "esc" => Some(53),
        "delete" | "backspace" => Some(51),
        "forward_delete" | "forward-delete" => Some(117),
        "left" | "left_arrow" | "left-arrow" => Some(123),
        "right" | "right_arrow" | "right-arrow" => Some(124),
        "down" | "down_arrow" | "down-arrow" => Some(125),
        "up" | "up_arrow" | "up-arrow" => Some(126),
        _ => None,
    }
}

fn value_string_path(value: Option<&Value>, path: &[&str]) -> Option<String> {
    let value = value_path(value?, path)?;
    match value {
        Value::String(value) if !value.trim().is_empty() => Some(value.to_string()),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

fn value_i64_path(value: Option<&Value>, path: &[&str]) -> Option<i64> {
    value_path(value?, path)?.as_i64()
}

fn value_string_array_path(value: Option<&Value>, path: &[&str]) -> Option<Vec<String>> {
    value_path(value?, path)?
        .as_array()
        .map(|values| {
            values
                .iter()
                .filter_map(|value| match value {
                    Value::String(value) if !value.trim().is_empty() => Some(value.to_string()),
                    Value::Number(value) => Some(value.to_string()),
                    _ => None,
                })
                .collect::<Vec<_>>()
        })
        .filter(|values| !values.is_empty())
}

fn value_coordinates_path(value: &Value, path: &[&str]) -> Option<(i64, i64)> {
    let value = value_path(value, path)?;
    if let Some(values) = value.as_array() {
        let x = values.first()?.as_i64()?;
        let y = values.get(1)?.as_i64()?;
        return Some((x, y));
    }
    let x = value_i64_path(Some(value), &["x"])?;
    let y = value_i64_path(Some(value), &["y"])?;
    Some((x, y))
}

fn value_bounds_center_path(value: &Value, path: &[&str]) -> Option<(i64, i64)> {
    let value = value_path(value, path)?;
    if let Some(values) = value.as_array() {
        let x = values.first()?.as_i64()?;
        let y = values.get(1)?.as_i64()?;
        let width_or_x2 = values.get(2)?.as_i64()?;
        let height_or_y2 = values.get(3)?.as_i64()?;
        if width_or_x2 > x && height_or_y2 > y {
            return Some(((x + width_or_x2) / 2, (y + height_or_y2) / 2));
        }
        return Some((x + width_or_x2 / 2, y + height_or_y2 / 2));
    }

    let x =
        value_i64_path(Some(value), &["x"]).or_else(|| value_i64_path(Some(value), &["left"]))?;
    let y =
        value_i64_path(Some(value), &["y"]).or_else(|| value_i64_path(Some(value), &["top"]))?;
    if let (Some(width), Some(height)) = (
        value_i64_path(Some(value), &["width"]),
        value_i64_path(Some(value), &["height"]),
    ) {
        return Some((x + width / 2, y + height / 2));
    }
    let x2 =
        value_i64_path(Some(value), &["x2"]).or_else(|| value_i64_path(Some(value), &["right"]))?;
    let y2 = value_i64_path(Some(value), &["y2"])
        .or_else(|| value_i64_path(Some(value), &["bottom"]))?;
    Some(((x + x2) / 2, (y + y2) / 2))
}

fn value_path<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    Some(current)
}

fn applescript_string(value: &str) -> String {
    format!(
        "\"{}\"",
        value
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
    )
}

fn normalize_runtime_base_url(value: &str) -> String {
    let value = value.trim().trim_end_matches('/');
    if value.starts_with("http://") || value.starts_with("https://") {
        value.to_string()
    } else {
        format!("http://{value}")
    }
}

fn url_query_value(value: &str) -> String {
    value.replace(' ', "%20")
}

fn sanitize_file_component(value: &str) -> String {
    let slug = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");

    if slug.is_empty() {
        "desktop-action".to_string()
    } else {
        slug
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn controller_options() -> ControllerOptions {
        ControllerOptions {
            runtime_base_url: "http://127.0.0.1:4788".to_string(),
            project_query: "?project=demo".to_string(),
            status: "succeeded".to_string(),
            controller_id: "external-vision-controller".to_string(),
            limit: usize::MAX,
            mode: ControllerMode::VisionHttp,
            osascript_path: "osascript".to_string(),
            vision_trace_path: None,
            vision_trace_output_path: None,
            vision_endpoint: Some("http://127.0.0.1:8795/vision".to_string()),
            vision_api_key: None,
            production_attestation: Some("external-vision-controller-prod-run-1".to_string()),
            evidence_bundle_path: None,
            project_slug: "demo".to_string(),
        }
    }

    fn request_with_payload(action_kind: &str, payload: Value) -> DesktopRecognitionRequest {
        DesktopRecognitionRequest {
            software_action_id: "action-1".to_string(),
            task_id: Some("task-1".to_string()),
            adapter_id: Some("touchdesigner".to_string()),
            action_kind: Some(action_kind.to_string()),
            desktop_request_path: Some("worlds/demo/output/control/request.json".to_string()),
            pool_desktop_action: Some(json!({
                "operation": "send_hotkey",
                "target_window": "TouchDesigner",
            })),
            desktop_payload: Some(payload),
        }
    }

    #[test]
    fn parses_vision_http_options() {
        let options = ControllerOptions::from_args([
            "http://localhost:4788".to_string(),
            "--project=show one".to_string(),
            "--mode=vision-http".to_string(),
            "--vision-endpoint=http://localhost:8795/vision".to_string(),
            "--vision-trace-output=worlds/demo/output/control/trace.json".to_string(),
            "--evidence-bundle=worlds/demo/output/control/vision-bundle.json".to_string(),
        ]);

        assert_eq!(options.runtime_base_url, "http://localhost:4788");
        assert_eq!(options.project_query, "?project=show%20one");
        assert_eq!(options.project_slug, "show one");
        assert_eq!(options.mode, ControllerMode::VisionHttp);
        assert_eq!(
            options.vision_endpoint.as_deref(),
            Some("http://localhost:8795/vision")
        );
        assert_eq!(
            options.vision_trace_output_path.as_deref(),
            Some("worlds/demo/output/control/trace.json")
        );
        assert_eq!(
            options.evidence_bundle_path.as_deref(),
            Some("worlds/demo/output/control/vision-bundle.json")
        );
    }

    #[test]
    fn builds_vision_http_payload_from_desktop_request() {
        let request = request_with_payload(
            "RunViewport",
            json!({
                "target_window": "TouchDesigner",
                "visual_targets": ["Perform", "Cue 1"]
            }),
        );
        let options = controller_options();

        let payload = vision_http_request_payload(
            &request,
            &options,
            "http://localhost:8795/vision",
            "worlds/demo/output/control/external-trace.json",
        );

        assert_eq!(payload["kind"], "pool_desktop_vision_request");
        assert_eq!(payload["controller_id"], "external-vision-controller");
        assert_eq!(payload["target_window"], "TouchDesigner");
        assert_eq!(payload["visual_targets"], json!(["Perform", "Cue 1"]));
        assert_eq!(
            payload["requested_trace_path"],
            "worlds/demo/output/control/external-trace.json"
        );
    }

    #[test]
    fn normalizes_vision_http_trace_as_external_model_evidence() {
        let request = request_with_payload(
            "RunViewport",
            json!({
                "target_window": "TouchDesigner",
                "visual_targets": ["Cue 1"]
            }),
        );
        let options = controller_options();
        let response = json!({
            "detections": [
                {
                    "text": "Cue 1",
                    "bbox": {"x": 200, "y": 100, "width": 80, "height": 40}
                }
            ],
            "screenshot_path": "worlds/demo/output/control/capture.png",
            "artifacts": ["worlds/demo/output/control/capture.json"]
        });

        let trace = normalize_vision_http_trace(
            &request,
            &options,
            "http://localhost:8795/vision",
            "worlds/demo/output/control/external-trace.json",
            response,
        );

        assert_eq!(trace["schema"], "pool.desktop_vision_trace.v1");
        assert_eq!(trace["external_visual_model"], true);
        assert_eq!(trace["source"], "external_vision_http");
        assert_eq!(vision_detection_count(&trace), 1);
        assert_eq!(
            visual_trace_coordinates(&trace, "Cue 1")
                .unwrap()
                .description(),
            "Cue 1 (240,120)"
        );
        assert_eq!(
            vision_response_artifacts(&trace),
            vec![
                "worlds/demo/output/control/capture.png".to_string(),
                "worlds/demo/output/control/capture.json".to_string()
            ]
        );
    }

    #[tokio::test]
    async fn vision_http_execution_posts_request_and_writes_trace() {
        use std::io::{Read, Write};
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}/vision", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buffer = [0_u8; 4096];
            let bytes = stream.read(&mut buffer).unwrap();
            let request = String::from_utf8_lossy(&buffer[..bytes]);
            assert!(request.starts_with("POST /vision HTTP/1.1"));
            assert!(request.contains("pool_desktop_vision_request"));

            let body = r#"{"detections":[{"label":"Cue 1","center":{"x":120,"y":90}}],"screenshot_path":"worlds/demo/output/control/capture.png"}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            stream.write_all(response.as_bytes()).unwrap();
        });

        let mut options = controller_options();
        let trace_path = std::env::temp_dir()
            .join(format!(
                "pool-vision-http-trace-{}.json",
                std::process::id()
            ))
            .to_string_lossy()
            .into_owned();
        options.vision_endpoint = Some(endpoint);
        options.vision_trace_output_path = Some(trace_path.clone());
        let request = request_with_payload(
            "RunViewport",
            json!({
                "target_window": "TouchDesigner",
                "visual_targets": ["Cue 1"]
            }),
        );

        let execution = vision_http_execution(&request, &options, &reqwest::Client::new()).await;

        assert_eq!(execution.status, "succeeded");
        assert_eq!(execution.result["external_visual_model"], true);
        assert_eq!(
            execution.result["controller_result"]["external_visual_model"],
            true
        );
        assert!(execution.artifacts.contains(&trace_path));
        assert!(execution
            .artifacts
            .contains(&"worlds/demo/output/control/capture.png".to_string()));
        let trace: Value =
            serde_json::from_str(&std::fs::read_to_string(&trace_path).unwrap()).unwrap();
        assert_eq!(trace["external_visual_model"], true);
        assert_eq!(vision_detection_count(&trace), 1);

        server.join().unwrap();
        let _ = std::fs::remove_file(trace_path);
    }

    #[test]
    fn builds_desktop_vision_production_evidence_item() {
        let mut options = controller_options();
        options.evidence_bundle_path =
            Some("target/desktop-vision-evidence/desktop-vision-bundle.json".to_string());
        let request = request_with_payload(
            "RunViewport",
            json!({
                "target_window": "TouchDesigner",
                "visual_targets": ["Cue 1"]
            }),
        );
        let trace_path = std::env::temp_dir()
            .join(format!(
                "pool-desktop-vision-evidence-{}.json",
                std::process::id()
            ))
            .to_string_lossy()
            .into_owned();
        std::fs::write(
            &trace_path,
            r#"{"external_visual_model":true,"detections":[]}"#,
        )
        .unwrap();
        let result = DesktopRecognitionResult {
            software_action_id: &request.software_action_id,
            task_id: request.task_id.as_deref(),
            status: "succeeded",
            message: "external vision completed".to_string(),
            artifacts: vec![
                trace_path.clone(),
                "https://example.com/remote-capture.png".to_string(),
            ],
            result: json!({
                "external_visual_model": true,
                "vision_trace_path": trace_path,
                "controller_result": {
                    "external_visual_model": true
                }
            }),
        };
        let item = desktop_vision_production_evidence_item(&request, &options, &result, &json!({}))
            .unwrap()
            .unwrap();

        assert_eq!(item["external_action_id"], "action-1");
        assert_eq!(item["controller_id"], "external-vision-controller");
        assert_eq!(
            item["production_attestation"],
            "external-vision-controller-prod-run-1"
        );
        assert_eq!(item["visual_model"], "external");
        assert_eq!(item["evidence_json"]["external_visual_model"], true);
        assert_eq!(
            item["evidence_json"]["production_attestation"],
            "external-vision-controller-prod-run-1"
        );
        assert_eq!(item["verification_json"]["external_visual_model"], true);
        assert_eq!(item["artifacts"].as_array().unwrap().len(), 1);

        let _ = std::fs::remove_file(
            item["trace_path"]
                .as_str()
                .expect("test evidence trace path"),
        );
    }

    #[test]
    fn defaults_vision_trace_output_next_to_desktop_request() {
        let request = request_with_payload(
            "DesktopClick",
            json!({
                "target_window": "MadMapper",
                "click_target": "Cue 1"
            }),
        );
        let options = controller_options();

        assert_eq!(
            vision_trace_output_path(&request, &options),
            "worlds/demo/output/control/action-1-external-vision-trace.json"
        );
    }

    #[test]
    fn builds_applescript_steps_from_hotkey_payload() {
        let request = request_with_payload(
            "DesktopHotkey",
            json!({
                "target_window": "TouchDesigner",
                "arguments": {
                    "hotkey": "cmd+shift+p"
                }
            }),
        );

        let plan = build_applescript_plan(&request, None);

        assert!(plan.has_action);
        assert_eq!(plan.steps.len(), 2);
        assert_eq!(plan.steps[0].operation, "activate_application");
        assert_eq!(
            plan.steps[1].script,
            "tell application \"System Events\" to keystroke \"p\" using {command down, shift down}"
        );
    }

    #[test]
    fn builds_applescript_steps_from_click_payload() {
        let request = request_with_payload(
            "DesktopClick",
            json!({
                "target_window": "MadMapper",
                "arguments": {
                    "coordinates": {
                        "x": 320,
                        "y": 240
                    }
                }
            }),
        );

        let plan = build_applescript_plan(&request, None);

        assert!(plan.has_action);
        assert_eq!(plan.steps.len(), 2);
        assert_eq!(
            plan.steps[1].script,
            "tell application \"System Events\" to click at {320, 240}"
        );
    }

    #[test]
    fn refuses_visual_only_run_viewport_payload() {
        let request = request_with_payload(
            "RunViewport",
            json!({
                "target_window": "TouchDesigner",
                "visual_targets": ["Perform", "Cue 1"]
            }),
        );

        let plan = build_applescript_plan(&request, None);

        assert!(!plan.has_action);
        assert_eq!(plan.steps.len(), 1);
    }

    #[test]
    fn resolves_visual_target_from_trace_bounds() {
        let request = request_with_payload(
            "RunViewport",
            json!({
                "target_window": "TouchDesigner",
                "visual_targets": ["Cue 1"]
            }),
        );
        let trace = json!({
            "detections": [
                {
                    "label": "Perform",
                    "bounds": {"x": 10, "y": 20, "width": 80, "height": 40}
                },
                {
                    "text": "Cue 1",
                    "bounds": {"x": 300, "y": 200, "width": 100, "height": 60}
                }
            ]
        });

        let plan = build_applescript_plan(&request, Some(&trace));

        assert!(plan.has_action);
        assert_eq!(plan.steps.len(), 2);
        assert_eq!(plan.steps[1].operation, "click");
        assert_eq!(
            plan.steps[1].description,
            "click visual target Cue 1 (350,230) at 350,230"
        );
        assert_eq!(
            plan.steps[1].script,
            "tell application \"System Events\" to click at {350, 230}"
        );
    }

    #[test]
    fn prefers_explicit_coordinates_over_vision_trace() {
        let request = request_with_payload(
            "DesktopClick",
            json!({
                "target_window": "MadMapper",
                "visual_targets": ["Cue 1"],
                "arguments": {
                    "coordinates": [20, 30]
                }
            }),
        );
        let trace = json!({
            "detections": [
                {
                    "label": "Cue 1",
                    "center": {"x": 400, "y": 500}
                }
            ]
        });

        let plan = build_applescript_plan(&request, Some(&trace));

        assert!(plan.has_action);
        assert_eq!(plan.steps[1].description, "click at 20,30");
        assert_eq!(
            plan.steps[1].script,
            "tell application \"System Events\" to click at {20, 30}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn runs_configured_osascript_command() {
        use std::os::unix::fs::PermissionsExt;

        let dir = std::env::temp_dir().join(format!("pool-fake-osascript-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let fake_osascript = dir.join("osascript");
        std::fs::write(&fake_osascript, "#!/bin/sh\nprintf '%s' \"$2\"\n").unwrap();
        let mut permissions = std::fs::metadata(&fake_osascript).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&fake_osascript, permissions).unwrap();

        let step = AppleScriptStep {
            operation: "hotkey".to_string(),
            description: "press command+s".to_string(),
            script: "tell application \"System Events\" to keystroke \"s\"".to_string(),
        };

        let report = run_applescript_step(fake_osascript.to_str().unwrap(), &step);

        assert_eq!(report["status"], "succeeded");
        assert_eq!(report["stdout"], step.script);

        let _ = std::fs::remove_file(fake_osascript);
        let _ = std::fs::remove_dir(dir);
    }

    #[test]
    fn escapes_applescript_string_literals() {
        assert_eq!(
            applescript_string("A \"quoted\" path"),
            "\"A \\\"quoted\\\" path\""
        );
    }
}
