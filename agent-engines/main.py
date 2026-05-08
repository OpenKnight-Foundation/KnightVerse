from __future__ import annotations

import asyncio
import json
import logging
import uuid
from datetime import datetime, timezone
from enum import Enum
from dataclasses import dataclass, field, asdict
from typing import Dict, Any, List, Optional

from gpu_worker.config import WorkerConfig
from gpu_worker.pool import WorkerPool
from gpu_worker.decentralized_orchestrator import DecentralizedOrchestrator
from gpu_worker.models import AnalysisRequest, AnalysisResult, NodeInfo
from gpu_worker.resource_optimizer import ResourceOptimizer, ResourceTier, ResourceLimits
from gpu_worker.deployment_pipeline import (
    DeploymentPipelineOrchestrator,
    PipelineConfig,
    PipelineResult,
    DeploymentTarget
)

# Configure logging with a professional format
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger("XLMate.AgentEngine")

class EngineType(Enum):
    STOCKFISH = "stockfish"
    LEELA_CHESS_ZERO = "lc0"
    MAIA = "maia"
    CUSTOM = "custom"

class DeploymentStatus(Enum):
    QUEUED = "queued"
    PROVISIONING = "provisioning"
    OPTIMIZING = "optimizing"
    READY = "ready"
    TERMINATED = "terminated"

@dataclass
class EngineConfig:
    engine_type: EngineType
    threads: int = 1
    memory_mb: int = 256
    hash_size_mb: int = 64
    custom_params: Dict[str, Any] = field(default_factory=dict)

class AgentEngineOrchestrator:
    """
    Manages the lifecycle and deployment of AI co-pilot engines for XLMate.
    Utilizes DecentralizedOrchestrator for multi-node coordination.
    Features auto-scaling, load balancing, and resource optimization.
    """
    
    def __init__(self, node_id: Optional[str] = None):
        self.config = WorkerConfig()
        self.pool = WorkerPool([self.config])
        self.decentralized = DecentralizedOrchestrator(self.pool, node_id=node_id)
        self.resource_optimizer = ResourceOptimizer()
        self.deployment_pipeline = DeploymentPipelineOrchestrator()
        self._active_engines: Dict[str, Dict[str, Any]] = {}
        self._pipelines: Dict[str, DeploymentStatus] = {}
        self._resource_allocations: Dict[str, ResourceLimits] = {}

    async def start(self):
        """Start the orchestrator services."""
        await self.decentralized.start()

    async def shutdown(self):
        """Shutdown the orchestrator services."""
        await self.decentralized.shutdown()

    async def provision_engine(
        self,
        agent_id: str,
        config: EngineConfig,
        resource_tier: ResourceTier = ResourceTier.STANDARD
    ) -> bool:
        """
        Starts the deployment pipeline for a specific agent engine.
        Includes resource optimization and validation.
        """
        if agent_id in self._active_engines:
            logger.warning(f"Agent {agent_id} is already provisioned.")
            return False

        logger.info(f"Initializing deployment pipeline for Agent {agent_id} ({config.engine_type.value})")
        self._pipelines[agent_id] = DeploymentStatus.QUEUED
        
        try:
            # Step 0: Resource Optimization
            current_metrics = self.resource_optimizer.get_current_metrics()
            current_load = current_metrics.cpu_percent / 100.0
            
            optimal_limits = self.resource_optimizer.calculate_optimal_allocation(
                engine_id=agent_id,
                tier=resource_tier,
                current_load=current_load
            )
            
            if not self.resource_optimizer.validate_allocation(optimal_limits):
                logger.error(f"Resource allocation validation failed for {agent_id}")
                self._pipelines[agent_id] = DeploymentStatus.TERMINATED
                return False
            
            self._resource_allocations[agent_id] = optimal_limits
            
            # Step 1: Provisioning Resources
            self._pipelines[agent_id] = DeploymentStatus.PROVISIONING
            logger.info(f"[{agent_id}] Provisioning {optimal_limits.max_threads} threads and {optimal_limits.max_memory_mb}MB RAM...")
            await asyncio.sleep(0.5)  # Simulate non-blocking I/O

            # Step 2: Optimization
            self._pipelines[agent_id] = DeploymentStatus.OPTIMIZING
            logger.info(f"[{agent_id}] Optimizing engine parameters for resource efficiency...")
            await asyncio.sleep(0.5)

            # Step 3: Deployment Successful
            self._pipelines[agent_id] = DeploymentStatus.READY
            self._active_engines[agent_id] = {
                "config": asdict(config),
                "status": DeploymentStatus.READY.value,
                "metrics": {
                    "cpu_usage": 0.0,
                    "memory_usage": optimal_limits.max_memory_mb,
                    "tier": resource_tier.value
                },
                "resource_limits": {
                    "max_threads": optimal_limits.max_threads,
                    "max_memory_mb": optimal_limits.max_memory_mb,
                    "max_cpu_percent": optimal_limits.max_cpu_percent
                }
            }
            logger.info(f"Agent {agent_id} is now ONLINE and ready for inference.")
            return True

        except Exception as e:
            logger.error(f"Failed to deploy Agent {agent_id}: {str(e)}")
            self._pipelines[agent_id] = DeploymentStatus.TERMINATED
            return False

    def get_orchestration_state(self, agent_id: Optional[str] = None) -> Dict[str, Any]:
        """
        Returns the current state of the orchestrator or a specific agent.
        """
        cluster_state = self.decentralized.get_cluster_state()
        resource_metrics = self.resource_optimizer.get_current_metrics()
        
        if agent_id:
            return {
                "agent_id": agent_id,
                "pipeline_status": self._pipelines.get(agent_id, "unknown").value if agent_id in self._pipelines else "unknown",
                "engine_data": self._active_engines.get(agent_id),
                "resource_allocation": self._resource_allocations.get(agent_id),
                "cluster_info": [asdict(n) for n in cluster_state]
            }
        
        return {
            "node_id": self.decentralized.node_id,
            "active_count": len(self._active_engines),
            "agents": list(self._active_engines.keys()),
            "pipelines": {k: v.value for k, v in self._pipelines.items()},
            "cluster_nodes": len(cluster_state),
            "system_resources": {
                "cpu_percent": resource_metrics.cpu_percent,
                "memory_used_mb": resource_metrics.memory_used_mb,
                "memory_available_mb": resource_metrics.memory_available_mb
            }
        }

    def get_resource_metrics(self) -> Dict[str, Any]:
        """
        Get comprehensive resource utilization metrics.
        """
        metrics = self.resource_optimizer.get_current_metrics()
        capacity = self.resource_optimizer.get_system_capacity()
        
        return {
            "current": asdict(metrics),
            "capacity": capacity,
            "allocations": {
                agent_id: {
                    "threads": alloc.max_threads,
                    "memory_mb": alloc.max_memory_mb,
                    "cpu_percent": alloc.max_cpu_percent
                }
                for agent_id, alloc in self._resource_allocations.items()
            }
        }

    async def deploy_engine_with_pipeline(
        self,
        agent_id: str,
        config: EngineConfig,
        resource_tier: ResourceTier = ResourceTier.STANDARD,
        target: DeploymentTarget = DeploymentTarget.LOCAL
    ) -> PipelineResult:
        """
        Deploy an engine using the full deployment pipeline orchestrator.
        This includes validation, building, testing, optimization, and verification.
        """
        pipeline_config = PipelineConfig(
            engine_id=agent_id,
            target=target,
            enable_rollback=True,
            run_tests=True,
            optimize_resources=True
        )
        
        result = await self.deployment_pipeline.execute_pipeline(pipeline_config)
        
        # If pipeline succeeded, provision the engine
        if result.status.value in ["completed", "rolled_back"]:
            await self.provision_engine(agent_id, config, resource_tier)
        
        return result

async def main() -> None:
    """Start the decentralized agent engine orchestrator."""
    orchestrator = AgentEngineOrchestrator()
    await orchestrator.start()
    
    logger.info(f"Decentralized Engine Orchestrator started on node {orchestrator.decentralized.node_id}")
    
    try:
        await asyncio.Event().wait()
    except (KeyboardInterrupt, asyncio.CancelledError):
        await orchestrator.shutdown()
        logger.info("Orchestrator shut down")

async def run_demonstration():
    orchestrator = AgentEngineOrchestrator(node_id="demo-node")
    await orchestrator.start()
    
    # Define a high-performance configuration
    pro_config = EngineConfig(
        engine_type=EngineType.STOCKFISH,
        threads=4,
        memory_mb=1024,
        custom_params={"Skill Level": 20}
    )

    # Deploy multiple engines concurrently with different resource tiers
    logger.info("Starting concurrent deployment of AI co-pilots...")
    await asyncio.gather(
        orchestrator.provision_engine("copilot-alpha", pro_config, ResourceTier.HIGH),
        orchestrator.provision_engine("copilot-beta", EngineConfig(EngineType.MAIA), ResourceTier.STANDARD)
    )

    print("\n--- Final Orchestration State ---")
    print(json.dumps(orchestrator.get_orchestration_state(), indent=2))
    
    print("\n--- Resource Metrics ---")
    print(json.dumps(orchestrator.get_resource_metrics(), indent=2, default=str))
    
    await orchestrator.shutdown()

if __name__ == "__main__":
    import sys
    if len(sys.argv) > 1 and sys.argv[1] == "--demo":
        asyncio.run(run_demonstration())
    else:
        asyncio.run(main())
