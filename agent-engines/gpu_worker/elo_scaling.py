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
"""

from __future__ import annotations

import bisect
import logging
from dataclasses import dataclass
from typing import Final

logger = logging.getLogger("KnightVerse.EloScaling")


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
