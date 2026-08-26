from __future__ import annotations

import asyncio
import time
from unittest.mock import MagicMock, patch

import pytest

from gpu_worker.resource_monitor import ResourceMonitor


class MockRedisClient:
    """Mock Redis client for testing."""
    
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


@pytest.fixture
def mock_redis():
    """Fixture providing a mock Redis client."""
    return MockRedisClient()


@pytest.fixture
def redis_config():
    """Fixture providing Redis configuration for testing."""
    return {
        "host": "localhost",
        "port": 6379,
        "db": 0,
        "queue_key": "test_queue"
    }


@pytest.fixture
def gpu_stats_provider():
    """Mock GPU stats provider for testing."""
    def provider():
        return {
            "available": True,
            "devices": [
                {
                    "device_id": 0,
                    "name": "NVIDIA GeForce RTX 4090",
                    "utilization_pct": 25.0,
                    "memory_used_mb": 2048.0,
                    "memory_total_mb": 24576.0,
                    "memory_free_mb": 22528.0,
                    "memory_utilization_pct": 8.33,
                    "temperature_c": 65.0,
                    "available_for_worker": True
                },
                {
                    "device_id": 1,
                    "name": "NVIDIA GeForce RTX 4090",
                    "utilization_pct": 85.0,
                    "memory_used_mb": 22000.0,
                    "memory_total_mb": 24576.0,
                    "memory_free_mb": 2576.0,
                    "memory_utilization_pct": 89.52,
                    "temperature_c": 82.0,
                    "available_for_worker": False  # Over threshold
                }
            ]
        }
    return provider


@pytest.fixture
def cpu_stats_provider():
    """Mock CPU stats provider for testing."""
    def provider():
        return {
            "cpu_utilization_pct": 45.2,
            "memory_used_mb": 8192.0,
            "memory_total_mb": 32768.0,
            "memory_utilization_pct": 25.0
        }
    return provider


class TestResourceMonitorEnhanced:
    """Test enhanced resource monitor functionality."""
    
    def test_initialization_with_redis(self, redis_config):
        """Test resource monitor initialization with Redis configuration."""
        with patch('gpu_worker.resource_monitor.redis') as mock_redis_module:
            mock_redis_module.Redis.return_value = MockRedisClient()
            
            monitor = ResourceMonitor(
                poll_interval_seconds=1.0,
                redis_config=redis_config,
                gpu_memory_threshold_percent=85.0
            )
            
            assert monitor.gpu_memory_threshold_percent == 85.0
            assert monitor._redis_client is not None
            assert monitor._queue_key == "test_queue"
            
    def test_initialization_without_redis(self):
        """Test resource monitor initialization without Redis."""
        monitor = ResourceMonitor(poll_interval_seconds=1.0)
        
        assert monitor._redis_client is None
        
    def test_queue_stats_collection(self, redis_config):
        """Test Redis queue statistics collection."""
        mock_redis_client = MockRedisClient()
        
        with patch('gpu_worker.resource_monitor.redis') as mock_redis_module:
            mock_redis_module.Redis.return_value = mock_redis_client
            
            monitor = ResourceMonitor(redis_config=redis_config)
            
            # Test empty queue
            stats = monitor._collect_queue_stats()
            assert stats["available"] is True
            assert stats["queue_length"] == 0
            assert stats["estimated_wait_time_ms"] == 0.0
            assert stats["oldest_item_age_seconds"] == 0
            
            # Add items to queue
            mock_redis_client.lpush("test_queue", "task1", "task2", "task3", "task4", "task5")
            
            stats = monitor._collect_queue_stats()
            assert stats["available"] is True
            assert stats["queue_length"] == 5
            assert stats["estimated_wait_time_ms"] == 500.0  # 5 * 100ms
            assert stats["oldest_item_age_seconds"] >= 0
            
    def test_queue_stats_redis_error(self, redis_config):
        """Test queue stats collection when Redis connection fails."""
        mock_redis_client = MagicMock()
        mock_redis_client.llen.side_effect = Exception("Connection failed")
        
        with patch('gpu_worker.resource_monitor.redis') as mock_redis_module:
            mock_redis_module.Redis.return_value = mock_redis_client
            
            monitor = ResourceMonitor(redis_config=redis_config)
            
            stats = monitor._collect_queue_stats()
            assert stats["available"] is False
            assert stats["queue_length"] == 0
            assert stats["error"] is not None
            
    def test_queue_stats_no_redis_client(self):
        """Test queue stats collection when Redis client is not configured."""
        monitor = ResourceMonitor()
        
        stats = monitor._collect_queue_stats()
        assert stats["available"] is False
        assert stats["queue_length"] == 0
        assert stats["error"] == "Redis client not configured"
        
    def test_gpu_memory_threshold_checking(self, gpu_stats_provider):
        """Test GPU memory threshold checking."""
        monitor = ResourceMonitor(
            gpu_stats_provider=gpu_stats_provider,
            gpu_memory_threshold_percent=90.0
        )
        
        result = monitor.check_gpu_memory_threshold()
        
        assert result["threshold_exceeded"] is False
        assert len(result["devices_over_threshold"]) == 0
        assert result["total_devices"] == 2
        
        # Test with lower threshold
        monitor.gpu_memory_threshold_percent = 50.0
        result = monitor.check_gpu_memory_threshold()
        
        assert result["threshold_exceeded"] is True
        assert len(result["devices_over_threshold"]) == 1
        assert result["devices_over_threshold"][0]["device_id"] == 1
        assert result["devices_over_threshold"][0]["memory_percent"] == 89.52
        
    def test_gpu_memory_threshold_specific_device(self, gpu_stats_provider):
        """Test GPU memory threshold checking for specific device."""
        monitor = ResourceMonitor(
            gpu_stats_provider=gpu_stats_provider,
            gpu_memory_threshold_percent=50.0
        )
        
        # Check device 0 (should be under threshold)
        result = monitor.check_gpu_memory_threshold(device_id=0)
        assert result["threshold_exceeded"] is False
        
        # Check device 1 (should be over threshold)
        result = monitor.check_gpu_memory_threshold(device_id=1)
        assert result["threshold_exceeded"] is True
        assert len(result["devices_over_threshold"]) == 1
        assert result["devices_over_threshold"][0]["device_id"] == 1
        
    def test_available_gpu_memory_calculation(self, gpu_stats_provider):
        """Test available GPU memory calculation."""
        monitor = ResourceMonitor(
            gpu_stats_provider=gpu_stats_provider,
            gpu_memory_threshold_percent=90.0
        )
        
        available_memory = monitor.get_available_gpu_memory()
        
        # Check device 0
        device_0 = available_memory[0]
        assert device_0["available_mb"] == 22528.0
        assert abs(device_0["available_percent"] - 91.67) < 0.1
        assert device_0["used_mb"] == 2048.0
        assert device_0["total_mb"] == 24576.0
        assert device_0["can_allocate_worker"] is True
        
        # Check device 1
        device_1 = available_memory[1]
        assert device_1["available_mb"] == 2576.0
        assert abs(device_1["available_percent"] - 10.48) < 0.1
        assert device_1["used_mb"] == 22000.0
        assert device_1["total_mb"] == 24576.0
        assert device_1["can_allocate_worker"] is False
        
    def test_combined_metrics(self, gpu_stats_provider, cpu_stats_provider, redis_config):
        """Test combined GPU, CPU, and queue metrics."""
        mock_redis_client = MockRedisClient()
        mock_redis_client.lpush("test_queue", "task1", "task2")
        
        with patch('gpu_worker.resource_monitor.redis') as mock_redis_module:
            mock_redis_module.Redis.return_value = mock_redis_client
            
            monitor = ResourceMonitor(
                gpu_stats_provider=gpu_stats_provider,
                cpu_stats_provider=cpu_stats_provider,
                redis_config=redis_config
            )
            
            metrics = monitor.get_combined_metrics()
            
            assert "gpu" in metrics
            assert "cpu" in metrics
            assert "queue" in metrics
            assert "timestamp" in metrics
            
            # Check GPU metrics
            assert metrics["gpu"]["available"] is True
            assert len(metrics["gpu"]["devices"]) == 2
            
            # Check CPU metrics
            assert metrics["cpu"]["cpu_utilization_pct"] == 45.2
            assert metrics["cpu"]["memory_used_mb"] == 8192.0
            
            # Check queue metrics
            assert metrics["queue"]["queue_length"] == 2
            assert metrics["queue"]["estimated_wait_time_ms"] == 200.0
            
            # Check timestamp is recent
            assert time.time() - metrics["timestamp"] < 1.0
            
    @pytest.mark.asyncio
    async def test_enhanced_monitoring_loop(self, gpu_stats_provider, cpu_stats_provider, redis_config):
        """Test enhanced monitoring loop with queue statistics."""
        mock_redis_client = MockRedisClient()
        
        with patch('gpu_worker.resource_monitor.redis') as mock_redis_module:
            mock_redis_module.Redis.return_value = mock_redis_client
            
            monitor = ResourceMonitor(
                poll_interval_seconds=0.1,  # Fast polling for testing
                gpu_stats_provider=gpu_stats_provider,
                cpu_stats_provider=cpu_stats_provider,
                redis_config=redis_config
            )
            
            await monitor.start()
            
            # Let it run for a few cycles
            await asyncio.sleep(0.25)
            
            # Check that stats are being collected
            gpu_stats = monitor.get_gpu_stats()
            cpu_stats = monitor.get_cpu_stats()
            queue_stats = monitor.get_queue_stats()
            
            assert gpu_stats["available"] is True
            assert cpu_stats["cpu_utilization_pct"] == 45.2
            assert queue_stats["available"] is True
            
            await monitor.stop()
            
    def test_enhanced_gpu_stats_with_memory_details(self):
        """Test enhanced GPU statistics with detailed memory information."""
        # Mock pynvml for testing
        with patch('gpu_worker.resource_monitor.pynvml') as mock_pynvml:
            mock_pynvml.nvmlInit.return_value = None
            mock_pynvml.nvmlDeviceGetCount.return_value = 2
            
            # Mock device handles
            mock_handle_0 = MagicMock()
            mock_handle_1 = MagicMock()
            mock_pynvml.nvmlDeviceGetHandleByIndex.side_effect = [mock_handle_0, mock_handle_1]
            
            # Mock utilization rates
            mock_util_0 = MagicMock()
            mock_util_0.gpu = 30.0
            mock_util_0.memory = 15.0
            
            mock_util_1 = MagicMock()
            mock_util_1.gpu = 80.0
            mock_util_1.memory = 85.0
            
            mock_pynvml.nvmlDeviceGetUtilizationRates.side_effect = [mock_util_0, mock_util_1]
            
            # Mock memory info
            mock_mem_0 = MagicMock()
            mock_mem_0.used = 2 * 1024 * 1024 * 1024  # 2GB
            mock_mem_0.total = 24 * 1024 * 1024 * 1024  # 24GB
            mock_mem_0.free = 22 * 1024 * 1024 * 1024  # 22GB
            
            mock_mem_1 = MagicMock()
            mock_mem_1.used = 20 * 1024 * 1024 * 1024  # 20GB
            mock_mem_1.total = 24 * 1024 * 1024 * 1024  # 24GB
            mock_mem_1.free = 4 * 1024 * 1024 * 1024   # 4GB
            
            mock_pynvml.nvmlDeviceGetMemoryInfo.side_effect = [mock_mem_0, mock_mem_1]
            
            # Mock temperature
            mock_pynvml.nvmlDeviceGetTemperature.side_effect = [65.0, 82.0]
            
            monitor = ResourceMonitor(gpu_memory_threshold_percent=90.0)
            stats = monitor._collect_gpu_stats()
            
            assert stats["available"] is True
            assert len(stats["devices"]) == 2
            
            # Check device 0
            device_0 = stats["devices"][0]
            assert device_0["device_id"] == 0
            assert device_0["utilization_pct"] == 30.0
            assert device_0["memory_used_mb"] == 2048.0
            assert device_0["memory_total_mb"] == 24576.0
            assert device_0["memory_free_mb"] == 22528.0
            assert abs(device_0["memory_utilization_pct"] - 8.33) < 0.1
            assert device_0["temperature_c"] == 65.0
            assert device_0["available_for_worker"] is True
            
            # Check device 1
            device_1 = stats["devices"][1]
            assert device_1["device_id"] == 1
            assert device_1["utilization_pct"] == 80.0
            assert device_1["memory_used_mb"] == 20480.0
            assert device_1["memory_total_mb"] == 24576.0
            assert device_1["memory_free_mb"] == 4096.0
            assert abs(device_1["memory_utilization_pct"] - 83.33) < 0.1
            assert device_1["temperature_c"] == 82.0
            assert device_1["available_for_worker"] is True  # Under 90% threshold
            
    def test_nvidia_smi_fallback_enhanced(self):
        """Test enhanced nvidia-smi fallback with memory details."""
        with patch('gpu_worker.resource_monitor.pynvml', None):
            with patch('gpu_worker.resource_monitor.shutil.which', return_value='/usr/bin/nvidia-smi'):
                with patch('gpu_worker.resource_monitor.subprocess.run') as mock_run:
                    # Mock nvidia-smi output
                    mock_result = MagicMock()
                    mock_result.stdout = "0, 25, 2048, 24576, 65\n1, 85, 20480, 24576, 82\n"
                    mock_run.return_value = mock_result
                    
                    monitor = ResourceMonitor(gpu_memory_threshold_percent=90.0)
                    stats = monitor._collect_gpu_stats()
                    
                    assert stats["available"] is True
                    assert len(stats["devices"]) == 2
                    
                    # Check enhanced fields are present
                    device_0 = stats["devices"][0]
                    assert "memory_free_mb" in device_0
                    assert "memory_utilization_pct" in device_0
                    assert "available_for_worker" in device_0
                    
                    assert device_0["memory_free_mb"] == 22528.0  # 24576 - 2048
                    assert abs(device_0["memory_utilization_pct"] - 8.33) < 0.1
                    assert device_0["available_for_worker"] is True
                    
                    device_1 = stats["devices"][1]
                    assert device_1["memory_free_mb"] == 4096.0  # 24576 - 20480
                    assert abs(device_1["memory_utilization_pct"] - 83.33) < 0.1
                    assert device_1["available_for_worker"] is True