use sea_orm_migration::{prelude::*, MigrationTrait};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_game_white_player")
                    .table(Game::Table)
                    .col(Game::WhitePlayer)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_game_black_player")
                    .table(Game::Table)
                    .col(Game::BlackPlayer)
                    .to_owned(),
            )
            .await?;
        // Create GIN index using raw SQL as IndexType::Gin is not available/standard in all sea-orm versions or requires specific features
        manager
            .get_connection()
            .execute_unprepared(
                r#"CREATE INDEX IF NOT EXISTS "idx_game_pgn" ON "smdb"."game" USING GIN ("pgn")"#,
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_player_username")
                    .table(Player::Table)
                    .col(Player::Username)
                    .unique()
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_game_started_at")
                    .table(Game::Table)
                    .col(Game::StartedAt)
                    .to_owned(),
            )
            .await?;
        println!("Common indexes created successfully.");
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_game_white_player")
                    .table(Game::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("idx_game_black_player")
                    .table(Game::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("idx_game_pgn")
                    .table(Game::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("idx_player_username")
                    .table(Player::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("idx_game_started_at")
                    .table(Game::Table)
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}

#[derive(Iden)]
#[iden = "smdb"] // Specify the schema here
enum Game {
    #[iden = "game"] // Specify the table name here
    Table,
    Id,
    WhitePlayer,
    BlackPlayer,
    StartedAt,
    Pgn,
    // Add other columns if needed for future migrations involving this table
}

#[derive(Iden)]
#[iden = "smdb"]
enum Player {
    #[iden = "player"]
    Table,
    Username,
}
