export type SeriesLength = 1 | 3 | 5;
export type SeriesResult = "win" | "loss" | "draw";
export type SeriesOutcome = SeriesResult | null;

export interface SeriesScore {
  length: SeriesLength;
  playerWins: number;
  opponentWins: number;
  draws: number;
  gameNumber: number;
}

export const SERIES_OPTIONS: { length: SeriesLength; label: string; payoutMultiplier: number }[] = [
  { length: 1, label: "Bo1", payoutMultiplier: 1 },
  { length: 3, label: "Bo3", payoutMultiplier: 1.8 },
  { length: 5, label: "Bo5", payoutMultiplier: 2.5 },
];

export function parseSeriesLength(value: string | null | undefined): SeriesLength {
  return value === "3" ? 3 : value === "5" ? 5 : 1;
}

export function seriesTarget(length: SeriesLength): number {
  return (length + 1) / 2;
}

export function initialSeries(length: SeriesLength): SeriesScore {
  return { length, playerWins: 0, opponentWins: 0, draws: 0, gameNumber: 1 };
}

export function seriesOutcome(score: SeriesScore): SeriesOutcome {
  const target = seriesTarget(score.length);
  if (score.playerWins >= target) return "win";
  if (score.opponentWins >= target) return "loss";
  if (score.gameNumber >= score.length) return "draw";
  return null;
}

export function recordSeriesGame(score: SeriesScore, result: SeriesResult): SeriesScore {
  const next: SeriesScore = {
    ...score,
    playerWins: score.playerWins + (result === "win" ? 1 : 0),
    opponentWins: score.opponentWins + (result === "loss" ? 1 : 0),
    draws: score.draws + (result === "draw" ? 1 : 0),
  };
  return seriesOutcome(next) === null
    ? { ...next, gameNumber: score.gameNumber + 1 }
    : next;
}

export function seriesPayout(stake: number | null, length: SeriesLength, outcome: SeriesOutcome): number | null {
  if (stake === null || !Number.isFinite(stake) || stake <= 0 || outcome !== "win") return null;
  const option = SERIES_OPTIONS.find((item) => item.length === length);
  return (option?.payoutMultiplier ?? 1) * stake;
}

export function seriesMultiplier(length: SeriesLength): number {
  return SERIES_OPTIONS.find((item) => item.length === length)?.payoutMultiplier ?? 1;
}
