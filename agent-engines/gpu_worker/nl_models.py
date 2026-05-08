"""Natural Language Agent interface for XLMate chess platform.

This module provides a natural language interface for interacting with chess engines,
allowing users to request analysis, suggestions, and explanations using plain English.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from enum import Enum
from typing import Any, Optional


class IntentType(str, Enum):
    """Types of user intents that can be recognized."""
    
    ANALYZE_POSITION = "analyze_position"
    SUGGEST_MOVE = "suggest_move"
    EXPLAIN_MOVE = "explain_move"
    EVALUATE_GAME = "evaluate_game"
    LEARN_CONCEPT = "learn_concept"
    GET_HINT = "get_hint"
    COMPARE_MOVES = "compare_moves"
    UNKNOWN = "unknown"


class ComplexityLevel(str, Enum):
    """Response complexity levels for different user expertise."""
    
    BEGINNER = "beginner"
    INTERMEDIATE = "intermediate"
    ADVANCED = "advanced"


@dataclass
class NLAnalysisRequest:
    """Represents a natural language analysis request from a user."""
    
    user_input: str
    fen: Optional[str] = None
    move_history: list[str] = field(default_factory=list)
    intent: IntentType = IntentType.UNKNOWN
    complexity: ComplexityLevel = ComplexityLevel.INTERMEDIATE
    context: dict[str, Any] = field(default_factory=dict)
    request_id: Optional[str] = None
    
    def to_dict(self) -> dict[str, Any]:
        """Convert request to dictionary representation."""
        return {
            "user_input": self.user_input,
            "fen": self.fen,
            "move_history": self.move_history,
            "intent": self.intent.value,
            "complexity": self.complexity.value,
            "context": self.context,
            "request_id": self.request_id,
        }


@dataclass
class MoveAnalysis:
    """Detailed analysis of a specific move."""
    
    move: str
    evaluation: float
    depth: int
    is_best: bool = False
    explanation: str = ""
    alternatives: list[str] = field(default_factory=list)


@dataclass
class NLAnalysisResponse:
    """Response from the natural language agent."""
    
    request_id: Optional[str]
    intent: IntentType
    natural_language_response: str
    best_move: Optional[str] = None
    evaluation: Optional[float] = None
    move_analyses: list[MoveAnalysis] = field(default_factory=list)
    principal_variation: list[str] = field(default_factory=list)
    confidence: float = 1.0
    metadata: dict[str, Any] = field(default_factory=dict)
    
    def to_dict(self) -> dict[str, Any]:
        """Convert response to dictionary representation."""
        return {
            "request_id": self.request_id,
            "intent": self.intent.value,
            "response": self.natural_language_response,
            "best_move": self.best_move,
            "evaluation": self.evaluation,
            "move_analyses": [
                {
                    "move": ma.move,
                    "evaluation": ma.evaluation,
                    "depth": ma.depth,
                    "is_best": ma.is_best,
                    "explanation": ma.explanation,
                    "alternatives": ma.alternatives,
                }
                for ma in self.move_analyses
            ],
            "principal_variation": self.principal_variation,
            "confidence": self.confidence,
            "metadata": self.metadata,
        }


@dataclass
class IntentRecognition:
    """Result of intent recognition from user input."""
    
    intent: IntentType
    confidence: float
    extracted_entities: dict[str, Any] = field(default_factory=dict)
    reasoning: str = ""
