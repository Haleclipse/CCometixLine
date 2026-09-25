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
- **Model display** with simplified Claude model names and the active effort level
- **Usage tracking** with five-hour and weekly limits, reset times and extra-usage credits
- **Context window tracking** based on transcript analysis
- **Directory display** showing current workspace
- **Minimal design** using Nerd Font icons

### Interactive TUI Features
- **Interactive main menu** when executed without input
- **TUI configuration interface** with real-time preview
- **Theme system** with multiple built-in presets
- **Segment customization** with granular control, including per-segment options
- **Unsaved-changes prompt** when quitting the TUI
- **Configuration management** (init, check, edit, `--options` / `--set` / `--unset`)

### Claude Code Enhancement
- **Context warning disabler** - Remove annoying "Context low" messages
- **Verbose mode enabler** - Enhanced output detail
- **Robust patcher** - Survives Claude Code version updates
- **Automatic backups** - Safe modification with easy recovery

## Installation

### Install from source

```bash
git clone https://github.com/Haleclipse/CCometixLine.git
cd CCometixLine
scripts/install.sh
```

The script builds the release binary, installs it to `~/.claude/ccline/ccline`
(the path Claude Code runs, backing up the previous binary as `ccline.bak`) and
links `~/.local/bin/ccline` to it so the `ccline` command works from a shell.
Re-run it after pulling changes. Pass `--no-build` to reuse an existing
`target/release` build.

If `@cometix/ccline` is also installed from npm, its postinstall hard-links the
npm binary over `~/.claude/ccline/ccline`. Remove it with
`npm uninstall -g @cometix/ccline` to keep the source build.

### Quick Install (npm)

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

**Cross-Platform (Recommended)**
```json
{
  "statusLine": {
    "type": "command",
    "command": "~/.claude/ccline/ccline",
    "padding": 0
  }
}
```

> **Note for Windows users:** Starting from Claude Code v2.1.47+, Unix-style path parsing is supported on Windows. The `~` symbol is automatically expanded to your user home directory. **Do not use `%USERPROFILE%`** - it no longer works reliably in v2.1.47+.
> - Recommended: `~/.claude/ccline/ccline` (works on all platforms)
> - Alternative: `"ccline"` (requires npm global installation)

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

## Usage

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

### Segment Options from the CLI

Segment options live under `[segments.options]` in `config.toml`. In the TUI
(`ccline -c`) each option is a row under **Options** in the Settings panel:
booleans render as a checkbox and enumerations (e.g. `reset_format`) as an
inline select with the active choice highlighted. `Enter` toggles or cycles,
`←`/`→` step through choices, numbers and strings open a value prompt (empty
value resets to the default), and `S` saves. Quitting with `Esc` while there
are unsaved changes asks whether to save, discard or keep editing. Options can
also be listed and edited without opening the TUI:

```bash
# List every option with its current value, default and description
ccline --options

# Set one or more options (values are typed: true/false, numbers, strings)
ccline --set usage.reset_format=countdown --set model.show_effort=false

# Remove an option so the segment falls back to its default
ccline --unset usage.reset_format
```

Targets are `SEGMENT.KEY`, where `SEGMENT` is the id used in `config.toml`
(`model`, `directory`, `git`, `context_window`, `usage`, `credits`, `cost`,
`session`, `output_style`, `update`). Unknown keys are saved with a warning.

The TUI starts from `config.toml`, the file the statusline renders. Theme
files under `~/.claude/ccline/themes/` are applied only when you switch
themes (number keys, `P`, `R` or `--theme`), so edits made by hand to a theme
file do not show up until you re-select that theme. Upstream reloaded the
active theme file on startup, which discarded `config.toml` edits on save.

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

When Claude Code reports the active effort level (`effort.level` in the
statusline input, Claude Code 2.1.28x+), it is appended to the model name,
e.g. `Fable 5.1 · high`. Disable it with the `show_effort` option:

```toml
[[segments]]
id = "model"
enabled = true

[segments.options]
show_effort = false
```

### Usage Display

Shows plan usage from the Claude OAuth usage API (subscription accounts only):

```
󰪟 24% · 03:20 │ 7d 15% · Thu 00h
```

- `24%` and `03:20`: five-hour session usage and the local time it resets.
- `7d 15% · Thu 00h`: weekly usage and the local weekday/hour it resets.
  The circle icon fills according to the weekly usage.

If the API is unreachable (or no OAuth token is found), the segment falls back
to the `rate_limits` block Claude Code passes in the statusline input.

Options:

```toml
[[segments]]
id = "usage"
enabled = true

[segments.options]
api_base_url = "https://api.anthropic.com" # override for proxies
cache_duration = 300                       # seconds to cache API responses
timeout = 2                                # API request timeout in seconds
show_weekly = true                         # set to false to hide the weekly part
reset_format = "time"                      # "time" (03:20 / Thu 00h) or "countdown" (4h 52m / 6d 3h)
```

### Credits Display

Extra-usage credits spent this month against the seat's monthly spend limit,
as its own segment (`credits`) with the same colors as the usage segment:

```
󰄐 $23.75/$50 · 48%
```

It reads the same usage API response as the Usage segment (one request per
render, shared) and is only rendered when the organization has usage credits
enabled. Existing configs get the segment added automatically after `usage`,
enabled if `usage` is enabled. Options:

```toml
[[segments]]
id = "credits"
enabled = true

[segments.options]
show_percent = true                        # set to false to hide the "· 48%" suffix
```

### Context Window Display

Token usage percentage based on transcript analysis with context limit tracking.

The context limit is resolved in this order:

1. `context_window.context_window_size` from the statusline JSON that Claude Code sends (authoritative for the active model, e.g. 1M for Fable / Mythos).
2. A `[[context_modifiers]]` match (e.g. `[1m]`) or a `[[models]]` entry in `models.toml`.
3. Built-in families: Sonnet / Opus / Haiku default to 200k, Fable / Mythos default to 1M.
4. 200k when nothing matches.

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

Supported segments: Model, Directory, Git, Context Window, Usage, Credits, Cost, Session, OutputStyle, Update

### Model Configuration (`models.toml`)

Location: `~/.claude/ccline/models.toml` (auto-created on first run)

This file configures how model IDs are displayed and their context window limits. Claude models (Sonnet, Opus, Haiku, Fable, Mythos) are automatically recognized with version extraction — you only need this file for overrides or third-party models.

```toml
# Model entries: simple substring matching on the model ID
# These take priority over built-in Claude model recognition
[[models]]
pattern = "glm-4.5"
display_name = "GLM-4.5"
context_limit = 128000

[[models]]
pattern = "kimi-k2"
display_name = "Kimi K2"
context_limit = 128000

# Context modifiers: matched independently and composable with model entries
# Overrides context_limit and appends display_suffix to the display name
# e.g., model "Opus 4" + modifier " 1M" = "Opus 4 1M"
[[context_modifiers]]
pattern = "[1m]"
display_suffix = " 1M"
context_limit = 1000000
```


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
