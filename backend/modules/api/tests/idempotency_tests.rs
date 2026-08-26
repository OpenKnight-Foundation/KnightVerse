//! BE-46: Integration tests for the Idempotency-Key middleware on payment, staking & tournament endpoints

use actix_web::{http::StatusCode, web, App, HttpResponse};
use api::idempotency::IdempotencyMiddleware;
use security::jwt::{JwtAuthMiddleware, JwtService};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use uuid::Uuid;

const JWT_SECRET: &str = "test_secret_for_idempotency_integration";

fn make_access_token(user_id: i32, username: &str) -> String {
    let jwt_service = JwtService::new(JWT_SECRET.to_string(), 3600);
    jwt_service
        .generate_token(user_id, username, Uuid::new_v4())
        .expect("failed to mint test JWT")
}

#[actix_web::test]
async fn test_tournament_registration_idempotency_flow() {
    let execution_counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = execution_counter.clone();

    let srv = actix_test::start(move || {
        let c = counter_clone.clone();
        App::new()
            .wrap(IdempotencyMiddleware::in_memory())
            .wrap(JwtAuthMiddleware::new(JWT_SECRET.to_string(), 3600))
            .route(
                "/api/v1/tournaments/{id}/register",
                web::post().to(move |path: web::Path<String>| {
                    let c = c.clone();
                    async move {
                        let exec_id = c.fetch_add(1, Ordering::SeqCst);
                        HttpResponse::Ok().json(serde_json::json!({
                            "status": "registered",
                            "tournament_id": path.into_inner(),
                            "execution_seq": exec_id
                        }))
                    }
                }),
            )
    });

    let client = awc::Client::default();
    let token = make_access_token(42, "grandmaster");
    let target_url = srv.url("/api/v1/tournaments/t-swiss-2026/register");

    // 1. Initial request with Idempotency-Key
    let mut resp1 = client
        .post(&target_url)
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .insert_header(("Idempotency-Key", "tourney-reg-key-42"))
        .send()
        .await
        .expect("Failed to send first request");

    assert_eq!(resp1.status(), StatusCode::OK);
    let body1 = resp1.body().await.expect("Failed to read body");
    let json1: serde_json::Value = serde_json::from_slice(&body1).expect("Invalid JSON");
    assert_eq!(json1["status"], "registered");
    assert_eq!(json1["tournament_id"], "t-swiss-2026");
    assert_eq!(json1["execution_seq"], 0);
    assert_eq!(execution_counter.load(Ordering::SeqCst), 1);

    // 2. Immediate duplicate request with same Idempotency-Key
    let mut resp2 = client
        .post(&target_url)
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .insert_header(("Idempotency-Key", "tourney-reg-key-42"))
        .send()
        .await
        .expect("Failed to send duplicate request");

    assert_eq!(resp2.status(), StatusCode::OK);
    assert_eq!(
        resp2
            .headers()
            .get("Idempotency-Replayed")
            .unwrap()
            .to_str()
            .unwrap(),
        "true"
    );

    let body2 = resp2.body().await.expect("Failed to read body");
    let json2: serde_json::Value = serde_json::from_slice(&body2).expect("Invalid JSON");
    assert_eq!(json2["status"], "registered");
    assert_eq!(json2["execution_seq"], 0); // Exact replayed response body

    // Underlying logic was NOT re-executed
    assert_eq!(execution_counter.load(Ordering::SeqCst), 1);
}

#[actix_web::test]
async fn test_staking_and_escrow_multi_tenant_isolation() {
    let execution_counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = execution_counter.clone();

    let srv = actix_test::start(move || {
        let c = counter_clone.clone();
        App::new()
            .wrap(IdempotencyMiddleware::in_memory())
            .wrap(JwtAuthMiddleware::new(JWT_SECRET.to_string(), 3600))
            .route(
                "/api/v1/staking/deposit",
                web::post().to(move || {
                    let c = c.clone();
                    async move {
                        let exec_id = c.fetch_add(1, Ordering::SeqCst);
                        HttpResponse::Ok().json(serde_json::json!({
                            "status": "deposit_success",
                            "execution_count": exec_id
                        }))
                    }
                }),
            )
    });

    let client = awc::Client::default();
    let token_user_1 = make_access_token(1001, "player_one");
    let token_user_2 = make_access_token(2002, "player_two");
    let target_url = srv.url("/api/v1/staking/deposit");

    // User 1 executes with key "stake-key-abc"
    let resp1 = client
        .post(&target_url)
        .insert_header(("Authorization", format!("Bearer {}", token_user_1)))
        .insert_header(("Idempotency-Key", "stake-key-abc"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp1.status(), StatusCode::OK);
    assert_eq!(execution_counter.load(Ordering::SeqCst), 1);

    // User 2 executes with the IDENTICAL key "stake-key-abc"
    let resp2 = client
        .post(&target_url)
        .insert_header(("Authorization", format!("Bearer {}", token_user_2)))
        .insert_header(("Idempotency-Key", "stake-key-abc"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp2.status(), StatusCode::OK);

    // Because idempotency keys are isolated per authenticated user ID, User 2 also executed
    assert_eq!(execution_counter.load(Ordering::SeqCst), 2);
}

#[actix_web::test]
async fn test_5xx_errors_release_lock_for_retry() {
    let attempts = Arc::new(AtomicUsize::new(0));
    let attempts_clone = attempts.clone();

    let srv = actix_test::start(move || {
        let a = attempts_clone.clone();
        App::new()
            .wrap(IdempotencyMiddleware::in_memory())
            .route(
                "/api/v1/escrow/transfer",
                web::post().to(move || {
                    let a = a.clone();
                    async move {
                        let count = a.fetch_add(1, Ordering::SeqCst);
                        if count == 0 {
                            HttpResponse::InternalServerError().json(serde_json::json!({
                                "error": "Temporary network timeout"
                            }))
                        } else {
                            HttpResponse::Ok().json(serde_json::json!({
                                "status": "transfer_success"
                            }))
                        }
                    }
                }),
            )
    });

    let client = awc::Client::default();
    let target_url = srv.url("/api/v1/escrow/transfer");

    // First attempt fails with 500
    let resp1 = client
        .post(&target_url)
        .insert_header(("Idempotency-Key", "escrow-retry-key"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp1.status(), StatusCode::INTERNAL_SERVER_ERROR);

    // Retry with SAME idempotency key must not be blocked by 409 or cached 500
    let mut resp2 = client
        .post(&target_url)
        .insert_header(("Idempotency-Key", "escrow-retry-key"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp2.status(), StatusCode::OK);

    let body2 = resp2.body().await.unwrap();
    let json2: serde_json::Value = serde_json::from_slice(&body2).unwrap();
    assert_eq!(json2["status"], "transfer_success");
    assert_eq!(attempts.load(Ordering::SeqCst), 2);
}
