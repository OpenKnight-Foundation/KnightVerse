"use client";

/**
 * Lightweight feature-flag system (Task 2)
 *
 * Flags are configured entirely through the NEXT_PUBLIC_FEATURE_FLAGS env var
 * so they can be changed at deploy time without a code change, and without
 * paying for a third-party service.
 *
 * Supported rule shapes:
 *   { "enabled": true }     — fully on for every user
 *   { "enabled": false }    — fully off for every user
 *   { "rollout": 0.25 }     — 25 % of users, stable per user identifier
 *
 * Usage:
 *   const { isEnabled } = useFeatureFlag();
 *   if (isEnabled("new_board_renderer")) { … }
 *
 * See frontend/.env.example for the NEXT_PUBLIC_FEATURE_FLAGS format.
 */

import React, {
  createContext,
  useContext,
  useMemo,
  type ReactNode,
} from "react";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/** A fully-on or fully-off rule. */
interface EnabledRule {
  enabled: boolean;
}

/** A percentage-based rollout rule (0.0 – 1.0). */
interface RolloutRule {
  rollout: number;
}

export type FlagRule = EnabledRule | RolloutRule;

/** The raw flags config object read from the env var. */
export type FlagsConfig = Record<string, FlagRule>;

// Known flag names — extend this union as new flags are introduced.
export type FlagName = "new_board_renderer" | "new_tournament_bracket" | (string & {});

// ---------------------------------------------------------------------------
// Stable hash helper
// ---------------------------------------------------------------------------

/**
 * Deterministic hash for (flagName, userId) that maps to [0, 1).
 * Uses the djb2 algorithm — tiny, no dependencies, sufficient distribution.
 */
function stableHash(input: string): number {
  let hash = 5381;
  for (let i = 0; i < input.length; i++) {
    hash = ((hash << 5) + hash) ^ input.charCodeAt(i);
    // Keep within 32-bit signed int range to avoid floating-point drift
    hash = hash >>> 0;
  }
  return hash / 0xffffffff;
}

// ---------------------------------------------------------------------------
// Parse flags from env
// ---------------------------------------------------------------------------

function parseFlagsConfig(): FlagsConfig {
  const raw = process.env.NEXT_PUBLIC_FEATURE_FLAGS;
  if (!raw) return {};
  try {
    const parsed = JSON.parse(raw);
    if (typeof parsed === "object" && parsed !== null && !Array.isArray(parsed)) {
      return parsed as FlagsConfig;
    }
    console.warn("[FeatureFlags] NEXT_PUBLIC_FEATURE_FLAGS must be a JSON object.");
    return {};
  } catch {
    console.warn("[FeatureFlags] Failed to parse NEXT_PUBLIC_FEATURE_FLAGS:", raw);
    return {};
  }
}

// ---------------------------------------------------------------------------
// Evaluation logic
// ---------------------------------------------------------------------------

/**
 * Evaluates a single flag rule against a user identifier.
 *
 * @param rule         - The flag rule from the config.
 * @param flagName     - The name of the flag (used to salt the hash).
 * @param userIdentifier - A stable per-user ID (e.g. user_id, wallet address).
 *                         Pass null / undefined for anonymous users — they will
 *                         always land outside any rollout bucket (flag = false).
 */
function evaluateRule(
  rule: FlagRule,
  flagName: string,
  userIdentifier: string | null | undefined,
): boolean {
  // Fully-on / fully-off
  if ("enabled" in rule) {
    return rule.enabled;
  }

  // Percentage rollout — requires a stable user identifier
  if ("rollout" in rule) {
    if (!userIdentifier) return false;
    const bucket = stableHash(`${flagName}:${userIdentifier}`);
    return bucket < rule.rollout;
  }

  return false;
}

// ---------------------------------------------------------------------------
// Context
// ---------------------------------------------------------------------------

interface FeatureFlagContextValue {
  /**
   * Returns true if the flag is enabled for the current user.
   * Falls back to false for any unknown flag.
   */
  isEnabled: (flag: FlagName) => boolean;

  /** Expose the raw config for debugging / DevTools. */
  config: FlagsConfig;
}

const FeatureFlagContext = createContext<FeatureFlagContextValue | undefined>(
  undefined,
);

// ---------------------------------------------------------------------------
// Provider
// ---------------------------------------------------------------------------

interface FeatureFlagProviderProps {
  children: ReactNode;
  /**
   * A stable per-user identifier used to keep rollout buckets consistent
   * across page loads.  Pass the authenticated user's ID or wallet address.
   * Omit (or pass null) for unauthenticated users.
   */
  userIdentifier?: string | null;
}

export function FeatureFlagProvider({
  children,
  userIdentifier,
}: FeatureFlagProviderProps) {
  // Config is parsed once at module-evaluation time (build) and memoised here.
  const config = useMemo(() => parseFlagsConfig(), []);

  const isEnabled = useMemo(
    () =>
      (flag: FlagName): boolean => {
        const rule = config[flag];
        if (rule === undefined) return false;
        return evaluateRule(rule, flag, userIdentifier ?? null);
      },
    [config, userIdentifier],
  );

  return (
    <FeatureFlagContext.Provider value={{ isEnabled, config }}>
      {children}
    </FeatureFlagContext.Provider>
  );
}

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

export function useFeatureFlag(): FeatureFlagContextValue {
  const ctx = useContext(FeatureFlagContext);
  if (!ctx) {
    throw new Error(
      "useFeatureFlag must be used within a <FeatureFlagProvider>.",
    );
  }
  return ctx;
}
