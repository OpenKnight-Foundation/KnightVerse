import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const ME = "GAAQCAIBAEAQCAIBAEAQCAIBAEAQCAIBAEAQCAIBAEAQCAIBAEAQDZ7H";
const OPPONENT = "GABAEAQCAIBAEAQCAIBAEAQCAIBAEAQCAIBAEAQCAIBAEAQCAIBAEJXA";

const auth = { user: null as null | { wallet_address?: string | null } };
const wallet = { address: ME as string | undefined };

vi.mock("@/context/authContext", () => ({ useAuth: () => auth }));
vi.mock("@/context/walletContext", () => ({ useAppContext: () => wallet }));

import { MATCHMAKING_POLL_INTERVAL_MS, useMatchmaking } from "@/hook/useMatchmaking";

function json(body: unknown, status = 200) {
  return Promise.resolve({ ok: status < 400, status, json: async () => body });
}

describe("useMatchmaking (#FE-98)", () => {
  let fetchMock: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    vi.useFakeTimers();
    auth.user = null;
    wallet.address = ME;
    fetchMock = vi.fn();
    vi.stubGlobal("fetch", fetchMock);
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.unstubAllGlobals();
  });

  async function flush() {
    await act(async () => {
      await Promise.resolve();
    });
  }

  it("joins the casual queue with the body the backend expects", async () => {
    fetchMock.mockReturnValueOnce(json({ status: "Added to queue", match_id: null, request_id: "req-1" }));
    const { result } = renderHook(() => useMatchmaking());

    await act(() => result.current.joinMatchmaking("Casual", "blitz"));

    const [url, init] = fetchMock.mock.calls[0];
    expect(url).toMatch(/\/v1\/matchmaking\/join-casual$/);
    expect(JSON.parse(init.body)).toEqual({
      wallet_address: ME,
      elo: 1200,
      invite_address: null,
      time_control: "blitz",
    });
    expect(result.current.status).toBe("searching");
  });

  it("uses the rated queue for rated games", async () => {
    fetchMock.mockReturnValueOnce(json({ status: "Added to queue", match_id: null, request_id: "req-2" }));
    const { result } = renderHook(() => useMatchmaking());

    await act(() => result.current.joinMatchmaking("Rated"));

    expect(fetchMock.mock.calls[0][0]).toMatch(/\/v1\/matchmaking\/join-rated-free$/);
    expect(JSON.parse(fetchMock.mock.calls[0][1].body)).toMatchObject({ max_elo_diff: null });
  });

  it("polls /status until matched, then reads the colour from /match", async () => {
    fetchMock
      .mockReturnValueOnce(json({ status: "Added to queue", match_id: null, request_id: "req-3" }))
      .mockReturnValueOnce(json({ status: "In queue", queue_status: { position: 1 } }))
      .mockReturnValueOnce(json({ status: "matched", match_id: "match-9" }))
      .mockReturnValueOnce(
        json({ id: "match-9", player1: { wallet_address: OPPONENT }, player2: { wallet_address: ME } }),
      );
    const { result } = renderHook(() => useMatchmaking());

    await act(() => result.current.joinMatchmaking("Casual"));
    await act(() => vi.advanceTimersByTimeAsync(MATCHMAKING_POLL_INTERVAL_MS));
    expect(result.current.status).toBe("searching");
    await act(() => vi.advanceTimersByTimeAsync(MATCHMAKING_POLL_INTERVAL_MS));
    await flush();

    expect(fetchMock.mock.calls[1][0]).toMatch(/\/v1\/matchmaking\/status\/req-3$/);
    expect(fetchMock.mock.calls[3][0]).toMatch(/\/v1\/matchmaking\/match\/match-9$/);
    expect(result.current.status).toBe("match_found");
    expect(result.current.gameId).toBe("match-9");
    expect(result.current.playerColor).toBe("black");
  });

  it("enters the match immediately when the join response already has one", async () => {
    fetchMock
      .mockReturnValueOnce(json({ status: "Match found", match_id: "match-1", request_id: "req-4" }))
      .mockReturnValueOnce(
        json({ id: "match-1", player1: { wallet_address: ME }, player2: { wallet_address: OPPONENT } }),
      );
    const { result } = renderHook(() => useMatchmaking());

    await act(() => result.current.joinMatchmaking("Casual"));
    await flush();

    expect(result.current.status).toBe("match_found");
    expect(result.current.playerColor).toBe("white");
  });

  it("prefers the linked wallet over the connected one (#FE-97)", async () => {
    auth.user = { wallet_address: OPPONENT };
    fetchMock.mockReturnValueOnce(json({ status: "Added to queue", match_id: null, request_id: "req-5" }));
    const { result } = renderHook(() => useMatchmaking());

    await act(() => result.current.joinMatchmaking("Casual"));

    expect(JSON.parse(fetchMock.mock.calls[0][1].body).wallet_address).toBe(OPPONENT);
  });

  it("fails clearly without a wallet and never calls the API", async () => {
    wallet.address = undefined;
    const { result } = renderHook(() => useMatchmaking());

    await act(() => result.current.joinMatchmaking("Casual"));

    expect(result.current.status).toBe("error");
    expect(result.current.error).toMatch(/connect your wallet/i);
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("stops with an error when the request disappears (404)", async () => {
    fetchMock
      .mockReturnValueOnce(json({ status: "Added to queue", match_id: null, request_id: "req-6" }))
      .mockReturnValueOnce(json({ status: "Request not found" }, 404));
    const { result } = renderHook(() => useMatchmaking());

    await act(() => result.current.joinMatchmaking("Casual"));
    await act(() => vi.advanceTimersByTimeAsync(MATCHMAKING_POLL_INTERVAL_MS));

    expect(result.current.status).toBe("error");
    expect(result.current.error).toMatch(/expired/i);
  });

  it("cancel posts the request id and stops polling", async () => {
    fetchMock
      .mockReturnValueOnce(json({ status: "Added to queue", match_id: null, request_id: "req-7" }))
      .mockReturnValue(json({ status: "cancelled" }));
    const { result } = renderHook(() => useMatchmaking());

    await act(() => result.current.joinMatchmaking("Casual"));
    act(() => result.current.cancelMatchmaking());
    await act(() => vi.advanceTimersByTimeAsync(MATCHMAKING_POLL_INTERVAL_MS * 3));

    const [url, init] = fetchMock.mock.calls[1];
    expect(url).toMatch(/\/v1\/matchmaking\/cancel$/);
    expect(JSON.parse(init.body)).toEqual({ request_id: "req-7" });
    expect(fetchMock).toHaveBeenCalledTimes(2);
    expect(result.current.status).toBe("idle");
  });
});
