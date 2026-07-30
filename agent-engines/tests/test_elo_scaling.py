"""Tests for ELO-based engine difficulty scaling (issue #761).

Covers:
- elo_to_engine_params: table boundary values, linear interpolation, edge cases
- EloScalingMiddleware.apply: parameter injection, respect of existing values,
  override_existing flag, EloAnalysisRequest vs plain AnalysisRequest fallback
- EloScalingMiddleware.configure_bridge: correct setoption calls sent to UCI bridge
- GPUAnalysisWorker.analyze: middleware invoked, depth / num_pv propagated to engine
"""

from __future__ import annotations

import asyncio
from unittest.mock import AsyncMock, MagicMock

import pytest

# Import directly from submodules to avoid the gpu_worker/__init__.py chain
# which transitively loads maia_worker → torch (unavailable in CI without GPU).
from gpu_worker.elo_middleware import EloAnalysisRequest, EloScalingMiddleware
from gpu_worker.elo_scaling import (
    EngineParams,
    _ELO_TABLE,
    elo_to_engine_params,
)
from gpu_worker.models import AnalysisRequest, AnalysisResult
from gpu_worker.uci_bridge import AsyncUciBridge, UciBestMove, UciInfo


# ============================================================================
# Helpers
# ============================================================================


def _make_request(elo: int | None = None, **kwargs) -> AnalysisRequest:
    """Build an AnalysisRequest (or EloAnalysisRequest) for testing."""
    defaults = {"fen": "8/8/8/8/8/8/8/K6k w - - 0 1"}
    defaults.update(kwargs)
    if elo is not None:
        return EloAnalysisRequest(**defaults, opponent_elo=elo)
    return AnalysisRequest(**defaults)


# ============================================================================
# elo_to_engine_params – unit tests
# ============================================================================


class TestEloToEngineParams:
    """Verify the ELO → engine-params mapping table and interpolation."""

    # --- Exact band boundaries ------------------------------------------------

    @pytest.mark.parametrize(
        "elo, expected_skill, expected_depth, expected_mpv",
        [
            (0,    0,   1,  5),   # lowest band
            (800,  2,   2,  4),
            (1000, 4,   4,  3),
            (1200, 7,   6,  3),
            (1400, 10,  8,  2),
            (1600, 13, 10,  2),
            (1800, 16, 14,  1),
            (2000, 18, 16,  1),
            (2200, 19, 18,  1),
            (2400, 20, 20,  1),  # highest band
        ],
    )
    def test_exact_band_boundaries(
        self, elo: int, expected_skill: int, expected_depth: int, expected_mpv: int
    ) -> None:
        params = elo_to_engine_params(elo)
        assert params.skill_level == expected_skill, f"ELO {elo}: skill"
        assert params.depth == expected_depth, f"ELO {elo}: depth"
        assert params.multi_pv == expected_mpv, f"ELO {elo}: multi_pv"
        assert params.elo == elo

    # --- Midpoint interpolation -----------------------------------------------

    def test_interpolation_midpoint_1200_1400(self) -> None:
        """1300 sits exactly at midpoint between 1200 and 1400 bands."""
        params = elo_to_engine_params(1300)
        # skill: lerp(7, 10, 0.5) = 8.5 → 9  (round-half-up)
        assert 8 <= params.skill_level <= 9, "skill should be ~8-9 at 1300 ELO"
        # depth: lerp(6, 8, 0.5) = 7
        assert params.depth == 7, "depth should be 7 at 1300 ELO"
        # multi_pv uses step function from lower band
        assert params.multi_pv == 3

    def test_interpolation_midpoint_1600_1800(self) -> None:
        """1700 sits exactly at midpoint between 1600 and 1800 bands."""
        params = elo_to_engine_params(1700)
        # skill: lerp(13, 16, 0.5) = 14.5 → 14 or 15
        assert 14 <= params.skill_level <= 15
        # depth: lerp(10, 14, 0.5) = 12
        assert params.depth == 12
        assert params.multi_pv == 2  # step from 1600 band

    def test_interpolation_midpoint_2000_2200(self) -> None:
        """2100 sits exactly at midpoint between 2000 and 2200 bands."""
        params = elo_to_engine_params(2100)
        # skill: lerp(18, 19, 0.5) = 18.5 → 18 or 19
        assert 18 <= params.skill_level <= 19
        assert params.multi_pv == 1

    # --- Edge cases -----------------------------------------------------------

    def test_negative_elo_clamped_to_zero(self) -> None:
        """Negative ELO values should produce the same result as ELO 0."""
        params = elo_to_engine_params(-500)
        assert params.skill_level == 0
        assert params.depth == 1
        assert params.multi_pv == 5
        assert params.elo == 0

    def test_very_high_elo_clamped_to_max(self) -> None:
        """ELO values above 3000 should produce full-strength settings."""
        params = elo_to_engine_params(9999)
        assert params.skill_level == 20
        assert params.depth == 20
        assert params.multi_pv == 1
        assert params.elo == 3000

    def test_just_below_band_boundary(self) -> None:
        """1199 interpolates into the 1000–1200 band, ending at skill=7, depth=6.

        bisect_right([..., 1000, 1200, ...], 1199) == index-of-1200, so left
        band is [1000, 1200) and t ≈ 0.995 → rounds to the 1200-anchor values.
        This is the correct and expected interpolation result.
        """
        params_1199 = elo_to_engine_params(1199)
        params_1200 = elo_to_engine_params(1200)
        # 1199 should produce params ≤ 1200 (equal is fine as the band endpoint)
        assert params_1199.skill_level <= params_1200.skill_level
        assert params_1199.depth <= params_1200.depth

    def test_just_above_band_boundary(self) -> None:
        """1201 should be very close to the 1200 band anchor values."""
        params_1200 = elo_to_engine_params(1200)
        params_1201 = elo_to_engine_params(1201)
        # 1201 is just one ELO above 1200; skill and depth should be ≥ 1200's
        assert params_1201.skill_level >= params_1200.skill_level
        assert params_1201.depth >= params_1200.depth

    # --- Monotonicity ---------------------------------------------------------

    def test_params_monotonically_increase_with_elo(self) -> None:
        """Higher ELO must always produce equal or stronger engine settings."""
        elos = list(range(0, 2601, 50))
        previous = elo_to_engine_params(elos[0])
        for elo in elos[1:]:
            current = elo_to_engine_params(elo)
            assert current.skill_level >= previous.skill_level, (
                f"skill_level not monotone at ELO {elo}"
            )
            assert current.depth >= previous.depth, (
                f"depth not monotone at ELO {elo}"
            )
            assert current.multi_pv <= previous.multi_pv, (
                f"multi_pv should be monotonically non-increasing at ELO {elo}"
            )
            previous = current

    # --- Dataclass validation -------------------------------------------------

    def test_engine_params_invalid_skill_level_raises(self) -> None:
        with pytest.raises(ValueError, match="skill_level"):
            EngineParams(skill_level=21, depth=10, multi_pv=1, elo=1500)

    def test_engine_params_invalid_depth_raises(self) -> None:
        with pytest.raises(ValueError, match="depth"):
            EngineParams(skill_level=10, depth=0, multi_pv=1, elo=1500)

    def test_engine_params_invalid_multi_pv_raises(self) -> None:
        with pytest.raises(ValueError, match="multi_pv"):
            EngineParams(skill_level=10, depth=10, multi_pv=0, elo=1500)

    def test_engine_params_to_dict(self) -> None:
        params = EngineParams(skill_level=10, depth=8, multi_pv=2, elo=1400)
        d = params.to_dict()
        assert d == {"skill_level": 10, "depth": 8, "multi_pv": 2, "elo": 1400}

    # --- Table completeness ---------------------------------------------------

    def test_table_is_sorted(self) -> None:
        elos = [row[0] for row in _ELO_TABLE]
        assert elos == sorted(elos), "ELO table must be sorted ascending"

    def test_all_table_values_in_valid_range(self) -> None:
        for elo_lower, skill, depth, mpv in _ELO_TABLE:
            assert 0 <= skill <= 20, f"skill out of range at ELO {elo_lower}"
            assert depth >= 1, f"depth < 1 at ELO {elo_lower}"
            assert mpv >= 1, f"multi_pv < 1 at ELO {elo_lower}"


# ============================================================================
# EloScalingMiddleware – unit tests
# ============================================================================


class TestEloScalingMiddleware:
    """Verify that the middleware correctly applies ELO parameters to requests."""

    def setup_method(self) -> None:
        self.middleware = EloScalingMiddleware(default_elo=1500)

    # --- apply() basic behaviour ----------------------------------------------

    def test_apply_sets_depth_and_num_pv_for_elo_request(self) -> None:
        request = _make_request(elo=1200)
        scaled, params = self.middleware.apply(request)

        expected = elo_to_engine_params(1200)
        assert scaled.depth == expected.depth
        assert scaled.num_pv == expected.multi_pv
        assert params.skill_level == expected.skill_level
        assert params.depth == expected.depth
        assert params.multi_pv == expected.multi_pv

    def test_apply_uses_default_elo_for_plain_request(self) -> None:
        """A plain AnalysisRequest without opponent_elo should use default_elo."""
        request = _make_request()  # no ELO
        scaled, params = self.middleware.apply(request)

        expected = elo_to_engine_params(self.middleware.default_elo)
        assert scaled.depth == expected.depth
        assert params.elo == self.middleware.default_elo

    def test_apply_does_not_mutate_original_request(self) -> None:
        request = _make_request(elo=800)
        original_depth = request.depth  # None before scaling
        self.middleware.apply(request)
        assert request.depth == original_depth

    def test_apply_preserves_fen(self) -> None:
        fen = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1"
        request = _make_request(elo=1500, fen=fen)
        scaled, _ = self.middleware.apply(request)
        assert scaled.fen == fen

    def test_apply_preserves_opponent_elo_on_result(self) -> None:
        request = _make_request(elo=1800)
        scaled, _ = self.middleware.apply(request)
        assert isinstance(scaled, EloAnalysisRequest)
        assert scaled.opponent_elo == 1800

    # --- override_existing flag -----------------------------------------------

    def test_apply_does_not_override_explicit_depth_by_default(self) -> None:
        """Explicitly set depth should be respected when override_existing=False."""
        request = _make_request(elo=2000, depth=5)
        scaled, _ = self.middleware.apply(request)
        assert scaled.depth == 5, "explicit depth should not be overridden"

    def test_apply_overrides_explicit_depth_when_flag_set(self) -> None:
        middleware = EloScalingMiddleware(default_elo=1500, override_existing=True)
        request = _make_request(elo=2000, depth=5)
        scaled, params = middleware.apply(request)
        assert scaled.depth == params.depth, "depth should be overridden"

    def test_apply_does_not_override_explicit_num_pv_by_default(self) -> None:
        request = _make_request(elo=1000, num_pv=4)
        scaled, _ = self.middleware.apply(request)
        assert scaled.num_pv == 4

    def test_apply_overrides_explicit_num_pv_when_flag_set(self) -> None:
        middleware = EloScalingMiddleware(default_elo=1500, override_existing=True)
        request = _make_request(elo=1000, num_pv=4)
        scaled, params = middleware.apply(request)
        assert scaled.num_pv == params.multi_pv

    # --- Extremes --------------------------------------------------------------

    def test_low_elo_produces_weakest_engine(self) -> None:
        request = _make_request(elo=100)
        scaled, params = self.middleware.apply(request)
        assert params.skill_level == 0
        assert params.depth == 1
        assert params.multi_pv == 5

    def test_high_elo_produces_strongest_engine(self) -> None:
        request = _make_request(elo=2800)
        scaled, params = self.middleware.apply(request)
        assert params.skill_level == 20
        assert params.depth == 20
        assert params.multi_pv == 1

    # --- configure_bridge() ---------------------------------------------------

    @pytest.mark.asyncio
    async def test_configure_bridge_sends_skill_level_and_multipv(self) -> None:
        """Middleware must send setoption commands to the UCI bridge."""
        bridge = MagicMock(spec=AsyncUciBridge)
        # Simulate the supported-options set that the real bridge tracks.
        bridge._supported_options = {"Skill Level", "MultiPV", "Hash", "Threads"}
        bridge._set_option_if_supported = AsyncMock()
        bridge.ensure_ready = AsyncMock()

        params = EngineParams(skill_level=7, depth=6, multi_pv=3, elo=1200)
        await self.middleware.configure_bridge(bridge, params)

        calls = {
            call.args for call in bridge._set_option_if_supported.call_args_list
        }
        assert ("Skill Level", "7") in calls, "Skill Level setoption not sent"
        assert ("MultiPV", "3") in calls, "MultiPV setoption not sent"
        bridge.ensure_ready.assert_called_once()

    @pytest.mark.asyncio
    async def test_configure_bridge_called_on_full_strength_engine(self) -> None:
        """Full-strength settings (ELO ≥ 2400) should still call configure_bridge."""
        bridge = MagicMock(spec=AsyncUciBridge)
        bridge._supported_options = {"Skill Level", "MultiPV"}
        bridge._set_option_if_supported = AsyncMock()
        bridge.ensure_ready = AsyncMock()

        params = EngineParams(skill_level=20, depth=20, multi_pv=1, elo=2400)
        await self.middleware.configure_bridge(bridge, params)

        bridge._set_option_if_supported.assert_any_call("Skill Level", "20")
        bridge._set_option_if_supported.assert_any_call("MultiPV", "1")


# ============================================================================
# EloAnalysisRequest – model tests
# ============================================================================


class TestEloAnalysisRequest:
    """Verify the extended request model."""

    def test_accepts_opponent_elo(self) -> None:
        req = EloAnalysisRequest(fen="8/8/8/8/8/8/8/K6k w - - 0 1", opponent_elo=1400)
        assert req.opponent_elo == 1400

    def test_opponent_elo_is_optional(self) -> None:
        req = EloAnalysisRequest(fen="8/8/8/8/8/8/8/K6k w - - 0 1")
        assert req.opponent_elo is None

    def test_is_subclass_of_analysis_request(self) -> None:
        req = EloAnalysisRequest(fen="8/8/8/8/8/8/8/K6k w - - 0 1", opponent_elo=1000)
        assert isinstance(req, AnalysisRequest)

    def test_inherits_fen_field_from_analysis_request(self) -> None:
        """EloAnalysisRequest must expose the same fen field as AnalysisRequest."""
        req = EloAnalysisRequest(fen="8/8/8/8/8/8/8/K6k w - - 0 1", opponent_elo=1200)
        assert req.fen == "8/8/8/8/8/8/8/K6k w - - 0 1"


# ============================================================================
# Integration: GPUAnalysisWorker – middleware wired in
# ============================================================================


class FakeStreamWriter:
    def __init__(self) -> None:
        self.buffer: list[str] = []
        self._closing = False

    def write(self, data: bytes) -> None:
        self.buffer.append(data.decode())

    async def drain(self) -> None:
        return

    def is_closing(self) -> bool:
        return self._closing


class FakeStreamReader:
    def __init__(self, lines: list[str]) -> None:
        self._queue: asyncio.Queue[bytes] = asyncio.Queue()
        for line in lines:
            self._queue.put_nowait(f"{line}\n".encode())

    async def readline(self) -> bytes:
        return await self._queue.get()


class FakeProcess:
    def __init__(self, lines: list[str]) -> None:
        self.stdin = FakeStreamWriter()
        self.stdout = FakeStreamReader(lines)
        self.returncode: int | None = None
        self.killed = False

    async def wait(self) -> int:
        self.returncode = 0
        return 0

    def kill(self) -> None:
        self.killed = True
        self.returncode = -9


def _build_engine_lines(best_move: str = "e2e4") -> list[str]:
    """Return a minimal UCI conversation that produces a best-move response."""
    return [
        "id name Stockfish 16.1",
        "option name Skill Level type spin default 20 min 0 max 20",
        "option name MultiPV type spin default 1 min 1 max 500",
        "option name Threads type spin default 1 min 1 max 1024",
        "option name Hash type spin default 16 min 1 max 33554432",
        "uciok",
        "readyok",   # initialize_options → ensure_ready
        "readyok",   # configure_bridge → ensure_ready
        f"info depth 6 score cp 30 nodes 500 pv {best_move}",
        f"bestmove {best_move}",
    ]


@pytest.mark.asyncio
async def test_worker_applies_elo_scaling_to_engine() -> None:
    """End-to-end: worker should send correct setoption commands for a 1200-ELO player."""
    from gpu_worker.config import EngineBackend, WorkerConfig
    from gpu_worker.worker import GPUAnalysisWorker
    from gpu_worker.uci_bridge import AsyncUciBridge

    process = FakeProcess(_build_engine_lines())
    config = WorkerConfig(engine_backend=EngineBackend.STOCKFISH)
    bridge = AsyncUciBridge(
        config,
        process_factory=lambda *a, **kw: process,
        command_timeout_seconds=5.0,
    )

    worker = GPUAnalysisWorker(config, bridge_factory=lambda _: bridge)
    await worker.start()

    request = EloAnalysisRequest(
        fen="8/8/8/8/8/8/8/K6k w - - 0 1",
        opponent_elo=1200,
    )
    result = await worker.analyze(request)

    assert result.best_move == "e2e4"
    commands = process.stdin.buffer

    # Skill Level and MultiPV must have been set before the go command.
    assert any("setoption name Skill Level value 7" in cmd for cmd in commands), (
        "Expected 'setoption name Skill Level value 7' in commands.\n"
        f"Actual commands:\n{commands}"
    )
    assert any("setoption name MultiPV value 3" in cmd for cmd in commands), (
        "Expected 'setoption name MultiPV value 3' in commands.\n"
        f"Actual commands:\n{commands}"
    )
    # go depth must match the ELO-derived depth (6 for 1200 ELO)
    assert any("go depth 6" in cmd for cmd in commands), (
        "Expected 'go depth 6' in commands.\n"
        f"Actual commands:\n{commands}"
    )


@pytest.mark.asyncio
async def test_worker_uses_default_elo_for_plain_request() -> None:
    """A plain AnalysisRequest (no opponent_elo) should fall back to default ELO."""
    from gpu_worker.config import EngineBackend, WorkerConfig
    from gpu_worker.worker import GPUAnalysisWorker
    from gpu_worker.uci_bridge import AsyncUciBridge

    default_elo = 1500
    expected_params = elo_to_engine_params(default_elo)

    process = FakeProcess(_build_engine_lines())
    config = WorkerConfig(engine_backend=EngineBackend.STOCKFISH)
    bridge = AsyncUciBridge(
        config,
        process_factory=lambda *a, **kw: process,
        command_timeout_seconds=5.0,
    )

    middleware = EloScalingMiddleware(default_elo=default_elo)
    worker = GPUAnalysisWorker(config, bridge_factory=lambda _: bridge, elo_middleware=middleware)
    await worker.start()

    request = AnalysisRequest(fen="8/8/8/8/8/8/8/K6k w - - 0 1")
    result = await worker.analyze(request)

    commands = process.stdin.buffer
    assert any(
        f"setoption name Skill Level value {expected_params.skill_level}" in cmd
        for cmd in commands
    )
    assert any(f"go depth {expected_params.depth}" in cmd for cmd in commands)


@pytest.mark.asyncio
async def test_worker_full_strength_for_2400_elo() -> None:
    """A 2400-ELO player should receive Skill Level 20, depth 20."""
    from gpu_worker.config import EngineBackend, WorkerConfig
    from gpu_worker.worker import GPUAnalysisWorker
    from gpu_worker.uci_bridge import AsyncUciBridge

    process = FakeProcess(_build_engine_lines())
    config = WorkerConfig(engine_backend=EngineBackend.STOCKFISH)
    bridge = AsyncUciBridge(
        config,
        process_factory=lambda *a, **kw: process,
        command_timeout_seconds=5.0,
    )

    worker = GPUAnalysisWorker(config, bridge_factory=lambda _: bridge)
    await worker.start()

    request = EloAnalysisRequest(
        fen="8/8/8/8/8/8/8/K6k w - - 0 1",
        opponent_elo=2400,
    )
    await worker.analyze(request)

    commands = process.stdin.buffer
    assert any("setoption name Skill Level value 20" in cmd for cmd in commands)
    assert any("go depth 20" in cmd for cmd in commands)
