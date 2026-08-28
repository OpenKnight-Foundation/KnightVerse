"""
Pytest suite for the Proof-of-Inference verification protocol added to
DecentralizedOrchestrator.

Run with:  pytest agent-engines/tests/test_decentralized_orchestrator.py -v

Uses unittest.IsolatedAsyncioTestCase (auto-discovered and run by pytest,
no extra plugin required) to match the async-heavy style of the existing
orchestrator test suite.
"""
import asyncio
import random
import unittest
from datetime import datetime, timezone

from gpu_worker.config import WorkerConfig
from gpu_worker.decentralized_orchestrator import (
    ChallengeVector,
    DecentralizedOrchestrator,
    NodeReputation,
)
from gpu_worker.models import AnalysisRequest, AnalysisResult, FullGameAnalysisRequest, NodeInfo
from gpu_worker.pool import WorkerPool


class FakeBridge:
    """A UCI bridge stand-in that always returns the same fixed analysis,
    regardless of the position it's given -- used to drive the local
    worker pool deterministically in tests."""

    def __init__(self, config) -> None:
        self.config = config
        self.started = False
        self.initialized = False
        self.positions = []
        self.quit_called = False

    async def start(self) -> None:
        self.started = True

    async def initialize_options(self) -> None:
        self.initialized = True

    async def set_position(self, fen: str) -> None:
        self.positions.append(fen)

    async def go(self, **_: object) -> tuple:
        from gpu_worker.uci_bridge import UciBestMove, UciInfo
        return UciBestMove(best_move="e2e4"), UciInfo(
            depth=20,
            evaluation=0.33,
            principal_variation=["e2e4", "e7e5"],
            nodes=2048,
        )

    async def quit(self) -> None:
        self.quit_called = True


def fake_worker_factory(cfg):
    from gpu_worker.worker import GPUAnalysisWorker
    from gpu_worker.resource_monitor import ResourceMonitor
    return GPUAnalysisWorker(
        cfg,
        bridge_factory=FakeBridge,
        resource_monitor=ResourceMonitor(),
    )


# A challenge vector calibrated to match FakeBridge's fixed response
# (best_move="e2e4", evaluation=0.33, depth=20), used to test the "honest
# node" path deterministically.
HONEST_MATCHING_VECTOR = ChallengeVector(
    vector_id="test-honest",
    fen="rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
    min_depth=10,
    eval_min=0.0,
    eval_max=1.0,
    allowed_best_moves=("e2e4",),
)

# A challenge vector whose bounds FakeBridge's fixed response can never
# satisfy, used to test the "rogue/faulty node" detection path.
IMPOSSIBLE_VECTOR = ChallengeVector(
    vector_id="test-impossible",
    fen="6k1/5ppp/8/8/8/8/5PPP/3R2K1 w - - 0 1",
    min_depth=10,
    eval_min=50.0,
    eval_max=100.0,
    allowed_best_moves=("d1d8",),
)


class TestDecentralizedOrchestrator(unittest.IsolatedAsyncioTestCase):
    async def asyncSetUp(self):
        self.config = WorkerConfig()
        self.pool = WorkerPool([self.config], worker_factory=fake_worker_factory)
        self.orchestrator = DecentralizedOrchestrator(
            self.pool,
            node_id="test-node",
            challenge_rate=0.0,  # deterministic per-test override below
            rng=random.Random(42),
        )

    async def asyncTearDown(self):
        await self.orchestrator.shutdown()

    async def _drain_background_challenges(self):
        # Fire-and-forget challenge tasks are scheduled with
        # asyncio.create_task; give the loop a couple of turns and then
        # wait for anything still tracked so assertions can run reliably.
        for _ in range(3):
            await asyncio.sleep(0)
        pending = list(self.orchestrator._challenge_tasks)
        if pending:
            await asyncio.gather(*pending, return_exceptions=True)

    # ------------------------------------------------------------
    # Baseline behaviour (unchanged from before PoI work)
    # ------------------------------------------------------------

    async def test_node_discovery(self):
        await self.orchestrator.start()
        peer = NodeInfo(
            node_id="peer-1", address="1.2.3.4", load=0.1,
            last_seen=datetime.now(timezone.utc),
        )
        await self.orchestrator.register_peer(peer)
        cluster = self.orchestrator.get_cluster_state()
        self.assertEqual(len(cluster), 2)
        self.assertEqual(cluster[1].node_id, "peer-1")

    async def test_load_balancing_dispatch(self):
        await self.orchestrator.start()
        peer_busy = NodeInfo(
            node_id="peer-busy", address="1.2.3.5", load=0.9,
            last_seen=datetime.now(timezone.utc),
        )
        await self.orchestrator.register_peer(peer_busy)
        request = AnalysisRequest(fen="rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1")
        result = await self.orchestrator.submit_task(request)
        self.assertEqual(result.request_id, request.id)

    async def test_shutdown_awaits_cancelled_health_check_task(self):
        await self.orchestrator.start()
        task = self.orchestrator._health_check_task
        self.assertIsNotNone(task)
        self.assertFalse(task.done())
        await self.orchestrator.shutdown()
        self.assertIsNone(self.orchestrator._health_check_task)
        self.assertTrue(task.done())
        self.assertTrue(task.cancelled())

    # ------------------------------------------------------------
    # Proof-of-Inference: honest node
    # ------------------------------------------------------------

    async def test_honest_local_node_passes_challenge_and_earns_credit(self):
        self.orchestrator.challenge_vectors = (HONEST_MATCHING_VECTOR,)
        self.orchestrator.challenge_rate = 1.0
        await self.orchestrator.start()

        request = AnalysisRequest(fen="rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1")
        result = await self.orchestrator.submit_task(request)
        self.assertEqual(result.request_id, request.id)  # user result unaffected

        await self._drain_background_challenges()

        rep = self.orchestrator.get_reputation("test-node")
        self.assertEqual(rep.challenges_passed, 1)
        self.assertEqual(rep.challenges_failed, 0)
        self.assertFalse(rep.credentials_revoked)
        self.assertGreater(rep.score, 100.0 - 1.0)  # score stayed high / increased

        proofs = self.orchestrator.get_verified_proofs("test-node")
        self.assertEqual(len(proofs), 1)
        self.assertEqual(proofs[0].vector_id, "test-honest")
        self.assertTrue(proofs[0].proof_hash)  # a hash was recorded

    async def test_challenge_hidden_from_full_game_analysis_results(self):
        """A hidden challenge folded into a batch must never appear in, or
        change the length/order of, the results returned to the caller."""
        self.orchestrator.challenge_vectors = (HONEST_MATCHING_VECTOR,)
        self.orchestrator.challenge_rate = 1.0
        await self.orchestrator.start()

        fens = [
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1",
            "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq e6 0 2",
        ]
        game_request = FullGameAnalysisRequest(fens=fens, depth=10, time_limit_ms=1000, priority=0)
        result = await self.orchestrator.submit_full_game_analysis(game_request)

        self.assertEqual(len(result.results), len(fens))
        returned_fens = {r.request_id for r in result.results}
        # The challenge request id must not leak into the user-visible results.
        self.assertEqual(len(returned_fens), len(fens))

        rep = self.orchestrator.get_reputation("test-node")
        self.assertEqual(rep.challenges_passed, 1)

    # ------------------------------------------------------------
    # Proof-of-Inference: rogue / faulty node detection
    # ------------------------------------------------------------

    async def test_rogue_remote_node_is_revoked_within_three_cycles(self):
        self.orchestrator.challenge_vectors = (IMPOSSIBLE_VECTOR,)
        self.orchestrator.challenge_rate = 1.0
        await self.orchestrator.start()

        rogue_peer = NodeInfo(
            node_id="rogue-1", address="9.9.9.9", status="online", load=0.0,
            last_seen=datetime.now(timezone.utc),
        )
        await self.orchestrator.register_peer(rogue_peer)

        # Force dispatch to the rogue peer by making it look idle relative
        # to local (real production code picks least-loaded node; we pin
        # local load high indirectly by always selecting the rogue node
        # via monkeypatched selection is unnecessary here since rogue has
        # load 0.0 and local starts at 0.0 too -- explicitly target it).
        async def run_challenge_against_rogue():
            await self.orchestrator._run_challenge(rogue_peer)

        for cycle in range(3):
            await run_challenge_against_rogue()

        rep = self.orchestrator.get_reputation("rogue-1")
        self.assertEqual(rep.challenges_failed, 3)
        self.assertTrue(rep.credentials_revoked)
        self.assertIsNotNone(rep.revoked_at)

        # Revoked node must be excluded from future dispatch consideration.
        cluster = self.orchestrator.get_cluster_state()
        cluster_ids = {n.node_id for n in cluster}
        self.assertNotIn("rogue-1", cluster_ids)

    async def test_erroring_node_is_penalized_without_raising(self):
        self.orchestrator.challenge_vectors = (HONEST_MATCHING_VECTOR,)
        await self.orchestrator.start()

        offline_peer = NodeInfo(
            node_id="offline-1", address="9.9.9.8", status="offline", load=0.0,
            last_seen=datetime.now(timezone.utc),
        )
        await self.orchestrator.register_peer(offline_peer)

        # _dispatch_to_remote raises RuntimeError for offline nodes; the
        # challenge runner must swallow this and record a failure rather
        # than propagating the exception.
        await self.orchestrator._run_challenge(offline_peer)

        rep = self.orchestrator.get_reputation("offline-1")
        self.assertEqual(rep.challenges_failed, 1)
        self.assertLess(rep.score, 100.0)

    async def test_user_task_unaffected_when_dispatch_node_later_fails_challenge(self):
        """Per acceptance criteria: a challenge failure must never fail the
        user's actual game request -- it only affects future eligibility."""
        self.orchestrator.challenge_vectors = (IMPOSSIBLE_VECTOR,)
        self.orchestrator.challenge_rate = 1.0
        await self.orchestrator.start()

        request = AnalysisRequest(fen="rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1")
        # This should complete successfully even though the background
        # challenge for the same node is guaranteed to fail its bounds check.
        result = await self.orchestrator.submit_task(request)
        self.assertEqual(result.request_id, request.id)
        self.assertEqual(result.best_move, "e2e4")

        await self._drain_background_challenges()
        rep = self.orchestrator.get_reputation("test-node")
        self.assertEqual(rep.challenges_failed, 1)

    async def test_no_challenge_when_rate_is_zero(self):
        self.orchestrator.challenge_vectors = (HONEST_MATCHING_VECTOR,)
        self.orchestrator.challenge_rate = 0.0
        await self.orchestrator.start()

        request = AnalysisRequest(fen="rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1")
        await self.orchestrator.submit_task(request)
        await self._drain_background_challenges()

        self.assertNotIn("test-node", self.orchestrator.reputations)

    async def test_verified_proof_hash_is_deterministic_and_bound_to_node(self):
        vector = HONEST_MATCHING_VECTOR
        result = AnalysisResult(
            request_id="r1", best_move="e2e4", evaluation=0.33, depth=20, time_ms=50
        )
        proof_a = self.orchestrator._build_proof("node-a", vector, result)
        proof_b = self.orchestrator._build_proof("node-b", vector, result)
        proof_a_again = self.orchestrator._build_proof("node-a", vector, result)

        self.assertEqual(proof_a.proof_hash, proof_a_again.proof_hash)
        self.assertNotEqual(proof_a.proof_hash, proof_b.proof_hash)


if __name__ == "__main__":
    unittest.main()