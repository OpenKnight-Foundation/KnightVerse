import unittest
import asyncio
from main import AgentEngineOrchestrator, EngineConfig, EngineType, DeploymentStatus

class TestAgentEngineOrchestrator(unittest.TestCase):
    def setUp(self):
        self.orchestrator = AgentEngineOrchestrator()
        self.loop = asyncio.new_event_loop()
        asyncio.set_event_loop(self.loop)

    def tearDown(self):
        self.loop.close()

    def test_successful_provisioning(self):
        async def run_test():
            config = EngineConfig(EngineType.STOCKFISH)
            success = await self.orchestrator.provision_engine("test-agent", config)
            self.assertTrue(success)
            state = self.orchestrator.get_orchestration_state("test-agent")
            self.assertEqual(state["pipeline_status"], "ready")
            self.assertIn("test-agent", self.orchestrator.get_orchestration_state()["agents"])
        
        self.loop.run_until_complete(run_test())

    def test_duplicate_provisioning_prevention(self):
        async def run_test():
            config = EngineConfig(EngineType.CUSTOM)
            await self.orchestrator.provision_engine("dup-agent", config)
            success = await self.orchestrator.provision_engine("dup-agent", config)
            self.assertFalse(success)
        
        self.loop.run_until_complete(run_test())

    def test_concurrent_orchestration(self):
        async def run_test():
            configs = [
                self.orchestrator.provision_engine("a1", EngineConfig(EngineType.MAIA)),
                self.orchestrator.provision_engine("a2", EngineConfig(EngineType.LEELA_CHESS_ZERO))
            ]
            results = await asyncio.gather(*configs)
            self.assertTrue(all(results))
            state = self.orchestrator.get_orchestration_state()
            self.assertEqual(state["active_count"], 2)
        
        self.loop.run_until_complete(run_test())

if __name__ == "__main__":
    unittest.main()
