//! Database connection pool with read-replica routing.
//!
//! # Architecture
//!
//! ```text
//!   Write path  ─────────────────────►  primary_pool  (read-write)
//!   Read path   ──► replica_pool ───►  replica_pool   (read-only)
//!                  (if configured)
//!                └─ fallback ────────►  primary_pool
//! ```
//!
//! # Environment Variables
//! * `DATABASE_URL`         – Required. Primary (read-write) PostgreSQL DSN.
//! * `DATABASE_REPLICA_URL` – Optional. Read-only replica DSN. When absent the
//!                            pool operates in single-node mode and all queries
//!                            hit the primary.
//!
//! # Prometheus Metrics
//! Three gauge families are exported under the `db_pool_` prefix:
//! * `db_pool_connections_active`  – connections currently checked out
//! * `db_pool_connections_idle`    – connections waiting in the pool
//! * `db_pool_connections_max`     – configured pool ceiling
//!
//! Each metric carries a `pool` label with values `"primary"` or `"replica"`.

#[allow(clippy::module_inception)]
pub mod db {
    use once_cell::sync::Lazy;
    use prometheus::{register_int_gauge_vec, IntGaugeVec};
    use sea_orm::{ConnectOptions, Database, DatabaseConnection};
    use std::sync::Arc;
    use tracing::{info, warn};

    // -------------------------------------------------------------------------
    // Prometheus metric descriptors (registered once at startup)
    // -------------------------------------------------------------------------

    /// Active (checked-out) connections per pool.
    pub static POOL_ACTIVE: Lazy<IntGaugeVec> = Lazy::new(|| {
        register_int_gauge_vec!(
            "db_pool_connections_active",
            "Number of connections currently checked out from the pool",
            &["pool"]
        )
        .expect("failed to register db_pool_connections_active metric")
    });

    /// Idle (available) connections per pool.
    pub static POOL_IDLE: Lazy<IntGaugeVec> = Lazy::new(|| {
        register_int_gauge_vec!(
            "db_pool_connections_idle",
            "Number of idle connections waiting in the pool",
            &["pool"]
        )
        .expect("failed to register db_pool_connections_idle metric")
    });

    /// Configured maximum connections per pool.
    pub static POOL_MAX: Lazy<IntGaugeVec> = Lazy::new(|| {
        register_int_gauge_vec!(
            "db_pool_connections_max",
            "Configured maximum number of connections in the pool",
            &["pool"]
        )
        .expect("failed to register db_pool_connections_max metric")
    });

    // -------------------------------------------------------------------------
    // DbPool
    // -------------------------------------------------------------------------

    /// Dual-pool wrapper that routes reads to the replica and writes to the primary.
    ///
    /// When no replica URL is configured the pool degrades gracefully to single-
    /// node mode: both [`DbPool::primary`] and [`DbPool::replica`] return the
    /// same underlying connection.
    #[derive(Clone, Debug)]
    pub struct DbPool {
        /// Read-write primary connection.
        primary: Arc<DatabaseConnection>,
        /// Read-only replica connection (or a clone of the primary in single-node mode).
        replica: Arc<DatabaseConnection>,
        /// `true` when a real replica is connected; `false` in single-node mode.
        has_replica: bool,
    }

    impl DbPool {
        /// Build a [`DbPool`] from environment variables.
        ///
        /// * `DATABASE_URL`         – required; primary DSN.
        /// * `DATABASE_REPLICA_URL` – optional; replica DSN.
        ///
        /// Panics only when `DATABASE_URL` is missing.
        pub async fn from_env() -> Self {
            dotenv::dotenv().ok();
            let primary_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
            let replica_url = std::env::var("DATABASE_REPLICA_URL").ok();

            Self::connect(&primary_url, replica_url.as_deref()).await
        }

        /// Build a [`DbPool`] with explicit DSN strings.
        ///
        /// `replica_url = None` activates single-node (fallback) mode.
        pub async fn connect(primary_url: &str, replica_url: Option<&str>) -> Self {
            let primary = Arc::new(
                Database::connect(Self::options(primary_url))
                    .await
                    .unwrap_or_else(|e| panic!("Failed to connect to primary DB: {e}")),
            );
            info!("Connected to primary database");

            // Initialise primary pool metrics ceiling from the pool config.
            let primary_max = Self::options(primary_url)
                .get_max_connections()
                .unwrap_or(20) as i64;
            POOL_MAX.with_label_values(&["primary"]).set(primary_max);

            let (replica, has_replica) = match replica_url {
                Some(url) => match Database::connect(Self::options(url)).await {
                    Ok(conn) => {
                        let replica_max =
                            Self::options(url).get_max_connections().unwrap_or(20) as i64;
                        POOL_MAX.with_label_values(&["replica"]).set(replica_max);
                        info!("Connected to replica database");
                        (Arc::new(conn), true)
                    }
                    Err(e) => {
                        warn!("Failed to connect to replica DB ({e}); falling back to primary");
                        POOL_MAX.with_label_values(&["replica"]).set(primary_max);
                        (primary.clone(), false)
                    }
                },
                None => {
                    info!("DATABASE_REPLICA_URL not set — operating in single-pool mode");
                    POOL_MAX.with_label_values(&["replica"]).set(primary_max);
                    (primary.clone(), false)
                }
            };

            Self {
                primary,
                replica,
                has_replica,
            }
        }

        // ------------------------------------------------------------------
        // Pool accessors
        // ------------------------------------------------------------------

        /// Return a reference to the **primary** (read-write) connection.
        ///
        /// Use this for any query that mutates data: `INSERT`, `UPDATE`,
        /// `DELETE`, and all transactional operations.
        #[inline]
        pub fn primary(&self) -> &DatabaseConnection {
            &self.primary
        }

        /// Return a reference to the **replica** (read-only) connection.
        ///
        /// When no replica is configured this transparently returns the primary
        /// connection so callers never need to special-case the absence of a
        /// replica.
        #[inline]
        pub fn replica(&self) -> &DatabaseConnection {
            &self.replica
        }

        /// `true` when a dedicated read replica is available.
        #[inline]
        pub fn has_replica(&self) -> bool {
            self.has_replica
        }

        // ------------------------------------------------------------------
        // Metrics
        // ------------------------------------------------------------------

        /// Snapshot pool statistics into the registered Prometheus gauges.
        ///
        /// Call this from a periodic background task or a `/metrics` handler.
        pub fn update_metrics(&self) {
            Self::record_pool_metrics("primary", &self.primary);
            if self.has_replica {
                Self::record_pool_metrics("replica", &self.replica);
            } else {
                // In single-pool mode the replica label mirrors the primary.
                Self::record_pool_metrics("replica", &self.primary);
            }
        }

        fn record_pool_metrics(label: &str, conn: &DatabaseConnection) {
            // get_postgres_connection_pool() panics on anything that isn't a live
            // Postgres pool, so skip connections that can't report pool stats
            // (mocks in tests, and any future non-Postgres backend). Recording
            // metrics should never be able to bring the process down.
            if !matches!(conn, DatabaseConnection::SqlxPostgresPoolConnection(_)) {
                return;
            }

            // sea-orm exposes the underlying sqlx pool through get_postgres_connection_pool().
            // The sqlx pool tracks active/idle/max connections.
            let pool = conn.get_postgres_connection_pool();
            let size = pool.size() as i64;
            let idle = pool.num_idle() as i64;
            let max = pool.options().get_max_connections() as i64;
            let active = size - idle;

            POOL_ACTIVE.with_label_values(&[label]).set(active.max(0));
            POOL_IDLE.with_label_values(&[label]).set(idle);
            POOL_MAX.with_label_values(&[label]).set(max);
        }

        // ------------------------------------------------------------------
        // Test helpers
        // ------------------------------------------------------------------

        /// Construct a `DbPool` directly from two pre-built connections.
        ///
        /// Intended for unit tests that use `MockDatabase` connections.
        /// Pass `has_replica = false` to simulate single-pool mode.
        pub fn from_connections(
            primary: Arc<DatabaseConnection>,
            replica: Arc<DatabaseConnection>,
            has_replica: bool,
        ) -> Self {
            Self {
                primary,
                replica,
                has_replica,
            }
        }

        /// Consume the pool and return the underlying `(primary, replica)` arcs.
        ///
        /// Intended for unit tests that need to inspect the transaction log on
        /// each mock connection after calling service methods.
        pub fn into_connections(self) -> (Arc<DatabaseConnection>, Arc<DatabaseConnection>) {
            (self.primary, self.replica)
        }

        // ------------------------------------------------------------------
        // Internals
        // ------------------------------------------------------------------

        fn options(url: &str) -> ConnectOptions {
            let mut opts = ConnectOptions::new(url.to_owned());
            // Sensible pool defaults — these can be overridden via env in a
            // future configuration struct.
            opts.max_connections(20)
                .min_connections(2)
                .sqlx_logging(false);
            opts
        }
    }

    // -------------------------------------------------------------------------
    // Legacy convenience helper (kept for backward compatibility)
    // -------------------------------------------------------------------------

    /// Connect to the primary database using `DATABASE_URL`.
    ///
    /// Prefer [`DbPool::from_env`] for new code.
    pub async fn get_db() -> DatabaseConnection {
        dotenv::dotenv().ok();
        let connect_options = ConnectOptions::new(
            std::env::var("DATABASE_URL").expect("DATABASE_URL is not defined"),
        )
        .to_owned();

        Database::connect(connect_options)
            .await
            .expect("Failed to connect to database")
    }
}
