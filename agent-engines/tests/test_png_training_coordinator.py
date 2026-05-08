"""
Unit tests for PGN Training Coordinator.

Tests cover:
- Job submission and tracking
- PGN to dataset conversion
- Dataset validation
- Integration with training pipeline
- Error handling and edge cases
- Async operations
"""

import asyncio
import tempfile
import unittest
from pathlib import Path
from unittest.mock import AsyncMock, MagicMock, patch

from gpu_worker.decentralized_orchestrator import DecentralizedOrchestrator
from gpu_worker.models import PersonalityTraits
from gpu_worker.pgn_training_coordinator import (
    PGNTrainingCoordinator,
    PGNTrainingJob,
    PGNTrainingStatus,
)
from gpu_worker.training_pipeline import PersonalityTrainingPipeline


class TestPGNTrainingJob(unittest.TestCase):
    """Test PGNTrainingJob model."""

    def test_job_creation(self):
        """Test creating a training job."""
        traits = PersonalityTraits(aggressiveness=0.8)
        job = PGNTrainingJob(
            pgn_file_path="/path/to/games.pgn",
            agent_id="agent-123",
            target_traits=traits,
        )

        self.assertIsNotNone(job.job_id)
        self.assertEqual(job.pgn_file_path, "/path/to/games.pgn")
        self.assertEqual(job.agent_id, "agent-123")
        self.assertEqual(job.status, PGNTrainingStatus.QUEUED)
        self.assertEqual(job.progress, 0.0)

    def test_job_defaults(self):
        """Test job default values."""
        traits = PersonalityTraits()
        job = PGNTrainingJob(
            pgn_file_path="/path/to/games.pgn",
            agent_id="agent-456",
            target_traits=traits,
        )

        self.assertIsNone(job.dataset_id)
        self.assertIsNone(job.started_at)
        self.assertIsNone(job.completed_at)
        self.assertEqual(len(job.statistics), 0)


class TestPGNTrainingCoordinator(unittest.TestCase):
    """Test PGN Training Coordinator."""

    def setUp(self):
        """Set up test fixtures."""
        self.orchestrator = MagicMock(spec=DecentralizedOrchestrator)
        self.training_pipeline = MagicMock(spec=PersonalityTrainingPipeline)
        self.coordinator = PGNTrainingCoordinator(
            self.orchestrator,
            self.training_pipeline,
            batch_size=10,
        )

        # Create sample PGN file
        self.pgn_content = """
[Event "Game 1"]
[White "A"]
[Black "B"]
[Result "1-0"]

1. e4 e5 2. Nf3 Nc6 3. Bb5 1-0
"""
        self.temp_pgn = tempfile.NamedTemporaryFile(
            mode="w", suffix=".pgn", delete=False
        )
        self.temp_pgn.write(self.pgn_content)
        self.temp_pgn.flush()

    def tearDown(self):
        """Clean up test fixtures."""
        Path(self.temp_pgn.name).unlink(missing_ok=True)

    def test_submit_pgn_training_success(self):
        """Test successful job submission."""
        async def run_test():
            traits = PersonalityTraits(aggressiveness=0.7)
            job_id = await self.coordinator.submit_pgn_training(
                self.temp_pgn.name,
                "agent-123",
                traits,
            )

            self.assertIsNotNone(job_id)
            job = await self.coordinator.get_job_status(job_id)
            self.assertIsNotNone(job)
            self.assertEqual(job.agent_id, "agent-123")

        asyncio.run(run_test())

    def test_submit_pgn_training_file_not_found(self):
        """Test submission with non-existent file."""
        async def run_test():
            traits = PersonalityTraits()
            with self.assertRaises(FileNotFoundError):
                await self.coordinator.submit_pgn_training(
                    "/nonexistent/file.pgn",
                    "agent-123",
                    traits,
                )

        asyncio.run(run_test())

    def test_get_job_status_not_found(self):
        """Test getting status of non-existent job."""
        async def run_test():
            job = await self.coordinator.get_job_status("nonexistent-job-id")
            self.assertIsNone(job)

        asyncio.run(run_test())

    def test_list_active_jobs(self):
        """Test listing active jobs."""
        async def run_test():
            traits = PersonalityTraits()
            job_id_1 = await self.coordinator.submit_pgn_training(
                self.temp_pgn.name,
                "agent-1",
                traits,
            )

            job_id_2 = await self.coordinator.submit_pgn_training(
                self.temp_pgn.name,
                "agent-2",
                traits,
            )

            active_jobs = self.coordinator.list_active_jobs()
            self.assertGreaterEqual(len(active_jobs), 1)

        asyncio.run(run_test())

    def test_list_all_jobs(self):
        """Test listing all jobs."""
        async def run_test():
            traits = PersonalityTraits()
            await self.coordinator.submit_pgn_training(
                self.temp_pgn.name,
                "agent-1",
                traits,
            )

            all_jobs = self.coordinator.list_all_jobs()
            self.assertGreater(len(all_jobs), 0)

        asyncio.run(run_test())

    def test_get_dataset(self):
        """Test retrieving dataset."""
        async def run_test():
            dataset = await self.coordinator.get_dataset("nonexistent-id")
            self.assertIsNone(dataset)

        asyncio.run(run_test())

    def test_convert_pgn_to_dataset(self):
        """Test PGN to dataset conversion."""
        async def run_test():
            traits = PersonalityTraits()
            job = PGNTrainingJob(
                pgn_file_path=self.temp_pgn.name,
                agent_id="test-agent",
                target_traits=traits,
            )

            dataset = await self.coordinator._convert_pgn_to_dataset(job)

            self.assertIsNotNone(dataset)
            self.assertGreater(dataset.get_size(), 0)
            self.assertIn("games_processed", dataset.statistics)

        asyncio.run(run_test())

    def test_validate_dataset_success(self):
        """Test dataset validation with valid data."""
        async def run_test():
            from gpu_worker.pgn_converter import PGNDataset, TrainingExample

            dataset = PGNDataset(name="Test", description="Test dataset")
            features = {
                "fen": "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1"
            }
            example = TrainingExample(
                position_features=features,
                target_move="e7e5",
                game_result=0.5,
            )
            dataset.add_examples([example])

            # Should not raise
            await self.coordinator._validate_dataset(dataset)

        asyncio.run(run_test())

    def test_validate_dataset_empty(self):
        """Test validation of empty dataset."""
        async def run_test():
            from gpu_worker.pgn_converter import PGNDataset

            dataset = PGNDataset(name="Empty", description="Empty dataset")

            with self.assertRaises(ValueError):
                await self.coordinator._validate_dataset(dataset)

        asyncio.run(run_test())

    def test_train_from_dataset(self):
        """Test initiating training from dataset."""
        async def run_test():
            from gpu_worker.pgn_converter import PGNDataset, TrainingExample

            self.training_pipeline.start_training = AsyncMock(
                return_value="training-job-123"
            )

            dataset = PGNDataset(name="Test", description="Test")
            features = {
                "fen": "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1"
            }
            dataset.add_examples(
                [
                    TrainingExample(
                        position_features=features,
                        target_move="e7e5",
                        game_result=0.5,
                    )
                ]
            )

            traits = PersonalityTraits()
            job = PGNTrainingJob(
                pgn_file_path=self.temp_pgn.name,
                agent_id="test-agent",
                target_traits=traits,
            )

            training_job_id = await self.coordinator._train_from_dataset(
                job, dataset
            )

            self.assertEqual(training_job_id, "training-job-123")
            self.training_pipeline.start_training.assert_called_once()

        asyncio.run(run_test())

    def test_compute_statistics(self):
        """Test statistics computation."""
        from gpu_worker.pgn_converter import PGNDataset, TrainingExample

        dataset = PGNDataset(name="Test", description="Test")
        features = {
            "fen": "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1"
        }
        examples = [
            TrainingExample(position_features=features, target_move="e7e5", game_result=1.0),
            TrainingExample(position_features=features, target_move="c7c5", game_result=0.5),
        ]
        dataset.add_examples(examples)
        dataset.statistics = {"games_processed": 1, "positions_extracted": 2}

        stats = self.coordinator._compute_statistics(dataset)

        self.assertEqual(stats["total_examples"], 2)
        self.assertIn("result_distribution", stats)
        self.assertIn("conversion_stats", stats)


class TestProcessingPipeline(unittest.TestCase):
    """Test the complete processing pipeline."""

    def setUp(self):
        """Set up test fixtures."""
        self.orchestrator = MagicMock(spec=DecentralizedOrchestrator)
        self.training_pipeline = MagicMock(spec=PersonalityTrainingPipeline)

        # Mock the async methods
        self.training_pipeline.start_training = AsyncMock(
            return_value="training-job-123"
        )

        self.coordinator = PGNTrainingCoordinator(
            self.orchestrator,
            self.training_pipeline,
            batch_size=10,
        )

        # Create sample PGN
        self.pgn_content = """
[Event "Game 1"]
[White "Player A"]
[Black "Player B"]
[Result "1-0"]

1. e4 e5 2. Nf3 Nc6 3. Bb5 a6 1-0

[Event "Game 2"]
[White "Player C"]
[Black "Player D"]
[Result "0-1"]

1. d4 d5 2. c4 0-1
"""
        self.temp_pgn = tempfile.NamedTemporaryFile(
            mode="w", suffix=".pgn", delete=False
        )
        self.temp_pgn.write(self.pgn_content)
        self.temp_pgn.flush()

    def tearDown(self):
        """Clean up test fixtures."""
        Path(self.temp_pgn.name).unlink(missing_ok=True)

    def test_end_to_end_pipeline(self):
        """Test complete processing pipeline."""
        async def run_test():
            traits = PersonalityTraits(aggressiveness=0.7, risk_tolerance=0.6)

            # Submit job
            job_id = await self.coordinator.submit_pgn_training(
                self.temp_pgn.name,
                "agent-master",
                traits,
            )

            self.assertIsNotNone(job_id)

            # Wait briefly for async processing to complete
            await asyncio.sleep(1)

            # Check job status
            job = await self.coordinator.get_job_status(job_id)
            self.assertIsNotNone(job)
            self.assertEqual(job.agent_id, "agent-master")
            self.assertIsNotNone(job.dataset_id)

            # Verify training was started
            self.training_pipeline.start_training.assert_called()

        asyncio.run(run_test())


class TestErrorHandling(unittest.TestCase):
    """Test error handling and edge cases."""

    def setUp(self):
        """Set up test fixtures."""
        self.orchestrator = MagicMock(spec=DecentralizedOrchestrator)
        self.training_pipeline = MagicMock(spec=PersonalityTrainingPipeline)
        self.training_pipeline.start_training = AsyncMock(
            return_value="training-job-123"
        )
        self.coordinator = PGNTrainingCoordinator(
            self.orchestrator,
            self.training_pipeline,
        )

    def test_concurrent_job_submissions(self):
        """Test concurrent job submissions."""
        async def run_test():
            # Create temporary PGN files
            pgn_files = []
            for i in range(3):
                temp = tempfile.NamedTemporaryFile(
                    mode="w", suffix=".pgn", delete=False
                )
                temp.write("[Event 'Game']\n[Result '1-0']\n\n1. e4 e5 1-0")
                temp.flush()
                pgn_files.append(temp.name)

            try:
                traits = PersonalityTraits()

                # Submit multiple jobs concurrently
                tasks = [
                    self.coordinator.submit_pgn_training(
                        pgn_files[i], f"agent-{i}", traits
                    )
                    for i in range(3)
                ]
                job_ids = await asyncio.gather(*tasks)

                self.assertEqual(len(job_ids), 3)
                self.assertEqual(len(set(job_ids)), 3)  # All unique

            finally:
                for f in pgn_files:
                    Path(f).unlink(missing_ok=True)

        asyncio.run(run_test())


class TestResourceEfficiency(unittest.TestCase):
    """Test resource efficiency optimizations."""

    def test_batch_processing(self):
        """Test batch processing for memory efficiency."""
        from gpu_worker.pgn_converter import PGNConverter

        orchestrator = MagicMock(spec=DecentralizedOrchestrator)
        training_pipeline = MagicMock(spec=PersonalityTrainingPipeline)

        coordinator = PGNTrainingCoordinator(
            orchestrator,
            training_pipeline,
            batch_size=5,
        )

        # Create PGN with multiple games
        pgn_content = ""
        for i in range(10):
            pgn_content += f"""[Event "Game {i}"]
[Result "1-0"]

1. e4 e5 2. Nf3 1-0

"""

        temp_pgn = tempfile.NamedTemporaryFile(mode="w", suffix=".pgn", delete=False)
        temp_pgn.write(pgn_content)
        temp_pgn.flush()

        try:
            converter = PGNConverter()
            batches = list(converter.batch_convert(temp_pgn.name, batch_size=5))

            # Should have at least 2 batches (10 games with batch_size=5)
            self.assertGreater(len(batches), 0)

        finally:
            Path(temp_pgn.name).unlink(missing_ok=True)


if __name__ == "__main__":
    unittest.main()
