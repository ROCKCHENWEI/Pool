# Kling Provider Adapter

## 目标

`KlingProvider` 是 Pool 第一批真实 AI 视频 ProviderAdapter。它把 Kling 的异步视频生成任务纳入统一运行闭环：

- `health`
- `submit`
- `poll`
- `download`
- `verify`
- `ProviderTaskRunner`
- `workflow_events`
- `assets`

## 认证

支持两种认证方式：

```bash
export POOL_KLING_API_KEY=...
```

或：

```bash
export POOL_KLING_ACCESS_KEY=...
export POOL_KLING_SECRET_KEY=...
```

`POOL_KLING_API_KEY` 会按 Bearer token 发送。`POOL_KLING_ACCESS_KEY` 与 `POOL_KLING_SECRET_KEY` 会生成 HS256 JWT，claims 包含 `iss`、`exp`、`nbf`。

默认 endpoint：

```bash
https://api.klingapi.com
```

可用 `POOL_KLING_ENDPOINT` 覆盖，例如切换到官方或代理网关：

```bash
export POOL_KLING_ENDPOINT=https://api-singapore.klingai.com
```

## 请求 JSON

最小文字转视频请求：

```json
{
  "prompt": "cinematic robot walking through rain",
  "duration": 5,
  "aspect_ratio": "16:9"
}
```

图片转视频请求：

```json
{
  "prompt": "animate this concept art into a slow camera push",
  "image_url": "https://example.com/concept.png",
  "duration": 5,
  "mode": "std"
}
```

Runtime `ProviderRequest.input_paths` 也可以触发图片转视频：当请求 JSON 没有显式 `image` / `image_url`，但 `input_paths` 包含本地图片时，Pool 会读取第一张本地图片，编码为 data URL 写入 submit body 的 `image` 字段，并调用 `/v1/videos/image2video`。metadata 会把 `image` 替换为 `local_image_data_url_redacted`，同时记录 `local_input_paths`；不会把图片 bytes 或 base64 payload 持久化到 request metadata。

未传 `model` 时默认使用 `kling-v2.6-std`。额外字段会透传到 provider request body，便于适配不同 gateway 的参数扩展。

## 运行

只检查本地配置：

```bash
cargo run -p pool-core --example kling_smoke
```

提交真实任务：

```bash
POOL_KLING_API_KEY=... cargo run -p pool-core --example kling_smoke -- request.json target/kling-smoke
```

运行结果会写入：

- `target/kling-smoke/pool-runtime.sqlite`
- `target/kling-smoke/.kling-<task_id>-request.json`
- `target/kling-smoke/N-kling-output.mp4`

## 本地优先契约

Kling 返回的远程视频 URL 不作为前端或引擎加载真相源。Pool 必须先下载为本地文件，再通过 `RuntimeRepository::index_local_outputs` 写入 `assets` 表。

## 当前边界

- 已实现 HTTP submit、poll、download、JWT 生成、本地 `input_paths` 图片转视频映射和单元测试。
- 已接入 `ProviderTaskRunner` smoke example。
- 真实 Kling endpoint 与字段可能因官方/第三方 gateway 不同而变化；通过 `POOL_KLING_ENDPOINT` 和请求 JSON 透传字段处理差异。
- 尚未在本地使用真实账号跑通端到端任务。

## 参考

- [KlingAPI docs](https://klingapi.com/docs)
- [Kling AI official user manual](https://kling.ai/document-api/quickStart%2FuserManual)
