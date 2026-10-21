use sea_orm_migration::prelude::*;

/// Brings the Postgres enum types in line with the values `db_entity::game`
/// actually writes. The original games migration created `result_side` as
/// white/black/draw/none and `game_variant` with "three-check", so inserting a
/// game with e.g. `white_wins`, `ongoing` or `three_check` failed.
#[derive(DeriveMigrationName)]
pub struct Migration;

const RESULT_SIDE_VALUES: [&str; 4] = ["ongoing", "white_wins", "black_wins", "abandoned"];
const GAME_VARIANT_VALUES: [&str; 4] = ["three_check", "blitz", "rapid", "classical"];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        for value in RESULT_SIDE_VALUES {
            db.execute_unprepared(&format!(
                "ALTER TYPE \"result_side\" ADD VALUE IF NOT EXISTS '{value}'"
            ))
            .await?;
        }
        for value in GAME_VARIANT_VALUES {
            db.execute_unprepared(&format!(
                "ALTER TYPE \"game_variant\" ADD VALUE IF NOT EXISTS '{value}'"
            ))
            .await?;
        }
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Postgres can't drop a value from an enum type; the added values are
        // harmless to keep.
        Ok(())
    }
}
