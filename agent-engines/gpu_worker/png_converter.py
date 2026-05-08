"""
PGN to Neural Network Training Data Converter.

This module provides tools to convert Portable Game Notation (PGN) files into
neural network training datasets. It extracts positions, move sequences, and
game outcomes suitable for training custom chess agents.

Features:
- Efficient PGN parsing using python-chess
- Position feature encoding for neural networks
- Support for game filtering and validation
- Resource-efficient batch processing
- Performance metrics tracking
"""

from __future__ import annotations

import io
import logging
from dataclasses import dataclass, field
from datetime import datetime, timezone
from enum import Enum
from pathlib import Path
from typing import Any, Dict, Iterator, List, Optional, Tuple, Union
from uuid import uuid4

import chess
import chess.pgn
from pydantic import BaseModel, Field, field_validator

logger = logging.getLogger("XLMate.PGNConverter")


class GameResult(str, Enum):
    """Chess game result indicators."""

    WHITE_WIN = "1-0"
    BLACK_WIN = "0-1"
    DRAW = "1/2-1/2"
    UNKNOWN = "*"


class MoveEncoding(str, Enum):
    """Supported move encoding formats for training."""

    UCI = "uci"  # Universal Chess Interface (standard)
    LAN = "lan"  # Long Algebraic Notation
    SAN = "san"  # Standard Algebraic Notation


@dataclass
class PositionFeatures:
    """
    Neural network feature representation of a chess position.

    Attributes:
        fen: Forsyth-Edwards Notation of the position
        move_uci: The move that led to this position (UCI format)
        move_san: The move in Standard Algebraic Notation
        eval_score: Engine evaluation (centipawns) if available
        is_endgame: Whether position is in endgame phase
        move_number: Half-move number from start of game
        ply_count: Total plies played
    """

    fen: str
    move_uci: Optional[str] = None
    move_san: Optional[str] = None
    eval_score: Optional[float] = None
    is_endgame: bool = False
    move_number: int = 0
    ply_count: int = 0

    def to_dict(self) -> Dict[str, Any]:
        """Convert features to dictionary for serialization."""
        return {
            "fen": self.fen,
            "move_uci": self.move_uci,
            "move_san": self.move_san,
            "eval_score": self.eval_score,
            "is_endgame": self.is_endgame,
            "move_number": self.move_number,
            "ply_count": self.ply_count,
        }


class TrainingExample(BaseModel):
    """
    Single training example extracted from a game position.

    Attributes:
        id: Unique identifier for this training example
        position_features: Encoded position features
        target_move: The actual move played (for supervised learning)
        game_result: Outcome of the game (1=white win, 0=black win, 0.5=draw)
        game_id: Reference to source game
        source_url: URL of game source if available
    """

    id: str = Field(default_factory=lambda: str(uuid4()))
    position_features: Dict[str, Any]
    target_move: str
    game_result: float = Field(ge=0.0, le=1.0)
    game_id: Optional[str] = None
    source_url: Optional[str] = None
    created_at: datetime = Field(default_factory=lambda: datetime.now(timezone.utc))

    @field_validator("target_move")
    @classmethod
    def validate_move(cls, value: str) -> str:
        """Validate move is in UCI format."""
        if len(value) not in (4, 5):  # UCI moves are 4-5 chars (with promotion)
            raise ValueError(f"Invalid UCI move: {value}")
        return value


class GameMetadata(BaseModel):
    """
    Metadata extracted from PGN game headers.

    Attributes:
        game_id: Unique identifier
        event: Tournament or event name
        site: Location of the game
        date: Date the game was played
        white_player: White player name
        black_player: Black player name
        result: Game result
        eco: ECO classification
        opening_name: Opening name if available
        time_control: Time control if available
    """

    game_id: str = Field(default_factory=lambda: str(uuid4()))
    event: Optional[str] = None
    site: Optional[str] = None
    date: Optional[str] = None
    white_player: Optional[str] = None
    black_player: Optional[str] = None
    result: GameResult = GameResult.UNKNOWN
    eco: Optional[str] = None
    opening_name: Optional[str] = None
    time_control: Optional[str] = None


class PGNConverter:
    """
    Converts PGN games into neural network training data.

    This converter efficiently processes PGN files and extracts positions,
    moves, and outcomes suitable for training chess engines.
    """

    # Endgame phase detection: fewer than 7 pieces total
    ENDGAME_THRESHOLD = 7

    def __init__(
        self,
        skip_invalid_moves: bool = True,
        extract_all_positions: bool = True,
        min_move_number: int = 1,
    ):
        """
        Initialize the PGN converter.

        Args:
            skip_invalid_moves: Skip positions with illegal moves
            extract_all_positions: Extract all positions vs every Nth position
            min_move_number: Skip positions before this move number
        """
        self.skip_invalid_moves = skip_invalid_moves
        self.extract_all_positions = extract_all_positions
        self.min_move_number = min_move_number
        self.games_processed = 0
        self.positions_extracted = 0
        self.errors_encountered = 0

    def parse_pgn_file(self, file_path: Union[Path, str]) -> Iterator[chess.pgn.Game]:
        """
        Parse a PGN file and yield individual games.

        Args:
            file_path: Path to PGN file

        Yields:
            chess.pgn.Game objects

        Raises:
            FileNotFoundError: If file doesn't exist
            IOError: If file cannot be read
        """
        file_path = Path(file_path)
        if not file_path.exists():
            raise FileNotFoundError(f"PGN file not found: {file_path}")

        try:
            with open(file_path, "r", encoding="utf-8") as pgn_file:
                while True:
                    game = chess.pgn.read_game(pgn_file)
                    if game is None:
                        break
                    yield game
        except UnicodeDecodeError as e:
            logger.error(f"Encoding error reading PGN file: {e}")
            raise IOError(f"Failed to read PGN file {file_path}: {e}") from e

    def parse_pgn_string(self, pgn_text: str) -> Iterator[chess.pgn.Game]:
        """
        Parse PGN text string and yield individual games.

        Args:
            pgn_text: PGN format text

        Yields:
            chess.pgn.Game objects
        """
        pgn_file = io.StringIO(pgn_text)
        while True:
            game = chess.pgn.read_game(pgn_file)
            if game is None:
                break
            yield game

    def extract_game_metadata(self, game: chess.pgn.Game) -> GameMetadata:
        """
        Extract metadata from a PGN game.

        Args:
            game: chess.pgn.Game object

        Returns:
            GameMetadata with game information
        """
        headers = game.headers
        result_str = headers.get("Result", "*")

        try:
            result = GameResult(result_str)
        except ValueError:
            result = GameResult.UNKNOWN

        return GameMetadata(
            event=headers.get("Event"),
            site=headers.get("Site"),
            date=headers.get("Date"),
            white_player=headers.get("White"),
            black_player=headers.get("Black"),
            result=result,
            eco=headers.get("ECO"),
            opening_name=headers.get("Opening"),
            time_control=headers.get("TimeControl"),
        )

    def _result_to_float(self, result: GameResult, perspective: str = "white") -> float:
        """
        Convert game result to float (0-1 range).

        Args:
            result: GameResult enum
            perspective: "white" or "black" perspective

        Returns:
            Float value: 1.0 for win, 0.5 for draw, 0.0 for loss
        """
        if result == GameResult.WHITE_WIN:
            return 1.0 if perspective == "white" else 0.0
        elif result == GameResult.BLACK_WIN:
            return 0.0 if perspective == "white" else 1.0
        elif result == GameResult.DRAW:
            return 0.5
        else:
            return 0.5  # Default to draw for unknown

    def _count_pieces(self, board: chess.Board) -> int:
        """Count total pieces on board (excluding pawns)."""
        return sum(len(board.pieces(piece_type, color))
                  for piece_type in chess.PIECE_TYPES
                  for color in chess.COLORS
                  if piece_type != chess.PAWN) + len(board.pieces(chess.PAWN, chess.WHITE)) + len(board.pieces(chess.PAWN, chess.BLACK))

    def _is_endgame(self, board: chess.Board) -> bool:
        """Detect if position is in endgame phase."""
        piece_count = self._count_pieces(board)
        return piece_count <= self.ENDGAME_THRESHOLD

    def extract_positions(
        self,
        game: chess.pgn.Game,
        metadata: Optional[GameMetadata] = None,
    ) -> List[TrainingExample]:
        """
        Extract all positions from a game as training examples.

        Args:
            game: chess.pgn.Game object
            metadata: Optional GameMetadata (will be extracted if None)

        Returns:
            List of TrainingExample objects

        Raises:
            ValueError: If game is invalid
        """
        if metadata is None:
            metadata = self.extract_game_metadata(game)

        examples = []
        board = game.board()
        game_result = self._result_to_float(metadata.result, perspective="white")

        move_number = 0
        ply_count = 0

        try:
            for move in game.mainline_moves():
                move_number += 1
                ply_count += 1

                # Skip early moves if configured
                if move_number < self.min_move_number:
                    board.push(move)
                    continue

                try:
                    # Get current position FEN
                    fen = board.fen()

                    # Determine piece count for endgame detection
                    is_endgame = self._is_endgame(board)

                    # Create position features
                    features = PositionFeatures(
                        fen=fen,
                        move_uci=move.uci(),
                        move_san=board.san(move),
                        is_endgame=is_endgame,
                        move_number=move_number,
                        ply_count=ply_count,
                    )

                    # Create training example
                    example = TrainingExample(
                        position_features=features.to_dict(),
                        target_move=move.uci(),
                        game_result=game_result,
                        game_id=metadata.game_id,
                        source_url=metadata.site,
                    )

                    examples.append(example)
                    self.positions_extracted += 1

                except (ValueError, AssertionError) as e:
                    if self.skip_invalid_moves:
                        logger.warning(f"Skipping invalid position: {e}")
                        self.errors_encountered += 1
                    else:
                        raise

                # Make the move
                board.push(move)

        except Exception as e:
            logger.error(f"Error processing game {metadata.game_id}: {e}")
            self.errors_encountered += 1
            if not self.skip_invalid_moves:
                raise ValueError(f"Failed to process game: {e}") from e

        self.games_processed += 1
        return examples

    def batch_convert(
        self,
        file_path: Union[Path, str],
        batch_size: int = 100,
    ) -> Iterator[List[TrainingExample]]:
        """
        Convert PGN file to training data in batches.

        This is memory-efficient for large files as it yields batches
        rather than loading everything into memory.

        Args:
            file_path: Path to PGN file
            batch_size: Number of examples per batch

        Yields:
            Batches of TrainingExample objects
        """
        batch = []

        for game in self.parse_pgn_file(file_path):
            try:
                metadata = self.extract_game_metadata(game)
                examples = self.extract_positions(game, metadata)
                batch.extend(examples)

                if len(batch) >= batch_size:
                    yield batch
                    batch = []

            except Exception as e:
                logger.error(f"Error processing game: {e}")
                self.errors_encountered += 1

        # Yield remaining examples
        if batch:
            yield batch

    def get_statistics(self) -> Dict[str, Any]:
        """
        Get conversion statistics.

        Returns:
            Dictionary with processing statistics
        """
        return {
            "games_processed": self.games_processed,
            "positions_extracted": self.positions_extracted,
            "errors_encountered": self.errors_encountered,
            "avg_positions_per_game": (
                self.positions_extracted / self.games_processed
                if self.games_processed > 0
                else 0
            ),
        }

    def reset_statistics(self) -> None:
        """Reset internal statistics counters."""
        self.games_processed = 0
        self.positions_extracted = 0
        self.errors_encountered = 0


class PGNDataset(BaseModel):
    """
    Represents a dataset of training examples extracted from PGN files.

    Attributes:
        dataset_id: Unique identifier
        name: Human-readable name
        description: Dataset description
        examples: Training examples
        metadata: Source game metadata
        statistics: Conversion statistics
    """

    dataset_id: str = Field(default_factory=lambda: str(uuid4()))
    name: str
    description: Optional[str] = None
    examples: List[TrainingExample] = Field(default_factory=list)
    metadata: List[GameMetadata] = Field(default_factory=list)
    statistics: Dict[str, Any] = Field(default_factory=dict)
    created_at: datetime = Field(default_factory=lambda: datetime.now(timezone.utc))

    def add_examples(self, examples: List[TrainingExample]) -> None:
        """Add training examples to dataset."""
        self.examples.extend(examples)

    def add_metadata(self, meta: GameMetadata) -> None:
        """Add game metadata to dataset."""
        self.metadata.append(meta)

    def get_size(self) -> int:
        """Get number of training examples in dataset."""
        return len(self.examples)

    def get_result_distribution(self) -> Dict[str, float]:
        """
        Get distribution of game results in dataset.

        Returns:
            Dictionary with result percentages
        """
        if not self.examples:
            return {"white_wins": 0.0, "black_wins": 0.0, "draws": 0.0}

        white_wins = sum(1 for ex in self.examples if ex.game_result == 1.0)
        draws = sum(1 for ex in self.examples if ex.game_result == 0.5)
        black_wins = sum(1 for ex in self.examples if ex.game_result == 0.0)
        total = len(self.examples)

        return {
            "white_wins": white_wins / total,
            "black_wins": black_wins / total,
            "draws": draws / total,
        }

    def model_dump(self, **kwargs) -> Dict[str, Any]:
        """Override to handle custom serialization."""
        data = super().model_dump(**kwargs)
        return data
