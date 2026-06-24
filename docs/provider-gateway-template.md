# Provider Gateway Template

## 目标

`provider_gateway_template` 是真实厂商接入前的本地翻译模板。它把 Pool runtime 发送到 gateway 的 `pool_media_profile` / `pool_gateway_profile` 和 `provider_payload` 转成上游 SDK/HTTP worker 需要的规范化请求。

它不调用真实 Midjourney、Nano Banana Pro、Suno、World Labs Marble、TripoSplat、SAM-3D、Spark 或群核科技服务；它固定真实 gateway worker 必须实现的翻译边界。

## 覆盖范围

- AI media：`midjourney`、`nano-banana-pro`、`suno`
- 3DGS：`worldlabs-marble`、`tripo-splat`、`sam-3d`、`spark-3dgs`、`qunhe-3d`
- 上游 endpoint 环境变量候选
- 上游 API key 环境变量候选
- `provider_payload` 透传或默认上游 request body
- Pool-compatible submit/poll response contract
- 本地文件优先策略

## 使用

查看总合同：

```bash
cargo run -p pool-core --example provider_gateway_template -- --contract
```

生成 Nano Banana Pro 上游翻译模板：

```bash
cargo run -p pool-core --example provider_gateway_template -- ai-media nano-banana-pro
```

生成 TripoSplat 上游翻译模板：

```bash
cargo run -p pool-core --example provider_gateway_template -- 3dgs tripo-splat
```

使用真实 Pool gateway request JSON：

```bash
cargo run -p pool-core --example provider_gateway_template -- 3dgs worldlabs-marble request.json
```

## Worker 需要实现的部分

1. 读取 `upstream.request_body`。
2. 使用 `upstream.endpoint_env` 与 `upstream.api_key_env` 找到真实厂商 endpoint 和凭证。
3. 读取 `upstream.request_body.inputs.local_input_manifest` 定位本地输入素材。
4. 调用厂商 SDK 或 HTTP API。
5. 把厂商 job id 规范化成 `job_id`。
6. poll 完成后把厂商输出规范化成 Pool gateway `outputs[]`。
7. 返回远程 URL 只作为 provenance；Pool runtime 继续负责下载成本地文件。

可先用 SDK worker 模板验证边界：

```bash
pool-cli provider-sdk-worker-template --once --output-root target/provider-sdk-worker-template
pool-cli provider-sdk-worker-template --bind 127.0.0.1:8798 --output-root target/provider-sdk-worker-template
```

这个模板会写出 `1-sdk-worker-request.json`，并返回 Pool-compatible `job_id/status/outputs`。它不调用真实厂商服务，不能作为生产上游证据；真实接入时把模板输出替换成厂商 SDK/API 调用即可。

## 与 mock server 的关系

- `provider_gateway_mock_server`：验证 Pool adapter 的 submit/poll/download 和本地资产落地。
- `provider_gateway_template`：指导真实 gateway/SDK worker 如何把 Pool request 翻译到厂商 API，并把结果翻译回 Pool response。
- `provider_gateway_worker`：复用本模板执行本地 HTTP 转发，把 Pool gateway request 接到真实或 mock upstream。
- `provider_sdk_worker_template`：提供可运行 SDK wrapper 骨架，验证上游 worker 如何接收 `local_input_manifest`、写审计文件并返回 Pool-compatible outputs。
