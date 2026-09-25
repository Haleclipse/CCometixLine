#!/usr/bin/env bash
# Build from source and install as the Claude Code statusline.
#
# - Installs the binary to ~/.claude/ccline/ccline (the path Claude Code runs).
# - Links ~/.local/bin/ccline to it so the `ccline` command works from a shell.
# - Keeps a backup of the previous binary as ccline.bak.
#
# Usage: scripts/install.sh [--no-build]
set -euo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
INSTALL_DIR="${CCLINE_INSTALL_DIR:-$HOME/.claude/ccline}"
BIN_DIR="${CCLINE_BIN_DIR:-$HOME/.local/bin}"
TARGET="$INSTALL_DIR/ccline"
BUILT="$REPO_DIR/target/release/ccometixline"

if [[ "${1:-}" != "--no-build" ]]; then
  echo "Building release binary..."
  (cd "$REPO_DIR" && cargo build --release)
fi

[[ -x "$BUILT" ]] || { echo "Built binary not found at $BUILT" >&2; exit 1; }

mkdir -p "$INSTALL_DIR"
if [[ -e "$TARGET" ]]; then
  cp "$TARGET" "$TARGET.bak"
  # Remove first so an npm-created hard link is not overwritten in place
  rm -f "$TARGET"
fi
cp "$BUILT" "$TARGET"
chmod 755 "$TARGET"

mkdir -p "$BIN_DIR"
ln -sf "$TARGET" "$BIN_DIR/ccline"

echo "Installed $("$TARGET" --version) to $TARGET"
echo "Linked $BIN_DIR/ccline -> $TARGET"
if ! echo ":$PATH:" | grep -q ":$BIN_DIR:"; then
  echo "Note: $BIN_DIR is not on your PATH"
fi
