use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BracketFormat {
    SingleElimination,
    DoubleElimination,
    RoundRobin,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MatchStatus {
    Pending,
    InProgress,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TournamentStatus {
    Registration,
    InProgress,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TournamentParticipant {
    pub id: Uuid,
    pub wallet_address: String,
    pub display_name: String,
    pub elo: u32,
    pub seed: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BracketMatch {
    pub id: Uuid,
    pub round: u32,
    pub match_number: u32,
    pub player1_id: Option<Uuid>,
    pub player2_id: Option<Uuid>,
    pub winner_id: Option<Uuid>,
    pub status: MatchStatus,
    pub scheduled_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TournamentBracket {
    pub id: Uuid,
    pub name: String,
    pub format: BracketFormat,
    pub status: TournamentStatus,
    pub participants: Vec<TournamentParticipant>,
    pub matches: Vec<BracketMatch>,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub total_rounds: u32,
    pub winner_id: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BracketError {
    NotEnoughPlayers,
    TournamentAlreadyStarted,
    TournamentNotStarted,
    MatchNotFound,
    PlayerNotInMatch,
    MatchAlreadyCompleted,
}

impl std::fmt::Display for BracketError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotEnoughPlayers => write!(f, "At least 2 players are required to start a tournament"),
            Self::TournamentAlreadyStarted => write!(f, "Tournament has already started"),
            Self::TournamentNotStarted => write!(f, "Tournament has not started yet"),
            Self::MatchNotFound => write!(f, "Match not found"),
            Self::PlayerNotInMatch => write!(f, "Player is not a participant in this match"),
            Self::MatchAlreadyCompleted => write!(f, "Match result has already been recorded"),
        }
    }
}

impl std::error::Error for BracketError {}

pub struct BracketService;

impl BracketService {
    /// Create a new bracket for the given participants.
    ///
    /// Participants are seeded by ELO (highest ELO = seed 1). For elimination
    /// formats the bracket size is rounded up to the next power of two; extra
    /// slots become automatic byes for the top seeds.
    pub fn create_bracket(
        tournament_id: Uuid,
        name: &str,
        mut participants: Vec<TournamentParticipant>,
        format: BracketFormat,
    ) -> Result<TournamentBracket, BracketError> {
        if participants.len() < 2 {
            return Err(BracketError::NotEnoughPlayers);
        }

        // Seed by ELO descending so highest-rated player is seed 1
        participants.sort_by(|a, b| b.elo.cmp(&a.elo));
        for (i, p) in participants.iter_mut().enumerate() {
            p.seed = (i + 1) as u32;
        }

        let total_rounds = match &format {
            BracketFormat::SingleElimination | BracketFormat::DoubleElimination => {
                let size = next_power_of_two(participants.len() as u32);
                (size as f64).log2() as u32
            }
            BracketFormat::RoundRobin => (participants.len() as u32).saturating_sub(1),
        };

        let matches = match &format {
            BracketFormat::SingleElimination | BracketFormat::DoubleElimination => {
                generate_single_elimination_matches(&participants, total_rounds)
            }
            BracketFormat::RoundRobin => generate_round_robin_matches(&participants),
        };

        Ok(TournamentBracket {
            id: tournament_id,
            name: name.to_string(),
            format,
            status: TournamentStatus::Registration,
            participants,
            matches,
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            total_rounds,
            winner_id: None,
        })
    }

    /// Transition the bracket from Registration to InProgress.
    pub fn start_tournament(bracket: &mut TournamentBracket) -> Result<(), BracketError> {
        if bracket.status != TournamentStatus::Registration {
            return Err(BracketError::TournamentAlreadyStarted);
        }
        bracket.status = TournamentStatus::InProgress;
        bracket.started_at = Some(Utc::now());
        Ok(())
    }

    /// Record the winner of a match and advance them to the next round.
    pub fn record_result(
        bracket: &mut TournamentBracket,
        match_id: Uuid,
        winner_id: Uuid,
    ) -> Result<(), BracketError> {
        let (match_round, match_number) = {
            let m = bracket
                .matches
                .iter()
                .find(|m| m.id == match_id)
                .ok_or(BracketError::MatchNotFound)?;

            if m.status == MatchStatus::Completed {
                return Err(BracketError::MatchAlreadyCompleted);
            }
            if bracket.status != TournamentStatus::InProgress {
                return Err(BracketError::TournamentNotStarted);
            }
            if m.player1_id != Some(winner_id) && m.player2_id != Some(winner_id) {
                return Err(BracketError::PlayerNotInMatch);
            }
            (m.round, m.match_number)
        };

        {
            let m = bracket.matches.iter_mut().find(|m| m.id == match_id).unwrap();
            m.winner_id = Some(winner_id);
            m.status = MatchStatus::Completed;
            m.completed_at = Some(Utc::now());
        }

        match &bracket.format {
            BracketFormat::SingleElimination | BracketFormat::DoubleElimination => {
                let next_round = match_round + 1;
                // Odd match_number feeds player1 slot; even feeds player2 slot
                let next_match_number = (match_number + 1) / 2;

                if let Some(next) = bracket
                    .matches
                    .iter_mut()
                    .find(|m| m.round == next_round && m.match_number == next_match_number)
                {
                    if match_number % 2 == 1 {
                        next.player1_id = Some(winner_id);
                    } else {
                        next.player2_id = Some(winner_id);
                    }
                } else {
                    // No next match — this winner is the champion
                    bracket.winner_id = Some(winner_id);
                    bracket.status = TournamentStatus::Completed;
                    bracket.completed_at = Some(Utc::now());
                }
            }
            BracketFormat::RoundRobin => {
                if bracket.matches.iter().all(|m| m.status == MatchStatus::Completed) {
                    bracket.winner_id = determine_round_robin_winner(bracket);
                    bracket.status = TournamentStatus::Completed;
                    bracket.completed_at = Some(Utc::now());
                }
            }
        }

        Ok(())
    }

    /// Returns `(participant_id, wins)` sorted by wins descending.
    pub fn get_standings(bracket: &TournamentBracket) -> Vec<(Uuid, u32)> {
        let mut wins: HashMap<Uuid, u32> = HashMap::new();
        for m in &bracket.matches {
            if let Some(w) = m.winner_id {
                *wins.entry(w).or_insert(0) += 1;
            }
        }
        let mut standings: Vec<(Uuid, u32)> = wins.into_iter().collect();
        standings.sort_by(|a, b| b.1.cmp(&a.1));
        standings
    }
}

fn next_power_of_two(n: u32) -> u32 {
    if n.is_power_of_two() { n } else { n.next_power_of_two() }
}

fn generate_single_elimination_matches(
    participants: &[TournamentParticipant],
    total_rounds: u32,
) -> Vec<BracketMatch> {
    let bracket_size = next_power_of_two(participants.len() as u32) as usize;
    let mut matches = Vec::new();

    // Round 1: standard seeding (1 vs N, 2 vs N-1, ...)
    let r1_count = bracket_size / 2;
    for i in 0..r1_count {
        let p1 = participants.get(i).map(|p| p.id);
        let p2 = participants.get(bracket_size - 1 - i).map(|p| p.id);

        let (p1_id, p2_id, status, winner_id, completed_at) = match (p1, p2) {
            // Bye: top seed advances automatically
            (Some(id), None) => (Some(id), None, MatchStatus::Completed, Some(id), Some(Utc::now())),
            (Some(id1), Some(id2)) => (Some(id1), Some(id2), MatchStatus::Pending, None, None),
            _ => (None, None, MatchStatus::Pending, None, None),
        };

        matches.push(BracketMatch {
            id: Uuid::new_v4(),
            round: 1,
            match_number: (i + 1) as u32,
            player1_id: p1_id,
            player2_id: p2_id,
            winner_id,
            status,
            scheduled_at: None,
            completed_at,
        });
    }

    // Subsequent rounds: slots are filled as winners advance
    for round in 2..=total_rounds {
        let count = bracket_size / 2_usize.pow(round);
        for i in 0..count {
            matches.push(BracketMatch {
                id: Uuid::new_v4(),
                round,
                match_number: (i + 1) as u32,
                player1_id: None,
                player2_id: None,
                winner_id: None,
                status: MatchStatus::Pending,
                scheduled_at: None,
                completed_at: None,
            });
        }
    }

    matches
}

fn generate_round_robin_matches(participants: &[TournamentParticipant]) -> Vec<BracketMatch> {
    let n = participants.len();
    let mut matches = Vec::new();
    let matches_per_round = n / 2;
    let mut match_number: u32 = 1;
    let mut round: u32 = 1;

    for i in 0..n {
        for j in (i + 1)..n {
            matches.push(BracketMatch {
                id: Uuid::new_v4(),
                round,
                match_number,
                player1_id: Some(participants[i].id),
                player2_id: Some(participants[j].id),
                winner_id: None,
                status: MatchStatus::Pending,
                scheduled_at: None,
                completed_at: None,
            });
            match_number += 1;
            if match_number > matches_per_round as u32 {
                round += 1;
                match_number = 1;
            }
        }
    }

    matches
}

fn determine_round_robin_winner(bracket: &TournamentBracket) -> Option<Uuid> {
    let mut wins: HashMap<Uuid, u32> = HashMap::new();
    for m in &bracket.matches {
        if let Some(w) = m.winner_id {
            *wins.entry(w).or_insert(0) += 1;
        }
    }
    wins.into_iter().max_by_key(|(_, w)| *w).map(|(id, _)| id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_participants(n: usize) -> Vec<TournamentParticipant> {
        (0..n)
            .map(|i| TournamentParticipant {
                id: Uuid::new_v4(),
                wallet_address: format!("G{:055}", i),
                display_name: format!("Player {}", i + 1),
                elo: 1500 + i as u32 * 50,
                seed: 0,
            })
            .collect()
    }

    #[test]
    fn create_bracket_requires_at_least_two_players() {
        let err = BracketService::create_bracket(
            Uuid::new_v4(),
            "Test",
            make_participants(1),
            BracketFormat::SingleElimination,
        )
        .unwrap_err();
        assert_eq!(err, BracketError::NotEnoughPlayers);
    }

    #[test]
    fn single_elimination_four_players_has_correct_structure() {
        let bracket = BracketService::create_bracket(
            Uuid::new_v4(),
            "Test",
            make_participants(4),
            BracketFormat::SingleElimination,
        )
        .unwrap();

        assert_eq!(bracket.total_rounds, 2);
        assert_eq!(bracket.matches.iter().filter(|m| m.round == 1).count(), 2);
        assert_eq!(bracket.matches.iter().filter(|m| m.round == 2).count(), 1);
    }

    #[test]
    fn participants_seeded_by_elo_descending() {
        let bracket = BracketService::create_bracket(
            Uuid::new_v4(),
            "Test",
            make_participants(4),
            BracketFormat::SingleElimination,
        )
        .unwrap();

        assert_eq!(bracket.participants[0].seed, 1);
        for i in 1..bracket.participants.len() {
            assert!(bracket.participants[i - 1].elo >= bracket.participants[i].elo);
        }
    }

    #[test]
    fn two_player_bracket_completes_on_single_result() {
        let mut bracket = BracketService::create_bracket(
            Uuid::new_v4(),
            "Test",
            make_participants(2),
            BracketFormat::SingleElimination,
        )
        .unwrap();

        BracketService::start_tournament(&mut bracket).unwrap();
        let m = bracket.matches.iter().find(|m| m.round == 1).unwrap().clone();
        let winner = m.player1_id.unwrap();

        BracketService::record_result(&mut bracket, m.id, winner).unwrap();

        assert_eq!(bracket.status, TournamentStatus::Completed);
        assert_eq!(bracket.winner_id, Some(winner));
    }

    #[test]
    fn four_player_bracket_requires_two_rounds_to_complete() {
        let mut bracket = BracketService::create_bracket(
            Uuid::new_v4(),
            "Test",
            make_participants(4),
            BracketFormat::SingleElimination,
        )
        .unwrap();

        BracketService::start_tournament(&mut bracket).unwrap();

        let r1: Vec<_> = bracket.matches.iter().filter(|m| m.round == 1).map(|m| m.id).collect();
        for id in r1 {
            let winner = bracket.matches.iter().find(|m| m.id == id).unwrap().player1_id.unwrap();
            BracketService::record_result(&mut bracket, id, winner).unwrap();
        }
        assert_eq!(bracket.status, TournamentStatus::InProgress, "Should still be in progress after round 1");

        let r2: Vec<_> = bracket.matches.iter().filter(|m| m.round == 2).map(|m| m.id).collect();
        for id in r2 {
            let winner = bracket.matches.iter().find(|m| m.id == id).unwrap().player1_id.unwrap();
            BracketService::record_result(&mut bracket, id, winner).unwrap();
        }
        assert_eq!(bracket.status, TournamentStatus::Completed);
    }

    #[test]
    fn cannot_record_result_before_tournament_starts() {
        let mut bracket = BracketService::create_bracket(
            Uuid::new_v4(),
            "Test",
            make_participants(2),
            BracketFormat::SingleElimination,
        )
        .unwrap();

        let m = bracket.matches[0].clone();
        let err = BracketService::record_result(&mut bracket, m.id, m.player1_id.unwrap())
            .unwrap_err();
        assert_eq!(err, BracketError::TournamentNotStarted);
    }

    #[test]
    fn cannot_record_result_twice_for_same_match() {
        let mut bracket = BracketService::create_bracket(
            Uuid::new_v4(),
            "Test",
            make_participants(2),
            BracketFormat::SingleElimination,
        )
        .unwrap();

        BracketService::start_tournament(&mut bracket).unwrap();
        let m = bracket.matches[0].clone();
        let winner = m.player1_id.unwrap();
        BracketService::record_result(&mut bracket, m.id, winner).unwrap();

        let err = BracketService::record_result(&mut bracket, m.id, winner).unwrap_err();
        assert_eq!(err, BracketError::MatchAlreadyCompleted);
    }

    #[test]
    fn player_not_in_match_returns_error() {
        let mut bracket = BracketService::create_bracket(
            Uuid::new_v4(),
            "Test",
            make_participants(2),
            BracketFormat::SingleElimination,
        )
        .unwrap();

        BracketService::start_tournament(&mut bracket).unwrap();
        let m = bracket.matches[0].clone();
        let err = BracketService::record_result(&mut bracket, m.id, Uuid::new_v4()).unwrap_err();
        assert_eq!(err, BracketError::PlayerNotInMatch);
    }

    #[test]
    fn cannot_start_already_started_tournament() {
        let mut bracket = BracketService::create_bracket(
            Uuid::new_v4(),
            "Test",
            make_participants(2),
            BracketFormat::SingleElimination,
        )
        .unwrap();

        BracketService::start_tournament(&mut bracket).unwrap();
        let err = BracketService::start_tournament(&mut bracket).unwrap_err();
        assert_eq!(err, BracketError::TournamentAlreadyStarted);
    }

    #[test]
    fn bye_slots_auto_advance_top_seed_in_odd_bracket() {
        // 3 players → bracket size 4, one bye for seed 1
        let bracket = BracketService::create_bracket(
            Uuid::new_v4(),
            "Test",
            make_participants(3),
            BracketFormat::SingleElimination,
        )
        .unwrap();

        let byes: Vec<_> = bracket
            .matches
            .iter()
            .filter(|m| m.round == 1 && m.status == MatchStatus::Completed)
            .collect();
        assert_eq!(byes.len(), 1, "Exactly one bye for 3-player bracket");
        assert!(byes[0].winner_id.is_some(), "Bye match should have a winner set");
    }

    #[test]
    fn round_robin_four_players_generates_six_matches() {
        let bracket = BracketService::create_bracket(
            Uuid::new_v4(),
            "Test",
            make_participants(4),
            BracketFormat::RoundRobin,
        )
        .unwrap();
        // C(4, 2) = 6
        assert_eq!(bracket.matches.len(), 6);
    }

    #[test]
    fn round_robin_completes_after_all_matches_played() {
        let mut bracket = BracketService::create_bracket(
            Uuid::new_v4(),
            "Test",
            make_participants(3),
            BracketFormat::RoundRobin,
        )
        .unwrap();

        BracketService::start_tournament(&mut bracket).unwrap();

        let ids: Vec<_> = bracket.matches.iter().map(|m| m.id).collect();
        for id in ids {
            let winner = bracket.matches.iter().find(|m| m.id == id).unwrap().player1_id.unwrap();
            BracketService::record_result(&mut bracket, id, winner).unwrap();
        }

        assert_eq!(bracket.status, TournamentStatus::Completed);
        assert!(bracket.winner_id.is_some());
    }

    #[test]
    fn get_standings_returns_sorted_by_wins() {
        let mut bracket = BracketService::create_bracket(
            Uuid::new_v4(),
            "Test",
            make_participants(3),
            BracketFormat::RoundRobin,
        )
        .unwrap();

        BracketService::start_tournament(&mut bracket).unwrap();

        // Player at index 0 (highest ELO) wins all their matches
        let ids: Vec<_> = bracket.matches.iter().map(|m| m.id).collect();
        for id in ids {
            let winner = bracket.matches.iter().find(|m| m.id == id).unwrap().player1_id.unwrap();
            BracketService::record_result(&mut bracket, id, winner).unwrap();
        }

        let standings = BracketService::get_standings(&bracket);
        assert!(!standings.is_empty());
        // First place must have at least as many wins as second
        if standings.len() > 1 {
            assert!(standings[0].1 >= standings[1].1);
        }
    }
}
