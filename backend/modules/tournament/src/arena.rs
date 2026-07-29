use crate::pairing::{Pairing, PairingStrategy, TournamentPlayer};
use std::collections::HashSet;

pub struct ArenaPairingStrategy;

impl Default for ArenaPairingStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl ArenaPairingStrategy {
    pub fn new() -> Self {
        Self
    }
}

impl PairingStrategy for ArenaPairingStrategy {
    fn pair(&self, mut players: Vec<TournamentPlayer>) -> (Vec<Pairing>, Vec<TournamentPlayer>) {
        if players.is_empty() {
            return (vec![], vec![]);
        }

        // 1. Sort by ELO (descending or ascending doesn't matter for closeness, let's use descending)
        players.sort_by(|a, b| b.elo.cmp(&a.elo));

        let mut pairings = Vec::new();
        let mut paired_indices = HashSet::new();
        let mut remaining_players = Vec::new();

        for i in 0..players.len() {
            if paired_indices.contains(&i) {
                continue;
            }

            let player_a = &players[i];
            let mut best_match_idx = None;
            let mut fallback_match_idx = None; // Closest player even if repeated

            // Search for a suitable opponent
            for j in (i + 1)..players.len() {
                if paired_indices.contains(&j) {
                    continue;
                }

                let player_b = &players[j];

                // Track the closest available player as fallback (soft constraint)
                if fallback_match_idx.is_none() {
                    fallback_match_idx = Some(j);
                }

                // Check soft constraint: avoid pairing if played recently
                // Assuming recent_opponents contains IDs of players played against.
                // We check if the LAST opponent is player_b.
                let played_recently = player_a
                    .recent_opponents
                    .last()
                    .is_some_and(|id| *id == player_b.id)
                    || player_b
                        .recent_opponents
                        .last()
                        .is_some_and(|id| *id == player_a.id);

                if !played_recently {
                    best_match_idx = Some(j);
                    break; // Found the best (closest ELO) non-repeat match
                }
            }

            // Use best match if found, otherwise fallback
            let target_idx = best_match_idx.or(fallback_match_idx);

            if let Some(j) = target_idx {
                paired_indices.insert(i);
                paired_indices.insert(j);
                pairings.push(Pairing {
                    player1: players[i].clone(),
                    player2: players[j].clone(),
                });
            } else {
                // No opponent found (e.g., last player), will be added to remaining
            }
        }

        // Collect remaining players
        for (i, player) in players.into_iter().enumerate() {
            if !paired_indices.contains(&i) {
                remaining_players.push(player);
            }
        }

        (pairings, remaining_players)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

    fn create_player(elo: u32, recent_opponents: Vec<Uuid>) -> TournamentPlayer {
        TournamentPlayer {
            id: Uuid::new_v4(),
            elo,
            joined_at: Utc::now(),
            recent_opponents,
        }
    }

    #[test]
    fn test_pair_basic() {
        let p1 = create_player(1000, vec![]);
        let p2 = create_player(1100, vec![]); // Closest to p1
        let p3 = create_player(1200, vec![]);

        let players = vec![p1.clone(), p2.clone(), p3.clone()];
        let strategy = ArenaPairingStrategy::new();
        let (pairs, left) = strategy.pair(players);

        assert_eq!(pairs.len(), 1);
        assert_eq!(left.len(), 1);

        // Should pair 1200 and 1100 (closest), leaving 1000?
        // Or 1200(p3), 1100(p2), 1000(p1) -> p3 paired with p2 (diff 100), p1 left.
        // Wait, p3(1200) vs p2(1100) = 100.
        // p2(1100) vs p1(1000) = 100.
        // Greedy: p3 (first) pairs with p2. p1 left.

        // Let's verify IDs.
        let paired_ids: Vec<Uuid> = pairs
            .iter()
            .flat_map(|p| vec![p.player1.id, p.player2.id])
            .collect();
        assert!(paired_ids.contains(&p3.id));
        assert!(paired_ids.contains(&p2.id));
        assert_eq!(left[0].id, p1.id);
    }

    #[test]
    fn test_pair_avoid_repeat() {
        // We need the first player in sort order to have the restriction.
        // p1(2000), p2(1990), p3(1900).
        // p1 played p2.
        // p1 checks p2 -> repeat.
        // p1 checks p3 -> OK.
        // Pairs p1-p3. p2 left.

        let id_b = Uuid::new_v4();
        let p_a = create_player(2000, vec![id_b]);
        let mut p_b = create_player(1990, vec![]);
        p_b.id = id_b;
        let p_c = create_player(1900, vec![]);

        let strat = ArenaPairingStrategy::new();
        let (pairs, _left) = strat.pair(vec![p_a.clone(), p_b.clone(), p_c.clone()]);

        assert_eq!(pairs[0].player2.id, p_c.id); // Should skip p_b and pick p_c
    }

    #[test]
    fn test_pair_soft_constraint_fallback() {
        // p1(2000) played p2(1900). No other players.
        // Should pair them anyway.
        let id2 = Uuid::new_v4();
        let p1 = create_player(2000, vec![id2]);
        let mut p2 = create_player(1900, vec![]);
        p2.id = id2;

        let strat = ArenaPairingStrategy::new();
        let (pairs, _left) = strat.pair(vec![p1, p2]);

        assert_eq!(pairs.len(), 1);
    }

    #[test]
    #[ignore]
    fn test_pair_performance_1000_players() {
        use std::time::Instant;

        // Generate 1000 players with random ELOs
        let mut players = Vec::new();
        for i in 0..1000 {
            players.push(create_player(1000 + (i % 500) as u32, vec![]));
        }

        let strat = ArenaPairingStrategy::new();
        let start = Instant::now();
        let (pairs, left) = strat.pair(players);
        let duration = start.elapsed();

        println!("Pairing 1000 players took: {:?}", duration);

        assert_eq!(pairs.len(), 500);
        assert_eq!(left.len(), 0);
        // Expect < 50ms, usually < 10ms for O(N^2) or O(N log N) with 1000 items
        assert!(
            duration.as_millis() < 50,
            "Pairing took too long: {:?}",
            duration
        );
    }
}
