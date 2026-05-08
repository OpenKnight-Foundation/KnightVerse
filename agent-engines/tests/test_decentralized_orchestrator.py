import unittest
import asyncio
from datetime import datetime, timezone
from gpu_worker.decentralized_orchestrator import DecentralizedOrchestrator
from gpu_worker.pool import WorkerPool
from gpu_worker.config import WorkerConfig
from gpu_worker.models import AnalysisRequest, NodeInfo

class TestDecentralizedOrchestrator(unittest.TestCase):
    def setUp(self):
        self.config = WorkerConfig()
        self.pool = WorkerPool([self.config])
        self.orchestrator = DecentralizedOrchestrator(self.pool, node_id="test-node")
        self.loop = asyncio.new_event_loop()
        asyncio.set_event_loop(self.loop)

    def tearDown(self):
        self.loop.run_until_complete(self.orchestrator.shutdown())
        self.loop.close()

    def test_node_discovery(self):
        async def run_test():
            await self.orchestrator.start()
            peer = NodeInfo(
                node_id="peer-1",
                address="1.2.3.4",
                load=0.1,
                last_seen=datetime.now(timezone.utc)
            )
            await self.orchestrator.register_peer(peer)
            
            cluster = self.orchestrator.get_cluster_state()
            self.assertEqual(len(cluster), 2)
            self.assertEqual(cluster[1].node_id, "peer-1")
        
        self.loop.run_until_complete(run_test())

    def test_load_balancing_dispatch(self):
        async def run_test():
            await self.orchestrator.start()
            
            # Add a remote node with lower load than local (local load is 0 initially)
            # Actually local load is 0, let's make remote node very busy
            peer_busy = NodeInfo(
                node_id="peer-busy",
                address="1.2.3.5",
                load=0.9,
                last_seen=datetime.now(timezone.utc)
            )
            await self.orchestrator.register_peer(peer_busy)
            
            request = AnalysisRequest(fen="rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1")
            
            # Should dispatch locally since local load (0.0) < busy peer (0.9)
            result = await self.orchestrator.submit_task(request)
            self.assertEqual(result.request_id, request.id)
            
        self.loop.run_until_complete(run_test())

    def test_offloading(self):
        async def run_test():
            await self.orchestrator.start()
            
            # Mock local pool to be "busy" (simulate by adding a peer with lower load)
            peer_idle = NodeInfo(
                node_id="peer-idle",
                address="1.2.3.6",
                load=-1.0, # Impossible load but good for testing min()
                last_seen=datetime.now(timezone.utc)
            )
            await self.orchestrator.register_peer(peer_idle)
            
            request = AnalysisRequest(fen="rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1")
            
            # Should dispatch to peer-idle
            result = await self.orchestrator.submit_task(request)
            self.assertEqual(result.request_id, request.id)
            # The mocked result in _dispatch_to_remote returns 'e2e4'
            self.assertEqual(result.best_move, "e2e4")
            
        self.loop.run_until_complete(run_test())

if __name__ == "__main__":
    unittest.main()
