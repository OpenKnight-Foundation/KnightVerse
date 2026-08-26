"""Tests for the KnightVerse Evaluation Head training pipeline.

Tests dataset loading, model architecture, forward/backward pass,
and loss computation without requiring GPU or Lightning.
"""

from __future__ import annotations

import math
import numpy as np
import pytest
import torch

from gpu_worker.train_valuation_head import (
    TrainingConfig,
    encode_board_from_fen,
    ChessPositionDataset,
    KnightVerseHead,
    ResidualBlock,
)


STARTING_FEN = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"


# ===================================================================
# SECTION 1: Board Encoding
# ===================================================================

class TestBoardEncoding:
    """Test FEN to tensor encoding."""

    def test_encode_shape(self) -> None:
        board = encode_board_from_fen(STARTING_FEN)
        assert board.shape == (18, 8, 8)

    def test_encode_custom_planes(self) -> None:
        board = encode_board_from_fen(STARTING_FEN, num_planes=14)
        assert board.shape == (14, 8, 8)

    def test_white_pawns_present(self) -> None:
        board = encode_board_from_fen(STARTING_FEN)
        # White pawns on rank 2 (row 6)
        assert np.sum(board[0, 6, :]) == 8.0  # 8 white pawns

    def test_black_pawns_present(self) -> None:
        board = encode_board_from_fen(STARTING_FEN)
        # Black pawns on rank 7 (row 1)
        assert np.sum(board[6, 1, :]) == 8.0  # 8 black pawns

    def test_white_king_present(self) -> None:
        board = encode_board_from_fen(STARTING_FEN)
        assert board[5, 7, 4] == 1.0  # White king on e1

    def test_black_king_present(self) -> None:
        board = encode_board_from_fen(STARTING_FEN)
        assert board[11, 0, 4] == 1.0  # Black king on e8

    def test_castling_rights(self) -> None:
        board = encode_board_from_fen(STARTING_FEN)
        assert np.all(board[12, :, :] == 1.0)  # White K castling
        assert np.all(board[13, :, :] == 1.0)  # White Q castling
        assert np.all(board[14, :, :] == 1.0)  # Black K castling
        assert np.all(board[15, :, :] == 1.0)  # Black Q castling

    def test_no_castling_rights(self) -> None:
        fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w - - 0 1"
        board = encode_board_from_fen(fen)
        assert np.sum(board[12, :, :]) == 0.0
        assert np.sum(board[13, :, :]) == 0.0

    def test_side_to_move_white(self) -> None:
        board = encode_board_from_fen(STARTING_FEN)
        assert np.all(board[17, :, :] == 1.0)

    def test_side_to_move_black(self) -> None:
        fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR b KQkq - 0 1"
        board = encode_board_from_fen(fen)
        assert np.all(board[17, :, :] == 0.0)

    def test_en_passant_file(self) -> None:
        fen = "rnbqkbnr/pp1p1ppp/8/2pPp3/8/8/PPP1PPPP/RNBQKBNR w KQkq c6 0 3"
        board = encode_board_from_fen(fen)
        # En passant on c-file (index 2)
        assert board[16, 0, 2] == 1.0
        assert np.sum(board[16, :, :]) == 8.0  # 8 squares on c-file

    def test_dtype_is_float32(self) -> None:
        board = encode_board_from_fen(STARTING_FEN)
        assert board.dtype == np.float32


# ===================================================================
# SECTION 2: Dataset
# ===================================================================

class TestDataset:
    """Test ChessPositionDataset."""

    def test_synthetic_dataset_length(self) -> None:
        dataset = ChessPositionDataset.from_synthetic(n=100)
        assert len(dataset) == 100

    def test_synthetic_dataset_getitem(self) -> None:
        dataset = ChessPositionDataset.from_synthetic(n=10)
        board, outcome, policy = dataset[0]
        assert isinstance(board, torch.Tensor)
        assert board.shape == (18, 8, 8)
        assert outcome.shape == (1,)
        assert -1.0 <= outcome.item() <= 1.0

    def test_custom_dataset(self) -> None:
        positions = [STARTING_FEN, STARTING_FEN]
        outcomes = [0.5, -0.3]
        dataset = ChessPositionDataset(positions, outcomes)
        assert len(dataset) == 2
        board, outcome, _ = dataset[0]
        assert outcome.item() == pytest.approx(0.5)


# ===================================================================
# SECTION 3: Model Architecture
# ===================================================================

class TestModelArchitecture:
    """Test KnightVerseHead model."""

    def test_model_creation(self) -> None:
        config = TrainingConfig(hidden_channels=64, num_residual_blocks=2)
        model = KnightVerseHead(config)
        assert model is not None

    def test_model_forward_pass(self) -> None:
        config = TrainingConfig(hidden_channels=64, num_residual_blocks=2)
        model = KnightVerseHead(config)
        model.eval()

        x = torch.randn(2, 18, 8, 8)
        value, policy = model(x)

        assert value.shape == (2, 1)
        assert policy.shape[0] == 2

    def test_value_output_range(self) -> None:
        config = TrainingConfig(hidden_channels=64, num_residual_blocks=2)
        model = KnightVerseHead(config)
        model.eval()

        x = torch.randn(4, 18, 8, 8)
        value, _ = model(x)

        # Value should be in [-1, 1] due to tanh
        assert value.min() >= -1.0
        assert value.max() <= 1.0

    def test_model_parameters_exist(self) -> None:
        config = TrainingConfig(hidden_channels=64, num_residual_blocks=2)
        model = KnightVerseHead(config)
        params = list(model.parameters())
        assert len(params) > 0

    def test_model_gradient_flow(self) -> None:
        config = TrainingConfig(hidden_channels=32, num_residual_blocks=1)
        model = KnightVerseHead(config)
        model.train()

        x = torch.randn(2, 18, 8, 8)
        value, policy = model(x)

        loss = value.sum() + policy.sum()
        loss.backward()

        for name, param in model.named_parameters():
            if param.requires_grad:
                assert param.grad is not None, f"No gradient for {name}"

    def test_residual_block(self) -> None:
        block = ResidualBlock(64)
        x = torch.randn(2, 64, 8, 8)
        out = block(x)
        assert out.shape == x.shape


# ===================================================================
# SECTION 4: Loss Computation
# ===================================================================

class TestLossComputation:
    """Test loss computation for value and policy heads."""

    def test_value_loss_computation(self) -> None:
        config = TrainingConfig(hidden_channels=32, num_residual_blocks=1)
        model = KnightVerseHead(config)
        model.train()

        x = torch.randn(4, 18, 8, 8)
        value, _ = model(x)

        target = torch.tensor([[0.5], [-0.3], [1.0], [0.0]])
        loss = torch.nn.MSELoss()(value.squeeze(-1), target)
        assert loss.item() > 0

    def test_policy_loss_computation(self) -> None:
        config = TrainingConfig(hidden_channels=32, num_residual_blocks=1)
        model = KnightVerseHead(config)
        model.train()

        x = torch.randn(4, 18, 8, 8)
        _, policy = model(x)

        target = torch.randint(0, policy.shape[-1], (4,))
        loss = torch.nn.CrossEntropyLoss()(policy, target)
        assert loss.item() > 0

    def test_combined_loss_backward(self) -> None:
        config = TrainingConfig(hidden_channels=32, num_residual_blocks=1)
        model = KnightVerseHead(config)
        model.train()

        x = torch.randn(2, 18, 8, 8)
        value, policy = model(x)

        value_target = torch.tensor([[0.5], [-0.5]])
        policy_target = torch.randint(0, policy.shape[-1], (2,))

        value_loss = torch.nn.MSELoss()(value.squeeze(-1), value_target)
        policy_loss = torch.nn.CrossEntropyLoss()(policy, policy_target)
        total_loss = value_loss + 0.5 * policy_loss

        total_loss.backward()
        assert total_loss.item() > 0


# ===================================================================
# SECTION 5: Training Config
# ===================================================================

class TestTrainingConfig:
    """Test TrainingConfig defaults."""

    def test_default_config(self) -> None:
        config = TrainingConfig()
        assert config.input_planes == 18
        assert config.hidden_channels == 128
        assert config.num_residual_blocks == 6
        assert config.precision == "16-mixed"
        assert config.max_epochs == 100

    def test_config_custom(self) -> None:
        config = TrainingConfig(hidden_channels=256, learning_rate=0.0005)
        assert config.hidden_channels == 256
        assert config.learning_rate == 0.0005
