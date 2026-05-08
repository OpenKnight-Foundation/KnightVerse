"""Tests for ai/ package — covers all four issues (#627, #642, #644, #645)."""

from __future__ import annotations

import asyncio
import pytest

# ---------------------------------------------------------------------------
# Issue #645 — Stockfish WASM
# ---------------------------------------------------------------------------
from ai.stockfish_wasm import StockfishWASMEngine, WASMEngineConfig, WASMEngineStatus


class TestWASMEngineConfig:
    def test_defaults(self):
        cfg = WASMEngineConfig()
        assert cfg.wasm_path == "stockfish-16.1.wasm"
        assert cfg.threads == 1
        assert cfg.skill_level == 20
        assert cfg.default_depth == 18

    def test_custom(self):
        cfg = WASMEngineConfig(threads=4, hash_size_mb=64, skill_level=15)
        assert cfg.threads == 4
        assert cfg.hash_size_mb == 64

    def test_to_dict(self):
        d = WASMEngineConfig(threads=2).to_dict()
        assert d["threads"] == 2
        assert "wasm_path" in d


@pytest.mark.asyncio
async def test_wasm_engine_lifecycle():
    engine = StockfishWASMEngine()
    assert engine.status == WASMEngineStatus.TERMINATED

    await engine.initialize()
    assert engine.status == WASMEngineStatus.READY

    result = await engine.analyze("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1")
    assert result.best_move
    assert result.depth == 18
    assert result.nodes_searched > 0

    await engine.shutdown()
    assert engine.status == WASMEngineStatus.TERMINATED


@pytest.mark.asyncio
async def test_wasm_engine_not_ready_raises():
    engine = StockfishWASMEngine()
    with pytest.raises(RuntimeError):
        await engine.analyze("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1")


@pytest.mark.asyncio
async def test_wasm_engine_custom_depth():
    engine = StockfishWASMEngine()
    await engine.initialize()
    result = await engine.analyze(
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        depth=10,
    )
    assert result.depth == 10
    await engine.shutdown()


def test_wasm_js_bridge_snippet():
    engine = StockfishWASMEngine(WASMEngineConfig(hash_size_mb=32, threads=2))
    snippet = engine.js_bridge_snippet()
    assert "Stockfish" in snippet
    assert "32" in snippet
    assert "2" in snippet


# ---------------------------------------------------------------------------
# Issue #642 — GPU worker (unit-tested with a mock engine subprocess)
# ---------------------------------------------------------------------------
from ai.worker import (
    AnalysisRequest,
    AnalysisResult,
    GPUAnalysisWorker,
    WorkerConfig,
    WorkerStatus,
)


class _FakeMonitor:
    def get_gpu_stats(self):
        return {"available": True, "devices": [{"device_id": 0, "utilization_pct": 55.0}]}


class _MockWorker(GPUAnalysisWorker):
    """Subclass that bypasses the real subprocess."""

    async def start(self) -> None:
        self._started_at = __import__("time").monotonic()
        self._status = WorkerStatus.IDLE

    async def analyze(self, request: AnalysisRequest) -> AnalysisResult:
        self._status = WorkerStatus.BUSY
        self._analyses_completed += 1
        self._status = WorkerStatus.IDLE
        return AnalysisResult(
            request_id=request.id,
            best_move="e2e4",
            evaluation=0.3,
            depth=request.depth,
            principal_variation=["e2e4", "e7e5"],
            time_ms=50,
            gpu_utilization=self._gpu_utilization(),
        )

    async def shutdown(self) -> None:
        self._status = WorkerStatus.SHUTTING_DOWN


@pytest.mark.asyncio
async def test_worker_lifecycle():
    worker = _MockWorker(WorkerConfig(), resource_monitor=_FakeMonitor())
    await worker.start()
    assert worker.status == WorkerStatus.IDLE
    assert worker.is_ready

    req = AnalysisRequest(fen="rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1")
    result = await worker.analyze(req)

    assert result.best_move == "e2e4"
    assert result.evaluation == pytest.approx(0.3)
    assert result.gpu_utilization == pytest.approx(55.0)
    assert worker.uptime_seconds() > 0

    await worker.shutdown()
    assert worker.status == WorkerStatus.SHUTTING_DOWN


@pytest.mark.asyncio
async def test_worker_error_state():
    class _FailWorker(_MockWorker):
        async def analyze(self, request):
            self._status = WorkerStatus.ERROR
            raise RuntimeError("engine crash")

    worker = _FailWorker(WorkerConfig())
    await worker.start()
    with pytest.raises(RuntimeError):
        await worker.analyze(AnalysisRequest(fen="rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"))
    assert worker.status == WorkerStatus.ERROR


def test_worker_config_defaults():
    cfg = WorkerConfig()
    assert cfg.engine_path == "stockfish"
    assert cfg.max_concurrent == 4
    assert cfg.hash_mb == 256


# ---------------------------------------------------------------------------
# Issue #644 — Resource monitor / dashboard
# ---------------------------------------------------------------------------
from ai.resource_monitor import DashboardSnapshot, ResourceMonitor


def _fake_gpu():
    return {
        "available": True,
        "devices": [{"device_id": 0, "utilization_pct": 72.0,
                     "memory_used_mb": 1024.0, "memory_total_mb": 8192.0,
                     "temperature_c": 65.0}],
    }


def _fake_cpu():
    return {"cpu_utilization_pct": 30.0, "memory_used_mb": 4096.0,
            "memory_total_mb": 16384.0, "memory_utilization_pct": 25.0}


def test_snapshot_structure():
    monitor = ResourceMonitor(gpu_stats_provider=_fake_gpu, cpu_stats_provider=_fake_cpu)
    monitor._refresh()
    snap = monitor.snapshot()

    assert snap.gpu_available is True
    assert len(snap.gpu_devices) == 1
    assert snap.gpu_devices[0].utilization_pct == pytest.approx(72.0)
    assert snap.cpu_utilization_pct == pytest.approx(30.0)
    assert snap.memory_total_mb == pytest.approx(16384.0)


def test_snapshot_to_dict():
    monitor = ResourceMonitor(gpu_stats_provider=_fake_gpu, cpu_stats_provider=_fake_cpu)
    monitor._refresh()
    d = monitor.snapshot().to_dict()
    assert d["gpu"]["available"] is True
    assert d["gpu"]["devices"][0]["utilization_pct"] == pytest.approx(72.0)
    assert d["cpu"]["utilization_pct"] == pytest.approx(30.0)


def test_get_gpu_stats_compat():
    monitor = ResourceMonitor(gpu_stats_provider=_fake_gpu, cpu_stats_provider=_fake_cpu)
    monitor._refresh()
    stats = monitor.get_gpu_stats()
    assert stats["available"] is True
    assert stats["devices"][0]["device_id"] == 0


@pytest.mark.asyncio
async def test_monitor_start_stop():
    monitor = ResourceMonitor(
        poll_interval_seconds=0.05,
        gpu_stats_provider=_fake_gpu,
        cpu_stats_provider=_fake_cpu,
    )
    await monitor.start()
    await asyncio.sleep(0.12)
    snap = monitor.snapshot()
    await monitor.stop()

    assert snap.gpu_available is True
    assert snap.cpu_utilization_pct == pytest.approx(30.0)


def test_no_gpu_returns_unavailable():
    monitor = ResourceMonitor(
        gpu_stats_provider=lambda: {"available": False, "devices": []},
        cpu_stats_provider=_fake_cpu,
    )
    monitor._refresh()
    assert monitor.snapshot().gpu_available is False
    assert monitor.snapshot().gpu_devices == []


# ---------------------------------------------------------------------------
# Issue #627 — Natural Language Agent
# ---------------------------------------------------------------------------
from ai.nl_agent import (
    ComplexityLevel,
    IntentType,
    NaturalLanguageAgent,
    NLResponse,
    _detect_complexity,
    _extract_moves,
    _recognize_intent,
)


class _FakeEngine:
    async def analyze(self, fen: str, depth: int = 18):
        from ai.stockfish_wasm import WASMAnalysisResult
        return WASMAnalysisResult(
            best_move="e2e4",
            evaluation=0.3,
            depth=depth,
            principal_variation=["e2e4", "e7e5", "g1f3"],
        )


_STARTING_FEN = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"


def test_intent_recognition():
    assert _recognize_intent("what is the best move here?")[0] == IntentType.SUGGEST_MOVE
    assert _recognize_intent("analyze this position")[0] == IntentType.ANALYZE_POSITION
    assert _recognize_intent("give me a hint")[0] == IntentType.GET_HINT
    assert _recognize_intent("what is a fork?")[0] == IntentType.LEARN_CONCEPT
    assert _recognize_intent("random text xyz")[0] == IntentType.UNKNOWN


def test_complexity_detection():
    assert _detect_complexity("explain it simply for a beginner") == ComplexityLevel.BEGINNER
    assert _detect_complexity("give me the advanced engine line") == ComplexityLevel.ADVANCED
    assert _detect_complexity("what should I play?") == ComplexityLevel.INTERMEDIATE


def test_extract_moves():
    moves = _extract_moves("compare e2e4 and d2d4")
    assert "e2e4" in moves
    assert "d2d4" in moves


@pytest.mark.asyncio
async def test_agent_suggest_move():
    agent = NaturalLanguageAgent(_FakeEngine())
    resp = await agent.process("what is the best move?", fen=_STARTING_FEN)
    assert resp.intent == IntentType.SUGGEST_MOVE
    assert resp.best_move == "e2e4"
    assert resp.evaluation == pytest.approx(0.3)
    assert "e2e4" in resp.text


@pytest.mark.asyncio
async def test_agent_hint_hides_move():
    agent = NaturalLanguageAgent(_FakeEngine())
    resp = await agent.process("give me a hint", fen=_STARTING_FEN)
    assert resp.intent == IntentType.GET_HINT
    assert resp.best_move is None  # hint should not reveal the move


@pytest.mark.asyncio
async def test_agent_learn_concept_fork():
    agent = NaturalLanguageAgent(_FakeEngine())
    resp = await agent.process("what is a fork?")
    assert resp.intent == IntentType.LEARN_CONCEPT
    assert "fork" in resp.text.lower()
    assert resp.metadata.get("concept") == "fork"


@pytest.mark.asyncio
async def test_agent_no_fen_returns_prompt():
    agent = NaturalLanguageAgent(_FakeEngine())
    resp = await agent.process("analyze this position")
    assert "position" in resp.text.lower() or "fen" in resp.text.lower()


@pytest.mark.asyncio
async def test_agent_beginner_complexity():
    agent = NaturalLanguageAgent(_FakeEngine())
    resp = await agent.process(
        "best move for a beginner?",
        fen=_STARTING_FEN,
        complexity=ComplexityLevel.BEGINNER,
    )
    assert "e2e4" in resp.text
    # Beginner response should not contain raw centipawn numbers
    assert "+0.30" not in resp.text


@pytest.mark.asyncio
async def test_agent_advanced_complexity():
    agent = NaturalLanguageAgent(_FakeEngine())
    resp = await agent.process(
        "give me the advanced line",
        fen=_STARTING_FEN,
        complexity=ComplexityLevel.ADVANCED,
    )
    assert "e2e4" in resp.text
    assert "PV" in resp.text or "→" in resp.text


def test_response_to_dict():
    resp = NLResponse(
        request_id="abc",
        intent=IntentType.SUGGEST_MOVE,
        text="Play e4",
        best_move="e2e4",
        evaluation=0.3,
        confidence=0.9,
    )
    d = resp.to_dict()
    assert d["response"] == "Play e4"
    assert d["best_move"] == "e2e4"
    assert d["confidence"] == pytest.approx(0.9)
