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

  /** Horizon */
  horizon: {
    transaction: (hash: string) =>
      `${HORIZON_URL}/transactions/${encodeURIComponent(hash)}`,
  },
} as const;
