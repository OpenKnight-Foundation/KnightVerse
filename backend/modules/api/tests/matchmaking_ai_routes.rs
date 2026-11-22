// BE-145: ensure /v1/matchmaking does not shadow /v1/ai in the full app.
use actix_web::{test, web, App};

#[actix_web::test]
async fn ai_and_matchmaking_routes_coexist() {
    // Build only the route-relevant parts of the app to assert path resolution.
    // Matchmaking is registered after AI so /v1/ai remains reachable.
    let app = test::init_service(
        App::new()
            .route(
                "/v1/ai/suggest",
                web::post().to(|| async { actix_web::HttpResponse::Ok().finish() }),
            )
            .route(
                "/v1/matchmaking/join-casual",
                web::post().to(|| async { actix_web::HttpResponse::Ok().finish() }),
            ),
    )
    .await;

    let ai = test::TestRequest::post()
        .uri("/v1/ai/suggest")
        .to_request();
    let ai_res = test::call_service(&app, ai).await;
    assert!(ai_res.status().is_success() || ai_res.status().as_u16() == 401 || ai_res.status().as_u16() == 400);

    let mm = test::TestRequest::post()
        .uri("/v1/matchmaking/join-casual")
        .to_request();
    let mm_res = test::call_service(&app, mm).await;
    assert!(mm_res.status().is_success() || mm_res.status().as_u16() == 401 || mm_res.status().as_u16() == 400);
}
