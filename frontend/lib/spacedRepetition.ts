/**
 * SM-2 spaced-repetition scheduler for Opening Repertoire Trainer (#1112 FE-53).
 * Classic SuperMemo-2 with KnightVerse-tuned bounds.
 */

export interface SrsCardState {
  /** Easiness factor, clamped to [1.3, 2.5]. */
  easiness: number;
  /** Current interval in days. */
  interval: number;
  /** Consecutive successful repetitions. */
  repetitions: number;
  /** Epoch ms when the card is next due. */
  nextReview: number;
  /** Total lapses (quality < 3 resets). */
  lapses: number;
  lastReviewed?: number;
  lastQuality?: number;
}

export const DEFAULT_EASINESS = 2.5;
export const MIN_EASINESS = 1.3;
export const MAX_EASINESS = 2.5;

export function initialCardState(now = Date.now()): SrsCardState {
  return {
    easiness: DEFAULT_EASINESS,
    interval: 0,
    repetitions: 0,
    nextReview: now,
    lapses: 0,
  };
}

/**
 * Apply one SM-2 review step.
 * quality: 0-5 (5 = perfect, 4 = hesitant, 3 = difficult but correct,
 * 0-2 = incorrect / blackout).
 */
export function sm2Review(
  prev: SrsCardState,
  quality: number,
  now = Date.now(),
): SrsCardState {
  const q = Math.max(0, Math.min(5, Math.round(quality)));
  let { easiness, interval, repetitions, lapses } = prev;

  // Update easiness factor first (applies even on failure, per SM-2).
  easiness =
    easiness + (0.1 - (5 - q) * (0.08 + (5 - q) * 0.02));
  if (easiness < MIN_EASINESS) easiness = MIN_EASINESS;
  if (easiness > MAX_EASINESS) easiness = MAX_EASINESS;

  if (q < 3) {
    // Failed recall: reset streak, short re-learning interval.
    repetitions = 0;
    lapses += 1;
    interval = 1;
  } else {
    repetitions += 1;
    if (repetitions === 1) interval = 1;
    else if (repetitions === 2) interval = 6;
    else interval = Math.max(1, Math.round(interval * easiness));
  }

  const nextReview = now + interval * 24 * 60 * 60 * 1000;
  return {
    easiness,
    interval,
    repetitions,
    nextReview,
    lapses,
    lastReviewed: now,
    lastQuality: q,
  };
}

/** Cards with nextReview <= now, oldest first. */
export function dueCards<T extends { srs: SrsCardState }>(
  cards: T[],
  now = Date.now(),
): T[] {
  return cards
    .filter((c) => c.srs.nextReview <= now)
    .sort((a, b) => a.srs.nextReview - b.srs.nextReview);
}

/** Map drill outcome to SM-2 quality. */
export function qualityFromDrillResult(opts: {
  correct: boolean;
  hintsUsed: number;
  attempts: number;
}): number {
  const { correct, hintsUsed, attempts } = opts;
  if (!correct) return attempts > 2 ? 0 : 2;
  if (hintsUsed > 0 || attempts > 1) return 3;
  if (attempts === 1 && hintsUsed === 0) return 5;
  return 4;
}
