//! `SeaORM` Entity for the admin audit log (BE-91).

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "admin_actions", schema_name = "smdb")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    /// Who performed the action (player UUID of the moderator/admin).
    pub actor_id: Uuid,
    /// Machine-readable action kind, e.g. `ban`, `mute`, `resolve_dispute`.
    pub action_type: String,
    /// The subject of the action (player, game or dispute id), when applicable.
    pub target_id: Option<Uuid>,
    /// Human-readable justification supplied by the moderator.
    #[sea_orm(column_type = "Text")]
    pub reason: Option<String>,
    /// Free-form structured context (durations, outcomes, related ids, ...).
    #[sea_orm(column_type = "JsonBinary")]
    pub metadata: Json,
    /// When the action was recorded.
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
