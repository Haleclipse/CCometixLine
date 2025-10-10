# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.9] - 2025-10-09

### Added
- **Two New Usage Segments**: Enhanced API usage monitoring with dedicated segments
  - **Usage5Hour Segment**: Displays 5-hour usage percentage with reset time (e.g., `24% -> 11am`)
  - **Usage7Day Segment**: Displays 7-day usage percentage with full reset datetime (e.g., `12% -> Oct 9, 5am`)
  - Both segments share efficient API cache to minimize redundant calls
  - Dynamic circle icons that change based on utilization level
  - Automatic UTC to local timezone conversion for reset times
  - Disabled by default (opt-in via config or TUI)

### Changed
- **API Cache Structure**: Updated to store both 5-hour and 7-day reset times separately
  - Cache now includes `five_hour_resets_at` and `seven_day_resets_at` fields
  - Backward compatible with graceful handling of old cache files
- **Segment ID Enum**: Added `Usage5Hour` and `Usage7Day` variants
- **All Theme Files**: Updated all 9 built-in themes to include new usage segments
- **TUI Components**: Enhanced UI components with new segment name mappings

### Technical Details
- New time formatting functions: `format_5hour_reset_time()` and `format_7day_reset_time()`
- Public API for `UsageSegment::load_usage_cache()` to enable cache sharing
- Added mock preview data for new segments in TUI configurator
- All UI match statements updated to handle new segment IDs

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

