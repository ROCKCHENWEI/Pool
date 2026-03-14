# Pool System Architecture

This document describes the architecture of the Pool video generation engine.

## Table of Contents

- [Overview](#overview)
- [Layer Architecture](#layer-architecture)
- [Core Components](#core-components)
- [Data Flow](#data-flow)
- [Storage Architecture](#storage-architecture)
- [API Integration](#api-integration)
- [Performance Considerations](#performance-considerations)
- [Security Model](#security-model)

## Overview

Pool is a localized AI video generation engine that enables users to create professional videos using AI-powered tools. The system is designed to run primarily on local hardware while optionally integrating with cloud AI services.

### Design Principles

1. **Local-First**: Prioritize local computation and data storage
2. **Modularity**: Pluggable components for flexibility
3. **Performance**: Optimized for real-time interaction
4. **Privacy**: User data stays on their machine by default
5. **Extensibility**: Easy to add new AI backends and features

## Layer Architecture

Pool follows a three-layer architecture:

```
┌──────────────────────────────────────────────────────────────────┐
│                    P0 Timeline Layer                             │
│                                                                  │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐  │
│  │  Timeline   │  │   Shot      │  │     Orchestration       │  │
│  │   Editor    │  │  Management │  │       Engine            │  │
│  └─────────────┘  └─────────────┘  └─────────────────────────┘  │
│                                                                  │
├──────────────────────────────────────────────────────────────────┤
│                   P1 Pool_node Layer                             │
│                                                                  │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐  │
│  │    Node     │  │  Workflow   │  │      Connection         │  │
│  │   Editor    │  │   Engine    │  │       Manager           │  │
│  └─────────────┘  └─────────────┘  └─────────────────────────┘  │
│                                                                  │
├──────────────────────────────────────────────────────────────────┤
│                   P2 V.I.S.C Layer                               │
│                 (Visual Intelligence Synthesis Core)             │
│                                                                  │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐  │
│  │    AI       │  │  Embedding  │  │       Model             │  │
│  │  Providers  │  │    Store    │  │      Manager            │  │
│  └─────────────┘  └─────────────┘  └─────────────────────────┘  │
│                                                                  │
└──────────────────────────────────────────────────────────────────┘
```

### P0 Timeline Layer

The Timeline Layer handles high-level project organization:

- **Project Management**: Create, organize, and manage video projects
- **Shot Sequencing**: Arrange shots in temporal order
- **Playback Control**: Preview and scrub through content
- **Export Pipeline**: Generate final video output

### P1 Pool_node Layer

The Node Layer provides visual programming capabilities:

- **Node System**: Modular processing units
- **Workflow Definition**: Connect nodes to create processing pipelines
- **Execution Engine**: Run workflows with proper scheduling
- **Type System**: Strong typing for node connections

### P2 V.I.S.C Layer

The Visual Intelligence Synthesis Core handles AI operations:

- **AI Providers**: Abstraction over multiple AI services
- **Embedding Store**: Vector storage for semantic search
- **Model Management**: Local and remote model handling
- **Generation Pipeline**: Coordinate AI generation tasks

## Core Components

### Shared Core Library (Rust)

The `pool-core` library is the heart of the system:

```
shared-core/src/
├── models/          # Data models and types
│   ├── project.rs   # Project and metadata
│   ├── shot.rs      # Shot definitions
│   └── workflow.rs  # Workflow and nodes
│
├── db/              # Database layer
│   ├── schema.rs    # SQL schema definitions
│   └── repository.rs # Data access patterns
│
├── engine/          # Processing engine
│   ├── node_engine.rs   # Node execution
│   └── executor.rs      # Workflow runner
│
├── api/             # External APIs
│   ├── gateway.rs   # API orchestration
│   └── providers/   # AI service adapters
│
├── comfyui/         # ComfyUI integration
│   ├── client.rs    # WebSocket client
│   └── workflow.rs  # Workflow translation
│
├── openclaw/        # OpenClaw features
│   ├── node_manager.rs
│   ├── embedding_store.rs
│   ├── mcp.rs
│   └── bots/
│
├── optimization/    # Performance utilities
│   ├── cache.rs     # LRU caching
│   └── async_executor.rs
│
└── ffi/             # Foreign function interface
    └── swift.rs     # macOS Swift bindings
```

### Native Applications

#### macOS App (Swift/SwiftUI)

```
apps/macos/
├── Sources/
│   ├── Pool/           # App lifecycle
│   │   └── main.swift
│   │
│   └── PoolCore/       # UI components
│       ├── ContentView.swift
│       ├── TimelineView.swift
│       ├── NodeEditorView.swift
│       ├── ModelsView.swift
│       └── SettingsViews.swift
│
└── Package.swift
```

## Data Flow

### Video Generation Flow

```
┌──────────┐     ┌──────────┐     ┌──────────┐     ┌──────────┐
│  User    │────▶│  Prompt  │────▶│   AI     │────▶│  Video   │
│  Input   │     │ Process  │     │ Generator│     │  Output  │
└──────────┘     └──────────┘     └──────────┘     └──────────┘
     │                │                │                │
     │                │                │                │
     ▼                ▼                ▼                ▼
┌──────────┐     ┌──────────┐     ┌──────────┐     ┌──────────┐
│  Create  │     │  Parse & │     │  Kling/  │     │  Export  │
│  Shot    │     │  Enhance │     │  ComfyUI │     │  MP4/MOV │
└──────────┘     └──────────┘     └──────────┘     └──────────┘
```

### Workflow Execution Flow

```
┌──────────────────────────────────────────────────────────────┐
│                    Workflow Executor                         │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│  1. Validate workflow graph                                  │
│  2. Topological sort for execution order                     │
│  3. Execute nodes in parallel where possible                 │
│  4. Handle errors and retries                                │
│  5. Collect and merge results                                │
│                                                              │
└──────────────────────────────────────────────────────────────┘
         │                    │                    │
         ▼                    ▼                    ▼
    ┌─────────┐         ┌─────────┐         ┌─────────┐
    │ Input   │         │ Process │         │ Output  │
    │  Node   │────────▶│  Node   │────────▶│  Node   │
    └─────────┘         └─────────┘         └─────────┘
```

## Storage Architecture

### Database Schema

```sql
-- Projects table
CREATE TABLE projects (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    settings TEXT,  -- JSON
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- Shots table
CREATE TABLE shots (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    sequence INTEGER NOT NULL,
    prompt TEXT NOT NULL,
    negative_prompt TEXT,
    duration REAL NOT NULL,
    status TEXT NOT NULL,
    result_url TEXT,
    metadata TEXT,  -- JSON
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (project_id) REFERENCES projects(id)
);

-- Workflows table
CREATE TABLE workflows (
    id TEXT PRIMARY KEY,
    project_id TEXT,
    name TEXT NOT NULL,
    nodes TEXT NOT NULL,  -- JSON
    connections TEXT NOT NULL,  -- JSON
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

### File Storage

```
~/.pool/
├── data/
│   └── pool.db           # SQLite database
│
├── projects/
│   └── {project-id}/
│       ├── media/        # Generated videos/images
│       └── cache/        # Project-specific cache
│
├── models/
│   ├── checkpoints/      # Base models
│   ├── lora/            # LoRA adapters
│   ├── embeddings/      # Textual inversions
│   └── vae/             # VAE models
│
└── cache/
    ├── embeddings/      # Cached embeddings
    └── responses/       # Cached API responses
```

## API Integration

### Supported AI Services

| Service | Type | Use Case |
|---------|------|----------|
| Kling AI | Cloud | Video generation |
| ComfyUI | Local | Image processing |
| OpenAI | Cloud | Text enhancement |

### API Gateway Pattern

```rust
pub trait VideoGeneratorAdapter: Send + Sync {
    async fn generate(&self, config: VideoGenerationConfig) -> Result<GenerationResult>;
    async fn get_status(&self, task_id: &str) -> Result<TaskStatus>;
    async fn cancel(&self, task_id: &str) -> Result<()>;
}

pub struct ApiGateway {
    adapters: HashMap<String, Box<dyn VideoGeneratorAdapter>>,
    default_adapter: String,
}
```

### Rate Limiting and Queuing

```
┌─────────────────────────────────────────────────┐
│               Task Queue                        │
├─────────────────────────────────────────────────┤
│  Priority Queue (by task priority)              │
│  ├── Critical tasks                            │
│  ├── High priority                             │
│  ├── Normal priority                           │
│  └── Low priority (background)                 │
├─────────────────────────────────────────────────┤
│  Rate Limiter (per-provider)                    │
│  └── Token bucket implementation               │
└─────────────────────────────────────────────────┘
```

## Performance Considerations

### Caching Strategy

```
┌─────────────────────────────────────────────────────────────┐
│                    Cache Hierarchy                          │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐    │
│  │   Memory    │    │    Disk     │    │   Remote    │    │
│  │    Cache    │───▶│    Cache    │───▶│    Cache    │    │
│  │   (LRU)     │    │   (SQLite)  │    │  (optional) │    │
│  └─────────────┘    └─────────────┘    └─────────────┘    │
│                                                             │
│  Embeddings: ✓        API Responses: ✓    Models: ✗       │
│  Node Results: ✓      Prompts: ✓          Videos: ✗       │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### Concurrent Task Execution

```rust
pub struct AsyncExecutor {
    max_concurrent: usize,        // Max parallel tasks
    semaphore: Arc<Semaphore>,    // Concurrency control
    priority_queues: [VecDeque<Task>; 4],  // Priority levels
}
```

### Memory Management

- Lazy loading of models
- Streaming for large files
- Reference counting for shared resources
- Periodic garbage collection for cache

## Security Model

### Data Security

- API keys stored in system keychain (macOS Keychain, Windows Credential Manager)
- Local database encrypted at rest (SQLCipher optional)
- No telemetry without explicit opt-in

### API Security

- HTTPS for all external API calls
- API key rotation support
- Request signing for sensitive operations

### Isolation

- Sandboxed execution for external tools
- Process isolation for ComfyUI integration
- Input validation and sanitization

---

**Version**: 0.1.0
**Last Updated**: March 2024
