/**
 * Centralized API configuration (#FE-28)
 *
 * Single source of truth for all API endpoints and host URLs.
 * Pulls values from environment variables with sensible defaults.
 * All modules should import from this file instead of reading
 * `process.env` directly.
 */

/**
 * Returns `value` unless it is unset or empty.
 *
 * Every call site below passes `process.env.NEXT_PUBLIC_…` as a *literal*
 * property access. Next.js only inlines `NEXT_PUBLIC_*` variables into the
 * client bundle when they are written that way; indexing `process.env` with a
 * variable is left as-is and is always `undefined` in the browser (#FE-86).
 * Don't refactor these back into a name-based helper.
 */
function withDefault(value: string | undefined, fallback: string): string {
  return value ? value : fallback;
}

/** Backend / game-server base URL (REST + WebSocket) */
export const API_BASE = withDefault(process.env.NEXT_PUBLIC_API_URL, "http://localhost:8080");

/** Derive the WebSocket base from the HTTP base automatically */
export const WS_BASE = API_BASE.replace(/^http/, "ws");

/** Legacy alias — some older services read `NEXT_PUBLIC_BACKEND_URL` */
export const BACKEND_BASE = withDefault(process.env.NEXT_PUBLIC_BACKEND_URL, API_BASE);

/** Stellar Horizon API */
export const HORIZON_URL = withDefault(
  process.env.NEXT_PUBLIC_HORIZON_URL,
  "https://horizon-testnet.stellar.org",
);

/** Soroban RPC endpoint */
export const SOROBAN_RPC = withDefault(
  process.env.NEXT_PUBLIC_SOROBAN_RPC,
  "https://soroban-testnet.stellar.org:443",
);

/** Stellar network passphrase */
export const NETWORK_PASSPHRASE = withDefault(
  process.env.NEXT_PUBLIC_NETWORK_PASSPHRASE,
  "Test SDF Network ; September 2015",
);

/** IPFS gateway for decentralized storage */
export const IPFS_GATEWAY = withDefault(
  process.env.NEXT_PUBLIC_IPFS_GATEWAY,
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
    joinCasual: () => `${API_BASE}/v1/matchmaking/join-casual`,
    joinRated: () => `${API_BASE}/v1/matchmaking/join-rated-free`,
    joinStaked: () => `${API_BASE}/v1/matchmaking/join-rated-staked`,
    status: (requestId: string) =>
      `${API_BASE}/v1/matchmaking/status/${encodeURIComponent(requestId)}`,
    match: (matchId: string) =>
      `${API_BASE}/v1/matchmaking/match/${encodeURIComponent(matchId)}`,
    cancel: () => `${API_BASE}/v1/matchmaking/cancel`,
  },

  /** Games */
  games: {
    live: () => `${API_BASE}/v1/games/live`,
    /** Game socket — served by the backend at `/v1/ws/game/{game_id}`. */
    ws: (gameId: string) =>
      `${WS_BASE}/v1/ws/game/${encodeURIComponent(gameId)}`,
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
    /**
     * Live online-player count.
     *
     * NOTE (#FE-102): the backend does not expose this route yet, so callers
     * must hide the counter gracefully when a request fails instead of polling
     * it on a tight interval. Pair with the backend "players online" endpoint
     * issue before surfacing a count to users.
     */
    online: () => `${API_BASE}/v1/players/online`,
    export: () => `${API_BASE}/v1/players/me/export`,
    preferences: () => `${API_BASE}/v1/players/me/preferences`,
    /**
     * Rating (ELO) history for a single player.
     *
     * NOTE (#FE-103): players are keyed by their UUID / wallet address on the
     * backend, *not* by the integer `user_id`. Pass the player identifier the
     * backend expects. Pairs with the backend player-stats endpoint issue.
     */
    eloHistory: (playerId: string) =>
      `${API_BASE}/v1/players/${encodeURIComponent(playerId)}/elo-history`,
    /** Wallet linking (#FE-97): challenge → Freighter signMessage → verify. */
    walletChallenge: () => `${API_BASE}/v1/players/me/wallet/challenge`,
    walletVerify: () => `${API_BASE}/v1/players/me/wallet/verify`,
    wallet: () => `${API_BASE}/v1/players/me/wallet`,
  },

  /** Tournaments */
  /**
   * Tournaments — the backend mounts these under its JWT-protected
  * `/v1/tournaments` scope. Only `register` exists server-side today;
   * list/create follow the same scope (see BE-114).
   */
  tournaments: {
    list: () => `${API_BASE}/v1/tournaments`,
    create: () => `${API_BASE}/v1/tournaments`,
    register: (id: string) =>
      `${API_BASE}/v1/tournaments/${encodeURIComponent(id)}/register`,
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
