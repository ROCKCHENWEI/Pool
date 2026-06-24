# ComfyUI Adapter

## 当前能力

`ComfyUiProvider` 是 `ProviderAdapter` 的真实 HTTP 实现骨架，面向本地 ComfyUI 服务。

- `health()` 调用 `GET /system_stats`
- `submit()` 调用 `POST /prompt`
- `poll()` 调用 `GET /history/{prompt_id}`
- `stream_progress_events()` 连接 `/ws?clientId=...` 并把 WebSocket progress 转成 `RuntimeEvent`
- `download()` 调用 `GET /view`，把输出文件下载到本地 `output_dir`
- `download_and_index()` 下载输出并同步为 `AssetRecord`
- `verify()` 校验本地输出路径
- request metadata 写入 `.comfyui-<prompt_id>-request.json`
- Runtime HTTP `POST /api/provider-runs` 可调度 ComfyUI adapter；`POOL_COMFYUI_ENDPOINT` / `COMFYUI_ENDPOINT` 可覆盖 endpoint，`POOL_COMFYUI_CLIENT_ID` 可固定 client id。

## 输入约定

`ProviderRequest.prompt` 必须是 ComfyUI workflow API JSON。

如果传入的是原始 workflow object，adapter 会包装为：

```json
{
  "prompt": {},
  "client_id": "pool-..."
}
```

如果传入的 JSON 已包含 `prompt` 字段，adapter 会保留原结构，并补 `client_id`。

## 运行方式

只检查 ComfyUI health：

```bash
POOL_COMFYUI_ENDPOINT=http://127.0.0.1:8188 cargo run -p pool-core --example comfyui_smoke
```

提交 workflow API JSON：

```bash
cargo run -p pool-core --example comfyui_smoke -- /path/to/workflow_api.json target/comfyui-smoke
```

提交 workflow 并把 WebSocket progress 写入 SQLite：

```bash
cargo run -p pool-core --example comfyui_smoke -- /path/to/workflow_api.json target/comfyui-smoke target/comfyui-events.sqlite
```

提交 workflow、写入 progress，并在完成后下载输出同步到 assets 表：

```bash
cargo run -p pool-core --example comfyui_smoke -- /path/to/workflow_api.json target/comfyui-smoke target/comfyui-events.sqlite index-assets
```

## 下一步

- 把 ComfyUI output node 与 Pool workflow node 建立更精确的 source mapping。
- 将 ComfyUI output node 与运行节点状态建立更精确的完成/失败映射。
