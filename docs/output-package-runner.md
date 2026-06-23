# Output Package Runner

`OutputPackageRunner` 是 Pool 首版三类输出的本地交付包运行器。它不替代 Unreal、Resolve、TouchDesigner 或 MadMapper，而是把这些后段软件产出的执行意图收束为可索引、可审计、可复用的本地 manifest。

## 目标

- 视频输出：生成时间线、镜头轨、资产轨和转码目标。
- 游戏输出：生成引擎、关卡、运行视口和构建目标。
- 交互艺术输出：生成 cue graph、实时视觉源、音频源和 OSC/MIDI/DMX 等设备接口。
- 所有输出都落地为本地文件，再写入 `assets` 表；远程或软件侧 URL 只作为 provenance。

## 本地文件契约

运行器会在项目 envelope 的 `worlds/<slug>/output/deliverables/` 下写入：

- `1-video-timeline.json`
- `2-game-build.json`
- `3-interactive-cues.json`

这些文件采用 image-blaster 风格的 indexed 命名，便于后续做版本化、回放和选择性重新生成。

## Runtime 写入

一次运行会写入：

- `tasks`：创建 `output-package` provider task，并在成功后标记为 `Succeeded`。
- `assets`：把三个本地 manifest 作为 metadata asset 入库。
- `workflow_events`：记录开始和成功事件。
- `report.manifests`：返回视频、游戏、交互艺术三个轻量摘要，供 Web/SwiftUI 面板直接显示；本地 JSON 文件仍是真相源。

## Runtime HTTP

本地 runtime 暴露一个读写同路径端点：

```text
GET /api/output-packages
POST /api/output-packages
POST /api/output-packages/results
```

`GET` 会从 `RuntimeSnapshot.assets` 和本地 manifest 文件生成三类输出 catalog：

- `summary.ready_targets`：本地文件存在且已入库的输出目标数量。
- `deliverables[].status`：`ready`、`missing` 或 `indexed_missing_file`。
- `deliverables[].preview_contract`：视频时间线、游戏运行视口、交互 cue graph 的预览/交接要求。
- `deliverables[].control_routes`：Resolve/FFmpeg、Unreal、TouchDesigner/MadMapper/OSC/MIDI/DMX 等后段控制路由。

同一内容也可通过 `pool://output-packages` 或 CLI 读取：

```bash
pool-cli --project demo output-packages
pool-cli --project demo mcp pool://output-packages
```

请求示例：

```bash
curl -X POST http://127.0.0.1:4788/api/output-packages \
  -H 'Content-Type: application/json' \
  -d '{"project_slug":"demo","node_id":"outputs","title":"Runtime output package","source_assets":["worlds/demo/output/1-world.glb"],"duration_ms":12000}'
```

后段软件执行完后，可把结果回填到对应 manifest：

```bash
curl -X POST http://127.0.0.1:4788/api/output-packages/results \
  -H 'Content-Type: application/json' \
  -d '{"project_slug":"demo","node_id":"outputs","target":"game","status":"succeeded","runtime":"Unreal","adapter_id":"unreal","software_action_id":"action-unreal","message":"play-in-editor viewport verified","artifacts":["unreal://level/demo_content_burst"],"metrics":[{"label":"fps","value":"60"}]}'
```

回填会更新本地 manifest 的 `execution_result` 和 `execution_history`，并写入 `output-package-result` task 与事件流。之后 `GET /api/output-packages` / `pool://output-packages` 会在 metrics 中显示 `execution`、`runtime_result`、`adapter`、`artifacts` 和 `message`。

CLI/MCP 等价入口：

```bash
pool-cli --project demo output-result game --status succeeded --runtime Unreal --adapter-id unreal --artifact unreal://level/demo_content_burst --metric fps=60
```

Web prototype 连接 Runtime HTTP 后，“三类输出”面板会在每张 manifest 卡片上显示“标记后段完成”。该按钮会向 `POST /api/output-packages/results` 发送当前 target、local path、runtime、adapter 和 verification metadata，并在返回后把 `execution`、`runtime_result`、`adapter`、`artifacts` 和 `message` 指标合并回卡片。

字段：

- `project_slug`：项目 slug；省略时使用 runtime 默认 project。
- `node_id`：可选输出节点 id，用于把生成 task 挂回节点图。
- `output_dir`：输出目录；省略时写入 runtime DB 同目录下的 `worlds/<slug>/output`。
- `title`：输出任务标题。
- `source_assets`：已经生成或组装好的本地资产路径。
- `duration_ms`：视频时间线和 cue 默认时长。

## Smoke

```bash
cargo run -p pool-core --example run_output_package
```

默认输出：

```text
target/output-package-runner/pool-runtime.sqlite
target/output-package-runner/worlds/demo/output/deliverables/1-video-timeline.json
target/output-package-runner/worlds/demo/output/deliverables/2-game-build.json
target/output-package-runner/worlds/demo/output/deliverables/3-interactive-cues.json
```

也可以指定输出目录：

```bash
cargo run -p pool-core --example run_output_package -- target/output-package-verify
cargo run -p pool-core --example run_prd_readiness_smoke -- target/prd-readiness-runner
```

`run_prd_readiness_smoke` 会先运行本地内容爆发闭环，再对 video、game、interactive_art 三个 manifest 写入后段执行结果，最后打印 PRD readiness requirement 状态。它证明本地 runtime 账本、Unreal mock 组装、输出 manifest 和结果回填链路可审计；真实 Resolve/Unreal/TouchDesigner/MadMapper 进程仍需要各自 adapter/plugin 返回生产证据。
