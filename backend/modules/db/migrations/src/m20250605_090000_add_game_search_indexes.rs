use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 1. Make `result` nullable to support active games
        manager
            .alter_table(
                Table::alter()
                    .table((Smdb, Game::Table))
                    .modify_column(ColumnDef::new(Game::Result).string().null())
                    .to_owned(),
            )
            .await?;

        // 2. Drop the CHECK constraint that enforces 'white', 'black', 'draw' if it prevents NULLs
        // Actually, CHECK (result IN (...)) usually allows NULL unless checked for NOT NULL.
        // But we should verify. If the check is `result IN (...)` and result is NULL, it evaluates to NULL which is allowed in Check constraints (True or Null passes).
        // However, if we want to be safe or if we want to allow new statuses, we could drop it.
        // For now, I will keep it but assume it allows NULL.
        // Wait, the previous migration added `ALTER TABLE "game" ADD CONSTRAINT "check_game_result" CHECK ("result" IN ('white', 'black', 'draw'))`.
        // If result is NULL, `NULL IN (...)` is NULL, which passes. So we don't need to drop the constraint for NULL support.

        // 3. Create composite indexes with DESC ordering (raw SQL: the index
        //    builder can't express per-column DESC).
        manager
            .get_connection()
            .execute_unprepared(
                r#"CREATE INDEX "idx_games_white_player_created_at_id" ON "smdb"."game" ("white_player", "created_at" DESC, "id" DESC)"#
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                r#"CREATE INDEX "idx_games_black_player_created_at_id" ON "smdb"."game" ("black_player", "created_at" DESC, "id" DESC)"#
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 1. Drop indexes
        manager
            .get_connection()
            .execute_unprepared(r#"DROP INDEX IF EXISTS "smdb"."idx_games_white_player_created_at_id""#)
            .await?;

        manager
            .get_connection()
            .execute_unprepared(r#"DROP INDEX IF EXISTS "smdb"."idx_games_black_player_created_at_id""#)
            .await?;

        // 2. Revert `result` to NOT NULL (this might fail if there are active games with NULL result, but for `down` it is expected to try)
        // We first need to ensure no NULLs exist or we accept it might fail. Use basic alter.
        manager
            .alter_table(
                Table::alter()
                    .table((Smdb, Game::Table))
                    .modify_column(ColumnDef::new(Game::Result).string().not_null())
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Game {
    Table,
    Result,
    WhitePlayer,
    BlackPlayer,
    CreatedAt,
    Id,
}

/// Postgres schema that holds the `game` table (see db_entity::game).
#[derive(DeriveIden)]
struct Smdb;
