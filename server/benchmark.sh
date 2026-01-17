#!/bin/bash
set -e

BINARY="$HOME/.local/share/nvim/odoo/odoo_ls_server"
CONFIG_DIR="/home/adruban/Workspace/Doodba_ENV/O19"
CONFIG_FILE="$CONFIG_DIR/odools.toml"
CACHE_DIR="$HOME/.local/share/odoo-ls"
LOG_DIR="$HOME/.local/share/nvim/odoo/logs"
RESULTS_FILE="/tmp/odoo_ls_benchmark_$(date +%Y%m%d_%H%M%S).txt"

echo "=== ODOO-LS BENCHMARK ===" | tee "$RESULTS_FILE"
echo "Date: $(date)" | tee -a "$RESULTS_FILE"
echo "Binary: $BINARY" | tee -a "$RESULTS_FILE"
echo "Config: $CONFIG_FILE" | tee -a "$RESULTS_FILE"
echo "" | tee -a "$RESULTS_FILE"

if [ ! -f "$BINARY" ]; then
    echo "ERROR: Binary not found at $BINARY" | tee -a "$RESULTS_FILE"
    exit 1
fi

if [ ! -f "$CONFIG_FILE" ]; then
    echo "ERROR: Config not found at $CONFIG_FILE" | tee -a "$RESULTS_FILE"
    exit 1
fi

measure_memory() {
    local pid=$1
    local max_mem=0
    local sample_count=0
    while kill -0 $pid 2>/dev/null; do
        local mem=$(ps -o rss= -p $pid 2>/dev/null | tr -d ' ')
        if [ -n "$mem" ] && [ "$mem" -gt "$max_mem" ]; then
            max_mem=$mem
        fi
        sample_count=$((sample_count + 1))
        sleep 1
    done
    echo $((max_mem / 1024))
}

run_test() {
    local test_name=$1
    local timeout_sec=$2
    
    echo "--- TEST: $test_name ---" | tee -a "$RESULTS_FILE"
    echo "Cache files before: $(ls $CACHE_DIR/modules/*.bin 2>/dev/null | wc -l)" | tee -a "$RESULTS_FILE"
    
    local start_time=$(date +%s)
    
    cd "$CONFIG_DIR"
    timeout $timeout_sec "$BINARY" --config-path "$CONFIG_FILE" > /tmp/odoo_ls_stdout.log 2>&1 &
    local pid=$!
    
    echo "Started LSP server (PID: $pid), measuring memory..." | tee -a "$RESULTS_FILE"
    
    local max_mem=$(measure_memory $pid)
    wait $pid 2>/dev/null || true
    local exit_code=$?
    
    local end_time=$(date +%s)
    local duration=$((end_time - start_time))
    
    echo "Duration: ${duration}s" | tee -a "$RESULTS_FILE"
    echo "Peak RAM: ${max_mem}MB" | tee -a "$RESULTS_FILE"
    echo "Exit code: $exit_code" | tee -a "$RESULTS_FILE"
    echo "Cache files after: $(ls $CACHE_DIR/modules/*.bin 2>/dev/null | wc -l)" | tee -a "$RESULTS_FILE"
    
    local cache_size=$(du -sh $CACHE_DIR 2>/dev/null | cut -f1)
    echo "Cache size: $cache_size" | tee -a "$RESULTS_FILE"
    
    local latest_log=$(ls -1t $LOG_DIR/*.log 2>/dev/null | head -1)
    if [ -f "$latest_log" ]; then
        echo "Latest log: $latest_log" | tee -a "$RESULTS_FILE"
        echo "LOG A count: $(grep -c '\[LOG A - CACHE\]' "$latest_log" 2>/dev/null || echo 0)" | tee -a "$RESULTS_FILE"
        echo "LOG B count: $(grep -c '\[LOG B - CACHE\]' "$latest_log" 2>/dev/null || echo 0)" | tee -a "$RESULTS_FILE"
        echo "LOG C count: $(grep -c '\[LOG C - CACHE\]' "$latest_log" 2>/dev/null || echo 0)" | tee -a "$RESULTS_FILE"
        echo "Modules loaded from cache: $(grep -c 'Loaded module.*from cache' "$latest_log" 2>/dev/null || echo 0)" | tee -a "$RESULTS_FILE"
        echo "Modules saved to cache: $(grep -c 'Saved module cache' "$latest_log" 2>/dev/null || echo 0)" | tee -a "$RESULTS_FILE"
        echo "Files registered from cache: $(grep 'Registered.*files from cache' "$latest_log" 2>/dev/null | wc -l)" | tee -a "$RESULTS_FILE"
        
        local indexing_complete=$(grep -c 'End building modules' "$latest_log" 2>/dev/null || echo 0)
        echo "Indexing completed: $([ $indexing_complete -gt 0 ] && echo 'YES' || echo 'NO')" | tee -a "$RESULTS_FILE"
    fi
    echo "" | tee -a "$RESULTS_FILE"
}

echo "=== COLD START TEST ===" | tee -a "$RESULTS_FILE"
rm -rf "$CACHE_DIR"
mkdir -p "$CACHE_DIR/modules"
echo "Cache cleared" | tee -a "$RESULTS_FILE"
run_test "Cold Start" 180

echo "=== Waiting 5 seconds between tests ===" | tee -a "$RESULTS_FILE"
sleep 5

echo "=== WARM START TEST ===" | tee -a "$RESULTS_FILE"
run_test "Warm Start" 120

echo "" | tee -a "$RESULTS_FILE"
echo "=== RESULTS SUMMARY ===" | tee -a "$RESULTS_FILE"
echo "Full results saved to: $RESULTS_FILE"
echo "" | tee -a "$RESULTS_FILE"

echo "To analyze logs manually:"
echo "  grep 'LOG.*CACHE' $(ls -1t $LOG_DIR/*.log | head -1)"
echo "  grep 'Loaded module.*from cache' $(ls -1t $LOG_DIR/*.log | head -1)"
