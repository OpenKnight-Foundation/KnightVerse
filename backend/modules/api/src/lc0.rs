//! Leela Chess Zero HTTP API (BE-132).
//!
//! `Lc0Orchestrator` shipped as an unreachable stub; these routes are what make
//! it callable. They sit under the JWT-gated `/v1/ai` scope alongside the
//! Stockfish endpoints.

use actix_web::{get, post, web, web::Json, HttpResponse, Result};
use dto::responses::ValidationErrorResponse;
use engine::lc0_orchestrator::{Lc0Analysis, Lc0Orchestrator};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use tracing::error;
use utoipa::ToSchema;

#[derive(Deserialize, ToSchema)]
pub struct Lc0AnalyzeRequest {
    /// FEN of the position to search. Defaults to the standard start position.
    pub fen: Option<String>,
    /// Search depth. Defaults to the orchestrator's configured depth.
    pub depth: Option<u8>,
}

#[derive(Serialize)]
pub struct Lc0EngineInfo {
    pub available: bool,
    pub engine: String,
    pub max_concurrency: usize,
}

#[utoipa::path(
    post,
    path = "/v1/ai/lc0/analyze",
    request_body = Lc0AnalyzeRequest,
    responses(
        (status = 200, description = "Lc0 analysis completed"),
        (status = 400, description = "Missing or malformed FEN"),
        (status = 503, description = "Lc0 is not installed on this host"),
        (status = 504, description = "Lc0 did not answer in time")
    ),
    security(("jwt_auth" = [])),
    tag = "AI"
)]
#[post("/lc0/analyze")]
pub async fn lc0_analyze(
    orchestrator: web::Data<Arc<Lc0Orchestrator>>,
    payload: Json<Lc0AnalyzeRequest>,
) -> Result<HttpResponse> {
    let fen = payload.0.fen.clone().unwrap_or_else(|| {
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".to_string()
    });

    // `shakmaty` parses castling in standard mode here, so Chess960 start FENs
    // are rejected until `fen_validator` gains a `CastlingMode::Chess960`
    // path. Standard positions are validated so Lc0 cannot be fed a position
    // the rest of the API would reject.
    if chess::fen_validator::validate_fen_legality(&fen).is_err() {
        return Ok(HttpResponse::BadRequest().json(ValidationErrorResponse {
            error: "Invalid FEN position".to_string(),
            code: 400,
            details: Some(vec![format!("`{}` is not a legal position", fen)]),
        }));
    }

    match orchestrator.analyze_position(&fen, payload.0.depth).await {
        Ok(analysis) => Ok(HttpResponse::Ok().json(analysis)),
        Err(engine::EngineError::Unknown(message)) => {
            error!("Lc0 unavailable: {}", message);
            Ok(HttpResponse::ServiceUnavailable().json(json!({
                "error": "Lc0 engine unavailable",
                "detail": message,
            })))
        }
        Err(engine::EngineError::Timeout) => Ok(HttpResponse::GatewayTimeout().json(json!({
            "error": "Lc0 timed out or all engine slots are busy"
        }))),
        Err(e) => {
            error!("Lc0 analysis failed: {}", e);
            Ok(HttpResponse::InternalServerError().json(json!({
                "error": "internal server error"
            })))
        }
    }
}

#[utoipa::path(
    get,
    path = "/v1/ai/lc0/health",
    responses((status = 200, description = "Lc0 availability")),
    security(("jwt_auth" = [])),
    tag = "AI"
)]
#[get("/lc0/health")]
pub async fn lc0_health(orchestrator: web::Data<Arc<Lc0Orchestrator>>) -> HttpResponse {
    HttpResponse::Ok().json(Lc0EngineInfo {
        available: orchestrator.is_available(),
        engine: orchestrator.get_engine_info(),
        max_concurrency: orchestrator.concurrency(),
    })
}