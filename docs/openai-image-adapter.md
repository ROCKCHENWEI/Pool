# OpenAI Image Provider Adapter

## 目标

`OpenAiImageProvider` 是 Pool 第一批真实 AI 图片 ProviderAdapter。它把 OpenAI Images API 纳入统一运行闭环：

- `health`
- `submit`
- `poll`
- `download`
- `verify`
- `ProviderTaskRunner`
- `workflow_events`
- `assets`

## 官方接口

当前实现使用 Image API 的两个端点：

```text
POST https://api.openai.com/v1/images/generations
POST https://api.openai.com/v1/images/edits
```

默认模型：

```text
gpt-image-2
```

官方文档显示 Images API 同时提供 generation 和 edit 入口。Pool 已支持单 prompt 图片生成，以及基于本地输入图片/mask 的 image edit；Responses API image tool 后续作为独立节点补充。

## 认证

```bash
export OPENAI_API_KEY=...
```

可选组织和项目头：

```bash
export OPENAI_ORG_ID=...
export OPENAI_PROJECT_ID=...
```

可选覆盖：

```bash
export POOL_OPENAI_ENDPOINT=https://api.openai.com/v1
export POOL_OPENAI_IMAGE_MODEL=gpt-image-2
```

生产证据输出：

```bash
export POOL_PROVIDER_PRODUCTION_ATTESTATION=real-openai-image-run-001
export POOL_PROVIDER_EVIDENCE_BUNDLE=worlds/demo/output/control/production-evidence/openai-image.bundle.json
```

`POOL_PROVIDER_PRODUCTION_ATTESTATION` 必须指向真实 OpenAI API run 或受控官方 SDK worker run；模板、mock、dry-run 或占位字符串会在 production evidence validate/import 阶段被拒绝。设置 `POOL_PROVIDER_EVIDENCE_BUNDLE` 或 `POOL_PRODUCTION_EVIDENCE_BUNDLE` 后，成功的 `openai_image_smoke` 会把本地图片 artifact、metadata path、OpenAI request id 和 attestation 写成 `providers[]` bundle；只设置 attestation 时默认写到输出目录下的 `provider-production-evidence-bundle.json`。

## 请求 JSON

最小请求可以是纯文本 prompt：

```text
production concept art of a neon stage
```

也可以是 JSON：

```json
{
  "model": "gpt-image-2",
  "prompt": "production concept art of a neon stage",
  "size": "1024x1024",
  "quality": "medium",
  "output_format": "png"
}
```

edit 请求使用同一个 JSON prompt，但必须传入本地图片路径。`operation:"edit"` 可显式选择 edit；如果存在 `image`、`images`、`input_images` 或 `mask` 字段，Pool 也会自动切换到 `POST /images/edits`：

```json
{
  "operation": "edit",
  "model": "gpt-image-2",
  "prompt": "replace the background with a stage lighting rig",
  "image": "worlds/demo/source/0-reference.png",
  "mask": "worlds/demo/source/1-mask.png",
  "size": "1024x1024",
  "output_format": "png"
}
```

`image` 可为单个本地路径，`images` / `input_images` 可为本地路径数组；`mask` 只能是单个本地路径。Runtime `ProviderRequest.input_paths` 也会作为 edit 输入：当 prompt JSON 没有显式 `image` / `images` / `input_images`，且没有显式 `operation:"generate"` 时，Pool 会把 `input_paths` 注入为 `input_images`。这使 Web/CLI 的图片+文字组合输入可以直接触发 OpenAI image edit，而不必把路径写进 prompt JSON。Pool 会在 submit 前拒绝远程 URL 或不存在的输入文件，并用 multipart/form-data 上传到 OpenAI。metadata 只记录 `image_paths` / `mask_path`，不会把输入图片 bytes 写入 request metadata。

未传 `model` 时默认使用 `POOL_OPENAI_IMAGE_MODEL`，未设置时为 `gpt-image-2`。除 edit 的本地文件字段外，额外字段会透传到 request body 或 multipart text fields，便于后续跟进官方新增参数。

## 运行

只检查本地配置：

```bash
cargo run -p pool-core --example openai_image_smoke
```

提交真实任务：

```bash
OPENAI_API_KEY=... cargo run -p pool-core --example openai_image_smoke -- request.json target/openai-image-smoke
```

提交并写出可导入的 production evidence bundle：

```bash
OPENAI_API_KEY=... \
POOL_PROVIDER_PRODUCTION_ATTESTATION=real-openai-image-run-001 \
POOL_PROVIDER_EVIDENCE_BUNDLE=target/openai-image-smoke/provider-production-evidence-bundle.json \
cargo run -p pool-core --example openai_image_smoke -- request.json target/openai-image-smoke

cargo run -p pool-cli -- --db target/openai-image-smoke/validate.sqlite --project openai-image-demo \
  validate-production-evidence target/openai-image-smoke/provider-production-evidence-bundle.json
```

运行结果会写入：

- `target/openai-image-smoke/pool-runtime.sqlite`
- `target/openai-image-smoke/.openai-image-<request_id>-request.json`
- `target/openai-image-smoke/N-openai-image.png`

## 本地优先契约

OpenAI generation/edit 返回的 `b64_json` 会直接解码写入本地图片。如果 gateway 返回 URL，Pool 也会先下载为本地文件，再通过 `RuntimeRepository::index_local_outputs` 写入 `assets` 表。

metadata 只保存 operation、endpoint path、request、response summary、usage、revised prompt、输入本地路径和输出本地路径，不持久化大体积 base64 payload 或输入图片 bytes。

## 当前边界

- 已实现 generation/edit HTTP submit、base64/URL 输出落地、metadata、verify、单元测试、smoke example，以及成功 OpenAI API run 后的 `providers[]` production evidence bundle 输出。
- 已接入 `ProviderTaskRunner`，成功后会写入 `assets` 和 `workflow_events`。
- 尚未在本机使用真实 OpenAI API key 跑通端到端任务。
- 暂未实现 Responses API image tool。

## 参考

- [OpenAI Image generation guide](https://platform.openai.com/docs/guides/image-generation)
- [OpenAI Images API reference](https://platform.openai.com/docs/api-reference/images/generate)
