from __future__ import annotations

import asyncio
import logging
from datetime import datetime, timezone
from typing import Dict, List, Optional

from gpu_worker.models import TrainingJob, TrainingStatus, PersonalityTraits
from gpu_worker.decentralized_orchestrator import DecentralizedOrchestrator
from gpu_worker.pgn_converter import PGNConverter

logger = logging.getLogger("XLMate.TrainingPipeline")

class PersonalityTrainingPipeline:
    """
    Manages the lifecycle of agent personality training jobs.
    Coordinates with DecentralizedOrchestrator for resource allocation.
    """

    def __init__(self, orchestrator: DecentralizedOrchestrator):
        self.orchestrator = orchestrator
        self.jobs: Dict[str, TrainingJob] = {}
        self._lock = asyncio.Lock()

    async def start_training(self, agent_id: str, target_traits: PersonalityTraits) -> str:
        """
        Initialize and queue a new training job.
        """
        job = TrainingJob(agent_id=agent_id, target_traits=target_traits)
        async with self._lock:
            self.jobs[job.job_id] = job
        
        logger.info(f"Queued training job {job.job_id} for agent {agent_id}")
        
        # Start background training task
        asyncio.create_task(self._run_training_job(job.job_id))
        
        return job.job_id

    async def get_job_status(self, job_id: str) -> Optional[TrainingJob]:
        """
        Return the current status of a training job.
        """
        return self.jobs.get(job_id)

    async def _run_training_job(self, job_id: str):
        """
        Simulate the training process: Queued -> Training -> Validating -> Completed.
        """
        job = self.jobs.get(job_id)
        if not job:
            return

        try:
            job.started_at = datetime.now(timezone.utc)
            job.node_id = self.orchestrator.node_id
            
            # Step 1: Data Preparation
            job.status = TrainingStatus.PROVISIONING
            logger.info(f"Job {job_id}: Preparing training data from PGN...")
            converter = PGNConverter()
            # In a real scenario, we would use job.pgn_path
            # For now, we simulate the time taken for conversion
            for p in range(0, 31, 10):
                job.progress = float(p)
                await asyncio.sleep(0.4)

            # Step 2: Training
            job.status = TrainingStatus.TRAINING
            logger.info(f"Job {job_id}: Training started on node {job.node_id}")
            for p in range(30, 81, 10):
                job.progress = float(p)
                await asyncio.sleep(0.5)  # Simulate GPU computation

            # Step 2: Validation
            job.status = TrainingStatus.VALIDATING
            logger.info(f"Job {job_id}: Validating personality traits...")
            for p in range(70, 101, 10):
                job.progress = float(p)
                await asyncio.sleep(0.3)

            # Step 3: Completion
            job.status = TrainingStatus.COMPLETED
            job.completed_at = datetime.now(timezone.utc)
            logger.info(f"Job {job_id}: Training completed successfully.")

        except Exception as e:
            logger.error(f"Job {job_id} failed: {str(e)}")
            job.status = TrainingStatus.FAILED
            job.completed_at = datetime.now(timezone.utc)

    def list_active_jobs(self) -> List[TrainingJob]:
        """
        List all active and completed training jobs.
        """
        return list(self.jobs.values())
