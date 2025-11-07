# Agent Guidelines for Tipitaka XML Parser

## Project Structure

When working on features, the PRD (Product Requirements Document) files are in
the `tasks/` folder. They often contain the reasoning and logic for existing
features.

Additional documentation is in the `docs/` folder. Keep it updated for relevant features.

## Build & Test Commands

- Build: `cargo build` or `cargo build --release`
- Test all: `cargo test`
- Test single file: `cargo test --test test_xml_fragment_position_tracking`
- Test single function: `cargo test test_s0101m_mul_position_tracking`
- Run binary: `cargo run -- <args>`
- Check: `cargo check`

## Code Style

- **Imports**: Use `anyhow::{Result, Context}` for error handling; `use` statements grouped by std, external crates, then internal modules
- **Error Handling**: Return `anyhow::Result<T>` from functions; use `.context()` for error chains; avoid unwrap/expect in production code
- **Types**: All public structs/enums have doc comments; use `#[derive(Debug, Clone, Serialize, Deserialize)]` for data structures
- **Naming**: snake_case for functions/variables, PascalCase for types/enums, SCREAMING_SNAKE_CASE for constants
- **Comments**: Doc comments (`///`) for public items; inline comments for complex logic; module-level docs (`//!`) at file top
- **Formatting**: Standard rustfmt; 4-space indents; max line length flexible for readability
- **Indexing**: 0-indexed for arrays/char positions, 1-indexed for line numbers (document clearly in comments)
- **Testing**: Test files in `tests/` for integration tests, `#[cfg(test)] mod` for unit tests; include usage examples in test doc comments

## Architecture
- Fragment-based XML parsing with line/char position tracking
- Uses quick-xml for parsing, diesel for SQLite database operations
- Core types in `types.rs`, parsing logic in `fragment_parser.rs`, nikaya detection in `nikaya_detector.rs`
