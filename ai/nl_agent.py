"""Natural Language Agent interface for XLMate (issue #627).

Translates plain-English chess requests into engine analysis calls and
returns human-readable explanations at the requested complexity level.
"""

from __future__ import annotations

import re
import uuid
from dataclasses import dataclass, field
from enum import Enum
from typing import Any, Protocol


# ---------------------------------------------------------------------------
# Domain models
# ---------------------------------------------------------------------------

class IntentType(str, Enum):
    ANALYZE_POSITION = "analyze_position"
    SUGGEST_MOVE = "suggest_move"
    EXPLAIN_MOVE = "explain_move"
    GET_HINT = "get_hint"
    COMPARE_MOVES = "compare_moves"
    LEARN_CONCEPT = "learn_concept"
    UNKNOWN = "unknown"


class ComplexityLevel(str, Enum):
    BEGINNER = "beginner"
    INTERMEDIATE = "intermediate"
    ADVANCED = "advanced"


@dataclass
class NLRequest:
    user_input: str
    fen: str | None = None
    complexity: ComplexityLevel = ComplexityLevel.INTERMEDIATE
    request_id: str = field(default_factory=lambda: str(uuid.uuid4()))


@dataclass
class NLResponse:
    request_id: str
    intent: IntentType
    text: str
    best_move: str | None = None
    evaluation: float | None = None
    principal_variation: list[str] = field(default_factory=list)
    confidence: float = 1.0
    metadata: dict[str, Any] = field(default_factory=dict)

    def to_dict(self) -> dict[str, Any]:
        return {
            "request_id": self.request_id,
            "intent": self.intent.value,
            "response": self.text,
            "best_move": self.best_move,
            "evaluation": self.evaluation,
            "principal_variation": self.principal_variation,
            "confidence": self.confidence,
            "metadata": self.metadata,
        }


# ---------------------------------------------------------------------------
# Engine protocol — decouples the agent from a concrete worker implementation
# ---------------------------------------------------------------------------

class EngineProtocol(Protocol):
    async def analyze(self, fen: str, depth: int) -> Any: ...


# ---------------------------------------------------------------------------
# Intent recognition helpers
# ---------------------------------------------------------------------------

_INTENT_PATTERNS: list[tuple[IntentType, list[str]]] = [
    (IntentType.SUGGEST_MOVE,     ["best move", "what should i play", "suggest", "recommend"]),
    (IntentType.EXPLAIN_MOVE,     ["why", "explain", "reason", "purpose"]),
    (IntentType.GET_HINT,         ["hint", "clue", "tip"]),
    (IntentType.COMPARE_MOVES,    ["compare", "better", "vs", "versus", "difference"]),
    (IntentType.LEARN_CONCEPT,    ["what is", "teach", "explain concept", "how does"]),
    (IntentType.ANALYZE_POSITION, ["analyze", "evaluation", "assess", "position"]),
]

_COMPLEXITY_PATTERNS: dict[ComplexityLevel, list[str]] = {
    ComplexityLevel.BEGINNER:     ["beginner", "simple", "easy", "basic", "newbie"],
    ComplexityLevel.ADVANCED:     ["advanced", "expert", "deep", "technical", "engine"],
}

_MOVE_RE = re.compile(r"\b([a-h][1-8][a-h][1-8][qrbn]?|[NBRQK][a-h]?[1-8]?x?[a-h][1-8])\b")
_FEN_RE  = re.compile(
    r"[rnbqkpRNBQKP1-8]{1,8}(?:/[rnbqkpRNBQKP1-8]{1,8}){7}\s[wb]\s[KQkq-]+\s[a-h3-6-]+\s\d+\s\d+"
)


def _recognize_intent(text: str) -> tuple[IntentType, float]:
    lower = text.lower()
    for intent, keywords in _INTENT_PATTERNS:
        if any(kw in lower for kw in keywords):
            return intent, 0.85
    return IntentType.UNKNOWN, 0.3


def _detect_complexity(text: str) -> ComplexityLevel:
    lower = text.lower()
    for level, keywords in _COMPLEXITY_PATTERNS.items():
        if any(kw in lower for kw in keywords):
            return level
    return ComplexityLevel.INTERMEDIATE


def _extract_fen(text: str) -> str | None:
    m = _FEN_RE.search(text)
    return m.group(0) if m else None


def _extract_moves(text: str) -> list[str]:
    return _MOVE_RE.findall(text)


# ---------------------------------------------------------------------------
# Agent
# ---------------------------------------------------------------------------

class NaturalLanguageAgent:
    """Bridges natural language chess requests with engine analysis.

    Pass any object that satisfies ``EngineProtocol`` (e.g. a ``GPUAnalysisWorker``
    or the ``StockfishWASMEngine``) as the engine.
    """

    _CONCEPTS: dict[str, dict[str, str]] = {
        "fork": {
            "beginner":     "A fork attacks two pieces at once — your opponent can only save one!",
            "intermediate": "A fork is a tactic where one piece attacks two enemy pieces simultaneously.",
            "advanced":     "Forks exploit piece geometry; knights are most effective due to their L-shaped movement.",
        },
        "pin": {
            "beginner":     "A pin stops a piece from moving because a more valuable piece is behind it.",
            "intermediate": "A pin restricts a piece that shields a more valuable piece from capture.",
            "advanced":     "Absolute pins (king behind) are legally binding; relative pins create strategic pressure.",
        },
        "skewer": {
            "beginner":     "A skewer forces a valuable piece to move, exposing a weaker piece behind it.",
            "intermediate": "A skewer attacks the more valuable piece first, winning the piece behind it.",
            "advanced":     "Skewers invert the pin dynamic and are most powerful when the rear piece is undefended.",
        },
    }

    def __init__(self, engine: EngineProtocol) -> None:
        self._engine = engine

    async def process(
        self,
        user_input: str,
        fen: str | None = None,
        complexity: ComplexityLevel | None = None,
    ) -> NLResponse:
        """Process a natural language request and return a response."""
        req_id = str(uuid.uuid4())
        intent, confidence = _recognize_intent(user_input)
        resolved_complexity = complexity or _detect_complexity(user_input)
        resolved_fen = fen or _extract_fen(user_input)

        try:
            return await self._dispatch(
                req_id, intent, confidence, user_input,
                resolved_fen, resolved_complexity,
            )
        except Exception as exc:
            return NLResponse(
                request_id=req_id,
                intent=intent,
                text="Sorry, I encountered an error. Please try again.",
                confidence=0.0,
                metadata={"error": str(exc)},
            )

    async def _dispatch(
        self,
        req_id: str,
        intent: IntentType,
        confidence: float,
        user_input: str,
        fen: str | None,
        complexity: ComplexityLevel,
    ) -> NLResponse:
        if intent == IntentType.LEARN_CONCEPT:
            return self._explain_concept(req_id, user_input, complexity)

        if not fen:
            return NLResponse(
                request_id=req_id,
                intent=intent,
                text="Please provide a board position (FEN) so I can help.",
                confidence=0.5,
            )

        result = await self._engine.analyze(fen, depth=18)
        best_move: str = getattr(result, "best_move", "")
        evaluation: float | None = getattr(result, "evaluation", None)
        pv: list[str] = getattr(result, "principal_variation", [])

        text = self._format_response(intent, best_move, evaluation, pv, complexity, user_input)
        return NLResponse(
            request_id=req_id,
            intent=intent,
            text=text,
            best_move=best_move if intent != IntentType.GET_HINT else None,
            evaluation=evaluation,
            principal_variation=pv,
            confidence=confidence,
        )

    def _format_response(
        self,
        intent: IntentType,
        best_move: str,
        evaluation: float | None,
        pv: list[str],
        complexity: ComplexityLevel,
        user_input: str,
    ) -> str:
        eval_str = self._fmt_eval(evaluation, complexity)
        pv_str = " → ".join(pv[:4]) if pv else "N/A"

        if intent == IntentType.GET_HINT:
            return (
                f"Think about {'central control' if evaluation and evaluation > 0 else 'defensive resources'}. "
                f"The best move starts with '{best_move[0] if best_move else '?'}'."
            )

        if complexity == ComplexityLevel.BEGINNER:
            return f"Play {best_move}. The position is {eval_str}."

        if complexity == ComplexityLevel.ADVANCED:
            return (
                f"Best: {best_move} | Eval: {eval_str} | "
                f"PV: {pv_str} | Depth: {getattr(self, '_last_depth', 18)}"
            )

        # INTERMEDIATE (default)
        return (
            f"The best move is {best_move}. "
            f"Position evaluates to {eval_str}. "
            f"Main line: {pv_str}."
        )

    def _explain_concept(
        self, req_id: str, user_input: str, complexity: ComplexityLevel
    ) -> NLResponse:
        lower = user_input.lower()
        for concept, explanations in self._CONCEPTS.items():
            if concept in lower:
                text = explanations.get(complexity.value, explanations["intermediate"])
                return NLResponse(
                    request_id=req_id,
                    intent=IntentType.LEARN_CONCEPT,
                    text=text,
                    confidence=0.9,
                    metadata={"concept": concept},
                )
        return NLResponse(
            request_id=req_id,
            intent=IntentType.LEARN_CONCEPT,
            text="Ask me about specific tactics like forks, pins, or skewers for a detailed explanation.",
            confidence=0.5,
        )

    @staticmethod
    def _fmt_eval(evaluation: float | None, complexity: ComplexityLevel) -> str:
        if evaluation is None:
            return "unknown"
        if complexity == ComplexityLevel.BEGINNER:
            if evaluation > 1.0:   return "clearly better for white"
            if evaluation > 0.3:   return "slightly better for white"
            if evaluation > -0.3:  return "roughly equal"
            if evaluation > -1.0:  return "slightly better for black"
            return "clearly better for black"
        return f"{evaluation:+.2f}"
