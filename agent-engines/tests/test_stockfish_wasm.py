"""Comprehensive tests for Stockfish WASM integration."""

import unittest
import asyncio
from unittest.mock import AsyncMock, MagicMock, patch

from gpu_worker.stockfish_wasm import (
    StockfishWASMEngine,
    WASMEngineConfig,
    WASMEngineStatus,
    WASMAnalysisResult,
)


class TestWASMEngineConfig(unittest.TestCase):
    """Test WASM engine configuration."""
    
    def test_default_config(self):
        """Test default configuration values."""
        config = WASMEngineConfig()
        
        self.assertEqual(config.wasm_path, "stockfish.wasm")
        self.assertEqual(config.js_bridge_path, "stockfish.js")
        self.assertEqual(config.threads, 1)
        self.assertEqual(config.hash_size_mb, 16)
        self.assertEqual(config.skill_level, 20)
        self.assertEqual(config.default_depth, 18)
        self.assertEqual(config.default_time_limit_ms, 3000)
        self.assertTrue(config.use_shared_array_buffer)
    
    def test_custom_config(self):
        """Test custom configuration."""
        config = WASMEngineConfig(
            wasm_path="custom.wasm",
            threads=4,
            hash_size_mb=64,
            skill_level=15,
            default_depth=20,
        )
        
        self.assertEqual(config.wasm_path, "custom.wasm")
        self.assertEqual(config.threads, 4)
        self.assertEqual(config.hash_size_mb, 64)
        self.assertEqual(config.skill_level, 15)
        self.assertEqual(config.default_depth, 20)
    
    def test_config_to_dict(self):
        """Test configuration serialization."""
        config = WASMEngineConfig(
            threads=2,
            hash_size_mb=32,
        )
        
        config_dict = config.to_dict()
        
        self.assertIsInstance(config_dict, dict)
        self.assertEqual(config_dict["threads"], 2)
        self.assertEqual(config_dict["hash_size_mb"], 32)
        self.assertIn("wasm_path", config_dict)
        self.assertIn("js_bridge_path", config_dict)


class TestWASMAnalysisResult(unittest.TestCase):
    """Test WASM analysis result model."""
    
    def test_result_creation(self):
        """Test result creation."""
        result = WASMAnalysisResult(
            best_move="e4",
            evaluation=0.5,
            depth=18,
            principal_variation=["e4", "e5", "Nf3"],
            nodes_searched=100000,
            time_ms=500,
        )
        
        self.assertEqual(result.best_move, "e4")
        self.assertEqual(result.evaluation, 0.5)
        self.assertEqual(result.depth, 18)
        self.assertEqual(len(result.principal_variation), 3)
    
    def test_result_to_dict(self):
        """Test result serialization."""
        result = WASMAnalysisResult(
            best_move="d4",
            evaluation=0.3,
            depth=20,
            nodes_searched=150000,
        )
        
        result_dict = result.to_dict()
        
        self.assertEqual(result_dict["best_move"], "d4")
        self.assertEqual(result_dict["evaluation"], 0.3)
        self.assertEqual(result_dict["depth"], 20)
        self.assertIn("metadata", result_dict)
    
    def test_result_with_none_evaluation(self):
        """Test result with None evaluation."""
        result = WASMAnalysisResult(
            best_move="Nf3",
            evaluation=None,
            depth=15,
        )
        
        self.assertIsNone(result.evaluation)
        result_dict = result.to_dict()
        self.assertIsNone(result_dict["evaluation"])


class TestStockfishWASMEngine(unittest.TestCase):
    """Test Stockfish WASM engine."""
    
    def setUp(self):
        """Set up test fixtures."""
        self.loop = asyncio.new_event_loop()
        asyncio.set_event_loop(self.loop)
        self.config = WASMEngineConfig(
            default_depth=15,
            default_time_limit_ms=1000,
        )
        self.engine = StockfishWASMEngine(self.config)
    
    def tearDown(self):
        """Clean up."""
        self.loop.close()
    
    def test_initial_state(self):
        """Test engine initial state."""
        self.assertEqual(self.engine.status, WASMEngineStatus.TERMINATED)
        self.assertIsNotNone(self.engine.config)
    
    def test_initialize_engine(self):
        """Test engine initialization."""
        async def run_test():
            await self.engine.initialize()
            self.assertEqual(self.engine.status, WASMEngineStatus.READY)
        
        self.loop.run_until_complete(run_test())
    
    def test_initialize_already_initialized(self):
        """Test initializing already initialized engine."""
        async def run_test():
            await self.engine.initialize()
            # Second initialization should not raise error
            await self.engine.initialize()
            self.assertEqual(self.engine.status, WASMEngineStatus.READY)
        
        self.loop.run_until_complete(run_test())
    
    def test_analyze_position(self):
        """Test position analysis."""
        async def run_test():
            await self.engine.initialize()
            
            result = await self.engine.analyze_position(
                fen="rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
                depth=18,
            )
            
            self.assertIsInstance(result, WASMAnalysisResult)
            self.assertIsNotNone(result.best_move)
            self.assertGreater(result.depth, 0)
        
        self.loop.run_until_complete(run_test())
    
    def test_analyze_without_initialization(self):
        """Test analysis without initialization raises error."""
        async def run_test():
            with self.assertRaises(RuntimeError):
                await self.engine.analyze_position(
                    fen="rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
                )
        
        self.loop.run_until_complete(run_test())
    
    def test_analyze_with_default_parameters(self):
        """Test analysis with default parameters from config."""
        async def run_test():
            await self.engine.initialize()
            
            result = await self.engine.analyze_position(
                fen="rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1"
            )
            
            # Should use config defaults
            self.assertEqual(result.depth, self.config.default_depth)
        
        self.loop.run_until_complete(run_test())
    
    def test_analyze_multiple_positions(self):
        """Test analyzing multiple positions concurrently."""
        async def run_test():
            await self.engine.initialize()
            
            positions = [
                "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
                "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1",
            ]
            
            results = await self.engine.analyze_multiple_positions(
                positions,
                depth=12,
            )
            
            self.assertEqual(len(results), 2)
            for result in results:
                self.assertIsInstance(result, WASMAnalysisResult)
        
        self.loop.run_until_complete(run_test())
    
    def test_stop_analysis(self):
        """Test stopping analysis."""
        async def run_test():
            await self.engine.initialize()
            
            # Should not raise error even if not analyzing
            await self.engine.stop_analysis()
        
        self.loop.run_until_complete(run_test())
    
    def test_shutdown(self):
        """Test engine shutdown."""
        async def run_test():
            await self.engine.initialize()
            await self.engine.shutdown()
            
            self.assertEqual(self.engine.status, WASMEngineStatus.TERMINATED)
        
        self.loop.run_until_complete(run_test())
    
    def test_engine_status_transitions(self):
        """Test engine status transitions."""
        async def run_test():
            # Initial state
            self.assertEqual(self.engine.status, WASMEngineStatus.TERMINATED)
            
            # Initialize
            await self.engine.initialize()
            self.assertEqual(self.engine.status, WASMEngineStatus.READY)
            
            # Analyze (status changes to BUSY during analysis)
            # After analysis completes, should return to READY
            await self.engine.analyze_position(
                fen="rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
            )
            self.assertEqual(self.engine.status, WASMEngineStatus.READY)
            
            # Shutdown
            await self.engine.shutdown()
            self.assertEqual(self.engine.status, WASMEngineStatus.TERMINATED)
        
        self.loop.run_until_complete(run_test())
    
    def test_generate_js_bridge_code(self):
        """Test JavaScript bridge code generation."""
        js_code = self.engine.generate_js_bridge_code()
        
        self.assertIsInstance(js_code, str)
        self.assertGreater(len(js_code), 0)
        self.assertIn("StockfishWASMBridge", js_code)
        self.assertIn("WebAssembly", js_code)
        self.assertIn("analyzePosition", js_code)
    
    def test_get_wasm_download_info(self):
        """Test WASM download information."""
        info = self.engine.get_wasm_download_info()
        
        self.assertIsInstance(info, dict)
        self.assertIn("official_source", info)
        self.assertIn("version", info)
        self.assertIn("files", info)
        self.assertIn("installation", info)
        
        self.assertEqual(info["version"], "16.1")
        self.assertIn("stockfish.wasm", info["files"]["wasm"])


class TestWASMIntegration(unittest.TestCase):
    """Test WASM integration scenarios."""
    
    def setUp(self):
        """Set up test fixtures."""
        self.loop = asyncio.new_event_loop()
        asyncio.set_event_loop(self.loop)
    
    def tearDown(self):
        """Clean up."""
        self.loop.close()
    
    def test_full_analysis_workflow(self):
        """Test complete analysis workflow."""
        async def run_test():
            # Create engine
            engine = StockfishWASMEngine()
            
            # Initialize
            await engine.initialize()
            self.assertEqual(engine.status, WASMEngineStatus.READY)
            
            # Analyze position
            result = await engine.analyze_position(
                fen="rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
                depth=15,
            )
            
            self.assertIsNotNone(result.best_move)
            self.assertEqual(engine.status, WASMEngineStatus.READY)
            
            # Shutdown
            await engine.shutdown()
            self.assertEqual(engine.status, WASMEngineStatus.TERMINATED)
        
        self.loop.run_until_complete(run_test())
    
    def test_concurrent_analyses(self):
        """Test concurrent position analyses."""
        async def run_test():
            engine = StockfishWASMEngine(
                WASMEngineConfig(default_time_limit_ms=500)
            )
            await engine.initialize()
            
            positions = [
                "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
                "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1",
                "rnbqkbnr/pppppppp/8/8/3P4/8/PPP1PPPP/RNBQKBNR b KQkq d3 0 1",
            ]
            
            results = await engine.analyze_multiple_positions(
                positions,
                depth=10,
            )
            
            self.assertEqual(len(results), 3)
            
            await engine.shutdown()
        
        self.loop.run_until_complete(run_test())
    
    def test_error_handling(self):
        """Test error handling in engine."""
        async def run_test():
            engine = StockfishWASMEngine()
            
            # Try to analyze without initialization
            with self.assertRaises(RuntimeError):
                await engine.analyze_position(
                    fen="rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
                )
            
            # Engine should be in error or terminated state
            self.assertIn(
                engine.status,
                [WASMEngineStatus.ERROR, WASMEngineStatus.TERMINATED]
            )
        
        self.loop.run_until_complete(run_test())
    
    def test_resource_cleanup(self):
        """Test proper resource cleanup."""
        async def run_test():
            engine = StockfishWASMEngine()
            
            await engine.initialize()
            await engine.analyze_position(
                fen="rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
            )
            
            # Shutdown should clean up all resources
            await engine.shutdown()
            
            self.assertIsNone(engine._engine)
            self.assertEqual(engine.status, WASMEngineStatus.TERMINATED)
        
        self.loop.run_until_complete(run_test())


if __name__ == "__main__":
    unittest.main()
