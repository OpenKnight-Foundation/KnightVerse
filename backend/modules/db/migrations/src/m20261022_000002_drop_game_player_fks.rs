use sea_orm_migration::prelude::*;

/// Drops the `game` → `player` foreign keys.
///
/// The game service uses the nil UUID as "seat not taken yet" for
/// `black_player` when a game is created, and for both players on imported
/// games (see `service::games`). Those rows can never satisfy a foreign key to
/// `player`, so creating a game failed with
/// `violates foreign key constraint "fk_game_black_player"`.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        for constraint in ["fk_game_white_player", "fk_game_black_player"] {
            db.execute_unprepared(&format!(
                "ALTER TABLE \"smdb\".\"game\" DROP CONSTRAINT IF EXISTS \"{constraint}\""
            ))
            .await?;
        }
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Re-adding the constraints would fail on any game with an open seat.
        Ok(())
    }
}
