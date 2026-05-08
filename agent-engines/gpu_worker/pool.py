from __future__ import annotations

import asyncio
from collections.abc import Callable

from gpu_worker.config import WorkerConfig
from gpu_worker.models import AnalysisRequest, AnalysisResult, WorkerInfo
from gpu_worker.worker import GPUAnalysisWorker


class WorkerPool:
    """Pool of GPU analysis workers with least-loaded dispatch."""

    def __init__(
        self,
        configs: list[WorkerConfig],
        *,
        worker_factory: Callable[[WorkerConfig], GPUAnalysisWorker] | None = None,
    ) -> None:
        if not configs:
            raise ValueError("WorkerPool requires at least one worker configuration")
        factory = worker_factory or (lambda cfg: GPUAnalysisWorker(cfg))
        self._workers = [factory(config) for config in configs]
        self._reservations = [0 for _ in self._workers]
        self._condition = asyncio.Condition()
        self._started = False

    async def start_all(self) -> None:
        """Initialize all workers in parallel."""

        if self._started:
            return
        await asyncio.gather(*(worker.start() for worker in self._workers))
        self._started = True

    async def submit(self, request: AnalysisRequest) -> AnalysisResult:
        """Dispatch an analysis request to the least-loaded worker."""

        if not self._started:
            raise RuntimeError("worker pool has not been started")
        worker = await self._acquire_worker()
        worker_index = self._workers.index(worker)
        try:
            return await worker.analyze(request)
        finally:
            async with self._condition:
                self._reservations[worker_index] -= 1
                self._condition.notify_all()

    async def shutdown_all(self) -> None:
        """Gracefully shut down all workers."""

        await asyncio.gather(*(worker.shutdown() for worker in self._workers))
        self._started = False

    def get_pool_status(self) -> list[WorkerInfo]:
        """Return per-worker monitoring information."""

        return [worker.get_info() for worker in self._workers]

    async def _acquire_worker(self) -> GPUAnalysisWorker:
        """Wait until a worker has capacity and return the least-loaded one."""

        async with self._condition:
            while True:
                indexed_candidates = [
                    (index, worker)
                    for index, worker in enumerate(self._workers)
                    if (worker.load + self._reservations[index])
                    < worker.config.max_concurrent_analyses
                ]
                if indexed_candidates:
                    worker_index, worker = min(
                        indexed_candidates,
                        key=lambda item: (
                            item[1].load + self._reservations[item[0]],
                            item[1].worker_id,
                        ),
                    )
                    self._reservations[worker_index] += 1
                    return worker
                await self._condition.wait()
