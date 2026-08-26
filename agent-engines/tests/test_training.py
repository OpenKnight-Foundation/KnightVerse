"""
Tests for LoRA Training Pipeline (training_pipeline.py).

Covers:
  - PGN parsing and move extraction
  - Move vocabulary building
  - Dataset preparation and padding
  - LoRA model forward pass and training
  - Adapter saving and loading
  - Full pipeline integration
"""

import os
import tempfile

import chess
import chess.pgn
import numpy as np
import pytest

from gpu_worker.training_pipeline import (
    LoRAModel,
    LoRATrainingPipeline,
    MoveVocabulary,
    PGNDatasetBuilder,
    TrainingConfig,
    TrainingResult,
)


# ------------------------------------------------------------------ #
# Sample PGN data                                                     #
# ------------------------------------------------------------------ #

SAMPLE_PGN = """\
[Event "Casual Game"]
[Site "Online"]
[Date "2024.01.15"]
[White "Alice"]
[Black "Bob"]
[Result "1-0"]

1. e4 e5 2. Nf3 Nc6 3. Bb5 a6 4. Ba4 Nf6 5. O-O 1-0

[Event "Casual Game 2"]
[Site "Online"]
[Date "2024.02.20"]
[White "Alice"]
[Black "Charlie"]
[Result "0-1"]

1. d4 d5 2. c4 e6 3. Nc3 Nf6 4. Bg5 Be7 0-1
"""


# ------------------------------------------------------------------ #
# MoveVocabulary                                                      #
# ------------------------------------------------------------------ #


class TestMoveVocabulary:
    def test_vocab_contains_special_tokens(self):
        vocab = MoveVocabulary()
        assert vocab.encode("<PAD>") == 0
        assert vocab.encode("<BOS>") == 1
        assert vocab.encode("<EOS>") == 2
        assert vocab.encode("<UNK>") == 3

    def test_vocab_contains_common_moves(self):
        vocab = MoveVocabulary()
        assert vocab.encode("e4") != vocab.encode("<UNK>")
        assert vocab.encode("Nf3") != vocab.encode("<UNK>")
        assert vocab.encode("O-O") != vocab.encode("<UNK>")

    def test_vocab_size_reasonable(self):
        vocab = MoveVocabulary()
        assert vocab.size > 1000
        assert vocab.size < 20000

    def test_encode_decode_roundtrip(self):
        vocab = MoveVocabulary()
        for san in ["e4", "Nf3", "Bb5", "O-O", "Qd1+"]:
            token_id = vocab.encode(san)
            assert vocab.decode(token_id) == san

    def test_unknown_token_returns_unk(self):
        vocab = MoveVocabulary()
        assert vocab.encode("ZZZZ") == vocab.encode("<UNK>")

    def test_encode_sequence(self):
        vocab = MoveVocabulary()
        seq = vocab.encode_sequence(["e4", "e5", "Nf3"])
        assert seq[0] == vocab.encode("<BOS>")
        assert seq[-1] == vocab.encode("<EOS>")
        assert len(seq) == 5  # BOS + 3 moves + EOS

    def test_decode_sequence(self):
        vocab = MoveVocabulary()
        original = ["e4", "e5", "Nf3"]
        token_ids = vocab.encode_sequence(original)
        decoded = vocab.decode_sequence(token_ids)
        assert decoded == original


# ------------------------------------------------------------------ #
# PGNDatasetBuilder                                                   #
# ------------------------------------------------------------------ #


class TestPGNDatasetBuilder:
    def test_parse_pgn(self):
        vocab = MoveVocabulary()
        builder = PGNDatasetBuilder(vocab)
        games = builder.parse_pgn(SAMPLE_PGN)
        assert len(games) == 2

    def test_extract_moves(self):
        vocab = MoveVocabulary()
        builder = PGNDatasetBuilder(vocab)
        games = builder.parse_pgn(SAMPLE_PGN)
        moves = builder.extract_moves(games[0])
        assert moves[0] == "e4"
        assert moves[1] == "e5"
        assert len(moves) == 9  # 1.e4 e5 2.Nf3 Nc6 3.Bb5 a6 4.Ba4 Nf6 5.O-O

    def test_build_dataset(self):
        vocab = MoveVocabulary()
        builder = PGNDatasetBuilder(vocab)
        sequences = builder.build_dataset(SAMPLE_PGN)
        assert len(sequences) == 2
        assert all(isinstance(seq, list) for seq in sequences)

    def test_pad_sequences(self):
        vocab = MoveVocabulary()
        builder = PGNDatasetBuilder(vocab)
        sequences = builder.build_dataset(SAMPLE_PGN)
        padded = builder.pad_sequences(sequences)
        assert padded.ndim == 2
        assert padded.shape[0] == 2
        assert padded.shape[1] > 0

    def test_build_dataset_empty_pgn(self):
        vocab = MoveVocabulary()
        builder = PGNDatasetBuilder(vocab)
        sequences = builder.build_dataset("")
        assert sequences == []

    def test_build_dataset_file_not_found(self):
        vocab = MoveVocabulary()
        builder = PGNDatasetBuilder(vocab)
        with pytest.raises(FileNotFoundError):
            builder.build_dataset_from_file("/nonexistent/path.pgn")


# ------------------------------------------------------------------ #
# LoRA Model                                                          #
# ------------------------------------------------------------------ #


class TestLoRAModel:
    def test_forward_shape(self):
        model = LoRAModel(vocab_size=100, embed_dim=32)
        token_ids = np.array([[1, 2, 3, 4]])
        logits = model.forward(token_ids)
        assert logits.shape == (1, 4, 100)

    def test_compute_loss(self):
        model = LoRAModel(vocab_size=100, embed_dim=32)
        token_ids = np.array([[1, 2, 3]])
        labels = np.array([[2, 3, 4]])
        loss = model.compute_loss(token_ids, labels)
        assert isinstance(loss, float)
        assert loss > 0

    def test_step_returns_loss(self):
        model = LoRAModel(vocab_size=100, embed_dim=32)
        token_ids = np.array([[1, 2, 3, 4]])
        labels = np.array([[2, 3, 4, 5]])
        loss = model.step(token_ids, labels, lr=0.01)
        assert isinstance(loss, float)
        assert loss > 0

    def test_training_reduces_loss(self):
        model = LoRAModel(vocab_size=50, embed_dim=16)
        token_ids = np.array([[1, 2, 3, 4, 5, 6, 7, 8]])
        labels = np.array([[2, 3, 4, 5, 6, 7, 8, 9]])

        initial_loss = model.compute_loss(token_ids, labels)
        for _ in range(20):
            model.step(token_ids, labels, lr=0.01)
        final_loss = model.compute_loss(token_ids, labels)

        assert final_loss < initial_loss

    def test_save_load_adapter(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            model = LoRAModel(vocab_size=100, embed_dim=32)
            adapter_path = os.path.join(tmpdir, "test_adapter.npz")
            size_mb = model.save_adapter(adapter_path)
            assert size_mb > 0
            assert os.path.exists(adapter_path)

            loaded = LoRAModel.load_adapter(adapter_path, vocab_size=100, embed_dim=32)
            np.testing.assert_array_equal(model.lora_A, loaded.lora_A)
            np.testing.assert_array_equal(model.lora_B, loaded.lora_B)

    def test_lora_state_dict(self):
        model = LoRAModel(vocab_size=100, embed_dim=32, lora_rank=4)
        state = model.get_lora_state_dict()
        assert "lora_A" in state
        assert "lora_B" in state
        assert state["lora_A"].shape == (32, 4)
        assert state["lora_B"].shape == (4, 32)


# ------------------------------------------------------------------ #
# Full Pipeline Integration                                           #
# ------------------------------------------------------------------ #


class TestTrainingPipeline:
    def test_train_produces_adapter(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            config = TrainingConfig(
                adapter_output_dir=tmpdir,
                epochs=1,
                batch_size=4,
                lora_rank=2,
            )
            pipeline = LoRATrainingPipeline(config)
            result = pipeline.train("test_player", SAMPLE_PGN)

            assert result.error is None
            assert result.games_processed == 2
            assert result.total_moves > 0
            assert result.training_loss > 0
            assert result.adapter_path is not None
            assert os.path.exists(result.adapter_path)
            assert result.adapter_size_mb > 0
            assert result.training_time_s > 0

    def test_adapter_size_under_limit(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            config = TrainingConfig(adapter_output_dir=tmpdir, lora_rank=4, epochs=1)
            pipeline = LoRATrainingPipeline(config)
            result = pipeline.train("test_player", SAMPLE_PGN)
            assert result.adapter_size_mb < 20  # < 20 MB

    def test_load_player_model(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            config = TrainingConfig(adapter_output_dir=tmpdir, lora_rank=4, epochs=1)
            pipeline = LoRATrainingPipeline(config)
            pipeline.train("alice", SAMPLE_PGN)

            model = pipeline.load_player_model("alice")
            assert model is not None
            assert isinstance(model, LoRAModel)

    def test_load_nonexistent_player(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            config = TrainingConfig(adapter_output_dir=tmpdir)
            pipeline = LoRATrainingPipeline(config)
            model = pipeline.load_player_model("nobody")
            assert model is None

    def test_train_empty_pgn(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            config = TrainingConfig(adapter_output_dir=tmpdir)
            pipeline = LoRATrainingPipeline(config)
            result = pipeline.train("test_player", "")
            assert result.error is not None
            assert "No valid games" in result.error

    def test_vocab_size_consistent(self):
        pipeline = LoRATrainingPipeline()
        assert pipeline.vocab.size > 0
        assert pipeline.vocab.size == len(pipeline.vocab.token_to_id)

    def test_training_result_defaults(self):
        result = TrainingResult(player_id="test")
        assert result.player_id == "test"
        assert result.adapter_path is None
        assert result.adapter_size_mb == 0.0
        assert result.training_loss == 0.0
        assert result.error is None


# ------------------------------------------------------------------ #
# TrainingConfig                                                      #
# ------------------------------------------------------------------ #


class TestTrainingConfig:
    def test_defaults(self):
        config = TrainingConfig()
        assert config.lora_rank == 8
        assert config.lora_alpha == 16
        assert config.epochs == 3
        assert config.batch_size == 8
        assert config.learning_rate == 5e-4
