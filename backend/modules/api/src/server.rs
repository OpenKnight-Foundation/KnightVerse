// src/server.rs

// --- Admin audit-log handlers ---
use crate::admin::{list_actions, record_action};

// --- AI handlers: move suggestions and position analysis ---
use crate::ai::{analyze_position, get_ai_suggestion};
// --- Auth handlers: login, registration, token refresh, logout ---
use crate::auth::{login, logout, logout_all, refresh, register};
use crate::config::AppConfig;
// --- Game lifecycle handlers ---
use crate::games::{
    abandon_game, complete_game, create_game, get_game, import_game, join_game, list_games,
    make_move,
};
use crate::chess960::configure_routes as configure_chess960_routes;
use crate::idempotency::IdempotencyMiddleware;
// --- Lc0 (Leela Chess Zero) engine endpoints ---
use crate::lc0::{lc0_analyze, lc0_health};
use crate::players::{add_player, delete_player, export_player_data, find_player_by_id, update_player};
use crate::rate_limiter::RedisRateLimiter;
use crate::request_id::RequestIdMiddleware;
// --- WebSocket pieces: route handler, connection tracking actor, lobby actor ---
use crate::ws::{ws_route, ConnectionStateTracker, LobbyState};
use actix::Actor;
use actix_cors::Cors;
// Governor provides in-memory (per-worker) rate limiting
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
use tracing::{info, warn};
use tracing_actix_web::TracingLogger;
// OpenAPI / API documentation tooling
use utoipa::OpenApi;
use utoipa_redoc::{Redoc, Servable};
use utoipa_swagger_ui::SwaggerUi;

use crate::openapi::ApiDoc;

/// Health check endpoint
/// Lightweight liveness probe; does not touch any dependencies.
async fn health() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({"status": "ok"}))
}

/// Redis health check endpoint
/// Readiness-style probe: checks out a pooled connection and sends PING,
/// reporting round-trip latency. Returns 503 if either step fails.
async fn health_redis(redis_pool: web::Data<deadpool_redis::Pool>) -> impl Responder {
    // Time the whole operation (pool checkout + PING)
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
                // Connected, but the PING command itself failed
                Err(e) => HttpResponse::ServiceUnavailable().json(serde_json::json!({
                    "status": "error",
                    "redis": "ping_failed",
                    "error": e.to_string()
                })),
            }
        }
        // Could not obtain a connection from the pool at all
        Err(e) => HttpResponse::ServiceUnavailable().json(serde_json::json!({
            "status": "error",
            "redis": "connection_failed",
            "error": e.to_string()
        })),
    }
}

/// Welcome endpoint
/// Simple root route, handy for confirming the server is reachable.
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
/// Wires up config, logging, DB, Redis, actors and services, builds the
/// Actix app factory with all routes/middleware, and runs the HTTP server
/// with graceful shutdown.
pub async fn main() -> std::io::Result<()> {
    // Build the OpenAPI spec once; cloned into each worker's Swagger/ReDoc services
    let openapi = ApiDoc::openapi();

    // Load environment variables from .env file
    // (.ok() ignores a missing file, e.g. in production where env is injected)
    dotenv().ok();

    // Initialize structured logger with JSON output in release builds
    {
        use tracing_subscriber::EnvFilter;

        // Respect RUST_LOG if set, otherwise default to "info"
        let env_filter =
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

        let subscriber = tracing_subscriber::fmt().with_env_filter(env_filter);

        // Human-friendly output while developing
        #[cfg(debug_assertions)]
        let subscriber = subscriber.pretty();

        // Machine-parseable JSON output for release/production builds
        #[cfg(not(debug_assertions))]
        let subscriber = subscriber.json();

        subscriber.init();
    }

    // Load configuration from environment — critical secrets have no fallbacks (BE-27)
    let server_addr = env::var("SERVER_ADDR").unwrap_or_else(|_| "127.0.0.1:8080".to_string());
    // Only validated here; the DB pool reads DATABASE_URL itself in from_env()
    let _database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set in .env");

    // JWT: strict env — crash on missing/insecure secret
    let jwt_service = JwtService::from_env();
    // Keep copies of the secret and expiry for building per-scope JWT middleware
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
    eprintln!("Running database migrations...");
    match Migrator::up(db_pool.primary(), None).await {
        Ok(_) => eprintln!("Database migrations completed successfully"),
        // Non-fatal: the server still starts even if migrations fail
        Err(e) => {
            eprintln!("Warning: Failed to run database migrations: {}", e);
        }
    }

    // Wrap in Arc so all workers and actors can share one pool
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
    // (rate-limit settings and other tunables)
    let config = AppConfig::from_env();

    // Initialize Matchmaking
    info!("Connecting to Redis for matchmaking at {}", redis_url);
    let redis_pool = create_redis_pool(&redis_url).expect("Failed to create Redis pool");

    // Optional: test connection
    // A failure only logs a warning; startup continues
    if let Err(e) = test_redis_connection(&redis_pool).await {
        warn!("Warning: Redis connection test failed: {}", e);
    }

    // Separate clone of the pool for the Redis-backed rate limiters / idempotency
    let rate_limiter_pool = redis_pool.clone();
    let matchmaking_service = MatchmakingService::new(redis_pool.clone());

    // Initialize Puzzle Validation Service
    // Uses the JWT secret (e.g. to sign/verify puzzle tokens)
    let puzzle_service = Arc::new(PuzzleValidationService::new(jwt_secret.clone()));

    info!("Starting HTTP server on {}", server_addr);

    // Define the app factory closure
    // Actix calls this once per worker thread, so every shared resource is
    // cloned inside it (cheap: Arc / handle clones).
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
                .max_age(3600); // browsers may cache preflight results for 1 hour

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
                // NOTE: if ALLOWED_ORIGINS is unset in production, this is wide open
                cors = cors.allow_any_origin();
            }

            cors
        };

        // Configure Governor for Auth (Strict per-second burst protection)
        // Per-IP, in-memory limiter; emits rate-limit headers on responses
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
        // Unlike Governor, these limit across all workers/instances using a time window
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
        // NOTE: currently constructed but not `.wrap()`ed on any scope below
        // (underscore prefix silences the unused warning).
        let _idempotency_middleware = IdempotencyMiddleware::new(rate_limiter_pool.clone());

        App::new()
            // Tags each request with an ID for log correlation
            .wrap(RequestIdMiddleware)
            // Structured per-request tracing/logging
            .wrap(TracingLogger::default())
            // Tell browsers to use HTTPS only for one year (incl. subdomains)
            .wrap(actix_web::middleware::DefaultHeaders::new().add((
                "Strict-Transport-Security",
                "max-age=31536000; includeSubDomains",
            )))
            // Global middleware
            // (wrapped last, so CORS runs outermost and handles preflights first)
            .wrap(cors)
            // App data
            // Shared state injected into handlers via web::Data<T>
            .app_data(web::Data::from(db_pool.clone()))
            .app_data(web::Data::new(redis_pool.clone()))
            .app_data(web::Data::new(jwt_service.clone()))
            .app_data(web::Data::new(lobby.clone()))
            .app_data(web::Data::new(connection_tracker.clone()))
            .app_data(web::Data::new(redis_broadcaster.clone()))
            .app_data(web::Data::new(matchmaking_service.clone()))
            .app_data(web::Data::new(puzzle_service.clone()))
            // Lc0 orchestrator is process-wide and internally bounded (BE-132).
            // NOTE: built inside the factory, so each worker gets its own instance.
            .app_data(web::Data::new(Arc::new(Lc0Orchestrator::from_env())))
            // Register your routes
            // Unauthenticated utility endpoints
            .route("/health", web::get().to(health))
            .route("/health/redis", web::get().to(health_redis))
            .route("/", web::get().to(greet))
            .route("/metrics", web::get().to(metrics_endpoint))
            // Puzzle routes
            .configure(configure_puzzle_routes)
            // Chess960 position library (BE-132) — previously unreachable
            // because it lived in the `knightverse-core` shell crate.
            .configure(configure_chess960_routes)
            // Player routes
            // JWT-protected; also hosts wallet-linking endpoints
            .service(
                web::scope("/v1/players")
                    .wrap(JwtAuthMiddleware::new_with_redis(
                        jwt_secret.clone(),
                        jwt_expiration,
                        Some(redis_pool.clone()), // Redis used e.g. for token revocation checks
                    ))
                    .service(export_player_data)
                    .service(crate::wallet_link::wallet_challenge)
                    .service(crate::wallet_link::wallet_verify)
                    .service(crate::wallet_link::wallet_unlink)
                    .service(add_player)
                    .service(find_player_by_id)
                    .service(update_player)
                    .service(delete_player),
            )
            // Admin audit-log routes (admin-gated inside the handlers)
            // JWT only proves identity here; the admin role check is in each handler
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
            // Middleware layers: Governor (per-IP burst) -> Redis limiter (global window) -> JWT auth
            // (Actix applies the last-added `.wrap` first, so JWT runs before the limiters)
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
            // Public (no JWT) but strictly rate limited to deter brute-force attempts
            .service(
                web::scope("/v1/auth")
                    .wrap(Governor::new(&auth_governor_conf))
                    .wrap(auth_redis_limiter.clone())
                    .service(login)
                    .service(register)
                    .service(refresh)
                    .service(logout)
                    .service(logout_all),
            )
            // Same auth handlers mounted under the /api/v1 prefix (alias for API versioning consistency)
            .service(
                web::scope("/api/v1/auth")
                    .wrap(Governor::new(&auth_governor_conf))
                    .wrap(auth_redis_limiter)
                    .service(login)
                    .service(register)
                    .service(refresh)
                    .service(logout)
                    .service(logout_all),
            )
            // Tournament routes (with Idempotency protection)
            // NOTE: placeholder handler returning a canned success; idempotency
            // middleware is not actually attached yet.
            .service(
                web::scope("/api/v1/tournaments")
                    .wrap(JwtAuthMiddleware::new_with_redis(
                        jwt_secret.clone(),
                        jwt_expiration,
                        Some(redis_pool.clone()),
                    ))
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
            // NOTE: stub — echoes the action name and always reports success
            .service(
                web::scope("/api/v1/escrow")
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
            // NOTE: stub — same canned-success behavior as escrow
            .service(
                web::scope("/api/v1/staking")
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
            // Live game connection; no JWT middleware on the scope, so any
            // authentication must happen inside `ws_route`
            .service(web::scope("/v1/ws").route("/game/{game_id}", web::get().to(ws_route)))
            // Matchmaking routes
            // Routes are defined in the matchmaking crate and mounted under /v1
            .service(web::scope("/v1").configure(matchmaking::routes::config))
            // AI routes
            // JWT-protected: move suggestions, position analysis, Lc0 engine access
            .service(
                web::scope("/v1/ai")
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
            // NFT routes (placeholder — not yet implemented)
            // .service(web::scope("/api/v1").configure(configure_nft_routes))
            // Swagger UI integration
            // Interactive docs at /api/docs/, spec served at /api/docs/openapi.json
            .service(
                SwaggerUi::new("/api/docs/{_:.*}")
                    .url("/api/docs/openapi.json", openapi.clone())
                    .config(utoipa_swagger_ui::Config::default().try_it_out_enabled(true)),
            )
            // ReDoc integration (alternative documentation UI)
            .service(Redoc::with_url("/api/redoc", openapi.clone()))
            // WebSocket documentation as static HTML
            // (actually served as text/markdown, despite the comment above)
            .route(
                "/api/docs/websocket",
                web::get().to(|| async {
                    HttpResponse::Ok()
                        .content_type("text/markdown")
                        .body(crate::openapi::websocket_documentation())
                }),
            )
    };

    // Bind the listener; `?` propagates any bind error (e.g. port in use)
    let mut http_server = HttpServer::new(app_factory).bind(&server_addr)?;

    // Optional override of the worker thread count (defaults to number of CPU cores)
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
    // Handle lets the signal task stop the server while `server` is awaited below
    let server_handle = server.handle();

    // Background task that waits for a shutdown signal, then stops the server
    actix_web::rt::spawn(async move {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};
            let mut sigterm =
                signal(SignalKind::terminate()).expect("failed to install SIGTERM handler");
            let mut sigint =
                signal(SignalKind::interrupt()).expect("failed to install SIGINT handler");
            // Whichever signal arrives first triggers shutdown
            tokio::select! {
                _ = sigterm.recv() => {
                    eprintln!("Received SIGTERM — initiating graceful shutdown...");
                }
                _ = sigint.recv() => {
                    eprintln!("Received SIGINT — initiating graceful shutdown...");
                }
            }
        }
        // Windows / non-Unix: only Ctrl+C is available
        #[cfg(not(unix))]
        {
            let _ = tokio::signal::ctrl_c().await;
            eprintln!("Received Ctrl+C — initiating graceful shutdown...");
        }
        // true = graceful: finish active connections, then drain workers
        server_handle.stop(true).await;
        eprintln!("Server stopped gracefully");
    });

    // Block until the server has fully stopped
    server.await
}