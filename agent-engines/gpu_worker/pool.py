"""
Multi-Engine Consensus Evaluator — Stockfish + LCZero + Berserk ensemble.

Runs chess positions through multiple engines concurrently, aggregates
evaluation scores and candidate moves, and computes a consensus agreement
score.  Survives individual engine crashes without failing the whole request.
"""

from __future__ import annotations

import asyncio
import logging
import signal
import time
from collections.abc import Callable
from typing import Dict, Optional, Set, Any
from dataclasses import dataclass
from multiprocessing import Process, Queue, Event

import prometheus_client
from prometheus_client import Counter, Gauge

import chess
import chess.engine

logger = logging.getLogger(__name__)

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------

_DEFAULT_ENGINES: dict[str, dict] = {
    "stockfish": {
        "engine_path": os.environ.get(
            "STOCKFISH_PATH", "stockfish"
        ),
        "threads": int(os.environ.get("STOCKFISH_THREADS", "2")),
        "hash_mb": int(os.environ.get("STOCKFISH_HASH_MB", "256")),
    },
    "lc0": {
        "engine_path": os.environ.get("LC0_PATH", "lc0"),
        "weights": os.environ.get("LC0_WEIGHTS", ""),
    },
    "berserk": {
        "engine_path": os.environ.get("BERSERK_PATH", "stockfish"),
        "threads": int(os.environ.get("BERSERK_THREADS", "2")),
        "hash_mb": int(os.environ.get("BERSERK_HASH_MB", "256")),
    },
}

DEFAULT_DEPTH = 18
DEFAULT_TIMEOUT_S = 10.0
ENGINE_CONNECT_TIMEOUT_S = 5.0

# Prometheus Metrics for AI Worker Pool
WORKER_COUNT = Gauge('ai_worker_pool_size', 'Number of active AI workers in the pool')
JOBS_PROCESSED = Counter('ai_worker_jobs_processed_total', 'Total number of jobs processed by the AI worker pool')
WORKER_STARTUP_TIME = Gauge('ai_worker_startup_seconds', 'Time taken to start a worker', ['worker_id'])
GRACEFUL_SHUTDOWNS = Counter('ai_worker_graceful_shutdowns_total', 'Number of graceful worker shutdowns')
FORCED_SHUTDOWNS = Counter('ai_worker_forced_shutdowns_total', 'Number of forced worker shutdowns')


@dataclass
class ProcessWorkerInfo:
    """Information about a worker process for autoscaling."""
    process_id: int
    gpu_device_id: int
    worker_config: WorkerConfig
    process_handle: Process
    started_at: float
    last_active: float
    is_busy: bool = False
    shutdown_requested: bool = False
    graceful_shutdown_timeout: float = 30.0


class AutoscalingWorkerPool:
    """
    Enhanced worker pool with autoscaling capabilities and graceful process management.
    Integrates with the autoscaling daemon for dynamic worker lifecycle management.
    """

    def __init__(
        self,
        base_configs: list[WorkerConfig],
        maia_configs: list[MaiaConfig],
        *,
        worker_factory: Callable[[WorkerConfig, OpeningBook | None], GPUAnalysisWorker] | None = None,
        maia_worker_factory: Callable[[WorkerConfig, MaiaConfig], MaiaWorker] | None = None,
        anomaly_detector: BotFarmAnomalyDetector | None = None,
        opening_book: OpeningBook | None = None,
        enable_autoscaling: bool = True,
        min_workers: int = 2,
        max_workers: int = 10,
    ) -> None:
        if not base_configs and not maia_configs:
            raise ValueError("WorkerPool requires at least one worker configuration")
        
        self.base_configs = base_configs
        self.maia_configs = maia_configs
        self.min_workers = min_workers
        self.max_workers = max_workers
        self.enable_autoscaling = enable_autoscaling
        
        factory = worker_factory or (lambda cfg, book: GPUAnalysisWorker(cfg, opening_book=book))
        self._workers = [factory(config, opening_book) for config in base_configs]

        maia_factory = maia_worker_factory or (lambda cfg, maia_cfg: MaiaWorker(cfg, maia_cfg.path))
        self._maia_workers = [maia_factory(base_configs[0], maia_config) for maia_config in maia_configs]

        self._reservations = [0 for _ in self._workers]
        self._maia_reservations = [0 for _ in self._maia_workers]
        self._condition = asyncio.Condition()
        self._started = False
        self.anomaly_detector = anomaly_detector or BotFarmAnomalyDetector()
        
        # Process management for autoscaling
        self._process_workers: Dict[int, ProcessWorkerInfo] = {}
        self._shutdown_event = asyncio.Event()
        self._shutdown_requested = False
        self._graceful_shutdown_timeout = 30.0
        
        # Signal handling setup
        self._original_sigterm_handler = None
        self._original_sigint_handler = None
        
    async def start_all(self) -> None:
        """Initialize all workers in parallel and set up signal handlers."""

        if self._started:
            return
            
        # Set up signal handlers for graceful shutdown
        self._setup_signal_handlers()
        
        await asyncio.gather(*(worker.start() for worker in self._workers))
        await asyncio.gather(*(worker.start() for worker in self._maia_workers))
        
        # Update Prometheus metrics
        WORKER_COUNT.set(len(self._workers) + len(self._maia_workers))
        
        self._started = True
        logger.info(f"Worker pool started with {len(self._workers)} standard and {len(self._maia_workers)} Maia workers")

    def _setup_signal_handlers(self) -> None:
        """Set up signal handlers for graceful shutdown."""
        try:
            # Store original handlers
            self._original_sigterm_handler = signal.signal(signal.SIGTERM, self._signal_handler)
            self._original_sigint_handler = signal.signal(signal.SIGINT, self._signal_handler)
            logger.debug("Signal handlers configured for graceful shutdown")
        except ValueError as e:
            # Signal handling might not be available in some contexts (e.g., threads)
            logger.warning(f"Could not set up signal handlers: {e}")
            
    def _signal_handler(self, signum: int, frame) -> None:
        """Handle shutdown signals gracefully."""
        logger.info(f"Received signal {signum}, initiating graceful shutdown")
        self._shutdown_requested = True
        self._shutdown_event.set()
        
        # Schedule graceful shutdown
        asyncio.create_task(self.shutdown_all(wait_for_pending=True))
        
    async def add_worker(self, config: WorkerConfig, gpu_device_id: int) -> bool:
        """
        Add a new worker to the pool dynamically.
        Returns True if worker was successfully added, False otherwise.
        """
        if len(self._workers) >= self.max_workers:
            logger.warning(f"Cannot add worker: already at maximum capacity ({self.max_workers})")
            return False
            
        try:
            start_time = time.time()
            
            # Create and start new worker
            factory = lambda cfg, book: GPUAnalysisWorker(cfg, opening_book=None)
            new_worker = factory(config, None)
            
            await new_worker.start()
            
            # Add to workers list
            async with self._condition:
                self._workers.append(new_worker)
                self._reservations.append(0)
                self._condition.notify_all()
                
            # Update metrics
            startup_time = time.time() - start_time
            WORKER_COUNT.set(len(self._workers) + len(self._maia_workers))
            WORKER_STARTUP_TIME.labels(worker_id=new_worker.worker_id).set(startup_time)
            
            logger.info(f"Added new worker {new_worker.worker_id} on GPU {gpu_device_id} (startup: {startup_time:.2f}s)")
            return True
            
        except Exception as e:
            logger.error(f"Failed to add worker on GPU {gpu_device_id}: {e}")
            return False
            
    async def remove_worker(self, worker_index: int, graceful: bool = True) -> bool:
        """
        Remove a worker from the pool dynamically.
        Returns True if worker was successfully removed, False otherwise.
        """
        if worker_index < 0 or worker_index >= len(self._workers):
            logger.error(f"Invalid worker index: {worker_index}")
            return False
            
        if len(self._workers) <= self.min_workers:
            logger.warning(f"Cannot remove worker: already at minimum capacity ({self.min_workers})")
            return False
            
        try:
            async with self._condition:
                worker = self._workers[worker_index]
                reservation_count = self._reservations[worker_index]
                
                if graceful and reservation_count > 0:
                    logger.info(f"Worker {worker.worker_id} has {reservation_count} active tasks, waiting for completion")
                    
                    # Wait for worker to become idle
                    timeout = time.time() + self._graceful_shutdown_timeout
                    while self._reservations[worker_index] > 0 and time.time() < timeout:
                        await asyncio.sleep(0.1)
                        
                    if self._reservations[worker_index] > 0:
                        logger.warning(f"Worker {worker.worker_id} still has active tasks after timeout, forcing shutdown")
                        FORCED_SHUTDOWNS.inc()
                    else:
                        GRACEFUL_SHUTDOWNS.inc()
                else:
                    if reservation_count > 0:
                        FORCED_SHUTDOWNS.inc()
                    else:
                        GRACEFUL_SHUTDOWNS.inc()
                
                # Shutdown and remove worker
                await worker.shutdown()
                self._workers.pop(worker_index)
                self._reservations.pop(worker_index)
                
                # Update indices in reservations for remaining workers
                self._condition.notify_all()
                
            # Update metrics
            WORKER_COUNT.set(len(self._workers) + len(self._maia_workers))
            
            logger.info(f"Removed worker {worker.worker_id} from pool")
            return True
            
        except Exception as e:
            logger.error(f"Failed to remove worker at index {worker_index}: {e}")
            return False

        elapsed = (time.perf_counter() - t0) * 1000
        return self._build_consensus(fen, clean, elapsed)

        if not self._started:
            raise RuntimeError("worker pool has not been started")
            
        if self._shutdown_requested:
            raise RuntimeError("worker pool is shutting down")
            
        anomaly_report = self.anomaly_detector.record_request(request)
        if anomaly_report.findings:
            log_anomaly_report(anomaly_report)

        # Check if the request is for a Maia personality.
        if request.actor_id and request.actor_id.startswith("maia-"):
            elo = int(request.actor_id.split("-")[1])
            worker = await self._acquire_maia_worker(elo)
            worker_index = self._maia_workers.index(worker)
            try:
                result = await worker.analyze(request)
                JOBS_PROCESSED.inc()
                return result
            finally:
                async with self._condition:
                    self._maia_reservations[worker_index] -= 1
                    self._condition.notify_all()
        else:
            worker = await self._acquire_worker()
            worker_index = self._workers.index(worker)
            try:
                # Pass the opening book to the worker's analyze method.
                result = await worker.analyze(request)
                JOBS_PROCESSED.inc()
                return result
            finally:
                async with self._condition:
                    self._reservations[worker_index] -= 1
                    self._condition.notify_all()

    async def _analyse_with_engine(
        self,
        name: str,
        cfg: dict,
        board: chess.Board,
        depth: int,
        timeout: float,
    ) -> EngineAnalysis:
        """Connect to *name* engine, analyse, and return result."""
        async with self._semaphore:
            return await asyncio.wait_for(
                self._run_engine(name, cfg, board, depth),
                timeout=timeout + ENGINE_CONNECT_TIMEOUT_S,
            )

    async def shutdown_all(self, wait_for_pending: bool = True, timeout: float | None = 30) -> None:
        """Gracefully shut down all workers.
        
        Args:
            wait_for_pending: Whether to wait for pending tasks to complete before shutdown
            timeout: Maximum time to wait for pending tasks in seconds
        """
        if not self._started:
            return
            
        logger.info("Initiating worker pool shutdown")
        self._shutdown_requested = True
        
        if wait_for_pending:
            try:
                await self.wait_for_pending_tasks(timeout=timeout)
            except asyncio.TimeoutError:
                pending_standard = sum(self._reservations)
                pending_maia = sum(self._maia_reservations)
                logger.warning(f"Timed out waiting for {pending_standard} standard and {pending_maia} Maia pending tasks to complete, proceeding with shutdown")
                FORCED_SHUTDOWNS.inc()

        # Shutdown all workers
        try:
            await asyncio.gather(*(worker.shutdown() for worker in self._workers), return_exceptions=True)
            await asyncio.gather(*(worker.shutdown() for worker in self._maia_workers), return_exceptions=True)
        except Exception as e:
            logger.error(f"Error during worker shutdown: {e}")
            
        # Restore original signal handlers
        self._restore_signal_handlers()
        
        self._started = False
        WORKER_COUNT.set(0)
        logger.info("Worker pool shutdown completed")
        
    def _restore_signal_handlers(self) -> None:
        """Restore original signal handlers."""
        try:
            if self._original_sigterm_handler is not None:
                signal.signal(signal.SIGTERM, self._original_sigterm_handler)
            if self._original_sigint_handler is not None:
                signal.signal(signal.SIGINT, self._original_sigint_handler)
        except ValueError as e:
            logger.debug(f"Could not restore signal handlers: {e}")

    def get_pool_status(self) -> list[WorkerInfo]:
        """Return per-worker monitoring information."""
        return [worker.get_info() for worker in self._workers]
        
    def get_detailed_status(self) -> Dict[str, Any]:
        """Return detailed pool status including autoscaling information."""
        return {
            "started": self._started,
            "shutdown_requested": self._shutdown_requested,
            "worker_count": len(self._workers),
            "maia_worker_count": len(self._maia_workers),
            "min_workers": self.min_workers,
            "max_workers": self.max_workers,
            "pending_tasks": sum(self._reservations),
            "pending_maia_tasks": sum(self._maia_reservations),
            "autoscaling_enabled": self.enable_autoscaling,
            "workers": [
                {
                    "worker_id": worker.worker_id,
                    "load": worker.load,
                    "reservations": self._reservations[i],
                    "status": worker.get_info().status.value if hasattr(worker.get_info().status, 'value') else str(worker.get_info().status)
                }
                for i, worker in enumerate(self._workers)
            ]
        }
        
    def can_scale_up(self) -> bool:
        """Check if the pool can add more workers."""
        return len(self._workers) < self.max_workers and self.enable_autoscaling
        
    def can_scale_down(self) -> bool:
        """Check if the pool can remove workers."""
        return len(self._workers) > self.min_workers and self.enable_autoscaling
        
    def get_idle_workers(self) -> list[int]:
        """Get indices of workers that are currently idle."""
        idle_workers = []
        for i, (worker, reservations) in enumerate(zip(self._workers, self._reservations)):
            if worker.load == 0 and reservations == 0:
                idle_workers.append(i)
        return idle_workers

            # Parse score
            score = analysis.get("score")
            if score is not None:
                score_obj = score.white() if board.turn == chess.WHITE else score.score()
                if score_obj is not None:
                    result.score_cp = score_obj.score(mate_score=10000)

        async with self._condition:
            while True:
                if self._shutdown_requested:
                    raise RuntimeError("Pool is shutting down")
                    
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

        except (FileNotFoundError, OSError) as exc:
            result.error = f"Engine process not found: {exc}"
            logger.warning("Engine %s not available: %s", name, exc)
        except chess.engine.EngineTerminatedError as exc:
            result.error = f"Engine terminated: {exc}"
            logger.warning("Engine %s terminated: %s", name, exc)
        except chess.engine.EngineError as exc:
            result.error = f"Engine error: {exc}"
            logger.warning("Engine %s error: %s", name, exc)
        except Exception as exc:
            result.error = f"Unexpected error: {exc}"
            logger.exception("Engine %s unexpected failure", name)
        finally:
            if engine_proc is not None:
                try:
                    engine_proc.quit()
                except Exception:
                    pass

        async with self._condition:
            while True:
                if self._shutdown_requested:
                    raise RuntimeError("Pool is shutting down")
                    
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


# Legacy WorkerPool class for backward compatibility
class WorkerPool(AutoscalingWorkerPool):
    """Legacy WorkerPool class that extends AutoscalingWorkerPool for backward compatibility."""
    
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
        # Initialize with autoscaling disabled for legacy behavior
        super().__init__(
            configs,
            maia_configs,
            worker_factory=worker_factory,
            maia_worker_factory=maia_worker_factory,
            anomaly_detector=anomaly_detector,
            opening_book=opening_book,
            enable_autoscaling=False,
            min_workers=len(configs),
            max_workers=len(configs)
        )

