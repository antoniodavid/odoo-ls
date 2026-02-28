# Odoo Language Server (odoo-ls) - Agent Guidelines

Welcome to the `odoo-ls` repository. This file provides guidelines for AI coding agents working on this Rust codebase. The primary component of this project is a Language Server implementation for Odoo, located in the `server/` directory.

## 🏗️ Build, Lint, and Test Commands

All development should happen inside the `server/` directory.

### Build & Run
- **Check (Fast):** `cargo check`
- **Build (Debug):** `cargo build`
- **Build (Release):** `cargo build --release`
- **Run Language Server:** `cargo run --bin odoo_ls_server`
- **Print Config Schema:** `cargo run --bin print_config_schema`

### Linting
- **Clippy (Linter):** `cargo clippy -- -D warnings`
  _Note: Fix all warnings before committing._
- **Formatting:** `cargo fmt` (if installed, standard Rust formatting conventions apply)

### Testing
- **Run all tests:** `cargo test`
- **Run tests in a specific file:** `cargo test --test <filename>` (e.g., `cargo test --test test_get_symbol`)
- **Run a specific test function:** `cargo test <test_function_name>` (e.g., `cargo test test_follow_ref`)
- **Run tests with output (for debugging):** `cargo test -- --nocapture`

## 📝 Code Style & Conventions

This project follows standard Rust conventions, but with some specific patterns tailored for this Language Server implementation.

### Architecture & Structure
1. **Directory Structure:**
   - `src/core/`: Central logic (Odoo instance state, symbols, models, AST evaluation, file management).
   - `src/features/`: LSP features (Hover, Completion, Definition, References, etc.).
   - `src/bin/`: Executable entry points.
2. **State Management:**
   - The main state is often held in `SessionInfo` and its inner `SyncOdoo` struct.
   - We heavily use `Rc<RefCell<T>>` for shared mutable state (especially for `Symbol`, `EntryPoint`, and `FileInfo`), forming a graph of symbols.
   - When modifying graphs or relationships, ensure you upgrade `Weak` references safely using `if let Some(rc) = weak.upgrade()`.
3. **AST Parsing:**
   - This project uses `ruff_python_ast` and related Ruff libraries for parsing Python code.
   - Familiarize yourself with the Ruff AST structures when working with Python evaluation or completions.

### Coding Practices
1. **Error Handling:**
   - Use `anyhow::Result` for general functions where error bubbling is needed, though many core methods return `Option<T>` or specific standard results.
   - Avoid `.unwrap()` or `.expect()` in production code unless the state is strictly guaranteed (like extracting a parent when iterating known children). Use `if let Some(...)` or `match`.
   - LSP specific errors use `lsp_server::ResponseError`.
2. **Naming Conventions:**
   - `snake_case` for variables, functions, and modules.
   - `CamelCase` for structs, traits, and enums.
   - `SCREAMING_SNAKE_CASE` for constants (e.g., `MAX_WATCHED_FILES_UPDATES_BEFORE_RESTART`).
3. **Imports:**
   - Standard library imports first (`std::...`).
   - Third-party crates next (`lsp_types`, `ruff_...`, `serde`, etc.).
   - Internal crate imports last (`crate::core::...`).
   - Group related imports where possible.
4. **Macros:**
   - We use custom macros like `oyarn!` for memory-efficient string deduplication (using the `byteyarn` crate) and `Sy!` for standard string symbol creation. Use them when creating or comparing strings that are repeated often (like module names or Odoo properties).
5. **Logging & Tracing:**
   - The project uses the `tracing` crate (`info!`, `warn!`, `error!`, `trace!`).
   - Use `session.log_message(MessageType::..., ...)` to send logs directly to the LSP client interface.
   - Combine both when an event is critical for both the server logs and the user's editor output.

### Concurrency & Performance
- The project implements an intricate architecture for "Rebuilding" the AST/Symbols (e.g., `process_rebuilds_batch`).
- Be extremely careful with `RefCell::borrow_mut()` as it can cause runtime panics if borrowed twice in the same execution path. Avoid deep nesting of mutable borrows.
- For complex symbol resolutions or memory-intensive tasks, prefer batched processing to prevent the language server from blocking editor responsiveness.
- Use caching mechanisms (like `ModuleCacheManager`) proactively for file metadata and symbols to maintain high performance.

## 🤖 AI Agent Workflow

1. **Understand First:** Before modifying any file in `src/core/` or `src/features/`, thoroughly read the surrounding code, especially the structs and their `RefCell` implementations.
2. **Absolute Paths:** Always use absolute paths for file reading/writing.
3. **Run Checks:** NEVER assume your Rust code compiles. Always run `cd server && cargo check` before claiming a task is complete.
4. **Test Your Fixes:** If fixing a bug, find the relevant test in `server/tests/` and run it, or write a minimal unit test to verify the fix.
5. **Format:** Stick to the existing indentation and spacing.
