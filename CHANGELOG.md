# Changelog

All notable changes to the Pool project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2024-03-15

### Added

#### Core Features
- Initial release of Pool video generation engine
- Rust shared core library (`pool-core`)
- SQLite database layer with async support
- Workflow execution engine with node-based processing

#### Data Models
- Project model with shots management
- Shot model with status tracking
- Workflow model with nodes and connections
- Node types: Input, Output, Processing, AI Generation

#### API Integration
- API Gateway for unified API access
- Kling AI adapter for cloud video generation
- ComfyUI client for local image generation
- Workflow translator for ComfyUI format

#### OpenClaw Features
- Node Manager for node registration and discovery
- Embedding Store for vector storage and search
- Feishu bot integration
- Telegram bot integration
- MCP (Model Context Protocol) support
- LLM Bar for language model interactions

#### Optimization
- LRU cache for embeddings and API responses
- Async task executor with priority scheduling
- Optimization manager for cache coordination

#### FFI Bindings
- Swift FFI bindings for macOS integration
- C-compatible interface for cross-platform support

#### UI (macOS)
- SwiftUI-based native application
- Timeline view for shot visualization
- Node editor for workflow creation
- Model manager interface
- Settings and preferences views

#### Documentation
- API documentation
- User guide
- Development guide
- Architecture documentation

### Security
- API keys stored in system keychain
- No telemetry without explicit consent

### Known Issues
- Windows support is planned but not yet implemented
- Some Swift views may require additional polish

---

## [Unreleased]

### Planned
- Windows platform support
- Additional AI backend integrations
- Plugin system
- Collaborative editing features
- Real-time preview

---

## Version History

| Version | Date | Description |
|---------|------|-------------|
| 0.1.0 | 2024-03-15 | Initial release |

---

## Development Phases

### Phase 1: Foundation (Completed)
- [x] Rust core library setup
- [x] Data models implementation
- [x] Database layer with SQLite
- [x] Workflow engine basics
- [x] API gateway structure
- [x] FFI bindings

### Phase 2: Core AI (Completed)
- [x] ComfyUI integration
- [x] Kling adapter
- [x] Basic workflow execution

### Phase 3: OpenClaw (Completed)
- [x] Node Manager
- [x] Embedding Store
- [x] Bot integrations (Feishu, Telegram)
- [x] MCP support
- [x] LLM Bar

### Phase 4: Optimization & Release (Current)
- [x] Performance optimization
- [x] UI/UX improvements
- [x] Documentation
- [x] Test coverage
- [x] Release preparation
