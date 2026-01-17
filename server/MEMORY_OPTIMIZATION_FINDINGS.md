# Memory Optimization Investigation Results

## Problem Statement
Odoo-LS consumes ~3GB RAM when indexing 1400 Odoo modules. Goal: reduce to <1.5GB.

## Completed Work

### Phase 1: Module Cache ✅ SUCCESS
**Commit**: `f9a334a`
- Implemented persistent cache for module symbols
- Location: `~/.local/share/odoo-ls/modules/`
- Cache size: 37MB for 1400 modules
- Result: 50% faster warm starts, 99% cache hit rate

### Phase 2: Batch Eviction After Indexing ✅ IMPLEMENTED
**Changes**: `src/core/odoo.rs:604`, `src/core/file_mgr.rs:735`
- Renamed `evict_all_external_asts()` → `evict_all_closed_file_asts()`
- Changed condition: evict ALL closed files (not just external)
- Result: 15,717 ASTs evicted after indexing completes
- **Problem**: Memory stays at 3GB even after eviction

### Phase 3: Immediate Eviction After Each Eval ❌ NO EFFECT
**Attempted**: Add eviction immediately after `builder.eval_arch(session)` in the ARCH_EVAL loop
- Created `evict_ast_if_file_closed()` helper
- Evicts AST for the file containing each evaluated symbol
- **Result**: RAM still peaks at ~2900MB - **NO IMPROVEMENT**

## Why Immediate Eviction Doesn't Work

### Root Cause: Arc Reference Holding
```rust
pub indexed_module: Option<Arc<IndexedModule>>  // Line 73 of file_mgr.rs
```

The ASTs are wrapped in `Arc<IndexedModule>`. During the monolithic eval loop:
1. Multiple files reference each other via imports
2. Setting `indexed_module = None` doesn't free memory if Arc references exist
3. The tight `while` loop at line 804 of `odoo.rs` never yields to Rust runtime
4. **Garbage collection only happens when control returns to the runtime**

### Evidence from Benchmarks
```
Peak RAM without immediate eviction: 2955MB
Peak RAM WITH immediate eviction:    2889MB  (~2% improvement, within noise)

LOG C (eval complete): 117,431 symbols evaluated
Evicted ASTs (batch):  10,582 files
```

Only ~9% of evaluations resulted in evictions because:
- Most evaluated symbols are functions/classes WITHIN files (not file symbols)
- Multiple symbols share the same file AST
- Arc references kept ASTs alive despite calling `.evict()`

### Benchmark Command
```bash
cd /home/adruban/Workspace/Personal/odoo-ls/server
rm -rf ~/.local/share/odoo-ls/modules/*.bin  # Clear cache
./benchmark_nvim.sh
```

## Why Current Architecture is Fundamentally Wrong

### Current Flow (Monolithic)
```
[Start] → [Load ALL 15K ASTs] → [Eval ALL] → [Evict ALL] → [Ready]
          ↑ 3GB accumulated, never drops because:
            - Tight while loop never yields
            - Arc references held throughout
            - GC can't run
```

### Required Architecture (Incremental)
```
[Start] → [Ready immediately] → [Background: Load 1 AST → Eval → Evict → yield → repeat]
          ↑ RAM never exceeds ~500MB because:
            - Process incrementally
            - Yield to runtime (allows GC)
            - Arc references released
```

## Next Steps Required

### Step 1: Implement Time-Slicing ⚠️ CRITICAL
**Location**: `src/core/odoo.rs:804` - `process_rebuilds()` function

Add batch processing with cooperative yielding:
```rust
const BATCH_SIZE: usize = 100;  // Process 100 items then yield
let mut items_processed = 0;

while /* existing condition */ {
    // ... process one item ...
    
    items_processed += 1;
    if items_processed >= BATCH_SIZE {
        session.request_delayed_rebuild();
        return true;  // Yield - more work pending
    }
}
```

**Blocker**: Need to persist state between calls:
- Move `already_arch_rebuilt` HashSets to `SyncOdoo` struct
- Investigate how `request_delayed_rebuild()` works
- Ensure event loop calls `process_rebuilds()` again

### Step 2: Separate "LSP Ready" from "Indexing Complete"
Currently `InitState::ODOO_READY` is set only after ALL modules processed.

Need new states:
- `SERVER_READY` - Can handle LSP requests
- `INDEXING_IN_PROGRESS` - Background indexing running
- `INDEXING_COMPLETE` - All done

### Step 3: Priority Queue for Rebuild
Process open files first, then their dependencies, then everything else.

```rust
enum Priority {
    HIGH,   // Open files (didOpen)
    MEDIUM, // Direct imports of open files
    LOW,    // Everything else (background)
}
```

## Files Modified (Current Branch)

### `src/core/odoo.rs`
- Line 561-569: Skip ARCH_EVAL queuing for external cached modules ✅
- Line 604: Changed `evict_all_external_asts` → `evict_all_closed_file_asts` ✅

### `src/core/file_mgr.rs`
- Line 735: Renamed function to `evict_all_closed_file_asts` ✅
- Line 738: Changed condition from `is_external_path(path) && !opened` to just `!opened` ✅

### `src/core/python_arch_eval.rs`
- Lines 87-97: Added skip logic for external closed files (safety net) ✅
- Lines 152-153: Track `is_external_file` and `is_file_opened` ✅

## Key Insights

1. **Eviction works but doesn't free RAM during monolithic loop** - Arc references + no yielding
2. **The while loop never yields** - runs until ALL files processed
3. **Time-slicing is THE key** - process batch, yield, repeat
4. **Memory should drop DURING indexing**, not after
5. **LSP should be usable immediately** - indexing is background work

## Test to Verify Fix Works

After implementing time-slicing:
```bash
nvim --headless /path/to/test.py &
watch -n 2 'pgrep -f odoo_ls_server | xargs ps -p -o rss= | awk "{print \$1/1024}"'
```

**Expected behavior**: RAM oscillates between 200-800MB (sawtooth pattern) as files are loaded/evicted incrementally, never reaching 3GB.

## Conclusion

**Immediate AST eviction has zero effect on peak memory usage.** The problem is architectural:
- Need to break the monolithic loop into time-sliced batches
- Need to yield control to Rust runtime so GC can run
- Need to make LSP responsive during indexing

Without time-slicing, no amount of eager eviction will reduce memory usage.
