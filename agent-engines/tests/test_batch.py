from __future__ import annotations

import asyncio
import time

import pytest

from gpu_worker.batch import BatchAnalyzer
from gpu_worker.config import GPUConfig, WorkerConfig
from gpu_worker.models import AnalysisRequest, AnalysisResult


class FakePool:
    def __init__(self) -> None:
        self.active_submissions = 0
        self.max_concurrency = 0

    async def submit(self, request: AnalysisRequest) -> AnalysisResult:
        self.active_submissions += 1
        self.max_concurrency = max(self.max_concurrency, self.active_submissions)
        await asyncio.sleep(0.01)
        self.active_submissions -= 1
        return AnalysisResult(request_id=request.id, best_move="e2e4")


@pytest.mark.asyncio
async def test_batch_flushes_on_size_limit() -> None:
    pool = FakePool()
    analyzer = BatchAnalyzer(pool, batch_size=2, flush_interval_ms=1000)
    requests = [
        AnalysisRequest(fen="8/8/8/8/8/8/8/K6k w - - 0 1"),
        AnalysisRequest(fen="8/8/8/8/8/8/8/K6k w - - 0 1"),
    ]

    results = await analyzer.submit_batch(requests)
    await analyzer.shutdown()

    assert [result.best_move for result in results] == ["e2e4", "e2e4"]
    assert pool.max_concurrency == 2


@pytest.mark.asyncio
async def test_batch_flushes_on_timer() -> None:
    pool = FakePool()
    analyzer = BatchAnalyzer(pool, batch_size=4, flush_interval_ms=10)
    started_at = time.monotonic()

    result = await analyzer.submit(
        AnalysisRequest(fen="8/8/8/8/8/8/8/K6k w - - 0 1", priority=5)
    )
    await analyzer.shutdown()

    assert result.best_move == "e2e4"
    assert time.monotonic() - started_at >= 0.01
    assert pool.max_concurrency == 1
