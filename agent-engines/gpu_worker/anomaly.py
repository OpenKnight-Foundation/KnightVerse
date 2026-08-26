from __future__ import annotations

from collections import defaultdict, deque
from collections.abc import Iterable
from datetime import datetime, timedelta, timezone
from enum import Enum
from typing import Any

from pydantic import BaseModel, Field, field_validator, model_validator

from gpu_worker.models import AnalysisRequest


class AnomalyRiskLevel(str, Enum):
    """Risk levels for bot-farm anomaly reports."""

    LOW = "low"
    MODERATE = "moderate"
    HIGH = "high"
    CRITICAL = "critical"


class BotFarmDetectionConfig(BaseModel):
    """Configuration for bot-farm anomaly heuristics."""

    sliding_window_seconds: int = Field(default=300, ge=1)
    bucket_size_seconds: int = Field(default=10, ge=1)
    shared_hash_actor_threshold: int = Field(default=5, ge=2)
    burst_actor_threshold: int = Field(default=8, ge=2)
    repeated_profile_actor_threshold: int = Field(default=5, ge=2)
    actor_rate_threshold: int = Field(default=20, ge=1)
    max_events_retained: int = Field(default=10000, ge=1)
    moderate_risk_score: int = Field(default=25, ge=0, le=100)
    high_risk_score: int = Field(default=60, ge=0, le=100)
    critical_risk_score: int = Field(default=85, ge=0, le=100)

    @model_validator(mode="after")
    def normalize_values(self) -> BotFarmDetectionConfig:
        """Normalize bucket sizes and validate risk thresholds."""

        if self.bucket_size_seconds > self.sliding_window_seconds:
            self.bucket_size_seconds = self.sliding_window_seconds
        if not (
            self.moderate_risk_score
            <= self.high_risk_score
            <= self.critical_risk_score
        ):
            raise ValueError("risk score thresholds must be ordered")
        return self


class BotFarmEvent(BaseModel):
    """Normalized request telemetry used by the anomaly detector."""

    request_id: str
    actor_id: str | None = None
    session_id: str | None = None
    ip_hash: str | None = None
    device_hash: str | None = None
    fen: str
    depth: int | None = None
    time_limit_ms: int | None = None
    search_moves: list[str] | None = None
    num_pv: int = Field(default=1, ge=1)
    created_at: datetime

    @field_validator("actor_id", "session_id", "ip_hash", "device_hash")
    @classmethod
    def normalize_optional_identifier(cls, value: str | None) -> str | None:
        """Normalize optional hashed identifiers."""

        if value is None:
            return None
        normalized = value.strip()
        return normalized or None

    @field_validator("created_at")
    @classmethod
    def normalize_created_at(cls, value: datetime) -> datetime:
        """Ensure timestamps are timezone-aware UTC values."""

        if value.tzinfo is None:
            return value.replace(tzinfo=timezone.utc)
        return value.astimezone(timezone.utc)

    @classmethod
    def from_request(cls, request: AnalysisRequest) -> BotFarmEvent:
        """Build a telemetry event from an analysis request."""

        return cls(
            request_id=request.id,
            actor_id=request.actor_id,
            session_id=request.session_id,
            ip_hash=request.ip_hash,
            device_hash=request.device_hash,
            fen=request.fen,
            depth=request.depth,
            time_limit_ms=request.time_limit_ms,
            search_moves=list(request.search_moves) if request.search_moves is not None else None,
            num_pv=request.num_pv,
            created_at=request.created_at,
        )

    def profile_key(self) -> tuple[Any, ...]:
        """Return the normalized search profile key."""

        return (
            self.fen,
            tuple(self.search_moves or ()),
            self.depth,
            self.time_limit_ms,
            self.num_pv,
        )


class BotFarmFinding(BaseModel):
    """Single anomaly finding with operator-readable evidence."""

    risk_level: AnomalyRiskLevel
    score: int = Field(ge=0, le=100)
    reason: str
    affected_actor_ids: list[str] = Field(default_factory=list)
    evidence: dict[str, Any] = Field(default_factory=dict)


class BotFarmReport(BaseModel):
    """Aggregate bot-farm anomaly report for a telemetry window."""

    risk_level: AnomalyRiskLevel
    score: int = Field(ge=0, le=100)
    findings: list[BotFarmFinding] = Field(default_factory=list)
    window_start: datetime | None = None
    window_end: datetime | None = None
    window_seconds: int
    bucket_size_seconds: int
    event_count: int
    distinct_actor_count: int


class BotFarmAnomalyDetector:
    """Passive detector for coordinated analysis-request anomalies."""

    def __init__(self, config: BotFarmDetectionConfig | None = None) -> None:
        self.config = config or BotFarmDetectionConfig()
        self._events: deque[BotFarmEvent] = deque()
        self._latest_event_at: datetime | None = None

    def record_request(self, request: AnalysisRequest) -> BotFarmReport:
        """Record an analysis request and return the current anomaly report."""

        return self.record_event(BotFarmEvent.from_request(request))

    def record_event(self, event: BotFarmEvent) -> BotFarmReport:
        """Record a normalized telemetry event and return the current report."""

        normalized = BotFarmEvent.model_validate(event)
        self._events.append(normalized)
        if self._latest_event_at is None or normalized.created_at > self._latest_event_at:
            self._latest_event_at = normalized.created_at
        self._prune(reference_time=self._latest_event_at)
        return self.analyze()

    def analyze(self, events: Iterable[BotFarmEvent] | None = None) -> BotFarmReport:
        """Analyze retained or supplied events using bounded linear heuristics."""

        if events is None:
            current_events = list(self._events)
        else:
            current_events = [BotFarmEvent.model_validate(event) for event in events]
            current_events = self._windowed_events(current_events)

        if not current_events:
            return self._build_report([], [])

        findings: list[BotFarmFinding] = []
        findings.extend(self._find_shared_hashes(current_events, "ip_hash"))
        findings.extend(self._find_shared_hashes(current_events, "device_hash"))
        findings.extend(self._find_synchronized_bursts(current_events))
        findings.extend(self._find_repeated_profiles(current_events))
        findings.extend(self._find_actor_rate_spikes(current_events))
        findings.sort(key=lambda finding: (-finding.score, finding.reason))
        return self._build_report(current_events, findings)

    def reset(self) -> None:
        """Clear retained telemetry events."""

        self._events.clear()
        self._latest_event_at = None

    @property
    def retained_event_count(self) -> int:
        """Number of events currently retained by the detector."""

        return len(self._events)

    def _prune(self, reference_time: datetime | None = None) -> None:
        if reference_time is None:
            reference_time = max((event.created_at for event in self._events), default=None)
        if reference_time is not None:
            cutoff = reference_time - timedelta(seconds=self.config.sliding_window_seconds)
            while self._events and self._events[0].created_at < cutoff:
                self._events.popleft()
        while len(self._events) > self.config.max_events_retained:
            self._events.popleft()

    def _windowed_events(self, events: list[BotFarmEvent]) -> list[BotFarmEvent]:
        if not events:
            return []
        reference_time = max(event.created_at for event in events)
        cutoff = reference_time - timedelta(seconds=self.config.sliding_window_seconds)
        retained = [event for event in events if event.created_at >= cutoff]
        if len(retained) > self.config.max_events_retained:
            retained = retained[-self.config.max_events_retained :]
        return retained

    def _find_shared_hashes(
        self, events: list[BotFarmEvent], hash_field: str
    ) -> list[BotFarmFinding]:
        actor_sets: dict[str, set[str]] = defaultdict(set)
        event_counts: dict[str, int] = defaultdict(int)
        for event in events:
            hash_value = getattr(event, hash_field)
            if not hash_value:
                continue
            event_counts[hash_value] += 1
            if event.actor_id:
                actor_sets[hash_value].add(event.actor_id)

        findings: list[BotFarmFinding] = []
        for hash_value, actor_ids in actor_sets.items():
            actor_count = len(actor_ids)
            if actor_count < self.config.shared_hash_actor_threshold:
                continue
            score = min(100, 45 + actor_count * 5)
            findings.append(
                BotFarmFinding(
                    risk_level=self._risk_for_score(score),
                    score=score,
                    reason=f"many actors share the same {hash_field}",
                    affected_actor_ids=sorted(actor_ids),
                    evidence={
                        "hash_type": hash_field,
                        "distinct_actor_count": actor_count,
                        "event_count": event_counts[hash_value],
                        "threshold": self.config.shared_hash_actor_threshold,
                    },
                )
            )
        return findings

    def _find_synchronized_bursts(
        self, events: list[BotFarmEvent]
    ) -> list[BotFarmFinding]:
        bucket_actors: dict[int, set[str]] = defaultdict(set)
        bucket_events: dict[int, int] = defaultdict(int)
        for event in events:
            bucket = self._bucket_for(event.created_at)
            bucket_events[bucket] += 1
            if event.actor_id:
                bucket_actors[bucket].add(event.actor_id)

        findings: list[BotFarmFinding] = []
        for bucket, actor_ids in bucket_actors.items():
            actor_count = len(actor_ids)
            if actor_count < self.config.burst_actor_threshold:
                continue
            score = min(100, 30 + actor_count * 4)
            findings.append(
                BotFarmFinding(
                    risk_level=self._risk_for_score(score),
                    score=score,
                    reason="many actors submitted requests in the same time bucket",
                    affected_actor_ids=sorted(actor_ids),
                    evidence={
                        "bucket_start": datetime.fromtimestamp(
                            bucket * self.config.bucket_size_seconds, tz=timezone.utc
                        ).isoformat(),
                        "bucket_size_seconds": self.config.bucket_size_seconds,
                        "distinct_actor_count": actor_count,
                        "event_count": bucket_events[bucket],
                        "threshold": self.config.burst_actor_threshold,
                    },
                )
            )
        return findings

    def _find_repeated_profiles(
        self, events: list[BotFarmEvent]
    ) -> list[BotFarmFinding]:
        profile_actors: dict[tuple[Any, ...], set[str]] = defaultdict(set)
        profile_events: dict[tuple[Any, ...], int] = defaultdict(int)
        for event in events:
            key = event.profile_key()
            profile_events[key] += 1
            if event.actor_id:
                profile_actors[key].add(event.actor_id)

        findings: list[BotFarmFinding] = []
        for key, actor_ids in profile_actors.items():
            actor_count = len(actor_ids)
            if actor_count < self.config.repeated_profile_actor_threshold:
                continue
            score = min(100, 30 + actor_count * 6)
            fen, search_moves, depth, time_limit_ms, num_pv = key
            findings.append(
                BotFarmFinding(
                    risk_level=self._risk_for_score(score),
                    score=score,
                    reason="many actors repeated the same position and search profile",
                    affected_actor_ids=sorted(actor_ids),
                    evidence={
                        "distinct_actor_count": actor_count,
                        "event_count": profile_events[key],
                        "threshold": self.config.repeated_profile_actor_threshold,
                        "fen": fen,
                        "search_moves": list(search_moves),
                        "depth": depth,
                        "time_limit_ms": time_limit_ms,
                        "num_pv": num_pv,
                    },
                )
            )
        return findings

    def _find_actor_rate_spikes(
        self, events: list[BotFarmEvent]
    ) -> list[BotFarmFinding]:
        actor_counts: dict[str, int] = defaultdict(int)
        for event in events:
            if event.actor_id:
                actor_counts[event.actor_id] += 1

        findings: list[BotFarmFinding] = []
        for actor_id, count in actor_counts.items():
            if count < self.config.actor_rate_threshold:
                continue
            score = min(100, 25 + count * 3)
            findings.append(
                BotFarmFinding(
                    risk_level=self._risk_for_score(score),
                    score=score,
                    reason="actor request rate exceeded the sliding-window threshold",
                    affected_actor_ids=[actor_id],
                    evidence={
                        "actor_request_count": count,
                        "threshold": self.config.actor_rate_threshold,
                        "window_seconds": self.config.sliding_window_seconds,
                    },
                )
            )
        return findings

    def _build_report(
        self, events: list[BotFarmEvent], findings: list[BotFarmFinding]
    ) -> BotFarmReport:
        if findings:
            score = min(100, max(finding.score for finding in findings) + min(15, 5 * (len(findings) - 1)))
        else:
            score = 0
        distinct_actors = {event.actor_id for event in events if event.actor_id}
        return BotFarmReport(
            risk_level=self._risk_for_score(score),
            score=score,
            findings=findings,
            window_start=min((event.created_at for event in events), default=None),
            window_end=max((event.created_at for event in events), default=None),
            window_seconds=self.config.sliding_window_seconds,
            bucket_size_seconds=self.config.bucket_size_seconds,
            event_count=len(events),
            distinct_actor_count=len(distinct_actors),
        )

    def _bucket_for(self, timestamp: datetime) -> int:
        return int(timestamp.timestamp()) // self.config.bucket_size_seconds

    def _risk_for_score(self, score: int) -> AnomalyRiskLevel:
        if score >= self.config.critical_risk_score:
            return AnomalyRiskLevel.CRITICAL
        if score >= self.config.high_risk_score:
            return AnomalyRiskLevel.HIGH
        if score >= self.config.moderate_risk_score:
            return AnomalyRiskLevel.MODERATE
        return AnomalyRiskLevel.LOW
