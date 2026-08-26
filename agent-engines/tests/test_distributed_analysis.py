from __future__ import annotations

import asyncio

import pytest

from gpu_worker.decentralized_orchestrator import DecentralizedOrchestrator
from gpu_worker.models import FullGameAnalysisRequest
from gpu_worker.pool import WorkerPool
from gpu_worker.worker import GPUAnalysisWorker


@pytest.mark.asyncio
async def test_full_game_analysis():
    """Test that a full game analysis is correctly distributed and reassembled."""

    class MockAnalysisWorker(GPUAnalysisWorker):
        async def analyze(self, request):
            # Simulate analysis
            await asyncio.sleep(0.1)
            return {
                "request_id": request.id,
                "best_move": "e2e4",
                "evaluation": 0.5,
                "depth": 20,
                "principal_variation": ["e2e4"],
                "nodes_searched": 1000,
                "time_ms": 100,
                "gpu_utilization": 0.5,
            }

    pool = WorkerPool(
        configs=[{"engine_backend": "stockfish", "engine_path": "/usr/local/bin/stockfish"}] * 2,
        maia_configs=[],
        worker_factory=lambda config, book: MockAnalysisWorker(config),
    )
    orchestrator = DecentralizedOrchestrator(pool)

    fens = ["rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"] * 5
    request = FullGameAnalysisRequest(fens=fens)

    result = await orchestrator.submit_full_game_analysis(request)

    assert len(result.results) == 5
    for i, analysis_result in enumerate(result.results):
        assert analysis_result["request_id"] != request.id