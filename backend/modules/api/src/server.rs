// src/server.rs

use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use actix_cors::Cors;
use dotenv::dotenv;
use sea_orm::{Database, DatabaseConnection};
use std::env;
use std::sync::Arc;
use security::JwtService;
use security::JwtAuthMiddleware;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;
use utoipa_redoc::{Redoc, Servable};
use actix::Actor;
use crate::players::{add_player, delete_player, find_player_by_id, update_player};
use crate::games::{create_game, get_game, make_move, list_games, join_game, abandon_game, import_game, complete_game};
use crate::auth::{login, register, refresh, logout};
use crate::ai::{get_ai_suggestion, analyze_position};
use crate::ws::{LobbyState, ws_route};
use crate::config::AppConfig;
use crate::metrics_middleware::create_metricsMiddleware;
use actix_governor::{Governor, GovernorConfigBuilder};
use matchmaking::service::MatchmakingService;
use matchmaking::redis::{create_redis_pool, test_redis_connection};
use challenge::puzzle_validation::PuzzleValidationService;
use challenge::api::configure_puzzle_routes;
use st_core::endpoint::configure as configure_nft_routes;
use metrics::{MetricsCollector, metrics_endpoint};

use crate::openapi::ApiDoc;

/// Health check endpoint
async fn health() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({"status": "ok"}))
}

/// Welcome endpoint
async fn greet() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({"message": "Welcome to KnightVerse API"}))
}

/// Main server initialization function
pub async fn main() -> std::io::Result<()> {
    let openapi = ApiDoc::openapi();

    // Load environment variables from .env file
    dotenv().ok();

    // Initialize logger
    env_logger::init();

    // Load configuration from environment
    let server_addr = env::var("SERVER_ADDR").unwrap_or_else(|_| "127.0.0.1:8080".to_string());
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set in .env");
    let jwt_secret = env::var("JWT_SECRET_KEY")
        .unwrap_or_else(|_| "knightverse_dev_secret_key_change_in_production".to_string());
    let jwt_expiration = env::var("JWT_EXPIRATION_SECS")
        .unwrap_or_else(|_| "3600".to_string())
        .parse::<usize>()
        .unwrap_or(3600);

    let redis_url = env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".to_string());

    eprintln!("Initializing KnightVerse Backend Server");
    eprintln!("Server address: {}", server_addr);

    // Connect to database
    let db = match Database::connect(&database_url).await {
        Ok(conn) => {
            eprintln!("Database connection successful");
            conn
        }
        Err(e) => {
            eprintln!("Failed to connect to database: {}", e);
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Database connection failed",
            ));
        }
    };

    // Initialize JWT service
    let jwt_service = JwtService::new(jwt_secret.clone(), jwt_expiration);
    let db = std::sync::Arc::new(db); // Wrap db in Arc

    // Create a shared LobbyState actor
    let lobby = LobbyState::new().start();

    // Load AppConfig
    let config = AppConfig::from_env();

    // Initialize Matchmaking
    eprintln!("Connecting to Redis for matchmaking at {}", redis_url);
    let redis_pool = create_redis_pool(&redis_url).expect("Failed to create Redis pool");
    
    // Optional: test connection
    if let Err(e) = test_redis_connection(&redis_pool).await {
        eprintln!("Warning: Redis connection test failed: {}", e);
    }
    
    let matchmaking_service = MatchmakingService::new(redis_pool);

    // Initialize Puzzle Validation Service
    let puzzle_service = Arc::new(PuzzleValidationService::new(jwt_secret.clone()));

    // Initialize Metrics Collector
    let metrics_collector = Arc::new(MetricsCollector::new());
    eprintln!("Metrics collector initialized");

    eprintln!("Starting HTTP server on {}", server_addr);

    // Define the app factory closure
    let app_factory = move || {
        let db = db.clone();
        let jwt_service = jwt_service.clone();
        let jwt_secret = jwt_secret.clone();
        let matchmaking_service = matchmaking_service.clone();
        let puzzle_service = puzzle_service.clone();
        let metrics_collector = metrics_collector.clone();
        
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
        
        // Configure Governor for Auth (Strict)
        let auth_governor_conf = GovernorConfigBuilder::default()
            .per_second(config.auth_rate_limit_per_sec)
            .burst_size(config.auth_rate_limit_burst)
            .use_headers()
            .finish()
            .unwrap();

        // Configure Governor for Games/General (Loose)
        let game_governor_conf = GovernorConfigBuilder::default()
            .per_second(config.game_rate_limit_per_sec)
            .burst_size(config.game_rate_limit_burst)
            .use_headers()
            .finish()
            .unwrap();

        App::new()
            // Global middleware
            .wrap(cors)
            .wrap(create_metricsMiddleware(metrics_collector.clone()))
            // App data
            .app_data(web::Data::from(db.clone()))
            .app_data(web::Data::new(jwt_service.clone()))
            .app_data(web::Data::new(lobby.clone()))
            .app_data(web::Data::new(matchmaking_service.clone()))
            .app_data(web::Data::new(puzzle_service.clone()))
            .app_data(web::Data::new(metrics_collector.clone()))
            // Register your routes
            .route("/health", web::get().to(health))
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
                    .service(login)
                    .service(register)
                    .service(refresh)
                    .service(logout)
            )
            // WebSocket routes
            .service(
                web::scope("/v1/ws")
                    .route("/game/{game_id}", web::get().to(ws_route))
            )
            // Matchmaking routes
            .service(
                web::scope("/v1")
                    .configure(matchmaking::routes::config)
            )
            // AI routes
            .service(
                web::scope("/v1/ai")
                    .wrap(JwtAuthMiddleware::new(jwt_secret.clone(), jwt_expiration))
                    .service(get_ai_suggestion)
                    .service(analyze_position),
            )
            // NFT routes
            .service(
                web::scope("/api/v1")
                    .configure(configure_nft_routes)
            )
            // Swagger UI integration
            .service(
                SwaggerUi::new("/api/docs/{_:.*}")
                    .url("/api/docs/openapi.json", openapi.clone())
                    .config(utoipa_swagger_ui::Config::default().try_it_out_enabled(true))
            )
            // ReDoc integration (alternative documentation UI)
            .service(
                Redoc::with_url("/api/redoc", openapi.clone())
            )
            // WebSocket documentation as static HTML
            .route("/api/docs/websocket", web::get().to(|| async {
                HttpResponse::Ok()
                    .content_type("text/markdown")
                    .body(crate::openapi::websocket_documentation())
            }))
    };

    let mut server = HttpServer::new(app_factory).bind(&server_addr)?;

    if let Ok(workers_str) = env::var("WORKERS") {
        if let Ok(workers) = workers_str.parse::<usize>() {
            println!("Setting worker count to {}", workers);
            server = server.workers(workers);
        }
    }

    server.run().await
}
