/**
 * ELO rating calculation service (#556)
 *
 * Pure TypeScript implementation of the standard Elo formula, mirroring the
 * Rust implementation in backend/modules/matchmaking/elo.rs.
 *
 * Formula:
 *   E  = 1 / (1 + 10^((opponent - rating) / 400))
 *   R' = R + K * (S - E)   where S = 1 (win) | 0.5 (draw) | 0 (loss)
 */

export type GameResult = "win" | "draw" | "loss";

export interface EloUpdateResult {
  newRating: number;
  delta: number;
}

export interface MatchEloResult {
  winner: EloUpdateResult;
  loser: EloUpdateResult;
}

/**
 * Standard K-factor selection based on rating and games played.
 *
 * FIDE conventions:
 *   - K = 40  for new players (< 30 games) or rating < 1000
 *   - K = 20  for established players (rating < 2400)
 *   - K = 10  for elite players (rating ≥ 2400)
 */
export function kFactor(rating: number, gamesPlayed: number): number {
  if (gamesPlayed < 30 || rating < 1000) return 40;
  if (rating < 2400) return 20;
  return 10;
}

/**
 * Expected score for `rating` against `opponentRating`.
 * Returns a value in [0, 1].
 */
export function expectedScore(rating: number, opponentRating: number): number {
  return 1 / (1 + Math.pow(10, (opponentRating - rating) / 400));
}

/**
 * Calculate the new Elo rating for a single player after one game.
 *
 * @param rating         Current rating
 * @param opponentRating Opponent's current rating
 * @param result         Outcome from this player's perspective
 * @param k              K-factor (use `kFactor()` if unsure)
 * @returns              New rating (floored to integer, minimum 100)
 */
export function calculateNewRating(
  rating: number,
  opponentRating: number,
  result: GameResult,
  k: number,
): EloUpdateResult {
  const score = result === "win" ? 1 : result === "draw" ? 0.5 : 0;
  const expected = expectedScore(rating, opponentRating);
  const delta = Math.round(k * (score - expected));
  const newRating = Math.max(100, rating + delta);
  return { newRating, delta };
}

/**
 * Calculate updated ratings for both players after a match.
 *
 * @param winnerRating       Winner's current rating
 * @param loserRating        Loser's current rating
 * @param winnerGamesPlayed  Used to determine winner's K-factor
 * @param loserGamesPlayed   Used to determine loser's K-factor
 */
export function calculateMatchRatings(
  winnerRating: number,
  loserRating: number,
  winnerGamesPlayed = 30,
  loserGamesPlayed = 30,
): MatchEloResult {
  const kWinner = kFactor(winnerRating, winnerGamesPlayed);
  const kLoser = kFactor(loserRating, loserGamesPlayed);

  const winner = calculateNewRating(winnerRating, loserRating, "win", kWinner);
  const loser = calculateNewRating(loserRating, winnerRating, "loss", kLoser);

  return { winner, loser };
}

/**
 * Calculate updated ratings for a drawn game.
 */
export function calculateDrawRatings(
  player1Rating: number,
  player2Rating: number,
  player1GamesPlayed = 30,
  player2GamesPlayed = 30,
): { player1: EloUpdateResult; player2: EloUpdateResult } {
  const k1 = kFactor(player1Rating, player1GamesPlayed);
  const k2 = kFactor(player2Rating, player2GamesPlayed);

  const player1 = calculateNewRating(player1Rating, player2Rating, "draw", k1);
  const player2 = calculateNewRating(player2Rating, player1Rating, "draw", k2);

  return { player1, player2 };
}
