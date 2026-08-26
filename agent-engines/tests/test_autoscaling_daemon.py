from __future__ import annotations

import asyncio
import time
from unittest.mock import AsyncMock, MagicMock, patch

import pytest

from gpu_worker.resource_optimizer import (
    AutoscalingConfig,
    AutoscalingDaemon,
    ScalingEvent,
    WorkerProcess,
)


class MockRedisClient:
    """Mock Redis client for testing."""
    
    def __init__(self):
        self.data = {}
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
def mock_redis():
    """Fixture providing a mock Redis client."""
    return MockRedisClient()


@pytest.fixture
def autoscaling_config():
    """Fixture providing autoscaling configuration for testing."""
    return AutoscalingConfig(
        min_workers=2,
        max_workers=6,
        scale_up_queue_threshold=3,
        scale_up_latency_threshold_ms=200.0,
        scale_down_idle_timeout_seconds=5,  # Short timeout for testing
        redis_host="localhost",
        redis_port=6379,
        redis_db=0,
        redis_queue_key="test_queue",
        monitoring_interval_seconds=0.1,  # Fast monitoring for testing
        gpu_memory_threshold_percent=80.0
    )


@pytest.fixture
def mock_worker_factory():
    """Fixture providing a mock worker factory."""
    def factory(gpu_device_id: int):
        return MagicMock()
    return factory


class TestAutoscalingConfig:
    """Test autoscaling configuration."""
    
    def test_default_config(self):
        """Test default configuration values."""
        config = AutoscalingConfig()
        assert config.min_workers == 2
        assert config.max_workers == 10
        assert config.scale_up_queue_threshold == 50
        assert config.scale_up_latency_threshold_ms == 500.0
        assert config.scale_down_idle_timeout_seconds == 300
        assert config.redis_host == "localhost"
        assert config.redis_port == 6379
        assert config.redis_db == 0
        assert config.redis_queue_key == "ai_task_queue"
        assert config.monitoring_interval_seconds == 10.0
        assert config.gpu_memory_threshold_percent == 90.0


class TestAutoscalingDaemon:
    """Test autoscaling daemon functionality."""
    
    @pytest.mark.asyncio
    async def test_daemon_initialization(self, autoscaling_config, mock_worker_factory):
        """Test daemon initialization."""
        with patch('gpu_worker.resource_optimizer.redis') as mock_redis_module:
            mock_redis_module.Redis.return_value = MockRedisClient()
            
            daemon = AutoscalingDaemon(autoscaling_config, mock_worker_factory)
            
            assert daemon.config == autoscaling_config
            assert daemon.worker_factory == mock_worker_factory
            assert not daemon._running
            assert len(daemon._workers) == 0
            
    @pytest.mark.asyncio
    async def test_daemon_start_stop(self, autoscaling_config, mock_worker_factory):
        """Test daemon start and stop lifecycle."""
        with patch('gpu_worker.resource_optimizer.redis') as mock_redis_module:
            mock_redis_module.Redis.return_value = MockRedisClient()
            
            daemon = AutoscalingDaemon(autoscaling_config, mock_worker_factory)
            
            # Mock the scale_to_target method to avoid actual worker creation
            daemon._scale_to_target = AsyncMock()
            
            await daemon.start()
            assert daemon._running
            assert daemon._monitoring_task is not None
            
            await daemon.stop(timeout=1.0)
            assert not daemon._running
            
    @pytest.mark.asyncio
    async def test_queue_metrics_collection(self, autoscaling_config, mock_worker_factory):
        """Test Redis queue metrics collection."""
        mock_redis_client = MockRedisClient()
        
        with patch('gpu_worker.resource_optimizer.redis') as mock_redis_module:
            mock_redis_module.Redis.return_value = mock_redis_client
            
            daemon = AutoscalingDaemon(autoscaling_config, mock_worker_factory)
            
            # Test empty queue
            metrics = await daemon._get_queue_metrics()
            assert metrics["queue_length"] == 0
            assert metrics["avg_wait_time_ms"] == 0.0
            
            # Add items to queue
            mock_redis_client.lpush("test_queue", "task1", "task2", "task3")
            
            metrics = await daemon._get_queue_metrics()
            assert metrics["queue_length"] == 3
            assert metrics["avg_wait_time_ms"] > 0
            
    @pytest.mark.asyncio
    async def test_scaling_decisions(self, autoscaling_config, mock_worker_factory):
        """Test scaling decision logic."""
        with patch('gpu_worker.resource_optimizer.redis') as mock_redis_module:
            mock_redis_module.Redis.return_value = MockRedisClient()
            
            daemon = AutoscalingDaemon(autoscaling_config, mock_worker_factory)
            
            # Mock methods to control behavior
            daemon._has_available_gpu_capacity = AsyncMock(return_value=True)
            daemon._get_idle_workers = AsyncMock(return_value=[])
            
            # Test scale-up decision (high queue length)
            queue_metrics = {"queue_length": 5, "avg_wait_time_ms": 100.0}
            resource_metrics = MagicMock()
            
            decision = await daemon._make_scaling_decision(queue_metrics, resource_metrics)
            assert decision == ScalingEvent.SCALE_UP
            
            # Test scale-up decision (high latency)
            queue_metrics = {"queue_length": 2, "avg_wait_time_ms": 300.0}
            decision = await daemon._make_scaling_decision(queue_metrics, resource_metrics)
            assert decision == ScalingEvent.SCALE_UP
            
            # Test no scaling needed
            queue_metrics = {"queue_length": 1, "avg_wait_time_ms": 50.0}
            decision = await daemon._make_scaling_decision(queue_metrics, resource_metrics)
            assert decision == ScalingEvent.NO_SCALING
            
    @pytest.mark.asyncio
    async def test_scale_up_at_max_workers(self, autoscaling_config, mock_worker_factory):
        """Test that scaling up is prevented when at maximum workers."""
        with patch('gpu_worker.resource_optimizer.redis') as mock_redis_module:
            mock_redis_module.Redis.return_value = MockRedisClient()
            
            daemon = AutoscalingDaemon(autoscaling_config, mock_worker_factory)
            
            # Fill workers to maximum
            for i in range(autoscaling_config.max_workers):
                daemon._workers[i] = WorkerProcess(
                    process_id=i,
                    gpu_device_id=i % 2,
                    started_at=time.time(),
                    last_active=time.time()
                )
            
            daemon._has_available_gpu_capacity = AsyncMock(return_value=True)
            
            # Test scale-up decision with max workers
            queue_metrics = {"queue_length": 10, "avg_wait_time_ms": 600.0}
            resource_metrics = MagicMock()
            
            decision = await daemon._make_scaling_decision(queue_metrics, resource_metrics)
            assert decision == ScalingEvent.NO_SCALING
            
    @pytest.mark.asyncio
    async def test_scale_down_decision(self, autoscaling_config, mock_worker_factory):
        """Test scale-down decision logic."""
        with patch('gpu_worker.resource_optimizer.redis') as mock_redis_module:
            mock_redis_module.Redis.return_value = MockRedisClient()
            
            daemon = AutoscalingDaemon(autoscaling_config, mock_worker_factory)
            
            # Create idle worker that has been idle long enough
            idle_worker = WorkerProcess(
                process_id=1,
                gpu_device_id=0,
                started_at=time.time() - 100,
                last_active=time.time() - 10,  # Idle for 10 seconds
                is_busy=False
            )
            
            # Add more than minimum workers
            for i in range(autoscaling_config.min_workers + 1):
                daemon._workers[i] = WorkerProcess(
                    process_id=i,
                    gpu_device_id=i % 2,
                    started_at=time.time(),
                    last_active=time.time()
                )
            
            daemon._get_idle_workers = AsyncMock(return_value=[idle_worker])
            
            # Test scale-down with empty queue and idle workers
            queue_metrics = {"queue_length": 0, "avg_wait_time_ms": 0.0}
            resource_metrics = MagicMock()
            
            decision = await daemon._make_scaling_decision(queue_metrics, resource_metrics)
            assert decision == ScalingEvent.SCALE_DOWN
            
    @pytest.mark.asyncio
    async def test_scale_down_at_min_workers(self, autoscaling_config, mock_worker_factory):
        """Test that scaling down is prevented when at minimum workers."""
        with patch('gpu_worker.resource_optimizer.redis') as mock_redis_module:
            mock_redis_module.Redis.return_value = MockRedisClient()
            
            daemon = AutoscalingDaemon(autoscaling_config, mock_worker_factory)
            
            # Set exactly minimum workers
            for i in range(autoscaling_config.min_workers):
                daemon._workers[i] = WorkerProcess(
                    process_id=i,
                    gpu_device_id=i % 2,
                    started_at=time.time(),
                    last_active=time.time() - 10
                )
            
            # Test scale-down decision with minimum workers
            queue_metrics = {"queue_length": 0, "avg_wait_time_ms": 0.0}
            resource_metrics = MagicMock()
            
            decision = await daemon._make_scaling_decision(queue_metrics, resource_metrics)
            assert decision == ScalingEvent.NO_SCALING
            
    @pytest.mark.asyncio
    async def test_worker_creation_and_termination(self, autoscaling_config, mock_worker_factory):
        """Test worker process creation and termination."""
        with patch('gpu_worker.resource_optimizer.redis') as mock_redis_module:
            mock_redis_module.Redis.return_value = MockRedisClient()
            
            daemon = AutoscalingDaemon(autoscaling_config, mock_worker_factory)
            
            # Test worker creation
            worker = await daemon._create_worker(0)
            assert worker is not None
            assert worker.gpu_device_id == 0
            assert worker.process_id is not None
            
            # Add worker to daemon
            daemon._workers[worker.process_id] = worker
            
            # Test worker termination
            await daemon._terminate_worker(worker.process_id, graceful=True)
            assert worker.process_id not in daemon._workers
            
    @pytest.mark.asyncio
    async def test_gpu_device_assignment(self, autoscaling_config, mock_worker_factory):
        """Test GPU device assignment logic."""
        with patch('gpu_worker.resource_optimizer.redis') as mock_redis_module:
            mock_redis_module.Redis.return_value = MockRedisClient()
            
            daemon = AutoscalingDaemon(autoscaling_config, mock_worker_factory)
            
            # Test finding available GPU device
            device_id = await daemon._find_available_gpu_device()
            assert device_id == 0  # Should start with device 0
            
            # Add worker on device 0
            daemon._workers[1] = WorkerProcess(
                process_id=1,
                gpu_device_id=0,
                started_at=time.time(),
                last_active=time.time()
            )
            
            # Should assign device 1 next
            device_id = await daemon._find_available_gpu_device()
            assert device_id == 1
            
    @pytest.mark.asyncio
    async def test_daemon_status_reporting(self, autoscaling_config, mock_worker_factory):
        """Test daemon status reporting."""
        with patch('gpu_worker.resource_optimizer.redis') as mock_redis_module:
            mock_redis_module.Redis.return_value = MockRedisClient()
            
            daemon = AutoscalingDaemon(autoscaling_config, mock_worker_factory)
            
            # Add some workers
            for i in range(3):
                daemon._workers[i] = WorkerProcess(
                    process_id=i,
                    gpu_device_id=i % 2,
                    started_at=time.time(),
                    last_active=time.time(),
                    is_busy=(i == 0)  # Make first worker busy
                )
            
            status = daemon.get_status()
            
            assert status["running"] == daemon._running
            assert status["worker_count"] == 3
            assert status["min_workers"] == autoscaling_config.min_workers
            assert status["max_workers"] == autoscaling_config.max_workers
            assert len(status["workers"]) == 3
            
            # Check worker details
            for worker_id, worker_info in status["workers"].items():
                assert "gpu_device_id" in worker_info
                assert "started_at" in worker_info
                assert "last_active" in worker_info
                assert "is_busy" in worker_info
                
    @pytest.mark.asyncio
    async def test_redis_connection_failure(self, autoscaling_config, mock_worker_factory):
        """Test handling of Redis connection failures."""
        with patch('gpu_worker.resource_optimizer.redis') as mock_redis_module:
            # Mock Redis to raise an exception
            mock_redis_client = MagicMock()
            mock_redis_client.llen.side_effect = Exception("Connection failed")
            mock_redis_module.Redis.return_value = mock_redis_client
            
            daemon = AutoscalingDaemon(autoscaling_config, mock_worker_factory)
            
            # Should handle Redis errors gracefully
            metrics = await daemon._get_queue_metrics()
            assert metrics["queue_length"] == 0
            assert metrics["error"] is not None
            
    @pytest.mark.asyncio
    async def test_monitoring_loop_interruption(self, autoscaling_config, mock_worker_factory):
        """Test monitoring loop interruption and cleanup."""
        with patch('gpu_worker.resource_optimizer.redis') as mock_redis_module:
            mock_redis_module.Redis.return_value = MockRedisClient()
            
            daemon = AutoscalingDaemon(autoscaling_config, mock_worker_factory)
            
            # Mock methods to avoid actual scaling
            daemon._scale_to_target = AsyncMock()
            daemon._get_queue_metrics = AsyncMock(return_value={"queue_length": 0, "avg_wait_time_ms": 0.0})
            daemon._get_resource_metrics = AsyncMock(return_value=MagicMock())
            daemon._make_scaling_decision = AsyncMock(return_value=ScalingEvent.NO_SCALING)
            daemon._execute_scaling_action = AsyncMock()
            daemon._cleanup_dead_workers = AsyncMock()
            daemon._update_prometheus_metrics = MagicMock()
            
            await daemon.start()
            
            # Let it run briefly
            await asyncio.sleep(0.2)
            
            # Stop daemon
            await daemon.stop(timeout=1.0)
            
            assert not daemon._running
            
    @pytest.mark.asyncio
    async def test_graceful_worker_termination(self, autoscaling_config, mock_worker_factory):
        """Test graceful termination of busy workers."""
        with patch('gpu_worker.resource_optimizer.redis') as mock_redis_module:
            mock_redis_module.Redis.return_value = MockRedisClient()
            
            daemon = AutoscalingDaemon(autoscaling_config, mock_worker_factory)
            
            # Create a busy worker
            busy_worker = WorkerProcess(
                process_id=1,
                gpu_device_id=0,
                started_at=time.time(),
                last_active=time.time(),
                is_busy=True,
                process_handle=MagicMock()
            )
            
            daemon._workers[1] = busy_worker
            
            # Mock the worker to become not busy after a delay
            async def make_not_busy():
                await asyncio.sleep(0.1)
                busy_worker.is_busy = False
                
            asyncio.create_task(make_not_busy())
            
            # Should wait for worker to become idle
            start_time = time.time()
            await daemon._terminate_worker(1, graceful=True)
            elapsed = time.time() - start_time
            
            # Should have waited at least a bit
            assert elapsed >= 0.1
            assert 1 not in daemon._workers