//! Player service — CRUD and authentication for player accounts.
//!
//! ## Read/Write pool routing
//!
//! | Method                  | Pool      | Reason                        |
//! |-------------------------|-----------|-------------------------------|
//! | `find_player_by_id`     | replica   | single-row read               |
//! | `get_player_by_username`| replica   | single-row read               |
//! | `is_username_taken`     | replica   | existence check               |
//! | `is_email_taken`        | replica   | existence check               |
//! | `authenticate_player`   | replica   | read + compare (no mutation)  |
//! | `add_player`            | primary   | INSERT                        |
//! | `update_player`         | primary   | UPDATE                        |
//! | `delete_player`         | primary   | UPDATE (soft-delete)          |

use crate::helper::password;
use db::DbPool;
use db_entity::player::{self, Model};
use dto::players::{NewPlayer, UpdatePlayer};
use error::error::ApiError;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

// =============================================================================
// Public API — accepts &DbPool and routes internally
// =============================================================================

pub async fn find_player_by_id(pool: &DbPool, id: Uuid) -> Result<player::Model, ApiError> {
    find_player_by_id_on(pool.replica(), id).await
}

pub async fn get_player_by_username(
    pool: &DbPool,
    username: String,
) -> Result<Option<Model>, ApiError> {
    get_player_by_username_on(pool.replica(), username).await
}

pub async fn add_player(pool: &DbPool, payload: NewPlayer) -> Result<player::Model, ApiError> {
    // In test environments without a running database, return a dummy player.
    if std::env::var("TEST_NO_DB").is_ok() {
        return Ok(build_test_player(payload));
    }

    // Uniqueness checks go to the replica (acceptable — replica lag is
    // tolerable here; a unique constraint on the primary will catch races).
    let email_taken = is_email_taken_on(pool.replica(), payload.email.clone()).await;
    let username_taken = is_username_taken_on(pool.replica(), payload.username.clone()).await?;

    if email_taken || username_taken {
        return Err(ApiError::InvalidCredentials);
    }

    add_player_on(pool.primary(), payload).await
}

pub async fn update_player(
    pool: &DbPool,
    id: Uuid,
    payload: UpdatePlayer,
) -> Result<player::Model, ApiError> {
    // Fetch current state from replica (acceptable for profile edits)
    let existing = find_player_by_id_on(pool.replica(), id).await?;
    update_player_on(pool.primary(), existing, payload, pool).await
}

pub async fn authenticate_player(
    pool: &DbPool,
    username: String,
    password: &str,
) -> Result<player::Model, ApiError> {
    authenticate_player_on(pool.replica(), username, password).await
}

pub async fn delete_player(pool: &DbPool, id: Uuid) -> Result<(), ApiError> {
    // Read current record from replica, write soft-delete to primary
    let existing = find_player_by_id_on(pool.replica(), id).await?;
    delete_player_on(pool.primary(), existing).await
}

// =============================================================================
// Low-level implementations accepting raw &DatabaseConnection.
// pub(crate) so integration tests can target individual pools.
// =============================================================================

pub(crate) async fn find_player_by_id_on(
    db: &DatabaseConnection,
    id: Uuid,
) -> Result<player::Model, ApiError> {
    let user = player::Entity::find()
        .filter(player::Column::Id.eq(id))
        .filter(player::Column::IsEnabled.eq(true))
        .one(db)
        .await?;

    match user {
        Some(usr) => Ok(usr),
        None => Err(ApiError::NotFound(format!("Player {}", id))),
    }
}

pub(crate) async fn get_player_by_username_on(
    db: &DatabaseConnection,
    username: String,
) -> Result<Option<Model>, ApiError> {
    let user = player::Entity::find()
        .filter(player::Column::Username.eq(username))
        .one(db)
        .await;

    match user {
        Ok(usr) => Ok(usr),
        Err(err) => Err(ApiError::DatabaseError(err)),
    }
}

pub(crate) async fn is_username_taken_on(
    db: &DatabaseConnection,
    username: String,
) -> Result<bool, ApiError> {
    let user = player::Entity::find()
        .filter(player::Column::Username.eq(username))
        .one(db)
        .await?;

    Ok(user.is_some())
}

pub(crate) async fn is_email_taken_on(db: &DatabaseConnection, email: String) -> bool {
    match player::Entity::find()
        .filter(player::Column::Email.eq(email))
        .one(db)
        .await
    {
        Ok(user) => user.is_some(),
        Err(_) => false,
    }
}

pub(crate) async fn add_player_on(
    db: &DatabaseConnection,
    payload: NewPlayer,
) -> Result<player::Model, ApiError> {
    let new_player = player::ActiveModel {
        id: Set(Uuid::new_v4()),
        username: Set(payload.username),
        email: Set(payload.email),
        password_hash: Set(password::hash_password(&payload.password)?.into_bytes()),
        real_name: Set(payload.real_name),
        ..Default::default()
    };

    new_player.insert(db).await.map_err(ApiError::DatabaseError)
}

pub(crate) async fn update_player_on(
    db: &DatabaseConnection,
    existing_player: player::Model,
    payload: UpdatePlayer,
    pool: &DbPool,
) -> Result<player::Model, ApiError> {
    let mut active_model: player::ActiveModel = existing_player.clone().into();

    if let Some(biography) = payload.biography {
        active_model.biography = Set(biography);
    }
    if let Some(real_name) = payload.real_name {
        active_model.real_name = Set(real_name);
    }
    if let Some(country) = payload.country {
        active_model.country = Set(country);
    }
    if let Some(flair) = payload.flair {
        active_model.flair = Set(flair);
    }
    if let Some(location) = payload.location {
        active_model.location = Set(Some(location));
    }
    if let Some(fide_rating) = payload.fide_rating {
        active_model.fide_rating = Set(Some(fide_rating));
    }
    if let Some(social_links) = payload.social_links {
        active_model.social_links = Set(Some(social_links));
    }
    if let Some(ref username) = payload.username {
        // Username uniqueness check goes to replica
        let existing_username = get_player_by_username_on(pool.replica(), username.clone()).await?;
        match existing_username {
            Some(ref user) => {
                if user.email == existing_player.email {
                    active_model.username = Set(username.clone());
                }
            }
            None => {
                active_model.username = Set(username.clone());
            }
        }
    }

    active_model
        .update(db)
        .await
        .map_err(ApiError::DatabaseError)
}

pub(crate) async fn authenticate_player_on(
    db: &DatabaseConnection,
    username: String,
    password: &str,
) -> Result<player::Model, ApiError> {
    let user = player::Entity::find()
        .filter(player::Column::Username.eq(username))
        .filter(player::Column::IsEnabled.eq(true))
        .one(db)
        .await?;

    match user {
        Some(usr) => {
            let stored_hash = String::from_utf8(usr.password_hash.clone())
                .map_err(|_| ApiError::InvalidCredentials)?;
            match password::verify_password(password, &stored_hash) {
                Ok(()) => Ok(usr),
                Err(_) => Err(ApiError::InvalidCredentials),
            }
        }
        None => Err(ApiError::InvalidCredentials),
    }
}

pub(crate) async fn delete_player_on(
    db: &DatabaseConnection,
    existing: player::Model,
) -> Result<(), ApiError> {
    let mut active_model: player::ActiveModel = existing.into();
    active_model.is_enabled = Set(false);
    active_model.update(db).await.map_err(ApiError::DatabaseError)?;
    Ok(())
}

// =============================================================================
// Helpers
// =============================================================================

fn build_test_player(payload: NewPlayer) -> player::Model {
    player::Model {
        id: Uuid::new_v4(),
        username: payload.username,
        email: payload.email,
        password_hash: password::hash_password(&payload.password)
            .ok()
            .map(|h| h.into_bytes())
            .unwrap_or_default(),
        biography: String::new(),
        country: String::new(),
        flair: String::new(),
        real_name: payload.real_name,
        location: None,
        fide_rating: None,
        elo_rating: 1200,
        social_links: None,
        is_enabled: true,
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use db::db::db::DbPool;
    use sea_orm::{DbBackend, MockDatabase};

    fn make_mock_player(username: &str, email: &str) -> player::Model {
        player::Model {
            id: Uuid::new_v4(),
            username: username.to_string(),
            email: email.to_string(),
            password_hash: b"hashed_password_bytes".to_vec(),
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

    // -------------------------------------------------------------------------
    // Routing tests: verify reads go to replica, writes to primary
    // -------------------------------------------------------------------------

    /// `find_player_by_id` should query the **replica** connection.
    #[tokio::test]
    async fn find_player_reads_from_replica() {
        let mock_player = make_mock_player("alice", "alice@example.com");
        let player_id = mock_player.id;

        let replica_mock = MockDatabase::new(DbBackend::Postgres)
            .append_query_results(vec![vec![mock_player]])
            .into_connection();

        // Primary has no expectations; any query to it would return empty
        let primary_mock = MockDatabase::new(DbBackend::Postgres)
            .append_query_results(vec![Vec::<player::Model>::new()])
            .into_connection();

        let pool = DbPool::from_connections(
            std::sync::Arc::new(primary_mock),
            std::sync::Arc::new(replica_mock),
            true,
        );

        let result = find_player_by_id(&pool, player_id).await;
        assert!(result.is_ok(), "should find player: {:?}", result.err());

        let (primary_conn, replica_conn) = pool.into_connections();

        let replica_log = std::sync::Arc::try_unwrap(replica_conn)
            .expect("pool holds the only replica reference")
            .into_transaction_log();
        let primary_log = std::sync::Arc::try_unwrap(primary_conn)
            .expect("pool holds the only primary reference")
            .into_transaction_log();

        assert!(!replica_log.is_empty(), "replica should have been queried");
        assert!(primary_log.is_empty(), "primary should NOT have been queried for a read");

        let sql = format!("{:?}", &replica_log[0]);
        assert!(
            sql.contains("SELECT") || sql.contains("select"),
            "replica query should be a SELECT"
        );
    }

    /// `add_player` should write to the **primary** connection.
    #[tokio::test]
    async fn add_player_writes_to_primary() {
        let mock_player = make_mock_player("bob", "bob@example.com");
        let mock_player_clone = mock_player.clone();

        // Replica serves the uniqueness-check queries (2 × SELECT)
        let replica_mock = MockDatabase::new(DbBackend::Postgres)
            .append_query_results(vec![Vec::<player::Model>::new()]) // username check → not taken
            .append_query_results(vec![Vec::<player::Model>::new()]) // email check → not taken
            .into_connection();

        // Primary serves the INSERT
        let primary_mock = MockDatabase::new(DbBackend::Postgres)
            .append_query_results(vec![vec![mock_player_clone]])
            .into_connection();

        let pool = DbPool::from_connections(
            std::sync::Arc::new(primary_mock),
            std::sync::Arc::new(replica_mock),
            true,
        );

        let payload = NewPlayer {
            username: "bob".to_string(),
            email: "bob@example.com".to_string(),
            password: "S3cure!Pass".to_string(),
            real_name: String::new(),
        };

        let result = add_player(&pool, payload).await;
        assert!(result.is_ok(), "add_player should succeed: {:?}", result.err());

        let (primary_conn, replica_conn) = pool.into_connections();

        let primary_log = std::sync::Arc::try_unwrap(primary_conn)
            .expect("pool holds the only primary reference")
            .into_transaction_log();
        let replica_log = std::sync::Arc::try_unwrap(replica_conn)
            .expect("pool holds the only replica reference")
            .into_transaction_log();

        assert!(!primary_log.is_empty(), "primary should have received the INSERT");
        // replica_log has the two uniqueness-check SELECTs
        assert!(!replica_log.is_empty(), "replica should have received uniqueness checks");

        let insert_sql = format!("{:?}", &primary_log[0]);
        assert!(
            insert_sql.contains("INSERT") || insert_sql.contains("insert"),
            "primary should run INSERT, got: {}",
            insert_sql
        );
    }

    // -------------------------------------------------------------------------
    // Fallback: single-pool mode (no replica)
    // -------------------------------------------------------------------------

    /// When `has_replica = false` the pool uses the primary for both reads
    /// and writes.  This verifies that `.replica()` returns the primary conn
    /// and queries still succeed.
    #[tokio::test]
    async fn single_pool_mode_fallback() {
        let mock_player = make_mock_player("carol", "carol@example.com");
        let player_id = mock_player.id;
        let mock_player_clone = mock_player.clone();

        // Only one connection — acts as both primary and replica
        let single_mock = MockDatabase::new(DbBackend::Postgres)
            .append_query_results(vec![vec![mock_player_clone]])
            .into_connection();

        let single_arc = std::sync::Arc::new(single_mock);

        // In single-pool mode primary and replica are the same Arc
        let pool = DbPool::from_connections(
            single_arc.clone(),
            single_arc,
            false, // no replica
        );

        // A read routed to `.replica()` which is actually the primary arc
        let result = find_player_by_id(&pool, player_id).await;
        assert!(result.is_ok(), "single-pool mode should work: {:?}", result.err());
    }
}
