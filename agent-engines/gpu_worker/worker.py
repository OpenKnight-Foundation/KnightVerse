from __future__ import annotations

import asyncio
import logging
from collections.abc import Callable
import time
import uuid
import chess
import chess.polyglot

from gpu_worker.config import WorkerConfig
from gpu_worker.elo_middleware import EloAnalysisRequest, EloScalingMiddleware
from gpu_worker.elo_scaling import (
    CandidateMove,
    DynamicEloController,
    DynamicScalingRegistry,
    EngineParams,
    GameMode,
    MODULATED_MODES,
    MoveChoice,
    ScalingDecision,
)
from gpu_worker.models import AnalysisRequest, AnalysisResult, WorkerInfo, WorkerStatus
from gpu_worker.resource_monitor import ResourceMonitor
from gpu_worker.tablebase_prober import TablebaseProber, WdlResult
from gpu_worker.uci_bridge import AsyncUciBridge, UciBestMove, UciInfo
from gpu_worker.opening_book import OpeningBook

logger = logging.getLogger("KnightVerse.Worker")


class GPUAnalysisWorker:
    """Single GPU analysis worker wrapping a UCI engine process."""

    def __init__(
        self,
        config: WorkerConfig,
        worker_id: str | None = None,
        *,
        bridge_factory: Callable[[WorkerConfig], AsyncUciBridge] | None = None,
        resource_monitor: ResourceMonitor | None = None,
        opening_book: OpeningBook | None = None,
        elo_middleware: EloScalingMiddleware | None = None,
        scaling_registry: DynamicScalingRegistry | None = None,
    ) -> None:
        self.config = config
        self.worker_id = worker_id or str(uuid.uuid4())
        self._bridge_factory = bridge_factory or (lambda cfg: AsyncUciBridge(cfg))
        self._bridge = self._bridge_factory(config)
        self._monitor = resource_monitor or ResourceMonitor()
        self._opening_book = opening_book
        # Syzygy tablebase prover for 7-piece-and-fewer endgames.
        self._tablebase_prober = TablebaseProber(
            local_path=getattr(config, "syzygy_tablebase_path", None),
            remote_url=getattr(config, "syzygy_remote_url", None),
            config=config,
        )
        # ELO-based difficulty scaling middleware; defaults to enabled.
        self._elo_middleware = elo_middleware or EloScalingMiddleware()
        # Per-session mid-game strength modulation. The registry is always
        # present, but it only engages for requests that declare a casual or
        # training game mode, so rated play and plain AnalysisRequests are
        # unaffected.
        # Compared against None rather than truth-tested: an empty registry is
        # falsy, and a caller-supplied one must never be silently replaced.
        self._scaling_registry = (
            scaling_registry if scaling_registry is not None else DynamicScalingRegistry()
        )
        self._status = WorkerStatus.IDLE
        self._started = False
        self._analyses_completed = 0
        self._started_at: float | None = None
        self._pending_count = 0
        self._pending_lock = asyncio.Lock()
        self._analysis_lock = asyncio.Lock()

    @property
    def status(self) -> WorkerStatus:
        """Return the current worker status."""

        return self._status

    @property
    def load(self) -> int:
        """Return the number of queued or active analyses assigned to the worker."""

        return self._pending_count

    @property
    def has_capacity(self) -> bool:
        """Whether the worker can accept another queued analysis."""

        return self._pending_count < self.config.max_concurrent_analyses

    async def start(self) -> None:
        """Spawn the engine process, configure options, and start monitoring."""

        if self._started:
            return
        try:
            await self._bridge.start()
            await self._bridge.initialize_options()
            await self._monitor.start()
        except Exception:
            self._status = WorkerStatus.ERROR
            raise
        self._started = True
        self._started_at = time.monotonic()
        self._status = WorkerStatus.IDLE

    async def analyze(self, request: AnalysisRequest) -> AnalysisResult:
        """Analyze one position and return the normalized result."""

        if not self._started:
            raise RuntimeError("worker has not been started")

        async with self._pending_lock:
            if self._pending_count >= self.config.max_concurrent_analyses:
                raise RuntimeError("worker is at capacity")
            self._pending_count += 1

        started_at = time.monotonic()
        try:
            # Check for a book move first.
            if self._opening_book:
                board = chess.Board(request.fen)
                book_move = self._opening_book.find_move(board)
                if book_move:
                    return AnalysisResult(
                        request_id=request.id,
                        best_move=book_move.uci(),
                        evaluation=0,
                        depth=0,
                        principal_variation=[book_move.uci()],
                        nodes_searched=0,
                        time_ms=int((time.monotonic() - started_at) * 1000),
                        gpu_utilization=0.0,
                        is_book_move=True,
                    )

            async with self._analysis_lock:
                self._status = WorkerStatus.BUSY

                # -------------------------------------------------------
                # Check tablebase for positions with 7 or fewer pieces.
                # If a tablebase hit is found, return WDL/DTZ metrics
                # immediately, bypassing the engine search.
                # -------------------------------------------------------
                board = chess.Board(request.fen)
                if self._tablebase_prober and self._tablebase_prober._check_piece_count(board):
                    tb_result = await self._tablebase_prober.probe(board)
                    if tb_result is not None:
                        gpu_stats = self._monitor.get_gpu_stats()
                        result = AnalysisResult(
                            request_id=request.id,
                            best_move=chess.Move.null().uci(),
                            evaluation=tb_result.wdl,
                            depth=0,
                            principal_variation=[],
                            nodes_searched=0,
                            time_ms=int((time.monotonic() - started_at) * 1000),
                            gpu_utilization=_gpu_utilization_for_device(
                                gpu_stats, self.config.gpu.device_id
                            ),
                            is_tablebase_move=True,
                            wdl_result=tb_result,
                        )
                        self._analyses_completed += 1
                        return result

                # -------------------------------------------------------
                # Apply ELO-based difficulty scaling.
                # The middleware derives Skill Level / depth / MultiPV from
                # the opponent_elo field present on EloAnalysisRequest.
                # For plain AnalysisRequest objects the middleware falls back
                # to its configured default_elo.
                # -------------------------------------------------------
                scaled_request, engine_params = self._elo_middleware.apply(request)

                # -------------------------------------------------------
                # Modulate strength for how the current game is going. Only
                # casual and training games are eligible; rated and tournament
                # play keeps the rating-derived parameters. The controller
                # modulates around whatever this request would otherwise have
                # searched, so an explicit caller override stays the baseline.
                # -------------------------------------------------------
                search_depth = (
                    scaled_request.depth
                    or engine_params.depth
                    or self.config.default_depth
                )
                search_pv = scaled_request.num_pv or engine_params.multi_pv
                baseline = EngineParams(
                    skill_level=engine_params.skill_level,
                    depth=search_depth,
                    multi_pv=search_pv,
                    elo=engine_params.elo,
                )

                controller, decision = self._plan_scaling(request, baseline)
                if decision is not None:
                    engine_params = decision.params
                    search_depth = engine_params.depth
                    search_pv = engine_params.multi_pv

                await self._elo_middleware.configure_bridge(self._bridge, engine_params)

                await self._bridge.set_position(scaled_request.fen)
                best_move, info = await self._bridge.go(
                    depth=search_depth,
                    time_limit_ms=scaled_request.time_limit_ms
                    or self.config.default_time_limit_ms,
                    search_moves=scaled_request.search_moves,
                    num_pv=search_pv,
                )

                chosen = self._choose_move(controller, decision, best_move, info)
                evaluation = info.evaluation
                principal_variation = info.principal_variation
                if chosen is not None and chosen.is_inaccuracy:
                    if chosen.candidate.evaluation is not None:
                        evaluation = chosen.candidate.evaluation
                    if chosen.candidate.principal_variation:
                        principal_variation = list(
                            chosen.candidate.principal_variation
                        )

                if controller is not None and evaluation is not None:
                    # Engine scores are relative to the side to move, which is
                    # the engine here, so the controller sees the negation.
                    controller.observe_engine_evaluation(evaluation)

                gpu_stats = self._monitor.get_gpu_stats()
                result = AnalysisResult(
                    request_id=request.id,
                    best_move=chosen.move if chosen is not None else best_move.best_move,
                    evaluation=evaluation,
                    depth=info.depth,
                    principal_variation=principal_variation,
                    nodes_searched=info.nodes,
                    time_ms=int((time.monotonic() - started_at) * 1000),
                    gpu_utilization=_gpu_utilization_for_device(
                        gpu_stats, self.config.gpu.device_id
                    ),
                )
                self._analyses_completed += 1
                return result
        except Exception:
            self._status = WorkerStatus.ERROR
            raise
        finally:
            async with self._pending_lock:
                self._pending_count -= 1
                if self._status != WorkerStatus.ERROR:
                    self._status = (
                        WorkerStatus.BUSY if self._pending_count > 0 else WorkerStatus.IDLE
                    )

    def _plan_scaling(
        self,
        request: AnalysisRequest,
        baseline: EngineParams,
    ) -> tuple[DynamicEloController | None, ScalingDecision | None]:
        """Decide how strongly to play this move.

        Args:
            request: The incoming request, which may declare a game mode.
            baseline: Rating-derived parameters to modulate around.

        Returns:
            A ``(controller, decision)`` pair. Both are ``None`` when the
            request has not opted into modulation, so the engine behaves
            exactly as it did before.
        """
        game_mode = _request_game_mode(request)
        if game_mode is None or self._scaling_registry is None:
            return None, None

        session_id = request.session_id or request.actor_id
        if session_id is None:
            logger.debug(
                "Request %s declares %s but carries no session; "
                "skipping dynamic scaling",
                request.id,
                game_mode.value,
            )
            return None, None

        controller = self._scaling_registry.controller_for(session_id)
        decision = controller.decide(baseline, game_mode)
        if game_mode not in MODULATED_MODES:
            # Rated or tournament game: keep the controller so the session's
            # history survives, but play at full rated strength.
            return controller, None

        logger.debug("Dynamic scaling for session %s: %s", session_id, decision.reason)
        return controller, decision

    def _choose_move(
        self,
        controller: DynamicEloController | None,
        decision: ScalingDecision | None,
        best_move: UciBestMove,
        info: UciInfo,
    ) -> MoveChoice | None:
        """Pick which candidate move to play, if modulation is active.

        Returns ``None`` when the engine's own best move should be played
        unchanged.
        """
        if controller is None or decision is None:
            return None

        candidates = _candidates_from_search(self._bridge, best_move, info)
        if not candidates:
            return None

        chosen = controller.select_move(candidates, decision)
        if chosen is not None and chosen.is_inaccuracy:
            logger.info(
                "Dynamic scaling played %s instead of %s (%s)",
                chosen.move,
                best_move.best_move,
                chosen.reason,
            )
        return chosen

    async def shutdown(self) -> None:
        """Gracefully stop monitoring and terminate the engine process."""

        self._status = WorkerStatus.SHUTTING_DOWN
        await self._monitor.stop()
        await self._bridge.quit()
        self._started = False

    def get_info(self) -> WorkerInfo:
        """Return a runtime snapshot for pool monitoring."""

        gpu_stats = self._monitor.get_gpu_stats()
        device_stats = _gpu_device_stats(gpu_stats, self.config.gpu.device_id)
        uptime_seconds = 0.0
        if self._started_at is not None:
            uptime_seconds = max(0.0, time.monotonic() - self._started_at)
        return WorkerInfo(
            worker_id=self.worker_id,
            status=self._status,
            gpu_device_id=self.config.gpu.device_id,
            gpu_memory_used_mb=float(device_stats.get("memory_used_mb", 0.0)),
            gpu_utilization_pct=float(device_stats.get("utilization_pct", 0.0)),
            analyses_completed=self._analyses_completed,
            uptime_seconds=uptime_seconds,
        )


def _request_game_mode(request: AnalysisRequest) -> GameMode | None:
    """Return the game mode a request declares, or ``None`` if it declares none."""
    mode = getattr(request, "game_mode", None)
    if mode is None:
        return None
    try:
        return GameMode(mode)
    except ValueError:
        logger.warning("Unknown game mode %r on request %s", mode, request.id)
        return None


def _candidates_from_search(
    bridge: object,
    best_move: UciBestMove,
    info: UciInfo,
) -> list[CandidateMove]:
    """Build the candidate list from the last search's MultiPV lines.

    Falls back to the single best move when the engine (or a test double) did
    not report multiple lines.
    """
    lines = getattr(bridge, "last_search_lines", None) or []
    candidates = [
        CandidateMove(
            move=line.principal_variation[0],
            evaluation=line.evaluation,
            principal_variation=tuple(line.principal_variation),
        )
        for line in lines
        if line.principal_variation
    ]
    if candidates:
        return candidates

    return [
        CandidateMove(
            move=best_move.best_move,
            evaluation=info.evaluation,
            principal_variation=tuple(info.principal_variation),
        )
    ]


def _gpu_device_stats(gpu_stats: dict, device_id: int) -> dict:
    """Return the monitoring payload for one GPU device."""

    for device in gpu_stats.get("devices", []):
        if device.get("device_id") == device_id:
            return device
    return {}


def _gpu_utilization_for_device(gpu_stats: dict, device_id: int) -> float | None:
    """Return the utilization percentage for one GPU device if known."""

    device = _gpu_device_stats(gpu_stats, device_id)
    utilization = device.get("utilization_pct")
    return None if utilization is None else float(utilization)