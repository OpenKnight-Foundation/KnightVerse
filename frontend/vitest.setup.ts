import "@testing-library/jest-dom/vitest";
import * as axeMatchers from "vitest-axe/matchers";
import { expect } from "vitest";

expect.extend(axeMatchers);

// Ensure cross-realm Uint8Array identity for js-xdr in jsdom
if (typeof Uint8Array !== "undefined") {
  try {
    Object.defineProperty(Uint8Array, Symbol.hasInstance, {
      value: (instance: unknown) =>
        Boolean(
          instance &&
            (Object.prototype.toString.call(instance) === "[object Uint8Array]" ||
              (instance as { constructor?: { name?: string } }).constructor?.name === "Uint8Array"),
        ),
      configurable: true,
    });
  } catch {
    // Ignore if not configurable
  }
}

// Polyfill localStorage in jsdom when Node 26 leaves experimental localStorage unconfigured
if (typeof window !== "undefined") {
  const store: Record<string, string> = {};
  const mockStorage = {
    getItem: (key: string) => store[key] ?? null,
    setItem: (key: string, value: string) => {
      store[key] = String(value);
    },
    removeItem: (key: string) => {
      delete store[key];
    },
    clear: () => {
      for (const k in store) delete store[k];
    },
    get length() {
      return Object.keys(store).length;
    },
    key: (i: number) => Object.keys(store)[i] ?? null,
  };
  try {
    Object.defineProperty(window, "localStorage", {
      value: mockStorage,
      writable: true,
      configurable: true,
    });
  } catch {}
  try {
    Object.defineProperty(globalThis, "localStorage", {
      value: mockStorage,
      writable: true,
      configurable: true,
    });
  } catch {}
}
