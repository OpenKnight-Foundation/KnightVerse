"""Natural Language Agent service for XLMate chess platform.

This module provides the main service layer that integrates natural language processing
with chess engine analysis to provide intelligent, conversational responses.
"""

from __future__ import annotations

import logging
import uuid
from typing import Any, Optional

from gpu_worker.config import WorkerConfig
from gpu_worker.models import AnalysisRequest, AnalysisResult
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

logger = logging.getLogger(__name__)


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
            response = (
                f"This position is {'good for white' if result.evaluation > 0.3 else 'good for black' if result.evaluation < -0.3 else 'roughly equal'}.\n\n"
                f"The best move is {result.best_move}.\n\n"
                f"I've analyzed this to depth {result.depth}, which gives us a pretty reliable assessment."
            )
        elif complexity == ComplexityLevel.ADVANCED:
            pv_text = " -> ".join(result.principal_variation[:5]) if result.principal_variation else "N/A"
            response = (
                f"Evaluation: {eval_text} ({result.evaluation} centipawns)\n"
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
            response = (
                f"I recommend playing {result.best_move}.\n\n"
                f"This move gives you {'an advantage' if result.evaluation > 0.3 else 'good chances' if result.evaluation > 0 else 'the best practical chances'} "
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
            response = (
                f"The best move in this position is {best_move}.\n\n"
                f"This move {'improves your position' if result.evaluation > 0 else 'is the best defense'} "
                f"and leads to {'good attacking chances' if result.evaluation > 1.0 else 'a solid position'}."
            )
        else:
            pv_text = " -> ".join(result.principal_variation[:4]) if result.principal_variation else "N/A"
            response = (
                f"{best_move} is the strongest move here.\n\n"
                f"It leads to a position evaluated at {result.evaluation:.2f}.\n\n"
                f"Main line: {pv_text}\n\n"
                f"This move {'creates threats' if result.evaluation > 0.5 else 'maintains equality' if abs(result.evaluation) <= 0.5 else 'defends against threats'} "
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
            hint = (
                f"Look for moves that {'attack' if result.evaluation > 0 else 'defend'} key pieces.\n\n"
                f"Consider {'central squares' if 'e' in best_move or 'd' in best_move else 'the flanks'} "
                f"and think about piece activity."
            )
        else:
            # Give first piece of the best move as hint
            first_char = best_move[0] if best_move else '?'
            hint = (
                f"Consider {'advancing' if result.evaluation > 0 else 'consolidating'} your position.\n\n"
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
            if best_move in moves_to_compare:
                response = (
                    f"{best_move} is superior among the moves you mentioned.\n\n"
                    f"Current evaluation: {result.evaluation:.2f}\n"
                    f"This move leads to the most favorable position."
                )
            else:
                response = (
                    f"The best move is actually {best_move}, not the ones you mentioned.\n\n"
                    f"Evaluation with {best_move}: {result.evaluation:.2f}\n\n"
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
