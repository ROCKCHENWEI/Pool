use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
struct SdkJobRecord {
    family: String,
    provider_id: String,
    output_slug: String,
    output_extension: Option<String>,
    job_dir: PathBuf,
}

pub struct ProviderSdkWorkerTemplate {
    base_url: String,
    output_root: PathBuf,
    next_id: u64,
    jobs: HashMap<String, SdkJobRecord>,
}

impl ProviderSdkWorkerTemplate {
    pub fn new(base_url: impl Into<String>, output_root: impl Into<PathBuf>) -> Self {
        Self {
            base_url: base_url.into(),
            output_root: output_root.into(),
            next_id: 1,
            jobs: HashMap::new(),
        }
    }

    pub fn self_check(&mut self) -> Result<Vec<(String, u16, usize)>> {
        let mut results = Vec::new();
        for (method, path, body) in [
            ("GET", "/health", ""),
            (
                "POST",
                "/v1/media/jobs",
                r#"{"service":"nano-banana-pro","mode":"reference_guided_image","prompt":"make hero","inputs":{"local_input_manifest":[{"path":"worlds/demo/source/ref.png","exists":false,"mime_type":"image/png"}]},"outputs":{"slug":"nano","extension":"png"}}"#,
            ),
            ("GET", "/v1/media/jobs/sdk-media-nano-banana-pro-0001", ""),
            (
                "POST",
                "/v1/3dgs/triposplat/jobs",
                r#"{"service":"tripo-splat","mode":"image_to_splat_object","prompt":"make object","inputs":{"local_input_manifest":[{"path":"worlds/demo/source/prop.png","exists":false,"mime_type":"image/png"}]},"outputs":{"slug":"object","contract":"image-blaster-indexed-files"}}"#,
            ),
            (
                "GET",
                "/v1/3dgs/triposplat/jobs/sdk-3dgs-tripo-splat-0002",
                "",
            ),
        ] {
            let response = self.handle(method, path, body)?;
            results.push((
                format!("{method} {path}"),
                response.status_code,
                response.body.len(),
            ));
        }
        Ok(results)
    }

    pub fn serve_listener(&mut self, listener: TcpListener, max_requests: usize) -> Result<usize> {
        let mut handled = 0_usize;
        for stream in listener.incoming() {
            let stream = stream.context("accept provider SDK worker connection")?;
            if let Err(error) = self.handle_tcp_connection(stream) {
                eprintln!("provider SDK worker connection error: {error}");
            }
            handled += 1;
            if max_requests > 0 && handled >= max_requests {
                break;
            }
        }
        Ok(handled)
    }

    fn handle_tcp_connection(&mut self, mut stream: TcpStream) -> Result<()> {
        let request = read_http_request(&mut stream)?;
        let response = self
            .handle(&request.method, &request.path, &request.body)
            .unwrap_or_else(HttpResponse::from_error);
        stream
            .write_all(&response.to_http_bytes())
            .context("write provider SDK worker HTTP response")
    }

    fn handle(&mut self, method: &str, raw_path: &str, body: &str) -> Result<HttpResponse> {
        let path = raw_path.split('?').next().unwrap_or(raw_path);
        if method == "OPTIONS" {
            return Ok(HttpResponse::empty(204));
        }
        if method == "GET" && matches!(path, "/" | "/health" | "/v1/health") {
            return HttpResponse::json(
                200,
                json!({
                    "status": "ready",
                    "service": "pool-provider-sdk-worker-template",
                    "purpose": "Replace call_vendor_sdk with real vendor SDK/API logic.",
                    "accepts_local_input_manifest": true,
                    "provider_urls_are_provenance": true,
                }),
            );
        }
        if method == "POST" && path == "/v1/media/jobs" {
            return self.submit("media", path, body);
        }
        if method == "POST" && is_3dgs_submit_path(path) {
            return self.submit("3dgs", path, body);
        }
        if method == "GET" {
            if let Some(job_id) = path.strip_prefix("/v1/media/jobs/") {
                return self.poll(job_id);
            }
            if let Some(job_id) = three_dgs_job_id(path) {
                return self.poll(job_id);
            }
            if let Some((job_id, file_name)) = output_path(path) {
                return self.output(job_id, file_name);
            }
        }
        HttpResponse::json(
            404,
            json!({
                "error": "not_found",
                "method": method,
                "path": path,
            }),
        )
    }

    fn submit(&mut self, family: &str, path: &str, body: &str) -> Result<HttpResponse> {
        let request = parse_json_body(body)?;
        let provider_id = provider_id(&request, family);
        let output_slug = output_slug(&request, family, &provider_id);
        let output_extension = if family == "media" {
            Some(output_extension(&request, &provider_id))
        } else {
            None
        };
        let manifest = local_input_manifest(&request);
        if manifest.is_empty() {
            bail!("local_input_manifest is required for provider SDK worker template submissions");
        }
        let job_id = format!("sdk-{family}-{provider_id}-{:04}", self.next_id);
        self.next_id += 1;
        let job_dir = self.output_root.join(&job_id);
        fs::create_dir_all(&job_dir)
            .with_context(|| format!("create SDK worker job dir {}", job_dir.display()))?;

        let audit = json!({
            "kind": "pool_provider_sdk_worker_template_request",
            "job_id": job_id,
            "family": family,
            "provider_id": provider_id,
            "submit_path": path,
            "request": request,
            "local_input_manifest": manifest,
            "implementation_note": "Replace this template write with the vendor SDK/API call, then normalize outputs into Pool gateway outputs[].",
        });
        fs::write(
            job_dir.join("1-sdk-worker-request.json"),
            serde_json::to_string_pretty(&audit)?,
        )
        .with_context(|| format!("write SDK worker audit {}", job_dir.display()))?;

        self.jobs.insert(
            job_id.clone(),
            SdkJobRecord {
                family: family.to_string(),
                provider_id: provider_id.clone(),
                output_slug,
                output_extension,
                job_dir,
            },
        );

        HttpResponse::json(
            200,
            json!({
                "job_id": job_id,
                "status": "queued",
                "pool_provider_sdk_worker_template": {
                    "family": family,
                    "provider_id": provider_id,
                    "audit_path": self.jobs[&job_id]
                        .job_dir
                        .join("1-sdk-worker-request.json")
                        .to_string_lossy(),
                    "next_step": "Replace template output generation with real vendor SDK/API call.",
                }
            }),
        )
    }

    fn poll(&mut self, job_id: &str) -> Result<HttpResponse> {
        let Some(job) = self.jobs.get(job_id).cloned() else {
            return HttpResponse::json(404, json!({"error":"unknown_job", "job_id": job_id}));
        };
        write_template_outputs(&job)?;
        let outputs = output_items(&self.base_url, job_id, &job);
        HttpResponse::json(
            200,
            json!({
                "job_id": job_id,
                "status": "completed",
                "outputs": outputs,
                "pool_provider_sdk_worker_template": {
                    "provider_id": job.provider_id,
                    "audit_path": job.job_dir.join("1-sdk-worker-request.json").to_string_lossy(),
                }
            }),
        )
    }

    fn output(&self, job_id: &str, file_name: &str) -> Result<HttpResponse> {
        let Some(job) = self.jobs.get(job_id) else {
            return HttpResponse::json(404, json!({"error":"unknown_job", "job_id": job_id}));
        };
        let path = job.job_dir.join(file_name);
        if !path.exists() {
            return HttpResponse::json(
                404,
                json!({"error":"missing_output", "job_id": job_id, "file_name": file_name}),
            );
        }
        Ok(HttpResponse {
            status_code: 200,
            content_type: content_type(file_name).to_string(),
            body: fs::read(&path)
                .with_context(|| format!("read SDK worker output {}", path.display()))?,
        })
    }
}

#[derive(Debug, Clone)]
struct HttpResponse {
    status_code: u16,
    content_type: String,
    body: Vec<u8>,
}

impl HttpResponse {
    fn empty(status_code: u16) -> Self {
        Self {
            status_code,
            content_type: "text/plain; charset=utf-8".to_string(),
            body: Vec::new(),
        }
    }

    fn json(status_code: u16, value: Value) -> Result<Self> {
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
                "error": "provider_sdk_worker_template_error",
                "message": error.to_string(),
            }),
        )
        .unwrap_or_else(|_| Self {
            status_code: 500,
            content_type: "application/json; charset=utf-8".to_string(),
            body: br#"{"error":"provider_sdk_worker_template_error"}"#.to_vec(),
        })
    }

    fn to_http_bytes(&self) -> Vec<u8> {
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
struct HttpRequest {
    method: String,
    path: String,
    body: String,
}

fn write_template_outputs(job: &SdkJobRecord) -> Result<()> {
    if job.family == "media" {
        let extension = job.output_extension.as_deref().unwrap_or("bin");
        let file_name = format!("{}-sdk-template.{extension}", job.output_slug);
        fs::write(
            job.job_dir.join(file_name),
            format!(
                "Pool SDK worker template output for provider {}.\nReplace this with real vendor SDK output.\n",
                job.provider_id
            ),
        )?;
        return Ok(());
    }

    fs::write(
        job.job_dir.join(format!("{}.json", job.output_slug)),
        serde_json::to_vec_pretty(&json!({
            "provider_id": job.provider_id,
            "kind": "sdk-worker-template-3dgs-metadata",
            "replace_with": "real vendor metadata",
        }))?,
    )?;
    fs::write(
        job.job_dir.join(format!("{}.glb", job.output_slug)),
        b"pool-sdk-worker-template-glb",
    )?;
    fs::write(
        job.job_dir
            .join(format!("{}-full_res.spz", job.output_slug)),
        b"pool-sdk-worker-template-spz",
    )?;
    Ok(())
}

fn output_items(base_url: &str, job_id: &str, job: &SdkJobRecord) -> Vec<Value> {
    if job.family == "media" {
        let extension = job.output_extension.as_deref().unwrap_or("bin");
        let file_name = format!("{}-sdk-template.{extension}", job.output_slug);
        return vec![json!({
            "name": file_name,
            "url": format!("{}/outputs/{}/{}", base_url.trim_end_matches('/'), job_id, file_name),
            "extension": extension,
        })];
    }

    [
        format!("{}.json", job.output_slug),
        format!("{}.glb", job.output_slug),
        format!("{}-full_res.spz", job.output_slug),
    ]
    .into_iter()
    .map(|file_name| {
        json!({
            "name": file_name,
            "url": format!("{}/outputs/{}/{}", base_url.trim_end_matches('/'), job_id, file_name),
        })
    })
    .collect()
}

fn provider_id(request: &Value, family: &str) -> String {
    string_at(request, "/provider_id")
        .or_else(|| string_at(request, "/service"))
        .unwrap_or_else(|| {
            if family == "media" {
                "generic-media".to_string()
            } else {
                "generic-3dgs".to_string()
            }
        })
}

fn output_slug(request: &Value, family: &str, provider_id: &str) -> String {
    string_at(request, "/output_slug")
        .or_else(|| string_at(request, "/outputs/slug"))
        .unwrap_or_else(|| {
            if family == "media" {
                media_default_slug(provider_id).to_string()
            } else {
                three_dgs_default_slug(provider_id).to_string()
            }
        })
}

fn output_extension(request: &Value, provider_id: &str) -> String {
    string_at(request, "/output_extension")
        .or_else(|| string_at(request, "/outputs/extension"))
        .unwrap_or_else(|| media_default_extension(provider_id).to_string())
}

fn local_input_manifest(request: &Value) -> Vec<Value> {
    request
        .pointer("/inputs/local_input_manifest")
        .or_else(|| request.get("local_input_manifest"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

fn media_default_slug(provider_id: &str) -> &'static str {
    match provider_id {
        "suno" => "suno-cue",
        "nano-banana-pro" | "nano-banana" | "nanobanana" => "nano",
        "midjourney" | "mj" => "midjourney",
        _ => "media",
    }
}

fn media_default_extension(provider_id: &str) -> &'static str {
    if provider_id == "suno" {
        "mp3"
    } else {
        "png"
    }
}

fn three_dgs_default_slug(provider_id: &str) -> &'static str {
    match provider_id {
        "tripo-splat" | "triposplat" | "sam-3d" | "sam3d" => "object",
        "spark-3dgs" | "spark" | "qunhe-3d" | "qunhe" => "scene",
        _ => "world",
    }
}

fn string_at(value: &Value, pointer: &str) -> Option<String> {
    value.pointer(pointer)?.as_str().map(ToString::to_string)
}

fn is_3dgs_submit_path(path: &str) -> bool {
    path.starts_with("/v1/3dgs") && path.ends_with("/jobs")
}

fn three_dgs_job_id(path: &str) -> Option<&str> {
    if !path.starts_with("/v1/3dgs") {
        return None;
    }
    path.rsplit_once("/jobs/")
        .map(|(_, job_id)| job_id)
        .filter(|job_id| !job_id.is_empty())
}

fn output_path(path: &str) -> Option<(&str, &str)> {
    let rest = path.strip_prefix("/outputs/")?;
    rest.split_once('/')
}

fn parse_json_body(body: &str) -> Result<Value> {
    if body.trim().is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_str(body).context("decode provider SDK worker JSON request")
}

fn read_http_request(stream: &mut impl Read) -> Result<HttpRequest> {
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

    Ok(HttpRequest {
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

fn content_type(file_name: &str) -> &'static str {
    match Path::new(file_name)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "mp3" => "audio/mpeg",
        "json" => "application/json",
        "glb" => "model/gltf-binary",
        _ => "application/octet-stream",
    }
}

fn status_text(status_code: u16) -> &'static str {
    match status_code {
        200 => "OK",
        204 => "No Content",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "OK",
    }
}
