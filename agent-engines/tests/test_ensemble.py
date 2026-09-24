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
    return EngineAnalysis(
        engine_name=name,
        score_cp=score_cp,
        best_move_san=best_move_san,
        pv_san=pv_san or [best_move_san],
        elapsed_ms=elapsed_ms,
        error=error,
    )


async def _run(coro):
    return await coro


# ------------------------------------------------------------------ #
# ConsensusResult aggregation                                         #
# ------------------------------------------------------------------ #


class TestConsensusBuilding:
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
        assert result.evaluation_delta == 0.0
        assert result.recommended_move == "e4"


# ------------------------------------------------------------------ #
# Async parallel dispatch (mocked engines)                            #
# ------------------------------------------------------------------ #


class TestAsyncDispatch:
    @pytest.mark.asyncio
    async def test_parallel_execution(self):
        """Verify engines are queried concurrently."""
        evaluator = EnsembleEvaluator(
            engine_configs={"stockfish": {}, "lc0": {}},
            max_concurrent=2,
        )

        results = []

        async def fake_analyse(name, cfg, board, depth, timeout):
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
            raise OSError(f"{name} not found")

        evaluator._analyse_with_engine = fail_analyse  # type: ignore

        result = await evaluator.analyze(chess.Board())
        assert result.recommended_move is None
        assert all(a.error is not None for a in result.analyses)


# ------------------------------------------------------------------ #
# EngineAnalysis data class                                           #
# ------------------------------------------------------------------ #


class TestEngineAnalysis:
    def test_defaults(self):
        ea = EngineAnalysis(engine_name="test")
        assert ea.score_cp is None
        assert ea.score_mate is None
        assert ea.best_move_san is None
        assert ea.pv_san == []
        assert ea.error is None

    def test_with_values(self):
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
        evaluator = EnsembleEvaluator(engine_configs={"stockfish": {}})

        async def fake_analyse(name, cfg, board, depth, timeout):
            return _make_analysis(name, best_move_san="e5")

        evaluator._analyse_with_engine = fake_analyse  # type: ignore

        result = await evaluator.analyze_fen(chess.STARTING_FEN)
        assert result.fen == chess.STARTING_FEN
        assert result.recommended_move == "e5"
