# Provider Gateway Worker

## 目标

`provider_gateway_worker` 是 Pool 的本地 HTTP 转发 worker。它接收 `GenericHttpMediaProvider` 和 `ThreeDgsGatewayProvider` 发出的 Pool gateway 请求，调用 `provider_gateway_template` 生成上游请求，然后把请求转发到真实厂商 worker、官方 SDK 包装服务或本地 `provider_gateway_mock_server`。

它不是厂商 SDK 本身；它固定了 Pool 运行时到外部 AI/3DGS 服务之间的可执行边界。

## 覆盖范围

- `GET /health`
- `POST /v1/media/jobs`
- `GET /v1/media/jobs/<job-id>`
- `POST /v1/3dgs/jobs`
- `POST /v1/3dgs/<provider>/jobs`
- `GET /v1/3dgs/.../jobs/<job-id>`
- Bearer API key 透传
- 按 provider 路由到不同上游 endpoint / bearer key
- Pool request 到上游 request body 的模板翻译
- 上游 job id、status、outputs 的 Pool-compatible 规范化

机器可读合同可通过 Runtime HTTP 或 MCP resource 读取：

```bash
curl http://127.0.0.1:4788/api/provider-gateway-worker
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo provider-gateway-worker-contract
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo mcp pool://provider-gateway-worker
```

走 MCP stdio 时，也可以用命名工具 `pool_provider_gateway_worker` 读取同一份合同，避免 Agent 自己拼 `pool://provider-gateway-worker` URI。

## 生成单 Provider 验收包

真实厂商 SDK/HTTP worker 接入前，可以为单个 Provider 导出本地 conformance package：

```bash
pool-cli --project demo provider-conformance-package worldlabs-marble --output-dir worlds/demo/output
```

等价 Runtime HTTP 入口：

```bash
curl -X POST http://127.0.0.1:4788/api/provider-conformance-packages \
  -H 'Content-Type: application/json' \
  -d '{"project_slug":"demo","provider_id":"worldlabs-marble","output_dir":"worlds/demo/output"}'
```

包会写到 `worlds/demo/output/control/provider-conformance/<provider-id>/`，包含 Provider contract、`provider_gateway_worker` contract、runbook、preflight、runner script 和 manifest。runner 的 `--preflight` 会检查 endpoint、upstream、API key 和 production attestation env；`local` 只跑 Pool gateway worker baseline；`run` 在预检通过后执行 real upstream worker、provider health/smoke、production evidence matrix 和 validate/import。MCP tool `pool_provider_conformance_package` 走同一入口。

## 使用 mock upstream 验证

最快的本地自检：

```bash
pool-cli provider-gateway-worker --once
```

`--once` 会启动内置 mock upstream，覆盖 `GET /health`、AI media submit/poll 和 3DGS submit/poll，用于确认 Pool gateway 翻译、forwarder 归一化和 mock upstream 合同一致。

先启动假上游：

```bash
cargo run -p pool-core --example provider_gateway_mock_server -- --bind=127.0.0.1:8787
```

再启动 worker：

```bash
cargo run -p pool-core --example provider_gateway_worker -- --bind=127.0.0.1:8788 --upstream=http://127.0.0.1:8787
```

Hermes/Agent CLI 侧也可以用 `pool-cli` 启动同一 worker：

```bash
cargo run -p pool-cli -- provider-gateway-worker --bind 127.0.0.1:8788 --upstream http://127.0.0.1:8787
```

真实矩阵通常需要多个厂商 SDK 包装服务。可以用 repeatable provider route 在一个 Pool worker 内分流：

```bash
cargo run -p pool-cli -- provider-gateway-worker \
  --bind 127.0.0.1:8788 \
  --provider-upstream midjourney=http://127.0.0.1:9701 \
  --provider-upstream nano-banana-pro=http://127.0.0.1:9702 \
  --provider-upstream suno=http://127.0.0.1:9703 \
  --provider-upstream worldlabs-marble=http://127.0.0.1:9711 \
  --provider-upstream tripo-splat=http://127.0.0.1:9712 \
  --provider-api-key-env midjourney=POOL_MIDJOURNEY_API_KEY \
  --provider-api-key-env tripo-splat=POOL_TRIPOSPLAT_API_KEY
```

路由优先级固定为 `request.upstream_endpoint > --provider-upstream > provider-specific endpoint env > --upstream`。Bearer key 优先级固定为 `--provider-api-key / --provider-api-key-env > provider-specific api_key_env > --api-key / --api-key-env`。

AI media adapter 指向 worker：

```bash
POOL_MEDIA_GATEWAY_ENDPOINT=http://127.0.0.1:8788 \
cargo run -p pool-core --example generic_media_smoke -- nano-banana-pro request.json target/generic-media-worker-smoke
```

3DGS adapter 指向 worker：

```bash
POOL_3DGS_GATEWAY_ENDPOINT=http://127.0.0.1:8788 \
cargo run -p pool-core --example three_dgs_gateway_smoke -- request.json target/three-dgs-worker-smoke worldlabs-marble
```

AI media + 3DGS 矩阵验证：

```bash
pool-cli --project demo production-evidence-provider-matrix target/provider-evidence-matrix --no-env
POOL_PROVIDER_PRODUCTION_ATTESTATION=real-vendor-sdk-worker-2026-06-17 pool-cli --project demo production-evidence-provider-matrix target/provider-evidence-matrix --production-upstream --media-endpoint=http://127.0.0.1:8788 --3dgs-endpoint=http://127.0.0.1:8788 --provider-endpoint midjourney=http://127.0.0.1:9701 --provider-endpoint tripo-splat=http://127.0.0.1:9712 --provider-api-key-env midjourney=POOL_PROVIDER_API_KEY_MIDJOURNEY --provider-api-key-env tripo-splat=POOL_PROVIDER_API_KEY_TRIPO_SPLAT --openai-api-key-env OPENAI_API_KEY
POOL_PROVIDER_PRODUCTION_ATTESTATION=real-vendor-sdk-worker-2026-06-17 pool-cli --project demo production-evidence-provider-matrix target/provider-evidence-matrix --production-upstream --media-endpoint=http://127.0.0.1:8788 --3dgs-endpoint=http://127.0.0.1:8788 --provider-endpoint midjourney=http://127.0.0.1:9701 --provider-endpoint tripo-splat=http://127.0.0.1:9712 --provider-api-key-env midjourney=POOL_PROVIDER_API_KEY_MIDJOURNEY --provider-api-key-env tripo-splat=POOL_PROVIDER_API_KEY_TRIPO_SPLAT --openai-api-key-env OPENAI_API_KEY --evidence-bundle=target/provider-evidence-matrix/provider-production-evidence-bundle.json
```

该 runner 会通过 Runtime HTTP `/api/provider-runs` 覆盖 9 个 required Provider，并把 `evidence_json` 写入 `provider_requests`。`--provider-endpoint provider=url` 会覆盖单个 Provider 的 endpoint，优先级高于 `--media-endpoint` / `--3dgs-endpoint`；也可用 `POOL_PROVIDER_ENDPOINT_<PROVIDER_ID>` 或 `POOL_<PROVIDER_ID>_ENDPOINT` 环境变量，例如 `POOL_PROVIDER_ENDPOINT_TRIPO_SPLAT`。`--provider-api-key provider=token` / `--provider-api-key-env provider=ENV_NAME` 会把单个 Provider bearer token 传给 runtime 请求，生产证据 metadata 只记录脱敏后的 request。`--no-env` 只做安全跳过检查；接真实厂商 SDK 包装服务后，再加 `--production-upstream` 且提供非占位 `POOL_PROVIDER_PRODUCTION_ATTESTATION`，才可作为生产上游证据。加 `--evidence-bundle=<path>` 后，runner 会把成功的真实上游结果写成 `providers[]` 生产证据 bundle，并为每个 Provider 写出本地 `provider-production-metadata.json`；bundle 只收已下载且存在的本地 artifact，远程 URL 只保留在 response/metadata 中做 provenance。该 bundle 可直接交给 `pool-cli validate-production-evidence` / `import-production-evidence`。本地 mock 或无配置路径不会生成可导入的生产上游证据。

## 接真实上游

真实接入时，`--upstream` 可以指向一个厂商 SDK 包装服务，例如：

```bash
cargo run -p pool-core --example provider_gateway_worker -- \
  --bind=127.0.0.1:8788 \
  --upstream=http://127.0.0.1:9788 \
  --api-key-env=POOL_WORLDLABS_API_KEY
```

等价的 `pool-cli` 入口：

```bash
cargo run -p pool-cli -- provider-gateway-worker \
  --bind 127.0.0.1:8788 \
  --upstream http://127.0.0.1:9788 \
  --api-key-env POOL_WORLDLABS_API_KEY
```

若同一台机器上同时运行多个厂商包装服务，优先使用 `--provider-upstream provider=url`，避免把所有 Provider 硬接到同一个 worker。3DGS path 中的 `triposplat`、`sam3d` 等别名会规范化为 Pool provider id，例如 `tripo-splat`、`sam-3d`。

Pool 也提供一个可运行的 SDK worker 模板，用来验证真实上游包装服务的边界：

```bash
pool-cli provider-sdk-worker-template --once --output-root target/provider-sdk-worker-template
pool-cli provider-sdk-worker-template --bind 127.0.0.1:8798 --output-root target/provider-sdk-worker-template
cargo run -p pool-cli -- provider-gateway-worker --bind 127.0.0.1:8788 --upstream http://127.0.0.1:8798
```

`provider_sdk_worker_template` 接收 `provider_gateway_worker` 转发后的 `upstream.request_body`，校验 `local_input_manifest`，把请求写入 `1-sdk-worker-request.json`，并返回 Pool-compatible `job_id/status/outputs`。它的输出只是模板占位，不应作为生产证据；真实接入时应把模板中的输出生成替换成厂商 SDK/API 调用，然后继续按同一 response contract 返回。

请求流程：

1. Pool adapter 发出 `pool_media_profile` 或 `pool_gateway_profile`。
2. Worker 用 `provider_gateway_template_translation` 生成上游 `request_body`。
3. Worker `POST` 到上游 submit endpoint。
4. Worker 记录 upstream job id 和 poll URL。
5. Pool adapter poll worker。
6. Worker poll 上游并规范化 `outputs[]`。
7. Pool adapter 继续下载远程 URL 到本地文件；远程 URL 只作为 provenance。

如果 Pool submit body 带有 `local_input_manifest`，worker 会把它合并到 `upstream.request_body.local_input_manifest` 和 `upstream.request_body.inputs.local_input_manifest`。真实厂商 SDK wrapper 应使用这个 manifest 定位本地输入文件；远程 URL 仍只能作为输出 provenance，不能作为 Pool runtime 的加载真相源。

## Conformance Runbook

`/api/provider-gateway-worker`、`pool://provider-gateway-worker` 和 `pool-cli provider-gateway-worker-contract` 会暴露 `conformance_runbook`，给 Hermes/Agent 或外部 worker 操作者逐步验收真实上游：

1. `pool-cli provider-gateway-worker --once`：先证明 Pool forwarder、template translation 和 submit/poll normalization 正常。
2. `pool-cli provider-gateway-worker --bind 127.0.0.1:8788 --upstream http://127.0.0.1:8798 --api-key-env POOL_VENDOR_API_KEY`：连接真实厂商 worker 或 SDK wrapper。
3. `generic_media_smoke` / `three_dgs_gateway_smoke`：验证 AI media 与 3DGS submit、poll、download、本地 metadata。
4. `production-evidence-provider-matrix --production-upstream --evidence-bundle=...`：生成带真实 attestation 和本地 artifact 的 Provider 生产证据 bundle。
5. `validate-production-evidence` / `import-production-evidence`：确认模板 id、远程 URL、缺失本地文件和占位 attestation 都会被拒绝。

通过条件是：上游返回可追踪 job id，poll 能进入完成态，Pool 能把输出下载成本地文件，真实 worker 消费 `local_input_manifest`，生产证据包含非占位 attestation 且只引用存在的本地 artifact。

## 与 template/mock 的关系

- `provider_gateway_template`：定义翻译合同，不发 HTTP 请求。
- `provider_gateway_mock_server`：提供假上游，验证 Pool contract 和本地资产落地。
- `provider_gateway_worker`：执行 HTTP 转发，把 Pool request 接到真实或假上游。
- `provider_sdk_worker_template`：提供真实 SDK wrapper 的最小可运行骨架，验证 `local_input_manifest`、审计文件和 Pool-compatible output contract。

## 当前边界

- 已实现本地 HTTP forwarder、submit/poll、API key bearer auth、status/output 规范化和 mock upstream 单元测试。
- 已实现 provider-specific upstream/API key 路由，支持一个 Pool worker 转发到多个真实厂商包装服务。
- 已实现 `local_input_manifest` 透传，供真实上游 worker 按本地文件合同读取 Pool 输入素材。
- 已新增 `provider_sdk_worker_template` example，作为真实厂商 SDK wrapper 的最小可运行样板。
- 已在机器可读合同中暴露 `conformance_runbook`，把 mock baseline、真实 upstream worker、AI media smoke、3DGS smoke、production matrix 和 validate/import 串成真实接入验收路径。
- 尚未内置 Midjourney、Nano Banana Pro、Suno、World Labs Marble、TripoSplat、SAM-3D、Spark 或群核科技的官方 SDK。
- 上游服务需要返回 `job_id` 或等价字段，并在 poll 时返回 `status` 和 `outputs`/`assets`/`files`/URL 字段。
