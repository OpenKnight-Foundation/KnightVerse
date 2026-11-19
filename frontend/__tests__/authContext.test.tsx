import React from "react";
import { act, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { AuthProvider, useAuth } from "@/context/authContext";
import { getSessionAccessToken } from "@/lib/sessionToken";

function makeToken(expiresInSeconds: number, overrides: Record<string, unknown> = {}): string {
  const encode = (value: object) =>
    btoa(JSON.stringify(value)).replace(/=+$/, "").replace(/\+/g, "-").replace(/\//g, "_");
  const exp = Math.floor(Date.now() / 1000) + expiresInSeconds;
  return `${encode({ alg: "HS256", typ: "JWT" })}.${encode({ exp, sub: "1", user_id: 7, username: "knight", ...overrides })}.signature`;
}

function AuthProbe() {
  const { isAuthenticated, isLoading, accessToken, user } = useAuth();
  return (
    <div>
      <span data-testid="state">{isLoading ? "loading" : isAuthenticated ? "authenticated" : "anonymous"}</span>
      <span data-testid="token">{accessToken ?? ""}</span>
      <span data-testid="user">{user?.username ?? ""}</span>
    </div>
  );
}

function renderAuth() {
  return render(
    <AuthProvider>
      <AuthProbe />
    </AuthProvider>,
  );
}

const state = () => screen.getByTestId("state").textContent;
const token = () => screen.getByTestId("token").textContent;

describe("AuthProvider", () => {
  let fetchMock: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    localStorage.clear();
    fetchMock = vi.fn();
    vi.stubGlobal("fetch", fetchMock);
    vi.spyOn(console, "error").mockImplementation(() => {});
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
    localStorage.clear();
  });

  describe("access token storage (#FE-166)", () => {
    it("never writes the access token or user profile to localStorage", async () => {
      const fresh = makeToken(3600);
      fetchMock.mockResolvedValue({ ok: true, json: async () => ({ access_token: fresh }) });

      renderAuth();

      await waitFor(() => expect(state()).toBe("authenticated"));
      expect(localStorage.getItem("access_token")).toBeNull();
      expect(localStorage.getItem("user")).toBeNull();
      // Only the in-memory holder carries it, for non-React fetch helpers.
      expect(getSessionAccessToken()).toBe(fresh);
    });
  });

  describe("silent restore from the refresh-token cookie on mount (#FE-166)", () => {
    it("calls /auth/refresh with credentials included and adopts the returned token", async () => {
      const fresh = makeToken(3600);
      fetchMock.mockResolvedValue({ ok: true, json: async () => ({ access_token: fresh }) });

      renderAuth();

      await waitFor(() => expect(state()).toBe("authenticated"));
      expect(fetchMock).toHaveBeenCalledTimes(1);
      expect(String(fetchMock.mock.calls[0][0])).toMatch(/\/v1\/auth\/refresh$/);
      expect(fetchMock.mock.calls[0][1]).toMatchObject({ credentials: "include" });
      expect(token()).toBe(fresh);
    });

    it("derives the user from the restored token's claims", async () => {
      const fresh = makeToken(3600, { user_id: 42, username: "rook" });
      fetchMock.mockResolvedValue({ ok: true, json: async () => ({ access_token: fresh }) });

      renderAuth();

      await waitFor(() => expect(screen.getByTestId("user").textContent).toBe("rook"));
    });

    it("stays loading (not authenticated) until the restore settles", async () => {
      let resolveRefresh: (value: unknown) => void = () => {};
      fetchMock.mockReturnValue(new Promise((resolve) => (resolveRefresh = resolve)));

      renderAuth();
      expect(state()).toBe("loading");

      await act(async () => resolveRefresh({ ok: true, json: async () => ({ access_token: makeToken(3600) }) }));
      await waitFor(() => expect(state()).toBe("authenticated"));
    });

    it("renders logged out when there is no valid refresh-token cookie", async () => {
      fetchMock.mockResolvedValue({ ok: false, status: 401, json: async () => ({}) });

      renderAuth();

      await waitFor(() => expect(state()).toBe("anonymous"));
      expect(token()).toBe("");
      expect(getSessionAccessToken()).toBeNull();
    });
  });

  describe("scheduled refresh before expiry (#FE-91)", () => {
    it("schedules another refresh before the restored token expires", async () => {
      vi.useFakeTimers();
      const first = makeToken(400); // inside the 5-minute margin after ~100s
      const second = makeToken(3600);
      fetchMock
        .mockResolvedValueOnce({ ok: true, json: async () => ({ access_token: first }) })
        .mockResolvedValueOnce({ ok: true, json: async () => ({ access_token: second }) });

      renderAuth();
      await act(async () => {
        await vi.runOnlyPendingTimersAsync();
      });
      expect(token()).toBe(first);

      await act(async () => {
        await vi.advanceTimersByTimeAsync(200 * 1000);
      });

      expect(fetchMock).toHaveBeenCalledTimes(2);
      expect(token()).toBe(second);
      vi.useRealTimers();
    });
  });

  describe("logout", () => {
    it("clears the in-memory session", async () => {
      const fresh = makeToken(3600);
      fetchMock.mockResolvedValue({ ok: true, json: async () => ({ access_token: fresh }) });

      function Harness() {
        const { logout, ...rest } = useAuth();
        return (
          <>
            <AuthProbe />
            <button onClick={() => void logout()}>Log out</button>
            <span data-testid="rest">{Object.keys(rest).length}</span>
          </>
        );
      }

      render(
        <AuthProvider>
          <Harness />
        </AuthProvider>,
      );
      await waitFor(() => expect(state()).toBe("authenticated"));

      fetchMock.mockResolvedValue({ ok: true, json: async () => ({}) });
      await act(async () => {
        screen.getByText("Log out").click();
      });

      await waitFor(() => expect(state()).toBe("anonymous"));
      expect(getSessionAccessToken()).toBeNull();
    });
  });
});
