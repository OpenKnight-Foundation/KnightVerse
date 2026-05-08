"""
Unit tests for PGN to Neural Network converter.

Tests cover:
- PGN parsing from files and strings
- Position feature extraction
- Training example generation
- Dataset creation and validation
- Edge cases and error handling
- Resource efficiency
"""

import asyncio
import tempfile
import unittest
from pathlib import Path

import chess
import chess.pgn

from gpu_worker.pgn_converter import (
    GameMetadata,
    GameResult,
    PositionFeatures,
    PGNConverter,
    PGNDataset,
    TrainingExample,
)


class TestPositionFeatures(unittest.TestCase):
    """Test PositionFeatures data class."""

    def test_position_features_creation(self):
        """Test creating position features."""
        features = PositionFeatures(
            fen="rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            move_uci="e2e4",
            move_san="e4",
            is_endgame=False,
            move_number=1,
        )
        self.assertEqual(features.move_uci, "e2e4")
        self.assertFalse(features.is_endgame)

    def test_position_features_to_dict(self):
        """Test converting features to dictionary."""
        features = PositionFeatures(
            fen="rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1",
            move_uci="e2e4",
            move_san="e4",
        )
        features_dict = features.to_dict()
        self.assertIn("fen", features_dict)
        self.assertEqual(features_dict["move_uci"], "e2e4")


class TestTrainingExample(unittest.TestCase):
    """Test TrainingExample model."""

    def test_training_example_creation(self):
        """Test creating training example."""
        features = {
            "fen": "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1",
            "move_uci": "e2e4",
        }
        example = TrainingExample(
            position_features=features,
            target_move="e7e5",
            game_result=0.5,
        )
        self.assertEqual(example.target_move, "e7e5")
        self.assertEqual(example.game_result, 0.5)

    def test_training_example_move_validation(self):
        """Test move validation in training example."""
        features = {"fen": "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1"}

        # Valid 4-char move
        example = TrainingExample(
            position_features=features,
            target_move="e7e5",
            game_result=0.5,
        )
        self.assertEqual(example.target_move, "e7e5")

        # Valid 5-char move (promotion)
        example = TrainingExample(
            position_features=features,
            target_move="e7e8q",
            game_result=1.0,
        )
        self.assertEqual(example.target_move, "e7e8q")

        # Invalid move
        with self.assertRaises(ValueError):
            TrainingExample(
                position_features=features,
                target_move="invalid",
                game_result=0.5,
            )


class TestGameMetadata(unittest.TestCase):
    """Test GameMetadata model."""

    def test_metadata_creation(self):
        """Test creating game metadata."""
        metadata = GameMetadata(
            event="Test Tournament",
            white_player="Player A",
            black_player="Player B",
            result=GameResult.WHITE_WIN,
        )
        self.assertEqual(metadata.event, "Test Tournament")
        self.assertEqual(metadata.result, GameResult.WHITE_WIN)

    def test_metadata_with_defaults(self):
        """Test metadata with default values."""
        metadata = GameMetadata()
        self.assertIsNone(metadata.event)
        self.assertEqual(metadata.result, GameResult.UNKNOWN)


class TestPGNConverter(unittest.TestCase):
    """Test PGN converter functionality."""

    def setUp(self):
        """Set up test fixtures."""
        self.converter = PGNConverter()

        # Sample PGN: Scholar's Mate (shortest possible mate)
        self.simple_pgn = """
[Event "Test"]
[Site "Test"]
[White "A"]
[Black "B"]
[Result "1-0"]

1. e4 e5 2. Bc4 Nc6 3. Qh5 Nf6 4. Qxf7# 1-0
"""

        # Sample PGN: Opening position
        self.opening_pgn = """
[Event "Opening"]
[White "A"]
[Black "B"]
[Result "1/2-1/2"]

1. e4 c5 2. Nf3 d6 1/2-1/2
"""

        # Multi-game PGN
        self.multi_game_pgn = """
[Event "Game 1"]
[Result "1-0"]

1. e4 e5 2. Nf3 1-0

[Event "Game 2"]
[Result "0-1"]

1. d4 d5 2. c4 0-1
"""

    def test_parse_pgn_string(self):
        """Test parsing PGN from string."""
        games = list(self.converter.parse_pgn_string(self.simple_pgn))
        self.assertEqual(len(games), 1)
        self.assertEqual(games[0].headers["Event"], "Test")

    def test_parse_multiple_games(self):
        """Test parsing multiple games from single string."""
        games = list(self.converter.parse_pgn_string(self.multi_game_pgn))
        self.assertEqual(len(games), 2)

    def test_extract_game_metadata(self):
        """Test extracting game metadata."""
        games = list(self.converter.parse_pgn_string(self.simple_pgn))
        metadata = self.converter.extract_game_metadata(games[0])

        self.assertEqual(metadata.event, "Test")
        self.assertEqual(metadata.white_player, "A")
        self.assertEqual(metadata.black_player, "B")
        self.assertEqual(metadata.result, GameResult.WHITE_WIN)

    def test_extract_positions_simple_game(self):
        """Test extracting positions from simple game."""
        games = list(self.converter.parse_pgn_string(self.opening_pgn))
        game = games[0]
        metadata = self.converter.extract_game_metadata(game)

        examples = self.converter.extract_positions(game, metadata)

        # Opening should have at least 4 positions (2 moves by each side)
        self.assertGreaterEqual(len(examples), 4)

        # Verify example structure
        for example in examples:
            self.assertIsNotNone(example.position_features["fen"])
            self.assertIsNotNone(example.target_move)
            self.assertIn(example.game_result, [0.0, 0.5, 1.0])

    def test_result_to_float_conversions(self):
        """Test result conversion to float."""
        # White win from white perspective
        result = self.converter._result_to_float(GameResult.WHITE_WIN, "white")
        self.assertEqual(result, 1.0)

        # White win from black perspective
        result = self.converter._result_to_float(GameResult.WHITE_WIN, "black")
        self.assertEqual(result, 0.0)

        # Draw
        result = self.converter._result_to_float(GameResult.DRAW, "white")
        self.assertEqual(result, 0.5)

    def test_endgame_detection(self):
        """Test endgame phase detection."""
        board = chess.Board()
        # Opening - not endgame
        self.assertFalse(self.converter._is_endgame(board))

        # Clear most pieces
        board.clear()
        board.set_piece_at(chess.E1, chess.Piece(chess.KING, chess.WHITE))
        board.set_piece_at(chess.E8, chess.Piece(chess.KING, chess.BLACK))
        board.set_piece_at(chess.A1, chess.Piece(chess.ROOK, chess.WHITE))
        board.set_piece_at(chess.H8, chess.Piece(chess.ROOK, chess.BLACK))

        # Should be endgame (only kings and one rook each)
        self.assertTrue(self.converter._is_endgame(board))

    def test_min_move_number_filter(self):
        """Test filtering positions before minimum move number."""
        converter = PGNConverter(min_move_number=10)
        games = list(converter.parse_pgn_string(self.opening_pgn))
        examples = converter.extract_positions(games[0])

        # Should skip all positions since game only has 2 moves
        self.assertEqual(len(examples), 0)

    def test_skip_invalid_moves(self):
        """Test skip_invalid_moves flag."""
        converter = PGNConverter(skip_invalid_moves=True)
        games = list(converter.parse_pgn_string(self.simple_pgn))

        # Should not raise even with potentially problematic PGN
        examples = converter.extract_positions(games[0])
        self.assertGreater(len(examples), 0)

    def test_pgn_file_parsing(self):
        """Test parsing PGN from file."""
        with tempfile.NamedTemporaryFile(mode="w", suffix=".pgn", delete=False) as f:
            f.write(self.multi_game_pgn)
            f.flush()
            temp_path = f.name

        try:
            games = list(self.converter.parse_pgn_file(temp_path))
            self.assertEqual(len(games), 2)
        finally:
            Path(temp_path).unlink()

    def test_pgn_file_not_found(self):
        """Test handling of missing PGN file."""
        with self.assertRaises(FileNotFoundError):
            list(self.converter.parse_pgn_file("/nonexistent/file.pgn"))

    def test_batch_convert(self):
        """Test batch conversion."""
        with tempfile.NamedTemporaryFile(mode="w", suffix=".pgn", delete=False) as f:
            f.write(self.multi_game_pgn)
            f.flush()
            temp_path = f.name

        try:
            batches = list(self.converter.batch_convert(temp_path, batch_size=2))
            self.assertGreater(len(batches), 0)

            # Verify batch structure
            for batch in batches:
                self.assertIsInstance(batch, list)
                for example in batch:
                    self.assertIsInstance(example, TrainingExample)
        finally:
            Path(temp_path).unlink()

    def test_statistics_tracking(self):
        """Test statistics accumulation."""
        self.converter.reset_statistics()
        self.assertEqual(self.converter.games_processed, 0)
        self.assertEqual(self.converter.positions_extracted, 0)

        games = list(self.converter.parse_pgn_string(self.multi_game_pgn))
        for game in games:
            self.converter.extract_positions(game)

        stats = self.converter.get_statistics()
        self.assertEqual(stats["games_processed"], 2)
        self.assertGreater(stats["positions_extracted"], 0)

    def test_statistics_reset(self):
        """Test resetting statistics."""
        games = list(self.converter.parse_pgn_string(self.simple_pgn))
        self.converter.extract_positions(games[0])

        self.converter.reset_statistics()
        stats = self.converter.get_statistics()

        self.assertEqual(stats["games_processed"], 0)
        self.assertEqual(stats["positions_extracted"], 0)


class TestPGNDataset(unittest.TestCase):
    """Test PGNDataset model."""

    def setUp(self):
        """Set up test fixtures."""
        self.dataset = PGNDataset(
            name="Test Dataset",
            description="Test dataset for unit tests",
        )

    def test_dataset_creation(self):
        """Test creating dataset."""
        self.assertEqual(self.dataset.name, "Test Dataset")
        self.assertEqual(self.dataset.get_size(), 0)

    def test_add_examples(self):
        """Test adding examples to dataset."""
        features = {"fen": "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1"}
        examples = [
            TrainingExample(position_features=features, target_move="e7e5", game_result=1.0),
            TrainingExample(position_features=features, target_move="c7c5", game_result=0.5),
        ]

        self.dataset.add_examples(examples)
        self.assertEqual(self.dataset.get_size(), 2)

    def test_result_distribution(self):
        """Test result distribution calculation."""
        features = {"fen": "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1"}
        examples = [
            TrainingExample(position_features=features, target_move="e7e5", game_result=1.0),
            TrainingExample(position_features=features, target_move="c7c5", game_result=1.0),
            TrainingExample(position_features=features, target_move="d7d5", game_result=0.5),
        ]

        self.dataset.add_examples(examples)
        distribution = self.dataset.get_result_distribution()

        self.assertAlmostEqual(distribution["white_wins"], 2 / 3, places=2)
        self.assertAlmostEqual(distribution["draws"], 1 / 3, places=2)
        self.assertAlmostEqual(distribution["black_wins"], 0.0, places=2)

    def test_empty_result_distribution(self):
        """Test result distribution on empty dataset."""
        distribution = self.dataset.get_result_distribution()
        self.assertEqual(distribution["white_wins"], 0.0)
        self.assertEqual(distribution["black_wins"], 0.0)
        self.assertEqual(distribution["draws"], 0.0)

    def test_add_metadata(self):
        """Test adding metadata to dataset."""
        metadata = GameMetadata(
            event="Test",
            white_player="A",
            result=GameResult.WHITE_WIN,
        )
        self.dataset.add_metadata(metadata)
        self.assertEqual(len(self.dataset.metadata), 1)


class TestEdgeCases(unittest.TestCase):
    """Test edge cases and error handling."""

    def setUp(self):
        """Set up test fixtures."""
        self.converter = PGNConverter(skip_invalid_moves=True)

    def test_empty_pgn_string(self):
        """Test parsing empty PGN string."""
        games = list(self.converter.parse_pgn_string(""))
        self.assertEqual(len(games), 0)

    def test_pgn_with_comments(self):
        """Test PGN with inline comments."""
        pgn_with_comments = """
[Event "Test"]
[Result "1-0"]

1. e4 {best move} e5 {black response} 2. Nf3 1-0
"""
        games = list(self.converter.parse_pgn_string(pgn_with_comments))
        self.assertEqual(len(games), 1)

        examples = self.converter.extract_positions(games[0])
        self.assertGreater(len(examples), 0)

    def test_pgn_with_variations(self):
        """Test PGN with variations (only mainline should be extracted)."""
        pgn_with_vars = """
[Event "Test"]
[Result "1-0"]

1. e4 (1. d4 d5) e5 2. Nf3 1-0
"""
        games = list(self.converter.parse_pgn_string(pgn_with_vars))
        self.assertEqual(len(games), 1)

        examples = self.converter.extract_positions(games[0])
        # Should only extract mainline, not variations
        self.assertGreater(len(examples), 0)

    def test_unicode_player_names(self):
        """Test handling unicode characters in metadata."""
        pgn_unicode = """
[Event "Τουρνουά"]
[White "Παίκτης Α"]
[Black "選手B"]
[Result "1-0"]

1. e4 e5 2. Nf3 1-0
"""
        games = list(self.converter.parse_pgn_string(pgn_unicode))
        metadata = self.converter.extract_game_metadata(games[0])

        self.assertIsNotNone(metadata.white_player)
        self.assertIsNotNone(metadata.black_player)


if __name__ == "__main__":
    unittest.main()
