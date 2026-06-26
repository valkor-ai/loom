#!/usr/bin/env sh
set -eu

AGENT=""
VERSION="0.1.0"
VERSION_FROM_ARGS=0
BASE_URL="https://github.com/valkor-ai/loom/releases/latest/download"
LOCAL_BUILD=0
REPO_ROOT=""

usage() {
  cat >&2 <<'EOF'
Usage:
  install.sh --agent codex|claude-code|opencode|all
  install.sh --agent codex|claude-code|opencode|all --local-build [--repo-root <path>]

Options:
  --agent        Target agent to install or upgrade.
  --version      Release package version to install. Defaults to the script release version.
  --base-url     Release asset base URL. Defaults to GitHub latest release assets.
  --local-build  Build and package the current repository, then install that package.
  --repo-root    Repository root for --local-build. Defaults to the current directory.
EOF
}

require_value() {
  if [ "$#" -lt 2 ] || [ -z "$2" ]; then
    echo "$1 requires a value" >&2
    exit 1
  fi
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --agent)
      require_value "$1" "${2:-}"
      AGENT="$2"
      shift 2
      ;;
    --version)
      require_value "$1" "${2:-}"
      VERSION="$2"
      VERSION_FROM_ARGS=1
      shift 2
      ;;
    --base-url)
      require_value "$1" "${2:-}"
      BASE_URL="$2"
      shift 2
      ;;
    --local-build)
      LOCAL_BUILD=1
      shift
      ;;
    --repo-root)
      require_value "$1" "${2:-}"
      REPO_ROOT="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown option: $1" >&2
      usage
      exit 1
      ;;
  esac
done

if [ -z "$AGENT" ]; then
  echo "--agent is required: codex, claude-code, opencode, or all" >&2
  usage
  exit 1
fi

OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCH="$(uname -m)"
case "$OS" in
  darwin) PLATFORM_OS="darwin" ;;
  linux) PLATFORM_OS="linux" ;;
  *) echo "unsupported OS: $OS" >&2; exit 1 ;;
esac
case "$ARCH" in
  arm64|aarch64) PLATFORM_ARCH="arm64" ;;
  x86_64|amd64) PLATFORM_ARCH="x64" ;;
  *) echo "unsupported architecture: $ARCH" >&2; exit 1 ;;
esac

PLATFORM="${PLATFORM_OS}-${PLATFORM_ARCH}"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

if [ "$LOCAL_BUILD" = "1" ]; then
  if [ -z "$REPO_ROOT" ]; then
    REPO_ROOT="$(pwd)"
  fi
  if [ ! -f "$REPO_ROOT/src/rust/Cargo.toml" ]; then
    echo "--local-build requires a Loom repository root with src/rust/Cargo.toml: $REPO_ROOT" >&2
    exit 1
  fi
  if ! command -v cargo >/dev/null 2>&1; then
    echo "--local-build requires cargo on PATH" >&2
    exit 1
  fi

  cargo build --release -p mcp-server -p setup --manifest-path "$REPO_ROOT/src/rust/Cargo.toml"
  PACKAGE_OUTPUT="$TMP_DIR/packages"
  mkdir -p "$PACKAGE_OUTPUT"
  "$REPO_ROOT/src/rust/target/release/loom-setup" package-layout \
    --output-dir "$PACKAGE_OUTPUT" \
    --platform "$PLATFORM" >/dev/null
  PACKAGE_ROOT="$(find "$PACKAGE_OUTPUT" -mindepth 1 -maxdepth 1 -type d -name 'loom-*' | sort | head -n 1)"
else
  PACKAGE="loom-${VERSION}-${PLATFORM}.tar.gz"
  if [ "$VERSION_FROM_ARGS" = "1" ] || [ "${LOOM_INSTALL_USE_VERSIONED_URL:-0}" = "1" ]; then
    BASE_URL="https://github.com/valkor-ai/loom/releases/download/v${VERSION}"
  fi

  ARCHIVE="$TMP_DIR/$PACKAGE"
  curl -fsSL "$BASE_URL/$PACKAGE" -o "$ARCHIVE"
  tar -xzf "$ARCHIVE" -C "$TMP_DIR"
  PACKAGE_ROOT="$(find "$TMP_DIR" -mindepth 1 -maxdepth 1 -type d -name 'loom-*' | sort | head -n 1)"
fi

if [ -z "$PACKAGE_ROOT" ]; then
  echo "package did not contain a loom-* directory" >&2
  exit 1
fi

"$PACKAGE_ROOT/bin/loom-setup" install --agent "$AGENT" --package-root "$PACKAGE_ROOT"
