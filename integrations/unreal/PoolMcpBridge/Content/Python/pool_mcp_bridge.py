"""Pool Unreal MCP Bridge.

This module is designed to run inside Unreal Editor's Python runtime. It also
imports outside Unreal for local syntax checks and dry-run request validation.
"""

from __future__ import annotations

import json
import os
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any, Callable
from urllib.parse import urlparse


try:
    import unreal  # type: ignore
except Exception:  # pragma: no cover - local non-Unreal validation path.
    unreal = None  # type: ignore


_SERVER: ThreadingHTTPServer | None = None
_SERVER_THREAD: threading.Thread | None = None


def start_from_env() -> ThreadingHTTPServer:
    host = os.environ.get("POOL_UNREAL_MCP_HOST", "127.0.0.1")
    port = int(os.environ.get("POOL_UNREAL_MCP_PORT", "8791"))
    audit_root = os.environ.get(
        "POOL_UNREAL_MCP_AUDIT_ROOT",
        "Saved/PoolMcpBridge",
    )
    return start_server(host=host, port=port, audit_root=audit_root)


def start_server(
    host: str = "127.0.0.1",
    port: int = 8791,
    audit_root: str | os.PathLike[str] = "Saved/PoolMcpBridge",
) -> ThreadingHTTPServer:
    global _SERVER, _SERVER_THREAD

    if _SERVER is not None:
        return _SERVER

    handler = make_handler(Path(audit_root))
    _SERVER = ThreadingHTTPServer((host, port), handler)
    _SERVER_THREAD = threading.Thread(target=_SERVER.serve_forever, daemon=True)
    _SERVER_THREAD.start()
    log("Pool MCP bridge listening on http://{}:{}".format(host, port))
    return _SERVER


def stop_server() -> None:
    global _SERVER, _SERVER_THREAD

    if _SERVER is None:
        return
    _SERVER.shutdown()
    _SERVER.server_close()
    _SERVER = None
    _SERVER_THREAD = None


def make_handler(audit_root: Path) -> type[BaseHTTPRequestHandler]:
    class PoolMcpBridgeHandler(BaseHTTPRequestHandler):
        server_version = "PoolMcpBridge/0.1"

        def do_OPTIONS(self) -> None:  # noqa: N802 - BaseHTTPRequestHandler API
            self.send_empty(204)

        def do_GET(self) -> None:  # noqa: N802 - BaseHTTPRequestHandler API
            path = urlparse(self.path).path
            if path in ("/", "/health", "/v1/health"):
                self.send_json(
                    200,
                    {
                        "ok": True,
                        "status": "ready",
                        "service": "pool-unreal-editor-bridge",
                        "unreal_available": unreal is not None,
                        "audit_root": str(audit_root),
                    },
                )
                return
            self.send_json(404, {"error": "not_found", "path": path})

        def do_POST(self) -> None:  # noqa: N802 - BaseHTTPRequestHandler API
            path = urlparse(self.path).path
            if path not in ("/mcp", "/v1/unreal/actions"):
                self.send_json(404, {"error": "not_found", "path": path})
                return

            try:
                content_length = int(self.headers.get("Content-Length", "0"))
                raw_body = self.rfile.read(content_length).decode("utf-8")
                request = json.loads(raw_body) if raw_body.strip() else {}
                result = handle_pool_request(request, audit_root)
                self.send_json(200 if result.get("ok") else 500, result)
            except ValueError as error:
                self.send_json(
                    400,
                    {
                        "ok": False,
                        "error": "invalid_pool_unreal_request",
                        "message": str(error),
                    },
                )
            except Exception as error:  # pragma: no cover - Unreal API failures.
                self.send_json(
                    500,
                    {
                        "ok": False,
                        "error": "pool_unreal_bridge_error",
                        "message": str(error),
                    },
                )

        def send_empty(self, status_code: int) -> None:
            self.send_response(status_code)
            self.send_header("Access-Control-Allow-Origin", "*")
            self.send_header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
            self.send_header("Access-Control-Allow-Headers", "Content-Type, Authorization")
            self.send_header("Content-Length", "0")
            self.end_headers()

        def send_json(self, status_code: int, payload: dict[str, Any]) -> None:
            body = json.dumps(payload, indent=2, ensure_ascii=False).encode("utf-8")
            self.send_response(status_code)
            self.send_header("Content-Type", "application/json; charset=utf-8")
            self.send_header("Access-Control-Allow-Origin", "*")
            self.send_header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
            self.send_header("Access-Control-Allow-Headers", "Content-Type, Authorization")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def log_message(self, fmt: str, *args: Any) -> None:
            log(fmt % args)

    return PoolMcpBridgeHandler


def handle_pool_request(request: dict[str, Any], audit_root: Path) -> dict[str, Any]:
    validate_request(request)
    pool_action = request["pool_unreal_action"]
    mcp_payload = request["mcp_payload"]
    tool = mcp_payload["tool"]
    arguments = mcp_payload.get("arguments") or {}
    audit_root.mkdir(parents=True, exist_ok=True)
    action_id = "{}-{}".format(
        pool_action.get("operation", "action").replace("_", "-"),
        len(list(audit_root.glob("*-request.json"))) + 1,
    )
    request_path = audit_root / "{}-request.json".format(action_id)
    response_path = audit_root / "{}-response.json".format(action_id)
    request_path.write_text(json.dumps(request, indent=2, ensure_ascii=False), encoding="utf-8")

    result = dispatch_tool(tool, arguments, request)
    result.setdefault("ok", True)
    result.setdefault("success", bool(result["ok"]))
    result.setdefault("status", "completed" if result["ok"] else "failed")
    result.setdefault("message", "pool unreal action {}".format(tool))
    result.setdefault("artifacts", [])
    result["artifacts"] = list(dict.fromkeys(result["artifacts"] + [str(request_path), str(response_path)]))
    result["pool_unreal_bridge"] = {
        "action_id": action_id,
        "tool": tool,
        "operation": pool_action.get("operation"),
        "profile_id": pool_action.get("profile_id"),
        "request_path": str(request_path),
        "response_path": str(response_path),
        "unreal_available": unreal is not None,
    }
    response_path.write_text(json.dumps(result, indent=2, ensure_ascii=False), encoding="utf-8")
    return result


def validate_request(request: dict[str, Any]) -> None:
    required = [
        "adapter_id",
        "action_kind",
        "priority",
        "pool_unreal_action",
        "mcp_payload",
    ]
    missing = [field for field in required if field not in request]
    if missing:
        raise ValueError("missing required fields: {}".format(", ".join(missing)))
    if request.get("adapter_id") != "unreal":
        raise ValueError("adapter_id must be unreal")
    pool_tool = request["pool_unreal_action"].get("mcp_tool")
    mcp_tool = request["mcp_payload"].get("tool")
    if not mcp_tool or not str(mcp_tool).startswith("unreal."):
        raise ValueError("mcp_payload.tool must start with unreal.")
    if pool_tool != mcp_tool:
        raise ValueError("pool_unreal_action.mcp_tool must match mcp_payload.tool")


def dispatch_tool(tool: str, arguments: dict[str, Any], request: dict[str, Any]) -> dict[str, Any]:
    handlers: dict[str, Callable[[dict[str, Any], dict[str, Any]], dict[str, Any]]] = {
        "unreal.open_project": open_project,
        "unreal.import_asset": import_asset,
        "unreal.create_scene": create_scene,
        "unreal.run_viewport": run_viewport,
        "unreal.render_sequence": render_sequence,
        "unreal.export_build": export_build,
        "unreal.transcode_media": transcode_media,
        "unreal.health": health_action,
    }
    handler = handlers.get(tool, execute_generic)
    return handler(arguments, request)


def health_action(arguments: dict[str, Any], request: dict[str, Any]) -> dict[str, Any]:
    return {
        "ok": True,
        "message": "Unreal Python bridge ready",
        "artifacts": [],
    }


def open_project(arguments: dict[str, Any], request: dict[str, Any]) -> dict[str, Any]:
    project_file = arguments.get("project_file")
    if unreal is not None and project_file:
        unreal.EditorLoadingAndSavingUtils.load_map(project_file)
    return {
        "message": "open_project {}".format(project_file or "current"),
        "artifacts": ["unreal://project/current"],
    }


def import_asset(arguments: dict[str, Any], request: dict[str, Any]) -> dict[str, Any]:
    asset_paths = list(arguments.get("asset_paths") or [])
    destination = arguments.get("destination") or "/Game/Pool/Imported"
    imported = []
    if unreal is not None and asset_paths:
        task_class = unreal.AssetImportTask
        tools = unreal.AssetToolsHelpers.get_asset_tools()
        tasks = []
        for path in asset_paths:
            task = task_class()
            task.filename = str(path)
            task.destination_path = destination
            task.automated = True
            task.save = True
            task.replace_existing = bool(arguments.get("replace_existing", False))
            tasks.append(task)
        tools.import_asset_tasks(tasks)
        for task in tasks:
            imported.extend([str(item) for item in task.imported_object_paths])
    else:
        imported = ["unreal://asset{}".format(destination)]
    return {
        "message": "imported {} asset(s)".format(len(asset_paths)),
        "artifacts": imported or ["unreal://asset{}".format(destination)],
    }


def create_scene(arguments: dict[str, Any], request: dict[str, Any]) -> dict[str, Any]:
    level = arguments.get("level") or "pool_content_burst"
    import_result = import_asset(arguments, request)
    spawned_artifacts: list[str] = []
    if unreal is not None:
        maybe_create_level(level)
        spawned_artifacts = maybe_spawn_scene_actors(arguments)
        maybe_add_camera_and_light(arguments)
    else:
        spawned_artifacts = dry_run_scene_actor_artifacts(arguments)
    manifest_path = write_scene_assembly_manifest(level, arguments, request, import_result, spawned_artifacts)
    artifacts = (
        ["unreal://level/{}".format(level)]
        + import_result.get("artifacts", [])
        + spawned_artifacts
        + [str(manifest_path)]
    )
    return {
        "message": "created scene {}".format(level),
        "artifacts": artifacts,
        "scene_assembly_manifest": str(manifest_path),
    }


def run_viewport(arguments: dict[str, Any], request: dict[str, Any]) -> dict[str, Any]:
    level = arguments.get("level") or "current"
    if unreal is not None:
        unreal.SystemLibrary.execute_console_command(None, "CAMERA ALIGN ACTIVEVIEWPORTONLY")
    return {
        "message": "viewport ready for {}".format(level),
        "artifacts": ["unreal://viewport/{}".format(level)],
    }


def render_sequence(arguments: dict[str, Any], request: dict[str, Any]) -> dict[str, Any]:
    sequence = arguments.get("sequence") or "main"
    output_dir = arguments.get("output_dir") or "Saved/PoolRenders"
    Path(output_dir).mkdir(parents=True, exist_ok=True)
    manifest = Path(output_dir) / "{}-render-manifest.json".format(sequence)
    manifest.write_text(
        json.dumps(
            {
                "sequence": sequence,
                "preset": arguments.get("preset") or "preview_1080p",
                "unreal_execution": unreal is not None,
            },
            indent=2,
        ),
        encoding="utf-8",
    )
    return {
        "message": "render_sequence queued {}".format(sequence),
        "artifacts": [str(manifest)],
    }


def export_build(arguments: dict[str, Any], request: dict[str, Any]) -> dict[str, Any]:
    output_dir = arguments.get("output_dir") or "Saved/PoolBuilds"
    Path(output_dir).mkdir(parents=True, exist_ok=True)
    manifest = Path(output_dir) / "pool-build-manifest.json"
    manifest.write_text(
        json.dumps(
            {
                "target_platform": arguments.get("target_platform") or "Mac",
                "configuration": arguments.get("configuration") or "Development",
                "unreal_execution": unreal is not None,
            },
            indent=2,
        ),
        encoding="utf-8",
    )
    return {
        "message": "export_build manifest written",
        "artifacts": [str(manifest)],
    }


def transcode_media(arguments: dict[str, Any], request: dict[str, Any]) -> dict[str, Any]:
    return {
        "message": "transcode handoff created",
        "artifacts": [
            str(arguments.get("input_path") or ""),
            str(arguments.get("output_path") or ""),
        ],
    }


def execute_generic(arguments: dict[str, Any], request: dict[str, Any]) -> dict[str, Any]:
    return {
        "message": "generic Unreal action accepted",
        "artifacts": ["unreal://action/generic"],
    }


def maybe_create_level(level: str) -> None:
    if unreal is None:
        return
    # Prefer creating/saving a map through Editor APIs when available. Unreal's
    # Python surface varies by version, so unsupported versions fall back to the
    # current level while still returning auditable artifacts to Pool.
    try:
        tools = unreal.AssetToolsHelpers.get_asset_tools()
        package_path = "/Game/Pool/Levels/{}".format(level)
        tools.create_asset(level, "/Game/Pool/Levels", unreal.World, None)
        unreal.EditorLoadingAndSavingUtils.save_map(unreal.EditorLevelLibrary.get_editor_world(), package_path)
    except Exception as error:  # pragma: no cover - version-specific Unreal API.
        log("create level fallback: {}".format(error))


def maybe_add_camera_and_light(arguments: dict[str, Any]) -> None:
    if unreal is None:
        return
    try:
        location = unreal.Vector(0.0, -400.0, 200.0)
        rotation = unreal.Rotator(-15.0, 0.0, 0.0)
        unreal.EditorLevelLibrary.spawn_actor_from_class(unreal.CineCameraActor, location, rotation)
        unreal.EditorLevelLibrary.spawn_actor_from_class(
            unreal.DirectionalLight,
            unreal.Vector(0.0, 0.0, 400.0),
            unreal.Rotator(-45.0, 45.0, 0.0),
        )
    except Exception as error:  # pragma: no cover - version-specific Unreal API.
        log("camera/light fallback: {}".format(error))


def maybe_spawn_scene_actors(arguments: dict[str, Any]) -> list[str]:
    if unreal is None:
        return []
    artifacts: list[str] = []
    for index, actor in enumerate(normalize_actor_specs(arguments)):
        asset_path = actor.get("asset_path") or actor.get("asset")
        if not asset_path:
            continue
        try:
            asset = unreal.load_asset(str(asset_path))
            if asset is None:
                log("scene actor asset not found: {}".format(asset_path))
                continue
            location = vector_from(actor.get("location"), default=(index * 150.0, 0.0, 0.0))
            rotation = rotator_from(actor.get("rotation"))
            spawned = unreal.EditorLevelLibrary.spawn_actor_from_object(asset, location, rotation)
            scale = actor.get("scale")
            if scale is not None and spawned is not None:
                spawned.set_actor_scale3d(vector_from(scale, default=(1.0, 1.0, 1.0)))
            actor_name = actor.get("name")
            if actor_name and spawned is not None:
                spawned.set_actor_label(str(actor_name))
            artifacts.append("unreal://actor/{}".format(actor_name or index))
        except Exception as error:  # pragma: no cover - version-specific Unreal API.
            log("spawn scene actor fallback: {}".format(error))
    return artifacts


def dry_run_scene_actor_artifacts(arguments: dict[str, Any]) -> list[str]:
    artifacts = []
    for index, actor in enumerate(normalize_actor_specs(arguments)):
        name = actor.get("name") or "actor-{}".format(index + 1)
        artifacts.append("unreal://actor/{}".format(name))
    return artifacts


def normalize_actor_specs(arguments: dict[str, Any]) -> list[dict[str, Any]]:
    explicit = arguments.get("actors")
    if isinstance(explicit, list):
        return [actor for actor in explicit if isinstance(actor, dict)]
    specs = []
    for index, asset_path in enumerate(list(arguments.get("asset_paths") or [])):
        specs.append(
            {
                "name": "pool_asset_{}".format(index + 1),
                "asset_path": asset_path,
                "location": [index * 150.0, 0.0, 0.0],
                "rotation": [0.0, 0.0, 0.0],
                "scale": [1.0, 1.0, 1.0],
            }
        )
    return specs


def write_scene_assembly_manifest(
    level: str,
    arguments: dict[str, Any],
    request: dict[str, Any],
    import_result: dict[str, Any],
    spawned_artifacts: list[str],
) -> Path:
    output_dir = Path(arguments.get("output_dir") or "Saved/PoolMcpBridge/SceneAssembly")
    output_dir.mkdir(parents=True, exist_ok=True)
    manifest_path = output_dir / "{}-scene-assembly.json".format(slug(level))
    manifest = {
        "kind": "pool_unreal_scene_assembly",
        "level": level,
        "asset_paths": list(arguments.get("asset_paths") or []),
        "imported_artifacts": import_result.get("artifacts", []),
        "actors": normalize_actor_specs(arguments),
        "cameras": normalize_named_specs(arguments, "cameras", "camera"),
        "lights": normalize_named_specs(arguments, "lights", "lighting"),
        "world_origin": arguments.get("world_origin") or [0, 0, 0],
        "spawned_artifacts": spawned_artifacts,
        "unreal_execution": unreal is not None,
        "pool_action": request.get("pool_unreal_action", {}),
        "handoff": request.get("mcp_payload", {}).get("handoff", {}),
    }
    manifest_path.write_text(json.dumps(manifest, indent=2, ensure_ascii=False), encoding="utf-8")
    return manifest_path


def normalize_named_specs(arguments: dict[str, Any], list_key: str, fallback_key: str) -> list[dict[str, Any]]:
    value = arguments.get(list_key)
    if isinstance(value, list):
        return [item if isinstance(item, dict) else {"name": str(item)} for item in value]
    fallback = arguments.get(fallback_key)
    if fallback:
        return [{"name": str(fallback)}]
    return []


def vector_from(value: Any, default: tuple[float, float, float] = (0.0, 0.0, 0.0)) -> Any:
    if isinstance(value, list) and len(value) >= 3:
        return unreal.Vector(float(value[0]), float(value[1]), float(value[2]))
    if isinstance(value, dict):
        return unreal.Vector(
            float(value.get("x", default[0])),
            float(value.get("y", default[1])),
            float(value.get("z", default[2])),
        )
    return unreal.Vector(*default)


def rotator_from(value: Any, default: tuple[float, float, float] = (0.0, 0.0, 0.0)) -> Any:
    if isinstance(value, list) and len(value) >= 3:
        return unreal.Rotator(float(value[0]), float(value[1]), float(value[2]))
    if isinstance(value, dict):
        return unreal.Rotator(
            float(value.get("pitch", default[0])),
            float(value.get("yaw", default[1])),
            float(value.get("roll", default[2])),
        )
    return unreal.Rotator(*default)


def slug(value: str) -> str:
    parts = []
    for character in value.lower():
        parts.append(character if character.isalnum() else "-")
    return "-".join(part for part in "".join(parts).split("-") if part) or "scene"


def log(message: str) -> None:
    if unreal is not None:
        unreal.log("[PoolMcpBridge] {}".format(message))
    else:
        print("[PoolMcpBridge] {}".format(message))
