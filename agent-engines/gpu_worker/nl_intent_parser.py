"""
Natural Language Intent Parser with Move Guardrails for Chess.

Parses LLM response text for chess notation (SAN, UCI, piece-to-square),
validates every detected move against the current board state using
python-chess, and defends against prompt injection via opponent chat.
"""

from __future__ import annotations

import re
import time
from dataclasses import dataclass, field
from typing import Optional

import chess


# ---------------------------------------------------------------------------
# Regex patterns for common chess notation formats
# ---------------------------------------------------------------------------

# Standard Algebraic Notation (SAN): e4, Nf3, Bxc6, O-O, Qd1+, Raxd1#
# Broken into two alternatives to avoid false positives like "e2e4" (UCI).
#   Pawn:  [a-h]x?[a-h][1-8]  or  [a-h][1-8]
#   Piece: [KQRBN][a-h]?[1-8]?x?[a-h][1-8]
_SAN_PATTERN = re.compile(
    r"\b(?P<san>"
    r"(?:O-O(?:-O)?"                     # castling
    r"|[KQRBN][a-h]?[1-8]?x?[a-h][1-8]"  # piece + optional disambiguation
    r"|[a-h]x?[a-h][1-8]"                 # pawn capture
    r"|[a-h][1-8])"                       # pawn advance
    r"(?:=[QRBN])?[+#]?"                  # promotion + check/checkmate
    r")\b"
)

# UCI coordinate notation: e2e4, g1f3, e7e8q
_UCI_PATTERN = re.compile(
    r"\b(?P<uci>[a-h][1-8][a-h][1-8][qrbn]?)\b"
)

# Piece-to-square textual coordinates: "Knight to f3", "pawn on e4"
_PIECE_TO_SQUARE_PATTERN = re.compile(
    r"\b(?P<piece>king|queen|rook|bishop|knight|pawn)s?\s+"
    r"(?:to|on|moves?\s+to|captures?\s+(?:on\s+)?)\s+"
    r"(?P<square>[a-h][1-8])\b",
    re.IGNORECASE,
)

# Map textual piece names to chess piece characters for SAN construction
_PIECE_NAME_MAP: dict[str, str] = {
    "king": "K",
    "queen": "Q",
    "rook": "R",
    "bishop": "B",
    "knight": "N",
    "pawn": "",
}

# Prompt-injection signatures commonly seen in adversarial chat messages
_INJECTION_SIGNATURES: list[str] = [
    "ignore previous instructions",
    "ignore all prior",
    "disregard your instructions",
    "forget your rules",
    "you are now",
    "system prompt",
    "override",
    "new instructions:",
    "act as",
    "pretend you are",
    "jailbreak",
    "do anything now",
    "DAN mode",
    "bypass",
    "unfiltered",
    "no restrictions",
    "ignore safety",
]


# ---------------------------------------------------------------------------
# Data classes
# ---------------------------------------------------------------------------

@dataclass
class DetectedMove:
    """A single move detected inside LLM text."""
    raw: str
    notation: str          # canonical SAN string
    is_legal: bool
    corrected_to: Optional[str] = None
    source_format: str = "SAN"  # SAN | UCI | PIECE_COORD


@dataclass
class GuardrailResult:
    """Outcome of guarding a single LLM response."""
    original_text: str
    sanitized_text: str
    detected_moves: list[DetectedMove] = field(default_factory=list)
    illegal_moves_count: int = 0
    corrections_applied: int = 0
    injection_detected: bool = False
    processing_time_ms: float = 0.0


# ---------------------------------------------------------------------------
# MoveGuardrail
# ---------------------------------------------------------------------------

class MoveGuardrail:
    """
    Validates and sanitizes every chess move mention in LLM output text.

    Usage::

        guardrail = MoveGuardrail()
        board = chess.Board()
        result = guardrail.validate_response(llm_text, board)
        if result.injection_detected or result.illegal_moves_count:
            # handle accordingly
    """

    def __init__(
        self,
        *,
        max_correction_depth: int = 3,
        injection_signatures: Optional[list[str]] = None,
    ) -> None:
        self.max_correction_depth = max_correction_depth
        self._injection_sigs = (
            injection_signatures
            if injection_signatures is not None
            else _INJECTION_SIGNATURES
        )

    # -- public API --------------------------------------------------------

    def validate_response(self, text: str, board: chess.Board) -> GuardrailResult:
        """Run the full guardrail pipeline on *text* against *board*."""
        t0 = time.perf_counter()
        result = GuardrailResult(original_text=text, sanitized_text=text)

        # 1. Prompt-injection defence
        if self._detect_injection(text):
            result.injection_detected = True
            result.sanitized_text = "[BLOCKED: prompt injection detected]"
            result.processing_time_ms = (time.perf_counter() - t0) * 1000
            return result

        # 2. Detect moves from all notation styles
        moves = self._detect_all_moves(text, board)
        result.detected_moves = moves

        # 3. Validate legality and attempt corrections
        sanitized = text
        for dm in moves:
            if not dm.is_legal:
                result.illegal_moves_count += 1
                correction = self._find_nearest_legal(dm.notation, board)
                if correction:
                    dm.corrected_to = correction
                    dm.is_legal = False
                    sanitized = sanitized.replace(dm.raw, correction)
                    result.corrections_applied += 1

        result.sanitized_text = sanitized
        result.processing_time_ms = (time.perf_counter() - t0) * 1000
        return result

    def validate_move(self, move_san: str, board: chess.Board) -> DetectedMove:
        """Validate a single SAN move against *board*."""
        san = move_san.strip()
        is_legal = self._is_legal_san(san, board)
        return DetectedMove(
            raw=san,
            notation=san,
            is_legal=is_legal,
            source_format="SAN",
        )

    def sanitize_input(self, user_text: str) -> str:
        """Strip prompt-injection attempts from user chat before passing to LLM."""
        if self._detect_injection(user_text):
            return ""
        return user_text

    # -- internals ---------------------------------------------------------

    def _detect_injection(self, text: str) -> bool:
        lower = text.lower()
        return any(sig in lower for sig in self._injection_sigs)

    @staticmethod
    def _is_legal_san(san: str, board: chess.Board) -> bool:
        """Check if a SAN string represents a legal move on *board*."""
        try:
            board.parse_san(san)
            return True
        except (ValueError, chess.InvalidMoveError):
            return False

    @staticmethod
    def _legal_sans(board: chess.Board) -> set[str]:
        """Return the set of all legal SAN strings for the position."""
        return {board.san(m) for m in board.legal_moves}

    def _detect_all_moves(
        self, text: str, board: chess.Board
    ) -> list[DetectedMove]:
        moves: list[DetectedMove] = []
        seen: set[str] = set()
        legal_sans = self._legal_sans(board)

        # SAN moves
        for m in _SAN_PATTERN.finditer(text):
            raw = m.group("san")
            if not raw or raw in seen:
                continue
            seen.add(raw)
            is_legal = raw in legal_sans
            moves.append(DetectedMove(
                raw=raw, notation=raw, is_legal=is_legal, source_format="SAN",
            ))

        # UCI moves
        for m in _UCI_PATTERN.finditer(text):
            raw = m.group("uci")
            if not raw or raw in seen:
                continue
            seen.add(raw)
            try:
                uci_move = chess.Move.from_uci(raw)
                is_legal = uci_move in board.legal_moves
                san = board.san(uci_move) if is_legal else raw
            except ValueError:
                is_legal = False
                san = raw
            moves.append(DetectedMove(
                raw=raw, notation=san, is_legal=is_legal, source_format="UCI",
            ))

        # Piece-to-square textual coordinates
        for m in _PIECE_TO_SQUARE_PATTERN.finditer(text):
            piece_char = _PIECE_NAME_MAP.get(m.group("piece").lower(), "")
            square = m.group("square")
            san_guess = f"{piece_char}{square}"
            if san_guess in seen:
                continue
            seen.add(san_guess)
            is_legal = san_guess in legal_sans
            moves.append(DetectedMove(
                raw=m.group(0), notation=san_guess, is_legal=is_legal,
                source_format="PIECE_COORD",
            ))

        return moves

    def _find_nearest_legal(self, san: str, board: chess.Board) -> Optional[str]:
        """Find the closest legal move for an illegal SAN (best-effort)."""
        legal_sans = self._legal_sans(board)
        for _ in range(self.max_correction_depth):
            candidates = self._generate_candidates(san, board)
            for cand in candidates:
                if cand in legal_sans:
                    return cand
        return None

    @staticmethod
    def _generate_candidates(san: str, board: chess.Board) -> list[str]:
        """Generate candidate SAN strings near the given move."""
        candidates: list[str] = []
        legal = {board.san(m) for m in board.legal_moves}

        # Try stripping check/checkmate symbols
        stripped = san.rstrip("+#")
        if stripped != san:
            candidates.append(stripped)

        # Try adding check/checkmate
        for suffix in ("", "+", "#"):
            candidate = stripped + suffix
            if candidate != san:
                candidates.append(candidate)

        # If a piece prefix is present, try removing disambiguation
        if len(stripped) >= 3 and stripped[0] in "KQRBN":
            # Try without the disambiguation file/rank
            candidates.append(stripped[0] + stripped[-2:])
            # Try without the piece letter (pawn)
            candidates.append(stripped[1:])

        return candidates
