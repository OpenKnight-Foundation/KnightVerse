
use crate::puzzle_validation::{
    ChessMove, Puzzle, PuzzleRewardToken, PuzzleSubmission, PuzzleValidationResult,
    PuzzleValidationService,
};
use actix_web::{web, Error, HttpMessage, HttpRequest, HttpResponse, Result};
use security::jwt::Claims;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

/// Request payload for submitting a puzzle solution.
#[derive(Debug, Deserialize)]
pub struct SubmitSolutionRequest {
    pub puzzle_id: Uuid,
    pub moves: Vec<ChessMove>,
}

/// Response returned after submitting a puzzle solution.
#[derive(Debug, Serialize)]
pub struct SubmitSolutionResponse {
    pub success: bool,
    pub result: PuzzleValidationResult,
}

/// Response containing the available puzzles.
#[derive(Debug, Serialize)]
pub struct PuzzleListResponse {
    pub puzzles: Vec<Puzzle>,
}

/// Request payload for verifying a reward token.
#[derive(Debug, Deserialize)]
pub struct VerifyTokenRequest {
    pub token: String,
}

/// Response returned after reward token verification.
#[derive(Debug, Serialize)]
pub struct VerifyTokenResponse {
    pub success: bool,
    pub reward_token: Option<PuzzleRewardToken>,
    pub error: Option<String>,
}

/// Extract authenticated user claims from the request.
///
/// Returns an unauthorized error when JWT claims are unavailable.
fn extract_claims(req: &HttpRequest) -> Result<Claims, Error> {
    req.extensions()
        .get::<Claims>()
        .cloned()
        .ok_or_else(|| {
            actix_web::error::ErrorUnauthorized("User not authenticated")
        })
}

/// Submit and validate a user's puzzle solution.
///
/// The authenticated user's identity is extracted from JWT claims
/// and attached to the puzzle submission.
pub async fn submit_solution(
    req: HttpRequest,
    puzzle_service: web::Data<Arc<PuzzleValidationService>>,
    solution_request: web::Json<SubmitSolutionRequest>,
) -> Result<HttpResponse, Error> {
    let claims = extract_claims(&req)?;

    let submission = PuzzleSubmission {
        puzzle_id: solution_request.puzzle_id,
        moves: solution_request.moves.clone(),
        user_id: claims.user_id,
        username: claims.username,
    };

    match puzzle_service.validate_puzzle_solution(submission) {
        Ok(result) => Ok(HttpResponse::Ok().json(SubmitSolutionResponse {
            success: true,
            result,
        })),

        Err(error) => Ok(HttpResponse::BadRequest().json(
            SubmitSolutionResponse {
                success: false,
                result: PuzzleValidationResult {
                    success: false,
                    correct: false,
                    message: format!("Validation error: {error}"),
                    reward_token: None,
                    reward_amount: None,
                },
            },
        )),
    }
}

/// Retrieve all available puzzles.
pub async fn get_puzzles(
    puzzle_service: web::Data<Arc<PuzzleValidationService>>,
) -> Result<HttpResponse, Error> {
    let puzzles = puzzle_service.get_puzzles();

    Ok(HttpResponse::Ok().json(PuzzleListResponse {
        puzzles: puzzles.clone(),
    }))
}

/// Retrieve a puzzle using its unique identifier.
///
/// Returns 404 when the requested puzzle does not exist.
pub async fn get_puzzle_by_id(
    puzzle_service: web::Data<Arc<PuzzleValidationService>>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, Error> {
    let puzzle_id = path.into_inner();

    match puzzle_service.get_puzzle_by_id(&puzzle_id) {
        Ok(puzzle) => Ok(HttpResponse::Ok().json(puzzle)),

        Err(error) => Ok(HttpResponse::NotFound().json(serde_json::json!({
            "error": format!("Puzzle not found: {error}")
        }))),
    }
}

/// Verify a puzzle reward token.
///
/// Returns the verified reward token when valid.
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

        Err(error) => Ok(HttpResponse::BadRequest().json(VerifyTokenResponse {
            success: false,
            reward_token: None,
            error: Some(format!("Token verification failed: {error}")),
        })),
    }
}

/// Register all puzzle-related API routes.
pub fn configure_puzzle_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/v1/puzzles")
            .route("", web::get().to(get_puzzles))
            .route("/{puzzle_id}", web::get().to(get_puzzle_by_id))
            .route("/submit", web::post().to(submit_solution))
            .route("/verify-token", web::post().to(verify_reward_token)),
    );
}
