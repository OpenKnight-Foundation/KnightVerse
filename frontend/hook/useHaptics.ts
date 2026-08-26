import { useState, useEffect, useCallback } from "react";

/**
 * Vibration patterns for different chess events (in milliseconds).
 * Each array is passed to navigator.vibrate() for tactile feedback.
 */
export const VIBRATION_PATTERNS = {
  /** Light pulse for normal piece moves */
  move: [15],
  /** Double pulse for captures */
  capture: [30, 20, 30],
  /** Heavy alert pulse for check */
  check: [60, 40, 60],
  /** Extended pattern for game over */
  gameOver: [100, 50, 100, 50, 200],
} as const;

export type HapticEvent = keyof typeof VIBRATION_PATTERNS;

interface UseHapticsReturn {
  /** Whether the device supports the Vibration API */
  isSupported: boolean;
  /** Whether haptic feedback is currently enabled */
  isEnabled: boolean;
  /** Toggle haptic feedback on/off */
  setIsEnabled: (enabled: boolean) => void;
  /** Trigger haptic feedback for a chess event */
  triggerHaptic: (event: HapticEvent) => void;
}

const STORAGE_KEY = "knightverse-haptics-enabled";

/**
 * Checks if the device supports the Web Vibration API.
 */
function checkVibrationSupport(): boolean {
  if (typeof navigator === "undefined") return false;
  return typeof navigator.vibrate === "function";
}

/**
 * Returns whether touch is the primary input device.
 */
function isTouchDevice(): boolean {
  if (typeof window === "undefined") return false;
  return (
    "ontouchstart" in window ||
    navigator.maxTouchPoints > 0
  );
}

/**
 * Custom hook for haptic feedback via the Web Vibration API.
 *
 * Provides tactile feedback when pieces snap to squares, captures occur,
 * or check is delivered. Safely degrades on unsupported browsers.
 *
 * @returns Object containing support detection, enable state, and trigger function
 */
export function useHaptics(): UseHapticsReturn {
  const [isSupported] = useState<boolean>(() => checkVibrationSupport());
  const [isEnabled, setIsEnabledState] = useState<boolean>(() => {
    if (typeof window === "undefined") return true;
    try {
      const stored = localStorage.getItem(STORAGE_KEY);
      if (stored !== null) return stored === "true";
    } catch {
      // localStorage may be unavailable in some environments
    }
    return isTouchDevice();
  });

  // Persist preference to localStorage
  useEffect(() => {
    if (typeof window === "undefined") return;
    try {
      localStorage.setItem(STORAGE_KEY, String(isEnabled));
    } catch {
      // Ignore storage errors
    }
  }, [isEnabled]);

  const setIsEnabled = useCallback((enabled: boolean) => {
    setIsEnabledState(enabled);
  }, []);

  const triggerHaptic = useCallback(
    (event: HapticEvent) => {
      if (!isSupported || !isEnabled) return;

      try {
        navigator.vibrate(VIBRATION_PATTERNS[event]);
      } catch {
        // Silently ignore vibration errors on unsupported browsers
      }
    },
    [isSupported, isEnabled],
  );

  return { isSupported, isEnabled, setIsEnabled, triggerHaptic };
}

/**
 * Standalone haptic trigger function for use outside React components.
 * Returns true if vibration was triggered, false otherwise.
 */
export function triggerHapticEvent(
  event: HapticEvent,
  enabled: boolean = true,
): boolean {
  if (!checkVibrationSupport() || !enabled) return false;

  try {
    navigator.vibrate(VIBRATION_PATTERNS[event]);
    return true;
  } catch {
    return false;
  }
}
