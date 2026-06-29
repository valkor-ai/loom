#!/usr/bin/env sh
set -eu

AGENT=""
VERSION="0.1.0"
VERSION_FROM_ARGS=0
BASE_URL="https://github.com/valkor-ai/loom/releases/latest/download"
BASE_URL_FROM_ARGS=0
LOCAL_BUILD=0
PRINT_PLAN=0
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
  --print-plan   Print the resolved install plan as JSON without downloading or installing.
EOF
}

fail() {
  echo "loom install: $*" >&2
  exit 1
}

require_value() {
  if [ "$#" -lt 2 ] || [ -z "$2" ]; then
    fail "$1 requires a value"
  fi
}

json_escape() {
  printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g'
}

json_string() {
  printf '"%s"' "$(json_escape "$1")"
}

download_file() {
  URL="$1"
  OUT="$2"
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$URL" -o "$OUT"
  elif command -v wget >/dev/null 2>&1; then
    wget -qO "$OUT" "$URL"
  else
    fail "curl or wget is required to download release assets"
  fi
}

sha256_file() {
  FILE="$1"
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$FILE" | awk '{print tolower($1)}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$FILE" | awk '{print tolower($1)}'
  elif command -v openssl >/dev/null 2>&1; then
    openssl dgst -sha256 "$FILE" | awk '{print tolower($NF)}'
  else
    fail "sha256sum, shasum, or openssl is required to verify release assets"
  fi
}

verify_archive_checksum() {
  ARCHIVE="$1"
  CHECKSUM_FILE="$2"
  EXPECTED="$(grep -Eio '[a-f0-9]{64}' "$CHECKSUM_FILE" | head -n 1 | tr '[:upper:]' '[:lower:]')"
  if [ -z "$EXPECTED" ]; then
    fail "checksum file did not contain a SHA-256 digest: $CHECKSUM_FILE"
  fi
  ACTUAL="$(sha256_file "$ARCHIVE")"
  if [ "$EXPECTED" != "$ACTUAL" ]; then
    fail "archive checksum mismatch for $(basename "$ARCHIVE"): expected $EXPECTED, got $ACTUAL"
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
      BASE_URL_FROM_ARGS=1
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
    --print-plan)
      PRINT_PLAN=1
      shift
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

case "$AGENT" in
  codex|claude-code|opencode|all) ;;
  "")
    echo "--agent is required: codex, claude-code, opencode, or all" >&2
    usage
    exit 1
    ;;
  *) fail "unsupported agent '$AGENT', expected codex, claude-code, opencode, or all" ;;
esac

OS="${LOOM_INSTALL_TEST_OS:-$(uname -s)}"
ARCH="${LOOM_INSTALL_TEST_ARCH:-$(uname -m)}"
OS="$(printf '%s' "$OS" | tr '[:upper:]' '[:lower:]')"
ARCH="$(printf '%s' "$ARCH" | tr '[:upper:]' '[:lower:]')"
case "$OS" in
  darwin) PLATFORM_OS="darwin" ;;
  linux) PLATFORM_OS="linux" ;;
  *) fail "unsupported OS: $OS" ;;
esac
case "$ARCH" in
  arm64|aarch64) PLATFORM_ARCH="arm64" ;;
  x86_64|amd64) PLATFORM_ARCH="x64" ;;
  *) fail "unsupported architecture: $ARCH" ;;
esac

PLATFORM="${PLATFORM_OS}-${PLATFORM_ARCH}"
PACKAGE="loom-${VERSION}-${PLATFORM}.tar.gz"
if { [ "$VERSION_FROM_ARGS" = "1" ] || [ "${LOOM_INSTALL_USE_VERSIONED_URL:-0}" = "1" ]; } && [ "$BASE_URL_FROM_ARGS" = "0" ]; then
  BASE_URL="https://github.com/valkor-ai/loom/releases/download/v${VERSION}"
fi
PACKAGE_URL="$BASE_URL/$PACKAGE"
CHECKSUM_URL="$PACKAGE_URL.sha256"

if [ "$PRINT_PLAN" = "1" ]; then
  printf '{'
  printf '"agent":%s,' "$(json_string "$AGENT")"
  printf '"version":%s,' "$(json_string "$VERSION")"
  printf '"platform":%s,' "$(json_string "$PLATFORM")"
  printf '"localBuild":%s,' "$([ "$LOCAL_BUILD" = "1" ] && printf true || printf false)"
  printf '"package":%s,' "$(json_string "$PACKAGE")"
  printf '"packageUrl":%s,' "$(json_string "$PACKAGE_URL")"
  printf '"checksumUrl":%s,' "$(json_string "$CHECKSUM_URL")"
  printf '"archiveChecksumRequired":true'
  printf '}\n'
  exit 0
fi

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

if [ "$LOCAL_BUILD" = "1" ]; then
  if [ -z "$REPO_ROOT" ]; then
    REPO_ROOT="$(pwd)"
  fi
  if [ ! -f "$REPO_ROOT/src/rust/Cargo.toml" ]; then
    fail "--local-build requires a Loom repository root with src/rust/Cargo.toml: $REPO_ROOT"
  fi
  if ! command -v cargo >/dev/null 2>&1; then
    fail "--local-build requires cargo on PATH"
  fi

  cargo build --release -p mcp-server -p setup --manifest-path "$REPO_ROOT/src/rust/Cargo.toml"
  PACKAGE_OUTPUT="$TMP_DIR/packages"
  mkdir -p "$PACKAGE_OUTPUT"
  "$REPO_ROOT/src/rust/target/release/loom-setup" package-layout \
    --output-dir "$PACKAGE_OUTPUT" \
    --platform "$PLATFORM" >/dev/null
  PACKAGE_ROOT="$(find "$PACKAGE_OUTPUT" -mindepth 1 -maxdepth 1 -type d -name 'loom-*' | sort | head -n 1)"
else
  ARCHIVE="$TMP_DIR/$PACKAGE"
  CHECKSUM_FILE="$TMP_DIR/$PACKAGE.sha256"
  download_file "$PACKAGE_URL" "$ARCHIVE"
  download_file "$CHECKSUM_URL" "$CHECKSUM_FILE"
  verify_archive_checksum "$ARCHIVE" "$CHECKSUM_FILE"
  if ! command -v tar >/dev/null 2>&1; then
    fail "tar is required to extract $PACKAGE"
  fi
  tar -xzf "$ARCHIVE" -C "$TMP_DIR"
  PACKAGE_ROOT="$(find "$TMP_DIR" -mindepth 1 -maxdepth 1 -type d -name 'loom-*' | sort | head -n 1)"
fi

if [ -z "$PACKAGE_ROOT" ]; then
  fail "package did not contain a loom-* directory"
fi

SETUP="$PACKAGE_ROOT/bin/loom-setup"
"$SETUP" install --agent "$AGENT" --package-root "$PACKAGE_ROOT"
"$SETUP" doctor --agent "$AGENT" --package-root "$PACKAGE_ROOT"
