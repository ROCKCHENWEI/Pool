# Agent Session Runner

## 目标

`AgentSessionRunner` 是 Pool 的 Hermes 内嵌控制与 Agent CLI 命令 staging/执行层。它把 Agent 指令从浏览器原型里的按钮推进为 runtime 可审计对象。

它当前可在显式 `execute:true` 时调用 Hermes HTTP endpoint；Hermes 作为软件矩阵/API-MCP 控制入口时由 `HermesMcpAdapter`、`hermes-mcp-bridge-worker` 和 `/api/software-actions` 承接。Agent CLI 只在显式 `execute:true` 且命令命中 allowlist 时用非 shell 方式受控执行。若 `execute:true` 因 `requires_confirmation` 或 token budget 进入 `waiting_approval`，runner 会把 `execution_request` 写入 transcript，后续 `/api/tasks/approve` 或 `/api/tasks/retry` 可用同一个 task 恢复执行。默认路径仍先完成本地优先的运行闭环：

- `RuntimeTask`
- `agent_sessions`
- `workflow_events`
- transcript/control JSON
- token budget / token used
- approval gate
- Hermes HTTP status/response body
- allowlist execution stdout/stderr/exit code
- execution request resume metadata

## Hermes Command

`HermesCommand` 包含：

- endpoint
- project slug
- instruction
- allowed tools
- requires confirmation

runner 会写入：

```text
worlds/<slug>/output/control/<slug>/hermes-<session_id>-transcript.json
```

如果 `requires_confirmation` 为 true，任务进入 `waiting_approval`。

如果 Runtime HTTP 请求包含 `execute:true`，runner 会 POST 到 `endpoint`，请求体包含 `project_slug`、`instruction`、`allowed_tools` 和 `requires_confirmation`。执行结果会写入同一个 transcript，并把 task 更新为 `succeeded` 或 `failed`。如果本地 API Key 表存在 `hermes/agent` 或 `hermes/provider`，Runtime HTTP 会以 Bearer token 形式传给 Hermes endpoint。若这次执行先被人工确认阻断，transcript 会记录 `execution_request.channel="hermes_http"`、timeout 和响应大小限制；审批后会清除 `requires_confirmation` 并恢复执行。

Agent/MCP 读取侧可通过 `pool://agent-sessions` 查看 Hermes 与 Agent CLI 会话、token 使用摘要和 transcript 路径；软件控制侧的 Hermes MCP 动作会进入 `pool://software-actions`。本地 `pool-cli hermes-mcp-bridge-worker` 可作为真实 Hermes MCP 服务的前置 dry-run/forwarder，审计 `pool_hermes_action` 与 `mcp_payload`。

## Agent CLI Command

`AgentCliCommand` 包含：

- id
- title
- command
- tools
- token budget

runner 会写入：

```text
worlds/<slug>/output/control/<slug>/agent-cli-<session_id>-transcript.json
```

如果估算 token 超过 `token_budget`，任务进入 `waiting_approval`，等待人工确认。

如果 Runtime HTTP 请求包含 `execute:true`，runner 会在任务未进入审批阻断时解析命令、校验 `allowed_commands`，再用 `std::process::Command` 直接执行二进制，不经过 shell。执行结果会写入同一个 transcript，并把 task 更新为 `succeeded` 或 `failed`。若 token budget 先触发审批，transcript 会记录 `execution_request.channel="agent_cli"`、allowlist、working dir、timeout 和输出上限；审批后会用同一 task 恢复执行。纯 staging 且没有 `execution_request` 的会话只会在审批后释放到 `ready`，不会被误执行。

当前仓库已提供 `pool-cli` 二进制，适合作为 Agent CLI 默认控制命令。常用模板：

```text
pool-cli --project demo node-context
pool-cli --project demo workflow-context
pool-cli --project demo runtime-graph
pool-cli --project demo mcp pool://tasks
```

## Agent/Hermes Conformance Package

Agent/Hermes 验收包把当前 session 合同、执行 runbook、preflight、runner script 和 manifest 写成本地文件，方便把 Hermes 内嵌控制与 Agent CLI allowlist 执行交给 Agent 或具体操作者验收。

```bash
pool-cli --project demo agent-conformance-package all --output-dir worlds/demo/output
pool-cli --project demo agent-conformance-package hermes --output-dir worlds/demo/output
pool-cli --project demo agent-conformance-package agent-cli --output-dir worlds/demo/output
```

Runtime HTTP 同源入口：

```bash
curl -X POST http://127.0.0.1:4788/api/agent-conformance-packages \
  -H 'Content-Type: application/json' \
  -d '{"project_slug":"demo","kind":"all","node_id":"agent","output_dir":"worlds/demo/output"}'
```

输出目录为 `worlds/demo/output/control/agent-conformance/<kind>/`。`4-agent-conformance-runner.sh local` 会跑 Hermes bridge worker baseline 和 Agent CLI allowlist smoke；`run` 模式会按 runbook staging Hermes session、执行 Hermes HTTP、staging Agent CLI、执行 allowlist CLI，并要求 `POOL_HERMES_ENDPOINT`。

## Smoke

```bash
cargo run -p pool-core --example stage_agent_sessions
```

输出包括：

- SQLite path
- Hermes status
- Hermes transcript path
- Agent CLI status
- Agent CLI transcript path
- tasks / agent_sessions / workflow_events 计数

Runtime HTTP 会话写入：

```bash
cargo run -p pool-core --example serve_runtime_http -- target/runtime-http-smoke/pool-runtime.sqlite --bind=127.0.0.1:4788
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
  -d '{"kind":"agent_cli","project_slug":"demo","command_id":"echo","title":"Execute allowed command","command":"/bin/echo runtime-agent-ok","tools":["cli"],"token_budget":4000,"execute":true,"allowed_commands":["/bin/echo","echo"],"timeout_ms":2000}'
```

## 当前边界

- 已实现 staging、DB 写入、Runtime HTTP 会话写入、transcript 落地、token 估算、审批阻断、Hermes HTTP 执行、Agent CLI allowlist 受控执行，以及带 `execution_request` 的 Agent session 审批/重试恢复执行。
- Hermes MCP 作为 `SoftwareAdapter` 已接入 `/api/software-actions`，会生成 `pool_hermes_action` 与 `mcp_payload`；`hermes-mcp-bridge-worker` 已能 dry-run 校验和转发该 wrapper；真实 Hermes 服务端仍需实现对应 tool/schema。
- Agent CLI 执行器不支持 shell 管道、重定向或交互式 TUI；需要用明确二进制和参数表达命令。
- 后续 Hermes 执行器应复用 session/task/transcript，不另建并行状态机。
