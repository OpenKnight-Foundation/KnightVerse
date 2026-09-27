from __future__ import annotations

from enum import Enum

from pydantic import BaseModel, Field, field_validator


class EngineBackend(str, Enum):
    """Supported UCI engine backends."""

    LC0 = "lc0"
    STOCKFISH = "stockfish"


class GPUConfig(BaseModel):
    """GPU execution settings for neural-engine inference."""

    # Which physical/logical GPU device to run inference on.
    device_id: int = Field(default=0, ge=0)
    # Upper bound on how many positions can be batched into one GPU call.
    max_batch_size: int = Field(default=32, ge=1)
    # Soft cap on GPU memory usage, in megabytes.
    memory_limit_mb: int = Field(default=2048, ge=128)
    # Inference backend library (e.g. "cudnn"), stored as a free-form string
    # rather than an enum since supported backends can vary by build.
    backend: str = Field(default="cudnn", min_length=1)

    @field_validator("backend")
    @classmethod
    def normalize_backend(cls, value: str) -> str:
        """Normalize backend values for engine configuration."""

        # Lowercase + trimmed so "CuDNN" / " cudnn " compare equal to "cudnn".
        return value.strip().lower()


class WorkerConfig(BaseModel):
    """Configuration for a single GPU analysis worker."""

    # Which UCI engine implementation this worker runs.
    engine_backend: EngineBackend = EngineBackend.LC0
    # Filesystem path to the engine executable.
    engine_path: str = "/usr/local/bin/lc0"
    # Nested GPU-specific settings (only meaningful for GPU-backed engines).
    gpu: GPUConfig = Field(default_factory=GPUConfig)
    # How many analyses this worker can run at the same time.
    max_concurrent_analyses: int = Field(default=8, ge=1)
    # Fallback search depth used when a request doesn't specify one.
    default_depth: int = Field(default=20, ge=1)
    # Fallback time budget (ms) used when a request doesn't specify one.
    default_time_limit_ms: int = Field(default=5000, ge=1)
    # CPU threads allocated to the engine process (mainly relevant for Stockfish).
    threads: int = Field(default=2, ge=1)
    # Engine transposition/hash table size, in megabytes.
    hash_size_mb: int = Field(default=512, ge=1)
    # Optional path to neural network weights (relevant for lc0-style engines);
    # None means use the engine's built-in/default weights.
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

        # Treat missing/blank paths the same way: normalize to None.
        if value is None:
            return None
        normalized = value.strip()
        return normalized or None