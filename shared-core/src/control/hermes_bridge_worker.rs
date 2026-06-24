use anyhow::{bail, Context, Result};
use reqwest::Client;
use serde_json::{json, Value};
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::thread;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct HermesMcpBridgeWorkerOptions {
    pub base_url: String,
    pub output_root: PathBuf,
    pub default_upstream_endpoint: Option<String>,
    pub api_key: Option<String>,
    pub action_path: String,
}

impl HermesMcpBridgeWorkerOptions {
    pub fn new(base_url: impl Into<String>, output_root: impl Into<PathBuf>) -> Self {
        Self {
            base_url: base_url.into(),
            output_root: output_root.into(),
            default_upstream_endpoint: None,
            api_key: None,
            action_path: "/mcp".to_string(),
        }
    }

    pub fn with_default_upstream_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.default_upstream_endpoint = Some(endpoint.into());
        self
    }

    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    pub fn with_action_path(mut self, action_path: impl Into<String>) -> Self {
        self.action_path = action_path.into();
        self
    }
}

pub struct HermesMcpBridgeWorker {
    options: HermesMcpBridgeWorkerOptions,
    client: Client,
}

impl HermesMcpBridgeWorker {
    pub fn new(options: HermesMcpBridgeWorkerOptions) -> Self {
        Self {
            options,
            client: Client::new(),
        }
    }

    pub fn serve_listener(&mut self, listener: TcpListener, max_requests: usize) -> Result<usize> {
        let mut handled = 0_usize;
        for stream in listener.incoming() {
            let stream = stream.context("accept Hermes MCP bridge worker connection")?;
            if let Err(error) = self.handle_tcp_connection(stream) {
                eprintln!("Hermes MCP bridge worker connection error: {error}");
            }
            handled += 1;
            if max_requests > 0 && handled >= max_requests {
                break;
            }
        }
        Ok(handled)
    }

    pub fn self_check(&mut self) -> Result<Vec<(String, u16, usize)>> {
        let request = json!({
            "adapter_id": "hermes",
            "action_kind": "CreateScene",
            "priority": "ApiMcp",
            "payload": {
                "project_slug": "demo",
                "instruction": "coordinate Unreal scene assembly",
                "target_adapter": "unreal",
                "target_action_kind": "CreateScene"
            },
            "requires_confirmation": false,
            "pool_hermes_action": {
                "profile_id": "hermes-coordinate-software",
                "operation": "coordinate_software_action",
                "mcp_tool": "hermes.coordinate",
                "stage": "agent_orchestration",
                "expected_artifacts": ["session", "transcript", "task_plan"],
                "output_contract": "hermes-mcp-action-result"
            },
            "mcp_payload": {
                "tool": "hermes.coordinate",
                "operation": "coordinate_software_action",
                "arguments": {
                    "project_slug": "demo",
                    "instruction": "coordinate Unreal scene assembly",
                    "target_adapter": "unreal",
                    "target_action_kind": "CreateScene"
                }
            }
        });
        let mut results = Vec::new();
        for (method, path, body) in [
            ("GET", "/health", String::new()),
            ("POST", "/mcp", request.to_string()),
        ] {
            let response = self.handle(method, path, &body)?;
            results.push((
                format!("{method} {path}"),
                response.status_code,
                response.body.len(),
            ));
        }
        Ok(results)
    }

    pub fn handle_tcp_connection(&mut self, mut stream: TcpStream) -> Result<()> {
        let request = read_http_request(&mut stream)?;
        let response = self
            .handle(&request.method, &request.path, &request.body)
            .unwrap_or_else(HermesMcpBridgeWorkerResponse::from_error);
        stream
            .write_all(&response.to_http_bytes())
            .context("write Hermes MCP bridge worker HTTP response")
    }

    pub fn handle(
        &mut self,
        method: &str,
        raw_path: &str,
        body: &str,
    ) -> Result<HermesMcpBridgeWorkerResponse> {
        let path = raw_path.split('?').next().unwrap_or(raw_path);
        if method == "OPTIONS" {
            return Ok(HermesMcpBridgeWorkerResponse::empty(204));
        }
        if method == "GET" && matches!(path, "/" | "/health" | "/v1/health") {
            return HermesMcpBridgeWorkerResponse::json(
                200,
                json!({
                    "ok": true,
                    "status": "ready",
                    "service": "pool-hermes-mcp-bridge",
                    "mode": if self.options.default_upstream_endpoint.is_some() { "forwarder" } else { "dry_run" },
                    "base_url": self.options.base_url,
                    "output_root": self.options.output_root,
                    "has_default_upstream": self.options.default_upstream_endpoint.is_some(),
                    "local_files_authoritative": true,
                }),
            );
        }
        if method == "POST" && matches!(path, "/mcp" | "/v1/hermes/actions") {
            return self.submit(body);
        }

        HermesMcpBridgeWorkerResponse::json(
            404,
            json!({
                "error": "not_found",
                "method": method,
                "path": path,
            }),
        )
    }

    fn submit(&mut self, body: &str) -> Result<HermesMcpBridgeWorkerResponse> {
        let request = match parse_json_body(body) {
            Ok(request) => request,
            Err(error) => {
                return HermesMcpBridgeWorkerResponse::json(
                    400,
                    json!({
                        "error": "invalid_hermes_mcp_bridge_request",
                        "message": error.to_string(),
                    }),
                );
            }
        };
        if let Err(error) = validate_hermes_bridge_request(&request) {
            return HermesMcpBridgeWorkerResponse::json(
                400,
                json!({
                    "error": "invalid_hermes_mcp_bridge_request",
                    "message": error.to_string(),
                }),
            );
        }
        let action_id = bridge_action_id(&request);
        let output_dir = self.output_dir()?;
        let request_path = output_dir.join(format!("{action_id}-request.json"));
        let response_path = output_dir.join(format!("{action_id}-response.json"));
        write_json_file(&request_path, &request)?;

        let upstream_response = if let Some(endpoint) = &self.options.default_upstream_endpoint {
            self.post_json(
                &action_url_from_endpoint(endpoint, &self.options.action_path),
                &request,
            )?
        } else {
            dry_run_hermes_response(&request, &action_id)
        };

        let response = normalize_hermes_response(
            &request,
            upstream_response,
            &action_id,
            &request_path,
            &response_path,
            self.options.default_upstream_endpoint.is_some(),
        );
        write_json_file(&response_path, &response)?;
        HermesMcpBridgeWorkerResponse::json(200, response)
    }

    fn output_dir(&self) -> Result<PathBuf> {
        let dir = self.options.output_root.join("control/hermes-mcp-bridge");
        fs::create_dir_all(&dir)
            .with_context(|| format!("create Hermes MCP bridge output dir {}", dir.display()))?;
        Ok(dir)
    }

    fn post_json(&self, url: &str, body: &Value) -> Result<Value> {
        let client = self.client.clone();
        let api_key = self.options.api_key.clone();
        let url = url.to_string();
        let body = body.clone();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .context("create Hermes MCP bridge worker tokio runtime")?;
        runtime.block_on(async move {
            let mut builder = client.post(&url).json(&body);
            if let Some(api_key) = api_key {
                builder = builder.bearer_auth(api_key);
            }
            builder
                .send()
                .await
                .with_context(|| format!("submit Hermes MCP bridge upstream action {url}"))?
                .error_for_status()
                .with_context(|| format!("upstream Hermes MCP bridge returned error for {url}"))?
                .json()
                .await
                .with_context(|| format!("decode upstream Hermes MCP bridge response {url}"))
        })
    }
}

pub fn spawn_hermes_mcp_bridge_worker(
    output_root: impl Into<PathBuf>,
    max_requests: usize,
) -> Result<String> {
    let listener = TcpListener::bind("127.0.0.1:0").context("bind Hermes MCP bridge worker")?;
    let addr = listener
        .local_addr()
        .context("read Hermes MCP bridge worker addr")?;
    let base_url = format!("http://{addr}");
    let options = HermesMcpBridgeWorkerOptions::new(base_url.clone(), output_root);
    thread::spawn(move || {
        let mut worker = HermesMcpBridgeWorker::new(options);
        if let Err(error) = worker.serve_listener(listener, max_requests) {
            eprintln!("Hermes MCP bridge worker server error: {error}");
        }
    });
    Ok(base_url)
}

#[derive(Debug, Clone)]
pub struct HermesMcpBridgeWorkerResponse {
    pub status_code: u16,
    pub content_type: String,
    pub body: Vec<u8>,
}

impl HermesMcpBridgeWorkerResponse {
    pub fn empty(status_code: u16) -> Self {
        Self {
            status_code,
            content_type: "text/plain; charset=utf-8".to_string(),
            body: Vec::new(),
        }
    }

    pub fn json(status_code: u16, value: Value) -> Result<Self> {
        Ok(Self {
            status_code,
            content_type: "application/json; charset=utf-8".to_string(),
            body: serde_json::to_vec_pretty(&value)?,
        })
    }

    fn from_error(error: anyhow::Error) -> Self {
        Self::json(
            500,
            json!({
                "error": "hermes_mcp_bridge_worker_error",
                "message": error.to_string(),
            }),
        )
        .unwrap_or_else(|_| Self {
            status_code: 500,
            content_type: "application/json; charset=utf-8".to_string(),
            body: br#"{"error":"hermes_mcp_bridge_worker_error"}"#.to_vec(),
        })
    }

    pub fn to_http_bytes(&self) -> Vec<u8> {
        let headers = format!(
            "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type, Authorization\r\nConnection: close\r\n\r\n",
            self.status_code,
            status_text(self.status_code),
            self.content_type,
            self.body.len(),
        );
        let mut response = headers.into_bytes();
        response.extend_from_slice(&self.body);
        response
    }
}

#[derive(Debug, Clone)]
struct BridgeWorkerHttpRequest {
    method: String,
    path: String,
    body: String,
}

fn validate_hermes_bridge_request(request: &Value) -> Result<()> {
    for field in [
        "adapter_id",
        "action_kind",
        "priority",
        "payload",
        "pool_hermes_action",
        "mcp_payload",
    ] {
        if request.get(field).is_none() {
            bail!("missing required Hermes MCP bridge field: {field}");
        }
    }
    if request.get("adapter_id").and_then(Value::as_str) != Some("hermes") {
        bail!("Hermes MCP bridge only accepts adapter_id=hermes");
    }
    let pool_tool = request
        .pointer("/pool_hermes_action/mcp_tool")
        .and_then(Value::as_str)
        .context("pool_hermes_action.mcp_tool is required")?;
    let mcp_tool = request
        .pointer("/mcp_payload/tool")
        .and_then(Value::as_str)
        .context("mcp_payload.tool is required")?;
    if !mcp_tool.starts_with("hermes.") {
        bail!("mcp_payload.tool must start with hermes.");
    }
    if pool_tool != mcp_tool {
        bail!("pool_hermes_action.mcp_tool must match mcp_payload.tool");
    }
    Ok(())
}

fn bridge_action_id(request: &Value) -> String {
    let operation = request
        .pointer("/pool_hermes_action/operation")
        .and_then(Value::as_str)
        .unwrap_or("action");
    format!("hermes-{}-{}", slug(operation), Uuid::new_v4().simple())
}

fn dry_run_hermes_response(request: &Value, action_id: &str) -> Value {
    let tool = request
        .pointer("/mcp_payload/tool")
        .and_then(Value::as_str)
        .unwrap_or("hermes.execute");
    let operation = request
        .pointer("/pool_hermes_action/operation")
        .and_then(Value::as_str)
        .unwrap_or("generic_action");
    json!({
        "ok": true,
        "success": true,
        "status": "completed",
        "message": format!("hermes-bridge-dry-run {tool}"),
        "session_id": action_id,
        "artifacts": [
            format!("hermes://session/{action_id}"),
            format!("hermes://bridge/{operation}/{action_id}")
        ],
    })
}

fn normalize_hermes_response(
    request: &Value,
    upstream_response: Value,
    action_id: &str,
    request_path: &Path,
    response_path: &Path,
    forwarded: bool,
) -> Value {
    let ok = response_ok(&upstream_response);
    let mut artifacts = collect_hermes_bridge_artifacts(&upstream_response);
    artifacts.push(request_path.to_string_lossy().into_owned());
    artifacts.push(response_path.to_string_lossy().into_owned());
    json!({
        "ok": ok,
        "success": ok,
        "status": if ok { "completed" } else { "failed" },
        "message": upstream_response
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or(if ok { "hermes-bridge-action-ok" } else { "hermes-bridge-action-failed" }),
        "artifacts": artifacts,
        "session_id": upstream_response
            .get("session_id")
            .and_then(Value::as_str)
            .unwrap_or(action_id),
        "pool_hermes_bridge": {
            "action_id": action_id,
            "mode": if forwarded { "forwarder" } else { "dry_run" },
            "tool": request.pointer("/mcp_payload/tool").and_then(Value::as_str),
            "operation": request.pointer("/pool_hermes_action/operation").and_then(Value::as_str),
            "profile_id": request.pointer("/pool_hermes_action/profile_id").and_then(Value::as_str),
            "request_path": request_path,
            "response_path": response_path,
            "local_files_authoritative": true,
        },
        "upstream_response": upstream_response,
    })
}

fn response_ok(response: &Value) -> bool {
    if let Some(ok) = response.get("ok").and_then(Value::as_bool) {
        return ok;
    }
    if let Some(success) = response.get("success").and_then(Value::as_bool) {
        return success;
    }
    let status = response
        .get("status")
        .or_else(|| response.get("state"))
        .and_then(Value::as_str)
        .unwrap_or("completed")
        .to_ascii_lowercase();
    matches!(
        status.as_str(),
        "completed" | "complete" | "succeeded" | "success" | "ok"
    )
}

fn collect_hermes_bridge_artifacts(response: &Value) -> Vec<String> {
    let mut artifacts = Vec::new();
    for pointer in ["/artifacts", "/data/artifacts", "/outputs", "/data/outputs"] {
        let Some(items) = response.pointer(pointer).and_then(Value::as_array) else {
            continue;
        };
        for item in items {
            if let Some(value) = item.as_str() {
                artifacts.push(value.to_string());
            } else if let Some(value) = item.get("path").and_then(Value::as_str) {
                artifacts.push(value.to_string());
            } else if let Some(value) = item.get("uri").and_then(Value::as_str) {
                artifacts.push(value.to_string());
            } else if let Some(value) = item.get("url").and_then(Value::as_str) {
                artifacts.push(value.to_string());
            }
        }
    }
    for key in ["artifact", "output", "session_uri", "transcript_path"] {
        if let Some(value) = response.get(key).and_then(Value::as_str) {
            artifacts.push(value.to_string());
        }
    }
    artifacts.sort();
    artifacts.dedup();
    artifacts
}

fn action_url_from_endpoint(endpoint: &str, action_path: &str) -> String {
    if endpoint_is_base(endpoint) {
        join_url(endpoint, action_path)
    } else {
        endpoint.to_string()
    }
}

fn endpoint_is_base(endpoint: &str) -> bool {
    let Some(after_scheme) = endpoint.split_once("://").map(|(_, rest)| rest) else {
        return !endpoint.contains('/');
    };
    after_scheme
        .split_once('/')
        .map(|(_, path)| path.trim().is_empty())
        .unwrap_or(true)
}

fn join_url(base: &str, path: &str) -> String {
    format!(
        "{}/{}",
        base.trim_end_matches('/'),
        path.trim_start_matches('/')
    )
}

fn write_json_file(path: &Path, value: &Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create Hermes MCP bridge dir {}", parent.display()))?;
    }
    fs::write(path, serde_json::to_vec_pretty(value)?)
        .with_context(|| format!("write Hermes MCP bridge file {}", path.display()))
}

fn parse_json_body(body: &str) -> Result<Value> {
    if body.trim().is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_str(body).context("decode Hermes MCP bridge JSON request")
}

fn read_http_request(stream: &mut impl Read) -> Result<BridgeWorkerHttpRequest> {
    let mut buffer = [0_u8; 8192];
    let bytes_read = stream.read(&mut buffer).context("read HTTP request")?;
    let mut request_bytes = buffer[..bytes_read].to_vec();
    let mut headers_end = find_headers_end(&request_bytes);
    let mut content_length = headers_end
        .and_then(|end| parse_content_length(&request_bytes[..end]))
        .unwrap_or(0);

    while headers_end.is_none() || request_body_len(&request_bytes, headers_end) < content_length {
        let mut chunk = [0_u8; 8192];
        let read = stream.read(&mut chunk).context("read HTTP request body")?;
        if read == 0 {
            break;
        }
        request_bytes.extend_from_slice(&chunk[..read]);
        headers_end = find_headers_end(&request_bytes);
        content_length = headers_end
            .and_then(|end| parse_content_length(&request_bytes[..end]))
            .unwrap_or(content_length);
    }

    let request = std::str::from_utf8(&request_bytes).context("parse HTTP request bytes")?;
    let first_line = request.lines().next().unwrap_or_default();
    let mut parts = first_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let path = parts.next().unwrap_or_default();
    if method.is_empty() {
        bail!("missing HTTP request method");
    }
    if !matches!(method, "GET" | "POST" | "OPTIONS") {
        bail!("unsupported HTTP method: {method}");
    }
    if path.is_empty() {
        bail!("missing HTTP request path");
    }

    Ok(BridgeWorkerHttpRequest {
        method: method.to_string(),
        path: path.to_string(),
        body: extract_body(&request_bytes, headers_end).unwrap_or_default(),
    })
}

fn find_headers_end(bytes: &[u8]) -> Option<usize> {
    bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|index| index + 4)
}

fn parse_content_length(headers: &[u8]) -> Option<usize> {
    let headers = std::str::from_utf8(headers).ok()?;
    headers.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        if name.eq_ignore_ascii_case("content-length") {
            value.trim().parse::<usize>().ok()
        } else {
            None
        }
    })
}

fn request_body_len(bytes: &[u8], headers_end: Option<usize>) -> usize {
    headers_end
        .map(|end| bytes.len().saturating_sub(end))
        .unwrap_or_default()
}

fn extract_body(bytes: &[u8], headers_end: Option<usize>) -> Option<String> {
    let body = bytes.get(headers_end?..)?;
    Some(String::from_utf8_lossy(body).to_string())
}

fn status_text(status_code: u16) -> &'static str {
    match status_code {
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        204 => "No Content",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        _ => "OK",
    }
}

fn slug(value: &str) -> String {
    let slug = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
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
        "action".to_string()
    } else {
        slug
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_request() -> Value {
        json!({
            "adapter_id": "hermes",
            "action_kind": "CreateScene",
            "priority": "ApiMcp",
            "payload": {
                "project_slug": "demo",
                "instruction": "coordinate Unreal scene assembly",
                "target_adapter": "unreal",
                "target_action_kind": "CreateScene"
            },
            "requires_confirmation": false,
            "pool_hermes_action": {
                "profile_id": "hermes-coordinate-software",
                "operation": "coordinate_software_action",
                "mcp_tool": "hermes.coordinate",
                "stage": "agent_orchestration",
                "expected_artifacts": ["session", "transcript", "task_plan"],
                "output_contract": "hermes-mcp-action-result"
            },
            "mcp_payload": {
                "tool": "hermes.coordinate",
                "operation": "coordinate_software_action",
                "arguments": {
                    "project_slug": "demo",
                    "instruction": "coordinate Unreal scene assembly",
                    "target_adapter": "unreal",
                    "target_action_kind": "CreateScene"
                },
                "handoff": {
                    "stage": "agent_orchestration",
                    "expected_artifacts": ["session", "transcript", "task_plan"]
                }
            }
        })
    }

    #[test]
    fn worker_health_reports_dry_run_mode() {
        let mut worker = HermesMcpBridgeWorker::new(HermesMcpBridgeWorkerOptions::new(
            "http://127.0.0.1:0",
            temp_output_dir("health"),
        ));
        let response = worker.handle("GET", "/health", "").unwrap();
        let value: Value = serde_json::from_slice(&response.body).unwrap();

        assert_eq!(response.status_code, 200);
        assert_eq!(value["service"], "pool-hermes-mcp-bridge");
        assert_eq!(value["mode"], "dry_run");
    }

    #[test]
    fn worker_dry_run_writes_request_and_response_files() {
        let output_root = temp_output_dir("dry-run");
        let mut worker = HermesMcpBridgeWorker::new(HermesMcpBridgeWorkerOptions::new(
            "http://127.0.0.1:0",
            &output_root,
        ));
        let response = worker
            .handle("POST", "/mcp", &sample_request().to_string())
            .unwrap();
        let value: Value = serde_json::from_slice(&response.body).unwrap();

        assert_eq!(response.status_code, 200);
        assert_eq!(value["ok"], true);
        assert_eq!(value["pool_hermes_bridge"]["mode"], "dry_run");
        assert_eq!(value["pool_hermes_bridge"]["tool"], "hermes.coordinate");
        let request_path = value["pool_hermes_bridge"]["request_path"]
            .as_str()
            .unwrap();
        let response_path = value["pool_hermes_bridge"]["response_path"]
            .as_str()
            .unwrap();
        assert!(Path::new(request_path).exists());
        assert!(Path::new(response_path).exists());
        assert!(value["artifacts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|artifact| artifact
                .as_str()
                .unwrap_or_default()
                .starts_with("hermes://bridge/coordinate_software_action/")));
    }

    #[test]
    fn worker_self_check_runs_health_and_dry_run_action() {
        let output_root = temp_output_dir("self-check");
        let mut worker = HermesMcpBridgeWorker::new(HermesMcpBridgeWorkerOptions::new(
            "http://127.0.0.1:8792",
            &output_root,
        ));

        let results = worker.self_check().unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, "GET /health");
        assert_eq!(results[0].1, 200);
        assert_eq!(results[1].0, "POST /mcp");
        assert_eq!(results[1].1, 200);
        assert!(output_root.join("control/hermes-mcp-bridge").exists());
    }

    #[test]
    fn worker_rejects_missing_pool_wrapper() {
        let mut worker = HermesMcpBridgeWorker::new(HermesMcpBridgeWorkerOptions::new(
            "http://127.0.0.1:0",
            temp_output_dir("invalid"),
        ));
        let response = worker
            .handle("POST", "/mcp", r#"{"adapter_id":"hermes"}"#)
            .unwrap();
        let value: Value = serde_json::from_slice(&response.body).unwrap();

        assert_eq!(response.status_code, 400);
        assert_eq!(value["error"], "invalid_hermes_mcp_bridge_request");
        assert!(value["message"]
            .as_str()
            .unwrap()
            .contains("missing required Hermes MCP bridge field"));
    }

    fn temp_output_dir(label: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "pool-hermes-mcp-bridge-{label}-{}",
            Uuid::new_v4().simple()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }
}
