from __future__ import annotations

import asyncio
import logging
import os
import time
import uuid
from collections.abc import Callable
from concurrent.futures import ThreadPoolExecutor
from typing import Tuple

import chess
import numpy as np

from gpu_worker.config import WorkerConfig
from gpu_worker.models import AnalysisRequest, AnalysisResult, WorkerInfo, WorkerStatus
from gpu_worker.resource_monitor import ResourceMonitor

logger = logging.getLogger(__name__)

_MAIA_RATINGS = (1100, 1300, 1500, 1700, 1900)
_MAIA_MOVE_VOCAB_SIZE = 4672

try:
    import onnxruntime as ort
    _ONNX_AVAILABLE = True
except ImportError:
    ort = None
    _ONNX_AVAILABLE = False
    logger.warning("onnxruntime not installed; Maia worker will use fallback mode")


def _build_move_vocabulary() -> dict[str, int]:
    """Build the Maia move vocabulary mapping UCI move strings to indices.

    Maia uses a fixed vocabulary of 4672 moves representing all legal
    promotions and normal moves on a chess board.
    """
    vocab: dict[str, int] = {}
    idx = 0

    for rank in range(8):
        for file in range(8):
            for target_rank in range(8):
                for target_file in range(8):
                    if rank == target_rank and file == target_file:
                        continue
                    move_str = (
                        chr(ord("a") + file) + str(rank + 1) +
                        chr(ord("a") + target_file) + str(target_rank + 1)
                    )
                    vocab[move_str] = idx
                    idx += 1

                    if rank == 6 and target_rank == 7:
                        for promo in ("q", "r", "b", "n"):
                            promo_str = move_str + promo
                            vocab[promo_str] = idx
                            idx += 1
                    elif rank == 1 and target_rank == 0:
                        for promo in ("q", "r", "b", "n"):
                            promo_str = move_str + promo
                            vocab[promo_str] = idx
                            idx += 1

    return vocab


_MOVE_VOCAB = _build_move_vocabulary()
_INDEX_TO_MOVE = {v: k for k, v in _MOVE_VOCAB.items()}


def _encode_board_tensor(board: chess.Board) -> np.ndarray:
    """Encode a chess board as a 13x8x8 tensor for Maia inference.

    Channels:
    0-5:   White pieces (P, N, B, R, Q, K)
    6-11:  Black pieces (P, N, B, R, Q, K)
    12:    Side to move (all 1 if white, all 0 if black)
    """
    tensor = np.zeros((13, 8, 8), dtype=np.float32)

    piece_map = board.piece_map()
    for square, piece in piece_map.items():
        rank = 7 - (square // 8)
        file = square % 8
        piece_type = piece.piece_type
        is_white = piece.color == chess.WHITE

        if is_white:
            channel = piece_type - 1
        else:
            channel = piece_type + 5

        tensor[channel, rank, file] = 1.0

    if board.turn == chess.WHITE:
        tensor[12, :, :] = 1.0

    return tensor


def _get_legal_move_indices(board: chess.Board) -> set[int]:
    """Return the set of Maia vocabulary indices for legal moves."""
    indices = set()
    for move in board.legal_moves:
        uci = move.uci()
        if uci in _MOVE_VOCAB:
            indices.add(_MOVE_VOCAB[uci])
    return indices


class MaiaModel:
    """ONNX Runtime wrapper for a single Maia checkpoint."""

    def __init__(self, model_path: str, target_elo: int) -> None:
        if not _ONNX_AVAILABLE:
            raise RuntimeError("onnxruntime is required for Maia inference")

        self.target_elo = target_elo
        self.model_path = model_path
        self.session = self._load_session(model_path)
        self.input_name = self.session.get_inputs()[0].name

    def _load_session(self, model_path: str) -> ort.InferenceSession:
        """Load ONNX model with GPU preference and CPU fallback."""
        providers = ort.get_available_providers()
        use_gpu = "CUDAExecutionProvider" in providers or "ROCMExecutionProvider" in providers

        if use_gpu:
            session_options = ort.SessionOptions()
            session_options.graph_optimization_level = ort.GraphOptimizationLevel.ORT_ENABLE_ALL
            session_options.intra_op_num_threads = 1
            try:
                return ort.InferenceSession(
                    model_path,
                    sess_options=session_options,
                    providers=["CUDAExecutionProvider", "CPUExecutionProvider"],
                )
            except Exception as exc:
                logger.warning("GPU inference failed for Elo %d, falling back to CPU: %s", self.target_elo, exc)

        session_options = ort.SessionOptions()
        session_options.graph_optimization_level = ort.GraphOptimizationLevel.ORT_ENABLE_ALL
        session_options.intra_op_num_threads = 4
        return ort.InferenceSession(
            model_path,
            sess_options=session_options,
            providers=["CPUExecutionProvider"],
        )

    def predict(self, board_tensor: np.ndarray) -> np.ndarray:
        """Run inference and return move probabilities."""
        input_batch = board_tensor[np.newaxis, ...]
        outputs = self.session.run(None, {self.input_name: input_batch})
        logits = outputs[0][0]
        return logits

    def predict_batch(self, board_tensors: np.ndarray) -> np.ndarray:
        """Run batched inference and return move probabilities."""
        outputs = self.session.run(None, {self.input_name: board_tensors})
        return outputs[0]


class MaiaWorker:
    """Worker that uses Maia Chess models to predict human-like moves.

    Supports all five Maia rating levels (1100, 1300, 1500, 1700, 1900)
    with ONNX Runtime inference on GPU (with CPU fallback).
    """

    def __init__(
        self,
        config: WorkerConfig,
        model_path: str,
        worker_id: str | None = None,
        *,
        resource_monitor: ResourceMonitor | None = None,
    ) -> None:
        self.config = config
        self.worker_id = worker_id or str(uuid.uuid4())
        self.model_path = model_path
        self._monitor = resource_monitor or ResourceMonitor()
        self._status = WorkerStatus.IDLE
        self._started = False
        self._analyses_completed = 0
        self._started_at: float | None = None
        self._pending_count = 0
        self._pending_lock = asyncio.Lock()
        self._analysis_lock = asyncio.Lock()

        self._models: dict[int, MaiaModel] = {}
        self._executor = ThreadPoolExecutor(max_workers=4)
        self._load_models()

    def _load_models(self) -> None:
        """Load all Maia models specified in config or use default paths."""
        maia_configs = self.config.maia_models
        if not maia_configs:
            base_dir = self.model_path
            for elo in _MAIA_RATINGS:
                model_file = os.path.join(base_dir, f"maia_{elo}.onnx")
                if os.path.isfile(model_file):
                    maia_configs.append(type("MaiaConfig", (), {"name": f"maia_{elo}", "path": model_file, "elo": elo})())

        for maia_cfg in maia_configs:
            elo = maia_cfg.elo
            if elo not in _MAIA_RATINGS:
                logger.warning("Skipping unknown Maia rating: %d", elo)
                continue
            if not os.path.isfile(maia_cfg.path):
                logger.warning("Maia model not found at '%s' for Elo %d", maia_cfg.path, elo)
                continue
            try:
                self._models[elo] = MaiaModel(maia_cfg.path, elo)
                logger.info("Loaded Maia %d model from %s", elo, maia_cfg.path)
            except Exception as exc:
                logger.error("Failed to load Maia %d model: %s", elo, exc)

    @property
    def status(self) -> WorkerStatus:
        return self._status

    @property
    def load(self) -> int:
        return self._pending_count

    @property
    def has_capacity(self) -> bool:
        return self._pending_count < self.config.max_concurrent_analyses

    @property
    def available_elos(self) -> list[int]:
        return sorted(self._models.keys())

    async def start(self) -> None:
        """Start monitoring the worker."""
        if self._started:
            return
        try:
            await self._monitor.start()
        except Exception:
            self._status = WorkerStatus.ERROR
            raise
        self._started = True
        self._started_at = time.monotonic()
        self._status = WorkerStatus.IDLE

    async def predict_human_move(self, fen: str, target_elo: int) -> Tuple[str, float]:
        """Predict a human-like move for the given position and target Elo.

        Args:
            fen: Board position in FEN notation.
            target_elo: Target rating (1100, 1300, 1500, 1700, or 1900).

        Returns:
            Tuple of (move_uci, confidence_score).

        Raises:
            ValueError: If target_elo is not supported or no model is loaded.
            RuntimeError: If worker has not been started.
        """
        if not self._started:
            raise RuntimeError("worker has not been started")

        model = self._models.get(target_elo)
        if model is None:
            available = sorted(self._models.keys())
            raise ValueError(
                f"No Maia model loaded for Elo {target_elo}. "
                f"Available: {available}"
            )

        loop = asyncio.get_event_loop()
        return await loop.run_in_executor(
            self._executor,
            self._sync_predict,
            fen,
            model,
        )

    def _sync_predict(self, fen: str, model: MaiaModel) -> Tuple[str, float]:
        """Synchronous prediction with illegal move masking."""
        board = chess.Board(fen)
        tensor = _encode_board_tensor(board)
        logits = model.predict(tensor)

        legal_indices = _get_legal_move_indices(board)
        if not legal_indices:
            raise ValueError(f"No legal moves in position: {fen}")

        masked_logits = np.full_like(logits, -np.inf)
        for idx in legal_indices:
            masked_logits[idx] = logits[idx]

        max_logit = np.max(masked_logits)
        exp_logits = np.exp(masked_logits - max_logit)
        probs = exp_logits / np.sum(exp_logits)

        rng = np.random.default_rng()
        move_idx = int(rng.choice(len(probs), p=probs))
        move_uci = _INDEX_TO_MOVE.get(move_idx)
        if move_uci is None:
            legal_moves = list(board.legal_moves)
            move_uci = legal_moves[0].uci()

        confidence = float(probs[move_idx])
        return move_uci, confidence

    async def predict_batch(
        self,
        fens: list[str],
        target_elo: int,
    ) -> list[Tuple[str, float]]:
        """Predict moves for multiple positions in a single batch.

        Args:
            fens: List of FEN positions.
            target_elo: Target rating for all positions.

        Returns:
            List of (move_uci, confidence) tuples.
        """
        if not self._started:
            raise RuntimeError("worker has not been started")

        model = self._models.get(target_elo)
        if model is None:
            available = sorted(self._models.keys())
            raise ValueError(
                f"No Maia model loaded for Elo {target_elo}. "
                f"Available: {available}"
            )

        loop = asyncio.get_event_loop()
        return await loop.run_in_executor(
            self._executor,
            self._sync_predict_batch,
            fens,
            model,
        )

    def _sync_predict_batch(
        self,
        fens: list[str],
        model: MaiaModel,
    ) -> list[Tuple[str, float]]:
        """Synchronous batched prediction."""
        boards = [chess.Board(fen) for fen in fens]
        tensors = np.stack([_encode_board_tensor(b) for b in boards])
        logits_batch = model.predict_batch(tensors)

        results: list[Tuple[str, float]] = []
        rng = np.random.default_rng()

        for i, board in enumerate(boards):
            logits = logits_batch[i]
            legal_indices = _get_legal_move_indices(board)

            if not legal_indices:
                results.append(("", 0.0))
                continue

            masked_logits = np.full_like(logits, -np.inf)
            for idx in legal_indices:
                masked_logits[idx] = logits[idx]

            max_logit = np.max(masked_logits)
            exp_logits = np.exp(masked_logits - max_logit)
            probs = exp_logits / np.sum(exp_logits)

            move_idx = int(rng.choice(len(probs), p=probs))
            move_uci = _INDEX_TO_MOVE.get(move_idx)
            if move_uci is None:
                legal_moves = list(board.legal_moves)
                move_uci = legal_moves[0].uci()

            results.append((move_uci, float(probs[move_idx])))

        return results

    async def analyze(self, request: AnalysisRequest) -> AnalysisResult:
        """Analyze one position and return the predicted move."""
        if not self._started:
            raise RuntimeError("worker has not been started")

        async with self._pending_lock:
            if self._pending_count >= self.config.max_concurrent_analyses:
                raise RuntimeError("worker is at capacity")
            self._pending_count += 1

        started_at = time.monotonic()
        try:
            async with self._analysis_lock:
                self._status = WorkerStatus.BUSY

                target_elo = self._pick_elo_for_request(request)
                move_uci, confidence = await self.predict_human_move(request.fen, target_elo)

                gpu_stats = self._monitor.get_gpu_stats()
                result = AnalysisResult(
                    request_id=request.id,
                    best_move=move_uci,
                    evaluation=confidence,
                    depth=0,
                    principal_variation=[move_uci],
                    nodes_searched=0,
                    time_ms=int((time.monotonic() - started_at) * 1000),
                    gpu_utilization=_gpu_utilization_for_device(
                        gpu_stats, self.config.gpu.device_id
                    ),
                )
                self._analyses_completed += 1
                return result
        except Exception:
            self._status = WorkerStatus.ERROR
            raise
        finally:
            async with self._pending_lock:
                self._pending_count -= 1
                if self._status != WorkerStatus.ERROR:
                    self._status = (
                        WorkerStatus.BUSY if self._pending_count > 0 else WorkerStatus.IDLE
                    )

    def _pick_elo_for_request(self, request: AnalysisRequest) -> int:
        """Select the closest available Maia Elo for a request."""
        if self.available_elos:
            return self.available_elos[0]
        return 1500

    async def shutdown(self) -> None:
        """Gracefully stop monitoring and release resources."""
        self._status = WorkerStatus.SHUTTING_DOWN
        await self._monitor.stop()
        self._executor.shutdown(wait=False)
        self._started = False

    def get_info(self) -> WorkerInfo:
        """Return a runtime snapshot for pool monitoring."""
        gpu_stats = self._monitor.get_gpu_stats()
        device_stats = _gpu_device_stats(gpu_stats, self.config.gpu.device_id)
        uptime_seconds = 0.0
        if self._started_at is not None:
            uptime_seconds = max(0.0, time.monotonic() - self._started_at)
        return WorkerInfo(
            worker_id=self.worker_id,
            status=self._status,
            gpu_device_id=self.config.gpu.device_id,
            gpu_memory_used_mb=float(device_stats.get("memory_used_mb", 0.0)),
            gpu_utilization_pct=float(device_stats.get("utilization_pct", 0.0)),
            analyses_completed=self._analyses_completed,
            uptime_seconds=uptime_seconds,
        )


def _gpu_device_stats(gpu_stats: dict, device_id: int) -> dict:
    """Return the monitoring payload for one GPU device."""
    for device in gpu_stats.get("devices", []):
        if device.get("device_id") == device_id:
            return device
    return {}


def _gpu_utilization_for_device(gpu_stats: dict, device_id: int) -> float | None:
    """Return the utilization percentage for one GPU device if known."""
    device = _gpu_device_stats(gpu_stats, device_id)
    utilization = device.get("utilization_pct")
    return None if utilization is None else float(utilization)
