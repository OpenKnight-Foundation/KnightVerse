import pytest
import numpy as np
import chess
import chess.pgn
import io
import os
from gpu_worker.pgn_converter import PGNConverter

def test_board_to_tensor_dimensions():
    converter = PGNConverter()
    board = chess.Board()
    tensor = converter.board_to_tensor(board)
    
    assert tensor.shape == (18, 8, 8)
    assert tensor.dtype == np.float32

def test_board_to_tensor_content():
    converter = PGNConverter()
    board = chess.Board()
    tensor = converter.board_to_tensor(board)
    
    # White pawns at row 1 (index 1 in 0-7)
    # Piece type 1 is Pawn. White plane is index (1-1)*2 + 0 = 0
    assert np.all(tensor[0, 1, :] == 1.0)
    # Black pawns at row 6
    # Black plane is index (1-1)*2 + 1 = 1
    assert np.all(tensor[1, 6, :] == 1.0)
    
    # Turn plane (White to move)
    assert np.all(tensor[12, :, :] == 1.0)

def test_pgn_conversion(tmp_path):
    pgn_content = """[Event "Example"]
[Site "Local"]
[Date "2024.01.01"]
[Round "1"]
[White "Player1"]
[Black "Player2"]
[Result "1-0"]

1. e4 e5 2. Nf3 Nc6 3. Bb5 a6 1-0"""
    
    pgn_file = tmp_path / "test.pgn"
    pgn_file.write_text(pgn_content)
    
    converter = PGNConverter()
    X, y = converter.convert_pgn(str(pgn_file))
    
    # 6 half-moves in the game
    assert len(X) == 6
    assert len(y) == 6
    assert X.shape == (6, 18, 8, 8)
    
    # Verify first move (e2e4)
    # e2 is square 12, e4 is square 28
    assert y[0][0] == 12 # from e2
    assert y[0][1] == 28 # to e4

def test_save_dataset(tmp_path):
    converter = PGNConverter()
    X = np.random.rand(10, 18, 8, 8).astype(np.float32)
    y = np.random.randint(0, 64, (10, 2))
    
    output_path = tmp_path / "dataset.npz"
    converter.save_dataset(X, y, str(output_path))
    
    assert os.path.exists(output_path)
    loaded = np.load(output_path)
    np.testing.assert_array_equal(loaded["X"], X)
    np.testing.assert_array_equal(loaded["y"], y)
