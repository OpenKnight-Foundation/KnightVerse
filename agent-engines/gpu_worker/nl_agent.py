"""Natural Language Agent service for KnightVerse chess platform.

This module provides the main service layer that integrates natural language processing
with chess engine analysis to provide intelligent, conversational responses.
"""

from __future__ import annotations

import logging
import time
import uuid
from dataclasses import dataclass, field
from typing import Any, List, Optional

import chess

from gpu_worker.config import WorkerConfig
from gpu_worker.models import AnalysisRequest, AnalysisResult, PersonalityTraits
from gpu_worker.nl_intent_parser import (
    detect_complexity,
    extract_fen_from_input,
    extract_moves_from_input,
    recognize_intent,
)
from gpu_worker.nl_models import (
    ComplexityLevel,
    IntentType,
    MoveAnalysis,
    NLAnalysisRequest,
    NLAnalysisResponse,
)
from gpu_worker.pool import WorkerPool
from gpu_worker.tactics import (
    TacticalAnalysis,
    TacticalMotif,
    TacticalPatternExtractor,
    color_name,
    parse_move,
)

logger = logging.getLogger(__name__)


# ---------------------------------------------------------------------------
# Conversational blunder coaching
# ---------------------------------------------------------------------------

#: Hard cap on a coaching line so it can be read at a glance during play.
MAX_EXPLANATION_LENGTH = 250

#: Evaluation drop (in pawns) at which a move is treated as a blunder.
BLUNDER_THRESHOLD = 1.5

#: Short persona flavour prefixes keyed by the companion's configured tone.
_TONE_PREFIXES: dict[str, str] = {
    "neutral": "",
    "aggressive": "Ouch! ",
    "humorous": "Yikes! ",
    "formal": "Note: ",
}

#: Message used when coaching is withheld during rated play.
COACHING_DISABLED_MESSAGE = (
    "Coaching is off during rated play. Enable companion mode and I'll break "
    "this position down with you."
)

#: Message used when the move or position could not be verified on the board.
UNVERIFIABLE_MESSAGE = (
    "I couldn't verify that move in this position, so I'd rather not guess at "
    "what went wrong."
)


@dataclass
class BlunderExplanation:
    """A coaching line about a blunder plus the evidence behind it."""

    text: str
    motifs: list[str] = field(default_factory=list)
    blunder_move: str = ""
    best_move: str = ""
    refutation: str = ""
    is_mate: bool = False
    material_swing: int = 0
    latency_ms: float = 0.0

    def to_dict(self) -> dict[str, Any]:
        """Convert the explanation to a dictionary representation."""
        return {
            "text": self.text,
            "motifs": self.motifs,
            "blunder_move": self.blunder_move,
            "best_move": self.best_move,
            "refutation": self.refutation,
            "is_mate": self.is_mate,
            "material_swing": self.material_swing,
            "latency_ms": self.latency_ms,
        }


class BlunderCoach:
    """Turns engine output about a blunder into a short coaching sentence.

    The coach is a deterministic template engine on top of
    :class:`~gpu_worker.tactics.TacticalPatternExtractor`. It never mentions a
    move it has not verified as legal, and never claims a threat that is not on
    the board, which keeps it usable as a live in-game companion.
    """

    def __init__(self, extractor: Optional[TacticalPatternExtractor] = None) -> None:
        """Initialize the coach.

        Args:
            extractor: Optional tactical pattern extractor to reuse.
        """
        self._extractor = extractor or TacticalPatternExtractor()

    def explain(
        self,
        fen: str,
        blunder_move: str,
        best_move: str,
        engine_pv: List[str],
        traits: Optional[PersonalityTraits] = None,
        companion_mode: bool = True,
        rated_game: bool = False,
        max_length: int = MAX_EXPLANATION_LENGTH,
    ) -> BlunderExplanation:
        """Explain a blunder and return the coaching line with its evidence.

        Args:
            fen: Position before the blunder was played.
            blunder_move: The move actually played, in UCI or SAN.
            best_move: The move the engine preferred, in UCI or SAN.
            engine_pv: The engine's refutation line after the blunder.
            traits: Optional companion personality driving the tone.
            companion_mode: Whether the coaching companion is enabled.
            rated_game: Whether this is an active rated game.
            max_length: Maximum length of the response.

        Returns:
            A :class:`BlunderExplanation` whose ``text`` is at most
            ``max_length`` characters.
        """
        start = time.perf_counter()

        if rated_game and not companion_mode:
            # Never hand out moves or threats in a rated game the player has not
            # opted into coaching for.
            return BlunderExplanation(
                text=_truncate(COACHING_DISABLED_MESSAGE, max_length),
                latency_ms=(time.perf_counter() - start) * 1000,
            )

        analysis = self._extractor.extract(fen, blunder_move, best_move, engine_pv)
        if not analysis.valid:
            return BlunderExplanation(
                text=_truncate(UNVERIFIABLE_MESSAGE, max_length),
                latency_ms=(time.perf_counter() - start) * 1000,
            )

        text = self._compose(analysis, traits, companion_mode, max_length)
        return BlunderExplanation(
            text=text,
            motifs=analysis.motif_names,
            blunder_move=analysis.blunder_label,
            best_move=analysis.best_label,
            refutation=analysis.refutation_label,
            is_mate=analysis.is_mate,
            material_swing=analysis.material_swing,
            latency_ms=(time.perf_counter() - start) * 1000,
        )

    # ------------------------------------------------------------------
    # Template engine
    # ------------------------------------------------------------------

    def _compose(
        self,
        analysis: TacticalAnalysis,
        traits: Optional[PersonalityTraits],
        companion_mode: bool,
        max_length: int,
    ) -> str:
        """Assemble prefix + cause + consequence + advice within the length budget."""
        prefix = self._tone_prefix(traits)
        sentence = self._cause_clause(analysis)

        consequence = self._threat_clause(analysis)
        if consequence:
            sentence = f"{sentence}, {consequence}"
        sentence = f"{sentence}."

        body = f"{prefix}{sentence}"

        # The suggested improvement is optional: it is the first thing dropped
        # when the budget is tight, and it is withheld outside companion mode.
        if companion_mode and analysis.best_label:
            with_advice = f"{body} Better was {analysis.best_label}."
            if len(with_advice) <= max_length:
                return with_advice

        if len(body) <= max_length:
            return body
        return _truncate(sentence, max_length)

    def _tone_prefix(self, traits: Optional[PersonalityTraits]) -> str:
        """Map the companion's personality tone to a short flavour prefix."""
        if traits is None:
            return ""
        return _TONE_PREFIXES.get(traits.tone, _TONE_PREFIXES["neutral"])

    def _cause_clause(self, analysis: TacticalAnalysis) -> str:
        """Describe what the played move gave away."""
        move = analysis.blunder_label
        cause = analysis.cause

        if cause is None:
            if analysis.threat is None:
                return f"{move} is the losing move here"
            return f"{move} misses the tactic"

        if cause.motif == TacticalMotif.BACK_RANK_MATE:
            return f"{move} abandons your back rank"

        square = cause.victim_squares[0] if cause.victim_squares else "the board"
        piece = cause.victims[0] if cause.victims else "piece"
        if cause.self_inflicted:
            return f"{move} puts your {piece} on {square} en prise"
        if cause.defended:
            return f"{move} leaves your {piece} on {square} under-defended"
        return f"{move} leaves your {piece} on {square} undefended"

    def _threat_clause(self, analysis: TacticalAnalysis) -> str:
        """Describe how the opponent punishes the move."""
        threat = analysis.threat
        if threat is None:
            return ""

        villain = color_name(analysis.villain)
        with_move = (
            f" with {analysis.refutation_label}" if analysis.refutation_label else ""
        )

        if threat.motif in (TacticalMotif.BACK_RANK_MATE, TacticalMotif.MATE_THREAT):
            mate = (
                "back-rank mate"
                if threat.motif == TacticalMotif.BACK_RANK_MATE
                else "mate"
            )
            if threat.immediate:
                return f"allowing {mate}{with_move}"
            if threat.forced:
                return f"allowing {villain} to force {mate}{with_move}"
            return f"allowing {villain} to threaten {mate}{with_move}"

        if threat.motif == TacticalMotif.FORK:
            return f"allowing {villain} to fork {threat.victim_phrase()}{with_move}"

        if threat.motif == TacticalMotif.PIN:
            front, back = threat.victims[0], threat.victims[1]
            return (
                f"allowing {villain} to pin your {front} on "
                f"{threat.victim_squares[0]} to your {back}{with_move}"
            )

        if threat.motif == TacticalMotif.SKEWER:
            front, back = threat.victims[0], threat.victims[1]
            return (
                f"allowing {villain} to skewer your {front} and win the "
                f"{back} on {threat.victim_squares[1]}{with_move}"
            )

        return f"allowing {villain} to win {threat.victim_phrase()}{with_move}"


def _truncate(text: str, max_length: int) -> str:
    """Trim ``text`` to ``max_length`` characters on a word boundary."""
    if len(text) <= max_length:
        return text
    clipped = text[: max_length - 1].rstrip()
    if " " in clipped:
        clipped = clipped[: clipped.rindex(" ")].rstrip(" ,;:")
    return f"{clipped}…"


#: Shared coach instance; the extractor is stateless so it is safe to reuse.
_DEFAULT_COACH = BlunderCoach()


def explain_blunder(
    fen: str,
    blunder_move: str,
    best_move: str,
    engine_pv: List[str],
    traits: Optional[PersonalityTraits] = None,
    companion_mode: bool = True,
    rated_game: bool = False,
) -> str:
    """Explain a blunder in plain chess English instead of raw engine output.

    Example::

        >>> fen = "2rb2k1/5ppp/8/3N4/8/8/P5P1/R5K1 b - - 0 34"
        >>> explain_blunder(fen, "d8a5", "d8e7", ["d5e7"])
        '34...Ba5 misses the tactic, allowing White to fork your King and Rook
         with 35.Ne7+. Better was 34...Be7.'

    Args:
        fen: Position before the blunder was played.
        blunder_move: The move actually played, in UCI or SAN.
        best_move: The move the engine preferred, in UCI or SAN.
        engine_pv: The engine's principal variation refuting the blunder.
        traits: Optional companion personality; its ``tone`` drives the phrasing.
        companion_mode: Whether the coaching companion is enabled. When it is
            off during a rated game no move is suggested.
        rated_game: Whether this is an active rated game.

    Returns:
        A coaching line of at most :data:`MAX_EXPLANATION_LENGTH` characters.
        Every move and threat it names is verified against the board, so an
        unverifiable input yields a safe fallback rather than a guess.
    """
    return _DEFAULT_COACH.explain(
        fen=fen,
        blunder_move=blunder_move,
        best_move=best_move,
        engine_pv=engine_pv,
        traits=traits,
        companion_mode=companion_mode,
        rated_game=rated_game,
    ).text


class NaturalLanguageAgent:
    """Natural language agent that bridges user input with chess engine analysis.
    
    This agent interprets natural language requests, determines the appropriate
    analysis to perform, and generates human-readable explanations.
    """
    
    def __init__(self, worker_pool: WorkerPool) -> None:
        """Initialize the natural language agent.
        
        Args:
            worker_pool: The worker pool for chess engine analysis.
        """
        self._pool = worker_pool
        self._request_history: dict[str, NLAnalysisRequest] = {}
        self._coach = BlunderCoach()
    
    async def process_request(
        self,
        user_input: str,
        fen: Optional[str] = None,
        move_history: Optional[list[str]] = None,
        complexity: Optional[ComplexityLevel] = None,
        context: Optional[dict[str, Any]] = None,
    ) -> NLAnalysisResponse:
        """Process a natural language request and return analysis.
        
        Args:
            user_input: The user's natural language input.
            fen: Optional FEN string for the position to analyze.
            move_history: Optional list of moves in the game.
            complexity: Optional desired complexity level.
            context: Optional additional context.
            
        Returns:
            NLAnalysisResponse with natural language explanation.
        """
        # Create request ID
        request_id = str(uuid.uuid4())
        
        # Recognize intent
        intent_recognition = recognize_intent(user_input)
        
        # Detect complexity if not specified
        if complexity is None:
            complexity = detect_complexity(user_input)
        
        # Extract FEN from input if not provided
        if fen is None:
            fen = extract_fen_from_input(user_input)
        
        # Extract moves from input
        if move_history is None:
            move_history = extract_moves_from_input(user_input)
        
        # Create structured request
        request = NLAnalysisRequest(
            user_input=user_input,
            fen=fen,
            move_history=move_history,
            intent=intent_recognition.intent,
            complexity=complexity,
            context=context or {},
            request_id=request_id,
        )
        
        # Store request in history
        self._request_history[request_id] = request
        
        logger.info(
            f"Processing NL request {request_id}: intent={intent_recognition.intent.value}, "
            f"confidence={intent_recognition.confidence}"
        )
        
        # Route to appropriate handler based on intent
        try:
            if intent_recognition.intent == IntentType.ANALYZE_POSITION:
                return await self._handle_analyze_position(request)
            elif intent_recognition.intent == IntentType.SUGGEST_MOVE:
                return await self._handle_suggest_move(request)
            elif intent_recognition.intent == IntentType.EXPLAIN_MOVE:
                return await self._handle_explain_move(request)
            elif intent_recognition.intent == IntentType.GET_HINT:
                return await self._handle_get_hint(request)
            elif intent_recognition.intent == IntentType.COMPARE_MOVES:
                return await self._handle_compare_moves(request)
            elif intent_recognition.intent == IntentType.LEARN_CONCEPT:
                return await self._handle_learn_concept(request)
            else:
                return await self._handle_unknown(request)
        except Exception as e:
            logger.error(f"Error processing request {request_id}: {e}", exc_info=True)
            return NLAnalysisResponse(
                request_id=request_id,
                intent=intent_recognition.intent,
                natural_language_response="I'm sorry, I encountered an error while analyzing your request. Please try again.",
                confidence=0.0,
                metadata={"error": str(e)},
            )
    
    async def _handle_analyze_position(self, request: NLAnalysisRequest) -> NLAnalysisResponse:
        """Handle position analysis requests."""
        if not request.fen:
            return NLAnalysisResponse(
                request_id=request.request_id,
                intent=IntentType.ANALYZE_POSITION,
                natural_language_response="I need a board position to analyze. Please provide the current position or make some moves first.",
                confidence=0.5,
            )
        
        # Perform engine analysis
        analysis_result = await self._analyze_position(request.fen, depth=18)
        
        # Generate natural language response
        response = self._generate_position_analysis_nl(
            request, analysis_result, request.complexity
        )
        
        return response
    
    async def _handle_suggest_move(self, request: NLAnalysisRequest) -> NLAnalysisResponse:
        """Handle move suggestion requests."""
        if not request.fen:
            return NLAnalysisResponse(
                request_id=request.request_id,
                intent=IntentType.SUGGEST_MOVE,
                natural_language_response="Please provide the current board position so I can suggest the best move.",
                confidence=0.5,
            )
        
        # Analyze position to find best move
        analysis_result = await self._analyze_position(request.fen, depth=20)
        
        # Generate suggestion
        response = self._generate_move_suggestion_nl(
            request, analysis_result, request.complexity
        )
        
        return response
    
    async def _handle_explain_move(self, request: NLAnalysisRequest) -> NLAnalysisResponse:
        """Handle move explanation requests."""
        if not request.fen:
            return NLAnalysisResponse(
                request_id=request.request_id,
                intent=IntentType.EXPLAIN_MOVE,
                natural_language_response="I need the position and the move you'd like me to explain.",
                confidence=0.5,
            )
        
        # When the caller tells us which move was actually played, run the
        # blunder pipeline so the answer names the tactic instead of the engine line.
        played_move = request.context.get("played_move")
        if played_move:
            return await self.coach_move(
                fen=request.fen,
                played_move=played_move,
                traits=request.context.get("traits"),
                companion_mode=request.context.get("companion_mode", True),
                rated_game=request.context.get("rated_game", False),
                request_id=request.request_id,
            )
        
        # Analyze position
        analysis_result = await self._analyze_position(request.fen, depth=18)
        
        # Generate explanation
        response = self._generate_move_explanation_nl(
            request, analysis_result, request.complexity
        )
        
        return response
    
    async def _handle_get_hint(self, request: NLAnalysisRequest) -> NLAnalysisResponse:
        """Handle hint requests."""
        if not request.fen:
            return NLAnalysisResponse(
                request_id=request.request_id,
                intent=IntentType.GET_HINT,
                natural_language_response="Please provide the current position so I can give you a hint.",
                confidence=0.5,
            )
        
        # Analyze with lower depth for hints
        analysis_result = await self._analyze_position(request.fen, depth=12)
        
        # Generate hint (less direct than full suggestion)
        response = self._generate_hint_nl(
            request, analysis_result, request.complexity
        )
        
        return response
    
    async def _handle_compare_moves(self, request: NLAnalysisRequest) -> NLAnalysisResponse:
        """Handle move comparison requests."""
        if not request.fen:
            return NLAnalysisResponse(
                request_id=request.request_id,
                intent=IntentType.COMPARE_MOVES,
                natural_language_response="Please provide the position and the moves you want to compare.",
                confidence=0.5,
            )
        
        # Extract moves to compare
        moves_to_compare = extract_moves_from_input(request.user_input)
        
        if not moves_to_compare:
            return NLAnalysisResponse(
                request_id=request.request_id,
                intent=IntentType.COMPARE_MOVES,
                natural_language_response="I couldn't identify which moves you want to compare. Please specify the moves (e.g., 'compare e4 and d4').",
                confidence=0.5,
            )
        
        # Analyze position
        analysis_result = await self._analyze_position(request.fen, depth=18)
        
        # Generate comparison
        response = self._generate_move_comparison_nl(
            request, analysis_result, moves_to_compare, request.complexity
        )
        
        return response
    
    async def _handle_learn_concept(self, request: NLAnalysisRequest) -> NLAnalysisResponse:
        """Handle concept learning requests."""
        # Extract concept from input
        concept = self._extract_concept(request.user_input)
        
        response_text = self._generate_concept_explanation(concept, request.complexity)
        
        return NLAnalysisResponse(
            request_id=request.request_id,
            intent=IntentType.LEARN_CONCEPT,
            natural_language_response=response_text,
            confidence=0.8,
            metadata={"concept": concept},
        )
    
    async def _handle_unknown(self, request: NLAnalysisRequest) -> NLAnalysisResponse:
        """Handle requests with unrecognized intent."""
        return NLAnalysisResponse(
            request_id=request.request_id,
            intent=IntentType.UNKNOWN,
            natural_language_response=(
                "I'm not sure what you'd like me to help with. You can ask me to:\n"
                "- Analyze a position\n"
                "- Suggest the best move\n"
                "- Explain why a move is good or bad\n"
                "- Give you a hint\n"
                "- Compare different moves\n"
                "- Teach you chess concepts"
            ),
            confidence=0.3,
        )
    
    async def coach_move(
        self,
        fen: str,
        played_move: str,
        traits: Optional[PersonalityTraits] = None,
        companion_mode: bool = True,
        rated_game: bool = False,
        depth: int = 16,
        blunder_threshold: float = BLUNDER_THRESHOLD,
        request_id: Optional[str] = None,
    ) -> NLAnalysisResponse:
        """Detect whether a played move was a blunder and coach the player on it.

        This is the bridge between the engine-side blunder detection and the
        natural language generator: the position is analysed before and after
        the move, and any evaluation collapse is explained in chess terms by
        :class:`BlunderCoach`.

        Args:
            fen: Position before the move was played.
            played_move: The move actually played, in UCI or SAN.
            traits: Optional companion personality driving the tone.
            companion_mode: Whether the coaching companion is enabled.
            rated_game: Whether this is an active rated game. Combined with
                ``companion_mode=False`` no move is suggested at all.
            depth: Search depth for both analyses.
            blunder_threshold: Evaluation drop (in pawns) that counts as a blunder.
            request_id: Optional request id to echo back.

        Returns:
            An :class:`NLAnalysisResponse` carrying the coaching line and the
            motifs behind it in ``metadata``.
        """
        request_id = request_id or str(uuid.uuid4())
        
        if rated_game and not companion_mode:
            return NLAnalysisResponse(
                request_id=request_id,
                intent=IntentType.EXPLAIN_MOVE,
                natural_language_response=COACHING_DISABLED_MESSAGE,
                confidence=1.0,
                metadata={"companion_mode": False, "rated_game": True},
            )
        
        try:
            board = chess.Board(fen)
        except ValueError:
            board = None
        
        move = parse_move(board, played_move) if board is not None else None
        if move is None:
            return NLAnalysisResponse(
                request_id=request_id,
                intent=IntentType.EXPLAIN_MOVE,
                natural_language_response=UNVERIFIABLE_MESSAGE,
                confidence=0.0,
                metadata={"error": "unverifiable_move", "played_move": played_move},
            )
        
        board.push(move)
        fen_after = board.fen()
        
        before = await self._analyze_position(fen, depth=depth)
        after = await self._analyze_position(fen_after, depth=depth)
        
        # UCI scores are relative to the side to move, so the swing across the
        # move is the sum of both evaluations.
        eval_loss: Optional[float] = None
        if before.evaluation is not None and after.evaluation is not None:
            eval_loss = before.evaluation + after.evaluation
        
        played_is_best = parse_move(chess.Board(fen), before.best_move) == move
        is_blunder = (
            not played_is_best
            and eval_loss is not None
            and eval_loss >= blunder_threshold
        )
        
        if not is_blunder:
            return NLAnalysisResponse(
                request_id=request_id,
                intent=IntentType.EXPLAIN_MOVE,
                natural_language_response=(
                    "That move holds up - I don't see a tactic against it here."
                ),
                best_move=before.best_move if companion_mode else None,
                evaluation=after.evaluation,
                principal_variation=after.principal_variation,
                confidence=0.8,
                metadata={
                    "is_blunder": False,
                    "eval_loss": eval_loss,
                    "played_move": played_move,
                },
            )
        
        explanation = self._coach.explain(
            fen=fen,
            blunder_move=played_move,
            best_move=before.best_move,
            engine_pv=after.principal_variation,
            traits=traits,
            companion_mode=companion_mode,
            rated_game=rated_game,
        )
        
        logger.info(
            f"Coached blunder {request_id}: motifs={explanation.motifs}, "
            f"loss={eval_loss}, latency={explanation.latency_ms:.1f}ms"
        )
        
        return NLAnalysisResponse(
            request_id=request_id,
            intent=IntentType.EXPLAIN_MOVE,
            natural_language_response=explanation.text,
            best_move=before.best_move if companion_mode else None,
            evaluation=after.evaluation,
            principal_variation=after.principal_variation,
            confidence=0.9,
            metadata={
                "is_blunder": True,
                "eval_loss": eval_loss,
                "played_move": played_move,
                **explanation.to_dict(),
            },
        )
    
    async def _analyze_position(self, fen: str, depth: int = 18) -> AnalysisResult:
        """Perform engine analysis on a position.
        
        Args:
            fen: FEN string of the position.
            depth: Search depth.
            
        Returns:
            AnalysisResult from the engine.
        """
        analysis_request = AnalysisRequest(
            fen=fen,
            depth=depth,
        )
        
        result = await self._pool.submit(analysis_request)
        return result
    
    def _generate_position_analysis_nl(
        self,
        request: NLAnalysisRequest,
        result: AnalysisResult,
        complexity: ComplexityLevel,
    ) -> NLAnalysisResponse:
        """Generate natural language position analysis."""
        eval_text = self._format_evaluation(result.evaluation, complexity)
        
        if complexity == ComplexityLevel.BEGINNER:
            if result.evaluation is not None:
                pos_desc = 'good for white' if result.evaluation > 0.3 else 'good for black' if result.evaluation < -0.3 else 'roughly equal'
            else:
                pos_desc = 'roughly equal'
            response = (
                f"This position is {pos_desc}.\n\n"
                f"The best move is {result.best_move}.\n\n"
                f"I've analyzed this to depth {result.depth}, which gives us a pretty reliable assessment."
            )
        elif complexity == ComplexityLevel.ADVANCED:
            pv_text = " -> ".join(result.principal_variation[:5]) if result.principal_variation else "N/A"
            eval_cp = f"{result.evaluation} centipawns" if result.evaluation is not None else "N/A"
            response = (
                f"Evaluation: {eval_text} ({eval_cp})\n"
                f"Depth: {result.depth}\n"
                f"Best move: {result.best_move}\n"
                f"Principal variation: {pv_text}\n"
                f"Nodes searched: {result.nodes_searched:,}"
            )
        else:  # INTERMEDIATE
            pv_text = " -> ".join(result.principal_variation[:3]) if result.principal_variation else "N/A"
            response = (
                f"This position evaluates to {eval_text}.\n\n"
                f"The best move is {result.best_move}, followed by the sequence: {pv_text}\n\n"
                f"Analysis depth: {result.depth}"
            )
        
        return NLAnalysisResponse(
            request_id=request.request_id,
            intent=IntentType.ANALYZE_POSITION,
            natural_language_response=response,
            best_move=result.best_move,
            evaluation=result.evaluation,
            principal_variation=result.principal_variation,
            confidence=0.9,
        )
    
    def _generate_move_suggestion_nl(
        self,
        request: NLAnalysisRequest,
        result: AnalysisResult,
        complexity: ComplexityLevel,
    ) -> NLAnalysisResponse:
        """Generate natural language move suggestion."""
        eval_text = self._format_evaluation(result.evaluation, complexity)
        
        if complexity == ComplexityLevel.BEGINNER:
            if result.evaluation is not None:
                adv_desc = 'an advantage' if result.evaluation > 0.3 else 'good chances' if result.evaluation > 0 else 'the best practical chances'
            else:
                adv_desc = 'the best practical chances'
            response = (
                f"I recommend playing {result.best_move}.\n\n"
                f"This move gives you {adv_desc} "
                f"in this position."
            )
        elif complexity == ComplexityLevel.ADVANCED:
            pv_text = " -> ".join(result.principal_variation[:5]) if result.principal_variation else "N/A"
            response = (
                f"Best move: {result.best_move}\n"
                f"Evaluation after {result.best_move}: {eval_text}\n"
                f"Main line: {pv_text}\n"
                f"Depth: {result.depth}, Nodes: {result.nodes_searched:,}"
            )
        else:  # INTERMEDIATE
            pv_text = " -> ".join(result.principal_variation[:3]) if result.principal_variation else "N/A"
            response = (
                f"The best move here is {result.best_move}.\n\n"
                f"After this move, the position evaluates to {eval_text}.\n\n"
                f"Expected continuation: {pv_text}"
            )
        
        return NLAnalysisResponse(
            request_id=request.request_id,
            intent=IntentType.SUGGEST_MOVE,
            natural_language_response=response,
            best_move=result.best_move,
            evaluation=result.evaluation,
            principal_variation=result.principal_variation,
            confidence=0.95,
        )
    
    def _generate_move_explanation_nl(
        self,
        request: NLAnalysisRequest,
        result: AnalysisResult,
        complexity: ComplexityLevel,
    ) -> NLAnalysisResponse:
        """Generate natural language move explanation."""
        # Check if the move in context is the best move
        best_move = result.best_move
        
        if complexity == ComplexityLevel.BEGINNER:
            if result.evaluation is not None:
                pos_desc = 'improves your position' if result.evaluation > 0 else 'is the best defense'
                chances_desc = 'good attacking chances' if result.evaluation > 1.0 else 'a solid position'
            else:
                pos_desc = 'is the best defense'
                chances_desc = 'a solid position'
            response = (
                f"The best move in this position is {best_move}.\n\n"
                f"This move {pos_desc} "
                f"and leads to {chances_desc}."
            )
        else:
            pv_text = " -> ".join(result.principal_variation[:4]) if result.principal_variation else "N/A"
            if result.evaluation is not None:
                eval_formatted = f"{result.evaluation:.2f}"
                threat_desc = 'creates threats' if result.evaluation > 0.5 else 'maintains equality' if abs(result.evaluation) <= 0.5 else 'defends against threats'
            else:
                eval_formatted = "N/A"
                threat_desc = 'maintains equality'
            response = (
                f"{best_move} is the strongest move here.\n\n"
                f"It leads to a position evaluated at {eval_formatted}.\n\n"
                f"Main line: {pv_text}\n\n"
                f"This move {threat_desc} "
                f"and follows sound chess principles."
            )
        
        return NLAnalysisResponse(
            request_id=request.request_id,
            intent=IntentType.EXPLAIN_MOVE,
            natural_language_response=response,
            best_move=best_move,
            evaluation=result.evaluation,
            principal_variation=result.principal_variation,
            confidence=0.85,
        )
    
    def _generate_hint_nl(
        self,
        request: NLAnalysisRequest,
        result: AnalysisResult,
        complexity: ComplexityLevel,
    ) -> NLAnalysisResponse:
        """Generate a helpful hint (not the full answer)."""
        best_move = result.best_move
        
        # Provide tactical/strategic hint without giving away the move
        if complexity == ComplexityLevel.BEGINNER:
            if result.evaluation is not None:
                attack_desc = 'attack' if result.evaluation > 0 else 'defend'
            else:
                attack_desc = 'defend'
            hint = (
                f"Look for moves that {attack_desc} key pieces.\n\n"
                f"Consider {'central squares' if 'e' in best_move or 'd' in best_move else 'the flanks'} "
                f"and think about piece activity."
            )
        else:
            # Give first piece of the best move as hint
            first_char = best_move[0] if best_move else '?'
            if result.evaluation is not None:
                advance_desc = 'advancing' if result.evaluation > 0 else 'consolidating'
            else:
                advance_desc = 'consolidating'
            hint = (
                f"Consider {advance_desc} your position.\n\n"
                f"The best move starts with '{first_char}' - think about what piece or pawn that could be."
            )
        
        return NLAnalysisResponse(
            request_id=request.request_id,
            intent=IntentType.GET_HINT,
            natural_language_response=hint,
            best_move=None,  # Don't give away the answer
            evaluation=result.evaluation,
            confidence=0.7,
        )
    
    def _generate_move_comparison_nl(
        self,
        request: NLAnalysisRequest,
        result: AnalysisResult,
        moves_to_compare: list[str],
        complexity: ComplexityLevel,
    ) -> NLAnalysisResponse:
        """Generate comparison between moves."""
        best_move = result.best_move
        
        if complexity == ComplexityLevel.BEGINNER:
            if best_move in moves_to_compare:
                response = (
                    f"Between the moves you mentioned, {best_move} is better.\n\n"
                    f"This move gives you a stronger position."
                )
            else:
                response = (
                    f"Neither of those moves is the best option.\n\n"
                    f"I recommend {best_move} instead, which is stronger than both moves you mentioned."
                )
        else:
            if result.evaluation is not None:
                eval_formatted = f"{result.evaluation:.2f}"
            else:
                eval_formatted = "N/A"
            if best_move in moves_to_compare:
                response = (
                    f"{best_move} is superior among the moves you mentioned.\n\n"
                    f"Current evaluation: {eval_formatted}\n"
                    f"This move leads to the most favorable position."
                )
            else:
                response = (
                    f"The best move is actually {best_move}, not the ones you mentioned.\n\n"
                    f"Evaluation with {best_move}: {eval_formatted}\n\n"
                    f"Consider analyzing why {best_move} is stronger than your candidate moves."
                )
        
        return NLAnalysisResponse(
            request_id=request.request_id,
            intent=IntentType.COMPARE_MOVES,
            natural_language_response=response,
            best_move=best_move,
            evaluation=result.evaluation,
            confidence=0.8,
        )
    
    def _generate_concept_explanation(
        self,
        concept: str,
        complexity: ComplexityLevel,
    ) -> str:
        """Generate explanation of chess concepts."""
        concepts_db = {
            "fork": {
                "beginner": "A fork is when one piece attacks two or more pieces at the same time. It's a powerful tactic because your opponent can only save one piece!",
                "intermediate": "A fork is a tactical motif where a single piece attacks multiple opponent pieces simultaneously. Knights are particularly effective at forking due to their unique movement pattern.",
                "advanced": "Forks represent a fundamental tactical pattern exploiting piece geometry. The forking piece creates multiple threats that cannot be simultaneously parried, often resulting in material gain. Knight forks are most common due to the knight's non-linear movement.",
            },
            "pin": {
                "beginner": "A pin is when a piece can't move because it would expose a more valuable piece behind it to capture.",
                "intermediate": "A pin is a tactical constraint where a piece cannot move without exposing a more valuable piece behind it. There are absolute pins (king behind) and relative pins (other pieces behind).",
                "advanced": "Pins create tactical vulnerabilities by restricting piece mobility. Absolute pins (king as the pinned piece's shield) are legally binding, while relative pins create strategic pressure. Exploiting pins often involves increasing pressure on the pinned piece.",
            },
            "skewer": {
                "beginner": "A skewer is similar to a pin, but it attacks a valuable piece first, forcing it to move and exposing a less valuable piece behind it.",
                "intermediate": "A skewer is a tactical pattern where a long-range piece (queen, rook, or bishop) attacks two pieces in a line, with the more valuable piece in front. The front piece must move, allowing capture of the rear piece.",
                "advanced": "Skewers invert the pin dynamic by attacking the more valuable piece first, creating a forced sequence. They're particularly devastating when the rear piece is undefended or when the skewer leads to checkmate threats.",
            },
        }
        
        concept_lower = concept.lower()
        for key, explanations in concepts_db.items():
            if key in concept_lower:
                return explanations.get(complexity.value, explanations["intermediate"])
        
        return f"I'd be happy to teach you about '{concept}'. This is a general explanation: Understanding chess concepts like this will improve your strategic thinking and tactical awareness. Try asking about specific tactics like forks, pins, or skewers for detailed explanations!"
    
    def _extract_concept(self, user_input: str) -> str:
        """Extract the chess concept from user input."""
        # Simple extraction - look for keywords after common phrases
        patterns = [
            r'(?:what is|explain|teach me about|tell me about)\s+(.+?)(?:\?|$)',
            r'(?:how does)\s+(.+?)(?:\s+work|\?|$)',
        ]
        
        import re
        for pattern in patterns:
            match = re.search(pattern, user_input, re.IGNORECASE)
            if match:
                return match.group(1).strip()
        
        return "general chess concept"
    
    def _format_evaluation(
        self,
        evaluation: float | None,
        complexity: ComplexityLevel,
    ) -> str:
        """Format evaluation in human-readable form."""
        if evaluation is None:
            return "unknown"
        
        if complexity == ComplexityLevel.BEGINNER:
            if evaluation > 1.0:
                return "clearly better for white"
            elif evaluation > 0.3:
                return "slightly better for white"
            elif evaluation > -0.3:
                return "about equal"
            elif evaluation > -1.0:
                return "slightly better for black"
            else:
                return "clearly better for black"
        else:
            return f"{evaluation:+.2f}"
    
    def get_request_history(self, request_id: Optional[str] = None) -> Any:
        """Get request history.
        
        Args:
            request_id: Optional specific request ID to retrieve.
            
        Returns:
            Request history or specific request.
        """
        if request_id:
            return self._request_history.get(request_id)
        return self._request_history
