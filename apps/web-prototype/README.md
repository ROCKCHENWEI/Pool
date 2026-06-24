# Pool 内容爆发工具原型

这是 `pool` 的本地静态原型，用来落地“5 人左右视频、游戏、交互艺术超级团队内容爆发工具”的总体规划。

## 当前实现

- 节点化流程图：展示创意输入、Agent 分析、2D/3D/3DGS 转换、外部软件控制、Unreal 拼装、三类输出；连接 runtime 后会读取 workflow connections，显示资产流、控制流、Agent 指令、审批和反馈循环。
- Agent 控制面板：展示 Agent 角色、工具权限、Token 消耗和运行状态；接入页会显示 Runtime 预算、凭证就绪和运行前检查摘要。
- 外部软件矩阵：覆盖 Unreal、Unity、DaVinci Resolve、剪辑软件、TouchDesigner、MadMapper、Blender、ComfyUI、动捕数据库、Nuke、Suno。
- 运行中心：展示任务队列、资产台账、Adapter 健康状态和事件流。
- API 接入：注册 AI 图片/视频生成 Provider、3DGS Provider、Hermes 内嵌控制和 Agent CLI 模板。
- 输出模块：视频、游戏、交互艺术三条生产路径。
- 交互控制：点击节点查看详情，点击“运行一次”推进节点状态；连接 runtime 时，“人工确认”会写入 Runtime HTTP 审批 API，Agent CLI 模板会按当前 project/workflow 生成真实 `pool-cli` 命令并写入 `/api/agent-sessions`，未连接时走本地模拟。
- Runtime 接入：支持 `?snapshot=...` 读取导出的 `RuntimeSnapshot`，也支持 `?runtime=http://127.0.0.1:4788` 直接读取本地 Runtime HTTP API；无 query 参数或 `?runtime=local/auto` 时会自动探测默认本地端口组，`?runtime_registry=runtime-registry.json`、`?runtime_ports=4788,4789` / `?runtime_endpoints=http://127.0.0.1:4788,http://127.0.0.1:4878` 可覆盖候选地址；`?project=slug` 可过滤项目，`?project=*` 可查看全部项目，连接后会读取 `/api/projects` 填充顶部项目选择器，读取 `/api/runtime-budget` 渲染预算、待审批 token、Provider Key 就绪和 Provider 请求摘要，读取 `/api/runtime-preflight` 渲染运行前阻塞项、警告和建议 CLI next actions，读取 `/api/runtime-handoff` 渲染 Hermes/Agent/桌面 controller/人工 operator 的执行接管 runbook，并可通过 `/api/handoff-packages` 将接管 runbook 落地为本地 handoff 文件包，读取 `/api/adapters` 同步 Provider 与软件 adapter 能力矩阵，“批量巡检 Adapter”会调用 `/api/adapter-health`，单个 Provider“测试连接”会调用 `/api/provider-health`，软件矩阵“检查”会调用 `/api/software-health`，运行事件流会优先使用 `/api/events/stream` EventSource/SSE，失败时回退 `/api/events` 轮询增量刷新，节点详情会读取 `/api/workflow-context?workflow_id=...` 展示当前 workflow 账本摘要，并读取 `/api/node-context?node_id=...` 下钻任务、资产、Provider 请求、软件动作和 Agent session，“运行节点”会调用 `/api/nodes/run`，并可保存 Provider API Key、创建 Provider 任务、运行 Provider adapter、显示 `provider_requests` 请求账本、写入 Hermes/Agent CLI 会话、写入软件控制动作、显示 `software_actions` 审计、落地 desktop recognition request、读取桌面识别接管队列并回填成功/失败结果，并通过任务队列按钮放行、取消或重试任务。
- 本地持久化：未连接 runtime 时，运行状态保存在浏览器 `localStorage`，支持导出当前项目 JSON 和重置演示状态。

## 接入范围

- AI 图片/视频生成：Midjourney、OpenAI image-2、Nano Banana Pro、ComfyUI、Suno。
- 3DGS / 2D→3D：World Labs Marble、Spark、群核科技、SAM-3D、TripoSplat。
- 内嵌控制：Hermes endpoint、控制指令、任务队列写入、运行 trace。
- Agent CLI：项目创建、Provider 任务、Hermes 控制、Agent 会话命令模板。
- 软件控制 fallback：Unreal MCP、通用 CLI、desktop recognition request 和 human takeover。

当前版本是可运行的 Adapter 骨架：可以配置、保存 key、测试连接、创建任务、触发 runtime provider run、写入队列并导出状态；真实端到端调用需要本地凭证、gateway endpoint 或厂商账号权限。

## 运行方式

直接用浏览器打开：

```bash
open index.html
```

或启动一个静态服务器：

```bash
python3 -m http.server 4173
```

然后访问 `http://localhost:4173`。

读取导出的 runtime snapshot：

```text
http://localhost:4173/apps/web-prototype/?snapshot=/target/runtime-snapshot-smoke/runtime-snapshot.json
```

读取本地 Runtime HTTP：

```bash
cargo run -p pool-core --example serve_runtime_http -- target/runtime-http-smoke/pool-runtime.sqlite --bind=127.0.0.1:4788
```

```text
http://localhost:4173/apps/web-prototype/?runtime=local
http://localhost:4173/apps/web-prototype/?runtime_registry=runtime-registry.json
http://localhost:4173/apps/web-prototype/?runtime=local&runtime_ports=4788,4789
http://localhost:4173/apps/web-prototype/
http://localhost:4173/apps/web-prototype/?runtime=local&project=demo
http://localhost:4173/apps/web-prototype/?runtime=local&project=*
```
