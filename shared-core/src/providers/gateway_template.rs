use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderGatewayTemplateFamily {
    AiMedia,
    ThreeDgs,
}

impl ProviderGatewayTemplateFamily {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AiMedia => "ai_media",
            Self::ThreeDgs => "3dgs",
        }
    }

    pub fn profile_key(self) -> &'static str {
        match self {
            Self::AiMedia => "pool_media_profile",
            Self::ThreeDgs => "pool_gateway_profile",
        }
    }

    pub fn output_contract(self) -> &'static str {
        match self {
            Self::AiMedia => "local-media-files",
            Self::ThreeDgs => "image-blaster-indexed-files",
        }
    }

    pub fn submit_path(self, provider_id: &str) -> &'static str {
        match self {
            Self::AiMedia => "/v1/media/jobs",
            Self::ThreeDgs => three_dgs_submit_path(provider_id),
        }
    }

    pub fn poll_path(self, provider_id: &str) -> &'static str {
        match self {
            Self::AiMedia => "/v1/media/jobs/{job_id}",
            Self::ThreeDgs => three_dgs_poll_path(provider_id),
        }
    }
}

pub fn provider_gateway_template_translation(
    family: ProviderGatewayTemplateFamily,
    provider_id_hint: &str,
    request: &Value,
) -> Result<Value> {
    let object = request
        .as_object()
        .context("Pool gateway template request must be a JSON object")?;
    let profile = request.get(family.profile_key()).unwrap_or(&Value::Null);
    let provider_payload = request.get("provider_payload").unwrap_or(&Value::Null);
    let provider_id = string_at(request, "/provider_id")
        .or_else(|| string_at(profile, "/provider_id"))
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| provider_id_hint.to_string());
    if provider_id.trim().is_empty() {
        bail!("provider_id is required for gateway template translation");
    }
    let profile_id = string_at(profile, "/profile_id").unwrap_or_else(|| provider_id.clone());
    let prompt = object
        .get("prompt")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let output_slug = string_at(request, "/output_slug")
        .or_else(|| string_at(profile, "/output_slug"))
        .unwrap_or_else(|| default_output_slug(family, &provider_id).to_string());
    let output_extension = string_at(request, "/output_extension")
        .or_else(|| string_at(profile, "/output_extension"))
        .or_else(|| default_output_extension(family, &provider_id).map(ToString::to_string));
    let service = string_at(provider_payload, "/service").unwrap_or_else(|| provider_id.clone());
    let mode = string_at(provider_payload, "/mode")
        .unwrap_or_else(|| default_mode(family, &provider_id).to_string());
    let mut upstream_body = if provider_payload.is_null() {
        json!({
            "service": service,
            "mode": mode,
            "prompt": prompt,
            "inputs": {
                "paths": request.get("input_paths").cloned().unwrap_or_else(|| json!([])),
                "local_input_manifest": request.get("local_input_manifest").cloned().unwrap_or_else(|| json!([])),
            },
            "outputs": {
                "slug": output_slug,
                "extension": output_extension,
                "contract": family.output_contract(),
            }
        })
    } else {
        provider_payload.clone()
    };
    merge_local_input_manifest(&mut upstream_body, request.get("local_input_manifest"));

    Ok(json!({
        "kind": "pool_provider_gateway_template_translation",
        "family": family.as_str(),
        "provider_id": provider_id,
        "profile_id": profile_id,
        "task_type": string_at(profile, "/task_type"),
        "pipeline": string_at(profile, "/pipeline"),
        "pool_submit": {
            "path": family.submit_path(&provider_id),
            "poll_path_template": family.poll_path(&provider_id),
            "body": request,
        },
        "upstream": {
            "service": service,
            "mode": mode,
            "endpoint_env": upstream_endpoint_env_candidates(&provider_id),
            "api_key_env": upstream_api_key_env_candidates(&provider_id),
            "request_body": upstream_body,
            "auth": {
                "strategy": "gateway_env_or_secret_store",
                "bearer_token_allowed": true,
            }
        },
        "sdk_worker_contract": {
            "step_1": "Read upstream.request_body and call the vendor SDK or HTTP API.",
            "step_2": "Normalize the vendor job id into job_id/task_id/id.",
            "step_3": "Normalize final outputs into Pool gateway outputs[].",
            "step_4": "Return provider URLs only as provenance; Pool runtime must download local files.",
        },
        "expected_pool_response": expected_pool_response_contract(family, &output_slug, output_extension.as_deref()),
        "local_output_policy": {
            "local_files_authoritative": true,
            "provider_urls_are_provenance": true,
            "output_contract": family.output_contract(),
        }
    }))
}

fn merge_local_input_manifest(upstream_body: &mut Value, manifest: Option<&Value>) {
    let Some(manifest) = manifest else {
        return;
    };
    if manifest
        .as_array()
        .is_some_and(|entries| entries.is_empty())
    {
        return;
    }
    let Some(body) = upstream_body.as_object_mut() else {
        return;
    };
    body.insert("local_input_manifest".to_string(), manifest.clone());
    let inputs = body
        .entry("inputs".to_string())
        .or_insert_with(|| json!({}));
    if let Some(inputs) = inputs.as_object_mut() {
        inputs.insert("local_input_manifest".to_string(), manifest.clone());
    }
}

pub fn provider_gateway_template_contract() -> Value {
    json!({
        "kind": "pool_provider_gateway_template_contract",
        "purpose": "Translate Pool gateway profile requests into vendor SDK or HTTP worker requests.",
        "families": {
            "ai_media": {
                "submit_path": "/v1/media/jobs",
                "poll_path_template": "/v1/media/jobs/{job_id}",
                "providers": ["midjourney", "nano-banana-pro", "suno"],
                "profile_key": "pool_media_profile",
                "output_contract": "local-media-files",
            },
            "3dgs": {
                "providers": ["worldlabs-marble", "tripo-splat", "sam-3d", "spark-3dgs", "qunhe-3d"],
                "profile_key": "pool_gateway_profile",
                "output_contract": "image-blaster-indexed-files",
            }
        },
        "translation_output": {
            "upstream.endpoint_env": "environment variable candidates for the local gateway or vendor SDK worker",
            "upstream.api_key_env": "environment variable candidates for vendor credentials",
            "upstream.request_body": "provider_payload if supplied, otherwise a normalized vendor worker body",
            "expected_pool_response": "Pool-compatible response shape that the gateway must return",
        },
        "local_output_policy": {
            "local_files_authoritative": true,
            "provider_urls_are_provenance": true,
        }
    })
}

pub fn sample_provider_gateway_template_request(
    family: ProviderGatewayTemplateFamily,
    provider_id: &str,
) -> Value {
    let output_slug = default_output_slug(family, provider_id);
    match family {
        ProviderGatewayTemplateFamily::AiMedia => json!({
            "project_slug": "demo",
            "provider_id": provider_id,
            "prompt": "Generate a Pool production reference asset",
            "input_paths": ["worlds/demo/source/0-reference.png"],
            "output_slug": output_slug,
            "output_extension": default_output_extension(family, provider_id),
            "pool_media_profile": {
                "profile_id": provider_id,
                "provider_id": provider_id,
                "modality": if provider_id == "suno" { "audio" } else { "image" },
                "pipeline": default_pipeline(family, provider_id),
                "task_type": default_task_type(family, provider_id),
                "output_contract": family.output_contract(),
                "output_slug": output_slug,
                "output_extension": default_output_extension(family, provider_id),
            },
            "provider_payload": {
                "service": provider_id,
                "mode": default_mode(family, provider_id),
                "prompt": "Generate a Pool production reference asset",
                "outputs": {
                    "slug": output_slug,
                    "extension": default_output_extension(family, provider_id),
                    "contract": family.output_contract(),
                }
            }
        }),
        ProviderGatewayTemplateFamily::ThreeDgs => json!({
            "project_slug": "demo",
            "provider_id": provider_id,
            "prompt": "Convert Pool references into a 3DGS asset",
            "input_paths": ["worlds/demo/source/0-reference.png"],
            "output_slug": output_slug,
            "pool_gateway_profile": {
                "profile_id": provider_id,
                "provider_id": provider_id,
                "pipeline": default_pipeline(family, provider_id),
                "task_type": default_task_type(family, provider_id),
                "asset_scope": if output_slug == "object" { "object" } else { "scene" },
                "output_contract": family.output_contract(),
                "output_slug": output_slug,
            },
            "provider_payload": {
                "service": provider_id,
                "mode": default_mode(family, provider_id),
                "prompt": "Convert Pool references into a 3DGS asset",
                "outputs": {
                    "slug": output_slug,
                    "contract": family.output_contract(),
                    "preferred_formats": ["json", "glb", "spz"],
                }
            }
        }),
    }
}

fn expected_pool_response_contract(
    family: ProviderGatewayTemplateFamily,
    output_slug: &str,
    output_extension: Option<&str>,
) -> Value {
    match family {
        ProviderGatewayTemplateFamily::AiMedia => json!({
            "submit": {
                "job_id": "vendor_job_id",
                "status": "queued"
            },
            "poll_completed": {
                "job_id": "vendor_job_id",
                "status": "completed",
                "outputs": [
                    {
                        "name": format!("{output_slug}.{}", output_extension.unwrap_or("bin")),
                        "url": "https://provider-or-gateway.example/output",
                        "extension": output_extension,
                    }
                ]
            }
        }),
        ProviderGatewayTemplateFamily::ThreeDgs => json!({
            "submit": {
                "job_id": "vendor_job_id",
                "status": "queued"
            },
            "poll_completed": {
                "job_id": "vendor_job_id",
                "status": "completed",
                "outputs": [
                    {"name": format!("{output_slug}.json"), "url": "https://provider-or-gateway.example/metadata.json"},
                    {"name": format!("{output_slug}.glb"), "url": "https://provider-or-gateway.example/model.glb"},
                    {"name": format!("{output_slug}-full_res.spz"), "url": "https://provider-or-gateway.example/full_res.spz"}
                ]
            }
        }),
    }
}

fn upstream_endpoint_env_candidates(provider_id: &str) -> Vec<String> {
    let prefix = provider_env_prefix(provider_id);
    vec![
        format!("POOL_GATEWAY_{prefix}_UPSTREAM_ENDPOINT"),
        format!("POOL_{prefix}_UPSTREAM_ENDPOINT"),
        "POOL_PROVIDER_GATEWAY_UPSTREAM_ENDPOINT".to_string(),
    ]
}

fn upstream_api_key_env_candidates(provider_id: &str) -> Vec<String> {
    let prefix = provider_env_prefix(provider_id);
    vec![
        format!("POOL_GATEWAY_{prefix}_API_KEY"),
        format!("POOL_{prefix}_API_KEY"),
        "POOL_PROVIDER_GATEWAY_API_KEY".to_string(),
    ]
}

fn string_at(value: &Value, pointer: &str) -> Option<String> {
    value.pointer(pointer)?.as_str().map(ToString::to_string)
}

fn provider_env_prefix(provider_id: &str) -> String {
    provider_id
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect()
}

fn default_output_slug(family: ProviderGatewayTemplateFamily, provider_id: &str) -> &'static str {
    match family {
        ProviderGatewayTemplateFamily::AiMedia => match provider_id {
            "midjourney" | "mj" => "midjourney",
            "nano-banana-pro" | "nano-banana" | "nanobanana" | "nanobananapro" => "nano",
            "suno" => "suno-cue",
            _ => "media",
        },
        ProviderGatewayTemplateFamily::ThreeDgs => match provider_id {
            "tripo-splat" | "triposplat" | "sam-3d" | "sam3d" => "object",
            "spark-3dgs" | "spark" | "qunhe-3d" | "qunhe" => "scene",
            _ => "world",
        },
    }
}

fn default_output_extension(
    family: ProviderGatewayTemplateFamily,
    provider_id: &str,
) -> Option<&'static str> {
    match family {
        ProviderGatewayTemplateFamily::AiMedia => match provider_id {
            "suno" => Some("mp3"),
            "generic-media" => Some("bin"),
            _ => Some("png"),
        },
        ProviderGatewayTemplateFamily::ThreeDgs => None,
    }
}

fn default_pipeline(family: ProviderGatewayTemplateFamily, provider_id: &str) -> &'static str {
    match family {
        ProviderGatewayTemplateFamily::AiMedia => match provider_id {
            "midjourney" | "mj" => "prompt_to_image",
            "nano-banana-pro" | "nano-banana" | "nanobanana" | "nanobananapro" => {
                "reference_guided_image_generation"
            }
            "suno" => "prompt_to_music",
            _ => "generic_media_generation",
        },
        ProviderGatewayTemplateFamily::ThreeDgs => match provider_id {
            "tripo-splat" | "triposplat" => "image_to_object_splat",
            "sam-3d" | "sam3d" => "segment_then_reconstruct_object",
            "spark-3dgs" | "spark" => "multi_view_scene_reconstruction",
            "qunhe-3d" | "qunhe" => "space_scene_reconstruction",
            _ => "image_or_text_to_world",
        },
    }
}

fn default_task_type(family: ProviderGatewayTemplateFamily, provider_id: &str) -> &'static str {
    match family {
        ProviderGatewayTemplateFamily::AiMedia => match provider_id {
            "midjourney" | "mj" => "midjourney_imagine",
            "nano-banana-pro" | "nano-banana" | "nanobanana" | "nanobananapro" => {
                "nano_banana_pro_image"
            }
            "suno" => "suno_music_generation",
            _ => "generic_media_job",
        },
        ProviderGatewayTemplateFamily::ThreeDgs => match provider_id {
            "tripo-splat" | "triposplat" => "tripo_splat_reconstruction",
            "sam-3d" | "sam3d" => "sam_3d_object_reconstruction",
            "spark-3dgs" | "spark" => "spark_3dgs_scene_reconstruction",
            "qunhe-3d" | "qunhe" => "qunhe_scene_package",
            _ => "marble_world_generation",
        },
    }
}

fn default_mode(family: ProviderGatewayTemplateFamily, provider_id: &str) -> &'static str {
    match family {
        ProviderGatewayTemplateFamily::AiMedia => match provider_id {
            "midjourney" | "mj" => "imagine",
            "nano-banana-pro" | "nano-banana" | "nanobanana" | "nanobananapro" => {
                "reference_guided_image"
            }
            "suno" => "music_or_cue",
            _ => "generic_media_job",
        },
        ProviderGatewayTemplateFamily::ThreeDgs => match provider_id {
            "tripo-splat" | "triposplat" => "image_to_splat_object",
            "sam-3d" | "sam3d" => "segment_then_reconstruct",
            "spark-3dgs" | "spark" => "scene_3dgs_reconstruction",
            "qunhe-3d" | "qunhe" => "space_scene_package",
            _ => "marble_world",
        },
    }
}

fn three_dgs_submit_path(provider_id: &str) -> &'static str {
    match provider_id {
        "tripo-splat" | "triposplat" => "/v1/3dgs/triposplat/jobs",
        "sam-3d" | "sam3d" => "/v1/3dgs/sam-3d/jobs",
        "spark-3dgs" | "spark" => "/v1/3dgs/spark/jobs",
        "qunhe-3d" | "qunhe" => "/v1/3dgs/qunhe/jobs",
        _ => "/v1/3dgs/jobs",
    }
}

fn three_dgs_poll_path(provider_id: &str) -> &'static str {
    match provider_id {
        "tripo-splat" | "triposplat" => "/v1/3dgs/triposplat/jobs/{job_id}",
        "sam-3d" | "sam3d" => "/v1/3dgs/sam-3d/jobs/{job_id}",
        "spark-3dgs" | "spark" => "/v1/3dgs/spark/jobs/{job_id}",
        "qunhe-3d" | "qunhe" => "/v1/3dgs/qunhe/jobs/{job_id}",
        _ => "/v1/3dgs/jobs/{job_id}",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translates_nano_banana_media_request_to_upstream_template() {
        let request = sample_provider_gateway_template_request(
            ProviderGatewayTemplateFamily::AiMedia,
            "nano-banana-pro",
        );

        let translated = provider_gateway_template_translation(
            ProviderGatewayTemplateFamily::AiMedia,
            "nano-banana-pro",
            &request,
        )
        .unwrap();

        assert_eq!(translated["family"], "ai_media");
        assert_eq!(translated["upstream"]["mode"], "reference_guided_image");
        assert_eq!(
            translated["upstream"]["endpoint_env"][0],
            "POOL_GATEWAY_NANO_BANANA_PRO_UPSTREAM_ENDPOINT"
        );
        assert_eq!(
            translated["expected_pool_response"]["poll_completed"]["outputs"][0]["name"],
            "nano.png"
        );
    }

    #[test]
    fn translates_triposplat_request_to_indexed_3dgs_template() {
        let request = sample_provider_gateway_template_request(
            ProviderGatewayTemplateFamily::ThreeDgs,
            "tripo-splat",
        );

        let translated = provider_gateway_template_translation(
            ProviderGatewayTemplateFamily::ThreeDgs,
            "tripo-splat",
            &request,
        )
        .unwrap();

        assert_eq!(translated["family"], "3dgs");
        assert_eq!(
            translated["pool_submit"]["path"],
            "/v1/3dgs/triposplat/jobs"
        );
        assert_eq!(translated["upstream"]["mode"], "image_to_splat_object");
        assert_eq!(
            translated["expected_pool_response"]["poll_completed"]["outputs"][2]["name"],
            "object-full_res.spz"
        );
    }

    #[test]
    fn preserves_custom_provider_payload_as_upstream_body() {
        let request = json!({
            "provider_id": "suno",
            "prompt": "make a cue",
            "local_input_manifest": [{
                "path": "worlds/demo/source/cue.wav",
                "mime_type": "audio/wav",
                "exists": true
            }],
            "pool_media_profile": {
                "profile_id": "suno",
                "provider_id": "suno",
                "output_slug": "cue",
                "output_extension": "mp3"
            },
            "provider_payload": {
                "service": "suno",
                "mode": "custom-song",
                "vendor": {"duration": 12}
            }
        });

        let translated = provider_gateway_template_translation(
            ProviderGatewayTemplateFamily::AiMedia,
            "suno",
            &request,
        )
        .unwrap();

        assert_eq!(
            translated["upstream"]["request_body"]["vendor"]["duration"],
            12
        );
        assert_eq!(
            translated["upstream"]["request_body"]["inputs"]["local_input_manifest"][0]["path"],
            "worlds/demo/source/cue.wav"
        );
        assert_eq!(
            translated["upstream"]["request_body"]["local_input_manifest"][0]["mime_type"],
            "audio/wav"
        );
        assert_eq!(
            translated["expected_pool_response"]["poll_completed"]["outputs"][0]["name"],
            "cue.mp3"
        );
    }
}
