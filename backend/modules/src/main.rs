use actix_web::{App, HttpServer};
use dotenv::dotenv;
use modules::matchmaking;
use std::env;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();
    env_logger::init();

    println!("Starting KnightVerse Backend Server...");

    // Initialize Redis connection pool
    let redis_url = env::var("REDIS_URL")
        .unwrap_or_else(|_| {
            println!("REDIS_URL not set, using default: redis://localhost:6379");
            "redis://localhost:6379".to_string()
        });

    let redis_pool = matchmaking::redis::create_redis_pool(&redis_url)
        .expect("Failed to create Redis pool");

    // Test Redis connection on startup
    match matchmaking::redis::test_redis_connection(&redis_pool).await {
        Ok(_) => println!("✅ Redis connection successful"),
        Err(e) => {
            eprintln!("⚠️  Warning: Redis connection failed: {}", e);
            eprintln!("Matchmaking service will not be available");
        }
    }

    println!("Server starting on http://127.0.0.1:8080");

    HttpServer::new(move || {
        App::new()
            .app_data(matchmaking::service::get_matchmaking_service(redis_pool.clone()))
            .configure(matchmaking::routes::config)
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
