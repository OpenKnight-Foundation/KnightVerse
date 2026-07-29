from __future__ import annotations

import prometheus_client
from prometheus_client import Counter, Gauge

# Prometheus Metrics for AI Worker Pool
WORKER_COUNT = Gauge('ai_worker_pool_size', 'Number of active AI workers in the pool')
JOBS_PROCESSED = Counter('ai_worker_jobs_processed_total', 'Total number of jobs processed by the AI worker pool')

import asyncio
import logging
from collections.abc import Callable

from gpu_worker.anomaly import BotFarmAnomalyDetector
from gpu_worker.anomaly_logger import log_anomaly_report
from gpu_worker.config import WorkerConfig
from gpu_worker.models import AnalysisRequest, AnalysisResult, WorkerInfo
from gpu_worker.worker import GPUAnalysisWorker
from gpu_worker.opening_book import OpeningBook
from gpu_worker.maia_worker import MaiaWorker
from gpu_worker.maia_config import MaiaConfig

logger = logging.getLogger("KnightVerse.WorkerPool")


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
        anomaly_report = self.anomaly_detector.record_request(request)
        if anomaly_report.findings:
            log_anomaly_report(anomaly_report)

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

    async def submit_batch(self, requests: list[AnalysisRequest]) -> list[AnalysisResult]:
        """Dispatch a batch of analysis requests to the workers."""

        if not self._started:
            raise RuntimeError("worker pool has not been started")

        tasks = [self.submit(request) for request in requests]
        return await asyncio.gather(*tasks)

    async def wait_for_pending_tasks(self, timeout: float | None = None) -> None:
        """Wait for all pending tasks to complete before shutting down."""
        async with self._condition:
            # Wait until all pending tasks are completed
            await asyncio.wait_for(
                self._condition.wait_for(
                    lambda: all(r == 0 for r in self._reservations) and 
                            all(r == 0 for r in self._maia_reservations)
                ),
                timeout=timeout
            )
            logger.info("All pending analysis tasks completed")

    async def shutdown_all(self, wait_for_pending: bool = True, timeout: float | None = 30) -> None:
        """Gracefully shut down all workers.
        
        Args:
            wait_for_pending: Whether to wait for pending tasks to complete before shutdown
            timeout: Maximum time to wait for pending tasks in seconds
        """
        if wait_for_pending:
            try:
                await self.wait_for_pending_tasks(timeout=timeout)
            except asyncio.TimeoutError:
                logger.warning(f"Timed out waiting for {sum(self._reservations)} standard and {sum(self._maia_reservations)} Maia pending tasks to complete, proceeding with shutdown")

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