import { describe, expect, it } from "vitest";
import { buildConnectSrc, buildCsp } from "@/lib/csp";

const MAINNET = "Public Global Stellar Network ; September 2015";
const TESTNET = "Test SDF Network ; September 2015";

describe("buildConnectSrc (#FE-87)", () => {
  it("allows the configured API over both HTTP(S) and WebSocket", () => {
    const sources = buildConnectSrc({ NEXT_PUBLIC_API_URL: "https://api.knightverse.example/base" });

    expect(sources).toContain("https://api.knightverse.example");
    expect(sources).toContain("wss://api.knightverse.example");
  });

  it("defaults to the local backend when no API URL is set", () => {
    const sources = buildConnectSrc({});

    expect(sources).toEqual(expect.arrayContaining(["'self'", "http://localhost:8000", "ws://localhost:8000"]));
  });

  it("allows the third-party hosts the app calls", () => {
    const sources = buildConnectSrc({});

    expect(sources).toContain("https://api.coingecko.com");
    expect(sources).toContain("https://gateway.pinata.cloud");
    expect(sources).toContain("https://arweave.net");
  });

  it("allows testnet Stellar hosts by default and mainnet Horizon only on mainnet", () => {
    const testnet = buildConnectSrc({ NEXT_PUBLIC_NETWORK_PASSPHRASE: TESTNET });
    expect(testnet).toContain("https://horizon-testnet.stellar.org");
    expect(testnet).toContain("https://soroban-testnet.stellar.org");
    expect(testnet).not.toContain("https://horizon.stellar.org");

    const mainnet = buildConnectSrc({
      NEXT_PUBLIC_NETWORK_PASSPHRASE: MAINNET,
      NEXT_PUBLIC_HORIZON_URL: "https://horizon.stellar.org",
      NEXT_PUBLIC_SOROBAN_RPC: "https://rpc.example-provider.com:443/soroban",
    });
    expect(mainnet).toContain("https://horizon.stellar.org");
    expect(mainnet).toContain("https://rpc.example-provider.com");
    expect(mainnet).not.toContain("https://horizon-testnet.stellar.org");
  });

  it("allows the Sentry ingest host from the DSN without leaking the key", () => {
    const sources = buildConnectSrc({ NEXT_PUBLIC_SENTRY_DSN: "https://abc123@o42.ingest.sentry.io/7" });

    expect(sources).toContain("https://o42.ingest.sentry.io");
    expect(sources.join(" ")).not.toContain("abc123");
  });

  it("ignores malformed URLs instead of emitting a broken policy", () => {
    const sources = buildConnectSrc({ NEXT_PUBLIC_API_URL: "not a url", NEXT_PUBLIC_SENTRY_DSN: "::" });

    expect(sources.every((s) => s === "'self'" || /^(https?|wss?):\/\/[^\s/]+$/.test(s))).toBe(true);
  });

  it("puts the list into the connect-src directive", () => {
    const csp = buildCsp({ NEXT_PUBLIC_API_URL: "https://api.knightverse.example" });

    expect(csp).toMatch(/connect-src 'self' [^;]*https:\/\/api\.knightverse\.example/);
  });
});
