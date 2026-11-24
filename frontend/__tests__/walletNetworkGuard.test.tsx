/**
 * FE-164: the wallet context must refuse to sign when Freighter is on a
 * different Stellar network than the app.
 */
import { act, renderHook, waitFor } from "@testing-library/react";
import React from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mockGetPublicKey = vi.fn();
const mockSignTransaction = vi.fn();
const mockGetNetworkDetails = vi.fn();

vi.mock("@stellar/freighter-api", () => ({
  getPublicKey: (...args: unknown[]) => mockGetPublicKey(...args),
  signTransaction: (...args: unknown[]) => mockSignTransaction(...args),
  getNetwork: vi.fn(),
  getNetworkDetails: (...args: unknown[]) => mockGetNetworkDetails(...args),
  isConnected: vi.fn().mockResolvedValue(true),
  isAllowed: vi.fn().mockResolvedValue(true),
}));

vi.mock("@/lib/api", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/api")>();
  return {
    ...actual,
    NETWORK_PASSPHRASE: "Test SDF Network ; September 2015",
  };
});

import { AppProvider, useAppContext } from "@/context/walletContext";
import { MAINNET_PASSPHRASE, TESTNET_PASSPHRASE } from "@/lib/walletNetwork";

const ADDRESS = "GBRPYHIL2CI3FNQ4BXLFMNDLFJUNPU2HY3ZMFSHONUCEOASW7QC7OX2H";

const wrapper = ({ children }: { children: React.ReactNode }) => (
  <AppProvider>{children}</AppProvider>
);

function setFreighter(networkDetails?: Record<string, unknown>) {
  (window as unknown as { freighterApi: unknown }).freighterApi = {
    getPublicKey: mockGetPublicKey,
    signTransaction: mockSignTransaction,
    getNetworkDetails: mockGetNetworkDetails,
  };
  if (networkDetails) {
    mockGetNetworkDetails.mockResolvedValue(networkDetails);
  }
}

describe("AppProvider network guard (#FE-164)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockGetPublicKey.mockResolvedValue(ADDRESS);
    mockSignTransaction.mockResolvedValue("signed-xdr");
    window.localStorage.clear();
  });

  afterEach(() => {
    delete (window as unknown as { freighterApi?: unknown }).freighterApi;
  });

  it("reports a mismatch after connecting a mainnet wallet to a testnet app", async () => {
    setFreighter({ network: "PUBLIC", networkPassphrase: MAINNET_PASSPHRASE });

    const { result } = renderHook(() => useAppContext(), { wrapper });
    await act(async () => {
      await result.current.connectWallet();
    });

    await waitFor(() => expect(result.current.networkMismatch).toBe(true));
    expect(result.current.walletNetwork).toBe(MAINNET_PASSPHRASE);
    expect(result.current.networkMismatchMessage).toContain("Mainnet");
    expect(result.current.networkMismatchMessage).toContain("Testnet");
  });

  it("reports no mismatch when the wallet is on the app's network", async () => {
    setFreighter({ network: "TESTNET", networkPassphrase: TESTNET_PASSPHRASE });

    const { result } = renderHook(() => useAppContext(), { wrapper });
    await act(async () => {
      await result.current.connectWallet();
    });

    await waitFor(() => expect(result.current.walletNetwork).toBe(TESTNET_PASSPHRASE));
    expect(result.current.networkMismatch).toBe(false);
    expect(result.current.networkMismatchMessage).toBeUndefined();
  });

  it("blocks signing on a mismatched network and explains why", async () => {
    setFreighter({ network: "PUBLIC", networkPassphrase: MAINNET_PASSPHRASE });

    const { result } = renderHook(() => useAppContext(), { wrapper });
    await act(async () => {
      await result.current.connectWallet();
    });

    await expect(
      result.current.signTransaction("unsigned-xdr"),
    ).rejects.toThrow(/Mainnet/);

    expect(mockSignTransaction).not.toHaveBeenCalled();
  });

  it("catches a network switch made after connecting", async () => {
    setFreighter({ network: "TESTNET", networkPassphrase: TESTNET_PASSPHRASE });

    const { result } = renderHook(() => useAppContext(), { wrapper });
    await act(async () => {
      await result.current.connectWallet();
    });
    expect(result.current.networkMismatch).toBe(false);

    mockGetNetworkDetails.mockResolvedValue({
      network: "PUBLIC",
      networkPassphrase: MAINNET_PASSPHRASE,
    });

    await expect(
      result.current.signTransaction("unsigned-xdr"),
    ).rejects.toThrow(/Mainnet/);
    expect(mockSignTransaction).not.toHaveBeenCalled();
  });

  it("still signs when the wallet's network cannot be read", async () => {
    setFreighter();
    mockGetNetworkDetails.mockRejectedValue(new Error("not supported"));

    const { result } = renderHook(() => useAppContext(), { wrapper });
    await act(async () => {
      await result.current.connectWallet();
    });

    expect(result.current.networkMismatch).toBe(false);
    await expect(
      result.current.signTransaction("unsigned-xdr"),
    ).resolves.toBe("signed-xdr");
  });
});
