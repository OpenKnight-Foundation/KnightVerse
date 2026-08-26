"""Intent parser for Natural Language Agent interface.

This module parses user input to recognize intent and extract relevant entities
for chess analysis requests.

Includes a natural language voice command parser for converting spoken chess
moves (e.g. "Knight takes d5") into standard UCI notation.
"""

from __future__ import annotations

import re
import hashlib
import time
from dataclasses import dataclass, field
from typing import Optional, Dict, Any, List

import chess

from gpu_worker.nl_models import ComplexityLevel, IntentRecognition, IntentType


# In-memory cache for intent recognition results, TTL 24 hours (86400 seconds)
INTENT_CACHE: Dict[str, Dict[str, Any]] = {}
CACHE_TTL = 86400  # 24 hours in seconds


def _cleanup_expired_cache_entries() -> None:
    """Remove expired cache entries to prevent memory leaks."""
    current_time = time.time()
    expired_keys = [
        key for key, value in INTENT_CACHE.items()
        if current_time - value['timestamp'] > CACHE_TTL
    ]
    for key in expired_keys:
        del INTENT_CACHE[key]


def _get_cache_key(user_input: str) -> str:
    """Generate a consistent cache key from user input."""
    # Use hash to keep keys manageable regardless of input length
    return hashlib.sha256(user_input.lower().strip().encode()).hexdigest()


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
    # Check cache first for exact string match
    cache_key = _get_cache_key(user_input)
    _cleanup_expired_cache_entries()  # Clean up expired entries before checking cache
    
    # Return cached result if available and not expired
    if cache_key in INTENT_CACHE:
        cached = INTENT_CACHE[cache_key]
        if time.time() - cached['timestamp'] < CACHE_TTL:
            return IntentRecognition(
                intent=cached['intent'],
                confidence=cached['confidence'],
                reasoning=cached['reasoning']
            )
    
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
    
    # Cache the result
    result = IntentRecognition(
        intent=best_intent,
        confidence=best_confidence,
        reasoning=best_reasoning,
    )
    
    INTENT_CACHE[cache_key] = {
        'intent': best_intent,
        'confidence': best_confidence,
        'reasoning': best_reasoning,
        'timestamp': time.time()
    }
    
    return result


def detect_complexity(user_input: str) -> ComplexityLevel:
    """Detect the desired complexity level from user input.
    
    Args:
        user_input: The user's natural language input.
        
    Returns:
        ComplexityLevel enum value.
    """
    # Check cache first for exact string match
    cache_key = _get_cache_key(user_input)
    _cleanup_expired_cache_entries()  # Clean up expired entries before checking cache
    
    # Return cached result if available and not expired
    if cache_key in INTENT_CACHE:
        cached = INTENT_CACHE[cache_key]
        if time.time() - cached['timestamp'] < CACHE_TTL and 'complexity' in cached:
            return cached['complexity']
    
    user_input_lower = user_input.lower()
    
    # Check for advanced keywords first
    detected_complexity = ComplexityLevel.INTERMEDIATE  # Default
    for pattern in COMPLEXITY_KEYWORDS[ComplexityLevel.ADVANCED]:
        if re.search(pattern, user_input_lower):
            detected_complexity = ComplexityLevel.ADVANCED
            break
    else:
        # Check for beginner keywords only if not already detected as advanced
        for pattern in COMPLEXITY_KEYWORDS[ComplexityLevel.BEGINNER]:
            if re.search(pattern, user_input_lower):
                detected_complexity = ComplexityLevel.BEGINNER
                break
    
    # Update cache with complexity if exists, or create new entry
    if cache_key in INTENT_CACHE:
        INTENT_CACHE[cache_key]['complexity'] = detected_complexity
    else:
        INTENT_CACHE[cache_key] = {
            'complexity': detected_complexity,
            'timestamp': time.time()
        }
    
    return detected_complexity


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


# ---------------------------------------------------------------------------
# Voice Command Parser – Natural Language Move Recognition
# ---------------------------------------------------------------------------

# Phonetic normalization map: common speech-to-text typos and variations
PHONETIC_MAP: Dict[str, str] = {
    "night": "knight",
    "nite": "knight",
    "nite to": "knight to",
    "nite takes": "knight takes",
    "see four": "c4",
    "see two": "c2",
    "sea four": "c4",
    "sea two": "c2",
    "bee five": "b5",
    "bee four": "b4",
    "dee five": "d5",
    "dee four": "d4",
    "eff four": "f4",
    "eff three": "f3",
    "aitch four": "h4",
    "aitch five": "h5",
    "jee one": "g1",
    "jay one": "g1",
    "gee one": "g1",
    "gee two": "g2",
    "ay four": "a4",
    "ay five": "a5",
    "e for": "e4",
    "d for": "d4",
    "g for": "g4",
    "c for": "c4",
    "short castle": "castles kingside",
    "long castle": "castles queenside",
    "castle kingside": "castles kingside",
    "castle queenside": "castles queenside",
    "castle king side": "castles kingside",
    "castle queen side": "castles queenside",
    "castle short": "castles kingside",
    "castle long": "castles queenside",
}

# Piece name to chess piece character
PIECE_NAMES: Dict[str, str] = {
    "knight": "N",
    "knights": "N",
    "bishop": "B",
    "bishops": "B",
    "rook": "R",
    "rooks": "R",
    "castle": "R",
    "castles": "R",
    "queen": "Q",
    "queens": "Q",
    "king": "K",
    "kings": "K",
    "pawn": "",
    "pawns": "",
}

# Square name normalization
SQUARE_NAMES = set(chess.SQUARE_NAMES) if hasattr(chess, "SQUARE_NAMES") else {
    "a1","b1","c1","d1","e1","f1","g1","h1",
    "a2","b2","c2","d2","e2","f2","g2","h2",
    "a3","b3","c3","d3","e3","f3","g3","h3",
    "a4","b4","c4","d4","e4","f4","g4","h4",
    "a5","b5","c5","d5","e5","f5","g5","h5",
    "a6","b6","c6","d6","e6","f6","g6","h6",
    "a7","b7","c7","d7","e7","f7","g7","h7",
    "a8","b8","c8","d8","e8","f8","g8","h8",
}


def normalize_phonetic(input_text: str) -> str:
    """Normalize common speech-to-text errors and phonetic variations.

    Uses word-boundary-aware replacement so that 'night' inside 'knight'
    is not incorrectly replaced.

    Args:
        input_text: Raw speech transcript.

    Returns:
        Normalized text with phonetic corrections applied.
    """
    result = input_text.lower().strip()

    # Apply phonetic corrections with word boundaries (longer phrases first)
    for wrong, correct in sorted(PHONETIC_MAP.items(), key=lambda x: -len(x[0])):
        result = re.sub(r"\b" + re.escape(wrong) + r"\b", correct, result)

    return result


def _find_square(name: str) -> Optional[str]:
    """Try to resolve a square name, handling phonetic variants."""
    name = name.lower().strip()
    if name in SQUARE_NAMES:
        return name
    # Strip trailing 's' for plurals like 'd5s'
    if len(name) > 2 and name[-1] == "s" and name[:-1] in SQUARE_NAMES:
        return name[:-1]
    return None


@dataclass
class VoiceMoveResult:
    """Structured result from a voice command parse."""

    uci: str
    san: str
    confidence: float
    needs_disambiguation: bool = False
    disambiguation_options: List[str] = field(default_factory=list)
    raw_input: str = ""
    explanation: str = ""


def parse_voice_move(
    transcript: str,
    board_fen: Optional[str] = None,
) -> VoiceMoveResult:
    """Parse a natural language voice command into a UCI chess move.

    Handles phrasings like:
        "Knight to f3"           -> g1f3
        "Take on d5 with bishop" -> c4d5
        "Short castle"           -> e1g1
        "Queen takes Queen check" -> d1d8

    Args:
        transcript: Raw speech transcript string.
        board_fen: Optional FEN string for the current board position.
            Used for disambiguation.

    Returns:
        VoiceMoveResult with UCI move, SAN, and confidence.
    """
    raw = transcript
    normalized = normalize_phonetic(transcript)

    # Check for castling
    castle_result = _try_parse_castle(normalized, board_fen)
    if castle_result is not None:
        castle_result.raw_input = raw
        return castle_result

    # Try standard voice move patterns
    result = _try_parse_piece_to_square(normalized, board_fen)
    if result is not None:
        result.raw_input = raw
        return result

    result = _try_parse_capture(normalized, board_fen)
    if result is not None:
        result.raw_input = raw
        return result

    result = _try_parse_pawn_move(normalized, board_fen)
    if result is not None:
        result.raw_input = raw
        return result

    # Try 'play/move/go' prefix patterns
    result = _try_parse_play_prefix(normalized, board_fen)
    if result is not None:
        result.raw_input = raw
        return result

    # Fallback: try to extract UCI directly from the transcript
    result = _try_parse_raw_uci(normalized, board_fen)
    if result is not None:
        result.raw_input = raw
        return result

    return VoiceMoveResult(
        uci="",
        san="",
        confidence=0.0,
        raw_input=raw,
        explanation="Could not parse the voice command into a valid chess move.",
    )


def _try_parse_castle(
    text: str, board_fen: Optional[str]
) -> Optional[VoiceMoveResult]:
    """Try to parse a castling command."""
    board = chess.Board(board_fen) if board_fen else chess.Board()

    kingside_patterns = [
        r"\b(?:short|king(?:'s|s)?)\s*(?:castle|castles?)\b",
        r"\bcastle(?:s)?\s+(?:short|king(?:'s|s)?)\b",
        r"\bcastles?\s+king(?:s|'s)?\s*side\b",
    ]
    queenside_patterns = [
        r"\b(?:long|queen(?:'s|s)?)\s*(?:castle|castles?)\b",
        r"\bcastle(?:s)?\s+(?:long|queen(?:'s|s)?)\b",
        r"\bcastles?\s+queen(?:s|'s)?\s*side\b",
    ]

    for pattern in kingside_patterns:
        if re.search(pattern, text):
            if board.has_kingside_castling_rights(board.turn):
                uci = "e1g1" if board.turn == chess.WHITE else "e8g8"
                san = board.san(chess.Move.from_uci(uci))
                return VoiceMoveResult(
                    uci=uci,
                    san=san,
                    confidence=0.98,
                    explanation="Kingside castling",
                )
            return VoiceMoveResult(
                uci="",
                san="",
                confidence=0.0,
                explanation="Kingside castling is not available in this position.",
            )

    for pattern in queenside_patterns:
        if re.search(pattern, text):
            if board.has_queenside_castling_rights(board.turn):
                uci = "e1c1" if board.turn == chess.WHITE else "e8c8"
                san = board.san(chess.Move.from_uci(uci))
                return VoiceMoveResult(
                    uci=uci,
                    san=san,
                    confidence=0.98,
                    explanation="Queenside castling",
                )
            return VoiceMoveResult(
                uci="",
                san="",
                confidence=0.0,
                explanation="Queenside castling is not available in this position.",
            )

    return None


def _try_parse_piece_to_square(
    text: str, board_fen: Optional[str]
) -> Optional[VoiceMoveResult]:
    """Parse patterns like 'Knight to f3', 'Bishop to c4', 'knight f3'."""
    # Pattern 1: piece to/on/at square
    pattern = (
        r"\b(knight|bishop|rook|queen|king|castle)s?\s+"
        r"(?:to|on|at)\s+(\w\d)\b"
    )
    match = re.search(pattern, text)
    if not match:
        # Pattern 2: shorthand 'knight f3', 'bishop c4' (no preposition)
        pattern_short = (
            r"\b(knight|bishop|rook|queen|king|castle)s?\s+(\w\d)\b"
        )
        match = re.search(pattern_short, text)
    if not match:
        # Pattern 3: 'play/move knight to f3'
        pattern_play = (
            r"\b(?:play|move|go)\s+"
            r"(knight|bishop|rook|queen|king|castle)s?\s+"
            r"(?:to|on|at)?\s*(\w\d)\b"
        )
        match = re.search(pattern_play, text)
    if not match:
        return None

    piece_name = match.group(1).lower()
    target = _find_square(match.group(2))
    if target is None:
        return VoiceMoveResult(
            uci="", san="", confidence=0.0,
            explanation=f"Could not resolve square '{match.group(2)}'.",
        )

    piece_char = PIECE_NAMES.get(piece_name, "")
    board = chess.Board(board_fen) if board_fen else chess.Board()

    # Find all legal moves of this piece type to the target
    target_sq = chess.parse_square(target)
    matching_moves = []
    for move in board.legal_moves:
        if move.to_square == target_sq and _piece_type_matches(board, move, piece_char):
            matching_moves.append(move)

    if not matching_moves:
        return VoiceMoveResult(
            uci="", san="", confidence=0.0,
            explanation=f"No legal {piece_name} move to {target}.",
        )

    if len(matching_moves) == 1:
        move = matching_moves[0]
        uci = move.uci()
        san = board.san(move)
        confidence = 0.96
        if "check" in text:
            confidence = 0.99
        return VoiceMoveResult(
            uci=uci, san=san, confidence=confidence,
            explanation=f"{piece_name.title()} to {target}",
        )

    # Disambiguation needed
    options = [m.uci() for m in matching_moves]
    return VoiceMoveResult(
        uci="", san="", confidence=0.5,
        needs_disambiguation=True,
        disambiguation_options=options,
        explanation=(
            f"Multiple {piece_name}s can move to {target}. "
            f"Please specify which one: {', '.join(options)}"
        ),
    )


def _try_parse_capture(
    text: str, board_fen: Optional[str]
) -> Optional[VoiceMoveResult]:
    """Parse capture patterns like 'Take on d5 with bishop' or 'Queen takes e5'."""
    # Pattern: [piece] takes [square] [optional piece]
    pattern = (
        r"\b(takes?|captures?|takes?)\s+(?:on\s+)?(\w\d)\b"
    )
    match = re.search(pattern, text)
    if not match:
        return None

    target = _find_square(match.group(2))
    if target is None:
        return VoiceMoveResult(
            uci="", san="", confidence=0.0,
            explanation=f"Could not resolve square '{match.group(2)}'.",
        )

    # Look for piece name before 'takes' or after 'with'
    before_match = text[:match.start()]
    with_match = re.search(r"\bwith\s+(\w+)\b", text)

    piece_name = None
    if with_match:
        piece_name = with_match.group(1).lower()
    else:
        piece_search = re.search(
            r"\b(knight|bishop|rook|queen|king|pawn|castle)s?\s*$",
            before_match,
        )
        if piece_search:
            piece_name = piece_search.group(1).lower()

    board = chess.Board(board_fen) if board_fen else chess.Board()
    target_sq = chess.parse_square(target)

    piece_char = PIECE_NAMES.get(piece_name, "") if piece_name else None

    matching_moves = []
    for move in board.legal_moves:
        if move.to_square == target_sq and board.is_capture(move):
            if piece_char is None or _piece_type_matches(board, move, piece_char):
                matching_moves.append(move)

    if not matching_moves:
        return VoiceMoveResult(
            uci="", san="", confidence=0.0,
            explanation=f"No legal capture on {target}.",
        )

    if len(matching_moves) == 1:
        move = matching_moves[0]
        uci = move.uci()
        san = board.san(move)
        return VoiceMoveResult(
            uci=uci, san=san, confidence=0.97,
            explanation=f"Capture on {target}",
        )

    options = [m.uci() for m in matching_moves]
    return VoiceMoveResult(
        uci="", san="", confidence=0.5,
        needs_disambiguation=True,
        disambiguation_options=options,
        explanation=(
            f"Multiple pieces can capture on {target}. "
            f"Please specify: {', '.join(options)}"
        ),
    )


def _try_parse_pawn_move(
    text: str, board_fen: Optional[str]
) -> Optional[VoiceMoveResult]:
    """Parse pawn moves like 'pawn to e4', 'e4', 'push e4'."""
    # 'pawn to <square>'
    match = re.search(r"\bpawn\s+(?:to|on|at)\s+(\w\d)\b", text)
    if not match:
        # bare square name like just "e4"
        match = re.search(r"^\s*(\w\d)\s*$", text)
    if not match:
        # 'push <square>'
        match = re.search(r"\bpush(?:es|ed)?\s+(\w\d)\b", text)
    if not match:
        return None

    target = _find_square(match.group(1))
    if target is None:
        return VoiceMoveResult(
            uci="", san="", confidence=0.0,
            explanation=f"Could not resolve square '{match.group(1)}'.",
        )

    board = chess.Board(board_fen) if board_fen else chess.Board()
    target_sq = chess.parse_square(target)

    pawn_moves = [
        m for m in board.legal_moves
        if m.to_square == target_sq and board.piece_type_at(m.from_square) == chess.PAWN
    ]

    if not pawn_moves:
        return VoiceMoveResult(
            uci="", san="", confidence=0.0,
            explanation=f"No legal pawn move to {target}.",
        )

    if len(pawn_moves) == 1:
        move = pawn_moves[0]
        uci = move.uci()
        san = board.san(move)
        return VoiceMoveResult(
            uci=uci, san=san, confidence=0.93,
            explanation=f"Pawn to {target}",
        )

    # Could be a capture or promotion
    if len(pawn_moves) == 2:
        # Likely one push and one capture, or two pushes (double push)
        move = pawn_moves[0]
        uci = move.uci()
        san = board.san(move)
        return VoiceMoveResult(
            uci=uci, san=san, confidence=0.85,
            explanation=f"Pawn move to {target}",
        )

    options = [m.uci() for m in pawn_moves]
    return VoiceMoveResult(
        uci="", san="", confidence=0.5,
        needs_disambiguation=True,
        disambiguation_options=options,
        explanation=f"Multiple pawn moves to {target}. Options: {', '.join(options)}",
    )


def _try_parse_play_prefix(
    text: str, board_fen: Optional[str]
) -> Optional[VoiceMoveResult]:
    """Parse 'play/move/go <piece> to <square>' or 'play/move/go <square>' patterns."""
    # play/move/go + piece to square
    pattern_piece = (
        r"\b(?:play|move|go)\s+"
        r"(knight|bishop|rook|queen|king|castle)s?\s+"
        r"(?:to|on|at)?\s*(\w\d)\b"
    )
    match = re.search(pattern_piece, text)
    if match:
        piece_name = match.group(1).lower()
        target = _find_square(match.group(2))
        if target is None:
            return VoiceMoveResult(
                uci="", san="", confidence=0.0,
                explanation=f"Could not resolve square '{match.group(2)}'.",
            )
        piece_char = PIECE_NAMES.get(piece_name, "")
        board = chess.Board(board_fen) if board_fen else chess.Board()
        target_sq = chess.parse_square(target)
        matching_moves = [
            m for m in board.legal_moves
            if m.to_square == target_sq and _piece_type_matches(board, m, piece_char)
        ]
        if len(matching_moves) == 1:
            move = matching_moves[0]
            return VoiceMoveResult(
                uci=move.uci(), san=board.san(move), confidence=0.95,
                explanation=f"{piece_name.title()} to {target}",
            )
        if len(matching_moves) > 1:
            return VoiceMoveResult(
                uci="", san="", confidence=0.5,
                needs_disambiguation=True,
                disambiguation_options=[m.uci() for m in matching_moves],
                explanation=f"Multiple {piece_name}s can move to {target}.",
            )

    # play/move/go + bare square (pawn move)
    pattern_square = r"\b(?:play|move|go)\s+(\w\d)\b"
    match = re.search(pattern_square, text)
    if match:
        target = _find_square(match.group(1))
        if target:
            board = chess.Board(board_fen) if board_fen else chess.Board()
            target_sq = chess.parse_square(target)
            pawn_moves = [
                m for m in board.legal_moves
                if m.to_square == target_sq and board.piece_type_at(m.from_square) == chess.PAWN
            ]
            if pawn_moves:
                move = pawn_moves[0]
                return VoiceMoveResult(
                    uci=move.uci(), san=board.san(move), confidence=0.90,
                    explanation=f"Pawn to {target}",
                )

    return None


def _try_parse_raw_uci(text: str, board_fen: Optional[str] = None) -> Optional[VoiceMoveResult]:
    """Try to parse a raw UCI string like 'e2e4' from the text."""
    match = re.search(r"\b([a-h][1-8][a-h][1-8][qrbn]?)\b", text)
    if not match:
        return None

    uci = match.group(1)
    try:
        move = chess.Move.from_uci(uci)
        board = chess.Board(board_fen) if board_fen else chess.Board()
        if move in board.legal_moves:
            san = board.san(move)
            return VoiceMoveResult(
                uci=uci, san=san, confidence=0.90,
                explanation=f"Raw UCI move: {uci}",
            )
    except (ValueError, chess.InvalidMoveError):
        pass

    return None


def _piece_type_matches(
    board: chess.Board, move: chess.Move, piece_char: str
) -> bool:
    """Check if a move matches the given piece character."""
    if not piece_char:
        return True  # No filter specified
    piece = board.piece_type_at(move.from_square)
    type_map = {
        "N": chess.KNIGHT, "B": chess.BISHOP, "R": chess.ROOK,
        "Q": chess.QUEEN, "K": chess.KING, "": chess.PAWN,
    }
    return piece == type_map.get(piece_char, None)