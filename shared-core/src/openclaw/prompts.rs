use anyhow::{bail, Context, Result};
use serde_json::{json, Map, Value};

pub fn pool_mcp_prompt_definitions() -> Vec<Value> {
    vec![
        pool_mcp_prompt(
            "pool_content_burst_runbook",
            "Pool Content Burst Runbook",
            "Plan and execute a local content-burst workflow from creative inputs to video, game, and interactive-art outputs.",
            &[
                ("project_slug", "Pool project slug to inspect and mutate", false),
                ("workflow_id", "Optional workflow id for scoped context", false),
                ("creative_brief", "Short creative brief or prompt", false),
                ("source_inputs", "Comma-separated local source files", false),
                ("output_targets", "Comma-separated targets: video, game, interactive_art", false),
            ],
        ),
        pool_mcp_prompt(
            "pool_3dgs_conversion_review",
            "Pool 3DGS Conversion Review",
            "Inspect 2D/3DGS conversion readiness, approval gates, provider requests, and local output contracts.",
            &[
                ("project_slug", "Pool project slug", false),
                ("workflow_id", "Optional workflow id for scoped context", false),
                ("node_id", "Optional 3DGS workflow node id", false),
                ("provider_id", "3DGS provider id such as worldlabs-marble or triposplat", false),
            ],
        ),
        pool_mcp_prompt(
            "pool_software_handoff",
            "Pool Software Handoff",
            "Prepare a safe external software control action through API/MCP, Skills/CLI, desktop recognition, or human takeover.",
            &[
                ("project_slug", "Pool project slug", false),
                ("adapter_id", "Software adapter id such as unreal, blender, resolve, touchdesigner, hermes", true),
                ("action_kind", "Action kind such as CreateScene, ExecuteCli, RunViewport, Render", false),
            ],
        ),
        pool_mcp_prompt(
            "pool_desktop_takeover",
            "Pool Desktop Takeover",
            "Guide a desktop recognition controller through request pickup, execution evidence, and result callback.",
            &[
                ("project_slug", "Pool project slug", false),
                ("software_action_id", "Optional software action id to complete", false),
                ("target_window", "Expected desktop application window", false),
            ],
        ),
    ]
}

pub fn pool_mcp_prompt_get_result(params: Value) -> Result<Value> {
    let params = prompt_arguments_object(params)?;
    let name = prompt_required_string_arg(&params, "name")?;
    let prompt_args = params
        .get("arguments")
        .cloned()
        .map(prompt_arguments_object)
        .transpose()?
        .unwrap_or_default();

    let (description, text) = match name.as_str() {
        "pool_content_burst_runbook" => (
            "Pool content-burst execution runbook",
            content_burst_prompt(&prompt_args),
        ),
        "pool_3dgs_conversion_review" => (
            "Pool 3DGS conversion review prompt",
            three_dgs_conversion_prompt(&prompt_args),
        ),
        "pool_software_handoff" => (
            "Pool external software control handoff prompt",
            software_handoff_prompt(&prompt_args)?,
        ),
        "pool_desktop_takeover" => (
            "Pool desktop recognition takeover prompt",
            desktop_takeover_prompt(&prompt_args),
        ),
        _ => bail!("unknown Pool MCP prompt: {name}"),
    };

    Ok(json!({
        "description": description,
        "messages": [
            {
                "role": "user",
                "content": {
                    "type": "text",
                    "text": text,
                }
            }
        ]
    }))
}

pub fn pool_mcp_prompt_http_path(name: &str) -> String {
    format!("/api/prompts?name={}", percent_encode_query_value(name))
}

fn pool_mcp_prompt(
    name: &str,
    title: &str,
    description: &str,
    args: &[(&str, &str, bool)],
) -> Value {
    json!({
        "name": name,
        "title": title,
        "description": description,
        "arguments": args
            .iter()
            .map(|(name, description, required)| {
                json!({
                    "name": name,
                    "description": description,
                    "required": required,
                })
            })
            .collect::<Vec<_>>(),
    })
}

fn content_burst_prompt(args: &Map<String, Value>) -> String {
    let project_slug = prompt_arg(args, "project_slug", "demo");
    let workflow_id = prompt_arg(args, "workflow_id", "<workflow-id>");
    let creative_brief = prompt_arg(args, "creative_brief", "Use current Pool project inputs");
    let source_inputs = prompt_arg(args, "source_inputs", "worlds/demo/source/0-reference.png");
    let output_targets = prompt_arg(args, "output_targets", "video, game, interactive_art");

    format!(
        "You are controlling Pool as a local-first content production OS.\n\
         Project: {project_slug}\n\
         Creative brief: {creative_brief}\n\
         Source inputs: {source_inputs}\n\
         Output targets: {output_targets}\n\n\
         Required sequence:\n\
         1. Read pool://status, pool://adapters, pool://runtime-graph, pool://workflow/{workflow_id}, and pool://tasks before mutating runtime when workflow_id is known.\n\
         2. Inspect node context for Agent, 3DGS, Unreal, and output nodes; prefer node control_context commands/tools when present.\n\
         3. Prefer pool_run_workflow for the full chain when the user wants an end-to-end burst.\n\
         4. Use pool_run_provider only for explicit AI/3DGS provider tasks and preserve local output paths.\n\
         5. Respect waiting approval gates; use pool_approve_task only after explicit user confirmation.\n\
         6. Summarize generated local files, provider requests, software actions, and next handoff."
    )
}

fn three_dgs_conversion_prompt(args: &Map<String, Value>) -> String {
    let project_slug = prompt_arg(args, "project_slug", "demo");
    let workflow_id = prompt_arg(args, "workflow_id", "<workflow-id>");
    let node_id = prompt_arg(args, "node_id", "<3dgs-node-id>");
    let provider_id = prompt_arg(args, "provider_id", "worldlabs-marble");

    format!(
        "Review Pool 2D/3DGS conversion readiness.\n\
         Project: {project_slug}\n\
         Node: {node_id}\n\
         Preferred provider: {provider_id}\n\n\
         Required checks:\n\
         1. Read pool://adapters, pool://runtime-graph, pool://workflow/{workflow_id}, and pool://node-context/{node_id} when workflow_id is known.\n\
         2. Read pool://provider-requests and pool://assets to verify local-first provenance.\n\
         3. If running conversion, prefer node control_context pool_run_provider arguments, then adjust execution_mode mock/gateway/auto as requested.\n\
         4. High-cost 3DGS work must remain waiting_approval unless the user explicitly approves.\n\
         5. Provider URLs are provenance only; report local files as the loading source of truth."
    )
}

fn software_handoff_prompt(args: &Map<String, Value>) -> Result<String> {
    let adapter_id = prompt_required_string_arg(args, "adapter_id")?;
    let project_slug = prompt_arg(args, "project_slug", "demo");
    let action_kind = prompt_arg(args, "action_kind", "HealthCheck");

    Ok(format!(
        "Prepare a Pool external software handoff.\n\
         Project: {project_slug}\n\
         Adapter: {adapter_id}\n\
         Action kind: {action_kind}\n\n\
         Required sequence:\n\
         1. Read pool://adapters and the relevant pool://node-context/<node-id> when a node is known.\n\
         2. Call pool_software_health for {adapter_id} before creating actions.\n\
         3. Use control priority API/MCP > Skills/CLI > Desktop Recognition > Human Takeover.\n\
         4. If using pool_run_software, prefer node control_context arguments, include a clear payload_json, and set requires_confirmation for risky actions.\n\
         5. For ExecuteCli actions, commands must be non-shell allowlisted commands.\n\
         6. For DesktopRecognition, read pool_desktop_requests and require controller evidence before pool_desktop_result.\n\
         7. Report task id, software_action id, artifacts, and verification message."
    ))
}

fn desktop_takeover_prompt(args: &Map<String, Value>) -> String {
    let project_slug = prompt_arg(args, "project_slug", "demo");
    let software_action_id = prompt_arg(args, "software_action_id", "<software-action-id>");
    let target_window = prompt_arg(args, "target_window", "<target-window>");

    format!(
        "Operate Pool desktop recognition handoff.\n\
         Project: {project_slug}\n\
         Software action: {software_action_id}\n\
         Target window: {target_window}\n\n\
         Required sequence:\n\
         1. Call pool_desktop_requests and inspect pool_desktop_action plus desktop_payload.\n\
         2. If visual_targets need screen/OCR resolution, write a Pool-compatible trace JSON with labels and center/bounds.\n\
         3. Do not claim execution unless a controller or human operator performed it.\n\
         4. Capture result evidence such as screen_trace_path and artifacts.\n\
         5. Call pool_desktop_result with status succeeded, failed, retryable, cancelled, or running.\n\
         6. Read pool://desktop-recognition and pool://software-actions after callback to verify ledger state."
    )
}

fn prompt_arguments_object(arguments: Value) -> Result<Map<String, Value>> {
    match arguments {
        Value::Null => Ok(Map::new()),
        Value::Object(object) => Ok(object),
        _ => bail!("MCP prompt arguments must be a JSON object"),
    }
}

fn prompt_required_string_arg(args: &Map<String, Value>, key: &str) -> Result<String> {
    args.get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string)
        .with_context(|| format!("MCP prompt argument {key} is required"))
}

fn prompt_arg(args: &Map<String, Value>, key: &str, fallback: &str) -> String {
    args.get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(fallback)
        .to_string()
}

fn percent_encode_query_value(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_pool_mcp_prompts() {
        let names = pool_mcp_prompt_definitions()
            .into_iter()
            .filter_map(|prompt| {
                prompt
                    .get("name")
                    .and_then(Value::as_str)
                    .map(ToString::to_string)
            })
            .collect::<Vec<_>>();

        assert!(names.contains(&"pool_content_burst_runbook".to_string()));
        assert!(names.contains(&"pool_3dgs_conversion_review".to_string()));
        assert!(names.contains(&"pool_software_handoff".to_string()));
        assert!(names.contains(&"pool_desktop_takeover".to_string()));
    }

    #[test]
    fn builds_software_handoff_prompt() {
        let result = pool_mcp_prompt_get_result(json!({
            "name": "pool_software_handoff",
            "arguments": {
                "project_slug": "demo",
                "adapter_id": "blender",
                "action_kind": "ExecuteCli"
            }
        }))
        .unwrap();

        assert_eq!(
            result["description"],
            "Pool external software control handoff prompt"
        );
        let text = result["messages"][0]["content"]["text"]
            .as_str()
            .unwrap_or_default();
        assert!(text.contains("Adapter: blender"));
        assert!(text.contains("pool://adapters"));
        assert!(text.contains("control_context"));
        assert!(text.contains("pool_software_health"));
        assert!(text.contains("pool_run_software"));
    }

    #[test]
    fn content_burst_prompt_includes_workflow_context_when_available() {
        let result = pool_mcp_prompt_get_result(json!({
            "name": "pool_content_burst_runbook",
            "arguments": {
                "project_slug": "demo",
                "workflow_id": "workflow-demo"
            }
        }))
        .unwrap();
        let text = result["messages"][0]["content"]["text"]
            .as_str()
            .unwrap_or_default();

        assert!(text.contains("pool://workflow/workflow-demo"));
        assert!(text.contains("pool://adapters"));
        assert!(text.contains("control_context"));
    }

    #[test]
    fn requires_handoff_adapter_id() {
        let error = pool_mcp_prompt_get_result(json!({
            "name": "pool_software_handoff",
            "arguments": {
                "project_slug": "demo"
            }
        }))
        .unwrap_err();

        assert!(error.to_string().contains("adapter_id"));
    }

    #[test]
    fn builds_prompt_http_path() {
        assert_eq!(
            pool_mcp_prompt_http_path("pool_software_handoff"),
            "/api/prompts?name=pool_software_handoff"
        );
    }
}
