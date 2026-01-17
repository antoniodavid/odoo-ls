#!/usr/bin/env python3
import subprocess
import json
import sys
import time
from pathlib import Path

BINARY = Path(__file__).parent / "target/release/odoo_ls_server"
CONFIG = "/home/adruban/Workspace/Odoo/O19/odools.toml"
TEST_FILE = "/home/adruban/Workspace/Odoo/O19/odoo/addons/sale/models/sale_order.py"

class LSPClient:
    def __init__(self):
        self.process = None
        self.msg_id = 0
        
    def start(self):
        self.process = subprocess.Popen(
            [str(BINARY), "--config-path", CONFIG],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=0
        )
        
    def send_request(self, method, params):
        self.msg_id += 1
        msg = {
            "jsonrpc": "2.0",
            "id": self.msg_id,
            "method": method,
            "params": params
        }
        content = json.dumps(msg)
        header = f"Content-Length: {len(content)}\r\n\r\n"
        self.process.stdin.write(header + content)
        self.process.stdin.flush()
        return self.msg_id
        
    def send_notification(self, method, params):
        msg = {
            "jsonrpc": "2.0",
            "method": method,
            "params": params
        }
        content = json.dumps(msg)
        header = f"Content-Length: {len(content)}\r\n\r\n"
        self.process.stdin.write(header + content)
        self.process.stdin.flush()
        
    def read_response(self, timeout=60):
        start = time.time()
        while time.time() - start < timeout:
            if self.process.poll() is not None:
                stderr = self.process.stderr.read()
                return None, stderr
                
            line = self.process.stdout.readline()
            if not line:
                time.sleep(0.1)
                continue
                
            if line.startswith("Content-Length:"):
                length = int(line.split(":")[1].strip())
                self.process.stdout.readline()
                content = self.process.stdout.read(length)
                return json.loads(content), None
                
        return None, "Timeout"
        
    def initialize(self):
        req_id = self.send_request("initialize", {
            "processId": None,
            "rootUri": "file:///home/adruban/Workspace/Odoo/O19",
            "capabilities": {}
        })
        
        response, err = self.read_response()
        if err:
            print(f"Initialize error: {err}")
            return False
            
        self.send_notification("initialized", {})
        return True
        
    def shutdown(self):
        if self.process:
            try:
                self.send_request("shutdown", None)
                time.sleep(1)
                self.send_notification("exit", None)
                self.process.wait(timeout=5)
            except:
                self.process.kill()
                
    def wait_for_indexing(self, seconds=45):
        print(f"Waiting {seconds}s for indexing...")
        time.sleep(seconds)

def test_scenario(name, expect_cache):
    print(f"\n{'='*60}")
    print(f"TEST: {name}")
    print(f"{'='*60}")
    
    cache_dir = Path.home() / ".local/share/odoo-ls/modules"
    
    print(f"Cache exists: {cache_dir.exists()}")
    if cache_dir.exists():
        cache_count = len(list(cache_dir.glob("*.bin")))
        print(f"Cache files: {cache_count}")
    
    print("\nStarting LSP server...")
    client = LSPClient()
    client.start()
    
    print("Initializing...")
    if not client.initialize():
        print("❌ Initialization failed")
        client.shutdown()
        return False
        
    print("✅ Initialized")
    
    client.wait_for_indexing(45)
    
    print("\nShutting down...")
    client.shutdown()
    
    print("\nChecking logs...")
    stderr = client.process.stderr.read() if client.process else ""
    
    log_a = stderr.count("[LOG A - CACHE]")
    log_b = stderr.count("[LOG B - CACHE]")
    log_c = stderr.count("[LOG C - CACHE]")
    loaded_from_cache = stderr.count("Loaded module") and "from cache" in stderr
    
    print(f"  [LOG A - CACHE]: {log_a}")
    print(f"  [LOG B - CACHE]: {log_b}")
    print(f"  [LOG C - CACHE]: {log_c}")
    print(f"  Modules loaded from cache: {'Yes' if loaded_from_cache else 'No'}")
    
    if expect_cache:
        if log_a > 0:
            print("  ✅ Cache being used for eval")
        else:
            print("  ❌ Cache NOT being used!")
            
        if log_b > 0:
            print("  ❌ Hash mismatches detected!")
        else:
            print("  ✅ No hash mismatches")
            
        if log_c > 0:
            print("  ✅ Eval completed")
        else:
            print("  ❌ Eval not completing!")
    else:
        if log_a == 0:
            print("  ✅ No cache on cold start (expected)")
        else:
            print("  ❌ Unexpected cache usage on cold start")
    
    if cache_dir.exists():
        cache_count = len(list(cache_dir.glob("*.bin")))
        print(f"\nCache after test: {cache_count} files")
    
    with open("test_lsp_stderr.log", "w") as f:
        f.write(stderr)
    print("\nFull stderr saved to: test_lsp_stderr.log")
    
    return True

def main():
    if not BINARY.exists():
        print(f"ERROR: Binary not found at {BINARY}")
        sys.exit(1)
        
    print("Clearing cache...")
    cache_dir = Path.home() / ".local/share/odoo-ls"
    if cache_dir.exists():
        import shutil
        shutil.rmtree(cache_dir)
    print("✅ Cache cleared\n")
    
    test_scenario("COLD START", expect_cache=False)
    
    print("\n\nPausing 5s between tests...")
    time.sleep(5)
    
    test_scenario("WARM START", expect_cache=True)
    
    print("\n" + "="*60)
    print("TESTS COMPLETE")
    print("="*60)

if __name__ == "__main__":
    main()
