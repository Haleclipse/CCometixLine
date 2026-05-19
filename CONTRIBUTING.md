# Contributing to CCometixLine

Thank you for contributing to CCometixLine!

## Prerequisites

- **Rust 1.70+** - Install via [rustup](https://rustup.rs/)
- Git for version control

## Development Setup

1. **Clone the repository**
   ```bash
   git clone https://github.com/Haleclipse/CCometixLine.git
   cd CCometixLine
   ```

2. **Build the project**
   ```bash
   cargo build --release
   ```

3. **Run tests**
   ```bash
   cargo test
   cargo clippy  # Linting
   cargo fmt     # Formatting
   ```

4. **Run in development mode**
   ```bash
   cargo run
   ```

## Project Structure

```
src/
├── main.rs           # Application entry point
└── lib.rs            # Library code (if applicable)

# Configuration
cliff.toml            # Conventional changelog configuration
lefthook.yml          # Git hooks configuration
```

## Code Style

- Run `cargo fmt` before committing
- Run `cargo clippy` to catch common mistakes
- Write idiomatic Rust with clear naming conventions
- Add doc comments (`///`) for public functions

## Conventional Commits

This project uses [Conventional Commits](https://www.conventionalcommits.org/):

- `feat:` - New feature
- `fix:` - Bug fix
- `docs:` - Documentation changes
- `refactor:` - Code refactoring
- `test:` - Adding or updating tests
- `chore:` - Maintenance tasks

## Testing

```bash
# Run all tests
cargo test --all

# Run with output
cargo test -- --nocapture

# Run specific test
cargo test test_name
```

## Pull Request Process

1. Fork the repository
2. Create a feature branch (`feat/your-feature`)
3. Make your changes
4. Ensure tests pass and clippy is clean
5. Commit using conventional commit format
6. Push and create a PR

## Reporting Issues

Include in your issue report:
- Rust version (`rustc --version`)
- Operating system and version
- Steps to reproduce the issue
- Expected behavior

## License

By contributing, you agree your contributions will be licensed under the project license.
