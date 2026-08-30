/**
 * AI-44: Low-Latency WebAssembly Stockfish Engine Worker Pool
 *
 * Manages a pool of Web Worker instances running Stockfish WASM so engine
 * searches never block the main UI thread.  Worker count scales with
 * navigator.hardwareConcurrency and workers are terminated cleanly on
 * component unmount.
 */

export interface SearchRequest {
  fen: string;
  depth: number;
  requestId: string;
}

export interface SearchResult {
  requestId: string;
  bestMove: string;
  score: number;
  depth: number;
}

type PendingRequest = {
  resolve: (result: SearchResult) => void;
  reject: (err: Error) => void;
};

/**
 * A single managed Stockfish Web Worker.
 */
class StockfishWorker {
  private readonly worker: Worker;
  private busy = false;
  private pending: PendingRequest | null = null;

  constructor(workerUrl: string) {
    this.worker = new Worker(workerUrl);
    this.worker.onmessage = (e: MessageEvent<SearchResult>) => {
      this.busy = false;
      if (this.pending) {
        this.pending.resolve(e.data);
        this.pending = null;
      }
    };
    this.worker.onerror = (e) => {
      this.busy = false;
      if (this.pending) {
        this.pending.reject(new Error(e.message));
        this.pending = null;
      }
    };
  }

  get isBusy(): boolean {
    return this.busy;
  }

  search(req: SearchRequest): Promise<SearchResult> {
    return new Promise((resolve, reject) => {
      this.busy = true;
      this.pending = { resolve, reject };
      // Transfer FEN as ArrayBuffer for zero-copy messaging (AI-44)
      const encoded = new TextEncoder().encode(JSON.stringify(req));
      const buffer = encoded.buffer;
      this.worker.postMessage(buffer, [buffer]);
    });
  }

  terminate(): void {
    this.worker.terminate();
  }
}

/**
 * Pool of StockfishWorkers.  Scales to hardwareConcurrency with a floor of 1
 * and a ceiling of 4 to avoid overwhelming low-end devices.
 */
export class StockfishWorkerPool {
  private readonly workers: StockfishWorker[] = [];
  private readonly queue: Array<{ req: SearchRequest } & PendingRequest> = [];

  constructor(workerUrl: string) {
    const count = Math.min(
      4,
      Math.max(1, typeof navigator !== "undefined" ? navigator.hardwareConcurrency ?? 2 : 2),
    );
    for (let i = 0; i < count; i++) {
      this.workers.push(new StockfishWorker(workerUrl));
    }
  }

  /** Submit a search request; resolves when a worker finishes. */
  search(req: SearchRequest): Promise<SearchResult> {
    const idle = this.workers.find((w) => !w.isBusy);
    if (idle) {
      return idle.search(req).then((result) => {
        this.drainQueue();
        return result;
      });
    }
    // All workers busy — queue the request
    return new Promise((resolve, reject) => {
      this.queue.push({ req, resolve, reject });
    });
  }

  private drainQueue(): void {
    if (this.queue.length === 0) return;
    const idle = this.workers.find((w) => !w.isBusy);
    if (!idle) return;
    const next = this.queue.shift()!;
    idle.search(next.req).then(next.resolve, next.reject).finally(() => this.drainQueue());
  }

  /** Terminate all workers — call on component unmount. */
  terminate(): void {
    this.workers.forEach((w) => w.terminate());
    this.workers.length = 0;
  }
}
