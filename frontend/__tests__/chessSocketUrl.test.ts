import { afterEach, describe, expect, it, vi } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";

class MockWebSocket {
  static instances: MockWebSocket[] = [];
  static OPEN = 1;
  static CLOSED = 3;
  readyState = 0;
  onopen: (() => void) | null = null;
  onmessage: ((event: MessageEvent) => void) | null = null;
  onerror: ((event: Event) => void) | null = null;
  onclose: ((event: CloseEvent) => void) | null = null;

  constructor(public url: string) {
    MockWebSocket.instances.push(this);
  }

  send() {}
  close() {
    this.readyState = MockWebSocket.CLOSED;
  }
}

describe("useChessSocket URL (#FE-90)", () => {
  afterEach(() => {
    MockWebSocket.instances = [];
    vi.unstubAllEnvs();
    vi.unstubAllGlobals();
    vi.resetModules();
  });

  async function openSocketFor(gameId: string) {
    vi.stubGlobal("WebSocket", MockWebSocket as unknown as typeof WebSocket);
    const { useChessSocket } = await import("@/hook/useChessSocket");
    const { unmount } = renderHook(() => useChessSocket(gameId));
    await waitFor(() => expect(MockWebSocket.instances).toHaveLength(1));
    unmount();
    return MockWebSocket.instances[0].url;
  }

  it("connects to the backend game socket derived from NEXT_PUBLIC_API_URL", async () => {
    vi.stubEnv("NEXT_PUBLIC_API_URL", "https://api.knightverse.example");

    await expect(openSocketFor("game-123")).resolves.toBe(
      "wss://api.knightverse.example/v1/ws/game/game-123",
    );
  });

  it("falls back to the local backend when no API URL is configured", async () => {
    vi.stubEnv("NEXT_PUBLIC_API_URL", "");

    await expect(openSocketFor("abc")).resolves.toBe("ws://localhost:8000/v1/ws/game/abc");
  });

  it("never connects to the old hardcoded scaling nodes", async () => {
    vi.stubEnv("NEXT_PUBLIC_API_URL", "https://api.knightverse.example");

    const url = await openSocketFor("game-1");
    expect(url).not.toMatch(/localhost:808[0-2]/);
  });
});
