from __future__ import annotations

import asyncio
from collections.abc import Callable

from gpu_worker.anomaly import BotFarmAnomalyDetector
from gpu_worker.config import WorkerConfig
from gpu_worker.models import AnalysisRequest, AnalysisResult, WorkerInfo
from gpu_worker.worker import GPUAnalysisWorker
from gpu_worker.opening_book import OpeningBook
from gpu_worker.maia_worker import MaiaWorker
from gpu_worker.maia_config import MaiaConfig


class WorkerPool:
    """Pool of GPU analysis workers with least-loaded dispatch."""

    def __init__(
        self,
        configs: list[WorkerConfig],
        maia_configs: list[MaiaConfig],
        *,
        worker_factory: Callable[[WorkerConfig, OpeningBook | None], GPUAnalysisWorker] | None = None,
        maia_worker_factory: Callable[[WorkerConfig, MaiaConfig], MaiaWorker] | None = None,
        anomaly_detector: BotFarmAnomalyDetector | None = None,
        opening_book: OpeningBook | None = None,
    ) -> None:
        if not configs and not maia_configs:
            raise ValueError("WorkerPool requires at least one worker configuration")
        
        factory = worker_factory or (lambda cfg, book: GPUAnalysisWorker(cfg, opening_book=book))
        self._workers = [factory(config, opening_book) for config in configs]

        maia_factory = maia_worker_factory or (lambda cfg, maia_cfg: MaiaWorker(cfg, maia_cfg.path))
        self._maia_workers = [maia_factory(configs[0], maia_config) for maia_config in maia_configs]

        self._reservations = [0 for _ in self._workers]
        self._maia_reservations = [0 for _ in self._maia_workers]
        self._condition = asyncio.Condition()
        self._started = False
        self.anomaly_detector = anomaly_detector or BotFarmAnomalyDetector()

    async def start_all(self) -> None:
        """Initialize all workers in parallel."""

        if self._started:
            return
        await asyncio.gather(*(worker.start() for worker in self._workers))
        await asyncio.gather(*(worker.start() for worker in self._maia_workers))
        self._started = True

    async def submit(self, request: AnalysisRequest, opening_book: OpeningBook | None = None) -> AnalysisResult:
        """Dispatch an analysis request to the least-loaded worker."""

        if not self._started:
            raise RuntimeError("worker pool has not been started")
        self.anomaly_detector.record_request(request)

        # Check if the request is for a Maia personality.
        if request.actor_id and request.actor_id.startswith("maia-"):
            elo = int(request.actor_id.split("-")[1])
            worker = await self._acquire_maia_worker(elo)
            worker_index = self._maia_workers.index(worker)
            try:
                return await worker.analyze(request)
            finally:
                async with self._condition:
                    self._maia_reservations[worker_index] -= 1
                    self._condition.notify_all()
        else:
            worker = await self._acquire_worker()
            worker_index = self._workers.index(worker)
            try:
                # Pass the opening book to the worker's analyze method.
                return await worker.analyze(request)
            finally:
                async with self._condition:
                    self._reservations[worker_index] -= 1
                    self._condition.notify_all()

    async def shutdown_all(self) -> None:
        """Gracefully shut down all workers."""

        await asyncio.gather(*(worker.shutdown() for worker in self._workers))
        await asyncio.gather(*(worker.shutdown() for worker in self._maia_workers))
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

    async def _acquire_maia_worker(self, elo: int) -> MaiaWorker:
        """Wait until a Maia worker with the specified ELO has capacity and return it."""

        async with self._condition:
            while True:
                indexed_candidates = [
                    (index, worker)
                    for index, worker in enumerate(self._maia_workers)
                    if worker.config.maia_models[0].elo == elo
                    and (worker.load + self._maia_reservations[index])
                    < worker.config.max_concurrent_analyses
                ]
                if indexed_candidates:
                    worker_index, worker = min(
                        indexed_candidates,
                        key=lambda item: (
                            item[1].load + self._maia_reservations[item[0]],
                            item[1].worker_id,
                        ),
                    )
                    self._maia_reservations[worker_index] += 1
                    return worker
                await self._condition.wait()