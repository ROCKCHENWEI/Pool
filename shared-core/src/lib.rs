//! Pool shared runtime core.
//!
//! This crate implements the local-first runtime skeleton that merges the
//! ROCKCHENWEI/Pool architecture with image-blaster style project envelopes.

#![recursion_limit = "256"]

pub mod assets;
pub mod control;
pub mod db;
pub mod engine;
pub mod models;
pub mod openclaw;
pub mod providers;
pub mod server;

pub use assets::{
    build_asset_records, infer_asset_type, materialize_project_envelope, parse_indexed_name,
    IndexedName, ProjectEnvelopeManifest,
};
pub use control::{
    default_software_adapters, desktop_recognition_contract_resource, software_control_contract,
    software_control_contracts_resource, spawn_hermes_mcp_bridge_worker,
    spawn_software_api_bridge_worker, spawn_unreal_mcp_bridge_worker,
    unreal_mcp_bridge_contract_resource, AgentCliCommand, AgentCliExecutionOptions,
    AgentCliExecutionReport, AgentSessionExecutionChannel, AgentSessionExecutionReport,
    AgentSessionKind, AgentSessionRunReport, AgentSessionRunner, CommandSoftwareAdapter,
    ControlPriority, DesktopRecognitionAdapter, GenericSoftwareApiAdapter, HermesCommand,
    HermesExecutionOptions, HermesMcpAdapter, HermesMcpAdapterOptions, HermesMcpBridgeWorker,
    HermesMcpBridgeWorkerOptions, HermesMcpBridgeWorkerResponse, MockUnrealAdapter,
    SoftwareActionKind, SoftwareActionResult, SoftwareActionRunReport, SoftwareActionRunner,
    SoftwareAdapter, SoftwareAdapterRegistry, SoftwareApiBridgeWorker,
    SoftwareApiBridgeWorkerOptions, SoftwareApiBridgeWorkerResponse, SoftwareControlAction,
    UnrealMcpAdapter, UnrealMcpAdapterOptions, UnrealMcpBridgeWorker, UnrealMcpBridgeWorkerOptions,
    UnrealMcpBridgeWorkerResponse,
};
pub use db::{
    AgentSessionSnapshot, ApiKeySnapshot, AssetSnapshot, EventSnapshot, NodeRuntimeState,
    ProjectSnapshot, ProviderRequestRecord, ProviderRequestSnapshot, RuntimeRepository,
    RuntimeRepositoryStats, RuntimeSnapshot, RuntimeSnapshotStats, SoftwareActionRecord,
    SoftwareActionSnapshot, TaskSnapshot, WorkflowSnapshot, SCHEMA,
};
pub use engine::{
    build_default_content_burst_plan, conformance_package_catalog_resource,
    core_architecture_package_catalog_resource, default_content_burst_output_root,
    output_package_catalog_resource, prd_completion_package_catalog_resource,
    production_evidence_handoff_package_catalog_resource, runtime_handoff_package_catalog_resource,
    ConformancePackageCatalog, ConformancePackageCatalogSummary, ConformancePackageKind,
    ConformancePackagePolicy, ConformancePackageSummary, ContentBurstAgentMode,
    ContentBurstProviderMode, ContentBurstRunReport, ContentBurstRunRequest, ContentBurstRunner,
    ContentBurstSoftwareMode, CoreArchitecturePackageCatalog,
    CoreArchitecturePackageCatalogSummary, CoreArchitecturePackagePolicy,
    CoreArchitecturePackageSummary, NodeEngine, NodeEngineError, OutputDeliverableResultReport,
    OutputDeliverableResultRequest, OutputDeliverableSummary, OutputManifestMetric,
    OutputPackageCatalog, OutputPackageCatalogSummary, OutputPackagePolicy, OutputPackageRequest,
    OutputPackageRunReport, OutputPackageRunner, PoolRuntimePlan, PrdCompletionPackageCatalog,
    PrdCompletionPackageCatalogSummary, PrdCompletionPackagePolicy, PrdCompletionPackageSummary,
    ProductionEvidenceHandoffPackageCatalog, ProductionEvidenceHandoffPackageCatalogSummary,
    ProductionEvidenceHandoffPackagePolicy, ProductionEvidenceHandoffPackageSummary,
    ProviderTaskRunReport, ProviderTaskRunner, RuntimeHandoffPackageCatalog,
    RuntimeHandoffPackageCatalogSummary, RuntimeHandoffPackagePolicy, RuntimeHandoffPackageRequest,
    RuntimeHandoffPackageRunReport, RuntimeHandoffPackageRunner, RuntimeHandoffPackageSummary,
    TaskQueue, TaskQueueSnapshot,
};
pub use models::{
    AgentSession, ApprovalGate, AssetRecord, AssetStatus, ConnectionKind, NodeStatus, NodeType,
    OutputTarget, Project, ProjectEnvelope, ProjectStatus, ProviderConfig, ProviderKind,
    RuntimeEvent, RuntimeEventLevel, RuntimeTask, Segment, SegmentKind, Shot, ShotStatus,
    SoftwareAdapterConfig, TaskStatus, Workflow, WorkflowConnection, WorkflowNode,
};
pub use openclaw::{
    pool_mcp_prompt_definitions, pool_mcp_prompt_get_result, pool_mcp_prompt_http_path,
    production_evidence_handoff_resource, production_evidence_run_plan_resource,
    production_evidence_tasks_resource, runtime_adapter_catalog_resource, runtime_budget_resource,
    runtime_core_architecture_readiness_resource, runtime_execution_plan_resource,
    runtime_graph_resource, runtime_handoff_resource, runtime_integration_readiness_resource,
    runtime_node_context_index_resource, runtime_node_context_resource,
    runtime_prd_completion_gate_resource, runtime_prd_readiness_resource,
    runtime_preflight_resource, runtime_production_evidence_requirements_resource,
    runtime_workflow_context_index_resource, runtime_workflow_context_resource, McpResource,
    McpServer,
};
pub use providers::{
    default_provider_configs, parse_progress_message, provider_contract,
    provider_contracts_resource, provider_gateway_template_contract,
    provider_gateway_template_translation, provider_gateway_worker_contract,
    sample_provider_gateway_template_request, spawn_provider_gateway_mock,
    spawn_provider_gateway_worker, ComfyUiProgressEvent, ComfyUiProvider, ComfyUiProviderOptions,
    GenericHttpMediaOptions, GenericHttpMediaProvider, GenericHttpMediaRequest, KlingAuth,
    KlingProvider, KlingProviderOptions, KlingVideoRequest, Mock3dgsProvider, OpenAiImageProvider,
    OpenAiImageProviderOptions, OpenAiImageRequest, ProviderAdapter, ProviderGatewayMock,
    ProviderGatewayMockResponse, ProviderGatewayTemplateFamily, ProviderGatewayWorker,
    ProviderGatewayWorkerOptions, ProviderGatewayWorkerResponse, ProviderHealth, ProviderJob,
    ProviderRegistry, ProviderRequest, ProviderSdkWorkerTemplate, ProviderVerification,
    ThreeDgsGatewayOptions, ThreeDgsGatewayProvider, ThreeDgsGatewayRequest,
};
pub use server::{RuntimeHttpConfig, RuntimeHttpResponse, RuntimeHttpServer};

pub fn init() {}
