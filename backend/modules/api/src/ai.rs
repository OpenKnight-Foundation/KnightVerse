use actix_web::{
    HttpResponse, post,
    web::Json,
};
use dto::{
    ai::{AiSuggestionRequest, AiSuggestionResponse, PositionAnalysisRequest, PositionAnalysisResponse},
    responses::ValidationErrorResponse,
};
use error::error::ApiError;
use serde_json::json;
use validator::Validate;

use service::engine_service::EngineService;
use std::env;
use crate::metrics::increment_ai_requests;

#[utoipa::path(
    post,
    path = "/v1/ai/suggest",
    request_body = AiSuggestionRequest,
    responses(
        (status = 200, description = "AI suggestion generated", body = AiSuggestionResponse),
        (status = 400, description = "Invalid FEN position", body = ValidationErrorResponse)
    ),
    security(
        ("jwt_auth" = [])
    ),
    tag = "AI"
)]
#[post("/suggest")]
pub async fn get_ai_suggestion(payload: Json<AiSuggestionRequest>) -> HttpResponse {
    // Track AI request
    increment_ai_requests("suggestion");
    
    match payload.0.validate() {
        Ok(_) => {
            let engine_path = env::var("ENGINE_PATH").unwrap_or_else(|_| "stockfish".to_string());
            let engine_service = EngineService::new(engine_path);
            
            let start_time = std::time::Instant::now();
            let result = engine_service.get_suggestion(
                &payload.0.fen,
                payload.0.depth,
                payload.0.time_limit_ms
            ).await;
            let elapsed = u32::try_from(start_time.elapsed().as_millis()).unwrap_or(u32::MAX);
            
            match result {
                Ok(result) => {
                    HttpResponse::Ok().json(AiSuggestionResponse {
                        best_move: result.best_move,
                        evaluation: result.evaluation.unwrap_or(0.0),
                        depth: result.depth.unwrap_or(payload.0.depth.unwrap_or(10)),
                        principal_variation: result.principal_variation,
                        computation_time_ms: elapsed,
                    })
                }
                Err(e) => {
                    log::error!("Engine error in get_ai_suggestion: {}", e);
                    HttpResponse::InternalServerError().json(json!({
                        "error": "internal server error"
                    }))
                }
            }
        }
        Err(errors) => {
            let error_strings: Vec<String> = errors
                .field_errors()
                .iter()
                .flat_map(|(_, errs)| errs.iter().map(|err| err.message.clone().unwrap_or_default().to_string()))
                .collect();
            
            HttpResponse::BadRequest().json(ValidationErrorResponse {
                error: "Invalid FEN position or parameters".to_string(),
                code: 400,
                details: Some(error_strings)
            })
        }
    }
}

#[utoipa::path(
    post,
    path = "/v1/ai/analyze",
    request_body = PositionAnalysisRequest,
    responses(
        (status = 200, description = "Position analysis completed", body = PositionAnalysisResponse),
        (status = 400, description = "Invalid FEN position", body = ValidationErrorResponse)
    ),
    security(
        ("jwt_auth" = [])
    ),
    tag = "AI"
)]
#[post("/analyze")]
pub async fn analyze_position(payload: Json<PositionAnalysisRequest>) -> HttpResponse {
    // Track AI request
    increment_ai_requests("analysis");
    
    match payload.0.validate() {
        Ok(_) => {
            let engine_path = env::var("ENGINE_PATH").unwrap_or_else(|_| "stockfish".to_string());
            let engine_service = EngineService::new(engine_path);
            
            match engine_service.analyze_position(&payload.0.fen, payload.0.depth).await {
                Ok(result) => {
                    HttpResponse::Ok().json(PositionAnalysisResponse {
                        evaluation: result.evaluation.unwrap_or(0.0),
                        best_line: result.principal_variation,
                        alternatives: vec![], // Engine trait could be extended for multi-pv
                        position_type: "Analyzed by Engine".to_string(),
                    })
                }
                Err(e) => {
                    log::error!("Engine error in analyze_position: {}", e);
                    HttpResponse::InternalServerError().json(json!({
                        "error": "internal server error"
                    }))
                }
            }
        }
        Err(errors) => {
            let error_strings: Vec<String> = errors
                .field_errors()
                .iter()
                .flat_map(|(_, errs)| errs.iter().map(|err| err.message.clone().unwrap_or_default().to_string()))
                .collect();
            
            HttpResponse::BadRequest().json(ValidationErrorResponse {
                error: "Invalid FEN position or parameters".to_string(),
                code: 400,
                details: Some(error_strings)
            })
        }
    }
}
