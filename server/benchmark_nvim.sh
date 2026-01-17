#!/bin/bash
set -e

CONFIG_DIR="/home/adruban/Workspace/Doodba_ENV/O19"
TEST_FILE="$CONFIG_DIR/odoo/custom/src/odoo/addons/sale/models/sale_order.py"
CACHE_DIR="$HOME/.local/share/odoo-ls"
LOG_DIR="$HOME/.local/share/nvim/odoo/logs"
RESULTS_FILE="/tmp/odoo_ls_benchmark_$(date +%Y%m%d_%H%M%S).txt"

echo "=== ODOO-LS BENCHMARK (Neovim-based) ===" | tee "$RESULTS_FILE"
echo "Date: $(date)" | tee -a "$RESULTS_FILE"
echo "Test file: $TEST_FILE" | tee -a "$RESULTS_FILE"
echo "" | tee -a "$RESULTS_FILE"

if [ ! -f "$TEST_FILE" ]; then
    echo "ERROR: Test file not found at $TEST_FILE" | tee -a "$RESULTS_FILE"
    exit 1
fi

monitor_lsp_memory() {
    local test_name=$1
    local duration=$2
    
    local max_mem=0
    local start_time=$(date +%s)
    
    while [ $(($(date +%s) - start_time)) -lt $duration ]; do
        local pid=$(pgrep -f "odoo_ls_server.*config-path" 2>/dev/null | head -1)
        if [ -n "$pid" ]; then
            local mem=$(ps -o rss= -p $pid 2>/dev/null | tr -d ' ')
            if [ -n "$mem" ] && [ "$mem" -gt "$max_mem" ]; then
                max_mem=$mem
                echo "[$test_name] Current RAM: $((mem / 1024))MB (PID: $pid)" | tee -a "$RESULTS_FILE"
            fi
        fi
        sleep 2
    done
    
    echo $((max_mem / 1024))
}

run_nvim_test() {
    local test_name=$1
    local wait_time=$2
    
    echo "--- TEST: $test_name ---" | tee -a "$RESULTS_FILE"
    echo "Cache files before: $(ls $CACHE_DIR/modules/*.bin 2>/dev/null | wc -l)" | tee -a "$RESULTS_FILE"
    
    local before_log=$(ls -1t $LOG_DIR/*.log 2>/dev/null | head -1)
    
    cd "$CONFIG_DIR"
    
    nvim --headless "$TEST_FILE" \
        -c "lua vim.wait(${wait_time}000, function() return false end)" \
        -c "qa!" &
    
    local nvim_pid=$!
    echo "Started Neovim (PID: $nvim_pid)" | tee -a "$RESULTS_FILE"
    
    sleep 2
    
    local max_mem=$(monitor_lsp_memory "$test_name" $wait_time)
    
    wait $nvim_pid 2>/dev/null || true
    
    echo "Peak RAM: ${max_mem}MB" | tee -a "$RESULTS_FILE"
    echo "Cache files after: $(ls $CACHE_DIR/modules/*.bin 2>/dev/null | wc -l)" | tee -a "$RESULTS_FILE"
    
    local cache_size=$(du -sh $CACHE_DIR 2>/dev/null | cut -f1)
    echo "Cache size: $cache_size" | tee -a "$RESULTS_FILE"
    
    sleep 3
    
    local latest_log=$(ls -1t $LOG_DIR/*.log 2>/dev/null | head -1)
    if [ -f "$latest_log" ] && [ "$latest_log" != "$before_log" ]; then
        echo "New log: $latest_log" | tee -a "$RESULTS_FILE"
        
        local log_a=$(grep -c '\[LOG A - CACHE\]' "$latest_log" 2>/dev/null || echo 0)
        local log_b=$(grep -c '\[LOG B - CACHE\]' "$latest_log" 2>/dev/null || echo 0)
        local log_c=$(grep -c '\[LOG C - CACHE\]' "$latest_log" 2>/dev/null || echo 0)
        local loaded=$(grep -c 'Loaded module.*from cache' "$latest_log" 2>/dev/null || echo 0)
        local saved=$(grep -c 'Saved module cache' "$latest_log" 2>/dev/null || echo 0)
        local registered=$(grep 'Registered.*files from cache' "$latest_log" 2>/dev/null | wc -l)
        local indexing=$(grep -c 'End building modules' "$latest_log" 2>/dev/null || echo 0)
        local total_modules=$(grep 'End building modules' "$latest_log" 2>/dev/null | grep -oP '\d+(?= modules loaded)' || echo 0)
        
        echo "  LOG A (eval from cache): $log_a" | tee -a "$RESULTS_FILE"
        echo "  LOG B (hash mismatch): $log_b" | tee -a "$RESULTS_FILE"
        echo "  LOG C (eval complete): $log_c" | tee -a "$RESULTS_FILE"
        echo "  Modules loaded from cache: $loaded" | tee -a "$RESULTS_FILE"
        echo "  Modules saved to cache: $saved" | tee -a "$RESULTS_FILE"
        echo "  Files registered from cache: $registered" | tee -a "$RESULTS_FILE"
        echo "  Total modules: $total_modules" | tee -a "$RESULTS_FILE"
        echo "  Indexing completed: $([ $indexing -gt 0 ] && echo 'YES' || echo 'NO')" | tee -a "$RESULTS_FILE"
    else
        echo "WARNING: No new log created" | tee -a "$RESULTS_FILE"
    fi
    echo "" | tee -a "$RESULTS_FILE"
}

echo "=== COLD START TEST ===" | tee -a "$RESULTS_FILE"
rm -rf "$CACHE_DIR"
mkdir -p "$CACHE_DIR/modules"
echo "Cache cleared" | tee -a "$RESULTS_FILE"
run_nvim_test "Cold Start" 120

echo "=== Waiting 10 seconds between tests ===" | tee -a "$RESULTS_FILE"
sleep 10

echo "=== WARM START TEST ===" | tee -a "$RESULTS_FILE"
run_nvim_test "Warm Start" 60

echo "" | tee -a "$RESULTS_FILE"
echo "=== RESULTS SUMMARY ===" | tee -a "$RESULTS_FILE"
echo "Full results saved to: $RESULTS_FILE"
cat "$RESULTS_FILE"
