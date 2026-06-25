#!/usr/bin/env sh
set -eu

AGENT=""
VERSION="0.1.0"
VERSION_FROM_ARGS=0
BASE_URL="https://github.com/valkor-ai/loom/releases/latest/download"

while [ "$#" -gt 0 ]; do
  case "$1" in
    --agent)
      AGENT="${2:-}"
      shift 2
      ;;
    --version)
      VERSION="${2:-latest}"
      VERSION_FROM_ARGS=1
      shift 2
      ;;
    --base-url)
      BASE_URL="${2:-}"
      shift 2
      ;;
    *)
      echo "unknown option: $1" >&2
      exit 1
      ;;
  esac
done

if [ -z "$AGENT" ]; then
  echo "--agent is required: codex, claude-code, opencode, or all" >&2
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
PACKAGE="loom-${VERSION}-${PLATFORM}.tar.gz"
if [ "$VERSION_FROM_ARGS" = "1" ] || [ "${LOOM_INSTALL_USE_VERSIONED_URL:-0}" = "1" ]; then
  BASE_URL="https://github.com/valkor-ai/loom/releases/download/v${VERSION}"
fi

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT
ARCHIVE="$TMP_DIR/$PACKAGE"

curl -fsSL "$BASE_URL/$PACKAGE" -o "$ARCHIVE"
tar -xzf "$ARCHIVE" -C "$TMP_DIR"
PACKAGE_ROOT="$(find "$TMP_DIR" -maxdepth 1 -type d -name 'loom-*' | head -n 1)"

if [ -z "$PACKAGE_ROOT" ]; then
  echo "release package did not contain a loom-* directory" >&2
  exit 1
fi

"$PACKAGE_ROOT/bin/loom-setup" install --agent "$AGENT" --package-root "$PACKAGE_ROOT"
