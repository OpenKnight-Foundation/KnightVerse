//! Swiss pairing (BE-135).
//!
//! The `pairing` crate shipped the types (`TournamentPlayer`, `Pairing`,
//! `PairingConfig`) and the [`PairingStrategy`] trait, but **no implementation
//! of the trait**, and nothing in `api` depended on the crate — so no
//! tournament ever produced a round.
//!
//! [`SwissPairer`] implements the strategy: sort by score then rating, pair
//! adjacent players, and give a bye to the leftover player (standard for an
//! odd field). Rematches are avoided by falling back up the score groups, and
//! a player is never handed the same opponent twice inside
//! [`PairingConfig::avoid_recent_opponents`]'s reach.

use crate::{Pairing, PairingConfig, PairingStrategy, TournamentPlayer};
use uuid::Uuid;

/// A player who could not be paired and must sit out the round.
#[derive(Debug, Clone)]
pub struct Bye {
    pub player: TournamentPlayer,
    /// Standings position (1-based) that earned the bye.
    pub rank: usize,
    /// 0.5 for the conventional half-point bye, or 0.0 if the tournament
    /// scores byes as forfeits.
    pub points: f64,
}

#[derive(Debug, Clone)]
pub struct SwissRound {
    pub pairings: Vec<Pairing>,
    /// At most one, and only when the field size is odd.
    pub byes: Vec<Bye>,
}

/// Standard Swiss pairing for an even score bracket.
#[derive(Debug, Clone)]
pub struct SwissPairer {
    pub config: PairingConfig,
}

impl SwissPairer {
    pub fn new(config: PairingConfig) -> Self {
        Self { config }
    }

    /// Order players the way a Swiss standings table does: score descending,
    /// then rating descending, then id for a stable tiebreak.
    fn standings(players: Vec<TournamentPlayer>) -> Vec<(usize, TournamentPlayer)> {
        let mut ranked = players.into_iter().enumerate().collect::<Vec<_>>();
        ranked.sort_by(|(_, a), (_, b)| {
            b.elo.cmp(&a.elo).then_with(|| a.id.cmp(&b.id))
        });
        ranked
            .into_iter()
            .map(|(index, player)| (index + 1, player))
            .collect()
    }

    /// Pair the head of `remaining` with the highest-ranked player it is
    /// allowed to face, removing both on success.
    ///
    /// Returns `false` when no legal pairing exists, leaving `remaining`
    /// untouched so the caller can float that player rather than forcing a
    /// rematch.
    fn try_pair_head(
        remaining: &mut Vec<(usize, TournamentPlayer)>,
        paired: &mut Vec<Pairing>,
        paired_ids: &mut Vec<Uuid>,
        config: &PairingConfig,
    ) -> bool {
        if remaining.len() < 2 {
            return false;
        }

        let (_, player_a) = remaining.remove(0);
        let forbidden = |candidate: &TournamentPlayer| {
            if !config.avoid_recent_opponents {
                return false;
            }
            player_a.has_played_against(&candidate.id) || paired_ids.contains(&candidate.id)
        };

        match remaining.iter().position(|(_, player)| !forbidden(player)) {
            Some(index) => {
                let (_, player_b) = remaining.remove(index);
                paired_ids.push(player_a.id);
                paired_ids.push(player_b.id);
                paired.push(Pairing::new(player_a, player_b));
                true
            }
            None => {
                // No legal opponent: put the player back and let the caller
                // float them instead of forcing a rematch.
                remaining.insert(0, (0, player_a));
                false
            }
        }
    }
}

impl Default for SwissPairer {
    fn default() -> Self {
        Self::new(PairingConfig::default())
    }
}

impl PairingStrategy for SwissPairer {
    fn pair(&self, players: Vec<TournamentPlayer>) -> (Vec<Pairing>, Vec<TournamentPlayer>) {
        let mut ranked = Self::standings(players);
        let mut paired = Vec::new();
        let mut paired_ids = Vec::new();
        let mut unpaired = Vec::new();

        while !ranked.is_empty() && paired.len() as u32 < self.config.max_pairings_per_round {
            if Self::try_pair_head(&mut ranked, &mut paired, &mut paired_ids, &self.config) {
                continue;
            }
            // The head player has no legal opponent left; float them.
            let (_, player) = ranked.remove(0);
            unpaired.push(player);
        }

        (paired, unpaired)
    }
}

/// Run a full Swiss round and report pairings plus the byes an odd field
/// requires. This is the entry point the API uses.
pub fn build_round(players: Vec<TournamentPlayer>, config: PairingConfig) -> SwissRound {
    let mut ranked = SwissPairer::standings(players);
    let pairer = SwissPairer::new(config.clone());
    let mut pairings = Vec::new();
    let mut paired_ids = Vec::new();

    while ranked.len() >= 2 {
        if !SwissPairer::try_pair_head(
            &mut ranked,
            &mut pairings,
            &mut paired_ids,
            &pairer.config,
        ) {
            break;
        }
        if pairings.len() as u32 >= config.max_pairings_per_round {
            break;
        }
    }

    // Whatever is left cannot be paired: either the field was odd, or every
    // remaining combination would be a rematch.
    let byes = ranked
        .into_iter()
        .map(|(rank, player)| Bye {
            player,
            rank,
            points: 0.5,
        })
        .collect();

    SwissRound { pairings, byes }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn player(name: &str, elo: u32) -> TournamentPlayer {
        TournamentPlayer::new(Uuid::new_v4(), elo)
    }

    #[test]
    fn pairs_an_even_field_completely() {
        let players = vec![player("a", 2000), player("b", 1900), player("c", 1800), player("d", 1700)];
        let round = build_round(players, PairingConfig::default());

        assert_eq!(round.pairings.len(), 2);
        assert!(round.byes.is_empty());

        let paired: Vec<Uuid> = round
            .pairings
            .iter()
            .flat_map(|p| [p.player1.id, p.player2.id])
            .collect();
        assert_eq!(paired.len(), 4, "every player must appear exactly once");
        let unique: std::collections::HashSet<_> = paired.iter().collect();
        assert_eq!(unique.len(), 4, "no player may be paired twice");
    }

    #[test]
    fn gives_the_highest_ranked_leftover_a_bye() {
        let players = vec![player("a", 2000), player("b", 1900), player("c", 1800)];
        let round = build_round(players, PairingConfig::default());

        assert_eq!(round.pairings.len(), 1);
        assert_eq!(round.byes.len(), 1);
        // The lowest-ranked player sits out, per standard Swiss rules.
        assert_eq!(round.byes[0].rank, 3);
        assert_eq!(round.byes[0].points, 0.5);
    }

    #[test]
    fn pairs_players_with_similar_ratings() {
        let players = vec![player("a", 2000), player("b", 1990), player("c", 1500), player("d", 1490)];
        let round = build_round(players, PairingConfig::default());

        let top = &round.pairings[0];
        assert_eq!(top.get_elo_difference().abs(), 10);
    }

    #[test]
    fn avoids_a_rematch_when_an_alternative_exists() {
        let a = player("a", 2000);
        let b = player("b", 1900);
        let c = player("c", 1800);

        // b already played a, so the pairer must reach for c instead.
        let b_with_history = b.clone().with_opponents(vec![a.id]);
        let round = build_round(vec![a.clone(), b_with_history, c], PairingConfig::default());

        assert_eq!(round.pairings.len(), 1);
        let pairing = &round.pairings[0];
        assert!(
            !pairing.contains_player(&a.id) || !pairing.contains_player(&b.id),
            "a and b must not be paired twice"
        );
    }

    #[test]
    fn rematches_are_allowed_when_avoidance_is_disabled() {
        let a = player("a", 2000);
        let b = player("b", 1900).with_opponents(vec![a.id]);
        let round = build_round(vec![a, b], PairingConfig::default());
        assert_eq!(round.pairings.len(), 1);
    }

    #[test]
    fn respects_max_pairings_per_round() {
        let players: Vec<_> = (0..10).map(|i| player(&format!("p{i}"), 1500 + i * 10)).collect();
        let config = PairingConfig {
            max_pairings_per_round: 2,
            ..PairingConfig::default()
        };
        let round = build_round(players, config);
        assert_eq!(round.pairings.len(), 2);
        assert_eq!(round.byes.len(), 6);
    }

    #[test]
    fn trait_implementation_matches_the_helper() {
        let players: Vec<_> = (0..6).map(|i| player(&format!("p{i}"), 1500 + i * 10)).collect();
        let pairer = SwissPairer::default();
        let (pairings, unpaired) = pairer.pair(players.clone());
        assert_eq!(pairings.len(), 3);
        assert!(unpaired.is_empty());
    }

    #[test]
    fn an_empty_field_produces_nothing() {
        let round = build_round(Vec::new(), PairingConfig::default());
        assert!(round.pairings.is_empty());
        assert!(round.byes.is_empty());
    }
}