import { afterEach, describe, expect, it, vi } from "vitest";
import { fetchNativeBalance, fetchWalletBalances, HORIZON_URLS, TOKEN_ISSUERS } from "@/services/stellarMarketService";

const ADDRESS = "GAAQCAIBAEAQCAIBAEAQCAIBAEAQCAIBAEAQCAIBAEAQCAIBAEAQDZ7H";

function mockHorizon(balance: string) {
  const fetchMock = vi.fn().mockResolvedValue({
    ok: true,
    json: async () => ({ balances: [{ asset_type: "native", balance }] }),
  });
  vi.stubGlobal("fetch", fetchMock);
  return fetchMock;
}

describe("fetchNativeBalance", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("queries testnet Horizon when the network is testnet", async () => {
    const fetchMock = mockHorizon("42.5000000");

    await expect(fetchNativeBalance(ADDRESS, "testnet")).resolves.toBe(42.5);
    expect(fetchMock).toHaveBeenCalledWith(
      `${HORIZON_URLS.testnet}/accounts/${ADDRESS}`,
      expect.anything(),
    );
  });

  it("queries mainnet Horizon when the network is mainnet", async () => {
    const fetchMock = mockHorizon("7.0000000");

    await expect(fetchNativeBalance(ADDRESS, "mainnet")).resolves.toBe(7);
    expect(fetchMock).toHaveBeenCalledWith(
      `${HORIZON_URLS.mainnet}/accounts/${ADDRESS}`,
      expect.anything(),
    );
  });

  it("throws when Horizon cannot find the account", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue({ ok: false, json: async () => ({}) }));

    await expect(fetchNativeBalance(ADDRESS, "testnet")).rejects.toThrow("Unable to load wallet balance");
  });
});

describe("fetchWalletBalances", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  function mockAccount(balances: Array<Record<string, string>>) {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue({ ok: true, json: async () => ({ balances }) }));
  }

  it("returns each token's own balance, matched by code and issuer", async () => {
    mockAccount([
      { asset_type: "native", balance: "3.0000000" },
      { asset_type: "credit_alphanum4", asset_code: "USDC", asset_issuer: TOKEN_ISSUERS.testnet.USDC, balance: "250.0000000" },
      { asset_type: "credit_alphanum4", asset_code: "EURC", asset_issuer: TOKEN_ISSUERS.testnet.EURC, balance: "80.5000000" },
    ]);

    await expect(fetchWalletBalances(ADDRESS, "testnet")).resolves.toEqual({ XLM: 3, USDC: 250, EURC: 80.5 });
  });

  it("ignores same-code assets from other issuers and treats missing trustlines as 0", async () => {
    mockAccount([
      { asset_type: "native", balance: "10.0000000" },
      { asset_type: "credit_alphanum4", asset_code: "USDC", asset_issuer: "GAAQCAIBAEAQCAIBAEAQCAIBAEAQCAIBAEAQCAIBAEAQCAIBAEAQDZ7H", balance: "999.0000000" },
    ]);

    await expect(fetchWalletBalances(ADDRESS, "mainnet")).resolves.toEqual({ XLM: 10, USDC: 0, EURC: 0 });
  });

  it("uses the issuer for the selected network", async () => {
    mockAccount([
      { asset_type: "credit_alphanum4", asset_code: "USDC", asset_issuer: TOKEN_ISSUERS.mainnet.USDC, balance: "50.0000000" },
    ]);

    await expect(fetchWalletBalances(ADDRESS, "testnet")).resolves.toMatchObject({ USDC: 0 });
  });
});
