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

from gpu_worker.nl_models import ComplexityLevel, IntentRecognition, IntentType


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


# ---------------------------------------------------------------------------
# Intent recognition
# ---------------------------------------------------------------------------

# Pattern definitions for intent recognition
INTENT_PATTERNS = {
    IntentType.ANALYZE_POSITION: [
        r'\b(analyz\w*|evaluat\w*|assess\w*|review\w*)\b.*\b(position|board|situation|game)\b',
        r'\b(what|how)\b.*\b(is the\s+)?(assessment|evaluation|status)\b.*\b(position|board)\b',
        r'\b(analysis|evaluation|assessment)\b',
    ],
    IntentType.SUGGEST_MOVE: [
        r'\b(suggest|recommend|advise|propose)\b.*\b(move|play)\b',
        r'\b(what|which)\b.*\b(best|good|should)\b.*\b(move|play)\b',
        r'\bwhat should i\b.*\b(play|do)\b',
        r'\bbest move\b',
    ],
    IntentType.EXPLAIN_MOVE: [
        r'\b(explain|why|reason)\b.*\b(move|played|chosen)\b',
        r'\bwhy\b.*\b(is|was)\b.*\b(good|bad|best|strong|weak)\b',
        r'\bwhat makes\b.*\b(move|position)\b',
    ],
    IntentType.GET_HINT: [
        r'\b(hint|help|clue|tip)s?\b',
        r'\bgive me\b.*\b(hint|help)\b',
        r'\b(stuck|don.*t know|unsure)\b',
    ],
    IntentType.COMPARE_MOVES: [
        r'\b(compare|difference|versus|vs)\b',
        r'\bwhich is better\b',
    ],
    IntentType.LEARN_CONCEPT: [
        r'\b(what is|explain|teach me|how does|tell me about)\b.*\b(tactic|strategy|opening|endgame|fork|pin|skewer|concept|mate|checkmate)\b',
        r'\b(learn|understand)\b.*\b(chess|position|move)\b',
        r'\b(what is a|what is an)\b',
        r'\btell me about\b',
    ],
}

# Keywords for complexity detection
COMPLEXITY_KEYWORDS = {
    ComplexityLevel.BEGINNER: [
        r'\b(basic|simple|easy|beginner|newbie)\b',
        r'\b(explain like i.*m 5|eli5)\b',
        r'\b(don.*t understand|confused)\b',
    ],
    ComplexityLevel.ADVANCED: [
        r'\b(advanced|expert|deep|detailed|complex)\b',
        r'\b(variation|line|tactical|positional)\b',
        r'\b(engine|evaluation|centipawn)\b',
    ],
}


def recognize_intent(user_input: str) -> IntentRecognition:
    """Recognize the intent from user input using pattern matching.
    
    Args:
        user_input: The user's natural language input.
        
    Returns:
        IntentRecognition object with identified intent and confidence.
    """
    user_input_lower = user_input.lower()
    
    best_intent = IntentType.UNKNOWN
    best_confidence = 0.0
    best_reasoning = ""
    
    for intent, patterns in INTENT_PATTERNS.items():
        for pattern in patterns:
            match = re.search(pattern, user_input_lower)
            if match:
                # Calculate confidence based on match quality
                confidence = _calculate_confidence(match, user_input_lower, intent)
                if confidence > best_confidence:
                    best_confidence = confidence
                    best_intent = intent
                    best_reasoning = f"Matched pattern: {pattern} with confidence {confidence}"
    
    return IntentRecognition(
        intent=best_intent,
        confidence=best_confidence,
        reasoning=best_reasoning,
    )


def detect_complexity(user_input: str) -> ComplexityLevel:
    """Detect the desired complexity level from user input.
    
    Args:
        user_input: The user's natural language input.
        
    Returns:
        ComplexityLevel enum value.
    """
    user_input_lower = user_input.lower()
    
    # Check for advanced keywords first
    for pattern in COMPLEXITY_KEYWORDS[ComplexityLevel.ADVANCED]:
        if re.search(pattern, user_input_lower):
            return ComplexityLevel.ADVANCED
    
    # Check for beginner keywords
    for pattern in COMPLEXITY_KEYWORDS[ComplexityLevel.BEGINNER]:
        if re.search(pattern, user_input_lower):
            return ComplexityLevel.BEGINNER
    
    # Default to intermediate
    return ComplexityLevel.INTERMEDIATE


def extract_fen_from_input(user_input: str) -> Optional[str]:
    """Extract FEN string from user input if present.
    
    Args:
        user_input: The user's natural language input.
        
    Returns:
        FEN string if found, None otherwise.
    """
    # FEN pattern: 6 space-separated segments (piece placement, side to move, castling, en passant, halfmove, fullmove)
    fen_pattern = r'(?:[rnbqkpRNBQKP1-8]{1,8}/){7}[rnbqkpRNBQKP1-8]{1,8}\s+[wb]\s+(?:[KQkqA-Ha-h]+|-)\s+(?:[a-h][36]|-)(?:\s+\d+\s+\d+)?'
    match = re.search(fen_pattern, user_input)
    if match:
        return match.group(0).strip()
    return None


def extract_moves_from_input(user_input: str) -> list[str]:
    """Extract chess moves from user input.
    
    Args:
        user_input: The user's natural language input.
        
    Returns:
        List of chess moves in algebraic notation.
    """
    # Simple pattern for algebraic notation (e.g., e4, Nf3, O-O, Qxd5)
    move_pattern = r'\b([KQRBN]?[a-h]?[1-8]?x?[a-h][1-8](?:=[QRBN])?[+#]?)(?!\w)'
    moves = re.findall(move_pattern, user_input)
    
    # Filter out common false positives
    chess_moves = []
    for move in moves:
        if len(move) >= 2 and not move.isdigit():
            chess_moves.append(move)
    
    return chess_moves


def _calculate_confidence(match: re.Match, text: str, intent: IntentType) -> float:
    """Calculate confidence score for an intent match.
    
    Args:
        match: The regex match object.
        text: The full user input text.
        intent: The recognized intent type.
        
    Returns:
        Confidence score between 0.0 and 1.0.
    """
    # Base confidence from match length relative to text
    match_length = match.end() - match.start()
    text_length = len(text)
    
    # Use proportion of match relative to text length
    base_confidence = match_length / max(text_length, 1)
    
    # Boost confidence for exact keyword matches
    matched_text = match.group(0).lower()
    
    boost_keywords = []
    if intent == IntentType.ANALYZE_POSITION:
        boost_keywords = ['analyze', 'evaluate', 'assessment', 'evaluation']
    elif intent == IntentType.SUGGEST_MOVE:
        boost_keywords = ['best move', 'suggest', 'recommend']
    elif intent == IntentType.EXPLAIN_MOVE:
        boost_keywords = ['explain why', 'why is', 'what makes']
    elif intent == IntentType.GET_HINT:
        boost_keywords = ['hint', 'stuck', 'help', 'tips']
    elif intent == IntentType.COMPARE_MOVES:
        boost_keywords = ['compare', 'difference', 'which is better']
    elif intent == IntentType.LEARN_CONCEPT:
        boost_keywords = ['what is a', 'what is', 'teach me', 'tell me about']
        
    if any(keyword in matched_text for keyword in boost_keywords):
        base_confidence += 0.25
        
    # Extra boost for GET_HINT if strong keywords are present anywhere in the text
    if intent == IntentType.GET_HINT and any(kw in text.lower() for kw in ['stuck', 'hint', 'help', 'tips']):
        base_confidence += 0.3
        
    return round(min(1.0, base_confidence), 2)
