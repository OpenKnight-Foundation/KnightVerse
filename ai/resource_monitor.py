"""Unified monitoring dashboard for AI health (issue #644).

Collects GPU and CPU metrics in a background loop and exposes a
snapshot API consumed by the worker pool and health endpoints.
"""

from __future__ import annotations

import asyncio
import shutil
import subprocess
from contextlib import suppress
from dataclasses import dataclass, field
from typing import Any, Callable

try:
    import psutil  # type: ignore
except ImportError:
    psutil = None  # type: ignore

try:
    import pynvml  # type: ignore
except ImportError:
    pynvml = None  # type: ignore


@dataclass
class DeviceStats:
    device_id: int
    utilization_pct: float = 0.0
    memory_used_mb: float = 0.0
    memory_total_mb: float = 0.0
    temperature_c: float = 0.0

    def to_dict(self) -> dict[str, Any]:
        return {
            "device_id": self.device_id,
            "utilization_pct": self.utilization_pct,
            "memory_used_mb": self.memory_used_mb,
            "memory_total_mb": self.memory_total_mb,
            "temperature_c": self.temperature_c,
        }


@dataclass
class DashboardSnapshot:
    """Point-in-time snapshot of all monitored resources."""

    gpu_available: bool = False
    gpu_devices: list[DeviceStats] = field(default_factory=list)
    cpu_utilization_pct: float = 0.0
    memory_used_mb: float = 0.0
    memory_total_mb: float = 0.0
    memory_utilization_pct: float = 0.0

    def to_dict(self) -> dict[str, Any]:
        return {
            "gpu": {
                "available": self.gpu_available,
                "devices": [d.to_dict() for d in self.gpu_devices],
            },
            "cpu": {
                "utilization_pct": self.cpu_utilization_pct,
                "memory_used_mb": self.memory_used_mb,
                "memory_total_mb": self.memory_total_mb,
                "memory_utilization_pct": self.memory_utilization_pct,
            },
        }


class ResourceMonitor:
    """Background monitor that polls GPU and CPU stats on a fixed interval.

    Inject ``gpu_stats_provider`` / ``cpu_stats_provider`` callables in tests
    to avoid real hardware dependencies.
    """

    def __init__(
        self,
        poll_interval_seconds: float = 1.0,
        gpu_stats_provider: Callable[[], dict[str, Any]] | None = None,
        cpu_stats_provider: Callable[[], dict[str, Any]] | None = None,
    ) -> None:
        self._interval = poll_interval_seconds
        self._gpu_provider = gpu_stats_provider or self._collect_gpu
        self._cpu_provider = cpu_stats_provider or self._collect_cpu
        self._snapshot = DashboardSnapshot()
        self._task: asyncio.Task[None] | None = None
        self._stop = asyncio.Event()

    async def start(self) -> None:
        """Start the background polling loop."""
        if self._task and not self._task.done():
            return
        self._stop = asyncio.Event()
        self._task = asyncio.create_task(self._loop())

    async def stop(self) -> None:
        """Stop the background polling loop."""
        if not self._task:
            return
        self._stop.set()
        self._task.cancel()
        with suppress(asyncio.CancelledError):
            await self._task
        self._task = None

    def snapshot(self) -> DashboardSnapshot:
        """Return the latest resource snapshot."""
        return self._snapshot

    # Convenience accessors kept for backward-compat with worker.py
    def get_gpu_stats(self) -> dict[str, Any]:
        s = self._snapshot
        return {
            "available": s.gpu_available,
            "devices": [d.to_dict() for d in s.gpu_devices],
        }

    def get_cpu_stats(self) -> dict[str, Any]:
        s = self._snapshot
        return {
            "cpu_utilization_pct": s.cpu_utilization_pct,
            "memory_used_mb": s.memory_used_mb,
            "memory_total_mb": s.memory_total_mb,
            "memory_utilization_pct": s.memory_utilization_pct,
        }

    # ------------------------------------------------------------------ loop

    async def _loop(self) -> None:
        while not self._stop.is_set():
            self._refresh()
            try:
                await asyncio.wait_for(self._stop.wait(), timeout=self._interval)
            except asyncio.TimeoutError:
                continue

    def _refresh(self) -> None:
        gpu_raw = self._gpu_provider()
        cpu_raw = self._cpu_provider()
        devices = [
            DeviceStats(
                device_id=d["device_id"],
                utilization_pct=float(d.get("utilization_pct", 0)),
                memory_used_mb=float(d.get("memory_used_mb", 0)),
                memory_total_mb=float(d.get("memory_total_mb", 0)),
                temperature_c=float(d.get("temperature_c", 0)),
            )
            for d in gpu_raw.get("devices", [])
        ]
        self._snapshot = DashboardSnapshot(
            gpu_available=bool(gpu_raw.get("available", False)),
            gpu_devices=devices,
            cpu_utilization_pct=float(cpu_raw.get("cpu_utilization_pct", 0)),
            memory_used_mb=float(cpu_raw.get("memory_used_mb", 0)),
            memory_total_mb=float(cpu_raw.get("memory_total_mb", 0)),
            memory_utilization_pct=float(cpu_raw.get("memory_utilization_pct", 0)),
        )

    # ------------------------------------------------------------------ collectors

    def _collect_gpu(self) -> dict[str, Any]:
        if pynvml is not None:
            try:
                pynvml.nvmlInit()
                count = pynvml.nvmlDeviceGetCount()
                devices = []
                for i in range(count):
                    h = pynvml.nvmlDeviceGetHandleByIndex(i)
                    util = pynvml.nvmlDeviceGetUtilizationRates(h)
                    mem = pynvml.nvmlDeviceGetMemoryInfo(h)
                    temp = pynvml.nvmlDeviceGetTemperature(h, pynvml.NVML_TEMPERATURE_GPU)
                    devices.append({
                        "device_id": i,
                        "utilization_pct": float(util.gpu),
                        "memory_used_mb": round(mem.used / 1024 ** 2, 2),
                        "memory_total_mb": round(mem.total / 1024 ** 2, 2),
                        "temperature_c": float(temp),
                    })
                return {"available": True, "devices": devices}
            except Exception:
                pass
            finally:
                with suppress(Exception):
                    pynvml.nvmlShutdown()

        if shutil.which("nvidia-smi") is None:
            return {"available": False, "devices": []}

        try:
            out = subprocess.run(
                ["nvidia-smi",
                 "--query-gpu=index,utilization.gpu,memory.used,memory.total,temperature.gpu",
                 "--format=csv,noheader,nounits"],
                check=True, capture_output=True, text=True, timeout=2,
            )
        except Exception:
            return {"available": False, "devices": []}

        devices = []
        for line in out.stdout.strip().splitlines():
            parts = [p.strip() for p in line.split(",")]
            if len(parts) == 5:
                devices.append({
                    "device_id": int(parts[0]),
                    "utilization_pct": float(parts[1]),
                    "memory_used_mb": float(parts[2]),
                    "memory_total_mb": float(parts[3]),
                    "temperature_c": float(parts[4]),
                })
        return {"available": True, "devices": devices}

    def _collect_cpu(self) -> dict[str, Any]:
        if psutil is None:
            return {"cpu_utilization_pct": 0.0, "memory_used_mb": 0.0,
                    "memory_total_mb": 0.0, "memory_utilization_pct": 0.0}
        vm = psutil.virtual_memory()
        return {
            "cpu_utilization_pct": float(psutil.cpu_percent(interval=None)),
            "memory_used_mb": round(vm.used / 1024 ** 2, 2),
            "memory_total_mb": round(vm.total / 1024 ** 2, 2),
            "memory_utilization_pct": float(vm.percent),
        }
