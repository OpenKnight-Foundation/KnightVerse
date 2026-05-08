"""XLMate AI package — Stockfish WASM, GPU worker, resource monitor, NL agent."""

from ai.nl_agent import NaturalLanguageAgent
from ai.resource_monitor import ResourceMonitor
from ai.stockfish_wasm import StockfishWASMEngine, WASMEngineConfig
from ai.worker import GPUAnalysisWorker

__all__ = [
    "GPUAnalysisWorker",
    "NaturalLanguageAgent",
    "ResourceMonitor",
    "StockfishWASMEngine",
    "WASMEngineConfig",
]
