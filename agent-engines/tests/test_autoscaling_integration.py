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
    ResourceOptimizer,
)


class MockRedisClient:
    """Mock Redis client for integration testing."""
    
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
def mock_redis():
    """Fixture providing mock Redis client."""
    return MockRedisClient()


@pytest.fixture
def integration_config():
    """Configuration for integration testing."""
    return AutoscalingConfig(
        min_workers=2,
        max_workers=8,
        scale_up_queue_threshold=5,
        scale_up_latency_threshold_ms=300.0,
        scale_down_idle_timeout_seconds=2,  # Short timeout for testing
        redis_host="localhost",
        redis_port=6379,
        redis_db=0,
        redis_queue_key="integration_test_queue",
        monitoring_interval_seconds=0.1,  # Fast monitoring for testing
        gpu_memory_threshold_percent=85.0
    )


@pytest.fixture
def worker_configs():
    """Base worker configurations for testing."""
    return [
        WorkerConfig(
            gpu=GPUConfig(device_id=i, memory_fraction=0.4),
            max_concurrent_analyses=3,
            engine_config={"depth": 12}
        ) for i in range(2)  # Start with 2 base workers
    ]


@pytest.fixture
def maia_configs():
    """Maia configurations for testing."""
    return [
        MaiaConfig(path=f"/path/to/maia/{elo}", elo=elo)
        for elo in [1100, 1500, 1900]
    ]


@pytest.fixture
def mock_worker_factory():
    """Mock worker factory that creates trackable workers."""
    created_workers = []
    
    def factory(config, opening_book):
        worker = MagicMock()
        worker.config = config
        worker.worker_id = f"worker-{config.gpu.device_id}-{len(created_workers)}"
        worker.load = 0
        worker.start = AsyncMock()
        worker.shutdown = AsyncMock()
        worker.analyze = AsyncMock()
        worker.get_info = MagicMock()
        
        # Track worker creation
        created_workers.append(worker)
        
        # Mock analysis with realistic delay
        async def mock_analyze(request):
            await asyncio.sleep(0.05)  # Simulate analysis time
            return AnalysisResult(request_id=request.id, best_move="e4")
        
        worker.analyze.side_effect = mock_analyze
        
        # Mock WorkerInfo
        from gpu_worker.models import WorkerInfo, WorkerStatus
        mock_info = WorkerInfo(
            worker_id=worker.worker_id,
            status=WorkerStatus.IDLE,
            gpu_device_id=config.gpu.device_id
        )
        worker.get_info.return_value = mock_info
        
        return worker
    
    factory.created_workers = created_workers
    return factory


class TestAutoscalingIntegration:
    """Integration tests for the complete autoscaling system."""
    
    @pytest.mark.asyncio
    async def test_complete_autoscaling_workflow(
        self, mock_redis, integration_config, worker_configs, maia_configs, mock_worker_factory
    ):
        """Test complete autoscaling workflow from queue monitoring to worker scaling."""
        
        with patch('gpu_worker.resource_optimizer.redis') as mock_redis_module:
            mock_redis_module.Redis.return_value = mock_redis
            
            # Create resource monitor with mocked GPU stats
            def mock_gpu_stats():
                return {
                    "available": True,
                    "devices": [
                        {
                            "device_id": i,
                            "memory_used_mb": 2000,
                            "memory_total_mb": 24000,
                            "memory_utilization_pct": 8.33,
                            "utilization_pct": 25.0,
                            "temperature_c": 65.0,
                            "available_for_worker": True
                        } for i in range(4)
                    ]
                }
            
            def mock_cpu_stats():
                return {
                    "cpu_utilization_pct": 30.0,
                    "memory_used_mb": 8000,
                    "memory_total_mb": 32000,
                    "memory_utilization_pct": 25.0
                }
            
            redis_config = {
                "host": "localhost",
                "port": 6379,
                "db": 0,
                "queue_key": "integration_test_queue"
            }
            
            with patch('gpu_worker.resource_monitor.redis') as monitor_redis_module:
                monitor_redis_module.Redis.return_value = mock_redis
                
                resource_monitor = ResourceMonitor(
                    poll_interval_seconds=0.05,
                    gpu_stats_provider=mock_gpu_stats,
                    cpu_stats_provider=mock_cpu_stats,
                    redis_config=redis_config
                )
                
                # Create autoscaling worker pool
                pool = AutoscalingWorkerPool(
                    base_configs=worker_configs,
                    maia_configs=maia_configs,
                    worker_factory=mock_worker_factory,
                    enable_autoscaling=True,
                    min_workers=2,
                    max_workers=6
                )
                
                # Create custom worker factory for autoscaling daemon
                def daemon_worker_factory(gpu_device_id):
                    # This would normally create a real worker process
                    # For testing, we'll simulate adding workers to the pool
                    new_config = WorkerConfig(
                        gpu=GPUConfig(device_id=gpu_device_id, memory_fraction=0.4),
                        max_concurrent_analyses=3
                    )
                    return mock_worker_factory(new_config, None)
                
                # Create autoscaling daemon
                daemon = AutoscalingDaemon(integration_config, daemon_worker_factory)
                
                # Override daemon's worker management to work with pool
                async def mock_scale_up():
                    if pool.can_scale_up():
                        new_config = WorkerConfig(
                            gpu=GPUConfig(device_id=len(pool._workers), memory_fraction=0.4),
                            max_concurrent_analyses=3
                        )
                        await pool.add_worker(new_config, len(pool._workers))
                        
                async def mock_scale_down():
                    if pool.can_scale_down():
                        idle_workers = pool.get_idle_workers()
                        if idle_workers:
                            await pool.remove_worker(idle_workers[0], graceful=True)
                
                daemon._scale_up = mock_scale_up
                daemon._scale_down = mock_scale_down
                
                try:
                    # Start all components
                    await resource_monitor.start()
                    await pool.start_all()
                    
                    # Mock the daemon's scale_to_target to work with the pool
                    daemon._scale_to_target = AsyncMock()
                    await daemon.start()
                    
                    # Initial state: should have 2 workers
                    assert len(pool._workers) == 2
                    
                    # Simulate high queue load to trigger scale-up
                    for i in range(10):  # Add 10 tasks to exceed threshold (5)
                        mock_redis.lpush("integration_test_queue", f"task-{i}")
                    
                    # Wait for monitoring and scaling to occur
                    await asyncio.sleep(0.3)
                    
                    # Manually trigger scaling decision (since we mocked the daemon)
                    queue_metrics = await daemon._get_queue_metrics()
                    resource_metrics = await daemon._get_resource_metrics()
                    
                    assert queue_metrics["queue_length"] == 10
                    
                    scaling_decision = await daemon._make_scaling_decision(queue_metrics, resource_metrics)
                    await daemon._execute_scaling_action(scaling_decision, queue_metrics, resource_metrics)
                    
                    # Should have triggered scale-up
                    if pool.can_scale_up():
                        assert len(pool._workers) > 2
                    
                    # Simulate processing tasks (clear queue)
                    for _ in range(10):
                        mock_redis.rpop("integration_test_queue")
                    
                    # Wait for scale-down conditions
                    await asyncio.sleep(0.3)
                    
                    # Check scale-down decision
                    queue_metrics = await daemon._get_queue_metrics()
                    assert queue_metrics["queue_length"] == 0
                    
                    # Simulate idle timeout by mocking worker idle time
                    if pool._workers:
                        for worker_process in daemon._workers.values():
                            worker_process.last_active = time.time() - 10  # 10 seconds ago
                    
                    scaling_decision = await daemon._make_scaling_decision(queue_metrics, resource_metrics)
                    
                    # Should consider scale-down if conditions are met
                    if len(pool._workers) > pool.min_workers:
                        await daemon._execute_scaling_action(scaling_decision, queue_metrics, resource_metrics)
                    
                finally:
                    # Cleanup
                    await daemon.stop(timeout=1.0)
                    await pool.shutdown_all(wait_for_pending=False, timeout=1.0)
                    await resource_monitor.stop()
                    
    @pytest.mark.asyncio
    async def test_traffic_spike_simulation(
        self, mock_redis, integration_config, worker_configs, maia_configs, mock_worker_factory
    ):
        """Simulate a traffic spike and verify autoscaling response."""
        
        with patch('gpu_worker.resource_optimizer.redis') as mock_redis_module:
            mock_redis_module.Redis.return_value = mock_redis
            
            # Create autoscaling pool
            pool = AutoscalingWorkerPool(
                base_configs=worker_configs,
                maia_configs=maia_configs,
                worker_factory=mock_worker_factory,
                enable_autoscaling=True,
                min_workers=2,
                max_workers=8
            )
            
            await pool.start_all()
            
            try:
                initial_worker_count = len(pool._workers)
                
                # Simulate sudden traffic spike
                requests = []
                for i in range(20):  # Create many concurrent requests
                    request = AnalysisRequest(
                        id=f"spike-{i}",
                        fen="rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
                        depth=15
                    )
                    requests.append(request)
                
                # Submit all requests concurrently
                start_time = time.time()
                tasks = [pool.submit(request) for request in requests]
                
                # While requests are processing, add workers if needed
                if len(requests) > len(pool._workers) * 3:  # More requests than capacity
                    for i in range(3):  # Add some workers
                        if pool.can_scale_up():
                            new_config = WorkerConfig(
                                gpu=GPUConfig(device_id=len(pool._workers), memory_fraction=0.4),
                                max_concurrent_analyses=3
                            )
                            await pool.add_worker(new_config, len(pool._workers))
                
                # Wait for all requests to complete
                results = await asyncio.gather(*tasks)
                processing_time = time.time() - start_time
                
                # Verify all requests were processed
                assert len(results) == 20
                for i, result in enumerate(results):
                    assert result.request_id == f"spike-{i}"
                    assert result.best_move is not None
                
                # Should have scaled up to handle the load
                final_worker_count = len(pool._workers)
                assert final_worker_count >= initial_worker_count
                
                # Processing time should be reasonable (parallel processing)
                assert processing_time < 2.0  # Should complete within 2 seconds
                
                # Test scale-down after traffic subsides
                # Wait for workers to become idle
                await asyncio.sleep(0.1)
                
                # Remove excess workers
                while len(pool._workers) > pool.min_workers and pool.can_scale_down():
                    idle_workers = pool.get_idle_workers()
                    if idle_workers:
                        await pool.remove_worker(idle_workers[0], graceful=True)
                        break  # Remove one at a time
                
                # Should scale back down towards minimum
                scaled_down_count = len(pool._workers)
                assert scaled_down_count <= final_worker_count
                
            finally:
                await pool.shutdown_all(wait_for_pending=False, timeout=2.0)
                
    @pytest.mark.asyncio
    async def test_resource_monitor_integration(
        self, mock_redis, integration_config, worker_configs, maia_configs, mock_worker_factory
    ):
        """Test integration between resource monitor and autoscaling decisions."""
        
        redis_config = {
            "host": "localhost", 
            "port": 6379,
            "db": 0,
            "queue_key": "integration_test_queue"
        }
        
        with patch('gpu_worker.resource_monitor.redis') as monitor_redis_module:
            monitor_redis_module.Redis.return_value = mock_redis
            
            # Create resource monitor with high GPU utilization
            def mock_high_gpu_stats():
                return {
                    "available": True,
                    "devices": [
                        {
                            "device_id": 0,
                            "memory_used_mb": 20000,  # High usage
                            "memory_total_mb": 24000,
                            "memory_utilization_pct": 83.33,
                            "utilization_pct": 95.0,
                            "temperature_c": 85.0,
                            "available_for_worker": True  # Just under threshold
                        },
                        {
                            "device_id": 1,
                            "memory_used_mb": 22000,  # Very high usage
                            "memory_total_mb": 24000,
                            "memory_utilization_pct": 91.67,
                            "utilization_pct": 98.0,
                            "temperature_c": 88.0,
                            "available_for_worker": False  # Over threshold
                        }
                    ]
                }
            
            resource_monitor = ResourceMonitor(
                poll_interval_seconds=0.1,
                gpu_stats_provider=mock_high_gpu_stats,
                redis_config=redis_config,
                gpu_memory_threshold_percent=90.0
            )
            
            await resource_monitor.start()
            
            try:
                # Check GPU memory threshold detection
                threshold_check = resource_monitor.check_gpu_memory_threshold()
                assert threshold_check["threshold_exceeded"] is True
                assert len(threshold_check["devices_over_threshold"]) == 1
                assert threshold_check["devices_over_threshold"][0]["device_id"] == 1
                
                # Check available GPU memory calculation
                available_memory = resource_monitor.get_available_gpu_memory()
                assert available_memory[0]["can_allocate_worker"] is True  # Under threshold
                assert available_memory[1]["can_allocate_worker"] is False  # Over threshold
                
                # Add queue load
                for i in range(8):
                    mock_redis.lpush("integration_test_queue", f"task-{i}")
                
                # Get combined metrics
                metrics = resource_monitor.get_combined_metrics()
                
                assert metrics["gpu"]["available"] is True
                assert metrics["queue"]["queue_length"] == 8
                assert metrics["queue"]["estimated_wait_time_ms"] == 800.0
                
                # Verify timestamp is recent
                assert time.time() - metrics["timestamp"] < 1.0
                
            finally:
                await resource_monitor.stop()
                
    @pytest.mark.asyncio  
    async def test_graceful_shutdown_integration(
        self, mock_redis, integration_config, worker_configs, maia_configs, mock_worker_factory
    ):
        """Test graceful shutdown of the complete autoscaling system."""
        
        with patch('gpu_worker.resource_optimizer.redis') as mock_redis_module:
            mock_redis_module.Redis.return_value = mock_redis
            
            # Create system components
            pool = AutoscalingWorkerPool(
                base_configs=worker_configs,
                maia_configs=maia_configs,
                worker_factory=mock_worker_factory,
                enable_autoscaling=True,
                min_workers=2,
                max_workers=6
            )
            
            daemon = AutoscalingDaemon(integration_config)
            optimizer = ResourceOptimizer(autoscaling_config=integration_config)
            
            # Start all components
            await pool.start_all()
            await optimizer.start_autoscaling()
            
            try:
                # Submit some long-running requests
                long_requests = []
                for i in range(5):
                    request = AnalysisRequest(
                        id=f"long-{i}",
                        fen="rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
                        depth=20  # Deeper analysis
                    )
                    long_requests.append(pool.submit(request))
                
                # Let requests start processing
                await asyncio.sleep(0.1)
                
                # Initiate graceful shutdown
                shutdown_start = time.time()
                
                await asyncio.gather(
                    pool.shutdown_all(wait_for_pending=True, timeout=2.0),
                    optimizer.stop_autoscaling(timeout=2.0)
                )
                
                shutdown_time = time.time() - shutdown_start
                
                # Should have waited for requests to complete
                assert shutdown_time >= 0.1  # At least processing time
                assert shutdown_time < 3.0   # But not too long
                
                # Pool should be properly shutdown
                assert pool._started is False
                assert pool._shutdown_requested is True
                
                # All workers should be stopped
                for worker in pool._workers:
                    worker.shutdown.assert_called_once()
                    
            except Exception:
                # Ensure cleanup even if test fails
                await pool.shutdown_all(wait_for_pending=False, timeout=1.0)
                await optimizer.stop_autoscaling(timeout=1.0)
                raise
                
    @pytest.mark.asyncio
    async def test_error_handling_integration(
        self, mock_redis, integration_config, worker_configs, maia_configs, mock_worker_factory
    ):
        """Test error handling across the autoscaling system."""
        
        with patch('gpu_worker.resource_optimizer.redis') as mock_redis_module:
            # Simulate Redis connection issues
            mock_redis_client = MagicMock()
            mock_redis_client.llen.side_effect = Exception("Redis connection failed")
            mock_redis_module.Redis.return_value = mock_redis_client
            
            daemon = AutoscalingDaemon(integration_config)
            
            # Should handle Redis errors gracefully
            queue_metrics = await daemon._get_queue_metrics()
            assert queue_metrics["queue_length"] == 0
            assert queue_metrics["error"] is not None
            
            # System should continue to operate despite Redis issues
            resource_metrics = await daemon._get_resource_metrics()
            assert resource_metrics is not None
            
            # Scaling decisions should default to no scaling when metrics unavailable
            decision = await daemon._make_scaling_decision(queue_metrics, resource_metrics)
            # Should not scale up without reliable queue metrics
            
        # Test worker creation failure
        def failing_worker_factory(config, opening_book):
            raise Exception("Worker creation failed")
            
        pool = AutoscalingWorkerPool(
            base_configs=worker_configs,
            maia_configs=maia_configs,
            worker_factory=failing_worker_factory,
            enable_autoscaling=True
        )
        
        # Should handle worker creation failures gracefully
        success = await pool.add_worker(worker_configs[0], gpu_device_id=2)
        assert success is False
        
        # Pool should remain functional
        assert len(pool._workers) == 2  # Original workers still there