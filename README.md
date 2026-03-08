# Pool

A localized AI video generation engine.

## Architecture

Pool follows a three-layer architecture:

- **P0 Timeline** - Timeline management and orchestration
- **P1 Pool_node** - Node-based processing layer
- **P2 V.I.S.C** - Visual Intelligence Synthesis Core

## Shared Core

The `shared-core` library is written in Rust and provides:

- Data models and types
- Database operations (SQLite via SQLx)
- Engine logic
- API interfaces
- ComfyUI integration
- OpenClaw integration
- FFI bindings for cross-platform support

### Platforms

- **macOS** - Swift integration via FFI
- **Windows** - C# integration via FFI

## Getting Started

### Prerequisites

- Rust 1.70 or later
- SQLite 3

### Build

```bash
cd shared-core
cargo build
```

### Test

```bash
cd shared-core
cargo test
```

## Project Status

This project is in early development.
