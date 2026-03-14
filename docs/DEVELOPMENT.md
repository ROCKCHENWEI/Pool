# Pool Development Guide

This guide covers setting up a development environment and contributing to the Pool project.

## Table of Contents

- [Development Setup](#development-setup)
- [Project Structure](#project-structure)
- [Architecture Overview](#architecture-overview)
- [Building the Project](#building-the-project)
- [Running Tests](#running-tests)
- [Code Style Guidelines](#code-style-guidelines)
- [Contributing](#contributing)
- [Release Process](#release-process)

## Development Setup

### Prerequisites

- **Rust**: 1.70 or later
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  rustup update stable
  ```

- **Xcode**: 14.0 or later (macOS development)
  - Install from the App Store
  - Install Command Line Tools: `xcode-select --install`

- **SQLite**: 3.x
  ```bash
  brew install sqlite
  ```

- **Just** (task runner, optional but recommended):
  ```bash
  brew install just
  ```

### Clone and Setup

```bash
# Clone the repository
git clone https://github.com/pool/pool.git
cd pool

# Initialize submodules (if any)
git submodule update --init --recursive

# Install Rust dependencies
cd shared-core
cargo fetch

# Setup pre-commit hooks (optional)
cp scripts/pre-commit .git/hooks/
chmod +x .git/hooks/pre-commit
```

### IDE Setup

#### VS Code (Recommended)

1. Install recommended extensions:
   - rust-analyzer
   - CodeLLDB
   - Swift (for macOS development)

2. Workspace settings are in `.vscode/settings.json`

#### IntelliJ IDEA / CLion

1. Install the Rust plugin
2. Open the project root directory
3. Configure Rust toolchain in Settings

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
│   │   ├── openclaw/     # OpenClaw integration
│   │   └── optimization/ # Performance utilities
│   ├── tests/            # Integration tests
│   └── Cargo.toml
│
├── apps/
│   └── macos/            # macOS Swift application
│       ├── Sources/
│       │   ├── Pool/     # App entry point
│       │   └── PoolCore/ # SwiftUI views
│       └── Package.swift
│
├── docs/                 # Documentation
├── scripts/              # Build and release scripts
└── tests/                # End-to-end tests
```

## Architecture Overview

Pool follows a three-layer architecture:

```
┌─────────────────────────────────────────┐
│           P0 Timeline Layer             │
│  (Timeline management, orchestration)   │
├─────────────────────────────────────────┤
│          P1 Pool_node Layer             │
│   (Node-based processing, workflows)    │
├─────────────────────────────────────────┤
│          P2 V.I.S.C Layer               │
│  (Visual Intelligence Synthesis Core)   │
└─────────────────────────────────────────┘
```

### Core Components

#### Shared Core (Rust)

The `shared-core` library provides:

- **Data Models**: Project, Shot, Workflow, Node types
- **Database**: SQLite-based persistence
- **Engine**: Workflow execution engine
- **API Gateway**: Unified API interface
- **FFI**: Cross-language bindings

#### macOS App (Swift)

The macOS application provides:

- Native SwiftUI interface
- Timeline visualization
- Node editor
- Integration with shared-core via FFI

### Data Flow

```
User Input (Swift UI)
    ↓
FFI Layer (Swift ↔ Rust)
    ↓
Shared Core (Rust)
    ↓
┌──────────────┬──────────────┐
│   Database   │  API Layer   │
│   (SQLite)   │  (External)  │
└──────────────┴──────────────┘
```

## Building the Project

### Build Rust Core

```bash
cd shared-core

# Debug build
cargo build

# Release build (optimized)
cargo build --release

# Build with all features
cargo build --all-features
```

### Build macOS App

```bash
cd apps/macos

# Debug build
swift build

# Release build
swift build -c release

# Generate Xcode project (optional)
swift package generate-xcodeproj
```

### Build Everything

```bash
# Using the build script
./scripts/build.sh

# Or manually
cargo build --release --manifest-path shared-core/Cargo.toml
swift build -c release --package-path apps/macos
```

## Running Tests

### Unit Tests

```bash
cd shared-core

# Run all tests
cargo test

# Run specific test
cargo test test_project_creation

# Run tests with output
cargo test -- --nocapture

# Run tests in a specific module
cargo test --test models_test
```

### Integration Tests

```bash
# Run integration tests
cargo test --test integration_test

# Run end-to-end tests
cargo test --test e2e_test
```

### Test Coverage

```bash
# Install tarpaulin
cargo install cargo-tarpaulin

# Generate coverage report
cargo tarpaulin --out Html
```

## Code Style Guidelines

### Rust Code Style

Follow the standard Rust style guide:

```bash
# Format code
cargo fmt

# Check for issues
cargo clippy -- -D warnings
```

Key conventions:

- Use `snake_case` for functions and variables
- Use `PascalCase` for types and traits
- Document public APIs with doc comments (`///`)
- Keep functions focused and under 50 lines
- Use meaningful variable names

### Swift Code Style

Follow the Swift API Design Guidelines:

- Use `camelCase` for methods and properties
- Use `PascalCase` for types
- Prefer SwiftUI patterns and idioms
- Organize code with `// MARK: -` comments

### Git Commit Messages

Follow conventional commits:

```
type(scope): description

[optional body]

[optional footer]
```

Types:
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation
- `style`: Formatting
- `refactor`: Code restructuring
- `test`: Adding tests
- `chore`: Maintenance

Example:
```
feat(api): add Kling video generation support

- Implement KlingAdapter trait
- Add video generation configuration
- Handle API rate limiting

Closes #42
```

## Contributing

### Pull Request Process

1. **Fork the repository** and create a feature branch:
   ```bash
   git checkout -b feature/my-feature
   ```

2. **Make your changes** following the code style guidelines

3. **Add tests** for new functionality

4. **Update documentation** if needed

5. **Run tests and linting**:
   ```bash
   cargo test
   cargo clippy
   cargo fmt --check
   ```

6. **Commit your changes**:
   ```bash
   git commit -m "feat(scope): description"
   ```

7. **Push and create a PR**:
   ```bash
   git push origin feature/my-feature
   ```

8. **Address review feedback** promptly

### Code Review Guidelines

- Be respectful and constructive
- Focus on code, not the author
- Explain reasoning behind suggestions
- Use conventional comment markers:
  - `NOTE:` for information
  - `TODO:` for future work
  - `FIXME:` for issues to fix
  - `HACK:` for workarounds

## Release Process

### Version Numbering

Pool follows [Semantic Versioning](https://semver.org/):

- **MAJOR**: Breaking changes
- **MINOR**: New features, backward compatible
- **PATCH**: Bug fixes

### Creating a Release

1. **Update version** in all Cargo.toml files

2. **Update CHANGELOG.md**:
   ```markdown
   ## [0.2.0] - 2024-04-01

   ### Added
   - Feature description

   ### Changed
   - Change description

   ### Fixed
   - Bug fix description
   ```

3. **Create a release branch**:
   ```bash
   git checkout -b release/0.2.0
   ```

4. **Run release script**:
   ```bash
   ./scripts/release.sh 0.2.0
   ```

5. **Tag the release**:
   ```bash
   git tag -a v0.2.0 -m "Release 0.2.0"
   git push origin v0.2.0
   ```

6. **Create GitHub release** with release notes

### CI/CD

The project uses GitHub Actions for:

- Running tests on PR
- Building release artifacts
- Publishing documentation

## Getting Help

- **Documentation**: [docs/](./)
- **Issues**: [GitHub Issues](https://github.com/pool/pool/issues)
- **Discussions**: [GitHub Discussions](https://github.com/pool/pool/discussions)

---

Thank you for contributing to Pool!
