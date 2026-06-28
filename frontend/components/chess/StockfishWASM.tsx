import { useState, useEffect, useCallback, useRef } from 'react';

interface WASMEngineConfig {
  wasmPath?: string;
  jsBridgePath?: string;
  threads?: number;
  hashSizeMB?: number;
  skillLevel?: number;
  defaultDepth?: number;
  defaultTimeLimit?: number;
}

export interface AnalysisResult {
  bestMove: string;
  evaluation: number | null;
  depth: number;
  principalVariation: string[];
  nodesSearched: number;
  timeMs: number;
}

interface UseStockfishWASMReturn {
  isReady: boolean;
  isAnalyzing: boolean;
  error: string | null;
  analyzePosition: (fen: string, depth?: number) => Promise<AnalysisResult>;
  stopAnalysis: () => void;
  shutdown: () => void;
}

/**
 * React hook for integrating Stockfish WASM engine via UCI protocol.
 *
 * The bundled stockfish.js (compiled by niklasf/stockfish.js via Emscripten)
 * communicates through `postMessage` using raw UCI text strings — e.g. "uci",
 * "isready", "position fen …", "go depth …" — and replies with UCI text
 * lines like "readyok", "info depth …", and "bestmove e2e4".
 *
 * @param config - Engine configuration options
 * @returns Engine control interface
 */
export function useStockfishWASM(config: WASMEngineConfig = {}): UseStockfishWASMReturn {
  const [isReady, setIsReady] = useState(false);
  const [isAnalyzing, setIsAnalyzing] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const workerRef = useRef<Worker | null>(null);
  const resolveRef = useRef<((result: AnalysisResult) => void) | null>(null);
  const rejectRef = useRef<((error: Error) => void) | null>(null);
  const timeoutRef = useRef<NodeJS.Timeout | null>(null);

  // Accumulator for the latest "info" line data while analyzing
  const latestInfo = useRef<{
    depth: number;
    evaluation: number | null;
    pv: string[];
    nodes: number;
    timeMs: number;
  }>({ depth: 0, evaluation: null, pv: [], nodes: 0, timeMs: 0 });

  // ─── Parse a UCI "info" line ───
  const parseInfoLine = useCallback((line: string) => {
    const tokens = line.split(' ');
    const info = { ...latestInfo.current };

    for (let i = 0; i < tokens.length; i++) {
      switch (tokens[i]) {
        case 'depth':
          info.depth = parseInt(tokens[++i], 10) || 0;
          break;
        case 'score': {
          const scoreType = tokens[++i]; // "cp" or "mate"
          const scoreVal = parseInt(tokens[++i], 10) || 0;
          info.evaluation = scoreType === 'cp' ? scoreVal / 100 : (scoreVal > 0 ? 100 : -100);
          break;
        }
        case 'nodes':
          info.nodes = parseInt(tokens[++i], 10) || 0;
          break;
        case 'time':
          info.timeMs = parseInt(tokens[++i], 10) || 0;
          break;
        case 'pv':
          info.pv = tokens.slice(i + 1);
          i = tokens.length; // consume rest
          break;
      }
    }

    latestInfo.current = info;
  }, []);

  // ─── Handle raw UCI messages from the worker ───
  const handleMessage = useCallback(
    (event: MessageEvent) => {
      const line = typeof event.data === 'string' ? event.data : '';

      if (line.startsWith('bestmove')) {
        setIsAnalyzing(false);
        const bestMove = line.split(' ')[1] || '';

        if (resolveRef.current) {
          resolveRef.current({
            bestMove,
            evaluation: latestInfo.current.evaluation,
            depth: latestInfo.current.depth,
            principalVariation: latestInfo.current.pv,
            nodesSearched: latestInfo.current.nodes,
            timeMs: latestInfo.current.timeMs,
          });
          resolveRef.current = null;
          rejectRef.current = null;
        }

        if (timeoutRef.current) {
          clearTimeout(timeoutRef.current);
          timeoutRef.current = null;
        }
      } else if (line.startsWith('info') && line.includes('depth')) {
        parseInfoLine(line);
      }
    },
    [parseInfoLine],
  );

  // ─── Cleanup ───
  const cleanup = useCallback(() => {
    if (timeoutRef.current) {
      clearTimeout(timeoutRef.current);
      timeoutRef.current = null;
    }

    if (workerRef.current) {
      workerRef.current.postMessage('quit');
      workerRef.current.terminate();
      workerRef.current = null;
    }

    setIsReady(false);
    setIsAnalyzing(false);
  }, []);

  // ─── Initialize the engine ───
  useEffect(() => {
    let cancelled = false;

    const initializeEngine = async () => {
      try {
        setError(null);

        const workerPath = config.jsBridgePath || '/assets/stockfish.js';
        // Using window.Worker (instead of bare `new Worker`) tells Turbopack
        // not to attempt static analysis of this dynamically resolved path
        // (fixes TP1001: "new Worker(...) is not statically analyse-able").
        const worker = new window.Worker(workerPath);
        workerRef.current = worker;

        worker.onerror = (err) => {
          console.error('WASM Worker error:', err);
          setError('Failed to load Stockfish WASM');
          setIsReady(false);
        };

        // Wait for the engine to respond to "isready" with "readyok"
        await new Promise<void>((resolve, reject) => {
          const timeout = setTimeout(() => {
            reject(new Error('Engine initialization timeout'));
          }, 15000);

          const onMsg = (event: MessageEvent) => {
            const line = typeof event.data === 'string' ? event.data : '';

            if (line === 'readyok') {
              clearTimeout(timeout);
              worker.removeEventListener('message', onMsg);
              if (!cancelled) {
                setIsReady(true);
                resolve();
              }
            }
          };

          worker.addEventListener('message', onMsg);

          // Send UCI init sequence
          worker.postMessage('uci');

          // Configure engine options
          const skillLevel = config.skillLevel ?? 20;
          const hashMB = config.hashSizeMB ?? 16;
          worker.postMessage(`setoption name Skill Level value ${skillLevel}`);
          worker.postMessage(`setoption name Hash value ${hashMB}`);

          // Ask the engine if it's ready
          worker.postMessage('isready');
        });

        // Attach the main message handler after init
        worker.addEventListener('message', handleMessage);
      } catch (err) {
        if (!cancelled) {
          const errorMessage = err instanceof Error ? err.message : 'Unknown error';
          setError(`Failed to initialize engine: ${errorMessage}`);
          console.error(err);
        }
      }
    };

    initializeEngine();

    return () => {
      cancelled = true;
      cleanup();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // ─── Analyze a position via UCI ───
  const analyzePosition = useCallback(
    async (fen: string, depth?: number): Promise<AnalysisResult> => {
      if (!isReady || !workerRef.current) {
        throw new Error('Engine not ready');
      }

      if (isAnalyzing) {
        throw new Error('Analysis already in progress');
      }

      setIsAnalyzing(true);
      setError(null);

      const searchDepth = depth || config.defaultDepth || 18;
      const timeLimit = config.defaultTimeLimit || 5000;

      // Reset info accumulator
      latestInfo.current = { depth: 0, evaluation: null, pv: [], nodes: 0, timeMs: 0 };

      return new Promise((resolve, reject) => {
        resolveRef.current = resolve;
        rejectRef.current = reject;

        // Send position + go command (UCI protocol)
        workerRef.current!.postMessage('ucinewgame');
        workerRef.current!.postMessage(`position fen ${fen}`);
        workerRef.current!.postMessage(`go depth ${searchDepth}`);

        // Set timeout
        timeoutRef.current = setTimeout(() => {
          // Try to stop the engine gracefully first
          workerRef.current?.postMessage('stop');
          setTimeout(() => {
            if (resolveRef.current) {
              setIsAnalyzing(false);
              reject(new Error('Analysis timeout'));
              resolveRef.current = null;
              rejectRef.current = null;
            }
          }, 500);
        }, timeLimit + 2000);
      });
    },
    [isReady, isAnalyzing, config],
  );

  // ─── Stop current analysis ───
  const stopAnalysis = useCallback(() => {
    if (workerRef.current && isAnalyzing) {
      workerRef.current.postMessage('stop');
      setIsAnalyzing(false);

      if (rejectRef.current) {
        rejectRef.current(new Error('Analysis stopped'));
        rejectRef.current = null;
      }

      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current);
        timeoutRef.current = null;
      }
    }
  }, [isAnalyzing]);

  // ─── Shutdown ───
  const shutdown = useCallback(() => {
    cleanup();
  }, [cleanup]);

  return {
    isReady,
    isAnalyzing,
    error,
    analyzePosition,
    stopAnalysis,
    shutdown,
  };
}

/**
 * React component for displaying analysis results
 */
interface AnalysisDisplayProps {
  result: AnalysisResult | null;
  isAnalyzing: boolean;
}

export function AnalysisDisplay({ result, isAnalyzing }: AnalysisDisplayProps) {
  if (isAnalyzing) {
    return (
      <div className="analysis-loading">
        <div className="spinner" />
        <p>Analyzing position...</p>
      </div>
    );
  }

  if (!result) {
    return (
      <div className="analysis-empty">
        <p>Click &quot;Analyze&quot; to get engine evaluation</p>
      </div>
    );
  }

  const formatEvaluation = (eval_: number | null): string => {
    if (eval_ === null) return '0.00';
    return eval_ > 0 ? `+${eval_.toFixed(2)}` : eval_.toFixed(2);
  };

  return (
    <div className="analysis-results">
      <div className="evaluation">
        <span className="eval-value">{formatEvaluation(result.evaluation)}</span>
        <span className="eval-depth">Depth: {result.depth}</span>
      </div>

      <div className="best-move">
        <strong>Best Move:</strong> {result.bestMove}
      </div>

      {result.principalVariation.length > 0 && (
        <div className="principal-variation">
          <strong>Principal Variation:</strong>
          <div className="pv-moves">
            {result.principalVariation.map((move, index) => (
              <span key={index} className="pv-move">
                {move}
              </span>
            ))}
          </div>
        </div>
      )}

      <div className="analysis-stats">
        <span>Nodes: {result.nodesSearched.toLocaleString()}</span>
        <span>Time: {result.timeMs}ms</span>
      </div>
    </div>
  );
}

/**
 * React component for WASM engine status indicator
 */
interface EngineStatusProps {
  isReady: boolean;
  isAnalyzing: boolean;
  error: string | null;
}

export function EngineStatus({ isReady, isAnalyzing, error }: EngineStatusProps) {
  if (error) {
    return (
      <div className="engine-status error">
        <span className="status-icon">❌</span>
        <span className="status-text">{error}</span>
      </div>
    );
  }

  if (isAnalyzing) {
    return (
      <div className="engine-status analyzing">
        <span className="status-icon">🔄</span>
        <span className="status-text">Analyzing...</span>
      </div>
    );
  }

  if (isReady) {
    return (
      <div className="engine-status ready">
        <span className="status-icon">✅</span>
        <span className="status-text">Stockfish WASM Ready</span>
      </div>
    );
  }

  return (
    <div className="engine-status loading">
      <span className="status-icon">⏳</span>
      <span className="status-text">Loading engine...</span>
    </div>
  );
}

export default useStockfishWASM;
