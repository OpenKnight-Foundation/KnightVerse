import { act, renderHook, waitFor } from "@testing-library/react";
import { useSpectatorSocket } from "@/hook/useSpectatorSocket";

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

  constructor(url: string) {
    this.url = url;
    MockWebSocket.instances.push(this);
  }

  send = vi.fn();

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

describe("useSpectatorSocket", () => {
  const originalWebSocket = globalThis.WebSocket;

  beforeEach(() => {
    MockWebSocket.instances = [];
    vi.stubGlobal("WebSocket", MockWebSocket as unknown as typeof WebSocket);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    globalThis.WebSocket = originalWebSocket;
  });

  it("initializes with idle status and null game state when no game is selected", () => {
    const { result } = renderHook(() => useSpectatorSocket(null));

    expect(result.current.status).toBe("idle");
    expect(result.current.gameState).toBeNull();
  });

  it("exposes disconnect and reconnect callbacks", () => {
    const { result } = renderHook(() => useSpectatorSocket(null));

    expect(typeof result.current.disconnect).toBe("function");
    expect(typeof result.current.reconnect).toBe("function");
  });

  it("stores sync state for live websocket games", async () => {
    const { result } = renderHook(() => useSpectatorSocket("live-game-42"));

    await waitFor(() => {
      expect(MockWebSocket.instances.length).toBeGreaterThan(0);
    });

    act(() => {
      for (const socket of MockWebSocket.instances) {
        socket.emitOpen();
        socket.emitMessage({
          type: "sync",
          fen: "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1",
          moves: [{ from: "e2", to: "e4", san: "e4", color: "w" }],
          whiteTime: 600,
          blackTime: 600,
          status: "playing",
          spectatorCount: 9,
          white: { address: "GWHITE123456", elo: 1300 },
          black: { address: "GBLACK654321", elo: 1290 },
        });
      }
    });

    await waitFor(() => {
      expect(MockWebSocket.instances.at(-1)?.url).toContain("/v1/games/live-game-42/spectate");
      expect(result.current.gameState?.fen).toContain("4P3");
      expect(result.current.gameState?.spectatorCount).toBe(9);
    });
  });
});
