"""Player behavior monitoring for bot detection.

Tracks player move times and mouse trajectory telemetry to detect
automated bot interactions via kinematic feature analysis.
"""

from __future__ import annotations

import math
import statistics
from collections import defaultdict
from dataclasses import dataclass, field
from typing import List, Optional

from gpu_worker.models import Game, Move, Player



# ---------------------------------------------------------------------------
# Mouse trajectory telemetry types
# ---------------------------------------------------------------------------

@dataclass
class MousePoint:
    """A single point in a mouse trajectory."""
    x: float
    y: float
    t: float  # timestamp in milliseconds


@dataclass
class TrajectoryFeatures:
    """Extracted kinematic features from a mouse trajectory."""
    total_duration_ms: float
    total_distance: float
    avg_velocity: float
    max_velocity: float
    velocity_stddev: float
    avg_acceleration: float
    acceleration_jitter: float
    curvature_score: float  # 0.0 = straight, 1.0 = very curved
    click_dwell_time_ms: float
    num_points: int


@dataclass
class BotDetectionResult:
    """Result of bot detection analysis on mouse telemetry."""
    risk_score: float  # 0.0 = human, 1.0 = bot
    is_bot: bool
    is_touch_device: bool
    features: TrajectoryFeatures
    reasons: List[str] = field(default_factory=list)


# ---------------------------------------------------------------------------
# Velocity & acceleration helpers
# ---------------------------------------------------------------------------

def _distance(p1: MousePoint, p2: MousePoint) -> float:
    """Euclidean distance between two points."""
    return math.sqrt((p2.x - p1.x) ** 2 + (p2.y - p1.y) ** 2)


def _compute_velocities(points: List[MousePoint]) -> List[float]:
    """Compute instantaneous velocities between consecutive points."""
    velocities = []
    for i in range(1, len(points)):
        dt = points[i].t - points[i - 1].t
        if dt <= 0:
            velocities.append(0.0)
        else:
            dist = _distance(points[i - 1], points[i])
            velocities.append(dist / dt)
    return velocities


def _compute_accelerations(velocities: List[float], points: List[MousePoint]) -> List[float]:
    """Compute accelerations from velocity series."""
    accelerations = []
    for i in range(1, len(velocities)):
        dt = points[i + 1].t - points[i].t
        if dt <= 0:
            accelerations.append(0.0)
        else:
            accelerations.append((velocities[i] - velocities[i - 1]) / dt)
    return accelerations


def _compute_curvature(points: List[MousePoint]) -> float:
    """Compute curvature: average deviation from straight-line path."""
    if len(points) < 3:
        return 0.0
    start, end = points[0], points[-1]
    line_length = _distance(start, end)
    if line_length < 1e-6:
        return 0.0
    total_deviation = 0.0
    for p in points[1:-1]:
        dx = end.x - start.x
        dy = end.y - start.y
        num = abs(dy * p.x - dx * p.y + end.x * start.y - end.y * start.x)
        den = math.sqrt(dx * dx + dy * dy)
        if den > 0:
            total_deviation += num / den
    avg_deviation = total_deviation / (len(points) - 2)
    return min(1.0, avg_deviation / max(line_length, 1.0) * 10)


def _compute_dwell_time(points: List[MousePoint], radius: float = 5.0) -> float:
    """Compute time cursor stays within radius of final position."""
    if len(points) < 2:
        return 0.0
    final = points[-1]
    dwell_start = final.t
    for p in reversed(points[:-1]):
        if _distance(p, final) > radius:
            break
        dwell_start = p.t
    return final.t - dwell_start


# ---------------------------------------------------------------------------
# Feature extraction
# ---------------------------------------------------------------------------

def extract_trajectory_features(points: List[MousePoint]) -> TrajectoryFeatures:
    """Extract kinematic features from a mouse trajectory."""
    if len(points) < 2:
        return TrajectoryFeatures(
            total_duration_ms=0.0, total_distance=0.0, avg_velocity=0.0,
            max_velocity=0.0, velocity_stddev=0.0, avg_acceleration=0.0,
            acceleration_jitter=0.0, curvature_score=0.0,
            click_dwell_time_ms=0.0, num_points=len(points),
        )
    duration = points[-1].t - points[0].t
    total_distance = sum(_distance(points[i], points[i + 1]) for i in range(len(points) - 1))
    velocities = _compute_velocities(points)
    accelerations = _compute_accelerations(velocities, points)
    avg_vel = statistics.mean(velocities) if velocities else 0.0
    max_vel = max(velocities) if velocities else 0.0
    vel_std = statistics.stdev(velocities) if len(velocities) >= 2 else 0.0
    avg_accel = statistics.mean(accelerations) if accelerations else 0.0
    accel_jitter = statistics.stdev(accelerations) if len(accelerations) >= 2 else 0.0
    curvature = _compute_curvature(points)
    dwell_time = _compute_dwell_time(points)
    return TrajectoryFeatures(
        total_duration_ms=duration, total_distance=total_distance,
        avg_velocity=avg_vel, max_velocity=max_vel, velocity_stddev=vel_std,
        avg_acceleration=avg_accel, acceleration_jitter=accel_jitter,
        curvature_score=curvature, click_dwell_time_ms=dwell_time,
        num_points=len(points),
    )


# ---------------------------------------------------------------------------
# Bot detection classifier
# ---------------------------------------------------------------------------

def classify_mouse_trajectory(
    points: List[MousePoint],
    is_touch_device: bool = False,
) -> BotDetectionResult:
    """Classify a mouse trajectory as human or bot.

    Analyzes kinematic features to detect synthetic mouse movements.
    Touch events are gracefully ignored (low risk).
    """
    if is_touch_device or len(points) < 2:
        features = extract_trajectory_features(points)
        return BotDetectionResult(
            risk_score=0.0, is_bot=False, is_touch_device=is_touch_device,
            features=features,
            reasons=["Touch device - analysis skipped"] if is_touch_device else ["Insufficient data"],
        )
    features = extract_trajectory_features(points)
    risk_score = 0.0
    reasons = []
    # Instantaneous teleport
    if features.total_distance > 50 and features.total_duration_ms < 5:
        risk_score += 0.5
        reasons.append(f"Instantaneous teleport: {features.total_distance:.0f}px in {features.total_duration_ms:.1f}ms")
    # Perfectly straight line
    if features.curvature_score < 0.01 and features.num_points > 5:
        risk_score += 0.25
        reasons.append(f"Perfectly straight trajectory (curvature={features.curvature_score:.4f})")
    # Zero velocity variance
    if features.velocity_stddev < 0.01 and features.avg_velocity > 0.1:
        risk_score += 0.2
        reasons.append(f"Constant velocity (stddev={features.velocity_stddev:.4f})")
    # Robotic click dwell (only flag with strong evidence)
    if (features.click_dwell_time_ms == 0.0 and features.total_duration_ms > 300
            and features.num_points > 25 and features.velocity_stddev < 0.01):
        risk_score += 0.15
        reasons.append("Zero click dwell time (instant click)")
    # Superhuman velocity
    if features.max_velocity > 5000:
        risk_score += 0.3
        reasons.append(f"Superhuman velocity: {features.max_velocity:.0f} px/ms")
    # Natural human features reduce risk
    if features.curvature_score > 0.1:
        risk_score -= 0.1
    if features.acceleration_jitter > 0.01:
        risk_score -= 0.05
    if features.velocity_stddev > 0.5:
        risk_score -= 0.1
    risk_score = max(0.0, min(1.0, risk_score))
    return BotDetectionResult(
        risk_score=risk_score, is_bot=risk_score >= 0.5,
        is_touch_device=False, features=features, reasons=reasons,
    )


# ---------------------------------------------------------------------------
# Original move-time monitoring (preserved)
# ---------------------------------------------------------------------------

class PlayerBehaviorMonitor:
    """Tracks player move times and decisions to detect suspicious patterns."""

    def __init__(self) -> None:
        self.player_move_times: dict[str, list[float]] = defaultdict(list)
        self.player_move_decisions: dict[str, list[Move]] = defaultdict(list)

    def record_move(self, player: Player, move: Move, move_time: float) -> None:
        """Records a single move for a player."""
        self.player_move_times[player.id].append(move_time)
        self.player_move_decisions[player.id].append(move)

    def get_move_time_variance(self, player_id: str) -> float | None:
        """Calculates the variance of move times for a player."""
        move_times = self.player_move_times.get(player_id)
        if not move_times or len(move_times) < 2:
            return None
        return statistics.variance(move_times)


class OfflineAnalysisService:
    """Analyzes recent rated games to detect cheating."""

    def __init__(self, stockfish_bridge, confidence_threshold: float = 0.85) -> None:
        self.stockfish_bridge = stockfish_bridge
        self.confidence_threshold = confidence_threshold
        self.player_monitor = PlayerBehaviorMonitor()

    def analyze_games(self, games: list[Game]) -> list[str]:
        """Analyzes a list of games and returns a list of flagged player IDs."""
        flagged_players = []
        for game in games:
            for move in game.moves:
                self.player_monitor.record_move(move.player, move, move.time_taken)
            for player in [game.white_player, game.black_player]:
                if self._is_suspicious(player, game):
                    flagged_players.append(player.id)
        return flagged_players

    def _is_suspicious(self, player: Player, game: Game) -> bool:
        """Checks if a player's behavior in a game is suspicious."""
        move_time_variance = self.player_monitor.get_move_time_variance(player.id)
        if move_time_variance is not None and move_time_variance < 0.1:
            return True
        player_moves = self.player_monitor.player_move_decisions[player.id]
        stockfish_correlation = self._calculate_stockfish_correlation(player_moves, game)
        return stockfish_correlation > self.confidence_threshold

    def _calculate_stockfish_correlation(self, player_moves: list[Move], game: Game) -> float:
        """Calculates the correlation between player moves and Stockfish's top choices."""
        return 0.0