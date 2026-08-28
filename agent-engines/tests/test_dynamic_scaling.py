"""Tests for mid-game dynamic ELO scaling.

Covers the controller's parameter modulation over simulated winning and losing
game trajectories, the plausibility rules around deliberate inaccuracies, the
rated-play lock, and the worker integration that applies all of it.
"""

from __future__ import annotations

import random

import pytest

from gpu_worker.config import GPUConfig, WorkerConfig
from gpu_worker.elo_middleware import EloAnalysisRequest
from gpu_worker.elo_scaling import (
    CandidateMove,
    DynamicEloController,
    DynamicScalingConfig,
    DynamicScalingRegistry,
    EngineParams,
    GameMode,
    MODULATED_MODES,
    ScalingState,
    elo_to_engine_params,
)
from gpu_worker.models import AnalysisRequest
from gpu_worker.resource_monitor import ResourceMonitor
from gpu_worker.uci_bridge import UciBestMove, UciInfo
from gpu_worker.worker import GPUAnalysisWorker

BASELINE = elo_to_engine_params(1500)

#: A game where the engine steadily runs away with it.
LOSING_TRAJECTORY = [0.0, -0.6, -1.4, -2.2, -3.1, -4.0, -4.8, -5.5, -6.0, -6.4]

#: A game where the player steadily takes over.
WINNING_TRAJECTORY = [0.0, 0.7, 1.5, 2.3, 3.2, 4.1, 4.9, 5.6, 6.1, 6.5]

#: A game that stays inside the target window.
BALANCED_TRAJECTORY = [0.0, 0.3, -0.4, 0.6, -0.2, 0.5, -0.5, 0.1]


def run_trajectory(
    controller: DynamicEloController,
    advantages: list[float],
    game_mode: GameMode = GameMode.CASUAL,
    baseline: EngineParams = BASELINE,
):
    """Feed a sequence of evaluations and collect one decision per move."""
    decisions = []
    for advantage in advantages:
        controller.observe(advantage)
        decisions.append(controller.decide(baseline, game_mode))
    return decisions


class TestConfiguration:
    """The configuration rejects settings that would break the controller."""

    def test_defaults_are_valid(self):
        config = DynamicScalingConfig()
        assert config.target_low < config.target_high
        assert config.struggle_threshold < config.target_low
        assert config.dominating_threshold > config.target_high

    @pytest.mark.parametrize(
        "kwargs",
        [
            {"target_low": 2.0, "target_high": 1.0},
            {"struggle_threshold": 0.0},
            {"dominating_threshold": 0.0},
            {"smoothing": 0.0},
            {"smoothing": 1.5},
            {"min_depth": 0},
            {"max_depth": 2, "min_depth": 4},
            {"max_depth_step": 0},
            {"candidate_pool": 0},
            {"max_inaccuracy_chance": 1.5},
            {"min_inaccuracy_loss": 2.0, "max_inaccuracy_loss": 1.0},
        ],
    )
    def test_invalid_configuration_is_rejected(self, kwargs):
        with pytest.raises(ValueError):
            DynamicScalingConfig(**kwargs)


class TestTargetWindow:
    """Inside the target window the engine plays its rated strength."""

    def test_balanced_game_is_left_alone(self):
        controller = DynamicEloController()
        for decision in run_trajectory(controller, BALANCED_TRAJECTORY):
            assert decision.state is ScalingState.NEUTRAL
            assert decision.params == BASELINE
            assert decision.inaccuracy_chance == 0.0
            assert not decision.is_modulated

    def test_first_move_uses_the_rated_strength(self):
        controller = DynamicEloController()
        decision = controller.decide(BASELINE)
        assert decision.params == BASELINE
        assert decision.state is ScalingState.NEUTRAL

    def test_window_edges_do_not_trigger_modulation(self):
        config = DynamicScalingConfig(smoothing=1.0)
        controller = DynamicEloController(config)
        for advantage in (config.target_low, config.target_high):
            controller.observe(advantage)
            assert controller.decide(BASELINE).state is ScalingState.NEUTRAL


class TestLosingTrajectory:
    """When the player falls behind, the engine eases off."""

    def test_depth_ramps_down_and_stops_at_the_floor(self):
        config = DynamicScalingConfig()
        controller = DynamicEloController(config)
        depths = [d.params.depth for d in run_trajectory(controller, LOSING_TRAJECTORY)]

        assert depths[0] == BASELINE.depth
        assert depths[-1] < BASELINE.depth
        assert depths == sorted(depths, reverse=True), "depth must not oscillate"
        assert min(depths) >= min(BASELINE.depth, config.min_depth)

    def test_skill_level_ramps_down_with_it(self):
        controller = DynamicEloController()
        skills = [
            d.params.skill_level for d in run_trajectory(controller, LOSING_TRAJECTORY)
        ]
        assert skills[-1] < BASELINE.skill_level
        assert skills == sorted(skills, reverse=True)

    def test_inaccuracy_chance_grows_as_the_player_falls_further_behind(self):
        config = DynamicScalingConfig()
        controller = DynamicEloController(config)
        chances = [
            d.inaccuracy_chance for d in run_trajectory(controller, LOSING_TRAJECTORY)
        ]

        assert chances[0] == 0.0
        assert chances == sorted(chances)
        assert max(chances) == pytest.approx(config.max_inaccuracy_chance)

    def test_easing_requests_enough_candidate_lines_to_choose_from(self):
        config = DynamicScalingConfig()
        controller = DynamicEloController(config)
        easing = [
            d
            for d in run_trajectory(controller, LOSING_TRAJECTORY)
            if d.inaccuracy_chance > 0
        ]

        assert easing, "the trajectory should have triggered easing"
        for decision in easing:
            assert decision.state is ScalingState.EASING
            assert decision.params.multi_pv >= config.candidate_pool

    def test_pressure_saturates_at_the_struggle_threshold(self):
        config = DynamicScalingConfig(smoothing=1.0)
        controller = DynamicEloController(config)
        controller.observe(config.struggle_threshold * 3)
        assert controller.decide(BASELINE).pressure == pytest.approx(-1.0)


class TestWinningTrajectory:
    """When the player takes over, the engine digs in."""

    def test_depth_ramps_up_towards_the_ceiling(self):
        config = DynamicScalingConfig()
        controller = DynamicEloController(config)
        depths = [d.params.depth for d in run_trajectory(controller, WINNING_TRAJECTORY)]

        assert depths[-1] > BASELINE.depth
        assert depths == sorted(depths)
        assert max(depths) <= max(BASELINE.depth, config.max_depth)

    def test_engine_never_weakens_itself_while_losing(self):
        controller = DynamicEloController()
        for decision in run_trajectory(controller, WINNING_TRAJECTORY):
            assert decision.inaccuracy_chance == 0.0
            assert decision.params.depth >= BASELINE.depth
            assert decision.params.skill_level >= BASELINE.skill_level

    def test_resisting_state_is_reported(self):
        controller = DynamicEloController()
        decisions = run_trajectory(controller, WINNING_TRAJECTORY)
        assert decisions[-1].state is ScalingState.RESISTING
        assert decisions[-1].is_modulated


class TestSmoothness:
    """Strength drifts; it never lurches."""

    @pytest.mark.parametrize(
        "trajectory", [LOSING_TRAJECTORY, WINNING_TRAJECTORY, BALANCED_TRAJECTORY]
    )
    def test_parameters_move_by_at_most_one_step_per_move(self, trajectory):
        config = DynamicScalingConfig()
        controller = DynamicEloController(config)
        decisions = run_trajectory(controller, trajectory)

        depths = [BASELINE.depth] + [d.params.depth for d in decisions]
        skills = [BASELINE.skill_level] + [d.params.skill_level for d in decisions]

        for before, after in zip(depths, depths[1:]):
            assert abs(after - before) <= config.max_depth_step
        for before, after in zip(skills, skills[1:]):
            assert abs(after - before) <= config.max_skill_step

    def test_a_single_evaluation_spike_barely_moves_the_engine(self):
        config = DynamicScalingConfig()
        controller = DynamicEloController(config)
        controller.observe(-9.0)
        decision = controller.decide(BASELINE)

        assert decision.smoothed_advantage > -9.0 * config.smoothing - 0.01
        assert BASELINE.depth - decision.params.depth <= config.max_depth_step

    def test_the_engine_recovers_when_the_game_evens_out(self):
        controller = DynamicEloController()
        run_trajectory(controller, LOSING_TRAJECTORY)
        eased = controller.decide(BASELINE)
        assert eased.params.depth < BASELINE.depth

        recovery = run_trajectory(controller, [0.0] * 12)
        assert recovery[-1].params.depth == BASELINE.depth
        assert recovery[-1].params.skill_level == BASELINE.skill_level
        assert recovery[-1].state is ScalingState.NEUTRAL

    def test_smoothing_lags_the_raw_evaluation(self):
        controller = DynamicEloController(DynamicScalingConfig(smoothing=0.35))
        controller.observe(-4.0)
        assert controller.smoothed_advantage == pytest.approx(-1.4)
        controller.observe(-4.0)
        assert -4.0 < controller.smoothed_advantage < -1.4


class TestCompetitiveIntegrity:
    """Rated and tournament games are never modulated."""

    @pytest.mark.parametrize("mode", [GameMode.RANKED, GameMode.TOURNAMENT])
    def test_rated_modes_pass_the_rated_parameters_through(self, mode):
        controller = DynamicEloController()
        for decision in run_trajectory(controller, LOSING_TRAJECTORY, game_mode=mode):
            assert decision.state is ScalingState.LOCKED
            assert decision.params == BASELINE
            assert decision.inaccuracy_chance == 0.0
            assert decision.pressure == 0.0

    @pytest.mark.parametrize("mode", [GameMode.RANKED, GameMode.TOURNAMENT])
    def test_rated_modes_never_play_an_inaccuracy(self, mode):
        controller = DynamicEloController()
        controller.observe(-6.0)
        decision = controller.decide(BASELINE, mode)
        choice = controller.select_move(
            [
                CandidateMove("e2e4", 0.8),
                CandidateMove("d2d4", 0.3),
            ],
            decision,
            rng=random.Random(0),
        )
        assert choice.move == "e2e4"
        assert not choice.is_inaccuracy

    def test_switching_out_of_a_rated_game_starts_from_the_rated_strength(self):
        controller = DynamicEloController()
        run_trajectory(controller, LOSING_TRAJECTORY, game_mode=GameMode.RANKED)
        casual = controller.decide(BASELINE, GameMode.CASUAL)
        assert BASELINE.depth - casual.params.depth <= 1

    def test_modulated_modes_are_the_casual_ones(self):
        assert MODULATED_MODES == {GameMode.CASUAL, GameMode.TRAINING}


class TestMoveSelection:
    """Deliberate mistakes stay plausible."""

    def setup_method(self):
        self.config = DynamicScalingConfig(max_inaccuracy_chance=1.0)
        self.controller = DynamicEloController(self.config)
        self.controller.observe(-8.0)
        self.controller.observe(-8.0)
        self.controller.observe(-8.0)
        self.decision = self.controller.decide(BASELINE)

    def test_easing_is_active_for_these_cases(self):
        assert self.decision.inaccuracy_chance == pytest.approx(1.0)

    def test_a_top_three_alternative_is_chosen(self):
        candidates = [
            CandidateMove("e2e4", 1.20),
            CandidateMove("d2d4", 0.70),
            CandidateMove("g1f3", 0.55),
        ]
        choice = self.controller.select_move(
            candidates, self.decision, rng=random.Random(7)
        )
        assert choice.is_inaccuracy
        assert choice.move in {"d2d4", "g1f3"}
        assert 0 < choice.eval_loss <= self.config.max_inaccuracy_loss

    def test_a_move_that_hangs_a_queen_is_never_chosen(self):
        candidates = [
            CandidateMove("e2e4", 1.20),
            CandidateMove("d1h5", -7.90),  # hangs the queen
            CandidateMove("a2a4", -6.50),
        ]
        for seed in range(50):
            choice = self.controller.select_move(
                candidates, self.decision, rng=random.Random(seed)
            )
            assert choice.move == "e2e4"
            assert not choice.is_inaccuracy

    def test_losses_beyond_the_cap_are_excluded(self):
        candidates = [
            CandidateMove("e2e4", 1.00),
            CandidateMove("d2d4", 0.40),  # 0.60 loss: acceptable
            CandidateMove("b1c3", -1.50),  # 2.50 loss: too much
        ]
        for seed in range(50):
            choice = self.controller.select_move(
                candidates, self.decision, rng=random.Random(seed)
            )
            assert choice.move in {"e2e4", "d2d4"}

    def test_equal_moves_are_not_treated_as_inaccuracies(self):
        candidates = [
            CandidateMove("e2e4", 0.50),
            CandidateMove("d2d4", 0.49),
        ]
        choice = self.controller.select_move(
            candidates, self.decision, rng=random.Random(1)
        )
        assert choice.move == "e2e4"
        assert not choice.is_inaccuracy

    def test_candidates_outside_the_pool_are_ignored(self):
        config = DynamicScalingConfig(max_inaccuracy_chance=1.0, candidate_pool=3)
        controller = DynamicEloController(config)
        controller.observe(-8.0)
        controller.observe(-8.0)
        controller.observe(-8.0)
        decision = controller.decide(BASELINE)
        candidates = [
            CandidateMove("e2e4", 1.00),  # best
            CandidateMove("d2d4", 0.85),  # second: inside the pool
            CandidateMove("g1f3", 0.80),  # third: inside the pool
            CandidateMove("h2h4", 0.60),  # fourth: playable, but outside the pool
        ]
        for seed in range(25):
            choice = controller.select_move(
                candidates, decision, rng=random.Random(seed)
            )
            assert choice.move != "h2h4"

    def test_the_best_move_is_played_when_no_alternative_qualifies(self):
        candidates = [CandidateMove("e2e4", 1.20)]
        choice = self.controller.select_move(
            candidates, self.decision, rng=random.Random(3)
        )
        assert choice.move == "e2e4"
        assert "no plausible inaccuracy" in choice.reason

    def test_unscored_candidates_are_skipped(self):
        candidates = [
            CandidateMove("e2e4", 1.20),
            CandidateMove("d2d4", None),
        ]
        choice = self.controller.select_move(
            candidates, self.decision, rng=random.Random(3)
        )
        assert choice.move == "e2e4"

    def test_no_candidates_yields_no_choice(self):
        assert self.controller.select_move([], self.decision) is None

    def test_candidates_are_ranked_before_choosing(self):
        candidates = [
            CandidateMove("d2d4", 0.70),
            CandidateMove("e2e4", 1.20),
        ]
        choice = self.controller.select_move(
            candidates, self.decision, rng=random.Random(2)
        )
        assert choice.move in {"e2e4", "d2d4"}
        if not choice.is_inaccuracy:
            assert choice.move == "e2e4"

    def test_a_neutral_decision_always_plays_the_best_move(self):
        controller = DynamicEloController()
        decision = controller.decide(BASELINE)
        candidates = [
            CandidateMove("e2e4", 1.20),
            CandidateMove("d2d4", 0.70),
        ]
        for seed in range(25):
            choice = controller.select_move(
                candidates, decision, rng=random.Random(seed)
            )
            assert choice.move == "e2e4"

    def test_a_missed_roll_plays_the_best_move(self):
        config = DynamicScalingConfig(max_inaccuracy_chance=0.05)
        controller = DynamicEloController(config)
        for _ in range(5):
            controller.observe(-8.0)
        decision = controller.decide(BASELINE)

        class NeverRolls(random.Random):
            def random(self) -> float:
                return 0.99

        choice = controller.select_move(
            [CandidateMove("e2e4", 1.2), CandidateMove("d2d4", 0.7)],
            decision,
            rng=NeverRolls(),
        )
        assert choice.move == "e2e4"
        assert not choice.is_inaccuracy


class TestRegistry:
    """Per-session controller bookkeeping."""

    def test_the_same_session_keeps_its_state(self):
        registry = DynamicScalingRegistry()
        first = registry.controller_for("game-1")
        first.observe(-4.0)
        assert registry.controller_for("game-1") is first
        assert registry.controller_for("game-1").smoothed_advantage < 0

    def test_sessions_are_isolated(self):
        registry = DynamicScalingRegistry()
        registry.controller_for("game-1").observe(-4.0)
        assert registry.controller_for("game-2").smoothed_advantage == 0.0

    def test_least_recently_used_sessions_are_evicted(self):
        registry = DynamicScalingRegistry(max_sessions=2)
        registry.controller_for("game-1")
        registry.controller_for("game-2")
        registry.controller_for("game-1")  # refresh game-1
        registry.controller_for("game-3")

        assert len(registry) == 2
        assert "game-2" not in registry
        assert "game-1" in registry and "game-3" in registry

    def test_finished_games_can_be_released(self):
        registry = DynamicScalingRegistry()
        registry.controller_for("game-1")
        registry.release("game-1")
        assert "game-1" not in registry
        registry.release("game-1")  # releasing twice is harmless

    def test_registry_rejects_an_empty_bound(self):
        with pytest.raises(ValueError):
            DynamicScalingRegistry(max_sessions=0)


# ---------------------------------------------------------------------------
# Worker integration
# ---------------------------------------------------------------------------

MIDGAME_FEN = "r1bqkb1r/pppp1ppp/2n2n2/4p3/2B1P3/5N2/PPPP1PPP/RNBQK2R w KQkq - 4 4"


class RecordingBridge:
    """Test double that reports three candidate lines and records its calls."""

    def __init__(self, config: WorkerConfig, evaluations: list[float] | None = None):
        self.config = config
        self.evaluations = list(evaluations or [0.4])
        self.go_calls: list[dict] = []
        self.options: dict[str, str] = {}
        self.last_search_lines: list[UciInfo] = []
        self.quit_called = False

    async def start(self) -> None:
        pass

    async def initialize_options(self) -> None:
        pass

    async def set_position(self, fen: str) -> None:
        pass

    async def _set_option_if_supported(self, name: str, value: str | None) -> None:
        self.options[name] = value

    async def ensure_ready(self) -> None:
        pass

    async def go(self, **kwargs) -> tuple[UciBestMove, UciInfo]:
        self.go_calls.append(kwargs)
        evaluation = self.evaluations[min(len(self.go_calls) - 1, len(self.evaluations) - 1)]
        self.last_search_lines = [
            UciInfo(
                depth=kwargs.get("depth"),
                evaluation=evaluation,
                principal_variation=["e2e4", "e7e5"],
                multipv=1,
            ),
            UciInfo(
                depth=kwargs.get("depth"),
                evaluation=evaluation - 0.4,
                principal_variation=["d2d4", "d7d5"],
                multipv=2,
            ),
            UciInfo(
                depth=kwargs.get("depth"),
                evaluation=evaluation - 3.5,
                principal_variation=["b2b4", "c7c5"],
                multipv=3,
            ),
        ]
        return UciBestMove(best_move="e2e4"), self.last_search_lines[0]

    async def quit(self) -> None:
        self.quit_called = True


def build_worker(bridge: RecordingBridge, registry: DynamicScalingRegistry | None = None):
    """Build a worker around a recording bridge."""
    monitor = ResourceMonitor(
        gpu_stats_provider=lambda: {"available": False, "devices": []},
        cpu_stats_provider=lambda: {"cpu_utilization_pct": 5.0},
    )
    return GPUAnalysisWorker(
        WorkerConfig(gpu=GPUConfig(device_id=0)),
        worker_id="worker-scaling",
        bridge_factory=lambda config: bridge,
        resource_monitor=monitor,
        scaling_registry=registry if registry is not None else DynamicScalingRegistry(),
    )


@pytest.mark.asyncio
async def test_worker_reduces_depth_over_a_losing_game() -> None:
    """A companion game that is going badly walks the search depth down."""
    # Engine-relative evaluations: strongly positive means the player is losing.
    bridge = RecordingBridge(WorkerConfig(), evaluations=[6.0] * 8)
    worker = build_worker(bridge)
    await worker.start()

    for _ in range(8):
        await worker.analyze(
            EloAnalysisRequest(
                fen=MIDGAME_FEN,
                opponent_elo=1500,
                game_mode=GameMode.CASUAL,
                session_id="game-1",
            )
        )
    await worker.shutdown()

    depths = [call["depth"] for call in bridge.go_calls]
    assert depths[0] == BASELINE.depth
    assert depths[-1] < depths[0]
    assert depths == sorted(depths, reverse=True)
    for before, after in zip(depths, depths[1:]):
        assert before - after <= 1


@pytest.mark.asyncio
async def test_worker_raises_depth_when_the_player_is_winning() -> None:
    """A companion game the engine is losing makes it defend harder."""
    bridge = RecordingBridge(WorkerConfig(), evaluations=[-6.0] * 8)
    worker = build_worker(bridge)
    await worker.start()

    for _ in range(8):
        await worker.analyze(
            EloAnalysisRequest(
                fen=MIDGAME_FEN,
                opponent_elo=1500,
                game_mode=GameMode.TRAINING,
                session_id="game-2",
            )
        )
    await worker.shutdown()

    depths = [call["depth"] for call in bridge.go_calls]
    assert depths[-1] > depths[0]
    assert depths == sorted(depths)


@pytest.mark.asyncio
async def test_worker_plays_a_plausible_alternative_when_easing() -> None:
    """While easing, the played move comes from the engine's top candidates."""
    registry = DynamicScalingRegistry(
        DynamicScalingConfig(max_inaccuracy_chance=1.0)
    )
    bridge = RecordingBridge(WorkerConfig(), evaluations=[6.0] * 6)
    worker = build_worker(bridge, registry)
    await worker.start()

    moves = []
    for _ in range(6):
        result = await worker.analyze(
            EloAnalysisRequest(
                fen=MIDGAME_FEN,
                opponent_elo=1500,
                game_mode=GameMode.CASUAL,
                session_id="game-3",
            )
        )
        moves.append(result.best_move)
    await worker.shutdown()

    # d2d4 concedes 0.4 pawns and is playable; b2b4 concedes 3.5 and is not.
    assert set(moves) <= {"e2e4", "d2d4"}
    assert "d2d4" in moves, "easing should eventually concede a little"
    assert "b2b4" not in moves


@pytest.mark.asyncio
async def test_worker_requests_enough_candidate_lines_while_easing() -> None:
    """MultiPV is raised so there is something to choose between."""
    bridge = RecordingBridge(WorkerConfig(), evaluations=[6.0] * 6)
    worker = build_worker(bridge)
    await worker.start()
    for _ in range(6):
        await worker.analyze(
            EloAnalysisRequest(
                fen=MIDGAME_FEN,
                opponent_elo=1500,
                game_mode=GameMode.CASUAL,
                session_id="game-4",
            )
        )
    await worker.shutdown()

    assert bridge.go_calls[-1]["num_pv"] >= 3


@pytest.mark.asyncio
async def test_worker_does_not_modulate_a_ranked_game() -> None:
    """Rated play keeps the rating-derived depth and always plays the best move."""
    bridge = RecordingBridge(WorkerConfig(), evaluations=[6.0] * 6)
    worker = build_worker(bridge)
    await worker.start()

    moves = []
    for _ in range(6):
        result = await worker.analyze(
            EloAnalysisRequest(
                fen=MIDGAME_FEN,
                opponent_elo=1500,
                game_mode=GameMode.RANKED,
                session_id="game-5",
            )
        )
        moves.append(result.best_move)
    await worker.shutdown()

    depths = {call["depth"] for call in bridge.go_calls}
    assert depths == {BASELINE.depth}
    assert set(moves) == {"e2e4"}


@pytest.mark.asyncio
async def test_worker_ignores_requests_that_declare_no_game_mode() -> None:
    """Plain analysis requests behave exactly as they did before."""
    bridge = RecordingBridge(WorkerConfig(), evaluations=[6.0] * 4)
    worker = build_worker(bridge)
    await worker.start()

    for _ in range(4):
        result = await worker.analyze(AnalysisRequest(fen=MIDGAME_FEN, depth=12))
        assert result.best_move == "e2e4"
    await worker.shutdown()

    assert {call["depth"] for call in bridge.go_calls} == {12}


@pytest.mark.asyncio
async def test_worker_keeps_sessions_apart() -> None:
    """One player's losing game does not weaken the engine in another game."""
    bridge = RecordingBridge(WorkerConfig(), evaluations=[6.0] * 10)
    registry = DynamicScalingRegistry()
    worker = build_worker(bridge, registry)
    await worker.start()

    for _ in range(6):
        await worker.analyze(
            EloAnalysisRequest(
                fen=MIDGAME_FEN,
                opponent_elo=1500,
                game_mode=GameMode.CASUAL,
                session_id="losing-game",
            )
        )
    losing_depth = bridge.go_calls[-1]["depth"]

    await worker.analyze(
        EloAnalysisRequest(
            fen=MIDGAME_FEN,
            opponent_elo=1500,
            game_mode=GameMode.CASUAL,
            session_id="fresh-game",
        )
    )
    await worker.shutdown()

    assert losing_depth < BASELINE.depth
    assert bridge.go_calls[-1]["depth"] == BASELINE.depth


@pytest.mark.asyncio
async def test_worker_applies_the_scaled_skill_level_to_the_engine() -> None:
    """The eased skill level actually reaches the engine via setoption."""
    bridge = RecordingBridge(WorkerConfig(), evaluations=[6.0] * 6)
    worker = build_worker(bridge)
    await worker.start()
    for _ in range(6):
        await worker.analyze(
            EloAnalysisRequest(
                fen=MIDGAME_FEN,
                opponent_elo=1500,
                game_mode=GameMode.CASUAL,
                session_id="game-6",
            )
        )
    await worker.shutdown()

    assert int(bridge.options["Skill Level"]) < BASELINE.skill_level


@pytest.mark.asyncio
async def test_worker_keeps_a_caller_supplied_registry() -> None:
    """An empty registry is falsy; the worker must still use the one it was given."""
    registry = DynamicScalingRegistry()
    bridge = RecordingBridge(WorkerConfig(), evaluations=[6.0] * 3)
    worker = build_worker(bridge, registry)
    await worker.start()
    for _ in range(3):
        await worker.analyze(
            EloAnalysisRequest(
                fen=MIDGAME_FEN,
                opponent_elo=1500,
                game_mode=GameMode.CASUAL,
                session_id="game-7",
            )
        )
    await worker.shutdown()

    assert "game-7" in registry
    assert registry.controller_for("game-7").smoothed_advantage < 0
