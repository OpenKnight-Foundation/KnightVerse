"""GPU-accelerated analysis worker for XLMate (issue #642).

Wraps a UCI chess engine (Stockfish / Lc0) with GPU-aware resource tracking
and exposes an async interface for the worker pool.
"""

from __future__ import annotations

import asyncio
import subprocess
import time
import uuid
from dataclasses import dataclass, field
from enum import Enum
from typing import Any


class WorkerStatus(str, Enum):
    IDLE = "idle"
    BUSY = "busy"
    ERROR = "error"
    SHUTTING_DOWN = "shutting_down"


@dataclass
class AnalysisRequest:
    """A single position analysis request."""

    fen: str
    depth: int = 18
    time_limit_ms: int = 3000
    num_pv: int = 1
    id: str = field(default_factory=lambda: str(uuid.uuid4()))


@dataclass
class AnalysisResult:
    """Result returned by the worker after analysis."""

    request_id: str
    best_move: str
    evaluation: float | None = None
    depth: int = 0
    principal_variation: list[str] = field(default_factory=list)
    nodes_searched: int = 0
    time_ms: int = 0
    gpu_utilization: float | None = None


@dataclass
class WorkerConfig:
    """Configuration for a GPU analysis worker."""

    engine_path: str = "stockfish"
    device_id: int = 0
    max_concurrent: int = 4
    default_depth: int = 18
    default_time_ms: int = 3000
    hash_mb: int = 256
    threads: int = 4


class GPUAnalysisWorker:
    """Single GPU-accelerated analysis worker.

    Spawns a UCI engine subprocess and routes analysis requests through it.
    GPU utilization is sampled from the resource monitor when available.
    """

    def __init__(
        self,
        config: WorkerConfig | None = None,
        worker_id: str | None = None,
        resource_monitor: Any = None,
    ) -> None:
        self.config = config or WorkerConfig()
        self.worker_id = worker_id or str(uuid.uuid4())
        self._monitor = resource_monitor
        self._status = WorkerStatus.IDLE
        self._proc: asyncio.subprocess.Process | None = None
        self._lock = asyncio.Lock()
        self._analyses_completed = 0
        self._started_at: float | None = None

    @property
    def status(self) -> WorkerStatus:
        return self._status

    @property
    def is_ready(self) -> bool:
        return self._status == WorkerStatus.IDLE

    async def start(self) -> None:
        """Spawn the engine process and wait for UCI readiness."""
        self._proc = await asyncio.create_subprocess_exec(
            self.config.engine_path,
            stdin=asyncio.subprocess.PIPE,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.DEVNULL,
        )
        await self._send("uci")
        await self._wait_for("uciok")
        await self._send(f"setoption name Hash value {self.config.hash_mb}")
        await self._send(f"setoption name Threads value {self.config.threads}")
        await self._send("isready")
        await self._wait_for("readyok")
        self._started_at = time.monotonic()
        self._status = WorkerStatus.IDLE

    async def analyze(self, request: AnalysisRequest) -> AnalysisResult:
        """Analyze a position and return the result."""
        if self._status != WorkerStatus.IDLE:
            raise RuntimeError(f"Worker not idle (status={self._status})")
        async with self._lock:
            self._status = WorkerStatus.BUSY
            t0 = time.monotonic()
            try:
                await self._send(f"position fen {request.fen}")
                await self._send(
                    f"go depth {request.depth} movetime {request.time_limit_ms}"
                )
                best_move, pv, score, depth = await self._collect_results()
                elapsed_ms = int((time.monotonic() - t0) * 1000)
                self._analyses_completed += 1
                self._status = WorkerStatus.IDLE
                return AnalysisResult(
                    request_id=request.id,
                    best_move=best_move,
                    evaluation=score,
                    depth=depth,
                    principal_variation=pv,
                    time_ms=elapsed_ms,
                    gpu_utilization=self._gpu_utilization(),
                )
            except Exception:
                self._status = WorkerStatus.ERROR
                raise

    async def shutdown(self) -> None:
        """Gracefully stop the engine process."""
        self._status = WorkerStatus.SHUTTING_DOWN
        if self._proc and self._proc.returncode is None:
            try:
                await self._send("quit")
                await asyncio.wait_for(self._proc.wait(), timeout=3.0)
            except Exception:
                self._proc.kill()
        self._proc = None

    def uptime_seconds(self) -> float:
        if self._started_at is None:
            return 0.0
        return max(0.0, time.monotonic() - self._started_at)

    # ------------------------------------------------------------------ helpers

    async def _send(self, cmd: str) -> None:
        assert self._proc and self._proc.stdin
        self._proc.stdin.write((cmd + "\n").encode())
        await self._proc.stdin.drain()

    async def _wait_for(self, token: str, timeout: float = 10.0) -> None:
        assert self._proc and self._proc.stdout
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            line = await asyncio.wait_for(self._proc.stdout.readline(), timeout=5.0)
            if token in line.decode():
                return
        raise TimeoutError(f"Timed out waiting for '{token}'")

    async def _collect_results(self) -> tuple[str, list[str], float | None, int]:
        """Read engine output until bestmove line."""
        assert self._proc and self._proc.stdout
        best_move = ""
        pv: list[str] = []
        score: float | None = None
        depth = 0
        while True:
            raw = await asyncio.wait_for(self._proc.stdout.readline(), timeout=30.0)
            line = raw.decode().strip()
            if line.startswith("bestmove"):
                parts = line.split()
                best_move = parts[1] if len(parts) > 1 else ""
                break
            if line.startswith("info"):
                tokens = line.split()
                if "depth" in tokens:
                    depth = int(tokens[tokens.index("depth") + 1])
                if "score" in tokens and "cp" in tokens:
                    idx = tokens.index("cp")
                    score = int(tokens[idx + 1]) / 100.0
                if "pv" in tokens:
                    pv = tokens[tokens.index("pv") + 1 :]
        return best_move, pv, score, depth

    def _gpu_utilization(self) -> float | None:
        if self._monitor is None:
            return None
        stats = self._monitor.get_gpu_stats()
        for dev in stats.get("devices", []):
            if dev.get("device_id") == self.config.device_id:
                return float(dev.get("utilization_pct", 0.0))
        return None
