# agent-engines

GPU-oriented engine infrastructure for the XLMate chess platform. The module provides an asyncio-based worker pool that wraps UCI-compatible engines, with first-class support for Leela Chess Zero (`lc0`), CPU fallback support for Stockfish, **Natural Language Agent interface**, **Stockfish 16.1 WASM integration**, and **intelligent resource orchestration**.

## Overview

The GPU worker subsystem is designed for long-running analysis services where neural-network inference should stay close to a dedicated GPU while requests are dispatched through a pool abstraction.

**New Features:**
- 🤖 **Natural Language Agent**: Interact with chess engines using plain English
- 🌐 **Stockfish WASM**: Browser-compatible chess engine via WebAssembly
- 🚀 **Soroban CI/CD**: Automated blockchain contract deployment pipeline
- ⚡ **Resource Optimizer**: Intelligent CPU/memory allocation with gas cost estimation
- 🔄 **Deployment Pipeline**: Automated validation, testing, optimization, and rollback
- 📊 **Performance Monitoring**: Real-time metrics and system capacity tracking

```text
Client/API
   |
   v
AnalysisRequest
   |
   v
WorkerPool ------------------> ResourceMonitor
   |
   +--> GPUAnalysisWorker #1 --> AsyncUciBridge --> lc0 / stockfish
   |
   +--> GPUAnalysisWorker #2 --> AsyncUciBridge --> lc0 / stockfish
   |
   +--> BatchAnalyzer (optional request coalescing)
```

## Package layout

- `gpu_worker/config.py`: worker and GPU configuration models.
- `gpu_worker/models.py`: request, result, and status models.
- `gpu_worker/anomaly.py`: passive bot-farm anomaly detection over request telemetry.
- `gpu_worker/uci_bridge.py`: async UCI subprocess bridge and protocol parsing.
- `gpu_worker/worker.py`: single-worker lifecycle and analysis orchestration.
- `gpu_worker/pool.py`: least-loaded worker dispatch.
- `gpu_worker/batch.py`: time- and size-based batching layer.
- `gpu_worker/resource_monitor.py`: CPU/GPU monitoring with graceful fallback.
- `gpu_worker/resource_optimizer.py`: **NEW** Intelligent resource allocation and optimization.
- `gpu_worker/deployment_pipeline.py`: **NEW** Automated deployment pipeline orchestrator.
- `gpu_worker/nl_models.py`: Natural language agent request/response models.
- `gpu_worker/nl_intent_parser.py`: Intent recognition and entity extraction.
- `gpu_worker/nl_agent.py`: Natural language agent service layer.
- `gpu_worker/stockfish_wasm.py`: Stockfish 16.1 WASM integration module.
- `gpu_worker/stockfish_wasm_bridge.py`: TypeScript bridge code generator.
- `main.py`: **ENHANCED** Agent engine orchestrator with resource optimization.

## Requirements

- Python 3.11+
- A UCI-compliant engine binary
- For GPU acceleration with `lc0`:
  - NVIDIA drivers and CUDA/cuDNN or another supported backend
  - Optional `pynvml` installation for detailed GPU metrics
  - Leela network weights (`.pb.gz` / `.onnx`) when required by the selected engine build

## Installation

```bash
cd agent-engines
python3 -m venv .venv
source .venv/bin/activate
pip install -e .[dev]
```

For NVIDIA monitoring:

```bash
pip install -e .[gpu,dev]
```

## Configuration

`WorkerConfig` controls engine selection, defaults, and GPU tuning.

```python
from gpu_worker.config import EngineBackend, GPUConfig, WorkerConfig

config = WorkerConfig(
    engine_backend=EngineBackend.LC0,
    engine_path="/usr/local/bin/lc0",
    gpu=GPUConfig(device_id=0, max_batch_size=32, memory_limit_mb=2048, backend="cudnn"),
    default_depth=22,
    default_time_limit_ms=3000,
    threads=2,
    hash_size_mb=512,
    network_weights_path="/models/bt4.pb.gz",
)
```

Key options:

- `engine_backend`: `lc0` or `stockfish`
- `engine_path`: path to the engine binary
- `gpu.device_id`: GPU index to bind the worker to
- `gpu.backend`: GPU backend such as `cudnn`, `cuda`, or `opencl`
- `max_concurrent_analyses`: queued work allowed per worker
- `default_depth` / `default_time_limit_ms`: search defaults when requests omit limits
- `threads`: engine thread count
- `hash_size_mb`: cache/hash allocation
- `network_weights_path`: lc0 weights file path

## Usage

### Start a worker pool

```bash
python main.py
```

### Natural Language Agent

Interact with chess engines using plain English:

```python
import asyncio

from gpu_worker.config import WorkerConfig
from gpu_worker.pool import WorkerPool
from gpu_worker.nl_agent import NaturalLanguageAgent


async def run() -> None:
    # Initialize worker pool
    pool = WorkerPool([WorkerConfig()])
    await pool.start_all()
    
    try:
        # Create natural language agent
        agent = NaturalLanguageAgent(pool)
        
        # Ask for move suggestion
        response = await agent.process_request(
            user_input="What's the best move in this position?",
            fen="rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1",
        )
        
        print(response.natural_language_response)
        print(f"Best move: {response.best_move}")
        
        # Ask for position analysis
        analysis = await agent.process_request(
            user_input="Analyze this position for me",
            fen="rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            complexity="beginner",  # or "intermediate", "advanced"
        )
        
        print(analysis.natural_language_response)
        
    finally:
        await pool.shutdown_all()


asyncio.run(run())
```

### Stockfish WASM Integration

Use Stockfish 16.1 in the browser via WebAssembly:

```python
import asyncio

from gpu_worker.stockfish_wasm import StockfishWASMEngine, WASMEngineConfig


async def run() -> None:
    # Create WASM engine
    config = WASMEngineConfig(
        threads=2,
        hash_size_mb=32,
        default_depth=18,
    )
    engine = StockfishWASMEngine(config)
    
    try:
        # Initialize engine
        await engine.initialize()
        
        # Analyze position
        result = await engine.analyze_position(
            fen="rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            depth=20,
        )
        
        print(f"Best move: {result.best_move}")
        print(f"Evaluation: {result.evaluation}")
        print(f"Depth: {result.depth}")
        
    finally:
        await engine.shutdown()


asyncio.run(run())
```

### Frontend WASM Integration (React/TypeScript)

```typescript
import { useStockfishWASM, AnalysisDisplay } from '@/components/chess/StockfishWASM';

function ChessAnalysisPanel() {
  const { isReady, isAnalyzing, error, analyzePosition } = useStockfishWASM({
    defaultDepth: 18,
    threads: 2,
  });

  const handleAnalyze = async () => {
    const result = await analyzePosition(
      'rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1'
    );
    console.log('Best move:', result.bestMove);
  };

  return (
    <div>
      <button onClick={handleAnalyze} disabled={!isReady}>
        Analyze Position
      </button>
      <AnalysisDisplay result={null} isAnalyzing={isAnalyzing} />
    </div>
  );
}
```

### Submit a single request

```python
import asyncio

from gpu_worker.config import WorkerConfig
from gpu_worker.models import AnalysisRequest
from gpu_worker.pool import WorkerPool


async def run() -> None:
    pool = WorkerPool([WorkerConfig()])
    await pool.start_all()
    try:
        result = await pool.submit(
            AnalysisRequest(
                fen="rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
                depth=18,
            )
        )
        print(result.model_dump())
    finally:
        await pool.shutdown_all()


asyncio.run(run())
```

### Batch analysis

```python
import asyncio

from gpu_worker.batch import BatchAnalyzer
from gpu_worker.config import WorkerConfig
from gpu_worker.models import AnalysisRequest
from gpu_worker.pool import WorkerPool


async def run() -> None:
    pool = WorkerPool([WorkerConfig(), WorkerConfig(gpu={"device_id": 1})])
    await pool.start_all()
    analyzer = BatchAnalyzer(pool, batch_size=8, flush_interval_ms=50)
    try:
        results = await analyzer.submit_batch(
            [
                AnalysisRequest(
                    fen="rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
                    priority=10,
                )
            ]
        )
        print([result.best_move for result in results])
    finally:
        await analyzer.shutdown()
        await pool.shutdown_all()


asyncio.run(run())
```

### Bot-farm anomaly detection

`BotFarmAnomalyDetector` passively scores request telemetry before or after dispatch; it never blocks `WorkerPool.submit()`.

```python
from gpu_worker.anomaly import BotFarmAnomalyDetector
from gpu_worker.models import AnalysisRequest


detector = BotFarmAnomalyDetector()
report = detector.record_request(
    AnalysisRequest(
        fen="rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        depth=18,
        actor_id="user-42",
        session_id="session-abc",
        ip_hash="sha256:ip-hash",
        device_hash="sha256:device-hash",
    )
)
print(report.risk_level, report.score)
```

## Testing

The test suite uses mocked UCI processes and does not require `lc0` or Stockfish to be installed.

```bash
pytest
```

### Run specific test suites

```bash
# Test Natural Language Agent
pytest tests/test_nl_agent.py -v

# Test Stockfish WASM integration
pytest tests/test_stockfish_wasm.py -v

# Test GPU worker pool
pytest tests/test_pool.py -v

# Run with coverage
pytest --cov=gpu_worker --cov-report=html
```

## Soroban Contract Deployment

XLMate includes a comprehensive CI/CD pipeline for deploying Soroban smart contracts to Stellar networks.

### Automated Deployment

The GitHub Actions workflow (`.github/workflows/soroban-deploy.yml`) provides:

- **Automated testing and validation** of all contracts
- **Multi-environment deployment** (testnet, futurenet)
- **Rollback support** with backup management
- **Deployment verification** and health checks
- **Artifact preservation** for audit trails

### Manual Deployment

Use the advanced deployment script for manual deployments:

```bash
cd contracts

# Deploy all contracts to testnet
./deploy_advanced.sh deploy testnet

# Deploy specific contract
./deploy_advanced.sh deploy testnet game_contract testnet-deployer

# Rollback to previous deployment
./deploy_advanced.sh rollback testnet game_contract

# Verify deployment
./deploy_advanced.sh verify testnet game_contract

# View deployment history
./deploy_advanced.sh history

# List available backups
./deploy_advanced.sh backups
```

### CI/CD Pipeline Features

1. **Validation Stage**: Runs tests, builds WASM, validates contracts
2. **Deployment Stage**: Deploys to specified network with identity management
3. **Verification Stage**: Confirms successful deployment
4. **Notification Stage**: Generates deployment summary

### Required Secrets

Set up these GitHub secrets for automated deployment:

- `SOROBAN_SECRET_KEY`: Testnet deployment key
- `SOROBAN_FUTURENET_SECRET_KEY`: Futurenet deployment key

### Environment Configuration

Configure GitHub environments:

- **testnet**: Requires approval for main branch deployments
- **futurenet**: Manual trigger only

## Notes

- The UCI bridge works with any UCI-compatible engine and only applies `setoption` calls for engine options reported during the `uci` handshake.
- GPU monitoring gracefully degrades to empty metrics when NVML or `nvidia-smi` are unavailable.
- `BatchAnalyzer` improves throughput for bursty workloads such as review pipelines or offline game processing.

## Resource Optimization & Orchestration

The agent-engines module now includes intelligent resource orchestration for efficient CPU/GPU utilization and automated deployment pipelines.

### Resource Optimizer

The `ResourceOptimizer` automatically calculates optimal resource allocation based on:
- System capacity (CPU cores, memory)
- Current system load
- Engine resource tier requirements
- Computational cost estimation (gas model)

```python
import asyncio
from main import AgentEngineOrchestrator, EngineConfig, EngineType
from gpu_worker.resource_optimizer import ResourceTier


async def run() -> None:
    orchestrator = AgentEngineOrchestrator()
    await orchestrator.start()
    
    try:
        # Provision engine with automatic resource optimization
        config = EngineConfig(
            engine_type=EngineType.STOCKFISH,
            threads=4,
            memory_mb=1024
        )
        
        # Choose resource tier: LIGHT, STANDARD, HIGH, or UNLIMITED
        success = await orchestrator.provision_engine(
            "copilot-alpha",
            config,
            resource_tier=ResourceTier.HIGH
        )
        
        # View resource metrics
        metrics = orchestrator.get_resource_metrics()
        print(f"CPU Usage: {metrics['current']['cpu_percent']}%")
        print(f"Memory Available: {metrics['current']['memory_available_mb']}MB")
        
    finally:
        await orchestrator.shutdown()


asyncio.run(run())
```

#### Resource Tiers

- **LIGHT**: 25% of available resources, quick responses
- **STANDARD**: 50% of available resources, balanced performance
- **HIGH**: 75% of available resources, maximum performance
- **UNLIMITED**: 100% of available resources (testing/development)

### Deployment Pipeline Orchestrator

The `DeploymentPipelineOrchestrator` manages the complete engine lifecycle:
1. **Validation**: Verify configuration and dependencies
2. **Building**: Compile engine artifacts
3. **Testing**: Run automated tests
4. **Optimization**: Optimize resource allocation
5. **Deployment**: Deploy to target environment
6. **Verification**: Confirm successful deployment
7. **Rollback**: Automatic rollback on failure (if enabled)

```python
import asyncio
from main import AgentEngineOrchestrator, EngineConfig, EngineType
from gpu_worker.resource_optimizer import ResourceTier
from gpu_worker.deployment_pipeline import DeploymentTarget


async def run() -> None:
    orchestrator = AgentEngineOrchestrator()
    await orchestrator.start()
    
    try:
        config = EngineConfig(
            engine_type=EngineType.MAIA,
            threads=2,
            memory_mb=512
        )
        
        # Deploy using full pipeline with automatic rollback
        result = await orchestrator.deploy_engine_with_pipeline(
            agent_id="copilot-beta",
            config=config,
            resource_tier=ResourceTier.STANDARD,
            target=DeploymentTarget.LOCAL
        )
        
        print(f"Pipeline status: {result.status.value}")
        print(f"Deployment ID: {result.deployment_id}")
        print(f"Duration: {result.duration_ms}ms")
        print(f"Rollback available: {result.rollback_available}")
        
    finally:
        await orchestrator.shutdown()
```

### Performance Monitoring

Monitor real-time system metrics and engine performance:

```python
import asyncio
from main import AgentEngineOrchestrator


async def run() -> None:
    orchestrator = AgentEngineOrchestrator()
    await orchestrator.start()
    
    try:
        # Get comprehensive orchestration state
        state = orchestrator.get_orchestration_state()
        
        print(f"Active engines: {state['active_count']}")
        print(f"Cluster nodes: {state['cluster_nodes']}")
        print(f"System resources:")
        print(f"  CPU: {state['system_resources']['cpu_percent']}%")
        print(f"  Memory: {state['system_resources']['memory_used_mb']}MB used")
        
        # Get detailed resource metrics
        metrics = orchestrator.get_resource_metrics()
        print(f"\nResource allocations:")
        for agent_id, alloc in metrics['allocations'].items():
            print(f"  {agent_id}: {alloc['threads']} threads, {alloc['memory_mb']}MB")
        
    finally:
        await orchestrator.shutdown()
```

### Gas Cost Estimation

The resource optimizer includes a gas cost model to estimate computational expenses:

```python
from gpu_worker.resource_optimizer import ResourceOptimizer

optimizer = ResourceOptimizer()

# Estimate cost for different engines and depths
stockfish_cost = optimizer.estimate_gas_cost("stockfish", complexity=20)
lc0_cost = optimizer.estimate_gas_cost("lc0", complexity=20)

print(f"Stockfish depth 20: {stockfish_cost:.2f} gas units")
print(f"LC0 depth 20: {lc0_cost:.2f} gas units")
```

### Auto-Scaling & Throttling

The orchestrator automatically adjusts resource allocation based on system load:

- **Under heavy load** (>80%): Reduces allocation by 50%
- **Under light load** (<30%): Increases allocation by up to 2x
- **Throttling**: Automatically triggers when CPU/memory exceeds 90%

```python
from gpu_worker.resource_optimizer import ResourceOptimizer

optimizer = ResourceOptimizer()

# Check if throttling should be applied
metrics = optimizer.get_current_metrics()
should_throttle = optimizer.should_throttle(metrics, threshold=90.0)

if should_throttle:
    print("System under heavy load - throttling new requests")
else:
    print("System operating normally")