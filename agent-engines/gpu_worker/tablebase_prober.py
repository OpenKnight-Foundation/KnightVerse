from __future__ import annotations

import asyncio
import builtins
import logging
import os
import time
import urllib.error
import urllib.request
from typing import Optional

import chess
import chess.syzygy

from gpu_worker.config import WorkerConfig

logger = logging.getLogger(__name__)


WDL_WIN = 2
WDL_DRAW = 0
WDL_LOSS = -2
WDL_CURSED_WIN = 1
WLED_BLESSED_LOSS = -1

WDL_TO_OUTCOME = {2: "win", 1: "cursed win", 0: "draw", -1: "blessed loss", -2: "loss"}


class WdlResult:
    """Result of a WDL tablebase probe.

    Used as a field type in Pydantic models with ``arbitrary_types_allowed``.
    """

    __slots__ = ("wdl", "dtz", "mate_in")

    def __init__(self, wdl: int, dtz: Optional[int], mate_in: Optional[int] = None) -> None:
        self.wdl = wdl
        self.dtz = dtz
        self.mate_in = mate_in

    @property
    def outcome(self) -> str:
        return WDL_TO_OUTCOME.get(self.wdl, "unknown")

    def __repr__(self) -> str:
        return f"WdlResult(wdl={self.wdl}, dtz={self.dtz}, outcome={self.outcome})"

    def __pydantic_self_check(self) -> None:
        """Pydantic hook - makes WdlResult compatible with Pydantic validation."""
        pass


class TablebaseProber:
    """Probe Syzygy tablebases for endgame WDL and DTZ metrics."""

    def __init__(
        self,
        *,
        local_path: str | None = None,
        remote_url: str | None = None,
        config: WorkerConfig | None = None,
    ) -> None:
        self._config = config or WorkerConfig()
        self._local_path = local_path or self._resolve_local_path()
        self._tablebase: chess.syzygy.Tablebase | None = None
        self._remote_url = remote_url
        self._remote_rate_limit = 0.2
        self._last_remote_call = 0.0

        if self._local_path:
            if not os.path.isdir(self._local_path):
                logger.warning(
                    "Syzygy tablebase directory not found at '%s'. "
                    "Local tablebase probes will fall back to remote or None.",
                    self._local_path,
                )
            else:
                self._tablebase = chess.syzygy.Tablebase()
                self._tablebase.add_directory(self._local_path)

    # ------------------------------------------------------------------
    # Local path resolution
    # ------------------------------------------------------------------

    def _resolve_local_path(self) -> str | None:
        """Resolve the Syzygy tablebase local directory path."""
        env_path = getattr(self._config, "syzygy_tablebase_path", None)  # type: ignore[attr-defined]
        if env_path:
            return env_path
        return None

    # ------------------------------------------------------------------
    # Probing
    # ------------------------------------------------------------------

    def _check_piece_count(self, board: chess.Board) -> bool:
        """Return True if the position has <= 7 pieces (syzygy limit)."""

        return chess.popcount(board.occupied) <= 7

    async def probe(self, board: chess.Board) -> WdlResult | None:
        """Probe the position in *board* using local then remote tables.

        Returns ``None`` if the position has >7 pieces or both probes fail.
        """

        if not self._check_piece_count(board):
            return None

        # --- Local probe ---
        if self._tablebase is not None:
            try:
                wdl = self._tablebase.get_wdl(board)
                dtz = self._tablebase.get_dtz(board)  # type: ignore[attr-in]
                return WdlResult(wdl=wdl, dtz=dtz)
            except (KeyError, Exception):
                pass  # fall through to remote

        # --- Remote probe ---
        if self._remote_url is not None:
            return await self._probe_remote(board)

        return None

    async def _probe_remote(self, board: chess.Board) -> WdlResult | None:
        """Probe the position via a remote HTTP tablebase API."""

        await asyncio.sleep(0)  # yield control

        elapsed = time.monotonic() - self._last_remote_call
        if elapsed < self._remote_rate_limit:
            await asyncio.sleep(self._remote_rate_limit - elapsed)

        self._last_remote_call = time.monotonic()

        try:
            fen = board.fen()
            encoded_fen = urllib.request.quote(fen, safe="")
            url = f"{self._remote_url}/probe?fen={encoded_fen}"

            request = urllib.request.Request(url)
            request.add_header("Accept", "application/json")

            timeout = float(getattr(self._config, "syzygy_remote_timeout", 1.5))  # type: ignore[attr-defined]
            response = urllib.request.urlopen(url, timeout=timeout)
            data = response.read().decode()

            # Parse simple JSON { "wdl": int, "dtz": int | null }
            wdl = None
            dtz = None
            if '"wdl"' in data:
                import json
                parsed = json.loads(data)
                wdl = int(parsed.get("wdl", 0))
                dtz = int(parsed.get("dtz")) if parsed.get("dtz") is not None else None

            if wdl is not None:
                return WdlResult(wdl=wdl, dtz=dtz)

        except (urllib.error.URLError, asyncio.TimeoutError, ValueError, OSError):
            pass  # fail over gracefully

        return None

    # ------------------------------------------------------------------
    # Convenience wrappers mirroring chess.syzygy signatures
    # ------------------------------------------------------------------

    def get_wdl(self, board: chess.Board, default: Optional[int] = None) -> Optional[int]:
        """Return WDL (-2..2) or *default* if the position is not in a tablebase."""

        if not self._check_piece_count(board):
            return default

        if self._tablebase is not None:
            try:
                return self._tablebase.get_wdl(board, default)  # type: ignore[attr-in]
            except Exception:
                pass

        if self._remote_url is not None:
            result = asyncio.run(self._probe_remote(board))
            if result is not None:
                return result.wdl

        return default

    def get_dtz(self, board: chess.Board, default: Optional[int] = None) -> Optional[int]:
        """Return DTZ value or *default* if not available."""

        if not self._check_piece_count(board):
            return default

        if self._tablebase is not None:
            try:
                return self._tablebase.get_dtz(board)  # type: ignore[attr-in]
            except Exception:
                pass

        if self._remote_url is not None:
            result = asyncio.run(self._probe_remote(board))
            if result is not None:
                return result.dtz

        return default

    # ------------------------------------------------------------------
    # Context manager support
    # ------------------------------------------------------------------

    def __enter__(self) -> "TablebaseProber":
        return self

    def __exit__(
        self,
        exc_type: Optional[type[BaseException]],
        exc_value: Optional[BaseException],
        traceback: Optional[object],
    ) -> None:
        if self._tablebase is not None:
            self._tablebase.close()  # type: ignore[attr-in]