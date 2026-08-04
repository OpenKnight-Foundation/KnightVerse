use db_entity::game::{GameVariant, ResultSide}; // Added imports
use db_entity::prelude::{Game, Player};
use db_entity::{game, player};
use dotenv::dotenv;
use rand::distributions::Alphanumeric;
use rand::prelude::*;
use sea_orm::{sea_query::Expr, ActiveValue::Set, EntityTrait, QueryFilter, QuerySelect, *};
use serde_json::{json, Value as JsonValue};
use std::env;
use std::time::Instant;
use tokio::time::{sleep, Duration};
use uuid::Uuid;

// Configuration
const NUM_PLAYERS_TO_CREATE: usize = 100;
const NUM_GAMES_TO_INSERT: usize = 1_000_000;
const BATCH_SIZE: usize = 100; // Insert games in batches

// Helper to connect to the database
async fn setup_db() -> Result<DatabaseConnection, DbErr> {
    dotenv().ok(); // load .env if present
    let db_url =
        env::var("DATABASE_URL").expect("DATABASE_URL environment variable not set for benchmark");
    Database::connect(&db_url).await
}

// Helper to generate random PGN-like JSON data
fn generate_random_pgn(rng: &mut ThreadRng) -> JsonValue {
    let num_moves: usize = rng.gen_range(20..100);
    let moves: Vec<String> = (0..num_moves)
        .map(|_| {
            let len = rng.gen_range(2..6); // Calculate len first
            rng.sample_iter(&Alphanumeric)
                .take(len)
                .map(char::from)
                .collect()
        })
        .collect();

    json!({
        "event": format!("Bench Event {}", rng.gen::<u16>()),
        "site": "Benchmark Site",
        "date": format!("2024.{:02}.{:02}", rng.gen_range(1..13), rng.gen_range(1..29)),
        "round": rng.gen_range(1..10).to_string(),
        "white": format!("Bench Player W{}", rng.gen::<u16>()),
        "black": format!("Bench Player B{}", rng.gen::<u16>()),
        "result": match rng.gen_range(0..3) { 0 => "1-0", 1 => "0-1", _ => "1/2-1/2" },
        "moves": moves,
        "clock_start": 180.0,
        "final_ply": num_moves
    })
}

// Helper to generate random FEN-like string
fn generate_random_fen(rng: &mut ThreadRng) -> String {
    let len = rng.gen_range(40..70); // Calculate len first
    rng.sample_iter(&Alphanumeric)
        .take(len)
        .map(char::from)
        .collect::<String>()
        + " w KQkq - 0 1"
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting game benchmark...");
    let db = setup_db().await?;
    let mut rng = thread_rng();

    // === Setup: Create Players ===
    println!("Creating {} players...", NUM_PLAYERS_TO_CREATE);
    let mut player_models = Vec::with_capacity(NUM_PLAYERS_TO_CREATE);
    for i in 0..NUM_PLAYERS_TO_CREATE {
        let player_id = Uuid::new_v4(); // Generate UUID here
        player_models.push(player::ActiveModel {
            id: Set(player_id), // Explicitly set the ID
            username: Set(format!("bench_user_{}_{}", i, Uuid::new_v4().simple())),
            email: Set(format!(
                "bench_email_{}_{}@bench.com",
                i,
                Uuid::new_v4().simple()
            )),
            password_hash: Set(b"bench_hash".to_vec()),
            biography: Set("Benchmark player biography".to_string()), // Provide a non-null value
            country: Set("Unknown".to_string()),                      // Add default
            flair: Set("Bench Flair".to_string()),                    // Add default
            real_name: Set("Bench Real Name".to_string()),            // Add default
            location: Set(Some("Bench Location".to_string())),        // Add default
            fide_rating: Set(Some(1500)),                             // Add default
            social_links: Set(Some(vec![])),                          // Add default (empty vec)
            ..Default::default()
        });
    }
    let _insert_res = Player::insert_many(player_models).exec(&db).await?;
    println!("Inserted {} players.", NUM_PLAYERS_TO_CREATE);

    // Fetch the IDs of the created players
    let players = Player::find()
        .filter(player::Column::Username.starts_with("bench_user_"))
        .limit(NUM_PLAYERS_TO_CREATE as u64)
        .all(&db)
        .await?;
    let player_ids: Vec<Uuid> = players.into_iter().map(|p| p.id).collect();

    if player_ids.len() < 2 {
        panic!("Need at least 2 players to create games");
    }
    println!("Fetched {} player IDs for game creation.", player_ids.len());

    // === Benchmark: Insertions ===
    println!(
        "Inserting {} games in batches of {}...",
        NUM_GAMES_TO_INSERT, BATCH_SIZE
    );
    let mut game_models = Vec::with_capacity(BATCH_SIZE);
    let variants = [
        GameVariant::Standard,
        GameVariant::Chess960,
        GameVariant::Blitz,
        GameVariant::Rapid,
        GameVariant::Classical,
    ]; // Update variants list
    let results = [
        ResultSide::WhiteWins,
        ResultSide::BlackWins,
        ResultSide::Draw,
    ]; // Update results list
    let insert_start = Instant::now();

    for i in 0..NUM_GAMES_TO_INSERT {
        let white_player_id = player_ids[rng.gen_range(0..player_ids.len())];
        let black_player_id = player_ids[rng.gen_range(0..player_ids.len())];
        let game_id = Uuid::new_v4(); // Generate UUID for the game

        game_models.push(game::ActiveModel {
            id: Set(game_id), // Explicitly set the game ID
            white_player: Set(white_player_id),
            black_player: Set(black_player_id),
            fen: Set(generate_random_fen(&mut rng)),
            pgn: Set(generate_random_pgn(&mut rng)),

            result: Set(Some(results[rng.gen_range(0..results.len())].clone())),
            variant: Set(variants[rng.gen_range(0..variants.len())].clone()),
            duration_sec: Set(rng.gen_range(30..600)),
            ..Default::default() // started_at has default
        });

        if game_models.len() >= BATCH_SIZE || i == NUM_GAMES_TO_INSERT - 1 {
            Game::insert_many(game_models.drain(..)).exec(&db).await?;
            if (i + 1) % (BATCH_SIZE * 10) == 0 {
                // Print progress
                println!("  Inserted {} games...", i + 1);
            }
        }
    }

    let insert_duration = insert_start.elapsed();
    println!(
        "Finished inserting {} games in {:.2?}. Average: {:.2} games/sec",
        NUM_GAMES_TO_INSERT,
        insert_duration,
        NUM_GAMES_TO_INSERT as f64 / insert_duration.as_secs_f64()
    );

    // Add a small delay to ensure data is queryable
    sleep(Duration::from_secs(1)).await;

    // === Benchmark: Queries ===
    println!("\nBenchmarking queries...");

    // 1. Query by Variant
    let query_variant = variants[rng.gen_range(0..variants.len())].clone();
    let query_start = Instant::now();
    let games_by_variant = Game::find()
        .filter(game::Column::Variant.eq(query_variant.clone()))
        .limit(1000) // Limit results for benchmark
        .all(&db)
        .await?;
    let query_duration = query_start.elapsed();
    println!(
        "- Query by variant ('{:?}'): Found {} games in {:.2?}",
        query_variant,
        games_by_variant.len(),
        query_duration
    );

    // 2. Query by StartedAt Range (e.g., last 10 seconds)
    // Note: This requires timezone handling or knowledge of DB timezone
    // For simplicity, using a placeholder query. A real benchmark might need
    // `chrono` and precise timestamp generation/querying.
    // Example using raw SQL for `NOW() - interval '10 second'` (PostgreSQL specific)
    let query_start = Instant::now();
    let games_recent = Game::find()
        .filter(Expr::cust("\"started_at\" > NOW() - interval '10 second'"))
        .limit(1000)
        .all(&db)
        .await?;
    let query_duration = query_start.elapsed();
    println!(
        "- Query by recent started_at (last 10s): Found {} games in {:.2?}",
        games_recent.len(),
        query_duration
    );

    // 3. Query PGN JSONB using GIN index (PostgreSQL specific operators)
    // Example: Find games where PGN contains the key "final_ply" with a value > 50
    let start_time = Instant::now();
    let games_by_pgn_content = Game::find()
        // Corrected filter to directly check the numeric value
        .filter(Expr::cust("(\"pgn\" ->> 'final_ply')::int > 50"))
        .all(&db)
        .await?;
    let duration = start_time.elapsed();
    println!(
        "Querying {} games by PGN content (final_ply > 50) took: {:?}",
        games_by_pgn_content.len(),
        duration
    );

    // === Cleanup (Optional but recommended) ===
    println!(
        "\nStarting cleanup (deleting benchmark games and players)... This might take a while."
    );
    let cleanup_start = Instant::now();

    // Delete games associated with the benchmark players
    let delete_games_res = Game::delete_many()
        .filter(
            game::Column::WhitePlayer
                .is_in(player_ids.clone())
                .or(game::Column::BlackPlayer.is_in(player_ids.clone())),
        )
        .exec(&db)
        .await?;
    println!("  Deleted {} game records.", delete_games_res.rows_affected);

    // Delete benchmark players
    let delete_players_res = Player::delete_many()
        .filter(player::Column::Username.starts_with("bench_user_"))
        .exec(&db)
        .await?;
    println!(
        "  Deleted {} player records.",
        delete_players_res.rows_affected
    );

    let cleanup_duration = cleanup_start.elapsed();
    println!("Cleanup finished in {:.2?}.", cleanup_duration);

    Ok(())
}
