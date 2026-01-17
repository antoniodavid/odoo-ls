#!/bin/bash
set -e

echo "=== CACHE SCENARIO TEST ==="
echo ""

CACHE_DIR="$HOME/.local/share/odoo-ls/modules"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BINARY="$SCRIPT_DIR/target/release/odoo_ls_server"

if [ ! -f "$BINARY" ]; then
    echo "ERROR: Binary not found at $BINARY"
    echo "Run: cargo build --release"
    exit 1
fi

echo "Step 1: Clear cache"
rm -rf "$HOME/.local/share/odoo-ls"
echo "  ✓ Cache cleared"

echo ""
echo "Step 2: Check cache directory after clear"
if [ -d "$CACHE_DIR" ]; then
    echo "  ✗ Cache directory exists (should not exist)"
    exit 1
else
    echo "  ✓ Cache directory does not exist"
fi

echo ""
echo "Step 3: Instructions for manual testing"
echo ""
echo "COLD START TEST:"
echo "  1. Open VS Code in /home/adruban/Workspace/Odoo/O19/"
echo "  2. Open any .py file from an Odoo module"
echo "  3. Wait for indexing to complete"
echo "  4. Test completion (Ctrl+Space) in a Python file"
echo "  5. Test go-to-definition (F12) on a model/method"
echo "  6. Check logs for '[LOG A - CACHE]' entries (should NOT appear on cold start)"
echo "  7. Verify: ls ~/.local/share/odoo-ls/modules/ | wc -l"
echo "     Expected: ~982 .bin files"
echo ""
echo "WARM START TEST:"
echo "  1. Close VS Code"
echo "  2. Reopen VS Code in /home/adruban/Workspace/Odoo/O19/"
echo "  3. Open the same .py file"
echo "  4. Wait for indexing (should be FASTER)"
echo "  5. Test completion (Ctrl+Space) - SHOULD WORK"
echo "  6. Test go-to-definition (F12) - SHOULD WORK"
echo "  7. Check logs for:"
echo "     - '[LOG A - CACHE]' entries (files entering eval_arch from cache)"
echo "     - '[LOG B - CACHE]' entries (SHOULD NOT APPEAR - means hash mismatch)"
echo "     - '[LOG C - CACHE]' entries (successful eval completions)"
echo ""
echo "SUCCESS CRITERIA:"
echo "  - Cold start: completion & goto work, cache files created"
echo "  - Warm start: faster startup, completion & goto STILL work"
echo "  - Logs show: LOG A present, LOG B absent, LOG C present"
echo ""
