use anyhow::{bail, Context, Result};
use reqwest::Client;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;

use super::gateway_template::{
    provider_gateway_template_translation, ProviderGatewayTemplateFamily,
};

#[derive(Debug, Clone)]
pub struct ProviderGatewayWorkerOptions {
    pub base_url: String,
    pub default_upstream_endpoint: Option<String>,
    pub api_key: Option<String>,
    pub provider_upstream_endpoints: HashMap<String, String>,
    pub provider_api_keys: HashMap<String, String>,
}

impl ProviderGatewayWorkerOptions {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            default_upstream_endpoint: None,
            api_key: None,
            provider_upstream_endpoints: HashMap::new(),
            provider_api_keys: HashMap::new(),
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

    pub fn with_provider_upstream(
        mut self,
        provider_id: impl AsRef<str>,
        endpoint: impl Into<String>,
    ) -> Self {
        self.provider_upstream_endpoints
            .insert(provider_route_key(provider_id.as_ref()), endpoint.into());
        self
    }

    pub fn with_provider_api_key(
        mut self,
        provider_id: impl AsRef<str>,
        api_key: impl Into<String>,
    ) -> Self {
        self.provider_api_keys
            .insert(provider_route_key(provider_id.as_ref()), api_key.into());
        self
    }
}

pub fn provider_gateway_worker_contract() -> Value {
    json!({
        "kind": "pool_provider_gateway_worker_contract",
        "version": 1,
        "service": "pool-provider-gateway-worker",
        "purpose": "Local HTTP forwarder for Pool AI media and 3DGS gateway requests. It translates Pool gateway bodies with provider_gateway_template and forwards them to a real vendor worker, official SDK wrapper, or mock upstream.",
        "cli": {
            "primary": "pool-cli provider-gateway-worker --bind 127.0.0.1:8788 --upstream http://127.0.0.1:8787",
            "example": "cargo run -p pool-cli -- provider-gateway-worker --bind 127.0.0.1:8788 --upstream http://127.0.0.1:8787",
            "self_check": "pool-cli provider-gateway-worker --once",
            "options": {
                "--bind": "Local bind address; default 127.0.0.1:8788.",
                "--upstream": "Base URL or submit URL for real vendor worker, SDK wrapper, or provider_gateway_mock_server.",
                "--provider-upstream": "Repeatable provider=url route for multi-upstream production runs, for example midjourney=http://127.0.0.1:9701.",
                "--max-requests": "Optional request limit for smoke tests; 0 means unlimited.",
                "--once": "Run an embedded mock-upstream self-check covering health, AI media submit/poll, and 3DGS submit/poll, then exit.",
                "--api-key": "Bearer token forwarded to upstream.",
                "--api-key-env": "Environment variable containing upstream bearer token.",
                "--provider-api-key": "Repeatable provider=token override for provider-specific upstream bearer auth.",
                "--provider-api-key-env": "Repeatable provider=ENV_NAME override for provider-specific upstream bearer auth."
            },
            "env": {
                "POOL_PROVIDER_GATEWAY_UPSTREAM": "Default upstream URL for pool-cli provider-gateway-worker.",
                "POOL_MEDIA_GATEWAY_ENDPOINT": "Set this to the worker base URL for GenericHttpMediaProvider.",
                "POOL_3DGS_GATEWAY_ENDPOINT": "Set this to the worker base URL for ThreeDgsGatewayProvider.",
                "provider_specific": "Worker also resolves provider-specific upstream endpoint env candidates emitted by provider_gateway_template_translation, such as POOL_MIDJOURNEY_ENDPOINT, POOL_TRIPOSPLAT_ENDPOINT, and POOL_QUNHE_ENDPOINT."
            }
        },
        "routes": {
            "health": {
                "method": "GET",
                "path": "/health"
            },
            "ai_media_submit": {
                "method": "POST",
                "path": "/v1/media/jobs",
                "request_profile": "pool_media_profile",
                "translated_by": "provider_gateway_template_translation(ai_media, provider_id, request)"
            },
            "ai_media_poll": {
                "method": "GET",
                "path": "/v1/media/jobs/<job-id>"
            },
            "three_dgs_submit": {
                "method": "POST",
                "path": "/v1/3dgs/<provider>/jobs",
                "request_profile": "pool_gateway_profile",
                "translated_by": "provider_gateway_template_translation(3dgs, provider_id, request)"
            },
            "three_dgs_poll": {
                "method": "GET",
                "path": "/v1/3dgs/.../jobs/<job-id>"
            }
        },
        "upstream_contract": {
            "submit": "Worker POSTs translated upstream.request_body to upstream submit URL.",
            "routing_order": ["request.upstream_endpoint", "--provider-upstream provider=url", "provider-specific endpoint env", "--upstream default"],
            "poll": "Worker polls upstream poll_url/status_url/result_url when returned, explicit upstream_poll_endpoint when provided, or the Pool-compatible poll path on upstream base URL.",
            "job_id_fields": ["job_id", "task_id", "id", "data.job_id", "data.task_id", "data.id"],
            "status_fields": ["status", "data.status"],
            "output_fields": ["outputs", "data.outputs", "assets", "data.assets", "files", "data.files", "images", "data.images", "audios", "data.audios", "output_url", "image_url", "audio_url", "video_url", "file_url", "download_url"],
            "auth": {
                "bearer_token_allowed": true,
                "routing_order": ["--provider-api-key provider=token", "--provider-api-key-env provider=ENV", "provider-specific api_key_env emitted by template", "--api-key or --api-key-env default"]
            }
        },
        "sdk_worker_template": {
            "purpose": "Runnable upstream SDK wrapper scaffold. It receives provider_gateway_worker upstream.request_body, validates local_input_manifest, writes an audit file, and returns Pool-compatible job/status/outputs. Replace template output generation with the real vendor SDK/API call.",
            "example": "pool-cli provider-sdk-worker-template --bind 127.0.0.1:8798 --output-root target/provider-sdk-worker-template",
            "self_check": "pool-cli provider-sdk-worker-template --once --output-root target/provider-sdk-worker-template",
            "forwarder_example": "cargo run -p pool-cli -- provider-gateway-worker --bind 127.0.0.1:8788 --upstream http://127.0.0.1:8798",
            "routes": {
                "health": {"method": "GET", "path": "/health"},
                "ai_media_submit": {"method": "POST", "path": "/v1/media/jobs"},
                "ai_media_poll": {"method": "GET", "path": "/v1/media/jobs/<job-id>"},
                "three_dgs_submit": {"method": "POST", "path": "/v1/3dgs/<provider>/jobs"},
                "three_dgs_poll": {"method": "GET", "path": "/v1/3dgs/.../jobs/<job-id>"},
                "output_download": {"method": "GET", "path": "/outputs/<job-id>/<file>"}
            },
            "input_contract": {
                "required": ["upstream.request_body.inputs.local_input_manifest"],
                "local_input_manifest_fields": ["path", "absolute_path", "file_name", "extension", "mime_type", "bytes", "exists"],
                "remote_input_urls_allowed": false,
                "large_file_bytes_in_json": false
            },
            "audit": {
                "request_file": "1-sdk-worker-request.json",
                "production_evidence_allowed": false,
                "note": "Template output is a scaffold only; production evidence still requires real vendor worker/SDK attestation and real local artifacts."
            }
        },
        "pool_adapter_usage": {
            "ai_media": {
                "provider_ids": ["midjourney", "nano-banana-pro", "suno"],
                "endpoint_env": "POOL_MEDIA_GATEWAY_ENDPOINT",
                "smoke": "POOL_MEDIA_GATEWAY_ENDPOINT=http://127.0.0.1:8788 cargo run -p pool-core --example generic_media_smoke -- nano-banana-pro request.json target/generic-media-worker-smoke"
            },
            "three_dgs": {
                "provider_ids": ["worldlabs-marble", "tripo-splat", "sam-3d", "spark-3dgs", "qunhe-3d"],
                "endpoint_env": "POOL_3DGS_GATEWAY_ENDPOINT",
                "smoke": "POOL_3DGS_GATEWAY_ENDPOINT=http://127.0.0.1:8788 cargo run -p pool-core --example three_dgs_gateway_smoke -- request.json target/three-dgs-worker-smoke worldlabs-marble"
            }
        },
        "conformance_runbook": {
            "purpose": "Prove that a real AI media or 3DGS upstream worker satisfies Pool's gateway boundary before it is used as production evidence.",
            "phases": [
                {
                    "id": "local_mock_baseline",
                    "command": "pool-cli provider-gateway-worker --once",
                    "proves": ["worker routes", "template translation", "Pool-compatible submit/poll normalization"],
                    "production_evidence": false
                },
                {
                    "id": "real_upstream_worker",
                    "command": "pool-cli provider-gateway-worker --bind 127.0.0.1:8788 --upstream http://127.0.0.1:8798 --api-key-env POOL_VENDOR_API_KEY",
                    "proves": ["real upstream reachability", "server-side bearer forwarding", "provider URL provenance boundary"],
                    "production_evidence": false
                },
                {
                    "id": "ai_media_smoke",
                    "command": "POOL_MEDIA_GATEWAY_ENDPOINT=http://127.0.0.1:8788 cargo run -p pool-core --example generic_media_smoke -- nano-banana-pro request.json target/generic-media-worker-smoke",
                    "proves": ["submit", "poll", "download to local media asset", "metadata file"],
                    "production_evidence": false
                },
                {
                    "id": "three_dgs_smoke",
                    "command": "POOL_3DGS_GATEWAY_ENDPOINT=http://127.0.0.1:8788 cargo run -p pool-core --example three_dgs_gateway_smoke -- request.json target/three-dgs-worker-smoke worldlabs-marble",
                    "proves": ["submit", "poll", "download to indexed local 3DGS assets", "hidden request metadata"],
                    "production_evidence": false
                },
                {
                    "id": "production_matrix",
                    "command": "POOL_PROVIDER_PRODUCTION_ATTESTATION=<real-worker-run-id> pool-cli --project demo production-evidence-provider-matrix target/provider-evidence-matrix --production-upstream --media-endpoint=http://127.0.0.1:8788 --3dgs-endpoint=http://127.0.0.1:8788 --evidence-bundle=target/provider-evidence-matrix/provider-production-evidence-bundle.json",
                    "proves": ["required Provider coverage", "non-mock upstream attestation", "local artifact existence", "production metadata paths"],
                    "production_evidence": true
                },
                {
                    "id": "validate_and_import",
                    "command": "pool-cli --project demo validate-production-evidence target/provider-evidence-matrix/provider-production-evidence-bundle.json && pool-cli --project demo import-production-evidence target/provider-evidence-matrix/provider-production-evidence-bundle.json",
                    "proves": ["template ids rejected before import", "remote URLs remain provenance only", "PRD readiness can consume imported evidence"],
                    "production_evidence": true
                }
            ],
            "pass_conditions": [
                "Every upstream response contains a job id or equivalent id field.",
                "Poll returns a completed/succeeded status before Pool downloads outputs.",
                "All outputs are downloadable by Pool into local files; remote URLs are not used as frontend truth sources.",
                "local_input_manifest is consumed by the real upstream worker when input media is required.",
                "Production evidence uses a real non-placeholder attestation and local artifact paths."
            ],
            "failure_conditions": [
                "Upstream only returns a remote URL that Pool cannot download.",
                "Provider evidence is produced by provider_gateway_mock_server or provider_sdk_worker_template without a real upstream run.",
                "API keys are echoed into response metadata or production evidence bundles.",
                "Input references are remote URLs instead of local paths or local_input_manifest entries."
            ]
        },
        "policy": {
            "local_files_authoritative": true,
            "provider_urls_are_provenance": true,
            "secrets_stay_server_side": true,
            "human_approval_required_for_high_cost_provider_runs": true,
            "control_priority": "API/MCP > Skills/CLI > Desktop Recognition > Human Takeover"
        },
        "mcp": {
            "resource": "pool://provider-gateway-worker",
            "http_path": "/api/mcp?uri=pool://provider-gateway-worker"
        }
    })
}

#[derive(Debug, Clone)]
struct WorkerJobRecord {
    family: ProviderGatewayTemplateFamily,
    upstream_poll_url: String,
    api_key: Option<String>,
}

pub struct ProviderGatewayWorker {
    options: ProviderGatewayWorkerOptions,
    client: Client,
    jobs: HashMap<String, WorkerJobRecord>,
}

impl ProviderGatewayWorker {
    pub fn new(options: ProviderGatewayWorkerOptions) -> Self {
        Self {
            options,
            client: Client::new(),
            jobs: HashMap::new(),
        }
    }

    pub fn serve_listener(&mut self, listener: TcpListener, max_requests: usize) -> Result<usize> {
        let mut handled = 0_usize;
        for stream in listener.incoming() {
            let stream = stream.context("accept provider gateway worker connection")?;
            if let Err(error) = self.handle_tcp_connection(stream) {
                eprintln!("provider gateway worker connection error: {error}");
            }
            handled += 1;
            if max_requests > 0 && handled >= max_requests {
                break;
            }
        }
        Ok(handled)
    }

    pub fn self_check(&mut self) -> Result<Vec<(String, u16, usize)>> {
        let mut results = Vec::new();

        let health = self.handle("GET", "/health", "")?;
        results.push((
            "GET /health".to_string(),
            health.status_code,
            health.body.len(),
        ));

        let media_body = json!({
            "provider_id": "nano-banana-pro",
            "prompt": "make hero",
            "output_slug": "nano",
            "output_extension": "png",
            "pool_media_profile": {
                "profile_id": "nano-banana-pro",
                "provider_id": "nano-banana-pro",
                "output_slug": "nano",
                "output_extension": "png"
            },
            "provider_payload": {
                "service": "nano-banana-pro",
                "provider_id": "nano-banana-pro",
                "mode": "reference_guided_image",
                "prompt": "make hero",
                "outputs": {
                    "slug": "nano",
                    "extension": "png"
                }
            }
        });
        let media_submit = self.handle("POST", "/v1/media/jobs", &media_body.to_string())?;
        let media_job_id = job_id_from_response(&media_submit.body)?;
        results.push((
            "POST /v1/media/jobs".to_string(),
            media_submit.status_code,
            media_submit.body.len(),
        ));
        let media_poll_path = format!("/v1/media/jobs/{media_job_id}");
        let media_poll = self.handle("GET", &media_poll_path, "")?;
        results.push((
            format!("GET {media_poll_path}"),
            media_poll.status_code,
            media_poll.body.len(),
        ));

        let three_dgs_body = json!({
            "provider_id": "tripo-splat",
            "prompt": "make object",
            "output_slug": "object",
            "pool_gateway_profile": {
                "profile_id": "tripo-splat",
                "provider_id": "tripo-splat",
                "output_slug": "object"
            },
            "provider_payload": {
                "service": "tripo-splat",
                "provider_id": "tripo-splat",
                "mode": "image_to_splat_object",
                "prompt": "make object",
                "output_slug": "object"
            }
        });
        let three_dgs_submit = self.handle(
            "POST",
            "/v1/3dgs/triposplat/jobs",
            &three_dgs_body.to_string(),
        )?;
        let three_dgs_job_id = job_id_from_response(&three_dgs_submit.body)?;
        results.push((
            "POST /v1/3dgs/triposplat/jobs".to_string(),
            three_dgs_submit.status_code,
            three_dgs_submit.body.len(),
        ));
        let three_dgs_poll_path = format!("/v1/3dgs/triposplat/jobs/{three_dgs_job_id}");
        let three_dgs_poll = self.handle("GET", &three_dgs_poll_path, "")?;
        results.push((
            format!("GET {three_dgs_poll_path}"),
            three_dgs_poll.status_code,
            three_dgs_poll.body.len(),
        ));

        Ok(results)
    }

    pub fn handle_tcp_connection(&mut self, mut stream: TcpStream) -> Result<()> {
        let request = read_http_request(&mut stream)?;
        let response = self
            .handle(&request.method, &request.path, &request.body)
            .unwrap_or_else(ProviderGatewayWorkerResponse::from_error);
        stream
            .write_all(&response.to_http_bytes())
            .context("write provider gateway worker HTTP response")
    }

    pub fn handle(
        &mut self,
        method: &str,
        raw_path: &str,
        body: &str,
    ) -> Result<ProviderGatewayWorkerResponse> {
        let path = raw_path.split('?').next().unwrap_or(raw_path);
        if method == "OPTIONS" {
            return Ok(ProviderGatewayWorkerResponse::empty(204));
        }
        if method == "GET" && matches!(path, "/" | "/health" | "/v1/health") {
            return ProviderGatewayWorkerResponse::json(
                200,
                json!({
                    "status": "ready",
                    "service": "pool-provider-gateway-worker",
                    "mode": "http_forwarder",
                    "base_url": self.options.base_url,
                    "has_default_upstream": self.options.default_upstream_endpoint.is_some(),
                    "local_files_authoritative": true,
                    "provider_urls_are_provenance": true,
                }),
            );
        }
        if method == "POST" && path == "/v1/media/jobs" {
            return self.submit(
                ProviderGatewayTemplateFamily::AiMedia,
                "generic-media",
                body,
            );
        }
        if method == "POST" && is_3dgs_submit_path(path) {
            let provider_id = three_dgs_submit_provider_id(path).unwrap_or("worldlabs-marble");
            return self.submit(ProviderGatewayTemplateFamily::ThreeDgs, provider_id, body);
        }
        if method == "GET" {
            if let Some(job_id) = path.strip_prefix("/v1/media/jobs/") {
                return self.poll(job_id);
            }
            if let Some(job_id) = three_dgs_job_id(path) {
                return self.poll(job_id);
            }
        }

        ProviderGatewayWorkerResponse::json(
            404,
            json!({
                "error": "not_found",
                "method": method,
                "path": path,
            }),
        )
    }

    fn submit(
        &mut self,
        family: ProviderGatewayTemplateFamily,
        default_provider_id: &str,
        body: &str,
    ) -> Result<ProviderGatewayWorkerResponse> {
        let request = parse_json_body(body)?;
        let provider_id = request
            .get("provider_id")
            .and_then(Value::as_str)
            .unwrap_or(default_provider_id);
        let translation = provider_gateway_template_translation(family, provider_id, &request)?;
        let upstream_endpoint =
            self.resolve_upstream_endpoint(provider_id, &request, &translation)?;
        let api_key = self.resolve_api_key(provider_id, &translation);
        let submit_url = submit_url_from_endpoint(&upstream_endpoint, &translation);
        let upstream_body = translation
            .pointer("/upstream/request_body")
            .cloned()
            .unwrap_or_else(|| json!({}));
        let upstream_response = self.post_json(&submit_url, &upstream_body, api_key.as_deref())?;
        let upstream_job_id = extract_job_id(&upstream_response)
            .unwrap_or_else(|| format!("gateway-worker-{}", self.jobs.len() + 1));
        let pool_job_id = upstream_job_id.clone();
        let upstream_poll_url = self.resolve_poll_url(
            &request,
            &translation,
            &upstream_endpoint,
            &submit_url,
            &upstream_response,
            &upstream_job_id,
        );
        let normalized = normalize_gateway_response(
            family,
            &pool_job_id,
            &upstream_response,
            Some(&translation),
        );
        self.jobs.insert(
            pool_job_id.clone(),
            WorkerJobRecord {
                family,
                upstream_poll_url,
                api_key,
            },
        );
        ProviderGatewayWorkerResponse::json(200, normalized)
    }

    fn poll(&mut self, pool_job_id: &str) -> Result<ProviderGatewayWorkerResponse> {
        let Some(job) = self.jobs.get(pool_job_id).cloned() else {
            return ProviderGatewayWorkerResponse::json(
                404,
                json!({"error": "unknown_job", "job_id": pool_job_id}),
            );
        };
        let upstream_response = self.get_json(&job.upstream_poll_url, job.api_key.as_deref())?;
        let normalized =
            normalize_gateway_response(job.family, pool_job_id, &upstream_response, None);
        ProviderGatewayWorkerResponse::json(200, normalized)
    }

    fn resolve_upstream_endpoint(
        &self,
        provider_id: &str,
        request: &Value,
        translation: &Value,
    ) -> Result<String> {
        if let Some(endpoint) = request.get("upstream_endpoint").and_then(Value::as_str) {
            return Ok(endpoint.to_string());
        }
        if let Some(endpoint) = self
            .options
            .provider_upstream_endpoints
            .get(&provider_route_key(provider_id))
        {
            return Ok(endpoint.clone());
        }
        if let Some(endpoint) = resolve_env_endpoint(translation, "/upstream/endpoint_env") {
            return Ok(endpoint);
        }
        if let Some(endpoint) = &self.options.default_upstream_endpoint {
            return Ok(endpoint.clone());
        }
        bail!(
            "upstream_not_configured: set --upstream, upstream_endpoint, or one of upstream.endpoint_env before forwarding provider gateway jobs"
        )
    }

    fn resolve_api_key(&self, provider_id: &str, translation: &Value) -> Option<String> {
        self.options
            .provider_api_keys
            .get(&provider_route_key(provider_id))
            .cloned()
            .or_else(|| resolve_env_endpoint(translation, "/upstream/api_key_env"))
            .or_else(|| self.options.api_key.clone())
    }

    fn resolve_poll_url(
        &self,
        request: &Value,
        translation: &Value,
        upstream_endpoint: &str,
        submit_url: &str,
        upstream_response: &Value,
        upstream_job_id: &str,
    ) -> String {
        if let Some(url) = ["poll_url", "status_url", "result_url"]
            .iter()
            .find_map(|key| upstream_response.get(*key).and_then(Value::as_str))
        {
            return url.to_string();
        }
        if let Some(url) = ["poll_url", "status_url", "result_url"]
            .iter()
            .find_map(|key| {
                upstream_response
                    .pointer(&format!("/data/{key}"))
                    .and_then(Value::as_str)
            })
        {
            return url.to_string();
        }
        if let Some(endpoint) = request
            .get("upstream_poll_endpoint")
            .and_then(Value::as_str)
        {
            return endpoint.replace("{job_id}", upstream_job_id);
        }
        if let Some(endpoint) = resolve_env_endpoint(translation, "/upstream/poll_endpoint_env") {
            return endpoint.replace("{job_id}", upstream_job_id);
        }
        if upstream_endpoint_is_base(upstream_endpoint) {
            let path = translation
                .pointer("/pool_submit/poll_path_template")
                .and_then(Value::as_str)
                .unwrap_or("/v1/media/jobs/{job_id}")
                .replace("{job_id}", upstream_job_id);
            return join_url(upstream_endpoint, &path);
        }
        format!("{}/{}", submit_url.trim_end_matches('/'), upstream_job_id)
    }

    fn post_json(&self, url: &str, body: &Value, api_key: Option<&str>) -> Result<Value> {
        let client = self.client.clone();
        let api_key = api_key.map(ToString::to_string);
        let url = url.to_string();
        let body = body.clone();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .context("create provider gateway worker tokio runtime")?;
        runtime.block_on(async move {
            let mut builder = client.post(&url).json(&body);
            if let Some(api_key) = api_key {
                builder = builder.bearer_auth(api_key);
            }
            builder
                .send()
                .await
                .with_context(|| format!("submit upstream provider gateway job {url}"))?
                .error_for_status()
                .with_context(|| format!("upstream submit returned error for {url}"))?
                .json()
                .await
                .with_context(|| format!("decode upstream submit response {url}"))
        })
    }

    fn get_json(&self, url: &str, api_key: Option<&str>) -> Result<Value> {
        let client = self.client.clone();
        let api_key = api_key.map(ToString::to_string);
        let url = url.to_string();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .context("create provider gateway worker tokio runtime")?;
        runtime.block_on(async move {
            let mut builder = client.get(&url);
            if let Some(api_key) = api_key {
                builder = builder.bearer_auth(api_key);
            }
            builder
                .send()
                .await
                .with_context(|| format!("poll upstream provider gateway job {url}"))?
                .error_for_status()
                .with_context(|| format!("upstream poll returned error for {url}"))?
                .json()
                .await
                .with_context(|| format!("decode upstream poll response {url}"))
        })
    }
}

pub fn spawn_provider_gateway_worker(
    default_upstream_endpoint: impl Into<String>,
    max_requests: usize,
) -> Result<String> {
    let listener = TcpListener::bind("127.0.0.1:0").context("bind provider gateway worker")?;
    let addr = listener
        .local_addr()
        .context("read provider gateway worker addr")?;
    let base_url = format!("http://{addr}");
    let options = ProviderGatewayWorkerOptions::new(base_url.clone())
        .with_default_upstream_endpoint(default_upstream_endpoint);
    thread::spawn(move || {
        let mut worker = ProviderGatewayWorker::new(options);
        if let Err(error) = worker.serve_listener(listener, max_requests) {
            eprintln!("provider gateway worker server error: {error}");
        }
    });
    Ok(base_url)
}

#[derive(Debug, Clone)]
pub struct ProviderGatewayWorkerResponse {
    pub status_code: u16,
    pub content_type: String,
    pub body: Vec<u8>,
}

impl ProviderGatewayWorkerResponse {
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
                "error": "provider_gateway_worker_error",
                "message": error.to_string(),
            }),
        )
        .unwrap_or_else(|_| Self {
            status_code: 500,
            content_type: "application/json; charset=utf-8".to_string(),
            body: br#"{"error":"provider_gateway_worker_error"}"#.to_vec(),
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
struct GatewayWorkerHttpRequest {
    method: String,
    path: String,
    body: String,
}

fn normalize_gateway_response(
    family: ProviderGatewayTemplateFamily,
    pool_job_id: &str,
    upstream_response: &Value,
    translation: Option<&Value>,
) -> Value {
    let outputs = collect_outputs(upstream_response);
    let status = if outputs.is_empty() {
        normalize_status(upstream_response)
    } else {
        "completed".to_string()
    };
    let mut response = json!({
        "job_id": pool_job_id,
        "status": status,
        "outputs": outputs,
        "pool_gateway_worker": {
            "family": family.as_str(),
            "upstream_job_id": extract_job_id(upstream_response),
        }
    });
    if let Some(translation) = translation {
        response["pool_gateway_worker"]["translation"] = translation.clone();
    }
    response
}

fn job_id_from_response(body: &[u8]) -> Result<String> {
    let value: Value =
        serde_json::from_slice(body).context("decode provider gateway self-check")?;
    value
        .get("job_id")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .context("provider gateway self-check response missing job_id")
}

fn collect_outputs(response: &Value) -> Vec<Value> {
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
            if item.as_str().is_some() {
                outputs.push(json!({"url": item.as_str().unwrap()}));
            } else if item.is_object() {
                outputs.push(item.clone());
            }
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
            outputs.push(json!({"url": url}));
        }
        if let Some(url) = response
            .pointer(&format!("/data/{key}"))
            .and_then(Value::as_str)
        {
            outputs.push(json!({"url": url}));
        }
    }
    outputs
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

fn normalize_status(response: &Value) -> String {
    let status = response
        .get("status")
        .or_else(|| response.pointer("/data/status"))
        .and_then(Value::as_str)
        .unwrap_or("running")
        .to_ascii_lowercase();
    match status.as_str() {
        "completed" | "complete" | "succeeded" | "success" => "completed",
        "failed" | "error" | "canceled" | "cancelled" => "failed",
        "queued" | "pending" => "queued",
        _ => "running",
    }
    .to_string()
}

fn resolve_env_endpoint(translation: &Value, pointer: &str) -> Option<String> {
    translation
        .pointer(pointer)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .find_map(|key| std::env::var(key).ok())
}

fn submit_url_from_endpoint(endpoint: &str, translation: &Value) -> String {
    if upstream_endpoint_is_base(endpoint) {
        let path = translation
            .pointer("/pool_submit/path")
            .and_then(Value::as_str)
            .unwrap_or("/v1/media/jobs");
        join_url(endpoint, path)
    } else {
        endpoint.to_string()
    }
}

fn upstream_endpoint_is_base(endpoint: &str) -> bool {
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

fn parse_json_body(body: &str) -> Result<Value> {
    if body.trim().is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_str(body).context("decode provider gateway worker JSON request")
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

fn three_dgs_submit_provider_id(path: &str) -> Option<&str> {
    let rest = path.strip_prefix("/v1/3dgs/")?;
    let (provider, suffix) = rest.split_once('/')?;
    if suffix == "jobs" && !provider.trim().is_empty() {
        Some(provider)
    } else {
        None
    }
}

fn provider_route_key(provider_id: &str) -> String {
    match provider_id
        .trim()
        .to_ascii_lowercase()
        .replace(['_', ' '], "-")
        .as_str()
    {
        "nano-banana" | "nanobanana" | "nanobananapro" | "nano-banana-pro" => "nano-banana-pro",
        "tripo" | "triposplat" | "tripo-splat" => "tripo-splat",
        "sam3d" | "sam-3d" => "sam-3d",
        "spark" | "spark-3d" | "spark-3dgs" => "spark-3dgs",
        "qunhe" | "qunhe-3d" | "qunhe-tech" => "qunhe-3d",
        "worldlabs" | "world-labs" | "worldlabs-marble" | "world-labs-marble" | "marble" => {
            "worldlabs-marble"
        }
        "midjourney" => "midjourney",
        "suno" => "suno",
        other => other,
    }
    .to_string()
}

fn read_http_request(stream: &mut impl Read) -> Result<GatewayWorkerHttpRequest> {
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

    Ok(GatewayWorkerHttpRequest {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::gateway_mock::spawn_provider_gateway_mock;
    use std::io::Write;
    use std::sync::mpsc;

    #[test]
    fn contract_exposes_cli_and_pool_adapter_usage() {
        let contract = provider_gateway_worker_contract();

        assert_eq!(contract["kind"], "pool_provider_gateway_worker_contract");
        assert_eq!(contract["service"], "pool-provider-gateway-worker");
        assert!(contract["cli"]["primary"]
            .as_str()
            .unwrap()
            .contains("pool-cli provider-gateway-worker"));
        assert_eq!(
            contract["pool_adapter_usage"]["three_dgs"]["endpoint_env"],
            "POOL_3DGS_GATEWAY_ENDPOINT"
        );
        assert_eq!(
            contract["sdk_worker_template"]["routes"]["output_download"]["path"],
            "/outputs/<job-id>/<file>"
        );
        assert_eq!(
            contract["sdk_worker_template"]["input_contract"]["remote_input_urls_allowed"],
            false
        );
        assert!(contract["conformance_runbook"]["phases"]
            .as_array()
            .unwrap()
            .iter()
            .any(|phase| phase["id"] == "three_dgs_smoke"
                && phase["command"]
                    .as_str()
                    .unwrap_or_default()
                    .contains("three_dgs_gateway_smoke")));
        assert!(contract["conformance_runbook"]["phases"]
            .as_array()
            .unwrap()
            .iter()
            .any(|phase| phase["id"] == "production_matrix"
                && phase["production_evidence"] == true
                && phase["command"]
                    .as_str()
                    .unwrap_or_default()
                    .contains("production-evidence-provider-matrix")));
        assert_eq!(
            contract["mcp"]["resource"],
            "pool://provider-gateway-worker"
        );
    }

    #[test]
    fn worker_forwards_media_submit_and_poll() {
        let upstream = spawn_provider_gateway_mock(4).unwrap();
        let mut worker = ProviderGatewayWorker::new(
            ProviderGatewayWorkerOptions::new("http://127.0.0.1:0")
                .with_default_upstream_endpoint(upstream),
        );
        let request = json!({
            "provider_id": "nano-banana-pro",
            "prompt": "make hero",
            "output_slug": "nano",
            "output_extension": "png",
            "pool_media_profile": {
                "profile_id": "nano-banana-pro",
                "provider_id": "nano-banana-pro",
                "output_slug": "nano",
                "output_extension": "png"
            },
            "provider_payload": {
                "service": "nano-banana-pro",
                "provider_id": "nano-banana-pro",
                "mode": "reference_guided_image",
                "prompt": "make hero",
                "output_slug": "nano",
                "output_extension": "png"
            }
        });

        let submit = worker
            .handle("POST", "/v1/media/jobs", &request.to_string())
            .unwrap();
        let submit_value: Value = serde_json::from_slice(&submit.body).unwrap();
        let job_id = submit_value["job_id"].as_str().unwrap();
        assert_eq!(submit_value["status"], "queued");

        let poll = worker
            .handle("GET", &format!("/v1/media/jobs/{job_id}"), "")
            .unwrap();
        let poll_value: Value = serde_json::from_slice(&poll.body).unwrap();
        assert_eq!(poll_value["status"], "completed");
        assert_eq!(poll_value["outputs"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn worker_self_check_covers_media_and_3dgs_forwarding() {
        let upstream = spawn_provider_gateway_mock(8).unwrap();
        let mut worker = ProviderGatewayWorker::new(
            ProviderGatewayWorkerOptions::new("http://127.0.0.1:0")
                .with_default_upstream_endpoint(upstream),
        );

        let results = worker.self_check().unwrap();

        assert_eq!(results.len(), 5);
        assert_eq!(results[0].0, "GET /health");
        assert_eq!(results[0].1, 200);
        assert_eq!(results[1].0, "POST /v1/media/jobs");
        assert_eq!(results[1].1, 200);
        assert!(results[2].0.starts_with("GET /v1/media/jobs/"));
        assert_eq!(results[2].1, 200);
        assert_eq!(results[3].0, "POST /v1/3dgs/triposplat/jobs");
        assert_eq!(results[3].1, 200);
        assert!(results[4].0.starts_with("GET /v1/3dgs/triposplat/jobs/"));
        assert_eq!(results[4].1, 200);
    }

    #[test]
    fn worker_forwards_local_input_manifest_to_upstream_body() {
        let (upstream, receiver) = spawn_capturing_upstream();
        let mut worker = ProviderGatewayWorker::new(
            ProviderGatewayWorkerOptions::new("http://127.0.0.1:0")
                .with_default_upstream_endpoint(upstream),
        );
        let request = json!({
            "provider_id": "nano-banana-pro",
            "prompt": "make hero",
            "output_slug": "nano",
            "output_extension": "png",
            "local_input_manifest": [{
                "path": "worlds/demo/source/ref.png",
                "absolute_path": "/tmp/pool/ref.png",
                "mime_type": "image/png",
                "bytes": 9,
                "exists": true
            }],
            "pool_media_profile": {
                "profile_id": "nano-banana-pro",
                "provider_id": "nano-banana-pro",
                "output_slug": "nano",
                "output_extension": "png"
            },
            "provider_payload": {
                "service": "nano-banana-pro",
                "mode": "reference_guided_image",
                "prompt": "make hero",
                "outputs": {
                    "slug": "nano",
                    "extension": "png"
                }
            }
        });

        let submit = worker
            .handle("POST", "/v1/media/jobs", &request.to_string())
            .unwrap();
        let submit_value: Value = serde_json::from_slice(&submit.body).unwrap();
        let captured = receiver.recv().unwrap();

        assert_eq!(submit_value["status"], "queued");
        assert_eq!(captured.path, "/v1/media/jobs");
        assert_eq!(
            captured.body["inputs"]["local_input_manifest"][0]["path"],
            "worlds/demo/source/ref.png"
        );
        assert_eq!(
            captured.body["local_input_manifest"][0]["mime_type"],
            "image/png"
        );
    }

    #[test]
    fn worker_uses_request_specific_upstream_endpoint() {
        let upstream = spawn_provider_gateway_mock(4).unwrap();
        let mut worker =
            ProviderGatewayWorker::new(ProviderGatewayWorkerOptions::new("http://127.0.0.1:0"));
        let request = json!({
            "provider_id": "nano-banana-pro",
            "upstream_endpoint": upstream,
            "prompt": "make hero",
            "output_slug": "nano",
            "output_extension": "png",
            "pool_media_profile": {
                "profile_id": "nano-banana-pro",
                "provider_id": "nano-banana-pro",
                "output_slug": "nano",
                "output_extension": "png"
            },
            "provider_payload": {
                "service": "nano-banana-pro",
                "mode": "reference_guided_image",
                "prompt": "make hero",
                "outputs": {
                    "slug": "nano",
                    "extension": "png"
                }
            }
        });

        let submit = worker
            .handle("POST", "/v1/media/jobs", &request.to_string())
            .unwrap();
        let submit_value: Value = serde_json::from_slice(&submit.body).unwrap();
        let job_id = submit_value["job_id"].as_str().unwrap();
        assert_eq!(job_id, "media-nano-banana-pro-0001");

        let poll = worker
            .handle("GET", &format!("/v1/media/jobs/{job_id}"), "")
            .unwrap();
        let poll_value: Value = serde_json::from_slice(&poll.body).unwrap();
        assert_eq!(poll_value["status"], "completed");
    }

    #[test]
    fn worker_uses_provider_specific_upstream_route() {
        let upstream = spawn_provider_gateway_mock(4).unwrap();
        let mut worker = ProviderGatewayWorker::new(
            ProviderGatewayWorkerOptions::new("http://127.0.0.1:0")
                .with_provider_upstream("triposplat", upstream),
        );
        let request = json!({
            "prompt": "make object",
            "output_slug": "object",
            "pool_gateway_profile": {
                "profile_id": "tripo-splat",
                "provider_id": "tripo-splat",
                "output_slug": "object"
            },
            "provider_payload": {
                "service": "tripo-splat",
                "mode": "image_to_splat_object",
                "prompt": "make object",
                "output_slug": "object"
            }
        });

        let submit = worker
            .handle("POST", "/v1/3dgs/triposplat/jobs", &request.to_string())
            .unwrap();
        let submit_value: Value = serde_json::from_slice(&submit.body).unwrap();
        let job_id = submit_value["job_id"].as_str().unwrap();
        assert_eq!(submit_value["status"], "queued");

        let poll = worker
            .handle("GET", &format!("/v1/3dgs/triposplat/jobs/{job_id}"), "")
            .unwrap();
        let poll_value: Value = serde_json::from_slice(&poll.body).unwrap();
        assert_eq!(poll_value["status"], "completed");
        assert_eq!(poll_value["outputs"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn worker_forwards_3dgs_submit_and_poll() {
        let upstream = spawn_provider_gateway_mock(4).unwrap();
        let mut worker = ProviderGatewayWorker::new(
            ProviderGatewayWorkerOptions::new("http://127.0.0.1:0")
                .with_default_upstream_endpoint(upstream),
        );
        let request = json!({
            "provider_id": "tripo-splat",
            "prompt": "make object",
            "output_slug": "object",
            "pool_gateway_profile": {
                "profile_id": "tripo-splat",
                "provider_id": "tripo-splat",
                "output_slug": "object"
            },
            "provider_payload": {
                "service": "tripo-splat",
                "provider_id": "tripo-splat",
                "mode": "image_to_splat_object",
                "prompt": "make object",
                "output_slug": "object"
            }
        });

        let submit = worker
            .handle("POST", "/v1/3dgs/triposplat/jobs", &request.to_string())
            .unwrap();
        let submit_value: Value = serde_json::from_slice(&submit.body).unwrap();
        let job_id = submit_value["job_id"].as_str().unwrap();
        assert_eq!(submit_value["status"], "queued");

        let poll = worker
            .handle("GET", &format!("/v1/3dgs/triposplat/jobs/{job_id}"), "")
            .unwrap();
        let poll_value: Value = serde_json::from_slice(&poll.body).unwrap();
        assert_eq!(poll_value["status"], "completed");
        assert_eq!(poll_value["outputs"].as_array().unwrap().len(), 3);
    }

    #[derive(Debug)]
    struct CapturedUpstreamRequest {
        path: String,
        body: Value,
    }

    fn spawn_capturing_upstream() -> (String, mpsc::Receiver<CapturedUpstreamRequest>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let request = read_http_request(&mut stream).unwrap();
            let body = parse_json_body(&request.body).unwrap();
            sender
                .send(CapturedUpstreamRequest {
                    path: request.path,
                    body,
                })
                .unwrap();
            let response_body = br#"{"job_id":"captured-upstream-job","status":"queued"}"#.to_vec();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                response_body.len()
            );
            stream.write_all(response.as_bytes()).unwrap();
            stream.write_all(&response_body).unwrap();
        });
        (format!("http://{addr}"), receiver)
    }
}
