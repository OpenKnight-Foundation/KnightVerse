from __future__ import annotations

import asyncio

import pytest

from gpu_worker.config import GPUConfig, WorkerConfig
from gpu_worker.models import AnalysisRequest
from gpu_worker.pool import WorkerPool


class FakePoolWorker:
    def __init__(self, config: WorkerConfig) -> None:
        self.config = config
        self.worker_id = f"worker-{config.gpu.device_id}"
        self.load = 0
        self.started = False
        self.stopped = False
        self.assigned_requests: list[str] = []

    @property
    def has_capacity(self) -> bool:
        return self.load < self.config.max_concurrent_analyses

    async def start(self) -> None:
        self.started = True

    async def analyze(self, request: AnalysisRequest):
        self.load += 1
        self.assigned_requests.append(request.id)
        await asyncio.sleep(0.02)
        self.load -= 1
        from gpu_worker.models import AnalysisResult, WorkerInfo, WorkerStatus

        return AnalysisResult(request_id=request.id, best_move=f"move-{self.worker_id}")

    async def shutdown(self) -> None:
        self.stopped = True

    def get_info(self):
        from gpu_worker.models import WorkerInfo, WorkerStatus

        return WorkerInfo(
            worker_id=self.worker_id,
            status=WorkerStatus.IDLE,
            gpu_device_id=self.config.gpu.device_id,
        )


@pytest.mark.asyncio
async def test_pool_dispatches_to_least_loaded_worker() -> None:
    configs = [
        WorkerConfig(gpu=GPUConfig(device_id=0), max_concurrent_analyses=2),
        WorkerConfig(gpu=GPUConfig(device_id=1), max_concurrent_analyses=2),
    ]
    pool = WorkerPool(configs, worker_factory=FakePoolWorker)
    await pool.start_all()

    requests = [
        AnalysisRequest(fen="8/8/8/8/8/8/8/K6k w - - 0 1", priority=index)
        for index in range(4)
    ]
    results = await asyncio.gather(*(pool.submit(request) for request in requests))
    status = pool.get_pool_status()
    await pool.shutdown_all()

    assert {result.best_move for result in results} == {"move-worker-0", "move-worker-1"}
    assert len(status) == 2
    assert {entry.worker_id for entry in status} == {"worker-0", "worker-1"}
