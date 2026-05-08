from __future__ import annotations

import asyncio
import logging
import uuid
from datetime import datetime, timezone
from enum import Enum
from dataclasses import dataclass, field
from typing import Dict, Any, List, Optional, Callable

logger = logging.getLogger("XLMate.DeploymentPipeline")


class PipelineStage(Enum):
    """Stages in the deployment pipeline."""
    INITIALIZED = "initialized"
    VALIDATING = "validating"
    BUILDING = "building"
    TESTING = "testing"
    OPTIMIZING = "optimizing"
    DEPLOYING = "deploying"
    VERIFYING = "verifying"
    COMPLETED = "completed"
    FAILED = "failed"
    ROLLED_BACK = "rolled_back"


class DeploymentTarget(Enum):
    """Deployment target environments."""
    LOCAL = "local"
    TESTNET = "testnet"
    FUTURENET = "futurenet"
    MAINNET = "mainnet"


@dataclass
class PipelineConfig:
    """Configuration for a deployment pipeline."""
    engine_id: str
    target: DeploymentTarget = DeploymentTarget.LOCAL
    enable_rollback: bool = True
    run_tests: bool = True
    optimize_resources: bool = True
    max_retries: int = 3
    timeout_seconds: int = 300
    pre_deploy_hooks: List[Callable] = field(default_factory=list)
    post_deploy_hooks: List[Callable] = field(default_factory=list)


@dataclass
class PipelineResult:
    """Result of a deployment pipeline execution."""
    pipeline_id: str
    engine_id: str
    status: PipelineStage
    duration_ms: int = 0
    error_message: Optional[str] = None
    deployment_id: Optional[str] = None
    rollback_available: bool = False
    metrics: Dict[str, Any] = field(default_factory=dict)


class DeploymentPipelineOrchestrator:
    """
    Orchestrates the complete deployment pipeline for AI engines.
    Manages validation, building, testing, optimization, deployment, and verification.
    """

    def __init__(self):
        self._active_pipelines: Dict[str, PipelineResult] = {}
        self._deployment_history: Dict[str, List[PipelineResult]] = {}
        self._lock = asyncio.Lock()

    async def execute_pipeline(
        self,
        config: PipelineConfig
    ) -> PipelineResult:
        """
        Execute a complete deployment pipeline for an AI engine.
        
        Args:
            config: Pipeline configuration
            
        Returns:
            PipelineResult with execution status and metrics
        """
        pipeline_id = str(uuid.uuid4())
        logger.info(f"Starting deployment pipeline {pipeline_id} for engine {config.engine_id}")
        
        result = PipelineResult(
            pipeline_id=pipeline_id,
            engine_id=config.engine_id,
            status=PipelineStage.INITIALIZED
        )
        
        start_time = datetime.now(timezone.utc)
        
        async with self._lock:
            self._active_pipelines[pipeline_id] = result
        
        try:
            # Stage 1: Validation
            result.status = PipelineStage.VALIDATING
            logger.info(f"[{pipeline_id}] Stage: Validating engine configuration")
            await self._validate_engine(config)
            
            # Run pre-deployment hooks
            await self._execute_hooks(config.pre_deploy_hooks, "pre-deploy")
            
            # Stage 2: Building
            result.status = PipelineStage.BUILDING
            logger.info(f"[{pipeline_id}] Stage: Building engine artifacts")
            await self._build_engine(config)
            
            # Stage 3: Testing
            if config.run_tests:
                result.status = PipelineStage.TESTING
                logger.info(f"[{pipeline_id}] Stage: Running tests")
                await self._test_engine(config)
            
            # Stage 4: Optimization
            if config.optimize_resources:
                result.status = PipelineStage.OPTIMIZING
                logger.info(f"[{pipeline_id}] Stage: Optimizing resource allocation")
                await self._optimize_engine(config)
            
            # Stage 5: Deployment
            result.status = PipelineStage.DEPLOYING
            logger.info(f"[{pipeline_id}] Stage: Deploying to {config.target.value}")
            deployment_id = await self._deploy_engine(config)
            result.deployment_id = deployment_id
            result.rollback_available = config.enable_rollback
            
            # Stage 6: Verification
            result.status = PipelineStage.VERIFYING
            logger.info(f"[{pipeline_id}] Stage: Verifying deployment")
            await self._verify_deployment(config, deployment_id)
            
            # Run post-deployment hooks
            await self._execute_hooks(config.post_deploy_hooks, "post-deploy")
            
            # Success
            result.status = PipelineStage.COMPLETED
            end_time = datetime.now(timezone.utc)
            result.duration_ms = int((end_time - start_time).total_seconds() * 1000)
            result.metrics = await self._collect_deployment_metrics(config, deployment_id)
            
            logger.info(f"Pipeline {pipeline_id} completed successfully in {result.duration_ms}ms")
            
        except Exception as e:
            logger.error(f"Pipeline {pipeline_id} failed: {str(e)}")
            result.status = PipelineStage.FAILED
            result.error_message = str(e)
            end_time = datetime.now(timezone.utc)
            result.duration_ms = int((end_time - start_time).total_seconds() * 1000)
            
            # Attempt rollback if enabled
            if config.enable_rollback and result.deployment_id:
                logger.info(f"Attempting rollback for pipeline {pipeline_id}")
                try:
                    await self._rollback_deployment(config, result.deployment_id)
                    result.status = PipelineStage.ROLLED_BACK
                except Exception as rollback_error:
                    logger.error(f"Rollback failed for pipeline {pipeline_id}: {str(rollback_error)}")
        
        async with self._lock:
            del self._active_pipelines[pipeline_id]
            
            # Add to history
            if config.engine_id not in self._deployment_history:
                self._deployment_history[config.engine_id] = []
            self._deployment_history[config.engine_id].append(result)
        
        return result

    async def _validate_engine(self, config: PipelineConfig) -> None:
        """Validate engine configuration and dependencies."""
        # Simulate validation
        await asyncio.sleep(0.2)
        logger.debug(f"Validated engine {config.engine_id}")

    async def _build_engine(self, config: PipelineConfig) -> None:
        """Build engine artifacts (WASM, models, etc.)."""
        # Simulate build process
        await asyncio.sleep(0.3)
        logger.debug(f"Built engine {config.engine_id}")

    async def _test_engine(self, config: PipelineConfig) -> None:
        """Run engine tests."""
        # Simulate testing
        await asyncio.sleep(0.4)
        logger.debug(f"Tests passed for engine {config.engine_id}")

    async def _optimize_engine(self, config: PipelineConfig) -> None:
        """Optimize resource allocation and performance."""
        # Simulate optimization
        await asyncio.sleep(0.2)
        logger.debug(f"Optimized engine {config.engine_id}")

    async def _deploy_engine(self, config: PipelineConfig) -> str:
        """Deploy engine to target environment."""
        # Simulate deployment
        deployment_id = f"deploy-{uuid.uuid4().hex[:8]}"
        await asyncio.sleep(0.3)
        logger.debug(f"Deployed engine {config.engine_id} with ID {deployment_id}")
        return deployment_id

    async def _verify_deployment(self, config: PipelineConfig, deployment_id: str) -> None:
        """Verify successful deployment."""
        # Simulate verification
        await asyncio.sleep(0.2)
        logger.debug(f"Verified deployment {deployment_id}")

    async def _rollback_deployment(self, config: PipelineConfig, deployment_id: str) -> None:
        """Rollback a deployment."""
        # Simulate rollback
        await asyncio.sleep(0.3)
        logger.info(f"Rolled back deployment {deployment_id}")

    async def _execute_hooks(self, hooks: List[Callable], hook_type: str) -> None:
        """Execute pre or post deployment hooks."""
        for hook in hooks:
            try:
                if asyncio.iscoroutinefunction(hook):
                    await hook()
                else:
                    hook()
                logger.debug(f"Executed {hook_type} hook")
            except Exception as e:
                logger.warning(f"Hook execution failed ({hook_type}): {str(e)}")

    async def _collect_deployment_metrics(
        self,
        config: PipelineConfig,
        deployment_id: str
    ) -> Dict[str, Any]:
        """Collect deployment metrics."""
        return {
            "deployment_id": deployment_id,
            "target": config.target.value,
            "timestamp": datetime.now(timezone.utc).isoformat()
        }

    def get_pipeline_status(self, pipeline_id: str) -> Optional[PipelineResult]:
        """Get status of an active pipeline."""
        return self._active_pipelines.get(pipeline_id)

    def get_deployment_history(self, engine_id: Optional[str] = None) -> Dict:
        """
        Get deployment history.
        
        Args:
            engine_id: Optional engine ID to filter history
            
        Returns:
            Dictionary of deployment history
        """
        if engine_id:
            return {engine_id: self._deployment_history.get(engine_id, [])}
        return self._deployment_history.copy()

    def get_active_pipelines(self) -> Dict[str, PipelineResult]:
        """Get all active pipelines."""
        return self._active_pipelines.copy()
