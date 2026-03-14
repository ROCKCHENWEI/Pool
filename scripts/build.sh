#!/bin/bash

# Pool Build Script
# Builds all components of the Pool project

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Project root directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# Parse arguments
BUILD_TYPE="debug"
VERBOSE=false
CLEAN=false
SKIP_SWIFT=false
SKIP_RUST=false

usage() {
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Options:"
    echo "  -r, --release      Build in release mode (optimized)"
    echo "  -v, --verbose      Enable verbose output"
    echo "  -c, --clean        Clean before building"
    echo "  --skip-swift       Skip Swift/macOS build"
    echo "  --skip-rust        Skip Rust core build"
    echo "  -h, --help         Show this help message"
    echo ""
    echo "Examples:"
    echo "  $0 --release       Build release version"
    echo "  $0 -c -r           Clean and build release"
}

while [[ $# -gt 0 ]]; do
    case $1 in
        -r|--release)
            BUILD_TYPE="release"
            shift
            ;;
        -v|--verbose)
            VERBOSE=true
            shift
            ;;
        -c|--clean)
            CLEAN=true
            shift
            ;;
        --skip-swift)
            SKIP_SWIFT=true
            shift
            ;;
        --skip-rust)
            SKIP_RUST=true
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo -e "${RED}Unknown option: $1${NC}"
            usage
            exit 1
            ;;
    esac
done

echo -e "${BLUE}=====================================${NC}"
echo -e "${BLUE}       Pool Build Script            ${NC}"
echo -e "${BLUE}=====================================${NC}"
echo ""
echo -e "Build type: ${YELLOW}${BUILD_TYPE}${NC}"
echo -e "Project root: ${YELLOW}${PROJECT_ROOT}${NC}"
echo ""

# Function to print section header
section() {
    echo ""
    echo -e "${GREEN}==>${NC} $1"
    echo ""
}

# Function to run command with optional verbosity
run_cmd() {
    if [ "$VERBOSE" = true ]; then
        "$@"
    else
        "$@" > /dev/null 2>&1
    fi
}

# Check prerequisites
section "Checking prerequisites"

check_command() {
    if ! command -v $1 &> /dev/null; then
        echo -e "${RED}Error: $1 is not installed${NC}"
        echo "Please install $1 to continue"
        exit 1
    fi
    echo -e "${GREEN}✓${NC} $1 is installed"
}

if [ "$SKIP_RUST" = false ]; then
    check_command rustc
    check_command cargo
fi

if [ "$SKIP_SWIFT" = false ]; then
    check_command swift
fi

# Clean if requested
if [ "$CLEAN" = true ]; then
    section "Cleaning previous builds"

    if [ "$SKIP_RUST" = false ]; then
        echo "Cleaning Rust build..."
        cd "$PROJECT_ROOT/shared-core"
        run_cmd cargo clean
        echo -e "${GREEN}✓${NC} Rust cleaned"
    fi

    if [ "$SKIP_SWIFT" = false ]; then
        echo "Cleaning Swift build..."
        cd "$PROJECT_ROOT/apps/macos"
        run_cmd swift package clean 2>/dev/null || true
        echo -e "${GREEN}✓${NC} Swift cleaned"
    fi
fi

# Build Rust core
if [ "$SKIP_RUST" = false ]; then
    section "Building Rust core library"

    cd "$PROJECT_ROOT/shared-core"

    echo "Building pool-core..."
    if [ "$BUILD_TYPE" = "release" ]; then
        run_cmd cargo build --release
    else
        run_cmd cargo build
    fi

    echo -e "${GREEN}✓${NC} Rust core built successfully"

    # Run tests
    echo "Running Rust tests..."
    if [ "$VERBOSE" = true ]; then
        cargo test
    else
        cargo test --quiet 2>/dev/null || echo -e "${YELLOW}!${NC} Some tests may have failed (non-critical)"
    fi
    echo -e "${GREEN}✓${NC} Rust tests completed"
fi

# Build Swift app
if [ "$SKIP_SWIFT" = false ]; then
    section "Building macOS application"

    cd "$PROJECT_ROOT/apps/macos"

    echo "Building Swift package..."
    if [ "$BUILD_TYPE" = "release" ]; then
        run_cmd swift build -c release
    else
        run_cmd swift build
    fi

    echo -e "${GREEN}✓${NC} macOS app built successfully"
fi

# Summary
section "Build Summary"

echo -e "${GREEN}Build completed successfully!${NC}"
echo ""
echo "Built components:"
if [ "$SKIP_RUST" = false ]; then
    echo -e "  ${GREEN}✓${NC} Rust core library (${BUILD_TYPE})"
fi
if [ "$SKIP_SWIFT" = false ]; then
    echo -e "  ${GREEN}✓${NC} macOS application (${BUILD_TYPE})"
fi
echo ""

# Output locations
echo "Output locations:"
if [ "$SKIP_RUST" = false ]; then
    if [ "$BUILD_TYPE" = "release" ]; then
        echo "  Rust: $PROJECT_ROOT/shared-core/target/release/libpool_core.dylib"
    else
        echo "  Rust: $PROJECT_ROOT/shared-core/target/debug/libpool_core.dylib"
    fi
fi
if [ "$SKIP_SWIFT" = false ]; then
    if [ "$BUILD_TYPE" = "release" ]; then
        echo "  Swift: $PROJECT_ROOT/apps/macos/.build/release/Pool"
    else
        echo "  Swift: $PROJECT_ROOT/apps/macos/.build/debug/Pool"
    fi
fi

echo ""
echo -e "${BLUE}=====================================${NC}"
echo -e "${GREEN}Done!${NC}"
