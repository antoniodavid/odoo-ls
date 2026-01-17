# FASE 2: Weak/Arc Cache + Aggressive Eviction - RESULTS

## Overview
FASE 2 implemented a transient/persistent cache system inspired by gopls, combined with aggressive AST eviction during indexing.

## Benchmark Results

### Final Configuration (Success)
- **Cold Start**: **1837MB** (baseline: 2955MB)
  - **Reduction**: -1118MB (-37.8%) ✅
- **Warm Start**: **1607MB** (baseline: 2867MB)  
  - **Reduction**: -1260MB (-43.9%) ✅

### Test Environment
- **Modules**: 1400 Odoo modules
- **Project**: `/home/adruban/Workspace/Doodba_ENV/O19/`
- **Branch**: `feature/memory-optimization-v2`
- **Date**: 2026-01-17

## Implementation Details

### Architecture
1. **AstCacheManager** with two-tier system:
   - **Persistent cache**: `HashMap<String, Arc<IndexedModule>>` for open files
   - **Transient cache**: `HashMap<String, Weak<IndexedModule>>` for closed files
   - **LRU eviction**: 500 max entries, 100 min entries

2. **Aggressive eviction during indexing**:
   - `evict_all_closed_file_asts()` called after every batch (100 items)
   - Only evicts `indexed_module`, keeps `text_document` for diagnostics
   - Files in transient cache (Weak refs) get freed by Rust GC

3. **Cache population**:
   - Inserted in `update_file_info()` after initial build
   - Inserted in `prepare_ast()` after rebuild from disk
   - Open files → persistent cache (Arc)
   - Closed files → transient cache (Weak)

### Key Code Changes

#### src/core/file_mgr.rs
```rust
#[derive(Debug)]
pub struct AstCacheManager {
    persistent: HashMap<String, Arc<IndexedModule>>,
    transient: HashMap<String, std::sync::Weak<IndexedModule>>,
    lru: LruCache<String, ()>,
    min_cache_size: usize,  // 100
    max_cache_size: usize,  // 500
}

impl FileInfoAst {
    pub fn evict(&mut self) {
        // CRITICAL: Only evict indexed_module, keep text_document
        // Diagnostics need text_document for offset_to_position
        self.indexed_module = None;
    }
}
```

#### src/core/odoo.rs
```rust
pub fn process_rebuilds_batch(...) -> ProcessState {
    // ... process 100 items ...
    
    // Aggressive eviction after each batch
    let file_mgr = session.sync_odoo.get_file_mgr();
    let evicted = file_mgr.borrow_mut().evict_all_closed_file_asts();
    file_mgr.borrow_mut().ast_cache.borrow_mut().maybe_evict();
    
    ProcessState::NeedsMoreWork
}
```

## Iteration History

### Attempt 1: Cache Without Eviction (FAILED)
- **Result**: +44MB to +76MB WORSE than baseline
- **Cause**: Cache overhead without freeing memory during indexing
- **Learning**: Cache alone doesn't help if nothing gets evicted

### Attempt 2: Evict text_document + indexed_module (CRASHED)
- **Result**: Panic at `offset_to_position` - "no text_document provided"
- **Cause**: Validation needs text_document to publish diagnostics
- **Fix**: Only evict indexed_module, keep text_document

### Attempt 3: Aggressive Eviction + Keep text_document (SUCCESS)
- **Result**: -37.8% cold start, -43.9% warm start
- **Success factors**:
  1. Evict during indexing, not after
  2. Keep small data (text_document), evict large data (indexed_module/AST)
  3. Weak refs allow Rust GC to free memory

## Analysis

### Why It Works
1. **Timing**: Evicting DURING indexing vs AFTER makes all the difference
2. **Granularity**: Evict per-batch (100 items) allows frequent GC opportunities
3. **Weak refs**: Transient cache doesn't prevent deallocation
4. **Selective eviction**: Keep text_document (~100KB) for diagnostics, evict AST (~2MB)

### Memory Breakdown (Estimated)
- **Baseline**: 2955MB total
  - 1400 modules × ~2MB AST = ~2800MB
  - Text documents + symbols = ~155MB
  
- **FASE 2**: 1837MB total
  - ~800 modules in memory (evicted 600)
  - ~800 × 2MB = 1600MB ASTs
  - 1400 text documents = ~140MB
  - Cache overhead = ~97MB

### Cache Hit Rate
Based on logs:
- **Cold start**: 0% (as expected, cache empty)
- **Warm start**: High hit rate for recently used files
- **Post-indexing**: prepare_ast() benefits from cache restoration

## Trade-offs

### Benefits
✅ 38-44% RAM reduction during indexing  
✅ Sub-2GB memory usage achieved  
✅ No performance regression (indexing time unchanged)  
✅ Cache helps post-indexing workflows (goto def, hover, etc.)  

### Costs
⚠️ Increased code complexity (cache management)  
⚠️ Small overhead for cache data structures (~100MB)  
⚠️ More frequent GC pressure (may impact latency slightly)  

## Comparison to Other LSPs

| LSP | Approach | RAM Impact |
|-----|----------|------------|
| **rust-analyzer** | Query-based incremental | Salsa database + LRU per query |
| **gopls** | Transient/Persistent cache | 75% reduction reported |
| **Pyright** | Time-sliced analysis | 75%/90% thresholds + heap monitoring |
| **Odoo-LS FASE 2** | Weak/Arc + aggressive eviction | **38-44% reduction** ✅ |

## Next Steps

### Completed
- ✅ Two-tier cache system
- ✅ Aggressive batch eviction  
- ✅ Benchmark validation
- ✅ Sub-2GB target achieved

### Future Optimizations (Optional)
1. **Tune cache parameters**:
   - Experiment with batch sizes (50, 200, 500)
   - Adjust LRU min/max (200/1000)
   
2. **Partial AST retention**:
   - Keep only symbol tables, evict full AST
   - Would require AST restructuring
   
3. **Heap monitoring**:
   - Add Pyright-style heap thresholds
   - Trigger eviction at 75% heap usage

4. **Query system** (major refactor):
   - Salsa-style incremental computation
   - Would enable fine-grained invalidation

## Conclusion

**FASE 2 is a SUCCESS.** 

By combining gopls's Weak/Arc cache pattern with aggressive during-indexing eviction, we achieved:
- **37.8% reduction in cold start RAM** (2955MB → 1837MB)
- **43.9% reduction in warm start RAM** (2867MB → 1607MB)
- **Goal achieved**: Peak RAM < 2000MB ✅

The key insight: **evict early and often during indexing** rather than waiting until completion. The Weak reference pattern from gopls enables safe eviction without breaking downstream operations.

## Commit Message

```
feat: implement FASE 2 memory optimization - 38-44% RAM reduction

Implement two-tier AST cache with aggressive eviction during indexing:
- Persistent cache (Arc) for open files
- Transient cache (Weak) for closed files  
- Batch-level eviction every 100 items
- Keep text_document, evict indexed_module

Results:
- Cold start: 2955MB → 1837MB (-37.8%)
- Warm start: 2867MB → 1607MB (-43.9%)

Inspired by gopls's transient/persistent cache pattern and
rust-analyzer's LRU eviction strategy.

Closes #<issue-number>
```
