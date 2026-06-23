# Unreal MCP Bridge Contract

## 目标

`Unreal MCP Bridge Contract` 是给 Unreal 插件或本地 gateway 实现方读取的机器可读协议。它把 Pool runtime 已经生成的 `pool_unreal_action` 与 `mcp_payload` 固定下来，让 Unreal 侧能按同一份工具 schema 执行导入、组装、视口、Sequencer 渲染和构建输出。

仓库现在提供一个可安装的 Unreal Python 插件脚手架：`integrations/unreal/PoolMcpBridge`。它实现 `GET /health`、`POST /mcp`、`POST /v1/unreal/actions`、Pool wrapper 校验、request/response 审计落地，以及 `unreal.*` tool 的基础分派。真实 UE 项目内的 Blueprint、Sequencer、Play-in-Editor、Movie Render Queue 和构建发布仍需要继续按项目版本补深度 mapping 与生产证据。

## 读取入口

```bash
curl http://127.0.0.1:4788/api/unreal-mcp-bridge
cargo run -p pool-cli -- unreal-mcp-bridge
cargo run -p pool-cli -- mcp pool://unreal-mcp-bridge
```

MCP stdio tool：

```json
{"name":"pool_unreal_mcp_bridge","arguments":{}}
```

## 本地 Worker

`pool-cli unreal-mcp-bridge-worker` 可启动一个本地 `/health` + `/mcp` bridge worker：

先做本地自检：

```bash
cargo run -p pool-cli -- unreal-mcp-bridge-worker \
  --once \
  --output-root worlds/demo/output
```

自检会执行 `GET /health` 和一次 dry-run `POST /mcp`，并把 request/response 审计文件写到本地，不占用常驻端口。

```bash
cargo run -p pool-cli -- unreal-mcp-bridge-worker \
  --bind 127.0.0.1:8790 \
  --output-root worlds/demo/output
```

默认 dry-run 模式会：

- 校验 `adapter_id`、`pool_unreal_action` 和 `mcp_payload`。
- 将原始请求写入 `worlds/demo/output/control/unreal-mcp-bridge/*-request.json`。
- 将结构化响应写入 `worlds/demo/output/control/unreal-mcp-bridge/*-response.json`。
- 返回 `ok:true`、`artifacts` 和 `pool_unreal_bridge` 审计字段。

接真实 Unreal 插件或 gateway 时加 `--upstream`：

```bash
cargo run -p pool-cli -- unreal-mcp-bridge-worker \
  --bind 127.0.0.1:8790 \
  --output-root worlds/demo/output \
  --upstream http://127.0.0.1:8791
```

然后让 Pool 的 Unreal adapter 指向这个 worker：

```bash
POOL_UNREAL_MCP_ENDPOINT=http://127.0.0.1:8790 cargo run -p pool-core --example run_unreal_mcp_action
```

## Unreal Python 插件脚手架

插件路径：

```text
integrations/unreal/PoolMcpBridge
```

安装方式：

1. 复制 `integrations/unreal/PoolMcpBridge` 到 `<YourProject>/Plugins/PoolMcpBridge`。
2. 启用 Unreal 的 `PythonScriptPlugin` 和 `EditorScriptingUtilities`。
3. 重启 Unreal Editor。`Content/Python/init_unreal.py` 默认会启动 HTTP bridge。

环境变量：

- `POOL_UNREAL_MCP_HOST`：默认 `127.0.0.1`
- `POOL_UNREAL_MCP_PORT`：默认 `8791`
- `POOL_UNREAL_MCP_AUDIT_ROOT`：默认 `Saved/PoolMcpBridge`
- `POOL_UNREAL_MCP_AUTOSTART=0`：禁用自动启动

Pool 可直接指向插件：

```bash
POOL_UNREAL_MCP_ENDPOINT=http://127.0.0.1:8791 cargo run -p pool-core --example run_unreal_mcp_action
```

也可以让 `unreal-mcp-bridge-worker` 指向插件作为前置校验/审计代理。

## 契约内容

合同包含：

- Runtime routes：`/api/unreal-mcp-bridge`、`pool://unreal-mcp-bridge`、`/api/software-actions`、`/api/software-health`。
- Endpoint 环境变量：`POOL_UNREAL_MCP_ENDPOINT`、`POOL_UNREAL_MCP_TOKEN`、`POOL_UNREAL_MCP_HEALTH_PATH`、`POOL_UNREAL_MCP_ACTION_PATH`。
- Transport：默认 `GET /health` 和 `POST /mcp`。
- Request wrapper：根字段、`pool_unreal_action` 元数据、`mcp_payload` 工具调用。
- Tool contracts：`unreal.open_project`、`unreal.import_asset`、`unreal.create_scene`、`unreal.run_viewport`、`unreal.render_sequence`、`unreal.export_build`、`unreal.transcode_media`、`unreal.health`。
- Response contract：`ok` / `success`、`status` / `state`、`message`、`artifacts`。

## 插件侧最低要求

Unreal 插件或 gateway 应实现；当前 Python 脚手架已覆盖前 4 项：

1. `GET /health` 返回 `{ "ok": true }`。
2. `POST /mcp` 接收 Pool body，记录 `pool_unreal_action.profile_id` 与 `mcp_payload.tool`。
3. 按 `mcp_payload.tool` 调用 Unreal Editor 能力。
4. 成功时返回 `ok:true` 和 `artifacts`。
5. 远程 URL 只能作为 provenance；供前端加载的输出必须先落成本地文件或 Unreal/Pool 可解析 URI。

## 当前状态

Pool 已完成 contract、HTTP、MCP resource、CLI command、bridge worker、Unreal Python 插件脚手架和本地语法检查。`unreal.create_scene` 插件侧已能读取 `asset_paths`、`actors[]`、`cameras[]`、`lights[]`、`world_origin` 和 `output_dir`，在可用 Unreal Python API 下尝试生成关卡、导入资产、放置 actor、添加相机/灯光，并始终写出 `pool_unreal_scene_assembly` manifest 作为本地真相源。下一步是在真实 Unreal Editor 项目内跑通插件，补齐 Blueprint/Sequencer、Play-in-Editor、Movie Render Queue 和 build manifest 的项目级执行 mapping。

本地 worker 已能作为 Unreal MCP adapter 的 smoke target 和真实插件前置代理；Python 插件脚手架可作为真实 Editor 侧起点，但还未提供实际项目中的 UE 运行截图、渲染产物和构建产物证据。
