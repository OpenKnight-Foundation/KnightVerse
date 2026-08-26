from __future__ import annotations

import asyncio

import pytest

from gpu_worker.config import EngineBackend, WorkerConfig
from gpu_worker.uci_bridge import (
    AsyncUciBridge,
    UciBridgeError,
    parse_bestmove_line,
    parse_info_line,
    parse_option_name,
)


class FakeStreamWriter:
    def __init__(self) -> None:
        self.buffer: list[str] = []
        self._closing = False

    def write(self, data: bytes) -> None:
        self.buffer.append(data.decode())

    async def drain(self) -> None:
        return None

    def is_closing(self) -> bool:
        return self._closing


class FakeStreamReader:
    def __init__(self, lines: list[str]) -> None:
        self._lines = asyncio.Queue[bytes]()
        for line in lines:
            self._lines.put_nowait(f"{line}\n".encode())

    async def readline(self) -> bytes:
        return await self._lines.get()


class FakeProcess:
    def __init__(self, lines: list[str]) -> None:
        self.stdin = FakeStreamWriter()
        self.stdout = FakeStreamReader(lines)
        self.returncode: int | None = None
        self.killed = False

    async def wait(self) -> int:
        self.returncode = 0
        return 0

    def kill(self) -> None:
        self.killed = True
        self.returncode = -9


@pytest.mark.asyncio
async def test_bridge_sends_commands_and_parses_search_result() -> None:
    process = FakeProcess(
        [
            "id name lc0",
            "option name Backend type combo default cudnn",
            "option name Threads type spin default 2 min 1 max 64",
            "option name MultiPV type spin default 1 min 1 max 10",
            "uciok",
            "readyok",
            "info depth 18 score cp 42 nodes 1234 pv e2e4 e7e5 g1f3",
            "bestmove e2e4 ponder e7e5",
            "readyok",
        ]
    )
    config = WorkerConfig(engine_backend=EngineBackend.LC0)
    bridge = AsyncUciBridge(config, process_factory=lambda *args, **kwargs: process)

    await bridge.start()
    await bridge.initialize_options()
    await bridge.set_position("8/8/8/8/8/8/8/K6k w - - 0 1")
    best_move, info = await bridge.go(depth=18, time_limit_ms=25, num_pv=2)
    await bridge.ensure_ready()

    assert best_move.best_move == "e2e4"
    assert best_move.ponder == "e7e5"
    assert info.depth == 18
    assert info.evaluation == pytest.approx(0.42)
    assert info.nodes == 1234
    assert info.principal_variation == ["e2e4", "e7e5", "g1f3"]
    assert process.stdin.buffer[0] == "uci\n"
    assert "setoption name Backend value cudnn\n" in process.stdin.buffer
    assert "setoption name Threads value 2\n" in process.stdin.buffer
    assert "setoption name MultiPV value 2\n" in process.stdin.buffer
    assert "position fen 8/8/8/8/8/8/8/K6k w - - 0 1\n" in process.stdin.buffer
    assert "go depth 18 movetime 25\n" in process.stdin.buffer


@pytest.mark.asyncio
async def test_bridge_times_out_when_engine_is_silent() -> None:
    process = FakeProcess(["uciok"])
    config = WorkerConfig(engine_backend=EngineBackend.STOCKFISH)
    bridge = AsyncUciBridge(
        config,
        process_factory=lambda *args, **kwargs: process,
        command_timeout_seconds=0.01,
    )

    await bridge.start()

    with pytest.raises(UciBridgeError):
        await bridge.ensure_ready()


def test_parse_info_and_bestmove_lines() -> None:
    info = parse_info_line("info depth 22 score mate 3 nodes 9000 pv e2e4 e7e5")
    best_move = parse_bestmove_line("bestmove e2e4 ponder e7e5")

    assert info is not None
    assert info.depth == 22
    assert info.evaluation == 100000.0
    assert info.nodes == 9000
    assert info.principal_variation == ["e2e4", "e7e5"]
    assert best_move is not None
    assert best_move.best_move == "e2e4"
    assert best_move.ponder == "e7e5"
    assert parse_option_name("option name Hash type spin default 16 min 1 max 2048") == "Hash"
