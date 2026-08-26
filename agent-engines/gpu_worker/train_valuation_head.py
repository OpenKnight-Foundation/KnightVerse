"""PyTorch Lightning Training Pipeline for KnightVerse Evaluation Head.

Implements a dual-headed neural network (Value Head + Policy Head) for
positional valuation designed for Human-AI co-pilot synergy. Supports
distributed data parallelism, mixed precision, and WandB tracking.

Usage:
    python -m gpu_worker.train_valuation_head --data_dir /path/to/pgn
"""

from __future__ import annotations

import argparse
import logging
import math
import os
from dataclasses import dataclass, field
from typing import Any, Dict, List, Optional, Tuple

import numpy as np
import torch
import torch.nn as nn
import torch.nn.functional as F
from torch.utils.data import Dataset, DataLoader, random_split

logger = logging.getLogger("KnightVerse.TrainingPipeline")


# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------

@dataclass
class TrainingConfig:
    """Configuration for the training pipeline."""

    # Model
    input_planes: int = 18  # piece planes (12) + castling + en passant + etc.
    board_size: int = 8
    hidden_channels: int = 128
    num_residual_blocks: int = 6
    value_head_hidden: int = 256
    policy_head_channels: int = 32

    # Training
    learning_rate: float = 0.001
    weight_decay: float = 1e-4
    batch_size: int = 256
    max_epochs: int = 100
    precision: str = "16-mixed"  # "16-mixed" for AMP, "32" for full precision
    num_workers: int = 4
    val_split: float = 0.1

    # DDP
    devices: int = -1  # -1 = auto-detect
    strategy: str = "auto"  # "auto" or "ddp"

    # WandB
    wandb_project: str = "knightverse"
    wandb_experiment: str = "valuation_head_v1"
    log_every_n_steps: int = 50

    # Checkpointing
    checkpoint_dir: str = "checkpoints"
    save_top_k: int = 3
    monitor_metric: str = "val_loss"


# ---------------------------------------------------------------------------
# Chess Board Encoding
# ---------------------------------------------------------------------------

# Piece type to plane index mapping
PIECE_TO_PLANE = {
    "P": 0, "N": 1, "B": 2, "R": 3, "Q": 4, "K": 5,
    "p": 6, "n": 7, "b": 8, "r": 9, "q": 10, "k": 11,
}


def encode_board_from_fen(fen: str, num_planes: int = 18) -> np.ndarray:
    """Encode a FEN position into a multi-plane tensor.

    Planes 0-11: Piece planes (one per piece type)
    Plane 12: White kingside castling
    Plane 13: White queenside castling
    Plane 14: Black kingside castling
    Plane 15: Black queenside castling
    Plane 16: En passant file (if any)
    Plane 17: Side to move (1.0 = white, 0.0 = black)

    Args:
        fen: FEN string of the position.
        num_planes: Number of feature planes.

    Returns:
        numpy array of shape (num_planes, 8, 8).
    """
    board = np.zeros((num_planes, 8, 8), dtype=np.float32)
    parts = fen.split()

    # Piece placement
    placement = parts[0]
    row, col = 0, 0
    for ch in placement:
        if ch == "/":
            row += 1
            col = 0
        elif ch.isdigit():
            col += int(ch)
        elif ch in PIECE_TO_PLANE:
            plane = PIECE_TO_PLANE[ch]
            board[plane, row, col] = 1.0
            col += 1

    # Castling rights
    castling = parts[2] if len(parts) > 2 else "-"
    if "K" in castling:
        board[12, :, :] = 1.0
    if "Q" in castling:
        board[13, :, :] = 1.0
    if "k" in castling:
        board[14, :, :] = 1.0
    if "q" in castling:
        board[15, :, :] = 1.0

    # En passant
    if len(parts) > 3 and parts[3] != "-":
        ep_file = ord(parts[3][0]) - ord("a")
        board[16, :, ep_file] = 1.0

    # Side to move
    if len(parts) > 1 and parts[1] == "w":
        board[17, :, :] = 1.0

    return board


# ---------------------------------------------------------------------------
# Neural Network Architecture
# ---------------------------------------------------------------------------

class ResidualBlock(nn.Module):
    """Residual block with skip connection."""

    def __init__(self, channels: int) -> None:
        super().__init__()
        self.conv1 = nn.Conv2d(channels, channels, 3, padding=1, bias=False)
        self.bn1 = nn.BatchNorm2d(channels)
        self.conv2 = nn.Conv2d(channels, channels, 3, padding=1, bias=False)
        self.bn2 = nn.BatchNorm2d(channels)

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        residual = x
        out = F.relu(self.bn1(self.conv1(x)))
        out = self.bn2(self.conv2(out))
        return F.relu(out + residual)


class KnightVerseHead(nn.Module):
    """Dual-headed neural network: Value Head + Policy Head.

    Architecture:
        Input -> Conv -> N x ResBlock -> [Value Head, Policy Head]

    Value Head: Predicts scalar game outcome (win/draw/loss)
    Policy Head: Predicts move probability distribution
    """

    def __init__(self, config: TrainingConfig) -> None:
        super().__init__()
        self.config = config

        # Initial convolution
        self.input_conv = nn.Sequential(
            nn.Conv2d(config.input_planes, config.hidden_channels, 3, padding=1, bias=False),
            nn.BatchNorm2d(config.hidden_channels),
            nn.ReLU(),
        )

        # Residual tower
        self.res_blocks = nn.Sequential(
            *[ResidualBlock(config.hidden_channels) for _ in range(config.num_residual_blocks)]
        )

        # Value Head: pool -> FC -> scalar
        self.value_head = nn.Sequential(
            nn.Conv2d(config.hidden_channels, 1, 1),
            nn.BatchNorm2d(1),
            nn.Flatten(),
            nn.Linear(config.board_size * config.board_size, config.value_head_hidden),
            nn.ReLU(),
            nn.Linear(config.value_head_hidden, 1),
            nn.Tanh(),  # Output in [-1, 1]
        )

        # Policy Head: conv -> 77 move outputs (all possible UCI moves)
        self.num_policy_outputs = 77 * 64  # 77 piece-type-to-square moves * 64 target squares
        self.policy_head = nn.Sequential(
            nn.Conv2d(config.hidden_channels, config.policy_head_channels, 1, bias=False),
            nn.BatchNorm2d(config.policy_head_channels),
            nn.ReLU(),
            nn.Flatten(),
            nn.Linear(config.policy_head_channels * config.board_size * config.board_size,
                      self.num_policy_outputs),
        )

    def forward(self, x: torch.Tensor) -> Tuple[torch.Tensor, torch.Tensor]:
        """Forward pass.

        Args:
            x: Input tensor of shape (batch, input_planes, 8, 8).

        Returns:
            Tuple of (value, policy) tensors.
            value: (batch, 1) scalar evaluation in [-1, 1]
            policy: (batch, num_policy_outputs) move logits
        """
        hidden = self.input_conv(x)
        hidden = self.res_blocks(hidden)
        value = self.value_head(hidden)
        policy = self.policy_head(hidden)
        return value, policy


# ---------------------------------------------------------------------------
# Dataset
# ---------------------------------------------------------------------------

class ChessPositionDataset(Dataset):
    """Dataset for chess positions with value and policy targets.

    Expected data format: numpy files or list of (fen, outcome, move) tuples.
    """

    def __init__(
        self,
        positions: List[str],
        outcomes: List[float],
        policy_targets: Optional[List[int]] = None,
        num_planes: int = 18,
    ) -> None:
        self.positions = positions
        self.outcomes = outcomes
        self.policy_targets = policy_targets or [0] * len(positions)
        self.num_planes = num_planes

    def __len__(self) -> int:
        return len(self.positions)

    def __getitem__(self, idx: int) -> Tuple[torch.Tensor, torch.Tensor, torch.Tensor]:
        board = encode_board_from_fen(self.positions[idx], self.num_planes)
        board_tensor = torch.from_numpy(board)
        outcome = torch.tensor([self.outcomes[idx]], dtype=torch.float32)
        policy = torch.tensor(self.policy_targets[idx], dtype=torch.long)
        return board_tensor, outcome, policy

    @classmethod
    def from_synthetic(cls, n: int = 1000, num_planes: int = 18) -> "ChessPositionDataset":
        """Generate synthetic data for testing."""
        starting_fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
        positions = [starting_fen] * n
        outcomes = [np.random.uniform(-1, 1) for _ in range(n)]
        policy_targets = [np.random.randint(0, 4928) for _ in range(n)]  # 77*64
        return cls(positions, outcomes, policy_targets, num_planes)


# ---------------------------------------------------------------------------
# Lightning DataModule
# ---------------------------------------------------------------------------

try:
    import pytorch_lightning as L
    from pytorch_lightning.callbacks import ModelCheckpoint, LearningRateMonitor
    HAS_LIGHTNING = True
except ImportError:
    HAS_LIGHTNING = False


if HAS_LIGHTNING:

    class KnightVerseDataModule(L.LightningDataModule):
        """Lightning DataModule for streaming PGN datasets."""

        def __init__(
            self,
            config: TrainingConfig,
            train_dataset: Optional[ChessPositionDataset] = None,
            val_dataset: Optional[ChessPositionDataset] = None,
        ) -> None:
            super().__init__()
            self.config = config
            self.train_dataset = train_dataset
            self.val_dataset = val_dataset

        def setup(self, stage: Optional[str] = None) -> None:
            if self.train_dataset is not None:
                return  # Already provided

            # Generate synthetic data for testing
            full_dataset = ChessPositionDataset.from_synthetic(
                n=2000, num_planes=self.config.input_planes
            )
            val_size = int(len(full_dataset) * self.config.val_split)
            train_size = len(full_dataset) - val_size
            self.train_dataset, self.val_dataset = random_split(
                full_dataset, [train_size, val_size]
            )

        def train_dataloader(self) -> DataLoader:
            return DataLoader(
                self.train_dataset,
                batch_size=self.config.batch_size,
                shuffle=True,
                num_workers=self.config.num_workers,
                pin_memory=True,
            )

        def val_dataloader(self) -> DataLoader:
            return DataLoader(
                self.val_dataset,
                batch_size=self.config.batch_size,
                shuffle=False,
                num_workers=self.config.num_workers,
                pin_memory=True,
            )


    class KnightVerseModule(L.LightningModule):
        """PyTorch Lightning Module for KnightVerse Evaluation Head."""

        def __init__(self, config: TrainingConfig) -> None:
            super().__init__()
            self.config = config
            self.save_hyperparameters(config.__dict__)

            self.model = KnightVerseHead(config)
            self.value_loss_fn = nn.MSELoss()
            self.policy_loss_fn = nn.CrossEntropyLoss()

        def forward(self, x: torch.Tensor) -> Tuple[torch.Tensor, torch.Tensor]:
            return self.model(x)

        def training_step(self, batch: Tuple, batch_idx: int) -> torch.Tensor:
            board, outcome, policy_target = batch
            value_pred, policy_pred = self(board)

            value_loss = self.value_loss_fn(value_pred.squeeze(-1), outcome)
            policy_loss = self.policy_loss_fn(policy_pred, policy_target)
            total_loss = value_loss + 0.5 * policy_loss

            self.log("train/value_loss", value_loss, prog_bar=True)
            self.log("train/policy_loss", policy_loss)
            self.log("train/total_loss", total_loss, prog_bar=True)

            # Top-1 accuracy for policy
            pred_moves = policy_pred.argmax(dim=-1)
            accuracy = (pred_moves == policy_target).float().mean()
            self.log("train/policy_accuracy", accuracy, prog_bar=True)

            return total_loss

        def validation_step(self, batch: Tuple, batch_idx: int) -> torch.Tensor:
            board, outcome, policy_target = batch
            value_pred, policy_pred = self(board)

            value_loss = self.value_loss_fn(value_pred.squeeze(-1), outcome)
            policy_loss = self.policy_loss_fn(policy_pred, policy_target)
            total_loss = value_loss + 0.5 * policy_loss

            self.log("val/value_loss", value_loss, prog_bar=True)
            self.log("val/total_loss", total_loss, prog_bar=True)

            pred_moves = policy_pred.argmax(dim=-1)
            accuracy = (pred_moves == policy_target).float().mean()
            self.log("val/policy_accuracy", accuracy, prog_bar=True)

            return total_loss

        def configure_optimizers(self) -> Dict[str, Any]:
            optimizer = torch.optim.AdamW(
                self.parameters(),
                lr=self.config.learning_rate,
                weight_decay=self.config.weight_decay,
            )
            scheduler = torch.optim.lr_scheduler.CosineAnnealingLR(
                optimizer, T_max=self.config.max_epochs, eta_min=1e-6
            )
            return {
                "optimizer": optimizer,
                "lr_scheduler": {
                    "scheduler": scheduler,
                    "monitor": "val/total_loss",
                },
            }


# ---------------------------------------------------------------------------
# Training Entry Point
# ---------------------------------------------------------------------------

def train_valuation_head(
    config: Optional[TrainingConfig] = None,
    train_dataset: Optional[ChessPositionDataset] = None,
    val_dataset: Optional[ChessPositionDataset] = None,
) -> Optional[Any]:
    """Run the training pipeline.

    Args:
        config: Training configuration (uses defaults if None).
        train_dataset: Optional pre-loaded training dataset.
        val_dataset: Optional pre-loaded validation dataset.

    Returns:
        Trained LightningModule or None if Lightning not installed.
    """
    if not HAS_LIGHTNING:
        logger.error("PyTorch Lightning not installed. Install with: pip install pytorch-lightning")
        return None

    if config is None:
        config = TrainingConfig()

    logger.info("Starting KnightVerse Evaluation Head training")
    logger.info(f"  Precision: {config.precision}")
    logger.info(f"  Max epochs: {config.max_epochs}")
    logger.info(f"  Batch size: {config.batch_size}")

    # Setup callbacks
    checkpoint_callback = ModelCheckpoint(
        dirpath=config.checkpoint_dir,
        filename="knightverse-{epoch:02d}-{val_loss:.4f}",
        save_top_k=config.save_top_k,
        monitor=config.monitor_metric,
        mode="min",
    )

    lr_monitor = LearningRateMonitor(logging_interval="epoch")

    # Setup WandB logger
    try:
        wandb_logger = L.pytorch_lightning.loggers.WandbLogger(
            project=config.wandb_project,
            name=config.wandb_experiment,
        )
    except Exception:
        wandb_logger = L.pytorch_lightning.loggers.TensorBoardLogger(
            name="knightverse",
        )
        logger.warning("WandB unavailable, falling back to TensorBoard")

    # Data module
    dm = KnightVerseDataModule(config, train_dataset, val_dataset)

    # Model
    model = KnightVerseModule(config)

    # Trainer
    trainer = L.Trainer(
        max_epochs=config.max_epochs,
        devices=config.devices,
        strategy=config.strategy,
        precision=config.precision,
        callbacks=[checkpoint_callback, lr_monitor],
        logger=wandb_logger,
        log_every_n_steps=config.log_every_n_steps,
    )

    trainer.fit(model, datamodule=dm)
    logger.info("Training completed")
    return model


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------

def main() -> None:
    parser = argparse.ArgumentParser(description="Train KnightVerse Evaluation Head")
    parser.add_argument("--data_dir", type=str, default=None, help="Path to PGN data directory")
    parser.add_argument("--max_epochs", type=int, default=100)
    parser.add_argument("--batch_size", type=int, default=256)
    parser.add_argument("--learning_rate", type=float, default=0.001)
    parser.add_argument("--devices", type=int, default=-1, help="-1 for auto")
    parser.add_argument("--precision", type=str, default="16-mixed")
    args = parser.parse_args()

    config = TrainingConfig(
        max_epochs=args.max_epochs,
        batch_size=args.batch_size,
        learning_rate=args.learning_rate,
        devices=args.devices,
        precision=args.precision,
    )
    train_valuation_head(config)


if __name__ == "__main__":
    main()
