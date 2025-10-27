#!/usr/bin/env bash

set -e  # Exit on error

echo "🔨 Building ccline from source..."
cargo build --release

echo ""
echo "📦 Installing ccline..."

# Determine Claude config directory
if [ -n "$CLAUDE_CONFIG_DIR" ]; then
    CONFIG_DIR="$CLAUDE_CONFIG_DIR"
else
    CONFIG_DIR="$HOME/.claude"
fi

INSTALL_DIR="$CONFIG_DIR/ccline"
BINARY_SOURCE="target/release/ccometixline"
BINARY_DEST="$INSTALL_DIR/ccline"

# Create installation directory
mkdir -p "$INSTALL_DIR"
echo "✓ Created directory: $INSTALL_DIR"

# Copy and rename binary
cp "$BINARY_SOURCE" "$BINARY_DEST"
chmod +x "$BINARY_DEST"
echo "✓ Installed binary: $BINARY_DEST"

# Verify installation
VERSION=$("$BINARY_DEST" --version)
echo "✓ Verified installation: $VERSION"

# Detect shell and appropriate rc file
SHELL_NAME=$(basename "$SHELL")
if [ "$SHELL_NAME" = "zsh" ]; then
    RC_FILE="$HOME/.zshrc"
elif [ "$SHELL_NAME" = "bash" ]; then
    RC_FILE="$HOME/.bashrc"
else
    RC_FILE="$HOME/.profile"
fi

# Add to PATH if not already present
PATH_EXPORT="export PATH=\"$INSTALL_DIR:\$PATH\""
if grep -q "$INSTALL_DIR" "$RC_FILE" 2>/dev/null; then
    echo "✓ PATH already configured in $RC_FILE"
else
    echo "" >> "$RC_FILE"
    echo "# ccline - Claude Code statusline" >> "$RC_FILE"
    echo "$PATH_EXPORT" >> "$RC_FILE"
    echo "✓ Added to PATH in $RC_FILE"
fi

echo ""
echo "✅ Installation complete!"
echo ""
echo "To use ccline immediately, run:"
echo "  export PATH=\"$INSTALL_DIR:\$PATH\""
echo ""
echo "Or start a new shell session to automatically load it."
echo ""
echo "Verify with: ccline --version"
