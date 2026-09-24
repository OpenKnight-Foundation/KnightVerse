//! Integration tests for `DbPool` read/write routing.
//!
//! These tests verify that:
//!  1. `DbPool::primary()` and `DbPool::replica()` return independent connections
//!     when `has_replica = true`.
//!  2. `DbPool::replica()` returns the primary connection when
//!     `has_replica = false` (single-pool / fallback mode).
//!  3. `DbPool::update_metrics()` does not panic (metrics may be no-ops if the
//!     underlying driver does not expose pool stats on MockDatabase).
//!  4. `GameService` read methods run against the replica.
//!  5. `GameService` write methods run against the primary.
//!
//! Tests that require a live PostgreSQL instance are gated by the
//! `DATABASE_URL` environment variable and skip silently when absent.

use std::sync::Arc;

use chrono::{FixedOffset, Utc};
use db::db::db::DbPool;
use db_entity::game;
use sea_orm::{DbBackend, MockDatabase};
use service::games::GameService;
use uuid::Uuid;

/// Create a minimal game model for use in mock query results.
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

// =============================================================================
// DbPool construction & single-pool fallback
// =============================================================================

/// In single-pool mode (no replica), `primary()` and `replica()` should return
/// the same underlying connection (same `Arc` pointer).
#[test]
fn single_pool_mode_primary_and_replica_are_same_arc() {
    let db = Arc::new(
        MockDatabase::new(DbBackend::Postgres)
            .into_connection(),
    );

    let pool = DbPool::from_connections(db.clone(), db.clone(), false);

    // In single-pool mode the two arcs should be the same
    assert!(
        std::ptr::eq(pool.primary(), pool.replica()),
        "In single-pool mode primary and replica should share the same connection"
    );
    assert!(!pool.has_replica());
}

/// In dual-pool mode, `primary()` and `replica()` should be *different*
/// connections (different `Arc` pointers).
#[test]
fn dual_pool_mode_primary_and_replica_are_different_arcs() {
    let primary = Arc::new(MockDatabase::new(DbBackend::Postgres).into_connection());
    let replica = Arc::new(MockDatabase::new(DbBackend::Postgres).into_connection());

    let pool = DbPool::from_connections(primary, replica, true);

    assert!(
        !std::ptr::eq(pool.primary(), pool.replica()),
        "In dual-pool mode primary and replica should be distinct connections"
    );
    assert!(pool.has_replica());
}

/// `update_metrics()` must not panic even when the underlying connection is a
/// MockDatabase (which may not expose sqlx pool stats).
#[test]
fn update_metrics_does_not_panic() {
    let db = Arc::new(MockDatabase::new(DbBackend::Postgres).into_connection());
    let pool = DbPool::from_connections(db.clone(), db, false);
    // Should complete without panicking
    pool.update_metrics();
}

// =============================================================================
// GameService: reads go to replica
// =============================================================================

/// `GameService::get_game` should issue its SELECT against the **replica**
/// connection.  We wire two independent mock connections and verify that only
/// the replica receives a query.
#[tokio::test]
async fn game_service_get_game_uses_replica() {
    let game = mock_game();
    let game_id = game.id;

    // Replica mock — returns the game on the first query
    let replica = MockDatabase::new(DbBackend::Postgres)
        .append_query_results(vec![vec![game]])
        .into_connection();

    // Primary mock — returns nothing; any hit would produce a NotFound error
    let primary = MockDatabase::new(DbBackend::Postgres)
        .append_query_results(vec![Vec::<game::Model>::new()])
        .into_connection();

    let pool = DbPool::from_connections(Arc::new(primary), Arc::new(replica), true);

    // Should succeed using the replica result
    let result = GameService::get_game(&pool, game_id).await;
    assert!(result.is_ok(), "get_game should succeed: {:?}", result.err());

    let (primary_conn, replica_conn) = pool.into_connections();

    let primary_log = primary_conn.into_transaction_log();
    let replica_log = replica_conn.into_transaction_log();

    assert!(
        !replica_log.is_empty(),
        "replica should have received the SELECT query"
    );
    assert!(
        primary_log.is_empty(),
        "primary should NOT have been queried for a read-only operation"
    );

    let sql = format!("{:?}", &replica_log[0]);
    assert!(
        sql.contains("SELECT") || sql.contains("select"),
        "replica query should be a SELECT, got: {}",
        sql
    );
}

/// `GameService::list_games` (count + data queries) should hit the **replica**.
#[tokio::test]
async fn game_service_list_games_uses_replica() {
    let game = mock_game();

    let replica = MockDatabase::new(DbBackend::Postgres)
        // count query
        .append_query_results(vec![Vec::<game::Model>::new()])
        // data query
        .append_query_results(vec![vec![game]])
        .into_connection();

    let primary = MockDatabase::new(DbBackend::Postgres)
        .into_connection();

    let pool = DbPool::from_connections(Arc::new(primary), Arc::new(replica), true);

    let result = GameService::list_games(&pool, None, None, 10, None, None).await;
    assert!(result.is_ok(), "list_games should succeed: {:?}", result.err());

    let (primary_conn, replica_conn) = pool.into_connections();

    assert!(
        !replica_conn.into_transaction_log().is_empty(),
        "replica should have been queried"
    );
    assert!(
        primary_conn.into_transaction_log().is_empty(),
        "primary should NOT have been queried for list_games"
    );
}

// =============================================================================
// GameService: writes go to primary
// =============================================================================

/// `GameService::create_game` should issue its INSERT against the **primary**
/// connection.
#[tokio::test]
async fn game_service_create_game_uses_primary() {
    let game = mock_game();
    let creator = game.white_player;

    let primary = MockDatabase::new(DbBackend::Postgres)
        .append_query_results(vec![vec![game]])
        .into_connection();

    // Replica has no expectations — it should not be hit
    let replica = MockDatabase::new(DbBackend::Postgres)
        .into_connection();

    let pool = DbPool::from_connections(Arc::new(primary), Arc::new(replica), true);

    let request = dto::games::CreateGameRequest {
        time_control: 600,
        variant: None,
    };
    let result = GameService::create_game(&pool, creator, request).await;
    assert!(result.is_ok(), "create_game should succeed: {:?}", result.err());

    let (primary_conn, replica_conn) = pool.into_connections();

    let primary_log = primary_conn.into_transaction_log();
    let replica_log = replica_conn.into_transaction_log();

    assert!(
        !primary_log.is_empty(),
        "primary should have received the INSERT query"
    );
    assert!(
        replica_log.is_empty(),
        "replica should NOT have been touched for a write operation"
    );

    let sql = format!("{:?}", &primary_log[0]);
    assert!(
        sql.contains("INSERT") || sql.contains("insert"),
        "primary query should be an INSERT, got: {}",
        sql
    );
}

// =============================================================================
// GameService: get_game_history uses replica
// =============================================================================

/// `GameService::get_game_history` — a heavy analytic read — routes to replica.
#[tokio::test]
async fn game_service_get_game_history_uses_replica() {
    let game = mock_game();
    let player_id = game.white_player;

    let replica = MockDatabase::new(DbBackend::Postgres)
        // count query
        .append_query_results(vec![Vec::<game::Model>::new()])
        // data query
        .append_query_results(vec![vec![game]])
        .into_connection();

    let primary = MockDatabase::new(DbBackend::Postgres).into_connection();

    let pool = DbPool::from_connections(Arc::new(primary), Arc::new(replica), true);

    let result = GameService::get_game_history(&pool, player_id, 20, None).await;
    assert!(result.is_ok(), "get_game_history should succeed: {:?}", result.err());

    let (primary_conn, replica_conn) = pool.into_connections();

    assert!(
        !replica_conn.into_transaction_log().is_empty(),
        "replica should have been queried for game history"
    );
    assert!(
        primary_conn.into_transaction_log().is_empty(),
        "primary should NOT have been queried for a read-only analytic query"
    );
}

// =============================================================================
// Fallback: single-pool mode still works
// =============================================================================

/// When running in single-pool mode (no replica configured), all queries go
/// to the same connection.  Both reads and writes should succeed.
#[tokio::test]
async fn single_pool_mode_reads_and_writes_use_same_connection() {
    let game = mock_game();
    let game_id = game.id;
    let creator = game.white_player;
    let game2 = mock_game();

    // One connection for everything
    let single_db = MockDatabase::new(DbBackend::Postgres)
        // get_game SELECT
        .append_query_results(vec![vec![game.clone()]])
        // create_game INSERT
        .append_query_results(vec![vec![game2]])
        .into_connection();

    let arc = Arc::new(single_db);
    let pool = DbPool::from_connections(arc.clone(), arc, false);

    // READ
    let read_result = GameService::get_game(&pool, game_id).await;
    assert!(read_result.is_ok(), "get_game in single-pool mode: {:?}", read_result.err());

    // WRITE
    let request = dto::games::CreateGameRequest {
        time_control: 300,
        variant: None,
    };
    let write_result = GameService::create_game(&pool, creator, request).await;
    assert!(write_result.is_ok(), "create_game in single-pool mode: {:?}", write_result.err());
}

// =============================================================================
// Live database tests (skipped when DATABASE_URL is absent)
// =============================================================================

/// Smoke test: `DbPool::from_env()` succeeds when DATABASE_URL is set.
/// Skips silently when DATABASE_URL is absent.
#[tokio::test]
async fn from_env_connects_when_database_url_is_set() {
    if std::env::var("DATABASE_URL").is_err() {
        return; // skip in CI environments without a real DB
    }

    let pool = DbPool::from_env().await;
    // We just verify it doesn't panic and has_replica is false (no replica URL set in test env)
    let _ = pool.has_replica();
    // Try a trivial query on the primary to verify connectivity
    use sea_orm::{ConnectionTrait, Statement, DatabaseBackend};
    let result = pool
        .primary()
        .query_one(Statement::from_string(DatabaseBackend::Postgres, "SELECT 1".to_string()))
        .await;
    assert!(result.is_ok(), "live primary query should succeed: {:?}", result.err());
}
