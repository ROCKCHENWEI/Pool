# Pool 内容爆发工具与运行框架

`pool` 当前从静态中文原型升级为本地优先的运行框架 scaffold。实现方向以 `ROCKCHENWEI/Pool` 的 Rust shared-core、Timeline、Node Engine、SQLite、Provider Gateway、OpenClaw/MCP 为主基线，同时吸收 `neilsonnn/image-blaster` 的本地资产包、indexed files、Provider provenance 和高成本步骤人工确认机制。

## 当前实现

- `shared-core/`：Rust 运行核心骨架，承接 ROCKCHENWEI/Pool 的长期主运行层。
- `pool-cli/`：本地 runtime 命令行入口，复用 `RuntimeHttpServer` 读取 snapshot、runtime graph、runtime budget、runtime preflight、runtime handoff、PRD readiness、node context、MCP resource、Provider gateway worker 合同、Unreal MCP bridge 合同、脱敏 API key 状态，并可触发 Provider health/run、软件 health/action、输出包、接管包、Hermes/Agent 会话、桌面识别接管、节点级执行和完整 workflow，也可启动 `provider-gateway-worker`、`unreal-mcp-bridge-worker`、`hermes-mcp-bridge-worker` 和 `software-api-bridge-worker` 本地 HTTP worker；`worker-self-checks` 会一次性运行 Provider gateway、SDK worker、Unreal、Hermes 和通用软件 bridge 自检并输出 JSON；`serve-mcp` 可把同一组 `pool://` resources、`pool_handoff_package` 接管包写入工具、`pool_worker_self_checks` 本地 smoke tool 和 Agent runbook prompts 暴露为本地 MCP stdio 服务，供 Hermes/Agent CLI/脚本使用。
- `apps/web-prototype/`：迁移后的中文 Web 控制台原型，用作未来 Web/SwiftUI 运行面板设计源。
- `Workflow/Node/Connection`：节点图不再只是展示，而是可执行计划模型，包含资产流、控制流、Agent 指令、反馈循环和审批连接。
- `Project/Shot/Segment`：P0 Timeline 抽象已进入 Rust core，可统一承载视频镜头、游戏关卡片段和交互艺术 cue。
- `NodeEngine`：实现拓扑排序、缺失节点校验和循环检测。
- `PoolRuntimePlan`：提供默认“创意输入 → Agent → AI 图片 → 3DGS 审批 → 本地资产包 → Unreal → 视频/游戏/交互艺术输出”的闭环蓝图。
- `TaskQueue`：实现 `queued / ready / running / waiting_approval` 等任务状态，支持高成本 Provider、软件控制动作和带 `execute:true` 的 Agent session 审批后继续执行，并支持失败/取消任务按 Provider/software/Agent transcript 账本重试。
- `ProviderTaskRunner`：统一执行 Provider 任务，串联成本估算、`provider_requests` 请求账本、审批状态、waiting approval 本地 request handoff、health、submit、poll、download、assets 入库和事件写入。
- `ProviderTaskRunner` progress：ComfyUI WebSocket progress 可通过 runner 写入 `workflow_events`。
- `OutputPackageRunner`：把 Unreal/Resolve/TouchDesigner 等后段输出收束为本地优先的三类交付清单，生成 indexed `1-video-timeline.json`、`2-game-build.json`、`3-interactive-cues.json` 并同步写入 assets、tasks 和 events。
- `RuntimeHandoffPackageRunner`：把 `/api/runtime-handoff`、运行前检查、runtime graph、integration readiness、带 operator checklist 的 manifest 和可选 snapshot 落地为 image-blaster 风格 indexed handoff 文件包，供 Hermes、Agent CLI、桌面 controller 或人工 operator 离线接管。
- `Runtime execution plan run-next`：`POST /api/runtime-execution-plan/run-next` 把只读 execution plan 提升为受控调度入口；默认只 preview 下一步或指定 node/task，显式 `execute:true` 才会复用节点运行、任务审批或任务重试，审批还必须显式 `allow_approval:true`。
- `ContentBurstRunner`：把默认运行蓝图、本地项目包、Agent/Hermes 决策、3DGS、Unreal 和三类输出包串成一键本地闭环；`agent_mode:"stage"` 默认写入 Hermes 决策会话，`auto` 模式优先使用已配置 3DGS gateway / Unreal MCP，缺省或失败时回落本地 mock。
- `ProviderAdapter`：统一 AI 图片/视频、音频、3DGS、软件与 Agent Provider 的 `health / submit / poll / download / verify / estimate_cost` 接口。
- `ComfyUiProvider`：实现 ComfyUI HTTP/WebSocket adapter，支持 `/system_stats` health、`/prompt` submit、`/history/{prompt_id}` poll、`/ws` progress event、`/view` 本地下载。
- `KlingProvider`：实现 Kling AI 视频 adapter 骨架，支持 Bearer API Key 或 Access/Secret JWT、text2video/image2video submit、task poll、结果 URL 下载到本地。
- `OpenAiImageProvider`：实现 OpenAI Images API adapter 骨架，支持 `gpt-image-2` 默认模型、Bearer API Key、base64/URL 输出落地和 assets 入库。
- `GenericHttpMediaProvider`：实现 Midjourney、Nano Banana Pro、Suno 等媒体服务的通用 HTTP gateway adapter，支持 media profile mapping、submit/poll/download、URL/base64 输出落地和 assets 入库。
- `Generic media production evidence`：`generic_media_smoke` 在真实 gateway run 成功、提供 `POOL_PROVIDER_PRODUCTION_ATTESTATION` 时，可直接写出可校验/导入的 `providers[]` production evidence bundle，减少外部 AI 图片/音频 worker 手工拼接证据。
- `Provider gateway mock server`：提供本地 AI media + 3DGS gateway 契约服务器，可在没有真实厂商 SDK/账号时验证 submit/poll/download、本地文件落地和 assets 入库。
- `Provider evidence matrix`：`pool-cli production-evidence-provider-matrix` 会通过 `/api/provider-runs` 依次执行 Midjourney、OpenAI image-2、Nano Banana Pro、Suno、World Labs Marble、TripoSplat、SAM-3D、Spark 和群核科技的 evidence profile，写入 `provider_requests.evidence`；media/3DGS 走 gateway，OpenAI image-2 走 native adapter，并支持 `--provider-endpoint provider=url` 与 `--provider-api-key provider=token` 把单个厂商证据任务指向不同真实 SDK/HTTP worker 和 bearer token，在 PRD readiness 中区分普通接入证据与真实上游生产证据。
- `Provider gateway template`：提供真实 gateway/SDK worker 的上游翻译模板，把 Pool profile request 转成厂商 SDK/HTTP worker 所需的 endpoint、auth、request body 和回填 response contract。
- `Provider gateway worker`：提供本地 HTTP forwarder，把 Pool media/3DGS gateway request 通过翻译模板转发到真实厂商 worker、官方 SDK 包装服务或本地 mock upstream，并把上游 job/status/outputs 规范化回 Pool contract；支持 `--provider-upstream provider=url` 和 per-provider bearer key，把 Midjourney、Nano Banana Pro、Suno 与 3DGS 厂商包装服务路由到不同上游；`/api/provider-gateway-worker` 与 `pool://provider-gateway-worker` 会暴露机器可读启动、上游合同和 `conformance_runbook` 真实接入验收步骤。
- `Provider conformance package`：`POST /api/provider-conformance-packages`、`pool-cli provider-conformance-package <provider-id>` 和 MCP tool `pool_provider_conformance_package` 会把单个 AI/3DGS Provider 的 contract、gateway worker contract、runbook、preflight、runner script 和 manifest 写到本地 `control/provider-conformance/<provider-id>/`，用于真实厂商 SDK/HTTP worker 接入验收。
- `Agent/Hermes conformance package`：`POST /api/agent-conformance-packages`、`pool-cli agent-conformance-package [all|hermes|agent-cli]` 和 MCP tool `pool_agent_conformance_package` 会把 Agent session 控制合同、Hermes/Agent CLI runbook、preflight、runner script 和 manifest 写到本地 `control/agent-conformance/<kind>/`，用于验收 Hermes 内嵌控制与 Agent CLI 受控执行。
- `Integration conformance package`：`POST /api/integration-conformance-packages`、`pool-cli integration-conformance-package` 和 MCP tool `pool_integration_conformance_package` 会一次性写出 AI/3DGS Provider、外部软件 adapter 与 Agent/Hermes 的总验收包，默认覆盖 9 个 required Provider、11 个软件 adapter 和 `all` Agent/Hermes，落地到 `control/integration-conformance/`。
- `Integration readiness matrix`：`GET /api/integration-readiness`、`pool://integration-readiness`、`pool-cli integration-readiness` 和 MCP tool `pool_integration_readiness` 会从同一份 snapshot 汇总 Provider、外部软件 adapter 与 Agent/Hermes 的接入状态，区分 `ready / needs_configuration / needs_execution / needs_attention`，并输出 5 人团队 lane、next action 和按优先级排序的 run plan。
- `Provider SDK worker template`：提供真实厂商 SDK wrapper 的最小可运行样板，接收 `provider_gateway_worker` 转发后的 `upstream.request_body`，校验 `local_input_manifest`，写入 `1-sdk-worker-request.json` 审计文件，并返回 Pool-compatible `job_id/status/outputs`；真实接入时把模板输出替换成厂商 SDK/API 调用。
- `AssetRecord` 同步：本地输出可按扩展名推断 image/video/audio/3d/metadata 类型，并写入 assets 表。
- `ProviderRegistry`：注册 ComfyUI、Kling、Midjourney、OpenAI image-2、Nano Banana Pro、Suno、World Labs Marble、TripoSplat、SAM-3D、Spark、群核科技。
- `Mock3dgsProvider`：按 image-blaster 风格生成本地优先的 `.N-request.json` 与 indexed output 路径契约。
- `ThreeDgsGatewayProvider`：实现通用 HTTP 3DGS gateway adapter，支持 Marble/TripoSplat/SAM-3D/Spark/群核 profile mapping、submit/poll/download、状态映射、indexed 输出命名和本地资产落地。
- `3DGS production evidence`：`three_dgs_gateway_smoke` 在真实 3DGS gateway run 成功、提供 `POOL_PROVIDER_PRODUCTION_ATTESTATION` 时，可直接写出带本地 indexed assets 与 metadata 的 `providers[]` production evidence bundle。
- `SoftwareAdapterRegistry`：默认接入 Unreal、Blender、ComfyUI、DaVinci Resolve、Unity、TouchDesigner、MadMapper、Nuke、动捕数据库、剪辑软件与 Hermes。
- `Software control contracts`：为 Unreal、Blender、Resolve、Unity、TouchDesigner、MadMapper、Nuke、动捕数据库、剪辑软件和 Hermes 暴露机器可读控制合同，统一说明 `/api/software-health`、`/api/software-actions`、API/MCP、Skills/CLI、桌面识别兜底、人工接管路径和 `conformance_runbook` 真实接入验收步骤。
- `SoftwareAdapter`：新增外部软件控制 trait，并提供 `UnrealMcpAdapter`、`HermesMcpAdapter` 与 `MockUnrealAdapter`，支持 Unreal/Hermes MCP/API HTTP 控制优先、本地 mock fallback；Unreal MCP 请求会补 `pool_unreal_action` 与 `mcp_payload`，`/api/unreal-mcp-bridge` 与 `pool://unreal-mcp-bridge` 会暴露插件/gateway 侧实现合同，`pool-cli unreal-mcp-bridge-worker --once` 可先跑 health + dry-run action 自检，常驻模式可作为 Unreal 插件前置代理，`integrations/unreal/PoolMcpBridge` 提供可安装的 Unreal Python 插件脚手架；Hermes MCP 请求会补 `pool_hermes_action` 与 `mcp_payload`，`pool-cli hermes-mcp-bridge-worker --once` 可先跑同类自检，常驻模式可作为真实 Hermes MCP 服务前置代理；其他软件 adapter 可通过 `pool-cli software-api-bridge-worker <adapter-id> --once` 验证通用 API/MCP wrapper，再用常驻 worker 获得 dry-run/forwarder 审计入口；软件合同中的 `conformance_runbook` 会串起 local bridge baseline、real upstream bridge、health/action smoke、production matrix 和 validate/import；`pool-cli worker-self-checks` 与 MCP tool `pool_worker_self_checks` 会把这些 worker smoke 聚合成单个 Agent 可解析的 JSON 报告。
- `CommandSoftwareAdapter`：为 Blender、Nuke、Resolve、TouchDesigner 等软件提供 `ExecuteCli` 受控执行骨架，命令必须通过 payload allowlist，且不经过 shell。
- `SoftwareActionRunner`：统一执行外部软件控制动作，写入 `software_actions`、`workflow_events` 和任务状态，支持人工确认阻断，并可在 `/api/tasks/approve` 后用同一 task 恢复执行已确认动作；payload 声明 `pool_output_result` 时，成功的软件动作会自动把后段执行结果写回视频、游戏或交互艺术 manifest。
- `AgentSessionRunner`：统一 staging/执行 Hermes 内嵌控制与 Agent CLI 命令，写入 `agent_sessions`、tasks、events 和本地 transcript/control 文件；Runtime HTTP 已提供 `/api/agent-sessions`，并支持显式 `execute:true` 的 Hermes HTTP 调用和 Agent CLI allowlist 受控执行；当 `execute:true` 因 `requires_confirmation` 或 token budget 进入 `waiting_approval` 时，transcript 会记录 `execution_request`，`/api/tasks/approve` 或 `/api/tasks/retry` 可用同一 task 恢复执行；Hermes 作为软件矩阵控制入口时可通过 `HermesMcpAdapter` 走 `/api/software-actions`。
- `DesktopRecognitionAdapter`：为没有 API/MCP/CLI 的外部软件生成桌面识别控制请求 JSON，补 `pool_desktop_action` 与 `desktop_payload`，落地到 `output/control/desktop-recognition/`；Runtime HTTP 提供 controller 领取队列、机器可读 contract 与结果回填接口；本地 controller example 默认 dry-run，显式 `--mode=applescript` 时可在 macOS 上执行明确的应用激活、坐标点击、快捷键和文本输入，并可通过 `--vision-trace` 消费外部视觉/OCR trace 把 `visual_targets` 解析成点击坐标。
- `Desktop vision trace evidence`：`run_desktop_vision_trace_smoke` 会写入 TouchDesigner 桌面识别任务、本地 Pool-compatible trace JSON 和 controller callback，把 `screen_trace_path` / `controller_result.vision_trace_path` 纳入 PRD readiness；该 smoke 只证明 Pool 协议与账本，不把本地 trace 冒充真实外部视觉模型。
- `SQLite schema`：定义 projects、shots、workflows、tasks、assets、provider_requests、workflow_events、software_actions、agent_sessions、embeddings、api_keys。
- `RuntimeRepository`：执行 SQLite migration，并把默认运行蓝图持久化为 project、shot、workflow、task 和 event。
- `RuntimeSnapshot`：从 SQLite 导出 projects、workflows、node states、tasks、assets、events、provider requests、software actions、agent sessions 的统一 JSON，供 Web/SwiftUI 控制台读取。
- `RuntimeSnapshot cost stats`：snapshot stats 汇总 task estimated tokens、waiting approval estimated tokens、Agent token used/budget 和 token_total；`/api/runtime-budget` 与 `pool://runtime-budget` 会进一步聚合 Provider Key 就绪、审批门和 Provider 请求账本摘要；`/api/runtime-preflight` 与 `pool://runtime-preflight` 会输出运行前阻塞项、警告和建议 CLI next actions，并始终给出非阻塞的 `worker-self-checks` / `pool_worker_self_checks` 本地桥接 smoke 建议，桌面接管会优先建议 `desktop-run-next` 并保留 `desktop-requests` 检查命令；`/api/runtime-execution-plan` 与 `pool://runtime-execution-plan` 会把 workflow 拓扑排序成带节点状态、审批门、Provider/软件合同和推荐 CLI/MCP 控制动作的可执行步骤；`/api/runtime-handoff` 与 `pool://runtime-handoff` 会把 preflight、可运行节点、本地 worker smoke、离线接管包、审批、凭证和桌面识别请求整理成 Hermes/Agent/人工可执行 runbook，并输出 5 人内容爆发团队的角色分工与 lane 绑定；`/api/prd-readiness` 与 `pool://prd-readiness` 会把当前 PRD 按要求拆成 ready/partial/blocked、证据、缺口和 next actions，并在 `completion_gate` 中输出是否可标记完成、未完成要求和 closeout/readiness 证明命令；`/api/prd-completion-gate` 与 `pool://prd-completion-gate` 会把同一完成门槛单独暴露给 Agent/Hermes/CI，并支持 `require_complete=true` 时对未完成快照返回 428；`POST /api/prd-completion-package`、`pool-cli prd-completion-package` 与 MCP tool `pool_prd_completion_package` 会把 readiness、completion gate、production evidence requirements、manifest 和可选 snapshot 写入本地 `control/prd-completion/`，同步入 task/assets/events，作为 PRD 完成或未完成缺口的可归档证明包；`/api/production-evidence/requirements` 与 `pool://production-evidence-requirements` 会把真实 Provider、真实软件侧和外部视觉模型生产证据缺口整理成导入前清单，`/api/production-evidence/tasks` 与 `pool://production-evidence-tasks` 会把这些缺口拆成外部 worker/operator/controller 可领取的任务队列，`pool://production-evidence-handoff` 会把 requirements、tasks 和 run-plan 组合成只读分派上下文。
- `Production evidence item-template resource`：`pool://production-evidence-item-template` 会列出每个缺口任务的单项模板 URI，`pool://production-evidence-item-template/<task-id>` 会直接返回可交给 `submit-production-evidence-item` 的单项 Provider、软件或 desktop vision evidence item JSON 脚手架，供外部 worker/operator/controller 替换真实外部 id 和本地文件后回填。
- `Production evidence task claim`：`POST /api/production-evidence/tasks/claim`、`pool-cli production-evidence-claim <task-id>` 与 MCP tool `pool_production_evidence_task_claim` 会把只读缺口任务登记成可追踪 runtime task，并写出本地 `control/production-evidence/claims/*-claim.json`，用于真实外部 worker/operator/controller 执行前的审计交接。
- `Production evidence runner handoff`：`POST /api/production-evidence/handoff-packages` 与 `pool-cli production-evidence-handoff-package` 会额外写出 `7-production-evidence-runner.sh` 和 `8-production-evidence-runner-preflight.json`，把 Provider matrix、software matrix、desktop vision、merge、closeout preflight/import 串成可审计脚本与机器可读预检合同；Provider 阶段可用 `POOL_MEDIA_GATEWAY_ENDPOINT` / `POOL_3DGS_GATEWAY_ENDPOINT` 覆盖整个 AI media/3DGS 家族，也可为每个 required Provider 提供 `POOL_PROVIDER_ENDPOINT_<PROVIDER>` / `POOL_<PROVIDER>_ENDPOINT`，OpenAI image-2 native adapter 还需要 `OPENAI_API_KEY` 或 provider-specific key env；生产证明则需要全局 `POOL_PROVIDER_PRODUCTION_ATTESTATION` 覆盖完整矩阵，或为每个 required Provider 提供 `POOL_PROVIDER_PRODUCTION_ATTESTATION_<PROVIDER>` / `POOL_<PROVIDER>_PRODUCTION_ATTESTATION` 证明真实上游 worker/SDK run，也可用 `--provider-endpoint[-env]`、`--provider-api-key[-env]` 和 `--provider-attestation[-env]` 对单个厂商 worker 显式注入；软件阶段需要真实 endpoint/command、production attestation 以及每个 adapter 对应的本地 artifact 路径，通用 endpoint 可指向转发到真实插件/gateway 的 `pool-cli software-api-bridge-worker <adapter-id>`，既可隐式读取 `POOL_*` 环境变量，也可用 `production-evidence-software-matrix` 的 `--software-endpoint[-env]`、`--software-command[-env]`、`--software-artifact[s-env]` 和 `--software-attestation[-env]` 对 Unreal、Blender、Resolve、TouchDesigner、Hermes 等 adapter 显式注入；桌面视觉阶段可用 `production-evidence-desktop-vision` 的 `--trace-env`、`--external-action-id-env` 和 `--production-attestation-env` 明确绑定外部视觉 controller 产出的 trace/action/attestation 环境变量。可先运行 `7-production-evidence-runner.sh --preflight` 检查环境；脚本会优先使用 PATH 上的 `pool-cli`，找不到时自动 fallback 到 `cargo run -q -p pool-cli --`，也可用 `POOL_CLI_CMD` 覆盖，只有实际使用 cargo fallback 时才要求本机有 cargo；脚本默认只做 Provider/software、merge 和 closeout preflight，需显式设置 `POOL_RUN_DESKTOP_VISION=1`、`POOL_DESKTOP_VISION_TRACE`、`POOL_DESKTOP_VISION_EXTERNAL_ACTION_ID`、`POOL_DESKTOP_VISION_PRODUCTION_ATTESTATION` 和 `POOL_IMPORT_PRODUCTION_EVIDENCE=1` 才会执行桌面视觉与正式导入。
- `Web prototype snapshot/runtime loader`：`apps/web-prototype` 可通过 `?snapshot=...` 读取 runtime snapshot JSON，也可通过 `?runtime=http://127.0.0.1:4788` 读取本地 Runtime HTTP API；无 query 参数或 `?runtime=local/auto` 时会自动探测默认本地端口组，并支持 `?runtime_registry=runtime-registry.json`、`?runtime_ports=4788,4789` / `?runtime_endpoints=http://127.0.0.1:4788,http://127.0.0.1:4878` 覆盖；支持 `?project=slug` / `?project=*` 项目过滤；连接 runtime 时会读取 `/api/projects` 填充顶部项目选择器，读取 `/api/discovery` 在 Agent 页展示 endpoint manifest、MCP resource/tool/prompt 计数、`pool_worker_self_checks`、`pool_handoff_package` 等工具摘要和 `serve-mcp` 启动命令，读取 `/api/runtime-graph` 作为节点图拓扑与任务类型来源，读取 `/api/runtime-execution-plan` 作为按拓扑排序的执行步骤、审批门、合同和推荐命令来源，读取 `/api/runtime-budget` 作为接入页预算与凭证就绪摘要，读取 `/api/runtime-preflight` 作为运行前阻塞/警告/next actions 摘要，读取 `/api/prd-readiness` 和 `/api/prd-completion-gate` 作为 PRD 审计与完成门槛来源，PRD 面板可调用 `/api/prd-completion-package` 写出本地完成证明包并显示 manifest 路径，读取 `/api/workflow-context?workflow_id=...` 作为当前 workflow 的任务、资产、Provider 请求、软件动作和 Agent session 摘要，读取 `/api/prompts` 填充 Hermes/Agent runbook 选择器，`/api/node-context?node_id=...` 可按单个节点下钻任务、资产、Provider 请求、软件动作和 `control_context` 控制入口，`/api/agent-sessions/transcript?session_id=...` 可展开 Hermes/Agent CLI 会话正文，随后优先订阅 `/api/agent-sessions/ws?session_id=...`，失败时回退 `/api/agent-sessions/stream?session_id=...`，接收该 session 的 transcript 和相关 runtime event，`/api/mcp?uri=pool://workflow/<workflow-id>` 可给 Agent/Hermes 精确读取单 workflow 的图、任务、资产、Provider 请求和软件动作上下文，读取 `/api/adapters` 同步 Provider 与外部软件能力矩阵，读取 `/api/provider-contracts` 并在 Provider 卡片展示 native/gateway contract 摘要，读取 `/api/provider-gateway-worker` 并在接入页展示 AI media/3DGS worker 启动命令、endpoint env 和 MCP tool，接入页可粘贴或一键填入覆盖 9 个 required Provider、11 个 required software adapter 和外部视觉模型的生产证据 bundle 模板，并可调用 `/api/production-evidence/merge` 把多路 bundle 合并回 textarea，也可调用 `/api/production-evidence/closeout` 做收口预检或显式收口导入；模板 id 会被 `/api/production-evidence/validate` 拒绝，必须替换为真实外部 job/action/controller id 后才能导入并刷新 PRD readiness；读取 `/api/software-contracts` 并在软件矩阵展示控制合同摘要，读取 `/api/desktop-recognition/contract` 并在桌面识别队列展示 controller contract 摘要，接入页“批量巡检 Adapter”会调用 `/api/adapter-health`，Provider“测试连接”会调用 `/api/provider-health`，软件矩阵“检查”会调用 `/api/software-health`，节点详情“运行节点”会调用 `/api/nodes/run`，任务队列“取消/重试”会调用 `/api/tasks/cancel` 与 `/api/tasks/retry`，桌面识别接管队列会调用 `/api/desktop-recognition/requests`，可用 `/api/desktop-recognition/run-next` dry-run 推进，并通过 `/api/desktop-recognition/results` 回填结果，运行事件流会优先使用 `/api/events/ws` WebSocket 长连接日志，失败时回退 `/api/events/stream` EventSource/SSE，再回退 `/api/events` 轮询增量刷新；节点图会显示 RuntimeGraph 的任务类型、资产流、控制流、Agent 指令、审批和反馈循环，并用最新 snapshot task 状态叠加；每次 runtime 写操作返回 snapshot 后，Web 会重新读取 `/api/discovery`、`/api/runtime-graph`、`/api/runtime-execution-plan`、`/api/runtime-budget`、`/api/runtime-preflight`、`/api/prd-readiness`、`/api/prd-completion-gate`、`/api/provider-contracts`、`/api/provider-gateway-worker`、`/api/software-contracts`、`/api/desktop-recognition/contract` 和 `/api/workflow-context` 再合并，避免 discovery、运行图语义、执行步骤、预算凭证、PRD 完成门槛、Provider contract、Provider gateway worker contract、软件控制合同、桌面 controller contract、运行前检查和 workflow 账本摘要陈旧；Provider 卡片和 3DGS 节点详情会显示 `provider_requests` 请求账本与 metadata 路径，软件卡片和软件节点详情会显示 `software_actions` 审计、control contract 与 artifacts。
- `Web integration readiness panel`：Web 连接 Runtime 后会读取 `/api/integration-readiness`，在接入页展示 Provider、软件 adapter、Agent/Hermes 三类 readiness 行、5 人团队 lane 摘要和 next-action run plan；每次 runtime 写操作刷新 snapshot 后也会同步重新读取该矩阵，避免接入状态和账本脱节。
- `Production evidence scaffold UI`：Web prototype 接入页连接 Runtime HTTP 后可读取 `GET /api/production-evidence/requirements` 展示真实 Provider、软件侧和外部视觉 controller 缺口；“生成脚手架”按钮会调用 `GET /api/production-evidence/template`，把 9 个 required Provider、11 个 required software adapter 和外部视觉 controller 的 bundle 脚手架写入校验文本框；“领取任务”按钮会调用 `POST /api/production-evidence/tasks/claim`，把当前 evidence task 登记为可审计 runtime task 并显示 claim JSON 路径；“生成单项”会调用 `GET /api/production-evidence/item-template`，外部 worker 替换真实 id 后可先用“预检单项”调用 `POST /api/production-evidence/items/validate` 做 `writes:0` dry-run，再用“提交单项”正式回填；“账本收口”按钮会调用 `GET /api/production-evidence/bundle-from-ledger?include_incomplete=true`，把已 ready 的 Provider/software/desktop vision ledger 批量写回 bundle textarea，并显示未 ready 诊断数量；“生成计划”按钮会调用 `GET /api/production-evidence/run-plan`，把剩余缺口整理成 Provider matrix、software matrix、desktop vision controller、merge、closeout preflight/import 和 completion proof 七段真实执行计划，并只在结果面板展示计划摘要，避免把 run-plan 当成可导入 bundle；CLI/MCP 的 `production-evidence-run-plan` / `pool_production_evidence_run_plan` 提供同一能力给 Hermes/Agent；未连接 runtime 时仍保留本地示例填入路径。
- `Web software conformance UI`：Web 软件矩阵读取 `/api/software-contracts` 后会在每张软件卡片展示 `conformance_runbook` phases，把 local bridge baseline、real upstream bridge、health/action smoke、production matrix 和 validate/import 变成可直接执行的验收清单。
- `Software conformance package`：`POST /api/software-conformance-packages`、`pool-cli software-conformance-package <adapter-id>` 和 MCP tool `pool_software_conformance_package` 会把单个软件 adapter 的合同、runbook、preflight、runner script 和 manifest 写到本地 `control/software-conformance/<adapter-id>/`，方便 Agent 或 5 人团队成员接手真实软件 bridge 验收。
- `Agent session WebSocket stream`：Hermes/Agent CLI 会话详情优先订阅 `/api/agent-sessions/ws?session_id=...`，接收 transcript 与该 session 相关 runtime event 的 JSON 文本帧；失败时回退 `/api/agent-sessions/stream?session_id=...` SSE。
- `API Key runtime control`：Web prototype 可将 Provider key 写入 `/api/api-keys`；snapshot/MCP 只返回脱敏 key 状态，Provider run 可自动读取已保存 key，`GET /api/api-keys?rotation_days=90` 会返回凭证 backend/source/owner/age/rotation_due 审计摘要，接入页会用 runtime budget 面板显示 Provider Key 配置比例、待审批 token、缺失凭证数和轮换审计。
- `Output package runtime control`：Web prototype 的“三类输出”面板会读取 `GET /api/output-packages` 的三类交付 catalog，显示视频、游戏、交互艺术 manifest 的 ready/missing 状态、本地路径、预览合同和控制路由；按钮继续调用 `POST /api/output-packages`，将 manifest 写入本地项目包、刷新资产台账，并展示时间线/构建/cue 摘要；每张 manifest 卡片可调用 `POST /api/output-packages/results` 标记后段完成，把 Resolve/Unreal/TouchDesigner/MadMapper 的执行结果回填到本地 manifest，并把 `execution` / `runtime_result` 指标显示回控制台。
- `Runtime handoff package control`：Web prototype 的 Runtime Handoff 卡片可调用 `/api/handoff-packages`，把接管 runbook、preflight、runtime graph、integration readiness、worker self-check runner、worker preflight、带 operator checklist 的 manifest、5 人团队分工和 snapshot 写入 `output/control/handoff/`，并在面板中显示最近接管包的 manifest、operator checklist、Agent/MCP 入口、readiness 和 worker smoke 路径。
- `Runtime execution plan control`：Web prototype 的 Runtime Handoff 面板提供“预览下一步”和“执行下一步”，连接 Runtime HTTP 后调用 `/api/runtime-execution-plan/run-next`，先把 Agent/Hermes 可执行的下一步动作展示出来，再由显式执行按钮推进。
- `Workflow runtime control`：Web prototype 顶部“运行一次”在连接 Runtime HTTP 后会调用 `/api/workflow-runs`，运行本地内容爆发闭环并刷新节点、任务、Agent 会话、资产和事件流；Hermes 面板与节点详情侧栏会展示 ContentBurst 的 Agent 模式、3DGS/Unreal adapter 模式、transcript 路径和最新 Agent session；Hermes 面板可从 `/api/prompts?name=...` 加载 runbook 并写入控制指令框。
- `Credential storage`：设置 `POOL_CREDENTIAL_STORE=keychain` 后，新写入的 API Key 会保存到 macOS Keychain，SQLite 只保存引用；设置 `POOL_CREDENTIAL_PASSPHRASE` 后可用 AES-256-GCM 封装保存；未设置时保留 legacy plaintext 兼容模式；`set-api-key --rotation-days` 与 `api-keys --rotation-days` 可做本地轮换审计。
- `materialize_project_envelope`：把默认运行蓝图写入 image-blaster 风格 `worlds/<slug>/` 本地项目包。
- `Pool MCP resources`：`pool://status`、`pool://projects`、`pool://tasks`、`pool://assets`、`pool://adapters`、`pool://provider-contracts`、`pool://provider-contracts/<provider-id>`、`pool://provider-gateway-worker`、`pool://software-contracts`、`pool://software-contracts/<adapter-id>`、`pool://unreal-mcp-bridge`、`pool://desktop-recognition-contract`、`pool://workflow`、`pool://workflow/<workflow-id>`、`pool://runtime-graph`、`pool://runtime-budget`、`pool://runtime-preflight`、`pool://runtime-execution-plan`、`pool://runtime-handoff`、`pool://prd-readiness`、`pool://prd-completion-gate`、`pool://production-evidence-requirements`、`pool://production-evidence-tasks`、`pool://production-evidence-run-plan`、`pool://production-evidence-handoff`、`pool://output-packages`、`pool://node-context/<node-id>`、`pool://provider-requests`、`pool://software-actions`、`pool://desktop-recognition`、`pool://agent-sessions`、`pool://api-keys` 等资源可由 `RuntimeSnapshot` 驱动；其中 adapter catalog 会暴露 Provider/软件矩阵、别名、控制优先级和本地优先策略，provider contracts 会暴露 AI media gateway、3DGS gateway 与 native adapter 的 submit/poll/body/status/output 合同，provider gateway worker 会暴露本地 forwarder 的 CLI、route、upstream 和 endpoint env 合同，software contracts 会暴露外部软件控制的 health/action body、优先级链、API/MCP、Skills/CLI、桌面识别和人工接管路径，unreal mcp bridge 会暴露 `pool_unreal_action`、`mcp_payload`、工具 schema、响应字段和插件侧检查项，desktop recognition contract 会暴露桌面 controller 的请求文件、队列、dry-run/AppleScript/vision trace 执行模式、回填状态和证据要求，运行图资源会把 workflow nodes、task 状态、连接标签和 task 类型合并成可执行控制图，runtime budget 会聚合 token、审批门、Provider Key 配置和 Provider 请求摘要，runtime preflight 会聚合阻塞项、警告、`pool_worker_self_checks` 本地 worker smoke 和建议 CLI next actions，runtime execution plan 会聚合按节点顺序执行的步骤、合同、审批门和推荐动作，runtime handoff 会输出 Hermes/Agent/本地 worker smoke/离线接管包/桌面 controller/人工接管 runbook 和 5 人团队 lane 绑定，prd readiness 会输出 PRD 要求级 ready/partial/blocked 审计、证据、缺口和 next actions，prd completion gate 会输出是否允许标记完成的机器门槛，production evidence requirements 会输出真实生产证据缺口、必填 bundle 字段和本地文件策略，production evidence tasks 会输出 Provider/软件/desktop vision 缺口任务队列和单项 item 回填入口，production evidence run plan 会输出七段真实生产证据执行顺序、命令、路径、HTTP/MCP 入口和人工检查项，production evidence handoff 会把 requirements、tasks、run-plan、分派 lanes、命令和 operator checklist 组合成 Agent 可读上下文，output packages 会把视频、游戏、交互艺术三个本地 indexed manifest 规整为 ready/missing catalog，单 workflow context 会聚合该 workflow 的图、任务、资产、Provider 请求、软件动作和相关 Agent session，节点上下文资源会聚合单个节点的任务、资产、Provider 请求、软件动作、相关 Agent session、绑定 adapter/provider 配置和建议 CLI/MCP 控制入口，桌面识别资源会把待 controller 接管的请求和历史结果分开暴露并内嵌同一份 contract。
- `Pool integration readiness resource`：`pool://integration-readiness` 使用同一份 snapshot，把 adapter catalog、api key、provider_requests、software_actions、tasks 和 agent_sessions 聚合成接入矩阵，并按 `orchestration / ai_media / spatial_engine / post_output / interactive_systems` 五条 lane 分派 next action，供 Hermes/Agent 在生成或执行 conformance package 前先判断缺配置、缺执行或需处理的项目。
- `Production evidence item-template MCP`：`pool://production-evidence-item-template` 是模板索引资源，`pool://production-evidence-item-template/<task-id>` 是具体单项 evidence item resource；它和 `pool_production_evidence_item_template` tool 共用同一类输出语义，但 resource 路径保持只读，便于 Agent/Hermes 在不触发写操作的情况下分发单项任务。
- `RuntimeHttpServer`：提供本地 runtime HTTP API，把 `/api/runtime-registry`、`/api/discovery`、`/.well-known/pool-runtime.json`、`/api/health`、`/api/snapshot`、`/api/projects`、`/api/events`、`/api/events/stream`、`/api/events/ws`、`/api/resources`、`/api/prompts`、`/api/runtime-graph`、`/api/runtime-execution-plan`、`/api/runtime-execution-plan/run-next`、`/api/runtime-budget`、`/api/runtime-preflight`、`/api/runtime-handoff`、`/api/prd-readiness`、`/api/prd-completion-gate`、`/api/prd-completion-package`、`/api/production-evidence/requirements`、`/api/workflow-context`、`/api/node-context`、`/api/mcp?uri=...`、`/api/api-keys`、`/api/adapters`、`/api/provider-contracts`、`/api/provider-gateway-worker`、`/api/software-contracts`、`/api/unreal-mcp-bridge`、`/api/adapter-health`、`/api/provider-health`、`/api/provider-requests/metadata`、`/api/software-health`、`/api/nodes/run`、`/api/tasks`、`/api/workflow-runs`、`/api/provider-runs`、`/api/output-packages`、`/api/output-packages/results`、`/api/agent-sessions`、`/api/agent-sessions/transcript`、`/api/agent-sessions/stream`、`/api/agent-sessions/ws`、`/api/tasks/approve`、`/api/tasks/cancel`、`/api/tasks/retry`、`/api/software-actions`、`/api/desktop-recognition/requests`、`/api/desktop-recognition/contract`、`/api/desktop-recognition/run-next` 和 `/api/desktop-recognition/results` 接到同一份 SQLite runtime；`/api/api-keys` 会返回脱敏 key 状态和本地轮换审计摘要；`/api/runtime-registry` 和 `write_runtime_registry()` 会生成 Web/Hermes/桌面 controller 可读取的本地服务注册表，`/api/events/ws` 会以 WebSocket JSON 文本帧持续推送 `workflow_events` 并暴露在 discovery 中，`/api/prd-readiness` 会返回当前 PRD 要求级机器审计，`/api/prd-completion-gate?require_complete=true` 会在未满足完成门槛时返回 `prd_completion_gate_incomplete` / HTTP 428，`/api/prd-completion-package` 会把当前 readiness/gate/production evidence requirements 写成本地完成证明包并入库为 task/assets/events，`/api/production-evidence/requirements` 会返回生产证据需求清单，`/api/provider-contracts` 会返回外部 AI/3DGS gateway 或 native adapter 的机器可读接入合同，`/api/provider-gateway-worker` 会返回本地 forwarder 的启动/upstream/endpoint env 合同，`/api/software-contracts` 会返回外部软件控制的机器可读 health/action/route 合同，`/api/unreal-mcp-bridge` 会返回 Unreal 插件/gateway 侧 `pool_unreal_action` / `mcp_payload` bridge contract，`/api/desktop-recognition/contract` 会返回桌面 controller 的请求文件、队列和结果回填协议，`/api/discovery` 会返回 endpoint manifest、每个 `pool://...` resource 的 HTTP 读取路径和 Agent runbook prompt manifest；节点级执行会把 node `control_context` 写入 Provider request ledger 或软件 action payload；审批高成本 Provider run 时会读取 `provider_requests` 并用同一 task 继续执行，审批需要人工确认的软件动作时会读取 `software_actions.command_json` 并用同一 task 恢复执行，审批带 `execution_request` 的 Agent session 时会读取 transcript 并用同一 task 恢复 Hermes HTTP 或 Agent CLI 执行，重试失败/取消任务时也会优先按 Provider/software/Agent 账本重跑；Provider retry 会追加新的 request attempt，保留旧失败响应，并在 `request_json.attempt` 里记录 retry parent。
- `RuntimeHttpServer integration readiness`：`GET /api/integration-readiness` 暴露同源只读接入矩阵，和 `pool://integration-readiness`、`pool-cli integration-readiness`、`pool_integration_readiness` 共用 payload。
- `RuntimeHttpServer workflow dispatch`：`/api/workflow-runs` 已能触发 `ContentBurstRunner`，并支持 `agent_mode`、`three_dgs_mode` 与 `unreal_mode` 在 Hermes staging/HTTP、真实 adapter 和本地 mock 间切换。
- `RuntimeHttpServer output package dispatch`：`GET /api/output-packages` 会从 snapshot/assets 和本地 manifest 文件生成三类交付 catalog；`POST /api/output-packages` 会触发 `OutputPackageRunner`，把视频、游戏、交互艺术三类本地交付 manifest 写入项目 `output/deliverables/`，返回 manifest summaries 并刷新 snapshot；`POST /api/output-packages/results` 会把后段执行结果写回 `execution_result` / `execution_history`，并写入 `output-package-result` task 与事件。
- `RuntimeHttpServer handoff package dispatch`：`/api/handoff-packages` 已能触发 `RuntimeHandoffPackageRunner`，生成 `1-runtime-handoff.json`、`2-runtime-preflight.json`、`3-runtime-graph.json`、`7-integration-readiness.json`、`8-runtime-handoff-package-manifest.json`、`5-worker-self-checks.sh`、`6-worker-self-checks-preflight.json` 和可选 `4-runtime-snapshot.json`，并在 report 直接返回 `operator_checklist`、`agent_entrypoint` 和 `mcp_resources`，同时返回 task 和刷新后的 snapshot。
- `RuntimeHttpServer provider dispatch`：`/api/provider-runs` 已能调度 ComfyUI、Kling、OpenAI image-2、Midjourney、Nano Banana Pro、Suno、3DGS gateway 与本地 mock 3DGS；会自动写入 adapter 成本估算和 `provider_requests` 请求账本，省略审批参数时高成本 Provider 进入 `waiting_approval`，同时在 output dir 写入 `.0-provider-approval__<provider>-request.json` 本地审批请求包；审批后可通过 `/api/tasks/approve` 恢复同一 Provider run；3DGS 默认安全走 mock，媒体类 Provider 需要显式 endpoint 或环境变量 gateway。
- `RuntimeHttpServer production evidence validation/import`：`GET /api/production-evidence/requirements` 可读取只读生产证据 doctor 清单；`GET /api/production-evidence/run-plan` 可把当前缺失证据整理成 Provider matrix、software matrix、desktop vision controller、merge、closeout preflight/import 和 completion proof 七段真实执行计划；`GET /api/production-evidence/template` 可生成覆盖 9 个 required Provider、11 个软件 adapter 和外部视觉 controller 的生产证据 bundle 脚手架；`GET /api/production-evidence/item-from-ledger` 可把已有 `provider_requests` / `software_actions` / desktop recognition callback 账本整理成单项 production evidence item 草稿，并用 schema、本地文件和 production flags 三重预检防止本地 mock 或 dry-run trace 被误当生产证据；`GET /api/production-evidence/bundle-from-ledger` 可把已 ready 的运行账本批量收成 production evidence `.bundle`，并可带出未 ready 诊断；`POST /api/production-evidence/merge` 可把多个外部 worker/operator/controller 返回的 bundle 合成一个 `.bundle`，返回 `writes:0` 且不写 SQLite；`POST /api/production-evidence/closeout` 可对多路 bundle 执行 merge + validate，默认不写 SQLite，只有显式 `import:true` 时才复用正式导入路径，成功后顶层返回 completion gate、PRD summary 和完成证明包命令，传 `completion_package` 时可直接写出本地 PRD completion package；`POST /api/production-evidence/validate` 可在不写入 SQLite 的情况下校验外部生产证据 bundle，并返回 provider/software/desktop evidence 计数、`writes:0`、PRD 生产证据 coverage 缺口和 `artifact_files` 本地文件预检；`POST /api/production-evidence` 可导入外部真实厂商 worker、真实软件插件/API/CLI 和真实桌面视觉 controller 已完成后的证据 bundle，写入 `provider_requests`、`software_actions`、tasks、events，并即时返回同源 coverage 与刷新后的 PRD readiness；导入会先预校验整包，将 Provider/软件别名标准化后写入账本，拒绝模板占位 external id、缺失或占位的 Provider/软件 `production_attestation`、远程 Provider artifact/metadata URL、缺失的 Provider 本地 artifact/metadata 文件、缺失的软件本地 artifact 文件、缺失的 desktop trace/artifact 文件和未显式外部视觉模型的 desktop trace，无效 bundle 不会部分写入账本；示例 schema 见 `docs/examples/production-evidence-bundle.example.json`，`import_production_evidence_bundle` example 可生成本地 artifact/metadata fixture 并验证完整导入路径，`closeout_production_evidence_bundle` example 会把同一证据拆成 Provider/软件/桌面视觉三路 bundle，经 closeout 预检和显式导入后验证 PRD readiness 达到 10/10。
- 生产证据 attestation 口径统一：Provider、software action 和 desktop vision item 都必须提供真实 `production_attestation` 或 `evidence_json.production_attestation`；模板、mock、dry-run 或缺少 attestation 的外部视觉 trace 不会被计入 PRD 完成证据。
- `pool-cli`：提供 `status`、`snapshot`、`projects`、`adapters`、`provider-contracts`、`provider-conformance-package`、`integration-conformance-package`、`software-contracts`、`software-conformance-package`、`agent-conformance-package`、`unreal-mcp-bridge`、`unreal-mcp-bridge-worker`、`software-api-bridge-worker`、`worker-self-checks`、`desktop-contract`、`tasks`、`events`、`runtime-budget`、`runtime-preflight`、`runtime-execution-plan`、`runtime-run-next`、`runtime-handoff`、`prd-readiness`、`prd-completion-gate --require-complete`、`prd-completion-package`、`production-evidence-requirements`、`production-evidence-run-plan`、`runtime-graph`、`workflow-context`、`node-context`、`mcp`、`serve-mcp`、`provider-gateway-worker-contract`、`provider-gateway-worker`、`provider-sdk-worker-template`、`api-keys --rotation-days`、`set-api-key --rotation-days`、`adapter-health`、`provider-health`、`run-provider`、`production-evidence-provider-matrix`、`production-evidence-software-matrix`、`production-evidence-desktop-vision`、`production-evidence-template`、`merge-production-evidence`、`closeout-production-evidence`、`validate-production-evidence`、`import-production-evidence`、`provider-request-metadata`、`software-health`、`output-packages`、`run-software`、`output-package`、`output-result`、`handoff-package`、`agent-session`、`agent-transcript`、`agent-stream`、`desktop-requests`、`desktop-run-next`、`desktop-result`、`run-node`、`run-workflow`、`approve-task`、`cancel-task` 和 `retry-task` 命令，作为 Hermes/Agent CLI 可调用的本地控制入口、PRD readiness 审计入口、完成门槛失败退出入口、PRD 完成证明包、生产证据 doctor、真实生产证据七段执行计划、Provider/软件/桌面视觉生产证据矩阵、Provider gateway 合同读取/forwarder、Provider conformance 交接包、Integration conformance 总交接包、Agent/Hermes conformance 交接包、Provider SDK wrapper 样板、Unreal bridge 合同读取/worker、通用软件 API bridge worker、聚合 worker 自检、MCP stdio server、Provider/API Key 接入 smoke、外部软件控制、生产证据脚手架/合并/收口/校验/导入、软件控制合同读取、输出交付 catalog 查询与后段结果回填、桌面识别 contract/dry-run controller、内容爆发闭环触发器与运行队列接管工具。
- `pool-cli integration-readiness`：读取 `/api/integration-readiness`，作为 conformance package 之前的只读接入盘点命令。

## 接入范围与优先级

- AI 图片/视频生成：ComfyUI、Kling、Midjourney、OpenAI image-2、Nano Banana Pro、Suno。
- 3DGS / 2D→3D：World Labs Marble、TripoSplat、SAM-3D、Spark、群核科技。
- 内嵌控制：Hermes endpoint、控制指令、任务队列写入、HTTP 执行和运行 trace。
- Agent CLI：workflow context 读取、Provider 任务、Hermes 控制、Agent 会话命令模板、session transcript/stream 读取和受控 allowlist 执行；Web 中的命令模板会按当前 project/workflow 动态生成真实 `pool-cli` 命令。
- 软件控制优先级：`API/MCP > Skills/CLI > Desktop Recognition > Human Takeover`。

真实 Provider 调用已接入 runtime adapter；端到端使用可通过本地环境变量、请求覆盖或 `/api/api-keys` 写入凭证。3DGS 与 Midjourney/Nano Banana Pro/Suno 已具备 Pool gateway profile mapping、mock gateway 契约级 smoke、真实 gateway worker 翻译模板和本地 HTTP forwarder；厂商专用 SDK、安全代理实现继续作为下一阶段。

## 设计审计

详见：

- [docs/runtime-framework.md](/Users/iqiyi-lab-vision/Documents/Coding/pool/docs/runtime-framework.md)
- [docs/reference-comparison.md](/Users/iqiyi-lab-vision/Documents/Coding/pool/docs/reference-comparison.md)
- [docs/requirements-audit.md](/Users/iqiyi-lab-vision/Documents/Coding/pool/docs/requirements-audit.md)
- [docs/runtime-snapshot.md](/Users/iqiyi-lab-vision/Documents/Coding/pool/docs/runtime-snapshot.md)
- [docs/runtime-http-server.md](/Users/iqiyi-lab-vision/Documents/Coding/pool/docs/runtime-http-server.md)
- [docs/pool-cli.md](/Users/iqiyi-lab-vision/Documents/Coding/pool/docs/pool-cli.md)
- [docs/credential-storage.md](/Users/iqiyi-lab-vision/Documents/Coding/pool/docs/credential-storage.md)
- [docs/kling-adapter.md](/Users/iqiyi-lab-vision/Documents/Coding/pool/docs/kling-adapter.md)
- [docs/openai-image-adapter.md](/Users/iqiyi-lab-vision/Documents/Coding/pool/docs/openai-image-adapter.md)
- [docs/generic-http-media-adapter.md](/Users/iqiyi-lab-vision/Documents/Coding/pool/docs/generic-http-media-adapter.md)
- [docs/three-dgs-gateway-adapter.md](/Users/iqiyi-lab-vision/Documents/Coding/pool/docs/three-dgs-gateway-adapter.md)
- [docs/provider-gateway-mock-server.md](/Users/iqiyi-lab-vision/Documents/Coding/pool/docs/provider-gateway-mock-server.md)
- [docs/provider-gateway-template.md](/Users/iqiyi-lab-vision/Documents/Coding/pool/docs/provider-gateway-template.md)
- [docs/provider-gateway-worker.md](/Users/iqiyi-lab-vision/Documents/Coding/pool/docs/provider-gateway-worker.md)
- [docs/unreal-mcp-bridge.md](/Users/iqiyi-lab-vision/Documents/Coding/pool/docs/unreal-mcp-bridge.md)
- [docs/hermes-mcp-bridge-worker.md](/Users/iqiyi-lab-vision/Documents/Coding/pool/docs/hermes-mcp-bridge-worker.md)
- [integrations/unreal/PoolMcpBridge/README.md](/Users/iqiyi-lab-vision/Documents/Coding/pool/integrations/unreal/PoolMcpBridge/README.md)
- [docs/software-control-runner.md](/Users/iqiyi-lab-vision/Documents/Coding/pool/docs/software-control-runner.md)
- [docs/agent-session-runner.md](/Users/iqiyi-lab-vision/Documents/Coding/pool/docs/agent-session-runner.md)
- [docs/content-burst-runner.md](/Users/iqiyi-lab-vision/Documents/Coding/pool/docs/content-burst-runner.md)
- [docs/output-package-runner.md](/Users/iqiyi-lab-vision/Documents/Coding/pool/docs/output-package-runner.md)

## 验证方式

Rust core：

```bash
cargo test
```

Runtime smoke：

```bash
cargo run -p pool-core --example persist_default_plan
```

Provider runner smoke：

```bash
cargo run -p pool-core --example run_mock_3dgs_task
```

Provider gateway mock：

```bash
cargo run -p pool-core --example provider_gateway_mock_server -- once
cargo run -p pool-core --example provider_gateway_template -- --contract
pool-cli worker-self-checks --output-root target/pool-worker-self-checks
pool-cli provider-gateway-worker --once
pool-cli provider-sdk-worker-template --once --output-root target/provider-sdk-worker-template
cargo check -p pool-core --example provider_gateway_worker
cargo check -p pool-cli
cargo run -p pool-core --example run_provider_evidence_matrix -- target/provider-evidence-matrix
pool-cli --project demo production-evidence-provider-matrix target/provider-evidence-matrix --no-env
pool-cli --project demo production-evidence-run-plan target/provider-run-plan.json --output-root target/provider-evidence-matrix
```

`production-evidence-run-plan` 会生成可直接交给真实 worker 的 Provider matrix 命令，显式包含 9 个 required Provider 的 `--provider-endpoint-env`、`--provider-api-key-env` 和 `--provider-attestation-env`。

Content burst runner smoke：

```bash
cargo run -p pool-core --example run_content_burst
```

PRD readiness local evidence smoke（同一个 SQLite runtime 内写入内容爆发、9 个 Provider profile、11 个软件 control profile、桌面 vision trace callback 和三类输出证据）：

```bash
cargo run -p pool-core --example run_prd_readiness_smoke -- target/prd-readiness-runner
cargo run -p pool-core --example run_prd_readiness_smoke -- --with-production-evidence target/prd-readiness-production-runner
cargo run -p pool-cli -- --db target/prd-readiness-runner/pool-runtime.sqlite --project demo production-evidence-requirements
cargo run -p pool-cli -- --db target/prd-readiness-runner/pool-runtime.sqlite --project demo production-evidence-template target/production-evidence-template/bundle.json --output-root target/production-evidence-template --source external-worker-handoff
cargo run -p pool-cli -- --db target/prd-readiness-runner/pool-runtime.sqlite --project demo validate-production-evidence docs/examples/production-evidence-bundle.example.json
cargo run -p pool-cli -- --db target/prd-readiness-runner/pool-runtime.sqlite --project demo prd-completion-package --output-dir target/prd-completion-package --include-snapshot
cargo run -p pool-core --example import_production_evidence_bundle -- target/production-evidence-import-smoke
cargo run -p pool-core --example closeout_production_evidence_bundle -- target/production-evidence-closeout-smoke
# import-production-evidence 需要 bundle 中的 Provider artifacts/metadata、software artifacts 和 desktop trace/artifacts 已经真实落成本地文件
```

Output package smoke：

```bash
cargo run -p pool-core --example run_output_package
```

3DGS gateway smoke：

```bash
cargo run -p pool-core --example three_dgs_gateway_smoke
POOL_3DGS_GATEWAY_ENDPOINT=http://127.0.0.1:8787 cargo run -p pool-core --example three_dgs_gateway_smoke -- request.json target/three-dgs-gateway-smoke worldlabs-marble
```

Software runner smoke：

```bash
cargo run -p pool-core --example run_mock_unreal_action
cargo run -p pool-core --example run_unreal_mcp_action
POOL_UNREAL_MCP_ENDPOINT=http://127.0.0.1:8787 cargo run -p pool-core --example run_unreal_mcp_action -- http://127.0.0.1:8787 target/unreal-mcp-runner
python3 -m py_compile integrations/unreal/PoolMcpBridge/Content/Python/init_unreal.py integrations/unreal/PoolMcpBridge/Content/Python/pool_mcp_bridge.py
# 本地 worker 自检，不占用常驻端口。
cargo run -p pool-cli -- unreal-mcp-bridge-worker --once --output-root target/unreal-mcp-bridge-worker
cargo run -p pool-cli -- hermes-mcp-bridge-worker --once --output-root target/hermes-mcp-bridge-worker
cargo run -p pool-cli -- software-api-bridge-worker resolve --once --output-root target/software-api-bridge-worker
cargo run -p pool-cli -- worker-self-checks --output-root target/pool-worker-self-checks
# 常驻 worker，另开终端运行；可加 --max-requests 限制处理请求数。
cargo run -p pool-cli -- unreal-mcp-bridge-worker --bind 127.0.0.1:8790 --output-root target/unreal-mcp-bridge-worker
cargo run -p pool-cli -- hermes-mcp-bridge-worker --bind 127.0.0.1:8792 --output-root target/hermes-mcp-bridge-worker --max-requests 1
cargo run -p pool-cli -- software-api-bridge-worker resolve --bind 127.0.0.1:8793 --output-root target/software-api-bridge-worker --max-requests 1
pool-cli --project demo production-evidence-software-matrix target/software-evidence-matrix --no-env
POOL_SOFTWARE_PRODUCTION_ATTESTATION=real-software-operator-run-001 POOL_UNREAL_MCP_ENDPOINT=http://127.0.0.1:8790 POOL_BLENDER_COMMAND="/Applications/Blender.app/Contents/MacOS/Blender --background --python scripts/pool_blender_evidence.py" pool-cli --project demo production-evidence-software-matrix target/software-evidence-matrix --production-software
cargo run -p pool-core --example stage_desktop_recognition_action
cargo run -p pool-core --example run_desktop_vision_trace_smoke -- target/desktop-vision-trace-smoke
POOL_DESKTOP_VISION_PRODUCTION_ATTESTATION=real-vision-controller-run-001 pool-cli --project demo production-evidence-desktop-vision target/desktop-vision-evidence --production-vision --trace worlds/demo/output/control/desktop-recognition/external-vision-trace.json --external-action-id=real-vision-action-001 --evidence-bundle=target/desktop-vision-evidence/desktop-vision-production-evidence-bundle.json
```

Agent/Hermes session smoke：

```bash
cargo run -p pool-core --example stage_agent_sessions
```

Runtime snapshot smoke：

```bash
cargo run -p pool-core --example export_runtime_snapshot
```

MCP resource smoke：

```bash
cargo run -p pool-core --example read_mcp_resources
```

Runtime HTTP smoke：

```bash
cargo run -p pool-core --example serve_runtime_http -- target/runtime-http-smoke/pool-runtime.sqlite once
cargo run -p pool-core --example serve_runtime_http -- target/runtime-http-smoke/pool-runtime.sqlite --bind=127.0.0.1:4788 --registry=target/runtime-http-smoke/runtime-registry.json
curl 'http://127.0.0.1:4788/api/discovery'
curl 'http://127.0.0.1:4788/api/runtime-registry'
curl 'http://127.0.0.1:4788/api/runtime-graph'
curl 'http://127.0.0.1:4788/api/runtime-execution-plan'
curl -X POST 'http://127.0.0.1:4788/api/runtime-execution-plan/run-next' \
  -H 'Content-Type: application/json' \
  -d '{"project_slug":"demo"}'
curl 'http://127.0.0.1:4788/api/runtime-budget'
curl 'http://127.0.0.1:4788/api/runtime-preflight'
curl 'http://127.0.0.1:4788/api/runtime-handoff'
curl 'http://127.0.0.1:4788/api/prompts'
curl 'http://127.0.0.1:4788/api/prompts?name=pool_software_handoff&project_slug=demo&adapter_id=blender&action_kind=ExecuteCli'
curl 'http://127.0.0.1:4788/api/provider-contracts?provider_id=triposplat'
curl 'http://127.0.0.1:4788/api/software-contracts?adapter_id=unreal'
curl 'http://127.0.0.1:4788/api/unreal-mcp-bridge'
curl 'http://127.0.0.1:4788/api/mcp?uri=pool%3A%2F%2Funreal-mcp-bridge'
curl 'http://127.0.0.1:4788/api/projects'
curl 'http://127.0.0.1:4788/api/events?limit=24'
curl 'http://127.0.0.1:4788/api/events/stream?limit=24'
curl -i 'http://127.0.0.1:4788/api/events/ws?limit=24'
curl -X POST http://127.0.0.1:4788/api/adapter-health \
  -H 'Content-Type: application/json' \
  -d '{"providers":[{"provider_id":"worldlabs-marble","execution_mode":"mock"}],"software_adapters":[{"adapter_id":"unreal","priority":"ApiMcp"}]}'
curl -X POST http://127.0.0.1:4788/api/agent-sessions \
  -H 'Content-Type: application/json' \
  -d '{"kind":"hermes","project_slug":"demo","instruction":"inspect Unreal import queue"}'
curl -X POST http://127.0.0.1:4788/api/agent-sessions \
  -H 'Content-Type: application/json' \
  -d '{"kind":"hermes","project_slug":"demo","endpoint":"http://127.0.0.1:8787/hermes","instruction":"inspect Unreal import queue","execute":true,"timeout_ms":2000}'
curl -X POST http://127.0.0.1:4788/api/agent-sessions \
  -H 'Content-Type: application/json' \
  -d '{"kind":"agent_cli","project_slug":"demo","command_id":"echo","title":"Execute allowed command","command":"/bin/echo runtime-agent-ok","tools":["cli"],"execute":true,"allowed_commands":["/bin/echo","echo"]}'
curl -i 'http://127.0.0.1:4788/api/agent-sessions/ws?session_id=<agent-session-id>'
curl -X POST http://127.0.0.1:4788/api/workflow-runs \
  -H 'Content-Type: application/json' \
  -d '{"project_slug":"demo","title":"Runtime local content burst","prompt":"run creative input to 3DGS to Unreal to outputs","source_inputs":["worlds/demo/source/0-reference.png"],"duration_ms":12000,"agent_mode":"stage","three_dgs_mode":"auto","unreal_mode":"auto"}'
curl -X POST http://127.0.0.1:4788/api/output-packages \
  -H 'Content-Type: application/json' \
  -d '{"project_slug":"demo","node_id":"outputs","title":"Runtime output package","source_assets":["worlds/demo/output/1-world.glb"],"duration_ms":12000}'
curl http://127.0.0.1:4788/api/output-packages?project=demo
curl -X POST http://127.0.0.1:4788/api/output-packages/results \
  -H 'Content-Type: application/json' \
  -d '{"project_slug":"demo","node_id":"outputs","target":"game","status":"succeeded","runtime":"Unreal","adapter_id":"unreal","message":"play-in-editor viewport verified","artifacts":["unreal://level/demo_content_burst"],"metrics":[{"label":"fps","value":"60"}]}'
curl -X POST http://127.0.0.1:4788/api/handoff-packages \
  -H 'Content-Type: application/json' \
  -d '{"project_slug":"demo","node_id":"agent","title":"Runtime handoff package","output_dir":"worlds/demo/output","include_snapshot":true}'
```

Pool CLI smoke：

```bash
cargo run -p pool-core --example serve_runtime_http -- target/pool-cli-smoke/pool-runtime.sqlite once
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo status
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo tasks
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo events --limit 24
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo adapters
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo integration-readiness
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo provider-contracts triposplat
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo software-contracts unreal
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo unreal-mcp-bridge
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo node-context
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo mcp pool://tasks
OPENAI_API_KEY=sk-test-local cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo set-api-key openai-image-2 --api-key-env OPENAI_API_KEY --metadata owner=local-smoke
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo api-keys
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo runtime-budget
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo runtime-preflight
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo runtime-execution-plan
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo runtime-run-next
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo runtime-handoff
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo provider-health world-labs-marble --execution-mode mock
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo run-provider world-labs-marble --execution-mode mock --no-approval --prompt "CLI provider smoke 3DGS" --output-dir worlds/demo/output
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo provider-request-metadata <provider-request-id>
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo adapter-health --software-only
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo software-health blender --priority SkillsCli
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo run-software blender --action execute-cli --priority SkillsCli --title "Blender CLI smoke" --payload-json '{"command":"/bin/echo blender-runtime-ok","allowed_commands":["/bin/echo","echo"],"timeout_ms":2000,"max_output_bytes":1024}'
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo output-package --node-id outputs --title "CLI output package" --source-asset worlds/demo/output/1-world.glb --duration-ms 12000
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo handoff-package --node-id agent --title "CLI handoff package" --output-dir worlds/demo/output --include-snapshot
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo agent-session agent-cli --command-id echo --title "Agent CLI echo" --command "/bin/echo pool-agent-ok" --tool cli --execute --allowed-command /bin/echo --allowed-command echo --timeout-ms 2000
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo agent-transcript <agent-session-id>
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo agent-stream <agent-session-id> --limit 24
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo run-software touchdesigner --action run-viewport --priority DesktopRecognition --title "TouchDesigner desktop cue" --payload-json '{"instruction":"find TouchDesigner perform mode and trigger cue 1","target_window":"TouchDesigner","visual_targets":["Perform","Cue 1","Output"]}'
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo desktop-requests
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo desktop-run-next --controller-id local-vision-dry-run --status succeeded
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo desktop-result <software-action-id> --status succeeded --message "desktop controller finished" --artifact worlds/demo/output/control/desktop-recognition/trace.json --result-json '{"controller":"desktop-vision"}'
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo run-node <3dgs-node-id> --execution-mode mock --prompt "CLI smoke 3DGS run"
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo run-workflow --title "CLI local content burst" --prompt "run creative input to 3DGS to Unreal to outputs" --source-input worlds/demo/source/0-reference.png --agent-mode stage --three-dgs-mode mock --unreal-mode mock --duration-ms 12000
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo approve-task <task-id>
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo cancel-task <task-id>
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo retry-task <task-id>
```

Pool MCP stdio smoke：

```bash
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' \
  '{"jsonrpc":"2.0","id":3,"method":"prompts/list","params":{}}' \
  '{"jsonrpc":"2.0","id":4,"method":"resources/read","params":{"uri":"pool://status"}}' \
  | cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo serve-mcp

printf '%s\n' \
  '{"jsonrpc":"2.0","id":5,"method":"prompts/get","params":{"name":"pool_software_handoff","arguments":{"project_slug":"demo","adapter_id":"blender","action_kind":"ExecuteCli"}}}' \
  | cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo serve-mcp

printf '%s\n' \
  '{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"pool_agent_transcript","arguments":{"session_id":"<agent-session-id>"}}}' \
  | cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo serve-mcp

printf '%s\n' \
  '{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"pool_agent_stream","arguments":{"session_id":"<agent-session-id>","limit":24}}}' \
  | cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo serve-mcp

printf '%s\n' \
  '{"jsonrpc":"2.0","id":8,"method":"tools/call","params":{"name":"pool_provider_request_metadata","arguments":{"provider_request_id":"<provider-request-id>"}}}' \
  | cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo serve-mcp

printf '%s\n' \
  '{"jsonrpc":"2.0","id":9,"method":"tools/call","params":{"name":"pool_worker_self_checks","arguments":{"output_root":"target/pool-worker-self-checks-mcp","software_adapter":"resolve"}}}' \
  | cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo serve-mcp

printf '%s\n' \
  '{"jsonrpc":"2.0","id":10,"method":"tools/call","params":{"name":"pool_run_software","arguments":{"adapter_id":"blender","action_kind":"ExecuteCli","priority":"SkillsCli","task_title":"MCP Blender CLI smoke","payload_json":{"command":"/bin/echo mcp-blender-ok","allowed_commands":["/bin/echo","echo"],"timeout_ms":2000,"max_output_bytes":1024}}}}' \
  | cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo serve-mcp
```

ComfyUI smoke：

```bash
POOL_COMFYUI_ENDPOINT=http://127.0.0.1:8188 cargo run -p pool-core --example comfyui_smoke
```

提交 ComfyUI workflow JSON：

```bash
cargo run -p pool-core --example comfyui_smoke -- /path/to/workflow_api.json target/comfyui-smoke
cargo run -p pool-core --example comfyui_smoke -- /path/to/workflow_api.json target/comfyui-smoke target/comfyui-events.sqlite index-assets
```

Kling smoke：

```bash
cargo run -p pool-core --example kling_smoke
POOL_KLING_API_KEY=... cargo run -p pool-core --example kling_smoke -- request.json target/kling-smoke
```

OpenAI image smoke：

```bash
cargo run -p pool-core --example openai_image_smoke
OPENAI_API_KEY=... cargo run -p pool-core --example openai_image_smoke -- request.json target/openai-image-smoke
```

Generic media gateway smoke：

```bash
cargo run -p pool-core --example generic_media_smoke
POOL_MEDIA_GATEWAY_ENDPOINT=http://127.0.0.1:8787 cargo run -p pool-core --example generic_media_smoke -- nano-banana-pro request.json target/generic-media-smoke
```

默认会生成：

- `target/pool-runtime-smoke/pool-runtime.sqlite`
- `target/pool-runtime-smoke/worlds/demo/project.json`
- `target/pool-runtime-smoke/worlds/demo/workflow.json`
- `target/pool-runtime-smoke/worlds/demo/scene.json`

Web 原型：

```bash
node --check apps/web-prototype/app.js
node scripts/web-runtime-node-context-smoke.mjs app.js
node scripts/web-runtime-node-context-smoke.mjs apps/web-prototype/app.js
python3 -m http.server 4173
```

访问 `http://localhost:4173/apps/web-prototype/`。

连接本地 Runtime HTTP：

```bash
cargo run -p pool-core --example serve_runtime_http -- target/runtime-http-smoke/pool-runtime.sqlite --bind=127.0.0.1:4788
python3 -m http.server 4173
```

访问 `http://localhost:4173/apps/web-prototype/?runtime=local`；如果 runtime 不在默认端口，可用 `?runtime_registry=/target/runtime-http-smoke/runtime-registry.json`、`?runtime=local&runtime_ports=4788,4789` 或 `?runtime_endpoints=http://127.0.0.1:4878` 指定候选地址。

## Web 原型直接运行

直接用浏览器打开：

```bash
open apps/web-prototype/index.html
```

或启动一个静态服务器：

```bash
python3 -m http.server 4173
```

然后访问 `http://localhost:4173/apps/web-prototype/`。
