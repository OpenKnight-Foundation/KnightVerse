from __future__ import annotations

import asyncio
import hashlib
import logging
import random
import uuid
from contextlib import suppress
from dataclasses import dataclass, field
from datetime import datetime, timezone
from typing import Dict, List, Optional, Sequence, Tuple

from gpu_worker.models import (
    AnalysisRequest,
    AnalysisResult,
    FullGameAnalysisRequest,
    FullGameAnalysisResult,
    NodeInfo,
)
from gpu_worker.pool import WorkerPool
from gpu_worker.opening_book import OpeningBook
from gpu_worker.redis_cache import RedisCache

logger = logging.getLogger("KnightVerse.DecentralizedOrchestrator")


# --------------------------------------------------------------------------
# Proof-of-Inference primitives
# --------------------------------------------------------------------------

@dataclass(frozen=True)
class ChallengeVector:
    """
    A hidden test position with known ground-truth bounds, used to verify
    that a worker node is actually running inference rather than returning
    fabricated / low-effort results while claiming compute rewards.

    NOTE: `eval_min`/`eval_max`/`min_depth`/allowed moves are intentionally
    *bounds*, not exact values -- different engine builds, hardware, and
    search parameters produce slightly different evaluations for the same
    position. These example vectors are illustrative placeholders; before
    production use they should be calibrated against the actual reference
    engine's output distribution (e.g. by running them offline on several
    trusted nodes and taking a tolerance band around the consensus result),
    and the vector pool should be rotated/expanded periodically so it can't
    be fingerprinted from a handful of observed challenges.
    """

    vector_id: str
    fen: str
    min_depth: int
    eval_min: float
    eval_max: float
    allowed_best_moves: Tuple[str, ...] = ()  # empty = any move accepted
    min_nodes: Optional[int] = None
    max_nodes: Optional[int] = None


# Illustrative default challenge pool. See calibration note above.
DEFAULT_CHALLENGE_VECTORS: Tuple[ChallengeVector, ...] = (
    ChallengeVector(
        vector_id="poi-startpos-book",
        fen="rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        min_depth=8,
        eval_min=-0.75,
        eval_max=0.75,
        allowed_best_moves=("e2e4", "d2d4", "g1f3", "c2c4"),
    ),
    ChallengeVector(
        vector_id="poi-rook-endgame-edge",
        fen="6k1/5ppp/8/8/8/8/5PPP/3R2K1 w - - 0 1",
        min_depth=8,
        eval_min=3.5,
        eval_max=20.0,
    ),
    ChallengeVector(
        vector_id="poi-material-down",
        fen="rnbqkb1r/pppp1ppp/5n2/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 1",
        min_depth=8,
        eval_min=-1.0,
        eval_max=1.0,
    ),
)

_CONSECUTIVE_FAILURES_BEFORE_REVOKE = 3
_DEFAULT_CHALLENGE_RATE = 0.05
_SCORE_ON_PASS = 2.0
_SCORE_PENALTY_ON_FAIL = 30.0
_SCORE_PENALTY_ON_ERROR = 15.0
_MIN_SCORE = 0.0
_MAX_SCORE = 100.0


@dataclass
class NodeReputation:
    """Tracks a node's Proof-of-Inference track record."""

    node_id: str
    score: float = _MAX_SCORE
    challenges_passed: int = 0
    challenges_failed: int = 0
    consecutive_failures: int = 0
    verified_proofs: int = 0
    credentials_revoked: bool = False
    last_challenge_at: Optional[datetime] = None
    revoked_at: Optional[datetime] = None


@dataclass
class VerifiedProof:
    """A record of a passed PoI challenge, suitable for export to an
    on-chain (or off-chain ledger) staking-reward distribution job."""

    node_id: str
    vector_id: str
    fen: str
    proof_hash: str
    evaluation: float
    depth: int
    time_ms: Optional[int]
    created_at: datetime = field(default_factory=lambda: datetime.now(timezone.utc))


class DecentralizedOrchestrator:
    """
    Orchestrates AI engines across a decentralized network of nodes.
    Supports node discovery, load balancing, fault tolerance, and
    Proof-of-Inference verification of worker responses.
    """

    def __init__(
        self,
        pool: WorkerPool,
        node_id: Optional[str] = None,
        opening_book_path: Optional[str] = None,
        redis_cache: Optional[RedisCache] = None,
        challenge_vectors: Optional[Sequence[ChallengeVector]] = None,
        challenge_rate: float = _DEFAULT_CHALLENGE_RATE,
        revoke_after_consecutive_failures: int = _CONSECUTIVE_FAILURES_BEFORE_REVOKE,
        rng: Optional[random.Random] = None,
    ):
        self.node_id = node_id or str(uuid.uuid4())
        self.pool = pool
        self.peers: Dict[str, NodeInfo] = {}
        self._lock = asyncio.Lock()
        self._health_check_task: Optional[asyncio.Task] = None
        self.opening_book = OpeningBook(opening_book_path) if opening_book_path else None
        self.redis_cache = redis_cache

        # --- Proof-of-Inference state ---
        # A cryptographically-seeded RNG is used (rather than the default
        # random module instance) so that challenge timing/selection is not
        # predictable from an attacker observing outputs over time. Tests
        # may inject a deterministic `random.Random` instance instead.
        self._rng = rng if rng is not None else random.SystemRandom()
        self.challenge_vectors: Tuple[ChallengeVector, ...] = tuple(
            challenge_vectors if challenge_vectors is not None else DEFAULT_CHALLENGE_VECTORS
        )
        self.challenge_rate = challenge_rate
        self.revoke_after_consecutive_failures = revoke_after_consecutive_failures
        self.reputations: Dict[str, NodeReputation] = {}
        self.verified_proofs: List[VerifiedProof] = []
        self._challenge_tasks: set[asyncio.Task] = set()

    async def start(self):
        """Start the orchestrator and background tasks."""
        await self.pool.start_all()
        self._health_check_task = asyncio.create_task(self._health_check_loop())
        logger.info(f"Decentralized Orchestrator {self.node_id} started.")

    async def shutdown(self, wait_for_pending: bool = True, timeout: float | None = 30):
        """Shutdown the orchestrator and workers.

        Args:
            wait_for_pending: Whether to wait for pending tasks to complete before shutdown
            timeout: Maximum time to wait for pending tasks in seconds
        """
        if self._health_check_task and not self._health_check_task.done():
            self._health_check_task.cancel()
            with suppress(asyncio.CancelledError):
                await self._health_check_task
            self._health_check_task = None

        if self._challenge_tasks:
            pending = list(self._challenge_tasks)
            if wait_for_pending:
                with suppress(asyncio.TimeoutError):
                    await asyncio.wait_for(
                        asyncio.gather(*pending, return_exceptions=True), timeout=timeout
                    )
            for task in pending:
                if not task.done():
                    task.cancel()
            self._challenge_tasks.clear()

        await self.pool.shutdown_all(wait_for_pending=wait_for_pending, timeout=timeout)
        logger.info(f"Decentralized Orchestrator {self.node_id} shut down.")

    async def register_peer(self, node: NodeInfo):
        """Register a new peer node in the cluster."""
        async with self._lock:
            self.peers[node.node_id] = node
            logger.info(f"Registered peer node: {node.node_id} at {node.address}")

    async def unregister_peer(self, node_id: str):
        """Remove a peer node from the cluster."""
        async with self._lock:
            if node_id in self.peers:
                del self.peers[node_id]
                logger.info(f"Unregistered peer node: {node_id}")

    async def update_peer_load(self, node_id: str, load: float):
        """Update the load and last_seen timestamp of a peer node."""
        async with self._lock:
            if node_id in self.peers:
                self.peers[node_id].load = load
                self.peers[node_id].last_seen = datetime.now(timezone.utc)
                logger.debug(f"Updated load for node {node_id}: {load}")

    def get_cluster_state(self) -> List[NodeInfo]:
        """Return the current state of all *eligible* nodes in the cluster.

        Nodes whose Proof-of-Inference credentials have been revoked are
        excluded from dispatch consideration so that live user requests are
        naturally routed away from dishonest/faulty nodes.
        """
        local_load = sum(w.load for w in self.pool._workers) / len(self.pool._workers) if self.pool._workers else 0.0
        local_node = NodeInfo(
            node_id=self.node_id,
            address="localhost",
            status="online",
            load=local_load,
            last_seen=datetime.now(timezone.utc)
        )
        all_nodes = [local_node] + list(self.peers.values())
        eligible = [n for n in all_nodes if not self.is_node_revoked(n.node_id)]
        # Never return a fully empty cluster (e.g. if local node were ever
        # flagged) -- fall back to the full node list so requests can still
        # be served rather than raising in min().
        return eligible or all_nodes

    # ----------------------------------------------------------------
    # Reputation / Proof-of-Inference accessors
    # ----------------------------------------------------------------

    def get_reputation(self, node_id: str) -> NodeReputation:
        """Return (creating if necessary) the reputation record for a node."""
        return self.reputations.setdefault(node_id, NodeReputation(node_id=node_id))

    def is_node_revoked(self, node_id: str) -> bool:
        rep = self.reputations.get(node_id)
        return bool(rep and rep.credentials_revoked)

    def get_verified_proofs(self, node_id: Optional[str] = None) -> List[VerifiedProof]:
        """Return recorded verified-computation proofs, optionally filtered
        to a single node. Intended to be consumed by an on-chain staking
        reward distribution job."""
        if node_id is None:
            return list(self.verified_proofs)
        return [p for p in self.verified_proofs if p.node_id == node_id]

    # ----------------------------------------------------------------
    # Task submission
    # ----------------------------------------------------------------

    async def submit_task(self, request: AnalysisRequest) -> AnalysisResult:
        """
        Submit an analysis task to the cluster.
        Dispatches to the least-loaded eligible node (local or remote).
        """
        # Check for a cached result first.
        if self.redis_cache:
            cache_key = f"analysis:{request.fen}:{request.depth}"
            cached_result = self.redis_cache.get(cache_key)
            if cached_result:
                logger.info(f"Returning cached result for task {request.id}.")
                return cached_result

        cluster = self.get_cluster_state()
        best_node = min(cluster, key=lambda n: n.load)

        if best_node.node_id == self.node_id:
            logger.debug(f"Executing task {request.id} locally.")
            # Pass the opening book to the worker if available.
            result = await self.pool.submit(request, opening_book=self.opening_book)
        else:
            logger.info(f"Offloading task {request.id} to remote node {best_node.node_id}.")
            result = await self._dispatch_to_remote(best_node, request)

        # Cache the result.
        if self.redis_cache:
            cache_key = f"analysis:{request.fen}:{request.depth}"
            self.redis_cache.set(cache_key, result, ttl=3600)

        # Opportunistically probe the node that served this task with a
        # hidden challenge. This runs fully in the background (fire-and-
        # forget) so it never adds latency to, or can fail, the user's
        # actual game request -- a dishonest response only affects the
        # node's reputation and future eligibility, never this response.
        if self.challenge_vectors and self._rng.random() < self.challenge_rate:
            task = asyncio.create_task(self._run_challenge(best_node))
            self._challenge_tasks.add(task)
            task.add_done_callback(self._challenge_tasks.discard)

        return result

    async def submit_full_game_analysis(
        self, request: FullGameAnalysisRequest
    ) -> FullGameAnalysisResult:
        """
        Submit a full game analysis task to the cluster.
        Splits the game's FENs into chunks and distributes them across available workers.

        With low probability, a hidden challenge position is folded into the
        dispatched batch. Its result is stripped out and verified before the
        response is returned to the caller, so it never surfaces to the user
        and never changes the order/length of the returned per-move results.
        """
        analysis_requests = [
            AnalysisRequest(
                fen=fen,
                depth=request.depth,
                time_limit_ms=request.time_limit_ms,
                priority=request.priority,
            )
            for fen in request.fens
        ]

        challenge_request: Optional[AnalysisRequest] = None
        challenge_vector: Optional[ChallengeVector] = None
        if self.challenge_vectors and self._rng.random() < self.challenge_rate:
            challenge_vector = self._select_challenge_vector()
            challenge_request = AnalysisRequest(
                fen=challenge_vector.fen,
                depth=max(challenge_vector.min_depth, request.depth),
                time_limit_ms=request.time_limit_ms,
                priority=request.priority,
            )
            # Insert at a random position so the challenge slot isn't
            # predictable from batch position alone.
            insert_at = self._rng.randrange(len(analysis_requests) + 1)
            analysis_requests.insert(insert_at, challenge_request)

        results = await self.pool.submit_batch(analysis_requests)

        if challenge_request is not None and challenge_vector is not None:
            challenge_result = next(
                (r for r in results if r.request_id == challenge_request.id), None
            )
            results = [r for r in results if r.request_id != challenge_request.id]
            if challenge_result is not None:
                # Batches are executed on the local pool; the local node id
                # is the one being verified here.
                self._verify_and_record(self.node_id, challenge_vector, challenge_result)
            else:
                logger.warning(
                    "PoI challenge %s was submitted but no matching result was returned.",
                    challenge_vector.vector_id,
                )

        return FullGameAnalysisResult(request_id=request.id, results=results)

    async def _dispatch_to_remote(self, node: NodeInfo, request: AnalysisRequest) -> AnalysisResult:
        """
        Simulate dispatching a task to a remote node.
        In a real implementation, this would involve a network call.
        """
        # For simulation purposes, we'll just wait a bit and return a mocked result
        # or fail if the node is "offline".
        await asyncio.sleep(0.1)
        if node.status != "online":
            raise RuntimeError(f"Remote node {node.node_id} is offline.")

        # Simulate remote execution result
        return AnalysisResult(
            request_id=request.id,
            best_move="e2e4",  # Dummy move
            evaluation=0.5,
            depth=20,
            time_ms=100
        )

    # ----------------------------------------------------------------
    # Proof-of-Inference challenge protocol
    # ----------------------------------------------------------------

    def _select_challenge_vector(self) -> ChallengeVector:
        return self._rng.choice(self.challenge_vectors)

    async def _run_challenge(self, node: NodeInfo) -> None:
        """Dispatch a single hidden challenge to `node` and verify the
        response. Never raises -- any failure (execution error, timeout,
        bad response) is treated as a challenge failure for reputation
        purposes and is fully isolated from user-facing task flows."""
        vector = self._select_challenge_vector()
        request = AnalysisRequest(
            fen=vector.fen,
            depth=max(vector.min_depth, 12),
            time_limit_ms=3000,
            priority=0,
        )
        try:
            if node.node_id == self.node_id:
                # Deliberately bypass the opening book so the worker is
                # forced to perform genuine search rather than a lookup,
                # even on well-known positions.
                result = await self.pool.submit(request, opening_book=None)
            else:
                result = await self._dispatch_to_remote(node, request)
        except Exception as exc:
            logger.warning(
                "PoI challenge %s to node %s errored: %s", vector.vector_id, node.node_id, exc
            )
            self._record_failure(node.node_id, penalty=_SCORE_PENALTY_ON_ERROR)
            return

        self._verify_and_record(node.node_id, vector, result)

    def _check_bounds(self, vector: ChallengeVector, result: AnalysisResult) -> bool:
        """Check a worker's response against the mathematical bounds of the
        hidden challenge vector (evaluation window, minimum search depth,
        and, when known, allowed best moves / search-node count range)."""
        if not (vector.eval_min <= result.evaluation <= vector.eval_max):
            return False
        if result.depth < vector.min_depth:
            return False
        if vector.allowed_best_moves and result.best_move not in vector.allowed_best_moves:
            return False
        nodes_searched = getattr(result, "nodes", None)
        if nodes_searched is not None:
            if vector.min_nodes is not None and nodes_searched < vector.min_nodes:
                return False
            if vector.max_nodes is not None and nodes_searched > vector.max_nodes:
                return False
        return True

    def _build_proof(
        self, node_id: str, vector: ChallengeVector, result: AnalysisResult
    ) -> VerifiedProof:
        """Build a compact, verifiable proof-of-computation record binding
        the node, the (hidden) challenge, and its response together via a
        hash -- suitable for later on-chain staking reward distribution."""
        nodes_searched = getattr(result, "nodes", "")
        payload = "|".join(
            str(x)
            for x in (
                node_id,
                vector.vector_id,
                vector.fen,
                result.best_move,
                result.evaluation,
                result.depth,
                getattr(result, "time_ms", ""),
                nodes_searched,
            )
        )
        proof_hash = hashlib.sha256(payload.encode("utf-8")).hexdigest()
        return VerifiedProof(
            node_id=node_id,
            vector_id=vector.vector_id,
            fen=vector.fen,
            proof_hash=proof_hash,
            evaluation=result.evaluation,
            depth=result.depth,
            time_ms=getattr(result, "time_ms", None),
        )

    def _record_pass(self, node_id: str, vector: ChallengeVector, result: AnalysisResult) -> None:
        rep = self.get_reputation(node_id)
        rep.challenges_passed += 1
        rep.consecutive_failures = 0
        rep.score = min(_MAX_SCORE, rep.score + _SCORE_ON_PASS)
        rep.last_challenge_at = datetime.now(timezone.utc)

        proof = self._build_proof(node_id, vector, result)
        rep.verified_proofs += 1
        self.verified_proofs.append(proof)
        logger.info(
            "Node %s passed PoI challenge %s (score=%.1f, verified_proofs=%d)",
            node_id, vector.vector_id, rep.score, rep.verified_proofs,
        )

    def _record_failure(self, node_id: str, penalty: float = _SCORE_PENALTY_ON_FAIL) -> None:
        rep = self.get_reputation(node_id)
        rep.challenges_failed += 1
        rep.consecutive_failures += 1
        rep.score = max(_MIN_SCORE, rep.score - penalty)
        rep.last_challenge_at = datetime.now(timezone.utc)

        logger.warning(
            "Node %s failed PoI challenge (score=%.1f, consecutive_failures=%d)",
            node_id, rep.score, rep.consecutive_failures,
        )

        if (
            rep.consecutive_failures >= self.revoke_after_consecutive_failures
            or rep.score <= _MIN_SCORE
        ):
            self._revoke_node(node_id)

    def _revoke_node(self, node_id: str) -> None:
        rep = self.get_reputation(node_id)
        if rep.credentials_revoked:
            return
        rep.credentials_revoked = True
        rep.revoked_at = datetime.now(timezone.utc)
        if node_id in self.peers:
            self.peers[node_id].status = "revoked"
        logger.error(
            "Node %s credentials REVOKED after %d consecutive PoI failures (score=%.1f).",
            node_id, rep.consecutive_failures, rep.score,
        )

    def _verify_and_record(
        self, node_id: str, vector: ChallengeVector, result: AnalysisResult
    ) -> bool:
        passed = self._check_bounds(vector, result)
        if passed:
            self._record_pass(node_id, vector, result)
        else:
            self._record_failure(node_id)
        return passed

    async def _health_check_loop(self):
        """Periodically check the health of peer nodes."""
        try:
            while True:
                await asyncio.sleep(10)
                async with self._lock:
                    now = datetime.now(timezone.utc)
                    expired_nodes = [
                        nid for nid, node in self.peers.items()
                        if (now - node.last_seen).total_seconds() > 30
                    ]
                    for nid in expired_nodes:
                        logger.warning(f"Node {nid} timed out. Removing from cluster.")
                        del self.peers[nid]
        except asyncio.CancelledError:
            pass