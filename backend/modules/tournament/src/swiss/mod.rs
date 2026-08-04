use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

pub mod pairer;
#[cfg(test)]
mod tests;

pub use pairer::{PairingError, SwissPairer};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Player {
    pub id: Uuid,
    pub name: String,
    pub rating: i32,
    pub score: f32,
    pub color_history: Vec<Color>,
    pub opponents: Vec<Uuid>,
    pub is_active: bool,
    pub float_score: i32, // Tracks up/down floating: positive = up, negative = down
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Color {
    White,
    Black,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pairing {
    pub white_player: Uuid,
    pub black_player: Uuid,
    pub round: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TournamentState {
    pub players: HashMap<Uuid, Player>,
    pub current_round: u32,
    pub pairings: Vec<Pairing>,
    pub completed_rounds: u32,
    pub total_rounds: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PairingResult {
    Paired(Pairing),
    Bye(Uuid),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwissConfig {
    pub total_rounds: u32,
    pub rating_importance: f32, // Weight for rating in tie-breaking
    pub color_balance_weight: f32,
}

impl Default for SwissConfig {
    fn default() -> Self {
        Self {
            total_rounds: 5,
            rating_importance: 0.1,
            color_balance_weight: 0.2,
        }
    }
}

impl Player {
    pub fn new(id: Uuid, name: String, rating: i32) -> Self {
        Self {
            id,
            name,
            rating,
            score: 0.0,
            color_history: Vec::new(),
            opponents: Vec::new(),
            is_active: true,
            float_score: 0,
        }
    }

    pub fn add_game_result(&mut self, opponent: Uuid, color: Color, result: GameResult) {
        self.opponents.push(opponent);
        self.color_history.push(color);

        match result {
            GameResult::Win => self.score += 1.0,
            GameResult::Draw => self.score += 0.5,
            GameResult::Loss => self.score += 0.0,
        }
    }

    pub fn has_played_against(&self, opponent_id: &Uuid) -> bool {
        self.opponents.contains(opponent_id)
    }

    pub fn get_color_balance(&self) -> i32 {
        let white_count = self
            .color_history
            .iter()
            .filter(|&&c| c == Color::White)
            .count() as i32;
        let black_count = self
            .color_history
            .iter()
            .filter(|&&c| c == Color::Black)
            .count() as i32;
        white_count - black_count
    }

    pub fn should_prefer_white(&self) -> bool {
        self.get_color_balance() < 0
    }

    pub fn can_be_paired_with(&self, other: &Player) -> bool {
        self.id != other.id && !self.has_played_against(&other.id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GameResult {
    Win,
    Draw,
    Loss,
}

impl TournamentState {
    pub fn new(players: Vec<Player>, total_rounds: u32) -> Self {
        let player_map: HashMap<Uuid, Player> = players.into_iter().map(|p| (p.id, p)).collect();

        Self {
            players: player_map,
            current_round: 1,
            pairings: Vec::new(),
            completed_rounds: 0,
            total_rounds,
        }
    }

    pub fn get_active_players(&self) -> Vec<&Player> {
        self.players.values().filter(|p| p.is_active).collect()
    }

    pub fn get_players_sorted_by_score_then_rating(&self) -> Vec<&Player> {
        let mut players: Vec<&Player> = self.get_active_players();
        players.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(b.rating.cmp(&a.rating))
        });
        players
    }

    pub fn apply_round_results(&mut self, results: Vec<(Uuid, GameResult)>) {
        for (player_id, result) in results {
            if let Some(player) = self.players.get_mut(&player_id) {
                // Find the opponent and color from current round pairings
                if let Some(pairing) = self.pairings.iter().find(|p| {
                    p.round == self.current_round
                        && (p.white_player == player_id || p.black_player == player_id)
                }) {
                    let opponent_id = if pairing.white_player == player_id {
                        pairing.black_player
                    } else {
                        pairing.white_player
                    };
                    let color = if pairing.white_player == player_id {
                        Color::White
                    } else {
                        Color::Black
                    };
                    player.add_game_result(opponent_id, color, result);
                }
            }
        }

        self.completed_rounds += 1;
        self.current_round += 1;
    }

    pub fn is_complete(&self) -> bool {
        self.completed_rounds >= self.total_rounds
    }
}
