import time
import requests
import json
import psutil
import os

API_URL = "http://127.0.0.1:8080/chat"

print("🚀 Initializing AEGIS Performance Benchmark...")
print(f"📡 Target API Endpoint: {API_URL}")
print("--------------------------------------------------")

def run_benchmark():
    payload = {
        "message": "Explain the core concept of local-first software and its privacy benefits in 2 short sentences."
    }
    headers = {"Content-Type": "application/json"}

    start_ram = psutil.virtual_memory().used / (1024 * 1024)
    start_time = time.time()
    ttft = None
    total_tokens = 0
    
    try:
        response = requests.post(API_URL, json=payload, headers=headers, stream=True)
        
        if response.status_code != 200:
            print(f"⚠️  Warning: API returned status code {response.status_code}.")
            print("💡 Tip: Verify your backend orchestration server configuration and endpoint schema.")
            return

        print("⏳ Streaming tokens and collecting metrics...")

        for line in response.iter_lines():
            if line:
                if ttft is None:
                    ttft = (time.time() - start_time) * 1000
                    print(f"⏱️  Time to First Token (TTFT): {ttft:.2f} ms")
                
                words = line.decode('utf-8').split()
                total_tokens += len(words)

        end_time = time.time()
        total_duration = end_time - start_time
        
        end_ram = psutil.virtual_memory().used / (1024 * 1024)
        ram_delta = end_ram - start_ram
        
        tokens_per_sec = total_tokens / total_duration if total_duration > 0 else 0

        print("\n================= AEGIS BENCHMARK METRICS =================")
        print(f"📊 TTFT (Inference Latency):    {ttft:.2f} ms")
        print(f"⚡ Throughput (Tokens/sec):     {tokens_per_sec:.2f} tokens/sec")
        print(f"⏱️  Total Generation Time:      {total_duration:.2f} seconds")
        print(f"🧠 Memory Usage Delta:          {ram_delta:+.2f} MB")
        print(f"📈 Current System RAM:          {end_ram:.2f} MB")
        print("===========================================================\n")

    except requests.exceptions.ConnectionError:
        print("❌ Connection Error: Failed to establish a connection with the backend engine.")
        print("💡 Tip: Ensure the Rust orchestration engine is actively listening.")

if __name__ == "__main__":
    run_benchmark()