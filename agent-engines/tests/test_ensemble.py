"""
Tests for EnsembleEvaluator (pool.py).

Uses mocked engine processes to verify parallel dispatch, aggregation,
consensus scoring, and crash resilience without requiring real chess engines.
"""

import asyncio
from unittest.mock import AsyncMock, MagicMock, patch

import chess
import chess.engine
import pytest

from gpu_worker.pool import (
    ConsensusResult,
    EngineAnalysis,
    EnsembleEvaluator,
)


# ------------------------------------------------------------------ #
# Helpers                                                             #
# ------------------------------------------------------------------ #


def _make_analysis(
    name: str,
    best_move_san: str = "e4",
    score_cp: int = 20,
    elapsed_ms: float = 50.0,
    error: str | None = None,
    pv_san: list[str] | None = None,
) -> EngineAnalysis:
    # Convenience factory for building an EngineAnalysis with sensible
    # defaults, so individual tests only need to specify the fields that
    # matter for what they're checking (e.g. just `error=` for a failure
    # case, or just `score_cp=` for a scoring case).
    return EngineAnalysis(
        engine_name=name,
        score_cp=score_cp,
        best_move_san=best_move_san,
        pv_san=pv_san or [best_move_san],
        elapsed_ms=elapsed_ms,
        error=error,
    )


async def _run(coro):
    # Small awaitable pass-through helper. (Not currently used by any test
    # below, but available for inline-awaiting a coroutine if needed.)
    return await coro


# ------------------------------------------------------------------ #
# ConsensusResult aggregation                                         #
# ------------------------------------------------------------------ #


class TestConsensusBuilding:
    # These tests exercise `EnsembleEvaluator._build_consensus` directly
    # (a pure function over a list of EngineAnalysis) rather than going
    # through the async engine-dispatch path, so they run fast and don't
    # need any mocking.

    def test_full_agreement(self):
        """All engines agree on best move and score."""
        analyses = [
            _make_analysis("stockfish", "e4", score_cp=30),
            _make_analysis("lc0", "e4", score_cp=28),
            _make_analysis("berserk", "e4", score_cp=31),
        ]
        result = EnsembleEvaluator._build_consensus(
            chess.STARTING_FEN, analyses, total_ms=120.0
        )
        # 3/3 engines picked the same move => full consensus.
        assert result.consensus_score == 1.0
        assert result.best_move_agreement is True
        assert result.recommended_move == "e4"
        assert result.divergent is False

    def test_partial_agreement(self):
        """Two of three engines agree."""
        analyses = [
            _make_analysis("stockfish", "e4", score_cp=30),
            _make_analysis("lc0", "Nf3", score_cp=25),
            _make_analysis("berserk", "e4", score_cp=28),
        ]
        result = EnsembleEvaluator._build_consensus(
            chess.STARTING_FEN, analyses, total_ms=150.0
        )
        # 2 of 3 engines agree on "e4" => consensus score of 2/3, and the
        # majority move should still be surfaced as the recommendation.
        assert result.consensus_score == pytest.approx(2 / 3)
        assert result.best_move_agreement is False
        assert result.recommended_move == "e4"

    def test_no_agreement(self):
        """All engines disagree."""
        analyses = [
            _make_analysis("stockfish", "e4", score_cp=30),
            _make_analysis("lc0", "Nf3", score_cp=25),
            _make_analysis("berserk", "d4", score_cp=20),
        ]
        result = EnsembleEvaluator._build_consensus(
            chess.STARTING_FEN, analyses, total_ms=200.0
        )
        # Three different moves among three engines => only 1/3 "agree"
        # with whichever move ends up chosen, and the result should be
        # flagged divergent.
        assert result.consensus_score == pytest.approx(1 / 3)
        assert result.divergent is True

    def test_divergent_detection_large_eval_delta(self):
        """Flag as divergent when evaluation delta exceeds threshold."""
        analyses = [
            _make_analysis("stockfish", "e4", score_cp=200),
            _make_analysis("lc0", "e4", score_cp=50),
            _make_analysis("berserk", "e4", score_cp=190),
        ]
        result = EnsembleEvaluator._build_consensus(
            chess.STARTING_FEN, analyses, total_ms=100.0
        )
        # All three engines agree on the move itself, but their centipawn
        # evaluations differ by a large margin (200 - 50 = 150), so the
        # result should still be marked divergent based on eval spread
        # even though move agreement is 100%.
        assert result.best_move_agreement is True
        assert result.divergent is True
        assert result.evaluation_delta == 150

    def test_single_engine_result(self):
        """Fallback when only one engine responds."""
        analyses = [
            _make_analysis("stockfish", "e4", score_cp=30),
            _make_analysis("lc0", error="Engine not found"),
            _make_analysis("berserk", error="Timeout"),
        ]
        result = EnsembleEvaluator._build_consensus(
            chess.STARTING_FEN, analyses, total_ms=50.0
        )
        # Only one engine actually produced a usable result, so its move
        # is still used as the recommendation, but consensus_score can't
        # be meaningfully computed with fewer than 2 valid results.
        assert result.recommended_move == "e4"
        assert result.consensus_score == 0.0  # can't compute without 2+

    def test_no_valid_results(self):
        """All engines failed."""
        analyses = [
            _make_analysis("stockfish", error="crash"),
            _make_analysis("lc0", error="crash"),
        ]
        result = EnsembleEvaluator._build_consensus(
            chess.STARTING_FEN, analyses, total_ms=10.0
        )
        # No engine produced a usable analysis, so there's nothing to
        # recommend and consensus is trivially 0.
        assert result.recommended_move is None
        assert result.consensus_score == 0.0

    def test_evaluation_delta_computed(self):
        """Verify eval delta calculation across engines."""
        analyses = [
            _make_analysis("stockfish", "e4", score_cp=100),
            _make_analysis("lc0", "e4", score_cp=-50),
        ]
        result = EnsembleEvaluator._build_consensus(
            chess.STARTING_FEN, analyses, total_ms=80.0
        )
        # Delta should be the spread between max and min score_cp values
        # across engines: 100 - (-50) = 150.
        assert result.evaluation_delta == 150

    def test_no_cp_scores(self):
        """Handle analyses without centipawn scores gracefully."""
        analyses = [
            _make_analysis("stockfish", "e4", score_cp=None),
            _make_analysis("lc0", "e4", score_cp=None),
        ]
        result = EnsembleEvaluator._build_consensus(
            chess.STARTING_FEN, analyses, total_ms=60.0
        )
        # With no numeric scores available at all (e.g. mate-only scores),
        # the eval delta should default to 0 rather than erroring, while
        # move-based recommendation logic still works independently.
        assert result.evaluation_delta == 0.0
        assert result.recommended_move == "e4"


# ------------------------------------------------------------------ #
# Async parallel dispatch (mocked engines)                            #
# ------------------------------------------------------------------ #


class TestAsyncDispatch:
    # These tests patch out `_analyse_with_engine` on the evaluator
    # instance with a fake async function, so no real chess engine
    # subprocess is ever spawned. This lets us control timing, errors,
    # and per-engine behavior precisely.

    @pytest.mark.asyncio
    async def test_parallel_execution(self):
        """Verify engines are queried concurrently."""
        evaluator = EnsembleEvaluator(
            engine_configs={"stockfish": {}, "lc0": {}},
            max_concurrent=2,
        )

        results = []

        async def fake_analyse(name, cfg, board, depth, timeout):
            # Record which engines were actually invoked, so we can
            # confirm both engines got dispatched (not just one).
            results.append(name)
            return _make_analysis(name)

        evaluator._analyse_with_engine = fake_analyse  # type: ignore

        result = await evaluator.analyze(chess.Board())
        assert len(results) == 2
        assert set(results) == {"stockfish", "lc0"}

    @pytest.mark.asyncio
    async def test_crash_does_not_fail_others(self):
        """One engine crashing shouldn't affect the others."""
        evaluator = EnsembleEvaluator(
            engine_configs={"stockfish": {}, "lc0": {}, "berserk": {}},
        )

        async def selective_analyse(name, cfg, board, depth, timeout):
            # Simulate one specific engine (lc0) crashing with an
            # exception while the others behave normally.
            if name == "lc0":
                raise RuntimeError("LC0 crashed")
            return _make_analysis(name)

        evaluator._analyse_with_engine = selective_analyse  # type: ignore

        result = await evaluator.analyze(chess.Board())
        # Should still have results from stockfish and berserk
        successful = [a for a in result.analyses if a.error is None]
        assert len(successful) == 2
        assert result.recommended_move is not None

    @pytest.mark.asyncio
    async def test_timeout_handling(self):
        """Engine that takes too long is handled gracefully."""
        evaluator = EnsembleEvaluator(
            engine_configs={"stockfish": {}, "lc0": {}},
        )

        async def slow_analyse(name, cfg, board, depth, timeout):
            # Simulate stockfish timing out while lc0 responds normally.
            if name == "stockfish":
                raise asyncio.TimeoutError("timed out")
            return _make_analysis(name)

        evaluator._analyse_with_engine = slow_analyse  # type: ignore

        result = await evaluator.analyze(chess.Board())
        # stockfish timed out but lc0 should still produce a result
        successful = [a for a in result.analyses if a.error is None]
        assert len(successful) == 1

    @pytest.mark.asyncio
    async def test_all_engines_fail(self):
        """Graceful degradation when all engines fail."""
        evaluator = EnsembleEvaluator(
            engine_configs={"stockfish": {}, "lc0": {}},
        )

        async def fail_analyse(name, cfg, board, depth, timeout):
            # Every engine raises, simulating e.g. missing binaries.
            raise OSError(f"{name} not found")

        evaluator._analyse_with_engine = fail_analyse  # type: ignore

        result = await evaluator.analyze(chess.Board())
        # With zero successful analyses, there's no move to recommend and
        # every per-engine result should carry an error.
        assert result.recommended_move is None
        assert all(a.error is not None for a in result.analyses)


# ------------------------------------------------------------------ #
# EngineAnalysis data class                                           #
# ------------------------------------------------------------------ #


class TestEngineAnalysis:
    def test_defaults(self):
        # Confirms the dataclass's default field values when only the
        # required `engine_name` is provided.
        ea = EngineAnalysis(engine_name="test")
        assert ea.score_cp is None
        assert ea.score_mate is None
        assert ea.best_move_san is None
        assert ea.pv_san == []
        assert ea.error is None

    def test_with_values(self):
        # Confirms fields round-trip correctly when explicitly populated.
        ea = EngineAnalysis(
            engine_name="stockfish",
            score_cp=50,
            best_move_san="Nf3",
            pv_san=["Nf3", "Nf6", "g3"],
            elapsed_ms=42.5,
        )
        assert ea.engine_name == "stockfish"
        assert ea.score_cp == 50
        assert len(ea.pv_san) == 3


# ------------------------------------------------------------------ #
# ConsensusResult data class                                          #
# ------------------------------------------------------------------ #


class TestConsensusResult:
    def test_defaults(self):
        # Confirms ConsensusResult's default values for an empty
        # analyses list — i.e. before any consensus has been computed.
        cr = ConsensusResult(fen=chess.STARTING_FEN, analyses=[])
        assert cr.consensus_score == 0.0
        assert cr.best_move_agreement is False
        assert cr.evaluation_delta == 0.0
        assert cr.divergent is False


# ------------------------------------------------------------------ #
# analyze_fen convenience                                             #
# ------------------------------------------------------------------ #


class TestAnalyzeFen:
    @pytest.mark.asyncio
    async def test_analyze_fen(self):
        # Verifies the `analyze_fen` convenience method (which presumably
        # builds a chess.Board from a FEN string internally and delegates
        # to `analyze`) correctly threads the FEN through to the result
        # and returns the mocked engine's recommended move.
        evaluator = EnsembleEvaluator(engine_configs={"stockfish": {}})

        async def fake_analyse(name, cfg, board, depth, timeout):
            return _make_analysis(name, best_move_san="e5")

        evaluator._analyse_with_engine = fake_analyse  # type: ignore

        result = await evaluator.analyze_fen(chess.STARTING_FEN)
        assert result.fen == chess.STARTING_FEN
        assert result.recommended_move == "e5"