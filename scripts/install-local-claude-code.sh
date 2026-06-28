#!/usr/bin/env sh
set -eu

SCRIPT_DIR="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
REPO_ROOT="$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)"

exec "$REPO_ROOT/install.sh" \
  --agent claude-code \
  --local-build \
  --repo-root "$REPO_ROOT"
