"""Intent parser for Natural Language Agent interface.

This module parses user input to recognize intent and extract relevant entities
for chess analysis requests.
"""

from __future__ import annotations

import re
from typing import Optional

from gpu_worker.nl_models import ComplexityLevel, IntentRecognition, IntentType


# Pattern definitions for intent recognition
INTENT_PATTERNS = {
    IntentType.ANALYZE_POSITION: [
        r'\b(analyz|evaluat|assess|review)\b.*\b(position|board|situation|game)\b',
        r'\b(what|how)\b.*\b(position|board)\b',
        r'\b(analysis|evaluation)\b',
    ],
    IntentType.SUGGEST_MOVE: [
        r'\b(suggest|recommend|advise|propose)\b.*\b(move|play)\b',
        r'\b(what|which)\b.*\b(move|play)\b.*\b(best|good|should)\b',
        r'\b(what should i\b.*\b(play|do)\b',
    ],
    IntentType.EXPLAIN_MOVE: [
        r'\b(explain|why|reason)\b.*\b(move|played|chosen)\b',
        r'\b(why\b.*\b(is|was)\b.*\b(good|bad|best|strong|weak)\b',
        r'\b(what makes\b.*\b(move|position)\b',
    ],
    IntentType.GET_HINT: [
        r'\b(hint|help|clue|tip)\b',
        r'\b(give me\b.*\b(hint|help)\b',
        r'\b(stuck|don.*t know|unsure)\b',
    ],
    IntentType.COMPARE_MOVES: [
        r'\b(compare|difference|versus|vs)\b.*\b(move|option)\b',
        r'\b(which is better\b',
        r'\b(move a\b.*\b(move b\b',
    ],
    IntentType.LEARN_CONCEPT: [
        r'\b(what is|explain|teach me|how does)\b.*\b(tactic|strategy|opening|endgame)\b',
        r'\b(learn|understand)\b.*\b(chess|position|move)\b',
        r'\b(tell me about\b',
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
                confidence = _calculate_confidence(match, user_input_lower)
                if confidence > best_confidence:
                    best_confidence = confidence
                    best_intent = intent
                    best_reasoning = f"Matched pattern: {pattern}"
    
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
    # FEN pattern: 8 ranks separated by /, with pieces, numbers, and side to move
    fen_pattern = r'([rnbqkpRNBQKP1-8]{1,8}/){7}[rnbqkpRNBQKP1-8]{1,8}\s+[wb]\s+'
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
    move_pattern = r'\b([KQRBN]?[a-h]?[1-8]?x?[a-h][1-8](?:=[QRBN])?[+#]?)\b'
    moves = re.findall(move_pattern, user_input)
    
    # Filter out common false positives
    chess_moves = []
    for move in moves:
        if len(move) >= 2 and not move.isdigit():
            chess_moves.append(move)
    
    return chess_moves


def _calculate_confidence(match: re.Match, text: str) -> float:
    """Calculate confidence score for an intent match.
    
    Args:
        match: The regex match object.
        text: The full user input text.
        
    Returns:
        Confidence score between 0.0 and 1.0.
    """
    # Base confidence from match length relative to text
    match_length = match.end() - match.start()
    text_length = len(text)
    
    # Longer matches in shorter texts are more confident
    base_confidence = min(1.0, match_length / max(text_length * 0.3, 1))
    
    # Boost confidence for exact keyword matches
    matched_text = match.group(0).lower()
    if any(keyword in matched_text for keyword in ['best move', 'analyze', 'explain']):
        base_confidence = min(1.0, base_confidence + 0.2)
    
    return round(base_confidence, 2)
