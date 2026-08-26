use actix_web::{
    delete, get, post, put,
    web::{self, Json, Path, Query},
    HttpMessage, HttpRequest, HttpResponse,
};
use db::DbPool;
use dto::games::{
    CompleteGameRequest, CompleteGameResponse, CreateGameRequest, GameStatus, ImportGameRequest,
    ImportGameResponse, JoinGameRequest, ListGamesQuery, MakeMoveRequest,
};
use error::error::ApiError;
use security::jwt::Claims;
use serde_json::json;
use service::games::GameService;
use uuid::Uuid;
use validator::Validate;

// ---------------------------------------------------------------------------
// Helper: extract authenticated player UUID from JWT claims.
// ---------------------------------------------------------------------------
fn authenticated_player(req: &HttpRequest) -> Result<Uuid, HttpResponse> {
    req.extensions()
        .get::<Claims>()
        .map(|c| c.player_id)
        .ok_or_else(|| {
            HttpResponse::Unauthorized().json(json!({
                "message": "Authentication required"
            }))
        })
}

// ---------------------------------------------------------------------------
// POST /v1/games  — WRITE → primary pool
// ---------------------------------------------------------------------------
#[utoipa::path(
    post,
    path = "/v1/games",
    request_body = CreateGameRequest,
    responses(
        (status = 201, description = "Game created successfully",  body = GameDisplayDTO),
        (status = 400, description = "Invalid request parameters", body = InvalidCredentialsResponse),
        (status = 401, description = "Unauthorized",               body = InvalidCredentialsResponse)
    ),
    security(("jwt_auth" = [])),
    tag = "Games"
)]
#[post("")]
pub async fn create_game(
    req: HttpRequest,
    payload: Json<CreateGameRequest>,
    pool: web::Data<DbPool>,
) -> HttpResponse {
    if let Err(errors) = payload.0.validate() {
        return ApiError::ValidationError(errors).error_response();
    }

    let creator_id = match authenticated_player(&req) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    match GameService::create_game(pool.get_ref(), creator_id, payload.0).await {
        Ok(game_dto) => HttpResponse::Created().json(json!({
            "message": "Game created successfully",
            "data": { "game": game_dto }
        })),
        Err(e) => {
            eprintln!("create_game error: {e}");
            HttpResponse::InternalServerError().json(json!({
                "message": "Failed to create game"
            }))
        }
    }
}

// ---------------------------------------------------------------------------
// GET /v1/games/{id}  — READ → replica pool
// ---------------------------------------------------------------------------
#[utoipa::path(
    get,
    path = "/v1/games/{id}",
    params(
        ("id" = Uuid, Path, description = "Game ID in UUID format", format = "uuid")
    ),
    responses(
        (status = 200, description = "Game found",     body = GameDisplayDTO),
        (status = 404, description = "Game not found", body = NotFoundResponse)
    ),
    security(("jwt_auth" = [])),
    tag = "Games"
)]
#[get("/{id}")]
pub async fn get_game(id: Path<Uuid>, pool: web::Data<DbPool>) -> HttpResponse {
    let game_id = id.into_inner();

    match GameService::get_game(pool.get_ref(), game_id).await {
        Ok(game_dto) => HttpResponse::Ok().json(json!({
            "message": "Game found",
            "data": { "game": game_dto }
        })),
        Err(ApiError::NotFound(_)) => HttpResponse::NotFound().json(json!({
            "message": "Game not found"
        })),
        Err(e) => {
            eprintln!("get_game error: {e}");
            HttpResponse::InternalServerError().json(json!({
                "message": "Failed to fetch game"
            }))
        }
    }
}

// ---------------------------------------------------------------------------
// PUT /v1/games/{id}/move  — WRITE → primary pool
// ---------------------------------------------------------------------------
#[utoipa::path(
    put,
    path = "/v1/games/{id}/move",
    params(
        ("id" = Uuid, Path, description = "Game ID in UUID format", format = "uuid")
    ),
    request_body = MakeMoveRequest,
    responses(
        (status = 200, description = "Move made successfully", body = GameDisplayDTO),
        (status = 400, description = "Invalid move",           body = InvalidCredentialsResponse),
        (status = 404, description = "Game not found",         body = NotFoundResponse)
    ),
    security(("jwt_auth" = [])),
    tag = "Games"
)]
#[put("/{id}/move")]
pub async fn make_move(
    req: HttpRequest,
    id: Path<Uuid>,
    payload: Json<MakeMoveRequest>,
    pool: web::Data<DbPool>,
) -> HttpResponse {
    if let Err(errors) = payload.0.validate() {
        return ApiError::ValidationError(errors).error_response();
    }

    let player_id = match authenticated_player(&req) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    let game_id = id.into_inner();

    match GameService::make_move(pool.get_ref(), game_id, player_id, payload.0).await {
        Ok(game_dto) => HttpResponse::Ok().json(json!({
            "message": "Move made successfully",
            "data": { "game": game_dto }
        })),
        Err(ApiError::NotFound(_)) => HttpResponse::NotFound().json(json!({
            "message": "Game not found"
        })),
        Err(ApiError::BadRequest(msg)) => HttpResponse::BadRequest().json(json!({
            "message": msg
        })),
        Err(ApiError::Forbidden(_)) => HttpResponse::Forbidden().json(json!({
            "message": "It is not your turn"
        })),
        Err(e) => {
            eprintln!("make_move error: {e}");
            HttpResponse::InternalServerError().json(json!({
                "message": "Failed to apply move"
            }))
        }
    }
}

// ---------------------------------------------------------------------------
// GET /v1/games  — READ → replica pool
// ---------------------------------------------------------------------------
#[utoipa::path(
    get,
    path = "/v1/games",
    params(
        ("status"    = Option<String>, Query, description = "Filter by status (waiting, in_progress, completed, aborted)"),
        ("player_id" = Option<Uuid>, Query, description = "Filter by player UUID", format = "uuid"),
        ("page"      = Option<i32>,    Query, description = "Page number"),
        ("limit"     = Option<i32>,    Query, description = "Items per page")
    ),
    responses(
        (status = 200, description = "List of games", body = Vec<GameDisplayDTO>)
    ),
    security(("jwt_auth" = [])),
    tag = "Games"
)]
#[get("")]
pub async fn list_games(
    query: Query<ListGamesQuery>,
    pool: web::Data<DbPool>,
) -> HttpResponse {
    let status_enum: Option<GameStatus> = query.status.as_deref().and_then(|s| match s {
        "waiting" => Some(GameStatus::Waiting),
        "in_progress" => Some(GameStatus::InProgress),
        "completed" => Some(GameStatus::Completed),
        "aborted" => Some(GameStatus::Aborted),
        _ => None,
    });

    let limit = query.limit.unwrap_or(10);
    let cursor = query.cursor.clone();

    let offset: Option<u64> = if cursor.is_none() {
        query.page.map(|p| {
            let page = if p < 1 {
                tracing::warn!("Invalid page value {} — clamping to 1", p);
                1
            } else {
                p as u64
            };
            (page - 1) * limit
        })
    } else {
        None
    };

    match GameService::list_games(
        pool.get_ref(),
        cursor,
        offset,
        limit,
        query.player_id,
        status_enum,
    )
    .await
    {
        Ok((games, next_cursor, total_count)) => {
            let game_dtos: Vec<serde_json::Value> = games
                .into_iter()
                .map(|g| {
                    let status = match &g.result {
                        Some(db_entity::game::ResultSide::Ongoing) => "in_progress",
                        Some(_) => "completed",
                        None => "waiting",
                    };
                    json!({
                        "id":              g.id,
                        "white_player_id": g.white_player,
                        "black_player_id": g.black_player,
                        "status":          status,
                        "result":          g.result,
                        "current_fen":     g.fen,
                        "created_at":      g.created_at,
                        "started_at":      g.started_at,
                    })
                })
                .collect();

            HttpResponse::Ok().json(json!({
                "message": "Games found",
                "data": {
                    "games":       game_dtos,
                    "total_count": total_count,
                    "next_cursor": next_cursor,
                    "limit":       limit,
                }
            }))
        }
        Err(e) => {
            eprintln!("list_games error: {e}");
            HttpResponse::InternalServerError().json(json!({
                "message": "Failed to list games"
            }))
        }
    }
}

// ---------------------------------------------------------------------------
// POST /v1/games/{id}/join  — WRITE → primary pool
// ---------------------------------------------------------------------------
#[utoipa::path(
    post,
    path = "/v1/games/{id}/join",
    params(
        ("id" = Uuid, Path, description = "Game ID in UUID format", format = "uuid")
    ),
    request_body = JoinGameRequest,
    responses(
        (status = 200, description = "Joined game successfully", body = GameDisplayDTO),
        (status = 400, description = "Cannot join game",         body = InvalidCredentialsResponse),
        (status = 404, description = "Game not found",           body = NotFoundResponse)
    ),
    security(("jwt_auth" = [])),
    tag = "Games"
)]
#[post("/{id}/join")]
pub async fn join_game(
    req: HttpRequest,
    id: Path<Uuid>,
    payload: Json<JoinGameRequest>,
    pool: web::Data<DbPool>,
) -> HttpResponse {
    if let Err(errors) = payload.0.validate() {
        return ApiError::ValidationError(errors).error_response();
    }

    let player_id = match authenticated_player(&req) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    if player_id.is_nil() {
        return HttpResponse::Unauthorized().json(json!({
            "message": "Player identity could not be resolved from token"
        }));
    }

    let game_id = id.into_inner();

    match GameService::join_game(pool.get_ref(), game_id, player_id).await {
        Ok(game_dto) => HttpResponse::Ok().json(json!({
            "message": "Joined game successfully",
            "data": { "game": game_dto }
        })),
        Err(ApiError::NotFound(_)) => HttpResponse::NotFound().json(json!({
            "message": "Game not found"
        })),
        Err(ApiError::BadRequest(msg)) => HttpResponse::BadRequest().json(json!({
            "message": msg
        })),
        Err(e) => {
            eprintln!("join_game error: {e}");
            HttpResponse::InternalServerError().json(json!({
                "message": "Failed to join game"
            }))
        }
    }
}

// ---------------------------------------------------------------------------
// DELETE /v1/games/{id}  — WRITE (soft-delete) → primary pool
// ---------------------------------------------------------------------------
#[utoipa::path(
    delete,
    path = "/v1/games/{id}",
    params(
        ("id" = Uuid, Path, description = "Game ID in UUID format", format = "uuid")
    ),
    responses(
        (status = 200, description = "Game abandoned successfully"),
        (status = 404, description = "Game not found",              body = NotFoundResponse)
    ),
    security(("jwt_auth" = [])),
    tag = "Games"
)]
#[delete("/{id}")]
pub async fn abandon_game(
    req: HttpRequest,
    id: Path<Uuid>,
    pool: web::Data<DbPool>,
) -> HttpResponse {
    let player_id = match authenticated_player(&req) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    let game_id = id.into_inner();

    match GameService::abandon_game(pool.get_ref(), game_id, player_id).await {
        Ok(_) => HttpResponse::Ok().json(json!({
            "message": "Game abandoned successfully",
            "data": {}
        })),
        Err(ApiError::NotFound(_)) => HttpResponse::NotFound().json(json!({
            "message": "Game not found"
        })),
        Err(ApiError::Forbidden(_)) => HttpResponse::Forbidden().json(json!({
            "message": "You are not a participant in this game"
        })),
        Err(e) => {
            eprintln!("abandon_game error: {e}");
            HttpResponse::InternalServerError().json(json!({
                "message": "Failed to abandon game"
            }))
        }
    }
}

// ---------------------------------------------------------------------------
// POST /v1/games/import  — WRITE → primary pool
// ---------------------------------------------------------------------------
#[utoipa::path(
    post,
    path = "/v1/games/import",
    request_body = ImportGameRequest,
    responses(
        (status = 201, description = "Game imported successfully", body = ImportGameResponse),
        (status = 400, description = "Invalid PGN format",         body = InvalidCredentialsResponse),
        (status = 422, description = "Illegal moves in PGN",       body = InvalidCredentialsResponse)
    ),
    security(("jwt_auth" = [])),
    tag = "Games"
)]
#[post("/import")]
pub async fn import_game(
    req: HttpRequest,
    payload: Json<ImportGameRequest>,
    pool: web::Data<DbPool>,
) -> HttpResponse {
    if let Err(errors) = payload.0.validate() {
        return ApiError::ValidationError(errors).error_response();
    }

    let importer_id = match authenticated_player(&req) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    let parsed = match chess::parse_pgn(&payload.pgn) {
        Ok(p) => p,
        Err(e) => {
            return HttpResponse::BadRequest().json(ImportGameResponse {
                success: false,
                game_id: None,
                white_player: String::new(),
                black_player: String::new(),
                result: String::new(),
                move_count: 0,
                final_fen: None,
                error: Some(e.to_string()),
            });
        }
    };

    let validated = match chess::validate_game(&parsed) {
        Ok(v) => v,
        Err(e) => {
            return HttpResponse::UnprocessableEntity().json(ImportGameResponse {
                success: false,
                game_id: None,
                white_player: parsed.headers.white.clone(),
                black_player: parsed.headers.black.clone(),
                result: String::new(),
                move_count: 0,
                final_fen: None,
                error: Some(e.to_string()),
            });
        }
    };

    let result_str = validated.headers.result.to_pgn_string().to_string();

    match GameService::import_game(pool.get_ref(), importer_id, &validated).await {
        Ok(game_id) => HttpResponse::Created().json(ImportGameResponse {
            success: true,
            game_id: Some(game_id),
            white_player: validated.headers.white,
            black_player: validated.headers.black,
            result: result_str,
            move_count: validated.ply_count,
            final_fen: Some(validated.final_fen),
            error: None,
        }),
        Err(e) => {
            eprintln!("import_game DB error: {e}");
            HttpResponse::InternalServerError().json(ImportGameResponse {
                success: false,
                game_id: None,
                white_player: validated.headers.white,
                black_player: validated.headers.black,
                result: result_str,
                move_count: validated.ply_count,
                final_fen: Some(validated.final_fen),
                error: Some("Failed to persist imported game".to_string()),
            })
        }
    }
}

// ---------------------------------------------------------------------------
// PUT /v1/games/{id}/complete  — WRITE (transaction) → primary pool
// ---------------------------------------------------------------------------
#[utoipa::path(
    put,
    path = "/v1/games/{id}/complete",
    params(
        ("id" = Uuid, Path, description = "Game ID in UUID format", format = "uuid")
    ),
    request_body = CompleteGameRequest,
    responses(
        (status = 200, description = "Game completed and ratings updated", body = CompleteGameResponse),
        (status = 400, description = "Invalid game result or game already completed", body = InvalidCredentialsResponse),
        (status = 404, description = "Game not found", body = NotFoundResponse)
    ),
    security(("jwt_auth" = [])),
    tag = "Games"
)]
#[put("/{id}/complete")]
pub async fn complete_game(
    req: HttpRequest,
    id: Path<Uuid>,
    payload: Json<CompleteGameRequest>,
    pool: web::Data<DbPool>,
) -> HttpResponse {
    if let Err(errors) = payload.0.validate() {
        return ApiError::ValidationError(errors).error_response();
    }

    let _player_id = match authenticated_player(&req) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    let game_id = id.into_inner();

    let result_enum = match payload.result.as_str() {
        "white_wins" => db_entity::game::ResultSide::WhiteWins,
        "black_wins" => db_entity::game::ResultSide::BlackWins,
        "draw" => db_entity::game::ResultSide::Draw,
        "abandoned" => db_entity::game::ResultSide::Abandoned,
        _ => {
            return HttpResponse::BadRequest().json(json!({
                "message": "Invalid result. Must be one of: white_wins, black_wins, draw, abandoned"
            }));
        }
    };

    let rating_config = chess::RatingConfig {
        k_factor: payload.k_factor.unwrap_or(32),
        ..Default::default()
    };

    // Fetch current ratings from replica before the write
    let white_old_rating =
        match GameService::get_player_rating_for_game(pool.get_ref(), game_id, true).await {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Failed to get white player rating: {e}");
                return HttpResponse::InternalServerError().json(json!({
                    "message": "Failed to get player ratings"
                }));
            }
        };

    let black_old_rating =
        match GameService::get_player_rating_for_game(pool.get_ref(), game_id, false).await {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Failed to get black player rating: {e}");
                return HttpResponse::InternalServerError().json(json!({
                    "message": "Failed to get player ratings"
                }));
            }
        };

    match GameService::complete_game(
        pool.get_ref(),
        game_id,
        result_enum.clone(),
        Some(rating_config),
    )
    .await
    {
        Ok((white_new_rating, black_new_rating)) => {
            let white_change = white_new_rating - white_old_rating;
            let black_change = black_new_rating - black_old_rating;

            HttpResponse::Ok().json(CompleteGameResponse {
                success: true,
                game_id,
                result: payload.result.clone(),
                white_new_rating,
                black_new_rating,
                rating_change_white: white_change,
                rating_change_black: black_change,
                error: None,
            })
        }
        Err(ApiError::NotFound(_)) => HttpResponse::NotFound().json(CompleteGameResponse {
            success: false,
            game_id,
            result: payload.result.clone(),
            white_new_rating: white_old_rating,
            black_new_rating: black_old_rating,
            rating_change_white: 0,
            rating_change_black: 0,
            error: Some("Game not found".to_string()),
        }),
        Err(ApiError::BadRequest(msg)) => HttpResponse::BadRequest().json(CompleteGameResponse {
            success: false,
            game_id,
            result: payload.result.clone(),
            white_new_rating: white_old_rating,
            black_new_rating: black_old_rating,
            rating_change_white: 0,
            rating_change_black: 0,
            error: Some(msg),
        }),
        Err(e) => {
            eprintln!("complete_game error: {e}");
            HttpResponse::InternalServerError().json(CompleteGameResponse {
                success: false,
                game_id,
                result: payload.result.clone(),
                white_new_rating: white_old_rating,
                black_new_rating: black_old_rating,
                rating_change_white: 0,
                rating_change_black: 0,
                error: Some("Failed to complete game".to_string()),
            })
        }
    }
}
