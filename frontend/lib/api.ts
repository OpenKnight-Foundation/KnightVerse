/**
 * Centralized API configuration (#FE-28)
 *
 * Single source of truth for all API endpoints and host URLs.
 * Pulls values from environment variables with sensible defaults.
 * All modules should import from this file instead of reading
 * `process.env` directly.
 */

function env(name: string, fallback: string): string {
  return process.env[name] ?? fallback;
}

/** Backend / game-server base URL (REST + WebSocket) */
export const API_BASE = env("NEXT_PUBLIC_API_URL", "http://localhost:8000");

/** Derive the WebSocket base from the HTTP base automatically */
export const WS_BASE = API_BASE.replace(/^http/, "ws");

/** Legacy alias — some older services read `NEXT_PUBLIC_BACKEND_URL` */
export const BACKEND_BASE = env("NEXT_PUBLIC_BACKEND_URL", API_BASE);

/** Stellar Horizon API */
export const HORIZON_URL = env(
  "NEXT_PUBLIC_HORIZON_URL",
  "https://horizon-testnet.stellar.org",
);

/** Soroban RPC endpoint */
export const SOROBAN_RPC = env(
  "NEXT_PUBLIC_SOROBAN_RPC",
  "https://soroban-testnet.stellar.org:443",
);

/** Stellar network passphrase */
export const NETWORK_PASSPHRASE = env(
  "NEXT_PUBLIC_NETWORK_PASSPHRASE",
  "Test SDF Network ; September 2015",
);

/** IPFS gateway for decentralized storage */
export const IPFS_GATEWAY = env(
  "NEXT_PUBLIC_IPFS_GATEWAY",
  "https://gateway.pinata.cloud",
);

// ── Retry / Backoff (#FE-75) ─────────────────────────────────────────────────

/** Options that control retry behaviour for `fetchWithRetry`. */
export interface RetryOptions {
  /** Maximum number of retry attempts (default: 3). */
  maxRetries?: number;
  /** Base delay in ms before the first retry (default: 200). Doubles each attempt. */
  baseDelayMs?: number;
  /** Cap on the total delay between retries in ms (default: 5000). */
  maxDelayMs?: number;
  /**
   * When true, non-idempotent methods (POST / PUT / DELETE / PATCH) are also
   * retried.  Defaults to false — only GET (and HEAD/OPTIONS) are retried by
   * default.
   */
  retryNonIdempotent?: boolean;
  /**
   * Custom predicate that decides whether a failed attempt should be retried.
   * Receives the `Response` (when the server replied) or `undefined` (network
   * error / timeout).  Defaults to retrying on 5xx status codes and network
   * errors.
   */
  shouldRetry?: (response: Response | undefined, attempt: number) => boolean;
}

/** Idempotent HTTP methods that are safe to retry without extra opt-in. */
const IDEMPOTENT_METHODS = new Set(["GET", "HEAD", "OPTIONS"]);

const DEFAULT_RETRY_OPTIONS: Required<Omit<RetryOptions, "shouldRetry">> = {
  maxRetries: 3,
  baseDelayMs: 200,
  maxDelayMs: 5000,
  retryNonIdempotent: false,
};

function defaultShouldRetry(response: Response | undefined): boolean {
  // Network / timeout errors (no response at all) are always retried.
  if (response === undefined) return true;
  // Retry on server-side errors; do NOT retry client errors (4xx).
  return response.status >= 500;
}

function computeDelay(attempt: number, baseDelayMs: number, maxDelayMs: number): number {
  // Exponential backoff: baseDelay * 2^attempt, capped at maxDelay.
  return Math.min(baseDelayMs * Math.pow(2, attempt), maxDelayMs);
}

/**
 * Drop-in replacement for `fetch` that adds exponential-backoff retry logic.
 *
 * Non-idempotent requests (POST, PUT, DELETE, PATCH) are NOT retried unless
 * `retryOptions.retryNonIdempotent` is set to `true`.
 *
 * @example
 * const res = await fetchWithRetry('/v1/puzzles');
 * const data = await res.json();
 */
export async function fetchWithRetry(
  url: string,
  options?: RequestInit,
  retryOptions?: RetryOptions,
): Promise<Response> {
  const {
    maxRetries,
    baseDelayMs,
    maxDelayMs,
    retryNonIdempotent,
  } = { ...DEFAULT_RETRY_OPTIONS, ...retryOptions };

  const shouldRetryFn = retryOptions?.shouldRetry ?? defaultShouldRetry;

  const method = (options?.method ?? "GET").toUpperCase();
  const isIdempotent = IDEMPOTENT_METHODS.has(method);
  const retryEnabled = isIdempotent || retryNonIdempotent;

  let lastResponse: Response | undefined;
  let lastError: unknown;

  for (let attempt = 0; attempt <= maxRetries; attempt++) {
    try {
      const response = await fetch(url, options);

      if (response.ok || !retryEnabled || !shouldRetryFn(response, attempt)) {
        return response;
      }

      lastResponse = response;
    } catch (err) {
      lastError = err;
      if (!retryEnabled || !shouldRetryFn(undefined, attempt)) {
        throw err;
      }
    }

    // Do not wait after the last attempt.
    if (attempt < maxRetries) {
      const delay = computeDelay(attempt, baseDelayMs, maxDelayMs);
      await new Promise<void>((resolve) => setTimeout(resolve, delay));
    }
  }

  // All retries exhausted.
  if (lastError !== undefined) {
    throw lastError;
  }

  // Return the last (failed) response so the caller can inspect the status.
  return lastResponse!;
}

// ── Typed endpoint builders ──────────────────────────────────────────────────

export const endpoints = {
  /** Auth */
  auth: {
    login: () => `${API_BASE}/v1/auth/login`,
    register: () => `${API_BASE}/v1/auth/register`,
    logout: () => `${API_BASE}/v1/auth/logout`,
    refresh: () => `${API_BASE}/v1/auth/refresh`,
    sessions: () => `${API_BASE}/v1/auth/sessions`,
    revokeSession: (id: string) =>
      `${API_BASE}/v1/auth/sessions/${encodeURIComponent(id)}`,
    revokeAllSessions: () => `${API_BASE}/v1/auth/sessions/revoke-all`,
  },

  /** Matchmaking */
  matchmaking: {
    join: () => `${API_BASE}/v1/matchmaking/join`,
    cancel: () => `${API_BASE}/v1/matchmaking/cancel`,
    ws: (sessionId: string) =>
      `${WS_BASE}/v1/matchmaking/ws?session=${encodeURIComponent(sessionId)}`,
  },

  /** Games */
  games: {
    live: () => `${API_BASE}/v1/games/live`,
    ws: (gameId: string) =>
      `${WS_BASE}/v1/games/${encodeURIComponent(gameId)}/ws`,
    spectate: (gameId: string) =>
      `${WS_BASE}/v1/games/${encodeURIComponent(gameId)}/spectate`,
    archivePgn: () => `${API_BASE}/v1/games/archive-pgn`,
  },

  /** Enhanced game socket */
  enhancedGame: {
    ws: (gameId: string) =>
      `${WS_BASE}/v1/ws/game/${encodeURIComponent(gameId)}`,
  },

  /** Players */
  players: {
    online: () => `${API_BASE}/v1/players/online`,
  },

  /** Tournaments */
  tournaments: {
    list: () => `${API_BASE}/v1/tournaments`,
    create: () => `${API_BASE}/v1/tournaments`,
  },

  /** Profile & Preferences */
  profile: {
    preferences: () => `${API_BASE}/v1/profile/preferences`,
    theme: () => `${API_BASE}/v1/profile/theme`,
  },

  /** Horizon */
  horizon: {
    transaction: (hash: string) =>
      `${HORIZON_URL}/transactions/${encodeURIComponent(hash)}`,
  },

  /** Puzzles (#FE-68) */
  puzzles: {
    list: () => `${API_BASE}/v1/puzzles`,
    submit: () => `${API_BASE}/v1/puzzles/submit`,
  },
} as const;
