from __future__ import annotations

import asyncio
from collections.abc import Callable
import time
import uuid

from gpu_worker.config import WorkerConfig
from gpu_worker.models import AnalysisRequest, AnalysisResult, WorkerInfo, WorkerStatus
from gpu_worker.resource_monitor import ResourceMonitor
from gpu_worker.uci_bridge import AsyncUciBridge


class GPUAnalysisWorker:
    """Single GPU analysis worker wrapping a UCI engine process."""

    def __init__(
        self,
        config: WorkerConfig,
        worker_id: str | None = None,
        *,
        bridge_factory: Callable[[WorkerConfig], AsyncUciBridge] | None = None,
        resource_monitor: ResourceMonitor | None = None,
    ) -> None:
        self.config = config
        self.worker_id = worker_id or str(uuid.uuid4())
        self._bridge_factory = bridge_factory or (lambda cfg: AsyncUciBridge(cfg))
        self._bridge = self._bridge_factory(config)
        self._monitor = resource_monitor or ResourceMonitor()
        self._status = WorkerStatus.IDLE
        self._started = False
        self._analyses_completed = 0
        self._started_at: float | None = None
        self._pending_count = 0
        self._pending_lock = asyncio.Lock()
        self._analysis_lock = asyncio.Lock()

    @property
    def status(self) -> WorkerStatus:
        """Return the current worker status."""

        return self._status

    @property
    def load(self) -> int:
        """Return the number of queued or active analyses assigned to the worker."""

        return self._pending_count

    @property
    def has_capacity(self) -> bool:
        """Whether the worker can accept another queued analysis."""

        return self._pending_count < self.config.max_concurrent_analyses

    async def start(self) -> None:
        """Spawn the engine process, configure options, and start monitoring."""

        if self._started:
            return
        try:
            await self._bridge.start()
            await self._bridge.initialize_options()
            await self._monitor.start()
        except Exception:
            self._status = WorkerStatus.ERROR
            raise
        self._started = True
        self._started_at = time.monotonic()
        self._status = WorkerStatus.IDLE

    async def analyze(self, request: AnalysisRequest) -> AnalysisResult:
        """Analyze one position and return the normalized result."""

        if not self._started:
            raise RuntimeError("worker has not been started")

        async with self._pending_lock:
            if self._pending_count >= self.config.max_concurrent_analyses:
                raise RuntimeError("worker is at capacity")
            self._pending_count += 1

        started_at = time.monotonic()
        try:
            async with self._analysis_lock:
                self._status = WorkerStatus.BUSY
                await self._bridge.set_position(request.fen)
                best_move, info = await self._bridge.go(
                    depth=request.depth or self.config.default_depth,
                    time_limit_ms=request.time_limit_ms
                    or self.config.default_time_limit_ms,
                    search_moves=request.search_moves,
                    num_pv=request.num_pv,
                )
                gpu_stats = self._monitor.get_gpu_stats()
                result = AnalysisResult(
                    request_id=request.id,
                    best_move=best_move.best_move,
                    evaluation=info.evaluation,
                    depth=info.depth,
                    principal_variation=info.principal_variation,
                    nodes_searched=info.nodes,
                    time_ms=int((time.monotonic() - started_at) * 1000),
                    gpu_utilization=_gpu_utilization_for_device(
                        gpu_stats, self.config.gpu.device_id
                    ),
                )
                self._analyses_completed += 1
                return result
        except Exception:
            self._status = WorkerStatus.ERROR
            raise
        finally:
            async with self._pending_lock:
                self._pending_count -= 1
                if self._status != WorkerStatus.ERROR:
                    self._status = (
                        WorkerStatus.BUSY if self._pending_count > 0 else WorkerStatus.IDLE
                    )

    async def shutdown(self) -> None:
        """Gracefully stop monitoring and terminate the engine process."""

        self._status = WorkerStatus.SHUTTING_DOWN
        await self._monitor.stop()
        await self._bridge.quit()
        self._started = False

    def get_info(self) -> WorkerInfo:
        """Return a runtime snapshot for pool monitoring."""

        gpu_stats = self._monitor.get_gpu_stats()
        device_stats = _gpu_device_stats(gpu_stats, self.config.gpu.device_id)
        uptime_seconds = 0.0
        if self._started_at is not None:
            uptime_seconds = max(0.0, time.monotonic() - self._started_at)
        return WorkerInfo(
            worker_id=self.worker_id,
            status=self._status,
            gpu_device_id=self.config.gpu.device_id,
            gpu_memory_used_mb=float(device_stats.get("memory_used_mb", 0.0)),
            gpu_utilization_pct=float(device_stats.get("utilization_pct", 0.0)),
            analyses_completed=self._analyses_completed,
            uptime_seconds=uptime_seconds,
        )


def _gpu_device_stats(gpu_stats: dict, device_id: int) -> dict:
    """Return the monitoring payload for one GPU device."""

    for device in gpu_stats.get("devices", []):
        if device.get("device_id") == device_id:
            return device
    return {}


def _gpu_utilization_for_device(gpu_stats: dict, device_id: int) -> float | None:
    """Return the utilization percentage for one GPU device if known."""

    device = _gpu_device_stats(gpu_stats, device_id)
    utilization = device.get("utilization_pct")
    return None if utilization is None else float(utilization)
