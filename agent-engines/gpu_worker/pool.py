"""
Multi-Engine Consensus Evaluator — Stockfish + LCZero + Berserk ensemble.

Runs chess positions through multiple engines concurrently, aggregates
evaluation scores and candidate moves, and computes a consensus agreement
score.  Survives individual engine crashes without failing the whole request.
"""

from __future__ import annotations

import asyncio
import logging
import os
import subprocess
import time
from dataclasses import dataclass, field
from typing import Optional

import chess
import chess.engine

logger = logging.getLogger(__name__)

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------

_DEFAULT_ENGINES: dict[str, dict] = {
    "stockfish": {
        "engine_path": os.environ.get(
            "STOCKFISH_PATH", "stockfish"
        ),
        "threads": int(os.environ.get("STOCKFISH_THREADS", "2")),
        "hash_mb": int(os.environ.get("STOCKFISH_HASH_MB", "256")),
    },
    "lc0": {
        "engine_path": os.environ.get("LC0_PATH", "lc0"),
        "weights": os.environ.get("LC0_WEIGHTS", ""),
    },
    "berserk": {
        "engine_path": os.environ.get("BERSERK_PATH", "stockfish"),
        "threads": int(os.environ.get("BERSERK_THREADS", "2")),
        "hash_mb": int(os.environ.get("BERSERK_HASH_MB", "256")),
    },
}

DEFAULT_DEPTH = 18
DEFAULT_TIMEOUT_S = 10.0
ENGINE_CONNECT_TIMEOUT_S = 5.0


# ---------------------------------------------------------------------------
# Data classes
# ---------------------------------------------------------------------------

@dataclass
class EngineAnalysis:
    """Raw analysis output from a single engine."""
    engine_name: str
    score_cp: Optional[int] = None       # centipawn evaluation
    score_mate: Optional[int] = None     # moves to mate (+ white, - black)
    best_move_san: Optional[str] = None
    pv_san: list[str] = field(default_factory=list)  # principal variation
    elapsed_ms: float = 0.0
    error: Optional[str] = None


@dataclass
class ConsensusResult:
    """Aggregated multi-engine analysis."""
    fen: str
    analyses: list[EngineAnalysis]
    consensus_score: float = 0.0         # 0.0 – 1.0 agreement ratio
    best_move_agreement: bool = False
    evaluation_delta: float = 0.0        # max abs difference in cp between engines
    divergent: bool = False              # True when neural vs classical disagree
    recommended_move: Optional[str] = None
    total_elapsed_ms: float = 0.0


# ---------------------------------------------------------------------------
# EnsembleEvaluator
# ---------------------------------------------------------------------------

class EnsembleEvaluator:
    """
    Runs multiple chess engines in parallel and computes ensemble consensus.

    Usage::

        evaluator = EnsembleEvaluator()
        result = await evaluator.analyze(board, depth=20, timeout=10)
    """

    def __init__(
        self,
        engine_configs: Optional[dict[str, dict]] = None,
        *,
        max_concurrent: int = 3,
    ) -> None:
        self._configs = engine_configs or _DEFAULT_ENGINES
        self._semaphore = asyncio.Semaphore(max_concurrent)

    # -- public async API --------------------------------------------------

    async def analyze(
        self,
        board: chess.Board,
        *,
        depth: int = DEFAULT_DEPTH,
        timeout: float = DEFAULT_TIMEOUT_S,
    ) -> ConsensusResult:
        """Analyse *board* with all configured engines in parallel."""
        t0 = time.perf_counter()
        fen = board.fen()

        tasks = [
            self._analyse_with_engine(name, cfg, board, depth, timeout)
            for name, cfg in self._configs.items()
        ]
        analyses = await asyncio.gather(*tasks, return_exceptions=True)

        # Flatten any unexpected exceptions into EngineAnalysis with error
        clean: list[EngineAnalysis] = []
        for i, res in enumerate(analyses):
            if isinstance(res, Exception):
                name = list(self._configs.keys())[i]
                clean.append(EngineAnalysis(
                    engine_name=name, error=str(res),
                ))
            else:
                clean.append(res)

        elapsed = (time.perf_counter() - t0) * 1000
        return self._build_consensus(fen, clean, elapsed)

    async def analyze_fen(
        self,
        fen: str,
        *,
        depth: int = DEFAULT_DEPTH,
        timeout: float = DEFAULT_TIMEOUT_S,
    ) -> ConsensusResult:
        """Convenience: analyse a FEN string."""
        board = chess.Board(fen)
        return await self.analyze(board, depth=depth, timeout=timeout)

    # -- internal ----------------------------------------------------------

    async def _analyse_with_engine(
        self,
        name: str,
        cfg: dict,
        board: chess.Board,
        depth: int,
        timeout: float,
    ) -> EngineAnalysis:
        """Connect to *name* engine, analyse, and return result."""
        async with self._semaphore:
            return await asyncio.wait_for(
                self._run_engine(name, cfg, board, depth),
                timeout=timeout + ENGINE_CONNECT_TIMEOUT_S,
            )

    async def _run_engine(
        self,
        name: str,
        cfg: dict,
        board: chess.Board,
        depth: int,
    ) -> EngineAnalysis:
        """Spawn engine process, analyse position, and parse output."""
        t0 = time.perf_counter()
        result = EngineAnalysis(engine_name=name)
        engine_proc: Optional[chess.engine.SimpleEngine] = None

        try:
            engine_proc = chess.engine.SimpleEngine.popen_uci(
                cfg["engine_path"]
            )

            # Apply engine-specific options
            if "threads" in cfg:
                engine_proc.configure({"Threads": cfg["threads"]})
            if "hash_mb" in cfg:
                engine_proc.configure({"Hash": cfg["hash_mb"]})
            if cfg.get("weights") and name == "lc0":
                engine_proc.configure({"WeightsFile": cfg["weights"]})

            analysis = engine_proc.analyse(
                board,
                chess.engine.Limit(depth=depth),
            )

            # Parse score
            score = analysis.get("score")
            if score is not None:
                score_obj = score.white() if board.turn == chess.WHITE else score.score()
                if score_obj is not None:
                    result.score_cp = score_obj.score(mate_score=10000)

            # Best move
            pv = analysis.get("pv", [])
            if pv:
                result.best_move_san = board.san(pv[0])
                result.pv_san = [board.san(m) for m in pv[:5]]
                # Play through PV to get mate distance if applicable
                score_white = analysis.get("score")
                if score_white is not None:
                    mate = score_white.white().mate()
                    if mate is not None:
                        result.score_mate = mate

        except (FileNotFoundError, OSError) as exc:
            result.error = f"Engine process not found: {exc}"
            logger.warning("Engine %s not available: %s", name, exc)
        except chess.engine.EngineTerminatedError as exc:
            result.error = f"Engine terminated: {exc}"
            logger.warning("Engine %s terminated: %s", name, exc)
        except chess.engine.EngineError as exc:
            result.error = f"Engine error: {exc}"
            logger.warning("Engine %s error: %s", name, exc)
        except Exception as exc:
            result.error = f"Unexpected error: {exc}"
            logger.exception("Engine %s unexpected failure", name)
        finally:
            if engine_proc is not None:
                try:
                    engine_proc.quit()
                except Exception:
                    pass

        result.elapsed_ms = (time.perf_counter() - t0) * 1000
        return result

    @staticmethod
    def _build_consensus(
        fen: str,
        analyses: list[EngineAnalysis],
        total_ms: float,
    ) -> ConsensusResult:
        """Compute agreement metrics from individual engine analyses."""
        valid = [a for a in analyses if a.error is None and a.best_move_san is not None]

        result = ConsensusResult(fen=fen, analyses=analyses, total_elapsed_ms=total_ms)

        if len(valid) < 2:
            if valid:
                result.recommended_move = valid[0].best_move_san
            return result

        # Best-move agreement
        moves = [a.best_move_san for a in valid]
        most_common = max(set(moves), key=moves.count)
        agreement_count = moves.count(most_common)
        result.consensus_score = agreement_count / len(valid)
        result.best_move_agreement = result.consensus_score == 1.0
        result.recommended_move = most_common

        # Evaluation delta
        cps = [a.score_cp for a in valid if a.score_cp is not None]
        if cps:
            result.evaluation_delta = max(cps) - min(cps)

        # Divergence detection: flag if evaluation delta > 100 cp
        # or if engines disagree on best move
        result.divergent = (
            result.evaluation_delta > 100 or not result.best_move_agreement
        )

        return result
