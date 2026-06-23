use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;

mod comfyui;
mod gateway_mock;
mod gateway_template;
mod gateway_worker;
mod http_media;
mod kling;
mod local_inputs;
mod openai_image;
mod sdk_worker_template;
mod three_dgs_gateway;

use crate::models::{ProviderConfig, ProviderKind, TaskStatus};

pub use comfyui::{
    parse_progress_message, ComfyUiProgressEvent, ComfyUiProvider, ComfyUiProviderOptions,
};
pub use gateway_mock::{
    spawn_provider_gateway_mock, ProviderGatewayMock, ProviderGatewayMockResponse,
};
pub use gateway_template::{
    provider_gateway_template_contract, provider_gateway_template_translation,
    sample_provider_gateway_template_request, ProviderGatewayTemplateFamily,
};
pub use gateway_worker::{
    provider_gateway_worker_contract, spawn_provider_gateway_worker, ProviderGatewayWorker,
    ProviderGatewayWorkerOptions, ProviderGatewayWorkerResponse,
};
pub use http_media::{
    generic_http_media_contract, GenericHttpMediaOptions, GenericHttpMediaProvider,
    GenericHttpMediaRequest,
};
pub use kling::{KlingAuth, KlingProvider, KlingProviderOptions, KlingVideoRequest};
pub use openai_image::{OpenAiImageProvider, OpenAiImageProviderOptions, OpenAiImageRequest};
pub use sdk_worker_template::ProviderSdkWorkerTemplate;
pub use three_dgs_gateway::{
    three_dgs_gateway_contract, ThreeDgsGatewayOptions, ThreeDgsGatewayProvider,
    ThreeDgsGatewayRequest,
};

#[derive(Debug, Clone, Default)]
pub struct ProviderRegistry {
    configs: BTreeMap<String, ProviderConfig>,
}

impl ProviderRegistry {
    pub fn new(configs: impl IntoIterator<Item = ProviderConfig>) -> Self {
        Self {
            configs: configs
                .into_iter()
                .map(|config| (config.id.clone(), config))
                .collect(),
        }
    }

    pub fn defaults() -> Self {
        Self::new(default_provider_configs())
    }

    pub fn get(&self, id: &str) -> Option<&ProviderConfig> {
        self.configs.get(id)
    }

    pub fn by_kind(&self, kind: ProviderKind) -> Vec<&ProviderConfig> {
        self.configs
            .values()
            .filter(|config| config.kind == kind)
            .collect()
    }

    pub fn ids(&self) -> Vec<&str> {
        self.configs.keys().map(String::as_str).collect()
    }

    pub fn configs(&self) -> Vec<&ProviderConfig> {
        self.configs.values().collect()
    }
}

pub fn default_provider_configs() -> Vec<ProviderConfig> {
    vec![
        ProviderConfig {
            id: "comfyui".to_string(),
            display_name: "ComfyUI".to_string(),
            kind: ProviderKind::AiImage,
            endpoint: "http://127.0.0.1:8188".to_string(),
            auth_env_key: None,
            output_contract: "local image/video files plus workflow metadata".to_string(),
            high_cost: false,
        },
        ProviderConfig {
            id: "kling".to_string(),
            display_name: "Kling".to_string(),
            kind: ProviderKind::AiVideo,
            endpoint: "provider://kling".to_string(),
            auth_env_key: Some("POOL_KLING_API_KEY".to_string()),
            output_contract: "downloaded local video files".to_string(),
            high_cost: true,
        },
        ProviderConfig {
            id: "midjourney".to_string(),
            display_name: "Midjourney".to_string(),
            kind: ProviderKind::AiImage,
            endpoint: "provider://midjourney".to_string(),
            auth_env_key: Some("POOL_MIDJOURNEY_API_KEY".to_string()),
            output_contract: "downloaded local image files".to_string(),
            high_cost: false,
        },
        ProviderConfig {
            id: "openai-image-2".to_string(),
            display_name: "OpenAI image-2".to_string(),
            kind: ProviderKind::AiImage,
            endpoint: "https://api.openai.com/v1/images/generations".to_string(),
            auth_env_key: Some("OPENAI_API_KEY".to_string()),
            output_contract: "OpenAI Images API output saved as local image files".to_string(),
            high_cost: false,
        },
        ProviderConfig {
            id: "nano-banana-pro".to_string(),
            display_name: "Nano Banana Pro".to_string(),
            kind: ProviderKind::AiImage,
            endpoint: "provider://nano-banana-pro".to_string(),
            auth_env_key: Some("POOL_NANO_BANANA_PRO_KEY".to_string()),
            output_contract: "downloaded local image files".to_string(),
            high_cost: false,
        },
        ProviderConfig {
            id: "suno".to_string(),
            display_name: "Suno".to_string(),
            kind: ProviderKind::Audio,
            endpoint: "provider://suno".to_string(),
            auth_env_key: Some("POOL_SUNO_API_KEY".to_string()),
            output_contract: "downloaded local audio stems".to_string(),
            high_cost: false,
        },
        ProviderConfig {
            id: "worldlabs-marble".to_string(),
            display_name: "World Labs Marble".to_string(),
            kind: ProviderKind::ThreeDgs,
            endpoint: "provider://worldlabs-marble".to_string(),
            auth_env_key: Some("POOL_MARBLE_API_KEY".to_string()),
            output_contract: "image-blaster indexed local 3DGS package".to_string(),
            high_cost: true,
        },
        ProviderConfig {
            id: "tripo-splat".to_string(),
            display_name: "TripoSplat".to_string(),
            kind: ProviderKind::ThreeDgs,
            endpoint: "provider://tripo-splat".to_string(),
            auth_env_key: Some("POOL_TRIPOSPLAT_API_KEY".to_string()),
            output_contract: "image-blaster indexed local 3DGS package".to_string(),
            high_cost: true,
        },
        ProviderConfig {
            id: "sam-3d".to_string(),
            display_name: "SAM-3D".to_string(),
            kind: ProviderKind::ThreeDgs,
            endpoint: "provider://sam-3d".to_string(),
            auth_env_key: Some("POOL_SAM3D_API_KEY".to_string()),
            output_contract: "image-blaster indexed local 3D asset package".to_string(),
            high_cost: true,
        },
        ProviderConfig {
            id: "spark-3dgs".to_string(),
            display_name: "Spark 3DGS".to_string(),
            kind: ProviderKind::ThreeDgs,
            endpoint: "provider://spark-3dgs".to_string(),
            auth_env_key: Some("POOL_SPARK_3DGS_API_KEY".to_string()),
            output_contract: "image-blaster indexed local 3DGS package".to_string(),
            high_cost: true,
        },
        ProviderConfig {
            id: "qunhe-3d".to_string(),
            display_name: "Qunhe 3D".to_string(),
            kind: ProviderKind::ThreeDgs,
            endpoint: "provider://qunhe-3d".to_string(),
            auth_env_key: Some("POOL_QUNHE_API_KEY".to_string()),
            output_contract: "image-blaster indexed local 3D scene package".to_string(),
            high_cost: true,
        },
    ]
}

pub fn provider_contracts_resource(provider_id: Option<&str>) -> Result<Value> {
    if let Some(provider_id) = provider_id {
        let Some(contract) = provider_contract(provider_id) else {
            anyhow::bail!("unknown provider contract: {provider_id}");
        };
        return Ok(contract);
    }

    let contracts = ProviderRegistry::defaults()
        .configs()
        .into_iter()
        .filter_map(|config| provider_contract(&config.id))
        .collect::<Vec<_>>();

    Ok(json!({
        "kind": "pool_provider_contracts",
        "version": 1,
        "summary": {
            "providers": contracts.len(),
            "gateway_families": ["ai_media", "3dgs"],
            "native_adapters": ["comfyui", "kling", "openai-image-2"],
        },
        "policy": {
            "local_files_authoritative": true,
            "provider_urls_are_provenance": true,
            "high_cost_requires_approval": true,
            "secrets_stay_server_side": true,
        },
        "contracts": contracts,
    }))
}

pub fn provider_contract(provider_id: &str) -> Option<Value> {
    let provider_id = canonical_provider_contract_id(provider_id);
    let registry = ProviderRegistry::defaults();
    let config = registry.get(&provider_id)?;

    if matches!(
        provider_id.as_str(),
        "midjourney" | "nano-banana-pro" | "suno"
    ) {
        return Some(generic_http_media_contract(&provider_id));
    }

    if config.kind == ProviderKind::ThreeDgs {
        return Some(three_dgs_gateway_contract(&provider_id));
    }

    Some(native_provider_contract(config))
}

fn native_provider_contract(config: &ProviderConfig) -> Value {
    let supported_operations = match config.id.as_str() {
        "openai-image-2" => json!(["images.generate", "images.edit"]),
        "kling" => json!(["videos.text2video", "videos.image2video"]),
        _ => json!(["submit"]),
    };
    let input_contract = match config.id.as_str() {
        "openai-image-2" => json!({
            "generate": {
                "prompt": "plain text or JSON prompt without image fields",
                "endpoint_path": "/images/generations"
            },
            "edit": {
                "prompt": "JSON prompt with operation:\"edit\" or image/images/input_images/mask, or plain prompt plus ProviderRequest.input_paths",
                "endpoint_path": "/images/edits",
                "local_file_fields": ["image", "images", "input_images", "mask"],
                "provider_request_input_paths": "used as input_images when no prompt image fields are present and operation is not generate",
                "remote_input_urls_allowed": false
            }
        }),
        "kling" => json!({
            "text2video": {
                "prompt": "JSON prompt without image/image_url fields",
                "endpoint_path": "/v1/videos/text2video"
            },
            "image2video": {
                "prompt": "JSON prompt with image/image_url, or JSON prompt plus ProviderRequest.input_paths",
                "endpoint_path": "/v1/videos/image2video",
                "provider_request_input_paths": "first local path is encoded as image data URL for submit; metadata stores only local_input_paths",
                "remote_input_urls_allowed": "only when prompt explicitly supplies provider-specific image_url"
            }
        }),
        _ => json!({
            "prompt": "provider-specific prompt or JSON body",
            "input_paths": "local source files"
        }),
    };
    json!({
        "provider_id": &config.id,
        "display_name": &config.display_name,
        "adapter_kind": "native_pool_adapter",
        "gateway_family": Value::Null,
        "kind": &config.kind,
        "endpoint": &config.endpoint,
        "auth_env_key": &config.auth_env_key,
        "supported_operations": supported_operations,
        "input_contract": input_contract,
        "output_contract": &config.output_contract,
        "high_cost": config.high_cost,
        "runtime_provider_run": {
            "method": "POST",
            "path": "/api/provider-runs",
            "body": {
                "project_slug": "demo",
                "provider_id": &config.id,
                "execution_mode": "adapter",
                "prompt": format!("Run {} through the native Pool adapter", config.display_name),
                "input_paths": ["worlds/demo/source/0-reference.png"],
                "output_dir": "worlds/demo/output",
            }
        },
        "notes": [
            "This provider is implemented as a native Pool adapter, not the generic gateway contract.",
            "Provider credentials remain in the Runtime process; browser/UI code must not call provider APIs directly.",
        ],
        "local_output_policy": {
            "local_files_authoritative": true,
            "provider_urls_are_provenance": true,
        },
    })
}

fn canonical_provider_contract_id(provider_id: &str) -> String {
    match provider_id.trim() {
        "world-labs-marble" | "worldlabs" | "marble" => "worldlabs-marble".to_string(),
        "triposplat" | "tripo" => "tripo-splat".to_string(),
        "spark" => "spark-3dgs".to_string(),
        "qunhe" | "qunhe-tech" => "qunhe-3d".to_string(),
        "openai" | "openai-image" | "image-2" => "openai-image-2".to_string(),
        "mj" => "midjourney".to_string(),
        "nanobanana" | "nano-banana" | "nanobananapro" => "nano-banana-pro".to_string(),
        value => value.to_string(),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderHealth {
    pub provider_id: String,
    pub status: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderRequest {
    pub project_slug: String,
    pub prompt: String,
    pub input_paths: Vec<String>,
    pub output_dir: String,
    pub require_approval: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderJob {
    pub provider_id: String,
    pub external_job_id: Option<String>,
    pub status: TaskStatus,
    pub request_metadata_path: String,
    pub expected_outputs: Vec<String>,
    pub metadata_json: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderVerification {
    pub ok: bool,
    pub local_paths: Vec<String>,
    pub message: String,
}

#[async_trait]
pub trait ProviderAdapter: Send + Sync {
    fn config(&self) -> &ProviderConfig;
    async fn health(&self) -> Result<ProviderHealth>;
    async fn submit(&self, request: ProviderRequest) -> Result<ProviderJob>;
    async fn poll(&self, job: &ProviderJob) -> Result<TaskStatus>;
    async fn download(&self, job: &ProviderJob) -> Result<Vec<String>>;
    async fn verify(&self, job: &ProviderJob) -> Result<ProviderVerification>;

    fn estimate_cost_tokens(&self, request: &ProviderRequest) -> u64 {
        let base = if self.config().high_cost {
            9_000
        } else {
            1_500
        };
        base + (request.input_paths.len() as u64 * 500)
    }
}

pub struct Mock3dgsProvider {
    config: ProviderConfig,
}

impl Mock3dgsProvider {
    pub fn new(id: impl Into<String>, display_name: impl Into<String>) -> Self {
        Self {
            config: ProviderConfig {
                id: id.into(),
                display_name: display_name.into(),
                kind: ProviderKind::ThreeDgs,
                endpoint: "mock://3dgs".to_string(),
                auth_env_key: None,
                output_contract: "N-world.json, N-world.glb, N-world-full_res.spz".to_string(),
                high_cost: true,
            },
        }
    }
}

#[async_trait]
impl ProviderAdapter for Mock3dgsProvider {
    fn config(&self) -> &ProviderConfig {
        &self.config
    }

    async fn health(&self) -> Result<ProviderHealth> {
        Ok(ProviderHealth {
            provider_id: self.config.id.clone(),
            status: "ready".to_string(),
            message: "mock 3DGS provider ready".to_string(),
        })
    }

    async fn submit(&self, request: ProviderRequest) -> Result<ProviderJob> {
        let status = if request.require_approval {
            TaskStatus::WaitingApproval
        } else {
            TaskStatus::Queued
        };
        Ok(ProviderJob {
            provider_id: self.config.id.clone(),
            external_job_id: Some(format!("mock-{}", request.project_slug)),
            status,
            request_metadata_path: format!("{}/.1-world-request.json", request.output_dir),
            expected_outputs: vec![
                format!("{}/1-world.json", request.output_dir),
                format!("{}/1-world.glb", request.output_dir),
                format!("{}/1-world-full_res.spz", request.output_dir),
            ],
            metadata_json: None,
        })
    }

    async fn poll(&self, job: &ProviderJob) -> Result<TaskStatus> {
        Ok(match job.status {
            TaskStatus::Queued | TaskStatus::Ready | TaskStatus::Running => TaskStatus::Succeeded,
            ref status => status.clone(),
        })
    }

    async fn download(&self, job: &ProviderJob) -> Result<Vec<String>> {
        Ok(job.expected_outputs.clone())
    }

    async fn verify(&self, job: &ProviderJob) -> Result<ProviderVerification> {
        Ok(ProviderVerification {
            ok: true,
            local_paths: job.expected_outputs.clone(),
            message: "mock outputs satisfy image-blaster local asset contract".to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_registry_contains_ai_and_3dgs_providers() {
        let registry = ProviderRegistry::defaults();

        assert!(registry.get("comfyui").is_some());
        assert!(registry.get("kling").is_some());
        assert!(registry.get("worldlabs-marble").is_some());
        assert!(registry.get("tripo-splat").is_some());
        assert!(!registry.by_kind(ProviderKind::AiImage).is_empty());
        assert!(!registry.by_kind(ProviderKind::ThreeDgs).is_empty());
        assert!(registry
            .configs()
            .iter()
            .any(|config| config.id == "openai-image-2"));
    }

    #[test]
    fn provider_contracts_cover_gateway_and_native_adapters() {
        let contracts = provider_contracts_resource(None).unwrap();

        assert_eq!(contracts["kind"], "pool_provider_contracts");
        assert!(contracts["contracts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|contract| contract["provider_id"] == "openai-image-2"
                && contract["adapter_kind"] == "native_pool_adapter"
                && contract["supported_operations"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|operation| operation == "images.edit")));
        assert!(contracts["contracts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|contract| contract["provider_id"] == "kling"
                && contract["supported_operations"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|operation| operation == "videos.image2video")));
        assert!(contracts["contracts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|contract| contract["provider_id"] == "midjourney"
                && contract["adapter_kind"] == "generic_http_media_gateway"));
        assert!(contracts["contracts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|contract| contract["provider_id"] == "worldlabs-marble"
                && contract["adapter_kind"] == "three_dgs_http_gateway"));
    }

    #[test]
    fn provider_contract_resolves_alias_and_exposes_gateway_body() {
        let media = provider_contract("mj").unwrap();
        assert_eq!(media["provider_id"], "midjourney");
        assert_eq!(media["gateway_submit"]["path"], "/v1/media/jobs");
        assert_eq!(
            media["gateway_submit"]["body"]["pool_media_profile"]["profile_id"],
            "midjourney"
        );

        let three_dgs = provider_contract("triposplat").unwrap();
        assert_eq!(three_dgs["provider_id"], "tripo-splat");
        assert_eq!(
            three_dgs["gateway_submit"]["body"]["pool_gateway_profile"]["profile_id"],
            "triposplat"
        );
        assert_eq!(
            three_dgs["local_output_policy"]["output_contract"],
            "image-blaster-indexed-files"
        );

        assert!(provider_contract("missing-provider").is_none());
    }
}
