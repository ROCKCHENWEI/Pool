# Runtime HTTP Server

## 目标

`RuntimeHttpServer` 是 Pool 本地运行核心的最小 HTTP 入口。它不引入新的状态源，只把 SQLite 中的 `RuntimeSnapshot`、snapshot-backed MCP resources 和受控任务动作暴露给 Web/SwiftUI 控制台、Hermes、Agent CLI 和外部自动化工具。

当前实现使用 Rust 标准库 TCP listener，避免在运行框架阶段引入额外 web framework。

## API

- `GET /api/health`：返回 runtime 状态、project filter 和统计；支持 `?project=slug` / `?project=*` query override。
- `GET /api/runtime-registry`：返回轻量本地 runtime 注册表，包含 `base_url`、`runtime_endpoint`、`discovery_url`、`well_known_url`、project filter 和 endpoint manifest；同一结构也可由 `RuntimeHttpServer::write_runtime_registry()` 写成 JSON 文件，供 Web `?runtime_registry=`、Hermes、Agent CLI 或桌面 controller 自动发现 runtime。
- `GET /api/discovery` / `GET /.well-known/pool-runtime.json`：返回 Pool runtime 服务描述、base URL、capabilities、endpoint manifest、MCP resource manifest、MCP prompt manifest、projects 和当前统计，供 Web/桌面端/Agent 作为 runtime 服务注册表读取。
- `GET /api/snapshot`：返回完整 `RuntimeSnapshot` JSON；支持 `?project=slug` / `?project=*` query override。
- `GET /api/projects`：返回全部 SQLite project record，并给 Web 控制台项目选择器提供当前 `project_filter`。
- `GET /api/events`：返回 `workflow_events` 增量切片；支持 `?project=slug` / `?project=*`、`?after_id=<event-id>` 和 `?limit=1..200`。
- `GET /api/events/stream`：在常驻 HTTP server 模式下保持 `text/event-stream` SSE 连接并持续推送新增 `workflow_events`；支持 `?project=slug` / `?project=*`、`?last_event_id=<event-id>`、`?poll_ms=500..30000` 和 `?limit=1..200`。`handle_request` 测试入口仍返回单次 SSE 切片。
- `GET /api/events/ws`：在常驻 HTTP server 模式下接受 WebSocket upgrade，发送 `pool-runtime-events` prelude、`runtime-event` JSON 文本帧和 heartbeat；支持 `?project=slug` / `?project=*`、`?last_event_id=<event-id>`、`?poll_ms=500..30000` 和 `?limit=1..200`。未带 upgrade headers 时返回 426，Web 可降级到 SSE 或轮询。
- `GET /api/agent-sessions/transcript?session_id=<agent-session-id>`：按已登记的 `agent_sessions.id` 读取本地 transcript JSON，返回 session metadata、token 和 transcript 内容；不接受任意文件路径。
- `GET /api/agent-sessions/stream?session_id=<agent-session-id>`：返回该 Agent/Hermes session 的 SSE 日志流，先发送 `agent-transcript` 帧，再持续推送包含该 session id 的 `runtime-event`；常驻 server 模式会保持连接。
- `GET /api/agent-sessions/ws?session_id=<agent-session-id>`：返回该 Agent/Hermes session 的 WebSocket 日志流，先发送 `agent-session` transcript JSON 文本帧，再持续推送包含该 session id 的 `runtime-event`；未带 upgrade headers 时返回 426，并给出 SSE fallback。
- `GET /api/resources`：返回可读的 Pool MCP resource 列表。
- `GET /api/prompts`：返回可用的 Pool MCP prompt / Agent runbook 列表；`GET /api/prompts?name=pool_software_handoff&adapter_id=blender` 会按 query 参数生成同一份 prompt 文本，供不走 stdio MCP 的外部 Agent/Hermes/controller 读取。
- `GET /api/runtime-graph`：返回从 `RuntimeSnapshot` 派生的可执行运行图，合并 workflow nodes、task 状态、连接标签、控制通道和 task 类型。
- `GET /api/runtime-execution-plan`：返回从 `RuntimeSnapshot` 派生的可执行步骤计划，把 workflow 节点按拓扑排序，合并节点状态、审批门、Provider/软件合同、控制优先级和推荐 CLI/MCP 动作。
- `POST /api/runtime-execution-plan/run-next`：选择 execution plan 的下一步、指定 `node_id` 或指定 `task_id`，默认只返回 preview；传 `execute:true` 后才复用 `/api/nodes/run`、`/api/tasks/approve` 或 `/api/tasks/retry` 执行推荐动作。审批步骤必须额外传 `allow_approval:true`。
- `GET /api/runtime-budget`：返回从 `RuntimeSnapshot` 派生的预算与凭证摘要，合并 task 估算 token、待审批 token、Agent token budget、脱敏 Provider Key 就绪状态、Provider 请求数和审批门。
- `GET /api/runtime-preflight`：返回从 `RuntimeSnapshot` 派生的运行前检查，合并 workflow graph、审批门、缺失 Provider credential、失败/可重试任务、desktop recognition handoff、Agent token budget，并给出建议 CLI next actions；桌面接管会优先给出 `desktop-run-next` 推进命令，同时保留 `desktop-requests` 检查命令。
- `GET /api/runtime-handoff`：返回从 `RuntimeSnapshot` 派生的执行接管 runbook，把 preflight、可运行节点、本地 worker smoke、离线接管包、审批、重试、凭证和 desktop recognition handoff 整理成 Hermes、Agent CLI、桌面 controller 与人工 operator 可消费的 lanes、命令列表和 5 人内容爆发团队角色绑定。
- `GET /api/core-architecture-readiness`：返回从 `RuntimeSnapshot` 派生的核心架构完成门槛，单独检查 project/workflow、节点执行计划、Hermes/Agent、Provider/软件合同、Unreal 优先、本地 indexed assets、三类输出和 handoff/MCP；它不替代严格 PRD completion gate，真实 Provider、真实软件和外部视觉模型证据仍由 `/api/prd-completion-gate` 阻断。
- `GET /api/core-architecture-gate`：返回 `core-architecture-readiness.architecture_gate` 的硬门 wrapper；传 `?require_ready=true`、`?require_complete=true` 或 `?fail_if_incomplete=true` 时，未满足本地核心架构门槛会返回 HTTP 428 和 `core_architecture_gate_incomplete`，供 Agent/Hermes/CI 在不读取完整 readiness 的情况下做硬性阻断。
- `GET /api/core-architecture-packages`：从 SQLite asset ledger 和本地 `control/core-architecture/8-core-architecture-package-manifest.json` 恢复已生成核心架构证明包 catalog，返回 ready 状态、manifest 路径、独立核心架构 gate 文件、命令、MCP resources 和缺失本地文件列表；远程 provider URL 只作为 provenance。
- `POST /api/core-architecture-package`：把当前核心架构 readiness、独立 core architecture gate、runtime graph、execution plan、handoff、output packages、严格 PRD completion gate、manifest 和可选 snapshot 写入 `output/control/core-architecture/`，并同步写入 task、assets 和 events。该入口只归档本地运行框架闭环证据；如果 PRD completion gate 未 ready，manifest 会保留生产证据缺口，不会替代真实 Provider/软件/视觉证据导入。
- `cargo run -q -p pool-core --example run_prd_readiness_smoke -- target/core-architecture-readiness-smoke`：可作为本地核心架构 smoke，运行后会要求 `/api/core-architecture-gate?require_ready=true` 返回 200，并写出包含 `2-core-architecture-gate.json` 的核心架构证明包；生产证据仍需另走 closeout/import。
- `GET /api/prd-readiness`：返回从 `RuntimeSnapshot` 派生的 PRD readiness 审计，把 Pool 总体规划拆成要求级 `ready` / `partial` / `blocked`，并给出证据、缺口、source resources、next actions 和 `completion_gate`；`completion_gate` 会明确当前 snapshot 是否足以标记 PRD 完成、列出未完成要求，并给出 closeout/readiness 证明命令。生产硬化 evidence 会单独汇总 desktop recognition controller callback、vision trace、本地 trace smoke 和真实外部视觉模型证据；支持 `?project=slug` / `?project=*` query override。
- `GET /api/prd-completion-gate`：返回单独的 PRD 完成门槛 wrapper，包含 `completion_gate.ready_for_completion`、未完成要求和 proof commands；传 `?require_complete=true` 或 `?fail_if_incomplete=true` 时，未完成快照会返回 HTTP 428 与 `prd_completion_gate_incomplete`，供 Hermes/Agent/CI 收口脚本硬性阻断。
- `GET /api/prd-completion-packages`：从 SQLite asset ledger 和本地 `control/prd-completion/4-prd-completion-package-manifest.json` 恢复已生成 PRD 完成证明包 catalog，返回 ready 状态、completion gate 状态、manifest/readiness/requirements/snapshot 路径、命令、operator checklist 和缺失本地文件列表；远程 provider URL 只作为 provenance。
- `POST /api/prd-completion-package`：把当前 PRD readiness、completion gate、production evidence requirements、manifest 和可选 runtime snapshot 写入 `output/control/prd-completion/`，并同步写入 task、assets 和 events。该入口只归档当前状态；如果 gate 未 ready，manifest 会保留 `ready_for_completion:false` 和未完成要求，不会替代真实生产证据导入。
- `GET /api/production-evidence/requirements`：返回从同一 `RuntimeSnapshot` 派生的生产证据需求清单，列出 required Provider、required software adapter、desktop vision controller 的当前状态、缺失项、必填 bundle 字段、本地文件策略和推荐 CLI 命令；不写 SQLite，用于在生成 template、validate 或 import 前让 Agent/Hermes/operator 对齐真实外部证据缺口。
- `GET /api/production-evidence/handoff`：返回 missing-only 生产证据交付包，组合 requirements、`evidence_tasks`、缺口 bundle、validate/import/readiness 命令、HTTP/MCP 入口和 operator checklist；不写 SQLite，可由 `pool-cli production-evidence-handoff` 写出完整 JSON 交给 Hermes、外部 Provider worker、软件操作员或视觉 controller operator。
- `POST /api/production-evidence/tasks/claim`：把一个只读缺口 evidence task 转成可追踪 runtime task，并写出本地 `control/production-evidence/claims/*-claim.json`。请求包含 `task_id`，可选 `assignee`、`role`、`output_root` 和 `source`；响应返回 runtime task、claim file、item template、validate item 和 submit item 命令。它用于外部 worker/operator/controller 正式领取任务，不导入生产证据。
- `GET /api/workflow-context?workflow_id=<workflow-id>`：返回单个 workflow 的运行上下文；不传 `workflow_id` 时返回可下钻 workflow 目录。返回内容与 `pool://workflow/<workflow-id>` 同源，包含 graph、node_states、tasks、assets、Provider 请求、软件动作、Agent session 和审批摘要。
- `GET /api/node-context?node_id=<node-id>`：返回单个 workflow 节点的运行上下文；不传 `node_id` 时返回可下钻节点目录。节点详情包含绑定 Provider/软件 adapter 配置、控制优先级、建议 MCP tools 和 CLI commands 的 `control_context`。
- `GET /api/mcp?uri=pool://tasks`：读取指定 MCP resource，例如 `pool://status`、`pool://tasks`、`pool://assets`、`pool://adapters`、`pool://integration-readiness`、`pool://provider-contracts`、`pool://provider-contracts/<provider-id>`、`pool://provider-gateway-worker`、`pool://software-contracts`、`pool://software-contracts/<adapter-id>`、`pool://unreal-mcp-bridge`、`pool://desktop-recognition-contract`、`pool://workflow`、`pool://workflow/<workflow-id>`、`pool://runtime-graph`、`pool://runtime-budget`、`pool://runtime-preflight`、`pool://runtime-execution-plan`、`pool://runtime-handoff`、`pool://runtime-handoff-packages`、`pool://prd-readiness`、`pool://prd-completion-gate`、`pool://prd-completion-packages`、`pool://production-evidence-requirements`、`pool://production-evidence-tasks`、`pool://production-evidence-run-plan`、`pool://production-evidence-handoff`、`pool://production-evidence-item-template`、`pool://production-evidence-item-template/<task-id>`、`pool://output-packages`、`pool://node-context/<node-id>`、`pool://provider-requests`、`pool://software-actions`、`pool://desktop-recognition`、`pool://agent-sessions`、`pool://snapshot`。`pool://adapters` 会返回 Provider/软件矩阵、别名、控制优先级和本地优先策略；`pool://integration-readiness` 会返回 Provider、软件 adapter 和 Agent/Hermes 的接入就绪矩阵；`pool://provider-contracts/<provider-id>` 会返回 AI media gateway / 3DGS gateway / native adapter 的机器可读接入合同；`pool://provider-gateway-worker` 会返回本地 HTTP forwarder 的 CLI、route、upstream 和 endpoint env 合同；`pool://software-contracts/<adapter-id>` 会返回外部软件控制的 health/action body、控制优先级链、API/MCP、Skills/CLI、桌面识别和人工接管路径；`pool://unreal-mcp-bridge` 会返回 Unreal 插件/gateway 侧 `pool_unreal_action`、`mcp_payload`、tool contracts、response contract 和 operator checks；`pool://desktop-recognition-contract` 会返回桌面 controller 的请求、队列、回填状态和证据合同；`pool://runtime-handoff-packages` 会从 asset ledger 和本地 manifest 恢复已生成 runtime handoff package catalog；`pool://prd-readiness` 会返回 Pool 总体规划的要求级机器审计；`pool://prd-completion-gate` 会返回同源完成门槛；`pool://prd-completion-packages` 会恢复已生成 PRD 完成证明包 catalog；`pool://production-evidence-requirements` 会返回真实生产证据缺口和导入前检查清单；`pool://production-evidence-tasks` 会返回真实生产证据缺口任务队列；`pool://production-evidence-run-plan` 会返回七段真实生产证据执行计划；`pool://production-evidence-handoff` 会返回 Agent/Hermes 可读的生产证据分派上下文；`pool://production-evidence-item-template` 会列出每个缺口 task 的 item template URI，`pool://production-evidence-item-template/<task-id>` 会返回只读单项 evidence item wrapper；`pool://output-packages` 会返回视频、游戏、交互艺术三类本地 deliverable 的 ready/missing catalog；单 workflow URI 会返回该 workflow 的 graph、node_states、tasks、assets、Provider 请求、软件动作和 Agent session 摘要。
- `GET /api/api-keys`：读取脱敏 Provider credential 状态和 `pool_api_key_audit` 轮换审计，不返回 secret；可用 `rotation_days=<days>` 覆盖默认 90 天审计窗口。
- `GET /api/adapters`：返回 runtime 注册的 Provider 矩阵、外部软件 adapter 矩阵、Provider alias map 和固定控制优先级；这是 capability registry，不代表实时健康探测。
- `GET /api/integration-readiness`：返回 snapshot-backed Provider、软件 adapter 和 Agent/Hermes 接入就绪矩阵；它聚合 adapter catalog、api key、provider_requests、software_actions、tasks 和 agent_sessions，并输出 5 人团队 lane、每行 next action 与按优先级排序的 run plan，只读不写 SQLite。
- `GET /api/core-architecture-readiness` 与 `pool://core-architecture-readiness`、`pool-cli core-architecture-readiness`、MCP tool `pool_core_architecture_readiness` 共用 payload；用于在生产证据未收齐前证明本地核心架构闭环。`GET /api/core-architecture-gate`、`pool://core-architecture-gate`、`pool-cli core-architecture-gate --require-ready` 和 MCP tool `pool_core_architecture_gate` 提供同源硬门；`GET /api/core-architecture-packages`、`pool://core-architecture-packages`、`pool-cli core-architecture-packages` 和 MCP tool `pool_core_architecture_packages` 会恢复已生成证明包 catalog；`POST /api/core-architecture-package`、`pool-cli core-architecture-package` 和 MCP tool `pool_core_architecture_package` 会把这组证据物化成本地 indexed package，方便 Agent/Hermes 交接复核。`GET /api/prd-completion-packages`、`pool://prd-completion-packages`、`pool-cli prd-completion-packages` 和 MCP tool `pool_prd_completion_packages` 会恢复已生成 PRD 完成证明包 catalog；`POST /api/prd-completion-package`、`pool-cli prd-completion-package` 和 MCP tool `pool_prd_completion_package` 写出同源证明包。
- `GET /api/provider-contracts?provider_id=<provider-id>`：返回 Provider 接入合同；不传 `provider_id` 时返回全部合同。Midjourney、Nano Banana Pro、Suno 暴露通用 AI media gateway 合同，World Labs Marble、TripoSplat、SAM-3D、Spark、群核科技暴露 3DGS gateway 合同，ComfyUI、Kling、OpenAI image-2 暴露 native adapter 合同。合同包含 runtime 调用 body、gateway submit/poll path、状态字段、输出字段、环境变量和本地文件优先策略。
- `GET /api/provider-gateway-worker`：返回本地 Provider gateway worker 合同，包含 `pool-cli provider-gateway-worker` 启动命令、AI media/3DGS route、上游 worker 返回字段、Pool adapter endpoint env 和本地文件优先策略。
- `GET /api/provider-conformance-packages`、`GET /api/software-conformance-packages`、`GET /api/agent-conformance-packages`、`GET /api/integration-conformance-packages`：从 SQLite asset ledger 与本地 manifest 恢复已生成 conformance package catalog，返回 ready 状态、manifest/runner/preflight/contract 路径、命令、缺失文件和本地优先策略；provider URL 只作为 provenance。
- `POST /api/provider-conformance-packages`：把单个 AI/3DGS Provider 的 contract、Provider gateway worker contract、runbook、preflight、runner script 和 manifest 写入 `output/control/provider-conformance/<provider-id>/`，同步入 task、assets 和 events。runner 支持 `--preflight`、`local` 与 `run`，用于把真实厂商 SDK/HTTP worker 验收任务交给 Agent 或具体 Provider 接入操作者。
- `POST /api/integration-conformance-packages`：一次性把 Provider、软件 adapter 和 Agent/Hermes 的 conformance 子包写入 `output/control/integration-conformance/`，并额外写顶层 request、runbook、runner script 和 manifest。默认覆盖 9 个 required Provider、11 个 required software adapter 与 `agent_kind=all`；请求可传 `providers[]`、`software_adapters[]`、`agent_kind` 和 `include_providers/include_software/include_agent` 缩小范围。
- `GET /api/software-contracts?adapter_id=<adapter-id>`：返回外部软件控制合同；不传 `adapter_id` 时返回全部合同。Unreal/Hermes 暴露专用 API/MCP route，其他软件 adapter 暴露通用 `generic_software_api_mcp` route、`pool-cli software-api-bridge-worker <adapter-id>` 本地 worker 命令、endpoint env、受控 CLI、桌面识别兜底和人工接管路径。合同包含 `/api/software-health` body、`/api/software-actions` body、支持 action kind、优先级链、环境变量和 fallback policy。
- `POST /api/software-conformance-packages`：把单个软件 adapter 的控制合同、`conformance_runbook`、preflight、runner script 和 manifest 写入 `output/control/software-conformance/<adapter-id>/`，同步入 task、assets 和 events。runner 支持 `--preflight`、`local` 与 `run`，用于把 Resolve、Blender、Unreal、TouchDesigner 等真实软件 bridge 验收任务交给 Agent 或具体操作者。
- `POST /api/agent-conformance-packages`：把 Agent session 控制合同、Hermes/Agent CLI runbook、preflight、runner script 和 manifest 写入 `output/control/agent-conformance/<kind>/`，同步入 task、assets 和 events。`kind` 支持 `all`、`hermes` 和 `agent-cli`，用于把 Hermes 内嵌控制与 Agent CLI 受控执行验收任务交给 Agent 或具体操作者。
- `GET /api/unreal-mcp-bridge`：返回 Unreal 插件或本地 gateway 实现方需要遵守的 bridge contract。它固定 `pool_unreal_action` / `mcp_payload` wrapper、默认 `/health` / `/mcp` transport、Unreal tool contracts、响应字段和本地 artifact policy；真实 Unreal 插件执行仍由外部插件/gateway 完成。
- `POST /api/adapter-health`：批量调用 Provider 与软件 adapter health；可传 `providers`、`software_adapters`、`include_providers` 和 `include_software`，单项失败不会中断整个 fan-out，不创建 task，也不写入请求/动作账本。
- `POST /api/provider-health`：按单个 `provider_id` 调用对应 `ProviderAdapter.health()`，可传 `endpoint`、`api_key` 和 `execution_mode`；该接口只做连接/配置检查，不创建 task，也不写入 `provider_requests`。
- `GET /api/provider-requests/metadata?provider_request_id=<provider-request-id>`：按已登记的 `provider_requests.id` 读取本地 metadata/handoff JSON，返回 provider/task/project metadata；不接受任意文件路径。
- `POST /api/software-health`：按单个 `adapter_id` 调用对应 `SoftwareAdapter.health()`，可传 `priority` 与 `payload_json.endpoint`；该接口只做软件控制路径检查，不创建 task，也不写入 `software_actions`。
- `POST /api/nodes/run`：按 workflow `node_id` 执行单个节点；Provider 节点分派到 `/api/provider-runs`，Agent/Hermes 节点分派到 `/api/agent-sessions`，软件节点分派到 `/api/software-actions`，输出节点分派到 `/api/output-packages`，普通节点只创建 runtime task。节点运行会从同一份 node context 读取 `control_context`，并把它写入 Provider request ledger 或软件 action payload，方便 Agent/Hermes 追踪这次执行来自哪个控制入口。
- `POST /api/tasks`：创建 runtime task，写入 `workflow_events`，返回 task 与更新后的 snapshot。
- `POST /api/api-keys`：保存或更新 Provider API Key，写入 `api_keys`，返回脱敏 key 状态和更新后的 snapshot；设置 `POOL_CREDENTIAL_STORE=keychain` 后新 key 会写入 macOS Keychain，SQLite 只保存引用；设置 `POOL_CREDENTIAL_PASSPHRASE` 后，新 key 会以本地 AES-256-GCM 封装保存。
- `POST /api/workflow-runs`：执行本地内容爆发闭环，串起默认运行蓝图、本地项目包、Agent/Hermes 决策、3DGS、Unreal 和三类输出交付包；`agent_mode` 支持 Hermes staging/HTTP/skip，`three_dgs_mode` 与 `unreal_mode` 支持 `auto`、真实 adapter 强制模式和本地 mock。
- `POST /api/provider-runs`：执行 Provider run，写入 tasks、`provider_requests`、assets 和 `workflow_events`。当前可调度 ComfyUI、Kling、OpenAI image-2、Midjourney、Nano Banana Pro、Suno、3DGS gateway 和本地 mock 3DGS；请求未传 `cost_estimate_tokens` 时由 adapter 自动估算，未传 `requires_approval` 且命中高成本阈值时进入 `waiting_approval`，同时在 output dir 写入 `.0-provider-approval__<provider>-request.json` 本地审批请求包；未接入 Provider 返回 `provider_not_executable`，缺少 endpoint/凭证返回 `provider_not_configured`。
- `GET /api/production-evidence/requirements`：只读生产证据 doctor 入口，按 PRD required list 输出 provider/software/desktop vision 生产证据要求和当前缺口；Hermes 或 Agent CLI 可先读它，再决定生成脚手架、补真实 upstream evidence、校验 bundle 或导入证据。
- `GET /api/production-evidence/tasks`：只读生产证据任务队列入口，从同一份 requirements 中提取当前缺失的 provider/software/desktop vision 任务，返回 `task_id`、`kind`、`target_id`、必填字段、artifact policy、item-template 命令和 submit-item 命令。适合外部 worker/operator/controller 领取单个任务。
- `POST /api/production-evidence/tasks/claim`：生产证据任务领取入口，写入 runtime task、事件流和本地 claim JSON，保留 assignee/role/source/output_root 和单项 item template，用于把只读缺口队列推进到可审计执行状态。
- `GET /api/production-evidence/run-plan`：只读真实生产证据运行计划入口，把当前 requirements、PRD completion gate 和 evidence task 队列串成 Provider matrix、software matrix、desktop vision、merge、closeout preflight、closeout import、completion proof 七段执行计划。可传 `output_root=<dir>` 和 `source=<label>`；响应包含每段命令、bundle 输出路径、ready 条件、HTTP/MCP 入口和 operator checklist，不写 SQLite，也不会把本地 mock 视为生产证据。
- `GET /api/production-evidence/handoff`：只读生产证据收口包入口，默认把当前缺口转成 missing-only bundle 与 operator checklist，适合直接交给外部执行者补齐真实 evidence。
- `GET /api/production-evidence/handoff-packages`：只读生产证据交付包 catalog，从 SQLite asset ledger 与本地 manifest/runner preflight 恢复已生成 package 的 ready/missing 状态、manifest/run-plan/bundle/runner 路径、item 文件数、Provider gateway worker 启动命令和 software bridge worker 启动命令。和 `pool://production-evidence-handoff-packages`、`pool-cli production-evidence-handoff-packages`、MCP tool `pool_production_evidence_handoff_packages` 共用 payload，供 Web/Hermes/Agent 在重连后恢复最近交付包。
- `POST /api/production-evidence/handoff-packages`：把生产证据 requirements、task queue、missing-only handoff、run-plan、bundle、每个缺口任务的 item JSON/隐藏 template provenance、可执行 runner script、runner preflight 合同和可选 snapshot 写入 `output/control/production-evidence/`，同步写入 task、assets 和 `workflow_events`。固定写出 `4-production-evidence-run-plan.json`、`5-production-evidence-bundle.json`、`6-production-evidence-package-manifest.json`、`7-production-evidence-runner.sh` 和 `8-production-evidence-runner-preflight.json`，可选 `9-runtime-snapshot.json`。runner script 的 Provider 阶段可用 shared media/3DGS gateway endpoint 覆盖家族，也可使用每个 required Provider 的 endpoint env；OpenAI image-2 需要 `OPENAI_API_KEY` 或 provider-specific key env；生产证明需要全局或 per-provider production attestation。软件阶段需要 Unreal/Hermes endpoint，以及 Blender/ComfyUI/Resolve/Unity/TouchDesigner/MadMapper/Nuke/动捕/剪辑等 adapter 的 `POOL_*_ENDPOINT` 或 `POOL_*_COMMAND`，并需要每个 adapter 的 `POOL_*_ARTIFACTS` 指向本地文件路径；通用 endpoint 会走标准 API/MCP 网关合同，也可指向转发到真实插件/gateway 的 `pool-cli software-api-bridge-worker <adapter-id>`；可先运行 `7-production-evidence-runner.sh --preflight` 做机器预检；merge/closeout 阶段优先用 PATH 上的 `pool-cli`，找不到时 fallback 到 `cargo run -q -p pool-cli --`，可用 `POOL_CLI_CMD` 覆盖，只有实际使用 cargo fallback 时才要求本机有 cargo；默认执行 Provider/software matrix、merge 和 closeout preflight；设置 `POOL_RUN_DESKTOP_VISION=1` 且提供 `POOL_DESKTOP_VISION_TRACE`、`POOL_DESKTOP_VISION_EXTERNAL_ACTION_ID` 与 `POOL_DESKTOP_VISION_PRODUCTION_ATTESTATION` 才执行桌面视觉阶段，设置 `POOL_IMPORT_PRODUCTION_EVIDENCE=1` 才执行 closeout import。这是给外部 Provider worker、软件 operator、desktop vision controller 的本地交付包，不会自动导入生产证据。
- `GET /api/production-evidence/template`：生成生产证据 bundle 脚手架，不写 SQLite。响应包含 `bundle`、`artifact_plan`、`operator_checklist` 和 validate/import 命令，覆盖 9 个 required Provider、11 个软件 adapter 和外部视觉 controller。脚手架故意使用 `replace-with-real-*` external id，`ready_for_import:false`，必须由外部 worker/插件/controller 替换真实 id 和本地文件后才能 validate/import。
- `GET /api/production-evidence/item-template`：生成单项生产证据 item 脚手架，不写 SQLite。可传 `task_id=provider:midjourney:production_upstream`，或传 `kind=provider|software_action|desktop_vision&target_id=<id>`；响应中的 `.item` 可保存成本地 JSON 后交给 `POST /api/production-evidence/items`。脚手架仍使用 `replace-with-real-*` external id，必须替换真实生产证据和本地文件。同一按 task 领取语义也暴露为只读 MCP resource：`pool://production-evidence-item-template` 提供索引，`pool://production-evidence-item-template/<task-id>` 返回具体模板 wrapper。
- `GET /api/production-evidence/item-from-ledger`：从已存在的 `provider_requests`、`software_actions` 或 desktop recognition callback 账本生成单项 production evidence item 草稿，不写 SQLite。传 `provider_request_id=<id>`、`software_action_id=<id>` 或 `desktop_vision_action_id=<software-action-id>`；响应包含 `.item`、`validation.valid`、`artifact_files.complete`、`production_flags.complete` 和 `ready_for_import`。本地 mock ledger 会保留 `local_mock_*:true` 或 `local_trace_smoke:true`，不会被标记为可导入生产证据；desktop vision 分支必须有外部视觉模型标记和本地 trace 文件。
- `GET /api/production-evidence/bundle-from-ledger`：从当前 runtime ledger 批量整理 ready 的 Provider、软件动作和 desktop vision 生产证据，返回 `.bundle`、ready item wrappers、`summary`、整包 `validation`、本地 artifact 预检和 validate/import/closeout 命令，不写 SQLite。默认只把 `ready_for_import:true` 的账本项放入 `.bundle`；传 `include_incomplete=true` 时会额外返回未 ready 的 item/error 诊断，方便 Agent/Hermes 继续补证。本地 mock Provider、mock 软件动作和 dry-run desktop trace 不会进入可导入 bundle。
- `POST /api/production-evidence/validate`：校验外部真实生产运行已经完成后的证据 bundle，但不写入 SQLite。它复用导入前的整包校验，拒绝模板 external id、远程 Provider artifact/metadata URL、远程 desktop vision artifact URL 和未显式外部视觉模型的 desktop trace；`desktop_vision[]` 必须设置 `visual_model:"external"` 或 `evidence_json.external_visual_model:true`。成功时返回 `pool_production_evidence_validation`、`writes:0`、各类 evidence 计数、将要处理的 provider/software/desktop 行摘要、canonical Provider/adapter id、原始输入 id、`artifact_files` 本地文件存在性预检，以及 PRD 生产证据 coverage：9 个 required Provider、11 个 required software adapter 和外部视觉模型证据是否完整覆盖。供 Hermes、Agent CLI 或人工 operator 在导入前做 dry-run。
- `POST /api/production-evidence/merge`：把多个外部 worker/operator/controller 返回的 production evidence bundle 合并成一个 bundle，但不校验本地文件、不写入 SQLite。请求体包含 `project_slug`、`source` 和 `bundles[]`，响应返回 `pool_production_evidence_merge`、`writes:0`、输入包数量、三类 evidence 计数、合并后的 `.bundle` 以及 closeout/validate/import/readiness 推荐命令；如果嵌套 bundle 混入不同 `project_slug` 会返回 `invalid_production_evidence_merge_request`。该入口用于 Hermes/MCP 自动收口多路证据，正式入账仍必须先走 `/api/production-evidence/closeout` 或 `/api/production-evidence/validate`。
- `POST /api/production-evidence/closeout`：把 merge 和 validate 串成一个外部证据收口入口，默认 `import:false`，只返回合并结果、校验结果、`ready_for_import` 和推荐命令，保持 `writes:0`。显式传 `import:true` 时才会把合并后的 bundle 交给现有 `/api/production-evidence` 导入路径；导入成功后 closeout 顶层会返回 `completion_gate`、`ready_for_completion`、`prd_overall_status`、`prd_summary` 和完成证明命令；如果请求带 `completion_package` 对象，会在 completion gate ready 后复用 `/api/prd-completion-package` 写出本地 PRD completion package。导入仍会执行本地文件存在性、占位 id、远程 URL 和外部视觉模型校验，失败时返回 `production_evidence_closeout_import_failed` 且不写入账本。该入口适合 Hermes/Agent 在收齐多路真实证据后做一次受控 closeout。
- `POST /api/production-evidence`：导入外部真实生产运行已经完成后的证据 bundle，不重新执行 Provider 或软件动作。`providers[]` 会按 canonical Provider id 写入 `provider_requests` 并标记 `production_upstream:true`；每个 Provider item 必须包含真实 `production_attestation` 或 `evidence_json.production_attestation`，用于标识真实上游 worker/SDK run。`software_actions[]` 会按 canonical adapter id 写入 `software_actions` 并标记 `production_software:true`；每个 software item 也必须包含真实 `production_attestation` 或 `evidence_json.production_attestation`，用于标识真实软件插件/API/CLI/MCP/桌面控制运行；原始输入 id 会保留为 `input_provider_id` / `input_adapter_id`。`desktop_vision[]` 会写入 desktop recognition 结果并标记 `external_visual_model:true`；每个 desktop item 也必须包含真实 `production_attestation` 或 `evidence_json.production_attestation`，用于标识真实外部视觉/OCR/screen model controller run。导入后响应包含同源 PRD production evidence `coverage`、`artifact_files` 和刷新后的 `prd_readiness`，可用于确认生产证据是否覆盖全部 PRD required providers/adapters。导入会先预校验整包，拒绝 `replace-with` / `placeholder` / `todo` / `dummy` / `fake` 这类模板 external id/attestation，并要求 Provider artifact/metadata、software artifact、desktop trace 和 desktop vision artifacts 使用已存在的本地文件路径，远程 Provider URL 只能作为 provenance/endpoint；本地 mock/dry-run desktop trace 不能作为生产视觉模型证据。无效 bundle 返回 `invalid_production_evidence_item` 或 `missing_production_artifact_files`，不写入部分 task/provider/software 账本。
- `POST /api/production-evidence/items/validate`：校验单个外部生产证据 item，但不写 SQLite。请求 schema 与 `POST /api/production-evidence/items` 相同，后端会先包装成单项 bundle，再复用生产证据导入前校验，返回 `pool_production_evidence_item_validation`、`writes:0` 和嵌套的标准 `pool_production_evidence_validation` 报告。它用于外部 worker/operator/controller 在正式 submit 前检查本地 artifact/metadata/trace 文件、占位 id、远程 URL 和外部视觉模型标记。
- `POST /api/production-evidence/items`：导入单个外部生产证据 item，适合外部 worker/operator/controller 完成一个任务后立即回填。请求必须包含 `kind:"provider" | "software_action" | "desktop_vision"`，并提供对应的 `provider`、`software_action` 或 `desktop_vision` 对象；后端会包装成同一套 bundle 导入路径，继承本地文件、占位 id、远程 URL 和外部视觉模型校验。
- `cargo run -p pool-core --example run_prd_readiness_smoke -- --with-production-evidence target/prd-readiness-production-runner` 会先生成完整 PRD 本地证据，再额外通过 closeout 导入三路 production evidence bundle，并断言 PRD readiness 达到 `ready:10`、`partial:0` 和 `overall_status:"ready"`。
- `cargo run -p pool-core --example import_production_evidence_bundle -- target/production-evidence-import-smoke` 会基于示例 schema 生成本地 artifact/metadata fixture，先调用 validate 确认 `writes:0`，再调用 import 并断言 PRD readiness 达到 `overall_status:"ready"`。该 example 是导入路径 smoke，真实项目应把 fixture 文件替换为外部 worker/插件/controller 产出的本地文件。
- `cargo run -p pool-core --example closeout_production_evidence_bundle -- target/production-evidence-closeout-smoke` 会把示例 schema 拆成 Provider、软件和桌面视觉三路 bundle，先通过 closeout 预检确认 `writes:0`、`ready_for_import:true`、`coverage.complete:true` 和 `artifact_files.complete:true`，再显式 `import:true` 进入导入路径并断言 PRD readiness 达到 `overall_status:"ready"`；Runtime closeout 响应还会返回顶层 completion gate，供 CI/Agent 紧接着写 PRD completion package。
- `GET /api/output-packages`：读取视频、游戏、交互艺术三类输出 catalog，从已入库 assets 和本地 manifest 文件判断 `ready`、`missing` 或 `indexed_missing_file`，并返回预览合同与控制路由。
- `POST /api/output-packages`：生成视频、游戏、交互艺术三类本地 manifest，写入 tasks、assets 和 `workflow_events`。
- `POST /api/output-packages/results`：把 Resolve/Unreal/TouchDesigner/MadMapper 等后段软件的执行结果回填到对应本地 manifest，写入 `execution_result` / `execution_history`，并创建 `output-package-result` task 与事件。
- `GET /api/handoff-packages`：从 SQLite asset ledger 和本地 `output/control/handoff/8-runtime-handoff-package-manifest.json` 恢复已生成 runtime handoff package catalog，返回每个包的 ready/indexed/missing 状态、文件路径、operator checklist、Agent entrypoint 和 MCP resources；只读不写 SQLite。
- `POST /api/handoff-packages`：把 runtime handoff runbook、preflight、runtime graph、`7-integration-readiness.json`、带 `read_order` / `operator_checklist` / `agent_entrypoint` / `mcp_resources` 的 `8-runtime-handoff-package-manifest.json`、`5-worker-self-checks.sh`、`6-worker-self-checks-preflight.json`、团队角色绑定和可选 snapshot 写入 `output/control/handoff/` indexed 文件，写入 task、assets 和 `workflow_events`，并在 `report` 直接返回 `operator_checklist`、`agent_entrypoint` 和 `mcp_resources`，用于 Agent/Hermes/桌面 controller 离线接管；worker self-check 脚本会优先调用 PATH 上的 `pool-cli`，缺失时 fallback 到 `cargo run -q -p pool-cli -- worker-self-checks`。
- `POST /api/agent-sessions`：写入 Hermes 或 Agent CLI 会话，落地 transcript/control JSON，并同步 tasks、`agent_sessions` 和 `workflow_events`。
- `GET /api/agent-sessions/transcript?session_id=<agent-session-id>`：读取该会话对应的本地 transcript，用于 Web/Hermes/Agent CLI 展开会话正文。
- `POST /api/tasks/approve?task_id=<task-id>`：放行 `WaitingApproval` 任务，写入 `workflow_events`，并返回更新后的 task 与 snapshot；如果该 task 有 `provider_requests` 账本，会用同一 task id 恢复原 Provider run 并返回 run report；如果最近一条 `software_actions` 是因 `requires_confirmation` 暂停的动作，会读取 `command_json`、清除确认标记，并用同一 task id 恢复执行该软件动作。
- `POST /api/tasks/cancel?task_id=<task-id>`：将未完成 task 标记为 `Cancelled`，写入取消事件，并返回更新后的 task 与 snapshot。
- `POST /api/tasks/retry?task_id=<task-id>`：将 `Failed`、`Retryable` 或 `Cancelled` task 恢复为 `Ready`，写入重试事件，并返回更新后的 task 与 snapshot；如果该 task 有 Provider 或软件动作账本，会用同一 task id 恢复原 Provider run 或原软件动作。Provider retry 会追加新的 `provider_requests` attempt，保留旧失败响应，并在 `request_json.attempt.retry_of_provider_request_id` 标明父记录；审批恢复仍复用原等待审批记录。
- `POST /api/software-actions`：创建外部软件控制动作。当前 `unreal` 会优先走 `UnrealMcpAdapter` HTTP MCP/gateway，并向 Unreal MCP body 补 `pool_unreal_action` 与 `mcp_payload`；未配置 endpoint 时回退到 `MockUnrealAdapter`。`hermes` 会优先走 `HermesMcpAdapter` HTTP MCP/gateway，并向 Hermes MCP body 补 `pool_hermes_action` 与 `mcp_payload`；未配置 endpoint 时进入 human takeover 队列。非 Unreal/Hermes 软件在 `priority:"ApiMcp"` 且 payload 带 endpoint 时会走通用 API/MCP adapter，调用默认 `/health` 与 `/mcp` 并发送 `pool_software_action` 与 `mcp_payload`；`ExecuteCli` 可走通用软件 CLI adapter；`DesktopRecognition` 可落地带 `pool_desktop_action` / `desktop_payload` 的桌面识别请求 JSON；仍无明确执行器时进入 human takeover 队列。可选 `evidence_json` 会合并到 `payload_json.evidence` 并进入 `software_actions.command`，用于 PRD readiness 区分本地控制 profile 与真实软件侧生产证据。
- `GET /api/desktop-recognition/requests`：列出已落库且仍待外部桌面 controller 消费的 desktop recognition request，返回 `software_action_id`、`task_id`、请求文件路径、`pool_desktop_action`、`desktop_payload`、原始 command 和 verification；支持 `?project=slug` / `?project=*` query override。
- `GET /api/desktop-recognition/contract`：返回桌面识别 controller 的机器可读合同，覆盖请求文件字段、`pool_desktop_action` / `desktop_payload` 输入、队列读取、dry-run/领取、AppleScript 执行模式、结果回填、可接受状态、task 状态映射和证据要求。
- `POST /api/desktop-recognition/run-next`：运行 runtime 内置 dry-run desktop controller，领取前 N 个待处理请求并复用结果回填逻辑更新 `software_actions.verification_json`、关联 task 和事件流；支持 `status`、`message`、`controller_id`、`limit`、`artifacts` 和 `screen_trace_path`。
- `POST /api/desktop-recognition/results`：由外部桌面 controller 回填执行结果，按 `software_action_id` 更新 `software_actions.verification_json`，同步关联 task 状态，并写入 `workflow_events`。`status` 支持 `running`、`succeeded`、`failed`、`retryable`、`cancelled` 和 `queued_for_desktop_recognition`。

所有响应默认包含 `Access-Control-Allow-Origin: *`，方便本地 Web prototype 直接读取。

## Smoke

一次性验证 handler：

```bash
cargo run -p pool-core --example serve_runtime_http -- target/runtime-http-smoke/pool-runtime.sqlite once
```

常驻运行：

```bash
cargo run -p pool-core --example serve_runtime_http -- target/runtime-http-smoke/pool-runtime.sqlite --bind=127.0.0.1:4788 --registry=target/runtime-http-smoke/runtime-registry.json
```

然后读取：

```bash
curl http://127.0.0.1:4788/api/health
curl http://127.0.0.1:4788/api/runtime-registry
curl http://127.0.0.1:4788/api/discovery
curl 'http://127.0.0.1:4788/api/health?project=demo'
curl 'http://127.0.0.1:4788/api/snapshot?project=*'
curl 'http://127.0.0.1:4788/api/projects'
curl 'http://127.0.0.1:4788/api/events?limit=24'
curl 'http://127.0.0.1:4788/api/events/stream?limit=24'
curl -i 'http://127.0.0.1:4788/api/events/ws?limit=24'
curl 'http://127.0.0.1:4788/api/events?after_id=<event-id>&limit=24'
curl 'http://127.0.0.1:4788/api/runtime-graph'
curl 'http://127.0.0.1:4788/api/runtime-execution-plan'
curl -X POST 'http://127.0.0.1:4788/api/runtime-execution-plan/run-next' \
  -H 'Content-Type: application/json' \
  -d '{"project_slug":"demo"}'
curl 'http://127.0.0.1:4788/api/runtime-handoff'
curl 'http://127.0.0.1:4788/api/prd-readiness'
curl 'http://127.0.0.1:4788/api/prd-completion-gate?require_complete=true'
curl 'http://127.0.0.1:4788/api/prd-completion-packages'
curl 'http://127.0.0.1:4788/api/production-evidence/requirements'
curl 'http://127.0.0.1:4788/api/prompts'
curl 'http://127.0.0.1:4788/api/prompts?name=pool_software_handoff&project_slug=demo&adapter_id=blender&action_kind=ExecuteCli'
curl 'http://127.0.0.1:4788/api/mcp?uri=pool%3A%2F%2Ftasks'
curl 'http://127.0.0.1:4788/api/workflow-context?workflow_id=<workflow-id>'
curl 'http://127.0.0.1:4788/api/mcp?uri=pool%3A%2F%2Fworkflow%2F<workflow-id>'
curl 'http://127.0.0.1:4788/api/mcp?uri=pool%3A%2F%2Fruntime-graph'
curl 'http://127.0.0.1:4788/api/mcp?uri=pool%3A%2F%2Fruntime-execution-plan'
curl 'http://127.0.0.1:4788/api/mcp?uri=pool%3A%2F%2Fprd-readiness'
curl 'http://127.0.0.1:4788/api/mcp?uri=pool%3A%2F%2Fprd-completion-gate'
curl 'http://127.0.0.1:4788/api/mcp?uri=pool%3A%2F%2Fprd-completion-packages'
curl 'http://127.0.0.1:4788/api/mcp?uri=pool%3A%2F%2Fproduction-evidence-requirements'
curl 'http://127.0.0.1:4788/api/mcp?uri=pool%3A%2F%2Fproduction-evidence-item-template'
curl 'http://127.0.0.1:4788/api/mcp?uri=pool%3A%2F%2Fproduction-evidence-item-template%2Fprovider%3Amidjourney%3Aproduction_upstream'
curl 'http://127.0.0.1:4788/api/mcp?uri=pool%3A%2F%2Fadapters'
curl 'http://127.0.0.1:4788/api/integration-readiness'
curl 'http://127.0.0.1:4788/api/mcp?uri=pool%3A%2F%2Fintegration-readiness'
curl 'http://127.0.0.1:4788/api/provider-contracts?provider_id=triposplat'
curl 'http://127.0.0.1:4788/api/mcp?uri=pool%3A%2F%2Fprovider-contracts%2Fmidjourney'
curl -X POST http://127.0.0.1:4788/api/provider-conformance-packages \
  -H 'Content-Type: application/json' \
  -d '{"project_slug":"demo","provider_id":"worldlabs-marble","node_id":"three","output_dir":"worlds/demo/output"}'
curl -X POST http://127.0.0.1:4788/api/integration-conformance-packages \
  -H 'Content-Type: application/json' \
  -d '{"project_slug":"demo","node_id":"agent","output_dir":"worlds/demo/output","providers":["worldlabs-marble"],"software_adapters":["resolve"],"agent_kind":"all"}'
curl 'http://127.0.0.1:4788/api/software-contracts?adapter_id=unreal'
curl -X POST http://127.0.0.1:4788/api/software-conformance-packages \
  -H 'Content-Type: application/json' \
  -d '{"project_slug":"demo","adapter_id":"resolve","node_id":"resolve","output_dir":"worlds/demo/output"}'
curl -X POST http://127.0.0.1:4788/api/agent-conformance-packages \
  -H 'Content-Type: application/json' \
  -d '{"project_slug":"demo","kind":"all","node_id":"agent","output_dir":"worlds/demo/output"}'
curl 'http://127.0.0.1:4788/api/mcp?uri=pool%3A%2F%2Fsoftware-contracts%2Funreal'
curl 'http://127.0.0.1:4788/api/unreal-mcp-bridge'
curl 'http://127.0.0.1:4788/api/mcp?uri=pool%3A%2F%2Funreal-mcp-bridge'
curl 'http://127.0.0.1:4788/api/mcp?uri=pool%3A%2F%2Fdesktop-recognition'
curl http://127.0.0.1:4788/api/api-keys
curl http://127.0.0.1:4788/api/adapters
curl http://127.0.0.1:4788/api/integration-readiness
curl -X POST http://127.0.0.1:4788/api/adapter-health \
  -H 'Content-Type: application/json' \
  -d '{"providers":[{"provider_id":"worldlabs-marble","execution_mode":"mock"}],"software_adapters":[{"adapter_id":"unreal","priority":"ApiMcp"}]}'
curl -X POST http://127.0.0.1:4788/api/provider-health \
  -H 'Content-Type: application/json' \
  -d '{"provider_id":"openai-image-2","api_key":"sk-..."}'
curl -X POST http://127.0.0.1:4788/api/software-health \
  -H 'Content-Type: application/json' \
  -d '{"adapter_id":"blender","priority":"SkillsCli"}'
curl -X POST http://127.0.0.1:4788/api/nodes/run \
  -H 'Content-Type: application/json' \
  -d '{"project_slug":"demo","node_id":"<workflow-node-id>"}'
curl -X POST http://127.0.0.1:4788/api/tasks \
  -H 'Content-Type: application/json' \
  -d '{"title":"World Labs Marble 生成任务","provider_id":"worldlabs-marble","cost_estimate_tokens":9000,"requires_approval":true}'
curl -X POST http://127.0.0.1:4788/api/api-keys \
  -H 'Content-Type: application/json' \
  -d '{"provider_id":"openai-image-2","service_type":"provider","api_key":"sk-...","metadata":{"env":"OPENAI_API_KEY"}}'

# 在启动 Runtime HTTP server 前设置，之后写入的新 key 会加密保存。
export POOL_CREDENTIAL_PASSPHRASE='local passphrase'
curl -X POST http://127.0.0.1:4788/api/api-keys \
  -H 'Content-Type: application/json' \
  -d '{"provider_id":"suno","service_type":"provider","api_key":"suno-...","metadata":{"env":"POOL_SUNO_API_KEY"}}'

# 或使用 macOS Keychain；SQLite 只保存 pool:v1:keychain 引用。
export POOL_CREDENTIAL_STORE=keychain
curl -X POST http://127.0.0.1:4788/api/api-keys \
  -H 'Content-Type: application/json' \
  -d '{"provider_id":"suno","service_type":"provider","api_key":"suno-...","metadata":{"env":"POOL_SUNO_API_KEY"}}'
curl -X POST http://127.0.0.1:4788/api/workflow-runs \
  -H 'Content-Type: application/json' \
  -d '{"project_slug":"demo","title":"Runtime local content burst","prompt":"run creative input to 3DGS to Unreal to outputs","source_inputs":["worlds/demo/source/0-reference.png"],"duration_ms":12000,"agent_mode":"stage","three_dgs_mode":"auto","unreal_mode":"auto"}'
curl -X POST http://127.0.0.1:4788/api/workflow-runs \
  -H 'Content-Type: application/json' \
  -d '{"project_slug":"demo","title":"Gateway + Unreal MCP content burst","prompt":"force real adapter path for verification","agent_mode":"hermes_http","hermes_endpoint":"http://127.0.0.1:3900/hermes","three_dgs_mode":"gateway","three_dgs_endpoint":"http://127.0.0.1:8787","unreal_mode":"unreal_mcp","unreal_endpoint":"http://127.0.0.1:8788"}'

# 下面的 gateway provider-run 示例可先启动本地契约服务器：
# cargo run -p pool-core --example provider_gateway_mock_server -- --bind=127.0.0.1:8787
curl -X POST http://127.0.0.1:4788/api/provider-runs \
  -H 'Content-Type: application/json' \
  -d '{"provider_id":"world-labs-marble","execution_mode":"auto","task_title":"World Labs Marble local run","prompt":"generate neon bazaar world","output_dir":"worlds/demo/output","requires_approval":false}'
curl -X POST http://127.0.0.1:4788/api/provider-runs \
  -H 'Content-Type: application/json' \
  -d '{"provider_id":"world-labs-marble","execution_mode":"gateway","endpoint":"http://127.0.0.1:8787","task_title":"World Labs Marble gateway run","prompt":"convert concept plate into 3DGS world","output_dir":"worlds/demo/output","requires_approval":false}'
curl -X POST http://127.0.0.1:4788/api/provider-runs \
  -H 'Content-Type: application/json' \
  -d '{"provider_id":"nano-banana-pro","endpoint":"http://127.0.0.1:8787","task_title":"Nano Banana Pro gateway run","prompt":"{\"prompt\":\"generate hero plate\",\"output_slug\":\"nano\",\"output_extension\":\"png\"}","output_dir":"worlds/demo/output","requires_approval":false}'
curl -X POST http://127.0.0.1:4788/api/provider-runs \
  -H 'Content-Type: application/json' \
  -d '{"provider_id":"suno","endpoint":"http://127.0.0.1:8787","task_title":"Suno cue run","prompt":"{\"prompt\":\"generate a short electronic cue\",\"output_slug\":\"suno-cue\",\"output_extension\":\"mp3\"}","output_dir":"worlds/demo/output","requires_approval":false}'
curl 'http://127.0.0.1:4788/api/production-evidence/tasks?project=demo'
curl -X POST http://127.0.0.1:4788/api/production-evidence/tasks/claim \
  -H 'Content-Type: application/json' \
  -d '{"project_slug":"demo","task_id":"provider:midjourney:production_upstream","assignee":"worker-1","role":"provider_worker","output_root":"target/production-evidence"}'
curl 'http://127.0.0.1:4788/api/production-evidence/item-template?project=demo&task_id=provider:midjourney:production_upstream&output_root=target/production-evidence'
curl -X POST http://127.0.0.1:4788/api/production-evidence/validate \
  -H 'Content-Type: application/json' \
  --data-binary @docs/examples/production-evidence-bundle.example.json
curl -X POST http://127.0.0.1:4788/api/production-evidence \
  -H 'Content-Type: application/json' \
  --data-binary @docs/examples/production-evidence-bundle.example.json
curl -X POST http://127.0.0.1:4788/api/production-evidence/items/validate \
  -H 'Content-Type: application/json' \
  -d '{"project_slug":"demo","source":"provider-worker","kind":"provider","provider":{"provider_id":"midjourney","external_job_id":"mj-real-job-1","metadata_path":"worlds/demo/output/production/midjourney-request.json","artifacts":["worlds/demo/output/production/midjourney.png"]}}'
curl -X POST http://127.0.0.1:4788/api/production-evidence/items \
  -H 'Content-Type: application/json' \
  -d '{"project_slug":"demo","source":"provider-worker","kind":"provider","provider":{"provider_id":"midjourney","external_job_id":"mj-real-job-1","metadata_path":"worlds/demo/output/production/midjourney-request.json","artifacts":["worlds/demo/output/production/midjourney.png"]}}'
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
curl -X POST http://127.0.0.1:4788/api/agent-sessions \
  -H 'Content-Type: application/json' \
  -d '{"kind":"hermes","project_slug":"demo","endpoint":"http://127.0.0.1:8787/hermes","instruction":"inspect Unreal import queue","allowed_tools":["api","mcp","unreal"],"requires_confirmation":false}'
curl -X POST http://127.0.0.1:4788/api/agent-sessions \
  -H 'Content-Type: application/json' \
  -d '{"kind":"hermes","project_slug":"demo","endpoint":"http://127.0.0.1:8787/hermes","instruction":"inspect Unreal import queue","allowed_tools":["api","mcp","unreal"],"requires_confirmation":false,"execute":true,"timeout_ms":2000}'
curl -X POST http://127.0.0.1:4788/api/agent-sessions \
  -H 'Content-Type: application/json' \
  -d '{"kind":"agent_cli","project_slug":"demo","command_id":"node-context","title":"Inspect runtime nodes","command":"pool-cli --project demo node-context","tools":["sqlite","filesystem"],"token_budget":4000}'
curl -X POST http://127.0.0.1:4788/api/agent-sessions \
  -H 'Content-Type: application/json' \
  -d '{"kind":"agent_cli","project_slug":"demo","command_id":"echo","title":"Execute allowed command","command":"/bin/echo runtime-agent-ok","tools":["cli"],"execute":true,"allowed_commands":["/bin/echo","echo"],"timeout_ms":2000}'
curl 'http://127.0.0.1:4788/api/agent-sessions/transcript?session_id=<agent-session-id>'
curl 'http://127.0.0.1:4788/api/agent-sessions/stream?session_id=<agent-session-id>'
curl -i 'http://127.0.0.1:4788/api/agent-sessions/ws?session_id=<agent-session-id>'
curl -X POST 'http://127.0.0.1:4788/api/tasks/approve?task_id=<task-id>'
curl -X POST 'http://127.0.0.1:4788/api/tasks/cancel?task_id=<task-id>'
curl -X POST 'http://127.0.0.1:4788/api/tasks/retry?task_id=<task-id>'
curl -X POST http://127.0.0.1:4788/api/software-actions \
  -H 'Content-Type: application/json' \
  -d '{"adapter_id":"unreal","action_kind":"CreateScene","priority":"ApiMcp","task_title":"Unreal scene assembly","payload_json":{"level":"demo"}}'
curl -X POST http://127.0.0.1:4788/api/software-actions \
  -H 'Content-Type: application/json' \
  -d '{"adapter_id":"unreal","action_kind":"CreateScene","priority":"ApiMcp","task_title":"Unreal MCP scene assembly","payload_json":{"endpoint":"http://127.0.0.1:8787","level":"demo","assets":["worlds/demo/output/1-world.glb"]}}'
curl -X POST http://127.0.0.1:4788/api/software-actions \
  -H 'Content-Type: application/json' \
  -d '{"adapter_id":"hermes","action_kind":"CreateScene","priority":"ApiMcp","task_title":"Hermes MCP orchestration","payload_json":{"endpoint":"http://127.0.0.1:8787","project_slug":"demo","instruction":"coordinate Unreal scene assembly","allowed_tools":["unreal","filesystem"],"target_adapter":"unreal"}}'
curl -X POST http://127.0.0.1:4788/api/software-actions \
  -H 'Content-Type: application/json' \
  -d '{"adapter_id":"blender","action_kind":"ExecuteCli","priority":"SkillsCli","task_title":"Blender CLI smoke","payload_json":{"command":"/bin/echo blender-runtime-ok","allowed_commands":["/bin/echo","echo"],"timeout_ms":2000,"max_output_bytes":1024}}'
curl -X POST http://127.0.0.1:4788/api/software-actions \
  -H 'Content-Type: application/json' \
  -d '{"adapter_id":"touchdesigner","action_kind":"RunViewport","priority":"DesktopRecognition","task_title":"TouchDesigner desktop cue","payload_json":{"instruction":"find TouchDesigner perform mode and trigger cue 1","target_window":"TouchDesigner","visual_targets":["Perform","Cue 1","Output"]}}'

curl http://127.0.0.1:4788/api/desktop-recognition/requests
curl -X POST http://127.0.0.1:4788/api/desktop-recognition/run-next -d '{"controller_id":"local-vision-dry-run","status":"succeeded"}'

curl -X POST http://127.0.0.1:4788/api/desktop-recognition/results \
  -H 'Content-Type: application/json' \
  -d '{"software_action_id":"<action-id>","status":"succeeded","message":"desktop controller finished","artifacts":["worlds/demo/output/control/desktop-recognition/trace.json"],"result":{"controller":"desktop-vision"}}'

cargo run -p pool-core --example run_desktop_vision_trace_smoke -- target/desktop-vision-trace-smoke
cargo run -p pool-core --example run_desktop_recognition_controller -- http://127.0.0.1:4788 --project=demo --status=succeeded
```

CLI 读取同一份 runtime：

```bash
cargo run -p pool-cli -- --db target/runtime-http-smoke/pool-runtime.sqlite --project demo status
cargo run -p pool-cli -- --db target/runtime-http-smoke/pool-runtime.sqlite --project demo runtime-graph
cargo run -p pool-cli -- --db target/runtime-http-smoke/pool-runtime.sqlite --project demo runtime-execution-plan
cargo run -p pool-cli -- --db target/runtime-http-smoke/pool-runtime.sqlite --project demo runtime-run-next
cargo run -p pool-cli -- --db target/runtime-http-smoke/pool-runtime.sqlite --project demo prd-readiness
cargo run -p pool-cli -- --db target/runtime-http-smoke/pool-runtime.sqlite --project demo node-context
cargo run -p pool-cli -- --db target/runtime-http-smoke/pool-runtime.sqlite --project demo mcp pool://tasks
```

AppleScript 桌面 controller 读取同一个 HTTP runtime 队列并回填结果：

```bash
cargo run -p pool-core --example run_desktop_recognition_controller -- \
  http://127.0.0.1:4788 \
  --project=demo \
  --mode=applescript \
  --osascript=/usr/bin/osascript \
  --vision-trace=worlds/demo/output/control/desktop-recognition/trace.json
```

外部视觉/OCR controller 可用 `vision-http` 模式读取同一队列，调用外部视觉 endpoint，写入本地 trace 并通过 `/api/desktop-recognition/results` 回填 `external_visual_model:true`：

```bash
cargo run -p pool-core --example run_desktop_recognition_controller -- \
  http://127.0.0.1:4788 \
  --project=demo \
  --mode=vision-http \
  --vision-endpoint=http://127.0.0.1:8795/vision \
  --vision-api-key-env=POOL_DESKTOP_VISION_API_KEY \
  --vision-trace-output=worlds/demo/output/control/desktop-recognition/external-vision-trace.json

POOL_DESKTOP_VISION_PRODUCTION_ATTESTATION=real-vision-controller-run-001 pool-cli --project demo production-evidence-desktop-vision \
  target/desktop-vision-evidence \
  --production-vision \
  --trace worlds/demo/output/control/desktop-recognition/external-vision-trace.json \
  --external-action-id=real-vision-action-001 \
  --evidence-bundle=target/desktop-vision-evidence/desktop-vision-production-evidence-bundle.json
```

`production-evidence-desktop-vision` 会把成功的外部视觉/OCR controller 结果整理成 `desktop_vision[]` production evidence bundle；该 bundle 可先用 `pool-cli validate-production-evidence` dry-run，再用 `import-production-evidence` 写入真实生产证据账本。

Web prototype 读取：

```text
http://localhost:4173/apps/web-prototype/?runtime=local
http://localhost:4173/apps/web-prototype/?runtime=http://127.0.0.1:4788
http://localhost:4173/apps/web-prototype/?runtime_registry=/target/runtime-http-smoke/runtime-registry.json
http://localhost:4173/apps/web-prototype/?runtime=local&runtime_ports=4788,4789
```

## 当前边界

- 当前 HTTP API 已支持 runtime registry endpoint/file、discovery service descriptor、snapshot/MCP 读取、MCP resource manifest、MCP prompt manifest、HTTP prompt 生成、Provider gateway/native contracts、Provider gateway worker contract、软件控制 contracts、Unreal MCP bridge contract、runtime graph 派生视图、runtime execution plan 可执行步骤、runtime budget/credential readiness 摘要、runtime preflight 阻塞/警告/next actions 摘要、单 workflow context endpoint、项目列表、增量事件轮询、常驻 SSE 事件流、常驻 runtime WebSocket 事件流、Agent session SSE/WebSocket 会话流、API Key 脱敏管理、本地加密封装、macOS Keychain 引用模式、API Key rotation audit、adapter capability registry、批量 Adapter health fan-out、单 Provider health 检查、Provider request metadata/handoff 安全读取、单软件 adapter health 检查、节点级执行入口、runtime task 创建、本地内容爆发闭环、Provider adapter run、生产证据 bundle validate-only 校验和导入、三类输出包生成、三类输出后段结果回填、Hermes HTTP 执行、Agent CLI 受控执行、Unreal MCP/mock 动作、Hermes MCP software action、通用软件 CLI 动作、desktop recognition schema/request staging、controller 请求领取、runtime dry-run 推进、AppleScript 确定性桌面执行、外部视觉 trace 到点击坐标桥接、`vision-http` 外部视觉/OCR endpoint 调用、本地 trace 写入与结果回填、desktop vision trace/callback evidence smoke、审批放行、任务取消/重试和软件动作写入；Provider run 会把 adapter 成本估算同步到 task/snapshot，并把请求/响应写入 `provider_requests`，可选 `evidence_json` 会落到 `provider_requests.request_json.evidence` 供 `pool://prd-readiness` 区分本地 mock gateway 与真实上游 gateway 证据，省略 `requires_approval` 时高成本 Provider 自动进入审批队列，审批后 `/api/tasks/approve` 会恢复同一 Provider run，`/api/tasks/retry` 也会按账本重跑 Provider run 并追加新的 `provider_requests` attempt；每条 request JSON 都包含 `attempt.kind`，retry attempt 会带 `retry_of_provider_request_id`；软件动作带 `requires_confirmation` 时会先写入等待审计，审批后用同一 task 恢复执行原动作，失败/取消后重试也会按 `software_actions.command_json` 重跑原动作；Agent session 带 `execute:true` 但被审批阻断时会在 transcript 记录 `execution_request`，审批/重试会用同一 task 恢复 Hermes HTTP 或 Agent CLI 执行，纯 staging session 只释放到 `ready`；显式传 `false` 可用于受控 smoke；`/api/workflow-runs` 默认会 staging Hermes 决策会话，`auto` 模式会优先使用已配置 3DGS gateway / Unreal MCP，缺省或失败时回落本地 mock。真实外部 Provider 需要本地环境变量、请求 endpoint/api_key 覆盖或 `/api/api-keys` 提供凭证。Unreal bridge contract 已定义插件/gateway 侧工具协议，但真实 Blueprint/Sequencer/Level import 仍需要外部 Unreal 插件执行。Midjourney/Nano Banana Pro/Suno 目前通过通用 HTTP media gateway 加 Pool media profile mapping 执行，尚未包含厂商官方 SDK；屏幕采集与视觉模型仍由外部 controller 提供，Pool 消费其 trace JSON，并要求真实 controller 显式写入 `external_visual_model:true` 后才把生产视觉模型证据视为 ready。
- Web prototype 已支持 `?runtime=...` 读取本地 runtime，并会自动探测默认本地端口组；`?runtime_ports=` 与 `?runtime_endpoints=` 可覆盖候选地址。
- MCP resource 仍是进程内实现，后续可继续扩展为标准 MCP server transport；`pool://desktop-recognition` 已可作为外部桌面 controller/Agent 的只读队列入口。
