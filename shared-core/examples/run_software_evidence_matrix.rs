use anyhow::{Context, Result};
use pool_core::{
    build_default_content_burst_plan, materialize_project_envelope, runtime_prd_readiness_resource,
    RuntimeHttpConfig, RuntimeHttpServer, RuntimeRepository,
};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const TARGETS: &[&str] = &[
    "unreal",
    "blender",
    "comfyui",
    "resolve",
    "unity",
    "touchdesigner",
    "madmapper",
    "nuke",
    "motion-db",
    "editing-suite",
    "hermes",
];

fn main() -> Result<()> {
    let options = MatrixOptions::from_args(std::env::args().skip(1));
    std::fs::create_dir_all(&options.output_root)
        .with_context(|| format!("create output root {}", options.output_root.display()))?;

    let db_path = options.output_root.join("pool-runtime.sqlite");
    let repository = RuntimeRepository::open(&db_path)?;
    repository.migrate()?;
    if repository.stats()?.projects == 0 {
        let plan = build_default_content_burst_plan("demo", "Pool software evidence matrix");
        repository.persist_plan(&plan)?;
        materialize_project_envelope(&options.output_root, &plan)?;
    }
    drop(repository);

    let server = RuntimeHttpServer::new(
        RuntimeHttpConfig::new(&db_path)
            .with_project_slug("demo")
            .with_bind_addr("127.0.0.1:4788"),
    );

    let mut succeeded = 0_usize;
    let mut failed = 0_usize;
    let mut production_evidence_items = Vec::new();
    println!("db={}", db_path.display());
    println!("output_root={}", options.output_root.display());
    println!("production_software={}", options.production_software);

    for adapter_id in TARGETS {
        let evidence_mode = if options.production_software {
            "production_software"
        } else {
            "local_control_profile"
        };
        let body = software_action_body(adapter_id, evidence_mode, options.production_software);
        let response =
            server.handle_request_with_body("POST", "/api/software-actions", &body.to_string())?;
        let value: Value = serde_json::from_str(&response.body)
            .with_context(|| format!("parse software response for {adapter_id}"))?;
        let status = value
            .pointer("/report/status")
            .and_then(Value::as_str)
            .or_else(|| value.get("error").and_then(Value::as_str))
            .unwrap_or("unknown");
        let action_id = value
            .pointer("/report/action_id")
            .and_then(Value::as_str)
            .unwrap_or("none");

        let production_ready = software_request_claims_real_production(&body);
        if response.status_code < 400
            && status == "Succeeded"
            && (!options.production_software || production_ready)
        {
            succeeded += 1;
            if options.production_software && production_ready {
                production_evidence_items.push(software_production_evidence_item(
                    adapter_id, &body, &value,
                )?);
            }
        } else {
            failed += 1;
        }
        println!(
            "adapter={} http={} status={} software_action_id={} evidence_mode={}",
            adapter_id, response.status_code, status, action_id, evidence_mode
        );
    }

    let evidence_bundle_path = options.evidence_bundle_path();
    let evidence_bundle = json!({
        "source": "run_software_evidence_matrix",
        "project_slug": "demo",
        "providers": [],
        "software_actions": production_evidence_items,
        "desktop_vision": [],
    });
    write_json_file(&evidence_bundle_path, &evidence_bundle)?;
    println!(
        "software_production_evidence_bundle={} software_actions={}",
        evidence_bundle_path.display(),
        evidence_bundle
            .get("software_actions")
            .and_then(Value::as_array)
            .map_or(0, Vec::len)
    );

    let repository = RuntimeRepository::open(&db_path)?;
    repository.migrate()?;
    let snapshot = repository.snapshot(Some("demo"))?;
    let readiness = runtime_prd_readiness_resource(&snapshot)?;
    println!(
        "summary=succeeded:{succeeded},failed:{failed},software_actions:{}",
        snapshot.software_actions.len()
    );
    println!("prd_summary={}", readiness["summary"]);
    if let Some(software_requirement) = readiness
        .get("requirements")
        .and_then(Value::as_array)
        .and_then(|items| {
            items.iter().find(|item| {
                item.get("id").and_then(Value::as_str) == Some("external_software_control")
            })
        })
    {
        println!(
            "external_software_control_status={}",
            software_requirement["status"]
        );
        println!(
            "external_software_control_evidence={}",
            software_requirement["evidence"]["software_evidence"]
        );
    }

    Ok(())
}

#[derive(Debug)]
struct MatrixOptions {
    output_root: PathBuf,
    production_software: bool,
    evidence_bundle_path: Option<PathBuf>,
}

impl MatrixOptions {
    fn from_args(args: impl IntoIterator<Item = String>) -> Self {
        let mut output_root = PathBuf::from("target/software-evidence-matrix");
        let mut production_software = false;
        let mut evidence_bundle_path = std::env::var("POOL_SOFTWARE_EVIDENCE_BUNDLE")
            .ok()
            .map(PathBuf::from);

        for arg in args {
            if arg == "--production-software" {
                production_software = true;
            } else if let Some(value) = arg.strip_prefix("--evidence-bundle=") {
                evidence_bundle_path = Some(PathBuf::from(value));
            } else if !arg.trim().is_empty() {
                output_root = PathBuf::from(arg);
            }
        }

        Self {
            output_root,
            production_software,
            evidence_bundle_path,
        }
    }

    fn evidence_bundle_path(&self) -> PathBuf {
        self.evidence_bundle_path.clone().unwrap_or_else(|| {
            self.output_root
                .join("worlds/demo/output/control/production-evidence/software-production-evidence-bundle.json")
        })
    }
}

fn software_action_body(adapter_id: &str, evidence_mode: &str, production_software: bool) -> Value {
    software_action_body_with_env(adapter_id, evidence_mode, production_software, |name| {
        std::env::var(name).ok()
    })
}

fn software_action_body_with_env(
    adapter_id: &str,
    evidence_mode: &str,
    production_software: bool,
    env_lookup: impl Fn(&str) -> Option<String>,
) -> Value {
    let production_attestation = production_software_attestation(adapter_id, &env_lookup);
    if production_software && production_attestation.is_none() {
        return missing_production_software_body(
            adapter_id,
            evidence_mode,
            &production_software_attestation_missing_config_message(adapter_id),
        );
    }
    let production_attestation = production_attestation.as_deref();

    if adapter_id == "unreal" {
        if production_software {
            if let Some(endpoint) = software_endpoint_env(adapter_id, &env_lookup) {
                let artifacts = production_software_artifacts(adapter_id, &env_lookup);
                if artifacts.is_empty() {
                    return missing_production_software_body(
                        adapter_id,
                        evidence_mode,
                        &production_software_missing_config_message(
                            &software_endpoint_env_names(adapter_id),
                            adapter_id,
                        ),
                    );
                }
                return json!({
                    "project_slug": "demo",
                    "adapter_id": "unreal",
                    "action_kind": "CreateScene",
                    "priority": "ApiMcp",
                    "task_title": "unreal production software evidence",
                    "payload_json": {
                        "mcp_endpoint": endpoint,
                        "level": "demo_software_evidence",
                        "assets": ["worlds/demo/output/1-world.glb"],
                        "artifacts": artifacts
                    },
                    "requires_confirmation": false,
                    "evidence_json": evidence_json(adapter_id, "api_mcp", evidence_mode, true, false, true, production_attestation),
                });
            }
            return missing_production_software_body(
                adapter_id,
                evidence_mode,
                &production_software_missing_config_message(
                    &software_endpoint_env_names(adapter_id),
                    adapter_id,
                ),
            );
        }

        return json!({
            "project_slug": "demo",
            "adapter_id": "unreal",
            "action_kind": "CreateScene",
            "priority": "ApiMcp",
            "task_title": "unreal software evidence",
            "payload_json": {
                "level": "demo_software_evidence",
                "assets": ["worlds/demo/output/1-world.glb"]
            },
            "requires_confirmation": false,
            "evidence_json": evidence_json(adapter_id, "api_mcp", evidence_mode, false, true, false, None),
        });
    }

    if adapter_id == "hermes" && production_software {
        if let Some(endpoint) = software_endpoint_env(adapter_id, &env_lookup) {
            let artifacts = production_software_artifacts(adapter_id, &env_lookup);
            if artifacts.is_empty() {
                return missing_production_software_body(
                    adapter_id,
                    evidence_mode,
                    &production_software_missing_config_message(
                        &software_endpoint_env_names(adapter_id),
                        adapter_id,
                    ),
                );
            }
            return json!({
                "project_slug": "demo",
                "adapter_id": "hermes",
                "action_kind": "CreateScene",
                "priority": "ApiMcp",
                "task_title": "hermes production software evidence",
                "payload_json": {
                    "hermes_endpoint": endpoint,
                    "mcp_endpoint": endpoint,
                    "instruction": "Run Pool production software evidence orchestration through Hermes.",
                    "project_slug": "demo",
                    "artifacts": artifacts
                },
                "requires_confirmation": false,
                "evidence_json": evidence_json(adapter_id, "api_mcp", evidence_mode, true, false, true, production_attestation),
            });
        }
    }

    if production_software {
        if let Some(endpoint) = software_endpoint_env(adapter_id, &env_lookup) {
            let artifacts = production_software_artifacts(adapter_id, &env_lookup);
            if artifacts.is_empty() {
                return missing_production_software_body(
                    adapter_id,
                    evidence_mode,
                    &production_software_missing_config_message(
                        &software_endpoint_env_names(adapter_id),
                        adapter_id,
                    ),
                );
            }
            return endpoint_software_body(
                adapter_id,
                evidence_mode,
                endpoint,
                artifacts,
                production_attestation,
            );
        }
        if let Some(command) = production_software_command(adapter_id, &env_lookup) {
            let artifacts = production_software_artifacts(adapter_id, &env_lookup);
            if artifacts.is_empty() {
                return missing_production_software_body(
                    adapter_id,
                    evidence_mode,
                    &production_software_missing_config_message(
                        &production_software_command_env_names(adapter_id),
                        adapter_id,
                    ),
                );
            }
            return command_software_body(
                adapter_id,
                evidence_mode,
                true,
                false,
                true,
                command,
                artifacts,
                production_attestation,
            );
        }
        return missing_production_software_body(
            adapter_id,
            evidence_mode,
            &production_software_missing_config_message(
                &production_software_control_env_names(adapter_id),
                adapter_id,
            ),
        );
    }

    command_software_body(
        adapter_id,
        evidence_mode,
        false,
        false,
        false,
        format!("/bin/echo pool-software-evidence-{adapter_id}"),
        vec![format!("software-evidence://{adapter_id}/cli")],
        None,
    )
}

fn endpoint_software_body(
    adapter_id: &str,
    evidence_mode: &str,
    endpoint: String,
    artifacts: Vec<String>,
    production_attestation: Option<&str>,
) -> Value {
    json!({
        "project_slug": "demo",
        "adapter_id": adapter_id,
        "action_kind": "CreateScene",
        "priority": "ApiMcp",
        "task_title": format!("{adapter_id} production software evidence"),
        "payload_json": {
            "endpoint": endpoint.clone(),
            "mcp_endpoint": endpoint,
            "project_slug": "demo",
            "instruction": format!("Run Pool production software evidence through the {adapter_id} API/MCP adapter."),
            "artifacts": artifacts
        },
        "requires_confirmation": false,
        "evidence_json": evidence_json(adapter_id, "api_mcp", evidence_mode, true, false, true, production_attestation),
    })
}

fn command_software_body(
    adapter_id: &str,
    evidence_mode: &str,
    production_software: bool,
    local_mock_software: bool,
    configured_real_software: bool,
    command: String,
    artifacts: Vec<String>,
    production_attestation: Option<&str>,
) -> Value {
    let allowed_commands = allowed_commands_from_command(&command);
    json!({
        "project_slug": "demo",
        "adapter_id": adapter_id,
        "action_kind": "ExecuteCli",
        "priority": "SkillsCli",
        "task_title": format!("{adapter_id} software evidence"),
        "payload_json": {
            "command": command,
            "allowed_commands": allowed_commands,
            "timeout_ms": 2000,
            "max_output_bytes": 2048,
            "artifacts": artifacts
        },
        "requires_confirmation": false,
        "evidence_json": evidence_json(
            adapter_id,
            "skills_cli",
            evidence_mode,
            production_software,
            local_mock_software,
            configured_real_software,
            production_attestation
        ),
    })
}

fn missing_production_software_body(
    adapter_id: &str,
    evidence_mode: &str,
    expected_env: &str,
) -> Value {
    let command = "/usr/bin/false".to_string();
    let mut body = command_software_body(
        adapter_id,
        "production_software_missing_config",
        false,
        true,
        false,
        command,
        vec![format!("software-evidence://{adapter_id}/missing-config")],
        None,
    );
    if let Some(evidence) = body.get_mut("evidence_json").and_then(Value::as_object_mut) {
        evidence.insert(
            "missing_production_software_config".to_string(),
            json!(expected_env),
        );
        evidence.insert("requested_evidence_mode".to_string(), json!(evidence_mode));
    }
    body
}

fn evidence_json(
    adapter_id: &str,
    control_profile: &str,
    evidence_mode: &str,
    production_software: bool,
    local_mock_software: bool,
    configured_real_software: bool,
    production_attestation: Option<&str>,
) -> Value {
    let mut evidence = json!({
        "source": "run_software_evidence_matrix",
        "adapter_id": adapter_id,
        "control_profile": control_profile,
        "evidence_mode": evidence_mode,
        "production_software": production_software,
        "local_mock_software": local_mock_software,
        "configured_real_software": configured_real_software,
    });
    if let Some(production_attestation) = production_attestation {
        if let Some(object) = evidence.as_object_mut() {
            object.insert(
                "production_attestation".to_string(),
                json!(production_attestation),
            );
        }
    }
    evidence
}

fn software_endpoint_env(
    adapter_id: &str,
    env_lookup: &impl Fn(&str) -> Option<String>,
) -> Option<String> {
    software_endpoint_env_names(adapter_id)
        .into_iter()
        .find_map(|name| env_lookup(&name).filter(|value| !value.trim().is_empty()))
}

fn production_software_command(
    adapter_id: &str,
    env_lookup: &impl Fn(&str) -> Option<String>,
) -> Option<String> {
    production_software_command_env_names(adapter_id)
        .into_iter()
        .find_map(|name| env_lookup(&name).filter(|value| !value.trim().is_empty()))
}

fn production_software_artifacts(
    adapter_id: &str,
    env_lookup: &impl Fn(&str) -> Option<String>,
) -> Vec<String> {
    production_software_artifact_env_names(adapter_id)
        .into_iter()
        .find_map(|name| env_lookup(&name).filter(|value| !value.trim().is_empty()))
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty() && !value.contains("://"))
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn production_software_attestation(
    adapter_id: &str,
    env_lookup: &impl Fn(&str) -> Option<String>,
) -> Option<String> {
    production_software_attestation_env_names(adapter_id)
        .into_iter()
        .find_map(|name| env_lookup(&name).filter(|value| !value.trim().is_empty()))
}

fn production_software_attestation_missing_config_message(adapter_id: &str) -> String {
    format!(
        "{} with a real software/plugin/API/CLI/MCP run attestation",
        production_software_attestation_env_names(adapter_id).join(" or ")
    )
}

fn production_software_missing_config_message(
    control_env_names: &[impl AsRef<str>],
    adapter_id: &str,
) -> String {
    let control = control_env_names
        .iter()
        .map(AsRef::as_ref)
        .collect::<Vec<_>>()
        .join(" or ");
    let artifacts = production_software_artifact_env_names(adapter_id).join(" or ");
    format!("{control}; plus {artifacts} with local file paths")
}

fn production_software_control_env_names(adapter_id: &str) -> Vec<String> {
    let mut names = software_endpoint_env_names(adapter_id);
    names.extend(production_software_command_env_names(adapter_id));
    dedup_env_names(names)
}

fn allowed_commands_from_command(command: &str) -> Vec<String> {
    let executable = command
        .split_whitespace()
        .next()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(command);
    vec![executable.to_string()]
}

fn production_software_command_env_names(adapter_id: &str) -> Vec<String> {
    let token = env_token(adapter_id);
    let mut names = vec![
        format!("POOL_SOFTWARE_{token}_COMMAND"),
        format!("POOL_{token}_COMMAND"),
    ];
    names.extend(alias_env_tokens(adapter_id).into_iter().flat_map(|alias| {
        [
            format!("POOL_SOFTWARE_{alias}_COMMAND"),
            format!("POOL_{alias}_COMMAND"),
        ]
    }));
    dedup_env_names(names)
}

fn production_software_attestation_env_names(adapter_id: &str) -> Vec<String> {
    let token = env_token(adapter_id);
    let mut names = vec![
        format!("POOL_SOFTWARE_{token}_PRODUCTION_ATTESTATION"),
        format!("POOL_{token}_PRODUCTION_ATTESTATION"),
        "POOL_SOFTWARE_PRODUCTION_ATTESTATION".to_string(),
    ];
    names.extend(alias_env_tokens(adapter_id).into_iter().flat_map(|alias| {
        [
            format!("POOL_SOFTWARE_{alias}_PRODUCTION_ATTESTATION"),
            format!("POOL_{alias}_PRODUCTION_ATTESTATION"),
        ]
    }));
    dedup_env_names(names)
}

fn production_software_artifact_env_names(adapter_id: &str) -> Vec<String> {
    let token = env_token(adapter_id);
    let mut names = vec![
        format!("POOL_SOFTWARE_{token}_ARTIFACTS"),
        format!("POOL_{token}_ARTIFACTS"),
    ];
    names.extend(alias_env_tokens(adapter_id).into_iter().flat_map(|alias| {
        [
            format!("POOL_SOFTWARE_{alias}_ARTIFACTS"),
            format!("POOL_{alias}_ARTIFACTS"),
        ]
    }));
    dedup_env_names(names)
}

fn software_endpoint_env_names(adapter_id: &str) -> Vec<String> {
    let token = env_token(adapter_id);
    let mut names = vec![
        format!("POOL_SOFTWARE_{token}_ENDPOINT"),
        format!("POOL_{token}_ENDPOINT"),
    ];
    names.extend(alias_env_tokens(adapter_id).into_iter().flat_map(|alias| {
        [
            format!("POOL_SOFTWARE_{alias}_ENDPOINT"),
            format!("POOL_{alias}_ENDPOINT"),
        ]
    }));
    if adapter_id == "unreal" {
        names.insert(0, "POOL_UNREAL_MCP_ENDPOINT".to_string());
    }
    if adapter_id == "hermes" {
        names.insert(0, "POOL_HERMES_MCP_ENDPOINT".to_string());
        names.insert(1, "POOL_HERMES_ENDPOINT".to_string());
    }
    dedup_env_names(names)
}

fn dedup_env_names(names: Vec<String>) -> Vec<String> {
    let mut deduped = Vec::new();
    for name in names {
        if !deduped.contains(&name) {
            deduped.push(name);
        }
    }
    deduped
}

fn alias_env_tokens(adapter_id: &str) -> Vec<String> {
    match adapter_id {
        "resolve" => vec!["DAVINCI_RESOLVE".to_string()],
        "motion-db" => vec!["MOTION_DB".to_string(), "MOCAP_DB".to_string()],
        "editing-suite" => vec!["EDITING_SUITE".to_string(), "EDITOR".to_string()],
        "touchdesigner" => vec!["TOUCH_DESIGNER".to_string()],
        _ => Vec::new(),
    }
}

fn env_token(adapter_id: &str) -> String {
    adapter_id
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

fn software_production_evidence_item(
    adapter_id: &str,
    request_body: &Value,
    response: &Value,
) -> Result<Value> {
    let action_id = response
        .pointer("/report/action_id")
        .or_else(|| response.pointer("/software_action/id"))
        .or_else(|| response.pointer("/software_action_id"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .with_context(|| {
            format!("software production evidence for {adapter_id} missing action id")
        })?
        .to_string();
    let artifacts = software_response_artifacts(response);
    let verification_json = software_verification_json(&action_id, request_body, response);
    let production_attestation = request_body
        .pointer("/evidence_json/production_attestation")
        .and_then(Value::as_str)
        .context("software production evidence missing production_attestation")?;
    Ok(json!({
        "adapter_id": adapter_id,
        "external_action_id": action_id,
        "production_attestation": production_attestation,
        "action_kind": request_body.get("action_kind").cloned(),
        "priority": request_body.get("priority").cloned(),
        "control_profile": request_body
            .pointer("/evidence_json/control_profile")
            .and_then(Value::as_str)
            .unwrap_or_else(|| default_control_profile(adapter_id)),
        "task_title": format!("{adapter_id} production software evidence"),
        "artifacts": artifacts,
        "evidence_json": {
            "source": "run_software_evidence_matrix",
            "evidence_mode": "production_software",
            "production_software": true,
            "local_mock_software": false,
            "configured_real_software": request_body
                .pointer("/evidence_json/configured_real_software")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            "software_action_id": action_id,
            "production_attestation": production_attestation,
        },
        "verification_json": verification_json,
    }))
}

fn software_request_claims_real_production(request_body: &Value) -> bool {
    request_body
        .pointer("/evidence_json/production_software")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        && request_body
            .pointer("/evidence_json/configured_real_software")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        && !request_body
            .pointer("/evidence_json/local_mock_software")
            .and_then(Value::as_bool)
            .unwrap_or(true)
}

fn software_verification_json(action_id: &str, request_body: &Value, response: &Value) -> Value {
    let action_snapshot = software_action_snapshot(response, action_id).cloned();
    json!({
        "source": "run_software_evidence_matrix",
        "production_software": true,
        "local_mock_software": false,
        "software_action_id": action_id,
        "request": request_body,
        "runtime_report": response.get("report"),
        "runtime_task": response.get("task"),
        "software_action": action_snapshot,
    })
}

fn software_action_snapshot<'a>(response: &'a Value, action_id: &str) -> Option<&'a Value> {
    response
        .pointer("/snapshot/software_actions")
        .and_then(Value::as_array)?
        .iter()
        .find(|action| action.get("id").and_then(Value::as_str) == Some(action_id))
}

fn software_response_artifacts(response: &Value) -> Vec<String> {
    let mut artifacts = Vec::new();
    collect_string_array(response.pointer("/report/result/artifacts"), &mut artifacts);
    collect_string_array(response.pointer("/report/artifacts"), &mut artifacts);
    collect_string_array(
        response.pointer("/software_action/verification/artifacts"),
        &mut artifacts,
    );
    collect_string_array(response.pointer("/task/artifacts"), &mut artifacts);
    if let Some(action_id) = response
        .pointer("/report/action_id")
        .and_then(Value::as_str)
    {
        if let Some(action) = software_action_snapshot(response, action_id) {
            collect_string_array(action.pointer("/verification/artifacts"), &mut artifacts);
            collect_string_array(
                action.pointer("/command/payload_json/artifacts"),
                &mut artifacts,
            );
        }
    }
    artifacts.sort();
    artifacts.dedup();
    artifacts
        .into_iter()
        .filter(|artifact| {
            let artifact = artifact.trim();
            !artifact.is_empty() && !artifact.contains("://")
        })
        .collect()
}

fn collect_string_array(value: Option<&Value>, output: &mut Vec<String>) {
    if let Some(values) = value.and_then(Value::as_array) {
        output.extend(values.iter().filter_map(|value| {
            match value {
                Value::String(path) => Some(path.clone()),
                Value::Object(object) => object
                    .get("local_path")
                    .or_else(|| object.get("path"))
                    .or_else(|| object.get("uri"))
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
                _ => None,
            }
        }));
    }
}

fn default_control_profile(adapter_id: &str) -> &'static str {
    match adapter_id {
        "unreal" | "unity" | "hermes" => "api_mcp",
        "touchdesigner" | "madmapper" => "desktop_recognition",
        "blender" | "comfyui" | "resolve" | "nuke" | "motion-db" | "editing-suite" => "skills_cli",
        _ => "skills_cli",
    }
}

fn write_json_file(path: &Path, value: &Value) -> Result<()> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_bundle_collects_software_artifacts_and_verification() {
        let request = software_action_body_with_env(
            "blender",
            "production_software",
            true,
            |name| match name {
                "POOL_BLENDER_COMMAND" => Some("/bin/echo blender-production".to_string()),
                "POOL_BLENDER_ARTIFACTS" => {
                    Some("worlds/demo/output/production/blender/1-cleanup.blend".to_string())
                }
                "POOL_SOFTWARE_PRODUCTION_ATTESTATION" => {
                    Some("blender-software-run-prod-1".to_string())
                }
                _ => None,
            },
        );
        let response = json!({
            "report": {
                "action_id": "software-action-1",
                "status": "Succeeded",
                "result": {
                    "artifacts": ["worlds/demo/output/production/blender/1-cleanup.blend"]
                }
            },
            "task": {"id": "task-1", "status": "Succeeded"},
            "snapshot": {
                "software_actions": [
                    {
                        "id": "software-action-1",
                        "adapter_id": "blender",
                        "verification": {
                            "artifacts": ["software-evidence://blender/cli"],
                            "ok": true
                        }
                    }
                ]
            }
        });

        let item = software_production_evidence_item("blender", &request, &response).unwrap();

        assert_eq!(item["adapter_id"], "blender");
        assert_eq!(item["external_action_id"], "software-action-1");
        assert_eq!(
            item["production_attestation"],
            "blender-software-run-prod-1"
        );
        assert_eq!(item["control_profile"], "skills_cli");
        assert_eq!(item["evidence_json"]["production_software"], true);
        assert_eq!(item["evidence_json"]["configured_real_software"], true);
        assert_eq!(
            item["evidence_json"]["production_attestation"],
            "blender-software-run-prod-1"
        );
        assert_eq!(item["verification_json"]["production_software"], true);
        assert_eq!(
            item["artifacts"],
            json!(["worlds/demo/output/production/blender/1-cleanup.blend"])
        );
    }

    #[test]
    fn production_mode_without_real_software_config_does_not_claim_production() {
        let request = software_action_body_with_env(
            "blender",
            "production_software",
            true,
            |name| match name {
                "POOL_SOFTWARE_PRODUCTION_ATTESTATION" => {
                    Some("blender-software-run-prod-1".to_string())
                }
                _ => None,
            },
        );

        assert_eq!(request["payload_json"]["command"], "/usr/bin/false");
        assert_eq!(
            request["evidence_json"]["evidence_mode"],
            "production_software_missing_config"
        );
        assert_eq!(request["evidence_json"]["production_software"], false);
        assert_eq!(request["evidence_json"]["local_mock_software"], true);
        assert_eq!(
            request["evidence_json"]["missing_production_software_config"],
            "POOL_SOFTWARE_BLENDER_ENDPOINT or POOL_BLENDER_ENDPOINT or POOL_SOFTWARE_BLENDER_COMMAND or POOL_BLENDER_COMMAND; plus POOL_SOFTWARE_BLENDER_ARTIFACTS or POOL_BLENDER_ARTIFACTS with local file paths"
        );
    }

    #[test]
    fn production_mode_without_software_attestation_does_not_claim_production() {
        let request = software_action_body_with_env(
            "blender",
            "production_software",
            true,
            |name| match name {
                "POOL_BLENDER_COMMAND" => Some("/bin/echo blender-production".to_string()),
                "POOL_BLENDER_ARTIFACTS" => {
                    Some("worlds/demo/output/production/blender/1-cleanup.blend".to_string())
                }
                _ => None,
            },
        );

        assert_eq!(request["payload_json"]["command"], "/usr/bin/false");
        assert_eq!(request["evidence_json"]["production_software"], false);
        assert_eq!(request["evidence_json"]["local_mock_software"], true);
        assert_eq!(
            request["evidence_json"]["missing_production_software_config"],
            "POOL_SOFTWARE_BLENDER_PRODUCTION_ATTESTATION or POOL_BLENDER_PRODUCTION_ATTESTATION or POOL_SOFTWARE_PRODUCTION_ATTESTATION with a real software/plugin/API/CLI/MCP run attestation"
        );
    }

    #[test]
    fn production_mode_accepts_alias_command_env_names() {
        let request = software_action_body_with_env(
            "resolve",
            "production_software",
            true,
            |name| match name {
                "POOL_DAVINCI_RESOLVE_COMMAND" => {
                    Some("/usr/local/bin/resolve-render --project demo".to_string())
                }
                "POOL_DAVINCI_RESOLVE_ARTIFACTS" => {
                    Some("worlds/demo/output/production/resolve/1-master.mov".to_string())
                }
                "POOL_SOFTWARE_PRODUCTION_ATTESTATION" => {
                    Some("resolve-software-run-prod-1".to_string())
                }
                _ => None,
            },
        );

        assert_eq!(
            request["payload_json"]["command"],
            "/usr/local/bin/resolve-render --project demo"
        );
        assert_eq!(
            request["payload_json"]["allowed_commands"],
            json!(["/usr/local/bin/resolve-render"])
        );
        assert_eq!(request["evidence_json"]["production_software"], true);
        assert_eq!(request["evidence_json"]["configured_real_software"], true);
        assert_eq!(
            request["payload_json"]["artifacts"],
            json!(["worlds/demo/output/production/resolve/1-master.mov"])
        );
    }

    #[test]
    fn production_mode_requires_local_artifact_env() {
        let request = software_action_body_with_env(
            "resolve",
            "production_software",
            true,
            |name| match name {
                "POOL_DAVINCI_RESOLVE_COMMAND" => {
                    Some("/usr/local/bin/resolve-render --project demo".to_string())
                }
                "POOL_SOFTWARE_PRODUCTION_ATTESTATION" => {
                    Some("resolve-software-run-prod-1".to_string())
                }
                _ => None,
            },
        );

        assert_eq!(request["payload_json"]["command"], "/usr/bin/false");
        assert_eq!(request["evidence_json"]["production_software"], false);
        assert_eq!(request["evidence_json"]["local_mock_software"], true);
        assert!(
            request["evidence_json"]["missing_production_software_config"]
                .as_str()
                .unwrap()
                .contains("POOL_SOFTWARE_RESOLVE_ARTIFACTS")
        );
    }

    #[test]
    fn production_mode_rejects_uri_artifact_env() {
        let request = software_action_body_with_env(
            "resolve",
            "production_software",
            true,
            |name| match name {
                "POOL_DAVINCI_RESOLVE_COMMAND" => {
                    Some("/usr/local/bin/resolve-render --project demo".to_string())
                }
                "POOL_DAVINCI_RESOLVE_ARTIFACTS" => Some("resolve://timeline/demo".to_string()),
                "POOL_SOFTWARE_PRODUCTION_ATTESTATION" => {
                    Some("resolve-software-run-prod-1".to_string())
                }
                _ => None,
            },
        );

        assert_eq!(request["payload_json"]["command"], "/usr/bin/false");
        assert_eq!(request["evidence_json"]["production_software"], false);
        assert_eq!(request["evidence_json"]["local_mock_software"], true);
    }

    #[test]
    fn matrix_options_accepts_evidence_bundle_path() {
        let options = MatrixOptions::from_args([
            "target/software-evidence".to_string(),
            "--production-software".to_string(),
            "--evidence-bundle=target/software-evidence/bundle.json".to_string(),
        ]);

        assert!(options.production_software);
        assert_eq!(
            options.evidence_bundle_path(),
            PathBuf::from("target/software-evidence/bundle.json")
        );
    }
}
