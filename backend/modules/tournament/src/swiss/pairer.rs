use super::*;
use std::collections::HashMap;

pub struct SwissPairer {
    config: SwissConfig,
}

impl SwissPairer {
    pub fn new(config: SwissConfig) -> Self {
        Self { config }
    }

    pub fn pair_round(&self, tournament: &mut TournamentState) -> Result<Vec<PairingResult>, PairingError> {
        // Clone players to avoid borrow issues
        let players: Vec<Player> = tournament.players.values().cloned().collect();
        let mut player_refs: Vec<&Player> = players.iter().collect();
        player_refs.sort_by(|a, b| {
            b.score.partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(b.rating.cmp(&a.rating))
        });
        
        // Handle odd number of players - assign bye to lowest ranked
        let pairings = if player_refs.len() % 2 == 1 {
            let bye_player_id = self.assign_bye(&mut player_refs, tournament)?;
            let pairings = self.pair_even_players(player_refs, tournament)?;
            pairings
                .into_iter()
                .chain(vec![PairingResult::Bye(bye_player_id)])
                .collect()
        } else {
            self.pair_even_players(player_refs, tournament)?
        };

        // Persist generated pairings so round results can be applied deterministically.
        for result in &pairings {
            if let PairingResult::Paired(pairing) = result {
                tournament.pairings.push(pairing.clone());
            }
        }

        Ok(pairings)
    }

    fn assign_bye(&self, players: &mut Vec<&Player>, tournament: &mut TournamentState) -> Result<Uuid, PairingError> {
        // Find the lowest ranked player who hasn't had a bye yet
        let bye_candidate = players
            .iter()
            .enumerate()
            .filter(|(_, p): &(_, &&Player)| !p.has_had_bye())
            .min_by(|(_, a), (_, b)| {
                a.score.partial_cmp(&b.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then(a.rating.cmp(&b.rating))
            });

        match bye_candidate {
            Some((index, player)) => {
                let player_id = player.id;
                players.remove(index);
                
                // Award 1 point for bye
                if let Some(p) = tournament.players.get_mut(&player_id) {
                    p.score += 1.0;
                }
                
                Ok(player_id)
            }
            None => Err(PairingError::NoValidByeCandidate),
        }
    }

    fn pair_even_players(&self, players: Vec<&Player>, tournament: &mut TournamentState) -> Result<Vec<PairingResult>, PairingError> {
        let mut pairings = Vec::new();
        let _unpaired_players: Vec<Uuid> = players.iter().map(|p| p.id).collect();
        let mut used_players = std::collections::HashSet::new();

        // Dutch System: Process score groups
        let mut score_groups = self.create_score_groups(&players);
        
        for group in score_groups.iter_mut() {
            if group.len() < 2 {
                continue;
            }

            // Sort within group by rating (higher first)
            group.sort_by(|a, b| b.rating.cmp(&a.rating));

            // Pair within score group first
            let group_pairings = self.pair_within_group(group, tournament, &mut used_players)?;
            pairings.extend(group_pairings);
        }

        // Handle remaining players with score differences (floaters)
        let remaining_players: Vec<&Player> = players
            .iter()
            .filter(|p| !used_players.contains(&p.id))
            .copied()
            .collect();

        if !remaining_players.is_empty() {
            let float_pairings = self.handle_floaters(remaining_players, tournament)?;
            pairings.extend(float_pairings);
        }

        Ok(pairings)
    }

    fn create_score_groups<'a>(&self, players: &[&'a Player]) -> Vec<Vec<&'a Player>> {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut groups: HashMap<u64, Vec<&'a Player>> = HashMap::new();
        
        for player in players {
            let mut hasher = DefaultHasher::new();
            let score_bits = player.score.to_bits();
            score_bits.hash(&mut hasher);
            let key = hasher.finish();
            
            groups
                .entry(key)
                .or_insert_with(Vec::new)
                .push(player);
        }

        let mut sorted_groups: Vec<Vec<&'a Player>> = groups
            .into_values()
            .collect();
        
        // Sort groups by score (highest first)
        sorted_groups.sort_by(|a, b| b[0].score.partial_cmp(&a[0].score).unwrap());
        sorted_groups
    }

    fn pair_within_group(
        &self,
        group: &[&Player],
        tournament: &mut TournamentState,
        used_players: &mut std::collections::HashSet<Uuid>,
    ) -> Result<Vec<PairingResult>, PairingError> {
        let mut pairings = Vec::new();
        let mut group_players: Vec<&Player> = group.to_vec();

        // Try to pair players avoiding color repeats and previous opponents
        while group_players.len() >= 2 {
            let player1 = group_players[0];
            let mut found_pair = false;

            // Find best opponent for player1
            for (i, &player2) in group_players.iter().enumerate().skip(1) {
                if self.can_pair(player1, player2, tournament) {
                    let pairing = self.create_pairing(player1, player2, tournament.current_round)?;
                    pairings.push(PairingResult::Paired(pairing));
                    
                    // Update float scores
                    self.update_float_scores(player1, player2, tournament, false);
                    
                    used_players.insert(player1.id);
                    used_players.insert(player2.id);
                    
                    group_players.remove(i);
                    group_players.remove(0);
                    found_pair = true;
                    break;
                }
            }

            if !found_pair {
                // No valid pair found in this group, will be handled as floater
                break;
            }
        }

        Ok(pairings)
    }

    fn handle_floaters(
        &self,
        remaining_players: Vec<&Player>,
        tournament: &mut TournamentState,
    ) -> Result<Vec<PairingResult>, PairingError> {
        let mut pairings = Vec::new();
        let mut players = remaining_players;

        // Sort remaining players by score then rating
        players.sort_by(|a, b| {
            b.score.partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(b.rating.cmp(&a.rating))
        });

        // Pair remaining players, allowing score differences
        for i in (0..players.len()).step_by(2) {
            if i + 1 >= players.len() {
                break;
            }

            let player1 = players[i];
            let player2 = players[i + 1];

            if self.can_pair(player1, player2, tournament) {
                let pairing = self.create_pairing(player1, player2, tournament.current_round)?;
                pairings.push(PairingResult::Paired(pairing));
                
                // Update float scores (these are floaters)
                self.update_float_scores(player1, player2, tournament, true);
            } else {
                return Err(PairingError::CannotPairRemainingPlayers);
            }
        }

        Ok(pairings)
    }

    fn can_pair(&self, player1: &Player, player2: &Player, _tournament: &TournamentState) -> bool {
        // Basic checks
        if !player1.can_be_paired_with(player2) {
            return false;
        }

        // Color balance preference
        let color_preference_ok = self.check_color_preference(player1, player2);

        color_preference_ok
    }

    fn check_color_preference(&self, player1: &Player, player2: &Player) -> bool {
        let p1_prefers_white = player1.should_prefer_white();
        let p2_prefers_white = player2.should_prefer_white();

        // Prefer giving white to player who needs it more
        if p1_prefers_white && !p2_prefers_white {
            return true;
        }
        if !p1_prefers_white && p2_prefers_white {
            return true;
        }

        // If both prefer same color, it's still acceptable but less ideal
        true
    }

    fn create_pairing(&self, player1: &Player, player2: &Player, round: u32) -> Result<Pairing, PairingError> {
        let (white_player, black_player) = if player1.should_prefer_white() {
            (player1.id, player2.id)
        } else if player2.should_prefer_white() {
            (player2.id, player1.id)
        } else {
            // If neither has strong preference, higher rating gets white
            if player1.rating >= player2.rating {
                (player1.id, player2.id)
            } else {
                (player2.id, player1.id)
            }
        };

        Ok(Pairing {
            white_player,
            black_player,
            round,
        })
    }

    fn update_float_scores(
        &self,
        player1: &Player,
        player2: &Player,
        tournament: &mut TournamentState,
        is_floater: bool,
    ) {
        if is_floater {
            // Update float scores for players paired across score groups
            let _score_diff = (player1.score - player2.score).abs();
            
            if player1.score > player2.score {
                if let Some(p) = tournament.players.get_mut(&player1.id) {
                    p.float_score += 1; // Floating down
                }
                if let Some(p) = tournament.players.get_mut(&player2.id) {
                    p.float_score -= 1; // Floating up
                }
            } else if player2.score > player1.score {
                if let Some(p) = tournament.players.get_mut(&player1.id) {
                    p.float_score -= 1; // Floating up
                }
                if let Some(p) = tournament.players.get_mut(&player2.id) {
                    p.float_score += 1; // Floating down
                }
            }
        }
    }
}

// Extension methods for Player
impl Player {
    pub fn has_had_bye(&self) -> bool {
        // Check if player has a full point from a round without an opponent
        self.score == 1.0 && self.opponents.is_empty() && self.completed_rounds() > 0
    }

    pub fn completed_rounds(&self) -> u32 {
        self.opponents.len() as u32
    }
}

#[derive(Debug, Clone)]
pub enum PairingError {
    NoValidByeCandidate,
    CannotPairRemainingPlayers,
    InsufficientPlayers,
    InvalidTournamentState,
}

impl std::fmt::Display for PairingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PairingError::NoValidByeCandidate => write!(f, "No valid candidate for bye assignment"),
            PairingError::CannotPairRemainingPlayers => write!(f, "Cannot pair remaining players"),
            PairingError::InsufficientPlayers => write!(f, "Insufficient players for pairing"),
            PairingError::InvalidTournamentState => write!(f, "Invalid tournament state"),
        }
    }
}

impl std::error::Error for PairingError {}
