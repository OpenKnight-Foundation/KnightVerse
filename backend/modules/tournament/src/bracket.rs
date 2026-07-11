use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use crate::swiss::{Player, GameResult, Color};
use crate::pairing::{Pairing, TournamentPlayer};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TournamentFormat {
    Swiss,
    RoundRobin,
    SingleElimination,
    DoubleElimination,
    Arena,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TournamentStatus {
    Registration,
    Pairing,
    InProgress,
    Completed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tournament {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub format: TournamentFormat,
    pub status: TournamentStatus,
    pub created_at: DateTime<Utc>,
    pub starts_at: Option<DateTime<Utc>>,
    pub ends_at: Option<DateTime<Utc>>,
    pub current_round: u32,
    pub total_rounds: u32,
    pub time_control: String,
    pub increment: Option<u32>,
    pub rated: bool,
    pub entry_fee: Option<u64>,
    pub prize_pool: Option<u64>,
    pub max_players: Option<u32>,
    pub min_players: u32,
    pub pairings: Vec<BracketPairing>,
    pub participants: HashMap<Uuid, TournamentParticipant>,
    pub standings: Vec<TournamentStanding>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TournamentParticipant {
    pub player_id: Uuid,
    pub name: String,
    pub rating: i32,
    pub joined_at: DateTime<Utc>,
    pub is_active: bool,
    pub score: f32,
    pub tie_break_score: f32,
    pub games_played: u32,
    pub wins: u32,
    pub draws: u32,
    pub losses: u32,
    pub byes: u32,
    pub color_balance: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BracketPairing {
    pub id: Uuid,
    pub round: u32,
    pub board: u32,
    pub white_player: Uuid,
    pub black_player: Uuid,
    pub result: Option<GameResult>,
    pub game_id: Option<Uuid>,
    pub scheduled_at: Option<DateTime<Utc>>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TournamentStanding {
    pub rank: u32,
    pub player_id: Uuid,
    pub name: String,
    pub score: f32,
    pub tie_break_score: f32,
    pub games_played: u32,
    pub wins: u32,
    pub draws: u32,
    pub losses: u32,
    pub performance_rating: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BracketConfig {
    pub tournament_id: Uuid,
    pub format: TournamentFormat,
    pub total_rounds: u32,
    pub pairings_per_round: u32,
    pub auto_pair: bool,
    pub pair_immediately: bool,
    pub allow_byes: bool,
    pub color_alternation: bool,
    pub rating_sort: bool,
}

impl Default for BracketConfig {
    fn default() -> Self {
        Self {
            tournament_id: Uuid::new_v4(),
            format: TournamentFormat::Swiss,
            total_rounds: 5,
            pairings_per_round: 0, // Auto-calculate based on participants
            auto_pair: true,
            pair_immediately: false,
            allow_byes: true,
            color_alternation: true,
            rating_sort: true,
        }
    }
}

impl Tournament {
    pub fn new(
        name: String,
        format: TournamentFormat,
        config: BracketConfig,
        time_control: String,
    ) -> Self {
        let id = Uuid::new_v4();
        Self {
            id: config.tournament_id,
            name,
            description: None,
            format,
            status: TournamentStatus::Registration,
            created_at: Utc::now(),
            starts_at: None,
            ends_at: None,
            current_round: 0,
            total_rounds: config.total_rounds,
            time_control,
            increment: None,
            rated: true,
            entry_fee: None,
            prize_pool: None,
            max_players: None,
            min_players: 2,
            pairings: Vec::new(),
            participants: HashMap::new(),
            standings: Vec::new(),
        }
    }

    pub fn add_participant(&mut self, player_id: Uuid, name: String, rating: i32) -> Result<(), String> {
        if self.status != TournamentStatus::Registration {
            return Err("Tournament is not in registration phase".to_string());
        }

        if let Some(max) = self.max_players {
            if self.participants.len() >= max as usize {
                return Err("Tournament is full".to_string());
            }
        }

        if self.participants.contains_key(&player_id) {
            return Err("Player already registered".to_string());
        }

        let participant = TournamentParticipant {
            player_id,
            name,
            rating,
            joined_at: Utc::now(),
            is_active: true,
            score: 0.0,
            tie_break_score: 0.0,
            games_played: 0,
            wins: 0,
            draws: 0,
            losses: 0,
            byes: 0,
            color_balance: 0,
        };

        self.participants.insert(player_id, participant);
        Ok(())
    }

    pub fn remove_participant(&mut self, player_id: Uuid) -> Result<(), String> {
        if self.status != TournamentStatus::Registration {
            return Err("Cannot remove participants after tournament has started".to_string());
        }

        self.participants.remove(&player_id).ok_or("Player not found")?;
        Ok(())
    }

    pub fn start_tournament(&mut self) -> Result<(), String> {
        if self.status != TournamentStatus::Registration {
            return Err("Tournament is already started or completed".to_string());
        }

        if self.participants.len() < self.min_players as usize {
            return Err(format!("Need at least {} participants", self.min_players));
        }

        self.status = TournamentStatus::InProgress;
        self.starts_at = Some(Utc::now());
        self.current_round = 1;

        // Generate initial pairings
        self.generate_pairings()?;

        Ok(())
    }

    pub fn generate_pairings(&mut self) -> Result<(), String> {
        if self.status != TournamentStatus::InProgress {
            return Err("Tournament must be in progress to generate pairings".to_string());
        }

        let active_participants: Vec<_> = self.participants
            .values()
            .filter(|p| p.is_active)
            .collect();

        match self.format {
            TournamentFormat::Swiss => self.generate_swiss_pairings(&active_participants),
            TournamentFormat::RoundRobin => self.generate_round_robin_pairings(&active_participants),
            TournamentFormat::Arena => self.generate_arena_pairings(&active_participants),
            TournamentFormat::SingleElimination => self.generate_elimination_pairings(&active_participants, false),
            TournamentFormat::DoubleElimination => self.generate_elimination_pairings(&active_participants, true),
        }
    }

    fn generate_swiss_pairings(&mut self, participants: &[&TournamentParticipant]) -> Result<(), String> {
        let mut pairings = Vec::new();
        let mut paired_players = std::collections::HashSet::new();

        // Sort participants by score (descending), then rating (descending)
        let mut sorted_participants: Vec<_> = participants.iter().collect();
        sorted_participants.sort_by(|a, b| {
            b.score.partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(b.rating.cmp(&a.rating))
        });

        for (i, &player_a) in sorted_participants.iter().enumerate() {
            if paired_players.contains(&player_a.player_id) {
                continue;
            }

            let mut opponent = None;
            
            // Find suitable opponent
            for &player_b in sorted_participants.iter().skip(i + 1) {
                if paired_players.contains(&player_b.player_id) {
                    continue;
                }

                // Check if they haven't played each other
                if !self.have_played_together(player_a.player_id, player_b.player_id) {
                    opponent = Some(player_b);
                    break;
                }
            }

            if let Some(opponent) = opponent {
                // Determine colors based on balance
                let (white, black) = if player_a.color_balance <= opponent.color_balance {
                    (player_a, opponent)
                } else {
                    (opponent, player_a)
                };

                let pairing = BracketPairing {
                    id: Uuid::new_v4(),
                    round: self.current_round,
                    board: (pairings.len() + 1) as u32,
                    white_player: white.player_id,
                    black_player: black.player_id,
                    result: None,
                    game_id: None,
                    scheduled_at: Some(Utc::now()),
                    started_at: None,
                    completed_at: None,
                };

                pairings.push(pairing);
                paired_players.insert(player_a.player_id);
                paired_players.insert(opponent.player_id);
            } else if participants.len() % 2 == 1 && !paired_players.contains(&player_a.player_id) {
                // Give bye
                self.award_bye(player_a.player_id)?;
                paired_players.insert(player_a.player_id);
            }
        }

        self.pairings.extend(pairings);
        Ok(())
    }

    fn generate_arena_pairings(&mut self, participants: &[&TournamentParticipant]) -> Result<(), String> {
        // Use arena pairing strategy
        let arena_players: Vec<TournamentPlayer> = participants.iter().map(|p| {
            TournamentPlayer {
                id: p.player_id,
                elo: p.rating as u32,
                joined_at: p.joined_at,
                recent_opponents: self.get_recent_opponents(p.player_id),
            }
        }).collect();

        let strategy = crate::arena::ArenaPairingStrategy::new();
        let (pairings, remaining) = strategy.pair(arena_players);

        for (i, pairing) in pairings.into_iter().enumerate() {
            let bracket_pairing = BracketPairing {
                id: Uuid::new_v4(),
                round: self.current_round,
                board: (i + 1) as u32,
                white_player: pairing.player1.id,
                black_player: pairing.player2.id,
                result: None,
                game_id: None,
                scheduled_at: Some(Utc::now()),
                started_at: None,
                completed_at: None,
            };
            self.pairings.push(bracket_pairing);
        }

        // Handle remaining players (give byes or wait for next round)
        for player in remaining {
            self.award_bye(player.id)?;
        }

        Ok(())
    }

    fn generate_round_robin_pairings(&mut self, participants: &[&TournamentParticipant]) -> Result<(), String> {
        // Simple round-robin pairing
        let mut pairings = Vec::new();
        let participants_vec: Vec<_> = participants.iter().collect();

        for i in 0..participants_vec.len() {
            for j in (i + 1)..participants_vec.len() {
                let round = ((i + j) % participants_vec.len()) as u32 + 1;
                
                // Alternate colors
                let (white, black) = if (i + j) % 2 == 0 {
                    (participants_vec[i], participants_vec[j])
                } else {
                    (participants_vec[j], participants_vec[i])
                };

                let pairing = BracketPairing {
                    id: Uuid::new_v4(),
                    round,
                    board: (pairings.len() + 1) as u32,
                    white_player: white.player_id,
                    black_player: black.player_id,
                    result: None,
                    game_id: None,
                    scheduled_at: None, // Will be scheduled when round starts
                    started_at: None,
                    completed_at: None,
                };

                pairings.push(pairing);
            }
        }

        self.pairings.extend(pairings);
        Ok(())
    }

    fn generate_elimination_pairings(&mut self, participants: &[&TournamentParticipant], double_elim: bool) -> Result<(), String> {
        let mut pairings = Vec::new();
        let mut participants_vec: Vec<_> = participants.iter().collect();
        
        // Sort by rating for seeding
        participants_vec.sort_by(|a, b| b.rating.cmp(&a.rating));

        // Pair highest with lowest, second highest with second lowest, etc.
        while participants_vec.len() >= 2 {
            let player1 = participants_vec.remove(0);
            let player2 = participants_vec.pop().unwrap();

            let pairing = BracketPairing {
                id: Uuid::new_v4(),
                round: self.current_round,
                board: (pairings.len() + 1) as u32,
                white_player: player1.player_id,
                black_player: player2.player_id,
                result: None,
                game_id: None,
                scheduled_at: Some(Utc::now()),
                started_at: None,
                completed_at: None,
            };

            pairings.push(pairing);
        }

        // Handle bye if odd number
        if let Some(player) = participants_vec.pop() {
            self.award_bye(player.player_id)?;
        }

        self.pairings.extend(pairings);
        Ok(())
    }

    pub fn submit_game_result(&mut self, pairing_id: Uuid, result: GameResult) -> Result<(), String> {
        let pairing = self.pairings.iter_mut()
            .find(|p| p.id == pairing_id)
            .ok_or("Pairing not found")?;

        if pairing.result.is_some() {
            return Err("Game result already submitted".to_string());
        }

        pairing.result = Some(result);
        pairing.completed_at = Some(Utc::now());

        // Update participant statistics
        let white_participant = self.participants.get_mut(&pairing.white_player)
            .ok_or("White participant not found")?;
        let black_participant = self.participants.get_mut(&pairing.black_player)
            .ok_or("Black participant not found")?;

        // Update scores and statistics
        match result {
            GameResult::Win => {
                white_participant.score += 1.0;
                white_participant.wins += 1;
                black_participant.losses += 1;
            }
            GameResult::Draw => {
                white_participant.score += 0.5;
                black_participant.score += 0.5;
                white_participant.draws += 1;
                black_participant.draws += 1;
            }
            GameResult::Loss => {
                black_participant.score += 1.0;
                black_participant.wins += 1;
                white_participant.losses += 1;
            }
        }

        white_participant.games_played += 1;
        black_participant.games_played += 1;
        white_participant.color_balance += 1;
        black_participant.color_balance -= 1;

        // Update standings
        self.update_standings();

        // Check if round is complete
        self.check_round_completion();

        Ok(())
    }

    fn award_bye(&mut self, player_id: Uuid) -> Result<(), String> {
        let participant = self.participants.get_mut(&player_id)
            .ok_or("Participant not found")?;

        participant.score += 1.0;
        participant.byes += 1;
        participant.games_played += 1;

        Ok(())
    }

    fn have_played_together(&self, player_a: Uuid, player_b: Uuid) -> bool {
        self.pairings.iter().any(|p| {
            (p.white_player == player_a && p.black_player == player_b) ||
            (p.white_player == player_b && p.black_player == player_a)
        })
    }

    fn get_recent_opponents(&self, player_id: Uuid) -> Vec<Uuid> {
        self.pairings.iter()
            .filter(|p| p.white_player == player_id || p.black_player == player_id)
            .map(|p| {
                if p.white_player == player_id {
                    p.black_player
                } else {
                    p.white_player
                }
            })
            .rev()
            .take(5) // Recent opponents
            .collect()
    }

    fn update_standings(&mut self) {
        let mut standings: Vec<_> = self.participants.values()
            .filter(|p| p.is_active)
            .collect();

        // Sort by score (descending), then tie-break score
        standings.sort_by(|a, b| {
            b.score.partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(b.tie_break_score.partial_cmp(&a.tie_break_score)
                    .unwrap_or(std::cmp::Ordering::Equal))
        });

        self.standings = standings.into_iter().enumerate().map(|(i, p)| {
            TournamentStanding {
                rank: (i + 1) as u32,
                player_id: p.player_id,
                name: p.name.clone(),
                score: p.score,
                tie_break_score: p.tie_break_score,
                games_played: p.games_played,
                wins: p.wins,
                draws: p.draws,
                losses: p.losses,
                performance_rating: None, // Calculate if needed
            }
        }).collect();
    }

    fn check_round_completion(&mut self) {
        let current_round_pairings: Vec<_> = self.pairings.iter()
            .filter(|p| p.round == self.current_round)
            .collect();

        let all_completed = current_round_pairings.iter().all(|p| p.result.is_some());

        if all_completed {
            self.current_round += 1;

            // Check if tournament is complete
            if self.current_round > self.total_rounds {
                self.status = TournamentStatus::Completed;
                self.ends_at = Some(Utc::now());
            } else if self.auto_pair_next_round() {
                let _ = self.generate_pairings();
            }
        }
    }

    fn auto_pair_next_round(&self) -> bool {
        // Logic to determine if next round should be auto-paired
        // For now, always auto-pair
        true
    }

    pub fn get_current_pairings(&self) -> Vec<&BracketPairing> {
        self.pairings.iter()
            .filter(|p| p.round == self.current_round && p.result.is_none())
            .collect()
    }

    pub fn get_player_pairings(&self, player_id: Uuid) -> Vec<&BracketPairing> {
        self.pairings.iter()
            .filter(|p| p.white_player == player_id || p.black_player == player_id)
            .collect()
    }

    pub fn get_tournament_stats(&self) -> TournamentStats {
        TournamentStats {
            total_participants: self.participants.len() as u32,
            active_participants: self.participants.values().filter(|p| p.is_active).count() as u32,
            total_games: self.pairings.len() as u32,
            completed_games: self.pairings.iter().filter(|p| p.result.is_some()).count() as u32,
            current_round: self.current_round,
            total_rounds: self.total_rounds,
            average_rating: if !self.participants.is_empty() {
                self.participants.values().map(|p| p.rating as f64).sum::<f64>() / self.participants.len() as f64
            } else {
                0.0
            } as i32,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TournamentStats {
    pub total_participants: u32,
    pub active_participants: u32,
    pub total_games: u32,
    pub completed_games: u32,
    pub current_round: u32,
    pub total_rounds: u32,
    pub average_rating: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tournament_creation() {
        let config = BracketConfig::default();
        let tournament = Tournament::new(
            "Test Tournament".to_string(),
            TournamentFormat::Swiss,
            config,
            "5+0".to_string(),
        );

        assert_eq!(tournament.name, "Test Tournament");
        assert_eq!(tournament.status, TournamentStatus::Registration);
        assert_eq!(tournament.current_round, 0);
    }

    #[test]
    fn test_add_participant() {
        let config = BracketConfig::default();
        let mut tournament = Tournament::new(
            "Test Tournament".to_string(),
            TournamentFormat::Swiss,
            config,
            "5+0".to_string(),
        );

        let player_id = Uuid::new_v4();
        tournament.add_participant(player_id, "Alice".to_string(), 1500).unwrap();

        assert_eq!(tournament.participants.len(), 1);
        assert!(tournament.participants.contains_key(&player_id));
    }

    #[test]
    fn test_start_tournament() {
        let config = BracketConfig::default();
        let mut tournament = Tournament::new(
            "Test Tournament".to_string(),
            TournamentFormat::Swiss,
            config,
            "5+0".to_string(),
        );

        let player1 = Uuid::new_v4();
        let player2 = Uuid::new_v4();
        
        tournament.add_participant(player1, "Alice".to_string(), 1500).unwrap();
        tournament.add_participant(player2, "Bob".to_string(), 1600).unwrap();

        tournament.start_tournament().unwrap();

        assert_eq!(tournament.status, TournamentStatus::InProgress);
        assert_eq!(tournament.current_round, 1);
        assert!(tournament.starts_at.is_some());
    }
}
