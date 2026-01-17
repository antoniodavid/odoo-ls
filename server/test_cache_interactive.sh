#!/bin/bash
set -e

CACHE_DIR="$HOME/.local/share/odoo-ls/modules"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BINARY="$SCRIPT_DIR/target/release/odoo_ls_server"
CONFIG="/home/adruban/Workspace/Odoo/O19/odools.toml"
LOG_FILE="$SCRIPT_DIR/test_lsp.log"

if [ ! -f "$BINARY" ]; then
    echo "ERROR: Binary not found at $BINARY"
    exit 1
fi

if [ ! -f "$CONFIG" ]; then
    echo "ERROR: Config not found at $CONFIG"
    exit 1
fi

echo "=== INTERACTIVE CACHE TEST ==="
echo ""

run_test() {
    local test_name="$1"
    local expect_cache="$2"
    
    echo "=== TEST: $test_name ==="
    echo "Cache dir exists: $([ -d "$CACHE_DIR" ] && echo "YES" || echo "NO")"
    if [ -d "$CACHE_DIR" ]; then
        echo "Cache files: $(ls "$CACHE_DIR" 2>/dev/null | wc -l)"
    fi
    
    echo ""
    echo "Starting LSP server (will run for 60s to allow indexing)..."
    
    timeout 60 "$BINARY" --config "$CONFIG" > "$LOG_FILE" 2>&1 &
    local pid=$!
    
    echo "LSP PID: $pid"
    echo "Waiting for indexing (45 seconds)..."
    sleep 45
    
    echo ""
    echo "Checking logs for cache markers..."
    echo "  [LOG A - CACHE] count: $(grep -c '\[LOG A - CACHE\]' "$LOG_FILE" 2>/dev/null || echo 0)"
    echo "  [LOG B - CACHE] count: $(grep -c '\[LOG B - CACHE\]' "$LOG_FILE" 2>/dev/null || echo 0)"
    echo "  [LOG C - CACHE] count: $(grep -c '\[LOG C - CACHE\]' "$LOG_FILE" 2>/dev/null || echo 0)"
    
    if [ "$expect_cache" = "no" ]; then
        local log_a=$(grep -c '\[LOG A - CACHE\]' "$LOG_FILE" 2>/dev/null || echo 0)
        if [ "$log_a" -gt 0 ]; then
            echo "  ❌ UNEXPECTED: Found LOG A entries on cold start"
        else
            echo "  ✅ Correct: No LOG A entries on cold start"
        fi
    else
        local log_a=$(grep -c '\[LOG A - CACHE\]' "$LOG_FILE" 2>/dev/null || echo 0)
        local log_b=$(grep -c '\[LOG B - CACHE\]' "$LOG_FILE" 2>/dev/null || echo 0)
        local log_c=$(grep -c '\[LOG C - CACHE\]' "$LOG_FILE" 2>/dev/null || echo 0)
        
        if [ "$log_a" -gt 0 ]; then
            echo "  ✅ LOG A present: Files entering eval from cache"
        else
            echo "  ❌ LOG A missing: Cache not being used!"
        fi
        
        if [ "$log_b" -gt 0 ]; then
            echo "  ❌ LOG B present: Hash mismatches detected!"
            grep '\[LOG B - CACHE\]' "$LOG_FILE" | head -5
        else
            echo "  ✅ LOG B absent: No hash mismatches"
        fi
        
        if [ "$log_c" -gt 0 ]; then
            echo "  ✅ LOG C present: Eval completed successfully"
        else
            echo "  ❌ LOG C missing: Eval not completing!"
        fi
    fi
    
    echo ""
    echo "Stopping LSP server..."
    kill $pid 2>/dev/null || true
    wait $pid 2>/dev/null || true
    
    echo "Cache after test:"
    if [ -d "$CACHE_DIR" ]; then
        echo "  Files cached: $(ls "$CACHE_DIR" 2>/dev/null | wc -l)"
    else
        echo "  No cache directory"
    fi
    echo ""
}

echo "Step 1: Clear cache"
rm -rf "$HOME/.local/share/odoo-ls"
echo "  ✅ Cache cleared"
echo ""

run_test "COLD START" "no"

echo "=== Pausing between tests (5s) ==="
sleep 5
echo ""

run_test "WARM START" "yes"

echo "=== TEST COMPLETE ==="
echo ""
echo "Full logs available at: $LOG_FILE"
echo ""
echo "To check logs manually:"
echo "  grep 'LOG.*CACHE' $LOG_FILE"
echo "  grep -E '(Loaded module.*from cache|Registered.*files from cache)' $LOG_FILE"
