/// Pure Elo rating calculation.
///
/// This module is intentionally stateless: it returns new ratings without
/// performing any persistence or side-effects.
///
/// Formula:
/// - Expected score: E = 1 / (1 + 10^((opp - rating)/400))
/// - New rating: R' = R + K * (S - E) where S is 1.0 for win, 0.0 for loss
pub fn calculate_new_ratings(winner: u32, loser: u32, k: u32) -> (u32, u32) {
    if k == 0 || winner == loser {
        // Winner/loser equality still yields +k/2/-k/2 in “real” Elo, but keeping
        // this branch makes the function behavior explicit and testable for k=0.
        // For winner==loser and k>0, the standard formula below produces +k/2.
        if k == 0 {
            return (winner, loser);
        }
    }

    let winner_f = winner as f64;
    let loser_f = loser as f64;
    let k_f = k as f64;

    // Expected score for the winner.
    let exponent = (loser_f - winner_f) / 400.0;
    let expected_winner = 1.0 / (1.0 + 10_f64.powf(exponent));

    // Winner gets score 1.0, loser gets 0.0.
    let delta = (k_f * (1.0 - expected_winner)).round();
    let delta_u32 = if delta <= 0.0 { 0 } else { delta as u32 };

    let new_winner = winner.saturating_add(delta_u32);
    let new_loser = loser.saturating_sub(delta_u32);

    (new_winner, new_loser)
}

#[cfg(test)]
mod tests {
    use super::calculate_new_ratings;

    #[test]
    fn equal_ratings_k_32() {
        let (w, l) = calculate_new_ratings(1500, 1500, 32);
        assert_eq!(w, 1516);
        assert_eq!(l, 1484);
    }

    #[test]
    fn equal_ratings_k_31_rounds_half_up() {
        // 31 * 0.5 = 15.5 -> round() => 16
        let (w, l) = calculate_new_ratings(1500, 1500, 31);
        assert_eq!(w, 1516);
        assert_eq!(l, 1484);
    }

    #[test]
    fn k_zero_is_noop() {
        let (w, l) = calculate_new_ratings(1200, 1800, 0);
        assert_eq!((w, l), (1200, 1800));
    }

    #[test]
    fn high_rated_wins_over_low_rated_small_change() {
        // With a large advantage, expected score ~1, so delta should be ~0.
        let (w, l) = calculate_new_ratings(3000, 100, 32);
        assert_eq!((w, l), (3000, 100));
    }

    #[test]
    fn low_rated_upset_over_high_rated_large_change() {
        // With a huge disadvantage, expected score ~0, so delta should be ~K.
        let (w, l) = calculate_new_ratings(100, 3000, 32);
        assert_eq!((w, l), (132, 2968));
    }

    #[test]
    fn typical_mismatch_changes_are_reasonable() {
        let (w, l) = calculate_new_ratings(1600, 1400, 32);
        // Winner is favored, but not overwhelmingly. Delta should be >0 and <16.
        assert!(w > 1600 && w < 1616, "winner new rating {}", w);
        assert_eq!(w - 1600, 1400 - l, "delta should be symmetric");
    }

    #[test]
    fn underflow_is_safely_clamped() {
        // If delta exceeds loser rating (possible with very low loser rating and high K),
        // saturating_sub clamps to 0.
        let (_, l) = calculate_new_ratings(0, 1, u32::MAX);
        assert_eq!(l, 0);
    }
}

