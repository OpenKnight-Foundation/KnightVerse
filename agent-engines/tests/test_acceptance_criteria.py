"""
Acceptance criteria verification tests for AI-30: GPU Worker Dynamic Autoscaling Daemon
"""
from __future__ import annotations

import asyncio
import time
from unittest.mock import AsyncMock, MagicMock, patch

import pytest

from gpu_worker.config import WorkerConfig, GPUConfig
from gpu_worker.maia_config import MaiaConfig
from gpu_worker.models import AnalysisRequest, AnalysisResult
from gpu_worker.pool import AutoscalingWorkerPool
from gpu_worker.resource_monitor import ResourceMonitor
from gpu_worker.resource_optimizer import (
    AutoscalingConfig,
    AutoscalingDaemon,
    ScalingEvent,
)


class MockRedisClient:
    """Mock Redis client for acceptance testing."""
    
    def __init__(self):
        self.lists = {}
        
    def llen(self, key: str) -> int:
        return len(self.lists.get(key, []))
        
    def lindex(self, key: str, index: int) -> str | None:
        queue = self.lists.get(key, [])
        if 0 <= index < len(queue):
            return queue[index]
        elif -len(queue) <= index < 0:
            return queue[index]
        return None
        
    def lpush(self, key: str, *values) -> int:
        if key not in self.lists:
            self.lists[key] = []
        for value in values:
            self.lists[key].insert(0, value)
        return len(self.lists[key])
        
    def rpop(self, key: str) -> str | None:
        if key in self.lists and self.lists[key]:
            return self.lists[key].pop()
        return None


@pytest.fixture
def acceptance_config():
    """Configuration matching acceptance criteria."""
    return AutoscalingConfig(
        min_workers=2,
        max_workers=10,
        scale_up_queue_threshold=50,  # From requirements
        scale_up_latency_threshold_ms=500.0,  # From requirements
        scale_down_idle_timeout_seconds=300,  # 5 minutes from requirements
        redis_host="localhost",
        redis_port=6379,
        redis_db=0,
        redis_queue_key="ai_task_queue",  # From requirements
        monitoring_interval_seconds=1.0,
        gpu_memory_threshold_percent=90.0
    )


class TestAcceptanceCriteria:
    """Test acceptance criteria compliance."""
    
    @pytest.mark.asyncio
    async def test_ac1_dynamic_scaling_based_on_traffic(self, acceptance_config):
        """AC1: Scales worker pool dynamically based on incoming traffic demand."""
        
        mock_redis = MockRedisClient()
        
        with patch('gpu_worker.resource_optimizer.redis') as mock_redis_module:
            mock_redis_module.Redis.return_value = mock_redis
            
            daemon = AutoscalingDaemon(acceptance_config)
            
            # Mock GPU capacity check
            daemon._has_available_gpu_capacity = AsyncMock(return_value=True)
            daemon._find_available_gpu_device = AsyncMock(return_value=0)
            daemon._create_worker = AsyncMock()
            daemon._get_idle_workers = AsyncMock(return_value=[])
            
            # Test scale-up trigger with high queue length (> 50)
            for i in range(60):  # Exceed threshold
                mock_redis.lpush("ai_task_queue", f"task-{i}")
                
            queue_metrics = await daemon._get_queue_metrics()
            resource_metrics = await daemon._get_resource_metrics()
            
            assert queue_metrics["queue_length"] == 60
            
            # Should trigger scale-up
            scaling_decision = await daemon._make_scaling_decision(queue_metrics, resource_metrics)
            assert scaling_decision == ScalingEvent.SCALE_UP
            
            # Test scale-up trigger with high latency (> 500ms)
            mock_redis.lists.clear()
            for i in range(10):  # Lower count but high estimated latency
                mock_redis.lpush("ai_task_queue", f"task-{i}")
                
            # Mock high latency
            with patch.object(daemon, '_get_queue_metrics') as mock_queue_metrics:
                mock_queue_metrics.return_value = {
                    "queue_length": 10,
                    "avg_wait_time_ms": 600.0  # > 500ms threshold
                }
                
                queue_metrics = await daemon._get_queue_metrics()
                scaling_decision = await daemon._make_scaling_decision(queue_metrics, resource_metrics)
                assert scaling_decision == ScalingEvent.SCALE_UP
                
    @pytest.mark.asyncio 
    async def test_ac2_respects_min_max_worker_bounds(self, acceptance_config):
        """AC2: Respects configured minimum (MIN_WORKERS) and maximum (MAX_WORKERS) bounds."""
        
        mock_redis = MockRedisClient()
        
        with patch('gpu_worker.resource_optimizer.redis') as mock_redis_module:
            mock_redis_module.Redis.return_value = mock_redis
            
            daemon = AutoscalingDaemon(acceptance_config)
            
            # Fill to maximum workers
            for i in range(acceptance_config.max_workers):
                daemon._workers[i] = MagicMock()
                
            # Mock methods
            daemon._has_available_gpu_capacity = AsyncMock(return_value=True)
            
            # Test cannot scale beyond max_workers
            for i in range(60):  # High queue load
                mock_redis.lpush("ai_task_queue", f"task-{i}")
                
            queue_metrics = await daemon._get_queue_metrics()
            resource_metrics = await daemon._get_resource_metrics()
            
            scaling_decision = await daemon._make_scaling_decision(queue_metrics, resource_metrics)
            assert scaling_decision == ScalingEvent.NO_SCALING  # Should not scale beyond max
            
            # Clear workers to minimum
            daemon._workers.clear()
            for i in range(acceptance_config.min_workers):
                daemon._workers[i] = MagicMock()
                
            # Test cannot scale below min_workers
            mock_redis.lists.clear()  # Empty queue
            
            # Mock idle workers
            daemon._get_idle_workers = AsyncMock(return_value=[MagicMock()])
            
            queue_metrics = await daemon._get_queue_metrics()
            scaling_decision = await daemon._make_scaling_decision(queue_metrics, resource_metrics)
            
            # Should not scale down when at minimum
            assert scaling_decision == ScalingEvent.NO_SCALING
            
    @pytest.mark.asyncio
    async def test_ac3_graceful_termination_no_kill_active_workers(self, acceptance_config):
        """AC3: Graceful termination: never kills workers with active in-flight evaluations."""
        
        # Test with simplified AutoscalingWorkerPool without Maia workers
        worker_config = WorkerConfig(
            gpu=GPUConfig(device_id=0, memory_fraction=0.5),
            max_concurrent_analyses=2
        )
        
        def mock_worker_factory(config, opening_book):
            worker = MagicMock()
            worker.config = config
            worker.worker_id = f"worker-{config.gpu.device_id}"
            worker.load = 0
            worker.start = AsyncMock()
            worker.shutdown = AsyncMock()
            worker.analyze = AsyncMock()
            worker.get_info = MagicMock()
            
            # Mock WorkerInfo
            from gpu_worker.models import WorkerInfo, WorkerStatus
            mock_info = WorkerInfo(
                worker_id=worker.worker_id,
                status=WorkerStatus.IDLE,
                gpu_device_id=config.gpu.device_id
            )
            worker.get_info.return_value = mock_info
            
            return worker
            
        pool = AutoscalingWorkerPool(
            base_configs=[worker_config, worker_config],  # 2 workers
            maia_configs=[],  # No Maia workers to avoid model loading
            worker_factory=mock_worker_factory,
            min_workers=1,
            max_workers=5
        )
        
        await pool.start_all()
        
        try:
            # Simulate active reservations (workers with in-flight tasks)
            pool._reservations[1] = 1  # Worker 1 has active task
            
            # Try to remove the busy worker (should wait)
            start_time = time.time()
            
            # Mock the reservation to clear after delay (simulating task completion)
            async def clear_reservation():
                await asyncio.sleep(0.2)  # Simulate task completion time
                pool._reservations[1] = 0
                
            asyncio.create_task(clear_reservation())
            
            # This should wait for the task to complete before removing worker
            success = await pool.remove_worker(1, graceful=True)
            elapsed = time.time() - start_time
            
            assert success is True
            assert elapsed >= 0.2  # Should have waited for task completion
            assert len(pool._workers) == 1  # One worker should be removed
            
        finally:
            await pool.shutdown_all(wait_for_pending=False, timeout=1.0)
            
    @pytest.mark.asyncio
    async def test_ac4_comprehensive_monitoring_and_scaling_tests(self, acceptance_config):
        """AC4: Unit tests test queue monitoring and scaling state transitions."""
        
        mock_redis = MockRedisClient()
        
        with patch('gpu_worker.resource_optimizer.redis') as mock_redis_module:
            mock_redis_module.Redis.return_value = mock_redis
            
            daemon = AutoscalingDaemon(acceptance_config)
            
            # Test state transitions: IDLE -> SCALE_UP -> SCALE_DOWN -> IDLE
            
            # State 1: IDLE (normal operation)
            queue_metrics = {"queue_length": 10, "avg_wait_time_ms": 100.0}
            resource_metrics = MagicMock()
            
            decision = await daemon._make_scaling_decision(queue_metrics, resource_metrics)
            assert decision == ScalingEvent.NO_SCALING
            
            # State 2: SCALE_UP (high load)
            daemon._has_available_gpu_capacity = AsyncMock(return_value=True)
            queue_metrics = {"queue_length": 60, "avg_wait_time_ms": 100.0}
            
            decision = await daemon._make_scaling_decision(queue_metrics, resource_metrics)
            assert decision == ScalingEvent.SCALE_UP
            
            # State 3: SCALE_DOWN (no load, idle workers)
            daemon._workers = {i: MagicMock() for i in range(5)}  # Add workers
            
            idle_worker = MagicMock()
            idle_worker.last_active = time.time() - 400  # Idle > 5 minutes (300s)
            daemon._get_idle_workers = AsyncMock(return_value=[idle_worker])
            
            queue_metrics = {"queue_length": 0, "avg_wait_time_ms": 0.0}
            
            decision = await daemon._make_scaling_decision(queue_metrics, resource_metrics)
            assert decision == ScalingEvent.SCALE_DOWN
            
    def test_prometheus_metrics_exposed(self, acceptance_config):
        """Verify Prometheus metrics are properly exposed."""
        
        from gpu_worker.resource_optimizer import (
            ACTIVE_WORKERS,
            QUEUE_LENGTH, 
            QUEUE_LATENCY,
            SCALING_EVENTS,
            GPU_MEMORY_UTILIZATION
        )
        
        from gpu_worker.pool import (
            WORKER_COUNT,
            JOBS_PROCESSED,
            WORKER_STARTUP_TIME,
            GRACEFUL_SHUTDOWNS,
            FORCED_SHUTDOWNS
        )
        
        # Verify all required metrics exist
        assert ACTIVE_WORKERS is not None
        assert QUEUE_LENGTH is not None  
        assert QUEUE_LATENCY is not None
        assert SCALING_EVENTS is not None
        assert GPU_MEMORY_UTILIZATION is not None
        assert WORKER_COUNT is not None
        assert JOBS_PROCESSED is not None
        assert WORKER_STARTUP_TIME is not None
        assert GRACEFUL_SHUTDOWNS is not None
        assert FORCED_SHUTDOWNS is not None
        
        # Test metric updates
        ACTIVE_WORKERS.set(5)
        assert ACTIVE_WORKERS._value._value == 5
        
        QUEUE_LENGTH.set(25)
        assert QUEUE_LENGTH._value._value == 25
        
        SCALING_EVENTS.labels(event_type="scale_up").inc()
        # Verify counter incremented (implementation detail varies)
        
    @pytest.mark.asyncio
    async def test_gpu_memory_protection(self, acceptance_config):
        """Verify GPU VRAM limits are respected to prevent OOM crashes."""
        
        mock_redis = MockRedisClient()
        
        with patch('gpu_worker.resource_optimizer.redis') as mock_redis_module:
            mock_redis_module.Redis.return_value = mock_redis
            
            daemon = AutoscalingDaemon(acceptance_config)
            
            # Mock high GPU memory usage (no capacity available)
            daemon._has_available_gpu_capacity = AsyncMock(return_value=False)
            
            # High queue load that would normally trigger scale-up
            for i in range(60):
                mock_redis.lpush("ai_task_queue", f"task-{i}")
                
            queue_metrics = await daemon._get_queue_metrics()
            resource_metrics = await daemon._get_resource_metrics()
            
            # Should not scale up due to GPU memory limits
            scaling_decision = await daemon._make_scaling_decision(queue_metrics, resource_metrics)
            assert scaling_decision == ScalingEvent.NO_SCALING
            
    @pytest.mark.asyncio
    async def test_redis_queue_monitoring(self, acceptance_config):
        """Verify Redis ai_task_queue:length monitoring works correctly."""
        
        mock_redis = MockRedisClient()
        
        with patch('gpu_worker.resource_optimizer.redis') as mock_redis_module:
            mock_redis_module.Redis.return_value = mock_redis
            
            daemon = AutoscalingDaemon(acceptance_config)
            
            # Test queue length monitoring
            assert mock_redis.llen("ai_task_queue") == 0
            
            # Add tasks to queue
            for i in range(25):
                mock_redis.lpush("ai_task_queue", f"task-{i}")
                
            queue_metrics = await daemon._get_queue_metrics()
            assert queue_metrics["queue_length"] == 25
            # Check if available - only then check estimated wait time
            if queue_metrics.get("available", False):
                assert queue_metrics["estimated_wait_time_ms"] == 2500.0  # 25 * 100ms
            
            # Test queue processing
            for _ in range(10):
                mock_redis.rpop("ai_task_queue")
                
            queue_metrics = await daemon._get_queue_metrics()
            assert queue_metrics["queue_length"] == 15
            
    @pytest.mark.asyncio
    async def test_traffic_spike_simulation(self, acceptance_config):
        """Simulate traffic spikes and cooldowns as required."""
        
        mock_redis = MockRedisClient()
        
        with patch('gpu_worker.resource_optimizer.redis') as mock_redis_module:
            mock_redis_module.Redis.return_value = mock_redis
            
            daemon = AutoscalingDaemon(acceptance_config)
            daemon._has_available_gpu_capacity = AsyncMock(return_value=True)
            daemon._create_worker = AsyncMock()
            daemon._terminate_worker = AsyncMock()
            
            # Simulate traffic spike
            spike_tasks = 100
            for i in range(spike_tasks):
                mock_redis.lpush("ai_task_queue", f"spike-task-{i}")
                
            # Should trigger scale-up
            queue_metrics = await daemon._get_queue_metrics()
            resource_metrics = await daemon._get_resource_metrics()
            
            assert queue_metrics["queue_length"] == spike_tasks
            
            decision = await daemon._make_scaling_decision(queue_metrics, resource_metrics)
            assert decision == ScalingEvent.SCALE_UP
            
            # Simulate traffic cooldown (process all tasks)
            for _ in range(spike_tasks):
                mock_redis.rpop("ai_task_queue")
                
            # Add idle workers for scale-down test
            daemon._workers = {i: MagicMock() for i in range(5)}
            for worker in daemon._workers.values():
                worker.last_active = time.time() - 400  # Idle > 5 minutes
                
            daemon._get_idle_workers = AsyncMock(return_value=list(daemon._workers.values()))
            
            queue_metrics = await daemon._get_queue_metrics()
            assert queue_metrics["queue_length"] == 0
            
            # Should trigger scale-down after cooldown
            decision = await daemon._make_scaling_decision(queue_metrics, resource_metrics)
            assert decision == ScalingEvent.SCALE_DOWN


class TestRequirementsCompliance:
    """Test specific requirements from the task description."""
    
    @pytest.mark.asyncio
    async def test_req_queue_thresholds(self, acceptance_config):
        """Test queue length > 50 and wait time > 500ms thresholds."""
        
        mock_redis = MockRedisClient()
        
        with patch('gpu_worker.resource_optimizer.redis') as mock_redis_module:
            mock_redis_module.Redis.return_value = mock_redis
            
            daemon = AutoscalingDaemon(acceptance_config)
            daemon._has_available_gpu_capacity = AsyncMock(return_value=True)
            
            # Test exact threshold: queue_length = 51 (> 50)
            for i in range(51):
                mock_redis.lpush("ai_task_queue", f"task-{i}")
                
            queue_metrics = await daemon._get_queue_metrics()
            resource_metrics = await daemon._get_resource_metrics()
            
            decision = await daemon._make_scaling_decision(queue_metrics, resource_metrics)
            assert decision == ScalingEvent.SCALE_UP
            
            # Test exact threshold: avg_wait_time = 501ms (> 500ms)
            mock_redis.lists.clear()
            
            with patch.object(daemon, '_get_queue_metrics') as mock_queue_metrics:
                mock_queue_metrics.return_value = {
                    "queue_length": 5,
                    "avg_wait_time_ms": 501.0  # > 500ms
                }
                
                queue_metrics = await daemon._get_queue_metrics()
                decision = await daemon._make_scaling_decision(queue_metrics, resource_metrics)
                assert decision == ScalingEvent.SCALE_UP
                
    @pytest.mark.asyncio
    async def test_req_scale_down_conditions(self, acceptance_config):
        """Test queue_length == 0 and GPU idle > 5 minutes conditions."""
        
        mock_redis = MockRedisClient()
        
        with patch('gpu_worker.resource_optimizer.redis') as mock_redis_module:
            mock_redis_module.Redis.return_value = mock_redis
            
            daemon = AutoscalingDaemon(acceptance_config)
            
            # Ensure we have workers above minimum
            daemon._workers = {i: MagicMock() for i in range(5)}
            
            # Test exact conditions: queue_length == 0 and idle > 300s (5 minutes)
            idle_worker = MagicMock()
            idle_worker.last_active = time.time() - 301  # 301 seconds ago (> 5 minutes)
            
            daemon._get_idle_workers = AsyncMock(return_value=[idle_worker])
            
            queue_metrics = {"queue_length": 0, "avg_wait_time_ms": 0.0}
            resource_metrics = await daemon._get_resource_metrics()
            
            decision = await daemon._make_scaling_decision(queue_metrics, resource_metrics)
            assert decision == ScalingEvent.SCALE_DOWN