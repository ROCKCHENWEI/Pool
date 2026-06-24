# Pool MCP Bridge for Unreal

This integration is a Python-based Unreal Editor bridge for Pool's Unreal MCP contract.

## Install

1. Copy `integrations/unreal/PoolMcpBridge` into `<YourProject>/Plugins/PoolMcpBridge`.
2. Enable Unreal plugins `PythonScriptPlugin` and `EditorScriptingUtilities`.
3. Restart Unreal Editor.

The plugin autostarts by default from `Content/Python/init_unreal.py`.

## Configure

Environment variables:

- `POOL_UNREAL_MCP_HOST`: default `127.0.0.1`
- `POOL_UNREAL_MCP_PORT`: default `8791`
- `POOL_UNREAL_MCP_AUDIT_ROOT`: default `Saved/PoolMcpBridge`
- `POOL_UNREAL_MCP_AUTOSTART`: set `0` to disable autostart

Pool can target the plugin directly:

```bash
POOL_UNREAL_MCP_ENDPOINT=http://127.0.0.1:8791 cargo run -p pool-core --example run_unreal_mcp_action
```

Or through the local Rust bridge worker:

```bash
cargo run -p pool-cli -- unreal-mcp-bridge-worker \
  --bind 127.0.0.1:8790 \
  --output-root worlds/demo/output \
  --upstream http://127.0.0.1:8791
```

Then use:

```bash
POOL_UNREAL_MCP_ENDPOINT=http://127.0.0.1:8790 cargo run -p pool-core --example run_unreal_mcp_action
```

## Routes

- `GET /health`
- `POST /mcp`
- `POST /v1/unreal/actions`

The handler accepts Pool's `pool_unreal_action` and `mcp_payload` wrapper and writes local audit request/response JSON.
`unreal.create_scene` also writes a scene assembly manifest to `Saved/PoolMcpBridge/SceneAssembly` by default, preserving the requested level, imported assets, actor placements, camera presets, light presets, world origin, Pool action metadata, and handoff metadata.

## Tool Coverage

- `unreal.open_project`
- `unreal.import_asset`
- `unreal.create_scene`
- `unreal.run_viewport`
- `unreal.render_sequence`
- `unreal.export_build`
- `unreal.transcode_media`
- `unreal.health`

The script imports outside Unreal for syntax checks and dry-run validation. Inside Unreal, the implementation uses Unreal Python APIs where available and falls back to auditable manifests when an Editor API is version-specific. For scene assembly, pass `asset_paths`, optional `actors[]` with `name` / `asset_path` / `location` / `rotation` / `scale`, optional `cameras[]`, optional `lights[]`, and optional `output_dir`.
