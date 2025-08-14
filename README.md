# CCometixLine

[English](README.md) | [中文](README.zh.md)

A high-performance Claude Code statusline tool written in Rust with Git integration and real-time usage tracking.

![Language:Rust](https://img.shields.io/static/v1?label=Language&message=Rust&color=orange&style=flat-square)
![License:MIT](https://img.shields.io/static/v1?label=License&message=MIT&color=blue&style=flat-square)

## Screenshots

![CCometixLine](assets/img1.png)

The statusline shows: Model | Directory | Git Branch Status | Usage | Cost Statistics | Burn Rate

## Features

- **High performance** with Rust native speed
- **Git integration** with branch, status, and tracking info  
- **Model display** with simplified Claude model names
- **Usage tracking** based on transcript analysis
- **Cost tracking** with session, daily, and billing block statistics
- **Burn rate monitoring** for real-time consumption patterns
- **Directory display** showing current workspace
- **Minimal design** using Nerd Font icons
- **Simple configuration** via command line options
- **Environment variable control** for feature customization

## Installation

### Quick Install (Recommended)

Install via npm (works on all platforms):

```bash
# Install globally
npm install -g @cometix/ccline

# Or using yarn
yarn global add @cometix/ccline

# Or using pnpm
pnpm add -g @cometix/ccline
```

Use npm mirror for faster download:
```bash
npm install -g @cometix/ccline --registry https://registry.npmmirror.com
```

After installation:
- ✅ Global command `ccline` is available everywhere  
- ✅ Automatically configured for Claude Code at `~/.claude/ccline/ccline`
- ✅ Ready to use immediately!

### Update

```bash
npm update -g @cometix/ccline
```

### Manual Installation

Alternatively, download from [Releases](https://github.com/Haleclipse/CCometixLine/releases):

#### Linux

#### Option 1: Dynamic Binary (Recommended)
```bash
mkdir -p ~/.claude/ccline
wget https://github.com/Haleclipse/CCometixLine/releases/latest/download/ccline-linux-x64.tar.gz
tar -xzf ccline-linux-x64.tar.gz
cp ccline ~/.claude/ccline/
chmod +x ~/.claude/ccline/ccline
```
*Requires: Ubuntu 22.04+, CentOS 9+, Debian 11+, RHEL 9+ (glibc 2.35+)*

#### Option 2: Static Binary (Universal Compatibility)
```bash
mkdir -p ~/.claude/ccline
wget https://github.com/Haleclipse/CCometixLine/releases/latest/download/ccline-linux-x64-static.tar.gz
tar -xzf ccline-linux-x64-static.tar.gz
cp ccline ~/.claude/ccline/
chmod +x ~/.claude/ccline/ccline
```
*Works on any Linux distribution (static, no dependencies)*

### macOS (Intel)

```bash  
mkdir -p ~/.claude/ccline
wget https://github.com/Haleclipse/CCometixLine/releases/latest/download/ccline-macos-x64.tar.gz
tar -xzf ccline-macos-x64.tar.gz
cp ccline ~/.claude/ccline/
chmod +x ~/.claude/ccline/ccline
```

### macOS (Apple Silicon)

```bash
mkdir -p ~/.claude/ccline  
wget https://github.com/Haleclipse/CCometixLine/releases/latest/download/ccline-macos-arm64.tar.gz
tar -xzf ccline-macos-arm64.tar.gz
cp ccline ~/.claude/ccline/
chmod +x ~/.claude/ccline/ccline
```

### Windows

```powershell
# Create directory and download
New-Item -ItemType Directory -Force -Path "$env:USERPROFILE\.claude\ccline"
Invoke-WebRequest -Uri "https://github.com/Haleclipse/CCometixLine/releases/latest/download/ccline-windows-x64.zip" -OutFile "ccline-windows-x64.zip"
Expand-Archive -Path "ccline-windows-x64.zip" -DestinationPath "."
Move-Item "ccline.exe" "$env:USERPROFILE\.claude\ccline\"
```

### Claude Code Configuration

Add to your Claude Code `settings.json`:

**Linux/macOS:**
```json
{
  "statusLine": {
    "type": "command", 
    "command": "~/.claude/ccline/ccline",
    "padding": 0
  }
}
```

**Windows:**
```json
{
  "statusLine": {
    "type": "command", 
    "command": "%USERPROFILE%\\.claude\\ccline\\ccline.exe",
    "padding": 0
  }
}
```

### Build from Source

```bash
git clone https://github.com/Haleclipse/CCometixLine.git
cd CCometixLine
cargo build --release

# Linux/macOS
mkdir -p ~/.claude/ccline
cp target/release/ccometixline ~/.claude/ccline/ccline
chmod +x ~/.claude/ccline/ccline

# Windows (PowerShell)
New-Item -ItemType Directory -Force -Path "$env:USERPROFILE\.claude\ccline"
copy target\release\ccometixline.exe "$env:USERPROFILE\.claude\ccline\ccline.exe"
```

## Usage

```bash
# Basic usage (displays all enabled segments)
ccline

# Show help
ccline --help

# Print default configuration  
ccline --print-config

# TUI configuration mode (planned)
ccline --configure

# Billing block management
ccline --set-block-start <time>    # Set billing block start time for today
ccline --clear-block-start          # Clear block start time override
ccline --show-block-status          # Show current block status
```

### Billing Block Synchronization

Solve the problem of billing blocks not syncing when switching between devices with the same account:

```bash
# Set block start time to 10am on device A
ccline --set-block-start 10

# Supported time formats:
ccline --set-block-start 10        # 10:00 (24-hour format)
ccline --set-block-start 10:30     # 10:30
ccline --set-block-start "10:30"   # With quotes works too

# View current settings
ccline --show-block-status

# Clear settings, restore automatic calculation
ccline --clear-block-start
```

## Default Segments

Displays: `Model | Directory | Git Branch Status | Usage | Cost Statistics | Burn Rate`

### Model Display

Shows simplified Claude model names:
- `claude-3-5-sonnet` → `Sonnet 3.5`
- `claude-4-sonnet` → `Sonnet 4`

### Directory Display

Shows current workspace directory with folder icon.

### Git Status Indicators

- Branch name with Nerd Font icon
- Status: `✓` Clean, `●` Dirty, `⚠` Conflicts  
- Remote tracking: `↑n` Ahead, `↓n` Behind

### Usage Display

Token usage percentage based on transcript analysis with context limit tracking.

### Cost Statistics

Real-time cost tracking with session, daily, and billing block information:
- **Session cost**: Cost for current Claude Code session
- **Daily total**: Total cost for today across all sessions
- **Billing blocks**: 5-hour billing periods with remaining time (supports manual sync)

#### Dynamic Billing Block Algorithm

Uses the same dual-condition triggering algorithm as ccusage:
- Automatically detects activity start time to create 5-hour billing blocks
- Starts new block when activity gap exceeds 5 hours
- Supports manual start time setting for multi-device synchronization

### Burn Rate Monitoring

Real-time token consumption rate with visual indicators:
- 🔥 High burn rate (>5000 tokens/min)
- ⚡ Medium burn rate (2000-5000 tokens/min)
- 📊 Normal burn rate (<2000 tokens/min)
- Shows cost per hour projection

## Environment Variables

### Cost Feature Control

- `CCLINE_DISABLE_COST=1` - Disable both cost statistics and burn rate monitoring
  - When set: Shows only core segments (Model | Directory | Git | Usage)
  - When unset: Shows all segments including cost tracking

### Performance Tuning

- `CCLINE_SHOW_TIMING=1` - Display performance timing information for debugging

## Configuration

Configuration support is planned for future releases. Currently uses sensible defaults for all segments.

## Performance

- **Startup time**: < 50ms (vs ~200ms for TypeScript equivalents)
- **Memory usage**: < 10MB (vs ~25MB for Node.js tools)
- **Binary size**: ~2MB optimized release build

## Requirements

- **Git**: Version 1.5+ (Git 2.22+ recommended for better branch detection)
- **Terminal**: Must support Nerd Fonts for proper icon display
  - Install a [Nerd Font](https://www.nerdfonts.com/) (e.g., FiraCode Nerd Font, JetBrains Mono Nerd Font)
  - Configure your terminal to use the Nerd Font
- **Claude Code**: For statusline integration

## Development

```bash
# Build development version
cargo build

# Run tests
cargo test

# Build optimized release
cargo build --release
```

## Roadmap

- [ ] TOML configuration file support
- [ ] TUI configuration interface
- [ ] Custom themes
- [ ] Plugin system
- [ ] Cross-platform binaries

## Acknowledgments

### ccusage Integration

Cost tracking features are built upon the statistical methods and pricing data from the [ccusage](https://github.com/ryoppippi/ccusage) project.

## Contributing

Contributions are welcome! Please feel free to submit issues or pull requests.

## License

This project is licensed under the [MIT License](LICENSE).

## Star History

[![Star History Chart](https://api.star-history.com/svg?repos=Haleclipse/CCometixLine&type=Date)](https://star-history.com/#Haleclipse/CCometixLine&Date)