//! Chess960 HTTP API (BE-132).
//!
//! These handlers used to live in `backend/src/chess960/api.rs`, inside the
//! `knightverse-core` shell crate. `api::server` never called
//! `configure_routes`, so Chess960 position lookup and FEN verification were
//! unreachable over HTTP. The domain logic now lives in `chess::chess960`;
//! the routes are registered by `server::main` under `/v1/chess960`.

use actix_web::{web, HttpResponse, Result};
use chess::chess960::{Chess960Generator, Chess960Library, Chess960Position};
use rand::Rng;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

/// The full 960-position library is expensive to build, so generate it once
/// per process instead of once per request.
fn library() -> &'static Chess960Library {
    static LIBRARY: OnceLock<Chess960Library> = OnceLock::new();
    LIBRARY.get_or_init(Chess960Generator::generate_all_positions)
}

fn king_distribution() -> &'static HashMap<usize, u32> {
    static DISTRIBUTION: OnceLock<HashMap<usize, u32>> = OnceLock::new();
    DISTRIBUTION.get_or_init(|| {
        let mut distribution = HashMap::new();
        for position in library().positions.values() {
            *distribution.entry(position.white_king_pos).or_insert(0) += 1;
        }
        distribution
    })
}

fn known_fens() -> &'static HashSet<String> {
    static FENS: OnceLock<HashSet<String>> = OnceLock::new();
    FENS.get_or_init(|| library().positions.values().map(|p| p.fen.clone()).collect())
}

#[derive(Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub message: String,
}

#[derive(Deserialize)]
pub struct PositionQuery {
    pub number: Option<u16>,
    pub random: Option<bool>,
}

#[derive(Deserialize, ToSchema)]
pub struct FenVerifyRequest {
    pub fen: String,
}

#[derive(Serialize)]
pub struct StatsResponse {
    pub total_positions: u16,
    pub king_distribution: HashMap<usize, u32>,
    pub version: String,
}

#[utoipa::path(
    get,
    path = "/v1/chess960/position",
    params(
        ("number" = Option<u16>, Query, description = "Position number, 1-960"),
        ("random" = Option<bool>, Query, description = "Return a random position instead")
    ),
    responses(
        (status = 200, description = "Chess960 starting position"),
        (status = 400, description = "Neither `number` nor `random=true` supplied"),
        (status = 404, description = "Position number out of range")
    ),
    tag = "Chess960"
)]
pub async fn get_position(query: web::Query<PositionQuery>) -> Result<HttpResponse> {
    let position = if query.random.unwrap_or(false) {
        let random_id = rand::thread_rng().gen_range(1..=960);
        library().positions.get(&random_id).cloned()
    } else if let Some(number) = query.number {
        library().positions.get(&number).cloned()
    } else {
        return Ok(HttpResponse::BadRequest().json(ApiResponse::<Chess960Position> {
            success: false,
            data: None,
            message: "Specify 'number' parameter or set 'random=true'".to_string(),
        }));
    };

    match position {
        Some(pos) => Ok(HttpResponse::Ok().json(ApiResponse {
            success: true,
            data: Some(pos),
            message: "Position retrieved successfully".to_string(),
        })),
        None => Ok(HttpResponse::NotFound().json(ApiResponse::<Chess960Position> {
            success: false,
            data: None,
            message: "Position not found".to_string(),
        })),
    }
}

#[utoipa::path(
    get,
    path = "/v1/chess960/fen/{number}",
    params(("number" = u16, Path, description = "Position number, 1-960")),
    responses(
        (status = 200, description = "Starting FEN for the position"),
        (status = 404, description = "Position number out of range")
    ),
    tag = "Chess960"
)]
pub async fn get_fen(path: web::Path<u16>) -> Result<HttpResponse> {
    let number = path.into_inner();

    match library().positions.get(&number) {
        Some(position) => Ok(HttpResponse::Ok().json(ApiResponse {
            success: true,
            data: Some(position.fen.clone()),
            message: format!("FEN for position {} retrieved", number),
        })),
        None => Ok(HttpResponse::NotFound().json(ApiResponse::<String> {
            success: false,
            data: None,
            message: format!("Position {} not found", number),
        })),
    }
}

#[utoipa::path(
    post,
    path = "/v1/chess960/verify",
    request_body = FenVerifyRequest,
    responses((status = 200, description = "Whether the FEN is a valid Chess960 start")),
    tag = "Chess960"
)]
pub async fn verify_fen(req: web::Json<FenVerifyRequest>) -> Result<HttpResponse> {
    let is_valid = known_fens().contains(&req.fen);

    Ok(HttpResponse::Ok().json(ApiResponse {
        success: true,
        data: Some(is_valid),
        message: if is_valid {
            "Valid Chess960 FEN".to_string()
        } else {
            "Not a valid Chess960 FEN".to_string()
        },
    }))
}

#[utoipa::path(
    get,
    path = "/v1/chess960/stats",
    responses((status = 200, description = "Library statistics")),
    tag = "Chess960"
)]
pub async fn get_stats() -> Result<HttpResponse> {
    let stats = StatsResponse {
        total_positions: library().total_positions,
        king_distribution: king_distribution().clone(),
        version: library().metadata.version.clone(),
    };

    Ok(HttpResponse::Ok().json(ApiResponse {
        success: true,
        data: Some(stats),
        message: "Statistics retrieved successfully".to_string(),
    }))
}

#[utoipa::path(
    get,
    path = "/v1/chess960/export",
    responses((status = 200, description = "The full position library as JSON")),
    tag = "Chess960"
)]
pub async fn export_json() -> Result<HttpResponse> {
    match serde_json::to_string_pretty(library()) {
        Ok(json) => Ok(HttpResponse::Ok()
            .content_type("application/json")
            .body(json)),
        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            data: None,
            message: format!("Failed to serialize library: {}", e),
        })),
    }
}

pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/v1/chess960")
            .route("/position", web::get().to(get_position))
            .route("/fen/{number}", web::get().to(get_fen))
            .route("/verify", web::post().to(verify_fen))
            .route("/stats", web::get().to(get_stats))
            .route("/export", web::get().to(export_json)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_holds_all_960_positions() {
        assert_eq!(library().total_positions, 960);
        assert_eq!(library().positions.len(), 960);
    }

    #[test]
    fn known_fens_contains_position_one() {
        let fen = &library().positions[&1].fen;
        assert!(known_fens().contains(fen));
        assert!(!known_fens().contains("not a fen"));
    }

    #[test]
    fn king_distribution_covers_every_960_position() {
        let total: u32 = king_distribution().values().sum();
        assert_eq!(total, 960);
    }
}