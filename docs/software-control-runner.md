# Software Control Runner

## 目标

`SoftwareActionRunner` 是 Pool 外部软件控制的运行闭环。它把 Unreal、Blender、DaVinci Resolve、TouchDesigner、MadMapper、Unity、Nuke、Hermes 等控制动作从“注册表配置”推进为可审计的执行单元。

## 执行顺序

1. 接收 `RuntimeTask`、`SoftwareControlAction` 和具体 `SoftwareAdapter`。
2. 校验 action 的 `adapter_id` 必须匹配 adapter。
3. 如果 action 或 task 需要人工确认，任务进入 `waiting_approval`，动作写入 `software_actions`，并写入 warn event；`/api/tasks/approve` 会读取最近的 `software_actions.command_json`，清除确认标记，并用同一 task 恢复执行原动作；`/api/tasks/retry` 会对失败、可重试或已取消任务按同一账本重跑。
4. 非确认动作进入 `running`。
5. 调用 adapter `health`。
6. 调用 adapter `execute`。
7. 将 command payload 和 verification result 写入 `software_actions`。
8. 将任务状态更新为 `succeeded` 或 `failed`。
9. 如果 payload 声明 `pool_output_result` 且动作成功，自动调用 `OutputPackageRunner::record_result`，把执行结果写回视频、游戏或交互艺术 manifest。
10. 写入 `workflow_events`，供运行中心和节点图读取。

## 控制优先级

所有软件控制动作继续遵守固定优先级：

```text
API/MCP > Skills/CLI > Desktop Recognition > Human Takeover
```

当前 `UnrealMcpAdapter`、`HermesMcpAdapter` 与通用 `GenericSoftwareApiAdapter` 已验证第一优先级 `ApiMcp` 的 HTTP MCP/gateway 闭环；通用路径可用 `pool-cli software-api-bridge-worker <adapter-id>` 启动本地 `/health` + `/mcp` dry-run/forwarder。Unreal 未配置 endpoint 时，Runtime HTTP 会回退到 `MockUnrealAdapter`，用于本地 smoke 和 UI 验证。Hermes 未配置 endpoint 时会进入 human takeover 队列；其他软件 adapter 未配置 endpoint/command/desktop controller 时也会进入接管，避免伪装成已执行。

Unreal、Hermes 和通用软件 API bridge worker 都支持 `--once` 自检，用同一套 wrapper 校验路径执行 `GET /health` 与 dry-run `POST /mcp`，并写出本地 request/response 审计文件：

```bash
cargo run -p pool-cli -- unreal-mcp-bridge-worker --once --output-root worlds/demo/output
cargo run -p pool-cli -- hermes-mcp-bridge-worker --once --output-root worlds/demo/output
cargo run -p pool-cli -- software-api-bridge-worker resolve --once --output-root worlds/demo/output
```

`/api/software-contracts` 和 `pool://software-contracts/<adapter-id>` 会为每个外部软件 adapter 暴露同一份 `conformance_runbook`。它把真实接入验收固定为六步：

1. `local_bridge_baseline`：本地 bridge worker `--once` 自检，写 request/response 审计文件。
2. `real_upstream_bridge`：同一 wrapper 加 `--upstream <real-plugin-or-gateway-url>` 转发到真实插件、MCP 服务、SDK worker 或软件 gateway。
3. `software_health`：通过 `/api/software-health` 证明 Runtime 能访问该 endpoint。
4. `software_action_smoke`：通过 `/api/software-actions` 写入最小动作，且结果必须包含本地 artifact path。
5. `production_matrix`：用真实 endpoint、artifact env 和 non-placeholder attestation 生成 software production evidence bundle。
6. `validate_and_import`：先 `validate-production-evidence`，再在模板 id、本地文件和密钥检查通过后 `import-production-evidence`。

Web 软件矩阵会直接展示这些 phase，供 Hermes/Agent 或操作员按 adapter 执行。Unreal/Hermes 使用专用 bridge worker；Blender、Resolve、Unity、TouchDesigner、MadMapper、Nuke、动捕数据库和剪辑软件使用 `pool-cli software-api-bridge-worker <adapter-id>`。

`POST /api/software-conformance-packages`、`pool-cli software-conformance-package <adapter-id>` 与 MCP tool `pool_software_conformance_package` 会把单个 adapter 的合同和 runbook 写为本地交接包：request、contract、runbook、preflight、runner script 和 manifest。runner 支持 `--preflight`、`local` 和 `run` 三种模式，便于 5 人团队把 Resolve、Blender、Unreal、TouchDesigner 等真实软件接入任务分配给具体操作者或 Agent。

`RuntimeHttpServer` 已提供 `POST /api/software-actions`，用于从 Web/SwiftUI、Hermes 或 Agent CLI 写入软件控制动作。当前 `unreal` 会优先走 `UnrealMcpAdapter`；`payload_json.endpoint`、`payload_json.mcp_endpoint` 或 `POOL_UNREAL_MCP_ENDPOINT` 可配置 endpoint。没有 endpoint 时会走 `MockUnrealAdapter`。`hermes` 会优先走 `HermesMcpAdapter`；`payload_json.endpoint`、`payload_json.hermes_endpoint`、`payload_json.mcp_endpoint`、`POOL_HERMES_MCP_ENDPOINT` 或 `POOL_HERMES_ENDPOINT` 可配置 endpoint。其他软件 adapter 在 `priority:"ApiMcp"` 且 payload 带 `endpoint`、`mcp_endpoint`、`api_endpoint`、`software_endpoint` 或 `control_endpoint` 时，会走通用 API/MCP adapter 并调用默认 `/health` 与 `/mcp` 合同；`action_kind:"ExecuteCli"` 且 adapter 存在于软件矩阵时，会走 `CommandSoftwareAdapter` 受控执行。仍无明确执行器的 adapter 会先写入 `software_actions` 并进入 human takeover 队列。

软件动作 payload 可选声明 `pool_output_result`，用于把后段软件执行结果直接回填到三类输出 manifest。该字段支持 `target`、`local_path`、`status`、`runtime`、`adapter_id`、`message`、`artifacts`、`metrics` 和 `verification`；动作成功后 runner 会自动写入 `execution_result` / `execution_history`，并在 report 中返回 `output_result`。如果回填失败，软件动作审计仍保留，report 会返回 `output_result_error` 并写入 warn event。

软件动作还可以通过 Runtime HTTP、CLI 或 MCP 传入 `evidence_json`。Runtime 会把它合并进 `payload_json.evidence`，最终进入 `software_actions.command`，供 `pool://prd-readiness` 计算软件控制证据矩阵。`pool-cli production-evidence-software-matrix` 可一次性验证 Unreal、Blender、ComfyUI、Resolve、Unity、TouchDesigner、MadMapper、Nuke、动捕数据库、剪辑软件和 Hermes 的本地控制 profile；只有真实软件插件/API/CLI 侧执行时，才应使用 `--production-software` 标记生产证据。生产模式不会使用默认 echo/mock 冒充证据：Unreal 需要 `POOL_UNREAL_MCP_ENDPOINT`，Hermes 需要 `POOL_HERMES_MCP_ENDPOINT` / `POOL_HERMES_ENDPOINT` 或显式 Hermes command，其余软件需要 `POOL_SOFTWARE_<ADAPTER>_ENDPOINT`/`POOL_<ADAPTER>_ENDPOINT` 或 `POOL_SOFTWARE_<ADAPTER>_COMMAND`/`POOL_<ADAPTER>_COMMAND`；所有 production software item 还需要 `POOL_SOFTWARE_PRODUCTION_ATTESTATION` 或 per-adapter `POOL_SOFTWARE_<ADAPTER>_PRODUCTION_ATTESTATION` / `POOL_<ADAPTER>_PRODUCTION_ATTESTATION`。

```bash
pool-cli --project demo production-evidence-software-matrix target/software-evidence-matrix --no-env
pool-cli --project demo production-evidence-software-matrix target/software-evidence-matrix --production-software
pool-cli --project demo production-evidence-software-matrix target/software-evidence-matrix --production-software --evidence-bundle=target/software-evidence-matrix/software-production-evidence-bundle.json
```

生产命令环境变量示例：

```bash
export POOL_UNREAL_MCP_ENDPOINT=http://127.0.0.1:8790
export POOL_SOFTWARE_PRODUCTION_ATTESTATION=real-software-operator-run-001
export POOL_UNREAL_ARTIFACTS=worlds/demo/output/production/unreal/1-level.umap
export POOL_BLENDER_COMMAND="/Applications/Blender.app/Contents/MacOS/Blender --background --python scripts/pool_blender_evidence.py"
export POOL_BLENDER_ARTIFACTS=worlds/demo/output/production/blender/1-cleanup.blend
export POOL_DAVINCI_RESOLVE_COMMAND="/usr/local/bin/pool-resolve-render --project demo"
export POOL_DAVINCI_RESOLVE_ARTIFACTS=worlds/demo/output/production/resolve/1-master.mov
export POOL_TOUCHDESIGNER_COMMAND="/usr/local/bin/pool-touchdesigner-cue --project demo"
export POOL_TOUCHDESIGNER_ARTIFACTS=worlds/demo/output/production/touchdesigner/1-performance.toe
export POOL_HERMES_MCP_ENDPOINT=http://127.0.0.1:8792
export POOL_HERMES_ARTIFACTS=worlds/demo/output/production/hermes/1-session.json
```

`POOL_SOFTWARE_<ADAPTER>_ARTIFACTS` 或 `POOL_<ADAPTER>_ARTIFACTS` 可用逗号列出真实软件输出的本地文件，进入 `software_actions[]` bundle 的 artifact 预检。加 `--evidence-bundle=<path>` 时，runner 只会把成功且显式配置过真实 endpoint/command、production attestation 与本地 artifact env 的结果整理成 `software_actions[]` 生产证据 bundle，并保留 `production_attestation`、`verification_json`、runtime report、task 和软件 action snapshot。通用 endpoint 可指向转发到真实插件/gateway 的 `pool-cli software-api-bridge-worker <adapter-id>`；`pool://production-evidence-tasks` 和 `pool://production-evidence-item-template/<task-id>` 会为 Blender、ComfyUI、Resolve、Unity、Nuke、动捕数据库和剪辑套件提供结构化 `bridge_worker` 启动模板。CLI command 仍作为没有 API/MCP 的受控 fallback。该 bundle 可直接交给 `pool-cli validate-production-evidence` / `import-production-evidence`。默认本地 control profile smoke 会写空 `software_actions:[]`；生产模式中未配置真实 endpoint/command、本地 artifact env 或 production attestation 的 adapter 会失败并暴露缺口，不会把本地 echo、mock Unreal、未配置 Hermes 或 URI artifact 冒充成生产软件证据。

示例：

```json
{
  "adapter_id": "unreal",
  "action_kind": "RunViewport",
  "payload_json": {
    "level": "demo_content_burst",
    "pool_output_result": {
      "target": "game",
      "runtime": "Unreal",
      "adapter_id": "unreal",
      "message": "play-in-editor viewport verified",
      "artifacts": ["unreal://level/demo_content_burst"],
      "metrics": { "fps": 60 }
    }
  }
}
```

Unreal MCP 请求会自动补两个 Pool 字段：

- `pool_unreal_action`：Pool 侧规范化动作 profile，包含 `profile_id`、`operation`、`mcp_tool`、`stage`、`expected_artifacts` 和 `output_contract`。
- `mcp_payload`：发给 Unreal MCP/gateway 的默认工具负载。若 `payload_json` 已显式包含 `mcp_payload`，Pool 会保留用户自定义值。

Unreal 插件或本地 gateway 的实现合同已通过以下入口暴露：

- Runtime HTTP：`GET /api/unreal-mcp-bridge`
- MCP resource：`pool://unreal-mcp-bridge`
- CLI：`pool-cli unreal-mcp-bridge`
- MCP tool：`pool_unreal_mcp_bridge`
- 本地 worker 自检：`pool-cli unreal-mcp-bridge-worker --once --output-root worlds/demo/output`
- 本地 worker 常驻：`pool-cli unreal-mcp-bridge-worker --bind 127.0.0.1:8790 --output-root worlds/demo/output`
- Unreal Python 插件脚手架：`integrations/unreal/PoolMcpBridge`

该合同固定默认 `GET /health`、`POST /mcp`、`pool_unreal_action`、`mcp_payload`、tool contracts、响应字段和 artifact policy。Pool 侧桥接协议、本地前置 worker 和 Python 插件脚手架已完成；真实 Unreal Editor 项目仍需要继续补资产导入、Level 创建、灯光/相机、Blueprint/Sequencer、Play-in-Editor、渲染和 build manifest 的项目级 mapping 与证据。

本地 worker 默认 dry-run，只做 wrapper 校验和 `output/control/unreal-mcp-bridge/*-request.json` / `*-response.json` 审计落地；加 `--once` 会执行 health + dry-run action 自检并退出；加 `--upstream <url>` 后可作为真实 Unreal 插件或 gateway 的前置校验/转发代理。

已内置的 Unreal action mapping：

| `action_kind` | `mcp_tool` | stage | 默认参数来源 |
| --- | --- | --- | --- |
| `OpenProject` | `unreal.open_project` | `project_bootstrap` | `project_file` / `uproject_path` |
| `ImportAsset` | `unreal.import_asset` | `asset_ingest` | `asset_paths` / `assets` / `input_paths` |
| `CreateScene` | `unreal.create_scene` | `scene_assembly` | `level`、`assets`、`camera`、`lighting` |
| `RunViewport` | `unreal.run_viewport` | `interactive_preview` | `level`、`camera`、`play_mode` |
| `Render` | `unreal.render_sequence` | `video_output` | `sequence`、`output_dir`、`preset` |
| `ExportBuild` | `unreal.export_build` | `game_output` | `target_platform`、`output_dir`、`configuration` |

Hermes MCP 请求同样会自动补两个 Pool 字段：

- `pool_hermes_action`：Pool 侧规范化 Agent 控制 profile，包含 `profile_id`、`operation`、`mcp_tool`、`stage`、`expected_artifacts` 和 `output_contract`。
- `mcp_payload`：发给 Hermes MCP/gateway 的默认工具负载。若 `payload_json` 已显式包含 `mcp_payload`，Pool 会保留用户自定义值。

已内置的 Hermes action mapping：

| `action_kind` | `mcp_tool` | stage | 默认参数来源 |
| --- | --- | --- | --- |
| `OpenProject` | `hermes.open_project` | `project_context` | `project_slug`、`instruction` |
| `ImportAsset` / `CreateScene` | `hermes.coordinate` | `agent_orchestration` | `instruction`、`allowed_tools`、`target_adapter` |
| `RunViewport` | `hermes.run_preview` | `interactive_preview` | `instruction`、`target_action_kind` |
| `Render` / `Transcode` / `ExportBuild` | `hermes.output_control` | `output_orchestration` | `instruction`、`context` |

Hermes MCP 可直接指向真实 Hermes 服务，也可先指向本地 bridge worker：

```bash
cargo run -p pool-cli -- hermes-mcp-bridge-worker \
  --once \
  --output-root worlds/demo/output

cargo run -p pool-cli -- hermes-mcp-bridge-worker \
  --bind 127.0.0.1:8792 \
  --output-root worlds/demo/output

POOL_HERMES_MCP_ENDPOINT=http://127.0.0.1:8792 cargo run -p pool-cli -- \
  --project demo \
  run-software hermes \
  --action-kind CreateScene \
  --priority ApiMcp \
  --payload-json '{"instruction":"coordinate Unreal scene assembly","target_adapter":"unreal","target_action_kind":"CreateScene"}'
```

worker 默认 dry-run，只做 wrapper 校验和 `output/control/hermes-mcp-bridge/*-request.json` / `*-response.json` 审计落地；加 `--upstream <url>` 后可作为真实 Hermes 服务端的前置校验/转发代理。

`priority:"DesktopRecognition"` 或 `action_kind:"DesktopClick" / "DesktopHotkey"` 会走 `DesktopRecognitionAdapter`。它先生成一个可审计、可被桌面控制进程消费的请求文件；默认 controller 仍是 dry-run，显式使用 AppleScript 模式时才会执行确定性的 macOS 激活、点击、快捷键或文本输入：

```text
worlds/<project>/output/control/desktop-recognition/desktop-recognition-<id>.json
```

桌面识别请求会自动补两个 Pool 字段：

- `pool_desktop_action`：Pool 侧规范化桌面动作 profile，包含 `profile_id`、`operation`、`desktop_tool`、`stage`、`target_window`、`expected_artifacts` 和 `output_contract`。
- `desktop_payload`：供桌面控制进程消费的默认负载。若 `payload_json` 已显式包含 `desktop_payload`，Pool 会保留用户自定义值。

已内置的 desktop action mapping：

| `action_kind` | `desktop_tool` | stage | 默认参数来源 |
| --- | --- | --- | --- |
| `OpenProject` | `desktop.open_project` | `project_bootstrap` | `target_window`、`instruction` |
| `ImportAsset` | `desktop.import_asset` | `asset_ingest` | `visual_targets`、原始 payload |
| `CreateScene` | `desktop.create_scene` | `scene_assembly` | `visual_targets`、原始 payload |
| `RunViewport` | `desktop.run_preview` | `interactive_preview` | `mode`、`cue`、`visual_targets` |
| `Render` | `desktop.render_output` | `video_output` | `output_dir`、`preset`、`visual_targets` |
| `Transcode` | `desktop.transcode_output` | `delivery_output` | `output_dir`、`preset` |
| `ExportBuild` | `desktop.export_build` | `game_output` | `output_dir`、`preset` |
| `DesktopClick` | `desktop.click` | `desktop_interaction` | `click_target`、`coordinates`、`visual_targets` |
| `DesktopHotkey` | `desktop.hotkey` | `desktop_interaction` | `hotkey`、`keys` |

Desktop recognition payload 示例：

```json
{
  "adapter_id": "touchdesigner",
  "action_kind": "RunViewport",
  "priority": "DesktopRecognition",
  "payload_json": {
    "instruction": "find TouchDesigner perform mode and trigger cue 1",
    "target_window": "TouchDesigner",
    "visual_targets": ["Perform", "Cue 1", "Output"]
  }
}
```

落地后的 request JSON 会保留原始 payload，同时提供规范化字段：

```json
{
  "status": "queued_for_desktop_recognition",
  "pool_desktop_action": {
    "profile_id": "desktop-run-preview",
    "operation": "run_preview",
    "desktop_tool": "desktop.run_preview",
    "stage": "interactive_preview",
    "target_window": "TouchDesigner",
    "output_contract": "desktop-recognition-control-request"
  },
  "desktop_payload": {
    "tool": "desktop.run_preview",
    "operation": "run_preview",
    "target_window": "TouchDesigner",
    "visual_targets": ["Perform", "Cue 1", "Output"]
  }
}
```

Runtime HTTP 同时提供桌面 controller 领取、dry-run 推进与回填协议：

```bash
curl http://127.0.0.1:4788/api/desktop-recognition/requests
curl -X POST http://127.0.0.1:4788/api/desktop-recognition/run-next \
  -H 'Content-Type: application/json' \
  -d '{"controller_id":"local-vision-dry-run","status":"succeeded"}'
```

```bash
curl -X POST http://127.0.0.1:4788/api/desktop-recognition/results \
  -H 'Content-Type: application/json' \
  -d '{
    "software_action_id": "<action-id>",
    "status": "succeeded",
    "message": "TouchDesigner cue triggered",
    "screen_trace_path": "worlds/demo/output/control/desktop-recognition/trace.json",
    "artifacts": ["worlds/demo/output/control/desktop-recognition/trace.json"],
    "result": {
      "controller": "desktop-vision",
      "attempts": 1
    }
  }'
```

回填后 Pool 会更新对应 `software_actions.verification_json`，把 `desktop_recognition_status`、`controller_result` 和 `screen_trace_path` 写入账本，并同步关联 task 状态。

本地 trace/callback 证据可用独立 smoke 写入。它会生成 TouchDesigner 桌面识别任务、Pool-compatible trace JSON、`screen_trace_path` 和 `controller_result.vision_trace_path`，并让 `pool://prd-readiness` 的 `production_hardening.evidence.desktop_vision_evidence` 能看到 controller callback 与 trace 证据；该 smoke 标记 `external_visual_model:false`，不代表真实视觉模型、OCR 或屏幕采集已经接入：

```bash
cargo run -p pool-core --example run_desktop_vision_trace_smoke -- target/desktop-vision-trace-smoke
```

对应的 `pool-cli` 命令可供 Hermes、Agent CLI 或桌面 controller 直接调用：

```bash
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo run-software touchdesigner \
  --action run-viewport \
  --priority DesktopRecognition \
  --title "TouchDesigner desktop cue" \
  --payload-json '{"instruction":"find TouchDesigner perform mode and trigger cue 1","target_window":"TouchDesigner","visual_targets":["Perform","Cue 1","Output"]}'

cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo desktop-requests

cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo desktop-run-next \
  --controller-id local-vision-dry-run \
  --status succeeded \
  --screen-trace-path worlds/demo/output/control/desktop-recognition/trace.json

cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo desktop-result <software-action-id> \
  --status succeeded \
  --message "desktop controller finished" \
  --artifact worlds/demo/output/control/desktop-recognition/trace.json \
  --result-json '{"controller":"desktop-vision"}'
```

也可以用本地 dry-run controller example 通过 HTTP runtime 直接跑一次领取和回填：

```bash
cargo run -p pool-core --example run_desktop_recognition_controller -- http://127.0.0.1:4788 --project=demo --status=succeeded
```

需要真实执行确定性桌面动作时，使用同一个 controller example 的 AppleScript 模式：

```bash
cargo run -p pool-core --example run_desktop_recognition_controller -- \
  http://127.0.0.1:4788 \
  --project=demo \
  --mode=applescript \
  --osascript=/usr/bin/osascript \
  --vision-trace=worlds/demo/output/control/desktop-recognition/trace.json
```

需要由外部视觉/OCR 服务生成 Pool-compatible trace 时，使用 `vision-http` 模式。该模式会读取同一队列，把 `pool_desktop_action`、`desktop_payload`、`target_window` 和 `visual_targets` POST 到外部 endpoint，成功后把返回的 detections 规范化为本地 trace 文件并回填 `external_visual_model:true`；endpoint 调用失败、非 2xx、JSON 解析失败或 trace 写盘失败时不会形成外部视觉生产证据：

```bash
POOL_DESKTOP_VISION_API_KEY=<redacted> POOL_DESKTOP_VISION_PRODUCTION_ATTESTATION=real-vision-controller-run-001 cargo run -p pool-core --example run_desktop_recognition_controller -- \
  http://127.0.0.1:4788 \
  --project=demo \
  --mode=vision-http \
  --vision-endpoint=http://127.0.0.1:8795/vision \
  --vision-api-key-env=POOL_DESKTOP_VISION_API_KEY \
  --vision-trace-output=worlds/demo/output/control/desktop-recognition/external-vision-trace.json
```

外部 controller 生成 trace 后，用 `pool-cli production-evidence-desktop-vision --production-vision --trace <path> --external-action-id <id>` 统一回填 runtime 并写成 `desktop_vision[]` 生产证据 bundle，包含 `trace_path`、`visual_model:"external"`、`production_attestation`、`evidence_json.external_visual_model:true` 和 controller callback verification，可直接交给 `pool-cli validate-production-evidence` / `import-production-evidence`。dry-run、AppleScript、失败结果、缺少 attestation 或缺少本地 trace 文件的外部视觉结果会写空 `desktop_vision:[]`，不冒充真实外部视觉模型证据。

AppleScript 模式支持：

- `target_window`：激活目标应用。
- `coordinates` / `{x,y}`：通过 System Events 点击明确坐标。
- `visual_targets` + `--vision-trace`：读取外部视觉/OCR trace，把目标 label 解析成点击坐标。
- `hotkey` / `keys`：发送快捷键，例如 `cmd+shift+p`。
- `text` / `type_text` / `input_text`：输入文本。

Pool-compatible trace JSON 示例：

```json
{
  "detections": [
    {
      "text": "Cue 1",
      "bounds": { "x": 300, "y": 200, "width": 100, "height": 60 }
    },
    {
      "label": "Render",
      "center": { "x": 640, "y": 720 }
    }
  ]
}
```

trace 根字段可为 `detections`、`targets`、`items`、`elements`、`ocr` 或直接数组；label 可用 `label`、`text`、`name`、`id`；位置可用 `center`、`point`、`position`、`coordinates`、`bounds`、`bbox`、`box`。匹配规则是忽略大小写和空格后先精确匹配，再做包含匹配。

边界：`desktop-run-next` 和 `POST /api/desktop-recognition/run-next` 仍只做协议级 dry-run；AppleScript controller 可以执行明确坐标、快捷键和文本输入，也可以消费外部视觉/OCR trace，但不负责屏幕采集和识别模型本身。`vision-http` controller 负责调用外部视觉/OCR 服务、写本地 trace 和回填 controller result，但不执行点击或热键。若 request 只有 `visual_targets` 且没有 `--vision-trace` 或 trace 命中结果，AppleScript 模式会回填 `failed`。PRD readiness 只有在真实外部视觉 controller 成功回填 `external_visual_model:true`、带 `production_attestation` 且本地 trace/artifacts 可审计时，才会把外部视觉模型证据标记为 ready。

Unreal MCP payload 示例：

```json
{
  "adapter_id": "unreal",
  "action_kind": "CreateScene",
  "priority": "ApiMcp",
  "payload_json": {
    "endpoint": "http://127.0.0.1:8787",
    "level": "demo_content_burst",
    "assets": ["worlds/demo/output/1-world.glb"],
    "camera": "hero_orbit"
  }
}
```

Unreal MCP adapter 默认调用：

```text
GET /health
POST /mcp
```

发送到 Unreal MCP/gateway 的 body 会保留原始 payload，同时增加规范化字段：

```json
{
  "adapter_id": "unreal",
  "action_kind": "CreateScene",
  "payload": {
    "level": "demo_content_burst",
    "assets": ["worlds/demo/output/1-world.glb"]
  },
  "pool_unreal_action": {
    "profile_id": "unreal-create-scene",
    "operation": "create_scene",
    "mcp_tool": "unreal.create_scene",
    "stage": "scene_assembly",
    "output_contract": "unreal-mcp-action-result"
  },
  "mcp_payload": {
    "tool": "unreal.create_scene",
    "operation": "create_scene",
    "arguments": {
      "level": "demo_content_burst",
      "asset_paths": ["worlds/demo/output/1-world.glb"],
      "camera": "hero_orbit",
      "lighting": "cinematic_day"
    }
  }
}
```

可通过环境变量覆盖：

```bash
export POOL_UNREAL_MCP_ENDPOINT=http://127.0.0.1:8787
export POOL_UNREAL_MCP_HEALTH_PATH=/health
export POOL_UNREAL_MCP_ACTION_PATH=/mcp
export POOL_UNREAL_MCP_TOKEN=...
```

Hermes MCP 环境变量：

```bash
export POOL_HERMES_MCP_ENDPOINT=http://127.0.0.1:8787
export POOL_HERMES_MCP_HEALTH_PATH=/health
export POOL_HERMES_MCP_ACTION_PATH=/mcp
export POOL_HERMES_MCP_TOKEN=...
```

CLI/脚本控制 payload 需要显式命令和 allowlist：

```json
{
  "adapter_id": "blender",
  "action_kind": "ExecuteCli",
  "priority": "SkillsCli",
  "payload_json": {
    "command": "/bin/echo blender-runtime-ok",
    "allowed_commands": ["/bin/echo", "echo"],
    "timeout_ms": 2000,
    "max_output_bytes": 1024,
    "artifacts": ["blender://script/smoke"]
  }
}
```

对应的 `pool-cli` smoke：

```bash
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo software-health blender \
  --priority SkillsCli

cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo run-software blender \
  --action execute-cli \
  --priority SkillsCli \
  --title "Blender CLI smoke" \
  --payload-json '{"command":"/bin/echo blender-runtime-ok","allowed_commands":["/bin/echo","echo"],"timeout_ms":2000,"max_output_bytes":1024}'
```

## Smoke

```bash
cargo run -p pool-core --example run_mock_unreal_action
cargo run -p pool-core --example run_unreal_mcp_action
POOL_UNREAL_MCP_ENDPOINT=http://127.0.0.1:8787 cargo run -p pool-core --example run_unreal_mcp_action -- http://127.0.0.1:8787 target/unreal-mcp-runner
cargo run -p pool-core --example stage_desktop_recognition_action
cargo run -p pool-core --example run_desktop_recognition_controller -- http://127.0.0.1:4788 --project=demo --status=succeeded
```

输出包括：

- SQLite path
- task status
- action id
- result message
- tasks / software_actions / workflow_events 计数

## 当前边界

- 已实现 runner、SQLite action 审计、事件写入、人工确认阻断、确认后同 task 恢复执行、失败/取消后同 task 重试执行、Unreal MCP HTTP 请求、Unreal action profile/mcp payload mapping、Hermes MCP HTTP 请求、Hermes action profile/mcp payload mapping、Hermes bridge worker、mock Unreal fallback、`pool_output_result` 到三类输出 manifest 的自动回填、desktop recognition action profile/desktop payload mapping、desktop recognition request staging、controller 队列读取、runtime dry-run 推进、结果回填、dry-run controller example、desktop vision trace/callback evidence smoke、Runtime HTTP software action 写入和 `pool-cli` 软件控制/桌面识别接管入口。
- Unreal MCP 当前完成 Pool 侧动作 schema、本地 bridge worker 和 Unreal Python 插件脚手架；后续需要在真实 Unreal Editor 项目中验证这些 `mcp_tool`，并继续补蓝图、关卡、Sequencer 的真实执行 mapping。
- Hermes HTTP 会话执行已由 `AgentSessionRunner` 覆盖；Hermes 软件动作 MCP 已由 `HermesMcpAdapter` 和本地 `hermes-mcp-bridge-worker` 覆盖，后续需要真实 Hermes 服务端实现这些 `mcp_tool` 并回填 session/transcript。
- Skills/CLI 已有通用 `CommandSoftwareAdapter` 骨架；desktop recognition 当前完成 schema 化请求落地和 controller 回填协议，尚未实现真正的屏幕识别、点击、热键执行器。
