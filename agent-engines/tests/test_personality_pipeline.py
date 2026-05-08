import unittest
import asyncio
from gpu_worker.models import PersonalityTraits, TrainingStatus
from gpu_worker.personality import PersonalityManager
from gpu_worker.training_pipeline import PersonalityTrainingPipeline
from gpu_worker.decentralized_orchestrator import DecentralizedOrchestrator
from gpu_worker.pool import WorkerPool
from gpu_worker.config import WorkerConfig

class TestPersonalityPipeline(unittest.TestCase):
    def setUp(self):
        self.config = WorkerConfig()
        self.pool = WorkerPool([self.config])
        self.orchestrator = DecentralizedOrchestrator(self.pool)
        self.pipeline = PersonalityTrainingPipeline(self.orchestrator)
        self.loop = asyncio.new_event_loop()
        asyncio.set_event_loop(self.loop)

    def test_trait_mapping(self):
        traits = PersonalityTraits(aggressiveness=0.8, tone="aggressive")
        sf_options = PersonalityManager.traits_to_stockfish_options(traits)
        self.assertEqual(sf_options["Contempt"], 30)
        
        lc0_options = PersonalityManager.traits_to_lc0_options(traits)
        self.assertAlmostEqual(lc0_options["Cpuct"], 1.12)

    def test_training_lifecycle(self):
        async def run_test():
            traits = PersonalityTraits(aggressiveness=0.9)
            job_id = await self.pipeline.start_training("agent-123", traits)
            
            job = await self.pipeline.get_job_status(job_id)
            self.assertIsNotNone(job)
            self.assertEqual(job.agent_id, "agent-123")
            
            # Wait for training to complete (simulated)
            await asyncio.sleep(2)
            
            job = await self.pipeline.get_job_status(job_id)
            self.assertEqual(job.status, TrainingStatus.COMPLETED)
            self.assertEqual(job.progress, 100.0)
        
        self.loop.run_until_complete(run_test())

if __name__ == "__main__":
    unittest.main()
