# XLMate AI

Python package implementing the four AI/Infra features for the XLMate chess platform.

| Module | Issue | Description |
|---|---|---|
| `stockfish_wasm.py` | [#645](https://github.com/NOVUS-X/XLMate/issues/645) | Stockfish 16.1 WASM integration |
| `worker.py` | [#642](https://github.com/NOVUS-X/XLMate/issues/642) | GPU-accelerated analysis worker |
| `resource_monitor.py` | [#644](https://github.com/NOVUS-X/XLMate/issues/644) | Unified monitoring dashboard for AI health |
| `nl_agent.py` | [#627](https://github.com/NOVUS-X/XLMate/issues/627) | Natural Language Agent interface |

## Setup

```bash
cd ai
pip install -e ".[dev]"
```

## Run tests

```bash
pytest tests/
```

## Quick usage

```python
import asyncio
from ai.stockfish_wasm import StockfishWASMEngine
from ai.worker import GPUAnalysisWorker, WorkerConfig, AnalysisRequest
from ai.resource_monitor import ResourceMonitor
from ai.nl_agent import NaturalLanguageAgent

# Stockfish WASM (issue #645)
async def wasm_example():
    engine = StockfishWASMEngine()
    await engine.initialize()
    result = await engine.analyze("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1")
    print(result.best_move, result.evaluation)
    await engine.shutdown()

# Resource monitor (issue #644)
monitor = ResourceMonitor(poll_interval_seconds=1.0)

# NL Agent (issue #627)
async def nl_example():
    engine = StockfishWASMEngine()
    await engine.initialize()
    agent = NaturalLanguageAgent(engine)
    resp = await agent.process(
        "What is the best move?",
        fen="rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1",
    )
    print(resp.text)
```
