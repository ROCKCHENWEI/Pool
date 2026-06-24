use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

use crate::db::{
    AgentSessionSnapshot, RuntimeSnapshot, SoftwareActionSnapshot, TaskSnapshot, WorkflowSnapshot,
};
use crate::models::{ConnectionKind, NodeStatus, NodeType, WorkflowConnection, WorkflowNode};
use crate::{
    conformance_package_catalog_resource, core_architecture_package_catalog_resource,
    desktop_recognition_contract_resource, output_package_catalog_resource,
    prd_completion_package_catalog_resource, production_evidence_handoff_package_catalog_resource,
    provider_contracts_resource, provider_gateway_worker_contract,
    runtime_handoff_package_catalog_resource, software_control_contracts_resource,
    unreal_mcp_bridge_contract_resource, ConformancePackageKind, ProviderRegistry,
    SoftwareAdapterRegistry,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResource {
    pub uri: String,
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct McpServer {
    resources: Vec<McpResource>,
    snapshot: Option<RuntimeSnapshot>,
}

impl McpServer {
    pub fn new() -> Self {
        Self {
            resources: default_resources(),
            snapshot: None,
        }
    }

    pub fn from_snapshot(snapshot: RuntimeSnapshot) -> Self {
        Self {
            resources: default_resources(),
            snapshot: Some(snapshot),
        }
    }

    pub fn with_snapshot(mut self, snapshot: RuntimeSnapshot) -> Self {
        self.snapshot = Some(snapshot);
        self
    }

    pub fn list_resources(&self) -> &[McpResource] {
        &self.resources
    }

    pub fn read_resource(&self, uri: &str) -> Result<String> {
        if let Some(snapshot) = &self.snapshot {
            return read_snapshot_resource(snapshot, uri);
        }

        match uri {
            "pool://status" => Ok(r#"{"status":"ready","runtime":"local"}"#.to_string()),
            "pool://tasks" => Ok(r#"{"queued":[],"approval_gates":[]}"#.to_string()),
            "pool://assets" => Ok(r#"{"assets":[]}"#.to_string()),
            "pool://adapters" => serde_json::to_string_pretty(&runtime_adapter_catalog_resource())
                .map_err(Into::into),
            "pool://integration-readiness" => serde_json::to_string_pretty(&json!({
                "kind": "pool_integration_readiness",
                "summary": {
                    "providers": 0,
                    "software_adapters": 0,
                    "agent_sessions": 0,
                    "ready": 0,
                    "needs_configuration": 0,
                    "needs_execution": 0,
                },
                "providers": [],
                "software_adapters": [],
                "agent": {
                    "status": "needs_runtime_snapshot",
                    "sessions": 0,
                    "next_actions": ["Use RuntimeHttpServer, pool-cli --db, or McpServer::from_snapshot."]
                }
            }))
            .map_err(Into::into),
            "pool://core-architecture-readiness" => serde_json::to_string_pretty(&json!({
                "kind": "pool_core_architecture_readiness",
                "overall_status": "requires_snapshot",
                "summary": {"ready": 0, "partial": 1, "blocked": 0, "total": 1},
                "architecture_gate": {
                    "status": "incomplete",
                    "ready_for_core_architecture": false,
                    "core_architecture_is_proven_by_current_snapshot": false,
                    "incomplete_requirements": [{
                        "id": "runtime_snapshot",
                        "status": "partial",
                        "gaps": ["No RuntimeSnapshot is attached to this MCP server."],
                        "next_actions": ["Use RuntimeHttpServer, pool-cli --db, or McpServer::from_snapshot."]
                    }]
                },
                "requirements": [{
                    "id": "runtime_snapshot",
                    "title": "Runtime snapshot required",
                    "status": "partial",
                    "summary": "Read this resource from a snapshot-backed runtime to compute core architecture readiness.",
                    "evidence": [],
                    "gaps": ["No RuntimeSnapshot is attached to this MCP server."],
                    "next_actions": ["Use RuntimeHttpServer, pool-cli --db, or McpServer::from_snapshot."]
                }]
            }))
            .map_err(Into::into),
            "pool://core-architecture-gate" => serde_json::to_string_pretty(&json!({
                "kind": "pool_core_architecture_gate",
                "overall_status": "requires_snapshot",
                "summary": {"ready": 0, "partial": 1, "blocked": 0, "total": 1},
                "architecture_gate": {
                    "status": "incomplete",
                    "ready_for_core_architecture": false,
                    "core_architecture_is_proven_by_current_snapshot": false,
                    "incomplete_requirements": [{
                        "id": "runtime_snapshot",
                        "status": "partial",
                        "gaps": ["No RuntimeSnapshot is attached to this MCP server."],
                        "next_actions": ["Use RuntimeHttpServer, pool-cli --db, or McpServer::from_snapshot."]
                    }]
                }
            }))
            .map_err(Into::into),
            "pool://core-architecture-packages" => {
                serde_json::to_string_pretty(&json!({
                    "kind": "pool_core_architecture_packages",
                    "summary": {
                        "package_count": 0,
                        "indexed_files": 0,
                        "ready_packages": 0,
                        "architecture_ready_packages": 0,
                        "local_file_failures": [],
                        "latest_asset_at": null
                    },
                    "packages": [],
                    "policy": {
                        "local_files_authoritative": true,
                        "provider_urls_are_provenance": true,
                        "expected_files": [
                            ".1-core-architecture-package-request.json",
                            "1-core-architecture-readiness.json",
                            "2-core-architecture-gate.json",
                            "3-runtime-graph.json",
                            "4-runtime-execution-plan.json",
                            "5-runtime-handoff.json",
                            "6-output-packages.json",
                            "7-strict-prd-completion-gate.json",
                            "8-core-architecture-package-manifest.json",
                            "9-runtime-snapshot.json"
                        ],
                        "production_evidence_is_out_of_scope": true
                    }
                }))
                .map_err(Into::into)
            }
            "pool://prd-completion-packages" => {
                serde_json::to_string_pretty(&json!({
                    "kind": "pool_prd_completion_packages",
                    "summary": {
                        "package_count": 0,
                        "indexed_files": 0,
                        "ready_packages": 0,
                        "completion_ready_packages": 0,
                        "local_file_failures": [],
                        "latest_asset_at": null
                    },
                    "packages": [],
                    "policy": {
                        "local_files_authoritative": true,
                        "provider_urls_are_provenance": true,
                        "expected_files": [
                            ".1-prd-completion-package-request.json",
                            "1-prd-readiness.json",
                            "2-prd-completion-gate.json",
                            "3-production-evidence-requirements.json",
                            "4-prd-completion-package-manifest.json",
                            "5-runtime-snapshot.json"
                        ],
                        "production_evidence_required_for_ready": true
                    }
                }))
                .map_err(Into::into)
            }
            "pool://production-evidence-handoff-packages" => {
                serde_json::to_string_pretty(&json!({
                    "kind": "pool_production_evidence_handoff_packages",
                    "summary": {
                        "package_count": 0,
                        "indexed_files": 0,
                        "ready_packages": 0,
                        "item_files": 0,
                        "runner_packages": 0,
                        "local_file_failures": [],
                        "latest_asset_at": null
                    },
                    "packages": [],
                    "policy": {
                        "local_files_authoritative": true,
                        "provider_urls_are_provenance": true,
                        "expected_files": [
                            ".1-production-evidence-handoff-package-request.json",
                            "1-production-evidence-requirements.json",
                            "2-production-evidence-tasks.json",
                            "3-production-evidence-handoff.json",
                            "4-production-evidence-run-plan.json",
                            "5-production-evidence-bundle.json",
                            "6-production-evidence-package-manifest.json",
                            "7-production-evidence-runner.sh",
                            "8-production-evidence-runner-preflight.json",
                            "9-runtime-snapshot.json"
                        ],
                        "item_templates_are_scaffolds": true
                    }
                }))
                .map_err(Into::into)
            }
            "pool://provider-conformance-packages" => {
                serde_json::to_string_pretty(&empty_conformance_catalog_json(
                    ConformancePackageKind::Provider,
                ))
                .map_err(Into::into)
            }
            "pool://software-conformance-packages" => {
                serde_json::to_string_pretty(&empty_conformance_catalog_json(
                    ConformancePackageKind::Software,
                ))
                .map_err(Into::into)
            }
            "pool://agent-conformance-packages" => {
                serde_json::to_string_pretty(&empty_conformance_catalog_json(
                    ConformancePackageKind::Agent,
                ))
                .map_err(Into::into)
            }
            "pool://integration-conformance-packages" => {
                serde_json::to_string_pretty(&empty_conformance_catalog_json(
                    ConformancePackageKind::Integration,
                ))
                .map_err(Into::into)
            }
            "pool://provider-contracts" => {
                serde_json::to_string_pretty(&provider_contracts_resource(None)?)
                    .map_err(Into::into)
            }
            "pool://provider-gateway-worker" => {
                serde_json::to_string_pretty(&provider_gateway_worker_contract())
                    .map_err(Into::into)
            }
            "pool://software-contracts" => {
                serde_json::to_string_pretty(&software_control_contracts_resource(None)?)
                    .map_err(Into::into)
            }
            "pool://unreal-mcp-bridge" => {
                serde_json::to_string_pretty(&unreal_mcp_bridge_contract_resource())
                    .map_err(Into::into)
            }
            _ if uri.starts_with("pool://software-contracts/") => {
                let adapter_id = uri
                    .strip_prefix("pool://software-contracts/")
                    .unwrap_or_default();
                serde_json::to_string_pretty(&software_control_contracts_resource(Some(adapter_id))?)
                    .map_err(Into::into)
            }
            "pool://desktop-recognition-contract" => {
                serde_json::to_string_pretty(&desktop_recognition_contract_resource())
                    .map_err(Into::into)
            }
            _ if uri.starts_with("pool://provider-contracts/") => {
                let provider_id = uri
                    .strip_prefix("pool://provider-contracts/")
                    .unwrap_or_default();
                serde_json::to_string_pretty(&provider_contracts_resource(Some(provider_id))?)
                    .map_err(Into::into)
            }
            "pool://projects" => Ok(r#"{"projects":[]}"#.to_string()),
            "pool://workflow" => Ok(r#"{"workflows":[],"node_states":[]}"#.to_string()),
            "pool://runtime-graph" => {
                Ok(r#"{"workflows":[],"summary":{"nodes":0,"edges":0}}"#.to_string())
            }
            "pool://runtime-execution-plan" => Ok(
                r#"{"workflows":[],"steps":[],"summary":{"steps":0,"runnable_steps":0}}"#
                    .to_string(),
            ),
            "pool://node-context" => Ok(r#"{"nodes":[]}"#.to_string()),
            "pool://events" => Ok(r#"{"events":[]}"#.to_string()),
            "pool://provider-requests" => Ok(r#"{"provider_requests":[]}"#.to_string()),
            "pool://software-actions" => Ok(r#"{"software_actions":[]}"#.to_string()),
            "pool://desktop-recognition" => {
                serde_json::to_string_pretty(&json!({
                    "requests": [],
                    "actions": [],
                    "summary": { "total": 0 },
                    "contract": desktop_recognition_contract_resource(),
                }))
                .map_err(Into::into)
            }
            "pool://agent-sessions" => Ok(r#"{"agent_sessions":[]}"#.to_string()),
            "pool://api-keys" => Ok(r#"{"api_keys":[]}"#.to_string()),
            "pool://runtime-budget" => Ok(
                r#"{"summary":{"task_estimated_tokens":0,"waiting_approval_estimated_tokens":0,"agent_token_used":0,"agent_token_budget":0,"token_total":0,"budget_remaining":null,"configured_api_keys":0,"missing_runtime_credentials":0},"provider_credentials":[],"approval_gates":[]}"#.to_string(),
            ),
            "pool://runtime-preflight" => Ok(
                r#"{"ready":true,"summary":{"blocked":0,"warnings":0,"passed":0},"checks":[],"next_actions":[]}"#.to_string(),
            ),
            "pool://runtime-handoff" => Ok(
                r#"{"ready":true,"summary":{"lanes":0,"commands":0},"lanes":[],"commands":[]}"#.to_string(),
            ),
            "pool://runtime-handoff-packages" => Ok(
                r#"{"kind":"pool_runtime_handoff_packages","summary":{"package_count":0,"indexed_files":0,"ready_packages":0,"local_file_failures":[],"latest_asset_at":null},"packages":[]}"#.to_string(),
            ),
            "pool://prd-readiness" => Ok(
                r#"{"kind":"pool_prd_readiness","overall_status":"partial","summary":{"ready":0,"partial":1,"blocked":0},"requirements":[{"id":"runtime_snapshot","title":"Runtime snapshot required","status":"partial","summary":"Read this resource from a snapshot-backed runtime to compute PRD readiness evidence.","evidence":[],"gaps":["No RuntimeSnapshot is attached to this MCP server."],"next_actions":["Use RuntimeHttpServer, pool-cli --db, or McpServer::from_snapshot."]}]}"#.to_string(),
            ),
            "pool://prd-completion-gate" => Ok(
                r#"{"kind":"pool_prd_completion_gate","overall_status":"requires_snapshot","summary":{"ready":0,"partial":1,"blocked":0},"completion_gate":{"status":"incomplete","ready_for_completion":false,"completion_is_proven_by_current_snapshot":false,"incomplete_requirements":[{"id":"runtime_snapshot","status":"partial","gaps":["No RuntimeSnapshot is attached to this MCP server."],"next_actions":["Use RuntimeHttpServer, pool-cli --db, or McpServer::from_snapshot."]}]}}"#.to_string(),
            ),
            "pool://production-evidence-requirements" => Ok(
                r#"{"kind":"pool_production_evidence_requirements","overall_status":"requires_snapshot","summary":{"complete":false},"gaps":["No RuntimeSnapshot is attached to this MCP server."],"next_actions":["Use RuntimeHttpServer, pool-cli --db, or McpServer::from_snapshot."]}"#.to_string(),
            ),
            "pool://production-evidence-tasks" => Ok(
                r#"{"kind":"pool_production_evidence_tasks","overall_status":"requires_snapshot","summary":{"total":null},"tasks":[],"gaps":["No RuntimeSnapshot is attached to this MCP server."],"next_actions":["Use RuntimeHttpServer, pool-cli --db, or McpServer::from_snapshot."]}"#.to_string(),
            ),
            "pool://production-evidence-run-plan" => Ok(
                r#"{"kind":"pool_production_evidence_run_plan","status":"requires_snapshot","ready_for_completion":false,"summary":{"missing_total":null},"gaps":["No RuntimeSnapshot is attached to this MCP server."],"next_actions":["Use RuntimeHttpServer, pool-cli --db, or McpServer::from_snapshot."]}"#.to_string(),
            ),
            "pool://production-evidence-handoff" => Ok(
                r#"{"kind":"pool_production_evidence_handoff","overall_status":"requires_snapshot","ready_for_import":false,"summary":{"missing_total":null},"gaps":["No RuntimeSnapshot is attached to this MCP server."],"next_actions":["Use RuntimeHttpServer, pool-cli --db, or McpServer::from_snapshot."]}"#.to_string(),
            ),
            "pool://production-evidence-item-template" => Ok(
                r#"{"kind":"pool_production_evidence_item_template_index","overall_status":"requires_snapshot","sample_uri":"pool://production-evidence-item-template/<task-id>","gaps":["No RuntimeSnapshot is attached to this MCP server."],"next_actions":["Use RuntimeHttpServer, pool-cli --db, or McpServer::from_snapshot."]}"#.to_string(),
            ),
            _ if uri.starts_with("pool://production-evidence-item-template/") => {
                let task_id = uri
                    .strip_prefix("pool://production-evidence-item-template/")
                    .unwrap_or_default();
                serde_json::to_string_pretty(&json!({
                    "kind": "pool_production_evidence_item_template",
                    "overall_status": "requires_snapshot",
                    "selector": {"task_id": task_id},
                    "ready_for_import": false,
                    "gaps": ["No RuntimeSnapshot is attached to this MCP server."],
                    "next_actions": ["Use RuntimeHttpServer, pool-cli --db, or McpServer::from_snapshot."]
                }))
                .map_err(Into::into)
            }
            "pool://output-packages" => Ok(
                r#"{"kind":"pool_output_packages","summary":{"total_targets":3,"indexed_targets":0,"ready_targets":0,"missing_targets":["video","game","interactive_art"],"local_file_failures":[],"latest_asset_at":null},"deliverables":[]}"#.to_string(),
            ),
            "pool://snapshot" => Ok(r#"{"version":1}"#.to_string()),
            _ => bail!("unknown Pool MCP resource: {uri}"),
        }
    }
}

fn default_resources() -> Vec<McpResource> {
    vec![
        McpResource {
            uri: "pool://status".to_string(),
            name: "Pool Status".to_string(),
            description: "Runtime status, tasks, and current adapters".to_string(),
        },
        McpResource {
            uri: "pool://projects".to_string(),
            name: "Projects".to_string(),
            description: "Local project envelopes and DB records".to_string(),
        },
        McpResource {
            uri: "pool://tasks".to_string(),
            name: "Tasks".to_string(),
            description: "Runtime queue and approval gates".to_string(),
        },
        McpResource {
            uri: "pool://assets".to_string(),
            name: "Assets".to_string(),
            description: "Indexed local assets and provider provenance".to_string(),
        },
        McpResource {
            uri: "pool://adapters".to_string(),
            name: "Adapter Catalog".to_string(),
            description: "Provider/software adapter matrix, aliases, control priority, and local-first policy".to_string(),
        },
        McpResource {
            uri: "pool://integration-readiness".to_string(),
            name: "Integration Readiness".to_string(),
            description: "Snapshot-backed readiness matrix for Provider, software, and Agent/Hermes integration work".to_string(),
        },
        McpResource {
            uri: "pool://core-architecture-readiness".to_string(),
            name: "Core Architecture Readiness".to_string(),
            description: "Snapshot-backed gate for local core architecture completion, separated from real production evidence closeout".to_string(),
        },
        McpResource {
            uri: "pool://core-architecture-gate".to_string(),
            name: "Core Architecture Gate".to_string(),
            description: "Machine-readable hard gate for local core architecture completion; use the CLI/tool require-ready mode for CI-style failure".to_string(),
        },
        McpResource {
            uri: "pool://core-architecture-packages".to_string(),
            name: "Core Architecture Packages".to_string(),
            description: "Snapshot-backed catalog of generated core architecture proof package files, manifest, commands, and MCP resources".to_string(),
        },
        McpResource {
            uri: "pool://prd-completion-packages".to_string(),
            name: "PRD Completion Packages".to_string(),
            description: "Snapshot-backed catalog of generated PRD completion proof package files, manifest, commands, and readiness status".to_string(),
        },
        McpResource {
            uri: "pool://production-evidence-handoff-packages".to_string(),
            name: "Production Evidence Handoff Packages".to_string(),
            description: "Snapshot-backed catalog of generated production evidence handoff packages, runner scripts, item files, and operator commands".to_string(),
        },
        McpResource {
            uri: "pool://provider-conformance-packages".to_string(),
            name: "Provider Conformance Packages".to_string(),
            description: "Snapshot-backed catalog of generated Provider conformance packages, local manifests, runner scripts, and gateway worker contracts".to_string(),
        },
        McpResource {
            uri: "pool://software-conformance-packages".to_string(),
            name: "Software Conformance Packages".to_string(),
            description: "Snapshot-backed catalog of generated software conformance packages, local manifests, runner scripts, and adapter contracts".to_string(),
        },
        McpResource {
            uri: "pool://agent-conformance-packages".to_string(),
            name: "Agent Conformance Packages".to_string(),
            description: "Snapshot-backed catalog of generated Agent/Hermes conformance packages, local manifests, runner scripts, and session contracts".to_string(),
        },
        McpResource {
            uri: "pool://integration-conformance-packages".to_string(),
            name: "Integration Conformance Packages".to_string(),
            description: "Snapshot-backed catalog of generated Provider + software + Agent integration conformance packages and child package manifests".to_string(),
        },
        McpResource {
            uri: "pool://provider-contracts".to_string(),
            name: "Provider Contracts".to_string(),
            description: "Machine-readable native/gateway Provider contracts; read pool://provider-contracts/<provider-id> for one Provider".to_string(),
        },
        McpResource {
            uri: "pool://provider-gateway-worker".to_string(),
            name: "Provider Gateway Worker".to_string(),
            description: "Machine-readable local HTTP forwarder contract and CLI launch instructions for AI media and 3DGS gateway workers".to_string(),
        },
        McpResource {
            uri: "pool://software-contracts".to_string(),
            name: "Software Control Contracts".to_string(),
            description: "Machine-readable software adapter control contracts; read pool://software-contracts/<adapter-id> for one adapter".to_string(),
        },
        McpResource {
            uri: "pool://unreal-mcp-bridge".to_string(),
            name: "Unreal MCP Bridge".to_string(),
            description: "Machine-readable Unreal plugin/gateway bridge contract for pool_unreal_action and mcp_payload".to_string(),
        },
        McpResource {
            uri: "pool://desktop-recognition-contract".to_string(),
            name: "Desktop Recognition Contract".to_string(),
            description: "Machine-readable desktop recognition request, queue, callback, and evidence contract".to_string(),
        },
        McpResource {
            uri: "pool://workflow".to_string(),
            name: "Workflow".to_string(),
            description: "Workflow graph and node runtime states; read pool://workflow/<workflow-id> for scoped tasks, assets, provider requests, software actions, and agent sessions".to_string(),
        },
        McpResource {
            uri: "pool://runtime-graph".to_string(),
            name: "Runtime Graph".to_string(),
            description: "Executable node graph with statuses, task types, and connection labels"
                .to_string(),
        },
        McpResource {
            uri: "pool://runtime-execution-plan".to_string(),
            name: "Runtime Execution Plan".to_string(),
            description: "Ordered executable workflow steps with node status, contracts, controls, gates, and next actions".to_string(),
        },
        McpResource {
            uri: "pool://node-context".to_string(),
            name: "Node Context".to_string(),
            description: "Node context index; read pool://node-context/<node-id> for tasks, assets, provider requests, and software actions".to_string(),
        },
        McpResource {
            uri: "pool://events".to_string(),
            name: "Events".to_string(),
            description: "Runtime event stream".to_string(),
        },
        McpResource {
            uri: "pool://provider-requests".to_string(),
            name: "Provider Requests".to_string(),
            description: "Provider request and response ledger".to_string(),
        },
        McpResource {
            uri: "pool://software-actions".to_string(),
            name: "Software Actions".to_string(),
            description: "External software control audit log".to_string(),
        },
        McpResource {
            uri: "pool://desktop-recognition".to_string(),
            name: "Desktop Recognition".to_string(),
            description: "Desktop recognition control queue and result history".to_string(),
        },
        McpResource {
            uri: "pool://agent-sessions".to_string(),
            name: "Agent Sessions".to_string(),
            description: "Hermes and Agent CLI sessions".to_string(),
        },
        McpResource {
            uri: "pool://api-keys".to_string(),
            name: "API Keys".to_string(),
            description: "Sanitized Provider credential status".to_string(),
        },
        McpResource {
            uri: "pool://runtime-budget".to_string(),
            name: "Runtime Budget".to_string(),
            description: "Token budget, approval cost, Provider credential readiness, and request ledger summary".to_string(),
        },
        McpResource {
            uri: "pool://runtime-preflight".to_string(),
            name: "Runtime Preflight".to_string(),
            description: "Run readiness, blocking checks, warnings, and suggested CLI actions"
                .to_string(),
        },
        McpResource {
            uri: "pool://runtime-handoff".to_string(),
            name: "Runtime Handoff".to_string(),
            description: "Machine-readable Agent/Hermes/operator handoff runbook derived from runtime state".to_string(),
        },
        McpResource {
            uri: "pool://runtime-handoff-packages".to_string(),
            name: "Runtime Handoff Packages".to_string(),
            description: "Snapshot-backed catalog of generated runtime handoff package files, operator checklist, Agent entrypoint, and MCP resources".to_string(),
        },
        McpResource {
            uri: "pool://prd-readiness".to_string(),
            name: "PRD Readiness".to_string(),
            description: "Requirement-by-requirement readiness audit for the Pool content-burst PRD, with evidence and remaining gaps".to_string(),
        },
        McpResource {
            uri: "pool://prd-completion-gate".to_string(),
            name: "PRD Completion Gate".to_string(),
            description: "Machine-readable completion gate derived from the PRD readiness audit".to_string(),
        },
        McpResource {
            uri: "pool://production-evidence-requirements".to_string(),
            name: "Production Evidence Requirements".to_string(),
            description: "Machine-readable checklist for real provider, software, and desktop-vision evidence required to close PRD production gaps".to_string(),
        },
        McpResource {
            uri: "pool://production-evidence-tasks".to_string(),
            name: "Production Evidence Tasks".to_string(),
            description: "Read-only task queue for missing real provider, software, and desktop-vision production evidence items".to_string(),
        },
        McpResource {
            uri: "pool://production-evidence-run-plan".to_string(),
            name: "Production Evidence Run Plan".to_string(),
            description: "Read-only seven-phase run plan for collecting, merging, validating, importing, and proving real production evidence".to_string(),
        },
        McpResource {
            uri: "pool://production-evidence-handoff".to_string(),
            name: "Production Evidence Handoff".to_string(),
            description: "Read-only handoff context for assigning real production evidence work to Provider workers, software operators, and desktop vision controllers".to_string(),
        },
        McpResource {
            uri: "pool://production-evidence-item-template".to_string(),
            name: "Production Evidence Item Template".to_string(),
            description: "Read pool://production-evidence-item-template/<task-id> for one submit-production-evidence-item JSON scaffold".to_string(),
        },
        McpResource {
            uri: "pool://output-packages".to_string(),
            name: "Output Packages".to_string(),
            description: "Video, game, and interactive-art deliverable readiness derived from local indexed manifests".to_string(),
        },
        McpResource {
            uri: "pool://snapshot".to_string(),
            name: "Runtime Snapshot".to_string(),
            description: "Complete RuntimeSnapshot JSON".to_string(),
        },
    ]
}

fn read_snapshot_resource(snapshot: &RuntimeSnapshot, uri: &str) -> Result<String> {
    let value = match uri {
        "pool://status" => json!({
            "status": "ready",
            "runtime": "local",
            "generated_at": snapshot.generated_at,
            "project_filter": snapshot.project_filter,
            "stats": snapshot.stats,
        }),
        "pool://projects" => json!({ "projects": snapshot.projects }),
        "pool://tasks" => json!({
            "tasks": snapshot.tasks,
            "approval_gates": snapshot
                .tasks
                .iter()
                .filter(|task| task.requires_approval || task.status == "WaitingApproval")
                .collect::<Vec<_>>()
        }),
        "pool://assets" => json!({ "assets": snapshot.assets }),
        "pool://adapters" => runtime_adapter_catalog_resource(),
        "pool://integration-readiness" => runtime_integration_readiness_resource(snapshot),
        "pool://core-architecture-readiness" => {
            runtime_core_architecture_readiness_resource(snapshot)?
        }
        "pool://core-architecture-gate" => {
            let readiness = runtime_core_architecture_readiness_resource(snapshot)?;
            json!({
                "kind": "pool_core_architecture_gate",
                "overall_status": readiness.get("overall_status").cloned().unwrap_or_else(|| json!("partial")),
                "summary": readiness.get("summary").cloned().unwrap_or_else(|| json!({})),
                "architecture_gate": readiness.get("architecture_gate").cloned().unwrap_or_else(|| json!({})),
                "requirements": readiness.get("requirements").cloned().unwrap_or_else(|| json!([])),
                "source_resources": readiness.get("source_resources").cloned().unwrap_or_else(|| json!([])),
            })
        }
        "pool://core-architecture-packages" => {
            serde_json::to_value(core_architecture_package_catalog_resource(snapshot))?
        }
        "pool://prd-completion-packages" => {
            serde_json::to_value(prd_completion_package_catalog_resource(snapshot))?
        }
        "pool://production-evidence-handoff-packages" => serde_json::to_value(
            production_evidence_handoff_package_catalog_resource(snapshot),
        )?,
        "pool://provider-conformance-packages" => serde_json::to_value(
            conformance_package_catalog_resource(snapshot, ConformancePackageKind::Provider),
        )?,
        "pool://software-conformance-packages" => serde_json::to_value(
            conformance_package_catalog_resource(snapshot, ConformancePackageKind::Software),
        )?,
        "pool://agent-conformance-packages" => serde_json::to_value(
            conformance_package_catalog_resource(snapshot, ConformancePackageKind::Agent),
        )?,
        "pool://integration-conformance-packages" => serde_json::to_value(
            conformance_package_catalog_resource(snapshot, ConformancePackageKind::Integration),
        )?,
        "pool://provider-contracts" => provider_contracts_resource(None)?,
        "pool://provider-gateway-worker" => provider_gateway_worker_contract(),
        "pool://software-contracts" => software_control_contracts_resource(None)?,
        "pool://unreal-mcp-bridge" => unreal_mcp_bridge_contract_resource(),
        _ if uri.starts_with("pool://software-contracts/") => {
            let adapter_id = uri
                .strip_prefix("pool://software-contracts/")
                .unwrap_or_default();
            software_control_contracts_resource(Some(adapter_id))?
        }
        "pool://desktop-recognition-contract" => desktop_recognition_contract_resource(),
        _ if uri.starts_with("pool://provider-contracts/") => {
            let provider_id = uri
                .strip_prefix("pool://provider-contracts/")
                .unwrap_or_default();
            provider_contracts_resource(Some(provider_id))?
        }
        "pool://workflow" => json!({
            "workflows": snapshot.workflows,
            "node_states": snapshot.node_states,
        }),
        _ if uri.starts_with("pool://workflow/") => {
            let workflow_id = uri.strip_prefix("pool://workflow/").unwrap_or_default();
            runtime_workflow_context_resource(snapshot, workflow_id)?
        }
        "pool://runtime-graph" => runtime_graph_resource(snapshot)?,
        "pool://runtime-execution-plan" => runtime_execution_plan_resource(snapshot)?,
        "pool://node-context" => runtime_node_context_index_resource(snapshot)?,
        _ if uri.starts_with("pool://node-context/") => {
            let node_id = uri.strip_prefix("pool://node-context/").unwrap_or_default();
            runtime_node_context_resource(snapshot, node_id)?
        }
        "pool://events" => json!({ "events": snapshot.events }),
        "pool://provider-requests" => {
            json!({ "provider_requests": snapshot.provider_requests })
        }
        "pool://software-actions" => json!({
            "software_actions": snapshot.software_actions,
            "summary": software_action_summary(snapshot),
        }),
        "pool://desktop-recognition" => desktop_recognition_resource(snapshot),
        "pool://agent-sessions" => json!({
            "agent_sessions": snapshot.agent_sessions,
            "summary": agent_session_summary(snapshot),
        }),
        "pool://api-keys" => json!({ "api_keys": snapshot.api_keys }),
        "pool://runtime-budget" => runtime_budget_resource(snapshot),
        "pool://runtime-preflight" => runtime_preflight_resource(snapshot)?,
        "pool://runtime-handoff" => runtime_handoff_resource(snapshot)?,
        "pool://runtime-handoff-packages" => {
            serde_json::to_value(runtime_handoff_package_catalog_resource(snapshot))?
        }
        "pool://prd-readiness" => runtime_prd_readiness_resource(snapshot)?,
        "pool://prd-completion-gate" => runtime_prd_completion_gate_resource(snapshot)?,
        "pool://production-evidence-requirements" => {
            runtime_production_evidence_requirements_resource(snapshot)?
        }
        "pool://production-evidence-tasks" => {
            let project_slug = snapshot
                .project_filter
                .as_deref()
                .filter(|project_slug| *project_slug != "*")
                .unwrap_or("<slug>");
            production_evidence_tasks_resource(project_slug, snapshot)?
        }
        "pool://production-evidence-run-plan" => {
            let project_slug = snapshot.project_filter.as_deref().unwrap_or("<slug>");
            production_evidence_run_plan_resource(
                project_slug,
                None,
                "mcp-production-evidence-run-plan-resource",
                snapshot,
            )?
        }
        "pool://production-evidence-handoff" => {
            let project_slug = snapshot
                .project_filter
                .as_deref()
                .filter(|project_slug| *project_slug != "*")
                .unwrap_or("<slug>");
            production_evidence_handoff_resource(
                project_slug,
                None,
                "mcp-production-evidence-handoff-resource",
                snapshot,
            )?
        }
        "pool://production-evidence-item-template" => {
            production_evidence_item_template_index_resource(snapshot)?
        }
        _ if uri.starts_with("pool://production-evidence-item-template/") => {
            let project_slug = snapshot
                .project_filter
                .as_deref()
                .filter(|project_slug| *project_slug != "*")
                .unwrap_or("<slug>");
            let task_id = uri
                .strip_prefix("pool://production-evidence-item-template/")
                .unwrap_or_default();
            production_evidence_item_template_resource(
                project_slug,
                None,
                "mcp-production-evidence-item-template-resource",
                task_id,
                snapshot,
            )?
        }
        "pool://output-packages" => {
            serde_json::to_value(output_package_catalog_resource(snapshot))?
        }
        "pool://snapshot" => serde_json::to_value(snapshot)?,
        _ => bail!("unknown Pool MCP resource: {uri}"),
    };

    serde_json::to_string_pretty(&value).map_err(Into::into)
}

pub fn runtime_adapter_catalog_resource() -> Value {
    let provider_registry = ProviderRegistry::defaults();
    let software_registry = SoftwareAdapterRegistry::defaults();

    json!({
        "providers": provider_registry.configs(),
        "software_adapters": software_registry.configs(),
        "control_priority_chain": SoftwareAdapterRegistry::control_priority_chain(),
        "provider_aliases": provider_aliases(),
        "policy": {
            "control_priority": "API/MCP > Skills/CLI > Desktop Recognition > Human Takeover",
            "local_files_authoritative": true,
            "provider_urls_are_provenance": true,
            "high_cost_requires_approval": true,
        },
    })
}

pub fn runtime_integration_readiness_resource(snapshot: &RuntimeSnapshot) -> Value {
    let provider_registry = ProviderRegistry::defaults();
    let software_registry = SoftwareAdapterRegistry::defaults();
    let providers = provider_registry
        .configs()
        .into_iter()
        .map(|provider| {
            integration_provider_readiness(snapshot, provider.id.as_str(), &json!(provider))
        })
        .collect::<Vec<_>>();
    let software_adapters = software_registry
        .configs()
        .into_iter()
        .map(|adapter| {
            integration_software_readiness(snapshot, adapter.id.as_str(), &json!(adapter))
        })
        .collect::<Vec<_>>();
    let agent = integration_agent_readiness(snapshot);

    let mut statuses = providers
        .iter()
        .chain(software_adapters.iter())
        .filter_map(|row| row["status"].as_str())
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    if let Some(agent_status) = agent["status"].as_str() {
        statuses.push(agent_status.to_string());
    }
    let lanes = integration_team_lanes(&providers, &software_adapters, &agent);
    let run_plan = integration_run_plan(&providers, &software_adapters, &agent);
    let ready = statuses
        .iter()
        .filter(|status| status.as_str() == "ready")
        .count();
    let needs_configuration = statuses
        .iter()
        .filter(|status| status.as_str() == "needs_configuration")
        .count();
    let needs_execution = statuses
        .iter()
        .filter(|status| status.as_str() == "needs_execution")
        .count();
    let needs_attention = statuses
        .iter()
        .filter(|status| status.as_str() == "needs_attention")
        .count();

    json!({
        "kind": "pool_integration_readiness",
        "project_filter": snapshot.project_filter,
        "generated_at": snapshot.generated_at,
        "summary": {
            "providers": providers.len(),
            "software_adapters": software_adapters.len(),
            "agent_sessions": snapshot.agent_sessions.len(),
            "ready": ready,
            "needs_configuration": needs_configuration,
            "needs_execution": needs_execution,
            "needs_attention": needs_attention,
            "total": statuses.len(),
            "lanes": lanes.len(),
            "actions": run_plan.len(),
        },
        "lanes": lanes,
        "run_plan": run_plan,
        "providers": providers,
        "software_adapters": software_adapters,
        "agent": agent,
        "commands": {
            "integration_conformance_package": "pool-cli --project <slug> integration-conformance-package --output-dir worlds/<slug>/output",
            "production_evidence_handoff_package": "pool-cli --project <slug> production-evidence-handoff-package --output-dir worlds/<slug>/output --output-root worlds/<slug>/output/production-evidence --include-snapshot",
            "prd_readiness": "pool-cli --project <slug> prd-readiness",
        },
        "policy": {
            "status_ready_requires_successful_local_ledger": true,
            "provider_urls_are_provenance": true,
            "local_files_authoritative": true,
            "control_priority": "API/MCP > Skills/CLI > Desktop Recognition > Human Takeover",
        }
    })
}

pub fn runtime_core_architecture_readiness_resource(snapshot: &RuntimeSnapshot) -> Result<Value> {
    let provider_registry = ProviderRegistry::defaults();
    let software_registry = SoftwareAdapterRegistry::defaults();
    let provider_configs = provider_registry.configs();
    let software_configs = software_registry.configs();
    let provider_ids = provider_configs
        .iter()
        .map(|config| config.id.clone())
        .collect::<Vec<_>>();
    let software_ids = software_configs
        .iter()
        .map(|config| config.id.clone())
        .collect::<Vec<_>>();
    let missing_required_providers = REQUIRED_PROVIDER_EVIDENCE
        .iter()
        .filter(|provider_id| !provider_ids.iter().any(|id| id == **provider_id))
        .map(|provider_id| provider_id.to_string())
        .collect::<Vec<_>>();
    let missing_required_software = REQUIRED_SOFTWARE_EVIDENCE
        .iter()
        .filter(|adapter_id| !software_ids.iter().any(|id| id == **adapter_id))
        .map(|adapter_id| adapter_id.to_string())
        .collect::<Vec<_>>();
    let graph = runtime_graph_resource(snapshot)?;
    let execution_plan = runtime_execution_plan_resource(snapshot)?;
    let budget = runtime_budget_resource(snapshot);
    let handoff = runtime_handoff_resource(snapshot)?;
    let handoff_catalog = serde_json::to_value(runtime_handoff_package_catalog_resource(snapshot))?;
    let output_catalog = output_package_catalog_resource(snapshot);
    let provider_contracts = provider_contracts_resource(None)?;
    let software_contracts = software_control_contracts_resource(None)?;
    let desktop_contract = desktop_recognition_contract_resource();
    let local_asset_count = snapshot
        .assets
        .iter()
        .filter(|asset| !asset.local_path.trim().is_empty())
        .count();
    let output_ready = output_catalog.summary.ready_targets == output_catalog.summary.total_targets
        && output_catalog.summary.total_targets > 0;
    let project_slug = snapshot.project_filter.as_deref().unwrap_or("<slug>");

    let requirements = vec![
        prd_requirement(
            "runtime_project_workflow",
            "Project, workflow, and SQLite runtime baseline",
            if snapshot.projects.is_empty() || snapshot.workflows.is_empty() {
                "partial"
            } else {
                "ready"
            },
            "The ROCKCHENWEI/Pool baseline is represented as local projects, workflows, tasks, events, and snapshot stats.",
            json!({
                "projects": snapshot.projects.len(),
                "workflows": snapshot.workflows.len(),
                "tasks": snapshot.tasks.len(),
                "events": snapshot.events.len(),
                "stats": snapshot.stats,
            }),
            if snapshot.projects.is_empty() || snapshot.workflows.is_empty() {
                vec!["No project/workflow is materialized in the current runtime snapshot."]
            } else {
                Vec::<&str>::new()
            },
            vec!["Run pool-cli --project <slug> run-workflow or persist_default_plan to materialize the runtime baseline."],
        ),
        prd_requirement(
            "node_graph_execution",
            "Executable node graph and task queue",
            if snapshot.workflows.is_empty() || snapshot.tasks.is_empty() {
                "partial"
            } else {
                "ready"
            },
            "Workflow graph, runtime execution plan, task statuses, approval gates, and run-next controls are derivable from the same snapshot.",
            json!({
                "runtime_graph_summary": graph.get("summary").cloned().unwrap_or_else(|| json!({})),
                "execution_plan_summary": execution_plan.get("summary").cloned().unwrap_or_else(|| json!({})),
            }),
            if snapshot.tasks.is_empty() {
                vec!["No runtime tasks are recorded for the current project."]
            } else {
                Vec::<&str>::new()
            },
            vec!["Use pool-cli runtime-execution-plan and runtime-run-next to inspect or advance nodes."],
        ),
        prd_requirement(
            "agent_hermes_control",
            "Hermes, Agent CLI, MCP, and token control",
            if snapshot.agent_sessions.is_empty() {
                "partial"
            } else {
                "ready"
            },
            "Agent sessions, transcript paths, MCP resources, prompts, API key status, and token budget are visible to operators.",
            json!({
                "agent_sessions": snapshot.agent_sessions.len(),
                "api_keys": snapshot.api_keys.len(),
                "budget_summary": budget.get("summary").cloned().unwrap_or_else(|| json!({})),
                "mcp_resources": default_resources().len(),
            }),
            if snapshot.agent_sessions.is_empty() {
                vec!["No Hermes or Agent CLI session has been staged or executed."]
            } else {
                Vec::<&str>::new()
            },
            vec!["Run pool-cli agent-session or run-workflow with agent mode enabled."],
        ),
        prd_requirement(
            "provider_adapter_contracts",
            "AI media and 3DGS Provider adapter contracts",
            if missing_required_providers.is_empty() {
                "ready"
            } else {
                "partial"
            },
            "Midjourney/image-2/Nano Banana Pro/Suno and 3DGS providers are represented through registry IDs, contracts, gateway worker, and request ledger paths.",
            json!({
                "registered_providers": provider_ids,
                "required_providers": REQUIRED_PROVIDER_EVIDENCE,
                "missing_required_providers": missing_required_providers.clone(),
                "provider_requests": snapshot.provider_requests.len(),
                "provider_contracts_kind": provider_contracts["kind"],
                "gateway_worker": provider_gateway_worker_contract()["kind"],
            }),
            if missing_required_providers.is_empty() {
                Vec::<String>::new()
            } else {
                vec![format!(
                    "Provider registry is missing required adapters: {}.",
                    missing_required_providers.join(", ")
                )]
            },
            vec!["Use production-evidence-provider-matrix later to prove real upstream execution; core architecture only requires registered contracts."],
        ),
        prd_requirement(
            "software_control_contracts",
            "External software control contracts",
            if missing_required_software.is_empty() {
                "ready"
            } else {
                "partial"
            },
            "Unreal, Unity, Resolve, editing software, TouchDesigner, MadMapper, Blender, ComfyUI, motion DB, Nuke, and Hermes are modeled through SoftwareAdapterConfig and control contracts.",
            json!({
                "registered_software": software_ids,
                "required_software": REQUIRED_SOFTWARE_EVIDENCE,
                "missing_required_software": missing_required_software.clone(),
                "software_actions": snapshot.software_actions.len(),
                "software_contracts_kind": software_contracts["kind"],
                "desktop_contract_kind": desktop_contract["kind"],
                "control_priority": ["API/MCP", "Skills/CLI", "Desktop Recognition", "Human Takeover"],
            }),
            if missing_required_software.is_empty() {
                Vec::<String>::new()
            } else {
                vec![format!(
                    "Software registry is missing required adapters: {}.",
                    missing_required_software.join(", ")
                )]
            },
            vec!["Use production-evidence-software-matrix later to prove real software execution; core architecture only requires registered contracts."],
        ),
        prd_requirement(
            "unreal_first_assembly",
            "Unreal-first assembly path",
            if has_software_action_for(snapshot, "unreal") {
                "ready"
            } else {
                "partial"
            },
            "Unreal is the first deep engine target and can be driven through Unreal MCP bridge, software action ledger, or mock fallback.",
            json!({
                "unreal_actions": snapshot.software_actions.iter().filter(|action| action.adapter_id == "unreal").count(),
                "unreal_bridge_kind": unreal_mcp_bridge_contract_resource()["kind"],
            }),
            if has_software_action_for(snapshot, "unreal") {
                Vec::<&str>::new()
            } else {
                vec!["No Unreal software action is recorded in the current snapshot."]
            },
            vec!["Run pool-cli run-software unreal or run-workflow with unreal mode enabled."],
        ),
        prd_requirement(
            "output_targets",
            "Video, game, and interactive-art output targets",
            if output_ready { "ready" } else { "partial" },
            "The output package catalog can recover indexed local manifests for video timeline, game build, and interactive-art cues.",
            json!({
                "summary": output_catalog.summary,
                "deliverables": output_catalog.deliverables,
            }),
            if output_ready {
                Vec::<&str>::new()
            } else {
                vec!["One or more output deliverable manifests are missing or not locally readable."]
            },
            vec!["Run pool-cli output-package or run-workflow to generate all three deliverable manifests."],
        ),
        prd_requirement(
            "local_first_asset_envelope",
            "image-blaster local-first asset envelope",
            if local_asset_count > 0 { "ready" } else { "partial" },
            "Indexed assets use local files as the loading source of truth; provider URLs remain provenance.",
            json!({
                "assets": snapshot.assets.len(),
                "local_asset_count": local_asset_count,
                "policy": {
                    "local_files_authoritative": true,
                    "provider_urls_are_provenance": true,
                    "indexed_files": true,
                },
            }),
            if local_asset_count > 0 {
                Vec::<&str>::new()
            } else {
                vec!["No local indexed asset is recorded in the current snapshot."]
            },
            vec!["Run a Provider/mock 3DGS/output package step so generated assets are indexed in SQLite."],
        ),
        prd_requirement(
            "handoff_and_mcp_surface",
            "Runtime handoff, MCP resources, and package readback",
            "ready",
            "The runtime exposes Agent/Hermes/operator handoff, MCP resources, and recoverable handoff package catalog.",
            json!({
                "handoff_summary": handoff.get("summary").cloned().unwrap_or_else(|| json!({})),
                "handoff_package_summary": handoff_catalog.get("summary").cloned().unwrap_or_else(|| json!({})),
                "resources": [
                    "pool://core-architecture-readiness",
                    "pool://runtime-handoff",
                    "pool://runtime-handoff-packages",
                    "pool://prd-completion-gate"
                ],
            }),
            Vec::<&str>::new(),
            vec!["Use pool-cli runtime-handoff-packages to recover the latest local handoff package after reconnect."],
        ),
        prd_requirement(
            "production_evidence_boundary",
            "Production evidence boundary remains strict",
            "ready",
            "Core architecture readiness is intentionally separate from PRD production completion, which still requires real Provider/software/desktop-vision evidence.",
            json!({
                "strict_prd_completion_gate": "pool://prd-completion-gate",
                "production_evidence_requirements": "pool://production-evidence-requirements",
                "deferred_not_blocking_core_architecture": [
                    "real upstream Provider evidence",
                    "real external software execution evidence",
                    "external visual model desktop trace evidence"
                ],
            }),
            Vec::<&str>::new(),
            vec!["Use pool-cli prd-completion-gate --require-complete only when closing the full production PRD."],
        ),
    ];
    let summary = prd_readiness_summary(&requirements);
    let overall_status = if summary.get("blocked").and_then(Value::as_u64).unwrap_or(0) > 0 {
        "blocked"
    } else if summary.get("partial").and_then(Value::as_u64).unwrap_or(0) > 0 {
        "partial"
    } else {
        "ready"
    };
    let architecture_gate =
        core_architecture_gate(project_slug, &requirements, &summary, overall_status);

    Ok(json!({
        "kind": "pool_core_architecture_readiness",
        "version": 1,
        "project_filter": snapshot.project_filter,
        "generated_at": snapshot.generated_at,
        "overall_status": overall_status,
        "summary": summary,
        "architecture_gate": architecture_gate,
        "requirements": requirements,
        "source_resources": [
            "pool://runtime-graph",
            "pool://runtime-execution-plan",
            "pool://integration-readiness",
            "pool://runtime-handoff",
            "pool://runtime-handoff-packages",
            "pool://provider-contracts",
            "pool://software-contracts",
            "pool://desktop-recognition-contract",
            "pool://output-packages",
            "pool://prd-completion-gate"
        ],
    }))
}

fn integration_provider_readiness(
    snapshot: &RuntimeSnapshot,
    provider_id: &str,
    config: &Value,
) -> Value {
    let tasks = snapshot
        .tasks
        .iter()
        .filter(|task| task.provider_id.as_deref() == Some(provider_id))
        .collect::<Vec<_>>();
    let requests = snapshot
        .provider_requests
        .iter()
        .filter(|request| request.provider_id == provider_id)
        .collect::<Vec<_>>();
    let api_key = snapshot
        .api_keys
        .iter()
        .find(|key| key.provider == provider_id && key.service_type == "provider");
    let success_count = tasks
        .iter()
        .filter(|task| task.status == "Succeeded")
        .count();
    let failed_count = tasks.iter().filter(|task| task.status == "Failed").count();
    let waiting_approval_count = tasks
        .iter()
        .filter(|task| task.status == "WaitingApproval")
        .count();
    let configured = api_key.is_some_and(|key| key.configured);
    let status = if success_count > 0 {
        "ready"
    } else if failed_count > 0 {
        "needs_attention"
    } else if configured || !requests.is_empty() {
        "needs_execution"
    } else {
        "needs_configuration"
    };
    let lane = integration_provider_lane(provider_id, config);
    let latest_task = tasks
        .iter()
        .max_by_key(|task| task.updated_at.as_str())
        .map(|task| {
            json!({
                "id": task.id,
                "status": task.status,
                "node_id": task.node_id,
                "updated_at": task.updated_at,
            })
        });
    let latest_request = requests
        .iter()
        .max_by_key(|request| request.created_at.as_str())
        .map(|request| {
            json!({
                "id": request.id,
                "task_id": request.task_id,
                "metadata_path": request.metadata_path,
                "created_at": request.created_at,
            })
        });
    let failed_task_id = tasks
        .iter()
        .find(|task| task.status == "Failed")
        .map(|task| task.id.as_str());
    let set_key_command =
        format!("pool-cli --project <slug> set-api-key {provider_id} --api-key-env <ENV>");
    let health_command = format!("pool-cli --project <slug> provider-health {provider_id}");
    let run_mock_command = format!(
        "pool-cli --project <slug> run-provider {provider_id} --execution-mode mock --no-approval --prompt \"integration smoke\""
    );
    let conformance_command = format!(
        "pool-cli --project <slug> provider-conformance-package {provider_id} --output-dir worlds/<slug>/output"
    );
    let retry_command =
        failed_task_id.map(|task_id| format!("pool-cli --project <slug> retry-task {task_id}"));
    let next_action = integration_provider_next_action(
        status,
        configured,
        retry_command.as_deref(),
        &set_key_command,
        &health_command,
        &run_mock_command,
        &conformance_command,
    );

    json!({
        "provider_id": provider_id,
        "display_name": config["display_name"].clone(),
        "kind": config["kind"].clone(),
        "lane": lane,
        "status": status,
        "next_action": next_action,
        "configured": configured,
        "key_hint": api_key.and_then(|key| key.key_hint.clone()),
        "task_count": tasks.len(),
        "request_count": requests.len(),
        "success_count": success_count,
        "failed_count": failed_count,
        "waiting_approval_count": waiting_approval_count,
        "latest_task": latest_task,
        "latest_request": latest_request,
        "commands": {
            "set_key": set_key_command,
            "health": health_command,
            "run_mock": run_mock_command,
            "conformance_package": conformance_command,
        }
    })
}

fn integration_software_readiness(
    snapshot: &RuntimeSnapshot,
    adapter_id: &str,
    config: &Value,
) -> Value {
    let actions = snapshot
        .software_actions
        .iter()
        .filter(|action| action.adapter_id == adapter_id)
        .collect::<Vec<_>>();
    let tasks = snapshot
        .tasks
        .iter()
        .filter(|task| task.provider_id.as_deref() == Some(adapter_id))
        .collect::<Vec<_>>();
    let success_count = actions
        .iter()
        .filter(|action| {
            action
                .verification
                .as_ref()
                .is_some_and(verification_succeeded)
        })
        .count()
        + tasks
            .iter()
            .filter(|task| task.status == "Succeeded")
            .count();
    let failed_count = tasks.iter().filter(|task| task.status == "Failed").count();
    let status = if success_count > 0 {
        "ready"
    } else if failed_count > 0 {
        "needs_attention"
    } else {
        "needs_execution"
    };
    let lane = integration_software_lane(adapter_id);
    let latest_action = actions
        .iter()
        .max_by_key(|action| action.created_at.as_str())
        .map(|action| {
            json!({
                "id": action.id,
                "task_id": action.task_id,
                "action_kind": action.action_kind,
                "created_at": action.created_at,
                "verification": action.verification,
            })
        });
    let failed_task_id = tasks
        .iter()
        .find(|task| task.status == "Failed")
        .map(|task| task.id.as_str());
    let health_command = format!("pool-cli --project <slug> software-health {adapter_id}");
    let run_command = format!(
        "pool-cli --project <slug> run-software {adapter_id} --action execute-cli --priority SkillsCli --payload-json '{{\"command\":\"/bin/echo {adapter_id}-ok\",\"allowed_commands\":[\"/bin/echo\",\"echo\"]}}'"
    );
    let conformance_command = format!(
        "pool-cli --project <slug> software-conformance-package {adapter_id} --output-dir worlds/<slug>/output"
    );
    let retry_command =
        failed_task_id.map(|task_id| format!("pool-cli --project <slug> retry-task {task_id}"));
    let next_action = integration_software_next_action(
        status,
        retry_command.as_deref(),
        &run_command,
        &conformance_command,
    );

    json!({
        "adapter_id": adapter_id,
        "display_name": config["display_name"].clone(),
        "lane": lane,
        "status": status,
        "next_action": next_action,
        "control_modes": config["control_modes"].clone(),
        "desktop_fallback": config["desktop_fallback"].clone(),
        "action_count": actions.len(),
        "task_count": tasks.len(),
        "success_count": success_count,
        "failed_count": failed_count,
        "latest_action": latest_action,
        "commands": {
            "health": health_command,
            "run": run_command,
            "conformance_package": conformance_command,
        }
    })
}

fn integration_agent_readiness(snapshot: &RuntimeSnapshot) -> Value {
    let sessions = snapshot.agent_sessions.len();
    let transcripts = snapshot
        .agent_sessions
        .iter()
        .filter(|session| session.transcript_path.is_some())
        .count();
    let latest_session = snapshot
        .agent_sessions
        .iter()
        .max_by_key(|session| session.updated_at.as_str())
        .map(|session| {
            json!({
                "id": session.id,
                "transcript_path": session.transcript_path,
                "token_used": session.token_used,
                "token_budget": session.token_budget,
                "updated_at": session.updated_at,
            })
        });
    let status = if sessions > 0 && transcripts > 0 {
        "ready"
    } else {
        "needs_execution"
    };

    json!({
        "lane": "orchestration",
        "status": status,
        "next_action": integration_agent_next_action(status),
        "sessions": sessions,
        "transcripts": transcripts,
        "latest_session": latest_session,
        "commands": {
            "stage": "pool-cli --project <slug> agent-session agent-cli --command-id workflow-context --command \"pool-cli --project <slug> workflow-context\" --tool cli --tool mcp",
            "conformance_package": "pool-cli --project <slug> agent-conformance-package all --output-dir worlds/<slug>/output",
        }
    })
}

fn integration_provider_lane(provider_id: &str, config: &Value) -> &'static str {
    match config["kind"].as_str().unwrap_or_default() {
        "ThreeDgs" => "spatial_engine",
        "Audio" | "AiImage" | "AiVideo" => "ai_media",
        "Agent" => "orchestration",
        "Software" => integration_software_lane(provider_id),
        _ => "ai_media",
    }
}

fn integration_software_lane(adapter_id: &str) -> &'static str {
    match adapter_id {
        "comfyui" => "ai_media",
        "unreal" | "unity" | "blender" => "spatial_engine",
        "resolve" | "nuke" | "editing-suite" => "post_output",
        "touchdesigner" | "madmapper" | "motion-db" => "interactive_systems",
        "hermes" => "orchestration",
        _ => "orchestration",
    }
}

fn integration_provider_next_action(
    status: &str,
    configured: bool,
    retry_command: Option<&str>,
    set_key_command: &str,
    health_command: &str,
    run_mock_command: &str,
    conformance_command: &str,
) -> Value {
    match status {
        "ready" => integration_next_action(
            "verify_production",
            "归档生产证据",
            conformance_command,
            "本地账本已有成功记录，下一步确认真实上游证据与交接包。",
        ),
        "needs_attention" => integration_next_action(
            "recover_failure",
            "处理失败任务",
            retry_command.unwrap_or(conformance_command),
            "已有失败任务或动作，优先重试或生成交接包给负责人处理。",
        ),
        "needs_execution" if configured => integration_next_action(
            "run_provider_smoke",
            "执行 Provider smoke",
            run_mock_command,
            "凭证或请求账本已存在，下一步运行 Provider smoke 形成成功记录。",
        ),
        "needs_execution" => integration_next_action(
            "check_provider_health",
            "检查 Provider health",
            health_command,
            "已有请求账本但缺少成功记录，先检查 Provider 控制路径。",
        ),
        _ => integration_next_action(
            "configure_key",
            "配置 Provider Key",
            set_key_command,
            "Provider 还没有可用凭证或运行证据。",
        ),
    }
}

fn integration_software_next_action(
    status: &str,
    retry_command: Option<&str>,
    run_command: &str,
    conformance_command: &str,
) -> Value {
    match status {
        "ready" => integration_next_action(
            "verify_production",
            "归档软件证据",
            conformance_command,
            "本地账本已有成功控制记录，下一步确认真实软件侧证据与交接包。",
        ),
        "needs_attention" => integration_next_action(
            "recover_failure",
            "处理软件失败",
            retry_command.unwrap_or(conformance_command),
            "已有失败软件任务，优先重试或交给软件负责人处理。",
        ),
        _ => integration_next_action(
            "run_software_smoke",
            "执行软件 smoke",
            run_command,
            "软件 adapter 尚无成功控制记录，下一步运行最小动作 smoke。",
        ),
    }
}

fn integration_agent_next_action(status: &str) -> Value {
    if status == "ready" {
        integration_next_action(
            "verify_handoff",
            "归档 Agent 交接",
            "pool-cli --project <slug> agent-conformance-package all --output-dir worlds/<slug>/output",
            "Agent/Hermes 已有会话与 transcript，下一步归档 conformance package。",
        )
    } else {
        integration_next_action(
            "stage_agent_session",
            "创建 Agent 会话",
            "pool-cli --project <slug> agent-session agent-cli --command-id workflow-context --command \"pool-cli --project <slug> workflow-context\" --tool cli --tool mcp",
            "Agent/Hermes 尚无可审计 transcript，下一步创建本地受控会话。",
        )
    }
}

fn integration_next_action(kind: &str, label: &str, command: &str, reason: &str) -> Value {
    json!({
        "kind": kind,
        "label": label,
        "command": command,
        "reason": reason,
    })
}

fn integration_team_lanes(
    providers: &[Value],
    software_adapters: &[Value],
    agent: &Value,
) -> Vec<Value> {
    integration_lane_catalog()
        .into_iter()
        .map(|(lane_id, title, owner)| {
            let rows = providers
                .iter()
                .chain(software_adapters.iter())
                .filter(|row| row["lane"].as_str() == Some(lane_id))
                .collect::<Vec<_>>();
            let agent_in_lane = agent["lane"].as_str() == Some(lane_id);
            let total = rows.len() + usize::from(agent_in_lane);
            let ready = rows
                .iter()
                .filter(|row| row["status"].as_str() == Some("ready"))
                .count()
                + usize::from(agent_in_lane && agent["status"].as_str() == Some("ready"));
            let needs_attention = rows
                .iter()
                .filter(|row| row["status"].as_str() == Some("needs_attention"))
                .count()
                + usize::from(agent_in_lane && agent["status"].as_str() == Some("needs_attention"));
            let targets = rows
                .iter()
                .filter_map(|row| {
                    row.get("provider_id")
                        .or_else(|| row.get("adapter_id"))
                        .and_then(Value::as_str)
                })
                .chain(if agent_in_lane { Some("agent") } else { None })
                .collect::<Vec<_>>();

            json!({
                "lane": lane_id,
                "title": title,
                "owner": owner,
                "total": total,
                "ready": ready,
                "needs_attention": needs_attention,
                "targets": targets,
            })
        })
        .collect()
}

fn integration_run_plan(
    providers: &[Value],
    software_adapters: &[Value],
    agent: &Value,
) -> Vec<Value> {
    let mut actions = providers
        .iter()
        .filter_map(|row| integration_run_plan_item("provider", "provider_id", row))
        .chain(
            software_adapters
                .iter()
                .filter_map(|row| integration_run_plan_item("software", "adapter_id", row)),
        )
        .collect::<Vec<_>>();
    if let Some(agent_action) = integration_run_plan_item("agent", "agent_id", agent) {
        actions.push(agent_action);
    }
    actions.sort_by(|left, right| {
        let left_key = (
            left["priority"].as_u64().unwrap_or(99),
            left["lane"].as_str().unwrap_or_default(),
            left["target_id"].as_str().unwrap_or_default(),
        );
        let right_key = (
            right["priority"].as_u64().unwrap_or(99),
            right["lane"].as_str().unwrap_or_default(),
            right["target_id"].as_str().unwrap_or_default(),
        );
        left_key.cmp(&right_key)
    });
    actions
}

fn integration_run_plan_item(kind: &str, id_field: &str, row: &Value) -> Option<Value> {
    let status = row["status"].as_str().unwrap_or_default();
    if status == "ready" {
        return None;
    }
    let next_action = row.get("next_action")?;
    let target_id = if kind == "agent" {
        "agent".to_string()
    } else {
        row[id_field].as_str()?.to_string()
    };
    Some(json!({
        "priority": integration_status_priority(status),
        "lane": row["lane"].as_str().unwrap_or("orchestration"),
        "target_kind": kind,
        "target_id": target_id,
        "display_name": row["display_name"].as_str().unwrap_or(target_id.as_str()),
        "status": status,
        "action": next_action,
    }))
}

fn integration_status_priority(status: &str) -> u64 {
    match status {
        "needs_attention" => 1,
        "needs_configuration" => 2,
        "needs_execution" => 3,
        _ => 9,
    }
}

fn integration_lane_catalog() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        (
            "orchestration",
            "制片 / Agent 编排",
            "Producer + Agent operator",
        ),
        ("ai_media", "AI 素材生成", "AI image/video/audio operator"),
        ("spatial_engine", "3D / 引擎组装", "3DGS + engine operator"),
        ("post_output", "视频 / 后期输出", "Editor + compositor"),
        (
            "interactive_systems",
            "交互 / 现场系统",
            "Interactive systems operator",
        ),
    ]
}

fn verification_succeeded(value: &Value) -> bool {
    value
        .get("status")
        .and_then(Value::as_str)
        .is_some_and(|status| matches!(status, "succeeded" | "Succeeded" | "ok" | "ready"))
        || value.get("ok").and_then(Value::as_bool) == Some(true)
}

fn provider_aliases() -> Value {
    json!({
        "world-labs-marble": "worldlabs-marble",
        "worldlabs": "worldlabs-marble",
        "marble": "worldlabs-marble",
        "triposplat": "tripo-splat",
        "tripo": "tripo-splat",
        "spark": "spark-3dgs",
        "qunhe": "qunhe-3d",
        "qunhe-tech": "qunhe-3d",
        "openai": "openai-image-2",
        "openai-image": "openai-image-2",
        "image-2": "openai-image-2",
        "mj": "midjourney",
        "nanobanana": "nano-banana-pro",
        "nano-banana": "nano-banana-pro",
        "nanobananapro": "nano-banana-pro",
    })
}

pub fn runtime_workflow_context_resource(
    snapshot: &RuntimeSnapshot,
    workflow_id: &str,
) -> Result<Value> {
    let workflow_id = workflow_id.trim();
    if workflow_id.is_empty() {
        bail!("workflow id is required for pool://workflow/<workflow-id>");
    }

    let Some(workflow) = snapshot
        .workflows
        .iter()
        .find(|workflow| workflow.id == workflow_id)
    else {
        bail!("unknown workflow: {workflow_id}");
    };

    let nodes: BTreeMap<String, WorkflowNode> = serde_json::from_value(workflow.nodes.clone())
        .with_context(|| format!("parse workflow nodes for {}", workflow.id))?;
    let node_ids = nodes.keys().cloned().collect::<BTreeSet<_>>();
    let tasks = snapshot
        .tasks
        .iter()
        .filter(|task| {
            task.node_id
                .as_deref()
                .is_some_and(|node_id| node_ids.contains(node_id))
        })
        .collect::<Vec<_>>();
    let task_ids = tasks
        .iter()
        .map(|task| task.id.clone())
        .collect::<BTreeSet<_>>();
    let assets = snapshot
        .assets
        .iter()
        .filter(|asset| {
            asset
                .source_node_id
                .as_deref()
                .is_some_and(|node_id| node_ids.contains(node_id))
        })
        .collect::<Vec<_>>();
    let provider_requests = snapshot
        .provider_requests
        .iter()
        .filter(|request| task_ids.contains(&request.task_id))
        .collect::<Vec<_>>();
    let software_actions = snapshot
        .software_actions
        .iter()
        .filter(|action| {
            action
                .task_id
                .as_deref()
                .is_some_and(|task_id| task_ids.contains(task_id))
        })
        .collect::<Vec<_>>();
    let node_states = snapshot
        .node_states
        .iter()
        .filter(|state| node_ids.contains(&state.node_id))
        .collect::<Vec<_>>();
    let agent_sessions = agent_sessions_for_workflow(snapshot, workflow, &nodes, &tasks);
    let graph = runtime_graph_workflow(snapshot, workflow)?;

    Ok(json!({
        "project_filter": snapshot.project_filter,
        "generated_at": snapshot.generated_at,
        "workflow_id": workflow.id.clone(),
        "workflow": {
            "id": workflow.id.clone(),
            "project_id": workflow.project_id.clone(),
            "shot_id": workflow.shot_id.clone(),
            "name": workflow.name.clone(),
            "created_at": workflow.created_at.clone(),
            "updated_at": workflow.updated_at.clone(),
        },
        "graph": graph,
        "node_states": node_states,
        "tasks": tasks,
        "assets": assets,
        "provider_requests": provider_requests,
        "software_actions": software_actions,
        "agent_sessions": agent_sessions,
        "summary": {
            "nodes": node_ids.len(),
            "tasks": tasks.len(),
            "assets": assets.len(),
            "provider_requests": provider_requests.len(),
            "software_actions": software_actions.len(),
            "agent_sessions": agent_sessions.len(),
            "node_states": node_states.len(),
            "waiting_approval": tasks
                .iter()
                .filter(|task| task.status == "WaitingApproval")
                .count(),
            "running": tasks
                .iter()
                .filter(|task| task.status == "Running")
                .count(),
            "failed": tasks
                .iter()
                .filter(|task| task.status == "Failed")
                .count(),
            "blocked_by_approval": tasks
                .iter()
                .any(|task| task.requires_approval || task.status == "WaitingApproval"),
        },
    }))
}

pub fn runtime_budget_resource(snapshot: &RuntimeSnapshot) -> Value {
    let configured_provider_keys = snapshot
        .api_keys
        .iter()
        .filter(|key| key.service_type == "provider" && key.configured)
        .map(|key| key.provider.clone())
        .collect::<BTreeSet<_>>();
    let mut provider_ids = configured_provider_keys.clone();
    provider_ids.extend(
        snapshot
            .tasks
            .iter()
            .filter_map(|task| task.provider_id.clone()),
    );
    provider_ids.extend(
        snapshot
            .provider_requests
            .iter()
            .map(|request| request.provider_id.clone()),
    );

    let provider_credentials = provider_ids
        .iter()
        .map(|provider_id| {
            let api_key = snapshot
                .api_keys
                .iter()
                .find(|key| key.provider == *provider_id && key.service_type == "provider");
            let provider_tasks = snapshot
                .tasks
                .iter()
                .filter(|task| task.provider_id.as_deref() == Some(provider_id.as_str()))
                .collect::<Vec<_>>();
            let requests = snapshot
                .provider_requests
                .iter()
                .filter(|request| request.provider_id == *provider_id)
                .collect::<Vec<_>>();
            let token_estimate = provider_tasks
                .iter()
                .map(|task| task.cost_estimate_tokens)
                .sum::<u64>();
            let waiting_approval_tokens = provider_tasks
                .iter()
                .filter(|task| task.status == "WaitingApproval")
                .map(|task| task.cost_estimate_tokens)
                .sum::<u64>();
            let configured = api_key.is_some_and(|key| key.configured);

            json!({
                "provider_id": provider_id,
                "configured": configured,
                "key_hint": api_key.and_then(|key| key.key_hint.clone()),
                "task_count": provider_tasks.len(),
                "provider_request_count": requests.len(),
                "token_estimate": token_estimate,
                "waiting_approval_tokens": waiting_approval_tokens,
                "last_request_at": requests.first().map(|request| request.created_at.clone()),
                "credential_status": if configured { "configured" } else { "not_recorded" },
            })
        })
        .collect::<Vec<_>>();
    let missing_runtime_credentials = provider_credentials
        .iter()
        .filter(|provider| {
            !provider
                .get("configured")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
        .count();
    let approval_gates = snapshot
        .tasks
        .iter()
        .filter(|task| task.requires_approval || task.status == "WaitingApproval")
        .collect::<Vec<_>>();

    json!({
        "project_filter": snapshot.project_filter,
        "generated_at": snapshot.generated_at,
        "summary": {
            "task_estimated_tokens": snapshot.stats.task_estimated_tokens,
            "waiting_approval_estimated_tokens": snapshot.stats.waiting_approval_estimated_tokens,
            "agent_token_used": snapshot.stats.agent_token_used,
            "agent_token_budget": snapshot.stats.agent_token_budget,
            "token_total": snapshot.stats.token_total,
            "budget_remaining": snapshot.stats.budget_remaining,
            "configured_api_keys": configured_provider_keys.len(),
            "tracked_providers": provider_credentials.len(),
            "missing_runtime_credentials": missing_runtime_credentials,
            "provider_requests": snapshot.provider_requests.len(),
            "approval_gates": approval_gates.len(),
        },
        "provider_credentials": provider_credentials,
        "approval_gates": approval_gates,
        "provider_requests_by_provider": count_by(
            snapshot
                .provider_requests
                .iter()
                .map(|request| request.provider_id.as_str())
        ),
    })
}

pub fn runtime_prd_readiness_resource(snapshot: &RuntimeSnapshot) -> Result<Value> {
    let provider_registry = ProviderRegistry::defaults();
    let software_registry = SoftwareAdapterRegistry::defaults();
    let graph = runtime_graph_resource(snapshot)?;
    let preflight = runtime_preflight_resource(snapshot)?;
    let budget = runtime_budget_resource(snapshot);
    let handoff = runtime_handoff_resource(snapshot)?;
    let output_catalog = output_package_catalog_resource(snapshot);
    let provider_configs = provider_registry.configs();
    let software_configs = software_registry.configs();
    let provider_ids = provider_configs
        .iter()
        .map(|config| config.id.clone())
        .collect::<Vec<_>>();
    let three_dgs_providers = provider_configs
        .iter()
        .filter(|config| format!("{:?}", config.kind) == "ThreeDgs")
        .map(|config| config.id.clone())
        .collect::<Vec<_>>();
    let ai_media_providers = provider_configs
        .iter()
        .filter(|config| {
            matches!(
                format!("{:?}", config.kind).as_str(),
                "AiImage" | "AiVideo" | "Audio"
            )
        })
        .map(|config| config.id.clone())
        .collect::<Vec<_>>();
    let software_ids = software_configs
        .iter()
        .map(|config| config.id.clone())
        .collect::<Vec<_>>();
    let graph_summary = graph.get("summary").cloned().unwrap_or_else(|| json!({}));
    let preflight_summary = preflight
        .get("summary")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let handoff_summary = handoff.get("summary").cloned().unwrap_or_else(|| json!({}));
    let budget_summary = budget.get("summary").cloned().unwrap_or_else(|| json!({}));
    let output_ready = output_catalog.summary.ready_targets == output_catalog.summary.total_targets
        && output_catalog.summary.total_targets > 0;
    let expected_outputs = output_catalog
        .policy
        .expected_targets
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let desktop_contract = desktop_recognition_contract_resource();
    let provider_contracts = provider_contracts_resource(None)?;
    let software_contracts = software_control_contracts_resource(None)?;
    let provider_evidence = provider_evidence_matrix(snapshot);
    let provider_evidence_summary = provider_evidence
        .get("summary")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let provider_gateway_ready = provider_evidence_summary
        .get("gateway_profile_ready")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let provider_production_ready = provider_evidence_summary
        .get("production_upstream_ready")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let provider_missing_gateway = string_array_at(
        &provider_evidence_summary,
        "missing_gateway_profile_success",
    );
    let provider_missing_production = string_array_at(
        &provider_evidence_summary,
        "missing_production_upstream_success",
    );
    let software_evidence = software_evidence_matrix(snapshot);
    let software_evidence_summary = software_evidence
        .get("summary")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let software_control_ready = software_evidence_summary
        .get("control_profile_ready")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let software_production_ready = software_evidence_summary
        .get("production_software_ready")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let software_missing_control = string_array_at(
        &software_evidence_summary,
        "missing_control_profile_success",
    );
    let software_missing_production = string_array_at(
        &software_evidence_summary,
        "missing_production_software_success",
    );
    let desktop_vision_evidence = desktop_vision_evidence(snapshot);
    let desktop_vision_summary = desktop_vision_evidence
        .get("summary")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let desktop_controller_callback_ready = desktop_vision_summary
        .get("controller_callback_ready")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let desktop_vision_trace_ready = desktop_vision_summary
        .get("vision_trace_ready")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let desktop_external_visual_model_ready = desktop_vision_summary
        .get("external_visual_model_ready")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let production_hardening_ready = provider_production_ready
        && software_production_ready
        && desktop_controller_callback_ready
        && desktop_vision_trace_ready
        && desktop_external_visual_model_ready;

    let requirements = vec![
        prd_requirement(
            "reference_architecture",
            "ROCKCHENWEI/Pool + image-blaster reference architecture",
            if snapshot.projects.is_empty() || snapshot.workflows.is_empty() {
                "partial"
            } else {
                "ready"
            },
            "Rust shared-core, SQLite runtime, workflow graph, and image-blaster local project envelope are represented in runtime state.",
            json!({
                "projects": snapshot.projects.len(),
                "workflows": snapshot.workflows.len(),
                "assets": snapshot.assets.len(),
                "mcp_resources": default_resources().len(),
                "local_first_policy": runtime_adapter_catalog_resource()["policy"],
            }),
            if snapshot.projects.is_empty() || snapshot.workflows.is_empty() {
                vec!["No project/workflow has been materialized in the current snapshot."]
            } else {
                Vec::<&str>::new()
            },
            vec!["Run cargo run -p pool-core --example run_prd_readiness_smoke, persist_default_plan, run-workflow, or connect an existing SQLite runtime."],
        ),
        prd_requirement(
            "node_graph_execution",
            "Node graph as executable plan",
            if snapshot.workflows.is_empty() {
                "partial"
            } else {
                "ready"
            },
            "Runtime graph, execution plan, node context, task queue, approval gates, and handoff resources are available.",
            json!({
                "runtime_graph_summary": graph_summary,
                "preflight_summary": preflight_summary,
                "handoff_summary": handoff_summary,
                "tasks": snapshot.tasks.len(),
                "events": snapshot.events.len(),
            }),
            if snapshot.workflows.is_empty() {
                vec!["No workflow graph is present in the current snapshot."]
            } else {
                Vec::<&str>::new()
            },
            vec!["Use /api/runtime-execution-plan/run-next or pool-cli runtime-run-next to advance the plan."],
        ),
        prd_requirement(
            "creative_input_to_agent_control",
            "Creative input, Hermes, Agent CLI, MCP and token control",
            if snapshot.agent_sessions.is_empty() {
                "partial"
            } else {
                "ready"
            },
            "Agent session runner, Hermes HTTP path, Agent CLI allowlist execution, MCP resources/prompts, API key status, and token budget summaries are exposed.",
            json!({
                "agent_sessions": snapshot.agent_sessions.len(),
                "api_keys": snapshot.api_keys.len(),
                "budget_summary": budget_summary,
                "mcp_prompts": ["pool_content_burst", "pool_3dgs_review", "pool_software_handoff", "pool_desktop_recognition_handoff"],
            }),
            if snapshot.agent_sessions.is_empty() {
                vec!["No Agent/Hermes session has been staged or executed in this snapshot."]
            } else {
                Vec::<&str>::new()
            },
            vec!["Run cargo run -p pool-core --example run_prd_readiness_smoke, pool-cli agent-session, or /api/workflow-runs to create an auditable Agent session."],
        ),
        prd_requirement(
            "ai_media_and_3dgs_providers",
            "AI image/video/audio and 2D/3D/3DGS provider adapters",
            if provider_production_ready {
                "ready"
            } else {
                "partial"
            },
            "Native ComfyUI/Kling/OpenAI image adapters, generic AI media gateway, 3DGS gateway, provider contracts, gateway worker, and mock gateway are modeled.",
            json!({
                "providers": provider_ids,
                "ai_media_providers": ai_media_providers,
                "three_dgs_providers": three_dgs_providers,
                "provider_requests": snapshot.provider_requests.len(),
                "provider_contracts_kind": provider_contracts["kind"],
                "provider_evidence": provider_evidence,
            }),
            provider_evidence_gaps(
                provider_gateway_ready,
                &provider_missing_gateway,
                provider_production_ready,
                &provider_missing_production,
            ),
            vec![
                "Run pool-cli production-evidence-provider-matrix --no-env to inspect the Provider matrix without touching configured endpoints.".to_string(),
                format!(
                    "{} against real upstream Provider services, or use POOL_PROVIDER_PRODUCTION_ATTESTATION as a global fallback.",
                    production_evidence_provider_matrix_command(
                        "demo",
                        "<output-root>",
                        "<provider-production-evidence-bundle.json>"
                    )
                ),
                "Store credentials with /api/api-keys or pass them through provider run requests.".to_string(),
            ],
        ),
        prd_requirement(
            "external_software_control",
            "External software control with API/MCP, CLI, desktop fallback, and human takeover",
            if software_control_ready {
                "ready"
            } else {
                "partial"
            },
            "Software registry, contracts, SoftwareActionRunner, Unreal/Hermes MCP adapters, CommandSoftwareAdapter, desktop recognition handoff, and output-result bridge are exposed.",
            json!({
                "software_adapters": software_ids,
                "software_actions": snapshot.software_actions.len(),
                "software_contracts_kind": software_contracts["kind"],
                "desktop_contract_kind": desktop_contract["kind"],
                "control_priority": ["API/MCP", "Skills/CLI", "Desktop Recognition", "Human Takeover"],
                "software_evidence": software_evidence,
            }),
            software_evidence_gaps(
                software_control_ready,
                &software_missing_control,
                software_production_ready,
                &software_missing_production,
            ),
            vec![
                "Run pool-cli production-evidence-software-matrix --no-env to inspect the software matrix without touching configured software endpoints or commands.",
                "Run pool-cli --project demo production-evidence-software-matrix <output-root> --production-software with --software-endpoint-env / --software-command-env / --software-artifacts-env / --software-attestation-env per adapter, or provide equivalent POOL_* env vars.",
                "Use pool_output_result in software action payloads to bind successful actions to output manifests.",
            ],
        ),
        prd_requirement(
            "unreal_assembly_priority",
            "Unreal-first scene assembly and runtime viewport",
            if has_software_action_for(snapshot, "unreal") {
                "ready"
            } else {
                "partial"
            },
            "Unreal is registered as the first deep software adapter and can run through Unreal MCP or local mock fallback.",
            json!({
                "unreal_contract": software_control_contracts_resource(Some("unreal"))?,
                "unreal_actions": snapshot.software_actions.iter().filter(|action| action.adapter_id == "unreal").count(),
            }),
            if has_software_action_for(snapshot, "unreal") {
                Vec::<&str>::new()
            } else {
                vec!["No Unreal software action is recorded in the current snapshot."]
            },
            vec!["Run cargo run -p pool-core --example run_prd_readiness_smoke, pool-cli run-software unreal, or /api/workflow-runs with unreal_mode enabled."],
        ),
        prd_requirement(
            "three_output_targets",
            "Video, game, and interactive-art output packages",
            if output_ready { "ready" } else { "partial" },
            "OutputPackageRunner generates indexed video timeline, game build, and interactive cue manifests and can record execution results.",
            json!({
                "expected_targets": expected_outputs,
                "summary": output_catalog.summary,
                "deliverables": output_catalog.deliverables,
            }),
            if output_ready {
                Vec::<&str>::new()
            } else {
                vec!["One or more output deliverable manifests are missing or not locally readable in this snapshot."]
            },
            vec!["Run cargo run -p pool-core --example run_prd_readiness_smoke, pool-cli output-package, or /api/output-packages, then record final software results."],
        ),
        prd_requirement(
            "five_person_team_handoff",
            "Five-person content burst team handoff",
            "ready",
            "Runtime handoff resource assigns Creative Director, Agent Operator, AI/3DGS TD, Engine Integrator, and Output Operator lanes.",
            json!({
                "team": handoff.get("team").cloned().unwrap_or_else(|| json!({})),
                "lanes": handoff.get("lanes").cloned().unwrap_or_else(|| json!([])),
            }),
            Vec::<&str>::new(),
            vec!["Use pool-cli handoff-package to export an offline handoff bundle."],
        ),
        prd_requirement(
            "web_console",
            "Chinese Web runtime console",
            "ready",
            "Web prototype is wired to runtime snapshot/discovery/graph/preflight/handoff/PRD readiness/provider contracts/software contracts/output package controls.",
            json!({
                "runtime_endpoints": ["/api/snapshot", "/api/discovery", "/api/runtime-graph", "/api/runtime-execution-plan", "/api/runtime-budget", "/api/runtime-preflight", "/api/runtime-handoff", "/api/prd-readiness", "/api/production-evidence/requirements", "/api/output-packages"],
                "project_filter": snapshot.project_filter,
            }),
            Vec::<&str>::new(),
            vec!["Open apps/web-prototype with ?runtime=local or ?runtime=http://127.0.0.1:4788."],
        ),
        prd_requirement(
            "production_hardening",
            "Production hardening for real deployment",
            if production_hardening_ready {
                "ready"
            } else {
                "partial"
            },
            "The local-first runtime scaffold is verifiable, but several production integrations intentionally remain external or credential-dependent.",
            json!({
                "credential_storage": "legacy SQLite compatibility, POOL_CREDENTIAL_PASSPHRASE AES-256-GCM wrapper, and optional macOS Keychain backend via POOL_CREDENTIAL_STORE=keychain",
                "desktop_recognition": "contract, queue, dry-run, AppleScript execution mode, external vision trace bridge, and callback implemented",
                "desktop_vision_evidence": desktop_vision_evidence,
                "real_provider_execution": {
                    "adapter_gateway_contracts": "implemented",
                    "gateway_profile_ready": provider_gateway_ready,
                    "production_upstream_ready": provider_production_ready,
                    "missing_production_upstream_success": provider_missing_production,
                },
                "real_software_execution": {
                    "adapter_contracts": "implemented",
                    "control_profile_ready": software_control_ready,
                    "production_software_ready": software_production_ready,
                    "missing_production_software_success": software_missing_production,
                },
            }),
            production_hardening_gaps(
                desktop_controller_callback_ready,
                desktop_vision_trace_ready,
                desktop_external_visual_model_ready,
                provider_production_ready,
                software_production_ready,
            ),
            vec![
                "Connect a real visual desktop recognition controller that writes Pool-compatible trace JSON.",
                "Run cargo run -p pool-core --example run_desktop_vision_trace_smoke to record local trace/callback evidence.",
                "Run authenticated provider/software E2E tests and attach evidence.",
                "Use POOL_CREDENTIAL_STORE=keychain for local secrets that should not be stored in SQLite.",
            ],
        ),
    ];
    let summary = prd_readiness_summary(&requirements);
    let overall_status = if summary.get("blocked").and_then(Value::as_u64).unwrap_or(0) > 0 {
        "blocked"
    } else if summary.get("partial").and_then(Value::as_u64).unwrap_or(0) > 0 {
        "partial"
    } else {
        "ready"
    };
    let project_slug = snapshot.project_filter.as_deref().unwrap_or("<slug>");
    let completion_gate =
        prd_completion_gate(project_slug, &requirements, &summary, overall_status);

    Ok(json!({
        "kind": "pool_prd_readiness",
        "version": 1,
        "project_filter": snapshot.project_filter,
        "generated_at": snapshot.generated_at,
        "overall_status": overall_status,
        "summary": summary,
        "completion_gate": completion_gate,
        "requirements": requirements,
        "source_resources": [
            "pool://status",
            "pool://runtime-graph",
            "pool://runtime-execution-plan",
            "pool://runtime-budget",
            "pool://runtime-preflight",
            "pool://runtime-handoff",
            "pool://provider-contracts",
            "pool://software-contracts",
            "pool://desktop-recognition",
            "pool://output-packages",
            "pool://agent-sessions",
            "pool://api-keys"
        ],
    }))
}

pub fn runtime_prd_completion_gate_resource(snapshot: &RuntimeSnapshot) -> Result<Value> {
    let readiness = runtime_prd_readiness_resource(snapshot)?;
    Ok(json!({
        "kind": "pool_prd_completion_gate",
        "project_filter": readiness.get("project_filter").cloned().unwrap_or(Value::Null),
        "overall_status": readiness.get("overall_status").cloned().unwrap_or_else(|| json!("unknown")),
        "summary": readiness.get("summary").cloned().unwrap_or_else(|| json!({})),
        "completion_gate": readiness
            .get("completion_gate")
            .cloned()
            .unwrap_or_else(|| json!({
                "status": "unknown",
                "ready_for_completion": false
            })),
    }))
}

pub fn runtime_production_evidence_requirements_resource(
    snapshot: &RuntimeSnapshot,
) -> Result<Value> {
    let provider_evidence = provider_evidence_matrix(snapshot);
    let provider_summary = provider_evidence
        .get("summary")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let missing_provider_gateway =
        string_array_at(&provider_summary, "missing_gateway_profile_success");
    let missing_provider_production =
        string_array_at(&provider_summary, "missing_production_upstream_success");
    let provider_gateway_ready = provider_summary
        .get("gateway_profile_ready")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let provider_production_ready = provider_summary
        .get("production_upstream_ready")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let software_evidence = software_evidence_matrix(snapshot);
    let software_summary = software_evidence
        .get("summary")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let missing_software_control =
        string_array_at(&software_summary, "missing_control_profile_success");
    let missing_software_production =
        string_array_at(&software_summary, "missing_production_software_success");
    let software_control_ready = software_summary
        .get("control_profile_ready")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let software_production_ready = software_summary
        .get("production_software_ready")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let desktop_vision = desktop_vision_evidence(snapshot);
    let desktop_summary = desktop_vision
        .get("summary")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let desktop_missing = production_desktop_vision_missing(&desktop_summary);
    let desktop_ready = desktop_missing.is_empty();
    let complete = provider_production_ready && software_production_ready && desktop_ready;
    let missing_total = missing_provider_production.len()
        + missing_software_production.len()
        + desktop_missing.len();
    let evidence_tasks = production_evidence_task_rows(
        snapshot,
        &missing_provider_production,
        &missing_software_production,
        &desktop_missing,
    );

    Ok(json!({
        "kind": "pool_production_evidence_requirements",
        "version": 1,
        "project_filter": snapshot.project_filter,
        "generated_at": snapshot.generated_at,
        "overall_status": if complete { "ready" } else { "partial" },
        "summary": {
            "complete": complete,
            "missing_total": missing_total,
            "provider_gateway_ready": provider_gateway_ready,
            "provider_production_ready": provider_production_ready,
            "software_control_ready": software_control_ready,
            "software_production_ready": software_production_ready,
            "desktop_vision_ready": desktop_ready,
            "missing_provider_gateway_profile_success": missing_provider_gateway,
            "missing_provider_production_upstream_success": missing_provider_production,
            "missing_software_control_profile_success": missing_software_control,
            "missing_software_production_success": missing_software_production,
            "missing_desktop_vision": desktop_missing,
        },
        "evidence_contract": {
            "provider_runtime_record": {
                "execution_mode": "gateway or adapter",
                "response_status": "Succeeded",
                "request_evidence": {
                    "production_upstream": true,
                    "local_mock_gateway": false,
                },
            },
            "software_runtime_record": {
                "priority": "ApiMcp, SkillsCli, DesktopRecognition, or HumanTakeover",
                "verification": "ok:true or desktop_recognition_status:succeeded",
                "payload_json_evidence": {
                    "production_software": true,
                    "local_mock_software": false,
                    "production_attestation": "real software plugin/API/MCP/CLI/desktop-control run attestation",
                },
            },
            "desktop_vision_record": {
                "controller_callback": "desktop_recognition_status:succeeded",
                "trace": "screen_trace_path or controller_result.vision_trace_path",
                "visual_model": "external visual model evidence, not local trace smoke only",
            },
            "local_file_policy": [
                "providers[].artifacts and providers[].metadata_path must resolve to local files before import.",
                "Remote provider URLs are provenance only and are not accepted as front-end load paths.",
                "Desktop vision traces must be written as local JSON files and marked external.",
            ],
        },
        "required_providers": production_provider_requirement_rows(&provider_evidence),
        "required_software": production_software_requirement_rows(&software_evidence),
        "required_desktop_vision": production_desktop_vision_requirement(&desktop_vision),
        "evidence_tasks": {
            "summary": {
                "total": evidence_tasks.len(),
                "provider_tasks": missing_provider_production.len(),
                "software_tasks": missing_software_production.len(),
                "desktop_vision_tasks": desktop_missing.len(),
            },
            "tasks": evidence_tasks,
        },
        "commands": {
            "template": "pool-cli --project <slug> production-evidence-template --output-root <root> <bundle.json>",
            "merge": "pool-cli --project <slug> merge-production-evidence <combined-bundle.json> <bundle-a.json> <bundle-b.json>...",
            "closeout": "pool-cli --project <slug> closeout-production-evidence --output <merged-bundle.json> <bundle-a.json> <bundle-b.json>...",
            "validate": "pool-cli --project <slug> validate-production-evidence <bundle.json>",
            "import": "pool-cli --project <slug> import-production-evidence <bundle.json>",
            "readiness": "pool-cli --project <slug> prd-readiness",
        },
        "source_resources": [
            "pool://prd-readiness",
            "pool://provider-requests",
            "pool://software-actions",
            "pool://desktop-recognition",
            "pool://provider-contracts",
            "pool://software-contracts",
            "pool://desktop-recognition-contract"
        ],
    }))
}

fn production_evidence_software_matrix_command(
    project_slug: &str,
    output_root: &str,
    software_bundle: &str,
) -> String {
    format!(
        "pool-cli --project {project_slug} production-evidence-software-matrix {output_root}/software-evidence-matrix --production-software --software-endpoint-env unreal=POOL_UNREAL_MCP_ENDPOINT --software-endpoint-env blender=POOL_BLENDER_ENDPOINT --software-endpoint-env comfyui=POOL_COMFYUI_ENDPOINT --software-endpoint-env resolve=POOL_RESOLVE_ENDPOINT --software-endpoint-env unity=POOL_UNITY_ENDPOINT --software-endpoint-env touchdesigner=POOL_TOUCHDESIGNER_ENDPOINT --software-endpoint-env madmapper=POOL_MADMAPPER_ENDPOINT --software-endpoint-env nuke=POOL_NUKE_ENDPOINT --software-endpoint-env motion-db=POOL_MOTION_DB_ENDPOINT --software-endpoint-env editing-suite=POOL_EDITING_SUITE_ENDPOINT --software-endpoint-env hermes=POOL_HERMES_MCP_ENDPOINT --software-command-env blender=POOL_BLENDER_COMMAND --software-command-env comfyui=POOL_COMFYUI_COMMAND --software-command-env resolve=POOL_RESOLVE_COMMAND --software-command-env unity=POOL_UNITY_COMMAND --software-command-env touchdesigner=POOL_TOUCHDESIGNER_COMMAND --software-command-env madmapper=POOL_MADMAPPER_COMMAND --software-command-env nuke=POOL_NUKE_COMMAND --software-command-env motion-db=POOL_MOTION_DB_COMMAND --software-command-env editing-suite=POOL_EDITING_SUITE_COMMAND --software-command-env hermes=POOL_HERMES_COMMAND --software-artifacts-env unreal=POOL_UNREAL_ARTIFACTS --software-artifacts-env blender=POOL_BLENDER_ARTIFACTS --software-artifacts-env comfyui=POOL_COMFYUI_ARTIFACTS --software-artifacts-env resolve=POOL_RESOLVE_ARTIFACTS --software-artifacts-env unity=POOL_UNITY_ARTIFACTS --software-artifacts-env touchdesigner=POOL_TOUCHDESIGNER_ARTIFACTS --software-artifacts-env madmapper=POOL_MADMAPPER_ARTIFACTS --software-artifacts-env nuke=POOL_NUKE_ARTIFACTS --software-artifacts-env motion-db=POOL_MOTION_DB_ARTIFACTS --software-artifacts-env editing-suite=POOL_EDITING_SUITE_ARTIFACTS --software-artifacts-env hermes=POOL_HERMES_ARTIFACTS --software-attestation-env unreal=POOL_UNREAL_PRODUCTION_ATTESTATION --software-attestation-env blender=POOL_BLENDER_PRODUCTION_ATTESTATION --software-attestation-env comfyui=POOL_COMFYUI_PRODUCTION_ATTESTATION --software-attestation-env resolve=POOL_RESOLVE_PRODUCTION_ATTESTATION --software-attestation-env unity=POOL_UNITY_PRODUCTION_ATTESTATION --software-attestation-env touchdesigner=POOL_TOUCHDESIGNER_PRODUCTION_ATTESTATION --software-attestation-env madmapper=POOL_MADMAPPER_PRODUCTION_ATTESTATION --software-attestation-env nuke=POOL_NUKE_PRODUCTION_ATTESTATION --software-attestation-env motion-db=POOL_MOTION_DB_PRODUCTION_ATTESTATION --software-attestation-env editing-suite=POOL_EDITING_SUITE_PRODUCTION_ATTESTATION --software-attestation-env hermes=POOL_HERMES_PRODUCTION_ATTESTATION --evidence-bundle={software_bundle}"
    )
}

fn production_evidence_desktop_vision_command(
    project_slug: &str,
    output_root: &str,
    desktop_bundle: &str,
) -> String {
    format!(
        "pool-cli --project {project_slug} production-evidence-desktop-vision {output_root}/desktop-vision --production-vision --trace-env POOL_DESKTOP_VISION_TRACE --controller-id-env POOL_DESKTOP_VISION_CONTROLLER_ID --external-action-id-env POOL_DESKTOP_VISION_EXTERNAL_ACTION_ID --production-attestation-env POOL_DESKTOP_VISION_PRODUCTION_ATTESTATION --evidence-bundle={desktop_bundle}"
    )
}

pub fn production_evidence_run_plan_resource(
    project_slug: &str,
    output_root: Option<&str>,
    source: &str,
    snapshot: &RuntimeSnapshot,
) -> Result<Value> {
    let default_output_root = format!("worlds/{project_slug}/output/production-evidence");
    let output_root = output_root.unwrap_or(default_output_root.as_str());
    let requirements = runtime_production_evidence_requirements_resource(snapshot)?;
    let readiness = runtime_prd_readiness_resource(snapshot)?;
    let completion_gate = runtime_prd_completion_gate_resource(snapshot)?;
    let missing_total = requirements
        .pointer("/summary/missing_total")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let task_summary = requirements
        .pointer("/evidence_tasks/summary")
        .cloned()
        .unwrap_or_else(|| json!({}));

    let provider_bundle = format!("{output_root}/provider-production-evidence-bundle.json");
    let software_bundle = format!("{output_root}/software-production-evidence-bundle.json");
    let desktop_bundle = format!("{output_root}/desktop-vision-production-evidence-bundle.json");
    let combined_bundle = format!("{output_root}/combined-production-evidence-bundle.json");
    let merged_bundle = format!("{output_root}/merged-production-evidence-bundle.json");
    let completion_package_dir = format!("worlds/{project_slug}/output/control/prd-completion");
    let software_matrix_command =
        production_evidence_software_matrix_command(project_slug, output_root, &software_bundle);
    let desktop_vision_command =
        production_evidence_desktop_vision_command(project_slug, output_root, &desktop_bundle);
    let provider_gateway_worker_start_commands =
        production_evidence_provider_gateway_worker_start_commands(output_root);
    let software_bridge_worker = production_evidence_generic_software_bridge_worker();
    let software_bridge_worker_start_commands =
        production_evidence_bridge_worker_start_commands(output_root, &software_bridge_worker);

    let ready_for_completion = completion_gate
        .pointer("/completion_gate/ready_for_completion")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let status = if ready_for_completion {
        "complete"
    } else if missing_total == 0 {
        "verify_completion_gate"
    } else {
        "needs_real_production_evidence"
    };
    let provider_tasks = task_summary
        .get("provider_tasks")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let software_tasks = task_summary
        .get("software_tasks")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let desktop_tasks = task_summary
        .get("desktop_vision_tasks")
        .and_then(Value::as_u64)
        .unwrap_or(0);

    Ok(json!({
        "kind": "pool_production_evidence_run_plan",
        "version": 1,
        "project_slug": project_slug,
        "project_filter": snapshot.project_filter,
        "generated_at": snapshot.generated_at,
        "source": source,
        "status": status,
        "ready_for_completion": ready_for_completion,
        "output_root": output_root,
        "summary": {
            "missing_total": missing_total,
            "provider_tasks": provider_tasks,
            "software_tasks": software_tasks,
            "desktop_vision_tasks": desktop_tasks,
            "ready_requirements": readiness.pointer("/summary/ready").cloned().unwrap_or_else(|| json!(0)),
            "partial_requirements": readiness.pointer("/summary/partial").cloned().unwrap_or_else(|| json!(0)),
            "blocked_requirements": readiness.pointer("/summary/blocked").cloned().unwrap_or_else(|| json!(0)),
        },
        "paths": {
            "provider_bundle": provider_bundle,
            "software_bundle": software_bundle,
            "desktop_vision_bundle": desktop_bundle,
            "combined_bundle": combined_bundle,
            "merged_bundle": merged_bundle,
            "completion_package_dir": completion_package_dir,
        },
        "requirements": requirements,
        "completion_gate": completion_gate.get("completion_gate").cloned().unwrap_or_else(|| json!({})),
        "phases": [
            {
                "id": "provider_evidence_matrix",
                "lane": "Provider worker",
                "status": if provider_tasks == 0 { "already_satisfied" } else { "needs_real_gateway" },
                "purpose": "Run the AI media and 3DGS provider matrix against real upstream gateway workers and write providers[] evidence.",
                "required": provider_tasks > 0,
                "outputs": [provider_bundle],
                "command": production_evidence_provider_matrix_command(project_slug, output_root, &provider_bundle),
                "ready_condition": "The gateway endpoints are backed by real Provider SDK/HTTP services, POOL_PROVIDER_PRODUCTION_ATTESTATION or per-provider production attestation env identifies the real upstream worker/SDK run, and every Provider artifact/metadata path is local.",
                "provider_gateway_worker_start_commands": provider_gateway_worker_start_commands
            },
            {
                "id": "software_evidence_matrix",
                "lane": "Software operator",
                "status": if software_tasks == 0 { "already_satisfied" } else { "needs_real_software_control" },
                "purpose": "Run Unreal, Blender, ComfyUI, Resolve, Unity, TouchDesigner, MadMapper, Nuke, mocap database, editing software, and Hermes through real plugin/API/MCP/CLI/control paths.",
                "required": software_tasks > 0,
                "outputs": [software_bundle],
                "command": software_matrix_command,
                "ready_condition": "The software actions came from real plugin/API/MCP/CLI/desktop-control execution, include POOL_SOFTWARE_PRODUCTION_ATTESTATION or per-adapter software production attestation, and are not local mock or dry-run control profiles.",
                "generic_api_bridge_worker": software_bridge_worker,
                "bridge_worker_start_commands": software_bridge_worker_start_commands
            },
            {
                "id": "desktop_vision_evidence",
                "lane": "Desktop vision controller",
                "status": if desktop_tasks == 0 { "already_satisfied" } else { "needs_external_visual_model" },
                "purpose": "Run the desktop recognition queue through an external visual/OCR controller, write a local trace, and emit desktop_vision[] evidence.",
                "required": desktop_tasks > 0,
                "outputs": [desktop_bundle],
                "command": desktop_vision_command,
                "ready_condition": "The controller uses a real external visual/OCR model, POOL_DESKTOP_VISION_PRODUCTION_ATTESTATION identifies the real controller/model run, writes a local trace file, and returns external_visual_model:true through production-evidence-desktop-vision."
            },
            {
                "id": "merge_bundles",
                "lane": "Agent/Hermes",
                "status": "pending_runner_outputs",
                "purpose": "Merge independently produced provider/software/desktop-vision bundles without writing SQLite.",
                "required": missing_total > 0,
                "outputs": [combined_bundle],
                "command": format!("pool-cli --project {project_slug} merge-production-evidence {combined_bundle} {provider_bundle} {software_bundle} {desktop_bundle}"),
                "ready_condition": "All input bundles use the same project_slug and contain only real external evidence."
            },
            {
                "id": "closeout_preflight",
                "lane": "Agent/Hermes",
                "status": "pending_merged_bundle",
                "purpose": "Run merge + validate with writes:0 and confirm coverage plus artifact files before import.",
                "required": true,
                "outputs": [merged_bundle],
                "command": format!("pool-cli --project {project_slug} closeout-production-evidence --output {merged_bundle} {combined_bundle}"),
                "ready_condition": "Response reports ready_for_import:true, validation.coverage.complete:true, artifact_files.complete:true, and writes:0."
            },
            {
                "id": "closeout_import",
                "lane": "Agent/Hermes",
                "status": "requires_human_confirmation",
                "purpose": "Explicitly import the validated production evidence bundle into the runtime ledger.",
                "required": true,
                "outputs": ["provider_requests", "software_actions", "workflow_events"],
                "command": format!("pool-cli --project {project_slug} closeout-production-evidence --import {merged_bundle}"),
                "ready_condition": "Import response returns completion_gate.ready_for_completion:true and prd_overall_status:\"ready\"."
            },
            {
                "id": "completion_proof",
                "lane": "Agent/Hermes",
                "status": if ready_for_completion { "ready" } else { "pending_closeout_import" },
                "purpose": "Verify the PRD completion gate and write a local proof package.",
                "required": true,
                "outputs": [completion_package_dir],
                "command": format!("pool-cli --project {project_slug} prd-completion-gate --require-complete && pool-cli --project {project_slug} prd-completion-package --output-dir {completion_package_dir} --include-snapshot"),
                "ready_condition": "Completion gate succeeds with HTTP 200/CLI success and the proof package records ready_for_completion:true."
            }
        ],
        "commands": {
            "requirements": format!("pool-cli --project {project_slug} production-evidence-requirements"),
            "tasks": format!("pool-cli --project {project_slug} production-evidence-tasks"),
            "handoff": format!("pool-cli --project {project_slug} production-evidence-handoff <handoff.json>"),
            "run_plan": format!("pool-cli --project {project_slug} production-evidence-run-plan <run-plan.json>"),
            "mcp_run_plan_resource": format!("pool-cli --project {project_slug} mcp pool://production-evidence-run-plan"),
            "mcp_handoff_resource": format!("pool-cli --project {project_slug} mcp pool://production-evidence-handoff"),
            "ledger_bundle": format!("pool-cli --project {project_slug} production-evidence-bundle-from-ledger --include-incomplete <ledger-bundle.json>"),
            "merge": format!("pool-cli --project {project_slug} merge-production-evidence {combined_bundle} {provider_bundle} {software_bundle} {desktop_bundle}"),
            "closeout_preflight": format!("pool-cli --project {project_slug} closeout-production-evidence --output {merged_bundle} {combined_bundle}"),
            "closeout_import": format!("pool-cli --project {project_slug} closeout-production-evidence --import {merged_bundle}"),
            "completion_gate": format!("pool-cli --project {project_slug} prd-completion-gate --require-complete"),
            "completion_package": format!("pool-cli --project {project_slug} prd-completion-package --output-dir {completion_package_dir} --include-snapshot"),
        },
        "http": {
            "run_plan": format!("GET /api/production-evidence/run-plan?project={project_slug}"),
            "mcp_run_plan_resource": format!("GET /api/mcp?uri=pool://production-evidence-run-plan&project={project_slug}"),
            "mcp_handoff_resource": format!("GET /api/mcp?uri=pool://production-evidence-handoff&project={project_slug}"),
            "requirements": format!("GET /api/production-evidence/requirements?project={project_slug}"),
            "tasks": format!("GET /api/production-evidence/tasks?project={project_slug}"),
            "bundle_from_ledger": format!("GET /api/production-evidence/bundle-from-ledger?project={project_slug}&include_incomplete=true"),
            "merge": "POST /api/production-evidence/merge",
            "closeout": "POST /api/production-evidence/closeout",
            "completion_gate": format!("GET /api/prd-completion-gate?project={project_slug}&require_complete=true"),
        },
        "mcp": {
            "resources": [
                "pool://production-evidence-run-plan",
                "pool://production-evidence-handoff",
                "pool://production-evidence-tasks",
                "pool://production-evidence-requirements",
                "pool://prd-completion-gate"
            ],
            "tools": [
                "pool_production_evidence_run_plan",
                "pool_production_evidence_requirements",
                "pool_production_evidence_tasks",
                "pool_production_evidence_bundle_from_ledger",
                "pool_merge_production_evidence",
                "pool_closeout_production_evidence",
                "pool_prd_completion_gate",
                "pool_prd_completion_package"
            ],
        },
        "operator_checklist": [
            "Do not use local mock gateway/server outputs as production evidence.",
            "Keep Provider artifacts and metadata as local files; remote URLs are provenance only.",
            "Only set production_upstream, production_software, or external_visual_model when the real external system completed the work.",
            "Run closeout preflight first; it must report writes:0 before import.",
            "After import, require prd-completion-gate --require-complete before marking the PRD complete."
        ],
        "next_actions": if ready_for_completion {
            vec![format!("Run pool-cli --project {project_slug} prd-completion-package --output-dir {completion_package_dir} --include-snapshot to archive proof.")]
        } else if missing_total == 0 {
            vec![format!("Run pool-cli --project {project_slug} prd-completion-gate --require-complete to verify the current snapshot.")]
        } else {
            vec![
                "Run the provider_evidence_matrix phase against real Provider gateway endpoints.".to_string(),
                "Run the software_evidence_matrix phase against real software plugin/API/MCP/CLI control paths, using software-api-bridge-worker only when it forwards to a real software plugin or gateway.".to_string(),
                "Run the desktop_vision_evidence phase against a real external visual/OCR endpoint.".to_string(),
                "Merge and close out the resulting bundles, then import explicitly.".to_string(),
            ]
        },
    }))
}

pub fn production_evidence_tasks_resource(
    project_slug: &str,
    snapshot: &RuntimeSnapshot,
) -> Result<Value> {
    let requirements = runtime_production_evidence_requirements_resource(snapshot)?;
    let tasks = requirements
        .pointer("/evidence_tasks/tasks")
        .cloned()
        .unwrap_or_else(|| json!([]));
    let summary = requirements
        .pointer("/evidence_tasks/summary")
        .cloned()
        .unwrap_or_else(|| json!({}));

    Ok(json!({
        "kind": "pool_production_evidence_tasks",
        "version": 1,
        "project_filter": snapshot.project_filter,
        "generated_at": snapshot.generated_at,
        "overall_status": requirements.get("overall_status").cloned().unwrap_or_else(|| json!("partial")),
        "summary": summary,
        "tasks": tasks,
        "commands": {
            "requirements": format!("pool-cli --project {project_slug} production-evidence-requirements"),
            "tasks_resource": format!("pool-cli --project {project_slug} mcp pool://production-evidence-tasks"),
            "handoff_resource": format!("pool-cli --project {project_slug} mcp pool://production-evidence-handoff"),
            "item_template": format!("pool-cli --project {project_slug} production-evidence-item-template <kind> <target-id> <item.json>"),
            "submit_item": format!("pool-cli --project {project_slug} submit-production-evidence-item <item.json>"),
            "handoff_package": format!("pool-cli --project {project_slug} production-evidence-handoff-package --output-dir worlds/{project_slug}/output"),
            "closeout": format!("pool-cli --project {project_slug} closeout-production-evidence --output <merged-bundle.json> <bundle-a.json> <bundle-b.json>..."),
            "readiness": format!("pool-cli --project {project_slug} prd-readiness"),
        },
        "http": {
            "tasks": format!("GET /api/production-evidence/tasks?project={project_slug}"),
            "tasks_resource": format!("GET /api/mcp?uri=pool://production-evidence-tasks&project={project_slug}"),
            "handoff_resource": format!("GET /api/mcp?uri=pool://production-evidence-handoff&project={project_slug}"),
            "item_template": "GET /api/production-evidence/item-template?kind=<provider|software_action|desktop_vision>&target_id=<id>",
            "handoff_package": "POST /api/production-evidence/handoff-packages",
            "closeout": "POST /api/production-evidence/closeout",
            "submit_item": "POST /api/production-evidence/items",
        },
            "mcp": {
                "resources": [
                    "pool://production-evidence-item-template",
                    "pool://production-evidence-tasks",
                    "pool://production-evidence-handoff",
                    "pool://production-evidence-requirements",
                "pool://production-evidence-run-plan"
            ],
            "tasks_tool": "pool_production_evidence_tasks",
            "item_template_tool": "pool_production_evidence_item_template",
            "handoff_package_tool": "pool_production_evidence_handoff_package",
            "closeout_tool": "pool_closeout_production_evidence",
            "submit_tool": "pool_submit_production_evidence_item",
        }
    }))
}

pub fn production_evidence_handoff_resource(
    project_slug: &str,
    output_root: Option<&str>,
    source: &str,
    snapshot: &RuntimeSnapshot,
) -> Result<Value> {
    let default_output_root = format!("worlds/{project_slug}/output/production-evidence");
    let output_root = output_root.unwrap_or(default_output_root.as_str());
    let requirements = runtime_production_evidence_requirements_resource(snapshot)?;
    let tasks = production_evidence_tasks_resource(project_slug, snapshot)?;
    let run_plan =
        production_evidence_run_plan_resource(project_slug, Some(output_root), source, snapshot)?;
    let provider_gateway_worker_start_commands =
        production_evidence_provider_gateway_worker_start_commands(output_root);
    let software_bridge_worker = production_evidence_generic_software_bridge_worker();
    let software_bridge_worker_start_commands =
        production_evidence_bridge_worker_start_commands(output_root, &software_bridge_worker);
    let missing_total = requirements
        .pointer("/summary/missing_total")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let evidence_task_count = tasks
        .pointer("/summary/total")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let provider_tasks = tasks
        .pointer("/summary/provider_tasks")
        .cloned()
        .unwrap_or_else(|| json!(0));
    let software_tasks = tasks
        .pointer("/summary/software_tasks")
        .cloned()
        .unwrap_or_else(|| json!(0));
    let desktop_vision_tasks = tasks
        .pointer("/summary/desktop_vision_tasks")
        .cloned()
        .unwrap_or_else(|| json!(0));
    let bundle_path = format!("{output_root}/missing-production-evidence-bundle.json");
    let combined_bundle = format!("{output_root}/combined-production-evidence-bundle.json");
    let merged_bundle = format!("{output_root}/merged-production-evidence-bundle.json");

    Ok(json!({
        "kind": "pool_production_evidence_handoff",
        "version": 1,
        "project_slug": project_slug,
        "project_filter": snapshot.project_filter,
        "generated_at": snapshot.generated_at,
        "source": source,
        "overall_status": requirements.get("overall_status").cloned().unwrap_or_else(|| json!("partial")),
        "ready_for_import": false,
        "reason": "This read-only MCP handoff assigns missing production evidence work. Replace placeholders with real upstream/software/visual-controller evidence, validate, then import explicitly.",
        "output_root": output_root,
        "summary": {
            "missing_total": missing_total,
            "evidence_tasks": evidence_task_count,
            "provider_tasks": provider_tasks,
            "software_tasks": software_tasks,
            "desktop_vision_tasks": desktop_vision_tasks,
        },
        "handoff_lanes": [
            {
                "id": "provider_worker",
                "role": "Provider worker",
                "task_kinds": ["provider_production_upstream"],
                "resource": "pool://production-evidence-tasks",
                "output_bundle": format!("{output_root}/provider-production-evidence-bundle.json"),
                "provider_gateway_worker_start_commands": provider_gateway_worker_start_commands.clone(),
            },
            {
                "id": "software_operator",
                "role": "Software operator",
                "task_kinds": ["software_production"],
                "resource": "pool://production-evidence-tasks",
                "output_bundle": format!("{output_root}/software-production-evidence-bundle.json"),
                "generic_api_bridge_worker": software_bridge_worker,
                "bridge_worker_start_commands": software_bridge_worker_start_commands.clone(),
            },
            {
                "id": "desktop_vision_controller",
                "role": "Desktop vision controller",
                "task_kinds": ["desktop_vision"],
                "resource": "pool://production-evidence-tasks",
                "output_bundle": format!("{output_root}/desktop-vision-production-evidence-bundle.json"),
            },
            {
                "id": "agent_hermes_closeout",
                "role": "Agent/Hermes closeout",
                "task_kinds": ["merge_bundles", "closeout_preflight", "closeout_import", "completion_proof"],
                "resource": "pool://production-evidence-run-plan",
                "output_bundle": merged_bundle,
            }
        ],
        "paths": {
            "missing_bundle": bundle_path,
            "combined_bundle": combined_bundle,
            "merged_bundle": merged_bundle,
            "handoff_package_dir": format!("worlds/{project_slug}/output/control/production-evidence"),
            "completion_package_dir": format!("worlds/{project_slug}/output/control/prd-completion"),
        },
        "requirements": requirements,
        "tasks": tasks,
        "run_plan": run_plan,
        "provider_gateway_worker_start_commands": provider_gateway_worker_start_commands,
        "software_bridge_worker_start_commands": software_bridge_worker_start_commands,
        "commands": {
            "requirements": format!("pool-cli --project {project_slug} production-evidence-requirements"),
            "tasks": format!("pool-cli --project {project_slug} production-evidence-tasks"),
            "tasks_resource": format!("pool-cli --project {project_slug} mcp pool://production-evidence-tasks"),
            "handoff_resource": format!("pool-cli --project {project_slug} mcp pool://production-evidence-handoff"),
            "handoff_file": format!("pool-cli --project {project_slug} production-evidence-handoff <handoff.json> --output-root {output_root}"),
            "handoff_package": format!("pool-cli --project {project_slug} production-evidence-handoff-package --output-dir worlds/{project_slug}/output --output-root {output_root} --include-snapshot"),
            "run_plan": format!("pool-cli --project {project_slug} production-evidence-run-plan <run-plan.json> --output-root {output_root}"),
            "item_template": format!("pool-cli --project {project_slug} production-evidence-item-template --task-id <task-id> <item.json>"),
            "submit_item": format!("pool-cli --project {project_slug} submit-production-evidence-item <item.json>"),
            "merge": format!("pool-cli --project {project_slug} merge-production-evidence {combined_bundle} <provider-bundle.json> <software-bundle.json> <desktop-vision-bundle.json>"),
            "closeout_preflight": format!("pool-cli --project {project_slug} closeout-production-evidence --output {merged_bundle} {combined_bundle}"),
            "closeout_import": format!("pool-cli --project {project_slug} closeout-production-evidence --import {merged_bundle}"),
            "completion_gate": format!("pool-cli --project {project_slug} prd-completion-gate --require-complete"),
        },
        "http": {
            "handoff": format!("GET /api/production-evidence/handoff?project={project_slug}"),
            "handoff_resource": format!("GET /api/mcp?uri=pool://production-evidence-handoff&project={project_slug}"),
            "tasks": format!("GET /api/production-evidence/tasks?project={project_slug}"),
            "run_plan": format!("GET /api/production-evidence/run-plan?project={project_slug}"),
            "item_template": "GET /api/production-evidence/item-template?task_id=<task-id>",
            "submit_item": "POST /api/production-evidence/items",
            "merge": "POST /api/production-evidence/merge",
            "closeout": "POST /api/production-evidence/closeout",
        },
            "mcp": {
                "resources": [
                    "pool://production-evidence-handoff",
                    "pool://production-evidence-item-template",
                    "pool://production-evidence-tasks",
                    "pool://production-evidence-run-plan",
                    "pool://production-evidence-requirements",
                "pool://prd-completion-gate"
            ],
            "tools": [
                "pool_production_evidence_handoff",
                "pool_production_evidence_tasks",
                "pool_production_evidence_run_plan",
                "pool_production_evidence_item_template",
                "pool_submit_production_evidence_item",
                "pool_production_evidence_handoff_package",
                "pool_merge_production_evidence",
                "pool_closeout_production_evidence",
                "pool_prd_completion_gate"
            ],
        },
        "operator_checklist": [
            "Assign each tasks.tasks item to a Provider worker, software operator, or visual-controller operator.",
            "Use production-evidence-item-template --task-id for per-task JSON handoff.",
            "Write every Provider artifact and metadata_path to local files before validation.",
            "Use real external_job_id, external_action_id, production_attestation, controller_id, and external visual model trace values; placeholders are rejected.",
            "Use submit-production-evidence-item for per-task evidence callback or merge bundles for batch closeout.",
            "Run closeout-production-evidence without --import first and confirm writes:0 plus ready_for_import:true.",
            "Import only after the evidence came from real upstream services, real software control, or a real visual controller."
        ],
        "next_actions": if missing_total == 0 {
            vec![format!("Run pool-cli --project {project_slug} prd-completion-gate --require-complete and archive a completion package.")]
        } else {
            vec![
                format!("Read pool://production-evidence-tasks and assign all {evidence_task_count} missing evidence tasks."),
                format!("Generate item JSON with pool-cli --project {project_slug} production-evidence-item-template --task-id <task-id> <item.json>."),
                "Replace placeholders with real external evidence and submit items or merge bundles.".to_string(),
                format!("Run pool-cli --project {project_slug} closeout-production-evidence --output {merged_bundle} {combined_bundle} before import."),
            ]
        },
    }))
}

pub fn production_evidence_item_template_index_resource(
    snapshot: &RuntimeSnapshot,
) -> Result<Value> {
    let project_slug = snapshot
        .project_filter
        .as_deref()
        .filter(|project_slug| *project_slug != "*")
        .unwrap_or("<slug>");
    let tasks = production_evidence_tasks_resource(project_slug, snapshot)?;
    let task_ids = tasks
        .get("tasks")
        .and_then(Value::as_array)
        .map(|tasks| {
            tasks
                .iter()
                .filter_map(|task| task.get("id").and_then(Value::as_str))
                .map(|task_id| {
                    json!({
                        "task_id": task_id,
                        "uri": format!("pool://production-evidence-item-template/{task_id}"),
                        "http_path": format!("GET /api/mcp?uri=pool://production-evidence-item-template/{task_id}&project={project_slug}"),
                        "tool": "pool_production_evidence_item_template",
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Ok(json!({
        "kind": "pool_production_evidence_item_template_index",
        "version": 1,
        "project_filter": snapshot.project_filter,
        "generated_at": snapshot.generated_at,
        "summary": {
            "task_templates": task_ids.len(),
        },
        "sample_uri": "pool://production-evidence-item-template/<task-id>",
        "task_templates": task_ids,
        "commands": {
            "tasks": format!("pool-cli --project {project_slug} production-evidence-tasks"),
            "read_template": format!("pool-cli --project {project_slug} mcp pool://production-evidence-item-template/<task-id>"),
            "tool_template": format!("pool-cli --project {project_slug} production-evidence-item-template --task-id <task-id> <item.json>"),
        },
        "mcp": {
            "resources": [
                "pool://production-evidence-item-template",
                "pool://production-evidence-tasks",
                "pool://production-evidence-handoff"
            ],
            "tool": "pool_production_evidence_item_template",
        },
    }))
}

pub fn production_evidence_item_template_resource(
    project_slug: &str,
    output_root: Option<&str>,
    source: &str,
    task_id: &str,
    snapshot: &RuntimeSnapshot,
) -> Result<Value> {
    let task_id = task_id.trim();
    if task_id.is_empty() {
        bail!("production evidence item template task_id cannot be empty");
    }
    let tasks = production_evidence_tasks_resource(project_slug, snapshot)?;
    let task = tasks
        .get("tasks")
        .and_then(Value::as_array)
        .and_then(|tasks| {
            tasks
                .iter()
                .find(|task| task.get("id").and_then(Value::as_str) == Some(task_id))
        })
        .cloned()
        .with_context(|| format!("production evidence task not found: {task_id}"))?;
    let target_id = task
        .get("target_id")
        .and_then(Value::as_str)
        .filter(|target_id| !target_id.trim().is_empty())
        .with_context(|| format!("production evidence task missing target_id: {task_id}"))?;
    let kind = production_evidence_item_kind_from_task_id(task_id)?;
    let item = production_evidence_item_template_item(
        project_slug,
        output_root,
        source,
        kind,
        target_id,
        task_id,
    )?;

    Ok(json!({
        "kind": "pool_production_evidence_item_template",
        "version": 1,
        "project_slug": project_slug,
        "project_filter": snapshot.project_filter,
        "generated_at": snapshot.generated_at,
        "ready_for_import": false,
        "reason": "Template identifiers and fixture paths must be replaced with real external job/action/controller ids and local files before submit.",
        "selector": {
            "task_id": task_id,
            "kind": kind,
            "target_id": target_id,
        },
        "task": task,
        "output_root": output_root.unwrap_or("."),
        "item": item,
        "commands": {
            "submit": format!("pool-cli --project {project_slug} submit-production-evidence-item <item.json>"),
            "tasks": format!("pool-cli --project {project_slug} production-evidence-tasks"),
            "read_resource": format!("pool-cli --project {project_slug} mcp pool://production-evidence-item-template/{task_id}"),
            "write_file": format!("pool-cli --project {project_slug} production-evidence-item-template --task-id {task_id} <item.json>"),
            "readiness": format!("pool-cli --project {project_slug} prd-readiness"),
        },
        "http": {
            "tasks": format!("GET /api/production-evidence/tasks?project={project_slug}"),
            "item_template": format!("GET /api/production-evidence/item-template?project={project_slug}&task_id={task_id}"),
            "item_template_resource": format!("GET /api/mcp?uri=pool://production-evidence-item-template/{task_id}&project={project_slug}"),
            "submit_item": "POST /api/production-evidence/items",
        },
        "mcp": {
            "resources": [
                "pool://production-evidence-item-template",
                "pool://production-evidence-tasks",
                "pool://production-evidence-handoff"
            ],
            "tasks_tool": "pool_production_evidence_tasks",
            "item_template_tool": "pool_production_evidence_item_template",
            "submit_tool": "pool_submit_production_evidence_item",
        },
        "operator_checklist": [
            "Replace external_job_id, production_attestation, external_action_id, or controller_id with the real upstream identifier or run attestation.",
            "Write every artifact, metadata_path, or trace_path in the item to local files before submit.",
            "Keep production_upstream/production_software/external_visual_model true only for real external evidence.",
            "Submit the item with submit-production-evidence-item after local files exist."
        ],
    }))
}

fn production_evidence_item_kind_from_task_id(task_id: &str) -> Result<&'static str> {
    if task_id.starts_with("provider:") {
        return Ok("provider");
    }
    if task_id.starts_with("software:") {
        return Ok("software_action");
    }
    if task_id.starts_with("desktop_vision:") {
        return Ok("desktop_vision");
    }
    bail!("task_id must start with provider:, software:, or desktop_vision:")
}

fn production_evidence_item_template_item(
    project_slug: &str,
    output_root: Option<&str>,
    source: &str,
    kind: &str,
    target_id: &str,
    task_id: &str,
) -> Result<Value> {
    match kind {
        "provider" => {
            let artifact_path = production_evidence_template_path(
                output_root,
                &format!(
                    "worlds/{project_slug}/output/production/{target_id}/{}",
                    production_evidence_provider_artifact_name(target_id)
                ),
            );
            let metadata_path = production_evidence_template_path(
                output_root,
                &format!(
                    "worlds/{project_slug}/output/production/{target_id}/request-metadata.json"
                ),
            );
            Ok(json!({
                "project_slug": project_slug,
                "source": source,
                "kind": "provider",
                "provider": {
                    "provider_id": target_id,
                    "external_job_id": format!("replace-with-real-{target_id}-job-id"),
                    "endpoint": format!("https://worker.example.com/{target_id}"),
                    "family": production_evidence_provider_family(target_id),
                    "production_attestation": format!("replace-with-real-{target_id}-worker-attestation"),
                    "metadata_path": metadata_path,
                    "artifacts": [artifact_path],
                    "evidence_json": {
                        "source": source,
                        "task_id": task_id,
                        "replace_with": "real provider worker evidence",
                        "production_upstream": true,
                        "local_mock_gateway": false,
                        "production_attestation": format!("replace-with-real-{target_id}-worker-attestation")
                    }
                }
            }))
        }
        "software_action" => {
            let (action_kind, priority, control_profile, artifact) =
                production_evidence_software_profile(target_id, project_slug, output_root);
            let bridge_worker = production_software_bridge_worker_profile(target_id, project_slug);
            Ok(json!({
                "project_slug": project_slug,
                "source": source,
                "kind": "software_action",
                "software_action": {
                    "adapter_id": target_id,
                    "external_action_id": format!("replace-with-real-{target_id}-action-id"),
                    "production_attestation": format!("replace-with-real-{target_id}-software-run-attestation"),
                    "action_kind": action_kind,
                    "priority": priority,
                    "control_profile": control_profile,
                    "bridge_worker": bridge_worker,
                    "artifacts": [artifact],
                    "evidence_json": {
                        "source": source,
                        "task_id": task_id,
                        "replace_with": "real software plugin/API/MCP/CLI evidence",
                        "production_software": true,
                        "local_mock_software": false,
                        "production_attestation": format!("replace-with-real-{target_id}-software-run-attestation")
                    }
                }
            }))
        }
        "desktop_vision" => {
            let trace_path = production_evidence_template_path(
                output_root,
                &format!(
                    "worlds/{project_slug}/output/production/desktop-vision/{target_id}-trace.json"
                ),
            );
            Ok(json!({
                "project_slug": project_slug,
                "source": source,
                "kind": "desktop_vision",
                "desktop_vision": {
                    "adapter_id": "touchdesigner",
                    "external_action_id": format!("replace-with-real-{target_id}-action-id"),
                    "controller_id": "replace-with-real-vision-controller-id",
                    "production_attestation": format!("replace-with-real-{target_id}-controller-attestation"),
                    "trace_path": trace_path,
                    "visual_model": "external",
                    "artifacts": [trace_path],
                    "evidence_json": {
                        "source": source,
                        "task_id": task_id,
                        "target_id": target_id,
                        "replace_with": "real desktop visual model trace",
                        "external_visual_model": true,
                        "local_trace_smoke": false,
                        "production_attestation": format!("replace-with-real-{target_id}-controller-attestation")
                    }
                }
            }))
        }
        _ => bail!(
            "production evidence item kind must be provider, software_action, or desktop_vision"
        ),
    }
}

fn production_evidence_provider_family(provider_id: &str) -> &'static str {
    match provider_id {
        "openai-image-2" => "ai_image",
        "midjourney" | "nano-banana-pro" | "suno" => "ai_media",
        _ => "3dgs",
    }
}

fn production_evidence_provider_artifact_name(provider_id: &str) -> &'static str {
    match provider_id {
        "midjourney" => "1-midjourney.png",
        "openai-image-2" => "1-openai-image.png",
        "nano-banana-pro" => "1-nano.png",
        "suno" => "1-cue.mp3",
        "worldlabs-marble" => "1-world.glb",
        "tripo-splat" => "1-object.glb",
        "sam-3d" => "1-mask-object.glb",
        "spark-3dgs" => "1-scene.glb",
        "qunhe-3d" => "1-layout.glb",
        _ => "1-provider-artifact.bin",
    }
}

fn production_evidence_software_profile(
    adapter_id: &str,
    project_slug: &str,
    output_root: Option<&str>,
) -> (&'static str, &'static str, &'static str, String) {
    match adapter_id {
        "unreal" => (
            "CreateScene",
            "ApiMcp",
            "api_mcp",
            production_evidence_template_path(
                output_root,
                &format!("worlds/{project_slug}/output/production/unreal/1-level.umap"),
            ),
        ),
        "unity" => (
            "ExportBuild",
            "ApiMcp",
            "api_mcp",
            production_evidence_template_path(
                output_root,
                &format!("worlds/{project_slug}/output/production/unity/1-build.zip"),
            ),
        ),
        "blender" => (
            "ExecuteCli",
            "ApiMcp",
            "api_mcp",
            production_evidence_template_path(
                output_root,
                &format!("worlds/{project_slug}/output/production/blender/1-cleanup.blend"),
            ),
        ),
        "comfyui" => (
            "ExecuteCli",
            "ApiMcp",
            "api_mcp",
            production_evidence_template_path(
                output_root,
                &format!("worlds/{project_slug}/output/production/comfyui/1-image.png"),
            ),
        ),
        "touchdesigner" => (
            "RunViewport",
            "DesktopRecognition",
            "desktop_recognition",
            production_evidence_template_path(
                output_root,
                &format!("worlds/{project_slug}/output/production/touchdesigner/1-performance.toe"),
            ),
        ),
        "madmapper" => (
            "RunViewport",
            "DesktopRecognition",
            "desktop_recognition",
            production_evidence_template_path(
                output_root,
                &format!("worlds/{project_slug}/output/production/madmapper/1-cues.mad"),
            ),
        ),
        "resolve" => (
            "Transcode",
            "ApiMcp",
            "api_mcp",
            production_evidence_template_path(
                output_root,
                &format!("worlds/{project_slug}/output/production/resolve/1-master.mov"),
            ),
        ),
        "nuke" => (
            "Render",
            "ApiMcp",
            "api_mcp",
            production_evidence_template_path(
                output_root,
                &format!("worlds/{project_slug}/output/production/nuke/1-comp.exr"),
            ),
        ),
        "motion-db" => (
            "ImportAsset",
            "ApiMcp",
            "api_mcp",
            production_evidence_template_path(
                output_root,
                &format!("worlds/{project_slug}/output/production/motion-db/1-take.fbx"),
            ),
        ),
        "editing-suite" => (
            "Transcode",
            "ApiMcp",
            "api_mcp",
            production_evidence_template_path(
                output_root,
                &format!("worlds/{project_slug}/output/production/editing-suite/1-delivery.mp4"),
            ),
        ),
        "hermes" => (
            "CreateScene",
            "ApiMcp",
            "api_mcp",
            production_evidence_template_path(
                output_root,
                &format!("worlds/{project_slug}/output/production/hermes/1-session.json"),
            ),
        ),
        _ => (
            "ExecuteCli",
            "SkillsCli",
            "skills_cli",
            production_evidence_template_path(
                output_root,
                &format!("worlds/{project_slug}/output/production/{adapter_id}/1-artifact.bin"),
            ),
        ),
    }
}

fn production_evidence_template_path(output_root: Option<&str>, relative_path: &str) -> String {
    let Some(root) = output_root.map(str::trim).filter(|root| !root.is_empty()) else {
        return relative_path.to_string();
    };
    format!(
        "{}/{}",
        root.trim_end_matches('/'),
        relative_path.trim_start_matches('/')
    )
}

fn production_software_bridge_worker_profile(adapter_id: &str, project_slug: &str) -> Value {
    let endpoint_env = format!(
        "POOL_{}_ENDPOINT",
        adapter_id.to_ascii_uppercase().replace('-', "_")
    );
    let output_root = format!("worlds/{project_slug}/output");

    match adapter_id {
        "blender" | "comfyui" | "resolve" | "unity" | "nuke" | "motion-db" | "editing-suite" => {
            json!({
                "available": true,
                "adapter_id": adapter_id,
                "endpoint_env": endpoint_env,
                "cli_template": format!(
                    "pool-cli software-api-bridge-worker {adapter_id} --bind 127.0.0.1:<port> --output-root {output_root} --upstream <real-plugin-or-gateway-url>"
                ),
                "endpoint_env_template": format!("{endpoint_env}=http://127.0.0.1:<port>"),
                "upstream_required": true,
                "production_rule": "The local worker is valid production evidence only when --upstream forwards to a real software plugin, API, MCP service, or gateway."
            })
        }
        "touchdesigner" | "madmapper" => json!({
            "available": false,
            "adapter_id": adapter_id,
            "reason": "desktop_recognition_priority",
            "production_rule": "Use an external visual/OCR controller and desktop-control trace instead of the generic API bridge worker."
        }),
        "unreal" | "hermes" => json!({
            "available": false,
            "adapter_id": adapter_id,
            "reason": "dedicated_bridge_worker",
            "production_rule": "Use the dedicated Unreal or Hermes MCP bridge contract for this adapter."
        }),
        _ => json!({
            "available": false,
            "adapter_id": adapter_id,
            "reason": "skills_cli_or_custom_adapter",
            "production_rule": "Use a real explicit CLI command, Skill, or custom adapter and attach local artifact evidence."
        }),
    }
}

fn production_evidence_task_rows(
    snapshot: &RuntimeSnapshot,
    missing_providers: &[String],
    missing_software: &[String],
    missing_desktop_vision: &[String],
) -> Vec<Value> {
    let project_slug = snapshot.project_filter.as_deref().unwrap_or("<slug>");
    let mut tasks = Vec::new();

    for provider_id in missing_providers {
        tasks.push(json!({
            "id": format!("provider:{provider_id}:production_upstream"),
            "kind": "provider_production_upstream",
            "target_id": provider_id,
            "status": "missing",
            "title": format!("Record real upstream production evidence for {provider_id}"),
            "bundle_path": "providers[]",
            "family": production_provider_family(provider_id),
            "required_bundle_fields": [
                "provider_id",
                "external_job_id",
                "production_attestation",
                "artifacts",
                "metadata_path",
                "evidence_json.production_upstream:true",
                "evidence_json.local_mock_gateway:false"
            ],
            "artifact_policy": "Download the upstream result and request/response metadata to local image-blaster style indexed files before import.",
            "commands": production_evidence_task_commands(project_slug),
            "http": production_evidence_task_http_paths(project_slug),
            "mcp": production_evidence_task_mcp(),
        }));
    }

    for adapter_id in missing_software {
        tasks.push(json!({
            "id": format!("software:{adapter_id}:production_software"),
            "kind": "software_production",
            "target_id": adapter_id,
            "status": "missing",
            "title": format!("Record real software execution evidence for {adapter_id}"),
            "bundle_path": "software_actions[]",
            "preferred_control_profile": production_software_control_profile(adapter_id),
            "required_bundle_fields": [
                "adapter_id",
                "external_action_id",
                "production_attestation",
                "artifacts",
                "evidence_json.production_software:true",
                "evidence_json.local_mock_software:false"
            ],
            "artifact_policy": "Record the real API/MCP, Skills/CLI, or desktop-control execution result and write every software artifact to an existing local file before import.",
            "bridge_worker_hint": "For api_mcp targets, start pool-cli software-api-bridge-worker <adapter-id> only as an audit/forwarder to a real plugin or gateway, then set POOL_<ADAPTER>_ENDPOINT to that local worker.",
            "bridge_worker": production_software_bridge_worker_profile(adapter_id, project_slug),
            "commands": production_evidence_task_commands(project_slug),
            "http": production_evidence_task_http_paths(project_slug),
            "mcp": production_evidence_task_mcp(),
        }));
    }

    for missing_id in missing_desktop_vision {
        tasks.push(json!({
            "id": format!("desktop_vision:{missing_id}"),
            "kind": "desktop_vision",
            "target_id": missing_id,
            "status": "missing",
            "title": production_desktop_vision_task_title(missing_id),
            "bundle_path": "desktop_vision[]",
            "required_bundle_fields": [
                "external_action_id",
                "controller_id",
                "production_attestation",
                "trace_path",
                "visual_model:external or evidence_json.external_visual_model:true",
                "artifacts"
            ],
            "artifact_policy": "Use a real external visual model/capture/OCR controller trace; local dry-run trace smoke is not enough for PRD production evidence.",
            "commands": production_evidence_task_commands(project_slug),
            "http": production_evidence_task_http_paths(project_slug),
            "mcp": production_evidence_task_mcp(),
        }));
    }

    tasks
}

fn production_evidence_task_commands(project_slug: &str) -> Value {
    json!({
        "template": format!("pool-cli --project {project_slug} production-evidence-template --output-root <root> <bundle.json>"),
        "item_template": format!("pool-cli --project {project_slug} production-evidence-item-template <kind> <target-id> <item.json>"),
        "submit_item": format!("pool-cli --project {project_slug} submit-production-evidence-item <item.json>"),
        "merge": format!("pool-cli --project {project_slug} merge-production-evidence <combined-bundle.json> <bundle-a.json> <bundle-b.json>..."),
        "closeout": format!("pool-cli --project {project_slug} closeout-production-evidence --output <merged-bundle.json> <bundle-a.json> <bundle-b.json>..."),
        "validate": format!("pool-cli --project {project_slug} validate-production-evidence <bundle.json>"),
        "import": format!("pool-cli --project {project_slug} import-production-evidence <bundle.json>"),
        "readiness": format!("pool-cli --project {project_slug} prd-readiness"),
    })
}

fn production_evidence_task_http_paths(project_slug: &str) -> Value {
    let suffix = if project_slug == "<slug>" {
        "project=<slug>".to_string()
    } else {
        format!("project={project_slug}")
    };
    json!({
        "requirements": format!("GET /api/production-evidence/requirements?{suffix}"),
        "tasks": format!("GET /api/production-evidence/tasks?{suffix}"),
        "template": format!("GET /api/production-evidence/template?{suffix}"),
        "item_template": format!("GET /api/production-evidence/item-template?kind=<kind>&target_id=<target-id>&{suffix}"),
        "validate": "POST /api/production-evidence/validate",
        "submit_item": "POST /api/production-evidence/items",
        "import": "POST /api/production-evidence",
    })
}

fn production_evidence_task_mcp() -> Value {
    json!({
        "resource": "pool://production-evidence-tasks",
        "requirements_resource": "pool://production-evidence-requirements",
        "tool": "pool_production_evidence_requirements",
        "tasks_tool": "pool_production_evidence_tasks",
        "item_template_tool": "pool_production_evidence_item_template",
        "submit_tool": "pool_submit_production_evidence_item",
    })
}

fn production_evidence_provider_matrix_command(
    project_slug: &str,
    output_root: &str,
    provider_bundle: &str,
) -> String {
    let provider_env_flags = production_evidence_provider_matrix_env_flags();
    format!(
        "pool-cli --project {project_slug} production-evidence-provider-matrix {output_root}/provider-evidence-matrix --production-upstream --media-endpoint=<shared-media-gateway> --3dgs-endpoint=<shared-3dgs-gateway> {provider_env_flags} --openai-api-key-env OPENAI_API_KEY --evidence-bundle={provider_bundle}"
    )
}

fn production_evidence_provider_matrix_env_flags() -> String {
    REQUIRED_PROVIDER_EVIDENCE
        .iter()
        .flat_map(|provider_id| {
            let env_key = production_evidence_provider_env_key(provider_id);
            [
                format!("--provider-endpoint-env {provider_id}=POOL_PROVIDER_ENDPOINT_{env_key}"),
                format!("--provider-api-key-env {provider_id}=POOL_PROVIDER_API_KEY_{env_key}"),
                format!("--provider-attestation-env {provider_id}=POOL_PROVIDER_PRODUCTION_ATTESTATION_{env_key}"),
            ]
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn production_evidence_provider_env_key(provider_id: &str) -> String {
    provider_id.replace('-', "_").to_ascii_uppercase()
}

fn production_evidence_provider_gateway_worker_start_commands(output_root: &str) -> Value {
    json!([
        {
            "family": "ai_media",
            "applies_to": ["midjourney", "nano-banana-pro", "suno"],
            "endpoint_env": "POOL_MEDIA_GATEWAY_ENDPOINT",
            "endpoint_assignment": "POOL_MEDIA_GATEWAY_ENDPOINT=http://127.0.0.1:<port>",
            "upstream_env": "POOL_MEDIA_GATEWAY_UPSTREAM_ENDPOINT",
            "api_key_env": "POOL_MEDIA_GATEWAY_API_KEY",
            "cli": format!("pool-cli provider-gateway-worker --bind 127.0.0.1:<port> --upstream $POOL_MEDIA_GATEWAY_UPSTREAM_ENDPOINT --api-key-env POOL_MEDIA_GATEWAY_API_KEY"),
            "output_root": output_root,
            "production_rule": "The worker is production-valid only when --upstream or --provider-upstream routes to real AI media vendor workers, official SDK wrappers, or approved gateway services; provider URLs remain provenance and local downloaded files are authoritative."
        },
        {
            "family": "3dgs",
            "applies_to": ["worldlabs-marble", "tripo-splat", "sam-3d", "spark-3dgs", "qunhe-3d"],
            "endpoint_env": "POOL_3DGS_GATEWAY_ENDPOINT",
            "endpoint_assignment": "POOL_3DGS_GATEWAY_ENDPOINT=http://127.0.0.1:<port>",
            "upstream_env": "POOL_3DGS_GATEWAY_UPSTREAM_ENDPOINT",
            "api_key_env": "POOL_3DGS_GATEWAY_API_KEY",
            "cli": format!("pool-cli provider-gateway-worker --bind 127.0.0.1:<port> --upstream $POOL_3DGS_GATEWAY_UPSTREAM_ENDPOINT --api-key-env POOL_3DGS_GATEWAY_API_KEY"),
            "output_root": output_root,
            "production_rule": "The worker is production-valid only when --upstream or --provider-upstream routes to real 3DGS vendor workers, official SDK wrappers, or approved gateway services; indexed local 3DGS assets are the loading source of truth."
        }
    ])
}

fn production_evidence_generic_software_bridge_worker() -> Value {
    json!({
        "applies_to": ["blender", "comfyui", "resolve", "unity", "nuke", "motion-db", "editing-suite"],
        "cli_template": "pool-cli software-api-bridge-worker <adapter-id> --bind 127.0.0.1:<port> --output-root worlds/<project>/output --upstream <real-plugin-or-gateway-url>",
        "endpoint_env_template": "POOL_<ADAPTER>_ENDPOINT=http://127.0.0.1:<port>",
        "operator_note": "Use the local bridge worker only as an audit/forwarder in production evidence mode; the upstream behind --upstream must be a real software plugin, API, MCP service, or gateway."
    })
}

fn production_evidence_bridge_worker_start_commands(
    output_root: &str,
    bridge_worker: &Value,
) -> Value {
    let commands = bridge_worker
        .get("applies_to")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::trim)
        .filter(|adapter_id| !adapter_id.is_empty())
        .map(|adapter_id| {
            let env_key = production_evidence_provider_env_key(adapter_id);
            let endpoint_env = format!("POOL_{env_key}_ENDPOINT");
            let upstream_env = format!("POOL_{env_key}_UPSTREAM_ENDPOINT");
            json!({
                "adapter_id": adapter_id,
                "endpoint_env": endpoint_env,
                "endpoint_assignment": format!("POOL_{env_key}_ENDPOINT=http://127.0.0.1:<port>"),
                "upstream_env": upstream_env,
                "cli": format!("pool-cli software-api-bridge-worker {adapter_id} --bind 127.0.0.1:<port> --output-root {output_root} --upstream ${upstream_env}"),
                "production_rule": "The bridge worker is production-valid only when --upstream points to a real software plugin, API, MCP service, or gateway, and local artifacts are written through POOL_*_ARTIFACTS."
            })
        })
        .collect::<Vec<_>>();
    json!(commands)
}

fn production_desktop_vision_task_title(missing_id: &str) -> String {
    match missing_id {
        "controller_callback_success" => {
            "Record desktop recognition controller callback success".to_string()
        }
        "vision_trace_path" => "Attach a local desktop vision trace JSON path".to_string(),
        "external_visual_model" => {
            "Attach real external visual model evidence for desktop recognition".to_string()
        }
        _ => format!("Record desktop vision evidence for {missing_id}"),
    }
}

fn production_provider_requirement_rows(provider_evidence: &Value) -> Vec<Value> {
    REQUIRED_PROVIDER_EVIDENCE
        .iter()
        .map(|provider_id| {
            let current = evidence_matrix_row(
                provider_evidence,
                "providers",
                "provider_id",
                provider_id,
            )
            .cloned()
            .unwrap_or_else(|| json!({ "provider_id": provider_id }));
            let gateway_success = current
                .get("gateway_profile_success")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let production_success = current
                .get("production_upstream_success")
                .and_then(Value::as_bool)
                .unwrap_or(false);

            json!({
                "provider_id": provider_id,
                "family": production_provider_family(provider_id),
                "status": production_requirement_status(gateway_success, production_success),
                "required_bundle_fields": [
                    "provider_id",
                    "external_job_id",
                    "production_attestation",
                    "artifacts",
                    "metadata_path",
                    "evidence_json.production_upstream:true",
                    "evidence_json.local_mock_gateway:false"
                ],
                "artifact_rule": "Download upstream result and request metadata to image-blaster style local indexed files before import.",
                "current": current,
            })
        })
        .collect()
}

fn production_software_requirement_rows(software_evidence: &Value) -> Vec<Value> {
    REQUIRED_SOFTWARE_EVIDENCE
        .iter()
        .map(|adapter_id| {
            let current = evidence_matrix_row(
                software_evidence,
                "adapters",
                "adapter_id",
                adapter_id,
            )
            .cloned()
            .unwrap_or_else(|| json!({ "adapter_id": adapter_id }));
            let control_success = current
                .get("control_profile_success")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let production_success = current
                .get("production_software_success")
                .and_then(Value::as_bool)
                .unwrap_or(false);

            json!({
                "adapter_id": adapter_id,
                "preferred_control_profile": production_software_control_profile(adapter_id),
                "status": production_requirement_status(control_success, production_success),
                "required_bundle_fields": [
                    "adapter_id",
                    "external_action_id",
                    "production_attestation",
                    "artifacts",
                    "evidence_json.production_software:true",
                    "evidence_json.local_mock_software:false"
                ],
                "artifact_rule": "Record the real plugin/API/MCP/CLI/desktop output and write every software artifact to an existing local file before import.",
                "bridge_worker_hint": "For ApiMcp generic adapters, POOL_<ADAPTER>_ENDPOINT may point to pool-cli software-api-bridge-worker <adapter-id> when that worker forwards to a real software plugin or gateway.",
                "bridge_worker": production_software_bridge_worker_profile(adapter_id, "<slug>"),
                "current": current,
            })
        })
        .collect()
}

fn production_desktop_vision_requirement(desktop_vision: &Value) -> Value {
    let summary = desktop_vision
        .get("summary")
        .cloned()
        .unwrap_or_else(|| json!({}));
    json!({
        "status": if production_desktop_vision_missing(&summary).is_empty() {
            "complete"
        } else {
            "missing"
        },
        "required_bundle_fields": [
            "external_action_id",
            "controller_id",
            "production_attestation",
            "trace_path",
            "visual_model:external or evidence_json.external_visual_model:true",
            "artifacts"
        ],
        "artifact_rule": "A local desktop trace smoke is useful for callback testing, but PRD production evidence needs a real visual model/capture/OCR controller trace.",
        "current": desktop_vision,
    })
}

fn evidence_matrix_row<'a>(
    matrix: &'a Value,
    collection: &str,
    id_field: &str,
    id: &str,
) -> Option<&'a Value> {
    matrix
        .get(collection)
        .and_then(Value::as_array)?
        .iter()
        .find(|row| row.get(id_field).and_then(Value::as_str) == Some(id))
}

fn production_requirement_status(profile_success: bool, production_success: bool) -> &'static str {
    if production_success {
        "complete"
    } else if profile_success {
        "needs_production_evidence"
    } else {
        "missing"
    }
}

fn production_provider_family(provider_id: &str) -> &'static str {
    match provider_id {
        "openai-image-2" => "ai_image",
        "midjourney" | "nano-banana-pro" | "suno" => "ai_media",
        _ => "3dgs",
    }
}

fn production_software_control_profile(adapter_id: &str) -> &'static str {
    match adapter_id {
        "unreal" | "blender" | "comfyui" | "resolve" | "unity" | "nuke" | "motion-db"
        | "editing-suite" | "hermes" => "api_mcp",
        "touchdesigner" | "madmapper" => "desktop_recognition",
        _ => "skills_cli",
    }
}

fn production_desktop_vision_missing(summary: &Value) -> Vec<String> {
    let mut missing = Vec::new();
    if !summary
        .get("controller_callback_ready")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        missing.push("controller_callback_success".to_string());
    }
    if !summary
        .get("vision_trace_ready")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        missing.push("vision_trace_path".to_string());
    }
    if !summary
        .get("external_visual_model_ready")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        missing.push("external_visual_model".to_string());
    }
    if !summary
        .get("production_attestation_ready")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        missing.push("production_attestation".to_string());
    }
    missing
}

fn prd_requirement<G, N>(
    id: &str,
    title: &str,
    status: &str,
    summary: &str,
    evidence: Value,
    gaps: Vec<G>,
    next_actions: Vec<N>,
) -> Value
where
    G: Into<String>,
    N: Into<String>,
{
    let gaps = gaps.into_iter().map(Into::into).collect::<Vec<_>>();
    let next_actions = next_actions.into_iter().map(Into::into).collect::<Vec<_>>();
    json!({
        "id": id,
        "title": title,
        "status": status,
        "summary": summary,
        "evidence": evidence,
        "gaps": gaps,
        "next_actions": next_actions,
    })
}

const REQUIRED_PROVIDER_EVIDENCE: &[&str] = &[
    "midjourney",
    "openai-image-2",
    "nano-banana-pro",
    "suno",
    "worldlabs-marble",
    "tripo-splat",
    "sam-3d",
    "spark-3dgs",
    "qunhe-3d",
];

fn provider_evidence_matrix(snapshot: &RuntimeSnapshot) -> Value {
    let providers = REQUIRED_PROVIDER_EVIDENCE
        .iter()
        .map(|provider_id| provider_evidence_row(snapshot, provider_id))
        .collect::<Vec<_>>();
    let missing_gateway_profile_success = providers
        .iter()
        .filter(|provider| {
            !provider
                .get("gateway_profile_success")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
        .filter_map(|provider| provider.get("provider_id").and_then(Value::as_str))
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let missing_production_upstream_success = providers
        .iter()
        .filter(|provider| {
            !provider
                .get("production_upstream_success")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
        .filter_map(|provider| provider.get("provider_id").and_then(Value::as_str))
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let gateway_profile_ready = missing_gateway_profile_success.is_empty();
    let production_upstream_ready = missing_production_upstream_success.is_empty();

    json!({
        "required_providers": REQUIRED_PROVIDER_EVIDENCE,
        "summary": {
            "required": REQUIRED_PROVIDER_EVIDENCE.len(),
            "gateway_profile_success": REQUIRED_PROVIDER_EVIDENCE.len() - missing_gateway_profile_success.len(),
            "production_upstream_success": REQUIRED_PROVIDER_EVIDENCE.len() - missing_production_upstream_success.len(),
            "gateway_profile_ready": gateway_profile_ready,
            "production_upstream_ready": production_upstream_ready,
            "missing_gateway_profile_success": missing_gateway_profile_success,
            "missing_production_upstream_success": missing_production_upstream_success,
        },
        "providers": providers,
    })
}

fn provider_evidence_row(snapshot: &RuntimeSnapshot, provider_id: &str) -> Value {
    let requests = snapshot
        .provider_requests
        .iter()
        .filter(|request| request.provider_id == provider_id)
        .collect::<Vec<_>>();
    let succeeded_requests = requests
        .iter()
        .filter(|request| provider_request_succeeded(request))
        .count();
    let gateway_profile_success = requests.iter().any(|request| {
        provider_request_succeeded(request)
            && matches!(
                provider_request_execution_mode(request).as_deref(),
                Some("gateway" | "adapter")
            )
    });
    let production_upstream_success = requests.iter().any(|request| {
        let evidence = request.request.get("evidence").unwrap_or(&Value::Null);
        provider_request_succeeded(request)
            && matches!(
                provider_request_execution_mode(request).as_deref(),
                Some("gateway" | "adapter")
            )
            && evidence
                .get("production_upstream")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            && !evidence
                .get("local_mock_gateway")
                .and_then(Value::as_bool)
                .unwrap_or(false)
    });
    let latest = requests.first();

    json!({
        "provider_id": provider_id,
        "requests": requests.len(),
        "succeeded_requests": succeeded_requests,
        "gateway_profile_success": gateway_profile_success,
        "production_upstream_success": production_upstream_success,
        "latest": latest.map(|request| {
            json!({
                "request_id": request.id,
                "status": provider_request_status(request),
                "execution_mode": provider_request_execution_mode(request),
                "endpoint": request.request.get("endpoint").cloned(),
                "evidence": request.request.get("evidence").cloned(),
                "metadata_path": request.metadata_path,
                "created_at": request.created_at,
            })
        }),
    })
}

fn provider_request_succeeded(request: &crate::db::ProviderRequestSnapshot) -> bool {
    provider_request_status(request).as_deref() == Some("Succeeded")
}

fn provider_request_status(request: &crate::db::ProviderRequestSnapshot) -> Option<String> {
    request
        .response
        .as_ref()
        .and_then(|response| response.get("status"))
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .or_else(|| {
            request
                .response
                .as_ref()
                .and_then(|response| response.pointer("/report/status"))
                .and_then(Value::as_str)
                .map(ToString::to_string)
        })
}

fn provider_request_execution_mode(request: &crate::db::ProviderRequestSnapshot) -> Option<String> {
    request
        .request
        .get("execution_mode")
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn provider_evidence_gaps(
    gateway_profile_ready: bool,
    missing_gateway_profile: &[String],
    production_upstream_ready: bool,
    missing_production: &[String],
) -> Vec<String> {
    let mut gaps = Vec::new();
    if !gateway_profile_ready {
        gaps.push(format!(
            "Gateway profile success evidence is missing for: {}.",
            missing_gateway_profile.join(", ")
        ));
    }
    if !production_upstream_ready {
        gaps.push(format!(
            "Production upstream success evidence is missing for: {}.",
            missing_production.join(", ")
        ));
    }
    gaps
}

fn string_array_at(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(ToString::to_string)
        .collect()
}

const REQUIRED_SOFTWARE_EVIDENCE: &[&str] = &[
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

fn software_evidence_matrix(snapshot: &RuntimeSnapshot) -> Value {
    let adapters = REQUIRED_SOFTWARE_EVIDENCE
        .iter()
        .map(|adapter_id| software_evidence_row(snapshot, adapter_id))
        .collect::<Vec<_>>();
    let missing_control_profile_success = adapters
        .iter()
        .filter(|adapter| {
            !adapter
                .get("control_profile_success")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
        .filter_map(|adapter| adapter.get("adapter_id").and_then(Value::as_str))
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let missing_production_software_success = adapters
        .iter()
        .filter(|adapter| {
            !adapter
                .get("production_software_success")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
        .filter_map(|adapter| adapter.get("adapter_id").and_then(Value::as_str))
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let control_profile_ready = missing_control_profile_success.is_empty();
    let production_software_ready = missing_production_software_success.is_empty();

    json!({
        "required_adapters": REQUIRED_SOFTWARE_EVIDENCE,
        "summary": {
            "required": REQUIRED_SOFTWARE_EVIDENCE.len(),
            "control_profile_success": REQUIRED_SOFTWARE_EVIDENCE.len() - missing_control_profile_success.len(),
            "production_software_success": REQUIRED_SOFTWARE_EVIDENCE.len() - missing_production_software_success.len(),
            "control_profile_ready": control_profile_ready,
            "production_software_ready": production_software_ready,
            "missing_control_profile_success": missing_control_profile_success,
            "missing_production_software_success": missing_production_software_success,
        },
        "adapters": adapters,
    })
}

fn software_evidence_row(snapshot: &RuntimeSnapshot, adapter_id: &str) -> Value {
    let actions = snapshot
        .software_actions
        .iter()
        .filter(|action| action.adapter_id == adapter_id)
        .collect::<Vec<_>>();
    let succeeded_actions = actions
        .iter()
        .filter(|action| software_action_succeeded(action))
        .count();
    let control_profile_success = actions.iter().any(|action| {
        software_action_succeeded(action) && software_action_control_mode(action).is_some()
    });
    let production_software_success = actions.iter().any(|action| {
        let evidence = software_action_evidence(action);
        let production_attestation = evidence
            .get("production_attestation")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_some();
        software_action_succeeded(action)
            && software_action_control_mode(action).is_some()
            && evidence
                .get("production_software")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            && !evidence
                .get("local_mock_software")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            && production_attestation
    });
    let latest = actions.first();

    json!({
        "adapter_id": adapter_id,
        "actions": actions.len(),
        "succeeded_actions": succeeded_actions,
        "control_profile_success": control_profile_success,
        "production_software_success": production_software_success,
        "latest": latest.map(|action| {
            json!({
                "action_id": action.id,
                "action_kind": action.action_kind,
                "status": software_action_status(action),
                "priority": action.command.get("priority").cloned(),
                "evidence": software_action_evidence(action),
                "created_at": action.created_at,
            })
        }),
    })
}

fn software_action_succeeded(action: &crate::db::SoftwareActionSnapshot) -> bool {
    action
        .verification
        .as_ref()
        .and_then(|verification| verification.get("ok"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || action
            .verification
            .as_ref()
            .and_then(|verification| verification.get("desktop_recognition_status"))
            .and_then(Value::as_str)
            == Some("succeeded")
}

fn software_action_status(action: &crate::db::SoftwareActionSnapshot) -> Option<String> {
    if software_action_succeeded(action) {
        Some("Succeeded".to_string())
    } else {
        action
            .verification
            .as_ref()
            .and_then(|verification| verification.get("desktop_recognition_status"))
            .and_then(Value::as_str)
            .map(ToString::to_string)
            .or_else(|| {
                action
                    .verification
                    .as_ref()
                    .and_then(|verification| verification.get("message"))
                    .and_then(Value::as_str)
                    .map(ToString::to_string)
            })
    }
}

fn software_action_control_mode(
    action: &crate::db::SoftwareActionSnapshot,
) -> Option<&'static str> {
    match action.command.get("priority").and_then(Value::as_str) {
        Some("ApiMcp") => Some("api_mcp"),
        Some("SkillsCli") => Some("skills_cli"),
        Some("DesktopRecognition") => Some("desktop_recognition"),
        Some("HumanTakeover") => Some("human_takeover"),
        _ => None,
    }
}

fn software_action_evidence(action: &crate::db::SoftwareActionSnapshot) -> Value {
    action
        .command
        .pointer("/payload_json/evidence")
        .cloned()
        .unwrap_or_else(|| json!({}))
}

fn software_evidence_gaps(
    control_profile_ready: bool,
    missing_control: &[String],
    production_software_ready: bool,
    missing_production: &[String],
) -> Vec<String> {
    let mut gaps = Vec::new();
    if !control_profile_ready {
        gaps.push(format!(
            "Software control profile success evidence is missing for: {}.",
            missing_control.join(", ")
        ));
    }
    if !production_software_ready {
        gaps.push(format!(
            "Production software-side success evidence is missing for: {}.",
            missing_production.join(", ")
        ));
    }
    gaps
}

fn desktop_vision_evidence(snapshot: &RuntimeSnapshot) -> Value {
    let desktop_actions = snapshot
        .software_actions
        .iter()
        .filter(|action| is_desktop_recognition_action(action))
        .collect::<Vec<_>>();
    let callback_actions = desktop_actions
        .iter()
        .filter(|action| desktop_controller_callback_recorded(action))
        .count();
    let succeeded_callbacks = desktop_actions
        .iter()
        .filter(|action| desktop_recognition_status_name(action) == "succeeded")
        .count();
    let trace_actions = desktop_actions
        .iter()
        .filter(|action| desktop_vision_trace_path(action).is_some())
        .count();
    let local_trace_smoke_actions = desktop_actions
        .iter()
        .filter(|action| desktop_local_trace_smoke(action))
        .count();
    let external_visual_model_actions = desktop_actions
        .iter()
        .filter(|action| {
            desktop_external_visual_model(action) && desktop_production_attestation_present(action)
        })
        .count();
    let latest = desktop_actions.first().map(|action| {
        json!({
            "software_action_id": action.id,
            "adapter_id": action.adapter_id,
            "action_kind": action.action_kind,
            "status": desktop_recognition_status_name(action),
            "trace_path": desktop_vision_trace_path(action),
            "local_trace_smoke": desktop_local_trace_smoke(action),
            "external_visual_model": desktop_external_visual_model(action),
            "production_attestation": desktop_production_attestation_value(action),
        })
    });

    json!({
        "summary": {
            "desktop_actions": desktop_actions.len(),
            "controller_callback_actions": callback_actions,
            "succeeded_callbacks": succeeded_callbacks,
            "vision_trace_actions": trace_actions,
            "local_trace_smoke_actions": local_trace_smoke_actions,
            "external_visual_model_actions": external_visual_model_actions,
            "desktop_queue_contract_ready": !desktop_actions.is_empty(),
            "controller_callback_ready": callback_actions > 0 && succeeded_callbacks > 0,
            "vision_trace_ready": trace_actions > 0,
            "local_trace_smoke_ready": local_trace_smoke_actions > 0,
            "production_attestation_ready": external_visual_model_actions > 0,
            "external_visual_model_ready": external_visual_model_actions > 0,
        },
        "latest": latest,
    })
}

fn production_hardening_gaps(
    desktop_controller_callback_ready: bool,
    desktop_vision_trace_ready: bool,
    desktop_external_visual_model_ready: bool,
    provider_production_ready: bool,
    software_production_ready: bool,
) -> Vec<String> {
    let mut gaps = Vec::new();
    if !desktop_controller_callback_ready {
        gaps.push(
            "Desktop recognition controller callback success evidence is missing.".to_string(),
        );
    }
    if !desktop_vision_trace_ready {
        gaps.push(
            "Desktop vision trace evidence is missing; record screen_trace_path or controller_result.vision_trace_path.".to_string(),
        );
    }
    if !desktop_external_visual_model_ready {
        gaps.push(
            "External visual model/capture/OCR evidence is missing; local trace smoke only proves the Pool callback contract.".to_string(),
        );
    }
    if !provider_production_ready || !software_production_ready {
        gaps.push(
            "Real vendor SDK wrappers and authenticated provider/software E2E runs are not proven by this snapshot.".to_string(),
        );
    }
    gaps
}

fn desktop_controller_callback_recorded(action: &SoftwareActionSnapshot) -> bool {
    action
        .verification
        .as_ref()
        .and_then(|verification| verification.get("desktop_recognition_status"))
        .is_some()
}

fn desktop_vision_trace_path(action: &SoftwareActionSnapshot) -> Option<String> {
    let verification = action.verification.as_ref()?;
    json_string_path(verification, &["screen_trace_path"])
        .or_else(|| json_string_path(verification, &["controller_result", "vision_trace_path"]))
}

fn desktop_local_trace_smoke(action: &SoftwareActionSnapshot) -> bool {
    let command_evidence = action.command.pointer("/payload_json/evidence");
    let verification = action.verification.as_ref();
    json_bool_path_opt(command_evidence, &["local_trace_smoke"]).unwrap_or(false)
        || json_bool_path_opt(verification, &["local_trace_smoke"]).unwrap_or(false)
        || json_bool_path_opt(verification, &["controller_result", "local_trace_smoke"])
            .unwrap_or(false)
        || json_string_path_opt(command_evidence, &["source"])
            == Some("run_desktop_vision_trace_smoke")
        || json_string_path_opt(verification, &["controller_result", "controller"])
            == Some("run_desktop_vision_trace_smoke")
}

fn desktop_external_visual_model(action: &SoftwareActionSnapshot) -> bool {
    let command_evidence = action.command.pointer("/payload_json/evidence");
    let verification = action.verification.as_ref();
    json_bool_path_opt(command_evidence, &["external_visual_model"]).unwrap_or(false)
        || json_bool_path_opt(verification, &["external_visual_model"]).unwrap_or(false)
        || json_bool_path_opt(
            verification,
            &["controller_result", "external_visual_model"],
        )
        .unwrap_or(false)
        || json_string_path_opt(verification, &["controller_result", "visual_model"])
            .is_some_and(|value| value == "external")
}

fn desktop_production_attestation_value(action: &SoftwareActionSnapshot) -> Option<String> {
    let command_evidence = action.command.pointer("/payload_json/evidence");
    let verification = action.verification.as_ref();
    json_string_path_opt(command_evidence, &["production_attestation"])
        .or_else(|| json_string_path_opt(verification, &["production_attestation"]))
        .or_else(|| {
            json_string_path_opt(
                verification,
                &["controller_result", "production_attestation"],
            )
        })
        .map(ToString::to_string)
}

fn desktop_production_attestation_present(action: &SoftwareActionSnapshot) -> bool {
    desktop_production_attestation_value(action)
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some()
}

fn json_string_path(value: &Value, path: &[&str]) -> Option<String> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    current
        .as_str()
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string)
}

fn json_string_path_opt<'a>(value: Option<&'a Value>, path: &[&str]) -> Option<&'a str> {
    let mut current = value?;
    for key in path {
        current = current.get(*key)?;
    }
    current.as_str().filter(|value| !value.trim().is_empty())
}

fn json_bool_path_opt(value: Option<&Value>, path: &[&str]) -> Option<bool> {
    let mut current = value?;
    for key in path {
        current = current.get(*key)?;
    }
    current.as_bool()
}

fn prd_readiness_summary(requirements: &[Value]) -> Value {
    let ready = requirements
        .iter()
        .filter(|requirement| requirement["status"] == "ready")
        .count();
    let partial = requirements
        .iter()
        .filter(|requirement| requirement["status"] == "partial")
        .count();
    let blocked = requirements
        .iter()
        .filter(|requirement| requirement["status"] == "blocked")
        .count();

    json!({
        "total": requirements.len(),
        "ready": ready,
        "partial": partial,
        "blocked": blocked,
    })
}

fn prd_completion_gate(
    project_slug: &str,
    requirements: &[Value],
    summary: &Value,
    overall_status: &str,
) -> Value {
    let incomplete_requirements = requirements
        .iter()
        .filter(|requirement| requirement["status"] != "ready")
        .map(|requirement| {
            json!({
                "id": requirement.get("id").cloned().unwrap_or_else(|| json!("unknown")),
                "status": requirement.get("status").cloned().unwrap_or_else(|| json!("unknown")),
                "gaps": requirement.get("gaps").cloned().unwrap_or_else(|| json!([])),
                "next_actions": requirement.get("next_actions").cloned().unwrap_or_else(|| json!([])),
            })
        })
        .collect::<Vec<_>>();
    let ready_for_completion = overall_status == "ready" && incomplete_requirements.is_empty();

    json!({
        "status": if ready_for_completion { "complete" } else { "incomplete" },
        "ready_for_completion": ready_for_completion,
        "completion_is_proven_by_current_snapshot": ready_for_completion,
        "summary": summary,
        "criteria": [
            "All PRD requirements in requirements[] must have status:\"ready\".",
            "AI media and 3DGS providers must include production_upstream:true evidence from real upstream workers or imported production evidence.",
            "External software adapters must include production_software:true evidence from real plugin/API/MCP/CLI/desktop-controller execution or imported production evidence.",
            "Desktop vision must include an external visual model trace, not only local dry-run or smoke trace evidence.",
            "Provider artifacts, metadata, software artifacts, desktop traces, and desktop artifacts must resolve to local files; remote URLs are provenance only."
        ],
        "incomplete_requirements": incomplete_requirements,
        "proof_commands": {
            "readiness": format!("pool-cli --project {project_slug} prd-readiness"),
            "requirements": format!("pool-cli --project {project_slug} production-evidence-requirements"),
            "handoff_package": format!("pool-cli --project {project_slug} production-evidence-handoff-package --output-dir worlds/{project_slug}/output --output-root worlds/{project_slug}/output/production-evidence --include-snapshot"),
            "closeout_preflight": format!("pool-cli --project {project_slug} closeout-production-evidence --output <merged-bundle.json> <provider-bundle.json> <software-bundle.json> <desktop-vision-bundle.json>"),
            "closeout_import": format!("pool-cli --project {project_slug} closeout-production-evidence --import <merged-bundle.json>"),
            "readiness_smoke_local": "cargo run -p pool-core --example run_prd_readiness_smoke -- target/prd-readiness-runner",
            "readiness_smoke_with_fixture_production_evidence": "cargo run -p pool-core --example run_prd_readiness_smoke -- --with-production-evidence target/prd-readiness-production-runner"
        },
    })
}

fn core_architecture_gate(
    project_slug: &str,
    requirements: &[Value],
    summary: &Value,
    overall_status: &str,
) -> Value {
    let incomplete_requirements = requirements
        .iter()
        .filter(|requirement| requirement["status"] != "ready")
        .map(|requirement| {
            json!({
                "id": requirement.get("id").cloned().unwrap_or_else(|| json!("unknown")),
                "status": requirement.get("status").cloned().unwrap_or_else(|| json!("unknown")),
                "gaps": requirement.get("gaps").cloned().unwrap_or_else(|| json!([])),
                "next_actions": requirement.get("next_actions").cloned().unwrap_or_else(|| json!([])),
            })
        })
        .collect::<Vec<_>>();
    let ready_for_core_architecture =
        overall_status == "ready" && incomplete_requirements.is_empty();

    json!({
        "status": if ready_for_core_architecture { "complete" } else { "incomplete" },
        "ready_for_core_architecture": ready_for_core_architecture,
        "core_architecture_is_proven_by_current_snapshot": ready_for_core_architecture,
        "summary": summary,
        "criteria": [
            "A local project and workflow must be materialized in the runtime snapshot.",
            "The node graph must produce executable plan steps and runtime tasks.",
            "Hermes/Agent control must have at least one auditable session or transcript.",
            "Required AI media, 3DGS, and software adapters must be registered with machine-readable contracts.",
            "Unreal-first assembly, local indexed assets, and video/game/interactive-art output manifests must be represented.",
            "Production evidence gaps are tracked separately and must not be counted as core architecture blockers."
        ],
        "incomplete_requirements": incomplete_requirements,
        "proof_commands": {
            "core_architecture_gate": format!("pool-cli --project {project_slug} core-architecture-gate --require-ready"),
            "core_architecture_readiness": format!("pool-cli --project {project_slug} core-architecture-readiness"),
            "core_architecture_package": format!("pool-cli --project {project_slug} core-architecture-package --output-dir worlds/{project_slug}/output --include-snapshot"),
            "core_architecture_packages": format!("pool-cli --project {project_slug} core-architecture-packages"),
            "core_architecture_smoke": "cargo run -q -p pool-core --example run_prd_readiness_smoke -- target/core-architecture-readiness-smoke",
            "runtime_workflow_probe": format!("pool-cli --project {project_slug} run-workflow --title \"Core architecture PRD probe\" --prompt \"core architecture verification\" --agent-mode stage --three-dgs-mode mock --unreal-mode mock"),
            "runtime_handoff": format!("pool-cli --project {project_slug} runtime-handoff"),
            "runtime_handoff_packages": format!("pool-cli --project {project_slug} runtime-handoff-packages"),
            "strict_prd_completion_gate": format!("pool-cli --project {project_slug} prd-completion-gate --require-complete")
        },
    })
}

fn has_software_action_for(snapshot: &RuntimeSnapshot, adapter_id: &str) -> bool {
    snapshot
        .software_actions
        .iter()
        .any(|action| action.adapter_id == adapter_id)
}

pub fn runtime_preflight_resource(snapshot: &RuntimeSnapshot) -> Result<Value> {
    let graph = runtime_graph_resource(snapshot)?;
    let graph_nodes = graph
        .get("workflows")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|workflow| workflow.get("nodes"))
        .filter_map(Value::as_array)
        .flatten()
        .cloned()
        .collect::<Vec<_>>();
    let runnable_nodes = graph_nodes
        .iter()
        .filter(|node| {
            node.get("can_run")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
        .cloned()
        .collect::<Vec<_>>();
    let blocked_nodes = graph_nodes
        .iter()
        .filter(|node| {
            node.get("blocked_by_approval")
                .and_then(Value::as_bool)
                .unwrap_or(false)
                || matches!(
                    node.get("status").and_then(Value::as_str),
                    Some("WaitingApproval" | "Failed")
                )
        })
        .cloned()
        .collect::<Vec<_>>();
    let approval_gates = snapshot
        .tasks
        .iter()
        .filter(|task| task.requires_approval || task.status == "WaitingApproval")
        .collect::<Vec<_>>();
    let failed_tasks = snapshot
        .tasks
        .iter()
        .filter(|task| matches!(task.status.as_str(), "Failed" | "Retryable" | "Cancelled"))
        .collect::<Vec<_>>();
    let missing_credentials = runtime_missing_provider_credentials(snapshot);
    let desktop_requests = snapshot
        .software_actions
        .iter()
        .filter(|action| is_open_desktop_recognition_request(action))
        .map(|action| desktop_recognition_action_value(action))
        .collect::<Vec<_>>();

    let mut checks = Vec::new();
    checks.push(preflight_check(
        "workflow_graph",
        if graph_nodes.is_empty() {
            "blocked"
        } else {
            "passed"
        },
        "Workflow graph",
        if graph_nodes.is_empty() {
            "No executable workflow nodes are available."
        } else {
            "Executable workflow graph is available."
        },
        json!({
            "nodes": graph_nodes.len(),
            "runnable_nodes": runnable_nodes.len(),
            "blocked_nodes": blocked_nodes.len(),
        }),
        None,
    ));
    checks.push(preflight_check(
        "approval_gates",
        if approval_gates.is_empty() {
            "passed"
        } else {
            "blocked"
        },
        "Approval gates",
        if approval_gates.is_empty() {
            "No task is blocked by manual approval."
        } else {
            "One or more high-cost or confirmation-gated tasks need approval."
        },
        json!({ "tasks": approval_gates.clone() }),
        Some("Approve or cancel waiting tasks before running the full workflow."),
    ));
    checks.push(preflight_check(
        "provider_credentials",
        if missing_credentials.is_empty() {
            "passed"
        } else {
            "warning"
        },
        "Provider credentials",
        if missing_credentials.is_empty() {
            "Tracked runtime providers have sanitized credential state."
        } else {
            "Some tracked providers do not have a saved runtime credential."
        },
        json!({ "providers": missing_credentials.clone() }),
        Some("Save provider keys or pass credentials through the provider run request/env."),
    ));
    checks.push(preflight_check(
        "failed_tasks",
        if failed_tasks.is_empty() {
            "passed"
        } else {
            "blocked"
        },
        "Failed or retryable tasks",
        if failed_tasks.is_empty() {
            "No failed, retryable, or cancelled task needs operator action."
        } else {
            "Failed, retryable, or cancelled tasks need retry or cancellation review."
        },
        json!({ "tasks": failed_tasks.clone() }),
        Some("Retry failed tasks or inspect their provider/software ledger."),
    ));
    checks.push(preflight_check(
        "desktop_recognition",
        if desktop_requests.is_empty() {
            "passed"
        } else {
            "warning"
        },
        "Desktop recognition handoff",
        if desktop_requests.is_empty() {
            "No open desktop recognition handoff is waiting."
        } else {
            "One or more software actions are waiting for desktop recognition/controller handoff."
        },
        json!({ "requests": desktop_requests.clone() }),
        Some("Run the desktop recognition controller or mark the handoff result."),
    ));
    checks.push(preflight_check(
        "agent_budget",
        if snapshot
            .stats
            .budget_remaining
            .is_some_and(|remaining| remaining < 0)
        {
            "warning"
        } else {
            "passed"
        },
        "Agent token budget",
        if snapshot
            .stats
            .budget_remaining
            .is_some_and(|remaining| remaining < 0)
        {
            "Agent sessions have exceeded the configured token budget."
        } else {
            "Agent token budget is unset or still within range."
        },
        json!({
            "agent_token_used": snapshot.stats.agent_token_used,
            "agent_token_budget": snapshot.stats.agent_token_budget,
            "budget_remaining": snapshot.stats.budget_remaining,
        }),
        Some("Review Agent sessions or raise the token budget for the next run."),
    ));

    let blocked = preflight_count(&checks, "blocked");
    let warnings = preflight_count(&checks, "warning");
    let passed = preflight_count(&checks, "passed");
    let next_actions = runtime_preflight_next_actions(snapshot, &approval_gates, &failed_tasks);

    Ok(json!({
        "project_filter": snapshot.project_filter,
        "generated_at": snapshot.generated_at,
        "ready": blocked == 0,
        "summary": {
            "blocked": blocked,
            "warnings": warnings,
            "passed": passed,
            "checks": checks.len(),
            "runnable_nodes": runnable_nodes.len(),
            "blocked_nodes": blocked_nodes.len(),
            "approval_gates": approval_gates.len(),
            "missing_credentials": missing_credentials.len(),
            "desktop_handoffs": desktop_requests.len(),
            "failed_tasks": failed_tasks.len(),
        },
        "checks": checks,
        "next_actions": next_actions,
        "runnable_nodes": runnable_nodes,
        "blocked_nodes": blocked_nodes,
    }))
}

pub fn runtime_handoff_resource(snapshot: &RuntimeSnapshot) -> Result<Value> {
    let preflight = runtime_preflight_resource(snapshot)?;
    let project_slug = snapshot_preflight_project(snapshot);
    let ready = preflight
        .get("ready")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let next_actions = preflight
        .get("next_actions")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let runnable_nodes = preflight
        .get("runnable_nodes")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let desktop_requests = snapshot
        .software_actions
        .iter()
        .filter(|action| is_open_desktop_recognition_request(action))
        .map(|action| desktop_recognition_action_value(action))
        .collect::<Vec<_>>();

    let approval_actions = handoff_actions_by_kind(&next_actions, "approval");
    let retry_actions = handoff_actions_by_kind(&next_actions, "retry");
    let credential_actions = handoff_actions_by_kind(&next_actions, "credential");
    let desktop_actions = handoff_actions_by_kind(&next_actions, "desktop_recognition");
    let local_worker_actions = handoff_actions_by_kind(&next_actions, "local_worker_self_check");
    let handoff_package_actions = vec![runtime_handoff_package_action(&project_slug)];
    let node_actions = runnable_nodes
        .iter()
        .take(6)
        .filter_map(|node| handoff_run_node_action(node, &project_slug))
        .collect::<Vec<_>>();
    let approval_action_count = approval_actions.len();
    let retry_action_count = retry_actions.len();
    let credential_action_count = credential_actions.len();
    let local_worker_action_count = local_worker_actions.len();
    let handoff_package_action_count = handoff_package_actions.len();
    let desktop_request_count = desktop_requests.len();
    let runnable_node_action_count = node_actions.len();

    let mut lanes = Vec::new();
    lanes.push(json!({
        "id": "agent_context",
        "title": "Agent/Hermes context load",
        "team_role": "agent_operator",
        "executor": "hermes_or_agent_cli",
        "status": "ready",
        "resources": [
            "pool://runtime-preflight",
            "pool://runtime-graph",
            "pool://tasks",
            "pool://assets",
            "pool://provider-requests",
            "pool://software-actions",
            "pool://desktop-recognition"
        ],
        "commands": [
            format!("pool-cli --project {} runtime-preflight", project_slug),
            format!("pool-cli --project {} runtime-graph", project_slug),
            format!("pool-cli --project {} workflow-context", project_slug)
        ],
    }));
    lanes.push(json!({
        "id": "local_worker_smoke",
        "title": "Local worker bridge self-checks",
        "team_role": "agent_operator",
        "executor": "hermes_or_agent_cli",
        "status": "ready",
        "actions": local_worker_actions.clone(),
    }));
    lanes.push(json!({
        "id": "handoff_package",
        "title": "Offline handoff package",
        "team_role": "agent_operator",
        "executor": "hermes_or_agent_cli",
        "status": "ready",
        "actions": handoff_package_actions.clone(),
    }));
    lanes.push(json!({
        "id": "manual_approval",
        "title": "Approval gates",
        "team_role": "creative_director",
        "executor": "human_operator_or_approved_agent",
        "status": if approval_actions.is_empty() { "clear" } else { "blocked" },
        "actions": approval_actions.clone(),
    }));
    lanes.push(json!({
        "id": "failed_task_recovery",
        "title": "Failed task recovery",
        "team_role": "agent_operator",
        "executor": "operator_or_agent_cli",
        "status": if retry_actions.is_empty() { "clear" } else { "blocked" },
        "actions": retry_actions.clone(),
    }));
    lanes.push(json!({
        "id": "credential_setup",
        "title": "Provider credentials",
        "team_role": "generation_td",
        "executor": "operator",
        "status": if credential_actions.is_empty() { "clear" } else { "warning" },
        "actions": credential_actions.clone(),
    }));
    lanes.push(json!({
        "id": "desktop_recognition",
        "title": "Desktop recognition handoff",
        "team_role": "engine_integrator",
        "executor": "desktop_controller_or_human_takeover",
        "status": if desktop_requests.is_empty() { "clear" } else { "waiting_handoff" },
        "actions": desktop_actions.clone(),
        "requests": desktop_requests.clone(),
    }));
    lanes.push(json!({
        "id": "runnable_nodes",
        "title": "Runnable workflow nodes",
        "team_role": "output_operator",
        "executor": "pool_cli_or_runtime_http",
        "status": if ready { "ready" } else { "gated" },
        "actions": node_actions.clone(),
    }));
    let team_roles = runtime_team_roles(&lanes);

    let commands = lanes
        .iter()
        .flat_map(handoff_lane_commands)
        .collect::<Vec<_>>();

    Ok(json!({
        "project_filter": snapshot.project_filter,
        "generated_at": snapshot.generated_at,
        "ready": ready,
        "summary": {
            "lanes": lanes.len(),
            "commands": commands.len(),
            "approval_actions": approval_action_count,
            "retry_actions": retry_action_count,
            "credential_actions": credential_action_count,
            "local_worker_actions": local_worker_action_count,
            "handoff_package_actions": handoff_package_action_count,
            "desktop_requests": desktop_request_count,
            "runnable_node_actions": runnable_node_action_count,
            "team_roles": team_roles.len(),
        },
        "team": {
            "size": team_roles.len(),
            "mode": "five_person_content_burst_team",
            "roles": team_roles,
        },
        "control_priority": [
            "API/MCP",
            "Skills/CLI",
            "Desktop Recognition",
            "Human Takeover"
        ],
        "preflight": {
            "ready": ready,
            "summary": preflight.get("summary").cloned().unwrap_or_else(|| json!({})),
            "next_actions": next_actions,
        },
        "lanes": lanes,
        "commands": commands,
        "mcp_resources": [
            "pool://runtime-handoff",
            "pool://runtime-preflight",
            "pool://runtime-graph",
            "pool://workflow",
            "pool://node-context",
            "pool://provider-gateway-worker",
            "pool://software-contracts",
            "pool://desktop-recognition"
        ],
    }))
}

fn runtime_team_roles(lanes: &[Value]) -> Vec<Value> {
    [
        (
            "creative_director",
            "Creative Director",
            "创意验收、审批门和参考方向把关",
            "human_approval",
            vec!["manual_approval"],
        ),
        (
            "agent_operator",
            "Agent Operator",
            "Hermes/Agent CLI 上下文读取、失败恢复和自动化调度",
            "agent_cli_mcp",
            vec![
                "agent_context",
                "local_worker_smoke",
                "handoff_package",
                "failed_task_recovery",
            ],
        ),
        (
            "generation_td",
            "AI / 3DGS TD",
            "AI 图片、视频、音频与 3DGS Provider 凭证和生成队列",
            "provider_gateway",
            vec!["credential_setup"],
        ),
        (
            "engine_integrator",
            "Engine Integrator",
            "Unreal/Unity/Blender/TouchDesigner 等外部软件接管",
            "software_control",
            vec!["desktop_recognition"],
        ),
        (
            "output_operator",
            "Output Operator",
            "视频、游戏和交互艺术输出节点推进",
            "runtime_execution",
            vec!["runnable_nodes"],
        ),
    ]
    .into_iter()
    .map(|(id, title, focus, primary_surface, assigned_lane_ids)| {
        let assigned_lanes = assigned_lane_ids
            .iter()
            .filter_map(|lane_id| {
                lanes
                    .iter()
                    .find(|lane| lane.get("id").and_then(Value::as_str) == Some(*lane_id))
            })
            .collect::<Vec<_>>();
        let queue_count = assigned_lanes
            .iter()
            .map(|lane| {
                lane.get("actions")
                    .and_then(Value::as_array)
                    .map(Vec::len)
                    .unwrap_or(0)
                    + lane
                        .get("requests")
                        .and_then(Value::as_array)
                        .map(Vec::len)
                        .unwrap_or(0)
            })
            .sum::<usize>();
        let status = team_role_status(&assigned_lanes);
        let lane_summaries = assigned_lanes
            .iter()
            .map(|lane| {
                json!({
                    "id": lane.get("id").and_then(Value::as_str).unwrap_or("lane"),
                    "title": lane.get("title").and_then(Value::as_str).unwrap_or("Handoff lane"),
                    "status": lane.get("status").and_then(Value::as_str).unwrap_or("ready"),
                    "executor": lane.get("executor").and_then(Value::as_str).unwrap_or("operator"),
                })
            })
            .collect::<Vec<_>>();

        json!({
            "id": id,
            "title": title,
            "focus": focus,
            "primary_surface": primary_surface,
            "status": status,
            "queue_count": queue_count,
            "assigned_lane_ids": assigned_lane_ids,
            "lanes": lane_summaries,
        })
    })
    .collect()
}

fn team_role_status(lanes: &[&Value]) -> &'static str {
    if lanes
        .iter()
        .any(|lane| lane.get("status").and_then(Value::as_str) == Some("blocked"))
    {
        "blocked"
    } else if lanes.iter().any(|lane| {
        matches!(
            lane.get("status").and_then(Value::as_str),
            Some("waiting_handoff" | "gated" | "warning")
        )
    }) {
        "attention"
    } else {
        "ready"
    }
}

fn preflight_check(
    id: &str,
    status: &str,
    title: &str,
    detail: &str,
    evidence: Value,
    action: Option<&str>,
) -> Value {
    json!({
        "id": id,
        "status": status,
        "title": title,
        "detail": detail,
        "evidence": evidence,
        "action": action,
    })
}

fn preflight_count(checks: &[Value], status: &str) -> usize {
    checks
        .iter()
        .filter(|check| check.get("status").and_then(Value::as_str) == Some(status))
        .count()
}

fn runtime_missing_provider_credentials(snapshot: &RuntimeSnapshot) -> Vec<Value> {
    let configured = snapshot
        .api_keys
        .iter()
        .filter(|key| key.service_type == "provider" && key.configured)
        .map(|key| key.provider.as_str())
        .collect::<BTreeSet<_>>();
    let mut provider_ids = snapshot
        .tasks
        .iter()
        .filter_map(|task| task.provider_id.as_deref())
        .collect::<BTreeSet<_>>();
    provider_ids.extend(
        snapshot
            .provider_requests
            .iter()
            .map(|request| request.provider_id.as_str()),
    );

    provider_ids
        .into_iter()
        .filter(|provider_id| !provider_id.trim().is_empty())
        .filter(|provider_id| provider_requires_preflight_credential(provider_id))
        .filter(|provider_id| !configured.contains(provider_id))
        .map(|provider_id| {
            json!({
                "provider_id": provider_id,
                "suggested_env": suggested_provider_env(provider_id),
            })
        })
        .collect()
}

fn provider_requires_preflight_credential(provider_id: &str) -> bool {
    !matches!(provider_id, "mock-3dgs" | "comfyui" | "sam-3d")
}

fn suggested_provider_env(provider_id: &str) -> &'static str {
    match provider_id {
        "openai-image-2" => "OPENAI_API_KEY",
        "kling" => "POOL_KLING_API_KEY",
        "midjourney" => "MIDJOURNEY_API_KEY",
        "nano-banana-pro" => "NANO_BANANA_KEY",
        "suno" => "SUNO_KEY",
        "worldlabs-marble" => "WORLD_LABS_API_KEY",
        "tripo-splat" => "TRIPOSPLAT_KEY",
        "spark-3dgs" => "SPARK_KEY",
        "qunhe-3d" => "QUNHE_TOKEN",
        _ => "PROVIDER_API_KEY",
    }
}

fn runtime_preflight_next_actions(
    snapshot: &RuntimeSnapshot,
    approval_gates: &[&TaskSnapshot],
    failed_tasks: &[&TaskSnapshot],
) -> Vec<Value> {
    let mut actions = Vec::new();
    for task in approval_gates.iter().take(5) {
        actions.push(json!({
            "kind": "approval",
            "title": format!("Approve task: {}", task.title),
            "task_id": task.id,
            "command": format!("pool-cli --project {} approve-task {}", task.project_slug, task.id),
        }));
    }
    for task in failed_tasks.iter().take(5) {
        actions.push(json!({
            "kind": "retry",
            "title": format!("Retry task: {}", task.title),
            "task_id": task.id,
            "command": format!("pool-cli --project {} retry-task {}", task.project_slug, task.id),
        }));
    }
    for provider in runtime_missing_provider_credentials(snapshot)
        .into_iter()
        .take(5)
    {
        let provider_id = provider
            .get("provider_id")
            .and_then(Value::as_str)
            .unwrap_or("provider");
        let env = provider
            .get("suggested_env")
            .and_then(Value::as_str)
            .unwrap_or("PROVIDER_API_KEY");
        actions.push(json!({
            "kind": "credential",
            "title": format!("Save Provider credential: {provider_id}"),
            "provider_id": provider_id,
            "command": format!(
                "pool-cli --project {} set-api-key {} --api-key-env {}",
                snapshot_preflight_project(snapshot),
                provider_id,
                env
            ),
        }));
    }
    let project_slug = snapshot_preflight_project(snapshot);
    actions.push(json!({
        "kind": "local_worker_self_check",
        "title": "Run local Provider/Hermes/software worker self-checks",
        "command": "pool-cli worker-self-checks --output-root target/pool-worker-self-checks --software-adapter resolve",
        "mcp_tool": "pool_worker_self_checks",
        "mcp_arguments": {
            "output_root": "target/pool-worker-self-checks",
            "software_adapter": "resolve"
        },
        "inspect_commands": [
            format!("pool-cli --project {project_slug} provider-gateway-worker-contract"),
            format!("pool-cli --project {project_slug} unreal-mcp-bridge"),
            format!("pool-cli --project {project_slug} software-contracts resolve")
        ],
        "optional": true,
        "reason": "Run before handing tasks to external AI media/3DGS workers, Hermes MCP, Unreal MCP, or generic software API bridge workers."
    }));
    if snapshot
        .software_actions
        .iter()
        .any(|action| is_open_desktop_recognition_request(action))
    {
        let project_slug = snapshot_preflight_project(snapshot);
        actions.push(json!({
            "kind": "desktop_recognition",
            "title": "Run desktop recognition controller",
            "command": format!("pool-cli --project {project_slug} desktop-run-next --controller-id local-vision-dry-run --status succeeded"),
            "inspect_command": format!("pool-cli --project {project_slug} desktop-requests"),
            "mode": "dry_run_controller",
        }));
    }
    actions
}

fn snapshot_preflight_project(snapshot: &RuntimeSnapshot) -> String {
    snapshot
        .project_filter
        .as_deref()
        .filter(|project| !project.trim().is_empty() && *project != "*")
        .map(ToString::to_string)
        .or_else(|| {
            snapshot
                .projects
                .first()
                .map(|project| project.slug.clone())
        })
        .unwrap_or_else(|| "demo".to_string())
}

fn handoff_actions_by_kind(actions: &[Value], kind: &str) -> Vec<Value> {
    actions
        .iter()
        .filter(|action| action.get("kind").and_then(Value::as_str) == Some(kind))
        .cloned()
        .collect()
}

fn runtime_handoff_package_action(project_slug: &str) -> Value {
    let output_dir = format!("worlds/{project_slug}/output");
    json!({
        "kind": "handoff_package",
        "title": "Write runtime handoff package",
        "command": format!(
            "pool-cli --project {project_slug} handoff-package --node-id agent --output-dir {output_dir} --include-snapshot"
        ),
        "mcp_tool": "pool_handoff_package",
        "mcp_arguments": {
            "project_slug": project_slug,
            "node_id": "agent",
            "output_dir": output_dir,
            "include_snapshot": true
        },
        "artifacts": [
            format!("{output_dir}/control/handoff/1-runtime-handoff.json"),
            format!("{output_dir}/control/handoff/2-runtime-preflight.json"),
            format!("{output_dir}/control/handoff/3-runtime-graph.json"),
            format!("{output_dir}/control/handoff/5-worker-self-checks.sh"),
            format!("{output_dir}/control/handoff/6-worker-self-checks-preflight.json"),
            format!("{output_dir}/control/handoff/7-integration-readiness.json"),
            format!("{output_dir}/control/handoff/8-runtime-handoff-package-manifest.json")
        ],
        "reason": "Create a local package that can be handed to Hermes, Agent CLI, desktop controller, or a human operator without re-querying the live runtime."
    })
}

fn handoff_run_node_action(node: &Value, project_slug: &str) -> Option<Value> {
    let node_id = node.get("id").and_then(Value::as_str)?;
    let title = node.get("title").and_then(Value::as_str).unwrap_or(node_id);
    let task_type = node
        .get("task_type")
        .and_then(Value::as_str)
        .unwrap_or("node");
    Some(json!({
        "kind": "run_node",
        "title": format!("Run node: {title}"),
        "node_id": node_id,
        "task_type": task_type,
        "command": format!("pool-cli --project {project_slug} run-node {node_id}"),
        "mcp_tool": "pool_run_node",
    }))
}

fn handoff_lane_commands(lane: &Value) -> Vec<Value> {
    let lane_id = lane.get("id").and_then(Value::as_str).unwrap_or("handoff");
    let string_commands = lane
        .get("commands")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(|command| {
            json!({
                "lane": lane_id,
                "command": command,
            })
        });
    let action_commands = lane
        .get("actions")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|action| {
            action
                .get("command")
                .and_then(Value::as_str)
                .map(|command| {
                    json!({
                        "lane": lane_id,
                        "kind": action.get("kind").and_then(Value::as_str).unwrap_or("action"),
                        "title": action.get("title").and_then(Value::as_str).unwrap_or("Action"),
                        "command": command,
                    })
                })
        });

    string_commands.chain(action_commands).collect()
}

pub fn runtime_workflow_context_index_resource(snapshot: &RuntimeSnapshot) -> Result<Value> {
    let workflows = snapshot
        .workflows
        .iter()
        .map(|workflow| {
            let nodes: BTreeMap<String, WorkflowNode> =
                serde_json::from_value(workflow.nodes.clone())
                    .with_context(|| format!("parse workflow nodes for {}", workflow.id))?;
            let connections: Vec<WorkflowConnection> =
                serde_json::from_value(workflow.connections.clone())
                    .with_context(|| format!("parse workflow connections for {}", workflow.id))?;
            let node_ids = nodes.keys().cloned().collect::<BTreeSet<_>>();
            let tasks = snapshot
                .tasks
                .iter()
                .filter(|task| {
                    task.node_id
                        .as_deref()
                        .is_some_and(|node_id| node_ids.contains(node_id))
                })
                .collect::<Vec<_>>();

            Ok(json!({
                "workflow_id": workflow.id.clone(),
                "project_id": workflow.project_id.clone(),
                "shot_id": workflow.shot_id.clone(),
                "name": workflow.name.clone(),
                "mcp_uri": format!("pool://workflow/{}", workflow.id),
                "http_path": format!("/api/workflow-context?workflow_id={}", workflow.id),
                "created_at": workflow.created_at.clone(),
                "updated_at": workflow.updated_at.clone(),
                "summary": {
                    "nodes": nodes.len(),
                    "edges": connections.len(),
                    "tasks": tasks.len(),
                    "waiting_approval": tasks
                        .iter()
                        .filter(|task| task.status == "WaitingApproval")
                        .count(),
                    "running": tasks
                        .iter()
                        .filter(|task| task.status == "Running")
                        .count(),
                    "failed": tasks
                        .iter()
                        .filter(|task| task.status == "Failed")
                        .count(),
                    "blocked_by_approval": tasks
                        .iter()
                        .any(|task| task.requires_approval || task.status == "WaitingApproval"),
                },
            }))
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(json!({
        "project_filter": snapshot.project_filter,
        "generated_at": snapshot.generated_at,
        "summary": {
            "workflows": workflows.len(),
        },
        "workflows": workflows,
    }))
}

pub fn runtime_graph_resource(snapshot: &RuntimeSnapshot) -> Result<Value> {
    let workflows = snapshot
        .workflows
        .iter()
        .map(|workflow| runtime_graph_workflow(snapshot, workflow))
        .collect::<Result<Vec<_>>>()?;
    let nodes = workflows
        .iter()
        .filter_map(|workflow| workflow.get("nodes"))
        .filter_map(Value::as_array)
        .map(Vec::len)
        .sum::<usize>();
    let edges = workflows
        .iter()
        .filter_map(|workflow| workflow.get("edges"))
        .filter_map(Value::as_array)
        .map(Vec::len)
        .sum::<usize>();

    Ok(json!({
        "project_filter": snapshot.project_filter,
        "generated_at": snapshot.generated_at,
        "summary": {
            "workflows": workflows.len(),
            "nodes": nodes,
            "edges": edges,
            "waiting_approval": snapshot.stats.waiting_approval,
            "running": snapshot.stats.running,
            "failed": snapshot.stats.failed,
        },
        "workflows": workflows,
    }))
}

pub fn runtime_execution_plan_resource(snapshot: &RuntimeSnapshot) -> Result<Value> {
    let workflows = snapshot
        .workflows
        .iter()
        .map(|workflow| runtime_execution_workflow(snapshot, workflow))
        .collect::<Result<Vec<_>>>()?;
    let steps = workflows
        .iter()
        .filter_map(|workflow| workflow.get("steps"))
        .filter_map(Value::as_array)
        .flatten()
        .cloned()
        .collect::<Vec<_>>();
    let runnable_steps = steps
        .iter()
        .filter(|step| {
            step.get("phase")
                .and_then(Value::as_str)
                .is_some_and(|phase| phase == "ready")
        })
        .count();
    let gated_steps = steps
        .iter()
        .filter(|step| {
            step.get("gate")
                .and_then(|gate| gate.get("kind"))
                .and_then(Value::as_str)
                .is_some_and(|kind| kind != "none")
        })
        .count();
    let next_steps = steps
        .iter()
        .filter(|step| {
            !matches!(
                step.get("phase").and_then(Value::as_str),
                Some("complete" | "running")
            )
        })
        .take(8)
        .cloned()
        .collect::<Vec<_>>();

    Ok(json!({
        "project_filter": snapshot.project_filter,
        "generated_at": snapshot.generated_at,
        "kind": "pool_runtime_execution_plan",
        "version": 1,
        "summary": {
            "workflows": workflows.len(),
            "steps": steps.len(),
            "runnable_steps": runnable_steps,
            "gated_steps": gated_steps,
            "phase_counts": count_by(
                steps
                    .iter()
                    .filter_map(|step| step.get("phase"))
                    .filter_map(Value::as_str)
            ),
            "task_type_counts": count_by(
                steps
                    .iter()
                    .filter_map(|step| step.get("task_type"))
                    .filter_map(Value::as_str)
            ),
        },
        "policy": {
            "graph_is_execution_source": true,
            "local_files_authoritative": true,
            "provider_urls_are_provenance": true,
            "high_cost_steps_require_approval": true,
            "control_priority": "API/MCP > Skills/CLI > Desktop Recognition > Human Takeover",
        },
        "next_steps": next_steps,
        "workflows": workflows,
        "mcp_resources": [
            "pool://runtime-execution-plan",
            "pool://runtime-graph",
            "pool://runtime-preflight",
            "pool://runtime-handoff",
            "pool://workflow",
            "pool://node-context",
            "pool://provider-contracts",
            "pool://software-contracts"
        ],
    }))
}

pub fn runtime_node_context_index_resource(snapshot: &RuntimeSnapshot) -> Result<Value> {
    let mut nodes = Vec::new();

    for workflow in &snapshot.workflows {
        let workflow_nodes: BTreeMap<String, WorkflowNode> =
            serde_json::from_value(workflow.nodes.clone())
                .with_context(|| format!("parse workflow nodes for {}", workflow.id))?;

        for node in workflow_nodes.values() {
            nodes.push(json!({
                "node_id": node.id.clone(),
                "workflow_id": workflow.id.clone(),
                "title": node.title.clone(),
                "node_type": node_type_name(&node.node_type),
                "task_type": runtime_task_type(node),
                "status": runtime_node_status(snapshot, node),
                "mcp_uri": format!("pool://node-context/{}", node.id),
                "http_path": format!("/api/node-context?node_id={}", node.id),
            }));
        }
    }

    Ok(json!({
        "project_filter": snapshot.project_filter,
        "generated_at": snapshot.generated_at,
        "summary": {
            "nodes": nodes.len(),
            "task_types": count_by(
                nodes
                    .iter()
                    .filter_map(|node| node.get("task_type"))
                    .filter_map(Value::as_str)
            ),
        },
        "nodes": nodes,
    }))
}

pub fn runtime_node_context_resource(snapshot: &RuntimeSnapshot, node_id: &str) -> Result<Value> {
    let node_id = node_id.trim();
    if node_id.is_empty() {
        bail!("node id is required for pool://node-context/<node-id>");
    }

    for workflow in &snapshot.workflows {
        let nodes: BTreeMap<String, WorkflowNode> = serde_json::from_value(workflow.nodes.clone())
            .with_context(|| format!("parse workflow nodes for {}", workflow.id))?;
        let Some(node) = nodes.get(node_id) else {
            continue;
        };
        let connections: Vec<WorkflowConnection> =
            serde_json::from_value(workflow.connections.clone())
                .with_context(|| format!("parse workflow connections for {}", workflow.id))?;

        let graph_node = runtime_graph_node(snapshot, node);
        let incoming_edges = connections
            .iter()
            .filter(|connection| connection.to_node_id == node.id)
            .map(|connection| runtime_graph_edge(snapshot, &nodes, connection))
            .collect::<Vec<_>>();
        let outgoing_edges = connections
            .iter()
            .filter(|connection| connection.from_node_id == node.id)
            .map(|connection| runtime_graph_edge(snapshot, &nodes, connection))
            .collect::<Vec<_>>();
        let tasks = snapshot
            .tasks
            .iter()
            .filter(|task| task.node_id.as_deref() == Some(node.id.as_str()))
            .collect::<Vec<_>>();
        let task_ids = tasks
            .iter()
            .map(|task| task.id.as_str())
            .collect::<Vec<_>>();
        let assets = snapshot
            .assets
            .iter()
            .filter(|asset| asset.source_node_id.as_deref() == Some(node.id.as_str()))
            .collect::<Vec<_>>();
        let provider_requests = snapshot
            .provider_requests
            .iter()
            .filter(|request| task_ids.contains(&request.task_id.as_str()))
            .collect::<Vec<_>>();
        let software_actions = snapshot
            .software_actions
            .iter()
            .filter(|action| {
                action
                    .task_id
                    .as_deref()
                    .is_some_and(|task_id| task_ids.contains(&task_id))
            })
            .collect::<Vec<_>>();
        let node_states = snapshot
            .node_states
            .iter()
            .filter(|state| state.node_id == node.id)
            .collect::<Vec<_>>();
        let agent_sessions = agent_sessions_for_node(snapshot, workflow, node, &tasks);
        let blocked_by_approval = graph_node
            .get("blocked_by_approval")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let project_slug = project_slug_for_workflow(snapshot, workflow)
            .or(snapshot.project_filter.as_deref())
            .or_else(|| {
                snapshot
                    .projects
                    .first()
                    .map(|project| project.slug.as_str())
            })
            .unwrap_or("demo");
        let control_context = runtime_node_control_context(node, project_slug);

        return Ok(json!({
            "project_filter": snapshot.project_filter,
            "generated_at": snapshot.generated_at,
            "node_id": node.id.clone(),
            "workflow_id": workflow.id.clone(),
            "workflow": {
                "id": workflow.id.clone(),
                "project_id": workflow.project_id.clone(),
                "shot_id": workflow.shot_id.clone(),
                "name": workflow.name.clone(),
            },
            "node": graph_node,
            "node_states": node_states,
            "incoming_edges": incoming_edges,
            "outgoing_edges": outgoing_edges,
            "tasks": tasks,
            "assets": assets,
            "provider_requests": provider_requests,
            "software_actions": software_actions,
            "agent_sessions": agent_sessions,
            "control_context": control_context,
            "summary": {
                "tasks": tasks.len(),
                "assets": assets.len(),
                "provider_requests": provider_requests.len(),
                "software_actions": software_actions.len(),
                "agent_sessions": agent_sessions.len(),
                "node_states": node_states.len(),
                "incoming_edges": incoming_edges.len(),
                "outgoing_edges": outgoing_edges.len(),
                "blocked_by_approval": blocked_by_approval,
            },
        }));
    }

    bail!("unknown workflow node: {node_id}")
}

fn runtime_node_control_context(node: &WorkflowNode, project_slug: &str) -> Value {
    let provider_registry = ProviderRegistry::defaults();
    let software_registry = SoftwareAdapterRegistry::defaults();
    let provider_id = node
        .provider_id
        .as_deref()
        .filter(|provider_id| !provider_id.trim().is_empty());
    let software_adapter_id = node
        .software_adapter_id
        .as_deref()
        .filter(|adapter_id| !adapter_id.trim().is_empty());
    let provider_config = provider_id.and_then(|provider_id| provider_registry.get(provider_id));
    let software_config =
        software_adapter_id.and_then(|adapter_id| software_registry.get(adapter_id));
    let action_kind =
        software_adapter_id.map(|adapter_id| node_context_software_action_kind(node, adapter_id));
    let output_dir = format!("worlds/{project_slug}/output");
    let mut cli_commands = Vec::new();
    let mut mcp_tools = Vec::new();

    if let Some(provider_id) = provider_id {
        if provider_config.is_some() {
            cli_commands.push(json!({
                "kind": "provider_health",
                "command": format!("pool-cli --project {project_slug} provider-health {provider_id} --execution-mode auto"),
            }));
            cli_commands.push(json!({
                "kind": "provider_run",
                "command": format!("pool-cli --project {project_slug} run-provider {provider_id} --execution-mode auto --node-id {} --output-dir {output_dir}", node.id),
            }));
            mcp_tools.push(json!({
                "name": "pool_provider_health",
                "arguments": {
                    "project_slug": project_slug,
                    "provider_id": provider_id,
                    "execution_mode": "auto",
                },
            }));
            mcp_tools.push(json!({
                "name": "pool_run_provider",
                "arguments": {
                    "project_slug": project_slug,
                    "provider_id": provider_id,
                    "node_id": node.id,
                    "execution_mode": "auto",
                    "output_dir": output_dir,
                },
            }));
        } else if provider_id == "hermes" {
            cli_commands.push(json!({
                "kind": "agent_session",
                "command": format!("pool-cli --project {project_slug} agent-session hermes --instruction \"inspect node {} and coordinate next control action\" --allowed-tool api --allowed-tool mcp --allowed-tool cli", node.id),
            }));
            mcp_tools.push(json!({
                "name": "pool_agent_session",
                "arguments": {
                    "project_slug": project_slug,
                    "kind": "hermes",
                    "instruction": format!("inspect node {} and coordinate next control action", node.id),
                    "allowed_tools": ["api", "mcp", "cli"],
                },
            }));
        }
    }

    if let (Some(adapter_id), Some(action_kind)) = (software_adapter_id, action_kind) {
        cli_commands.push(json!({
            "kind": "software_health",
            "command": format!("pool-cli --project {project_slug} software-health {adapter_id} --priority ApiMcp"),
        }));
        cli_commands.push(json!({
            "kind": "software_action",
            "command": format!("pool-cli --project {project_slug} run-software {adapter_id} --action {action_kind} --priority ApiMcp --node-id {}", node.id),
        }));
        mcp_tools.push(json!({
            "name": "pool_software_health",
            "arguments": {
                "adapter_id": adapter_id,
                "priority": "ApiMcp",
            },
        }));
        mcp_tools.push(json!({
            "name": "pool_run_software",
            "arguments": {
                "project_slug": project_slug,
                "adapter_id": adapter_id,
                "node_id": node.id,
                "action_kind": action_kind,
                "priority": "ApiMcp",
            },
        }));
    }

    json!({
        "project_slug": project_slug,
        "task_type": runtime_task_type(node),
        "provider": provider_id.map(|provider_id| json!({
            "id": provider_id,
            "registered": provider_config.is_some(),
            "config": provider_config,
        })),
        "software_adapter": software_adapter_id.map(|adapter_id| json!({
            "id": adapter_id,
            "registered": software_config.is_some(),
            "action_kind": action_kind,
            "config": software_config,
        })),
        "control_priority_chain": SoftwareAdapterRegistry::control_priority_chain(),
        "mcp_resources": [
            "pool://adapters",
            "pool://runtime-preflight",
            "pool://runtime-handoff",
            format!("pool://node-context/{}", node.id),
        ],
        "mcp_tools": mcp_tools,
        "cli_commands": cli_commands,
    })
}

fn node_context_software_action_kind(node: &WorkflowNode, adapter_id: &str) -> &'static str {
    match (node_type_name(&node.node_type), adapter_id) {
        ("Unreal", _) => "CreateScene",
        ("GameOutput", _) => "ExportBuild",
        ("VideoOutput", "resolve" | "editing-suite") => "Transcode",
        ("VideoOutput", _) => "Render",
        ("InteractiveOutput", _) => "RunViewport",
        ("Blender", _) | (_, "blender") => "ImportAsset",
        ("Nuke", _) | (_, "nuke") => "Render",
        ("TouchDesigner", _) | ("MadMapper", _) => "RunViewport",
        ("MotionCaptureDb", _) | (_, "motion-db") => "ImportAsset",
        _ => "HealthCheck",
    }
}

fn agent_sessions_for_workflow<'a>(
    snapshot: &'a RuntimeSnapshot,
    workflow: &'a WorkflowSnapshot,
    nodes: &BTreeMap<String, WorkflowNode>,
    tasks: &[&'a TaskSnapshot],
) -> Vec<&'a AgentSessionSnapshot> {
    if !nodes
        .values()
        .any(|node| runtime_task_type(node) == "agent")
    {
        return Vec::new();
    }

    let mut project_slugs = tasks
        .iter()
        .map(|task| task.project_slug.as_str())
        .collect::<BTreeSet<_>>();
    if let Some(project_slug) = project_slug_for_workflow(snapshot, workflow) {
        project_slugs.insert(project_slug);
    }
    if let Some(project_slug) = snapshot.project_filter.as_deref() {
        project_slugs.insert(project_slug);
    }
    if project_slugs.is_empty() && snapshot.projects.len() == 1 {
        project_slugs.insert(snapshot.projects[0].slug.as_str());
    }

    snapshot
        .agent_sessions
        .iter()
        .filter(|session| project_slugs.contains(session.project_slug.as_str()))
        .collect::<Vec<_>>()
}

fn agent_sessions_for_node<'a>(
    snapshot: &'a RuntimeSnapshot,
    workflow: &WorkflowSnapshot,
    node: &WorkflowNode,
    tasks: &[&'a TaskSnapshot],
) -> Vec<&'a AgentSessionSnapshot> {
    if runtime_task_type(node) != "agent" {
        return Vec::new();
    }

    let mut project_slugs = tasks
        .iter()
        .map(|task| task.project_slug.as_str())
        .collect::<Vec<_>>();
    if let Some(project_slug) = project_slug_for_workflow(snapshot, workflow) {
        project_slugs.push(project_slug);
    }
    if let Some(project_slug) = snapshot.project_filter.as_deref() {
        project_slugs.push(project_slug);
    }
    if project_slugs.is_empty() && snapshot.projects.len() == 1 {
        project_slugs.push(snapshot.projects[0].slug.as_str());
    }

    snapshot
        .agent_sessions
        .iter()
        .filter(|session| project_slugs.contains(&session.project_slug.as_str()))
        .collect::<Vec<_>>()
}

fn project_slug_for_workflow<'a>(
    snapshot: &'a RuntimeSnapshot,
    workflow: &WorkflowSnapshot,
) -> Option<&'a str> {
    let project_id = workflow.project_id.as_deref()?;
    snapshot
        .projects
        .iter()
        .find(|project| project.id == project_id)
        .map(|project| project.slug.as_str())
}

fn runtime_graph_workflow(
    snapshot: &RuntimeSnapshot,
    workflow: &WorkflowSnapshot,
) -> Result<Value> {
    let nodes: BTreeMap<String, WorkflowNode> = serde_json::from_value(workflow.nodes.clone())
        .with_context(|| format!("parse workflow nodes for {}", workflow.id))?;
    let connections: Vec<WorkflowConnection> = serde_json::from_value(workflow.connections.clone())
        .with_context(|| format!("parse workflow connections for {}", workflow.id))?;
    let graph_nodes = nodes
        .values()
        .map(|node| runtime_graph_node(snapshot, node))
        .collect::<Vec<_>>();
    let graph_edges = connections
        .iter()
        .map(|connection| runtime_graph_edge(snapshot, &nodes, connection))
        .collect::<Vec<_>>();

    Ok(json!({
        "workflow_id": workflow.id.clone(),
        "project_id": workflow.project_id.clone(),
        "shot_id": workflow.shot_id.clone(),
        "name": workflow.name.clone(),
        "created_at": workflow.created_at.clone(),
        "updated_at": workflow.updated_at.clone(),
        "summary": {
            "nodes": graph_nodes.len(),
            "edges": graph_edges.len(),
            "task_types": count_by(nodes.values().map(runtime_task_type)),
            "connection_kinds": count_by(connections.iter().map(|connection| connection_kind_name(&connection.kind))),
        },
        "nodes": graph_nodes,
        "edges": graph_edges,
    }))
}

fn runtime_execution_workflow(
    snapshot: &RuntimeSnapshot,
    workflow: &WorkflowSnapshot,
) -> Result<Value> {
    let nodes: BTreeMap<String, WorkflowNode> = serde_json::from_value(workflow.nodes.clone())
        .with_context(|| format!("parse workflow nodes for {}", workflow.id))?;
    let connections: Vec<WorkflowConnection> = serde_json::from_value(workflow.connections.clone())
        .with_context(|| format!("parse workflow connections for {}", workflow.id))?;
    let node_order = runtime_execution_node_order(&nodes, &connections);
    let project_slug = project_slug_for_workflow(snapshot, workflow)
        .or(snapshot.project_filter.as_deref())
        .or_else(|| {
            snapshot
                .projects
                .first()
                .map(|project| project.slug.as_str())
        })
        .unwrap_or("demo");
    let topology_complete = node_order.len() == nodes.len();
    let steps = node_order
        .iter()
        .enumerate()
        .filter_map(|(index, node_id)| {
            nodes.get(node_id).map(|node| {
                runtime_execution_step(
                    snapshot,
                    workflow,
                    node,
                    &nodes,
                    &connections,
                    index + 1,
                    project_slug,
                )
            })
        })
        .collect::<Vec<_>>();

    Ok(json!({
        "workflow_id": workflow.id.clone(),
        "project_id": workflow.project_id.clone(),
        "project_slug": project_slug,
        "shot_id": workflow.shot_id.clone(),
        "name": workflow.name.clone(),
        "topology_complete": topology_complete,
        "summary": {
            "nodes": nodes.len(),
            "edges": connections.len(),
            "steps": steps.len(),
            "phase_counts": count_by(
                steps
                    .iter()
                    .filter_map(|step| step.get("phase"))
                    .filter_map(Value::as_str)
            ),
            "connection_kinds": count_by(connections.iter().map(|connection| connection_kind_name(&connection.kind))),
        },
        "steps": steps,
    }))
}

fn runtime_execution_node_order(
    nodes: &BTreeMap<String, WorkflowNode>,
    connections: &[WorkflowConnection],
) -> Vec<String> {
    let mut incoming = nodes
        .keys()
        .map(|node_id| (node_id.clone(), 0usize))
        .collect::<BTreeMap<_, _>>();
    let mut outgoing = nodes
        .keys()
        .map(|node_id| (node_id.clone(), BTreeSet::<String>::new()))
        .collect::<BTreeMap<_, _>>();

    for connection in connections {
        if !nodes.contains_key(&connection.from_node_id)
            || !nodes.contains_key(&connection.to_node_id)
        {
            continue;
        }
        outgoing
            .entry(connection.from_node_id.clone())
            .or_default()
            .insert(connection.to_node_id.clone());
        *incoming.entry(connection.to_node_id.clone()).or_default() += 1;
    }

    let mut ready = incoming
        .iter()
        .filter(|(_, count)| **count == 0)
        .map(|(node_id, _)| node_id.clone())
        .collect::<BTreeSet<_>>();
    let mut order = Vec::new();

    while let Some(node_id) = ready.iter().next().cloned() {
        ready.remove(&node_id);
        order.push(node_id.clone());
        for child_id in outgoing.get(&node_id).into_iter().flatten() {
            let Some(count) = incoming.get_mut(child_id) else {
                continue;
            };
            *count = count.saturating_sub(1);
            if *count == 0 {
                ready.insert(child_id.clone());
            }
        }
    }

    for node_id in nodes.keys() {
        if !order.contains(node_id) {
            order.push(node_id.clone());
        }
    }

    order
}

fn runtime_execution_step(
    snapshot: &RuntimeSnapshot,
    workflow: &WorkflowSnapshot,
    node: &WorkflowNode,
    nodes: &BTreeMap<String, WorkflowNode>,
    connections: &[WorkflowConnection],
    sequence: usize,
    project_slug: &str,
) -> Value {
    let graph_node = runtime_graph_node(snapshot, node);
    let status = graph_node
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or_else(|| node_status_name(&node.status));
    let task_type = runtime_task_type(node);
    let blocked_by_approval = graph_node
        .get("blocked_by_approval")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let phase = runtime_execution_phase(status, blocked_by_approval);
    let tasks = snapshot
        .tasks
        .iter()
        .filter(|task| task.node_id.as_deref() == Some(node.id.as_str()))
        .collect::<Vec<_>>();
    let latest_task = tasks.first();
    let incoming_edges = connections
        .iter()
        .filter(|connection| connection.to_node_id == node.id)
        .map(|connection| runtime_graph_edge(snapshot, nodes, connection))
        .collect::<Vec<_>>();
    let outgoing_edges = connections
        .iter()
        .filter(|connection| connection.from_node_id == node.id)
        .map(|connection| runtime_graph_edge(snapshot, nodes, connection))
        .collect::<Vec<_>>();
    let provider_id = node
        .provider_id
        .as_deref()
        .filter(|provider_id| !provider_id.trim().is_empty());
    let software_adapter_id = node
        .software_adapter_id
        .as_deref()
        .filter(|adapter_id| !adapter_id.trim().is_empty());
    let software_action_kind =
        software_adapter_id.map(|adapter_id| node_context_software_action_kind(node, adapter_id));
    let control_context = runtime_node_control_context(node, project_slug);

    json!({
        "id": format!("{}::{}", workflow.id, node.id),
        "sequence": sequence,
        "workflow_id": workflow.id.clone(),
        "node_id": node.id.clone(),
        "title": node.title.clone(),
        "node_type": node_type_name(&node.node_type),
        "task_type": task_type,
        "status": status,
        "phase": phase,
        "gate": runtime_execution_gate(node, status, latest_task),
        "provider_id": provider_id,
        "software_adapter_id": software_adapter_id,
        "software_action_kind": software_action_kind,
        "contracts": runtime_execution_contracts(workflow, node, provider_id, software_adapter_id),
        "incoming_edges": incoming_edges,
        "outgoing_edges": outgoing_edges,
        "control": {
            "context": control_context,
            "recommended_action": runtime_execution_recommended_action(
                node,
                task_type,
                status,
                phase,
                latest_task,
                project_slug,
                software_action_kind,
            ),
        },
        "evidence": {
            "latest_task": latest_task,
            "task_count": tasks.len(),
            "asset_count": graph_node.get("asset_count").cloned().unwrap_or_else(|| json!(0)),
            "provider_request_count": graph_node
                .get("provider_request_count")
                .cloned()
                .unwrap_or_else(|| json!(0)),
            "software_action_count": graph_node
                .get("software_action_count")
                .cloned()
                .unwrap_or_else(|| json!(0)),
        },
    })
}

fn runtime_execution_phase(status: &str, blocked_by_approval: bool) -> &'static str {
    if blocked_by_approval || status == "WaitingApproval" {
        "waiting_approval"
    } else {
        match status {
            "Running" => "running",
            "Succeeded" => "complete",
            "Failed" | "Retryable" | "Cancelled" => "needs_recovery",
            _ => "ready",
        }
    }
}

fn runtime_execution_gate(
    node: &WorkflowNode,
    status: &str,
    latest_task: Option<&&TaskSnapshot>,
) -> Value {
    if node.requires_approval || status == "WaitingApproval" {
        return json!({
            "kind": "approval",
            "status": "waiting_approval",
            "task_id": latest_task.map(|task| task.id.clone()),
            "reason": "high-cost or confirmation-gated step requires explicit approval",
        });
    }
    if matches!(status, "Failed" | "Retryable" | "Cancelled") {
        return json!({
            "kind": "recovery",
            "status": status,
            "task_id": latest_task.map(|task| task.id.clone()),
            "reason": "step needs retry or operator inspection before continuing",
        });
    }
    json!({
        "kind": "none",
        "status": "clear",
    })
}

fn runtime_execution_contracts(
    workflow: &WorkflowSnapshot,
    node: &WorkflowNode,
    provider_id: Option<&str>,
    software_adapter_id: Option<&str>,
) -> Vec<Value> {
    let mut contracts = vec![
        json!({
            "kind": "workflow_context",
            "mcp_uri": format!("pool://workflow/{}", workflow.id),
            "http_path": format!("/api/workflow-context?workflow_id={}", workflow.id),
        }),
        json!({
            "kind": "node_context",
            "mcp_uri": format!("pool://node-context/{}", node.id),
            "http_path": format!("/api/node-context?node_id={}", node.id),
        }),
    ];
    if let Some(provider_id) = provider_id {
        contracts.push(json!({
            "kind": "provider_contract",
            "provider_id": provider_id,
            "mcp_uri": format!("pool://provider-contracts/{provider_id}"),
            "http_path": format!("/api/provider-contracts?provider_id={provider_id}"),
        }));
    }
    if let Some(adapter_id) = software_adapter_id {
        contracts.push(json!({
            "kind": "software_contract",
            "adapter_id": adapter_id,
            "mcp_uri": format!("pool://software-contracts/{adapter_id}"),
            "http_path": format!("/api/software-contracts?adapter_id={adapter_id}"),
        }));
        contracts.push(json!({
            "kind": "desktop_recognition_contract",
            "mcp_uri": "pool://desktop-recognition-contract",
            "http_path": "/api/desktop-recognition/contract",
        }));
    }
    contracts
}

fn runtime_execution_recommended_action(
    node: &WorkflowNode,
    task_type: &str,
    status: &str,
    phase: &str,
    latest_task: Option<&&TaskSnapshot>,
    project_slug: &str,
    software_action_kind: Option<&str>,
) -> Value {
    if phase == "waiting_approval" {
        if let Some(task) = latest_task {
            return json!({
                "kind": "approve_task",
                "title": format!("Approve task: {}", task.title),
                "command": format!("pool-cli --project {project_slug} approve-task {}", task.id),
                "mcp_tool": "pool_approve_task",
                "arguments": {
                    "project_slug": project_slug,
                    "task_id": task.id,
                },
            });
        }
        return json!({
            "kind": "inspect_approval",
            "title": "Inspect approval gate",
            "command": format!("pool-cli --project {project_slug} node-context {}", node.id),
            "mcp_tool": "pool_node_context",
        });
    }
    if phase == "needs_recovery" {
        if let Some(task) = latest_task {
            return json!({
                "kind": "retry_task",
                "title": format!("Retry task: {}", task.title),
                "command": format!("pool-cli --project {project_slug} retry-task {}", task.id),
                "mcp_tool": "pool_retry_task",
                "arguments": {
                    "project_slug": project_slug,
                    "task_id": task.id,
                },
            });
        }
    }
    if phase == "running" {
        return json!({
            "kind": "inspect_running_step",
            "title": "Inspect running step",
            "command": format!("pool-cli --project {project_slug} node-context {}", node.id),
            "mcp_tool": "pool_node_context",
        });
    }
    if phase == "complete" {
        return json!({
            "kind": "completed",
            "title": "Step already completed",
            "command": format!("pool-cli --project {project_slug} node-context {}", node.id),
        });
    }

    match task_type {
        "ai_provider" | "3dgs" => json!({
            "kind": "run_provider_node",
            "title": format!("Run Provider node: {}", node.title),
            "command": format!("pool-cli --project {project_slug} run-node {}", node.id),
            "mcp_tool": "pool_run_node",
            "arguments": {
                "project_slug": project_slug,
                "node_id": node.id,
            },
        }),
        "software_control" => json!({
            "kind": "run_software_node",
            "title": format!("Run software node: {}", node.title),
            "command": format!("pool-cli --project {project_slug} run-node {}", node.id),
            "mcp_tool": "pool_run_node",
            "arguments": {
                "project_slug": project_slug,
                "node_id": node.id,
                "action_kind": software_action_kind,
            },
        }),
        "agent" => json!({
            "kind": "run_agent_node",
            "title": format!("Run Agent/Hermes node: {}", node.title),
            "command": format!("pool-cli --project {project_slug} run-node {}", node.id),
            "mcp_tool": "pool_run_node",
            "arguments": {
                "project_slug": project_slug,
                "node_id": node.id,
            },
        }),
        "output" => json!({
            "kind": "run_output_node",
            "title": format!("Run output node: {}", node.title),
            "command": format!("pool-cli --project {project_slug} run-node {}", node.id),
            "mcp_tool": "pool_run_node",
            "arguments": {
                "project_slug": project_slug,
                "node_id": node.id,
            },
        }),
        _ => json!({
            "kind": "run_node",
            "title": format!("Run node: {}", node.title),
            "command": format!("pool-cli --project {project_slug} run-node {}", node.id),
            "mcp_tool": "pool_run_node",
            "arguments": {
                "project_slug": project_slug,
                "node_id": node.id,
            },
            "status": status,
        }),
    }
}

fn runtime_graph_node(snapshot: &RuntimeSnapshot, node: &WorkflowNode) -> Value {
    let tasks = snapshot
        .tasks
        .iter()
        .filter(|task| task.node_id.as_deref() == Some(node.id.as_str()))
        .collect::<Vec<_>>();
    let task_ids = tasks
        .iter()
        .map(|task| task.id.as_str())
        .collect::<Vec<_>>();
    let latest_task = tasks.first();
    let runtime_status = latest_task
        .map(|task| task.status.as_str())
        .or_else(|| {
            snapshot
                .node_states
                .iter()
                .find(|state| state.node_id == node.id)
                .map(|state| state.status.as_str())
        })
        .unwrap_or_else(|| node_status_name(&node.status));
    let assets = snapshot
        .assets
        .iter()
        .filter(|asset| asset.source_node_id.as_deref() == Some(node.id.as_str()))
        .collect::<Vec<_>>();
    let provider_requests = snapshot
        .provider_requests
        .iter()
        .filter(|request| task_ids.contains(&request.task_id.as_str()))
        .collect::<Vec<_>>();
    let software_actions = snapshot
        .software_actions
        .iter()
        .filter(|action| {
            action
                .task_id
                .as_deref()
                .is_some_and(|task_id| task_ids.contains(&task_id))
        })
        .collect::<Vec<_>>();

    json!({
        "id": node.id.clone(),
        "title": node.title.clone(),
        "node_type": node_type_name(&node.node_type),
        "task_type": runtime_task_type(node),
        "status": runtime_status,
        "static_status": node_status_name(&node.status),
        "provider_id": node.provider_id.clone(),
        "software_adapter_id": node.software_adapter_id.clone(),
        "requires_approval": node.requires_approval,
        "cost_estimate_tokens": node.cost_estimate_tokens,
        "position": node.position.clone(),
        "parameters": node.parameters.clone(),
        "latest_task": latest_task,
        "tasks": tasks,
        "asset_count": assets.len(),
        "provider_request_count": provider_requests.len(),
        "software_action_count": software_actions.len(),
        "can_run": !matches!(runtime_status, "Running" | "WaitingApproval"),
        "blocked_by_approval": node.requires_approval || runtime_status == "WaitingApproval",
    })
}

fn runtime_graph_edge(
    snapshot: &RuntimeSnapshot,
    nodes: &BTreeMap<String, WorkflowNode>,
    connection: &WorkflowConnection,
) -> Value {
    let from_node = nodes.get(&connection.from_node_id);
    let to_node = nodes.get(&connection.to_node_id);

    json!({
        "id": connection.id.clone(),
        "from_node_id": connection.from_node_id.clone(),
        "to_node_id": connection.to_node_id.clone(),
        "from_title": from_node.map(|node| node.title.as_str()),
        "to_title": to_node.map(|node| node.title.as_str()),
        "from_status": from_node.map(|node| runtime_node_status(snapshot, node)),
        "to_status": to_node.map(|node| runtime_node_status(snapshot, node)),
        "kind": connection_kind_name(&connection.kind),
        "channel": connection_channel(&connection.kind),
        "label": connection.label.clone(),
    })
}

fn runtime_node_status(snapshot: &RuntimeSnapshot, node: &WorkflowNode) -> String {
    snapshot
        .tasks
        .iter()
        .find(|task| task.node_id.as_deref() == Some(node.id.as_str()))
        .map(|task| task.status.clone())
        .or_else(|| {
            snapshot
                .node_states
                .iter()
                .find(|state| state.node_id == node.id)
                .map(|state| state.status.clone())
        })
        .unwrap_or_else(|| node_status_name(&node.status).to_string())
}

fn runtime_task_type(node: &WorkflowNode) -> &'static str {
    match node.node_type {
        NodeType::Agent | NodeType::AgentCli | NodeType::Hermes => "agent",
        NodeType::AiImage
        | NodeType::AiVideo
        | NodeType::Audio
        | NodeType::ComfyUi
        | NodeType::Suno => "ai_provider",
        NodeType::ThreeDgs => "3dgs",
        NodeType::AssetPackage => "asset_package",
        NodeType::SoftwareControl
        | NodeType::Unreal
        | NodeType::Blender
        | NodeType::Resolve
        | NodeType::Unity
        | NodeType::TouchDesigner
        | NodeType::MadMapper
        | NodeType::Nuke
        | NodeType::MotionCaptureDb => "software_control",
        NodeType::ApprovalGate => "approval",
        NodeType::VideoOutput | NodeType::GameOutput | NodeType::InteractiveOutput => "output",
        NodeType::Input | NodeType::Prompt | NodeType::Storyboard => "creative_input",
    }
}

fn connection_channel(kind: &ConnectionKind) -> &'static str {
    match kind {
        ConnectionKind::AssetFlow => "asset",
        ConnectionKind::ControlFlow => "control",
        ConnectionKind::AgentInstruction => "agent_instruction",
        ConnectionKind::FeedbackLoop => "feedback",
        ConnectionKind::Approval => "approval",
    }
}

fn node_type_name(value: &NodeType) -> &'static str {
    match value {
        NodeType::Input => "Input",
        NodeType::Prompt => "Prompt",
        NodeType::Storyboard => "Storyboard",
        NodeType::Agent => "Agent",
        NodeType::AgentCli => "AgentCli",
        NodeType::Hermes => "Hermes",
        NodeType::AiImage => "AiImage",
        NodeType::AiVideo => "AiVideo",
        NodeType::Audio => "Audio",
        NodeType::ComfyUi => "ComfyUi",
        NodeType::ThreeDgs => "ThreeDgs",
        NodeType::AssetPackage => "AssetPackage",
        NodeType::SoftwareControl => "SoftwareControl",
        NodeType::Unreal => "Unreal",
        NodeType::Blender => "Blender",
        NodeType::Resolve => "Resolve",
        NodeType::Unity => "Unity",
        NodeType::TouchDesigner => "TouchDesigner",
        NodeType::MadMapper => "MadMapper",
        NodeType::Nuke => "Nuke",
        NodeType::MotionCaptureDb => "MotionCaptureDb",
        NodeType::Suno => "Suno",
        NodeType::ApprovalGate => "ApprovalGate",
        NodeType::VideoOutput => "VideoOutput",
        NodeType::GameOutput => "GameOutput",
        NodeType::InteractiveOutput => "InteractiveOutput",
    }
}

fn node_status_name(value: &NodeStatus) -> &'static str {
    match value {
        NodeStatus::Idle => "Idle",
        NodeStatus::Ready => "Ready",
        NodeStatus::Running => "Running",
        NodeStatus::WaitingApproval => "WaitingApproval",
        NodeStatus::Succeeded => "Succeeded",
        NodeStatus::Failed => "Failed",
        NodeStatus::Skipped => "Skipped",
    }
}

fn connection_kind_name(value: &ConnectionKind) -> &'static str {
    match value {
        ConnectionKind::AssetFlow => "AssetFlow",
        ConnectionKind::ControlFlow => "ControlFlow",
        ConnectionKind::AgentInstruction => "AgentInstruction",
        ConnectionKind::FeedbackLoop => "FeedbackLoop",
        ConnectionKind::Approval => "Approval",
    }
}

fn software_action_summary(snapshot: &RuntimeSnapshot) -> Value {
    json!({
        "total": snapshot.software_actions.len(),
        "desktop_recognition": snapshot
            .software_actions
            .iter()
            .filter(|action| is_desktop_recognition_action(action))
            .count(),
        "by_adapter": count_by(
            snapshot
                .software_actions
                .iter()
                .map(|action| action.adapter_id.as_str())
        ),
        "by_action_kind": count_by(
            snapshot
                .software_actions
                .iter()
                .map(|action| action.action_kind.as_str())
        ),
    })
}

fn agent_session_summary(snapshot: &RuntimeSnapshot) -> Value {
    let token_budget = snapshot
        .agent_sessions
        .iter()
        .filter_map(|session| session.token_budget)
        .sum::<u64>();

    json!({
        "total": snapshot.agent_sessions.len(),
        "token_used": snapshot
            .agent_sessions
            .iter()
            .map(|session| session.token_used)
            .sum::<u64>(),
        "token_budget": token_budget,
        "budget_remaining": if token_budget == 0 {
            None
        } else {
            Some(token_budget as i64 - snapshot
                .agent_sessions
                .iter()
                .map(|session| session.token_used)
                .sum::<u64>() as i64)
        },
        "by_project": count_by(
            snapshot
                .agent_sessions
                .iter()
                .map(|session| session.project_slug.as_str())
        ),
    })
}

fn desktop_recognition_resource(snapshot: &RuntimeSnapshot) -> Value {
    let desktop_actions = snapshot
        .software_actions
        .iter()
        .filter(|action| is_desktop_recognition_action(action))
        .collect::<Vec<_>>();
    let requests = desktop_actions
        .iter()
        .filter_map(|action| desktop_recognition_request_value(action))
        .collect::<Vec<_>>();
    let actions = desktop_actions
        .iter()
        .map(|action| desktop_recognition_action_value(action))
        .collect::<Vec<_>>();

    json!({
        "project_filter": snapshot.project_filter,
        "summary": {
            "total": desktop_actions.len(),
            "queued_for_desktop_recognition": desktop_actions
                .iter()
                .filter(|action| desktop_recognition_status_name(action) == "queued_for_desktop_recognition")
                .count(),
            "running": desktop_actions
                .iter()
                .filter(|action| desktop_recognition_status_name(action) == "running")
                .count(),
            "retryable": desktop_actions
                .iter()
                .filter(|action| desktop_recognition_status_name(action) == "retryable")
                .count(),
            "succeeded": desktop_actions
                .iter()
                .filter(|action| desktop_recognition_status_name(action) == "succeeded")
                .count(),
            "failed": desktop_actions
                .iter()
                .filter(|action| desktop_recognition_status_name(action) == "failed")
                .count(),
            "cancelled": desktop_actions
                .iter()
                .filter(|action| desktop_recognition_status_name(action) == "cancelled")
                .count(),
            "open_requests": requests.len(),
        },
        "requests": requests,
        "actions": actions,
        "contract": desktop_recognition_contract_resource(),
    })
}

fn desktop_recognition_request_value(action: &SoftwareActionSnapshot) -> Option<Value> {
    if !is_open_desktop_recognition_request(action) {
        return None;
    }

    let request_path = desktop_request_path(action);
    let request_file =
        request_path
            .as_deref()
            .and_then(|path| match std::fs::read_to_string(path) {
                Ok(raw) => serde_json::from_str::<Value>(&raw).ok(),
                Err(_) => None,
            });
    let pool_desktop_action = request_file
        .as_ref()
        .and_then(|value| value.get("pool_desktop_action"))
        .cloned();
    let desktop_payload = request_file
        .as_ref()
        .and_then(|value| value.get("desktop_payload"))
        .cloned();

    Some(json!({
        "software_action_id": action.id.clone(),
        "task_id": action.task_id.clone(),
        "adapter_id": action.adapter_id.clone(),
        "action_kind": action.action_kind.clone(),
        "status": desktop_recognition_status_name(action),
        "desktop_request_path": request_path,
        "request_file_available": request_file.is_some(),
        "pool_desktop_action": pool_desktop_action,
        "desktop_payload": desktop_payload,
        "command": action.command.clone(),
        "verification": action.verification.clone(),
        "created_at": action.created_at.clone(),
    }))
}

fn desktop_recognition_action_value(action: &SoftwareActionSnapshot) -> Value {
    json!({
        "software_action_id": action.id.clone(),
        "task_id": action.task_id.clone(),
        "adapter_id": action.adapter_id.clone(),
        "action_kind": action.action_kind.clone(),
        "status": desktop_recognition_status_name(action),
        "desktop_request_path": desktop_request_path(action),
        "command": action.command.clone(),
        "verification": action.verification.clone(),
        "created_at": action.created_at.clone(),
    })
}

fn is_open_desktop_recognition_request(action: &SoftwareActionSnapshot) -> bool {
    if !is_desktop_recognition_action(action) {
        return false;
    }

    matches!(
        desktop_recognition_status_name(action),
        "queued_for_desktop_recognition" | "retryable"
    )
}

fn is_desktop_recognition_action(action: &SoftwareActionSnapshot) -> bool {
    let priority = action
        .command
        .get("priority")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let command_action_kind = action
        .command
        .get("action_kind")
        .and_then(Value::as_str)
        .unwrap_or(action.action_kind.as_str());

    priority == "DesktopRecognition"
        || matches!(command_action_kind, "DesktopClick" | "DesktopHotkey")
        || desktop_request_path(action).is_some()
        || desktop_recognition_artifacts(action)
            .iter()
            .any(|artifact| artifact.starts_with("desktop-recognition://"))
}

fn desktop_recognition_status_name(action: &SoftwareActionSnapshot) -> &'static str {
    desktop_recognition_status(action)
        .and_then(normalize_desktop_recognition_status)
        .unwrap_or("queued_for_desktop_recognition")
}

fn desktop_recognition_status(action: &SoftwareActionSnapshot) -> Option<&str> {
    action
        .verification
        .as_ref()
        .and_then(|value| {
            value
                .get("desktop_recognition_status")
                .or_else(|| value.get("status"))
        })
        .and_then(Value::as_str)
}

fn desktop_request_path(action: &SoftwareActionSnapshot) -> Option<String> {
    desktop_recognition_artifacts(action)
        .into_iter()
        .find(|artifact| artifact.ends_with(".json") && artifact.contains("desktop-recognition"))
}

fn desktop_recognition_artifacts(action: &SoftwareActionSnapshot) -> Vec<String> {
    action
        .verification
        .as_ref()
        .and_then(|value| value.get("artifacts"))
        .and_then(Value::as_array)
        .map(|artifacts| {
            artifacts
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn normalize_desktop_recognition_status(status: &str) -> Option<&'static str> {
    match status.trim().to_ascii_lowercase().as_str() {
        "queued" | "queued_for_desktop_recognition" | "pending" => {
            Some("queued_for_desktop_recognition")
        }
        "running" | "in_progress" | "claimed" | "working" => Some("running"),
        "succeeded" | "success" | "ok" | "completed" | "complete" => Some("succeeded"),
        "failed" | "failure" | "error" => Some("failed"),
        "retryable" | "retry" => Some("retryable"),
        "cancelled" | "canceled" => Some("cancelled"),
        _ => None,
    }
}

fn count_by<'a>(values: impl Iterator<Item = &'a str>) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for value in values {
        *counts.entry(value.to_string()).or_insert(0) += 1;
    }
    counts
}

fn empty_conformance_catalog_json(package_kind: ConformancePackageKind) -> Value {
    let (kind, expected_files) = match package_kind {
        ConformancePackageKind::Provider => (
            "pool_provider_conformance_packages",
            vec![
                ".1-provider-conformance-package-request.json",
                "1-provider-contract.json",
                "2-provider-gateway-worker-contract.json",
                "3-provider-conformance-runbook.json",
                "4-provider-conformance-preflight.json",
                "5-provider-conformance-runner.sh",
                "6-provider-conformance-package-manifest.json",
            ],
        ),
        ConformancePackageKind::Software => (
            "pool_software_conformance_packages",
            vec![
                ".1-software-conformance-package-request.json",
                "1-software-control-contract.json",
                "2-software-conformance-runbook.json",
                "3-software-conformance-preflight.json",
                "4-software-conformance-runner.sh",
                "5-software-conformance-package-manifest.json",
            ],
        ),
        ConformancePackageKind::Agent => (
            "pool_agent_conformance_packages",
            vec![
                ".1-agent-conformance-package-request.json",
                "1-agent-session-contract.json",
                "2-agent-conformance-runbook.json",
                "3-agent-conformance-preflight.json",
                "4-agent-conformance-runner.sh",
                "5-agent-conformance-package-manifest.json",
            ],
        ),
        ConformancePackageKind::Integration => (
            "pool_integration_conformance_packages",
            vec![
                ".1-integration-conformance-package-request.json",
                "1-integration-conformance-runbook.json",
                "2-integration-conformance-runner.sh",
                "3-integration-conformance-package-manifest.json",
            ],
        ),
    };

    json!({
        "kind": kind,
        "package_kind": package_kind,
        "summary": {
            "package_count": 0,
            "indexed_files": 0,
            "ready_packages": 0,
            "runner_packages": 0,
            "local_file_failures": [],
            "latest_asset_at": null
        },
        "packages": [],
        "policy": {
            "local_files_authoritative": true,
            "provider_urls_are_provenance": true,
            "secrets_stay_server_side": true,
            "expected_files": expected_files
        }
    })
}

impl Default for McpServer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::control::{
        ControlPriority, SoftwareActionKind, SoftwareActionResult, SoftwareControlAction,
    };
    use crate::db::RuntimeRepository;
    use crate::engine::{
        build_default_content_burst_plan, RuntimeHandoffPackageRequest, RuntimeHandoffPackageRunner,
    };
    use crate::models::{NodeType, RuntimeTask};
    use std::path::PathBuf;
    use uuid::Uuid;

    #[test]
    fn default_server_lists_core_resources() {
        let server = McpServer::new();
        let uris = server
            .list_resources()
            .iter()
            .map(|resource| resource.uri.as_str())
            .collect::<Vec<_>>();

        assert!(uris.contains(&"pool://status"));
        assert!(uris.contains(&"pool://runtime-graph"));
        assert!(uris.contains(&"pool://runtime-execution-plan"));
        assert!(uris.contains(&"pool://adapters"));
        assert!(uris.contains(&"pool://integration-readiness"));
        assert!(uris.contains(&"pool://core-architecture-readiness"));
        assert!(uris.contains(&"pool://provider-contracts"));
        assert!(uris.contains(&"pool://provider-gateway-worker"));
        assert!(uris.contains(&"pool://software-contracts"));
        assert!(uris.contains(&"pool://unreal-mcp-bridge"));
        assert!(uris.contains(&"pool://desktop-recognition-contract"));
        assert!(uris.contains(&"pool://desktop-recognition"));
        assert!(uris.contains(&"pool://output-packages"));
        assert!(uris.contains(&"pool://runtime-handoff-packages"));
        assert!(uris.contains(&"pool://prd-readiness"));
        assert!(uris.contains(&"pool://prd-completion-gate"));
        assert!(uris.contains(&"pool://production-evidence-requirements"));
        assert!(uris.contains(&"pool://production-evidence-tasks"));
        assert!(uris.contains(&"pool://production-evidence-run-plan"));
        assert!(uris.contains(&"pool://production-evidence-handoff"));
        assert!(uris.contains(&"pool://production-evidence-item-template"));
        assert!(uris.contains(&"pool://snapshot"));
        assert!(server
            .read_resource("pool://status")
            .unwrap()
            .contains("ready"));
        let output_packages_payload = server.read_resource("pool://output-packages").unwrap();
        let output_packages: serde_json::Value =
            serde_json::from_str(&output_packages_payload).unwrap();
        assert_eq!(output_packages["kind"], "pool_output_packages");
        assert_eq!(output_packages["summary"]["ready_targets"], 0);
        let handoff_packages_payload = server
            .read_resource("pool://runtime-handoff-packages")
            .unwrap();
        let handoff_packages: serde_json::Value =
            serde_json::from_str(&handoff_packages_payload).unwrap();
        assert_eq!(handoff_packages["kind"], "pool_runtime_handoff_packages");
        assert_eq!(handoff_packages["summary"]["package_count"], 0);
        let prd_readiness_payload = server.read_resource("pool://prd-readiness").unwrap();
        let prd_readiness: serde_json::Value =
            serde_json::from_str(&prd_readiness_payload).unwrap();
        assert_eq!(prd_readiness["kind"], "pool_prd_readiness");
        assert_eq!(prd_readiness["overall_status"], "partial");
        assert_eq!(prd_readiness["requirements"][0]["id"], "runtime_snapshot");
        let core_readiness_payload = server
            .read_resource("pool://core-architecture-readiness")
            .unwrap();
        let core_readiness: serde_json::Value =
            serde_json::from_str(&core_readiness_payload).unwrap();
        assert_eq!(core_readiness["kind"], "pool_core_architecture_readiness");
        assert_eq!(core_readiness["overall_status"], "requires_snapshot");
        assert_eq!(
            core_readiness["architecture_gate"]["ready_for_core_architecture"],
            false
        );
        let production_requirements_payload = server
            .read_resource("pool://production-evidence-requirements")
            .unwrap();
        let production_requirements: serde_json::Value =
            serde_json::from_str(&production_requirements_payload).unwrap();
        assert_eq!(
            production_requirements["kind"],
            "pool_production_evidence_requirements"
        );
        assert_eq!(
            production_requirements["overall_status"],
            "requires_snapshot"
        );
        let production_tasks_payload = server
            .read_resource("pool://production-evidence-tasks")
            .unwrap();
        let production_tasks: serde_json::Value =
            serde_json::from_str(&production_tasks_payload).unwrap();
        assert_eq!(production_tasks["kind"], "pool_production_evidence_tasks");
        assert_eq!(production_tasks["overall_status"], "requires_snapshot");
        let production_run_plan_payload = server
            .read_resource("pool://production-evidence-run-plan")
            .unwrap();
        let production_run_plan: serde_json::Value =
            serde_json::from_str(&production_run_plan_payload).unwrap();
        assert_eq!(
            production_run_plan["kind"],
            "pool_production_evidence_run_plan"
        );
        assert_eq!(production_run_plan["status"], "requires_snapshot");
        let production_handoff_payload = server
            .read_resource("pool://production-evidence-handoff")
            .unwrap();
        let production_handoff: serde_json::Value =
            serde_json::from_str(&production_handoff_payload).unwrap();
        assert_eq!(
            production_handoff["kind"],
            "pool_production_evidence_handoff"
        );
        assert_eq!(production_handoff["overall_status"], "requires_snapshot");
        let production_item_template_payload = server
            .read_resource(
                "pool://production-evidence-item-template/provider:midjourney:production_upstream",
            )
            .unwrap();
        let production_item_template: serde_json::Value =
            serde_json::from_str(&production_item_template_payload).unwrap();
        assert_eq!(
            production_item_template["kind"],
            "pool_production_evidence_item_template"
        );
        assert_eq!(
            production_item_template["overall_status"],
            "requires_snapshot"
        );
        let software_contract_payload = server
            .read_resource("pool://software-contracts/unreal")
            .unwrap();
        let software_contract: serde_json::Value =
            serde_json::from_str(&software_contract_payload).unwrap();
        assert_eq!(software_contract["adapter_id"], "unreal");
        assert_eq!(
            software_contract["runtime_action"]["path"],
            "/api/software-actions"
        );
        let unreal_bridge_payload = server.read_resource("pool://unreal-mcp-bridge").unwrap();
        let unreal_bridge: serde_json::Value =
            serde_json::from_str(&unreal_bridge_payload).unwrap();
        assert_eq!(unreal_bridge["kind"], "pool_unreal_mcp_bridge_contract");
        assert_eq!(
            unreal_bridge["pool_runtime_routes"]["contract_mcp"],
            "pool://unreal-mcp-bridge"
        );
        let gateway_worker_payload = server
            .read_resource("pool://provider-gateway-worker")
            .unwrap();
        let gateway_worker: serde_json::Value =
            serde_json::from_str(&gateway_worker_payload).unwrap();
        assert_eq!(
            gateway_worker["kind"],
            "pool_provider_gateway_worker_contract"
        );
        assert_eq!(
            gateway_worker["cli"]["env"]["POOL_3DGS_GATEWAY_ENDPOINT"],
            "Set this to the worker base URL for ThreeDgsGatewayProvider."
        );
        let integration_readiness_payload = server
            .read_resource("pool://integration-readiness")
            .unwrap();
        let integration_readiness: serde_json::Value =
            serde_json::from_str(&integration_readiness_payload).unwrap();
        assert_eq!(integration_readiness["kind"], "pool_integration_readiness");
        assert_eq!(
            integration_readiness["agent"]["status"],
            "needs_runtime_snapshot"
        );
    }

    #[test]
    fn server_reads_adapter_catalog_resource() {
        let server = McpServer::new();
        let payload = server.read_resource("pool://adapters").unwrap();
        let value: serde_json::Value = serde_json::from_str(&payload).unwrap();

        assert!(value["providers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|provider| provider["id"] == "worldlabs-marble"));
        assert!(value["software_adapters"]
            .as_array()
            .unwrap()
            .iter()
            .any(|adapter| adapter["id"] == "unreal"));
        assert_eq!(value["control_priority_chain"][0], "ApiMcp");
        assert_eq!(value["provider_aliases"]["triposplat"], "tripo-splat");
        assert_eq!(value["policy"]["local_files_authoritative"], true);
    }

    #[test]
    fn snapshot_server_reads_integration_readiness_resource() {
        let db_path = temp_db_path("mcp-integration-readiness");
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        let plan = build_default_content_burst_plan("demo", "MCP readiness");
        repository.persist_plan(&plan).unwrap();
        let snapshot = repository.snapshot(Some("demo")).unwrap();
        let server = McpServer::from_snapshot(snapshot);

        let payload = server
            .read_resource("pool://integration-readiness")
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&payload).unwrap();

        assert_eq!(value["kind"], "pool_integration_readiness");
        assert_eq!(value["project_filter"], "demo");
        assert_eq!(value["summary"]["lanes"], 5);
        assert!(value["summary"]["actions"].as_u64().unwrap() > 0);
        assert!(value["lanes"].as_array().unwrap().iter().any(|lane| {
            lane["lane"] == "spatial_engine" && lane["title"] == "3D / 引擎组装"
        }));
        assert!(value["run_plan"]
            .as_array()
            .unwrap()
            .iter()
            .any(|action| { action["lane"] == "ai_media" || action["lane"] == "spatial_engine" }));
        assert!(value["providers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|provider| provider["provider_id"] == "worldlabs-marble"
                && provider["lane"] == "spatial_engine"
                && provider["next_action"]["command"]
                    .as_str()
                    .unwrap_or_default()
                    .contains("set-api-key")));
        assert!(value["software_adapters"]
            .as_array()
            .unwrap()
            .iter()
            .any(|adapter| adapter["adapter_id"] == "unreal"));
        assert_eq!(
            value["commands"]["integration_conformance_package"],
            "pool-cli --project <slug> integration-conformance-package --output-dir worlds/<slug>/output"
        );
    }

    #[test]
    fn snapshot_server_reads_core_architecture_readiness_resource() {
        let db_path = temp_db_path("mcp-core-architecture-readiness");
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        let plan = build_default_content_burst_plan("demo", "Core architecture readiness");
        repository.persist_plan(&plan).unwrap();
        let snapshot = repository.snapshot(Some("demo")).unwrap();
        let server = McpServer::from_snapshot(snapshot);

        let payload = server
            .read_resource("pool://core-architecture-readiness")
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&payload).unwrap();

        assert_eq!(value["kind"], "pool_core_architecture_readiness");
        assert_eq!(value["project_filter"], "demo");
        assert_eq!(value["summary"]["total"], 10);
        assert!(value["requirements"]
            .as_array()
            .unwrap()
            .iter()
            .any(
                |requirement| requirement["id"] == "provider_adapter_contracts"
                    && requirement["status"] == "ready"
            ));
        assert!(value["requirements"]
            .as_array()
            .unwrap()
            .iter()
            .any(
                |requirement| requirement["id"] == "production_evidence_boundary"
                    && requirement["status"] == "ready"
            ));
        assert_eq!(
            value["architecture_gate"]["proof_commands"]["strict_prd_completion_gate"],
            "pool-cli --project demo prd-completion-gate --require-complete"
        );
        assert_eq!(
            value["architecture_gate"]["proof_commands"]["core_architecture_smoke"],
            "cargo run -q -p pool-core --example run_prd_readiness_smoke -- target/core-architecture-readiness-smoke"
        );
        assert!(value["source_resources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|resource| resource.as_str() == Some("pool://prd-completion-gate")));
    }

    #[test]
    fn server_reads_provider_contract_resources() {
        let server = McpServer::new();
        let list_payload = server.read_resource("pool://provider-contracts").unwrap();
        let list_value: serde_json::Value = serde_json::from_str(&list_payload).unwrap();
        assert!(list_value["contracts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|contract| contract["provider_id"] == "worldlabs-marble"));

        let scoped_payload = server
            .read_resource("pool://provider-contracts/triposplat")
            .unwrap();
        let scoped_value: serde_json::Value = serde_json::from_str(&scoped_payload).unwrap();
        assert_eq!(scoped_value["provider_id"], "tripo-splat");
        assert_eq!(scoped_value["adapter_kind"], "three_dgs_http_gateway");
    }

    #[test]
    fn snapshot_server_reads_real_tasks_and_approval_gates() {
        let repository = RuntimeRepository::in_memory().unwrap();
        repository.migrate().unwrap();
        let plan = build_default_content_burst_plan("demo", "Pool demo");
        repository.persist_plan(&plan).unwrap();
        let snapshot = repository.snapshot(Some("demo")).unwrap();
        let server = McpServer::from_snapshot(snapshot);

        let tasks = server.read_resource("pool://tasks").unwrap();
        let value: serde_json::Value = serde_json::from_str(&tasks).unwrap();

        assert_eq!(
            value["tasks"].as_array().unwrap().len(),
            plan.workflow.nodes.len()
        );
        assert_eq!(value["approval_gates"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn snapshot_server_reads_runtime_execution_plan() {
        let repository = RuntimeRepository::in_memory().unwrap();
        repository.migrate().unwrap();
        let plan = build_default_content_burst_plan("demo", "Pool demo");
        repository.persist_plan(&plan).unwrap();
        let snapshot = repository.snapshot(Some("demo")).unwrap();
        let server = McpServer::from_snapshot(snapshot);

        let payload = server
            .read_resource("pool://runtime-execution-plan")
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&payload).unwrap();
        let steps = value["workflows"][0]["steps"].as_array().unwrap();
        let three_dgs_step = steps
            .iter()
            .find(|step| step["task_type"] == "3dgs")
            .expect("3DGS step");
        let unreal_step = steps
            .iter()
            .find(|step| step["software_adapter_id"] == "unreal")
            .expect("Unreal step");

        assert_eq!(value["kind"], "pool_runtime_execution_plan");
        assert_eq!(value["summary"]["steps"], plan.workflow.nodes.len());
        assert_eq!(three_dgs_step["phase"], "waiting_approval");
        assert_eq!(three_dgs_step["gate"]["kind"], "approval");
        assert!(three_dgs_step["contracts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|contract| contract["kind"] == "provider_contract"));
        assert_eq!(
            unreal_step["control"]["recommended_action"]["mcp_tool"],
            "pool_run_node"
        );
        assert!(unreal_step["contracts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|contract| contract["mcp_uri"] == "pool://software-contracts/unreal"));
    }

    #[test]
    fn snapshot_server_reads_workflow_resource() {
        let repository = RuntimeRepository::in_memory().unwrap();
        repository.migrate().unwrap();
        let plan = build_default_content_burst_plan("demo", "Pool demo");
        repository.persist_plan(&plan).unwrap();
        let three_dgs_node = plan
            .workflow
            .nodes
            .values()
            .find(|node| node.node_type == NodeType::ThreeDgs)
            .unwrap();
        let first_snapshot = repository.snapshot(Some("demo")).unwrap();
        let three_dgs_task = first_snapshot
            .tasks
            .iter()
            .find(|task| task.node_id.as_deref() == Some(three_dgs_node.id.as_str()))
            .unwrap();
        repository
            .insert_provider_request(
                &three_dgs_task.id,
                "worldlabs-marble",
                &json!({"prompt":"生成可导入 Unreal 的 3DGS 场景"}),
                Some("worlds/demo/output/.1-3dgs-request.json"),
            )
            .unwrap();
        repository
            .index_local_outputs(
                "demo",
                Some(&three_dgs_node.id),
                Some("https://provider.example/jobs/job-1"),
                &["worlds/demo/output/1-3dgs-scene.glb".to_string()],
            )
            .unwrap();
        let snapshot = repository.snapshot(Some("demo")).unwrap();
        let server = McpServer::from_snapshot(snapshot);

        let workflow = server.read_resource("pool://workflow").unwrap();
        let value: serde_json::Value = serde_json::from_str(&workflow).unwrap();

        assert_eq!(value["workflows"].as_array().unwrap().len(), 1);
        assert_eq!(
            value["node_states"].as_array().unwrap().len(),
            plan.workflow.nodes.len()
        );

        let workflow_context = server
            .read_resource(&format!("pool://workflow/{}", plan.workflow.id))
            .unwrap();
        let context: serde_json::Value = serde_json::from_str(&workflow_context).unwrap();
        assert_eq!(context["workflow_id"], plan.workflow.id);
        assert_eq!(
            context["graph"]["summary"]["nodes"],
            plan.workflow.nodes.len()
        );
        assert_eq!(
            context["graph"]["summary"]["edges"],
            plan.workflow.connections.len()
        );
        assert_eq!(context["summary"]["tasks"], plan.workflow.nodes.len());
        assert_eq!(context["summary"]["assets"], 1);
        assert_eq!(context["summary"]["provider_requests"], 1);
        assert_eq!(context["summary"]["blocked_by_approval"], true);
        assert!(context["graph"]["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|node| node["task_type"] == "3dgs"));
    }

    #[test]
    fn snapshot_server_reads_runtime_graph_resource() {
        let repository = RuntimeRepository::in_memory().unwrap();
        repository.migrate().unwrap();
        let plan = build_default_content_burst_plan("demo", "Pool demo");
        repository.persist_plan(&plan).unwrap();
        let snapshot = repository.snapshot(Some("demo")).unwrap();
        let server = McpServer::from_snapshot(snapshot);

        let graph = server.read_resource("pool://runtime-graph").unwrap();
        let value: serde_json::Value = serde_json::from_str(&graph).unwrap();

        assert_eq!(value["summary"]["workflows"], 1);
        assert_eq!(value["summary"]["nodes"], plan.workflow.nodes.len());
        assert_eq!(value["summary"]["edges"], plan.workflow.connections.len());
        assert!(value["workflows"][0]["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|node| node["task_type"] == "3dgs"
                && node["blocked_by_approval"] == true
                && node["status"] == "WaitingApproval"));
        assert!(value["workflows"][0]["edges"]
            .as_array()
            .unwrap()
            .iter()
            .any(|edge| edge["kind"] == "Approval" && edge["channel"] == "approval"));
    }

    #[test]
    fn snapshot_server_reads_runtime_budget_resource() {
        let repository = RuntimeRepository::in_memory().unwrap();
        repository.migrate().unwrap();
        let plan = build_default_content_burst_plan("demo", "Pool demo");
        repository.persist_plan(&plan).unwrap();
        repository
            .upsert_api_key(
                "worldlabs-marble",
                "provider",
                "wl-secret",
                json!({"env":"WORLD_LABS_API_KEY"}),
            )
            .unwrap();
        let snapshot = repository.snapshot(Some("demo")).unwrap();
        let server = McpServer::from_snapshot(snapshot);

        let payload = server.read_resource("pool://runtime-budget").unwrap();
        let value: serde_json::Value = serde_json::from_str(&payload).unwrap();

        assert_eq!(value["summary"]["configured_api_keys"], 1);
        assert_eq!(value["summary"]["waiting_approval_estimated_tokens"], 9_000);
        assert!(value["provider_credentials"]
            .as_array()
            .unwrap()
            .iter()
            .any(|provider| provider["provider_id"] == "worldlabs-marble"
                && provider["configured"] == true));
        assert!(value["approval_gates"].as_array().unwrap().len() >= 1);
        assert!(!payload.contains("wl-secret"));
    }

    #[test]
    fn snapshot_server_reads_runtime_preflight_resource() {
        let repository = RuntimeRepository::in_memory().unwrap();
        repository.migrate().unwrap();
        let plan = build_default_content_burst_plan("demo", "Pool demo");
        repository.persist_plan(&plan).unwrap();
        let snapshot = repository.snapshot(Some("demo")).unwrap();
        let server = McpServer::from_snapshot(snapshot);

        let payload = server.read_resource("pool://runtime-preflight").unwrap();
        let value: serde_json::Value = serde_json::from_str(&payload).unwrap();

        assert_eq!(value["ready"], false);
        assert_eq!(value["summary"]["approval_gates"], 1);
        assert_eq!(value["summary"]["blocked"], 1);
        assert!(value["checks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|check| check["id"] == "approval_gates" && check["status"] == "blocked"));
        assert!(value["next_actions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|action| action["kind"] == "approval"
                && action["command"]
                    .as_str()
                    .unwrap_or_default()
                    .contains("approve-task")));
    }

    #[test]
    fn snapshot_server_reads_runtime_handoff_resource() {
        let repository = RuntimeRepository::in_memory().unwrap();
        repository.migrate().unwrap();
        let plan = build_default_content_burst_plan("demo", "Pool demo");
        repository.persist_plan(&plan).unwrap();
        let snapshot = repository.snapshot(Some("demo")).unwrap();
        let server = McpServer::from_snapshot(snapshot);

        let payload = server.read_resource("pool://runtime-handoff").unwrap();
        let value: serde_json::Value = serde_json::from_str(&payload).unwrap();

        assert_eq!(value["ready"], false);
        assert_eq!(value["summary"]["approval_actions"], 1);
        assert_eq!(value["summary"]["team_roles"], 5);
        assert_eq!(value["team"]["mode"], "five_person_content_burst_team");
        assert!(value["team"]["roles"]
            .as_array()
            .unwrap()
            .iter()
            .any(|role| role["id"] == "creative_director" && role["status"] == "blocked"));
        assert!(value["team"]["roles"]
            .as_array()
            .unwrap()
            .iter()
            .any(|role| role["id"] == "agent_operator"
                && role["assigned_lane_ids"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|lane| lane.as_str() == Some("agent_context"))));
        assert!(value["team"]["roles"]
            .as_array()
            .unwrap()
            .iter()
            .any(|role| role["id"] == "agent_operator"
                && role["assigned_lane_ids"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|lane| lane.as_str() == Some("handoff_package"))));
        assert!(value["lanes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|lane| lane["id"] == "manual_approval"
                && lane["status"] == "blocked"
                && lane["team_role"] == "creative_director"));
        assert!(value["commands"]
            .as_array()
            .unwrap()
            .iter()
            .any(|command| command["command"]
                .as_str()
                .unwrap_or_default()
                .contains("runtime-preflight")));
        assert!(value["commands"]
            .as_array()
            .unwrap()
            .iter()
            .any(|command| command["command"]
                .as_str()
                .unwrap_or_default()
                .contains("approve-task")));
        assert!(value["commands"]
            .as_array()
            .unwrap()
            .iter()
            .any(|command| command["kind"] == "handoff_package"
                && command["command"]
                    .as_str()
                    .unwrap_or_default()
                    .contains("handoff-package")));
        let handoff_artifacts = value["lanes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|lane| lane["id"] == "handoff_package")
            .and_then(|lane| lane["actions"].as_array())
            .and_then(|actions| actions.first())
            .and_then(|action| action["artifacts"].as_array())
            .unwrap();
        assert!(handoff_artifacts.iter().any(|path| path
            .as_str()
            .unwrap_or_default()
            .ends_with("3-runtime-graph.json")));
        assert!(handoff_artifacts.iter().any(|path| path
            .as_str()
            .unwrap_or_default()
            .ends_with("7-integration-readiness.json")));
        assert!(handoff_artifacts.iter().any(|path| path
            .as_str()
            .unwrap_or_default()
            .ends_with("8-runtime-handoff-package-manifest.json")));
    }

    #[test]
    fn snapshot_server_reads_runtime_handoff_package_catalog_resource() {
        let repository = RuntimeRepository::in_memory().unwrap();
        repository.migrate().unwrap();
        let plan = build_default_content_burst_plan("demo", "Pool demo");
        repository.persist_plan(&plan).unwrap();
        let output_dir =
            std::env::temp_dir().join(format!("mcp-runtime-handoff-package-{}", Uuid::new_v4()));
        let runner = RuntimeHandoffPackageRunner::new(&repository);
        let report = runner
            .run(RuntimeHandoffPackageRequest {
                project_slug: "demo".to_string(),
                node_id: Some("agent".to_string()),
                output_dir: output_dir.to_string_lossy().to_string(),
                title: "MCP runtime handoff package".to_string(),
                include_snapshot: true,
            })
            .unwrap();
        let snapshot = repository.snapshot(Some("demo")).unwrap();
        let server = McpServer::from_snapshot(snapshot);

        let payload = server
            .read_resource("pool://runtime-handoff-packages")
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&payload).unwrap();

        assert_eq!(value["kind"], "pool_runtime_handoff_packages");
        assert_eq!(value["summary"]["package_count"], 1);
        assert_eq!(value["summary"]["ready_packages"], 1);
        assert_eq!(value["packages"][0]["manifest_path"], report.manifest_path);
        assert!(value["packages"][0]["operator_checklist"]
            .as_array()
            .unwrap()
            .iter()
            .any(|step| step["owner"] == "ai_3dgs_td"));
        assert!(value["packages"][0]["mcp_resources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|resource| resource.as_str() == Some("pool://runtime-handoff")));

        std::fs::remove_dir_all(output_dir).unwrap();
    }

    #[test]
    fn snapshot_server_reads_prd_readiness_resource() {
        let repository = RuntimeRepository::in_memory().unwrap();
        repository.migrate().unwrap();
        let plan = build_default_content_burst_plan("demo", "Pool demo");
        repository.persist_plan(&plan).unwrap();
        let snapshot = repository.snapshot(Some("demo")).unwrap();
        let server = McpServer::from_snapshot(snapshot);

        let payload = server.read_resource("pool://prd-readiness").unwrap();
        let value: serde_json::Value = serde_json::from_str(&payload).unwrap();

        assert_eq!(value["kind"], "pool_prd_readiness");
        assert_eq!(value["overall_status"], "partial");
        assert_eq!(value["summary"]["total"], 10);
        assert!(value["summary"]["ready"].as_u64().unwrap() >= 4);
        assert!(value["summary"]["partial"].as_u64().unwrap() >= 3);
        assert_eq!(value["completion_gate"]["status"], "incomplete");
        assert_eq!(value["completion_gate"]["ready_for_completion"], false);
        assert!(value["completion_gate"]["incomplete_requirements"]
            .as_array()
            .unwrap()
            .iter()
            .any(|requirement| requirement["id"] == "ai_media_and_3dgs_providers"));
        assert!(
            value["completion_gate"]["proof_commands"]["closeout_preflight"]
                .as_str()
                .unwrap()
                .contains("closeout-production-evidence")
        );

        let gate_payload = server.read_resource("pool://prd-completion-gate").unwrap();
        let gate_value: serde_json::Value = serde_json::from_str(&gate_payload).unwrap();
        assert_eq!(gate_value["kind"], "pool_prd_completion_gate");
        assert_eq!(gate_value["completion_gate"]["ready_for_completion"], false);
        assert_eq!(
            gate_value["completion_gate"]["status"],
            value["completion_gate"]["status"]
        );
        assert!(value["source_resources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|resource| resource.as_str() == Some("pool://runtime-graph")));
        assert!(value["requirements"]
            .as_array()
            .unwrap()
            .iter()
            .any(
                |requirement| requirement["id"] == "ai_media_and_3dgs_providers"
                    && requirement["status"] == "partial"
                    && requirement["evidence"]["three_dgs_providers"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|provider| provider.as_str() == Some("tripo-splat"))
            ));
        assert!(value["requirements"]
            .as_array()
            .unwrap()
            .iter()
            .any(|requirement| requirement["id"] == "web_console"
                && requirement["status"] == "ready"));

        let production_payload = server
            .read_resource("pool://production-evidence-requirements")
            .unwrap();
        let production: serde_json::Value = serde_json::from_str(&production_payload).unwrap();

        assert_eq!(production["kind"], "pool_production_evidence_requirements");
        assert_eq!(production["overall_status"], "partial");
        assert!(
            production["summary"]["missing_provider_production_upstream_success"]
                .as_array()
                .unwrap()
                .iter()
                .any(|provider| provider.as_str() == Some("midjourney"))
        );
        assert!(production["required_providers"]
            .as_array()
            .unwrap()
            .iter()
            .any(
                |provider| provider["provider_id"] == "tripo-splat" && provider["family"] == "3dgs"
            ));
        assert!(production["evidence_tasks"]["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|task| task["kind"] == "software_production"
                && task["target_id"] == "unreal"
                && task["bundle_path"] == "software_actions[]"
                && task["commands"]["merge"]
                    == "pool-cli --project demo merge-production-evidence <combined-bundle.json> <bundle-a.json> <bundle-b.json>..."
                && task["commands"]["closeout"]
                    == "pool-cli --project demo closeout-production-evidence --output <merged-bundle.json> <bundle-a.json> <bundle-b.json>..."
                && task["commands"]["validate"]
                    == "pool-cli --project demo validate-production-evidence <bundle.json>"));
        assert_eq!(
            production["commands"]["merge"],
            "pool-cli --project <slug> merge-production-evidence <combined-bundle.json> <bundle-a.json> <bundle-b.json>..."
        );
        assert_eq!(
            production["commands"]["closeout"],
            "pool-cli --project <slug> closeout-production-evidence --output <merged-bundle.json> <bundle-a.json> <bundle-b.json>..."
        );
        assert!(
            production["required_desktop_vision"]["required_bundle_fields"]
                .as_array()
                .unwrap()
                .iter()
                .any(|field| field.as_str()
                    == Some("visual_model:external or evidence_json.external_visual_model:true"))
        );

        let tasks_payload = server
            .read_resource("pool://production-evidence-tasks")
            .unwrap();
        let tasks: serde_json::Value = serde_json::from_str(&tasks_payload).unwrap();
        assert_eq!(tasks["kind"], "pool_production_evidence_tasks");
        assert_eq!(tasks["overall_status"], "partial");
        assert_eq!(tasks["summary"]["total"], 24);
        assert!(tasks["mcp"]["resources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|resource| resource.as_str() == Some("pool://production-evidence-tasks")));
        assert!(tasks["mcp"]["resources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|resource| resource.as_str() == Some("pool://production-evidence-item-template")));
        assert!(tasks["mcp"]["resources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|resource| resource.as_str() == Some("pool://production-evidence-handoff")));
        assert!(tasks["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .any(
                |task| task["id"] == "provider:midjourney:production_upstream"
                    && task["mcp"]["resource"] == "pool://production-evidence-tasks"
            ));
        assert!(tasks["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|task| task["id"] == "software:resolve:production_software"
                && task["preferred_control_profile"] == "api_mcp"
                && task["bridge_worker_hint"]
                    .as_str()
                    .unwrap_or_default()
                    .contains("software-api-bridge-worker")
                && task["bridge_worker"]["available"] == true
                && task["bridge_worker"]["endpoint_env"] == "POOL_RESOLVE_ENDPOINT"
                && task["bridge_worker"]["cli_template"]
                    .as_str()
                    .unwrap_or_default()
                    .contains("software-api-bridge-worker resolve")));

        let run_plan_payload = server
            .read_resource("pool://production-evidence-run-plan")
            .unwrap();
        let run_plan: serde_json::Value = serde_json::from_str(&run_plan_payload).unwrap();
        assert_eq!(run_plan["kind"], "pool_production_evidence_run_plan");
        assert_eq!(run_plan["project_slug"], "demo");
        assert_eq!(run_plan["status"], "needs_real_production_evidence");
        assert_eq!(run_plan["phases"].as_array().unwrap().len(), 7);
        let provider_phase_command = run_plan["phases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|phase| phase["id"] == "provider_evidence_matrix")
            .and_then(|phase| phase["command"].as_str())
            .unwrap();
        assert!(provider_phase_command
            .contains("--provider-endpoint-env sam-3d=POOL_PROVIDER_ENDPOINT_SAM_3D"));
        assert!(provider_phase_command
            .contains("--provider-api-key-env qunhe-3d=POOL_PROVIDER_API_KEY_QUNHE_3D"));
        assert!(provider_phase_command.contains(
            "--provider-attestation-env worldlabs-marble=POOL_PROVIDER_PRODUCTION_ATTESTATION_WORLDLABS_MARBLE"
        ));
        let provider_phase = run_plan["phases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|phase| phase["id"] == "provider_evidence_matrix")
            .expect("provider phase");
        let three_dgs_gateway_command = provider_phase["provider_gateway_worker_start_commands"]
            .as_array()
            .unwrap()
            .iter()
            .find(|command| command["family"] == "3dgs")
            .expect("3dgs provider gateway command");
        assert_eq!(
            three_dgs_gateway_command["endpoint_env"],
            "POOL_3DGS_GATEWAY_ENDPOINT"
        );
        assert_eq!(
            three_dgs_gateway_command["upstream_env"],
            "POOL_3DGS_GATEWAY_UPSTREAM_ENDPOINT"
        );
        assert!(three_dgs_gateway_command["cli"]
            .as_str()
            .unwrap()
            .contains("provider-gateway-worker"));
        let software_phase_command = run_plan["phases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|phase| phase["id"] == "software_evidence_matrix")
            .and_then(|phase| phase["command"].as_str())
            .unwrap();
        assert!(software_phase_command
            .contains("--software-endpoint-env blender=POOL_BLENDER_ENDPOINT"));
        assert!(software_phase_command
            .contains("--software-endpoint-env resolve=POOL_RESOLVE_ENDPOINT"));
        assert!(
            software_phase_command.contains("--software-command-env blender=POOL_BLENDER_COMMAND")
        );
        let software_phase = run_plan["phases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|phase| phase["id"] == "software_evidence_matrix")
            .expect("software phase");
        assert!(software_phase["generic_api_bridge_worker"]["applies_to"]
            .as_array()
            .unwrap()
            .iter()
            .any(|adapter| adapter.as_str() == Some("resolve")));
        assert!(software_phase["generic_api_bridge_worker"]["cli_template"]
            .as_str()
            .unwrap()
            .contains("software-api-bridge-worker"));
        let resolve_bridge_command = software_phase["bridge_worker_start_commands"]
            .as_array()
            .unwrap()
            .iter()
            .find(|command| command["adapter_id"] == "resolve")
            .expect("resolve bridge worker start command");
        assert_eq!(
            resolve_bridge_command["endpoint_env"],
            "POOL_RESOLVE_ENDPOINT"
        );
        assert_eq!(
            resolve_bridge_command["upstream_env"],
            "POOL_RESOLVE_UPSTREAM_ENDPOINT"
        );
        assert!(resolve_bridge_command["cli"]
            .as_str()
            .unwrap()
            .contains("software-api-bridge-worker resolve"));
        assert!(run_plan["mcp"]["resources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|resource| resource.as_str() == Some("pool://production-evidence-tasks")));
        assert!(run_plan["mcp"]["resources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|resource| resource.as_str() == Some("pool://production-evidence-run-plan")));

        let handoff_payload = server
            .read_resource("pool://production-evidence-handoff")
            .unwrap();
        let handoff: serde_json::Value = serde_json::from_str(&handoff_payload).unwrap();
        assert_eq!(handoff["kind"], "pool_production_evidence_handoff");
        assert_eq!(handoff["project_slug"], "demo");
        assert_eq!(handoff["summary"]["evidence_tasks"], 24);
        assert_eq!(handoff["ready_for_import"], false);
        assert!(handoff["handoff_lanes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|lane| lane["id"] == "provider_worker"
                && lane["resource"] == "pool://production-evidence-tasks"));
        let handoff_media_gateway = handoff["provider_gateway_worker_start_commands"]
            .as_array()
            .unwrap()
            .iter()
            .find(|command| command["family"] == "ai_media")
            .expect("handoff media gateway worker command");
        assert_eq!(
            handoff_media_gateway["endpoint_env"],
            "POOL_MEDIA_GATEWAY_ENDPOINT"
        );
        assert_eq!(
            handoff_media_gateway["upstream_env"],
            "POOL_MEDIA_GATEWAY_UPSTREAM_ENDPOINT"
        );
        let provider_lane = handoff["handoff_lanes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|lane| lane["id"] == "provider_worker")
            .expect("provider worker lane");
        assert!(provider_lane["provider_gateway_worker_start_commands"]
            .as_array()
            .unwrap()
            .iter()
            .any(|command| command["family"] == "3dgs"));
        let handoff_resolve_bridge = handoff["software_bridge_worker_start_commands"]
            .as_array()
            .unwrap()
            .iter()
            .find(|command| command["adapter_id"] == "resolve")
            .expect("handoff resolve bridge worker command");
        assert_eq!(
            handoff_resolve_bridge["endpoint_env"],
            "POOL_RESOLVE_ENDPOINT"
        );
        assert_eq!(
            handoff_resolve_bridge["upstream_env"],
            "POOL_RESOLVE_UPSTREAM_ENDPOINT"
        );
        assert!(handoff_resolve_bridge["cli"]
            .as_str()
            .unwrap()
            .contains("software-api-bridge-worker resolve"));
        let software_lane = handoff["handoff_lanes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|lane| lane["id"] == "software_operator")
            .expect("software operator lane");
        assert!(software_lane["bridge_worker_start_commands"]
            .as_array()
            .unwrap()
            .iter()
            .any(|command| command["adapter_id"] == "resolve"));
        assert!(handoff["mcp"]["resources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|resource| resource.as_str() == Some("pool://production-evidence-handoff")));

        let item_index_payload = server
            .read_resource("pool://production-evidence-item-template")
            .unwrap();
        let item_index: serde_json::Value = serde_json::from_str(&item_index_payload).unwrap();
        assert_eq!(
            item_index["kind"],
            "pool_production_evidence_item_template_index"
        );
        assert_eq!(item_index["summary"]["task_templates"], 24);

        let item_template_payload = server
            .read_resource(
                "pool://production-evidence-item-template/provider:midjourney:production_upstream",
            )
            .unwrap();
        let item_template: serde_json::Value =
            serde_json::from_str(&item_template_payload).unwrap();
        assert_eq!(
            item_template["kind"],
            "pool_production_evidence_item_template"
        );
        assert_eq!(
            item_template["selector"]["task_id"],
            "provider:midjourney:production_upstream"
        );
        assert_eq!(item_template["item"]["kind"], "provider");
        assert_eq!(
            item_template["item"]["provider"]["provider_id"],
            "midjourney"
        );
        assert_eq!(item_template["ready_for_import"], false);

        let software_item_template_payload = server
            .read_resource(
                "pool://production-evidence-item-template/software:unreal:production_software",
            )
            .unwrap();
        let software_item_template: serde_json::Value =
            serde_json::from_str(&software_item_template_payload).unwrap();
        assert_eq!(
            software_item_template["selector"]["kind"],
            "software_action"
        );
        assert_eq!(
            software_item_template["item"]["software_action"]["adapter_id"],
            "unreal"
        );
        assert_eq!(
            software_item_template["item"]["software_action"]["artifacts"][0],
            "worlds/demo/output/production/unreal/1-level.umap"
        );
        assert!(
            !software_item_template["item"]["software_action"]["artifacts"][0]
                .as_str()
                .unwrap()
                .contains("://")
        );

        let resolve_item_template_payload = server
            .read_resource(
                "pool://production-evidence-item-template/software:resolve:production_software",
            )
            .unwrap();
        let resolve_item_template: serde_json::Value =
            serde_json::from_str(&resolve_item_template_payload).unwrap();
        assert_eq!(
            resolve_item_template["item"]["software_action"]["priority"],
            "ApiMcp"
        );
        assert_eq!(
            resolve_item_template["item"]["software_action"]["control_profile"],
            "api_mcp"
        );
        assert_eq!(
            resolve_item_template["item"]["software_action"]["bridge_worker"]["available"],
            true
        );
        assert_eq!(
            resolve_item_template["item"]["software_action"]["bridge_worker"]["endpoint_env"],
            "POOL_RESOLVE_ENDPOINT"
        );
        assert!(
            resolve_item_template["item"]["software_action"]["bridge_worker"]["production_rule"]
                .as_str()
                .unwrap()
                .contains("--upstream")
        );
    }

    #[test]
    fn snapshot_server_reads_runtime_node_context_resource() {
        let repository = RuntimeRepository::in_memory().unwrap();
        repository.migrate().unwrap();
        let plan = build_default_content_burst_plan("demo", "Pool demo");
        repository.persist_plan(&plan).unwrap();
        let three_dgs_node = plan
            .workflow
            .nodes
            .values()
            .find(|node| node.node_type == NodeType::ThreeDgs)
            .unwrap();
        let first_snapshot = repository.snapshot(Some("demo")).unwrap();
        let three_dgs_task = first_snapshot
            .tasks
            .iter()
            .find(|task| task.node_id.as_deref() == Some(three_dgs_node.id.as_str()))
            .unwrap();
        repository
            .insert_provider_request(
                &three_dgs_task.id,
                "worldlabs-marble",
                &json!({"prompt":"生成可导入 Unreal 的 3DGS 场景"}),
                Some("worlds/demo/output/.1-3dgs-request.json"),
            )
            .unwrap();
        repository
            .index_local_outputs(
                "demo",
                Some(&three_dgs_node.id),
                Some("https://provider.example/jobs/job-1"),
                &["worlds/demo/output/1-3dgs-scene.glb".to_string()],
            )
            .unwrap();
        let snapshot = repository.snapshot(Some("demo")).unwrap();
        let server = McpServer::from_snapshot(snapshot);

        let index = server.read_resource("pool://node-context").unwrap();
        let index_value: serde_json::Value = serde_json::from_str(&index).unwrap();
        assert!(index_value["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|node| node["node_id"] == three_dgs_node.id));

        let context = server
            .read_resource(&format!("pool://node-context/{}", three_dgs_node.id))
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&context).unwrap();

        assert_eq!(value["node"]["task_type"], "3dgs");
        assert_eq!(value["summary"]["tasks"], 1);
        assert_eq!(value["summary"]["assets"], 1);
        assert_eq!(value["summary"]["provider_requests"], 1);
        assert_eq!(value["summary"]["blocked_by_approval"], true);
        assert_eq!(
            value["control_context"]["provider"]["id"],
            "worldlabs-marble"
        );
        assert_eq!(
            value["control_context"]["provider"]["config"]["output_contract"],
            "image-blaster indexed local 3DGS package"
        );
        assert!(value["control_context"]["mcp_resources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|resource| resource == "pool://adapters"));
        assert!(value["control_context"]["mcp_tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|tool| tool["name"] == "pool_run_provider"
                && tool["arguments"]["provider_id"] == "worldlabs-marble"
                && tool["arguments"]["node_id"] == three_dgs_node.id));
        assert!(value["control_context"]["cli_commands"]
            .as_array()
            .unwrap()
            .iter()
            .any(|command| command["command"]
                .as_str()
                .unwrap_or_default()
                .contains("run-provider worldlabs-marble")));
        assert!(value["incoming_edges"]
            .as_array()
            .unwrap()
            .iter()
            .any(|edge| edge["channel"] == "asset"));
        assert!(value["outgoing_edges"]
            .as_array()
            .unwrap()
            .iter()
            .any(|edge| edge["channel"] == "approval"));
    }

    #[test]
    fn snapshot_server_reads_provider_request_resource() {
        let repository = RuntimeRepository::in_memory().unwrap();
        repository.migrate().unwrap();
        let task = RuntimeTask::new("demo", "Provider ledger task");
        repository.insert_task(&task).unwrap();
        repository
            .insert_provider_request(
                &task.id,
                "worldlabs-marble",
                &json!({
                    "provider_id": "worldlabs-marble",
                    "provider_request": {
                        "project_slug": "demo",
                        "prompt": "make world",
                        "input_paths": [],
                        "output_dir": "worlds/demo/output",
                        "require_approval": true
                    }
                }),
                Some("worlds/demo/output/.1-world-request.json"),
            )
            .unwrap();
        let snapshot = repository.snapshot(Some("demo")).unwrap();
        let server = McpServer::from_snapshot(snapshot);

        let payload = server.read_resource("pool://provider-requests").unwrap();
        let value: serde_json::Value = serde_json::from_str(&payload).unwrap();

        assert_eq!(value["provider_requests"].as_array().unwrap().len(), 1);
        assert_eq!(
            value["provider_requests"][0]["provider_id"],
            "worldlabs-marble"
        );
    }

    #[test]
    fn snapshot_server_reads_desktop_recognition_resource() {
        let repository = RuntimeRepository::in_memory().unwrap();
        repository.migrate().unwrap();
        let task = RuntimeTask::new("demo", "TouchDesigner desktop control");
        let task_id = task.id.clone();
        repository.insert_task(&task).unwrap();

        let action = SoftwareControlAction {
            adapter_id: "touchdesigner".to_string(),
            action_kind: SoftwareActionKind::RunViewport,
            priority: ControlPriority::DesktopRecognition,
            payload_json: json!({"target_window":"TouchDesigner"}),
            requires_confirmation: false,
        };
        let result = SoftwareActionResult {
            adapter_id: "touchdesigner".to_string(),
            action_kind: SoftwareActionKind::RunViewport,
            priority: ControlPriority::DesktopRecognition,
            ok: true,
            message: "queued for desktop recognition".to_string(),
            artifacts: vec!["desktop-recognition://touchdesigner/1".to_string()],
        };
        repository
            .insert_software_action("action-queued", Some(&task_id), &action, Some(&result))
            .unwrap();
        repository
            .insert_software_action("action-done", Some(&task_id), &action, Some(&result))
            .unwrap();
        repository
            .update_software_action_verification(
                "action-done",
                json!({
                    "desktop_recognition_status": "succeeded",
                    "artifacts": ["desktop-recognition://touchdesigner/2"]
                }),
            )
            .unwrap();

        let snapshot = repository.snapshot(Some("demo")).unwrap();
        let server = McpServer::from_snapshot(snapshot);

        let software_payload = server.read_resource("pool://software-actions").unwrap();
        let software_value: serde_json::Value = serde_json::from_str(&software_payload).unwrap();
        assert_eq!(software_value["summary"]["total"], 2);
        assert_eq!(software_value["summary"]["desktop_recognition"], 2);

        let desktop_payload = server.read_resource("pool://desktop-recognition").unwrap();
        let desktop_value: serde_json::Value = serde_json::from_str(&desktop_payload).unwrap();
        assert_eq!(
            desktop_value["contract"]["kind"],
            "pool_desktop_recognition_contract"
        );
        assert_eq!(desktop_value["summary"]["total"], 2);
        assert_eq!(
            desktop_value["summary"]["queued_for_desktop_recognition"],
            1
        );
        assert_eq!(desktop_value["summary"]["succeeded"], 1);
        assert_eq!(desktop_value["summary"]["open_requests"], 1);
        assert_eq!(desktop_value["requests"].as_array().unwrap().len(), 1);
        assert_eq!(desktop_value["actions"].as_array().unwrap().len(), 2);

        let preflight_payload = server.read_resource("pool://runtime-preflight").unwrap();
        let preflight_value: serde_json::Value = serde_json::from_str(&preflight_payload).unwrap();
        let desktop_action = preflight_value["next_actions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|action| action["kind"] == "desktop_recognition")
            .expect("desktop recognition next action");
        assert!(desktop_action["command"]
            .as_str()
            .unwrap_or_default()
            .contains("desktop-run-next"));
        assert!(desktop_action["inspect_command"]
            .as_str()
            .unwrap_or_default()
            .contains("desktop-requests"));

        let handoff_payload = server.read_resource("pool://runtime-handoff").unwrap();
        let handoff_value: serde_json::Value = serde_json::from_str(&handoff_payload).unwrap();
        assert!(handoff_value["commands"]
            .as_array()
            .unwrap()
            .iter()
            .any(|command| command["command"]
                .as_str()
                .unwrap_or_default()
                .contains("desktop-run-next")));

        let contract_payload = server
            .read_resource("pool://desktop-recognition-contract")
            .unwrap();
        let contract_value: serde_json::Value = serde_json::from_str(&contract_payload).unwrap();
        assert_eq!(
            contract_value["queue"]["result_callback"]["http"],
            "POST /api/desktop-recognition/results"
        );
    }

    fn temp_db_path(prefix: &str) -> PathBuf {
        std::env::temp_dir().join(format!("{prefix}-{}.sqlite", Uuid::new_v4()))
    }
}
