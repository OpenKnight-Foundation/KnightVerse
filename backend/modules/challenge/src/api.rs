use crate::puzzle_validation::{
    ChessMove, Puzzle, PuzzleRewardToken, PuzzleSubmission, PuzzleValidationResult,
    PuzzleValidationService,
};
use actix_web::{web, Error, HttpMessage, HttpResponse, Result};
use security::jwt::Claims;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

/// API request/response types
#[derive(Debug, Deserialize)]
pub struct SubmitSolutionRequest {
    pub puzzle_id: Uuid,
    pub moves: Vec<ChessMove>,
}

#[derive(Debug, Serialize)]
pub struct SubmitSolutionResponse {
    pub success: bool,
    pub result: PuzzleValidationResult,
}

#[derive(Debug, Serialize)]
pub struct PuzzleListResponse {
    pub puzzles: Vec<Puzzle>,
}

#[derive(Debug, Deserialize)]
pub struct VerifyTokenRequest {
    pub token: String,
}

#[derive(Debug, Serialize)]
pub struct VerifyTokenResponse {
    pub success: bool,
    pub reward_token: Option<PuzzleRewardToken>,
    pub error: Option<String>,
}

/// Submit puzzle solution
pub async fn submit_solution(
    req: actix_web::HttpRequest,
    puzzle_service: web::Data<Arc<PuzzleValidationService>>,
    solution_request: web::Json<SubmitSolutionRequest>,
) -> Result<HttpResponse, Error> {
    // Extract user info from JWT claims
    let claims = req
        .extensions()
        .get::<Claims>()
        .ok_or_else(|| {
            actix_web::error::ErrorUnauthorized(
                "User not authenticated",
            )
        })?
        .clone();

    let user_id = claims.user_id;
    let username = claims.username;

    // Create puzzle submission
    let submission = PuzzleSubmission {
        puzzle_id: solution_request.puzzle_id,
        moves: solution_request.moves.clone(),
        user_id,
        username,
    };

    // Validate the solution
    match puzzle_service.validate_puzzle_solution(submission) {
        Ok(result) => Ok(HttpResponse::Ok().json(SubmitSolutionResponse {
            success: true,
            result,
        })),
        Err(e) => Ok(HttpResponse::BadRequest().json(SubmitSolutionResponse {
            success: false,
            result: PuzzleValidationResult {
                success: false,
                correct: false,
                message: format!("Validation error: {}", e),
                reward_token: None,
                reward_amount: None,
            },
        })),
    }
}

/// Get list of available puzzles
pub async fn get_puzzles(
    puzzle_service: web::Data<Arc<PuzzleValidationService>>,
) -> Result<HttpResponse, Error> {
    let puzzles = puzzle_service.get_puzzles();

    Ok(HttpResponse::Ok().json(PuzzleListResponse {
        puzzles: puzzles.clone(),
    }))
}

/// Get specific puzzle by ID
pub async fn get_puzzle_by_id(
    puzzle_service: web::Data<Arc<PuzzleValidationService>>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, Error> {
    let puzzle_id = path.into_inner();

    match puzzle_service.get_puzzle_by_id(&puzzle_id) {
        Ok(puzzle) => Ok(HttpResponse::Ok().json(puzzle)),
        Err(e) => Ok(HttpResponse::NotFound().json(serde_json::json!({
            "error": format!("Puzzle not found: {}", e)
        }))),
    }
}

/// Verify reward token
pub async fn verify_reward_token(
    puzzle_service: web::Data<Arc<PuzzleValidationService>>,
    token_request: web::Json<VerifyTokenRequest>,
) -> Result<HttpResponse, Error> {
    match puzzle_service.verify_reward_token(&token_request.token) {
        Ok(reward_token) => Ok(HttpResponse::Ok().json(VerifyTokenResponse {
            success: true,
            reward_token: Some(reward_token),
            error: None,
        })),
        Err(e) => Ok(HttpResponse::BadRequest().json(VerifyTokenResponse {
            success: false,
            reward_token: None,
            error: Some(format!("Token verification failed: {}", e)),
        })),
    }
}

/// Configure puzzle routes
pub fn configure_puzzle_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/v1/puzzles")
            .route("", web::get().to(get_puzzles))
            .route("/{puzzle_id}", web::get().to(get_puzzle_by_id))
            .route("/submit", web::post().to(submit_solution))
            .route("/verify-token", web::post().to(verify_reward_token)),
    );
}
