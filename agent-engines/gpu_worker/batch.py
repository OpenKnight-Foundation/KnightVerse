"""
High-Performance FEN Tokenizer & Batched Tensor Evaluation Cache.

Converts FEN strings to NumPy bitboard tensors (8×8×14) using vectorized
operations, and caches evaluation results via an LRU cache keyed on
position hash.  Designed for >100k FENs/second throughput on a single core.
"""

from __future__ import annotations

import hashlib
import time
from collections import OrderedDict
from dataclasses import dataclass, field
from typing import Optional

import chess
import numpy as np


# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

# Piece type indices (14 planes: 6 white + 6 black + en-passant + turn)
PIECE_INDEX = {
    (chess.PAWN, chess.WHITE): 0,
    (chess.KNIGHT, chess.WHITE): 1,
    (chess.BISHOP, chess.WHITE): 2,
    (chess.ROOK, chess.WHITE): 3,
    (chess.QUEEN, chess.WHITE): 4,
    (chess.KING, chess.WHITE): 5,
    (chess.PAWN, chess.BLACK): 6,
    (chess.KNIGHT, chess.BLACK): 7,
    (chess.BISHOP, chess.BLACK): 8,
    (chess.ROOK, chess.BLACK): 9,
    (chess.QUEEN, chess.BLACK): 10,
    (chess.KING, chess.BLACK): 11,
}

# Board dimensions
BOARD_SIZE = 8
NUM_PLANES = 14  # 12 piece planes + en-passant + side-to-move


# ---------------------------------------------------------------------------
# FEN-to-Tensor Tokenizer
# ---------------------------------------------------------------------------

class FENTokenizer:
    """
    Vectorised FEN → (8, 8, 14) binary tensor converter.

    The tensor layout is:
      planes 0-5:   White P, N, B, R, Q, K
      planes 6-11:  Black P, N, B, R, Q, K
      plane 12:     En-passant square (if any)
      plane 13:     Side to move (1 = white, 0 = black)

    Usage::

        tokenizer = FENTokenizer()
        tensor = tokenizer.fen_to_tensor(board.fen())
        tensors = tokenizer.batch_fen_to_tensors([fen1, fen2, ...])
    """

    # Pre-computed mapping: ASCII char → (piece_type, colour) or None
    _CHAR_MAP: dict[str, Optional[tuple[int, bool]]] = {}

    @classmethod
    def _build_char_map(cls) -> None:
        if cls._CHAR_MAP:
            return
        for piece_char, (pt, c) in {
            "P": (chess.PAWN, chess.WHITE),
            "N": (chess.KNIGHT, chess.WHITE),
            "B": (chess.BISHOP, chess.WHITE),
            "R": (chess.ROOK, chess.WHITE),
            "Q": (chess.QUEEN, chess.WHITE),
            "K": (chess.KING, chess.WHITE),
            "p": (chess.PAWN, chess.BLACK),
            "n": (chess.KNIGHT, chess.BLACK),
            "b": (chess.BISHOP, chess.BLACK),
            "r": (chess.ROOK, chess.BLACK),
            "q": (chess.QUEEN, chess.BLACK),
            "k": (chess.KING, chess.BLACK),
        }.items():
            cls._CHAR_MAP[piece_char] = (pt, c)

    def __init__(self) -> None:
        self._build_char_map()
        # Pre-compute a flat lookup: ASCII char → plane index, or -1
        self._plane_lut = np.full(128, -1, dtype=np.int32)
        for ch, (pt, c) in self._CHAR_MAP.items():
            self._plane_lut[ord(ch)] = PIECE_INDEX[(pt, c)]

    # -- single FEN -------------------------------------------------------

    def fen_to_tensor(self, fen: str) -> np.ndarray:
        """Convert a FEN string to an (8, 8, 14) binary tensor.

        Uses direct string parsing for maximum throughput, bypassing
        python-chess Board construction.
        """
        tensor = np.zeros((BOARD_SIZE, BOARD_SIZE, NUM_PLANES), dtype=np.float32)
        lut = self._plane_lut

        # Split FEN: piece_placement side castling ep_square halfmove fullmove
        parts = fen.split()
        placement = parts[0]
        side_to_move = parts[1] if len(parts) > 1 else "w"
        ep_square = parts[3] if len(parts) > 3 else "-"

        # Parse piece placement row by row
        rank = 7  # FEN starts from rank 8 (index 7)
        file = 0
        for ch in placement:
            o = ord(ch)
            if o == 47:  # '/'
                rank -= 1
                file = 0
            elif 48 <= o <= 57:  # '0'-'9'
                file += o - 48
            else:
                plane = lut[o]
                if plane >= 0:
                    tensor[rank, file, plane] = 1.0
                file += 1

        # En-passant plane
        if ep_square != "-" and len(ep_square) == 2:
            ep_file = ord(ep_square[0]) - 97  # 'a'
            ep_rank = ord(ep_square[1]) - 49  # '1'
            tensor[ep_rank, ep_file, 12] = 1.0

        # Side-to-move plane
        if side_to_move == "w":
            tensor[:, :, 13] = 1.0

        return tensor

    def board_to_tensor(self, board: chess.Board) -> np.ndarray:
        """Convert a python-chess Board directly to tensor."""
        tensor = np.zeros((BOARD_SIZE, BOARD_SIZE, NUM_PLANES), dtype=np.float32)

        # Vectorised piece placement via piece_map
        piece_map = board.piece_map()
        if piece_map:
            ranks = np.array([chess.square_rank(sq) for sq in piece_map], dtype=np.intp)
            files = np.array([chess.square_file(sq) for sq in piece_map], dtype=np.intp)
            planes = np.array(
                [PIECE_INDEX[(p.piece_type, p.color)] for p in piece_map.values()],
                dtype=np.intp,
            )
            tensor[ranks, files, planes] = 1.0

        # En-passant plane
        ep = board.ep_square
        if ep is not None:
            tensor[chess.square_rank(ep), chess.square_file(ep), 12] = 1.0

        # Side-to-move plane
        if board.turn == chess.WHITE:
            tensor[:, :, 13] = 1.0

        return tensor

    # -- batch ------------------------------------------------------------

    def batch_fen_to_tensors(self, fens: list[str]) -> np.ndarray:
        """Convert a list of FENs to a (N, 8, 8, 14) batch tensor."""
        tensors = [self.fen_to_tensor(f) for f in fens]
        return np.stack(tensors) if tensors else np.zeros((0, 8, 8, 14), dtype=np.float32)

    def batch_boards_to_tensor(self, boards: list[chess.Board]) -> np.ndarray:
        """Convert a list of Board objects to a (N, 8, 8, 14) batch tensor."""
        tensors = [self.board_to_tensor(b) for b in boards]
        return np.stack(tensors) if tensors else np.zeros((0, 8, 8, 14), dtype=np.float32)


# ---------------------------------------------------------------------------
# Position Hash
# ---------------------------------------------------------------------------

def position_hash(board: chess.Board) -> str:
    """Compute a unique hash for a board position (ignores halfmove clock)."""
    # Use Zobrist-style FEN without halfmove/fullmove counters
    fen_key = board.fen().rsplit(" ", 2)[0]  # remove halfmove and fullmove
    return hashlib.blake2b(fen_key.encode(), digest_size=16).hexdigest()


def fen_hash(fen: str) -> str:
    """Hash a FEN string (strips move counters for position equivalence)."""
    fen_key = fen.rsplit(" ", 2)[0]
    return hashlib.blake2b(fen_key.encode(), digest_size=16).hexdigest()


# ---------------------------------------------------------------------------
# LRU Tensor Evaluation Cache
# ---------------------------------------------------------------------------

@dataclass
class CacheEntry:
    """A single cached evaluation."""
    tensor: np.ndarray
    evaluation: Optional[float] = None  # centipawn score
    best_move: Optional[str] = None
    timestamp: float = field(default_factory=time.monotonic)


class TensorEvaluationCache:
    """
    In-memory LRU cache for tensor ↔ evaluation mappings.

    Keyed on position hash.  Deduplicates identical positions across
    concurrent games.  Evicts least-recently-used entries when at capacity.

    Usage::

        cache = TensorEvaluationCache(maxsize=100_000)
        cache.put(hash, tensor, evaluation=25, best_move="e4")
        entry = cache.get(hash)
    """

    def __init__(self, maxsize: int = 100_000) -> None:
        self.maxsize = maxsize
        self._cache: OrderedDict[str, CacheEntry] = OrderedDict()
        self._hits = 0
        self._misses = 0

    def get(self, key: str) -> Optional[CacheEntry]:
        """Retrieve a cached entry, promoting it to most-recently-used."""
        if key in self._cache:
            self._cache.move_to_end(key)
            self._hits += 1
            return self._cache[key]
        self._misses += 1
        return None

    def put(
        self,
        key: str,
        tensor: np.ndarray,
        *,
        evaluation: Optional[float] = None,
        best_move: Optional[str] = None,
    ) -> None:
        """Insert or update a cache entry, evicting LRU if at capacity."""
        entry = CacheEntry(
            tensor=tensor, evaluation=evaluation, best_move=best_move,
        )
        if key in self._cache:
            self._cache.move_to_end(key)
            self._cache[key] = entry
        else:
            if len(self._cache) >= self.maxsize:
                self._cache.popitem(last=False)  # evict LRU
            self._cache[key] = entry

    def contains(self, key: str) -> bool:
        return key in self._cache

    def remove(self, key: str) -> bool:
        if key in self._cache:
            del self._cache[key]
            return True
        return False

    def clear(self) -> None:
        self._cache.clear()
        self._hits = 0
        self._misses = 0

    @property
    def size(self) -> int:
        return len(self._cache)

    @property
    def hit_rate(self) -> float:
        total = self._hits + self._misses
        return self._hits / total if total > 0 else 0.0

    @property
    def stats(self) -> dict:
        return {
            "size": self.size,
            "maxsize": self.maxsize,
            "hits": self._hits,
            "misses": self._misses,
            "hit_rate": self.hit_rate,
        }


# ---------------------------------------------------------------------------
# Batch Evaluation Manager (ties tokenizer + cache together)
# ---------------------------------------------------------------------------

class BatchEvaluator:
    """
    High-level batch evaluator that uses the tokenizer and cache.

    Deduplicates positions, evaluates only cache misses, and populates
    the cache with results.

    Usage::

        evaluator = BatchEvaluator()
        fens = ["fen1", "fen2", ...]
        results = evaluator.evaluate_batch(fens, eval_fn=my_evaluator)
    """

    def __init__(self, cache_maxsize: int = 100_000) -> None:
        self.tokenizer = FENTokenizer()
        self.cache = TensorEvaluationCache(maxsize=cache_maxsize)

    def evaluate_batch(
        self,
        fens: list[str],
        eval_fn: Optional[callable] = None,
    ) -> list[dict]:
        """
        Evaluate a batch of FENs.

        Returns a list of dicts with keys: fen, hash, tensor, evaluation,
        best_move, cached.
        """
        results = []
        for fen in fens:
            key = fen_hash(fen)
            cached = self.cache.get(key)

            if cached is not None:
                results.append({
                    "fen": fen,
                    "hash": key,
                    "tensor": cached.tensor,
                    "evaluation": cached.evaluation,
                    "best_move": cached.best_move,
                    "cached": True,
                })
            else:
                tensor = self.tokenizer.fen_to_tensor(fen)
                evaluation = None
                best_move = None

                if eval_fn is not None:
                    eval_result = eval_fn(chess.Board(fen))
                    if isinstance(eval_result, dict):
                        evaluation = eval_result.get("evaluation")
                        best_move = eval_result.get("best_move")
                    elif isinstance(eval_result, (int, float)):
                        evaluation = eval_result

                self.cache.put(key, tensor, evaluation=evaluation, best_move=best_move)

                results.append({
                    "fen": fen,
                    "hash": key,
                    "tensor": tensor,
                    "evaluation": evaluation,
                    "best_move": best_move,
                    "cached": False,
                })

        return results

    def get_tensors(self, fens: list[str]) -> np.ndarray:
        """Get tensors for a list of FENs, using cache where possible."""
        tensors = []
        for fen in fens:
            key = fen_hash(fen)
            cached = self.cache.get(key)
            if cached is not None:
                tensors.append(cached.tensor)
            else:
                tensor = self.tokenizer.fen_to_tensor(fen)
                self.cache.put(key, tensor)
                tensors.append(tensor)
        return np.stack(tensors) if tensors else np.zeros((0, 8, 8, 14), dtype=np.float32)
