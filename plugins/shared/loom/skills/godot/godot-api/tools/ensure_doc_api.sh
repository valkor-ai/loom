#!/usr/bin/env bash
# Bootstrap doc_api for godot-api skill.
# Generates per-class markdown API reference in both GDScript and C# formats.
# Supports version-specific generation — docs are cached per Godot version.
#
# Usage:
#   bash ensure_doc_api.sh [version]
#
# Version detection (in priority order):
#   1. Explicit argument:  bash ensure_doc_api.sh 4.4
#   2. project.godot:      reads config/features from ./project.godot
#   3. Default:            falls back to "latest" (clones default branch)
#
# Output directories:
#   doc_api/{version}/          — GDScript API docs
#   doc_api_csharp/{version}/   — C# API docs
#   doc_source/                 — cloned Godot doc/classes (shared across versions)
#
# Safe to re-run — skips if target version's docs already exist.
set -euo pipefail

SKILL_DIR="$(cd "$(dirname "$0")/.." && pwd)"
TOOLS_DIR="$SKILL_DIR/tools"
DOC_SOURCE="$SKILL_DIR/doc_source"

# --- Version detection ---
VERSION="${1:-}"

if [ -z "$VERSION" ]; then
    # Try reading from project.godot in current working directory
    if [ -f "project.godot" ]; then
        # Extract version from config/features line, e.g.: config/features=PackedStringArray("4.4", "Forward Plus")
        VERSION=$(grep -oP 'config/features=.*?"(\d+\.\d+)' project.godot 2>/dev/null | grep -oP '\d+\.\d+' | head -1 || true)
    fi
fi

if [ -z "$VERSION" ]; then
    VERSION="latest"
    echo "No version specified and no project.godot found — using latest"
fi

echo "Target Godot version: $VERSION"

DOC_API="$SKILL_DIR/doc_api/$VERSION"
DOC_API_CSHARP="$SKILL_DIR/doc_api_csharp/$VERSION"

# Skip if both already exist for this version
if [ -d "$DOC_API" ] && [ -f "$DOC_API/_common.md" ] && \
   [ -d "$DOC_API_CSHARP" ] && [ -f "$DOC_API_CSHARP/_common.md" ]; then
    echo "doc_api already exists for version $VERSION — skipping"
    exit 0
fi

echo "Bootstrapping doc_api for version $VERSION..."

# --- Clone or update Godot docs ---
# Godot tags: 4.4-stable, 4.4.1-stable, etc.
# For "latest" we use --depth 1 on default branch.
# For specific versions, we clone the {version}-stable tag.
if [ "$VERSION" = "latest" ]; then
    GIT_REF=""
    CLONE_ARGS="--depth 1"
else
    GIT_REF="${VERSION}-stable"
    CLONE_ARGS="--depth 1 --branch $GIT_REF"
fi

# Each version gets its own source directory to avoid conflicts
VERSION_SOURCE="$DOC_SOURCE/$VERSION"

if [ ! -d "$VERSION_SOURCE/godot/doc/classes" ]; then
    mkdir -p "$VERSION_SOURCE"
    git clone $CLONE_ARGS --filter=blob:none --sparse \
        https://github.com/godotengine/godot.git "$VERSION_SOURCE/godot" || {
        echo "ERROR: Failed to clone Godot docs for version $VERSION."
        echo "Check that tag '$GIT_REF' exists at https://github.com/godotengine/godot/tags"
        exit 1
    }
    git -C "$VERSION_SOURCE/godot" sparse-checkout set doc/classes
fi

# --- Generate GDScript API docs ---
if [ ! -d "$DOC_API" ] || [ ! -f "$DOC_API/_common.md" ]; then
    echo "Generating GDScript API docs for $VERSION..."
    mkdir -p "$DOC_API"
    PYTHONPATH="$TOOLS_DIR" python3 "$TOOLS_DIR/godot_api_converter.py" \
        -i "$VERSION_SOURCE/godot/doc/classes" \
        --split-dir "$DOC_API" \
        --class-desc full \
        --method-desc full \
        --property-desc full \
        --signal-desc full \
        --constant-desc full \
        --include-virtual \
        --full-signals
fi

# --- Generate C# API docs ---
if [ ! -d "$DOC_API_CSHARP" ] || [ ! -f "$DOC_API_CSHARP/_common.md" ]; then
    echo "Generating C# API docs for $VERSION..."
    mkdir -p "$DOC_API_CSHARP"
    PYTHONPATH="$TOOLS_DIR" python3 "$TOOLS_DIR/godot_api_converter.py" \
        -i "$VERSION_SOURCE/godot/doc/classes" \
        --split-dir "$DOC_API_CSHARP" \
        --class-desc full \
        --method-desc full \
        --property-desc full \
        --signal-desc full \
        --constant-desc full \
        --include-virtual \
        --full-signals \
        --lang csharp
fi

echo "doc_api ready for version $VERSION:"
echo "  GDScript: $DOC_API"
echo "  C#:       $DOC_API_CSHARP"
