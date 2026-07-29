use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add is_imported column to the game table
        manager
            .alter_table(
                Table::alter()
                    .table((Smdb, Game::Table))
                    .add_column(
                        ColumnDef::new(Game::IsImported)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .add_column(ColumnDef::new(Game::OriginalPgn).text().null())
                    .to_owned(),
            )
            .await?;

        // Create index on is_imported for faster queries on imported games
        manager
            .create_index(
                Index::create()
                    .name("idx_games_is_imported")
                    .table((Smdb, Game::Table))
                    .col(Game::IsImported)
                    .to_owned(),
            )
            .await?;

        println!("Added is_imported and original_pgn columns to game table.");
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop the index
        manager
            .drop_index(
                Index::drop()
                    .name("idx_games_is_imported")
                    .table((Smdb, Game::Table))
                    .to_owned(),
            )
            .await?;

        // Remove the columns
        manager
            .alter_table(
                Table::alter()
                    .table((Smdb, Game::Table))
                    .drop_column(Game::IsImported)
                    .drop_column(Game::OriginalPgn)
                    .to_owned(),
            )
            .await?;

        println!("Removed is_imported and original_pgn columns from game table.");
        Ok(())
    }
}

// Reference to the Game table columns we're adding
#[derive(DeriveIden)]
enum Game {
    Table,
    IsImported,
    OriginalPgn,
}

// Define the schema identifier
#[derive(DeriveIden)]
struct Smdb;
