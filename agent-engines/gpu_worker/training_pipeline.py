"""
Automated LoRA Fine-Tuning Pipeline for Player-Customized AI Personalities.

Ingests player PGN game history, tokenizes chess moves, fine-tunes a
lightweight policy model via LoRA/PEFT, and saves compact player-specific
adapter weights (< 20 MB) for dynamic loading at inference time.
"""

from __future__ import annotations

import io
import json
import logging
import os
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Optional

import chess
import chess.pgn
import numpy as np

logger = logging.getLogger(__name__)

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

# All unique chess tokens: piece-letter + square combinations + special tokens
SPECIAL_TOKENS = ["<PAD>", "<BOS>", "<EOS>", "<UNK>"]
PIECE_CHARS = ["P", "N", "B", "R", "Q", "K", "p", "n", "b", "r", "q", "k"]
SQUARES = [f"{f}{r}" for f in "abcdefgh" for r in "12345678"]

# Build move vocabulary from all legal SAN moves across all positions
DEFAULT_VOCAB_SIZE = 1968  # approximate number of unique SAN moves

MAX_SEQUENCE_LENGTH = 512
MAX_ADAPTER_SIZE_MB = 20
DEFAULT_LORA_RANK = 8
DEFAULT_LORA_ALPHA = 16
DEFAULT_EPOCHS = 3
DEFAULT_BATCH_SIZE = 8
DEFAULT_LR = 5e-4


# ---------------------------------------------------------------------------
# Data classes
# ---------------------------------------------------------------------------

@dataclass
class TrainingConfig:
    """Configuration for LoRA fine-tuning."""
    lora_rank: int = DEFAULT_LORA_RANK
    lora_alpha: int = DEFAULT_LORA_ALPHA
    lora_dropout: float = 0.1
    epochs: int = DEFAULT_EPOCHS
    batch_size: int = DEFAULT_BATCH_SIZE
    learning_rate: float = DEFAULT_LR
    max_sequence_length: int = MAX_SEQUENCE_LENGTH
    target_modules: list[str] = field(default_factory=lambda: ["q_proj", "v_proj"])
    adapter_output_dir: str = "./adapters"


@dataclass
class TrainingResult:
    """Outcome of a fine-tuning run."""
    player_id: str
    adapter_path: Optional[str] = None
    adapter_size_mb: float = 0.0
    training_loss: float = 0.0
    games_processed: int = 0
    total_moves: int = 0
    training_time_s: float = 0.0
    error: Optional[str] = None


# ---------------------------------------------------------------------------
# Chess Move Vocabulary
# ---------------------------------------------------------------------------

class MoveVocabulary:
    """Maps chess SAN moves to integer token IDs."""

    def __init__(self) -> None:
        self.token_to_id: dict[str, int] = {}
        self.id_to_token: dict[int, str] = {}
        self._build_vocab()

    def _build_vocab(self) -> None:
        """Build vocabulary from special tokens + all possible SAN moves."""
        idx = 0
        for token in SPECIAL_TOKENS:
            self.token_to_id[token] = idx
            self.id_to_token[idx] = token
            idx += 1

        # Generate all plausible SAN strings
        san_patterns = self._generate_all_sans()
        for san in sorted(san_patterns):
            if san not in self.token_to_id:
                self.token_to_id[san] = idx
                self.id_to_token[idx] = san
                idx += 1

    @staticmethod
    def _generate_all_sans() -> set[str]:
        """Generate all possible SAN strings for vocabulary."""
        sans: set[str] = set()
        files = "abcdefgh"
        ranks = "12345678"

        # Pawn moves: e4, e5, exd5, e8=Q, etc.
        for f in files:
            for r in ranks:
                sans.add(f"{f}{r}")
                sans.add(f"{f}x{f}{r}")
                sans.add(f"{f}{r}=Q")
                sans.add(f"{f}{r}=R")
                sans.add(f"{f}{r}=B")
                sans.add(f"{f}{r}=N")
                for rf in files:
                    if rf != f:
                        sans.add(f"{rf}x{f}{r}")

        # Piece moves: Nf3, Bxe5, Qd1+, etc.
        for piece in "KQRBN":
            for f in files:
                for r in ranks:
                    sans.add(f"{piece}{f}{r}")
                    sans.add(f"{piece}x{f}{r}")
                    sans.add(f"{piece}{f}{r}+")
                    sans.add(f"{piece}{f}{r}#")
                    sans.add(f"{piece}x{f}{r}+")
                    sans.add(f"{piece}x{f}{r}#")
                    for df in files:
                        sans.add(f"{piece}{df}{f}{r}")
                        sans.add(f"{piece}{df}x{f}{r}")
                    for dr in ranks:
                        sans.add(f"{piece}{dr}{f}{r}")
                        sans.add(f"{piece}{dr}x{f}{r}")

        # Castling
        sans.update({"O-O", "O-O+"})

        return sans

    @property
    def size(self) -> int:
        return len(self.token_to_id)

    def encode(self, san: str) -> int:
        return self.token_to_id.get(san, self.token_to_id["<UNK>"])

    def decode(self, token_id: int) -> str:
        return self.id_to_token.get(token_id, "<UNK>")

    def encode_sequence(self, sans: list[str]) -> list[int]:
        tokens = [self.token_to_id["<BOS>"]]
        tokens.extend(self.encode(san) for san in sans)
        tokens.append(self.token_to_id["<EOS>"])
        return tokens

    def decode_sequence(self, token_ids: list[int]) -> list[str]:
        result = []
        for tid in token_ids:
            tok = self.decode(tid)
            if tok in ("<PAD>", "<BOS>", "<EOS>"):
                continue
            result.append(tok)
        return result


# ---------------------------------------------------------------------------
# PGN Dataset Builder
# ---------------------------------------------------------------------------

class PGNDatasetBuilder:
    """Ingests PGN text and builds tokenized training sequences."""

    def __init__(
        self,
        vocab: MoveVocabulary,
        max_length: int = MAX_SEQUENCE_LENGTH,
    ) -> None:
        self.vocab = vocab
        self.max_length = max_length

    def parse_pgn(self, pgn_text: str) -> list[chess.pgn.Game]:
        """Parse all games from a PGN string."""
        games = []
        stream = io.StringIO(pgn_text)
        while True:
            game = chess.pgn.read_game(stream)
            if game is None:
                break
            games.append(game)
        return games

    def extract_moves(self, game: chess.pgn.Game) -> list[str]:
        """Extract SAN move list from a game."""
        moves = []
        board = game.board()
        for move in game.mainline_moves():
            moves.append(board.san(move))
            board.push(move)
        return moves

    def build_dataset(
        self,
        pgn_text: str,
    ) -> list[list[int]]:
        """Parse PGN and return list of tokenized move sequences."""
        games = self.parse_pgn(pgn_text)
        sequences = []
        for game in games:
            moves = self.extract_moves(game)
            if not moves:
                continue
            token_ids = self.vocab.encode_sequence(moves)
            # Truncate to max length
            if len(token_ids) > self.max_length:
                token_ids = token_ids[: self.max_length]
            sequences.append(token_ids)
        return sequences

    def build_dataset_from_file(self, pgn_path: str) -> list[list[int]]:
        """Read a PGN file and build dataset."""
        path = Path(pgn_path)
        if not path.exists():
            raise FileNotFoundError(f"PGN file not found: {pgn_path}")
        pgn_text = path.read_text(encoding="utf-8")
        return self.build_dataset(pgn_text)

    def pad_sequences(
        self,
        sequences: list[list[int]],
        pad_id: int = 0,
    ) -> np.ndarray:
        """Pad sequences to uniform length for batching."""
        max_len = max(len(s) for s in sequences) if sequences else 0
        max_len = min(max_len, self.max_length)
        padded = np.full((len(sequences), max_len), pad_id, dtype=np.int64)
        for i, seq in enumerate(sequences):
            padded[i, : len(seq)] = seq[:max_len]
        return padded


# ---------------------------------------------------------------------------
# LoRA Model (lightweight, no heavy deps at import time)
# ---------------------------------------------------------------------------

class LoRAModel:
    """
    Minimal LoRA wrapper around a small policy network.

    In production this would use HuggingFace PEFT + a transformer model.
    Here we provide a self-contained implementation for the training loop
    that can run without GPU or large model downloads.
    """

    def __init__(
        self,
        vocab_size: int,
        embed_dim: int = 64,
        lora_rank: int = DEFAULT_LORA_RANK,
        lora_alpha: int = DEFAULT_LORA_ALPHA,
        lora_dropout: float = 0.1,
    ) -> None:
        self.vocab_size = vocab_size
        self.embed_dim = embed_dim
        self.lora_rank = lora_rank
        self.lora_alpha = lora_alpha
        self.lora_dropout = lora_dropout

        # Base embeddings
        rng = np.random.RandomState(42)
        self.embeddings = rng.randn(vocab_size, embed_dim).astype(np.float32) * 0.02

        # LoRA A and B matrices for low-rank adaptation
        self.lora_A = rng.randn(embed_dim, lora_rank).astype(np.float32) * 0.02
        self.lora_B = np.zeros((lora_rank, embed_dim), dtype=np.float32)

        # Output projection
        self.output_weight = rng.randn(embed_dim, vocab_size).astype(np.float32) * 0.02
        self.output_bias = np.zeros(vocab_size, dtype=np.float32)

        # Gradient storage
        self._grads: dict[str, np.ndarray] = {}

    def forward(self, token_ids: np.ndarray) -> np.ndarray:
        """Forward pass: token IDs -> logits over vocabulary."""
        batch_size, seq_len = token_ids.shape
        # Embed
        x = self.embeddings[token_ids]  # (B, S, D)
        # LoRA adaptation
        lora_out = x @ self.lora_A @ self.lora_B  # (B, S, D)
        x = x + lora_out
        # Output logits
        logits = x @ self.output_weight + self.output_bias  # (B, S, V)
        return logits

    def compute_loss(
        self,
        token_ids: np.ndarray,
        labels: np.ndarray,
    ) -> float:
        """Compute cross-entropy loss."""
        logits = self.forward(token_ids)
        B, S, V = logits.shape
        # Flatten for cross-entropy
        flat_logits = logits.reshape(-1, V)
        flat_labels = labels.reshape(-1)
        # Softmax + log (numerically stable)
        max_logits = np.max(flat_logits, axis=1, keepdims=True)
        exp_logits = np.exp(flat_logits - max_logits)
        probs = exp_logits / np.sum(exp_logits, axis=1, keepdims=True)
        log_probs = np.log(probs + 1e-12)
        # Cross-entropy
        nll = -log_probs[np.arange(len(flat_labels)), flat_labels]
        return float(np.mean(nll))

    def step(
        self,
        token_ids: np.ndarray,
        labels: np.ndarray,
        lr: float = DEFAULT_LR,
    ) -> float:
        """Single training step with gradient computation and parameter update."""
        loss = self.compute_loss(token_ids, labels)
        # Simplified gradient update (approximation for demo)
        # In production, use autograd via PyTorch
        logits = self.forward(token_ids)
        B, S, V = logits.shape

        probs = np.exp(logits - np.max(logits, axis=-1, keepdims=True))
        probs = probs / np.sum(probs, axis=-1, keepdims=True)

        # Gradient w.r.t. logits
        grad_logits = probs.copy()
        grad_logits[np.arange(B)[:, None], np.arange(S), labels] -= 1.0
        grad_logits /= (B * S)

        # Update output layer
        x = self.embeddings[token_ids]
        lora_out = x @ self.lora_A @ self.lora_B
        h = x + lora_out

        self.output_weight -= lr * (h.reshape(-1, self.embed_dim).T @ grad_logits.reshape(-1, V))
        self.output_bias -= lr * np.mean(grad_logits.reshape(-1, V), axis=0)

        return loss

    def get_lora_state_dict(self) -> dict[str, np.ndarray]:
        """Return LoRA parameters as a state dict."""
        return {
            "lora_A": self.lora_A.copy(),
            "lora_B": self.lora_B.copy(),
            "embeddings": self.embeddings.copy(),
        }

    def load_lora_state_dict(self, state: dict[str, np.ndarray]) -> None:
        """Load LoRA parameters from a state dict."""
        self.lora_A = state["lora_A"].copy()
        self.lora_B = state["lora_B"].copy()
        if "embeddings" in state:
            self.embeddings = state["embeddings"].copy()

    def save_adapter(self, path: str) -> float:
        """Save adapter weights to disk. Returns size in MB."""
        state = self.get_lora_state_dict()
        os.makedirs(os.path.dirname(path) if os.path.dirname(path) else ".", exist_ok=True)
        np.savez_compressed(path, **state)
        size_mb = os.path.getsize(path) / (1024 * 1024)
        return size_mb

    @classmethod
    def load_adapter(cls, path: str, vocab_size: int, **kwargs) -> "LoRAModel":
        """Load a saved adapter from disk."""
        data = np.load(path, allow_pickle=False)
        state = {k: data[k] for k in data.files}
        model = cls(vocab_size=vocab_size, **kwargs)
        model.load_lora_state_dict(state)
        return model


# ---------------------------------------------------------------------------
# Training Pipeline
# ---------------------------------------------------------------------------

class LoRATrainingPipeline:
    """
    End-to-end pipeline: PGN -> tokenize -> fine-tune -> save adapter.

    Usage::

        pipeline = LoRATrainingPipeline()
        result = pipeline.train(player_id="alice", pgn_text=pgn_data)
    """

    def __init__(self, config: Optional[TrainingConfig] = None) -> None:
        self.config = config or TrainingConfig()
        self.vocab = MoveVocabulary()
        self.dataset_builder = PGNDatasetBuilder(
            self.vocab, max_length=self.config.max_sequence_length
        )

    def train(
        self,
        player_id: str,
        pgn_text: str,
        *,
        progress_callback: Any = None,
    ) -> TrainingResult:
        """Run the full training pipeline for a player."""
        t0 = time.perf_counter()
        result = TrainingResult(player_id=player_id)

        try:
            # 1. Build dataset
            sequences = self.dataset_builder.build_dataset(pgn_text)
            if not sequences:
                result.error = "No valid games found in PGN data"
                return result

            result.games_processed = len(sequences)
            result.total_moves = sum(len(s) - 2 for s in sequences)  # exclude BOS/EOS

            # 2. Pad sequences
            pad_id = self.vocab.token_to_id["<PAD>"]
            padded = self.dataset_builder.pad_sequences(sequences, pad_id=pad_id)

            # 3. Create model
            model = LoRAModel(
                vocab_size=self.vocab.size,
                lora_rank=self.config.lora_rank,
                lora_alpha=self.config.lora_alpha,
                lora_dropout=self.config.lora_dropout,
            )

            # 4. Training loop
            total_loss = 0.0
            n_batches = 0

            for epoch in range(self.config.epochs):
                # Shuffle data
                indices = np.random.permutation(len(padded))
                epoch_loss = 0.0
                epoch_batches = 0

                for start in range(0, len(padded), self.config.batch_size):
                    batch_idx = indices[start : start + self.config.batch_size]
                    batch = padded[batch_idx]

                    # Input = all tokens except last, labels = all tokens except first
                    input_ids = batch[:, :-1]
                    labels = batch[:, 1:]

                    loss = model.step(
                        input_ids, labels, lr=self.config.learning_rate
                    )
                    epoch_loss += loss
                    epoch_batches += 1

                if epoch_batches > 0:
                    avg_loss = epoch_loss / epoch_batches
                    total_loss += avg_loss
                    n_batches += 1
                    logger.info(
                        "Epoch %d/%d — loss: %.4f",
                        epoch + 1,
                        self.config.epochs,
                        avg_loss,
                    )

            result.training_loss = total_loss / max(n_batches, 1)

            # 5. Save adapter
            adapter_path = os.path.join(
                self.config.adapter_output_dir, f"{player_id}_adapter.npz"
            )
            size_mb = model.save_adapter(adapter_path)
            result.adapter_path = adapter_path
            result.adapter_size_mb = size_mb

            if size_mb > MAX_ADAPTER_SIZE_MB:
                logger.warning(
                    "Adapter size %.2f MB exceeds %d MB limit",
                    size_mb,
                    MAX_ADAPTER_SIZE_MB,
                )

        except Exception as exc:
            result.error = str(exc)
            logger.exception("Training failed for player %s", player_id)

        result.training_time_s = time.perf_counter() - t0
        return result

    def load_player_model(self, player_id: str) -> Optional[LoRAModel]:
        """Load a player's saved adapter for inference."""
        adapter_path = os.path.join(
            self.config.adapter_output_dir, f"{player_id}_adapter.npz"
        )
        if not os.path.exists(adapter_path):
            logger.warning("No adapter found for player %s at %s", player_id, adapter_path)
            return None

        try:
            model = LoRAModel.load_adapter(
                adapter_path,
                vocab_size=self.vocab.size,
                lora_rank=self.config.lora_rank,
                lora_alpha=self.config.lora_alpha,
            )
            return model
        except Exception as exc:
            logger.exception("Failed to load adapter for %s", player_id)
            return None
