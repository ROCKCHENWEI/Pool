"""Autoload entrypoint for the Pool Unreal MCP Bridge plugin.

Unreal executes init_unreal.py from plugin Content/Python when the Python
Script Plugin is enabled. Set POOL_UNREAL_MCP_AUTOSTART=0 to disable autostart.
"""

import os

import pool_mcp_bridge


if os.environ.get("POOL_UNREAL_MCP_AUTOSTART", "1") != "0":
    pool_mcp_bridge.start_from_env()
