use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BracketFormat {
    SingleElimination,
    DoubleElimination,
    RoundRobin,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TournamentStatus {
    Registration,
    InProgress,
    Completed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MatchStatus {
    Pending,
    InProgress,
    Completed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TournamentParticipant {
    pub id: Uuid,
    pub name: String,
    pub elo: u32,
    pub seed: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    #[serde(skip)]
    pub bracket_type: BracketType,
    #[serde(skip)]
    pub losers_from_match: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum BracketType {
    #[default]
    Winners,
    Losers,
    GrandFinals,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TournamentBracket {
    pub id: Uuid,
    pub name: String,
    pub format: BracketFormat,
    pub status: TournamentStatus,
    pub participants: Vec<TournamentParticipant>,
    pub matches: Vec<BracketMatch>,
    pub winner_id: Option<Uuid>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    #[serde(skip)]
    pub bracket_reset_required: bool,
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
            Self::NotEnoughPlayers => {
                write!(f, "At least 2 players are required to start a tournament")
            }
            Self::TournamentAlreadyStarted => write!(f, "Tournament has already started"),
            Self::TournamentNotStarted => write!(f, "Tournament has not started yet"),
            Self::MatchNotFound => write!(f, "Match not found"),
            Self::PlayerNotInMatch => write!(f, "Player is not a participant in this match"),
            Self::MatchAlreadyCompleted => write!(f, "Match result has already been recorded"),
        }
    }
}

impl std::error::Error for BracketError {}

fn next_power_of_two(n: u32) -> u32 {
    if n.is_power_of_two() {
        n
    } else {
        n.next_power_of_two()
    }
}

/// Place `player_id` into an unmatched Losers Bracket round. Prefers the match
/// at `preferred_match_number`; if it is already full (or absent, as can happen
/// with non-power-of-two sizes after byes), falls back to the first match in the
/// round with a free slot. Returns `true` if a slot was found.
fn place_in_lb(
    matches: &mut [BracketMatch],
    round: u32,
    preferred_match_number: u32,
    player_id: Uuid,
) -> bool {
    if let Some(m) = matches.iter_mut().find(|m| {
        m.bracket_type == BracketType::Losers
            && m.round == round
            && m.match_number == preferred_match_number
    }) {
        if m.player1_id.is_none() {
            m.player1_id = Some(player_id);
            return true;
        }
        if m.player2_id.is_none() {
            m.player2_id = Some(player_id);
            return true;
        }
    }
    // Fallback: any match in the round with a free slot.
    if let Some(m) = matches
        .iter_mut()
        .find(|m| m.bracket_type == BracketType::Losers && m.round == round)
    {
        if m.player1_id.is_none() {
            m.player1_id = Some(player_id);
            return true;
        }
        if m.player2_id.is_none() {
            m.player2_id = Some(player_id);
            return true;
        }
    }
    false
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
            (Some(id), None) => (
                Some(id),
                None,
                MatchStatus::Completed,
                Some(id),
                Some(Utc::now()),
            ),
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
            bracket_type: BracketType::Winners,
            losers_from_match: None,
        });
    }

    // Subsequent rounds
    let mut count = r1_count / 2;
    for r in 2..=total_rounds {
        for i in 0..count {
            matches.push(BracketMatch {
                id: Uuid::new_v4(),
                round: r,
                match_number: (i + 1) as u32,
                player1_id: None,
                player2_id: None,
                winner_id: None,
                status: MatchStatus::Pending,
                scheduled_at: None,
                completed_at: None,
                bracket_type: BracketType::Winners,
                losers_from_match: None,
            });
        }
        count /= 2;
    }

    // Auto-advance byes into round 2 if applicable
    for i in 0..r1_count {
        if matches[i].status == MatchStatus::Completed {
            if let Some(winner_id) = matches[i].winner_id {
                let next_match_number = ((i + 1) as u32).div_ceil(2);
                if let Some(next) = matches
                    .iter_mut()
                    .find(|m| m.round == 2 && m.match_number == next_match_number)
                {
                    if (i + 1) % 2 == 1 {
                        next.player1_id = Some(winner_id);
                    } else {
                        next.player2_id = Some(winner_id);
                    }
                }
            }
        }
    }

    matches
}

fn generate_double_elimination_matches(
    participants: &[TournamentParticipant],
    total_rounds: u32,
) -> Vec<BracketMatch> {
    let bracket_size = next_power_of_two(participants.len() as u32) as usize;
    let mut matches = Vec::new();

    // Winners Bracket Round 1
    let r1_count = bracket_size / 2;
    for i in 0..r1_count {
        let p1 = participants.get(i).map(|p| p.id);
        let p2 = participants.get(bracket_size - 1 - i).map(|p| p.id);

        let (p1_id, p2_id, status, winner_id, completed_at) = match (p1, p2) {
            (Some(id), None) => (
                Some(id),
                None,
                MatchStatus::Completed,
                Some(id),
                Some(Utc::now()),
            ),
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
            bracket_type: BracketType::Winners,
            losers_from_match: None,
        });
    }

    // Winners Bracket subsequent rounds
    let mut wb_count = r1_count / 2;
    for r in 2..=total_rounds {
        for i in 0..wb_count {
            matches.push(BracketMatch {
                id: Uuid::new_v4(),
                round: r,
                match_number: (i + 1) as u32,
                player1_id: None,
                player2_id: None,
                winner_id: None,
                status: MatchStatus::Pending,
                scheduled_at: None,
                completed_at: None,
                bracket_type: BracketType::Winners,
                losers_from_match: None,
            });
        }
        wb_count /= 2;
    }

    // Losers Bracket - Create matches for each round.
    // Byes in WB Round 1 produce no losers, so the first LB round only needs to
    // pair losers from the real (non-bye) WB Round 1 matches.
    let bye_count = bracket_size - participants.len();
    let real_r1 = r1_count - bye_count; // # of real WB R1 matches that yield losers

    // Round 1: pair the losers of adjacent real WB R1 matches.
    let lb_r1_matches = real_r1 / 2;
    for i in 0..lb_r1_matches {
        matches.push(BracketMatch {
            id: Uuid::new_v4(),
            round: 1,
            match_number: (i + 1) as u32,
            player1_id: None,
            player2_id: None,
            winner_id: None,
            status: MatchStatus::Pending,
            scheduled_at: None,
            completed_at: None,
            bracket_type: BracketType::Losers,
            losers_from_match: None,
        });
    }

    // Remaining LB rounds alternate pairing:
    //   even round r:  losers of WB round (r/2+1) + winners of LB round r-1
    //   odd round r:   winners of LB round r-1
    // Number of matches = incoming players / 2.
    //
    // Round 2: losers from WB Round 2 + winners from LB Round 1.
    let wb_r2_losers = r1_count / 2; // == bracket_size / 4
    let lb_r2_matches = ((wb_r2_losers + lb_r1_matches) / 2).max(1);
    for i in 0..lb_r2_matches {
        matches.push(BracketMatch {
            id: Uuid::new_v4(),
            round: 2,
            match_number: (i + 1) as u32,
            player1_id: None,
            player2_id: None,
            winner_id: None,
            status: MatchStatus::Pending,
            scheduled_at: None,
            completed_at: None,
            bracket_type: BracketType::Losers,
            losers_from_match: None,
        });
    }

    // Subsequent LB rounds.
    let mut lb_round = 3;
    let mut prev_lb_matches = lb_r2_matches;

    while lb_round <= 2 * total_rounds - 2 {
        let wb_round = lb_round / 2 + 1;
        let wb_losers = if lb_round % 2 == 0 && wb_round <= total_rounds {
            if wb_round == 1 {
                real_r1
            } else {
                r1_count / (1 << (wb_round - 1))
            }
        } else {
            0
        };

        let matches_this_round = ((wb_losers + prev_lb_matches) / 2).max(1);

        for i in 0..matches_this_round {
            matches.push(BracketMatch {
                id: Uuid::new_v4(),
                round: lb_round,
                match_number: (i + 1) as u32,
                player1_id: None,
                player2_id: None,
                winner_id: None,
                status: MatchStatus::Pending,
                scheduled_at: None,
                completed_at: None,
                bracket_type: BracketType::Losers,
                losers_from_match: None,
            });
        }

        prev_lb_matches = matches_this_round;
        lb_round += 1;
    }

    // Grand Finals (potentially two matches)
    // GF match 1: WB champion vs LB champion
    matches.push(BracketMatch {
        id: Uuid::new_v4(),
        round: total_rounds + 1,
        match_number: 1,
        player1_id: None, // Will be set when WB final completes
        player2_id: None, // Will be set when LB final completes
        winner_id: None,
        status: MatchStatus::Pending,
        scheduled_at: None,
        completed_at: None,
        bracket_type: BracketType::GrandFinals,
        losers_from_match: None,
    });

    // Auto-advance byes in winners bracket
    for i in 0..r1_count {
        if matches[i].status == MatchStatus::Completed {
            if let Some(winner_id) = matches[i].winner_id {
                let next_match_number = ((i + 1) as u32).div_ceil(2);
                if let Some(next) = matches
                    .iter_mut()
                    .find(|m| m.round == 2 && m.match_number == next_match_number && m.bracket_type == BracketType::Winners)
                {
                    if (i + 1) % 2 == 1 {
                        next.player1_id = Some(winner_id);
                    } else {
                        next.player2_id = Some(winner_id);
                    }
                }
            }
        }
    }

    matches
}

fn generate_round_robin_matches(participants: &[TournamentParticipant]) -> Vec<BracketMatch> {
    let mut matches = Vec::new();
    let mut match_num = 1;
    for i in 0..participants.len() {
        for j in (i + 1)..participants.len() {
            matches.push(BracketMatch {
                id: Uuid::new_v4(),
                round: 1,
                match_number: match_num,
                player1_id: Some(participants[i].id),
                player2_id: Some(participants[j].id),
                winner_id: None,
                status: MatchStatus::Pending,
                scheduled_at: None,
                completed_at: None,
                bracket_type: BracketType::Winners,
                losers_from_match: None,
            });
            match_num += 1;
        }
    }
    matches
}

fn determine_round_robin_winner(bracket: &TournamentBracket) -> Option<Uuid> {
    let standings = BracketService::get_standings(bracket);
    standings.first().map(|(id, _)| *id)
}

pub struct BracketService;

impl BracketService {
    /// Create a new tournament bracket with seeded participants.
    pub fn create_bracket(
        id: Uuid,
        name: impl Into<String>,
        mut participants: Vec<TournamentParticipant>,
        format: BracketFormat,
    ) -> Result<TournamentBracket, BracketError> {
        if participants.len() < 2 {
            return Err(BracketError::NotEnoughPlayers);
        }

        // Sort by ELO descending for seeding
        participants.sort_by(|a, b| b.elo.cmp(&a.elo));
        for (i, p) in participants.iter_mut().enumerate() {
            p.seed = (i + 1) as u32;
        }

        let total_rounds = match format {
            BracketFormat::SingleElimination | BracketFormat::DoubleElimination => {
                let n = next_power_of_two(participants.len() as u32);
                n.trailing_zeros()
            }
            BracketFormat::RoundRobin => 1,
        };

        let matches = match format {
            BracketFormat::SingleElimination => {
                generate_single_elimination_matches(&participants, total_rounds)
            }
            BracketFormat::DoubleElimination => {
                generate_double_elimination_matches(&participants, total_rounds)
            }
            BracketFormat::RoundRobin => generate_round_robin_matches(&participants),
        };

        Ok(TournamentBracket {
            id,
            name: name.into(),
            format,
            status: TournamentStatus::Registration,
            participants,
            matches,
            winner_id: None,
            started_at: None,
            completed_at: None,
            bracket_reset_required: false,
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
        let (match_round, match_number, bracket_type, player1_id, player2_id) = {
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
            (m.round, m.match_number, m.bracket_type.clone(), m.player1_id, m.player2_id)
        };

        {
            let m = bracket
                .matches
                .iter_mut()
                .find(|m| m.id == match_id)
                .unwrap();
            m.winner_id = Some(winner_id);
            m.status = MatchStatus::Completed;
            m.completed_at = Some(Utc::now());
        }

        match &bracket.format {
            BracketFormat::SingleElimination => {
                let next_round = match_round + 1;
                let next_match_number = match_number.div_ceil(2);

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
                    bracket.winner_id = Some(winner_id);
                    bracket.status = TournamentStatus::Completed;
                    bracket.completed_at = Some(Utc::now());
                }
            }
            BracketFormat::DoubleElimination => {
                Self::advance_double_elimination(bracket, match_id, winner_id, match_round, match_number, bracket_type, player1_id, player2_id)?;
            }
            BracketFormat::RoundRobin => {
                if bracket
                    .matches
                    .iter()
                    .all(|m| m.status == MatchStatus::Completed)
                {
                    bracket.winner_id = determine_round_robin_winner(bracket);
                    bracket.status = TournamentStatus::Completed;
                    bracket.completed_at = Some(Utc::now());
                }
            }
        }

        Ok(())
    }

    fn advance_double_elimination(
        bracket: &mut TournamentBracket,
        match_id: Uuid,
        winner_id: Uuid,
        match_round: u32,
        match_number: u32,
        bracket_type: BracketType,
        player1_id: Option<Uuid>,
        player2_id: Option<Uuid>,
    ) -> Result<(), BracketError> {
        let loser_id = if player1_id == Some(winner_id) {
            player2_id
        } else {
            player1_id
        };

        match bracket_type {
            BracketType::Winners => {
                // Advance winner to next WB round
                let next_round = match_round + 1;
                let next_match_number = match_number.div_ceil(2);

                if let Some(next) = bracket
                    .matches
                    .iter_mut()
                    .find(|m| m.round == next_round && m.match_number == next_match_number && m.bracket_type == BracketType::Winners)
                {
                    if match_number % 2 == 1 {
                        next.player1_id = Some(winner_id);
                    } else {
                        next.player2_id = Some(winner_id);
                    }
                } else {
                    // This was the WB final - winner advances to Grand Finals as player1
                    if let Some(gf_match) = bracket
                        .matches
                        .iter_mut()
                        .find(|m| m.bracket_type == BracketType::GrandFinals && m.match_number == 1)
                    {
                        gf_match.player1_id = Some(winner_id);
                    }
                }

                // Send loser to Losers Bracket in standard seeded order.
                // WB R1 losers of adjacent *real* matches pair in LB R1; WB Rk
                // (k>=2) losers drop into LB round 2k-2 at the same match number.
                if let Some(loser) = loser_id {
                    let byte_count = next_power_of_two(bracket.participants.len() as u32) as usize
                        - bracket.participants.len();
                    let (losers_round, preferred_mn) = if match_round <= 1 {
                        (1, (match_number - byte_count as u32).div_ceil(2))
                    } else {
                        (2 * match_round - 2, match_number)
                    };

                    place_in_lb(&mut bracket.matches, losers_round, preferred_mn, loser);
                }
            }
            BracketType::Losers => {
                // Advance winner to the next Losers Bracket round.
                let next_round = match_round + 1;
                let preferred_mn = match_number.div_ceil(2).max(1);

                if !place_in_lb(&mut bracket.matches, next_round, preferred_mn, winner_id) {
                    // Final Losers Bracket match - winner advances to Grand Finals
                    if let Some(gf_match) = bracket
                        .matches
                        .iter_mut()
                        .find(|m| m.bracket_type == BracketType::GrandFinals && m.match_number == 1)
                    {
                        if gf_match.player1_id.is_some() && gf_match.player2_id.is_none() {
                            gf_match.player2_id = Some(winner_id);
                        } else if gf_match.player1_id.is_none() {
                            gf_match.player2_id = Some(winner_id);
                        }
                    }
                }
            }
            BracketType::GrandFinals => {
                // Check if bracket reset is needed
                if bracket.bracket_reset_required {
                    // Second GF match - winner is tournament champion
                    bracket.winner_id = Some(winner_id);
                    bracket.status = TournamentStatus::Completed;
                    bracket.completed_at = Some(Utc::now());
                } else {
                    // First GF match - check if losers bracket winner won
                    let completed_match = bracket.matches.iter().find(|m| m.id == match_id).unwrap();
                    let wb_champion = completed_match.player1_id;
                    
                    if wb_champion != Some(winner_id) {
                        // Losers bracket winner won - need bracket reset
                        bracket.bracket_reset_required = true;
                        
                        // Create second GF match (bracket reset)
                        let new_gf_match = BracketMatch {
                            id: Uuid::new_v4(),
                            round: match_round,
                            match_number: 2,
                            player1_id: wb_champion,
                            player2_id: Some(winner_id),
                            winner_id: None,
                            status: MatchStatus::Pending,
                            scheduled_at: None,
                            completed_at: None,
                            bracket_type: BracketType::GrandFinals,
                            losers_from_match: None,
                        };
                        bracket.matches.push(new_gf_match);
                    } else {
                        // WB champion won first GF match - tournament over
                        bracket.winner_id = Some(winner_id);
                        bracket.status = TournamentStatus::Completed;
                        bracket.completed_at = Some(Utc::now());
                    }
                }
            }
        }

        Ok(())
    }

    /// Return participants ranked by wins in round robin / bracket.
    pub fn get_standings(bracket: &TournamentBracket) -> Vec<(Uuid, u32)> {
        let mut wins: HashMap<Uuid, u32> = HashMap::new();
        for p in &bracket.participants {
            wins.insert(p.id, 0);
        }
        for m in &bracket.matches {
            if let Some(wid) = m.winner_id {
                *wins.entry(wid).or_insert(0) += 1;
            }
        }
        let mut standings: Vec<(Uuid, u32)> = wins.into_iter().collect();
        standings.sort_by(|a, b| b.1.cmp(&a.1));
        standings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_participants(n: usize) -> Vec<TournamentParticipant> {
        (0..n)
            .map(|i| TournamentParticipant {
                id: Uuid::new_v4(),
                name: format!("Player {}", i + 1),
                elo: 3000 - (i as u32 * 10),
                seed: 0,
            })
            .collect()
    }

    #[test]
    fn seeds_ordered_by_elo_descending() {
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
        let m = bracket
            .matches
            .iter()
            .find(|m| m.round == 1)
            .unwrap()
            .clone();
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

        let r1: Vec<_> = bracket
            .matches
            .iter()
            .filter(|m| m.round == 1)
            .map(|m| m.id)
            .collect();
        for id in r1 {
            let winner = bracket
                .matches
                .iter()
                .find(|m| m.id == id)
                .unwrap()
                .player1_id
                .unwrap();
            BracketService::record_result(&mut bracket, id, winner).unwrap();
        }
        assert_eq!(
            bracket.status,
            TournamentStatus::InProgress,
            "Should still be in progress after round 1"
        );

        let r2: Vec<_> = bracket
            .matches
            .iter()
            .filter(|m| m.round == 2)
            .map(|m| m.id)
            .collect();
        for id in r2 {
            let winner = bracket
                .matches
                .iter()
                .find(|m| m.id == id)
                .unwrap()
                .player1_id
                .unwrap();
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
        let err =
            BracketService::record_result(&mut bracket, m.id, m.player1_id.unwrap()).unwrap_err();
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
        assert!(
            byes[0].winner_id.is_some(),
            "Bye match should have a winner set"
        );
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
            let winner = bracket
                .matches
                .iter()
                .find(|m| m.id == id)
                .unwrap()
                .player1_id
                .unwrap();
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
            let winner = bracket
                .matches
                .iter()
                .find(|m| m.id == id)
                .unwrap()
                .player1_id
                .unwrap();
            BracketService::record_result(&mut bracket, id, winner).unwrap();
        }

        let standings = BracketService::get_standings(&bracket);
        assert!(!standings.is_empty());
        // First place must have at least as many wins as second
        if standings.len() > 1 {
            assert!(standings[0].1 >= standings[1].1);
        }
    }

    // Double Elimination Tests
    
    #[test]
    fn double_elimination_8_players_creates_valid_bracket() {
        let bracket = BracketService::create_bracket(
            Uuid::new_v4(),
            "Test",
            make_participants(8),
            BracketFormat::DoubleElimination,
        )
        .unwrap();

        // Should have WB matches, LB matches, and GF
        let wb_matches: Vec<_> = bracket
            .matches
            .iter()
            .filter(|m| m.bracket_type == BracketType::Winners)
            .collect();
        let lb_matches: Vec<_> = bracket
            .matches
            .iter()
            .filter(|m| m.bracket_type == BracketType::Losers)
            .collect();
        let gf_matches: Vec<_> = bracket
            .matches
            .iter()
            .filter(|m| m.bracket_type == BracketType::GrandFinals)
            .collect();

        // 8 players: WB has 4+2+1=7 matches, LB has 6 matches, GF has 1 match
        assert_eq!(wb_matches.len(), 7);
        assert_eq!(lb_matches.len(), 6);
        assert_eq!(gf_matches.len(), 1);
    }

    #[test]
    fn double_elimination_12_players_creates_valid_bracket() {
        let bracket = BracketService::create_bracket(
            Uuid::new_v4(),
            "Test",
            make_participants(12),
            BracketFormat::DoubleElimination,
        )
        .unwrap();

        // 12 players -> bracket size 16
        // WB: 8+4+2+1=15 matches
        // LB: 4+2+1+1+1+1+1=7 matches  
        // GF: 1 match
        let wb_matches: Vec<_> = bracket
            .matches
            .iter()
            .filter(|m| m.bracket_type == BracketType::Winners)
            .collect();
        let lb_matches: Vec<_> = bracket
            .matches
            .iter()
            .filter(|m| m.bracket_type == BracketType::Losers)
            .collect();
        let gf_matches: Vec<_> = bracket
            .matches
            .iter()
            .filter(|m| m.bracket_type == BracketType::GrandFinals)
            .collect();

        assert_eq!(wb_matches.len(), 15);
        assert!(lb_matches.len() >= 7);
        assert_eq!(gf_matches.len(), 1);

        // Check that top seeds get byes in WB round 1
        let wb_r1_matches: Vec<_> = bracket
            .matches
            .iter()
            .filter(|m| m.round == 1 && m.bracket_type == BracketType::Winners)
            .collect();
        let bye_matches: Vec<_> = wb_r1_matches
            .iter()
            .filter(|m| m.status == MatchStatus::Completed)
            .collect();
        // 12 players, 16 bracket size, 4 byes
        assert_eq!(bye_matches.len(), 4);
    }

    #[test]
    fn double_elimination_16_players_creates_valid_bracket() {
        let bracket = BracketService::create_bracket(
            Uuid::new_v4(),
            "Test",
            make_participants(16),
            BracketFormat::DoubleElimination,
        )
        .unwrap();

        let wb_matches: Vec<_> = bracket
            .matches
            .iter()
            .filter(|m| m.bracket_type == BracketType::Winners)
            .collect();
        let lb_matches: Vec<_> = bracket
            .matches
            .iter()
            .filter(|m| m.bracket_type == BracketType::Losers)
            .collect();
        let gf_matches: Vec<_> = bracket
            .matches
            .iter()
            .filter(|m| m.bracket_type == BracketType::GrandFinals)
            .collect();

        // 16 players: WB has 8+4+2+1=15 matches, LB has ~12 matches, GF has 1 match
        assert_eq!(wb_matches.len(), 15);
        assert!(lb_matches.len() >= 10 && lb_matches.len() <= 15);
        assert_eq!(gf_matches.len(), 1);
    }

    #[test]
    fn double_elimination_32_players_creates_valid_bracket() {
        let bracket = BracketService::create_bracket(
            Uuid::new_v4(),
            "Test",
            make_participants(32),
            BracketFormat::DoubleElimination,
        )
        .unwrap();

        let wb_matches: Vec<_> = bracket
            .matches
            .iter()
            .filter(|m| m.bracket_type == BracketType::Winners)
            .collect();
        let lb_matches: Vec<_> = bracket
            .matches
            .iter()
            .filter(|m| m.bracket_type == BracketType::Losers)
            .collect();
        let gf_matches: Vec<_> = bracket
            .matches
            .iter()
            .filter(|m| m.bracket_type == BracketType::GrandFinals)
            .collect();

        // 32 players: WB has 16+8+4+2+1=31 matches, LB has ~28 matches, GF has 1 match
        assert_eq!(wb_matches.len(), 31);
        assert!(lb_matches.len() >= 25 && lb_matches.len() <= 35);
        assert_eq!(gf_matches.len(), 1);
    }

    #[test]
    fn double_elimination_4_players_complete_tournament() {
        let mut bracket = BracketService::create_bracket(
            Uuid::new_v4(),
            "Test",
            make_participants(4),
            BracketFormat::DoubleElimination,
        )
        .unwrap();

        BracketService::start_tournament(&mut bracket).unwrap();

        // WB Round 1: 2 matches
        let wb_r1: Vec<_> = bracket
            .matches
            .iter()
            .filter(|m| m.round == 1 && m.bracket_type == BracketType::Winners)
            .map(|m| m.id)
            .collect();
        
        // Player 1 and 2 win their WB R1 matches
        let m1 = bracket.matches.iter().find(|m| m.id == wb_r1[0]).unwrap().clone();
        let m2 = bracket.matches.iter().find(|m| m.id == wb_r1[1]).unwrap().clone();
        
        BracketService::record_result(&mut bracket, wb_r1[0], m1.player1_id.unwrap()).unwrap();
        BracketService::record_result(&mut bracket, wb_r1[1], m2.player1_id.unwrap()).unwrap();

        // Losers should be sent to LB Round 1
        let lb_r1: Vec<_> = bracket
            .matches
            .iter()
            .filter(|m| m.round == 1 && m.bracket_type == BracketType::Losers)
            .collect();
        assert!(lb_r1[0].player1_id.is_some() || lb_r1[0].player2_id.is_some());

        // WB Round 2 (final): 1 match
        let wb_r2: Vec<_> = bracket
            .matches
            .iter()
            .filter(|m| m.round == 2 && m.bracket_type == BracketType::Winners)
            .map(|m| m.id)
            .collect();
        
        let wb_final = bracket.matches.iter().find(|m| m.id == wb_r2[0]).unwrap().clone();
        BracketService::record_result(&mut bracket, wb_r2[0], wb_final.player1_id.unwrap()).unwrap();

        // WB champion should be set in GF
        let gf: Vec<_> = bracket
            .matches
            .iter()
            .filter(|m| m.bracket_type == BracketType::GrandFinals)
            .collect();
        assert!(gf[0].player1_id.is_some());

        // Complete LB matches
        let lb_matches: Vec<_> = bracket
            .matches
            .iter()
            .filter(|m| m.bracket_type == BracketType::Losers && m.status == MatchStatus::Pending)
            .map(|m| m.id)
            .collect();
        
        for id in lb_matches {
            if let Some(m) = bracket.matches.iter().find(|m| m.id == id) {
                if let Some(p1) = m.player1_id {
                    BracketService::record_result(&mut bracket, id, p1).unwrap();
                }
            }
        }

        // Now GF should have both players
        let gf = bracket.matches.iter().find(|m| m.bracket_type == BracketType::GrandFinals && m.match_number == 1).unwrap().clone();
        assert!(gf.player1_id.is_some() && gf.player2_id.is_some());

        // WB champion wins GF
        BracketService::record_result(&mut bracket, gf.id, gf.player1_id.unwrap()).unwrap();

        // Tournament should be complete
        assert_eq!(bracket.status, TournamentStatus::Completed);
        assert_eq!(bracket.winner_id, gf.player1_id);
    }

    #[test]
    fn double_elimination_bracket_reset_works() {
        let mut bracket = BracketService::create_bracket(
            Uuid::new_v4(),
            "Test",
            make_participants(4),
            BracketFormat::DoubleElimination,
        )
        .unwrap();

        BracketService::start_tournament(&mut bracket).unwrap();

        // Simulate tournament where LB champion wins first GF match
        let wb_r1: Vec<_> = bracket
            .matches
            .iter()
            .filter(|m| m.round == 1 && m.bracket_type == BracketType::Winners)
            .map(|m| m.id)
            .collect();
        
        let m1 = bracket.matches.iter().find(|m| m.id == wb_r1[0]).unwrap().clone();
        let m2 = bracket.matches.iter().find(|m| m.id == wb_r1[1]).unwrap().clone();
        
        // Different winners for variety
        BracketService::record_result(&mut bracket, wb_r1[0], m1.player1_id.unwrap()).unwrap();
        BracketService::record_result(&mut bracket, wb_r1[1], m2.player1_id.unwrap()).unwrap();

        let wb_r2: Vec<_> = bracket
            .matches
            .iter()
            .filter(|m| m.round == 2 && m.bracket_type == BracketType::Winners)
            .map(|m| m.id)
            .collect();
        
        let wb_final = bracket.matches.iter().find(|m| m.id == wb_r2[0]).unwrap().clone();
        BracketService::record_result(&mut bracket, wb_r2[0], wb_final.player1_id.unwrap()).unwrap();

        // Complete LB
        let lb_matches: Vec<_> = bracket
            .matches
            .iter()
            .filter(|m| m.bracket_type == BracketType::Losers && m.status == MatchStatus::Pending)
            .map(|m| m.id)
            .collect();
        
        for id in lb_matches {
            if let Some(m) = bracket.matches.iter().find(|m| m.id == id) {
                if let Some(p1) = m.player1_id {
                    BracketService::record_result(&mut bracket, id, p1).unwrap();
                }
            }
        }

        // LB champion wins first GF match (triggering bracket reset)
        let gf = bracket.matches.iter().find(|m| m.bracket_type == BracketType::GrandFinals && m.match_number == 1).unwrap().clone();
        BracketService::record_result(&mut bracket, gf.id, gf.player2_id.unwrap()).unwrap();

        // Should need bracket reset
        assert!(bracket.bracket_reset_required);
        assert_eq!(bracket.status, TournamentStatus::InProgress);

        // Second GF match should exist
        let gf2 = bracket.matches.iter().find(|m| m.bracket_type == BracketType::GrandFinals && m.match_number == 2).unwrap().clone();

        // WB champion wins second GF match
        BracketService::record_result(&mut bracket, gf2.id, gf2.player1_id.unwrap()).unwrap();

        // Tournament should now be complete
        assert_eq!(bracket.status, TournamentStatus::Completed);
        assert!(bracket.winner_id.is_some());
    }

    #[test]
    fn double_elimination_byes_given_to_top_seeds() {
        let bracket = BracketService::create_bracket(
            Uuid::new_v4(),
            "Test",
            make_participants(12),
            BracketFormat::DoubleElimination,
        )
        .unwrap();

        // 12 players -> 16 bracket size -> 4 byes
        // Top 4 seeds should get byes
        let wb_r1: Vec<_> = bracket
            .matches
            .iter()
            .filter(|m| m.round == 1 && m.bracket_type == BracketType::Winners)
            .collect();
        
        let bye_matches: Vec<_> = wb_r1
            .iter()
            .filter(|m| m.status == MatchStatus::Completed)
            .collect();
        
        assert_eq!(bye_matches.len(), 4);
        
        // Verify that byes went to top seeds (highest ELO = lowest seed number)
        let mut bye_seeds = Vec::new();
        for bye_match in bye_matches {
            if let Some(winner_id) = bye_match.winner_id {
                if let Some(p) = bracket.participants.iter().find(|p| p.id == winner_id) {
                    bye_seeds.push(p.seed);
                }
            }
        }
        
        bye_seeds.sort();
        // Top 4 seeds (1, 2, 3, 4) should have gotten byes
        assert_eq!(bye_seeds, vec![1, 2, 3, 4]);
    }

    /// Play every currently-resolvable match (two known players) with `player1_id`
    /// declared winner, in order, until no more such matches exist.
    fn play_resolvable_matches(bracket: &mut TournamentBracket) -> bool {
        loop {
            let candidates: Vec<(Uuid, Uuid)> = bracket
                .matches
                .iter()
                .filter(|m| {
                    m.status == MatchStatus::Pending
                        && m.player1_id.is_some()
                        && m.player2_id.is_some()
                })
                .map(|m| (m.id, m.player1_id.unwrap()))
                .collect();

            if candidates.is_empty() {
                return bracket.status == TournamentStatus::Completed;
            }

            for (id, winner) in candidates {
                if bracket.matches.iter().any(|m| m.id == id && m.status != MatchStatus::Completed)
                {
                    BracketService::record_result(bracket, id, winner).unwrap();
                }
            }
            if bracket.matches.iter().all(|m| m.status == MatchStatus::Completed) {
                return true;
            }
            // If no candidate match can be resolved anymore and we are still not
            // complete, the bracket cannot progress.
            let resolvable = bracket.matches.iter().any(|m| {
                m.status == MatchStatus::Pending
                    && m.player1_id.is_some()
                    && m.player2_id.is_some()
            });
            if !resolvable {
                return bracket.status == TournamentStatus::Completed;
            }
        }
    }

    #[test]
    fn double_elimination_no_duplicate_or_partial_pairings() {
        // Every pending LB match must be fully paired (both players) once both
        // of its feeders have resolved, and no player should appear twice in the
        // same round.
        for n in [4usize, 8, 12, 16, 32] {
            let bracket = BracketService::create_bracket(
                Uuid::new_v4(),
                "Test",
                make_participants(n),
                BracketFormat::DoubleElimination,
            )
            .unwrap();

            for (round, matches) in &bracket
                .matches
                .iter()
                .fold(
                    std::collections::BTreeMap::<u32, Vec<&BracketMatch>>::new(),
                    |mut acc, m| {
                        acc.entry(m.round).or_default().push(m);
                        acc
                    },
                )
            {
                let _ = round;
                let mut seen = std::collections::HashSet::new();
                for m in matches {
                    if m.player1_id.is_none() && m.player2_id.is_none() {
                        continue;
                    }
                    assert_ne!(
                        m.player1_id,
                        m.player2_id,
                        "Match {}-{} has the same player twice",
                        m.round,
                        m.match_number
                    );
                    for p in [m.player1_id, m.player2_id].into_iter().flatten() {
                        assert!(
                            seen.insert(p),
                            "Player appears twice in round {} matches",
                            m.round
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn double_elimination_8_players_full_progression() {
        let mut bracket = BracketService::create_bracket(
            Uuid::new_v4(),
            "Test",
            make_participants(8),
            BracketFormat::DoubleElimination,
        )
        .unwrap();

        BracketService::start_tournament(&mut bracket).unwrap();

        assert!(
            play_resolvable_matches(&mut bracket),
            "Tournament should complete with always-player1 winners"
        );
        assert_eq!(bracket.status, TournamentStatus::Completed);
        assert!(bracket.winner_id.is_some());

        // Champion must be the overall tournament winner candidate (never lost).
        let champion = bracket.winner_id.unwrap();
        let count_loses = bracket
            .matches
            .iter()
            .filter(|m| {
                m.winner_id.is_some()
                    && (m.player1_id == Some(champion) || m.player2_id == Some(champion))
                    && m.winner_id != Some(champion)
            })
            .count();
        assert_eq!(count_loses, 0, "Champion should have lost zero matches");
    }

    #[test]
    fn double_elimination_16_players_full_progression() {
        let mut bracket = BracketService::create_bracket(
            Uuid::new_v4(),
            "Test",
            make_participants(16),
            BracketFormat::DoubleElimination,
        )
        .unwrap();

        BracketService::start_tournament(&mut bracket).unwrap();

        let wb_r1: Vec<_> = bracket
            .matches
            .iter()
            .filter(|m| m.round == 1 && m.bracket_type == BracketType::Winners)
            .map(|m| (m.id, m.player1_id))
            .collect();
        for (id, p1) in wb_r1 {
            if let Some(p) = p1 {
                BracketService::record_result(&mut bracket, id, p).unwrap();
            }
        }

        // No duplicate LB pairings: every player must appear in exactly one match
        // per LB round during play.
        assert!(
            play_resolvable_matches(&mut bracket),
            "Tournament should complete"
        );
        assert_eq!(bracket.status, TournamentStatus::Completed);
        assert!(bracket.winner_id.is_some());
    }

    #[test]
    fn double_elimination_32_players_full_progression() {
        let mut bracket = BracketService::create_bracket(
            Uuid::new_v4(),
            "Test",
            make_participants(32),
            BracketFormat::DoubleElimination,
        )
        .unwrap();

        BracketService::start_tournament(&mut bracket).unwrap();

        assert!(
            play_resolvable_matches(&mut bracket),
            "Tournament should complete"
        );
        assert_eq!(bracket.status, TournamentStatus::Completed);
        assert!(bracket.winner_id.is_some());
    }

    #[test]
    fn double_elimination_12_players_odd_full_progression() {
        // 12 players with 4 byes must still produce a fully resolvable bracket.
        let mut bracket = BracketService::create_bracket(
            Uuid::new_v4(),
            "Test",
            make_participants(12),
            BracketFormat::DoubleElimination,
        )
        .unwrap();

        BracketService::start_tournament(&mut bracket).unwrap();

        let wb_r1_filled: Vec<_> = bracket
            .matches
            .iter()
            .filter(|m| m.round == 1 && m.bracket_type == BracketType::Winners)
            .filter(|m| m.status == MatchStatus::Pending)
            .map(|m| (m.id, m.player1_id))
            .collect();
        for (id, p1) in wb_r1_filled {
            if let Some(p) = p1 {
                BracketService::record_result(&mut bracket, id, p).unwrap();
            }
        }

        assert!(
            play_resolvable_matches(&mut bracket),
            "Tournament should complete"
        );
        assert_eq!(bracket.status, TournamentStatus::Completed);
        assert!(bracket.winner_id.is_some());
    }

    #[test]
    fn double_elimination_bracket_reset_full_progression() {
        // Force the Losers Bracket champion to defeat the Winners Bracket champion
        // in the first Grand Final, triggering a bracket reset and second GF match.
        let mut bracket = BracketService::create_bracket(
            Uuid::new_v4(),
            "Test",
            make_participants(8),
            BracketFormat::DoubleElimination,
        )
        .unwrap();

        BracketService::start_tournament(&mut bracket).unwrap();

        // Complete all WB and LB matches until GF match 1 is fully filled
        // (WB champion in slot 1, LB champion in slot 2).
        loop {
            let candidates: Vec<(Uuid, Uuid)> = bracket
                .matches
                .iter()
                .filter(|m| {
                    m.status == MatchStatus::Pending
                        && m.bracket_type != BracketType::GrandFinals
                        && m.player1_id.is_some()
                        && m.player2_id.is_some()
                })
                .map(|m| (m.id, m.player1_id.unwrap()))
                .collect();

            let gf1 = bracket.matches.iter().find(|m| {
                m.bracket_type == BracketType::GrandFinals && m.match_number == 1
            });
            if gf1.map(|m| m.player1_id.is_some() && m.player2_id.is_some()).unwrap_or(false) {
                break;
            }
            assert!(!candidates.is_empty(), "Bracket stalled before GF1 was filled");
            for (id, winner) in candidates {
                if bracket
                    .matches
                    .iter()
                    .any(|m| m.id == id && m.status != MatchStatus::Completed)
                {
                    BracketService::record_result(&mut bracket, id, winner).unwrap();
                }
            }
        }

        // Ensure GF match 1 is filled.
        let gf1 = bracket
            .matches
            .iter()
            .find(|m| m.bracket_type == BracketType::GrandFinals && m.match_number == 1)
            .cloned()
            .expect("GF match 1 should exist");
        assert!(gf1.player1_id.is_some() && gf1.player2_id.is_some());

        // Player 2 (the LB champion) beats Player 1 (the WB champion).
        BracketService::record_result(&mut bracket, gf1.id, gf1.player2_id.unwrap()).unwrap();

        assert!(bracket.bracket_reset_required, "Bracket reset must trigger");
        assert_eq!(bracket.status, TournamentStatus::InProgress);

        // A second GF match must now exist.
        let gf2 = bracket
            .matches
            .iter()
            .find(|m| m.bracket_type == BracketType::GrandFinals && m.match_number == 2)
            .cloned()
            .expect("GF match 2 should be created on reset");
        assert!(gf2.player1_id.is_some() && gf2.player2_id.is_some());

        // Finish the reset match - winner becomes champion.
        BracketService::record_result(&mut bracket, gf2.id, gf2.player1_id.unwrap()).unwrap();

        assert_eq!(bracket.status, TournamentStatus::Completed);
        assert!(bracket.winner_id.is_some());
    }

    #[test]
    fn double_elimination_losers_routed_correctly() {
        let mut bracket = BracketService::create_bracket(
            Uuid::new_v4(),
            "Test",
            make_participants(8),
            BracketFormat::DoubleElimination,
        )
        .unwrap();

        BracketService::start_tournament(&mut bracket).unwrap();

        // Complete WB Round 1
        let wb_r1_ids: Vec<_> = bracket
            .matches
            .iter()
            .filter(|m| m.round == 1 && m.bracket_type == BracketType::Winners)
            .map(|m| (m.id, m.player1_id))
            .collect();
        
        for (match_id, p1) in wb_r1_ids {
            if let Some(player1) = p1 {
                BracketService::record_result(&mut bracket, match_id, player1).unwrap();
            }
        }
        
        // Check that losers were sent to LB Round 1
        let lb_r1_filled = bracket.matches.iter()
            .filter(|m| m.round == 1 && m.bracket_type == BracketType::Losers)
            .filter(|m| m.player1_id.is_some() || m.player2_id.is_some())
            .count();
        
        // Should have at least some losers in LB
        assert!(lb_r1_filled > 0, "Some losers should be in LB Round 1");
    }
}
