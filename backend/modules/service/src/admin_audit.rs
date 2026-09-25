//! Admin audit log for moderator actions (BE-91).
//!
//! Moderation-adjacent capabilities (anti-cheat flagging, player reports)
//! existed without any durable record of *who* took *what* action *when*. This
//! module persists every moderator action to the `admin_actions` table and
//! exposes a paginated, filterable listing for administrators.
//!
//! ## Read/Write pool routing
//!
//! | Method                   | Pool    | Reason             |
//! |--------------------------|---------|--------------------|
//! | `record_admin_action`    | primary | INSERT             |
//! | `perform_admin_action`   | primary | UPDATE + INSERT    |
//! | `list_admin_actions`     | replica | paginated SELECT   |

use chrono::Utc;
use db::DbPool;
use db_entity::admin_action;
use error::error::ApiError;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, Set,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

/// Rows returned per page when the caller does not specify `per_page`.
pub const DEFAULT_PER_PAGE: u64 = 20;

/// Upper bound on `per_page`, so a single request cannot scan the whole table.
pub const MAX_PER_PAGE: u64 = 100;

/// Kinds of moderator/admin action recorded in the audit log.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdminActionType {
    Ban,
    Unban,
    Mute,
    Unmute,
    ResolveDispute,
    FlagCheating,
}

impl AdminActionType {
    /// Stable string stored in the `action_type` column.
    pub fn as_str(&self) -> &'static str {
        match self {
            AdminActionType::Ban => "ban",
            AdminActionType::Unban => "unban",
            AdminActionType::Mute => "mute",
            AdminActionType::Unmute => "unmute",
            AdminActionType::ResolveDispute => "resolve_dispute",
            AdminActionType::FlagCheating => "flag_cheating",
        }
    }

    /// Parse the wire representation accepted by the admin endpoints.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "ban" => Some(AdminActionType::Ban),
            "unban" => Some(AdminActionType::Unban),
            "mute" => Some(AdminActionType::Mute),
            "unmute" => Some(AdminActionType::Unmute),
            "resolve_dispute" => Some(AdminActionType::ResolveDispute),
            "flag_cheating" => Some(AdminActionType::FlagCheating),
            _ => None,
        }
    }
}

/// A pending audit-log entry, before persistence assigns an id and timestamp.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewAdminAction {
    pub actor_id: Uuid,
    pub action_type: AdminActionType,
    pub target_id: Option<Uuid>,
    pub reason: Option<String>,
    pub metadata: Value,
}

impl NewAdminAction {
    pub fn new(actor_id: Uuid, action_type: AdminActionType) -> Self {
        Self {
            actor_id,
            action_type,
            target_id: None,
            reason: None,
            metadata: json!({}),
        }
    }

    pub fn target(mut self, target_id: Uuid) -> Self {
        self.target_id = Some(target_id);
        self
    }

    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }

    pub fn metadata(mut self, metadata: Value) -> Self {
        self.metadata = metadata;
        self
    }
}

/// Optional filters accepted by the listing endpoint.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AdminActionFilter {
    /// Only actions performed by this actor.
    pub actor_id: Option<Uuid>,
    /// Only actions taken against this target.
    pub target_id: Option<Uuid>,
    /// 1-based page number (defaults to 1).
    pub page: Option<u64>,
    /// Rows per page, clamped to `1..=MAX_PER_PAGE` (defaults to `DEFAULT_PER_PAGE`).
    pub per_page: Option<u64>,
}

/// One page of audit-log entries plus pagination metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminActionPage {
    pub items: Vec<admin_action::Model>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
    pub total_pages: u64,
}

/// Normalise a requested `(page, per_page)` pair into safe bounds.
pub fn resolve_paging(filter: &AdminActionFilter) -> (u64, u64) {
    let per_page = filter
        .per_page
        .unwrap_or(DEFAULT_PER_PAGE)
        .clamp(1, MAX_PER_PAGE);
    let page = filter.page.unwrap_or(1).max(1);
    (page, per_page)
}

/// Record exactly one moderator action in the audit log.
///
/// Routes to the **primary** pool (INSERT).
pub async fn record_admin_action(
    pool: &DbPool,
    entry: NewAdminAction,
) -> Result<admin_action::Model, ApiError> {
    record_admin_action_on(pool.primary(), entry).await
}

pub(crate) async fn record_admin_action_on(
    db: &DatabaseConnection,
    entry: NewAdminAction,
) -> Result<admin_action::Model, ApiError> {
    let model = admin_action::ActiveModel {
        id: Set(Uuid::new_v4()),
        actor_id: Set(entry.actor_id),
        action_type: Set(entry.action_type.as_str().to_string()),
        target_id: Set(entry.target_id),
        reason: Set(entry.reason),
        metadata: Set(entry.metadata),
        created_at: Set(Utc::now().into()),
    };

    model.insert(db).await.map_err(ApiError::DatabaseError)
}

/// Perform a moderator action and record exactly one audit row for it.
///
/// A `ban` additionally reuses the existing player soft-delete path so the
/// account state change (`is_enabled = false`) and its audit entry stay in
/// lock-step. Mute and dispute-resolution actions have no persisted state in
/// the backend today, so the audit log is their durable record; any details
/// (mute duration, dispute outcome, ...) go in `metadata`.
///
/// Routes to the **primary** pool.
pub async fn perform_admin_action(
    pool: &DbPool,
    actor_id: Uuid,
    action: AdminActionType,
    target_id: Uuid,
    reason: Option<String>,
    metadata: Value,
) -> Result<admin_action::Model, ApiError> {
    if action == AdminActionType::Ban {
        crate::players::delete_player(pool, target_id).await?;
    }

    record_admin_action(
        pool,
        NewAdminAction {
            actor_id,
            action_type: action,
            target_id: Some(target_id),
            reason,
            metadata,
        },
    )
    .await
}

/// List recent admin actions, newest first, with pagination and optional
/// actor/target filters.
///
/// Routes to the **replica** pool (COUNT + SELECT).
pub async fn list_admin_actions(
    pool: &DbPool,
    filter: AdminActionFilter,
) -> Result<AdminActionPage, ApiError> {
    list_admin_actions_on(pool.replica(), filter).await
}

pub(crate) async fn list_admin_actions_on(
    db: &DatabaseConnection,
    filter: AdminActionFilter,
) -> Result<AdminActionPage, ApiError> {
    let (page, per_page) = resolve_paging(&filter);

    let mut query = admin_action::Entity::find();
    if let Some(actor_id) = filter.actor_id {
        query = query.filter(admin_action::Column::ActorId.eq(actor_id));
    }
    if let Some(target_id) = filter.target_id {
        query = query.filter(admin_action::Column::TargetId.eq(target_id));
    }
    query = query.order_by_desc(admin_action::Column::CreatedAt);

    let paginator = query.paginate(db, per_page);
    let total = paginator.num_items().await.map_err(ApiError::DatabaseError)?;
    let items = paginator
        .fetch_page(page.saturating_sub(1))
        .await
        .map_err(ApiError::DatabaseError)?;

    let total_pages = if total == 0 {
        0
    } else {
        (total + per_page - 1) / per_page
    };

    Ok(AdminActionPage {
        items,
        total,
        page,
        per_page,
        total_pages,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use db_entity::player;
    use sea_orm::{DbBackend, MockDatabase};
    use std::sync::Arc;

    fn make_audit_model(action_type: &str) -> admin_action::Model {
        admin_action::Model {
            id: Uuid::new_v4(),
            actor_id: Uuid::new_v4(),
            action_type: action_type.to_string(),
            target_id: Some(Uuid::new_v4()),
            reason: Some("unit test".to_string()),
            metadata: json!({ "source": "test" }),
            created_at: Utc::now().into(),
        }
    }

    fn make_player() -> player::Model {
        player::Model {
            id: Uuid::new_v4(),
            username: "target".to_string(),
            email: "target@example.com".to_string(),
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

    fn audit_insert_count<T: std::fmt::Debug>(log: &[T]) -> usize {
        log.iter()
            .filter(|entry| format!("{:?}", entry).contains("admin_actions"))
            .count()
    }

    #[test]
    fn action_type_round_trips() {
        for kind in [
            AdminActionType::Ban,
            AdminActionType::Unban,
            AdminActionType::Mute,
            AdminActionType::Unmute,
            AdminActionType::ResolveDispute,
            AdminActionType::FlagCheating,
        ] {
            assert_eq!(AdminActionType::parse(kind.as_str()), Some(kind));
        }
        assert_eq!(AdminActionType::parse("nonsense"), None);
    }

    #[test]
    fn new_admin_action_defaults_are_empty() {
        let entry = NewAdminAction::new(Uuid::new_v4(), AdminActionType::Mute);
        assert!(entry.target_id.is_none());
        assert!(entry.reason.is_none());
        assert_eq!(entry.metadata, json!({}));
    }

    #[test]
    fn paging_is_clamped_to_safe_bounds() {
        let defaults = resolve_paging(&AdminActionFilter::default());
        assert_eq!(defaults, (1, DEFAULT_PER_PAGE));

        let clamped_high = resolve_paging(&AdminActionFilter {
            page: Some(0),
            per_page: Some(10_000),
            ..Default::default()
        });
        assert_eq!(clamped_high, (1, MAX_PER_PAGE));

        let clamped_low = resolve_paging(&AdminActionFilter {
            page: Some(3),
            per_page: Some(0),
            ..Default::default()
        });
        assert_eq!(clamped_low, (3, 1));
    }

    #[tokio::test]
    async fn record_admin_action_writes_exactly_one_row() {
        let saved = make_audit_model("mute");
        let primary = MockDatabase::new(DbBackend::Postgres)
            .append_query_results(vec![vec![saved.clone()]])
            .into_connection();
        let replica = MockDatabase::new(DbBackend::Postgres).into_connection();
        let pool = DbPool::from_connections(Arc::new(primary), Arc::new(replica), true);

        let entry = NewAdminAction::new(Uuid::new_v4(), AdminActionType::Mute)
            .target(Uuid::new_v4())
            .reason("chat spam")
            .metadata(json!({ "duration_seconds": 3600 }));

        let stored = record_admin_action(&pool, entry)
            .await
            .expect("audit insert should succeed");
        assert_eq!(stored.action_type, "mute");

        let (primary_conn, _replica_conn) = pool.into_connections();
        let log = Arc::try_unwrap(primary_conn)
            .expect("pool holds the only primary reference")
            .into_transaction_log();
        assert_eq!(audit_insert_count(&log), 1, "one action => one admin_actions row");
        assert!(
            format!("{:?}", log[0]).to_uppercase().contains("INSERT"),
            "the audit row must be written with an INSERT"
        );
    }

    #[tokio::test]
    async fn ban_writes_exactly_one_row_after_disabling_the_player() {
        let existing = make_player();
        let target = existing.id;
        let mut disabled = existing.clone();
        disabled.is_enabled = false;
        let saved = make_audit_model("ban");

        let replica = MockDatabase::new(DbBackend::Postgres)
            .append_query_results(vec![vec![existing]])
            .into_connection();
        let primary = MockDatabase::new(DbBackend::Postgres)
            .append_query_results(vec![vec![disabled]])
            .append_query_results(vec![vec![saved]])
            .into_connection();
        let pool = DbPool::from_connections(Arc::new(primary), Arc::new(replica), true);

        perform_admin_action(
            &pool,
            Uuid::new_v4(),
            AdminActionType::Ban,
            target,
            Some("engine assistance".to_string()),
            json!({}),
        )
        .await
        .expect("ban should succeed");

        let (primary_conn, _replica_conn) = pool.into_connections();
        let log = Arc::try_unwrap(primary_conn)
            .expect("pool holds the only primary reference")
            .into_transaction_log();
        assert_eq!(
            audit_insert_count(&log),
            1,
            "a ban must write exactly one admin_actions row"
        );
        assert!(
            log.iter()
                .any(|entry| format!("{:?}", entry).to_uppercase().contains("UPDATE")),
            "the ban must also disable the player account"
        );
    }

    #[tokio::test]
    async fn mute_and_dispute_each_write_exactly_one_row() {
        for (action, expected) in [
            (AdminActionType::Mute, "mute"),
            (AdminActionType::ResolveDispute, "resolve_dispute"),
        ] {
            let saved = make_audit_model(expected);
            let primary = MockDatabase::new(DbBackend::Postgres)
                .append_query_results(vec![vec![saved]])
                .into_connection();
            let replica = MockDatabase::new(DbBackend::Postgres).into_connection();
            let pool = DbPool::from_connections(Arc::new(primary), Arc::new(replica), true);

            let stored = perform_admin_action(
                &pool,
                Uuid::new_v4(),
                action,
                Uuid::new_v4(),
                Some("policy violation".to_string()),
                json!({ "outcome": "upheld" }),
            )
            .await
            .expect("action should succeed");
            assert_eq!(stored.action_type, expected);

            let (primary_conn, _replica_conn) = pool.into_connections();
            let log = Arc::try_unwrap(primary_conn)
                .expect("pool holds the only primary reference")
                .into_transaction_log();
            assert_eq!(
                audit_insert_count(&log),
                1,
                "{expected} must write exactly one admin_actions row"
            );
            assert!(
                log.iter()
                    .all(|entry| format!("{:?}", entry).contains("admin_actions")),
                "{expected} should only write the audit row"
            );
        }
    }
}
