"""ELO-based dynamic engine parameter scaling for KnightVerse.

This module maps human ELO ratings to Stockfish engine parameters so that
the engine plays at a roughly equivalent strength to the requesting player.
The goal is to guarantee a fair, fun experience: a 1200-ELO player should
never face a full-strength Stockfish Level 20 engine.

Mapping strategy
----------------
Stockfish exposes three levers that together control playing strength:

* ``Skill Level`` (0–20) — directly degrades move selection via error injection.
* ``depth``         (1–20) — limits how many half-moves the engine looks ahead.
* ``MultiPV``       (1–5)  — returning more candidate lines makes the engine
  consider weaker moves and effectively dilutes best-move accuracy.

The table below was calibrated against widely-cited Stockfish ELO benchmarks
and community testing data:

  ELO band      | Skill | Depth | MultiPV
  --------------|-------|-------|--------
  < 800         |  0    |  1    |  5
  800 – 999     |  2    |  2    |  4
  1000 – 1199   |  4    |  4    |  3
  1200 – 1399   |  7    |  6    |  3
  1400 – 1599   |  10   |  8    |  2
  1600 – 1799   |  13   |  10   |  2
  1800 – 1999   |  16   |  14   |  1
  2000 – 2199   |  18   |  16   |  1
  2200 – 2399   |  19   |  18   |  1
  2400+         |  20   |  20   |  1

For ELO values that fall between band boundaries the module applies linear
interpolation of Skill Level and depth so that the transition is smooth rather
than a sudden step.

Dynamic mid-game scaling
------------------------

The table above fixes a strength for the whole game. :class:`DynamicEloController`
adjusts it move by move from how the game is actually going, so a companion
game stays competitive: the engine eases off when the player is being run over
and digs in when the player is running away with it. It only does so in the
game modes listed in :data:`MODULATED_MODES` -- rated and tournament games
always get the engine's rated strength.
"""

from __future__ import annotations

import bisect
import logging
import math
import random
from collections import OrderedDict
from collections.abc import Sequence
from dataclasses import dataclass
from enum import Enum
from typing import Final

logger = logging.getLogger("KnightVerse.EloScaling")

#: Default random source for inaccuracy selection; tests inject their own.
_RNG: Final[random.Random] = random.Random()


@dataclass(frozen=True)
class EngineParams:
    """Engine parameters derived from a player's ELO rating.

    Attributes:
        skill_level: Stockfish ``Skill Level`` option (0–20).
        depth:       Search depth in half-moves (plies).
        multi_pv:    Number of principal variations (lines) to consider.
        elo:         The original ELO rating that produced these params.
    """

    skill_level: int
    depth: int
    multi_pv: int
    elo: int

    def __post_init__(self) -> None:
        if not 0 <= self.skill_level <= 20:
            raise ValueError(f"skill_level must be 0–20, got {self.skill_level}")
        if self.depth < 1:
            raise ValueError(f"depth must be >= 1, got {self.depth}")
        if self.multi_pv < 1:
            raise ValueError(f"multi_pv must be >= 1, got {self.multi_pv}")

    def to_dict(self) -> dict[str, int]:
        """Serialise to a plain dict (useful for logging / JSON payloads)."""
        return {
            "skill_level": self.skill_level,
            "depth": self.depth,
            "multi_pv": self.multi_pv,
            "elo": self.elo,
        }


# ---------------------------------------------------------------------------
# Reference table
# Each entry: (lower_elo_bound, skill_level, depth, multi_pv)
# The table *must* be sorted ascending by elo_lower.
# ---------------------------------------------------------------------------

_ELO_TABLE: Final[list[tuple[int, int, int, int]]] = [
    (0,    0,   1,  5),
    (800,  2,   2,  4),
    (1000, 4,   4,  3),
    (1200, 7,   6,  3),
    (1400, 10,  8,  2),
    (1600, 13, 10,  2),
    (1800, 16, 14,  1),
    (2000, 18, 16,  1),
    (2200, 19, 18,  1),
    (2400, 20, 20,  1),
]

# Pre-compute sorted breakpoints for binary search.
_ELO_BREAKPOINTS: Final[list[int]] = [row[0] for row in _ELO_TABLE]

# The minimum and maximum representable ELO.
_MIN_ELO: Final[int] = 0
_MAX_ELO: Final[int] = 3000  # above 2400, table saturates at full strength


def _clamp(value: float, lo: float, hi: float) -> float:
    """Clamp *value* to [lo, hi]."""
    return max(lo, min(hi, value))


def _lerp(a: float, b: float, t: float) -> float:
    """Linear interpolation between *a* and *b* by factor *t* ∈ [0, 1]."""
    return a + (b - a) * t


def _lookup_row(index: int) -> tuple[int, int, int, int]:
    """Return the table row at *index*, clamped to valid range."""
    clamped = max(0, min(index, len(_ELO_TABLE) - 1))
    return _ELO_TABLE[clamped]


def elo_to_engine_params(elo: int) -> EngineParams:
    """Map a player's ELO rating to Stockfish engine parameters.

    The function performs linear interpolation of ``skill_level`` and ``depth``
    between the two nearest table entries.  ``multi_pv`` uses the lower-bound
    row value (step function) because fractional MultiPV values don't exist.

    Args:
        elo: The player's ELO rating.  Values below 0 are treated as 0;
             values above 3000 are treated as 3000 (full strength).

    Returns:
        An :class:`EngineParams` instance with the recommended settings.

    Examples:
        >>> params = elo_to_engine_params(1200)
        >>> params.skill_level
        7
        >>> params.depth
        6

        >>> params = elo_to_engine_params(1300)  # midpoint of 1200–1400 band
        >>> params.skill_level  # lerped between 7 and 10
        8
    """
    clamped_elo = int(_clamp(elo, _MIN_ELO, _MAX_ELO))

    # Find the index of the first breakpoint that is *strictly greater* than
    # clamped_elo.  bisect_right gives us exactly that.
    right_index = bisect.bisect_right(_ELO_BREAKPOINTS, clamped_elo)

    # The player's ELO sits between table[left_index] and table[right_index].
    left_index = max(0, right_index - 1)

    lo_elo, lo_skill, lo_depth, lo_mpv = _lookup_row(left_index)
    hi_elo, hi_skill, hi_depth, _hi_mpv = _lookup_row(min(right_index, len(_ELO_TABLE) - 1))

    # Compute interpolation factor t ∈ [0, 1].
    band_width = hi_elo - lo_elo
    if band_width > 0 and left_index != right_index:
        t = _clamp((clamped_elo - lo_elo) / band_width, 0.0, 1.0)
    else:
        t = 0.0

    skill_level = round(_lerp(lo_skill, hi_skill, t))
    depth = round(_lerp(lo_depth, hi_depth, t))
    # MultiPV: step function – use the lower bound value (less dilution as
    # the player is stronger within a band is fine; the big jumps happen at
    # band boundaries which is desirable).
    multi_pv = lo_mpv

    params = EngineParams(
        skill_level=int(_clamp(skill_level, 0, 20)),
        depth=int(max(1, depth)),
        multi_pv=int(max(1, multi_pv)),
        elo=clamped_elo,
    )

    logger.debug(
        "ELO %d → skill=%d depth=%d multi_pv=%d (t=%.2f)",
        clamped_elo,
        params.skill_level,
        params.depth,
        params.multi_pv,
        t,
    )
    return params


def apply_params_to_request(
    request_kwargs: dict,
    params: EngineParams,
) -> dict:
    """Inject ELO-derived parameters into an analysis-request keyword dict.

    This is a pure helper used by the middleware layer so that the mapping
    logic is decoupled from request construction.

    Args:
        request_kwargs: Mutable dict of :class:`~gpu_worker.models.AnalysisRequest`
                        field values.
        params: Engine parameters produced by :func:`elo_to_engine_params`.

    Returns:
        The *same* dict with ``depth`` and ``num_pv`` set (existing values are
        overwritten only if the caller has not already set them explicitly; set
        ``depth`` and ``num_pv`` to ``None`` before calling if you always want
        the ELO-derived values).
    """
    if request_kwargs.get("depth") is None:
        request_kwargs["depth"] = params.depth
    if request_kwargs.get("num_pv") is None:
        request_kwargs["num_pv"] = params.multi_pv
    return request_kwargs


# ---------------------------------------------------------------------------
# Dynamic mid-game scaling
#
# The static table above picks a strength for the *player's rating*. The
# controller below adjusts that strength for how the *current game* is
# actually going, so a companion game stays close instead of turning into a
# rout in either direction.
# ---------------------------------------------------------------------------


class GameMode(str, Enum):
    """How a game is being played.

    Only :data:`MODULATED_MODES` may have the engine's strength adjusted
    mid-game; rated and tournament play always uses the unmodified parameters.
    """

    CASUAL = "casual"
    TRAINING = "training"
    RANKED = "ranked"
    TOURNAMENT = "tournament"


#: Modes in which mid-game strength modulation is permitted.
MODULATED_MODES: Final[frozenset[GameMode]] = frozenset(
    {GameMode.CASUAL, GameMode.TRAINING}
)


class ScalingState(str, Enum):
    """What the controller is currently doing to the engine's strength."""

    #: Rated or tournament play: parameters are passed through untouched.
    LOCKED = "locked"
    #: The game is inside the target window; the engine plays its rated strength.
    NEUTRAL = "neutral"
    #: The player is falling behind; the engine is holding back.
    EASING = "easing"
    #: The player is dominating; the engine is digging in.
    RESISTING = "resisting"


@dataclass(frozen=True)
class DynamicScalingConfig:
    """Tuning knobs for :class:`DynamicEloController`.

    Evaluations are in pawns from the *player's* point of view: positive means
    the human is better, negative means the engine is better.

    Attributes:
        target_low: Bottom of the window the controller tries to keep the game in.
        target_high: Top of that window.
        struggle_threshold: Advantage at which the engine holds back as much as
            it is allowed to.
        dominating_threshold: Advantage at which the engine resists as hard as
            it is allowed to.
        smoothing: EMA factor applied to observed evaluations, so a single
            sharp swing does not swing the engine's strength with it.
        min_depth: Floor on search depth while easing off.
        max_depth: Ceiling on search depth while resisting.
        min_skill_level: Floor on Stockfish ``Skill Level`` while easing off.
        max_skill_level: Ceiling on ``Skill Level`` while resisting.
        max_depth_step: Maximum plies the depth may move per decision.
        max_skill_step: Maximum skill levels the engine may move per decision.
        candidate_pool: How many engine candidate moves to choose between.
        max_inaccuracy_chance: Highest probability of playing a candidate other
            than the best one, reached at ``struggle_threshold``.
        min_inaccuracy_loss: An alternative must concede at least this much to
            count as a real inaccuracy.
        max_inaccuracy_loss: An alternative conceding more than this is never
            played -- this is what keeps the mistakes positional rather than
            hanging a piece.
    """

    target_low: float = -1.0
    target_high: float = 1.0
    struggle_threshold: float = -3.0
    dominating_threshold: float = 3.0
    smoothing: float = 0.35
    min_depth: int = 4
    max_depth: int = 22
    min_skill_level: int = 3
    max_skill_level: int = 20
    max_depth_step: int = 1
    max_skill_step: int = 1
    candidate_pool: int = 3
    max_inaccuracy_chance: float = 0.45
    min_inaccuracy_loss: float = 0.10
    max_inaccuracy_loss: float = 0.90

    def __post_init__(self) -> None:
        if self.target_low > self.target_high:
            raise ValueError("target_low must not exceed target_high")
        if self.struggle_threshold >= self.target_low:
            raise ValueError("struggle_threshold must be below target_low")
        if self.dominating_threshold <= self.target_high:
            raise ValueError("dominating_threshold must be above target_high")
        if not 0.0 < self.smoothing <= 1.0:
            raise ValueError("smoothing must be in (0, 1]")
        if self.min_depth < 1:
            raise ValueError("min_depth must be >= 1")
        if self.max_depth < self.min_depth:
            raise ValueError("max_depth must be >= min_depth")
        if not 0 <= self.min_skill_level <= self.max_skill_level <= 20:
            raise ValueError("skill bounds must satisfy 0 <= min <= max <= 20")
        if self.max_depth_step < 1 or self.max_skill_step < 1:
            raise ValueError("step limits must be >= 1")
        if self.candidate_pool < 1:
            raise ValueError("candidate_pool must be >= 1")
        if not 0.0 <= self.max_inaccuracy_chance <= 1.0:
            raise ValueError("max_inaccuracy_chance must be in [0, 1]")
        if not 0.0 <= self.min_inaccuracy_loss <= self.max_inaccuracy_loss:
            raise ValueError("inaccuracy loss bounds must satisfy 0 <= min <= max")


@dataclass(frozen=True)
class ScalingDecision:
    """The engine settings to use for one move, and why.

    Attributes:
        params: Parameters to apply to this search.
        state: What the controller is doing.
        smoothed_advantage: Smoothed player advantage the decision was based on.
        pressure: Normalised intervention in ``[-1, 1]``; negative eases off,
            positive resists, zero leaves the rated strength alone.
        inaccuracy_chance: Probability that :meth:`DynamicEloController.select_move`
            plays a candidate other than the best one.
        reason: Human-readable summary, useful in logs and replays.
    """

    params: EngineParams
    state: ScalingState
    smoothed_advantage: float
    pressure: float
    inaccuracy_chance: float
    reason: str

    @property
    def is_modulated(self) -> bool:
        """Whether this decision changes anything about the engine's play."""
        return self.state not in (ScalingState.LOCKED, ScalingState.NEUTRAL)

    def to_dict(self) -> dict:
        """Serialise to a plain dict (useful for logging / JSON payloads)."""
        return {
            "params": self.params.to_dict(),
            "state": self.state.value,
            "smoothed_advantage": round(self.smoothed_advantage, 3),
            "pressure": round(self.pressure, 3),
            "inaccuracy_chance": self.inaccuracy_chance,
            "reason": self.reason,
        }


@dataclass(frozen=True)
class CandidateMove:
    """One engine candidate line, as reported by MultiPV.

    Attributes:
        move: The move in UCI notation.
        evaluation: Evaluation in pawns from the *engine's* point of view, so
            higher is better for the engine. ``None`` when the engine did not
            report a score for the line.
        principal_variation: The line the engine expects after this move.
    """

    move: str
    evaluation: float | None = None
    principal_variation: tuple[str, ...] = ()


@dataclass(frozen=True)
class MoveChoice:
    """The move the controller decided to play.

    Attributes:
        candidate: The chosen candidate.
        is_inaccuracy: True when a deliberately weaker move was chosen.
        eval_loss: How much was conceded against the best move, in pawns.
        reason: Human-readable summary.
    """

    candidate: CandidateMove
    is_inaccuracy: bool
    eval_loss: float
    reason: str

    @property
    def move(self) -> str:
        """The chosen move in UCI notation."""
        return self.candidate.move


class DynamicEloController:
    """Adjusts engine strength during a single game to keep it competitive.

    The controller watches the evaluation from the player's point of view. While
    the game sits inside the target window it does nothing. As the player falls
    behind it walks the search depth and skill level down and starts choosing
    between the engine's top candidate moves; as the player takes over it walks
    them back up so the engine defends harder.

    Two properties are deliberately built in:

    * **Smoothness.** Observations are smoothed with an EMA and depth/skill may
      only move by :attr:`DynamicScalingConfig.max_depth_step` /
      ``max_skill_step`` per decision, so strength drifts rather than lurches.
    * **Plausibility.** A weaker move is only ever taken from the engine's top
      candidates, and only when it concedes between
      :attr:`DynamicScalingConfig.min_inaccuracy_loss` and ``max_inaccuracy_loss``
      pawns. Moves that drop material outright are never chosen.

    One instance tracks one game. Use :class:`DynamicScalingRegistry` to keep a
    controller per session.
    """

    def __init__(
        self,
        config: DynamicScalingConfig | None = None,
        *,
        initial_advantage: float = 0.0,
    ) -> None:
        """Initialize the controller.

        Args:
            config: Tuning knobs; the defaults suit a companion game.
            initial_advantage: Advantage to start the smoothing from, for a
                controller attached to a game already in progress.
        """
        self.config = config or DynamicScalingConfig()
        self._advantage = float(initial_advantage)
        self._depth: float | None = None
        self._skill: float | None = None
        self._observations = 0

    # ------------------------------------------------------------------
    # Observation
    # ------------------------------------------------------------------

    @property
    def smoothed_advantage(self) -> float:
        """Current smoothed advantage, positive when the player is better."""
        return self._advantage

    @property
    def observations(self) -> int:
        """How many evaluations have been fed to the controller."""
        return self._observations

    def observe(self, player_advantage: float) -> float:
        """Record the live evaluation of the game.

        Args:
            player_advantage: Evaluation in pawns from the player's point of
                view: positive means the human is winning.

        Returns:
            The updated smoothed advantage.
        """
        alpha = self.config.smoothing
        self._advantage = (1.0 - alpha) * self._advantage + alpha * float(
            player_advantage
        )
        self._observations += 1
        return self._advantage

    def observe_engine_evaluation(self, evaluation: float) -> float:
        """Record an evaluation reported from the *engine's* point of view.

        Engine scores are relative to the side to move, which during the
        engine's own search is the engine itself, so the player's advantage is
        the negation.

        Args:
            evaluation: Evaluation in pawns, positive meaning good for the engine.

        Returns:
            The updated smoothed advantage.
        """
        return self.observe(-float(evaluation))

    # ------------------------------------------------------------------
    # Decision
    # ------------------------------------------------------------------

    def decide(
        self,
        baseline: EngineParams,
        game_mode: GameMode = GameMode.CASUAL,
    ) -> ScalingDecision:
        """Choose the engine settings for the next move.

        Args:
            baseline: The rating-derived parameters to modulate around.
            game_mode: Mode of the game in progress. Rated and tournament games
                are returned unmodulated.

        Returns:
            A :class:`ScalingDecision` for this move.
        """
        if game_mode not in MODULATED_MODES:
            # Competitive integrity: never modulate, and drop any ramp state so
            # a later casual game starts from the rated strength.
            self._depth = None
            self._skill = None
            return ScalingDecision(
                params=baseline,
                state=ScalingState.LOCKED,
                smoothed_advantage=self._advantage,
                pressure=0.0,
                inaccuracy_chance=0.0,
                reason=f"{game_mode.value} game: strength modulation disabled",
            )

        pressure = self._pressure(self._advantage)
        depth = self._step_depth(baseline, pressure)
        skill = self._step_skill(baseline, pressure)
        chance = round(max(0.0, -pressure) * self.config.max_inaccuracy_chance, 3)
        multi_pv = (
            max(baseline.multi_pv, self.config.candidate_pool)
            if chance > 0.0
            else baseline.multi_pv
        )

        if pressure < 0:
            state = ScalingState.EASING
            reason = (
                f"player behind by {abs(self._advantage):.2f}; easing to "
                f"depth {depth}, skill {skill}"
            )
        elif pressure > 0:
            state = ScalingState.RESISTING
            reason = (
                f"player ahead by {self._advantage:.2f}; resisting at "
                f"depth {depth}, skill {skill}"
            )
        else:
            state = ScalingState.NEUTRAL
            reason = (
                f"evaluation {self._advantage:+.2f} inside target window "
                f"[{self.config.target_low:+.1f}, {self.config.target_high:+.1f}]"
            )

        params = EngineParams(
            skill_level=skill,
            depth=depth,
            multi_pv=multi_pv,
            elo=baseline.elo,
        )

        logger.debug(
            "Dynamic scaling: advantage=%.2f pressure=%+.2f → depth=%d skill=%d "
            "multi_pv=%d inaccuracy=%.2f (%s)",
            self._advantage,
            pressure,
            depth,
            skill,
            multi_pv,
            chance,
            state.value,
        )

        return ScalingDecision(
            params=params,
            state=state,
            smoothed_advantage=self._advantage,
            pressure=pressure,
            inaccuracy_chance=chance,
            reason=reason,
        )

    # ------------------------------------------------------------------
    # Move selection
    # ------------------------------------------------------------------

    def select_move(
        self,
        candidates: Sequence[CandidateMove],
        decision: ScalingDecision,
        rng: random.Random | None = None,
    ) -> MoveChoice | None:
        """Pick which of the engine's candidate moves to actually play.

        The best move is played unless the decision calls for an inaccuracy and
        a *plausible* alternative exists: one of the top
        :attr:`DynamicScalingConfig.candidate_pool` candidates that concedes
        between ``min_inaccuracy_loss`` and ``max_inaccuracy_loss`` pawns. That
        upper bound is what keeps the mistake positional -- a move that drops a
        piece costs far more than the cap and can never be selected.

        Args:
            candidates: Engine candidate lines. Evaluations are from the
                engine's point of view.
            decision: The decision returned by :meth:`decide` for this move.
            rng: Random source, injectable for deterministic tests.

        Returns:
            The chosen move, or ``None`` when there are no candidates.
        """
        ranked = sorted(
            candidates,
            key=lambda candidate: (
                candidate.evaluation if candidate.evaluation is not None else -math.inf
            ),
            reverse=True,
        )
        if not ranked:
            return None

        best = ranked[0]
        if decision.inaccuracy_chance <= 0.0:
            return MoveChoice(
                candidate=best,
                is_inaccuracy=False,
                eval_loss=0.0,
                reason="playing the best move",
            )

        generator = rng or _RNG
        if generator.random() >= decision.inaccuracy_chance:
            return MoveChoice(
                candidate=best,
                is_inaccuracy=False,
                eval_loss=0.0,
                reason="inaccuracy roll missed; playing the best move",
            )

        plausible = self._plausible_alternatives(ranked)
        if not plausible:
            return MoveChoice(
                candidate=best,
                is_inaccuracy=False,
                eval_loss=0.0,
                reason="no plausible inaccuracy available; playing the best move",
            )

        candidate, loss = plausible[generator.randrange(len(plausible))]
        return MoveChoice(
            candidate=candidate,
            is_inaccuracy=True,
            eval_loss=round(loss, 3),
            reason=f"conceding {loss:.2f} pawns to keep the game close",
        )

    def _plausible_alternatives(
        self, ranked: Sequence[CandidateMove]
    ) -> list[tuple[CandidateMove, float]]:
        """Return the alternatives that are weaker but not blunders."""
        best = ranked[0]
        if best.evaluation is None:
            return []

        alternatives: list[tuple[CandidateMove, float]] = []
        for candidate in ranked[1 : self.config.candidate_pool]:
            if candidate.evaluation is None:
                continue
            loss = best.evaluation - candidate.evaluation
            if (
                self.config.min_inaccuracy_loss
                <= loss
                <= self.config.max_inaccuracy_loss
            ):
                alternatives.append((candidate, loss))
        return alternatives

    # ------------------------------------------------------------------
    # Internals
    # ------------------------------------------------------------------

    def _pressure(self, advantage: float) -> float:
        """Map the smoothed advantage to an intervention in ``[-1, 1]``.

        Zero inside the target window, ramping linearly to ``-1`` at
        ``struggle_threshold`` and ``+1`` at ``dominating_threshold``.
        """
        config = self.config
        if advantage < config.target_low:
            span = config.target_low - config.struggle_threshold
            return -_clamp((config.target_low - advantage) / span, 0.0, 1.0)
        if advantage > config.target_high:
            span = config.dominating_threshold - config.target_high
            return _clamp((advantage - config.target_high) / span, 0.0, 1.0)
        return 0.0

    def _step_depth(self, baseline: EngineParams, pressure: float) -> int:
        """Move the depth one bounded step towards its target."""
        floor = min(baseline.depth, self.config.min_depth)
        ceiling = max(baseline.depth, self.config.max_depth)
        if pressure < 0:
            target = baseline.depth + pressure * (baseline.depth - floor)
        else:
            target = baseline.depth + pressure * (ceiling - baseline.depth)

        current = self._depth if self._depth is not None else float(baseline.depth)
        step = float(self.config.max_depth_step)
        self._depth = _clamp(
            _clamp(target, current - step, current + step), floor, ceiling
        )
        return int(round(self._depth))

    def _step_skill(self, baseline: EngineParams, pressure: float) -> int:
        """Move the skill level one bounded step towards its target."""
        floor = min(baseline.skill_level, self.config.min_skill_level)
        ceiling = max(baseline.skill_level, self.config.max_skill_level)
        if pressure < 0:
            target = baseline.skill_level + pressure * (baseline.skill_level - floor)
        else:
            target = baseline.skill_level + pressure * (ceiling - baseline.skill_level)

        current = self._skill if self._skill is not None else float(baseline.skill_level)
        step = float(self.config.max_skill_step)
        self._skill = _clamp(
            _clamp(target, current - step, current + step), floor, ceiling
        )
        return int(round(self._skill))


class DynamicScalingRegistry:
    """Keeps one :class:`DynamicEloController` per live game session.

    Workers serve many games, so the per-game smoothing state lives here rather
    than on the worker. The registry is bounded: the least recently used
    session is dropped once ``max_sessions`` is exceeded, which keeps abandoned
    games from accumulating.
    """

    def __init__(
        self,
        config: DynamicScalingConfig | None = None,
        *,
        max_sessions: int = 512,
    ) -> None:
        """Initialize the registry.

        Args:
            config: Configuration handed to every controller it creates.
            max_sessions: Maximum number of sessions tracked at once.
        """
        if max_sessions < 1:
            raise ValueError("max_sessions must be >= 1")
        self.config = config or DynamicScalingConfig()
        self.max_sessions = max_sessions
        self._controllers: OrderedDict[str, DynamicEloController] = OrderedDict()

    def controller_for(self, session_id: str) -> DynamicEloController:
        """Return the controller for *session_id*, creating one if needed."""
        controller = self._controllers.get(session_id)
        if controller is None:
            controller = DynamicEloController(self.config)
            self._controllers[session_id] = controller
            if len(self._controllers) > self.max_sessions:
                evicted, _ = self._controllers.popitem(last=False)
                logger.debug("Evicted dynamic scaling state for session %s", evicted)
        else:
            self._controllers.move_to_end(session_id)
        return controller

    def release(self, session_id: str) -> None:
        """Drop the controller for a finished game."""
        self._controllers.pop(session_id, None)

    def __len__(self) -> int:
        """Number of sessions currently tracked."""
        return len(self._controllers)

    def __contains__(self, session_id: object) -> bool:
        """Whether a session is currently tracked."""
        return session_id in self._controllers
