# Odoo-LS Technical Documentation

## 1. Overview
**Odoo-LS** is a high-performance Language Server Protocol (LSP) implementation specifically designed for Odoo development. Written in **Rust**, it aims to provide a fast, reliable, and deep understanding of Odoo's unique patterns and structures, which are often challenging for generic Python language servers.

- **Project Location**: `/home/adruban/Workspace/Personal/odoo-ls/server`
- **Current Version**: `1.2.0`
- **Language**: Rust
- **Purpose**: To empower Odoo developers with IDE-like features such as context-aware autocompletion, precise navigation, and real-time diagnostics for both Python and XML files.

## 2. Core Features
Odoo-LS implements several key LSP features tailored for the Odoo ecosystem:

- **Autocompletion**: 
    - Intelligent suggestions for Odoo model names, fields, and methods.
    - Context-aware completions for `_inherit`, `_name`, `compute` fields, and `@api.depends`.
    - XML completions for field names, model attributes, and view inheritance.
- **Go-to-Definition**: Direct navigation to the definition of models, fields, and methods across the entire project and its dependencies.
- **Hover Information**: Detailed popups showing field types, model definitions, and method signatures with documentation.
- **Diagnostics**: 
    - Real-time Python syntax error detection.
    - Odoo-specific validation (e.g., checking if an inherited model exists).
    - Resolution of Odoo imports and module dependencies.
- **Document Symbols**: Provides a hierarchical outline of classes, methods, and fields within a Python file.
- **Find References**: Quickly locate all occurrences of a specific field or method across the codebase.

## 3. Architecture
The server is structured into modular components, with a clear separation between core logic and LSP feature implementations:

- `src/core/evaluation.rs`: The heart of the type evaluation system, responsible for resolving types and values in Python and Odoo contexts.
- `src/core/symbols/symbol.rs`: Manages the symbol hierarchy, including models, fields, classes, and functions. It handles symbol registration, lookup, and lifecycle.
- `src/core/python_arch_builder.rs`: Builds the internal representation (AST) of Python files using the `ruff_python_parser`.
- `src/core/python_arch_eval.rs`: Performs static analysis and evaluation on the Python AST to extract Odoo-specific metadata.
- `src/core/odoo.rs`: Contains Odoo-specific logic, such as module loading rules, manifest parsing, and model inheritance resolution.
- `src/features/completion.rs`: Implements the autocompletion logic for Python files.
- `src/features/definition.rs`: Handles the "Go-to-Definition" requests by mapping AST nodes to their corresponding symbols.
- `src/features/hover.rs`: Generates the markdown content for hover popups based on evaluated symbol information.

## 4. How Autocompletion Works

### Python Autocompletion
When a user triggers completion in a `.py` file:
1. The LSP client sends a `textDocument/completion` request.
2. `CompletionFeature::autocomplete()` is invoked with the current file and cursor position.
3. The system identifies the **Completion Context** based on the AST and surrounding tokens.
4. **Resolution**:
    - If in `_inherit = "..."`, it fetches all available Odoo model names.
    - If in `self.env["..."]`, it suggests registered models.
    - If in `@api.depends("...")`, it resolves fields of the current model.
5. **Response**: Returns a list of `CompletionItem` objects with appropriate labels, kinds (Field, Method, Class), and documentation.

**Key Contexts Supported**:
- `_inherit = "|"` → Odoo model names.
- `_name = "|"` → New model names (often suggests based on file path).
- `compute="|"` → Suggests methods defined in the same class.
- `self.env["|"` → All available Odoo models.
- `@api.depends("|"` → Fields available in the current model.
- `domain="[('|"` → Field names relevant to the target model in a domain expression.

### XML Autocompletion
When working in an Odoo XML file:
1. The system uses `roxmltree` to parse the XML structure.
2. It identifies the target model for the current view or field (e.g., by looking at the `<record model="...">` or `<field name="..." model="...">`).
3. **Context Identification**:
    - Inside `<field name="|"`: Suggests fields belonging to the active model.
    - Inside `model="|"`: Suggests all available Odoo models.
    - Inside `inherit_id="|"`: Suggests external XML IDs of existing views.
4. Returns filtered completion items based on the XML tag and attribute context.

## 5. Symbol System
Odoo-LS employs a robust symbol management system to track code elements:

- **Hierarchy**: Symbols are organized in a tree: `Root` → `Namespace` (Addons) → `Package` (Module) → `File` → `Class`/`Function`/`Variable`.
- **Symbol Resolution**: Efficiently finds symbols by their fully qualified path (e.g., `odoo.addons.base.models.res_partner.Partner`).
- **Lazy Loading**: To minimize memory usage, symbols are loaded and parsed on-demand when requested by the user or required for evaluation.
- **Evaluations**: Stores cached type information and Odoo metadata (like whether a class is a Model or a TransientModel).
- **Weak References**: Uses `Weak<RefCell<Symbol>>` to prevent reference cycles and memory leaks, ensuring that symbols can be dropped when they are no longer needed (e.g., when a file is closed).

## 6. Advantages
- **Performance**: Built with Rust, providing near-instant responses even in large Odoo projects with thousands of files.
- **Type Safety**: Leverages Rust's strict type system to ensure internal consistency and prevent common runtime errors found in Python-based tools.
- **Memory Safety**: No garbage collector overhead; memory is managed precisely, preventing leaks during long-running sessions.
- **Cross-platform**: Fully compatible with Linux, Windows, and macOS.
- **LSP Standard**: Works seamlessly with any editor that supports LSP (VS Code, Neovim, Emacs, etc.).
- **Odoo-specific Intelligence**: Deeply understands Odoo-specific patterns like `_inherit`, `_name`, and the `env` object, which generic servers often miss.
- **Local Analysis**: All processing happens locally on the developer's machine, ensuring privacy and offline availability.

## 7. Disadvantages
- **Complex Setup**: Building from source requires the Rust toolchain and cargo.
- **Learning Curve**: Contributing to the codebase requires proficiency in Rust, which has a steeper learning curve than Python.
- **Community Size**: Currently has a smaller contributor base compared to established generic Python tools like Pyright or Jedi.
- **Documentation**: Internal API documentation is still evolving.
- **Community Path Dependency**: Many integration tests require a local copy of the Odoo Community source code (`COMMUNITY_PATH`).
- **Build Times**: Rust's compilation process can be slower than interpreted languages or Go.

## 8. Improvement Opportunities

### Short-term
1. **Increase Test Coverage**: Expand unit tests to cover more edge cases in model inheritance (currently ~17 core tests passing).
2. **Improve Documentation**: Better internal docstrings and a dedicated contributor guide to lower the entry barrier for new developers.
3. **Performance Optimization**: Profile and optimize hot paths in symbol resolution and XML parsing.
4. **Bug Fixes**: Address known issues in complex multi-level inheritance resolution.

### Medium-term
1. **Enhanced Completion Contexts**:
   - Add deep support for QWeb templates (`t-field`, `t-foreach`).
   - Implement intelligent completion for XML domain expressions.
   - Provide completion for Wizard method calls and `context` keys.
2. **Better Error Recovery**: Implement a more tolerant parser to provide completions even when the Python or XML file has syntax errors.
3. **Smart Caching**: Improve cache invalidation logic to only re-parse files that have actually changed.

### Long-term
1. **Odoo JavaScript Support**: Extend the server to understand Odoo's Owl framework and legacy JS assets.
2. **Advanced Refactoring**:
   - Rename refactoring that updates both Python code and XML references.
   - Code actions for common Odoo tasks (e.g., "Extract to computed field").
3. **Inlay Hints**: Show inferred types and field technical names directly in the editor.
4. **Semantic Tokens**: Provide better syntax highlighting based on the semantic meaning of Odoo patterns.

## 9. Technical Stack
- **Language**: Rust (Edition 2024)
- **LSP Foundation**: `lsp-types`, `lsp-server`
- **Parsing**: `ruff_python_parser` (from Astral) for Python, `roxmltree` for XML.
- **Serialization**: `serde`, `serde_json`
- **Utilities**: `itertools`, `anyhow`, `tracing` for logging.

## 10. Project Structure
```text
odoo-ls/
├── src/
│   ├── core/          # Core logic (evaluation, symbols, Odoo resolution)
│   │   ├── symbols/   # Specialized symbol types (Class, Function, etc.)
│   │   └── ...
│   ├── features/      # LSP feature implementations
│   │   ├── completion.rs
│   │   ├── definition.rs
│   │   └── hover.rs
│   ├── server.rs      # LSP message handling and routing
│   ├── main.rs        # Entry point
│   └── ...
├── tests/             # Integration and unit test suite
├── Cargo.toml         # Dependency management
└── README.md          # Project overview
```

---
*This documentation is intended for contributors and technical users of Odoo-LS.*
