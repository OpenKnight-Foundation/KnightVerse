use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TournamentPlayer {
    pub id: Uuid,
    pub elo: u32,
    pub joined_at: DateTime<Utc>,
    // Track previous opponents to avoid repeats if possible
    pub recent_opponents: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pairing {
    pub player1: TournamentPlayer,
    pub player2: TournamentPlayer,
}

pub trait PairingStrategy {
    /// Find pairings within a pool of available players.
    /// Returns a list of pairings and the remaining players who couldn't be paired.
    fn pair(&self, players: Vec<TournamentPlayer>) -> (Vec<Pairing>, Vec<TournamentPlayer>);
}
