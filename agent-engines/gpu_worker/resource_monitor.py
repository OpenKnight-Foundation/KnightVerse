from __future__ import annotations

import asyncio
from collections.abc import Awaitable, Callable
from contextlib import suppress
import shutil
import subprocess
import logging
import time
from typing import Any, Dict, Optional

import psutil

logger = logging.getLogger(__name__)

try:
    import pynvml  # type: ignore
except ImportError:
    pynvml = None

try:
    import redis
except ImportError:
    redis = None


class ResourceMonitor:
    """Monitor GPU and CPU resource utilization with enhanced queue monitoring."""

    def __init__(
        self,
        poll_interval_seconds: float = 1.0,
        gpu_stats_provider: Callable[[], dict[str, Any]] | None = None,
        cpu_stats_provider: Callable[[], dict[str, Any]] | None = None,
        redis_config: Dict[str, Any] | None = None,
        gpu_memory_threshold_percent: float = 90.0,
    ) -> None:
        self.poll_interval_seconds = poll_interval_seconds
        self.gpu_memory_threshold_percent = gpu_memory_threshold_percent
        self._gpu_stats_provider = gpu_stats_provider or self._collect_gpu_stats
        self._cpu_stats_provider = cpu_stats_provider or self._collect_cpu_stats
        self._gpu_stats: dict[str, Any] = {}
        self._cpu_stats: dict[str, Any] = {}
        self._queue_stats: dict[str, Any] = {}
        self._task: asyncio.Task[None] | None = None
        self._stop_event = asyncio.Event()
        
        # Redis configuration for queue monitoring
        self._redis_client: Optional[Any] = None
        if redis_config and redis is not None:
            try:
                self._redis_client = redis.Redis(
                    host=redis_config.get("host", "localhost"),
                    port=redis_config.get("port", 6379),
                    db=redis_config.get("db", 0),
                    decode_responses=True
                )
                self._queue_key = redis_config.get("queue_key", "ai_task_queue")
            except Exception as e:
                logger.error(f"Failed to initialize Redis client: {e}")
                self._redis_client = None

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
        
    def get_queue_stats(self) -> dict[str, Any]:
        """Return the latest queue metrics snapshot."""
        
        if not self._queue_stats:
            self._queue_stats = self._collect_queue_stats()
        return dict(self._queue_stats)
        
    def get_combined_metrics(self) -> dict[str, Any]:
        """Return combined GPU, CPU, and queue metrics."""
        
        return {
            "gpu": self.get_gpu_stats(),
            "cpu": self.get_cpu_stats(),
            "queue": self.get_queue_stats(),
            "timestamp": time.time()
        }
        
    def check_gpu_memory_threshold(self, device_id: Optional[int] = None) -> dict[str, Any]:
        """Check if GPU memory usage exceeds threshold."""
        
        gpu_stats = self.get_gpu_stats()
        threshold_exceeded = []
        
        if gpu_stats.get("available", False):
            for device in gpu_stats.get("devices", []):
                if device_id is not None and device.get("device_id") != device_id:
                    continue
                    
                memory_used = device.get("memory_used_mb", 0)
                memory_total = device.get("memory_total_mb", 1)
                memory_percent = (memory_used / memory_total) * 100 if memory_total > 0 else 0
                
                if memory_percent > self.gpu_memory_threshold_percent:
                    threshold_exceeded.append({
                        "device_id": device.get("device_id"),
                        "memory_percent": memory_percent,
                        "memory_used_mb": memory_used,
                        "memory_total_mb": memory_total,
                        "threshold": self.gpu_memory_threshold_percent
                    })
                    
        return {
            "threshold_exceeded": len(threshold_exceeded) > 0,
            "devices_over_threshold": threshold_exceeded,
            "total_devices": len(gpu_stats.get("devices", [])),
        }
        
    def get_available_gpu_memory(self) -> dict[str, Any]:
        """Get available GPU memory per device."""
        
        gpu_stats = self.get_gpu_stats()
        available_memory = {}
        
        if gpu_stats.get("available", False):
            for device in gpu_stats.get("devices", []):
                device_id = device.get("device_id")
                memory_used = device.get("memory_used_mb", 0)
                memory_total = device.get("memory_total_mb", 0)
                memory_available = max(0, memory_total - memory_used)
                memory_percent_available = (memory_available / memory_total * 100) if memory_total > 0 else 0
                
                available_memory[device_id] = {
                    "available_mb": memory_available,
                    "available_percent": memory_percent_available,
                    "used_mb": memory_used,
                    "total_mb": memory_total,
                    "can_allocate_worker": memory_percent_available > (100 - self.gpu_memory_threshold_percent)
                }
                
        return available_memory

    async def _poll_loop(self) -> None:
        """Periodically refresh CPU, GPU, and queue statistics."""

        while not self._stop_event.is_set():
            self._gpu_stats = self._gpu_stats_provider()
            self._cpu_stats = self._cpu_stats_provider()
            self._queue_stats = self._collect_queue_stats()
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
                            "memory_free_mb": round(memory.free / (1024 * 1024), 2),
                            "memory_utilization_pct": round((memory.used / memory.total) * 100, 2),
                            "temperature_c": float(temperature),
                            "available_for_worker": ((memory.used / memory.total) * 100) < self.gpu_memory_threshold_percent
                        }
                    )
                return {"available": True, "devices": devices}
            except Exception as exc:
                logger.error("NVML GPU error: %s", exc)
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
                    "memory_free_mb": float(memory_total) - float(memory_used),
                    "memory_utilization_pct": round((float(memory_used) / float(memory_total)) * 100, 2),
                    "temperature_c": float(temperature),
                    "available_for_worker": ((float(memory_used) / float(memory_total)) * 100) < self.gpu_memory_threshold_percent
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

    def _collect_queue_stats(self) -> dict[str, Any]:
        """Collect Redis queue statistics."""
        
        stats = {
            "available": False,
            "queue_length": 0,
            "estimated_wait_time_ms": 0.0,
            "error": None
        }
        
        if not self._redis_client:
            stats["error"] = "Redis client not configured"
            return stats
            
        try:
            # Get queue length
            queue_length = self._redis_client.llen(self._queue_key)
            stats["queue_length"] = queue_length
            
            # Estimate wait time based on queue length and processing rate
            # This is a simplified estimation - in production you'd track actual processing times
            estimated_wait_time_ms = queue_length * 100  # Assume 100ms per task average
            stats["estimated_wait_time_ms"] = estimated_wait_time_ms
            
            # Get queue age (time since oldest item was added)
            if queue_length > 0:
                try:
                    # Try to get timestamp from oldest item if items contain timestamps
                    oldest_item = self._redis_client.lindex(self._queue_key, -1)
                    if oldest_item:
                        # In a real implementation, you'd parse the timestamp from the item
                        # For now, estimate based on queue length
                        current_time = time.time()
                        stats["oldest_item_age_seconds"] = queue_length * 0.1  # Rough estimate
                except Exception as e:
                    logger.debug(f"Could not determine queue age: {e}")
                    stats["oldest_item_age_seconds"] = 0
            else:
                stats["oldest_item_age_seconds"] = 0
            
            stats["available"] = True
            
        except Exception as e:
            logger.error(f"Failed to collect queue stats: {e}")
            stats["error"] = str(e)
            
        return stats
