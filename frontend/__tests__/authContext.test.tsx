import React from "react";
import { act, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { AuthProvider, useAuth } from "@/context/authContext";

function makeToken(expiresInSeconds: number): string {
  const encode = (value: object) =>
    btoa(JSON.stringify(value)).replace(/=+$/, "").replace(/\+/g, "-").replace(/\//g, "_");
  const exp = Math.floor(Date.now() / 1000) + expiresInSeconds;
  return `${encode({ alg: "HS256", typ: "JWT" })}.${encode({ exp, sub: "1" })}.signature`;
}

const USER = { user_id: 7, username: "knight" };

function seedSession(token: string, user: unknown = USER) {
  localStorage.setItem("access_token", token);
  localStorage.setItem("user", typeof user === "string" ? user : JSON.stringify(user));
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

/** Simulates another tab writing to localStorage (storage events only fire cross-tab). */
function otherTabWrites(key: string, value: string | null) {
  if (value === null) localStorage.removeItem(key);
  else localStorage.setItem(key, value);
  window.dispatchEvent(new StorageEvent("storage", { key, newValue: value }));
}

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

  describe("expired stored token (#FE-91)", () => {
    it("refreshes immediately on mount and adopts the new token", async () => {
      const fresh = makeToken(3600);
      seedSession(makeToken(-60));
      fetchMock.mockResolvedValue({ ok: true, json: async () => ({ access_token: fresh }) });

      renderAuth();

      await waitFor(() => expect(state()).toBe("authenticated"));
      expect(fetchMock).toHaveBeenCalledTimes(1);
      expect(String(fetchMock.mock.calls[0][0])).toMatch(/\/v1\/auth\/refresh$/);
      expect(token()).toBe(fresh);
      expect(localStorage.getItem("access_token")).toBe(fresh);
    });

    it("also refreshes immediately when the token is inside the 5-minute margin", async () => {
      seedSession(makeToken(120));
      fetchMock.mockResolvedValue({ ok: true, json: async () => ({ access_token: makeToken(3600) }) });

      renderAuth();

      await waitFor(() => expect(fetchMock).toHaveBeenCalledTimes(1));
    });

    it("stays loading (not authenticated) until the refresh settles", async () => {
      seedSession(makeToken(-60));
      let resolveRefresh: (value: unknown) => void = () => {};
      fetchMock.mockReturnValue(new Promise((resolve) => (resolveRefresh = resolve)));

      renderAuth();
      expect(state()).toBe("loading");

      await act(async () => resolveRefresh({ ok: true, json: async () => ({ access_token: makeToken(3600) }) }));
      await waitFor(() => expect(state()).toBe("authenticated"));
    });

    it("clears auth state and storage when the refresh fails", async () => {
      seedSession(makeToken(-60));
      fetchMock.mockResolvedValue({ ok: false, status: 401, json: async () => ({}) });

      renderAuth();

      await waitFor(() => expect(state()).toBe("anonymous"));
      expect(token()).toBe("");
      expect(localStorage.getItem("access_token")).toBeNull();
      expect(localStorage.getItem("user")).toBeNull();
    });

    it("does not refresh a token that is still valid", async () => {
      seedSession(makeToken(3600));

      renderAuth();

      await waitFor(() => expect(state()).toBe("authenticated"));
      expect(fetchMock).not.toHaveBeenCalled();
    });
  });

  describe("corrupt stored user (#FE-92)", () => {
    it("renders logged out and clears both keys when `user` is invalid JSON", async () => {
      seedSession(makeToken(3600), "{not json");

      renderAuth();

      await waitFor(() => expect(state()).toBe("anonymous"));
      expect(screen.getByTestId("user").textContent).toBe("");
      expect(localStorage.getItem("access_token")).toBeNull();
      expect(localStorage.getItem("user")).toBeNull();
    });

    it("renders logged out when the stored token can't be decoded", async () => {
      seedSession("not-a-jwt");

      renderAuth();

      await waitFor(() => expect(state()).toBe("anonymous"));
      expect(fetchMock).not.toHaveBeenCalled();
    });
  });

  describe("cross-tab sync (#FE-93)", () => {
    it("logs out when another tab logs out", async () => {
      seedSession(makeToken(3600));
      renderAuth();
      await waitFor(() => expect(state()).toBe("authenticated"));

      act(() => {
        otherTabWrites("access_token", null);
        otherTabWrites("user", null);
      });

      await waitFor(() => expect(state()).toBe("anonymous"));
    });

    it("adopts a login or refreshed token written by another tab", async () => {
      renderAuth();
      await waitFor(() => expect(state()).toBe("anonymous"));
      const fromOtherTab = makeToken(3600);

      act(() => {
        otherTabWrites("user", JSON.stringify(USER));
        otherTabWrites("access_token", fromOtherTab);
      });

      await waitFor(() => expect(token()).toBe(fromOtherTab));
      expect(screen.getByTestId("user").textContent).toBe("knight");
      expect(fetchMock).not.toHaveBeenCalled();
    });

    it("reuses a token another tab refreshed while this tab waited for the lock", async () => {
      const refreshedElsewhere = makeToken(3600);
      seedSession(makeToken(-60));
      // Simulate the Web Locks API: while this tab waits, another tab holding
      // the lock refreshes and writes a fresh token to storage.
      vi.stubGlobal("navigator", {
        ...navigator,
        locks: {
          request: async (_name: string, fn: () => Promise<void>) => {
            localStorage.setItem("access_token", refreshedElsewhere);
            return fn();
          },
        },
      });

      renderAuth();

      await waitFor(() => expect(token()).toBe(refreshedElsewhere));
      expect(state()).toBe("authenticated");
      expect(fetchMock).not.toHaveBeenCalled();
    });

    it("serialises refreshes through the cross-tab lock", async () => {
      seedSession(makeToken(-60));
      const request = vi.fn(async (_name: string, fn: () => Promise<void>) => fn());
      vi.stubGlobal("navigator", { ...navigator, locks: { request } });
      fetchMock.mockResolvedValue({ ok: true, json: async () => ({ access_token: makeToken(3600) }) });

      renderAuth();

      await waitFor(() => expect(state()).toBe("authenticated"));
      expect(request).toHaveBeenCalledWith("knightverse-auth-refresh", expect.any(Function));
      expect(fetchMock).toHaveBeenCalledTimes(1);
    });
  });
});
