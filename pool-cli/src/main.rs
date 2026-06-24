use anyhow::{bail, Context, Result};
use pool_core::{
    pool_mcp_prompt_definitions, pool_mcp_prompt_get_result, spawn_provider_gateway_mock,
    HermesMcpBridgeWorker, HermesMcpBridgeWorkerOptions, ProviderGatewayWorker,
    ProviderGatewayWorkerOptions, ProviderSdkWorkerTemplate, RuntimeHttpConfig,
    RuntimeHttpResponse, RuntimeHttpServer, SoftwareApiBridgeWorker,
    SoftwareApiBridgeWorkerOptions, UnrealMcpBridgeWorker, UnrealMcpBridgeWorkerOptions,
};
use serde_json::{json, Map, Value};
use std::env;
use std::fs;
use std::io::{self, BufRead, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
struct Cli {
    db_path: PathBuf,
    project_slug: Option<String>,
    command: Command,
}

#[derive(Debug, Clone)]
enum Command {
    Status,
    Snapshot,
    Projects,
    Resources,
    ApiKeys(ApiKeysArgs),
    Adapters,
    IntegrationReadiness,
    ProviderContracts { provider_id: Option<String> },
    ProviderConformancePackages,
    ProviderConformancePackage(ProviderConformancePackageArgs),
    SoftwareContracts { adapter_id: Option<String> },
    SoftwareConformancePackages,
    SoftwareConformancePackage(SoftwareConformancePackageArgs),
    Tasks,
    Events(EventsArgs),
    RuntimeBudget,
    RuntimePreflight,
    RuntimeExecutionPlan,
    RuntimeExecutionPlanRunNext(RuntimeRunNextArgs),
    RuntimeHandoff,
    RuntimeHandoffPackages,
    CoreArchitectureReadiness,
    CoreArchitectureGate(CoreArchitectureGateArgs),
    CoreArchitecturePackages,
    CoreArchitecturePackage(CoreArchitecturePackageArgs),
    PrdReadiness,
    PrdCompletionGate(PrdCompletionGateArgs),
    PrdCompletionPackages,
    PrdCompletionPackage(PrdCompletionPackageArgs),
    RuntimeGraph,
    WorkflowContext { workflow_id: Option<String> },
    NodeContext { node_id: Option<String> },
    Mcp { uri: String },
    ServeMcp,
    ProviderGatewayWorkerContract,
    ProviderGatewayWorker(ProviderGatewayWorkerArgs),
    ProviderSdkWorkerTemplate(ProviderSdkWorkerTemplateArgs),
    UnrealMcpBridgeContract,
    UnrealMcpBridgeWorker(UnrealMcpBridgeWorkerArgs),
    HermesMcpBridgeWorker(HermesMcpBridgeWorkerArgs),
    SoftwareApiBridgeWorker(SoftwareApiBridgeWorkerArgs),
    WorkerSelfChecks(WorkerSelfChecksArgs),
    AdapterHealth(AdapterHealthArgs),
    ProviderHealth(ProviderHealthArgs),
    RunProvider(ProviderRunArgs),
    ProductionEvidenceProviderMatrix(ProviderEvidenceProviderMatrixArgs),
    ProductionEvidenceSoftwareMatrix(SoftwareEvidenceMatrixArgs),
    ProductionEvidenceDesktopVision(DesktopVisionEvidenceArgs),
    ProductionEvidenceRequirements,
    ProductionEvidenceTasks,
    ProductionEvidenceTaskClaim(ProductionEvidenceTaskClaimArgs),
    ProductionEvidenceRunPlan(ProductionEvidenceRunPlanArgs),
    ProductionEvidenceHandoff(ProductionEvidenceHandoffArgs),
    ProductionEvidenceHandoffPackages,
    ProductionEvidenceHandoffPackage(ProductionEvidenceHandoffPackageArgs),
    ProductionEvidenceTemplate(ProductionEvidenceTemplateArgs),
    ProductionEvidenceItemTemplate(ProductionEvidenceItemTemplateArgs),
    ProductionEvidenceItemFromLedger(ProductionEvidenceItemFromLedgerArgs),
    ProductionEvidenceBundleFromLedger(ProductionEvidenceBundleFromLedgerArgs),
    MergeProductionEvidence(ProductionEvidenceMergeArgs),
    CloseoutProductionEvidence(ProductionEvidenceCloseoutArgs),
    ValidateProductionEvidence { path: String },
    ImportProductionEvidence { path: String },
    ValidateProductionEvidenceItem { path: String },
    SubmitProductionEvidenceItem { path: String },
    ProviderRequestMetadata { provider_request_id: String },
    SoftwareHealth(SoftwareHealthArgs),
    OutputPackages,
    RunNode(RunNodeArgs),
    RunSoftware(SoftwareActionArgs),
    RunWorkflow(WorkflowRunArgs),
    OutputPackage(OutputPackageArgs),
    OutputResult(OutputResultArgs),
    HandoffPackage(HandoffPackageArgs),
    IntegrationConformancePackages,
    IntegrationConformancePackage(IntegrationConformancePackageArgs),
    AgentConformancePackages,
    AgentConformancePackage(AgentConformancePackageArgs),
    AgentSession(AgentSessionArgs),
    AgentTranscript { session_id: String },
    AgentStream(AgentStreamArgs),
    DesktopContract,
    DesktopRequests,
    DesktopRunNext(DesktopRunNextArgs),
    DesktopResult(DesktopResultArgs),
    SetApiKey(SetApiKeyArgs),
    TaskAction(TaskAction),
}

#[derive(Debug, Clone)]
struct EventsArgs {
    after_id: Option<String>,
    limit: Option<u16>,
}

#[derive(Debug, Clone)]
struct AgentStreamArgs {
    session_id: String,
    after_id: Option<String>,
    limit: Option<u16>,
}

#[derive(Debug, Clone)]
struct ApiKeysArgs {
    rotation_days: Option<u64>,
}

#[derive(Debug, Clone, Default)]
struct PrdCompletionGateArgs {
    require_complete: bool,
}

#[derive(Debug, Clone)]
enum TaskActionKind {
    Approve,
    Cancel,
    Retry,
}

#[derive(Debug, Clone)]
struct TaskAction {
    kind: TaskActionKind,
    task_id: String,
}

#[derive(Debug, Clone)]
struct ProviderHealthArgs {
    provider_id: String,
    execution_mode: Option<String>,
    endpoint: Option<String>,
    api_key: Option<String>,
}

#[derive(Debug, Clone)]
struct ProviderRunArgs {
    provider_id: String,
    node_id: Option<String>,
    task_title: Option<String>,
    execution_mode: Option<String>,
    endpoint: Option<String>,
    api_key: Option<String>,
    prompt: Option<String>,
    input_paths: Vec<String>,
    output_dir: Option<String>,
    cost_estimate_tokens: Option<u64>,
    requires_approval: Option<bool>,
    evidence_json: Option<Value>,
}

#[derive(Debug, Clone)]
struct ProviderEvidenceProviderMatrixArgs {
    output_root: Option<String>,
    media_endpoint: Option<String>,
    provider_endpoints: Vec<(String, String)>,
    provider_api_keys: Vec<(String, String)>,
    provider_attestations: Vec<(String, String)>,
    openai_endpoint: Option<String>,
    openai_api_key: Option<String>,
    three_dgs_endpoint: Option<String>,
    evidence_bundle_path: Option<String>,
    production_upstream: bool,
    production_attestation: Option<String>,
    use_env: bool,
}

#[derive(Debug, Clone)]
struct SoftwareEvidenceMatrixArgs {
    output_root: Option<String>,
    software_endpoints: Vec<(String, String)>,
    software_commands: Vec<(String, String)>,
    software_artifacts: Vec<(String, String)>,
    software_attestations: Vec<(String, String)>,
    evidence_bundle_path: Option<String>,
    production_software: bool,
    use_env: bool,
}

#[derive(Debug, Clone)]
struct DesktopVisionEvidenceArgs {
    output_root: Option<String>,
    evidence_bundle_path: Option<String>,
    production_vision: bool,
    trace_path: Option<String>,
    trace_env: Option<String>,
    controller_id: Option<String>,
    controller_id_env: Option<String>,
    external_action_id: Option<String>,
    external_action_id_env: Option<String>,
    production_attestation: Option<String>,
    production_attestation_env: Option<String>,
    limit: usize,
    use_env: bool,
}

#[derive(Debug, Clone)]
struct AdapterHealthArgs {
    include_providers: Option<bool>,
    include_software: Option<bool>,
}

#[derive(Debug, Clone)]
struct SoftwareHealthArgs {
    adapter_id: String,
    priority: Option<String>,
    payload_json: Value,
}

#[derive(Debug, Clone)]
struct SoftwareActionArgs {
    adapter_id: String,
    node_id: Option<String>,
    task_title: Option<String>,
    action_kind: Option<String>,
    priority: Option<String>,
    payload_json: Value,
    evidence_json: Option<Value>,
    requires_confirmation: Option<bool>,
}

#[derive(Debug, Clone)]
struct RunNodeArgs {
    node_id: String,
    prompt: Option<String>,
    execution_mode: Option<String>,
    endpoint: Option<String>,
    api_key: Option<String>,
    input_paths: Vec<String>,
    output_dir: Option<String>,
    duration_ms: Option<u64>,
}

#[derive(Debug, Clone, Default)]
struct RuntimeRunNextArgs {
    node_id: Option<String>,
    task_id: Option<String>,
    execute: bool,
    allow_approval: bool,
    prompt: Option<String>,
    execution_mode: Option<String>,
    endpoint: Option<String>,
    api_key: Option<String>,
    input_paths: Vec<String>,
    output_dir: Option<String>,
    duration_ms: Option<u64>,
}

#[derive(Debug, Clone)]
struct WorkflowRunArgs {
    title: Option<String>,
    prompt: Option<String>,
    source_inputs: Vec<String>,
    output_root: Option<String>,
    duration_ms: Option<u64>,
    agent_mode: Option<String>,
    hermes_endpoint: Option<String>,
    hermes_auth_token: Option<String>,
    agent_requires_confirmation: bool,
    three_dgs_mode: Option<String>,
    three_dgs_provider_id: Option<String>,
    three_dgs_endpoint: Option<String>,
    three_dgs_api_key: Option<String>,
    unreal_mode: Option<String>,
    unreal_endpoint: Option<String>,
    unreal_auth_token: Option<String>,
}

#[derive(Debug, Clone)]
struct OutputPackageArgs {
    node_id: Option<String>,
    title: Option<String>,
    output_dir: Option<String>,
    source_assets: Vec<String>,
    duration_ms: Option<u64>,
}

#[derive(Debug, Clone)]
struct OutputResultArgs {
    node_id: Option<String>,
    target: String,
    local_path: Option<String>,
    status: String,
    runtime: Option<String>,
    adapter_id: Option<String>,
    software_action_id: Option<String>,
    message: Option<String>,
    artifacts: Vec<String>,
    metrics: Vec<(String, String)>,
    verification: Option<Value>,
}

#[derive(Debug, Clone)]
struct HandoffPackageArgs {
    node_id: Option<String>,
    title: Option<String>,
    output_dir: Option<String>,
    include_snapshot: bool,
}

#[derive(Debug, Clone)]
struct IntegrationConformancePackageArgs {
    node_id: Option<String>,
    title: Option<String>,
    output_dir: Option<String>,
    providers: Vec<String>,
    software_adapters: Vec<String>,
    agent_kind: Option<String>,
    include_providers: bool,
    include_software: bool,
    include_agent: bool,
}

#[derive(Debug, Clone)]
struct AgentConformancePackageArgs {
    kind: String,
    node_id: Option<String>,
    title: Option<String>,
    output_dir: Option<String>,
}

#[derive(Debug, Clone)]
struct SoftwareConformancePackageArgs {
    adapter_id: String,
    node_id: Option<String>,
    title: Option<String>,
    output_dir: Option<String>,
}

#[derive(Debug, Clone)]
struct ProviderConformancePackageArgs {
    provider_id: String,
    node_id: Option<String>,
    title: Option<String>,
    output_dir: Option<String>,
}

#[derive(Debug, Clone)]
struct ProductionEvidenceHandoffPackageArgs {
    node_id: Option<String>,
    title: Option<String>,
    output_dir: Option<String>,
    output_root: Option<String>,
    source: Option<String>,
    include_items: bool,
    include_snapshot: bool,
}

#[derive(Debug, Clone)]
struct ProductionEvidenceTaskClaimArgs {
    task_id: String,
    assignee: Option<String>,
    role: Option<String>,
    output_root: Option<String>,
    source: Option<String>,
}

#[derive(Debug, Clone)]
struct PrdCompletionPackageArgs {
    node_id: Option<String>,
    title: Option<String>,
    output_dir: Option<String>,
    source: Option<String>,
    include_snapshot: bool,
}

#[derive(Debug, Clone)]
struct CoreArchitecturePackageArgs {
    node_id: Option<String>,
    title: Option<String>,
    output_dir: Option<String>,
    source: Option<String>,
    include_snapshot: bool,
}

#[derive(Debug, Clone, Default)]
struct CoreArchitectureGateArgs {
    require_ready: bool,
}

#[derive(Debug, Clone)]
struct AgentSessionArgs {
    kind: String,
    control_dir: Option<String>,
    endpoint: Option<String>,
    instruction: Option<String>,
    allowed_tools: Vec<String>,
    requires_confirmation: Option<bool>,
    command_id: Option<String>,
    title: Option<String>,
    command: Option<String>,
    tools: Vec<String>,
    token_budget: Option<u64>,
    execute: bool,
    allowed_commands: Vec<String>,
    working_dir: Option<String>,
    max_output_bytes: Option<usize>,
    timeout_ms: Option<u64>,
}

#[derive(Debug, Clone)]
struct DesktopResultArgs {
    software_action_id: String,
    task_id: Option<String>,
    status: String,
    message: Option<String>,
    artifacts: Vec<String>,
    screen_trace_path: Option<String>,
    result: Option<Value>,
    verification: Option<Value>,
}

#[derive(Debug, Clone)]
struct DesktopRunNextArgs {
    status: String,
    message: Option<String>,
    controller_id: String,
    limit: usize,
    artifacts: Vec<String>,
    screen_trace_path: Option<String>,
}

#[derive(Debug, Clone)]
struct SetApiKeyArgs {
    provider_id: String,
    service_type: String,
    api_key: String,
    metadata: Value,
}

#[derive(Debug, Clone)]
struct ProviderGatewayWorkerArgs {
    bind_addr: String,
    upstream: String,
    provider_upstreams: Vec<(String, String)>,
    api_key: Option<String>,
    provider_api_keys: Vec<(String, String)>,
    max_requests: usize,
    once: bool,
}

#[derive(Debug, Clone)]
struct ProviderSdkWorkerTemplateArgs {
    bind_addr: String,
    output_root: PathBuf,
    max_requests: usize,
    once: bool,
}

#[derive(Debug, Clone)]
struct UnrealMcpBridgeWorkerArgs {
    bind_addr: String,
    output_root: PathBuf,
    upstream: Option<String>,
    api_key: Option<String>,
    max_requests: usize,
    once: bool,
}

#[derive(Debug, Clone)]
struct HermesMcpBridgeWorkerArgs {
    bind_addr: String,
    output_root: PathBuf,
    upstream: Option<String>,
    api_key: Option<String>,
    max_requests: usize,
    once: bool,
}

#[derive(Debug, Clone)]
struct SoftwareApiBridgeWorkerArgs {
    adapter_id: String,
    bind_addr: String,
    output_root: PathBuf,
    upstream: Option<String>,
    api_key: Option<String>,
    max_requests: usize,
    once: bool,
}

#[derive(Debug, Clone)]
struct WorkerSelfChecksArgs {
    output_root: PathBuf,
    software_adapter_id: String,
}

#[derive(Debug, Clone)]
struct ProductionEvidenceTemplateArgs {
    path: Option<String>,
    output_root: Option<String>,
    source: Option<String>,
    missing_only: bool,
}

#[derive(Debug, Clone)]
struct ProductionEvidenceHandoffArgs {
    path: Option<String>,
    output_root: Option<String>,
    source: Option<String>,
}

#[derive(Debug, Clone)]
struct ProductionEvidenceRunPlanArgs {
    path: Option<String>,
    output_root: Option<String>,
    source: Option<String>,
}

#[derive(Debug, Clone)]
struct ProductionEvidenceItemTemplateArgs {
    path: Option<String>,
    output_root: Option<String>,
    source: Option<String>,
    task_id: Option<String>,
    kind: Option<String>,
    target_id: Option<String>,
}

#[derive(Debug, Clone)]
struct ProductionEvidenceItemFromLedgerArgs {
    path: Option<String>,
    source: Option<String>,
    provider_request_id: Option<String>,
    software_action_id: Option<String>,
    desktop_vision_action_id: Option<String>,
}

#[derive(Debug, Clone)]
struct ProductionEvidenceBundleFromLedgerArgs {
    path: Option<String>,
    source: Option<String>,
    include_incomplete: bool,
}

#[derive(Debug, Clone)]
struct ProductionEvidenceMergeArgs {
    output_path: String,
    input_paths: Vec<String>,
    source: Option<String>,
}

#[derive(Debug, Clone)]
struct ProductionEvidenceCloseoutArgs {
    input_paths: Vec<String>,
    source: Option<String>,
    import: bool,
    output_path: Option<String>,
    completion_package: bool,
    completion_package_output_dir: Option<String>,
    completion_package_node_id: Option<String>,
    completion_package_title: Option<String>,
    completion_package_source: Option<String>,
    completion_package_include_snapshot: bool,
}

fn main() -> Result<()> {
    let cli = parse_cli(env::args().skip(1).collect())?;
    if matches!(cli.command, Command::ServeMcp) {
        return serve_mcp_stdio(&cli);
    }
    if let Command::WorkerSelfChecks(args) = cli.command.clone() {
        return run_worker_self_checks(args);
    }
    if let Command::ProviderGatewayWorker(args) = cli.command.clone() {
        return serve_provider_gateway_worker(args);
    }
    if let Command::ProviderSdkWorkerTemplate(args) = cli.command.clone() {
        return serve_provider_sdk_worker_template(args);
    }
    if let Command::UnrealMcpBridgeWorker(args) = cli.command.clone() {
        return serve_unreal_mcp_bridge_worker(args);
    }
    if let Command::HermesMcpBridgeWorker(args) = cli.command.clone() {
        return serve_hermes_mcp_bridge_worker(args);
    }
    if let Command::SoftwareApiBridgeWorker(args) = cli.command.clone() {
        return serve_software_api_bridge_worker(args);
    }
    let response = dispatch(cli)?;
    println!("{}", response.body);
    if response.status_code >= 400 {
        bail!("pool-cli request failed with HTTP {}", response.status_code);
    }
    Ok(())
}

fn serve_provider_gateway_worker(args: ProviderGatewayWorkerArgs) -> Result<()> {
    if args.once {
        let mock_upstream = spawn_provider_gateway_mock(8)?;
        let mut worker = ProviderGatewayWorker::new(
            ProviderGatewayWorkerOptions::new("http://127.0.0.1:8788")
                .with_default_upstream_endpoint(mock_upstream.clone()),
        );
        for (route, status, bytes) in worker.self_check()? {
            println!("{route} status={status} bytes={bytes}");
        }
        println!("mock_upstream={mock_upstream}");
        return Ok(());
    }

    let listener = TcpListener::bind(&args.bind_addr)
        .with_context(|| format!("bind provider gateway worker {}", args.bind_addr))?;
    let local_addr = listener
        .local_addr()
        .context("read provider gateway worker local addr")?;
    let mut options = ProviderGatewayWorkerOptions::new(format!("http://{local_addr}"))
        .with_default_upstream_endpoint(args.upstream.clone());
    if let Some(api_key) = args.api_key {
        options = options.with_api_key(api_key);
    }
    for (provider_id, endpoint) in &args.provider_upstreams {
        options = options.with_provider_upstream(provider_id, endpoint.clone());
    }
    for (provider_id, api_key) in &args.provider_api_keys {
        options = options.with_provider_api_key(provider_id, api_key.clone());
    }
    let mut worker = ProviderGatewayWorker::new(options);

    println!("Pool provider gateway worker listening on http://{local_addr}");
    println!("  upstream {}", args.upstream);
    for (provider_id, endpoint) in &args.provider_upstreams {
        println!("  provider upstream {provider_id} -> {endpoint}");
    }
    println!("  GET  /health");
    println!("  POST /v1/media/jobs");
    println!("  GET  /v1/media/jobs/<job-id>");
    println!("  POST /v1/3dgs/jobs");
    println!("  POST /v1/3dgs/<provider>/jobs");
    println!("  GET  /v1/3dgs/.../jobs/<job-id>");
    let handled = worker.serve_listener(listener, args.max_requests)?;
    if args.max_requests > 0 {
        println!("Pool provider gateway worker handled {handled} request(s) and exited");
    }
    Ok(())
}

fn serve_provider_sdk_worker_template(args: ProviderSdkWorkerTemplateArgs) -> Result<()> {
    fs::create_dir_all(&args.output_root).with_context(|| {
        format!(
            "create provider SDK worker template output root {}",
            args.output_root.display()
        )
    })?;

    if args.once {
        let mut worker =
            ProviderSdkWorkerTemplate::new("http://127.0.0.1:8798", args.output_root.clone());
        for (route, status, bytes) in worker.self_check()? {
            println!("{route} status={status} bytes={bytes}");
        }
        println!("output_root={}", args.output_root.display());
        return Ok(());
    }

    let listener = TcpListener::bind(&args.bind_addr)
        .with_context(|| format!("bind provider SDK worker template {}", args.bind_addr))?;
    let local_addr = listener
        .local_addr()
        .context("read provider SDK worker template local addr")?;
    let mut worker =
        ProviderSdkWorkerTemplate::new(format!("http://{local_addr}"), args.output_root.clone());

    println!("Pool provider SDK worker template listening on http://{local_addr}");
    println!("  output_root {}", args.output_root.display());
    println!("  GET  /health");
    println!("  POST /v1/media/jobs");
    println!("  GET  /v1/media/jobs/<job-id>");
    println!("  POST /v1/3dgs/jobs");
    println!("  POST /v1/3dgs/<provider>/jobs");
    println!("  GET  /v1/3dgs/.../jobs/<job-id>");
    println!("  GET  /outputs/<job-id>/<file>");
    let handled = worker.serve_listener(listener, args.max_requests)?;
    if args.max_requests > 0 {
        println!("Pool provider SDK worker template handled {handled} request(s) and exited");
    }
    Ok(())
}

fn serve_unreal_mcp_bridge_worker(args: UnrealMcpBridgeWorkerArgs) -> Result<()> {
    if args.once {
        fs::create_dir_all(&args.output_root).with_context(|| {
            format!(
                "create Unreal MCP bridge worker output root {}",
                args.output_root.display()
            )
        })?;
        let mut options =
            UnrealMcpBridgeWorkerOptions::new("http://127.0.0.1:8790", args.output_root.clone());
        if let Some(upstream) = args.upstream {
            options = options.with_default_upstream_endpoint(upstream);
        }
        if let Some(api_key) = args.api_key {
            options = options.with_api_key(api_key);
        }
        let mut worker = UnrealMcpBridgeWorker::new(options);
        for (route, status, bytes) in worker.self_check()? {
            println!("{route} status={status} bytes={bytes}");
        }
        println!("output_root={}", args.output_root.display());
        return Ok(());
    }

    let listener = TcpListener::bind(&args.bind_addr)
        .with_context(|| format!("bind Unreal MCP bridge worker {}", args.bind_addr))?;
    let local_addr = listener
        .local_addr()
        .context("read Unreal MCP bridge worker local addr")?;
    let mut options =
        UnrealMcpBridgeWorkerOptions::new(format!("http://{local_addr}"), args.output_root);
    if let Some(upstream) = args.upstream {
        options = options.with_default_upstream_endpoint(upstream);
    }
    if let Some(api_key) = args.api_key {
        options = options.with_api_key(api_key);
    }
    let mut worker = UnrealMcpBridgeWorker::new(options);

    println!("Pool Unreal MCP bridge worker listening on http://{local_addr}");
    println!("  GET  /health");
    println!("  POST /mcp");
    let handled = worker.serve_listener(listener, args.max_requests)?;
    if args.max_requests > 0 {
        println!("Pool Unreal MCP bridge worker handled {handled} request(s) and exited");
    }
    Ok(())
}

fn serve_hermes_mcp_bridge_worker(args: HermesMcpBridgeWorkerArgs) -> Result<()> {
    if args.once {
        fs::create_dir_all(&args.output_root).with_context(|| {
            format!(
                "create Hermes MCP bridge worker output root {}",
                args.output_root.display()
            )
        })?;
        let mut options =
            HermesMcpBridgeWorkerOptions::new("http://127.0.0.1:8792", args.output_root.clone());
        if let Some(upstream) = args.upstream {
            options = options.with_default_upstream_endpoint(upstream);
        }
        if let Some(api_key) = args.api_key {
            options = options.with_api_key(api_key);
        }
        let mut worker = HermesMcpBridgeWorker::new(options);
        for (route, status, bytes) in worker.self_check()? {
            println!("{route} status={status} bytes={bytes}");
        }
        println!("output_root={}", args.output_root.display());
        return Ok(());
    }

    let listener = TcpListener::bind(&args.bind_addr)
        .with_context(|| format!("bind Hermes MCP bridge worker {}", args.bind_addr))?;
    let local_addr = listener
        .local_addr()
        .context("read Hermes MCP bridge worker local addr")?;
    let mut options =
        HermesMcpBridgeWorkerOptions::new(format!("http://{local_addr}"), args.output_root);
    if let Some(upstream) = args.upstream {
        options = options.with_default_upstream_endpoint(upstream);
    }
    if let Some(api_key) = args.api_key {
        options = options.with_api_key(api_key);
    }
    let mut worker = HermesMcpBridgeWorker::new(options);

    println!("Pool Hermes MCP bridge worker listening on http://{local_addr}");
    println!("  GET  /health");
    println!("  POST /mcp");
    let handled = worker.serve_listener(listener, args.max_requests)?;
    if args.max_requests > 0 {
        println!("Pool Hermes MCP bridge worker handled {handled} request(s) and exited");
    }
    Ok(())
}

fn serve_software_api_bridge_worker(args: SoftwareApiBridgeWorkerArgs) -> Result<()> {
    if args.once {
        fs::create_dir_all(&args.output_root).with_context(|| {
            format!(
                "create software API bridge worker output root {}",
                args.output_root.display()
            )
        })?;
        let mut options = SoftwareApiBridgeWorkerOptions::new(
            args.adapter_id.clone(),
            "http://127.0.0.1:8793",
            args.output_root.clone(),
        );
        if let Some(upstream) = args.upstream {
            options = options.with_default_upstream_endpoint(upstream);
        }
        if let Some(api_key) = args.api_key {
            options = options.with_api_key(api_key);
        }
        let mut worker = SoftwareApiBridgeWorker::new(options);
        for (route, status, bytes) in worker.self_check()? {
            println!("{route} status={status} bytes={bytes}");
        }
        println!("adapter_id={}", args.adapter_id);
        println!("output_root={}", args.output_root.display());
        return Ok(());
    }

    let listener = TcpListener::bind(&args.bind_addr)
        .with_context(|| format!("bind software API bridge worker {}", args.bind_addr))?;
    let local_addr = listener
        .local_addr()
        .context("read software API bridge worker local addr")?;
    let mut options = SoftwareApiBridgeWorkerOptions::new(
        args.adapter_id.clone(),
        format!("http://{local_addr}"),
        args.output_root,
    );
    if let Some(upstream) = args.upstream {
        options = options.with_default_upstream_endpoint(upstream);
    }
    if let Some(api_key) = args.api_key {
        options = options.with_api_key(api_key);
    }
    let mut worker = SoftwareApiBridgeWorker::new(options);

    println!(
        "Pool software API bridge worker for {} listening on http://{local_addr}",
        args.adapter_id
    );
    println!("  GET  /health");
    println!("  POST /mcp");
    let handled = worker.serve_listener(listener, args.max_requests)?;
    if args.max_requests > 0 {
        println!("Pool software API bridge worker handled {handled} request(s) and exited");
    }
    Ok(())
}

fn run_worker_self_checks(args: WorkerSelfChecksArgs) -> Result<()> {
    let report = worker_self_checks_report(args)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn worker_self_checks_report(args: WorkerSelfChecksArgs) -> Result<Value> {
    fs::create_dir_all(&args.output_root).with_context(|| {
        format!(
            "create worker self-check output root {}",
            args.output_root.display()
        )
    })?;

    let mock_upstream = spawn_provider_gateway_mock(8)?;
    let mut provider_gateway = ProviderGatewayWorker::new(
        ProviderGatewayWorkerOptions::new("http://127.0.0.1:8788")
            .with_default_upstream_endpoint(mock_upstream.clone()),
    );
    let provider_gateway_routes = provider_gateway.self_check()?;

    let provider_sdk_root = args.output_root.join("provider-sdk-worker-template");
    let mut provider_sdk =
        ProviderSdkWorkerTemplate::new("http://127.0.0.1:8798", provider_sdk_root.clone());
    let provider_sdk_routes = provider_sdk.self_check()?;

    let unreal_root = args.output_root.join("unreal-mcp-bridge-worker");
    let mut unreal = UnrealMcpBridgeWorker::new(UnrealMcpBridgeWorkerOptions::new(
        "http://127.0.0.1:8790",
        unreal_root.clone(),
    ));
    let unreal_routes = unreal.self_check()?;

    let hermes_root = args.output_root.join("hermes-mcp-bridge-worker");
    let mut hermes = HermesMcpBridgeWorker::new(HermesMcpBridgeWorkerOptions::new(
        "http://127.0.0.1:8792",
        hermes_root.clone(),
    ));
    let hermes_routes = hermes.self_check()?;

    let software_root = args.output_root.join("software-api-bridge-worker");
    let mut software = SoftwareApiBridgeWorker::new(SoftwareApiBridgeWorkerOptions::new(
        args.software_adapter_id.clone(),
        "http://127.0.0.1:8793",
        software_root.clone(),
    ));
    let software_routes = software.self_check()?;

    let report = json!({
        "kind": "pool_worker_self_checks",
        "output_root": args.output_root,
        "checks": [
            worker_check_json("provider-gateway-worker", provider_gateway_routes, json!({
                "mock_upstream": mock_upstream
            })),
            worker_check_json("provider-sdk-worker-template", provider_sdk_routes, json!({
                "output_root": provider_sdk_root
            })),
            worker_check_json("unreal-mcp-bridge-worker", unreal_routes, json!({
                "output_root": unreal_root
            })),
            worker_check_json("hermes-mcp-bridge-worker", hermes_routes, json!({
                "output_root": hermes_root
            })),
            worker_check_json("software-api-bridge-worker", software_routes, json!({
                "adapter_id": args.software_adapter_id,
                "output_root": software_root
            }))
        ]
    });
    Ok(report)
}

fn worker_check_json(id: &str, routes: Vec<(String, u16, usize)>, metadata: Value) -> Value {
    let ok = routes
        .iter()
        .all(|(_, status, _)| (200..300).contains(status));
    let routes = routes
        .into_iter()
        .map(|(route, status, bytes)| {
            json!({
                "route": route,
                "status": status,
                "bytes": bytes,
            })
        })
        .collect::<Vec<_>>();

    json!({
        "id": id,
        "ok": ok,
        "routes": routes,
        "metadata": metadata,
    })
}

fn parse_cli(args: Vec<String>) -> Result<Cli> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_help();
        std::process::exit(0);
    }

    let mut db_path = env::var("POOL_RUNTIME_DB")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("target/runtime-http-smoke/pool-runtime.sqlite"));
    let mut project_slug = env::var("POOL_PROJECT").ok();
    let mut command_args = Vec::new();
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--db" => {
                index += 1;
                let value = args.get(index).context("--db requires a path")?;
                db_path = PathBuf::from(value);
            }
            "--project" => {
                index += 1;
                let value = args.get(index).context("--project requires a slug")?;
                project_slug = Some(value.clone());
            }
            value => command_args.push(value.to_string()),
        }
        index += 1;
    }

    let command = parse_command(command_args)?;
    Ok(Cli {
        db_path,
        project_slug,
        command,
    })
}

fn parse_command(args: Vec<String>) -> Result<Command> {
    let Some(command) = args.first().map(String::as_str) else {
        bail!("missing command; run pool-cli --help");
    };

    match command {
        "status" => Ok(Command::Status),
        "snapshot" => Ok(Command::Snapshot),
        "projects" => Ok(Command::Projects),
        "resources" => Ok(Command::Resources),
        "api-keys" => parse_api_keys(args.into_iter().skip(1).collect()).map(Command::ApiKeys),
        "adapters" | "adapter-catalog" => Ok(Command::Adapters),
        "integration-readiness" | "integration-matrix" | "readiness-matrix" => {
            Ok(Command::IntegrationReadiness)
        }
        "provider-contracts" | "provider-contract" => Ok(Command::ProviderContracts {
            provider_id: args.get(1).cloned(),
        }),
        "provider-conformance-packages" | "provider-conformance-catalog" => {
            Ok(Command::ProviderConformancePackages)
        }
        "provider-conformance-package" | "provider-conformance" | "provider-runbook-package" => {
            parse_provider_conformance_package(args.into_iter().skip(1).collect())
                .map(Command::ProviderConformancePackage)
        }
        "integration-conformance-packages" | "integration-conformance-catalog" => {
            Ok(Command::IntegrationConformancePackages)
        }
        "integration-conformance-package"
        | "integration-conformance"
        | "integration-runbook-package" => {
            parse_integration_conformance_package(args.into_iter().skip(1).collect())
                .map(Command::IntegrationConformancePackage)
        }
        "software-contracts" | "software-contract" => Ok(Command::SoftwareContracts {
            adapter_id: args.get(1).cloned(),
        }),
        "software-conformance-packages" | "software-conformance-catalog" => {
            Ok(Command::SoftwareConformancePackages)
        }
        "software-conformance-package" | "software-conformance" | "software-runbook-package" => {
            parse_software_conformance_package(args.into_iter().skip(1).collect())
                .map(Command::SoftwareConformancePackage)
        }
        "tasks" => Ok(Command::Tasks),
        "events" => parse_events(args.into_iter().skip(1).collect()).map(Command::Events),
        "runtime-budget" | "budget" => Ok(Command::RuntimeBudget),
        "runtime-preflight" | "preflight" => Ok(Command::RuntimePreflight),
        "runtime-execution-plan" | "execution-plan" | "plan" => Ok(Command::RuntimeExecutionPlan),
        "runtime-run-next" | "execution-plan-run-next" | "plan-run-next" => {
            parse_runtime_run_next(args.into_iter().skip(1).collect())
                .map(Command::RuntimeExecutionPlanRunNext)
        }
        "runtime-handoff" | "handoff" => Ok(Command::RuntimeHandoff),
        "runtime-handoff-packages" | "handoff-packages" => Ok(Command::RuntimeHandoffPackages),
        "core-architecture-readiness" | "core-readiness" | "architecture-readiness" => {
            Ok(Command::CoreArchitectureReadiness)
        }
        "core-architecture-gate" | "core-gate" | "architecture-gate" => {
            parse_core_architecture_gate(args.into_iter().skip(1).collect())
                .map(Command::CoreArchitectureGate)
        }
        "core-architecture-packages" | "core-packages" | "architecture-packages" => {
            Ok(Command::CoreArchitecturePackages)
        }
        "core-architecture-package" | "core-package" | "architecture-package" => {
            parse_core_architecture_package(args.into_iter().skip(1).collect())
                .map(Command::CoreArchitecturePackage)
        }
        "prd-readiness" | "readiness" | "prd-audit" => Ok(Command::PrdReadiness),
        "prd-completion-gate" | "prd-completion" | "completion-gate" => {
            parse_prd_completion_gate(args.into_iter().skip(1).collect())
                .map(Command::PrdCompletionGate)
        }
        "prd-completion-packages" | "completion-packages" | "prd-proof-packages" => {
            Ok(Command::PrdCompletionPackages)
        }
        "prd-completion-package" | "completion-package" | "prd-proof-package" => {
            parse_prd_completion_package(args.into_iter().skip(1).collect())
                .map(Command::PrdCompletionPackage)
        }
        "runtime-graph" | "graph" => Ok(Command::RuntimeGraph),
        "workflow-context" => Ok(Command::WorkflowContext {
            workflow_id: args.get(1).cloned(),
        }),
        "node-context" => Ok(Command::NodeContext {
            node_id: args.get(1).cloned(),
        }),
        "mcp" => {
            let uri = args.get(1).context("mcp requires <pool://... uri>")?;
            Ok(Command::Mcp { uri: uri.clone() })
        }
        "serve-mcp" | "mcp-serve" => Ok(Command::ServeMcp),
        "provider-gateway-worker-contract"
        | "gateway-worker-contract"
        | "provider-gateway-contract" => Ok(Command::ProviderGatewayWorkerContract),
        "provider-gateway-worker" | "serve-provider-gateway-worker" | "gateway-worker" => {
            parse_provider_gateway_worker(args.into_iter().skip(1).collect())
                .map(Command::ProviderGatewayWorker)
        }
        "provider-sdk-worker-template"
        | "serve-provider-sdk-worker-template"
        | "sdk-worker-template" => {
            parse_provider_sdk_worker_template(args.into_iter().skip(1).collect())
                .map(Command::ProviderSdkWorkerTemplate)
        }
        "unreal-mcp-bridge" | "unreal-mcp-bridge-contract" | "unreal-bridge-contract" => {
            Ok(Command::UnrealMcpBridgeContract)
        }
        "unreal-mcp-bridge-worker" | "serve-unreal-mcp-bridge" | "unreal-bridge-worker" => {
            parse_unreal_mcp_bridge_worker(args.into_iter().skip(1).collect())
                .map(Command::UnrealMcpBridgeWorker)
        }
        "hermes-mcp-bridge-worker" | "serve-hermes-mcp-bridge" | "hermes-bridge-worker" => {
            parse_hermes_mcp_bridge_worker(args.into_iter().skip(1).collect())
                .map(Command::HermesMcpBridgeWorker)
        }
        "software-api-bridge-worker" | "serve-software-api-bridge" | "software-bridge-worker" => {
            parse_software_api_bridge_worker(args.into_iter().skip(1).collect())
                .map(Command::SoftwareApiBridgeWorker)
        }
        "worker-self-checks" | "runtime-worker-self-checks" | "bridge-self-checks" => {
            parse_worker_self_checks(args.into_iter().skip(1).collect())
                .map(Command::WorkerSelfChecks)
        }
        "adapter-health" => {
            parse_adapter_health(args.into_iter().skip(1).collect()).map(Command::AdapterHealth)
        }
        "provider-health" => {
            parse_provider_health(args.into_iter().skip(1).collect()).map(Command::ProviderHealth)
        }
        "run-provider" => {
            parse_provider_run(args.into_iter().skip(1).collect()).map(Command::RunProvider)
        }
        "production-evidence-provider-matrix" | "provider-evidence-matrix" => {
            parse_provider_evidence_provider_matrix(args.into_iter().skip(1).collect())
                .map(Command::ProductionEvidenceProviderMatrix)
        }
        "production-evidence-software-matrix" | "software-evidence-matrix" => {
            parse_software_evidence_matrix(args.into_iter().skip(1).collect())
                .map(Command::ProductionEvidenceSoftwareMatrix)
        }
        "production-evidence-desktop-vision" | "desktop-vision-evidence" => {
            parse_desktop_vision_evidence(args.into_iter().skip(1).collect())
                .map(Command::ProductionEvidenceDesktopVision)
        }
        "production-evidence-requirements" | "production-evidence-doctor" => {
            Ok(Command::ProductionEvidenceRequirements)
        }
        "production-evidence-tasks" | "production-evidence-queue" => {
            Ok(Command::ProductionEvidenceTasks)
        }
        "production-evidence-claim" | "claim-production-evidence-task" => {
            parse_production_evidence_task_claim(args.into_iter().skip(1).collect())
                .map(Command::ProductionEvidenceTaskClaim)
        }
        "production-evidence-run-plan" | "production-evidence-plan" => {
            parse_production_evidence_run_plan(args.into_iter().skip(1).collect())
                .map(Command::ProductionEvidenceRunPlan)
        }
        "production-evidence-handoff" | "production-evidence-package" => {
            parse_production_evidence_handoff(args.into_iter().skip(1).collect())
                .map(Command::ProductionEvidenceHandoff)
        }
        "production-evidence-handoff-packages"
        | "production-evidence-package-catalog"
        | "production-evidence-proof-packages" => Ok(Command::ProductionEvidenceHandoffPackages),
        "production-evidence-handoff-package" | "production-evidence-package-files" => {
            parse_production_evidence_handoff_package(args.into_iter().skip(1).collect())
                .map(Command::ProductionEvidenceHandoffPackage)
        }
        "production-evidence-template" | "production-evidence-scaffold" => {
            parse_production_evidence_template(args.into_iter().skip(1).collect())
                .map(Command::ProductionEvidenceTemplate)
        }
        "production-evidence-item-template" | "production-evidence-item-scaffold" => {
            parse_production_evidence_item_template(args.into_iter().skip(1).collect())
                .map(Command::ProductionEvidenceItemTemplate)
        }
        "production-evidence-item-from-ledger" | "production-evidence-ledger-item" => {
            parse_production_evidence_item_from_ledger(args.into_iter().skip(1).collect())
                .map(Command::ProductionEvidenceItemFromLedger)
        }
        "production-evidence-bundle-from-ledger" | "production-evidence-ledger-bundle" => {
            parse_production_evidence_bundle_from_ledger(args.into_iter().skip(1).collect())
                .map(Command::ProductionEvidenceBundleFromLedger)
        }
        "merge-production-evidence" | "production-evidence-merge" => {
            parse_merge_production_evidence(args.into_iter().skip(1).collect())
                .map(Command::MergeProductionEvidence)
        }
        "closeout-production-evidence" | "production-evidence-closeout" => {
            parse_closeout_production_evidence(args.into_iter().skip(1).collect())
                .map(Command::CloseoutProductionEvidence)
        }
        "validate-production-evidence" | "production-evidence-validate" => {
            let path = args
                .get(1)
                .context("validate-production-evidence requires <bundle.json>")?;
            Ok(Command::ValidateProductionEvidence { path: path.clone() })
        }
        "validate-production-evidence-item" | "production-evidence-item-validate" => {
            let path = args
                .get(1)
                .context("validate-production-evidence-item requires <item.json>")?;
            Ok(Command::ValidateProductionEvidenceItem { path: path.clone() })
        }
        "import-production-evidence" | "production-evidence" => {
            let path = args
                .get(1)
                .context("import-production-evidence requires <bundle.json>")?;
            Ok(Command::ImportProductionEvidence { path: path.clone() })
        }
        "submit-production-evidence-item" | "production-evidence-item" => {
            let path = args
                .get(1)
                .context("submit-production-evidence-item requires <item.json>")?;
            Ok(Command::SubmitProductionEvidenceItem { path: path.clone() })
        }
        "provider-request-metadata" | "provider-metadata" => {
            let provider_request_id = args
                .get(1)
                .context("provider-request-metadata requires <provider-request-id>")?;
            Ok(Command::ProviderRequestMetadata {
                provider_request_id: provider_request_id.clone(),
            })
        }
        "software-health" => {
            parse_software_health(args.into_iter().skip(1).collect()).map(Command::SoftwareHealth)
        }
        "output-packages" | "deliverables" => Ok(Command::OutputPackages),
        "run-software" => {
            parse_software_action(args.into_iter().skip(1).collect()).map(Command::RunSoftware)
        }
        "run-node" => parse_run_node(args.into_iter().skip(1).collect()).map(Command::RunNode),
        "run-workflow" | "workflow-run" => {
            parse_workflow_run(args.into_iter().skip(1).collect()).map(Command::RunWorkflow)
        }
        "output-package" | "run-output-package" => {
            parse_output_package(args.into_iter().skip(1).collect()).map(Command::OutputPackage)
        }
        "output-result" | "deliverable-result" => {
            parse_output_result(args.into_iter().skip(1).collect()).map(Command::OutputResult)
        }
        "handoff-package" | "runtime-handoff-package" => {
            parse_handoff_package(args.into_iter().skip(1).collect()).map(Command::HandoffPackage)
        }
        "agent-conformance-packages" | "agent-conformance-catalog" => {
            Ok(Command::AgentConformancePackages)
        }
        "agent-conformance-package" | "agent-conformance" | "agent-runbook-package" => {
            parse_agent_conformance_package(args.into_iter().skip(1).collect())
                .map(Command::AgentConformancePackage)
        }
        "agent-session" => {
            parse_agent_session(args.into_iter().skip(1).collect()).map(Command::AgentSession)
        }
        "agent-transcript" | "agent-session-transcript" => {
            let session_id = args
                .get(1)
                .context("agent-transcript requires <session-id>")?;
            Ok(Command::AgentTranscript {
                session_id: session_id.clone(),
            })
        }
        "agent-stream" | "agent-session-stream" => {
            parse_agent_stream(args.into_iter().skip(1).collect()).map(Command::AgentStream)
        }
        "desktop-contract" | "desktop-recognition-contract" => Ok(Command::DesktopContract),
        "desktop-requests" => Ok(Command::DesktopRequests),
        "desktop-run-next" | "desktop-controller" | "desktop-run" => {
            parse_desktop_run_next(args.into_iter().skip(1).collect()).map(Command::DesktopRunNext)
        }
        "desktop-result" => {
            parse_desktop_result(args.into_iter().skip(1).collect()).map(Command::DesktopResult)
        }
        "set-api-key" => {
            parse_set_api_key(args.into_iter().skip(1).collect()).map(Command::SetApiKey)
        }
        "approve-task" | "approve" => {
            parse_task_action(TaskActionKind::Approve, args.into_iter().skip(1).collect())
                .map(Command::TaskAction)
        }
        "cancel-task" | "cancel" => {
            parse_task_action(TaskActionKind::Cancel, args.into_iter().skip(1).collect())
                .map(Command::TaskAction)
        }
        "retry-task" | "retry" => {
            parse_task_action(TaskActionKind::Retry, args.into_iter().skip(1).collect())
                .map(Command::TaskAction)
        }
        _ => bail!("unknown command: {command}; run pool-cli --help"),
    }
}

fn parse_events(args: Vec<String>) -> Result<EventsArgs> {
    let mut events = EventsArgs {
        after_id: None,
        limit: None,
    };
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--after-id" => {
                index += 1;
                events.after_id = Some(args.get(index).context("--after-id requires id")?.clone());
            }
            "--limit" => {
                index += 1;
                let value = args.get(index).context("--limit requires integer")?;
                events.limit = Some(value.parse().context("--limit must be an integer")?);
            }
            value => bail!("unknown events option: {value}"),
        }
        index += 1;
    }

    Ok(events)
}

fn parse_api_keys(args: Vec<String>) -> Result<ApiKeysArgs> {
    let mut rotation_days = None;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--rotation-days" | "--rotation-interval-days" => {
                index += 1;
                let value = args
                    .get(index)
                    .context("--rotation-days requires integer")?;
                rotation_days = Some(
                    value
                        .parse()
                        .context("--rotation-days must be an integer")?,
                );
            }
            value => bail!("unknown api-keys option: {value}"),
        }
        index += 1;
    }

    Ok(ApiKeysArgs { rotation_days })
}

fn parse_agent_stream(args: Vec<String>) -> Result<AgentStreamArgs> {
    let session_id = args
        .first()
        .context("agent-stream requires <session-id>")?
        .clone();
    let mut stream = AgentStreamArgs {
        session_id,
        after_id: None,
        limit: None,
    };
    let mut index = 1;

    while index < args.len() {
        match args[index].as_str() {
            "--after-id" | "--last-event-id" => {
                index += 1;
                stream.after_id = Some(args.get(index).context("--after-id requires id")?.clone());
            }
            "--limit" => {
                index += 1;
                let value = args.get(index).context("--limit requires integer")?;
                stream.limit = Some(value.parse().context("--limit must be an integer")?);
            }
            value => bail!("unknown agent-stream option: {value}"),
        }
        index += 1;
    }

    Ok(stream)
}

fn parse_provider_gateway_worker(args: Vec<String>) -> Result<ProviderGatewayWorkerArgs> {
    let mut worker = ProviderGatewayWorkerArgs {
        bind_addr: "127.0.0.1:8788".to_string(),
        upstream: env::var("POOL_PROVIDER_GATEWAY_UPSTREAM")
            .unwrap_or_else(|_| "http://127.0.0.1:8787".to_string()),
        provider_upstreams: Vec::new(),
        api_key: None,
        provider_api_keys: Vec::new(),
        max_requests: 0,
        once: false,
    };
    let mut api_key_env = None;
    let mut index = 0;

    while index < args.len() {
        let arg = args[index].as_str();
        if let Some(value) = arg.strip_prefix("--bind=") {
            worker.bind_addr = value.to_string();
        } else if let Some(value) = arg.strip_prefix("--upstream=") {
            worker.upstream = value.to_string();
        } else if let Some(value) = arg.strip_prefix("--provider-upstream=") {
            worker
                .provider_upstreams
                .push(parse_provider_pair(value, "--provider-upstream")?);
        } else if let Some(value) = arg.strip_prefix("--max-requests=") {
            worker.max_requests = value.parse().context("--max-requests must be an integer")?;
        } else if let Some(value) = arg.strip_prefix("--api-key=") {
            worker.api_key = Some(value.to_string());
        } else if let Some(value) = arg.strip_prefix("--api-key-env=") {
            api_key_env = Some(value.to_string());
        } else if let Some(value) = arg.strip_prefix("--provider-api-key=") {
            worker
                .provider_api_keys
                .push(parse_provider_pair(value, "--provider-api-key")?);
        } else if let Some(value) = arg.strip_prefix("--provider-api-key-env=") {
            let (provider_id, env_name) = parse_provider_pair(value, "--provider-api-key-env")?;
            if let Ok(api_key) = env::var(&env_name) {
                worker.provider_api_keys.push((provider_id, api_key));
            }
        } else {
            match arg {
                "once" | "--once" => worker.once = true,
                "--bind" => {
                    index += 1;
                    worker.bind_addr = args.get(index).context("--bind requires addr")?.clone();
                }
                "--upstream" => {
                    index += 1;
                    worker.upstream = args.get(index).context("--upstream requires url")?.clone();
                }
                "--provider-upstream" => {
                    index += 1;
                    worker.provider_upstreams.push(parse_provider_pair(
                        args.get(index)
                            .context("--provider-upstream requires provider=url")?,
                        "--provider-upstream",
                    )?);
                }
                "--max-requests" => {
                    index += 1;
                    let value = args.get(index).context("--max-requests requires integer")?;
                    worker.max_requests =
                        value.parse().context("--max-requests must be an integer")?;
                }
                "--api-key" => {
                    index += 1;
                    worker.api_key =
                        Some(args.get(index).context("--api-key requires value")?.clone());
                }
                "--api-key-env" => {
                    index += 1;
                    api_key_env = Some(
                        args.get(index)
                            .context("--api-key-env requires environment variable name")?
                            .clone(),
                    );
                }
                "--provider-api-key" => {
                    index += 1;
                    worker.provider_api_keys.push(parse_provider_pair(
                        args.get(index)
                            .context("--provider-api-key requires provider=token")?,
                        "--provider-api-key",
                    )?);
                }
                "--provider-api-key-env" => {
                    index += 1;
                    let (provider_id, env_name) = parse_provider_pair(
                        args.get(index)
                            .context("--provider-api-key-env requires provider=ENV_NAME")?,
                        "--provider-api-key-env",
                    )?;
                    if let Ok(api_key) = env::var(&env_name) {
                        worker.provider_api_keys.push((provider_id, api_key));
                    }
                }
                value => bail!("unknown provider-gateway-worker option: {value}"),
            }
        }
        index += 1;
    }

    worker.api_key = resolve_api_key(worker.api_key, api_key_env)?;
    Ok(worker)
}

fn parse_provider_sdk_worker_template(args: Vec<String>) -> Result<ProviderSdkWorkerTemplateArgs> {
    let mut worker = ProviderSdkWorkerTemplateArgs {
        bind_addr: "127.0.0.1:8798".to_string(),
        output_root: PathBuf::from("target/provider-sdk-worker-template"),
        max_requests: 0,
        once: false,
    };
    let mut index = 0;

    while index < args.len() {
        let arg = args[index].as_str();
        if let Some(value) = arg.strip_prefix("--bind=") {
            worker.bind_addr = value.to_string();
        } else if let Some(value) = arg.strip_prefix("--output-root=") {
            worker.output_root = PathBuf::from(value);
        } else if let Some(value) = arg.strip_prefix("--max-requests=") {
            worker.max_requests = value.parse().context("--max-requests must be an integer")?;
        } else {
            match arg {
                "once" | "--once" => worker.once = true,
                "--bind" => {
                    index += 1;
                    worker.bind_addr = args.get(index).context("--bind requires addr")?.clone();
                }
                "--output-root" => {
                    index += 1;
                    worker.output_root = PathBuf::from(
                        args.get(index)
                            .context("--output-root requires directory")?,
                    );
                }
                "--max-requests" => {
                    index += 1;
                    let value = args.get(index).context("--max-requests requires integer")?;
                    worker.max_requests =
                        value.parse().context("--max-requests must be an integer")?;
                }
                value => bail!("unknown provider-sdk-worker-template option: {value}"),
            }
        }
        index += 1;
    }

    Ok(worker)
}

fn parse_provider_pair(raw: &str, option_name: &str) -> Result<(String, String)> {
    let (provider_id, value) = raw
        .split_once('=')
        .with_context(|| format!("{option_name} requires provider=value"))?;
    let provider_id = provider_id.trim();
    let value = value.trim();
    if provider_id.is_empty() || value.is_empty() {
        bail!("{option_name} requires non-empty provider and value");
    }
    Ok((provider_id.to_string(), value.to_string()))
}

fn parse_unreal_mcp_bridge_worker(args: Vec<String>) -> Result<UnrealMcpBridgeWorkerArgs> {
    let mut worker = UnrealMcpBridgeWorkerArgs {
        bind_addr: "127.0.0.1:8790".to_string(),
        output_root: PathBuf::from(
            env::var("POOL_UNREAL_MCP_BRIDGE_OUTPUT_ROOT")
                .unwrap_or_else(|_| "worlds/demo/output".to_string()),
        ),
        upstream: env::var("POOL_UNREAL_MCP_BRIDGE_UPSTREAM").ok(),
        api_key: None,
        max_requests: 0,
        once: false,
    };
    let mut api_key_env = None;
    let mut index = 0;

    while index < args.len() {
        let arg = args[index].as_str();
        if let Some(value) = arg.strip_prefix("--bind=") {
            worker.bind_addr = value.to_string();
        } else if let Some(value) = arg.strip_prefix("--output-root=") {
            worker.output_root = PathBuf::from(value);
        } else if let Some(value) = arg.strip_prefix("--upstream=") {
            worker.upstream = Some(value.to_string());
        } else if let Some(value) = arg.strip_prefix("--max-requests=") {
            worker.max_requests = value.parse().context("--max-requests must be an integer")?;
        } else if let Some(value) = arg.strip_prefix("--api-key=") {
            worker.api_key = Some(value.to_string());
        } else if let Some(value) = arg.strip_prefix("--api-key-env=") {
            api_key_env = Some(value.to_string());
        } else {
            match arg {
                "once" | "--once" => worker.once = true,
                "--bind" => {
                    index += 1;
                    worker.bind_addr = args.get(index).context("--bind requires addr")?.clone();
                }
                "--output-root" => {
                    index += 1;
                    worker.output_root =
                        PathBuf::from(args.get(index).context("--output-root requires path")?);
                }
                "--upstream" => {
                    index += 1;
                    worker.upstream =
                        Some(args.get(index).context("--upstream requires url")?.clone());
                }
                "--max-requests" => {
                    index += 1;
                    let value = args.get(index).context("--max-requests requires integer")?;
                    worker.max_requests =
                        value.parse().context("--max-requests must be an integer")?;
                }
                "--api-key" => {
                    index += 1;
                    worker.api_key =
                        Some(args.get(index).context("--api-key requires value")?.clone());
                }
                "--api-key-env" => {
                    index += 1;
                    api_key_env = Some(
                        args.get(index)
                            .context("--api-key-env requires environment variable name")?
                            .clone(),
                    );
                }
                value => bail!("unknown unreal-mcp-bridge-worker option: {value}"),
            }
        }
        index += 1;
    }

    worker.api_key = resolve_api_key(worker.api_key, api_key_env)?;
    Ok(worker)
}

fn parse_hermes_mcp_bridge_worker(args: Vec<String>) -> Result<HermesMcpBridgeWorkerArgs> {
    let mut worker = HermesMcpBridgeWorkerArgs {
        bind_addr: "127.0.0.1:8792".to_string(),
        output_root: PathBuf::from(
            env::var("POOL_HERMES_MCP_BRIDGE_OUTPUT_ROOT")
                .unwrap_or_else(|_| "worlds/demo/output".to_string()),
        ),
        upstream: env::var("POOL_HERMES_MCP_BRIDGE_UPSTREAM").ok(),
        api_key: None,
        max_requests: 0,
        once: false,
    };
    let mut api_key_env = None;
    let mut index = 0;

    while index < args.len() {
        let arg = args[index].as_str();
        if let Some(value) = arg.strip_prefix("--bind=") {
            worker.bind_addr = value.to_string();
        } else if let Some(value) = arg.strip_prefix("--output-root=") {
            worker.output_root = PathBuf::from(value);
        } else if let Some(value) = arg.strip_prefix("--upstream=") {
            worker.upstream = Some(value.to_string());
        } else if let Some(value) = arg.strip_prefix("--max-requests=") {
            worker.max_requests = value.parse().context("--max-requests must be an integer")?;
        } else if let Some(value) = arg.strip_prefix("--api-key=") {
            worker.api_key = Some(value.to_string());
        } else if let Some(value) = arg.strip_prefix("--api-key-env=") {
            api_key_env = Some(value.to_string());
        } else {
            match arg {
                "once" | "--once" => worker.once = true,
                "--bind" => {
                    index += 1;
                    worker.bind_addr = args.get(index).context("--bind requires addr")?.clone();
                }
                "--output-root" => {
                    index += 1;
                    worker.output_root =
                        PathBuf::from(args.get(index).context("--output-root requires path")?);
                }
                "--upstream" => {
                    index += 1;
                    worker.upstream =
                        Some(args.get(index).context("--upstream requires url")?.clone());
                }
                "--max-requests" => {
                    index += 1;
                    let value = args.get(index).context("--max-requests requires integer")?;
                    worker.max_requests =
                        value.parse().context("--max-requests must be an integer")?;
                }
                "--api-key" => {
                    index += 1;
                    worker.api_key =
                        Some(args.get(index).context("--api-key requires value")?.clone());
                }
                "--api-key-env" => {
                    index += 1;
                    api_key_env = Some(
                        args.get(index)
                            .context("--api-key-env requires environment variable name")?
                            .clone(),
                    );
                }
                value => bail!("unknown hermes-mcp-bridge-worker option: {value}"),
            }
        }
        index += 1;
    }

    worker.api_key = resolve_api_key(worker.api_key, api_key_env)?;
    Ok(worker)
}

fn parse_software_api_bridge_worker(args: Vec<String>) -> Result<SoftwareApiBridgeWorkerArgs> {
    let mut worker = SoftwareApiBridgeWorkerArgs {
        adapter_id: String::new(),
        bind_addr: "127.0.0.1:8793".to_string(),
        output_root: PathBuf::from(
            env::var("POOL_SOFTWARE_API_BRIDGE_OUTPUT_ROOT")
                .unwrap_or_else(|_| "worlds/demo/output".to_string()),
        ),
        upstream: env::var("POOL_SOFTWARE_API_BRIDGE_UPSTREAM").ok(),
        api_key: None,
        max_requests: 0,
        once: false,
    };
    let mut api_key_env = None;
    let mut index = 0;

    while index < args.len() {
        let arg = args[index].as_str();
        if let Some(value) = arg.strip_prefix("--adapter=") {
            worker.adapter_id = value.to_string();
        } else if let Some(value) = arg.strip_prefix("--bind=") {
            worker.bind_addr = value.to_string();
        } else if let Some(value) = arg.strip_prefix("--output-root=") {
            worker.output_root = PathBuf::from(value);
        } else if let Some(value) = arg.strip_prefix("--upstream=") {
            worker.upstream = Some(value.to_string());
        } else if let Some(value) = arg.strip_prefix("--max-requests=") {
            worker.max_requests = value.parse().context("--max-requests must be an integer")?;
        } else if let Some(value) = arg.strip_prefix("--api-key=") {
            worker.api_key = Some(value.to_string());
        } else if let Some(value) = arg.strip_prefix("--api-key-env=") {
            api_key_env = Some(value.to_string());
        } else {
            match arg {
                "once" | "--once" => worker.once = true,
                "--adapter" => {
                    index += 1;
                    worker.adapter_id = args.get(index).context("--adapter requires id")?.clone();
                }
                "--bind" => {
                    index += 1;
                    worker.bind_addr = args.get(index).context("--bind requires addr")?.clone();
                }
                "--output-root" => {
                    index += 1;
                    worker.output_root =
                        PathBuf::from(args.get(index).context("--output-root requires path")?);
                }
                "--upstream" => {
                    index += 1;
                    worker.upstream =
                        Some(args.get(index).context("--upstream requires url")?.clone());
                }
                "--max-requests" => {
                    index += 1;
                    let value = args.get(index).context("--max-requests requires integer")?;
                    worker.max_requests =
                        value.parse().context("--max-requests must be an integer")?;
                }
                "--api-key" => {
                    index += 1;
                    worker.api_key =
                        Some(args.get(index).context("--api-key requires value")?.clone());
                }
                "--api-key-env" => {
                    index += 1;
                    api_key_env = Some(
                        args.get(index)
                            .context("--api-key-env requires environment variable name")?
                            .clone(),
                    );
                }
                value if !value.starts_with('-') && worker.adapter_id.is_empty() => {
                    worker.adapter_id = value.to_string();
                }
                value => bail!("unknown software-api-bridge-worker option: {value}"),
            }
        }
        index += 1;
    }

    if worker.adapter_id.trim().is_empty() {
        bail!("software-api-bridge-worker requires <adapter-id> or --adapter <id>");
    }
    worker.api_key = resolve_api_key(worker.api_key, api_key_env)?;
    Ok(worker)
}

fn parse_worker_self_checks(args: Vec<String>) -> Result<WorkerSelfChecksArgs> {
    let mut checks = WorkerSelfChecksArgs {
        output_root: PathBuf::from("target/pool-worker-self-checks"),
        software_adapter_id: "resolve".to_string(),
    };
    let mut index = 0;

    while index < args.len() {
        let arg = args[index].as_str();
        if let Some(value) = arg.strip_prefix("--output-root=") {
            checks.output_root = PathBuf::from(value);
        } else if let Some(value) = arg.strip_prefix("--software-adapter=") {
            checks.software_adapter_id = value.to_string();
        } else if let Some(value) = arg.strip_prefix("--adapter=") {
            checks.software_adapter_id = value.to_string();
        } else {
            match arg {
                "--output-root" => {
                    index += 1;
                    checks.output_root =
                        PathBuf::from(args.get(index).context("--output-root requires path")?);
                }
                "--software-adapter" | "--adapter" => {
                    index += 1;
                    checks.software_adapter_id = args
                        .get(index)
                        .context("--software-adapter requires id")?
                        .clone();
                }
                value => bail!("unknown worker-self-checks option: {value}"),
            }
        }
        index += 1;
    }

    if checks.software_adapter_id.trim().is_empty() {
        bail!("worker-self-checks requires non-empty --software-adapter");
    }
    Ok(checks)
}

fn parse_production_evidence_template(args: Vec<String>) -> Result<ProductionEvidenceTemplateArgs> {
    let mut template = ProductionEvidenceTemplateArgs {
        path: None,
        output_root: None,
        source: None,
        missing_only: false,
    };
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--output-root" => {
                index += 1;
                template.output_root = Some(
                    args.get(index)
                        .context("--output-root requires path")?
                        .clone(),
                );
            }
            "--source" => {
                index += 1;
                template.source = Some(args.get(index).context("--source requires value")?.clone());
            }
            "--missing-only" => {
                template.missing_only = true;
            }
            value if value.starts_with("--output-root=") => {
                template.output_root = Some(
                    value
                        .strip_prefix("--output-root=")
                        .unwrap_or_default()
                        .to_string(),
                );
            }
            value if value.starts_with("--source=") => {
                template.source = Some(
                    value
                        .strip_prefix("--source=")
                        .unwrap_or_default()
                        .to_string(),
                );
            }
            value if value.starts_with("--missing-only=") => {
                template.missing_only =
                    cli_query_bool(value.strip_prefix("--missing-only=").unwrap_or_default());
            }
            value if value.starts_with('-') => {
                bail!("unknown production-evidence-template option: {value}");
            }
            value => {
                if template.path.is_some() {
                    bail!("production-evidence-template accepts at most one output path");
                }
                template.path = Some(value.to_string());
            }
        }
        index += 1;
    }

    Ok(template)
}

fn parse_production_evidence_handoff(args: Vec<String>) -> Result<ProductionEvidenceHandoffArgs> {
    let mut handoff = ProductionEvidenceHandoffArgs {
        path: None,
        output_root: None,
        source: None,
    };
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--output-root" => {
                index += 1;
                handoff.output_root = Some(
                    args.get(index)
                        .context("--output-root requires path")?
                        .clone(),
                );
            }
            "--source" => {
                index += 1;
                handoff.source = Some(args.get(index).context("--source requires value")?.clone());
            }
            value if value.starts_with("--output-root=") => {
                handoff.output_root = Some(
                    value
                        .strip_prefix("--output-root=")
                        .unwrap_or_default()
                        .to_string(),
                );
            }
            value if value.starts_with("--source=") => {
                handoff.source = Some(
                    value
                        .strip_prefix("--source=")
                        .unwrap_or_default()
                        .to_string(),
                );
            }
            value if value.starts_with('-') => {
                bail!("unknown production-evidence-handoff option: {value}");
            }
            value => {
                if handoff.path.is_some() {
                    bail!("production-evidence-handoff accepts at most one output path");
                }
                handoff.path = Some(value.to_string());
            }
        }
        index += 1;
    }

    Ok(handoff)
}

fn parse_production_evidence_run_plan(args: Vec<String>) -> Result<ProductionEvidenceRunPlanArgs> {
    let mut run_plan = ProductionEvidenceRunPlanArgs {
        path: None,
        output_root: None,
        source: None,
    };
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--output-root" => {
                index += 1;
                run_plan.output_root = Some(
                    args.get(index)
                        .context("--output-root requires path")?
                        .clone(),
                );
            }
            "--source" => {
                index += 1;
                run_plan.source = Some(args.get(index).context("--source requires value")?.clone());
            }
            value if value.starts_with("--output-root=") => {
                run_plan.output_root = Some(
                    value
                        .strip_prefix("--output-root=")
                        .unwrap_or_default()
                        .to_string(),
                );
            }
            value if value.starts_with("--source=") => {
                run_plan.source = Some(
                    value
                        .strip_prefix("--source=")
                        .unwrap_or_default()
                        .to_string(),
                );
            }
            value if value.starts_with('-') => {
                bail!("unknown production-evidence-run-plan option: {value}");
            }
            value => {
                if run_plan.path.is_some() {
                    bail!("production-evidence-run-plan accepts at most one output path");
                }
                run_plan.path = Some(value.to_string());
            }
        }
        index += 1;
    }

    Ok(run_plan)
}

fn parse_production_evidence_task_claim(
    args: Vec<String>,
) -> Result<ProductionEvidenceTaskClaimArgs> {
    let mut claim = ProductionEvidenceTaskClaimArgs {
        task_id: String::new(),
        assignee: None,
        role: None,
        output_root: None,
        source: None,
    };
    let mut task_id = None;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--assignee" => {
                index += 1;
                claim.assignee = Some(
                    args.get(index)
                        .context("--assignee requires value")?
                        .clone(),
                );
            }
            "--role" => {
                index += 1;
                claim.role = Some(args.get(index).context("--role requires value")?.clone());
            }
            "--output-root" => {
                index += 1;
                claim.output_root = Some(
                    args.get(index)
                        .context("--output-root requires path")?
                        .clone(),
                );
            }
            "--source" => {
                index += 1;
                claim.source = Some(args.get(index).context("--source requires value")?.clone());
            }
            value if value.starts_with("--assignee=") => {
                claim.assignee = Some(
                    value
                        .strip_prefix("--assignee=")
                        .unwrap_or_default()
                        .to_string(),
                );
            }
            value if value.starts_with("--role=") => {
                claim.role = Some(
                    value
                        .strip_prefix("--role=")
                        .unwrap_or_default()
                        .to_string(),
                );
            }
            value if value.starts_with("--output-root=") => {
                claim.output_root = Some(
                    value
                        .strip_prefix("--output-root=")
                        .unwrap_or_default()
                        .to_string(),
                );
            }
            value if value.starts_with("--source=") => {
                claim.source = Some(
                    value
                        .strip_prefix("--source=")
                        .unwrap_or_default()
                        .to_string(),
                );
            }
            value if value.starts_with('-') => {
                bail!("unknown production-evidence-claim option: {value}");
            }
            value => {
                if task_id.is_some() {
                    bail!("production-evidence-claim accepts exactly one task id");
                }
                task_id = Some(value.to_string());
            }
        }
        index += 1;
    }

    claim.task_id = task_id.context("production-evidence-claim requires <task-id>")?;
    Ok(claim)
}

fn parse_production_evidence_item_template(
    args: Vec<String>,
) -> Result<ProductionEvidenceItemTemplateArgs> {
    let mut template = ProductionEvidenceItemTemplateArgs {
        path: None,
        output_root: None,
        source: None,
        task_id: None,
        kind: None,
        target_id: None,
    };
    let mut positionals = Vec::new();
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--output-root" => {
                index += 1;
                template.output_root = Some(
                    args.get(index)
                        .context("--output-root requires path")?
                        .clone(),
                );
            }
            "--source" => {
                index += 1;
                template.source = Some(args.get(index).context("--source requires value")?.clone());
            }
            "--task-id" => {
                index += 1;
                template.task_id = Some(args.get(index).context("--task-id requires id")?.clone());
            }
            "--kind" => {
                index += 1;
                template.kind = Some(args.get(index).context("--kind requires value")?.clone());
            }
            "--target-id" => {
                index += 1;
                template.target_id =
                    Some(args.get(index).context("--target-id requires id")?.clone());
            }
            value if value.starts_with("--output-root=") => {
                template.output_root = Some(
                    value
                        .strip_prefix("--output-root=")
                        .unwrap_or_default()
                        .to_string(),
                );
            }
            value if value.starts_with("--source=") => {
                template.source = Some(
                    value
                        .strip_prefix("--source=")
                        .unwrap_or_default()
                        .to_string(),
                );
            }
            value if value.starts_with("--task-id=") => {
                template.task_id = Some(
                    value
                        .strip_prefix("--task-id=")
                        .unwrap_or_default()
                        .to_string(),
                );
            }
            value if value.starts_with("--kind=") => {
                template.kind = Some(
                    value
                        .strip_prefix("--kind=")
                        .unwrap_or_default()
                        .to_string(),
                );
            }
            value if value.starts_with("--target-id=") => {
                template.target_id = Some(
                    value
                        .strip_prefix("--target-id=")
                        .unwrap_or_default()
                        .to_string(),
                );
            }
            value if value.starts_with('-') => {
                bail!("unknown production-evidence-item-template option: {value}");
            }
            value => positionals.push(value.to_string()),
        }
        index += 1;
    }

    if template.task_id.is_some() {
        if positionals.len() > 1 {
            bail!(
                "production-evidence-item-template with --task-id accepts at most one output path"
            );
        }
        template.path = positionals.into_iter().next();
        return Ok(template);
    }

    match positionals.len() {
        0 => {}
        1 => {
            if positionals[0].contains(':') {
                template.task_id = Some(positionals[0].clone());
            } else if template.kind.is_none() {
                template.kind = Some(positionals[0].clone());
            } else if template.target_id.is_none() {
                template.target_id = Some(positionals[0].clone());
            } else {
                template.path = Some(positionals[0].clone());
            }
        }
        2 => {
            if positionals[0].contains(':') && template.kind.is_none() && template.target_id.is_none() {
                template.task_id = Some(positionals[0].clone());
                template.path = Some(positionals[1].clone());
            } else if template.kind.is_none() {
                template.kind = Some(positionals[0].clone());
            } else if template.target_id.is_none() {
                template.target_id = Some(positionals[0].clone());
            } else {
                template.path = Some(positionals[0].clone());
            }
            if template.target_id.is_none() {
                template.target_id = Some(positionals[1].clone());
            } else if template.path.is_none() {
                template.path = Some(positionals[1].clone());
            } else {
                bail!("production-evidence-item-template received too many positional arguments");
            }
        }
        3 => {
            if template.kind.is_none() {
                template.kind = Some(positionals[0].clone());
            }
            if template.target_id.is_none() {
                template.target_id = Some(positionals[1].clone());
            }
            template.path = Some(positionals[2].clone());
        }
        _ => bail!("production-evidence-item-template accepts kind target-id [item.json] or --task-id <id> [item.json]"),
    }

    if template.task_id.is_none() && (template.kind.is_none() || template.target_id.is_none()) {
        bail!("production-evidence-item-template requires --task-id or <kind> <target-id>");
    }

    Ok(template)
}

fn parse_production_evidence_item_from_ledger(
    args: Vec<String>,
) -> Result<ProductionEvidenceItemFromLedgerArgs> {
    let mut parsed = ProductionEvidenceItemFromLedgerArgs {
        path: None,
        source: None,
        provider_request_id: None,
        software_action_id: None,
        desktop_vision_action_id: None,
    };
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--provider-request-id" => {
                index += 1;
                parsed.provider_request_id = Some(
                    args.get(index)
                        .context("--provider-request-id requires id")?
                        .clone(),
                );
            }
            "--software-action-id" => {
                index += 1;
                parsed.software_action_id = Some(
                    args.get(index)
                        .context("--software-action-id requires id")?
                        .clone(),
                );
            }
            "--desktop-vision-action-id" => {
                index += 1;
                parsed.desktop_vision_action_id = Some(
                    args.get(index)
                        .context("--desktop-vision-action-id requires id")?
                        .clone(),
                );
            }
            "--source" => {
                index += 1;
                parsed.source = Some(args.get(index).context("--source requires value")?.clone());
            }
            value if value.starts_with("--provider-request-id=") => {
                parsed.provider_request_id = Some(
                    value
                        .strip_prefix("--provider-request-id=")
                        .unwrap_or_default()
                        .to_string(),
                );
            }
            value if value.starts_with("--software-action-id=") => {
                parsed.software_action_id = Some(
                    value
                        .strip_prefix("--software-action-id=")
                        .unwrap_or_default()
                        .to_string(),
                );
            }
            value if value.starts_with("--desktop-vision-action-id=") => {
                parsed.desktop_vision_action_id = Some(
                    value
                        .strip_prefix("--desktop-vision-action-id=")
                        .unwrap_or_default()
                        .to_string(),
                );
            }
            value if value.starts_with("--source=") => {
                parsed.source = Some(
                    value
                        .strip_prefix("--source=")
                        .unwrap_or_default()
                        .to_string(),
                );
            }
            value if value.starts_with('-') => {
                bail!("unknown production-evidence-item-from-ledger option: {value}");
            }
            value => {
                if parsed.path.is_some() {
                    bail!("production-evidence-item-from-ledger accepts at most one output path");
                }
                parsed.path = Some(value.to_string());
            }
        }
        index += 1;
    }

    let selected_ids = [
        parsed.provider_request_id.is_some(),
        parsed.software_action_id.is_some(),
        parsed.desktop_vision_action_id.is_some(),
    ]
    .into_iter()
    .filter(|selected| *selected)
    .count();
    if selected_ids != 1 {
        bail!("production-evidence-item-from-ledger requires exactly one of --provider-request-id, --software-action-id, or --desktop-vision-action-id");
    }

    Ok(parsed)
}

fn parse_production_evidence_bundle_from_ledger(
    args: Vec<String>,
) -> Result<ProductionEvidenceBundleFromLedgerArgs> {
    let mut parsed = ProductionEvidenceBundleFromLedgerArgs {
        path: None,
        source: None,
        include_incomplete: false,
    };
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--source" => {
                index += 1;
                parsed.source = Some(args.get(index).context("--source requires value")?.clone());
            }
            "--include-incomplete" | "--include_incomplete" => {
                parsed.include_incomplete = true;
            }
            value if value.starts_with("--source=") => {
                parsed.source = Some(
                    value
                        .strip_prefix("--source=")
                        .unwrap_or_default()
                        .to_string(),
                );
            }
            value if value.starts_with("--include-incomplete=") => {
                parsed.include_incomplete = cli_query_bool(
                    value
                        .strip_prefix("--include-incomplete=")
                        .unwrap_or_default(),
                );
            }
            value if value.starts_with("--include_incomplete=") => {
                parsed.include_incomplete = cli_query_bool(
                    value
                        .strip_prefix("--include_incomplete=")
                        .unwrap_or_default(),
                );
            }
            value if value.starts_with('-') => {
                bail!("unknown production-evidence-bundle-from-ledger option: {value}");
            }
            value => {
                if parsed.path.is_some() {
                    bail!("production-evidence-bundle-from-ledger accepts at most one output path");
                }
                parsed.path = Some(value.to_string());
            }
        }
        index += 1;
    }

    Ok(parsed)
}

fn parse_merge_production_evidence(args: Vec<String>) -> Result<ProductionEvidenceMergeArgs> {
    let mut source = None;
    let mut positional = Vec::new();
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--source" => {
                index += 1;
                source = Some(args.get(index).context("--source requires value")?.clone());
            }
            value if value.starts_with("--source=") => {
                source = Some(
                    value
                        .strip_prefix("--source=")
                        .unwrap_or_default()
                        .to_string(),
                );
            }
            value if value.starts_with('-') => {
                bail!("unknown merge-production-evidence option: {value}");
            }
            value => positional.push(value.to_string()),
        }
        index += 1;
    }

    if positional.len() < 2 {
        bail!("merge-production-evidence requires <combined-bundle.json> <input-bundle.json>...");
    }

    Ok(ProductionEvidenceMergeArgs {
        output_path: positional[0].clone(),
        input_paths: positional[1..].to_vec(),
        source,
    })
}

fn parse_closeout_production_evidence(args: Vec<String>) -> Result<ProductionEvidenceCloseoutArgs> {
    let mut source = None;
    let mut import = false;
    let mut output_path = None;
    let mut completion_package = false;
    let mut completion_package_output_dir = None;
    let mut completion_package_node_id = None;
    let mut completion_package_title = None;
    let mut completion_package_source = None;
    let mut completion_package_include_snapshot = true;
    let mut input_paths = Vec::new();
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--source" => {
                index += 1;
                source = Some(args.get(index).context("--source requires value")?.clone());
            }
            value if value.starts_with("--source=") => {
                source = Some(
                    value
                        .strip_prefix("--source=")
                        .unwrap_or_default()
                        .to_string(),
                );
            }
            "--import" => import = true,
            "--completion-package" => completion_package = true,
            "--completion-package-output-dir" => {
                index += 1;
                completion_package = true;
                completion_package_output_dir = Some(
                    args.get(index)
                        .context("--completion-package-output-dir requires path")?
                        .clone(),
                );
            }
            value if value.starts_with("--completion-package-output-dir=") => {
                completion_package = true;
                completion_package_output_dir = Some(
                    value
                        .strip_prefix("--completion-package-output-dir=")
                        .unwrap_or_default()
                        .to_string(),
                );
            }
            "--completion-package-node-id" => {
                index += 1;
                completion_package = true;
                completion_package_node_id = Some(
                    args.get(index)
                        .context("--completion-package-node-id requires id")?
                        .clone(),
                );
            }
            "--completion-package-title" => {
                index += 1;
                completion_package = true;
                completion_package_title = Some(
                    args.get(index)
                        .context("--completion-package-title requires text")?
                        .clone(),
                );
            }
            "--completion-package-source" => {
                index += 1;
                completion_package = true;
                completion_package_source = Some(
                    args.get(index)
                        .context("--completion-package-source requires value")?
                        .clone(),
                );
            }
            "--no-completion-package-snapshot" => {
                completion_package = true;
                completion_package_include_snapshot = false;
            }
            "--output" | "-o" => {
                index += 1;
                output_path = Some(args.get(index).context("--output requires value")?.clone());
            }
            value if value.starts_with("--output=") => {
                output_path = Some(
                    value
                        .strip_prefix("--output=")
                        .unwrap_or_default()
                        .to_string(),
                );
            }
            value if value.starts_with('-') => {
                bail!("unknown closeout-production-evidence option: {value}");
            }
            value => input_paths.push(value.to_string()),
        }
        index += 1;
    }

    if input_paths.is_empty() {
        bail!("closeout-production-evidence requires <bundle.json>...");
    }

    Ok(ProductionEvidenceCloseoutArgs {
        input_paths,
        source,
        import,
        output_path,
        completion_package,
        completion_package_output_dir,
        completion_package_node_id,
        completion_package_title,
        completion_package_source,
        completion_package_include_snapshot,
    })
}

fn parse_task_action(kind: TaskActionKind, args: Vec<String>) -> Result<TaskAction> {
    let task_id = args
        .first()
        .with_context(|| format!("{} requires <task-id>", task_action_name(&kind)))?
        .clone();
    if args.len() > 1 {
        bail!(
            "{} accepts only <task-id>; got extra argument: {}",
            task_action_name(&kind),
            args[1]
        );
    }
    Ok(TaskAction { kind, task_id })
}

fn parse_provider_health(args: Vec<String>) -> Result<ProviderHealthArgs> {
    let provider_id = args
        .first()
        .context("provider-health requires <provider-id>")?
        .clone();
    let mut run = ProviderHealthArgs {
        provider_id,
        execution_mode: None,
        endpoint: None,
        api_key: None,
    };
    let mut api_key_env = None;
    let mut index = 1;

    while index < args.len() {
        match args[index].as_str() {
            "--execution-mode" => {
                index += 1;
                run.execution_mode = Some(
                    args.get(index)
                        .context("--execution-mode requires auto|mock|adapter|gateway")?
                        .clone(),
                );
            }
            "--endpoint" => {
                index += 1;
                run.endpoint = Some(args.get(index).context("--endpoint requires url")?.clone());
            }
            "--api-key" => {
                index += 1;
                run.api_key = Some(args.get(index).context("--api-key requires value")?.clone());
            }
            "--api-key-env" => {
                index += 1;
                api_key_env = Some(
                    args.get(index)
                        .context("--api-key-env requires environment variable name")?
                        .clone(),
                );
            }
            value => bail!("unknown provider-health option: {value}"),
        }
        index += 1;
    }
    run.api_key = resolve_api_key(run.api_key, api_key_env)?;
    Ok(run)
}

fn parse_provider_run(args: Vec<String>) -> Result<ProviderRunArgs> {
    let provider_id = args
        .first()
        .context("run-provider requires <provider-id>")?
        .clone();
    let mut run = ProviderRunArgs {
        provider_id,
        node_id: None,
        task_title: None,
        execution_mode: None,
        endpoint: None,
        api_key: None,
        prompt: None,
        input_paths: Vec::new(),
        output_dir: None,
        cost_estimate_tokens: None,
        requires_approval: None,
        evidence_json: None,
    };
    let mut api_key_env = None;
    let mut index = 1;

    while index < args.len() {
        match args[index].as_str() {
            "--node-id" => {
                index += 1;
                run.node_id = Some(args.get(index).context("--node-id requires id")?.clone());
            }
            "--title" | "--task-title" => {
                index += 1;
                run.task_title = Some(args.get(index).context("--title requires text")?.clone());
            }
            "--execution-mode" => {
                index += 1;
                run.execution_mode = Some(
                    args.get(index)
                        .context("--execution-mode requires auto|mock|adapter|gateway")?
                        .clone(),
                );
            }
            "--endpoint" => {
                index += 1;
                run.endpoint = Some(args.get(index).context("--endpoint requires url")?.clone());
            }
            "--api-key" => {
                index += 1;
                run.api_key = Some(args.get(index).context("--api-key requires value")?.clone());
            }
            "--api-key-env" => {
                index += 1;
                api_key_env = Some(
                    args.get(index)
                        .context("--api-key-env requires environment variable name")?
                        .clone(),
                );
            }
            "--prompt" => {
                index += 1;
                run.prompt = Some(args.get(index).context("--prompt requires text")?.clone());
            }
            "--input" => {
                index += 1;
                run.input_paths
                    .push(args.get(index).context("--input requires path")?.clone());
            }
            "--output-dir" => {
                index += 1;
                run.output_dir = Some(
                    args.get(index)
                        .context("--output-dir requires path")?
                        .clone(),
                );
            }
            "--cost-estimate-tokens" => {
                index += 1;
                let value = args
                    .get(index)
                    .context("--cost-estimate-tokens requires integer")?;
                run.cost_estimate_tokens = Some(
                    value
                        .parse()
                        .context("--cost-estimate-tokens must be an integer")?,
                );
            }
            "--requires-approval" => {
                run.requires_approval = Some(true);
            }
            "--no-approval" => {
                run.requires_approval = Some(false);
            }
            "--evidence-json" => {
                index += 1;
                let value = args.get(index).context("--evidence-json requires JSON")?;
                let parsed: Value =
                    serde_json::from_str(value).context("--evidence-json must be valid JSON")?;
                let Some(mut evidence) = parsed.as_object().cloned() else {
                    bail!("--evidence-json must be a JSON object");
                };
                if let Some(Value::Object(existing)) = run.evidence_json.take() {
                    for (key, value) in existing {
                        evidence.entry(key).or_insert(value);
                    }
                }
                run.evidence_json = Some(Value::Object(evidence));
            }
            "--production-upstream" => {
                let mut evidence = match run.evidence_json.take() {
                    Some(Value::Object(object)) => object,
                    Some(_) => {
                        bail!("--production-upstream requires --evidence-json to be a JSON object")
                    }
                    None => Map::new(),
                };
                evidence
                    .entry("source".to_string())
                    .or_insert_with(|| json!("pool-cli"));
                evidence.insert("production_upstream".to_string(), json!(true));
                run.evidence_json = Some(Value::Object(evidence));
            }
            value => bail!("unknown run-provider option: {value}"),
        }
        index += 1;
    }
    run.api_key = resolve_api_key(run.api_key, api_key_env)?;
    Ok(run)
}

fn parse_provider_evidence_provider_matrix(
    args: Vec<String>,
) -> Result<ProviderEvidenceProviderMatrixArgs> {
    let mut parsed = ProviderEvidenceProviderMatrixArgs {
        output_root: None,
        media_endpoint: None,
        provider_endpoints: Vec::new(),
        provider_api_keys: Vec::new(),
        provider_attestations: Vec::new(),
        openai_endpoint: None,
        openai_api_key: None,
        three_dgs_endpoint: None,
        evidence_bundle_path: None,
        production_upstream: false,
        production_attestation: None,
        use_env: true,
    };
    let mut openai_api_key_env = None;
    let mut index = 0;

    while index < args.len() {
        let arg = args[index].as_str();
        if let Some(value) = arg.strip_prefix("--media-endpoint=") {
            parsed.media_endpoint = Some(value.to_string());
        } else if let Some(value) = arg.strip_prefix("--provider-endpoint=") {
            parsed
                .provider_endpoints
                .push(parse_provider_pair(value, "--provider-endpoint")?);
        } else if let Some(value) = arg.strip_prefix("--provider-endpoint-env=") {
            let (provider_id, env_name) = parse_provider_pair(value, "--provider-endpoint-env")?;
            if let Ok(endpoint) = env::var(&env_name) {
                parsed.provider_endpoints.push((provider_id, endpoint));
            }
        } else if let Some(value) = arg.strip_prefix("--provider-api-key=") {
            parsed
                .provider_api_keys
                .push(parse_provider_pair(value, "--provider-api-key")?);
        } else if let Some(value) = arg.strip_prefix("--provider-api-key-env=") {
            let (provider_id, env_name) = parse_provider_pair(value, "--provider-api-key-env")?;
            if let Ok(api_key) = env::var(&env_name) {
                parsed.provider_api_keys.push((provider_id, api_key));
            }
        } else if let Some(value) = arg.strip_prefix("--provider-attestation=") {
            parsed
                .provider_attestations
                .push(parse_provider_pair(value, "--provider-attestation")?);
        } else if let Some(value) = arg.strip_prefix("--provider-attestation-env=") {
            let (provider_id, env_name) = parse_provider_pair(value, "--provider-attestation-env")?;
            if let Ok(attestation) = env::var(&env_name) {
                parsed
                    .provider_attestations
                    .push((provider_id, attestation));
            }
        } else if let Some(value) = arg.strip_prefix("--3dgs-endpoint=") {
            parsed.three_dgs_endpoint = Some(value.to_string());
        } else if let Some(value) = arg.strip_prefix("--openai-endpoint=") {
            parsed.openai_endpoint = Some(value.to_string());
        } else if let Some(value) = arg.strip_prefix("--openai-api-key=") {
            parsed.openai_api_key = Some(value.to_string());
        } else if let Some(value) = arg.strip_prefix("--openai-api-key-env=") {
            openai_api_key_env = Some(value.to_string());
        } else if let Some(value) = arg.strip_prefix("--endpoint=") {
            parsed.media_endpoint = Some(value.to_string());
            parsed.three_dgs_endpoint = Some(value.to_string());
        } else if let Some(value) = arg.strip_prefix("--production-attestation=") {
            parsed.production_attestation = Some(value.to_string());
        } else if let Some(value) = arg.strip_prefix("--evidence-bundle=") {
            parsed.evidence_bundle_path = Some(value.to_string());
        } else {
            match arg {
                "--media-endpoint" => {
                    index += 1;
                    parsed.media_endpoint = Some(
                        args.get(index)
                            .context("--media-endpoint requires url")?
                            .clone(),
                    );
                }
                "--provider-endpoint" => {
                    index += 1;
                    parsed.provider_endpoints.push(parse_provider_pair(
                        args.get(index)
                            .context("--provider-endpoint requires provider=url")?,
                        "--provider-endpoint",
                    )?);
                }
                "--provider-endpoint-env" => {
                    index += 1;
                    let (provider_id, env_name) = parse_provider_pair(
                        args.get(index)
                            .context("--provider-endpoint-env requires provider=ENV_NAME")?,
                        "--provider-endpoint-env",
                    )?;
                    if let Ok(endpoint) = env::var(&env_name) {
                        parsed.provider_endpoints.push((provider_id, endpoint));
                    }
                }
                "--provider-api-key" => {
                    index += 1;
                    parsed.provider_api_keys.push(parse_provider_pair(
                        args.get(index)
                            .context("--provider-api-key requires provider=key")?,
                        "--provider-api-key",
                    )?);
                }
                "--provider-api-key-env" => {
                    index += 1;
                    let (provider_id, env_name) = parse_provider_pair(
                        args.get(index)
                            .context("--provider-api-key-env requires provider=ENV_NAME")?,
                        "--provider-api-key-env",
                    )?;
                    if let Ok(api_key) = env::var(&env_name) {
                        parsed.provider_api_keys.push((provider_id, api_key));
                    }
                }
                "--provider-attestation" => {
                    index += 1;
                    parsed.provider_attestations.push(parse_provider_pair(
                        args.get(index)
                            .context("--provider-attestation requires provider=attestation")?,
                        "--provider-attestation",
                    )?);
                }
                "--provider-attestation-env" => {
                    index += 1;
                    let (provider_id, env_name) = parse_provider_pair(
                        args.get(index)
                            .context("--provider-attestation-env requires provider=ENV_NAME")?,
                        "--provider-attestation-env",
                    )?;
                    if let Ok(attestation) = env::var(&env_name) {
                        parsed
                            .provider_attestations
                            .push((provider_id, attestation));
                    }
                }
                "--3dgs-endpoint" => {
                    index += 1;
                    parsed.three_dgs_endpoint = Some(
                        args.get(index)
                            .context("--3dgs-endpoint requires url")?
                            .clone(),
                    );
                }
                "--openai-endpoint" => {
                    index += 1;
                    parsed.openai_endpoint = Some(
                        args.get(index)
                            .context("--openai-endpoint requires url")?
                            .clone(),
                    );
                }
                "--openai-api-key" => {
                    index += 1;
                    parsed.openai_api_key = Some(
                        args.get(index)
                            .context("--openai-api-key requires value")?
                            .clone(),
                    );
                }
                "--openai-api-key-env" => {
                    index += 1;
                    openai_api_key_env = Some(
                        args.get(index)
                            .context("--openai-api-key-env requires environment variable name")?
                            .clone(),
                    );
                }
                "--endpoint" => {
                    index += 1;
                    let endpoint = args.get(index).context("--endpoint requires url")?.clone();
                    parsed.media_endpoint = Some(endpoint.clone());
                    parsed.three_dgs_endpoint = Some(endpoint);
                }
                "--production-upstream" => parsed.production_upstream = true,
                "--production-attestation" => {
                    index += 1;
                    parsed.production_attestation = Some(
                        args.get(index)
                            .context("--production-attestation requires value")?
                            .clone(),
                    );
                }
                "--evidence-bundle" => {
                    index += 1;
                    parsed.evidence_bundle_path = Some(
                        args.get(index)
                            .context("--evidence-bundle requires path")?
                            .clone(),
                    );
                }
                "--no-env" => parsed.use_env = false,
                value if !value.starts_with("--") && parsed.output_root.is_none() => {
                    parsed.output_root = Some(value.to_string());
                }
                value if !value.starts_with("--") => {
                    bail!("production-evidence-provider-matrix accepts at most one output root; got extra argument: {value}");
                }
                value => bail!("unknown production-evidence-provider-matrix option: {value}"),
            }
        }
        index += 1;
    }

    parsed.openai_api_key = resolve_api_key(parsed.openai_api_key, openai_api_key_env)?;
    Ok(parsed)
}

fn parse_software_evidence_matrix(args: Vec<String>) -> Result<SoftwareEvidenceMatrixArgs> {
    let mut parsed = SoftwareEvidenceMatrixArgs {
        output_root: None,
        software_endpoints: Vec::new(),
        software_commands: Vec::new(),
        software_artifacts: Vec::new(),
        software_attestations: Vec::new(),
        evidence_bundle_path: None,
        production_software: false,
        use_env: true,
    };
    let mut index = 0;

    while index < args.len() {
        let arg = args[index].as_str();
        if let Some(value) = arg.strip_prefix("--evidence-bundle=") {
            parsed.evidence_bundle_path = Some(value.to_string());
        } else if let Some(value) = arg.strip_prefix("--software-endpoint=") {
            parsed
                .software_endpoints
                .push(parse_provider_pair(value, "--software-endpoint")?);
        } else if let Some(value) = arg.strip_prefix("--software-endpoint-env=") {
            let (adapter_id, env_name) = parse_provider_pair(value, "--software-endpoint-env")?;
            if let Ok(endpoint) = env::var(&env_name) {
                parsed.software_endpoints.push((adapter_id, endpoint));
            }
        } else if let Some(value) = arg.strip_prefix("--software-command=") {
            parsed
                .software_commands
                .push(parse_provider_pair(value, "--software-command")?);
        } else if let Some(value) = arg.strip_prefix("--software-command-env=") {
            let (adapter_id, env_name) = parse_provider_pair(value, "--software-command-env")?;
            if let Ok(command) = env::var(&env_name) {
                parsed.software_commands.push((adapter_id, command));
            }
        } else if let Some(value) = arg.strip_prefix("--software-artifact=") {
            parsed
                .software_artifacts
                .push(parse_provider_pair(value, "--software-artifact")?);
        } else if let Some(value) = arg.strip_prefix("--software-artifacts-env=") {
            let (adapter_id, env_name) = parse_provider_pair(value, "--software-artifacts-env")?;
            if let Ok(artifacts) = env::var(&env_name) {
                parsed.software_artifacts.push((adapter_id, artifacts));
            }
        } else if let Some(value) = arg.strip_prefix("--software-attestation=") {
            parsed
                .software_attestations
                .push(parse_provider_pair(value, "--software-attestation")?);
        } else if let Some(value) = arg.strip_prefix("--software-attestation-env=") {
            let (adapter_id, env_name) = parse_provider_pair(value, "--software-attestation-env")?;
            if let Ok(attestation) = env::var(&env_name) {
                parsed.software_attestations.push((adapter_id, attestation));
            }
        } else {
            match arg {
                "--production-software" => parsed.production_software = true,
                "--software-endpoint" => {
                    index += 1;
                    parsed.software_endpoints.push(parse_provider_pair(
                        args.get(index)
                            .context("--software-endpoint requires adapter=url")?,
                        "--software-endpoint",
                    )?);
                }
                "--software-endpoint-env" => {
                    index += 1;
                    let (adapter_id, env_name) = parse_provider_pair(
                        args.get(index)
                            .context("--software-endpoint-env requires adapter=ENV_NAME")?,
                        "--software-endpoint-env",
                    )?;
                    if let Ok(endpoint) = env::var(&env_name) {
                        parsed.software_endpoints.push((adapter_id, endpoint));
                    }
                }
                "--software-command" => {
                    index += 1;
                    parsed.software_commands.push(parse_provider_pair(
                        args.get(index)
                            .context("--software-command requires adapter=command")?,
                        "--software-command",
                    )?);
                }
                "--software-command-env" => {
                    index += 1;
                    let (adapter_id, env_name) = parse_provider_pair(
                        args.get(index)
                            .context("--software-command-env requires adapter=ENV_NAME")?,
                        "--software-command-env",
                    )?;
                    if let Ok(command) = env::var(&env_name) {
                        parsed.software_commands.push((adapter_id, command));
                    }
                }
                "--software-artifact" => {
                    index += 1;
                    parsed.software_artifacts.push(parse_provider_pair(
                        args.get(index)
                            .context("--software-artifact requires adapter=path")?,
                        "--software-artifact",
                    )?);
                }
                "--software-artifacts-env" => {
                    index += 1;
                    let (adapter_id, env_name) = parse_provider_pair(
                        args.get(index)
                            .context("--software-artifacts-env requires adapter=ENV_NAME")?,
                        "--software-artifacts-env",
                    )?;
                    if let Ok(artifacts) = env::var(&env_name) {
                        parsed.software_artifacts.push((adapter_id, artifacts));
                    }
                }
                "--software-attestation" => {
                    index += 1;
                    parsed.software_attestations.push(parse_provider_pair(
                        args.get(index)
                            .context("--software-attestation requires adapter=id")?,
                        "--software-attestation",
                    )?);
                }
                "--software-attestation-env" => {
                    index += 1;
                    let (adapter_id, env_name) = parse_provider_pair(
                        args.get(index)
                            .context("--software-attestation-env requires adapter=ENV_NAME")?,
                        "--software-attestation-env",
                    )?;
                    if let Ok(attestation) = env::var(&env_name) {
                        parsed.software_attestations.push((adapter_id, attestation));
                    }
                }
                "--evidence-bundle" => {
                    index += 1;
                    parsed.evidence_bundle_path = Some(
                        args.get(index)
                            .context("--evidence-bundle requires path")?
                            .clone(),
                    );
                }
                "--no-env" => parsed.use_env = false,
                value if !value.starts_with("--") && parsed.output_root.is_none() => {
                    parsed.output_root = Some(value.to_string());
                }
                value if !value.starts_with("--") => {
                    bail!("production-evidence-software-matrix accepts at most one output root; got extra argument: {value}");
                }
                value => bail!("unknown production-evidence-software-matrix option: {value}"),
            }
        }
        index += 1;
    }

    Ok(parsed)
}

fn parse_desktop_vision_evidence(args: Vec<String>) -> Result<DesktopVisionEvidenceArgs> {
    let mut parsed = DesktopVisionEvidenceArgs {
        output_root: None,
        evidence_bundle_path: None,
        production_vision: false,
        trace_path: None,
        trace_env: None,
        controller_id: None,
        controller_id_env: None,
        external_action_id: None,
        external_action_id_env: None,
        production_attestation: None,
        production_attestation_env: None,
        limit: usize::MAX,
        use_env: true,
    };
    let mut index = 0;

    while index < args.len() {
        let arg = args[index].as_str();
        if let Some(value) = arg.strip_prefix("--evidence-bundle=") {
            parsed.evidence_bundle_path = Some(value.to_string());
        } else if let Some(value) = arg.strip_prefix("--trace=") {
            parsed.trace_path = Some(value.to_string());
        } else if let Some(value) = arg.strip_prefix("--trace-path=") {
            parsed.trace_path = Some(value.to_string());
        } else if let Some(value) = arg.strip_prefix("--trace-env=") {
            parsed.trace_env = Some(value.to_string());
        } else if let Some(value) = arg.strip_prefix("--controller-id=") {
            parsed.controller_id = Some(value.to_string());
        } else if let Some(value) = arg.strip_prefix("--controller-id-env=") {
            parsed.controller_id_env = Some(value.to_string());
        } else if let Some(value) = arg.strip_prefix("--external-action-id=") {
            parsed.external_action_id = Some(value.to_string());
        } else if let Some(value) = arg.strip_prefix("--external-action-id-env=") {
            parsed.external_action_id_env = Some(value.to_string());
        } else if let Some(value) = arg.strip_prefix("--production-attestation=") {
            parsed.production_attestation = Some(value.to_string());
        } else if let Some(value) = arg.strip_prefix("--production-attestation-env=") {
            parsed.production_attestation_env = Some(value.to_string());
        } else if let Some(value) = arg.strip_prefix("--limit=") {
            parsed.limit = value
                .parse()
                .with_context(|| format!("invalid --limit value: {value}"))?;
        } else {
            match arg {
                "--production-vision" => parsed.production_vision = true,
                "--evidence-bundle" => {
                    index += 1;
                    parsed.evidence_bundle_path = Some(
                        args.get(index)
                            .context("--evidence-bundle requires path")?
                            .clone(),
                    );
                }
                "--trace" | "--trace-path" => {
                    index += 1;
                    parsed.trace_path =
                        Some(args.get(index).context("--trace requires path")?.clone());
                }
                "--trace-env" => {
                    index += 1;
                    parsed.trace_env = Some(
                        args.get(index)
                            .context("--trace-env requires env name")?
                            .clone(),
                    );
                }
                "--controller-id" => {
                    index += 1;
                    parsed.controller_id = Some(
                        args.get(index)
                            .context("--controller-id requires value")?
                            .clone(),
                    );
                }
                "--controller-id-env" => {
                    index += 1;
                    parsed.controller_id_env = Some(
                        args.get(index)
                            .context("--controller-id-env requires env name")?
                            .clone(),
                    );
                }
                "--external-action-id" => {
                    index += 1;
                    parsed.external_action_id = Some(
                        args.get(index)
                            .context("--external-action-id requires value")?
                            .clone(),
                    );
                }
                "--external-action-id-env" => {
                    index += 1;
                    parsed.external_action_id_env = Some(
                        args.get(index)
                            .context("--external-action-id-env requires env name")?
                            .clone(),
                    );
                }
                "--production-attestation" => {
                    index += 1;
                    parsed.production_attestation = Some(
                        args.get(index)
                            .context("--production-attestation requires value")?
                            .clone(),
                    );
                }
                "--production-attestation-env" => {
                    index += 1;
                    parsed.production_attestation_env = Some(
                        args.get(index)
                            .context("--production-attestation-env requires env name")?
                            .clone(),
                    );
                }
                "--limit" => {
                    index += 1;
                    let value = args.get(index).context("--limit requires count")?;
                    parsed.limit = value
                        .parse()
                        .with_context(|| format!("invalid --limit value: {value}"))?;
                }
                "--no-env" => parsed.use_env = false,
                value if !value.starts_with("--") && parsed.output_root.is_none() => {
                    parsed.output_root = Some(value.to_string());
                }
                value if !value.starts_with("--") => {
                    bail!("production-evidence-desktop-vision accepts at most one output root; got extra argument: {value}");
                }
                value => bail!("unknown production-evidence-desktop-vision option: {value}"),
            }
        }
        index += 1;
    }

    Ok(parsed)
}

fn parse_adapter_health(args: Vec<String>) -> Result<AdapterHealthArgs> {
    let mut run = AdapterHealthArgs {
        include_providers: None,
        include_software: None,
    };
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--providers-only" => {
                run.include_providers = Some(true);
                run.include_software = Some(false);
            }
            "--software-only" => {
                run.include_providers = Some(false);
                run.include_software = Some(true);
            }
            "--no-providers" => run.include_providers = Some(false),
            "--no-software" => run.include_software = Some(false),
            value => bail!("unknown adapter-health option: {value}"),
        }
        index += 1;
    }

    Ok(run)
}

fn parse_software_health(args: Vec<String>) -> Result<SoftwareHealthArgs> {
    let adapter_id = args
        .first()
        .context("software-health requires <adapter-id>")?
        .clone();
    let mut run = SoftwareHealthArgs {
        adapter_id,
        priority: None,
        payload_json: json!({}),
    };
    let mut index = 1;

    while index < args.len() {
        match args[index].as_str() {
            "--priority" => {
                index += 1;
                let value = args.get(index).context("--priority requires value")?;
                run.priority = Some(normalize_control_priority(value)?);
            }
            "--endpoint" => {
                index += 1;
                let value = args.get(index).context("--endpoint requires url")?;
                insert_payload_string(&mut run.payload_json, "endpoint", value)?;
            }
            "--payload" => {
                index += 1;
                let value = args.get(index).context("--payload requires key=value")?;
                insert_payload_pair(&mut run.payload_json, value)?;
            }
            "--payload-json" => {
                index += 1;
                let value = args
                    .get(index)
                    .context("--payload-json requires JSON object")?;
                merge_payload_json(&mut run.payload_json, value)?;
            }
            value => bail!("unknown software-health option: {value}"),
        }
        index += 1;
    }

    Ok(run)
}

fn parse_software_action(args: Vec<String>) -> Result<SoftwareActionArgs> {
    let adapter_id = args
        .first()
        .context("run-software requires <adapter-id>")?
        .clone();
    let mut run = SoftwareActionArgs {
        adapter_id,
        node_id: None,
        task_title: None,
        action_kind: None,
        priority: None,
        payload_json: json!({}),
        evidence_json: None,
        requires_confirmation: None,
    };
    let mut index = 1;

    while index < args.len() {
        match args[index].as_str() {
            "--node-id" => {
                index += 1;
                run.node_id = Some(args.get(index).context("--node-id requires id")?.clone());
            }
            "--title" | "--task-title" => {
                index += 1;
                run.task_title = Some(args.get(index).context("--title requires text")?.clone());
            }
            "--action-kind" | "--action" => {
                index += 1;
                let value = args.get(index).context("--action-kind requires value")?;
                run.action_kind = Some(normalize_software_action_kind(value)?);
            }
            "--priority" => {
                index += 1;
                let value = args.get(index).context("--priority requires value")?;
                run.priority = Some(normalize_control_priority(value)?);
            }
            "--endpoint" => {
                index += 1;
                let value = args.get(index).context("--endpoint requires url")?;
                insert_payload_string(&mut run.payload_json, "endpoint", value)?;
            }
            "--payload" => {
                index += 1;
                let value = args.get(index).context("--payload requires key=value")?;
                insert_payload_pair(&mut run.payload_json, value)?;
            }
            "--payload-json" => {
                index += 1;
                let value = args
                    .get(index)
                    .context("--payload-json requires JSON object")?;
                merge_payload_json(&mut run.payload_json, value)?;
            }
            "--evidence-json" => {
                index += 1;
                let value = args.get(index).context("--evidence-json requires JSON")?;
                let parsed = parse_json_value(value, "--evidence-json")?;
                let Some(mut evidence) = parsed.as_object().cloned() else {
                    bail!("--evidence-json must be a JSON object");
                };
                if let Some(Value::Object(existing)) = run.evidence_json.take() {
                    for (key, value) in existing {
                        evidence.entry(key).or_insert(value);
                    }
                }
                run.evidence_json = Some(Value::Object(evidence));
            }
            "--production-software" => {
                let mut evidence = match run.evidence_json.take() {
                    Some(Value::Object(object)) => object,
                    Some(_) => {
                        bail!("--production-software requires --evidence-json to be a JSON object")
                    }
                    None => Map::new(),
                };
                evidence
                    .entry("source".to_string())
                    .or_insert_with(|| json!("pool-cli"));
                evidence.insert("production_software".to_string(), json!(true));
                run.evidence_json = Some(Value::Object(evidence));
            }
            "--requires-confirmation" => run.requires_confirmation = Some(true),
            "--no-confirmation" => run.requires_confirmation = Some(false),
            value => bail!("unknown run-software option: {value}"),
        }
        index += 1;
    }

    Ok(run)
}

fn parse_set_api_key(args: Vec<String>) -> Result<SetApiKeyArgs> {
    let provider_id = args
        .first()
        .context("set-api-key requires <provider-id>")?
        .clone();
    let mut service_type = "provider".to_string();
    let mut api_key = None;
    let mut api_key_env = None;
    let mut metadata = Map::new();
    let mut index = 1;

    while index < args.len() {
        match args[index].as_str() {
            "--service-type" => {
                index += 1;
                service_type = args
                    .get(index)
                    .context("--service-type requires value")?
                    .clone();
            }
            "--api-key" => {
                index += 1;
                api_key = Some(args.get(index).context("--api-key requires value")?.clone());
            }
            "--api-key-env" => {
                index += 1;
                let env_name = args
                    .get(index)
                    .context("--api-key-env requires environment variable name")?
                    .clone();
                api_key_env = Some(env_name);
            }
            "--metadata" => {
                index += 1;
                let entry = args.get(index).context("--metadata requires key=value")?;
                let (key, value) = entry
                    .split_once('=')
                    .context("--metadata must use key=value")?;
                metadata.insert(key.to_string(), json!(value));
            }
            "--rotation-days" | "--rotation-interval-days" => {
                index += 1;
                let value = args
                    .get(index)
                    .context("--rotation-days requires integer")?;
                let days = value
                    .parse::<u64>()
                    .context("--rotation-days must be an integer")?;
                metadata.insert("rotation_days".to_string(), json!(days));
            }
            value => bail!("unknown set-api-key option: {value}"),
        }
        index += 1;
    }
    let api_key_env_name = api_key_env.clone();
    let api_key = resolve_required_api_key(api_key, api_key_env)?;
    if let Some(env_name) = api_key_env_name {
        metadata.insert("source".to_string(), json!("env"));
        metadata.insert("env".to_string(), json!(env_name));
    } else if !metadata.contains_key("source") {
        metadata.insert("source".to_string(), json!("cli"));
    }
    Ok(SetApiKeyArgs {
        provider_id,
        service_type,
        api_key,
        metadata: Value::Object(metadata),
    })
}

fn parse_run_node(args: Vec<String>) -> Result<RunNodeArgs> {
    let node_id = args.first().context("run-node requires <node-id>")?.clone();
    let mut run = RunNodeArgs {
        node_id,
        prompt: None,
        execution_mode: None,
        endpoint: None,
        api_key: None,
        input_paths: Vec::new(),
        output_dir: None,
        duration_ms: None,
    };
    let mut index = 1;

    while index < args.len() {
        match args[index].as_str() {
            "--prompt" => {
                index += 1;
                run.prompt = Some(args.get(index).context("--prompt requires text")?.clone());
            }
            "--execution-mode" => {
                index += 1;
                run.execution_mode = Some(
                    args.get(index)
                        .context("--execution-mode requires auto|mock|adapter|gateway")?
                        .clone(),
                );
            }
            "--endpoint" => {
                index += 1;
                run.endpoint = Some(args.get(index).context("--endpoint requires url")?.clone());
            }
            "--api-key" => {
                index += 1;
                run.api_key = Some(args.get(index).context("--api-key requires value")?.clone());
            }
            "--input" => {
                index += 1;
                run.input_paths
                    .push(args.get(index).context("--input requires path")?.clone());
            }
            "--output-dir" => {
                index += 1;
                run.output_dir = Some(
                    args.get(index)
                        .context("--output-dir requires path")?
                        .clone(),
                );
            }
            "--duration-ms" => {
                index += 1;
                let value = args.get(index).context("--duration-ms requires integer")?;
                run.duration_ms = Some(value.parse().context("--duration-ms must be an integer")?);
            }
            value => bail!("unknown run-node option: {value}"),
        }
        index += 1;
    }

    Ok(run)
}

fn parse_runtime_run_next(args: Vec<String>) -> Result<RuntimeRunNextArgs> {
    let mut run = RuntimeRunNextArgs::default();
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--node-id" => {
                index += 1;
                run.node_id = Some(args.get(index).context("--node-id requires id")?.clone());
            }
            "--task-id" => {
                index += 1;
                run.task_id = Some(args.get(index).context("--task-id requires id")?.clone());
            }
            "--execute" => run.execute = true,
            "--allow-approval" => run.allow_approval = true,
            "--prompt" => {
                index += 1;
                run.prompt = Some(args.get(index).context("--prompt requires text")?.clone());
            }
            "--execution-mode" => {
                index += 1;
                run.execution_mode = Some(
                    args.get(index)
                        .context("--execution-mode requires auto|mock|adapter|gateway")?
                        .clone(),
                );
            }
            "--endpoint" => {
                index += 1;
                run.endpoint = Some(args.get(index).context("--endpoint requires url")?.clone());
            }
            "--api-key" => {
                index += 1;
                run.api_key = Some(args.get(index).context("--api-key requires value")?.clone());
            }
            "--input" => {
                index += 1;
                run.input_paths
                    .push(args.get(index).context("--input requires path")?.clone());
            }
            "--output-dir" => {
                index += 1;
                run.output_dir = Some(
                    args.get(index)
                        .context("--output-dir requires path")?
                        .clone(),
                );
            }
            "--duration-ms" => {
                index += 1;
                let value = args.get(index).context("--duration-ms requires integer")?;
                run.duration_ms = Some(value.parse().context("--duration-ms must be an integer")?);
            }
            value => bail!("unknown runtime-run-next option: {value}"),
        }
        index += 1;
    }

    Ok(run)
}

fn parse_workflow_run(args: Vec<String>) -> Result<WorkflowRunArgs> {
    let mut run = WorkflowRunArgs {
        title: None,
        prompt: None,
        source_inputs: Vec::new(),
        output_root: None,
        duration_ms: None,
        agent_mode: None,
        hermes_endpoint: None,
        hermes_auth_token: None,
        agent_requires_confirmation: false,
        three_dgs_mode: None,
        three_dgs_provider_id: None,
        three_dgs_endpoint: None,
        three_dgs_api_key: None,
        unreal_mode: None,
        unreal_endpoint: None,
        unreal_auth_token: None,
    };
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--title" => {
                index += 1;
                run.title = Some(args.get(index).context("--title requires text")?.clone());
            }
            "--prompt" => {
                index += 1;
                run.prompt = Some(args.get(index).context("--prompt requires text")?.clone());
            }
            "--source-input" => {
                index += 1;
                run.source_inputs.push(
                    args.get(index)
                        .context("--source-input requires path")?
                        .clone(),
                );
            }
            "--output-root" => {
                index += 1;
                run.output_root = Some(
                    args.get(index)
                        .context("--output-root requires path")?
                        .clone(),
                );
            }
            "--duration-ms" => {
                index += 1;
                let value = args.get(index).context("--duration-ms requires integer")?;
                run.duration_ms = Some(value.parse().context("--duration-ms must be an integer")?);
            }
            "--agent-mode" => {
                index += 1;
                run.agent_mode = Some(
                    args.get(index)
                        .context("--agent-mode requires stage|skip|hermes_http")?
                        .clone(),
                );
            }
            "--hermes-endpoint" => {
                index += 1;
                run.hermes_endpoint = Some(
                    args.get(index)
                        .context("--hermes-endpoint requires url")?
                        .clone(),
                );
            }
            "--hermes-auth-token" => {
                index += 1;
                run.hermes_auth_token = Some(
                    args.get(index)
                        .context("--hermes-auth-token requires value")?
                        .clone(),
                );
            }
            "--agent-requires-confirmation" => {
                run.agent_requires_confirmation = true;
            }
            "--three-dgs-mode" => {
                index += 1;
                run.three_dgs_mode = Some(
                    args.get(index)
                        .context("--three-dgs-mode requires auto|mock|gateway")?
                        .clone(),
                );
            }
            "--three-dgs-provider-id" => {
                index += 1;
                run.three_dgs_provider_id = Some(
                    args.get(index)
                        .context("--three-dgs-provider-id requires provider id")?
                        .clone(),
                );
            }
            "--three-dgs-endpoint" => {
                index += 1;
                run.three_dgs_endpoint = Some(
                    args.get(index)
                        .context("--three-dgs-endpoint requires url")?
                        .clone(),
                );
            }
            "--three-dgs-api-key" => {
                index += 1;
                run.three_dgs_api_key = Some(
                    args.get(index)
                        .context("--three-dgs-api-key requires value")?
                        .clone(),
                );
            }
            "--unreal-mode" => {
                index += 1;
                run.unreal_mode = Some(
                    args.get(index)
                        .context("--unreal-mode requires auto|mock|unreal_mcp")?
                        .clone(),
                );
            }
            "--unreal-endpoint" => {
                index += 1;
                run.unreal_endpoint = Some(
                    args.get(index)
                        .context("--unreal-endpoint requires url")?
                        .clone(),
                );
            }
            "--unreal-auth-token" => {
                index += 1;
                run.unreal_auth_token = Some(
                    args.get(index)
                        .context("--unreal-auth-token requires value")?
                        .clone(),
                );
            }
            value => bail!("unknown run-workflow option: {value}"),
        }
        index += 1;
    }

    Ok(run)
}

fn parse_output_package(args: Vec<String>) -> Result<OutputPackageArgs> {
    let mut run = OutputPackageArgs {
        node_id: None,
        title: None,
        output_dir: None,
        source_assets: Vec::new(),
        duration_ms: None,
    };
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--node-id" => {
                index += 1;
                run.node_id = Some(args.get(index).context("--node-id requires id")?.clone());
            }
            "--title" => {
                index += 1;
                run.title = Some(args.get(index).context("--title requires text")?.clone());
            }
            "--output-dir" => {
                index += 1;
                run.output_dir = Some(
                    args.get(index)
                        .context("--output-dir requires path")?
                        .clone(),
                );
            }
            "--source-asset" => {
                index += 1;
                run.source_assets.push(
                    args.get(index)
                        .context("--source-asset requires path")?
                        .clone(),
                );
            }
            "--duration-ms" => {
                index += 1;
                let value = args.get(index).context("--duration-ms requires integer")?;
                run.duration_ms = Some(value.parse().context("--duration-ms must be an integer")?);
            }
            value => bail!("unknown output-package option: {value}"),
        }
        index += 1;
    }

    Ok(run)
}

fn parse_output_result(args: Vec<String>) -> Result<OutputResultArgs> {
    let target = args
        .first()
        .context("output-result requires <video|game|interactive_art>")?
        .clone();
    let mut run = OutputResultArgs {
        node_id: None,
        target,
        local_path: None,
        status: "succeeded".to_string(),
        runtime: None,
        adapter_id: None,
        software_action_id: None,
        message: None,
        artifacts: Vec::new(),
        metrics: Vec::new(),
        verification: None,
    };
    let mut index = 1;

    while index < args.len() {
        match args[index].as_str() {
            "--node-id" => {
                index += 1;
                run.node_id = Some(args.get(index).context("--node-id requires id")?.clone());
            }
            "--local-path" => {
                index += 1;
                run.local_path = Some(
                    args.get(index)
                        .context("--local-path requires path")?
                        .clone(),
                );
            }
            "--status" => {
                index += 1;
                run.status = args.get(index).context("--status requires value")?.clone();
            }
            "--runtime" => {
                index += 1;
                run.runtime = Some(args.get(index).context("--runtime requires value")?.clone());
            }
            "--adapter-id" => {
                index += 1;
                run.adapter_id = Some(args.get(index).context("--adapter-id requires id")?.clone());
            }
            "--software-action-id" => {
                index += 1;
                run.software_action_id = Some(
                    args.get(index)
                        .context("--software-action-id requires id")?
                        .clone(),
                );
            }
            "--message" => {
                index += 1;
                run.message = Some(args.get(index).context("--message requires text")?.clone());
            }
            "--artifact" => {
                index += 1;
                run.artifacts
                    .push(args.get(index).context("--artifact requires path")?.clone());
            }
            "--metric" => {
                index += 1;
                let value = args.get(index).context("--metric requires label=value")?;
                let Some((label, metric_value)) = value.split_once('=') else {
                    bail!("--metric requires label=value");
                };
                run.metrics
                    .push((label.to_string(), metric_value.to_string()));
            }
            "--verification-json" => {
                index += 1;
                let value = args
                    .get(index)
                    .context("--verification-json requires JSON object")?;
                run.verification = Some(
                    serde_json::from_str(value)
                        .context("--verification-json must be valid JSON")?,
                );
            }
            value => bail!("unknown output-result option: {value}"),
        }
        index += 1;
    }

    Ok(run)
}

fn parse_handoff_package(args: Vec<String>) -> Result<HandoffPackageArgs> {
    let mut run = HandoffPackageArgs {
        node_id: None,
        title: None,
        output_dir: None,
        include_snapshot: false,
    };
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--node-id" => {
                index += 1;
                run.node_id = Some(args.get(index).context("--node-id requires id")?.clone());
            }
            "--title" => {
                index += 1;
                run.title = Some(args.get(index).context("--title requires text")?.clone());
            }
            "--output-dir" => {
                index += 1;
                run.output_dir = Some(
                    args.get(index)
                        .context("--output-dir requires path")?
                        .clone(),
                );
            }
            "--include-snapshot" => run.include_snapshot = true,
            value => bail!("unknown handoff-package option: {value}"),
        }
        index += 1;
    }

    Ok(run)
}

fn parse_agent_conformance_package(args: Vec<String>) -> Result<AgentConformancePackageArgs> {
    let mut run = AgentConformancePackageArgs {
        kind: "all".to_string(),
        node_id: None,
        title: None,
        output_dir: None,
    };
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--kind" => {
                index += 1;
                run.kind = normalize_agent_conformance_kind(
                    args.get(index)
                        .context("--kind requires all|hermes|agent-cli")?,
                )?;
            }
            "--node-id" => {
                index += 1;
                run.node_id = Some(args.get(index).context("--node-id requires id")?.clone());
            }
            "--title" => {
                index += 1;
                run.title = Some(args.get(index).context("--title requires text")?.clone());
            }
            "--output-dir" => {
                index += 1;
                run.output_dir = Some(
                    args.get(index)
                        .context("--output-dir requires path")?
                        .clone(),
                );
            }
            value if !value.starts_with("--") && index == 0 => {
                run.kind = normalize_agent_conformance_kind(value)?;
            }
            value => bail!("unknown agent-conformance-package option: {value}"),
        }
        index += 1;
    }

    Ok(run)
}

fn parse_integration_conformance_package(
    args: Vec<String>,
) -> Result<IntegrationConformancePackageArgs> {
    let mut run = IntegrationConformancePackageArgs {
        node_id: None,
        title: None,
        output_dir: None,
        providers: Vec::new(),
        software_adapters: Vec::new(),
        agent_kind: Some("all".to_string()),
        include_providers: true,
        include_software: true,
        include_agent: true,
    };
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--provider" => {
                index += 1;
                run.providers.extend(parse_conformance_list(
                    args.get(index).context("--provider requires id")?,
                ));
            }
            "--providers" => {
                index += 1;
                run.providers.extend(parse_conformance_list(
                    args.get(index).context("--providers requires ids")?,
                ));
            }
            "--software" | "--adapter" => {
                index += 1;
                run.software_adapters.extend(parse_conformance_list(
                    args.get(index).context("--software requires id")?,
                ));
            }
            "--software-adapters" | "--adapters" => {
                index += 1;
                run.software_adapters.extend(parse_conformance_list(
                    args.get(index)
                        .context("--software-adapters requires ids")?,
                ));
            }
            "--agent-kind" | "--kind" => {
                index += 1;
                run.agent_kind = Some(normalize_agent_conformance_kind(
                    args.get(index)
                        .context("--agent-kind requires all|hermes|agent-cli")?,
                )?);
            }
            "--node-id" => {
                index += 1;
                run.node_id = Some(args.get(index).context("--node-id requires id")?.clone());
            }
            "--title" => {
                index += 1;
                run.title = Some(args.get(index).context("--title requires text")?.clone());
            }
            "--output-dir" => {
                index += 1;
                run.output_dir = Some(
                    args.get(index)
                        .context("--output-dir requires path")?
                        .clone(),
                );
            }
            "--no-providers" => run.include_providers = false,
            "--no-software" => run.include_software = false,
            "--no-agent" => {
                run.include_agent = false;
                run.agent_kind = None;
            }
            value => bail!("unknown integration-conformance-package option: {value}"),
        }
        index += 1;
    }

    Ok(run)
}

fn parse_software_conformance_package(args: Vec<String>) -> Result<SoftwareConformancePackageArgs> {
    let adapter_id = args
        .first()
        .context("software-conformance-package requires <adapter-id>")?
        .clone();
    let mut run = SoftwareConformancePackageArgs {
        adapter_id,
        node_id: None,
        title: None,
        output_dir: None,
    };
    let mut index = 1;

    while index < args.len() {
        match args[index].as_str() {
            "--node-id" => {
                index += 1;
                run.node_id = Some(args.get(index).context("--node-id requires id")?.clone());
            }
            "--title" => {
                index += 1;
                run.title = Some(args.get(index).context("--title requires text")?.clone());
            }
            "--output-dir" => {
                index += 1;
                run.output_dir = Some(
                    args.get(index)
                        .context("--output-dir requires path")?
                        .clone(),
                );
            }
            value => bail!("unknown software-conformance-package option: {value}"),
        }
        index += 1;
    }

    Ok(run)
}

fn parse_provider_conformance_package(args: Vec<String>) -> Result<ProviderConformancePackageArgs> {
    let provider_id = args
        .first()
        .context("provider-conformance-package requires <provider-id>")?
        .clone();
    let mut run = ProviderConformancePackageArgs {
        provider_id,
        node_id: None,
        title: None,
        output_dir: None,
    };
    let mut index = 1;

    while index < args.len() {
        match args[index].as_str() {
            "--node-id" => {
                index += 1;
                run.node_id = Some(args.get(index).context("--node-id requires id")?.clone());
            }
            "--title" => {
                index += 1;
                run.title = Some(args.get(index).context("--title requires text")?.clone());
            }
            "--output-dir" => {
                index += 1;
                run.output_dir = Some(
                    args.get(index)
                        .context("--output-dir requires path")?
                        .clone(),
                );
            }
            value => bail!("unknown provider-conformance-package option: {value}"),
        }
        index += 1;
    }

    Ok(run)
}

fn parse_production_evidence_handoff_package(
    args: Vec<String>,
) -> Result<ProductionEvidenceHandoffPackageArgs> {
    let mut run = ProductionEvidenceHandoffPackageArgs {
        node_id: None,
        title: None,
        output_dir: None,
        output_root: None,
        source: None,
        include_items: true,
        include_snapshot: false,
    };
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--node-id" => {
                index += 1;
                run.node_id = Some(args.get(index).context("--node-id requires id")?.clone());
            }
            "--title" => {
                index += 1;
                run.title = Some(args.get(index).context("--title requires text")?.clone());
            }
            "--output-dir" => {
                index += 1;
                run.output_dir = Some(
                    args.get(index)
                        .context("--output-dir requires path")?
                        .clone(),
                );
            }
            "--output-root" => {
                index += 1;
                run.output_root = Some(
                    args.get(index)
                        .context("--output-root requires path")?
                        .clone(),
                );
            }
            "--source" => {
                index += 1;
                run.source = Some(args.get(index).context("--source requires value")?.clone());
            }
            value if value.starts_with("--source=") => {
                run.source = Some(value.trim_start_matches("--source=").to_string());
            }
            "--include-snapshot" => run.include_snapshot = true,
            "--no-items" => run.include_items = false,
            value => bail!("unknown production-evidence-handoff-package option: {value}"),
        }
        index += 1;
    }

    Ok(run)
}

fn parse_agent_session(args: Vec<String>) -> Result<AgentSessionArgs> {
    let kind = args
        .first()
        .context("agent-session requires <hermes|agent_cli>")?;
    let mut run = AgentSessionArgs {
        kind: normalize_agent_session_kind(kind)?,
        control_dir: None,
        endpoint: None,
        instruction: None,
        allowed_tools: Vec::new(),
        requires_confirmation: None,
        command_id: None,
        title: None,
        command: None,
        tools: Vec::new(),
        token_budget: None,
        execute: false,
        allowed_commands: Vec::new(),
        working_dir: None,
        max_output_bytes: None,
        timeout_ms: None,
    };
    let mut index = 1;

    while index < args.len() {
        match args[index].as_str() {
            "--control-dir" => {
                index += 1;
                run.control_dir = Some(
                    args.get(index)
                        .context("--control-dir requires path")?
                        .clone(),
                );
            }
            "--endpoint" => {
                index += 1;
                run.endpoint = Some(args.get(index).context("--endpoint requires url")?.clone());
            }
            "--instruction" => {
                index += 1;
                run.instruction = Some(
                    args.get(index)
                        .context("--instruction requires text")?
                        .clone(),
                );
            }
            "--allowed-tool" => {
                index += 1;
                run.allowed_tools.push(
                    args.get(index)
                        .context("--allowed-tool requires name")?
                        .clone(),
                );
            }
            "--requires-confirmation" => run.requires_confirmation = Some(true),
            "--no-confirmation" => run.requires_confirmation = Some(false),
            "--command-id" => {
                index += 1;
                run.command_id = Some(args.get(index).context("--command-id requires id")?.clone());
            }
            "--title" => {
                index += 1;
                run.title = Some(args.get(index).context("--title requires text")?.clone());
            }
            "--command" => {
                index += 1;
                run.command = Some(args.get(index).context("--command requires text")?.clone());
            }
            "--tool" => {
                index += 1;
                run.tools
                    .push(args.get(index).context("--tool requires name")?.clone());
            }
            "--token-budget" => {
                index += 1;
                let value = args.get(index).context("--token-budget requires integer")?;
                run.token_budget =
                    Some(value.parse().context("--token-budget must be an integer")?);
            }
            "--execute" => run.execute = true,
            "--allowed-command" => {
                index += 1;
                run.allowed_commands.push(
                    args.get(index)
                        .context("--allowed-command requires command")?
                        .clone(),
                );
            }
            "--working-dir" => {
                index += 1;
                run.working_dir = Some(
                    args.get(index)
                        .context("--working-dir requires path")?
                        .clone(),
                );
            }
            "--max-output-bytes" => {
                index += 1;
                let value = args
                    .get(index)
                    .context("--max-output-bytes requires integer")?;
                run.max_output_bytes = Some(
                    value
                        .parse()
                        .context("--max-output-bytes must be an integer")?,
                );
            }
            "--timeout-ms" => {
                index += 1;
                let value = args.get(index).context("--timeout-ms requires integer")?;
                run.timeout_ms = Some(value.parse().context("--timeout-ms must be an integer")?);
            }
            value => bail!("unknown agent-session option: {value}"),
        }
        index += 1;
    }

    Ok(run)
}

fn parse_desktop_result(args: Vec<String>) -> Result<DesktopResultArgs> {
    let software_action_id = args
        .first()
        .context("desktop-result requires <software-action-id>")?
        .clone();
    let mut run = DesktopResultArgs {
        software_action_id,
        task_id: None,
        status: "succeeded".to_string(),
        message: None,
        artifacts: Vec::new(),
        screen_trace_path: None,
        result: None,
        verification: None,
    };
    let mut index = 1;

    while index < args.len() {
        match args[index].as_str() {
            "--task-id" => {
                index += 1;
                run.task_id = Some(args.get(index).context("--task-id requires id")?.clone());
            }
            "--status" => {
                index += 1;
                run.status = args.get(index).context("--status requires value")?.clone();
            }
            "--message" => {
                index += 1;
                run.message = Some(args.get(index).context("--message requires text")?.clone());
            }
            "--artifact" => {
                index += 1;
                run.artifacts
                    .push(args.get(index).context("--artifact requires path")?.clone());
            }
            "--screen-trace-path" => {
                index += 1;
                run.screen_trace_path = Some(
                    args.get(index)
                        .context("--screen-trace-path requires path")?
                        .clone(),
                );
            }
            "--result-json" => {
                index += 1;
                let value = args.get(index).context("--result-json requires JSON")?;
                run.result = Some(parse_json_value(value, "--result-json")?);
            }
            "--verification-json" => {
                index += 1;
                let value = args
                    .get(index)
                    .context("--verification-json requires JSON")?;
                run.verification = Some(parse_json_value(value, "--verification-json")?);
            }
            value => bail!("unknown desktop-result option: {value}"),
        }
        index += 1;
    }

    Ok(run)
}

fn parse_desktop_run_next(args: Vec<String>) -> Result<DesktopRunNextArgs> {
    let mut run = DesktopRunNextArgs {
        status: "succeeded".to_string(),
        message: None,
        controller_id: "pool-cli-desktop-controller".to_string(),
        limit: 1,
        artifacts: Vec::new(),
        screen_trace_path: None,
    };
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--status" => {
                index += 1;
                run.status = args.get(index).context("--status requires value")?.clone();
            }
            "--message" => {
                index += 1;
                run.message = Some(args.get(index).context("--message requires text")?.clone());
            }
            "--controller-id" => {
                index += 1;
                run.controller_id = args
                    .get(index)
                    .context("--controller-id requires value")?
                    .clone();
            }
            "--limit" => {
                index += 1;
                let value = args.get(index).context("--limit requires integer")?;
                run.limit = value.parse().context("--limit must be an integer")?;
            }
            "--artifact" => {
                index += 1;
                run.artifacts
                    .push(args.get(index).context("--artifact requires path")?.clone());
            }
            "--screen-trace-path" => {
                index += 1;
                run.screen_trace_path = Some(
                    args.get(index)
                        .context("--screen-trace-path requires path")?
                        .clone(),
                );
            }
            value => bail!("unknown desktop-run-next option: {value}"),
        }
        index += 1;
    }

    Ok(run)
}

fn parse_prd_completion_gate(args: Vec<String>) -> Result<PrdCompletionGateArgs> {
    let mut parsed = PrdCompletionGateArgs::default();
    for arg in args {
        match arg.as_str() {
            "--require-complete" | "--fail-if-incomplete" => parsed.require_complete = true,
            value => bail!("unknown prd-completion-gate option: {value}"),
        }
    }
    Ok(parsed)
}

fn parse_core_architecture_gate(args: Vec<String>) -> Result<CoreArchitectureGateArgs> {
    let mut parsed = CoreArchitectureGateArgs::default();
    for arg in args {
        match arg.as_str() {
            "--require-ready" | "--require-complete" | "--fail-if-incomplete" => {
                parsed.require_ready = true
            }
            value => bail!("unknown core-architecture-gate option: {value}"),
        }
    }
    Ok(parsed)
}

fn parse_prd_completion_package(args: Vec<String>) -> Result<PrdCompletionPackageArgs> {
    let mut package = PrdCompletionPackageArgs {
        node_id: None,
        title: None,
        output_dir: None,
        source: None,
        include_snapshot: true,
    };
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--node-id" => {
                index += 1;
                package.node_id = Some(args.get(index).context("--node-id requires id")?.clone());
            }
            "--title" => {
                index += 1;
                package.title = Some(args.get(index).context("--title requires text")?.clone());
            }
            "--output-dir" => {
                index += 1;
                package.output_dir = Some(
                    args.get(index)
                        .context("--output-dir requires path")?
                        .clone(),
                );
            }
            "--source" => {
                index += 1;
                package.source = Some(args.get(index).context("--source requires value")?.clone());
            }
            value if value.starts_with("--source=") => {
                package.source = Some(value.trim_start_matches("--source=").to_string());
            }
            "--include-snapshot" => package.include_snapshot = true,
            "--no-snapshot" => package.include_snapshot = false,
            value => bail!("unknown prd-completion-package option: {value}"),
        }
        index += 1;
    }

    Ok(package)
}

fn parse_core_architecture_package(args: Vec<String>) -> Result<CoreArchitecturePackageArgs> {
    let mut package = CoreArchitecturePackageArgs {
        node_id: None,
        title: None,
        output_dir: None,
        source: None,
        include_snapshot: true,
    };
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--node-id" => {
                index += 1;
                package.node_id = Some(args.get(index).context("--node-id requires id")?.clone());
            }
            "--title" => {
                index += 1;
                package.title = Some(args.get(index).context("--title requires text")?.clone());
            }
            "--output-dir" => {
                index += 1;
                package.output_dir = Some(
                    args.get(index)
                        .context("--output-dir requires path")?
                        .clone(),
                );
            }
            "--source" => {
                index += 1;
                package.source = Some(args.get(index).context("--source requires value")?.clone());
            }
            value if value.starts_with("--source=") => {
                package.source = Some(value.trim_start_matches("--source=").to_string());
            }
            "--include-snapshot" => package.include_snapshot = true,
            "--no-snapshot" => package.include_snapshot = false,
            value => bail!("unknown core-architecture-package option: {value}"),
        }
        index += 1;
    }

    Ok(package)
}

fn dispatch(cli: Cli) -> Result<RuntimeHttpResponse> {
    let server = runtime_server(&cli);
    match cli.command {
        Command::Status => server.handle_path(&path_with_project("/api/health", &cli.project_slug)),
        Command::Snapshot => {
            server.handle_path(&path_with_project("/api/snapshot", &cli.project_slug))
        }
        Command::Projects => {
            server.handle_path(&path_with_project("/api/projects", &cli.project_slug))
        }
        Command::Resources => {
            server.handle_path(&path_with_project("/api/resources", &cli.project_slug))
        }
        Command::ApiKeys(args) => server.handle_path(&api_keys_path(&args, &cli.project_slug)),
        Command::Adapters => server.handle_path("/api/adapters"),
        Command::IntegrationReadiness => server.handle_path(&path_with_project(
            "/api/integration-readiness",
            &cli.project_slug,
        )),
        Command::ProviderContracts { provider_id } => {
            server.handle_path(&provider_contracts_path(provider_id.as_deref()))
        }
        Command::ProviderConformancePackages => server.handle_path(&path_with_project(
            "/api/provider-conformance-packages",
            &cli.project_slug,
        )),
        Command::ProviderConformancePackage(args) => {
            if cli.project_slug.as_deref() == Some("*") {
                bail!("provider-conformance-package requires a concrete --project slug, not *");
            }
            let body = provider_conformance_package_body(&cli.project_slug, args);
            server.handle_request_with_body(
                "POST",
                "/api/provider-conformance-packages",
                &body.to_string(),
            )
        }
        Command::IntegrationConformancePackage(args) => {
            if cli.project_slug.as_deref() == Some("*") {
                bail!("integration-conformance-package requires a concrete --project slug, not *");
            }
            let body = integration_conformance_package_body(&cli.project_slug, args);
            server.handle_request_with_body(
                "POST",
                "/api/integration-conformance-packages",
                &body.to_string(),
            )
        }
        Command::SoftwareContracts { adapter_id } => {
            server.handle_path(&software_contracts_path(adapter_id.as_deref()))
        }
        Command::SoftwareConformancePackages => server.handle_path(&path_with_project(
            "/api/software-conformance-packages",
            &cli.project_slug,
        )),
        Command::SoftwareConformancePackage(args) => {
            if cli.project_slug.as_deref() == Some("*") {
                bail!("software-conformance-package requires a concrete --project slug, not *");
            }
            let body = software_conformance_package_body(&cli.project_slug, args);
            server.handle_request_with_body(
                "POST",
                "/api/software-conformance-packages",
                &body.to_string(),
            )
        }
        Command::Tasks => server.handle_path(&path_with_query(
            "/api/mcp",
            &[("uri", "pool://tasks")],
            &cli.project_slug,
        )),
        Command::Events(args) => server.handle_path(&events_path(&args, &cli.project_slug)),
        Command::RuntimeBudget => {
            server.handle_path(&path_with_project("/api/runtime-budget", &cli.project_slug))
        }
        Command::RuntimePreflight => server.handle_path(&path_with_project(
            "/api/runtime-preflight",
            &cli.project_slug,
        )),
        Command::RuntimeExecutionPlan => server.handle_path(&path_with_project(
            "/api/runtime-execution-plan",
            &cli.project_slug,
        )),
        Command::RuntimeExecutionPlanRunNext(args) => {
            if cli.project_slug.as_deref() == Some("*") {
                bail!("runtime-run-next requires a concrete --project slug, not *");
            }
            let body = runtime_run_next_body(&cli.project_slug, args);
            server.handle_request_with_body(
                "POST",
                &path_with_project("/api/runtime-execution-plan/run-next", &cli.project_slug),
                &body.to_string(),
            )
        }
        Command::RuntimeHandoff => server.handle_path(&path_with_project(
            "/api/runtime-handoff",
            &cli.project_slug,
        )),
        Command::RuntimeHandoffPackages => server.handle_path(&path_with_project(
            "/api/handoff-packages",
            &cli.project_slug,
        )),
        Command::CoreArchitectureReadiness => server.handle_path(&path_with_project(
            "/api/core-architecture-readiness",
            &cli.project_slug,
        )),
        Command::CoreArchitectureGate(args) => {
            server.handle_path(&core_architecture_gate_path(&cli.project_slug, &args))
        }
        Command::CoreArchitecturePackages => server.handle_path(&path_with_project(
            "/api/core-architecture-packages",
            &cli.project_slug,
        )),
        Command::CoreArchitecturePackage(args) => {
            if cli.project_slug.as_deref() == Some("*") {
                bail!("core-architecture-package requires a concrete --project slug, not *");
            }
            let body = core_architecture_package_body(&cli.project_slug, args);
            server.handle_request_with_body(
                "POST",
                "/api/core-architecture-package",
                &body.to_string(),
            )
        }
        Command::PrdReadiness => {
            server.handle_path(&path_with_project("/api/prd-readiness", &cli.project_slug))
        }
        Command::PrdCompletionGate(args) => {
            server.handle_path(&prd_completion_gate_path(&cli.project_slug, &args))
        }
        Command::PrdCompletionPackages => server.handle_path(&path_with_project(
            "/api/prd-completion-packages",
            &cli.project_slug,
        )),
        Command::PrdCompletionPackage(args) => {
            if cli.project_slug.as_deref() == Some("*") {
                bail!("prd-completion-package requires a concrete --project slug, not *");
            }
            let body = prd_completion_package_body(&cli.project_slug, args);
            server.handle_request_with_body(
                "POST",
                "/api/prd-completion-package",
                &body.to_string(),
            )
        }
        Command::RuntimeGraph => {
            server.handle_path(&path_with_project("/api/runtime-graph", &cli.project_slug))
        }
        Command::WorkflowContext { workflow_id } => {
            if let Some(workflow_id) = workflow_id {
                server.handle_path(&path_with_query(
                    "/api/workflow-context",
                    &[("workflow_id", workflow_id.as_str())],
                    &cli.project_slug,
                ))
            } else {
                server.handle_path(&path_with_project(
                    "/api/workflow-context",
                    &cli.project_slug,
                ))
            }
        }
        Command::NodeContext { node_id } => {
            if let Some(node_id) = node_id {
                server.handle_path(&path_with_query(
                    "/api/node-context",
                    &[("node_id", node_id.as_str())],
                    &cli.project_slug,
                ))
            } else {
                server.handle_path(&path_with_project("/api/node-context", &cli.project_slug))
            }
        }
        Command::Mcp { uri } => server.handle_path(&path_with_query(
            "/api/mcp",
            &[("uri", uri.as_str())],
            &cli.project_slug,
        )),
        Command::ServeMcp => bail!("serve-mcp is handled before runtime HTTP dispatch"),
        Command::ProviderGatewayWorkerContract => {
            server.handle_path("/api/provider-gateway-worker")
        }
        Command::ProviderGatewayWorker(_) => {
            bail!("provider-gateway-worker is handled before runtime HTTP dispatch")
        }
        Command::ProviderSdkWorkerTemplate(_) => {
            bail!("provider-sdk-worker-template is handled before runtime HTTP dispatch")
        }
        Command::UnrealMcpBridgeContract => server.handle_path("/api/unreal-mcp-bridge"),
        Command::UnrealMcpBridgeWorker(_) => {
            bail!("unreal-mcp-bridge-worker is handled before runtime HTTP dispatch")
        }
        Command::HermesMcpBridgeWorker(_) => {
            bail!("hermes-mcp-bridge-worker is handled before runtime HTTP dispatch")
        }
        Command::SoftwareApiBridgeWorker(_) => {
            bail!("software-api-bridge-worker is handled before runtime HTTP dispatch")
        }
        Command::WorkerSelfChecks(_) => {
            bail!("worker-self-checks is handled before runtime HTTP dispatch")
        }
        Command::AdapterHealth(args) => {
            let body = adapter_health_body(args);
            server.handle_request_with_body("POST", "/api/adapter-health", &body.to_string())
        }
        Command::ProviderHealth(args) => {
            let body = provider_health_body(args);
            server.handle_request_with_body("POST", "/api/provider-health", &body.to_string())
        }
        Command::RunProvider(args) => {
            if cli.project_slug.as_deref() == Some("*") {
                bail!("run-provider requires a concrete --project slug, not *");
            }
            let body = provider_run_body(&cli.project_slug, args);
            server.handle_request_with_body("POST", "/api/provider-runs", &body.to_string())
        }
        Command::ProductionEvidenceProviderMatrix(args) => {
            if cli.project_slug.as_deref() == Some("*") {
                bail!(
                    "production-evidence-provider-matrix requires a concrete --project slug, not *"
                );
            }
            production_evidence_provider_matrix_response(&server, &cli.project_slug, args)
        }
        Command::ProductionEvidenceSoftwareMatrix(args) => {
            if cli.project_slug.as_deref() == Some("*") {
                bail!(
                    "production-evidence-software-matrix requires a concrete --project slug, not *"
                );
            }
            production_evidence_software_matrix_response(&server, &cli.project_slug, args)
        }
        Command::ProductionEvidenceDesktopVision(args) => {
            if cli.project_slug.as_deref() == Some("*") {
                bail!(
                    "production-evidence-desktop-vision requires a concrete --project slug, not *"
                );
            }
            production_evidence_desktop_vision_response(&server, &cli.project_slug, args)
        }
        Command::ProductionEvidenceRequirements => server.handle_path(&path_with_project(
            "/api/production-evidence/requirements",
            &cli.project_slug,
        )),
        Command::ProductionEvidenceTasks => server.handle_path(&path_with_project(
            "/api/production-evidence/tasks",
            &cli.project_slug,
        )),
        Command::ProductionEvidenceTaskClaim(args) => {
            if cli.project_slug.as_deref() == Some("*") {
                bail!("production-evidence-claim requires a concrete --project slug, not *");
            }
            let body = production_evidence_task_claim_body(&cli.project_slug, args);
            server.handle_request_with_body(
                "POST",
                "/api/production-evidence/tasks/claim",
                &body.to_string(),
            )
        }
        Command::ProductionEvidenceRunPlan(args) => {
            if cli.project_slug.as_deref() == Some("*") {
                bail!("production-evidence-run-plan requires a concrete --project slug, not *");
            }
            let response = server.handle_path(&production_evidence_run_plan_path(
                &cli.project_slug,
                args.output_root.as_deref(),
                args.source.as_deref(),
            ))?;
            write_json_response_to_path(
                response,
                args.path.as_deref(),
                "production evidence run plan",
                "written_run_plan_path",
            )
        }
        Command::ProductionEvidenceHandoff(args) => {
            if cli.project_slug.as_deref() == Some("*") {
                bail!("production-evidence-handoff requires a concrete --project slug, not *");
            }
            let response = server.handle_path(&production_evidence_handoff_path(
                &cli.project_slug,
                args.output_root.as_deref(),
                args.source.as_deref(),
            ))?;
            write_production_evidence_handoff_response(response, args.path.as_deref())
        }
        Command::ProductionEvidenceHandoffPackages => server.handle_path(&path_with_project(
            "/api/production-evidence/handoff-packages",
            &cli.project_slug,
        )),
        Command::ProductionEvidenceHandoffPackage(args) => {
            if cli.project_slug.as_deref() == Some("*") {
                bail!(
                    "production-evidence-handoff-package requires a concrete --project slug, not *"
                );
            }
            let body = production_evidence_handoff_package_body(&cli.project_slug, args);
            server.handle_request_with_body(
                "POST",
                "/api/production-evidence/handoff-packages",
                &body.to_string(),
            )
        }
        Command::ProductionEvidenceTemplate(args) => {
            if cli.project_slug.as_deref() == Some("*") {
                bail!("production-evidence-template requires a concrete --project slug, not *");
            }
            let response = server.handle_path(&production_evidence_template_path(
                &cli.project_slug,
                args.output_root.as_deref(),
                args.source.as_deref(),
                args.missing_only,
            ))?;
            write_production_evidence_template_response(response, args.path.as_deref())
        }
        Command::ProductionEvidenceItemTemplate(args) => {
            if cli.project_slug.as_deref() == Some("*") {
                bail!(
                    "production-evidence-item-template requires a concrete --project slug, not *"
                );
            }
            let response = server.handle_path(&production_evidence_item_template_path(
                &cli.project_slug,
                args.output_root.as_deref(),
                args.source.as_deref(),
                args.task_id.as_deref(),
                args.kind.as_deref(),
                args.target_id.as_deref(),
            ))?;
            write_production_evidence_item_template_response(response, args.path.as_deref())
        }
        Command::ProductionEvidenceItemFromLedger(args) => {
            if cli.project_slug.as_deref() == Some("*") {
                bail!(
                    "production-evidence-item-from-ledger requires a concrete --project slug, not *"
                );
            }
            let response = server.handle_path(&production_evidence_item_from_ledger_path(
                &cli.project_slug,
                args.source.as_deref(),
                args.provider_request_id.as_deref(),
                args.software_action_id.as_deref(),
                args.desktop_vision_action_id.as_deref(),
            ))?;
            write_production_evidence_item_template_response(response, args.path.as_deref())
        }
        Command::ProductionEvidenceBundleFromLedger(args) => {
            if cli.project_slug.as_deref() == Some("*") {
                bail!(
                    "production-evidence-bundle-from-ledger requires a concrete --project slug, not *"
                );
            }
            let response = server.handle_path(&production_evidence_bundle_from_ledger_path(
                &cli.project_slug,
                args.source.as_deref(),
                args.include_incomplete,
            ))?;
            write_production_evidence_bundle_from_ledger_response(response, args.path.as_deref())
        }
        Command::MergeProductionEvidence(args) => {
            merge_production_evidence_response(&cli.project_slug, args)
        }
        Command::CloseoutProductionEvidence(args) => {
            if cli.project_slug.as_deref() == Some("*") {
                bail!("closeout-production-evidence requires a concrete --project slug, not *");
            }
            let body = production_evidence_closeout_body(&cli.project_slug, &args)?;
            let response = server.handle_request_with_body(
                "POST",
                "/api/production-evidence/closeout",
                &body.to_string(),
            )?;
            write_production_evidence_closeout_response(response, args.output_path.as_deref())
        }
        Command::ValidateProductionEvidence { path } => {
            if cli.project_slug.as_deref() == Some("*") {
                bail!("validate-production-evidence requires a concrete --project slug, not *");
            }
            let body = production_evidence_body(&cli.project_slug, path)?;
            server.handle_request_with_body(
                "POST",
                "/api/production-evidence/validate",
                &body.to_string(),
            )
        }
        Command::ImportProductionEvidence { path } => {
            if cli.project_slug.as_deref() == Some("*") {
                bail!("import-production-evidence requires a concrete --project slug, not *");
            }
            let body = production_evidence_body(&cli.project_slug, path)?;
            server.handle_request_with_body("POST", "/api/production-evidence", &body.to_string())
        }
        Command::ValidateProductionEvidenceItem { path } => {
            if cli.project_slug.as_deref() == Some("*") {
                bail!(
                    "validate-production-evidence-item requires a concrete --project slug, not *"
                );
            }
            let body = production_evidence_item_body(&cli.project_slug, path)?;
            server.handle_request_with_body(
                "POST",
                "/api/production-evidence/items/validate",
                &body.to_string(),
            )
        }
        Command::SubmitProductionEvidenceItem { path } => {
            if cli.project_slug.as_deref() == Some("*") {
                bail!("submit-production-evidence-item requires a concrete --project slug, not *");
            }
            let body = production_evidence_item_body(&cli.project_slug, path)?;
            server.handle_request_with_body(
                "POST",
                "/api/production-evidence/items",
                &body.to_string(),
            )
        }
        Command::ProviderRequestMetadata {
            provider_request_id,
        } => server.handle_path(&provider_request_metadata_path(
            provider_request_id.as_str(),
            &cli.project_slug,
        )),
        Command::SoftwareHealth(args) => {
            let body = software_health_body(args);
            server.handle_request_with_body("POST", "/api/software-health", &body.to_string())
        }
        Command::OutputPackages => server.handle_path(&path_with_project(
            "/api/output-packages",
            &cli.project_slug,
        )),
        Command::RunNode(args) => {
            if cli.project_slug.as_deref() == Some("*") {
                bail!("run-node requires a concrete --project slug, not *");
            }
            let body = run_node_body(&cli.project_slug, args);
            server.handle_request_with_body("POST", "/api/nodes/run", &body.to_string())
        }
        Command::RunSoftware(args) => {
            if cli.project_slug.as_deref() == Some("*") {
                bail!("run-software requires a concrete --project slug, not *");
            }
            let body = software_action_body(&cli.project_slug, args);
            server.handle_request_with_body("POST", "/api/software-actions", &body.to_string())
        }
        Command::RunWorkflow(args) => {
            if cli.project_slug.as_deref() == Some("*") {
                bail!("run-workflow requires a concrete --project slug, not *");
            }
            let body = workflow_run_body(&cli.project_slug, args);
            server.handle_request_with_body("POST", "/api/workflow-runs", &body.to_string())
        }
        Command::OutputPackage(args) => {
            if cli.project_slug.as_deref() == Some("*") {
                bail!("output-package requires a concrete --project slug, not *");
            }
            let body = output_package_body(&cli.project_slug, args);
            server.handle_request_with_body("POST", "/api/output-packages", &body.to_string())
        }
        Command::OutputResult(args) => {
            if cli.project_slug.as_deref() == Some("*") {
                bail!("output-result requires a concrete --project slug, not *");
            }
            let body = output_result_body(&cli.project_slug, args);
            server.handle_request_with_body(
                "POST",
                "/api/output-packages/results",
                &body.to_string(),
            )
        }
        Command::HandoffPackage(args) => {
            if cli.project_slug.as_deref() == Some("*") {
                bail!("handoff-package requires a concrete --project slug, not *");
            }
            let body = handoff_package_body(&cli.project_slug, args);
            server.handle_request_with_body("POST", "/api/handoff-packages", &body.to_string())
        }
        Command::IntegrationConformancePackages => server.handle_path(&path_with_project(
            "/api/integration-conformance-packages",
            &cli.project_slug,
        )),
        Command::AgentConformancePackages => server.handle_path(&path_with_project(
            "/api/agent-conformance-packages",
            &cli.project_slug,
        )),
        Command::AgentConformancePackage(args) => {
            if cli.project_slug.as_deref() == Some("*") {
                bail!("agent-conformance-package requires a concrete --project slug, not *");
            }
            let body = agent_conformance_package_body(&cli.project_slug, args);
            server.handle_request_with_body(
                "POST",
                "/api/agent-conformance-packages",
                &body.to_string(),
            )
        }
        Command::AgentSession(args) => {
            if cli.project_slug.as_deref() == Some("*") {
                bail!("agent-session requires a concrete --project slug, not *");
            }
            let body = agent_session_body(&cli.project_slug, args);
            server.handle_request_with_body("POST", "/api/agent-sessions", &body.to_string())
        }
        Command::AgentTranscript { session_id } => server.handle_path(&path_with_query(
            "/api/agent-sessions/transcript",
            &[("session_id", session_id.as_str())],
            &cli.project_slug,
        )),
        Command::AgentStream(args) => {
            server.handle_path(&agent_stream_path(&args, &cli.project_slug))
        }
        Command::DesktopContract => server.handle_path("/api/desktop-recognition/contract"),
        Command::DesktopRequests => server.handle_path(&path_with_project(
            "/api/desktop-recognition/requests",
            &cli.project_slug,
        )),
        Command::DesktopRunNext(args) => {
            let body = desktop_run_next_body(args);
            server.handle_request_with_body(
                "POST",
                &path_with_project("/api/desktop-recognition/run-next", &cli.project_slug),
                &body.to_string(),
            )
        }
        Command::DesktopResult(args) => {
            let body = desktop_result_body(args);
            server.handle_request_with_body(
                "POST",
                "/api/desktop-recognition/results",
                &body.to_string(),
            )
        }
        Command::SetApiKey(args) => {
            let body = set_api_key_body(&cli.project_slug, args);
            server.handle_request_with_body("POST", "/api/api-keys", &body.to_string())
        }
        Command::TaskAction(action) => server.handle_request(
            "POST",
            &path_with_query(
                task_action_path(&action.kind),
                &[("task_id", action.task_id.as_str())],
                &cli.project_slug,
            ),
        ),
    }
}

fn runtime_server(cli: &Cli) -> RuntimeHttpServer {
    let config = match cli.project_slug.as_deref() {
        Some("*") | None => RuntimeHttpConfig::new(&cli.db_path),
        Some(project_slug) => RuntimeHttpConfig::new(&cli.db_path).with_project_slug(project_slug),
    };
    RuntimeHttpServer::new(config)
}

#[derive(Debug, Clone)]
struct McpHttpToolRequest {
    method: &'static str,
    path: String,
    body: Option<Value>,
}

const MCP_PROTOCOL_VERSION: &str = "2025-11-25";

fn serve_mcp_stdio(cli: &Cli) -> Result<()> {
    let server = runtime_server(cli);
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = line.context("read MCP stdio message")?;
        if line.trim().is_empty() {
            continue;
        }
        if let Some(response) = handle_mcp_stdio_line(&server, &cli.project_slug, &line) {
            write_mcp_stdio_message(&mut stdout, &response)?;
        }
    }

    Ok(())
}

fn handle_mcp_stdio_line(
    server: &RuntimeHttpServer,
    project_slug: &Option<String>,
    line: &str,
) -> Option<Value> {
    let message = match serde_json::from_str::<Value>(line) {
        Ok(message) => message,
        Err(error) => {
            return Some(mcp_error_response(
                Value::Null,
                -32700,
                "parse_error",
                Some(json!({ "message": error.to_string() })),
            ));
        }
    };
    let request_id = message.get("id").cloned();

    match handle_mcp_message(server, project_slug, message) {
        Ok(response) => response,
        Err(error) => Some(mcp_error_response(
            request_id.unwrap_or(Value::Null),
            -32603,
            "internal_error",
            Some(json!({ "message": error.to_string() })),
        )),
    }
}

fn handle_mcp_message(
    server: &RuntimeHttpServer,
    project_slug: &Option<String>,
    message: Value,
) -> Result<Option<Value>> {
    let id = message.get("id").cloned();
    let method = message.get("method").and_then(Value::as_str);

    let Some(method) = method else {
        return Ok(Some(mcp_error_response(
            id.unwrap_or(Value::Null),
            -32600,
            "invalid_request",
            Some(json!({ "message": "MCP JSON-RPC request requires method" })),
        )));
    };

    if id.is_none() {
        return Ok(None);
    }
    let id = id.unwrap_or(Value::Null);

    let result = match method {
        "initialize" => mcp_initialize_result(),
        "ping" => json!({}),
        "resources/list" => mcp_resources_list_result(server, project_slug)?,
        "resources/read" => {
            let params = message.get("params").cloned().unwrap_or_else(|| json!({}));
            let uri = params
                .get("uri")
                .and_then(Value::as_str)
                .context("resources/read requires params.uri")?;
            mcp_resource_read_result(server, project_slug, uri)?
        }
        "tools/list" => json!({ "tools": mcp_tool_definitions() }),
        "tools/call" => {
            let params = message.get("params").cloned().unwrap_or_else(|| json!({}));
            let name = params
                .get("name")
                .and_then(Value::as_str)
                .context("tools/call requires params.name")?;
            let arguments = params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            execute_mcp_tool(server, project_slug, name, arguments)?
        }
        "prompts/list" => json!({ "prompts": pool_mcp_prompt_definitions() }),
        "prompts/get" => {
            let params = message.get("params").cloned().unwrap_or_else(|| json!({}));
            pool_mcp_prompt_get_result(params)?
        }
        _ => {
            return Ok(Some(mcp_error_response(
                id,
                -32601,
                "method_not_found",
                Some(json!({ "method": method })),
            )));
        }
    };

    Ok(Some(mcp_success_response(id, result)))
}

fn write_mcp_stdio_message(writer: &mut impl Write, response: &Value) -> Result<()> {
    let line = serde_json::to_string(response)?;
    writer
        .write_all(line.as_bytes())
        .context("write MCP stdio response")?;
    writer.write_all(b"\n").context("write MCP stdio newline")?;
    writer.flush().context("flush MCP stdio response")
}

fn mcp_success_response(id: Value, result: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result,
    })
}

fn mcp_error_response(id: Value, code: i64, message: &str, data: Option<Value>) -> Value {
    let mut error = Map::new();
    error.insert("code".to_string(), json!(code));
    error.insert("message".to_string(), json!(message));
    if let Some(data) = data {
        error.insert("data".to_string(), data);
    }
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": Value::Object(error),
    })
}

fn mcp_initialize_result() -> Value {
    json!({
        "protocolVersion": MCP_PROTOCOL_VERSION,
        "capabilities": {
            "resources": {},
            "tools": {},
            "prompts": {},
        },
        "serverInfo": {
            "name": "pool-runtime",
            "version": env!("CARGO_PKG_VERSION"),
        },
        "instructions": "Pool local runtime control surface. Read pool:// resources before mutating tasks; high-cost generation still requires Pool approval gates.",
    })
}

fn mcp_resources_list_result(
    server: &RuntimeHttpServer,
    project_slug: &Option<String>,
) -> Result<Value> {
    let response = server.handle_path(&path_with_project("/api/resources", project_slug))?;
    let payload = runtime_response_json(&response);
    let resources = payload
        .get("resources")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|resource| {
            let mut resource = resource.as_object().cloned().unwrap_or_default();
            resource.insert("mimeType".to_string(), json!("application/json"));
            Value::Object(resource)
        })
        .collect::<Vec<_>>();
    Ok(json!({ "resources": resources }))
}

fn mcp_resource_read_result(
    server: &RuntimeHttpServer,
    project_slug: &Option<String>,
    uri: &str,
) -> Result<Value> {
    let response =
        server.handle_path(&path_with_query("/api/mcp", &[("uri", uri)], project_slug))?;
    Ok(json!({
        "contents": [
            {
                "uri": uri,
                "mimeType": "application/json",
                "text": response.body,
            }
        ]
    }))
}

fn execute_mcp_tool(
    server: &RuntimeHttpServer,
    project_slug: &Option<String>,
    name: &str,
    arguments: Value,
) -> Result<Value> {
    if name == "pool_worker_self_checks" {
        let report = worker_self_checks_report(mcp_worker_self_checks_args(arguments)?)?;
        return Ok(mcp_tool_result_from_value(report, false)?);
    }

    let request = mcp_tool_http_request(name, arguments, project_slug)?;
    let response = if request.method == "GET" {
        server.handle_path(&request.path)?
    } else {
        let body = request.body.unwrap_or_else(|| json!({}));
        server.handle_request_with_body(request.method, &request.path, &body.to_string())?
    };

    Ok(mcp_tool_result_from_response(response))
}

fn mcp_tool_result_from_value(structured_content: Value, is_error: bool) -> Result<Value> {
    Ok(json!({
        "content": [
            {
                "type": "text",
                "text": serde_json::to_string_pretty(&structured_content)?,
            }
        ],
        "structuredContent": structured_content,
        "isError": is_error,
    }))
}

fn mcp_tool_result_from_response(response: RuntimeHttpResponse) -> Value {
    let structured_content = runtime_response_json(&response);
    json!({
        "content": [
            {
                "type": "text",
                "text": response.body,
            }
        ],
        "structuredContent": structured_content,
        "isError": response.status_code >= 400,
    })
}

fn runtime_response_json(response: &RuntimeHttpResponse) -> Value {
    serde_json::from_str(&response.body).unwrap_or_else(|_| {
        json!({
            "status_code": response.status_code,
            "content_type": response.content_type,
            "body": response.body,
        })
    })
}

fn mcp_tool_http_request(
    name: &str,
    arguments: Value,
    default_project_slug: &Option<String>,
) -> Result<McpHttpToolRequest> {
    let args = mcp_arguments_object(arguments)?;
    let project_slug = mcp_project_slug(&args, default_project_slug);

    match name {
        "pool_status" => Ok(McpHttpToolRequest {
            method: "GET",
            path: path_with_project("/api/health", &project_slug),
            body: None,
        }),
        "pool_snapshot" => Ok(McpHttpToolRequest {
            method: "GET",
            path: path_with_project("/api/snapshot", &project_slug),
            body: None,
        }),
        "pool_projects" => Ok(McpHttpToolRequest {
            method: "GET",
            path: path_with_project("/api/projects", &project_slug),
            body: None,
        }),
        "pool_events" => Ok(McpHttpToolRequest {
            method: "GET",
            path: events_path(
                &EventsArgs {
                    after_id: mcp_optional_string_arg(&args, "after_id"),
                    limit: mcp_optional_u16_arg(&args, "limit")?,
                },
                &project_slug,
            ),
            body: None,
        }),
        "pool_adapters" => Ok(McpHttpToolRequest {
            method: "GET",
            path: "/api/adapters".to_string(),
            body: None,
        }),
        "pool_integration_readiness" => Ok(McpHttpToolRequest {
            method: "GET",
            path: path_with_project("/api/integration-readiness", &project_slug),
            body: None,
        }),
        "pool_software_contracts" => Ok(McpHttpToolRequest {
            method: "GET",
            path: software_contracts_path(mcp_optional_string_arg(&args, "adapter_id").as_deref()),
            body: None,
        }),
        "pool_software_conformance_packages" => Ok(McpHttpToolRequest {
            method: "GET",
            path: path_with_project("/api/software-conformance-packages", &project_slug),
            body: None,
        }),
        "pool_software_conformance_package" => Ok(McpHttpToolRequest {
            method: "POST",
            path: "/api/software-conformance-packages".to_string(),
            body: Some(mcp_body_with_project(args, default_project_slug)?),
        }),
        "pool_provider_conformance_packages" => Ok(McpHttpToolRequest {
            method: "GET",
            path: path_with_project("/api/provider-conformance-packages", &project_slug),
            body: None,
        }),
        "pool_provider_conformance_package" => Ok(McpHttpToolRequest {
            method: "POST",
            path: "/api/provider-conformance-packages".to_string(),
            body: Some(mcp_body_with_project(args, default_project_slug)?),
        }),
        "pool_integration_conformance_packages" => Ok(McpHttpToolRequest {
            method: "GET",
            path: path_with_project("/api/integration-conformance-packages", &project_slug),
            body: None,
        }),
        "pool_integration_conformance_package" => Ok(McpHttpToolRequest {
            method: "POST",
            path: "/api/integration-conformance-packages".to_string(),
            body: Some(mcp_body_with_project(args, default_project_slug)?),
        }),
        "pool_runtime_graph" => Ok(McpHttpToolRequest {
            method: "GET",
            path: path_with_project("/api/runtime-graph", &project_slug),
            body: None,
        }),
        "pool_runtime_budget" => Ok(McpHttpToolRequest {
            method: "GET",
            path: path_with_project("/api/runtime-budget", &project_slug),
            body: None,
        }),
        "pool_runtime_preflight" => Ok(McpHttpToolRequest {
            method: "GET",
            path: path_with_project("/api/runtime-preflight", &project_slug),
            body: None,
        }),
        "pool_runtime_execution_plan" => Ok(McpHttpToolRequest {
            method: "GET",
            path: path_with_project("/api/runtime-execution-plan", &project_slug),
            body: None,
        }),
        "pool_runtime_execution_plan_run_next" => Ok(McpHttpToolRequest {
            method: "POST",
            path: path_with_project("/api/runtime-execution-plan/run-next", &project_slug),
            body: Some(mcp_body_without_project(args)),
        }),
        "pool_runtime_handoff" => Ok(McpHttpToolRequest {
            method: "GET",
            path: path_with_project("/api/runtime-handoff", &project_slug),
            body: None,
        }),
        "pool_runtime_handoff_packages" => Ok(McpHttpToolRequest {
            method: "GET",
            path: path_with_project("/api/handoff-packages", &project_slug),
            body: None,
        }),
        "pool_output_packages" => Ok(McpHttpToolRequest {
            method: "GET",
            path: path_with_project("/api/output-packages", &project_slug),
            body: None,
        }),
        "pool_workflow_context" => {
            let path = if let Some(workflow_id) = mcp_optional_string_arg(&args, "workflow_id") {
                path_with_query(
                    "/api/workflow-context",
                    &[("workflow_id", workflow_id.as_str())],
                    &project_slug,
                )
            } else {
                path_with_project("/api/workflow-context", &project_slug)
            };
            Ok(McpHttpToolRequest {
                method: "GET",
                path,
                body: None,
            })
        }
        "pool_node_context" => {
            let path = if let Some(node_id) = mcp_optional_string_arg(&args, "node_id") {
                path_with_query(
                    "/api/node-context",
                    &[("node_id", node_id.as_str())],
                    &project_slug,
                )
            } else {
                path_with_project("/api/node-context", &project_slug)
            };
            Ok(McpHttpToolRequest {
                method: "GET",
                path,
                body: None,
            })
        }
        "pool_read_resource" => {
            let uri = mcp_required_string_arg(&args, "uri")?;
            Ok(McpHttpToolRequest {
                method: "GET",
                path: path_with_query("/api/mcp", &[("uri", uri.as_str())], &project_slug),
                body: None,
            })
        }
        "pool_provider_gateway_worker" => Ok(McpHttpToolRequest {
            method: "GET",
            path: "/api/provider-gateway-worker".to_string(),
            body: None,
        }),
        "pool_unreal_mcp_bridge" => Ok(McpHttpToolRequest {
            method: "GET",
            path: "/api/unreal-mcp-bridge".to_string(),
            body: None,
        }),
        "pool_prd_readiness" => Ok(McpHttpToolRequest {
            method: "GET",
            path: path_with_project("/api/prd-readiness", &project_slug),
            body: None,
        }),
        "pool_core_architecture_readiness" => Ok(McpHttpToolRequest {
            method: "GET",
            path: path_with_project("/api/core-architecture-readiness", &project_slug),
            body: None,
        }),
        "pool_core_architecture_gate" => {
            let path = if mcp_optional_bool_arg(&args, "require_ready")
                .or_else(|| mcp_optional_bool_arg(&args, "require_complete"))
                .unwrap_or(false)
            {
                path_with_query(
                    "/api/core-architecture-gate",
                    &[("require_ready", "true")],
                    &project_slug,
                )
            } else {
                path_with_project("/api/core-architecture-gate", &project_slug)
            };
            Ok(McpHttpToolRequest {
                method: "GET",
                path,
                body: None,
            })
        }
        "pool_core_architecture_packages" => Ok(McpHttpToolRequest {
            method: "GET",
            path: path_with_project("/api/core-architecture-packages", &project_slug),
            body: None,
        }),
        "pool_core_architecture_package" => {
            let selected_project = project_slug.clone().unwrap_or_else(|| "demo".to_string());
            Ok(McpHttpToolRequest {
                method: "POST",
                path: "/api/core-architecture-package".to_string(),
                body: Some(json!({
                    "project_slug": selected_project.clone(),
                    "node_id": mcp_optional_string_arg(&args, "node_id"),
                    "title": mcp_optional_string_arg(&args, "title")
                        .unwrap_or_else(|| "Core architecture proof package".to_string()),
                    "output_dir": mcp_optional_string_arg(&args, "output_dir")
                        .unwrap_or_else(|| format!("worlds/{selected_project}/output")),
                    "source": mcp_optional_string_arg(&args, "source")
                        .unwrap_or_else(|| "mcp-core-architecture-package".to_string()),
                    "include_snapshot": mcp_optional_bool_arg(&args, "include_snapshot").unwrap_or(true),
                })),
            })
        }
        "pool_prd_completion_gate" => {
            let path = if mcp_optional_bool_arg(&args, "require_complete").unwrap_or(false) {
                path_with_query(
                    "/api/prd-completion-gate",
                    &[("require_complete", "true")],
                    &project_slug,
                )
            } else {
                path_with_project("/api/prd-completion-gate", &project_slug)
            };
            Ok(McpHttpToolRequest {
                method: "GET",
                path,
                body: None,
            })
        }
        "pool_prd_completion_packages" => Ok(McpHttpToolRequest {
            method: "GET",
            path: path_with_project("/api/prd-completion-packages", &project_slug),
            body: None,
        }),
        "pool_production_evidence_requirements" => Ok(McpHttpToolRequest {
            method: "GET",
            path: path_with_project("/api/production-evidence/requirements", &project_slug),
            body: None,
        }),
        "pool_production_evidence_tasks" => Ok(McpHttpToolRequest {
            method: "GET",
            path: path_with_project("/api/production-evidence/tasks", &project_slug),
            body: None,
        }),
        "pool_production_evidence_task_claim" => Ok(McpHttpToolRequest {
            method: "POST",
            path: "/api/production-evidence/tasks/claim".to_string(),
            body: Some(mcp_body_with_project(args, default_project_slug)?),
        }),
        "pool_production_evidence_run_plan" => {
            let mut query = Vec::new();
            if let Some(output_root) = mcp_optional_string_arg(&args, "output_root") {
                query.push(("output_root".to_string(), output_root));
            }
            if let Some(source) = mcp_optional_string_arg(&args, "source") {
                query.push(("source".to_string(), source));
            }
            Ok(McpHttpToolRequest {
                method: "GET",
                path: path_with_owned_query(
                    "/api/production-evidence/run-plan",
                    query,
                    &project_slug,
                ),
                body: None,
            })
        }
        "pool_production_evidence_handoff" => {
            let mut query = Vec::new();
            if let Some(output_root) = mcp_optional_string_arg(&args, "output_root") {
                query.push(("output_root".to_string(), output_root));
            }
            if let Some(source) = mcp_optional_string_arg(&args, "source") {
                query.push(("source".to_string(), source));
            }
            Ok(McpHttpToolRequest {
                method: "GET",
                path: path_with_owned_query(
                    "/api/production-evidence/handoff",
                    query,
                    &project_slug,
                ),
                body: None,
            })
        }
        "pool_adapter_health" => Ok(McpHttpToolRequest {
            method: "POST",
            path: "/api/adapter-health".to_string(),
            body: Some(mcp_body_without_project(args)),
        }),
        "pool_provider_health" => Ok(McpHttpToolRequest {
            method: "POST",
            path: "/api/provider-health".to_string(),
            body: Some(mcp_body_without_project(args)),
        }),
        "pool_run_provider" => Ok(McpHttpToolRequest {
            method: "POST",
            path: "/api/provider-runs".to_string(),
            body: Some(mcp_body_with_project(args, default_project_slug)?),
        }),
        "pool_production_evidence_template" => {
            let mut query = Vec::new();
            if let Some(output_root) = mcp_optional_string_arg(&args, "output_root") {
                query.push(("output_root".to_string(), output_root));
            }
            if let Some(source) = mcp_optional_string_arg(&args, "source") {
                query.push(("source".to_string(), source));
            }
            if mcp_optional_bool_arg(&args, "missing_only").unwrap_or(false) {
                query.push(("missing_only".to_string(), "true".to_string()));
            }
            Ok(McpHttpToolRequest {
                method: "GET",
                path: path_with_owned_query(
                    "/api/production-evidence/template",
                    query,
                    &project_slug,
                ),
                body: None,
            })
        }
        "pool_production_evidence_item_template" => {
            let mut query = Vec::new();
            if let Some(output_root) = mcp_optional_string_arg(&args, "output_root") {
                query.push(("output_root".to_string(), output_root));
            }
            if let Some(source) = mcp_optional_string_arg(&args, "source") {
                query.push(("source".to_string(), source));
            }
            if let Some(task_id) = mcp_optional_string_arg(&args, "task_id") {
                query.push(("task_id".to_string(), task_id));
            }
            if let Some(kind) = mcp_optional_string_arg(&args, "kind") {
                query.push(("kind".to_string(), kind));
            }
            if let Some(target_id) = mcp_optional_string_arg(&args, "target_id") {
                query.push(("target_id".to_string(), target_id));
            }
            Ok(McpHttpToolRequest {
                method: "GET",
                path: path_with_owned_query(
                    "/api/production-evidence/item-template",
                    query,
                    &project_slug,
                ),
                body: None,
            })
        }
        "pool_production_evidence_item_from_ledger" => {
            let mut query = Vec::new();
            if let Some(source) = mcp_optional_string_arg(&args, "source") {
                query.push(("source".to_string(), source));
            }
            if let Some(provider_request_id) = mcp_optional_string_arg(&args, "provider_request_id")
            {
                query.push(("provider_request_id".to_string(), provider_request_id));
            }
            if let Some(software_action_id) = mcp_optional_string_arg(&args, "software_action_id") {
                query.push(("software_action_id".to_string(), software_action_id));
            }
            if let Some(desktop_vision_action_id) =
                mcp_optional_string_arg(&args, "desktop_vision_action_id")
            {
                query.push((
                    "desktop_vision_action_id".to_string(),
                    desktop_vision_action_id,
                ));
            }
            Ok(McpHttpToolRequest {
                method: "GET",
                path: path_with_owned_query(
                    "/api/production-evidence/item-from-ledger",
                    query,
                    &project_slug,
                ),
                body: None,
            })
        }
        "pool_production_evidence_bundle_from_ledger" => {
            let mut query = Vec::new();
            if let Some(source) = mcp_optional_string_arg(&args, "source") {
                query.push(("source".to_string(), source));
            }
            if mcp_optional_bool_arg(&args, "include_incomplete").unwrap_or(false) {
                query.push(("include_incomplete".to_string(), "true".to_string()));
            }
            Ok(McpHttpToolRequest {
                method: "GET",
                path: path_with_owned_query(
                    "/api/production-evidence/bundle-from-ledger",
                    query,
                    &project_slug,
                ),
                body: None,
            })
        }
        "pool_validate_production_evidence" => Ok(McpHttpToolRequest {
            method: "POST",
            path: "/api/production-evidence/validate".to_string(),
            body: Some(mcp_body_with_project(args, default_project_slug)?),
        }),
        "pool_merge_production_evidence" => Ok(McpHttpToolRequest {
            method: "POST",
            path: "/api/production-evidence/merge".to_string(),
            body: Some(mcp_body_with_project(args, default_project_slug)?),
        }),
        "pool_closeout_production_evidence" => Ok(McpHttpToolRequest {
            method: "POST",
            path: "/api/production-evidence/closeout".to_string(),
            body: Some(mcp_body_with_project(args, default_project_slug)?),
        }),
        "pool_import_production_evidence" => Ok(McpHttpToolRequest {
            method: "POST",
            path: "/api/production-evidence".to_string(),
            body: Some(mcp_body_with_project(args, default_project_slug)?),
        }),
        "pool_validate_production_evidence_item" => Ok(McpHttpToolRequest {
            method: "POST",
            path: "/api/production-evidence/items/validate".to_string(),
            body: Some(mcp_body_with_project(args, default_project_slug)?),
        }),
        "pool_submit_production_evidence_item" => Ok(McpHttpToolRequest {
            method: "POST",
            path: "/api/production-evidence/items".to_string(),
            body: Some(mcp_body_with_project(args, default_project_slug)?),
        }),
        "pool_provider_request_metadata" => {
            let provider_request_id = mcp_required_string_arg(&args, "provider_request_id")?;
            Ok(McpHttpToolRequest {
                method: "GET",
                path: provider_request_metadata_path(provider_request_id.as_str(), &project_slug),
                body: None,
            })
        }
        "pool_software_health" => Ok(McpHttpToolRequest {
            method: "POST",
            path: "/api/software-health".to_string(),
            body: Some(mcp_body_without_project(args)),
        }),
        "pool_run_software" => Ok(McpHttpToolRequest {
            method: "POST",
            path: "/api/software-actions".to_string(),
            body: Some(mcp_body_with_project(args, default_project_slug)?),
        }),
        "pool_run_node" => Ok(McpHttpToolRequest {
            method: "POST",
            path: "/api/nodes/run".to_string(),
            body: Some(mcp_body_with_project(args, default_project_slug)?),
        }),
        "pool_run_workflow" => Ok(McpHttpToolRequest {
            method: "POST",
            path: "/api/workflow-runs".to_string(),
            body: Some(mcp_body_with_project(args, default_project_slug)?),
        }),
        "pool_output_package" => Ok(McpHttpToolRequest {
            method: "POST",
            path: "/api/output-packages".to_string(),
            body: Some(mcp_body_with_project(args, default_project_slug)?),
        }),
        "pool_output_result" => Ok(McpHttpToolRequest {
            method: "POST",
            path: "/api/output-packages/results".to_string(),
            body: Some(mcp_body_with_project(args, default_project_slug)?),
        }),
        "pool_handoff_package" => Ok(McpHttpToolRequest {
            method: "POST",
            path: "/api/handoff-packages".to_string(),
            body: Some(mcp_body_with_project(args, default_project_slug)?),
        }),
        "pool_prd_completion_package" => Ok(McpHttpToolRequest {
            method: "POST",
            path: "/api/prd-completion-package".to_string(),
            body: Some(mcp_body_with_project(args, default_project_slug)?),
        }),
        "pool_production_evidence_handoff_packages" => Ok(McpHttpToolRequest {
            method: "GET",
            path: path_with_project("/api/production-evidence/handoff-packages", &project_slug),
            body: None,
        }),
        "pool_production_evidence_handoff_package" => Ok(McpHttpToolRequest {
            method: "POST",
            path: "/api/production-evidence/handoff-packages".to_string(),
            body: Some(mcp_body_with_project(args, default_project_slug)?),
        }),
        "pool_agent_session" => Ok(McpHttpToolRequest {
            method: "POST",
            path: "/api/agent-sessions".to_string(),
            body: Some(mcp_body_with_project(args, default_project_slug)?),
        }),
        "pool_agent_conformance_package" => Ok(McpHttpToolRequest {
            method: "POST",
            path: "/api/agent-conformance-packages".to_string(),
            body: Some(mcp_body_with_project(args, default_project_slug)?),
        }),
        "pool_agent_conformance_packages" => Ok(McpHttpToolRequest {
            method: "GET",
            path: path_with_project("/api/agent-conformance-packages", &project_slug),
            body: None,
        }),
        "pool_agent_transcript" => {
            let session_id = mcp_required_string_arg(&args, "session_id")?;
            Ok(McpHttpToolRequest {
                method: "GET",
                path: path_with_query(
                    "/api/agent-sessions/transcript",
                    &[("session_id", session_id.as_str())],
                    &project_slug,
                ),
                body: None,
            })
        }
        "pool_agent_stream" => {
            let session_id = mcp_required_string_arg(&args, "session_id")?;
            Ok(McpHttpToolRequest {
                method: "GET",
                path: agent_stream_path(
                    &AgentStreamArgs {
                        session_id,
                        after_id: mcp_optional_string_arg(&args, "after_id")
                            .or_else(|| mcp_optional_string_arg(&args, "last_event_id")),
                        limit: mcp_optional_u16_arg(&args, "limit")?,
                    },
                    &project_slug,
                ),
                body: None,
            })
        }
        "pool_desktop_requests" => Ok(McpHttpToolRequest {
            method: "GET",
            path: path_with_project("/api/desktop-recognition/requests", &project_slug),
            body: None,
        }),
        "pool_desktop_run_next" => Ok(McpHttpToolRequest {
            method: "POST",
            path: path_with_project("/api/desktop-recognition/run-next", &project_slug),
            body: Some(mcp_body_without_project(args)),
        }),
        "pool_desktop_result" => Ok(McpHttpToolRequest {
            method: "POST",
            path: "/api/desktop-recognition/results".to_string(),
            body: Some(mcp_body_without_project(args)),
        }),
        "pool_approve_task" => mcp_task_action_request("/api/tasks/approve", args, &project_slug),
        "pool_cancel_task" => mcp_task_action_request("/api/tasks/cancel", args, &project_slug),
        "pool_retry_task" => mcp_task_action_request("/api/tasks/retry", args, &project_slug),
        _ => bail!("unknown Pool MCP tool: {name}"),
    }
}

fn mcp_worker_self_checks_args(arguments: Value) -> Result<WorkerSelfChecksArgs> {
    let args = mcp_arguments_object(arguments)?;
    let output_root = mcp_optional_string_arg(&args, "output_root")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/pool-worker-self-checks"));
    let software_adapter_id = mcp_optional_string_arg(&args, "software_adapter")
        .or_else(|| mcp_optional_string_arg(&args, "software_adapter_id"))
        .or_else(|| mcp_optional_string_arg(&args, "adapter_id"))
        .unwrap_or_else(|| "resolve".to_string());

    if software_adapter_id.trim().is_empty() {
        bail!("MCP tool argument software_adapter must be non-empty");
    }

    Ok(WorkerSelfChecksArgs {
        output_root,
        software_adapter_id,
    })
}

fn mcp_task_action_request(
    path: &str,
    args: Map<String, Value>,
    project_slug: &Option<String>,
) -> Result<McpHttpToolRequest> {
    let task_id = mcp_required_string_arg(&args, "task_id")?;
    Ok(McpHttpToolRequest {
        method: "POST",
        path: path_with_query(path, &[("task_id", task_id.as_str())], project_slug),
        body: None,
    })
}

fn mcp_arguments_object(arguments: Value) -> Result<Map<String, Value>> {
    match arguments {
        Value::Null => Ok(Map::new()),
        Value::Object(object) => Ok(object),
        _ => bail!("MCP tool arguments must be a JSON object"),
    }
}

fn mcp_project_slug(
    args: &Map<String, Value>,
    default_project_slug: &Option<String>,
) -> Option<String> {
    args.get("project_slug")
        .or_else(|| args.get("project"))
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .or_else(|| default_project_slug.clone())
}

fn mcp_body_without_project(mut args: Map<String, Value>) -> Value {
    if let Some(project) = args.remove("project") {
        args.entry("project_slug".to_string()).or_insert(project);
    }
    Value::Object(args)
}

fn mcp_body_with_project(
    mut args: Map<String, Value>,
    default_project_slug: &Option<String>,
) -> Result<Value> {
    if let Some(project) = args.remove("project") {
        args.entry("project_slug".to_string()).or_insert(project);
    }
    if !args.contains_key("project_slug") {
        if let Some(project_slug) = default_project_slug
            .as_deref()
            .filter(|project_slug| *project_slug != "*")
        {
            args.insert("project_slug".to_string(), json!(project_slug));
        }
    }
    if args
        .get("project_slug")
        .and_then(Value::as_str)
        .is_some_and(|project_slug| project_slug == "*")
    {
        bail!("write tools require a concrete project_slug, not *");
    }
    Ok(Value::Object(args))
}

fn mcp_required_string_arg(args: &Map<String, Value>, key: &str) -> Result<String> {
    args.get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string)
        .with_context(|| format!("MCP tool argument {key} is required"))
}

fn mcp_optional_string_arg(args: &Map<String, Value>, key: &str) -> Option<String> {
    args.get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string)
}

fn mcp_optional_bool_arg(args: &Map<String, Value>, key: &str) -> Option<bool> {
    args.get(key).and_then(Value::as_bool)
}

fn mcp_optional_u16_arg(args: &Map<String, Value>, key: &str) -> Result<Option<u16>> {
    let Some(value) = args.get(key) else {
        return Ok(None);
    };
    let Some(value) = value.as_u64() else {
        bail!("MCP tool argument {key} must be an unsigned integer");
    };
    Ok(Some(value.try_into().context("MCP integer exceeds u16")?))
}

fn mcp_tool_definitions() -> Vec<Value> {
    vec![
        mcp_tool(
            "pool_status",
            "Read Pool runtime health, project filter, and stats.",
            json!({"type":"object","properties":mcp_project_property()}),
        ),
        mcp_tool(
            "pool_snapshot",
            "Read the complete sanitized RuntimeSnapshot.",
            json!({"type":"object","properties":mcp_project_property()}),
        ),
        mcp_tool(
            "pool_projects",
            "List local Pool projects in SQLite.",
            json!({"type":"object","properties":mcp_project_property()}),
        ),
        mcp_tool(
            "pool_events",
            "Read recent runtime events.",
            json!({
                "type":"object",
                "properties":{
                    "project_slug":{"type":"string"},
                    "after_id":{"type":"string"},
                    "limit":{"type":"integer","minimum":1,"maximum":1000}
                }
            }),
        ),
        mcp_tool(
            "pool_adapters",
            "Read Provider/software adapter catalog, aliases, control priority, and local-first policy.",
            json!({"type":"object","properties":{}}),
        ),
        mcp_tool(
            "pool_integration_readiness",
            "Read snapshot-backed Provider, software, and Agent/Hermes integration readiness matrix.",
            json!({"type":"object","properties":mcp_project_property()}),
        ),
        mcp_tool(
            "pool_software_contracts",
            "Read machine-readable software adapter control contracts.",
            json!({
                "type":"object",
                "properties":{
                    "adapter_id":{"type":"string"}
                }
            }),
        ),
        mcp_tool(
            "pool_software_conformance_packages",
            "Read generated software conformance package catalog from the asset ledger and local manifests.",
            json!({"type":"object","properties":mcp_project_property()}),
        ),
        mcp_tool(
            "pool_software_conformance_package",
            "Write a local software adapter conformance package with contract, runbook, preflight, and runner script.",
            json!({
                "type":"object",
                "required":["adapter_id"],
                "properties":{
                    "project_slug":{"type":"string"},
                    "adapter_id":{"type":"string"},
                    "node_id":{"type":"string"},
                    "title":{"type":"string"},
                    "output_dir":{"type":"string"}
                }
            }),
        ),
        mcp_tool(
            "pool_runtime_graph",
            "Read the executable workflow graph with task types and statuses.",
            json!({"type":"object","properties":mcp_project_property()}),
        ),
        mcp_tool(
            "pool_runtime_budget",
            "Read token budget, approval cost, Provider credential readiness, and request ledger summary.",
            json!({"type":"object","properties":mcp_project_property()}),
        ),
        mcp_tool(
            "pool_runtime_preflight",
            "Read run readiness, blocking checks, warnings, and suggested CLI actions.",
            json!({"type":"object","properties":mcp_project_property()}),
        ),
        mcp_tool(
            "pool_runtime_execution_plan",
            "Read ordered executable workflow steps with contracts, controls, gates, and next actions.",
            json!({"type":"object","properties":mcp_project_property()}),
        ),
        mcp_tool(
            "pool_runtime_execution_plan_run_next",
            "Preview or dispatch one selected runtime execution plan step. Set execute:true to run; approval steps also require allow_approval:true.",
            json!({
                "type":"object",
                "properties":{
                    "project_slug":{"type":"string"},
                    "node_id":{"type":"string"},
                    "task_id":{"type":"string"},
                    "execute":{"type":"boolean"},
                    "allow_approval":{"type":"boolean"},
                    "prompt":{"type":"string"},
                    "execution_mode":{"type":"string"},
                    "endpoint":{"type":"string"},
                    "input_paths":{"type":"array","items":{"type":"string"}},
                    "output_dir":{"type":"string"},
                    "duration_ms":{"type":"integer"}
                }
            }),
        ),
        mcp_tool(
            "pool_runtime_handoff",
            "Read a machine-readable Agent/Hermes/operator handoff runbook derived from runtime state.",
            json!({"type":"object","properties":mcp_project_property()}),
        ),
        mcp_tool(
            "pool_runtime_handoff_packages",
            "Read generated runtime handoff package files, operator checklist, Agent entrypoint, and MCP resources.",
            json!({"type":"object","properties":mcp_project_property()}),
        ),
        mcp_tool(
            "pool_core_architecture_readiness",
            "Read local core architecture completion gate, separated from strict production evidence closeout.",
            json!({"type":"object","properties":mcp_project_property()}),
        ),
        mcp_tool(
            "pool_core_architecture_gate",
            "Read the local core architecture hard gate. Set require_ready:true to make incomplete snapshots return HTTP 428.",
            json!({
                "type":"object",
                "properties":{
                    "project_slug":{"type":"string"},
                    "require_ready":{"type":"boolean"},
                    "require_complete":{"type":"boolean"}
                }
            }),
        ),
        mcp_tool(
            "pool_core_architecture_packages",
            "Read generated core architecture proof package files, manifest, commands, and MCP resources.",
            json!({"type":"object","properties":mcp_project_property()}),
        ),
        mcp_tool(
            "pool_core_architecture_package",
            "Write a local core architecture proof package with readiness, graph, execution plan, handoff, output catalog, strict PRD gate, manifest, and optional snapshot.",
            json!({
                "type":"object",
                "properties":{
                    "project_slug":{"type":"string"},
                    "node_id":{"type":"string"},
                    "title":{"type":"string"},
                    "output_dir":{"type":"string"},
                    "source":{"type":"string"},
                    "include_snapshot":{"type":"boolean"}
                }
            }),
        ),
        mcp_tool(
            "pool_prd_readiness",
            "Read requirement-by-requirement Pool PRD readiness, evidence, remaining gaps, and next actions.",
            json!({"type":"object","properties":mcp_project_property()}),
        ),
        mcp_tool(
            "pool_prd_completion_gate",
            "Read the PRD completion gate. Set require_complete:true to make incomplete snapshots return HTTP 428.",
            json!({
                "type":"object",
                "properties":{
                    "project_slug":{"type":"string"},
                    "require_complete":{"type":"boolean"}
                }
            }),
        ),
        mcp_tool(
            "pool_prd_completion_packages",
            "Read generated PRD completion proof package files, manifest, commands, and readiness status.",
            json!({"type":"object","properties":mcp_project_property()}),
        ),
        mcp_tool(
            "pool_production_evidence_requirements",
            "Read the machine-readable checklist for real Provider, software, and desktop-vision evidence required to close PRD production gaps.",
            json!({"type":"object","properties":mcp_project_property()}),
        ),
        mcp_tool(
            "pool_production_evidence_tasks",
            "Read the current missing production evidence task queue for external Provider workers, software operators, and desktop vision controllers.",
            json!({"type":"object","properties":mcp_project_property()}),
        ),
        mcp_tool(
            "pool_production_evidence_task_claim",
            "Claim one missing production evidence task and create a tracked runtime task plus local claim metadata.",
            json!({
                "type":"object",
                "required":["task_id"],
                "properties":{
                    "project_slug":{"type":"string"},
                    "task_id":{"type":"string"},
                    "assignee":{"type":"string"},
                    "role":{"type":"string"},
                    "output_root":{"type":"string"},
                    "source":{"type":"string"}
                }
            }),
        ),
        mcp_tool(
            "pool_production_evidence_run_plan",
            "Read an ordered real production evidence execution plan for Provider matrix, software matrix, desktop vision, merge, closeout, and completion proof.",
            json!({
                "type":"object",
                "properties":{
                    "project_slug":{"type":"string"},
                    "output_root":{"type":"string"},
                    "source":{"type":"string"}
                }
            }),
        ),
        mcp_tool(
            "pool_output_packages",
            "Read video/game/interactive-art deliverable readiness and local manifest contracts.",
            json!({"type":"object","properties":mcp_project_property()}),
        ),
        mcp_tool(
            "pool_workflow_context",
            "Read workflow context index or one workflow's graph, tasks, assets, provider requests, software actions, and Agent sessions.",
            json!({
                "type":"object",
                "properties":{
                    "project_slug":{"type":"string"},
                    "workflow_id":{"type":"string"}
                }
            }),
        ),
        mcp_tool(
            "pool_node_context",
            "Read node context index or one node's tasks, assets, provider requests, software actions, and Agent sessions.",
            json!({
                "type":"object",
                "properties":{
                    "project_slug":{"type":"string"},
                    "node_id":{"type":"string"}
                }
            }),
        ),
        mcp_tool(
            "pool_read_resource",
            "Read any pool:// MCP resource.",
            json!({
                "type":"object",
                "required":["uri"],
                "properties":{
                    "project_slug":{"type":"string"},
                    "uri":{"type":"string"}
                }
            }),
        ),
        mcp_tool(
            "pool_provider_gateway_worker",
            "Read the local Provider Gateway Worker launch, route, upstream, and Pool adapter contract.",
            json!({"type":"object","properties":{}}),
        ),
        mcp_tool(
            "pool_provider_conformance_packages",
            "Read generated Provider conformance package catalog from the asset ledger and local manifests.",
            json!({"type":"object","properties":mcp_project_property()}),
        ),
        mcp_tool(
            "pool_provider_conformance_package",
            "Write a local Provider conformance package with provider contract, gateway worker contract, runbook, preflight, and runner script.",
            json!({
                "type":"object",
                "required":["provider_id"],
                "properties":{
                    "project_slug":{"type":"string"},
                    "provider_id":{"type":"string"},
                    "node_id":{"type":"string"},
                    "title":{"type":"string"},
                    "output_dir":{"type":"string"}
                }
            }),
        ),
        mcp_tool(
            "pool_worker_self_checks",
            "Run local Provider gateway, SDK worker, Unreal, Hermes, and software bridge self-checks and return a JSON report.",
            json!({
                "type":"object",
                "properties":{
                    "output_root":{"type":"string"},
                    "software_adapter":{"type":"string"},
                    "software_adapter_id":{"type":"string"},
                    "adapter_id":{"type":"string"}
                }
            }),
        ),
        mcp_tool(
            "pool_unreal_mcp_bridge",
            "Read the Unreal plugin/gateway bridge contract for pool_unreal_action and mcp_payload.",
            json!({"type":"object","properties":{}}),
        ),
        mcp_tool(
            "pool_adapter_health",
            "Run batch Provider/software adapter health checks.",
            json!({"type":"object","properties":{"include_providers":{"type":"boolean"},"include_software":{"type":"boolean"},"providers":{"type":"array"},"software_adapters":{"type":"array"}}}),
        ),
        mcp_tool(
            "pool_provider_health",
            "Check one Provider adapter without creating a task.",
            json!({"type":"object","required":["provider_id"],"properties":{"provider_id":{"type":"string"},"execution_mode":{"type":"string"},"endpoint":{"type":"string"},"api_key":{"type":"string"}}}),
        ),
        mcp_tool(
            "pool_run_provider",
            "Run a Provider task such as ComfyUI, OpenAI image, media gateway, or 3DGS gateway/mock.",
            json!({"type":"object","required":["provider_id"],"properties":{"project_slug":{"type":"string"},"provider_id":{"type":"string"},"node_id":{"type":"string"},"task_title":{"type":"string"},"execution_mode":{"type":"string"},"endpoint":{"type":"string"},"api_key":{"type":"string"},"prompt":{"type":"string"},"input_paths":{"type":"array","items":{"type":"string"}},"output_dir":{"type":"string"},"cost_estimate_tokens":{"type":"integer"},"requires_approval":{"type":"boolean"},"evidence_json":{"type":"object"}}}),
        ),
        mcp_tool(
            "pool_production_evidence_template",
            "Read a production evidence bundle scaffold for external Provider workers, software plugins, and desktop vision controllers.",
            json!({"type":"object","properties":{"project_slug":{"type":"string"},"output_root":{"type":"string"},"source":{"type":"string"},"missing_only":{"type":"boolean"}}}),
        ),
        mcp_tool(
            "pool_production_evidence_item_template",
            "Read one production evidence item scaffold selected by task_id or kind + target_id.",
            json!({
                "type":"object",
                "properties":{
                    "project_slug":{"type":"string"},
                    "task_id":{"type":"string"},
                    "kind":{"type":"string","enum":["provider","software_action","desktop_vision"]},
                    "target_id":{"type":"string"},
                    "output_root":{"type":"string"},
                    "source":{"type":"string"}
                }
            }),
        ),
        mcp_tool(
            "pool_production_evidence_handoff",
            "Read a missing-only production evidence handoff with requirements, bundle scaffold, commands, and operator checklist.",
            json!({"type":"object","properties":{"project_slug":{"type":"string"},"output_root":{"type":"string"},"source":{"type":"string"}}}),
        ),
        mcp_tool(
            "pool_production_evidence_handoff_packages",
            "Read generated production evidence handoff package files, runner scripts, item files, and operator commands.",
            json!({"type":"object","properties":mcp_project_property()}),
        ),
        mcp_tool(
            "pool_production_evidence_item_from_ledger",
            "Build a submit-production-evidence-item draft from an existing Provider request, software action, or desktop vision ledger record.",
            json!({
                "type":"object",
                "properties":{
                    "project_slug":{"type":"string"},
                    "provider_request_id":{"type":"string"},
                    "software_action_id":{"type":"string"},
                    "desktop_vision_action_id":{"type":"string"},
                    "source":{"type":"string"}
                }
            }),
        ),
        mcp_tool(
            "pool_production_evidence_bundle_from_ledger",
            "Build a production evidence bundle from ready Provider, software, and desktop vision ledger records without writing to SQLite.",
            json!({
                "type":"object",
                "properties":{
                    "project_slug":{"type":"string"},
                    "source":{"type":"string"},
                    "include_incomplete":{"type":"boolean"}
                }
            }),
        ),
        mcp_tool(
            "pool_validate_production_evidence",
            "Validate externally completed production evidence without writing to the Pool runtime ledger.",
            mcp_production_evidence_schema(),
        ),
        mcp_tool(
            "pool_merge_production_evidence",
            "Merge multiple production evidence bundles into one bundle without writing to the Pool runtime ledger.",
            mcp_production_evidence_merge_schema(),
        ),
        mcp_tool(
            "pool_closeout_production_evidence",
            "Merge and validate production evidence bundles, then optionally import them when import is explicitly true.",
            mcp_production_evidence_closeout_schema(),
        ),
        mcp_tool(
            "pool_import_production_evidence",
            "Import externally completed production Provider, software, and desktop vision evidence into the Pool runtime ledger.",
            mcp_production_evidence_schema(),
        ),
        mcp_tool(
            "pool_validate_production_evidence_item",
            "Validate one completed production evidence item without writing to the Pool runtime ledger.",
            mcp_production_evidence_item_schema(),
        ),
        mcp_tool(
            "pool_submit_production_evidence_item",
            "Import one completed production evidence item immediately after an external worker, software operator, or desktop vision controller finishes.",
            mcp_production_evidence_item_schema(),
        ),
        mcp_tool(
            "pool_provider_request_metadata",
            "Read a registered Provider request metadata handoff file by provider_request_id.",
            json!({"type":"object","required":["provider_request_id"],"properties":{"project_slug":{"type":"string"},"provider_request_id":{"type":"string"}}}),
        ),
        mcp_tool(
            "pool_software_health",
            "Check one external software adapter without creating a software action.",
            json!({"type":"object","required":["adapter_id"],"properties":{"adapter_id":{"type":"string"},"priority":{"type":"string"},"payload_json":{"type":"object"}}}),
        ),
        mcp_tool(
            "pool_run_software",
            "Run or stage an external software control action through API/MCP, CLI, desktop recognition, or human takeover.",
            json!({"type":"object","required":["adapter_id"],"properties":{"project_slug":{"type":"string"},"adapter_id":{"type":"string"},"node_id":{"type":"string"},"task_title":{"type":"string"},"action_kind":{"type":"string"},"priority":{"type":"string"},"payload_json":{"type":"object"},"evidence_json":{"type":"object"},"requires_confirmation":{"type":"boolean"}}}),
        ),
        mcp_tool(
            "pool_run_node",
            "Run a workflow node by node_id.",
            json!({"type":"object","required":["node_id"],"properties":{"project_slug":{"type":"string"},"node_id":{"type":"string"},"prompt":{"type":"string"},"execution_mode":{"type":"string"},"endpoint":{"type":"string"},"api_key":{"type":"string"},"input_paths":{"type":"array","items":{"type":"string"}},"output_dir":{"type":"string"},"duration_ms":{"type":"integer"}}}),
        ),
        mcp_tool(
            "pool_run_workflow",
            "Run the local content-burst workflow: Agent/Hermes, 3DGS, Unreal, and three output manifests.",
            json!({"type":"object","properties":{"project_slug":{"type":"string"},"title":{"type":"string"},"prompt":{"type":"string"},"source_inputs":{"type":"array","items":{"type":"string"}},"output_root":{"type":"string"},"duration_ms":{"type":"integer"},"agent_mode":{"type":"string"},"hermes_endpoint":{"type":"string"},"three_dgs_mode":{"type":"string"},"three_dgs_endpoint":{"type":"string"},"unreal_mode":{"type":"string"},"unreal_endpoint":{"type":"string"}}}),
        ),
        mcp_tool(
            "pool_output_package",
            "Generate video/game/interactive-art deliverable manifests.",
            json!({"type":"object","properties":{"project_slug":{"type":"string"},"node_id":{"type":"string"},"title":{"type":"string"},"output_dir":{"type":"string"},"source_assets":{"type":"array","items":{"type":"string"}},"duration_ms":{"type":"integer"}}}),
        ),
        mcp_tool(
            "pool_output_result",
            "Record Resolve/Unreal/TouchDesigner/MadMapper deliverable execution results back into local output manifests.",
            json!({"type":"object","required":["target","status"],"properties":{"project_slug":{"type":"string"},"node_id":{"type":"string"},"target":{"type":"string","enum":["video","game","interactive_art"]},"local_path":{"type":"string"},"status":{"type":"string"},"runtime":{"type":"string"},"adapter_id":{"type":"string"},"software_action_id":{"type":"string"},"message":{"type":"string"},"artifacts":{"type":"array","items":{"type":"string"}},"metrics":{"type":"array","items":{"type":"object","properties":{"label":{"type":"string"},"value":{"type":"string"}}}},"verification":{"type":"object"}}}),
        ),
        mcp_tool(
            "pool_handoff_package",
            "Materialize the current runtime handoff runbook into local project JSON files.",
            json!({"type":"object","properties":{"project_slug":{"type":"string"},"node_id":{"type":"string"},"title":{"type":"string"},"output_dir":{"type":"string"},"include_snapshot":{"type":"boolean"}}}),
        ),
        mcp_tool(
            "pool_prd_completion_package",
            "Materialize the current PRD readiness, completion gate, production evidence requirements, manifest, and optional snapshot into a local proof package.",
            json!({"type":"object","properties":{"project_slug":{"type":"string"},"node_id":{"type":"string"},"title":{"type":"string"},"output_dir":{"type":"string"},"source":{"type":"string"},"include_snapshot":{"type":"boolean"}}}),
        ),
        mcp_tool(
            "pool_production_evidence_handoff_package",
            "Materialize production evidence requirements, task queue, item templates, and bundle handoff into local JSON files.",
            json!({"type":"object","properties":{"project_slug":{"type":"string"},"node_id":{"type":"string"},"title":{"type":"string"},"output_dir":{"type":"string"},"output_root":{"type":"string"},"source":{"type":"string"},"include_items":{"type":"boolean"},"include_snapshot":{"type":"boolean"}}}),
        ),
        mcp_tool(
            "pool_agent_session",
            "Stage or execute Hermes / Agent CLI control sessions.",
            json!({"type":"object","required":["kind"],"properties":{"project_slug":{"type":"string"},"kind":{"type":"string","enum":["hermes","agent_cli"]},"control_dir":{"type":"string"},"endpoint":{"type":"string"},"instruction":{"type":"string"},"allowed_tools":{"type":"array","items":{"type":"string"}},"requires_confirmation":{"type":"boolean"},"command_id":{"type":"string"},"title":{"type":"string"},"command":{"type":"string"},"tools":{"type":"array","items":{"type":"string"}},"token_budget":{"type":"integer"},"execute":{"type":"boolean"},"allowed_commands":{"type":"array","items":{"type":"string"}},"working_dir":{"type":"string"},"max_output_bytes":{"type":"integer"},"timeout_ms":{"type":"integer"}}}),
        ),
        mcp_tool(
            "pool_agent_conformance_package",
            "Write a local Agent/Hermes conformance package with session contract, runbook, preflight, and runner script.",
            json!({"type":"object","properties":{"project_slug":{"type":"string"},"kind":{"type":"string","enum":["all","hermes","agent-cli"]},"node_id":{"type":"string"},"title":{"type":"string"},"output_dir":{"type":"string"}}}),
        ),
        mcp_tool(
            "pool_agent_conformance_packages",
            "Read generated Agent/Hermes conformance package catalog from the asset ledger and local manifests.",
            json!({"type":"object","properties":mcp_project_property()}),
        ),
        mcp_tool(
            "pool_integration_conformance_packages",
            "Read generated integration conformance package catalog from the asset ledger and local manifests.",
            json!({"type":"object","properties":mcp_project_property()}),
        ),
        mcp_tool(
            "pool_integration_conformance_package",
            "Write a local Provider + software + Agent/Hermes integration conformance package.",
            json!({"type":"object","properties":{"project_slug":{"type":"string"},"node_id":{"type":"string"},"title":{"type":"string"},"output_dir":{"type":"string"},"providers":{"type":"array","items":{"type":"string"}},"software_adapters":{"type":"array","items":{"type":"string"}},"agent_kind":{"type":"string","enum":["all","hermes","agent-cli"]},"include_providers":{"type":"boolean"},"include_software":{"type":"boolean"},"include_agent":{"type":"boolean"}}}),
        ),
        mcp_tool(
            "pool_agent_transcript",
            "Read a registered Hermes / Agent CLI transcript by agent session id.",
            json!({"type":"object","required":["session_id"],"properties":{"project_slug":{"type":"string"},"session_id":{"type":"string"}}}),
        ),
        mcp_tool(
            "pool_agent_stream",
            "Read the Agent/Hermes session SSE slice: transcript plus related runtime events.",
            json!({"type":"object","required":["session_id"],"properties":{"project_slug":{"type":"string"},"session_id":{"type":"string"},"after_id":{"type":"string"},"last_event_id":{"type":"string"},"limit":{"type":"integer","minimum":1,"maximum":200}}}),
        ),
        mcp_tool(
            "pool_desktop_requests",
            "Read desktop recognition requests waiting for an external controller.",
            json!({"type":"object","properties":mcp_project_property()}),
        ),
        mcp_tool(
            "pool_desktop_run_next",
            "Dry-run the next queued desktop recognition requests through the Runtime HTTP controller endpoint.",
            json!({"type":"object","properties":{"project_slug":{"type":"string"},"status":{"type":"string"},"message":{"type":"string"},"controller_id":{"type":"string"},"limit":{"type":"integer","minimum":1},"artifacts":{"type":"array","items":{"type":"string"}},"screen_trace_path":{"type":"string"}}}),
        ),
        mcp_tool(
            "pool_desktop_result",
            "Return desktop recognition controller status to Pool.",
            json!({"type":"object","required":["software_action_id"],"properties":{"software_action_id":{"type":"string"},"task_id":{"type":"string"},"status":{"type":"string"},"message":{"type":"string"},"artifacts":{"type":"array","items":{"type":"string"}},"screen_trace_path":{"type":"string"},"result":{"type":"object"},"verification":{"type":"object"}}}),
        ),
        mcp_tool(
            "pool_approve_task",
            "Approve a waiting task.",
            mcp_task_action_schema(),
        ),
        mcp_tool(
            "pool_cancel_task",
            "Cancel a task.",
            mcp_task_action_schema(),
        ),
        mcp_tool(
            "pool_retry_task",
            "Retry a cancelled, failed, or retryable task.",
            mcp_task_action_schema(),
        ),
    ]
}

fn mcp_tool(name: &str, description: &str, input_schema: Value) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": input_schema,
    })
}

fn mcp_project_property() -> Value {
    json!({
        "project_slug": {
            "type": "string",
            "description": "Optional Pool project slug. Use * for read-only all-project views."
        }
    })
}

fn mcp_production_evidence_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "project_slug": {"type": "string"},
            "source": {"type": "string"},
            "providers": {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": ["provider_id", "external_job_id", "production_attestation", "artifacts"],
                    "properties": {
                        "provider_id": {"type": "string"},
                        "external_job_id": {"type": "string"},
                        "production_attestation": {"type": "string"},
                        "endpoint": {"type": "string"},
                        "family": {"type": "string"},
                        "node_id": {"type": "string"},
                        "task_title": {"type": "string"},
                        "metadata_path": {"type": "string"},
                        "artifacts": {"type": "array", "items": {"type": "string"}},
                        "evidence_json": {"type": "object"},
                        "response_json": {"type": "object"}
                    }
                }
            },
            "software_actions": {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": ["adapter_id", "external_action_id", "production_attestation"],
                    "properties": {
                        "adapter_id": {"type": "string"},
                        "external_action_id": {"type": "string"},
                        "production_attestation": {"type": "string"},
                        "action_kind": {"type": "string"},
                        "priority": {"type": "string"},
                        "control_profile": {"type": "string"},
                        "node_id": {"type": "string"},
                        "task_title": {"type": "string"},
                        "artifacts": {"type": "array", "items": {"type": "string"}},
                        "evidence_json": {"type": "object"},
                        "verification_json": {"type": "object"}
                    }
                }
            },
            "desktop_vision": {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": ["external_action_id", "controller_id", "production_attestation", "trace_path", "visual_model"],
                    "properties": {
                        "adapter_id": {"type": "string"},
                        "external_action_id": {"type": "string"},
                        "controller_id": {"type": "string"},
                        "production_attestation": {"type": "string"},
                        "trace_path": {"type": "string"},
                        "visual_model": {
                            "type": "string",
                            "const": "external",
                            "description": "Must identify a real external visual/OCR/screen model; local mock or dry-run traces are rejected."
                        },
                        "node_id": {"type": "string"},
                        "task_title": {"type": "string"},
                        "artifacts": {"type": "array", "items": {"type": "string"}},
                        "evidence_json": {"type": "object"},
                        "verification_json": {"type": "object"}
                    }
                }
            }
        }
    })
}

fn mcp_production_evidence_merge_schema() -> Value {
    let bundle_schema = mcp_production_evidence_schema();
    json!({
        "type": "object",
        "required": ["bundles"],
        "properties": {
            "project_slug": {"type": "string"},
            "source": {"type": "string"},
            "bundles": {
                "type": "array",
                "items": bundle_schema
            }
        }
    })
}

fn mcp_production_evidence_closeout_schema() -> Value {
    let bundle_schema = mcp_production_evidence_schema();
    json!({
        "type": "object",
        "required": ["bundles"],
        "properties": {
            "project_slug": {"type": "string"},
            "source": {"type": "string"},
            "import": {
                "type": "boolean",
                "description": "Defaults to false. When true, closeout imports the merged bundle after validation."
            },
            "completion_package": {
                "type": "object",
                "description": "Optional. When import:true succeeds and the PRD completion gate is ready, write a local PRD completion proof package.",
                "properties": {
                    "node_id": {"type": "string"},
                    "title": {"type": "string"},
                    "output_dir": {"type": "string"},
                    "source": {"type": "string"},
                    "include_snapshot": {"type": "boolean"}
                }
            },
            "bundles": {
                "type": "array",
                "items": bundle_schema
            }
        }
    })
}

fn mcp_production_evidence_item_schema() -> Value {
    json!({
        "type": "object",
        "required": ["kind"],
        "properties": {
            "project_slug": {"type": "string"},
            "source": {"type": "string"},
            "kind": {
                "type": "string",
                "enum": ["provider", "software_action", "desktop_vision"]
            },
            "provider": {
                "type": "object",
                "required": ["provider_id", "external_job_id", "production_attestation", "artifacts"],
                "properties": {
                    "provider_id": {"type": "string"},
                    "external_job_id": {"type": "string"},
                    "production_attestation": {"type": "string"},
                    "endpoint": {"type": "string"},
                    "family": {"type": "string"},
                    "node_id": {"type": "string"},
                    "task_title": {"type": "string"},
                    "metadata_path": {"type": "string"},
                    "artifacts": {"type": "array", "items": {"type": "string"}},
                    "evidence_json": {"type": "object"},
                    "response_json": {"type": "object"}
                }
            },
            "software_action": {
                "type": "object",
                "required": ["adapter_id", "external_action_id", "production_attestation"],
                "properties": {
                    "adapter_id": {"type": "string"},
                    "external_action_id": {"type": "string"},
                    "production_attestation": {"type": "string"},
                    "action_kind": {"type": "string"},
                    "priority": {"type": "string"},
                    "control_profile": {"type": "string"},
                    "node_id": {"type": "string"},
                    "task_title": {"type": "string"},
                    "artifacts": {"type": "array", "items": {"type": "string"}},
                    "evidence_json": {"type": "object"},
                    "verification_json": {"type": "object"}
                }
            },
            "desktop_vision": {
                "type": "object",
                "required": ["external_action_id", "controller_id", "production_attestation", "trace_path", "visual_model"],
                "properties": {
                    "adapter_id": {"type": "string"},
                    "external_action_id": {"type": "string"},
                    "controller_id": {"type": "string"},
                    "production_attestation": {"type": "string"},
                    "trace_path": {"type": "string"},
                    "visual_model": {
                        "type": "string",
                        "const": "external",
                        "description": "Must identify a real external visual/OCR/screen model; local mock or dry-run traces are rejected."
                    },
                    "node_id": {"type": "string"},
                    "task_title": {"type": "string"},
                    "artifacts": {"type": "array", "items": {"type": "string"}},
                    "evidence_json": {"type": "object"},
                    "verification_json": {"type": "object"}
                }
            }
        }
    })
}

fn mcp_task_action_schema() -> Value {
    json!({
        "type":"object",
        "required":["task_id"],
        "properties":{
            "project_slug":{"type":"string"},
            "task_id":{"type":"string"}
        }
    })
}

fn project_body(project_slug: &Option<String>) -> Map<String, Value> {
    let mut body = Map::new();
    if let Some(project_slug) = project_slug.as_deref().filter(|project| *project != "*") {
        body.insert(
            "project_slug".to_string(),
            Value::String(project_slug.to_string()),
        );
    }
    body
}

fn run_node_body(project_slug: &Option<String>, args: RunNodeArgs) -> Value {
    let mut body = project_body(project_slug);
    body.insert("node_id".to_string(), Value::String(args.node_id));
    insert_optional(&mut body, "prompt", args.prompt);
    insert_optional(&mut body, "execution_mode", args.execution_mode);
    insert_optional(&mut body, "endpoint", args.endpoint);
    insert_optional(&mut body, "api_key", args.api_key);
    if !args.input_paths.is_empty() {
        body.insert("input_paths".to_string(), json!(args.input_paths));
    }
    insert_optional(&mut body, "output_dir", args.output_dir);
    if let Some(duration_ms) = args.duration_ms {
        body.insert("duration_ms".to_string(), json!(duration_ms));
    }
    Value::Object(body)
}

fn runtime_run_next_body(project_slug: &Option<String>, args: RuntimeRunNextArgs) -> Value {
    let mut body = project_body(project_slug);
    insert_optional(&mut body, "node_id", args.node_id);
    insert_optional(&mut body, "task_id", args.task_id);
    body.insert("execute".to_string(), json!(args.execute));
    body.insert("allow_approval".to_string(), json!(args.allow_approval));
    insert_optional(&mut body, "prompt", args.prompt);
    insert_optional(&mut body, "execution_mode", args.execution_mode);
    insert_optional(&mut body, "endpoint", args.endpoint);
    insert_optional(&mut body, "api_key", args.api_key);
    if !args.input_paths.is_empty() {
        body.insert("input_paths".to_string(), json!(args.input_paths));
    }
    insert_optional(&mut body, "output_dir", args.output_dir);
    if let Some(duration_ms) = args.duration_ms {
        body.insert("duration_ms".to_string(), json!(duration_ms));
    }
    Value::Object(body)
}

fn adapter_health_body(args: AdapterHealthArgs) -> Value {
    let mut body = Map::new();
    if let Some(include_providers) = args.include_providers {
        body.insert("include_providers".to_string(), json!(include_providers));
    }
    if let Some(include_software) = args.include_software {
        body.insert("include_software".to_string(), json!(include_software));
    }
    Value::Object(body)
}

fn provider_health_body(args: ProviderHealthArgs) -> Value {
    let mut body = Map::new();
    body.insert("provider_id".to_string(), Value::String(args.provider_id));
    insert_optional(&mut body, "execution_mode", args.execution_mode);
    insert_optional(&mut body, "endpoint", args.endpoint);
    insert_optional(&mut body, "api_key", args.api_key);
    Value::Object(body)
}

fn provider_run_body(project_slug: &Option<String>, args: ProviderRunArgs) -> Value {
    let mut body = project_body(project_slug);
    body.insert("provider_id".to_string(), Value::String(args.provider_id));
    insert_optional(&mut body, "node_id", args.node_id);
    insert_optional(&mut body, "task_title", args.task_title);
    insert_optional(&mut body, "execution_mode", args.execution_mode);
    insert_optional(&mut body, "endpoint", args.endpoint);
    insert_optional(&mut body, "api_key", args.api_key);
    insert_optional(&mut body, "prompt", args.prompt);
    if !args.input_paths.is_empty() {
        body.insert("input_paths".to_string(), json!(args.input_paths));
    }
    insert_optional(&mut body, "output_dir", args.output_dir);
    if let Some(cost_estimate_tokens) = args.cost_estimate_tokens {
        body.insert(
            "cost_estimate_tokens".to_string(),
            json!(cost_estimate_tokens),
        );
    }
    if let Some(requires_approval) = args.requires_approval {
        body.insert("requires_approval".to_string(), json!(requires_approval));
    }
    insert_optional_value(&mut body, "evidence_json", args.evidence_json);
    Value::Object(body)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CliProviderEvidenceFamily {
    Media,
    OpenAiImage,
    ThreeDgs,
}

#[derive(Debug, Clone, Copy)]
struct CliProviderEvidenceTarget {
    provider_id: &'static str,
    family: CliProviderEvidenceFamily,
}

const CLI_PROVIDER_EVIDENCE_TARGETS: &[CliProviderEvidenceTarget] = &[
    CliProviderEvidenceTarget {
        provider_id: "midjourney",
        family: CliProviderEvidenceFamily::Media,
    },
    CliProviderEvidenceTarget {
        provider_id: "openai-image-2",
        family: CliProviderEvidenceFamily::OpenAiImage,
    },
    CliProviderEvidenceTarget {
        provider_id: "nano-banana-pro",
        family: CliProviderEvidenceFamily::Media,
    },
    CliProviderEvidenceTarget {
        provider_id: "suno",
        family: CliProviderEvidenceFamily::Media,
    },
    CliProviderEvidenceTarget {
        provider_id: "worldlabs-marble",
        family: CliProviderEvidenceFamily::ThreeDgs,
    },
    CliProviderEvidenceTarget {
        provider_id: "tripo-splat",
        family: CliProviderEvidenceFamily::ThreeDgs,
    },
    CliProviderEvidenceTarget {
        provider_id: "sam-3d",
        family: CliProviderEvidenceFamily::ThreeDgs,
    },
    CliProviderEvidenceTarget {
        provider_id: "spark-3dgs",
        family: CliProviderEvidenceFamily::ThreeDgs,
    },
    CliProviderEvidenceTarget {
        provider_id: "qunhe-3d",
        family: CliProviderEvidenceFamily::ThreeDgs,
    },
];

fn production_evidence_provider_matrix_response(
    server: &RuntimeHttpServer,
    project_slug: &Option<String>,
    args: ProviderEvidenceProviderMatrixArgs,
) -> Result<RuntimeHttpResponse> {
    let project_slug = concrete_project_slug(project_slug).unwrap_or("demo");
    let output_root = PathBuf::from(args.output_root.clone().unwrap_or_else(|| {
        format!("worlds/{project_slug}/output/production-evidence/provider-matrix")
    }));
    fs::create_dir_all(&output_root).with_context(|| {
        format!(
            "create provider evidence output root {}",
            output_root.display()
        )
    })?;
    let evidence_bundle_path = args
        .evidence_bundle_path
        .clone()
        .map(PathBuf::from)
        .unwrap_or_else(|| output_root.join("provider-production-evidence-bundle.json"));

    let media_endpoint = configured_string(
        args.media_endpoint.clone(),
        args.use_env
            .then(|| env::var("POOL_MEDIA_GATEWAY_ENDPOINT").ok())
            .flatten(),
    );
    let three_dgs_endpoint = configured_string(
        args.three_dgs_endpoint.clone(),
        args.use_env
            .then(|| env::var("POOL_3DGS_GATEWAY_ENDPOINT").ok())
            .flatten(),
    );
    let openai_endpoint = configured_string(
        args.openai_endpoint.clone(),
        args.use_env
            .then(|| env::var("POOL_OPENAI_ENDPOINT").ok())
            .flatten(),
    )
    .unwrap_or_else(|| "https://api.openai.com/v1".to_string());
    let openai_api_key = configured_string(
        args.openai_api_key.clone(),
        args.use_env
            .then(|| env::var("OPENAI_API_KEY").ok())
            .flatten(),
    );
    let global_production_attestation = if args.production_upstream {
        configured_string(
            args.production_attestation.clone(),
            args.use_env
                .then(|| env::var("POOL_PROVIDER_PRODUCTION_ATTESTATION").ok())
                .flatten(),
        )
    } else {
        None
    };

    let mut results = Vec::new();
    let mut provider_items = Vec::new();
    let mut succeeded = 0_usize;
    let mut failed = 0_usize;
    let mut skipped = 0_usize;

    for target in CLI_PROVIDER_EVIDENCE_TARGETS {
        let provider_endpoint = provider_matrix_provider_endpoint(
            &args.provider_endpoints,
            target.provider_id,
            args.use_env,
        );
        let endpoint_source;
        let endpoint = if let Some(endpoint) = provider_endpoint.as_deref() {
            endpoint_source = "provider";
            Some(endpoint)
        } else {
            endpoint_source = match target.family {
                CliProviderEvidenceFamily::Media => "media",
                CliProviderEvidenceFamily::OpenAiImage => "openai",
                CliProviderEvidenceFamily::ThreeDgs => "3dgs",
            };
            match target.family {
                CliProviderEvidenceFamily::Media => media_endpoint.as_deref(),
                CliProviderEvidenceFamily::OpenAiImage => Some(openai_endpoint.as_str()),
                CliProviderEvidenceFamily::ThreeDgs => three_dgs_endpoint.as_deref(),
            }
        };
        let provider_api_key = provider_matrix_provider_api_key(
            &args.provider_api_keys,
            target.provider_id,
            args.use_env,
        );
        let resolved_api_key = provider_api_key.as_deref().or_else(|| {
            (target.family == CliProviderEvidenceFamily::OpenAiImage)
                .then_some(openai_api_key.as_deref())
                .flatten()
        });
        let api_key_source = if provider_api_key.is_some() {
            Some("provider")
        } else if target.family == CliProviderEvidenceFamily::OpenAiImage
            && openai_api_key
                .as_deref()
                .is_some_and(|key| !key.trim().is_empty())
        {
            Some("openai")
        } else {
            None
        };
        if target.family == CliProviderEvidenceFamily::OpenAiImage
            && resolved_api_key
                .as_deref()
                .map(str::trim)
                .unwrap_or("")
                .is_empty()
        {
            skipped += 1;
            results.push(provider_matrix_result(
                target,
                "skipped",
                "missing_openai_api_key",
                None,
            ));
            continue;
        }
        let Some(endpoint) = endpoint else {
            skipped += 1;
            results.push(provider_matrix_result(
                target,
                "skipped",
                "missing_endpoint",
                None,
            ));
            continue;
        };
        let production_attestation = if args.production_upstream {
            let attestation = provider_matrix_provider_attestation(
                &args.provider_attestations,
                target.provider_id,
                args.use_env,
                global_production_attestation.as_deref(),
            )?;
            if attestation.is_none() {
                failed += 1;
                results.push(provider_matrix_result(
                    target,
                    "failed",
                    "missing_production_attestation",
                    None,
                ));
                continue;
            }
            attestation
        } else {
            None
        };

        let provider_output_dir = output_root.join("providers").join(target.provider_id);
        let body = provider_matrix_run_body(
            project_slug,
            target,
            endpoint,
            endpoint_source,
            &provider_output_dir,
            resolved_api_key,
            api_key_source,
            production_attestation.as_deref(),
        );
        let response =
            server.handle_request_with_body("POST", "/api/provider-runs", &body.to_string())?;
        let value: Value = serde_json::from_str(&response.body).with_context(|| {
            format!("parse provider matrix response for {}", target.provider_id)
        })?;
        let status = value
            .pointer("/report/status")
            .and_then(Value::as_str)
            .or_else(|| value.get("error").and_then(Value::as_str))
            .unwrap_or("unknown")
            .to_string();
        if response.status_code < 400 && status == "Succeeded" {
            if let Some(attestation) = production_attestation.as_deref() {
                match provider_matrix_evidence_item(
                    target,
                    endpoint,
                    &body,
                    &value,
                    &provider_output_dir,
                    attestation,
                ) {
                    Ok(item) => {
                        succeeded += 1;
                        provider_items.push(item);
                        results.push(provider_matrix_result(
                            target,
                            "succeeded",
                            &status,
                            Some(&value),
                        ));
                    }
                    Err(error) => {
                        failed += 1;
                        results.push(provider_matrix_result(
                            target,
                            "failed",
                            &format!("production_evidence_item_failed: {error:#}"),
                            Some(&value),
                        ));
                    }
                }
            } else {
                succeeded += 1;
                results.push(provider_matrix_result(
                    target,
                    "succeeded",
                    &status,
                    Some(&value),
                ));
            }
        } else {
            failed += 1;
            results.push(provider_matrix_result(
                target,
                "failed",
                &status,
                Some(&value),
            ));
        }
    }

    let evidence_bundle = json!({
        "source": "pool-cli production-evidence-provider-matrix",
        "project_slug": project_slug,
        "providers": provider_items,
        "software_actions": [],
        "desktop_vision": [],
    });
    write_json_value(&evidence_bundle_path, &evidence_bundle)?;

    RuntimeHttpResponse::json(
        200,
        json!({
            "kind": "pool_provider_production_evidence_matrix",
            "project_slug": project_slug,
            "output_root": output_root,
            "evidence_bundle_path": evidence_bundle_path,
            "summary": {
                "total": CLI_PROVIDER_EVIDENCE_TARGETS.len(),
                "succeeded": succeeded,
                "failed": failed,
                "skipped": skipped,
                "production_upstream": args.production_upstream,
                "production_evidence_items": evidence_bundle["providers"].as_array().map_or(0, Vec::len),
            },
            "results": results,
            "bundle": evidence_bundle,
            "commands": {
                "validate": format!("pool-cli --project {project_slug} validate-production-evidence {}", evidence_bundle_path.display()),
                "closeout": format!("pool-cli --project {project_slug} closeout-production-evidence --output <merged-bundle.json> {}", evidence_bundle_path.display()),
            }
        }),
    )
}

fn concrete_project_slug(project_slug: &Option<String>) -> Option<&str> {
    project_slug
        .as_deref()
        .filter(|project| *project != "*" && !project.trim().is_empty())
}

fn first_configured<'a>(primary: Option<&'a str>, fallback: Option<&'a str>) -> Option<&'a str> {
    primary
        .filter(|value| !value.trim().is_empty())
        .or_else(|| fallback.filter(|value| !value.trim().is_empty()))
}

fn configured_string(primary: Option<String>, fallback: Option<String>) -> Option<String> {
    first_configured(primary.as_deref(), fallback.as_deref()).map(ToString::to_string)
}

fn provider_matrix_provider_endpoint(
    provider_endpoints: &[(String, String)],
    provider_id: &str,
    use_env: bool,
) -> Option<String> {
    let route_key = provider_matrix_provider_route_key(provider_id);
    provider_endpoints
        .iter()
        .rev()
        .find(|(candidate, endpoint)| {
            provider_matrix_provider_route_key(candidate) == route_key
                && !endpoint.trim().is_empty()
        })
        .map(|(_, endpoint)| endpoint.clone())
        .or_else(|| {
            if !use_env {
                return None;
            }
            provider_matrix_provider_endpoint_env_candidates(&route_key)
                .into_iter()
                .find_map(|name| configured_string(None, env::var(name).ok()))
        })
}

fn provider_matrix_provider_endpoint_env_candidates(provider_route_key: &str) -> Vec<String> {
    let env_key = provider_matrix_env_key(provider_route_key);
    vec![
        format!("POOL_PROVIDER_ENDPOINT_{env_key}"),
        format!("POOL_{env_key}_ENDPOINT"),
    ]
}

fn provider_matrix_provider_api_key(
    provider_api_keys: &[(String, String)],
    provider_id: &str,
    use_env: bool,
) -> Option<String> {
    let route_key = provider_matrix_provider_route_key(provider_id);
    provider_api_keys
        .iter()
        .rev()
        .find(|(candidate, api_key)| {
            provider_matrix_provider_route_key(candidate) == route_key && !api_key.trim().is_empty()
        })
        .map(|(_, api_key)| api_key.clone())
        .or_else(|| {
            if !use_env {
                return None;
            }
            provider_matrix_provider_api_key_env_candidates(&route_key)
                .into_iter()
                .find_map(|name| configured_string(None, env::var(name).ok()))
        })
}

fn provider_matrix_provider_api_key_env_candidates(provider_route_key: &str) -> Vec<String> {
    let env_key = provider_matrix_env_key(provider_route_key);
    vec![
        format!("POOL_PROVIDER_API_KEY_{env_key}"),
        format!("POOL_{env_key}_API_KEY"),
    ]
}

fn provider_matrix_provider_attestation(
    provider_attestations: &[(String, String)],
    provider_id: &str,
    use_env: bool,
    global_attestation: Option<&str>,
) -> Result<Option<String>> {
    let route_key = provider_matrix_provider_route_key(provider_id);
    let value = provider_attestations
        .iter()
        .rev()
        .find(|(candidate, attestation)| {
            provider_matrix_provider_route_key(candidate) == route_key
                && !attestation.trim().is_empty()
        })
        .map(|(_, attestation)| attestation.clone())
        .or_else(|| {
            if !use_env {
                return None;
            }
            provider_matrix_provider_attestation_env_candidates(&route_key)
                .into_iter()
                .find_map(|name| configured_string(None, env::var(name).ok()))
        })
        .or_else(|| global_attestation.map(ToString::to_string));

    value
        .as_deref()
        .map(|attestation| validated_provider_matrix_attestation(Some(attestation)))
        .transpose()
}

fn provider_matrix_provider_attestation_env_candidates(provider_route_key: &str) -> Vec<String> {
    let env_key = provider_matrix_env_key(provider_route_key);
    vec![
        format!("POOL_PROVIDER_PRODUCTION_ATTESTATION_{env_key}"),
        format!("POOL_{env_key}_PRODUCTION_ATTESTATION"),
    ]
}

fn provider_matrix_env_key(provider_route_key: &str) -> String {
    provider_route_key
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect::<String>()
}

fn provider_matrix_provider_route_key(provider_id: &str) -> String {
    match provider_id
        .trim()
        .to_ascii_lowercase()
        .replace(['_', ' '], "-")
        .as_str()
    {
        "nano-banana" | "nanobanana" | "nanobananapro" | "nano-banana-pro" => {
            "nano-banana-pro".to_string()
        }
        "openai" | "openai-image" | "openai-image-2" | "image-2" => "openai-image-2".to_string(),
        "tripo" | "triposplat" | "tripo-splat" => "tripo-splat".to_string(),
        "sam3d" | "sam-3d" => "sam-3d".to_string(),
        "spark" | "spark-3d" | "spark-3dgs" => "spark-3dgs".to_string(),
        "qunhe" | "qunhe-3d" | "qunhe-tech" => "qunhe-3d".to_string(),
        "worldlabs" | "world-labs" | "worldlabs-marble" | "world-labs-marble" | "marble" => {
            "worldlabs-marble".to_string()
        }
        value => value.to_string(),
    }
}

fn provider_matrix_run_body(
    project_slug: &str,
    target: &CliProviderEvidenceTarget,
    endpoint: &str,
    endpoint_source: &str,
    output_dir: &Path,
    api_key: Option<&str>,
    api_key_source: Option<&str>,
    production_attestation: Option<&str>,
) -> Value {
    json!({
        "project_slug": project_slug,
        "provider_id": target.provider_id,
        "execution_mode": match target.family {
            CliProviderEvidenceFamily::OpenAiImage => "adapter",
            _ => "gateway",
        },
        "endpoint": endpoint,
        "api_key": api_key,
        "task_title": format!("{} provider evidence", target.provider_id),
        "prompt": provider_matrix_prompt(target),
        "input_paths": if target.family == CliProviderEvidenceFamily::OpenAiImage {
            json!([])
        } else {
            json!([format!("worlds/{project_slug}/source/0-reference.png")])
        },
        "output_dir": output_dir.to_string_lossy(),
        "requires_approval": false,
        "evidence_json": {
            "source": "pool-cli production-evidence-provider-matrix",
            "family": provider_matrix_family(target),
            "evidence_mode": if target.family == CliProviderEvidenceFamily::OpenAiImage { "native_api" } else { "configured_gateway" },
            "production_upstream": production_attestation.is_some(),
            "local_mock_gateway": false,
            "endpoint_source": endpoint_source,
            "api_key_source": api_key_source,
            "production_attestation": production_attestation,
        }
    })
}

fn provider_matrix_prompt(target: &CliProviderEvidenceTarget) -> String {
    match target.family {
        CliProviderEvidenceFamily::Media => format!(
            "Pool provider evidence run for {}. Generate one local media output for audit.",
            target.provider_id
        ),
        CliProviderEvidenceFamily::OpenAiImage => json!({
            "prompt": "Pool provider evidence run for OpenAI image-2. Generate one local audit image.",
            "size": "1024x1024",
            "quality": "medium",
            "output_format": "png"
        })
        .to_string(),
        CliProviderEvidenceFamily::ThreeDgs => format!(
            "Pool provider evidence run for {}. Convert reference input to image-blaster indexed 3DGS outputs.",
            target.provider_id
        ),
    }
}

fn provider_matrix_family(target: &CliProviderEvidenceTarget) -> &'static str {
    match target.family {
        CliProviderEvidenceFamily::Media => "ai_media",
        CliProviderEvidenceFamily::OpenAiImage => "ai_image",
        CliProviderEvidenceFamily::ThreeDgs => "3dgs",
    }
}

fn provider_matrix_result(
    target: &CliProviderEvidenceTarget,
    status: &str,
    reason: &str,
    response: Option<&Value>,
) -> Value {
    json!({
        "provider_id": target.provider_id,
        "family": provider_matrix_family(target),
        "status": status,
        "reason": reason,
        "provider_request_id": response
            .and_then(|value| value.get("provider_request_id"))
            .and_then(Value::as_str),
        "http_status": response
            .and_then(|value| value.get("status_code"))
            .and_then(Value::as_u64),
    })
}

fn provider_matrix_evidence_item(
    target: &CliProviderEvidenceTarget,
    endpoint: &str,
    request_body: &Value,
    response: &Value,
    output_dir: &Path,
    production_attestation: &str,
) -> Result<Value> {
    let local_artifacts = provider_matrix_response_artifacts(response);
    let (artifacts, missing_artifacts): (Vec<_>, Vec<_>) = local_artifacts
        .into_iter()
        .partition(|path| Path::new(path.as_str()).exists());
    if artifacts.is_empty() {
        bail!(
            "production evidence for {} requires at least one local artifact path",
            target.provider_id
        );
    }
    if !missing_artifacts.is_empty() {
        bail!(
            "production evidence for {} references missing local artifacts: {}",
            target.provider_id,
            missing_artifacts.join(", ")
        );
    }
    let metadata_path = output_dir.join("provider-production-metadata.json");
    let sanitized_request = provider_matrix_sanitized_request_body(request_body);
    write_json_value(
        &metadata_path,
        &json!({
            "kind": "pool_provider_production_evidence_metadata",
            "provider_id": target.provider_id,
            "endpoint": endpoint,
            "request": sanitized_request,
            "response": response,
            "production_attestation": production_attestation,
            "artifact_policy": {
                "local_files_authoritative": true,
                "provider_urls_are_provenance": true
            }
        }),
    )?;

    Ok(json!({
        "provider_id": target.provider_id,
        "external_job_id": provider_matrix_external_job_id(response, target.provider_id),
        "endpoint": endpoint,
        "family": provider_matrix_family(target),
        "task_title": format!("{} production upstream evidence", target.provider_id),
        "metadata_path": metadata_path.to_string_lossy(),
        "artifacts": artifacts,
        "evidence_json": {
            "source": "pool-cli production-evidence-provider-matrix",
            "evidence_mode": "production_upstream",
            "production_upstream": true,
            "local_mock_gateway": false,
            "configured_gateway": true,
            "production_attestation": production_attestation,
            "provider_request_id": response.get("provider_request_id").and_then(Value::as_str),
        },
        "response_json": response,
    }))
}

fn provider_matrix_sanitized_request_body(request_body: &Value) -> Value {
    let mut sanitized = request_body.clone();
    if let Some(object) = sanitized.as_object_mut() {
        if object
            .get("api_key")
            .and_then(Value::as_str)
            .is_some_and(|value| !value.trim().is_empty())
        {
            object.insert("api_key".to_string(), json!("[redacted]"));
        }
    }
    sanitized
}

fn provider_matrix_response_artifacts(response: &Value) -> Vec<String> {
    let mut artifacts = Vec::new();
    collect_provider_matrix_string_array(response.pointer("/report/assets"), &mut artifacts);
    collect_provider_matrix_string_array(response.pointer("/assets"), &mut artifacts);
    collect_provider_matrix_string_array(response.pointer("/report/artifacts"), &mut artifacts);
    collect_provider_matrix_string_array(response.pointer("/artifacts"), &mut artifacts);
    if let Some(values) = response
        .pointer("/snapshot/assets")
        .and_then(Value::as_array)
    {
        artifacts.extend(values.iter().filter_map(|value| {
            value
                .get("local_path")
                .and_then(Value::as_str)
                .map(ToString::to_string)
        }));
    }
    artifacts.sort();
    artifacts.dedup();
    artifacts
        .into_iter()
        .filter(|path| provider_matrix_is_local_path(path))
        .collect()
}

fn collect_provider_matrix_string_array(value: Option<&Value>, output: &mut Vec<String>) {
    if let Some(values) = value.and_then(Value::as_array) {
        output.extend(values.iter().filter_map(|value| {
            match value {
                Value::String(path) => Some(path.clone()),
                Value::Object(object) => object
                    .get("local_path")
                    .or_else(|| object.get("path"))
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
                _ => None,
            }
        }));
    }
}

fn provider_matrix_external_job_id(response: &Value, provider_id: &str) -> String {
    response
        .pointer("/report/job_id")
        .or_else(|| response.pointer("/report/provider_job/job_id"))
        .or_else(|| response.pointer("/job_id"))
        .or_else(|| response.pointer("/provider_job_id"))
        .or_else(|| response.pointer("/provider_request_id"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(|| {
            format!(
                "{}-{}",
                provider_id,
                response
                    .get("provider_request_id")
                    .and_then(Value::as_str)
                    .unwrap_or("production-evidence")
            )
        })
}

fn provider_matrix_is_local_path(path: &str) -> bool {
    let trimmed = path.trim();
    !trimmed.is_empty()
        && !trimmed.starts_with("http://")
        && !trimmed.starts_with("https://")
        && !trimmed.starts_with("s3://")
}

fn validated_provider_matrix_attestation(value: Option<&str>) -> Result<String> {
    let attestation = value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .context("--production-upstream requires --production-attestation=<real-worker-attestation> or POOL_PROVIDER_PRODUCTION_ATTESTATION")?;
    let lowered = attestation.to_ascii_lowercase();
    if attestation.len() < 8
        || [
            "replace-with",
            "placeholder",
            "todo",
            "dummy",
            "fake",
            "mock",
        ]
        .iter()
        .any(|blocked| lowered.contains(blocked))
    {
        bail!("provider production attestation must identify a real upstream worker/SDK run and must not use placeholder, todo, dummy, fake, or mock text");
    }
    Ok(attestation.to_string())
}

fn write_json_value(path: &Path, value: &Value) -> Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    fs::write(path, serde_json::to_string_pretty(value)?)
        .with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

fn production_evidence_body(project_slug: &Option<String>, path: String) -> Result<Value> {
    let text = fs::read_to_string(&path)
        .with_context(|| format!("read production evidence bundle {path}"))?;
    let mut value: Value = serde_json::from_str(&text)
        .with_context(|| format!("parse production evidence bundle {path}"))?;
    let Value::Object(ref mut object) = value else {
        bail!("production evidence bundle must be a JSON object");
    };
    if !object.contains_key("project_slug") {
        if let Some(project_slug) = project_slug.as_deref().filter(|project| *project != "*") {
            object.insert(
                "project_slug".to_string(),
                Value::String(project_slug.to_string()),
            );
        }
    }
    Ok(value)
}

fn production_evidence_item_body(project_slug: &Option<String>, path: String) -> Result<Value> {
    let text = fs::read_to_string(&path)
        .with_context(|| format!("read production evidence item {path}"))?;
    let mut value: Value = serde_json::from_str(&text)
        .with_context(|| format!("parse production evidence item {path}"))?;
    let Value::Object(ref mut object) = value else {
        bail!("production evidence item must be a JSON object");
    };
    if !object.contains_key("project_slug") {
        if let Some(project_slug) = project_slug.as_deref().filter(|project| *project != "*") {
            object.insert(
                "project_slug".to_string(),
                Value::String(project_slug.to_string()),
            );
        }
    }
    Ok(value)
}

fn production_evidence_closeout_body(
    project_slug: &Option<String>,
    args: &ProductionEvidenceCloseoutArgs,
) -> Result<Value> {
    let mut bundles = Vec::new();
    for path in &args.input_paths {
        let text = fs::read_to_string(path)
            .with_context(|| format!("read production evidence bundle {path}"))?;
        let value: Value = serde_json::from_str(&text)
            .with_context(|| format!("parse production evidence bundle {path}"))?;
        if !value.is_object() {
            bail!("production evidence bundle {path} must be a JSON object");
        }
        bundles.push(value);
    }

    let mut body = Map::new();
    if let Some(project_slug) = project_slug
        .as_deref()
        .filter(|project| *project != "*" && !project.trim().is_empty())
    {
        body.insert(
            "project_slug".to_string(),
            Value::String(project_slug.to_string()),
        );
    }
    body.insert(
        "source".to_string(),
        Value::String(
            args.source
                .as_deref()
                .filter(|source| !source.trim().is_empty())
                .unwrap_or("pool-cli closeout-production-evidence")
                .to_string(),
        ),
    );
    body.insert("import".to_string(), Value::Bool(args.import));
    if args.completion_package {
        body.insert(
            "completion_package".to_string(),
            json!({
                "node_id": args.completion_package_node_id.clone(),
                "title": args.completion_package_title.clone(),
                "output_dir": args.completion_package_output_dir.clone(),
                "source": args.completion_package_source.clone(),
                "include_snapshot": args.completion_package_include_snapshot,
            }),
        );
    }
    body.insert("bundles".to_string(), Value::Array(bundles));
    Ok(Value::Object(body))
}

fn merge_production_evidence_response(
    project_slug: &Option<String>,
    args: ProductionEvidenceMergeArgs,
) -> Result<RuntimeHttpResponse> {
    let mut input_bundles = Vec::new();
    for path in &args.input_paths {
        let text = fs::read_to_string(path)
            .with_context(|| format!("read production evidence bundle {path}"))?;
        let value: Value = serde_json::from_str(&text)
            .with_context(|| format!("parse production evidence bundle {path}"))?;
        input_bundles.push((path.clone(), value));
    }

    let bundle = merge_production_evidence_bundle_values(
        project_slug,
        args.source.as_deref(),
        &input_bundles,
    )?;
    let output_path = PathBuf::from(&args.output_path);
    if let Some(parent) = output_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("create output dir {}", parent.display()))?;
    }
    fs::write(&output_path, serde_json::to_string_pretty(&bundle)?)
        .with_context(|| format!("write merged production evidence {}", output_path.display()))?;

    RuntimeHttpResponse::json(
        200,
        json!({
            "kind": "pool_production_evidence_merge",
            "written_bundle_path": output_path.to_string_lossy(),
            "summary": production_evidence_bundle_summary(&bundle),
            "input_paths": args.input_paths,
            "bundle": bundle,
        }),
    )
}

fn write_production_evidence_closeout_response(
    response: RuntimeHttpResponse,
    path: Option<&str>,
) -> Result<RuntimeHttpResponse> {
    let Some(path) = path else {
        return Ok(response);
    };
    if response.status_code >= 400 {
        return Ok(response);
    }
    let mut value: Value =
        serde_json::from_str(&response.body).context("parse production evidence closeout")?;
    let bundle = value
        .pointer("/merge/bundle")
        .cloned()
        .context("production evidence closeout response missing merge.bundle")?;
    let output_path = PathBuf::from(path);
    if let Some(parent) = output_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("create output dir {}", parent.display()))?;
    }
    fs::write(&output_path, serde_json::to_string_pretty(&bundle)?).with_context(|| {
        format!(
            "write production evidence closeout bundle {}",
            output_path.display()
        )
    })?;
    value["written_bundle_path"] = json!(output_path.to_string_lossy());
    value["written_bundle_kind"] = json!("production_evidence_closeout_bundle");

    RuntimeHttpResponse::json(response.status_code, value)
}

fn merge_production_evidence_bundle_values(
    project_slug: &Option<String>,
    source: Option<&str>,
    input_bundles: &[(String, Value)],
) -> Result<Value> {
    let mut merged_project_slug = project_slug
        .as_deref()
        .filter(|project| *project != "*" && !project.trim().is_empty())
        .map(ToString::to_string);
    let mut providers = Vec::new();
    let mut software_actions = Vec::new();
    let mut desktop_vision = Vec::new();
    let mut inputs = Vec::new();

    for (path, value) in input_bundles {
        let object = value
            .as_object()
            .with_context(|| format!("production evidence bundle {path} must be a JSON object"))?;
        if let Some(bundle_project) = optional_string_field(object, "project_slug")? {
            if let Some(existing) = merged_project_slug.as_deref() {
                if existing != bundle_project {
                    bail!(
                        "conflicting project_slug in production evidence bundle {path}: expected {existing}, got {bundle_project}"
                    );
                }
            } else {
                merged_project_slug = Some(bundle_project.to_string());
            }
        }

        let provider_count = append_evidence_items(object, "providers", path, &mut providers)?;
        let software_count =
            append_evidence_items(object, "software_actions", path, &mut software_actions)?;
        let desktop_count =
            append_evidence_items(object, "desktop_vision", path, &mut desktop_vision)?;
        inputs.push(json!({
            "path": path,
            "source": object.get("source").and_then(Value::as_str),
            "project_slug": object.get("project_slug").and_then(Value::as_str),
            "providers": provider_count,
            "software_actions": software_count,
            "desktop_vision": desktop_count,
        }));
    }

    let mut object = Map::new();
    object.insert(
        "source".to_string(),
        Value::String(
            source
                .filter(|value| !value.trim().is_empty())
                .unwrap_or("pool-cli merge-production-evidence")
                .to_string(),
        ),
    );
    if let Some(project_slug) = merged_project_slug {
        object.insert("project_slug".to_string(), Value::String(project_slug));
    }
    object.insert("providers".to_string(), Value::Array(providers));
    object.insert(
        "software_actions".to_string(),
        Value::Array(software_actions),
    );
    object.insert("desktop_vision".to_string(), Value::Array(desktop_vision));
    object.insert(
        "merge".to_string(),
        json!({
            "input_count": input_bundles.len(),
            "inputs": inputs,
        }),
    );

    Ok(Value::Object(object))
}

fn append_evidence_items(
    object: &Map<String, Value>,
    field: &str,
    path: &str,
    output: &mut Vec<Value>,
) -> Result<usize> {
    let Some(value) = object.get(field) else {
        return Ok(0);
    };
    let items = value.as_array().with_context(|| {
        format!("production evidence bundle {path} field {field} must be an array")
    })?;
    output.extend(items.iter().cloned());
    Ok(items.len())
}

fn optional_string_field<'a>(
    object: &'a Map<String, Value>,
    field: &str,
) -> Result<Option<&'a str>> {
    match object.get(field) {
        None => Ok(None),
        Some(Value::String(value)) if value.trim().is_empty() => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.as_str())),
        Some(_) => bail!("production evidence bundle field {field} must be a string"),
    }
}

fn production_evidence_bundle_summary(bundle: &Value) -> Value {
    json!({
        "providers": bundle
            .get("providers")
            .and_then(Value::as_array)
            .map_or(0, Vec::len),
        "software_actions": bundle
            .get("software_actions")
            .and_then(Value::as_array)
            .map_or(0, Vec::len),
        "desktop_vision": bundle
            .get("desktop_vision")
            .and_then(Value::as_array)
            .map_or(0, Vec::len),
    })
}

fn write_production_evidence_template_response(
    response: RuntimeHttpResponse,
    path: Option<&str>,
) -> Result<RuntimeHttpResponse> {
    let Some(path) = path else {
        return Ok(response);
    };
    if response.status_code >= 400 {
        return Ok(response);
    }
    let mut value: Value =
        serde_json::from_str(&response.body).context("parse production evidence template")?;
    let bundle = value
        .get("bundle")
        .cloned()
        .context("production evidence template response missing bundle")?;
    let output_path = PathBuf::from(path);
    if let Some(parent) = output_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("create output dir {}", parent.display()))?;
    }
    fs::write(&output_path, serde_json::to_string_pretty(&bundle)?)
        .with_context(|| format!("write production evidence bundle {}", output_path.display()))?;
    value["written_bundle_path"] = json!(output_path.to_string_lossy());
    value["written_bundle_kind"] = json!("production_evidence_bundle_template");

    RuntimeHttpResponse::json(response.status_code, value)
}

fn write_production_evidence_bundle_from_ledger_response(
    response: RuntimeHttpResponse,
    path: Option<&str>,
) -> Result<RuntimeHttpResponse> {
    let Some(path) = path else {
        return Ok(response);
    };
    if response.status_code >= 400 {
        return Ok(response);
    }
    let mut value: Value =
        serde_json::from_str(&response.body).context("parse ledger production evidence bundle")?;
    let bundle = value
        .get("bundle")
        .cloned()
        .context("ledger production evidence response missing bundle")?;
    let output_path = PathBuf::from(path);
    if let Some(parent) = output_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("create output dir {}", parent.display()))?;
    }
    fs::write(&output_path, serde_json::to_string_pretty(&bundle)?).with_context(|| {
        format!(
            "write ledger production evidence bundle {}",
            output_path.display()
        )
    })?;
    value["written_bundle_path"] = json!(output_path.to_string_lossy());
    value["written_bundle_kind"] = json!("production_evidence_bundle_from_ledger");

    RuntimeHttpResponse::json(response.status_code, value)
}

fn write_production_evidence_handoff_response(
    response: RuntimeHttpResponse,
    path: Option<&str>,
) -> Result<RuntimeHttpResponse> {
    let Some(path) = path else {
        return Ok(response);
    };
    if response.status_code >= 400 {
        return Ok(response);
    }
    let mut value: Value =
        serde_json::from_str(&response.body).context("parse production evidence handoff")?;
    let output_path = PathBuf::from(path);
    if let Some(parent) = output_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("create output dir {}", parent.display()))?;
    }
    fs::write(&output_path, serde_json::to_string_pretty(&value)?).with_context(|| {
        format!(
            "write production evidence handoff {}",
            output_path.display()
        )
    })?;
    value["written_handoff_path"] = json!(output_path.to_string_lossy());
    value["written_handoff_kind"] = json!("production_evidence_handoff");

    RuntimeHttpResponse::json(response.status_code, value)
}

fn write_json_response_to_path(
    response: RuntimeHttpResponse,
    path: Option<&str>,
    label: &str,
    written_path_key: &str,
) -> Result<RuntimeHttpResponse> {
    let Some(path) = path else {
        return Ok(response);
    };
    if response.status_code >= 400 {
        return Ok(response);
    }
    let mut value: Value =
        serde_json::from_str(&response.body).with_context(|| format!("parse {label}"))?;
    let output_path = PathBuf::from(path);
    if let Some(parent) = output_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("create output dir {}", parent.display()))?;
    }
    fs::write(&output_path, serde_json::to_string_pretty(&value)?)
        .with_context(|| format!("write {label} {}", output_path.display()))?;
    value[written_path_key] = json!(output_path.to_string_lossy());

    RuntimeHttpResponse::json(response.status_code, value)
}

fn write_production_evidence_item_template_response(
    response: RuntimeHttpResponse,
    path: Option<&str>,
) -> Result<RuntimeHttpResponse> {
    let Some(path) = path else {
        return Ok(response);
    };
    if response.status_code >= 400 {
        return Ok(response);
    }
    let mut value: Value =
        serde_json::from_str(&response.body).context("parse production evidence item template")?;
    let item = value
        .get("item")
        .cloned()
        .context("production evidence item template response missing item")?;
    let output_path = PathBuf::from(path);
    if let Some(parent) = output_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("create output dir {}", parent.display()))?;
    }
    fs::write(&output_path, serde_json::to_string_pretty(&item)?).with_context(|| {
        format!(
            "write production evidence item template {}",
            output_path.display()
        )
    })?;
    value["written_item_path"] = json!(output_path.to_string_lossy());
    value["written_item_kind"] = json!("production_evidence_item_template");

    RuntimeHttpResponse::json(response.status_code, value)
}

fn software_health_body(args: SoftwareHealthArgs) -> Value {
    let mut body = Map::new();
    body.insert("adapter_id".to_string(), Value::String(args.adapter_id));
    insert_optional(&mut body, "priority", args.priority);
    body.insert("payload_json".to_string(), args.payload_json);
    Value::Object(body)
}

fn software_action_body(project_slug: &Option<String>, args: SoftwareActionArgs) -> Value {
    let mut body = project_body(project_slug);
    body.insert("adapter_id".to_string(), Value::String(args.adapter_id));
    insert_optional(&mut body, "node_id", args.node_id);
    insert_optional(&mut body, "task_title", args.task_title);
    insert_optional(&mut body, "action_kind", args.action_kind);
    insert_optional(&mut body, "priority", args.priority);
    body.insert("payload_json".to_string(), args.payload_json);
    insert_optional_value(&mut body, "evidence_json", args.evidence_json);
    if let Some(requires_confirmation) = args.requires_confirmation {
        body.insert(
            "requires_confirmation".to_string(),
            json!(requires_confirmation),
        );
    }
    Value::Object(body)
}

const CLI_SOFTWARE_EVIDENCE_TARGETS: &[&str] = &[
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

struct SoftwareMatrixOverrides<'a> {
    endpoints: &'a [(String, String)],
    commands: &'a [(String, String)],
    artifacts: &'a [(String, String)],
    attestations: &'a [(String, String)],
}

fn production_evidence_software_matrix_response(
    server: &RuntimeHttpServer,
    project_slug: &Option<String>,
    args: SoftwareEvidenceMatrixArgs,
) -> Result<RuntimeHttpResponse> {
    let project_slug = concrete_project_slug(project_slug).unwrap_or("demo");
    let output_root = PathBuf::from(args.output_root.clone().unwrap_or_else(|| {
        format!("worlds/{project_slug}/output/production-evidence/software-matrix")
    }));
    fs::create_dir_all(&output_root).with_context(|| {
        format!(
            "create software evidence output root {}",
            output_root.display()
        )
    })?;
    let evidence_bundle_path = args
        .evidence_bundle_path
        .clone()
        .map(PathBuf::from)
        .unwrap_or_else(|| output_root.join("software-production-evidence-bundle.json"));
    let env_lookup = |name: &str| {
        if args.use_env {
            env::var(name).ok()
        } else {
            None
        }
    };
    let overrides = SoftwareMatrixOverrides {
        endpoints: &args.software_endpoints,
        commands: &args.software_commands,
        artifacts: &args.software_artifacts,
        attestations: &args.software_attestations,
    };

    let mut results = Vec::new();
    let mut software_items = Vec::new();
    let mut succeeded = 0_usize;
    let mut failed = 0_usize;

    for adapter_id in CLI_SOFTWARE_EVIDENCE_TARGETS {
        let evidence_mode = if args.production_software {
            "production_software"
        } else {
            "local_control_profile"
        };
        let body = cli_software_matrix_body(
            project_slug,
            adapter_id,
            evidence_mode,
            args.production_software,
            &overrides,
            &env_lookup,
        );
        let response =
            server.handle_request_with_body("POST", "/api/software-actions", &body.to_string())?;
        let value: Value = serde_json::from_str(&response.body)
            .with_context(|| format!("parse software matrix response for {adapter_id}"))?;
        let status = value
            .pointer("/report/status")
            .and_then(Value::as_str)
            .or_else(|| value.get("error").and_then(Value::as_str))
            .unwrap_or("unknown")
            .to_string();
        let production_ready = cli_software_request_claims_real_production(&body);
        if response.status_code < 400
            && status == "Succeeded"
            && (!args.production_software || production_ready)
        {
            if args.production_software && production_ready {
                match cli_software_production_evidence_item(adapter_id, &body, &value) {
                    Ok(item) => {
                        succeeded += 1;
                        software_items.push(item);
                        results.push(cli_software_matrix_result(
                            adapter_id,
                            "succeeded",
                            &status,
                            Some(&value),
                        ));
                    }
                    Err(error) => {
                        failed += 1;
                        results.push(cli_software_matrix_result(
                            adapter_id,
                            "failed",
                            &format!("production_evidence_item_failed: {error:#}"),
                            Some(&value),
                        ));
                    }
                }
            } else {
                succeeded += 1;
                results.push(cli_software_matrix_result(
                    adapter_id,
                    "succeeded",
                    &status,
                    Some(&value),
                ));
            }
        } else {
            failed += 1;
            let reason = body
                .pointer("/evidence_json/missing_production_software_config")
                .and_then(Value::as_str)
                .unwrap_or(&status);
            results.push(cli_software_matrix_result(
                adapter_id,
                "failed",
                reason,
                Some(&value),
            ));
        }
    }

    let evidence_bundle = json!({
        "source": "pool-cli production-evidence-software-matrix",
        "project_slug": project_slug,
        "providers": [],
        "software_actions": software_items,
        "desktop_vision": [],
    });
    write_json_value(&evidence_bundle_path, &evidence_bundle)?;

    RuntimeHttpResponse::json(
        200,
        json!({
            "kind": "pool_software_production_evidence_matrix",
            "project_slug": project_slug,
            "output_root": output_root,
            "evidence_bundle_path": evidence_bundle_path,
            "summary": {
                "total": CLI_SOFTWARE_EVIDENCE_TARGETS.len(),
                "succeeded": succeeded,
                "failed": failed,
                "production_software": args.production_software,
                "production_evidence_items": evidence_bundle["software_actions"].as_array().map_or(0, Vec::len),
            },
            "results": results,
            "bundle": evidence_bundle,
            "commands": {
                "validate": format!("pool-cli --project {project_slug} validate-production-evidence {}", evidence_bundle_path.display()),
                "closeout": format!("pool-cli --project {project_slug} closeout-production-evidence --output <merged-bundle.json> {}", evidence_bundle_path.display()),
            }
        }),
    )
}

fn cli_software_matrix_body(
    project_slug: &str,
    adapter_id: &str,
    evidence_mode: &str,
    production_software: bool,
    overrides: &SoftwareMatrixOverrides,
    env_lookup: &impl Fn(&str) -> Option<String>,
) -> Value {
    let production_attestation = cli_software_attestation(adapter_id, overrides, env_lookup);
    if production_software && production_attestation.is_none() {
        return cli_software_missing_config_body(
            project_slug,
            adapter_id,
            evidence_mode,
            &format!(
                "{} with a real software/plugin/API/CLI/MCP run attestation",
                cli_software_attestation_env_names(adapter_id).join(" or ")
            ),
        );
    }
    let production_attestation = production_attestation.as_deref();

    if adapter_id == "unreal" {
        if production_software {
            if let Some(endpoint) = cli_software_endpoint(adapter_id, overrides, env_lookup) {
                let artifacts = cli_software_artifacts(adapter_id, overrides, env_lookup);
                if artifacts.is_empty() {
                    return cli_software_missing_config_body(
                        project_slug,
                        adapter_id,
                        evidence_mode,
                        &cli_software_missing_control_message(
                            &cli_software_endpoint_env_names(adapter_id),
                            adapter_id,
                        ),
                    );
                }
                return json!({
                    "project_slug": project_slug,
                    "adapter_id": "unreal",
                    "action_kind": "CreateScene",
                    "priority": "ApiMcp",
                    "task_title": "unreal production software evidence",
                    "payload_json": {
                        "mcp_endpoint": endpoint,
                        "level": "demo_software_evidence",
                        "assets": [format!("worlds/{project_slug}/output/1-world.glb")],
                        "artifacts": artifacts
                    },
                    "requires_confirmation": false,
                    "evidence_json": cli_software_evidence_json(adapter_id, "api_mcp", evidence_mode, true, false, true, production_attestation),
                });
            }
            return cli_software_missing_config_body(
                project_slug,
                adapter_id,
                evidence_mode,
                &cli_software_missing_control_message(
                    &cli_software_endpoint_env_names(adapter_id),
                    adapter_id,
                ),
            );
        }
        return json!({
            "project_slug": project_slug,
            "adapter_id": "unreal",
            "action_kind": "CreateScene",
            "priority": "ApiMcp",
            "task_title": "unreal software evidence",
            "payload_json": {
                "level": "demo_software_evidence",
                "assets": [format!("worlds/{project_slug}/output/1-world.glb")]
            },
            "requires_confirmation": false,
            "evidence_json": cli_software_evidence_json(adapter_id, "api_mcp", evidence_mode, false, true, false, None),
        });
    }

    if adapter_id == "hermes" && production_software {
        if let Some(endpoint) = cli_software_endpoint(adapter_id, overrides, env_lookup) {
            let artifacts = cli_software_artifacts(adapter_id, overrides, env_lookup);
            if !artifacts.is_empty() {
                return json!({
                    "project_slug": project_slug,
                    "adapter_id": "hermes",
                    "action_kind": "CreateScene",
                    "priority": "ApiMcp",
                    "task_title": "hermes production software evidence",
                    "payload_json": {
                        "hermes_endpoint": endpoint,
                        "mcp_endpoint": endpoint,
                        "instruction": "Run Pool production software evidence orchestration through Hermes.",
                        "project_slug": project_slug,
                        "artifacts": artifacts
                    },
                    "requires_confirmation": false,
                    "evidence_json": cli_software_evidence_json(adapter_id, "api_mcp", evidence_mode, true, false, true, production_attestation),
                });
            }
        }
    }

    if production_software {
        if let Some(endpoint) = cli_software_endpoint(adapter_id, overrides, env_lookup) {
            let artifacts = cli_software_artifacts(adapter_id, overrides, env_lookup);
            if artifacts.is_empty() {
                return cli_software_missing_config_body(
                    project_slug,
                    adapter_id,
                    evidence_mode,
                    &cli_software_missing_control_message(
                        &cli_software_endpoint_env_names(adapter_id),
                        adapter_id,
                    ),
                );
            }
            return cli_endpoint_software_body(
                project_slug,
                adapter_id,
                evidence_mode,
                endpoint,
                artifacts,
                production_attestation,
            );
        }
        if let Some(command) = cli_software_command(adapter_id, overrides, env_lookup) {
            let artifacts = cli_software_artifacts(adapter_id, overrides, env_lookup);
            if artifacts.is_empty() {
                return cli_software_missing_config_body(
                    project_slug,
                    adapter_id,
                    evidence_mode,
                    &cli_software_missing_control_message(
                        &cli_software_command_env_names(adapter_id),
                        adapter_id,
                    ),
                );
            }
            return cli_command_software_body(
                project_slug,
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
        return cli_software_missing_config_body(
            project_slug,
            adapter_id,
            evidence_mode,
            &cli_software_missing_control_message(
                &cli_software_control_env_names(adapter_id),
                adapter_id,
            ),
        );
    }

    cli_command_software_body(
        project_slug,
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

fn cli_endpoint_software_body(
    project_slug: &str,
    adapter_id: &str,
    evidence_mode: &str,
    endpoint: String,
    artifacts: Vec<String>,
    production_attestation: Option<&str>,
) -> Value {
    json!({
        "project_slug": project_slug,
        "adapter_id": adapter_id,
        "action_kind": "CreateScene",
        "priority": "ApiMcp",
        "task_title": format!("{adapter_id} software evidence"),
        "payload_json": {
            "endpoint": endpoint.clone(),
            "mcp_endpoint": endpoint,
            "project_slug": project_slug,
            "instruction": format!("Run Pool production software evidence through the {adapter_id} API/MCP adapter."),
            "artifacts": artifacts
        },
        "requires_confirmation": false,
        "evidence_json": cli_software_evidence_json(
            adapter_id,
            "api_mcp",
            evidence_mode,
            true,
            false,
            true,
            production_attestation
        ),
    })
}

fn cli_command_software_body(
    project_slug: &str,
    adapter_id: &str,
    evidence_mode: &str,
    production_software: bool,
    local_mock_software: bool,
    configured_real_software: bool,
    command: String,
    artifacts: Vec<String>,
    production_attestation: Option<&str>,
) -> Value {
    json!({
        "project_slug": project_slug,
        "adapter_id": adapter_id,
        "action_kind": "ExecuteCli",
        "priority": "SkillsCli",
        "task_title": format!("{adapter_id} software evidence"),
        "payload_json": {
            "command": command,
            "allowed_commands": [command.split_whitespace().next().unwrap_or("").to_string()],
            "timeout_ms": 2000,
            "max_output_bytes": 2048,
            "artifacts": artifacts
        },
        "requires_confirmation": false,
        "evidence_json": cli_software_evidence_json(
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

fn cli_software_missing_config_body(
    project_slug: &str,
    adapter_id: &str,
    evidence_mode: &str,
    expected_env: &str,
) -> Value {
    let mut body = cli_command_software_body(
        project_slug,
        adapter_id,
        "production_software_missing_config",
        false,
        true,
        false,
        "/usr/bin/false".to_string(),
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

fn cli_software_evidence_json(
    adapter_id: &str,
    control_profile: &str,
    evidence_mode: &str,
    production_software: bool,
    local_mock_software: bool,
    configured_real_software: bool,
    production_attestation: Option<&str>,
) -> Value {
    let mut evidence = json!({
        "source": "pool-cli production-evidence-software-matrix",
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

fn cli_software_request_claims_real_production(request_body: &Value) -> bool {
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

fn cli_software_production_evidence_item(
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
            .unwrap_or_else(|| cli_default_software_control_profile(adapter_id)),
        "task_title": format!("{adapter_id} production software evidence"),
        "artifacts": cli_software_response_artifacts(response),
        "evidence_json": {
            "source": "pool-cli production-evidence-software-matrix",
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
        "verification_json": {
            "source": "pool-cli production-evidence-software-matrix",
            "production_software": true,
            "local_mock_software": false,
            "software_action_id": action_id,
            "request": request_body,
            "runtime_report": response.get("report"),
            "runtime_task": response.get("task"),
        },
    }))
}

fn cli_software_matrix_result(
    adapter_id: &str,
    status: &str,
    reason: &str,
    response: Option<&Value>,
) -> Value {
    json!({
        "adapter_id": adapter_id,
        "status": status,
        "reason": reason,
        "software_action_id": response
            .and_then(|value| value.pointer("/report/action_id"))
            .and_then(Value::as_str),
    })
}

fn cli_software_response_artifacts(response: &Value) -> Vec<String> {
    let mut artifacts = Vec::new();
    collect_provider_matrix_string_array(
        response.pointer("/report/result/artifacts"),
        &mut artifacts,
    );
    collect_provider_matrix_string_array(response.pointer("/report/artifacts"), &mut artifacts);
    collect_provider_matrix_string_array(
        response.pointer("/software_action/verification/artifacts"),
        &mut artifacts,
    );
    collect_provider_matrix_string_array(response.pointer("/task/artifacts"), &mut artifacts);
    if let Some(action_id) = response
        .pointer("/report/action_id")
        .and_then(Value::as_str)
    {
        if let Some(action) = response
            .pointer("/snapshot/software_actions")
            .and_then(Value::as_array)
            .and_then(|actions| {
                actions
                    .iter()
                    .find(|action| action.get("id").and_then(Value::as_str) == Some(action_id))
            })
        {
            collect_provider_matrix_string_array(
                action.pointer("/verification/artifacts"),
                &mut artifacts,
            );
            collect_provider_matrix_string_array(
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

fn production_evidence_desktop_vision_response(
    server: &RuntimeHttpServer,
    project_slug: &Option<String>,
    args: DesktopVisionEvidenceArgs,
) -> Result<RuntimeHttpResponse> {
    let project_slug = concrete_project_slug(project_slug).unwrap_or("demo");
    let output_root = PathBuf::from(args.output_root.clone().unwrap_or_else(|| {
        format!("worlds/{project_slug}/output/production-evidence/desktop-vision")
    }));
    fs::create_dir_all(&output_root).with_context(|| {
        format!(
            "create desktop vision evidence output root {}",
            output_root.display()
        )
    })?;
    let evidence_bundle_path = args
        .evidence_bundle_path
        .clone()
        .map(PathBuf::from)
        .unwrap_or_else(|| output_root.join("desktop-vision-production-evidence-bundle.json"));
    let env_lookup = |name: &str| {
        if args.use_env {
            env::var(name).ok()
        } else {
            None
        }
    };

    let trace_path = args
        .trace_path
        .clone()
        .or_else(|| cli_env_arg_value(args.trace_env.as_deref(), &env_lookup))
        .or_else(|| env_lookup("POOL_DESKTOP_VISION_TRACE"))
        .or_else(|| env_lookup("POOL_DESKTOP_VISION_TRACE_OUTPUT"));
    let controller_id = args
        .controller_id
        .clone()
        .or_else(|| cli_env_arg_value(args.controller_id_env.as_deref(), &env_lookup))
        .or_else(|| env_lookup("POOL_DESKTOP_VISION_CONTROLLER_ID"))
        .unwrap_or_else(|| "pool-cli-desktop-vision-controller".to_string());
    let external_action_id = args
        .external_action_id
        .clone()
        .or_else(|| cli_env_arg_value(args.external_action_id_env.as_deref(), &env_lookup))
        .or_else(|| env_lookup("POOL_DESKTOP_VISION_EXTERNAL_ACTION_ID"));
    let production_attestation = args
        .production_attestation
        .clone()
        .or_else(|| cli_env_arg_value(args.production_attestation_env.as_deref(), &env_lookup))
        .or_else(|| env_lookup("POOL_DESKTOP_VISION_PRODUCTION_ATTESTATION"));

    let project_slug_option = Some(project_slug.to_string());
    let queue_response = server.handle_path(&path_with_project(
        "/api/desktop-recognition/requests",
        &project_slug_option,
    ))?;
    let queue_value: Value = serde_json::from_str(&queue_response.body)
        .context("parse desktop recognition queue response")?;
    let requests = queue_value
        .get("requests")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let trace_path_exists = trace_path
        .as_deref()
        .is_some_and(|path| Path::new(path).exists());
    let can_write_production = args.production_vision
        && trace_path_exists
        && external_action_id
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
        && production_attestation
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty());

    let mut results = Vec::new();
    let mut desktop_items = Vec::new();
    let mut succeeded = 0_usize;
    let mut failed = 0_usize;

    if can_write_production {
        let trace_path = trace_path.clone().expect("checked above");
        let external_action_id = external_action_id.clone().expect("checked above");
        let production_attestation = production_attestation.clone().expect("checked above");
        for request in requests.iter().take(args.limit) {
            let Some(software_action_id) = request
                .get("software_action_id")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
            else {
                failed += 1;
                results.push(cli_desktop_vision_result(
                    request,
                    "failed",
                    "missing_software_action_id",
                    None,
                ));
                continue;
            };
            let mut artifacts = vec![trace_path.clone()];
            collect_provider_matrix_string_array(
                request.pointer("/pool_desktop_action/artifacts"),
                &mut artifacts,
            );
            let body = json!({
                "software_action_id": software_action_id,
                "task_id": request.get("task_id").cloned(),
                "status": "succeeded",
                "message": "Pool CLI desktop vision production evidence callback",
                "artifacts": dedup_string_vec(artifacts),
                "screen_trace_path": trace_path,
                "result": {
                    "controller": controller_id,
                    "controller_id": controller_id,
                    "external_action_id": external_action_id,
                    "production_attestation": production_attestation,
                    "external_visual_model": true,
                    "visual_model": "external",
                    "vision_trace_path": trace_path,
                    "source": "pool-cli production-evidence-desktop-vision"
                },
                "verification": {
                    "source": "pool-cli production-evidence-desktop-vision",
                    "external_action_id": external_action_id,
                    "controller_id": controller_id,
                    "production_attestation": production_attestation,
                    "external_visual_model": true,
                    "local_trace_smoke": false
                }
            });
            let callback_response = server.handle_request_with_body(
                "POST",
                "/api/desktop-recognition/results",
                &body.to_string(),
            )?;
            let callback_value: Value = serde_json::from_str(&callback_response.body)
                .context("parse desktop vision callback response")?;
            if callback_response.status_code >= 400 {
                failed += 1;
                results.push(cli_desktop_vision_result(
                    request,
                    "failed",
                    callback_value
                        .get("error")
                        .and_then(Value::as_str)
                        .unwrap_or("desktop_vision_callback_failed"),
                    Some(&callback_value),
                ));
                continue;
            }

            let ledger_response =
                server.handle_path(&production_evidence_item_from_ledger_path(
                    &Some(project_slug.to_string()),
                    Some("pool-cli production-evidence-desktop-vision"),
                    None,
                    None,
                    Some(software_action_id),
                ))?;
            let ledger_value: Value = serde_json::from_str(&ledger_response.body)
                .context("parse desktop vision ledger item response")?;
            if ledger_response.status_code < 400 {
                if let Some(item) = ledger_value
                    .get("item")
                    .and_then(|item| item.get("desktop_vision"))
                {
                    desktop_items.push(item.clone());
                    succeeded += 1;
                    results.push(cli_desktop_vision_result(
                        request,
                        "succeeded",
                        "desktop_vision_evidence_item_written",
                        Some(&ledger_value),
                    ));
                    continue;
                }
            }
            failed += 1;
            results.push(cli_desktop_vision_result(
                request,
                "failed",
                ledger_value
                    .get("error")
                    .and_then(Value::as_str)
                    .unwrap_or("desktop_vision_ledger_item_failed"),
                Some(&ledger_value),
            ));
        }
    } else {
        let reason = if !args.production_vision {
            "production_vision_flag_missing"
        } else if trace_path.is_none() {
            "missing_desktop_vision_trace_path"
        } else if !trace_path_exists {
            "desktop_vision_trace_path_not_found"
        } else if external_action_id.is_none() {
            "missing_external_action_id"
        } else {
            "missing_production_attestation"
        };
        failed = requests.len().min(args.limit);
        results.extend(
            requests
                .iter()
                .take(args.limit)
                .map(|request| cli_desktop_vision_result(request, "skipped", reason, None)),
        );
    }

    let evidence_bundle = json!({
        "source": "pool-cli production-evidence-desktop-vision",
        "project_slug": project_slug,
        "providers": [],
        "software_actions": [],
        "desktop_vision": desktop_items,
    });
    write_json_value(&evidence_bundle_path, &evidence_bundle)?;

    RuntimeHttpResponse::json(
        200,
        json!({
            "kind": "pool_desktop_vision_production_evidence",
            "project_slug": project_slug,
            "output_root": output_root,
            "evidence_bundle_path": evidence_bundle_path,
            "summary": {
                "queued": requests.len(),
                "processed": succeeded + failed,
                "succeeded": succeeded,
                "failed": failed,
                "production_vision": args.production_vision,
                "production_evidence_items": evidence_bundle["desktop_vision"].as_array().map_or(0, Vec::len),
            },
            "requirements": {
                "trace_path": trace_path,
                "trace_path_exists": trace_path_exists,
                "external_action_id": external_action_id,
                "controller_id": controller_id,
                "production_attestation": production_attestation,
            },
            "results": results,
            "bundle": evidence_bundle,
            "commands": {
                "validate": format!("pool-cli --project {project_slug} validate-production-evidence {}", evidence_bundle_path.display()),
                "closeout": format!("pool-cli --project {project_slug} closeout-production-evidence --output <merged-bundle.json> {}", evidence_bundle_path.display()),
            }
        }),
    )
}

fn cli_desktop_vision_result(
    request: &Value,
    status: &str,
    reason: &str,
    response: Option<&Value>,
) -> Value {
    json!({
        "software_action_id": request.get("software_action_id").and_then(Value::as_str),
        "adapter_id": request.get("adapter_id").and_then(Value::as_str),
        "status": status,
        "reason": reason,
        "response_status": response.and_then(|value| value.get("status").and_then(Value::as_str)),
    })
}

fn cli_default_software_control_profile(adapter_id: &str) -> &'static str {
    match adapter_id {
        "unreal" | "unity" | "hermes" => "api_mcp",
        "touchdesigner" | "madmapper" => "desktop_recognition",
        _ => "skills_cli",
    }
}

fn cli_software_command(
    adapter_id: &str,
    overrides: &SoftwareMatrixOverrides,
    env_lookup: &impl Fn(&str) -> Option<String>,
) -> Option<String> {
    cli_software_override_value(overrides.commands, adapter_id).or_else(|| {
        cli_software_command_env_names(adapter_id)
            .into_iter()
            .find_map(|name| env_lookup(&name).filter(|value| !value.trim().is_empty()))
    })
}

fn cli_software_endpoint(
    adapter_id: &str,
    overrides: &SoftwareMatrixOverrides,
    env_lookup: &impl Fn(&str) -> Option<String>,
) -> Option<String> {
    cli_software_override_value(overrides.endpoints, adapter_id).or_else(|| {
        cli_software_endpoint_env_names(adapter_id)
            .into_iter()
            .find_map(|name| env_lookup(&name).filter(|value| !value.trim().is_empty()))
    })
}

fn cli_software_attestation(
    adapter_id: &str,
    overrides: &SoftwareMatrixOverrides,
    env_lookup: &impl Fn(&str) -> Option<String>,
) -> Option<String> {
    cli_software_override_value(overrides.attestations, adapter_id).or_else(|| {
        cli_software_attestation_env_names(adapter_id)
            .into_iter()
            .find_map(|name| env_lookup(&name).filter(|value| !value.trim().is_empty()))
    })
}

fn cli_software_artifacts(
    adapter_id: &str,
    overrides: &SoftwareMatrixOverrides,
    env_lookup: &impl Fn(&str) -> Option<String>,
) -> Vec<String> {
    let mut artifacts = cli_software_override_values(overrides.artifacts, adapter_id);
    if artifacts.is_empty() {
        artifacts = cli_software_artifact_env_names(adapter_id)
            .into_iter()
            .find_map(|name| env_lookup(&name).filter(|value| !value.trim().is_empty()))
            .map(|value| cli_software_split_artifacts(&value))
            .unwrap_or_default();
    }
    artifacts
        .into_iter()
        .filter(|value| !value.trim().is_empty() && !value.contains("://"))
        .collect()
}

fn cli_software_override_value(overrides: &[(String, String)], adapter_id: &str) -> Option<String> {
    let route_key = cli_software_route_key(adapter_id);
    overrides
        .iter()
        .rev()
        .find(|(candidate, value)| {
            cli_software_route_key(candidate) == route_key && !value.trim().is_empty()
        })
        .map(|(_, value)| value.clone())
}

fn cli_software_override_values(overrides: &[(String, String)], adapter_id: &str) -> Vec<String> {
    let route_key = cli_software_route_key(adapter_id);
    overrides
        .iter()
        .filter(|(candidate, _)| cli_software_route_key(candidate) == route_key)
        .flat_map(|(_, value)| cli_software_split_artifacts(value))
        .collect()
}

fn cli_software_split_artifacts(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn cli_software_route_key(adapter_id: &str) -> String {
    match adapter_id
        .trim()
        .to_ascii_lowercase()
        .replace(['_', ' '], "-")
        .as_str()
    {
        "davinci" | "davinci-resolve" | "da-vinci-resolve" | "resolve" => "resolve".to_string(),
        "touch-designer" | "touchdesigner" => "touchdesigner".to_string(),
        "motiondb" | "motion-db" | "mocap-db" | "mocap-database" | "motion-database" => {
            "motion-db".to_string()
        }
        "editor" | "editing" | "editing-suite" | "editing-software" => "editing-suite".to_string(),
        value => value.to_string(),
    }
}

fn cli_software_missing_control_message(control_env_names: &[String], adapter_id: &str) -> String {
    format!(
        "{}; plus {} with local file paths",
        control_env_names.join(" or "),
        cli_software_artifact_env_names(adapter_id).join(" or ")
    )
}

fn cli_software_control_env_names(adapter_id: &str) -> Vec<String> {
    let mut names = cli_software_endpoint_env_names(adapter_id);
    names.extend(cli_software_command_env_names(adapter_id));
    dedup_string_vec(names)
}

fn cli_software_command_env_names(adapter_id: &str) -> Vec<String> {
    let token = cli_env_token(adapter_id);
    let mut names = vec![
        format!("POOL_SOFTWARE_{token}_COMMAND"),
        format!("POOL_{token}_COMMAND"),
    ];
    names.extend(
        cli_alias_env_tokens(adapter_id)
            .into_iter()
            .flat_map(|alias| {
                [
                    format!("POOL_SOFTWARE_{alias}_COMMAND"),
                    format!("POOL_{alias}_COMMAND"),
                ]
            }),
    );
    dedup_string_vec(names)
}

fn cli_software_endpoint_env_names(adapter_id: &str) -> Vec<String> {
    let token = cli_env_token(adapter_id);
    let mut names = vec![
        format!("POOL_SOFTWARE_{token}_ENDPOINT"),
        format!("POOL_{token}_ENDPOINT"),
    ];
    names.extend(
        cli_alias_env_tokens(adapter_id)
            .into_iter()
            .flat_map(|alias| {
                [
                    format!("POOL_SOFTWARE_{alias}_ENDPOINT"),
                    format!("POOL_{alias}_ENDPOINT"),
                ]
            }),
    );
    if adapter_id == "unreal" {
        names.insert(0, "POOL_UNREAL_MCP_ENDPOINT".to_string());
    }
    if adapter_id == "hermes" {
        names.insert(0, "POOL_HERMES_MCP_ENDPOINT".to_string());
        names.insert(1, "POOL_HERMES_ENDPOINT".to_string());
    }
    dedup_string_vec(names)
}

fn cli_software_attestation_env_names(adapter_id: &str) -> Vec<String> {
    let token = cli_env_token(adapter_id);
    let mut names = vec![
        format!("POOL_SOFTWARE_{token}_PRODUCTION_ATTESTATION"),
        format!("POOL_{token}_PRODUCTION_ATTESTATION"),
        "POOL_SOFTWARE_PRODUCTION_ATTESTATION".to_string(),
    ];
    names.extend(
        cli_alias_env_tokens(adapter_id)
            .into_iter()
            .flat_map(|alias| {
                [
                    format!("POOL_SOFTWARE_{alias}_PRODUCTION_ATTESTATION"),
                    format!("POOL_{alias}_PRODUCTION_ATTESTATION"),
                ]
            }),
    );
    dedup_string_vec(names)
}

fn cli_software_artifact_env_names(adapter_id: &str) -> Vec<String> {
    let token = cli_env_token(adapter_id);
    let mut names = vec![
        format!("POOL_SOFTWARE_{token}_ARTIFACTS"),
        format!("POOL_{token}_ARTIFACTS"),
    ];
    names.extend(
        cli_alias_env_tokens(adapter_id)
            .into_iter()
            .flat_map(|alias| {
                [
                    format!("POOL_SOFTWARE_{alias}_ARTIFACTS"),
                    format!("POOL_{alias}_ARTIFACTS"),
                ]
            }),
    );
    dedup_string_vec(names)
}

fn cli_alias_env_tokens(adapter_id: &str) -> Vec<String> {
    match adapter_id {
        "resolve" => vec!["DAVINCI_RESOLVE".to_string()],
        "motion-db" => vec!["MOTION_DB".to_string(), "MOCAP_DB".to_string()],
        "editing-suite" => vec!["EDITING_SUITE".to_string(), "EDITOR".to_string()],
        "touchdesigner" => vec!["TOUCH_DESIGNER".to_string()],
        _ => Vec::new(),
    }
}

fn cli_env_token(adapter_id: &str) -> String {
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

fn dedup_string_vec(names: Vec<String>) -> Vec<String> {
    let mut deduped = Vec::new();
    for name in names {
        if !deduped.contains(&name) {
            deduped.push(name);
        }
    }
    deduped
}

fn cli_env_arg_value(
    env_name: Option<&str>,
    env_lookup: &impl Fn(&str) -> Option<String>,
) -> Option<String> {
    env_name
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .and_then(|name| env_lookup(name))
        .filter(|value| !value.trim().is_empty())
}

fn workflow_run_body(project_slug: &Option<String>, args: WorkflowRunArgs) -> Value {
    let mut body = project_body(project_slug);
    insert_optional(&mut body, "title", args.title);
    insert_optional(&mut body, "prompt", args.prompt);
    insert_optional(&mut body, "output_root", args.output_root);
    if !args.source_inputs.is_empty() {
        body.insert("source_inputs".to_string(), json!(args.source_inputs));
    }
    if let Some(duration_ms) = args.duration_ms {
        body.insert("duration_ms".to_string(), json!(duration_ms));
    }
    insert_optional(&mut body, "agent_mode", args.agent_mode);
    insert_optional(&mut body, "hermes_endpoint", args.hermes_endpoint);
    insert_optional(&mut body, "hermes_auth_token", args.hermes_auth_token);
    if args.agent_requires_confirmation {
        body.insert("agent_requires_confirmation".to_string(), json!(true));
    }
    insert_optional(&mut body, "three_dgs_mode", args.three_dgs_mode);
    insert_optional(
        &mut body,
        "three_dgs_provider_id",
        args.three_dgs_provider_id,
    );
    insert_optional(&mut body, "three_dgs_endpoint", args.three_dgs_endpoint);
    insert_optional(&mut body, "three_dgs_api_key", args.three_dgs_api_key);
    insert_optional(&mut body, "unreal_mode", args.unreal_mode);
    insert_optional(&mut body, "unreal_endpoint", args.unreal_endpoint);
    insert_optional(&mut body, "unreal_auth_token", args.unreal_auth_token);
    Value::Object(body)
}

fn output_package_body(project_slug: &Option<String>, args: OutputPackageArgs) -> Value {
    let mut body = project_body(project_slug);
    insert_optional(&mut body, "node_id", args.node_id);
    insert_optional(&mut body, "title", args.title);
    insert_optional(&mut body, "output_dir", args.output_dir);
    if !args.source_assets.is_empty() {
        body.insert("source_assets".to_string(), json!(args.source_assets));
    }
    if let Some(duration_ms) = args.duration_ms {
        body.insert("duration_ms".to_string(), json!(duration_ms));
    }
    Value::Object(body)
}

fn output_result_body(project_slug: &Option<String>, args: OutputResultArgs) -> Value {
    let mut body = project_body(project_slug);
    insert_optional(&mut body, "node_id", args.node_id);
    body.insert("target".to_string(), json!(args.target));
    insert_optional(&mut body, "local_path", args.local_path);
    body.insert("status".to_string(), json!(args.status));
    insert_optional(&mut body, "runtime", args.runtime);
    insert_optional(&mut body, "adapter_id", args.adapter_id);
    insert_optional(&mut body, "software_action_id", args.software_action_id);
    insert_optional(&mut body, "message", args.message);
    if !args.artifacts.is_empty() {
        body.insert("artifacts".to_string(), json!(args.artifacts));
    }
    if !args.metrics.is_empty() {
        body.insert(
            "metrics".to_string(),
            json!(args
                .metrics
                .into_iter()
                .map(|(label, value)| json!({ "label": label, "value": value }))
                .collect::<Vec<_>>()),
        );
    }
    if let Some(verification) = args.verification {
        body.insert("verification".to_string(), verification);
    }
    Value::Object(body)
}

fn handoff_package_body(project_slug: &Option<String>, args: HandoffPackageArgs) -> Value {
    let mut body = project_body(project_slug);
    insert_optional(&mut body, "node_id", args.node_id);
    insert_optional(&mut body, "title", args.title);
    insert_optional(&mut body, "output_dir", args.output_dir);
    if args.include_snapshot {
        body.insert("include_snapshot".to_string(), json!(true));
    }
    Value::Object(body)
}

fn software_conformance_package_body(
    project_slug: &Option<String>,
    args: SoftwareConformancePackageArgs,
) -> Value {
    let mut body = project_body(project_slug);
    body.insert("adapter_id".to_string(), json!(args.adapter_id));
    insert_optional(&mut body, "node_id", args.node_id);
    insert_optional(&mut body, "title", args.title);
    insert_optional(&mut body, "output_dir", args.output_dir);
    Value::Object(body)
}

fn agent_conformance_package_body(
    project_slug: &Option<String>,
    args: AgentConformancePackageArgs,
) -> Value {
    let mut body = project_body(project_slug);
    body.insert("kind".to_string(), json!(args.kind));
    insert_optional(&mut body, "node_id", args.node_id);
    insert_optional(&mut body, "title", args.title);
    insert_optional(&mut body, "output_dir", args.output_dir);
    Value::Object(body)
}

fn integration_conformance_package_body(
    project_slug: &Option<String>,
    args: IntegrationConformancePackageArgs,
) -> Value {
    let mut body = project_body(project_slug);
    insert_optional(&mut body, "node_id", args.node_id);
    insert_optional(&mut body, "title", args.title);
    insert_optional(&mut body, "output_dir", args.output_dir);
    if !args.providers.is_empty() {
        body.insert("providers".to_string(), json!(args.providers));
    }
    if !args.software_adapters.is_empty() {
        body.insert(
            "software_adapters".to_string(),
            json!(args.software_adapters),
        );
    }
    insert_optional(&mut body, "agent_kind", args.agent_kind);
    if !args.include_providers {
        body.insert("include_providers".to_string(), json!(false));
    }
    if !args.include_software {
        body.insert("include_software".to_string(), json!(false));
    }
    if !args.include_agent {
        body.insert("include_agent".to_string(), json!(false));
    }
    Value::Object(body)
}

fn provider_conformance_package_body(
    project_slug: &Option<String>,
    args: ProviderConformancePackageArgs,
) -> Value {
    let mut body = project_body(project_slug);
    body.insert("provider_id".to_string(), json!(args.provider_id));
    insert_optional(&mut body, "node_id", args.node_id);
    insert_optional(&mut body, "title", args.title);
    insert_optional(&mut body, "output_dir", args.output_dir);
    Value::Object(body)
}

fn production_evidence_handoff_package_body(
    project_slug: &Option<String>,
    args: ProductionEvidenceHandoffPackageArgs,
) -> Value {
    let mut body = project_body(project_slug);
    insert_optional(&mut body, "node_id", args.node_id);
    insert_optional(&mut body, "title", args.title);
    insert_optional(&mut body, "output_dir", args.output_dir);
    insert_optional(&mut body, "output_root", args.output_root);
    insert_optional(&mut body, "source", args.source);
    body.insert("include_items".to_string(), json!(args.include_items));
    if args.include_snapshot {
        body.insert("include_snapshot".to_string(), json!(true));
    }
    Value::Object(body)
}

fn production_evidence_task_claim_body(
    project_slug: &Option<String>,
    args: ProductionEvidenceTaskClaimArgs,
) -> Value {
    let mut body = project_body(project_slug);
    body.insert("task_id".to_string(), json!(args.task_id));
    insert_optional(&mut body, "assignee", args.assignee);
    insert_optional(&mut body, "role", args.role);
    insert_optional(&mut body, "output_root", args.output_root);
    insert_optional(&mut body, "source", args.source);
    Value::Object(body)
}

fn prd_completion_package_body(
    project_slug: &Option<String>,
    args: PrdCompletionPackageArgs,
) -> Value {
    let mut body = project_body(project_slug);
    insert_optional(&mut body, "node_id", args.node_id);
    insert_optional(&mut body, "title", args.title);
    insert_optional(&mut body, "output_dir", args.output_dir);
    insert_optional(&mut body, "source", args.source);
    body.insert("include_snapshot".to_string(), json!(args.include_snapshot));
    Value::Object(body)
}

fn core_architecture_package_body(
    project_slug: &Option<String>,
    args: CoreArchitecturePackageArgs,
) -> Value {
    let mut body = project_body(project_slug);
    insert_optional(&mut body, "node_id", args.node_id);
    insert_optional(&mut body, "title", args.title);
    insert_optional(&mut body, "output_dir", args.output_dir);
    insert_optional(&mut body, "source", args.source);
    body.insert("include_snapshot".to_string(), json!(args.include_snapshot));
    Value::Object(body)
}

fn agent_session_body(project_slug: &Option<String>, args: AgentSessionArgs) -> Value {
    let mut body = project_body(project_slug);
    body.insert("kind".to_string(), Value::String(args.kind));
    insert_optional(&mut body, "control_dir", args.control_dir);
    insert_optional(&mut body, "endpoint", args.endpoint);
    insert_optional(&mut body, "instruction", args.instruction);
    if !args.allowed_tools.is_empty() {
        body.insert("allowed_tools".to_string(), json!(args.allowed_tools));
    }
    if let Some(requires_confirmation) = args.requires_confirmation {
        body.insert(
            "requires_confirmation".to_string(),
            json!(requires_confirmation),
        );
    }
    insert_optional(&mut body, "command_id", args.command_id);
    insert_optional(&mut body, "title", args.title);
    insert_optional(&mut body, "command", args.command);
    if !args.tools.is_empty() {
        body.insert("tools".to_string(), json!(args.tools));
    }
    if let Some(token_budget) = args.token_budget {
        body.insert("token_budget".to_string(), json!(token_budget));
    }
    if args.execute {
        body.insert("execute".to_string(), json!(true));
    }
    if !args.allowed_commands.is_empty() {
        body.insert("allowed_commands".to_string(), json!(args.allowed_commands));
    }
    insert_optional(&mut body, "working_dir", args.working_dir);
    if let Some(max_output_bytes) = args.max_output_bytes {
        body.insert("max_output_bytes".to_string(), json!(max_output_bytes));
    }
    if let Some(timeout_ms) = args.timeout_ms {
        body.insert("timeout_ms".to_string(), json!(timeout_ms));
    }
    Value::Object(body)
}

fn desktop_result_body(args: DesktopResultArgs) -> Value {
    let mut body = Map::new();
    body.insert(
        "software_action_id".to_string(),
        Value::String(args.software_action_id),
    );
    insert_optional(&mut body, "task_id", args.task_id);
    body.insert("status".to_string(), Value::String(args.status));
    insert_optional(&mut body, "message", args.message);
    if !args.artifacts.is_empty() {
        body.insert("artifacts".to_string(), json!(args.artifacts));
    }
    insert_optional(&mut body, "screen_trace_path", args.screen_trace_path);
    if let Some(result) = args.result {
        body.insert("result".to_string(), result);
    }
    if let Some(verification) = args.verification {
        body.insert("verification".to_string(), verification);
    }
    Value::Object(body)
}

fn desktop_run_next_body(args: DesktopRunNextArgs) -> Value {
    let mut body = Map::new();
    body.insert("status".to_string(), Value::String(args.status));
    insert_optional(&mut body, "message", args.message);
    body.insert(
        "controller_id".to_string(),
        Value::String(args.controller_id),
    );
    body.insert("limit".to_string(), json!(args.limit));
    if !args.artifacts.is_empty() {
        body.insert("artifacts".to_string(), json!(args.artifacts));
    }
    insert_optional(&mut body, "screen_trace_path", args.screen_trace_path);
    Value::Object(body)
}

fn set_api_key_body(project_slug: &Option<String>, args: SetApiKeyArgs) -> Value {
    let mut body = project_body(project_slug);
    body.insert("provider_id".to_string(), Value::String(args.provider_id));
    body.insert("service_type".to_string(), Value::String(args.service_type));
    body.insert("api_key".to_string(), Value::String(args.api_key));
    body.insert("metadata".to_string(), args.metadata);
    Value::Object(body)
}

fn insert_optional(body: &mut Map<String, Value>, key: &str, value: Option<String>) {
    if let Some(value) = value {
        body.insert(key.to_string(), Value::String(value));
    }
}

fn insert_optional_value(body: &mut Map<String, Value>, key: &str, value: Option<Value>) {
    if let Some(value) = value {
        body.insert(key.to_string(), value);
    }
}

fn insert_payload_string(payload: &mut Value, key: &str, value: &str) -> Result<()> {
    let Some(object) = payload.as_object_mut() else {
        bail!("payload must be a JSON object");
    };
    object.insert(key.to_string(), Value::String(value.to_string()));
    Ok(())
}

fn insert_payload_pair(payload: &mut Value, entry: &str) -> Result<()> {
    let (key, value) = entry
        .split_once('=')
        .context("--payload must use key=value")?;
    insert_payload_string(payload, key, value)
}

fn merge_payload_json(payload: &mut Value, raw_json: &str) -> Result<()> {
    let value = parse_json_value(raw_json, "--payload-json")?;
    let Some(target) = payload.as_object_mut() else {
        bail!("payload must be a JSON object");
    };
    let Value::Object(source) = value else {
        bail!("--payload-json must be a JSON object");
    };
    for (key, value) in source {
        target.insert(key, value);
    }
    Ok(())
}

fn parse_json_value(raw_json: &str, option_name: &str) -> Result<Value> {
    serde_json::from_str(raw_json).with_context(|| format!("{option_name} must be valid JSON"))
}

fn normalize_control_priority(value: &str) -> Result<String> {
    match normalize_token(value).as_str() {
        "api_mcp" | "apimcp" | "api" | "mcp" => Ok("ApiMcp".to_string()),
        "skills_cli" | "skillscli" | "cli" | "skills" => Ok("SkillsCli".to_string()),
        "desktop_recognition" | "desktoprecognition" | "desktop" => {
            Ok("DesktopRecognition".to_string())
        }
        "human_takeover" | "humantakeover" | "human" => Ok("HumanTakeover".to_string()),
        _ => bail!("unknown control priority: {value}"),
    }
}

fn normalize_software_action_kind(value: &str) -> Result<String> {
    match normalize_token(value).as_str() {
        "health_check" | "healthcheck" | "health" => Ok("HealthCheck".to_string()),
        "open_project" | "openproject" | "open" => Ok("OpenProject".to_string()),
        "import_asset" | "importasset" | "import" => Ok("ImportAsset".to_string()),
        "create_scene" | "createscene" | "scene" => Ok("CreateScene".to_string()),
        "run_viewport" | "runviewport" | "viewport" | "preview" => Ok("RunViewport".to_string()),
        "render" => Ok("Render".to_string()),
        "transcode" => Ok("Transcode".to_string()),
        "export_build" | "exportbuild" | "build" => Ok("ExportBuild".to_string()),
        "execute_cli" | "executecli" | "cli" => Ok("ExecuteCli".to_string()),
        "desktop_click" | "desktopclick" | "click" => Ok("DesktopClick".to_string()),
        "desktop_hotkey" | "desktophotkey" | "hotkey" => Ok("DesktopHotkey".to_string()),
        _ => bail!("unknown software action kind: {value}"),
    }
}

fn normalize_agent_session_kind(value: &str) -> Result<String> {
    match normalize_token(value).as_str() {
        "hermes" => Ok("hermes".to_string()),
        "agent_cli" | "agentcli" | "agent" | "cli" => Ok("agent_cli".to_string()),
        _ => bail!("unknown agent session kind: {value}"),
    }
}

fn normalize_agent_conformance_kind(value: &str) -> Result<String> {
    match normalize_token(value).as_str() {
        "" | "all" | "agent" | "agenthermes" | "hermesagent" => Ok("all".to_string()),
        "hermes" => Ok("hermes".to_string()),
        "agent_cli" | "agentcli" | "cli" => Ok("agent-cli".to_string()),
        _ => bail!("unknown agent conformance kind: {value}"),
    }
}

fn parse_conformance_list(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn normalize_token(value: &str) -> String {
    let mut normalized = String::new();
    let mut previous_was_separator = false;
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            normalized.push(character.to_ascii_lowercase());
            previous_was_separator = false;
        } else if !previous_was_separator {
            normalized.push('_');
            previous_was_separator = true;
        }
    }
    normalized.trim_matches('_').to_string()
}

fn resolve_api_key(direct: Option<String>, env_name: Option<String>) -> Result<Option<String>> {
    match (direct, env_name) {
        (Some(_), Some(_)) => bail!("use either --api-key or --api-key-env, not both"),
        (Some(value), None) => Ok(Some(value)),
        (None, Some(env_name)) => {
            Ok(Some(env::var(&env_name).with_context(|| {
                format!("environment variable {env_name} is not set")
            })?))
        }
        (None, None) => Ok(None),
    }
}

fn resolve_required_api_key(direct: Option<String>, env_name: Option<String>) -> Result<String> {
    resolve_api_key(direct, env_name)?.context("set-api-key requires --api-key-env or --api-key")
}

fn path_with_project(path: &str, project_slug: &Option<String>) -> String {
    match project_slug.as_deref() {
        Some(project_slug) => format!("{path}?project={}", percent_encode(project_slug)),
        None => path.to_string(),
    }
}

fn path_with_query(path: &str, pairs: &[(&str, &str)], project_slug: &Option<String>) -> String {
    let mut query = pairs
        .iter()
        .map(|(key, value)| format!("{}={}", percent_encode(key), percent_encode(value)))
        .collect::<Vec<_>>();
    if let Some(project_slug) = project_slug.as_deref() {
        query.push(format!("project={}", percent_encode(project_slug)));
    }
    format!("{path}?{}", query.join("&"))
}

fn prd_completion_gate_path(project_slug: &Option<String>, args: &PrdCompletionGateArgs) -> String {
    if args.require_complete {
        path_with_query(
            "/api/prd-completion-gate",
            &[("require_complete", "true")],
            project_slug,
        )
    } else {
        path_with_project("/api/prd-completion-gate", project_slug)
    }
}

fn core_architecture_gate_path(
    project_slug: &Option<String>,
    args: &CoreArchitectureGateArgs,
) -> String {
    if args.require_ready {
        path_with_query(
            "/api/core-architecture-gate",
            &[("require_ready", "true")],
            project_slug,
        )
    } else {
        path_with_project("/api/core-architecture-gate", project_slug)
    }
}

fn events_path(args: &EventsArgs, project_slug: &Option<String>) -> String {
    let mut query = Vec::new();
    if let Some(after_id) = args.after_id.as_deref() {
        query.push(("after_id".to_string(), after_id.to_string()));
    }
    if let Some(limit) = args.limit {
        query.push(("limit".to_string(), limit.to_string()));
    }
    path_with_owned_query("/api/events", query, project_slug)
}

fn api_keys_path(args: &ApiKeysArgs, project_slug: &Option<String>) -> String {
    let mut query = Vec::new();
    if let Some(rotation_days) = args.rotation_days {
        query.push(("rotation_days".to_string(), rotation_days.to_string()));
    }
    path_with_owned_query("/api/api-keys", query, project_slug)
}

fn agent_stream_path(args: &AgentStreamArgs, project_slug: &Option<String>) -> String {
    let mut query = vec![("session_id".to_string(), args.session_id.clone())];
    if let Some(after_id) = args.after_id.as_deref() {
        query.push(("last_event_id".to_string(), after_id.to_string()));
    }
    if let Some(limit) = args.limit {
        query.push(("limit".to_string(), limit.to_string()));
    }
    path_with_owned_query("/api/agent-sessions/stream", query, project_slug)
}

fn provider_request_metadata_path(
    provider_request_id: &str,
    project_slug: &Option<String>,
) -> String {
    path_with_query(
        "/api/provider-requests/metadata",
        &[("provider_request_id", provider_request_id)],
        project_slug,
    )
}

fn production_evidence_template_path(
    project_slug: &Option<String>,
    output_root: Option<&str>,
    source: Option<&str>,
    missing_only: bool,
) -> String {
    let mut query = Vec::new();
    if let Some(output_root) = output_root {
        query.push(("output_root".to_string(), output_root.to_string()));
    }
    if let Some(source) = source {
        query.push(("source".to_string(), source.to_string()));
    }
    if missing_only {
        query.push(("missing_only".to_string(), "true".to_string()));
    }
    path_with_owned_query("/api/production-evidence/template", query, project_slug)
}

fn production_evidence_handoff_path(
    project_slug: &Option<String>,
    output_root: Option<&str>,
    source: Option<&str>,
) -> String {
    let mut query = Vec::new();
    if let Some(output_root) = output_root {
        query.push(("output_root".to_string(), output_root.to_string()));
    }
    if let Some(source) = source {
        query.push(("source".to_string(), source.to_string()));
    }
    path_with_owned_query("/api/production-evidence/handoff", query, project_slug)
}

fn production_evidence_run_plan_path(
    project_slug: &Option<String>,
    output_root: Option<&str>,
    source: Option<&str>,
) -> String {
    let mut query = Vec::new();
    if let Some(output_root) = output_root {
        query.push(("output_root".to_string(), output_root.to_string()));
    }
    if let Some(source) = source {
        query.push(("source".to_string(), source.to_string()));
    }
    path_with_owned_query("/api/production-evidence/run-plan", query, project_slug)
}

fn production_evidence_item_template_path(
    project_slug: &Option<String>,
    output_root: Option<&str>,
    source: Option<&str>,
    task_id: Option<&str>,
    kind: Option<&str>,
    target_id: Option<&str>,
) -> String {
    let mut query = Vec::new();
    if let Some(output_root) = output_root {
        query.push(("output_root".to_string(), output_root.to_string()));
    }
    if let Some(source) = source {
        query.push(("source".to_string(), source.to_string()));
    }
    if let Some(task_id) = task_id {
        query.push(("task_id".to_string(), task_id.to_string()));
    }
    if let Some(kind) = kind {
        query.push(("kind".to_string(), kind.to_string()));
    }
    if let Some(target_id) = target_id {
        query.push(("target_id".to_string(), target_id.to_string()));
    }
    path_with_owned_query(
        "/api/production-evidence/item-template",
        query,
        project_slug,
    )
}

fn production_evidence_item_from_ledger_path(
    project_slug: &Option<String>,
    source: Option<&str>,
    provider_request_id: Option<&str>,
    software_action_id: Option<&str>,
    desktop_vision_action_id: Option<&str>,
) -> String {
    let mut query = Vec::new();
    if let Some(source) = source {
        query.push(("source".to_string(), source.to_string()));
    }
    if let Some(provider_request_id) = provider_request_id {
        query.push((
            "provider_request_id".to_string(),
            provider_request_id.to_string(),
        ));
    }
    if let Some(software_action_id) = software_action_id {
        query.push((
            "software_action_id".to_string(),
            software_action_id.to_string(),
        ));
    }
    if let Some(desktop_vision_action_id) = desktop_vision_action_id {
        query.push((
            "desktop_vision_action_id".to_string(),
            desktop_vision_action_id.to_string(),
        ));
    }
    path_with_owned_query(
        "/api/production-evidence/item-from-ledger",
        query,
        project_slug,
    )
}

fn production_evidence_bundle_from_ledger_path(
    project_slug: &Option<String>,
    source: Option<&str>,
    include_incomplete: bool,
) -> String {
    let mut query = Vec::new();
    if let Some(source) = source {
        query.push(("source".to_string(), source.to_string()));
    }
    if include_incomplete {
        query.push(("include_incomplete".to_string(), "true".to_string()));
    }
    path_with_owned_query(
        "/api/production-evidence/bundle-from-ledger",
        query,
        project_slug,
    )
}

fn cli_query_bool(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "y" | "on"
    )
}

fn provider_contracts_path(provider_id: Option<&str>) -> String {
    if let Some(provider_id) = provider_id {
        return path_with_query(
            "/api/provider-contracts",
            &[("provider_id", provider_id)],
            &None,
        );
    }
    "/api/provider-contracts".to_string()
}

fn software_contracts_path(adapter_id: Option<&str>) -> String {
    if let Some(adapter_id) = adapter_id {
        return path_with_query(
            "/api/software-contracts",
            &[("adapter_id", adapter_id)],
            &None,
        );
    }
    "/api/software-contracts".to_string()
}

fn path_with_owned_query(
    path: &str,
    pairs: Vec<(String, String)>,
    project_slug: &Option<String>,
) -> String {
    let mut query = pairs
        .iter()
        .map(|(key, value)| format!("{}={}", percent_encode(key), percent_encode(value)))
        .collect::<Vec<_>>();
    if let Some(project_slug) = project_slug.as_deref() {
        query.push(format!("project={}", percent_encode(project_slug)));
    }
    if query.is_empty() {
        path.to_string()
    } else {
        format!("{path}?{}", query.join("&"))
    }
}

fn task_action_path(kind: &TaskActionKind) -> &'static str {
    match kind {
        TaskActionKind::Approve => "/api/tasks/approve",
        TaskActionKind::Cancel => "/api/tasks/cancel",
        TaskActionKind::Retry => "/api/tasks/retry",
    }
}

fn task_action_name(kind: &TaskActionKind) -> &'static str {
    match kind {
        TaskActionKind::Approve => "approve-task",
        TaskActionKind::Cancel => "cancel-task",
        TaskActionKind::Retry => "retry-task",
    }
}

fn percent_encode(value: &str) -> String {
    value
        .bytes()
        .flat_map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                vec![byte as char]
            }
            _ => format!("%{byte:02X}").chars().collect(),
        })
        .collect()
}

fn print_help() {
    println!(
        r#"pool-cli

Local-first CLI for Pool runtime snapshots, MCP resources, workflow/node context, and node execution.

USAGE:
  pool-cli [--db <sqlite-path>] [--project <slug|*>] <command>

COMMANDS:
  status                         Read /api/health
  snapshot                       Read /api/snapshot
  projects                       Read /api/projects
  resources                      Read /api/resources
  api-keys [options]             Read /api/api-keys and credential audit
  adapters                       Read /api/adapters
  integration-readiness          Read /api/integration-readiness
  provider-contracts [id]        Read /api/provider-contracts
  provider-conformance-packages  GET /api/provider-conformance-packages
  provider-conformance-package <provider-id>
                                  POST /api/provider-conformance-packages
  integration-conformance-packages
                                  GET /api/integration-conformance-packages
  integration-conformance-package [options]
                                  POST /api/integration-conformance-packages
  agent-conformance-packages      GET /api/agent-conformance-packages
  agent-conformance-package [all|hermes|agent-cli]
                                  POST /api/agent-conformance-packages
  software-contracts [id]        Read /api/software-contracts
  software-conformance-packages  GET /api/software-conformance-packages
  software-conformance-package <adapter-id>
                                  POST /api/software-conformance-packages
  tasks                          Read pool://tasks
  events [options]               Read /api/events
  runtime-budget                 Read /api/runtime-budget
  runtime-preflight              Read /api/runtime-preflight
  runtime-execution-plan         Read /api/runtime-execution-plan
  runtime-run-next [options]     Preview or POST /api/runtime-execution-plan/run-next
  runtime-handoff                Read /api/runtime-handoff
  runtime-handoff-packages       GET /api/handoff-packages
  core-architecture-readiness    Read /api/core-architecture-readiness
  core-architecture-gate [--require-ready]
                                  Read architecture_gate from /api/core-architecture-readiness
  core-architecture-packages     GET /api/core-architecture-packages
  core-architecture-package      POST /api/core-architecture-package
  prd-readiness                  Read /api/prd-readiness
  prd-completion-gate [--require-complete]
                                  Read completion_gate from /api/prd-readiness
  prd-completion-packages        GET /api/prd-completion-packages
  prd-completion-package         POST /api/prd-completion-package
  runtime-graph                  Read /api/runtime-graph
  workflow-context [workflow-id]  Read /api/workflow-context index or workflow detail
  node-context [node-id]          Read /api/node-context index or node detail
  mcp <pool://uri>                Read /api/mcp?uri=<uri>
  serve-mcp                      Serve Pool as a newline-delimited MCP stdio server
  provider-gateway-worker-contract
                                  Read /api/provider-gateway-worker
  provider-gateway-worker         Serve local AI media/3DGS gateway HTTP forwarder
  provider-sdk-worker-template    Serve runnable upstream SDK worker scaffold
  unreal-mcp-bridge               Read /api/unreal-mcp-bridge
  unreal-mcp-bridge-worker        Serve local Unreal MCP bridge worker
  hermes-mcp-bridge-worker        Serve local Hermes MCP bridge worker
  software-api-bridge-worker      Serve generic software API/MCP bridge worker
  worker-self-checks              Run all local worker self-checks and emit JSON
  adapter-health [options]        POST /api/adapter-health
  provider-health <provider-id>   POST /api/provider-health
  run-provider <provider-id>      POST /api/provider-runs
  production-evidence-provider-matrix [output-root]
                                  Run required Provider evidence matrix and write providers[] bundle
  production-evidence-software-matrix [output-root]
                                  Run required software evidence matrix and write software_actions[] bundle
  production-evidence-desktop-vision [output-root]
                                  Callback queued desktop vision requests and write desktop_vision[] bundle
  production-evidence-requirements
                                  GET /api/production-evidence/requirements
  production-evidence-tasks       GET /api/production-evidence/tasks
  production-evidence-claim <task-id>
                                  POST /api/production-evidence/tasks/claim
  production-evidence-run-plan [run-plan.json]
                                  GET /api/production-evidence/run-plan
  production-evidence-handoff [handoff.json]
                                  GET /api/production-evidence/handoff
  production-evidence-handoff-packages
                                  GET /api/production-evidence/handoff-packages
  production-evidence-handoff-package [options]
                                  POST /api/production-evidence/handoff-packages
  production-evidence-template [bundle.json] [--missing-only]
                                  GET /api/production-evidence/template
  production-evidence-item-template <kind> <target-id> [item.json]
                                  GET /api/production-evidence/item-template
  production-evidence-item-from-ledger [options] [item.json]
                                  GET /api/production-evidence/item-from-ledger
  production-evidence-bundle-from-ledger [options] [bundle.json]
                                  GET /api/production-evidence/bundle-from-ledger
  merge-production-evidence <combined.json> <bundle.json>...
                                  Merge provider/software/desktop evidence bundles
  closeout-production-evidence [--output merged.json] [--import] [--completion-package] [--completion-package-output-dir dir] <bundle.json>...
                                  POST /api/production-evidence/closeout
  validate-production-evidence <bundle.json>
                                  POST /api/production-evidence/validate
  import-production-evidence <bundle.json>
                                  POST /api/production-evidence
  validate-production-evidence-item <item.json>
                                  POST /api/production-evidence/items/validate
  submit-production-evidence-item <item.json>
                                  POST /api/production-evidence/items
  provider-request-metadata <id>  Read /api/provider-requests/metadata
  software-health <adapter-id>    POST /api/software-health
  output-packages                 GET /api/output-packages
  run-software <adapter-id>       POST /api/software-actions
  run-node <node-id> [options]    POST /api/nodes/run
  run-workflow [options]          POST /api/workflow-runs
  output-package [options]        POST /api/output-packages
  output-result <target> [opts]   POST /api/output-packages/results
  handoff-package [options]       POST /api/handoff-packages
  agent-session <kind> [options]  POST /api/agent-sessions
  agent-transcript <session-id>   Read /api/agent-sessions/transcript
  agent-stream <session-id>       Read /api/agent-sessions/stream SSE slice
  desktop-contract                Read /api/desktop-recognition/contract
  desktop-requests               Read /api/desktop-recognition/requests
  desktop-run-next [options]      POST /api/desktop-recognition/run-next
  desktop-result <action-id>      POST /api/desktop-recognition/results
  set-api-key <provider-id>       POST /api/api-keys
  approve-task <task-id>          POST /api/tasks/approve
  cancel-task <task-id>           POST /api/tasks/cancel
  retry-task <task-id>            POST /api/tasks/retry

EVENTS OPTIONS:
  --after-id <event-id>
  --limit <count>

API-KEYS OPTIONS:
  --rotation-days <days>          Audit keys against this rotation window

AGENT-STREAM OPTIONS:
  --after-id <event-id>
  --last-event-id <event-id>
  --limit <count>

ADAPTER-HEALTH OPTIONS:
  --providers-only
  --software-only
  --no-providers
  --no-software

PROVIDER OPTIONS:
  --execution-mode <auto|mock|adapter|gateway>
  --endpoint <url>
  --api-key <key>
  --api-key-env <env>
  --prompt <text>
  --input <path>                  Repeatable, run-provider only
  --output-dir <path>             run-provider only
  --node-id <id>                  run-provider only
  --title <text>                  run-provider only
  --cost-estimate-tokens <n>      run-provider only
  --requires-approval             run-provider only
  --no-approval                   run-provider only
  --evidence-json <json-object>   run-provider only, persisted in provider_requests
  --production-upstream           run-provider only, marks evidence_json.production_upstream=true

PRODUCTION-EVIDENCE-PROVIDER-MATRIX OPTIONS:
  [output-root]                   Default worlds/<project>/output/production-evidence/provider-matrix
  --media-endpoint <url>          Gateway for Midjourney/Nano Banana Pro/Suno
  --provider-endpoint <id=url>    Repeatable endpoint override for one Provider
  --provider-endpoint-env <id=env>
                                  Repeatable endpoint env override for one Provider
  --provider-api-key <id=key>     Repeatable bearer token override for one Provider
  --provider-api-key-env <id=env> Repeatable bearer token env override for one Provider
  --provider-attestation <id=attestation>
                                  Repeatable production attestation for one Provider
  --provider-attestation-env <id=env>
                                  Repeatable production attestation env for one Provider
  --3dgs-endpoint <url>           Gateway for Marble/Tripo/SAM/Spark/Qunhe 3DGS providers
  --endpoint <url>                Use one gateway for media and 3DGS
  --openai-endpoint <url>         Default POOL_OPENAI_ENDPOINT or https://api.openai.com/v1
  --openai-api-key <key>
  --openai-api-key-env <env>
  --production-upstream           Write providers[] evidence items for successful real runs
  --production-attestation <id>   Required with --production-upstream unless env is set
  --evidence-bundle <path>        Default <output-root>/provider-production-evidence-bundle.json
  --no-env                        Ignore endpoint/key/attestation environment variables

PRODUCTION-EVIDENCE-SOFTWARE-MATRIX OPTIONS:
  [output-root]                   Default worlds/<project>/output/production-evidence/software-matrix
  --production-software           Write software_actions[] evidence items for successful real runs
  --software-endpoint <id=url>    Repeatable endpoint override for one adapter
  --software-endpoint-env <id=env>
                                  Repeatable endpoint env override for one adapter
  --software-command <id=cmd>     Repeatable CLI command override for one adapter
  --software-command-env <id=env> Repeatable CLI command env override for one adapter
  --software-artifact <id=path>   Repeatable local artifact path for one adapter
  --software-artifacts-env <id=env>
                                  Repeatable comma-separated artifact env for one adapter
  --software-attestation <id=attestation>
                                  Repeatable production attestation for one adapter
  --software-attestation-env <id=env>
                                  Repeatable production attestation env for one adapter
  --evidence-bundle <path>        Default <output-root>/software-production-evidence-bundle.json
  --no-env                        Ignore software endpoint/command/artifact/attestation env vars

PRODUCTION-EVIDENCE-DESKTOP-VISION OPTIONS:
  [output-root]                   Default worlds/<project>/output/production-evidence/desktop-vision
  --production-vision             Required to write desktop_vision[] evidence items
  --trace <path>                  Existing external visual/OCR trace JSON file
  --trace-env <env>               Env var containing external visual/OCR trace path
  --controller-id <id>            Default POOL_DESKTOP_VISION_CONTROLLER_ID or pool-cli-desktop-vision-controller
  --controller-id-env <env>       Env var containing external controller id
  --external-action-id <id>       Required unless POOL_DESKTOP_VISION_EXTERNAL_ACTION_ID is set
  --external-action-id-env <env>  Env var containing external action id
  --production-attestation <id>   Required unless POOL_DESKTOP_VISION_PRODUCTION_ATTESTATION is set
  --production-attestation-env <env>
                                  Env var containing real controller/model run attestation
  --evidence-bundle <path>        Default <output-root>/desktop-vision-production-evidence-bundle.json
  --limit <count>                 Max queued desktop requests to callback
  --no-env                        Ignore desktop vision trace/id/attestation env vars

PROVIDER-GATEWAY-WORKER OPTIONS:
  --bind <addr>                   Default 127.0.0.1:8788
  --upstream <url>                Default POOL_PROVIDER_GATEWAY_UPSTREAM or http://127.0.0.1:8787
  --provider-upstream <id=url>    Repeatable per-provider upstream route
  --max-requests <n>              Default 0, unlimited
  --once                          Run health + media/3DGS mock-forward self-check and exit
  --api-key <key>
  --api-key-env <env>
  --provider-api-key <id=key>     Repeatable per-provider bearer token
  --provider-api-key-env <id=env> Repeatable per-provider bearer token env

WORKER-SELF-CHECKS OPTIONS:
  --output-root <path>            Default target/pool-worker-self-checks
  --software-adapter <id>         Default resolve, used by software-api-bridge-worker self-check

PRODUCTION-EVIDENCE-ITEM-TEMPLATE OPTIONS:
  <kind> <target-id> [item.json]  kind: provider | software_action | desktop_vision
  --task-id <id> [item.json]      Use id from production-evidence-tasks
  --output-root <path>
  --source <value>

MERGE-PRODUCTION-EVIDENCE OPTIONS:
  --source <value>                Source label written to the merged bundle

PRODUCTION-EVIDENCE-HANDOFF-PACKAGE OPTIONS:
  --node-id <id>
  --title <text>
  --output-dir <path>             Writes control/production-evidence under this dir
  --output-root <path>            Local artifact root embedded in item templates
  --source <value>
  --include-snapshot
  --no-items                      Skip per-task item JSON files

PRODUCTION-EVIDENCE-ITEM-FROM-LEDGER OPTIONS:
  --provider-request-id <id>      Build item from provider_requests ledger
  --software-action-id <id>       Build item from software_actions ledger
  --desktop-vision-action-id <id> Build desktop_vision item from software_actions ledger
  --source <value>

PRD-COMPLETION-PACKAGE OPTIONS:
  --node-id <id>
  --title <text>
  --output-dir <path>             Writes control/prd-completion under this dir
  --source <value>
  --include-snapshot              Default
  --no-snapshot

INTEGRATION-CONFORMANCE-PACKAGE OPTIONS:
  --provider <id[,id]>            Repeatable; default all required Providers
  --software <id[,id]>            Repeatable; default all required software adapters
  --agent-kind <all|hermes|agent-cli>
  --node-id <id>
  --title <text>
  --output-dir <path>             Writes control/integration-conformance under this dir
  --no-providers
  --no-software
  --no-agent

UNREAL-MCP-BRIDGE-WORKER OPTIONS:
  --bind <addr>                   Default 127.0.0.1:8790
  --output-root <path>            Default POOL_UNREAL_MCP_BRIDGE_OUTPUT_ROOT or worlds/demo/output
  --upstream <url>                Optional real Unreal plugin/gateway endpoint
  --max-requests <n>              Default 0, unlimited
  --once                          Run health + dry-run action self-check and exit
  --api-key <key>
  --api-key-env <env>

HERMES-MCP-BRIDGE-WORKER OPTIONS:
  --bind <addr>                   Default 127.0.0.1:8792
  --output-root <path>            Default POOL_HERMES_MCP_BRIDGE_OUTPUT_ROOT or worlds/demo/output
  --upstream <url>                Optional real Hermes MCP/gateway endpoint
  --max-requests <n>              Default 0, unlimited
  --once                          Run health + dry-run action self-check and exit
  --api-key <key>
  --api-key-env <env>

SOFTWARE-API-BRIDGE-WORKER OPTIONS:
  <adapter-id>                    Required unless --adapter is supplied
  --adapter <id>
  --bind <addr>                   Default 127.0.0.1:8793
  --output-root <path>            Default POOL_SOFTWARE_API_BRIDGE_OUTPUT_ROOT or worlds/demo/output
  --upstream <url>                Optional real software plugin/gateway endpoint
  --max-requests <n>              Default 0, unlimited
  --once                          Run health + dry-run action self-check and exit
  --api-key <key>
  --api-key-env <env>

PROVIDER-CONFORMANCE-PACKAGE OPTIONS:
  <provider-id>                   Provider id, for example worldlabs-marble or midjourney
  --node-id <id>
  --title <text>
  --output-dir <path>             Writes control/provider-conformance/<provider-id> under this dir

AGENT-CONFORMANCE-PACKAGE OPTIONS:
  [all|hermes|agent-cli]          Default all
  --kind <all|hermes|agent-cli>
  --node-id <id>
  --title <text>
  --output-dir <path>             Writes control/agent-conformance/<kind> under this dir

SOFTWARE-CONFORMANCE-PACKAGE OPTIONS:
  <adapter-id>                    Software adapter id, for example resolve or unreal
  --node-id <id>
  --title <text>
  --output-dir <path>             Writes control/software-conformance/<adapter-id> under this dir

SOFTWARE OPTIONS:
  --priority <ApiMcp|SkillsCli|DesktopRecognition|HumanTakeover>
  --endpoint <url>                Adds payload_json.endpoint
  --payload <key=value>           Repeatable string payload field
  --payload-json <json-object>    Merged into payload_json
  --evidence-json <json-object>   run-software only, persisted in payload_json.evidence
  --production-software           run-software only, marks evidence_json.production_software=true
  --node-id <id>                  run-software only
  --title <text>                  run-software only
  --action-kind <kind>            run-software only
  --requires-confirmation         run-software only
  --no-confirmation               run-software only

RUN-NODE OPTIONS:
  --prompt <text>
  --execution-mode <auto|mock|adapter|gateway>
  --endpoint <url>
  --api-key <key>
  --input <path>                  Repeatable
  --output-dir <path>
  --duration-ms <ms>

RUN-WORKFLOW OPTIONS:
  --title <text>
  --prompt <text>
  --source-input <path>           Repeatable
  --output-root <path>
  --duration-ms <ms>
  --agent-mode <stage|skip|hermes_http>
  --hermes-endpoint <url>
  --hermes-auth-token <token>
  --agent-requires-confirmation
  --three-dgs-mode <auto|mock|gateway>
  --three-dgs-provider-id <id>
  --three-dgs-endpoint <url>
  --three-dgs-api-key <key>
  --unreal-mode <auto|mock|unreal_mcp>
  --unreal-endpoint <url>
  --unreal-auth-token <token>

OUTPUT-PACKAGE OPTIONS:
  --node-id <id>
  --title <text>
  --output-dir <path>
  --source-asset <path>           Repeatable
  --duration-ms <ms>

HANDOFF-PACKAGE OPTIONS:
  --node-id <id>
  --title <text>
  --output-dir <path>
  --include-snapshot

AGENT-SESSION OPTIONS:
  kind: hermes | agent_cli
  --control-dir <path>
  --endpoint <url>                Hermes only
  --instruction <text>            Hermes only
  --allowed-tool <name>           Repeatable, Hermes only
  --requires-confirmation
  --no-confirmation
  --command-id <id>               Agent CLI only
  --title <text>                  Agent CLI only
  --command <text>                Agent CLI only
  --tool <name>                   Repeatable, Agent CLI only
  --token-budget <n>              Agent CLI only
  --execute
  --allowed-command <command>     Repeatable, execute only
  --working-dir <path>            execute only
  --max-output-bytes <n>          execute only
  --timeout-ms <ms>               execute only

DESKTOP-RESULT OPTIONS:
  --task-id <id>
  --status <status>
  --message <text>
  --artifact <path>               Repeatable
  --screen-trace-path <path>
  --result-json <json>
  --verification-json <json>

DESKTOP-RUN-NEXT OPTIONS:
  --status <status>               Default succeeded
  --message <text>
  --controller-id <id>
  --limit <count>                 Default 1
  --artifact <path>               Repeatable
  --screen-trace-path <path>

SET-API-KEY OPTIONS:
  --service-type <provider|agent|software>
  --api-key-env <env>             Recommended
  --api-key <key>
  --rotation-days <days>          Store per-key rotation policy metadata
  --metadata <key=value>          Repeatable

ENV:
  POOL_RUNTIME_DB                 Default SQLite path
  POOL_PROJECT                    Default project slug
  POOL_PROVIDER_GATEWAY_UPSTREAM  Default provider-gateway-worker upstream
  POOL_PROVIDER_ENDPOINT_<ID>     Provider matrix endpoint override, e.g. POOL_PROVIDER_ENDPOINT_TRIPO_SPLAT
  POOL_<ID>_ENDPOINT              Provider matrix endpoint override alias
  POOL_PROVIDER_API_KEY_<ID>      Provider matrix bearer token override
  POOL_<ID>_API_KEY               Provider matrix bearer token override alias

MCP STDIO:
  pool-cli --db target/runtime-http-smoke/pool-runtime.sqlite --project demo serve-mcp
  Exposes pool:// resources and named tools such as pool_run_workflow,
  pool_adapters, pool_integration_readiness, pool_run_provider, pool_provider_request_metadata, pool_run_software, pool_agent_session,
  pool_agent_transcript, pool_agent_stream, pool_handoff_package, pool_provider_gateway_worker,
  pool_worker_self_checks, pool_unreal_mcp_bridge, pool_production_evidence_task_claim,
  pool_validate_production_evidence_item, pool_submit_production_evidence_item,
  pool_desktop_run_next, and pool_desktop_result.
  API key writes stay outside MCP; use set-api-key.
"#
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use pool_core::{build_default_content_burst_plan, RuntimeRepository};

    #[test]
    fn parses_node_context_command_with_runtime_options() {
        let cli = parse_cli(vec![
            "--db".to_string(),
            "runtime.sqlite".to_string(),
            "--project".to_string(),
            "demo".to_string(),
            "node-context".to_string(),
            "node-1".to_string(),
        ])
        .unwrap();

        assert_eq!(cli.db_path, PathBuf::from("runtime.sqlite"));
        assert_eq!(cli.project_slug.as_deref(), Some("demo"));
        match cli.command {
            Command::NodeContext { node_id } => assert_eq!(node_id.as_deref(), Some("node-1")),
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_workflow_context_command_with_runtime_options() {
        let cli = parse_cli(vec![
            "--db".to_string(),
            "runtime.sqlite".to_string(),
            "--project".to_string(),
            "demo".to_string(),
            "workflow-context".to_string(),
            "workflow-1".to_string(),
        ])
        .unwrap();

        assert_eq!(cli.db_path, PathBuf::from("runtime.sqlite"));
        assert_eq!(cli.project_slug.as_deref(), Some("demo"));
        match cli.command {
            Command::WorkflowContext { workflow_id } => {
                assert_eq!(workflow_id.as_deref(), Some("workflow-1"))
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_runtime_budget_command_with_runtime_options() {
        let cli = parse_cli(vec![
            "--db".to_string(),
            "runtime.sqlite".to_string(),
            "--project".to_string(),
            "demo".to_string(),
            "runtime-budget".to_string(),
        ])
        .unwrap();

        assert_eq!(cli.db_path, PathBuf::from("runtime.sqlite"));
        assert_eq!(cli.project_slug.as_deref(), Some("demo"));
        assert!(matches!(cli.command, Command::RuntimeBudget));
    }

    #[test]
    fn parses_runtime_preflight_command_with_runtime_options() {
        let cli = parse_cli(vec![
            "--db".to_string(),
            "runtime.sqlite".to_string(),
            "--project".to_string(),
            "demo".to_string(),
            "runtime-preflight".to_string(),
        ])
        .unwrap();

        assert_eq!(cli.db_path, PathBuf::from("runtime.sqlite"));
        assert_eq!(cli.project_slug.as_deref(), Some("demo"));
        assert!(matches!(cli.command, Command::RuntimePreflight));
    }

    #[test]
    fn parses_runtime_execution_plan_command_with_runtime_options() {
        let cli = parse_cli(vec![
            "--db".to_string(),
            "runtime.sqlite".to_string(),
            "--project".to_string(),
            "demo".to_string(),
            "runtime-execution-plan".to_string(),
        ])
        .unwrap();

        assert_eq!(cli.db_path, PathBuf::from("runtime.sqlite"));
        assert_eq!(cli.project_slug.as_deref(), Some("demo"));
        assert!(matches!(cli.command, Command::RuntimeExecutionPlan));
    }

    #[test]
    fn parses_runtime_run_next_command_with_runtime_options() {
        let cli = parse_cli(vec![
            "--db".to_string(),
            "runtime.sqlite".to_string(),
            "--project".to_string(),
            "demo".to_string(),
            "runtime-run-next".to_string(),
            "--node-id".to_string(),
            "node-1".to_string(),
            "--execute".to_string(),
            "--allow-approval".to_string(),
            "--execution-mode".to_string(),
            "mock".to_string(),
        ])
        .unwrap();

        assert_eq!(cli.db_path, PathBuf::from("runtime.sqlite"));
        assert_eq!(cli.project_slug.as_deref(), Some("demo"));
        match cli.command {
            Command::RuntimeExecutionPlanRunNext(args) => {
                assert_eq!(args.node_id.as_deref(), Some("node-1"));
                assert!(args.execute);
                assert!(args.allow_approval);
                assert_eq!(args.execution_mode.as_deref(), Some("mock"));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_runtime_handoff_command_with_runtime_options() {
        let cli = parse_cli(vec![
            "--db".to_string(),
            "runtime.sqlite".to_string(),
            "--project".to_string(),
            "demo".to_string(),
            "runtime-handoff".to_string(),
        ])
        .unwrap();

        assert_eq!(cli.db_path, PathBuf::from("runtime.sqlite"));
        assert_eq!(cli.project_slug.as_deref(), Some("demo"));
        assert!(matches!(cli.command, Command::RuntimeHandoff));
    }

    #[test]
    fn parses_runtime_handoff_packages_command_with_runtime_options() {
        let cli = parse_cli(vec![
            "--db".to_string(),
            "runtime.sqlite".to_string(),
            "--project".to_string(),
            "demo".to_string(),
            "handoff-packages".to_string(),
        ])
        .unwrap();

        assert_eq!(cli.db_path, PathBuf::from("runtime.sqlite"));
        assert_eq!(cli.project_slug.as_deref(), Some("demo"));
        assert!(matches!(cli.command, Command::RuntimeHandoffPackages));
    }

    #[test]
    fn parses_closeout_production_evidence_command_with_output_and_import_flag() {
        let cli = parse_cli(vec![
            "--db".to_string(),
            "runtime.sqlite".to_string(),
            "--project".to_string(),
            "demo".to_string(),
            "closeout-production-evidence".to_string(),
            "--source".to_string(),
            "operator-closeout".to_string(),
            "--output".to_string(),
            "merged.json".to_string(),
            "--import".to_string(),
            "--completion-package-output-dir".to_string(),
            "worlds/demo/output".to_string(),
            "--completion-package-node-id".to_string(),
            "agent".to_string(),
            "--completion-package-title".to_string(),
            "Final PRD proof".to_string(),
            "--completion-package-source".to_string(),
            "cli-closeout".to_string(),
            "--no-completion-package-snapshot".to_string(),
            "provider.json".to_string(),
            "software.json".to_string(),
        ])
        .unwrap();

        assert_eq!(cli.db_path, PathBuf::from("runtime.sqlite"));
        assert_eq!(cli.project_slug.as_deref(), Some("demo"));
        match cli.command {
            Command::CloseoutProductionEvidence(args) => {
                assert_eq!(args.source.as_deref(), Some("operator-closeout"));
                assert_eq!(args.output_path.as_deref(), Some("merged.json"));
                assert!(args.import);
                assert!(args.completion_package);
                assert_eq!(
                    args.completion_package_output_dir.as_deref(),
                    Some("worlds/demo/output")
                );
                assert_eq!(args.completion_package_node_id.as_deref(), Some("agent"));
                assert_eq!(
                    args.completion_package_title.as_deref(),
                    Some("Final PRD proof")
                );
                assert_eq!(
                    args.completion_package_source.as_deref(),
                    Some("cli-closeout")
                );
                assert!(!args.completion_package_include_snapshot);
                assert_eq!(args.input_paths, vec!["provider.json", "software.json"]);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_adapters_command_with_runtime_options() {
        let cli = parse_cli(vec![
            "--db".to_string(),
            "runtime.sqlite".to_string(),
            "--project".to_string(),
            "demo".to_string(),
            "adapters".to_string(),
        ])
        .unwrap();

        assert_eq!(cli.db_path, PathBuf::from("runtime.sqlite"));
        assert_eq!(cli.project_slug.as_deref(), Some("demo"));
        assert!(matches!(cli.command, Command::Adapters));
    }

    #[test]
    fn parses_integration_readiness_command_with_runtime_options() {
        let cli = parse_cli(vec![
            "--db".to_string(),
            "runtime.sqlite".to_string(),
            "--project".to_string(),
            "demo".to_string(),
            "integration-readiness".to_string(),
        ])
        .unwrap();

        assert_eq!(cli.db_path, PathBuf::from("runtime.sqlite"));
        assert_eq!(cli.project_slug.as_deref(), Some("demo"));
        assert!(matches!(cli.command, Command::IntegrationReadiness));
    }

    #[test]
    fn parses_provider_contracts_command() {
        let command = parse_command(vec![
            "provider-contracts".to_string(),
            "triposplat".to_string(),
        ])
        .unwrap();

        match command {
            Command::ProviderContracts { provider_id } => {
                assert_eq!(provider_id.as_deref(), Some("triposplat"));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_provider_gateway_worker_command() {
        let command = parse_command(vec![
            "provider-gateway-worker".to_string(),
            "--bind=127.0.0.1:18888".to_string(),
            "--upstream".to_string(),
            "http://127.0.0.1:18787".to_string(),
            "--provider-upstream".to_string(),
            "midjourney=http://127.0.0.1:19701".to_string(),
            "--provider-api-key".to_string(),
            "midjourney=provider-secret".to_string(),
            "--max-requests".to_string(),
            "3".to_string(),
            "--api-key".to_string(),
            "secret".to_string(),
            "--once".to_string(),
        ])
        .unwrap();

        match command {
            Command::ProviderGatewayWorker(args) => {
                assert_eq!(args.bind_addr, "127.0.0.1:18888");
                assert_eq!(args.upstream, "http://127.0.0.1:18787");
                assert_eq!(
                    args.provider_upstreams,
                    vec![(
                        "midjourney".to_string(),
                        "http://127.0.0.1:19701".to_string()
                    )]
                );
                assert_eq!(
                    args.provider_api_keys,
                    vec![("midjourney".to_string(), "provider-secret".to_string())]
                );
                assert_eq!(args.max_requests, 3);
                assert_eq!(args.api_key.as_deref(), Some("secret"));
                assert!(args.once);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_provider_sdk_worker_template_command() {
        let command = parse_command(vec![
            "provider-sdk-worker-template".to_string(),
            "--bind".to_string(),
            "127.0.0.1:18998".to_string(),
            "--output-root=target/sdk-template".to_string(),
            "--max-requests".to_string(),
            "5".to_string(),
            "--once".to_string(),
        ])
        .unwrap();

        match command {
            Command::ProviderSdkWorkerTemplate(args) => {
                assert_eq!(args.bind_addr, "127.0.0.1:18998");
                assert_eq!(args.output_root, PathBuf::from("target/sdk-template"));
                assert_eq!(args.max_requests, 5);
                assert!(args.once);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_unreal_mcp_bridge_worker_command() {
        let command = parse_command(vec![
            "unreal-mcp-bridge-worker".to_string(),
            "--bind=127.0.0.1:18890".to_string(),
            "--output-root".to_string(),
            "target/unreal-bridge".to_string(),
            "--upstream".to_string(),
            "http://127.0.0.1:18891".to_string(),
            "--max-requests".to_string(),
            "2".to_string(),
            "--api-key".to_string(),
            "secret".to_string(),
            "--once".to_string(),
        ])
        .unwrap();

        match command {
            Command::UnrealMcpBridgeWorker(args) => {
                assert_eq!(args.bind_addr, "127.0.0.1:18890");
                assert_eq!(args.output_root, PathBuf::from("target/unreal-bridge"));
                assert_eq!(args.upstream.as_deref(), Some("http://127.0.0.1:18891"));
                assert_eq!(args.max_requests, 2);
                assert_eq!(args.api_key.as_deref(), Some("secret"));
                assert!(args.once);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_hermes_mcp_bridge_worker_command() {
        let command = parse_command(vec![
            "hermes-mcp-bridge-worker".to_string(),
            "--bind=127.0.0.1:18892".to_string(),
            "--output-root".to_string(),
            "target/hermes-bridge".to_string(),
            "--upstream".to_string(),
            "http://127.0.0.1:18893".to_string(),
            "--max-requests".to_string(),
            "2".to_string(),
            "--api-key".to_string(),
            "secret".to_string(),
            "--once".to_string(),
        ])
        .unwrap();

        match command {
            Command::HermesMcpBridgeWorker(args) => {
                assert_eq!(args.bind_addr, "127.0.0.1:18892");
                assert_eq!(args.output_root, PathBuf::from("target/hermes-bridge"));
                assert_eq!(args.upstream.as_deref(), Some("http://127.0.0.1:18893"));
                assert_eq!(args.max_requests, 2);
                assert_eq!(args.api_key.as_deref(), Some("secret"));
                assert!(args.once);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_software_api_bridge_worker_command() {
        let command = parse_command(vec![
            "software-api-bridge-worker".to_string(),
            "resolve".to_string(),
            "--bind=127.0.0.1:18894".to_string(),
            "--output-root".to_string(),
            "target/software-bridge".to_string(),
            "--upstream".to_string(),
            "http://127.0.0.1:18895".to_string(),
            "--max-requests".to_string(),
            "2".to_string(),
            "--api-key".to_string(),
            "secret".to_string(),
            "once".to_string(),
        ])
        .unwrap();

        match command {
            Command::SoftwareApiBridgeWorker(args) => {
                assert_eq!(args.adapter_id, "resolve");
                assert_eq!(args.bind_addr, "127.0.0.1:18894");
                assert_eq!(args.output_root, PathBuf::from("target/software-bridge"));
                assert_eq!(args.upstream.as_deref(), Some("http://127.0.0.1:18895"));
                assert_eq!(args.max_requests, 2);
                assert_eq!(args.api_key.as_deref(), Some("secret"));
                assert!(args.once);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_worker_self_checks_command() {
        let command = parse_command(vec![
            "worker-self-checks".to_string(),
            "--output-root".to_string(),
            "target/worker-checks".to_string(),
            "--software-adapter=blender".to_string(),
        ])
        .unwrap();

        match command {
            Command::WorkerSelfChecks(args) => {
                assert_eq!(args.output_root, PathBuf::from("target/worker-checks"));
                assert_eq!(args.software_adapter_id, "blender");
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_software_contracts_command() {
        let command =
            parse_command(vec!["software-contracts".to_string(), "unreal".to_string()]).unwrap();

        match command {
            Command::SoftwareContracts { adapter_id } => {
                assert_eq!(adapter_id.as_deref(), Some("unreal"));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_software_conformance_package_command() {
        let command = parse_command(vec![
            "software-conformance-package".to_string(),
            "resolve".to_string(),
            "--node-id".to_string(),
            "software-node".to_string(),
            "--title".to_string(),
            "Resolve conformance".to_string(),
            "--output-dir".to_string(),
            "worlds/demo/output".to_string(),
        ])
        .unwrap();

        match command {
            Command::SoftwareConformancePackage(args) => {
                assert_eq!(args.adapter_id, "resolve");
                assert_eq!(args.node_id.as_deref(), Some("software-node"));
                assert_eq!(args.title.as_deref(), Some("Resolve conformance"));
                assert_eq!(args.output_dir.as_deref(), Some("worlds/demo/output"));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_software_conformance_packages_command() {
        let command = parse_command(vec!["software-conformance-packages".to_string()]).unwrap();

        assert!(matches!(command, Command::SoftwareConformancePackages));
    }

    #[test]
    fn parses_provider_conformance_package_command() {
        let command = parse_command(vec![
            "provider-conformance-package".to_string(),
            "worldlabs-marble".to_string(),
            "--node-id".to_string(),
            "provider-node".to_string(),
            "--title".to_string(),
            "Marble conformance".to_string(),
            "--output-dir".to_string(),
            "worlds/demo/output".to_string(),
        ])
        .unwrap();

        match command {
            Command::ProviderConformancePackage(args) => {
                assert_eq!(args.provider_id, "worldlabs-marble");
                assert_eq!(args.node_id.as_deref(), Some("provider-node"));
                assert_eq!(args.title.as_deref(), Some("Marble conformance"));
                assert_eq!(args.output_dir.as_deref(), Some("worlds/demo/output"));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_provider_conformance_packages_command() {
        let command = parse_command(vec!["provider-conformance-packages".to_string()]).unwrap();

        assert!(matches!(command, Command::ProviderConformancePackages));
    }

    #[test]
    fn parses_integration_conformance_package_command() {
        let command = parse_command(vec![
            "integration-conformance-package".to_string(),
            "--provider".to_string(),
            "worldlabs-marble,midjourney".to_string(),
            "--software".to_string(),
            "resolve".to_string(),
            "--agent-kind".to_string(),
            "agent-cli".to_string(),
            "--node-id".to_string(),
            "agent-node".to_string(),
            "--title".to_string(),
            "Integration conformance".to_string(),
            "--output-dir".to_string(),
            "worlds/demo/output".to_string(),
        ])
        .unwrap();

        match command {
            Command::IntegrationConformancePackage(args) => {
                assert_eq!(args.providers, vec!["worldlabs-marble", "midjourney"]);
                assert_eq!(args.software_adapters, vec!["resolve"]);
                assert_eq!(args.agent_kind.as_deref(), Some("agent-cli"));
                assert_eq!(args.node_id.as_deref(), Some("agent-node"));
                assert_eq!(args.title.as_deref(), Some("Integration conformance"));
                assert_eq!(args.output_dir.as_deref(), Some("worlds/demo/output"));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_integration_conformance_packages_command() {
        let command = parse_command(vec!["integration-conformance-packages".to_string()]).unwrap();

        assert!(matches!(command, Command::IntegrationConformancePackages));
    }

    #[test]
    fn parses_agent_conformance_package_command() {
        let command = parse_command(vec![
            "agent-conformance-package".to_string(),
            "agent-cli".to_string(),
            "--node-id".to_string(),
            "agent-node".to_string(),
            "--title".to_string(),
            "Agent conformance".to_string(),
            "--output-dir".to_string(),
            "worlds/demo/output".to_string(),
        ])
        .unwrap();

        match command {
            Command::AgentConformancePackage(args) => {
                assert_eq!(args.kind, "agent-cli");
                assert_eq!(args.node_id.as_deref(), Some("agent-node"));
                assert_eq!(args.title.as_deref(), Some("Agent conformance"));
                assert_eq!(args.output_dir.as_deref(), Some("worlds/demo/output"));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_agent_conformance_packages_command() {
        let command = parse_command(vec!["agent-conformance-packages".to_string()]).unwrap();

        assert!(matches!(command, Command::AgentConformancePackages));
    }

    #[test]
    fn parses_handoff_package_command_with_runtime_options() {
        let cli = parse_cli(vec![
            "--db".to_string(),
            "runtime.sqlite".to_string(),
            "--project".to_string(),
            "demo".to_string(),
            "handoff-package".to_string(),
            "--node-id".to_string(),
            "agent".to_string(),
            "--title".to_string(),
            "Runtime handoff package".to_string(),
            "--output-dir".to_string(),
            "worlds/demo/output".to_string(),
            "--include-snapshot".to_string(),
        ])
        .unwrap();

        assert_eq!(cli.db_path, PathBuf::from("runtime.sqlite"));
        assert_eq!(cli.project_slug.as_deref(), Some("demo"));
        match cli.command {
            Command::HandoffPackage(args) => {
                assert_eq!(args.node_id.as_deref(), Some("agent"));
                assert_eq!(args.title.as_deref(), Some("Runtime handoff package"));
                assert_eq!(args.output_dir.as_deref(), Some("worlds/demo/output"));
                assert!(args.include_snapshot);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn builds_run_node_body_with_optional_fields() {
        let body = run_node_body(
            &Some("demo".to_string()),
            RunNodeArgs {
                node_id: "three-dgs".to_string(),
                prompt: Some("convert plate".to_string()),
                execution_mode: Some("mock".to_string()),
                endpoint: None,
                api_key: None,
                input_paths: vec!["worlds/demo/source/0-reference.png".to_string()],
                output_dir: Some("worlds/demo/output".to_string()),
                duration_ms: Some(12_000),
            },
        );

        assert_eq!(body["project_slug"], "demo");
        assert_eq!(body["node_id"], "three-dgs");
        assert_eq!(body["execution_mode"], "mock");
        assert_eq!(body["input_paths"][0], "worlds/demo/source/0-reference.png");
        assert_eq!(body["duration_ms"], 12_000);
    }

    #[test]
    fn parses_provider_health_with_connection_overrides() {
        let command = parse_command(vec![
            "provider-health".to_string(),
            "openai-image-2".to_string(),
            "--execution-mode".to_string(),
            "adapter".to_string(),
            "--endpoint".to_string(),
            "http://127.0.0.1:8787".to_string(),
            "--api-key".to_string(),
            "sk-test".to_string(),
        ])
        .unwrap();

        match command {
            Command::ProviderHealth(run) => {
                assert_eq!(run.provider_id, "openai-image-2");
                assert_eq!(run.execution_mode.as_deref(), Some("adapter"));
                assert_eq!(run.endpoint.as_deref(), Some("http://127.0.0.1:8787"));
                assert_eq!(run.api_key.as_deref(), Some("sk-test"));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn builds_provider_run_body_with_approval_override() {
        let body = provider_run_body(
            &Some("demo".to_string()),
            ProviderRunArgs {
                provider_id: "world-labs-marble".to_string(),
                node_id: Some("convert-3dgs".to_string()),
                task_title: Some("CLI 3DGS smoke".to_string()),
                execution_mode: Some("mock".to_string()),
                endpoint: None,
                api_key: None,
                prompt: Some("convert reference into splat scene".to_string()),
                input_paths: vec!["worlds/demo/source/0-reference.png".to_string()],
                output_dir: Some("worlds/demo/output".to_string()),
                cost_estimate_tokens: Some(2_400),
                requires_approval: Some(false),
                evidence_json: Some(json!({
                    "source": "pool-cli-test",
                    "production_upstream": true
                })),
            },
        );

        assert_eq!(body["project_slug"], "demo");
        assert_eq!(body["provider_id"], "world-labs-marble");
        assert_eq!(body["node_id"], "convert-3dgs");
        assert_eq!(body["task_title"], "CLI 3DGS smoke");
        assert_eq!(body["execution_mode"], "mock");
        assert_eq!(body["prompt"], "convert reference into splat scene");
        assert_eq!(body["input_paths"][0], "worlds/demo/source/0-reference.png");
        assert_eq!(body["output_dir"], "worlds/demo/output");
        assert_eq!(body["cost_estimate_tokens"], 2_400);
        assert_eq!(body["requires_approval"], false);
        assert_eq!(body["evidence_json"]["source"], "pool-cli-test");
        assert_eq!(body["evidence_json"]["production_upstream"], true);
    }

    #[test]
    fn parses_provider_run_evidence_options() {
        let command = parse_command(vec![
            "run-provider".to_string(),
            "midjourney".to_string(),
            "--execution-mode".to_string(),
            "gateway".to_string(),
            "--evidence-json".to_string(),
            r#"{"source":"agent-smoke","evidence_mode":"configured_gateway"}"#.to_string(),
            "--production-upstream".to_string(),
        ])
        .unwrap();

        match command {
            Command::RunProvider(run) => {
                let evidence = run.evidence_json.unwrap();
                assert_eq!(evidence["source"], "agent-smoke");
                assert_eq!(evidence["evidence_mode"], "configured_gateway");
                assert_eq!(evidence["production_upstream"], true);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_provider_evidence_provider_matrix_command() {
        let command = parse_command(vec![
            "production-evidence-provider-matrix".to_string(),
            "target/provider-matrix".to_string(),
            "--endpoint".to_string(),
            "http://127.0.0.1:8788".to_string(),
            "--provider-endpoint".to_string(),
            "triposplat=http://127.0.0.1:9712".to_string(),
            "--provider-api-key".to_string(),
            "triposplat=tripo-secret".to_string(),
            "--provider-attestation".to_string(),
            "triposplat=real-tripo-worker-run-001".to_string(),
            "--openai-api-key".to_string(),
            "test-openai-key".to_string(),
            "--production-upstream".to_string(),
            "--production-attestation".to_string(),
            "real-provider-worker-run-001".to_string(),
            "--evidence-bundle".to_string(),
            "target/provider-bundle.json".to_string(),
            "--no-env".to_string(),
        ])
        .unwrap();

        match command {
            Command::ProductionEvidenceProviderMatrix(args) => {
                assert_eq!(args.output_root.as_deref(), Some("target/provider-matrix"));
                assert_eq!(
                    args.media_endpoint.as_deref(),
                    Some("http://127.0.0.1:8788")
                );
                assert_eq!(
                    args.three_dgs_endpoint.as_deref(),
                    Some("http://127.0.0.1:8788")
                );
                assert_eq!(
                    args.provider_endpoints,
                    vec![(
                        "triposplat".to_string(),
                        "http://127.0.0.1:9712".to_string()
                    )]
                );
                assert_eq!(
                    args.provider_api_keys,
                    vec![("triposplat".to_string(), "tripo-secret".to_string())]
                );
                assert_eq!(
                    args.provider_attestations,
                    vec![(
                        "triposplat".to_string(),
                        "real-tripo-worker-run-001".to_string()
                    )]
                );
                assert!(args.production_upstream);
                assert_eq!(
                    args.production_attestation.as_deref(),
                    Some("real-provider-worker-run-001")
                );
                assert_eq!(
                    args.evidence_bundle_path.as_deref(),
                    Some("target/provider-bundle.json")
                );
                assert!(!args.use_env);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn provider_evidence_provider_matrix_skips_unconfigured_targets_without_env() {
        let output_root = temp_cli_dir("provider-evidence-matrix");
        let response = dispatch(Cli {
            db_path: temp_cli_db_path("provider-evidence-matrix"),
            project_slug: Some("demo".to_string()),
            command: Command::ProductionEvidenceProviderMatrix(
                ProviderEvidenceProviderMatrixArgs {
                    output_root: Some(output_root.to_string_lossy().into_owned()),
                    media_endpoint: None,
                    provider_endpoints: Vec::new(),
                    provider_api_keys: Vec::new(),
                    provider_attestations: Vec::new(),
                    openai_endpoint: None,
                    openai_api_key: None,
                    three_dgs_endpoint: None,
                    evidence_bundle_path: None,
                    production_upstream: false,
                    production_attestation: None,
                    use_env: false,
                },
            ),
        })
        .unwrap();
        let value: Value = serde_json::from_str(&response.body).unwrap();
        let bundle_path = output_root.join("provider-production-evidence-bundle.json");
        let written: Value =
            serde_json::from_str(&fs::read_to_string(&bundle_path).unwrap()).unwrap();

        assert_eq!(response.status_code, 200);
        assert_eq!(value["kind"], "pool_provider_production_evidence_matrix");
        assert_eq!(value["summary"]["total"], 9);
        assert_eq!(value["summary"]["succeeded"], 0);
        assert_eq!(value["summary"]["skipped"], 9);
        assert_eq!(value["bundle"]["providers"].as_array().unwrap().len(), 0);
        assert_eq!(written["providers"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn provider_evidence_matrix_resolves_provider_specific_endpoint_aliases() {
        let endpoint = provider_matrix_provider_endpoint(
            &[(
                "triposplat".to_string(),
                "http://127.0.0.1:9712".to_string(),
            )],
            "tripo-splat",
            false,
        );

        assert_eq!(endpoint.as_deref(), Some("http://127.0.0.1:9712"));
        assert_eq!(
            provider_matrix_provider_endpoint_env_candidates("nano-banana-pro"),
            vec![
                "POOL_PROVIDER_ENDPOINT_NANO_BANANA_PRO".to_string(),
                "POOL_NANO_BANANA_PRO_ENDPOINT".to_string()
            ]
        );
        assert_eq!(
            provider_matrix_provider_api_key(
                &[("tripo".to_string(), "tripo-secret".to_string())],
                "tripo-splat",
                false,
            )
            .as_deref(),
            Some("tripo-secret")
        );
        assert_eq!(
            provider_matrix_provider_api_key_env_candidates("tripo-splat"),
            vec![
                "POOL_PROVIDER_API_KEY_TRIPO_SPLAT".to_string(),
                "POOL_TRIPO_SPLAT_API_KEY".to_string()
            ]
        );
        assert_eq!(
            provider_matrix_provider_attestation(
                &[(
                    "triposplat".to_string(),
                    "real-tripo-worker-run-001".to_string()
                )],
                "tripo-splat",
                false,
                Some("real-global-provider-run-001"),
            )
            .unwrap()
            .as_deref(),
            Some("real-tripo-worker-run-001")
        );
        assert_eq!(
            provider_matrix_provider_attestation(
                &[],
                "midjourney",
                false,
                Some("real-global-provider-run-001"),
            )
            .unwrap()
            .as_deref(),
            Some("real-global-provider-run-001")
        );
        assert_eq!(
            provider_matrix_provider_attestation_env_candidates("tripo-splat"),
            vec![
                "POOL_PROVIDER_PRODUCTION_ATTESTATION_TRIPO_SPLAT".to_string(),
                "POOL_TRIPO_SPLAT_PRODUCTION_ATTESTATION".to_string()
            ]
        );
    }

    #[test]
    fn provider_evidence_metadata_redacts_inline_api_key() {
        let temp_dir = temp_cli_dir("provider-evidence-redacts-key");
        fs::create_dir_all(&temp_dir).unwrap();
        let artifact_path = temp_dir.join("1-output.png");
        fs::write(&artifact_path, b"image").unwrap();
        let item = provider_matrix_evidence_item(
            &CliProviderEvidenceTarget {
                provider_id: "midjourney",
                family: CliProviderEvidenceFamily::Media,
            },
            "http://127.0.0.1:8788",
            &json!({
                "provider_id": "midjourney",
                "api_key": "provider-secret",
            }),
            &json!({
                "provider_request_id": "provider-request-1",
                "report": {
                    "job_id": "vendor-job-1",
                    "assets": [artifact_path.to_string_lossy()]
                }
            }),
            &temp_dir,
            "real-vendor-sdk-worker-2026-06-17",
        )
        .unwrap();
        let metadata_path = item["metadata_path"].as_str().unwrap();
        let metadata = fs::read_to_string(metadata_path).unwrap();

        assert!(metadata.contains("\"api_key\": \"[redacted]\""));
        assert!(!metadata.contains("provider-secret"));
    }

    #[test]
    fn parses_production_evidence_software_matrix_command() {
        let command = parse_command(vec![
            "production-evidence-software-matrix".to_string(),
            "target/software-matrix".to_string(),
            "--production-software".to_string(),
            "--software-endpoint".to_string(),
            "unreal=http://127.0.0.1:8790".to_string(),
            "--software-command".to_string(),
            "davinci-resolve=/bin/echo resolve-ok".to_string(),
            "--software-artifact".to_string(),
            "resolve=target/resolve-output.mov".to_string(),
            "--software-attestation".to_string(),
            "resolve=real-resolve-run-001".to_string(),
            "--evidence-bundle".to_string(),
            "target/software-bundle.json".to_string(),
            "--no-env".to_string(),
        ])
        .unwrap();

        match command {
            Command::ProductionEvidenceSoftwareMatrix(args) => {
                assert_eq!(args.output_root.as_deref(), Some("target/software-matrix"));
                assert!(args.production_software);
                assert_eq!(
                    args.software_endpoints,
                    vec![("unreal".to_string(), "http://127.0.0.1:8790".to_string())]
                );
                assert_eq!(
                    args.software_commands,
                    vec![(
                        "davinci-resolve".to_string(),
                        "/bin/echo resolve-ok".to_string()
                    )]
                );
                assert_eq!(
                    args.software_artifacts,
                    vec![(
                        "resolve".to_string(),
                        "target/resolve-output.mov".to_string()
                    )]
                );
                assert_eq!(
                    args.software_attestations,
                    vec![("resolve".to_string(), "real-resolve-run-001".to_string())]
                );
                assert_eq!(
                    args.evidence_bundle_path.as_deref(),
                    Some("target/software-bundle.json")
                );
                assert!(!args.use_env);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn software_evidence_matrix_without_env_writes_empty_bundle() {
        let output_root = temp_cli_dir("software-evidence-matrix");
        let response = dispatch(Cli {
            db_path: temp_cli_db_path("software-evidence-matrix"),
            project_slug: Some("demo".to_string()),
            command: Command::ProductionEvidenceSoftwareMatrix(SoftwareEvidenceMatrixArgs {
                output_root: Some(output_root.to_string_lossy().into_owned()),
                software_endpoints: Vec::new(),
                software_commands: Vec::new(),
                software_artifacts: Vec::new(),
                software_attestations: Vec::new(),
                evidence_bundle_path: None,
                production_software: true,
                use_env: false,
            }),
        })
        .unwrap();
        let value: Value = serde_json::from_str(&response.body).unwrap();
        let bundle_path = output_root.join("software-production-evidence-bundle.json");
        let written: Value =
            serde_json::from_str(&fs::read_to_string(&bundle_path).unwrap()).unwrap();

        assert_eq!(response.status_code, 200);
        assert_eq!(value["kind"], "pool_software_production_evidence_matrix");
        assert_eq!(value["summary"]["total"], 11);
        assert_eq!(value["summary"]["succeeded"], 0);
        assert_eq!(value["summary"]["failed"], 11);
        assert_eq!(
            value["bundle"]["software_actions"]
                .as_array()
                .unwrap()
                .len(),
            0
        );
        assert_eq!(written["software_actions"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn software_matrix_body_uses_per_adapter_overrides_and_aliases() {
        let artifact_path = temp_cli_dir("software-matrix-overrides").join("resolve-output.mov");
        let endpoints = vec![("unreal".to_string(), "http://127.0.0.1:8790".to_string())];
        let commands = vec![(
            "davinci-resolve".to_string(),
            "/bin/echo resolve-ok".to_string(),
        )];
        let artifacts = vec![(
            "resolve".to_string(),
            artifact_path.to_string_lossy().into_owned(),
        )];
        let attestations = vec![("resolve".to_string(), "real-resolve-run-001".to_string())];
        let overrides = SoftwareMatrixOverrides {
            endpoints: &endpoints,
            commands: &commands,
            artifacts: &artifacts,
            attestations: &attestations,
        };
        let body = cli_software_matrix_body(
            "demo",
            "resolve",
            "production_software",
            true,
            &overrides,
            &|_| None,
        );

        assert_eq!(body["adapter_id"], "resolve");
        assert_eq!(body["payload_json"]["command"], "/bin/echo resolve-ok");
        assert_eq!(
            body["payload_json"]["artifacts"],
            json!([artifact_path.to_string_lossy()])
        );
        assert_eq!(body["evidence_json"]["production_software"], true);
        assert_eq!(body["evidence_json"]["configured_real_software"], true);
        assert_eq!(
            body["evidence_json"]["production_attestation"],
            "real-resolve-run-001"
        );
    }

    #[test]
    fn software_matrix_body_uses_generic_endpoint_when_configured() {
        let artifact_path = temp_cli_dir("software-matrix-endpoint").join("resolve-output.mov");
        let endpoints = vec![(
            "davinci-resolve".to_string(),
            "http://127.0.0.1:8791".to_string(),
        )];
        let commands = Vec::new();
        let artifacts = vec![(
            "resolve".to_string(),
            artifact_path.to_string_lossy().into_owned(),
        )];
        let attestations = vec![(
            "resolve".to_string(),
            "real-resolve-api-run-001".to_string(),
        )];
        let overrides = SoftwareMatrixOverrides {
            endpoints: &endpoints,
            commands: &commands,
            artifacts: &artifacts,
            attestations: &attestations,
        };
        let body = cli_software_matrix_body(
            "demo",
            "resolve",
            "production_software",
            true,
            &overrides,
            &|_| None,
        );

        assert_eq!(body["adapter_id"], "resolve");
        assert_eq!(body["action_kind"], "CreateScene");
        assert_eq!(body["priority"], "ApiMcp");
        assert_eq!(body["payload_json"]["endpoint"], "http://127.0.0.1:8791");
        assert_eq!(body["evidence_json"]["control_profile"], "api_mcp");
        assert_eq!(body["evidence_json"]["configured_real_software"], true);
        assert_eq!(
            body["payload_json"]["artifacts"],
            json!([artifact_path.to_string_lossy()])
        );
    }

    #[test]
    fn parses_production_evidence_desktop_vision_command() {
        let command = parse_command(vec![
            "production-evidence-desktop-vision".to_string(),
            "target/desktop-vision".to_string(),
            "--production-vision".to_string(),
            "--trace".to_string(),
            "target/desktop-vision/trace.json".to_string(),
            "--trace-env".to_string(),
            "POOL_TEST_DESKTOP_TRACE".to_string(),
            "--controller-id".to_string(),
            "external-vision-controller".to_string(),
            "--controller-id-env".to_string(),
            "POOL_TEST_DESKTOP_CONTROLLER".to_string(),
            "--external-action-id".to_string(),
            "external-action-1".to_string(),
            "--external-action-id-env".to_string(),
            "POOL_TEST_DESKTOP_ACTION".to_string(),
            "--production-attestation".to_string(),
            "real-vision-run-1".to_string(),
            "--production-attestation-env".to_string(),
            "POOL_TEST_DESKTOP_ATTESTATION".to_string(),
            "--evidence-bundle".to_string(),
            "target/desktop-vision-bundle.json".to_string(),
            "--limit".to_string(),
            "3".to_string(),
            "--no-env".to_string(),
        ])
        .unwrap();

        match command {
            Command::ProductionEvidenceDesktopVision(args) => {
                assert_eq!(args.output_root.as_deref(), Some("target/desktop-vision"));
                assert!(args.production_vision);
                assert_eq!(
                    args.trace_path.as_deref(),
                    Some("target/desktop-vision/trace.json")
                );
                assert_eq!(args.trace_env.as_deref(), Some("POOL_TEST_DESKTOP_TRACE"));
                assert_eq!(
                    args.controller_id.as_deref(),
                    Some("external-vision-controller")
                );
                assert_eq!(
                    args.controller_id_env.as_deref(),
                    Some("POOL_TEST_DESKTOP_CONTROLLER")
                );
                assert_eq!(
                    args.external_action_id.as_deref(),
                    Some("external-action-1")
                );
                assert_eq!(
                    args.external_action_id_env.as_deref(),
                    Some("POOL_TEST_DESKTOP_ACTION")
                );
                assert_eq!(
                    args.production_attestation.as_deref(),
                    Some("real-vision-run-1")
                );
                assert_eq!(
                    args.production_attestation_env.as_deref(),
                    Some("POOL_TEST_DESKTOP_ATTESTATION")
                );
                assert_eq!(
                    args.evidence_bundle_path.as_deref(),
                    Some("target/desktop-vision-bundle.json")
                );
                assert_eq!(args.limit, 3);
                assert!(!args.use_env);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn desktop_vision_evidence_callbacks_queue_and_writes_bundle() {
        let db_path = temp_cli_db_path("desktop-vision-evidence");
        let output_root = temp_cli_dir("desktop-vision-evidence");
        let trace_path = output_root.join("external-vision-trace.json");
        write_json_value(
            &trace_path,
            &json!({
                "visual_model": "external",
                "source": "test external vision controller",
            }),
        )
        .unwrap();

        dispatch(Cli {
            db_path: db_path.clone(),
            project_slug: Some("demo".to_string()),
            command: Command::RunSoftware(SoftwareActionArgs {
                adapter_id: "touchdesigner".to_string(),
                node_id: None,
                task_title: Some("TouchDesigner external vision cue".to_string()),
                action_kind: Some("RunViewport".to_string()),
                priority: Some("DesktopRecognition".to_string()),
                payload_json: json!({"target_window":"TouchDesigner"}),
                evidence_json: None,
                requires_confirmation: Some(false),
            }),
        })
        .unwrap();

        let response = dispatch(Cli {
            db_path,
            project_slug: Some("demo".to_string()),
            command: Command::ProductionEvidenceDesktopVision(DesktopVisionEvidenceArgs {
                output_root: Some(output_root.to_string_lossy().into_owned()),
                evidence_bundle_path: None,
                production_vision: true,
                trace_path: Some(trace_path.to_string_lossy().into_owned()),
                trace_env: None,
                controller_id: Some("external-vision-controller".to_string()),
                controller_id_env: None,
                external_action_id: Some("external-action-1".to_string()),
                external_action_id_env: None,
                production_attestation: Some("real-vision-run-1".to_string()),
                production_attestation_env: None,
                limit: 1,
                use_env: false,
            }),
        })
        .unwrap();
        let value: Value = serde_json::from_str(&response.body).unwrap();
        let bundle_path = output_root.join("desktop-vision-production-evidence-bundle.json");
        let written: Value =
            serde_json::from_str(&fs::read_to_string(&bundle_path).unwrap()).unwrap();

        assert_eq!(response.status_code, 200);
        assert_eq!(value["kind"], "pool_desktop_vision_production_evidence");
        assert_eq!(value["summary"]["queued"], 1);
        assert_eq!(value["summary"]["succeeded"], 1);
        assert_eq!(
            written["desktop_vision"][0]["external_action_id"],
            "external-action-1"
        );
        assert_eq!(
            written["desktop_vision"][0]["production_attestation"],
            "real-vision-run-1"
        );
        assert_eq!(written["desktop_vision"][0]["visual_model"], "external");
    }

    #[test]
    fn parses_provider_request_metadata_command() {
        let command = parse_command(vec![
            "provider-request-metadata".to_string(),
            "provider-request-1".to_string(),
        ])
        .unwrap();

        match command {
            Command::ProviderRequestMetadata {
                provider_request_id,
            } => {
                assert_eq!(provider_request_id, "provider-request-1");
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_validate_production_evidence_command() {
        let command = parse_command(vec![
            "validate-production-evidence".to_string(),
            "docs/examples/production-evidence-bundle.example.json".to_string(),
        ])
        .unwrap();

        match command {
            Command::ValidateProductionEvidence { path } => {
                assert_eq!(
                    path,
                    "docs/examples/production-evidence-bundle.example.json"
                );
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_validate_production_evidence_item_command() {
        let command = parse_command(vec![
            "validate-production-evidence-item".to_string(),
            "target/evidence/provider-item.json".to_string(),
        ])
        .unwrap();

        match command {
            Command::ValidateProductionEvidenceItem { path } => {
                assert_eq!(path, "target/evidence/provider-item.json");
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_submit_production_evidence_item_command() {
        let command = parse_command(vec![
            "submit-production-evidence-item".to_string(),
            "target/evidence/provider-item.json".to_string(),
        ])
        .unwrap();

        match command {
            Command::SubmitProductionEvidenceItem { path } => {
                assert_eq!(path, "target/evidence/provider-item.json");
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_production_evidence_requirements_command() {
        let command = parse_command(vec!["production-evidence-requirements".to_string()]).unwrap();

        match command {
            Command::ProductionEvidenceRequirements => {}
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_production_evidence_tasks_command() {
        let command = parse_command(vec!["production-evidence-tasks".to_string()]).unwrap();

        match command {
            Command::ProductionEvidenceTasks => {}
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_production_evidence_task_claim_command() {
        let command = parse_command(vec![
            "production-evidence-claim".to_string(),
            "provider:midjourney:production_upstream".to_string(),
            "--assignee=worker-1".to_string(),
            "--role".to_string(),
            "provider_worker".to_string(),
            "--output-root".to_string(),
            "target/evidence".to_string(),
            "--source=agent-claim".to_string(),
        ])
        .unwrap();

        match command {
            Command::ProductionEvidenceTaskClaim(args) => {
                assert_eq!(args.task_id, "provider:midjourney:production_upstream");
                assert_eq!(args.assignee.as_deref(), Some("worker-1"));
                assert_eq!(args.role.as_deref(), Some("provider_worker"));
                assert_eq!(args.output_root.as_deref(), Some("target/evidence"));
                assert_eq!(args.source.as_deref(), Some("agent-claim"));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_production_evidence_run_plan_command() {
        let command = parse_command(vec![
            "production-evidence-run-plan".to_string(),
            "target/evidence/run-plan.json".to_string(),
            "--output-root=target/evidence".to_string(),
            "--source".to_string(),
            "agent-run-plan".to_string(),
        ])
        .unwrap();

        match command {
            Command::ProductionEvidenceRunPlan(args) => {
                assert_eq!(args.path.as_deref(), Some("target/evidence/run-plan.json"));
                assert_eq!(args.output_root.as_deref(), Some("target/evidence"));
                assert_eq!(args.source.as_deref(), Some("agent-run-plan"));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_production_evidence_handoff_command() {
        let command = parse_command(vec![
            "production-evidence-handoff".to_string(),
            "target/evidence/handoff.json".to_string(),
            "--output-root".to_string(),
            "target/evidence".to_string(),
            "--source=operator-handoff".to_string(),
        ])
        .unwrap();

        match command {
            Command::ProductionEvidenceHandoff(args) => {
                assert_eq!(args.path.as_deref(), Some("target/evidence/handoff.json"));
                assert_eq!(args.output_root.as_deref(), Some("target/evidence"));
                assert_eq!(args.source.as_deref(), Some("operator-handoff"));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_production_evidence_handoff_packages_command() {
        let command =
            parse_command(vec!["production-evidence-handoff-packages".to_string()]).unwrap();

        assert!(matches!(
            command,
            Command::ProductionEvidenceHandoffPackages
        ));
    }

    #[test]
    fn parses_production_evidence_handoff_package_command() {
        let command = parse_command(vec![
            "production-evidence-handoff-package".to_string(),
            "--node-id".to_string(),
            "agent".to_string(),
            "--output-dir".to_string(),
            "worlds/demo/output".to_string(),
            "--output-root".to_string(),
            "worlds/demo/output/production-evidence".to_string(),
            "--source=external-worker".to_string(),
            "--include-snapshot".to_string(),
            "--no-items".to_string(),
        ])
        .unwrap();

        match command {
            Command::ProductionEvidenceHandoffPackage(args) => {
                assert_eq!(args.node_id.as_deref(), Some("agent"));
                assert_eq!(args.output_dir.as_deref(), Some("worlds/demo/output"));
                assert_eq!(
                    args.output_root.as_deref(),
                    Some("worlds/demo/output/production-evidence")
                );
                assert_eq!(args.source.as_deref(), Some("external-worker"));
                assert!(args.include_snapshot);
                assert!(!args.include_items);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_production_evidence_template_command() {
        let command = parse_command(vec![
            "production-evidence-template".to_string(),
            "target/evidence/bundle.json".to_string(),
            "--output-root".to_string(),
            "target/evidence".to_string(),
            "--source=external-worker-handoff".to_string(),
            "--missing-only".to_string(),
        ])
        .unwrap();

        match command {
            Command::ProductionEvidenceTemplate(args) => {
                assert_eq!(args.path.as_deref(), Some("target/evidence/bundle.json"));
                assert_eq!(args.output_root.as_deref(), Some("target/evidence"));
                assert_eq!(args.source.as_deref(), Some("external-worker-handoff"));
                assert!(args.missing_only);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_production_evidence_item_template_command() {
        let command = parse_command(vec![
            "production-evidence-item-template".to_string(),
            "provider".to_string(),
            "midjourney".to_string(),
            "target/evidence/midjourney-item.json".to_string(),
            "--output-root".to_string(),
            "target/evidence".to_string(),
            "--source=provider-worker".to_string(),
        ])
        .unwrap();

        match command {
            Command::ProductionEvidenceItemTemplate(args) => {
                assert_eq!(args.kind.as_deref(), Some("provider"));
                assert_eq!(args.target_id.as_deref(), Some("midjourney"));
                assert_eq!(
                    args.path.as_deref(),
                    Some("target/evidence/midjourney-item.json")
                );
                assert_eq!(args.output_root.as_deref(), Some("target/evidence"));
                assert_eq!(args.source.as_deref(), Some("provider-worker"));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_production_evidence_item_template_task_id_form() {
        let command = parse_command(vec![
            "production-evidence-item-template".to_string(),
            "--task-id".to_string(),
            "software:unreal:production_software".to_string(),
            "target/evidence/unreal-item.json".to_string(),
        ])
        .unwrap();

        match command {
            Command::ProductionEvidenceItemTemplate(args) => {
                assert_eq!(
                    args.task_id.as_deref(),
                    Some("software:unreal:production_software")
                );
                assert_eq!(
                    args.path.as_deref(),
                    Some("target/evidence/unreal-item.json")
                );
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_production_evidence_item_from_ledger_command() {
        let command = parse_command(vec![
            "production-evidence-item-from-ledger".to_string(),
            "--provider-request-id".to_string(),
            "provider-request-1".to_string(),
            "--source=runtime-ledger-test".to_string(),
            "target/evidence/provider-item.json".to_string(),
        ])
        .unwrap();

        match command {
            Command::ProductionEvidenceItemFromLedger(args) => {
                assert_eq!(
                    args.provider_request_id.as_deref(),
                    Some("provider-request-1")
                );
                assert!(args.software_action_id.is_none());
                assert!(args.desktop_vision_action_id.is_none());
                assert_eq!(args.source.as_deref(), Some("runtime-ledger-test"));
                assert_eq!(
                    args.path.as_deref(),
                    Some("target/evidence/provider-item.json")
                );
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_production_evidence_item_from_desktop_vision_ledger_command() {
        let command = parse_command(vec![
            "production-evidence-item-from-ledger".to_string(),
            "--desktop-vision-action-id=desktop-action-1".to_string(),
            "target/evidence/desktop-item.json".to_string(),
        ])
        .unwrap();

        match command {
            Command::ProductionEvidenceItemFromLedger(args) => {
                assert_eq!(
                    args.desktop_vision_action_id.as_deref(),
                    Some("desktop-action-1")
                );
                assert!(args.provider_request_id.is_none());
                assert!(args.software_action_id.is_none());
                assert_eq!(
                    args.path.as_deref(),
                    Some("target/evidence/desktop-item.json")
                );
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_production_evidence_bundle_from_ledger_command() {
        let command = parse_command(vec![
            "production-evidence-bundle-from-ledger".to_string(),
            "--source=runtime ledger".to_string(),
            "--include-incomplete".to_string(),
            "target/evidence/ledger-bundle.json".to_string(),
        ])
        .unwrap();

        match command {
            Command::ProductionEvidenceBundleFromLedger(args) => {
                assert_eq!(args.source.as_deref(), Some("runtime ledger"));
                assert!(args.include_incomplete);
                assert_eq!(
                    args.path.as_deref(),
                    Some("target/evidence/ledger-bundle.json")
                );
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_merge_production_evidence_command() {
        let command = parse_command(vec![
            "merge-production-evidence".to_string(),
            "target/evidence/combined.json".to_string(),
            "target/evidence/providers.json".to_string(),
            "target/evidence/software.json".to_string(),
            "--source=operator-closeout".to_string(),
        ])
        .unwrap();

        match command {
            Command::MergeProductionEvidence(args) => {
                assert_eq!(args.output_path, "target/evidence/combined.json");
                assert_eq!(
                    args.input_paths,
                    vec![
                        "target/evidence/providers.json".to_string(),
                        "target/evidence/software.json".to_string()
                    ]
                );
                assert_eq!(args.source.as_deref(), Some("operator-closeout"));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn merges_production_evidence_bundles() {
        let bundles = vec![
            (
                "provider.json".to_string(),
                json!({
                    "source": "provider-runner",
                    "project_slug": "demo",
                    "providers": [{"provider_id": "midjourney", "external_job_id": "mj-real-1", "production_attestation": "midjourney-worker-merge-run-1"}],
                    "software_actions": [],
                    "desktop_vision": [],
                }),
            ),
            (
                "software.json".to_string(),
                json!({
                    "source": "software-runner",
                    "project_slug": "demo",
                    "providers": [],
                    "software_actions": [{"adapter_id": "unreal", "external_action_id": "ue-real-1", "production_attestation": "unreal-software-cli-merge-run-1"}],
                    "desktop_vision": [{"adapter_id": "touchdesigner", "trace_path": "worlds/demo/output/vision.json"}],
                }),
            ),
        ];

        let merged = merge_production_evidence_bundle_values(
            &Some("demo".to_string()),
            Some("closeout"),
            &bundles,
        )
        .unwrap();

        assert_eq!(merged["source"], "closeout");
        assert_eq!(merged["project_slug"], "demo");
        assert_eq!(merged["providers"].as_array().unwrap().len(), 1);
        assert_eq!(merged["software_actions"].as_array().unwrap().len(), 1);
        assert_eq!(merged["desktop_vision"].as_array().unwrap().len(), 1);
        assert_eq!(merged["merge"]["input_count"], 2);
        assert_eq!(
            production_evidence_bundle_summary(&merged),
            json!({"providers":1,"software_actions":1,"desktop_vision":1})
        );
    }

    #[test]
    fn merge_production_evidence_rejects_conflicting_project_slug() {
        let bundles = vec![
            (
                "a.json".to_string(),
                json!({"project_slug":"demo","providers":[]}),
            ),
            (
                "b.json".to_string(),
                json!({"project_slug":"other","providers":[]}),
            ),
        ];

        let error = merge_production_evidence_bundle_values(&None, None, &bundles)
            .expect_err("conflicting project slugs should fail");

        assert!(error.to_string().contains("conflicting project_slug"));
    }

    #[test]
    fn builds_provider_request_metadata_path_with_project_filter() {
        let path = provider_request_metadata_path("provider request 1", &Some("demo".to_string()));

        assert_eq!(
            path,
            "/api/provider-requests/metadata?provider_request_id=provider%20request%201&project=demo"
        );
    }

    #[test]
    fn builds_production_evidence_template_path_with_options() {
        assert_eq!(
            production_evidence_template_path(
                &Some("demo".to_string()),
                Some("target/prod evidence"),
                Some("external worker"),
                true,
            ),
            "/api/production-evidence/template?output_root=target%2Fprod%20evidence&source=external%20worker&missing_only=true&project=demo"
        );
    }

    #[test]
    fn builds_production_evidence_handoff_path_with_options() {
        assert_eq!(
            production_evidence_handoff_path(
                &Some("demo".to_string()),
                Some("target/prod evidence"),
                Some("external worker"),
            ),
            "/api/production-evidence/handoff?output_root=target%2Fprod%20evidence&source=external%20worker&project=demo"
        );
    }

    #[test]
    fn builds_production_evidence_run_plan_path_with_options() {
        assert_eq!(
            production_evidence_run_plan_path(
                &Some("demo".to_string()),
                Some("target/prod evidence"),
                Some("external worker"),
            ),
            "/api/production-evidence/run-plan?output_root=target%2Fprod%20evidence&source=external%20worker&project=demo"
        );
    }

    #[test]
    fn builds_production_evidence_item_template_path_with_task_id() {
        assert_eq!(
            production_evidence_item_template_path(
                &Some("demo".to_string()),
                Some("target/prod evidence"),
                Some("external worker"),
                Some("provider:midjourney:production_upstream"),
                None,
                None,
            ),
            "/api/production-evidence/item-template?output_root=target%2Fprod%20evidence&source=external%20worker&task_id=provider%3Amidjourney%3Aproduction_upstream&project=demo"
        );
    }

    #[test]
    fn builds_production_evidence_item_from_ledger_path_with_provider_request() {
        assert_eq!(
            production_evidence_item_from_ledger_path(
                &Some("demo".to_string()),
                Some("runtime ledger"),
                Some("provider request 1"),
                None,
                None,
            ),
            "/api/production-evidence/item-from-ledger?source=runtime%20ledger&provider_request_id=provider%20request%201&project=demo"
        );
    }

    #[test]
    fn builds_production_evidence_item_from_ledger_path_with_desktop_vision_action() {
        assert_eq!(
            production_evidence_item_from_ledger_path(
                &Some("demo".to_string()),
                Some("runtime ledger"),
                None,
                None,
                Some("desktop action 1"),
            ),
            "/api/production-evidence/item-from-ledger?source=runtime%20ledger&desktop_vision_action_id=desktop%20action%201&project=demo"
        );
    }

    #[test]
    fn builds_production_evidence_bundle_from_ledger_path() {
        assert_eq!(
            production_evidence_bundle_from_ledger_path(
                &Some("demo".to_string()),
                Some("runtime ledger"),
                true,
            ),
            "/api/production-evidence/bundle-from-ledger?source=runtime%20ledger&include_incomplete=true&project=demo"
        );
    }

    #[test]
    fn builds_production_evidence_item_template_path_with_kind_target() {
        assert_eq!(
            production_evidence_item_template_path(
                &Some("demo".to_string()),
                None,
                None,
                None,
                Some("software_action"),
                Some("unreal"),
            ),
            "/api/production-evidence/item-template?kind=software_action&target_id=unreal&project=demo"
        );
    }

    #[test]
    fn builds_provider_contracts_path_with_provider_filter() {
        assert_eq!(
            provider_contracts_path(Some("tripo splat")),
            "/api/provider-contracts?provider_id=tripo%20splat"
        );
        assert_eq!(provider_contracts_path(None), "/api/provider-contracts");
    }

    #[test]
    fn builds_software_contracts_path_with_adapter_filter() {
        assert_eq!(
            software_contracts_path(Some("Unreal Editor")),
            "/api/software-contracts?adapter_id=Unreal%20Editor"
        );
        assert_eq!(software_contracts_path(None), "/api/software-contracts");
    }

    #[test]
    fn provider_contracts_dispatch_reads_gateway_contract() {
        let response = dispatch(Cli {
            db_path: temp_cli_db_path("provider-contracts"),
            project_slug: Some("demo".to_string()),
            command: Command::ProviderContracts {
                provider_id: Some("mj".to_string()),
            },
        })
        .unwrap();
        let value: Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 200);
        assert_eq!(value["provider_id"], "midjourney");
        assert_eq!(value["adapter_kind"], "generic_http_media_gateway");
    }

    #[test]
    fn software_contracts_dispatch_reads_adapter_contract() {
        let response = dispatch(Cli {
            db_path: temp_cli_db_path("software-contracts"),
            project_slug: Some("demo".to_string()),
            command: Command::SoftwareContracts {
                adapter_id: Some("unreal".to_string()),
            },
        })
        .unwrap();
        let value: Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 200);
        assert_eq!(value["adapter_id"], "unreal");
        assert_eq!(value["runtime_action"]["path"], "/api/software-actions");
        assert_eq!(value["control_routes"][0]["adapter_kind"], "unreal_mcp");
    }

    #[test]
    fn integration_readiness_dispatch_reads_snapshot_matrix() {
        let db_path = temp_cli_db_path("integration-readiness");
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        let plan = build_default_content_burst_plan("demo", "Pool readiness CLI");
        repository.persist_plan(&plan).unwrap();
        drop(repository);

        let response = dispatch(Cli {
            db_path,
            project_slug: Some("demo".to_string()),
            command: Command::IntegrationReadiness,
        })
        .unwrap();
        let value: Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 200);
        assert_eq!(value["kind"], "pool_integration_readiness");
        assert!(value["summary"]["providers"].as_u64().unwrap() >= 9);
        assert_eq!(value["summary"]["lanes"], 5);
        assert!(value["run_plan"].as_array().unwrap().iter().any(|item| {
            item["lane"] == "spatial_engine"
                && item["action"]["command"]
                    .as_str()
                    .unwrap_or_default()
                    .contains("set-api-key")
        }));
        assert!(value["providers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|provider| {
                provider["provider_id"] == "worldlabs-marble"
                    && provider["lane"] == "spatial_engine"
                    && provider["commands"]["health"]
                        .as_str()
                        .unwrap_or_default()
                        .contains("provider-health worldlabs-marble")
            }));
    }

    #[test]
    fn software_conformance_package_dispatch_writes_local_package() {
        let output_dir = temp_cli_dir("software-conformance-package");
        let response = dispatch(Cli {
            db_path: temp_cli_db_path("software-conformance-package"),
            project_slug: Some("demo".to_string()),
            command: Command::SoftwareConformancePackage(SoftwareConformancePackageArgs {
                adapter_id: "resolve".to_string(),
                node_id: Some("resolve-node".to_string()),
                title: None,
                output_dir: Some(output_dir.to_string_lossy().to_string()),
            }),
        })
        .unwrap();
        let value: Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 201);
        assert_eq!(value["report"]["adapter_id"], "resolve");
        assert!(value["report"]["paths"]["runner_script"]
            .as_str()
            .unwrap()
            .ends_with("4-software-conformance-runner.sh"));
        assert!(Path::new(value["report"]["paths"]["preflight"].as_str().unwrap()).exists());
        let runner =
            fs::read_to_string(value["report"]["paths"]["runner_script"].as_str().unwrap())
                .unwrap();
        assert!(runner.contains("software-api-bridge-worker resolve"));
        assert!(runner.contains("production-evidence-software-matrix"));
        assert_eq!(value["task"]["status"], "Succeeded");
    }

    #[test]
    fn provider_conformance_package_dispatch_writes_local_package() {
        let output_dir = temp_cli_dir("provider-conformance-package");
        let response = dispatch(Cli {
            db_path: temp_cli_db_path("provider-conformance-package"),
            project_slug: Some("demo".to_string()),
            command: Command::ProviderConformancePackage(ProviderConformancePackageArgs {
                provider_id: "world-labs-marble".to_string(),
                node_id: Some("provider-node".to_string()),
                title: None,
                output_dir: Some(output_dir.to_string_lossy().to_string()),
            }),
        })
        .unwrap();
        let value: Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 201);
        assert_eq!(value["report"]["provider_id"], "worldlabs-marble");
        assert!(value["report"]["paths"]["runner_script"]
            .as_str()
            .unwrap()
            .ends_with("5-provider-conformance-runner.sh"));
        assert!(Path::new(
            value["report"]["paths"]["gateway_worker_contract"]
                .as_str()
                .unwrap()
        )
        .exists());
        let runner =
            fs::read_to_string(value["report"]["paths"]["runner_script"].as_str().unwrap())
                .unwrap();
        assert!(runner.contains("provider-gateway-worker --once"));
        assert!(runner.contains("production-evidence-provider-matrix"));
        assert!(runner.contains("POOL_WORLDLABS_MARBLE_UPSTREAM_ENDPOINT"));
        assert_eq!(value["task"]["status"], "Succeeded");
    }

    #[test]
    fn agent_conformance_package_dispatch_writes_local_package() {
        let output_dir = temp_cli_dir("agent-conformance-package");
        let response = dispatch(Cli {
            db_path: temp_cli_db_path("agent-conformance-package"),
            project_slug: Some("demo".to_string()),
            command: Command::AgentConformancePackage(AgentConformancePackageArgs {
                kind: "all".to_string(),
                node_id: Some("agent-node".to_string()),
                title: None,
                output_dir: Some(output_dir.to_string_lossy().to_string()),
            }),
        })
        .unwrap();
        let value: Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 201);
        assert_eq!(value["report"]["session_kind"], "all");
        assert!(value["report"]["paths"]["runner_script"]
            .as_str()
            .unwrap()
            .ends_with("4-agent-conformance-runner.sh"));
        assert!(Path::new(value["report"]["paths"]["runbook"].as_str().unwrap()).exists());
        let runner =
            fs::read_to_string(value["report"]["paths"]["runner_script"].as_str().unwrap())
                .unwrap();
        assert!(runner.contains("agent-session hermes"));
        assert!(runner.contains("agent-session agent-cli"));
        assert!(runner.contains("hermes-mcp-bridge-worker --once"));
        assert_eq!(value["task"]["status"], "Succeeded");
    }

    #[test]
    fn integration_conformance_package_dispatch_writes_local_package() {
        let output_dir = temp_cli_dir("integration-conformance-package");
        let response = dispatch(Cli {
            db_path: temp_cli_db_path("integration-conformance-package"),
            project_slug: Some("demo".to_string()),
            command: Command::IntegrationConformancePackage(IntegrationConformancePackageArgs {
                node_id: Some("agent-node".to_string()),
                title: None,
                output_dir: Some(output_dir.to_string_lossy().to_string()),
                providers: vec!["worldlabs-marble".to_string()],
                software_adapters: vec!["resolve".to_string()],
                agent_kind: Some("all".to_string()),
                include_providers: true,
                include_software: true,
                include_agent: true,
            }),
        })
        .unwrap();
        let value: Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 201);
        assert_eq!(
            value["report"]["kind"],
            "pool_integration_conformance_package_report"
        );
        assert_eq!(value["report"]["summary"]["providers"], 1);
        assert_eq!(value["report"]["summary"]["software_adapters"], 1);
        assert!(value["report"]["paths"]["runner_script"]
            .as_str()
            .unwrap()
            .ends_with("2-integration-conformance-runner.sh"));
        let runner =
            fs::read_to_string(value["report"]["paths"]["runner_script"].as_str().unwrap())
                .unwrap();
        assert!(runner.contains("providers/worldlabs-marble/5-provider-conformance-runner.sh"));
        assert!(runner.contains("software/resolve/4-software-conformance-runner.sh"));
        assert!(runner.contains("agent/all/4-agent-conformance-runner.sh"));
        assert_eq!(value["task"]["status"], "Succeeded");
    }

    #[test]
    fn unreal_mcp_bridge_dispatch_reads_bridge_contract() {
        let response = dispatch(Cli {
            db_path: temp_cli_db_path("unreal-mcp-bridge"),
            project_slug: Some("demo".to_string()),
            command: Command::UnrealMcpBridgeContract,
        })
        .unwrap();
        let value: Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 200);
        assert_eq!(value["kind"], "pool_unreal_mcp_bridge_contract");
        assert!(value["tool_contracts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|tool| tool["tool"] == "unreal.render_sequence"));
    }

    #[test]
    fn production_evidence_template_dispatch_writes_bundle_file() {
        let output_path = std::env::temp_dir().join(format!(
            "pool-cli-production-evidence-template-{}.json",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let response = dispatch(Cli {
            db_path: temp_cli_db_path("production-evidence-template"),
            project_slug: Some("demo".to_string()),
            command: Command::ProductionEvidenceTemplate(ProductionEvidenceTemplateArgs {
                path: Some(output_path.to_string_lossy().into_owned()),
                output_root: Some("target/prod".to_string()),
                source: Some("external-worker-handoff".to_string()),
                missing_only: false,
            }),
        })
        .unwrap();
        let value: Value = serde_json::from_str(&response.body).unwrap();
        let written: Value =
            serde_json::from_str(&fs::read_to_string(&output_path).unwrap()).unwrap();

        assert_eq!(response.status_code, 200);
        assert_eq!(value["kind"], "pool_production_evidence_bundle_template");
        assert_eq!(
            value["written_bundle_path"].as_str(),
            Some(output_path.to_string_lossy().as_ref())
        );
        assert_eq!(written["project_slug"], "demo");
        assert_eq!(written["providers"].as_array().unwrap().len(), 9);
        assert!(written["providers"][0]["metadata_path"]
            .as_str()
            .unwrap()
            .starts_with("target/prod/worlds/demo/output/production/"));
        let _ = fs::remove_file(output_path);
    }

    #[test]
    fn production_evidence_handoff_dispatch_writes_full_handoff_file() {
        let output_path = std::env::temp_dir().join(format!(
            "pool-cli-production-evidence-handoff-{}.json",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let response = dispatch(Cli {
            db_path: temp_cli_db_path("production-evidence-handoff"),
            project_slug: Some("demo".to_string()),
            command: Command::ProductionEvidenceHandoff(ProductionEvidenceHandoffArgs {
                path: Some(output_path.to_string_lossy().into_owned()),
                output_root: Some("target/prod".to_string()),
                source: Some("external-worker-handoff".to_string()),
            }),
        })
        .unwrap();
        let value: Value = serde_json::from_str(&response.body).unwrap();
        let written: Value =
            serde_json::from_str(&fs::read_to_string(&output_path).unwrap()).unwrap();

        assert_eq!(response.status_code, 200);
        assert_eq!(value["kind"], "pool_production_evidence_handoff");
        assert_eq!(
            value["written_handoff_path"].as_str(),
            Some(output_path.to_string_lossy().as_ref())
        );
        assert_eq!(written["kind"], "pool_production_evidence_handoff");
        assert_eq!(written["project_slug"], "demo");
        assert_eq!(written["bundle"]["providers"].as_array().unwrap().len(), 9);
        let _ = fs::remove_file(output_path);
    }

    #[test]
    fn production_evidence_item_template_dispatch_writes_item_file() {
        let output_path = std::env::temp_dir().join(format!(
            "pool-cli-production-evidence-item-template-{}.json",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let response = dispatch(Cli {
            db_path: temp_cli_db_path("production-evidence-item-template"),
            project_slug: Some("demo".to_string()),
            command: Command::ProductionEvidenceItemTemplate(ProductionEvidenceItemTemplateArgs {
                path: Some(output_path.to_string_lossy().into_owned()),
                output_root: Some("target/prod".to_string()),
                source: Some("external-worker".to_string()),
                task_id: Some("provider:midjourney:production_upstream".to_string()),
                kind: None,
                target_id: None,
            }),
        })
        .unwrap();
        let value: Value = serde_json::from_str(&response.body).unwrap();
        let written: Value =
            serde_json::from_str(&fs::read_to_string(&output_path).unwrap()).unwrap();

        assert_eq!(response.status_code, 200);
        assert_eq!(value["kind"], "pool_production_evidence_item_template");
        assert_eq!(
            value["written_item_path"].as_str(),
            Some(output_path.to_string_lossy().as_ref())
        );
        assert_eq!(written["project_slug"], "demo");
        assert_eq!(written["kind"], "provider");
        assert_eq!(written["provider"]["provider_id"], "midjourney");
        assert!(written["provider"]["metadata_path"]
            .as_str()
            .unwrap()
            .starts_with("target/prod/worlds/demo/output/production/midjourney/"));
        let _ = fs::remove_file(output_path);
    }

    #[test]
    fn merge_production_evidence_dispatch_writes_combined_bundle() {
        let root = std::env::temp_dir().join(format!(
            "pool-cli-production-evidence-merge-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let provider_path = root.join("provider.json");
        let software_path = root.join("software.json");
        let output_path = root.join("combined.json");
        fs::write(
            &provider_path,
            serde_json::to_string_pretty(&json!({
                "source": "provider-runner",
                "project_slug": "demo",
                "providers": [{"provider_id": "midjourney", "external_job_id": "mj-real-1", "production_attestation": "midjourney-worker-cli-merge-run-1"}],
                "software_actions": [],
                "desktop_vision": [],
            }))
            .unwrap(),
        )
        .unwrap();
        fs::write(
            &software_path,
            serde_json::to_string_pretty(&json!({
                "source": "software-runner",
                "project_slug": "demo",
                "providers": [],
                "software_actions": [{"adapter_id": "unreal", "external_action_id": "ue-real-1", "production_attestation": "unreal-software-cli-merge-run-1"}],
                "desktop_vision": [],
            }))
            .unwrap(),
        )
        .unwrap();

        let response = dispatch(Cli {
            db_path: temp_cli_db_path("production-evidence-merge"),
            project_slug: Some("demo".to_string()),
            command: Command::MergeProductionEvidence(ProductionEvidenceMergeArgs {
                output_path: output_path.to_string_lossy().into_owned(),
                input_paths: vec![
                    provider_path.to_string_lossy().into_owned(),
                    software_path.to_string_lossy().into_owned(),
                ],
                source: Some("operator-closeout".to_string()),
            }),
        })
        .unwrap();
        let value: Value = serde_json::from_str(&response.body).unwrap();
        let written: Value =
            serde_json::from_str(&fs::read_to_string(&output_path).unwrap()).unwrap();

        assert_eq!(response.status_code, 200);
        assert_eq!(value["kind"], "pool_production_evidence_merge");
        assert_eq!(
            value["written_bundle_path"].as_str(),
            Some(output_path.to_string_lossy().as_ref())
        );
        assert_eq!(written["source"], "operator-closeout");
        assert_eq!(written["project_slug"], "demo");
        assert_eq!(written["providers"].as_array().unwrap().len(), 1);
        assert_eq!(written["software_actions"].as_array().unwrap().len(), 1);
        assert_eq!(written["desktop_vision"].as_array().unwrap().len(), 0);

        let _ = fs::remove_file(provider_path);
        let _ = fs::remove_file(software_path);
        let _ = fs::remove_file(output_path);
        let _ = fs::remove_dir(root);
    }

    #[test]
    fn closeout_production_evidence_dispatch_validates_without_writes_and_writes_merged_bundle() {
        let root = std::env::temp_dir().join(format!(
            "pool-cli-production-evidence-closeout-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let provider_path = root.join("provider.json");
        let software_path = root.join("software.json");
        let desktop_path = root.join("desktop.json");
        let output_path = root.join("closeout-merged.json");
        fs::write(
            &provider_path,
            serde_json::to_string_pretty(&json!({
                "source": "provider-runner",
                "project_slug": "demo",
                "providers": [{
                    "provider_id": "worldlabs-marble",
                    "external_job_id": "marble-real-cli-closeout-1",
                    "production_attestation": "worldlabs-marble-worker-cli-closeout-run-1",
                    "artifacts": ["worlds/demo/output/production/marble.glb"]
                }],
                "software_actions": [],
                "desktop_vision": [],
            }))
            .unwrap(),
        )
        .unwrap();
        fs::write(
            &software_path,
            serde_json::to_string_pretty(&json!({
                "source": "software-runner",
                "project_slug": "demo",
                "providers": [],
                "software_actions": [{
                    "adapter_id": "unreal",
                    "external_action_id": "unreal-real-cli-closeout-1",
                    "production_attestation": "unreal-software-cli-closeout-run-1",
                    "action_kind": "CreateScene",
                    "priority": "ApiMcp",
                    "verification_json": {"ok": true}
                }],
                "desktop_vision": [],
            }))
            .unwrap(),
        )
        .unwrap();
        fs::write(
            &desktop_path,
            serde_json::to_string_pretty(&json!({
                "source": "vision-runner",
                "project_slug": "demo",
                "providers": [],
                "software_actions": [],
                "desktop_vision": [{
                    "adapter_id": "touchdesigner",
                    "external_action_id": "vision-real-cli-closeout-1",
                    "controller_id": "external-vision-controller",
                    "production_attestation": "external-vision-controller-cli-closeout-run-1",
                    "trace_path": "worlds/demo/output/production/vision-trace.json",
                    "visual_model": "external"
                }],
            }))
            .unwrap(),
        )
        .unwrap();
        let db_path = temp_cli_db_path("production-evidence-closeout");

        let response = dispatch(Cli {
            db_path: db_path.clone(),
            project_slug: Some("demo".to_string()),
            command: Command::CloseoutProductionEvidence(ProductionEvidenceCloseoutArgs {
                input_paths: vec![
                    provider_path.to_string_lossy().into_owned(),
                    software_path.to_string_lossy().into_owned(),
                    desktop_path.to_string_lossy().into_owned(),
                ],
                source: Some("operator-closeout".to_string()),
                import: false,
                output_path: Some(output_path.to_string_lossy().into_owned()),
                completion_package: false,
                completion_package_output_dir: None,
                completion_package_node_id: None,
                completion_package_title: None,
                completion_package_source: None,
                completion_package_include_snapshot: true,
            }),
        })
        .unwrap();
        let value: Value = serde_json::from_str(&response.body).unwrap();
        let written: Value =
            serde_json::from_str(&fs::read_to_string(&output_path).unwrap()).unwrap();

        assert_eq!(response.status_code, 200);
        assert_eq!(value["kind"], "pool_production_evidence_closeout");
        assert_eq!(value["mode"], "validate");
        assert_eq!(value["writes"], 0);
        assert_eq!(
            value["written_bundle_path"].as_str(),
            Some(output_path.to_string_lossy().as_ref())
        );
        assert_eq!(value["merge"]["summary"]["input_bundles"], 3);
        assert_eq!(value["validation"]["summary"]["providers"], 1);
        assert_eq!(value["validation"]["summary"]["software_actions"], 1);
        assert_eq!(value["validation"]["summary"]["desktop_vision"], 1);
        assert_eq!(written["source"], "operator-closeout");
        assert_eq!(written["project_slug"], "demo");
        assert_eq!(written["providers"].as_array().unwrap().len(), 1);
        assert_eq!(written["software_actions"].as_array().unwrap().len(), 1);
        assert_eq!(written["desktop_vision"].as_array().unwrap().len(), 1);

        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        assert_eq!(repository.table_count("tasks").unwrap(), 0);
        assert_eq!(repository.table_count("provider_requests").unwrap(), 0);
        assert_eq!(repository.table_count("software_actions").unwrap(), 0);
        assert_eq!(repository.table_count("workflow_events").unwrap(), 0);

        let _ = fs::remove_file(provider_path);
        let _ = fs::remove_file(software_path);
        let _ = fs::remove_file(desktop_path);
        let _ = fs::remove_file(output_path);
        let _ = fs::remove_dir(root);
    }

    #[test]
    fn closeout_production_evidence_body_includes_completion_package_request() {
        let root = std::env::temp_dir().join(format!(
            "pool-cli-production-evidence-closeout-body-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let provider_path = root.join("provider.json");
        fs::write(
            &provider_path,
            serde_json::to_string_pretty(&json!({
                "project_slug": "demo",
                "providers": [{
                    "provider_id": "midjourney",
                    "external_job_id": "mj-real-closeout-body-1",
                    "production_attestation": "midjourney-worker-cli-closeout-body-run-1",
                    "artifacts": ["worlds/demo/output/production/midjourney.png"]
                }]
            }))
            .unwrap(),
        )
        .unwrap();

        let body = production_evidence_closeout_body(
            &Some("demo".to_string()),
            &ProductionEvidenceCloseoutArgs {
                input_paths: vec![provider_path.to_string_lossy().into_owned()],
                source: Some("operator-closeout".to_string()),
                import: true,
                output_path: None,
                completion_package: true,
                completion_package_output_dir: Some("worlds/demo/output".to_string()),
                completion_package_node_id: Some("agent".to_string()),
                completion_package_title: Some("Final PRD proof".to_string()),
                completion_package_source: Some("cli-closeout".to_string()),
                completion_package_include_snapshot: false,
            },
        )
        .unwrap();

        assert_eq!(body["project_slug"], "demo");
        assert_eq!(body["import"], true);
        assert_eq!(
            body["completion_package"]["output_dir"],
            "worlds/demo/output"
        );
        assert_eq!(body["completion_package"]["node_id"], "agent");
        assert_eq!(body["completion_package"]["title"], "Final PRD proof");
        assert_eq!(body["completion_package"]["source"], "cli-closeout");
        assert_eq!(body["completion_package"]["include_snapshot"], false);
        assert_eq!(body["bundles"].as_array().unwrap().len(), 1);

        let _ = fs::remove_file(provider_path);
        let _ = fs::remove_dir(root);
    }

    #[test]
    fn runtime_execution_plan_dispatch_reads_ordered_steps() {
        let db_path = temp_cli_db_path("runtime-execution-plan");
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        repository
            .persist_plan(&build_default_content_burst_plan("demo", "CLI plan test"))
            .unwrap();

        let response = dispatch(Cli {
            db_path,
            project_slug: Some("demo".to_string()),
            command: Command::RuntimeExecutionPlan,
        })
        .unwrap();
        let value: Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 200);
        assert_eq!(value["kind"], "pool_runtime_execution_plan");
        assert!(value["summary"]["steps"].as_u64().unwrap_or_default() > 0);
        assert!(value["next_steps"]
            .as_array()
            .unwrap()
            .iter()
            .any(|step| step["control"]["recommended_action"]["command"]
                .as_str()
                .unwrap_or_default()
                .contains("pool-cli --project demo")));
    }

    #[test]
    fn prd_completion_gate_dispatch_reads_gate_and_can_require_complete() {
        let db_path = temp_cli_db_path("prd-completion-gate");
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        repository
            .persist_plan(&build_default_content_burst_plan(
                "demo",
                "CLI completion gate test",
            ))
            .unwrap();

        let response = dispatch(Cli {
            db_path: db_path.clone(),
            project_slug: Some("demo".to_string()),
            command: Command::PrdCompletionGate(PrdCompletionGateArgs {
                require_complete: false,
            }),
        })
        .unwrap();
        let value: Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 200);
        assert_eq!(value["kind"], "pool_prd_completion_gate");
        assert_eq!(value["completion_gate"]["status"], "incomplete");
        assert_eq!(value["completion_gate"]["ready_for_completion"], false);
        assert!(
            value["completion_gate"]["proof_commands"]["closeout_preflight"]
                .as_str()
                .unwrap()
                .contains("closeout-production-evidence")
        );

        let required = dispatch(Cli {
            db_path,
            project_slug: Some("demo".to_string()),
            command: Command::PrdCompletionGate(PrdCompletionGateArgs {
                require_complete: true,
            }),
        })
        .unwrap();
        let required_value: Value = serde_json::from_str(&required.body).unwrap();

        assert_eq!(required.status_code, 428);
        assert_eq!(required_value["error"], "prd_completion_gate_incomplete");
        assert_eq!(
            required_value["completion_gate"]["ready_for_completion"],
            false
        );
    }

    #[test]
    fn core_architecture_gate_dispatch_reads_gate_and_can_require_ready() {
        let db_path = temp_cli_db_path("core-architecture-gate");
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        repository
            .persist_plan(&build_default_content_burst_plan(
                "demo",
                "CLI core architecture gate test",
            ))
            .unwrap();

        let response = dispatch(Cli {
            db_path: db_path.clone(),
            project_slug: Some("demo".to_string()),
            command: Command::CoreArchitectureGate(CoreArchitectureGateArgs {
                require_ready: false,
            }),
        })
        .unwrap();
        let value: Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 200);
        assert_eq!(value["kind"], "pool_core_architecture_gate");
        assert_eq!(
            value["architecture_gate"]["ready_for_core_architecture"],
            false
        );

        let required = dispatch(Cli {
            db_path,
            project_slug: Some("demo".to_string()),
            command: Command::CoreArchitectureGate(CoreArchitectureGateArgs {
                require_ready: true,
            }),
        })
        .unwrap();
        let required_value: Value = serde_json::from_str(&required.body).unwrap();

        assert_eq!(required.status_code, 428);
        assert_eq!(required_value["error"], "core_architecture_gate_incomplete");
    }

    #[test]
    fn prd_completion_package_dispatch_writes_completion_files() {
        let db_path = temp_cli_db_path("prd-completion-package");
        let output_dir = temp_cli_dir("prd-completion-package-output");
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        repository
            .persist_plan(&build_default_content_burst_plan(
                "demo",
                "CLI completion package test",
            ))
            .unwrap();

        let response = dispatch(Cli {
            db_path: db_path.clone(),
            project_slug: Some("demo".to_string()),
            command: Command::PrdCompletionPackage(PrdCompletionPackageArgs {
                node_id: Some("agent".to_string()),
                title: Some("Completion package smoke".to_string()),
                output_dir: Some(output_dir.to_string_lossy().into_owned()),
                source: Some("cli-test".to_string()),
                include_snapshot: true,
            }),
        })
        .unwrap();
        let value: Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 201);
        assert_eq!(value["kind"], "pool_prd_completion_package");
        assert_eq!(value["report"]["ready_for_completion"], false);
        assert!(PathBuf::from(value["report"]["completion_gate_path"].as_str().unwrap()).exists());
        assert!(PathBuf::from(
            value["report"]["production_evidence_requirements_path"]
                .as_str()
                .unwrap()
        )
        .exists());
        assert!(PathBuf::from(value["report"]["manifest_path"].as_str().unwrap()).exists());
        assert_eq!(value["task"]["status"], "Succeeded");
        assert!(value["assets"].as_array().unwrap().len() >= 5);

        let catalog_response = dispatch(Cli {
            db_path,
            project_slug: Some("demo".to_string()),
            command: Command::PrdCompletionPackages,
        })
        .unwrap();
        let catalog: Value = serde_json::from_str(&catalog_response.body).unwrap();
        assert_eq!(catalog_response.status_code, 200);
        assert_eq!(catalog["kind"], "pool_prd_completion_packages");
        assert_eq!(catalog["summary"]["package_count"], 1);
        assert_eq!(
            catalog["packages"][0]["manifest_path"],
            value["report"]["manifest_path"]
        );
        assert_eq!(
            catalog["packages"][0]["ready_for_completion"],
            value["report"]["ready_for_completion"]
        );

        let _ = fs::remove_dir_all(output_dir);
    }

    #[test]
    fn core_architecture_package_dispatch_writes_proof_files() {
        let db_path = temp_cli_db_path("core-architecture-package");
        let output_dir = temp_cli_dir("core-architecture-package-output");
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        repository
            .persist_plan(&build_default_content_burst_plan(
                "demo",
                "CLI core architecture package test",
            ))
            .unwrap();

        let response = dispatch(Cli {
            db_path: db_path.clone(),
            project_slug: Some("demo".to_string()),
            command: Command::CoreArchitecturePackage(CoreArchitecturePackageArgs {
                node_id: Some("agent".to_string()),
                title: Some("Core architecture package smoke".to_string()),
                output_dir: Some(output_dir.to_string_lossy().into_owned()),
                source: Some("cli-test".to_string()),
                include_snapshot: true,
            }),
        })
        .unwrap();
        let value: Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 201);
        assert_eq!(value["kind"], "pool_core_architecture_package");
        assert_eq!(value["report"]["ready_for_core_architecture"], false);
        assert!(PathBuf::from(value["report"]["readiness_path"].as_str().unwrap()).exists());
        assert!(PathBuf::from(
            value["report"]["core_architecture_gate_path"]
                .as_str()
                .unwrap()
        )
        .exists());
        assert!(PathBuf::from(value["report"]["runtime_graph_path"].as_str().unwrap()).exists());
        assert!(PathBuf::from(
            value["report"]["strict_prd_completion_gate_path"]
                .as_str()
                .unwrap()
        )
        .exists());
        assert!(PathBuf::from(value["report"]["manifest_path"].as_str().unwrap()).exists());
        assert_eq!(value["task"]["status"], "Succeeded");
        assert!(value["assets"].as_array().unwrap().len() >= 7);

        let catalog_response = dispatch(Cli {
            db_path,
            project_slug: Some("demo".to_string()),
            command: Command::CoreArchitecturePackages,
        })
        .unwrap();
        let catalog: Value = serde_json::from_str(&catalog_response.body).unwrap();
        assert_eq!(catalog_response.status_code, 200);
        assert_eq!(catalog["kind"], "pool_core_architecture_packages");
        assert_eq!(catalog["summary"]["package_count"], 1);
        assert_eq!(catalog["summary"]["ready_packages"], 1);
        assert_eq!(
            catalog["packages"][0]["manifest_path"],
            value["report"]["manifest_path"]
        );
        assert_eq!(
            catalog["packages"][0]["core_architecture_gate_path"],
            value["report"]["core_architecture_gate_path"]
        );

        let _ = fs::remove_dir_all(output_dir);
    }

    #[test]
    fn runtime_handoff_packages_dispatch_reads_generated_catalog() {
        let db_path = temp_cli_db_path("runtime-handoff-packages");
        let output_dir = temp_cli_dir("runtime-handoff-packages-output");
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        repository
            .persist_plan(&build_default_content_burst_plan(
                "demo",
                "CLI handoff package catalog test",
            ))
            .unwrap();
        drop(repository);

        let generated = dispatch(Cli {
            db_path: db_path.clone(),
            project_slug: Some("demo".to_string()),
            command: Command::HandoffPackage(HandoffPackageArgs {
                node_id: Some("agent".to_string()),
                title: Some("CLI runtime handoff package".to_string()),
                output_dir: Some(output_dir.to_string_lossy().into_owned()),
                include_snapshot: true,
            }),
        })
        .unwrap();
        assert_eq!(generated.status_code, 201);

        let catalog_response = dispatch(Cli {
            db_path,
            project_slug: Some("demo".to_string()),
            command: Command::RuntimeHandoffPackages,
        })
        .unwrap();
        let catalog: Value = serde_json::from_str(&catalog_response.body).unwrap();

        assert_eq!(catalog_response.status_code, 200);
        assert_eq!(catalog["kind"], "pool_runtime_handoff_packages");
        assert_eq!(catalog["summary"]["package_count"], 1);
        assert_eq!(catalog["summary"]["ready_packages"], 1);
        assert!(catalog["packages"][0]["agent_entrypoint"]["mcp_stdio"]
            .as_str()
            .unwrap()
            .contains("serve-mcp"));

        let _ = fs::remove_dir_all(output_dir);
    }

    #[test]
    fn runtime_run_next_dispatch_previews_selected_step() {
        let db_path = temp_cli_db_path("runtime-run-next");
        let repository = RuntimeRepository::open(&db_path).unwrap();
        repository.migrate().unwrap();
        let plan = build_default_content_burst_plan("demo", "CLI run next test");
        let node_id = plan.workflow.nodes.values().next().unwrap().id.clone();
        repository.persist_plan(&plan).unwrap();

        let response = dispatch(Cli {
            db_path,
            project_slug: Some("demo".to_string()),
            command: Command::RuntimeExecutionPlanRunNext(RuntimeRunNextArgs {
                node_id: Some(node_id.clone()),
                ..RuntimeRunNextArgs::default()
            }),
        })
        .unwrap();
        let value: Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 200);
        assert_eq!(value["mode"], "preview");
        assert_eq!(value["executed"], false);
        assert_eq!(value["selected_step"]["node_id"], node_id);
    }

    #[test]
    fn provider_request_metadata_dispatch_reads_registered_handoff() {
        let db_path = temp_cli_db_path("provider-request-metadata");
        let project_slug = Some("demo".to_string());
        let queued = dispatch(Cli {
            db_path: db_path.clone(),
            project_slug: project_slug.clone(),
            command: Command::RunProvider(ProviderRunArgs {
                provider_id: "world-labs-marble".to_string(),
                node_id: None,
                task_title: Some("CLI approval handoff".to_string()),
                execution_mode: Some("mock".to_string()),
                endpoint: None,
                api_key: None,
                prompt: Some("convert plate".to_string()),
                input_paths: Vec::new(),
                output_dir: Some(
                    "target/pool-cli-provider-request-metadata/worlds/demo/output".to_string(),
                ),
                cost_estimate_tokens: None,
                requires_approval: None,
                evidence_json: None,
            }),
        })
        .unwrap();
        let queued_value: Value = serde_json::from_str(&queued.body).unwrap();
        let provider_request_id = queued_value["provider_request_id"].as_str().unwrap();

        let metadata = dispatch(Cli {
            db_path,
            project_slug,
            command: Command::ProviderRequestMetadata {
                provider_request_id: provider_request_id.to_string(),
            },
        })
        .unwrap();
        let metadata_value: Value = serde_json::from_str(&metadata.body).unwrap();

        assert_eq!(metadata.status_code, 200);
        assert_eq!(metadata_value["provider_request_id"], provider_request_id);
        assert_eq!(metadata_value["provider_id"], "worldlabs-marble");
        assert_eq!(
            metadata_value["metadata"]["kind"],
            "pool_provider_approval_handoff"
        );
    }

    #[test]
    fn parses_set_api_key_with_env_metadata() {
        let command = parse_command(vec![
            "set-api-key".to_string(),
            "openai-image-2".to_string(),
            "--service-type".to_string(),
            "provider".to_string(),
            "--api-key-env".to_string(),
            "PATH".to_string(),
            "--rotation-days".to_string(),
            "30".to_string(),
            "--metadata".to_string(),
            "owner=local-smoke".to_string(),
        ])
        .unwrap();

        match command {
            Command::SetApiKey(args) => {
                assert_eq!(args.provider_id, "openai-image-2");
                assert_eq!(args.service_type, "provider");
                assert!(!args.api_key.is_empty());
                assert_eq!(args.metadata["source"], "env");
                assert_eq!(args.metadata["env"], "PATH");
                assert_eq!(args.metadata["owner"], "local-smoke");
                assert_eq!(args.metadata["rotation_days"], 30);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_api_keys_rotation_audit_window() {
        let command = parse_command(vec![
            "api-keys".to_string(),
            "--rotation-days".to_string(),
            "45".to_string(),
        ])
        .unwrap();

        match command {
            Command::ApiKeys(args) => {
                assert_eq!(args.rotation_days, Some(45));
                assert_eq!(
                    api_keys_path(&args, &Some("demo".to_string())),
                    "/api/api-keys?rotation_days=45&project=demo"
                );
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn builds_set_api_key_body_with_project() {
        let body = set_api_key_body(
            &Some("demo".to_string()),
            SetApiKeyArgs {
                provider_id: "suno".to_string(),
                service_type: "provider".to_string(),
                api_key: "suno-test-secret".to_string(),
                metadata: json!({
                    "source": "env",
                    "env": "POOL_SUNO_API_KEY"
                }),
            },
        );

        assert_eq!(body["project_slug"], "demo");
        assert_eq!(body["provider_id"], "suno");
        assert_eq!(body["service_type"], "provider");
        assert_eq!(body["api_key"], "suno-test-secret");
        assert_eq!(body["metadata"]["env"], "POOL_SUNO_API_KEY");
    }

    #[test]
    fn rejects_conflicting_api_key_sources() {
        let error = parse_command(vec![
            "provider-health".to_string(),
            "openai-image-2".to_string(),
            "--api-key".to_string(),
            "sk-test".to_string(),
            "--api-key-env".to_string(),
            "OPENAI_API_KEY".to_string(),
        ])
        .unwrap_err()
        .to_string();

        assert!(error.contains("use either --api-key or --api-key-env"));
    }

    #[test]
    fn builds_software_action_body_with_payload_json() {
        let command = parse_command(vec![
            "run-software".to_string(),
            "touchdesigner".to_string(),
            "--action".to_string(),
            "run-viewport".to_string(),
            "--priority".to_string(),
            "desktop-recognition".to_string(),
            "--title".to_string(),
            "TouchDesigner cue".to_string(),
            "--payload-json".to_string(),
            r#"{"instruction":"trigger cue 1","target_window":"TouchDesigner"}"#.to_string(),
            "--payload".to_string(),
            "cue=Cue 1".to_string(),
            "--evidence-json".to_string(),
            r#"{"source":"cli-test","control_mode":"desktop"}"#.to_string(),
            "--production-software".to_string(),
        ])
        .unwrap();

        match command {
            Command::RunSoftware(args) => {
                let body = software_action_body(&Some("demo".to_string()), args);
                assert_eq!(body["project_slug"], "demo");
                assert_eq!(body["adapter_id"], "touchdesigner");
                assert_eq!(body["action_kind"], "RunViewport");
                assert_eq!(body["priority"], "DesktopRecognition");
                assert_eq!(body["task_title"], "TouchDesigner cue");
                assert_eq!(body["payload_json"]["target_window"], "TouchDesigner");
                assert_eq!(body["payload_json"]["cue"], "Cue 1");
                assert_eq!(body["evidence_json"]["source"], "cli-test");
                assert_eq!(body["evidence_json"]["production_software"], true);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn builds_software_health_body_without_creating_action() {
        let command = parse_command(vec![
            "software-health".to_string(),
            "blender".to_string(),
            "--priority".to_string(),
            "SkillsCli".to_string(),
        ])
        .unwrap();

        match command {
            Command::SoftwareHealth(args) => {
                let body = software_health_body(args);
                assert_eq!(body["adapter_id"], "blender");
                assert_eq!(body["priority"], "SkillsCli");
                assert_eq!(body["payload_json"], json!({}));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn builds_agent_session_body_for_cli_execution() {
        let command = parse_command(vec![
            "agent-session".to_string(),
            "agent-cli".to_string(),
            "--command-id".to_string(),
            "echo".to_string(),
            "--title".to_string(),
            "Echo smoke".to_string(),
            "--command".to_string(),
            "/bin/echo pool-ok".to_string(),
            "--tool".to_string(),
            "cli".to_string(),
            "--execute".to_string(),
            "--allowed-command".to_string(),
            "/bin/echo".to_string(),
            "--timeout-ms".to_string(),
            "2000".to_string(),
        ])
        .unwrap();

        match command {
            Command::AgentSession(args) => {
                let body = agent_session_body(&Some("demo".to_string()), args);
                assert_eq!(body["project_slug"], "demo");
                assert_eq!(body["kind"], "agent_cli");
                assert_eq!(body["command_id"], "echo");
                assert_eq!(body["command"], "/bin/echo pool-ok");
                assert_eq!(body["tools"][0], "cli");
                assert_eq!(body["execute"], true);
                assert_eq!(body["allowed_commands"][0], "/bin/echo");
                assert_eq!(body["timeout_ms"], 2000);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_agent_transcript_command() {
        let command = parse_command(vec![
            "agent-transcript".to_string(),
            "agent-session-1".to_string(),
        ])
        .unwrap();

        match command {
            Command::AgentTranscript { session_id } => {
                assert_eq!(session_id, "agent-session-1");
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_agent_stream_command_with_cursor_options() {
        let command = parse_command(vec![
            "agent-stream".to_string(),
            "agent-session-1".to_string(),
            "--last-event-id".to_string(),
            "event 1".to_string(),
            "--limit".to_string(),
            "12".to_string(),
        ])
        .unwrap();

        match command {
            Command::AgentStream(args) => {
                assert_eq!(args.session_id, "agent-session-1");
                assert_eq!(args.after_id.as_deref(), Some("event 1"));
                assert_eq!(args.limit, Some(12));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn builds_agent_stream_path_with_project_filter() {
        let path = agent_stream_path(
            &AgentStreamArgs {
                session_id: "agent session 1".to_string(),
                after_id: Some("event 1".to_string()),
                limit: Some(24),
            },
            &Some("demo".to_string()),
        );

        assert_eq!(
            path,
            "/api/agent-sessions/stream?session_id=agent%20session%201&last_event_id=event%201&limit=24&project=demo"
        );
    }

    #[test]
    fn agent_stream_dispatch_reads_session_sse_slice() {
        let db_path = temp_cli_db_path("agent-stream");
        let project_slug = Some("demo".to_string());
        let staged = dispatch(Cli {
            db_path: db_path.clone(),
            project_slug: project_slug.clone(),
            command: Command::AgentSession(AgentSessionArgs {
                kind: "hermes".to_string(),
                control_dir: None,
                endpoint: None,
                instruction: Some("inspect Unreal assembly".to_string()),
                allowed_tools: vec!["unreal".to_string()],
                requires_confirmation: Some(false),
                command_id: None,
                title: Some("Hermes stream smoke".to_string()),
                command: None,
                tools: Vec::new(),
                token_budget: Some(4_000),
                execute: false,
                allowed_commands: Vec::new(),
                working_dir: None,
                max_output_bytes: None,
                timeout_ms: None,
            }),
        })
        .unwrap();
        let staged_value: Value = serde_json::from_str(&staged.body).unwrap();
        let session_id = staged_value["report"]["session_id"].as_str().unwrap();

        let stream = dispatch(Cli {
            db_path,
            project_slug,
            command: Command::AgentStream(AgentStreamArgs {
                session_id: session_id.to_string(),
                after_id: None,
                limit: Some(10),
            }),
        })
        .unwrap();

        assert_eq!(stream.status_code, 200);
        assert_eq!(stream.content_type, "text/event-stream; charset=utf-8");
        assert!(stream.body.contains(": pool-agent-session"));
        assert!(stream.body.contains("event: agent-transcript"));
        assert!(stream.body.contains("event: runtime-event"));
        assert!(stream.body.contains(session_id));
    }

    #[test]
    fn builds_output_package_and_desktop_result_bodies() {
        let output = output_package_body(
            &Some("demo".to_string()),
            OutputPackageArgs {
                node_id: Some("outputs".to_string()),
                title: Some("CLI deliverables".to_string()),
                output_dir: Some("worlds/demo/output/deliverables".to_string()),
                source_assets: vec!["worlds/demo/output/1-world.glb".to_string()],
                duration_ms: Some(12_000),
            },
        );
        assert_eq!(output["project_slug"], "demo");
        assert_eq!(output["source_assets"][0], "worlds/demo/output/1-world.glb");
        assert_eq!(output["duration_ms"], 12_000);

        let output_result = output_result_body(
            &Some("demo".to_string()),
            OutputResultArgs {
                node_id: Some("outputs".to_string()),
                target: "video".to_string(),
                local_path: Some(
                    "worlds/demo/output/deliverables/1-video-timeline.json".to_string(),
                ),
                status: "succeeded".to_string(),
                runtime: Some("DaVinci Resolve".to_string()),
                adapter_id: Some("resolve".to_string()),
                software_action_id: Some("action-resolve".to_string()),
                message: Some("timeline rendered".to_string()),
                artifacts: vec!["worlds/demo/output/final.mp4".to_string()],
                metrics: vec![("frames".to_string(), "288".to_string())],
                verification: Some(json!({ "checksum": "abc" })),
            },
        );
        assert_eq!(output_result["project_slug"], "demo");
        assert_eq!(output_result["target"], "video");
        assert_eq!(output_result["status"], "succeeded");
        assert_eq!(output_result["runtime"], "DaVinci Resolve");
        assert_eq!(output_result["metrics"][0]["label"], "frames");
        assert_eq!(output_result["verification"]["checksum"], "abc");

        let handoff = handoff_package_body(
            &Some("demo".to_string()),
            HandoffPackageArgs {
                node_id: Some("agent".to_string()),
                title: Some("Runtime handoff package".to_string()),
                output_dir: Some("worlds/demo/output".to_string()),
                include_snapshot: true,
            },
        );
        assert_eq!(handoff["project_slug"], "demo");
        assert_eq!(handoff["node_id"], "agent");
        assert_eq!(handoff["title"], "Runtime handoff package");
        assert_eq!(handoff["output_dir"], "worlds/demo/output");
        assert_eq!(handoff["include_snapshot"], true);

        let production_evidence_handoff = production_evidence_handoff_package_body(
            &Some("demo".to_string()),
            ProductionEvidenceHandoffPackageArgs {
                node_id: Some("agent".to_string()),
                title: Some("Production evidence handoff package".to_string()),
                output_dir: Some("worlds/demo/output".to_string()),
                output_root: Some("worlds/demo/output/production-evidence".to_string()),
                source: Some("external-worker".to_string()),
                include_items: true,
                include_snapshot: true,
            },
        );
        assert_eq!(production_evidence_handoff["project_slug"], "demo");
        assert_eq!(production_evidence_handoff["node_id"], "agent");
        assert_eq!(
            production_evidence_handoff["title"],
            "Production evidence handoff package"
        );
        assert_eq!(
            production_evidence_handoff["output_root"],
            "worlds/demo/output/production-evidence"
        );
        assert_eq!(production_evidence_handoff["source"], "external-worker");
        assert_eq!(production_evidence_handoff["include_items"], true);
        assert_eq!(production_evidence_handoff["include_snapshot"], true);

        let result = desktop_result_body(DesktopResultArgs {
            software_action_id: "action-1".to_string(),
            task_id: Some("task-1".to_string()),
            status: "succeeded".to_string(),
            message: Some("controller finished".to_string()),
            artifacts: vec!["trace.json".to_string()],
            screen_trace_path: Some("trace.json".to_string()),
            result: Some(json!({ "controller": "desktop-vision" })),
            verification: None,
        });
        assert_eq!(result["software_action_id"], "action-1");
        assert_eq!(result["task_id"], "task-1");
        assert_eq!(result["status"], "succeeded");
        assert_eq!(result["artifacts"][0], "trace.json");
        assert_eq!(result["result"]["controller"], "desktop-vision");
    }

    #[test]
    fn parses_desktop_run_next_command() {
        let command = parse_command(vec![
            "desktop-run-next".to_string(),
            "--status".to_string(),
            "retryable".to_string(),
            "--controller-id".to_string(),
            "vision-controller".to_string(),
            "--limit".to_string(),
            "2".to_string(),
            "--artifact".to_string(),
            "trace.json".to_string(),
        ])
        .unwrap();

        match command {
            Command::DesktopRunNext(args) => {
                assert_eq!(args.status, "retryable");
                assert_eq!(args.controller_id, "vision-controller");
                assert_eq!(args.limit, 2);
                assert_eq!(args.artifacts, vec!["trace.json"]);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_desktop_contract_command() {
        let command = parse_command(vec!["desktop-contract".to_string()]).unwrap();
        assert!(matches!(command, Command::DesktopContract));
    }

    #[test]
    fn builds_desktop_run_next_body() {
        let body = desktop_run_next_body(DesktopRunNextArgs {
            status: "succeeded".to_string(),
            message: None,
            controller_id: "pool-cli-desktop-controller".to_string(),
            limit: 1,
            artifacts: vec!["screen.png".to_string()],
            screen_trace_path: Some("trace.json".to_string()),
        });

        assert_eq!(body["status"], "succeeded");
        assert_eq!(body["controller_id"], "pool-cli-desktop-controller");
        assert_eq!(body["limit"], 1);
        assert_eq!(body["screen_trace_path"], "trace.json");
        assert_eq!(body["artifacts"][0], "screen.png");
    }

    #[test]
    fn desktop_contract_dispatch_reads_runtime_contract() {
        let response = dispatch(Cli {
            db_path: temp_cli_db_path("desktop-contract"),
            project_slug: Some("demo".to_string()),
            command: Command::DesktopContract,
        })
        .unwrap();
        let value: Value = serde_json::from_str(&response.body).unwrap();

        assert_eq!(response.status_code, 200);
        assert_eq!(value["kind"], "pool_desktop_recognition_contract");
        assert_eq!(
            value["queue"]["read_requests"]["http"],
            "GET /api/desktop-recognition/requests"
        );
        assert_eq!(value["result_callback"]["statuses"][2], "succeeded");
    }

    #[test]
    fn desktop_run_next_dispatches_queued_request() {
        let db_path = temp_cli_db_path("desktop-run-next");
        let project_slug = Some("demo".to_string());

        let stage = dispatch(Cli {
            db_path: db_path.clone(),
            project_slug: project_slug.clone(),
            command: Command::RunSoftware(SoftwareActionArgs {
                adapter_id: "touchdesigner".to_string(),
                node_id: Some("node-touchdesigner".to_string()),
                task_title: Some("TouchDesigner desktop cue".to_string()),
                action_kind: Some("RunViewport".to_string()),
                priority: Some("DesktopRecognition".to_string()),
                payload_json: json!({
                    "instruction": "find TouchDesigner perform mode and trigger cue 1",
                    "target_window": "TouchDesigner",
                    "visual_targets": ["Perform", "Cue 1"]
                }),
                evidence_json: None,
                requires_confirmation: Some(false),
            }),
        })
        .unwrap();
        assert_eq!(stage.status_code, 200);

        let run = dispatch(Cli {
            db_path: db_path.clone(),
            project_slug: project_slug.clone(),
            command: Command::DesktopRunNext(DesktopRunNextArgs {
                status: "succeeded".to_string(),
                message: Some("desktop controller dry-run finished".to_string()),
                controller_id: "pool-cli-desktop-controller-test".to_string(),
                limit: 1,
                artifacts: vec!["screen-trace.json".to_string()],
                screen_trace_path: Some("screen-trace.json".to_string()),
            }),
        })
        .unwrap();
        let value: Value = serde_json::from_str(&run.body).unwrap();
        assert_eq!(run.status_code, 200);
        assert_eq!(value["processed_count"], 1);
        assert_eq!(
            value["callbacks"][0]["response"]["task"]["status"],
            "Succeeded"
        );
        assert_eq!(
            value["callbacks"][0]["response"]["software_action"]["verification"]
                ["controller_result"]["mode"],
            "dry_run"
        );

        let queue = dispatch(Cli {
            db_path,
            project_slug,
            command: Command::DesktopRequests,
        })
        .unwrap();
        let queue_value: Value = serde_json::from_str(&queue.body).unwrap();
        assert_eq!(queue_value["count"], 0);
    }

    #[test]
    fn parses_workflow_run_command_with_adapter_modes() {
        let command = parse_command(vec![
            "run-workflow".to_string(),
            "--title".to_string(),
            "CLI burst".to_string(),
            "--prompt".to_string(),
            "make scene".to_string(),
            "--source-input".to_string(),
            "worlds/demo/source/0-reference.png".to_string(),
            "--agent-mode".to_string(),
            "stage".to_string(),
            "--three-dgs-mode".to_string(),
            "mock".to_string(),
            "--unreal-mode".to_string(),
            "mock".to_string(),
        ])
        .unwrap();

        match command {
            Command::RunWorkflow(run) => {
                assert_eq!(run.title.as_deref(), Some("CLI burst"));
                assert_eq!(run.prompt.as_deref(), Some("make scene"));
                assert_eq!(run.source_inputs.len(), 1);
                assert_eq!(run.agent_mode.as_deref(), Some("stage"));
                assert_eq!(run.three_dgs_mode.as_deref(), Some("mock"));
                assert_eq!(run.unreal_mode.as_deref(), Some("mock"));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_task_action_alias() {
        let command = parse_command(vec!["approve".to_string(), "task-1".to_string()]).unwrap();

        match command {
            Command::TaskAction(action) => {
                assert!(matches!(action.kind, TaskActionKind::Approve));
                assert_eq!(action.task_id, "task-1");
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_prd_completion_gate_require_complete() {
        let command = parse_command(vec![
            "prd-completion-gate".to_string(),
            "--require-complete".to_string(),
        ])
        .unwrap();

        match command {
            Command::PrdCompletionGate(args) => {
                assert!(args.require_complete);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_prd_completion_package_command() {
        let command = parse_command(vec![
            "prd-completion-package".to_string(),
            "--node-id".to_string(),
            "agent".to_string(),
            "--title".to_string(),
            "Final proof".to_string(),
            "--output-dir".to_string(),
            "worlds/demo/output".to_string(),
            "--source=operator".to_string(),
            "--no-snapshot".to_string(),
        ])
        .unwrap();

        match command {
            Command::PrdCompletionPackage(args) => {
                assert_eq!(args.node_id.as_deref(), Some("agent"));
                assert_eq!(args.title.as_deref(), Some("Final proof"));
                assert_eq!(args.output_dir.as_deref(), Some("worlds/demo/output"));
                assert_eq!(args.source.as_deref(), Some("operator"));
                assert!(!args.include_snapshot);
                assert_eq!(
                    prd_completion_package_body(&Some("demo".to_string()), args)["project_slug"],
                    "demo"
                );
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_prd_completion_packages_command() {
        let command = parse_command(vec!["prd-completion-packages".to_string()]).unwrap();

        assert!(matches!(command, Command::PrdCompletionPackages));
    }

    #[test]
    fn builds_workflow_run_body_with_modes() {
        let body = workflow_run_body(
            &Some("demo".to_string()),
            WorkflowRunArgs {
                title: Some("CLI burst".to_string()),
                prompt: Some("make a playable scene".to_string()),
                source_inputs: vec!["worlds/demo/source/0-reference.png".to_string()],
                output_root: Some("target/cli-burst".to_string()),
                duration_ms: Some(18_000),
                agent_mode: Some("stage".to_string()),
                hermes_endpoint: None,
                hermes_auth_token: None,
                agent_requires_confirmation: true,
                three_dgs_mode: Some("mock".to_string()),
                three_dgs_provider_id: Some("worldlabs-marble".to_string()),
                three_dgs_endpoint: None,
                three_dgs_api_key: None,
                unreal_mode: Some("mock".to_string()),
                unreal_endpoint: None,
                unreal_auth_token: None,
            },
        );

        assert_eq!(body["project_slug"], "demo");
        assert_eq!(body["title"], "CLI burst");
        assert_eq!(
            body["source_inputs"][0],
            "worlds/demo/source/0-reference.png"
        );
        assert_eq!(body["duration_ms"], 18_000);
        assert_eq!(body["agent_requires_confirmation"], true);
        assert_eq!(body["three_dgs_mode"], "mock");
        assert_eq!(body["unreal_mode"], "mock");
    }

    #[test]
    fn builds_events_path_with_project_filter() {
        let path = events_path(
            &EventsArgs {
                after_id: Some("event 1".to_string()),
                limit: Some(24),
            },
            &Some("demo".to_string()),
        );

        assert_eq!(path, "/api/events?after_id=event%201&limit=24&project=demo");
    }

    #[test]
    fn maps_task_actions_to_runtime_paths() {
        assert_eq!(
            task_action_path(&TaskActionKind::Approve),
            "/api/tasks/approve"
        );
        assert_eq!(
            task_action_path(&TaskActionKind::Cancel),
            "/api/tasks/cancel"
        );
        assert_eq!(task_action_path(&TaskActionKind::Retry), "/api/tasks/retry");
    }

    #[test]
    fn percent_encodes_mcp_uri() {
        assert_eq!(
            path_with_query("/api/mcp", &[("uri", "pool://node-context/node 1")], &None),
            "/api/mcp?uri=pool%3A%2F%2Fnode-context%2Fnode%201"
        );
    }

    #[test]
    fn parses_serve_mcp_command() {
        let command = parse_command(vec!["serve-mcp".to_string()]).unwrap();
        assert!(matches!(command, Command::ServeMcp));
    }

    #[test]
    fn parses_provider_gateway_worker_contract_command() {
        let command = parse_command(vec!["gateway-worker-contract".to_string()]).unwrap();
        assert!(matches!(command, Command::ProviderGatewayWorkerContract));
    }

    #[test]
    fn parses_unreal_mcp_bridge_contract_command() {
        let command = parse_command(vec!["unreal-bridge-contract".to_string()]).unwrap();
        assert!(matches!(command, Command::UnrealMcpBridgeContract));
    }

    #[test]
    fn parses_output_packages_query_command() {
        let command = parse_command(vec!["output-packages".to_string()]).unwrap();
        assert!(matches!(command, Command::OutputPackages));
    }

    #[test]
    fn parses_prd_readiness_query_command() {
        let command = parse_command(vec!["prd-readiness".to_string()]).unwrap();
        assert!(matches!(command, Command::PrdReadiness));
    }

    #[test]
    fn parses_core_architecture_readiness_query_command() {
        let command = parse_command(vec!["core-architecture-readiness".to_string()]).unwrap();
        assert!(matches!(command, Command::CoreArchitectureReadiness));
    }

    #[test]
    fn parses_core_architecture_gate_require_ready() {
        let command = parse_command(vec![
            "core-architecture-gate".to_string(),
            "--require-ready".to_string(),
        ])
        .unwrap();
        match command {
            Command::CoreArchitectureGate(args) => assert!(args.require_ready),
            _ => panic!("expected core architecture gate command"),
        }
    }

    #[test]
    fn parses_core_architecture_packages_query_command() {
        let command = parse_command(vec!["core-architecture-packages".to_string()]).unwrap();
        assert!(matches!(command, Command::CoreArchitecturePackages));
    }

    #[test]
    fn parses_core_architecture_package_command() {
        let command = parse_command(vec![
            "core-architecture-package".to_string(),
            "--node-id".to_string(),
            "agent".to_string(),
            "--title".to_string(),
            "Core proof".to_string(),
            "--output-dir".to_string(),
            "worlds/demo/output".to_string(),
            "--source=cli-test".to_string(),
            "--no-snapshot".to_string(),
        ])
        .unwrap();

        match command {
            Command::CoreArchitecturePackage(args) => {
                assert_eq!(args.node_id.as_deref(), Some("agent"));
                assert_eq!(args.title.as_deref(), Some("Core proof"));
                assert_eq!(args.output_dir.as_deref(), Some("worlds/demo/output"));
                assert_eq!(args.source.as_deref(), Some("cli-test"));
                assert!(!args.include_snapshot);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_output_result_command() {
        let command = parse_command(vec![
            "output-result".to_string(),
            "interactive_art".to_string(),
            "--status".to_string(),
            "succeeded".to_string(),
            "--runtime".to_string(),
            "TouchDesigner".to_string(),
            "--adapter-id".to_string(),
            "touchdesigner".to_string(),
            "--artifact".to_string(),
            "worlds/demo/output/cue-1.toe".to_string(),
            "--metric".to_string(),
            "cues=1".to_string(),
        ])
        .unwrap();
        match command {
            Command::OutputResult(args) => {
                assert_eq!(args.target, "interactive_art");
                assert_eq!(args.status, "succeeded");
                assert_eq!(args.runtime.as_deref(), Some("TouchDesigner"));
                assert_eq!(args.artifacts[0], "worlds/demo/output/cue-1.toe");
                assert_eq!(args.metrics[0], ("cues".to_string(), "1".to_string()));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn builds_mcp_initialize_response() {
        let result = mcp_initialize_result();
        assert_eq!(result["protocolVersion"], MCP_PROTOCOL_VERSION);
        assert_eq!(result["serverInfo"]["name"], "pool-runtime");
        assert_eq!(result["capabilities"]["resources"], json!({}));
        assert_eq!(result["capabilities"]["tools"], json!({}));
    }

    #[test]
    fn exposes_pool_mcp_tools_without_secret_write_tool() {
        let names = mcp_tool_definitions()
            .into_iter()
            .filter_map(|tool| tool.get("name").and_then(Value::as_str).map(str::to_string))
            .collect::<Vec<_>>();

        assert!(names.contains(&"pool_run_workflow".to_string()));
        assert!(names.contains(&"pool_runtime_budget".to_string()));
        assert!(names.contains(&"pool_runtime_preflight".to_string()));
        assert!(names.contains(&"pool_runtime_execution_plan".to_string()));
        assert!(names.contains(&"pool_runtime_execution_plan_run_next".to_string()));
        assert!(names.contains(&"pool_runtime_handoff".to_string()));
        assert!(names.contains(&"pool_runtime_handoff_packages".to_string()));
        assert!(names.contains(&"pool_core_architecture_readiness".to_string()));
        assert!(names.contains(&"pool_core_architecture_gate".to_string()));
        assert!(names.contains(&"pool_core_architecture_packages".to_string()));
        assert!(names.contains(&"pool_core_architecture_package".to_string()));
        assert!(names.contains(&"pool_prd_readiness".to_string()));
        assert!(names.contains(&"pool_prd_completion_gate".to_string()));
        assert!(names.contains(&"pool_prd_completion_packages".to_string()));
        assert!(names.contains(&"pool_prd_completion_package".to_string()));
        assert!(names.contains(&"pool_output_packages".to_string()));
        assert!(names.contains(&"pool_output_result".to_string()));
        assert!(names.contains(&"pool_adapters".to_string()));
        assert!(names.contains(&"pool_integration_readiness".to_string()));
        assert!(names.contains(&"pool_software_contracts".to_string()));
        assert!(names.contains(&"pool_software_conformance_packages".to_string()));
        assert!(names.contains(&"pool_software_conformance_package".to_string()));
        assert!(names.contains(&"pool_provider_conformance_packages".to_string()));
        assert!(names.contains(&"pool_provider_conformance_package".to_string()));
        assert!(names.contains(&"pool_agent_conformance_packages".to_string()));
        assert!(names.contains(&"pool_agent_conformance_package".to_string()));
        assert!(names.contains(&"pool_integration_conformance_packages".to_string()));
        assert!(names.contains(&"pool_integration_conformance_package".to_string()));
        assert!(names.contains(&"pool_handoff_package".to_string()));
        assert!(names.contains(&"pool_workflow_context".to_string()));
        assert!(names.contains(&"pool_run_provider".to_string()));
        assert!(names.contains(&"pool_provider_request_metadata".to_string()));
        assert!(names.contains(&"pool_provider_gateway_worker".to_string()));
        assert!(names.contains(&"pool_worker_self_checks".to_string()));
        assert!(names.contains(&"pool_unreal_mcp_bridge".to_string()));
        assert!(names.contains(&"pool_production_evidence_tasks".to_string()));
        assert!(names.contains(&"pool_production_evidence_task_claim".to_string()));
        assert!(names.contains(&"pool_production_evidence_item_template".to_string()));
        assert!(names.contains(&"pool_production_evidence_handoff".to_string()));
        assert!(names.contains(&"pool_prd_completion_package".to_string()));
        assert!(names.contains(&"pool_production_evidence_handoff_packages".to_string()));
        assert!(names.contains(&"pool_production_evidence_handoff_package".to_string()));
        assert!(names.contains(&"pool_production_evidence_template".to_string()));
        assert!(names.contains(&"pool_validate_production_evidence".to_string()));
        assert!(names.contains(&"pool_merge_production_evidence".to_string()));
        assert!(names.contains(&"pool_closeout_production_evidence".to_string()));
        assert!(names.contains(&"pool_import_production_evidence".to_string()));
        assert!(names.contains(&"pool_validate_production_evidence_item".to_string()));
        assert!(names.contains(&"pool_submit_production_evidence_item".to_string()));
        assert!(names.contains(&"pool_run_software".to_string()));
        assert!(names.contains(&"pool_agent_session".to_string()));
        assert!(names.contains(&"pool_agent_transcript".to_string()));
        assert!(names.contains(&"pool_agent_stream".to_string()));
        assert!(names.contains(&"pool_desktop_run_next".to_string()));
        assert!(names.contains(&"pool_desktop_result".to_string()));
        assert!(!names.contains(&"pool_set_api_key".to_string()));
    }

    #[test]
    fn parses_mcp_worker_self_checks_args() {
        let args = mcp_worker_self_checks_args(json!({
            "output_root": "target/mcp-worker-checks",
            "adapter_id": "touchdesigner"
        }))
        .unwrap();

        assert_eq!(args.output_root, PathBuf::from("target/mcp-worker-checks"));
        assert_eq!(args.software_adapter_id, "touchdesigner");
    }

    #[test]
    fn production_evidence_mcp_schema_requires_external_visual_model() {
        for tool_name in [
            "pool_validate_production_evidence",
            "pool_import_production_evidence",
        ] {
            let tools = mcp_tool_definitions();
            let tool = tools
                .iter()
                .find(|tool| tool["name"] == tool_name)
                .unwrap_or_else(|| panic!("missing MCP tool {tool_name}"));
            let desktop = &tool["inputSchema"]["properties"]["desktop_vision"]["items"];
            let required = desktop["required"].as_array().unwrap();

            assert!(required.iter().any(|field| field == "visual_model"));
            assert_eq!(desktop["properties"]["visual_model"]["const"], "external");
            assert!(desktop["properties"]["visual_model"]["description"]
                .as_str()
                .unwrap_or_default()
                .contains("external visual"));
        }
    }

    #[test]
    fn production_evidence_item_mcp_schema_requires_kind() {
        let tools = mcp_tool_definitions();
        for tool_name in [
            "pool_validate_production_evidence_item",
            "pool_submit_production_evidence_item",
        ] {
            let tool = tools
                .iter()
                .find(|tool| tool["name"] == tool_name)
                .unwrap_or_else(|| panic!("missing MCP tool {tool_name}"));
            let required = tool["inputSchema"]["required"].as_array().unwrap();

            assert!(required.iter().any(|field| field == "kind"));
            assert_eq!(tool["inputSchema"]["properties"]["kind"]["type"], "string");
            assert_eq!(
                tool["inputSchema"]["properties"]["desktop_vision"]["properties"]["visual_model"]
                    ["const"],
                "external"
            );
        }
    }

    #[test]
    fn production_evidence_merge_mcp_schema_requires_bundles() {
        let tools = mcp_tool_definitions();
        let tool = tools
            .iter()
            .find(|tool| tool["name"] == "pool_merge_production_evidence")
            .unwrap();
        let required = tool["inputSchema"]["required"].as_array().unwrap();

        assert!(required.iter().any(|field| field == "bundles"));
        assert_eq!(
            tool["inputSchema"]["properties"]["bundles"]["type"],
            "array"
        );
        assert_eq!(
            tool["inputSchema"]["properties"]["bundles"]["items"]["properties"]["desktop_vision"]
                ["items"]["properties"]["visual_model"]["const"],
            "external"
        );
    }

    #[test]
    fn production_evidence_closeout_mcp_schema_requires_bundles_and_declares_import_flag() {
        let tools = mcp_tool_definitions();
        let tool = tools
            .iter()
            .find(|tool| tool["name"] == "pool_closeout_production_evidence")
            .unwrap();
        let required = tool["inputSchema"]["required"].as_array().unwrap();

        assert!(required.iter().any(|field| field == "bundles"));
        assert_eq!(
            tool["inputSchema"]["properties"]["bundles"]["type"],
            "array"
        );
        assert_eq!(
            tool["inputSchema"]["properties"]["import"]["type"],
            "boolean"
        );
        assert_eq!(
            tool["inputSchema"]["properties"]["completion_package"]["properties"]["output_dir"]
                ["type"],
            "string"
        );
        assert_eq!(
            tool["inputSchema"]["properties"]["bundles"]["items"]["properties"]["desktop_vision"]
                ["items"]["properties"]["visual_model"]["const"],
            "external"
        );
    }

    #[test]
    fn provider_mcp_tool_declares_evidence_json() {
        let tools = mcp_tool_definitions();
        let provider_tool = tools
            .iter()
            .find(|tool| tool["name"] == "pool_run_provider")
            .unwrap();

        assert_eq!(
            provider_tool["inputSchema"]["properties"]["evidence_json"]["type"],
            "object"
        );
    }

    #[test]
    fn exposes_pool_mcp_prompts() {
        let prompts = pool_mcp_prompt_definitions();
        let names = prompts
            .iter()
            .filter_map(|prompt| prompt.get("name").and_then(Value::as_str))
            .collect::<Vec<_>>();

        assert!(names.contains(&"pool_content_burst_runbook"));
        assert!(names.contains(&"pool_3dgs_conversion_review"));
        assert!(names.contains(&"pool_software_handoff"));
        assert!(names.contains(&"pool_desktop_takeover"));
    }

    #[test]
    fn builds_pool_mcp_prompt_get_result() {
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
        assert_eq!(result["messages"][0]["role"], "user");
        let text = result["messages"][0]["content"]["text"]
            .as_str()
            .unwrap_or_default();
        assert!(text.contains("Adapter: blender"));
        assert!(text.contains("pool_software_health"));
        assert!(text.contains("pool_run_software"));
    }

    #[test]
    fn maps_mcp_resource_tool_to_runtime_path() {
        let request = mcp_tool_http_request(
            "pool_read_resource",
            json!({ "uri": "pool://node-context/node 1" }),
            &Some("demo".to_string()),
        )
        .unwrap();

        assert_eq!(request.method, "GET");
        assert_eq!(
            request.path,
            "/api/mcp?uri=pool%3A%2F%2Fnode-context%2Fnode%201&project=demo"
        );
        assert!(request.body.is_none());
    }

    #[test]
    fn maps_provider_gateway_worker_tool_to_runtime_path() {
        let request =
            mcp_tool_http_request("pool_provider_gateway_worker", json!({}), &None).unwrap();

        assert_eq!(request.method, "GET");
        assert_eq!(request.path, "/api/provider-gateway-worker");
        assert!(request.body.is_none());
    }

    #[test]
    fn maps_unreal_mcp_bridge_tool_to_runtime_path() {
        let request = mcp_tool_http_request("pool_unreal_mcp_bridge", json!({}), &None).unwrap();

        assert_eq!(request.method, "GET");
        assert_eq!(request.path, "/api/unreal-mcp-bridge");
        assert!(request.body.is_none());
    }

    #[test]
    fn maps_mcp_run_provider_tool_with_evidence_json() {
        let request = mcp_tool_http_request(
            "pool_run_provider",
            json!({
                "provider_id": "worldlabs-marble",
                "execution_mode": "gateway",
                "evidence_json": {
                    "source": "mcp-agent",
                    "production_upstream": true
                }
            }),
            &Some("demo".to_string()),
        )
        .unwrap();
        let body = request.body.unwrap();

        assert_eq!(request.method, "POST");
        assert_eq!(request.path, "/api/provider-runs");
        assert_eq!(body["project_slug"], "demo");
        assert_eq!(body["provider_id"], "worldlabs-marble");
        assert_eq!(body["evidence_json"]["source"], "mcp-agent");
        assert_eq!(body["evidence_json"]["production_upstream"], true);
    }

    #[test]
    fn maps_mcp_production_evidence_template_tool_to_runtime_path() {
        let request = mcp_tool_http_request(
            "pool_production_evidence_template",
            json!({
                "project_slug": "demo",
                "output_root": "target/prod evidence",
                "source": "external worker",
                "missing_only": true
            }),
            &None,
        )
        .unwrap();

        assert_eq!(request.method, "GET");
        assert_eq!(
            request.path,
            "/api/production-evidence/template?output_root=target%2Fprod%20evidence&source=external%20worker&missing_only=true&project=demo"
        );
        assert!(request.body.is_none());
    }

    #[test]
    fn maps_mcp_production_evidence_handoff_tool_to_runtime_path() {
        let request = mcp_tool_http_request(
            "pool_production_evidence_handoff",
            json!({
                "project_slug": "demo",
                "output_root": "target/prod evidence",
                "source": "external worker"
            }),
            &None,
        )
        .unwrap();

        assert_eq!(request.method, "GET");
        assert_eq!(
            request.path,
            "/api/production-evidence/handoff?output_root=target%2Fprod%20evidence&source=external%20worker&project=demo"
        );
        assert!(request.body.is_none());
    }

    #[test]
    fn maps_mcp_production_evidence_requirements_tool_to_runtime_path() {
        let request = mcp_tool_http_request(
            "pool_production_evidence_requirements",
            json!({ "project_slug": "demo" }),
            &None,
        )
        .unwrap();

        assert_eq!(request.method, "GET");
        assert_eq!(
            request.path,
            "/api/production-evidence/requirements?project=demo"
        );
        assert!(request.body.is_none());
    }

    #[test]
    fn maps_mcp_prd_completion_gate_tool_to_runtime_path() {
        let request = mcp_tool_http_request(
            "pool_prd_completion_gate",
            json!({
                "project_slug": "demo",
                "require_complete": true
            }),
            &None,
        )
        .unwrap();

        assert_eq!(request.method, "GET");
        assert_eq!(
            request.path,
            "/api/prd-completion-gate?require_complete=true&project=demo"
        );
        assert!(request.body.is_none());
    }

    #[test]
    fn maps_mcp_prd_completion_packages_tool_to_runtime_path() {
        let request = mcp_tool_http_request(
            "pool_prd_completion_packages",
            json!({ "project_slug": "demo" }),
            &None,
        )
        .unwrap();

        assert_eq!(request.method, "GET");
        assert_eq!(request.path, "/api/prd-completion-packages?project=demo");
        assert!(request.body.is_none());
    }

    #[test]
    fn maps_mcp_production_evidence_handoff_packages_tool_to_runtime_path() {
        let request = mcp_tool_http_request(
            "pool_production_evidence_handoff_packages",
            json!({ "project_slug": "demo" }),
            &None,
        )
        .unwrap();

        assert_eq!(request.method, "GET");
        assert_eq!(
            request.path,
            "/api/production-evidence/handoff-packages?project=demo"
        );
        assert!(request.body.is_none());
    }

    #[test]
    fn maps_mcp_production_evidence_tasks_tool_to_runtime_path() {
        let request = mcp_tool_http_request(
            "pool_production_evidence_tasks",
            json!({ "project_slug": "demo" }),
            &None,
        )
        .unwrap();

        assert_eq!(request.method, "GET");
        assert_eq!(request.path, "/api/production-evidence/tasks?project=demo");
        assert!(request.body.is_none());
    }

    #[test]
    fn maps_mcp_production_evidence_task_claim_tool_to_runtime_path() {
        let request = mcp_tool_http_request(
            "pool_production_evidence_task_claim",
            json!({
                "project_slug": "demo",
                "task_id": "provider:midjourney:production_upstream",
                "assignee": "worker-1",
                "role": "provider_worker",
                "output_root": "target/evidence",
                "source": "agent-claim"
            }),
            &None,
        )
        .unwrap();
        let body = request.body.unwrap();

        assert_eq!(request.method, "POST");
        assert_eq!(request.path, "/api/production-evidence/tasks/claim");
        assert_eq!(body["project_slug"], "demo");
        assert_eq!(body["task_id"], "provider:midjourney:production_upstream");
        assert_eq!(body["assignee"], "worker-1");
        assert_eq!(body["role"], "provider_worker");
    }

    #[test]
    fn maps_mcp_production_evidence_run_plan_tool_to_runtime_path() {
        let request = mcp_tool_http_request(
            "pool_production_evidence_run_plan",
            json!({
                "project_slug": "demo",
                "output_root": "target/prod evidence",
                "source": "agent run plan"
            }),
            &None,
        )
        .unwrap();

        assert_eq!(request.method, "GET");
        assert_eq!(
            request.path,
            "/api/production-evidence/run-plan?output_root=target%2Fprod%20evidence&source=agent%20run%20plan&project=demo"
        );
        assert!(request.body.is_none());
    }

    #[test]
    fn maps_mcp_production_evidence_handoff_package_tool_to_runtime_path() {
        let request = mcp_tool_http_request(
            "pool_production_evidence_handoff_package",
            json!({
                "project_slug": "demo",
                "node_id": "agent",
                "output_dir": "worlds/demo/output",
                "output_root": "worlds/demo/output/production-evidence",
                "include_snapshot": true
            }),
            &None,
        )
        .unwrap();

        assert_eq!(request.method, "POST");
        assert_eq!(request.path, "/api/production-evidence/handoff-packages");
        let body = request.body.unwrap();
        assert_eq!(body["project_slug"], "demo");
        assert_eq!(body["node_id"], "agent");
        assert_eq!(body["output_dir"], "worlds/demo/output");
        assert_eq!(
            body["output_root"],
            "worlds/demo/output/production-evidence"
        );
        assert_eq!(body["include_snapshot"], true);
    }

    #[test]
    fn maps_mcp_production_evidence_item_template_tool_to_runtime_path() {
        let request = mcp_tool_http_request(
            "pool_production_evidence_item_template",
            json!({
                "project_slug": "demo",
                "task_id": "provider:midjourney:production_upstream",
                "output_root": "target/prod evidence",
                "source": "external worker"
            }),
            &None,
        )
        .unwrap();

        assert_eq!(request.method, "GET");
        assert_eq!(
            request.path,
            "/api/production-evidence/item-template?output_root=target%2Fprod%20evidence&source=external%20worker&task_id=provider%3Amidjourney%3Aproduction_upstream&project=demo"
        );
        assert!(request.body.is_none());
    }

    #[test]
    fn maps_mcp_production_evidence_item_from_ledger_tool_to_runtime_path() {
        let request = mcp_tool_http_request(
            "pool_production_evidence_item_from_ledger",
            json!({
                "project_slug": "demo",
                "provider_request_id": "provider request 1",
                "source": "runtime ledger"
            }),
            &None,
        )
        .unwrap();

        assert_eq!(request.method, "GET");
        assert_eq!(
            request.path,
            "/api/production-evidence/item-from-ledger?source=runtime%20ledger&provider_request_id=provider%20request%201&project=demo"
        );
        assert!(request.body.is_none());
    }

    #[test]
    fn maps_mcp_production_evidence_desktop_item_from_ledger_tool_to_runtime_path() {
        let request = mcp_tool_http_request(
            "pool_production_evidence_item_from_ledger",
            json!({
                "project_slug": "demo",
                "desktop_vision_action_id": "desktop action 1",
                "source": "runtime ledger"
            }),
            &None,
        )
        .unwrap();

        assert_eq!(request.method, "GET");
        assert_eq!(
            request.path,
            "/api/production-evidence/item-from-ledger?source=runtime%20ledger&desktop_vision_action_id=desktop%20action%201&project=demo"
        );
        assert!(request.body.is_none());
    }

    #[test]
    fn maps_mcp_production_evidence_bundle_from_ledger_tool_to_runtime_path() {
        let request = mcp_tool_http_request(
            "pool_production_evidence_bundle_from_ledger",
            json!({
                "project_slug": "demo",
                "source": "runtime ledger",
                "include_incomplete": true
            }),
            &None,
        )
        .unwrap();

        assert_eq!(request.method, "GET");
        assert_eq!(
            request.path,
            "/api/production-evidence/bundle-from-ledger?source=runtime%20ledger&include_incomplete=true&project=demo"
        );
        assert!(request.body.is_none());
    }

    #[test]
    fn maps_mcp_validate_production_evidence_tool_to_runtime_path() {
        let request = mcp_tool_http_request(
            "pool_validate_production_evidence",
            json!({
                "providers": [{
                    "provider_id": "worldlabs-marble",
                    "external_job_id": "marble-real-1",
                    "production_attestation": "worldlabs-marble-worker-mcp-validate-run-1",
                    "artifacts": ["worlds/demo/output/production/marble.glb"]
                }]
            }),
            &Some("demo".to_string()),
        )
        .unwrap();
        let body = request.body.unwrap();

        assert_eq!(request.method, "POST");
        assert_eq!(request.path, "/api/production-evidence/validate");
        assert_eq!(body["project_slug"], "demo");
        assert_eq!(body["providers"][0]["provider_id"], "worldlabs-marble");
    }

    #[test]
    fn maps_mcp_merge_production_evidence_tool_to_runtime_path() {
        let request = mcp_tool_http_request(
            "pool_merge_production_evidence",
            json!({
                "source": "agent-closeout",
                "bundles": [{
                    "project_slug": "demo",
                    "providers": [{
                        "provider_id": "worldlabs-marble",
                        "external_job_id": "marble-real-1",
                        "production_attestation": "worldlabs-marble-worker-mcp-merge-run-1",
                        "artifacts": ["worlds/demo/output/production/marble.glb"]
                    }]
                }]
            }),
            &Some("demo".to_string()),
        )
        .unwrap();
        let body = request.body.unwrap();

        assert_eq!(request.method, "POST");
        assert_eq!(request.path, "/api/production-evidence/merge");
        assert_eq!(body["project_slug"], "demo");
        assert_eq!(body["source"], "agent-closeout");
        assert_eq!(
            body["bundles"][0]["providers"][0]["provider_id"],
            "worldlabs-marble"
        );
    }

    #[test]
    fn maps_mcp_closeout_production_evidence_tool_to_runtime_path() {
        let request = mcp_tool_http_request(
            "pool_closeout_production_evidence",
            json!({
                "source": "agent-closeout",
                "import": true,
                "bundles": [{
                    "project_slug": "demo",
                    "providers": [{
                        "provider_id": "worldlabs-marble",
                        "external_job_id": "marble-real-1",
                        "production_attestation": "worldlabs-marble-worker-mcp-closeout-run-1",
                        "artifacts": ["worlds/demo/output/production/marble.glb"]
                    }]
                }]
            }),
            &Some("demo".to_string()),
        )
        .unwrap();
        let body = request.body.unwrap();

        assert_eq!(request.method, "POST");
        assert_eq!(request.path, "/api/production-evidence/closeout");
        assert_eq!(body["project_slug"], "demo");
        assert_eq!(body["source"], "agent-closeout");
        assert_eq!(body["import"], true);
        assert_eq!(
            body["bundles"][0]["providers"][0]["provider_id"],
            "worldlabs-marble"
        );
    }

    #[test]
    fn maps_mcp_validate_production_evidence_item_tool_to_runtime_path() {
        let request = mcp_tool_http_request(
            "pool_validate_production_evidence_item",
            json!({
                "kind": "provider",
                "provider": {
                    "provider_id": "midjourney",
                    "external_job_id": "mj-real-1",
                    "production_attestation": "midjourney-worker-mcp-item-validate-run-1",
                    "artifacts": ["worlds/demo/output/production/mj.png"]
                }
            }),
            &Some("demo".to_string()),
        )
        .unwrap();
        let body = request.body.unwrap();

        assert_eq!(request.method, "POST");
        assert_eq!(request.path, "/api/production-evidence/items/validate");
        assert_eq!(body["project_slug"], "demo");
        assert_eq!(body["kind"], "provider");
        assert_eq!(body["provider"]["provider_id"], "midjourney");
    }

    #[test]
    fn maps_mcp_submit_production_evidence_item_tool_to_runtime_path() {
        let request = mcp_tool_http_request(
            "pool_submit_production_evidence_item",
            json!({
                "kind": "provider",
                "provider": {
                    "provider_id": "midjourney",
                    "external_job_id": "mj-real-1",
                    "production_attestation": "midjourney-worker-mcp-item-submit-run-1",
                    "artifacts": ["worlds/demo/output/production/mj.png"]
                }
            }),
            &Some("demo".to_string()),
        )
        .unwrap();
        let body = request.body.unwrap();

        assert_eq!(request.method, "POST");
        assert_eq!(request.path, "/api/production-evidence/items");
        assert_eq!(body["project_slug"], "demo");
        assert_eq!(body["kind"], "provider");
        assert_eq!(body["provider"]["provider_id"], "midjourney");
    }

    #[test]
    fn maps_mcp_runtime_budget_tool_to_runtime_path() {
        let request = mcp_tool_http_request(
            "pool_runtime_budget",
            json!({ "project_slug": "demo" }),
            &None,
        )
        .unwrap();

        assert_eq!(request.method, "GET");
        assert_eq!(request.path, "/api/runtime-budget?project=demo");
        assert!(request.body.is_none());
    }

    #[test]
    fn maps_mcp_runtime_preflight_tool_to_runtime_path() {
        let request = mcp_tool_http_request(
            "pool_runtime_preflight",
            json!({ "project_slug": "demo" }),
            &None,
        )
        .unwrap();

        assert_eq!(request.method, "GET");
        assert_eq!(request.path, "/api/runtime-preflight?project=demo");
        assert!(request.body.is_none());
    }

    #[test]
    fn maps_mcp_runtime_execution_plan_tool_to_runtime_path() {
        let request = mcp_tool_http_request(
            "pool_runtime_execution_plan",
            json!({ "project_slug": "demo" }),
            &None,
        )
        .unwrap();

        assert_eq!(request.method, "GET");
        assert_eq!(request.path, "/api/runtime-execution-plan?project=demo");
        assert!(request.body.is_none());
    }

    #[test]
    fn maps_mcp_runtime_execution_plan_run_next_tool_to_runtime_path() {
        let request = mcp_tool_http_request(
            "pool_runtime_execution_plan_run_next",
            json!({ "project_slug": "demo", "node_id": "node-1", "execute": true }),
            &None,
        )
        .unwrap();

        assert_eq!(request.method, "POST");
        assert_eq!(
            request.path,
            "/api/runtime-execution-plan/run-next?project=demo"
        );
        assert_eq!(request.body.unwrap()["node_id"], "node-1");
    }

    #[test]
    fn maps_mcp_runtime_handoff_tool_to_runtime_path() {
        let request = mcp_tool_http_request(
            "pool_runtime_handoff",
            json!({ "project_slug": "demo" }),
            &None,
        )
        .unwrap();

        assert_eq!(request.method, "GET");
        assert_eq!(request.path, "/api/runtime-handoff?project=demo");
        assert!(request.body.is_none());
    }

    #[test]
    fn maps_mcp_runtime_handoff_packages_tool_to_runtime_path() {
        let request = mcp_tool_http_request(
            "pool_runtime_handoff_packages",
            json!({ "project_slug": "demo" }),
            &None,
        )
        .unwrap();

        assert_eq!(request.method, "GET");
        assert_eq!(request.path, "/api/handoff-packages?project=demo");
        assert!(request.body.is_none());
    }

    #[test]
    fn maps_mcp_core_architecture_readiness_tool_to_runtime_path() {
        let request = mcp_tool_http_request(
            "pool_core_architecture_readiness",
            json!({ "project_slug": "demo" }),
            &None,
        )
        .unwrap();

        assert_eq!(request.method, "GET");
        assert_eq!(
            request.path,
            "/api/core-architecture-readiness?project=demo"
        );
        assert!(request.body.is_none());
    }

    #[test]
    fn maps_mcp_core_architecture_gate_tool_to_runtime_path() {
        let request = mcp_tool_http_request(
            "pool_core_architecture_gate",
            json!({ "project_slug": "demo", "require_ready": true }),
            &None,
        )
        .unwrap();

        assert_eq!(request.method, "GET");
        assert_eq!(
            request.path,
            "/api/core-architecture-gate?require_ready=true&project=demo"
        );
        assert!(request.body.is_none());
    }

    #[test]
    fn maps_mcp_core_architecture_packages_tool_to_runtime_path() {
        let request = mcp_tool_http_request(
            "pool_core_architecture_packages",
            json!({ "project_slug": "demo" }),
            &None,
        )
        .unwrap();

        assert_eq!(request.method, "GET");
        assert_eq!(request.path, "/api/core-architecture-packages?project=demo");
        assert!(request.body.is_none());
    }

    #[test]
    fn maps_mcp_core_architecture_package_tool_to_runtime_path() {
        let request = mcp_tool_http_request(
            "pool_core_architecture_package",
            json!({
                "project_slug": "demo",
                "node_id": "agent",
                "output_dir": "worlds/demo/output",
                "include_snapshot": false
            }),
            &None,
        )
        .unwrap();

        assert_eq!(request.method, "POST");
        assert_eq!(request.path, "/api/core-architecture-package");
        let body = request.body.unwrap();
        assert_eq!(body["project_slug"], "demo");
        assert_eq!(body["node_id"], "agent");
        assert_eq!(body["output_dir"], "worlds/demo/output");
        assert_eq!(body["include_snapshot"], false);
    }

    #[test]
    fn maps_mcp_prd_readiness_tool_to_runtime_path() {
        let request = mcp_tool_http_request(
            "pool_prd_readiness",
            json!({ "project_slug": "demo" }),
            &None,
        )
        .unwrap();

        assert_eq!(request.method, "GET");
        assert_eq!(request.path, "/api/prd-readiness?project=demo");
        assert!(request.body.is_none());
    }

    #[test]
    fn maps_mcp_adapters_tool_to_adapter_catalog_path() {
        let request = mcp_tool_http_request("pool_adapters", json!({}), &None).unwrap();

        assert_eq!(request.method, "GET");
        assert_eq!(request.path, "/api/adapters");
        assert!(request.body.is_none());
    }

    #[test]
    fn maps_mcp_integration_readiness_tool_to_runtime_path() {
        let request = mcp_tool_http_request(
            "pool_integration_readiness",
            json!({ "project_slug": "demo" }),
            &None,
        )
        .unwrap();

        assert_eq!(request.method, "GET");
        assert_eq!(request.path, "/api/integration-readiness?project=demo");
        assert!(request.body.is_none());
    }

    #[test]
    fn maps_mcp_software_contracts_tool_to_runtime_path() {
        let request = mcp_tool_http_request(
            "pool_software_contracts",
            json!({ "adapter_id": "unreal" }),
            &Some("demo".to_string()),
        )
        .unwrap();

        assert_eq!(request.method, "GET");
        assert_eq!(request.path, "/api/software-contracts?adapter_id=unreal");
        assert!(request.body.is_none());
    }

    #[test]
    fn maps_mcp_software_conformance_package_tool_to_runtime_path() {
        let request = mcp_tool_http_request(
            "pool_software_conformance_package",
            json!({
                "project_slug": "demo",
                "adapter_id": "resolve",
                "output_dir": "worlds/demo/output"
            }),
            &None,
        )
        .unwrap();

        assert_eq!(request.method, "POST");
        assert_eq!(request.path, "/api/software-conformance-packages");
        let body = request.body.unwrap();
        assert_eq!(body["project_slug"], "demo");
        assert_eq!(body["adapter_id"], "resolve");
    }

    #[test]
    fn maps_mcp_software_conformance_packages_tool_to_runtime_path() {
        let request = mcp_tool_http_request(
            "pool_software_conformance_packages",
            json!({ "project_slug": "demo" }),
            &None,
        )
        .unwrap();

        assert_eq!(request.method, "GET");
        assert_eq!(
            request.path,
            "/api/software-conformance-packages?project=demo"
        );
        assert!(request.body.is_none());
    }

    #[test]
    fn maps_mcp_provider_conformance_package_tool_to_runtime_path() {
        let request = mcp_tool_http_request(
            "pool_provider_conformance_package",
            json!({
                "project_slug": "demo",
                "provider_id": "worldlabs-marble",
                "output_dir": "worlds/demo/output"
            }),
            &None,
        )
        .unwrap();

        assert_eq!(request.method, "POST");
        assert_eq!(request.path, "/api/provider-conformance-packages");
        let body = request.body.unwrap();
        assert_eq!(body["project_slug"], "demo");
        assert_eq!(body["provider_id"], "worldlabs-marble");
    }

    #[test]
    fn maps_mcp_provider_conformance_packages_tool_to_runtime_path() {
        let request = mcp_tool_http_request(
            "pool_provider_conformance_packages",
            json!({ "project_slug": "demo" }),
            &None,
        )
        .unwrap();

        assert_eq!(request.method, "GET");
        assert_eq!(
            request.path,
            "/api/provider-conformance-packages?project=demo"
        );
        assert!(request.body.is_none());
    }

    #[test]
    fn maps_mcp_agent_conformance_package_tool_to_runtime_path() {
        let request = mcp_tool_http_request(
            "pool_agent_conformance_package",
            json!({
                "project_slug": "demo",
                "kind": "all",
                "output_dir": "worlds/demo/output"
            }),
            &None,
        )
        .unwrap();

        assert_eq!(request.method, "POST");
        assert_eq!(request.path, "/api/agent-conformance-packages");
        let body = request.body.unwrap();
        assert_eq!(body["project_slug"], "demo");
        assert_eq!(body["kind"], "all");
    }

    #[test]
    fn maps_mcp_agent_conformance_packages_tool_to_runtime_path() {
        let request = mcp_tool_http_request(
            "pool_agent_conformance_packages",
            json!({ "project_slug": "demo" }),
            &None,
        )
        .unwrap();

        assert_eq!(request.method, "GET");
        assert_eq!(request.path, "/api/agent-conformance-packages?project=demo");
        assert!(request.body.is_none());
    }

    #[test]
    fn maps_mcp_integration_conformance_package_tool_to_runtime_path() {
        let request = mcp_tool_http_request(
            "pool_integration_conformance_package",
            json!({
                "project_slug": "demo",
                "providers": ["worldlabs-marble"],
                "software_adapters": ["resolve"],
                "agent_kind": "all",
                "output_dir": "worlds/demo/output"
            }),
            &None,
        )
        .unwrap();

        assert_eq!(request.method, "POST");
        assert_eq!(request.path, "/api/integration-conformance-packages");
        let body = request.body.unwrap();
        assert_eq!(body["project_slug"], "demo");
        assert_eq!(body["providers"][0], "worldlabs-marble");
        assert_eq!(body["software_adapters"][0], "resolve");
        assert_eq!(body["agent_kind"], "all");
    }

    #[test]
    fn maps_mcp_integration_conformance_packages_tool_to_runtime_path() {
        let request = mcp_tool_http_request(
            "pool_integration_conformance_packages",
            json!({ "project_slug": "demo" }),
            &None,
        )
        .unwrap();

        assert_eq!(request.method, "GET");
        assert_eq!(
            request.path,
            "/api/integration-conformance-packages?project=demo"
        );
        assert!(request.body.is_none());
    }

    #[test]
    fn maps_mcp_provider_request_metadata_tool_to_runtime_path() {
        let request = mcp_tool_http_request(
            "pool_provider_request_metadata",
            json!({ "provider_request_id": "provider request 1", "project_slug": "demo" }),
            &None,
        )
        .unwrap();

        assert_eq!(request.method, "GET");
        assert_eq!(
            request.path,
            "/api/provider-requests/metadata?provider_request_id=provider%20request%201&project=demo"
        );
        assert!(request.body.is_none());
    }

    #[test]
    fn maps_mcp_handoff_package_tool_to_runtime_path() {
        let request = mcp_tool_http_request(
            "pool_handoff_package",
            json!({
                "project_slug": "demo",
                "node_id": "agent",
                "output_dir": "worlds/demo/output",
                "include_snapshot": true
            }),
            &None,
        )
        .unwrap();

        assert_eq!(request.method, "POST");
        assert_eq!(request.path, "/api/handoff-packages");
        let body = request.body.unwrap();
        assert_eq!(body["project_slug"], "demo");
        assert_eq!(body["node_id"], "agent");
        assert_eq!(body["output_dir"], "worlds/demo/output");
        assert_eq!(body["include_snapshot"], true);
    }

    #[test]
    fn maps_mcp_prd_completion_package_tool_to_runtime_path() {
        let request = mcp_tool_http_request(
            "pool_prd_completion_package",
            json!({
                "project_slug": "demo",
                "node_id": "agent",
                "title": "MCP PRD completion",
                "output_dir": "worlds/demo/output",
                "source": "mcp-test",
                "include_snapshot": true
            }),
            &None,
        )
        .unwrap();

        assert_eq!(request.method, "POST");
        assert_eq!(request.path, "/api/prd-completion-package");
        let body = request.body.unwrap();
        assert_eq!(body["project_slug"], "demo");
        assert_eq!(body["node_id"], "agent");
        assert_eq!(body["title"], "MCP PRD completion");
        assert_eq!(body["output_dir"], "worlds/demo/output");
        assert_eq!(body["source"], "mcp-test");
        assert_eq!(body["include_snapshot"], true);
    }

    #[test]
    fn maps_mcp_workflow_context_tool_to_runtime_path() {
        let request = mcp_tool_http_request(
            "pool_workflow_context",
            json!({ "workflow_id": "workflow 1" }),
            &Some("demo".to_string()),
        )
        .unwrap();

        assert_eq!(request.method, "GET");
        assert_eq!(
            request.path,
            "/api/workflow-context?workflow_id=workflow%201&project=demo"
        );
        assert!(request.body.is_none());
    }

    #[test]
    fn maps_mcp_agent_transcript_tool_to_runtime_path() {
        let request = mcp_tool_http_request(
            "pool_agent_transcript",
            json!({ "session_id": "agent session 1", "project_slug": "demo" }),
            &None,
        )
        .unwrap();

        assert_eq!(request.method, "GET");
        assert_eq!(
            request.path,
            "/api/agent-sessions/transcript?session_id=agent%20session%201&project=demo"
        );
        assert!(request.body.is_none());
    }

    #[test]
    fn maps_mcp_agent_stream_tool_to_runtime_path() {
        let request = mcp_tool_http_request(
            "pool_agent_stream",
            json!({
                "session_id": "agent session 1",
                "project_slug": "demo",
                "after_id": "event 1",
                "limit": 12
            }),
            &None,
        )
        .unwrap();

        assert_eq!(request.method, "GET");
        assert_eq!(
            request.path,
            "/api/agent-sessions/stream?session_id=agent%20session%201&last_event_id=event%201&limit=12&project=demo"
        );
        assert!(request.body.is_none());
    }

    #[test]
    fn maps_mcp_run_software_tool_with_default_project() {
        let request = mcp_tool_http_request(
            "pool_run_software",
            json!({
                "adapter_id": "blender",
                "action_kind": "ExecuteCli",
                "priority": "SkillsCli",
                "payload_json": {
                    "command": "/bin/echo mcp-ok",
                    "allowed_commands": ["/bin/echo", "echo"]
                },
                "evidence_json": {
                    "source": "mcp-test",
                    "control_profile": "skills_cli"
                }
            }),
            &Some("demo".to_string()),
        )
        .unwrap();

        assert_eq!(request.method, "POST");
        assert_eq!(request.path, "/api/software-actions");
        let body = request.body.unwrap();
        assert_eq!(body["project_slug"], "demo");
        assert_eq!(body["adapter_id"], "blender");
        assert_eq!(body["payload_json"]["command"], "/bin/echo mcp-ok");
        assert_eq!(body["evidence_json"]["source"], "mcp-test");
    }

    #[test]
    fn maps_mcp_desktop_run_next_tool_to_runtime_path() {
        let request = mcp_tool_http_request(
            "pool_desktop_run_next",
            json!({
                "project_slug": "demo",
                "controller_id": "mcp-desktop-dry-run",
                "status": "succeeded",
                "limit": 2,
                "artifacts": ["screen-trace.json"]
            }),
            &None,
        )
        .unwrap();

        assert_eq!(request.method, "POST");
        assert_eq!(
            request.path,
            "/api/desktop-recognition/run-next?project=demo"
        );
        let body = request.body.unwrap();
        assert_eq!(body["controller_id"], "mcp-desktop-dry-run");
        assert_eq!(body["status"], "succeeded");
        assert_eq!(body["limit"], 2);
        assert_eq!(body["artifacts"][0], "screen-trace.json");
    }

    #[test]
    fn maps_mcp_task_action_tool_to_runtime_path() {
        let request = mcp_tool_http_request(
            "pool_approve_task",
            json!({ "task_id": "task 1" }),
            &Some("demo".to_string()),
        )
        .unwrap();

        assert_eq!(request.method, "POST");
        assert_eq!(
            request.path,
            "/api/tasks/approve?task_id=task%201&project=demo"
        );
        assert!(request.body.is_none());
    }

    #[test]
    fn rejects_mcp_write_tool_with_wildcard_project() {
        let error =
            mcp_tool_http_request("pool_run_workflow", json!({ "project_slug": "*" }), &None)
                .unwrap_err()
                .to_string();

        assert!(error.contains("concrete project_slug"));
    }

    fn temp_cli_db_path(name: &str) -> PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("pool-cli-{name}-{unique}.sqlite"))
    }

    fn temp_cli_dir(name: &str) -> PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("pool-cli-{name}-{unique}"))
    }
}
