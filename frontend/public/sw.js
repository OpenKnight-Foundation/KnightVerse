/* eslint-disable no-restricted-globals */

const CACHE_NAME = "knightverse-shell-v1";
const OFFLINE_URL = "/offline";

const APP_SHELL_ASSETS = [
  OFFLINE_URL,
  "/manifest.webmanifest",
  "/icons/icon-192x192.png",
  "/icons/icon-512x512.png",
  "/images/KnightVerseLogo.png",
];

const STATIC_ASSET_PREFIXES = [
  "/_next/static/",
  "/icons/",
  "/images/",
  "/assets/",
];

function isAuthenticatedOrApiRequest(request, url) {
  if (request.headers.has("authorization") || request.headers.has("cookie")) {
    return true;
  }

  const pathname = url.pathname;
  if (
    pathname === "/api" ||
    pathname.startsWith("/api/") ||
    pathname === "/auth" ||
    pathname.startsWith("/auth/")
  ) {
    return true;
  }

  return false;
}

function isCacheableResponse(response) {
  if (!response || !response.ok || response.type !== "basic") {
    return false;
  }

  const cacheControl = (response.headers.get("cache-control") || "").toLowerCase();
  if (cacheControl.includes("no-store") || cacheControl.includes("private")) {
    return false;
  }

  if (response.headers.has("set-cookie")) {
    return false;
  }

  const vary = (response.headers.get("vary") || "").toLowerCase();
  if (vary.includes("authorization") || vary.includes("cookie")) {
    return false;
  }

  return true;
}

function isStaticAppShellAsset(pathname) {
  if (pathname === "/manifest.webmanifest") {
    return true;
  }
  return STATIC_ASSET_PREFIXES.some((prefix) => pathname.startsWith(prefix));
}

self.addEventListener("install", (event) => {
  event.waitUntil(
    caches
      .open(CACHE_NAME)
      .then((cache) => cache.addAll(APP_SHELL_ASSETS))
      .then(() => self.skipWaiting())
  );
});

self.addEventListener("activate", (event) => {
  event.waitUntil(
    caches
      .keys()
      .then((keys) =>
        Promise.all(
          keys
            .filter((key) => key !== CACHE_NAME)
            .map((key) => caches.delete(key))
        )
      )
      .then(() => self.clients.claim())
  );
});

self.addEventListener("fetch", (event) => {
  const { request } = event;

  // Never intercept non-GET requests (mutations, auth posts, etc.).
  if (request.method !== "GET") {
    return;
  }

  const url = new URL(request.url);

  // Never cache cross-origin requests (external APIs, Stellar RPC, etc.).
  if (url.origin !== self.location.origin) {
    return;
  }

  // Never cache authenticated or API requests.
  if (isAuthenticatedOrApiRequest(request, url)) {
    return;
  }

  // Navigation requests: network-first, falling back to the cached /offline shell.
  if (request.mode === "navigate") {
    event.respondWith(
      fetch(request).catch(async () => {
        const cachedOffline = await caches.match(OFFLINE_URL);
        if (cachedOffline) {
          return cachedOffline;
        }
        return new Response("Offline", {
          status: 503,
          statusText: "Service Unavailable",
          headers: { "Content-Type": "text/plain; charset=utf-8" },
        });
      })
    );
    return;
  }

  // Static app-shell assets: cache-first with safe response validation.
  if (isStaticAppShellAsset(url.pathname)) {
    event.respondWith(
      caches.match(request).then(async (cached) => {
        if (cached) {
          return cached;
        }
        const networkResponse = await fetch(request);
        if (isCacheableResponse(networkResponse)) {
          const cache = await caches.open(CACHE_NAME);
          cache.put(request, networkResponse.clone());
        }
        return networkResponse;
      })
    );
  }
});
