"""Tests for Natural Language Agent interface."""

import unittest
import asyncio
from unittest.mock import AsyncMock, MagicMock, patch

from gpu_worker.nl_models import (
    ComplexityLevel,
    IntentType,
    MoveAnalysis,
    NLAnalysisRequest,
    NLAnalysisResponse,
    IntentRecognition,
)
from gpu_worker.nl_intent_parser import (
    recognize_intent,
    detect_complexity,
    extract_fen_from_input,
    extract_moves_from_input,
)
from gpu_worker.nl_agent import NaturalLanguageAgent
from gpu_worker.models import AnalysisResult


class TestNLModels(unittest.TestCase):
    """Test natural language models."""
    
    def test_nl_analysis_request_to_dict(self):
        """Test NLAnalysisRequest serialization."""
        request = NLAnalysisRequest(
            user_input="What's the best move here?",
            fen="rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1",
            intent=IntentType.SUGGEST_MOVE,
            complexity=ComplexityLevel.BEGINNER,
            request_id="test-123",
        )
        
        result = request.to_dict()
        self.assertEqual(result["user_input"], "What's the best move here?")
        self.assertEqual(result["intent"], "suggest_move")
        self.assertEqual(result["complexity"], "beginner")
        self.assertEqual(result["request_id"], "test-123")
    
    def test_nl_analysis_response_to_dict(self):
        """Test NLAnalysisResponse serialization."""
        response = NLAnalysisResponse(
            request_id="test-123",
            intent=IntentType.SUGGEST_MOVE,
            natural_language_response="Play e5",
            best_move="e5",
            evaluation=0.5,
            confidence=0.9,
        )
        
        result = response.to_dict()
        self.assertEqual(result["response"], "Play e5")
        self.assertEqual(result["best_move"], "e5")
        self.assertEqual(result["evaluation"], 0.5)
        self.assertEqual(result["confidence"], 0.9)
    
    def test_move_analysis_creation(self):
        """Test MoveAnalysis dataclass."""
        analysis = MoveAnalysis(
            move="e4",
            evaluation=0.3,
            depth=18,
            is_best=True,
            explanation="Controls the center",
        )
        
        self.assertEqual(analysis.move, "e4")
        self.assertTrue(analysis.is_best)
        self.assertEqual(analysis.evaluation, 0.3)


class TestIntentParser(unittest.TestCase):
    """Test intent recognition and parsing."""
    
    def test_recognize_analyze_position(self):
        """Test recognition of analyze position intent."""
        test_cases = [
            "Analyze this position for me",
            "Can you evaluate the board?",
            "What's the assessment of this position?",
            "I need an analysis of this game",
        ]
        
        for input_text in test_cases:
            result = recognize_intent(input_text)
            self.assertEqual(result.intent, IntentType.ANALYZE_POSITION)
            self.assertGreater(result.confidence, 0.0)
    
    def test_recognize_suggest_move(self):
        """Test recognition of suggest move intent."""
        test_cases = [
            "What's the best move?",
            "Suggest a move for me",
            "What should I play here?",
            "Recommend the best move",
        ]
        
        for input_text in test_cases:
            result = recognize_intent(input_text)
            self.assertEqual(result.intent, IntentType.SUGGEST_MOVE)
            self.assertGreater(result.confidence, 0.0)
    
    def test_recognize_explain_move(self):
        """Test recognition of explain move intent."""
        test_cases = [
            "Explain why this move is good",
            "Why is that the best move?",
            "What makes this position strong?",
        ]
        
        for input_text in test_cases:
            result = recognize_intent(input_text)
            self.assertEqual(result.intent, IntentType.EXPLAIN_MOVE)
            self.assertGreater(result.confidence, 0.0)
    
    def test_recognize_get_hint(self):
        """Test recognition of hint intent."""
        test_cases = [
            "Give me a hint",
            "I need some help",
            "I'm stuck, what should I do?",
            "Any tips?",
        ]
        
        for input_text in test_cases:
            result = recognize_intent(input_text)
            self.assertEqual(result.intent, IntentType.GET_HINT)
            self.assertGreater(result.confidence, 0.0)
    
    def test_recognize_compare_moves(self):
        """Test recognition of compare moves intent."""
        test_cases = [
            "Compare e4 and d4",
            "Which is better, Nf3 or c4?",
            "What's the difference between these moves?",
        ]
        
        for input_text in test_cases:
            result = recognize_intent(input_text)
            self.assertEqual(result.intent, IntentType.COMPARE_MOVES)
            self.assertGreater(result.confidence, 0.0)
    
    def test_recognize_learn_concept(self):
        """Test recognition of learn concept intent."""
        test_cases = [
            "What is a fork?",
            "Tell me about pins",
            "Teach me about endgame strategy",
        ]
        
        for input_text in test_cases:
            result = recognize_intent(input_text)
            self.assertEqual(result.intent, IntentType.LEARN_CONCEPT)
            self.assertGreater(result.confidence, 0.0)
    
    def test_unknown_intent(self):
        """Test unknown intent for unrecognized input."""
        result = recognize_intent("Hello, how are you?")
        self.assertEqual(result.intent, IntentType.UNKNOWN)
        self.assertEqual(result.confidence, 0.0)
    
    def test_detect_complexity_beginner(self):
        """Test beginner complexity detection."""
        test_cases = [
            "Explain like I'm 5",
            "Keep it simple please",
            "I'm a beginner, basic explanation",
        ]
        
        for input_text in test_cases:
            complexity = detect_complexity(input_text)
            self.assertEqual(complexity, ComplexityLevel.BEGINNER)
    
    def test_detect_complexity_advanced(self):
        """Test advanced complexity detection."""
        test_cases = [
            "Give me a detailed analysis",
            "Show me the tactical variations",
            "Advanced evaluation with centipawn loss",
        ]
        
        for input_text in test_cases:
            complexity = detect_complexity(input_text)
            self.assertEqual(complexity, ComplexityLevel.ADVANCED)
    
    def test_detect_complexity_intermediate(self):
        """Test default intermediate complexity."""
        complexity = detect_complexity("What's the best move?")
        self.assertEqual(complexity, ComplexityLevel.INTERMEDIATE)
    
    def test_extract_fen(self):
        """Test FEN extraction from input."""
        fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
        input_text = f"Analyze this position: {fen}"
        
        extracted = extract_fen_from_input(input_text)
        self.assertIsNotNone(extracted)
        self.assertIn("rnbqkbnr", extracted)
    
    def test_extract_fen_not_present(self):
        """Test FEN extraction when not present."""
        input_text = "What's the best move in the starting position?"
        extracted = extract_fen_from_input(input_text)
        self.assertIsNone(extracted)
    
    def test_extract_moves(self):
        """Test move extraction from input."""
        input_text = "Compare e4 and d4, or maybe Nf3"
        moves = extract_moves_from_input(input_text)
        
        self.assertIn("e4", moves)
        self.assertIn("d4", moves)
        self.assertIn("Nf3", moves)
    
    def test_extract_moves_with_captures(self):
        """Test move extraction with captures and checks."""
        input_text = "What about Qxd5+ or Bxf7#"
        moves = extract_moves_from_input(input_text)
        
        self.assertIn("Qxd5+", moves)
        self.assertIn("Bxf7#", moves)


class TestNaturalLanguageAgent(unittest.TestCase):
    """Test NaturalLanguageAgent service."""
    
    def setUp(self):
        """Set up test fixtures."""
        self.loop = asyncio.new_event_loop()
        asyncio.set_event_loop(self.loop)
        
        # Mock worker pool
        self.mock_pool = MagicMock()
        self.agent = NaturalLanguageAgent(self.mock_pool)
    
    def tearDown(self):
        """Clean up."""
        self.loop.close()
    
    def test_process_suggest_move_request(self):
        """Test processing a move suggestion request."""
        async def run_test():
            # Mock the pool submit
            mock_result = AnalysisResult(
                request_id="test-1",
                best_move="e5",
                evaluation=0.5,
                depth=18,
                principal_variation=["e5", "Nf3", "Nc6"],
                nodes_searched=100000,
                time_ms=500,
            )
            self.mock_pool.submit = AsyncMock(return_value=mock_result)
            
            response = await self.agent.process_request(
                user_input="What's the best move?",
                fen="rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1",
            )
            
            self.assertEqual(response.intent, IntentType.SUGGEST_MOVE)
            self.assertEqual(response.best_move, "e5")
            self.assertGreater(len(response.natural_language_response), 0)
        
        self.loop.run_until_complete(run_test())
    
    def test_process_analyze_position_request(self):
        """Test processing a position analysis request."""
        async def run_test():
            mock_result = AnalysisResult(
                request_id="test-2",
                best_move="Nf3",
                evaluation=0.3,
                depth=18,
                principal_variation=["Nf3", "e5", "e4"],
                nodes_searched=150000,
                time_ms=600,
            )
            self.mock_pool.submit = AsyncMock(return_value=mock_result)
            
            response = await self.agent.process_request(
                user_input="Analyze this position",
                fen="rnbqkbnr/pppppppp/8/8/8/5N2/PPPPPPPP/RNBQKB1R b KQkq - 1 1",
            )
            
            self.assertEqual(response.intent, IntentType.ANALYZE_POSITION)
            self.assertIsNotNone(response.evaluation)
        
        self.loop.run_until_complete(run_test())
    
    def test_process_without_fen(self):
        """Test processing request without FEN."""
        async def run_test():
            response = await self.agent.process_request(
                user_input="What's the best move?",
            )
            
            # Should ask for position
            self.assertGreater(len(response.natural_language_response), 0)
            self.assertIn("position", response.natural_language_response.lower())
        
        self.loop.run_until_complete(run_test())
    
    def test_process_hint_request(self):
        """Test processing a hint request."""
        async def run_test():
            mock_result = AnalysisResult(
                request_id="test-3",
                best_move="Qh5",
                evaluation=1.5,
                depth=12,
                principal_variation=["Qh5", "Nf6"],
                nodes_searched=50000,
                time_ms=300,
            )
            self.mock_pool.submit = AsyncMock(return_value=mock_result)
            
            response = await self.agent.process_request(
                user_input="Give me a hint",
                fen="r1bqkbnr/pppp1ppp/2n5/4p2Q/4P3/8/PPPP1PPP/RNB1KBNR w KQkq - 2 3",
            )
            
            self.assertEqual(response.intent, IntentType.GET_HINT)
            # Hint should not reveal the full best move
            self.assertIsNone(response.best_move)
        
        self.loop.run_until_complete(run_test())
    
    def test_process_learn_concept_request(self):
        """Test processing a concept learning request."""
        async def run_test():
            response = await self.agent.process_request(
                user_input="What is a fork in chess?",
            )
            
            self.assertEqual(response.intent, IntentType.LEARN_CONCEPT)
            self.assertIn("fork", response.natural_language_response.lower())
            self.assertGreater(len(response.natural_language_response), 0)
        
        self.loop.run_until_complete(run_test())
    
    def test_process_unknown_intent(self):
        """Test processing request with unknown intent."""
        async def run_test():
            response = await self.agent.process_request(
                user_input="What's the weather like?",
            )
            
            self.assertEqual(response.intent, IntentType.UNKNOWN)
            self.assertIn("not sure", response.natural_language_response.lower())
        
        self.loop.run_until_complete(run_test())
    
    def test_different_complexity_levels(self):
        """Test responses at different complexity levels."""
        async def run_test():
            mock_result = AnalysisResult(
                request_id="test-4",
                best_move="e4",
                evaluation=0.4,
                depth=18,
                principal_variation=["e4", "e5", "Nf3"],
                nodes_searched=200000,
                time_ms=700,
            )
            self.mock_pool.submit = AsyncMock(return_value=mock_result)
            
            # Test beginner
            beginner_response = await self.agent.process_request(
                user_input="Suggest a move, keep it simple",
                fen="rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            )
            
            # Test advanced
            advanced_response = await self.agent.process_request(
                user_input="Give me advanced analysis with detailed variations",
                fen="rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            )
            
            # Both should succeed but with different content
            self.assertEqual(beginner_response.intent, IntentType.SUGGEST_MOVE)
            self.assertEqual(advanced_response.intent, IntentType.SUGGEST_MOVE)
            
            # Advanced should mention more technical terms
            self.assertNotEqual(
                beginner_response.natural_language_response,
                advanced_response.natural_language_response,
            )
        
        self.loop.run_until_complete(run_test())
    
    def test_request_history(self):
        """Test request history tracking."""
        async def run_test():
            mock_result = AnalysisResult(
                request_id="test-5",
                best_move="d4",
                evaluation=0.2,
                depth=18,
                principal_variation=["d4"],
                nodes_searched=100000,
                time_ms=500,
            )
            self.mock_pool.submit = AsyncMock(return_value=mock_result)
            
            response = await self.agent.process_request(
                user_input="Best move?",
                fen="rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            )
            
            # Check history
            history = self.agent.get_request_history()
            self.assertGreater(len(history), 0)
            
            # Check specific request
            specific = self.agent.get_request_history(response.request_id)
            self.assertIsNotNone(specific)
            self.assertEqual(specific.request_id, response.request_id)
        
        self.loop.run_until_complete(run_test())


if __name__ == "__main__":
    unittest.main()
