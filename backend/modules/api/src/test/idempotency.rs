use crate::idempotency::IdempotencyMiddleware;
use actix_web::{http::StatusCode, test, web, App, HttpMessage, HttpResponse};
use security::jwt::{Claims, TokenType};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use uuid::Uuid;

#[actix_web::test]
async fn test_idempotent_first_request_executes() {
    let call_count = Arc::new(AtomicUsize::new(0));
    let count_clone = call_count.clone();

    let app = test::init_service(App::new().wrap(IdempotencyMiddleware::in_memory()).route(
        "/api/v1/staking/stake",
        web::post().to(move || {
            let c = count_clone.clone();
            async move {
                c.fetch_add(1, Ordering::SeqCst);
                HttpResponse::Ok().json(serde_json::json!({
                    "status": "staked",
                    "amount": 100
                }))
            }
        }),
    ))
    .await;

    let req = test::TestRequest::post()
        .uri("/api/v1/staking/stake")
        .insert_header(("Idempotency-Key", "test-key-001"))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body = test::read_body(resp).await;
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "staked");
    assert_eq!(json["amount"], 100);
    assert_eq!(call_count.load(Ordering::SeqCst), 1);
}

#[actix_web::test]
async fn test_idempotent_duplicate_request_returns_cached_response() {
    let call_count = Arc::new(AtomicUsize::new(0));
    let count_clone = call_count.clone();

    let app = test::init_service(App::new().wrap(IdempotencyMiddleware::in_memory()).route(
        "/api/v1/tournaments/t-123/register",
        web::post().to(move || {
            let c = count_clone.clone();
            async move {
                let count = c.fetch_add(1, Ordering::SeqCst);
                HttpResponse::Created().json(serde_json::json!({
                    "status": "registered",
                    "execution_id": count
                }))
            }
        }),
    ))
    .await;

    // First request
    let req1 = test::TestRequest::post()
        .uri("/api/v1/tournaments/t-123/register")
        .insert_header(("Idempotency-Key", "unique-tourney-key"))
        .to_request();

    let resp1 = test::call_service(&app, req1).await;
    assert_eq!(resp1.status(), StatusCode::CREATED);

    let body1 = test::read_body(resp1).await;
    let json1: serde_json::Value = serde_json::from_slice(&body1).unwrap();
    assert_eq!(json1["status"], "registered");
    assert_eq!(json1["execution_id"], 0);
    assert_eq!(call_count.load(Ordering::SeqCst), 1);

    // Second request with SAME idempotency key
    let req2 = test::TestRequest::post()
        .uri("/api/v1/tournaments/t-123/register")
        .insert_header(("Idempotency-Key", "unique-tourney-key"))
        .to_request();

    let resp2 = test::call_service(&app, req2).await;
    assert_eq!(resp2.status(), StatusCode::CREATED);
    assert_eq!(resp2.headers().get("Idempotency-Replayed").unwrap(), "true");

    let body2 = test::read_body(resp2).await;
    let json2: serde_json::Value = serde_json::from_slice(&body2).unwrap();
    assert_eq!(json2["status"], "registered");
    assert_eq!(json2["execution_id"], 0); // Exact same response body replayed

    // Handler must NOT have been called a second time
    assert_eq!(call_count.load(Ordering::SeqCst), 1);
}

#[actix_web::test]
async fn test_idempotent_concurrent_request_returns_409_conflict() {
    let middleware = IdempotencyMiddleware::in_memory();

    // Pre-populate storage with a PENDING lock
    let pending_key = "idempotency:anon:pending-concurrent-key";
    middleware.storage.try_lock(pending_key, 120).await;

    let app =
        test::init_service(App::new().wrap(middleware).route(
            "/api/v1/escrow/release",
            web::post().to(|| async {
                HttpResponse::Ok().json(serde_json::json!({"status": "released"}))
            }),
        ))
        .await;

    // Concurrent request arriving while key is still PENDING
    let req = test::TestRequest::post()
        .uri("/api/v1/escrow/release")
        .insert_header(("Idempotency-Key", "pending-concurrent-key"))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::CONFLICT);

    let body = test::read_body(resp).await;
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["code"], "OPERATION_IN_PROGRESS");
    assert_eq!(json["error"], "Conflict");
}

#[actix_web::test]
async fn test_idempotency_keys_scoped_per_user() {
    let call_count = Arc::new(AtomicUsize::new(0));
    let count_clone = call_count.clone();

    let app = test::init_service(App::new().wrap(IdempotencyMiddleware::in_memory()).route(
        "/api/v1/staking/deposit",
        web::post().to(move || {
            let c = count_clone.clone();
            async move {
                let count = c.fetch_add(1, Ordering::SeqCst);
                HttpResponse::Ok().json(serde_json::json!({
                    "status": "deposited",
                    "execution_count": count
                }))
            }
        }),
    ))
    .await;

    // User A (user_id: 101)
    let claims_user_a = Claims {
        sub: "101".to_string(),
        user_id: 101,
        player_id: Uuid::new_v4(),
        username: "alice".to_string(),
        exp: 9999999999,
        iat: 1000000000,
        jti: None,
        token_type: TokenType::Access,
    };

    let req_user_a = test::TestRequest::post()
        .uri("/api/v1/staking/deposit")
        .insert_header(("Idempotency-Key", "shared-client-key-100"))
        .to_request();
    req_user_a.extensions_mut().insert(claims_user_a);

    let resp_a = test::call_service(&app, req_user_a).await;
    assert_eq!(resp_a.status(), StatusCode::OK);
    assert_eq!(call_count.load(Ordering::SeqCst), 1);

    // User B (user_id: 202) using the SAME idempotency key
    let claims_user_b = Claims {
        sub: "202".to_string(),
        user_id: 202,
        player_id: Uuid::new_v4(),
        username: "bob".to_string(),
        exp: 9999999999,
        iat: 1000000000,
        jti: None,
        token_type: TokenType::Access,
    };

    let req_user_b = test::TestRequest::post()
        .uri("/api/v1/staking/deposit")
        .insert_header(("Idempotency-Key", "shared-client-key-100"))
        .to_request();
    req_user_b.extensions_mut().insert(claims_user_b);

    let resp_b = test::call_service(&app, req_user_b).await;
    assert_eq!(resp_b.status(), StatusCode::OK);

    // Because users are isolated, handler was called for User B too!
    assert_eq!(call_count.load(Ordering::SeqCst), 2);
}

#[actix_web::test]
async fn test_server_error_5xx_not_cached_as_completed() {
    let call_count = Arc::new(AtomicUsize::new(0));
    let count_clone = call_count.clone();

    let app = test::init_service(App::new().wrap(IdempotencyMiddleware::in_memory()).route(
        "/api/v1/escrow/transfer",
        web::post().to(move || {
            let c = count_clone.clone();
            async move {
                let count = c.fetch_add(1, Ordering::SeqCst);
                if count == 0 {
                    // First attempt fails with 500
                    HttpResponse::InternalServerError().json(serde_json::json!({
                        "error": "Database temporarily unavailable"
                    }))
                } else {
                    // Retry succeeds
                    HttpResponse::Ok().json(serde_json::json!({
                        "status": "transfer_complete"
                    }))
                }
            }
        }),
    ))
    .await;

    // First request returns 500
    let req1 = test::TestRequest::post()
        .uri("/api/v1/escrow/transfer")
        .insert_header(("Idempotency-Key", "retryable-key-500"))
        .to_request();

    let resp1 = test::call_service(&app, req1).await;
    assert_eq!(resp1.status(), StatusCode::INTERNAL_SERVER_ERROR);

    // Second request with SAME key should re-execute and NOT be blocked by 409 or cached 500
    let req2 = test::TestRequest::post()
        .uri("/api/v1/escrow/transfer")
        .insert_header(("Idempotency-Key", "retryable-key-500"))
        .to_request();

    let resp2 = test::call_service(&app, req2).await;
    assert_eq!(resp2.status(), StatusCode::OK);

    let body2 = test::read_body(resp2).await;
    let json2: serde_json::Value = serde_json::from_slice(&body2).unwrap();
    assert_eq!(json2["status"], "transfer_complete");
    assert_eq!(call_count.load(Ordering::SeqCst), 2);
}

#[actix_web::test]
async fn test_get_requests_bypass_idempotency_middleware() {
    let call_count = Arc::new(AtomicUsize::new(0));
    let count_clone = call_count.clone();

    let app = test::init_service(App::new().wrap(IdempotencyMiddleware::in_memory()).route(
        "/api/v1/staking/info",
        web::get().to(move || {
            let c = count_clone.clone();
            async move {
                let count = c.fetch_add(1, Ordering::SeqCst);
                HttpResponse::Ok().json(serde_json::json!({
                    "pool_size": 1000 + count
                }))
            }
        }),
    ))
    .await;

    let req1 = test::TestRequest::get()
        .uri("/api/v1/staking/info")
        .insert_header(("Idempotency-Key", "ignored-on-get"))
        .to_request();
    let resp1 = test::call_service(&app, req1).await;
    assert_eq!(resp1.status(), StatusCode::OK);

    let req2 = test::TestRequest::get()
        .uri("/api/v1/staking/info")
        .insert_header(("Idempotency-Key", "ignored-on-get"))
        .to_request();
    let resp2 = test::call_service(&app, req2).await;
    assert_eq!(resp2.status(), StatusCode::OK);

    // Both GETs executed
    assert_eq!(call_count.load(Ordering::SeqCst), 2);
}

#[actix_web::test]
async fn test_x_idempotency_key_header_support() {
    let call_count = Arc::new(AtomicUsize::new(0));
    let count_clone = call_count.clone();

    let app = test::init_service(App::new().wrap(IdempotencyMiddleware::in_memory()).route(
        "/api/v1/staking/claim",
        web::put().to(move || {
            let c = count_clone.clone();
            async move {
                c.fetch_add(1, Ordering::SeqCst);
                HttpResponse::Ok().json(serde_json::json!({"claimed": true}))
            }
        }),
    ))
    .await;

    let req1 = test::TestRequest::put()
        .uri("/api/v1/staking/claim")
        .insert_header(("X-Idempotency-Key", "x-prefix-key-999"))
        .to_request();
    let resp1 = test::call_service(&app, req1).await;
    assert_eq!(resp1.status(), StatusCode::OK);

    let req2 = test::TestRequest::put()
        .uri("/api/v1/staking/claim")
        .insert_header(("X-Idempotency-Key", "x-prefix-key-999"))
        .to_request();
    let resp2 = test::call_service(&app, req2).await;
    assert_eq!(resp2.status(), StatusCode::OK);
    assert_eq!(resp2.headers().get("Idempotency-Replayed").unwrap(), "true");

    assert_eq!(call_count.load(Ordering::SeqCst), 1);
}
