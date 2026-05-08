import unittest
import asyncio
from unittest.mock import Mock, patch
from main import AgentEngineOrchestrator, EngineConfig, EngineType
from gpu_worker.resource_optimizer import ResourceOptimizer, ResourceTier, ResourceLimits
from gpu_worker.deployment_pipeline import (
    DeploymentPipelineOrchestrator,
    PipelineConfig,
    DeploymentTarget
)


class TestResourceOptimizer(unittest.TestCase):
    """Test suite for ResourceOptimizer."""

    def setUp(self):
        self.optimizer = ResourceOptimizer()

    def test_get_system_capacity(self):
        """Test system capacity calculation."""
        capacity = self.optimizer.get_system_capacity()
        
        self.assertIn("cpu_cores", capacity)
        self.assertIn("total_memory_mb", capacity)
        self.assertIn("available_cpu_percent", capacity)
        self.assertIn("available_memory_mb", capacity)
        self.assertGreater(capacity["cpu_cores"], 0)
        self.assertGreater(capacity["total_memory_mb"], 0)

    def test_get_current_metrics(self):
        """Test current metrics collection."""
        metrics = self.optimizer.get_current_metrics()
        
        self.assertIsNotNone(metrics)
        self.assertGreaterEqual(metrics.cpu_percent, 0)
        self.assertGreaterEqual(metrics.memory_used_mb, 0)
        self.assertGreaterEqual(metrics.memory_available_mb, 0)

    def test_calculate_optimal_allocation_light_tier(self):
        """Test resource allocation for light tier."""
        limits = self.optimizer.calculate_optimal_allocation(
            engine_id="test-engine-1",
            tier=ResourceTier.LIGHT,
            current_load=0.5
        )
        
        self.assertIsInstance(limits, ResourceLimits)
        self.assertEqual(limits.tier, ResourceTier.LIGHT)
        self.assertGreater(limits.max_threads, 0)
        self.assertGreater(limits.max_memory_mb, 0)

    def test_calculate_optimal_allocation_high_tier(self):
        """Test resource allocation for high tier."""
        limits = self.optimizer.calculate_optimal_allocation(
            engine_id="test-engine-2",
            tier=ResourceTier.HIGH,
            current_load=0.5
        )
        
        # High tier should get more resources than light tier
        light_limits = self.optimizer.calculate_optimal_allocation(
            engine_id="test-engine-3",
            tier=ResourceTier.LIGHT,
            current_load=0.5
        )
        
        self.assertGreaterEqual(limits.max_threads, light_limits.max_threads)
        self.assertGreaterEqual(limits.max_memory_mb, light_limits.max_memory_mb)

    def test_allocation_under_heavy_load(self):
        """Test resource allocation under heavy system load."""
        limits_heavy = self.optimizer.calculate_optimal_allocation(
            engine_id="test-engine-4",
            tier=ResourceTier.STANDARD,
            current_load=0.9
        )
        
        limits_light = self.optimizer.calculate_optimal_allocation(
            engine_id="test-engine-5",
            tier=ResourceTier.STANDARD,
            current_load=0.2
        )
        
        # Under heavy load, should allocate fewer resources
        self.assertLessEqual(limits_heavy.max_threads, limits_light.max_threads)
        self.assertLessEqual(limits_heavy.max_memory_mb, limits_light.max_memory_mb)

    def test_validate_allocation(self):
        """Test allocation validation."""
        limits = ResourceLimits(
            max_threads=2,
            max_memory_mb=512,
            max_cpu_percent=50.0
        )
        
        # Should be valid for reasonable limits
        self.assertTrue(self.optimizer.validate_allocation(limits))

    def test_estimate_gas_cost_stockfish(self):
        """Test gas cost estimation for Stockfish."""
        cost = self.optimizer.estimate_gas_cost("stockfish", 20)
        self.assertGreater(cost, 0)
        
        # Higher depth should cost more
        cost_deeper = self.optimizer.estimate_gas_cost("stockfish", 30)
        self.assertGreater(cost_deeper, cost)

    def test_estimate_gas_cost_lc0(self):
        """Test gas cost estimation for LC0 (should be higher than Stockfish)."""
        stockfish_cost = self.optimizer.estimate_gas_cost("stockfish", 20)
        lc0_cost = self.optimizer.estimate_gas_cost("lc0", 20)
        
        # LC0 is GPU-based and should be more expensive
        self.assertGreater(lc0_cost, stockfish_cost)

    def test_should_throttle(self):
        """Test throttling decision."""
        from gpu_worker.resource_optimizer import ResourceMetrics
        
        # High CPU usage should trigger throttling
        high_cpu_metrics = ResourceMetrics(cpu_percent=95.0, memory_used_mb=1024)
        self.assertTrue(self.optimizer.should_throttle(high_cpu_metrics, threshold=90.0))
        
        # Low CPU usage should not trigger throttling
        low_cpu_metrics = ResourceMetrics(cpu_percent=30.0, memory_used_mb=1024)
        self.assertFalse(self.optimizer.should_throttle(low_cpu_metrics, threshold=90.0))

    def test_allocation_history(self):
        """Test allocation history tracking."""
        self.optimizer.calculate_optimal_allocation("engine-1", ResourceTier.STANDARD)
        self.optimizer.calculate_optimal_allocation("engine-1", ResourceTier.HIGH)
        self.optimizer.calculate_optimal_allocation("engine-2", ResourceTier.LIGHT)
        
        history = self.optimizer.get_allocation_history()
        self.assertIn("engine-1", history)
        self.assertIn("engine-2", history)
        self.assertEqual(len(history["engine-1"]), 2)
        self.assertEqual(len(history["engine-2"]), 1)


class TestDeploymentPipelineOrchestrator(unittest.TestCase):
    """Test suite for DeploymentPipelineOrchestrator."""

    def setUp(self):
        self.orchestrator = DeploymentPipelineOrchestrator()

    def test_execute_pipeline_success(self):
        """Test successful pipeline execution."""
        async def run_test():
            config = PipelineConfig(
                engine_id="test-engine",
                target=DeploymentTarget.LOCAL,
                enable_rollback=True,
                run_tests=True
            )
            
            result = await self.orchestrator.execute_pipeline(config)
            
            self.assertIsNotNone(result)
            self.assertEqual(result.engine_id, "test-engine")
            self.assertEqual(result.status.value, "completed")
            self.assertIsNotNone(result.deployment_id)
            self.assertTrue(result.rollback_available)
            self.assertGreater(result.duration_ms, 0)
        
        asyncio.get_event_loop().run_until_complete(run_test())

    def test_execute_pipeline_with_hooks(self):
        """Test pipeline execution with pre and post deploy hooks."""
        async def run_test():
            pre_hook_called = False
            post_hook_called = False
            
            def pre_hook():
                nonlocal pre_hook_called
                pre_hook_called = True
            
            def post_hook():
                nonlocal post_hook_called
                post_hook_called = True
            
            config = PipelineConfig(
                engine_id="test-engine-hooks",
                target=DeploymentTarget.LOCAL,
                pre_deploy_hooks=[pre_hook],
                post_deploy_hooks=[post_hook]
            )
            
            result = await self.orchestrator.execute_pipeline(config)
            
            self.assertEqual(result.status.value, "completed")
            self.assertTrue(pre_hook_called)
            self.assertTrue(post_hook_called)
        
        asyncio.get_event_loop().run_until_complete(run_test())

    def test_execute_pipeline_no_tests(self):
        """Test pipeline execution without tests."""
        async def run_test():
            config = PipelineConfig(
                engine_id="test-engine-no-tests",
                target=DeploymentTarget.LOCAL,
                run_tests=False
            )
            
            result = await self.orchestrator.execute_pipeline(config)
            
            self.assertEqual(result.status.value, "completed")
        
        asyncio.get_event_loop().run_until_complete(run_test())

    def test_deployment_history(self):
        """Test deployment history tracking."""
        async def run_test():
            config1 = PipelineConfig(engine_id="engine-1", target=DeploymentTarget.LOCAL)
            config2 = PipelineConfig(engine_id="engine-1", target=DeploymentTarget.TESTNET)
            config3 = PipelineConfig(engine_id="engine-2", target=DeploymentTarget.LOCAL)
            
            await self.orchestrator.execute_pipeline(config1)
            await self.orchestrator.execute_pipeline(config2)
            await self.orchestrator.execute_pipeline(config3)
            
            history = self.orchestrator.get_deployment_history()
            self.assertIn("engine-1", history)
            self.assertIn("engine-2", history)
            self.assertEqual(len(history["engine-1"]), 2)
            self.assertEqual(len(history["engine-2"]), 1)
            
            # Filtered history
            filtered = self.orchestrator.get_deployment_history("engine-1")
            self.assertEqual(len(filtered["engine-1"]), 2)
        
        asyncio.get_event_loop().run_until_complete(run_test())

    def test_get_active_pipelines(self):
        """Test active pipeline tracking."""
        async def run_test():
            config = PipelineConfig(engine_id="test-engine-active")
            
            # After completion, pipeline should not be active
            result = await self.orchestrator.execute_pipeline(config)
            active = self.orchestrator.get_active_pipelines()
            
            self.assertNotIn(result.pipeline_id, active)
        
        asyncio.get_event_loop().run_until_complete(run_test())


class TestAgentEngineOrchestratorIntegration(unittest.TestCase):
    """Integration tests for AgentEngineOrchestrator with new features."""

    def setUp(self):
        self.orchestrator = AgentEngineOrchestrator(node_id="test-node")

    def test_provision_engine_with_resource_tier(self):
        """Test engine provisioning with resource tier."""
        async def run_test():
            config = EngineConfig(
                engine_type=EngineType.STOCKFISH,
                threads=2,
                memory_mb=512
            )
            
            result = await self.orchestrator.provision_engine(
                "test-engine-1",
                config,
                ResourceTier.STANDARD
            )
            
            self.assertTrue(result)
            
            state = self.orchestrator.get_orchestration_state("test-engine-1")
            self.assertEqual(state["pipeline_status"], "ready")
            self.assertIn("resource_allocation", state)
        
        asyncio.get_event_loop().run_until_complete(run_test())

    def test_get_resource_metrics(self):
        """Test resource metrics collection."""
        async def run_test():
            # Provision an engine first
            config = EngineConfig(engine_type=EngineType.STOCKFISH)
            await self.orchestrator.provision_engine("test-engine-metrics", config)
            
            metrics = self.orchestrator.get_resource_metrics()
            
            self.assertIn("current", metrics)
            self.assertIn("capacity", metrics)
            self.assertIn("allocations", metrics)
            self.assertIn("test-engine-metrics", metrics["allocations"])
        
        asyncio.get_event_loop().run_until_complete(run_test())

    def test_deploy_engine_with_pipeline(self):
        """Test full pipeline deployment."""
        async def run_test():
            config = EngineConfig(
                engine_type=EngineType.MAIA,
                threads=2,
                memory_mb=512
            )
            
            result = await self.orchestrator.deploy_engine_with_pipeline(
                "test-pipeline-engine",
                config,
                ResourceTier.STANDARD,
                DeploymentTarget.LOCAL
            )
            
            self.assertEqual(result.status.value, "completed")
            self.assertIsNotNone(result.deployment_id)
        
        asyncio.get_event_loop().run_until_complete(run_test())

    def test_concurrent_provisioning_with_different_tiers(self):
        """Test concurrent engine provisioning with different resource tiers."""
        async def run_test():
            config_light = EngineConfig(engine_type=EngineType.STOCKFISH)
            config_high = EngineConfig(engine_type=EngineType.MAIA, threads=4, memory_mb=1024)
            
            results = await asyncio.gather(
                self.orchestrator.provision_engine("light-engine", config_light, ResourceTier.LIGHT),
                self.orchestrator.provision_engine("high-engine", config_high, ResourceTier.HIGH)
            )
            
            self.assertTrue(all(results))
            
            state = self.orchestrator.get_orchestration_state()
            self.assertEqual(state["active_count"], 2)
            
            # Verify different resource allocations
            light_alloc = state.get("agents", [])
            self.assertIn("light-engine", light_alloc)
            self.assertIn("high-engine", light_alloc)
        
        asyncio.get_event_loop().run_until_complete(run_test())


if __name__ == "__main__":
    unittest.main()
