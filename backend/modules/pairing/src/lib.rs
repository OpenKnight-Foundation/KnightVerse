use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TournamentPlayer {
    pub id: Uuid,
    pub elo: u32,
    pub joined_at: DateTime<Utc>,
    pub recent_opponents: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pairing {
    pub player1: TournamentPlayer,
    pub player2: TournamentPlayer,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairingResult {
    pub player1_id: Uuid,
    pub player2_id: Uuid,
    pub board_number: u32,
}

pub trait PairingStrategy {
    fn pair(&self, players: Vec<TournamentPlayer>) -> (Vec<Pairing>, Vec<TournamentPlayer>);
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairingConfig {
    pub max_elo_difference: u32,
    pub avoid_recent_opponents: bool,
    pub balance_colors: bool,
    pub max_pairings_per_round: u32,
}

impl Default for PairingConfig {
    fn default() -> Self {
        Self {
            max_elo_difference: 200,
            avoid_recent_opponents: true,
            balance_colors: true,
            max_pairings_per_round: 1000,
        }
    }
}

impl TournamentPlayer {
    pub fn new(id: Uuid, elo: u32) -> Self {
        Self {
            id,
            elo,
            joined_at: Utc::now(),
            recent_opponents: Vec::new(),
        }
    }

    pub fn with_opponents(mut self, opponents: Vec<Uuid>) -> Self {
        self.recent_opponents = opponents;
        self
    }

    pub fn has_played_against(&self, opponent_id: &Uuid) -> bool {
        self.recent_opponents.contains(opponent_id)
    }

    pub fn add_opponent(&mut self, opponent_id: Uuid) {
        self.recent_opponents.push(opponent_id);
        // Keep only last 10 opponents
        if self.recent_opponents.len() > 10 {
            self.recent_opponents.remove(0);
        }
    }
}

impl Pairing {
    pub fn new(player1: TournamentPlayer, player2: TournamentPlayer) -> Self {
        Self { player1, player2 }
    }

    pub fn get_elo_difference(&self) -> i32 {
        self.player1.elo as i32 - self.player2.elo as i32
    }

    pub fn contains_player(&self, player_id: &Uuid) -> bool {
        self.player1.id == *player_id || self.player2.id == *player_id
    }

    pub fn get_opponent(&self, player_id: &Uuid) -> Option<&TournamentPlayer> {
        if self.player1.id == *player_id {
            Some(&self.player2)
        } else if self.player2.id == *player_id {
            Some(&self.player1)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tournament_player_creation() {
        let id = Uuid::new_v4();
        let player = TournamentPlayer::new(id, 1500);
        
        assert_eq!(player.id, id);
        assert_eq!(player.elo, 1500);
        assert!(player.recent_opponents.is_empty());
    }

    #[test]
    fn test_tournament_player_opponents() {
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let mut player = TournamentPlayer::new(id1, 1500);
        
        assert!(!player.has_played_against(&id2));
        
        player.add_opponent(id2);
        assert!(player.has_played_against(&id2));
    }

    #[test]
    fn test_pairing_creation() {
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let player1 = TournamentPlayer::new(id1, 1500);
        let player2 = TournamentPlayer::new(id2, 1600);
        
        let pairing = Pairing::new(player1.clone(), player2.clone());
        
        assert!(pairing.contains_player(&id1));
        assert!(pairing.contains_player(&id2));
        assert_eq!(pairing.get_elo_difference(), -100);
        assert_eq!(pairing.get_opponent(&id1).unwrap().id, id2);
        assert_eq!(pairing.get_opponent(&id2).unwrap().id, id1);
    }

    #[test]
    fn test_pairing_config_default() {
        let config = PairingConfig::default();
        
        assert_eq!(config.max_elo_difference, 200);
        assert!(config.avoid_recent_opponents);
        assert!(config.balance_colors);
        assert_eq!(config.max_pairings_per_round, 1000);
    }
}
