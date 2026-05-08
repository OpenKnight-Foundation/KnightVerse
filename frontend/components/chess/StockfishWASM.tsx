import React, { useState, useEffect, useCallback, useRef } from 'react';

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
 * React hook for integrating Stockfish WASM engine
 * 
 * @param config - Engine configuration options
 * @returns Engine control interface
 */
export function useStockfishWASM(config: WASMEngineConfig = {}): UseStockfishWASMReturn {
  const [isReady, setIsReady] = useState(false);
  const [isAnalyzing, setIsAnalyzing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  
  const workerRef = useRef<Worker | null>(null);
  const engineRef = useRef<unknown>(null);
  const resolveRef = useRef<((result: AnalysisResult) => void) | null>(null);
  const rejectRef = useRef<((error: Error) => void) | null>(null);
  const timeoutRef = useRef<NodeJS.Timeout | null>(null);

  // Handle messages from engine
  const handleEngineMessage = useCallback((data: { type: string; bestMove?: string; evaluation?: number | null; depth?: number; pv?: string[]; nodes?: number; timeMs?: number; message?: string }) => {
    if (data.type === 'bestmove') {
      setIsAnalyzing(false);
      
      if (resolveRef.current) {
        const result: AnalysisResult = {
          bestMove: data.bestMove || '',
          evaluation: data.evaluation || null,
          depth: data.depth || 0,
          principalVariation: data.pv || [],
          nodesSearched: data.nodes || 0,
          timeMs: data.timeMs || 0,
        };
        
        resolveRef.current(result);
        resolveRef.current = null;
        rejectRef.current = null;
      }

      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current);
        timeoutRef.current = null;
      }
    } else if (data.type === 'info') {
      // Handle intermediate analysis info
      console.log('Analysis progress:', data);
    } else if (data.type === 'error') {
      setIsAnalyzing(false);
      setError(data.message || 'Engine error');
      
      if (rejectRef.current) {
        rejectRef.current(new Error(data.message));
        rejectRef.current = null;
      }
    }
  }, []);

  // Cleanup resources
  const cleanup = useCallback(() => {
    if (timeoutRef.current) {
      clearTimeout(timeoutRef.current);
      timeoutRef.current = null;
    }
    
    if (workerRef.current) {
      workerRef.current.postMessage({ type: 'quit' });
      workerRef.current.terminate();
      workerRef.current = null;
    }
    
    engineRef.current = null;
    setIsReady(false);
    setIsAnalyzing(false);
  }, []);

  // Initialize WASM engine
  useEffect(() => {
    let cancelled = false;

    const initializeEngine = async () => {
      try {
        setError(null);
        
        // Load Stockfish worker
        const workerPath = config.jsBridgePath || '/assets/stockfish.js';
        const worker = new Worker(workerPath);
        workerRef.current = worker;

        // Setup message handler
        worker.onmessage = (event) => {
          handleEngineMessage(event.data);
        };

        worker.onerror = (error) => {
          console.error('WASM Worker error:', error);
          setError('Failed to load Stockfish WASM');
          setIsReady(false);
        };

        // Initialize engine with configuration
        worker.postMessage({
          type: 'init',
          config: {
            Threads: config.threads || 1,
            Hash: config.hashSizeMB || 16,
            'Skill Level': config.skillLevel || 20,
          },
        });

        // Wait for ready signal
        await new Promise<void>((resolve, reject) => {
          const timeout = setTimeout(() => {
            reject(new Error('Engine initialization timeout'));
          }, 10000);

          const checkReady = (event: MessageEvent) => {
            if (event.data.type === 'ready') {
              clearTimeout(timeout);
              worker.removeEventListener('message', checkReady);
              if (!cancelled) {
                setIsReady(true);
                resolve();
              }
            }
          };

          worker.addEventListener('message', checkReady);
        });

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
  }, [config, cleanup, handleEngineMessage]);

  // Analyze a position
  const analyzePosition = useCallback(async (
    fen: string,
    depth?: number
  ): Promise<AnalysisResult> => {
    if (!isReady || !workerRef.current) {
      throw new Error('Engine not ready');
    }

    if (isAnalyzing) {
      throw new Error('Analysis already in progress');
    }

    setIsAnalyzing(true);
    setError(null);

    const searchDepth = depth || config.defaultDepth || 18;
    const timeLimit = config.defaultTimeLimit || 3000;

    return new Promise((resolve, reject) => {
      resolveRef.current = resolve;
      rejectRef.current = reject;

      // Send analysis request
      workerRef.current!.postMessage({
        type: 'analyze',
        fen,
        depth: searchDepth,
        timeLimit,
      });

      // Set timeout
      timeoutRef.current = setTimeout(() => {
        setIsAnalyzing(false);
        reject(new Error('Analysis timeout'));
        resolveRef.current = null;
        rejectRef.current = null;
      }, timeLimit + 2000);
    });
  }, [isReady, isAnalyzing, config]);

  // Stop current analysis
  const stopAnalysis = useCallback(() => {
    if (workerRef.current && isAnalyzing) {
      workerRef.current.postMessage({ type: 'stop' });
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

  // Shutdown engine
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
