//! Persisted tournament creation and registration endpoints (BE-114).

use actix_web::{web, HttpMessage, HttpRequest, HttpResponse};
use db::DbPool;
use db_entity::{player, tournament as tournament_entity, tournament_participant};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter, QueryOrder,
};
use security::jwt::Claims;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tournament::{BracketFormat, BracketService, TournamentParticipant};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TournamentFormat {
    SingleElimination,
    DoubleElimination,
    RoundRobin,
}

impl From<TournamentFormat> for BracketFormat {
    fn from(format: TournamentFormat) -> Self {
        match format {
            TournamentFormat::SingleElimination => Self::SingleElimination,
            TournamentFormat::DoubleElimination => Self::DoubleElimination,
            TournamentFormat::RoundRobin => Self::RoundRobin,
        }
    }
}

impl TournamentFormat {
    fn as_str(self) -> &'static str {
        match self {
            Self::SingleElimination => "single_elimination",
            Self::DoubleElimination => "double_elimination",
            Self::RoundRobin => "round_robin",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "single_elimination" => Some(Self::SingleElimination),
            "double_elimination" => Some(Self::DoubleElimination),
            "round_robin" => Some(Self::RoundRobin),
            _ => None,
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateTournamentRequest {
    pub name: String,
    #[serde(default = "default_format")]
    pub format: TournamentFormat,
}

fn default_format() -> TournamentFormat {
    TournamentFormat::SingleElimination
}

#[utoipa::path(
    get,
    path = "/api/v1/tournaments",
    responses((status = 200, description = "Tournament list")),
    security(("jwt_auth" = [])),
    tag = "Tournaments"
)]
pub async fn list_tournaments(pool: web::Data<DbPool>) -> HttpResponse {
    match tournament_entity::Entity::find()
        .order_by_desc(tournament_entity::Column::CreatedAt)
        .all(pool.replica())
        .await
    {
        Ok(items) => HttpResponse::Ok().json(json!({ "data": items })),
        Err(error) => {
            tracing::error!(%error, "failed to list tournaments");
            HttpResponse::InternalServerError().json(json!({ "message": "Failed to list tournaments" }))
        }
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/tournaments",
    request_body = CreateTournamentRequest,
    responses(
        (status = 201, description = "Tournament created"),
        (status = 400, description = "Invalid tournament name")
    ),
    security(("jwt_auth" = [])),
    tag = "Tournaments"
)]
pub async fn create_tournament(
    payload: web::Json<CreateTournamentRequest>,
    pool: web::Data<DbPool>,
) -> HttpResponse {
    let name = payload.name.trim();
    if name.is_empty() {
        return HttpResponse::BadRequest().json(json!({ "message": "Tournament name is required" }));
    }

    let model = tournament_entity::ActiveModel {
        id: Set(Uuid::new_v4()),
        name: Set(name.to_owned()),
        format: Set(payload.format.as_str().to_owned()),
        status: Set("registration".to_owned()),
        bracket_data: Set(None),
        created_at: Default::default(),
    };
    match model.insert(pool.primary()).await {
        Ok(created) => HttpResponse::Created().json(json!({ "data": created })),
        Err(error) => {
            tracing::error!(%error, "failed to create tournament");
            HttpResponse::InternalServerError().json(json!({ "message": "Failed to create tournament" }))
        }
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/tournaments/{id}/register",
    params(("id" = Uuid, Path, description = "Tournament ID")),
    responses(
        (status = 200, description = "Player registered"),
        (status = 404, description = "Tournament not found"),
        (status = 409, description = "Player is already registered")
    ),
    security(("jwt_auth" = [])),
    tag = "Tournaments"
)]
pub async fn register_for_tournament(
    req: HttpRequest,
    id: web::Path<Uuid>,
    pool: web::Data<DbPool>,
) -> HttpResponse {
    let player_id = match req.extensions().get::<Claims>().map(|claims| claims.player_id) {
        Some(id) => id,
        None => return HttpResponse::Unauthorized().json(json!({ "message": "Authentication required" })),
    };
    let conn = pool.primary();
    let tournament_id = id.into_inner();

    let event = match tournament_entity::Entity::find_by_id(tournament_id).one(conn).await {
        Ok(Some(event)) => event,
        Ok(None) => return HttpResponse::NotFound().json(json!({ "message": "Tournament not found" })),
        Err(error) => {
            tracing::error!(%error, "failed to load tournament");
            return HttpResponse::InternalServerError().json(json!({ "message": "Failed to load tournament" }));
        }
    };

    let existing_registration = match tournament_participant::Entity::find()
        .filter(tournament_participant::Column::TournamentId.eq(tournament_id))
        .filter(tournament_participant::Column::PlayerId.eq(player_id))
        .one(conn)
        .await
    {
        Ok(registration) => registration,
        Err(error) => {
            tracing::error!(%error, "failed to check tournament registration");
            return HttpResponse::InternalServerError().json(json!({ "message": "Failed to check registration" }));
        }
    };
    if existing_registration.is_some() {
        return HttpResponse::Conflict().json(json!({ "message": "Player is already registered" }));
    }

    let player = match player::Entity::find_by_id(player_id).one(conn).await {
        Ok(Some(player)) => player,
        Ok(None) => return HttpResponse::NotFound().json(json!({ "message": "Player not found" })),
        Err(error) => {
            tracing::error!(%error, "failed to load player for tournament registration");
            return HttpResponse::InternalServerError().json(json!({ "message": "Failed to load player" }));
        }
    };

    let participant = tournament_participant::ActiveModel {
        id: Set(Uuid::new_v4()),
        tournament_id: Set(tournament_id),
        player_id: Set(player_id),
        display_name: Set(player.username),
        elo: Set(player.elo_rating.max(0)),
        created_at: Default::default(),
    };
    if let Err(error) = participant.insert(conn).await {
        let message = error.to_string();
        if message.to_ascii_lowercase().contains("unique")
            || message.to_ascii_lowercase().contains("duplicate")
        {
            return HttpResponse::Conflict().json(json!({ "message": "Player is already registered" }));
        }
        tracing::error!(%error, "failed to register player for tournament");
        return HttpResponse::InternalServerError().json(json!({ "message": "Failed to register player" }));
    }

    let participants = match tournament_participant::Entity::find()
        .filter(tournament_participant::Column::TournamentId.eq(tournament_id))
        .order_by_asc(tournament_participant::Column::CreatedAt)
        .all(conn)
        .await
    {
        Ok(rows) => rows,
        Err(error) => {
            tracing::error!(%error, "failed to load registered tournament players");
            return HttpResponse::InternalServerError().json(json!({ "message": "Failed to load registered players" }));
        }
    };

    let bracket = if participants.len() >= 2 {
        let Some(format) = TournamentFormat::parse(&event.format) else {
            return HttpResponse::InternalServerError().json(json!({ "message": "Tournament format is invalid" }));
        };
        let bracket_participants = participants
            .iter()
            .map(|participant| TournamentParticipant {
                id: participant.player_id,
                name: participant.display_name.clone(),
                elo: participant.elo as u32,
                seed: 0,
            })
            .collect();
        match BracketService::create_bracket(
            tournament_id,
            event.name.clone(),
            bracket_participants,
            format.into(),
        ) {
            Ok(bracket) => match serde_json::to_value(bracket) {
                Ok(value) => {
                    let mut update: tournament_entity::ActiveModel = event.into();
                    update.bracket_data = Set(Some(value.into()));
                    if let Err(error) = update.update(conn).await {
                        tracing::error!(%error, "failed to persist tournament bracket");
                        return HttpResponse::InternalServerError().json(json!({ "message": "Failed to generate tournament bracket" }));
                    }
                    Some(value)
                }
                Err(error) => {
                    tracing::error!(%error, "failed to serialize tournament bracket");
                    return HttpResponse::InternalServerError().json(json!({ "message": "Failed to generate tournament bracket" }));
                }
            },
            Err(error) => {
                tracing::error!(%error, "failed to generate tournament bracket");
                return HttpResponse::InternalServerError().json(json!({ "message": "Failed to generate tournament bracket" }));
            }
        }
    } else {
        None
    };

    HttpResponse::Ok().json(json!({
        "status": "registered",
        "tournament_id": tournament_id,
        "participant_count": participants.len(),
        "bracket": bracket
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tournament_format_uses_api_snake_case() {
        assert_eq!(
            serde_json::to_string(&TournamentFormat::DoubleElimination).unwrap(),
            "\"double_elimination\""
        );
        assert!(TournamentFormat::parse("round_robin").is_some());
        assert!(TournamentFormat::parse("unknown").is_none());
    }

    #[test]
    fn registering_two_players_uses_bracket_generation() {
        let players = vec![
            TournamentParticipant {
                id: Uuid::new_v4(),
                name: "Alice".into(),
                elo: 1600,
                seed: 0,
            },
            TournamentParticipant {
                id: Uuid::new_v4(),
                name: "Bob".into(),
                elo: 1500,
                seed: 0,
            },
        ];
        let bracket = BracketService::create_bracket(
            Uuid::new_v4(),
            "Test".into(),
            players,
            BracketFormat::SingleElimination,
        )
        .unwrap();
        assert_eq!(bracket.matches.len(), 1);
        assert_eq!(bracket.participants[0].seed, 1);
    }
}
