import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render } from "@testing-library/react";
import fs from "node:fs";
import path from "node:path";
import vm from "node:vm";
import ServiceWorkerRegistrar from "@/components/ServiceWorkerRegistrar";

const PUBLIC_DIR = path.resolve(__dirname, "../public");

describe("PWA web manifest and icons", () => {
  it("provides a valid web manifest with existing 192x192 and 512x512 icons", () => {
    const manifestPath = path.join(PUBLIC_DIR, "manifest.webmanifest");
    expect(fs.existsSync(manifestPath)).toBe(true);

    const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
    expect(manifest.name).toBe("KnightVerse");
    expect(manifest.short_name).toBe("KnightVerse");
    expect(manifest.start_url).toBe("/");
    expect(manifest.display).toBe("standalone");
    expect(Array.isArray(manifest.icons)).toBe(true);

    const sizes = manifest.icons.map((icon: { sizes: string }) => icon.sizes);
    expect(sizes).toContain("192x192");
    expect(sizes).toContain("512x512");

    for (const icon of manifest.icons) {
      const iconPath = path.join(PUBLIC_DIR, icon.src.replace(/^\//, ""));
      expect(fs.existsSync(iconPath)).toBe(true);
      expect(fs.statSync(iconPath).size).toBeGreaterThan(0);
    }
  });
});

describe("ServiceWorkerRegistrar", () => {
  const originalServiceWorker = navigator.serviceWorker;

  afterEach(() => {
    Object.defineProperty(navigator, "serviceWorker", {
      value: originalServiceWorker,
      configurable: true,
      writable: true,
    });
  });

  it("registers /sw.js with root scope on mount", () => {
    const register = vi.fn().mockResolvedValue({});
    Object.defineProperty(navigator, "serviceWorker", {
      value: { register },
      configurable: true,
      writable: true,
    });

    render(<ServiceWorkerRegistrar />);

    expect(register).toHaveBeenCalledTimes(1);
    expect(register).toHaveBeenCalledWith("/sw.js", { scope: "/" });
  });
});

describe("Service worker (public/sw.js)", () => {
  const swSource = fs.readFileSync(path.join(PUBLIC_DIR, "sw.js"), "utf8");

  type Listener = (event: Record<string, unknown>) => void;

  function createSwEnv() {
    const listeners: Record<string, Listener> = {};
    const store = new Map<string, Map<string, Response>>();

    const cachesMock = {
      open: vi.fn(async (name: string) => {
        if (!store.has(name)) {
          store.set(name, new Map());
        }
        const cacheMap = store.get(name)!;
        return {
          addAll: vi.fn(async (urls: string[]) => {
            for (const u of urls) {
              cacheMap.set(u, new Response(`cached:${u}`, { status: 200 }));
            }
          }),
          put: vi.fn(async (req: Request | string, res: Response) => {
            const key = typeof req === "string" ? req : new URL(req.url).pathname;
            cacheMap.set(key, res);
          }),
        };
      }),
      match: vi.fn(async (req: Request | string) => {
        const key = typeof req === "string" ? req : new URL(req.url).pathname;
        for (const cacheMap of store.values()) {
          if (cacheMap.has(key)) {
            return cacheMap.get(key);
          }
        }
        return undefined;
      }),
      keys: vi.fn(async () => [...store.keys()]),
      delete: vi.fn(async (name: string) => store.delete(name)),
    };

    const fetchMock = vi.fn();
    const skipWaiting = vi.fn().mockResolvedValue(undefined);
    const claim = vi.fn().mockResolvedValue(undefined);

    const selfMock = {
      location: { origin: "https://knightverse.app" },
      addEventListener: (type: string, cb: Listener) => {
        listeners[type] = cb;
      },
      skipWaiting,
      clients: { claim },
    };

    vm.runInNewContext(swSource, {
      self: selfMock,
      caches: cachesMock,
      fetch: fetchMock,
      URL,
      Response,
      Headers,
    });

    return { listeners, store, cachesMock, fetchMock, skipWaiting, claim };
  }

  async function installSw(env: ReturnType<typeof createSwEnv>) {
    let waitPromise: Promise<unknown> = Promise.resolve();
    env.listeners.install({
      waitUntil: (p: Promise<unknown>) => {
        waitPromise = p;
      },
    });
    await waitPromise;
  }

  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("precaches /offline and app shell assets on install", async () => {
    const env = createSwEnv();
    await installSw(env);

    expect(env.skipWaiting).toHaveBeenCalledTimes(1);
    const cachedOffline = await env.cachesMock.match("/offline");
    expect(cachedOffline).toBeDefined();
    expect(await cachedOffline!.text()).toBe("cached:/offline");
  });

  it("serves cached /offline when a navigation request fails", async () => {
    const env = createSwEnv();
    await installSw(env);

    env.fetchMock.mockRejectedValueOnce(new TypeError("NetworkError"));

    let responsePromise: Promise<Response> | undefined;
    env.listeners.fetch({
      request: {
        method: "GET",
        mode: "navigate",
        url: "https://knightverse.app/play",
        headers: new Headers(),
      },
      respondWith: (p: Promise<Response>) => {
        responsePromise = p;
      },
    });

    expect(responsePromise).toBeDefined();
    const res = await responsePromise!;
    expect(await res.text()).toBe("cached:/offline");
  });

  it("never intercepts or caches /api/* requests", async () => {
    const env = createSwEnv();
    await installSw(env);

    const respondWith = vi.fn();
    env.listeners.fetch({
      request: {
        method: "GET",
        mode: "cors",
        url: "https://knightverse.app/api/v1/profile",
        headers: new Headers(),
      },
      respondWith,
    });

    expect(respondWith).not.toHaveBeenCalled();
  });

  it("never intercepts or caches requests with Authorization header", async () => {
    const env = createSwEnv();
    await installSw(env);

    const respondWith = vi.fn();
    env.listeners.fetch({
      request: {
        method: "GET",
        mode: "cors",
        url: "https://knightverse.app/images/private-avatar.png",
        headers: new Headers({ Authorization: "Bearer secret-token" }),
      },
      respondWith,
    });

    expect(respondWith).not.toHaveBeenCalled();
  });

  it("caches safe static app-shell assets and skips private/no-store responses", async () => {
    const env = createSwEnv();
    await installSw(env);

    // Private response should NOT be cached
    const privateRes = {
      ok: true,
      type: "basic",
      headers: new Headers({ "Cache-Control": "private, no-store" }),
      clone: () => privateRes,
    };
    env.fetchMock.mockResolvedValueOnce(privateRes);

    let privatePromise: Promise<Response> | undefined;
    env.listeners.fetch({
      request: {
        method: "GET",
        mode: "no-cors",
        url: "https://knightverse.app/_next/static/chunks/app.js",
        headers: new Headers(),
      },
      respondWith: (p: Promise<Response>) => {
        privatePromise = p;
      },
    });
    await privatePromise;
    expect(await env.cachesMock.match("/_next/static/chunks/app.js")).toBeUndefined();

    // Public static response SHOULD be cached
    const publicRes = {
      ok: true,
      type: "basic",
      headers: new Headers({ "Cache-Control": "public, max-age=31536000" }),
      clone: () => publicRes,
    };
    env.fetchMock.mockResolvedValueOnce(publicRes);

    let publicPromise: Promise<Response> | undefined;
    env.listeners.fetch({
      request: {
        method: "GET",
        mode: "no-cors",
        url: "https://knightverse.app/_next/static/chunks/vendor.js",
        headers: new Headers(),
      },
      respondWith: (p: Promise<Response>) => {
        publicPromise = p;
      },
    });
    await publicPromise;
    expect(await env.cachesMock.match("/_next/static/chunks/vendor.js")).toBeDefined();
  });
});
