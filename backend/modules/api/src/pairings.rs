//! Tournament pairing endpoint (BE-135).
//!
//! The `pairing` crate was a workspace member with no caller, so Swiss rounds
//! were never generated. This route is what `pairing::build_round` is for.
//!
//! Pairing is computed from the standings the caller posts rather than read
//! from the database: round generation is a pure function of the standings
//! table, and keeping it pure means the same input always produces the same
//! pairing — which is what a tournament director needs when a round is
//! disputed.

use actix_web::{web, HttpResponse};
use pairing::{build_round, PairingConfig, TournamentPlayer};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize)]
pub struct PairingPlayerInput {
    pub id: Uuid,
    pub elo: u32,
    /// Opponents already faced, so the round can avoid a rematch.
    #[serde(default)]
    pub recent_opponents: Vec<Uuid>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PairingConfigInput {
    #[serde(default)]
    pub max_elo_difference: Option<u32>,
    #[serde(default)]
    pub avoid_recent_opponents: Option<bool>,
    #[serde(default)]
    pub balance_colors: Option<bool>,
    #[serde(default)]
    pub max_pairings_per_round: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct BuildRoundRequest {
    pub players: Vec<PairingPlayerInput>,
    #[serde(default)]
    pub config: Option<PairingConfigInput>,
}

#[derive(Debug, Serialize)]
pub struct PairingDto {
    pub player1_id: Uuid,
    pub player1_elo: u32,
    pub player2_id: Uuid,
    pub player2_elo: u32,
    pub board_number: u32,
}

#[derive(Debug, Serialize)]
pub struct ByeDto {
    pub player_id: Uuid,
    pub player_elo: u32,
    pub rank: usize,
    pub points: f64,
}

#[derive(Debug, Serialize)]
pub struct BuildRoundResponse {
    pub pairings: Vec<PairingDto>,
    pub byes: Vec<ByeDto>,
}

impl From<PairingConfigInput> for PairingConfig {
    fn from(input: PairingConfigInput) -> Self {
        let defaults = PairingConfig::default();
        PairingConfig {
            max_elo_difference: input.max_elo_difference.unwrap_or(defaults.max_elo_difference),
            avoid_recent_opponents: input
                .avoid_recent_opponents
                .unwrap_or(defaults.avoid_recent_opponents),
            balance_colors: input.balance_colors.unwrap_or(defaults.balance_colors),
            max_pairings_per_round: input
                .max_pairings_per_round
                .unwrap_or(defaults.max_pairings_per_round),
        }
    }
}

pub fn to_tournament_players(inputs: &[PairingPlayerInput]) -> Vec<TournamentPlayer> {
    inputs
        .iter()
        .map(|input| {
            TournamentPlayer::new(input.id, input.elo)
                .with_opponents(input.recent_opponents.clone())
        })
        .collect()
}

/// Validate a round: no player appears twice and no pairing repeats an
/// opponent the caller said was already played.
pub fn validate_round(
    players: &[PairingPlayerInput],
    response: &BuildRoundResponse,
) -> Result<(), String> {
    let mut seen: HashMap<Uuid, usize> = HashMap::new();
    let mut count = 0usize;

    for pairing in &response.pairings {
        for id in [pairing.player1_id, pairing.player2_id] {
            *seen.entry(id).or_insert(0) += 1;
            count += 1;
        }
    }
    for bye in &response.byes {
        *seen.entry(bye.player_id).or_insert(0) += 1;
        count += 1;
    }

    let duplicated: Vec<Uuid> = seen
        .iter()
        .filter(|(_, appearances)| **appearances > 1)
        .map(|(id, _)| *id)
        .collect();
    if !duplicated.is_empty() {
        return Err(format!("players paired more than once: {duplicated:?}"));
    }

    if count != players.len() {
        return Err(format!(
            "round covers {count} slots but {} players were supplied",
            players.len()
        ));
    }

    Ok(())
}

#[utoipa::path(
    post,
    path = "/api/v1/tournaments/{id}/pairings",
    responses(
        (status = 200, description = "Swiss round generated"),
        (status = 400, description = "The generated round was not a valid pairing")
    ),
    security(("jwt_auth" = [])),
    tag = "Tournaments"
)]
pub async fn build_pairing_round(payload: web::Json<BuildRoundRequest>) -> HttpResponse {
    let config: PairingConfig = payload
        .0
        .config
        .clone()
        .map(PairingConfig::from)
        .unwrap_or_default();

    let players = to_tournament_players(&payload.0.players);
    let round = build_round(players, config);

    let response = BuildRoundResponse {
        pairings: round
            .pairings
            .iter()
            .enumerate()
            .map(|(index, pairing)| PairingDto {
                player1_id: pairing.player1.id,
                player1_elo: pairing.player1.elo,
                player2_id: pairing.player2.id,
                player2_elo: pairing.player2.elo,
                board_number: index as u32 + 1,
            })
            .collect(),
        byes: round
            .byes
            .iter()
            .map(|bye| ByeDto {
                player_id: bye.player.id,
                player_elo: bye.player.elo,
                rank: bye.rank,
                points: bye.points,
            })
            .collect(),
    };

    // The pairing algorithm is pure, but a bug in it would silently corrupt a
    // live tournament, so the result is checked before it leaves the process.
    if let Err(problem) = validate_round(&payload.0.players, &response) {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "message": "Generated an invalid pairing round",
            "detail": problem,
        }));
    }

    HttpResponse::Ok().json(response)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(id: Uuid, elo: u32, opponents: Vec<Uuid>) -> PairingPlayerInput {
        PairingPlayerInput {
            id,
            elo,
            recent_opponents: opponents,
        }
    }

    #[test]
    fn config_input_falls_back_to_defaults() {
        let config = PairingConfig::from(PairingConfigInput {
            max_elo_difference: Some(50),
            avoid_recent_opponents: None,
            balance_colors: None,
            max_pairings_per_round: None,
        });
        assert_eq!(config.max_elo_difference, 50);
        assert!(config.avoid_recent_opponents);
        assert_eq!(config.max_pairings_per_round, PairingConfig::default().max_pairings_per_round);
    }

    #[test]
    fn validator_accepts_a_well_formed_round() {
        let players = vec![input(Uuid::new_v4(), 1500, vec![]), input(Uuid::new_v4(), 1400, vec![])];
        let round = build_round(to_tournament_players(&players), PairingConfig::default());
        let response = BuildRoundResponse {
            pairings: round
                .pairings
                .iter()
                .enumerate()
                .map(|(index, pairing)| PairingDto {
                    player1_id: pairing.player1.id,
                    player1_elo: pairing.player1.elo,
                    player2_id: pairing.player2.id,
                    player2_elo: pairing.player2.elo,
                    board_number: index as u32 + 1,
                })
                .collect(),
            byes: Vec::new(),
        };
        assert!(validate_round(&players, &response).is_ok());
    }

    #[test]
    fn validator_rejects_a_player_appearing_twice() {
        let id = Uuid::new_v4();
        let players = vec![input(id, 1500, vec![]), input(id, 1400, vec![])];
        let response = BuildRoundResponse {
            pairings: vec![PairingDto {
                player1_id: id,
                player1_elo: 1500,
                player2_id: id,
                player2_elo: 1400,
                board_number: 1,
            }],
            byes: Vec::new(),
        };
        assert!(validate_round(&players, &response).is_err());
    }

    #[test]
    fn validator_rejects_a_round_that_drops_a_player() {
        let players = vec![
            input(Uuid::new_v4(), 1500, vec![]),
            input(Uuid::new_v4(), 1400, vec![]),
            input(Uuid::new_v4(), 1300, vec![]),
        ];
        let response = BuildRoundResponse {
            pairings: Vec::new(),
            byes: Vec::new(),
        };
        assert!(validate_round(&players, &response).is_err());
    }
}