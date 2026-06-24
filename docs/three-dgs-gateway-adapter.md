# 3DGS Gateway Provider Adapter

## 目标

`ThreeDgsGatewayProvider` 是 Pool 的通用 2D/3DGS ProviderAdapter。它用于把 World Labs Marble、TripoSplat、SAM-3D、Spark、群核科技等 3DGS/3D 生成服务先统一映射到 Pool runtime，而不是在 core 中硬编码每个厂商的私有 API。

当前实现的是 Pool 到本地 gateway 的 profile mapping，不是厂商官方 SDK 协议。真实厂商 API 可由本地 gateway 进程、Hermes adapter 或后续专用 SDK adapter 继续翻译。

核心契约：

- `health`
- `submit`
- `poll`
- `download`
- `verify`
- `ProviderTaskRunner`
- image-blaster indexed local outputs

## Gateway Contract

通用与 `worldlabs-marble` 默认 submit endpoint：

```text
POST /v1/3dgs/jobs
```

通用与 `worldlabs-marble` 默认 poll endpoint：

```text
GET /v1/3dgs/jobs/{job_id}
```

其他 3DGS provider 会按 profile 使用不同默认 path，仍可被环境变量覆盖：

| Provider | 默认 submit | 默认 poll | 默认 slug | 资产范围 |
| --- | --- | --- | --- | --- |
| `worldlabs-marble` | `/v1/3dgs/jobs` | `/v1/3dgs/jobs/{job_id}` | `world` | scene |
| `tripo-splat` | `/v1/3dgs/triposplat/jobs` | `/v1/3dgs/triposplat/jobs/{job_id}` | `object` | object |
| `sam-3d` | `/v1/3dgs/sam-3d/jobs` | `/v1/3dgs/sam-3d/jobs/{job_id}` | `object` | object |
| `spark-3dgs` | `/v1/3dgs/spark/jobs` | `/v1/3dgs/spark/jobs/{job_id}` | `scene` | scene |
| `qunhe-3d` | `/v1/3dgs/qunhe/jobs` | `/v1/3dgs/qunhe/jobs/{job_id}` | `scene` | scene |

submit response 可返回：

```json
{
  "job_id": "job_123"
}
```

poll/result response 可返回：

```json
{
  "status": "completed",
  "outputs": [
    { "name": "world.json", "url": "https://cdn.example/world.json" },
    { "name": "world.glb", "url": "https://cdn.example/world.glb" },
    { "name": "world-full_res.spz", "url": "https://cdn.example/world-full_res.spz" }
  ]
}
```

状态支持：

- `queued / pending`
- `running / processing`
- `completed / succeeded / success`
- `failed / error / cancelled`

## 环境变量

通用 gateway：

```bash
export POOL_3DGS_GATEWAY_ENDPOINT=http://127.0.0.1:8787
export POOL_3DGS_GATEWAY_API_KEY=...
```

生产证据输出：

```bash
export POOL_PROVIDER_PRODUCTION_ATTESTATION=real-3dgs-worker-run-001
export POOL_PROVIDER_EVIDENCE_BUNDLE=worlds/demo/output/control/production-evidence/3dgs-provider.bundle.json
```

`POOL_PROVIDER_PRODUCTION_ATTESTATION` 必须指向真实 3DGS 厂商 worker/SDK/gateway run；模板、mock、dry-run 或占位字符串会在 production evidence validate/import 阶段被拒绝。设置 `POOL_PROVIDER_EVIDENCE_BUNDLE` 或 `POOL_PRODUCTION_EVIDENCE_BUNDLE` 后，成功的 `three_dgs_gateway_smoke` 会把本地 indexed assets、metadata path、external job id 和 attestation 写成 `providers[]` bundle；只设置 attestation 时默认写到输出目录下的 `provider-production-evidence-bundle.json`。

按 provider 覆盖，例如 `worldlabs-marble`：

```bash
export POOL_WORLDLABS_MARBLE_ENDPOINT=http://127.0.0.1:8787
export POOL_WORLDLABS_MARBLE_API_KEY=...
export POOL_WORLDLABS_MARBLE_SUBMIT_PATH=/v1/3dgs/jobs
export POOL_WORLDLABS_MARBLE_POLL_PATH=/v1/3dgs/jobs/{job_id}
export POOL_WORLDLABS_MARBLE_OUTPUT_SLUG=world
export POOL_WORLDLABS_MARBLE_ASSET_INDEX=1
```

## 请求 JSON

最小请求可以是纯文本 prompt：

```text
convert this concept plate into a navigable 3DGS scene
```

也可以是 JSON：

```json
{
  "prompt": "convert this concept plate into a navigable 3DGS scene",
  "input_paths": ["worlds/demo/source/plate.png"],
  "output_slug": "world",
  "expected_outputs": [
    "worlds/demo/output/1-world.json",
    "worlds/demo/output/1-world.glb",
    "worlds/demo/output/1-world-full_res.spz"
  ],
  "quality": "high"
}
```

额外字段会透传到 gateway request body，便于对接不同 3DGS 服务。

Pool 会补充两个 gateway 字段：

- `pool_gateway_profile`：Pool 侧规范化 profile，包含 provider、pipeline、task type、asset scope、expected output kinds 和本地 indexed output contract。
- `provider_payload`：按 provider profile 生成的 gateway 默认负载。若请求 JSON 已显式传入 `provider_payload`，Pool 会保留用户自定义值。
- `local_input_manifest`：由 `input_paths` 生成的本地输入清单，包含原始路径、可用时的绝对路径、文件名、扩展名、MIME、字节数和 `exists` 状态；不会写入图片、视频或 3D 文件内容。远程 URL 与 data URL 会被拒绝，保持 image-blaster 风格的本地资产优先原则。

例如 `tripo-splat` 会生成类似结构：

```json
{
  "provider_id": "tripo-splat",
  "pool_gateway_profile": {
    "profile_id": "triposplat",
    "pipeline": "image_to_object_splat",
    "task_type": "tripo_splat_reconstruction",
    "asset_scope": "object",
    "output_contract": "image-blaster-indexed-files"
  },
  "local_input_manifest": [
    {
      "path": "worlds/demo/source/plate.png",
      "absolute_path": "/abs/worlds/demo/source/plate.png",
      "file_name": "plate.png",
      "extension": "png",
      "mime_type": "image/png",
      "bytes": 12345,
      "exists": true
    }
  ],
  "provider_payload": {
    "service": "tripo-splat",
    "mode": "image_to_splat_object",
    "handoff": {
      "preferred_formats": ["spz", "glb", "preview"],
      "scene_role": "placeable_object"
    }
  }
}
```

## 本地优先契约

远程 URL 不作为加载真相源。Gateway 返回的所有输出必须下载成本地 indexed files：

```text
1-world.json
1-world.glb
1-world-full_res.spz
.1-world__worldlabs-marble-request.json
```

Pool runtime 之后只用本地路径写入 `assets` 表。

真实 3DGS gateway 或 SDK wrapper 应读取 submit body 中的 `local_input_manifest` 来定位本机素材。`provider-gateway-worker` 会把该 manifest 合并到 `upstream.request_body.inputs.local_input_manifest`，让 Marble、TripoSplat、SAM-3D、Spark、群核科技等外部 worker 能共享同一个本地输入合同。

## Smoke

只检查配置：

```bash
cargo run -p pool-core --example three_dgs_gateway_smoke
```

本地 mock gateway 自检：

```bash
cargo run -p pool-core --example provider_gateway_mock_server -- once
```

启动本地 mock gateway：

```bash
cargo run -p pool-core --example provider_gateway_mock_server -- --bind=127.0.0.1:8787
```

提交真实 gateway 任务：

```bash
POOL_3DGS_GATEWAY_ENDPOINT=http://127.0.0.1:8787 \
cargo run -p pool-core --example three_dgs_gateway_smoke -- request.json target/three-dgs-gateway-smoke worldlabs-marble
```

提交并写出可导入的 production evidence bundle：

```bash
POOL_3DGS_GATEWAY_ENDPOINT=http://127.0.0.1:8787 \
POOL_PROVIDER_PRODUCTION_ATTESTATION=real-3dgs-worker-run-001 \
POOL_PROVIDER_EVIDENCE_BUNDLE=target/three-dgs-gateway-smoke/provider-production-evidence-bundle.json \
cargo run -p pool-core --example three_dgs_gateway_smoke -- request.json target/three-dgs-gateway-smoke worldlabs-marble

cargo run -p pool-cli -- --db target/three-dgs-gateway-smoke/validate.sqlite --project three-dgs-demo \
  validate-production-evidence target/three-dgs-gateway-smoke/provider-production-evidence-bundle.json
```

矩阵化验证所有首批 3DGS profile：

```bash
pool-cli --project demo production-evidence-provider-matrix target/provider-evidence-matrix --no-env
POOL_PROVIDER_PRODUCTION_ATTESTATION=real-vendor-sdk-worker-2026-06-17 pool-cli --project demo production-evidence-provider-matrix target/provider-evidence-matrix --production-upstream --media-endpoint=http://127.0.0.1:8788 --3dgs-endpoint=http://127.0.0.1:8788 --provider-endpoint midjourney=http://127.0.0.1:9701 --provider-endpoint tripo-splat=http://127.0.0.1:9712 --provider-api-key-env midjourney=POOL_PROVIDER_API_KEY_MIDJOURNEY --provider-api-key-env tripo-splat=POOL_PROVIDER_API_KEY_TRIPO_SPLAT --openai-api-key-env OPENAI_API_KEY
POOL_PROVIDER_PRODUCTION_ATTESTATION=real-vendor-sdk-worker-2026-06-17 pool-cli --project demo production-evidence-provider-matrix target/provider-evidence-matrix --production-upstream --media-endpoint=http://127.0.0.1:8788 --3dgs-endpoint=http://127.0.0.1:8788 --provider-endpoint midjourney=http://127.0.0.1:9701 --provider-endpoint tripo-splat=http://127.0.0.1:9712 --provider-api-key-env midjourney=POOL_PROVIDER_API_KEY_MIDJOURNEY --provider-api-key-env tripo-splat=POOL_PROVIDER_API_KEY_TRIPO_SPLAT --openai-api-key-env OPENAI_API_KEY --evidence-bundle=target/provider-evidence-matrix/provider-production-evidence-bundle.json
```

矩阵 runner 会覆盖 `worldlabs-marble`、`tripo-splat`、`sam-3d`、`spark-3dgs` 和 `qunhe-3d`，并把 `evidence_json` 记录进 `provider_requests`。`--no-env` 只验证调度和跳过逻辑；只有显式 `--production-upstream`、提供非占位 `POOL_PROVIDER_PRODUCTION_ATTESTATION` 并连接真实厂商 gateway/SDK worker，才会让 PRD readiness 的 `production_upstream_ready` 变为完成。加 `--evidence-bundle=<path>` 时，成功的真实上游结果会被写成可校验/导入的 `providers[]` 生产证据 bundle，并附带每个 Provider 的本地 metadata 文件。

## 当前边界

- 已实现通用 HTTP gateway adapter、provider-specific gateway profile mapping、状态映射、结果下载、本地 indexed 命名、metadata、smoke example、本地 mock gateway contract server，以及成功 gateway run 后的 `providers[]` production evidence bundle 输出。
- submit body 已携带 `local_input_manifest`，并拒绝远程 URL/data URL 形式的 `input_paths`，使真实上游 worker 可以按本地文件合同读取输入素材。
- Runtime HTTP 的 `POST /api/provider-runs` 已支持 `execution_mode:"gateway"` 调度 `ThreeDgsGatewayProvider`；默认 `auto` 在未配置 gateway 时仍走本地 `Mock3dgsProvider`，用于低成本验证 3DGS 任务、资产入库和前端运行按钮。
- 已覆盖 plain prompt、JSON request、profile 默认 path、TripoSplat profile payload、自定义 provider payload 保留、job id 提取、状态映射、output URL 收集、indexed suffix 保留等单元测试。
- 尚未接入某个厂商的官方 SDK；下一步应把各 provider profile 对接到真实本地 gateway 或官方 SDK adapter，`provider_gateway_mock_server` 用于验证 Pool contract 和本地资产落地，`provider_gateway_template` 用于生成真实 SDK/HTTP worker 的上游翻译模板，`provider_gateway_worker` 可作为本地 HTTP forwarder 接真实或 mock upstream。
