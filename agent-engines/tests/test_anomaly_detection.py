from __future__ import annotations

from datetime import datetime, timedelta, timezone

from gpu_worker.anomaly import (
    AnomalyRiskLevel,
    BotFarmAnomalyDetector,
    BotFarmDetectionConfig,
    BotFarmEvent,
)
from gpu_worker.models import AnalysisRequest

# Two distinct, valid chess positions (kings only) used to vary or repeat the position under analysis
FEN = "8/8/8/8/8/8/8/K6k w - - 0 1"
ALT_FEN = "8/8/8/8/8/8/K7/7k w - - 0 1"


# Test helper: builds a BotFarmEvent with sensible defaults so each test
# only needs to specify the fields it cares about.
def make_event(
    index: int,
    *,
    created_at: datetime,
    actor_id: str | None = None,
    ip_hash: str | None = None,
    device_hash: str | None = None,
    fen: str = FEN,
    depth: int | None = 12,
    time_limit_ms: int | None = 1000,
    search_moves: list[str] | None = None,
    num_pv: int = 1,
) -> BotFarmEvent:
    return BotFarmEvent(
        # index makes each request ID unique
        request_id=f"request-{index}",
        actor_id=actor_id,
        ip_hash=ip_hash,
        device_hash=device_hash,
        fen=fen,
        depth=depth,
        time_limit_ms=time_limit_ms,
        search_moves=search_moves,
        num_pv=num_pv,
        created_at=created_at,
    )


# Baseline: normal-looking traffic (unique actor/IP/device each, varied positions
# and depths, spaced 20s apart) should produce no findings at all.
def test_clean_traffic_remains_low_risk() -> None:
    detector = BotFarmAnomalyDetector()
    base_time = datetime(2026, 1, 1, tzinfo=timezone.utc)
    events = [
        make_event(
            index,
            created_at=base_time + timedelta(seconds=index * 20),
            actor_id=f"actor-{index}",
            ip_hash=f"ip-{index}",
            device_hash=f"device-{index}",
            # Alternate between the two positions
            fen=FEN if index % 2 == 0 else ALT_FEN,
            depth=10 + index,
        )
        for index in range(4)
    ]

    report = detector.analyze(events)

    assert report.risk_level is AnomalyRiskLevel.LOW
    assert report.score == 0
    assert report.findings == []
    assert report.event_count == 4
    assert report.distinct_actor_count == 4


# Classic bot-farm signal: many different accounts all sharing one IP and one device
def test_shared_ip_and_device_across_many_actors_is_high_risk() -> None:
    detector = BotFarmAnomalyDetector()
    base_time = datetime(2026, 1, 1, tzinfo=timezone.utc)

    report = detector.analyze(
        [
            make_event(
                index,
                created_at=base_time + timedelta(seconds=index),
                actor_id=f"actor-{index}",
                ip_hash="shared-ip-hash",
                device_hash="shared-device-hash",
                depth=10 + index,
            )
            for index in range(8)
        ]
    )

    assert report.risk_level in {AnomalyRiskLevel.HIGH, AnomalyRiskLevel.CRITICAL}
    assert report.score >= 85
    # Both the shared IP and the shared device should be reported separately
    assert any(finding.evidence.get("hash_type") == "ip_hash" for finding in report.findings)
    assert any(finding.evidence.get("hash_type") == "device_hash" for finding in report.findings)
    # Every shared-hash finding should implicate all 8 actors
    assert {len(finding.affected_actor_ids) for finding in report.findings if "hash" in finding.reason} == {8}


# Different actors (and IPs) all requesting the exact same search on the same
# position suggests coordinated automation.
def test_repeated_position_search_profiles_across_actors_are_flagged() -> None:
    detector = BotFarmAnomalyDetector()
    base_time = datetime(2026, 1, 1, tzinfo=timezone.utc)
    events = [
        make_event(
            index,
            created_at=base_time + timedelta(seconds=index),
            actor_id=f"actor-{index}",
            ip_hash=f"ip-{index}",
            # Identical search parameters for every event
            search_moves=["e2e4", "d2d4"],
            depth=16,
            time_limit_ms=1500,
            num_pv=2,
        )
        for index in range(5)
    ]

    report = detector.analyze(events)

    assert report.risk_level is AnomalyRiskLevel.HIGH
    # Exactly one finding should describe the repeated profile
    profile_findings = [finding for finding in report.findings if "same position" in finding.reason]
    assert len(profile_findings) == 1
    assert profile_findings[0].score >= 60
    assert profile_findings[0].evidence["search_moves"] == ["e2e4", "d2d4"]
    assert profile_findings[0].evidence["distinct_actor_count"] == 5


# A single actor sending requests faster than the configured threshold gets flagged
# (at a lower severity than coordinated multi-actor behaviour).
def test_per_actor_request_rate_spikes_are_flagged() -> None:
    # Lower the threshold so 4 requests in a few seconds counts as a spike
    config = BotFarmDetectionConfig(actor_rate_threshold=4)
    detector = BotFarmAnomalyDetector(config)
    base_time = datetime(2026, 1, 1, tzinfo=timezone.utc)

    report = detector.analyze(
        [
            make_event(
                index,
                created_at=base_time + timedelta(seconds=index),
                actor_id="spiky-actor",
                # Distinct IPs/positions/depths so only the rate rule can fire
                ip_hash=f"ip-{index}",
                fen=FEN if index % 2 == 0 else ALT_FEN,
                depth=20 + index,
            )
            for index in range(4)
        ]
    )

    assert report.risk_level is AnomalyRiskLevel.MODERATE
    rate_findings = [finding for finding in report.findings if "request rate" in finding.reason]
    assert len(rate_findings) == 1
    assert rate_findings[0].affected_actor_ids == ["spiky-actor"]
    assert rate_findings[0].evidence["actor_request_count"] == 4


# The detector keeps a rolling window of events: old ones age out, and total
# memory is capped even if many events arrive inside the window.
def test_sliding_window_pruning_and_max_retention_cap() -> None:
    config = BotFarmDetectionConfig(
        sliding_window_seconds=30,
        max_events_retained=3,
        # Raise the thresholds so these events could only trigger findings
        # if stale ones were wrongly kept
        shared_hash_actor_threshold=4,
        burst_actor_threshold=4,
        repeated_profile_actor_threshold=4,
    )
    detector = BotFarmAnomalyDetector(config)
    base_time = datetime(2026, 1, 1, tzinfo=timezone.utc)

    # Four events from different actors sharing one IP, all within the first few seconds
    for index in range(4):
        detector.record_event(
            make_event(
                index,
                created_at=base_time + timedelta(seconds=index),
                actor_id=f"old-actor-{index}",
                ip_hash="old-shared-ip",
                depth=20 + index,
            )
        )
    # A new event 35s later falls outside the 30s window of the earlier ones,
    # so they are pruned and the shared-IP pattern is no longer visible
    stale_report = detector.record_event(
        make_event(
            99,
            created_at=base_time + timedelta(seconds=35),
            actor_id="new-actor",
            ip_hash="new-ip",
            fen=ALT_FEN,
            depth=30,
        )
    )

    assert stale_report.risk_level is AnomalyRiskLevel.LOW
    assert stale_report.event_count == 1
    assert detector.retained_event_count == 1

    # Five more events inside the window exceed max_events_retained (3),
    # so only the 3 most recent should be kept
    for index in range(5):
        detector.record_event(
            make_event(
                100 + index,
                created_at=base_time + timedelta(seconds=40 + index),
                actor_id=f"retained-actor-{index}",
                ip_hash=f"retained-ip-{index}",
                fen=ALT_FEN,
                depth=40 + index,
            )
        )

    # analyze() with no argument evaluates the detector's retained events
    report = detector.analyze()
    assert detector.retained_event_count == 3
    assert report.event_count == 3
    assert report.distinct_actor_count == 3


# Guards against false positives: when telemetry (actor, IP, device) is absent,
# identical events must not be treated as suspicious "shared" identities.
def test_missing_optional_telemetry_does_not_false_positive() -> None:
    detector = BotFarmAnomalyDetector()
    base_time = datetime(2026, 1, 1, tzinfo=timezone.utc)
    events = [
        # Same timestamp, no identifiers: 12 anonymous events
        make_event(index, created_at=base_time, actor_id=None, ip_hash=None, device_hash=None)
        for index in range(12)
    ]

    report = detector.analyze(events)

    assert report.risk_level is AnomalyRiskLevel.LOW
    assert report.score == 0
    assert report.findings == []
    assert report.event_count == 12
    # No actor IDs means zero distinct actors
    assert report.distinct_actor_count == 0


# Verifies the new telemetry fields on AnalysisRequest: they default to None
# (backward compatible), get whitespace-trimmed, and carry through to BotFarmEvent.
def test_analysis_request_telemetry_fields_round_trip_and_remain_optional() -> None:
    # Request with no telemetry, as existing callers would send it
    existing = AnalysisRequest(fen=FEN)
    # Request with padded values to confirm they are stripped
    request = AnalysisRequest(
        fen=f"  {FEN}  ",
        actor_id=" actor-1 ",
        session_id=" session-1 ",
        ip_hash=" ip-hash ",
        device_hash=" device-hash ",
    )

    assert existing.actor_id is None
    assert existing.session_id is None
    assert existing.ip_hash is None
    assert existing.device_hash is None
    assert request.fen == FEN
    assert request.actor_id == "actor-1"
    assert request.session_id == "session-1"
    assert request.ip_hash == "ip-hash"
    assert request.device_hash == "device-hash"

    # Converting the request into a detector event preserves the cleaned values
    event = BotFarmEvent.from_request(request)
    assert event.request_id == request.id
    assert event.actor_id == "actor-1"
    assert event.session_id == "session-1"
    assert event.ip_hash == "ip-hash"
    assert event.device_hash == "device-hash"