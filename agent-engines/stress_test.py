from __future__ import annotations

import asyncio
import json
import time
import uuid
from typing import List, Dict, Any

import websockets

from gpu_worker.models import AnalysisRequest

async def run_test_client(uri: str, requests: List[AnalysisRequest]) -> Dict[str, Any]:
    """
    Simulates a single virtual user sending a batch of analysis requests.
    """
    latencies = []
    errors = 0
    start_time = time.monotonic()

    try:
        async with websockets.connect(uri) as websocket:
            for request in requests:
                req_start = time.monotonic()
                await websocket.send(request.model_dump_json())
                response = await websocket.recv()
                latencies.append(time.monotonic() - req_start)
                # Optionally, validate the response
                json.loads(response)
    except Exception:
        errors += 1

    return {
        "latencies": latencies,
        "errors": errors,
        "duration": time.monotonic() - start_time,
    }

async def run_stress_test(uri: str, num_users: int, requests_per_user: int):
    """
    Orchestrates the stress test by spawning multiple virtual users.
    """
    print(f"Starting stress test with {num_users} users, {requests_per_user} requests each...")
    
    # Generate a pool of unique requests
    all_requests = [
        AnalysisRequest(
            fen="rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            depth=10 + (i % 10),
            id=str(uuid.uuid4())
        )
        for i in range(num_users * requests_per_user)
    ]
    
    tasks = []
    for i in range(num_users):
        user_requests = all_requests[i * requests_per_user : (i + 1) * requests_per_user]
        tasks.append(run_test_client(uri, user_requests))
        
    results = await asyncio.gather(*tasks)
    
    # Aggregate and print results
    total_requests = num_users * requests_per_user
    total_errors = sum(r["errors"] for r in results)
    all_latencies = [lat for r in results for lat in r["latencies"]]
    
    print("\n--- Stress Test Results ---")
    print(f"Total Requests: {total_requests}")
    print(f"Successful: {total_requests - total_errors}")
    print(f"Failed: {total_errors}")
    
    if all_latencies:
        print(f"Average Latency: {sum(all_latencies) / len(all_latencies):.4f}s")
        print(f"Max Latency: {max(all_latencies):.4f}s")
        print(f"Min Latency: {min(all_latencies):.4f}s")

if __name__ == "__main__":
    WEBSOCKET_URI = "ws://localhost:8765"  # Replace with your actual WebSocket endpoint
    NUM_USERS = 1000
    REQUESTS_PER_USER = 10
    
    asyncio.run(run_stress_test(WEBSOCKET_URI, NUM_USERS, REQUESTS_PER_USER))