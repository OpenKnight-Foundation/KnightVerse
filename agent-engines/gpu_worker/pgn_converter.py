from __future__ import annotations

import logging
import chess
import chess.pgn
import numpy as np
from typing import List, Tuple, Optional

logger = logging.getLogger("XLMate.PGNConverter")

class PGNConverter:
    """
    Converter for PGN files into Neural Network training tensors.
    Uses an 8x8xN bitboard representation.
    """

    def __init__(self, include_metadata: bool = True):
        self.include_metadata = include_metadata

    def board_to_tensor(self, board: chess.Board) -> np.ndarray:
        """
        Converts a chess.Board to an 8x8x18 bitboard tensor.
        - 12 planes for pieces (6 white, 6 black)
        - 1 plane for turn (all 1s if white to move, all 0s if black)
        - 4 planes for castling rights (K, Q, k, q)
        - 1 plane for en passant square
        """
        tensor = np.zeros((18, 8, 8), dtype=np.float32)

        # Piece planes
        for piece_type in range(1, 7):
            for color in [chess.WHITE, chess.BLACK]:
                plane_idx = (piece_type - 1) * 2 + (0 if color == chess.WHITE else 1)
                squares = board.pieces(piece_type, color)
                for square in squares:
                    row, col = divmod(square, 8)
                    tensor[plane_idx, row, col] = 1.0

        # Turn plane
        if board.turn == chess.WHITE:
            tensor[12, :, :] = 1.0

        # Castling planes
        if board.has_kingside_castling_rights(chess.WHITE):
            tensor[13, :, :] = 1.0
        if board.has_queenside_castling_rights(chess.WHITE):
            tensor[14, :, :] = 1.0
        if board.has_kingside_castling_rights(chess.BLACK):
            tensor[15, :, :] = 1.0
        if board.has_queenside_castling_rights(chess.BLACK):
            tensor[16, :, :] = 1.0

        # En passant plane
        if board.ep_square is not None:
            row, col = divmod(board.ep_square, 8)
            tensor[17, row, col] = 1.0

        return tensor

    def convert_pgn(self, pgn_path: str, max_games: Optional[int] = None) -> Tuple[np.ndarray, np.ndarray]:
        """
        Converts a PGN file into a dataset of (X, y) where:
        X: Board tensors
        y: Target moves (represented as square indices)
        """
        logger.info(f"Starting conversion of PGN: {pgn_path}")
        X, y = [], []
        games_processed = 0

        with open(pgn_path, "r") as pgn_file:
            while True:
                game = chess.pgn.read_game(pgn_file)
                if game is None or (max_games and games_processed >= max_games):
                    break

                board = game.board()
                for move in game.mainline_moves():
                    X.append(self.board_to_tensor(board))
                    # Encode move as (from_square, to_square)
                    y.append(np.array([move.from_square, move.to_square]))
                    board.push(move)

                games_processed += 1
                if games_processed % 100 == 0:
                    logger.info(f"Processed {games_processed} games...")

        logger.info(f"Finished conversion. Total games: {games_processed}, Total positions: {len(X)}")
        return np.array(X), np.array(y)

    def save_dataset(self, X: np.ndarray, y: np.ndarray, output_path: str):
        """Saves the dataset as a compressed NumPy file."""
        np.savez_compressed(output_path, X=X, y=y)
        logger.info(f"Dataset saved to {output_path}")
