import fs from "node:fs";
import path from "node:path";
import vm from "node:vm";

const repoRoot = path.resolve(import.meta.dirname, "..");
const appPath = path.resolve(repoRoot, process.argv[2] ?? "app.js");
const indexPath = appPath.includes(`${path.sep}apps${path.sep}web-prototype${path.sep}`)
  ? path.resolve(repoRoot, "apps/web-prototype/index.html")
  : path.resolve(repoRoot, "index.html");
const appSource = fs.readFileSync(appPath, "utf8");
const indexSource = fs.readFileSync(indexPath, "utf8");
const fetchCalls = [];
const fetchRequests = [];
const eventSourceUrls = [];
const webSocketUrls = [];
const webSockets = [];

class MockElement {
  constructor(selector) {
    this.selector = selector;
    this.innerHTML = "";
    this.textContent = "";
    this.value = "";
    this.disabled = false;
    this.dataset = {};
    this.style = {};
    this.classList = {
      toggle: () => {},
      add: () => {},
      remove: () => {},
    };
  }

  addEventListener() {}
  setAttribute(name, value) {
    this[name] = value;
  }
  querySelectorAll() {
    return [];
  }
  insertAdjacentHTML(_position, html) {
    this.innerHTML += html;
  }
}

const elements = new Map();

function elementFor(selector) {
  if (!elements.has(selector)) {
    elements.set(selector, new MockElement(selector));
  }
  return elements.get(selector);
}

const document = {
  querySelector(selector) {
    return elementFor(selector);
  },
  querySelectorAll(selector) {
    if (selector === ".panel") return [elementFor("#overview")];
    if (selector === ".rail-item") {
      const item = elementFor("#rail-overview");
      item.dataset.panel = "overview";
      return [item];
    }
    return [];
  },
};

const localStorageStore = new Map();
const localStorage = {
  getItem(key) {
    return localStorageStore.has(key) ? localStorageStore.get(key) : null;
  },
  setItem(key, value) {
    localStorageStore.set(key, String(value));
  },
  removeItem(key) {
    localStorageStore.delete(key);
  },
};

class MockEventSource {
  constructor(url) {
    this.url = String(url);
    this.listeners = new Map();
    eventSourceUrls.push(this.url);
  }

  addEventListener(name, callback) {
    this.listeners.set(name, callback);
  }

  close() {}
}

class MockWebSocket {
  constructor(url) {
    this.url = String(url);
    this.onmessage = null;
    this.onerror = null;
    this.onclose = null;
    webSocketUrls.push(this.url);
    webSockets.push(this);
  }

  emit(payload) {
    this.onmessage?.({ data: JSON.stringify(payload) });
  }

  close() {}
}

const snapshot = {
  version: 1,
  generated_at: "2026-06-11T00:00:00Z",
  project_filter: "demo",
  stats: {
    projects: 1,
    workflows: 1,
    tasks: 2,
    assets: 1,
    events: 1,
    provider_requests: 1,
    software_actions: 0,
    agent_sessions: 0,
    api_keys: 0,
    waiting_approval: 1,
    running: 0,
    failed: 0,
    token_total: 9000,
    agent_token_budget: 0,
  },
  projects: [
    {
      id: "project-demo",
      slug: "demo",
      name: "Pool demo",
      status: "active",
      created_at: "2026-06-11T00:00:00Z",
      updated_at: "2026-06-11T00:00:00Z",
    },
  ],
  workflows: [],
  node_states: [],
  tasks: [
    {
      id: "task-brief",
      project_slug: "demo",
      node_id: "brief",
      title: "起始输入",
      status: "Ready",
      provider_id: null,
      cost_estimate_tokens: 0,
      requires_approval: false,
      request_metadata_path: null,
      created_at: "2026-06-11T00:00:00Z",
      updated_at: "2026-06-11T00:00:00Z",
    },
    {
      id: "task-three",
      project_slug: "demo",
      node_id: "three",
      title: "2D/3DGS 转换",
      status: "WaitingApproval",
      provider_id: "worldlabs-marble",
      cost_estimate_tokens: 9000,
      requires_approval: true,
      request_metadata_path: "worlds/demo/output/.1-3dgs-request.json",
      created_at: "2026-06-11T00:00:00Z",
      updated_at: "2026-06-11T00:00:00Z",
    },
  ],
  assets: [
    {
      id: "asset-three",
      project_slug: "demo",
      name: "1-3dgs-scene.glb",
      asset_type: "Model3D",
      local_path: "worlds/demo/output/1-3dgs-scene.glb",
      source_node_id: "three",
      provider_url: null,
      status: "Local",
      created_at: "2026-06-11T00:00:00Z",
    },
  ],
  events: [
    {
      id: "event-1",
      project_slug: "demo",
      level: "Info",
      message: "runtime ready",
      created_at: "2026-06-11T00:00:00Z",
    },
  ],
  provider_requests: [
    {
      id: "request-three",
      task_id: "task-three",
      project_slug: "demo",
      provider_id: "worldlabs-marble",
      request: {
        execution_mode: "mock",
        provider_request: {
          prompt: "3DGS scene",
          require_approval: true,
        },
      },
      response: {
        status: "waiting_approval",
      },
      metadata_path: "worlds/demo/output/.1-3dgs-request.json",
      created_at: "2026-06-11T00:00:00Z",
    },
  ],
  software_actions: [],
  agent_sessions: [],
  api_keys: [],
};

let agentSessionPersisted = false;
const agentSessionTask = {
  id: "task-agent-cli",
  project_slug: "demo",
  node_id: "agent",
  title: "读取工作流上下文",
  status: "Ready",
  provider_id: null,
  cost_estimate_tokens: 4000,
  requires_approval: false,
  request_metadata_path: "worlds/demo/output/agent-cli-transcript.json",
  created_at: "2026-06-11T00:01:00Z",
  updated_at: "2026-06-11T00:01:00Z",
};
const agentSession = {
  id: "agent-session-cli",
  project_slug: "demo",
  tools: ["cli", "mcp"],
  token_budget: 74000,
  token_used: 1200,
  transcript_path: "worlds/demo/output/agent-cli-transcript.json",
  created_at: "2026-06-11T00:01:00Z",
  updated_at: "2026-06-11T00:01:00Z",
};

function runtimeSnapshotPayload() {
  if (!agentSessionPersisted) return snapshot;
  return {
    ...snapshot,
    stats: {
      ...snapshot.stats,
      tasks: snapshot.stats.tasks + 1,
      agent_sessions: 1,
      agent_token_used: 1200,
      agent_token_budget: 74000,
    },
    tasks: [...snapshot.tasks, agentSessionTask],
    agent_sessions: [agentSession],
  };
}

const runtimeGraph = {
  project_filter: "demo",
  generated_at: "2026-06-11T00:00:00Z",
  summary: {
    workflows: 1,
    nodes: 2,
    edges: 1,
    waiting_approval: 1,
    running: 0,
    failed: 0,
  },
  workflows: [
    {
      workflow_id: "workflow-demo",
      project_id: "project-demo",
      name: "creative input to multi-output runtime",
      nodes: [
        {
          id: "brief",
          title: "起始输入",
          node_type: "Input",
          task_type: "creative_input",
          status: "Ready",
          static_status: "Ready",
          provider_id: null,
          software_adapter_id: null,
          requires_approval: false,
          cost_estimate_tokens: 0,
          position: { x: 0, y: 100 },
          parameters: {},
          latest_task: snapshot.tasks[0],
          tasks: [snapshot.tasks[0]],
          asset_count: 0,
          provider_request_count: 0,
          software_action_count: 0,
          can_run: true,
          blocked_by_approval: false,
        },
        {
          id: "three",
          title: "2D/3DGS 转换",
          node_type: "ThreeDgs",
          task_type: "3dgs",
          status: "WaitingApproval",
          static_status: "WaitingApproval",
          provider_id: "worldlabs-marble",
          software_adapter_id: null,
          requires_approval: true,
          cost_estimate_tokens: 9000,
          position: { x: 420, y: 100 },
          parameters: {},
          latest_task: snapshot.tasks[1],
          tasks: [snapshot.tasks[1]],
          asset_count: 1,
          provider_request_count: 1,
          software_action_count: 0,
          can_run: false,
          blocked_by_approval: true,
        },
      ],
      edges: [
        {
          id: "edge-brief-three",
          from_node_id: "brief",
          to_node_id: "three",
          from_title: "起始输入",
          to_title: "2D/3DGS 转换",
          from_status: "Ready",
          to_status: "WaitingApproval",
          kind: "AssetFlow",
          channel: "asset",
          label: "generated plates",
        },
      ],
    },
  ],
};

const runtimeExecutionPlan = {
  kind: "pool_runtime_execution_plan",
  version: 1,
  generated_at: "2026-06-11T00:00:00Z",
  summary: {
    workflows: 1,
    steps: 2,
    runnable_steps: 1,
    gated_steps: 1,
    phase_counts: {
      ready: 1,
      waiting_approval: 1,
    },
    task_type_counts: {
      creative_input: 1,
      "3dgs": 1,
    },
  },
  policy: {
    graph_is_execution_source: true,
    local_files_authoritative: true,
    provider_urls_are_provenance: true,
    high_cost_steps_require_approval: true,
    control_priority: ["API/MCP", "Skills/CLI", "Desktop Recognition", "Human Takeover"],
  },
  workflows: [
    {
      workflow_id: "workflow-demo",
      project_slug: "demo",
      name: "creative input to multi-output runtime",
      topology_complete: true,
      summary: {
        steps: 2,
        runnable_steps: 1,
        gated_steps: 1,
      },
      steps: [
        {
          id: "workflow-demo::brief",
          sequence: 1,
          workflow_id: "workflow-demo",
          node_id: "brief",
          title: "起始输入",
          task_type: "creative_input",
          node_type: "Input",
          status: "Ready",
          phase: "ready",
          gate: { kind: "none" },
          contracts: [
            { kind: "workflow_context", mcp_uri: "pool://workflow/workflow-demo" },
            { kind: "node_context", mcp_uri: "pool://node-context/brief" },
          ],
          control: {
            recommended_action: {
              kind: "run_node",
              command: "pool-cli --project demo run-node brief",
              mcp_tool: "pool_run_node",
            },
          },
        },
        {
          id: "workflow-demo::three",
          sequence: 2,
          workflow_id: "workflow-demo",
          node_id: "three",
          title: "2D/3DGS 转换",
          task_type: "3dgs",
          node_type: "ThreeDgs",
          status: "WaitingApproval",
          phase: "waiting_approval",
          provider_id: "worldlabs-marble",
          gate: {
            kind: "approval",
            task_id: "task-three",
            command: "pool-cli --project demo approve-task task-three",
          },
          contracts: [
            { kind: "workflow_context", mcp_uri: "pool://workflow/workflow-demo" },
            { kind: "node_context", mcp_uri: "pool://node-context/three" },
            { kind: "provider_contract", mcp_uri: "pool://provider-contracts/worldlabs-marble" },
          ],
          control: {
            recommended_action: {
              kind: "approval",
              command: "pool-cli --project demo approve-task task-three",
              mcp_tool: "pool_approve_task",
            },
          },
        },
      ],
    },
  ],
  next_steps: [
    {
      id: "workflow-demo::three",
      sequence: 2,
      workflow_id: "workflow-demo",
      node_id: "three",
      title: "2D/3DGS 转换",
      task_type: "3dgs",
      status: "WaitingApproval",
      phase: "waiting_approval",
      gate: {
        kind: "approval",
        task_id: "task-three",
        command: "pool-cli --project demo approve-task task-three",
      },
      contracts: [
        { kind: "provider_contract", mcp_uri: "pool://provider-contracts/worldlabs-marble" },
      ],
      control: {
        recommended_action: {
          kind: "approval",
          command: "pool-cli --project demo approve-task task-three",
          mcp_tool: "pool_approve_task",
        },
      },
    },
  ],
};

const nodeContext = {
  project_filter: "demo",
  generated_at: "2026-06-11T00:00:00Z",
  node_id: "three",
  workflow_id: "workflow-demo",
  workflow: {
    id: "workflow-demo",
    project_id: "project-demo",
    shot_id: null,
    name: "creative input to multi-output runtime",
  },
  node: runtimeGraph.workflows[0].nodes[1],
  node_states: [],
  incoming_edges: [runtimeGraph.workflows[0].edges[0]],
  outgoing_edges: [],
  tasks: [snapshot.tasks[1]],
  assets: [snapshot.assets[0]],
  provider_requests: [snapshot.provider_requests[0]],
  software_actions: [],
  agent_sessions: [],
  control_context: {
    project_slug: "demo",
    task_type: "3dgs",
    provider: {
      id: "worldlabs-marble",
      registered: true,
      config: {
        id: "worldlabs-marble",
        display_name: "World Labs Marble",
        output_contract: "image-blaster indexed local 3DGS package",
      },
    },
    software_adapter: null,
    control_priority_chain: ["ApiMcp", "SkillsCli", "DesktopRecognition", "HumanTakeover"],
    mcp_resources: ["pool://adapters", "pool://node-context/three"],
    mcp_tools: [
      {
        name: "pool_provider_health",
        arguments: { project_slug: "demo", provider_id: "worldlabs-marble" },
      },
      {
        name: "pool_run_provider",
        arguments: { project_slug: "demo", provider_id: "worldlabs-marble", node_id: "three" },
      },
    ],
    cli_commands: [
      {
        kind: "provider_run",
        command: "pool-cli --project demo run-provider worldlabs-marble --execution-mode auto --node-id three --output-dir worlds/demo/output",
      },
    ],
  },
  summary: {
    tasks: 1,
    assets: 1,
    provider_requests: 1,
    software_actions: 0,
    agent_sessions: 0,
    node_states: 0,
    incoming_edges: 1,
    outgoing_edges: 0,
    blocked_by_approval: true,
  },
};

const workflowContext = {
  project_filter: "demo",
  generated_at: "2026-06-11T00:00:00Z",
  workflow_id: "workflow-demo",
  workflow: {
    id: "workflow-demo",
    project_id: "project-demo",
    shot_id: null,
    name: "creative input to multi-output runtime",
  },
  graph: runtimeGraph.workflows[0],
  node_states: [],
  tasks: snapshot.tasks,
  assets: snapshot.assets,
  provider_requests: snapshot.provider_requests,
  software_actions: [],
  agent_sessions: [],
  summary: {
    nodes: 2,
    tasks: 2,
    assets: 1,
    provider_requests: 1,
    software_actions: 0,
    agent_sessions: 0,
    node_states: 0,
    waiting_approval: 1,
    running: 0,
    failed: 0,
    blocked_by_approval: true,
  },
};

const runtimeBudget = {
  project_filter: "demo",
  generated_at: "2026-06-11T00:00:00Z",
  summary: {
    task_estimated_tokens: 9000,
    waiting_approval_estimated_tokens: 9000,
    agent_token_used: 1200,
    agent_token_budget: 74000,
    token_total: 9000,
    budget_remaining: 72800,
    configured_api_keys: 0,
    tracked_providers: 1,
    missing_runtime_credentials: 1,
    provider_requests: 1,
    approval_gates: 1,
  },
  provider_credentials: [
    {
      provider_id: "worldlabs-marble",
      configured: false,
      key_hint: null,
      task_count: 1,
      provider_request_count: 1,
      token_estimate: 9000,
      waiting_approval_tokens: 9000,
      last_request_at: "2026-06-11T00:00:00Z",
      credential_status: "not_recorded",
    },
  ],
  approval_gates: [snapshot.tasks[1]],
};

const runtimeApiKeys = {
  api_keys: [
    {
      id: "api-key-openai",
      provider: "openai-image-2",
      service_type: "provider",
      configured: true,
      key_hint: "...cret",
      metadata: {
        source: "env",
        env: "OPENAI_API_KEY",
        owner: "generation-td",
        rotation_days: 30,
        credential: {
          storage: "pool:v1:aes256gcm",
          backend: "sqlite-encrypted",
          encrypted: true,
          key_hint: "...cret",
        },
      },
      created_at: "2026-01-01T00:00:00Z",
      updated_at: "2026-01-01T00:00:00Z",
    },
  ],
  audit: {
    kind: "pool_api_key_audit",
    default_rotation_days: 90,
    total: 1,
    configured: 1,
    rotation_due: 1,
    unencrypted: 0,
    items: [
      {
        provider: "openai-image-2",
        service_type: "provider",
        configured: true,
        key_hint: "...cret",
        source: "env",
        env: "OPENAI_API_KEY",
        owner: "generation-td",
        storage: "pool:v1:aes256gcm",
        backend: "sqlite-encrypted",
        encrypted: true,
        created_at: "2026-01-01T00:00:00Z",
        updated_at: "2026-01-01T00:00:00Z",
        age_days: 90,
        rotation_days: 30,
        rotation_due: true,
      },
    ],
  },
};

const runtimePreflight = {
  project_filter: "demo",
  generated_at: "2026-06-11T00:00:00Z",
  ready: false,
  summary: {
    blocked: 1,
    warnings: 1,
    passed: 4,
    checks: 6,
    runnable_nodes: 1,
    blocked_nodes: 1,
    approval_gates: 1,
    missing_credentials: 1,
    desktop_handoffs: 0,
    failed_tasks: 0,
  },
  checks: [
    {
      id: "approval_gates",
      status: "blocked",
      title: "Approval gates",
      detail: "One or more high-cost tasks need approval.",
      action: "Approve or cancel waiting tasks before running the full workflow.",
    },
    {
      id: "provider_credentials",
      status: "warning",
      title: "Provider credentials",
      detail: "Some tracked providers do not have a saved runtime credential.",
      action: "Save provider keys or pass credentials through the request/env.",
    },
  ],
  next_actions: [
    {
      kind: "approval",
      title: "Approve task: 2D/3DGS 转换",
      task_id: "task-three",
      command: "pool-cli --project demo approve-task task-three",
    },
    {
      kind: "local_worker_self_check",
      title: "Run local worker bridge self-checks",
      command: "pool-cli worker-self-checks --output-root target/pool-worker-self-checks --software-adapter resolve",
      mcp_tool: "pool_worker_self_checks",
      optional: true,
    },
  ],
  runnable_nodes: [runtimeGraph.workflows[0].nodes[0]],
  blocked_nodes: [runtimeGraph.workflows[0].nodes[1]],
};

const prompts = {
  prompts: [
    {
      name: "pool_software_handoff",
      title: "Pool Software Handoff",
      description: "Prepare a safe external software control action.",
      arguments: [
        { name: "project_slug", required: false },
        { name: "adapter_id", required: true },
        { name: "action_kind", required: false },
      ],
    },
  ],
};

const providerContracts = {
  kind: "pool_provider_contracts",
  contracts: [
    {
      provider_id: "worldlabs-marble",
      adapter_kind: "three_dgs_http_gateway",
      gateway_submit: {
        path: "/v1/3dgs/jobs",
      },
      gateway_poll: {
        path_template: "/v1/3dgs/jobs/{job_id}",
      },
      profile: {
        profile_id: "worldlabs-marble",
        task_type: "marble_world_generation",
      },
      local_output_policy: {
        output_contract: "image-blaster-indexed-files",
        local_files_authoritative: true,
      },
    },
    {
      provider_id: "midjourney",
      adapter_kind: "generic_http_media_gateway",
      gateway_submit: {
        path: "/v1/media/jobs",
      },
      gateway_poll: {
        path_template: "/v1/media/jobs/{job_id}",
      },
      profile: {
        profile_id: "midjourney",
        task_type: "midjourney_imagine",
      },
      local_output_policy: {
        local_files_authoritative: true,
      },
    },
  ],
};

const providerGatewayWorkerContract = {
  kind: "pool_provider_gateway_worker_contract",
  service: "pool-provider-gateway-worker",
  purpose: "Local HTTP forwarder for Pool AI media and 3DGS gateway requests.",
  cli: {
    primary: "pool-cli provider-gateway-worker --bind 127.0.0.1:8788 --upstream http://127.0.0.1:8787",
  },
  pool_adapter_usage: {
    ai_media: {
      endpoint_env: "POOL_MEDIA_GATEWAY_ENDPOINT",
    },
    three_dgs: {
      endpoint_env: "POOL_3DGS_GATEWAY_ENDPOINT",
    },
  },
  conformance_runbook: {
    phases: [
      {
        id: "local_mock_baseline",
        command: "pool-cli provider-gateway-worker --once",
        production_evidence: false,
      },
      {
        id: "three_dgs_smoke",
        command: "POOL_3DGS_GATEWAY_ENDPOINT=http://127.0.0.1:8788 cargo run -p pool-core --example three_dgs_gateway_smoke -- request.json target/three-dgs-worker-smoke worldlabs-marble",
        production_evidence: false,
      },
      {
        id: "production_matrix",
        command: "POOL_PROVIDER_PRODUCTION_ATTESTATION=real-worker-run pool-cli --project demo production-evidence-provider-matrix target/provider-evidence-matrix --production-upstream --evidence-bundle=target/provider-evidence-matrix/provider-production-evidence-bundle.json",
        production_evidence: true,
      },
    ],
    pass_conditions: [
      "All outputs are downloadable by Pool into local files",
      "Production evidence uses a real non-placeholder attestation",
    ],
  },
};

const runtimeDiscovery = {
  service: "pool-runtime",
  version: 1,
  base_url: "http://127.0.0.1:4788",
  project_filter: "demo",
  capabilities: {
    runtime_execution_plan: true,
    agent_sessions: true,
    event_websocket: true,
    event_stream_transports: ["websocket", "sse", "polling"],
    mcp_resources: true,
    mcp_tools: true,
    mcp_prompts: true,
  },
  endpoints: {
    events_websocket: "/api/events/ws",
    runtime_execution_plan: "/api/runtime-execution-plan",
    runtime_execution_plan_run_next: "/api/runtime-execution-plan/run-next",
    agent_sessions: "/api/agent-sessions",
    agent_session_stream: "/api/agent-sessions/stream?session_id=<agent-session-id>",
    provider_gateway_worker: "/api/provider-gateway-worker",
    integration_readiness: "/api/integration-readiness",
    provider_conformance_packages: "/api/provider-conformance-packages",
    integration_conformance_packages: "/api/integration-conformance-packages",
    software_conformance_packages: "/api/software-conformance-packages",
    agent_conformance_packages: "/api/agent-conformance-packages",
    desktop_recognition_requests: "/api/desktop-recognition/requests",
    agent_session_websocket: "/api/agent-sessions/ws?session_id=<agent-session-id>",
  },
  mcp_resources: [
    { uri: "pool://runtime-execution-plan", http_path: "/api/mcp?uri=pool%3A%2F%2Fruntime-execution-plan" },
    { uri: "pool://integration-readiness", http_path: "/api/mcp?uri=pool%3A%2F%2Fintegration-readiness" },
    { uri: "pool://provider-gateway-worker", http_path: "/api/mcp?uri=pool%3A%2F%2Fprovider-gateway-worker" },
    { uri: "pool://agent-sessions", http_path: "/api/mcp?uri=pool%3A%2F%2Fagent-sessions" },
  ],
  mcp_tools: [
    { name: "pool_provider_gateway_worker", category: "read", transport: "mcp_stdio" },
    { name: "pool_integration_readiness", category: "read", transport: "mcp_stdio" },
    { name: "pool_worker_self_checks", category: "local_smoke", transport: "mcp_stdio" },
    { name: "pool_handoff_package", category: "write", transport: "mcp_stdio" },
    { name: "pool_provider_conformance_package", category: "write", transport: "mcp_stdio" },
    { name: "pool_integration_conformance_package", category: "write", transport: "mcp_stdio" },
    { name: "pool_software_conformance_package", category: "write", transport: "mcp_stdio" },
    { name: "pool_agent_conformance_package", category: "write", transport: "mcp_stdio" },
    { name: "pool_run_software", category: "write", transport: "mcp_stdio" },
  ],
  mcp_prompts: [
    { name: "pool_content_burst_runbook", http_path: "/api/prompts?name=pool_content_burst_runbook" },
    { name: "pool_software_handoff", http_path: "/api/prompts?name=pool_software_handoff" },
  ],
};

const integrationReadiness = {
  kind: "pool_integration_readiness",
  project_filter: "demo",
  generated_at: "2026-06-11T00:00:00Z",
  summary: {
    providers: 2,
    software_adapters: 3,
    agent_sessions: 1,
    ready: 2,
    needs_configuration: 1,
    needs_execution: 3,
    needs_attention: 0,
    total: 6,
    lanes: 5,
    actions: 3,
  },
  lanes: [
    { lane: "orchestration", title: "制片 / Agent 编排", owner: "Producer + Agent operator", total: 1, ready: 1, needs_attention: 0, targets: ["agent"] },
    { lane: "ai_media", title: "AI 素材生成", owner: "AI image/video/audio operator", total: 1, ready: 0, needs_attention: 0, targets: ["midjourney"] },
    { lane: "spatial_engine", title: "3D / 引擎组装", owner: "3DGS + engine operator", total: 2, ready: 1, needs_attention: 0, targets: ["worldlabs-marble", "unreal"] },
    { lane: "post_output", title: "视频 / 后期输出", owner: "Editor + compositor", total: 1, ready: 0, needs_attention: 0, targets: ["resolve"] },
    { lane: "interactive_systems", title: "交互 / 现场系统", owner: "Interactive systems operator", total: 0, ready: 0, needs_attention: 0, targets: [] },
  ],
  run_plan: [
    {
      priority: 2,
      lane: "ai_media",
      target_kind: "provider",
      target_id: "midjourney",
      display_name: "Midjourney",
      status: "needs_configuration",
      action: {
        kind: "configure_key",
        label: "配置 Provider Key",
        command: "pool-cli --project <slug> set-api-key midjourney --api-key-env <ENV>",
        reason: "Provider 还没有可用凭证或运行证据。",
      },
    },
    {
      priority: 3,
      lane: "spatial_engine",
      target_kind: "provider",
      target_id: "worldlabs-marble",
      display_name: "World Labs Marble",
      status: "needs_execution",
      action: {
        kind: "run_provider_smoke",
        label: "执行 Provider smoke",
        command: "pool-cli --project <slug> run-provider worldlabs-marble --execution-mode mock --no-approval --prompt \"integration smoke\"",
        reason: "凭证或请求账本已存在，下一步运行 Provider smoke 形成成功记录。",
      },
    },
    {
      priority: 3,
      lane: "post_output",
      target_kind: "software",
      target_id: "resolve",
      display_name: "DaVinci Resolve",
      status: "needs_execution",
      action: {
        kind: "run_software_smoke",
        label: "执行软件 smoke",
        command: "pool-cli --project <slug> run-software resolve --action execute-cli --priority SkillsCli",
        reason: "软件 adapter 尚无成功控制记录。",
      },
    },
  ],
  providers: [
    {
      provider_id: "worldlabs-marble",
      display_name: "World Labs Marble",
      kind: "ThreeDgs",
      lane: "spatial_engine",
      status: "needs_execution",
      next_action: {
        kind: "run_provider_smoke",
        label: "执行 Provider smoke",
        command: "pool-cli --project <slug> run-provider worldlabs-marble --execution-mode mock --no-approval --prompt \"integration smoke\"",
        reason: "凭证或请求账本已存在，下一步运行 Provider smoke 形成成功记录。",
      },
      configured: true,
      key_hint: "marble-key",
      task_count: 1,
      request_count: 1,
      success_count: 0,
      failed_count: 0,
      waiting_approval_count: 1,
      commands: {
        health: "pool-cli --project <slug> provider-health worldlabs-marble",
        conformance_package: "pool-cli --project <slug> provider-conformance-package worldlabs-marble --output-dir worlds/<slug>/output",
      },
    },
    {
      provider_id: "midjourney",
      display_name: "Midjourney",
      kind: "AiImage",
      lane: "ai_media",
      status: "needs_configuration",
      next_action: {
        kind: "configure_key",
        label: "配置 Provider Key",
        command: "pool-cli --project <slug> set-api-key midjourney --api-key-env <ENV>",
        reason: "Provider 还没有可用凭证或运行证据。",
      },
      configured: false,
      task_count: 0,
      request_count: 0,
      success_count: 0,
      failed_count: 0,
      waiting_approval_count: 0,
      commands: {
        health: "pool-cli --project <slug> provider-health midjourney",
      },
    },
  ],
  software_adapters: [
    {
      adapter_id: "unreal",
      display_name: "Unreal",
      lane: "spatial_engine",
      status: "ready",
      next_action: {
        kind: "verify_production",
        label: "归档软件证据",
        command: "pool-cli --project <slug> software-conformance-package unreal --output-dir worlds/<slug>/output",
      },
      control_modes: ["api/mcp", "skills/cli", "desktop-recognition"],
      desktop_fallback: true,
      action_count: 1,
      task_count: 0,
      success_count: 1,
      failed_count: 0,
      commands: {
        health: "pool-cli --project <slug> software-health unreal",
        conformance_package: "pool-cli --project <slug> software-conformance-package unreal --output-dir worlds/<slug>/output",
      },
    },
    {
      adapter_id: "resolve",
      display_name: "DaVinci Resolve",
      lane: "post_output",
      status: "needs_execution",
      next_action: {
        kind: "run_software_smoke",
        label: "执行软件 smoke",
        command: "pool-cli --project <slug> run-software resolve --action execute-cli --priority SkillsCli",
      },
      control_modes: ["api/mcp", "skills/cli", "desktop-recognition"],
      desktop_fallback: true,
      action_count: 0,
      task_count: 0,
      success_count: 0,
      failed_count: 0,
      commands: {
        health: "pool-cli --project <slug> software-health resolve",
      },
    },
  ],
  agent: {
    lane: "orchestration",
    status: "ready",
    next_action: {
      kind: "verify_handoff",
      label: "归档 Agent 交接",
      command: "pool-cli --project <slug> agent-conformance-package all --output-dir worlds/<slug>/output",
    },
    sessions: 1,
    transcripts: 1,
    commands: {
      conformance_package: "pool-cli --project <slug> agent-conformance-package all --output-dir worlds/<slug>/output",
    },
  },
  commands: {
    integration_conformance_package: "pool-cli --project <slug> integration-conformance-package --output-dir worlds/<slug>/output",
  },
};

const softwareContracts = {
  kind: "pool_software_control_contracts",
  contracts: [
    {
      kind: "pool_software_control_contract",
      adapter_id: "unreal",
      display_name: "Unreal",
      control_modes: ["api/mcp", "skills/cli", "desktop-recognition"],
      runtime_action: {
        method: "POST",
        path: "/api/software-actions",
        body: {
          priority: "ApiMcp",
          action_kind: "CreateScene",
        },
      },
      control_routes: [
        {
          priority: "ApiMcp",
          adapter_kind: "unreal_mcp",
        },
      ],
    },
    {
      kind: "pool_software_control_contract",
      adapter_id: "touchdesigner",
      display_name: "TouchDesigner",
      control_modes: ["python-api", "osc", "desktop-recognition"],
      runtime_action: {
        method: "POST",
        path: "/api/software-actions",
        body: {
          priority: "SkillsCli",
          action_kind: "RunViewport",
        },
      },
      control_routes: [
        {
          priority: "SkillsCli",
          adapter_kind: "command_software_adapter",
        },
      ],
    },
    {
      kind: "pool_software_control_contract",
      adapter_id: "resolve",
      display_name: "DaVinci Resolve",
      control_modes: ["api/mcp", "skills/cli", "desktop-recognition"],
      runtime_action: {
        method: "POST",
        path: "/api/software-actions",
        body: {
          priority: "ApiMcp",
          action_kind: "Transcode",
        },
      },
      control_routes: [
        {
          priority: "ApiMcp",
          adapter_kind: "generic_software_api_mcp",
          local_worker: {
            cli: "pool-cli software-api-bridge-worker resolve --bind 127.0.0.1:8793 --output-root worlds/demo/output",
            endpoint_env: "POOL_RESOLVE_ENDPOINT=http://127.0.0.1:8793",
            forwarder: "add --upstream <url> to proxy to a real Resolve gateway",
          },
        },
      ],
      conformance_runbook: {
        phases: [
          {
            id: "local_bridge_baseline",
            command: "pool-cli software-api-bridge-worker resolve --once --output-root worlds/demo/output",
          },
          {
            id: "real_upstream_bridge",
            command: "pool-cli software-api-bridge-worker resolve --bind 127.0.0.1:8793 --output-root worlds/demo/output --upstream <real-plugin-or-gateway-url>",
          },
          {
            id: "software_health",
            command: "pool-cli --project demo software-health resolve --endpoint http://127.0.0.1:8793",
          },
          {
            id: "software_action_smoke",
            command: "pool-cli --project demo run-software resolve --action-kind Transcode --priority ApiMcp --endpoint http://127.0.0.1:8793 --payload-json '{\"mcp_path\":\"/mcp\"}' --no-confirmation",
          },
          {
            id: "production_matrix",
            command: "POOL_RESOLVE_ENDPOINT=http://127.0.0.1:8793 POOL_RESOLVE_ARTIFACTS=worlds/demo/output/resolve-software-smoke.json POOL_RESOLVE_PRODUCTION_ATTESTATION=<real-software-run-id> pool-cli --project demo production-evidence-software-matrix target/software-evidence-matrix --production-software --software-endpoint-env resolve=POOL_RESOLVE_ENDPOINT --software-artifacts-env resolve=POOL_RESOLVE_ARTIFACTS --software-attestation-env resolve=POOL_RESOLVE_PRODUCTION_ATTESTATION --evidence-bundle=target/software-evidence-matrix/software-production-evidence-bundle.json",
          },
        ],
        pass_conditions: [
          "software action result includes local file artifacts",
          "production matrix includes endpoint, local artifacts, and non-placeholder production attestation",
        ],
      },
    },
  ],
};

const desktopRecognitionContract = {
  kind: "pool_desktop_recognition_contract",
  summary: {
    request_contract: "desktop-recognition-control-request",
    software_targets: 6,
  },
  queue: {
    read_requests: {
      http: "GET /api/desktop-recognition/requests",
    },
    result_callback: {
      http: "POST /api/desktop-recognition/results",
    },
  },
  result_callback: {
    statuses: [
      "queued_for_desktop_recognition",
      "running",
      "succeeded",
      "failed",
      "retryable",
      "cancelled",
    ],
  },
  software_targets: [
    {
      adapter_id: "touchdesigner",
      display_name: "TouchDesigner",
    },
  ],
};

const runtimeHandoff = {
  project_filter: "demo",
  generated_at: "2026-06-11T00:00:00Z",
  ready: false,
  summary: {
    lanes: 7,
    commands: 4,
    approval_actions: 1,
    retry_actions: 0,
    credential_actions: 1,
    local_worker_actions: 1,
    handoff_package_actions: 1,
    desktop_requests: 0,
    runnable_node_actions: 1,
    team_roles: 5,
  },
  team: {
    size: 5,
    mode: "five_person_content_burst_team",
    roles: [
      {
        id: "creative_director",
        title: "Creative Director",
        focus: "创意验收、审批门和参考方向把关",
        primary_surface: "human_approval",
        status: "blocked",
        queue_count: 1,
        assigned_lane_ids: ["manual_approval"],
      },
      {
        id: "agent_operator",
        title: "Agent Operator",
        focus: "Hermes/Agent CLI 上下文读取、失败恢复和自动化调度",
        primary_surface: "agent_cli_mcp",
        status: "ready",
        queue_count: 2,
        assigned_lane_ids: ["agent_context", "local_worker_smoke", "handoff_package"],
      },
    ],
  },
  control_priority: ["API/MCP", "Skills/CLI", "Desktop Recognition", "Human Takeover"],
  lanes: [
    {
      id: "agent_context",
      title: "Agent/Hermes context load",
      team_role: "agent_operator",
      executor: "hermes_or_agent_cli",
      status: "ready",
      commands: [
        "pool-cli --project demo runtime-preflight",
        "pool-cli --project demo runtime-graph",
      ],
      actions: [],
      requests: [],
    },
    {
      id: "manual_approval",
      title: "Approval gates",
      team_role: "creative_director",
      executor: "human_operator_or_approved_agent",
      status: "blocked",
      actions: [
        {
          kind: "approval",
          title: "Approve task: 2D/3DGS 转换",
          command: "pool-cli --project demo approve-task task-three",
        },
      ],
      requests: [],
    },
    {
      id: "local_worker_smoke",
      title: "Local worker bridge self-checks",
      team_role: "agent_operator",
      executor: "hermes_or_agent_cli",
      status: "ready",
      actions: [
        {
          kind: "local_worker_self_check",
          title: "Run local worker bridge self-checks",
          command: "pool-cli worker-self-checks --output-root target/pool-worker-self-checks --software-adapter resolve",
          mcp_tool: "pool_worker_self_checks",
        },
      ],
      requests: [],
    },
    {
      id: "handoff_package",
      title: "Offline handoff package",
      team_role: "agent_operator",
      executor: "hermes_or_agent_cli",
      status: "ready",
      actions: [
        {
          kind: "handoff_package",
          title: "Write runtime handoff package",
          command: "pool-cli --project demo handoff-package --node-id agent --output-dir worlds/demo/output --include-snapshot",
          mcp_tool: "pool_handoff_package",
        },
      ],
      requests: [],
    },
  ],
  commands: [
    {
      lane: "agent_context",
      title: "Runtime preflight",
      command: "pool-cli --project demo runtime-preflight",
    },
    {
      lane: "manual_approval",
      kind: "approval",
      title: "Approve task: 2D/3DGS 转换",
      command: "pool-cli --project demo approve-task task-three",
    },
    {
      lane: "local_worker_smoke",
      kind: "local_worker_self_check",
      title: "Run local worker bridge self-checks",
      command: "pool-cli worker-self-checks --output-root target/pool-worker-self-checks --software-adapter resolve",
    },
    {
      lane: "handoff_package",
      kind: "handoff_package",
      title: "Write runtime handoff package",
      command: "pool-cli --project demo handoff-package --node-id agent --output-dir worlds/demo/output --include-snapshot",
    },
  ],
};

const prdReadiness = {
  kind: "pool_prd_readiness",
  version: 1,
  project_filter: "demo",
  generated_at: "2026-06-11T00:00:00Z",
  overall_status: "partial",
  summary: {
    total: 10,
    ready: 7,
    partial: 3,
    blocked: 0,
  },
  completion_gate: {
    status: "incomplete",
    ready_for_completion: false,
    incomplete_requirements: [
      {
        id: "ai_media_and_3dgs_providers",
        status: "partial",
        gaps: ["Real vendor SDK/service credentials are not proven by the runtime snapshot."],
      },
    ],
    proof_commands: {
      readiness: "pool-cli --project demo prd-readiness",
      closeout_preflight: "pool-cli --project demo closeout-production-evidence --output <merged-bundle.json> <provider-bundle.json> <software-bundle.json> <desktop-vision-bundle.json>",
    },
  },
  requirements: [
    {
      id: "node_graph_execution",
      title: "Node graph as executable plan",
      status: "ready",
      summary: "Runtime graph and execution plan are available.",
      gaps: [],
      next_actions: ["pool-cli --project demo runtime-run-next"],
    },
    {
      id: "ai_media_and_3dgs_providers",
      title: "AI image/video/audio and 2D/3D/3DGS provider adapters",
      status: "partial",
      summary: "Gateway contracts are ready; real upstreams still need credentials.",
      gaps: ["Real vendor SDK/service credentials are not proven by the runtime snapshot."],
      next_actions: ["Use provider-gateway-worker with real upstream services."],
    },
  ],
  source_resources: ["pool://runtime-graph", "pool://prd-readiness"],
};

const prdCompletionGate = {
  kind: "pool_prd_completion_gate",
  project_filter: "demo",
  overall_status: "partial",
  summary: prdReadiness.summary,
  completion_gate: {
    ...prdReadiness.completion_gate,
    proof_commands: {
      ...prdReadiness.completion_gate.proof_commands,
      closeout_preflight: "pool-cli --project demo closeout-production-evidence --output gate-merged.json provider.json software.json desktop.json",
    },
  },
};

const productionEvidenceRequirements = {
  kind: "pool_production_evidence_requirements",
  version: 1,
  project_filter: "demo",
  generated_at: "2026-06-11T00:00:00Z",
  overall_status: "partial",
  summary: {
    complete: false,
    missing_total: 21,
    provider_gateway_ready: true,
    provider_production_ready: false,
    software_control_ready: true,
    software_production_ready: false,
    desktop_vision_ready: false,
    missing_provider_gateway_profile_success: [],
    missing_provider_production_upstream_success: [
      "midjourney",
      "openai-image-2",
      "nano-banana-pro",
      "suno",
      "worldlabs-marble",
      "tripo-splat",
      "sam-3d",
      "spark-3dgs",
      "qunhe-3d",
    ],
    missing_software_control_profile_success: [],
    missing_software_production_success: [
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
    ],
    missing_desktop_vision: ["external_visual_model"],
  },
  commands: {
    template: "pool-cli --project demo production-evidence-template --output-root target/prod bundle.json",
    merge: "pool-cli --project demo merge-production-evidence <combined-bundle.json> <bundle-a.json> <bundle-b.json>...",
    closeout: "pool-cli --project demo closeout-production-evidence --output <merged-bundle.json> <bundle-a.json> <bundle-b.json>...",
    validate: "pool-cli --project demo validate-production-evidence <bundle.json>",
    import: "pool-cli --project demo import-production-evidence <bundle.json>",
    readiness: "pool-cli --project demo prd-readiness",
  },
  evidence_tasks: {
    summary: {
      total: 21,
      provider_tasks: 9,
      software_tasks: 11,
      desktop_vision_tasks: 1,
    },
    tasks: [
      {
        id: "provider:midjourney:production_upstream",
        kind: "provider_production_upstream",
        target_id: "midjourney",
        status: "missing",
        title: "Record real upstream production evidence for midjourney",
        bundle_path: "providers[]",
        commands: {
          merge: "pool-cli --project demo merge-production-evidence <combined-bundle.json> <bundle-a.json> <bundle-b.json>...",
          validate: "pool-cli --project demo validate-production-evidence <bundle.json>",
        },
      },
      {
        id: "software:unreal:production_software",
        kind: "software_production",
        target_id: "unreal",
        status: "missing",
        title: "Record real software execution evidence for unreal",
        bundle_path: "software_actions[]",
        commands: {
          merge: "pool-cli --project demo merge-production-evidence <combined-bundle.json> <bundle-a.json> <bundle-b.json>...",
          validate: "pool-cli --project demo validate-production-evidence <bundle.json>",
        },
      },
      {
        id: "software:resolve:production_software",
        kind: "software_production",
        target_id: "resolve",
        status: "missing",
        title: "Record real software execution evidence for resolve",
        bundle_path: "software_actions[]",
        preferred_control_profile: "api_mcp",
        bridge_worker: {
          available: true,
          adapter_id: "resolve",
          endpoint_env: "POOL_RESOLVE_ENDPOINT",
          endpoint_env_template: "POOL_RESOLVE_ENDPOINT=http://127.0.0.1:<port>",
          cli_template: "pool-cli software-api-bridge-worker resolve --bind 127.0.0.1:<port> --output-root worlds/demo/output --upstream <real-plugin-or-gateway-url>",
          upstream_required: true,
          production_rule: "The local worker is valid production evidence only when --upstream forwards to a real software plugin, API, MCP service, or gateway.",
        },
        commands: {
          merge: "pool-cli --project demo merge-production-evidence <combined-bundle.json> <bundle-a.json> <bundle-b.json>...",
          validate: "pool-cli --project demo validate-production-evidence <bundle.json>",
        },
      },
      {
        id: "desktop_vision:external_visual_model",
        kind: "desktop_vision",
        target_id: "external_visual_model",
        status: "missing",
        title: "Attach real external visual model evidence for desktop recognition",
        bundle_path: "desktop_vision[]",
        commands: {
          merge: "pool-cli --project demo merge-production-evidence <combined-bundle.json> <bundle-a.json> <bundle-b.json>...",
          validate: "pool-cli --project demo validate-production-evidence <bundle.json>",
        },
      },
    ],
  },
};

const outputPackages = {
  kind: "pool_output_packages",
  project_filter: "demo",
  generated_at: "2026-06-11T00:00:00Z",
  summary: {
    total_targets: 3,
    indexed_targets: 3,
    ready_targets: 3,
    missing_targets: [],
    local_file_failures: [],
    latest_asset_at: "2026-06-11T00:00:00Z",
  },
  deliverables: [
    {
      target: "video",
      title: "时间线与转码",
      expected_file: "1-video-timeline.json",
      primary_runtime: "DaVinci Resolve / FFmpeg",
      status: "ready",
      local_path: "worlds/demo/output/deliverables/1-video-timeline.json",
      asset_id: "asset-video",
      file_found: true,
      manifest_found: true,
      metrics: [
        { label: "duration", value: "12.0s" },
        { label: "transcode", value: "mp4 h264 1920x1080" },
      ],
      control_routes: ["resolve", "editing-software", "ffmpeg-cli"],
    },
    {
      target: "game",
      title: "运行原型",
      expected_file: "2-game-build.json",
      primary_runtime: "Unreal",
      status: "ready",
      local_path: "worlds/demo/output/deliverables/2-game-build.json",
      asset_id: "asset-game",
      file_found: true,
      manifest_found: true,
      metrics: [
        { label: "level", value: "demo_content_burst" },
        { label: "viewport", value: "play_in_editor" },
      ],
      control_routes: ["unreal", "unity-future"],
    },
    {
      target: "interactive_art",
      title: "节点与现场控制",
      expected_file: "3-interactive-cues.json",
      primary_runtime: "TouchDesigner / MadMapper",
      status: "ready",
      local_path: "worlds/demo/output/deliverables/3-interactive-cues.json",
      asset_id: "asset-interactive",
      file_found: true,
      manifest_found: true,
      metrics: [
        { label: "interfaces", value: "osc, midi, dmx" },
        { label: "routes", value: "osc:/pool/cue/1, dmx:universe-1" },
      ],
      control_routes: ["touchdesigner", "madmapper", "osc", "midi", "dmx"],
    },
  ],
  policy: {
    local_files_authoritative: true,
    provider_urls_are_provenance: true,
    expected_targets: ["video", "game", "interactive_art"],
  },
};

const outputPackagesWithGameResult = {
  ...outputPackages,
  deliverables: outputPackages.deliverables.map((deliverable) =>
    deliverable.target === "game"
      ? {
          ...deliverable,
          metrics: [
            ...deliverable.metrics,
            { label: "execution", value: "succeeded" },
            { label: "runtime_result", value: "Unreal" },
            { label: "adapter", value: "unreal" },
            { label: "artifacts", value: "1" },
            { label: "message", value: "运行原型 后段执行完成" },
          ],
        }
      : deliverable,
  ),
};

let desktopQueueOpen = true;

const desktopRecognitionRequest = {
  software_action_id: "action-desktop",
  task_id: "task-desktop",
  adapter_id: "touchdesigner",
  action_kind: "RunViewport",
  status: "queued_for_desktop_recognition",
  desktop_request_path: "worlds/demo/output/control/desktop-recognition/1-touchdesigner.json",
  request_file_available: true,
  pool_desktop_action: {
    operation: "run_preview",
    desktop_tool: "desktop.run_preview",
    target_window: "TouchDesigner",
  },
  desktop_payload: {
    tool: "desktop.run_preview",
    operation: "run_preview",
    target_window: "TouchDesigner",
  },
  command: {
    payload_json: {
      instruction: "trigger cue 1",
      target_window: "TouchDesigner",
    },
  },
  verification: {},
  created_at: "2026-06-11T00:00:00Z",
};

function productionEvidenceRequestHasTemplateIdentifiers(body) {
  const desktopVisionItems = Array.isArray(body.desktop_vision) ? body.desktop_vision : [];
  const desktopVisionItem = body.desktop_vision && !Array.isArray(body.desktop_vision)
    ? body.desktop_vision
    : null;
  const identifiers = [
    ...(body.providers ?? []).map((provider) => provider.external_job_id),
    ...(body.software_actions ?? []).map((action) => action.external_action_id),
    ...desktopVisionItems.map((vision) => vision.external_action_id),
    ...desktopVisionItems.map((vision) => vision.controller_id),
    ...(body.provider ? [body.provider.external_job_id] : []),
    ...(body.software_action ? [body.software_action.external_action_id] : []),
    ...(desktopVisionItem ? [desktopVisionItem.external_action_id, desktopVisionItem.controller_id] : []),
  ];
  return identifiers
    .filter(Boolean)
    .some((identifier) => /replace-with|placeholder|todo|dummy|fake|sample-|example-|template-|web-prod/i.test(identifier));
}

function productionEvidenceTemplateBundle() {
  const projectSlug = "demo";
  const providers = [
    ["midjourney", "ai_media", "1-midjourney.png"],
    ["openai-image-2", "ai_image", "1-openai-image.png"],
    ["nano-banana-pro", "ai_media", "1-nano.png"],
    ["suno", "ai_media", "1-cue.mp3"],
    ["worldlabs-marble", "3dgs", "1-world.glb"],
    ["tripo-splat", "3dgs", "1-object.glb"],
    ["sam-3d", "3dgs", "1-mask-object.glb"],
    ["spark-3dgs", "3dgs", "1-scene.glb"],
    ["qunhe-3d", "3dgs", "1-layout.glb"],
  ].map(([providerId, family, fileName]) => ({
    provider_id: providerId,
    external_job_id: `replace-with-real-${providerId}-job-001`,
    endpoint: `https://worker.example.test/${providerId}`,
    family,
    metadata_path: `worlds/${projectSlug}/output/production/${providerId}/request-metadata.json`,
    artifacts: [`worlds/${projectSlug}/output/production/${providerId}/${fileName}`],
  }));
  const softwareActions = [
    ["unreal", "CreateScene", "ApiMcp", "api_mcp", `unreal://project/${projectSlug}/level/production`],
    ["blender", "ExecuteCli", "SkillsCli", "skills_cli", `worlds/${projectSlug}/output/production/blender/1-cleanup.blend`],
    ["comfyui", "ExecuteCli", "SkillsCli", "skills_cli", `worlds/${projectSlug}/output/production/comfyui/1-image.png`],
    ["resolve", "Transcode", "SkillsCli", "skills_cli", `worlds/${projectSlug}/output/production/resolve/1-master.mov`],
    ["unity", "ExportBuild", "ApiMcp", "api_mcp", `unity://project/${projectSlug}/build/production`],
    ["touchdesigner", "RunViewport", "DesktopRecognition", "desktop_recognition", `touchdesigner://project/${projectSlug}/perform`],
    ["madmapper", "RunViewport", "DesktopRecognition", "desktop_recognition", `madmapper://project/${projectSlug}/cues`],
    ["nuke", "Render", "SkillsCli", "skills_cli", `worlds/${projectSlug}/output/production/nuke/1-comp.exr`],
    ["motion-db", "ImportAsset", "SkillsCli", "skills_cli", `worlds/${projectSlug}/output/production/motion-db/1-take.fbx`],
    ["editing-suite", "Transcode", "SkillsCli", "skills_cli", `worlds/${projectSlug}/output/production/editing-suite/1-delivery.mp4`],
    ["hermes", "CreateScene", "ApiMcp", "api_mcp", "pool://agent-sessions/replace-with-real-hermes-session-id"],
  ].map(([adapterId, actionKind, priority, controlProfile, artifact]) => ({
    adapter_id: adapterId,
    external_action_id: `replace-with-real-${adapterId}-action-001`,
    action_kind: actionKind,
    priority,
    control_profile: controlProfile,
    artifacts: [artifact],
  }));
  return {
    project_slug: projectSlug,
    source: "runtime-production-evidence-template",
    providers,
    software_actions: softwareActions,
    desktop_vision: [
      {
        adapter_id: "touchdesigner",
        external_action_id: "replace-with-real-vision-action-001",
        controller_id: "replace-with-real-vision-controller-id",
        trace_path: `worlds/${projectSlug}/output/production/desktop-vision/1-touchdesigner-trace.json`,
        visual_model: "external",
        artifacts: [`worlds/${projectSlug}/output/production/desktop-vision/1-touchdesigner-trace.json`],
      },
    ],
  };
}

function productionEvidenceTasksResponse() {
  return {
    kind: "pool_production_evidence_tasks",
    version: 1,
    project_filter: "demo",
    generated_at: "2026-06-11T00:00:00Z",
    overall_status: "partial",
    summary: productionEvidenceRequirements.evidence_tasks.summary,
    commands: {
      item_template: "pool-cli --project demo production-evidence-item-template <kind> <target-id> <item.json>",
      submit_item: "pool-cli --project demo submit-production-evidence-item <item.json>",
      readiness: "pool-cli --project demo prd-readiness",
    },
    tasks: productionEvidenceRequirements.evidence_tasks.tasks.map((task) => ({
      ...task,
      required_fields: task.kind === "provider_production_upstream"
        ? ["provider_id", "external_job_id", "metadata_path", "artifacts"]
        : task.kind === "software_production"
          ? ["adapter_id", "external_action_id", "action_kind", "artifacts"]
          : ["controller_id", "trace_path", "visual_model", "artifacts"],
      artifact_policy: "local_files_required_provider_urls_are_provenance",
      commands: {
        ...task.commands,
        item_template: `pool-cli --project demo production-evidence-item-template --task-id ${task.id} <item.json>`,
        submit_item: "pool-cli --project demo submit-production-evidence-item <item.json>",
      },
    })),
  };
}

function productionEvidenceRunPlanResponse() {
  return {
    kind: "pool_production_evidence_run_plan",
    version: 1,
    project_slug: "demo",
    generated_at: "2026-06-11T00:00:00Z",
    source: "web-production-evidence-run-plan",
    status: "needs_real_production_evidence",
    ready_for_completion: false,
    output_root: "worlds/demo/output/production-evidence",
    summary: {
      missing_total: 21,
      provider_tasks: 9,
      software_tasks: 11,
      desktop_vision_tasks: 1,
      ready: 7,
      partial: 3,
      blocked: 0,
    },
    paths: {
      provider_bundle: "worlds/demo/output/production-evidence/provider-production-evidence-bundle.json",
      software_bundle: "worlds/demo/output/production-evidence/software-production-evidence-bundle.json",
      desktop_vision_bundle: "worlds/demo/output/production-evidence/desktop-vision-production-evidence-bundle.json",
      combined_bundle: "worlds/demo/output/production-evidence/combined-production-evidence-bundle.json",
    },
    phases: [
      {
        id: "provider_evidence_matrix",
        status: "pending_external_run",
        command: "pool-cli --project demo production-evidence-provider-matrix worlds/demo/output/production-evidence/provider-evidence-matrix --production-upstream --evidence-bundle=worlds/demo/output/production-evidence/provider-production-evidence-bundle.json",
        ready_condition: "Provider bundle contains production_upstream:true evidence.",
        provider_gateway_worker_start_commands: [
          {
            family: "3dgs",
            endpoint_env: "POOL_3DGS_GATEWAY_ENDPOINT",
            upstream_env: "POOL_3DGS_GATEWAY_UPSTREAM_ENDPOINT",
            cli: "pool-cli provider-gateway-worker --bind 127.0.0.1:<port> --upstream $POOL_3DGS_GATEWAY_UPSTREAM_ENDPOINT --api-key-env POOL_3DGS_GATEWAY_API_KEY",
          },
        ],
      },
      {
        id: "software_evidence_matrix",
        status: "pending_external_run",
        command: "pool-cli --project demo production-evidence-software-matrix worlds/demo/output/production-evidence/software-evidence-matrix --production-software --evidence-bundle=worlds/demo/output/production-evidence/software-production-evidence-bundle.json",
        ready_condition: "Software bundle contains production_software:true evidence.",
        generic_api_bridge_worker: {
          applies_to: ["blender", "comfyui", "resolve", "unity", "nuke", "motion-db", "editing-suite"],
          cli_template: "pool-cli software-api-bridge-worker <adapter-id> --bind 127.0.0.1:<port> --output-root worlds/demo/output --upstream <real-plugin-or-gateway-url>",
          endpoint_env_template: "POOL_<ADAPTER>_ENDPOINT=http://127.0.0.1:<port>",
          operator_note: "Use only as an audit/forwarder to a real software plugin or gateway.",
        },
        bridge_worker_start_commands: [
          {
            adapter_id: "resolve",
            endpoint_env: "POOL_RESOLVE_ENDPOINT",
            upstream_env: "POOL_RESOLVE_UPSTREAM_ENDPOINT",
            cli: "pool-cli software-api-bridge-worker resolve --bind 127.0.0.1:<port> --output-root worlds/demo/output/production-evidence --upstream $POOL_RESOLVE_UPSTREAM_ENDPOINT",
          },
        ],
      },
      {
        id: "desktop_vision_evidence",
        status: "pending_external_run",
        command: "pool-cli --project demo production-evidence-desktop-vision worlds/demo/output/production-evidence/desktop-vision --production-vision --trace=<real-vision-trace> --external-action-id=<real-vision-action-id> --evidence-bundle=worlds/demo/output/production-evidence/desktop-vision-production-evidence-bundle.json",
        ready_condition: "Desktop evidence contains external_visual_model:true trace.",
      },
      {
        id: "merge_bundles",
        status: "pending_inputs",
        command: "pool-cli --project demo merge-production-evidence worlds/demo/output/production-evidence/combined-production-evidence-bundle.json worlds/demo/output/production-evidence/provider-production-evidence-bundle.json worlds/demo/output/production-evidence/software-production-evidence-bundle.json worlds/demo/output/production-evidence/desktop-vision-production-evidence-bundle.json",
        ready_condition: "Merged bundle has provider/software/desktop evidence.",
      },
      {
        id: "closeout_preflight",
        status: "pending_inputs",
        command: "pool-cli --project demo closeout-production-evidence --output worlds/demo/output/production-evidence/combined-production-evidence-bundle.json worlds/demo/output/production-evidence/combined-production-evidence-bundle.json",
        ready_condition: "Closeout preflight returns ready_for_import:true.",
      },
      {
        id: "closeout_import",
        status: "pending_preflight",
        command: "pool-cli --project demo closeout-production-evidence --import worlds/demo/output/production-evidence/combined-production-evidence-bundle.json",
        ready_condition: "Import returns ready_for_completion:true.",
      },
      {
        id: "completion_proof",
        status: "pending_closeout_import",
        command: "pool-cli --project demo prd-completion-gate --require-complete && pool-cli --project demo prd-completion-package --output-dir worlds/demo/output/control/prd-completion --include-snapshot",
        ready_condition: "Completion gate succeeds.",
      },
    ],
    commands: {
      run_plan: "pool-cli --project demo production-evidence-run-plan <run-plan.json>",
      closeout_preflight: "pool-cli --project demo closeout-production-evidence --output worlds/demo/output/production-evidence/combined-production-evidence-bundle.json worlds/demo/output/production-evidence/provider-production-evidence-bundle.json worlds/demo/output/production-evidence/software-production-evidence-bundle.json worlds/demo/output/production-evidence/desktop-vision-production-evidence-bundle.json",
      closeout_import: "pool-cli --project demo closeout-production-evidence --import worlds/demo/output/production-evidence/combined-production-evidence-bundle.json",
      completion_gate: "pool-cli --project demo prd-completion-gate --require-complete",
    },
  };
}

function productionEvidenceItemTemplateResponse(taskId) {
  if (taskId !== "provider:midjourney:production_upstream") {
    throw new Error(`unexpected production evidence item template task_id: ${taskId}`);
  }
  const item = {
    project_slug: "demo",
    source: "runtime-production-evidence-item-template",
    kind: "provider",
    provider: {
      provider_id: "midjourney",
      external_job_id: "replace-with-real-midjourney-job-id",
      endpoint: "https://worker.example.com/midjourney",
      family: "ai_media",
      metadata_path: "worlds/demo/output/production/midjourney/request-metadata.json",
      artifacts: ["worlds/demo/output/production/midjourney/1-midjourney.png"],
      evidence_json: {
        source: "runtime-production-evidence-item-template",
        task_id: taskId,
        production_upstream: true,
        local_mock_gateway: false,
      },
    },
  };
  return {
    kind: "pool_production_evidence_item_template",
    version: 1,
    project_slug: "demo",
    ready_for_import: false,
    selector: {
      task_id: taskId,
      kind: "provider",
      target_id: "midjourney",
    },
    output_root: ".",
    item,
    commands: {
      submit: "pool-cli --project demo submit-production-evidence-item <item.json>",
      tasks: "pool-cli --project demo production-evidence-tasks",
      readiness: "pool-cli --project demo prd-readiness",
    },
  };
}

function payloadForUrl(rawUrl, options = {}) {
  const url = new URL(rawUrl);
  if (url.pathname === "/api/health") return { status: "ready", project_filter: "demo", stats: runtimeSnapshotPayload().stats };
  if (url.pathname === "/api/snapshot") return runtimeSnapshotPayload();
  if (url.pathname === "/api/runtime-graph") return runtimeGraph;
  if (url.pathname === "/api/runtime-execution-plan") return runtimeExecutionPlan;
  if (url.pathname === "/api/runtime-execution-plan/run-next") {
    const body = JSON.parse(options.body ?? "{}");
    if (body.project_slug !== "demo" || body.execute !== false) {
      throw new Error(`unexpected runtime execution plan run-next body: ${options.body}`);
    }
    const selectedStep = runtimeExecutionPlan.next_steps[0];
    return {
      mode: "preview",
      executed: false,
      project_slug: "demo",
      selected_step: selectedStep,
      action: selectedStep.control.recommended_action,
      message: "set execute:true to dispatch this runtime execution plan step",
    };
  }
  if (url.pathname === "/api/runtime-budget") return runtimeBudget;
  if (url.pathname === "/api/api-keys") return runtimeApiKeys;
  if (url.pathname === "/api/runtime-preflight") return runtimePreflight;
  if (url.pathname === "/api/runtime-handoff") return runtimeHandoff;
  if (url.pathname === "/api/prd-readiness") return prdReadiness;
  if (url.pathname === "/api/prd-completion-gate") return prdCompletionGate;
  if (url.pathname === "/api/integration-readiness") return integrationReadiness;
  if (url.pathname === "/api/prd-completion-package") {
    const body = JSON.parse(options.body ?? "{}");
    if (
      body.project_slug !== "demo" ||
      body.node_id !== "agent" ||
      body.output_dir !== "worlds/demo/output" ||
      body.source !== "web-prd-completion-package" ||
      body.include_snapshot !== true
    ) {
      throw new Error(`unexpected PRD completion package body: ${options.body}`);
    }
    return {
      kind: "pool_prd_completion_package",
      report: {
        status: "Succeeded",
        package_dir: "worlds/demo/output/control/prd-completion",
        readiness_path: "worlds/demo/output/control/prd-completion/1-prd-readiness.json",
        completion_gate_path: "worlds/demo/output/control/prd-completion/2-prd-completion-gate.json",
        production_evidence_requirements_path: "worlds/demo/output/control/prd-completion/3-production-evidence-requirements.json",
        manifest_path: "worlds/demo/output/control/prd-completion/4-prd-completion-package-manifest.json",
        snapshot_path: "worlds/demo/output/control/prd-completion/5-runtime-snapshot.json",
        ready_for_completion: false,
        completion_status: "incomplete",
        local_paths: [
          "worlds/demo/output/control/prd-completion/.1-prd-completion-package-request.json",
          "worlds/demo/output/control/prd-completion/1-prd-readiness.json",
          "worlds/demo/output/control/prd-completion/2-prd-completion-gate.json",
          "worlds/demo/output/control/prd-completion/3-production-evidence-requirements.json",
          "worlds/demo/output/control/prd-completion/5-runtime-snapshot.json",
          "worlds/demo/output/control/prd-completion/4-prd-completion-package-manifest.json",
        ],
      },
      task: {
        id: "task-prd-completion-package",
        status: "Succeeded",
      },
      assets: [
        { id: "asset-prd-readiness" },
        { id: "asset-prd-gate" },
        { id: "asset-prd-requirements" },
        { id: "asset-prd-snapshot" },
        { id: "asset-prd-manifest" },
      ],
      snapshot: runtimeSnapshotPayload(),
    };
  }
  if (url.pathname === "/api/production-evidence/requirements") return productionEvidenceRequirements;
  if (url.pathname === "/api/production-evidence/tasks") return productionEvidenceTasksResponse();
  if (url.pathname === "/api/production-evidence/run-plan") {
    if (
      url.searchParams.get("source") !== "web-production-evidence-run-plan" ||
      url.searchParams.get("output_root") !== "worlds/demo/output/production-evidence"
    ) {
      throw new Error(`unexpected production evidence run plan query: ${url.search}`);
    }
    return productionEvidenceRunPlanResponse();
  }
  if (url.pathname === "/api/production-evidence/handoff") {
    return {
      kind: "pool_production_evidence_handoff",
      project_slug: "demo",
      overall_status: "partial",
      output_root: "worlds/demo/output/production-evidence",
      summary: {
        missing_total: 21,
        evidence_tasks: 21,
        provider_tasks: 9,
        software_tasks: 11,
        desktop_vision_tasks: 1,
      },
      bundle: productionEvidenceTemplateBundle(),
      provider_gateway_worker_start_commands: [
        {
          family: "ai_media",
          endpoint_env: "POOL_MEDIA_GATEWAY_ENDPOINT",
          upstream_env: "POOL_MEDIA_GATEWAY_UPSTREAM_ENDPOINT",
          cli: "pool-cli provider-gateway-worker --bind 127.0.0.1:<port> --upstream $POOL_MEDIA_GATEWAY_UPSTREAM_ENDPOINT --api-key-env POOL_MEDIA_GATEWAY_API_KEY",
        },
      ],
      software_bridge_worker_start_commands: [
        {
          adapter_id: "resolve",
          endpoint_env: "POOL_RESOLVE_ENDPOINT",
          upstream_env: "POOL_RESOLVE_UPSTREAM_ENDPOINT",
          cli: "pool-cli software-api-bridge-worker resolve --bind 127.0.0.1:<port> --output-root worlds/demo/output/production-evidence --upstream $POOL_RESOLVE_UPSTREAM_ENDPOINT",
        },
      ],
      commands: {
        merge: "pool-cli --project demo merge-production-evidence <combined-bundle.json> <bundle-a.json> <bundle-b.json>...",
        closeout: "pool-cli --project demo closeout-production-evidence --output <merged-bundle.json> <bundle-a.json> <bundle-b.json>...",
        validate: "pool-cli --project demo validate-production-evidence <bundle.json>",
        import: "pool-cli --project demo import-production-evidence <bundle.json>",
      },
    };
  }
  if (url.pathname === "/api/production-evidence/item-template") {
    return productionEvidenceItemTemplateResponse(url.searchParams.get("task_id"));
  }
  if (url.pathname === "/api/production-evidence/tasks/claim") {
    const body = JSON.parse(options.body ?? "{}");
    if (
      body.project_slug !== "demo" ||
      body.task_id !== "provider:midjourney:production_upstream" ||
      body.assignee !== "web-operator" ||
      body.role !== "provider_worker" ||
      body.output_root !== "worlds/demo/output/production-evidence" ||
      body.source !== "web-production-evidence-task-claim"
    ) {
      throw new Error(`unexpected production evidence task claim body: ${options.body}`);
    }
    return {
      kind: "pool_production_evidence_task_claim",
      project_slug: "demo",
      task_id: body.task_id,
      runtime_task: {
        id: "task-production-evidence-claim-midjourney",
        project_slug: "demo",
        node_id: "agent",
        title: "Claim production evidence task",
        status: "Running",
        provider_id: "midjourney",
        request_metadata_path:
          "worlds/demo/output/production-evidence/control/claims/provider-midjourney-production-upstream-claim.json",
      },
      claim_path: "worlds/demo/output/production-evidence/control/claims/provider-midjourney-production-upstream-claim.json",
      claim: {
        kind: "pool_production_evidence_task_claim",
        project_slug: "demo",
        task_id: body.task_id,
        runtime_task_id: "task-production-evidence-claim-midjourney",
        assignee: body.assignee,
        role: body.role,
        source: body.source,
        output_root: body.output_root,
        selector: {
          kind: "provider",
          target_id: "midjourney",
        },
        item_template: productionEvidenceItemTemplateResponse(body.task_id).item,
        commands: {
          validate_item: "pool-cli --project demo validate-production-evidence-item <item.json>",
          submit_item: "pool-cli --project demo submit-production-evidence-item <item.json>",
          readiness: "pool-cli --project demo prd-readiness",
        },
      },
      snapshot: runtimeSnapshotPayload(),
    };
  }
  if (url.pathname === "/api/production-evidence/handoff-packages") {
    const body = JSON.parse(options.body ?? "{}");
    if (
      body.project_slug !== "demo" ||
      body.node_id !== "agent" ||
      body.output_dir !== "worlds/demo/output" ||
      body.output_root !== "worlds/demo/output/production-evidence" ||
      body.source !== "web-production-evidence-handoff-package" ||
      body.include_items !== true ||
      body.include_snapshot !== true
    ) {
      throw new Error(`unexpected production evidence handoff package body: ${options.body}`);
    }
    return {
      kind: "pool_production_evidence_handoff_package",
      report: {
        status: "Succeeded",
        project_slug: "demo",
        node_id: "agent",
        title: "Production evidence handoff package",
        package_dir: "worlds/demo/output/control/production-evidence",
        manifest_path: "worlds/demo/output/control/production-evidence/6-production-evidence-package-manifest.json",
        run_plan_path: "worlds/demo/output/control/production-evidence/4-production-evidence-run-plan.json",
        runner_script_path: "worlds/demo/output/control/production-evidence/7-production-evidence-runner.sh",
        runner_preflight_path: "worlds/demo/output/control/production-evidence/8-production-evidence-runner-preflight.json",
        bundle_path: "worlds/demo/output/control/production-evidence/5-production-evidence-bundle.json",
        tasks_path: "worlds/demo/output/control/production-evidence/2-production-evidence-tasks.json",
        item_count: 21,
        provider_gateway_worker_start_commands: [
          {
            family: "3dgs",
            endpoint_env: "POOL_3DGS_GATEWAY_ENDPOINT",
            endpoint_assignment: "POOL_3DGS_GATEWAY_ENDPOINT=http://127.0.0.1:<port>",
            upstream_env: "POOL_3DGS_GATEWAY_UPSTREAM_ENDPOINT",
            cli: "pool-cli provider-gateway-worker --bind 127.0.0.1:<port> --upstream $POOL_3DGS_GATEWAY_UPSTREAM_ENDPOINT --api-key-env POOL_3DGS_GATEWAY_API_KEY",
            production_rule: "The worker is production-valid only when --upstream routes to a real 3DGS vendor worker.",
          },
        ],
        software_bridge_worker_start_commands: [
          {
            adapter_id: "resolve",
            endpoint_env: "POOL_RESOLVE_ENDPOINT",
            endpoint_assignment: "POOL_RESOLVE_ENDPOINT=http://127.0.0.1:<port>",
            upstream_env: "POOL_RESOLVE_UPSTREAM_ENDPOINT",
            cli: "pool-cli software-api-bridge-worker resolve --bind 127.0.0.1:<port> --output-root worlds/demo/output/production-evidence --upstream $POOL_RESOLVE_UPSTREAM_ENDPOINT",
            production_rule: "The bridge worker is production-valid only when --upstream points to a real software plugin, API, MCP service, or gateway.",
          },
        ],
        items: [
          {
            task_id: "provider:midjourney:production_upstream",
            kind: "provider",
            target_id: "midjourney",
            bundle_path: "providers[]",
            item_path: "worlds/demo/output/control/production-evidence/items/1-provider-midjourney-item.json",
          },
          {
            task_id: "software:resolve:production_software",
            kind: "software_action",
            target_id: "resolve",
            bundle_path: "software_actions[]",
            preferred_control_profile: "api_mcp",
            item_path: "worlds/demo/output/control/production-evidence/items/8-software_action-resolve-item.json",
            bridge_worker: {
              available: true,
              endpoint_env: "POOL_RESOLVE_ENDPOINT",
              cli_template: "pool-cli software-api-bridge-worker resolve --bind 127.0.0.1:<port> --output-root worlds/demo/output --upstream <real-plugin-or-gateway-url>",
            },
          },
        ],
        local_paths: [
          "worlds/demo/output/control/production-evidence/.1-production-evidence-handoff-package-request.json",
          "worlds/demo/output/control/production-evidence/1-production-evidence-requirements.json",
          "worlds/demo/output/control/production-evidence/2-production-evidence-tasks.json",
          "worlds/demo/output/control/production-evidence/3-production-evidence-handoff.json",
          "worlds/demo/output/control/production-evidence/4-production-evidence-run-plan.json",
          "worlds/demo/output/control/production-evidence/5-production-evidence-bundle.json",
          "worlds/demo/output/control/production-evidence/6-production-evidence-package-manifest.json",
          "worlds/demo/output/control/production-evidence/7-production-evidence-runner.sh",
          "worlds/demo/output/control/production-evidence/8-production-evidence-runner-preflight.json",
        ],
      },
      task: {
        id: "task-production-evidence-handoff-package",
        project_slug: "demo",
        node_id: "agent",
        title: "Production evidence handoff package",
        status: "Succeeded",
        provider_id: "production-evidence-handoff-package",
      },
      snapshot: runtimeSnapshotPayload(),
    };
  }
  if (url.pathname === "/api/production-evidence/template") {
    const bundle = productionEvidenceTemplateBundle();
    return {
      kind: "pool_production_evidence_bundle_template",
      version: 1,
      project_slug: "demo",
      ready_for_import: false,
      bundle,
      artifact_plan: {
        providers: bundle.providers.map((provider) => ({
          provider_id: provider.provider_id,
          artifact_path: provider.artifacts[0],
          metadata_path: provider.metadata_path,
          family: provider.family,
        })),
        desktop_vision: [
          {
            adapter_id: "touchdesigner",
            trace_path: bundle.desktop_vision[0].trace_path,
          },
        ],
      },
      operator_checklist: ["replace ids", "download local files", "validate before import"],
      commands: {
        validate: "pool-cli --project demo validate-production-evidence <bundle.json>",
        import: "pool-cli --project demo import-production-evidence <bundle.json>",
      },
    };
  }
  if (url.pathname === "/api/production-evidence/bundle-from-ledger") {
    if (
      url.searchParams.get("source") !== "web-production-evidence-ledger" ||
      url.searchParams.get("include_incomplete") !== "true"
    ) {
      throw new Error(`unexpected production evidence ledger bundle query: ${url.search}`);
    }
    const bundle = JSON.parse(
      JSON.stringify(productionEvidenceTemplateBundle()).replaceAll("replace-with-real-", "real-"),
    );
    bundle.source = "web-production-evidence-ledger";
    return {
      kind: "pool_production_evidence_bundle_from_ledger",
      version: 1,
      project_slug: "demo",
      source: "web-production-evidence-ledger",
      ready_for_import: true,
      include_incomplete: true,
      summary: {
        providers: bundle.providers.length,
        software_actions: bundle.software_actions.length,
        desktop_vision: bundle.desktop_vision.length,
        ready_items: bundle.providers.length + bundle.software_actions.length + bundle.desktop_vision.length,
        incomplete_items: 2,
        ledger_candidates: bundle.providers.length + bundle.software_actions.length + bundle.desktop_vision.length + 2,
      },
      bundle,
      items: [],
      incomplete_items: [
        { ledger: { kind: "provider_request", id: "mock-provider-request" }, ready_for_import: false },
        { ledger: { kind: "desktop_vision_action", id: "dry-run-desktop" }, ready_for_import: false },
      ],
      validation: {
        valid: true,
        artifact_files: { complete: true },
      },
      commands: {
        validate: "pool-cli --project demo validate-production-evidence <bundle.json>",
        import: "pool-cli --project demo import-production-evidence <bundle.json>",
        closeout: "pool-cli --project demo closeout-production-evidence --import <bundle.json>",
      },
    };
  }
  if (url.pathname === "/api/production-evidence/items/validate") {
    const body = JSON.parse(options.body ?? "{}");
    if (productionEvidenceRequestHasTemplateIdentifiers(body)) {
      throw new Error("production evidence template identifiers are not accepted");
    }
    if (
      body.project_slug !== "demo" ||
      body.kind !== "provider" ||
      body.provider?.provider_id !== "midjourney" ||
      body.provider?.external_job_id !== "real-midjourney-job-id" ||
      body.provider?.metadata_path !== "worlds/demo/output/production/midjourney/request-metadata.json" ||
      body.provider?.artifacts?.[0] !== "worlds/demo/output/production/midjourney/1-midjourney.png"
    ) {
      throw new Error(`unexpected production evidence item validation body: ${options.body}`);
    }
    return {
      kind: "pool_production_evidence_item_validation",
      project_slug: "demo",
      source: body.source ?? "runtime-production-evidence-item-template",
      valid: true,
      writes: 0,
      validation: {
        kind: "pool_production_evidence_validation",
        project_slug: "demo",
        source: body.source ?? "runtime-production-evidence-item-template",
        valid: true,
        writes: 0,
        summary: {
          providers: 1,
          software_actions: 0,
          desktop_vision: 0,
        },
        coverage: {
          complete: false,
          would_satisfy_prd_production_evidence: false,
          providers: {
            required: ["midjourney", "openai-image-2", "nano-banana-pro", "suno", "worldlabs-marble", "tripo-splat", "sam-3d", "spark-3dgs", "qunhe-3d"],
            provided: ["midjourney"],
            covered: 1,
            missing: ["openai-image-2", "nano-banana-pro", "suno", "worldlabs-marble", "tripo-splat", "sam-3d", "spark-3dgs", "qunhe-3d"],
            complete: false,
          },
          software_actions: {
            required: ["unreal", "blender", "comfyui", "resolve", "unity", "touchdesigner", "madmapper", "nuke", "motion-db", "editing-suite", "hermes"],
            provided: [],
            covered: 0,
            missing: ["unreal", "blender", "comfyui", "resolve", "unity", "touchdesigner", "madmapper", "nuke", "motion-db", "editing-suite", "hermes"],
            complete: false,
          },
          desktop_vision: {
            required: ["external_visual_model"],
            provided: 0,
            external_visual_model: false,
            missing: ["external_visual_model"],
            complete: false,
          },
        },
      },
      commands: {
        submit: "pool-cli --project demo submit-production-evidence-item <item.json>",
        validate_bundle: "pool-cli --project demo validate-production-evidence <bundle.json>",
        readiness: "pool-cli --project demo prd-readiness",
      },
    };
  }
  if (url.pathname === "/api/production-evidence/items") {
    const body = JSON.parse(options.body ?? "{}");
    if (productionEvidenceRequestHasTemplateIdentifiers(body)) {
      throw new Error("production evidence template identifiers are not accepted");
    }
    if (
      body.project_slug !== "demo" ||
      body.kind !== "provider" ||
      body.provider?.provider_id !== "midjourney" ||
      body.provider?.external_job_id !== "real-midjourney-job-id" ||
      body.provider?.metadata_path !== "worlds/demo/output/production/midjourney/request-metadata.json" ||
      body.provider?.artifacts?.[0] !== "worlds/demo/output/production/midjourney/1-midjourney.png"
    ) {
      throw new Error(`unexpected production evidence item body: ${options.body}`);
    }
    return {
      kind: "pool_production_evidence_import",
      project_slug: "demo",
      source: body.source ?? "runtime-production-evidence-item-template",
      imported_at: "2026-06-11T00:02:00Z",
      summary: {
        providers: 1,
        software_actions: 0,
        desktop_vision: 0,
      },
      coverage: {
        complete: false,
        would_satisfy_prd_production_evidence: false,
        providers: {
          required: ["midjourney", "openai-image-2", "nano-banana-pro", "suno", "worldlabs-marble", "tripo-splat", "sam-3d", "spark-3dgs", "qunhe-3d"],
          provided: ["midjourney"],
          covered: 1,
          missing: ["openai-image-2", "nano-banana-pro", "suno", "worldlabs-marble", "tripo-splat", "sam-3d", "spark-3dgs", "qunhe-3d"],
          complete: false,
        },
        software_actions: {
          required: ["unreal", "blender", "comfyui", "resolve", "unity", "touchdesigner", "madmapper", "nuke", "motion-db", "editing-suite", "hermes"],
          provided: [],
          covered: 0,
          missing: ["unreal", "blender", "comfyui", "resolve", "unity", "touchdesigner", "madmapper", "nuke", "motion-db", "editing-suite", "hermes"],
          complete: false,
        },
        desktop_vision: {
          required: ["external_visual_model"],
          provided: 0,
          external_visual_model: false,
          missing: ["external_visual_model"],
          complete: false,
        },
      },
      prd_readiness: prdReadiness,
      snapshot: runtimeSnapshotPayload(),
    };
  }
  if (url.pathname === "/api/production-evidence/merge") {
    const body = JSON.parse(options.body ?? "{}");
    if (
      body.project_slug !== "demo" ||
      body.source !== "web-production-evidence-merge" ||
      body.bundles?.length !== 1 ||
      body.bundles?.[0]?.providers?.length !== 9 ||
      body.bundles?.[0]?.software_actions?.length !== 11 ||
      body.bundles?.[0]?.desktop_vision?.length !== 1
    ) {
      throw new Error(`unexpected production evidence merge body: ${options.body}`);
    }
    const bundle = {
      project_slug: "demo",
      source: "web-production-evidence-merge",
      providers: body.bundles.flatMap((bundle) => bundle.providers ?? []),
      software_actions: body.bundles.flatMap((bundle) => bundle.software_actions ?? []),
      desktop_vision: body.bundles.flatMap((bundle) => bundle.desktop_vision ?? []),
      merge: {
        input_count: body.bundles.length,
        inputs: body.bundles.map((bundle, index) => ({
          index: index + 1,
          source: bundle.source,
          project_slug: bundle.project_slug,
          providers: bundle.providers?.length ?? 0,
          software_actions: bundle.software_actions?.length ?? 0,
          desktop_vision: bundle.desktop_vision?.length ?? 0,
        })),
      },
    };
    return {
      kind: "pool_production_evidence_merge",
      project_slug: "demo",
      source: "web-production-evidence-merge",
      writes: 0,
      summary: {
        input_bundles: body.bundles.length,
        providers: bundle.providers.length,
        software_actions: bundle.software_actions.length,
        desktop_vision: bundle.desktop_vision.length,
      },
      input_summaries: bundle.merge.inputs,
      bundle,
      commands: {
        closeout: "pool-cli --project demo closeout-production-evidence --output <merged-bundle.json> <bundle-a.json> <bundle-b.json>...",
        validate: "pool-cli --project demo validate-production-evidence <merged-bundle.json>",
        import: "pool-cli --project demo import-production-evidence <merged-bundle.json>",
        readiness: "pool-cli --project demo prd-readiness",
      },
    };
  }
  if (url.pathname === "/api/production-evidence/closeout") {
    const body = JSON.parse(options.body ?? "{}");
    if (
      body.project_slug !== "demo" ||
      body.source !== "web-production-evidence-closeout" ||
      body.bundles?.length !== 1 ||
      body.bundles?.[0]?.providers?.length !== 9 ||
      body.bundles?.[0]?.software_actions?.length !== 11 ||
      body.bundles?.[0]?.desktop_vision?.length !== 1
    ) {
      throw new Error(`unexpected production evidence closeout body: ${options.body}`);
    }
    const bundle = {
      project_slug: "demo",
      source: "web-production-evidence-closeout",
      providers: body.bundles.flatMap((bundle) => bundle.providers ?? []),
      software_actions: body.bundles.flatMap((bundle) => bundle.software_actions ?? []),
      desktop_vision: body.bundles.flatMap((bundle) => bundle.desktop_vision ?? []),
      merge: {
        input_count: body.bundles.length,
        inputs: body.bundles.map((bundle, index) => ({
          index: index + 1,
          source: bundle.source,
          project_slug: bundle.project_slug,
          providers: bundle.providers?.length ?? 0,
          software_actions: bundle.software_actions?.length ?? 0,
          desktop_vision: bundle.desktop_vision?.length ?? 0,
        })),
      },
    };
    const requiredProviders = ["midjourney", "openai-image-2", "nano-banana-pro", "suno", "worldlabs-marble", "tripo-splat", "sam-3d", "spark-3dgs", "qunhe-3d"];
    const requiredSoftware = ["unreal", "blender", "comfyui", "resolve", "unity", "touchdesigner", "madmapper", "nuke", "motion-db", "editing-suite", "hermes"];
    const validation = {
      kind: "pool_production_evidence_validation",
      valid: true,
      writes: 0,
      project_slug: "demo",
      source: "web-production-evidence-closeout",
      summary: {
        providers: bundle.providers.length,
        software_actions: bundle.software_actions.length,
        desktop_vision: bundle.desktop_vision.length,
      },
      coverage: {
        complete: true,
        would_satisfy_prd_production_evidence: true,
        providers: {
          required: requiredProviders,
          provided: bundle.providers.map((provider) => provider.provider_id),
          covered: 9,
          missing: [],
          complete: true,
        },
        software_actions: {
          required: requiredSoftware,
          provided: bundle.software_actions.map((action) => action.adapter_id),
          covered: 11,
          missing: [],
          complete: true,
        },
        desktop_vision: {
          required: ["external_visual_model"],
          provided: 1,
          external_visual_model: true,
          missing: [],
          complete: true,
        },
      },
    };
    const merge = {
      kind: "pool_production_evidence_merge",
      project_slug: "demo",
      source: "web-production-evidence-closeout",
      writes: 0,
      summary: {
        input_bundles: body.bundles.length,
        providers: bundle.providers.length,
        software_actions: bundle.software_actions.length,
        desktop_vision: bundle.desktop_vision.length,
      },
      input_summaries: bundle.merge.inputs,
      bundle,
      commands: {
        closeout: "pool-cli --project demo closeout-production-evidence --output <merged-bundle.json> <bundle-a.json> <bundle-b.json>...",
        validate: "pool-cli --project demo validate-production-evidence <merged-bundle.json>",
        import: "pool-cli --project demo import-production-evidence <merged-bundle.json>",
        readiness: "pool-cli --project demo prd-readiness",
      },
    };
    const readyPrdReadiness = {
      ...prdReadiness,
      overall_status: "ready",
      summary: {
        total: 10,
        ready: 10,
        partial: 0,
        blocked: 0,
      },
      requirements: prdReadiness.requirements.map((requirement) => ({
        ...requirement,
        status: "ready",
        gaps: [],
      })),
      completion_gate: {
        ...prdReadiness.completion_gate,
        status: "complete",
        ready_for_completion: true,
        incomplete_requirements: [],
      },
    };
    const importResponse = {
      kind: "pool_production_evidence_import",
      project_slug: "demo",
      source: "web-production-evidence-closeout",
      imported_at: "2026-06-11T00:04:00Z",
      summary: validation.summary,
      coverage: validation.coverage,
      prd_readiness: readyPrdReadiness,
      snapshot: runtimeSnapshotPayload(),
    };
    return {
      kind: "pool_production_evidence_closeout",
      project_slug: "demo",
      source: "web-production-evidence-closeout",
      mode: body.import ? "import" : "validate",
      writes: body.import ? 20 : 0,
      ready_for_import: true,
      ...(body.import
        ? {
            ready_for_completion: true,
            prd_overall_status: "ready",
            prd_summary: readyPrdReadiness.summary,
            completion_gate: readyPrdReadiness.completion_gate,
          }
        : {}),
      merge,
      validation,
      ...(body.import ? { import: importResponse } : {}),
      commands: {
        closeout: "pool-cli --project demo closeout-production-evidence --output <merged-bundle.json> <bundle-a.json> <bundle-b.json>...",
        validate: "pool-cli --project demo validate-production-evidence <merged-bundle.json>",
        import: "pool-cli --project demo import-production-evidence <merged-bundle.json>",
        readiness: "pool-cli --project demo prd-readiness",
        completion_gate: "pool-cli --project demo prd-completion-gate --require-complete",
        completion_package: "pool-cli --project demo prd-completion-package --output-dir worlds/demo/output --include-snapshot",
      },
    };
  }
  if (url.pathname === "/api/production-evidence/validate") {
    const body = JSON.parse(options.body ?? "{}");
    if (productionEvidenceRequestHasTemplateIdentifiers(body)) {
      throw new Error("production evidence template identifiers are not accepted");
    }
    if (
      body.project_slug !== "demo" ||
      body.providers?.length !== 9 ||
      body.providers?.[0]?.provider_id !== "midjourney" ||
      body.providers?.[4]?.provider_id !== "worldlabs-marble" ||
      body.software_actions?.length !== 11 ||
      body.software_actions?.[0]?.adapter_id !== "unreal" ||
      body.software_actions?.[3]?.adapter_id !== "resolve" ||
      body.desktop_vision?.[0]?.controller_id !== "external-vision-controller"
    ) {
      throw new Error(`unexpected production evidence validation body: ${options.body}`);
    }
    const requiredProviders = ["midjourney", "openai-image-2", "nano-banana-pro", "suno", "worldlabs-marble", "tripo-splat", "sam-3d", "spark-3dgs", "qunhe-3d"];
    const requiredSoftware = ["unreal", "blender", "comfyui", "resolve", "unity", "touchdesigner", "madmapper", "nuke", "motion-db", "editing-suite", "hermes"];
    return {
      kind: "pool_production_evidence_validation",
      valid: true,
      writes: 0,
      project_slug: "demo",
      source: body.source ?? "web-production-evidence-example",
      summary: {
        providers: body.providers.length,
        software_actions: body.software_actions.length,
        desktop_vision: body.desktop_vision.length,
      },
      coverage: {
        complete: true,
        would_satisfy_prd_production_evidence: true,
        providers: {
          required: requiredProviders,
          provided: body.providers.map((provider) => provider.provider_id),
          covered: 9,
          missing: [],
          complete: true,
        },
        software_actions: {
          required: requiredSoftware,
          provided: body.software_actions.map((action) => action.adapter_id),
          covered: 11,
          missing: [],
          complete: true,
        },
        desktop_vision: {
          required: ["external_visual_model"],
          provided: 1,
          external_visual_model: true,
          missing: [],
          complete: true,
        },
      },
      providers: body.providers.map((provider) => ({
        provider_id: provider.provider_id,
        external_job_id: provider.external_job_id,
        artifacts: provider.artifacts.length,
        writes_on_validate: 0,
      })),
      software_actions: body.software_actions.map((action) => ({
        adapter_id: action.adapter_id,
        external_action_id: action.external_action_id,
        artifacts: action.artifacts.length,
        writes_on_validate: 0,
      })),
      desktop_vision: [
        {
          adapter_id: body.desktop_vision[0].adapter_id,
          external_action_id: body.desktop_vision[0].external_action_id,
          writes_on_validate: 0,
        },
      ],
    };
  }
  if (url.pathname === "/api/production-evidence") {
    const body = JSON.parse(options.body ?? "{}");
    if (productionEvidenceRequestHasTemplateIdentifiers(body)) {
      throw new Error("production evidence template identifiers are not accepted");
    }
    if (
      body.project_slug !== "demo" ||
      body.providers?.length !== 9 ||
      body.providers?.[0]?.provider_id !== "midjourney" ||
      body.providers?.[4]?.provider_id !== "worldlabs-marble" ||
      body.software_actions?.length !== 11 ||
      body.software_actions?.[0]?.adapter_id !== "unreal" ||
      body.software_actions?.[3]?.adapter_id !== "resolve" ||
      body.desktop_vision?.[0]?.controller_id !== "external-vision-controller"
    ) {
      throw new Error(`unexpected production evidence body: ${options.body}`);
    }
    const requiredProviders = ["midjourney", "openai-image-2", "nano-banana-pro", "suno", "worldlabs-marble", "tripo-splat", "sam-3d", "spark-3dgs", "qunhe-3d"];
    const requiredSoftware = ["unreal", "blender", "comfyui", "resolve", "unity", "touchdesigner", "madmapper", "nuke", "motion-db", "editing-suite", "hermes"];
    const readyPrdReadiness = {
      ...prdReadiness,
      overall_status: "ready",
      summary: {
        total: 10,
        ready: 10,
        partial: 0,
        blocked: 0,
      },
      requirements: prdReadiness.requirements.map((requirement) => ({
        ...requirement,
        status: "ready",
        gaps: [],
      })),
      completion_gate: {
        ...prdReadiness.completion_gate,
        status: "complete",
        ready_for_completion: true,
        incomplete_requirements: [],
      },
    };
    return {
      kind: "pool_production_evidence_import",
      project_slug: "demo",
      source: body.source ?? "web-production-evidence-example",
      imported_at: "2026-06-11T00:03:00Z",
      summary: {
        providers: body.providers.length,
        software_actions: body.software_actions.length,
        desktop_vision: body.desktop_vision.length,
      },
      coverage: {
        complete: true,
        would_satisfy_prd_production_evidence: true,
        providers: {
          required: requiredProviders,
          provided: body.providers.map((provider) => provider.provider_id),
          covered: 9,
          missing: [],
          complete: true,
        },
        software_actions: {
          required: requiredSoftware,
          provided: body.software_actions.map((action) => action.adapter_id),
          covered: 11,
          missing: [],
          complete: true,
        },
        desktop_vision: {
          required: ["external_visual_model"],
          provided: 1,
          external_visual_model: true,
          missing: [],
          complete: true,
        },
      },
      prd_readiness: readyPrdReadiness,
      snapshot: runtimeSnapshotPayload(),
    };
  }
  if (url.pathname === "/api/output-packages/results") {
    const body = JSON.parse(options.body ?? "{}");
    if (
      body.project_slug !== "demo" ||
      body.target !== "game" ||
      body.status !== "succeeded" ||
      body.adapter_id !== "unreal" ||
      body.local_path !== "worlds/demo/output/deliverables/2-game-build.json"
    ) {
      throw new Error(`unexpected output package result body: ${options.body}`);
    }
    return {
      report: {
        task_id: "task-output-result",
        status: "Succeeded",
        target: "game",
        local_path: body.local_path,
        manifest: {
          target: "game",
          execution_result: {
            status: "succeeded",
            runtime: "Unreal",
            adapter_id: "unreal",
            message: "运行原型 后段执行完成",
            artifacts: body.artifacts,
          },
        },
        catalog: outputPackagesWithGameResult,
      },
      task: {
        id: "task-output-result",
        project_slug: "demo",
        node_id: "outputs",
        title: "Output result: 运行原型",
        status: "Succeeded",
      },
      snapshot: runtimeSnapshotPayload(),
    };
  }
  if (url.pathname === "/api/output-packages") return outputPackages;
  if (url.pathname === "/api/software-conformance-packages") {
    const body = options.body ? JSON.parse(options.body) : {};
    return {
      report: {
        kind: "pool_software_conformance_package_report",
        adapter_id: body.adapter_id ?? "resolve",
        paths: {
          runner_script: `worlds/demo/output/control/software-conformance/${body.adapter_id ?? "resolve"}/4-software-conformance-runner.sh`,
          manifest: `worlds/demo/output/control/software-conformance/${body.adapter_id ?? "resolve"}/5-software-conformance-package-manifest.json`,
        },
        commands: {
          preflight: "worlds/demo/output/control/software-conformance/resolve/4-software-conformance-runner.sh --preflight",
        },
      },
      task: {
        id: "task-software-conformance-package",
        project_slug: "demo",
        node_id: body.node_id ?? "software",
        status: "Succeeded",
      },
      snapshot: runtimeSnapshotPayload(),
    };
  }
  if (url.pathname === "/api/provider-conformance-packages") {
    const body = options.body ? JSON.parse(options.body) : {};
    return {
      report: {
        kind: "pool_provider_conformance_package_report",
        provider_id: body.provider_id ?? "worldlabs-marble",
        paths: {
          runner_script: `worlds/demo/output/control/provider-conformance/${body.provider_id ?? "worldlabs-marble"}/5-provider-conformance-runner.sh`,
          manifest: `worlds/demo/output/control/provider-conformance/${body.provider_id ?? "worldlabs-marble"}/6-provider-conformance-package-manifest.json`,
        },
        commands: {
          preflight: "worlds/demo/output/control/provider-conformance/worldlabs-marble/5-provider-conformance-runner.sh --preflight",
        },
      },
      task: {
        id: "task-provider-conformance-package",
        project_slug: "demo",
        node_id: body.node_id ?? "three",
        status: "Succeeded",
      },
      snapshot: runtimeSnapshotPayload(),
    };
  }
  if (url.pathname === "/api/integration-conformance-packages") {
    const body = options.body ? JSON.parse(options.body) : {};
    return {
      report: {
        kind: "pool_integration_conformance_package_report",
        summary: {
          providers: 9,
          software_adapters: 11,
          agent: body.include_agent !== false,
          local_files: 139,
        },
        paths: {
          runner_script: "worlds/demo/output/control/integration-conformance/2-integration-conformance-runner.sh",
          manifest: "worlds/demo/output/control/integration-conformance/3-integration-conformance-package-manifest.json",
        },
        commands: {
          preflight: "worlds/demo/output/control/integration-conformance/2-integration-conformance-runner.sh --preflight",
        },
      },
      task: {
        id: "task-integration-conformance-package",
        project_slug: "demo",
        node_id: body.node_id ?? "agent",
        status: "Succeeded",
      },
      snapshot: runtimeSnapshotPayload(),
    };
  }
  if (url.pathname === "/api/agent-conformance-packages") {
    const body = options.body ? JSON.parse(options.body) : {};
    return {
      report: {
        kind: "pool_agent_conformance_package_report",
        session_kind: body.kind ?? "all",
        paths: {
          runner_script: `worlds/demo/output/control/agent-conformance/${body.kind ?? "all"}/4-agent-conformance-runner.sh`,
          manifest: `worlds/demo/output/control/agent-conformance/${body.kind ?? "all"}/5-agent-conformance-package-manifest.json`,
        },
        commands: {
          preflight: "worlds/demo/output/control/agent-conformance/all/4-agent-conformance-runner.sh --preflight",
        },
      },
      task: {
        id: "task-agent-conformance-package",
        project_slug: "demo",
        node_id: body.node_id ?? "agent",
        status: "Succeeded",
      },
      snapshot: runtimeSnapshotPayload(),
    };
  }
  if (url.pathname === "/api/discovery") return runtimeDiscovery;
  if (url.pathname === "/api/workflow-context") return workflowContext;
  if (url.pathname === "/api/adapters") {
    return {
      providers: [
        {
          id: "worldlabs-marble",
          display_name: "World Labs Marble",
          kind: "ThreeDgs",
          endpoint: "provider://worldlabs-marble",
          auth_env_key: "POOL_MARBLE_API_KEY",
          output_contract: "image-blaster indexed local 3DGS package",
          high_cost: true,
        },
        {
          id: "midjourney",
          display_name: "Midjourney",
          kind: "AiImage",
          endpoint: "provider://midjourney",
          auth_env_key: "POOL_MIDJOURNEY_API_KEY",
          output_contract: "downloaded local image files",
          high_cost: false,
        },
      ],
      software_adapters: [
        {
          id: "unreal",
          display_name: "Unreal",
          control_modes: ["api/mcp", "skills/cli", "desktop-recognition"],
          priority: 1,
          desktop_fallback: true,
        },
        {
          id: "touchdesigner",
          display_name: "TouchDesigner",
          control_modes: ["python-api", "osc", "desktop-recognition"],
          priority: 6,
          desktop_fallback: true,
        },
        {
          id: "resolve",
          display_name: "DaVinci Resolve",
          control_modes: ["api/mcp", "skills/cli", "desktop-recognition"],
          priority: 4,
          desktop_fallback: true,
        },
      ],
      provider_aliases: {
        "world-labs-marble": "worldlabs-marble",
      },
    };
  }
  if (url.pathname === "/api/provider-contracts") return providerContracts;
  if (url.pathname === "/api/provider-gateway-worker") return providerGatewayWorkerContract;
  if (url.pathname === "/api/software-contracts") return softwareContracts;
  if (url.pathname === "/api/desktop-recognition/contract") return desktopRecognitionContract;
  if (url.pathname === "/api/prompts") {
    if (url.searchParams.get("name")) {
      return {
        description: "Pool external software control handoff prompt",
        messages: [
          {
            role: "user",
            content: {
              type: "text",
              text: `Prepare a Pool external software handoff.\nAdapter: ${url.searchParams.get("adapter_id")}`,
            },
          },
        ],
      };
    }
    return prompts;
  }
  if (url.pathname === "/api/agent-sessions/transcript") {
    if (url.searchParams.get("session_id") !== "agent-session-cli") {
      throw new Error(`unexpected transcript session id: ${url.searchParams.get("session_id")}`);
    }
    return {
      session_id: "agent-session-cli",
      project_slug: "demo",
      tools: ["cli", "mcp"],
      token_budget: 74000,
      token_used: 1200,
      transcript_path: "worlds/demo/output/agent-cli-transcript.json",
      bytes: 180,
      transcript: {
        session_id: "agent-session-cli",
        kind: "agent_cli",
        project_slug: "demo",
        token_used: 1200,
        command: {
          id: "workflow-context",
          title: "读取工作流上下文",
          command: "pool-cli --project demo workflow-context workflow-demo",
        },
      },
      transcript_text: null,
    };
  }
  if (url.pathname === "/api/projects") return { project_filter: "demo", projects: snapshot.projects };
  if (url.pathname === "/api/desktop-recognition/requests") {
    return { requests: desktopQueueOpen ? [desktopRecognitionRequest] : [] };
  }
  if (url.pathname === "/api/desktop-recognition/run-next") {
    const body = JSON.parse(options.body ?? "{}");
    if (body.controller_id !== "web-prototype-dry-run" || body.status !== "succeeded" || body.limit !== 1) {
      throw new Error(`unexpected desktop run-next body: ${options.body}`);
    }
    desktopQueueOpen = false;
    return {
      controller: "web-prototype-dry-run",
      mode: "dry_run",
      requested_status: "succeeded",
      queued_count: 1,
      processed_count: 1,
      skipped: [],
      callbacks: [
        {
          software_action_id: "action-desktop",
          status_code: 200,
          response: {
            task: {
              id: "task-desktop",
              status: "Succeeded",
            },
            software_action: {
              id: "action-desktop",
              verification: {
                desktop_recognition_status: "succeeded",
                controller_result: {
                  controller: "web-prototype-dry-run",
                  mode: "dry_run",
                },
              },
            },
            snapshot: runtimeSnapshotPayload(),
          },
        },
      ],
    };
  }
  if (url.pathname === "/api/handoff-packages") {
    return {
      report: {
        task_id: "task-handoff-package",
        status: "Succeeded",
        local_paths: [
          "worlds/demo/output/control/handoff/.1-runtime-handoff-request.json",
          "worlds/demo/output/control/handoff/1-runtime-handoff.json",
          "worlds/demo/output/control/handoff/2-runtime-preflight.json",
          "worlds/demo/output/control/handoff/3-runtime-graph.json",
          "worlds/demo/output/control/handoff/5-worker-self-checks.sh",
          "worlds/demo/output/control/handoff/6-worker-self-checks-preflight.json",
          "worlds/demo/output/control/handoff/7-integration-readiness.json",
          "worlds/demo/output/control/handoff/8-runtime-handoff-package-manifest.json",
          "worlds/demo/output/control/handoff/4-runtime-snapshot.json",
        ],
        request_path: "worlds/demo/output/control/handoff/.1-runtime-handoff-request.json",
        handoff_path: "worlds/demo/output/control/handoff/1-runtime-handoff.json",
        preflight_path: "worlds/demo/output/control/handoff/2-runtime-preflight.json",
        graph_path: "worlds/demo/output/control/handoff/3-runtime-graph.json",
        worker_self_checks_path: "worlds/demo/output/control/handoff/5-worker-self-checks.sh",
        worker_self_checks_preflight_path: "worlds/demo/output/control/handoff/6-worker-self-checks-preflight.json",
        integration_readiness_path: "worlds/demo/output/control/handoff/7-integration-readiness.json",
        manifest_path: "worlds/demo/output/control/handoff/8-runtime-handoff-package-manifest.json",
        snapshot_path: "worlds/demo/output/control/handoff/4-runtime-snapshot.json",
        operator_checklist: [
          {
            step: 1,
            owner: "agent_operator",
            action: "Open the offline handoff manifest",
            path: "worlds/demo/output/control/handoff/8-runtime-handoff-package-manifest.json",
            verify: "Team lanes visible",
          },
          {
            step: 2,
            owner: "creative_director",
            action: "Resolve blocking approval gates",
            path: "worlds/demo/output/control/handoff/2-runtime-preflight.json",
            command: "pool-cli --project demo runtime-preflight",
            verify: "No high-cost task before approval",
          },
          {
            step: 3,
            owner: "ai_3dgs_td",
            action: "Use integration readiness run plan",
            path: "worlds/demo/output/control/handoff/7-integration-readiness.json",
            command: "pool-cli --project demo integration-readiness",
            verify: "Rows move toward ready",
          },
        ],
        agent_entrypoint: {
          first_file: "worlds/demo/output/control/handoff/8-runtime-handoff-package-manifest.json",
          primary_runbook: "worlds/demo/output/control/handoff/1-runtime-handoff.json",
          readiness: "worlds/demo/output/control/handoff/7-integration-readiness.json",
          worker_smoke: "worlds/demo/output/control/handoff/5-worker-self-checks.sh",
          mcp_stdio: "pool-cli --project demo serve-mcp",
        },
        mcp_resources: [
          "pool://runtime-handoff",
          "pool://runtime-preflight",
          "pool://integration-readiness",
        ],
        assets: [
          {
            id: "asset-handoff",
            project_slug: "demo",
            task_id: "task-handoff-package",
            path: "worlds/demo/output/control/handoff/1-runtime-handoff.json",
            kind: "runtime_handoff_package",
            provider_url: "pool-runtime-handoff://package",
            created_at: "2026-06-11T00:00:00Z",
          },
          {
            id: "asset-handoff-worker-self-checks",
            project_slug: "demo",
            task_id: "task-handoff-package",
            path: "worlds/demo/output/control/handoff/5-worker-self-checks.sh",
            kind: "runtime_handoff_package",
            provider_url: "pool-runtime-handoff://package",
            created_at: "2026-06-11T00:00:00Z",
          },
        ],
      },
      task: {
        id: "task-handoff-package",
        project_slug: "demo",
        node_id: "agent",
        title: "Runtime handoff package",
        status: "Succeeded",
        provider_id: "runtime-handoff-package",
        cost_estimate_tokens: 200,
        requires_approval: false,
        request_metadata_path: "worlds/demo/output/control/handoff/.1-runtime-handoff-request.json",
        created_at: "2026-06-11T00:00:00Z",
        updated_at: "2026-06-11T00:00:00Z",
      },
      snapshot: runtimeSnapshotPayload(),
    };
  }
  if (url.pathname === "/api/agent-sessions") {
    agentSessionPersisted = true;
    return {
      task: agentSessionTask,
      snapshot: runtimeSnapshotPayload(),
    };
  }
  if (url.pathname === "/api/node-context") {
    if (url.searchParams.get("node_id") === "three") return nodeContext;
    return { ...nodeContext, node_id: "brief", node: runtimeGraph.workflows[0].nodes[0], summary: { tasks: 1, assets: 0 } };
  }
  throw new Error(`unexpected fetch: ${rawUrl}`);
}

const context = vm.createContext({
  console,
  document,
  localStorage,
  URL,
  URLSearchParams,
  Blob,
  Date,
  Math,
  JSON,
  Promise,
  setTimeout,
  clearTimeout,
  setInterval: () => 1,
  clearInterval: () => {},
  EventSource: MockEventSource,
  WebSocket: MockWebSocket,
  window: {
    location: {
      search: "?runtime=http://runtime.test&project=demo",
      pathname: "/apps/web-prototype/",
      hash: "",
    },
    history: {
      replaceState() {},
    },
  },
  fetch: async (url, options = {}) => {
    fetchCalls.push(String(url));
    fetchRequests.push({ url: String(url), options });
    const payload = payloadForUrl(String(url), options);
    return {
      ok: true,
      status: 200,
      json: async () => payload,
    };
  },
});

vm.runInContext(appSource, context, { filename: "app.js" });

await new Promise((resolve) => setTimeout(resolve, 20));
vm.runInContext('selectNode("three")', context);
await new Promise((resolve) => setTimeout(resolve, 20));
await vm.runInContext('applyHermesRunbook()', context);
await new Promise((resolve) => setTimeout(resolve, 20));
await vm.runInContext('stageCliCommand("workflow-context")', context);
await new Promise((resolve) => setTimeout(resolve, 20));
await vm.runInContext('loadAgentTranscript("agent-session-cli")', context);
await new Promise((resolve) => setTimeout(resolve, 20));
await vm.runInContext('createHandoffPackage()', context);
await new Promise((resolve) => setTimeout(resolve, 20));
const handoffPackageEventHtml = vm.runInContext('document.querySelector("#eventStream").innerHTML', context);
await vm.runInContext('runRuntimeExecutionPlanNext(false)', context);
await new Promise((resolve) => setTimeout(resolve, 20));
await vm.runInContext('runNextDesktopRecognitionRequest()', context);
await new Promise((resolve) => setTimeout(resolve, 20));
await vm.runInContext('exportSoftwareConformancePackage("resolve")', context);
await new Promise((resolve) => setTimeout(resolve, 20));
const softwareConformancePackageEventHtml = vm.runInContext('document.querySelector("#eventStream").innerHTML', context);
await vm.runInContext('exportProviderConformancePackage("worldlabs-marble")', context);
await new Promise((resolve) => setTimeout(resolve, 20));
const providerConformancePackageEventHtml = vm.runInContext('document.querySelector("#eventStream").innerHTML', context);
await vm.runInContext('exportAgentConformancePackage("all")', context);
await new Promise((resolve) => setTimeout(resolve, 20));
const agentConformancePackageEventHtml = vm.runInContext('document.querySelector("#eventStream").innerHTML', context);
await vm.runInContext('exportIntegrationConformancePackage()', context);
await new Promise((resolve) => setTimeout(resolve, 20));
const integrationConformancePackageEventHtml = vm.runInContext('document.querySelector("#eventStream").innerHTML', context);
await vm.runInContext('recordOutputResult("game")', context);
await new Promise((resolve) => setTimeout(resolve, 20));
vm.runInContext('selectNode("three")', context);
await new Promise((resolve) => setTimeout(resolve, 20));
webSockets.find((socket) => socket.url.includes("/api/agent-sessions/ws"))?.emit({
  type: "agent-session",
  session_id: "agent-session-cli",
  latest_event_id: "event-agent-session-ws",
  transcript: {
    session_id: "agent-session-cli",
    project_slug: "demo",
    transcript_path: "worlds/demo/output/agent-cli-transcript.json",
    transcript: {
      kind: "agent_cli",
      websocket_stream: true,
      command: {
        id: "workflow-context",
        title: "读取工作流上下文",
        command: "pool-cli --project demo workflow-context workflow-demo",
      },
    },
  },
});
webSockets.find((socket) => socket.url.includes("/api/events/ws"))?.emit({
  type: "runtime-event",
  event: {
    id: "event-websocket",
    project_slug: "demo",
    level: "Ok",
    message: "runtime websocket event",
    created_at: "2026-06-11T00:02:00Z",
  },
});
await new Promise((resolve) => setTimeout(resolve, 20));

const result = vm.runInContext(
  `JSON.stringify({
    selectedNode: state.selectedNode,
    selectedNodeContextStatus: state.selectedNodeContextStatus,
    selectedNodeContextId: state.selectedNodeContext?.node_id,
    workflowContextStatus: state.workflowContextStatus,
    workflowContextId: state.workflowContext?.workflow_id,
    runbookStatus: state.hermes.runbookStatus,
    runbookCount: state.hermes.runbooks.length,
    hermesPrompt: document.querySelector("#hermesPrompt").value,
    hermesSessionHtml: document.querySelector("#hermesSessionStream").innerHTML,
    cliHtml: document.querySelector("#cliCommandList").innerHTML,
    budgetSummary: document.querySelector("#runtimeBudgetSummary").textContent,
    budgetHtml: document.querySelector("#runtimeBudgetPanel").innerHTML,
    preflightSummary: document.querySelector("#runtimePreflightSummary").textContent,
    preflightHtml: document.querySelector("#runtimePreflightPanel").innerHTML,
    handoffSummary: document.querySelector("#runtimeHandoffSummary").textContent,
    handoffHtml: document.querySelector("#runtimeHandoffPanel").innerHTML,
    prdSummary: document.querySelector("#prdReadinessSummary").textContent,
    prdHtml: document.querySelector("#prdReadinessPanel").innerHTML,
    integrationReadinessSummary: document.querySelector("#integrationReadinessSummary").textContent,
    integrationReadinessHtml: document.querySelector("#integrationReadinessPanel").innerHTML,
    productionEvidenceHtml: document.querySelector("#productionEvidenceResult").innerHTML,
    outputManifestHtml: document.querySelector("#outputManifestPanel").innerHTML,
    runtimeDiscoveryHtml: document.querySelector("#runtimeDiscoveryPanel").innerHTML,
    providerGatewayWorkerHtml: document.querySelector("#providerGatewayWorkerPanel").innerHTML,
    providerHtml: document.querySelector("#gsProviderGrid").innerHTML + document.querySelector("#aiProviderGrid").innerHTML,
    softwareHtml: document.querySelector("#softwareTable").innerHTML,
    desktopSummary: document.querySelector("#desktopQueueSummary").textContent,
    desktopQueueHtml: document.querySelector("#desktopRecognitionQueue").innerHTML,
    desktopRunDisabled: document.querySelector("#runDesktopQueue").disabled,
    nodeLog: document.querySelector("#nodeLog").innerHTML,
    eventHtml: document.querySelector("#eventStream").innerHTML,
  })`,
  context,
);

const parsed = JSON.parse(result);
await vm.runInContext('createPrdCompletionPackage()', context);
await new Promise((resolve) => setTimeout(resolve, 20));
const prdCompletionPackageResult = JSON.parse(
  vm.runInContext(
    `JSON.stringify({
      prdSummary: document.querySelector("#prdReadinessSummary").textContent,
      prdHtml: document.querySelector("#prdReadinessPanel").innerHTML,
      eventHtmlAfterPrdCompletionPackage: document.querySelector("#eventStream").innerHTML
    })`,
    context,
  ),
);
await vm.runInContext('createProductionEvidenceHandoffPackage()', context);
await new Promise((resolve) => setTimeout(resolve, 20));
const productionEvidenceHandoffPackageResult = JSON.parse(
  vm.runInContext(
    `JSON.stringify({
      productionEvidenceSummary: document.querySelector("#productionEvidenceSummary").textContent,
      productionEvidenceHtml: document.querySelector("#productionEvidenceResult").innerHTML,
      eventHtmlAfterProductionEvidencePackage: document.querySelector("#eventStream").innerHTML
    })`,
    context,
  ),
);
await vm.runInContext('createProductionEvidenceRunPlan()', context);
await new Promise((resolve) => setTimeout(resolve, 20));
const productionEvidenceRunPlanResult = JSON.parse(
  vm.runInContext(
    `JSON.stringify({
      productionEvidenceSummary: document.querySelector("#productionEvidenceSummary").textContent,
      productionEvidenceHtml: document.querySelector("#productionEvidenceResult").innerHTML,
      eventHtmlAfterProductionEvidenceRunPlan: document.querySelector("#eventStream").innerHTML
    })`,
    context,
  ),
);
await vm.runInContext('claimProductionEvidenceTask()', context);
await new Promise((resolve) => setTimeout(resolve, 20));
const productionEvidenceTaskClaimResult = JSON.parse(
  vm.runInContext(
    `JSON.stringify({
      productionEvidenceSummary: document.querySelector("#productionEvidenceSummary").textContent,
      productionEvidenceHtml: document.querySelector("#productionEvidenceResult").innerHTML,
      eventHtmlAfterProductionEvidenceTaskClaim: document.querySelector("#eventStream").innerHTML
    })`,
    context,
  ),
);
await vm.runInContext('loadProductionEvidenceLedgerBundle()', context);
await new Promise((resolve) => setTimeout(resolve, 20));
const productionEvidenceLedgerBundleResult = JSON.parse(
  vm.runInContext(
    `JSON.stringify({
      productionEvidenceSummary: document.querySelector("#productionEvidenceSummary").textContent,
      productionEvidenceHtml: document.querySelector("#productionEvidenceResult").innerHTML,
      productionEvidenceBundle: document.querySelector("#productionEvidenceBundle").value,
      eventHtmlAfterLedgerBundle: document.querySelector("#eventStream").innerHTML
    })`,
    context,
  ),
);
await vm.runInContext('loadProductionEvidenceItemTemplate()', context);
await new Promise((resolve) => setTimeout(resolve, 20));
const runtimeItemTemplateResult = JSON.parse(
  vm.runInContext(
    `JSON.stringify({
      productionEvidenceSummary: document.querySelector("#productionEvidenceSummary").textContent,
      productionEvidenceHtml: document.querySelector("#productionEvidenceResult").innerHTML,
      productionEvidenceBundle: document.querySelector("#productionEvidenceBundle").value,
      eventHtmlAfterItemTemplateLoad: document.querySelector("#eventStream").innerHTML
    })`,
    context,
  ),
);
vm.runInContext(
  `document.querySelector("#productionEvidenceBundle").value = document
    .querySelector("#productionEvidenceBundle")
    .value
    .replaceAll("replace-with-real-", "real-")`,
  context,
);
await vm.runInContext('validateProductionEvidenceItem()', context);
await new Promise((resolve) => setTimeout(resolve, 20));
const productionItemValidationResult = JSON.parse(
  vm.runInContext(
    `JSON.stringify({
      productionEvidenceSummary: document.querySelector("#productionEvidenceSummary").textContent,
      productionEvidenceHtml: document.querySelector("#productionEvidenceResult").innerHTML,
      eventHtmlAfterItemValidation: document.querySelector("#eventStream").innerHTML
    })`,
    context,
  ),
);
await vm.runInContext('submitProductionEvidenceItem()', context);
await new Promise((resolve) => setTimeout(resolve, 20));
const productionItemResult = JSON.parse(
  vm.runInContext(
    `JSON.stringify({
      productionEvidenceSummary: document.querySelector("#productionEvidenceSummary").textContent,
      productionEvidenceHtml: document.querySelector("#productionEvidenceResult").innerHTML,
      eventHtmlAfterItemSubmit: document.querySelector("#eventStream").innerHTML
    })`,
    context,
  ),
);
await vm.runInContext('loadProductionEvidenceTemplate()', context);
await new Promise((resolve) => setTimeout(resolve, 20));
const runtimeTemplateResult = JSON.parse(
  vm.runInContext(
    `JSON.stringify({
      productionEvidenceSummary: document.querySelector("#productionEvidenceSummary").textContent,
      productionEvidenceHtml: document.querySelector("#productionEvidenceResult").innerHTML,
      productionEvidenceBundle: document.querySelector("#productionEvidenceBundle").value,
      eventHtmlAfterTemplateLoad: document.querySelector("#eventStream").innerHTML
    })`,
    context,
  ),
);
await vm.runInContext('fillProductionEvidenceExample()', context);
await new Promise((resolve) => setTimeout(resolve, 20));
await vm.runInContext('validateProductionEvidence()', context);
await new Promise((resolve) => setTimeout(resolve, 20));
const templateValidationResult = JSON.parse(
  vm.runInContext(
    `JSON.stringify({
      productionEvidenceSummary: document.querySelector("#productionEvidenceSummary").textContent,
      productionEvidenceHtml: document.querySelector("#productionEvidenceResult").innerHTML,
      eventHtmlAfterTemplateValidation: document.querySelector("#eventStream").innerHTML
    })`,
    context,
  ),
);
vm.runInContext(
  `document.querySelector("#productionEvidenceBundle").value = document
    .querySelector("#productionEvidenceBundle")
    .value
    .replaceAll("replace-with-real-", "real-")`,
  context,
);
await vm.runInContext('mergeProductionEvidence()', context);
await new Promise((resolve) => setTimeout(resolve, 20));
const mergeResult = JSON.parse(
  vm.runInContext(
    `JSON.stringify({
      productionEvidenceSummary: document.querySelector("#productionEvidenceSummary").textContent,
      productionEvidenceHtml: document.querySelector("#productionEvidenceResult").innerHTML,
      productionEvidenceBundle: document.querySelector("#productionEvidenceBundle").value,
      eventHtmlAfterMerge: document.querySelector("#eventStream").innerHTML
    })`,
    context,
  ),
);
await vm.runInContext('closeoutProductionEvidence(false)', context);
await new Promise((resolve) => setTimeout(resolve, 20));
const closeoutResult = JSON.parse(
  vm.runInContext(
    `JSON.stringify({
      productionEvidenceSummary: document.querySelector("#productionEvidenceSummary").textContent,
      productionEvidenceHtml: document.querySelector("#productionEvidenceResult").innerHTML,
      productionEvidenceBundle: document.querySelector("#productionEvidenceBundle").value,
      eventHtmlAfterCloseout: document.querySelector("#eventStream").innerHTML
    })`,
    context,
  ),
);
await vm.runInContext('validateProductionEvidence()', context);
await new Promise((resolve) => setTimeout(resolve, 20));
const validationResult = JSON.parse(
  vm.runInContext(
    `JSON.stringify({
      productionEvidenceSummary: document.querySelector("#productionEvidenceSummary").textContent,
      productionEvidenceHtml: document.querySelector("#productionEvidenceResult").innerHTML,
      eventHtmlAfterValidation: document.querySelector("#eventStream").innerHTML
    })`,
    context,
  ),
);
await vm.runInContext('importProductionEvidence()', context);
await new Promise((resolve) => setTimeout(resolve, 20));
const productionResult = JSON.parse(
  vm.runInContext(
    `JSON.stringify({
      productionEvidenceSummary: document.querySelector("#productionEvidenceSummary").textContent,
      productionEvidenceHtml: document.querySelector("#productionEvidenceResult").innerHTML,
      prdSummaryAfterProduction: document.querySelector("#prdReadinessSummary").textContent,
      eventHtmlAfterProduction: document.querySelector("#eventStream").innerHTML
    })`,
    context,
  ),
);
await vm.runInContext('closeoutProductionEvidence(true)', context);
await new Promise((resolve) => setTimeout(resolve, 20));
const closeoutImportResult = JSON.parse(
  vm.runInContext(
    `JSON.stringify({
      productionEvidenceSummary: document.querySelector("#productionEvidenceSummary").textContent,
      productionEvidenceHtml: document.querySelector("#productionEvidenceResult").innerHTML,
      prdSummaryAfterCloseoutImport: document.querySelector("#prdReadinessSummary").textContent,
      eventHtmlAfterCloseoutImport: document.querySelector("#eventStream").innerHTML
    })`,
    context,
  ),
);
const nodeContextCalls = fetchCalls.filter((url) => url.includes("/api/node-context"));
const workflowContextCalls = fetchCalls.filter((url) => url.includes("/api/workflow-context"));
const runtimeBudgetCalls = fetchCalls.filter((url) => url.includes("/api/runtime-budget"));
const runtimeApiKeyCalls = fetchCalls.filter((url) => url.includes("/api/api-keys"));
const runtimePreflightCalls = fetchCalls.filter((url) => url.includes("/api/runtime-preflight"));
const runtimeExecutionPlanCalls = fetchCalls.filter((url) => url.includes("/api/runtime-execution-plan"));
const runtimeHandoffCalls = fetchCalls.filter((url) => url.includes("/api/runtime-handoff"));
const prdReadinessCalls = fetchCalls.filter((url) => url.includes("/api/prd-readiness"));
const prdCompletionGateCalls = fetchCalls.filter((url) => url.includes("/api/prd-completion-gate"));
const productionEvidenceRequirementsCalls = fetchCalls.filter((url) => url.includes("/api/production-evidence/requirements"));
const productionEvidenceTaskCalls = fetchCalls.filter((url) => url.includes("/api/production-evidence/tasks"));
const productionEvidenceHandoffCalls = fetchCalls.filter((url) => url.includes("/api/production-evidence/handoff"));
const outputPackageCalls = fetchCalls.filter((url) => url.includes("/api/output-packages"));
const runtimeDiscoveryCalls = fetchCalls.filter((url) => url.includes("/api/discovery"));
const adapterCalls = fetchCalls.filter((url) => url.includes("/api/adapters"));
const integrationReadinessCalls = fetchCalls.filter((url) => url.includes("/api/integration-readiness"));
const providerContractCalls = fetchCalls.filter((url) => url.includes("/api/provider-contracts"));
const providerGatewayWorkerCalls = fetchCalls.filter((url) => url.includes("/api/provider-gateway-worker"));
const providerConformancePackageRequests = fetchRequests.filter((request) => request.url.includes("/api/provider-conformance-packages"));
const integrationConformancePackageRequests = fetchRequests.filter((request) => request.url.includes("/api/integration-conformance-packages"));
const softwareContractCalls = fetchCalls.filter((url) => url.includes("/api/software-contracts"));
const softwareConformancePackageRequests = fetchRequests.filter((request) => request.url.includes("/api/software-conformance-packages"));
const agentConformancePackageRequests = fetchRequests.filter((request) => request.url.includes("/api/agent-conformance-packages"));
const desktopContractCalls = fetchCalls.filter((url) => url.includes("/api/desktop-recognition/contract"));
const promptCalls = fetchCalls.filter((url) => url.includes("/api/prompts"));
const transcriptCalls = fetchCalls.filter((url) => url.includes("/api/agent-sessions/transcript"));
const agentSessionRequests = fetchRequests.filter((request) => request.url.includes("/api/agent-sessions"));
const handoffPackageRequests = fetchRequests.filter((request) => request.url.includes("/api/handoff-packages"));
const runtimeRunNextRequests = fetchRequests.filter((request) => request.url.includes("/api/runtime-execution-plan/run-next"));
const desktopRunNextRequests = fetchRequests.filter((request) => request.url.includes("/api/desktop-recognition/run-next"));
const outputPackageResultRequests = fetchRequests.filter((request) => request.url.includes("/api/output-packages/results"));
const prdCompletionPackageRequests = fetchRequests.filter((request) => request.url.includes("/api/prd-completion-package"));
const productionEvidenceValidationRequests = fetchRequests.filter((request) => request.url.includes("/api/production-evidence/validate"));
const productionEvidenceMergeRequests = fetchRequests.filter((request) => request.url.includes("/api/production-evidence/merge"));
const productionEvidenceCloseoutRequests = fetchRequests.filter((request) => request.url.includes("/api/production-evidence/closeout"));
const productionEvidenceTemplateRequests = fetchRequests.filter((request) => request.url.includes("/api/production-evidence/template"));
const productionEvidenceItemTemplateRequests = fetchRequests.filter((request) => request.url.includes("/api/production-evidence/item-template"));
const productionEvidenceTaskClaimRequests = fetchRequests.filter((request) => request.url.includes("/api/production-evidence/tasks/claim"));
const productionEvidenceItemValidationRequests = fetchRequests.filter((request) => request.url.includes("/api/production-evidence/items/validate"));
const productionEvidenceRunPlanRequests = fetchRequests.filter((request) => request.url.includes("/api/production-evidence/run-plan"));
const productionEvidenceLedgerBundleRequests = fetchRequests.filter((request) => request.url.includes("/api/production-evidence/bundle-from-ledger"));
const productionEvidenceHandoffPackageRequests = fetchRequests.filter((request) => request.url.includes("/api/production-evidence/handoff-packages"));
const productionEvidenceItemRequests = fetchRequests.filter((request) => {
  const url = new URL(request.url);
  return url.pathname === "/api/production-evidence/items";
});
const productionEvidenceRequests = fetchRequests.filter((request) => {
  const url = new URL(request.url);
  return url.pathname === "/api/production-evidence";
});

if (!indexSource.includes('id="runDesktopQueue"')) {
  throw new Error(`${indexPath} is missing the desktop run-next button`);
}
if (!indexSource.includes('id="hermesSessionStream"')) {
  throw new Error(`${indexPath} is missing the Hermes session stream panel`);
}
if (!indexSource.includes('id="agentConformancePackage"')) {
  throw new Error(`${indexPath} is missing the Agent/Hermes conformance package button`);
}
if (!indexSource.includes('id="integrationConformancePackage"')) {
  throw new Error(`${indexPath} is missing the integration conformance package button`);
}
if (!indexSource.includes('id="providerGatewayWorkerPanel"')) {
  throw new Error(`${indexPath} is missing the provider gateway worker panel`);
}
if (!indexSource.includes('id="integrationReadinessPanel"')) {
  throw new Error(`${indexPath} is missing the integration readiness panel`);
}
if (!indexSource.includes('id="runtimeDiscoveryPanel"')) {
  throw new Error(`${indexPath} is missing the runtime discovery panel`);
}
if (!indexSource.includes('id="prdReadinessPanel"')) {
  throw new Error(`${indexPath} is missing the PRD readiness panel`);
}
if (
  !indexSource.includes('id="productionEvidenceBundle"') ||
  !indexSource.includes('id="productionEvidenceTaskSelect"') ||
  !indexSource.includes('id="loadProductionEvidenceTemplate"') ||
  !indexSource.includes('id="loadProductionEvidenceItemTemplate"') ||
  !indexSource.includes('id="claimProductionEvidenceTask"') ||
  !indexSource.includes('id="loadProductionEvidenceLedgerBundle"') ||
  !indexSource.includes('id="createProductionEvidenceHandoffPackage"') ||
  !indexSource.includes('id="createProductionEvidenceRunPlan"') ||
  !indexSource.includes('id="mergeProductionEvidence"') ||
  !indexSource.includes('id="validateProductionEvidence"') ||
  !indexSource.includes('id="validateProductionEvidenceItem"') ||
  !indexSource.includes('id="importProductionEvidence"') ||
  !indexSource.includes('id="submitProductionEvidenceItem"')
) {
  throw new Error(`${indexPath} is missing the production evidence import controls`);
}
if (!appSource.includes("runtimeDesktopRecognitionRunNextUrl") || !appSource.includes("runNextDesktopRecognitionRequest")) {
  throw new Error(`${appPath} is missing the desktop run-next runtime handler`);
}
if (!appSource.includes("runtimeProductionEvidenceBundleFromLedgerUrl") || !appSource.includes("loadProductionEvidenceLedgerBundle")) {
  throw new Error(`${appPath} is missing the production evidence ledger bundle handler`);
}
if (!appSource.includes("runtimeProductionEvidenceRunPlanUrl") || !appSource.includes("createProductionEvidenceRunPlan")) {
  throw new Error(`${appPath} is missing the production evidence run-plan handler`);
}
if (!appSource.includes("runtimeProviderGatewayWorkerUrl") || !appSource.includes("renderProviderGatewayWorkerPanel")) {
  throw new Error(`${appPath} is missing the provider gateway worker runtime handler`);
}
if (
  !appSource.includes("runtimeIntegrationReadinessUrl") ||
  !appSource.includes("renderIntegrationReadiness") ||
  !appSource.includes("renderIntegrationReadinessLanes") ||
  !appSource.includes("renderIntegrationRunPlan")
) {
  throw new Error(`${appPath} is missing the integration readiness runtime handler`);
}
if (!appSource.includes("runtimeProviderConformancePackagesUrl") || !appSource.includes("exportProviderConformancePackage")) {
  throw new Error(`${appPath} is missing the provider conformance package runtime handler`);
}
if (!appSource.includes("runtimeAgentConformancePackagesUrl") || !appSource.includes("exportAgentConformancePackage")) {
  throw new Error(`${appPath} is missing the Agent/Hermes conformance package runtime handler`);
}
if (!appSource.includes("runtimeIntegrationConformancePackagesUrl") || !appSource.includes("exportIntegrationConformancePackage")) {
  throw new Error(`${appPath} is missing the integration conformance package runtime handler`);
}
if (!appSource.includes("runtimeDiscoveryUrl") || !appSource.includes("renderRuntimeDiscoveryPanel")) {
  throw new Error(`${appPath} is missing the runtime discovery handler`);
}
if (!appSource.includes("runtimeEventsWebSocketUrl") || !appSource.includes("startRuntimeEventWebSocket")) {
  throw new Error(`${appPath} is missing the runtime WebSocket event stream handler`);
}
if (!appSource.includes("runtimeExecutionPlanRunNextUrl") || !appSource.includes("runRuntimeExecutionPlanNext")) {
  throw new Error(`${appPath} is missing the runtime execution plan run-next handler`);
}
if (!appSource.includes("runtimeApiKeysUrl") || !appSource.includes("normalizeApiKeyAudit")) {
  throw new Error(`${appPath} is missing the runtime API key audit handler`);
}
if (!appSource.includes("runtimeOutputPackagesUrl") || !appSource.includes("mergeOutputPackages")) {
  throw new Error(`${appPath} is missing the runtime output package catalog handler`);
}
if (
  !appSource.includes("runtimeProductionEvidenceTemplateUrl") ||
  !appSource.includes("runtimeProductionEvidenceTasksUrl") ||
  !appSource.includes("runtimeProductionEvidenceHandoffPackageUrl") ||
  !appSource.includes("runtimeProductionEvidenceRunPlanUrl") ||
  !appSource.includes("runtimeProductionEvidenceItemTemplateUrl") ||
  !appSource.includes("runtimeProductionEvidenceTaskClaimUrl") ||
  !appSource.includes("runtimeProductionEvidenceItemsValidateUrl") ||
  !appSource.includes("runtimeProductionEvidenceItemsUrl") ||
  !appSource.includes("loadProductionEvidenceTemplate") ||
  !appSource.includes("loadProductionEvidenceItemTemplate") ||
  !appSource.includes("createProductionEvidenceHandoffPackage") ||
  !appSource.includes("createProductionEvidenceRunPlan") ||
  !appSource.includes("claimProductionEvidenceTask") ||
  !appSource.includes("runtimeProductionEvidenceMergeUrl") ||
  !appSource.includes("mergeProductionEvidence") ||
  !appSource.includes("runtimeProductionEvidenceValidateUrl") ||
  !appSource.includes("validateProductionEvidence") ||
  !appSource.includes("validateProductionEvidenceItem") ||
  !appSource.includes("runtimeProductionEvidenceUrl") ||
  !appSource.includes("importProductionEvidence") ||
  !appSource.includes("submitProductionEvidenceItem")
) {
  throw new Error(`${appPath} is missing the production evidence validation/import handler`);
}
if (!appSource.includes("renderHermesSessionStream")) {
  throw new Error(`${appPath} is missing the Hermes session stream renderer`);
}
if (!appSource.includes("runtimeAgentTranscriptUrl") || !appSource.includes("loadAgentTranscript")) {
  throw new Error(`${appPath} is missing the Hermes transcript loader`);
}
if (!appSource.includes("runtimeAgentStreamUrl") || !appSource.includes("startAgentSessionStream")) {
  throw new Error(`${appPath} is missing the Hermes transcript stream loader`);
}
if (!appSource.includes("runtimeAgentWebSocketUrl") || !appSource.includes("startAgentSessionWebSocket")) {
  throw new Error(`${appPath} is missing the Hermes transcript WebSocket stream loader`);
}

if (parsed.selectedNode !== "three") {
  throw new Error(`expected selected node three, got ${parsed.selectedNode}`);
}
if (parsed.selectedNodeContextStatus !== "loaded") {
  throw new Error(`expected loaded node context, got ${parsed.selectedNodeContextStatus}`);
}
if (parsed.selectedNodeContextId !== "three") {
  throw new Error(`expected node context for three, got ${parsed.selectedNodeContextId}`);
}
if (parsed.workflowContextStatus !== "loaded") {
  throw new Error(`expected loaded workflow context, got ${parsed.workflowContextStatus}`);
}
if (parsed.workflowContextId !== "workflow-demo") {
  throw new Error(`expected workflow context for workflow-demo, got ${parsed.workflowContextId}`);
}
if (parsed.runbookStatus !== "runtime" || parsed.runbookCount < 1) {
  throw new Error(`expected runtime runbooks, got ${parsed.runbookStatus}/${parsed.runbookCount}`);
}
if (!nodeContextCalls.some((url) => url.includes("node_id=three"))) {
  throw new Error(`missing /api/node-context fetch for three: ${nodeContextCalls.join(", ")}`);
}
if (!workflowContextCalls.some((url) => url.includes("workflow_id=workflow-demo"))) {
  throw new Error(`missing /api/workflow-context fetch for workflow-demo: ${workflowContextCalls.join(", ")}`);
}
if (!runtimeBudgetCalls.some((url) => url.includes("project=demo"))) {
  throw new Error(`missing /api/runtime-budget fetch: ${fetchCalls.join(", ")}`);
}
if (!runtimeApiKeyCalls.some((url) => url.includes("project=demo") && url.includes("rotation_days=90"))) {
  throw new Error(`missing /api/api-keys audit fetch: ${fetchCalls.join(", ")}`);
}
if (!parsed.budgetHtml.includes("Credential audit") || !parsed.budgetHtml.includes("需轮换") || !parsed.budgetHtml.includes("generation-td")) {
  throw new Error(`runtime budget panel did not render API key audit: ${parsed.budgetHtml}`);
}
if (!runtimePreflightCalls.some((url) => url.includes("project=demo"))) {
  throw new Error(`missing /api/runtime-preflight fetch: ${fetchCalls.join(", ")}`);
}
if (!runtimeExecutionPlanCalls.some((url) => url.includes("project=demo"))) {
  throw new Error(`missing /api/runtime-execution-plan fetch: ${fetchCalls.join(", ")}`);
}
if (!runtimeHandoffCalls.some((url) => url.includes("project=demo"))) {
  throw new Error(`missing /api/runtime-handoff fetch: ${fetchCalls.join(", ")}`);
}
if (!prdReadinessCalls.some((url) => url.includes("project=demo"))) {
  throw new Error(`missing /api/prd-readiness fetch: ${fetchCalls.join(", ")}`);
}
if (!prdCompletionGateCalls.some((url) => url.includes("project=demo"))) {
  throw new Error(`missing /api/prd-completion-gate fetch: ${fetchCalls.join(", ")}`);
}
if (!productionEvidenceRequirementsCalls.some((url) => url.includes("project=demo"))) {
  throw new Error(`missing /api/production-evidence/requirements fetch: ${fetchCalls.join(", ")}`);
}
if (!productionEvidenceTaskCalls.some((url) => url.includes("project=demo"))) {
  throw new Error(`missing /api/production-evidence/tasks fetch: ${fetchCalls.join(", ")}`);
}
if (!productionEvidenceHandoffCalls.some((url) => url.includes("project=demo"))) {
  throw new Error(`missing /api/production-evidence/handoff fetch: ${fetchCalls.join(", ")}`);
}
if (
  !parsed.productionEvidenceHtml.includes("生产证据缺口 21") ||
  !parsed.productionEvidenceHtml.includes("task queue 21") ||
  !parsed.productionEvidenceHtml.includes("生产证据交付包 21 tasks") ||
  !parsed.productionEvidenceHtml.includes("9 provider upstream") ||
  !parsed.productionEvidenceHtml.includes("11 software evidence") ||
  !parsed.productionEvidenceHtml.includes("external_visual_model") ||
  !parsed.productionEvidenceHtml.includes("providers[]") ||
  !parsed.productionEvidenceHtml.includes("software_actions[]") ||
  !parsed.productionEvidenceHtml.includes("provider handoff") ||
  !parsed.productionEvidenceHtml.includes("POOL_MEDIA_GATEWAY_UPSTREAM_ENDPOINT") ||
  !parsed.productionEvidenceHtml.includes("provider-gateway-worker") ||
  !parsed.productionEvidenceHtml.includes("bridge worker") ||
  !parsed.productionEvidenceHtml.includes("bridge handoff") ||
  !parsed.productionEvidenceHtml.includes("POOL_RESOLVE_ENDPOINT=http://127.0.0.1:<port>") ||
  !parsed.productionEvidenceHtml.includes("POOL_RESOLVE_UPSTREAM_ENDPOINT") ||
  !parsed.productionEvidenceHtml.includes("software-api-bridge-worker resolve") ||
  !parsed.productionEvidenceHtml.includes("desktop_vision[]") ||
  !parsed.productionEvidenceHtml.includes("merge-production-evidence")
) {
  throw new Error(`production evidence requirements did not render, got ${parsed.productionEvidenceHtml}`);
}
if (!outputPackageCalls.some((url) => url.includes("project=demo"))) {
  throw new Error(`missing /api/output-packages fetch: ${fetchCalls.join(", ")}`);
}
if (!runtimeDiscoveryCalls.some((url) => url.includes("project=demo"))) {
  throw new Error(`missing /api/discovery fetch: ${fetchCalls.join(", ")}`);
}
if (!adapterCalls.length) {
  throw new Error(`missing /api/adapters fetch: ${fetchCalls.join(", ")}`);
}
if (!integrationReadinessCalls.some((url) => url.includes("project=demo"))) {
  throw new Error(`missing /api/integration-readiness fetch: ${fetchCalls.join(", ")}`);
}
if (
  parsed.integrationReadinessSummary !== "2/6 ready" ||
  !parsed.integrationReadinessHtml.includes("World Labs Marble") ||
  !parsed.integrationReadinessHtml.includes("Unreal") ||
  !parsed.integrationReadinessHtml.includes("制片 / Agent 编排") ||
  !parsed.integrationReadinessHtml.includes("执行 Provider smoke") ||
  !parsed.integrationReadinessHtml.includes("配置 Provider Key") ||
  !parsed.integrationReadinessHtml.includes("spatial_engine") ||
  !parsed.integrationReadinessHtml.includes("integration-conformance-package")
) {
  throw new Error(`integration readiness panel did not render, got ${parsed.integrationReadinessSummary} / ${parsed.integrationReadinessHtml}`);
}
if (!providerContractCalls.length) {
  throw new Error(`missing /api/provider-contracts fetch: ${fetchCalls.join(", ")}`);
}
if (!providerGatewayWorkerCalls.length) {
  throw new Error(`missing /api/provider-gateway-worker fetch: ${fetchCalls.join(", ")}`);
}
if (!softwareContractCalls.length) {
  throw new Error(`missing /api/software-contracts fetch: ${fetchCalls.join(", ")}`);
}
if (!desktopContractCalls.length) {
  throw new Error(`missing /api/desktop-recognition/contract fetch: ${fetchCalls.join(", ")}`);
}
if (
  !parsed.providerHtml.includes("three_dgs_http_gateway") ||
  !parsed.providerHtml.includes("image-blaster-indexed-files") ||
  !parsed.providerHtml.includes("导出验收包")
) {
  throw new Error(`expected provider contract summary in Provider cards, got ${parsed.providerHtml}`);
}
if (
  !parsed.providerGatewayWorkerHtml.includes("pool-provider-gateway-worker") ||
  !parsed.providerGatewayWorkerHtml.includes("POOL_3DGS_GATEWAY_ENDPOINT") ||
  !parsed.providerGatewayWorkerHtml.includes("pool_provider_gateway_worker") ||
  !parsed.providerGatewayWorkerHtml.includes("three_dgs_smoke") ||
  !parsed.providerGatewayWorkerHtml.includes("production-evidence-provider-matrix") ||
  !parsed.providerGatewayWorkerHtml.includes("local files")
) {
  throw new Error(`expected provider gateway worker contract summary, got ${parsed.providerGatewayWorkerHtml}`);
}
if (
  !parsed.runtimeDiscoveryHtml.includes("pool-runtime") ||
  !parsed.runtimeDiscoveryHtml.includes("/api/runtime-execution-plan") ||
  !parsed.runtimeDiscoveryHtml.includes("/api/events/ws") ||
  !parsed.runtimeDiscoveryHtml.includes("/api/agent-sessions/ws") ||
  !parsed.runtimeDiscoveryHtml.includes("4 MCP resources") ||
  !parsed.runtimeDiscoveryHtml.includes("9 MCP tools") ||
  !parsed.runtimeDiscoveryHtml.includes("pool_integration_readiness") ||
  !parsed.runtimeDiscoveryHtml.includes("pool_worker_self_checks") ||
  !parsed.runtimeDiscoveryHtml.includes("pool_handoff_package") ||
  !parsed.runtimeDiscoveryHtml.includes("pool_provider_conformance_package") ||
  !parsed.runtimeDiscoveryHtml.includes("pool_integration_conformance_package") ||
  !parsed.runtimeDiscoveryHtml.includes("pool_software_conformance_package") ||
  !parsed.runtimeDiscoveryHtml.includes("pool_agent_conformance_package") ||
  !parsed.runtimeDiscoveryHtml.includes("pool-cli --project demo serve-mcp")
) {
  throw new Error(`expected runtime discovery summary, got ${parsed.runtimeDiscoveryHtml}`);
}
if (!webSocketUrls.some((url) => url.startsWith("ws://runtime.test/api/events/ws") && url.includes("project=demo"))) {
  throw new Error(`expected runtime WebSocket event stream: ${webSocketUrls.join(", ")}`);
}
if (!parsed.eventHtml.includes("runtime websocket event")) {
  throw new Error(`expected WebSocket runtime event in event stream UI, got ${parsed.eventHtml}`);
}
if (
  !handoffPackageEventHtml.includes("Runtime 接管包已生成：9 个本地文件已入库") ||
  !handoffPackageEventHtml.includes("8-runtime-handoff-package-manifest.json") ||
  !handoffPackageEventHtml.includes("5-worker-self-checks.sh")
) {
  throw new Error(`expected handoff package worker smoke event, got ${handoffPackageEventHtml}`);
}
if (
  !parsed.softwareHtml.includes("POST /api/software-actions") ||
  !parsed.softwareHtml.includes("api/mcp / skills/cli / desktop-recognition") ||
  !parsed.softwareHtml.includes("generic_software_api_mcp") ||
  !parsed.softwareHtml.includes("POOL_RESOLVE_ENDPOINT=http://127.0.0.1:8793") ||
  !parsed.softwareHtml.includes("software-api-bridge-worker resolve") ||
  !parsed.softwareHtml.includes("production_matrix") ||
  !parsed.softwareHtml.includes("local file artifacts") ||
  !parsed.softwareHtml.includes("导出验收包")
) {
  throw new Error(`expected software control contract summary, got ${parsed.softwareHtml}`);
}
if (!softwareConformancePackageRequests.length) {
  throw new Error("expected /api/software-conformance-packages request");
}
if (!softwareConformancePackageEventHtml.includes("软件验收包已导出")) {
  throw new Error(`expected software conformance package event, got ${softwareConformancePackageEventHtml}`);
}
if (!providerConformancePackageRequests.length) {
  throw new Error("expected /api/provider-conformance-packages request");
}
if (!providerConformancePackageEventHtml.includes("Provider 验收包已导出")) {
  throw new Error(`expected provider conformance package event, got ${providerConformancePackageEventHtml}`);
}
if (!agentConformancePackageRequests.length) {
  throw new Error("expected /api/agent-conformance-packages request");
}
if (!agentConformancePackageEventHtml.includes("Agent/Hermes 验收包已导出")) {
  throw new Error(`expected Agent/Hermes conformance package event, got ${agentConformancePackageEventHtml}`);
}
if (!integrationConformancePackageRequests.length) {
  throw new Error("expected /api/integration-conformance-packages request");
}
if (!integrationConformancePackageEventHtml.includes("总验收包已导出")) {
  throw new Error(`expected integration conformance package event, got ${integrationConformancePackageEventHtml}`);
}
if (!promptCalls.length) {
  throw new Error(`expected runtime prompts fetch: ${fetchCalls.join(", ")}`);
}
if (!promptCalls.some((url) => url.includes("name=pool_software_handoff"))) {
  throw new Error(`expected runtime prompt get: ${promptCalls.join(", ")}`);
}
if (!transcriptCalls.some((url) => url.includes("session_id=agent-session-cli"))) {
  throw new Error(`expected agent transcript fetch: ${transcriptCalls.join(", ")}`);
}
if (!webSocketUrls.some((url) => url.startsWith("ws://runtime.test/api/agent-sessions/ws") && url.includes("session_id=agent-session-cli"))) {
  throw new Error(`expected agent session WebSocket stream: ${webSocketUrls.join(", ")}`);
}
if (!handoffPackageRequests.length) {
  throw new Error("expected /api/handoff-packages request");
}
const handoffPackageBody = JSON.parse(handoffPackageRequests[0].options.body);
if (handoffPackageBody.project_slug !== "demo" || handoffPackageBody.node_id !== "agent") {
  throw new Error(`unexpected handoff package request body: ${handoffPackageRequests[0].options.body}`);
}
if (handoffPackageBody.output_dir !== "worlds/demo/output" || handoffPackageBody.include_snapshot !== true) {
  throw new Error(`handoff package request did not ask for local snapshot: ${handoffPackageRequests[0].options.body}`);
}
if (!prdCompletionPackageRequests.length) {
  throw new Error("expected /api/prd-completion-package request");
}
const prdCompletionPackageBody = JSON.parse(prdCompletionPackageRequests[0].options.body);
if (
  prdCompletionPackageBody.project_slug !== "demo" ||
  prdCompletionPackageBody.node_id !== "agent" ||
  prdCompletionPackageBody.output_dir !== "worlds/demo/output" ||
  prdCompletionPackageBody.include_snapshot !== true
) {
  throw new Error(`unexpected PRD completion package request body: ${prdCompletionPackageRequests[0].options.body}`);
}
if (
  prdCompletionPackageResult.prdSummary !== "partial" ||
  !prdCompletionPackageResult.prdHtml.includes("4-prd-completion-package-manifest.json") ||
  !prdCompletionPackageResult.prdHtml.includes("6 files") ||
  !prdCompletionPackageResult.prdHtml.includes("incomplete") ||
  !prdCompletionPackageResult.eventHtmlAfterPrdCompletionPackage.includes("PRD 完成证明包已写出")
) {
  throw new Error(`expected PRD completion package UI, got ${JSON.stringify(prdCompletionPackageResult)}`);
}
if (!runtimeRunNextRequests.length) {
  throw new Error("expected /api/runtime-execution-plan/run-next request");
}
const runtimeRunNextBody = JSON.parse(runtimeRunNextRequests[0].options.body);
if (runtimeRunNextBody.project_slug !== "demo" || runtimeRunNextBody.execute !== false) {
  throw new Error(`unexpected runtime execution plan run-next body: ${runtimeRunNextRequests[0].options.body}`);
}
if (!desktopRunNextRequests.length) {
  throw new Error("expected /api/desktop-recognition/run-next request");
}
const desktopRunNextBody = JSON.parse(desktopRunNextRequests[0].options.body);
if (desktopRunNextBody.controller_id !== "web-prototype-dry-run" || desktopRunNextBody.limit !== 1) {
  throw new Error(`unexpected desktop run-next request body: ${desktopRunNextRequests[0].options.body}`);
}
if (!outputPackageResultRequests.length) {
  throw new Error("expected /api/output-packages/results request");
}
const outputPackageResultBody = JSON.parse(outputPackageResultRequests[0].options.body);
if (
  outputPackageResultBody.target !== "game" ||
  outputPackageResultBody.status !== "succeeded" ||
  outputPackageResultBody.adapter_id !== "unreal" ||
  outputPackageResultBody.metrics?.[0]?.label !== "confirmed_by"
) {
  throw new Error(`unexpected output package result request body: ${outputPackageResultRequests[0].options.body}`);
}
if (!productionEvidenceValidationRequests.length) {
  throw new Error("expected /api/production-evidence/validate request");
}
if (!productionEvidenceHandoffPackageRequests.length) {
  throw new Error("expected /api/production-evidence/handoff-packages request");
}
const productionEvidenceHandoffPackageBody = JSON.parse(productionEvidenceHandoffPackageRequests[0].options.body);
if (
  productionEvidenceHandoffPackageBody.project_slug !== "demo" ||
  productionEvidenceHandoffPackageBody.node_id !== "agent" ||
  productionEvidenceHandoffPackageBody.output_dir !== "worlds/demo/output" ||
  productionEvidenceHandoffPackageBody.output_root !== "worlds/demo/output/production-evidence" ||
  productionEvidenceHandoffPackageBody.include_items !== true ||
  productionEvidenceHandoffPackageBody.include_snapshot !== true
) {
  throw new Error(`unexpected production evidence handoff package request body: ${productionEvidenceHandoffPackageRequests[0].options.body}`);
}
if (
  productionEvidenceHandoffPackageResult.productionEvidenceSummary !== "21 package" ||
  !productionEvidenceHandoffPackageResult.productionEvidenceHtml.includes("21 item files / 9 local files") ||
  !productionEvidenceHandoffPackageResult.productionEvidenceHtml.includes("6-production-evidence-package-manifest.json") ||
  !productionEvidenceHandoffPackageResult.productionEvidenceHtml.includes("4-production-evidence-run-plan.json") ||
  !productionEvidenceHandoffPackageResult.productionEvidenceHtml.includes("7-production-evidence-runner.sh") ||
  !productionEvidenceHandoffPackageResult.productionEvidenceHtml.includes("8-production-evidence-runner-preflight.json") ||
  !productionEvidenceHandoffPackageResult.productionEvidenceHtml.includes("5-production-evidence-bundle.json") ||
  !productionEvidenceHandoffPackageResult.productionEvidenceHtml.includes("bridge item") ||
  !productionEvidenceHandoffPackageResult.productionEvidenceHtml.includes("provider start") ||
  !productionEvidenceHandoffPackageResult.productionEvidenceHtml.includes("POOL_3DGS_GATEWAY_UPSTREAM_ENDPOINT") ||
  !productionEvidenceHandoffPackageResult.productionEvidenceHtml.includes("provider-gateway-worker") ||
  !productionEvidenceHandoffPackageResult.productionEvidenceHtml.includes("bridge start") ||
  !productionEvidenceHandoffPackageResult.productionEvidenceHtml.includes("software:resolve:production_software") ||
  !productionEvidenceHandoffPackageResult.productionEvidenceHtml.includes("POOL_RESOLVE_ENDPOINT") ||
  !productionEvidenceHandoffPackageResult.productionEvidenceHtml.includes("POOL_RESOLVE_UPSTREAM_ENDPOINT") ||
  !productionEvidenceHandoffPackageResult.productionEvidenceHtml.includes("software-api-bridge-worker resolve") ||
  !productionEvidenceHandoffPackageResult.eventHtmlAfterProductionEvidencePackage.includes("生产证据交付包已写出")
) {
  throw new Error(`expected production evidence handoff package UI, got ${JSON.stringify(productionEvidenceHandoffPackageResult)}`);
}
if (
  !productionEvidenceRunPlanRequests.length ||
  productionEvidenceRunPlanResult.productionEvidenceSummary !== "7 plan" ||
  !productionEvidenceRunPlanResult.productionEvidenceHtml.includes("21 missing") ||
  !productionEvidenceRunPlanResult.productionEvidenceHtml.includes("7 execution phases") ||
  !productionEvidenceRunPlanResult.productionEvidenceHtml.includes("9 provider / 11 software / 1 vision tasks") ||
  !productionEvidenceRunPlanResult.productionEvidenceHtml.includes("provider start") ||
  !productionEvidenceRunPlanResult.productionEvidenceHtml.includes("POOL_3DGS_GATEWAY_UPSTREAM_ENDPOINT") ||
  !productionEvidenceRunPlanResult.productionEvidenceHtml.includes("provider-gateway-worker") ||
  !productionEvidenceRunPlanResult.productionEvidenceHtml.includes("software bridge") ||
  !productionEvidenceRunPlanResult.productionEvidenceHtml.includes("bridge start") ||
  !productionEvidenceRunPlanResult.productionEvidenceHtml.includes("POOL_<ADAPTER>_ENDPOINT=http://127.0.0.1:<port>") ||
  !productionEvidenceRunPlanResult.productionEvidenceHtml.includes("POOL_RESOLVE_UPSTREAM_ENDPOINT") ||
  !productionEvidenceRunPlanResult.productionEvidenceHtml.includes("software-api-bridge-worker <adapter-id>") ||
  !productionEvidenceRunPlanResult.productionEvidenceHtml.includes("software-api-bridge-worker resolve") ||
  !productionEvidenceRunPlanResult.productionEvidenceHtml.includes("production-evidence-provider-matrix") ||
  !productionEvidenceRunPlanResult.productionEvidenceHtml.includes("closeout-production-evidence --output") ||
  !productionEvidenceRunPlanResult.eventHtmlAfterProductionEvidenceRunPlan.includes("生产证据运行计划已生成")
) {
  throw new Error(`expected production evidence run-plan UI, got ${JSON.stringify(productionEvidenceRunPlanResult)}`);
}
const runPlanUrl = new URL(productionEvidenceRunPlanRequests[0].url);
if (
  runPlanUrl.pathname !== "/api/production-evidence/run-plan" ||
  runPlanUrl.searchParams.get("project") !== "demo" ||
  runPlanUrl.searchParams.get("source") !== "web-production-evidence-run-plan" ||
  runPlanUrl.searchParams.get("output_root") !== "worlds/demo/output/production-evidence"
) {
  throw new Error(`unexpected production evidence run-plan request: ${productionEvidenceRunPlanRequests[0].url}`);
}
if (
  !productionEvidenceLedgerBundleRequests.length ||
  productionEvidenceLedgerBundleResult.productionEvidenceSummary !== "21 ledger" ||
  !productionEvidenceLedgerBundleResult.productionEvidenceHtml.includes("9 providers / 11 software / 1 vision") ||
  !productionEvidenceLedgerBundleResult.productionEvidenceHtml.includes("21 ready") ||
  !productionEvidenceLedgerBundleResult.productionEvidenceHtml.includes("2 incomplete") ||
  !productionEvidenceLedgerBundleResult.productionEvidenceBundle.includes('"source": "web-production-evidence-ledger"') ||
  !productionEvidenceLedgerBundleResult.eventHtmlAfterLedgerBundle.includes("已从 Runtime 账本收口生产证据")
) {
  throw new Error(`expected production evidence ledger bundle UI, got ${JSON.stringify(productionEvidenceLedgerBundleResult)}`);
}
const ledgerBundleUrl = new URL(productionEvidenceLedgerBundleRequests[0].url);
if (
  ledgerBundleUrl.pathname !== "/api/production-evidence/bundle-from-ledger" ||
  ledgerBundleUrl.searchParams.get("project") !== "demo" ||
  ledgerBundleUrl.searchParams.get("source") !== "web-production-evidence-ledger" ||
  ledgerBundleUrl.searchParams.get("include_incomplete") !== "true"
) {
  throw new Error(`unexpected production evidence ledger bundle request: ${productionEvidenceLedgerBundleRequests[0].url}`);
}
if (!productionEvidenceTaskClaimRequests.length) {
  throw new Error("expected /api/production-evidence/tasks/claim request");
}
const productionEvidenceTaskClaimBody = JSON.parse(productionEvidenceTaskClaimRequests[0].options.body);
if (
  productionEvidenceTaskClaimBody.project_slug !== "demo" ||
  productionEvidenceTaskClaimBody.task_id !== "provider:midjourney:production_upstream" ||
  productionEvidenceTaskClaimBody.assignee !== "web-operator" ||
  productionEvidenceTaskClaimBody.role !== "provider_worker" ||
  productionEvidenceTaskClaimBody.output_root !== "worlds/demo/output/production-evidence" ||
  productionEvidenceTaskClaimBody.source !== "web-production-evidence-task-claim"
) {
  throw new Error(`unexpected production evidence task claim request body: ${productionEvidenceTaskClaimRequests[0].options.body}`);
}
if (
  productionEvidenceTaskClaimResult.productionEvidenceSummary !== "1 claim" ||
  !productionEvidenceTaskClaimResult.productionEvidenceHtml.includes("provider · midjourney") ||
  !productionEvidenceTaskClaimResult.productionEvidenceHtml.includes("provider-midjourney-production-upstream-claim.json") ||
  !productionEvidenceTaskClaimResult.productionEvidenceHtml.includes("validate-production-evidence-item") ||
  !productionEvidenceTaskClaimResult.eventHtmlAfterProductionEvidenceTaskClaim.includes("生产证据任务已领取")
) {
  throw new Error(`expected production evidence task claim UI, got ${JSON.stringify(productionEvidenceTaskClaimResult)}`);
}
if (
  !productionEvidenceItemTemplateRequests.length ||
  runtimeItemTemplateResult.productionEvidenceSummary !== "1 item" ||
  !runtimeItemTemplateResult.productionEvidenceHtml.includes("provider · midjourney") ||
  !runtimeItemTemplateResult.productionEvidenceHtml.includes("submit-production-evidence-item") ||
  !runtimeItemTemplateResult.productionEvidenceBundle.includes('"kind": "provider"') ||
  !runtimeItemTemplateResult.productionEvidenceBundle.includes('"provider_id": "midjourney"') ||
  !runtimeItemTemplateResult.eventHtmlAfterItemTemplateLoad.includes("已从 Runtime 生成单项生产证据")
) {
  throw new Error(`expected runtime production evidence item template UI, got ${JSON.stringify(runtimeItemTemplateResult)}`);
}
const itemTemplateUrl = new URL(productionEvidenceItemTemplateRequests[0].url);
if (
  itemTemplateUrl.pathname !== "/api/production-evidence/item-template" ||
  itemTemplateUrl.searchParams.get("project") !== "demo" ||
  itemTemplateUrl.searchParams.get("task_id") !== "provider:midjourney:production_upstream"
) {
  throw new Error(`unexpected production evidence item template request: ${productionEvidenceItemTemplateRequests[0].url}`);
}
if (!productionEvidenceItemValidationRequests.length) {
  throw new Error("expected /api/production-evidence/items/validate request");
}
const productionEvidenceItemValidationBody = JSON.parse(productionEvidenceItemValidationRequests[0].options.body);
if (
  productionEvidenceItemValidationBody.project_slug !== "demo" ||
  productionEvidenceItemValidationBody.kind !== "provider" ||
  productionEvidenceItemValidationBody.provider?.provider_id !== "midjourney" ||
  productionEvidenceItemValidationBody.provider?.external_job_id !== "real-midjourney-job-id" ||
  productionEvidenceItemValidationBody.provider?.evidence_json?.task_id !== "provider:midjourney:production_upstream"
) {
  throw new Error(`unexpected production evidence item validation body: ${productionEvidenceItemValidationRequests[0].options.body}`);
}
if (
  productionItemValidationResult.productionEvidenceSummary !== "1 item dry-run" ||
  !productionItemValidationResult.productionEvidenceHtml.includes("1 providers / 0 software / 0 vision") ||
  !productionItemValidationResult.productionEvidenceHtml.includes("writes 0") ||
  !productionItemValidationResult.productionEvidenceHtml.includes("validate-production-evidence") ||
  !productionItemValidationResult.eventHtmlAfterItemValidation.includes("单项生产证据预检通过")
) {
  throw new Error(`expected production evidence item validation UI, got ${JSON.stringify(productionItemValidationResult)}`);
}
if (!productionEvidenceItemRequests.length) {
  throw new Error("expected /api/production-evidence/items request");
}
const productionEvidenceItemBody = JSON.parse(productionEvidenceItemRequests[0].options.body);
if (
  productionEvidenceItemBody.project_slug !== "demo" ||
  productionEvidenceItemBody.kind !== "provider" ||
  productionEvidenceItemBody.provider?.provider_id !== "midjourney" ||
  productionEvidenceItemBody.provider?.external_job_id !== "real-midjourney-job-id" ||
  productionEvidenceItemBody.provider?.evidence_json?.task_id !== "provider:midjourney:production_upstream"
) {
  throw new Error(`unexpected production evidence item request body: ${productionEvidenceItemRequests[0].options.body}`);
}
if (
  productionItemResult.productionEvidenceSummary !== "1 imported" ||
  !productionItemResult.productionEvidenceHtml.includes("1 providers / 0 software / 0 vision") ||
  !productionItemResult.productionEvidenceHtml.includes("coverage 1/9 providers") ||
  !productionItemResult.eventHtmlAfterItemSubmit.includes("单项生产证据已提交")
) {
  throw new Error(`expected production evidence item submit UI, got ${JSON.stringify(productionItemResult)}`);
}
if (
  !productionEvidenceTemplateRequests.length ||
  runtimeTemplateResult.productionEvidenceSummary !== "21 template" ||
  !runtimeTemplateResult.productionEvidenceHtml.includes("生产证据缺口 21") ||
  !runtimeTemplateResult.productionEvidenceHtml.includes("9 providers / 11 software / 1 vision") ||
  !runtimeTemplateResult.productionEvidenceHtml.includes("pool-cli --project demo validate-production-evidence") ||
  !runtimeTemplateResult.productionEvidenceBundle.includes('"metadata_path"') ||
  !runtimeTemplateResult.eventHtmlAfterTemplateLoad.includes("已从 Runtime 生成生产证据脚手架")
) {
  throw new Error(`expected runtime production evidence template UI, got ${JSON.stringify(runtimeTemplateResult)}`);
}
const templateUrl = new URL(productionEvidenceTemplateRequests[0].url);
if (
  templateUrl.pathname !== "/api/production-evidence/template" ||
  templateUrl.searchParams.get("project") !== "demo" ||
  templateUrl.searchParams.get("missing_only") !== "true"
) {
  throw new Error(`unexpected production evidence template request: ${productionEvidenceTemplateRequests[0].url}`);
}
if (!productionEvidenceMergeRequests.length) {
  throw new Error("expected /api/production-evidence/merge request");
}
const productionEvidenceMergeBody = JSON.parse(productionEvidenceMergeRequests[0].options.body);
if (
  productionEvidenceMergeBody.project_slug !== "demo" ||
  productionEvidenceMergeBody.source !== "web-production-evidence-merge" ||
  productionEvidenceMergeBody.bundles?.length !== 1 ||
  productionEvidenceMergeBody.bundles?.[0]?.providers?.length !== 9 ||
  productionEvidenceMergeBody.bundles?.[0]?.software_actions?.length !== 11 ||
  productionEvidenceMergeBody.bundles?.[0]?.desktop_vision?.length !== 1
) {
  throw new Error(`unexpected production evidence merge request body: ${productionEvidenceMergeRequests[0].options.body}`);
}
if (
  mergeResult.productionEvidenceSummary !== "21 merged" ||
  !mergeResult.productionEvidenceHtml.includes("9 providers / 11 software / 1 vision") ||
  !mergeResult.productionEvidenceHtml.includes("1 input bundles") ||
  !mergeResult.productionEvidenceHtml.includes("merge writes 0") ||
  !mergeResult.productionEvidenceBundle.includes('"merge"') ||
  !mergeResult.productionEvidenceBundle.includes('"input_count": 1') ||
  !mergeResult.eventHtmlAfterMerge.includes("生产证据已合并")
) {
  throw new Error(`expected production evidence merge UI, got ${JSON.stringify(mergeResult)}`);
}
if (productionEvidenceCloseoutRequests.length !== 2) {
  throw new Error(`expected two /api/production-evidence/closeout requests, got ${productionEvidenceCloseoutRequests.length}`);
}
const productionEvidenceCloseoutBody = JSON.parse(productionEvidenceCloseoutRequests[0].options.body);
if (
  productionEvidenceCloseoutBody.project_slug !== "demo" ||
  productionEvidenceCloseoutBody.source !== "web-production-evidence-closeout" ||
  productionEvidenceCloseoutBody.import !== false ||
  productionEvidenceCloseoutBody.bundles?.length !== 1 ||
  productionEvidenceCloseoutBody.bundles?.[0]?.providers?.length !== 9 ||
  productionEvidenceCloseoutBody.bundles?.[0]?.software_actions?.length !== 11 ||
  productionEvidenceCloseoutBody.bundles?.[0]?.desktop_vision?.length !== 1
) {
  throw new Error(`unexpected production evidence closeout request body: ${productionEvidenceCloseoutRequests[0].options.body}`);
}
if (
  closeoutResult.productionEvidenceSummary !== "21 closeout" ||
  !closeoutResult.productionEvidenceHtml.includes("9 providers / 11 software / 1 vision") ||
  !closeoutResult.productionEvidenceHtml.includes("1 input bundles") ||
  !closeoutResult.productionEvidenceHtml.includes("closeout writes 0") ||
  !closeoutResult.productionEvidenceHtml.includes("coverage 9/9 providers") ||
  !closeoutResult.productionEvidenceBundle.includes('"source": "web-production-evidence-closeout"') ||
  !closeoutResult.eventHtmlAfterCloseout.includes("生产证据收口预检完成")
) {
  throw new Error(`expected production evidence closeout UI, got ${JSON.stringify(closeoutResult)}`);
}
const templateProductionEvidenceValidationBody = JSON.parse(productionEvidenceValidationRequests[0].options.body);
if (
  templateProductionEvidenceValidationBody.project_slug !== "demo" ||
  templateProductionEvidenceValidationBody.providers?.length !== 9 ||
  !templateProductionEvidenceValidationBody.providers?.[0]?.external_job_id?.startsWith("replace-with-real-") ||
  !templateValidationResult.eventHtmlAfterTemplateValidation.includes("生产证据校验失败") ||
  !templateValidationResult.eventHtmlAfterTemplateValidation.includes("template identifiers")
) {
  throw new Error(`expected template production evidence validation to fail, got ${JSON.stringify(templateValidationResult)}`);
}
const productionEvidenceValidationBody = JSON.parse(productionEvidenceValidationRequests.at(-1).options.body);
if (
  productionEvidenceValidationBody.project_slug !== "demo" ||
  productionEvidenceValidationBody.providers?.length !== 9 ||
  productionEvidenceValidationBody.providers?.[0]?.provider_id !== "midjourney" ||
  productionEvidenceValidationBody.providers?.[0]?.external_job_id !== "real-midjourney-job-001" ||
  productionEvidenceValidationBody.providers?.[4]?.provider_id !== "worldlabs-marble" ||
  productionEvidenceValidationBody.software_actions?.length !== 11 ||
  productionEvidenceValidationBody.software_actions?.[0]?.adapter_id !== "unreal" ||
  productionEvidenceValidationBody.software_actions?.[0]?.external_action_id !== "real-unreal-action-001" ||
  productionEvidenceValidationBody.software_actions?.[3]?.adapter_id !== "resolve" ||
  productionEvidenceValidationBody.desktop_vision?.[0]?.visual_model !== "external"
) {
  throw new Error(`unexpected production evidence validation body: ${productionEvidenceValidationRequests.at(-1).options.body}`);
}
if (
  validationResult.productionEvidenceSummary !== "21 valid" ||
  !validationResult.productionEvidenceHtml.includes("9 providers / 11 software / 1 vision") ||
  !validationResult.productionEvidenceHtml.includes("validate writes 0") ||
  !validationResult.productionEvidenceHtml.includes("coverage 9/9 providers") ||
  !validationResult.productionEvidenceHtml.includes("11/11 software") ||
  !validationResult.eventHtmlAfterValidation.includes("生产证据校验通过")
) {
  throw new Error(`expected production evidence validation UI, got ${JSON.stringify(validationResult)}`);
}
if (!productionEvidenceRequests.length) {
  throw new Error("expected /api/production-evidence request");
}
const productionEvidenceBody = JSON.parse(productionEvidenceRequests[0].options.body);
if (
  productionEvidenceBody.project_slug !== "demo" ||
  productionEvidenceBody.providers?.length !== 9 ||
  productionEvidenceBody.providers?.[0]?.provider_id !== "midjourney" ||
  productionEvidenceBody.providers?.[4]?.provider_id !== "worldlabs-marble" ||
  productionEvidenceBody.software_actions?.length !== 11 ||
  productionEvidenceBody.software_actions?.[0]?.adapter_id !== "unreal" ||
  productionEvidenceBody.software_actions?.[3]?.adapter_id !== "resolve" ||
  productionEvidenceBody.desktop_vision?.[0]?.visual_model !== "external"
) {
  throw new Error(`unexpected production evidence request body: ${productionEvidenceRequests[0].options.body}`);
}
if (
  productionResult.productionEvidenceSummary !== "21 imported" ||
  !productionResult.productionEvidenceHtml.includes("9 providers / 11 software / 1 vision") ||
  !productionResult.productionEvidenceHtml.includes("coverage 9/9 providers") ||
  !productionResult.productionEvidenceHtml.includes("11/11 software") ||
  productionResult.prdSummaryAfterProduction !== "ready" ||
  !productionResult.eventHtmlAfterProduction.includes("生产证据已导入")
) {
  throw new Error(`expected production evidence import UI, got ${JSON.stringify(productionResult)}`);
}
const productionEvidenceCloseoutImportBody = JSON.parse(productionEvidenceCloseoutRequests[1].options.body);
if (
  productionEvidenceCloseoutImportBody.project_slug !== "demo" ||
  productionEvidenceCloseoutImportBody.source !== "web-production-evidence-closeout" ||
  productionEvidenceCloseoutImportBody.import !== true ||
  productionEvidenceCloseoutImportBody.bundles?.[0]?.providers?.length !== 9 ||
  productionEvidenceCloseoutImportBody.bundles?.[0]?.software_actions?.length !== 11 ||
  productionEvidenceCloseoutImportBody.bundles?.[0]?.desktop_vision?.[0]?.visual_model !== "external"
) {
  throw new Error(`unexpected production evidence closeout import body: ${productionEvidenceCloseoutRequests[1].options.body}`);
}
if (
  closeoutImportResult.productionEvidenceSummary !== "21 imported" ||
  !closeoutImportResult.productionEvidenceHtml.includes("9 providers / 11 software / 1 vision") ||
  !closeoutImportResult.productionEvidenceHtml.includes("coverage 9/9 providers") ||
  !closeoutImportResult.productionEvidenceHtml.includes("PRD complete") ||
  !closeoutImportResult.productionEvidenceHtml.includes("prd-completion-package --output-dir worlds/demo/output --include-snapshot") ||
  closeoutImportResult.prdSummaryAfterCloseoutImport !== "ready" ||
  !closeoutImportResult.eventHtmlAfterCloseoutImport.includes("生产证据收口已导入")
) {
  throw new Error(`expected production evidence closeout import UI, got ${JSON.stringify(closeoutImportResult)}`);
}
if (parsed.desktopSummary !== "0 waiting" || !parsed.desktopRunDisabled) {
  throw new Error(`expected desktop queue to clear and disable run-next, got ${parsed.desktopSummary}/${parsed.desktopRunDisabled}`);
}
if (!parsed.desktopQueueHtml.includes("当前没有等待桌面识别接管的动作")) {
  throw new Error(`expected empty desktop queue UI, got ${parsed.desktopQueueHtml}`);
}
if (!parsed.desktopQueueHtml.includes("desktop-recognition-control-request") || !parsed.desktopQueueHtml.includes("POST /api/desktop-recognition/results")) {
  throw new Error(`expected desktop recognition contract summary, got ${parsed.desktopQueueHtml}`);
}
if (!parsed.hermesPrompt.includes("Adapter: blender")) {
  throw new Error(`expected runbook text in Hermes prompt, got ${parsed.hermesPrompt}`);
}
if (
  !parsed.hermesSessionHtml.includes("读取工作流上下文") ||
  !parsed.hermesSessionHtml.includes("1200/74000 tokens") ||
  !parsed.hermesSessionHtml.includes("worlds/demo/output/agent-cli-transcript.json") ||
  !parsed.hermesSessionHtml.includes("session-transcript") ||
  !parsed.hermesSessionHtml.includes("agent_cli") ||
  !parsed.hermesSessionHtml.includes("websocket_stream")
) {
  throw new Error(`expected Hermes session stream to show Agent CLI transcript, got ${parsed.hermesSessionHtml}`);
}
if (!parsed.cliHtml.includes("pool-cli --project demo workflow-context workflow-demo")) {
  throw new Error(`expected workflow-context CLI command in UI, got ${parsed.cliHtml}`);
}
if (!parsed.budgetSummary.includes("9.0k") || !parsed.budgetHtml.includes("World Labs Marble")) {
  throw new Error(`expected runtime budget panel, got ${parsed.budgetSummary} / ${parsed.budgetHtml}`);
}
if (
  !parsed.preflightSummary.includes("1 blocked") ||
  !parsed.preflightHtml.includes("approve-task task-three") ||
  !parsed.preflightHtml.includes("worker-self-checks")
) {
  throw new Error(`expected runtime preflight panel, got ${parsed.preflightSummary} / ${parsed.preflightHtml}`);
}
if (
  !parsed.handoffSummary.includes("4 cmds") ||
  !parsed.handoffSummary.includes("2 steps") ||
  !parsed.handoffHtml.includes("runtime-preflight") ||
  !parsed.handoffHtml.includes("approve-task task-three") ||
  !parsed.handoffHtml.includes("worker-self-checks") ||
  !parsed.handoffHtml.includes("最近接管包") ||
  !parsed.handoffHtml.includes("8-runtime-handoff-package-manifest.json") ||
  !parsed.handoffHtml.includes("7-integration-readiness.json") ||
  !parsed.handoffHtml.includes("creative_director") ||
  !parsed.handoffHtml.includes("pool-cli --project demo integration-readiness") ||
  !parsed.handoffHtml.includes("pool-cli --project demo serve-mcp") ||
  !parsed.handoffHtml.includes("pool-cli --project demo handoff-package") ||
  !parsed.handoffHtml.includes("provider_contract") ||
  !parsed.handoffHtml.includes("execution-step") ||
  !parsed.handoffHtml.includes("Creative Director") ||
  !parsed.handoffHtml.includes("Agent Operator") ||
  !parsed.handoffHtml.includes("human_approval") ||
  !parsed.handoffHtml.includes("预览下一步") ||
  !parsed.handoffHtml.includes("preview")
) {
  throw new Error(`expected runtime handoff panel, got ${parsed.handoffSummary} / ${parsed.handoffHtml}`);
}
if (
  parsed.prdSummary !== "partial" ||
  !parsed.prdHtml.includes("AI image/video/audio") ||
  !parsed.prdHtml.includes("PRD 完成门槛未满足") ||
  !parsed.prdHtml.includes("closeout-production-evidence") ||
  !parsed.prdHtml.includes("gate-merged.json") ||
  !parsed.prdHtml.includes("7") ||
  !parsed.prdHtml.includes("3") ||
  !parsed.prdHtml.includes("provider-gateway-worker")
) {
  throw new Error(`expected PRD readiness panel, got ${parsed.prdSummary} / ${parsed.prdHtml}`);
}
if (
  !parsed.outputManifestHtml.includes("时间线与转码") ||
  !parsed.outputManifestHtml.includes("运行原型") ||
  !parsed.outputManifestHtml.includes("节点与现场控制") ||
  !parsed.outputManifestHtml.includes("mp4 h264 1920x1080") ||
  !parsed.outputManifestHtml.includes("osc, midi, dmx") ||
  !parsed.outputManifestHtml.includes("execution: succeeded") ||
  !parsed.outputManifestHtml.includes("runtime_result: Unreal") ||
  !parsed.outputManifestHtml.includes("标记后段完成")
) {
  throw new Error(`expected runtime output package catalog in manifest panel, got ${parsed.outputManifestHtml}`);
}
const workflowContextAgentRequest = agentSessionRequests.find((request) => {
  const body = JSON.parse(request.options?.body ?? "{}");
  return body.command_id === "workflow-context";
});
if (!workflowContextAgentRequest) {
  throw new Error(`expected /api/agent-sessions request for workflow-context: ${agentSessionRequests.length}`);
}
const workflowContextAgentBody = JSON.parse(workflowContextAgentRequest.options.body);
if (workflowContextAgentBody.command !== "pool-cli --project demo workflow-context workflow-demo") {
  throw new Error(`unexpected Agent CLI command: ${workflowContextAgentBody.command}`);
}
if (!parsed.nodeLog.includes("Runtime 工作流上下文") || !parsed.nodeLog.includes("1 provider")) {
  throw new Error("node detail did not render runtime workflow context summary");
}
if (!parsed.nodeLog.includes("Runtime 节点上下文") || !parsed.nodeLog.includes("1 assets")) {
  throw new Error("node detail did not render runtime node context summary");
}
if (!parsed.nodeLog.includes("Runtime 控制入口") || !parsed.nodeLog.includes("pool_run_provider")) {
  throw new Error("node detail did not render runtime control context");
}
if (!parsed.nodeLog.includes('data-node-control-run="three"') || !parsed.nodeLog.includes("pool-cli --project demo run-provider")) {
  throw new Error("node detail did not render executable runtime control actions");
}

console.log(`web runtime node-context smoke passed (${workflowContextCalls.length} workflow-context calls, ${nodeContextCalls.length} node-context calls, ${runtimeBudgetCalls.length} runtime-budget calls, ${runtimeApiKeyCalls.length} api-key audit calls, ${runtimePreflightCalls.length} runtime-preflight calls, ${runtimeExecutionPlanCalls.length} runtime-execution-plan calls, ${runtimeHandoffCalls.length} runtime-handoff calls, ${prdReadinessCalls.length} prd-readiness calls, ${prdCompletionGateCalls.length} prd-completion-gate calls, ${prdCompletionPackageRequests.length} prd-completion-package calls, ${productionEvidenceTaskCalls.length} production-evidence task calls, ${productionEvidenceHandoffCalls.length} production-evidence handoff calls, ${productionEvidenceHandoffPackageRequests.length} production-evidence handoff-package calls, ${productionEvidenceRunPlanRequests.length} production-evidence run-plan calls, ${outputPackageCalls.length} output-package calls, ${outputPackageResultRequests.length} output-result calls, ${productionEvidenceItemTemplateRequests.length} production-evidence item-template calls, ${productionEvidenceItemRequests.length} production-evidence item calls, ${productionEvidenceTemplateRequests.length} production-evidence template calls, ${productionEvidenceLedgerBundleRequests.length} production-evidence ledger-bundle calls, ${productionEvidenceMergeRequests.length} production-evidence merge calls, ${productionEvidenceCloseoutRequests.length} production-evidence closeout calls, ${productionEvidenceValidationRequests.length} production-evidence validate calls, ${productionEvidenceRequests.length} production-evidence import calls, ${runtimeDiscoveryCalls.length} discovery calls, ${adapterCalls.length} adapter calls, ${providerContractCalls.length} provider-contract calls, ${providerGatewayWorkerCalls.length} provider-gateway-worker calls, ${softwareContractCalls.length} software-contract calls, ${desktopContractCalls.length} desktop-contract calls, ${handoffPackageRequests.length} handoff-package calls, ${runtimeRunNextRequests.length} runtime-run-next calls, ${desktopRunNextRequests.length} desktop-run-next calls, ${promptCalls.length} prompts calls, ${transcriptCalls.length} transcript calls, ${webSocketUrls.length} websocket streams, ${eventSourceUrls.length} event streams)`);
