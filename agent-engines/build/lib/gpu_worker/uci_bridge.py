from __future__ import annotations

import asyncio
from asyncio.subprocess import PIPE
from collections.abc import Callable
from contextlib import suppress
from dataclasses import dataclass, field
from enum import Enum
import re
from typing import Protocol

from gpu_worker.config import EngineBackend, WorkerConfig


class UciBridgeError(RuntimeError):
    """Raised when the UCI bridge cannot complete a requested operation."""


class UciScoreKind(str, Enum):
    """Supported UCI score encodings."""

    CP = "cp"
    MATE = "mate"


@dataclass(slots=True)
class UciInfo:
    """Parsed state from an `info` line emitted by a UCI engine."""

    depth: int | None = None
    evaluation: float | None = None
    principal_variation: list[str] = field(default_factory=list)
    nodes: int | None = None
    multipv: int = 1


@dataclass(slots=True)
class UciBestMove:
    """Parsed `bestmove` message."""

    best_move: str
    ponder: str | None = None


class AsyncLineReader(Protocol):
    """Protocol for async line readers used by the bridge."""

    async def readline(self) -> bytes: ...


class AsyncLineWriter(Protocol):
    """Protocol for async line writers used by the bridge."""

    def write(self, data: bytes) -> object: ...

    async def drain(self) -> None: ...

    def is_closing(self) -> bool: ...


class UciProcess(Protocol):
    """Protocol implemented by asyncio subprocess objects used by the bridge."""

    stdin: AsyncLineWriter | None
    stdout: AsyncLineReader | None
    returncode: int | None

    async def wait(self) -> int: ...

    def kill(self) -> None: ...


ProcessFactory = Callable[..., asyncio.Future | asyncio.Task | asyncio.subprocess.Process]

_OPTION_RE = re.compile(r"^option name (?P<name>.+?) type ")


class AsyncUciBridge:
    """Async bridge for communicating with UCI-compatible engines."""

    def __init__(
        self,
        config: WorkerConfig,
        *,
        startup_timeout_seconds: float = 5.0,
        command_timeout_seconds: float = 30.0,
        process_factory: Callable[..., object] | None = None,
    ) -> None:
        self.config = config
        self.startup_timeout_seconds = startup_timeout_seconds
        self.command_timeout_seconds = command_timeout_seconds
        self._process_factory = process_factory
        self._process: UciProcess | None = None
        self._command_lock = asyncio.Lock()
        self._supported_options: set[str] = set()

    async def start(self) -> None:
        """Spawn the engine process and complete the UCI handshake."""

        if self._process is not None:
            return
        process_factory = self._process_factory or asyncio.create_subprocess_exec
        process = process_factory(*self.build_command(), stdin=PIPE, stdout=PIPE, stderr=PIPE)
        if asyncio.iscoroutine(process):
            process = await process
        self._process = process  # type: ignore[assignment]
        await self._send_command("uci")
        await self._collect_startup_messages()

    def build_command(self) -> list[str]:
        """Build the engine launch command from worker configuration."""

        command = [self.config.engine_path]
        if self.config.engine_backend is EngineBackend.LC0:
            command.extend(
                [
                    f"--backend={self.config.gpu.backend}",
                    f"--gpu={self.config.gpu.device_id}",
                ]
            )
            if self.config.network_weights_path:
                command.append(f"--weights={self.config.network_weights_path}")
        return command

    async def initialize_options(self) -> None:
        """Apply engine options after the UCI handshake and wait for readiness."""

        if self.config.engine_backend is EngineBackend.LC0:
            await self._set_option_if_supported("Backend", self.config.gpu.backend)
            await self._set_option_if_supported(
                "WeightsFile", self.config.network_weights_path
            )
            await self._set_option_if_supported("GPU", str(self.config.gpu.device_id))
            await self._set_option_if_supported("Threads", str(self.config.threads))
            await self._set_option_if_supported(
                "NNCacheSize", str(self.config.hash_size_mb)
            )
        else:
            await self._set_option_if_supported("Threads", str(self.config.threads))
            await self._set_option_if_supported("Hash", str(self.config.hash_size_mb))
        await self.ensure_ready()

    async def ensure_ready(self) -> None:
        """Wait for the engine to acknowledge readiness."""

        await self._send_command("isready")
        await self._wait_for(lambda value: True if value == "readyok" else None)

    async def set_position(self, fen: str) -> None:
        """Set the current board position for the next search."""

        await self._send_command(f"position fen {fen}")

    async def go(
        self,
        *,
        depth: int | None = None,
        time_limit_ms: int | None = None,
        search_moves: list[str] | None = None,
        num_pv: int = 1,
    ) -> tuple[UciBestMove, UciInfo]:
        """Start a search and return the best move plus the latest info snapshot."""

        if num_pv > 1:
            await self._set_option_if_supported("MultiPV", str(num_pv))

        command = ["go"]
        if depth is not None:
            command.extend(["depth", str(depth)])
        if time_limit_ms is not None:
            command.extend(["movetime", str(time_limit_ms)])
        if search_moves:
            command.extend(["searchmoves", *search_moves])

        await self._send_command(" ".join(command))
        info = UciInfo()

        async def matcher(line: str) -> UciBestMove | None:
            nonlocal info
            parsed_info = parse_info_line(line)
            if parsed_info is not None and parsed_info.multipv == 1:
                info = parsed_info
                return None
            best_move = parse_bestmove_line(line)
            return best_move

        best_move = await self._wait_for(matcher, timeout_seconds=self._search_timeout(time_limit_ms))
        return best_move, info

    async def stop(self) -> None:
        """Stop the current search if the engine is still running."""

        if self._process is None:
            return
        await self._send_command("stop")

    async def quit(self) -> None:
        """Gracefully terminate the engine process."""

        if self._process is None:
            return
        process = self._process
        self._process = None
        with suppress(UciBridgeError):
            await self._send_command("quit", process=process)
        try:
            await asyncio.wait_for(process.wait(), timeout=2)
        except asyncio.TimeoutError:
            process.kill()
            with suppress(Exception):
                await process.wait()

    async def _collect_startup_messages(self) -> None:
        """Read startup metadata until `uciok` is observed."""

        async def matcher(line: str) -> bool | None:
            option_name = parse_option_name(line)
            if option_name:
                self._supported_options.add(option_name)
            if line == "uciok":
                return True
            return None

        await self._wait_for(matcher, timeout_seconds=self.startup_timeout_seconds)

    async def _set_option_if_supported(self, name: str, value: str | None) -> None:
        """Send `setoption` only when the engine reported support for it."""

        if value is None or name not in self._supported_options:
            return
        await self._send_command(f"setoption name {name} value {value}")

    async def _send_command(self, command: str, process: UciProcess | None = None) -> None:
        """Write a command to the engine stdin."""

        current_process = process or self._process
        if current_process is None or current_process.stdin is None:
            raise UciBridgeError("engine process is not running")
        if current_process.stdin.is_closing():
            raise UciBridgeError("engine stdin is closed")
        async with self._command_lock:
            current_process.stdin.write(f"{command}\n".encode())
            await current_process.stdin.drain()

    async def _read_line(self) -> str:
        """Read and decode one stdout line from the engine."""

        if self._process is None or self._process.stdout is None:
            raise UciBridgeError("engine stdout is not available")
        raw_line = await self._process.stdout.readline()
        if raw_line == b"":
            raise UciBridgeError("engine process closed its stdout")
        return raw_line.decode().strip()

    async def _wait_for(
        self,
        matcher: Callable[[str], object],
        *,
        timeout_seconds: float | None = None,
    ) -> object:
        """Read stdout lines until the matcher returns a non-None value."""

        timeout_value = timeout_seconds or self.command_timeout_seconds

        async def loop() -> object:
            while True:
                line = await self._read_line()
                result = matcher(line)
                if asyncio.iscoroutine(result):
                    result = await result
                if result is not None:
                    return result

        try:
            return await asyncio.wait_for(loop(), timeout=timeout_value)
        except asyncio.TimeoutError as exc:
            raise UciBridgeError("timed out waiting for engine response") from exc

    def _search_timeout(self, time_limit_ms: int | None) -> float:
        """Derive a search timeout with slack beyond movetime."""

        if time_limit_ms is None:
            return self.command_timeout_seconds
        return max(self.command_timeout_seconds, (time_limit_ms / 1000.0) + 1.0)


def parse_option_name(line: str) -> str | None:
    """Extract a UCI option name from an engine startup line."""

    match = _OPTION_RE.match(line)
    return match.group("name") if match else None


def parse_info_line(line: str) -> UciInfo | None:
    """Parse a UCI `info` line into structured state."""

    parts = line.split()
    if not parts or parts[0] != "info":
        return None

    depth: int | None = None
    evaluation: float | None = None
    nodes: int | None = None
    multipv = 1
    pv: list[str] = []

    index = 1
    while index < len(parts):
        token = parts[index]
        if token == "depth" and index + 1 < len(parts):
            depth = int(parts[index + 1])
            index += 2
            continue
        if token == "nodes" and index + 1 < len(parts):
            nodes = int(parts[index + 1])
            index += 2
            continue
        if token == "multipv" and index + 1 < len(parts):
            multipv = int(parts[index + 1])
            index += 2
            continue
        if token == "score" and index + 2 < len(parts):
            score_kind = parts[index + 1]
            score_value = int(parts[index + 2])
            if score_kind == UciScoreKind.CP.value:
                evaluation = score_value / 100.0
            elif score_kind == UciScoreKind.MATE.value:
                evaluation = 100000.0 if score_value > 0 else -100000.0
            index += 3
            continue
        if token == "pv":
            pv = parts[index + 1 :]
            break
        index += 1

    return UciInfo(
        depth=depth,
        evaluation=evaluation,
        principal_variation=pv,
        nodes=nodes,
        multipv=multipv,
    )


def parse_bestmove_line(line: str) -> UciBestMove | None:
    """Parse a UCI `bestmove` line."""

    parts = line.split()
    if len(parts) < 2 or parts[0] != "bestmove":
        return None
    ponder = parts[3] if len(parts) >= 4 and parts[2] == "ponder" else None
    return UciBestMove(best_move=parts[1], ponder=ponder)
