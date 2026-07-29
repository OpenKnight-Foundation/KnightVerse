use actix_governor::{Governor, GovernorConfigBuilder};
use actix_web::{test, web, App, HttpResponse, Responder};
use std::thread;
use std::time::Duration;

async fn mock_handler() -> impl Responder {
    HttpResponse::Ok().body("OK")
}

#[actix_web::test]
async fn test_auth_rate_limiting() {
    // Configure Governor for Auth (Strict: 1 per sec, burst 2)
    // We use a small burst to easily trigger the limit
    let auth_governor_conf = GovernorConfigBuilder::default()
        .per_second(1)
        .burst_size(2)
        .use_headers()
        .finish()
        .unwrap();

    let app = test::init_service(
        App::new().service(
            web::scope("/v1/auth")
                .wrap(Governor::new(&auth_governor_conf))
                .route("/login", web::post().to(mock_handler)),
        ),
    )
    .await;

    // Request 1: Should pass
    let req = test::TestRequest::post()
        .uri("/v1/auth/login")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // Request 2: Should pass (burst allows 2)
    let req = test::TestRequest::post()
        .uri("/v1/auth/login")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // Request 3: Should fail (Rate limited)
    let req = test::TestRequest::post()
        .uri("/v1/auth/login")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 429); // Too Many Requests
}

#[actix_web::test]
async fn test_game_rate_limiting() {
    // Configure Governor for Games (Loose: 10 per sec, burst 20)
    // We'll set it tighter for testing: 2 per sec, burst 3
    let game_governor_conf = GovernorConfigBuilder::default()
        .per_second(2)
        .burst_size(3)
        .use_headers()
        .finish()
        .unwrap();

    let app = test::init_service(
        App::new().service(
            web::scope("/v1/games")
                .wrap(Governor::new(&game_governor_conf))
                .route("/create", web::post().to(mock_handler)),
        ),
    )
    .await;

    // Send 3 requests, all should pass
    for _ in 0..3 {
        let req = test::TestRequest::post()
            .uri("/v1/games/create")
            .peer_addr("127.0.0.1:12345".parse().unwrap())
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    // 4th request should fail
    let req = test::TestRequest::post()
        .uri("/v1/games/create")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 429);
}
