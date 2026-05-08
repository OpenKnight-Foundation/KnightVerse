from __future__ import annotations

import asyncio
from collections.abc import Awaitable, Callable
from contextlib import suppress
import shutil
import subprocess
from typing import Any

import psutil

try:
    import pynvml  # type: ignore
except ImportError:
    pynvml = None


class ResourceMonitor:
    """Monitor GPU and CPU resource utilization."""

    def __init__(
        self,
        poll_interval_seconds: float = 1.0,
        gpu_stats_provider: Callable[[], dict[str, Any]] | None = None,
        cpu_stats_provider: Callable[[], dict[str, Any]] | None = None,
    ) -> None:
        self.poll_interval_seconds = poll_interval_seconds
        self._gpu_stats_provider = gpu_stats_provider or self._collect_gpu_stats
        self._cpu_stats_provider = cpu_stats_provider or self._collect_cpu_stats
        self._gpu_stats: dict[str, Any] = {}
        self._cpu_stats: dict[str, Any] = {}
        self._task: asyncio.Task[None] | None = None
        self._stop_event = asyncio.Event()

    async def start(self) -> None:
        """Start the background monitoring loop if not already running."""

        if self._task and not self._task.done():
            return
        self._stop_event = asyncio.Event()
        self._task = asyncio.create_task(self._poll_loop())

    async def stop(self) -> None:
        """Stop the background monitoring loop."""

        if not self._task:
            return
        self._stop_event.set()
        self._task.cancel()
        with suppress(asyncio.CancelledError):
            await self._task
        self._task = None

    def get_gpu_stats(self) -> dict[str, Any]:
        """Return the latest GPU utilization snapshot."""

        if not self._gpu_stats:
            self._gpu_stats = self._gpu_stats_provider()
        return dict(self._gpu_stats)

    def get_cpu_stats(self) -> dict[str, Any]:
        """Return the latest CPU utilization snapshot."""

        if not self._cpu_stats:
            self._cpu_stats = self._cpu_stats_provider()
        return dict(self._cpu_stats)

    async def _poll_loop(self) -> None:
        """Periodically refresh CPU and GPU statistics."""

        while not self._stop_event.is_set():
            self._gpu_stats = self._gpu_stats_provider()
            self._cpu_stats = self._cpu_stats_provider()
            try:
                await asyncio.wait_for(
                    self._stop_event.wait(), timeout=self.poll_interval_seconds
                )
            except asyncio.TimeoutError:
                continue

    def _collect_gpu_stats(self) -> dict[str, Any]:
        """Collect GPU metrics using NVML or nvidia-smi when available."""

        if pynvml is not None:
            try:
                pynvml.nvmlInit()
                device_count = pynvml.nvmlDeviceGetCount()
                devices: list[dict[str, Any]] = []
                for index in range(device_count):
                    handle = pynvml.nvmlDeviceGetHandleByIndex(index)
                    utilization = pynvml.nvmlDeviceGetUtilizationRates(handle)
                    memory = pynvml.nvmlDeviceGetMemoryInfo(handle)
                    temperature = pynvml.nvmlDeviceGetTemperature(
                        handle, pynvml.NVML_TEMPERATURE_GPU
                    )
                    devices.append(
                        {
                            "device_id": index,
                            "utilization_pct": float(utilization.gpu),
                            "memory_used_mb": round(memory.used / (1024 * 1024), 2),
                            "memory_total_mb": round(memory.total / (1024 * 1024), 2),
                            "temperature_c": float(temperature),
                        }
                    )
                return {"available": True, "devices": devices}
            except Exception:
                pass
            finally:
                with suppress(Exception):
                    pynvml.nvmlShutdown()

        if shutil.which("nvidia-smi") is None:
            return {"available": False, "devices": []}

        try:
            output = subprocess.run(
                [
                    "nvidia-smi",
                    "--query-gpu=index,utilization.gpu,memory.used,memory.total,temperature.gpu",
                    "--format=csv,noheader,nounits",
                ],
                check=True,
                capture_output=True,
                text=True,
                timeout=2,
            )
        except (subprocess.CalledProcessError, subprocess.TimeoutExpired, FileNotFoundError):
            return {"available": False, "devices": []}

        devices = []
        for line in output.stdout.strip().splitlines():
            if not line.strip():
                continue
            index, utilization, memory_used, memory_total, temperature = [
                part.strip() for part in line.split(",")
            ]
            devices.append(
                {
                    "device_id": int(index),
                    "utilization_pct": float(utilization),
                    "memory_used_mb": float(memory_used),
                    "memory_total_mb": float(memory_total),
                    "temperature_c": float(temperature),
                }
            )
        return {"available": True, "devices": devices}

    def _collect_cpu_stats(self) -> dict[str, Any]:
        """Collect host CPU and RAM utilization metrics."""

        virtual_memory = psutil.virtual_memory()
        return {
            "cpu_utilization_pct": float(psutil.cpu_percent(interval=None)),
            "memory_used_mb": round(virtual_memory.used / (1024 * 1024), 2),
            "memory_total_mb": round(virtual_memory.total / (1024 * 1024), 2),
            "memory_utilization_pct": float(virtual_memory.percent),
        }
