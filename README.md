# Pool

A localized AI video generation engine for creating professional videos with AI-powered tools.

## Overview

Pool is a desktop application that enables users to create AI-generated videos through an intuitive visual interface. It combines local computation with optional cloud AI services, prioritizing privacy and performance.

### Key Features

- **Timeline Editor**: Visual shot-based video editing
- **Node Editor**: Visual programming for AI workflows
- **Model Management**: Manage local AI models (checkpoints, LoRAs, embeddings)
- **Multi-Backend Support**: ComfyUI (local) and Kling AI (cloud)
- **Cross-Platform**: macOS (SwiftUI), Windows (planned)

## Architecture

Pool follows a three-layer architecture:

```
┌─────────────────────────────────────────┐
│           P0 Timeline Layer             │
│  Timeline management and orchestration  │
├─────────────────────────────────────────┤
│          P1 Pool_node Layer             │
│   Node-based processing, workflows      │
├─────────────────────────────────────────┤
│          P2 V.I.S.C Layer               │
│  Visual Intelligence Synthesis Core     │
└─────────────────────────────────────────┘
```

### Technology Stack

| Layer | Technology |
|-------|------------|
| Core Library | Rust |
| macOS App | Swift, SwiftUI |
| Database | SQLite |
| Local AI | ComfyUI |
| Cloud AI | Kling AI |

## Project Structure

```
pool/
├── shared-core/           # Rust shared library
│   ├── src/
│   │   ├── api/          # API gateway and providers
│   │   ├── comfyui/      # ComfyUI integration
│   │   ├── db/           # Database operations
│   │   ├── engine/       # Workflow engine
│   │   ├── ffi/          # FFI bindings
│   │   ├── models/       # Data models
│   │   ├── openclaw/     # OpenClaw features
│   │   └── optimization/ # Performance utilities
│   └── tests/            # Integration tests
│
├── apps/
│   └── macos/            # macOS application
│       └── Sources/PoolCore/
│           ├── ContentView.swift
│           ├── TimelineView.swift
│           └── NodeEditorView.swift
│
├── docs/                 # Documentation
│   ├── API.md
│   ├── USER_GUIDE.md
│   ├── DEVELOPMENT.md
│   └── ARCHITECTURE.md
│
├── scripts/              # Build and release scripts
└── tests/                # End-to-end tests
```

## Quick Start

### Prerequisites

- Rust 1.70 or later
- Xcode 14+ (macOS development)
- SQLite 3
- ComfyUI (optional, for local generation)

### Build

```bash
# Clone the repository
git clone https://github.com/pool/pool.git
cd pool

# Build the Rust core
cd shared-core
cargo build --release

# Build the macOS app
cd ../apps/macos
swift build -c release
```

### Run Tests

```bash
cd shared-core
cargo test
```

### Using the Build Script

```bash
./scripts/build.sh
```

## Usage

### Basic Workflow

1. **Create a Project**: Start a new video project
2. **Add Shots**: Define video segments with prompts
3. **Configure Generation**: Set style, quality, and duration
4. **Generate**: Run AI generation for each shot
5. **Export**: Render the final video

### Node Editor

Create custom workflows using the visual node editor:

```
[Image Input] ──▶ [Style Transfer] ──▶ [Upscale] ──▶ [Video Output]
```

## Documentation

- [API Documentation](docs/API.md) - Rust API reference
- [User Guide](docs/USER_GUIDE.md) - Installation and usage
- [Development Guide](docs/DEVELOPMENT.md) - Contributing
- [Architecture](docs/ARCHITECTURE.md) - System design

## Development Status

Pool is currently in early development (v0.1.0).

### Completed (Phase 1-3)

- [x] Rust core library with models and database
- [x] Workflow engine and node system
- [x] API gateway with Kling adapter
- [x] ComfyUI integration
- [x] OpenClaw features (Node Manager, Embedding Store, Bots, MCP, LLM Bar)
- [x] FFI bindings for Swift

### In Progress (Phase 4)

- [x] Performance optimization (caching, async executor)
- [x] UI/UX improvements
- [x] Documentation
- [ ] Test coverage improvement
- [ ] Release preparation

### Planned

- [ ] Windows support
- [ ] Additional AI backends
- [ ] Collaborative editing
- [ ] Plugin system

## Contributing

We welcome contributions! Please see [DEVELOPMENT.md](docs/DEVELOPMENT.md) for:

- Development setup instructions
- Code style guidelines
- Pull request process
- Release process

## License

[License information to be added]

## Acknowledgments

- [ComfyUI](https://github.com/comfyanonymous/ComfyUI) - Local AI image generation
- [Kling AI](https://kling.ai) - Cloud video generation API

---

**Version**: 0.1.0
**Status**: Early Development
