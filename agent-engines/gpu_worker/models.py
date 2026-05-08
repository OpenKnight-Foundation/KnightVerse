from __future__ import annotations

from datetime import datetime, timezone
from enum import Enum
import uuid
from typing import Optional, List

import chess
from pydantic import BaseModel, Field, field_validator


class AnalysisRequest(BaseModel):
    """Request payload for a single chess position analysis."""

    id: str = Field(default_factory=lambda: str(uuid.uuid4()))
    fen: str
    depth: Optional[int] = Field(default=None, ge=1)
    time_limit_ms: Optional[int] = Field(default=None, ge=1)
    search_moves: Optional[List[str]] = None
    num_pv: int = Field(default=1, ge=1)
    priority: int = 0
    actor_id: Optional[str] = None
    session_id: Optional[str] = None
    ip_hash: Optional[str] = None
    device_hash: Optional[str] = None
    created_at: datetime = Field(default_factory=lambda: datetime.now(timezone.utc))

    @field_validator("fen")
    @classmethod
    def validate_fen(cls, value: str) -> str:
        """Validate FEN strings using python-chess."""

        normalized = value.strip()
        try:
            chess.Board(normalized)
        except ValueError as exc:
            raise ValueError(f"invalid FEN: {exc}") from exc
        return normalized

    @field_validator("actor_id", "session_id", "ip_hash", "device_hash")
    @classmethod
    def normalize_optional_identifier(cls, value: Optional[str]) -> Optional[str]:
        """Normalize optional telemetry identifiers."""

        if value is None:
            return None
        normalized = value.strip()
        return normalized or None


class AnalysisResult(BaseModel):
    """Normalized analysis result returned by the worker pool."""

    request_id: str
    best_move: str
    evaluation: Optional[float] = None
    depth: Optional[int] = None
    principal_variation: List[str] = Field(default_factory=list)
    nodes_searched: Optional[int] = None
    time_ms: Optional[int] = None
    gpu_utilization: Optional[float] = None


class WorkerStatus(str, Enum):
    """Runtime status of a worker instance."""

    IDLE = "idle"
    BUSY = "busy"
    ERROR = "error"
    SHUTTING_DOWN = "shutting_down"


class WorkerInfo(BaseModel):
    """Snapshot of worker health and utilization."""

    worker_id: str
    status: WorkerStatus
    gpu_device_id: int
    gpu_memory_used_mb: float = 0.0
    gpu_utilization_pct: float = 0.0
    analyses_completed: int = 0
    uptime_seconds: float = 0.0


class NodeInfo(BaseModel):
    """Information about an orchestrator node in the decentralized cluster."""

    node_id: str
    address: str
    status: str = "online"
    load: float = 0.0  # Average load across local workers
    last_seen: datetime = Field(default_factory=lambda: datetime.now(timezone.utc))


class PersonalityTraits(BaseModel):
    """Quantitative traits defining an agent's playing and interaction style."""

    aggressiveness: float = Field(default=0.5, ge=0.0, le=1.0)
    positional_style: float = Field(default=0.5, ge=0.0, le=1.0)
    risk_tolerance: float = Field(default=0.5, ge=0.0, le=1.0)
    tone: str = "neutral"  # neutral, aggressive, humorous, formal


class TrainingStatus(str, Enum):
    """Status of a personality training job."""

    QUEUED = "queued"
    TRAINING = "training"
    VALIDATING = "validating"
    COMPLETED = "completed"
    FAILED = "failed"


class TrainingJob(BaseModel):
    """Metadata and status for an agent personality training session."""

    job_id: str = Field(default_factory=lambda: str(uuid.uuid4()))
    agent_id: str
    target_traits: PersonalityTraits
    status: TrainingStatus = TrainingStatus.QUEUED
    progress: float = 0.0  # 0.0 to 100.0
    started_at: Optional[datetime] = None
    completed_at: Optional[datetime] = None
    node_id: Optional[str] = None  # Node where training is occurring
