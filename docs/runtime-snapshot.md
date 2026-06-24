# Runtime Snapshot

## 目标

`RuntimeSnapshot` 是 Pool Web/SwiftUI 控制台的只读数据出口。它把 SQLite runtime 中的状态整理成一个稳定 JSON，供节点图、运行中心、资产台账、Hermes/Agent 面板和软件控制面板读取。

它是替换浏览器 `localStorage` 原型的第一步。

## 内容

Snapshot v1 包含：

- `projects`
- `workflows`
- `node_states`
- `tasks`
- `assets`
- `events`
- `provider_requests`
- `software_actions`
- `agent_sessions`
- `api_keys`
- `stats`

`node_states` 从 `tasks.node_id` 派生，用于节点图显示每个节点的运行状态、provider/software id、审批状态和更新时间。

`stats` 同时包含运行消耗汇总：

- `task_estimated_tokens`：所有 runtime tasks 的估算 token/cost。
- `waiting_approval_estimated_tokens`：正在等待人工确认的任务估算 token/cost。
- `provider_requests`：Provider 请求/响应账本数量。
- `agent_token_used`：Agent/Hermes 会话已估算使用 token。
- `agent_token_budget`：Agent/Hermes 会话预算总和。
- `token_total`：控制台用于消耗仪表的总量。
- `budget_remaining`：有预算时返回剩余额度。

## 使用

生成 smoke snapshot：

```bash
cargo run -p pool-core --example export_runtime_snapshot
```

默认输出：

```text
target/runtime-snapshot-smoke/runtime-snapshot.json
```

项目过滤：

```rust
let snapshot = repository.snapshot(Some("demo"))?;
```

全部项目：

```rust
let snapshot = repository.snapshot(None)?;
```

## UI 接入方向

Web/SwiftUI 控制台应优先读取 `RuntimeSnapshot`：

- 节点图读取 `workflows` + `node_states`，并用 `workflows.connections` 渲染资产流、控制流、Agent 指令、审批和反馈循环。
- 运行中心读取 `tasks` + `events`
- Provider/API 面板读取 `provider_requests`，展示请求、响应、metadata 路径和审批恢复状态。
- 资产台账读取 `assets`
- 软件控制面板读取 `software_actions`，在软件卡片和软件节点详情中展示 action kind、priority、verification message 和 artifacts；连接 Runtime HTTP 时还会读取 desktop recognition request 队列，并可回填成功/失败结果。
- Hermes/Agent 面板读取 `agent_sessions`
- API 接入面板读取脱敏 `api_keys`，只显示 `key_hint`、configured 状态和 credential storage metadata，不返回明文 key。

浏览器 `localStorage` 只保留为 prototype fallback，不再作为长期运行真相源。

当前 Web 原型已支持可选 snapshot 加载：

```text
http://localhost:4173/apps/web-prototype/?snapshot=/target/runtime-snapshot-smoke/runtime-snapshot.json
```

如果没有提供 `snapshot` query 参数，页面会尝试读取同目录的 `runtime-snapshot.json`；如果读取失败，则回退到原始静态演示状态和 `localStorage`。

Web 原型也支持直接读取本地 runtime HTTP；如果没有提供 `runtime` 或 `snapshot` query 参数，会自动探测默认本地端口组，探测失败后才回退默认 `runtime-snapshot.json`。`?runtime=local/auto` 会强制走同一套本地发现流程；`?runtime_ports=4788,4789` 和 `?runtime_endpoints=http://127.0.0.1:4788,http://127.0.0.1:4878` 可覆盖候选地址：

```text
http://localhost:4173/apps/web-prototype/?runtime=local
http://localhost:4173/apps/web-prototype/?runtime_registry=runtime-registry.json
http://localhost:4173/apps/web-prototype/?runtime=http://127.0.0.1:4788
http://localhost:4173/apps/web-prototype/?runtime=local&runtime_ports=4788,4789
http://localhost:4173/apps/web-prototype/
http://localhost:4173/apps/web-prototype/?runtime=local&project=demo
http://localhost:4173/apps/web-prototype/?runtime=local&project=*
```

`project` / `project_slug` query 会传给 Runtime HTTP 的 `/api/health` 和 `/api/snapshot`。如果用户显式提供项目过滤，Web prototype 会把选择写入本地 `localStorage`，后续无 query 参数打开时继续使用该项目过滤；空值或 `*` 表示全部项目。

连接 runtime 后，Web prototype 的 token 消耗仪表会优先读取 `snapshot.stats.token_total`，并在 `agent_token_budget` 存在时使用 runtime 预算替换静态演示预算。节点图会优先读取 `/api/runtime-graph` 的任务类型、连接通道和 labels，再叠加最新 snapshot task 状态；如果 runtime graph 不可用，则回退到 `workflows` + `node_states`。接入页会读取 `/api/runtime-budget` 或从 snapshot 派生预算与凭证摘要，显示总估算 token、待审批 token、Agent 预算余量、Provider Key 就绪比例、缺失凭证数和 Provider 请求数；还会读取 `/api/runtime-preflight` 或从 snapshot 派生运行前检查，显示阻塞项、警告和建议 CLI next actions，其中桌面接管会优先建议 `desktop-run-next` 并附带 `desktop-requests` 检查命令；也会读取 `/api/runtime-handoff` 或从当前状态派生执行接管 runbook，显示 Hermes/Agent/桌面 controller/人工 operator 的 lanes、控制优先级、命令列表和 5 人团队角色分工。Hermes 面板会读取 `/api/prompts` 填充 Agent runbook registry，并可从 `/api/prompts?name=...` 取回标准 prompt 写入指令框。单 workflow 下钻可读取 `/api/workflow-context?workflow_id=...` 或 `pool://workflow/<workflow-id>`，它会聚合该 workflow 的 graph、node_states、tasks、assets、provider_requests、software_actions、agent_sessions 和审批摘要。单节点下钻可读取 `/api/node-context?node_id=...` 或 `pool://node-context/<node-id>`，它会聚合该节点的 edges、tasks、assets、provider_requests、software_actions、相关 agent_sessions 和 `control_context` 建议控制入口。所有返回 snapshot 的 runtime 写操作都会重新读取 `/api/runtime-graph`、`/api/runtime-budget`、`/api/runtime-preflight`、`/api/runtime-handoff` 和 `/api/workflow-context` 后再合并状态，避免节点图拓扑、任务类型、连接通道、预算凭证、运行前检查、执行接管 runbook 或 workflow 账本摘要陈旧。Hermes 面板和节点详情侧栏会读取 `agent_sessions`、`provider_requests` 与匹配 task 的 `request_metadata_path`，展示最新 Agent/Hermes transcript、Provider request metadata、token 消耗和 ContentBurst adapter 模式。

OpenClaw/MCP resources 也应优先使用同一份 snapshot：

```rust
let snapshot = repository.snapshot(Some("demo"))?;
let server = McpServer::from_snapshot(snapshot);
let tasks_json = server.read_resource("pool://tasks")?;
let runtime_graph_json = server.read_resource("pool://runtime-graph")?;
let runtime_budget_json = server.read_resource("pool://runtime-budget")?;
let runtime_preflight_json = server.read_resource("pool://runtime-preflight")?;
let runtime_handoff_json = server.read_resource("pool://runtime-handoff")?;
let workflow_context_json = server.read_resource("pool://workflow/<workflow-id>")?;
let node_context_json = server.read_resource("pool://node-context/<node-id>")?;
let provider_requests_json = server.read_resource("pool://provider-requests")?;
let desktop_recognition_json = server.read_resource("pool://desktop-recognition")?;
```

本地 HTTP API 可直接读取同一份状态：

```bash
cargo run -p pool-core --example serve_runtime_http -- target/runtime-http-smoke/pool-runtime.sqlite once
```

常驻服务默认暴露：

- `/api/health`
- `/api/snapshot`
- `/api/resources`
- `/api/mcp?uri=pool://tasks`
- `/api/runtime-graph`
- `/api/runtime-budget`
- `/api/runtime-preflight`
- `/api/runtime-handoff`
- `/api/workflow-context?workflow_id=<workflow-id>`
- `/api/node-context?node_id=<node-id>`
- `/api/mcp?uri=pool://workflow/<workflow-id>`
- `/api/mcp?uri=pool://runtime-graph`
- `/api/mcp?uri=pool://runtime-budget`
- `/api/mcp?uri=pool://runtime-preflight`
- `/api/mcp?uri=pool://runtime-handoff`
- `/api/mcp?uri=pool://node-context/<node-id>`
- `/api/mcp?uri=pool://provider-requests`
- `/api/mcp?uri=pool://software-actions`
- `/api/mcp?uri=pool://integration-readiness`
- `/api/mcp?uri=pool://desktop-recognition`
- `/api/mcp?uri=pool://agent-sessions`
- `/api/api-keys`
- `/api/integration-readiness`
- `/api/tasks`
- `/api/provider-runs`
- `/api/agent-sessions`
- `/api/agent-sessions/ws`
- `/api/tasks/approve`
- `/api/software-actions`
- `/api/desktop-recognition/requests`
- `/api/desktop-recognition/run-next`
- `/api/desktop-recognition/results`

## 当前边界

- 已实现 SQLite snapshot 查询、project filter、provider_requests 账本、stats 和 smoke export。
- Web prototype 已实现可选 snapshot JSON 加载、runtime graph 读取、runtime budget/credential readiness 面板、runtime preflight 阻塞/警告/next actions 面板、runtime handoff 执行接管 runbook 面板、Provider contracts 读取与卡片摘要、workflow context / node context 下钻与 nodes/connections/tasks/assets/events/provider_requests/software_actions 映射，节点图会显示 runtime graph task type、workflow connection label、资产流、控制流、Agent 指令、审批和反馈循环；节点详情会读取 `/api/workflow-context?workflow_id=...` 展示当前 workflow 的任务、资产、Provider 请求、软件动作和 Agent session 摘要，并读取 `/api/node-context?node_id=...` 展示单节点任务、资产、Provider 请求、软件动作、Agent session 和 Runtime 控制入口；Provider 卡片会展示 Provider contract、请求账本与 metadata 路径，3DGS 节点详情会展示请求账本与 metadata 路径，软件卡片、软件节点详情和桌面识别接管队列会展示软件动作审计。
- OpenClaw/MCP resources 已实现 snapshot-backed read path，并新增 `pool://adapters` 读取 Provider/软件矩阵、Provider alias、控制优先级和本地优先策略，`pool://integration-readiness` 从 adapter catalog、api key、provider_requests、software_actions、tasks 和 agent_sessions 派生接入就绪矩阵、5 人团队 lane 与 next-action run plan。
- Runtime HTTP server 已实现 snapshot/MCP 读取 API、runtime budget/credential readiness 摘要、runtime preflight 阻塞/警告/next actions 摘要、runtime handoff 执行接管 runbook、runtime handoff 本地文件包，并支持 API Key 脱敏管理、任务创建、Provider adapter run、Provider 请求账本、Provider 审批/重试恢复、Hermes/Agent CLI 会话、软件动作、软件动作审批/重试恢复、desktop recognition controller 队列读取、dry-run 推进、结果回填和审批任务写入。
- Web prototype 已实现 `?runtime=`、默认端口组自动发现、`?runtime_registry=` 服务注册表读取、`?runtime_ports=`/`?runtime_endpoints=` 候选地址覆盖、`?project=` 项目过滤、顶部项目选择器与本地持久化读取 runtime HTTP API，并可从 Provider“运行”、Hermes、Agent CLI、软件矩阵、desktop recognition 接管队列、ContentBurst“运行一次”和人工确认按钮触发对应写入 API；Hermes 面板已接入 `/api/prompts` runbook registry 与 prompt get，Provider 面板已接入 `/api/provider-contracts` contract 摘要，节点详情侧栏已能显示 workflow run report 与 Agent session 决策摘要。
- Web prototype 已支持 `/api/events/ws` WebSocket 长连接日志、`/api/events/stream` EventSource/SSE fallback 和 `/api/events` 轮询 fallback。
