"""
PGN Training Coordinator - Orchestrates PGN conversion and training integration.

This module coordinates the conversion of PGN files into training datasets
and integrates with the existing personality training pipeline.
"""

from __future__ import annotations

import asyncio
import logging
from datetime import datetime, timezone
from enum import Enum
from pathlib import Path
from typing import Any, Dict, List, Optional

from pydantic import BaseModel, Field

from gpu_worker.decentralized_orchestrator import DecentralizedOrchestrator
from gpu_worker.models import PersonalityTraits, TrainingJob, TrainingStatus
from gpu_worker.pgn_converter import (
    GameMetadata,
    PGNConverter,
    PGNDataset,
    TrainingExample,
)
from gpu_worker.training_pipeline import PersonalityTrainingPipeline

logger = logging.getLogger("XLMate.PGNTrainingCoordinator")


class PGNTrainingStatus(str, Enum):
    """Status of PGN training job."""

    QUEUED = "queued"
    CONVERTING = "converting"
    VALIDATING = "validating"
    TRAINING = "training"
    COMPLETED = "completed"
    FAILED = "failed"


class PGNTrainingJob(BaseModel):
    """
    Metadata for a PGN-based training job.

    Attributes:
        job_id: Unique identifier
        pgn_file_path: Path to PGN file
        agent_id: Target agent for training
        target_traits: Personality traits to train toward
        status: Current job status
        dataset_id: Reference to generated dataset
        progress: Progress percentage (0-100)
        started_at: Job start time
        completed_at: Job completion time
        statistics: Job statistics
    """

    job_id: str = Field(default_factory=lambda: __import__("uuid").uuid4().__str__())
    pgn_file_path: str
    agent_id: str
    target_traits: PersonalityTraits
    status: PGNTrainingStatus = PGNTrainingStatus.QUEUED
    dataset_id: Optional[str] = None
    progress: float = 0.0
    started_at: Optional[datetime] = None
    completed_at: Optional[datetime] = None
    statistics: Dict[str, Any] = Field(default_factory=dict)


class PGNTrainingCoordinator:
    """
    Coordinates PGN file conversion and neural network training.

    This coordinator manages the pipeline from raw PGN files to trained
    personality models, integrating with the DecentralizedOrchestrator.
    """

    def __init__(
        self,
        orchestrator: DecentralizedOrchestrator,
        training_pipeline: PersonalityTrainingPipeline,
        batch_size: int = 100,
    ):
        """
        Initialize the coordinator.

        Args:
            orchestrator: DecentralizedOrchestrator instance
            training_pipeline: PersonalityTrainingPipeline instance
            batch_size: Batch size for PGN processing
        """
        self.orchestrator = orchestrator
        self.training_pipeline = training_pipeline
        self.batch_size = batch_size
        self.jobs: Dict[str, PGNTrainingJob] = {}
        self.datasets: Dict[str, PGNDataset] = {}
        self._lock = asyncio.Lock()

    async def submit_pgn_training(
        self,
        pgn_file_path: str,
        agent_id: str,
        target_traits: PersonalityTraits,
    ) -> str:
        """
        Submit a PGN file for training.

        Args:
            pgn_file_path: Path to PGN file
            agent_id: Target agent identifier
            target_traits: Desired personality traits

        Returns:
            Job ID for tracking

        Raises:
            FileNotFoundError: If PGN file doesn't exist
            ValueError: If parameters are invalid
        """
        # Validate file exists
        file_path = Path(pgn_file_path)
        if not file_path.exists():
            raise FileNotFoundError(f"PGN file not found: {pgn_file_path}")

        # Create job
        job = PGNTrainingJob(
            pgn_file_path=str(file_path.absolute()),
            agent_id=agent_id,
            target_traits=target_traits,
        )

        async with self._lock:
            self.jobs[job.job_id] = job

        logger.info(
            f"Submitted PGN training job {job.job_id} for agent {agent_id} "
            f"from {pgn_file_path}"
        )

        # Start background processing
        asyncio.create_task(self._process_pgn_training(job.job_id))

        return job.job_id

    async def _process_pgn_training(self, job_id: str) -> None:
        """
        Process PGN training job end-to-end.

        Args:
            job_id: Job identifier
        """
        job = self.jobs.get(job_id)
        if not job:
            logger.error(f"Job not found: {job_id}")
            return

        try:
            job.status = PGNTrainingStatus.CONVERTING
            job.started_at = datetime.now(timezone.utc)
            logger.info(f"Starting conversion for job {job_id}")

            # Step 1: Convert PGN to dataset
            dataset = await self._convert_pgn_to_dataset(job)
            job.dataset_id = dataset.dataset_id
            job.progress = 33.0

            logger.info(
                f"Conversion complete for job {job_id}: "
                f"{len(dataset.examples)} examples extracted"
            )

            # Step 2: Validate dataset
            job.status = PGNTrainingStatus.VALIDATING
            job.progress = 66.0
            await self._validate_dataset(dataset)

            logger.info(f"Dataset validation passed for job {job_id}")

            # Step 3: Train personality model
            job.status = PGNTrainingStatus.TRAINING
            training_job_id = await self._train_from_dataset(job, dataset)
            job.progress = 99.0

            logger.info(
                f"Training initiated for job {job_id}, "
                f"training job: {training_job_id}"
            )

            # Step 4: Completion
            job.status = PGNTrainingStatus.COMPLETED
            job.progress = 100.0
            job.completed_at = datetime.now(timezone.utc)
            job.statistics = self._compute_statistics(dataset)

            logger.info(
                f"PGN training job {job_id} completed successfully. "
                f"Statistics: {job.statistics}"
            )

        except Exception as e:
            logger.error(f"PGN training job {job_id} failed: {e}")
            job.status = PGNTrainingStatus.FAILED
            job.completed_at = datetime.now(timezone.utc)

    async def _convert_pgn_to_dataset(self, job: PGNTrainingJob) -> PGNDataset:
        """
        Convert PGN file to training dataset.

        Args:
            job: PGNTrainingJob instance

        Returns:
            PGNDataset with training examples
        """
        converter = PGNConverter(
            skip_invalid_moves=True,
            extract_all_positions=True,
            min_move_number=5,  # Skip opening book-like moves
        )

        dataset = PGNDataset(
            name=f"Dataset from {Path(job.pgn_file_path).name}",
            description=f"Training data for agent {job.agent_id}",
        )

        # Process in batches for memory efficiency
        for batch in converter.batch_convert(job.pgn_file_path, self.batch_size):
            dataset.add_examples(batch)
            await asyncio.sleep(0)  # Yield to other tasks

        # Store metadata
        dataset.statistics = converter.get_statistics()

        async with self._lock:
            self.datasets[dataset.dataset_id] = dataset

        return dataset

    async def _validate_dataset(self, dataset: PGNDataset) -> None:
        """
        Validate dataset quality and completeness.

        Args:
            dataset: PGNDataset to validate

        Raises:
            ValueError: If validation fails
        """
        if len(dataset.examples) == 0:
            raise ValueError("Dataset is empty")

        # Check for valid FENs and moves
        for i, example in enumerate(dataset.examples[:100]):  # Sample validation
            features = example.position_features
            if not features.get("fen"):
                raise ValueError(f"Example {i} has no FEN")
            if not features.get("move_uci"):
                raise ValueError(f"Example {i} has no move")

            # Validate game result
            if not (0.0 <= example.game_result <= 1.0):
                raise ValueError(
                    f"Example {i} has invalid game result: {example.game_result}"
                )

        logger.info(f"Dataset {dataset.dataset_id} validation passed")

    async def _train_from_dataset(
        self, job: PGNTrainingJob, dataset: PGNDataset
    ) -> str:
        """
        Initiate personality training from dataset.

        Args:
            job: PGNTrainingJob instance
            dataset: PGNDataset with training examples

        Returns:
            Training job ID
        """
        # Submit to training pipeline
        training_job_id = await self.training_pipeline.start_training(
            job.agent_id, job.target_traits
        )

        logger.info(
            f"Started training job {training_job_id} for agent {job.agent_id} "
            f"from dataset {dataset.dataset_id}"
        )

        return training_job_id

    def _compute_statistics(self, dataset: PGNDataset) -> Dict[str, Any]:
        """
        Compute dataset statistics.

        Args:
            dataset: PGNDataset instance

        Returns:
            Dictionary with statistics
        """
        result_dist = dataset.get_result_distribution()
        return {
            "total_examples": dataset.get_size(),
            "result_distribution": result_dist,
            "conversion_stats": dataset.statistics,
        }

    async def get_job_status(self, job_id: str) -> Optional[PGNTrainingJob]:
        """
        Get status of a PGN training job.

        Args:
            job_id: Job identifier

        Returns:
            PGNTrainingJob or None if not found
        """
        return self.jobs.get(job_id)

    async def get_dataset(self, dataset_id: str) -> Optional[PGNDataset]:
        """
        Get a training dataset.

        Args:
            dataset_id: Dataset identifier

        Returns:
            PGNDataset or None if not found
        """
        return self.datasets.get(dataset_id)

    def list_active_jobs(self) -> List[PGNTrainingJob]:
        """
        List all active training jobs.

        Returns:
            List of PGNTrainingJob objects
        """
        return [
            job
            for job in self.jobs.values()
            if job.status not in (PGNTrainingStatus.COMPLETED, PGNTrainingStatus.FAILED)
        ]

    def list_all_jobs(self) -> List[PGNTrainingJob]:
        """
        List all training jobs.

        Returns:
            List of all PGNTrainingJob objects
        """
        return list(self.jobs.values())
