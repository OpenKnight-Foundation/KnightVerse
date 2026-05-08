from __future__ import annotations

import asyncio

import pytest

from gpu_worker.config import GPUConfig, WorkerConfig
from gpu_worker.models import AnalysisRequest, WorkerStatus
from gpu_worker.resource_monitor import ResourceMonitor
from gpu_worker.uci_bridge import UciBestMove, UciInfo
from gpu_worker.worker import GPUAnalysisWorker


class FakeBridge:
    def __init__(self, config: WorkerConfig) -> None:
        self.config = config
        self.started = False
        self.initialized = False
        self.positions: list[str] = []
        self.quit_called = False

    async def start(self) -> None:
        self.started = True

    async def initialize_options(self) -> None:
        self.initialized = True

    async def set_position(self, fen: str) -> None:
        self.positions.append(fen)

    async def go(self, **_: object) -> tuple[UciBestMove, UciInfo]:
        return UciBestMove(best_move="e2e4"), UciInfo(
            depth=20,
            evaluation=0.33,
            principal_variation=["e2e4", "e7e5"],
            nodes=2048,
        )

    async def quit(self) -> None:
        self.quit_called = True


@pytest.mark.asyncio
async def test_worker_lifecycle_and_analysis_result() -> None:
    config = WorkerConfig()
    monitor = ResourceMonitor(
        gpu_stats_provider=lambda: {
            "available": True,
            "devices": [{"device_id": 0, "utilization_pct": 76.0, "memory_used_mb": 512.0}],
        },
        cpu_stats_provider=lambda: {"cpu_utilization_pct": 10.0},
    )
    worker = GPUAnalysisWorker(
        config,
        worker_id="worker-1",
        bridge_factory=FakeBridge,
        resource_monitor=monitor,
    )

    await worker.start()
    result = await worker.analyze(
        AnalysisRequest(fen="8/8/8/8/8/8/8/K6k w - - 0 1", depth=12)
    )
    info = worker.get_info()
    await worker.shutdown()

    assert result.best_move == "e2e4"
    assert result.evaluation == pytest.approx(0.33)
    assert result.depth == 20
    assert result.nodes_searched == 2048
    assert result.gpu_utilization == 76.0
    assert result.principal_variation == ["e2e4", "e7e5"]
    assert info.worker_id == "worker-1"
    assert info.analyses_completed == 1
    assert info.status in {WorkerStatus.IDLE, WorkerStatus.SHUTTING_DOWN}


@pytest.mark.asyncio
async def test_worker_transitions_to_error_state_on_failure() -> None:
    class FailingBridge(FakeBridge):
        async def go(self, **_: object) -> tuple[UciBestMove, UciInfo]:
            raise RuntimeError("engine failure")

    worker = GPUAnalysisWorker(
        WorkerConfig(), bridge_factory=FailingBridge, resource_monitor=ResourceMonitor()
    )
    await worker.start()

    with pytest.raises(RuntimeError):
        await worker.analyze(AnalysisRequest(fen="8/8/8/8/8/8/8/K6k w - - 0 1"))

    assert worker.status == WorkerStatus.ERROR
    await worker.shutdown()


def test_worker_config_defaults_and_validation() -> None:
    config = WorkerConfig()

    assert config.engine_path == "/usr/local/bin/lc0"
    assert config.gpu.backend == "cudnn"
    assert config.max_concurrent_analyses == 8

    with pytest.raises(ValueError):
        WorkerConfig(engine_path="   ")

    custom = WorkerConfig(gpu=GPUConfig(device_id=2, max_batch_size=64))
    assert custom.gpu.device_id == 2
    assert custom.gpu.max_batch_size == 64


def test_analysis_request_validates_fen() -> None:
    request = AnalysisRequest(fen="8/8/8/8/8/8/8/K6k w - - 0 1")
    assert request.fen == "8/8/8/8/8/8/8/K6k w - - 0 1"

    with pytest.raises(ValueError):
        AnalysisRequest(fen="not-a-fen")
