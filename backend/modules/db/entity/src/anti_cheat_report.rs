use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "anti_cheat_reports", schema_name = "smdb")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub game_id: Uuid,
    pub player_id: Uuid,
    pub flagged: bool,
    #[sea_orm(column_type = "Text")]
    pub flag_reason: Option<String>,
    #[sea_orm(column_type = "JsonBinary")]
    pub metrics: Json,
    #[sea_orm(column_type = "JsonBinary")]
    pub report: Json,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
