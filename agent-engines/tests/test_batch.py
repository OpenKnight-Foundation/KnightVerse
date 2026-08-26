"""
Tests for FEN Tokenizer & Tensor Evaluation Cache (batch.py).

Covers:
  - FEN-to-tensor conversion accuracy against python-chess reference
  - Batch tensor conversion
  - LRU cache hit/miss/eviction behaviour
  - Position hash equivalence (same position different move counters)
  - BatchEvaluator deduplication and caching
  - Benchmark: throughput of tokenizer
"""

import time

import chess
import numpy as np
import pytest

from gpu_worker.batch import (
    BatchEvaluator,
    FENTokenizer,
    TensorEvaluationCache,
    fen_hash,
    position_hash,
)


# ------------------------------------------------------------------ #
# Fixtures                                                            #
# ------------------------------------------------------------------ #


@pytest.fixture
def tokenizer():
    return FENTokenizer()


@pytest.fixture
def start_board():
    return chess.Board()


# ------------------------------------------------------------------ #
# FEN-to-Tensor Accuracy                                              #
# ------------------------------------------------------------------ #


class TestFENTokenizer:
    def test_starting_position_tensor_shape(self, tokenizer):
        tensor = tokenizer.fen_to_tensor(chess.STARTING_FEN)
        assert tensor.shape == (8, 8, 14)

    def test_starting_position_white_pawns(self, tokenizer):
        tensor = tokenizer.fen_to_tensor(chess.STARTING_FEN)
        # White pawns on rank 1 (index 1, row 1)
        for f in range(8):
            assert tensor[1, f, 0] == 1.0  # pawn plane

    def test_starting_position_white_pieces(self, tokenizer):
        tensor = tokenizer.fen_to_tensor(chess.STARTING_FEN)
        # Rook a1 = (rank 0, file 0), plane 3 (rook)
        assert tensor[0, 0, 3] == 1.0
        # Knight b1 = (rank 0, file 1), plane 1 (knight)
        assert tensor[0, 1, 1] == 1.0
        # Bishop c1 = (rank 0, file 2), plane 2 (bishop)
        assert tensor[0, 2, 2] == 1.0
        # Queen d1 = (rank 0, file 3), plane 4 (queen)
        assert tensor[0, 3, 4] == 1.0
        # King e1 = (rank 0, file 4), plane 5 (king)
        assert tensor[0, 4, 5] == 1.0

    def test_starting_position_black_pieces(self, tokenizer):
        tensor = tokenizer.fen_to_tensor(chess.STARTING_FEN)
        # Black pieces on rank 7 (index 7)
        assert tensor[7, 0, 9] == 1.0   # rook a8
        assert tensor[7, 1, 7] == 1.0   # knight b8
        assert tensor[7, 4, 11] == 1.0  # king e8

    def test_starting_position_side_to_move(self, tokenizer):
        tensor = tokenizer.fen_to_tensor(chess.STARTING_FEN)
        # White to move → plane 13 all ones
        assert np.all(tensor[:, :, 13] == 1.0)

    def test_black_to_move(self, tokenizer):
        # After 1.e4, it's black to move
        board = chess.Board(chess.STARTING_FEN)
        board.push_san("e4")
        tensor = tokenizer.fen_to_tensor(board.fen())
        assert np.all(tensor[:, :, 13] == 0.0)

    def test_empty_board(self, tokenizer):
        fen = "8/8/8/8/8/8/8/8 w - - 0 1"
        tensor = tokenizer.fen_to_tensor(fen)
        # No piece planes should be active
        assert np.sum(tensor[:, :, :12]) == 0.0

    def test_en_passant_plane(self, tokenizer):
        # After 1.e4 a5 2.e5 f5, white can en-passant capture on f6
        board = chess.Board(chess.STARTING_FEN)
        board.push_san("e4")
        board.push_san("a5")
        board.push_san("e5")
        board.push_san("f5")
        tensor = tokenizer.fen_to_tensor(board.fen())
        # En passant square is f6 (rank 5, file 5)
        assert tensor[5, 5, 12] == 1.0

    def test_no_en_passant(self, tokenizer):
        tensor = tokenizer.fen_to_tensor(chess.STARTING_FEN)
        # No en passant at start
        assert np.sum(tensor[:, :, 12]) == 0.0

    def test_board_to_tensor_matches_fen(self, tokenizer):
        board = chess.Board(chess.STARTING_FEN)
        board.push_san("e4")
        board.push_san("e5")
        board.push_san("Nf3")
        fen_tensor = tokenizer.fen_to_tensor(board.fen())
        board_tensor = tokenizer.board_to_tensor(board)
        np.testing.assert_array_equal(fen_tensor, board_tensor)

    def test_tensor_is_binary(self, tokenizer):
        tensor = tokenizer.fen_to_tensor(chess.STARTING_FEN)
        # All values should be 0 or 1
        unique = np.unique(tensor)
        assert set(unique).issubset({0.0, 1.0})

    def test_castled_position(self, tokenizer):
        board = chess.Board(chess.STARTING_FEN)
        board.push_san("e4")
        board.push_san("e5")
        board.push_san("Nf3")
        board.push_san("Nc6")
        board.push_san("Bc4")
        board.push_san("Nf6")
        board.push_san("O-O")
        tensor = tokenizer.board_to_tensor(board)
        # King should be on g1 (rank 0, file 6), plane 5
        assert tensor[0, 6, 5] == 1.0
        # Rook should be on f1 (rank 0, file 5), plane 3
        assert tensor[0, 5, 3] == 1.0


# ------------------------------------------------------------------ #
# Batch Conversion                                                    #
# ------------------------------------------------------------------ #


class TestBatchConversion:
    def test_batch_shape(self, tokenizer):
        fens = [chess.STARTING_FEN, chess.STARTING_FEN]
        batch = tokenizer.batch_fen_to_tensors(fens)
        assert batch.shape == (2, 8, 8, 14)

    def test_batch_single(self, tokenizer):
        batch = tokenizer.batch_fen_to_tensors([chess.STARTING_FEN])
        assert batch.shape == (1, 8, 8, 14)

    def test_batch_empty(self, tokenizer):
        batch = tokenizer.batch_fen_to_tensors([])
        assert batch.shape == (0, 8, 8, 14)

    def test_batch_boards(self, tokenizer):
        boards = [chess.Board(chess.STARTING_FEN)]
        batch = tokenizer.batch_boards_to_tensor(boards)
        assert batch.shape == (1, 8, 8, 14)


# ------------------------------------------------------------------ #
# Position Hash                                                       #
# ------------------------------------------------------------------ #


class TestPositionHash:
    def test_same_position_same_hash(self):
        b1 = chess.Board(chess.STARTING_FEN)
        b2 = chess.Board(chess.STARTING_FEN)
        assert position_hash(b1) == position_hash(b2)

    def test_different_positions_different_hash(self):
        b1 = chess.Board(chess.STARTING_FEN)
        b2 = chess.Board(chess.STARTING_FEN)
        b2.push_san("e4")
        assert position_hash(b1) != position_hash(b2)

    def test_fen_hash_consistency(self):
        fen = chess.STARTING_FEN
        assert fen_hash(fen) == fen_hash(fen)

    def test_fen_hash_ignores_move_counters(self):
        # Same position, different halfmove clock
        fen1 = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1"
        fen2 = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 50 99"
        assert fen_hash(fen1) == fen_hash(fen2)


# ------------------------------------------------------------------ #
# LRU Cache                                                           #
# ------------------------------------------------------------------ #


class TestTensorCache:
    def test_put_and_get(self):
        cache = TensorEvaluationCache(maxsize=10)
        tensor = np.zeros((8, 8, 14), dtype=np.float32)
        cache.put("abc", tensor, evaluation=25, best_move="e4")
        entry = cache.get("abc")
        assert entry is not None
        assert entry.evaluation == 25
        assert entry.best_move == "e4"

    def test_cache_miss(self):
        cache = TensorEvaluationCache(maxsize=10)
        assert cache.get("nonexistent") is None

    def test_lru_eviction(self):
        cache = TensorEvaluationCache(maxsize=3)
        for i in range(5):
            cache.put(f"key_{i}", np.zeros((8, 8, 14)))
        # Keys 0 and 1 should be evicted
        assert cache.get("key_0") is None
        assert cache.get("key_1") is None
        assert cache.get("key_2") is not None
        assert cache.get("key_3") is not None
        assert cache.get("key_4") is not None

    def test_lru_promotes_on_get(self):
        cache = TensorEvaluationCache(maxsize=3)
        cache.put("a", np.zeros((8, 8, 14)))
        cache.put("b", np.zeros((8, 8, 14)))
        cache.put("c", np.zeros((8, 8, 14)))
        # Access "a" to promote it
        cache.get("a")
        # Now "b" is LRU — adding a 4th should evict "b"
        cache.put("d", np.zeros((8, 8, 14)))
        assert cache.get("a") is not None
        assert cache.get("b") is None
        assert cache.get("c") is not None
        assert cache.get("d") is not None

    def test_hit_rate(self):
        cache = TensorEvaluationCache(maxsize=10)
        tensor = np.zeros((8, 8, 14))
        cache.put("x", tensor)
        cache.get("x")  # hit
        cache.get("y")  # miss
        cache.get("x")  # hit
        assert cache.hit_rate == pytest.approx(2 / 3)

    def test_contains(self):
        cache = TensorEvaluationCache(maxsize=10)
        cache.put("k", np.zeros((8, 8, 14)))
        assert cache.contains("k") is True
        assert cache.contains("z") is False

    def test_remove(self):
        cache = TensorEvaluationCache(maxsize=10)
        cache.put("k", np.zeros((8, 8, 14)))
        assert cache.remove("k") is True
        assert cache.get("k") is None
        assert cache.remove("k") is False

    def test_clear(self):
        cache = TensorEvaluationCache(maxsize=10)
        cache.put("a", np.zeros((8, 8, 14)))
        cache.get("a")
        cache.clear()
        assert cache.size == 0
        assert cache.hit_rate == 0.0

    def test_stats(self):
        cache = TensorEvaluationCache(maxsize=5)
        cache.put("a", np.zeros((8, 8, 14)))
        cache.get("a")
        stats = cache.stats
        assert stats["size"] == 1
        assert stats["maxsize"] == 5
        assert stats["hits"] == 1

    def test_update_existing_key(self):
        cache = TensorEvaluationCache(maxsize=5)
        tensor = np.zeros((8, 8, 14))
        cache.put("k", tensor, evaluation=10)
        cache.put("k", tensor, evaluation=20)
        entry = cache.get("k")
        assert entry.evaluation == 20


# ------------------------------------------------------------------ #
# BatchEvaluator                                                      #
# ------------------------------------------------------------------ #


class TestBatchEvaluator:
    def test_deduplication(self):
        evaluator = BatchEvaluator()
        fens = [chess.STARTING_FEN, chess.STARTING_FEN]
        results = evaluator.evaluate_batch(fens)
        # Second should be cached
        assert results[0]["cached"] is False
        assert results[1]["cached"] is True

    def test_different_positions(self):
        evaluator = BatchEvaluator()
        b1 = chess.Board(chess.STARTING_FEN)
        b2 = chess.Board(chess.STARTING_FEN)
        b2.push_san("e4")
        results = evaluator.evaluate_batch([b1.fen(), b2.fen()])
        assert results[0]["cached"] is False
        assert results[1]["cached"] is False

    def test_eval_fn_called(self):
        evaluator = BatchEvaluator()

        def dummy_eval(board):
            return {"evaluation": 42, "best_move": "e4"}

        results = evaluator.evaluate_batch([chess.STARTING_FEN], eval_fn=dummy_eval)
        assert results[0]["evaluation"] == 42
        assert results[0]["best_move"] == "e4"

    def test_eval_fn_simple_return(self):
        evaluator = BatchEvaluator()

        def simple_eval(board):
            return 100

        results = evaluator.evaluate_batch([chess.STARTING_FEN], eval_fn=simple_eval)
        assert results[0]["evaluation"] == 100

    def test_get_tensors(self):
        evaluator = BatchEvaluator()
        fens = [chess.STARTING_FEN, chess.STARTING_FEN]
        batch = evaluator.get_tensors(fens)
        assert batch.shape == (2, 8, 8, 14)
        # Both tensors should be equal (same position)
        np.testing.assert_array_equal(batch[0], batch[1])

    def test_cache_hit_rate_after_repeated(self):
        evaluator = BatchEvaluator()
        fen = chess.STARTING_FEN
        for _ in range(5):
            evaluator.evaluate_batch([fen])
        assert evaluator.cache.hit_rate > 0.4


# ------------------------------------------------------------------ #
# Benchmark                                                           #
# ------------------------------------------------------------------ #


class TestBenchmark:
    def test_throughput_100k(self, tokenizer):
        """Verify tokenizer can handle >100k FENs/second."""
        board = chess.Board(chess.STARTING_FEN)
        fens = []
        # Generate a variety of positions
        moves = ["e4", "e5", "Nf3", "Nc6", "Bb5", "a6"]
        for move in moves:
            board.push_san(move)
            fens.append(board.fen())

        # Repeat to build up volume
        all_fens = fens * 5000  # 30,000 FENs

        t0 = time.perf_counter()
        for fen in all_fens:
            tokenizer.fen_to_tensor(fen)
        elapsed = time.perf_counter() - t0

        throughput = len(all_fens) / elapsed
        assert throughput > 100_000, (
            f"Throughput {throughput:.0f} FENs/s is below 100k threshold"
        )
