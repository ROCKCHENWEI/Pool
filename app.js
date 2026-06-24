const STORAGE_KEY = "pool-control-state:v3";
const PROJECT_STORAGE_KEY = `${STORAGE_KEY}:project-filter`;
const RUNTIME_ENDPOINT_STORAGE_KEY = `${STORAGE_KEY}:runtime-endpoint`;
let eventPollTimer = null;
let eventStream = null;
let agentSessionStreams = new Map();
let nodeContextRequestId = 0;

const initialState = {
  selectedNode: "brief",
  selectedNodeContext: null,
  selectedNodeContextStatus: "idle",
  selectedNodeContextError: "",
  runStep: 0,
  tokenTotal: 42800,
  budgetLimit: 74000,
  apiKeys: [],
  apiKeyAudit: null,
  providerRequests: [],
  providerContracts: {},
  providerGatewayWorkerContract: null,
  integrationReadiness: null,
  runtimeBudget: null,
  runtimePreflight: null,
  runtimeExecutionPlan: null,
  runtimeRunNextResult: null,
  runtimeHandoff: null,
  runtimeHandoffPackage: null,
  runtimeCoreArchitectureReadiness: null,
  runtimeCoreArchitectureGate: null,
  runtimeCoreArchitecturePackage: null,
  runtimePrdReadiness: null,
  runtimePrdCompletionGate: null,
  runtimePrdCompletionPackage: null,
  providerConformancePackage: null,
  softwareConformancePackage: null,
  agentConformancePackage: null,
  integrationConformancePackage: null,
  productionEvidenceRequirements: null,
  productionEvidenceTasks: null,
  productionEvidenceHandoff: null,
  productionEvidenceHandoffPackage: null,
  productionEvidenceRunPlan: null,
  productionEvidenceTemplate: null,
  productionEvidenceItemTemplate: null,
  productionEvidenceTaskClaim: null,
  productionEvidenceItemValidation: null,
  productionEvidenceLedgerBundle: null,
  productionEvidenceMerge: null,
  productionEvidenceCloseout: null,
  productionEvidenceValidation: null,
  productionEvidenceImport: null,
  runtimeDiscovery: null,
  softwareActions: [],
  softwareContracts: {},
  desktopRecognitionRequests: [],
  desktopRecognitionContract: null,
  runtimeGraph: null,
  workflowContext: null,
  workflowContextStatus: "idle",
  workflowContextError: "",
  projects: [],
  providerAliases: {},
  connections: [],
  nodes: [
    {
      id: "brief",
      type: "Input Node",
      title: "创意启动包",
      status: "succeeded",
      agent: "Creative Scout",
      control: "API / 文件导入",
      input: "图片、视频、文字、Prompt",
      output: "CreativeBrief + source/",
      progress: 100,
      cost: 0,
      x: 46,
      y: 54,
      log: "已接收 4 类输入，并建立 image-blaster 风格项目包。",
    },
    {
      id: "agent",
      type: "Agent Node",
      title: "Agent 分析与拆解",
      status: "running",
      agent: "Pipeline Director",
      control: "MCP / Skills / CLI",
      input: "CreativeBrief",
      output: "候选节点与任务计划",
      progress: 48,
      cost: 2800,
      x: 355,
      y: 106,
      log: "正在生成视频、游戏、交互艺术三条输出路径。",
    },
    {
      id: "image3d",
      type: "3DGS Node",
      title: "2D / 3D / 3DGS 转换",
      status: "waiting_approval",
      agent: "World Builder",
      control: "Marble / Spark / TripoSplat Adapter",
      input: "source/ + image.json",
      output: "GLB / SPZ / 场景深度资产",
      progress: 12,
      cost: 9200,
      x: 670,
      y: 54,
      log: "检测到付费生成步骤，等待人工确认。",
    },
    {
      id: "software",
      type: "Software Control",
      title: "外部软件控制",
      status: "ready",
      agent: "Control Pilot",
      control: "API -> MCP -> CLI -> 桌面识别",
      input: "节点任务与软件状态",
      output: "可验证动作记录",
      progress: 0,
      cost: 1200,
      x: 670,
      y: 260,
      log: "Unreal 适配器可用，Blender/ComfyUI/Resolve 进入待检。",
    },
    {
      id: "unreal",
      type: "Unreal Node",
      title: "Unreal 自动拼装",
      status: "ready",
      agent: "Unreal Assembler",
      control: "Unreal MCP / Python / Editor Utility",
      input: "GLB、SPZ、材质、场景计划",
      output: "关卡、灯光、相机、蓝图",
      progress: 0,
      cost: 3600,
      x: 340,
      y: 390,
      log: "准备创建 neon-bazaar 原型关卡。",
    },
    {
      id: "outputs",
      type: "Output Node",
      title: "三类输出",
      status: "idle",
      agent: "Release Operator",
      control: "转码 / 构建 / 设备控制",
      input: "Unreal 场景与资产池",
      output: "视频、游戏原型、交互艺术控制包",
      progress: 0,
      cost: 1800,
      x: 735,
      y: 390,
      log: "等待 Unreal 组装完成后分发到三条输出线。",
    },
  ],
  agents: [
    {
      name: "Creative Scout",
      role: "创意测试、输入整理、参考素材归纳",
      token: 32,
      tools: ["MJ", "image-2", "ComfyUI", "素材库"],
      status: "在线",
    },
    {
      name: "Pipeline Director",
      role: "把创意拆成节点图、审批门和任务队列",
      token: 58,
      tools: ["MCP", "Skills", "CLI", "Git"],
      status: "运行中",
    },
    {
      name: "World Builder",
      role: "2D/3D/3DGS 转换与 image-blaster 风格资产落地",
      token: 72,
      tools: ["Marble", "Spark", "SAM-3D", "TripoSplat"],
      status: "待确认",
    },
    {
      name: "Control Pilot",
      role: "控制外部软件，API 不可用时切换桌面识别",
      token: 44,
      tools: ["API", "MCP", "桌面识别", "OCR"],
      status: "在线",
    },
    {
      name: "Unreal Assembler",
      role: "资产导入、关卡拼装、灯光相机和蓝图",
      token: 51,
      tools: ["Unreal MCP", "Python", "Editor Utility"],
      status: "待机",
    },
    {
      name: "Release Operator",
      role: "视频转码、游戏构建、交互艺术控制包输出",
      token: 24,
      tools: ["Resolve", "Build", "OSC", "DMX"],
      status: "待机",
    },
  ],
  software: [
    {
      name: "Unreal",
      priority: "深度集成",
      mode: "MCP / Python / CLI / 桌面识别",
      scope: "关卡拼装、蓝图、Sequencer、运行视口",
      runtimeEndpoint: "http://127.0.0.1:8787",
      health: "ready",
      latency: "42ms",
    },
    {
      name: "Unity",
      priority: "第二引擎",
      mode: "Editor Script / CLI / 桌面识别",
      scope: "Prefab、Scene、Timeline、构建",
      health: "planned",
      latency: "-",
    },
    {
      name: "DaVinci Resolve",
      priority: "视频输出",
      mode: "Python API / 渲染队列",
      scope: "素材池、时间线、调色、转码",
      health: "checking",
      latency: "180ms",
    },
    {
      name: "剪辑软件",
      priority: "通用 NLE",
      mode: "Adapter / 桌面识别",
      scope: "Premiere、Final Cut、CapCut 辅助控制",
      health: "planned",
      latency: "-",
    },
    {
      name: "TouchDesigner",
      priority: "交互艺术",
      mode: "Python / OSC / MIDI",
      scope: "节点网络、实时视觉、设备参数",
      health: "planned",
      latency: "-",
    },
    {
      name: "MadMapper",
      priority: "现场映射",
      mode: "OSC / 桌面识别",
      scope: "投影映射、媒体触发、灯光输出",
      health: "planned",
      latency: "-",
    },
    {
      name: "Blender",
      priority: "DCC 处理",
      mode: "Python / CLI",
      scope: "模型修复、材质、绑定、格式转换",
      health: "ready",
      latency: "85ms",
    },
    {
      name: "ComfyUI",
      priority: "AI 工作流",
      mode: "API / Workflow JSON",
      scope: "图像、视频、队列、结果回写",
      health: "checking",
      latency: "230ms",
    },
    {
      name: "动捕数据库",
      priority: "动作资产",
      mode: "API / 文件导入",
      scope: "动作搜索、FBX/BVH/GLB、重定向",
      health: "planned",
      latency: "-",
    },
    {
      name: "Nuke",
      priority: "合成",
      mode: "Python / CLI",
      scope: "节点合成、批量渲染、镜头回写",
      health: "planned",
      latency: "-",
    },
    {
      name: "Suno",
      priority: "音乐生成",
      mode: "授权 API / 人工确认",
      scope: "音乐、声音草稿、版权与成本提示",
      health: "guarded",
      latency: "manual",
    },
  ],
  apiProviders: [
    {
      id: "midjourney",
      group: "ai",
      name: "Midjourney",
      mode: "Bridge API / Discord Bot / Desktop fallback",
      endpoint: "https://api.midjourney.example/v1/imagine",
      runtimeEndpoint: "http://127.0.0.1:8787",
      auth: "MJ_API_KEY",
      output: "image/png + prompt metadata",
      status: "needs_key",
      cost: 1800,
    },
    {
      id: "openai-image-2",
      group: "ai",
      name: "OpenAI image-2",
      mode: "REST API",
      endpoint: "https://api.openai.com/v1/images",
      auth: "OPENAI_API_KEY",
      output: "image/png + request JSON",
      status: "checking",
      cost: 2200,
    },
    {
      id: "nano-banana-pro",
      group: "ai",
      name: "Nano Banana Pro",
      mode: "Generic HTTP Media Gateway",
      endpoint: "provider://nano-banana-pro/generate",
      runtimeEndpoint: "http://127.0.0.1:8787",
      auth: "NANO_BANANA_KEY",
      output: "image edit / clean plate",
      status: "planned",
      cost: 1400,
    },
    {
      id: "comfyui",
      group: "ai",
      name: "ComfyUI",
      mode: "Workflow JSON + Queue API",
      endpoint: "http://localhost:8188/prompt",
      auth: "local",
      output: "image/video + workflow trace",
      status: "checking",
      cost: 900,
    },
    {
      id: "suno",
      group: "ai",
      name: "Suno",
      mode: "Generic HTTP Media Gateway / human approval",
      endpoint: "provider://suno/music",
      runtimeEndpoint: "http://127.0.0.1:8787",
      auth: "SUNO_KEY",
      output: "music stem + license notes",
      status: "guarded",
      cost: 2600,
    },
    {
      id: "world-labs-marble",
      group: "3dgs",
      name: "World Labs Marble",
      mode: "REST API + asset downloader",
      endpoint: "https://platform.worldlabs.ai/api/v1/worlds",
      auth: "WORLD_LABS_API_KEY",
      output: "SPZ / GLB collider / pano / thumbnail",
      status: "checking",
      cost: 9200,
    },
    {
      id: "spark",
      group: "3dgs",
      name: "Spark",
      mode: "Local SDK / REST Adapter",
      endpoint: "provider://spark/3dgs",
      auth: "SPARK_KEY",
      output: "Gaussian splat package",
      status: "planned",
      cost: 6400,
    },
    {
      id: "qunhe",
      group: "3dgs",
      name: "群核科技",
      mode: "Enterprise API Adapter",
      endpoint: "provider://qunhe/scene-reconstruct",
      auth: "QUNHE_TOKEN",
      output: "scene mesh / reconstruction metadata",
      status: "planned",
      cost: 7800,
    },
    {
      id: "sam-3d",
      group: "3dgs",
      name: "SAM-3D",
      mode: "Model service / local runner",
      endpoint: "provider://sam-3d/segment-reconstruct",
      auth: "local",
      output: "segmented 3D assets",
      status: "planned",
      cost: 4200,
    },
    {
      id: "triposplat",
      group: "3dgs",
      name: "TripoSplat",
      mode: "REST API / FAL queue",
      endpoint: "provider://triposplat/generate",
      auth: "TRIPOSPLAT_KEY",
      output: "SPZ / GLB / preview video",
      status: "checking",
      cost: 6900,
    },
  ],
  hermes: {
    endpoint: "http://localhost:8787/hermes",
    status: "standby",
    lastCommand: "检查 Unreal Adapter，准备导入 neon-bazaar 的 3DGS 与 GLB 资产。",
    selectedRunbook: "pool_software_handoff",
    runbookTarget: "blender",
    runbookPreview: "",
    runbookStatus: "local",
    runbooks: [
      {
        name: "pool_content_burst_runbook",
        title: "Pool Content Burst Runbook",
        description: "Plan and execute a local content-burst workflow.",
        arguments: [],
      },
      {
        name: "pool_3dgs_conversion_review",
        title: "Pool 3DGS Conversion Review",
        description: "Inspect 2D/3DGS conversion readiness and approval gates.",
        arguments: [],
      },
      {
        name: "pool_software_handoff",
        title: "Pool Software Handoff",
        description: "Prepare an external software control action.",
        arguments: [],
      },
      {
        name: "pool_desktop_takeover",
        title: "Pool Desktop Takeover",
        description: "Guide a desktop recognition controller handoff.",
        arguments: [],
      },
    ],
    decisions: [],
    sessions: [],
    sessionTranscripts: {},
    workflowReport: null,
    trace: [
      "Hermes 控制通道已注册为嵌入式执行目标。",
      "当前策略：先走 API/MCP，失败后切换桌面识别控制。",
    ],
  },
  cliCommands: [
    {
      id: "workflow-context",
      title: "读取工作流上下文",
      command: "pool-cli --project demo workflow-context <workflow-id>",
      description: "读取当前 workflow 的图、任务、资产和控制账本。",
    },
    {
      id: "run-provider",
      title: "提交 Provider 任务",
      command: "pool-cli --project demo run-provider world-labs-marble --execution-mode mock --no-approval --prompt \"Agent CLI 3DGS smoke\"",
      description: "通过统一 Adapter 提交 AI/3DGS 生成任务。",
    },
    {
      id: "hermes-control",
      title: "Hermes 控制",
      command: "pool-cli --project demo agent-session hermes --instruction \"inspect workflow context and coordinate Unreal handoff\" --allowed-tool api --allowed-tool mcp --allowed-tool unreal",
      description: "向 Hermes 内嵌控制通道发送软件控制任务。",
    },
    {
      id: "agent-cli",
      title: "Agent CLI 会话",
      command: "pool-cli --project demo agent-session agent-cli --command-id workflow-context --title \"Inspect workflow context\" --command \"pool-cli --project demo workflow-context\" --tool cli --token-budget 74000",
      description: "启动带工具权限和 Token 预算的 Agent 会话。",
    },
  ],
  tasks: [
    {
      id: "task-001",
      nodeId: "agent",
      title: "生成三路线节点执行计划",
      type: "agent-command",
      status: "running",
      tool: "MCP + Skills",
      risk: "low",
      cost: 2800,
    },
    {
      id: "task-002",
      nodeId: "image3d",
      title: "Marble/Spark 3DGS 转换预检",
      type: "approval-gate",
      status: "waiting_approval",
      tool: "3DGS Adapter",
      risk: "high",
      cost: 9200,
    },
    {
      id: "task-003",
      nodeId: "software",
      title: "Unreal Adapter 健康检查",
      type: "software-control",
      status: "ready",
      tool: "Unreal MCP",
      risk: "medium",
      cost: 1200,
    },
  ],
  assets: [
    {
      id: "asset-brief",
      name: "neon-bazaar-brief.json",
      type: "CreativeBrief",
      path: "worlds/neon-bazaar/project.json",
      source: "brief",
      status: "indexed",
    },
    {
      id: "asset-source",
      name: "0-neon-bazaar-source.png",
      type: "Source Image",
      path: "worlds/neon-bazaar/source/0-neon-bazaar.png",
      source: "brief",
      status: "local",
    },
  ],
  outputManifests: [],
  events: [
    {
      at: "20:55:31",
      level: "ok",
      text: "Pool 控制台启动，本地资产结构已建立。",
    },
    {
      at: "20:55:36",
      level: "info",
      text: "节点图写入执行计划，等待 3DGS 付费前确认。",
    },
    {
      at: "20:55:41",
      level: "warn",
      text: "ComfyUI Adapter 正在等待健康检查回执。",
    },
  ],
};

let state = loadState();

const nodeLayer = document.querySelector("#nodeLayer");
const connectionLayer = document.querySelector(".connections");
const panels = document.querySelectorAll(".panel");
const navButtons = document.querySelectorAll(".rail-item");
const DEFAULT_RUNTIME_PORTS = [4788, 4789, 4790, 4878];
const DEFAULT_RUNTIME_HOSTS = ["127.0.0.1", "localhost"];

function clone(value) {
  return JSON.parse(JSON.stringify(value));
}

function loadState() {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return clone(initialState);
    const loaded = JSON.parse(raw);
    return { ...clone(initialState), ...loaded };
  } catch {
    return clone(initialState);
  }
}

function saveState() {
  const persisted = {
    ...state,
    apiProviders: state.apiProviders.map(({ contract, ...provider }) => provider),
    software: state.software.map(({ contract, ...software }) => software),
    providerContracts: {},
    providerGatewayWorkerContract: null,
    softwareContracts: {},
    desktopRecognitionContract: null,
    runtimeDiscovery: null,
    runtimeHandoffPackage: null,
    runtimePrdReadiness: null,
    runtimePrdCompletionGate: null,
    runtimePrdCompletionPackage: null,
    providerConformancePackage: null,
    softwareConformancePackage: null,
    agentConformancePackage: null,
    integrationConformancePackage: null,
    productionEvidenceRequirements: null,
    productionEvidenceTasks: null,
    productionEvidenceItemTemplate: null,
    productionEvidenceTaskClaim: null,
    productionEvidenceItemValidation: null,
    productionEvidenceLedgerBundle: null,
    productionEvidenceMerge: null,
    productionEvidenceCloseout: null,
    productionEvidenceHandoff: null,
    productionEvidenceHandoffPackage: null,
    productionEvidenceRunPlan: null,
    workflowContext: null,
    workflowContextStatus: "idle",
    workflowContextError: "",
    selectedNodeContext: null,
    selectedNodeContextStatus: "idle",
    selectedNodeContextError: "",
  };
  localStorage.setItem(STORAGE_KEY, JSON.stringify(persisted));
}

function runtimeBaseUrl() {
  const params = new URLSearchParams(window.location.search);
  const runtime = params.get("runtime")?.trim();
  if (!runtime) return null;
  if (isRuntimeDiscoveryValue(runtime)) return null;
  return normalizeRuntimeEndpoint(runtime);
}

function runtimeDiscoveryRequested() {
  const params = new URLSearchParams(window.location.search);
  const runtime = params.get("runtime")?.trim();
  return !runtime || isRuntimeDiscoveryValue(runtime);
}

function isRuntimeDiscoveryValue(value) {
  return ["local", "auto", "*"].includes(String(value ?? "").trim().toLowerCase());
}

function normalizeRuntimeEndpoint(value) {
  const trimmed = String(value ?? "").trim();
  if (!trimmed) return "";
  const withProtocol = /^https?:\/\//i.test(trimmed) ? trimmed : `http://${trimmed}`;
  return withProtocol.replace(/\/+$/, "");
}

async function runtimeDiscoveryEndpoints() {
  const params = new URLSearchParams(window.location.search);
  const registryEndpoints = await runtimeRegistryEndpoints(params.get("runtime_registry"));
  const declared = parseRuntimeEndpointList(params.get("runtime_endpoints"));
  const stored = storedRuntimeEndpoint();
  const fromPorts = runtimeDiscoveryPorts()
    .flatMap((port) => DEFAULT_RUNTIME_HOSTS.map((host) => `http://${host}:${port}`));

  return uniqueRuntimeEndpoints([...registryEndpoints, stored, ...declared, ...fromPorts]);
}

function parseRuntimeEndpointList(value) {
  return String(value ?? "")
    .split(",")
    .map(normalizeRuntimeEndpoint)
    .filter(Boolean);
}

async function runtimeRegistryEndpoints(value) {
  const registryUrls = String(value ?? "")
    .split(",")
    .map((url) => url.trim())
    .filter(Boolean);
  const endpoints = [];
  for (const url of registryUrls) {
    try {
      endpoints.push(...runtimeEndpointsFromRegistry(await fetchJson(url)));
    } catch {
      addEvent("warn", `Runtime registry 不可读：${url}`);
    }
  }
  return uniqueRuntimeEndpoints(endpoints);
}

function runtimeEndpointsFromRegistry(registry) {
  if (!registry) return [];
  if (Array.isArray(registry)) return registry.flatMap(runtimeEndpointsFromRegistry);
  const direct = [
    registry.base_url,
    registry.baseUrl,
    registry.runtime_url,
    registry.runtimeUrl,
    registry.runtime_endpoint,
    registry.runtimeEndpoint,
    registry.endpoint,
  ];
  const nested = [
    ...registryEndpointsFromList(registry.endpoints),
    ...registryEndpointsFromList(registry.runtimes),
    ...registryEndpointsFromList(registry.services),
  ];
  return uniqueRuntimeEndpoints([...direct, ...nested].map(normalizeRuntimeEndpoint));
}

function registryEndpointsFromList(value) {
  if (!Array.isArray(value)) return [];
  return value.flatMap((item) => {
    if (typeof item === "string") return [item];
    if (item && typeof item === "object") return runtimeEndpointsFromRegistry(item);
    return [];
  });
}

function runtimeDiscoveryPorts() {
  const params = new URLSearchParams(window.location.search);
  const declared = params.get("runtime_ports") ?? params.get("ports");
  const ports = String(declared ?? "")
    .split(",")
    .map((value) => Number.parseInt(value.trim(), 10))
    .filter((port) => Number.isInteger(port) && port > 0 && port <= 65535);
  return ports.length ? ports : DEFAULT_RUNTIME_PORTS;
}

function uniqueRuntimeEndpoints(endpoints) {
  return [...new Set(endpoints.filter(Boolean))];
}

function storedRuntimeEndpoint() {
  try {
    return normalizeRuntimeEndpoint(localStorage.getItem(RUNTIME_ENDPOINT_STORAGE_KEY) ?? "");
  } catch {
    return "";
  }
}

function setStoredRuntimeEndpoint(value) {
  try {
    const endpoint = normalizeRuntimeEndpoint(value);
    if (endpoint) localStorage.setItem(RUNTIME_ENDPOINT_STORAGE_KEY, endpoint);
  } catch {
    // localStorage can be unavailable in private or embedded contexts.
  }
}

function runtimeProjectFilter() {
  const params = new URLSearchParams(window.location.search);
  const explicit = params.has("project_slug") ? params.get("project_slug") : params.get("project");
  if (explicit !== null) {
    const value = explicit.trim();
    setStoredProjectFilter(value);
    return value;
  }
  try {
    return localStorage.getItem(PROJECT_STORAGE_KEY) ?? "";
  } catch {
    return "";
  }
}

function setStoredProjectFilter(value) {
  try {
    if (value) {
      localStorage.setItem(PROJECT_STORAGE_KEY, value);
    } else {
      localStorage.removeItem(PROJECT_STORAGE_KEY);
    }
  } catch {
    // localStorage can be unavailable in private or embedded contexts.
  }
}

function setRuntimeProjectFilter(value) {
  const project = String(value ?? "").trim();
  setStoredProjectFilter(project);
  if (typeof window === "undefined" || !window.history) return;
  const params = new URLSearchParams(window.location.search);
  params.delete("project_slug");
  if (project) {
    params.set("project", project);
  } else {
    params.delete("project");
  }
  const query = params.toString();
  const nextUrl = `${window.location.pathname}${query ? `?${query}` : ""}${window.location.hash}`;
  window.history.replaceState(null, "", nextUrl);
}

function runtimeQuerySuffix() {
  const project = runtimeProjectFilter();
  return project ? `?project=${encodeURIComponent(project)}` : "";
}

function runtimeHealthUrl(baseUrl) {
  return `${baseUrl}/api/health${runtimeQuerySuffix()}`;
}

function runtimeSnapshotUrl(baseUrl) {
  return `${baseUrl}/api/snapshot${runtimeQuerySuffix()}`;
}

function runtimeGraphUrl(baseUrl) {
  return `${baseUrl}/api/runtime-graph${runtimeQuerySuffix()}`;
}

function runtimeBudgetUrl(baseUrl) {
  return `${baseUrl}/api/runtime-budget${runtimeQuerySuffix()}`;
}

function runtimeApiKeysUrl(baseUrl) {
  const params = new URLSearchParams();
  const project = runtimeProjectFilter();
  if (project) params.set("project", project);
  params.set("rotation_days", "90");
  return `${baseUrl}/api/api-keys?${params.toString()}`;
}

function runtimePreflightUrl(baseUrl) {
  return `${baseUrl}/api/runtime-preflight${runtimeQuerySuffix()}`;
}

function runtimeExecutionPlanUrl(baseUrl) {
  return `${baseUrl}/api/runtime-execution-plan${runtimeQuerySuffix()}`;
}

function runtimeExecutionPlanRunNextUrl(baseUrl) {
  return `${baseUrl}/api/runtime-execution-plan/run-next${runtimeQuerySuffix()}`;
}

function runtimeHandoffUrl(baseUrl) {
  return `${baseUrl}/api/runtime-handoff${runtimeQuerySuffix()}`;
}

function runtimeCoreArchitectureReadinessUrl(baseUrl) {
  return `${baseUrl}/api/core-architecture-readiness${runtimeQuerySuffix()}`;
}

function runtimeCoreArchitectureGateUrl(baseUrl) {
  return `${baseUrl}/api/core-architecture-gate${runtimeQuerySuffix()}`;
}

function runtimeCoreArchitecturePackagesUrl(baseUrl) {
  return `${baseUrl}/api/core-architecture-packages${runtimeQuerySuffix()}`;
}

function runtimeCoreArchitecturePackageUrl(baseUrl) {
  return `${baseUrl}/api/core-architecture-package`;
}

function runtimePrdReadinessUrl(baseUrl) {
  return `${baseUrl}/api/prd-readiness${runtimeQuerySuffix()}`;
}

function runtimePrdCompletionGateUrl(baseUrl) {
  return `${baseUrl}/api/prd-completion-gate${runtimeQuerySuffix()}`;
}

function runtimePrdCompletionPackagesUrl(baseUrl) {
  return `${baseUrl}/api/prd-completion-packages${runtimeQuerySuffix()}`;
}

function runtimePrdCompletionPackageUrl(baseUrl) {
  return `${baseUrl}/api/prd-completion-package`;
}

function runtimeProductionEvidenceRequirementsUrl(baseUrl) {
  return `${baseUrl}/api/production-evidence/requirements${runtimeQuerySuffix()}`;
}

function runtimeProductionEvidenceTasksUrl(baseUrl) {
  return `${baseUrl}/api/production-evidence/tasks${runtimeQuerySuffix()}`;
}

function runtimeProductionEvidenceHandoffUrl(baseUrl) {
  return `${baseUrl}/api/production-evidence/handoff${runtimeQuerySuffix()}`;
}

function runtimeProductionEvidenceHandoffPackagesUrl(baseUrl) {
  return `${baseUrl}/api/production-evidence/handoff-packages${runtimeQuerySuffix()}`;
}

function runtimeProductionEvidenceRunPlanUrl(baseUrl, options = {}) {
  const params = new URLSearchParams();
  const project = runtimeProjectFilter();
  if (project) params.set("project", project);
  if (options.outputRoot) params.set("output_root", options.outputRoot);
  if (options.source) params.set("source", options.source);
  const query = params.toString();
  return `${baseUrl}/api/production-evidence/run-plan${query ? `?${query}` : ""}`;
}

function runtimeProductionEvidenceHandoffPackageUrl(baseUrl) {
  return `${baseUrl}/api/production-evidence/handoff-packages`;
}

function runtimeProductionEvidenceUrl(baseUrl) {
  return `${baseUrl}/api/production-evidence`;
}

function runtimeProductionEvidenceItemsUrl(baseUrl) {
  return `${baseUrl}/api/production-evidence/items`;
}

function runtimeProductionEvidenceItemsValidateUrl(baseUrl) {
  return `${baseUrl}/api/production-evidence/items/validate`;
}

function runtimeProductionEvidenceTaskClaimUrl(baseUrl) {
  return `${baseUrl}/api/production-evidence/tasks/claim`;
}

function runtimeProductionEvidenceTemplateUrl(baseUrl, options = {}) {
  const params = new URLSearchParams();
  const project = runtimeProjectFilter();
  if (project) params.set("project", project);
  if (options.missingOnly) params.set("missing_only", "true");
  const query = params.toString();
  return `${baseUrl}/api/production-evidence/template${query ? `?${query}` : ""}`;
}

function runtimeProductionEvidenceItemTemplateUrl(baseUrl, options = {}) {
  const params = new URLSearchParams();
  const project = runtimeProjectFilter();
  if (project) params.set("project", project);
  if (options.taskId) params.set("task_id", options.taskId);
  if (options.kind) params.set("kind", options.kind);
  if (options.targetId) params.set("target_id", options.targetId);
  if (options.outputRoot) params.set("output_root", options.outputRoot);
  const query = params.toString();
  return `${baseUrl}/api/production-evidence/item-template${query ? `?${query}` : ""}`;
}

function runtimeProductionEvidenceBundleFromLedgerUrl(baseUrl, options = {}) {
  const params = new URLSearchParams();
  const project = runtimeProjectFilter();
  if (project) params.set("project", project);
  if (options.source) params.set("source", options.source);
  if (options.includeIncomplete) params.set("include_incomplete", "true");
  const query = params.toString();
  return `${baseUrl}/api/production-evidence/bundle-from-ledger${query ? `?${query}` : ""}`;
}

function runtimeProductionEvidenceValidateUrl(baseUrl) {
  return `${baseUrl}/api/production-evidence/validate`;
}

function runtimeProductionEvidenceMergeUrl(baseUrl) {
  return `${baseUrl}/api/production-evidence/merge`;
}

function runtimeProductionEvidenceCloseoutUrl(baseUrl) {
  return `${baseUrl}/api/production-evidence/closeout`;
}

function runtimeOutputPackagesUrl(baseUrl) {
  return `${baseUrl}/api/output-packages${runtimeQuerySuffix()}`;
}

function runtimeOutputPackageResultsUrl(baseUrl) {
  return `${baseUrl}/api/output-packages/results`;
}

function runtimeHandoffPackageUrl(baseUrl) {
  return `${baseUrl}/api/handoff-packages`;
}

function runtimeHandoffPackagesUrl(baseUrl) {
  return `${baseUrl}/api/handoff-packages${runtimeQuerySuffix()}`;
}

function runtimeDiscoveryUrl(baseUrl) {
  return `${baseUrl}/api/discovery${runtimeQuerySuffix()}`;
}

function runtimeWorkflowContextUrl(baseUrl, workflowId = "") {
  const params = new URLSearchParams();
  const project = runtimeProjectFilter();
  if (project) params.set("project", project);
  if (workflowId) params.set("workflow_id", workflowId);
  const suffix = params.toString();
  return `${baseUrl}/api/workflow-context${suffix ? `?${suffix}` : ""}`;
}

function runtimeNodeContextUrl(baseUrl, nodeId) {
  const params = new URLSearchParams();
  const project = runtimeProjectFilter();
  if (project) params.set("project", project);
  params.set("node_id", nodeId);
  return `${baseUrl}/api/node-context?${params.toString()}`;
}

function runtimeProjectsUrl(baseUrl) {
  return `${baseUrl}/api/projects${runtimeQuerySuffix()}`;
}

function runtimeAdaptersUrl(baseUrl) {
  return `${baseUrl}/api/adapters`;
}

function runtimeIntegrationReadinessUrl(baseUrl) {
  return `${baseUrl}/api/integration-readiness${runtimeQuerySuffix()}`;
}

function runtimeProviderContractsUrl(baseUrl) {
  return `${baseUrl}/api/provider-contracts`;
}

function runtimeProviderGatewayWorkerUrl(baseUrl) {
  return `${baseUrl}/api/provider-gateway-worker`;
}

function runtimeProviderConformancePackagesUrl(baseUrl) {
  return `${baseUrl}/api/provider-conformance-packages${runtimeQuerySuffix()}`;
}

function runtimeIntegrationConformancePackagesUrl(baseUrl) {
  return `${baseUrl}/api/integration-conformance-packages${runtimeQuerySuffix()}`;
}

function runtimeSoftwareContractsUrl(baseUrl) {
  return `${baseUrl}/api/software-contracts`;
}

function runtimeSoftwareConformancePackagesUrl(baseUrl) {
  return `${baseUrl}/api/software-conformance-packages${runtimeQuerySuffix()}`;
}

function runtimeAgentConformancePackagesUrl(baseUrl) {
  return `${baseUrl}/api/agent-conformance-packages${runtimeQuerySuffix()}`;
}

function runtimePromptsUrl(baseUrl) {
  return `${baseUrl}/api/prompts`;
}

function runtimePromptUrl(baseUrl, name, args = {}) {
  const params = new URLSearchParams();
  params.set("name", name);
  Object.entries(args).forEach(([key, value]) => {
    if (value !== undefined && value !== null && String(value).trim()) {
      params.set(key, String(value));
    }
  });
  return `${baseUrl}/api/prompts?${params.toString()}`;
}

function runtimeAgentTranscriptUrl(baseUrl, sessionId) {
  const params = new URLSearchParams();
  const project = runtimeProjectFilter();
  if (project) params.set("project", project);
  params.set("session_id", sessionId);
  return `${baseUrl}/api/agent-sessions/transcript?${params.toString()}`;
}

function runtimeAgentStreamUrl(baseUrl, sessionId, afterId = "") {
  const params = new URLSearchParams();
  const project = runtimeProjectFilter();
  if (project) params.set("project", project);
  params.set("session_id", sessionId);
  if (afterId) params.set("last_event_id", afterId);
  params.set("limit", "24");
  return `${baseUrl}/api/agent-sessions/stream?${params.toString()}`;
}

function runtimeAgentWebSocketUrl(baseUrl, sessionId, afterId = "") {
  const url = new URL(runtimeAgentStreamUrl(baseUrl, sessionId, afterId).replace("/api/agent-sessions/stream", "/api/agent-sessions/ws"));
  url.protocol = url.protocol === "https:" ? "wss:" : "ws:";
  return url.toString();
}

function runtimeDesktopRecognitionRequestsUrl(baseUrl) {
  return `${baseUrl}/api/desktop-recognition/requests${runtimeQuerySuffix()}`;
}

function runtimeDesktopRecognitionContractUrl(baseUrl) {
  return `${baseUrl}/api/desktop-recognition/contract`;
}

function runtimeDesktopRecognitionRunNextUrl(baseUrl) {
  return `${baseUrl}/api/desktop-recognition/run-next${runtimeQuerySuffix()}`;
}

function runtimeDesktopRecognitionResultsUrl(baseUrl) {
  return `${baseUrl}/api/desktop-recognition/results`;
}

function runtimeEventsUrl(baseUrl, afterId = "") {
  const params = new URLSearchParams();
  const project = runtimeProjectFilter();
  if (project) params.set("project", project);
  if (afterId) params.set("after_id", afterId);
  params.set("limit", "24");
  return `${baseUrl}/api/events?${params.toString()}`;
}

function runtimeEventsStreamUrl(baseUrl, afterId = "") {
  const params = new URLSearchParams();
  const project = runtimeProjectFilter();
  if (project) params.set("project", project);
  if (afterId) params.set("last_event_id", afterId);
  params.set("limit", "24");
  return `${baseUrl}/api/events/stream?${params.toString()}`;
}

function runtimeEventsWebSocketUrl(baseUrl, afterId = "") {
  const url = new URL(runtimeEventsStreamUrl(baseUrl, afterId).replace("/api/events/stream", "/api/events/ws"));
  url.protocol = url.protocol === "https:" ? "wss:" : "ws:";
  return url.toString();
}

function snapshotUrl() {
  const params = new URLSearchParams(window.location.search);
  return params.get("snapshot") ?? "runtime-snapshot.json";
}

function explicitSnapshotUrl() {
  const params = new URLSearchParams(window.location.search);
  return params.has("snapshot") ? params.get("snapshot") : null;
}

async function applyRuntimeSnapshotIfAvailable() {
  const params = new URLSearchParams(window.location.search);
  const hasRuntimeParam = params.has("runtime");
  const runtime = runtimeBaseUrl();
  if (runtime && (await applyRuntimeHttpSnapshot(runtime, { explicit: true }))) return true;

  const explicitSnapshot = explicitSnapshotUrl();
  if (!hasRuntimeParam && explicitSnapshot) return applySnapshotUrl(explicitSnapshot);

  if (runtimeDiscoveryRequested() && (await applyAutoDiscoveredRuntime())) return true;

  if (explicitSnapshot) return applySnapshotUrl(explicitSnapshot);

  const url = snapshotUrl();
  if (!url) return false;

  return applySnapshotUrl(url);
}

async function applyAutoDiscoveredRuntime() {
  const endpoints = await runtimeDiscoveryEndpoints();
  for (const baseUrl of endpoints) {
    if (await applyRuntimeHttpSnapshot(baseUrl, { auto: true })) return true;
  }
  addEvent("info", `未发现本地 Runtime HTTP（已探测 ${endpoints.length} 个候选端口），继续读取默认 snapshot 文件。`);
  return false;
}

async function applySnapshotUrl(url) {
  try {
    const snapshot = await fetchJson(url);
    mergeRuntimeSnapshot(snapshot, url, { mode: "snapshot-file" });
    return true;
  } catch {
    return false;
  }
}

async function applyRuntimeHttpSnapshot(baseUrl, options = {}) {
  try {
    const healthUrl = runtimeHealthUrl(baseUrl);
    const snapshotUrl = runtimeSnapshotUrl(baseUrl);
    const [health, snapshot, runtimeGraph, runtimeExecutionPlan, runtimeBudget, apiKeyAudit, runtimePreflight, runtimeHandoff, runtimeHandoffPackages, runtimeCoreArchitectureReadiness, runtimeCoreArchitectureGate, runtimeCoreArchitecturePackages, runtimePrdReadiness, runtimePrdCompletionGate, runtimePrdCompletionPackages, productionEvidenceRequirements, productionEvidenceTasks, productionEvidenceHandoff, productionEvidenceHandoffPackages, providerConformancePackages, softwareConformancePackages, agentConformancePackages, integrationConformancePackages, outputPackages, runtimeDiscovery, adapters, integrationReadiness, providerContracts, providerGatewayWorker, softwareContracts, desktopContract, prompts, projects, desktopRequests] = await Promise.all([
      fetchJson(healthUrl),
      fetchJson(snapshotUrl),
      fetchJson(runtimeGraphUrl(baseUrl)).catch(() => null),
      fetchJson(runtimeExecutionPlanUrl(baseUrl)).catch(() => null),
      fetchJson(runtimeBudgetUrl(baseUrl)).catch(() => null),
      fetchJson(runtimeApiKeysUrl(baseUrl)).catch(() => null),
      fetchJson(runtimePreflightUrl(baseUrl)).catch(() => null),
      fetchJson(runtimeHandoffUrl(baseUrl)).catch(() => null),
      fetchJson(runtimeHandoffPackagesUrl(baseUrl)).catch(() => null),
      fetchJson(runtimeCoreArchitectureReadinessUrl(baseUrl)).catch(() => null),
      fetchJson(runtimeCoreArchitectureGateUrl(baseUrl)).catch(() => null),
      fetchJson(runtimeCoreArchitecturePackagesUrl(baseUrl)).catch(() => null),
      fetchJson(runtimePrdReadinessUrl(baseUrl)).catch(() => null),
      fetchJson(runtimePrdCompletionGateUrl(baseUrl)).catch(() => null),
      fetchJson(runtimePrdCompletionPackagesUrl(baseUrl)).catch(() => null),
      fetchJson(runtimeProductionEvidenceRequirementsUrl(baseUrl)).catch(() => null),
      fetchJson(runtimeProductionEvidenceTasksUrl(baseUrl)).catch(() => null),
      fetchJson(runtimeProductionEvidenceHandoffUrl(baseUrl)).catch(() => null),
      fetchJson(runtimeProductionEvidenceHandoffPackagesUrl(baseUrl)).catch(() => null),
      fetchJson(runtimeProviderConformancePackagesUrl(baseUrl)).catch(() => null),
      fetchJson(runtimeSoftwareConformancePackagesUrl(baseUrl)).catch(() => null),
      fetchJson(runtimeAgentConformancePackagesUrl(baseUrl)).catch(() => null),
      fetchJson(runtimeIntegrationConformancePackagesUrl(baseUrl)).catch(() => null),
      fetchJson(runtimeOutputPackagesUrl(baseUrl)).catch(() => null),
      fetchJson(runtimeDiscoveryUrl(baseUrl)).catch(() => null),
      fetchJson(runtimeAdaptersUrl(baseUrl)).catch(() => null),
      fetchJson(runtimeIntegrationReadinessUrl(baseUrl)).catch(() => null),
      fetchJson(runtimeProviderContractsUrl(baseUrl)).catch(() => null),
      fetchJson(runtimeProviderGatewayWorkerUrl(baseUrl)).catch(() => null),
      fetchJson(runtimeSoftwareContractsUrl(baseUrl)).catch(() => null),
      fetchJson(runtimeDesktopRecognitionContractUrl(baseUrl)).catch(() => null),
      fetchJson(runtimePromptsUrl(baseUrl)).catch(() => null),
      fetchJson(runtimeProjectsUrl(baseUrl)).catch(() => null),
      fetchJson(runtimeDesktopRecognitionRequestsUrl(baseUrl)).catch(() => null),
    ]);
    const workflowContext = await fetchRuntimeWorkflowContext(baseUrl, snapshot, runtimeGraph);
    mergeRuntimeAdapterRegistry(adapters);
    mergeRuntimeProviderContracts(providerContracts);
    mergeRuntimeProviderGatewayWorkerContract(providerGatewayWorker);
    mergeRuntimeSoftwareContracts(softwareContracts);
    mergeRuntimeDesktopRecognitionContract(desktopContract);
    mergeRuntimePrompts(prompts);
    mergeRuntimeProjects(projects ?? snapshot);
    mergeRuntimeSnapshot(snapshot, snapshotUrl, {
      mode: "runtime-http",
      runtime: baseUrl,
      health,
      runtimeGraph,
      runtimeExecutionPlan,
      runtimeBudget,
      apiKeyAudit,
      runtimePreflight,
      runtimeHandoff,
      runtimeHandoffPackages,
      runtimeCoreArchitectureReadiness,
      runtimeCoreArchitectureGate,
      runtimeCoreArchitecturePackages,
      runtimePrdReadiness,
      runtimePrdCompletionGate,
      runtimePrdCompletionPackages,
      productionEvidenceRequirements,
      productionEvidenceTasks,
      productionEvidenceHandoff,
      productionEvidenceHandoffPackages,
      providerConformancePackages,
      softwareConformancePackages,
      agentConformancePackages,
      integrationConformancePackages,
      outputPackages,
      runtimeDiscovery,
      integrationReadiness,
      workflowContext,
      desktopRequests,
    });
    setStoredRuntimeEndpoint(baseUrl);
    return true;
  } catch {
    if (options.explicit) {
      addEvent("warn", `Runtime HTTP 未连接，回退到 snapshot 文件：${baseUrl}`);
    }
    return false;
  }
}

async function mergeRuntimeMutationSnapshot(snapshot, runtime, options = {}) {
  if (!snapshot || !runtime) return;
  const runtimeGraph = options.runtimeGraph ?? await fetchJson(runtimeGraphUrl(runtime)).catch(() => state.runtimeGraph);
  const runtimeExecutionPlan = options.runtimeExecutionPlan ?? await fetchJson(runtimeExecutionPlanUrl(runtime)).catch(() => state.runtimeExecutionPlan);
  const runtimeBudget = options.runtimeBudget ?? await fetchJson(runtimeBudgetUrl(runtime)).catch(() => state.runtimeBudget);
  const apiKeyAudit = options.apiKeyAudit ?? await fetchJson(runtimeApiKeysUrl(runtime)).catch(() => state.apiKeyAudit);
  const runtimePreflight = options.runtimePreflight ?? await fetchJson(runtimePreflightUrl(runtime)).catch(() => state.runtimePreflight);
  const runtimeHandoff = options.runtimeHandoff ?? await fetchJson(runtimeHandoffUrl(runtime)).catch(() => state.runtimeHandoff);
  const runtimeHandoffPackages = options.runtimeHandoffPackages ?? await fetchJson(runtimeHandoffPackagesUrl(runtime)).catch(() => null);
  const runtimeCoreArchitectureReadiness = options.runtimeCoreArchitectureReadiness ?? await fetchJson(runtimeCoreArchitectureReadinessUrl(runtime)).catch(() => state.runtimeCoreArchitectureReadiness);
  const runtimeCoreArchitectureGate = options.runtimeCoreArchitectureGate ?? await fetchJson(runtimeCoreArchitectureGateUrl(runtime)).catch(() => state.runtimeCoreArchitectureGate);
  const runtimeCoreArchitecturePackages = options.runtimeCoreArchitecturePackages ?? await fetchJson(runtimeCoreArchitecturePackagesUrl(runtime)).catch(() => null);
  const runtimePrdReadiness = options.runtimePrdReadiness ?? await fetchJson(runtimePrdReadinessUrl(runtime)).catch(() => state.runtimePrdReadiness);
  const runtimePrdCompletionGate = options.runtimePrdCompletionGate ?? await fetchJson(runtimePrdCompletionGateUrl(runtime)).catch(() => state.runtimePrdCompletionGate);
  const runtimePrdCompletionPackages = options.runtimePrdCompletionPackages ?? await fetchJson(runtimePrdCompletionPackagesUrl(runtime)).catch(() => null);
  const productionEvidenceRequirements = options.productionEvidenceRequirements ?? await fetchJson(runtimeProductionEvidenceRequirementsUrl(runtime)).catch(() => state.productionEvidenceRequirements);
  const productionEvidenceTasks = options.productionEvidenceTasks ?? await fetchJson(runtimeProductionEvidenceTasksUrl(runtime)).catch(() => state.productionEvidenceTasks);
  const productionEvidenceHandoff = options.productionEvidenceHandoff ?? await fetchJson(runtimeProductionEvidenceHandoffUrl(runtime)).catch(() => state.productionEvidenceHandoff);
  const productionEvidenceHandoffPackages = options.productionEvidenceHandoffPackages ?? await fetchJson(runtimeProductionEvidenceHandoffPackagesUrl(runtime)).catch(() => null);
  const providerConformancePackages = options.providerConformancePackages ?? await fetchJson(runtimeProviderConformancePackagesUrl(runtime)).catch(() => null);
  const softwareConformancePackages = options.softwareConformancePackages ?? await fetchJson(runtimeSoftwareConformancePackagesUrl(runtime)).catch(() => null);
  const agentConformancePackages = options.agentConformancePackages ?? await fetchJson(runtimeAgentConformancePackagesUrl(runtime)).catch(() => null);
  const integrationConformancePackages = options.integrationConformancePackages ?? await fetchJson(runtimeIntegrationConformancePackagesUrl(runtime)).catch(() => null);
  const outputPackages = options.outputPackages ?? await fetchJson(runtimeOutputPackagesUrl(runtime)).catch(() => null);
  const runtimeDiscovery = options.runtimeDiscovery ?? await fetchJson(runtimeDiscoveryUrl(runtime)).catch(() => state.runtimeDiscovery);
  const integrationReadiness = options.integrationReadiness ?? await fetchJson(runtimeIntegrationReadinessUrl(runtime)).catch(() => state.integrationReadiness);
  const providerContracts = options.providerContracts ?? await fetchJson(runtimeProviderContractsUrl(runtime)).catch(() => null);
  const providerGatewayWorker = options.providerGatewayWorker ?? await fetchJson(runtimeProviderGatewayWorkerUrl(runtime)).catch(() => state.providerGatewayWorkerContract);
  const softwareContracts = options.softwareContracts ?? await fetchJson(runtimeSoftwareContractsUrl(runtime)).catch(() => null);
  const desktopContract = options.desktopContract ?? await fetchJson(runtimeDesktopRecognitionContractUrl(runtime)).catch(() => null);
  const workflowContext = options.workflowContext ?? await fetchRuntimeWorkflowContext(runtime, snapshot, runtimeGraph);
  const desktopRequests = options.desktopRequests ?? await fetchJson(runtimeDesktopRecognitionRequestsUrl(runtime)).catch(() => ({ requests: state.desktopRecognitionRequests ?? [] }));
  const health = options.health ?? (snapshot.stats ? { stats: snapshot.stats } : undefined);
  mergeRuntimeProviderContracts(providerContracts);
  mergeRuntimeProviderGatewayWorkerContract(providerGatewayWorker);
  mergeRuntimeSoftwareContracts(softwareContracts);
  mergeRuntimeDesktopRecognitionContract(desktopContract);
  mergeRuntimeSnapshot(snapshot, runtimeSnapshotUrl(runtime), {
    ...options,
    mode: options.mode ?? "runtime-http",
    runtime: options.runtime ?? runtime,
    health,
    runtimeGraph,
    runtimeExecutionPlan,
    runtimeBudget,
    apiKeyAudit,
    runtimePreflight,
    runtimeHandoff,
    runtimeHandoffPackages,
    runtimeCoreArchitectureReadiness,
    runtimeCoreArchitectureGate,
    runtimeCoreArchitecturePackages,
    runtimePrdReadiness,
    runtimePrdCompletionGate,
    runtimePrdCompletionPackages,
    productionEvidenceRequirements,
    productionEvidenceTasks,
    productionEvidenceHandoff,
    productionEvidenceHandoffPackages,
    providerConformancePackages,
    softwareConformancePackages,
    agentConformancePackages,
    integrationConformancePackages,
    outputPackages,
    runtimeDiscovery,
    integrationReadiness,
    workflowContext: workflowContext ?? state.workflowContext,
    desktopRequests,
  });
}

async function fetchRuntimeWorkflowContext(baseUrl, snapshot, runtimeGraph) {
  const workflowId = currentRuntimeWorkflowId(snapshot, runtimeGraph);
  return fetchJson(runtimeWorkflowContextUrl(baseUrl, workflowId)).catch(() => null);
}

function currentRuntimeWorkflowId(snapshot = null, runtimeGraph = state.runtimeGraph) {
  return runtimeGraphWorkflow(runtimeGraph)?.workflow_id
    ?? snapshot?.workflows?.[0]?.id
    ?? state.workflowContext?.workflow_id
    ?? "";
}

async function fetchJson(url, options = {}) {
  const response = await fetch(url, { cache: "no-store", ...options });
  if (!response.ok) throw new Error(`HTTP ${response.status}: ${url}`);
  return response.json();
}

function startRuntimeEventPolling() {
  if (state.snapshot?.mode !== "runtime-http" || eventPollTimer || eventStream) return;
  if (startRuntimeEventWebSocket()) return;
  if (startRuntimeEventStream()) return;
  if (typeof setInterval !== "function") return;
  eventPollTimer = setInterval(refreshRuntimeEvents, 4000);
}

function stopRuntimeEventPolling() {
  if (eventStream) {
    eventStream.onerror = null;
    eventStream.onclose = null;
    eventStream.close();
    eventStream = null;
  }
  stopAgentSessionStreams();
  if (!eventPollTimer || typeof clearInterval !== "function") return;
  clearInterval(eventPollTimer);
  eventPollTimer = null;
}

function stopAgentSessionStreams() {
  agentSessionStreams.forEach((stream) => {
    stream.onerror = null;
    stream.onclose = null;
    stream.close();
  });
  agentSessionStreams.clear();
}

function startRuntimeEventWebSocket() {
  if (eventStream || typeof WebSocket !== "function") return false;
  const runtime = state.snapshot?.runtime ?? runtimeBaseUrl();
  if (!runtime) return false;
  let socket;
  try {
    socket = new WebSocket(runtimeEventsWebSocketUrl(runtime, state.snapshot?.latestEventId));
  } catch {
    return false;
  }
  eventStream = socket;
  socket.onmessage = handleRuntimeEventWebSocketMessage;
  socket.onerror = () => fallbackRuntimeEventStream(socket);
  socket.onclose = () => fallbackRuntimeEventStream(socket);
  return true;
}

function fallbackRuntimeEventStream(stream) {
  if (eventStream !== stream) return;
  eventStream = null;
  if (startRuntimeEventStream()) return;
  if (!eventPollTimer && typeof setInterval === "function") {
    eventPollTimer = setInterval(refreshRuntimeEvents, 4000);
  }
}

function startRuntimeEventStream() {
  if (eventStream || typeof EventSource !== "function") return false;
  const runtime = state.snapshot?.runtime ?? runtimeBaseUrl();
  if (!runtime) return false;
  eventStream = new EventSource(runtimeEventsStreamUrl(runtime, state.snapshot?.latestEventId));
  eventStream.addEventListener("runtime-event", handleRuntimeEventStreamMessage);
  eventStream.addEventListener("cursor", (event) => {
    if (event.data) state.snapshot = { ...state.snapshot, latestEventId: event.data };
  });
  eventStream.onerror = () => {
    if (eventStream) {
      eventStream.close();
      eventStream = null;
    }
    if (!eventPollTimer && typeof setInterval === "function") {
      eventPollTimer = setInterval(refreshRuntimeEvents, 4000);
    }
  };
  return true;
}

function handleRuntimeEventWebSocketMessage(message) {
  try {
    const payload = JSON.parse(message.data);
    if (payload.type === "pool-runtime-events" && payload.latest_event_id) {
      state.snapshot = { ...state.snapshot, latestEventId: payload.latest_event_id };
      return;
    }
    if (payload.type !== "runtime-event" || !payload.event) return;
    mergeRuntimeEvents([payload.event]);
    state.snapshot = { ...state.snapshot, latestEventId: payload.event.id ?? state.snapshot?.latestEventId };
    renderEvents();
  } catch {
    // Malformed WebSocket frames should not interrupt workflow control.
  }
}

function startAgentSessionStream(sessionId) {
  if (!sessionId || agentSessionStreams.has(sessionId)) return false;
  const runtime = state.snapshot?.runtime ?? runtimeBaseUrl();
  if (!runtime || state.snapshot?.mode !== "runtime-http") return false;
  if (startAgentSessionWebSocket(sessionId, runtime)) return true;
  return startAgentSessionEventSource(sessionId, runtime);
}

function startAgentSessionWebSocket(sessionId, runtime) {
  if (typeof WebSocket !== "function") return false;
  let stream;
  try {
    stream = new WebSocket(runtimeAgentWebSocketUrl(runtime, sessionId, state.snapshot?.latestEventId));
  } catch {
    return false;
  }
  stream.onmessage = (event) => handleAgentSessionWebSocketMessage(sessionId, event);
  stream.onerror = () => fallbackAgentSessionStream(sessionId, stream, runtime);
  stream.onclose = () => fallbackAgentSessionStream(sessionId, stream, runtime);
  agentSessionStreams.set(sessionId, stream);
  return true;
}

function startAgentSessionEventSource(sessionId, runtime) {
  if (typeof EventSource !== "function") return false;
  const stream = new EventSource(runtimeAgentStreamUrl(runtime, sessionId, state.snapshot?.latestEventId));
  stream.addEventListener("agent-transcript", (event) => {
    try {
      const transcript = JSON.parse(event.data);
      state.hermes.sessionTranscripts = {
        ...(state.hermes.sessionTranscripts ?? {}),
        [sessionId]: transcript,
      };
      renderHermes();
    } catch {
      // A malformed session frame should not interrupt the global runtime stream.
    }
  });
  stream.addEventListener("runtime-event", handleRuntimeEventStreamMessage);
  stream.addEventListener("cursor", (event) => {
    if (event.data) state.snapshot = { ...state.snapshot, latestEventId: event.data };
  });
  stream.onerror = () => {
    stream.close();
    agentSessionStreams.delete(sessionId);
  };
  agentSessionStreams.set(sessionId, stream);
  return true;
}

function fallbackAgentSessionStream(sessionId, stream, runtime) {
  if (agentSessionStreams.get(sessionId) !== stream) return;
  agentSessionStreams.delete(sessionId);
  startAgentSessionEventSource(sessionId, runtime);
}

function handleRuntimeEventStreamMessage(message) {
  try {
    const event = JSON.parse(message.data);
    mergeRuntimeEvents([event]);
    state.snapshot = { ...state.snapshot, latestEventId: event.id ?? state.snapshot?.latestEventId };
    renderEvents();
  } catch {
    // Malformed stream frames should not interrupt workflow control.
  }
}

function handleAgentSessionWebSocketMessage(sessionId, message) {
  try {
    const payload = JSON.parse(message.data);
    if (payload.type === "agent-session" && payload.transcript) {
      state.hermes.sessionTranscripts = {
        ...(state.hermes.sessionTranscripts ?? {}),
        [sessionId]: payload.transcript,
      };
      if (payload.latest_event_id) {
        state.snapshot = { ...state.snapshot, latestEventId: payload.latest_event_id };
      }
      renderHermes();
      return;
    }
    if (payload.type !== "runtime-event" || !payload.event) return;
    mergeRuntimeEvents([payload.event]);
    state.snapshot = { ...state.snapshot, latestEventId: payload.event.id ?? state.snapshot?.latestEventId };
    renderEvents();
  } catch {
    // Malformed session WebSocket frames should not interrupt workflow control.
  }
}

async function refreshRuntimeEvents() {
  const runtime = state.snapshot?.runtime ?? runtimeBaseUrl();
  if (!runtime || state.snapshot?.mode !== "runtime-http") return;
  try {
    const result = await fetchJson(runtimeEventsUrl(runtime, state.snapshot?.latestEventId));
    mergeRuntimeEvents(result.events ?? []);
    if (result.latest_event_id) {
      state.snapshot = { ...state.snapshot, latestEventId: result.latest_event_id };
    }
    renderEvents();
  } catch {
    // Event polling should not interrupt active workflow control.
  }
}

function mergeRuntimeEvents(events) {
  if (!Array.isArray(events) || !events.length) return;
  const merged = new Map();
  [...events.map(runtimeEvent), ...state.events].forEach((event) => {
    const id = event.id ?? `${event.at}:${event.text}`;
    if (!merged.has(id)) merged.set(id, event);
  });
  state.events = [...merged.values()].slice(0, 48);
}

function mergeRuntimeProjects(payload) {
  if (!Array.isArray(payload?.projects)) return;
  state.projects = payload.projects.map(runtimeProject).filter((project) => project.slug);
}

function runtimeProject(project) {
  return {
    id: project.id,
    slug: project.slug ?? "",
    name: project.name ?? project.slug ?? "Untitled project",
    status: project.status ?? "active",
    createdAt: project.created_at,
    updatedAt: project.updated_at,
  };
}

function activeProjectSlug() {
  const filter = state.snapshot?.projectFilter;
  if (filter && filter !== "*") return filter;
  return state.projects[0]?.slug ?? "demo";
}

function currentProjectOptionValue() {
  const filter = state.snapshot?.projectFilter ?? runtimeProjectFilter();
  if (!filter || filter === "*") return "*";
  return filter;
}

function mergeRuntimeAdapterRegistry(registry) {
  if (!registry || typeof registry !== "object") return;
  if (registry.provider_aliases && typeof registry.provider_aliases === "object") {
    state.providerAliases = { ...state.providerAliases, ...registry.provider_aliases };
  }
  if (Array.isArray(registry.providers)) {
    const existing = new Map(state.apiProviders.map((provider) => [canonicalProviderId(provider.id), provider]));
    state.apiProviders = registry.providers
      .filter((provider) => ["ai", "3dgs"].includes(providerGroup(provider.kind)))
      .map((provider) => runtimeProviderConfig(provider, existing.get(canonicalProviderId(provider.id))));
  }
  if (Array.isArray(registry.software_adapters)) {
    const existing = new Map(state.software.map((software) => [softwareAdapterId(software.name), software]));
    state.software = registry.software_adapters.map((adapter) => runtimeSoftwareConfig(adapter, existing.get(adapter.id)));
  }
}

function mergeRuntimeProviderContracts(payload) {
  if (!payload || typeof payload !== "object") return;
  const contracts = Array.isArray(payload.contracts) ? payload.contracts : [payload];
  const nextContracts = { ...(state.providerContracts ?? {}) };
  contracts
    .filter((contract) => contract && typeof contract === "object" && contract.provider_id)
    .forEach((contract) => {
      nextContracts[canonicalProviderId(contract.provider_id)] = contract;
    });
  state.providerContracts = nextContracts;
  state.apiProviders = state.apiProviders.map((provider) => ({
    ...provider,
    contract: nextContracts[canonicalProviderId(provider.id)] ?? provider.contract,
  }));
}

function mergeRuntimeProviderGatewayWorkerContract(payload) {
  if (!payload || typeof payload !== "object") return;
  const contract = payload.contract && typeof payload.contract === "object" ? payload.contract : payload;
  if (contract.kind !== "pool_provider_gateway_worker_contract") return;
  state.providerGatewayWorkerContract = contract;
}

function mergeRuntimeDesktopRecognitionContract(payload) {
  if (!payload || typeof payload !== "object") return;
  const contract = payload.contract && typeof payload.contract === "object" ? payload.contract : payload;
  if (contract.kind !== "pool_desktop_recognition_contract") return;
  state.desktopRecognitionContract = contract;
}

function mergeRuntimeSoftwareContracts(payload) {
  if (!payload || typeof payload !== "object") return;
  const contracts = Array.isArray(payload.contracts) ? payload.contracts : [payload];
  const nextContracts = { ...(state.softwareContracts ?? {}) };
  contracts
    .filter((contract) => contract && typeof contract === "object" && contract.adapter_id)
    .forEach((contract) => {
      nextContracts[softwareAdapterId(contract.adapter_id)] = contract;
    });
  state.softwareContracts = nextContracts;
  state.software = state.software.map((software) => ({
    ...software,
    contract: nextContracts[softwareAdapterId(software.id ?? software.name)] ?? software.contract,
  }));
}

function mergeRuntimePrompts(payload) {
  if (!Array.isArray(payload?.prompts) || !payload.prompts.length) return;
  state.hermes.runbooks = payload.prompts.map(runtimePromptDefinition);
  if (!state.hermes.runbooks.some((runbook) => runbook.name === state.hermes.selectedRunbook)) {
    state.hermes.selectedRunbook = state.hermes.runbooks[0].name;
  }
  state.hermes.runbookStatus = "runtime";
}

function runtimePromptDefinition(prompt) {
  return {
    name: prompt.name,
    title: prompt.title ?? prompt.name,
    description: prompt.description ?? "",
    arguments: Array.isArray(prompt.arguments) ? prompt.arguments : [],
  };
}

function runtimeProviderConfig(config, existing = {}) {
  return {
    id: config.id,
    group: providerGroup(config.kind),
    name: config.display_name ?? existing.name ?? config.id,
    mode: providerModeLabel(config),
    endpoint: config.endpoint ?? existing.endpoint ?? "provider://runtime",
    runtimeEndpoint: existing.runtimeEndpoint,
    auth: config.auth_env_key ?? existing.auth ?? "local",
    output: config.output_contract ?? existing.output ?? "local files",
    status: existing.status ?? (config.high_cost ? "guarded" : "checking"),
    cost: config.high_cost ? 9000 : existing.cost ?? 1500,
    runtimeRegistered: true,
    contract: existing.contract ?? state.providerContracts?.[canonicalProviderId(config.id)] ?? null,
  };
}

function providerGroup(kind) {
  return {
    AiImage: "ai",
    AiVideo: "ai",
    Audio: "ai",
    ThreeDgs: "3dgs",
    ai_image: "ai",
    ai_video: "ai",
    audio: "ai",
    three_dgs: "3dgs",
  }[kind] ?? "other";
}

function providerModeLabel(config) {
  if (config.id === "comfyui") return "HTTP API / WebSocket / Workflow JSON";
  if (config.id === "openai-image-2") return "REST API / Images";
  if (["midjourney", "nano-banana-pro", "suno"].includes(config.id)) return "Generic HTTP Media Gateway";
  if (providerGroup(config.kind) === "3dgs") return "3DGS Gateway / local mock fallback";
  return "ProviderAdapter";
}

function runtimeSoftwareConfig(config, existing = {}) {
  return {
    id: config.id,
    name: config.display_name ?? existing.name ?? config.id,
    priority: existing.priority ?? softwarePriorityLabel(config),
    mode: (config.control_modes ?? []).join(" / ") || existing.mode || "adapter",
    scope: existing.scope ?? softwareScope(config.id),
    runtimeEndpoint: existing.runtimeEndpoint,
    health: existing.health ?? (config.desktop_fallback ? "checking" : "planned"),
    latency: existing.latency ?? "registry",
    runtimeRegistered: true,
    contract: existing.contract ?? state.softwareContracts?.[softwareAdapterId(config.id)] ?? null,
  };
}

function softwarePriorityLabel(config) {
  if (config.id === "unreal") return "深度集成";
  if (config.id === "hermes") return "Agent 控制";
  if (config.id === "comfyui") return "AI 工作流";
  if (config.desktop_fallback) return "可接管";
  return `P${config.priority ?? "-"}`;
}

function softwareScope(id) {
  return {
    unreal: "关卡拼装、蓝图、Sequencer、运行视口",
    blender: "模型修复、材质、绑定、格式转换",
    comfyui: "图像、视频、队列、结果回写",
    resolve: "素材池、时间线、调色、转码",
    unity: "Prefab、Scene、Timeline、构建",
    touchdesigner: "节点网络、实时视觉、设备参数",
    madmapper: "投影映射、媒体触发、灯光输出",
    nuke: "节点合成、批量渲染、镜头回写",
    "motion-db": "动作搜索、FBX/BVH/GLB、重定向",
    "editing-suite": "Premiere、Final Cut、CapCut 辅助控制",
    hermes: "Agent 编排、MCP 转发、控制指令审计",
  }[id] ?? "外部软件控制与审计";
}

function mergeRuntimeSnapshot(snapshot, url, options = {}) {
  if (!snapshot || !Array.isArray(snapshot.tasks)) return;
  if (options.mode !== "runtime-http" || !state.projects.length) {
    mergeRuntimeProjects(snapshot);
  }
  const runtimeGraph = options.runtimeGraph ?? state.runtimeGraph;
  const graphWorkflow = runtimeGraphWorkflow(runtimeGraph);
  const workflow = snapshot.workflows?.[0];
  const workflowNodes = workflow?.nodes && typeof workflow.nodes === "object" ? Object.values(workflow.nodes) : [];
  const nodeStateById = new Map((snapshot.node_states ?? []).map((node) => [node.node_id, node]));

  if (graphWorkflow?.nodes?.length) {
    state.runtimeGraph = runtimeGraph;
    const layout = runtimeWorkflowLayout(graphWorkflow.nodes);
    state.nodes = graphWorkflow.nodes.map((node, index) => runtimeGraphNode(node, snapshot, index, layout));
    state.connections = (graphWorkflow.edges ?? []).map((connection) => runtimeGraphConnection(connection, state.nodes));
    state.selectedNode = state.nodes.find((node) => node.id === state.selectedNode)?.id ?? state.nodes[0]?.id ?? state.selectedNode;
  } else if (workflowNodes.length) {
    const layout = runtimeWorkflowLayout(workflowNodes);
    state.nodes = workflowNodes.map((node, index) => runtimeNode(node, nodeStateById.get(node.id), index, layout));
    state.connections = (workflow.connections ?? []).map((connection) => runtimeConnection(connection, state.nodes));
    state.selectedNode = state.nodes[0]?.id ?? state.selectedNode;
  }
  if (state.selectedNodeContext?.node_id !== state.selectedNode) {
    state.selectedNodeContext = null;
    state.selectedNodeContextStatus = "idle";
    state.selectedNodeContextError = "";
  }
  if (options.mode === "runtime-http") {
    state.workflowContext = options.workflowContext ?? state.workflowContext ?? null;
    state.workflowContextStatus = state.workflowContext ? "loaded" : "idle";
    state.workflowContextError = state.workflowContext ? "" : state.workflowContextError;
  } else {
    state.workflowContext = null;
    state.workflowContextStatus = "idle";
    state.workflowContextError = "";
  }

  state.tasks = snapshot.tasks.map((task) => runtimeTask(task, state.selectedNode));
  state.assets = (snapshot.assets ?? []).map((asset) => runtimeAsset(asset, state.selectedNode));
  state.events = (snapshot.events ?? []).map(runtimeEvent);
  state.apiKeys = snapshot.api_keys ?? [];
  state.apiKeyAudit = normalizeApiKeyAudit(options.apiKeyAudit ?? deriveApiKeyAuditFromSnapshot(snapshot));
  state.providerRequests = (snapshot.provider_requests ?? []).map(runtimeProviderRequest);
  state.runtimeExecutionPlan = options.runtimeExecutionPlan
    ? normalizeRuntimeExecutionPlan(options.runtimeExecutionPlan)
    : null;
  state.runtimeBudget = normalizeRuntimeBudget(options.runtimeBudget ?? deriveRuntimeBudgetFromSnapshot(snapshot));
  state.runtimePreflight = normalizeRuntimePreflight(options.runtimePreflight ?? deriveRuntimePreflightFromSnapshot(snapshot));
  state.runtimeCoreArchitectureReadiness = normalizeRuntimeCoreArchitectureReadiness(options.runtimeCoreArchitectureReadiness);
  state.runtimeCoreArchitectureGate = normalizeRuntimeCoreArchitectureGate(options.runtimeCoreArchitectureGate);
  state.runtimePrdReadiness = normalizeRuntimePrdReadiness(options.runtimePrdReadiness);
  state.runtimePrdCompletionGate = normalizeRuntimePrdCompletionGate(options.runtimePrdCompletionGate);
  if (options.runtimePrdCompletionPackages) mergeRuntimePrdCompletionPackages(options.runtimePrdCompletionPackages);
  state.productionEvidenceRequirements = normalizeProductionEvidenceRequirements(options.productionEvidenceRequirements);
  state.productionEvidenceTasks = normalizeProductionEvidenceTasks(options.productionEvidenceTasks);
  state.productionEvidenceHandoff = normalizeProductionEvidenceHandoff(options.productionEvidenceHandoff);
  if (options.productionEvidenceHandoffPackages) mergeProductionEvidenceHandoffPackages(options.productionEvidenceHandoffPackages);
  if (options.providerConformancePackages) mergeConformancePackageCatalog("provider", options.providerConformancePackages);
  if (options.softwareConformancePackages) mergeConformancePackageCatalog("software", options.softwareConformancePackages);
  if (options.agentConformancePackages) mergeConformancePackageCatalog("agent", options.agentConformancePackages);
  if (options.integrationConformancePackages) mergeConformancePackageCatalog("integration", options.integrationConformancePackages);
  state.integrationReadiness = normalizeIntegrationReadiness(options.integrationReadiness);
  state.softwareActions = (snapshot.software_actions ?? []).map(runtimeSoftwareAction);
  state.runtimeDiscovery = normalizeRuntimeDiscovery(options.runtimeDiscovery);
  state.desktopRecognitionRequests = Array.isArray(options.desktopRequests?.requests)
    ? options.desktopRequests.requests.map(runtimeDesktopRecognitionRequest)
    : deriveDesktopRecognitionRequestsFromSoftwareActions();
  state.runtimeHandoff = normalizeRuntimeHandoff(options.runtimeHandoff ?? deriveRuntimeHandoffFromState());
  if (options.runtimeHandoffPackages) mergeRuntimeHandoffPackages(options.runtimeHandoffPackages);
  if (options.runtimeCoreArchitecturePackages) mergeRuntimeCoreArchitecturePackages(options.runtimeCoreArchitecturePackages);
  if (!mergeOutputPackages(options.outputPackages)) mergeOutputManifestsFromSnapshot(snapshot);
  mergeAgentSessions(snapshot);
  state.tokenTotal = snapshot.stats?.token_total ?? state.tasks.reduce((total, task) => total + (task.cost ?? 0), 0);
  if (snapshot.stats?.agent_token_budget > 0) {
    state.budgetLimit = snapshot.stats.agent_token_budget;
  }
  state.snapshot = {
    source: url,
    mode: options.mode ?? "snapshot-file",
    runtime: options.runtime,
    projectFilter: snapshot.project_filter,
    latestEventId: snapshot.events?.[0]?.id,
    version: snapshot.version,
    generatedAt: snapshot.generated_at,
    stats: snapshot.stats,
    health: options.health,
    runtimeGraphSummary: runtimeGraph?.summary,
    workflowContextSummary: state.workflowContext?.summary,
  };
  const sourceLabel = options.mode === "runtime-http" ? `Runtime HTTP ${options.runtime}` : "RuntimeSnapshot 文件";
  addEvent("ok", `${sourceLabel} 已加载：${snapshot.project_filter ?? "all projects"}`);
  if (state.snapshot.mode === "runtime-http") {
    startRuntimeEventPolling();
  } else {
    stopRuntimeEventPolling();
  }
}

function mergeAgentSessions(snapshot) {
  const decisions = runtimeAgentDecisions(snapshot);
  state.hermes.decisions = decisions;
  state.hermes.sessions = decisions;
  if (!decisions.length) return;
  const latest = decisions[0];
  state.hermes.status = latest.status;
  state.hermes.trace = [
    `${latest.at} -> Runtime Agent: ${latest.title} / ${statusLabel(latest.status)} / ${latest.tokenUsed} tokens`,
    ...state.hermes.trace.filter((line) => !line.includes("Runtime Agent:")).slice(0, 7),
  ];
}

function runtimeAgentDecisions(snapshot) {
  const tasks = snapshot.tasks ?? [];
  return (snapshot.agent_sessions ?? [])
    .map((session) => {
      const transcriptPath = session.transcript_path ?? "";
      const task = tasks.find((item) => item.request_metadata_path === transcriptPath);
      const status = normalizeRuntimeStatus(task?.status ?? "ready");
      return {
        id: session.id,
        title: task?.title ?? "Agent/Hermes decision",
        status,
        at: formatSnapshotTime(session.updated_at ?? session.created_at),
        tokenUsed: session.token_used ?? 0,
        tokenBudget: session.token_budget,
        tools: runtimeTools(session.tools),
        transcriptPath,
      };
    })
    .sort((a, b) => String(b.at).localeCompare(String(a.at)))
    .slice(0, 4);
}

function runtimeTools(tools) {
  if (Array.isArray(tools)) return tools;
  if (tools && typeof tools === "object") return Object.values(tools).map(String);
  return [];
}

function applyWorkflowRunReport(report) {
  if (!report) return;
  state.hermes.workflowReport = {
    agentMode: report.agent_mode ?? "stage",
    threeDgsMode: report.three_dgs_mode ?? "auto",
    unrealMode: report.unreal_mode ?? "auto",
    agentStatus: normalizeRuntimeStatus(report.agent_report?.status ?? "ready"),
    providerStatus: normalizeRuntimeStatus(report.provider_report?.status),
    softwareStatus: normalizeRuntimeStatus(report.software_report?.status),
    outputStatus: normalizeRuntimeStatus(report.output_report?.status),
    assetsIndexed: report.assets_indexed ?? 0,
    transcriptPath: report.agent_report?.transcript_path,
  };
  state.hermes.trace.unshift(
    `${nowTime()} -> ContentBurst: agent=${state.hermes.workflowReport.agentMode}, 3DGS=${state.hermes.workflowReport.threeDgsMode}, Unreal=${state.hermes.workflowReport.unrealMode}`,
  );
  applyOutputPackageReport(report.output_report);
}

function applyOutputPackageReport(report) {
  if (!report?.manifests?.length) return;
  state.outputManifests = report.manifests.map(runtimeOutputManifest);
}

function applyOutputPackageResultReport(report) {
  if (!report) return;
  if (mergeOutputPackages(report.catalog)) return;
  if (!report.target) return;

  const index = state.outputManifests.findIndex((manifest) => manifest.target === report.target);
  if (index === -1) return;
  const current = state.outputManifests[index];
  state.outputManifests[index] = {
    ...current,
    status: normalizeRuntimeStatus(report.status ?? current.status),
    metrics: mergeOutputMetrics(current.metrics, executionMetricsFromManifest(report.manifest)),
  };
}

function mergeOutputPackages(outputPackages) {
  if (!Array.isArray(outputPackages?.deliverables)) return false;
  const manifests = outputPackages.deliverables
    .filter((deliverable) => deliverable.asset_id || deliverable.status !== "missing")
    .map(runtimeOutputManifestFromDeliverable);
  if (!manifests.length) return false;
  state.outputManifests = manifests;
  return true;
}

function mergeOutputManifestsFromSnapshot(snapshot) {
  const inferred = (snapshot.assets ?? [])
    .filter((asset) => String(asset.local_path ?? "").includes("/deliverables/"))
    .map(runtimeOutputManifestFromAsset)
    .filter(Boolean);
  if (inferred.length) state.outputManifests = inferred;
}

function runtimeOutputManifest(manifest) {
  return {
    target: manifest.target ?? outputTargetFromPath(manifest.local_path),
    title: manifest.title ?? outputTitleForTarget(manifest.target),
    localPath: manifest.local_path ?? "",
    primaryRuntime: manifest.primary_runtime ?? outputRuntimeForTarget(manifest.target),
    status: normalizeRuntimeStatus(manifest.status ?? "ready"),
    metrics: (manifest.metrics ?? []).map((metric) => ({
      label: metric.label ?? "metric",
      value: metric.value ?? "",
    })),
  };
}

function runtimeOutputManifestFromDeliverable(deliverable) {
  return {
    target: deliverable.target ?? outputTargetFromPath(deliverable.local_path),
    title: deliverable.title ?? outputTitleForTarget(deliverable.target),
    localPath: deliverable.local_path ?? "",
    primaryRuntime: deliverable.primary_runtime ?? outputRuntimeForTarget(deliverable.target),
    status: normalizeRuntimeStatus(deliverable.status),
    metrics: (deliverable.metrics ?? []).map((metric) => ({
      label: metric.label ?? "metric",
      value: metric.value ?? "",
    })),
  };
}

function runtimeOutputManifestFromAsset(asset) {
  const path = asset.local_path ?? "";
  const target = outputTargetFromPath(path);
  if (!target) return null;
  return {
    target,
    title: outputTitleForTarget(target),
    localPath: path,
    primaryRuntime: outputRuntimeForTarget(target),
    metrics: [
      { label: "status", value: normalizeRuntimeStatus(asset.status) },
      { label: "asset", value: asset.name ?? "manifest" },
    ],
  };
}

function mergeOutputMetrics(existing = [], incoming = []) {
  const byLabel = new Map(existing.map((metric) => [metric.label, metric]));
  incoming.forEach((metric) => byLabel.set(metric.label, metric));
  return Array.from(byLabel.values());
}

function executionMetricsFromManifest(manifest) {
  const result = manifest?.execution_result;
  if (!result) return [];
  return [
    result.status ? { label: "execution", value: result.status } : null,
    result.runtime ? { label: "runtime_result", value: result.runtime } : null,
    result.adapter_id ? { label: "adapter", value: result.adapter_id } : null,
    Array.isArray(result.artifacts) ? { label: "artifacts", value: String(result.artifacts.length) } : null,
    result.message ? { label: "message", value: result.message } : null,
  ].filter(Boolean);
}

function outputManifestMetric(manifest, label) {
  return (manifest.metrics ?? []).find((metric) => metric.label === label)?.value ?? "";
}

function outputExecutionStatus(manifest) {
  return outputManifestMetric(manifest, "execution");
}

function outputTargetFromPath(path = "") {
  if (path.includes("video-timeline")) return "video";
  if (path.includes("game-build")) return "game";
  if (path.includes("interactive-cues")) return "interactive_art";
  return "";
}

function outputTitleForTarget(target = "") {
  return {
    video: "时间线与转码",
    game: "运行原型",
    interactive_art: "节点与现场控制",
  }[target] ?? "输出 Manifest";
}

function outputRuntimeForTarget(target = "") {
  return {
    video: "DaVinci Resolve / FFmpeg",
    game: "Unreal",
    interactive_art: "TouchDesigner / MadMapper",
  }[target] ?? "Runtime";
}

function outputAdapterForTarget(target = "") {
  return {
    video: "resolve",
    game: "unreal",
    interactive_art: "touchdesigner",
  }[target] ?? "output-runtime";
}

function runtimeWorkflowLayout(nodes) {
  const positions = nodes
    .map((node) => node.position)
    .filter((position) => Number.isFinite(position?.x) && Number.isFinite(position?.y));
  if (!positions.length) return null;
  const maxX = Math.max(...positions.map((position) => position.x));
  const maxY = Math.max(...positions.map((position) => position.y));
  return {
    scaleX: Math.min(1, 760 / Math.max(maxX, 760)),
    scaleY: Math.min(1, 430 / Math.max(maxY, 430)),
    offsetX: 40,
    offsetY: 40,
  };
}

function runtimeGraphWorkflow(runtimeGraph) {
  return Array.isArray(runtimeGraph?.workflows) ? runtimeGraph.workflows[0] : null;
}

function runtimeGraphNode(node, snapshot, index, layout = null) {
  const task = latestTaskForGraphNode(node, snapshot);
  const status = normalizeRuntimeStatus(task?.status ?? node.status ?? node.static_status);
  const adapter = node.provider_id ?? node.software_adapter_id ?? task?.provider_id ?? "runtime";
  const position = runtimeNodePosition(node, index, layout);
  const taskType = node.task_type ?? runtimeTaskTypeForNodeType(node.node_type);
  return {
    id: node.id,
    type: node.node_type ?? "Runtime Node",
    taskType,
    taskTypeLabel: taskTypeLabel(taskType),
    title: node.title ?? `Runtime Node ${index + 1}`,
    status,
    agent: adapter,
    control: node.software_adapter_id
      ? `SoftwareAdapter / ${node.software_adapter_id}`
      : node.provider_id
        ? `ProviderAdapter / ${node.provider_id}`
        : "Runtime",
    input: "RuntimeGraph",
    output: node.blocked_by_approval ? "ApprovalGate" : `${node.asset_count ?? 0} assets`,
    progress: progressForStatus(status),
    cost: task?.cost_estimate_tokens ?? node.cost_estimate_tokens ?? 0,
    x: position.x,
    y: position.y,
    canRun: node.can_run,
    blockedByApproval: node.blocked_by_approval,
    providerRequestCount: node.provider_request_count ?? 0,
    softwareActionCount: node.software_action_count ?? 0,
    log: [
      `${node.title ?? node.id} / ${taskTypeLabel(taskType)} / ${statusLabel(status)}`,
      task?.updated_at ? `updated ${formatSnapshotTime(task.updated_at)}` : "来自 RuntimeGraph 可执行运行图。",
      node.blocked_by_approval ? "需要审批或人工确认。" : "",
    ]
      .filter(Boolean)
      .join(" / "),
  };
}

function latestTaskForGraphNode(node, snapshot) {
  const tasks = snapshot?.tasks ?? [];
  return tasks.find((task) => task.id === node.latest_task?.id)
    ?? tasks.find((task) => task.node_id === node.id)
    ?? node.latest_task
    ?? node.tasks?.[0]
    ?? null;
}

function runtimeGraphConnection(connection, nodes) {
  const from = nodes.find((node) => node.id === connection.from_node_id);
  const to = nodes.find((node) => node.id === connection.to_node_id);
  const kind = normalizeConnectionKind(connection.kind ?? connection.channel);
  return {
    id: connection.id,
    fromId: connection.from_node_id,
    toId: connection.to_node_id,
    kind,
    channel: connection.channel ?? kind,
    channelLabel: connectionChannelLabel(connection.channel ?? kind),
    label: connection.label ?? "",
    from,
    to,
  };
}

function runtimeNode(node, nodeState, index, layout = null) {
  const status = normalizeRuntimeStatus(nodeState?.status ?? node.status);
  const adapter = node.provider_id ?? node.software_adapter_id ?? nodeState?.provider_id ?? "runtime";
  const taskType = runtimeTaskTypeForNodeType(node.node_type);
  const position = runtimeNodePosition(node, index, layout);
  return {
    id: node.id,
    type: node.node_type ?? "Runtime Node",
    taskType,
    taskTypeLabel: taskTypeLabel(taskType),
    title: node.title ?? `Runtime Node ${index + 1}`,
    status,
    agent: adapter,
    control: node.software_adapter_id ? "SoftwareAdapter" : node.provider_id ? "ProviderAdapter" : "Runtime",
    input: "RuntimeSnapshot",
    output: node.requires_approval ? "ApprovalGate" : "TaskState",
    progress: progressForStatus(status),
    cost: nodeState?.cost_estimate_tokens ?? node.cost_estimate_tokens ?? 0,
    x: position.x,
    y: position.y,
    log: nodeState
      ? `${nodeState.title} / ${statusLabel(status)} / ${nodeState.updated_at}`
      : "来自 RuntimeSnapshot workflow nodes。",
  };
}

function runtimeNodePosition(node, index, layout) {
  if (layout && Number.isFinite(node.position?.x) && Number.isFinite(node.position?.y)) {
    return {
      x: Math.round(layout.offsetX + node.position.x * layout.scaleX),
      y: Math.round(layout.offsetY + node.position.y * layout.scaleY),
    };
  }
  return {
    x: 80 + (index % 4) * 280,
    y: 80 + Math.floor(index / 4) * 170,
  };
}

function runtimeConnection(connection, nodes) {
  const from = nodes.find((node) => node.id === connection.from_node_id);
  const to = nodes.find((node) => node.id === connection.to_node_id);
  return {
    id: connection.id,
    fromId: connection.from_node_id,
    toId: connection.to_node_id,
    kind: normalizeConnectionKind(connection.kind),
    channel: normalizeConnectionKind(connection.kind),
    channelLabel: connectionChannelLabel(connection.kind),
    label: connection.label ?? "",
    from,
    to,
  };
}

function normalizeConnectionKind(kind) {
  return {
    AssetFlow: "asset",
    ControlFlow: "control",
    AgentInstruction: "agent",
    FeedbackLoop: "feedback",
    Approval: "gate",
    asset_flow: "asset",
    asset: "asset",
    control_flow: "control",
    control: "control",
    agent_instruction: "agent",
    agent: "agent",
    feedback_loop: "feedback",
    feedback: "feedback",
    approval: "gate",
    gate: "gate",
  }[kind] ?? "control";
}

function connectionChannelLabel(channel = "") {
  return {
    AssetFlow: "资产流",
    ControlFlow: "控制流",
    AgentInstruction: "Agent 指令",
    FeedbackLoop: "反馈循环",
    Approval: "审批门",
    asset: "资产流",
    control: "控制流",
    agent: "Agent 指令",
    agent_instruction: "Agent 指令",
    feedback: "反馈循环",
    approval: "审批门",
    gate: "审批门",
  }[channel] ?? "控制流";
}

function runtimeTaskTypeForNodeType(nodeType = "") {
  const text = String(nodeType).toLowerCase();
  if (text.includes("agent") || text.includes("hermes")) return "agent";
  if (text.includes("three") || text.includes("3dgs")) return "3dgs";
  if (text.includes("image") || text.includes("video") || text.includes("audio") || text.includes("comfy") || text.includes("suno")) return "ai_provider";
  if (text.includes("asset")) return "asset_package";
  if (text.includes("output")) return "output";
  if (text.includes("approval")) return "approval";
  if (text.includes("unreal") || text.includes("blender") || text.includes("resolve") || text.includes("unity") || text.includes("touch") || text.includes("madmapper") || text.includes("nuke") || text.includes("motion")) return "software_control";
  return "creative_input";
}

function taskTypeLabel(taskType = "") {
  return {
    agent: "Agent",
    ai_provider: "AI Provider",
    "3dgs": "3DGS",
    asset_package: "资产包",
    software_control: "软件控制",
    approval: "审批",
    output: "输出",
    creative_input: "输入",
  }[taskType] ?? "任务";
}

function runtimeTask(task, fallbackNodeId) {
  const status = normalizeRuntimeStatus(task.status);
  const cost = task.cost_estimate_tokens ?? 0;
  return {
    id: task.id,
    nodeId: task.node_id ?? fallbackNodeId,
    title: task.title,
    type: task.provider_id ?? "runtime-task",
    status,
    tool: task.provider_id ?? "runtime",
    risk: task.requires_approval || cost >= 6000 ? "high" : "medium",
    cost,
    requestMetadataPath: task.request_metadata_path,
  };
}

function runtimeAsset(asset, fallbackNodeId) {
  return {
    id: asset.id,
    name: asset.name,
    type: asset.asset_type,
    path: asset.local_path,
    source: asset.source_node_id ?? fallbackNodeId,
    status: normalizeRuntimeStatus(asset.status),
  };
}

function runtimeEvent(event) {
  return {
    id: event.id,
    at: formatSnapshotTime(event.created_at),
    createdAt: event.created_at,
    level: normalizeEventLevel(event.level),
    text: event.message,
  };
}

function runtimeProviderRequest(entry) {
  const request = entry.request ?? {};
  const providerRequest = request.provider_request ?? {};
  const response = entry.response ?? {};
  return {
    id: entry.id,
    taskId: entry.task_id,
    projectSlug: entry.project_slug,
    providerId: canonicalProviderId(entry.provider_id ?? request.provider_id ?? "provider"),
    executionMode: request.execution_mode ?? "auto",
    status: normalizeRuntimeStatus(response.status ?? request.task?.status ?? "queued"),
    requiresApproval: Boolean(providerRequest.require_approval ?? request.task?.requires_approval),
    prompt: compactPrompt(providerRequest.prompt ?? request.prompt ?? ""),
    metadataPath: entry.metadata_path ?? request.task?.request_metadata_path ?? "",
    createdAt: formatSnapshotTime(entry.created_at),
  };
}

function normalizeRuntimeBudget(budget) {
  const summary = budget?.summary ?? {};
  return {
    projectFilter: budget?.project_filter ?? budget?.projectFilter ?? state.snapshot?.projectFilter,
    generatedAt: budget?.generated_at ?? budget?.generatedAt,
    summary: {
      taskEstimatedTokens: numberValue(summary.task_estimated_tokens ?? summary.taskEstimatedTokens),
      waitingApprovalEstimatedTokens: numberValue(
        summary.waiting_approval_estimated_tokens ?? summary.waitingApprovalEstimatedTokens,
      ),
      agentTokenUsed: numberValue(summary.agent_token_used ?? summary.agentTokenUsed),
      agentTokenBudget: numberValue(summary.agent_token_budget ?? summary.agentTokenBudget),
      tokenTotal: numberValue(summary.token_total ?? summary.tokenTotal),
      budgetRemaining: summary.budget_remaining ?? summary.budgetRemaining ?? null,
      configuredApiKeys: numberValue(summary.configured_api_keys ?? summary.configuredApiKeys),
      trackedProviders: numberValue(summary.tracked_providers ?? summary.trackedProviders),
      missingRuntimeCredentials: numberValue(
        summary.missing_runtime_credentials ?? summary.missingRuntimeCredentials,
      ),
      providerRequests: numberValue(summary.provider_requests ?? summary.providerRequests),
      approvalGates: numberValue(summary.approval_gates ?? summary.approvalGates),
    },
    providerCredentials: (budget?.provider_credentials ?? budget?.providerCredentials ?? []).map(runtimeBudgetProvider),
    approvalGates: budget?.approval_gates ?? budget?.approvalGates ?? [],
  };
}

function deriveRuntimeBudgetFromSnapshot(snapshot) {
  const providerIds = new Set();
  const configuredKeys = (snapshot.api_keys ?? []).filter((key) => key.service_type === "provider" && key.configured);
  configuredKeys.forEach((key) => providerIds.add(canonicalProviderId(key.provider)));
  (snapshot.tasks ?? []).forEach((task) => {
    if (task.provider_id) providerIds.add(canonicalProviderId(task.provider_id));
  });
  (snapshot.provider_requests ?? []).forEach((request) => providerIds.add(canonicalProviderId(request.provider_id)));
  const providerCredentials = [...providerIds]
    .filter(Boolean)
    .sort()
    .map((providerId) => {
      const key = configuredKeys.find((item) => canonicalProviderId(item.provider) === providerId);
      const tasks = (snapshot.tasks ?? []).filter((task) => canonicalProviderId(task.provider_id ?? "") === providerId);
      const requests = (snapshot.provider_requests ?? []).filter(
        (request) => canonicalProviderId(request.provider_id ?? "") === providerId,
      );
      return {
        provider_id: providerId,
        configured: Boolean(key?.configured),
        key_hint: key?.key_hint,
        task_count: tasks.length,
        provider_request_count: requests.length,
        token_estimate: tasks.reduce((total, task) => total + (task.cost_estimate_tokens ?? 0), 0),
        waiting_approval_tokens: tasks
          .filter((task) => task.status === "WaitingApproval")
          .reduce((total, task) => total + (task.cost_estimate_tokens ?? 0), 0),
        last_request_at: requests[0]?.created_at,
      };
    });
  const approvalGates = (snapshot.tasks ?? []).filter((task) => task.requires_approval || task.status === "WaitingApproval");
  return {
    project_filter: snapshot.project_filter,
    generated_at: snapshot.generated_at,
    summary: {
      task_estimated_tokens: snapshot.stats?.task_estimated_tokens ?? 0,
      waiting_approval_estimated_tokens: snapshot.stats?.waiting_approval_estimated_tokens ?? 0,
      agent_token_used: snapshot.stats?.agent_token_used ?? 0,
      agent_token_budget: snapshot.stats?.agent_token_budget ?? 0,
      token_total: snapshot.stats?.token_total ?? 0,
      budget_remaining: snapshot.stats?.budget_remaining ?? null,
      configured_api_keys: configuredKeys.length,
      tracked_providers: providerCredentials.length,
      missing_runtime_credentials: providerCredentials.filter((provider) => !provider.configured).length,
      provider_requests: snapshot.provider_requests?.length ?? 0,
      approval_gates: approvalGates.length,
    },
    provider_credentials: providerCredentials,
    approval_gates: approvalGates,
  };
}

function normalizeApiKeyAudit(payload) {
  const audit = payload?.audit ?? payload;
  if (!audit || typeof audit !== "object") {
    return deriveApiKeyAuditFromSnapshot({ api_keys: state.apiKeys ?? [] });
  }
  const items = (audit.items ?? []).map(apiKeyAuditItem);
  return {
    kind: audit.kind ?? "pool_api_key_audit",
    defaultRotationDays: numberValue(audit.default_rotation_days ?? audit.defaultRotationDays ?? 90),
    total: numberValue(audit.total ?? items.length),
    configured: numberValue(audit.configured ?? items.filter((item) => item.configured).length),
    rotationDue: numberValue(audit.rotation_due ?? audit.rotationDue ?? items.filter((item) => item.rotationDue).length),
    unencrypted: numberValue(audit.unencrypted ?? items.filter((item) => !item.encrypted).length),
    items,
  };
}

function deriveApiKeyAuditFromSnapshot(snapshot) {
  const items = (snapshot.api_keys ?? []).map((key) => {
    const metadata = key.metadata ?? {};
    const credential = metadata.credential ?? {};
    const rotationDays = metadata.rotation_days === 0 ? 0 : numberValue(metadata.rotation_days ?? 90);
    const ageDays = apiKeyAgeDays(key.updated_at);
    const rotationDue = ageDays === null ? Boolean(key.configured) : ageDays >= rotationDays;
    return apiKeyAuditItem({
      provider: key.provider,
      service_type: key.service_type,
      configured: key.configured,
      key_hint: key.key_hint,
      source: metadata.source,
      env: metadata.env,
      owner: metadata.owner,
      storage: credential.storage,
      backend: credential.backend,
      encrypted: Boolean(credential.encrypted),
      created_at: key.created_at,
      updated_at: key.updated_at,
      age_days: ageDays,
      rotation_days: rotationDays,
      rotation_due: rotationDue,
    });
  });
  return {
    kind: "pool_api_key_audit",
    defaultRotationDays: 90,
    total: items.length,
    configured: items.filter((item) => item.configured).length,
    rotationDue: items.filter((item) => item.rotationDue).length,
    unencrypted: items.filter((item) => !item.encrypted).length,
    items,
  };
}

function apiKeyAuditItem(item) {
  return {
    provider: canonicalProviderId(item.provider ?? item.provider_id ?? item.providerId ?? ""),
    serviceType: item.service_type ?? item.serviceType ?? "provider",
    configured: Boolean(item.configured),
    keyHint: item.key_hint ?? item.keyHint ?? "",
    source: item.source ?? "",
    env: item.env ?? "",
    owner: item.owner ?? "",
    storage: item.storage ?? "",
    backend: item.backend ?? "",
    encrypted: Boolean(item.encrypted),
    createdAt: item.created_at ?? item.createdAt ?? "",
    updatedAt: item.updated_at ?? item.updatedAt ?? "",
    ageDays: item.age_days === null || item.ageDays === null ? null : numberValue(item.age_days ?? item.ageDays),
    rotationDays: item.rotation_days === 0 || item.rotationDays === 0 ? 0 : numberValue(item.rotation_days ?? item.rotationDays ?? 90),
    rotationDue: Boolean(item.rotation_due ?? item.rotationDue),
  };
}

function apiKeyAgeDays(value) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return null;
  return Math.max(0, Math.floor((Date.now() - date.getTime()) / 86_400_000));
}

function runtimeBudgetProvider(entry) {
  return {
    providerId: canonicalProviderId(entry.provider_id ?? entry.providerId ?? ""),
    configured: Boolean(entry.configured),
    keyHint: entry.key_hint ?? entry.keyHint ?? "",
    credentialStatus: entry.credential_status ?? entry.credentialStatus ?? (entry.configured ? "configured" : "not_recorded"),
    taskCount: numberValue(entry.task_count ?? entry.taskCount),
    providerRequestCount: numberValue(entry.provider_request_count ?? entry.providerRequestCount),
    tokenEstimate: numberValue(entry.token_estimate ?? entry.tokenEstimate),
    waitingApprovalTokens: numberValue(entry.waiting_approval_tokens ?? entry.waitingApprovalTokens),
    lastRequestAt: entry.last_request_at ?? entry.lastRequestAt ?? "",
  };
}

function normalizeRuntimePreflight(preflight) {
  const summary = preflight?.summary ?? {};
  return {
    ready: Boolean(preflight?.ready),
    projectFilter: preflight?.project_filter ?? preflight?.projectFilter ?? state.snapshot?.projectFilter,
    generatedAt: preflight?.generated_at ?? preflight?.generatedAt,
    summary: {
      blocked: numberValue(summary.blocked),
      warnings: numberValue(summary.warnings),
      passed: numberValue(summary.passed),
      checks: numberValue(summary.checks),
      runnableNodes: numberValue(summary.runnable_nodes ?? summary.runnableNodes),
      blockedNodes: numberValue(summary.blocked_nodes ?? summary.blockedNodes),
      approvalGates: numberValue(summary.approval_gates ?? summary.approvalGates),
      missingCredentials: numberValue(summary.missing_credentials ?? summary.missingCredentials),
      desktopHandoffs: numberValue(summary.desktop_handoffs ?? summary.desktopHandoffs),
      failedTasks: numberValue(summary.failed_tasks ?? summary.failedTasks),
    },
    checks: (preflight?.checks ?? []).map(runtimePreflightCheck),
    nextActions: (preflight?.next_actions ?? preflight?.nextActions ?? []).map(runtimePreflightAction),
  };
}

function deriveRuntimePreflightFromSnapshot(snapshot) {
  const approvalGates = (snapshot.tasks ?? []).filter((task) => task.requires_approval || task.status === "WaitingApproval");
  const failedTasks = (snapshot.tasks ?? []).filter((task) => ["Failed", "Retryable", "Cancelled"].includes(task.status));
  const configuredKeys = new Set(
    (snapshot.api_keys ?? [])
      .filter((key) => key.service_type === "provider" && key.configured)
      .map((key) => canonicalProviderId(key.provider)),
  );
  const providerIds = new Set();
  (snapshot.tasks ?? []).forEach((task) => {
    if (task.provider_id) providerIds.add(canonicalProviderId(task.provider_id));
  });
  (snapshot.provider_requests ?? []).forEach((request) => providerIds.add(canonicalProviderId(request.provider_id)));
  const missingCredentials = [...providerIds].filter((id) => id && !["mock-3dgs", "comfyui", "sam-3d"].includes(id) && !configuredKeys.has(id));
  const checks = [
    {
      id: "workflow_graph",
      status: (snapshot.workflows ?? []).length ? "passed" : "blocked",
      title: "Workflow graph",
      detail: (snapshot.workflows ?? []).length ? "Executable workflow graph is available." : "No executable workflow graph is available.",
    },
    {
      id: "approval_gates",
      status: approvalGates.length ? "blocked" : "passed",
      title: "Approval gates",
      detail: approvalGates.length ? "One or more tasks need approval." : "No task is blocked by approval.",
    },
    {
      id: "provider_credentials",
      status: missingCredentials.length ? "warning" : "passed",
      title: "Provider credentials",
      detail: missingCredentials.length ? "Some providers do not have saved runtime credentials." : "Provider credential state is ready.",
    },
    {
      id: "failed_tasks",
      status: failedTasks.length ? "blocked" : "passed",
      title: "Failed or retryable tasks",
      detail: failedTasks.length ? "Some tasks need retry or review." : "No failed task needs action.",
    },
  ];
  const blocked = checks.filter((check) => check.status === "blocked").length;
  const warnings = checks.filter((check) => check.status === "warning").length;
  return {
    ready: blocked === 0,
    project_filter: snapshot.project_filter,
    generated_at: snapshot.generated_at,
    summary: {
      blocked,
      warnings,
      passed: checks.filter((check) => check.status === "passed").length,
      checks: checks.length,
      runnable_nodes: Math.max(0, (snapshot.node_states ?? []).length - approvalGates.length - failedTasks.length),
      blocked_nodes: approvalGates.length + failedTasks.length,
      approval_gates: approvalGates.length,
      missing_credentials: missingCredentials.length,
      desktop_handoffs: 0,
      failed_tasks: failedTasks.length,
    },
    checks,
    next_actions: [
      ...approvalGates.slice(0, 3).map((task) => ({
        kind: "approval",
        title: `Approve task: ${task.title}`,
        command: `pool-cli --project ${task.project_slug} approve-task ${task.id}`,
      })),
      ...missingCredentials.slice(0, 3).map((providerId) => ({
        kind: "credential",
        title: `Save Provider credential: ${providerId}`,
        command: `pool-cli --project ${snapshot.project_filter ?? "demo"} set-api-key ${providerId} --api-key-env PROVIDER_API_KEY`,
      })),
    ],
  };
}

function runtimePreflightCheck(entry) {
  return {
    id: entry.id ?? "check",
    status: normalizePreflightStatus(entry.status),
    title: entry.title ?? entry.id ?? "Preflight check",
    detail: entry.detail ?? "",
    action: entry.action ?? "",
  };
}

function runtimePreflightAction(entry) {
  return {
    kind: entry.kind ?? "action",
    title: entry.title ?? "Next action",
    command: entry.command ?? "",
    nodeId: entry.node_id ?? entry.nodeId ?? "",
    taskType: entry.task_type ?? entry.taskType ?? "",
    mcpTool: entry.mcp_tool ?? entry.mcpTool ?? "",
  };
}

function normalizeRuntimePrdReadiness(readiness) {
  if (!readiness) return null;
  const summary = readiness.summary ?? {};
  const gate = readiness.completion_gate ?? readiness.completionGate ?? {};
  return {
    kind: readiness.kind ?? "pool_prd_readiness",
    overallStatus: readiness.overall_status ?? readiness.overallStatus ?? "partial",
    projectFilter: readiness.project_filter ?? readiness.projectFilter ?? state.snapshot?.projectFilter,
    generatedAt: readiness.generated_at ?? readiness.generatedAt,
    summary: {
      total: numberValue(summary.total),
      ready: numberValue(summary.ready),
      partial: numberValue(summary.partial),
      blocked: numberValue(summary.blocked),
    },
    completionGate: {
      status: gate.status ?? "unknown",
      readyForCompletion: Boolean(gate.ready_for_completion ?? gate.readyForCompletion),
      incompleteRequirements: gate.incomplete_requirements ?? gate.incompleteRequirements ?? [],
      proofCommands: gate.proof_commands ?? gate.proofCommands ?? {},
    },
    requirements: (readiness.requirements ?? []).map(runtimePrdRequirement),
    sourceResources: readiness.source_resources ?? readiness.sourceResources ?? [],
  };
}

function normalizeRuntimeCoreArchitectureReadiness(readiness) {
  if (!readiness) return null;
  const summary = readiness.summary ?? {};
  const gate = readiness.architecture_gate ?? readiness.architectureGate ?? {};
  return {
    kind: readiness.kind ?? "pool_core_architecture_readiness",
    overallStatus: readiness.overall_status ?? readiness.overallStatus ?? "partial",
    projectFilter: readiness.project_filter ?? readiness.projectFilter ?? state.snapshot?.projectFilter,
    generatedAt: readiness.generated_at ?? readiness.generatedAt,
    summary: {
      total: numberValue(summary.total),
      ready: numberValue(summary.ready),
      partial: numberValue(summary.partial),
      blocked: numberValue(summary.blocked),
    },
    architectureGate: {
      status: gate.status ?? "unknown",
      readyForCoreArchitecture: Boolean(gate.ready_for_core_architecture ?? gate.readyForCoreArchitecture),
      incompleteRequirements: gate.incomplete_requirements ?? gate.incompleteRequirements ?? [],
      proofCommands: gate.proof_commands ?? gate.proofCommands ?? {},
    },
    requirements: (readiness.requirements ?? []).map(runtimePrdRequirement),
    sourceResources: readiness.source_resources ?? readiness.sourceResources ?? [],
  };
}

function normalizeRuntimePrdCompletionGate(payload) {
  if (!payload) return null;
  const summary = payload.summary ?? {};
  const gate = payload.completion_gate ?? payload.completionGate ?? {};
  return {
    kind: payload.kind ?? "pool_prd_completion_gate",
    overallStatus: payload.overall_status ?? payload.overallStatus ?? "partial",
    projectFilter: payload.project_filter ?? payload.projectFilter ?? state.snapshot?.projectFilter,
    summary: {
      total: numberValue(summary.total),
      ready: numberValue(summary.ready),
      partial: numberValue(summary.partial),
      blocked: numberValue(summary.blocked),
    },
    status: gate.status ?? "unknown",
    readyForCompletion: Boolean(gate.ready_for_completion ?? gate.readyForCompletion),
    incompleteRequirements: gate.incomplete_requirements ?? gate.incompleteRequirements ?? [],
    proofCommands: gate.proof_commands ?? gate.proofCommands ?? {},
    criteria: gate.criteria ?? [],
  };
}

function normalizeRuntimeCoreArchitectureGate(payload) {
  if (!payload) return null;
  const summary = payload.summary ?? {};
  const gate = payload.architecture_gate ?? payload.architectureGate ?? {};
  return {
    kind: payload.kind ?? "pool_core_architecture_gate",
    overallStatus: payload.overall_status ?? payload.overallStatus ?? "partial",
    projectFilter: payload.project_filter ?? payload.projectFilter ?? state.snapshot?.projectFilter,
    summary: {
      total: numberValue(summary.total),
      ready: numberValue(summary.ready),
      partial: numberValue(summary.partial),
      blocked: numberValue(summary.blocked),
    },
    status: gate.status ?? "unknown",
    readyForCoreArchitecture: Boolean(gate.ready_for_core_architecture ?? gate.readyForCoreArchitecture),
    incompleteRequirements: gate.incomplete_requirements ?? gate.incompleteRequirements ?? [],
    proofCommands: gate.proof_commands ?? gate.proofCommands ?? {},
    criteria: gate.criteria ?? [],
  };
}

function normalizeRuntimePrdCompletionPackage(result) {
  if (!result) return null;
  const report = result.report ?? {};
  return {
    kind: result.kind ?? "pool_prd_completion_package",
    status: report.status ?? result.status ?? "unknown",
    packageDir: report.package_dir ?? report.packageDir ?? "",
    readinessPath: report.readiness_path ?? report.readinessPath ?? "",
    completionGatePath: report.completion_gate_path ?? report.completionGatePath ?? "",
    productionEvidenceRequirementsPath:
      report.production_evidence_requirements_path ??
      report.productionEvidenceRequirementsPath ??
      "",
    manifestPath: report.manifest_path ?? report.manifestPath ?? "",
    snapshotPath: report.snapshot_path ?? report.snapshotPath ?? "",
    readyForCompletion: Boolean(report.ready_for_completion ?? report.readyForCompletion),
    completionStatus: report.completion_status ?? report.completionStatus ?? "unknown",
    localFiles: Array.isArray(report.local_paths)
      ? report.local_paths.length
      : Array.isArray(report.local_files)
        ? report.local_files.length
      : numberValue(report.local_files ?? report.localFiles),
    taskId: result.task?.id ?? "",
    assetCount: Array.isArray(result.assets) ? result.assets.length : 0,
  };
}

function mergeRuntimePrdCompletionPackages(catalog) {
  if (!Array.isArray(catalog?.packages)) return false;
  if (!catalog.packages.length) {
    state.runtimePrdCompletionPackage = null;
    return false;
  }
  state.runtimePrdCompletionPackage = normalizeRuntimePrdCompletionPackage({
    kind: "pool_prd_completion_package",
    report: catalog.packages[0],
  });
  return true;
}

function normalizeConformancePackageSummary(packageSummary, fallbackKind = "") {
  if (!packageSummary) return null;
  const localFiles = packageSummary.local_files ?? packageSummary.localFiles ?? [];
  const packageKind = packageSummary.package_kind ?? packageSummary.packageKind ?? fallbackKind;
  return {
    packageId: packageSummary.package_id ?? packageSummary.packageId ?? "",
    packageKind,
    targetId:
      packageSummary.target_id ??
      packageSummary.targetId ??
      packageSummary.provider_id ??
      packageSummary.adapter_id ??
      packageSummary.session_kind ??
      "",
    projectSlug: packageSummary.project_slug ?? packageSummary.projectSlug ?? "",
    status: packageSummary.status ?? "missing",
    title: packageSummary.title ?? "",
    packageDir: packageSummary.package_dir ?? packageSummary.packageDir ?? "",
    manifestPath: packageSummary.manifest_path ?? packageSummary.manifestPath ?? "",
    runnerScriptPath:
      packageSummary.runner_script_path ??
      packageSummary.runnerScriptPath ??
      packageSummary.paths?.runner_script ??
      packageSummary.paths?.runnerScript ??
      "",
    preflightPath: packageSummary.preflight_path ?? packageSummary.preflightPath ?? "",
    contractPath: packageSummary.contract_path ?? packageSummary.contractPath ?? "",
    gatewayWorkerContractPath:
      packageSummary.gateway_worker_contract_path ??
      packageSummary.gatewayWorkerContractPath ??
      "",
    runbookPath: packageSummary.runbook_path ?? packageSummary.runbookPath ?? "",
    requestPath: packageSummary.request_path ?? packageSummary.requestPath ?? "",
    localFiles: Array.isArray(localFiles) ? localFiles.length : numberValue(localFiles),
    localFileFailures:
      packageSummary.local_file_failures ??
      packageSummary.localFileFailures ??
      [],
    commands: packageSummary.commands ?? {},
    nextActions: packageSummary.next_actions ?? packageSummary.nextActions ?? {},
    summary: packageSummary.summary ?? {},
  };
}

function mergeConformancePackageCatalog(kind, catalog) {
  if (!Array.isArray(catalog?.packages)) return false;
  const latest = catalog.packages.length
    ? normalizeConformancePackageSummary(catalog.packages[0], kind)
    : null;
  if (kind === "provider") state.providerConformancePackage = latest;
  if (kind === "software") state.softwareConformancePackage = latest;
  if (kind === "agent") state.agentConformancePackage = latest;
  if (kind === "integration") state.integrationConformancePackage = latest;
  return Boolean(latest);
}

function normalizeRuntimeCoreArchitecturePackage(result) {
  if (!result) return null;
  const report = result.report ?? {};
  return {
    kind: result.kind ?? "pool_core_architecture_package",
    status: report.status ?? result.status ?? "unknown",
    packageDir: report.package_dir ?? report.packageDir ?? "",
    readinessPath: report.readiness_path ?? report.readinessPath ?? "",
    coreArchitectureGatePath: report.core_architecture_gate_path ?? report.coreArchitectureGatePath ?? "",
    runtimeGraphPath: report.runtime_graph_path ?? report.runtimeGraphPath ?? "",
    runtimeExecutionPlanPath: report.runtime_execution_plan_path ?? report.runtimeExecutionPlanPath ?? "",
    runtimeHandoffPath: report.runtime_handoff_path ?? report.runtimeHandoffPath ?? "",
    outputPackagesPath: report.output_packages_path ?? report.outputPackagesPath ?? "",
    strictPrdCompletionGatePath:
      report.strict_prd_completion_gate_path ??
      report.strictPrdCompletionGatePath ??
      "",
    manifestPath: report.manifest_path ?? report.manifestPath ?? "",
    snapshotPath: report.snapshot_path ?? report.snapshotPath ?? "",
    readyForCoreArchitecture: Boolean(report.ready_for_core_architecture ?? report.readyForCoreArchitecture),
    architectureStatus: report.architecture_status ?? report.architectureStatus ?? "unknown",
    localFiles: Array.isArray(report.local_paths)
      ? report.local_paths.length
      : Array.isArray(report.local_files)
        ? report.local_files.length
      : numberValue(report.local_files ?? report.localFiles),
    taskId: result.task?.id ?? "",
    assetCount: Array.isArray(result.assets) ? result.assets.length : 0,
  };
}

function normalizeProductionEvidenceRequirements(requirements) {
  if (!requirements) return null;
  const summary = requirements.summary ?? {};
  const taskSummary = requirements.evidence_tasks?.summary ?? {};
  const evidenceTasks = Array.isArray(requirements.evidence_tasks?.tasks)
    ? requirements.evidence_tasks.tasks.map(normalizeProductionEvidenceTask)
    : [];
  const providerMissing = (summary.missing_provider_production_upstream_success ?? []).map(String);
  const softwareMissing = (summary.missing_software_production_success ?? []).map(String);
  const desktopMissing = (summary.missing_desktop_vision ?? []).map(String);
  return {
    kind: requirements.kind ?? "pool_production_evidence_requirements",
    overallStatus: requirements.overall_status ?? requirements.overallStatus ?? "partial",
    generatedAt: requirements.generated_at ?? requirements.generatedAt,
    complete: Boolean(summary.complete),
    missingTotal: numberValue(summary.missing_total ?? providerMissing.length + softwareMissing.length + desktopMissing.length),
    providerGatewayReady: Boolean(summary.provider_gateway_ready),
    providerProductionReady: Boolean(summary.provider_production_ready),
    softwareControlReady: Boolean(summary.software_control_ready),
    softwareProductionReady: Boolean(summary.software_production_ready),
    desktopVisionReady: Boolean(summary.desktop_vision_ready),
    providerMissing,
    softwareMissing,
    desktopMissing,
    commands: requirements.commands ?? {},
    evidenceTaskSummary: {
      total: numberValue(taskSummary.total ?? evidenceTasks.length),
      providerTasks: numberValue(taskSummary.provider_tasks ?? 0),
      softwareTasks: numberValue(taskSummary.software_tasks ?? 0),
      desktopVisionTasks: numberValue(taskSummary.desktop_vision_tasks ?? 0),
    },
    evidenceTasks,
  };
}

function normalizeProductionEvidenceTask(task) {
  const bridgeWorker = task.bridge_worker ?? task.bridgeWorker ?? null;
  return {
    id: task.id ?? "",
    kind: task.kind ?? "production_evidence",
    targetId: task.target_id ?? "",
    status: task.status ?? "missing",
    title: task.title ?? "生产证据任务",
    bundlePath: task.bundle_path ?? "",
    artifactPolicy: task.artifact_policy ?? "",
    preferredControlProfile: task.preferred_control_profile ?? task.preferredControlProfile ?? "",
    bridgeWorker: bridgeWorker ? {
      available: Boolean(bridgeWorker.available),
      adapterId: bridgeWorker.adapter_id ?? bridgeWorker.adapterId ?? "",
      endpointEnv: bridgeWorker.endpoint_env ?? bridgeWorker.endpointEnv ?? "",
      endpointEnvTemplate: bridgeWorker.endpoint_env_template ?? bridgeWorker.endpointEnvTemplate ?? "",
      cliTemplate: bridgeWorker.cli_template ?? bridgeWorker.cliTemplate ?? "",
      productionRule: bridgeWorker.production_rule ?? bridgeWorker.productionRule ?? "",
      reason: bridgeWorker.reason ?? "",
    } : null,
    commands: task.commands ?? {},
  };
}

function normalizeProductionEvidenceTasks(result) {
  if (!result) return null;
  const summary = result.summary ?? {};
  const tasks = Array.isArray(result.tasks)
    ? result.tasks.map(normalizeProductionEvidenceTask)
    : [];
  return {
    kind: result.kind ?? "pool_production_evidence_tasks",
    overallStatus: result.overall_status ?? result.overallStatus ?? "partial",
    summary: {
      total: numberValue(summary.total ?? tasks.length),
      providerTasks: numberValue(summary.provider_tasks ?? 0),
      softwareTasks: numberValue(summary.software_tasks ?? 0),
      desktopVisionTasks: numberValue(summary.desktop_vision_tasks ?? 0),
    },
    tasks,
    commands: result.commands ?? {},
  };
}

function normalizeProductionEvidenceHandoff(handoff) {
  if (!handoff) return null;
  const summary = handoff.summary ?? {};
  const bundle = handoff.bundle ?? {};
  const commands = handoff.commands ?? {};
  const providerGatewayWorkerStartCommands = (handoff.provider_gateway_worker_start_commands ?? handoff.providerGatewayWorkerStartCommands ?? []).map((command) => ({
    family: command.family ?? "",
    endpointEnv: command.endpoint_env ?? command.endpointEnv ?? "",
    upstreamEnv: command.upstream_env ?? command.upstreamEnv ?? "",
    cli: command.cli ?? "",
  }));
  const bridgeWorkerStartCommands = (handoff.software_bridge_worker_start_commands ?? handoff.softwareBridgeWorkerStartCommands ?? []).map((command) => ({
    adapterId: command.adapter_id ?? command.adapterId ?? "",
    endpointEnv: command.endpoint_env ?? command.endpointEnv ?? "",
    upstreamEnv: command.upstream_env ?? command.upstreamEnv ?? "",
    cli: command.cli ?? "",
  }));
  return {
    kind: handoff.kind ?? "pool_production_evidence_handoff",
    overallStatus: handoff.overall_status ?? handoff.overallStatus ?? "partial",
    outputRoot: handoff.output_root ?? handoff.outputRoot ?? "",
    missingTotal: numberValue(summary.missing_total ?? summary.missingTotal),
    evidenceTasks: numberValue(summary.evidence_tasks ?? summary.evidenceTasks),
    commands: {
      merge: commands.merge ?? "pool-cli merge-production-evidence <combined-bundle.json> <bundle-a.json> <bundle-b.json>...",
      validate: commands.validate ?? "pool-cli validate-production-evidence <bundle.json>",
      import: commands.import ?? "pool-cli import-production-evidence <bundle.json>",
      submit_item: commands.submit_item ?? "pool-cli submit-production-evidence-item <item.json>",
      ...commands,
    },
    bundleSummary: {
      providers: Array.isArray(bundle.providers) ? bundle.providers.length : 0,
      softwareActions: Array.isArray(bundle.software_actions) ? bundle.software_actions.length : 0,
      desktopVision: Array.isArray(bundle.desktop_vision) ? bundle.desktop_vision.length : 0,
    },
    providerGatewayWorkerStartCommands,
    bridgeWorkerStartCommands,
  };
}

function normalizeProductionEvidenceRunPlan(plan) {
  if (!plan) return null;
  const summary = plan.summary ?? {};
  const phases = Array.isArray(plan.phases) ? plan.phases : [];
  return {
    kind: plan.kind ?? "pool_production_evidence_run_plan",
    source: plan.source ?? "production-evidence-run-plan",
    status: plan.status ?? "needs_real_production_evidence",
    readyForCompletion: Boolean(plan.ready_for_completion),
    outputRoot: plan.output_root ?? plan.outputRoot ?? "",
    phaseCount: phases.length,
    phases: phases.map((phase) => ({
      id: phase.id ?? phase.name ?? "phase",
      status: phase.status ?? "pending",
      command: phase.command ?? "",
      readyCondition: phase.ready_condition ?? phase.readyCondition ?? "",
      genericApiBridgeWorker: phase.generic_api_bridge_worker ? {
        appliesTo: (phase.generic_api_bridge_worker.applies_to ?? []).map(String),
        cliTemplate: phase.generic_api_bridge_worker.cli_template ?? "",
        endpointEnvTemplate: phase.generic_api_bridge_worker.endpoint_env_template ?? "",
        operatorNote: phase.generic_api_bridge_worker.operator_note ?? "",
      } : null,
      bridgeWorkerStartCommands: (phase.bridge_worker_start_commands ?? phase.bridgeWorkerStartCommands ?? []).map((command) => ({
        adapterId: command.adapter_id ?? command.adapterId ?? "",
        endpointEnv: command.endpoint_env ?? command.endpointEnv ?? "",
        upstreamEnv: command.upstream_env ?? command.upstreamEnv ?? "",
        cli: command.cli ?? "",
      })),
      providerGatewayWorkerStartCommands: (phase.provider_gateway_worker_start_commands ?? phase.providerGatewayWorkerStartCommands ?? []).map((command) => ({
        family: command.family ?? "",
        endpointEnv: command.endpoint_env ?? command.endpointEnv ?? "",
        upstreamEnv: command.upstream_env ?? command.upstreamEnv ?? "",
        cli: command.cli ?? "",
      })),
    })),
    summary: {
      missingTotal: numberValue(summary.missing_total ?? summary.missingTotal),
      providerTasks: numberValue(summary.provider_tasks ?? summary.providerTasks),
      softwareTasks: numberValue(summary.software_tasks ?? summary.softwareTasks),
      desktopVisionTasks: numberValue(summary.desktop_vision_tasks ?? summary.desktopVisionTasks),
      ready: numberValue(summary.ready),
      partial: numberValue(summary.partial),
      blocked: numberValue(summary.blocked),
    },
    commands: {
      run_plan: plan.commands?.run_plan ?? "pool-cli production-evidence-run-plan <run-plan.json>",
      closeout_preflight: plan.commands?.closeout_preflight ?? "pool-cli closeout-production-evidence --output <merged-bundle.json> <bundle.json>",
      closeout_import: plan.commands?.closeout_import ?? "pool-cli closeout-production-evidence --import <bundle.json>",
      completion_gate: plan.commands?.completion_gate ?? "pool-cli prd-completion-gate --require-complete",
      ...plan.commands,
    },
  };
}

function runtimePrdRequirement(entry) {
  return {
    id: entry.id ?? "requirement",
    title: entry.title ?? entry.id ?? "Requirement",
    status: entry.status ?? "partial",
    summary: entry.summary ?? "",
    gaps: (entry.gaps ?? []).map(String),
    nextActions: (entry.next_actions ?? entry.nextActions ?? []).map(String),
  };
}

function normalizeRuntimeHandoff(handoff) {
  const summary = handoff?.summary ?? {};
  const lanes = (handoff?.lanes ?? []).map(runtimeHandoffLane);
  const teamRoles = (handoff?.team?.roles ?? handoff?.teamRoles ?? []).map(runtimeHandoffTeamRole);
  return {
    ready: Boolean(handoff?.ready),
    projectFilter: handoff?.project_filter ?? handoff?.projectFilter ?? state.snapshot?.projectFilter,
    generatedAt: handoff?.generated_at ?? handoff?.generatedAt,
    summary: {
      lanes: numberValue(summary.lanes ?? lanes.length),
      commands: numberValue(summary.commands),
      approvalActions: numberValue(summary.approval_actions ?? summary.approvalActions),
      retryActions: numberValue(summary.retry_actions ?? summary.retryActions),
      credentialActions: numberValue(summary.credential_actions ?? summary.credentialActions),
      desktopRequests: numberValue(summary.desktop_requests ?? summary.desktopRequests),
      runnableNodeActions: numberValue(summary.runnable_node_actions ?? summary.runnableNodeActions),
      teamRoles: numberValue(summary.team_roles ?? summary.teamRoles ?? teamRoles.length),
    },
    team: {
      size: numberValue(handoff?.team?.size ?? teamRoles.length),
      mode: handoff?.team?.mode ?? "five_person_content_burst_team",
      roles: teamRoles,
    },
    controlPriority: handoff?.control_priority ?? handoff?.controlPriority ?? [
      "API/MCP",
      "Skills/CLI",
      "Desktop Recognition",
      "Human Takeover",
    ],
    lanes,
    commands: (handoff?.commands ?? []).map(runtimeHandoffCommand),
    mcpResources: handoff?.mcp_resources ?? handoff?.mcpResources ?? [],
  };
}

function normalizeRuntimeHandoffPackage(result) {
  const report = result?.report ?? {};
  const operatorChecklist = Array.isArray(report.operator_checklist)
    ? report.operator_checklist
    : Array.isArray(report.operatorChecklist)
      ? report.operatorChecklist
      : [];
  return {
    status: report.status ?? result?.status ?? "unknown",
    localFiles: Array.isArray(report.local_paths)
      ? report.local_paths.length
      : numberValue(report.local_files ?? report.localFiles),
    manifestPath: report.manifest_path ?? report.manifestPath ?? "",
    integrationReadinessPath:
      report.integration_readiness_path ??
      report.integrationReadinessPath ??
      "",
    handoffPath: report.handoff_path ?? report.handoffPath ?? "",
    preflightPath: report.preflight_path ?? report.preflightPath ?? "",
    graphPath: report.graph_path ?? report.graphPath ?? "",
    workerSelfChecksPath:
      report.worker_self_checks_path ??
      report.workerSelfChecksPath ??
      "",
    workerSelfChecksPreflightPath:
      report.worker_self_checks_preflight_path ??
      report.workerSelfChecksPreflightPath ??
      "",
    snapshotPath: report.snapshot_path ?? report.snapshotPath ?? "",
    operatorChecklist: operatorChecklist.map((step) => ({
      step: numberValue(step?.step),
      owner: step?.owner ?? "",
      action: step?.action ?? "",
      command: step?.command ?? "",
      path: step?.path ?? "",
      verify: step?.verify ?? "",
    })),
    agentEntrypoint: report.agent_entrypoint ?? report.agentEntrypoint ?? {},
    mcpResources: Array.isArray(report.mcp_resources)
      ? report.mcp_resources
      : Array.isArray(report.mcpResources)
        ? report.mcpResources
        : [],
    taskId: report.task_id ?? result?.task?.id ?? "",
    assetCount: Array.isArray(report.assets) ? report.assets.length : 0,
  };
}

function normalizeRuntimeHandoffPackageSummary(packageSummary) {
  const localFiles = Array.isArray(packageSummary?.local_files)
    ? packageSummary.local_files
    : Array.isArray(packageSummary?.localFiles)
      ? packageSummary.localFiles
      : [];
  const operatorChecklist = Array.isArray(packageSummary?.operator_checklist)
    ? packageSummary.operator_checklist
    : Array.isArray(packageSummary?.operatorChecklist)
      ? packageSummary.operatorChecklist
      : [];
  return {
    status: packageSummary?.status ?? "unknown",
    localFiles: localFiles.length,
    manifestPath: packageSummary?.manifest_path ?? packageSummary?.manifestPath ?? "",
    integrationReadinessPath:
      packageSummary?.integration_readiness_path ??
      packageSummary?.integrationReadinessPath ??
      "",
    handoffPath: packageSummary?.handoff_path ?? packageSummary?.handoffPath ?? "",
    preflightPath: packageSummary?.preflight_path ?? packageSummary?.preflightPath ?? "",
    graphPath: packageSummary?.graph_path ?? packageSummary?.graphPath ?? "",
    workerSelfChecksPath:
      packageSummary?.worker_self_checks_path ??
      packageSummary?.workerSelfChecksPath ??
      "",
    workerSelfChecksPreflightPath:
      packageSummary?.worker_self_checks_preflight_path ??
      packageSummary?.workerSelfChecksPreflightPath ??
      "",
    snapshotPath: packageSummary?.snapshot_path ?? packageSummary?.snapshotPath ?? "",
    operatorChecklist: operatorChecklist.map((step) => ({
      step: numberValue(step?.step),
      owner: step?.owner ?? "",
      action: step?.action ?? "",
      command: step?.command ?? "",
      path: step?.path ?? "",
      verify: step?.verify ?? "",
    })),
    agentEntrypoint: packageSummary?.agent_entrypoint ?? packageSummary?.agentEntrypoint ?? {},
    mcpResources: Array.isArray(packageSummary?.mcp_resources)
      ? packageSummary.mcp_resources
      : Array.isArray(packageSummary?.mcpResources)
        ? packageSummary.mcpResources
        : [],
    taskId: "",
    assetCount: localFiles.length,
  };
}

function mergeRuntimeHandoffPackages(catalog) {
  if (!Array.isArray(catalog?.packages)) return false;
  if (!catalog.packages.length) {
    state.runtimeHandoffPackage = null;
    return false;
  }
  state.runtimeHandoffPackage = normalizeRuntimeHandoffPackageSummary(catalog.packages[0]);
  return true;
}

function mergeRuntimeCoreArchitecturePackages(catalog) {
  if (!Array.isArray(catalog?.packages)) return false;
  if (!catalog.packages.length) {
    state.runtimeCoreArchitecturePackage = null;
    return false;
  }
  state.runtimeCoreArchitecturePackage = normalizeRuntimeCoreArchitecturePackage({
    kind: "pool_core_architecture_package",
    report: catalog.packages[0],
  });
  return true;
}

function normalizeRuntimeDiscovery(payload) {
  if (!payload || typeof payload !== "object") return null;
  const endpoints = payload.endpoints && typeof payload.endpoints === "object" ? payload.endpoints : {};
  const resources = Array.isArray(payload.mcp_resources) ? payload.mcp_resources : [];
  const tools = Array.isArray(payload.mcp_tools) ? payload.mcp_tools : [];
  const prompts = Array.isArray(payload.mcp_prompts) ? payload.mcp_prompts : [];
  return {
    service: payload.service ?? "pool-runtime",
    version: payload.version ?? 1,
    baseUrl: payload.base_url ?? state.snapshot?.runtime ?? "",
    projectFilter: payload.project_filter ?? runtimeProjectFilter() ?? "",
    capabilities: payload.capabilities ?? {},
    endpoints,
    resources,
    tools,
    prompts,
    endpointCount: Object.keys(endpoints).length,
    resourceCount: resources.length,
    toolCount: tools.length,
    promptCount: prompts.length,
  };
}

function normalizeIntegrationReadiness(payload) {
  if (!payload || typeof payload !== "object") return null;
  const summary = payload.summary ?? {};
  return {
    kind: payload.kind ?? "pool_integration_readiness",
    projectFilter: payload.project_filter ?? payload.projectFilter ?? state.snapshot?.projectFilter,
    generatedAt: payload.generated_at ?? payload.generatedAt,
    summary: {
      providers: numberValue(summary.providers),
      softwareAdapters: numberValue(summary.software_adapters ?? summary.softwareAdapters),
      agentSessions: numberValue(summary.agent_sessions ?? summary.agentSessions),
      ready: numberValue(summary.ready),
      needsConfiguration: numberValue(summary.needs_configuration ?? summary.needsConfiguration),
      needsExecution: numberValue(summary.needs_execution ?? summary.needsExecution),
      needsAttention: numberValue(summary.needs_attention ?? summary.needsAttention),
      total: numberValue(summary.total),
    },
    lanes: payload.lanes ?? [],
    runPlan: payload.run_plan ?? payload.runPlan ?? [],
    providers: (payload.providers ?? []).map(integrationProviderReadiness),
    softwareAdapters: (payload.software_adapters ?? payload.softwareAdapters ?? []).map(integrationSoftwareReadiness),
    agent: integrationAgentReadiness(payload.agent),
    commands: payload.commands ?? {},
    policy: payload.policy ?? {},
  };
}

function integrationProviderReadiness(entry) {
  return {
    id: canonicalProviderId(entry.provider_id ?? entry.providerId ?? ""),
    displayName: entry.display_name ?? entry.displayName ?? entry.provider_id ?? "provider",
    kind: entry.kind ?? "provider",
    lane: entry.lane ?? "ai_media",
    status: normalizeRuntimeStatus(entry.status),
    nextAction: entry.next_action ?? entry.nextAction ?? null,
    configured: Boolean(entry.configured),
    keyHint: entry.key_hint ?? entry.keyHint ?? "",
    taskCount: numberValue(entry.task_count ?? entry.taskCount),
    requestCount: numberValue(entry.request_count ?? entry.requestCount),
    successCount: numberValue(entry.success_count ?? entry.successCount),
    failedCount: numberValue(entry.failed_count ?? entry.failedCount),
    waitingApprovalCount: numberValue(entry.waiting_approval_count ?? entry.waitingApprovalCount),
    latestTask: entry.latest_task ?? entry.latestTask ?? null,
    latestRequest: entry.latest_request ?? entry.latestRequest ?? null,
    commands: entry.commands ?? {},
  };
}

function integrationSoftwareReadiness(entry) {
  return {
    id: softwareAdapterId(entry.adapter_id ?? entry.adapterId ?? ""),
    displayName: entry.display_name ?? entry.displayName ?? entry.adapter_id ?? "adapter",
    lane: entry.lane ?? "orchestration",
    status: normalizeRuntimeStatus(entry.status),
    nextAction: entry.next_action ?? entry.nextAction ?? null,
    controlModes: entry.control_modes ?? entry.controlModes ?? [],
    desktopFallback: Boolean(entry.desktop_fallback ?? entry.desktopFallback),
    actionCount: numberValue(entry.action_count ?? entry.actionCount),
    taskCount: numberValue(entry.task_count ?? entry.taskCount),
    successCount: numberValue(entry.success_count ?? entry.successCount),
    failedCount: numberValue(entry.failed_count ?? entry.failedCount),
    latestAction: entry.latest_action ?? entry.latestAction ?? null,
    commands: entry.commands ?? {},
  };
}

function integrationAgentReadiness(agent = {}) {
  return {
    lane: agent.lane ?? "orchestration",
    status: normalizeRuntimeStatus(agent.status),
    nextAction: agent.next_action ?? agent.nextAction ?? null,
    sessions: numberValue(agent.sessions),
    transcripts: numberValue(agent.transcripts),
    latestSession: agent.latest_session ?? agent.latestSession ?? null,
    commands: agent.commands ?? {},
    nextActions: agent.next_actions ?? agent.nextActions ?? [],
  };
}

function normalizeRuntimeExecutionPlan(plan) {
  const workflows = (plan?.workflows ?? []).map(runtimeExecutionPlanWorkflow);
  const steps = workflows.flatMap((workflow) => workflow.steps);
  const summary = plan?.summary ?? {};
  return {
    kind: plan?.kind ?? "pool_runtime_execution_plan",
    generatedAt: plan?.generated_at ?? plan?.generatedAt,
    workflows,
    steps,
    nextSteps: (plan?.next_steps ?? plan?.nextSteps ?? []).map(runtimeExecutionPlanStep),
    summary: {
      workflows: numberValue(summary.workflows ?? workflows.length),
      steps: numberValue(summary.steps ?? steps.length),
      runnableSteps: numberValue(summary.runnable_steps ?? summary.runnableSteps ?? steps.filter((step) => step.phase === "ready").length),
      gatedSteps: numberValue(summary.gated_steps ?? summary.gatedSteps ?? steps.filter((step) => step.gateKind !== "none").length),
      phaseCounts: summary.phase_counts ?? summary.phaseCounts ?? {},
      taskTypeCounts: summary.task_type_counts ?? summary.taskTypeCounts ?? {},
    },
  };
}

function runtimeExecutionPlanWorkflow(entry) {
  return {
    id: entry.workflow_id ?? entry.workflowId ?? "",
    name: entry.name ?? "Workflow",
    projectSlug: entry.project_slug ?? entry.projectSlug ?? "",
    topologyComplete: entry.topology_complete ?? entry.topologyComplete ?? true,
    summary: entry.summary ?? {},
    steps: (entry.steps ?? []).map(runtimeExecutionPlanStep),
  };
}

function runtimeExecutionPlanStep(entry) {
  const action = entry.control?.recommended_action ?? entry.control?.recommendedAction ?? {};
  const gate = entry.gate ?? {};
  return {
    id: entry.id ?? entry.node_id ?? entry.nodeId ?? "",
    sequence: numberValue(entry.sequence),
    workflowId: entry.workflow_id ?? entry.workflowId ?? "",
    nodeId: entry.node_id ?? entry.nodeId ?? "",
    title: entry.title ?? entry.node_id ?? "Step",
    taskType: entry.task_type ?? entry.taskType ?? "node",
    nodeType: entry.node_type ?? entry.nodeType ?? "",
    status: entry.status ?? "Ready",
    phase: entry.phase ?? "ready",
    gateKind: gate.kind ?? "none",
    command: action.command ?? "",
    actionKind: action.kind ?? "",
    mcpTool: action.mcp_tool ?? action.mcpTool ?? "",
    contracts: entry.contracts ?? [],
  };
}

function runtimeHandoffLane(entry) {
  return {
    id: entry.id ?? "lane",
    title: entry.title ?? entry.id ?? "Handoff lane",
    teamRole: entry.team_role ?? entry.teamRole ?? "",
    executor: entry.executor ?? "operator",
    status: entry.status ?? "ready",
    resources: entry.resources ?? [],
    commands: (entry.commands ?? []).map((command) => String(command)),
    actions: (entry.actions ?? []).map(runtimePreflightAction),
    requests: entry.requests ?? [],
  };
}

function runtimeHandoffTeamRole(entry) {
  return {
    id: entry.id ?? "role",
    title: entry.title ?? entry.id ?? "Team role",
    focus: entry.focus ?? "",
    primarySurface: entry.primary_surface ?? entry.primarySurface ?? "",
    status: entry.status ?? "ready",
    queueCount: numberValue(entry.queue_count ?? entry.queueCount),
    assignedLaneIds: entry.assigned_lane_ids ?? entry.assignedLaneIds ?? [],
    lanes: entry.lanes ?? [],
  };
}

function runtimeHandoffCommand(entry) {
  if (typeof entry === "string") {
    return {
      lane: "handoff",
      kind: "command",
      title: "Command",
      command: entry,
    };
  }
  return {
    lane: entry.lane ?? "handoff",
    kind: entry.kind ?? "command",
    title: entry.title ?? entry.kind ?? "Command",
    command: entry.command ?? "",
  };
}

function deriveRuntimeHandoffFromState() {
  const project = activeProjectSlug();
  const preflight = state.runtimePreflight ?? normalizeRuntimePreflight(deriveRuntimePreflightFromSnapshot({
    project_filter: project,
    generated_at: state.snapshot?.generatedAt,
    workflows: state.runtimeGraph?.workflows ?? [],
    node_states: state.tasks.map((task) => ({ node_id: task.nodeId })),
    tasks: state.tasks.map((task) => ({
      id: task.id,
      project_slug: project,
      title: task.title,
      provider_id: task.tool,
      status: task.status === "waiting_approval" ? "WaitingApproval" : task.status,
      requires_approval: task.status === "waiting_approval",
    })),
    provider_requests: state.providerRequests.map((request) => ({ provider_id: request.providerId })),
    api_keys: state.apiKeys,
  }));
  const approvalActions = preflight.nextActions.filter((action) => action.kind === "approval");
  const retryActions = preflight.nextActions.filter((action) => action.kind === "retry");
  const credentialActions = preflight.nextActions.filter((action) => action.kind === "credential");
  const desktopActions = preflight.nextActions.filter((action) => action.kind === "desktop_recognition");
  const runnableActions = state.nodes
    .filter((node) => !["running", "waiting_approval"].includes(node.status))
    .slice(0, 6)
    .map((node) => ({
      kind: "run_node",
      title: `Run node: ${node.title}`,
      node_id: node.id,
      task_type: node.group ?? "node",
      command: `pool-cli --project ${project} run-node ${node.id}`,
      mcp_tool: "pool_run_node",
    }));
  const lanes = [
    {
      id: "agent_context",
      title: "Agent/Hermes context load",
      team_role: "agent_operator",
      executor: "hermes_or_agent_cli",
      status: "ready",
      resources: ["pool://runtime-preflight", "pool://runtime-graph", "pool://tasks"],
      commands: [
        `pool-cli --project ${project} runtime-preflight`,
        `pool-cli --project ${project} runtime-graph`,
        `pool-cli --project ${project} workflow-context`,
      ],
    },
    { id: "manual_approval", title: "Approval gates", team_role: "creative_director", executor: "human_operator_or_approved_agent", status: approvalActions.length ? "blocked" : "clear", actions: approvalActions },
    { id: "failed_task_recovery", title: "Failed task recovery", team_role: "agent_operator", executor: "operator_or_agent_cli", status: retryActions.length ? "blocked" : "clear", actions: retryActions },
    { id: "credential_setup", title: "Provider credentials", team_role: "generation_td", executor: "operator", status: credentialActions.length ? "warning" : "clear", actions: credentialActions },
    { id: "desktop_recognition", title: "Desktop recognition handoff", team_role: "engine_integrator", executor: "desktop_controller_or_human_takeover", status: state.desktopRecognitionRequests.length ? "waiting_handoff" : "clear", actions: desktopActions, requests: state.desktopRecognitionRequests },
    { id: "runnable_nodes", title: "Runnable workflow nodes", team_role: "output_operator", executor: "pool_cli_or_runtime_http", status: preflight.ready ? "ready" : "gated", actions: runnableActions },
  ];
  const teamRoles = deriveRuntimeTeamRoles(lanes);
  const commands = lanes.flatMap((lane) => [
    ...(lane.commands ?? []).map((command) => ({ lane: lane.id, command })),
    ...(lane.actions ?? []).filter((action) => action.command).map((action) => ({
      lane: lane.id,
      kind: action.kind,
      title: action.title,
      command: action.command,
    })),
  ]);
  return {
    ready: preflight.ready,
    project_filter: project,
    generated_at: state.snapshot?.generatedAt,
    summary: {
      lanes: lanes.length,
      commands: commands.length,
      approval_actions: approvalActions.length,
      retry_actions: retryActions.length,
      credential_actions: credentialActions.length,
      desktop_requests: state.desktopRecognitionRequests.length,
      runnable_node_actions: runnableActions.length,
      team_roles: teamRoles.length,
    },
    team: {
      size: teamRoles.length,
      mode: "five_person_content_burst_team",
      roles: teamRoles,
    },
    control_priority: ["API/MCP", "Skills/CLI", "Desktop Recognition", "Human Takeover"],
    lanes,
    commands,
  };
}

function deriveRuntimeTeamRoles(lanes) {
  const roleDefs = [
    ["creative_director", "Creative Director", "创意验收、审批门和参考方向把关", "human_approval", ["manual_approval"]],
    ["agent_operator", "Agent Operator", "Hermes/Agent CLI 上下文读取、失败恢复和自动化调度", "agent_cli_mcp", ["agent_context", "failed_task_recovery"]],
    ["generation_td", "AI / 3DGS TD", "AI 图片、视频、音频与 3DGS Provider 凭证和生成队列", "provider_gateway", ["credential_setup"]],
    ["engine_integrator", "Engine Integrator", "Unreal/Unity/Blender/TouchDesigner 等外部软件接管", "software_control", ["desktop_recognition"]],
    ["output_operator", "Output Operator", "视频、游戏和交互艺术输出节点推进", "runtime_execution", ["runnable_nodes"]],
  ];
  return roleDefs.map(([id, title, focus, primarySurface, assignedLaneIds]) => {
    const assigned = lanes.filter((lane) => assignedLaneIds.includes(lane.id));
    const queueCount = assigned.reduce((total, lane) => total + (lane.actions?.length ?? 0) + (lane.requests?.length ?? 0), 0);
    return {
      id,
      title,
      focus,
      primary_surface: primarySurface,
      status: teamRoleStatus(assigned),
      queue_count: queueCount,
      assigned_lane_ids: assignedLaneIds,
      lanes: assigned.map((lane) => ({ id: lane.id, title: lane.title, status: lane.status, executor: lane.executor })),
    };
  });
}

function teamRoleStatus(lanes) {
  if (lanes.some((lane) => lane.status === "blocked")) return "blocked";
  if (lanes.some((lane) => ["waiting_handoff", "gated", "warning"].includes(lane.status))) return "attention";
  return "ready";
}

function normalizePreflightStatus(status) {
  return {
    blocked: "blocked",
    warning: "warning",
    warn: "warning",
    passed: "passed",
    pass: "passed",
  }[String(status ?? "").toLowerCase()] ?? "warning";
}

function numberValue(value) {
  const number = Number(value ?? 0);
  return Number.isFinite(number) ? number : 0;
}

function runtimeSoftwareAction(entry) {
  const command = entry.command ?? {};
  const payload = command.payload_json ?? {};
  const verification = entry.verification ?? {};
  const artifacts = Array.isArray(verification.artifacts) ? verification.artifacts : [];
  const desktopStatus = verification.desktop_recognition_status ?? verification.status;
  const status = desktopStatus
    ? normalizeRuntimeStatus(desktopStatus)
    : verification.ok === true
      ? "succeeded"
      : verification.ok === false
        ? "failed"
        : "queued";
  return {
    id: entry.id,
    taskId: entry.task_id,
    adapterId: command.adapter_id ?? entry.adapter_id,
    actionKind: command.action_kind ?? entry.action_kind,
    priority: command.priority ?? "HumanTakeover",
    status,
    message: verification.message ?? payload.instruction ?? payload.scope ?? "software action queued",
    targetWindow: payload.target_window ?? payload.desktop_payload?.target_window,
    artifacts,
    createdAt: formatSnapshotTime(entry.created_at),
  };
}

function runtimeDesktopRecognitionRequest(entry) {
  const command = entry.command ?? {};
  const payload = command.payload_json ?? {};
  const desktopPayload = entry.desktop_payload ?? payload.desktop_payload ?? {};
  const poolAction = entry.pool_desktop_action ?? {};
  const verification = entry.verification ?? {};
  const artifacts = Array.isArray(verification.artifacts) ? verification.artifacts : [];
  const status = normalizeRuntimeStatus(entry.status ?? verification.desktop_recognition_status ?? verification.status ?? "queued");
  return {
    id: entry.software_action_id ?? entry.action_id ?? entry.id,
    taskId: entry.task_id,
    adapterId: entry.adapter_id ?? command.adapter_id,
    actionKind: entry.action_kind ?? command.action_kind,
    status,
    targetWindow: desktopPayload.target_window ?? poolAction.target_window ?? payload.target_window ?? "",
    desktopTool: desktopPayload.tool ?? poolAction.desktop_tool ?? "desktop.control",
    operation: desktopPayload.operation ?? poolAction.operation ?? entry.action_kind ?? "control",
    requestPath: entry.desktop_request_path ?? artifacts.find((artifact) => artifact.includes("desktop-recognition") && artifact.endsWith(".json")) ?? "",
    message: verification.message ?? payload.instruction ?? "等待桌面 controller 接管",
    requestFileAvailable: entry.request_file_available !== false,
    createdAt: formatSnapshotTime(entry.created_at),
  };
}

function deriveDesktopRecognitionRequestsFromSoftwareActions() {
  return (state.softwareActions ?? [])
    .filter((action) => {
      const priority = String(action.priority ?? "").toLowerCase();
      const kind = String(action.actionKind ?? "").toLowerCase();
      const hasDesktopArtifact = (action.artifacts ?? []).some((artifact) => artifact.includes("desktop-recognition"));
      return (
        ["queued", "queued_for_desktop_recognition", "retryable", "running"].includes(action.status)
        && (priority.includes("desktop") || kind.includes("desktop") || hasDesktopArtifact)
      );
    })
    .map((action) => ({
      id: action.id,
      taskId: action.taskId,
      adapterId: action.adapterId,
      actionKind: action.actionKind,
      status: action.status,
      targetWindow: action.targetWindow ?? action.adapterId,
      desktopTool: "desktop.control",
      operation: action.actionKind,
      requestPath: action.artifacts?.find((artifact) => artifact.endsWith(".json")) ?? "",
      message: action.message,
      requestFileAvailable: Boolean(action.artifacts?.length),
      createdAt: action.createdAt,
    }));
}

function normalizeRuntimeStatus(status) {
  if (!status) return "idle";
  return String(status)
    .replace(/([a-z0-9])([A-Z])/g, "$1_$2")
    .replace(/[\s-]+/g, "_")
    .toLowerCase();
}

function normalizeEventLevel(level) {
  return {
    info: "info",
    ok: "ok",
    warn: "warn",
    error: "error",
  }[normalizeRuntimeStatus(level)] ?? "info";
}

function progressForStatus(status) {
  return {
    idle: 0,
    ready: 0,
    queued: 5,
    running: 62,
    waiting_approval: 20,
    succeeded: 100,
    failed: 100,
    cancelled: 100,
    retryable: 35,
  }[status] ?? 0;
}

function formatSnapshotTime(value) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return nowTime();
  return date.toLocaleTimeString("zh-CN", { hour12: false });
}

function compactPrompt(value) {
  const text = typeof value === "string" ? value : JSON.stringify(value ?? "");
  if (text.length <= 90) return text;
  return `${text.slice(0, 87)}...`;
}

function escapeHtml(value) {
  return String(value ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;");
}

function nowTime() {
  return new Date().toLocaleTimeString("zh-CN", { hour12: false });
}

function statusLabel(status) {
  return {
    idle: "空闲",
    ready: "就绪",
    running: "运行中",
    queued: "已排队",
    queued_for_desktop_recognition: "等待桌面接管",
    waiting_approval: "等待确认",
    retryable: "可重试",
    cancelled: "已取消",
    blocked: "阻塞",
    failed: "失败",
    succeeded: "完成",
    skipped: "跳过",
    indexed: "已索引",
    local: "本地",
    planned: "规划中",
    checking: "检查中",
    guarded: "受保护",
    needs_key: "缺少 Key",
    needs_configuration: "待配置",
    needs_execution: "待执行",
    needs_attention: "需处理",
    needs_runtime_snapshot: "需 Runtime",
  }[status] ?? status;
}

function nodeById(id) {
  return state.nodes.find((node) => node.id === id) ?? state.nodes[0];
}

function addEvent(level, text) {
  state.events.unshift({ at: nowTime(), level, text });
  state.events = state.events.slice(0, 24);
}

function renderNodes() {
  renderConnections();
  nodeLayer.innerHTML = state.nodes
    .map(
      (node) => `
        <button
          class="workflow-node status-${node.status} ${node.id === state.selectedNode ? "active" : ""}"
          style="left: ${node.x}px; top: ${node.y}px"
          data-node="${node.id}"
          type="button"
        >
          <div class="node-meta">
            <span>${node.type}</span>
            <span>${node.taskTypeLabel ?? taskTypeLabel(node.taskType)}</span>
            <i class="status-dot" aria-hidden="true"></i>
          </div>
          <strong>${node.title}</strong>
          <small>${statusLabel(node.status)}</small>
          <div class="node-progress" aria-label="node progress">
            <span style="width: ${node.progress}%"></span>
          </div>
        </button>
      `,
    )
    .join("");

  document.querySelectorAll("[data-node]").forEach((button) => {
    button.addEventListener("click", () => selectNode(button.dataset.node));
  });
}

function renderConnections() {
  if (!connectionLayer) return;
  if (!(state.connections ?? []).length) {
    if (state.snapshot) connectionLayer.innerHTML = "";
    return;
  }
  connectionLayer.setAttribute("viewBox", "0 0 980 620");
  connectionLayer.innerHTML = state.connections
    .filter((connection) => connection.from && connection.to)
    .map(renderConnection)
    .join("");
}

function renderConnection(connection, index) {
  const from = connectionAnchor(connection.from, "out");
  const to = connectionAnchor(connection.to, "in");
  const curve = Math.max(60, Math.abs(to.x - from.x) * 0.45);
  const yBend = connection.kind === "feedback" ? -70 : 0;
  const path = `M${from.x} ${from.y} C${from.x + curve} ${from.y + yBend} ${to.x - curve} ${to.y + yBend} ${to.x} ${to.y}`;
  const label = connection.label
    ? `${connection.label} · ${connection.channelLabel ?? connectionChannelLabel(connection.channel)}`
    : connection.channelLabel ?? connectionChannelLabel(connection.kind);
  const labelX = Math.round((from.x + to.x) / 2);
  const labelY = Math.round((from.y + to.y) / 2 + (connection.kind === "gate" ? -14 : -10) + (index % 2) * 18);
  const title = `${connection.from?.title ?? connection.fromId} -> ${connection.to?.title ?? connection.toId} / ${label}`;
  return `
    <g class="connection-route ${connection.kind}-flow" data-channel="${escapeHtml(connection.channel ?? connection.kind)}">
      <title>${escapeHtml(title)}</title>
      <path class="line ${connection.kind}-flow" d="${path}" />
      <text class="connection-label" x="${labelX}" y="${labelY}">${escapeHtml(label)}</text>
    </g>
  `;
}

function connectionAnchor(node, side) {
  return {
    x: Math.round(node.x + (side === "out" ? 180 : 0)),
    y: Math.round(node.y + 54),
  };
}

function selectNode(id) {
  state.selectedNode = id;
  const node = nodeById(id);
  const shouldFetchContext = canFetchRuntimeNodeContext();
  if (state.selectedNodeContext?.node_id !== id) {
    state.selectedNodeContext = null;
    state.selectedNodeContextError = "";
    state.selectedNodeContextStatus = shouldFetchContext ? "loading" : "idle";
  }
  renderSelectedNodeDetails(node);
  saveState();
  renderNodes();
  if (shouldFetchContext) queueRuntimeNodeContextRefresh(id);
}

function canFetchRuntimeNodeContext() {
  return state.snapshot?.mode === "runtime-http" && Boolean(state.snapshot.runtime);
}

function renderSelectedNodeDetails(node = nodeById(state.selectedNode)) {
  document.querySelector("#nodeType").textContent = node.type;
  document.querySelector("#nodeTitle").textContent = node.title;
  document.querySelector("#nodeStatus").textContent = `${statusLabel(node.status)} / ${node.progress}%`;
  document.querySelector("#nodeAgent").textContent = node.agent;
  document.querySelector("#nodeControl").textContent = node.control;
  document.querySelector("#nodeInput").textContent = node.input;
  document.querySelector("#nodeOutput").textContent = node.output;
  const nodeLog = document.querySelector("#nodeLog");
  nodeLog.innerHTML = `
    <span>${nowTime()}</span>
    <p>${node.log}</p>
    ${renderNodeDecision(node)}
    ${renderWorkflowRuntimeContext()}
    ${renderNodeRuntimeContext(node)}
  `;
  document.querySelectorAll("[data-node-control-run]").forEach((button) => {
    button.addEventListener("click", () => {
      state.selectedNode = button.dataset.nodeControlRun;
      runSelectedNode();
    });
  });
}

function queueRuntimeNodeContextRefresh(nodeId) {
  const runtime = state.snapshot?.runtime;
  if (!runtime) return;
  const requestId = ++nodeContextRequestId;

  fetchJson(runtimeNodeContextUrl(runtime, nodeId))
    .then((context) => {
      if (requestId !== nodeContextRequestId || state.selectedNode !== nodeId) return;
      state.selectedNodeContext = context;
      state.selectedNodeContextStatus = "loaded";
      state.selectedNodeContextError = "";
      renderSelectedNodeDetails(nodeById(nodeId));
      saveState();
    })
    .catch((error) => {
      if (requestId !== nodeContextRequestId || state.selectedNode !== nodeId) return;
      state.selectedNodeContext = null;
      state.selectedNodeContextStatus = "failed";
      state.selectedNodeContextError = error.message;
      renderSelectedNodeDetails(nodeById(nodeId));
      saveState();
    });
}

function renderNodeDecision(node) {
  const decision = nodeDecisionFor(node);
  if (!decision) return "";
  return `
    <div class="decision-row node-decision">
      <strong>${decision.title}</strong>
      <div class="decision-metrics">
        ${decision.metrics.map((metric) => `<span>${metric}</span>`).join("")}
      </div>
      <span>${decision.summary}</span>
      ${decision.path ? `<code>${decision.path}</code>` : ""}
    </div>
  `;
}

function renderNodeRuntimeContext(node) {
  if (state.snapshot?.mode !== "runtime-http") return "";
  const context = state.selectedNodeContext?.node_id === node.id ? state.selectedNodeContext : null;
  if (!context) {
    const failed = state.selectedNodeContextStatus === "failed";
    return `
      <div class="decision-row node-decision">
        <strong>Runtime 节点上下文</strong>
        <div class="decision-metrics">
          <span>${failed ? "读取失败" : "读取中"}</span>
          <span>${state.snapshot.projectFilter ?? "all projects"}</span>
        </div>
        <span>${failed ? state.selectedNodeContextError : "正在读取 /api/node-context 的任务、资产和控制账本。"}</span>
      </div>
    `;
  }

  const summary = context.summary ?? {};
  const path = nodeContextPrimaryPath(context);
  return `
    <div class="decision-row node-decision">
      <strong>Runtime 节点上下文</strong>
      <div class="decision-metrics">
        <span>${summary.tasks ?? 0} tasks</span>
        <span>${summary.assets ?? 0} assets</span>
        <span>${summary.provider_requests ?? 0} provider</span>
        <span>${summary.software_actions ?? 0} software</span>
        <span>${summary.agent_sessions ?? 0} agent</span>
      </div>
      <span>${nodeContextSummaryText(context)}</span>
      ${path ? `<code>${path}</code>` : ""}
      ${renderNodeControlContext(context)}
    </div>
  `;
}

function renderWorkflowRuntimeContext() {
  if (state.snapshot?.mode !== "runtime-http") return "";
  const context = state.workflowContext;
  if (!context) {
    const failed = state.workflowContextStatus === "failed";
    return `
      <div class="decision-row node-decision">
        <strong>Runtime 工作流上下文</strong>
        <div class="decision-metrics">
          <span>${failed ? "读取失败" : "等待上下文"}</span>
          <span>${state.snapshot.projectFilter ?? "all projects"}</span>
        </div>
        <span>${failed ? state.workflowContextError : "正在读取 /api/workflow-context 的图、任务和控制账本。"}</span>
      </div>
    `;
  }

  const summary = context.summary ?? {};
  const graphSummary = context.graph?.summary ?? {};
  const path = workflowContextPrimaryPath(context);
  return `
    <div class="decision-row node-decision">
      <strong>Runtime 工作流上下文</strong>
      <div class="decision-metrics">
        <span>${summary.nodes ?? graphSummary.nodes ?? 0} nodes</span>
        <span>${summary.tasks ?? 0} tasks</span>
        <span>${summary.provider_requests ?? 0} provider</span>
        <span>${summary.software_actions ?? 0} software</span>
        <span>${summary.agent_sessions ?? 0} agent</span>
      </div>
      <span>${workflowContextSummaryText(context)}</span>
      ${path ? `<code>${path}</code>` : ""}
    </div>
  `;
}

function workflowContextSummaryText(context) {
  const summary = context.summary ?? {};
  const graphSummary = context.graph?.summary ?? {};
  const workflowName = context.workflow?.name ?? context.workflow_id ?? "workflow";
  const edges = graphSummary.edges ?? 0;
  const approval = summary.blocked_by_approval ? "审批阻断" : "可继续执行";
  return `${workflowName} / ${edges} edges / ${approval}`;
}

function workflowContextPrimaryPath(context) {
  return context.provider_requests?.[0]?.metadata_path
    ?? context.tasks?.[0]?.request_metadata_path
    ?? context.assets?.[0]?.local_path
    ?? context.software_actions?.[0]?.verification?.artifacts?.[0]
    ?? context.agent_sessions?.[0]?.transcript_path
    ?? "";
}

function nodeContextSummaryText(context) {
  const summary = context.summary ?? {};
  const flow = `${summary.incoming_edges ?? 0} in / ${summary.outgoing_edges ?? 0} out`;
  const approval = summary.blocked_by_approval ? "审批阻断" : "可继续执行";
  return `${context.workflow?.name ?? context.workflow_id} / ${flow} / ${approval}`;
}

function nodeContextPrimaryPath(context) {
  return context.provider_requests?.[0]?.metadata_path
    ?? context.tasks?.[0]?.request_metadata_path
    ?? context.assets?.[0]?.local_path
    ?? context.software_actions?.[0]?.verification?.artifacts?.[0]
    ?? context.agent_sessions?.[0]?.transcript_path
    ?? "";
}

function renderNodeControlContext(context) {
  const control = context.control_context;
  if (!control) return "";
  const provider = control.provider?.id ? `provider ${control.provider.id}` : "";
  const software = control.software_adapter?.id ? `software ${control.software_adapter.id}` : "";
  const tools = (control.mcp_tools ?? []).filter((tool) => tool.name).slice(0, 3);
  const commands = (control.cli_commands ?? []).filter((command) => command.command).slice(0, 2);
  const label = [provider, software].filter(Boolean).join(" / ") || control.task_type || "control";
  return `
    <div class="node-control-context">
      <div class="node-control-heading">
        <span>Runtime 控制入口：${label}</span>
        <button class="mini-command primary-mini" data-node-control-run="${context.node_id}" type="button">运行此节点</button>
      </div>
      ${tools.length ? `
        <div class="node-control-list">
          ${tools.map((tool) => `
            <span>
              ${tool.name}
              ${tool.arguments ? `<code>${compactPrompt(tool.arguments)}</code>` : ""}
            </span>
          `).join("")}
        </div>
      ` : ""}
      ${commands.map((command) => `<code>${command.command}</code>`).join("")}
    </div>
  `;
}

function nodeDecisionFor(node) {
  const report = state.hermes.workflowReport;
  const nodeText = `${node.id} ${node.type} ${node.taskType ?? ""} ${node.title} ${node.agent} ${node.control}`.toLowerCase();
  if (nodeText.includes("agent") || nodeText.includes("hermes")) {
    const latest = state.hermes.decisions?.[0];
    if (!latest && !report) return null;
    return {
      title: "Agent/Hermes 决策",
      metrics: [
        `mode ${report?.agentMode ?? "stage"}`,
        statusLabel(report?.agentStatus ?? latest?.status ?? "ready"),
        `${latest?.tokenUsed ?? 0} tokens`,
      ],
      summary: latest
        ? `${latest.title} / tools: ${latest.tools.slice(0, 4).join(", ") || "pending"}`
        : "等待 Runtime Agent session 写入。",
      path: report?.transcriptPath ?? latest?.transcriptPath,
    };
  }
  if (nodeText.includes("3dgs") || nodeText.includes("3d") || nodeText.includes("marble")) {
    const ledger = latestProviderRequestForNode(node);
    return {
      title: "3DGS Adapter 路径",
      metrics: [
        `mode ${ledger?.executionMode ?? report?.threeDgsMode ?? "auto"}`,
        statusLabel(ledger?.status ?? report?.providerStatus ?? node.status),
        "API/MCP > CLI > 桌面接管",
      ],
      summary: ledger
        ? `Provider 请求账本已写入：${ledger.providerId}${ledger.requiresApproval ? " / 审批门" : ""}。`
        : report
          ? "ContentBurst 已记录 3DGS adapter 选择；auto 失败时回落本地 mock。"
          : "运行一次后显示实际 3DGS provider、回退状态和资产入库结果。",
      path: ledger?.metadataPath ?? report?.transcriptPath,
    };
  }
  if (nodeText.includes("unreal") || nodeText.includes("software")) {
    const action = latestSoftwareActionForNode(node);
    return {
      title: "软件控制路径",
      metrics: [
        `mode ${action?.priority ?? report?.unrealMode ?? "auto"}`,
        statusLabel(action?.status ?? report?.softwareStatus ?? node.status),
        action?.actionKind ?? "human takeover ready",
      ],
      summary: action
        ? `软件动作审计已写入：${action.adapterId}${action.targetWindow ? ` / ${action.targetWindow}` : ""}。`
        : report
          ? "ContentBurst 已记录 Unreal MCP/mock 选择；无 API/MCP 时保留人工接管和桌面识别路径。"
          : "运行一次后显示 Unreal MCP、mock fallback 或接管建议。",
      path: action?.artifacts?.[0] ?? report?.transcriptPath,
    };
  }
  if (nodeText.includes("output") || nodeText.includes("输出")) {
    return {
      title: "输出交付状态",
      metrics: [
        statusLabel(report?.outputStatus ?? node.status),
        `${report?.assetsIndexed ?? state.assets.length} assets`,
      ],
      summary: "视频、游戏和交互艺术 manifest 会从 runtime assets 与 software action 结果生成。",
      path: report?.transcriptPath,
    };
  }
  return null;
}

function renderAgents() {
  document.querySelector("#runtimeDiscoveryPanel").innerHTML = renderRuntimeDiscoveryPanel(state.runtimeDiscovery);
  document.querySelector("#agentGrid").innerHTML = state.agents
    .map(
      (agent) => `
        <article class="agent-card">
          <header>
            <div>
              <h3>${agent.name}</h3>
              <p>${agent.role}</p>
            </div>
            <span class="tag ${agent.status === "待确认" ? "warn" : ""}">${agent.status}</span>
          </header>
          <meter min="0" max="100" value="${agent.token}"></meter>
          <div class="tool-list">${agent.tools.map((tool) => `<span>${tool}</span>`).join("")}</div>
        </article>
      `,
    )
    .join("");
}

function renderRuntimeDiscoveryPanel(discovery) {
  if (!discovery) {
    return `
      <article class="runtime-discovery-card muted-panel">
        <div>
          <span>Runtime Discovery</span>
          <strong>等待 Runtime HTTP</strong>
        </div>
        <p>连接 Runtime 后这里会显示 Agent/Hermes 可读取的 endpoint manifest、MCP resource 和 prompt registry。</p>
      </article>
    `;
  }
  const project = discovery.projectFilter || runtimeProjectFilter() || "demo";
  const endpointRows = discoveryEndpointRows(discovery).map((entry) => `
    <div>
      <dt>${entry.label}</dt>
      <dd>${entry.path}</dd>
    </div>
  `).join("");
  const toolNames = discovery.tools
    .map((tool) => tool.name)
    .filter(Boolean)
    .slice(0, 20)
    .join(" / ");
  const command = `pool-cli --project ${project} serve-mcp`;
  return `
    <article class="runtime-discovery-card">
      <div>
        <span>Runtime Discovery</span>
        <strong>${discovery.service} · ${discovery.baseUrl || "local"}</strong>
      </div>
      <p>Agent/Hermes 可先读取 /api/discovery，再按 endpoint、MCP resource 和 prompt registry 选择控制入口。</p>
      <dl>${endpointRows}</dl>
      <div class="runtime-discovery-metrics">
        <span>${discovery.endpointCount} endpoints</span>
        <span>${discovery.resourceCount} MCP resources</span>
        <span>${discovery.toolCount} MCP tools</span>
        <span>${discovery.promptCount} prompts</span>
      </div>
      ${toolNames ? `<p>${escapeHtml(toolNames)}</p>` : ""}
      <code>${command}</code>
    </article>
  `;
}

function discoveryEndpointRows(discovery) {
  const endpoints = discovery?.endpoints ?? {};
  return [
    ["execution plan", "runtime_execution_plan"],
    ["run next", "runtime_execution_plan_run_next"],
    ["events websocket", "events_websocket"],
    ["agent sessions", "agent_sessions"],
    ["session stream", "agent_session_stream"],
    ["session websocket", "agent_session_websocket"],
    ["provider worker", "provider_gateway_worker"],
    ["production evidence", "production_evidence"],
    ["desktop queue", "desktop_recognition_requests"],
  ]
    .map(([label, key]) => ({ label, path: endpoints[key] }))
    .filter((entry) => entry.path);
}

function renderSoftware() {
  document.querySelector("#softwareTable").innerHTML = state.software
    .map((item) => {
      const action = latestSoftwareActionForSoftware(item);
      return `
        <article class="software-card health-${item.health}">
          <header>
            <h3>${item.name}</h3>
            <span class="tag">${item.priority}</span>
          </header>
          <span class="mode">${item.mode}</span>
          <p>${item.scope}</p>
          <div class="adapter-health">
            <span>${statusLabel(item.health)}</span>
            <strong>${item.latency}</strong>
          </div>
          ${item.lastHealth ? `<p class="software-health-note">${item.lastHealth}</p>` : ""}
          ${renderSoftwareLedger(action)}
          ${renderSoftwareContract(item.contract)}
          <div class="inline-actions">
            <button class="mini-command" data-test-software="${item.id ?? softwareAdapterId(item.name)}" type="button">检查</button>
            <button class="mini-command primary-mini" data-stage-software="${item.id ?? softwareAdapterId(item.name)}" type="button">写入动作</button>
            <button class="mini-command" data-software-conformance-package="${item.id ?? softwareAdapterId(item.name)}" type="button">导出验收包</button>
          </div>
        </article>
      `;
    })
    .join("");

  document.querySelectorAll("[data-stage-software]").forEach((button) => {
    button.addEventListener("click", () => stageSoftwareAction(button.dataset.stageSoftware));
  });
  document.querySelectorAll("[data-test-software]").forEach((button) => {
    button.addEventListener("click", () => testSoftwareHealth(button.dataset.testSoftware));
  });
  document.querySelectorAll("[data-software-conformance-package]").forEach((button) => {
    button.addEventListener("click", () => exportSoftwareConformancePackage(button.dataset.softwareConformancePackage));
  });
}

function renderSoftwareContract(contract) {
  if (!contract) return "";
  const route = Array.isArray(contract.control_routes) ? contract.control_routes[0] : null;
  const action = contract.runtime_action?.body ?? {};
  const modes = Array.isArray(contract.control_modes) ? contract.control_modes.join(" / ") : "";
  const localWorker = route?.local_worker;
  const conformance = contract.conformance_runbook ?? {};
  const phases = (conformance.phases ?? []).slice(0, 5);
  const passConditions = (conformance.pass_conditions ?? conformance.passConditions ?? []).slice(0, 2);
  const conformanceHtml = phases.length
    ? `
      <section class="software-conformance">
        <div class="software-conformance-head">
          <span>Conformance</span>
          <strong>${phases.length} phases</strong>
        </div>
        <ol>
          ${phases.map((phase) => `
            <li>
              <strong>${escapeHtml(phase.id ?? "phase")}</strong>
              <code>${escapeHtml(phase.command ?? "")}</code>
            </li>
          `).join("")}
        </ol>
        ${passConditions.length ? `<p>${passConditions.map(escapeHtml).join(" · ")}</p>` : ""}
      </section>
    `
    : "";
  return `
    <div class="software-contract">
      <div>
        <span>Contract</span>
        <strong>${action.priority ?? route?.priority ?? "auto"}</strong>
        <em>${action.action_kind ?? "SoftwareAction"}</em>
      </div>
      <p>${contract.runtime_action?.method ?? "POST"} ${contract.runtime_action?.path ?? "/api/software-actions"}</p>
      <code>${(route?.adapter_kind ?? modes) || contract.adapter_id}</code>
      ${modes ? `<small>${modes}</small>` : ""}
      ${localWorker ? `<small>${localWorker.endpoint_env ?? ""} · ${localWorker.cli ?? ""}</small>` : ""}
      ${conformanceHtml}
    </div>
  `;
}

function renderSoftwareLedger(action) {
  if (!action) return "";
  return `
    <div class="software-ledger status-${action.status}">
      <div>
        <span>${statusLabel(action.status)}</span>
        <strong>${action.priority}</strong>
        <em>${action.actionKind}</em>
      </div>
      <p>${action.message}</p>
      ${action.artifacts?.[0] ? `<code>${action.artifacts[0]}</code>` : ""}
    </div>
  `;
}

function renderDesktopRecognitionQueue() {
  const summary = document.querySelector("#desktopQueueSummary");
  const container = document.querySelector("#desktopRecognitionQueue");
  const runNextButton = document.querySelector("#runDesktopQueue");
  if (!summary || !container) return;
  const requests = state.desktopRecognitionRequests ?? [];
  summary.textContent = `${requests.length} waiting`;
  if (runNextButton) {
    runNextButton.disabled = state.snapshot?.mode !== "runtime-http" || !requests.length;
  }
  container.innerHTML = `
    ${renderDesktopRecognitionContract(state.desktopRecognitionContract)}
    ${requests.length
      ? requests.map(renderDesktopRecognitionRequestCard).join("")
      : `
      <div class="desktop-empty">
        当前没有等待桌面识别接管的动作。
      </div>
    `}
  `;

  document.querySelectorAll("[data-desktop-result]").forEach((button) => {
    button.addEventListener("click", () => {
      completeDesktopRecognitionRequest(button.dataset.desktopResult, button.dataset.desktopStatus);
    });
  });
}

function renderDesktopRecognitionContract(contract) {
  if (!contract) return "";
  const readPath = contract.queue?.read_requests?.http ?? "GET /api/desktop-recognition/requests";
  const callbackPath = contract.queue?.result_callback?.http ?? "POST /api/desktop-recognition/results";
  const statuses = Array.isArray(contract.result_callback?.statuses)
    ? contract.result_callback.statuses.join(" / ")
    : "succeeded / failed / retryable";
  const targets = contract.summary?.software_targets ?? contract.software_targets?.length ?? 0;
  return `
    <article class="desktop-contract">
      <div>
        <span>Contract</span>
        <strong>${contract.summary?.request_contract ?? "desktop-recognition-control-request"}</strong>
        <em>${targets} targets</em>
      </div>
      <p>${readPath} -> ${callbackPath}</p>
      <code>${statuses}</code>
    </article>
  `;
}

function renderDesktopRecognitionRequestCard(request) {
  const runtimeReady = state.snapshot?.mode === "runtime-http";
  const disabled = runtimeReady ? "" : "disabled";
  return `
    <article class="desktop-request-card status-${request.status}">
      <header>
        <div>
          <h4>${request.targetWindow || request.adapterId || "Desktop target"}</h4>
          <p>${request.desktopTool} / ${request.operation}</p>
        </div>
        <span class="tag">${statusLabel(request.status)}</span>
      </header>
      <p>${request.message}</p>
      ${request.requestPath ? `<code>${request.requestPath}</code>` : ""}
      <div class="inline-actions">
        <button class="mini-command primary-mini" data-desktop-result="${request.id}" data-desktop-status="succeeded" ${disabled} type="button">标记成功</button>
        <button class="mini-command" data-desktop-result="${request.id}" data-desktop-status="failed" ${disabled} type="button">标记失败</button>
      </div>
    </article>
  `;
}

function renderProviders() {
  const aiProviders = state.apiProviders.filter((provider) => provider.group === "ai");
  const gsProviders = state.apiProviders.filter((provider) => provider.group === "3dgs");
  const readyAi = aiProviders.filter((provider) => provider.status === "ready").length;
  const readyGs = gsProviders.filter((provider) => provider.status === "ready").length;

  document.querySelector("#aiProviderSummary").textContent = `${readyAi}/${aiProviders.length} ready`;
  document.querySelector("#gsProviderSummary").textContent = `${readyGs}/${gsProviders.length} ready`;
  document.querySelector("#providerGatewayWorkerPanel").innerHTML = renderProviderGatewayWorkerPanel(state.providerGatewayWorkerContract);
  document.querySelector("#aiProviderGrid").innerHTML = aiProviders.map(renderProviderCard).join("");
  document.querySelector("#gsProviderGrid").innerHTML = gsProviders.map(renderProviderCard).join("");

  document.querySelectorAll("[data-test-provider]").forEach((button) => {
    button.addEventListener("click", () => testProviderConnection(button.dataset.testProvider));
  });
  document.querySelectorAll("[data-enqueue-provider]").forEach((button) => {
    button.addEventListener("click", () => enqueueProviderTask(button.dataset.enqueueProvider));
  });
  document.querySelectorAll("[data-run-provider]").forEach((button) => {
    button.addEventListener("click", () => runProviderTask(button.dataset.runProvider));
  });
  document.querySelectorAll("[data-provider-conformance-package]").forEach((button) => {
    button.addEventListener("click", () => exportProviderConformancePackage(button.dataset.providerConformancePackage));
  });
  document.querySelectorAll("[data-save-provider-key]").forEach((button) => {
    button.addEventListener("click", () => saveProviderApiKey(button.dataset.saveProviderKey));
  });
}

function renderIntegrationReadiness() {
  const summaryEl = document.querySelector("#integrationReadinessSummary");
  const panelEl = document.querySelector("#integrationReadinessPanel");
  if (!summaryEl || !panelEl) return;
  const readiness = state.integrationReadiness;
  if (!readiness) {
    summaryEl.textContent = state.snapshot?.mode === "runtime-http" ? "loading" : "local";
    panelEl.innerHTML = `
      <div class="integration-readiness-empty">
        <strong>等待 Runtime 矩阵</strong>
        <span>连接 Runtime HTTP 后读取 /api/integration-readiness 和 pool://integration-readiness。</span>
      </div>
    `;
    return;
  }

  const summary = readiness.summary;
  const total = summary.total || summary.providers + summary.softwareAdapters + 1;
  summaryEl.textContent = `${summary.ready}/${total} ready`;
  panelEl.innerHTML = `
    <div class="integration-readiness-metrics">
      <div><span>Ready</span><strong>${summary.ready}</strong></div>
      <div><span>待配置</span><strong>${summary.needsConfiguration}</strong></div>
      <div><span>待执行</span><strong>${summary.needsExecution}</strong></div>
      <div><span>需处理</span><strong>${summary.needsAttention}</strong></div>
    </div>
    <div class="integration-readiness-lanes">
      ${renderIntegrationReadinessLanes(readiness.lanes)}
    </div>
    <div class="integration-readiness-run-plan">
      <span>Next actions</span>
      ${renderIntegrationRunPlan(readiness.runPlan)}
    </div>
    ${renderConformancePackageCatalogs()}
    <div class="integration-readiness-columns">
      <section>
        <span>Providers</span>
        ${renderIntegrationReadinessRows(readiness.providers, "provider")}
      </section>
      <section>
        <span>Software</span>
        ${renderIntegrationReadinessRows(readiness.softwareAdapters, "software")}
      </section>
      <section>
        <span>Agent / Hermes</span>
        ${renderIntegrationAgentReadiness(readiness.agent)}
      </section>
    </div>
    <code>${escapeHtml(readiness.commands.integration_conformance_package ?? "pool-cli integration-readiness")}</code>
  `;
}

function renderConformancePackageCatalogs() {
  const project = activeProjectSlug();
  const packages = [
    {
      label: "Provider",
      item: state.providerConformancePackage,
      command: `pool-cli --project ${project} provider-conformance-packages`,
    },
    {
      label: "Software",
      item: state.softwareConformancePackage,
      command: `pool-cli --project ${project} software-conformance-packages`,
    },
    {
      label: "Agent/Hermes",
      item: state.agentConformancePackage,
      command: `pool-cli --project ${project} agent-conformance-packages`,
    },
    {
      label: "Integration",
      item: state.integrationConformancePackage,
      command: `pool-cli --project ${project} integration-conformance-packages`,
    },
  ];
  const rows = packages
    .map(({ label, item, command }) => {
      if (!item) {
        return `
          <div class="integration-readiness-row status-partial">
            <strong>${label} 验收包目录</strong>
            <small>等待本地 manifest 入库后恢复最近包。</small>
            <code>${escapeHtml(command)}</code>
          </div>
        `;
      }
      const target = item.targetId || item.projectSlug || item.packageKind || "catalog";
      const runner = item.runnerScriptPath || item.commands?.preflight || command;
      return `
        <div class="integration-readiness-row status-${item.status}">
          <strong>${label} 验收包 · ${escapeHtml(target)}</strong>
          <small>${escapeHtml(item.manifestPath || item.packageDir || "manifest pending")} · ${item.localFiles} files</small>
          ${item.preflightPath ? `<small>preflight · ${escapeHtml(item.preflightPath)}</small>` : ""}
          <code>${escapeHtml(runner)}</code>
        </div>
      `;
    })
    .join("");

  return `
    <div class="integration-readiness-run-plan">
      <span>Conformance catalogs</span>
      ${rows}
    </div>
  `;
}

function renderIntegrationReadinessLanes(lanes = []) {
  if (!lanes.length) return "";
  return lanes
    .map((lane) => `
      <div>
        <span>${escapeHtml(lane.title ?? lane.lane ?? "lane")}</span>
        <strong>${numberValue(lane.ready)}/${numberValue(lane.total)} ready</strong>
        <small>${escapeHtml((lane.targets ?? []).slice(0, 5).join(" / ") || lane.owner || "")}</small>
      </div>
    `)
    .join("");
}

function renderIntegrationRunPlan(items = []) {
  if (!items.length) {
    return `<div class="integration-readiness-row status-ready"><strong>接入闭环已就绪</strong><small>当前 readiness matrix 没有待执行项。</small></div>`;
  }
  return items
    .slice(0, 5)
    .map((item) => {
      const action = item.action ?? {};
      return `
        <div class="integration-readiness-row status-${item.status}">
          <strong>${escapeHtml(action.label ?? item.display_name ?? item.target_id ?? "next action")}</strong>
          <small>${escapeHtml(`${item.lane ?? "lane"} / ${item.target_kind ?? "target"} / ${item.target_id ?? ""}`)}</small>
          ${action.reason ? `<span>${escapeHtml(action.reason)}</span>` : ""}
          ${action.command ? `<code>${escapeHtml(action.command)}</code>` : ""}
        </div>
      `;
    })
    .join("");
}

function renderIntegrationReadinessRows(items, kind) {
  if (!items.length) {
    return `<div class="integration-readiness-row"><strong>等待记录</strong><small>暂无 ${kind} readiness 行。</small></div>`;
  }
  return items
    .slice(0, 5)
    .map((item) => {
      const detail = kind === "provider"
        ? `${item.taskCount} tasks / ${item.requestCount} requests / ${item.configured ? item.keyHint || "key ready" : "no key"}`
        : `${item.actionCount} actions / ${item.taskCount} tasks / ${item.controlModes.join(" + ") || "adapter"}`;
      const command = item.commands.conformance_package ?? item.commands.health ?? "";
      return `
        <div class="integration-readiness-row status-${item.status}">
          <strong>${escapeHtml(item.displayName)}</strong>
          <small>${escapeHtml(`${item.lane} / ${detail}`)}</small>
          <span>${statusLabel(item.status)}</span>
          ${item.nextAction?.label ? `<span>${escapeHtml(item.nextAction.label)}</span>` : ""}
          ${command ? `<code>${escapeHtml(command)}</code>` : ""}
        </div>
      `;
    })
    .join("");
}

function renderIntegrationAgentReadiness(agent) {
  const command = agent.commands?.conformance_package ?? agent.commands?.stage ?? "";
  return `
    <div class="integration-readiness-row status-${agent.status}">
      <strong>Hermes / Agent CLI</strong>
      <small>${agent.lane} / ${agent.sessions} sessions / ${agent.transcripts} transcripts</small>
      <span>${statusLabel(agent.status)}</span>
      ${agent.nextAction?.label ? `<span>${escapeHtml(agent.nextAction.label)}</span>` : ""}
      ${command ? `<code>${escapeHtml(command)}</code>` : ""}
    </div>
  `;
}

function renderProviderCard(provider) {
  const key = apiKeyForProvider(provider.id);
  const ledger = latestProviderRequestForProvider(provider.id);
  const keyStatus = key?.configured ? key.key_hint ?? "已保存" : state.snapshot?.mode === "runtime-http" ? "未保存" : provider.auth;
  return `
    <article class="provider-item status-${provider.status}">
      <header>
        <div>
          <span>${provider.group === "ai" ? "AI" : "3DGS"}</span>
          <h4>${provider.name}</h4>
        </div>
        <strong>${statusLabel(provider.status)}</strong>
      </header>
      <dl class="provider-spec">
        <div>
          <dt>控制</dt>
          <dd>${provider.mode}</dd>
        </div>
        <div>
          <dt>Endpoint</dt>
          <dd>${provider.endpoint}</dd>
        </div>
        <div>
          <dt>Auth</dt>
          <dd>${provider.auth}</dd>
        </div>
        <div>
          <dt>Key</dt>
          <dd>${keyStatus}</dd>
        </div>
        <div>
          <dt>输出</dt>
          <dd>${provider.output}</dd>
        </div>
      </dl>
      <div class="provider-key-row">
        <input class="control-input provider-key-input" data-provider-key-input="${provider.id}" type="password" autocomplete="off" placeholder="${provider.auth}" />
        <button class="mini-command" data-save-provider-key="${provider.id}" type="button">保存 Key</button>
      </div>
      ${renderProviderLedger(ledger)}
      ${renderProviderContract(provider.contract)}
      ${provider.lastHealth ? `<p class="provider-health-note">${provider.lastHealth}</p>` : ""}
      <div class="inline-actions">
        <button class="mini-command" data-test-provider="${provider.id}" type="button">测试连接</button>
        <button class="mini-command primary-mini" data-enqueue-provider="${provider.id}" type="button">创建任务</button>
        <button class="mini-command primary-mini" data-run-provider="${provider.id}" type="button">运行</button>
        <button class="mini-command" data-provider-conformance-package="${provider.id}" type="button">导出验收包</button>
      </div>
    </article>
  `;
}

function renderProviderGatewayWorkerPanel(contract) {
  if (!contract) {
    return `
      <article class="provider-gateway-worker muted-panel">
        <div>
          <span>Provider Gateway Worker</span>
          <strong>等待 Runtime 合同</strong>
        </div>
        <p>连接 Runtime HTTP 后读取 /api/provider-gateway-worker。</p>
      </article>
    `;
  }
  const aiEnv = contract.pool_adapter_usage?.ai_media?.endpoint_env ?? "POOL_MEDIA_GATEWAY_ENDPOINT";
  const gsEnv = contract.pool_adapter_usage?.three_dgs?.endpoint_env ?? "POOL_3DGS_GATEWAY_ENDPOINT";
  const cli = contract.cli?.primary ?? "pool-cli provider-gateway-worker";
  const mcpTool = "pool_provider_gateway_worker";
  const conformance = contract.conformance_runbook ?? {};
  const phases = (conformance.phases ?? []).slice(0, 6);
  const passConditions = (conformance.pass_conditions ?? conformance.passConditions ?? []).slice(0, 3);
  const conformanceHtml = phases.length
    ? `
      <section class="provider-conformance">
        <div class="provider-conformance-head">
          <span>Conformance</span>
          <strong>${phases.length} phases</strong>
        </div>
        <ol>
          ${phases.map((phase) => `
            <li>
              <strong>${escapeHtml(phase.id ?? "phase")}</strong>
              <code>${escapeHtml(phase.command ?? "")}</code>
            </li>
          `).join("")}
        </ol>
        ${passConditions.length ? `<p>${passConditions.map(escapeHtml).join(" · ")}</p>` : ""}
      </section>
    `
    : "";
  return `
    <article class="provider-gateway-worker">
      <div>
        <span>Provider Gateway Worker</span>
        <strong>${contract.service ?? "pool-provider-gateway-worker"}</strong>
      </div>
      <p>${contract.purpose ?? "AI media / 3DGS local HTTP forwarder"}</p>
      <dl>
        <div>
          <dt>AI media env</dt>
          <dd>${aiEnv}</dd>
        </div>
        <div>
          <dt>3DGS env</dt>
          <dd>${gsEnv}</dd>
        </div>
        <div>
          <dt>MCP tool</dt>
          <dd>${mcpTool}</dd>
        </div>
      </dl>
      <code>${cli}</code>
      ${conformanceHtml}
    </article>
  `;
}

function renderProviderContract(contract) {
  if (!contract) return "";
  const profile = contract.profile ?? {};
  const submit = contract.gateway_submit ?? {};
  const poll = contract.gateway_poll ?? {};
  const policy = contract.local_output_policy ?? {};
  const label = contract.adapter_kind ?? "provider_contract";
  const profileLabel = profile.profile_id ?? profile.task_type ?? contract.provider_id;
  const submitPath = submit.path ?? contract.runtime_provider_run?.path ?? "/api/provider-runs";
  const pollPath = poll.path_template ?? "native";
  const localPolicy = policy.output_contract ?? (policy.local_files_authoritative ? "local files" : "runtime");
  return `
    <div class="provider-contract">
      <div>
        <span>Contract</span>
        <strong>${label}</strong>
      </div>
      <p>${profileLabel} · submit ${submitPath} · poll ${pollPath}</p>
      <code>${localPolicy}</code>
    </div>
  `;
}

function renderProviderLedger(ledger) {
  if (!ledger) return "";
  return `
    <div class="provider-ledger">
      <div>
        <span>${statusLabel(ledger.status)}</span>
        <strong>${ledger.executionMode}</strong>
        <em>${ledger.createdAt}</em>
      </div>
      ${ledger.prompt ? `<p>${ledger.prompt}</p>` : ""}
      ${ledger.metadataPath ? `<code>${ledger.metadataPath}</code>` : ""}
    </div>
  `;
}

function renderRuntimeBudget() {
  const summaryEl = document.querySelector("#runtimeBudgetSummary");
  const panelEl = document.querySelector("#runtimeBudgetPanel");
  if (!summaryEl || !panelEl) return;
  const budget = state.runtimeBudget ?? normalizeRuntimeBudget(deriveRuntimeBudgetFromSnapshot({
    project_filter: state.snapshot?.projectFilter,
    generated_at: state.snapshot?.generatedAt,
    stats: state.snapshot?.stats ?? {},
    tasks: state.tasks.map((task) => ({
      provider_id: task.tool,
      status: task.status === "waiting_approval" ? "WaitingApproval" : task.status,
      cost_estimate_tokens: task.cost,
      requires_approval: task.status === "waiting_approval",
    })),
    provider_requests: state.providerRequests.map((request) => ({
      provider_id: request.providerId,
      created_at: request.createdAt,
    })),
    api_keys: state.apiKeys,
  }));
  const summary = budget.summary;
  const apiKeyAudit = state.apiKeyAudit ?? deriveApiKeyAuditFromSnapshot({ api_keys: state.apiKeys ?? [] });
  const providerRows = budget.providerCredentials.length
    ? budget.providerCredentials
        .map((provider) => {
          return `
            <div class="runtime-provider-row status-${provider.configured ? "ready" : "needs_key"}">
              <span>${providerDisplayName(provider.providerId)}</span>
              <strong>${provider.configured ? provider.keyHint || "已配置" : "未记录"}</strong>
              <small>${provider.providerRequestCount} requests / ${formatTokens(provider.tokenEstimate)} tokens</small>
            </div>
          `;
        })
        .join("")
    : `
      <div class="runtime-provider-row">
        <span>Provider credentials</span>
        <strong>等待 Runtime</strong>
        <small>连接 Runtime 或写入 API Key 后显示。</small>
      </div>
    `;
  const apiKeyAuditRows = apiKeyAudit.items.length
    ? apiKeyAudit.items
        .slice(0, 5)
        .map((item) => {
          const status = item.rotationDue || !item.encrypted ? "needs_key" : "ready";
          const detail = [
            item.backend || item.storage || "unknown",
            item.owner ? `owner ${item.owner}` : "",
            item.ageDays === null ? "" : `${item.ageDays}d old`,
          ].filter(Boolean).join(" / ");
          return `
            <div class="runtime-provider-row status-${status}">
              <span>${providerDisplayName(item.provider)}</span>
              <strong>${item.rotationDue ? "需轮换" : "有效"}</strong>
              <small>${detail || "credential audit"}</small>
            </div>
          `;
        })
        .join("")
    : `
      <div class="runtime-provider-row">
        <span>Credential audit</span>
        <strong>等待 Key</strong>
        <small>连接 Runtime 后从 /api/api-keys 读取轮换审计。</small>
      </div>
    `;

  summaryEl.textContent = `${formatTokens(summary.tokenTotal)} tokens`;
  panelEl.innerHTML = `
    <div class="runtime-budget-metrics">
      <div>
        <span>总估算</span>
        <strong>${formatTokens(summary.tokenTotal)}</strong>
      </div>
      <div>
        <span>待审批</span>
        <strong>${formatTokens(summary.waitingApprovalEstimatedTokens)}</strong>
      </div>
      <div>
        <span>Agent 预算余量</span>
        <strong>${summary.budgetRemaining === null ? "未设置" : formatSignedTokens(summary.budgetRemaining)}</strong>
      </div>
      <div>
        <span>Provider Key</span>
        <strong>${summary.configuredApiKeys}/${summary.trackedProviders}</strong>
      </div>
    </div>
    <div class="runtime-budget-source">
      <span>${state.snapshot?.mode === "runtime-http" ? "Runtime HTTP" : "Snapshot"}</span>
      <small>${summary.missingRuntimeCredentials} 个 Provider 未记录 Runtime Key，${summary.approvalGates} 个确认门。</small>
    </div>
    <div class="runtime-budget-source">
      <span>Credential audit</span>
      <small>${apiKeyAudit.rotationDue} 个需轮换，${apiKeyAudit.unencrypted} 个 legacy/plaintext，默认 ${apiKeyAudit.defaultRotationDays} 天。</small>
    </div>
    <div class="runtime-provider-list">${providerRows}</div>
    <div class="runtime-provider-list">${apiKeyAuditRows}</div>
  `;
}

function renderRuntimePreflight() {
  const summaryEl = document.querySelector("#runtimePreflightSummary");
  const panelEl = document.querySelector("#runtimePreflightPanel");
  if (!summaryEl || !panelEl) return;
  const preflight = state.runtimePreflight ?? normalizeRuntimePreflight(deriveRuntimePreflightFromSnapshot({
    project_filter: state.snapshot?.projectFilter,
    generated_at: state.snapshot?.generatedAt,
    workflows: state.runtimeGraph?.workflows ?? [],
    node_states: state.tasks.map((task) => ({ node_id: task.nodeId })),
    tasks: state.tasks.map((task) => ({
      id: task.id,
      project_slug: activeProjectSlug(),
      title: task.title,
      provider_id: task.tool,
      status: task.status === "waiting_approval" ? "WaitingApproval" : task.status,
      requires_approval: task.status === "waiting_approval",
    })),
    provider_requests: state.providerRequests.map((request) => ({
      provider_id: request.providerId,
    })),
    api_keys: state.apiKeys,
  }));
  const summary = preflight.summary;
  const checkRows = preflight.checks.length
    ? preflight.checks
        .map(
          (check) => `
            <div class="preflight-check status-${check.status}">
              <span>${statusLabel(check.status)}</span>
              <strong>${check.title}</strong>
              <small>${check.detail}</small>
            </div>
          `,
        )
        .join("")
    : `
      <div class="preflight-check status-passed">
        <span>ready</span>
        <strong>等待 Runtime 检查</strong>
        <small>连接 Runtime 后显示运行前阻塞项和建议命令。</small>
      </div>
    `;
  const actionRows = preflight.nextActions.length
    ? preflight.nextActions
        .slice(0, 4)
        .map(
          (action) => `
            <div class="preflight-action">
              <span>${action.kind}</span>
              <strong>${action.title}</strong>
              ${action.command ? `<code>${action.command}</code>` : ""}
            </div>
          `,
        )
        .join("")
    : `
      <div class="preflight-action">
        <span>next</span>
        <strong>无必需人工动作</strong>
      </div>
    `;

  summaryEl.textContent = preflight.ready ? "ready" : `${summary.blocked} blocked`;
  panelEl.innerHTML = `
    <div class="preflight-summary">
      <div><span>Blocked</span><strong>${summary.blocked}</strong></div>
      <div><span>Warnings</span><strong>${summary.warnings}</strong></div>
      <div><span>Runnable</span><strong>${summary.runnableNodes}</strong></div>
      <div><span>Approvals</span><strong>${summary.approvalGates}</strong></div>
    </div>
    <div class="preflight-check-list">${checkRows}</div>
    <div class="preflight-action-list">${actionRows}</div>
  `;
}

function renderRuntimeHandoff() {
  const summaryEl = document.querySelector("#runtimeHandoffSummary");
  const panelEl = document.querySelector("#runtimeHandoffPanel");
  if (!summaryEl || !panelEl) return;
  const handoff = state.runtimeHandoff ?? normalizeRuntimeHandoff(deriveRuntimeHandoffFromState());
  const executionPlan = state.runtimeExecutionPlan ?? normalizeRuntimeExecutionPlan(null);
  const summary = handoff.summary;
  const planSummary = executionPlan.summary;
  const lanes = handoff.lanes.length
    ? handoff.lanes
        .map((lane) => {
          const commands = [
            ...lane.commands,
            ...lane.actions.map((action) => action.command).filter(Boolean),
          ].slice(0, 3);
          return `
            <div class="handoff-lane status-${lane.status}">
              <span>${lane.teamRole || lane.executor}</span>
              <strong>${lane.title}</strong>
              <small>${lane.status} / ${lane.executor} / ${lane.actions.length} actions / ${lane.requests.length} requests</small>
              ${commands.length ? `<div>${commands.map((command) => `<code>${command}</code>`).join("")}</div>` : ""}
            </div>
          `;
        })
        .join("")
    : `
      <div class="handoff-lane status-ready">
        <span>handoff</span>
        <strong>等待 Runtime runbook</strong>
        <small>连接 Runtime 后显示 Hermes、Agent CLI、桌面控制器与人工接管队列。</small>
      </div>
    `;
  const teamRows = handoff.team.roles.length
    ? handoff.team.roles
        .map(
          (role) => `
            <div class="handoff-team-role status-${role.status}">
              <span>${role.primarySurface}</span>
              <strong>${role.title}</strong>
              <small>${role.status} / ${role.queueCount} queue / ${role.assignedLaneIds.join(", ")}</small>
              <p>${role.focus}</p>
            </div>
          `,
        )
        .join("")
    : `
      <div class="handoff-team-role status-ready">
        <span>team</span>
        <strong>等待团队分工</strong>
        <small>连接 Runtime 后显示 5 人内容爆发团队的 lane 绑定。</small>
      </div>
    `;
  const commandRows = handoff.commands.length
    ? handoff.commands
        .slice(0, 5)
        .map(
          (command) => `
            <div class="handoff-command">
              <span>${command.lane}</span>
              <strong>${command.title}</strong>
              <code>${command.command}</code>
            </div>
          `,
        )
        .join("")
    : `
      <div class="handoff-command">
        <span>command</span>
        <strong>暂无执行命令</strong>
      </div>
    `;
  const executionRows = executionPlan.steps.length
    ? executionPlan.steps
        .slice(0, 8)
        .map((step) => {
          const contractLabels = step.contracts
            .map((contract) => contract.kind ?? contract.mcp_uri ?? "contract")
            .slice(0, 3)
            .join(" / ");
          return `
            <div class="execution-step phase-${step.phase}">
              <span>${step.sequence || "-"} · ${step.taskType}</span>
              <strong>${step.title}</strong>
              <small>${step.phase} / ${step.status} / ${step.gateKind}</small>
              ${contractLabels ? `<em>${contractLabels}</em>` : ""}
              ${step.command ? `<code>${step.command}</code>` : ""}
            </div>
          `;
        })
        .join("")
    : `
      <div class="execution-step phase-ready">
        <span>plan</span>
        <strong>等待 Runtime execution plan</strong>
        <small>连接 Runtime 后显示按节点拓扑排序的执行步骤。</small>
      </div>
    `;
  const runNextResult = state.runtimeRunNextResult;
  const latestHandoffPackage = state.runtimeHandoffPackage;
  const handoffChecklistRows = latestHandoffPackage?.operatorChecklist?.length
    ? latestHandoffPackage.operatorChecklist
        .slice(0, 3)
        .map((step) => {
          const action = step.action || step.verify || step.path || "handoff step";
          const command = step.command ? `<code>${escapeHtml(step.command)}</code>` : "";
          return `<small>${escapeHtml(step.owner || `step ${step.step}`)} · ${escapeHtml(action)}</small>${command}`;
        })
        .join("")
    : "";
  const handoffAgentCommand =
    latestHandoffPackage?.agentEntrypoint?.mcp_stdio ??
    latestHandoffPackage?.agentEntrypoint?.mcpStdio ??
    "";
  const handoffMcpResources = Array.isArray(latestHandoffPackage?.mcpResources)
    ? latestHandoffPackage.mcpResources.slice(0, 3).join(" · ")
    : "";
  const handoffPackageRow = latestHandoffPackage
    ? `
      <div class="handoff-command">
        <span>${escapeHtml(latestHandoffPackage.status)}</span>
        <strong>最近接管包 · ${latestHandoffPackage.localFiles} files</strong>
        <small>manifest · ${escapeHtml(latestHandoffPackage.manifestPath || "pending")}</small>
        ${latestHandoffPackage.integrationReadinessPath ? `<small>readiness · ${escapeHtml(latestHandoffPackage.integrationReadinessPath)}</small>` : ""}
        ${handoffChecklistRows}
        ${handoffAgentCommand ? `<code>${escapeHtml(handoffAgentCommand)}</code>` : ""}
        ${handoffMcpResources ? `<small>MCP · ${escapeHtml(handoffMcpResources)}</small>` : ""}
        ${latestHandoffPackage.workerSelfChecksPath ? `<code>${escapeHtml(latestHandoffPackage.workerSelfChecksPath)}</code>` : ""}
      </div>
    `
    : "";
  const runNextRow = runNextResult
    ? `
      <div class="handoff-command">
        <span>${runNextResult.executed ? "executed" : "preview"}</span>
        <strong>${runNextResult.selected_step?.title ?? "Runtime 下一步"}</strong>
        <code>${runNextResult.action?.command ?? runNextResult.action?.mcp_tool ?? "runtime execution plan"}</code>
      </div>
    `
    : "";
  const runNextDisabled = state.snapshot?.mode === "runtime-http" ? "" : "disabled";

  summaryEl.textContent = `${summary.commands} cmds / ${planSummary.steps} steps`;
  panelEl.innerHTML = `
    <div class="handoff-priority">${handoff.controlPriority.map((item) => `<span>${item}</span>`).join("")}</div>
    <div class="preflight-summary">
      <div><span>Lanes</span><strong>${summary.lanes}</strong></div>
      <div><span>Commands</span><strong>${summary.commands}</strong></div>
      <div><span>Steps</span><strong>${planSummary.steps}</strong></div>
      <div><span>Gated</span><strong>${planSummary.gatedSteps}</strong></div>
      <div><span>Desktop</span><strong>${summary.desktopRequests}</strong></div>
      <div><span>Run nodes</span><strong>${summary.runnableNodeActions}</strong></div>
      <div><span>Team</span><strong>${summary.teamRoles || handoff.team.size}</strong></div>
    </div>
    <div class="inline-actions">
      <button class="mini-command" data-runtime-run-next="preview" ${runNextDisabled} type="button">预览下一步</button>
      <button class="mini-command primary-mini" data-runtime-run-next="execute" ${runNextDisabled} type="button">执行下一步</button>
    </div>
    ${runNextRow}
    ${handoffPackageRow}
    <div class="handoff-team-list">${teamRows}</div>
    <div class="execution-step-list">${executionRows}</div>
    <div class="handoff-lane-list">${lanes}</div>
    <div class="handoff-command-list">${commandRows}</div>
  `;
  panelEl.querySelectorAll("[data-runtime-run-next]").forEach((button) => {
    button.addEventListener("click", () => runRuntimeExecutionPlanNext(button.dataset.runtimeRunNext === "execute"));
  });
}

function renderPrdReadiness() {
  const summaryEl = document.querySelector("#prdReadinessSummary");
  const panelEl = document.querySelector("#prdReadinessPanel");
  if (!summaryEl || !panelEl) return;
  const readiness = state.runtimePrdReadiness;
  if (!readiness) {
    summaryEl.textContent = "waiting";
    panelEl.innerHTML = `
      <div class="prd-requirement status-partial">
        <span>readiness</span>
        <strong>等待 Runtime PRD 审计</strong>
        <small>连接 Runtime 后读取 /api/prd-readiness、/api/prd-completion-gate 和 pool://prd-readiness。</small>
      </div>
    `;
    return;
  }

  const summary = readiness.summary;
  const gate = state.runtimePrdCompletionGate ?? readiness.completionGate;
  const coreReadiness = state.runtimeCoreArchitectureReadiness;
  const coreGate = state.runtimeCoreArchitectureGate ?? coreReadiness?.architectureGate;
  const coreSummary = state.runtimeCoreArchitectureGate?.summary ?? coreReadiness?.summary ?? {};
  const corePackage = state.runtimeCoreArchitecturePackage;
  const completionPackage = state.runtimePrdCompletionPackage;
  const runtimeReady = state.snapshot?.mode === "runtime-http";
  const gateCommand =
    gate?.proofCommands?.closeout_preflight ??
    gate?.proofCommands?.readiness ??
    "pool-cli --project demo prd-readiness";
  const packageCommand = `pool-cli --project ${activeProjectSlug()} prd-completion-package --output-dir worlds/${activeProjectSlug()}/output --include-snapshot`;
  const rows = readiness.requirements
    .filter((requirement) => requirement.status !== "ready")
    .slice(0, 5)
    .map(
      (requirement) => `
        <div class="prd-requirement status-${requirement.status}">
          <span>${statusLabel(requirement.status)}</span>
          <strong>${requirement.title}</strong>
          <small>${requirement.gaps[0] ?? requirement.summary}</small>
          ${requirement.nextActions[0] ? `<code>${requirement.nextActions[0]}</code>` : ""}
        </div>
      `,
    )
    .join("");
  summaryEl.textContent = readiness.overallStatus;
  panelEl.innerHTML = `
    <div class="prd-readiness-summary">
      <div><span>Ready</span><strong>${summary.ready}</strong></div>
      <div><span>Partial</span><strong>${summary.partial}</strong></div>
      <div><span>Blocked</span><strong>${summary.blocked}</strong></div>
      <div><span>Total</span><strong>${summary.total}</strong></div>
    </div>
    ${
      coreReadiness
        ? `
          <div class="prd-requirement status-${coreGate?.readyForCoreArchitecture ? "ready" : "partial"}">
            <span>${coreGate?.status ?? coreReadiness.overallStatus}</span>
            <strong>${coreGate?.readyForCoreArchitecture ? "核心架构门槛已满足" : "核心架构门槛未满足"}</strong>
            <small>${coreGate?.readyForCoreArchitecture ? `本地运行框架 ${coreSummary.ready ?? coreReadiness.summary.ready}/${coreSummary.total ?? coreReadiness.summary.total} 项已证明；真实生产证据仍由 PRD completion gate 单独阻断。` : `${coreGate?.incompleteRequirements?.length ?? coreSummary.partial ?? coreReadiness.summary.partial} 个核心架构要求仍需补齐。`}</small>
            <code>${escapeHtml(coreGate?.proofCommands?.core_architecture_gate ?? coreGate?.proofCommands?.core_architecture_readiness ?? "pool-cli --project demo core-architecture-gate --require-ready")}</code>
            <div class="inline-actions compact-actions">
              <button id="createCoreArchitecturePackage" class="mini-command" type="button" ${runtimeReady ? "" : "disabled"}>写核心架构包</button>
            </div>
            <code>${escapeHtml(coreGate?.proofCommands?.core_architecture_package ?? `pool-cli --project ${activeProjectSlug()} core-architecture-package --output-dir worlds/${activeProjectSlug()}/output --include-snapshot`)}</code>
            ${
              corePackage
                ? `<small>最近核心包：${escapeHtml(corePackage.manifestPath || corePackage.packageDir)} · ${corePackage.localFiles} files · ${corePackage.readyForCoreArchitecture ? "ready" : "incomplete"}</small>`
                : ""
            }
          </div>
        `
        : ""
    }
    <div class="prd-requirement status-${gate?.readyForCompletion ? "ready" : "partial"}">
      <span>${gate?.status ?? "gate"}</span>
      <strong>${gate?.readyForCompletion ? "PRD 完成门槛已满足" : "PRD 完成门槛未满足"}</strong>
      <small>${gate?.readyForCompletion ? "当前 Runtime snapshot 已证明全部 PRD requirement ready。" : `${gate?.incompleteRequirements?.length ?? summary.partial} 个要求仍需补齐生产证据或复核。`}</small>
      <code>${escapeHtml(gateCommand)}</code>
      <div class="inline-actions compact-actions">
        <button id="createPrdCompletionPackage" class="mini-command" type="button" ${runtimeReady ? "" : "disabled"}>写完成证明包</button>
      </div>
      <code>${escapeHtml(packageCommand)}</code>
      ${
        completionPackage
          ? `<small>最近证明包：${escapeHtml(completionPackage.manifestPath || completionPackage.packageDir)} · ${completionPackage.localFiles} files · ${completionPackage.readyForCompletion ? "ready" : "incomplete"}</small>`
          : ""
      }
    </div>
    <div class="prd-requirement-list">
      ${
        rows || `
          <div class="prd-requirement status-ready">
            <span>ready</span>
            <strong>所有 PRD 项均已通过当前审计</strong>
            <small>继续以真实外部服务和软件执行证据复核。</small>
          </div>
        `
      }
    </div>
  `;
  document.querySelector("#createCoreArchitecturePackage")?.addEventListener("click", createCoreArchitecturePackage);
  document.querySelector("#createPrdCompletionPackage")?.addEventListener("click", createPrdCompletionPackage);
}

function renderProductionEvidenceImport() {
  const summaryEl = document.querySelector("#productionEvidenceSummary");
  const inputEl = document.querySelector("#productionEvidenceBundle");
  const resultEl = document.querySelector("#productionEvidenceResult");
  const taskSelect = document.querySelector("#productionEvidenceTaskSelect");
  const templateButton = document.querySelector("#loadProductionEvidenceTemplate");
  const itemTemplateButton = document.querySelector("#loadProductionEvidenceItemTemplate");
  const taskClaimButton = document.querySelector("#claimProductionEvidenceTask");
  const validateItemButton = document.querySelector("#validateProductionEvidenceItem");
  const ledgerBundleButton = document.querySelector("#loadProductionEvidenceLedgerBundle");
  const handoffPackageButton = document.querySelector("#createProductionEvidenceHandoffPackage");
  const runPlanButton = document.querySelector("#createProductionEvidenceRunPlan");
  const validateButton = document.querySelector("#validateProductionEvidence");
  const mergeButton = document.querySelector("#mergeProductionEvidence");
  const closeoutButton = document.querySelector("#closeoutProductionEvidence");
  const closeoutImportButton = document.querySelector("#closeoutImportProductionEvidence");
  const importButton = document.querySelector("#importProductionEvidence");
  const submitItemButton = document.querySelector("#submitProductionEvidenceItem");
  if (!summaryEl || !inputEl || !resultEl) return;

  const latestTemplate = state.productionEvidenceTemplate;
  const latestItemTemplate = state.productionEvidenceItemTemplate;
  const latestTaskClaim = state.productionEvidenceTaskClaim;
  const latestItemValidation = state.productionEvidenceItemValidation;
  const latestLedgerBundle = state.productionEvidenceLedgerBundle;
  const latestHandoffPackage = state.productionEvidenceHandoffPackage;
  const latestRunPlan = state.productionEvidenceRunPlan;
  const latestMerge = state.productionEvidenceMerge;
  const latestCloseout = state.productionEvidenceCloseout;
  const latestImport = state.productionEvidenceImport;
  const latestValidation = state.productionEvidenceValidation;
  const latestRequirements = state.productionEvidenceRequirements;
  const latestTasks = state.productionEvidenceTasks;
  const latestHandoff = state.productionEvidenceHandoff;
  const runtimeReady = state.snapshot?.mode === "runtime-http";
  const evidenceTasks = latestTasks?.tasks?.length
    ? latestTasks.tasks
    : latestRequirements?.evidenceTasks ?? [];
  const requirementsBlock = renderProductionEvidenceRequirementsBlock(latestRequirements, latestTasks, latestHandoff, runtimeReady);
  if (taskSelect) {
    const previousValue = taskSelect.value;
    taskSelect.innerHTML = evidenceTasks.length
      ? evidenceTasks.map((task) => `<option value="${escapeHtml(task.id)}">${escapeHtml(task.kind)} · ${escapeHtml(task.targetId || task.title)}</option>`).join("")
      : `<option value="">等待 evidence task</option>`;
    if (previousValue && evidenceTasks.some((task) => task.id === previousValue)) {
      taskSelect.value = previousValue;
    }
    taskSelect.disabled = !runtimeReady || !evidenceTasks.length;
  }
  if (templateButton) templateButton.disabled = !runtimeReady;
  if (itemTemplateButton) itemTemplateButton.disabled = !runtimeReady || !evidenceTasks.length;
  if (taskClaimButton) taskClaimButton.disabled = !runtimeReady || !evidenceTasks.length;
  if (validateItemButton) validateItemButton.disabled = !runtimeReady;
  if (ledgerBundleButton) ledgerBundleButton.disabled = !runtimeReady;
  if (handoffPackageButton) handoffPackageButton.disabled = !runtimeReady;
  if (runPlanButton) runPlanButton.disabled = !runtimeReady;
  if (mergeButton) mergeButton.disabled = !runtimeReady;
  if (closeoutButton) closeoutButton.disabled = !runtimeReady;
  if (closeoutImportButton) closeoutImportButton.disabled = !runtimeReady;
  if (validateButton) validateButton.disabled = !runtimeReady;
  if (importButton) importButton.disabled = !runtimeReady;
  if (submitItemButton) submitItemButton.disabled = !runtimeReady;
  inputEl.placeholder = defaultProductionEvidenceBundleText();

  if (!latestImport && !latestValidation) {
    if (latestItemValidation) {
      const validation = latestItemValidation.validation;
      summaryEl.textContent = `${validation.summary.providers + validation.summary.softwareActions + validation.summary.desktopVision} item dry-run`;
      resultEl.innerHTML = `
        ${requirementsBlock}
        <div>
          <span>${latestItemValidation.valid ? "valid" : "invalid"}</span>
          <strong>${validation.summary.providers} providers / ${validation.summary.softwareActions} software / ${validation.summary.desktopVision} vision</strong>
          <small>writes ${latestItemValidation.writes} · ${latestItemValidation.commands.submit}</small>
          <small>${latestItemValidation.commands.validateBundle}</small>
        </div>
      `;
      return;
    }

    if (latestTaskClaim) {
      summaryEl.textContent = "1 claim";
      resultEl.innerHTML = `
        ${requirementsBlock}
        <div>
          <span>${latestTaskClaim.status}</span>
          <strong>${latestTaskClaim.selector.kind} · ${latestTaskClaim.selector.targetId}</strong>
          <small>${latestTaskClaim.claimPath} · ${latestTaskClaim.commands.validateItem}</small>
          <small>${latestTaskClaim.commands.submitItem}</small>
        </div>
      `;
      return;
    }

    if (latestItemTemplate) {
      summaryEl.textContent = "1 item";
      resultEl.innerHTML = `
        ${requirementsBlock}
        <div>
          <span>item</span>
          <strong>${latestItemTemplate.selector.kind} · ${latestItemTemplate.selector.targetId}</strong>
          <small>${latestItemTemplate.commands.submit ?? "submit-production-evidence-item <item.json>"}</small>
        </div>
      `;
      return;
    }

    if (latestTemplate) {
      const total = latestTemplate.summary.providers + latestTemplate.summary.softwareActions + latestTemplate.summary.desktopVision;
      summaryEl.textContent = `${total} template`;
      resultEl.innerHTML = `
        ${requirementsBlock}
        <div>
          <span>${latestTemplate.readyForImport ? "ready" : "scaffold"}</span>
          <strong>${latestTemplate.summary.providers} providers / ${latestTemplate.summary.softwareActions} software / ${latestTemplate.summary.desktopVision} vision</strong>
          <small>${latestTemplate.source} · ${latestTemplate.commands.validate}</small>
        </div>
      `;
      return;
    }

    if (latestLedgerBundle) {
      const total = latestLedgerBundle.summary.providers + latestLedgerBundle.summary.softwareActions + latestLedgerBundle.summary.desktopVision;
      summaryEl.textContent = `${total} ledger`;
      resultEl.innerHTML = `
        ${requirementsBlock}
        <div>
          <span>${latestLedgerBundle.readyForImport ? "ready" : "not ready"}</span>
          <strong>${latestLedgerBundle.summary.providers} providers / ${latestLedgerBundle.summary.softwareActions} software / ${latestLedgerBundle.summary.desktopVision} vision</strong>
          <small>${latestLedgerBundle.readyItems} ready · ${latestLedgerBundle.incompleteItems} incomplete · ${latestLedgerBundle.commands.validate}</small>
        </div>
      `;
      return;
    }

    if (latestHandoffPackage) {
      const bridgeItem = latestHandoffPackage.items.find((item) => item.bridgeWorker?.available);
      const providerStartCommand = latestHandoffPackage.providerGatewayWorkerStartCommands[0];
      const bridgeStartCommand = latestHandoffPackage.bridgeWorkerStartCommands[0];
      summaryEl.textContent = `${latestHandoffPackage.itemCount} package`;
      resultEl.innerHTML = `
        ${requirementsBlock}
        <div>
          <span>${latestHandoffPackage.status}</span>
          <strong>${latestHandoffPackage.itemCount} item files / ${latestHandoffPackage.localFiles} local files</strong>
          <small>${latestHandoffPackage.manifestPath || latestHandoffPackage.packageDir} · ${latestHandoffPackage.bundlePath}</small>
          ${latestHandoffPackage.runPlanPath ? `<small>run plan · ${latestHandoffPackage.runPlanPath}</small>` : ""}
          ${latestHandoffPackage.runnerScriptPath ? `<small>runner · ${latestHandoffPackage.runnerScriptPath}</small>` : ""}
          ${latestHandoffPackage.runnerPreflightPath ? `<small>runner preflight · ${latestHandoffPackage.runnerPreflightPath}</small>` : ""}
          ${bridgeItem ? `<small>bridge item · ${bridgeItem.taskId} · ${bridgeItem.bridgeWorker.endpointEnv} · ${bridgeItem.bridgeWorker.cliTemplate}</small>` : ""}
          ${providerStartCommand ? `<small>provider start · ${providerStartCommand.family} · ${providerStartCommand.endpointEnv} · ${providerStartCommand.upstreamEnv} · ${providerStartCommand.cli}</small>` : ""}
          ${bridgeStartCommand ? `<small>bridge start · ${bridgeStartCommand.adapterId} · ${bridgeStartCommand.endpointEnv} · ${bridgeStartCommand.upstreamEnv} · ${bridgeStartCommand.cli}</small>` : ""}
        </div>
      `;
      return;
    }

    if (latestRunPlan) {
      const firstPending = latestRunPlan.phases.find((phase) => phase.status !== "ready") ?? latestRunPlan.phases[0];
      const bridgePhase = latestRunPlan.phases.find((phase) => phase.genericApiBridgeWorker?.cliTemplate);
      const bridgeStartCommand = latestRunPlan.phases.flatMap((phase) => phase.bridgeWorkerStartCommands)[0];
      const providerStartCommand = latestRunPlan.phases.flatMap((phase) => phase.providerGatewayWorkerStartCommands)[0];
      summaryEl.textContent = `${latestRunPlan.phaseCount} plan`;
      resultEl.innerHTML = `
        ${requirementsBlock}
        <div>
          <span>${latestRunPlan.status}</span>
          <strong>${latestRunPlan.summary.missingTotal} missing · ${latestRunPlan.phaseCount} execution phases</strong>
          <small>${latestRunPlan.outputRoot} · ${latestRunPlan.commands.closeout_preflight}</small>
          <small>${latestRunPlan.summary.providerTasks} provider / ${latestRunPlan.summary.softwareTasks} software / ${latestRunPlan.summary.desktopVisionTasks} vision tasks</small>
          ${providerStartCommand ? `<small>provider start · ${providerStartCommand.family} · ${providerStartCommand.endpointEnv} · ${providerStartCommand.upstreamEnv} · ${providerStartCommand.cli}</small>` : ""}
          ${bridgePhase ? `<small>software bridge · ${bridgePhase.genericApiBridgeWorker.endpointEnvTemplate} · ${bridgePhase.genericApiBridgeWorker.cliTemplate}</small>` : ""}
          ${bridgeStartCommand ? `<small>bridge start · ${bridgeStartCommand.adapterId} · ${bridgeStartCommand.endpointEnv} · ${bridgeStartCommand.upstreamEnv} · ${bridgeStartCommand.cli}</small>` : ""}
          ${firstPending ? `<small>${firstPending.id} · ${firstPending.command}</small>` : ""}
        </div>
      `;
      return;
    }

    if (latestMerge) {
      const total = latestMerge.summary.providers + latestMerge.summary.softwareActions + latestMerge.summary.desktopVision;
      summaryEl.textContent = `${total} merged`;
      resultEl.innerHTML = `
        ${requirementsBlock}
        <div>
          <span>merged</span>
          <strong>${latestMerge.summary.providers} providers / ${latestMerge.summary.softwareActions} software / ${latestMerge.summary.desktopVision} vision</strong>
          <small>${latestMerge.inputBundles} input bundles · merge writes ${latestMerge.writes} · ${latestMerge.commands.validate}</small>
        </div>
      `;
      return;
    }

    if (latestCloseout) {
      const total = latestCloseout.summary.providers + latestCloseout.summary.softwareActions + latestCloseout.summary.desktopVision;
      const coverageLine = latestCloseout.coverage
        ? `<small>coverage ${latestCloseout.coverage.providersCovered}/${latestCloseout.coverage.providersRequired} providers · ${latestCloseout.coverage.softwareCovered}/${latestCloseout.coverage.softwareRequired} software · ${latestCloseout.coverage.desktopComplete ? "vision ready" : "vision missing"}</small>`
        : "";
      const completionLine = latestCloseout.completionGate.status !== "unknown"
        ? `<small>PRD ${latestCloseout.readyForCompletion ? "complete" : "incomplete"} · ${latestCloseout.readyForCompletion ? latestCloseout.commands.completionPackage : latestCloseout.commands.completionGate}</small>`
        : "";
      summaryEl.textContent = `${total} closeout`;
      resultEl.innerHTML = `
        ${requirementsBlock}
        <div>
          <span>${latestCloseout.readyForImport ? "ready" : "not ready"}</span>
          <strong>${latestCloseout.summary.providers} providers / ${latestCloseout.summary.softwareActions} software / ${latestCloseout.summary.desktopVision} vision</strong>
          <small>${latestCloseout.inputBundles} input bundles · closeout writes ${latestCloseout.writes} · ${latestCloseout.commands.import}</small>
          ${coverageLine}
          ${completionLine}
        </div>
      `;
      return;
    }

    summaryEl.textContent = runtimeReady ? "ready" : "offline";
    resultEl.innerHTML = `
      ${requirementsBlock}
      <div>
        <span>${runtimeReady ? "waiting" : "offline"}</span>
        <strong>${runtimeReady ? "等待生产证据 bundle" : "连接 Runtime 后可导入"}</strong>
        <small>CLI 等价命令：${latestRequirements?.commands?.template ?? "pool-cli production-evidence-template target/production-evidence/bundle.json"}</small>
      </div>
    `;
    return;
  }

  const latest = latestImport ?? latestValidation;
  const total = latest.summary.providers + latest.summary.softwareActions + latest.summary.desktopVision;
  const mode = latestImport ? "imported" : "valid";
  const coverageLine = latest.coverage
    ? `<small>coverage ${latest.coverage.providersCovered}/${latest.coverage.providersRequired} providers · ${latest.coverage.softwareCovered}/${latest.coverage.softwareRequired} software · ${latest.coverage.desktopComplete ? "vision ready" : "vision missing"}</small>`
    : "";
  const closeoutCompletionLine = latestImport && latestCloseout?.mode === "import"
    ? `<small>PRD ${latestCloseout.readyForCompletion ? "complete" : "incomplete"} · ${latestCloseout.readyForCompletion ? latestCloseout.commands.completionPackage : latestCloseout.commands.completionGate}</small>`
    : "";
  summaryEl.textContent = `${total} ${mode}`;
  resultEl.innerHTML = `
    ${requirementsBlock}
    <div>
      <span>${latestImport ? latest.overallStatus : latest.status}</span>
      <strong>${latest.summary.providers} providers / ${latest.summary.softwareActions} software / ${latest.summary.desktopVision} vision</strong>
      <small>${latest.source} · ${latestImport ? latest.importedAt || "imported" : `validate writes ${latest.writes}`}</small>
      ${coverageLine}
      ${closeoutCompletionLine}
    </div>
  `;
}

function renderProductionEvidenceRequirementsBlock(requirements, tasks, handoff, runtimeReady) {
  if (!runtimeReady) return "";
  if (!requirements) {
    return `
      <div>
        <span>doctor</span>
        <strong>等待 Runtime 生产证据需求清单</strong>
        <small>读取 /api/production-evidence/requirements 后显示 Provider、软件和外部视觉缺口。</small>
      </div>
    `;
  }
  const providerLine = requirements.providerMissing.length
    ? `${requirements.providerMissing.length} provider upstream`
    : "provider upstream ready";
  const softwareLine = requirements.softwareMissing.length
    ? `${requirements.softwareMissing.length} software evidence`
    : "software evidence ready";
  const desktopLine = requirements.desktopMissing.length
    ? `${requirements.desktopMissing.join(", ")}`
    : "external vision ready";
  const taskList = tasks?.tasks?.length ? tasks.tasks : requirements.evidenceTasks;
  const taskRows = taskList.slice(0, 4).map((task) => {
    const bridgeLine = task.bridgeWorker?.available
      ? `<small>bridge worker · ${task.bridgeWorker.endpointEnvTemplate || task.bridgeWorker.endpointEnv} · ${task.bridgeWorker.cliTemplate}</small>`
      : task.preferredControlProfile
        ? `<small>control ${task.preferredControlProfile}</small>`
        : "";
    return `
      <div>
        <span>${task.kind}</span>
        <strong>${task.targetId || task.title}</strong>
        <small>${task.bundlePath || "item"} · ${task.commands.submit_item ?? task.commands.item_template ?? requirements.commands.validate ?? "production-evidence-item-template"}</small>
        ${bridgeLine}
      </div>
    `;
  }).join("");
  const handoffProviderStart = handoff?.providerGatewayWorkerStartCommands?.[0];
  const handoffProviderLine = handoffProviderStart
    ? `<small>provider handoff · ${handoffProviderStart.family} · ${handoffProviderStart.endpointEnv} · ${handoffProviderStart.upstreamEnv} · ${handoffProviderStart.cli}</small>`
    : "";
  const handoffBridgeStart = handoff?.bridgeWorkerStartCommands?.[0];
  const handoffBridgeLine = handoffBridgeStart
    ? `<small>bridge handoff · ${handoffBridgeStart.adapterId} · ${handoffBridgeStart.endpointEnv} · ${handoffBridgeStart.upstreamEnv} · ${handoffBridgeStart.cli}</small>`
    : "";
  const handoffRow = handoff ? `
    <div>
      <span>handoff</span>
      <strong>生产证据交付包 ${handoff.evidenceTasks} tasks</strong>
      <small>${handoff.bundleSummary.providers} providers / ${handoff.bundleSummary.softwareActions} software / ${handoff.bundleSummary.desktopVision} vision · ${handoff.commands.merge} -> ${handoff.commands.validate}</small>
      ${handoffProviderLine}
      ${handoffBridgeLine}
    </div>
  ` : "";
  return `
    <div>
      <span>${requirements.overallStatus}</span>
      <strong>生产证据缺口 ${requirements.missingTotal}</strong>
      <small>${providerLine} · ${softwareLine} · ${desktopLine}${tasks ? ` · task queue ${tasks.summary.total}` : ""}</small>
    </div>
    ${handoffRow}
    ${taskRows}
  `;
}

function normalizeProductionEvidenceValidation(result) {
  const summary = result?.summary ?? {};
  const coverage = result?.coverage ?? {};
  const providerCoverage = coverage.providers ?? {};
  const softwareCoverage = coverage.software_actions ?? {};
  const desktopCoverage = coverage.desktop_vision ?? {};
  return {
    source: result?.source ?? "production_evidence_bundle",
    status: result?.valid ? "valid" : "invalid",
    writes: numberValue(result?.writes),
    coverage: {
      complete: Boolean(coverage.complete),
      wouldSatisfyPrd: Boolean(coverage.would_satisfy_prd_production_evidence),
      providersCovered: numberValue(providerCoverage.covered),
      providersRequired: Array.isArray(providerCoverage.required) ? providerCoverage.required.length : 0,
      missingProviders: (providerCoverage.missing ?? []).map(String),
      softwareCovered: numberValue(softwareCoverage.covered),
      softwareRequired: Array.isArray(softwareCoverage.required) ? softwareCoverage.required.length : 0,
      missingSoftware: (softwareCoverage.missing ?? []).map(String),
      desktopComplete: Boolean(desktopCoverage.complete),
      missingDesktopVision: (desktopCoverage.missing ?? []).map(String),
    },
    summary: {
      providers: numberValue(summary.providers),
      softwareActions: numberValue(summary.software_actions ?? summary.softwareActions),
      desktopVision: numberValue(summary.desktop_vision ?? summary.desktopVision),
    },
  };
}

function normalizeProductionEvidenceTemplate(result) {
  const bundle = result?.bundle ?? {};
  const providers = Array.isArray(bundle.providers) ? bundle.providers.length : 0;
  const softwareActions = Array.isArray(bundle.software_actions) ? bundle.software_actions.length : 0;
  const desktopVision = Array.isArray(bundle.desktop_vision) ? bundle.desktop_vision.length : 0;
  return {
    source: bundle.source ?? result?.source ?? "production_evidence_template",
    readyForImport: Boolean(result?.ready_for_import),
    commands: {
      validate: result?.commands?.validate ?? "pool-cli validate-production-evidence <bundle.json>",
      import: result?.commands?.import ?? "pool-cli import-production-evidence <bundle.json>",
    },
    summary: {
      providers,
      softwareActions,
      desktopVision,
    },
  };
}

function normalizeProductionEvidenceItemTemplate(result) {
  const selector = result?.selector ?? {};
  return {
    readyForImport: Boolean(result?.ready_for_import),
    selector: {
      taskId: selector.task_id ?? "",
      kind: selector.kind ?? result?.item?.kind ?? "provider",
      targetId: selector.target_id ?? "",
    },
    commands: {
      submit: result?.commands?.submit ?? "pool-cli submit-production-evidence-item <item.json>",
      tasks: result?.commands?.tasks ?? "pool-cli production-evidence-tasks",
    },
  };
}

function normalizeProductionEvidenceTaskClaim(result) {
  const claim = result?.claim ?? {};
  const selector = result?.selector ?? claim.selector ?? {};
  const runtimeTask = result?.runtime_task ?? {};
  const commands = result?.commands ?? claim.commands ?? {};
  return {
    taskId: result?.task_id ?? claim.task_id ?? "",
    runtimeTaskId: result?.runtime_task_id ?? claim.runtime_task_id ?? runtimeTask.id ?? "",
    status: runtimeTask.status ?? "Running",
    claimPath: result?.claim_path ?? "",
    assignee: claim.assignee ?? "",
    role: claim.role ?? "",
    selector: {
      kind: selector.kind ?? "production_evidence",
      targetId: selector.target_id ?? selector.targetId ?? "",
    },
    commands: {
      validateItem: commands.validate_item ?? "pool-cli validate-production-evidence-item <item.json>",
      submitItem: commands.submit_item ?? "pool-cli submit-production-evidence-item <item.json>",
    },
  };
}

function normalizeProductionEvidenceItemValidation(result) {
  const validation = normalizeProductionEvidenceValidation(result?.validation ?? result);
  const commands = result?.commands ?? {};
  return {
    source: result?.source ?? validation.source,
    valid: Boolean(result?.valid ?? validation.status === "valid"),
    writes: numberValue(result?.writes),
    validation,
    commands: {
      submit: commands.submit ?? "pool-cli submit-production-evidence-item <item.json>",
      validateBundle: commands.validate_bundle ?? "pool-cli validate-production-evidence <bundle.json>",
      readiness: commands.readiness ?? "pool-cli prd-readiness",
    },
  };
}

function normalizeProductionEvidenceLedgerBundle(result) {
  const summary = result?.summary ?? {};
  return {
    source: result?.source ?? "runtime-ledger",
    readyForImport: Boolean(result?.ready_for_import),
    readyItems: numberValue(summary.ready_items),
    incompleteItems: numberValue(summary.incomplete_items),
    ledgerCandidates: numberValue(summary.ledger_candidates),
    commands: {
      validate: result?.commands?.validate ?? "pool-cli validate-production-evidence <bundle.json>",
      import: result?.commands?.import ?? "pool-cli import-production-evidence <bundle.json>",
      closeout: result?.commands?.closeout ?? "pool-cli closeout-production-evidence --import <bundle.json>",
    },
    summary: {
      providers: numberValue(summary.providers),
      softwareActions: numberValue(summary.software_actions ?? summary.softwareActions),
      desktopVision: numberValue(summary.desktop_vision ?? summary.desktopVision),
    },
  };
}

function normalizeProductionEvidenceMerge(result) {
  const summary = result?.summary ?? {};
  return {
    source: result?.source ?? "production_evidence_merge",
    writes: numberValue(result?.writes),
    inputBundles: numberValue(summary.input_bundles),
    commands: {
      validate: result?.commands?.validate ?? "pool-cli validate-production-evidence <merged-bundle.json>",
      import: result?.commands?.import ?? "pool-cli import-production-evidence <merged-bundle.json>",
    },
    summary: {
      providers: numberValue(summary.providers),
      softwareActions: numberValue(summary.software_actions ?? summary.softwareActions),
      desktopVision: numberValue(summary.desktop_vision ?? summary.desktopVision),
    },
  };
}

function normalizeProductionEvidenceCloseout(result) {
  const validation = normalizeProductionEvidenceValidation(result?.validation ?? {});
  const merge = normalizeProductionEvidenceMerge(result?.merge ?? {});
  const gate = result?.completion_gate ?? result?.completionGate ?? result?.import?.prd_readiness?.completion_gate ?? {};
  const prdSummary = result?.prd_summary ?? result?.prdSummary ?? result?.import?.prd_readiness?.summary ?? {};
  return {
    source: result?.source ?? validation.source ?? "production_evidence_closeout",
    mode: result?.mode ?? "validate",
    writes: numberValue(result?.writes),
    readyForImport: Boolean(result?.ready_for_import),
    readyForCompletion: Boolean(result?.ready_for_completion ?? gate.ready_for_completion ?? gate.readyForCompletion),
    prdOverallStatus: result?.prd_overall_status ?? result?.prdOverallStatus ?? result?.import?.prd_readiness?.overall_status ?? "unknown",
    prdReady: numberValue(prdSummary.ready),
    inputBundles: merge.inputBundles,
    commands: {
      validate: result?.commands?.validate ?? merge.commands.validate,
      import: result?.commands?.import ?? merge.commands.import,
      readiness: result?.commands?.readiness ?? "pool-cli prd-readiness",
      completionGate: result?.commands?.completion_gate ?? result?.commands?.completionGate ?? "pool-cli prd-completion-gate --require-complete",
      completionPackage: result?.commands?.completion_package ?? result?.commands?.completionPackage ?? "pool-cli prd-completion-package --include-snapshot",
    },
    completionGate: {
      status: gate.status ?? (result?.ready_for_completion ? "complete" : "unknown"),
      readyForCompletion: Boolean(gate.ready_for_completion ?? gate.readyForCompletion ?? result?.ready_for_completion),
      incompleteRequirements: (gate.incomplete_requirements ?? gate.incompleteRequirements ?? []).map(String),
    },
    coverage: validation.coverage,
    summary: validation.summary,
  };
}

function normalizeProductionEvidenceHandoffPackage(result) {
  const report = result?.report ?? {};
  const summary = report.summary ?? {};
  const items = Array.isArray(report.items) ? report.items.map((item) => {
    const bridgeWorker = item.bridge_worker ?? item.bridgeWorker ?? null;
    return {
      taskId: item.task_id ?? "",
      kind: item.kind ?? "",
      targetId: item.target_id ?? "",
      bundlePath: item.bundle_path ?? "",
      preferredControlProfile: item.preferred_control_profile ?? "",
      itemPath: item.item_path ?? "",
      bridgeWorker: bridgeWorker ? {
        available: Boolean(bridgeWorker.available),
        endpointEnv: bridgeWorker.endpoint_env ?? "",
        cliTemplate: bridgeWorker.cli_template ?? "",
      } : null,
    };
  }) : [];
  const bridgeWorkerStartCommands = (report.software_bridge_worker_start_commands ?? report.softwareBridgeWorkerStartCommands ?? []).map((command) => ({
    adapterId: command.adapter_id ?? command.adapterId ?? "",
    endpointEnv: command.endpoint_env ?? command.endpointEnv ?? "",
    endpointAssignment: command.endpoint_assignment ?? command.endpointAssignment ?? "",
    upstreamEnv: command.upstream_env ?? command.upstreamEnv ?? "",
    cli: command.cli ?? "",
    productionRule: command.production_rule ?? command.productionRule ?? "",
  }));
  const providerGatewayWorkerStartCommands = (report.provider_gateway_worker_start_commands ?? report.providerGatewayWorkerStartCommands ?? []).map((command) => ({
    family: command.family ?? "",
    endpointEnv: command.endpoint_env ?? command.endpointEnv ?? "",
    endpointAssignment: command.endpoint_assignment ?? command.endpointAssignment ?? "",
    upstreamEnv: command.upstream_env ?? command.upstreamEnv ?? "",
    cli: command.cli ?? "",
    productionRule: command.production_rule ?? command.productionRule ?? "",
  }));
  return {
    status: report.status ?? "unknown",
    packageDir: report.package_dir ?? "",
    manifestPath: report.manifest_path ?? "",
    runPlanPath: report.run_plan_path ?? "",
    runnerScriptPath: report.runner_script_path ?? "",
    runnerPreflightPath: report.runner_preflight_path ?? "",
    bundlePath: report.bundle_path ?? "",
    tasksPath: report.tasks_path ?? "",
    itemCount: numberValue(report.item_count ?? summary.item_templates),
    localFiles: Array.isArray(report.local_paths)
      ? report.local_paths.length
      : Array.isArray(report.local_files)
        ? report.local_files.length
      : numberValue(report.local_files ?? summary.local_files),
    items,
    providerGatewayWorkerStartCommands,
    bridgeWorkerStartCommands,
    taskId: result?.task?.id ?? "",
  };
}

function mergeProductionEvidenceHandoffPackages(catalog) {
  if (!Array.isArray(catalog?.packages)) return false;
  if (!catalog.packages.length) {
    state.productionEvidenceHandoffPackage = null;
    return false;
  }
  state.productionEvidenceHandoffPackage = normalizeProductionEvidenceHandoffPackage({
    kind: "pool_production_evidence_handoff_package",
    report: catalog.packages[0],
  });
  return true;
}

function normalizeProductionEvidenceImport(result) {
  const summary = result?.summary ?? {};
  const coverage = result?.coverage ?? {};
  const providerCoverage = coverage.providers ?? {};
  const softwareCoverage = coverage.software_actions ?? {};
  const desktopCoverage = coverage.desktop_vision ?? {};
  const readiness = normalizeRuntimePrdReadiness(result?.prd_readiness);
  return {
    source: result?.source ?? "production_evidence_bundle",
    importedAt: result?.imported_at ?? "",
    overallStatus: readiness?.overallStatus ?? result?.prd_readiness?.overall_status ?? "unknown",
    coverage: {
      complete: Boolean(coverage.complete),
      wouldSatisfyPrd: Boolean(coverage.would_satisfy_prd_production_evidence),
      providersCovered: numberValue(providerCoverage.covered),
      providersRequired: Array.isArray(providerCoverage.required) ? providerCoverage.required.length : 0,
      missingProviders: (providerCoverage.missing ?? []).map(String),
      softwareCovered: numberValue(softwareCoverage.covered),
      softwareRequired: Array.isArray(softwareCoverage.required) ? softwareCoverage.required.length : 0,
      missingSoftware: (softwareCoverage.missing ?? []).map(String),
      desktopComplete: Boolean(desktopCoverage.complete),
      missingDesktopVision: (desktopCoverage.missing ?? []).map(String),
    },
    summary: {
      providers: numberValue(summary.providers),
      softwareActions: numberValue(summary.software_actions ?? summary.softwareActions),
      desktopVision: numberValue(summary.desktop_vision ?? summary.desktopVision),
    },
    prdReadiness: readiness,
  };
}

function productionEvidenceExampleBundle() {
  const projectSlug = activeProjectSlug();
  return {
    project_slug: projectSlug,
    source: "web-production-evidence-example",
    providers: [
      {
        provider_id: "midjourney",
        external_job_id: "replace-with-real-midjourney-job-001",
        endpoint: "https://worker.example.test/midjourney",
        family: "ai_media",
        artifacts: [`worlds/${projectSlug}/output/production/midjourney/1-midjourney.png`],
      },
      {
        provider_id: "openai-image-2",
        external_job_id: "replace-with-real-openai-image-2-job-001",
        endpoint: "https://api.openai.com/v1/images/generations",
        family: "ai_image",
        artifacts: [`worlds/${projectSlug}/output/production/openai-image-2/1-openai-image.png`],
      },
      {
        provider_id: "nano-banana-pro",
        external_job_id: "replace-with-real-nano-banana-pro-job-001",
        endpoint: "https://worker.example.test/nano-banana-pro",
        family: "ai_media",
        artifacts: [`worlds/${projectSlug}/output/production/nano-banana-pro/1-nano.png`],
      },
      {
        provider_id: "suno",
        external_job_id: "replace-with-real-suno-job-001",
        endpoint: "https://worker.example.test/suno",
        family: "ai_media",
        artifacts: [`worlds/${projectSlug}/output/production/suno/1-cue.mp3`],
      },
      {
        provider_id: "worldlabs-marble",
        external_job_id: "replace-with-real-worldlabs-marble-job-001",
        endpoint: "https://worker.example.test/worldlabs-marble",
        family: "3dgs",
        artifacts: [`worlds/${projectSlug}/output/production/worldlabs-marble/1-world.glb`],
      },
      {
        provider_id: "tripo-splat",
        external_job_id: "replace-with-real-tripo-splat-job-001",
        endpoint: "https://worker.example.test/tripo-splat",
        family: "3dgs",
        artifacts: [`worlds/${projectSlug}/output/production/tripo-splat/1-object.glb`],
      },
      {
        provider_id: "sam-3d",
        external_job_id: "replace-with-real-sam-3d-job-001",
        endpoint: "https://worker.example.test/sam-3d",
        family: "3dgs",
        artifacts: [`worlds/${projectSlug}/output/production/sam-3d/1-mask-object.glb`],
      },
      {
        provider_id: "spark-3dgs",
        external_job_id: "replace-with-real-spark-3dgs-job-001",
        endpoint: "https://worker.example.test/spark-3dgs",
        family: "3dgs",
        artifacts: [`worlds/${projectSlug}/output/production/spark-3dgs/1-scene.glb`],
      },
      {
        provider_id: "qunhe-3d",
        external_job_id: "replace-with-real-qunhe-3d-job-001",
        endpoint: "https://worker.example.test/qunhe-3d",
        family: "3dgs",
        artifacts: [`worlds/${projectSlug}/output/production/qunhe-3d/1-layout.glb`],
      },
    ],
    software_actions: [
      {
        adapter_id: "unreal",
        external_action_id: "replace-with-real-unreal-action-001",
        action_kind: "CreateScene",
        priority: "ApiMcp",
        control_profile: "api_mcp",
        artifacts: [`unreal://project/${projectSlug}/level/production`],
      },
      {
        adapter_id: "blender",
        external_action_id: "replace-with-real-blender-action-001",
        action_kind: "ExecuteCli",
        priority: "SkillsCli",
        control_profile: "skills_cli",
        artifacts: [`worlds/${projectSlug}/output/production/blender/1-cleanup.blend`],
      },
      {
        adapter_id: "comfyui",
        external_action_id: "replace-with-real-comfyui-action-001",
        action_kind: "ExecuteCli",
        priority: "SkillsCli",
        control_profile: "skills_cli",
        artifacts: [`worlds/${projectSlug}/output/production/comfyui/1-image.png`],
      },
      {
        adapter_id: "resolve",
        external_action_id: "replace-with-real-resolve-action-001",
        action_kind: "Transcode",
        priority: "SkillsCli",
        control_profile: "skills_cli",
        artifacts: [`worlds/${projectSlug}/output/production/resolve/1-master.mov`],
      },
      {
        adapter_id: "unity",
        external_action_id: "replace-with-real-unity-action-001",
        action_kind: "ExportBuild",
        priority: "ApiMcp",
        control_profile: "api_mcp",
        artifacts: [`unity://project/${projectSlug}/build/production`],
      },
      {
        adapter_id: "touchdesigner",
        external_action_id: "replace-with-real-touchdesigner-action-001",
        action_kind: "RunViewport",
        priority: "DesktopRecognition",
        control_profile: "desktop_recognition",
        artifacts: [`touchdesigner://project/${projectSlug}/perform`],
      },
      {
        adapter_id: "madmapper",
        external_action_id: "replace-with-real-madmapper-action-001",
        action_kind: "RunViewport",
        priority: "DesktopRecognition",
        control_profile: "desktop_recognition",
        artifacts: [`madmapper://project/${projectSlug}/cues`],
      },
      {
        adapter_id: "nuke",
        external_action_id: "replace-with-real-nuke-action-001",
        action_kind: "Render",
        priority: "SkillsCli",
        control_profile: "skills_cli",
        artifacts: [`worlds/${projectSlug}/output/production/nuke/1-comp.exr`],
      },
      {
        adapter_id: "motion-db",
        external_action_id: "replace-with-real-motion-db-action-001",
        action_kind: "ImportAsset",
        priority: "SkillsCli",
        control_profile: "skills_cli",
        artifacts: [`worlds/${projectSlug}/output/production/motion-db/1-take.fbx`],
      },
      {
        adapter_id: "editing-suite",
        external_action_id: "replace-with-real-editing-suite-action-001",
        action_kind: "Transcode",
        priority: "SkillsCli",
        control_profile: "skills_cli",
        artifacts: [`worlds/${projectSlug}/output/production/editing-suite/1-delivery.mp4`],
      },
      {
        adapter_id: "hermes",
        external_action_id: "replace-with-real-hermes-action-001",
        action_kind: "CreateScene",
        priority: "ApiMcp",
        control_profile: "api_mcp",
        artifacts: [`pool://agent-sessions/replace-with-real-hermes-action-001`],
      },
    ],
    desktop_vision: [
      {
        adapter_id: "touchdesigner",
        external_action_id: "replace-with-real-vision-action-001",
        controller_id: "external-vision-controller",
        trace_path: `worlds/${projectSlug}/output/production/vision-trace.json`,
        visual_model: "external",
        artifacts: [`worlds/${projectSlug}/output/production/vision-trace.json`],
      },
    ],
  };
}

function defaultProductionEvidenceBundleText() {
  return JSON.stringify(productionEvidenceExampleBundle(), null, 2);
}

function fillProductionEvidenceExample() {
  const inputEl = document.querySelector("#productionEvidenceBundle");
  if (!inputEl) return;
  inputEl.value = defaultProductionEvidenceBundleText();
  state.productionEvidenceTaskClaim = null;
  state.productionEvidenceItemValidation = null;
  state.productionEvidenceLedgerBundle = null;
  state.productionEvidenceRunPlan = null;
  state.productionEvidenceMerge = null;
  state.productionEvidenceCloseout = null;
  addEvent("info", "已填入完整生产证据 bundle 模板，请替换为真实外部运行证据后再导入。");
  renderEvents();
}

async function loadProductionEvidenceTemplate() {
  if (state.snapshot?.mode !== "runtime-http") {
    addEvent("warn", "需要先连接 Runtime HTTP 才能生成生产证据脚手架。");
    renderEvents();
    return;
  }

  const inputEl = document.querySelector("#productionEvidenceBundle");
  const runtime = state.snapshot.runtime ?? runtimeBaseUrl();
  const missingOnly = Boolean(state.productionEvidenceRequirements?.missingTotal);
  try {
    const result = await fetchJson(runtimeProductionEvidenceTemplateUrl(runtime, { missingOnly }));
    const bundle = result?.bundle;
    if (!bundle || typeof bundle !== "object" || Array.isArray(bundle)) {
      throw new Error("Runtime template response missing bundle");
    }
    if (inputEl) inputEl.value = JSON.stringify(bundle, null, 2);
    state.productionEvidenceTemplate = normalizeProductionEvidenceTemplate(result);
    state.productionEvidenceItemTemplate = null;
    state.productionEvidenceTaskClaim = null;
    state.productionEvidenceItemValidation = null;
    state.productionEvidenceLedgerBundle = null;
    state.productionEvidenceRunPlan = null;
    state.productionEvidenceMerge = null;
    state.productionEvidenceCloseout = null;
    state.productionEvidenceValidation = null;
    state.productionEvidenceImport = null;
    addEvent(
      "info",
      `已从 Runtime 生成生产证据脚手架：${state.productionEvidenceTemplate.summary.providers} providers / ${state.productionEvidenceTemplate.summary.softwareActions} software。`,
    );
    saveState();
    renderAll();
  } catch (error) {
    addEvent("warn", `生产证据脚手架生成失败：${error.message}`);
    renderEvents();
  }
}

function selectedProductionEvidenceTask() {
  const taskSelect = document.querySelector("#productionEvidenceTaskSelect");
  const taskId = taskSelect?.value || state.productionEvidenceTasks?.tasks?.[0]?.id || state.productionEvidenceRequirements?.evidenceTasks?.[0]?.id;
  const tasks = state.productionEvidenceTasks?.tasks?.length
    ? state.productionEvidenceTasks.tasks
    : state.productionEvidenceRequirements?.evidenceTasks ?? [];
  return tasks.find((task) => task.id === taskId) ?? (taskId ? { id: taskId, kind: "production_evidence", targetId: "" } : null);
}

function productionEvidenceClaimRoleForTask(task) {
  const kind = task?.kind ?? "";
  if (kind === "provider" || kind.startsWith("provider_")) return "provider_worker";
  if (kind === "software" || kind.startsWith("software_")) return "software_operator";
  if (task?.kind === "desktop_vision") return "desktop_vision_controller";
  return "production_evidence_operator";
}

async function loadProductionEvidenceItemTemplate() {
  if (state.snapshot?.mode !== "runtime-http") {
    addEvent("warn", "需要先连接 Runtime HTTP 才能生成单项生产证据。");
    renderEvents();
    return;
  }

  const inputEl = document.querySelector("#productionEvidenceBundle");
  const task = selectedProductionEvidenceTask();
  if (!task?.id) {
    addEvent("warn", "没有可生成的生产证据任务。");
    renderEvents();
    return;
  }

  const runtime = state.snapshot.runtime ?? runtimeBaseUrl();
  try {
    const result = await fetchJson(runtimeProductionEvidenceItemTemplateUrl(runtime, { taskId: task.id }));
    const item = result?.item;
    if (!item || typeof item !== "object" || Array.isArray(item)) {
      throw new Error("Runtime item template response missing item");
    }
    if (inputEl) inputEl.value = JSON.stringify(item, null, 2);
    state.productionEvidenceItemTemplate = normalizeProductionEvidenceItemTemplate(result);
    state.productionEvidenceTemplate = null;
    state.productionEvidenceTaskClaim = null;
    state.productionEvidenceItemValidation = null;
    state.productionEvidenceLedgerBundle = null;
    state.productionEvidenceRunPlan = null;
    state.productionEvidenceMerge = null;
    state.productionEvidenceCloseout = null;
    state.productionEvidenceValidation = null;
    state.productionEvidenceImport = null;
    addEvent(
      "info",
      `已从 Runtime 生成单项生产证据：${state.productionEvidenceItemTemplate.selector.kind} / ${state.productionEvidenceItemTemplate.selector.targetId}。`,
    );
    saveState();
    renderAll();
  } catch (error) {
    addEvent("warn", `单项生产证据生成失败：${error.message}`);
    renderEvents();
  }
}

async function claimProductionEvidenceTask() {
  if (state.snapshot?.mode !== "runtime-http") {
    addEvent("warn", "需要先连接 Runtime HTTP 才能领取生产证据任务。");
    renderEvents();
    return;
  }

  const task = selectedProductionEvidenceTask();
  if (!task?.id) {
    addEvent("warn", "没有可领取的生产证据任务。");
    renderEvents();
    return;
  }

  const runtime = state.snapshot.runtime ?? runtimeBaseUrl();
  const projectSlug = activeProjectSlug();
  try {
    const result = await fetchJson(runtimeProductionEvidenceTaskClaimUrl(runtime), {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        project_slug: projectSlug,
        task_id: task.id,
        assignee: "web-operator",
        role: productionEvidenceClaimRoleForTask(task),
        output_root: `worlds/${projectSlug}/output/production-evidence`,
        source: "web-production-evidence-task-claim",
      }),
    });
    state.productionEvidenceTaskClaim = normalizeProductionEvidenceTaskClaim(result);
    state.productionEvidenceItemValidation = null;
    state.productionEvidenceTemplate = null;
    state.productionEvidenceLedgerBundle = null;
    state.productionEvidenceHandoffPackage = null;
    state.productionEvidenceRunPlan = null;
    state.productionEvidenceMerge = null;
    state.productionEvidenceCloseout = null;
    state.productionEvidenceValidation = null;
    state.productionEvidenceImport = null;
    await mergeRuntimeMutationSnapshot(result.snapshot, runtime);
    addEvent(
      "ok",
      `生产证据任务已领取：${state.productionEvidenceTaskClaim.selector.kind} / ${state.productionEvidenceTaskClaim.selector.targetId}。`,
    );
    saveState();
    renderAll();
  } catch (error) {
    addEvent("warn", `生产证据任务领取失败：${error.message}`);
    renderEvents();
  }
}

async function loadProductionEvidenceLedgerBundle() {
  if (state.snapshot?.mode !== "runtime-http") {
    addEvent("warn", "需要先连接 Runtime HTTP 才能从账本收口生产证据。");
    renderEvents();
    return;
  }

  const inputEl = document.querySelector("#productionEvidenceBundle");
  const runtime = state.snapshot.runtime ?? runtimeBaseUrl();
  try {
    const result = await fetchJson(runtimeProductionEvidenceBundleFromLedgerUrl(runtime, {
      source: "web-production-evidence-ledger",
      includeIncomplete: true,
    }));
    const bundle = result?.bundle;
    if (!bundle || typeof bundle !== "object" || Array.isArray(bundle)) {
      throw new Error("Runtime ledger bundle response missing bundle");
    }
    if (inputEl) inputEl.value = JSON.stringify(bundle, null, 2);
    state.productionEvidenceLedgerBundle = normalizeProductionEvidenceLedgerBundle(result);
    state.productionEvidenceTemplate = null;
    state.productionEvidenceItemTemplate = null;
    state.productionEvidenceTaskClaim = null;
    state.productionEvidenceItemValidation = null;
    state.productionEvidenceRunPlan = null;
    state.productionEvidenceMerge = null;
    state.productionEvidenceCloseout = null;
    state.productionEvidenceValidation = null;
    state.productionEvidenceImport = null;
    addEvent(
      "ok",
      `已从 Runtime 账本收口生产证据：${state.productionEvidenceLedgerBundle.readyItems} ready / ${state.productionEvidenceLedgerBundle.incompleteItems} incomplete。`,
    );
    saveState();
    renderAll();
  } catch (error) {
    addEvent("warn", `生产证据账本收口失败：${error.message}`);
    renderEvents();
  }
}

async function createProductionEvidenceHandoffPackage() {
  if (state.snapshot?.mode !== "runtime-http") {
    addEvent("warn", "需要先连接 Runtime HTTP 才能写出生产证据交付包。");
    renderEvents();
    return;
  }

  const runtime = state.snapshot.runtime ?? runtimeBaseUrl();
  const projectSlug = activeProjectSlug();
  try {
    const result = await fetchJson(runtimeProductionEvidenceHandoffPackageUrl(runtime), {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        project_slug: projectSlug,
        node_id: "agent",
        title: "Production evidence handoff package",
        output_dir: `worlds/${projectSlug}/output`,
        output_root: `worlds/${projectSlug}/output/production-evidence`,
        source: "web-production-evidence-handoff-package",
        include_items: true,
        include_snapshot: true,
      }),
    });
    state.productionEvidenceHandoffPackage = normalizeProductionEvidenceHandoffPackage(result);
    state.productionEvidenceTemplate = null;
    state.productionEvidenceItemTemplate = null;
    state.productionEvidenceTaskClaim = null;
    state.productionEvidenceItemValidation = null;
    state.productionEvidenceLedgerBundle = null;
    state.productionEvidenceRunPlan = null;
    state.productionEvidenceMerge = null;
    state.productionEvidenceCloseout = null;
    state.productionEvidenceValidation = null;
    state.productionEvidenceImport = null;
    await mergeRuntimeMutationSnapshot(result.snapshot, runtime);
    addEvent(
      "ok",
      `生产证据交付包已写出：${state.productionEvidenceHandoffPackage.itemCount} item files。`,
    );
    saveState();
    renderAll();
  } catch (error) {
    addEvent("warn", `生产证据交付包写出失败：${error.message}`);
    renderEvents();
  }
}

async function createProductionEvidenceRunPlan() {
  if (state.snapshot?.mode !== "runtime-http") {
    addEvent("warn", "需要先连接 Runtime HTTP 才能生成生产证据运行计划。");
    renderEvents();
    return;
  }

  const runtime = state.snapshot.runtime ?? runtimeBaseUrl();
  const projectSlug = activeProjectSlug();
  try {
    const result = await fetchJson(runtimeProductionEvidenceRunPlanUrl(runtime, {
      outputRoot: `worlds/${projectSlug}/output/production-evidence`,
      source: "web-production-evidence-run-plan",
    }));
    state.productionEvidenceRunPlan = normalizeProductionEvidenceRunPlan(result);
    state.productionEvidenceTemplate = null;
    state.productionEvidenceItemTemplate = null;
    state.productionEvidenceTaskClaim = null;
    state.productionEvidenceItemValidation = null;
    state.productionEvidenceLedgerBundle = null;
    state.productionEvidenceHandoffPackage = null;
    state.productionEvidenceMerge = null;
    state.productionEvidenceCloseout = null;
    state.productionEvidenceValidation = null;
    state.productionEvidenceImport = null;
    addEvent(
      "info",
      `生产证据运行计划已生成：${state.productionEvidenceRunPlan.phaseCount} phases / ${state.productionEvidenceRunPlan.summary.missingTotal} missing。`,
    );
    saveState();
    renderAll();
  } catch (error) {
    addEvent("warn", `生产证据运行计划生成失败：${error.message}`);
    renderEvents();
  }
}

async function createPrdCompletionPackage() {
  if (state.snapshot?.mode !== "runtime-http") {
    addEvent("warn", "需要先连接 Runtime HTTP 才能写出 PRD 完成证明包。");
    renderEvents();
    return;
  }

  const runtime = state.snapshot.runtime ?? runtimeBaseUrl();
  const projectSlug = activeProjectSlug();
  try {
    const result = await fetchJson(runtimePrdCompletionPackageUrl(runtime), {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        project_slug: projectSlug,
        node_id: "agent",
        title: "PRD completion proof package",
        output_dir: `worlds/${projectSlug}/output`,
        source: "web-prd-completion-package",
        include_snapshot: true,
      }),
    });
    state.runtimePrdCompletionPackage = normalizeRuntimePrdCompletionPackage(result);
    await mergeRuntimeMutationSnapshot(result.snapshot, runtime);
    addEvent(
      "ok",
      `PRD 完成证明包已写出：${state.runtimePrdCompletionPackage.readyForCompletion ? "ready" : "incomplete"} / ${state.runtimePrdCompletionPackage.localFiles} files。`,
    );
    saveState();
    renderAll();
  } catch (error) {
    addEvent("warn", `PRD 完成证明包写出失败：${error.message}`);
    renderEvents();
  }
}

async function createCoreArchitecturePackage() {
  if (state.snapshot?.mode !== "runtime-http") {
    addEvent("warn", "需要先连接 Runtime HTTP 才能写出核心架构证明包。");
    renderEvents();
    return;
  }

  const runtime = state.snapshot.runtime ?? runtimeBaseUrl();
  const projectSlug = activeProjectSlug();
  try {
    const result = await fetchJson(runtimeCoreArchitecturePackageUrl(runtime), {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        project_slug: projectSlug,
        node_id: "agent",
        title: "Core architecture proof package",
        output_dir: `worlds/${projectSlug}/output`,
        source: "web-core-architecture-package",
        include_snapshot: true,
      }),
    });
    state.runtimeCoreArchitecturePackage = normalizeRuntimeCoreArchitecturePackage(result);
    await mergeRuntimeMutationSnapshot(result.snapshot, runtime);
    addEvent(
      "ok",
      `核心架构证明包已写出：${state.runtimeCoreArchitecturePackage.readyForCoreArchitecture ? "ready" : "incomplete"} / ${state.runtimeCoreArchitecturePackage.localFiles} files。`,
    );
    saveState();
    renderAll();
  } catch (error) {
    addEvent("warn", `核心架构证明包写出失败：${error.message}`);
    renderEvents();
  }
}

function productionEvidencePayloadFromInput() {
  if (state.snapshot?.mode !== "runtime-http") {
    throw new Error("需要先连接 Runtime HTTP");
  }

  const inputEl = document.querySelector("#productionEvidenceBundle");
  const raw = inputEl?.value?.trim() ?? "";
  if (!raw) {
    fillProductionEvidenceExample();
    throw new Error("生产证据 JSON 为空，已填入示例模板");
  }

  let payload;
  try {
    payload = JSON.parse(raw);
  } catch (error) {
    throw new Error(`生产证据 JSON 解析失败：${error.message}`);
  }
  if (!payload || typeof payload !== "object" || Array.isArray(payload)) {
    throw new Error("生产证据 JSON 必须是 JSON object");
  }
  if (!payload.project_slug) payload.project_slug = activeProjectSlug();
  return payload;
}

function productionEvidenceMergePayloadFromInput() {
  if (state.snapshot?.mode !== "runtime-http") {
    throw new Error("需要先连接 Runtime HTTP");
  }

  const inputEl = document.querySelector("#productionEvidenceBundle");
  const raw = inputEl?.value?.trim() ?? "";
  if (!raw) {
    fillProductionEvidenceExample();
    throw new Error("生产证据 JSON 为空，已填入示例模板");
  }

  let payload;
  try {
    payload = JSON.parse(raw);
  } catch (error) {
    throw new Error(`生产证据 JSON 解析失败：${error.message}`);
  }

  let request;
  if (Array.isArray(payload)) {
    request = {
      project_slug: activeProjectSlug(),
      source: "web-production-evidence-merge",
      bundles: payload,
    };
  } else if (payload && typeof payload === "object") {
    if (Array.isArray(payload.bundles)) {
      request = { ...payload };
      if (!request.project_slug) request.project_slug = activeProjectSlug();
      if (!request.source) request.source = "web-production-evidence-merge";
    } else {
      request = {
        project_slug: payload.project_slug ?? activeProjectSlug(),
        source: "web-production-evidence-merge",
        bundles: [payload],
      };
    }
  } else {
    throw new Error("生产证据 merge 输入必须是 JSON object 或 bundle array");
  }

  if (!Array.isArray(request.bundles) || request.bundles.length === 0) {
    throw new Error("生产证据 merge 需要至少一个 bundle");
  }
  return request;
}

function productionEvidenceCloseoutPayloadFromInput(importEvidence = false) {
  const request = productionEvidenceMergePayloadFromInput();
  if (!request.source || request.source === "web-production-evidence-merge") {
    request.source = "web-production-evidence-closeout";
  }
  request.import = Boolean(importEvidence);
  return request;
}

async function mergeProductionEvidence() {
  let payload;
  try {
    payload = productionEvidenceMergePayloadFromInput();
  } catch (error) {
    addEvent("warn", `生产证据合并失败：${error.message}`);
    renderAll();
    return;
  }

  const runtime = state.snapshot.runtime ?? runtimeBaseUrl();
  try {
    const result = await fetchJson(runtimeProductionEvidenceMergeUrl(runtime), {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(payload),
    });
    const bundle = result?.bundle;
    if (!bundle || typeof bundle !== "object" || Array.isArray(bundle)) {
      throw new Error("Runtime merge response missing bundle");
    }
    const inputEl = document.querySelector("#productionEvidenceBundle");
    if (inputEl) inputEl.value = JSON.stringify(bundle, null, 2);
    state.productionEvidenceMerge = normalizeProductionEvidenceMerge(result);
    state.productionEvidenceTemplate = null;
    state.productionEvidenceItemTemplate = null;
    state.productionEvidenceTaskClaim = null;
    state.productionEvidenceItemValidation = null;
    state.productionEvidenceLedgerBundle = null;
    state.productionEvidenceHandoffPackage = null;
    state.productionEvidenceRunPlan = null;
    state.productionEvidenceCloseout = null;
    state.productionEvidenceValidation = null;
    state.productionEvidenceImport = null;
    addEvent(
      "ok",
      `生产证据已合并：${state.productionEvidenceMerge.inputBundles} bundles / writes=${state.productionEvidenceMerge.writes}。`,
    );
    saveState();
    renderAll();
  } catch (error) {
    addEvent("warn", `生产证据合并失败：${error.message}`);
    renderEvents();
  }
}

async function closeoutProductionEvidence(importEvidence = false) {
  let payload;
  try {
    payload = productionEvidenceCloseoutPayloadFromInput(importEvidence);
  } catch (error) {
    addEvent("warn", `生产证据收口失败：${error.message}`);
    renderAll();
    return;
  }

  const runtime = state.snapshot.runtime ?? runtimeBaseUrl();
  try {
    const result = await fetchJson(runtimeProductionEvidenceCloseoutUrl(runtime), {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(payload),
    });
    const bundle = result?.merge?.bundle;
    if (!bundle || typeof bundle !== "object" || Array.isArray(bundle)) {
      throw new Error("Runtime closeout response missing merged bundle");
    }
    const inputEl = document.querySelector("#productionEvidenceBundle");
    if (inputEl) inputEl.value = JSON.stringify(bundle, null, 2);
    state.productionEvidenceCloseout = normalizeProductionEvidenceCloseout(result);
    state.productionEvidenceMerge = null;
    state.productionEvidenceTemplate = null;
    state.productionEvidenceItemTemplate = null;
    state.productionEvidenceTaskClaim = null;
    state.productionEvidenceItemValidation = null;
    state.productionEvidenceLedgerBundle = null;
    state.productionEvidenceHandoffPackage = null;
    state.productionEvidenceRunPlan = null;
    state.productionEvidenceValidation = null;
    if (importEvidence) {
      state.productionEvidenceImport = normalizeProductionEvidenceImport(result.import);
      await mergeRuntimeMutationSnapshot(result.import?.snapshot, runtime, {
        runtimePrdReadiness: result.import?.prd_readiness,
      });
      addEvent("ok", `生产证据收口已导入：${state.productionEvidenceImport.overallStatus}。`);
    } else {
      state.productionEvidenceImport = null;
      addEvent(
        "ok",
        `生产证据收口预检完成：ready=${state.productionEvidenceCloseout.readyForImport} / writes=${state.productionEvidenceCloseout.writes}。`,
      );
    }
    saveState();
    renderAll();
  } catch (error) {
    addEvent("warn", `生产证据收口失败：${error.message}`);
    renderEvents();
  }
}

async function validateProductionEvidence() {
  let payload;
  try {
    payload = productionEvidencePayloadFromInput();
  } catch (error) {
    addEvent("warn", `生产证据校验失败：${error.message}`);
    renderAll();
    return;
  }

  const runtime = state.snapshot.runtime ?? runtimeBaseUrl();
  try {
    const result = await fetchJson(runtimeProductionEvidenceValidateUrl(runtime), {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(payload),
    });
    state.productionEvidenceValidation = normalizeProductionEvidenceValidation(result);
    state.productionEvidenceItemValidation = null;
    state.productionEvidenceLedgerBundle = null;
    state.productionEvidenceRunPlan = null;
    addEvent("ok", `生产证据校验通过：writes=${state.productionEvidenceValidation.writes}。`);
    saveState();
    renderAll();
  } catch (error) {
    addEvent("warn", `生产证据校验失败：${error.message}`);
    renderEvents();
  }
}

async function importProductionEvidence() {
  let payload;
  try {
    payload = productionEvidencePayloadFromInput();
  } catch (error) {
    addEvent("warn", `生产证据导入失败：${error.message}`);
    renderAll();
    return;
  }

  const runtime = state.snapshot.runtime ?? runtimeBaseUrl();
  try {
    const result = await fetchJson(runtimeProductionEvidenceUrl(runtime), {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(payload),
    });
    state.productionEvidenceImport = normalizeProductionEvidenceImport(result);
    state.productionEvidenceItemValidation = null;
    state.productionEvidenceLedgerBundle = null;
    state.productionEvidenceRunPlan = null;
    await mergeRuntimeMutationSnapshot(result.snapshot, runtime, {
      runtimePrdReadiness: result.prd_readiness,
    });
    addEvent("ok", `生产证据已导入：${state.productionEvidenceImport.overallStatus}。`);
    saveState();
    renderAll();
  } catch (error) {
    addEvent("warn", `生产证据导入失败：${error.message}`);
    renderEvents();
  }
}

async function validateProductionEvidenceItem() {
  let payload;
  try {
    payload = productionEvidencePayloadFromInput();
  } catch (error) {
    addEvent("warn", `单项生产证据预检失败：${error.message}`);
    renderAll();
    return;
  }

  const runtime = state.snapshot.runtime ?? runtimeBaseUrl();
  try {
    const result = await fetchJson(runtimeProductionEvidenceItemsValidateUrl(runtime), {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(payload),
    });
    state.productionEvidenceItemValidation = normalizeProductionEvidenceItemValidation(result);
    state.productionEvidenceImport = null;
    state.productionEvidenceLedgerBundle = null;
    state.productionEvidenceRunPlan = null;
    addEvent("ok", `单项生产证据预检通过：writes=${state.productionEvidenceItemValidation.writes}。`);
    saveState();
    renderAll();
  } catch (error) {
    addEvent("warn", `单项生产证据预检失败：${error.message}`);
    renderEvents();
  }
}

async function submitProductionEvidenceItem() {
  let payload;
  try {
    payload = productionEvidencePayloadFromInput();
  } catch (error) {
    addEvent("warn", `单项生产证据提交失败：${error.message}`);
    renderAll();
    return;
  }

  const runtime = state.snapshot.runtime ?? runtimeBaseUrl();
  try {
    const result = await fetchJson(runtimeProductionEvidenceItemsUrl(runtime), {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(payload),
    });
    state.productionEvidenceImport = normalizeProductionEvidenceImport(result);
    state.productionEvidenceItemTemplate = null;
    state.productionEvidenceTaskClaim = null;
    state.productionEvidenceItemValidation = null;
    state.productionEvidenceLedgerBundle = null;
    state.productionEvidenceRunPlan = null;
    await mergeRuntimeMutationSnapshot(result.snapshot, runtime, {
      runtimePrdReadiness: result.prd_readiness,
    });
    addEvent("ok", `单项生产证据已提交：${state.productionEvidenceImport.overallStatus}。`);
    saveState();
    renderAll();
  } catch (error) {
    addEvent("warn", `单项生产证据提交失败：${error.message}`);
    renderEvents();
  }
}

function apiKeyForProvider(providerId) {
  const canonical = canonicalProviderId(providerId);
  return (state.apiKeys ?? []).find((key) => key.provider === canonical && key.service_type === "provider");
}

function latestProviderRequestForProvider(providerId) {
  const canonical = canonicalProviderId(providerId);
  return (state.providerRequests ?? []).find((request) => request.providerId === canonical);
}

function latestProviderRequestForNode(node) {
  const nodeTasks = state.tasks.filter((task) => task.nodeId === node.id);
  const taskIds = new Set(nodeTasks.map((task) => task.id));
  const direct = (state.providerRequests ?? []).find((request) => taskIds.has(request.taskId));
  if (direct) return direct;
  const providerIds = new Set([
    canonicalProviderId(node.agent ?? ""),
    ...nodeTasks.map((task) => canonicalProviderId(task.tool ?? "")),
  ]);
  return (state.providerRequests ?? []).find((request) => providerIds.has(request.providerId));
}

function latestSoftwareActionForSoftware(software) {
  const adapterId = software.id ?? softwareAdapterId(software.name);
  return (state.softwareActions ?? []).find((action) => action.adapterId === adapterId);
}

function latestSoftwareActionForNode(node) {
  const nodeTasks = state.tasks.filter((task) => task.nodeId === node.id);
  const taskIds = new Set(nodeTasks.map((task) => task.id));
  const direct = (state.softwareActions ?? []).find((action) => taskIds.has(action.taskId));
  if (direct) return direct;
  const nodeText = `${node.id} ${node.type} ${node.taskType ?? ""} ${node.title} ${node.agent} ${node.control}`.toLowerCase();
  if (nodeText.includes("unreal")) return (state.softwareActions ?? []).find((action) => action.adapterId === "unreal");
  if (nodeText.includes("hermes")) return (state.softwareActions ?? []).find((action) => action.adapterId === "hermes");
  if (nodeText.includes("software") || nodeText.includes("control")) return (state.softwareActions ?? [])[0];
  return null;
}

function canonicalProviderId(providerId) {
  if (!providerId) return "";
  if (state.providerAliases?.[providerId]) return state.providerAliases[providerId];
  const aliases = {
    "world-labs-marble": "worldlabs-marble",
    triposplat: "tripo-splat",
    spark: "spark-3dgs",
    qunhe: "qunhe-3d",
    openai: "openai-image-2",
    "openai-image": "openai-image-2",
    "image-2": "openai-image-2",
    mj: "midjourney",
    "nano-banana": "nano-banana-pro",
    nanobanana: "nano-banana-pro",
    nanobananapro: "nano-banana-pro",
  };
  return aliases[providerId] ?? providerId;
}

function runtimeEndpointForProvider(provider) {
  const endpoint = provider.runtimeEndpoint?.trim();
  if (!endpoint || endpoint.startsWith("provider://")) return undefined;
  return endpoint;
}

function renderHermes() {
  document.querySelector("#hermesStatus").textContent = statusLabel(state.hermes.status);
  document.querySelector("#hermesEndpoint").value = state.hermes.endpoint;
  document.querySelector("#hermesPrompt").value = state.hermes.lastCommand;
  const runbookSelect = document.querySelector("#hermesRunbookSelect");
  runbookSelect.innerHTML = state.hermes.runbooks
    .map((runbook) => `<option value="${runbook.name}">${runbook.title}</option>`)
    .join("");
  runbookSelect.value = state.hermes.selectedRunbook;
  document.querySelector("#hermesRunbookTarget").value = state.hermes.runbookTarget;
  document.querySelector("#hermesRunbookSource").textContent = state.hermes.runbookStatus === "runtime" ? "runtime" : "local";
  document.querySelector("#hermesRunbookPreview").innerHTML = renderHermesRunbookPreview();
  document.querySelector("#hermesDecisionPanel").innerHTML = renderHermesDecisions();
  document.querySelector("#hermesSessionStream").innerHTML = renderHermesSessionStream();
  document.querySelectorAll("[data-agent-transcript]").forEach((button) => {
    button.addEventListener("click", () => loadAgentTranscript(button.dataset.agentTranscript));
  });
  document.querySelector("#hermesTrace").innerHTML = state.hermes.trace
    .slice(0, 8)
    .map((line) => `<p>${line}</p>`)
    .join("");
}

function renderHermesRunbookPreview() {
  const runbook = selectedHermesRunbook();
  const preview = state.hermes.runbookPreview || runbook?.description || "";
  return `
    <div>
      <strong>${runbook?.title ?? "Runbook"}</strong>
      <span>${runbookArgumentSummary(runbook)}</span>
    </div>
    ${preview ? `<p>${compactPrompt(preview)}</p>` : ""}
  `;
}

function selectedHermesRunbook() {
  return state.hermes.runbooks.find((runbook) => runbook.name === state.hermes.selectedRunbook) ?? state.hermes.runbooks[0];
}

function runbookArgumentSummary(runbook) {
  const args = runbook?.arguments?.map((argument) => argument.name).filter(Boolean) ?? [];
  return args.length ? args.join(" / ") : "ready";
}

function renderHermesDecisions() {
  const report = state.hermes.workflowReport;
  const reportRow = report
    ? `
      <div class="decision-row">
        <strong>ContentBurst 运行决策</strong>
        <div class="decision-metrics">
          <span>Agent ${report.agentMode}</span>
          <span>3DGS ${report.threeDgsMode}</span>
          <span>Unreal ${report.unrealMode}</span>
          <span>${report.assetsIndexed} assets</span>
        </div>
        <span>Provider ${statusLabel(report.providerStatus)} / Software ${statusLabel(report.softwareStatus)} / Output ${statusLabel(report.outputStatus)}</span>
        ${report.transcriptPath ? `<code>${report.transcriptPath}</code>` : ""}
      </div>
    `
    : "";
  const decisionRows = (state.hermes.decisions ?? [])
    .map(
      (decision) => `
        <div class="decision-row">
          <strong>${decision.title}</strong>
          <div class="decision-metrics">
            <span>${statusLabel(decision.status)}</span>
            <span>${decision.tokenUsed} tokens</span>
            <span>${decision.tools.slice(0, 3).join(" / ") || "tools pending"}</span>
          </div>
          <span>${decision.at}</span>
          ${decision.transcriptPath ? `<code>${decision.transcriptPath}</code>` : ""}
        </div>
      `,
    )
    .join("");
  if (reportRow || decisionRows) return `${reportRow}${decisionRows}`;
  return `
    <div class="decision-row">
      <strong>等待 Runtime Agent 决策</strong>
      <span>运行一次后会显示 Hermes transcript、adapter 模式和回退状态。</span>
    </div>
  `;
}

function renderHermesSessionStream() {
  const sessions = state.hermes.sessions ?? [];
  if (!sessions.length) {
    return `
      <div class="session-row empty-session">
        <strong>等待 Agent 会话</strong>
        <span>Hermes 或 Agent CLI 写入 Runtime 后，这里会显示 transcript、token 和工具范围。</span>
      </div>
    `;
  }
  return sessions
    .map((session) => {
      const transcript = state.hermes.sessionTranscripts?.[session.id];
      return `
        <article class="session-row status-${session.status}">
          <header>
            <strong>${session.title}</strong>
            <span>${statusLabel(session.status)} / ${session.at}</span>
          </header>
          <div class="decision-metrics">
            <span>${session.tokenUsed}${session.tokenBudget ? `/${session.tokenBudget}` : ""} tokens</span>
            <span>${session.tools.slice(0, 4).join(" / ") || "tools pending"}</span>
          </div>
          ${session.transcriptPath ? `<code>${session.transcriptPath}</code>` : ""}
          ${session.transcriptPath ? `
            <div class="inline-actions">
              <button class="mini-command" data-agent-transcript="${session.id}" type="button">${transcript ? "刷新 transcript" : "读取 transcript"}</button>
            </div>
          ` : ""}
          ${transcript ? `<pre class="session-transcript">${escapeHtml(formatAgentTranscript(transcript))}</pre>` : ""}
        </article>
      `;
    })
    .join("");
}

function formatAgentTranscript(payload) {
  const body = payload?.transcript && payload.transcript !== null
    ? payload.transcript
    : payload?.transcript_text ?? payload;
  const text = JSON.stringify(body, null, 2);
  return text.length > 2000 ? `${text.slice(0, 1997)}...` : text;
}

function renderCliCommands() {
  document.querySelector("#cliSummary").textContent = `${state.cliCommands.length} commands`;
  document.querySelector("#cliCommandList").innerHTML = state.cliCommands
    .map(
      (item) => {
        const command = cliCommandText(item);
        return `
        <article class="cli-row">
          <div>
            <span>${item.title}</span>
            <p>${item.description}</p>
          </div>
          <code>${command}</code>
          <div class="inline-actions">
            <button class="mini-command" data-copy-cli="${item.id}" type="button">复制</button>
            <button class="mini-command primary-mini" data-stage-cli="${item.id}" type="button">写入队列</button>
          </div>
        </article>
      `;
      },
    )
    .join("");

  document.querySelectorAll("[data-copy-cli]").forEach((button) => {
    button.addEventListener("click", () => copyCliCommand(button.dataset.copyCli));
  });
  document.querySelectorAll("[data-stage-cli]").forEach((button) => {
    button.addEventListener("click", () => stageCliCommand(button.dataset.stageCli));
  });
}

function renderTasks() {
  const runningCount = state.tasks.filter((task) => task.status === "running").length;
  document.querySelector("#queueSummary").textContent = `${runningCount} running`;
  document.querySelector("#taskQueue").innerHTML = state.tasks
    .map((task) => {
      const node = nodeById(task.nodeId);
      return `
        <article class="task-row status-${task.status}">
          <button class="task-main" data-task-node="${task.nodeId}" type="button">
            <span>${task.id}</span>
            <strong>${task.title}</strong>
            <small>${node.title} / ${task.tool}</small>
            <em>${statusLabel(task.status)} · ${task.risk} · ${formatTokens(task.cost)}</em>
          </button>
          <div class="task-actions">
            <button class="mini-command" data-cancel-task="${task.id}" type="button">取消</button>
            <button class="mini-command primary-mini" data-retry-task="${task.id}" type="button">重试</button>
          </div>
        </article>
      `;
    })
    .join("");

  document.querySelectorAll("[data-task-node]").forEach((button) => {
    button.addEventListener("click", () => {
      showPanel("workflow");
      selectNode(button.dataset.taskNode);
    });
  });
  document.querySelectorAll("[data-cancel-task]").forEach((button) => {
    button.addEventListener("click", () => cancelTask(button.dataset.cancelTask));
  });
  document.querySelectorAll("[data-retry-task]").forEach((button) => {
    button.addEventListener("click", () => retryTask(button.dataset.retryTask));
  });
}

async function cancelTask(id) {
  const task = state.tasks.find((item) => item.id === id);
  if (!task) return;

  if (state.snapshot?.mode === "runtime-http") {
    try {
      const result = await updateRuntimeTaskStatus(id, "cancel");
      addEvent("warn", `Runtime task 已取消：${result.task.title}`);
      saveState();
      renderAll();
      selectNode(result.task.node_id ?? task.nodeId);
      return;
    } catch (error) {
      addEvent("warn", `Runtime task 取消失败：${error.message}`);
      saveState();
      renderAll();
      selectNode(task.nodeId);
      return;
    }
  }

  task.status = "cancelled";
  addEvent("warn", `任务已在本地标记取消：${task.title}`);
  saveState();
  renderAll();
  selectNode(task.nodeId);
}

async function retryTask(id) {
  const task = state.tasks.find((item) => item.id === id);
  if (!task) return;

  if (state.snapshot?.mode === "runtime-http") {
    try {
      const result = await updateRuntimeTaskStatus(id, "retry");
      addEvent("info", `Runtime task 已重试为就绪：${result.task.title}`);
      saveState();
      renderAll();
      selectNode(result.task.node_id ?? task.nodeId);
      return;
    } catch (error) {
      addEvent("warn", `Runtime task 重试失败：${error.message}`);
      saveState();
      renderAll();
      selectNode(task.nodeId);
      return;
    }
  }

  task.status = "ready";
  addEvent("info", `任务已在本地恢复为就绪：${task.title}`);
  saveState();
  renderAll();
  selectNode(task.nodeId);
}

async function updateRuntimeTaskStatus(id, action) {
  const runtime = state.snapshot?.runtime ?? runtimeBaseUrl();
  if (!runtime) throw new Error("runtime endpoint missing");
  const result = await fetchJson(`${runtime}/api/tasks/${action}?task_id=${encodeURIComponent(id)}`, {
    method: "POST",
  });
  await mergeRuntimeMutationSnapshot(result.snapshot, runtime);
  return result;
}

async function runRuntimeExecutionPlanNext(execute = false) {
  const runtime = state.snapshot?.runtime ?? runtimeBaseUrl();
  if (!runtime) {
    addEvent("warn", "Runtime endpoint missing，无法调度 execution plan。");
    renderAll();
    return;
  }
  try {
    const result = await fetchJson(runtimeExecutionPlanRunNextUrl(runtime), {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        project_slug: activeProjectSlug(),
        execute,
      }),
    });
    state.runtimeRunNextResult = result;
    if (result.dispatch?.snapshot) {
      await mergeRuntimeMutationSnapshot(result.dispatch.snapshot, runtime);
    }
    const title = result.selected_step?.title ?? "下一步";
    addEvent(execute ? "ok" : "info", execute ? `Runtime 已调度：${title}` : `Runtime 下一步预览：${title}`);
    saveState();
    renderAll();
  } catch (error) {
    addEvent("warn", `Runtime execution plan 调度失败：${error.message}`);
    saveState();
    renderAll();
  }
}

function renderAssets() {
  document.querySelector("#assetSummary").textContent = `${state.assets.length} files`;
  document.querySelector("#assetLedger").innerHTML = state.assets
    .map(
      (asset) => `
        <button class="asset-row" data-asset-node="${asset.source}" type="button">
          <span>${asset.type}</span>
          <strong>${asset.name}</strong>
          <small>${asset.path}</small>
          <em>${statusLabel(asset.status)}</em>
        </button>
      `,
    )
    .join("");

  document.querySelectorAll("[data-asset-node]").forEach((button) => {
    button.addEventListener("click", () => {
      showPanel("workflow");
      selectNode(button.dataset.assetNode);
    });
  });
}

function renderAdapters() {
  const adapters = state.software.filter((item) => ["ready", "checking", "guarded"].includes(item.health));
  const readyCount = state.software.filter((item) => item.health === "ready").length;
  document.querySelector("#adapterSummary").textContent = `${readyCount}/${state.software.length} ready`;
  document.querySelector("#adapterPanel").innerHTML = adapters
    .map(
      (item) => `
        <div class="adapter-row health-${item.health}">
          <span>${item.name}</span>
          <strong>${statusLabel(item.health)}</strong>
          <small>${item.mode}</small>
        </div>
      `,
    )
    .join("");
}

function renderOutputManifests() {
  const manifests = state.outputManifests ?? [];
  document.querySelector("#outputManifestPanel").innerHTML = manifests.length
    ? manifests
        .map(
          (manifest) => {
            const execution = outputExecutionStatus(manifest);
            const canRecord = state.snapshot?.mode === "runtime-http" && manifest.target;
            return `
            <article class="manifest-card target-${manifest.target}">
              <span>${manifest.target}</span>
              <strong>${manifest.title}</strong>
              <small>${manifest.primaryRuntime} / ${statusLabel(manifest.status ?? "ready")}</small>
              ${
                execution
                  ? `<div class="manifest-result status-${normalizeRuntimeStatus(execution)}"><span>执行结果</span><strong>${statusLabel(execution)}</strong></div>`
                  : ""
              }
              <div class="manifest-metrics">
                ${manifest.metrics.map((metric) => `<em>${metric.label}: ${metric.value}</em>`).join("")}
              </div>
              ${manifest.localPath ? `<code>${manifest.localPath}</code>` : ""}
              ${
                canRecord
                  ? `<div class="manifest-actions"><button class="mini-command primary-mini" type="button" data-output-result-target="${manifest.target}">标记后段完成</button></div>`
                  : ""
              }
            </article>
          `;
          },
        )
        .join("")
    : `
      <article class="manifest-card">
        <span>manifest</span>
        <strong>等待输出包</strong>
        <small>生成输出包后显示视频时间线、游戏构建和交互 cue 摘要。</small>
      </article>
    `;

  document.querySelectorAll("[data-output-result-target]").forEach((button) => {
    button.addEventListener("click", () => recordOutputResult(button.dataset.outputResultTarget));
  });
}

function renderEvents() {
  document.querySelector("#eventSummary").textContent = `${state.events.length} events`;
  document.querySelector("#eventStream").innerHTML = state.events
    .map(
      (event) => `
        <div class="event-row level-${event.level}">
          <span>${event.at}</span>
          <p>${event.text}</p>
        </div>
      `,
    )
    .join("");
}

function renderProjectSelector() {
  const selector = document.querySelector("#projectSelector");
  if (!selector) return;
  const current = currentProjectOptionValue();
  const projectOptions = state.projects
    .map(
      (project) =>
        `<option value="${project.slug}" ${project.slug === current ? "selected" : ""}>${project.slug}</option>`,
    )
    .join("");
  const selected = current === "*" ? "selected" : "";
  selector.innerHTML = `<option value="*" ${selected}>全部项目</option>${projectOptions}`;
  if (current !== "*" && !state.projects.some((project) => project.slug === current)) {
    selector.insertAdjacentHTML("beforeend", `<option value="${current}" selected>${current}</option>`);
  }
  selector.value = current;
  selector.disabled = state.snapshot?.mode !== "runtime-http";
}

function renderMetrics() {
  const active = state.nodes.filter((node) => ["ready", "running", "waiting_approval"].includes(node.status)).length;
  const waiting = state.tasks.filter((task) => task.status === "waiting_approval").length;
  const readyAdapters = state.software.filter((item) => item.health === "ready").map((item) => item.name);
  const highRisk = state.tasks.filter((task) => task.risk === "high").length;
  const budgetPercent = Math.min(100, Math.round((state.tokenTotal / state.budgetLimit) * 100));
  const providerReady = state.apiProviders.filter((provider) => provider.status === "ready").length;
  const providerActionable = state.apiProviders.filter((provider) => ["ready", "checking", "guarded"].includes(provider.status)).length;
  const currentFilter = state.snapshot?.projectFilter;
  const currentProject = state.projects.find((project) => project.slug === currentFilter);

  document.querySelector("#currentProject").textContent = currentFilter ?? "全部项目";
  document.querySelector("#currentProjectText").textContent = currentFilter
    ? `${currentProject?.name ?? currentFilter} / ${currentProject?.status ?? "active"}`
    : `${state.projects.length || 1} 个项目汇总`;
  document.querySelector("#activeNodes").textContent = `${active} / ${state.nodes.length}`;
  document.querySelector("#assetCount").textContent = state.assets.length;
  document.querySelector("#queueRisk").textContent = `${waiting} 个确认门，${highRisk} 个高风险任务`;
  document.querySelector("#adapterHealth").textContent = readyAdapters[0] ?? "待检";
  document.querySelector("#adapterHealthText").textContent = `${readyAdapters.length} 个 Adapter 可用`;
  document.querySelector("#providerCount").textContent = `${providerReady}/${state.apiProviders.length}`;
  document.querySelector("#providerHealthText").textContent = `${providerActionable} 个可进入连接流程`;
  document.querySelector("#tokenTotal").textContent = formatTokens(state.tokenTotal);
  document.querySelector(".budget-meter span").style.width = `${budgetPercent}%`;
}

function renderAll() {
  renderProjectSelector();
  renderNodes();
  renderAgents();
  renderSoftware();
  renderDesktopRecognitionQueue();
  renderProviders();
  renderIntegrationReadiness();
  renderProductionEvidenceImport();
  renderRuntimeBudget();
  renderRuntimePreflight();
  renderRuntimeHandoff();
  renderPrdReadiness();
  renderHermes();
  renderCliCommands();
  renderTasks();
  renderAssets();
  renderAdapters();
  renderOutputManifests();
  renderEvents();
  renderMetrics();
}

function showPanel(id) {
  panels.forEach((panel) => panel.classList.toggle("active-panel", panel.id === id));
  navButtons.forEach((button) => button.classList.toggle("active", button.dataset.panel === id));
}

async function changeRuntimeProject(value) {
  const project = value || "*";
  setRuntimeProjectFilter(project);
  if (state.snapshot?.mode !== "runtime-http") {
    addEvent("info", `项目过滤已保存：${project}`);
    renderAll();
    return;
  }

  const runtime = state.snapshot.runtime;
  stopRuntimeEventPolling();
  try {
    const connected = await applyRuntimeHttpSnapshot(runtime, { explicit: true });
    if (!connected) addEvent("warn", `Runtime 项目切换失败：${runtime}`);
    renderAll();
    selectNode(state.selectedNode);
  } catch (error) {
    addEvent("warn", `Runtime 项目切换失败：${error.message}`);
    renderAll();
  }
}

function formatTokens(value) {
  if (value >= 1000) return `${(value / 1000).toFixed(1)}k`;
  return String(value);
}

function formatSignedTokens(value) {
  const sign = value < 0 ? "-" : "";
  return `${sign}${formatTokens(Math.abs(value))}`;
}

function completeRunningNodes() {
  state.nodes.forEach((node) => {
    if (node.status === "running") {
      node.status = "succeeded";
      node.progress = 100;
      node.log = `${node.title} 已完成，输出已进入下游节点。`;
    }
  });
  state.tasks.forEach((task) => {
    if (task.status === "running") task.status = "succeeded";
  });
}

function addAssetForNode(node) {
  const assetMap = {
    agent: ["workflow-plan.json", "WorkflowGraph", "worlds/neon-bazaar/workflow-plan.json"],
    image3d: ["1-world-full_res.spz", "3DGS Splat", "worlds/neon-bazaar/output/world/1-world-full_res.spz"],
    software: ["desktop-action-trace.json", "Control Trace", "worlds/neon-bazaar/output/control/desktop-action-trace.json"],
    unreal: ["neon-bazaar.umap", "Unreal Level", "worlds/neon-bazaar/output/unreal/neon-bazaar.umap"],
    outputs: ["review-cut.mp4", "Video Export", "worlds/neon-bazaar/output/video/review-cut.mp4"],
  };
  const asset = assetMap[node.id];
  if (!asset || state.assets.some((item) => item.name === asset[0])) return;
  state.assets.unshift({
    id: `asset-${Date.now()}`,
    name: asset[0],
    type: asset[1],
    path: asset[2],
    source: node.id,
    status: "local",
  });
}

async function simulateRun() {
  if (state.snapshot?.mode === "runtime-http") {
    try {
      const result = await runRuntimeWorkflow();
      addEvent("ok", `Runtime 工作流闭环完成：${result.report.assets_indexed} 个资产已入库。`);
      showPanel("ops");
      saveState();
      renderAll();
      selectNode(state.nodes.find((node) => /输出|output/i.test(`${node.type} ${node.title}`))?.id ?? state.selectedNode);
      return;
    } catch (error) {
      addEvent("warn", `Runtime 工作流运行失败，改用本地演示步进：${error.message}`);
    }
  }

  const sequence = ["brief", "agent", "image3d", "software", "unreal", "outputs"];
  completeRunningNodes();

  const currentId = sequence[state.runStep % sequence.length];
  const current = nodeById(currentId);
  if (current.id === "outputs" && state.snapshot?.mode === "runtime-http") {
    await createOutputPackage();
    state.runStep += 1;
    return;
  }
  const approvalRequired = current.id === "image3d";
  current.status = approvalRequired ? "waiting_approval" : "running";
  current.progress = approvalRequired ? Math.max(current.progress, 20) : Math.min(94, Math.max(18, current.progress + 34));
  current.log = approvalRequired
    ? "该节点将调用付费生成或外部软件批量动作，已进入人工确认门。"
    : `正在执行 ${current.title}，任务状态已写入节点日志。`;

  const task = state.tasks.find((item) => item.nodeId === current.id);
  if (task) task.status = current.status;
  state.tokenTotal += current.cost;
  addAssetForNode(current);
  addEvent(approvalRequired ? "warn" : "info", `${current.title} 已进入 ${statusLabel(current.status)}。`);
  state.runStep += 1;
  saveState();
  renderAll();
  selectNode(current.id);
}

async function runSelectedNode() {
  const node = nodeById(state.selectedNode);
  if (!node) return;

  if (state.snapshot?.mode === "runtime-http") {
    try {
      const result = await createRuntimeNodeRun(node);
      applyOutputPackageReport(result.report);
      addEvent("ok", `${node.title} 已通过 Runtime HTTP 节点运行入口执行。`);
      showPanel("ops");
      saveState();
      renderAll();
      selectNode(result.task?.node_id ?? node.id);
      return;
    } catch (error) {
      addEvent("warn", `Runtime HTTP 节点运行失败，改用本地节点状态：${error.message}`);
    }
  }

  node.status = node.status === "waiting_approval" ? "waiting_approval" : "running";
  node.progress = Math.min(96, Math.max(node.progress + 24, 24));
  node.log = `${node.title} 已从节点详情触发运行。`;
  state.tasks.unshift({
    id: `task-${String(state.tasks.length + 1).padStart(3, "0")}`,
    nodeId: node.id,
    title: `${node.title} 节点运行`,
    type: "node-run",
    status: node.status,
    tool: node.control,
    risk: node.status === "waiting_approval" ? "high" : "medium",
    cost: node.cost ?? 0,
  });
  addEvent(node.status === "waiting_approval" ? "warn" : "info", `${node.title} 已写入本地节点运行队列。`);
  saveState();
  renderAll();
  selectNode(node.id);
}

async function createRuntimeNodeRun(node) {
  const runtime = state.snapshot?.runtime ?? runtimeBaseUrl();
  if (!runtime) throw new Error("runtime endpoint missing");
  const result = await fetchJson(`${runtime}/api/nodes/run`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      project_slug: activeProjectSlug(),
      node_id: node.id,
      prompt: node.log,
      duration_ms: 12000,
    }),
  });
  if (result.snapshot) {
    await mergeRuntimeMutationSnapshot(result.snapshot, runtime);
  }
  return result;
}

async function approveGate() {
  const gatedNode = state.nodes.find((node) => node.status === "waiting_approval");
  if (!gatedNode) {
    addEvent("info", "当前没有等待人工确认的节点。");
    renderAll();
    selectNode(state.selectedNode);
    return;
  }

  const gatedTask = state.tasks.find(
    (task) => task.nodeId === gatedNode.id && task.status === "waiting_approval",
  );
  if (state.snapshot?.mode === "runtime-http" && gatedTask) {
    try {
      await approveRuntimeTask(gatedTask);
      addEvent("ok", `${gatedNode.title} 已通过 Runtime HTTP 审批。`);
      saveState();
      renderAll();
      selectNode(gatedNode.id);
      return;
    } catch (error) {
      addEvent("warn", `Runtime HTTP 审批失败，保留本地确认路径：${error.message}`);
    }
  }

  gatedNode.status = "running";
  gatedNode.progress = Math.max(gatedNode.progress, 45);
  gatedNode.log = "人工确认已通过，Agent 可继续调用生成接口与外部软件控制器。";
  state.tasks
    .filter((task) => task.nodeId === gatedNode.id)
    .forEach((task) => {
      task.status = "running";
    });
  addEvent("ok", `${gatedNode.title} 的人工确认已通过。`);
  saveState();
  renderAll();
  selectNode(gatedNode.id);
}

async function approveRuntimeTask(task) {
  const runtime = state.snapshot?.runtime ?? runtimeBaseUrl();
  if (!runtime) throw new Error("runtime endpoint missing");
  const result = await fetchJson(`${runtime}/api/tasks/approve?task_id=${encodeURIComponent(task.id)}`, {
    method: "POST",
  });
  await mergeRuntimeMutationSnapshot(result.snapshot, runtime);
  return result.task;
}

function exportState() {
  const blob = new Blob([JSON.stringify(state, null, 2)], { type: "application/json" });
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = "pool-neon-bazaar-state.json";
  link.click();
  URL.revokeObjectURL(url);
  addEvent("ok", "已导出当前项目状态 JSON。");
  saveState();
  renderAll();
}

function resetState() {
  stopRuntimeEventPolling();
  state = clone(initialState);
  localStorage.removeItem(STORAGE_KEY);
  addEvent("info", "演示状态已重置到初始项目。");
  saveState();
  renderAll();
  selectNode(state.selectedNode);
}

function providerById(id) {
  const canonical = canonicalProviderId(id);
  return state.apiProviders.find((provider) => canonicalProviderId(provider.id) === canonical);
}

function providerDisplayName(id) {
  const canonical = canonicalProviderId(id);
  const provider = providerById(canonical);
  if (provider?.name) return provider.name;
  return {
    "worldlabs-marble": "World Labs Marble",
    "tripo-splat": "TripoSplat",
    "sam-3d": "SAM-3D",
    "spark-3dgs": "Spark",
    "qunhe-3d": "群核科技",
    "openai-image-2": "OpenAI image-2",
    "nano-banana-pro": "Nano Banana Pro",
  }[canonical] ?? id;
}

async function testProviderConnection(id) {
  const provider = providerById(id);
  if (!provider) return;

  if (state.snapshot?.mode === "runtime-http") {
    provider.status = "checking";
    provider.lastHealth = "Runtime provider health 检查中...";
    renderAll();
    try {
      const result = await createRuntimeProviderHealth(provider);
      const health = result.health ?? {};
      provider.status = providerStatusFromHealth(health.status);
      provider.lastHealth = `${result.adapter_mode ?? "adapter"} / ${health.status ?? "unknown"}: ${health.message ?? "no message"}`;
      addEvent(
        provider.status === "ready" ? "ok" : "warn",
        `${provider.name} Runtime health：${statusLabel(provider.status)} / ${health.message ?? "无详情"}`,
      );
      saveState();
      renderAll();
      return;
    } catch (error) {
      provider.status = "checking";
      provider.lastHealth = `Runtime health failed: ${error.message}`;
      addEvent("warn", `${provider.name} Runtime health 检查失败，已切回本地模拟：${error.message}`);
    }
  }

  if (provider.status === "needs_key") {
    provider.status = "checking";
    addEvent("warn", `${provider.name} 需要配置 ${provider.auth}，已进入凭证检查。`);
  } else if (provider.status === "planned") {
    provider.status = "checking";
    addEvent("info", `${provider.name} Adapter 已加入连接测试队列。`);
  } else if (provider.status === "guarded") {
    addEvent("warn", `${provider.name} 需要授权 API 与人工确认，不执行自动探测。`);
  } else {
    provider.status = "ready";
    addEvent("ok", `${provider.name} 连接测试通过，可创建生成任务。`);
  }

  if (provider.status === "checking" && provider.auth === "local") {
    provider.status = "ready";
    addEvent("ok", `${provider.name} 本地接口已标记可用。`);
  }

  saveState();
  renderAll();
}

async function createRuntimeProviderHealth(provider) {
  const runtime = state.snapshot?.runtime ?? runtimeBaseUrl();
  if (!runtime) throw new Error("runtime endpoint missing");
  return fetchJson(`${runtime}/api/provider-health`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      provider_id: provider.id,
      execution_mode: provider.group === "3dgs" ? "auto" : "adapter",
      endpoint: runtimeEndpointForProvider(provider),
    }),
  });
}

async function runAdapterHealthSweep() {
  if (state.snapshot?.mode !== "runtime-http") {
    state.apiProviders.forEach((provider) => {
      provider.status = provider.status === "guarded" ? "guarded" : provider.auth === "local" ? "ready" : "checking";
      provider.lastHealth = "本地模拟批量巡检，连接 Runtime HTTP 后可执行真实 adapter health。";
    });
    state.software.forEach((software) => {
      software.health = software.health === "guarded" ? "guarded" : "ready";
      software.lastHealth = "本地模拟批量巡检，连接 Runtime HTTP 后可执行真实软件 adapter health。";
    });
    addEvent("info", "本地 Adapter 批量巡检已更新模拟状态。");
    saveState();
    renderAll();
    return;
  }

  state.apiProviders.forEach((provider) => {
    provider.status = "checking";
    provider.lastHealth = "Runtime 批量巡检中...";
  });
  state.software.forEach((software) => {
    software.health = "checking";
    software.lastHealth = "Runtime 批量巡检中...";
  });
  renderAll();

  try {
    const result = await createRuntimeAdapterHealthSweep();
    applyAdapterHealthSweep(result);
    const summary = result.summary ?? {};
    addEvent(
      summary.failed ? "warn" : "ok",
      `Adapter 批量巡检完成：Provider ${summary.providers_ready ?? 0}/${summary.providers_total ?? 0}，Software ${summary.software_ready ?? 0}/${summary.software_total ?? 0}。`,
    );
    saveState();
    renderAll();
  } catch (error) {
    addEvent("warn", `Runtime Adapter 批量巡检失败：${error.message}`);
    saveState();
    renderAll();
  }
}

async function createRuntimeAdapterHealthSweep() {
  const runtime = state.snapshot?.runtime ?? runtimeBaseUrl();
  if (!runtime) throw new Error("runtime endpoint missing");
  return fetchJson(`${runtime}/api/adapter-health`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      providers: state.apiProviders.map((provider) => ({
        provider_id: provider.id,
        execution_mode: provider.group === "3dgs" ? "auto" : "adapter",
        endpoint: runtimeEndpointForProvider(provider),
      })),
      software_adapters: state.software.map((software) => {
        const endpoint = software.runtimeEndpoint?.trim();
        return {
          adapter_id: software.id ?? softwareAdapterId(software.name),
          priority: softwareControlPriority(software),
          payload_json: {
            software: software.name,
            scope: software.scope,
            ...(endpoint ? { endpoint } : {}),
            requested_from: "web-prototype-adapter-health",
          },
        };
      }),
    }),
  });
}

function applyAdapterHealthSweep(result) {
  const providerResults = new Map(
    (result.providers ?? []).map((item) => [canonicalProviderId(item.provider_id ?? ""), item]),
  );
  state.apiProviders.forEach((provider) => {
    const item = providerResults.get(canonicalProviderId(provider.id));
    if (!item) return;
    provider.status = providerStatusFromHealth(item.health?.status);
    provider.lastHealth = `${item.adapter_mode ?? "adapter"} / ${item.health?.status ?? item.error ?? "failed"}: ${item.health?.message ?? item.message ?? "no message"}`;
  });

  const softwareResults = new Map(
    (result.software_adapters ?? []).map((item) => [item.adapter_id, item]),
  );
  state.software.forEach((software) => {
    const adapterId = software.id ?? softwareAdapterId(software.name);
    const item = softwareResults.get(adapterId);
    if (!item) return;
    software.health = softwareStatusFromHealth(item);
    software.latency = item.adapter_mode ?? "runtime";
    software.lastHealth = `${item.adapter_mode ?? "adapter"} / ${item.health?.ok ? "ok" : item.error ?? "needs attention"}: ${item.health?.message ?? item.message ?? "no message"}`;
  });
}

function providerStatusFromHealth(status) {
  const normalized = normalizeRuntimeStatus(status);
  if (normalized === "ready") return "ready";
  if (normalized === "missing_auth") return "needs_key";
  if (normalized === "missing_endpoint" || normalized === "unhealthy") return "checking";
  if (normalized === "failed") return "failed";
  return "checking";
}

async function enqueueProviderTask(id) {
  const provider = providerById(id);
  if (!provider) return;

  if (state.snapshot?.mode === "runtime-http") {
    try {
      const result = await createRuntimeProviderTask(provider);
      addEvent("ok", `${provider.name} 任务已写入 Runtime HTTP：${statusLabel(normalizeRuntimeStatus(result.status))}。`);
      showPanel("ops");
      saveState();
      renderAll();
      selectNode(result.node_id ?? state.selectedNode);
      return;
    } catch (error) {
      addEvent("warn", `Runtime HTTP 创建任务失败，保留本地队列路径：${error.message}`);
    }
  }

  const nodeId = nodeIdForProvider(provider);
  const risk = provider.cost >= 6000 || provider.status !== "ready" ? "high" : "medium";
  const status = risk === "high" ? "waiting_approval" : "ready";
  const task = {
    id: `task-${String(state.tasks.length + 1).padStart(3, "0")}`,
    nodeId,
    title: `${provider.name} 生成任务`,
    type: provider.group === "3dgs" ? "3dgs-provider" : "ai-provider",
    status,
    tool: provider.mode,
    risk,
    cost: provider.cost,
  };

  state.tasks.unshift(task);
  state.assets.unshift({
    id: `asset-provider-${Date.now()}`,
    name: `${provider.id}-request.json`,
    type: "Provider Request",
    path: `worlds/neon-bazaar/output/requests/${provider.id}-request.json`,
    source: nodeId,
    status: "indexed",
  });
  state.tokenTotal += Math.round(provider.cost * 0.12);
  addEvent(status === "waiting_approval" ? "warn" : "info", `${provider.name} 任务已写入队列，状态：${statusLabel(status)}。`);
  showPanel("ops");
  saveState();
  renderAll();
  selectNode(nodeId);
}

async function createRuntimeProviderTask(provider) {
  const runtime = state.snapshot?.runtime ?? runtimeBaseUrl();
  if (!runtime) throw new Error("runtime endpoint missing");
  const risk = provider.cost >= 6000 || provider.status !== "ready";
  const result = await fetchJson(`${runtime}/api/tasks`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      project_slug: activeProjectSlug(),
      node_id: nodeIdForProvider(provider),
      title: `${provider.name} 生成任务`,
      provider_id: provider.id,
      cost_estimate_tokens: provider.cost,
      requires_approval: risk,
    }),
  });
  await mergeRuntimeMutationSnapshot(result.snapshot, runtime);
  return result.task;
}

async function runProviderTask(id) {
  const provider = providerById(id);
  if (!provider) return;
  if (state.snapshot?.mode !== "runtime-http") {
    await enqueueProviderTask(id);
    return;
  }

  try {
    const result = await runRuntimeProvider(provider);
    addEvent("ok", `${provider.name} Provider run 已写入 Runtime HTTP：${statusLabel(normalizeRuntimeStatus(result.report.status))}。`);
    showPanel("ops");
    saveState();
    renderAll();
    selectNode(result.task.node_id ?? state.selectedNode);
  } catch (error) {
    addEvent("warn", `Runtime HTTP Provider run 失败，已保留创建任务路径：${error.message}`);
    await enqueueProviderTask(id);
  }
}

async function runRuntimeProvider(provider) {
  const runtime = state.snapshot?.runtime ?? runtimeBaseUrl();
  if (!runtime) throw new Error("runtime endpoint missing");
  const projectSlug = activeProjectSlug();
  const requiresApproval = provider.cost >= 6000 && provider.status !== "ready";
  const endpoint = runtimeEndpointForProvider(provider);
  const result = await fetchJson(`${runtime}/api/provider-runs`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      project_slug: projectSlug,
      node_id: nodeIdForProvider(provider),
      task_title: `${provider.name} Provider run`,
      provider_id: provider.id,
      execution_mode: provider.group === "3dgs" ? "auto" : "adapter",
      ...(endpoint ? { endpoint } : {}),
      prompt: `${provider.name} generate content burst asset package`,
      input_paths: [`worlds/${projectSlug}/source/0-reference.png`],
      output_dir: `worlds/${projectSlug}/output`,
      cost_estimate_tokens: provider.cost,
      requires_approval: requiresApproval,
    }),
  });
  await mergeRuntimeMutationSnapshot(result.snapshot, runtime);
  return result;
}

async function runRuntimeWorkflow() {
  const runtime = state.snapshot?.runtime ?? runtimeBaseUrl();
  if (!runtime) throw new Error("runtime endpoint missing");
  const projectSlug = activeProjectSlug();
  const sourceInputs = state.assets
    .map((asset) => asset.path)
    .filter(Boolean)
    .slice(0, 8);
  const result = await fetchJson(`${runtime}/api/workflow-runs`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      project_slug: projectSlug,
      title: "Runtime local content burst",
      prompt: "run creative input to 3DGS to Unreal to video/game/interactive outputs",
      source_inputs: sourceInputs.length ? sourceInputs : [`worlds/${projectSlug}/source/0-reference.png`],
      duration_ms: 12000,
      three_dgs_mode: "auto",
      unreal_mode: "auto",
      agent_mode: "stage",
    }),
  });
  await mergeRuntimeMutationSnapshot(result.snapshot, runtime);
  applyWorkflowRunReport(result.report);
  return result;
}

async function createOutputPackage() {
  if (state.snapshot?.mode !== "runtime-http") {
    addEvent("warn", "生成输出包需要先连接 Runtime HTTP。");
    showPanel("outputs");
    renderAll();
    return;
  }

  try {
    const result = await runRuntimeOutputPackage();
    applyOutputPackageReport(result.report);
    addEvent("ok", `三类输出包已生成：${result.report.assets.length} 个 manifest 已入库。`);
    showPanel("ops");
    saveState();
    renderAll();
    selectNode(result.task.node_id ?? outputPackageNodeId());
  } catch (error) {
    addEvent("warn", `Runtime HTTP 输出包生成失败：${error.message}`);
    renderEvents();
  }
}

async function runRuntimeOutputPackage() {
  const runtime = state.snapshot?.runtime ?? runtimeBaseUrl();
  if (!runtime) throw new Error("runtime endpoint missing");
  const projectSlug = activeProjectSlug();
  const sourceAssets = state.assets
    .map((asset) => asset.path)
    .filter(Boolean)
    .slice(0, 16);
  const result = await fetchJson(`${runtime}/api/output-packages`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      project_slug: projectSlug,
      node_id: outputPackageNodeId(),
      title: "Runtime output package",
      source_assets: sourceAssets.length ? sourceAssets : [`worlds/${projectSlug}/output/1-world.glb`],
      duration_ms: 12000,
    }),
  });
  await mergeRuntimeMutationSnapshot(result.snapshot, runtime);
  return result;
}

async function recordOutputResult(target) {
  if (state.snapshot?.mode !== "runtime-http") {
    addEvent("warn", "回填输出执行结果需要先连接 Runtime HTTP。");
    showPanel("outputs");
    renderAll();
    return;
  }

  try {
    const manifest = (state.outputManifests ?? []).find((item) => item.target === target);
    if (!manifest) throw new Error(`output manifest missing for ${target}`);
    const result = await runRuntimeOutputResult(manifest);
    applyOutputPackageResultReport(result.report);
    addEvent("ok", `${manifest.title} 后段执行结果已回填。`);
    showPanel("outputs");
    saveState();
    renderAll();
    selectNode(result.task?.node_id ?? outputPackageNodeId());
  } catch (error) {
    addEvent("warn", `输出执行结果回填失败：${error.message}`);
    renderEvents();
  }
}

async function runRuntimeOutputResult(manifest) {
  const runtime = state.snapshot?.runtime ?? runtimeBaseUrl();
  if (!runtime) throw new Error("runtime endpoint missing");
  const projectSlug = activeProjectSlug();
  const result = await fetchJson(runtimeOutputPackageResultsUrl(runtime), {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      project_slug: projectSlug,
      node_id: outputPackageNodeId(),
      target: manifest.target,
      local_path: manifest.localPath || undefined,
      status: "succeeded",
      runtime: manifest.primaryRuntime || outputRuntimeForTarget(manifest.target),
      adapter_id: outputAdapterForTarget(manifest.target),
      message: `${manifest.title} 后段执行完成`,
      artifacts: manifest.localPath ? [manifest.localPath] : [],
      metrics: [{ label: "confirmed_by", value: "web-prototype" }],
      verification: {
        source: "web-prototype",
        confirmed_at: new Date().toISOString(),
      },
    }),
  });
  await mergeRuntimeMutationSnapshot(result.snapshot, runtime, {
    outputPackages: result.report?.catalog,
  });
  return result;
}

function outputPackageNodeId() {
  const explicit = state.nodes.find((node) => node.id === "outputs");
  if (explicit) return explicit.id;
  const matching = state.nodes.find((node) => /输出|output|video|game|interactive/i.test(`${node.type} ${node.title}`));
  return matching?.id ?? state.selectedNode;
}

async function createHandoffPackage() {
  if (state.snapshot?.mode !== "runtime-http") {
    addEvent("warn", "生成接管包需要先连接 Runtime HTTP。");
    showPanel("ops");
    renderAll();
    return;
  }

  try {
    const result = await runRuntimeHandoffPackage();
    state.runtimeHandoffPackage = normalizeRuntimeHandoffPackage(result);
    const fileCount = state.runtimeHandoffPackage.localFiles;
    const workerScript = state.runtimeHandoffPackage.workerSelfChecksPath;
    const manifestPath = state.runtimeHandoffPackage.manifestPath;
    addEvent(
      "ok",
      [
        `Runtime 接管包已生成：${fileCount} 个本地文件已入库`,
        manifestPath ? `manifest：${manifestPath}` : "",
        workerScript ? `worker smoke 脚本：${workerScript}` : "",
      ]
        .filter(Boolean)
        .join("，") + "。",
    );
    showPanel("ops");
    saveState();
    renderAll();
    selectNode(result.task?.node_id ?? "agent");
  } catch (error) {
    addEvent("warn", `Runtime 接管包生成失败：${error.message}`);
    renderEvents();
  }
}

async function runRuntimeHandoffPackage() {
  const runtime = state.snapshot?.runtime ?? runtimeBaseUrl();
  if (!runtime) throw new Error("runtime endpoint missing");
  const projectSlug = activeProjectSlug();
  const result = await fetchJson(runtimeHandoffPackageUrl(runtime), {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      project_slug: projectSlug,
      node_id: "agent",
      title: "Runtime handoff package",
      output_dir: `worlds/${projectSlug}/output`,
      include_snapshot: true,
    }),
  });
  await mergeRuntimeMutationSnapshot(result.snapshot, runtime);
  return result;
}

async function saveProviderApiKey(id) {
  const provider = providerById(id);
  if (!provider) return;
  const input = document.querySelector(`[data-provider-key-input="${id}"]`);
  const apiKey = input?.value?.trim() ?? "";
  if (!apiKey) {
    addEvent("warn", `${provider.name} API Key 为空，未写入。`);
    renderEvents();
    return;
  }
  if (state.snapshot?.mode !== "runtime-http") {
    addEvent("warn", `${provider.name} API Key 需要连接 Runtime HTTP 后写入。`);
    renderEvents();
    return;
  }

  try {
    const result = await createRuntimeApiKey(provider, apiKey);
    state.apiKeys = result.api_keys ?? result.snapshot?.api_keys ?? state.apiKeys;
    addEvent("ok", `${provider.name} API Key 已写入 Runtime HTTP：${result.api_key?.key_hint ?? "已保存"}。`);
    saveState();
    renderAll();
  } catch (error) {
    addEvent("warn", `${provider.name} API Key 写入失败：${error.message}`);
    renderEvents();
  }
}

async function createRuntimeApiKey(provider, apiKey) {
  const runtime = state.snapshot?.runtime ?? runtimeBaseUrl();
  if (!runtime) throw new Error("runtime endpoint missing");
  const result = await fetchJson(`${runtime}/api/api-keys`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      project_slug: activeProjectSlug(),
      provider_id: provider.id,
      service_type: "provider",
      api_key: apiKey,
      metadata: {
        auth: provider.auth,
        endpoint: provider.endpoint,
        group: provider.group,
      },
    }),
  });
  await mergeRuntimeMutationSnapshot(result.snapshot, runtime);
  return result;
}

async function stageSoftwareAction(name) {
  const software = state.software.find((item) => item.id === name || item.name === name);
  if (!software) return;

  if (state.snapshot?.mode === "runtime-http") {
    try {
      const result = await createRuntimeSoftwareAction(software);
      addEvent("ok", `${software.name} 动作已写入 Runtime HTTP：${statusLabel(normalizeRuntimeStatus(result.task.status))}。`);
      await refreshDesktopRecognitionRequests({ silent: true });
      showPanel("ops");
      saveState();
      renderAll();
      selectNode(result.task.node_id ?? "software");
      return;
    } catch (error) {
      addEvent("warn", `Runtime HTTP 软件动作失败，保留本地队列路径：${error.message}`);
    }
  }

  state.tasks.unshift({
    id: `task-${String(state.tasks.length + 1).padStart(3, "0")}`,
    nodeId: "software",
    title: `${software.name} 软件控制动作`,
    type: "software-control",
    status: software.health === "ready" ? "ready" : "waiting_approval",
    tool: software.mode,
    risk: software.health === "ready" ? "medium" : "high",
    cost: 1200,
  });
  addEvent("info", `${software.name} 软件控制动作已写入本地队列。`);
  showPanel("ops");
  saveState();
  renderAll();
  selectNode("software");
}

async function testSoftwareHealth(name) {
  const software = state.software.find((item) => item.id === name || item.name === name);
  if (!software) return;

  if (state.snapshot?.mode === "runtime-http") {
    software.health = "checking";
    software.lastHealth = "Runtime software health 检查中...";
    renderAll();
    try {
      const result = await createRuntimeSoftwareHealth(software);
      software.health = softwareStatusFromHealth(result);
      software.latency = result.adapter_mode ?? "runtime";
      software.lastHealth = `${result.adapter_mode ?? "adapter"} / ${result.health?.ok ? "ok" : "needs attention"}: ${result.health?.message ?? "no message"}`;
      addEvent(
        software.health === "ready" ? "ok" : "warn",
        `${software.name} Runtime health：${statusLabel(software.health)} / ${result.health?.message ?? "无详情"}`,
      );
      saveState();
      renderAll();
      return;
    } catch (error) {
      software.health = "checking";
      software.lastHealth = `Runtime software health failed: ${error.message}`;
      addEvent("warn", `${software.name} Runtime health 检查失败：${error.message}`);
      saveState();
      renderAll();
      return;
    }
  }

  software.health = software.health === "ready" ? "checking" : "ready";
  software.lastHealth = "本地模拟 software health 状态已切换。";
  addEvent("info", `${software.name} 软件 health 已在本地模拟中切换为 ${statusLabel(software.health)}。`);
  saveState();
  renderAll();
}

async function exportProviderConformancePackage(id) {
  const provider = providerById(id);
  if (!provider) return;
  if (state.snapshot?.mode !== "runtime-http") {
    addEvent("warn", "导出 Provider 验收包需要连接 Runtime HTTP。");
    renderEvents();
    return;
  }

  const runtime = state.snapshot?.runtime ?? runtimeBaseUrl();
  const providerId = canonicalProviderId(provider.id);
  try {
    const result = await fetchJson(runtimeProviderConformancePackagesUrl(runtime), {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        project_slug: activeProjectSlug(),
        node_id: nodeIdForProvider(provider),
        provider_id: providerId,
        title: `${provider.name} Provider 验收包`,
      }),
    });
    await mergeRuntimeMutationSnapshot(result.snapshot, runtime);
    const runner = result.report?.paths?.runner_script ?? result.report?.commands?.preflight ?? "provider conformance package";
    addEvent("ok", `${provider.name} Provider 验收包已导出：${runner}`);
    showPanel("ops");
    saveState();
    renderAll();
  } catch (error) {
    addEvent("warn", `${provider.name} Provider 验收包导出失败：${error.message}`);
    renderEvents();
  }
}

async function exportSoftwareConformancePackage(name) {
  const software = state.software.find((item) => item.id === name || item.name === name);
  if (!software) return;
  if (state.snapshot?.mode !== "runtime-http") {
    addEvent("warn", "导出软件验收包需要连接 Runtime HTTP。");
    renderEvents();
    return;
  }

  const runtime = state.snapshot?.runtime ?? runtimeBaseUrl();
  const adapterId = software.id ?? softwareAdapterId(software.name);
  try {
    const result = await fetchJson(runtimeSoftwareConformancePackagesUrl(runtime), {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        project_slug: activeProjectSlug(),
        node_id: nodeIdForSoftware(software),
        adapter_id: adapterId,
        title: `${software.name} 软件验收包`,
      }),
    });
    await mergeRuntimeMutationSnapshot(result.snapshot, runtime);
    const runner = result.report?.paths?.runner_script ?? result.report?.commands?.preflight ?? "software conformance package";
    addEvent("ok", `${software.name} 软件验收包已导出：${runner}`);
    showPanel("ops");
    saveState();
    renderAll();
  } catch (error) {
    addEvent("warn", `${software.name} 软件验收包导出失败：${error.message}`);
    renderEvents();
  }
}

async function exportAgentConformancePackage(kind = "all") {
  if (state.snapshot?.mode !== "runtime-http") {
    addEvent("warn", "导出 Agent/Hermes 验收包需要连接 Runtime HTTP。");
    renderEvents();
    return;
  }

  const runtime = state.snapshot?.runtime ?? runtimeBaseUrl();
  try {
    const result = await fetchJson(runtimeAgentConformancePackagesUrl(runtime), {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        project_slug: activeProjectSlug(),
        node_id: "agent",
        kind,
        title: "Agent/Hermes 验收包",
      }),
    });
    await mergeRuntimeMutationSnapshot(result.snapshot, runtime);
    const runner = result.report?.paths?.runner_script ?? result.report?.commands?.preflight ?? "agent conformance package";
    addEvent("ok", `Agent/Hermes 验收包已导出：${runner}`);
    showPanel("ops");
    saveState();
    renderAll();
  } catch (error) {
    addEvent("warn", `Agent/Hermes 验收包导出失败：${error.message}`);
    renderEvents();
  }
}

async function exportIntegrationConformancePackage() {
  if (state.snapshot?.mode !== "runtime-http") {
    addEvent("warn", "导出总验收包需要连接 Runtime HTTP。");
    renderEvents();
    return;
  }

  const runtime = state.snapshot?.runtime ?? runtimeBaseUrl();
  try {
    const result = await fetchJson(runtimeIntegrationConformancePackagesUrl(runtime), {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        project_slug: activeProjectSlug(),
        node_id: "agent",
        title: "AI/3DGS/软件/Agent 总验收包",
        agent_kind: "all",
      }),
    });
    await mergeRuntimeMutationSnapshot(result.snapshot, runtime);
    const summary = result.report?.summary ?? {};
    const runner = result.report?.paths?.runner_script ?? result.report?.commands?.preflight ?? "integration conformance package";
    addEvent(
      "ok",
      `总验收包已导出：${summary.providers ?? 0} Provider / ${summary.software_adapters ?? 0} 软件 / ${summary.agent ? "Agent" : "无 Agent"}，${runner}`,
    );
    showPanel("ops");
    saveState();
    renderAll();
  } catch (error) {
    addEvent("warn", `总验收包导出失败：${error.message}`);
    renderEvents();
  }
}

async function refreshDesktopRecognitionRequests(options = {}) {
  const runtime = state.snapshot?.runtime ?? runtimeBaseUrl();
  if (!runtime || state.snapshot?.mode !== "runtime-http") {
    if (!options.silent) addEvent("warn", "桌面识别队列需要连接 Runtime HTTP。");
    renderDesktopRecognitionQueue();
    return [];
  }
  try {
    const result = await fetchJson(runtimeDesktopRecognitionRequestsUrl(runtime));
    state.desktopRecognitionRequests = (result.requests ?? []).map(runtimeDesktopRecognitionRequest);
    if (!options.silent) addEvent("ok", `桌面识别队列已刷新：${state.desktopRecognitionRequests.length} 个等待项。`);
    saveState();
    renderDesktopRecognitionQueue();
    renderMetrics();
    return state.desktopRecognitionRequests;
  } catch (error) {
    if (!options.silent) addEvent("warn", `桌面识别队列刷新失败：${error.message}`);
    renderDesktopRecognitionQueue();
    return state.desktopRecognitionRequests ?? [];
  }
}

async function completeDesktopRecognitionRequest(actionId, status) {
  const runtime = state.snapshot?.runtime ?? runtimeBaseUrl();
  if (!runtime || state.snapshot?.mode !== "runtime-http") {
    addEvent("warn", "桌面识别结果回填需要连接 Runtime HTTP。");
    return;
  }
  const request = (state.desktopRecognitionRequests ?? []).find((item) => item.id === actionId);
  if (!request) return;
  try {
    const result = await fetchJson(runtimeDesktopRecognitionResultsUrl(runtime), {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        software_action_id: request.id,
        task_id: request.taskId,
        status,
        message: `Web 控制台回填桌面识别结果：${statusLabel(normalizeRuntimeStatus(status))}`,
        artifacts: request.requestPath ? [request.requestPath] : [],
        result: {
          controller: "web-prototype",
          target_window: request.targetWindow,
          desktop_tool: request.desktopTool,
        },
      }),
    });
    await mergeRuntimeMutationSnapshot(result.snapshot, runtime);
    await refreshDesktopRecognitionRequests({ silent: true });
    addEvent("ok", `${request.targetWindow || request.adapterId} 桌面识别结果已回填：${statusLabel(normalizeRuntimeStatus(status))}。`);
    saveState();
    renderAll();
  } catch (error) {
    addEvent("warn", `桌面识别结果回填失败：${error.message}`);
    renderEvents();
  }
}

async function runNextDesktopRecognitionRequest() {
  const runtime = state.snapshot?.runtime ?? runtimeBaseUrl();
  if (!runtime || state.snapshot?.mode !== "runtime-http") {
    addEvent("warn", "桌面识别 dry-run 推进需要连接 Runtime HTTP。");
    renderEvents();
    return;
  }
  const nextRequest = state.desktopRecognitionRequests?.[0];
  if (!nextRequest) {
    addEvent("info", "桌面识别队列为空，无需推进。");
    renderEvents();
    return;
  }
  try {
    const result = await fetchJson(runtimeDesktopRecognitionRunNextUrl(runtime), {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        controller_id: "web-prototype-dry-run",
        status: "succeeded",
        limit: 1,
        message: `Web 控制台 dry-run 推进：${nextRequest.targetWindow || nextRequest.adapterId}`,
      }),
    });
    const callbackSnapshot = result.callbacks?.[0]?.response?.snapshot;
    if (callbackSnapshot) {
      await mergeRuntimeMutationSnapshot(callbackSnapshot, runtime);
    }
    await refreshDesktopRecognitionRequests({ silent: true });
    addEvent("ok", `桌面识别 dry-run 已推进：${result.processed_count ?? 0}/${result.queued_count ?? 0}。`);
    saveState();
    renderAll();
  } catch (error) {
    addEvent("warn", `桌面识别 dry-run 推进失败：${error.message}`);
    renderEvents();
  }
}

async function createRuntimeSoftwareAction(software) {
  const runtime = state.snapshot?.runtime ?? runtimeBaseUrl();
  if (!runtime) throw new Error("runtime endpoint missing");
  const adapterId = software.id ?? softwareAdapterId(software.name);
  const endpoint = software.runtimeEndpoint?.trim();
  const priority = softwareControlPriority(software);
  const desktopRecognitionPayload = priority === "DesktopRecognition" ? {
    instruction: `Use desktop recognition to control ${software.name}: ${software.scope}`,
    target_window: software.name,
    visual_targets: [software.name, software.priority, software.scope],
  } : {};
  const result = await fetchJson(`${runtime}/api/software-actions`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      project_slug: activeProjectSlug(),
      node_id: nodeIdForSoftware(software),
      task_title: `${software.name} 软件控制动作`,
      adapter_id: adapterId,
      action_kind: softwareActionKind(adapterId),
      priority,
      payload_json: {
        software: software.name,
        scope: software.scope,
        ...(endpoint ? { endpoint } : {}),
        ...desktopRecognitionPayload,
        requested_from: "web-prototype",
      },
      requires_confirmation: software.health !== "ready" && priority !== "DesktopRecognition",
    }),
  });
  await mergeRuntimeMutationSnapshot(result.snapshot, runtime);
  return result;
}

async function createRuntimeSoftwareHealth(software) {
  const runtime = state.snapshot?.runtime ?? runtimeBaseUrl();
  if (!runtime) throw new Error("runtime endpoint missing");
  const adapterId = software.id ?? softwareAdapterId(software.name);
  const endpoint = software.runtimeEndpoint?.trim();
  const priority = softwareControlPriority(software);
  return fetchJson(`${runtime}/api/software-health`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      adapter_id: adapterId,
      priority,
      payload_json: {
        software: software.name,
        scope: software.scope,
        ...(endpoint ? { endpoint } : {}),
        requested_from: "web-prototype",
      },
    }),
  });
}

function softwareStatusFromHealth(result) {
  if (result?.health?.ok) return "ready";
  if (result?.adapter_mode === "human_takeover") return "guarded";
  return "checking";
}

function softwareAdapterId(name) {
  return {
    "DaVinci Resolve": "resolve",
    "剪辑软件": "editing-suite",
    "TouchDesigner": "touchdesigner",
    "MadMapper": "madmapper",
    "动捕数据库": "motion-db",
  }[name] ?? name.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
}

function softwareActionKind(adapterId) {
  return {
    unreal: "CreateScene",
    blender: "ImportAsset",
    comfyui: "HealthCheck",
    resolve: "Render",
    unity: "ExportBuild",
    touchdesigner: "RunViewport",
    madmapper: "RunViewport",
    nuke: "Render",
    "motion-db": "ImportAsset",
    "editing-suite": "Transcode",
    hermes: "ExecuteCli",
  }[adapterId] ?? "HealthCheck";
}

function softwareControlPriority(software) {
  const mode = software.mode.toLowerCase();
  if (mode.includes("mcp") || mode.includes("api")) return "ApiMcp";
  if (mode.includes("cli") || mode.includes("python")) return "SkillsCli";
  if (mode.includes("desktop") || mode.includes("桌面识别")) return "DesktopRecognition";
  return "HumanTakeover";
}

function nodeIdForSoftware(software) {
  if (state.snapshot?.mode === "runtime-http") {
    const adapterId = software.id ?? softwareAdapterId(software.name);
    const matching = state.nodes.find((node) => normalizeProviderId(node.agent) === normalizeProviderId(adapterId));
    if (matching) return matching.id;
    const softwareNode = state.nodes.find((node) => /software|软件|control/i.test(`${node.type} ${node.title}`));
    if (softwareNode) return softwareNode.id;
  }
  return "software";
}

function nodeIdForProvider(provider) {
  if (state.snapshot?.mode === "runtime-http") {
    const matchingAgent = state.nodes.find((node) => normalizeProviderId(node.agent) === normalizeProviderId(provider.id));
    if (matchingAgent) return matchingAgent.id;
    const fallback = provider.group === "3dgs"
      ? state.nodes.find((node) => /3dgs|ThreeDgs/i.test(`${node.type} ${node.title}`))
      : state.nodes.find((node) => /ai|图片|image/i.test(`${node.type} ${node.title}`));
    if (fallback) return fallback.id;
  }
  return provider.group === "3dgs" ? "image3d" : "brief";
}

function normalizeProviderId(value) {
  return String(value ?? "").replace(/[^a-z0-9]/gi, "").toLowerCase();
}

function selectHermesRunbook(name) {
  state.hermes.selectedRunbook = name;
  state.hermes.runbookTarget = defaultRunbookTarget(name, state.hermes.runbookTarget);
  state.hermes.runbookPreview = "";
  saveState();
  renderHermes();
}

function defaultRunbookTarget(name, current = "") {
  if (current && current !== "<software-action-id>" && current !== "<3dgs-node-id>") return current;
  if (name === "pool_3dgs_conversion_review") {
    return state.nodes.find((node) => node.taskType === "3dgs")?.id ?? "image3d";
  }
  if (name === "pool_desktop_takeover") return state.desktopRecognitionRequests[0]?.softwareActionId ?? "<software-action-id>";
  if (name === "pool_content_burst_runbook") return "video, game, interactive_art";
  return "blender";
}

function hermesRunbookArguments(name) {
  const target = state.hermes.runbookTarget.trim();
  const projectSlug = activeProjectSlug();
  const workflowId = currentRuntimeWorkflowId() || "<workflow-id>";
  if (name === "pool_content_burst_runbook") {
    return {
      project_slug: projectSlug,
      workflow_id: workflowId,
      creative_brief: state.hermes.lastCommand,
      source_inputs: `worlds/${projectSlug}/source/0-reference.png`,
      output_targets: target || "video, game, interactive_art",
    };
  }
  if (name === "pool_3dgs_conversion_review") {
    const node = state.nodes.find((item) => item.id === target) ?? state.nodes.find((item) => item.taskType === "3dgs");
    return {
      project_slug: projectSlug,
      workflow_id: workflowId,
      node_id: node?.id || target || "image3d",
      provider_id: node?.agent && node.agent !== "runtime" ? node.agent : "worldlabs-marble",
    };
  }
  if (name === "pool_desktop_takeover") {
    return {
      project_slug: projectSlug,
      software_action_id: target || state.desktopRecognitionRequests[0]?.softwareActionId || "<software-action-id>",
      target_window: state.desktopRecognitionRequests[0]?.targetWindow || "TouchDesigner",
    };
  }
  return {
    project_slug: projectSlug,
    adapter_id: target || "blender",
    action_kind: target === "unreal" ? "CreateScene" : "ExecuteCli",
  };
}

async function applyHermesRunbook() {
  const runbook = selectedHermesRunbook();
  if (!runbook) return;
  state.hermes.runbookTarget = document.querySelector("#hermesRunbookTarget").value.trim() || defaultRunbookTarget(runbook.name);
  const args = hermesRunbookArguments(runbook.name);
  const runtime = state.snapshot?.runtime ?? runtimeBaseUrl();

  try {
    if (!runtime || state.snapshot?.mode !== "runtime-http") throw new Error("runtime prompts unavailable");
    const prompt = await fetchJson(runtimePromptUrl(runtime, runbook.name, args));
    const text = prompt?.messages?.[0]?.content?.text;
    if (!text) throw new Error("empty runbook prompt");
    state.hermes.lastCommand = text;
    state.hermes.runbookPreview = text;
    state.hermes.trace.unshift(`${nowTime()} -> runbook: ${runbook.title}`);
    addEvent("ok", `Agent Runbook 已加载：${runbook.title}`);
  } catch (error) {
    const text = localHermesRunbookText(runbook, args);
    state.hermes.lastCommand = text;
    state.hermes.runbookPreview = text;
    state.hermes.trace.unshift(`${nowTime()} -> local runbook: ${runbook.title}`);
    addEvent("warn", `Runtime Runbook 读取失败，已使用本地模板：${error.message}`);
  }

  saveState();
  renderAll();
}

function localHermesRunbookText(runbook, args) {
  if (runbook.name === "pool_3dgs_conversion_review") {
    return `Review Pool 2D/3DGS conversion readiness.\nProject: ${args.project_slug}\nWorkflow: ${args.workflow_id}\nNode: ${args.node_id}\nPreferred provider: ${args.provider_id}\n\nRequired checks:\n1. Read pool://runtime-graph, pool://workflow/${args.workflow_id}, and pool://node-context/${args.node_id}.\n2. Verify provider_requests, local assets, and approval gates.\n3. Keep provider URLs as provenance and report local files as source of truth.`;
  }
  if (runbook.name === "pool_content_burst_runbook") {
    return `You are controlling Pool as a local-first content production OS.\nProject: ${args.project_slug}\nWorkflow: ${args.workflow_id}\nCreative brief: ${args.creative_brief}\nSource inputs: ${args.source_inputs}\nOutput targets: ${args.output_targets}\n\nRequired sequence:\n1. Read pool://status, pool://runtime-graph, pool://workflow/${args.workflow_id}, and pool://tasks.\n2. Inspect Agent, 3DGS, Unreal, and output nodes.\n3. Use pool_run_workflow for the full chain and respect approval gates.`;
  }
  if (runbook.name === "pool_desktop_takeover") {
    return `Operate Pool desktop recognition handoff.\nProject: ${args.project_slug}\nSoftware action: ${args.software_action_id}\nTarget window: ${args.target_window}\n\nRequired sequence:\n1. Read pool_desktop_requests.\n2. Capture result evidence.\n3. Return status through pool_desktop_result.`;
  }
  return `Prepare a Pool external software handoff.\nProject: ${args.project_slug}\nAdapter: ${args.adapter_id}\nAction kind: ${args.action_kind}\n\nRequired sequence:\n1. Call pool_software_health for ${args.adapter_id}.\n2. Use API/MCP > Skills/CLI > Desktop Recognition > Human Takeover.\n3. Use pool_run_software with clear payload_json and confirmation for risky actions.`;
}

async function sendHermesCommand() {
  state.hermes.endpoint = document.querySelector("#hermesEndpoint").value.trim() || state.hermes.endpoint;
  state.hermes.lastCommand = document.querySelector("#hermesPrompt").value.trim();
  if (state.snapshot?.mode === "runtime-http") {
    try {
      const result = await createRuntimeHermesSession(false, true);
      state.hermes.status = normalizeRuntimeStatus(result.task.status);
      state.hermes.trace.unshift(`${nowTime()} -> Runtime Hermes: ${state.hermes.lastCommand}`);
      addEvent("ok", `Hermes HTTP 指令已写入并执行：${statusLabel(normalizeRuntimeStatus(result.task.status))}。`);
      saveState();
      renderAll();
      return;
    } catch (error) {
      addEvent("warn", `Runtime HTTP Hermes 写入失败，已切回本地模拟：${error.message}`);
    }
  }
  state.hermes.status = "running";
  state.hermes.trace.unshift(`${nowTime()} -> Hermes: ${state.hermes.lastCommand}`);
  state.tokenTotal += 900;
  addEvent("info", "Hermes 内嵌控制指令已发送到模拟通道。");
  saveState();
  renderAll();
}

async function stageHermesTask() {
  state.hermes.endpoint = document.querySelector("#hermesEndpoint").value.trim() || state.hermes.endpoint;
  state.hermes.lastCommand = document.querySelector("#hermesPrompt").value.trim();
  if (state.snapshot?.mode === "runtime-http") {
    try {
      const result = await createRuntimeHermesSession(false);
      state.hermes.status = normalizeRuntimeStatus(result.task.status);
      state.hermes.trace.unshift(`${nowTime()} -> Runtime queued: ${state.hermes.lastCommand}`);
      addEvent("ok", `Hermes 控制任务已写入 Runtime HTTP：${statusLabel(normalizeRuntimeStatus(result.task.status))}。`);
      showPanel("ops");
      saveState();
      renderAll();
      selectNode("software");
      return;
    } catch (error) {
      addEvent("warn", `Runtime HTTP Hermes 任务写入失败，已切回本地队列：${error.message}`);
    }
  }
  state.tasks.unshift({
    id: `task-${String(state.tasks.length + 1).padStart(3, "0")}`,
    nodeId: "software",
    title: "Hermes 内嵌控制任务",
    type: "hermes-control",
    status: "ready",
    tool: state.hermes.endpoint,
    risk: "medium",
    cost: 1500,
  });
  state.hermes.trace.unshift(`${nowTime()} -> queued: ${state.hermes.lastCommand}`);
  addEvent("ok", "Hermes 控制任务已写入运行中心。");
  showPanel("ops");
  saveState();
  renderAll();
  selectNode("software");
}

async function createRuntimeHermesSession(requiresConfirmation, execute = false) {
  const runtime = state.snapshot?.runtime ?? runtimeBaseUrl();
  if (!runtime) throw new Error("runtime endpoint missing");
  const result = await fetchJson(`${runtime}/api/agent-sessions`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      kind: "hermes",
      project_slug: activeProjectSlug(),
      endpoint: state.hermes.endpoint,
      instruction: state.hermes.lastCommand,
      allowed_tools: ["api", "mcp", "skills", "cli", "desktop"],
      requires_confirmation: requiresConfirmation,
      execute,
      timeout_ms: execute ? 30000 : undefined,
    }),
  });
  await mergeRuntimeMutationSnapshot(result.snapshot, runtime);
  return result;
}

async function copyCliCommand(id) {
  const item = state.cliCommands.find((command) => command.id === id);
  if (!item) return;
  const command = cliCommandText(item);

  try {
    await navigator.clipboard.writeText(command);
    addEvent("ok", `已复制 CLI：${item.title}`);
  } catch {
    addEvent("warn", `浏览器未允许剪贴板访问，请手动复制：${command}`);
  }
  saveState();
  renderAll();
}

async function stageCliCommand(id) {
  const item = state.cliCommands.find((command) => command.id === id);
  if (!item) return;

  if (state.snapshot?.mode === "runtime-http") {
    try {
      const result = await createRuntimeCliSession(item);
      addEvent("ok", `Agent CLI 会话已写入 Runtime HTTP：${item.title} / ${statusLabel(normalizeRuntimeStatus(result.task.status))}。`);
      showPanel("ops");
      saveState();
      renderAll();
      selectNode(item.id === "hermes-control" ? "software" : "agent");
      return;
    } catch (error) {
      addEvent("warn", `Runtime HTTP Agent CLI 写入失败，已切回本地队列：${error.message}`);
    }
  }

  state.tasks.unshift({
    id: `task-${String(state.tasks.length + 1).padStart(3, "0")}`,
    nodeId: item.id === "hermes-control" ? "software" : "agent",
    title: item.title,
    type: "agent-cli",
    status: "ready",
    tool: cliCommandText(item),
    risk: item.id === "run-provider" ? "high" : "medium",
    cost: item.id === "run-provider" ? 4200 : 1100,
  });
  addEvent("info", `Agent CLI 命令已写入队列：${item.title}`);
  showPanel("ops");
  saveState();
  renderAll();
}

async function createRuntimeCliSession(item) {
  const runtime = state.snapshot?.runtime ?? runtimeBaseUrl();
  if (!runtime) throw new Error("runtime endpoint missing");
  const result = await fetchJson(`${runtime}/api/agent-sessions`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      kind: "agent_cli",
      project_slug: activeProjectSlug(),
      command_id: item.id,
      title: item.title,
      command: cliCommandText(item),
      tools: toolsForCliCommand(item),
      token_budget: tokenBudgetForCliCommand(item),
    }),
  });
  await mergeRuntimeMutationSnapshot(result.snapshot, runtime);
  return result;
}

async function loadAgentTranscript(sessionId) {
  const runtime = state.snapshot?.runtime ?? runtimeBaseUrl();
  if (!runtime || !sessionId) return;
  try {
    const transcript = await fetchJson(runtimeAgentTranscriptUrl(runtime, sessionId));
    state.hermes.sessionTranscripts = {
      ...(state.hermes.sessionTranscripts ?? {}),
      [sessionId]: transcript,
    };
    addEvent("ok", `Agent transcript 已读取：${sessionId}`);
    saveState();
    renderAll();
    startAgentSessionStream(sessionId);
  } catch (error) {
    addEvent("warn", `Agent transcript 读取失败：${error.message}`);
    renderEvents();
  }
}

function toolsForCliCommand(item) {
  if (item.id === "workflow-context") return ["sqlite", "mcp", "cli"];
  if (item.id === "run-provider") return ["api", "mcp", "provider", "cli"];
  if (item.id === "hermes-control") return ["api", "mcp", "hermes", "cli"];
  return ["api", "mcp", "skills", "cli", "desktop"];
}

function tokenBudgetForCliCommand(item) {
  if (item.id === "agent-cli") return 74000;
  if (item.id === "run-provider") return 9000;
  return 4000;
}

function cliCommandText(item) {
  const projectSlug = activeProjectSlug();
  const projectArgs = projectSlug ? ` --project ${cliArg(projectSlug)}` : "";
  const prefix = `pool-cli${projectArgs}`;
  const workflowId = currentRuntimeWorkflowId();
  const workflowArg = workflowId ? ` ${cliArg(workflowId)}` : "";
  const workflowCommand = `${prefix} workflow-context${workflowArg}`;

  if (item.id === "workflow-context") return workflowCommand;
  if (item.id === "run-provider") {
    return `${prefix} run-provider world-labs-marble --execution-mode mock --no-approval --prompt ${cliArg("Agent CLI 3DGS smoke")}`;
  }
  if (item.id === "hermes-control") {
    const instruction = workflowId
      ? `inspect workflow ${workflowId} and coordinate Unreal handoff`
      : "inspect workflow context and coordinate Unreal handoff";
    return `${prefix} agent-session hermes --instruction ${cliArg(instruction)} --allowed-tool api --allowed-tool mcp --allowed-tool unreal`;
  }
  if (item.id === "agent-cli") {
    return `${prefix} agent-session agent-cli --command-id workflow-context --title ${cliArg("Inspect workflow context")} --command ${cliArg(workflowCommand)} --tool cli --tool mcp --token-budget 74000`;
  }
  return item.command;
}

function cliArg(value) {
  const text = String(value ?? "");
  return /^[A-Za-z0-9_./:@=*-]+$/.test(text) ? text : JSON.stringify(text);
}

navButtons.forEach((button) => {
  button.addEventListener("click", () => showPanel(button.dataset.panel));
});

document.querySelector("#simulateRun").addEventListener("click", simulateRun);
document.querySelector("#runSelectedNode").addEventListener("click", runSelectedNode);
document.querySelector("#approvalGate").addEventListener("click", approveGate);
document.querySelector("#outputPackage").addEventListener("click", createOutputPackage);
document.querySelector("#runtimeHandoffPackage").addEventListener("click", createHandoffPackage);
document.querySelector("#adapterHealthSweep").addEventListener("click", runAdapterHealthSweep);
document.querySelector("#integrationConformancePackage").addEventListener("click", exportIntegrationConformancePackage);
document.querySelector("#loadProductionEvidenceTemplate").addEventListener("click", loadProductionEvidenceTemplate);
document.querySelector("#loadProductionEvidenceItemTemplate").addEventListener("click", loadProductionEvidenceItemTemplate);
document.querySelector("#claimProductionEvidenceTask").addEventListener("click", claimProductionEvidenceTask);
document.querySelector("#loadProductionEvidenceLedgerBundle").addEventListener("click", loadProductionEvidenceLedgerBundle);
document.querySelector("#createProductionEvidenceHandoffPackage").addEventListener("click", createProductionEvidenceHandoffPackage);
document.querySelector("#createProductionEvidenceRunPlan").addEventListener("click", createProductionEvidenceRunPlan);
document.querySelector("#loadProductionEvidenceExample").addEventListener("click", fillProductionEvidenceExample);
document.querySelector("#mergeProductionEvidence").addEventListener("click", mergeProductionEvidence);
document.querySelector("#closeoutProductionEvidence").addEventListener("click", () => closeoutProductionEvidence(false));
document.querySelector("#validateProductionEvidence").addEventListener("click", validateProductionEvidence);
document.querySelector("#validateProductionEvidenceItem").addEventListener("click", validateProductionEvidenceItem);
document.querySelector("#submitProductionEvidenceItem").addEventListener("click", submitProductionEvidenceItem);
document.querySelector("#closeoutImportProductionEvidence").addEventListener("click", () => closeoutProductionEvidence(true));
document.querySelector("#importProductionEvidence").addEventListener("click", importProductionEvidence);
document.querySelector("#refreshDesktopQueue").addEventListener("click", () => refreshDesktopRecognitionRequests());
document.querySelector("#runDesktopQueue").addEventListener("click", runNextDesktopRecognitionRequest);
document.querySelector("#exportState").addEventListener("click", exportState);
document.querySelector("#resetState").addEventListener("click", resetState);
document.querySelector("#sendHermesCommand").addEventListener("click", sendHermesCommand);
document.querySelector("#stageHermesTask").addEventListener("click", stageHermesTask);
document.querySelector("#agentConformancePackage").addEventListener("click", () => exportAgentConformancePackage("all"));
document.querySelector("#hermesRunbookSelect").addEventListener("change", (event) => {
  selectHermesRunbook(event.target.value);
});
document.querySelector("#hermesRunbookTarget").addEventListener("input", (event) => {
  state.hermes.runbookTarget = event.target.value;
  saveState();
});
document.querySelector("#applyHermesRunbook").addEventListener("click", applyHermesRunbook);
document.querySelector("#projectSelector").addEventListener("change", (event) => {
  changeRuntimeProject(event.target.value);
});

async function boot() {
  await applyRuntimeSnapshotIfAvailable();
  renderAll();
  selectNode(state.selectedNode);
}

boot();
