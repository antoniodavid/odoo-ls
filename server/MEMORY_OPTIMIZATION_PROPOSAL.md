# Memory Optimization Proposal for Odoo-LS

**Branch:** `feature/memory-optimization-v2`
**Base:** `alpha` (commit `56eb111`)
**Date:** 2026-01-15

---

## 1. Current Architecture Analysis

### 1.1 Main Data Structures

The server maintains several large data structures in memory:

| Structure | Location | Description | Memory Impact |
|-----------|----------|-------------|---------------|
| `FileMgr.files` | `file_mgr.rs:479` | HashMap of all FileInfo | HIGH - stores all parsed files |
| `FileInfoAst.indexed_module` | `file_mgr.rs:71` | Arc<IndexedModule> with full AST | VERY HIGH - full Python AST per file |
| `FileInfoAst.text_document` | `file_mgr.rs:70` | Source text + line index | HIGH - full source code |
| `Symbol (various)` | `symbols/*.rs` | Class/Function/Variable definitions | MEDIUM-HIGH |
| `Evaluation` | `evaluation.rs:72` | Type evaluations with AST refs | MEDIUM |
| `Model` | `model.rs` | Odoo model registry | LOW-MEDIUM |

### 1.2 Memory Flow

```
Project Load
    │
    ▼
┌─────────────────────────────────────────────────────────────┐
│  For EACH .py file in Odoo + addons (~3000+ files):         │
│    1. Read file content → TextDocument (full source)        │
│    2. Parse → IndexedModule (full AST tree)                 │
│    3. Build symbols → FileSymbol/ClassSymbol/FunctionSymbol │
│    4. Evaluate → Evaluations with type info                 │
│    5. ALL kept in memory permanently                        │
└─────────────────────────────────────────────────────────────┘
    │
    ▼
Memory Usage: 2-4 GB for typical Odoo 17 installation
```

### 1.3 Key Observation

**80% of files are "external"** (Odoo core, enterprise, OCA modules) that:
- Are never modified by the user
- Are only needed for type resolution and completion
- Their full AST is rarely accessed after initial parsing

---

## 2. Proposed Optimizations

### Phase 1: AST Eviction for External Files (LOW RISK)

**Problem:** Every parsed file keeps its full AST in memory forever.

**Solution:** Evict AST from external files after they've been processed.

**Implementation:**
```rust
// In FileInfoAst
pub fn evict(&mut self) {
    self.text_document = None;
    self.indexed_module = None;
}

pub fn is_evicted(&self) -> bool {
    self.indexed_module.is_none()
}
```

**When to evict:**
- After `ARCH_EVAL` step completes for a file
- Only for files marked `is_external = true`
- Never for files in workspace or currently open

**When to reload:**
- On-demand when AST access is needed (e.g., go-to-definition)
- Lazy re-parse from disk

**Expected savings:** ~60-70% of AST memory

**Risk:** LOW - ASTs can always be re-parsed from disk

---

### Phase 2: LRU Cache for File Access (LOW RISK)

**Problem:** All FileInfo objects stay in memory.

**Solution:** Use LRU cache for external file access patterns.

**Implementation:**
```rust
// In FileMgr
pub struct FileMgr {
    pub files: HashMap<String, Rc<RefCell<FileInfo>>>,
    ast_lru: LruCache<String, ()>,  // Track recently used
    // ...
}

pub fn touch_file(&mut self, path: &str) {
    if self.is_external(path) {
        if let Some((evicted, _)) = self.ast_lru.push(path.to_string(), ()) {
            // Evict AST from least recently used file
            if let Some(file) = self.files.get(&evicted) {
                file.borrow_mut().file_info_ast.borrow_mut().evict();
            }
        }
    }
}
```

**Configuration:** LRU size of 256-512 files (configurable)

**Expected savings:** Bounds memory growth

**Risk:** LOW - only affects AST storage, not symbols

---

### Phase 3: Lazy Symbol Loading (MEDIUM RISK)

**Problem:** All symbols for all files are built upfront.

**Solution:** Build symbols on-demand when accessed.

**Implementation:**
```rust
// In ModuleSymbol/PythonPackageSymbol
pub loaded: bool,  // Track if children are loaded

// In Symbol
pub fn ensure_loaded(symbol: &Rc<RefCell<Symbol>>, session: &mut SessionInfo) {
    if symbol.borrow().is_loaded() {
        return;
    }
    // Mark as loading FIRST to prevent recursion
    symbol.borrow_mut().set_loaded(true);
    
    // Then build
    SyncOdoo::build_now(session, symbol, BuildSteps::ARCH);
    SyncOdoo::build_now(session, BuildSteps::ARCH_EVAL);
}
```

**Critical:** Must mark `loaded = true` BEFORE building to prevent infinite recursion.

**When to load:**
- When `get_member_symbol()` is called on an external symbol
- When completing `_inherit = "module."`

**Expected savings:** ~40-50% of symbol memory for unused modules

**Risk:** MEDIUM - requires careful recursion prevention

---

### Phase 4: Evaluation Pruning (MEDIUM RISK)

**Problem:** Evaluations store AST references that keep ASTs alive.

**Solution:** After evaluation, store only the resolved type reference.

**Current:**
```rust
pub struct Evaluation {
    pub symbol: EvaluationSymbol,  // Type reference
    pub value: Option<EvaluationValue>,  // AST nodes
    pub range: Option<TextRange>,
}
```

**Optimized:**
```rust
pub struct Evaluation {
    pub symbol: EvaluationSymbol,
    pub value: Option<EvaluationValue>,
    pub range: Option<TextRange>,
    pub ast_evicted: bool,  // Track if AST data was cleaned
}

impl Evaluation {
    pub fn evict_ast_data(&mut self) {
        if let Some(EvaluationValue::CONSTANT(_)) = &self.value {
            // Keep constants as they're small
        } else {
            self.value = None;
        }
        self.ast_evicted = true;
    }
}
```

**Expected savings:** ~20-30% of evaluation memory

**Risk:** MEDIUM - some features may need AST access

---

## 3. Implementation Order & Safety

### Recommended Order:

1. **Phase 1** (AST Eviction) - Safest, biggest impact
2. **Phase 2** (LRU Cache) - Builds on Phase 1
3. **Phase 3** (Lazy Loading) - More complex, requires testing
4. **Phase 4** (Evaluation Pruning) - Optional, lower priority

### Safety Measures:

1. **Feature flag:** Add `memory_optimization: bool` to config
2. **Incremental:** Each phase is independent and can be reverted
3. **Testing:** Run completion/hover/definition tests after each phase
4. **Metrics:** Add memory logging to track actual savings

---

## 4. What NOT to Do

Based on the failed `fix/xml-context-completion` branch:

1. **DON'T** skip adding modules to `rebuild_arch` (broke sync)
2. **DON'T** call `build_now` inside `ensure_loaded` after checking `is_loaded` (causes stack overflow)
3. **DON'T** use disk cache (bincode) without version checking (causes panics)
4. **DON'T** change `get_member_symbol` signature without updating all callers

---

## 5. Expected Results

| Metric | Current (alpha) | Target |
|--------|-----------------|--------|
| Memory (Odoo 17 + enterprise) | ~2.5 GB | ~800 MB - 1.2 GB |
| Initial sync time | ~5 min | ~5 min (no change) |
| Completion latency | ~100ms | ~150ms (acceptable) |
| Go-to-definition latency | ~50ms | ~200ms (if re-parse needed) |

---

## 6. Next Steps

1. **Approve this proposal** or request modifications
2. **Implement Phase 1** with tests
3. **Verify completion still works** with Odoo project
4. **Measure actual memory savings**
5. **Proceed to Phase 2** if successful

---

## Questions for Review

1. Is the LRU cache size of 256 files acceptable?
2. Should we add a config option to disable optimizations?
3. Are there specific features that MUST have instant access to ASTs?
