from __future__ import annotations

import logging
import os
from enum import Enum

from pydantic import BaseModel, Field, field_validator, model_validator

logger = logging.getLogger(__name__)

# Default engine paths per backend, overridable via environment variables.
_DEFAULT_ENGINE_PATHS: dict[str, str] = {
    "lc0": "/usr/local/bin/lc0",
    "stockfish": "/usr/local/bin/stockfish",
}


def _resolve_engine_path(backend: str) -> str:
    """Return the engine path for *backend*, preferring environment overrides.

    Resolution order:
    1. ``ENGINE_PATH`` – generic override for any backend.
    2. ``LC0_PATH`` / ``STOCKFISH_PATH`` – backend-specific overrides.
    3. Compiled-in default path for the backend.

    If none of the environment variables are set and the default path does not
    exist, a warning is logged so the operator has a clear signal instead of an
    opaque crash later when the process is actually spawned.
    """
    env_key = f"{backend.upper()}_PATH"
    path = (
        os.environ.get("ENGINE_PATH")
        or os.environ.get(env_key)
        or _DEFAULT_ENGINE_PATHS.get(backend, "/usr/local/bin/lc0")
    )
    if not os.path.isfile(path):
        logger.warning(
            "Engine binary not found at '%s' for backend '%s'. "
            "Set the ENGINE_PATH or %s environment variable to override.",
            path,
            backend,
            env_key,
        )
    return path


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
    engine_path: str = Field(
        default_factory=lambda: _resolve_engine_path(EngineBackend.LC0.value)
    )
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

    @model_validator(mode="after")
    def resolve_engine_path_for_backend(self) -> "WorkerConfig":
        """Re-resolve engine_path against the selected backend when it was not
        explicitly supplied (i.e. still holds the LC0 default while the backend
        is Stockfish).  This ensures the env-var override is always consulted
        with the correct backend key."""

        lc0_default = _resolve_engine_path(EngineBackend.LC0.value)
        if self.engine_path == lc0_default and self.engine_backend is not EngineBackend.LC0:
            object.__setattr__(
                self, "engine_path", _resolve_engine_path(self.engine_backend.value)
            )
        return self
