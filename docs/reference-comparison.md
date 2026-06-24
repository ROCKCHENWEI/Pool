# 参考项目对比与吸收映射

## ROCKCHENWEI/Pool

`ROCKCHENWEI/Pool` 作为主运行框架参考，价值集中在本地优先、Rust shared-core、Timeline、Node Engine、SQLite、Provider Gateway、OpenClaw/MCP 与桌面 FFI 方向。

| 参考设计 | Pool 当前吸收位置 |
| --- | --- |
| Rust shared-core | `shared-core/` workspace crate |
| P0 Timeline | `models/timeline.rs` 的 `Project/Shot/Segment/OutputTarget` |
| P1 Pool_node | `models/workflow.rs` 与 `engine/node_engine.rs` |
| P2 V.I.S.C | `providers/mod.rs`、`control/mod.rs`、`openclaw/mcp.rs` |
| SQLite schema | `db/schema.rs` |
| Provider Gateway | `ProviderAdapter`、`ProviderRegistry`、`default_provider_configs` |
| Provider 运行闭环 | `ProviderTaskRunner` |
| ComfyUI/Kling/OpenAI adapter 思路 | ComfyUI HTTP/WebSocket adapter 已实现；Kling HTTP/JWT adapter 骨架已实现；OpenAI Images API adapter 骨架已实现 |
| OpenClaw/MCP resource | `pool://status`、`pool://projects`、`pool://tasks`、`pool://assets` |
| 桌面端桥接方向 | `shared-core` 导出稳定模型，后续供 SwiftUI/Web runtime 读取 |

结论：Pool 当前运行框架以 ROCKCHENWEI/Pool 为主基线，不再继续把浏览器 `localStorage` 原型作为长期核心。

## neilsonnn/image-blaster

`image-blaster` 作为 2D 到 3D/3DGS 资产生成流程参考，价值集中在本地资产 envelope、indexed file、Provider provenance 和高成本步骤人工确认。

| 参考设计 | Pool 当前吸收位置 |
| --- | --- |
| `worlds/<slug>` 项目包 | `ProjectEnvelope::for_slug` |
| `source/` 与 `output/` | `ProjectEnvelope.source_dir`、`ProjectEnvelope.output_dir` |
| `N-slug.ext` indexed files | `assets/indexed.rs` |
| `.N-slug-request.json` metadata | `assets/indexed.rs` 与 `Mock3dgsProvider` |
| Provider URL 只做 provenance | `AssetRecord.provider_url` 与 docs 中的本地加载原则 |
| 高成本生成前确认 | `RuntimeTask::with_approval_gate`、`TaskQueue::approve`、`NodeStatus::WaitingApproval` |
| 图片到 3D 世界流程 | `build_default_content_burst_plan` 的 AI image -> 3DGS -> asset package -> Unreal |

结论：Pool 不直接合并 image-blaster 代码，而是吸收其资产和生成流程契约，用于 3DGS/媒体资产落地规范。

## 当前本地原型

当前中文 Web 原型保留为控制台体验参考，迁移到 `apps/web-prototype/`。

| 原型价值 | Pool 当前吸收位置 |
| --- | --- |
| 中文“5 人超级团队内容爆发工具”定位 | README 与 Web prototype |
| 节点化流程图 UI | `apps/web-prototype/` 与 `Workflow` 模型 |
| 运行中心/API 接入/Hermes/Agent CLI | `apps/web-prototype/`、`ProviderRegistry`、`SoftwareAdapterRegistry` |
| 外部软件矩阵 | `default_software_adapters` |
| 视频/游戏/交互艺术三类输出 | `OutputTarget`、`PoolRuntimePlan` 与 Web prototype |
