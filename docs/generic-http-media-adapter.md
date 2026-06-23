# Generic HTTP Media Provider Adapter

## 目标

`GenericHttpMediaProvider` 是 Pool 给 Midjourney、Nano Banana Pro、Suno 等媒体生成服务准备的通用接入层。它先把这些服务统一映射成可执行的 `ProviderAdapter`，并在通用 gateway 契约上增加 Pool media profile mapping；后续真实厂商 API 可由本地 gateway、Hermes adapter 或官方 SDK adapter 继续翻译。

它覆盖：

- `health`
- `submit`
- `poll`
- `download`
- `verify`
- 本地 media output 落地
- `ProviderTaskRunner` 资产入库

## Gateway Contract

默认 submit endpoint：

```text
POST /v1/media/jobs
```

默认 poll endpoint：

```text
GET /v1/media/jobs/{job_id}
```

当前已内置 profile：

| Provider | profile | modality | 默认 slug | 默认扩展名 | 典型用途 |
| --- | --- | --- | --- | --- | --- |
| `midjourney` | `midjourney` | image | `midjourney` | `png` | 概念图 / key art |
| `nano-banana-pro` | `nano-banana-pro` | image | `nano` | `png` | 参考图 / 图像生成与编辑 |
| `suno` | `suno` | audio | `suno-cue` | `mp3` | 视频配乐 / 交互艺术 cue |

默认路径仍保持 `/v1/media/jobs`，便于一个本地 media gateway 同时承接多个 provider。需要拆分真实服务路由时可用 `POOL_<PROVIDER_ID>_SUBMIT_PATH` 和 `POOL_<PROVIDER_ID>_POLL_PATH` 覆盖。

submit response 可返回：

```json
{
  "job_id": "media_123",
  "status": "queued"
}
```

poll/result response 可返回：

```json
{
  "status": "completed",
  "outputs": [
    { "name": "hero.png", "url": "https://cdn.example/hero.png" },
    { "name": "cue.mp3", "url": "https://cdn.example/cue.mp3" }
  ]
}
```

同步 gateway 也可以直接在 submit response 里返回 `outputs`，Pool 会立即下载成本地文件。

支持的输出字段包括：

- `outputs[]`
- `data.outputs[]`
- `assets[]`
- `files[]`
- `images[]`
- `audios[]`
- `output_url`
- `image_url`
- `audio_url`
- `video_url`
- `file_url`

每个 output item 支持 `url`、`download_url`、`image_url`、`audio_url`、`video_url`、`file_url`、`b64_json`、`base64`、`data`、`local_path`。

## 环境变量

通用 gateway：

```bash
export POOL_MEDIA_GATEWAY_ENDPOINT=http://127.0.0.1:8787
export POOL_MEDIA_GATEWAY_API_KEY=...
```

生产证据输出：

```bash
export POOL_PROVIDER_PRODUCTION_ATTESTATION=real-media-worker-run-001
export POOL_PROVIDER_EVIDENCE_BUNDLE=worlds/demo/output/control/production-evidence/media-provider.bundle.json
```

`POOL_PROVIDER_PRODUCTION_ATTESTATION` 必须指向真实厂商 worker/SDK/gateway run；模板、mock、dry-run 或占位字符串会在 production evidence validate/import 阶段被拒绝。设置 `POOL_PROVIDER_EVIDENCE_BUNDLE` 或 `POOL_PRODUCTION_EVIDENCE_BUNDLE` 后，成功的 `generic_media_smoke` 会把本地 artifact、metadata path、external job id 和 attestation 写成 `providers[]` bundle；只设置 attestation 时默认写到输出目录下的 `provider-production-evidence-bundle.json`。

按 provider 覆盖：

```bash
export POOL_MIDJOURNEY_ENDPOINT=http://127.0.0.1:8787
export POOL_MIDJOURNEY_API_KEY=...
export POOL_NANO_BANANA_PRO_ENDPOINT=http://127.0.0.1:8787
export POOL_NANO_BANANA_PRO_API_KEY=...
export POOL_SUNO_ENDPOINT=http://127.0.0.1:8787
export POOL_SUNO_API_KEY=...
```

可选路径覆盖：

```bash
export POOL_NANO_BANANA_PRO_SUBMIT_PATH=/v1/media/jobs
export POOL_NANO_BANANA_PRO_POLL_PATH=/v1/media/jobs/{job_id}
export POOL_NANO_BANANA_PRO_OUTPUT_SLUG=nano
export POOL_NANO_BANANA_PRO_ASSET_INDEX=1
```

## 请求 JSON

最小请求可以是纯文本 prompt：

```text
generate a production concept image for an interactive art stage
```

也可以是 JSON：

```json
{
  "prompt": "generate a production concept image for an interactive art stage",
  "input_paths": ["worlds/demo/source/ref.png"],
  "output_slug": "nano",
  "output_extension": "png",
  "aspect_ratio": "16:9",
  "style": "cinematic"
}
```

Suno 类音频任务：

```json
{
  "prompt": "generate a short electronic cue for interactive art",
  "output_slug": "suno-cue",
  "output_extension": "mp3",
  "duration": 12
}
```

额外字段会透传到 gateway request body。

Pool 会补充两个 gateway 字段：

- `pool_media_profile`：Pool 侧规范化 profile，包含 provider、modality、pipeline、task type、media role、expected output kinds 和本地输出契约。
- `provider_payload`：按 provider profile 生成的 gateway 默认负载。若请求 JSON 已显式传入 `provider_payload`，Pool 会保留用户自定义值。
- `local_input_manifest`：由 `input_paths` 生成的本地输入清单，包含原始路径、可用时的绝对路径、文件名、扩展名、MIME、字节数和 `exists` 状态；不会写入文件内容。`http://`、`https://`、`s3://`、`gs://` 和 `data:` 输入会被拒绝，避免远程 URL 成为加载真相源。

例如 `nano-banana-pro` 会生成类似结构：

```json
{
  "provider_id": "nano-banana-pro",
  "pool_media_profile": {
    "profile_id": "nano-banana-pro",
    "modality": "image",
    "pipeline": "reference_guided_image_generation",
    "task_type": "nano_banana_pro_image",
    "media_role": "reference_plate",
    "output_contract": "local-media-files"
  },
  "local_input_manifest": [
    {
      "path": "worlds/demo/source/ref.png",
      "absolute_path": "/abs/worlds/demo/source/ref.png",
      "file_name": "ref.png",
      "extension": "png",
      "mime_type": "image/png",
      "bytes": 12345,
      "exists": true
    }
  ],
  "provider_payload": {
    "service": "nano-banana-pro",
    "mode": "reference_guided_image",
    "handoff": {
      "media_role": "reference_plate",
      "next_stage": "comfyui_or_3dgs"
    }
  }
}
```

## Runtime HTTP

`POST /api/provider-runs` 已支持：

- `midjourney`
- `nano-banana-pro`
- `nanobananapro`
- `suno`

媒体 Provider 可以纳入矩阵化证据 smoke：

```bash
pool-cli --project demo production-evidence-provider-matrix target/provider-evidence-matrix --no-env
POOL_PROVIDER_PRODUCTION_ATTESTATION=real-vendor-sdk-worker-2026-06-17 pool-cli --project demo production-evidence-provider-matrix target/provider-evidence-matrix --production-upstream --media-endpoint=http://127.0.0.1:8788 --3dgs-endpoint=http://127.0.0.1:8788 --provider-endpoint midjourney=http://127.0.0.1:9701 --provider-endpoint tripo-splat=http://127.0.0.1:9712 --provider-api-key-env midjourney=POOL_PROVIDER_API_KEY_MIDJOURNEY --provider-api-key-env tripo-splat=POOL_PROVIDER_API_KEY_TRIPO_SPLAT --openai-api-key-env OPENAI_API_KEY
POOL_PROVIDER_PRODUCTION_ATTESTATION=real-vendor-sdk-worker-2026-06-17 pool-cli --project demo production-evidence-provider-matrix target/provider-evidence-matrix --production-upstream --media-endpoint=http://127.0.0.1:8788 --3dgs-endpoint=http://127.0.0.1:8788 --provider-endpoint midjourney=http://127.0.0.1:9701 --provider-endpoint tripo-splat=http://127.0.0.1:9712 --provider-api-key-env midjourney=POOL_PROVIDER_API_KEY_MIDJOURNEY --provider-api-key-env tripo-splat=POOL_PROVIDER_API_KEY_TRIPO_SPLAT --openai-api-key-env OPENAI_API_KEY --evidence-bundle=target/provider-evidence-matrix/provider-production-evidence-bundle.json
```

这个 runner 会把 `evidence_json` 写入 `provider_requests.request_json.evidence`。`--no-env` 只验证矩阵调度和 bundle 写出，不接触本机 endpoint/key；配置真实 gateway 后，`evidence_mode:"configured_gateway"` 证明接入了指定 endpoint。只有显式 `--production-upstream`、提供非占位 `POOL_PROVIDER_PRODUCTION_ATTESTATION`，并且 endpoint 背后是真实厂商 worker/SDK 服务时，PRD readiness 才会把它计为真实上游证据。加 `--evidence-bundle=<path>` 时，成功的真实上游结果还会被整理成可校验/导入的 `providers[]` 生产证据 bundle，并附带每个 Provider 的本地 metadata 文件。

示例：

```bash
curl -X POST http://127.0.0.1:4788/api/provider-runs \
  -H 'Content-Type: application/json' \
  -d '{"provider_id":"nano-banana-pro","endpoint":"http://127.0.0.1:8787","task_title":"Nano run","prompt":"{\"prompt\":\"generate hero plate\",\"output_slug\":\"nano\",\"output_extension\":\"png\"}","output_dir":"worlds/demo/output","requires_approval":false}'
```

如果没有配置 endpoint，Runtime HTTP 会返回 `provider_not_configured`，避免误触发未知远端服务。

## 本地优先契约

Provider URL 只作为 provenance。Pool 会把 URL 或 base64 输出保存成本地文件，例如：

```text
1-nano-hero.png
.1-nano__nano-banana-pro-request.json
```

随后 `RuntimeRepository::index_local_outputs` 会把本地路径写入 `assets` 表，前端和 MCP resource 都读取本地路径。

真实 SDK wrapper 或 `provider-gateway-worker` 应优先读取 submit body 里的 `local_input_manifest`，而不是自行把远程输入 URL 当作参考素材。`provider-gateway-worker` 会把该 manifest 合并到 `upstream.request_body.inputs.local_input_manifest`，供上游 worker 直接使用。

metadata 只保存 response summary，不保存大体积 base64 payload。

## Smoke

只检查配置：

```bash
cargo run -p pool-core --example generic_media_smoke
```

本地 mock gateway 自检：

```bash
cargo run -p pool-core --example provider_gateway_mock_server -- once
```

启动本地 mock gateway：

```bash
cargo run -p pool-core --example provider_gateway_mock_server -- --bind=127.0.0.1:8787
```

提交 gateway 任务：

```bash
POOL_MEDIA_GATEWAY_ENDPOINT=http://127.0.0.1:8787 \
cargo run -p pool-core --example generic_media_smoke -- nano-banana-pro request.json target/generic-media-smoke
```

提交并写出可导入的 production evidence bundle：

```bash
POOL_MEDIA_GATEWAY_ENDPOINT=http://127.0.0.1:8787 \
POOL_PROVIDER_PRODUCTION_ATTESTATION=real-media-worker-run-001 \
POOL_PROVIDER_EVIDENCE_BUNDLE=target/generic-media-smoke/provider-production-evidence-bundle.json \
cargo run -p pool-core --example generic_media_smoke -- nano-banana-pro request.json target/generic-media-smoke

cargo run -p pool-cli -- --db target/generic-media-smoke/validate.sqlite --project generic-media-demo \
  validate-production-evidence target/generic-media-smoke/provider-production-evidence-bundle.json
```

## 当前边界

- 已实现通用 HTTP gateway、Midjourney/Nano Banana Pro/Suno media profile mapping、状态映射、URL/base64 输出落地、metadata 摘要、Runtime HTTP 调度、单元测试、smoke example、本地 mock gateway contract server，以及成功 gateway run 后的 `providers[]` production evidence bundle 输出。
- 尚未实现 Midjourney、Nano Banana Pro、Suno 的官方 SDK 或厂商私有协议。
- 需要真实服务时，应把厂商 API 映射到本文契约；`provider_gateway_mock_server` 可验证 Pool contract，`provider_gateway_template` 可生成真实 SDK/HTTP worker 的上游翻译模板，`provider_gateway_worker` 可作为本地 HTTP forwarder 接真实或 mock upstream。
