# CCometixLine

[English](README.md) | [中文](README.zh.md)

A high-performance Claude Code statusline tool written in Rust with Git integration, usage tracking, interactive TUI configuration, and Claude Code enhancement utilities.

![Language:Rust](https://img.shields.io/static/v1?label=Language&message=Rust&color=orange&style=flat-square)
![License:MIT](https://img.shields.io/static/v1?label=License&message=MIT&color=blue&style=flat-square)

## Screenshots

![CCometixLine](assets/img1.png)

The statusline shows: Model | Directory | Git Branch Status | Context Window Information

## Features

### Core Functionality
- **Git integration** with branch, status, and tracking info  
- **Model display** with simplified Claude model names
- **Usage tracking** based on transcript analysis
- **Directory display** showing current workspace
- **Minimal design** using Nerd Font icons

### Interactive TUI Features
- **Interactive main menu** when executed without input
- **TUI configuration interface** with real-time preview
- **Theme system** with multiple built-in presets
- **Segment customization** with granular control
- **Configuration management** (init, check, edit)

### Claude Code Enhancement
- **Context warning disabler** - Remove annoying "Context low" messages
- **Verbose mode enabler** - Enhanced output detail
- **Robust patcher** - Survives Claude Code version updates
- **Automatic backups** - Safe modification with easy recovery

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
- ⚙️ Follow the configuration steps below to integrate with Claude Code
- 🎨 Run `ccline -c` to open configuration panel for theme selection

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

**Fallback (npm installation):**
```json
{
  "statusLine": {
    "type": "command", 
    "command": "ccline",
    "padding": 0
  }
}
```
*Use this if npm global installation is available in PATH*

### Update

```bash
npm update -g @cometix/ccline
```

<details>
<summary>Manual Installation (Click to expand)</summary>

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

#### macOS (Intel)

```bash  
mkdir -p ~/.claude/ccline
wget https://github.com/Haleclipse/CCometixLine/releases/latest/download/ccline-macos-x64.tar.gz
tar -xzf ccline-macos-x64.tar.gz
cp ccline ~/.claude/ccline/
chmod +x ~/.claude/ccline/ccline
```

#### macOS (Apple Silicon)

```bash
mkdir -p ~/.claude/ccline  
wget https://github.com/Haleclipse/CCometixLine/releases/latest/download/ccline-macos-arm64.tar.gz
tar -xzf ccline-macos-arm64.tar.gz
cp ccline ~/.claude/ccline/
chmod +x ~/.claude/ccline/ccline
```

#### Windows

```powershell
# Create directory and download
New-Item -ItemType Directory -Force -Path "$env:USERPROFILE\.claude\ccline"
Invoke-WebRequest -Uri "https://github.com/Haleclipse/CCometixLine/releases/latest/download/ccline-windows-x64.zip" -OutFile "ccline-windows-x64.zip"
Expand-Archive -Path "ccline-windows-x64.zip" -DestinationPath "."
Move-Item "ccline.exe" "$env:USERPROFILE\.claude\ccline\"
```

</details>

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

### Running a Forked Version

If you're running a forked version with the latest changes, you'll need to rebuild and reinstall after pulling updates:

```bash
# Navigate to your forked repository
cd /path/to/your/CCometixLine

# Pull latest changes
git pull

# Rebuild the binary
cargo build --release

# Install to local directory (recommended for testing)
# Linux/macOS
cp target/release/ccometixline ~/.claude/ccline/ccline

# Or install to system location (if using Homebrew path)
# macOS
cp target/release/ccometixline /opt/homebrew/bin/ccline

# Or install to system location
# Linux
sudo cp target/release/ccometixline /usr/local/bin/ccline

# Verify version
ccline --version
```

**After updating the binary**, if you've added new segments or themes:
1. Remove old theme cache: `rm -rf ~/.claude/ccline/themes`
2. Reinitialize if needed: `ccline --init`
3. Use TUI configurator to enable new segments: `ccline -c`

**Note**: The binary name in the repository is `ccometixline`, but it's renamed to `ccline` for convenience.

## Usage

### Configuration Management

```bash
# Initialize configuration file
ccline --init

# Check configuration validity  
ccline --check

# Print current configuration
ccline --print

# Enter TUI configuration mode
ccline --config
```

### Theme Override

```bash
# Temporarily use specific theme (overrides config file)
ccline --theme cometix
ccline --theme minimal
ccline --theme gruvbox
ccline --theme nord
ccline --theme powerline-dark

# Or use custom theme files from ~/.claude/ccline/themes/
ccline --theme my-custom-theme
```

### Claude Code Enhancement

```bash
# Disable context warnings and enable verbose mode
ccline --patch /path/to/claude-code/cli.js

# Example for common installation
ccline --patch ~/.local/share/fnm/node-versions/v24.4.1/installation/lib/node_modules/@anthropic-ai/claude-code/cli.js
```

## Default Segments

Displays: `Directory | Git Branch Status | Model | Context Window`

### Git Status Indicators

- Branch name with Nerd Font icon
- Status: `✓` Clean, `●` Dirty, `⚠` Conflicts  
- Remote tracking: `↑n` Ahead, `↓n` Behind

### Model Display

Shows simplified Claude model names:
- `claude-3-5-sonnet` → `Sonnet 3.5`
- `claude-4-sonnet` → `Sonnet 4`

### Context Window Display

Token usage percentage based on transcript analysis with context limit tracking.

### Usage Segments

Three usage tracking segments are available for monitoring Claude API usage:

**Usage (Original)** - Shows combined usage info:
- Displays 5-hour usage percentage
- Shows 7-day reset date in compact format
- Format: `24% · 10-7-2` (24% used, resets Oct 7 at 2am)

**Usage (5-hour)** - Focused 5-hour window:
- Shows 5-hour usage percentage with reset time
- Format: `24% → 11am`
- Ideal for monitoring short-term API limits

**Usage (7-day)** - Weekly usage tracking:
- Shows 7-day usage percentage with full reset datetime
- Format: `12% → Oct 9:5am`
- Perfect for tracking weekly quota

All usage segments:
- Share the same API call and cache (efficient)
- Use dynamic circle icons that change with utilization level
- Are disabled by default (enable via config or TUI)
- Auto-convert reset times from UTC to local timezone
- **Support threshold-based warning colors** (see below)

## Configuration

CCometixLine supports full configuration via TOML files and interactive TUI:

- **Configuration file**: `~/.claude/ccline/config.toml`
- **Interactive TUI**: `ccline --config` for real-time editing with preview
- **Theme files**: `~/.claude/ccline/themes/*.toml` for custom themes
- **Automatic initialization**: `ccline --init` creates default configuration

### Available Segments

All segments are configurable with:
- Enable/disable toggle
- Custom separators and icons
- Color customization
- Format options

Supported segments: Directory, Git, Model, ContextWindow, Usage, Usage5Hour, Usage7Day, Cost, Session, OutputStyle, Update

### Threshold-Based Warning Colors

Usage segments (Usage5Hour and Usage7Day) support dynamic color changes based on utilization thresholds. This allows you to get visual warnings when your API usage approaches limits.

**Configuration example:**

```toml
[[segments]]
id = "usage_5hour"
enabled = true

[segments.colors]
# Default colors (used when under warning threshold)
icon.c16 = 14  # Cyan
text.c16 = 14

[segments.options]
warning_threshold = 60    # Turn yellow at 60% usage
critical_threshold = 80   # Turn red at 80% usage
warning_color.c16 = 11    # Yellow (16-color palette)
critical_color.c16 = 9    # Red (16-color palette)
```

**How it works:**
- **< 60%**: Uses default segment colors (cyan)
- **≥ 60%**: Text changes to warning color (yellow)
- **≥ 80%**: Text changes to critical color (red)

You can customize thresholds and colors for each usage segment independently. The colors can be specified using:
- `c16`: 16-color ANSI palette (0-15)
- `c256`: 256-color palette (0-255)
- RGB values (e.g., `{r = 255, g = 165, b = 0}`)

**Common color codes:**
- Yellow: `c256 = 226` or `c16 = 11`
- Red: `c256 = 196` or `c16 = 9`
- Orange: `c256 = 208` or `c256 = 214`


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

- [x] TOML configuration file support
- [x] TUI configuration interface
- [x] Custom themes
- [x] Interactive main menu
- [x] Claude Code enhancement tools

## Contributing

Contributions are welcome! Please feel free to submit issues or pull requests.

## Related Projects

- [tweakcc](https://github.com/Piebald-AI/tweakcc) - Command-line tool to customize your Claude Code themes, thinking verbs, and more.

## License

This project is licensed under the [MIT License](LICENSE).

## Star History

[![Star History Chart](https://api.star-history.com/svg?repos=Haleclipse/CCometixLine&type=Date)](https://star-history.com/#Haleclipse/CCometixLine&Date)
