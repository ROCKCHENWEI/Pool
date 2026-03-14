# Pool API Documentation

This document describes the Rust API provided by the `pool-core` shared library.

## Table of Contents

- [Core Modules](#core-modules)
- [Data Models](#data-models)
- [Database Operations](#database-operations)
- [Engine API](#engine-api)
- [API Gateway](#api-gateway)
- [ComfyUI Integration](#comfyui-integration)
- [OpenClaw Module](#openclaw-module)
- [Optimization Module](#optimization-module)
- [FFI Bindings](#ffi-bindings)

## Core Modules

The library is organized into the following modules:

```rust
pub mod api;        // API gateway and providers
pub mod comfyui;    // ComfyUI integration
pub mod db;         // Database operations
pub mod engine;     // Workflow engine
pub mod ffi;        // Foreign Function Interface
pub mod models;     // Data models
pub mod openclaw;   // OpenClaw integration
pub mod optimization; // Performance optimization
```

## Data Models

### Project

```rust
pub struct Project {
    pub id: String,
    pub name: String,
    pub description: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub shots: Vec<Shot>,
}

impl Project {
    /// Create a new project
    pub fn new(name: String, description: String) -> Self;

    /// Add a shot to the project
    pub fn add_shot(&mut self, shot: Shot);

    /// Remove a shot by ID
    pub fn remove_shot(&mut self, shot_id: &str) -> Option<Shot>;

    /// Get total duration in seconds
    pub fn total_duration(&self) -> f64;
}
```

### Shot

```rust
pub struct Shot {
    pub id: String,
    pub project_id: String,
    pub sequence: i32,
    pub prompt: String,
    pub negative_prompt: Option<String>,
    pub duration: f64,
    pub status: ShotStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub enum ShotStatus {
    Draft,
    Pending,
    Processing,
    Completed,
    Failed,
}
```

### Workflow

```rust
pub struct Workflow {
    pub id: String,
    pub name: String,
    pub nodes: Vec<Node>,
    pub connections: Vec<Connection>,
}

pub struct Node {
    pub id: String,
    pub node_type: NodeType,
    pub position: (f64, f64),
    pub params: Vec<NodeParam>,
}

pub enum NodeType {
    Input,
    Output,
    Processing,
    AIGeneration,
}

pub struct Connection {
    pub id: String,
    pub from_node: String,
    pub from_output: String,
    pub to_node: String,
    pub to_input: String,
}
```

## Database Operations

### Database Connection

```rust
use pool_core::db::Database;

// Create database connection
let db = Database::new("pool.db").await?;

// Initialize schema
db.init_schema().await?;
```

### Project Repository

```rust
// Create a project
let project = Project::new("My Project".to_string(), "Description".to_string());
db.create_project(&project).await?;

// Get project by ID
let project = db.get_project("project-id").await?;

// List all projects
let projects = db.list_projects().await?;

// Update project
db.update_project(&project).await?;

// Delete project
db.delete_project("project-id").await?;
```

### Shot Repository

```rust
// Create a shot
let shot = Shot::new(project_id, sequence, prompt);
db.create_shot(&shot).await?;

// Get shots by project
let shots = db.get_shots_by_project("project-id").await?;

// Update shot status
db.update_shot_status("shot-id", ShotStatus::Completed).await?;
```

## Engine API

### Node Engine

```rust
use pool_core::engine::{NodeEngine, WorkflowExecutor};

// Create node engine
let engine = NodeEngine::new();

// Register node types
engine.register_node_type("ImageInput", image_input_factory);
engine.register_node_type("AIGenerate", ai_generate_factory);

// Validate workflow
let validation = engine.validate_workflow(&workflow)?;
```

### Workflow Executor

```rust
// Create executor
let executor = WorkflowExecutor::new(db);

// Execute workflow
let result = executor.execute(&workflow).await?;

// Execute with progress callback
executor.execute_with_progress(&workflow, |progress| {
    println!("Progress: {}%", progress.percent);
}).await?;
```

## API Gateway

### Creating API Gateway

```rust
use pool_core::api::{ApiGateway, VideoGenerationConfig};

let gateway = ApiGateway::new(config);

// Submit generation task
let task = gateway.submit_task(VideoGenerationConfig {
    prompt: "A beautiful sunset".to_string(),
    duration: 5.0,
    ..Default::default()
}).await?;

// Check task status
let status = gateway.get_task_status(&task.id).await?;
```

### Kling Adapter

```rust
use pool_core::api::KlingAdapter;

let adapter = KlingAdapter::new(api_key);

// Generate video
let result = adapter.generate_video(VideoGenerationConfig {
    prompt: "A cat playing piano".to_string(),
    duration: 5.0,
    ..Default::default()
}).await?;
```

## ComfyUI Integration

### ComfyUI Client

```rust
use pool_core::comfyui::ComfyUIClient;

// Connect to ComfyUI server
let client = ComfyUIClient::new("http://127.0.0.1:8188")?;

// Queue workflow
let task_id = client.queue_workflow(workflow_json).await?;

// Wait for completion
let result = client.wait_for_result(&task_id).await?;

// Get generated images
let images = client.get_images(&result).await?;
```

### Workflow Translator

```rust
use pool_core::comfyui::WorkflowTranslator;

let translator = WorkflowTranslator::new();

// Convert Pool workflow to ComfyUI format
let comfy_workflow = translator.to_comfyui(&pool_workflow)?;

// Convert ComfyUI workflow to Pool format
let pool_workflow = translator.from_comfyui(&comfy_workflow)?;
```

## OpenClaw Module

### Node Manager

```rust
use pool_core::openclaw::NodeManager;

let manager = NodeManager::new();

// Register node
manager.register_node(NodeDefinition {
    id: "custom-node".to_string(),
    name: "Custom Node".to_string(),
    inputs: vec!["input1".to_string()],
    outputs: vec!["output1".to_string()],
    ..Default::default()
})?;

// Get available nodes
let nodes = manager.list_nodes();
```

### Embedding Store

```rust
use pool_core::openclaw::EmbeddingStore;

let store = EmbeddingStore::new(db);

// Store embedding
store.store("text-id", embedding).await?;

// Search similar embeddings
let results = store.search(query_embedding, 10).await?;
```

## Optimization Module

### LRU Cache

```rust
use pool_core::optimization::{LruCache, CacheEntry};
use std::time::Duration;

// Create cache
let mut cache = LruCache::<String, CacheEntry>::new(
    100 * 1024 * 1024,  // 100MB max size
    Duration::from_secs(3600)  // 1 hour TTL
);

// Insert entry
cache.insert("key".to_string(), entry);

// Get entry
if let Some(entry) = cache.get(&"key".to_string()) {
    // Use entry
}

// Get statistics
let stats = cache.stats();
println!("Hit rate: {}%", stats.hit_rate());
```

### Async Executor

```rust
use pool_core::optimization::{AsyncExecutor, TaskPriority};

let executor = AsyncExecutor::new(10);  // Max 10 concurrent tasks

// Submit task
let handle = executor.submit(
    "my-task",
    TaskPriority::High,
    async move {
        // Task logic
        Ok(())
    }
)?;

// Check status
if handle.is_finished() {
    println!("Task completed!");
}
```

### Optimization Manager

```rust
use pool_core::optimization::{OptimizationManager, OptimizationConfig};

let manager = OptimizationManager::new(OptimizationConfig {
    max_cache_size: 200 * 1024 * 1024,  // 200MB
    cache_ttl: Duration::from_secs(1800),  // 30 minutes
    max_concurrent_tasks: 8,
    task_timeout: Duration::from_secs(300),  // 5 minutes
});

// Cache embedding
manager.store_embedding("text-key".to_string(), embedding);

// Retrieve cached embedding
if let Some(cached) = manager.get_embedding("text-key") {
    // Use cached embedding
}

// Get overall stats
let stats = manager.cache_stats();
```

## FFI Bindings

### Swift (macOS)

The FFI module provides C-compatible bindings for Swift integration:

```swift
// In Swift
import Foundation

// Pool Core FFI functions
@_silgen_name("pool_init")
func poolInit() -> UnsafeMutableRawPointer

@_silgen_name("pool_create_project")
func poolCreateProject(
    _ ctx: UnsafeMutableRawPointer,
    name: UnsafePointer<CChar>,
    description: UnsafePointer<CChar>
) -> UnsafeMutablePointer<CChar>

@_silgen_name("pool_free_string")
func poolFreeString(_ s: UnsafeMutablePointer<CChar>)
```

### Error Handling

```rust
use pool_core::{PoolError, Result};

// All functions return Result<T, PoolError>
match some_function().await {
    Ok(result) => { /* success */ },
    Err(PoolError::Database(msg)) => eprintln!("Database error: {}", msg),
    Err(PoolError::Api(msg)) => eprintln!("API error: {}", msg),
    Err(PoolError::Engine(msg)) => eprintln!("Engine error: {}", msg),
    Err(e) => eprintln!("Error: {}", e),
}
```

## Versioning

The API follows semantic versioning:

- **Major version**: Breaking changes
- **Minor version**: New features, backward compatible
- **Patch version**: Bug fixes

Current version: `0.1.0`
