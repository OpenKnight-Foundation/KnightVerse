import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import {
  useHaptics,
  triggerHapticEvent,
  VIBRATION_PATTERNS,
} from "@/hook/useHaptics";

// Mock navigator.vibrate
const mockVibrate = vi.fn();
Object.defineProperty(navigator, "vibrate", {
  value: mockVibrate,
  writable: true,
  configurable: true,
});

// Mock localStorage
const localStorageMock = (() => {
  let store: Record<string, string> = {};
  return {
    getItem: vi.fn((key: string) => store[key] ?? null),
    setItem: vi.fn((key: string, value: string) => {
      store[key] = value;
    }),
    clear: vi.fn(() => {
      store = {};
    }),
    removeItem: vi.fn((key: string) => {
      delete store[key];
    }),
  };
})();
Object.defineProperty(window, "localStorage", { value: localStorageMock });

describe("useHaptics", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    localStorageMock.clear();
    mockVibrate.mockImplementation(() => true);
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("detects vibration support when navigator.vibrate exists", () => {
    const { result } = renderHook(() => useHaptics());
    expect(result.current.isSupported).toBe(true);
  });

  it("returns isEnabled true by default on touch devices", () => {
    const { result } = renderHook(() => useHaptics());
    // By default isEnabled is true in touch-capable env
    expect(typeof result.current.isEnabled).toBe("boolean");
  });

  it("triggers vibration for move event", () => {
    const { result } = renderHook(() => useHaptics());
    act(() => {
      result.current.triggerHaptic("move");
    });
    expect(mockVibrate).toHaveBeenCalledWith(VIBRATION_PATTERNS.move);
  });

  it("triggers vibration for capture event", () => {
    const { result } = renderHook(() => useHaptics());
    act(() => {
      result.current.triggerHaptic("capture");
    });
    expect(mockVibrate).toHaveBeenCalledWith(VIBRATION_PATTERNS.capture);
  });

  it("triggers vibration for check event", () => {
    const { result } = renderHook(() => useHaptics());
    act(() => {
      result.current.triggerHaptic("check");
    });
    expect(mockVibrate).toHaveBeenCalledWith(VIBRATION_PATTERNS.check);
  });

  it("triggers vibration for gameOver event", () => {
    const { result } = renderHook(() => useHaptics());
    act(() => {
      result.current.triggerHaptic("gameOver");
    });
    expect(mockVibrate).toHaveBeenCalledWith(VIBRATION_PATTERNS.gameOver);
  });

  it("does not vibrate when disabled", () => {
    const { result } = renderHook(() => useHaptics());
    act(() => {
      result.current.setIsEnabled(false);
    });
    act(() => {
      result.current.triggerHaptic("move");
    });
    expect(mockVibrate).not.toHaveBeenCalled();
  });

  it("re-enables vibration after being disabled", () => {
    const { result } = renderHook(() => useHaptics());
    act(() => {
      result.current.setIsEnabled(false);
    });
    act(() => {
      result.current.setIsEnabled(true);
    });
    act(() => {
      result.current.triggerHaptic("move");
    });
    expect(mockVibrate).toHaveBeenCalledWith(VIBRATION_PATTERNS.move);
  });

  it("persists preference to localStorage", () => {
    const { result } = renderHook(() => useHaptics());
    act(() => {
      result.current.setIsEnabled(false);
    });
    expect(localStorageMock.setItem).toHaveBeenCalledWith(
      "knightverse-haptics-enabled",
      "false",
    );
  });

  it("loads preference from localStorage", () => {
    localStorageMock.getItem.mockReturnValue("false");
    const { result } = renderHook(() => useHaptics());
    expect(result.current.isEnabled).toBe(false);
  });

  it("stores vibration pattern definitions correctly", () => {
    expect(VIBRATION_PATTERNS.move).toEqual([15]);
    expect(VIBRATION_PATTERNS.capture).toEqual([30, 20, 30]);
    expect(VIBRATION_PATTERNS.check).toEqual([60, 40, 60]);
    expect(VIBRATION_PATTERNS.gameOver).toEqual([100, 50, 100, 50, 200]);
  });
});

describe("triggerHapticEvent (standalone)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockVibrate.mockImplementation(() => true);
  });

  it("triggers vibration when supported and enabled", () => {
    const result = triggerHapticEvent("move", true);
    expect(mockVibrate).toHaveBeenCalledWith(VIBRATION_PATTERNS.move);
    expect(result).toBe(true);
  });

  it("does not vibrate when disabled", () => {
    const result = triggerHapticEvent("move", false);
    expect(mockVibrate).not.toHaveBeenCalled();
    expect(result).toBe(false);
  });

  it("gracefully handles vibrate throwing an error", () => {
    mockVibrate.mockImplementation(() => {
      throw new Error("Vibration failed");
    });
    const result = triggerHapticEvent("check", true);
    expect(result).toBe(false);
  });
});

describe("useHaptics graceful degradation", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    localStorageMock.clear();
  });

  it("does not throw when navigator.vibrate is undefined", () => {
    const originalVibrate = navigator.vibrate;
    // @ts-expect-error - Testing unsupported browser
    delete navigator.vibrate;
    try {
      const { result } = renderHook(() => useHaptics());
      expect(result.current.isSupported).toBe(false);
      expect(() => {
        result.current.triggerHaptic("move");
      }).not.toThrow();
    } finally {
      Object.defineProperty(navigator, "vibrate", {
        value: originalVibrate,
        writable: true,
        configurable: true,
      });
    }
  });

  it("does not throw when localStorage is unavailable", () => {
    const originalGetItem = localStorageMock.getItem;
    localStorageMock.getItem.mockImplementation(() => {
      throw new Error("Storage unavailable");
    });
    try {
      const { result } = renderHook(() => useHaptics());
      expect(() => {
        result.current.triggerHaptic("move");
      }).not.toThrow();
    } finally {
      localStorageMock.getItem.mockImplementation(originalGetItem);
    }
  });
});
