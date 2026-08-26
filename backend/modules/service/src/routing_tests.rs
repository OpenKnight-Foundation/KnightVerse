//! Service-layer pool-routing integration tests.
//!
//! Verifies that `GameService` and `PlayerService` route reads to the replica
//! and writes to the primary by inspecting `MockDatabase` transaction logs.

#[cfg(test)]
mod game_service_routing {
    use std::sync::Arc;

    use chrono::{FixedOffset, Utc};
    use db::db::db::DbPool;
    use db_entity::game;
    use dto::games::{CreateGameRequest, GameStatus};
    use sea_orm::{DbBackend, MockDatabase};
    use uuid::Uuid;

    use crate::games::GameService;

    fn mock_game() -> game::Model {
        let now = Utc::now().with_timezone(&FixedOffset::east_opt(0).unwrap());
        game::Model {
            id: Uuid::new_v4(),
            white_player: Uuid::new_v4(),
            black_player: Uuid::new_v4(),
            fen: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".to_string(),
            pgn: serde_json::json!([]),
            result: None,
            variant: db_entity::game::GameVariant::Standard,
            started_at: now,
            duration_sec: 600,
            created_at: now,
            updated_at: now,
            is_imported: false,
            original_pgn: None,
        }
    }

    // -----------------------------------------------------------------
    // Reads → replica
    // -----------------------------------------------------------------

    #[tokio::test]
    async fn get_game_routes_to_replica() {
        let game = mock_game();
        let game_id = game.id;

        let replica = MockDatabase::new(DbBackend::Postgres)
            .append_query_results(vec![vec![game]])
            .into_connection();
        let primary = MockDatabase::new(DbBackend::Postgres).into_connection();

        let pool = DbPool::from_connections(Arc::new(primary), Arc::new(replica), true);

        let result = GameService::get_game(&pool, game_id).await;
        assert!(result.is_ok(), "get_game should succeed: {:?}", result.err());

        let (p, r) = pool.into_connections();
        assert!(p.into_transaction_log().is_empty(), "primary must not be touched for a read");
        assert!(!r.into_transaction_log().is_empty(), "replica must receive the SELECT");
    }

    #[tokio::test]
    async fn list_games_routes_to_replica() {
        let game = mock_game();

        let replica = MockDatabase::new(DbBackend::Postgres)
            .append_query_results(vec![Vec::<game::Model>::new()]) // count
            .append_query_results(vec![vec![game]])                // data
            .into_connection();
        let primary = MockDatabase::new(DbBackend::Postgres).into_connection();

        let pool = DbPool::from_connections(Arc::new(primary), Arc::new(replica), true);
        let result = GameService::list_games(&pool, None, None, 10, None, None).await;
        assert!(result.is_ok());

        let (p, r) = pool.into_connections();
        assert!(p.into_transaction_log().is_empty(), "primary not touched for list_games");
        assert!(!r.into_transaction_log().is_empty(), "replica receives list_games queries");
    }

    #[tokio::test]
    async fn get_game_history_routes_to_replica() {
        let game = mock_game();
        let player_id = game.white_player;

        let replica = MockDatabase::new(DbBackend::Postgres)
            .append_query_results(vec![Vec::<game::Model>::new()]) // count
            .append_query_results(vec![vec![game]])                // data
            .into_connection();
        let primary = MockDatabase::new(DbBackend::Postgres).into_connection();

        let pool = DbPool::from_connections(Arc::new(primary), Arc::new(replica), true);
        let result = GameService::get_game_history(&pool, player_id, 20, None).await;
        assert!(result.is_ok(), "get_game_history: {:?}", result.err());

        let (p, r) = pool.into_connections();
        assert!(p.into_transaction_log().is_empty(), "primary not touched for game history");
        assert!(!r.into_transaction_log().is_empty(), "replica receives game history query");
    }

    // -----------------------------------------------------------------
    // Writes → primary
    // -----------------------------------------------------------------

    #[tokio::test]
    async fn create_game_routes_to_primary() {
        let game = mock_game();
        let creator = game.white_player;

        let primary = MockDatabase::new(DbBackend::Postgres)
            .append_query_results(vec![vec![game]])
            .into_connection();
        let replica = MockDatabase::new(DbBackend::Postgres).into_connection();

        let pool = DbPool::from_connections(Arc::new(primary), Arc::new(replica), true);
        let request = CreateGameRequest { time_control: 600, variant: None };
        let result = GameService::create_game(&pool, creator, request).await;
        assert!(result.is_ok(), "create_game: {:?}", result.err());

        let (p, r) = pool.into_connections();
        assert!(!p.into_transaction_log().is_empty(), "primary must receive INSERT");
        assert!(r.into_transaction_log().is_empty(), "replica must not be touched for writes");
    }

    // -----------------------------------------------------------------
    // Fallback: single-pool mode
    // -----------------------------------------------------------------

    #[tokio::test]
    async fn single_pool_fallback_reads_and_writes_succeed() {
        let game = mock_game();
        let game_id = game.id;
        let creator = game.white_player;
        let game2 = mock_game();

        let single = MockDatabase::new(DbBackend::Postgres)
            .append_query_results(vec![vec![game]])  // get_game SELECT
            .append_query_results(vec![vec![game2]]) // create_game INSERT
            .into_connection();

        let arc = Arc::new(single);
        let pool = DbPool::from_connections(arc.clone(), arc, false);

        assert!(GameService::get_game(&pool, game_id).await.is_ok());
        let request = CreateGameRequest { time_control: 300, variant: None };
        assert!(GameService::create_game(&pool, creator, request).await.is_ok());
    }

    // -----------------------------------------------------------------
    // Status filter routing (analytic read → replica)
    // -----------------------------------------------------------------

    #[tokio::test]
    async fn list_games_with_status_filter_routes_to_replica() {
        let game = mock_game();

        let replica = MockDatabase::new(DbBackend::Postgres)
            .append_query_results(vec![Vec::<game::Model>::new()]) // count
            .append_query_results(vec![vec![game]])                // data
            .into_connection();
        let primary = MockDatabase::new(DbBackend::Postgres).into_connection();

        let pool = DbPool::from_connections(Arc::new(primary), Arc::new(replica), true);
        let result = GameService::list_games(
            &pool,
            None,
            None,
            5,
            None,
            Some(GameStatus::Waiting),
        )
        .await;
        assert!(result.is_ok());

        let (p, r) = pool.into_connections();
        let r_log = r.into_transaction_log();
        assert!(!r_log.is_empty(), "replica receives filtered list query");
        assert!(p.into_transaction_log().is_empty(), "primary not touched");

        // Verify the SQL contains the status filter
        let sql = format!("{:?}", &r_log[0]);
        assert!(
            sql.contains("result") || sql.contains("IS NULL"),
            "status filter should reference the result column"
        );
    }
}

#[cfg(test)]
mod player_service_routing {
    use std::sync::Arc;

    use db::db::db::DbPool;
    use db_entity::player;
    use dto::players::NewPlayer;
    use sea_orm::{DbBackend, MockDatabase};
    use uuid::Uuid;

    use crate::players::{add_player, find_player_by_id};

    fn mock_player(username: &str) -> player::Model {
        player::Model {
            id: Uuid::new_v4(),
            username: username.to_string(),
            email: format!("{}@example.com", username),
            password_hash: b"fake_bcrypt_hash".to_vec(),
            biography: String::new(),
            country: String::new(),
            flair: String::new(),
            real_name: String::new(),
            location: None,
            fide_rating: None,
            elo_rating: 1200,
            social_links: None,
            is_enabled: true,
        }
    }

    // -----------------------------------------------------------------
    // Reads → replica
    // -----------------------------------------------------------------

    #[tokio::test]
    async fn find_player_by_id_routes_to_replica() {
        let player = mock_player("alice");
        let player_id = player.id;

        let replica = MockDatabase::new(DbBackend::Postgres)
            .append_query_results(vec![vec![player]])
            .into_connection();
        let primary = MockDatabase::new(DbBackend::Postgres)
            .append_query_results(vec![Vec::<player::Model>::new()])
            .into_connection();

        let pool = DbPool::from_connections(Arc::new(primary), Arc::new(replica), true);
        let result = find_player_by_id(&pool, player_id).await;
        assert!(result.is_ok(), "find_player should succeed: {:?}", result.err());

        let (p, r) = pool.into_connections();
        assert!(!r.into_transaction_log().is_empty(), "replica should be queried");
        assert!(p.into_transaction_log().is_empty(), "primary should not be touched for read");
    }

    // -----------------------------------------------------------------
    // Writes → primary
    // -----------------------------------------------------------------

    #[tokio::test]
    async fn add_player_routes_insert_to_primary() {
        let player = mock_player("bob");
        let player_clone = player.clone();

        // Replica handles uniqueness checks (2 SELECTs)
        let replica = MockDatabase::new(DbBackend::Postgres)
            .append_query_results(vec![Vec::<player::Model>::new()]) // username check
            .append_query_results(vec![Vec::<player::Model>::new()]) // email check
            .into_connection();

        // Primary handles the INSERT
        let primary = MockDatabase::new(DbBackend::Postgres)
            .append_query_results(vec![vec![player_clone]])
            .into_connection();

        let pool = DbPool::from_connections(Arc::new(primary), Arc::new(replica), true);

        let payload = NewPlayer {
            username: "bob".to_string(),
            email: "bob@example.com".to_string(),
            password: "S3cure!Pass".to_string(),
            real_name: String::new(),
        };

        let result = add_player(&pool, payload).await;
        assert!(result.is_ok(), "add_player: {:?}", result.err());

        let (p, r) = pool.into_connections();

        let p_log = p.into_transaction_log();
        let r_log = r.into_transaction_log();

        assert!(!p_log.is_empty(), "primary should receive the INSERT");
        assert!(!r_log.is_empty(), "replica should receive uniqueness-check SELECTs");

        let insert_sql = format!("{:?}", &p_log[0]);
        assert!(
            insert_sql.contains("INSERT") || insert_sql.contains("insert"),
            "primary query should be an INSERT, got: {}",
            insert_sql
        );
    }
}
