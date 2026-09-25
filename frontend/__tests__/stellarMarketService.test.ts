import { afterEach, describe, expect, it, vi } from "vitest";
import { fetchNativeBalance, HORIZON_URLS } from "@/services/stellarMarketService";

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
