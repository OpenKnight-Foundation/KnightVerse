from __future__ import annotations

from enum import Enum

from pydantic import BaseModel, Field, field_validator


class EngineBackend(str, Enum):
    """Supported UCI engine backends."""

    LC0 = "lc0"
    STOCKFISH = "stockfish"


class GPUConfig(BaseModel):
    """GPU execution settings for neural-engine inference."""

    device_id: int = Field(default=0, ge=0)
    max_batch_size: int = Field(default=32, ge=1)
    memory_limit_mb: int = Field(default=2048, ge=128)
    backend: str = Field(default="cudnn", min_length=1)

    @field_validator("backend")
    @classmethod
    def normalize_backend(cls, value: str) -> str:
        """Normalize backend values for engine configuration."""

        return value.strip().lower()


class WorkerConfig(BaseModel):
    """Configuration for a single GPU analysis worker."""

    engine_backend: EngineBackend = EngineBackend.LC0
    engine_path: str = "/usr/local/bin/lc0"
    gpu: GPUConfig = Field(default_factory=GPUConfig)
    max_concurrent_analyses: int = Field(default=8, ge=1)
    default_depth: int = Field(default=20, ge=1)
    default_time_limit_ms: int = Field(default=5000, ge=1)
    threads: int = Field(default=2, ge=1)
    hash_size_mb: int = Field(default=512, ge=1)
    network_weights_path: str | None = None

    @field_validator("engine_path")
    @classmethod
    def validate_engine_path(cls, value: str) -> str:
        """Ensure engine paths are non-empty after trimming."""

        normalized = value.strip()
        if not normalized:
            raise ValueError("engine_path must not be empty")
        return normalized

    @field_validator("network_weights_path")
    @classmethod
    def normalize_optional_path(cls, value: str | None) -> str | None:
        """Normalize optional filesystem paths."""

        if value is None:
            return None
        normalized = value.strip()
        return normalized or None
