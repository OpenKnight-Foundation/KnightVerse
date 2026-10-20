/**
 * Content-Security-Policy builder (#FE-87).
 *
 * `next.config.ts` runs in Node at build/start time, so it can read the same
 * `NEXT_PUBLIC_*` values that `lib/api.ts` bakes into the client bundle and
 * allow exactly the origins the browser will talk to.
 */

const MAINNET_PASSPHRASE = "Public Global Stellar Network ; September 2015";

const STELLAR_HOSTS = {
  // Mainnet has no SDF-hosted Soroban RPC; the configured NEXT_PUBLIC_SOROBAN_RPC is added below.
  mainnet: ["https://horizon.stellar.org"],
  testnet: ["https://horizon-testnet.stellar.org", "https://soroban-testnet.stellar.org"],
} as const;

/** Third-party hosts the app calls regardless of configuration. */
const STATIC_CONNECT_HOSTS = [
  "https://api.coingecko.com", // staking modal token prices
  "https://arweave.net", // game archive / NFT metadata
];

export type CspEnv = {
  // Lets callers pass `process.env` directly.
  [name: string]: string | undefined;
  NEXT_PUBLIC_API_URL?: string;
  NEXT_PUBLIC_BACKEND_URL?: string;
  NEXT_PUBLIC_HORIZON_URL?: string;
  NEXT_PUBLIC_SOROBAN_RPC?: string;
  NEXT_PUBLIC_NETWORK_PASSPHRASE?: string;
  NEXT_PUBLIC_IPFS_GATEWAY?: string;
  NEXT_PUBLIC_SENTRY_DSN?: string;
};

/** `https://host:port/path` → `https://host:port`; returns null for unset/invalid values. */
function originOf(url: string | undefined): string | null {
  if (!url) return null;
  try {
    return new URL(url).origin;
  } catch {
    return null;
  }
}

/** The WebSocket origin matching an HTTP(S) origin (`https:` → `wss:`). */
function wsOriginOf(httpOrigin: string): string {
  return httpOrigin.replace(/^http/, "ws");
}

/** Builds the `connect-src` source list for the configured deployment. */
export function buildConnectSrc(env: CspEnv): string[] {
  const sources = new Set<string>(["'self'"]);

  for (const value of [env.NEXT_PUBLIC_API_URL ?? "http://localhost:8000", env.NEXT_PUBLIC_BACKEND_URL]) {
    const origin = originOf(value);
    if (origin) {
      sources.add(origin);
      sources.add(wsOriginOf(origin));
    }
  }

  const network = env.NEXT_PUBLIC_NETWORK_PASSPHRASE === MAINNET_PASSPHRASE ? "mainnet" : "testnet";
  for (const host of STELLAR_HOSTS[network]) sources.add(host);
  for (const value of [env.NEXT_PUBLIC_HORIZON_URL, env.NEXT_PUBLIC_SOROBAN_RPC]) {
    const origin = originOf(value);
    if (origin) sources.add(origin);
  }

  sources.add(originOf(env.NEXT_PUBLIC_IPFS_GATEWAY) ?? "https://gateway.pinata.cloud");
  for (const host of STATIC_CONNECT_HOSTS) sources.add(host);

  // Sentry DSNs look like https://<key>@o123.ingest.sentry.io/<project>.
  const sentryOrigin = originOf(env.NEXT_PUBLIC_SENTRY_DSN);
  if (sentryOrigin) sources.add(sentryOrigin);

  return [...sources];
}

export function buildCsp(env: CspEnv): string {
  return [
    "default-src 'self'",
    "script-src 'self' 'unsafe-eval' 'unsafe-inline' https://fonts.googleapis.com blob:",
    "style-src 'self' 'unsafe-inline' https://fonts.googleapis.com",
    "img-src 'self' data: blob: https://gateway.pinata.cloud https://arweave.net",
    "font-src 'self' https://fonts.gstatic.com",
    `connect-src ${buildConnectSrc(env).join(" ")}`,
    "worker-src 'self' blob:",
    "frame-ancestors 'none'",
    "base-uri 'self'",
    "form-action 'self'",
  ].join("; ");
}
