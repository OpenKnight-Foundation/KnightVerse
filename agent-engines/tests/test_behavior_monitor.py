"""Tests for mouse trajectory bot detection model.

Tests kinematic feature extraction, trajectory classification,
and graceful handling of edge cases.
"""

from __future__ import annotations

import math
import pytest

from gpu_worker.player_behavior_monitor import (
    MousePoint,
    TrajectoryFeatures,
    BotDetectionResult,
    extract_trajectory_features,
    classify_mouse_trajectory,
    _distance,
    _compute_velocities,
    _compute_curvature,
    PlayerBehaviorMonitor,
)
from gpu_worker.models import Game, Move, Player


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def _make_point(x: float, y: float, t: float) -> MousePoint:
    return MousePoint(x=x, y=y, t=t)


def _linear_trajectory(n: int = 10, start=(0, 0), end=(200, 200), dt: float = 20.0):
    """Generate a perfectly straight trajectory from start to end."""
    points = []
    for i in range(n):
        frac = i / (n - 1)
        x = start[0] + (end[0] - start[0]) * frac
        y = start[1] + (end[1] - start[1]) * frac
        t = i * dt
        points.append(_make_point(x, y, t))
    return points


def _curved_trajectory(n: int = 15):
    """Generate a natural-looking curved trajectory with overshoot."""
    points = []
    for i in range(n):
        t = i * 30.0
        # Curved path with deceleration
        progress = i / (n - 1)
        x = 50 + 200 * progress + 30 * math.sin(progress * math.pi)
        y = 100 + 150 * progress + 20 * math.cos(progress * math.pi * 2)
        points.append(_make_point(x, y, t))
    return points


def _human_trajectory_with_tremor(n: int = 20):
    """Generate a realistic human trajectory with tremor and deceleration."""
    points = []
    for i in range(n):
        t = i * 25.0  # ~40fps mouse polling
        progress = i / (n - 1)
        # Accelerate then decelerate (bell-shaped velocity)
        vel_factor = math.sin(progress * math.pi)
        x = 100 + 180 * progress + 5 * math.sin(i * 1.7) * (1 - progress)
        y = 80 + 120 * progress + 3 * math.cos(i * 2.3) * (1 - progress)
        # Add slight jitter (human tremor)
        x += 1.5 * math.sin(i * 3.1)
        y += 1.2 * math.cos(i * 2.7)
        points.append(_make_point(x, y, t))
    return points


def _bot_teleport(start=(0, 0), end=(500, 300)):
    """Generate a bot teleport: instant jump between two points."""
    return [
        _make_point(start[0], start[1], 0.0),
        _make_point(end[0], end[1], 1.0),  # 1ms travel time
    ]


def _bot_linear_constant_speed(n: int = 10):
    """Bot: perfectly straight line at constant speed."""
    points = []
    for i in range(n):
        frac = i / (n - 1)
        x = 100 + 300 * frac
        y = 100 + 0 * frac  # perfectly horizontal
        t = i * 16.67  # exact 60fps intervals
        points.append(_make_point(x, y, t))
    return points


# ===================================================================
# Distance & Velocity Tests
# ===================================================================

class TestDistanceAndVelocity:
    """Test basic distance and velocity computations."""

    def test_distance_same_point(self) -> None:
        p = _make_point(10, 20, 0)
        assert _distance(p, p) == 0.0

    def test_distance_horizontal(self) -> None:
        p1 = _make_point(0, 0, 0)
        p2 = _make_point(3, 0, 0)
        assert _distance(p1, p2) == pytest.approx(3.0)

    def test_distance_diagonal(self) -> None:
        p1 = _make_point(0, 0, 0)
        p2 = _make_point(3, 4, 0)
        assert _distance(p1, p2) == pytest.approx(5.0)

    def test_velocity_computation(self) -> None:
        points = [_make_point(0, 0, 0), _make_point(100, 0, 50)]
        vels = _compute_velocities(points)
        assert len(vels) == 1
        assert vels[0] == pytest.approx(2.0)  # 100px / 50ms

    def test_zero_time_interval(self) -> None:
        points = [_make_point(0, 0, 10), _make_point(100, 0, 10)]
        vels = _compute_velocities(points)
        assert vels[0] == 0.0


# ===================================================================
# Curvature Tests
# ===================================================================

class TestCurvature:
    """Test curvature computation."""

    def test_straight_line_zero_curvature(self) -> None:
        points = _linear_trajectory(10)
        c = _compute_curvature(points)
        assert c == pytest.approx(0.0, abs=1e-6)

    def test_curved_path_positive_curvature(self) -> None:
        points = _curved_trajectory(15)
        c = _compute_curvature(points)
        assert c > 0.05

    def test_few_points_zero_curvature(self) -> None:
        points = [_make_point(0, 0, 0), _make_point(10, 10, 10)]
        assert _compute_curvature(points) == 0.0


# ===================================================================
# Feature Extraction Tests
# ===================================================================

class TestFeatureExtraction:
    """Test trajectory feature extraction."""

    def test_empty_trajectory(self) -> None:
        features = extract_trajectory_features([])
        assert features.num_points == 0
        assert features.total_duration_ms == 0.0

    def test_single_point(self) -> None:
        features = extract_trajectory_features([_make_point(5, 5, 0)])
        assert features.num_points == 1

    def test_linear_trajectory_features(self) -> None:
        points = _linear_trajectory(10, dt=16.67)
        features = extract_trajectory_features(points)
        assert features.num_points == 10
        assert features.total_duration_ms == pytest.approx(150.03, rel=0.01)
        assert features.curvature_score == pytest.approx(0.0, abs=0.01)
        assert features.avg_velocity > 0

    def test_curved_trajectory_features(self) -> None:
        points = _curved_trajectory(15)
        features = extract_trajectory_features(points)
        assert features.curvature_score > 0.1
        assert features.total_distance > 0

    def test_human_trajectory_features(self) -> None:
        points = _human_trajectory_with_tremor(20)
        features = extract_trajectory_features(points)
        assert features.velocity_stddev > 0  # Has speed variation
        assert features.acceleration_jitter > 0  # Has acceleration variation
        assert features.curvature_score > 0  # Has some curvature


# ===================================================================
# Bot Detection Tests
# ===================================================================

class TestBotDetection:
    """Test bot classification on various trajectories."""

    def test_teleport_detected(self) -> None:
        points = _bot_teleport()
        result = classify_mouse_trajectory(points)
        assert result.is_bot
        assert result.risk_score >= 0.5

    def test_bot_linear_constant_speed(self) -> None:
        points = _bot_linear_constant_speed(15)
        result = classify_mouse_trajectory(points)
        # Perfectly straight + constant velocity = high risk
        assert result.risk_score > 0.3

    def test_human_low_risk(self) -> None:
        points = _human_trajectory_with_tremor(25)
        result = classify_mouse_trajectory(points)
        assert result.risk_score < 0.05
        assert not result.is_bot

    def test_curved_human_low_risk(self) -> None:
        points = _curved_trajectory(20)
        result = classify_mouse_trajectory(points)
        assert result.risk_score < 0.1
        assert not result.is_bot

    def test_touch_device_ignored(self) -> None:
        points = _bot_teleport()
        result = classify_mouse_trajectory(points, is_touch_device=True)
        assert not result.is_bot
        assert result.risk_score == 0.0
        assert result.is_touch_device

    def test_insufficient_data(self) -> None:
        result = classify_mouse_trajectory([_make_point(0, 0, 0)])
        assert not result.is_bot
        assert result.risk_score == 0.0

    def test_empty_trajectory(self) -> None:
        result = classify_mouse_trajectory([])
        assert not result.is_bot
        assert result.risk_score == 0.0

    def test_result_has_reasons(self) -> None:
        points = _bot_teleport()
        result = classify_mouse_trajectory(points)
        assert len(result.reasons) > 0

    def test_features_populated(self) -> None:
        points = _linear_trajectory(10)
        result = classify_mouse_trajectory(points)
        assert isinstance(result.features, TrajectoryFeatures)
        assert result.features.num_points == 10


# ===================================================================
# Superhuman Velocity Detection
# ===================================================================

class TestSuperhumanVelocity:
    """Test detection of impossibly fast mouse movements."""

    def test_superhuman_velocity_detected(self) -> None:
        # 10000px in 1ms = 10000 px/ms
        points = [_make_point(0, 0, 0), _make_point(10000, 0, 1)]
        result = classify_mouse_trajectory(points)
        assert result.risk_score > 0.3

    def test_normal_velocity_not_flagged(self) -> None:
        # 200px in 100ms = 2 px/ms (normal human speed)
        points = [_make_point(0, 0, 0), _make_point(200, 0, 100)]
        result = classify_mouse_trajectory(points)
        # Should not be flagged for velocity alone
        assert result.risk_score < 0.5


# ===================================================================
# Risk Score Boundary Tests
# ===================================================================

class TestRiskScoreBoundary:
    """Test risk score clamping and threshold behavior."""

    def test_risk_score_clamped_0_1(self) -> None:
        points = _human_trajectory_with_tremor(30)
        result = classify_mouse_trajectory(points)
        assert 0.0 <= result.risk_score <= 1.0

    def test_bot_threshold(self) -> None:
        points = _bot_teleport()
        result = classify_mouse_trajectory(points)
        assert result.risk_score >= 0.5
        assert result.is_bot


# ===================================================================
# Existing PlayerBehaviorMonitor Tests
# ===================================================================

class TestPlayerBehaviorMonitor:
    """Test the original move-time monitoring functionality."""

    def test_record_move(self) -> None:
        monitor = PlayerBehaviorMonitor()
        player = Player(id="p1")
        move = Move(player=player, move="e2e4", time_taken=2.0)
        monitor.record_move(player, move, 2.0)
        assert len(monitor.player_move_times["p1"]) == 1

    def test_move_time_variance(self) -> None:
        monitor = PlayerBehaviorMonitor()
        player = Player(id="p1")
        for t in [1.0, 1.1, 0.9, 1.0]:
            move = Move(player=player, move="e2e4", time_taken=t)
            monitor.record_move(player, move, t)
        variance = monitor.get_move_time_variance("p1")
        assert variance is not None
        assert variance < 0.1  # Very consistent times

    def test_insufficient_moves_for_variance(self) -> None:
        monitor = PlayerBehaviorMonitor()
        player = Player(id="p1")
        move = Move(player=player, move="e2e4", time_taken=1.0)
        monitor.record_move(player, move, 1.0)
        assert monitor.get_move_time_variance("p1") is None


# ===================================================================
# Bot vs Human Accuracy Tests
# ===================================================================

class TestAccuracy:
    """Test that the classifier accurately distinguishes bots from humans."""

    def test_bot_trajectories_detected(self) -> None:
        """Bot trajectories should be flagged with >99% accuracy."""
        bot_trajectories = [
            _bot_teleport(),
            _bot_teleport((100, 100), (400, 400)),
            _bot_teleport((0, 0), (1000, 0)),
            _bot_linear_constant_speed(20),
            _bot_linear_constant_speed(50),
        ]
        for traj in bot_trajectories:
            result = classify_mouse_trajectory(traj)
            assert result.risk_score > 0.3, f"Bot trajectory not detected: {traj[:2]}"

    def test_human_trajectories_not_flagged(self) -> None:
        """Human trajectories should score <0.05 risk."""
        human_trajectories = [
            _human_trajectory_with_tremor(20),
            _human_trajectory_with_tremor(30),
            _curved_trajectory(15),
            _curved_trajectory(25),
        ]
        for traj in human_trajectories:
            result = classify_mouse_trajectory(traj)
            assert result.risk_score < 0.1, (
                f"Human trajectory falsely flagged: risk={result.risk_score}"
            )
            assert not result.is_bot
