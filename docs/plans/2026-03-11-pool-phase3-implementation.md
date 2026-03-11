# Pool Phase 3: openclaw 集成实现计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 实现 openclaw 统一管理层，包括 Node Manager、Embedding Store、外部通信 (飞书/Telegram) 和 MCP Server。

**Architecture:** openclaw 作为横切关注点，管理 P1/P2 节点的 CRUD 操作，存储角色/风格/场景嵌入向量，通过飞书和 Telegram Bot 提供远程控制，并通过 MCP Server 暴露 Resources 和 Tools 给外部 Agent。

**Tech Stack:** Rust (shared-core), SQLite (向量存储), reqwest (HTTP), tokio (async), serde (JSON)

---

## Phase 3 任务概览

| Task | 名称 | 主要功能 |
|------|------|----------|
| 10 | openclaw Node Manager | P1/P2 节点的 CRUD 操作、工作流组装 |
| 11 | Embedding Store | 角色/风格/场景嵌入向量存储与相似度搜索 |
| 12 | 飞书 Bot 集成 | 飞书机器人，支持远程命令交互 |
| 13 | Telegram Bot 集成 | Telegram 机器人，支持远程控制 |
| 14 | MCP Server 实现 | 暴露 Resources 和 Tools 给外部 Agent |
| 15 | LLM 操作栏实现 | 多 LLM 提供商适配，支持提示词增强 |

---

## Task 10: openclaw Node Manager

**Files:**
- Create: `pool/shared-core/src/openclaw/mod.rs`
- Create: `pool/shared-core/src/openclaw/node_manager.rs`
- Test: `pool/shared-core/tests/openclaw_test.rs`

**功能:**
- `create_node(node)` - 创建节点
- `get_node(id)` - 获取节点
- `update_node(node)` - 更新节点
- `delete_node(id)` - 删除节点
- `list_nodes()` - 列出所有节点
- `list_nodes_by_type(node_type)` - 按类型筛选
- `create_workflow(workflow)` - 创建工作流
- `execute_workflow(id)` - 执行工作流（使用 NodeEngine 拓扑排序）

**Commit:** `feat: add openclaw Node Manager for P1/P2 node management`

---

## Task 11: Embedding Store 向量存储

**Files:**
- Create: `pool/shared-core/src/openclaw/embedding_store.rs`
- Update: `pool/shared-core/tests/openclaw_test.rs`

**数据结构:**
```rust
pub enum EmbeddingType {
    Character,  // 角色嵌入
    Style,      // 风格嵌入
    Scene,      // 场景嵌入
}

pub struct Embedding {
    pub id: String,
    pub name: String,
    pub embedding_type: EmbeddingType,
    pub vector: Vec<f64>,
    pub metadata: HashMap<String, String>,
}
```

**功能:**
- `create_embedding(name, type, vector)` - 创建嵌入
- `get_embedding(id)` / `get_embedding_by_name(name)` - 获取嵌入
- `update_embedding(embedding)` - 更新嵌入
- `delete_embedding(id)` - 删除嵌入
- `list_all()` / `list_by_type(type)` - 列出嵌入
- `cosine_similarity(a, b)` - 计算余弦相似度
- `find_similar(vector, limit)` - 查找相似嵌入

**Commit:** `feat: add Embedding Store for character/style/scene vectors`

---

## Task 12: 飞书 Bot 集成

**Files:**
- Create: `pool/shared-core/src/openclaw/external_comm.rs`
- Create: `pool/shared-core/src/openclaw/feishu.rs`
- Test: `pool/shared-core/tests/feishu_test.rs`

**功能:**
- `FeishuBot::new(app_id, app_secret)` - 创建 Bot
- `get_tenant_token()` - 获取租户访问令牌
- `send_message(chat_id, content)` - 发送消息

**API:** `https://open.feishu.cn/open-apis`

**Commit:** `feat: add Feishu Bot integration for remote control`

---

## Task 13: Telegram Bot 集成

**Files:**
- Create: `pool/shared-core/src/openclaw/telegram.rs`
- Test: `pool/shared-core/tests/telegram_test.rs`

**功能:**
- `TelegramBot::new(token)` - 创建 Bot
- `send_message(chat_id, text)` - 发送消息
- `get_updates(offset, timeout)` - 获取更新（长轮询）

**API:** `https://api.telegram.org/bot{token}/{method}`

**Commit:** `feat: add Telegram Bot integration for remote control`

---

## Task 14: MCP Server 实现

**Files:**
- Create: `pool/shared-core/src/openclaw/mcp.rs`
- Test: `pool/shared-core/tests/mcp_test.rs`

**Resources:**
| URI | 名称 | 描述 |
|-----|------|------|
| `pool://status` | Pool Status | 当前引擎状态 |
| `pool://shots` | Shots List | 当前项目的镜头列表 |
| `pool://queue` | Render Queue | 渲染队列状态 |
| `pool://embeddings` | Embeddings | 可用的嵌入向量 |

**Tools:**
| 名称 | 描述 | 参数 |
|------|------|------|
| `pool_execute_shot` | 执行镜头工作流 | `shot_id` |
| `pool_create_shot` | 创建新镜头 | `name`, `prompt?` |
| `pool_batch_render` | 批量渲染 | `shot_ids[]` |

**Commit:** `feat: add MCP Server with resources and tools for external agents`

---

## Task 15: LLM 操作栏实现

**Files:**
- Create: `pool/shared-core/src/openclaw/llm_bar.rs`
- Test: `pool/shared-core/tests/llm_bar_test.rs`

**支持的 LLM 提供商:**
| Provider | API Base URL | Default Model |
|----------|--------------|---------------|
| Zhipu (智谱) | `https://open.bigmodel.cn/api/paas/v4` | `glm-4` |
| Minimax | `https://api.minimax.chat/v1` | `abab6.5-chat` |
| Kimi | `https://api.moonshot.cn/v1` | `moonshot-v1-8k` |
| Stepfun (阶跃) | `https://api.stepfun.com/v1` | `step-1-8k` |
| Gemini | `https://generativelanguage.googleapis.com/v1` | `gemini-pro` |
| Claude | `https://api.anthropic.com/v1` | `claude-3-sonnet-20240229` |

**功能:**
- `LlmBar::new(provider)` - 创建 LLM 操作栏
- `set_api_key(api_key)` - 设置 API Key
- `enhance_prompt(prompt)` - 增强提示词

**Commit:** `feat: add LLM Bar for prompt enhancement with multiple providers`

---

## 目录结构

完成 Phase 3 后的目录结构:

```
pool/shared-core/src/openclaw/
├── mod.rs              # 模块入口
├── node_manager.rs     # Task 10: 节点管理器
├── embedding_store.rs  # Task 11: 嵌入向量存储
├── external_comm.rs    # Task 12: 外部通信入口
├── feishu.rs           # Task 12: 飞书 Bot
├── telegram.rs         # Task 13: Telegram Bot
├── mcp.rs              # Task 14: MCP Server
└── llm_bar.rs          # Task 15: LLM 操作栏
```

---

## 执行选项

**1. Subagent-Driven (当前会话)** - 逐个任务执行，每个任务完成后进行审查

**2. Parallel Session (单独会话)** - 在新会话中批量执行

---

**文档版本:** v1.0
**创建日期:** 2026-03-11
**关联文档:** [设计文档](./2026-03-08-pool-design.md) | [PRD](./2026-03-08-pool-prd.md) | [Phase 1-2 实现](./2026-03-08-pool-implementation.md)
