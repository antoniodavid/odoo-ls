# Odoo-LS Memory Optimization Plan

## Problem Statement

Odoo-LS consumes 2GB+ RAM during indexing of large Odoo codebases. This is unacceptable for daily development use.

## Root Cause Analysis

### Memory Hotspot #1: FileMgr (file_mgr.rs)

**Location**: `pub files: HashMap<String, Rc<RefCell<FileInfo>>>`

**Issue**: Every file ever opened is kept in memory forever.

```rust
pub struct FileInfo {
    pub file_info_ast: Rc<RefCell<FileInfoAst>>,  // THE BIG ONE
    diagnostics: HashMap<BuildSteps, Vec<Diagnostic>>,
    noqas_blocs: HashMap<u32, NoqaInfo>,
    // ...
}

pub struct FileInfoAst {
    pub text_hash: u64,
    pub text_document: Option<TextDocument>,     // Full source text
    pub indexed_module: Option<Arc<IndexedModule>>,  // Full parsed AST
    pub ast_type: AstType,
}
```

**Evidence**:
- `text_document` contains the ENTIRE file contents as a string
- `indexed_module` contains the FULL parsed AST from ruff_python_parser
- No eviction policy - files accumulate indefinitely

**Note**: There IS a `prepare_ast()` method (line 226) that can reload ASTs, suggesting the original authors planned for eviction but never implemented it.

### Memory Hotspot #2: Symbol Graph (symbol.rs)

**Location**: `Rc<RefCell<Symbol>>` everywhere

**Issue**: Millions of small heap allocations for symbol nodes.

```rust
pub enum Symbol {
    Root(RootSymbol),
    DiskDir(DiskDirSymbol),
    Namespace(NamespaceSymbol),
    Package(PackageSymbol),
    File(FileSymbol),
    Class(ClassSymbol),
    Function(FunctionSymbol),
    Variable(VariableSymbol),
    // ...
}
```

Each symbol is wrapped in `Rc<RefCell<>>`, causing:
- 16-24 bytes overhead per allocation (Rc refcount + RefCell borrow flag)
- Memory fragmentation from millions of small allocations
- Poor cache locality

### Memory Hotspot #3: No External File Eviction

Files from Python stdlib, site-packages, and Odoo core are parsed and kept in memory just like workspace files, despite rarely being accessed after initial indexing.

## Proposed Solutions

### Phase 1: LRU Cache for FileInfoAst (LOW RISK, HIGH IMPACT)

**Strategy**: Implement an LRU cache that evicts `text_document` and `indexed_module` from non-workspace files.

**Implementation**:
1. Add `lru` crate to dependencies
2. Create `AstCache` wrapper around file AST storage
3. Keep only file metadata (path, hash, diagnostics) permanently
4. On cache miss: reload from disk via existing `prepare_ast()` method

```rust
// Proposed structure
pub struct AstCache {
    cache: LruCache<String, Rc<RefCell<FileInfoAst>>>,
    capacity: usize,  // e.g., 500 files
}

impl AstCache {
    pub fn get(&mut self, path: &str, session: &mut SessionInfo) -> Option<Rc<RefCell<FileInfoAst>>> {
        if let Some(ast) = self.cache.get(path) {
            return Some(ast.clone());
        }
        // Cache miss - reload from disk
        self.load_and_cache(path, session)
    }
}
```

**Files to modify**:
- `server/src/core/file_mgr.rs` - Add LRU cache
- `server/Cargo.toml` - Add `lru` dependency

**Expected impact**: 50-70% memory reduction for large codebases

### Phase 2: Separate Workspace vs External Storage (MEDIUM RISK)

**Strategy**: Keep full ASTs only for workspace files; store only "signatures" for external files.

**What to keep for external files**:
- Symbol names and types
- Function signatures (parameters, return types)
- Class hierarchies
- Model definitions

**What to evict for external files**:
- Full AST nodes
- Source text
- Statement bodies

**Expected impact**: Additional 20-30% reduction

### Phase 3: Arena Allocator (HIGH RISK, LONG-TERM)

**Reference**: PR #154 (stalled since Nov 2024)

**Strategy**: Replace `Rc<RefCell<Symbol>>` with arena-allocated indices.

```rust
// Current (fragmented)
pub type SymbolPtr = Rc<RefCell<Symbol>>;

// Proposed (arena-based)
pub type SymbolIdx = u32;  // Index into arena
pub struct SymbolArena {
    symbols: Vec<Symbol>,
    // ...
}
```

**Benefits**:
- Eliminates refcount overhead
- Contiguous memory allocation
- Better cache performance
- Bulk deallocation possible

**Risks**:
- Major refactor (touches every file)
- Requires careful lifetime management
- PR #154 has been stalled for over a year

**Recommendation**: Defer until Phase 1 & 2 prove insufficient

## Implementation Priority

| Phase | Effort | Risk | Impact | Priority |
|-------|--------|------|--------|----------|
| Phase 1: LRU Cache | 1-2 days | Low | High (50-70%) | **IMMEDIATE** |
| Phase 2: Signature-only external | 3-5 days | Medium | Medium (20-30%) | After Phase 1 |
| Phase 3: Arena Allocator | 2-4 weeks | High | Medium (10-20%) | Future |

## Existing Infrastructure

### Tracking Allocator (allocator.rs)
Already exists! Can be used to measure memory impact:
```rust
pub static ALLOCATED: AtomicUsize = AtomicUsize::new(0);
```

### prepare_ast() Method
Already exists in FileInfo - designed for lazy AST loading:
```rust
pub fn prepare_ast(&mut self, session: &mut SessionInfo) {
    if self.file_info_ast.borrow_mut().text_document.is_none() {
        // Reload from disk
    }
}
```

## Next Steps

1. **Implement Phase 1** - LRU cache for FileInfoAst
2. **Add memory telemetry** - Use existing TrackingAllocator to log memory usage
3. **Benchmark** - Compare memory usage before/after with large Odoo codebase
4. **Iterate** - If insufficient, proceed to Phase 2

## References

- PR #154: Arena allocator (stalled draft)
- PR #477: OdooLS Spy memory visualizer
- Discussion #313: XML features request (unrelated but prioritized)
