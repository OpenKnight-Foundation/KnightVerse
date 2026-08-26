//! Integration tests for `DbPool` construction and routing behaviour.
//!
//! These tests do **not** require a live database.  They use `MockDatabase`
//! instances to verify that `DbPool` correctly exposes primary / replica
//! connections and that the pool-construction helpers work as expected.
//!
//! Tests that require a live PostgreSQL instance are gated by `DATABASE_URL`
//! and skip silently when absent.

use std::sync::Arc;

use db::db::db::DbPool;
use sea_orm::{ConnectionTrait, DatabaseBackend, MockDatabase, Statement};

// =============================================================================
// Construction helpers
// =============================================================================

/// In single-pool mode (has_replica = false), primary and replica are the same
/// underlying Arc.
#[test]
fn single_pool_primary_and_replica_are_same_pointer() {
    let db = Arc::new(MockDatabase::new(DbBackend::Postgres).into_connection());
    let pool = DbPool::from_connections(db.clone(), db.clone(), false);

    assert!(
        std::ptr::eq(pool.primary(), pool.replica()),
        "single-pool: primary and replica should be the same connection"
    );
    assert!(!pool.has_replica());
}

/// In dual-pool mode (has_replica = true), primary and replica are distinct.
#[test]
fn dual_pool_primary_and_replica_are_distinct_pointers() {
    let primary = Arc::new(MockDatabase::new(DbBackend::Postgres).into_connection());
    let replica = Arc::new(MockDatabase::new(DbBackend::Postgres).into_connection());
    let pool = DbPool::from_connections(primary, replica, true);

    assert!(
        !std::ptr::eq(pool.primary(), pool.replica()),
        "dual-pool: primary and replica should be distinct connections"
    );
    assert!(pool.has_replica());
}

/// `into_connections` returns the arcs in (primary, replica) order.
#[test]
fn into_connections_returns_both_arcs() {
    let primary = Arc::new(MockDatabase::new(DbBackend::Postgres).into_connection());
    let replica = Arc::new(MockDatabase::new(DbBackend::Postgres).into_connection());

    let primary_ptr = Arc::as_ptr(&primary);
    let replica_ptr = Arc::as_ptr(&replica);

    let pool = DbPool::from_connections(primary, replica, true);
    let (p_out, r_out) = pool.into_connections();

    assert_eq!(Arc::as_ptr(&p_out), primary_ptr);
    assert_eq!(Arc::as_ptr(&r_out), replica_ptr);
}

// =============================================================================
// Metrics
// =============================================================================

/// `update_metrics()` must not panic.  On MockDatabase it may be a no-op
/// because sqlx pool introspection is unavailable, but it must not crash.
#[test]
fn update_metrics_is_infallible() {
    let db = Arc::new(MockDatabase::new(DbBackend::Postgres).into_connection());
    let pool = DbPool::from_connections(db.clone(), db, false);
    pool.update_metrics(); // must not panic
}

// =============================================================================
// Mock query routing
// =============================================================================

/// Queries issued via `pool.replica()` reach only the replica connection.
#[tokio::test]
async fn query_on_replica_does_not_touch_primary() {
    let replica = MockDatabase::new(DbBackend::Postgres)
        .append_exec_results(vec![sea_orm::MockExecResult {
            last_insert_id: 0,
            rows_affected: 0,
        }])
        .into_connection();

    let primary = MockDatabase::new(DbBackend::Postgres).into_connection();

    let pool = DbPool::from_connections(Arc::new(primary), Arc::new(replica), true);

    // Issue a query on the replica
    let _ = pool
        .replica()
        .execute(Statement::from_string(
            DatabaseBackend::Postgres,
            "SELECT 1".to_string(),
        ))
        .await;

    let (primary_conn, replica_conn) = pool.into_connections();
    assert!(
        primary_conn.into_transaction_log().is_empty(),
        "primary should NOT have been touched when querying replica"
    );
    assert!(
        !replica_conn.into_transaction_log().is_empty(),
        "replica should have received the query"
    );
}

/// Queries issued via `pool.primary()` reach only the primary connection.
#[tokio::test]
async fn query_on_primary_does_not_touch_replica() {
    let primary = MockDatabase::new(DbBackend::Postgres)
        .append_exec_results(vec![sea_orm::MockExecResult {
            last_insert_id: 0,
            rows_affected: 0,
        }])
        .into_connection();

    let replica = MockDatabase::new(DbBackend::Postgres).into_connection();

    let pool = DbPool::from_connections(Arc::new(primary), Arc::new(replica), true);

    let _ = pool
        .primary()
        .execute(Statement::from_string(
            DatabaseBackend::Postgres,
            "INSERT INTO test VALUES (1)".to_string(),
        ))
        .await;

    let (primary_conn, replica_conn) = pool.into_connections();
    assert!(
        !primary_conn.into_transaction_log().is_empty(),
        "primary should have received the query"
    );
    assert!(
        replica_conn.into_transaction_log().is_empty(),
        "replica should NOT have been touched when querying primary"
    );
}

// =============================================================================
// Live database smoke test (skips when DATABASE_URL is absent)
// =============================================================================

/// When DATABASE_URL is present, `DbPool::from_env()` should connect and allow
/// a trivial query against the primary.
#[tokio::test]
async fn from_env_connects_and_queries_primary_live() {
    if std::env::var("DATABASE_URL").is_err() {
        return; // skip — no live database available
    }

    let pool = DbPool::from_env().await;
    let result = pool
        .primary()
        .query_one(Statement::from_string(
            DatabaseBackend::Postgres,
            "SELECT 1".to_string(),
        ))
        .await;

    assert!(
        result.is_ok(),
        "live primary query should succeed: {:?}",
        result.err()
    );
}
