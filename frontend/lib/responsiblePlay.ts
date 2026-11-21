export const SELF_EXCLUSION_DELAY_HOURS = 24;

export interface DailyUsage {
  date: string;
  wageredUsd: number;
}

export interface ResponsiblePlaySettings {
  dailyLimitUsd: number;
  selfExclusionStartsAt?: string | null;
  selfExclusionEndsAt?: string | null;
}

export interface SelfExclusionState {
  active: boolean;
  pending: boolean;
  hoursUntilActive: number;
  startsAt?: string;
  endsAt?: string;
}

const SETTINGS_KEY = "knightverse_responsible_play_settings";
const USAGE_KEY = "knightverse_responsible_play_usage";
const DEFAULT_LIMIT = 100;

function localDateKey(now: Date): string {
  const year = now.getFullYear();
  const month = String(now.getMonth() + 1).padStart(2, "0");
  const day = String(now.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

function readJson<T>(key: string): T | null {
  if (typeof window === "undefined") return null;
  try {
    const raw = window.localStorage.getItem(key);
    return raw ? (JSON.parse(raw) as T) : null;
  } catch {
    return null;
  }
}

function writeJson(key: string, value: unknown): void {
  if (typeof window === "undefined") return;
  window.localStorage.setItem(key, JSON.stringify(value));
}

export function getResponsiblePlaySettings(): ResponsiblePlaySettings {
  const stored = readJson<Partial<ResponsiblePlaySettings>>(SETTINGS_KEY);
  return {
    dailyLimitUsd:
      typeof stored?.dailyLimitUsd === "number" && Number.isFinite(stored.dailyLimitUsd)
        ? Math.max(0, stored.dailyLimitUsd)
        : DEFAULT_LIMIT,
    selfExclusionStartsAt: stored?.selfExclusionStartsAt ?? null,
    selfExclusionEndsAt: stored?.selfExclusionEndsAt ?? null,
  };
}

export function saveDailyLimitUsd(amount: number): ResponsiblePlaySettings {
  const next = {
    ...getResponsiblePlaySettings(),
    dailyLimitUsd: Math.max(0, amount),
  };
  writeJson(SETTINGS_KEY, next);
  return next;
}

export function requestSelfExclusion(days: number, now: Date = new Date()): ResponsiblePlaySettings {
  const startsAt = new Date(now.getTime() + SELF_EXCLUSION_DELAY_HOURS * 60 * 60 * 1000);
  const endsAt = new Date(startsAt.getTime() + Math.max(1, days) * 24 * 60 * 60 * 1000);
  const next: ResponsiblePlaySettings = {
    ...getResponsiblePlaySettings(),
    selfExclusionStartsAt: startsAt.toISOString(),
    selfExclusionEndsAt: endsAt.toISOString(),
  };
  writeJson(SETTINGS_KEY, next);
  return next;
}

export function cancelSelfExclusion(): ResponsiblePlaySettings {
  const next: ResponsiblePlaySettings = {
    ...getResponsiblePlaySettings(),
    selfExclusionStartsAt: null,
    selfExclusionEndsAt: null,
  };
  writeJson(SETTINGS_KEY, next);
  return next;
}

export function getSelfExclusionState(
  settings: ResponsiblePlaySettings,
  now: Date = new Date(),
): SelfExclusionState {
  const startsAt = settings.selfExclusionStartsAt
    ? new Date(settings.selfExclusionStartsAt)
    : null;
  const endsAt = settings.selfExclusionEndsAt
    ? new Date(settings.selfExclusionEndsAt)
    : null;
  const validStart = startsAt && !Number.isNaN(startsAt.getTime()) ? startsAt : null;
  const validEnd = endsAt && !Number.isNaN(endsAt.getTime()) ? endsAt : null;
  const time = now.getTime();
  const active = Boolean(
    validStart && validEnd && time >= validStart.getTime() && time < validEnd.getTime(),
  );
  const pending = Boolean(validStart && validEnd && time < validStart.getTime());

  return {
    active,
    pending,
    hoursUntilActive: validStart
      ? Math.max(0, Math.ceil((validStart.getTime() - time) / (60 * 60 * 1000)))
      : 0,
    startsAt: validStart?.toISOString(),
    endsAt: validEnd?.toISOString(),
  };
}

export function getDailyUsage(now: Date = new Date()): DailyUsage {
  const date = localDateKey(now);
  const stored = readJson<DailyUsage>(USAGE_KEY);
  if (!stored || stored.date !== date || !Number.isFinite(stored.wageredUsd)) {
    return { date, wageredUsd: 0 };
  }
  return stored;
}

export function recordWager(amountUsd: number, now: Date = new Date()): DailyUsage {
  const usage = getDailyUsage(now);
  const next = {
    date: usage.date,
    wageredUsd: usage.wageredUsd + Math.max(0, amountUsd),
  };
  writeJson(USAGE_KEY, next);
  return next;
}

export function remainingDailyLimit(
  settings: ResponsiblePlaySettings,
  usage: DailyUsage,
): number {
  return Math.max(0, settings.dailyLimitUsd - usage.wageredUsd);
}

export function formatUsd(value: number): string {
  return `$${value.toLocaleString(undefined, {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  })}`;
}
