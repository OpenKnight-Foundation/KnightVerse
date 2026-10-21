use actix_web::{post, web, web::Json, HttpResponse};
use dto::{
    ai::{
        AiSuggestionRequest, AiSuggestionResponse, PositionAnalysisRequest,
        PositionAnalysisResponse,
    },
    responses::ValidationErrorResponse,
};
use serde_json::json;
use tracing::error;
use validator::Validate;

use crate::metrics::increment_ai_requests;
use service::engine_pool::{EnginePool, PoolStats};
use service::engine_service::EngineService;
use std::env;

/// Status returned when the engine pool is saturated. `429` (rather than 500)
/// tells the caller the request was shed on purpose and is worth retrying.
fn pool_exhausted_response() -> HttpResponse {
    HttpResponse::TooManyRequests().json(json!({
        "error": "engine pool saturated",
        "detail": "All engine slots are in use. Retry shortly.",
    }))
}

#[utoipa::path(
    post,
    path = "/v1/ai/suggest",
    request_body = AiSuggestionRequest,
    responses(
        (status = 200, description = "AI suggestion generated", body = AiSuggestionResponse),
        (status = 400, description = "Invalid FEN position", body = ValidationErrorResponse),
        (status = 429, description = "Engine pool saturated")
    ),
    security(
        ("jwt_auth" = [])
    ),
    tag = "AI"
)]
#[post("/suggest")]
pub async fn get_ai_suggestion(
    payload: Json<AiSuggestionRequest>,
    pool: web::Data<EnginePool>,
) -> HttpResponse {
    // Track AI request
    increment_ai_requests("suggestion");

    if let Err(errors) = payload.0.validate() {
        return validation_error_response(&errors);
    }

    // BE-137: a permit must be held for the whole search, otherwise this is
    // just a slower way of spawning one engine per request.
    let _permit = match pool.acquire().await {
        Ok(permit) => permit,
        Err(e) => {
            error!("AI suggestion shed by engine pool: {}", e);
            return pool_exhausted_response();
        }
    };

    let engine_path = env::var("ENGINE_PATH").unwrap_or_else(|_| "stockfish".to_string());
    let engine_service = EngineService::new(engine_path);

    let start_time = std::time::Instant::now();
    let result = engine_service
        .get_suggestion(&payload.0.fen, payload.0.depth, payload.0.time_limit_ms)
        .await;
    let elapsed = u32::try_from(start_time.elapsed().as_millis()).unwrap_or(u32::MAX);

    match result {
        Ok(result) => HttpResponse::Ok().json(AiSuggestionResponse {
            best_move: result.best_move,
            evaluation: result.evaluation.unwrap_or(0.0),
            depth: result.depth.unwrap_or(payload.0.depth.unwrap_or(10)),
            principal_variation: result.principal_variation,
            computation_time_ms: elapsed,
        }),
        Err(e) => {
            error!("Engine error in get_ai_suggestion: {}", e);
            HttpResponse::InternalServerError().json(json!({
                "error": "internal server error"
            }))
        }
    }
}

#[utoipa::path(
    post,
    path = "/v1/ai/analyze",
    request_body = PositionAnalysisRequest,
    responses(
        (status = 200, description = "Position analysis completed", body = PositionAnalysisResponse),
        (status = 400, description = "Invalid FEN position", body = ValidationErrorResponse),
        (status = 429, description = "Engine pool saturated")
    ),
    security(
        ("jwt_auth" = [])
    ),
    tag = "AI"
)]
#[post("/analyze")]
pub async fn analyze_position(
    payload: Json<PositionAnalysisRequest>,
    pool: web::Data<EnginePool>,
) -> HttpResponse {
    // Track AI request
    increment_ai_requests("analysis");

    if let Err(errors) = payload.0.validate() {
        return validation_error_response(&errors);
    }

    let _permit = match pool.acquire().await {
        Ok(permit) => permit,
        Err(e) => {
            error!("AI analysis shed by engine pool: {}", e);
            return pool_exhausted_response();
        }
    };

    let engine_path = env::var("ENGINE_PATH").unwrap_or_else(|_| "stockfish".to_string());
    let engine_service = EngineService::new(engine_path);

    match engine_service
        .analyze_position(&payload.0.fen, payload.0.depth)
        .await
    {
        Ok(result) => HttpResponse::Ok().json(PositionAnalysisResponse {
            evaluation: result.evaluation.unwrap_or(0.0),
            best_line: result.principal_variation,
            alternatives: vec![], // Engine trait could be extended for multi-pv
            position_type: "Analyzed by Engine".to_string(),
        }),
        Err(e) => {
            error!("Engine error in analyze_position: {}", e);
            HttpResponse::InternalServerError().json(json!({
                "error": "internal server error"
            }))
        }
    }
}

/// Deliberately mounted outside `/v1/ai`: pool saturation is exactly the
/// condition an operator needs to see, so this route stays unauthenticated and
/// un-rate-limited (it does no work beyond reading atomics).
#[utoipa::path(
    get,
    path = "/health/engine",
    responses((status = 200, description = "Engine pool capacity and admission counters")),
    tag = "Health"
)]
pub async fn ai_health(pool: web::Data<EnginePool>) -> HttpResponse {
    HttpResponse::Ok().json(AiHealthResponse {
        status: if pool.available() > 0 { "ok" } else { "saturated" },
        engine_path: env::var("ENGINE_PATH").unwrap_or_else(|_| "stockfish".to_string()),
        pool: pool.stats(),
    })
}

#[derive(serde::Serialize)]
pub struct AiHealthResponse {
    pub status: &'static str,
    pub engine_path: String,
    pub pool: PoolStats,
}

fn validation_error_response(
    errors: &validator::ValidationErrors,
) -> HttpResponse {
    let error_strings: Vec<String> = errors
        .field_errors()
        .values()
        .flat_map(|errs| {
            errs.iter()
                .map(|err| err.message.clone().unwrap_or_default().to_string())
        })
        .collect();

    HttpResponse::BadRequest().json(ValidationErrorResponse {
        error: "Invalid FEN position or parameters".to_string(),
        code: 400,
        details: Some(error_strings),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, web, App};
    use dto::ai::{AiSuggestionRequest, PositionAnalysisRequest};

    const START_FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

    /// The permit must be taken *before* any engine process is created, so a
    /// saturated pool has to shed the request instead of running the engine.
    #[actix_web::test]
    async fn suggest_returns_429_when_the_pool_is_saturated() {
        let pool = EnginePool::new(1, 30);
        let _held = pool.acquire().await.expect("occupy the only slot");

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(pool))
                .service(web::scope("/v1/ai").service(get_ai_suggestion)),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/v1/ai/suggest")
            .set_json(AiSuggestionRequest {
                fen: START_FEN.to_string(),
                depth: Some(1),
                time_limit_ms: None,
            })
            .to_request();
        let res = test::call_service(&app, req).await;
        assert_eq!(res.status(), 429);

        let body: serde_json::Value = test::read_body_json(res).await;
        assert_eq!(body["error"], "engine pool saturated");
    }

    #[actix_web::test]
    async fn analyze_returns_429_when_the_pool_is_saturated() {
        let pool = EnginePool::new(1, 30);
        let _held = pool.acquire().await.expect("occupy the only slot");

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(pool))
                .service(web::scope("/v1/ai").service(analyze_position)),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/v1/ai/analyze")
            .set_json(PositionAnalysisRequest {
                fen: START_FEN.to_string(),
                depth: 1,
            })
            .to_request();
        let res = test::call_service(&app, req).await;
        assert_eq!(res.status(), 429);
    }

    #[actix_web::test]
    async fn invalid_fen_is_rejected_before_the_pool_is_touched() {
        let pool = EnginePool::new(1, 30);
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(pool))
                .service(web::scope("/v1/ai").service(get_ai_suggestion)),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/v1/ai/suggest")
            .set_json(AiSuggestionRequest {
                fen: "not-a-fen".to_string(),
                depth: None,
                time_limit_ms: None,
            })
            .to_request();
        let res = test::call_service(&app, req).await;
        assert_eq!(res.status(), 400);
    }

    #[actix_web::test]
    async fn health_reports_capacity_and_in_flight() {
        let pool = EnginePool::new(2, 30);
        let held = pool.acquire().await.expect("take one slot");

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(pool))
                .route("/health/engine", web::get().to(ai_health)),
        )
        .await;

        let req = test::TestRequest::get().uri("/health/engine").to_request();
        let res = test::call_service(&app, req).await;
        assert_eq!(res.status(), 200);

        let body: serde_json::Value = test::read_body_json(res).await;
        assert_eq!(body["pool"]["capacity"], 2);
        assert_eq!(body["pool"]["in_flight"], 1);
        assert_eq!(body["status"], "ok");
        drop(held);
    }

    #[actix_web::test]
    async fn health_reports_saturated_when_every_slot_is_taken() {
        let pool = EnginePool::new(1, 30);
        let _held = pool.acquire().await.expect("take the only slot");

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(pool))
                .route("/health/engine", web::get().to(ai_health)),
        )
        .await;

        let req = test::TestRequest::get().uri("/v1/ai/health").to_request();
        let res = test::call_service(&app, req).await;
        let body: serde_json::Value = test::read_body_json(res).await;
        assert_eq!(body["status"], "saturated");
    }
}