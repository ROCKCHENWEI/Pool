# Pool 运行框架整合说明

## 主基线

Pool 的运行框架以 `ROCKCHENWEI/Pool` 为主基线：Rust shared-core 承担长期运行状态、工作流模型、任务队列、Provider 统一接口、SQLite schema 和 MCP resource。当前本地中文 Web 原型迁移到 `apps/web-prototype/`，定位为控制台设计资源和未来 Web/SwiftUI 运行面板。

## image-blaster 吸收点

`image-blaster` 的价值集中在 2D 到 3D/3DGS 的生成流程和本地资产契约：

- 项目 envelope 使用 `worlds/<slug>/project.json`、`workflow.json`、`scene.json`、`source/`、`output/`。
- 生成结果使用 indexed files，例如 `1-world.glb`、`1-world-full_res.spz`。
- Provider request metadata 使用隐藏文件，例如 `.1-world-request.json`。
- Provider URL 只作为 provenance；前端和引擎加载必须使用本地文件。
- 高成本 3DGS/视频生成必须先进入 `waiting_approval`。

这些约束已落到 `ProjectEnvelope`、`parse_indexed_name`、`Mock3dgsProvider`、`TaskQueue`、`RuntimeRepository`、`materialize_project_envelope` 和 `PoolRuntimePlan`。

## P0 / P1 / P2 映射

- P0 Timeline：`Project/Shot/Segment` 统一抽象视频镜头、游戏关卡片段和交互艺术 cue/scene。
- P1 Pool_node：`Workflow/WorkflowNode/WorkflowConnection`、`NodeEngine` 与 `PoolRuntimePlan` 是执行计划核心。
- P2 V.I.S.C：`ProviderAdapter`、`ProviderRegistry`、`SoftwareAdapterRegistry`、`SoftwareAdapterConfig` 和 MCP resources 是 Provider、软件控制、Agent 控制与素材记忆层的入口。

## 默认运行蓝图

`build_default_content_burst_plan(slug, title)` 生成首版内容爆发闭环：

1. 起始输入接收图片、视频、文字、prompt 与参考素材。
2. Agent 节点通过 Hermes/Agent 控制层生成创意分析与执行计划。
3. ComfyUI 节点生成图片或视频 plate。
4. 3DGS 节点使用 World Labs Marble 作为默认高成本 provider，进入人工审批。
5. 本地资产包节点承接 image-blaster indexed files。
6. Unreal 节点负责组装场景、资产、灯光、相机和运行视口。
7. Resolve、Unreal、TouchDesigner 分别承接视频、游戏、交互艺术输出。

## 当前运行入口

`RuntimeRepository` 负责 SQLite migration 与运行蓝图落库：

- project 写入 `projects`
- shot 写入 `shots`
- workflow 写入 `workflows`
- 每个 workflow node 展开成一个 `tasks` 记录
- 运行事件写入 `workflow_events`

`RuntimeSnapshot` 负责把 SQLite runtime 状态导出成 Web/SwiftUI 可读 JSON：

- projects
- workflows
- node_states
- tasks
- assets
- workflow_events
- provider_requests
- software_actions
- agent_sessions
- approval/running/failed 统计
- task estimated tokens、Agent token used/budget、token_total 等消耗统计

可用命令：

```bash
cargo run -p pool-core --example export_runtime_snapshot
```

默认输出：

```text
target/runtime-snapshot-smoke/runtime-snapshot.json
```

`apps/web-prototype` 已支持可选 snapshot 文件加载：

```text
http://localhost:4173/apps/web-prototype/?snapshot=/target/runtime-snapshot-smoke/runtime-snapshot.json
```

加载成功后，页面会用 snapshot 中的 `workflows`、`node_states`、`tasks`、`assets`、`events` 和 `stats.token_total` 替换静态演示状态；加载失败时保留 `localStorage` fallback。

`McpServer::from_snapshot` 已支持用同一份 `RuntimeSnapshot` 驱动 OpenClaw/MCP resources：

- `pool://status`
- `pool://projects`
- `pool://tasks`
- `pool://assets`
- `pool://adapters`
- `pool://integration-readiness`
- `pool://provider-contracts`
- `pool://provider-contracts/<provider-id>`
- `pool://provider-gateway-worker`
- `pool://software-contracts`
- `pool://software-contracts/<adapter-id>`
- `pool://unreal-mcp-bridge`
- `pool://workflow`
- `pool://workflow/<workflow-id>`
- `pool://runtime-graph`
- `pool://runtime-budget`
- `pool://runtime-preflight`
- `pool://runtime-execution-plan`
- `pool://runtime-handoff`
- `pool://prd-readiness`
- `pool://prd-completion-gate`
- `pool://production-evidence-requirements`
- `pool://production-evidence-tasks`
- `pool://production-evidence-run-plan`
- `pool://production-evidence-handoff`
- `pool://node-context/<node-id>`
- `pool://events`
- `pool://provider-requests`
- `pool://software-actions`
- `pool://desktop-recognition`
- `pool://agent-sessions`
- `pool://api-keys`
- `pool://snapshot`

可用命令：

```bash
cargo run -p pool-core --example read_mcp_resources
```

`RuntimeHttpServer` 已把同一份 SQLite snapshot 和 MCP resource 暴露成本地 HTTP API：

- `GET /api/health`
- `GET /api/discovery`
- `GET /.well-known/pool-runtime.json`
- `GET /api/snapshot`
- `GET /api/projects`
- `GET /api/events`
- `GET /api/events/stream`
- `GET /api/events/ws`
- `GET /api/resources`
- `GET /api/prompts`
- `GET /api/mcp?uri=pool://tasks`
- `GET /api/runtime-graph`
- `GET /api/runtime-budget`
- `GET /api/runtime-preflight`
- `GET /api/runtime-execution-plan`
- `POST /api/runtime-execution-plan/run-next`
- `GET /api/runtime-handoff`
- `GET /api/prd-readiness`
- `GET /api/prd-completion-gate`
- `POST /api/prd-completion-package`
- `GET /api/production-evidence/requirements`
- `GET /api/workflow-context?workflow_id=<workflow-id>`
- `GET /api/node-context?node_id=<node-id>`
- `GET /api/mcp?uri=pool://workflow/<workflow-id>`
- `GET /api/mcp?uri=pool://runtime-graph`
- `GET /api/mcp?uri=pool://runtime-budget`
- `GET /api/mcp?uri=pool://runtime-preflight`
- `GET /api/mcp?uri=pool://runtime-execution-plan`
- `GET /api/mcp?uri=pool://runtime-handoff`
- `GET /api/mcp?uri=pool://prd-readiness`
- `GET /api/mcp?uri=pool://prd-completion-gate`
- `GET /api/mcp?uri=pool://production-evidence-requirements`
- `GET /api/mcp?uri=pool://adapters`
- `GET /api/mcp?uri=pool://integration-readiness`
- `GET /api/mcp?uri=pool://node-context/<node-id>`
- `GET /api/mcp?uri=pool://desktop-recognition`
- `GET /api/api-keys`
- `GET /api/adapters`
- `GET /api/integration-readiness`
- `GET /api/provider-contracts?provider_id=<provider-id>`
- `GET /api/software-contracts?adapter_id=<adapter-id>`
- `POST /api/adapter-health`
- `POST /api/provider-health`
- `POST /api/software-health`
- `POST /api/nodes/run`
- `POST /api/tasks`
- `POST /api/api-keys`
- `POST /api/workflow-runs`
- `POST /api/provider-runs`
- `POST /api/output-packages`
- `POST /api/handoff-packages`
- `POST /api/agent-sessions`
- `GET /api/agent-sessions/ws?session_id=<agent-session-id>`
- `POST /api/tasks/approve?task_id=<task-id>`
- `POST /api/tasks/cancel?task_id=<task-id>`
- `POST /api/tasks/retry?task_id=<task-id>`
- `POST /api/software-actions`
- `GET /api/desktop-recognition/requests`
- `POST /api/desktop-recognition/run-next`
- `POST /api/desktop-recognition/results`

`/api/events/ws` 会通过 WebSocket upgrade 持续推送 `pool-runtime-events` 和 `runtime-event` JSON 文本帧；未 upgrade 时返回 426，并可回退 `/api/events/stream` SSE 或 `/api/events` 轮询。`/api/runtime-graph` 和 `pool://runtime-graph` 会把 workflow nodes、task 状态、连接标签、控制通道和 task 类型合并成一个可执行运行图；`/api/runtime-budget` 和 `pool://runtime-budget` 会把 task 估算 token、待审批 token、Agent 预算、Provider Key 就绪状态、Provider 请求和审批门聚合成运行前预算摘要；`/api/runtime-preflight` 和 `pool://runtime-preflight` 会进一步输出阻塞项、警告和建议 CLI next actions，供 Agent/Hermes 决定先执行非阻塞 `pool_worker_self_checks` / `worker-self-checks` 本地桥接 smoke，再审批、补 Key、重试任务或接管桌面识别，桌面接管 action 会给出 `desktop-run-next` 推进命令和 `desktop-requests` 检查命令；`/api/runtime-execution-plan` 和 `pool://runtime-execution-plan` 会把 workflow 拓扑整理成可执行步骤，并在每步带上状态、审批门、Provider/软件合同、node context URI 和推荐 CLI/MCP 动作；`/api/provider-contracts` 与 `pool://provider-contracts/<provider-id>` 会输出 AI/3DGS Provider 的机器可读接入合同；`/api/provider-gateway-worker` 与 `pool://provider-gateway-worker` 会输出本地 HTTP forwarder 的 CLI、route、上游 worker 和 endpoint env 合同；`/api/software-contracts` 与 `pool://software-contracts/<adapter-id>` 会输出外部软件控制的 health/action body、优先级链、API/MCP、Skills/CLI、桌面识别和人工接管路径；`/api/runtime-handoff` 和 `pool://runtime-handoff` 会把这些检查结果、本地 worker smoke、离线接管包、可运行节点和桌面接管请求整理成机器可读 handoff lanes、命令列表和 5 人团队角色绑定；`/api/prd-readiness` 和 `pool://prd-readiness` 会从同一份 snapshot 生成 PRD 要求级 ready/partial/blocked 审计，列出证据、缺口和 next actions；`/api/prd-completion-gate` 和 `pool://prd-completion-gate` 会从同源审计中抽出完成门槛，并在 `require_complete=true` 时对未完成快照返回 428；`POST /api/prd-completion-package` 和 MCP tool `pool_prd_completion_package` 会把 readiness、completion gate、production evidence requirements、manifest 和可选 snapshot 写入 `control/prd-completion/` 并入库为 task/assets/events；`/api/production-evidence/requirements` 和 `pool://production-evidence-requirements` 会输出真实生产证据缺口清单，`/api/production-evidence/tasks`、`pool://production-evidence-tasks` 和 MCP tool `pool_production_evidence_tasks` 会输出同源缺口任务队列，`/api/production-evidence/run-plan`、`pool://production-evidence-run-plan` 和 MCP tool `pool_production_evidence_run_plan` 会输出同源七段真实生产证据执行计划，`/api/production-evidence/handoff`、`pool://production-evidence-handoff` 和 MCP tool `pool_production_evidence_handoff` 会输出同源分派上下文，`pool://production-evidence-item-template` 与 `pool://production-evidence-item-template/<task-id>` 会输出只读单项证据模板索引和具体 task item wrapper；`/api/workflow-context?workflow_id=...` 与 `pool://workflow/<workflow-id>` 会返回单个 workflow 的 graph、node_states、tasks、assets、provider_requests、software_actions、agent_sessions 和审批摘要，供 Hermes/Agent 在执行前锁定上下文；`/api/node-context?node_id=...` 与 `pool://node-context/<node-id>` 用同一份 snapshot 下钻单个节点，返回它的 incoming/outgoing edges、tasks、assets、provider_requests、software_actions、agent_sessions、审批摘要和 `control_context` 建议 CLI/MCP 控制入口；`/api/prompts` 会暴露内容爆发、3DGS、软件接管和桌面识别四类 Agent runbook；`/api/discovery` 同时返回 endpoint manifest、MCP resource manifest、MCP tool manifest 与 MCP prompt manifest，外部 Agent、Hermes 或桌面 controller 可以先读取 discovery，再按 `mcp_tools[]` 选择 `pool_worker_self_checks`、`pool_handoff_package`、`pool_run_provider`、`pool_run_software` 等 stdio tool，按 `mcp_prompts[].http_path` 选择标准 runbook，并按 `mcp_resources[].http_path` 获取 `pool://tasks`、`pool://workflow/<workflow-id>`、`pool://runtime-graph`、`pool://runtime-budget`、`pool://runtime-preflight`、`pool://runtime-execution-plan`、`pool://runtime-handoff`、`pool://prd-readiness`、`pool://prd-completion-gate`、`pool://production-evidence-requirements`、`pool://production-evidence-tasks`、`pool://production-evidence-run-plan`、`pool://production-evidence-handoff`、`pool://production-evidence-item-template`、`pool://node-context`、`pool://provider-gateway-worker`、`pool://software-contracts`、`pool://software-actions`、`pool://desktop-recognition` 等运行状态。

`POST /api/runtime-execution-plan/run-next` 会把只读 plan 变成受控调度入口：默认 preview，不改变 SQLite；显式 `execute:true` 才分派到节点运行、任务审批或任务重试；审批动作还必须传 `allow_approval:true`，防止 Agent 自动放行高成本 3DGS/视频任务。

一次性 smoke：

```bash
cargo run -p pool-core --example serve_runtime_http -- target/runtime-http-smoke/pool-runtime.sqlite once
```

常驻本地服务：

```bash
cargo run -p pool-core --example serve_runtime_http -- target/runtime-http-smoke/pool-runtime.sqlite --bind=127.0.0.1:4788
```

Web prototype 可直接读取本地 Runtime HTTP：

```text
http://localhost:4173/apps/web-prototype/?runtime=local
http://localhost:4173/apps/web-prototype/?runtime_registry=runtime-registry.json
http://localhost:4173/apps/web-prototype/?runtime=http://127.0.0.1:4788
http://localhost:4173/apps/web-prototype/?runtime=local&runtime_ports=4788,4789
http://localhost:4173/apps/web-prototype/
http://localhost:4173/apps/web-prototype/?runtime=local&project=demo
http://localhost:4173/apps/web-prototype/?runtime=local&project=*
```

无 query 参数或 `?runtime=local/auto` 时，Web prototype 会自动探测默认本地端口组，并可用 `?runtime_registry=runtime-registry.json`、`?runtime_ports=4788,4789` 或 `?runtime_endpoints=http://127.0.0.1:4788,http://127.0.0.1:4878` 覆盖候选地址；显式 `?runtime=http://...` 和 `?snapshot=` 仍保持优先级。`?project=slug` / `?project_slug=slug` 会传给 `/api/health`、`/api/snapshot`、`/api/runtime-graph`、`/api/runtime-budget`、`/api/runtime-preflight`、`/api/prd-readiness`、`/api/prd-completion-gate`、`/api/production-evidence/requirements` 与 `/api/workflow-context` 并持久化，`?project=*` 表示全部项目。连接 runtime 后，Web prototype 会读取 `GET /api/projects` 填充顶部项目选择器，读取 `GET /api/discovery` 在 Agent 页展示 endpoint manifest、MCP resource/prompt 计数和 `serve-mcp` 启动命令，切换项目时会更新 `?project=` 并重新读取 snapshot、runtime graph、runtime budget、runtime preflight、PRD readiness、PRD completion gate、production evidence requirements 与 workflow context；节点图优先使用 `GET /api/runtime-graph` 的任务类型、连接通道和 labels，再叠加最新 snapshot task 状态；接入页会用 `GET /api/runtime-budget` 显示预算、待审批 token、Provider Key 就绪和 Provider 请求摘要，读取 `GET /api/provider-contracts` 在 Provider 卡片展示 native/gateway contract 摘要，读取 `GET /api/provider-gateway-worker` 展示 AI media/3DGS worker 启动命令、endpoint env 和 MCP tool，读取 `GET /api/software-contracts` 在软件矩阵展示控制合同摘要，用 `GET /api/runtime-preflight` 显示阻塞项、警告和建议 CLI next actions，并用 `GET /api/prd-readiness` 与 `GET /api/prd-completion-gate` 显示 PRD 要求级 ready/partial/blocked 审计和独立完成门槛，用 `GET /api/production-evidence/requirements` 显示真实 Provider/软件/桌面视觉生产证据缺口清单；节点详情侧栏会同时展示 `GET /api/workflow-context?workflow_id=...` 的 workflow 账本摘要和 `GET /api/node-context?node_id=...` 的单节点上下文；Hermes 面板会读取 `GET /api/prompts` 填充 Agent runbook 选择器，并可通过 `GET /api/prompts?name=...` 把标准 runbook 写入控制指令框。所有返回 snapshot 的 runtime 写操作会通过同一个合并 helper 重新读取 `/api/discovery`、`/api/runtime-graph`、`/api/runtime-budget`、`/api/runtime-preflight`、`/api/prd-readiness`、`/api/prd-completion-gate`、`/api/production-evidence/requirements`、`/api/provider-contracts`、`/api/provider-gateway-worker`、`/api/software-contracts` 和 `/api/workflow-context`，再刷新 discovery、节点图、预算凭证、运行前检查、PRD readiness、PRD completion gate、production evidence requirements、Provider/software contract、Provider gateway worker contract 与 workflow 摘要，避免运行/审批/重试/软件动作之后仍显示旧的运行图语义或账本计数。顶部“运行一次”会调用 `POST /api/workflow-runs` 执行本地内容爆发闭环，并在 Hermes 面板和节点详情侧栏显示 Agent 模式、3DGS/Unreal adapter 模式、transcript 路径和最新 Agent session；运行中心事件流会优先通过 `GET /api/events/ws?last_event_id=...` 接收 WebSocket JSON 文本帧，失败时回退 `GET /api/events/stream?last_event_id=...` EventSource/SSE，再回退 `GET /api/events?after_id=...` 轮询，不重载节点图；节点详情“运行节点”会调用 `POST /api/nodes/run`，按节点类型分派到 Provider、Agent、软件动作、输出包或普通 task，并把 node `control_context` 写入 Provider request ledger 或软件 action payload；Provider Key 保存按钮会调用 `POST /api/api-keys` 写入脱敏 credential 状态；接入页“批量巡检 Adapter”会调用 `POST /api/adapter-health`，并把 Provider 与软件 adapter health 写回卡片；Provider“测试连接”会调用 `POST /api/provider-health`，Provider“创建任务”按钮会优先调用 `POST /api/tasks` 写入 SQLite；Provider“运行”按钮会调用 `POST /api/provider-runs` 调度真实 Provider adapter 或本地 mock 3DGS，并写入 tasks、assets 与事件流；三类输出面板“生成输出包”按钮会调用 `POST /api/output-packages` 生成视频、游戏、交互艺术三类本地 manifest；Hermes“发送指令”会用 `execute:true` 调用 `POST /api/agent-sessions` 执行 HTTP endpoint，Hermes“写入任务队列”只 staging 会话、任务和 transcript；Agent CLI 按钮会调用 `POST /api/agent-sessions` 写入会话、任务和 transcript；软件矩阵“检查”会调用 `POST /api/software-health`，软件矩阵“写入动作”按钮会调用 `POST /api/software-actions` 写入软件控制审计，显式 `ExecuteCli` payload 可通过 `CommandSoftwareAdapter` 受控执行；桌面识别接管队列会调用 `GET /api/desktop-recognition/requests`，可通过 `POST /api/desktop-recognition/run-next` dry-run 推进，并可通过 `POST /api/desktop-recognition/results` 回填成功/失败；“人工确认”按钮会优先调用 `POST /api/tasks/approve` 放行高成本任务，任务队列“取消/重试”按钮会调用 `POST /api/tasks/cancel` 与 `POST /api/tasks/retry` 并刷新 snapshot。未连接 runtime 时保留本地模拟路径。

生产证据接入页新增 Runtime 需求清单、任务队列、任务领取、文件交付包、运行计划、脚手架和合并入口：连接 Runtime HTTP 后，会先读取 `GET /api/production-evidence/requirements` 展示 required Provider、required software adapter、desktop vision controller 的缺口，并可读取 `GET /api/production-evidence/tasks` 让外部 worker/operator/controller 查看单个 evidence task；`POST /api/production-evidence/tasks/claim` 会把其中一个缺口 task 转成可追踪 runtime task，并写出本地 claim JSON。通过“生成计划”调用 `GET /api/production-evidence/run-plan`，会把 requirements、evidence tasks 和 completion gate 串成 Provider matrix、software matrix、desktop vision controller、merge、closeout preflight、closeout import、completion proof 七段真实执行计划，返回 bundle 路径、命令、ready 条件、HTTP/MCP 入口和 operator checklist；Web 只在结果面板展示计划摘要，不覆盖 bundle textarea。通过“写交付包”调用 `POST /api/production-evidence/handoff-packages`，会把 requirements、task queue、missing-only handoff、`4-production-evidence-run-plan.json`、`5-production-evidence-bundle.json`、per-task item JSON、`6-production-evidence-package-manifest.json`、`7-production-evidence-runner.sh`、`8-production-evidence-runner-preflight.json` 和可选 `9-runtime-snapshot.json` 落到本地 `control/production-evidence/`；runner script 的 Provider 阶段可用 `POOL_MEDIA_GATEWAY_ENDPOINT` / `POOL_3DGS_GATEWAY_ENDPOINT` 覆盖 AI media/3DGS 家族，也可为每个 required Provider 提供 per-provider endpoint，OpenAI image-2 native adapter 还需要 `OPENAI_API_KEY` 或 provider-specific key env；生产证明可用全局 `POOL_PROVIDER_PRODUCTION_ATTESTATION` 覆盖完整矩阵或每个 required Provider 的 per-provider attestation，run-plan 会显式展开 9 个 required Provider 的 endpoint/key/attestation env；软件阶段需要 Unreal/Hermes endpoint、各软件显式 `POOL_*_ENDPOINT` 或 `POOL_*_COMMAND`，以及每个 adapter 的 `POOL_*_ARTIFACTS` 本地文件路径；通用 endpoint 会走标准 API/MCP 网关合同，也可指向转发到真实插件/gateway 的 `pool-cli software-api-bridge-worker <adapter-id>`，可先运行 `7-production-evidence-runner.sh --preflight` 做机器预检，merge/closeout 阶段优先用 PATH 上的 `pool-cli`，找不到时 fallback 到 `cargo run -q -p pool-cli --`，也可用 `POOL_CLI_CMD` 覆盖，默认执行 Provider/software matrix、merge 和 closeout preflight，只有设置 `POOL_RUN_DESKTOP_VISION=1`、`POOL_DESKTOP_VISION_TRACE`、`POOL_DESKTOP_VISION_EXTERNAL_ACTION_ID` 与 `POOL_DESKTOP_VISION_PRODUCTION_ATTESTATION` 才执行桌面视觉阶段，只有设置 `POOL_IMPORT_PRODUCTION_EVIDENCE=1` 才正式 import。通过“生成脚手架”读取 `GET /api/production-evidence/template`，可把 provider/software/desktop vision bundle 写入校验文本框；通过 `GET /api/production-evidence/item-template?task_id=...` 或只读 MCP resource `pool://production-evidence-item-template/<task-id>` 可生成单项 `submit-production-evidence-item` JSON wrapper，`POST /api/production-evidence/items/validate` 可在正式 submit 前对单项 item 做 `writes:0` 预检；通过 `GET /api/production-evidence/item-from-ledger?provider_request_id=...`、`software_action_id=...` 或 `desktop_vision_action_id=...` 可把已运行的 Provider/software/desktop recognition callback 账本整理成单项 production evidence item 草稿，并返回 schema、本地文件和 production flags 三重预检；通过 `GET /api/production-evidence/bundle-from-ledger?include_incomplete=true` 可把已 ready 的账本项批量收成 `.bundle`，并返回未 ready 诊断；通过“合并证据”调用 `POST /api/production-evidence/merge`，可把 textarea 中的单个 bundle、bundle array 或 `{bundles:[...]}` 合并成一个 `.bundle` 并写回 textarea，保持 `writes:0`。Runtime 另提供 `POST /api/production-evidence/closeout`，默认只执行 merge + validate 并返回 `ready_for_import`，只有显式 `import:true` 时才进入正式导入路径；导入成功后 closeout 顶层会返回 `completion_gate`、`ready_for_completion`、`prd_overall_status`、`prd_summary` 和完成证明包命令，传 `completion_package` 时可直接物化本地 PRD completion package。界面会展示覆盖数量、operator checklist、`pool-cli production-evidence-claim`、`pool-cli production-evidence-run-plan`、`pool-cli merge-production-evidence`、`validate-production-evidence`、`validate-production-evidence-item`、`import-production-evidence`、`production-evidence-item-template`、`production-evidence-item-from-ledger`、`production-evidence-bundle-from-ledger`、`production-evidence-handoff-package` 与 `submit-production-evidence-item` 命令；MCP stdio 同步提供 `pool://production-evidence-item-template/<task-id>`、`pool_production_evidence_task_claim`、`pool_validate_production_evidence_item`、`pool_production_evidence_run_plan`、`pool_merge_production_evidence`、`pool_production_evidence_item_from_ledger`、`pool_production_evidence_bundle_from_ledger` 和 `pool_closeout_production_evidence`，用于让 Hermes/Agent 生成真实执行计划、按 task 领取只读模板、登记任务领取、预检单项回填、把多路 runner/operator 返回的 bundle 合成一个 `.bundle`，从 runtime ledger 生成单项或批量回填草稿，并在需要时执行受控 closeout。该入口只生成待替换或待核验的证据；多路 runner 输出可先 merge 成单个 bundle，真实导入仍必须经过 validate/import/submit 的本地文件、production flags 和外部 id 校验。

`7-production-evidence-runner.sh --preflight` 只有在实际使用 `cargo run -q -p pool-cli --` fallback 或 `POOL_CLI_CMD` 以 `cargo` 开头时才要求本机安装 cargo；发行版可直接使用 PATH 上的 `pool-cli` 或自定义非 cargo `POOL_CLI_CMD`。

桌面视觉生产阶段还必须提供 `POOL_DESKTOP_VISION_PRODUCTION_ATTESTATION` 或 controller 命令的 `--production-attestation=`。只有 endpoint 而没有真实 controller/model run attestation 时，runner 会保留普通 callback/trace，但不会把结果计入可导入的 `desktop_vision[]` 生产证据。

凭证治理也进入 Web runtime 面板：连接 Runtime HTTP 后，Web prototype 会读取 `GET /api/api-keys?rotation_days=90`，把 `pool_api_key_audit` 的 `rotation_due`、`unencrypted`、backend、owner 和 age 摘要显示在 Runtime Budget 面板；runtime 写操作刷新后会重新读取该审计，确保保存 Key 后 UI 状态同步。

`ContentBurstRunner` 负责本地一键闭环：

- DB 为空时持久化默认内容爆发运行蓝图。
- 写入 image-blaster 风格 `worlds/<slug>/` 项目包。
- `agent_mode:stage` 默认写入 Hermes 决策会话和 transcript，并把 task 绑定回 Agent 节点；`hermes_http` 可显式调用 Hermes endpoint，`skip` 可关闭。
- `three_dgs_mode:auto` 会优先使用已配置 3DGS gateway；缺少配置或 gateway 失败时回落 `Mock3dgsProvider`；`gateway` / `mock` 可显式强制。
- `unreal_mode:auto` 会优先使用已配置 Unreal MCP/gateway；缺少配置或 MCP 失败时回落 `MockUnrealAdapter`；`unreal_mcp` / `mock` 可显式强制。
- 调用 `OutputPackageRunner` 生成视频、游戏、交互艺术三类 manifest。
- 返回统一 report，并刷新 RuntimeSnapshot。

可用命令：

```bash
cargo run -p pool-core --example run_content_burst
cargo run -p pool-core --example run_prd_readiness_smoke -- target/prd-readiness-runner
curl -X POST http://127.0.0.1:4788/api/workflow-runs \
  -H 'Content-Type: application/json' \
  -d '{"project_slug":"demo","title":"Runtime local content burst","prompt":"run creative input to 3DGS to Unreal to outputs","source_inputs":["worlds/demo/source/0-reference.png"],"duration_ms":12000,"agent_mode":"stage","three_dgs_mode":"auto","unreal_mode":"auto"}'

curl -X POST http://127.0.0.1:4788/api/workflow-runs \
  -H 'Content-Type: application/json' \
  -d '{"project_slug":"demo","title":"Gateway + Unreal MCP content burst","prompt":"force real adapter path for verification","agent_mode":"hermes_http","hermes_endpoint":"http://127.0.0.1:3900/hermes","three_dgs_mode":"gateway","three_dgs_endpoint":"http://127.0.0.1:8787","unreal_mode":"unreal_mcp","unreal_endpoint":"http://127.0.0.1:8788"}'
```

`ProviderTaskRunner` 负责统一 Provider 运行闭环：

- 用 adapter `estimate_cost_tokens` 补齐 task 成本估算
- 对省略审批参数的高成本 provider 自动进入 `waiting_approval`
- 将 Provider 请求/响应写入 `provider_requests` 账本
- 等待审批时，在 output dir 写入 `.0-provider-approval__<provider>-request.json` 本地请求包，并把路径写入 task 与 provider request ledger，供 Agent/Hermes/gateway 在真正调用外部 Provider 前审查
- 写入初始 task
- 拦截 `waiting_approval`
- 审批后通过 `/api/tasks/approve` 恢复同一 task 的 Provider run
- 失败/取消后通过 `/api/tasks/retry` 按 `provider_requests` 账本重跑同一 task，并追加新的 provider request attempt；新 attempt 的 `request_json.attempt.retry_of_provider_request_id` 会指向旧失败记录
- 调用 provider health
- 调用 provider submit
- 调用 provider poll
- 成功后调用 provider download
- 将本地输出写入 `assets`
- 将关键状态写入 `workflow_events`
- 接收外部 progress event，并统一写入 `workflow_events`

`materialize_project_envelope` 负责写入本地项目包：

- `worlds/<slug>/project.json`
- `worlds/<slug>/workflow.json`
- `worlds/<slug>/scene.json`
- `worlds/<slug>/source/`
- `worlds/<slug>/output/`
- `worlds/<slug>/output/requests/`
- `worlds/<slug>/output/control/`

可用命令：

```bash
cargo run -p pool-core --example persist_default_plan
cargo run -p pool-core --example run_mock_3dgs_task
```

`OutputPackageRunner` 负责把后段输出收束为本地交付包：

- 在 `worlds/<slug>/output/deliverables/` 写入三类 indexed manifest。
- `1-video-timeline.json` 描述视频时间线、轨道、镜头时长和转码目标。
- `2-game-build.json` 描述 Unreal 关卡、运行视口和构建目标。
- `3-interactive-cues.json` 描述交互艺术 cue graph、实时视觉源、音频源和 OSC/MIDI/DMX 控制接口。
- 三个 manifest 会同步进入 `assets` 表，task 状态和事件写入 SQLite，HTTP report 会返回供 Web 面板显示的 manifest summaries。
- `GET /api/output-packages` 与 `pool://output-packages` 会把已入库的三类 manifest 规整成 deliverable catalog，标记 `ready`、`missing` 或 `indexed_missing_file`，并暴露预览合同与控制路由。
- `POST /api/output-packages/results` 会把 Resolve/Unreal/TouchDesigner/MadMapper 等后段执行结果写回对应本地 manifest 的 `execution_result` / `execution_history`，同时写入 task 和事件流，供 Agent/Hermes 继续从 catalog 读取真实执行证据。

可用命令：

```bash
cargo run -p pool-core --example run_output_package
curl http://127.0.0.1:4788/api/output-packages?project=demo
curl -X POST http://127.0.0.1:4788/api/output-packages \
  -H 'Content-Type: application/json' \
  -d '{"project_slug":"demo","node_id":"outputs","title":"Runtime output package","source_assets":["worlds/demo/output/1-world.glb"],"duration_ms":12000}'
curl -X POST http://127.0.0.1:4788/api/output-packages/results \
  -H 'Content-Type: application/json' \
  -d '{"project_slug":"demo","node_id":"outputs","target":"game","status":"succeeded","runtime":"Unreal","adapter_id":"unreal","message":"play-in-editor viewport verified","artifacts":["unreal://level/demo_content_burst"],"metrics":[{"label":"fps","value":"60"}]}'
curl -X POST http://127.0.0.1:4788/api/handoff-packages \
  -H 'Content-Type: application/json' \
  -d '{"project_slug":"demo","node_id":"agent","title":"Runtime handoff package","output_dir":"worlds/demo/output","include_snapshot":true}'
```

`SoftwareAdapter` 负责外部软件控制执行骨架；当前提供 `UnrealMcpAdapter` 和 `MockUnrealAdapter`，用于验证 Unreal 优先控制路径：

- `health`
- `execute`
- `SoftwareControlAction`
- `SoftwareActionResult`

`UnrealMcpAdapter` 是第一优先级 `API/MCP` 路径：

- 默认 `GET /health` 检查 Unreal MCP/gateway。
- 默认 `POST /mcp` 提交 `CreateScene`、`ImportAsset`、`RunViewport`、`Render` 等动作。
- 可用 `POOL_UNREAL_MCP_ENDPOINT` 或 `payload_json.endpoint` 指定 endpoint。
- request 会补 `pool_unreal_action` 与 `mcp_payload`，把 `OpenProject`、`ImportAsset`、`CreateScene`、`RunViewport`、`Render`、`ExportBuild` 映射成稳定的 Unreal MCP tool schema。
- Python 插件侧 `unreal.create_scene` 会读取 `asset_paths`、`actors[]`、`cameras[]`、`lights[]`、`world_origin` 和 `output_dir`，并写出 scene assembly manifest，作为真实 UE 项目映射前的本地审计真相源。
- 返回的 `artifacts`、`viewport_url`、`level_url` 会写入 `SoftwareActionResult`。
- Runtime HTTP 在未配置 endpoint 时继续使用 `MockUnrealAdapter`，保证本地 smoke 与 UI 验证不依赖 Unreal 进程。

`HermesMcpAdapter` 让 Hermes 也能作为软件矩阵里的 `API/MCP` 控制入口：

- 默认 `GET /health` 检查 Hermes MCP/gateway。
- 默认 `POST /mcp` 提交 Agent 控制动作。
- 可用 `POOL_HERMES_MCP_ENDPOINT`、`POOL_HERMES_ENDPOINT` 或 `payload_json.endpoint` 指定 endpoint。
- request 会补 `pool_hermes_action` 与 `mcp_payload`，把 `OpenProject`、`CreateScene`、`RunViewport`、`Render`、`ExportBuild` 等控制意图映射成 Hermes MCP tool schema。
- Runtime HTTP 在未配置 endpoint 时不会伪装成功，而是进入 human takeover 队列。

`SoftwareActionRunner` 负责把外部软件动作纳入统一运行审计：

- 插入或更新 `RuntimeTask`
- 拦截 `requires_confirmation` / `requires_approval`，进入 `waiting_approval`
- 审批后 `/api/tasks/approve` 会读取最近的 `software_actions.command_json`，清除确认标记，并用同一 task 恢复执行原动作
- 失败/取消后 `/api/tasks/retry` 会读取最近的 `software_actions.command_json`，用同一 task 重跑原动作
- 调用 adapter `health`
- 调用 adapter `execute`
- 将 action payload 与 verification result 写入 `software_actions`
- 将开始、health、完成或失败写入 `workflow_events`
- 将任务状态同步为 `running / waiting_approval / succeeded / failed`

`CommandSoftwareAdapter` 提供第一层 Skills/CLI 控制骨架：

- 支持 `action_kind:"ExecuteCli"`
- 从 `payload_json.command` 解析二进制和参数，不经过 shell
- 只有命中 `payload_json.allowed_commands` 的命令才会执行
- 记录 exit code、stdout/stderr 摘要和 artifacts

`DesktopRecognitionAdapter` 提供第一层桌面识别 fallback：

- 支持 `priority:"DesktopRecognition"`、`DesktopClick` 和 `DesktopHotkey`。
- 将控制意图、目标窗口、视觉目标、原始 payload 写入 `output/control/desktop-recognition/*.json`。
- request 会补 `pool_desktop_action` 与 `desktop_payload`，把 `RunViewport`、`Render`、`DesktopClick`、`DesktopHotkey` 等动作映射成稳定的桌面控制请求 schema。
- Runtime HTTP 已提供 `GET /api/desktop-recognition/requests`、`POST /api/desktop-recognition/run-next` 与 `POST /api/desktop-recognition/results`，供外部 desktop controller 领取请求、dry-run 推进队列、回填状态、artifacts、screen trace 和结果 payload。
- `pool-cli desktop-run-next` 会调用同一个 Runtime HTTP run-next endpoint，验证 controller handoff 不需要另写回填客户端。
- `run_desktop_recognition_controller --mode=applescript` 会读取同一队列，通过 macOS System Events 执行明确 `target_window`、`coordinates`、`hotkey` / `keys` 和 `text` / `type_text` / `input_text`，也可通过 `--vision-trace` 把外部视觉/OCR trace 中的 `visual_targets` 解析成点击坐标，再回填 `/api/desktop-recognition/results`。
- `run_desktop_recognition_controller --mode=vision-http --vision-endpoint=<url>` 会读取同一队列，把桌面请求 POST 给外部视觉/OCR endpoint，成功后写入 Pool-compatible 本地 trace，并回填 `external_visual_model:true`；正式生产证据 bundle 统一交给 `pool-cli production-evidence-desktop-vision --production-vision --trace <path> --external-action-id <id>` 整理；失败、缺少 attestation 或缺少本地 trace 文件时保持普通失败/非生产回填，不把本地 smoke 或弱外部标记冒充外部视觉模型生产证据。
- 生成的 request file 可由 Computer Use、OCR/视觉识别进程或人工接管工具消费。
- 当前完成请求落地、队列读取、dry-run、AppleScript 确定性桌面执行、视觉 trace 到点击坐标桥接、`vision-http` 外部视觉/OCR 服务调用、本地 trace 落盘、结果回填和审计；屏幕采集与具体视觉模型仍是外部 controller/服务职责。

可用命令：

```bash
cargo run -p pool-core --example run_mock_unreal_action
cargo run -p pool-core --example run_unreal_mcp_action
POOL_UNREAL_MCP_ENDPOINT=http://127.0.0.1:8787 cargo run -p pool-core --example run_unreal_mcp_action -- http://127.0.0.1:8787 target/unreal-mcp-runner
cargo run -p pool-core --example stage_desktop_recognition_action
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo desktop-run-next --controller-id local-vision-dry-run
cargo run -p pool-core --example run_desktop_recognition_controller -- http://127.0.0.1:4788 --project=demo --status=succeeded
cargo run -p pool-core --example run_desktop_recognition_controller -- http://127.0.0.1:4788 --project=demo --mode=applescript --vision-trace=worlds/demo/output/control/desktop-recognition/trace.json
POOL_DESKTOP_VISION_PRODUCTION_ATTESTATION=real-vision-controller-run-001 pool-cli --project demo production-evidence-desktop-vision target/desktop-vision-evidence --production-vision --trace worlds/demo/output/control/desktop-recognition/external-vision-trace.json --external-action-id=real-vision-action-001 --evidence-bundle=target/desktop-vision-evidence/desktop-vision-production-evidence-bundle.json
```

`AgentSessionRunner` 负责把 Hermes 与 Agent CLI 纳入统一任务和会话审计：

- staging Hermes command，并写入本地 transcript/control JSON
- 在显式 `execute:true` 时调用 Hermes HTTP endpoint，并记录 HTTP status/response body
- staging Agent CLI command，并记录命令模板、工具列表和 token budget
- 在显式 `execute:true` 且命令命中 allowlist 时，用非 shell 方式执行 Agent CLI 并记录 stdout/stderr/exit code
- 当显式 `execute:true` 先被 `requires_confirmation` 或 token budget 阻断时，把 `execution_request` 写入 transcript，审批/重试后用同一 task 恢复执行
- `pool-cli` 已作为本仓库的本地 Agent CLI 入口，可读取 runtime graph、workflow context、node context、MCP resource，并可触发 `run-node`
- 将会话写入 `agent_sessions`
- 将任务写入 `tasks`
- 将会话状态写入 `workflow_events`
- token 预算超限或命令要求确认时进入 `waiting_approval`

Hermes 的职责现在分为两条路径：`/api/agent-sessions` 管会话、transcript 和 Agent CLI 审计；`/api/software-actions` 的 `HermesMcpAdapter` 管软件矩阵中的 Hermes MCP 控制动作。

可用命令：

```bash
cargo run -p pool-core --example stage_agent_sessions
cargo run -p pool-cli -- --db target/runtime-http-smoke/pool-runtime.sqlite --project demo node-context
```

`ComfyUiProvider` 已进入真实 HTTP adapter 骨架：

- `GET /system_stats` 做 health check
- `POST /prompt` 提交 workflow API JSON
- `GET /history/{prompt_id}` 轮询状态
- `GET /ws?clientId=...` 接收 progress，并转换成 `RuntimeEvent`
- `GET /view` 下载图片/视频输出到本地 `output_dir`
- 下载后的本地输出同步成 `AssetRecord`
- request metadata 写入 `.comfyui-<prompt_id>-request.json`

可用命令：

```bash
cargo run -p pool-core --example comfyui_smoke
cargo run -p pool-core --example comfyui_smoke -- /path/to/workflow_api.json target/comfyui-smoke
cargo run -p pool-core --example comfyui_smoke -- /path/to/workflow_api.json target/comfyui-smoke target/comfyui-events.sqlite
cargo run -p pool-core --example comfyui_smoke -- /path/to/workflow_api.json target/comfyui-smoke target/comfyui-events.sqlite index-assets
```

`KlingProvider` 已进入真实 HTTP adapter 骨架：

- `POOL_KLING_API_KEY` 支持 Bearer token 认证路径。
- `POOL_KLING_ACCESS_KEY` + `POOL_KLING_SECRET_KEY` 支持官方 Access/Secret JWT 认证路径。
- `POOL_KLING_ENDPOINT` 可切换第三方 gateway 或官方兼容 endpoint。
- `POST /v1/videos/text2video` 提交文字转视频。
- `POST /v1/videos/image2video` 提交图片转视频。
- `GET /v1/videos/{task_id}` 轮询任务状态和获取结果 URL。
- 输出 URL 必须下载为本地 `N-kling-output.mp4`，再进入 `assets` 表；远程 URL 只做 provenance。
- request metadata 写入 `.kling-<task_id>-request.json`。

可用命令：

```bash
cargo run -p pool-core --example kling_smoke
POOL_KLING_API_KEY=... cargo run -p pool-core --example kling_smoke -- /path/to/kling-request.json target/kling-smoke
```

`OpenAiImageProvider` 已进入真实 HTTP adapter 骨架：

- `OPENAI_API_KEY` 支持 Bearer token 认证。
- `OPENAI_ORG_ID` 与 `OPENAI_PROJECT_ID` 可写入 OpenAI organization/project headers。
- `POOL_OPENAI_ENDPOINT` 可切换官方或兼容代理 endpoint。
- `POOL_OPENAI_IMAGE_MODEL` 可覆盖默认模型，默认 `gpt-image-2`。
- `POST /v1/images/generations` 提交图片生成任务。
- `b64_json` 直接解码成本地 `N-openai-image.png`；URL 输出也必须先下载成本地文件。
- metadata 保存 request、response summary、usage 和本地路径，不保存大体积 base64 payload。

可用命令：

```bash
cargo run -p pool-core --example openai_image_smoke
OPENAI_API_KEY=... cargo run -p pool-core --example openai_image_smoke -- /path/to/openai-image-request.json target/openai-image-smoke
```

`GenericHttpMediaProvider` 已进入通用 HTTP media gateway adapter 骨架：

- 覆盖 Midjourney、Nano Banana Pro、Suno 的首版可执行接入。
- 默认 submit: `POST /v1/media/jobs`
- 默认 poll: `GET /v1/media/jobs/{job_id}`
- `POOL_MEDIA_GATEWAY_ENDPOINT` / `POOL_MEDIA_GATEWAY_API_KEY` 可配置统一 gateway。
- `POOL_<PROVIDER_ID>_ENDPOINT` / `POOL_<PROVIDER_ID>_API_KEY` 可按 provider 覆盖。
- `midjourney`、`nano-banana-pro`、`suno` 已有默认 output slug/extension、`pool_media_profile` 与 `provider_payload`。
- 支持 URL、`b64_json`、`base64`、data URL 和已存在 `local_path` 输出。
- 远程输出必须先下载为本地 media files，再进入 `assets` 表。
- request metadata 写入 `.N-<output_slug>__<provider>-request.json`，metadata 不保存大体积 base64 payload。

可用命令：

```bash
cargo run -p pool-core --example generic_media_smoke
POOL_MEDIA_GATEWAY_ENDPOINT=http://127.0.0.1:8787 cargo run -p pool-core --example generic_media_smoke -- nano-banana-pro /path/to/media-request.json target/generic-media-smoke
POOL_MEDIA_GATEWAY_ENDPOINT=http://127.0.0.1:8787 POOL_PROVIDER_PRODUCTION_ATTESTATION=real-media-worker-run-001 POOL_PROVIDER_EVIDENCE_BUNDLE=target/generic-media-smoke/provider-production-evidence-bundle.json cargo run -p pool-core --example generic_media_smoke -- nano-banana-pro /path/to/media-request.json target/generic-media-smoke
```

`ThreeDgsGatewayProvider` 已进入通用 HTTP adapter 骨架，并增加 Pool gateway profile mapping：

- 默认 submit: `POST /v1/3dgs/jobs`
- 默认 poll: `GET /v1/3dgs/jobs/{job_id}`
- `POOL_3DGS_GATEWAY_ENDPOINT` / `POOL_3DGS_GATEWAY_API_KEY` 可配置统一 gateway。
- `POOL_<PROVIDER_ID>_ENDPOINT` / `POOL_<PROVIDER_ID>_API_KEY` 可按 provider 覆盖。
- `worldlabs-marble`、`tripo-splat`、`sam-3d`、`spark-3dgs`、`qunhe-3d` 已有默认 submit/poll path、默认 output slug、`pool_gateway_profile` 与 `provider_payload`。
- request 会声明 `output_contract=image-blaster-indexed-files`。
- 远程输出必须下载为 `N-world.json`、`N-world.glb`、`N-world-full_res.spz` 等本地 indexed files。
- request metadata 写入 `.N-world__<provider>-request.json`。

可用命令：

```bash
cargo run -p pool-core --example three_dgs_gateway_smoke
POOL_3DGS_GATEWAY_ENDPOINT=http://127.0.0.1:8787 cargo run -p pool-core --example three_dgs_gateway_smoke -- /path/to/3dgs-request.json target/three-dgs-gateway-smoke worldlabs-marble
POOL_3DGS_GATEWAY_ENDPOINT=http://127.0.0.1:8787 POOL_PROVIDER_PRODUCTION_ATTESTATION=real-3dgs-worker-run-001 POOL_PROVIDER_EVIDENCE_BUNDLE=target/three-dgs-gateway-smoke/provider-production-evidence-bundle.json cargo run -p pool-core --example three_dgs_gateway_smoke -- /path/to/3dgs-request.json target/three-dgs-gateway-smoke worldlabs-marble
```

## 下一步实现顺序

1. 按 `/api/unreal-mcp-bridge` / `pool://unreal-mcp-bridge` 合同继续深化真实 Unreal 插件侧 tool：当前 Python 插件已支持 create_scene assembly manifest、资产导入、actor placement、相机和灯光参数归档；下一步补 Blueprint/Sequencer、Play-in-Editor、Movie Render Queue、构建发布和真实 UE 项目证据。
2. 把 3DGS provider profile 接到真实本地 gateway 或官方 SDK adapter，并按 `/api/provider-gateway-worker.conformance_runbook` 跑通真实服务的 schema、鉴权、submit/poll、download、本地 artifact 与 production evidence 验收。
3. 把 Midjourney、Nano Banana Pro、Suno media profile 接到真实本地 gateway 或官方 SDK adapter，替换当前 profile-only 翻译。
4. 在现有 passphrase/Keychain credential backend 和本地 rotation audit 基础上，继续补更细的权限审计、团队环境迁移和组织级凭证策略。
