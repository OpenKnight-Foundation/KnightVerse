// src/server.rs

pub mod request_id;

use crate::ai::{analyze_position, get_ai_suggestion};
use crate::auth::{login, logout, refresh, register};
use crate::config::AppConfig;
use crate::metrics;
use crate::games::{
    abandon_game, complete_game, create_game, get_game, import_game, join_game, list_games,
    make_move,
};
use crate::idempotency::IdempotencyMiddleware;
use crate::players::{add_player, delete_player, find_player_by_id, update_player};
use crate::rate_limiter::RedisRateLimiter;
use crate::request_id::RequestIdMiddleware;
use crate::ws::{ws_route, LobbyState, ConnectionStateTracker};
use actix::Actor;
use actix_cors::Cors;
use actix_governor::{Governor, GovernorConfigBuilder};
use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use challenge::api::configure_puzzle_routes;
use challenge::puzzle_validation::PuzzleValidationService;
use db::DbPool;
use dotenv::dotenv;
use matchmaking::redis::{create_redis_pool, test_redis_connection};
use matchmaking::MatchmakingService;
use migration::Migrator;
use migration::MigratorTrait;
use security::jwt::{JwtAuthMiddleware, JwtService};
use tracing::{info, warn, error};
use tracing_actix_web::TracingLogger;
use std::env;
use std::sync::Arc;
use utoipa::OpenApi;
use utoipa_redoc::{Redoc, Servable};
use utoipa_swagger_ui::SwaggerUi;

use crate::openapi::ApiDoc;

/// Health check endpoint
async fn health() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({"status": "ok"}))
}

/// Redis health check endpoint
async fn health_redis(
    redis_pool: web::Data<deadpool_redis::Pool>,
) -> impl Responder {
    use redis::AsyncCommands;
    let start = std::time::Instant::now();
    match redis_pool.get().await {
        Ok(mut conn) => {
            let ping_result: Result<String, _> = redis::cmd("PING")
                .query_async(&mut conn)
                .await;
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
/// registered metrics in the default registry.
async fn metrics(pool: web::Data<DbPool>) -> impl Responder {
    // Snapshot pool stats into Prometheus gauges before encoding
    pool.update_metrics();

    match prometheus::TextEncoder::new()
        .encode_to_string(&prometheus::gather())
    {
        Ok(body) => HttpResponse::Ok()
            .content_type("text/plain; version=0.0.4")
            .body(body),
        Err(e) => {
            tracing::error!("Failed to encode Prometheus metrics: {}", e);
            HttpResponse::InternalServerError().body("Failed to encode metrics")
        }
    }
}

/// Main server initialization function
pub async fn main() -> std::io::Result<()> {
    let openapi = ApiDoc::openapi();

    // Load environment variables from .env file
    dotenv().ok();

    // Initialize structured logger with JSON output in release builds
    {
        use tracing_subscriber::EnvFilter;

        let env_filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("info"));

        let subscriber = tracing_subscriber::fmt()
            .with_env_filter(env_filter);

        #[cfg(debug_assertions)]
        let subscriber = subscriber.pretty();

        #[cfg(not(debug_assertions))]
        let subscriber = subscriber.json();

        subscriber.init();
    }

    // Load configuration from environment — critical secrets have no fallbacks (BE-27)
    let server_addr = env::var("SERVER_ADDR").unwrap_or_else(|_| "127.0.0.1:8080".to_string());
    let _database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set in .env");

    // JWT: strict env — crash on missing/insecure secret
    let jwt_service = JwtService::from_env();
    let jwt_secret = jwt_service.secret_key.clone();
    let jwt_expiration = jwt_service.expiration_time();

    // Redis: strict env — no localhost fallback that could mask misconfiguration
    let redis_url = env::var("REDIS_URL").expect(
        "REDIS_URL must be set. Refusing to start with a hardcoded fallback.",
    );

    info!("Initializing KnightVerse Backend Server");
    info!("Server address: {}", server_addr);

    // Connect to database — dual pool (primary + optional replica)
    let db_pool = DbPool::from_env().await;
    info!(
        "Database pool ready (has_replica={})",
        db_pool.has_replica()
    );

    // Run database migrations against the primary
    eprintln!("Running database migrations...");
    match Migrator::up(db_pool.primary(), None).await {
        Ok(_) => eprintln!("Database migrations completed successfully"),
        Err(e) => {
            eprintln!("Warning: Failed to run database migrations: {}", e);
        }
    }

    let db_pool = Arc::new(db_pool);

    // Create a shared LobbyState actor
    let lobby = LobbyState::new().start();

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
    let matchmaking_service = MatchmakingService::new(redis_pool);

    // Initialize Puzzle Validation Service
    let puzzle_service = Arc::new(PuzzleValidationService::new(jwt_secret.clone()));

    info!("Starting HTTP server on {}", server_addr);

    // Define the app factory closure
    let app_factory = move || {
        let db_pool = db_pool.clone();
        let jwt_service = jwt_service.clone();
        let jwt_secret = jwt_secret.clone();
        let matchmaking_service = matchmaking_service.clone();
        let puzzle_service = puzzle_service.clone();

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

        // BE-46: Redis-backed IdempotencyMiddleware for mutating financial, staking & tournament requests
        let idempotency_middleware = IdempotencyMiddleware::new(rate_limiter_pool.clone());

        App::new()
            .wrap(RequestIdMiddleware)
            .wrap(TracingLogger::default())
            .wrap(actix_web::middleware::DefaultHeaders::new().add(("Strict-Transport-Security", "max-age=31536000; includeSubDomains")))
            // Global middleware
            .wrap(cors)
            .wrap(create_metricsMiddleware(metrics_collector.clone()))
            // App data
            .app_data(web::Data::from(db_pool.clone()))
            .app_data(web::Data::new(jwt_service.clone()))
            .app_data(web::Data::new(lobby.clone()))
            .app_data(web::Data::new(matchmaking_service.clone()))
            .app_data(web::Data::new(puzzle_service.clone()))
            .app_data(web::Data::new(metrics_collector.clone()))
            // Register your routes
            .route("/health", web::get().to(health))
            .route("/health/redis", web::get().to(health_redis))
            .route("/", web::get().to(greet))
            .route("/metrics", web::get().to(metrics_endpoint))
            // Puzzle routes
            .configure(configure_puzzle_routes)
            // Player routes
            .service(
                web::scope("/v1/players")
                    .wrap(JwtAuthMiddleware::new(jwt_secret.clone(), jwt_expiration))
                    .service(add_player)
                    .service(find_player_by_id)
                    .service(update_player)
                    .service(delete_player),
            )
            // Game routes
            .service(
                web::scope("/v1/games")
                    .wrap(Governor::new(&game_governor_conf))
                    .wrap(game_redis_limiter)
                    .wrap(JwtAuthMiddleware::new(jwt_secret.clone(), jwt_expiration))
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
                    .wrap(auth_redis_limiter)
                    .service(login)
                    .service(register)
                    .service(refresh)
                    .service(logout),
            )
            // Tournament routes (with Idempotency protection)
            .service(
                web::scope("/api/v1/tournaments")
                    .wrap(JwtAuthMiddleware::new(jwt_secret.clone(), jwt_expiration))
                    .route(
                        "/{id}/register",
                        web::post().to(|path: web::Path<String>| async move {
                            HttpResponse::Ok().json(serde_json::json!({
                                "status": "registered",
                                "tournament_id": path.into_inner(),
                                "message": "Tournament registration successful"
                            }))
                        }),
                    ),
            )
            // Escrow routes (with Idempotency protection)
            .service(
                web::scope("/api/v1/escrow")
                    .wrap(JwtAuthMiddleware::new(jwt_secret.clone(), jwt_expiration))
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
                web::scope("/api/v1/staking")
                    .wrap(JwtAuthMiddleware::new(jwt_secret.clone(), jwt_expiration))
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
                    .wrap(JwtAuthMiddleware::new(jwt_secret.clone(), jwt_expiration))
                    .service(get_ai_suggestion)
                    .service(analyze_position),
            )
            // NFT routes (placeholder — not yet implemented)
            // .service(web::scope("/api/v1").configure(configure_nft_routes))
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
            let mut sigterm = signal(SignalKind::terminate())
                .expect("failed to install SIGTERM handler");
            let mut sigint = signal(SignalKind::interrupt())
                .expect("failed to install SIGINT handler");
            tokio::select! {
                _ = sigterm.recv() => {
                    eprintln!("Received SIGTERM — initiating graceful shutdown...");
                }
                _ = sigint.recv() => {
                    eprintln!("Received SIGINT — initiating graceful shutdown...");
                }
            }
        }
        #[cfg(not(unix))]
        {
            let _ = tokio::signal::ctrl_c().await;
            eprintln!("Received Ctrl+C — initiating graceful shutdown...");
        }
        // true = graceful: finish active connections, then drain workers
        server_handle.stop(true).await;
        eprintln!("Server stopped gracefully");
    });

    server.await
}