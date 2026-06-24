# Hermes MCP Bridge Worker

## 目标

`Hermes MCP Bridge Worker` 是 Hermes/Agent 内嵌控制的本地 HTTP bridge。它和 Unreal bridge worker 对称：先校验 Pool runtime 生成的 `pool_hermes_action` 与 `mcp_payload`，把 request/response 落成本地审计文件，再选择 dry-run 或转发到真实 Hermes MCP/gateway endpoint。

它不替代 `/api/agent-sessions`。`AgentSessionRunner` 仍负责 Hermes 决策会话、transcript、token budget 和审批恢复；Hermes bridge worker 负责 `/api/software-actions` 中 `adapter_id:"hermes"` 的 MCP 软件控制动作。

## 启动

先做本地自检：

```bash
cargo run -p pool-cli -- hermes-mcp-bridge-worker \
  --once \
  --output-root worlds/demo/output
```

自检会执行 `GET /health` 和一次 dry-run `POST /mcp`，并把 request/response 审计文件写到本地，不占用常驻端口。

```bash
cargo run -p pool-cli -- hermes-mcp-bridge-worker \
  --bind 127.0.0.1:8792 \
  --output-root worlds/demo/output
```

默认 dry-run 模式提供：

- `GET /health`
- `POST /mcp`
- `POST /v1/hermes/actions`

请求和响应会写入：

```text
worlds/demo/output/control/hermes-mcp-bridge/*-request.json
worlds/demo/output/control/hermes-mcp-bridge/*-response.json
```

接真实 Hermes 服务时：

```bash
cargo run -p pool-cli -- hermes-mcp-bridge-worker \
  --bind 127.0.0.1:8792 \
  --output-root worlds/demo/output \
  --upstream http://127.0.0.1:3900
```

然后让 Pool Hermes adapter 指向 worker：

```bash
POOL_HERMES_MCP_ENDPOINT=http://127.0.0.1:8792 cargo run -p pool-cli -- \
  --db target/pool-runtime.sqlite \
  --project demo \
  run-software hermes \
  --action-kind CreateScene \
  --priority ApiMcp \
  --payload-json '{"instruction":"coordinate Unreal scene assembly","target_adapter":"unreal","target_action_kind":"CreateScene"}'
```

## Request Contract

Worker 要求 Pool wrapper 包含：

- `adapter_id:"hermes"`
- `action_kind`
- `priority`
- `payload`
- `pool_hermes_action`
- `mcp_payload`

校验规则：

- `pool_hermes_action.mcp_tool` 必须等于 `mcp_payload.tool`
- `mcp_payload.tool` 必须以 `hermes.` 开头
- 远程 Hermes 响应中的 artifact 会被保留，同时追加本地 request/response 审计路径

## 当前边界

已验证本地 dry-run、wrapper 校验、审计文件落地、CLI 解析，以及 `HermesMcpAdapter` 指向 worker 的端到端执行。真实 Hermes 服务端仍需要实现 `hermes.open_project`、`hermes.coordinate`、`hermes.run_preview`、`hermes.output_control` 等 tool，并把真实 session/transcript 或执行报告回填为本地 artifact。
