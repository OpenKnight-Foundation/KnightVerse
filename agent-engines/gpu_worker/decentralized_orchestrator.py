from __future__ import annotations

import asyncio
import logging
import uuid
from datetime import datetime, timezone
from typing import Dict, List, Optional

from gpu_worker.models import AnalysisRequest, AnalysisResult, NodeInfo
from gpu_worker.pool import WorkerPool

logger = logging.getLogger("XLMate.DecentralizedOrchestrator")

class DecentralizedOrchestrator:
    """
    Orchestrates AI engines across a decentralized network of nodes.
    Supports node discovery, load balancing, and fault tolerance.
    """

    def __init__(self, pool: WorkerPool, node_id: Optional[str] = None):
        self.node_id = node_id or str(uuid.uuid4())
        self.pool = pool
        self.peers: Dict[str, NodeInfo] = {}
        self._lock = asyncio.Lock()
        self._health_check_task: Optional[asyncio.Task] = None

    async def start(self):
        """Start the orchestrator and background tasks."""
        await self.pool.start_all()
        self._health_check_task = asyncio.create_task(self._health_check_loop())
        logger.info(f"Decentralized Orchestrator {self.node_id} started.")

    async def shutdown(self):
        """Shutdown the orchestrator and workers."""
        if self._health_check_task:
            self._health_check_task.cancel()
        await self.pool.shutdown_all()
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
        """Return the current state of all nodes in the cluster."""
        local_load = sum(w.load for w in self.pool._workers) / len(self.pool._workers) if self.pool._workers else 0.0
        local_node = NodeInfo(
            node_id=self.node_id,
            address="localhost",
            status="online",
            load=local_load,
            last_seen=datetime.now(timezone.utc)
        )
        return [local_node] + list(self.peers.values())

    async def submit_task(self, request: AnalysisRequest) -> AnalysisResult:
        """
        Submit an analysis task to the cluster.
        Dispatches to the least-loaded node (local or remote).
        """
        cluster = self.get_cluster_state()
        best_node = min(cluster, key=lambda n: n.load)

        if best_node.node_id == self.node_id:
            logger.debug(f"Executing task {request.id} locally.")
            return await self.pool.submit(request)
        else:
            logger.info(f"Offloading task {request.id} to remote node {best_node.node_id}.")
            return await self._dispatch_to_remote(best_node, request)

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
