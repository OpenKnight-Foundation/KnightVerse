use actix_web::{
    delete, get, post, put,
    web::{self, Json, Path},
    HttpMessage, HttpRequest, HttpResponse,
};
use db::DbPool;
use dto::players::{DisplayPlayer, NewPlayer, UpdatePlayer, UpdatedPlayer};
use error::error::ApiError;
use security::jwt::Claims;
use serde_json::json;
use service::games::GameService;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use validator::Validate;

use service::players::{
    add_player as add_new_player, delete_player as delete_player_by_id,
    find_player_by_id as get_single_player_by_id, update_player as update_player_by_id,
};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// In-memory export rate-limit store: player_id → last export timestamp
// ---------------------------------------------------------------------------
static EXPORT_RATE_LIMIT: std::sync::OnceLock<Mutex<HashMap<Uuid, Instant>>> =
    std::sync::OnceLock::new();

fn export_rate_limit_store() -> &'static Mutex<HashMap<Uuid, Instant>> {
    EXPORT_RATE_LIMIT.get_or_init(|| Mutex::new(HashMap::new()))
}

// ---------------------------------------------------------------------------
// POST /v1/players  — WRITE → primary pool
// ---------------------------------------------------------------------------
#[utoipa::path(
    post,
    path = "/v1/players",
    responses(
        (status = 200, description = "New player added", body=PlayerAdded),
        (status = 400, description = "Bad request", body=InvalidCredentialsResponse)
    )
)]
#[post("")]
pub async fn add_player(pool: web::Data<DbPool>, payload: Json<NewPlayer>) -> HttpResponse {
    match payload.0.validate() {
        Ok(_) => {
            let player = add_new_player(pool.get_ref(), payload.0).await;

            match player {
                Ok(plyr) => HttpResponse::Ok().json(json!({
                    "message":"New player added",
                    "data":DisplayPlayer::from(plyr)
                })),
                Err(err) => err.error_response(),
            }
        }
        Err(errors) => ApiError::ValidationError(errors).error_response(),
    }
}

// ---------------------------------------------------------------------------
// GET /v1/players/{id}  — READ → replica pool
// ---------------------------------------------------------------------------
#[utoipa::path(
    get,
    path = "/v1/players/{id}",
    params(
        ("id" = Uuid, Path, description = "Player ID in UUID format", format="uuid")
    ),
    responses(
        (status = 200, description = "Player found", body=PlayerFound),
        (status = 404, description = "Not found", body=NotFoundResponse)
    )
)]
#[get("/{id}")]
pub async fn find_player_by_id(pool: web::Data<DbPool>, id: Path<Uuid>) -> HttpResponse {
    let player = get_single_player_by_id(pool.get_ref(), id.into_inner()).await;

    match player {
        Ok(plyr) => HttpResponse::Ok().json(json!({
            "message":"Player found",
            "data":{
                "player": DisplayPlayer::from(plyr)
            }
        })),
        Err(err) => err.error_response(),
    }
}

// ---------------------------------------------------------------------------
// PUT /v1/players/{id}  — WRITE → primary pool
// ---------------------------------------------------------------------------
#[utoipa::path(
    put,
    path = "/v1/players/{id}",
    params(
        ("id" = Uuid, Path, description = "Player ID in UUID format", format="uuid")
    ),
    responses(
        (status = 200, description = "Player updated", body=PlayerUpdated),
        (status = 404, description = "Not found", body=NotFoundResponse)
    )
)]
#[put("/{id}")]
pub async fn update_player(
    req: HttpRequest,
    pool: web::Data<DbPool>,
    id: Path<Uuid>,
    payload: Json<UpdatePlayer>,
) -> HttpResponse {
    let path_uuid = id.into_inner();

    // IDOR check: the authenticated caller must own this profile.
    if let Some(claims) = req.extensions().get::<Claims>() {
        if claims.player_id != path_uuid {
            return HttpResponse::Forbidden().json(json!({
                "message": "You are not authorized to modify this profile"
            }));
        }
    } else {
        return HttpResponse::Unauthorized().json(json!({
            "message": "Authentication required"
        }));
    }

    match payload.0.validate() {
        Ok(_) => {
            let player = update_player_by_id(pool.get_ref(), path_uuid, payload.0).await;

            match player {
                Ok(plyr) => HttpResponse::Ok().json(json!({
                    "message":"Player updated",
                    "data":{
                        "player": UpdatedPlayer::from(plyr)
                    }
                })),
                Err(err) => err.error_response(),
            }
        }
        Err(err) => ApiError::ValidationError(err).error_response(),
    }
}

// ---------------------------------------------------------------------------
// DELETE /v1/players/{id}  — WRITE (soft-delete) → primary pool
// ---------------------------------------------------------------------------
#[utoipa::path(
    delete,
    path = "/v1/players/{id}",
    params(
        ("id" = Uuid, Path, description = "Player ID in UUID format", format="uuid")
    ),
    responses(
        (status = 200, description = "Player deleted", body=PlayerDeleted),
        (status = 404, description = "Not found", body=NotFoundResponse)
    )
)]
#[delete("/{id}")]
pub async fn delete_player(
    req: HttpRequest,
    pool: web::Data<DbPool>,
    id: Path<Uuid>,
) -> HttpResponse {
    let path_uuid = id.into_inner();

    // IDOR check: the authenticated caller must own this profile.
    if let Some(claims) = req.extensions().get::<Claims>() {
        if claims.player_id != path_uuid {
            return HttpResponse::Forbidden().json(json!({
                "message": "You are not authorized to delete this profile"
            }));
        }
    } else {
        return HttpResponse::Unauthorized().json(json!({
            "message": "Authentication required"
        }));
    }

    match delete_player_by_id(pool.get_ref(), path_uuid).await {
        Ok(_) => HttpResponse::Ok().json(json!({
            "message":"Player deleted",
            "data":{}
        })),
        Err(err) => err.error_response(),
    }
}

// ---------------------------------------------------------------------------
// GET /v1/players/me/export  — BE-94
//
// Returns a JSON bundle with the authenticated player's profile and the full
// PGN of every game they have participated in.
//
// Rate-limited to one export per player per 60 seconds.
// ---------------------------------------------------------------------------
#[utoipa::path(
    get,
    path = "/v1/players/me/export",
    responses(
        (status = 200, description = "Player data export bundle"),
        (status = 401, description = "Authentication required"),
        (status = 429, description = "Export rate limit exceeded")
    ),
    security(("jwt_auth" = [])),
    tag = "Players"
)]
#[get("/me/export")]
pub async fn export_player_data(req: HttpRequest, pool: web::Data<DbPool>) -> HttpResponse {
    // Extract JWT claims — authentication required.
    let player_id = if let Some(claims) = req.extensions().get::<Claims>() {
        claims.player_id
    } else {
        return HttpResponse::Unauthorized().json(json!({
            "message": "Authentication required"
        }));
    };

    // Rate-limit: one export per player per 60 seconds.
    {
        let mut store = export_rate_limit_store().lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        if let Some(&last) = store.get(&player_id) {
            if now.duration_since(last) < Duration::from_secs(60) {
                let remaining_secs =
                    60u64.saturating_sub(now.duration_since(last).as_secs());
                return HttpResponse::TooManyRequests().json(json!({
                    "error": "Export rate limit exceeded",
                    "message": format!(
                        "You may only request an export once every 60 seconds. \
                         Please wait {} more second(s).",
                        remaining_secs
                    ),
                    "retry_after_seconds": remaining_secs
                }));
            }
        }
        store.insert(player_id, now);
    }

    // Fetch player profile.
    let profile = match get_single_player_by_id(pool.get_ref(), player_id).await {
        Ok(p) => DisplayPlayer::from(p),
        Err(err) => return err.error_response(),
    };

    // Fetch all games the player participated in (up to 10 000 entries).
    let game_models = match GameService::get_game_history(pool.get_ref(), player_id, 10_000, None).await {
        Ok((models, _total)) => models,
        Err(e) => {
            return HttpResponse::InternalServerError().json(json!({
                "message": format!("Failed to fetch game history: {}", e)
            }));
        }
    };

    // Build the games array, rendering each game's PGN.
    let mut games: Vec<serde_json::Value> = Vec::with_capacity(game_models.len());
    for model in &game_models {
        // Determine the opponent from the caller's perspective.
        let opponent = if model.white_player == player_id {
            model.black_player.to_string()
        } else {
            model.white_player.to_string()
        };

        // Export PGN (no analysis annotations in bulk export).
        let pgn_text = match GameService::export_pgn(pool.get_ref(), model.id, false).await {
            Ok(text) => text,
            Err(_) => String::new(), // gracefully skip broken records
        };

        games.push(json!({
            "pgn":       pgn_text,
            "played_at": model.started_at,
            "opponent":  opponent
        }));
    }

    HttpResponse::Ok().json(json!({
        "message": "Player data export",
        "data": {
            "profile": profile,
            "games":   games
        }
    }))
}
