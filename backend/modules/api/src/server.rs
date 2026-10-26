// src/server.rs

use crate::admin::{list_actions, record_action};
use crate::ai::{ai_health, analyze_position, get_ai_suggestion};
use crate::auth::{issue_ws_ticket, login, logout, logout_all, refresh, register};
use crate::config::AppConfig;
use service::engine_pool::EnginePool;
use crate::games::{
    abandon_game, complete_game, create_game, get_game, import_game, join_game, list_games,
    make_move,
};
use crate::chess960::configure_routes as configure_chess960_routes;
use crate::idempotency::IdempotencyMiddleware;
use crate::lc0::{lc0_analyze, lc0_health};
use crate::players::{
    add_player, delete_player, export_player_data, find_player_by_id, get_preferences,
    put_preferences, update_player,
};
use crate::rate_limiter::RedisRateLimiter;
use crate::request_id::RequestIdMiddleware;
use crate::ws::{ws_route, ConnectionStateTracker, LobbyState};
use actix::Actor;
use actix_cors::Cors;
use actix_governor::{Governor, GovernorConfigBuilder};
use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use challenge::api::configure_puzzle_routes;
use challenge::puzzle_validation::PuzzleValidationService;
use db::DbPool;
use engine::Lc0Orchestrator;
use dotenv::dotenv;
use matchmaking::redis::{create_redis_pool, test_redis_connection};
use matchmaking::MatchmakingService;
use migration::Migrator;
use migration::MigratorTrait;
use security::jwt::{JwtAuthMiddleware, JwtService};
use std::env;
use std::sync::Arc;
use tracing::{error, info, warn};
use tracing_actix_web::TracingLogger;
use utoipa::OpenApi;
use utoipa_redoc::{Redoc, Servable};
use utoipa_swagger_ui::SwaggerUi;

use crate::openapi::ApiDoc;

/// Liveness endpoint — reports only that the process is up and serving HTTP.
/// Dependency checks live in [`readiness`] so a flaky DB/Redis doesn't get the
/// pod restarted, only taken out of rotation.
async fn health() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({"status": "ok"}))
}

/// Upper bound for each dependency probe in `/health/ready` (BE-138).
const READINESS_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

/// Ping the primary database, failing if it errors or exceeds `timeout`.
async fn check_db(
    conn: &sea_orm::DatabaseConnection,
    timeout: std::time::Duration,
) -> Result<(), String> {
    match tokio::time::timeout(timeout, conn.ping()).await {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => Err(e.to_string()),
        Err(_) => Err(format!("timed out after {}ms", timeout.as_millis())),
    }
}

/// Acquire a Redis connection and `PING` it, failing if either step errors or
/// the whole probe exceeds `timeout`.
async fn check_redis(
    pool: &deadpool_redis::Pool,
    timeout: std::time::Duration,
) -> Result<(), String> {
    let probe = async {
        let mut conn = pool.get().await.map_err(|e| e.to_string())?;
        redis::cmd("PING")
            .query_async::<_, String>(&mut conn)
            .await
            .map(|_| ())
            .map_err(|e| e.to_string())
    };
    match tokio::time::timeout(timeout, probe).await {
        Ok(result) => result,
        Err(_) => Err(format!("timed out after {}ms", timeout.as_millis())),
    }
}

async fn readiness_response(
    db_pool: &DbPool,
    redis_pool: &deadpool_redis::Pool,
    timeout: std::time::Duration,
) -> HttpResponse {
    let (db, redis) = tokio::join!(
        check_db(db_pool.primary(), timeout),
        check_redis(redis_pool, timeout)
    );
    let ready = db.is_ok() && redis.is_ok();
    let check_json = |r: &Result<(), String>| match r {
        Ok(()) => serde_json::json!({"status": "ok"}),
        Err(e) => serde_json::json!({"status": "error", "error": e}),
    };
    let body = serde_json::json!({
        "status": if ready { "ok" } else { "error" },
        "checks": {
            "database": check_json(&db),
            "redis": check_json(&redis),
        }
    });
    if ready {
        HttpResponse::Ok().json(body)
    } else {
        HttpResponse::ServiceUnavailable().json(body)
    }
}

/// Readiness endpoint — pings the primary DB and Redis with a short timeout and
/// returns 503 if either is unavailable, so orchestrators stop routing traffic.
async fn readiness(
    db_pool: web::Data<DbPool>,
    redis_pool: web::Data<deadpool_redis::Pool>,
) -> impl Responder {
    readiness_response(&db_pool, &redis_pool, READINESS_TIMEOUT).await
}

/// Redis health check endpoint
async fn health_redis(redis_pool: web::Data<deadpool_redis::Pool>) -> impl Responder {
    let start = std::time::Instant::now();
    match redis_pool.get().await {
        Ok(mut conn) => {
            let ping_result: Result<String, _> = redis::cmd("PING").query_async(&mut conn).await;
            let latency_ms = start.elapsed().as_millis() as u64;
            match ping_result {
                Ok(_) => HttpResponse::Ok().json(serde_json::json!({
                    "status": "ok",
                    "redis": "connected",
                    "latency_ms": latency_ms
                })),
                Err(e) => HttpResponse::ServiceUnavailable().json(serde_json::json!({
                    "status": "error",
                    "redis": "ping_failed",
                    "error": e.to_string()
                })),
            }
        }
        Err(e) => HttpResponse::ServiceUnavailable().json(serde_json::json!({
            "status": "error",
            "redis": "connection_failed",
            "error": e.to_string()
        })),
    }
}

/// Welcome endpoint
async fn greet() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({"message": "Welcome to KnightVerse API"}))
}

/// Prometheus metrics endpoint — exposes `db_pool_connections_*` and any other
/// registered metrics in the crate's metrics registry.
async fn metrics_endpoint(pool: web::Data<DbPool>) -> impl Responder {
    // Snapshot pool stats into Prometheus gauges before encoding
    pool.update_metrics();
    crate::metrics::metrics_handler().await
}

/// Main server initialization function
pub async fn main() -> std::io::Result<()> {
    let openapi = ApiDoc::openapi();

    // Load environment variables from .env file
    dotenv().ok();

    // Initialize structured logger with JSON output in release builds
    {
        use tracing_subscriber::EnvFilter;

        let env_filter =
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

        let subscriber = tracing_subscriber::fmt().with_env_filter(env_filter);

        #[cfg(debug_assertions)]
        let subscriber = subscriber.pretty();

        #[cfg(not(debug_assertions))]
        let subscriber = subscriber.json();

        let _ = subscriber.try_init();
    }

    // Load configuration from environment — critical secrets have no fallbacks (BE-27)
    let server_addr = env::var("SERVER_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string());
    let _database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set in .env");

    // JWT: strict env — crash on missing/insecure secret
    // JWT: strict env — crash on missing/insecure secret
    let jwt_service = JwtService::from_env();
    let jwt_secret = jwt_service.secret_key.clone();
    let jwt_expiration = jwt_service.expiration_time();

    // Redis: strict env — no localhost fallback that could mask misconfiguration
    let redis_url = env::var("REDIS_URL")
        .expect("REDIS_URL must be set. Refusing to start with a hardcoded fallback.");

    info!("Initializing KnightVerse Backend Server");
    info!("Server address: {}", server_addr);

    // Connect to database — dual pool (primary + optional replica)
    let db_pool = DbPool::from_env().await;
    info!(
        "Database pool ready (has_replica={})",
        db_pool.has_replica()
    );

    // Run database migrations against the primary
    info!("Running database migrations");
    Migrator::up(db_pool.primary(), None).await.map_err(|e| {
        error!(error = %e, "Database migrations failed; refusing to start");
        std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("Database migrations failed: {e}"),
        )
    })?;
    info!("Database migrations completed successfully");

    let db_pool = Arc::new(db_pool);

    // Create a shared LobbyState actor
    let lobby = LobbyState::new().start();

    // Create a shared ConnectionStateTracker actor with DB pool for game session management
    let connection_tracker = ConnectionStateTracker::new(Some((*db_pool).clone())).start();

    // Create the Redis pub/sub broadcaster used to fan messages out to spectators
    let redis_broadcaster = crate::redis_broadcast::RedisBroadcaster::new(&redis_url)
        .expect("Failed to create Redis broadcaster");

    // Initialize application-level Prometheus metrics
    crate::metrics::init_metrics();

    // Load AppConfig
    let config = AppConfig::from_env();

    // Initialize Matchmaking
    info!("Connecting to Redis for matchmaking at {}", redis_url);
    let redis_pool = create_redis_pool(&redis_url).expect("Failed to create Redis pool");

    // Optional: test connection
    if let Err(e) = test_redis_connection(&redis_pool).await {
        warn!("Warning: Redis connection test failed: {}", e);
    }

    let rate_limiter_pool = redis_pool.clone();
    let matchmaking_service = MatchmakingService::new(redis_pool.clone());

    // Initialize Puzzle Validation Service
    let puzzle_service = Arc::new(PuzzleValidationService::new(jwt_secret.clone()));

    info!("Starting HTTP server on {}", server_addr);

    // Define the app factory closure
    let app_factory = move || {
        let db_pool = db_pool.clone();
        let jwt_service = jwt_service.clone();
        let jwt_secret = jwt_secret.clone();
        let redis_pool = redis_pool.clone();
        let redis_broadcaster = redis_broadcaster.clone();
        let matchmaking_service = matchmaking_service.clone();
        let puzzle_service = puzzle_service.clone();
        let connection_tracker = connection_tracker.clone();

        // Configure CORS middleware with environment variables for flexibility
        let cors = {
            let mut cors = Cors::default()
                .allow_any_method()
                .allow_any_header()
                .max_age(3600);

            // Get allowed origins from environment variable, fallback to all origins in development
            if let Ok(allowed_origins) = env::var("ALLOWED_ORIGINS") {
                // Parse comma-separated list of allowed origins
                let origins: Vec<&str> = allowed_origins.split(',').collect();
                for origin in origins {
                    cors = cors.allowed_origin(origin.trim());
                }
                // We don't print here to avoid spamming logs on every worker start
            } else {
                // In development, allow all origins by default
                cors = cors.allow_any_origin();
            }

            cors
        };

        // Configure Governor for Auth (Strict per-second burst protection)
        let auth_governor_conf = GovernorConfigBuilder::default()
            .per_second(config.auth_rate_limit_per_sec)
            .burst_size(config.auth_rate_limit_burst)
            .use_headers()
            .finish()
            .unwrap();

        // Configure Governor for Games/General (Loose per-second burst protection)
        let game_governor_conf = GovernorConfigBuilder::default()
            .per_second(config.game_rate_limit_per_sec)
            .burst_size(config.game_rate_limit_burst)
            .use_headers()
            .finish()
            .unwrap();

        // BE-137: engine calls are CPU-bound, so `/v1/ai` gets a tighter
        // per-second budget than games.
        let ai_governor_conf = GovernorConfigBuilder::default()
            .per_second(config.ai_rate_limit_per_sec)
            .burst_size(config.ai_rate_limit_burst)
            .use_headers()
            .finish()
            .unwrap();

        // Create Redis-backed rate limiters (shared across workers via Redis)
        let auth_redis_limiter = RedisRateLimiter::new(
            rate_limiter_pool.clone(),
            config.redis_auth_rate_limit,
            config.redis_auth_rate_limit_window,
        );

        let game_redis_limiter = RedisRateLimiter::new(
            rate_limiter_pool.clone(),
            config.redis_game_rate_limit,
            config.redis_game_rate_limit_window,
        );

        let ai_redis_limiter = RedisRateLimiter::new(
            rate_limiter_pool.clone(),
            config.redis_ai_rate_limit,
            config.redis_ai_rate_limit_window,
        );

        // BE-46: Redis-backed IdempotencyMiddleware for mutating financial, staking & tournament requests
        let idempotency_middleware = IdempotencyMiddleware::new(rate_limiter_pool.clone());

        App::new()
            .wrap(RequestIdMiddleware)
            .wrap(TracingLogger::default())
            .wrap(actix_web::middleware::DefaultHeaders::new().add((
                "Strict-Transport-Security",
                "max-age=31536000; includeSubDomains",
            )))
            // Global middleware
            .wrap(cors)
            // App data
            .app_data(web::Data::from(db_pool.clone()))
            .app_data(web::Data::new(redis_pool.clone()))
            .app_data(web::Data::new(jwt_service.clone()))
            .app_data(web::Data::new(lobby.clone()))
            .app_data(web::Data::new(connection_tracker.clone()))
            .app_data(web::Data::new(redis_broadcaster.clone()))
            .app_data(web::Data::new(matchmaking_service.clone()))
            .app_data(web::Data::new(puzzle_service.clone()))
            // BE-137: bounded admission control in front of the engine.
            .app_data(web::Data::new(EnginePool::from_env()))
            // Lc0 orchestrator is process-wide and internally bounded (BE-132).
            .app_data(web::Data::new(Arc::new(Lc0Orchestrator::from_env())))
            // Register your routes
            .route("/health", web::get().to(health))
            .route("/health/redis", web::get().to(health_redis))
            .route("/health/ready", web::get().to(readiness))
            .route("/", web::get().to(greet))
            .route("/metrics", web::get().to(metrics_endpoint))
            // Puzzle routes
            .configure(configure_puzzle_routes)
            // Chess960 position library (BE-132) — previously unreachable
            // because it lived in the `knightverse-core` shell crate.
            .configure(configure_chess960_routes)
            // Player routes
            .service(
                web::scope("/v1/players")
                    .wrap(JwtAuthMiddleware::new_with_redis(
                        jwt_secret.clone(),
                        jwt_expiration,
                        Some(redis_pool.clone()),
                    ))
                    .service(export_player_data)
                    .service(get_preferences)
                    .service(put_preferences)
                    .service(crate::wallet_link::wallet_challenge)
                    .service(crate::wallet_link::wallet_verify)
                    .service(crate::wallet_link::wallet_unlink)
                    .service(add_player)
                    .service(find_player_by_id)
                    .service(update_player)
                    .service(delete_player),
            )
            // Admin audit-log routes (admin-gated inside the handlers)
            .service(
                web::scope("/v1/admin")
                    .wrap(JwtAuthMiddleware::new_with_redis(
                        jwt_secret.clone(),
                        jwt_expiration,
                        Some(redis_pool.clone()),
                    ))
                    .service(list_actions)
                    .service(record_action),
            )
            // Game routes
            .service(
                web::scope("/v1/games")
                    .wrap(Governor::new(&game_governor_conf))
                    .wrap(game_redis_limiter)
                    .wrap(JwtAuthMiddleware::new_with_redis(
                        jwt_secret.clone(),
                        jwt_expiration,
                        Some(redis_pool.clone()),
                    ))
                    .service(create_game)
                    .service(get_game)
                    .service(list_games)
                    .service(join_game)
                    .service(make_move)
                    .service(abandon_game)
                    .service(import_game)
                    .service(complete_game),
            )
            // Auth routes
            .service(
                web::scope("/v1/auth")
                    .wrap(Governor::new(&auth_governor_conf))
                    .wrap(auth_redis_limiter.clone())
                    .service(login)
                    .service(register)
                    .service(refresh)
                    .service(issue_ws_ticket)
                    .service(logout)
                    .service(logout_all),
            )
            // Tournament routes (with Idempotency protection)
            .service(
                web::scope("/v1/tournaments")
                    .wrap(idempotency_middleware.clone())
                    .wrap(JwtAuthMiddleware::new_with_redis(
                        jwt_secret.clone(),
                        jwt_expiration,
                        Some(redis_pool.clone()),
                    ))
                    // BE-135: Swiss round generation, previously unreachable
                    // because nothing depended on the `pairing` crate.
                    .route("", web::get().to(crate::tournaments::list_tournaments))
                    .route("", web::post().to(crate::tournaments::create_tournament))
                    .route("/{id}/pairings", web::post().to(crate::pairings::build_pairing_round))
                    .route(
                        "/{id}/register",
                        web::post().to(crate::tournaments::register_for_tournament),
                    ),
            )
            // Canonical v1 tournament routes. Keep the /api/v1 aliases above
            // for clients already using that route shape.
            .service(
                web::scope("/v1/tournaments")
                    .wrap(JwtAuthMiddleware::new_with_redis(
                        jwt_secret.clone(),
                        jwt_expiration,
                        Some(redis_pool.clone()),
                    ))
                    .route("", web::get().to(crate::tournaments::list_tournaments))
                    .route("", web::post().to(crate::tournaments::create_tournament))
                    .route("/{id}/pairings", web::post().to(crate::pairings::build_pairing_round))
                    .route(
                        "/{id}/register",
                        web::post().to(crate::tournaments::register_for_tournament),
                    ),
            )
            // Escrow routes (with Idempotency protection)
            .service(
                web::scope("/v1/escrow")
                    .wrap(idempotency_middleware.clone())
                    .wrap(JwtAuthMiddleware::new_with_redis(
                        jwt_secret.clone(),
                        jwt_expiration,
                        Some(redis_pool.clone()),
                    ))
                    .route(
                        "/{action}",
                        web::post().to(|path: web::Path<String>| async move {
                            HttpResponse::Ok().json(serde_json::json!({
                                "status": "success",
                                "operation": path.into_inner(),
                                "message": "Escrow operation completed successfully"
                            }))
                        }),
                    ),
            )
            // Staking routes (with Idempotency protection)
            .service(
                web::scope("/v1/staking")
                    .wrap(idempotency_middleware.clone())
                    .wrap(JwtAuthMiddleware::new_with_redis(
                        jwt_secret.clone(),
                        jwt_expiration,
                        Some(redis_pool.clone()),
                    ))
                    .route(
                        "/{action}",
                        web::post().to(|path: web::Path<String>| async move {
                            HttpResponse::Ok().json(serde_json::json!({
                                "status": "success",
                                "operation": path.into_inner(),
                                "message": "Staking operation completed successfully"
                            }))
                        }),
                    ),
            )
            // WebSocket routes
            .service(web::scope("/v1/ws").route("/game/{game_id}", web::get().to(ws_route)))
            // Matchmaking routes
            .service(web::scope("/v1").configure(matchmaking::routes::config))
            // AI routes
            .service(
                web::scope("/v1/ai")
                    .wrap(Governor::new(&ai_governor_conf))
                    .wrap(ai_redis_limiter)
                    .wrap(JwtAuthMiddleware::new_with_redis(
                        jwt_secret.clone(),
                        jwt_expiration,
                        Some(redis_pool.clone()),
                    ))
                    .service(get_ai_suggestion)
                    .service(analyze_position)
                    .service(lc0_analyze)
                    .service(lc0_health),
            )
            // Engine-pool saturation is exactly what an operator needs to see
            // during an incident, so this one route stays outside the auth
            // gate and the limiters.
            .route("/health/engine", web::get().to(ai_health))
            // NFT routes (placeholder — not yet implemented)
            // .service(web::scope("/v1").configure(configure_nft_routes))
            // Swagger UI integration
            .service(
                SwaggerUi::new("/api/docs/{_:.*}")
                    .url("/api/docs/openapi.json", openapi.clone())
                    .config(utoipa_swagger_ui::Config::default().try_it_out_enabled(true)),
            )
            // ReDoc integration (alternative documentation UI)
            .service(Redoc::with_url("/api/redoc", openapi.clone()))
            // WebSocket documentation as static HTML
            .route(
                "/api/docs/websocket",
                web::get().to(|| async {
                    HttpResponse::Ok()
                        .content_type("text/markdown")
                        .body(crate::openapi::websocket_documentation())
                }),
            )
    };

    let mut http_server = HttpServer::new(app_factory).bind(&server_addr)?;

    if let Ok(workers_str) = env::var("WORKERS") {
        if let Ok(workers) = workers_str.parse::<usize>() {
            info!("Setting worker count to {}", workers);
            http_server = http_server.workers(workers);
        }
    }

    // BE-26: Graceful shutdown on SIGTERM / SIGINT
    // stop(true) waits for in-flight requests and drains the worker thread pool
    // so active WebSocket connections and DB transactions can complete cleanly.
    let server = http_server.run();
    let server_handle = server.handle();

    actix_web::rt::spawn(async move {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};
            let mut sigterm =
                signal(SignalKind::terminate()).expect("failed to install SIGTERM handler");
            let mut sigint =
                signal(SignalKind::interrupt()).expect("failed to install SIGINT handler");
            tokio::select! {
                _ = sigterm.recv() => {
                    info!("Received SIGTERM; initiating graceful shutdown");
                }
                _ = sigint.recv() => {
                    info!("Received SIGINT; initiating graceful shutdown");
                }
            }
        }
        #[cfg(not(unix))]
        {
            let _ = tokio::signal::ctrl_c().await;
            info!("Received Ctrl+C; initiating graceful shutdown");
        }
        // true = graceful: finish active connections, then drain workers
        server_handle.stop(true).await;
        info!("Server stopped gracefully");
    });

    server.await
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{http::StatusCode, test};
    use sea_orm::{DatabaseConnection, DbBackend, MockDatabase};
    use std::time::Duration;

    const TEST_TIMEOUT: Duration = Duration::from_millis(300);

    fn mock_db() -> Arc<DatabaseConnection> {
        Arc::new(MockDatabase::new(DbBackend::Postgres).into_connection())
    }

    fn pool_of(conn: Arc<DatabaseConnection>) -> DbPool {
        DbPool::from_connections(conn.clone(), conn, false)
    }

    /// Redis pool pointed at a port nothing listens on (connection refused).
    fn refused_redis_pool() -> deadpool_redis::Pool {
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        create_redis_pool(&format!("redis://127.0.0.1:{port}")).unwrap()
    }

    #[actix_web::test]
    async fn health_is_liveness_only_and_needs_no_dependencies() {
        let app = test::init_service(App::new().route("/health", web::get().to(health))).await;
        let res = test::call_service(&app, test::TestRequest::get().uri("/health").to_request())
            .await;
        assert_eq!(res.status(), StatusCode::OK);
        let body: serde_json::Value = test::read_body_json(res).await;
        assert_eq!(body, serde_json::json!({"status": "ok"}));
    }

    #[actix_web::test]
    async fn check_db_ok_when_primary_responds() {
        assert!(check_db(&mock_db(), TEST_TIMEOUT).await.is_ok());
    }

    #[actix_web::test]
    async fn check_db_fails_when_primary_disconnected() {
        assert!(check_db(&DatabaseConnection::Disconnected, TEST_TIMEOUT)
            .await
            .is_err());
    }

    #[actix_web::test]
    async fn check_redis_fails_when_connection_refused() {
        assert!(check_redis(&refused_redis_pool(), TEST_TIMEOUT)
            .await
            .is_err());
    }

    #[actix_web::test]
    async fn check_redis_times_out_when_server_never_replies() {
        // Accept connections but never write a byte back, so PING hangs.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let _accept = tokio::spawn(async move {
            let mut held = Vec::new();
            while let Ok((sock, _)) = listener.accept().await {
                held.push(sock);
            }
        });
        let pool = create_redis_pool(&format!("redis://127.0.0.1:{port}")).unwrap();

        let start = std::time::Instant::now();
        let err = check_redis(&pool, TEST_TIMEOUT).await.unwrap_err();
        assert!(err.contains("timed out"), "unexpected error: {err}");
        assert!(start.elapsed() < Duration::from_secs(2));
    }

    #[actix_web::test]
    async fn readiness_returns_503_when_redis_down() {
        let res = readiness_response(&pool_of(mock_db()), &refused_redis_pool(), TEST_TIMEOUT)
            .await;
        assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
        let bytes = actix_web::body::to_bytes(res.into_body()).await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body["status"], "error");
        assert_eq!(body["checks"]["database"]["status"], "ok");
        assert_eq!(body["checks"]["redis"]["status"], "error");
    }

    #[actix_web::test]
    async fn readiness_returns_503_when_db_down() {
        let res = readiness_response(
            &pool_of(Arc::new(DatabaseConnection::Disconnected)),
            &refused_redis_pool(),
            TEST_TIMEOUT,
        )
        .await;
        assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
        let bytes = actix_web::body::to_bytes(res.into_body()).await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body["checks"]["database"]["status"], "error");
    }

    #[actix_web::test]
    async fn readiness_route_is_registered_and_reports_unready() {
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(pool_of(mock_db())))
                .app_data(web::Data::new(refused_redis_pool()))
                .route("/health/ready", web::get().to(readiness)),
        )
        .await;
        let res = test::call_service(
            &app,
            test::TestRequest::get().uri("/health/ready").to_request(),
        )
        .await;
        assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
    }
}
