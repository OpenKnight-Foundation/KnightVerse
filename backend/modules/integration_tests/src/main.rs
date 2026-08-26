use std::collections::HashMap;
use uuid::Uuid;
use chrono::Utc;
use log::info;

// Import all the modules we need to test
use metrics::{MetricsCollector, GameMetrics, UserMetrics, TournamentMetrics};
use tournament::{Tournament, TournamentFormat, BracketConfig, TournamentStatus};
use validation::{RealTimeMoveValidator, MoveValidationRequest, ValidationError};
use archiving::{PGNArchiver, PGNGame, ArchiveRequest, ArchiveNetwork, ArchiveMetadata, GameMetadata, TimeControlCategory, GameType};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    
    info!("Starting XLMate Backend Integration Tests");
    
    // Test 1: Metrics Collection
    test_metrics_integration().await?;
    
    // Test 2: Tournament Management
    test_tournament_integration().await?;
    
    // Test 3: Move Validation
    test_validation_integration().await?;
    
    // Test 4: PGN Archiving
    test_archiving_integration().await?;
    
    // Test 5: End-to-End Workflow
    test_end_to_end_workflow().await?;
    
    info!("All integration tests completed successfully!");
    Ok(())
}

async fn test_metrics_integration() -> Result<(), Box<dyn std::error::Error>> {
    info!("Testing Metrics Integration");
    
    let metrics = MetricsCollector::new();
    
    // Test game metrics
    metrics.inc_games_created();
    metrics.inc_moves_made();
    metrics.inc_games_completed("white_wins");
    metrics.set_active_games(5.0);
    
    // Test user metrics
    metrics.inc_users_registered();
    metrics.set_active_users(10.0);
    
    // Test tournament metrics
    metrics.inc_tournaments_created();
    metrics.set_active_tournaments(2.0);
    metrics.set_tournament_participants(15.0);
    
    // Export and verify metrics
    let exported = metrics.export()?;
    assert!(exported.contains("games_created_total"));
    assert!(exported.contains("users_registered_total"));
    assert!(exported.contains("tournaments_created_total"));
    
    info!("✅ Metrics integration test passed");
    Ok(())
}

async fn test_tournament_integration() -> Result<(), Box<dyn std::error::Error>> {
    info!("Testing Tournament Integration");
    
    let config = BracketConfig {
        tournament_id: Uuid::new_v4(),
        format: TournamentFormat::Swiss,
        total_rounds: 3,
        pairings_per_round: 0,
        auto_pair: true,
        pair_immediately: false,
        allow_byes: true,
        color_alternation: true,
        rating_sort: true,
    };
    
    let mut tournament = Tournament::new(
        "Integration Test Tournament".to_string(),
        TournamentFormat::Swiss,
        config,
        "5+0".to_string(),
    );
    
    // Add participants
    let player1 = Uuid::new_v4();
    let player2 = Uuid::new_v4();
    let player3 = Uuid::new_v4();
    
    tournament.add_participant(player1, "Alice".to_string(), 1500)?;
    tournament.add_participant(player2, "Bob".to_string(), 1600)?;
    tournament.add_participant(player3, "Charlie".to_string(), 1400)?;
    
    assert_eq!(tournament.participants.len(), 3);
    assert_eq!(tournament.status, TournamentStatus::Registration);
    
    // Start tournament
    tournament.start_tournament()?;
    assert_eq!(tournament.status, TournamentStatus::InProgress);
    assert_eq!(tournament.current_round, 1);
    
    // Check that pairings were generated
    assert!(!tournament.pairings.is_empty());
    
    // Get tournament stats
    let stats = tournament.get_tournament_stats();
    assert_eq!(stats.total_participants, 3);
    assert_eq!(stats.active_participants, 3);
    
    info!("✅ Tournament integration test passed");
    Ok(())
}

async fn test_validation_integration() -> Result<(), Box<dyn std::error::Error>> {
    info!("Testing Validation Integration");
    
    // Note: This test uses a mock Redis setup since we can't connect to a real Redis instance
    // In a real environment, you would use a test Redis instance
    
    let validator = RealTimeMoveValidator::with_config(
        "redis://localhost:6379", // This will fail in test but shows the structure
        300, // cache_ttl_seconds
        100, // max_batch_size
        30,  // rate_limit_per_minute
    );
    
    let request = MoveValidationRequest {
        game_id: Uuid::new_v4(),
        player_id: Uuid::new_v4(),
        move_notation: "e4".to_string(),
        timestamp: Utc::now(),
        client_version: Some("1.0.0".to_string()),
    };
    
    // Test move notation parsing (this doesn't require Redis)
    let processed_move = validator.process_move_notation("e4").await?;
    assert_eq!(processed_move.piece, "P");
    assert_eq!(processed_move.to_square, "e4");
    assert!(!processed_move.is_capture);
    
    // Test castling
    let castle_move = validator.process_move_notation("O-O").await?;
    assert_eq!(castle_move.piece, "K");
    assert!(castle_move.is_castling);
    assert_eq!(castle_move.from_square, "e1");
    assert_eq!(castle_move.to_square, "g1");
    
    // Test UCI notation
    let uci_move = validator.process_move_notation("e2e4").await?;
    assert_eq!(uci_move.from_square, "e2");
    assert_eq!(uci_move.to_square, "e4");
    assert_eq!(uci_move.uci, "e2e4");
    
    info!("✅ Validation integration test passed");
    Ok(())
}

async fn test_archiving_integration() -> Result<(), Box<dyn std::error::Error>> {
    info!("Testing Archiving Integration");
    
    // Create a sample PGN game
    let pgn_game = PGNGame {
        id: Uuid::new_v4(),
        event: "Integration Test Game".to_string(),
        site: "XLMate".to_string(),
        date: "2024.01.01".to_string(),
        round: "1".to_string(),
        white: "TestPlayer1".to_string(),
        black: "TestPlayer2".to_string(),
        result: "1-0".to_string(),
        white_elo: Some(1500),
        black_elo: Some(1400),
        time_control: "5+0".to_string(),
        eco: Some("C20".to_string()),
        opening: Some("King's Pawn Game".to_string()),
        moves: vec![
            "e4".to_string(),
            "e5".to_string(),
            "Nf3".to_string(),
            "Nc6".to_string(),
            "Bb5".to_string(),
            "a6".to_string(),
        ],
        annotations: None,
        metadata: GameMetadata {
            game_id: Uuid::new_v4(),
            tournament_id: None,
            created_at: Utc::now(),
            completed_at: Utc::now(),
            duration_seconds: 300,
            total_moves: 6,
            average_move_time: 50.0,
            time_control_category: TimeControlCategory::Blitz,
            game_type: GameType::Rated,
        },
    };
    
    // Test PGN to string conversion
    let archiver = PGNArchiver::new(
        sea_orm::Database::connect("sqlite::memory:").await?,
        "https://ipfs.infura.io:5001".to_string(),
        "https://arweave.net".to_string(),
    );
    
    let pgn_string = archiver.pgn_to_string(&pgn_game)?;
    assert!(pgn_string.contains("[Event \"Integration Test Game\"]"));
    assert!(pgn_string.contains("[White \"TestPlayer1\"]"));
    assert!(pgn_string.contains("1. e4 e5"));
    
    // Test hash calculation
    let hash = archiver.calculate_hash(pgn_string.as_bytes());
    assert_eq!(hash.len(), 64); // SHA256 hex string
    
    // Test cost estimation
    let ipfs_cost = archiver.estimate_ipfs_cost(pgn_string.len() as u64);
    let arweave_cost = archiver.estimate_arwear_cost(pgn_string.len() as u64);
    assert!(ipfs_cost > 0.0);
    assert!(arweave_cost > 0.0);
    
    info!("✅ Archiving integration test passed");
    Ok(())
}

async fn test_end_to_end_workflow() -> Result<(), Box<dyn std::error::Error>> {
    info!("Testing End-to-End Workflow");
    
    // 1. Initialize metrics collector
    let metrics = MetricsCollector::new();
    
    // 2. Create and start a tournament
    let config = BracketConfig::default();
    let mut tournament = Tournament::new(
        "E2E Test Tournament".to_string(),
        TournamentFormat::Swiss,
        config,
        "3+0".to_string(),
    );
    
    let player1_id = Uuid::new_v4();
    let player2_id = Uuid::new_v4();
    
    tournament.add_participant(player1_id, "Player1".to_string(), 1500)?;
    tournament.add_participant(player2_id, "Player2".to_string(), 1600)?;
    
    tournament.start_tournament()?;
    
    // 3. Record metrics for tournament creation
    metrics.inc_tournaments_created();
    metrics.inc_users_registered();
    metrics.inc_users_registered();
    metrics.set_active_tournaments(1.0);
    metrics.set_tournament_participants(2.0);
    
    // 4. Simulate game play with move validation
    let validator = RealTimeMoveValidator::with_config(
        "redis://localhost:6379",
        300,
        100,
        30,
    );
    
    let game_moves = vec!["e4", "e5", "Nf3", "Nc6"];
    
    for (i, move_notation) in game_moves.iter().enumerate() {
        let request = MoveValidationRequest {
            game_id: Uuid::new_v4(),
            player_id: if i % 2 == 0 { player1_id } else { player2_id },
            move_notation: move_notation.to_string(),
            timestamp: Utc::now(),
            client_version: Some("1.0.0".to_string()),
        };
        
        // Parse the move (we can't validate without Redis, but we can parse)
        let _processed = validator.process_move_notation(move_notation).await?;
        metrics.inc_moves_made();
    }
    
    // 5. Create PGN record for the game
    let pgn_game = PGNGame {
        id: Uuid::new_v4(),
        event: "E2E Test Game".to_string(),
        site: "XLMate".to_string(),
        date: Utc::now().format("%Y.%m.%d").to_string(),
        round: "1".to_string(),
        white: "Player1".to_string(),
        black: "Player2".to_string(),
        result: "1-0".to_string(),
        white_elo: Some(1500),
        black_elo: Some(1600),
        time_control: "3+0".to_string(),
        eco: Some("C20".to_string()),
        opening: Some("King's Pawn Game".to_string()),
        moves: game_moves.into_iter().map(|m| m.to_string()).collect(),
        annotations: None,
        metadata: GameMetadata {
            game_id: Uuid::new_v4(),
            tournament_id: Some(tournament.id),
            created_at: Utc::now(),
            completed_at: Utc::now(),
            duration_seconds: 180,
            total_moves: 4,
            average_move_time: 45.0,
            time_control_category: TimeControlCategory::Blitz,
            game_type: GameType::Tournament,
        },
    };
    
    // 6. Archive the game
    let archiver = PGNArchiver::new(
        sea_orm::Database::connect("sqlite::memory:").await?,
        "https://ipfs.infura.io:5001".to_string(),
        "https://arweave.net".to_string(),
    );
    
    let archive_request = ArchiveRequest {
        game_id: pgn_game.id,
        pgn_data: pgn_game.clone(),
        archive_immediately: false, // Don't actually upload in test
        preferred_network: ArchiveNetwork::IPFS,
        metadata: ArchiveMetadata {
            tags: vec!["tournament".to_string(), "blitz".to_string()],
            description: Some("End-to-end test game".to_string()),
            visibility: archiving::ArchiveVisibility::Public,
            encryption_key: None,
            compression: true,
        },
    };
    
    // Test PGN conversion (without actual upload)
    let pgn_string = archiver.pgn_to_string(&pgn_game)?;
    assert!(!pgn_string.is_empty());
    
    // 7. Update final metrics
    metrics.inc_games_created();
    metrics.inc_games_completed("white_wins");
    metrics.set_active_games(1.0);
    
    // 8. Verify all components are working
    let exported_metrics = metrics.export()?;
    assert!(exported.contains("games_created_total"));
    assert!(exported.contains("moves_made_total"));
    assert!(exported.contains("tournaments_created_total"));
    
    assert_eq!(tournament.status, TournamentStatus::InProgress);
    assert_eq!(tournament.participants.len(), 2);
    
    info!("✅ End-to-end workflow test passed");
    Ok(())
}
