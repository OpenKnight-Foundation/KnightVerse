use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add elo_rating column to the player table
        manager
            .alter_table(
                Table::alter()
                    .table(Player::Table)
                    .add_column(
                        ColumnDef::new(Player::EloRating)
                            .integer()
                            .not_null()
                            .default(1200), // Standard starting Elo rating
                    )
                    .to_owned(),
            )
            .await?;

        println!("Added elo_rating column to player table with default value 1200.");
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Remove elo_rating column from player table
        manager
            .alter_table(
                Table::alter()
                    .table(Player::Table)
                    .drop_column(Player::EloRating)
                    .to_owned(),
            )
            .await?;

        println!("Removed elo_rating column from player table.");
        Ok(())
    }
}

#[derive(DeriveIden)]
enum Player {
    Table,
    EloRating,
}
