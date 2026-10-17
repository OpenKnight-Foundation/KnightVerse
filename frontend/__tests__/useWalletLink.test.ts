import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const ADDRESS = "GAAQCAIBAEAQCAIBAEAQCAIBAEAQCAIBAEAQCAIBAEAQCAIBAEAQDZ7H";

const setLinkedWallet = vi.fn();
const auth = {
  accessToken: "token-123" as string | null,
  user: { user_id: 1, username: "knight", wallet_address: null as string | null },
  setLinkedWallet,
};
const wallet = { address: ADDRESS as string | undefined };

vi.mock("@/context/authContext", () => ({ useAuth: () => auth }));
vi.mock("@/context/walletContext", () => ({ useAppContext: () => wallet }));

import { normalizeSignature, useWalletLink } from "@/hook/useWalletLink";

describe("normalizeSignature", () => {
  it("passes through a plain string", () => {
    expect(normalizeSignature("c2ln")).toBe("c2ln");
  });

  it("reads `signedMessage` strings and byte arrays", () => {
    expect(normalizeSignature({ signedMessage: "c2ln" })).toBe("c2ln");
    expect(normalizeSignature({ signedMessage: new Uint8Array([115, 105, 103]) })).toBe("c2ln");
    expect(normalizeSignature({ signedMessage: { data: [115, 105, 103] } })).toBe("c2ln");
  });

  it("throws on a Freighter error or missing signature", () => {
    expect(() => normalizeSignature({ error: { message: "User declined" } })).toThrow("User declined");
    expect(() => normalizeSignature({ signedMessage: null })).toThrow(/did not return/);
  });
});

describe("useWalletLink (#FE-97)", () => {
  let fetchMock: ReturnType<typeof vi.fn>;
  let signMessage: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    auth.accessToken = "token-123";
    auth.user.wallet_address = null;
    wallet.address = ADDRESS;
    setLinkedWallet.mockReset();
    fetchMock = vi.fn();
    vi.stubGlobal("fetch", fetchMock);
    signMessage = vi.fn().mockResolvedValue({ signedMessage: "c2lnbmF0dXJl" });
    (window as unknown as { freighterApi: unknown }).freighterApi = { signMessage };
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    delete (window as unknown as { freighterApi?: unknown }).freighterApi;
  });

  it("requests a challenge, signs it with Freighter, verifies, and records the wallet", async () => {
    fetchMock
      .mockResolvedValueOnce({ ok: true, json: async () => ({ message: "link me", expires_in: 300 }) })
      .mockResolvedValueOnce({ ok: true, json: async () => ({ wallet_address: ADDRESS }) });
    const { result } = renderHook(() => useWalletLink());

    await act(() => result.current.linkWallet());

    const [challengeUrl, challengeInit] = fetchMock.mock.calls[0];
    expect(challengeUrl).toMatch(/\/v1\/players\/me\/wallet\/challenge$/);
    expect(challengeInit.headers.Authorization).toBe("Bearer token-123");
    expect(JSON.parse(challengeInit.body)).toEqual({ wallet_address: ADDRESS });

    expect(signMessage).toHaveBeenCalledWith("link me", { address: ADDRESS });

    const [verifyUrl, verifyInit] = fetchMock.mock.calls[1];
    expect(verifyUrl).toMatch(/\/v1\/players\/me\/wallet\/verify$/);
    expect(JSON.parse(verifyInit.body)).toEqual({ wallet_address: ADDRESS, signature: "c2lnbmF0dXJl" });

    expect(setLinkedWallet).toHaveBeenCalledWith(ADDRESS);
    expect(result.current.error).toBeNull();
  });

  it("surfaces the backend's error when verification fails", async () => {
    fetchMock
      .mockResolvedValueOnce({ ok: true, json: async () => ({ message: "link me", expires_in: 300 }) })
      .mockResolvedValueOnce({ ok: false, status: 401, json: async () => ({ message: "Signature verification failed" }) });
    const { result } = renderHook(() => useWalletLink());

    await act(() => result.current.linkWallet());

    expect(result.current.error).toBe("Signature verification failed");
    expect(setLinkedWallet).not.toHaveBeenCalled();
  });

  it("requires sign-in and a connected wallet", async () => {
    auth.accessToken = null;
    const { result, rerender } = renderHook(() => useWalletLink());
    await act(() => result.current.linkWallet());
    expect(result.current.error).toMatch(/sign in/i);

    auth.accessToken = "token-123";
    wallet.address = undefined;
    rerender();
    await act(() => result.current.linkWallet());
    expect(result.current.error).toMatch(/connect your freighter/i);
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("flags a connected wallet that differs from the linked one", () => {
    auth.user.wallet_address = "GABAEAQCAIBAEAQCAIBAEAQCAIBAEAQCAIBAEAQCAIBAEAQCAIBAEJXA";
    const { result } = renderHook(() => useWalletLink());

    expect(result.current.isMismatched).toBe(true);
  });
});
