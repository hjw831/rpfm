
import subprocess
import json
import time
import sys

def send_request(process, method, params=None, request_id=1):
    request = {
        "jsonrpc": "2.0",
        "id": request_id,
        "method": method,
        "params": params or {}
    }
    payload = json.dumps(request)
    print(f"DEBUG: Sending request {request_id} -> {method}")
    process.stdin.write(payload + "\n")
    process.stdin.flush()
    
    # Read response with timeout
    start_time = time.time()
    while time.time() - start_time < 5:
        line = process.stdout.readline()
        if line:
            trimmed = line.strip()
            if trimmed:
                print(f"DEBUG: Received response: {trimmed}")
                try:
                    return json.loads(trimmed)
                except json.JSONDecodeError:
                    return {"error": "Invalid JSON", "raw": trimmed}
        time.sleep(0.1)
    
    print(f"DEBUG: Timeout waiting for request {request_id}")
    return {"error": "Timeout"}

def main():
    cmd = ["C:/Users/jw900/VScodeProjects/rpfm/target/release/rpfm_server.exe", "--mcp-stdio"]
    process = subprocess.Popen(
        cmd,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE, # Separated for better debugging
        text=True,
        bufsize=1
    )

    try:
        # Read stderr in a separate thread or non-blocking way? 
        # For now, let's just try the sequence.
        
        # 1. Initialize Handshake
        init_req = {
            "jsonrpc": "2.0",
            "id": 0,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "qwen-manual-client", "version": "1.0.0"}
            }
        }
        print("DEBUG: Initializing...")
        process.stdin.write(json.dumps(init_req) + "\n")
        process.stdin.flush()
        
        # Give time for init response
        time.sleep(1)
        while True:
            line = process.stdout.readline()
            if not line: break
            print(f"DEBUG: Init-phase output: {line.strip()}")
            if '"id": 0' in line: break

        # 2. set_game
        res1 = send_request(process, "tools/call", {"name": "set_game", "arguments": {"game": "warhammer3"}}, 1)
        
        # 3. open_pack
        res2 = send_request(process, "tools/call", {"name": "open_pack", "arguments": {"pack_path": "C:\\GAME\\TWmod\\originals\\tenzin_fps.pack"}}, 2)
        
        # 4. list_files
        res3 = send_request(process, "tools/call", {"name": "list_files", "arguments": {}}, 3)
        
        # Save to a file in a more reliable way
        with open("C:/Users/jw900/VScodeProjects/rpfm/rpfm_result.txt", "w", encoding="utf-8") as f:
            f.write(f"Set Game: {json.dumps(res1, indent=2)}\n\n")
            f.write(f"Open Pack: {json.dumps(res2, indent=2)}\n\n")
            f.write(f"List Files: {json.dumps(res3, indent=2)}\n")

    except Exception as e:
        with open("C:/Users/jw900/VScodeProjects/rpfm/rpfm_result.txt", "w", encoding="utf-8") as f:
            f.write(f"Error occurred: {str(e)}")
    finally:
        # Capture any remaining stderr
        stderr_out = process.stderr.read()
        with open("C:/Users/jw900/VScodeProjects/rpfm/rpfm_stderr.txt", "w", encoding="utf-8") as f:
            f.write(stderr_out)
        process.terminate()

if __name__ == "__main__":
    main()
