#!/bin/bash

# Pool Release Script
# Automates the release process for Pool

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

# Default values
VERSION=""
DRY_RUN=false
SKIP_TESTS=false
SKIP_BUILD=false

usage() {
    echo "Usage: $0 VERSION [OPTIONS]"
    echo ""
    echo "Arguments:"
    echo "  VERSION            Version number (e.g., 0.2.0)"
    echo ""
    echo "Options:"
    echo "  -n, --dry-run      Show what would be done without making changes"
    echo "  --skip-tests       Skip running tests"
    echo "  --skip-build       Skip building"
    echo "  -h, --help         Show this help message"
    echo ""
    echo "Examples:"
    echo "  $0 0.2.0           Create release 0.2.0"
    echo "  $0 0.2.0 --dry-run Preview release 0.2.0"
}

# Parse arguments
if [[ $# -eq 0 ]]; then
    usage
    exit 1
fi

while [[ $# -gt 0 ]]; do
    case $1 in
        -n|--dry-run)
            DRY_RUN=true
            shift
            ;;
        --skip-tests)
            SKIP_TESTS=true
            shift
            ;;
        --skip-build)
            SKIP_BUILD=true
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            if [[ -z "$VERSION" ]]; then
                # Validate version format
                if [[ ! $1 =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
                    echo -e "${RED}Error: Invalid version format. Use X.Y.Z (e.g., 0.2.0)${NC}"
                    exit 1
                fi
                VERSION=$1
            else
                echo -e "${RED}Error: Unexpected argument: $1${NC}"
                usage
                exit 1
            fi
            shift
            ;;
    esac
done

if [[ -z "$VERSION" ]]; then
    echo -e "${RED}Error: Version number is required${NC}"
    usage
    exit 1
fi

echo -e "${BLUE}=====================================${NC}"
echo -e "${BLUE}       Pool Release Script          ${NC}"
echo -e "${BLUE}=====================================${NC}"
echo ""
echo -e "Version: ${YELLOW}${VERSION}${NC}"
echo -e "Dry run: ${YELLOW}${DRY_RUN}${NC}"
echo ""

# Function to print section header
section() {
    echo ""
    echo -e "${GREEN}==>${NC} $1"
    echo ""
}

# Function to run command (respects dry-run)
run_cmd() {
    if [ "$DRY_RUN" = true ]; then
        echo "[DRY RUN] $@"
    else
        "$@"
    fi
}

# Get current version from Cargo.toml
get_current_version() {
    grep '^version = ' "$PROJECT_ROOT/shared-core/Cargo.toml" | sed 's/version = "\(.*\)"/\1/'
}

CURRENT_VERSION=$(get_current_version)

section "Current version: ${CURRENT_VERSION}"
section "Target version: ${VERSION}"

# Check for uncommitted changes
if [ "$DRY_RUN" = false ]; then
    section "Checking for uncommitted changes"
    if ! git diff-index --quiet HEAD --; then
        echo -e "${RED}Error: You have uncommitted changes. Please commit or stash them first.${NC}"
        exit 1
    fi
    echo -e "${GREEN}✓${NC} Working directory is clean"
fi

# Create release branch
BRANCH_NAME="release/v${VERSION}"
section "Creating release branch: ${BRANCH_NAME}"

if [ "$DRY_RUN" = false ]; then
    if git show-ref --verify --quiet "refs/heads/${BRANCH_NAME}"; then
        echo -e "${YELLOW}Branch ${BRANCH_NAME} already exists. Checking out...${NC}"
        git checkout "$BRANCH_NAME"
    else
        git checkout -b "$BRANCH_NAME"
    fi
fi

# Update version numbers
section "Updating version numbers"

update_version() {
    local file=$1
    local pattern=$2

    if [ -f "$file" ]; then
        echo "Updating $file..."
        if [ "$DRY_RUN" = true ]; then
            echo "[DRY RUN] sed -i '' \"s/${pattern}/${VERSION}/g\" $file"
        else
            sed -i '' "s/${pattern}/${VERSION}/g" "$file"
        fi
    fi
}

# Update Cargo.toml
update_version "$PROJECT_ROOT/shared-core/Cargo.toml" "version = \"[0-9]*\.[0-9]*\.[0-9]*\""

# Update Package.swift
update_version "$PROJECT_ROOT/apps/macos/Package.swift" "version: \"[0-9]*\.[0-9]*\.[0-9]*\""

# Update CHANGELOG.md
section "Updating CHANGELOG.md"

if [ "$DRY_RUN" = true ]; then
    echo "[DRY RUN] Would add release notes to CHANGELOG.md"
else
    # Add release date to changelog
    if grep -q "## \[${VERSION}\]" "$PROJECT_ROOT/CHANGELOG.md"; then
        # Update unreleased section
        sed -i '' "s/## \[${VERSION}\] - Unreleased/## [${VERSION}] - $(date +%Y-%m-%d)/" "$PROJECT_ROOT/CHANGELOG.md"
    fi
fi

# Run tests
if [ "$SKIP_TESTS" = false ]; then
    section "Running tests"

    echo "Running Rust tests..."
    if [ "$DRY_RUN" = true ]; then
        echo "[DRY RUN] cargo test --manifest-path $PROJECT_ROOT/shared-core/Cargo.toml"
    else
        cd "$PROJECT_ROOT/shared-core"
        cargo test --quiet || {
            echo -e "${RED}Error: Tests failed${NC}"
            exit 1
        }
    fi
    echo -e "${GREEN}✓${NC} Tests passed"
fi

# Build release
if [ "$SKIP_BUILD" = false ]; then
    section "Building release"

    if [ "$DRY_RUN" = true ]; then
        echo "[DRY RUN] $SCRIPT_DIR/build.sh --release"
    else
        "$SCRIPT_DIR/build.sh" --release || {
            echo -e "${RED}Error: Build failed${NC}"
            exit 1
        }
    fi
    echo -e "${GREEN}✓${NC} Build completed"
fi

# Commit changes
section "Committing release changes"

if [ "$DRY_RUN" = true ]; then
    echo "[DRY RUN] git add -A"
    echo "[DRY RUN] git commit -m \"chore: release v${VERSION}\""
else
    cd "$PROJECT_ROOT"
    git add -A
    git commit -m "chore: release v${VERSION}

- Update version to ${VERSION}
- Update CHANGELOG.md
- Prepare release artifacts

Co-Authored-By: Pool Release Script <release@pool.dev>"
fi

# Create tag
section "Creating git tag"

if [ "$DRY_RUN" = true ]; then
    echo "[DRY RUN] git tag -a v${VERSION} -m \"Release v${VERSION}\""
else
    git tag -a "v${VERSION}" -m "Release ${VERSION}"
fi

# Summary
section "Release Summary"

echo -e "${GREEN}Release v${VERSION} prepared successfully!${NC}"
echo ""

if [ "$DRY_RUN" = true ]; then
    echo -e "${YELLOW}This was a dry run. No changes were made.${NC}"
else
    echo "Next steps:"
    echo "  1. Review the changes in the release branch"
    echo "  2. Push the branch: git push origin ${BRANCH_NAME}"
    echo "  3. Push the tag: git push origin v${VERSION}"
    echo "  4. Create a GitHub release from the tag"
    echo "  5. Merge the release branch to main"
fi

echo ""
echo -e "${BLUE}=====================================${NC}"
echo -e "${GREEN}Done!${NC}"
