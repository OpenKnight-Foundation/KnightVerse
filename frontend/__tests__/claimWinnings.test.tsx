/**
 * FE-163: claim on-chain winnings after a won staked game.
 */
import { act, render, renderHook, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const ADDRESS = "GPLAYERXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX";
const GAME_ID = "6f1d1a2c-0f1e-4b4e-9d43-2b0b2f1a1234";
const SIGNATURE = Buffer.alloc(64, 7);

const wallet = {
  address: ADDRESS as string | undefined,
  signTransaction: vi.fn(async (xdr: string) => `signed:${xdr}`),
};
const auth = {
  accessToken: "token-123" as string | null,
  refreshToken: vi.fn(async () => {}),
  logout: vi.fn(async () => {}),
};

const claimGameWin = vi.fn();

vi.mock("@/context/walletContext", () => ({ useAppContext: () => wallet }));
vi.mock("@/context/authContext", () => ({ useAuth: () => auth }));
vi.mock("@/lib/api", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/api")>();
  return { ...actual, authFetch: (...args: unknown[]) => authFetchMock(...args) };
});
vi.mock("@/contracts/client", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/contracts/client")>();
  return {
    ...actual,
    claimGameWin: (...args: unknown[]) => claimGameWin(...args),
    getGameContractId: () => "CGAMECONTRACTXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX",
  };
});

const authFetchMock = vi.fn();

import { useClaimWinnings } from "@/hook/useClaimWinnings";
import { ClaimWinningsPanel } from "@/components/ClaimWinningsPanel";

function jsonResponse(body: unknown, status = 200) {
  return {
    ok: status >= 200 && status < 300,
    status,
    json: async () => body,
  } as unknown as Response;
}

const signedClaim = {
  game_id: "42",
  winner: ADDRESS,
  signature: SIGNATURE.toString("base64"),
};

const stakedMatch = { id: GAME_ID, stake_info: { token: "USDC", amount: 100 } };

function stubFetch(match: unknown = stakedMatch) {
  vi.stubGlobal(
    "fetch",
    vi.fn(async () => jsonResponse(match)),
  );
}

describe("useClaimWinnings (#FE-163)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    wallet.address = ADDRESS;
    auth.accessToken = "token-123";
    stubFetch();
    authFetchMock.mockResolvedValue(jsonResponse(signedClaim));
    claimGameWin.mockResolvedValue({ hash: "txhash123" });
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("stays unavailable when the player did not win", async () => {
    const { result } = renderHook(() =>
      useClaimWinnings({ gameId: GAME_ID, enabled: false }),
    );
    await waitFor(() => expect(result.current.status).toBe("idle"));
    expect(result.current.isUnavailable).toBe(true);
  });

  it("stays unavailable for an unstaked match", async () => {
    stubFetch({ id: GAME_ID, stake_info: null });
    const { result } = renderHook(() =>
      useClaimWinnings({ gameId: GAME_ID, enabled: true }),
    );
    await waitFor(() => expect(fetch).toHaveBeenCalled());
    expect(result.current.stake).toBeNull();
    expect(result.current.isUnavailable).toBe(true);
  });

  it("exposes the wager for a staked win", async () => {
    const { result } = renderHook(() =>
      useClaimWinnings({ gameId: GAME_ID, enabled: true }),
    );
    await waitFor(() =>
      expect(result.current.stake).toEqual({ token: "USDC", amount: 100 }),
    );
    expect(result.current.isUnavailable).toBe(false);
  });

  it("fetches the signed result, signs, and reports the transaction hash", async () => {
    const { result } = renderHook(() =>
      useClaimWinnings({ gameId: GAME_ID, enabled: true }),
    );
    await waitFor(() => expect(result.current.isUnavailable).toBe(false));

    await act(async () => {
      await result.current.claimWinnings();
    });

    expect(authFetchMock).toHaveBeenCalledWith(
      expect.stringContaining(`/v1/games/${GAME_ID}/win-claim`),
      {},
      expect.objectContaining({ getToken: expect.any(Function) }),
    );
    const passed = claimGameWin.mock.calls[0][0];
    expect(passed.claim.game_id).toBe(BigInt(42));
    expect(passed.claim.winner).toBe(ADDRESS);
    expect(Buffer.from(passed.claim.signature).equals(SIGNATURE)).toBe(true);
    expect(passed.signer.address).toBe(ADDRESS);
    expect(typeof passed.signer.sign).toBe("function");
    expect(result.current.status).toBe("confirmed");
    expect(result.current.hash).toBe("txhash123");
    expect(result.current.error).toBeNull();
  });

  it("reports progress while the wallet signs", async () => {
    const seen: string[] = [];
    claimGameWin.mockImplementation(async ({ signer }: { signer: { sign: (x: string) => Promise<string> } }) => {
      seen.push("submitting");
      await signer.sign("envelope-xdr");
      return { hash: "txhash123" };
    });

    const { result } = renderHook(() =>
      useClaimWinnings({ gameId: GAME_ID, enabled: true }),
    );
    await waitFor(() => expect(result.current.isUnavailable).toBe(false));

    await act(async () => {
      await result.current.claimWinnings();
    });

    expect(wallet.signTransaction).toHaveBeenCalledWith("envelope-xdr");
    expect(seen).toEqual(["submitting"]);
    expect(result.current.status).toBe("confirmed");
  });

  it("surfaces an error and allows a retry", async () => {
    claimGameWin.mockRejectedValueOnce(new Error("User declined in Freighter"));

    const { result } = renderHook(() =>
      useClaimWinnings({ gameId: GAME_ID, enabled: true }),
    );
    await waitFor(() => expect(result.current.isUnavailable).toBe(false));

    await act(async () => {
      await result.current.claimWinnings();
    });
    expect(result.current.status).toBe("error");
    expect(result.current.error).toBe("User declined in Freighter");

    await act(async () => {
      await result.current.claimWinnings();
    });
    expect(result.current.status).toBe("confirmed");
    expect(result.current.hash).toBe("txhash123");
  });

  it("refuses a signed result that names a different winner", async () => {
    authFetchMock.mockResolvedValue(
      jsonResponse({ ...signedClaim, winner: "GSOMEONEELSE" }),
    );

    const { result } = renderHook(() =>
      useClaimWinnings({ gameId: GAME_ID, enabled: true }),
    );
    await waitFor(() => expect(result.current.isUnavailable).toBe(false));

    await act(async () => {
      await result.current.claimWinnings();
    });

    expect(result.current.status).toBe("error");
    expect(result.current.error).toMatch(/different winner/);
    expect(claimGameWin).not.toHaveBeenCalled();
  });

  it("explains that signed results are not available yet", async () => {
    authFetchMock.mockResolvedValue(jsonResponse({}, 404));

    const { result } = renderHook(() =>
      useClaimWinnings({ gameId: GAME_ID, enabled: true }),
    );
    await waitFor(() => expect(result.current.isUnavailable).toBe(false));

    await act(async () => {
      await result.current.claimWinnings();
    });

    expect(result.current.status).toBe("error");
    expect(result.current.error).toMatch(/not available yet/);
  });
});

describe("ClaimWinningsPanel (#FE-163)", () => {
  it("shows a claim button, progress, and retry states", () => {
    const { rerender } = render(
      <ClaimWinningsPanel status="idle" hash={null} error={null} onClaim={vi.fn()} />,
    );
    expect(screen.getByRole("button", { name: "Claim winnings" })).toBeEnabled();

    rerender(
      <ClaimWinningsPanel status="signing" hash={null} error={null} onClaim={vi.fn()} />,
    );
    expect(screen.getByRole("button")).toBeDisabled();
    expect(screen.getByRole("button")).toHaveTextContent(/Freighter/);

    rerender(
      <ClaimWinningsPanel
        status="error"
        hash={null}
        error="User declined"
        onClaim={vi.fn()}
      />,
    );
    expect(screen.getByRole("alert")).toHaveTextContent("User declined");
    expect(screen.getByRole("button")).toHaveTextContent(/Retry/);
  });

  it("shows the full transaction hash once confirmed", () => {
    render(
      <ClaimWinningsPanel
        status="confirmed"
        hash="abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789"
        error={null}
        onClaim={vi.fn()}
      />,
    );
    expect(screen.getByText(/Winnings claimed/)).toBeInTheDocument();
    expect(
      screen.getByText(/abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789/),
    ).toBeInTheDocument();
    expect(screen.queryByRole("button")).toBeNull();
  });
});
