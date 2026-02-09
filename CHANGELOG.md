# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.1.1] - 2026-02-09

### Changed
- **Model Recognition Redesign**: Replace hardcoded model entries with regex-based Claude model family recognition
  - Auto-extract version numbers from model IDs (e.g., `claude-opus-4-6-20250901` → "Opus 4.6")
  - Support both naming conventions: `claude-{variant}-{major}-{minor}-{date}` and `claude-{major}-{minor}-{variant}-{date}`
  - New model versions (Opus 4.7, Sonnet 5, etc.) require zero config changes
  - Handle text suffixes like `-thinking`, `-preview`, `-latest`
- **Context Modifier System**: Decouple `[1m]` context modifier from model identity
  - Modifiers compose independently with any model: "Opus 4.6" + " 1M" = "Opus 4.6 1M"
  - Fixes incorrect display when Opus models gained `[1m]` variants (was hardcoded as "Sonnet 4.5 1M")
  - User-configurable via `[[context_modifiers]]` in `models.toml`
- **Single-pass Model Resolution**: New `resolve()` method eliminates redundant matching
  - `get_display_name()`, `get_context_limit()`, `get_display_suffix()` all delegate to one pass
  - Compiled regex cached via `OnceLock` for zero-cost repeated access

### Fixed
- **Empty Display Name Fallback**: Show raw model_id when upstream `display_name` is empty
  - Third-party models like `minimax/minimax-2-1` now display correctly instead of blank
- **Opus Model Recognition**: `claude-opus-4-6-*[1m]` now correctly shows "Opus 4.6 1M" instead of "Sonnet 4.5 1M"

## [1.1.0] - 2026-01-26

### Added
- **Linux ARM64 Support**: Full ARM64 (aarch64) platform support for Linux (#68)
  - New `linux-arm64` and `linux-arm64-musl` NPM packages
  - ARM64 cross-compilation in GitHub Actions workflow
  - Improved libc detection (glibc vs musl) for ARM64
- **Custom Credentials Path**: Prioritize `CLAUDE_CONFIG_DIR` environment variable for OAuth credentials (#45)
  - Support reading from `$CLAUDE_CONFIG_DIR/.credentials.json`
  - Fallback to default `~/.claude/.credentials.json`

### Changed
- **Patcher Optimization**: Single-pass AST parsing for 5-6x speedup
  - Parse AST only once and reuse for all 6 patches
  - Fix verbose property matching to only check direct props object keys
  - Rename "Verbose property" to "Spinner token counter" for clarity
- **Session Segment Colors**: Line changes now use fixed colors
  - Added lines (`+XX`) displayed in green
  - Removed lines (`-XX`) displayed in red

### Fixed
- **Usage Segment Backgrounds**: Sync all powerline themes with TOML configurations
  - powerline-dark: RGB(209,213,219) text, RGB(45,50,59) background
  - powerline-light: RGB(255,255,255) text, RGB(40,167,69) background
  - powerline-rose-pine: RGB(246,193,119) text, RGB(35,33,54) background
  - powerline-tokyo-night: RGB(224,175,104) text, RGB(36,40,59) background
  - nord: RGB(46,52,64) text, RGB(235,203,139) background
- **Preview Session Colors**: Apply green/red ANSI colors to preview mock data

### Removed
- **npm Deprecation Warning Patch**: Use `DISABLE_INSTALLATION_CHECKS=1` environment variable instead
- **Legacy Patcher APIs**: Removed unused single-patch functions
- **Native Context Window API**: Reverted PR #71 for further refinement

### Refactored
- **Credentials Module**: Extract `read_token_from_path()` helper to reduce code duplication

## [1.0.9] - 2025-12-21

### Added
- **Claude in Chrome Subscription Bypass**: New patcher functionality to bypass Chrome feature subscription checks
  - `bypass_chrome_subscription_check()`: Enables Chrome feature without subscription
  - `remove_chrome_startup_notification_check()`: Disables startup subscription notification
  - `remove_chrome_command_subscription_message()`: Hides /chrome command subscription error
- **Lefthook Integration**: Pre-commit hooks for automatic code formatting and clippy checks

### Fixed
- **Icon Selector Input**: Fixed bug where 'c' key couldn't be typed in custom icon input mode
- **Icon Selector Save**: Fixed custom icon not being saved properly on Enter
- **Git Lock Conflicts**: Added `--no-optional-locks` flag to all git commands to prevent `.git/index.lock` conflicts
- **Disabled Segments**: Skip disabled segments in `collect_all_segments` to avoid unnecessary API requests
- **Main Menu UX**: Keep menu open after "Check Configuration" or "Initialize Config" actions
- **Help Panel Height**: Fixed height calculation to properly show status messages in configuration mode

### Changed
- **ESC Interrupt Pattern**: Updated pattern matching for new Claude Code versions with legacy fallback
- **Config Init Return**: `Config::init()` now returns `InitResult` enum for better status handling

## [1.0.8] - 2025-10-08

### Fixed
- **API Usage Timezone**: Convert API usage reset time from UTC to local timezone with proper rounding

## [1.0.7] - 2025-10-02

### Fixed
- **Proxy Support**: Added proxy support for API usage requests
- **CI Pipeline**: Install rustfmt and clippy components in CI
- **Debug Output**: Remove debug output from proxy configuration

## [1.0.6] - 2025-10-02

### Added
- **API Usage Segment**: New segment showing Anthropic API usage statistics
- **ESC Interrupt Disabler**: Claude Code patcher can now disable "esc to interrupt" display

### Changed
- **Token Usage Renamed**: Renamed "Token Usage" segment to "Context Window" for clarity

## [1.0.5] - 2025-09-09

### Fixed
- **Third-party Model Usage**: Resolved usage calculation issues for third-party models (GLM-4.5, etc.)

### Documentation
- Added related projects section to README

## [1.0.4] - 2025-08-28

### Added
- **Interactive Main Menu**: Direct execution now shows TUI menu instead of hanging
- **Claude Code Patcher**: `--patch` command to disable context warnings and enable verbose mode
- **Three New Segments**: Extended statusline with additional information
  - **Cost Segment**: Shows monetary cost with intelligent zero-cost handling
  - **Session Segment**: Displays session duration and line changes  
  - **OutputStyle Segment**: Shows current output style name
- **Enhanced Theme System**: Comprehensive theme architecture with 9 built-in themes
  - Modular theme organization with individual theme modules
  - 4 new Powerline theme variants (dark, light, rose pine, tokyo night)
  - Enhanced existing themes (cometix, default, minimal, gruvbox, nord)
- **Model Management System**: Intelligent model recognition and configuration

### Fixed
- **Direct Execution Hanging**: No longer hangs when executed without stdin input
- **Help Component Styling**: Consistent key highlighting across all TUI help displays
- **Cross-platform Path Support**: Enhanced Windows %USERPROFILE% and Unix ~/ path handling


## [1.0.3] - 2025-08-17

### Fixed
- **TUI Preview Display**: Complete redesign of preview system for cross-platform reliability
  - Replaced environment-dependent segment collection with pure mock data generation
  - Fixed Git segment not showing in preview on Windows and Linux systems
  - Ensures consistent preview display across all supported platforms
- **Documentation Accuracy**: Corrected CLI parameter reference from `--interactive` to `--config`
  - Fixed changelog and documentation to reflect actual CLI parameters
- **Preview Data Quality**: Enhanced mock data to better represent actual usage
  - Usage segment now displays proper format: "78.2% · 156.4k"
  - Update segment displays dynamic version number from Cargo.toml
  - All segments show realistic and informative preview data

### Changed
- **Preview Architecture**: Complete rewrite of preview component for better maintainability
  - Removed dependency on real file system and Git repository detection
  - Implemented `generate_mock_segments_data()` for environment-independent previews
  - Simplified code structure and improved performance
  - Preview now works reliably in any environment without external dependencies

### Technical Details
- Environment-independent mock data generation for all segment types
- Dynamic version display using `env!("CARGO_PKG_VERSION")`
- Optimized preview rendering without file system calls or Git operations
- Consistent cross-platform display: "Sonnet 4 | CCometixLine | main ✓ | 78.2% · 156.4k"

## [1.0.2] - 2025-08-17

### Fixed
- **Windows PowerShell Compatibility**: Fixed double key event triggering in TUI interface
  - Resolved issue #18 where keystrokes were registered twice on Windows PowerShell
  - Added proper KeyEventKind filtering to only process key press events
  - Maintained cross-platform compatibility with Unix/Linux/macOS systems

### Technical Details
- Import KeyEventKind from crossterm::event module  
- Filter out KeyUp events to prevent double triggering on Windows Console API
- Uses efficient continue statement to skip non-press events
- No impact on existing behavior on Unix-based systems

## [1.0.1] - 2025-08-17

### Fixed
- NPM package publishing workflow compatibility issues
- Cargo.lock version synchronization with package version
- GitHub Actions release pipeline for NPM distribution

### Changed
- Enhanced npm postinstall script with improved binary lookup for different package managers
- Better error handling and user feedback in installation process
- Improved cross-platform compatibility for npm package installation

### Technical
- Updated dependency versions (bitflags, proc-macro2)
- Resolved NPM version conflict preventing 1.0.0 re-publication
- Ensured proper version alignment across all distribution channels

## [1.0.0] - 2025-08-16

### Added
- **Interactive TUI Mode**: Full-featured terminal user interface with ratatui
  - Real-time statusline preview while editing configuration
  - Live theme switching with instant visual feedback
  - Intuitive keyboard navigation (Tab, Escape, Enter, Arrow keys)
  - Comprehensive help system with context-sensitive guidance
- **Comprehensive Theme System**: Modular theme architecture with multiple presets
  - Default, Minimal, Powerline, Compact themes included
  - Custom color schemes and icon sets
  - Theme validation and error reporting
  - Powerline theme importer for external theme compatibility
- **Enhanced Configuration System**: Robust config management with validation
  - TOML-based configuration with schema validation
  - Dynamic config loading with intelligent defaults
  - Interactive mode support and theme selection
  - Configuration error handling and user feedback
- **Advanced Segment System**: Modular statusline segments with improved functionality
  - Enhanced Git segment with stash detection and conflict status
  - Model segment with simplified display names for Claude models
  - Directory segment with customizable display options
  - Usage segment with better token calculation accuracy
  - Update segment for version management and notifications
- **CLI Interface Enhancements**: Improved command-line experience
  - `--config` flag for launching TUI configuration mode
  - Enhanced argument parsing with better error messages
  - Theme selection via command line options
  - Comprehensive help and version information

### Changed
- **Architecture**: Complete modularization of codebase for better maintainability
  - Separated core logic from presentation layer
  - Improved error handling throughout all modules
  - Better separation of concerns between data and UI
- **Dependencies**: Added TUI and terminal handling capabilities
  - ratatui for terminal user interface components
  - crossterm for cross-platform terminal manipulation
  - ansi_term and ansi-to-tui for color processing
- **Configuration**: Enhanced config structure for theme and TUI mode support
  - Expanded config types to support new features
  - Improved validation and default value handling
  - Better error messages for configuration issues

### Technical Improvements
- **Performance**: Optimized statusline generation and rendering
- **Code Quality**: Comprehensive refactoring with improved error handling
- **User Experience**: Intuitive interface design with immediate visual feedback
- **Extensibility**: Modular architecture allows easy addition of new themes and segments

### Breaking Changes
- Configuration file format has been extended (backward compatible for basic usage)
- Some internal APIs have been restructured for better modularity
- Minimum supported features now include optional TUI dependencies

## [0.1.1] - 2025-08-12

### Added
- Support for `total_tokens` field in token calculation for better accuracy with GLM-4.5 and similar providers
- Proper Git repository detection using `git rev-parse --git-dir`
- Cross-platform compatibility improvements for Windows path handling
- Pre-commit hooks for automatic code formatting
- **Static Linux binary**: Added musl-based static binary for universal Linux compatibility without glibc dependencies

### Changed
- **Token calculation priority**: `total_tokens` → Claude format → OpenAI format → fallback
- **Display formatting**: Removed redundant ".0" from integer percentages and token counts
  - `0.0%` → `0%`, `25.0%` → `25%`, `50.0k` → `50k`
- **CI/CD**: Updated GitHub Actions to use Ubuntu 22.04 for Linux builds and ubuntu-latest for Windows cross-compilation
- **Binary distribution**: Now provides two Linux options - dynamic (glibc) and static (musl) binaries
- **Version management**: Unified version number using `env!("CARGO_PKG_VERSION")`

### Fixed
- Git segment now properly hides for non-Git directories instead of showing misleading "detached" status
- Windows Git repository path handling issues by removing overly aggressive path sanitization
- GitHub Actions runner compatibility issues (updated to supported versions: ubuntu-22.04 for Linux, ubuntu-latest for Windows)
- **Git version compatibility**: Added fallback to `git symbolic-ref` for Git versions < 2.22 when `--show-current` is not available

### Removed
- Path sanitization function that could break Windows paths in Git operations

## [0.1.0] - 2025-08-11

### Added
- Initial release of CCometixLine
- High-performance Rust-based statusline tool for Claude Code
- Git integration with branch, status, and tracking info
- Model display with simplified Claude model names
- Usage tracking based on transcript analysis
- Directory display showing current workspace
- Minimal design using Nerd Font icons
- Cross-platform support (Linux, macOS, Windows)
- Command-line configuration options
- GitHub Actions CI/CD pipeline

### Technical Details
- Context limit: 200,000 tokens
- Startup time: < 50ms
- Memory usage: < 10MB
- Binary size: ~2MB optimized release build

