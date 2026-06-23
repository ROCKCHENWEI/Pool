# Content Burst Runner

`ContentBurstRunner` 是 Pool 首版本地闭环执行器。它把已经实现的运行模块串成一条可验证生产线：

```text
默认运行蓝图 -> image-blaster 项目包 -> Agent/Hermes 决策 -> 3DGS -> Unreal -> 三类输出交付包
```

它不是最终通用工作流引擎，而是本地优先 MVP 的 smoke runner，用来证明节点图可以落到 SQLite、文件系统、软件控制审计和资产台账。

## 执行内容

- 如果 runtime DB 为空，写入默认 `build_default_content_burst_plan`。
- 写入 `worlds/<slug>/project.json`、`workflow.json`、`scene.json` 和本地目录结构。
- 通过 `AgentSessionRunner` 写入 Agent/Hermes 决策会话，记录 adapter 选择、失败回退和人工接管建议，并把 task 绑定回 Agent 节点。
- 通过 `ProviderTaskRunner` 执行 3DGS：`auto` 模式优先使用已配置 3DGS gateway，缺省或失败时回落 `Mock3dgsProvider`。
- 通过 `SoftwareActionRunner` 执行 Unreal：`auto` 模式优先使用已配置 Unreal MCP，缺省或失败时回落 `MockUnrealAdapter`。
- 通过 `OutputPackageRunner` 生成视频、游戏、交互艺术三类 manifest。
- 返回统一 report，并刷新 `RuntimeSnapshot`。

## Adapter 模式

- `agent_mode:"stage"`：默认只 staging Hermes 决策会话，不调用外部 endpoint。
- `agent_mode:"hermes_http"`：调用 `hermes_endpoint`，并把 HTTP status/response body 写回 transcript。
- `agent_mode:"skip"`：跳过本次 Agent/Hermes 决策会话。
- `three_dgs_mode:"auto"`：有 `three_dgs_endpoint` 或 `POOL_3DGS_GATEWAY_ENDPOINT` 时走 `ThreeDgsGatewayProvider`，否则走 mock。
- `three_dgs_mode:"gateway"`：强制走 3DGS gateway。
- `three_dgs_mode:"mock"`：强制走本地 mock。
- `unreal_mode:"auto"`：有 `unreal_endpoint` 或 `POOL_UNREAL_MCP_ENDPOINT` 时走 `UnrealMcpAdapter`，否则走 mock。
- `unreal_mode:"unreal_mcp"`：强制走 Unreal MCP。
- `unreal_mode:"mock"`：强制走本地 mock。

## Runtime HTTP

本地 runtime 暴露：

```text
POST /api/workflow-runs
```

请求示例：

```bash
curl -X POST http://127.0.0.1:4788/api/workflow-runs \
  -H 'Content-Type: application/json' \
  -d '{"project_slug":"demo","title":"Runtime local content burst","prompt":"run creative input to 3DGS to Unreal to outputs","source_inputs":["worlds/demo/source/0-reference.png"],"duration_ms":12000,"agent_mode":"stage","three_dgs_mode":"auto","unreal_mode":"auto"}'
```

强制真实 gateway/MCP 的请求示例：

```bash
curl -X POST http://127.0.0.1:4788/api/workflow-runs \
  -H 'Content-Type: application/json' \
  -d '{"project_slug":"demo","title":"Runtime gateway content burst","prompt":"run real adapters","source_inputs":["worlds/demo/source/0-reference.png"],"agent_mode":"hermes_http","hermes_endpoint":"http://127.0.0.1:3900/hermes","three_dgs_mode":"gateway","three_dgs_endpoint":"http://127.0.0.1:8787","unreal_mode":"unreal_mcp","unreal_endpoint":"http://127.0.0.1:8788"}'
```

Web prototype 连接 Runtime HTTP 后，顶部“运行一次”按钮会优先调用这个 endpoint；失败时回退到本地演示步进。

## Pool CLI

`pool-cli run-workflow` 复用同一个 `/api/workflow-runs` endpoint，适合 Hermes、Agent CLI 或本地脚本直接触发完整闭环：

```bash
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo run-workflow \
  --title "CLI local content burst" \
  --prompt "run creative input to 3DGS to Unreal to outputs" \
  --source-input worlds/demo/source/0-reference.png \
  --agent-mode stage \
  --three-dgs-mode mock \
  --unreal-mode mock \
  --duration-ms 12000
```

真实 adapter 路径可用 `--agent-mode hermes_http`、`--hermes-endpoint`、`--three-dgs-mode gateway`、`--three-dgs-endpoint`、`--unreal-mode unreal_mcp` 和 `--unreal-endpoint` 显式指定。

## Smoke

```bash
cargo run -p pool-core --example run_content_burst
```

默认输出：

```text
target/content-burst-runner/pool-runtime.sqlite
target/content-burst-runner/worlds/demo/project.json
target/content-burst-runner/worlds/demo/output/deliverables/1-video-timeline.json
target/content-burst-runner/worlds/demo/output/deliverables/2-game-build.json
target/content-burst-runner/worlds/demo/output/deliverables/3-interactive-cues.json
```
