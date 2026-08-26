from __future__ import annotations

import asyncio
import signal
import time
from unittest.mock import AsyncMock, MagicMock, patch

import pytest

from gpu_worker.config import WorkerConfig, GPUConfig
from gpu_worker.maia_config import MaiaConfig
from gpu_worker.models import AnalysisRequest, WorkerStatus
from gpu_worker.pool import AutoscalingWorkerPool, WorkerPool


@pytest.fixture
def worker_config():
    """Fixture providing worker configuration for testing."""
    return WorkerConfig(
        gpu=GPUConfig(device_id=0, memory_fraction=0.5),
        max_concurrent_analyses=2,
        engine_config={"depth": 15}
    )


@pytest.fixture
def maia_config():
    """Fixture providing Maia configuration for testing."""
    return MaiaConfig(
        path="/path/to/maia/model",
        elo=1500
    )


@pytest.fixture
def mock_worker_factory():
    """Mock worker factory for testing."""
    def factory(config, opening_book):
        worker = MagicMock()
        worker.config = config
        worker.worker_id = f"worker-{config.gpu.device_id}"
        worker.load = 0
        worker.start = AsyncMock()
        worker.shutdown = AsyncMock()
        worker.analyze = AsyncMock()
        worker.get_info = MagicMock()
        
        # Mock WorkerInfo return
        mock_info = MagicMock()
        mock_info.worker_id = worker.worker_id
        mock_info.status = WorkerStatus.IDLE
        mock_info.gpu_device_id = config.gpu.device_id
        worker.get_info.return_value = mock_info
        
        return worker
    return factory


@pytest.fixture
def mock_maia_factory():
    """Mock Maia worker factory for testing."""
    def factory(config, maia_config):
        worker = MagicMock()
        worker.config = MagicMock()
        worker.config.max_concurrent_analyses = config.max_concurrent_analyses
        worker.config.maia_models = [MagicMock()]
        worker.config.maia_models[0].elo = maia_config.elo
        worker.worker_id = f"maia-worker-{maia_config.elo}"
        worker.load = 0
        worker.start = AsyncMock()
        worker.shutdown = AsyncMock()
        worker.analyze = AsyncMock()
        worker.get_info = MagicMock()
        
        # Mock WorkerInfo return
        mock_info = MagicMock()
        mock_info.worker_id = worker.worker_id
        mock_info.status = WorkerStatus.IDLE
        worker.get_info.return_value = mock_info
        
        return worker
    return factory


class TestAutoscalingWorkerPool:
    """Test autoscaling worker pool functionality."""
    
    @pytest.mark.asyncio
    async def test_pool_initialization(self, worker_config, maia_config, mock_worker_factory, mock_maia_factory):
        """Test pool initialization with autoscaling enabled."""
        pool = AutoscalingWorkerPool(
            base_configs=[worker_config],
            maia_configs=[maia_config],
            worker_factory=mock_worker_factory,
            maia_worker_factory=mock_maia_factory,
            enable_autoscaling=True,
            min_workers=1,
            max_workers=5
        )
        
        assert pool.enable_autoscaling is True
        assert pool.min_workers == 1
        assert pool.max_workers == 5
        assert len(pool._workers) == 1
        assert len(pool._maia_workers) == 1
        assert not pool._started
        assert not pool._shutdown_requested
        
    @pytest.mark.asyncio
    async def test_pool_start_stop_lifecycle(self, worker_config, maia_config, mock_worker_factory, mock_maia_factory):
        """Test pool start and stop lifecycle with signal handlers."""
        pool = AutoscalingWorkerPool(
            base_configs=[worker_config],
            maia_configs=[maia_config],
            worker_factory=mock_worker_factory,
            maia_worker_factory=mock_maia_factory
        )
        
        await pool.start_all()
        
        assert pool._started is True
        # Check that workers were started
        for worker in pool._workers:
            worker.start.assert_called_once()
        for worker in pool._maia_workers:
            worker.start.assert_called_once()
            
        await pool.shutdown_all(wait_for_pending=False, timeout=1.0)
        
        assert pool._started is False
        assert pool._shutdown_requested is True
        
    @pytest.mark.asyncio
    async def test_add_worker_success(self, worker_config, maia_config, mock_worker_factory, mock_maia_factory):
        """Test successful worker addition."""
        pool = AutoscalingWorkerPool(
            base_configs=[worker_config],
            maia_configs=[maia_config],
            worker_factory=mock_worker_factory,
            maia_worker_factory=mock_maia_factory,
            max_workers=5
        )
        
        await pool.start_all()
        
        initial_count = len(pool._workers)
        success = await pool.add_worker(worker_config, gpu_device_id=1)
        
        assert success is True
        assert len(pool._workers) == initial_count + 1
        assert len(pool._reservations) == len(pool._workers)
        
    @pytest.mark.asyncio
    async def test_add_worker_at_max_capacity(self, worker_config, maia_config, mock_worker_factory, mock_maia_factory):
        """Test worker addition when at maximum capacity."""
        pool = AutoscalingWorkerPool(
            base_configs=[worker_config],
            maia_configs=[maia_config],
            worker_factory=mock_worker_factory,
            maia_worker_factory=mock_maia_factory,
            max_workers=1  # Set to current worker count
        )
        
        await pool.start_all()
        
        success = await pool.add_worker(worker_config, gpu_device_id=1)
        
        assert success is False
        assert len(pool._workers) == 1  # Should remain unchanged
        
    @pytest.mark.asyncio
    async def test_remove_worker_success(self, worker_config, maia_config, mock_worker_factory, mock_maia_factory):
        """Test successful worker removal."""
        # Create pool with multiple workers
        configs = [
            WorkerConfig(gpu=GPUConfig(device_id=i), max_concurrent_analyses=2)
            for i in range(3)
        ]
        
        pool = AutoscalingWorkerPool(
            base_configs=configs,
            maia_configs=[maia_config],
            worker_factory=mock_worker_factory,
            maia_worker_factory=mock_maia_factory,
            min_workers=1
        )
        
        await pool.start_all()
        
        initial_count = len(pool._workers)
        success = await pool.remove_worker(worker_index=1, graceful=True)
        
        assert success is True
        assert len(pool._workers) == initial_count - 1
        assert len(pool._reservations) == len(pool._workers)
        
    @pytest.mark.asyncio
    async def test_remove_worker_at_min_capacity(self, worker_config, maia_config, mock_worker_factory, mock_maia_factory):
        """Test worker removal when at minimum capacity."""
        pool = AutoscalingWorkerPool(
            base_configs=[worker_config],
            maia_configs=[maia_config],
            worker_factory=mock_worker_factory,
            maia_worker_factory=mock_maia_factory,
            min_workers=1  # Set to current worker count
        )
        
        await pool.start_all()
        
        success = await pool.remove_worker(worker_index=0, graceful=True)
        
        assert success is False
        assert len(pool._workers) == 1  # Should remain unchanged
        
    @pytest.mark.asyncio
    async def test_remove_worker_with_active_tasks(self, worker_config, maia_config, mock_worker_factory, mock_maia_factory):
        """Test worker removal when worker has active tasks."""
        # Create pool with multiple workers
        configs = [
            WorkerConfig(gpu=GPUConfig(device_id=i), max_concurrent_analyses=2)
            for i in range(3)
        ]
        
        pool = AutoscalingWorkerPool(
            base_configs=configs,
            maia_configs=[maia_config],
            worker_factory=mock_worker_factory,
            maia_worker_factory=mock_maia_factory,
            min_workers=1
        )
        
        await pool.start_all()
        
        # Simulate active task on worker 1
        pool._reservations[1] = 1
        
        # Mock the reservation to clear after a short delay
        async def clear_reservation():
            await asyncio.sleep(0.1)
            pool._reservations[1] = 0
            
        asyncio.create_task(clear_reservation())
        
        start_time = time.time()
        success = await pool.remove_worker(worker_index=1, graceful=True)
        elapsed = time.time() - start_time
        
        assert success is True
        assert elapsed >= 0.1  # Should have waited for task completion
        
    @pytest.mark.asyncio
    async def test_submit_analysis_request(self, worker_config, maia_config, mock_worker_factory, mock_maia_factory):
        """Test submitting analysis requests."""
        pool = AutoscalingWorkerPool(
            base_configs=[worker_config],
            maia_configs=[maia_config],
            worker_factory=mock_worker_factory,
            maia_worker_factory=mock_maia_factory
        )
        
        await pool.start_all()
        
        # Mock analysis result
        from gpu_worker.models import AnalysisResult
        mock_result = AnalysisResult(request_id="test-123", best_move="e4")
        pool._workers[0].analyze.return_value = mock_result
        
        request = AnalysisRequest(
            id="test-123",
            fen="rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            depth=15
        )
        
        result = await pool.submit(request)
        
        assert result == mock_result
        pool._workers[0].analyze.assert_called_once_with(request)
        
    @pytest.mark.asyncio
    async def test_submit_maia_request(self, worker_config, maia_config, mock_worker_factory, mock_maia_factory):
        """Test submitting Maia analysis requests."""
        pool = AutoscalingWorkerPool(
            base_configs=[worker_config],
            maia_configs=[maia_config],
            worker_factory=mock_worker_factory,
            maia_worker_factory=mock_maia_factory
        )
        
        await pool.start_all()
        
        # Mock analysis result
        from gpu_worker.models import AnalysisResult
        mock_result = AnalysisResult(request_id="maia-test-123", best_move="d4")
        pool._maia_workers[0].analyze.return_value = mock_result
        
        request = AnalysisRequest(
            id="maia-test-123",
            fen="rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            actor_id="maia-1500"  # This should route to Maia worker
        )
        
        result = await pool.submit(request)
        
        assert result == mock_result
        pool._maia_workers[0].analyze.assert_called_once_with(request)
        
    @pytest.mark.asyncio
    async def test_submit_during_shutdown(self, worker_config, maia_config, mock_worker_factory, mock_maia_factory):
        """Test that requests are rejected during shutdown."""
        pool = AutoscalingWorkerPool(
            base_configs=[worker_config],
            maia_configs=[maia_config],
            worker_factory=mock_worker_factory,
            maia_worker_factory=mock_maia_factory
        )
        
        await pool.start_all()
        
        # Mark pool as shutting down
        pool._shutdown_requested = True
        
        request = AnalysisRequest(
            id="test-shutdown",
            fen="rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
        )
        
        with pytest.raises(RuntimeError, match="worker pool is shutting down"):
            await pool.submit(request)
            
    @pytest.mark.asyncio
    async def test_wait_for_pending_tasks(self, worker_config, maia_config, mock_worker_factory, mock_maia_factory):
        """Test waiting for pending tasks to complete."""
        pool = AutoscalingWorkerPool(
            base_configs=[worker_config],
            maia_configs=[maia_config],
            worker_factory=mock_worker_factory,
            maia_worker_factory=mock_maia_factory
        )
        
        await pool.start_all()
        
        # Simulate pending tasks
        pool._reservations[0] = 2
        
        # Clear reservations after delay
        async def clear_reservations():
            await asyncio.sleep(0.1)
            pool._reservations[0] = 0
            
        asyncio.create_task(clear_reservations())
        
        start_time = time.time()
        await pool.wait_for_pending_tasks(timeout=1.0)
        elapsed = time.time() - start_time
        
        assert elapsed >= 0.1
        assert pool._reservations[0] == 0
        
    @pytest.mark.asyncio
    async def test_wait_for_pending_tasks_timeout(self, worker_config, maia_config, mock_worker_factory, mock_maia_factory):
        """Test timeout when waiting for pending tasks."""
        pool = AutoscalingWorkerPool(
            base_configs=[worker_config],
            maia_configs=[maia_config],
            worker_factory=mock_worker_factory,
            maia_worker_factory=mock_maia_factory
        )
        
        await pool.start_all()
        
        # Simulate pending tasks that won't clear
        pool._reservations[0] = 2
        
        with pytest.raises(asyncio.TimeoutError):
            await pool.wait_for_pending_tasks(timeout=0.1)
            
    def test_scaling_capability_checks(self, worker_config, maia_config, mock_worker_factory, mock_maia_factory):
        """Test scaling capability checks."""
        pool = AutoscalingWorkerPool(
            base_configs=[worker_config],
            maia_configs=[maia_config],
            worker_factory=mock_worker_factory,
            maia_worker_factory=mock_maia_factory,
            enable_autoscaling=True,
            min_workers=1,
            max_workers=5
        )
        
        # Should be able to scale up (1 < 5)
        assert pool.can_scale_up() is True
        
        # Should not be able to scale down (1 == 1)
        assert pool.can_scale_down() is False
        
        # Add more workers to test scale down
        for i in range(3):
            pool._workers.append(MagicMock())
            pool._reservations.append(0)
            
        # Now should be able to scale down (4 > 1)
        assert pool.can_scale_down() is True
        
        # Fill to max workers
        pool._workers.append(MagicMock())
        pool._reservations.append(0)
        
        # Should not be able to scale up (5 == 5)
        assert pool.can_scale_up() is False
        
    def test_scaling_disabled(self, worker_config, maia_config, mock_worker_factory, mock_maia_factory):
        """Test scaling when autoscaling is disabled."""
        pool = AutoscalingWorkerPool(
            base_configs=[worker_config],
            maia_configs=[maia_config],
            worker_factory=mock_worker_factory,
            maia_worker_factory=mock_maia_factory,
            enable_autoscaling=False,
            min_workers=1,
            max_workers=5
        )
        
        # Should not be able to scale when disabled
        assert pool.can_scale_up() is False
        assert pool.can_scale_down() is False
        
    def test_idle_worker_detection(self, worker_config, maia_config, mock_worker_factory, mock_maia_factory):
        """Test detection of idle workers."""
        # Create pool with multiple workers
        configs = [
            WorkerConfig(gpu=GPUConfig(device_id=i), max_concurrent_analyses=2)
            for i in range(3)
        ]
        
        pool = AutoscalingWorkerPool(
            base_configs=configs,
            maia_configs=[maia_config],
            worker_factory=mock_worker_factory,
            maia_worker_factory=mock_maia_factory
        )
        
        # Set up worker loads and reservations
        pool._workers[0].load = 0  # Idle
        pool._reservations[0] = 0
        
        pool._workers[1].load = 1  # Busy
        pool._reservations[1] = 0
        
        pool._workers[2].load = 0  # Idle
        pool._reservations[2] = 1  # But has reservations
        
        idle_workers = pool.get_idle_workers()
        
        # Only worker 0 should be considered idle
        assert idle_workers == [0]
        
    def test_detailed_status_reporting(self, worker_config, maia_config, mock_worker_factory, mock_maia_factory):
        """Test detailed pool status reporting."""
        pool = AutoscalingWorkerPool(
            base_configs=[worker_config],
            maia_configs=[maia_config],
            worker_factory=mock_worker_factory,
            maia_worker_factory=mock_maia_factory,
            enable_autoscaling=True,
            min_workers=1,
            max_workers=5
        )
        
        status = pool.get_detailed_status()
        
        assert status["started"] is False
        assert status["shutdown_requested"] is False
        assert status["worker_count"] == 1
        assert status["maia_worker_count"] == 1
        assert status["min_workers"] == 1
        assert status["max_workers"] == 5
        assert status["pending_tasks"] == 0
        assert status["pending_maia_tasks"] == 0
        assert status["autoscaling_enabled"] is True
        assert len(status["workers"]) == 1
        
        # Check worker details
        worker_info = status["workers"][0]
        assert "worker_id" in worker_info
        assert "load" in worker_info
        assert "reservations" in worker_info
        assert "status" in worker_info
        
    @pytest.mark.asyncio
    async def test_signal_handler_setup(self, worker_config, maia_config, mock_worker_factory, mock_maia_factory):
        """Test signal handler setup and cleanup."""
        pool = AutoscalingWorkerPool(
            base_configs=[worker_config],
            maia_configs=[maia_config],
            worker_factory=mock_worker_factory,
            maia_worker_factory=mock_maia_factory
        )
        
        with patch('signal.signal') as mock_signal:
            await pool.start_all()
            
            # Should have set up SIGTERM and SIGINT handlers
            expected_calls = [
                ((signal.SIGTERM, pool._signal_handler),),
                ((signal.SIGINT, pool._signal_handler),)
            ]
            
            for expected_call in expected_calls:
                assert expected_call in mock_signal.call_args_list
                
    @pytest.mark.asyncio
    async def test_acquire_worker_during_shutdown(self, worker_config, maia_config, mock_worker_factory, mock_maia_factory):
        """Test worker acquisition fails during shutdown."""
        pool = AutoscalingWorkerPool(
            base_configs=[worker_config],
            maia_configs=[maia_config],
            worker_factory=mock_worker_factory,
            maia_worker_factory=mock_maia_factory
        )
        
        await pool.start_all()
        pool._shutdown_requested = True
        
        with pytest.raises(RuntimeError, match="Pool is shutting down"):
            await pool._acquire_worker()


class TestLegacyWorkerPool:
    """Test legacy WorkerPool class for backward compatibility."""
    
    @pytest.mark.asyncio
    async def test_legacy_pool_compatibility(self, worker_config, maia_config, mock_worker_factory, mock_maia_factory):
        """Test that legacy WorkerPool still works with autoscaling disabled."""
        pool = WorkerPool(
            configs=[worker_config],
            maia_configs=[maia_config],
            worker_factory=mock_worker_factory,
            maia_worker_factory=mock_maia_factory
        )
        
        # Should be an instance of AutoscalingWorkerPool but with autoscaling disabled
        assert isinstance(pool, AutoscalingWorkerPool)
        assert pool.enable_autoscaling is False
        assert pool.min_workers == 1
        assert pool.max_workers == 1
        
        await pool.start_all()
        
        # Should not be able to scale
        assert pool.can_scale_up() is False
        assert pool.can_scale_down() is False
        
        await pool.shutdown_all(wait_for_pending=False)