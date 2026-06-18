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
