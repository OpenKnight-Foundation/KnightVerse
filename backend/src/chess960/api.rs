use super::generator::Chess960Generator;
use super::models::{Chess960Library, Chess960Position};
use actix_web::{web, HttpResponse, Result};
use lazy_static::lazy_static;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

lazy_static::lazy_static! {
    static ref CHESS960_LIBRARY: Arc<Chess960Library> = {
        Arc::new(Chess960Generator::generate_all_positions())
    };
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

#[derive(Deserialize)]
pub struct FenVerifyRequest {
    pub fen: String,
}

#[derive(Serialize)]
pub struct StatsResponse {
    pub total_positions: u16,
    pub king_distribution: std::collections::HashMap<usize, u32>,
    pub version: String,
}

pub async fn get_position(query: web::Query<PositionQuery>) -> Result<HttpResponse> {
    let library = &*CHESS960_LIBRARY;

    let position = if query.random.unwrap_or(false) {
        // Get random position
        let mut rng = rand::thread_rng();
        let random_id = rng.gen_range(1..=960);
        library.positions.get(&random_id).cloned()
    } else if let Some(number) = query.number {
        library.positions.get(&number).cloned()
    } else {
        return Ok(HttpResponse::BadRequest().json(ApiResponse {
            success: false,
            data: None::<Chess960Position>,
            message: "Specify 'number' parameter or set 'random=true'".to_string(),
        }));
    };

    match position {
        Some(pos) => Ok(HttpResponse::Ok().json(ApiResponse {
            success: true,
            data: Some(pos),
            message: "Position retrieved successfully".to_string(),
        })),
        None => Ok(HttpResponse::NotFound().json(ApiResponse {
            success: false,
            data: None::<Chess960Position>,
            message: "Position not found".to_string(),
        })),
    }
}

pub async fn get_fen(path: web::Path<u16>) -> Result<HttpResponse> {
    let number = path.into_inner();
    let library = &*CHESS960_LIBRARY;

    match library.positions.get(&number) {
        Some(position) => Ok(HttpResponse::Ok().json(ApiResponse {
            success: true,
            data: Some(&position.fen),
            message: format!("FEN for position {} retrieved", number),
        })),
        None => Ok(HttpResponse::NotFound().json(ApiResponse {
            success: false,
            data: None::<String>,
            message: format!("Position {} not found", number),
        })),
    }
}
lazy_static! {
    static ref CHESS960_FENS: std::collections::HashSet<String> = {
        CHESS960_LIBRARY
            .positions
            .values()
            .map(|pos| pos.fen.clone())
            .collect()
    };
}

pub async fn verify_fen(req: web::Json<FenVerifyRequest>) -> Result<HttpResponse> {
    let is_valid = CHESS960_FENS.contains(&req.fen);

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
lazy_static! {
    static ref KING_DISTRIBUTION: std::collections::HashMap<usize, u32> = {
        let mut distribution = std::collections::HashMap::new();
        for position in CHESS960_LIBRARY.positions.values() {
            let count = distribution.entry(position.white_king_pos).or_insert(0);
            *count += 1;
        }
        distribution
    };
}

pub async fn get_stats() -> Result<HttpResponse> {
    let library = &*CHESS960_LIBRARY;

    let stats = StatsResponse {
        total_positions: library.total_positions,
        king_distribution: KING_DISTRIBUTION.clone(),
        version: library.metadata.version.clone(),
    };

    Ok(HttpResponse::Ok().json(ApiResponse {
        success: true,
        data: Some(stats),
        message: "Statistics retrieved successfully".to_string(),
    }))
}

pub async fn export_json() -> Result<HttpResponse> {
    let library = &*CHESS960_LIBRARY;

    match serde_json::to_string_pretty(&**library) {
        Ok(json) => Ok(HttpResponse::Ok()
            .content_type("application/json")
            .body(json)),
        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse {
            success: false,
            data: None::<String>,
            message: format!("Failed to serialize library: {}", e),
        })),
    }
}

pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/chess960")
            .route("/position", web::get().to(get_position))
            .route("/fen/{number}", web::get().to(get_fen))
            .route("/verify", web::post().to(verify_fen))
            .route("/stats", web::get().to(get_stats))
            .route("/export", web::get().to(export_json)),
    );
}
