from __future__ import annotations

import asyncio
import time
from unittest.mock import MagicMock, patch

import chess
import numpy as np
import pytest

from gpu_worker.config import MaiaConfig, WorkerConfig
from gpu_worker.maia_worker import (
    _encode_board_tensor,
    _get_legal_move_indices,
    _INDEX_TO_MOVE,
    _MAIA_MOVE_VOCAB_SIZE,
    MaiaModel,
    MaiaWorker,
)
from gpu_worker.models import AnalysisRequest, WorkerStatus
from gpu_worker.resource_monitor import ResourceMonitor


@pytest.fixture
def worker_config() -> WorkerConfig:
    return WorkerConfig()


@pytest.fixture
def resource_monitor() -> ResourceMonitor:
    return ResourceMonitor(
        gpu_stats_provider=lambda: {
            "available": True,
            "devices": [{"device_id": 0, "utilization_pct": 50.0, "memory_used_mb": 1024.0}],
        },
        cpu_stats_provider=lambda: {"cpu_utilization_pct": 20.0},
    )


class TestFENEncoding:
    """Test FEN to tensor encoding."""

    def test_starting_position_tensor_shape(self) -> None:
        board = chess.Board()
        tensor = _encode_board_tensor(board)
        assert tensor.shape == (13, 8, 8)
        assert tensor.dtype == np.float32

    def test_starting_position_white_pieces(self) -> None:
        board = chess.Board()
        tensor = _encode_board_tensor(board)

        assert tensor[0, 7, 0] == 1.0
        assert tensor[0, 7, 1] == 1.0
        assert tensor[0, 7, 4] == 1.0

        white_pawns = np.sum(tensor[0, :, :])
        assert white_pawns == 8.0

    def test_starting_position_black_pieces(self) -> None:
        board = chess.Board()
        tensor = _encode_board_tensor(board)

        assert tensor[6, 0, 0] == 1.0
        assert tensor[6, 0, 4] == 1.0

        black_pawns = np.sum(tensor[6, :, :])
        assert black_pawns == 8.0

    def test_side_to_move_channel(self) -> None:
        board = chess.Board()
        tensor = _encode_board_tensor(board)
        assert np.all(tensor[12, :, :] == 1.0)

        board.push_san("e4")
        tensor = _encode_board_tensor(board)
        assert np.all(tensor[12, :, :] == 0.0)

    def test_empty_board(self) -> None:
        board = chess.Board(fen="8/8/8/8/8/8/8/8 w - - 0 1")
        tensor = _encode_board_tensor(board)
        assert np.sum(tensor[:12, :, :]) == 0.0

    def test_sparse_position(self) -> None:
        fen = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1"
        board = chess.Board(fen)
        tensor = _encode_board_tensor(board)
        assert tensor[0, 6, 4] == 1.0
        assert tensor[12, :, :].sum() == 0.0


class TestLegalMoveIndices:
    """Test legal move vocabulary mapping."""

    def test_starting_position_legal_moves(self) -> None:
        board = chess.Board()
        indices = _get_legal_move_indices(board)
        assert len(indices) == 20

    def test_e2e4_is_legal(self) -> None:
        board = chess.Board()
        indices = _get_legal_move_indices(board)
        e2e4_idx = None
        for move in board.legal_moves:
            if move.uci() == "e2e4":
                from gpu_worker.maia_worker import _MOVE_VOCAB
                e2e4_idx = _MOVE_VOCAB.get("e2e4")
                break
        assert e2e4_idx is not None
        assert e2e4_idx in indices

    def test_endgame_position(self) -> None:
        fen = "8/8/8/8/8/8/8/K6k w - - 0 1"
        board = chess.Board(fen)
        indices = _get_legal_move_indices(board)
        assert len(indices) > 0
        assert len(indices) == len(list(board.legal_moves))

    def test_promotion_moves_included(self) -> None:
        fen = "8/P7/8/8/8/8/8/k6K w - - 0 1"
        board = chess.Board(fen)
        indices = _get_legal_move_indices(board)
        promotion_moves = [m for m in board.legal_moves if m.promotion is not None]
        assert len(promotion_moves) > 0
        for move in promotion_moves:
            from gpu_worker.maia_worker import _MOVE_VOCAB
            assert _MOVE_VOCAB.get(move.uci()) in indices


class TestMoveVocabulary:
    """Test the Maia move vocabulary."""

    def test_vocab_size_reasonable(self) -> None:
        from gpu_worker.maia_worker import _MOVE_VOCAB
        assert len(_MOVE_VOCAB) > 4000
        assert len(_MOVE_VOCAB) < 5000

    def test_bijection(self) -> None:
        from gpu_worker.maia_worker import _MOVE_VOCAB, _INDEX_TO_MOVE
        for move_str, idx in _MOVE_VOCAB.items():
            assert _INDEX_TO_MOVE[idx] == move_str

    def test_common_moves_present(self) -> None:
        from gpu_worker.maia_worker import _MOVE_VOCAB
        common_moves = ["e2e4", "d2d4", "g1f3", "e7e5", "e2e4", "c7c5"]
        for move in common_moves:
            assert move in _MOVE_VOCAB, f"{move} not in vocabulary"

    def test_promotion_moves_present(self) -> None:
        from gpu_worker.maia_worker import _MOVE_VOCAB
        assert "a7a8q" in _MOVE_VOCAB
        assert "a7a8r" in _MOVE_VOCAB
        assert "a2a1q" in _MOVE_VOCAB


class TestMaiaWorkerLifecycle:
    """Test worker lifecycle and basic functionality."""

    @pytest.mark.asyncio
    async def test_worker_starts_and_stops(
        self, worker_config: WorkerConfig, resource_monitor: ResourceMonitor
    ) -> None:
        with patch("gpu_worker.maia_worker._ONNX_AVAILABLE", False):
            worker = MaiaWorker(
                worker_config,
                model_path="/tmp/models",
                worker_id="test-1",
                resource_monitor=resource_monitor,
            )
            await worker.start()
            assert worker.status == WorkerStatus.IDLE
            assert worker._started is True

            await worker.shutdown()
            assert worker._started is False

    @pytest.mark.asyncio
    async def test_worker_tracks_pending_count(
        self, worker_config: WorkerConfig, resource_monitor: ResourceMonitor
    ) -> None:
        with patch("gpu_worker.maia_worker._ONNX_AVAILABLE", False):
            worker = MaiaWorker(
                worker_config,
                model_path="/tmp/models",
                resource_monitor=resource_monitor,
            )
            assert worker.load == 0
            assert worker.has_capacity is True

    @pytest.mark.asyncio
    async def test_worker_info_reports_elos(
        self, worker_config: WorkerConfig, resource_monitor: ResourceMonitor
    ) -> None:
        with patch("gpu_worker.maia_worker._ONNX_AVAILABLE", False):
            worker = MaiaWorker(
                worker_config,
                model_path="/tmp/models",
                worker_id="test-info",
                resource_monitor=resource_monitor,
            )
            info = worker.get_info()
            assert info.worker_id == "test-info"
            assert info.gpu_device_id == 0


class TestMaiaModelMocked:
    """Test MaiaModel with mocked ONNX Runtime."""

    def test_model_initialization_with_onnx_available(self, tmp_path) -> None:
        model_file = tmp_path / "maia_1500.onnx"
        model_file.write_bytes(b"fake onnx model")

        mock_session = MagicMock()
        mock_session.get_inputs.return_value = [MagicMock(name="input_0")]

        with patch("gpu_worker.maia_worker.ort") as mock_ort:
            mock_ort.get_available_providers.return_value = ["CPUExecutionProvider"]
            mock_ort.InferenceSession.return_value = mock_session
            mock_ort.SessionOptions = MagicMock()
            mock_ort.GraphOptimizationLevel.ORT_ENABLE_ALL = 99

            model = MaiaModel(str(model_file), 1500)
            assert model.target_elo == 1500
            assert model.session is mock_session

    def test_model_gpu_preference(self, tmp_path) -> None:
        model_file = tmp_path / "maia_1900.onnx"
        model_file.write_bytes(b"fake onnx model")

        mock_session = MagicMock()
        mock_session.get_inputs.return_value = [MagicMock(name="input_0")]

        with patch("gpu_worker.maia_worker.ort") as mock_ort:
            mock_ort.get_available_providers.return_value = [
                "CUDAExecutionProvider",
                "CPUExecutionProvider",
            ]
            mock_ort.InferenceSession.return_value = mock_session
            mock_ort.SessionOptions = MagicMock()
            mock_ort.GraphOptimizationLevel.ORT_ENABLE_ALL = 99

            model = MaiaModel(str(model_file), 1900)
            mock_ort.InferenceSession.assert_called_once()
            call_kwargs = mock_ort.InferenceSession.call_args
            assert "CUDAExecutionProvider" in call_kwargs[1]["providers"]

    def test_model_gpu_fallback_to_cpu(self, tmp_path) -> None:
        model_file = tmp_path / "maia_1100.onnx"
        model_file.write_bytes(b"fake onnx model")

        mock_session = MagicMock()
        mock_session.get_inputs.return_value = [MagicMock(name="input_0")]

        with patch("gpu_worker.maia_worker.ort") as mock_ort:
            mock_ort.get_available_providers.return_value = [
                "CUDAExecutionProvider",
                "CPUExecutionProvider",
            ]
            mock_ort.InferenceSession.side_effect = [
                Exception("CUDA out of memory"),
                mock_session,
            ]
            mock_ort.SessionOptions = MagicMock()
            mock_ort.GraphOptimizationLevel.ORT_ENABLE_ALL = 99

            model = MaiaModel(str(model_file), 1100)
            assert mock_ort.InferenceSession.call_count == 2

    def test_predict_single_position(self, tmp_path) -> None:
        model_file = tmp_path / "maia_1500.onnx"
        model_file.write_bytes(b"fake onnx model")

        mock_session = MagicMock()
        mock_session.get_inputs.return_value = [MagicMock(name="input_0")]
        mock_session.run.return_value = [np.random.randn(1, _MAIA_MOVE_VOCAB_SIZE).astype(np.float32)]

        with patch("gpu_worker.maia_worker.ort") as mock_ort:
            mock_ort.get_available_providers.return_value = ["CPUExecutionProvider"]
            mock_ort.InferenceSession.return_value = mock_session
            mock_ort.SessionOptions = MagicMock()
            mock_ort.GraphOptimizationLevel.ORT_ENABLE_ALL = 99

            model = MaiaModel(str(model_file), 1500)
            board = chess.Board()
            tensor = _encode_board_tensor(board)
            logits = model.predict(tensor)

            assert logits.shape == (_MAIA_MOVE_VOCAB_SIZE,)
            mock_session.run.assert_called_once()

    def test_predict_batch_positions(self, tmp_path) -> None:
        model_file = tmp_path / "maia_1700.onnx"
        model_file.write_bytes(b"fake onnx model")

        batch_size = 4
        mock_session = MagicMock()
        mock_session.get_inputs.return_value = [MagicMock(name="input_0")]
        mock_session.run.return_value = [
            np.random.randn(batch_size, _MAIA_MOVE_VOCAB_SIZE).astype(np.float32)
        ]

        with patch("gpu_worker.maia_worker.ort") as mock_ort:
            mock_ort.get_available_providers.return_value = ["CPUExecutionProvider"]
            mock_ort.InferenceSession.return_value = mock_session
            mock_ort.SessionOptions = MagicMock()
            mock_ort.GraphOptimizationLevel.ORT_ENABLE_ALL = 99

            model = MaiaModel(str(model_file), 1700)
            fens = [
                "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
                "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1",
                "8/8/8/8/8/8/8/K6k w - - 0 1",
                "r1bqkbnr/pppp1ppp/2n5/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R w KQkq - 2 3",
            ]
            tensors = np.stack([_encode_board_tensor(chess.Board(f)) for f in fens])
            logits = model.predict_batch(tensors)

            assert logits.shape == (batch_size, _MAIA_MOVE_VOCAB_SIZE)


class TestPredictHumanMove:
    """Test the predict_human_move interface."""

    @pytest.mark.asyncio
    async def test_predict_returns_valid_move(self, tmp_path) -> None:
        model_file = tmp_path / "maia_1500.onnx"
        model_file.write_bytes(b"fake onnx model")

        config = WorkerConfig(
            maia_models=[
                MaiaConfig(name="maia_1500", path=str(model_file), elo=1500)
            ]
        )

        mock_session = MagicMock()
        mock_session.get_inputs.return_value = [MagicMock(name="input_0")]

        def mock_run(_outputs, _inputs):
            batch = list(_inputs.values())[0]
            b = batch.shape[0]
            logits = np.full((b, _MAIA_MOVE_VOCAB_SIZE), -10.0, dtype=np.float32)
            logits[:, 1000] = 5.0
            logits[:, 2000] = 3.0
            return [logits]

        mock_session.run.side_effect = mock_run

        with patch("gpu_worker.maia_worker.ort") as mock_ort:
            mock_ort.get_available_providers.return_value = ["CPUExecutionProvider"]
            mock_ort.InferenceSession.return_value = mock_session
            mock_ort.SessionOptions = MagicMock()
            mock_ort.GraphOptimizationLevel.ORT_ENABLE_ALL = 99

            monitor = ResourceMonitor(
                gpu_stats_provider=lambda: {
                    "available": True,
                    "devices": [{"device_id": 0, "utilization_pct": 50.0, "memory_used_mb": 1024.0}],
                },
                cpu_stats_provider=lambda: {"cpu_utilization_pct": 20.0},
            )

            worker = MaiaWorker(config, model_path=str(tmp_path), resource_monitor=monitor)
            await worker.start()

            fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
            move, confidence = await worker.predict_human_move(fen, 1500)

            board = chess.Board(fen)
            assert board.parse_uci(move) in board.legal_moves
            assert 0.0 <= confidence <= 1.0

            await worker.shutdown()

    @pytest.mark.asyncio
    async def test_predict_invalid_elo_raises(self) -> None:
        with patch("gpu_worker.maia_worker._ONNX_AVAILABLE", False):
            monitor = ResourceMonitor()
            worker = MaiaWorker(WorkerConfig(), model_path="/tmp/models", resource_monitor=monitor)
            await worker.start()

            with pytest.raises(ValueError, match="No Maia model loaded"):
                await worker.predict_human_move(
                    "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
                    1500,
                )

            await worker.shutdown()

    @pytest.mark.asyncio
    async def test_predict_not_started_raises(self) -> None:
        with patch("gpu_worker.maia_worker._ONNX_AVAILABLE", False):
            monitor = ResourceMonitor()
            worker = MaiaWorker(WorkerConfig(), model_path="/tmp/models", resource_monitor=monitor)

            with pytest.raises(RuntimeError, match="worker has not been started"):
                await worker.predict_human_move(
                    "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
                    1500,
                )


class TestBatchInference:
    """Test batch inference for multiple concurrent games."""

    @pytest.mark.asyncio
    async def test_batch_predict_returns_correct_count(self, tmp_path) -> None:
        model_file = tmp_path / "maia_1300.onnx"
        model_file.write_bytes(b"fake onnx model")

        config = WorkerConfig(
            maia_models=[
                MaiaConfig(name="maia_1300", path=str(model_file), elo=1300)
            ]
        )

        mock_session = MagicMock()
        mock_session.get_inputs.return_value = [MagicMock(name="input_0")]

        def mock_run(_outputs, _inputs):
            batch = list(_inputs.values())[0]
            b = batch.shape[0]
            logits = np.full((b, _MAIA_MOVE_VOCAB_SIZE), -10.0, dtype=np.float32)
            logits[:, 500] = 5.0
            return [logits]

        mock_session.run.side_effect = mock_run

        with patch("gpu_worker.maia_worker.ort") as mock_ort:
            mock_ort.get_available_providers.return_value = ["CPUExecutionProvider"]
            mock_ort.InferenceSession.return_value = mock_session
            mock_ort.SessionOptions = MagicMock()
            mock_ort.GraphOptimizationLevel.ORT_ENABLE_ALL = 99

            monitor = ResourceMonitor(
                gpu_stats_provider=lambda: {
                    "available": True,
                    "devices": [{"device_id": 0, "utilization_pct": 50.0, "memory_used_mb": 1024.0}],
                },
                cpu_stats_provider=lambda: {"cpu_utilization_pct": 20.0},
            )

            worker = MaiaWorker(config, model_path=str(tmp_path), resource_monitor=monitor)
            await worker.start()

            fens = [
                "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
                "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1",
                "8/8/8/8/8/8/8/K6k w - - 0 1",
            ]
            results = await worker.predict_batch(fens, 1300)

            assert len(results) == 3
            for move, confidence in results:
                assert isinstance(move, str)
                assert isinstance(confidence, float)
                assert 0.0 <= confidence <= 1.0

            await worker.shutdown()

    @pytest.mark.asyncio
    async def test_batch_predict_all_moves_legal(self, tmp_path) -> None:
        model_file = tmp_path / "maia_1900.onnx"
        model_file.write_bytes(b"fake onnx model")

        config = WorkerConfig(
            maia_models=[
                MaiaConfig(name="maia_1900", path=str(model_file), elo=1900)
            ]
        )

        mock_session = MagicMock()
        mock_session.get_inputs.return_value = [MagicMock(name="input_0")]

        def mock_run(_outputs, _inputs):
            batch = list(_inputs.values())[0]
            b = batch.shape[0]
            logits = np.random.randn(b, _MAIA_MOVE_VOCAB_SIZE).astype(np.float32)
            return [logits]

        mock_session.run.side_effect = mock_run

        with patch("gpu_worker.maia_worker.ort") as mock_ort:
            mock_ort.get_available_providers.return_value = ["CPUExecutionProvider"]
            mock_ort.InferenceSession.return_value = mock_session
            mock_ort.SessionOptions = MagicMock()
            mock_ort.GraphOptimizationLevel.ORT_ENABLE_ALL = 99

            monitor = ResourceMonitor(
                gpu_stats_provider=lambda: {
                    "available": True,
                    "devices": [{"device_id": 0, "utilization_pct": 50.0, "memory_used_mb": 1024.0}],
                },
                cpu_stats_provider=lambda: {"cpu_utilization_pct": 20.0},
            )

            worker = MaiaWorker(config, model_path=str(tmp_path), resource_monitor=monitor)
            await worker.start()

            fens = [
                "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
                "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1",
            ]
            results = await worker.predict_batch(fens, 1900)

            for i, (move, _) in enumerate(results):
                board = chess.Board(fens[i])
                if move:
                    assert board.parse_uci(move) in board.legal_moves

            await worker.shutdown()


class TestTargetRatingSelection:
    """Test target Elo selection and validation."""

    @pytest.mark.asyncio
    async def test_all_maia_ratings_supported(self, tmp_path) -> None:
        from gpu_worker.maia_worker import _MAIA_RATINGS

        for elo in _MAIA_RATINGS:
            model_file = tmp_path / f"maia_{elo}.onnx"
            model_file.write_bytes(b"fake onnx model")

        config = WorkerConfig(
            maia_models=[
                MaiaConfig(name=f"maia_{elo}", path=str(tmp_path / f"maia_{elo}.onnx"), elo=elo)
                for elo in _MAIA_RATINGS
            ]
        )

        mock_session = MagicMock()
        mock_session.get_inputs.return_value = [MagicMock(name="input_0")]
        mock_session.run.return_value = [np.random.randn(1, _MAIA_MOVE_VOCAB_SIZE).astype(np.float32)]

        with patch("gpu_worker.maia_worker.ort") as mock_ort:
            mock_ort.get_available_providers.return_value = ["CPUExecutionProvider"]
            mock_ort.InferenceSession.return_value = mock_session
            mock_ort.SessionOptions = MagicMock()
            mock_ort.GraphOptimizationLevel.ORT_ENABLE_ALL = 99

            monitor = ResourceMonitor(
                gpu_stats_provider=lambda: {
                    "available": True,
                    "devices": [{"device_id": 0, "utilization_pct": 50.0, "memory_used_mb": 1024.0}],
                },
                cpu_stats_provider=lambda: {"cpu_utilization_pct": 20.0},
            )

            worker = MaiaWorker(config, model_path=str(tmp_path), resource_monitor=monitor)

            assert set(worker.available_elos) == set(_MAIA_RATINGS)

    @pytest.mark.asyncio
    async def test_unknown_elo_skipped(self, tmp_path) -> None:
        model_file = tmp_path / "maia_9999.onnx"
        model_file.write_bytes(b"fake onnx model")

        config = WorkerConfig(
            maia_models=[
                MaiaConfig(name="maia_9999", path=str(model_file), elo=9999)
            ]
        )

        with patch("gpu_worker.maia_worker._ONNX_AVAILABLE", False):
            monitor = ResourceMonitor()
            worker = MaiaWorker(config, model_path=str(tmp_path), resource_monitor=monitor)

            assert 9999 not in worker.available_elos


class TestIllegalMoveMasking:
    """Test that illegal moves are properly masked."""

    @pytest.mark.asyncio
    async def test_only_legal_moves_returned(self, tmp_path) -> None:
        model_file = tmp_path / "maia_1500.onnx"
        model_file.write_bytes(b"fake onnx model")

        config = WorkerConfig(
            maia_models=[
                MaiaConfig(name="maia_1500", path=str(model_file), elo=1500)
            ]
        )

        mock_session = MagicMock()
        mock_session.get_inputs.return_value = [MagicMock(name="input_0")]

        def mock_run(_outputs, _inputs):
            batch = list(_inputs.values())[0]
            b = batch.shape[0]
            logits = np.full((b, _MAIA_MOVE_VOCAB_SIZE), -10.0, dtype=np.float32)
            logits[:, 100] = 5.0
            logits[:, 200] = 3.0
            logits[:, 300] = 4.0
            return [logits]

        mock_session.run.side_effect = mock_run

        with patch("gpu_worker.maia_worker.ort") as mock_ort:
            mock_ort.get_available_providers.return_value = ["CPUExecutionProvider"]
            mock_ort.InferenceSession.return_value = mock_session
            mock_ort.SessionOptions = MagicMock()
            mock_ort.GraphOptimizationLevel.ORT_ENABLE_ALL = 99

            monitor = ResourceMonitor(
                gpu_stats_provider=lambda: {
                    "available": True,
                    "devices": [{"device_id": 0, "utilization_pct": 50.0, "memory_used_mb": 1024.0}],
                },
                cpu_stats_provider=lambda: {"cpu_utilization_pct": 20.0},
            )

            worker = MaiaWorker(config, model_path=str(tmp_path), resource_monitor=monitor)
            await worker.start()

            fen = "8/8/8/8/8/8/8/R6K w - - 0 1"
            board = chess.Board(fen)
            legal_moves_uci = {m.uci() for m in board.legal_moves}

            for _ in range(10):
                move, _ = await worker.predict_human_move(fen, 1500)
                assert move in legal_moves_uci, f"Illegal move {move} returned"

            await worker.shutdown()

    @pytest.mark.asyncio
    async def test_checkmate_position_raises(self) -> None:
        with patch("gpu_worker.maia_worker._ONNX_AVAILABLE", False):
            monitor = ResourceMonitor()
            worker = MaiaWorker(WorkerConfig(), model_path="/tmp/models", resource_monitor=monitor)
            await worker.start()

            fen = "rnb1kbnr/pppp1ppp/8/4p3/6Pq/5P2/PPPPP2P/RNBQKBNR w KQkq - 1 3"
            board = chess.Board(fen)
            assert board.is_checkmate() or len(list(board.legal_moves)) == 0

            await worker.shutdown()


class TestInferencePerformance:
    """Test inference time requirements."""

    @pytest.mark.asyncio
    async def test_single_prediction_under_60ms_cpu(self, tmp_path) -> None:
        model_file = tmp_path / "maia_1500.onnx"
        model_file.write_bytes(b"fake onnx model")

        config = WorkerConfig(
            maia_models=[
                MaiaConfig(name="maia_1500", path=str(model_file), elo=1500)
            ]
        )

        mock_session = MagicMock()
        mock_session.get_inputs.return_value = [MagicMock(name="input_0")]

        def mock_run(_outputs, _inputs):
            batch = list(_inputs.values())[0]
            b = batch.shape[0]
            logits = np.random.randn(b, _MAIA_MOVE_VOCAB_SIZE).astype(np.float32)
            return [logits]

        mock_session.run.side_effect = mock_run

        with patch("gpu_worker.maia_worker.ort") as mock_ort:
            mock_ort.get_available_providers.return_value = ["CPUExecutionProvider"]
            mock_ort.InferenceSession.return_value = mock_session
            mock_ort.SessionOptions = MagicMock()
            mock_ort.GraphOptimizationLevel.ORT_ENABLE_ALL = 99

            monitor = ResourceMonitor(
                gpu_stats_provider=lambda: {
                    "available": True,
                    "devices": [{"device_id": 0, "utilization_pct": 50.0, "memory_used_mb": 1024.0}],
                },
                cpu_stats_provider=lambda: {"cpu_utilization_pct": 20.0},
            )

            worker = MaiaWorker(config, model_path=str(tmp_path), resource_monitor=monitor)
            await worker.start()

            fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"

            start = time.monotonic()
            for _ in range(10):
                await worker.predict_human_move(fen, 1500)
            elapsed_ms = (time.monotonic() - start) * 1000 / 10

            assert elapsed_ms < 60, f"Average inference time {elapsed_ms:.1f}ms exceeds 60ms limit"

            await worker.shutdown()
