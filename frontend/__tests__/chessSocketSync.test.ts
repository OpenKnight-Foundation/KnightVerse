import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi, type Mock } from "vitest";
import { useChessSocket } from "@/hook/useChessSocket";

class MockWebSocket {
  static instances: MockWebSocket[] = [];
  static OPEN = 1;
  static CONNECTING = 0;
  static CLOSED = 3;

  url: string;
  readyState = MockWebSocket.CONNECTING;
  onopen: (() => void) | null = null;
  onmessage: ((event: { data: string }) => void) | null = null;
  onerror: (() => void) | null = null;
  onclose: ((event: { code: number; reason: string }) => void) | null = null;

  send: Mock = vi.fn();

  constructor(url: string) {
    this.url = url;
    MockWebSocket.instances.push(this);
  }

  close(code = 1000, reason = "") {
    this.readyState = MockWebSocket.CLOSED;
    this.onclose?.({ code, reason });
  }

  emitOpen() {
    this.readyState = MockWebSocket.OPEN;
    this.onopen?.();
  }

  emitMessage(data: unknown) {
    this.onmessage?.({ data: JSON.stringify(data) });
  }
}

describe("useChessSocket sync & offline queue (FE-113 / FE-114)", () => {
  const originalWebSocket = globalThis.WebSocket;

  beforeEach(() => {
    MockWebSocket.instances = [];
    vi.stubGlobal("WebSocket", MockWebSocket as unknown as typeof WebSocket);
    // Game sockets authenticate with a single-use ticket fetched with the
    // signed-in user's access token (BE-103), so provide both.
    localStorage.setItem("access_token", "test-access-token");
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: true,
        json: async () => ({ ticket: "single-use-ticket", expires_in: 60 }),
      }),
    );
  });

  afterEach(() => {
    localStorage.removeItem("access_token");
    vi.unstubAllGlobals();
    globalThis.WebSocket = originalWebSocket;
  });

  it("exposes the server's synced board after a reconnect so missed moves are applied", async () => {
    const { result } = renderHook(() => useChessSocket("game-9"));

    await waitFor(() => expect(MockWebSocket.instances.length).toBe(1));
    const socket = MockWebSocket.instances[0];

    act(() => socket.emitOpen());
    act(() => {
      socket.emitMessage({
        type: "sync",
        fen: "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 2",
        moves: [
          { from: "e2", to: "e4", san: "e4", color: "w" },
          { from: "e7", to: "e5", san: "e5", color: "b" },
        ],
        whiteTime: 598,
        blackTime: 599,
        status: "playing",
        lastPly: 2,
      });
    });

    await waitFor(() => {
      expect(result.current.lastSyncedState?.fen).toContain("4p3");
      expect(result.current.lastSyncedState?.moves).toHaveLength(2);
      expect(result.current.lastSyncedState?.whiteTime).toBe(598);
      expect(result.current.lastSyncedState?.lastPly).toBe(2);
    });
  });

  it("stamps the ply index on every outgoing move", async () => {
    const { result } = renderHook(() => useChessSocket("game-10"));

    await waitFor(() => expect(MockWebSocket.instances.length).toBe(1));
    const socket = MockWebSocket.instances[0];

    act(() => socket.emitOpen());
    act(() => result.current.sendMove({ from: "e2", to: "e4", ply: 1 }));

    await waitFor(() => expect(socket.send).toHaveBeenCalled());
    const payload = JSON.parse(socket.send.mock.calls.at(-1)?.[0] as string);
    expect(payload).toMatchObject({ type: "move", from: "e2", to: "e4", ply: 1 });
  });

  it("drops queued moves the server has already acknowledged", async () => {
    const { result } = renderHook(() => useChessSocket("game-11"));

    await waitFor(() => expect(MockWebSocket.instances.length).toBe(1));
    const socket = MockWebSocket.instances[0];

    act(() => socket.emitOpen());

    // Simulate the socket dropping so the next move is queued offline.
    act(() => {
      socket.readyState = MockWebSocket.CLOSED;
      result.current.sendMove({ from: "g1", to: "f3", ply: 3 });
    });

    // The server acks ply 3 — the queued move must not be re-sent.
    act(() => socket.emitMessage({ type: "ack", ply: 3 }));
    act(() => {
      socket.readyState = MockWebSocket.OPEN;
      const sendsBefore = (socket.send as Mock).mock.calls.length;
      // Manual reconnect requests sync; the acked move should be dropped on flush.
      socket.emitMessage({ type: "sync", fen: "", moves: [], lastPly: 3 });
      expect((socket.send as Mock).mock.calls.length).toBe(sendsBefore + 0);
    });
  });
});
