# Provider Gateway Mock Server

## 目标

`provider_gateway_mock_server` 是本地 Provider gateway 契约服务器，用来在没有真实厂商账号、SDK 或安全代理时验证 Pool 的 AI media gateway 与 3DGS gateway adapter。命令行 example 和 Runtime HTTP 单元测试共用 `pool_core::ProviderGatewayMock`，避免维护多套临时 provider 协议。

它不是 Midjourney、Nano Banana Pro、Suno、World Labs Marble、TripoSplat、SAM-3D、Spark 或群核科技的真实实现。它的作用是固定 Pool 到本地 gateway 的 HTTP 契约，让后续厂商 SDK、安全代理或 Hermes gateway 可以替换同一组 endpoint。

## 覆盖范围

- `GET /health`
- `POST /v1/media/jobs`
- `GET /v1/media/jobs/<job-id>`
- `POST /v1/3dgs/jobs`
- `POST /v1/3dgs/triposplat/jobs`
- `POST /v1/3dgs/sam-3d/jobs`
- `POST /v1/3dgs/spark/jobs`
- `POST /v1/3dgs/qunhe/jobs`
- `GET /v1/3dgs/.../jobs/<job-id>`
- `GET /outputs/<family>/<job-id>/<file>`

Media submit 会返回 `queued` job，poll 返回 `completed` 与一个可下载 output URL，便于验证 `GenericHttpMediaProvider` 的 submit/poll/download、本地落地、metadata 和 assets 入库。

3DGS submit 会返回 `queued` job，poll 返回 `completed` 与 `json/glb/spz` 三个 output URL，便于验证 `ThreeDgsGatewayProvider` 的 submit/poll/download 和 image-blaster indexed file 命名。

## 运行

只做 route 自检：

```bash
cargo run -p pool-core --example provider_gateway_mock_server -- once
```

启动常驻 mock gateway：

```bash
cargo run -p pool-core --example provider_gateway_mock_server -- --bind=127.0.0.1:8787
```

用于脚本 smoke 时可以限制处理请求数，完成后自动退出：

```bash
cargo run -p pool-core --example provider_gateway_mock_server -- --bind=127.0.0.1:8787 --max-requests=12
```

## Media Smoke

```bash
POOL_MEDIA_GATEWAY_ENDPOINT=http://127.0.0.1:8787 \
cargo run -p pool-core --example generic_media_smoke -- nano-banana-pro request.json target/generic-media-smoke
```

可换成：

- `midjourney`
- `nano-banana-pro`
- `suno`

## 3DGS Smoke

```bash
POOL_3DGS_GATEWAY_ENDPOINT=http://127.0.0.1:8787 \
cargo run -p pool-core --example three_dgs_gateway_smoke -- request.json target/three-dgs-gateway-smoke worldlabs-marble
```

可换成：

- `worldlabs-marble`
- `tripo-splat`
- `sam-3d`
- `spark-3dgs`
- `qunhe-3d`

## 契约边界

- Mock server 返回 deterministic placeholder bytes，验证的是 Pool runtime 的 gateway contract，不验证厂商生成质量。
- Provider URL 仍只作为 provenance；adapter 必须下载为本地文件后再写入 assets。
- 真实接入时应保持同一 submit/poll/output 字段，或者在本地 gateway 内完成厂商私有协议到 Pool 契约的翻译。
