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
pub struct SoftwareApiBridgeWorkerOptions {
    pub adapter_id: String,
    pub base_url: String,
    pub output_root: PathBuf,
    pub default_upstream_endpoint: Option<String>,
    pub api_key: Option<String>,
    pub action_path: String,
}

impl SoftwareApiBridgeWorkerOptions {
    pub fn new(
        adapter_id: impl Into<String>,
        base_url: impl Into<String>,
        output_root: impl Into<PathBuf>,
    ) -> Self {
        Self {
            adapter_id: adapter_id.into(),
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
}

pub struct SoftwareApiBridgeWorker {
    options: SoftwareApiBridgeWorkerOptions,
    client: Client,
}

impl SoftwareApiBridgeWorker {
    pub fn new(options: SoftwareApiBridgeWorkerOptions) -> Self {
        Self {
            options,
            client: Client::new(),
        }
    }

    pub fn serve_listener(&mut self, listener: TcpListener, max_requests: usize) -> Result<usize> {
        let mut handled = 0_usize;
        for stream in listener.incoming() {
            let stream = stream.context("accept software API bridge worker connection")?;
            if let Err(error) = self.handle_tcp_connection(stream) {
                eprintln!("software API bridge worker connection error: {error}");
            }
            handled += 1;
            if max_requests > 0 && handled >= max_requests {
                break;
            }
        }
        Ok(handled)
    }

    pub fn self_check(&mut self) -> Result<Vec<(String, u16, usize)>> {
        let operation = format!("{}_self_check", slug(&self.options.adapter_id));
        let request = json!({
            "adapter_id": self.options.adapter_id,
            "action_kind": "CreateScene",
            "priority": "ApiMcp",
            "payload": {
                "project_slug": "demo",
                "artifacts": [
                    format!("worlds/demo/output/production/{}/1-self-check.json", slug(&self.options.adapter_id))
                ]
            },
            "requires_confirmation": false,
            "pool_software_action": {
                "profile_id": format!("{}-generic-api-mcp", slug(&self.options.adapter_id)),
                "operation": operation,
                "stage": "software_api_mcp_control",
                "output_contract": "pool-generic-software-api-result"
            },
            "mcp_payload": {
                "tool": format!("{}.execute", slug(&self.options.adapter_id)),
                "operation": operation,
                "arguments": {
                    "project_slug": "demo",
                    "adapter_id": self.options.adapter_id
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
            .unwrap_or_else(SoftwareApiBridgeWorkerResponse::from_error);
        stream
            .write_all(&response.to_http_bytes())
            .context("write software API bridge worker HTTP response")
    }

    pub fn handle(
        &mut self,
        method: &str,
        raw_path: &str,
        body: &str,
    ) -> Result<SoftwareApiBridgeWorkerResponse> {
        let path = raw_path.split('?').next().unwrap_or(raw_path);
        if method == "OPTIONS" {
            return Ok(SoftwareApiBridgeWorkerResponse::empty(204));
        }
        if method == "GET" && matches!(path, "/" | "/health" | "/v1/health") {
            return SoftwareApiBridgeWorkerResponse::json(
                200,
                json!({
                    "ok": true,
                    "status": "ready",
                    "service": "pool-software-api-bridge",
                    "adapter_id": self.options.adapter_id,
                    "mode": if self.options.default_upstream_endpoint.is_some() { "forwarder" } else { "dry_run" },
                    "base_url": self.options.base_url,
                    "output_root": self.options.output_root,
                    "has_default_upstream": self.options.default_upstream_endpoint.is_some(),
                    "local_files_authoritative": true,
                }),
            );
        }
        if method == "POST" && matches!(path, "/mcp" | "/v1/software/actions") {
            return self.submit(body);
        }

        SoftwareApiBridgeWorkerResponse::json(
            404,
            json!({
                "error": "not_found",
                "method": method,
                "path": path,
            }),
        )
    }

    fn submit(&mut self, body: &str) -> Result<SoftwareApiBridgeWorkerResponse> {
        let request = match parse_json_body(body) {
            Ok(request) => request,
            Err(error) => {
                return SoftwareApiBridgeWorkerResponse::json(
                    400,
                    json!({
                        "error": "invalid_software_api_bridge_request",
                        "message": error.to_string(),
                    }),
                );
            }
        };
        if let Err(error) = validate_software_api_bridge_request(&self.options.adapter_id, &request)
        {
            return SoftwareApiBridgeWorkerResponse::json(
                400,
                json!({
                    "error": "invalid_software_api_bridge_request",
                    "message": error.to_string(),
                }),
            );
        }
        let action_id = bridge_action_id(&self.options.adapter_id, &request);
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
            dry_run_software_api_response(&self.options.adapter_id, &request, &action_id)
        };

        let response = normalize_software_api_response(
            &self.options.adapter_id,
            &request,
            upstream_response,
            &action_id,
            &request_path,
            &response_path,
            self.options.default_upstream_endpoint.is_some(),
        );
        write_json_file(&response_path, &response)?;
        SoftwareApiBridgeWorkerResponse::json(200, response)
    }

    fn output_dir(&self) -> Result<PathBuf> {
        let dir = self
            .options
            .output_root
            .join("control/software-api-bridge")
            .join(slug(&self.options.adapter_id));
        fs::create_dir_all(&dir)
            .with_context(|| format!("create software API bridge output dir {}", dir.display()))?;
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
            .context("create software API bridge worker tokio runtime")?;
        runtime.block_on(async move {
            let mut builder = client.post(&url).json(&body);
            if let Some(api_key) = api_key {
                builder = builder.bearer_auth(api_key);
            }
            builder
                .send()
                .await
                .with_context(|| format!("submit software API bridge upstream action {url}"))?
                .error_for_status()
                .with_context(|| format!("upstream software API bridge returned error for {url}"))?
                .json()
                .await
                .with_context(|| format!("decode upstream software API bridge response {url}"))
        })
    }
}

pub fn spawn_software_api_bridge_worker(
    adapter_id: impl Into<String>,
    output_root: impl Into<PathBuf>,
    max_requests: usize,
) -> Result<String> {
    let listener = TcpListener::bind("127.0.0.1:0").context("bind software API bridge worker")?;
    let addr = listener
        .local_addr()
        .context("read software API bridge worker addr")?;
    let base_url = format!("http://{addr}");
    let options = SoftwareApiBridgeWorkerOptions::new(adapter_id, base_url.clone(), output_root);
    thread::spawn(move || {
        let mut worker = SoftwareApiBridgeWorker::new(options);
        if let Err(error) = worker.serve_listener(listener, max_requests) {
            eprintln!("software API bridge worker server error: {error}");
        }
    });
    Ok(base_url)
}

#[derive(Debug, Clone)]
pub struct SoftwareApiBridgeWorkerResponse {
    pub status_code: u16,
    pub content_type: String,
    pub body: Vec<u8>,
}

impl SoftwareApiBridgeWorkerResponse {
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
                "error": "software_api_bridge_worker_error",
                "message": error.to_string(),
            }),
        )
        .unwrap_or_else(|_| Self {
            status_code: 500,
            content_type: "application/json; charset=utf-8".to_string(),
            body: br#"{"error":"software_api_bridge_worker_error"}"#.to_vec(),
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

fn validate_software_api_bridge_request(adapter_id: &str, request: &Value) -> Result<()> {
    for field in [
        "adapter_id",
        "action_kind",
        "priority",
        "payload",
        "pool_software_action",
        "mcp_payload",
    ] {
        if request.get(field).is_none() {
            bail!("missing required software API bridge field: {field}");
        }
    }
    if request.get("adapter_id").and_then(Value::as_str) != Some(adapter_id) {
        bail!("software API bridge only accepts its configured adapter_id");
    }
    let operation = request
        .pointer("/pool_software_action/operation")
        .and_then(Value::as_str)
        .context("pool_software_action.operation is required")?;
    let mcp_operation = request
        .pointer("/mcp_payload/operation")
        .and_then(Value::as_str)
        .context("mcp_payload.operation is required")?;
    if operation != mcp_operation {
        bail!("pool_software_action.operation must match mcp_payload.operation");
    }
    Ok(())
}

fn bridge_action_id(adapter_id: &str, request: &Value) -> String {
    let operation = request
        .pointer("/pool_software_action/operation")
        .and_then(Value::as_str)
        .unwrap_or("action");
    format!(
        "{}-{}-{}",
        slug(adapter_id),
        slug(operation),
        Uuid::new_v4().simple()
    )
}

fn dry_run_software_api_response(adapter_id: &str, request: &Value, action_id: &str) -> Value {
    let operation = request
        .pointer("/pool_software_action/operation")
        .and_then(Value::as_str)
        .unwrap_or("generic_action");
    json!({
        "ok": true,
        "success": true,
        "status": "completed",
        "message": format!("{adapter_id}-software-api-bridge-dry-run {operation}"),
        "artifacts": [
            format!("software-api://{adapter_id}/{action_id}"),
            format!("software-api://{adapter_id}/{operation}/{action_id}")
        ],
    })
}

fn normalize_software_api_response(
    adapter_id: &str,
    request: &Value,
    upstream_response: Value,
    action_id: &str,
    request_path: &Path,
    response_path: &Path,
    forwarded: bool,
) -> Value {
    let ok = response_ok(&upstream_response);
    let mut artifacts = collect_software_api_artifacts(&upstream_response);
    artifacts.push(request_path.to_string_lossy().into_owned());
    artifacts.push(response_path.to_string_lossy().into_owned());
    json!({
        "ok": ok,
        "success": ok,
        "status": if ok { "completed" } else { "failed" },
        "message": upstream_response
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or(if ok { "software-api-bridge-action-ok" } else { "software-api-bridge-action-failed" }),
        "artifacts": artifacts,
        "pool_software_api_bridge": {
            "action_id": action_id,
            "adapter_id": adapter_id,
            "mode": if forwarded { "forwarder" } else { "dry_run" },
            "operation": request.pointer("/pool_software_action/operation").and_then(Value::as_str),
            "profile_id": request.pointer("/pool_software_action/profile_id").and_then(Value::as_str),
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

fn collect_software_api_artifacts(response: &Value) -> Vec<String> {
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
    for key in [
        "artifact",
        "artifact_path",
        "output_path",
        "result_path",
        "report_path",
    ] {
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
            .with_context(|| format!("create software API bridge dir {}", parent.display()))?;
    }
    fs::write(path, serde_json::to_vec_pretty(value)?)
        .with_context(|| format!("write software API bridge file {}", path.display()))
}

fn parse_json_body(body: &str) -> Result<Value> {
    if body.trim().is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_str(body).context("decode software API bridge JSON request")
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
            "adapter_id": "resolve",
            "action_kind": "CreateScene",
            "priority": "ApiMcp",
            "payload": {
                "project_slug": "demo",
                "artifacts": ["worlds/demo/output/production/resolve/1-edit.mov"]
            },
            "requires_confirmation": false,
            "pool_software_action": {
                "profile_id": "resolve-generic-api-mcp",
                "operation": "create_scene",
                "stage": "software_api_mcp_control",
                "output_contract": "pool-generic-software-api-result"
            },
            "mcp_payload": {
                "tool": "resolve.execute",
                "operation": "create_scene",
                "arguments": {"project_slug": "demo"}
            }
        })
    }

    #[test]
    fn dry_run_bridge_writes_request_and_response() {
        let output_root =
            std::env::temp_dir().join(format!("pool-software-api-bridge-{}", Uuid::new_v4()));
        let mut worker = SoftwareApiBridgeWorker::new(SoftwareApiBridgeWorkerOptions::new(
            "resolve",
            "http://127.0.0.1:8793",
            &output_root,
        ));
        let response = worker
            .handle("POST", "/mcp", &sample_request().to_string())
            .unwrap();
        let value: Value = serde_json::from_slice(&response.body).unwrap();

        assert_eq!(response.status_code, 200);
        assert_eq!(value["ok"], true);
        assert_eq!(value["pool_software_api_bridge"]["adapter_id"], "resolve");
        assert_eq!(value["pool_software_api_bridge"]["mode"], "dry_run");
        assert!(value["artifacts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|artifact| artifact
                .as_str()
                .unwrap_or_default()
                .contains("-request.json")));
    }

    #[test]
    fn self_check_runs_health_and_dry_run_action() {
        let output_root =
            std::env::temp_dir().join(format!("pool-software-api-bridge-{}", Uuid::new_v4()));
        let mut worker = SoftwareApiBridgeWorker::new(SoftwareApiBridgeWorkerOptions::new(
            "resolve",
            "http://127.0.0.1:8793",
            &output_root,
        ));

        let results = worker.self_check().unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, "GET /health");
        assert_eq!(results[0].1, 200);
        assert_eq!(results[1].0, "POST /mcp");
        assert_eq!(results[1].1, 200);
        assert!(output_root
            .join("control/software-api-bridge/resolve")
            .exists());
    }

    #[test]
    fn rejects_wrong_adapter_id() {
        let output_root =
            std::env::temp_dir().join(format!("pool-software-api-bridge-{}", Uuid::new_v4()));
        let mut request = sample_request();
        request["adapter_id"] = json!("blender");
        let mut worker = SoftwareApiBridgeWorker::new(SoftwareApiBridgeWorkerOptions::new(
            "resolve",
            "http://127.0.0.1:8793",
            output_root,
        ));
        let response = worker.handle("POST", "/mcp", &request.to_string()).unwrap();
        let value: Value = serde_json::from_slice(&response.body).unwrap();

        assert_eq!(response.status_code, 400);
        assert_eq!(value["error"], "invalid_software_api_bridge_request");
    }
}
