use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose, Engine as _};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;

#[derive(Debug, Clone)]
struct JobRecord {
    family: String,
    provider_id: String,
    output_slug: String,
    output_extension: Option<String>,
}

pub struct ProviderGatewayMock {
    base_url: String,
    next_id: u64,
    jobs: HashMap<String, JobRecord>,
}

impl ProviderGatewayMock {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
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
                r#"{"provider_id":"nano-banana-pro","prompt":"make hero plate","output_slug":"nano","output_extension":"png"}"#,
            ),
            ("GET", "/v1/media/jobs/media-nano-banana-pro-0001", ""),
            (
                "POST",
                "/v1/3dgs/jobs",
                r#"{"provider_id":"worldlabs-marble","prompt":"make world","output_slug":"world"}"#,
            ),
            ("GET", "/v1/3dgs/jobs/3dgs-worldlabs-marble-0002", ""),
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
            let stream = stream.context("accept mock gateway connection")?;
            if let Err(error) = self.handle_tcp_connection(stream) {
                eprintln!("mock gateway connection error: {error}");
            }
            handled += 1;
            if max_requests > 0 && handled >= max_requests {
                break;
            }
        }
        Ok(handled)
    }

    pub fn handle_tcp_connection(&mut self, mut stream: TcpStream) -> Result<()> {
        let request = read_http_request(&mut stream)?;
        let response = self
            .handle(&request.method, &request.path, &request.body)
            .unwrap_or_else(ProviderGatewayMockResponse::from_error);
        stream
            .write_all(&response.to_http_bytes())
            .context("write mock gateway HTTP response")
    }

    pub fn handle(
        &mut self,
        method: &str,
        raw_path: &str,
        body: &str,
    ) -> Result<ProviderGatewayMockResponse> {
        let path = raw_path.split('?').next().unwrap_or(raw_path);
        if method == "OPTIONS" {
            return Ok(ProviderGatewayMockResponse::empty(204));
        }
        if method == "GET" && matches!(path, "/" | "/health" | "/v1/health") {
            return ProviderGatewayMockResponse::json(
                200,
                json!({
                    "status": "ready",
                    "service": "pool-provider-gateway-mock",
                    "families": ["ai_media", "3dgs"],
                    "local_files_authoritative": true,
                    "provider_urls_are_provenance": true,
                }),
            );
        }
        if method == "POST" && path == "/v1/media/jobs" {
            return self.submit_media(body);
        }
        if method == "GET" {
            if let Some(job_id) = path.strip_prefix("/v1/media/jobs/") {
                return self.poll_media(job_id);
            }
            if let Some(job_id) = three_dgs_job_id(path) {
                return self.poll_3dgs(job_id);
            }
            if let Some((family, job_id, file_name)) = output_path(path) {
                return self.output(family, job_id, file_name);
            }
        }
        if method == "POST" && is_3dgs_submit_path(path) {
            return self.submit_3dgs(body);
        }

        ProviderGatewayMockResponse::json(
            404,
            json!({
                "error": "not_found",
                "method": method,
                "path": path,
            }),
        )
    }

    fn submit_media(&mut self, body: &str) -> Result<ProviderGatewayMockResponse> {
        let request = parse_json_body(body)?;
        let provider_id = string_at(&request, "/provider_id")
            .or_else(|| string_at(&request, "/pool_media_profile/provider_id"))
            .or_else(|| string_at(&request, "/service"))
            .unwrap_or_else(|| "generic-media".to_string());
        let output_slug = string_at(&request, "/output_slug")
            .or_else(|| string_at(&request, "/pool_media_profile/output_slug"))
            .or_else(|| string_at(&request, "/outputs/slug"))
            .unwrap_or_else(|| media_default_slug(&provider_id).to_string());
        let output_extension = string_at(&request, "/output_extension")
            .or_else(|| string_at(&request, "/pool_media_profile/output_extension"))
            .or_else(|| string_at(&request, "/outputs/extension"))
            .unwrap_or_else(|| media_default_extension(&provider_id).to_string());
        let job_id = self.next_job_id("media", &provider_id);
        self.jobs.insert(
            job_id.clone(),
            JobRecord {
                family: "media".to_string(),
                provider_id: provider_id.clone(),
                output_slug,
                output_extension: Some(output_extension.clone()),
            },
        );

        ProviderGatewayMockResponse::json(
            200,
            json!({
                "job_id": job_id,
                "status": "queued",
                "pool_gateway_mock": {
                    "family": "ai_media",
                    "provider_id": provider_id,
                    "profile_id": string_at(&request, "/pool_media_profile/profile_id"),
                    "output_contract": "local-media-files",
                }
            }),
        )
    }

    fn poll_media(&self, job_id: &str) -> Result<ProviderGatewayMockResponse> {
        let Some(job) = self.jobs.get(job_id) else {
            return ProviderGatewayMockResponse::json(
                404,
                json!({"error":"unknown_job", "job_id": job_id}),
            );
        };
        let extension = job.output_extension.as_deref().unwrap_or("bin");
        let file_name = format!("{}-mock.{extension}", job.output_slug);
        ProviderGatewayMockResponse::json(
            200,
            json!({
                "job_id": job_id,
                "status": "completed",
                "outputs": [
                    {
                        "name": file_name,
                        "url": self.output_url("media", job_id, &file_name),
                        "extension": extension,
                    }
                ],
            }),
        )
    }

    fn submit_3dgs(&mut self, body: &str) -> Result<ProviderGatewayMockResponse> {
        let request = parse_json_body(body)?;
        let provider_id = string_at(&request, "/provider_id")
            .or_else(|| string_at(&request, "/pool_gateway_profile/provider_id"))
            .or_else(|| string_at(&request, "/service"))
            .unwrap_or_else(|| "generic-3dgs".to_string());
        let output_slug = string_at(&request, "/output_slug")
            .or_else(|| string_at(&request, "/pool_gateway_profile/output_slug"))
            .or_else(|| string_at(&request, "/outputs/slug"))
            .unwrap_or_else(|| three_dgs_default_slug(&provider_id).to_string());
        let job_id = self.next_job_id("3dgs", &provider_id);
        self.jobs.insert(
            job_id.clone(),
            JobRecord {
                family: "3dgs".to_string(),
                provider_id: provider_id.clone(),
                output_slug,
                output_extension: None,
            },
        );

        ProviderGatewayMockResponse::json(
            200,
            json!({
                "job_id": job_id,
                "status": "queued",
                "pool_gateway_mock": {
                    "family": "3dgs",
                    "provider_id": provider_id,
                    "profile_id": string_at(&request, "/pool_gateway_profile/profile_id"),
                    "output_contract": "image-blaster-indexed-files",
                }
            }),
        )
    }

    fn poll_3dgs(&self, job_id: &str) -> Result<ProviderGatewayMockResponse> {
        let Some(job) = self.jobs.get(job_id) else {
            return ProviderGatewayMockResponse::json(
                404,
                json!({"error":"unknown_job", "job_id": job_id}),
            );
        };
        let metadata_name = format!("{}.json", job.output_slug);
        let mesh_name = format!("{}.glb", job.output_slug);
        let splat_name = format!("{}-full_res.spz", job.output_slug);
        ProviderGatewayMockResponse::json(
            200,
            json!({
                "job_id": job_id,
                "status": "completed",
                "outputs": [
                    {"name": metadata_name, "url": self.output_url("3dgs", job_id, &metadata_name)},
                    {"name": mesh_name, "url": self.output_url("3dgs", job_id, &mesh_name)},
                    {"name": splat_name, "url": self.output_url("3dgs", job_id, &splat_name)}
                ],
            }),
        )
    }

    fn output(
        &self,
        family: &str,
        job_id: &str,
        file_name: &str,
    ) -> Result<ProviderGatewayMockResponse> {
        let Some(job) = self.jobs.get(job_id) else {
            return ProviderGatewayMockResponse::json(
                404,
                json!({"error":"unknown_job", "job_id": job_id}),
            );
        };
        if job.family != family {
            return ProviderGatewayMockResponse::json(
                404,
                json!({"error":"wrong_family", "family": family, "job_id": job_id}),
            );
        }

        let extension = file_name
            .rsplit_once('.')
            .map(|(_, extension)| extension.to_ascii_lowercase())
            .unwrap_or_else(|| "bin".to_string());
        match extension.as_str() {
            "json" => ProviderGatewayMockResponse::bytes(
                200,
                "application/json; charset=utf-8",
                serde_json::to_vec_pretty(&json!({
                    "kind": "pool_gateway_mock_output",
                    "family": family,
                    "job_id": job_id,
                    "provider_id": job.provider_id,
                    "output_slug": job.output_slug,
                    "file_name": file_name,
                }))?,
            ),
            "png" => ProviderGatewayMockResponse::bytes(
                200,
                "image/png",
                general_purpose::STANDARD
                    .decode(PNG_1X1_BASE64)
                    .context("decode embedded mock png")?,
            ),
            "mp3" => ProviderGatewayMockResponse::bytes(
                200,
                "audio/mpeg",
                b"ID3\x04\x00\x00\x00\x00\x00\x15Pool mock audio output\n".to_vec(),
            ),
            "glb" => ProviderGatewayMockResponse::bytes(
                200,
                "model/gltf-binary",
                b"glTF\x02\x00\x00\x00\x0c\x00\x00\x00".to_vec(),
            ),
            _ => ProviderGatewayMockResponse::bytes(
                200,
                "application/octet-stream",
                format!(
                    "Pool gateway mock output\nfamily={family}\njob_id={job_id}\nfile={file_name}\n"
                )
                .into_bytes(),
            ),
        }
    }

    fn output_url(&self, family: &str, job_id: &str, file_name: &str) -> String {
        format!(
            "{}/outputs/{}/{}/{}",
            self.base_url.trim_end_matches('/'),
            family,
            job_id,
            file_name
        )
    }

    fn next_job_id(&mut self, family: &str, provider_id: &str) -> String {
        let job_id = format!("{family}-{}-{:04}", safe_slug(provider_id), self.next_id);
        self.next_id += 1;
        job_id
    }
}

pub fn spawn_provider_gateway_mock(max_requests: usize) -> Result<String> {
    let listener = TcpListener::bind("127.0.0.1:0").context("bind provider gateway mock")?;
    let addr = listener
        .local_addr()
        .context("read provider gateway mock addr")?;
    let base_url = format!("http://{addr}");
    let thread_base_url = base_url.clone();
    thread::spawn(move || {
        let mut gateway = ProviderGatewayMock::new(thread_base_url);
        if let Err(error) = gateway.serve_listener(listener, max_requests) {
            eprintln!("provider gateway mock server error: {error}");
        }
    });
    Ok(base_url)
}

#[derive(Debug, Clone)]
pub struct ProviderGatewayMockResponse {
    pub status_code: u16,
    pub content_type: String,
    pub body: Vec<u8>,
}

impl ProviderGatewayMockResponse {
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

    pub fn bytes(status_code: u16, content_type: impl Into<String>, body: Vec<u8>) -> Result<Self> {
        Ok(Self {
            status_code,
            content_type: content_type.into(),
            body,
        })
    }

    fn from_error(error: anyhow::Error) -> Self {
        Self::json(
            500,
            json!({
                "error": "provider_gateway_mock_error",
                "message": error.to_string(),
            }),
        )
        .unwrap_or_else(|_| Self {
            status_code: 500,
            content_type: "application/json; charset=utf-8".to_string(),
            body: br#"{"error":"provider_gateway_mock_error"}"#.to_vec(),
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
struct MockHttpRequest {
    method: String,
    path: String,
    body: String,
}

fn read_http_request(stream: &mut impl Read) -> Result<MockHttpRequest> {
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

    Ok(MockHttpRequest {
        method: method.to_string(),
        path: path.to_string(),
        body: extract_body(&request_bytes, headers_end).unwrap_or_default(),
    })
}

fn parse_json_body(body: &str) -> Result<Value> {
    if body.trim().is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_str(body).context("decode mock gateway JSON request")
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

fn output_path(path: &str) -> Option<(&str, &str, &str)> {
    let path = path.strip_prefix("/outputs/")?;
    let mut parts = path.splitn(3, '/');
    let family = parts.next()?;
    let job_id = parts.next()?;
    let file_name = parts.next()?;
    if family.is_empty() || job_id.is_empty() || file_name.is_empty() {
        return None;
    }
    Some((family, job_id, file_name))
}

fn media_default_slug(provider_id: &str) -> &'static str {
    match provider_id {
        "midjourney" | "mj" => "midjourney",
        "nano-banana-pro" | "nano-banana" | "nanobanana" | "nanobananapro" => "nano",
        "suno" => "suno-cue",
        _ => "media",
    }
}

fn media_default_extension(provider_id: &str) -> &'static str {
    match provider_id {
        "suno" => "mp3",
        "generic-media" => "bin",
        _ => "png",
    }
}

fn three_dgs_default_slug(provider_id: &str) -> &'static str {
    match provider_id {
        "tripo-splat" | "triposplat" | "sam-3d" | "sam3d" => "object",
        "spark-3dgs" | "spark" | "qunhe-3d" | "qunhe" => "scene",
        _ => "world",
    }
}

fn safe_slug(value: &str) -> String {
    let slug = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    if slug.is_empty() {
        "provider".to_string()
    } else {
        slug
    }
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
        500 => "Internal Server Error",
        _ => "OK",
    }
}

const PNG_1X1_BASE64: &str =
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+/p9sAAAAASUVORK5CYII=";
