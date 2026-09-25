from __future__ import annotations

import asyncio
import json
import logging
import os
import psutil
import signal
import subprocess
import time
from dataclasses import dataclass, field
from typing import Dict, Optional, List, Callable, Any
from enum import Enum
from multiprocessing import Process

try:
    import redis
except ImportError:
    redis = None

try:
    import pynvml
    PYNVML_AVAILABLE = True
except ImportError:
    pynvml = None
    PYNVML_AVAILABLE = False

import prometheus_client
from prometheus_client import Counter, Gauge, Histogram

logger = logging.getLogger("KnightVerse.ResourceOptimizer")

# Initialize NVML if available
_NVML_INITIALIZED = False
def _init_nvml():
    global _NVML_INITIALIZED
    if PYNVML_AVAILABLE and not _NVML_INITIALIZED:
        try:
            pynvml.nvmlInit()
            _NVML_INITIALIZED = True
            logger.info("NVML initialized successfully for GPU monitoring")
        except Exception as e:
            logger.warning(f"Failed to initialize NVML: {e}")


class ResourceTier(Enum):
    """Resource allocation tiers for different engine workloads."""
    LIGHT = "light"          # Minimal resources, quick responses
    STANDARD = "standard"    # Balanced performance
    HIGH = "high"           # Maximum performance for complex analysis
    UNLIMITED = "unlimited" # No constraints (testing/development)


class ScalingEvent(Enum):
    """Types of autoscaling events."""
    SCALE_UP = "scale_up"
    SCALE_DOWN = "scale_down"
    NO_SCALING = "no_scaling"


@dataclass
class ResourceLimits:
    """Defines resource constraints for an engine instance."""
    max_threads: int = 2
    max_memory_mb: int = 512
    max_cpu_percent: float = 50.0
    max_gpu_memory_mb: int = 1024
    tier: ResourceTier = ResourceTier.STANDARD


@dataclass
class ResourceMetrics:
    """Current resource utilization metrics."""
    cpu_percent: float = 0.0
    memory_used_mb: float = 0.0
    memory_available_mb: float = 0.0
    gpu_utilization: float = 0.0
    gpu_memory_used_mb: float = 0.0
    active_workers: int = 0
    queued_tasks: int = 0


@dataclass
class AutoscalingConfig:
    """Configuration for autoscaling daemon."""
    min_workers: int = 2
    max_workers: int = 10
    scale_up_queue_threshold: int = 50
    scale_up_latency_threshold_ms: float = 500.0
    scale_down_idle_timeout_seconds: int = 300  # 5 minutes
    redis_host: str = "localhost"
    redis_port: int = 6379
    redis_db: int = 0
    redis_queue_key: str = "ai_task_queue"
    monitoring_interval_seconds: float = 10.0
    gpu_memory_threshold_percent: float = 90.0
    gpu_memory_per_worker_mb: int = 1024  # Minimum GPU memory required per worker
    graceful_shutdown_timeout_seconds: int = 300  # 5 minutes to wait for in-flight tasks


@dataclass
class WorkerProcess:
    """Represents a managed worker process."""
    process_id: int
    gpu_device_id: int
    started_at: float
    last_active: float
    is_busy: bool = False
    process_handle: Optional[Process] = None
    current_task_started_at: Optional[float] = None
    current_task_id: Optional[str] = None
    processing_durations: List[float] = field(default_factory=list)


# Prometheus Metrics for Autoscaling
ACTIVE_WORKERS = Gauge('ai_autoscaler_active_workers', 'Number of active AI workers')
QUEUE_LENGTH = Gauge('ai_autoscaler_queue_length', 'Length of AI task queue')
QUEUE_LATENCY = Histogram('ai_autoscaler_queue_latency_seconds', 'Queue latency in seconds')
SCALING_EVENTS = Counter('ai_autoscaler_scaling_events_total', 'Total scaling events', ['event_type'])
GPU_MEMORY_UTILIZATION = Gauge('ai_autoscaler_gpu_memory_utilization_percent', 'GPU memory utilization', ['gpu_device'])

# Additional metrics for worker processing time tracking
WORKER_PROCESSING_DURATION = Histogram('ai_worker_processing_duration_seconds', 'Worker processing duration', ['worker_id'])
WORKER_TASKS_COMPLETED = Counter('ai_worker_tasks_completed_total', 'Total tasks completed by worker', ['worker_id'])


class AutoscalingDaemon:
    """
    Dynamic autoscaling daemon that monitors Redis queue and GPU resources
    to automatically scale worker processes up/down based on demand.
    """

    def __init__(self, config: AutoscalingConfig, worker_factory: Optional[Callable] = None):
        """
        Initialize the autoscaling daemon.
        
        Args:
            config: Autoscaling configuration
            worker_factory: Factory function to create worker processes
        """
        if redis is None:
            raise ImportError("Redis is required for autoscaling. Install with: pip install redis")
            
        self.config = config
        self.worker_factory = worker_factory or self._default_worker_factory
        self._redis_client = redis.Redis(
            host=config.redis_host,
            port=config.redis_port,
            db=config.redis_db,
            decode_responses=True
        )
        self._workers: Dict[int, WorkerProcess] = {}
        self._running = False
        self._monitoring_task: Optional[asyncio.Task] = None
        self._shutdown_event = asyncio.Event()
        
    async def start(self) -> None:
        """Start the autoscaling daemon."""
        if self._running:
            logger.warning("Autoscaling daemon is already running")
            return
            
        logger.info("Starting autoscaling daemon")
        self._running = True
        self._shutdown_event.clear()
        
        # Start with minimum number of workers
        await self._scale_to_target(self.config.min_workers)
        
        # Start monitoring loop
        self._monitoring_task = asyncio.create_task(self._monitoring_loop())
        
        # Set up signal handlers for graceful shutdown
        for sig in [signal.SIGTERM, signal.SIGINT]:
            signal.signal(sig, self._signal_handler)
        
        logger.info(f"Autoscaling daemon started with {len(self._workers)} workers")
        
    async def stop(self, timeout: Optional[float] = 30.0) -> None:
        """Stop the autoscaling daemon gracefully."""
        if not self._running:
            return
            
        logger.info("Stopping autoscaling daemon")
        self._running = False
        self._shutdown_event.set()
        
        # Cancel monitoring task
        if self._monitoring_task:
            self._monitoring_task.cancel()
            try:
                await asyncio.wait_for(self._monitoring_task, timeout=5.0)
            except (asyncio.TimeoutError, asyncio.CancelledError):
                logger.warning("Monitoring task did not stop gracefully")
        
        # Gracefully shutdown all workers
        await self._shutdown_all_workers(timeout)
        
        logger.info("Autoscaling daemon stopped")
        
    def _signal_handler(self, signum: int, frame) -> None:
        """Handle shutdown signals."""
        logger.info(f"Received signal {signum}, initiating graceful shutdown")
        asyncio.create_task(self.stop())
        
    async def _monitoring_loop(self) -> None:
        """Main monitoring loop that checks queue metrics and scales workers."""
        logger.info("Starting monitoring loop")
        
        while self._running:
            try:
                # Get current metrics
                queue_metrics = await self._get_queue_metrics()
                resource_metrics = await self._get_resource_metrics()
                
                # Update Prometheus metrics
                self._update_prometheus_metrics(queue_metrics, resource_metrics)
                
                # Make scaling decision
                scaling_decision = await self._make_scaling_decision(queue_metrics, resource_metrics)
                
                # Execute scaling action
                await self._execute_scaling_action(scaling_decision, queue_metrics, resource_metrics)
                
                # Clean up dead workers
                await self._cleanup_dead_workers()
                
            except Exception as e:
                logger.error(f"Error in monitoring loop: {e}", exc_info=True)
                
            # Wait for next monitoring cycle
            try:
                await asyncio.wait_for(
                    self._shutdown_event.wait(), 
                    timeout=self.config.monitoring_interval_seconds
                )
                break  # Shutdown requested
            except asyncio.TimeoutError:
                continue  # Continue monitoring
                
        logger.info("Monitoring loop stopped")
        
    async def _get_queue_metrics(self) -> Dict[str, Any]:
        """Get Redis queue metrics."""
        try:
            # Get queue length
            queue_length = self._redis_client.llen(self.config.redis_queue_key)

            # Calculate average wait time using real enqueue timestamps from queue items
            avg_wait_time_ms = 0.0
            if queue_length > 0:
                # Sample items from the queue to estimate wait time
                sample_size = min(10, queue_length)
                current_time = time.time()
                total_wait = 0.0
                valid_samples = 0

                for i in range(sample_size):
                    item = self._redis_client.lindex(self.config.redis_queue_key, i)
                    if item:
                        try:
                            # Parse the queue item - expect JSON with 'enqueued_at' timestamp
                            task_data = json.loads(item)
                            enqueued_at = task_data.get('enqueued_at')
                            if enqueued_at:
                                wait_time = current_time - float(enqueued_at)
                                total_wait += wait_time
                                valid_samples += 1
                            else:
                                # Fallback: if no timestamp, skip this item
                                logger.debug(f"Queue item at index {i} missing enqueued_at timestamp")
                        except (json.JSONDecodeError, ValueError, TypeError) as e:
                            logger.debug(f"Failed to parse queue item at index {i}: {e}")
                            continue

                if valid_samples > 0:
                    avg_wait_time_ms = (total_wait / valid_samples) * 1000
                else:
                    logger.debug("No queue items with valid enqueued_at timestamps found")

            return {
                "queue_length": queue_length,
                "avg_wait_time_ms": avg_wait_time_ms
            }

        except Exception as e:
            logger.error(f"Failed to get queue metrics: {e}")
            return {"queue_length": 0, "avg_wait_time_ms": 0.0, "error": str(e)}
            
    async def _get_resource_metrics(self) -> ResourceMetrics:
        """Get current system resource metrics."""
        # Get basic system metrics
        cpu_percent = psutil.cpu_percent(interval=0.1)
        memory = psutil.virtual_memory()
        memory_used_mb = memory.used / (1024 * 1024)
        memory_available_mb = memory.available / (1024 * 1024)

        # Get GPU metrics using NVML if available
        gpu_utilization = 0.0
        gpu_memory_used_mb = 0.0
        
        if PYNVML_AVAILABLE:
            _init_nvml()
            if _NVML_INITIALIZED:
                try:
                    device_count = pynvml.nvmlDeviceGetCount()
                    total_gpu_memory_used = 0
                    total_gpu_utilization = 0
                    
                    for i in range(device_count):
                        handle = pynvml.nvmlDeviceGetHandleByIndex(i)
                        mem_info = pynvml.nvmlDeviceGetMemoryInfo(handle)
                        total_gpu_memory_used += mem_info.used / (1024 * 1024)
                        
                        util = pynvml.nvmlDeviceGetUtilizationRates(handle)
                        total_gpu_utilization += util.gpu
                    
                    if device_count > 0:
                        gpu_memory_used_mb = total_gpu_memory_used
                        gpu_utilization = total_gpu_utilization / device_count
                        
                except Exception as e:
                    logger.warning(f"Failed to query GPU metrics via NVML: {e}")

        # Count active workers
        active_workers = len([w for w in self._workers.values() if w.is_busy])

        return ResourceMetrics(
            cpu_percent=cpu_percent,
            memory_used_mb=memory_used_mb,
            memory_available_mb=memory_available_mb,
            gpu_utilization=gpu_utilization,
            gpu_memory_used_mb=gpu_memory_used_mb,
            active_workers=active_workers,
            queued_tasks=0  # Will be updated from queue metrics
        )
        
    def _update_prometheus_metrics(self, queue_metrics: Dict, resource_metrics: ResourceMetrics) -> None:
        """Update Prometheus metrics with current values."""
        ACTIVE_WORKERS.set(len(self._workers))
        QUEUE_LENGTH.set(queue_metrics["queue_length"])
        QUEUE_LATENCY.observe(queue_metrics["avg_wait_time_ms"] / 1000.0)
        
        # Update GPU metrics per device
        for worker in self._workers.values():
            GPU_MEMORY_UTILIZATION.labels(gpu_device=str(worker.gpu_device_id)).set(
                resource_metrics.gpu_utilization
            )
            
    async def _make_scaling_decision(self, queue_metrics: Dict, resource_metrics: ResourceMetrics) -> ScalingEvent:
        """Determine if scaling action is needed based on current metrics."""
        current_workers = len(self._workers)
        queue_length = queue_metrics["queue_length"]
        avg_wait_time_ms = queue_metrics["avg_wait_time_ms"]
        
        # Check scale-up conditions
        if (queue_length > self.config.scale_up_queue_threshold or 
            avg_wait_time_ms > self.config.scale_up_latency_threshold_ms):
            
            if current_workers < self.config.max_workers:
                # Check if we have available GPU memory
                if await self._has_available_gpu_capacity():
                    logger.info(f"Scale-up triggered: queue_length={queue_length}, "
                               f"avg_wait_time={avg_wait_time_ms}ms")
                    return ScalingEvent.SCALE_UP
                else:
                    logger.warning("Scale-up requested but no available GPU capacity")
                    return ScalingEvent.NO_SCALING
            else:
                logger.warning(f"Scale-up requested but already at max workers ({current_workers})")
                return ScalingEvent.NO_SCALING
        
        # Check scale-down conditions
        if queue_length == 0 and current_workers > self.config.min_workers:
            # Check if any workers have been idle long enough
            idle_workers = await self._get_idle_workers()
            if idle_workers:
                current_time = time.time()
                for worker in idle_workers:
                    idle_time = current_time - worker.last_active
                    if idle_time > self.config.scale_down_idle_timeout_seconds:
                        logger.info(f"Scale-down triggered: worker {worker.process_id} "
                                   f"idle for {idle_time:.1f}s")
                        return ScalingEvent.SCALE_DOWN
        
        return ScalingEvent.NO_SCALING
        
    async def _execute_scaling_action(self, action: ScalingEvent, queue_metrics: Dict, resource_metrics: ResourceMetrics) -> None:
        """Execute the determined scaling action."""
        if action == ScalingEvent.SCALE_UP:
            await self._scale_up()
            SCALING_EVENTS.labels(event_type="scale_up").inc()
        elif action == ScalingEvent.SCALE_DOWN:
            await self._scale_down()
            SCALING_EVENTS.labels(event_type="scale_down").inc()
            
    async def _scale_up(self) -> None:
        """Add a new worker process."""
        try:
            # Find available GPU device
            gpu_device_id = await self._find_available_gpu_device()
            if gpu_device_id is None:
                logger.error("No available GPU devices for scale-up")
                return
                
            # Create new worker process
            worker_process = await self._create_worker(gpu_device_id)
            if worker_process:
                self._workers[worker_process.process_id] = worker_process
                logger.info(f"Scaled up: added worker {worker_process.process_id} "
                           f"on GPU {gpu_device_id}")
            else:
                logger.error("Failed to create new worker process")
                
        except Exception as e:
            logger.error(f"Error during scale-up: {e}", exc_info=True)
            
    async def _scale_down(self) -> None:
        """Remove an idle worker process gracefully."""
        try:
            idle_workers = await self._get_idle_workers()
            if not idle_workers:
                logger.warning("No idle workers available for scale-down")
                return
                
            # Find the worker that has been idle the longest
            current_time = time.time()
            longest_idle_worker = max(idle_workers, 
                                    key=lambda w: current_time - w.last_active)
            
            # Gracefully terminate the worker
            await self._terminate_worker(longest_idle_worker.process_id, graceful=True)
            
            logger.info(f"Scaled down: removed worker {longest_idle_worker.process_id}")
            
        except Exception as e:
            logger.error(f"Error during scale-down: {e}", exc_info=True)
            
    async def _has_available_gpu_capacity(self) -> bool:
        """Check if there's available GPU memory capacity for new workers."""
        try:
            if not PYNVML_AVAILABLE:
                logger.warning("pynvml not available, cannot check GPU capacity - refusing scale-up (fail closed)")
                return False
            
            _init_nvml()
            if not _NVML_INITIALIZED:
                logger.warning("NVML not initialized, cannot check GPU capacity - refusing scale-up (fail closed)")
                return False
            
            # Check each GPU device for available memory
            device_count = pynvml.nvmlDeviceGetCount()
            for i in range(device_count):
                handle = pynvml.nvmlDeviceGetHandleByIndex(i)
                mem_info = pynvml.nvmlDeviceGetMemoryInfo(handle)
                
                total_memory_mb = mem_info.total / (1024 * 1024)
                used_memory_mb = mem_info.used / (1024 * 1024)
                free_memory_mb = mem_info.free / (1024 * 1024)
                usage_percent = (used_memory_mb / total_memory_mb) * 100
                
                # Check if this GPU has enough free memory for a new worker
                # We need at least the configured GPU memory per worker plus some buffer
                required_memory_mb = getattr(self.config, 'gpu_memory_per_worker_mb', 1024)
                if free_memory_mb >= required_memory_mb and usage_percent < self.config.gpu_memory_threshold_percent:
                    logger.debug(f"GPU {i} has capacity: {free_memory_mb:.0f}MB free, {usage_percent:.1f}% used")
                    return True
                else:
                    logger.debug(f"GPU {i} at capacity: {free_memory_mb:.0f}MB free, {usage_percent:.1f}% used")
            
            logger.warning("No GPU devices have available capacity for new worker")
            return False
            
        except Exception as e:
            logger.error(f"Error checking GPU capacity: {e}")
            return False  # Fail closed - refuse to scale up if we can't verify capacity
            
    async def _find_available_gpu_device(self) -> Optional[int]:
        """Find the least utilized GPU device for a new worker."""
        if PYNVML_AVAILABLE:
            _init_nvml()
            if _NVML_INITIALIZED:
                try:
                    device_count = pynvml.nvmlDeviceGetCount()
                    if device_count > 0:
                        utilization = [
                            (i, pynvml.nvmlDeviceGetUtilizationRates(
                                pynvml.nvmlDeviceGetHandleByIndex(i)).gpu)
                            for i in range(device_count)
                        ]
                        return min(utilization, key=lambda item: item[1])[0]
                except Exception as e:
                    logger.warning(f"Failed to query GPU utilization via NVML: {e}")

        # Without NVML, fall back to spreading workers across device ids
        used_devices = {w.gpu_device_id for w in self._workers.values()}
        
        # Try devices 0-7 (common GPU setup)
        for device_id in range(8):
            if device_id not in used_devices:
                return device_id
                
        # If all devices are used, assign to the least loaded one
        if self._workers:
            device_counts = {}
            for worker in self._workers.values():
                device_counts[worker.gpu_device_id] = device_counts.get(worker.gpu_device_id, 0) + 1
            return min(device_counts, key=device_counts.get)
            
        return 0  # Default to device 0
        
    async def _get_idle_workers(self) -> List[WorkerProcess]:
        """Get list of workers that are not busy."""
        return [worker for worker in self._workers.values() if not worker.is_busy]
        
    async def _create_worker(self, gpu_device_id: int) -> Optional[WorkerProcess]:
        """Create a new worker process."""
        try:
            # Spawn an actual worker process using the worker factory
            process_handle = self.worker_factory(gpu_device_id)
            
            current_time = time.time()
            
            worker = WorkerProcess(
                process_id=len(self._workers) + 1000,  # Simple ID generation
                gpu_device_id=gpu_device_id,
                started_at=current_time,
                last_active=current_time,
                is_busy=False,
                process_handle=process_handle
            )
            
            return worker
            
        except Exception as e:
            logger.error(f"Failed to create worker: {e}")
            return None
            
    def _default_worker_factory(self, gpu_device_id: int) -> Optional[Process]:
        """Default factory for creating worker processes.
        
        Spawns an actual GPU worker subprocess that can process analysis requests.
        The worker_main function should be defined in the caller's module or imported.
        """
        try:
            # Import here to avoid circular imports
            from gpu_worker.worker import GPUWorker
            from gpu_worker.config import WorkerConfig, EngineBackend, GPUConfig
            
            # Create a worker config for this GPU device
            config = WorkerConfig(
                engine_backend=EngineBackend.LC0,
                engine_path="/usr/local/bin/lc0",  # Should be configurable
                gpu=GPUConfig(device_id=gpu_device_id, max_batch_size=32, memory_limit_mb=2048, backend="cudnn"),
                default_depth=22,
                default_time_limit_ms=3000,
                threads=2,
                hash_size_mb=512,
            )
            
            # Create the worker instance
            worker = GPUWorker(config)
            
            # Return a Process that runs the worker's main loop
            return Process(target=self._run_worker, args=(worker,), daemon=True)
            
        except ImportError:
            logger.warning("GPUWorker not available, cannot spawn real worker")
            return None
        except Exception as e:
            logger.error(f"Failed to create real worker process: {e}")
            return None

    def _run_worker(self, worker):
        """Run the worker's main processing loop in a separate process."""
        import asyncio
        asyncio.run(worker.start())
        # Keep the process alive to handle requests
        # The worker will process tasks from its internal queue
        asyncio.get_event_loop().run_forever()
        
    async def _terminate_worker(self, process_id: int, graceful: bool = True) -> None:
        """Terminate a worker process."""
        if process_id not in self._workers:
            logger.warning(f"Worker {process_id} not found for termination")
            return
            
        worker = self._workers[process_id]
        
        try:
            if graceful and worker.is_busy:
                logger.info(f"Worker {process_id} is busy, waiting for current task to complete before termination")
                # Wait for the current task to complete (up to a configurable timeout)
                timeout = getattr(self.config, 'graceful_shutdown_timeout_seconds', 300)
                wait_start = time.time()
                
                while worker.is_busy and (time.time() - wait_start) < timeout:
                    await asyncio.sleep(1.0)
                
                if worker.is_busy:
                    logger.warning(f"Worker {process_id} did not complete task within {timeout}s, forcing termination")
                else:
                    logger.info(f"Worker {process_id} completed task gracefully")
            
            # Terminate the process
            if worker.process_handle:
                worker.process_handle.terminate()
                worker.process_handle.join(timeout=5.0)
                if worker.process_handle.is_alive():
                    logger.warning(f"Worker {process_id} did not terminate gracefully, killing")
                    worker.process_handle.kill()
                    
            # Remove from workers dict
            del self._workers[process_id]
            
            logger.info(f"Worker {process_id} terminated successfully")
            
        except Exception as e:
            logger.error(f"Error terminating worker {process_id}: {e}")
            
    async def _cleanup_dead_workers(self) -> None:
        """Remove dead worker processes from tracking."""
        dead_workers = []
        
        for process_id, worker in self._workers.items():
            if worker.process_handle and not worker.process_handle.is_alive():
                logger.warning(f"Detected dead worker {process_id}")
                dead_workers.append(process_id)
                
        for process_id in dead_workers:
            del self._workers[process_id]
            
    async def _scale_to_target(self, target_workers: int) -> None:
        """Scale worker pool to target number of workers."""
        current_workers = len(self._workers)
        
        if target_workers > current_workers:
            # Scale up
            for _ in range(target_workers - current_workers):
                await self._scale_up()
        elif target_workers < current_workers:
            # Scale down
            for _ in range(current_workers - target_workers):
                await self._scale_down()
                
    async def _shutdown_all_workers(self, timeout: Optional[float] = None) -> None:
        """Shutdown all worker processes gracefully."""
        logger.info(f"Shutting down {len(self._workers)} workers")
        
        # First, try graceful shutdown
        shutdown_tasks = []
        for process_id in list(self._workers.keys()):
            task = asyncio.create_task(self._terminate_worker(process_id, graceful=True))
            shutdown_tasks.append(task)
            
        if shutdown_tasks:
            try:
                await asyncio.wait_for(
                    asyncio.gather(*shutdown_tasks, return_exceptions=True),
                    timeout=timeout
                )
            except asyncio.TimeoutError:
                logger.warning("Graceful shutdown timed out, forcing termination")
                
                # Force kill remaining workers
                for worker in self._workers.values():
                    if worker.process_handle and worker.process_handle.is_alive():
                        worker.process_handle.kill()
                        
        self._workers.clear()
        logger.info("All workers shut down")
        
    def get_status(self) -> Dict[str, Any]:
        """Get current autoscaling daemon status."""
        return {
            "running": self._running,
            "worker_count": len(self._workers),
            "min_workers": self.config.min_workers,
            "max_workers": self.config.max_workers,
            "workers": {
                worker_id: {
                    "gpu_device_id": worker.gpu_device_id,
                    "started_at": worker.started_at,
                    "last_active": worker.last_active,
                    "is_busy": worker.is_busy
                }
                for worker_id, worker in self._workers.items()
            }
        }


class ResourceOptimizer:
    """
    Optimizes resource allocation for AI engines based on system capacity
    and workload demands. Ensures efficient CPU/GPU utilization.
    Now includes autoscaling daemon integration.
    """

    def __init__(self, reserved_cpu_percent: float = 20.0, reserved_memory_mb: int = 1024,
                 autoscaling_config: Optional[AutoscalingConfig] = None):
        """
        Initialize the resource optimizer.
        
        Args:
            reserved_cpu_percent: CPU percentage to keep reserved for system
            reserved_memory_mb: Memory (MB) to keep reserved for system
            autoscaling_config: Optional autoscaling configuration
        """
        self.reserved_cpu_percent = reserved_cpu_percent
        self.reserved_memory_mb = reserved_memory_mb
        self._allocation_history: Dict[str, List[ResourceLimits]] = {}
        
        # Initialize autoscaling daemon if configured
        self._autoscaling_daemon: Optional[AutoscalingDaemon] = None
        if autoscaling_config:
            self._autoscaling_daemon = AutoscalingDaemon(autoscaling_config)
            
    async def start_autoscaling(self) -> None:
        """Start the autoscaling daemon if configured."""
        if self._autoscaling_daemon:
            await self._autoscaling_daemon.start()
            logger.info("Autoscaling daemon started")
        else:
            logger.warning("No autoscaling configuration provided")
            
    async def stop_autoscaling(self, timeout: Optional[float] = 30.0) -> None:
        """Stop the autoscaling daemon gracefully."""
        if self._autoscaling_daemon:
            await self._autoscaling_daemon.stop(timeout)
            logger.info("Autoscaling daemon stopped")
            
    def get_autoscaling_status(self) -> Optional[Dict[str, Any]]:
        """Get current autoscaling daemon status."""
        if self._autoscaling_daemon:
            return self._autoscaling_daemon.get_status()
        return None
        
    def get_system_capacity(self) -> Dict[str, float]:
        """Get total system resource capacity."""
        cpu_count = psutil.cpu_count(logical=True)
        total_memory_mb = psutil.virtual_memory().total / (1024 * 1024)
        
        return {
            "cpu_cores": cpu_count,
            "total_memory_mb": total_memory_mb,
            "available_cpu_percent": 100.0 - self.reserved_cpu_percent,
            "available_memory_mb": total_memory_mb - self.reserved_memory_mb
        }
    
    def get_current_metrics(self) -> ResourceMetrics:
        """Get current system resource utilization."""
        cpu_percent = psutil.cpu_percent(interval=0.1)
        memory = psutil.virtual_memory()
        memory_used_mb = memory.used / (1024 * 1024)
        memory_available_mb = memory.available / (1024 * 1024)
        
        # GPU metrics would require nvidia-ml-py or similar
        # Placeholder for now
        gpu_utilization = 0.0
        gpu_memory_used_mb = 0.0
        
        return ResourceMetrics(
            cpu_percent=cpu_percent,
            memory_used_mb=memory_used_mb,
            memory_available_mb=memory_available_mb,
            gpu_utilization=gpu_utilization,
            gpu_memory_used_mb=gpu_memory_used_mb
        )
    
    def calculate_optimal_allocation(
        self,
        engine_id: str,
        tier: ResourceTier = ResourceTier.STANDARD,
        current_load: float = 0.0
    ) -> ResourceLimits:
        """
        Calculate optimal resource allocation for an engine based on
        system capacity, tier requirements, and current load.
        
        Args:
            engine_id: Unique identifier for the engine
            tier: Resource tier for this engine
            current_load: Current system load (0.0 to 1.0)
            
        Returns:
            ResourceLimits with optimized allocation
        """
        capacity = self.get_system_capacity()
        metrics = self.get_current_metrics()
        
        # Calculate available resources
        available_cpu = capacity["available_cpu_percent"] - metrics.cpu_percent
        available_memory = capacity["available_memory_mb"] - metrics.memory_used_mb
        
        # Ensure we don't allocate negative resources
        available_cpu = max(0, available_cpu)
        available_memory = max(0, available_memory)
        
        # Tier-based multipliers
        tier_multipliers = {
            ResourceTier.LIGHT: 0.25,
            ResourceTier.STANDARD: 0.5,
            ResourceTier.HIGH: 0.75,
            ResourceTier.UNLIMITED: 1.0
        }
        
        multiplier = tier_multipliers[tier]
        
        # Calculate allocation based on tier and availability
        max_threads = max(1, int(capacity["cpu_cores"] * multiplier))
        max_memory = int(available_memory * multiplier)
        max_cpu = available_cpu * multiplier
        
        # Adjust based on current load
        if current_load > 0.8:
            # System is under heavy load, reduce allocation
            max_threads = max(1, max_threads // 2)
            max_memory = max_memory // 2
            max_cpu = max_cpu * 0.5
        elif current_load < 0.3:
            # System is lightly loaded, can allocate more
            max_threads = min(max_threads * 2, int(capacity["cpu_cores"]))
            max_cpu = min(max_cpu * 1.5, available_cpu)
        
        # Apply hard limits
        max_threads = min(max_threads, int(capacity["cpu_cores"]))
        max_memory = min(max_memory, int(available_memory))
        max_cpu = min(max_cpu, available_cpu)
        
        limits = ResourceLimits(
            max_threads=max_threads,
            max_memory_mb=max_memory,
            max_cpu_percent=max_cpu,
            tier=tier
        )
        
        # Track allocation history
        if engine_id not in self._allocation_history:
            self._allocation_history[engine_id] = []
        self._allocation_history[engine_id].append(limits)
        
        logger.info(
            f"Allocated resources for {engine_id} [{tier.value}]: "
            f"{max_threads} threads, {max_memory}MB RAM, {max_cpu:.1f}% CPU"
        )
        
        return limits
    
    def validate_allocation(self, limits: ResourceLimits) -> bool:
        """
        Validate that resource allocation is within safe bounds.
        
        Args:
            limits: Resource limits to validate
            
        Returns:
            True if allocation is valid, False otherwise
        """
        capacity = self.get_system_capacity()
        metrics = self.get_current_metrics()
        
        # Check CPU allocation
        if limits.max_cpu_percent > capacity["available_cpu_percent"]:
            logger.warning(f"CPU allocation {limits.max_cpu_percent}% exceeds available {capacity['available_cpu_percent']}%")
            return False
        
        # Check memory allocation
        if limits.max_memory_mb > capacity["available_memory_mb"]:
            logger.warning(f"Memory allocation {limits.max_memory_mb}MB exceeds available {capacity['available_memory_mb']}MB")
            return False
        
        # Check thread count
        if limits.max_threads > capacity["cpu_cores"]:
            logger.warning(f"Thread count {limits.max_threads} exceeds available cores {capacity['cpu_cores']}")
            return False
        
        return True
    
    def get_allocation_history(self, engine_id: Optional[str] = None) -> Dict:
        """
        Get resource allocation history.
        
        Args:
            engine_id: Optional engine ID to filter history
            
        Returns:
            Dictionary of allocation history
        """
        if engine_id:
            return {engine_id: self._allocation_history.get(engine_id, [])}
        return self._allocation_history.copy()
    
    def estimate_gas_cost(self, engine_type: str, complexity: int) -> float:
        """
        Estimate computational cost (analogous to gas) for an analysis task.
        This helps optimize resource usage and prevent expensive operations.
        
        Args:
            engine_type: Type of engine (stockfish, lc0, maia, etc.)
            complexity: Analysis depth or complexity level
            
        Returns:
            Estimated computational cost (arbitrary units)
        """
        # Base costs per engine type
        base_costs = {
            "stockfish": 10,
            "lc0": 50,  # GPU-based, more expensive
            "maia": 15,
            "custom": 20
        }
        
        base_cost = base_costs.get(engine_type.lower(), 20)
        
        # Complexity scales quadratically (deeper analysis = much more expensive)
        gas_estimate = base_cost * (complexity ** 1.5)
        
        logger.debug(f"Estimated gas cost for {engine_type} at depth {complexity}: {gas_estimate:.2f}")
        
        return gas_estimate
    
    def should_throttle(self, metrics: ResourceMetrics, threshold: float = 90.0) -> bool:
        """
        Determine if we should throttle new requests based on resource usage.
        
        Args:
            metrics: Current resource metrics
            threshold: Usage percentage threshold for throttling
            
        Returns:
            True if throttling should be applied
        """
        if metrics.cpu_percent > threshold:
            logger.warning(f"CPU usage {metrics.cpu_percent}% exceeds threshold {threshold}% - throttling recommended")
            return True
        
        if metrics.memory_used_mb > (psutil.virtual_memory().total / (1024 * 1024)) * (threshold / 100):
            logger.warning(f"Memory usage exceeds threshold {threshold}% - throttling recommended")
            return True
        
        return False
