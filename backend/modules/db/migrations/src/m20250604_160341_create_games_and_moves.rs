use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 1. Add created_at and updated_at columns to the existing game table in smdb schema
        manager
            .alter_table(
                Table::alter()
                    .table((Smdb, Game::Table))
                    .add_column(
                        ColumnDef::new(Game::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .add_column(
                        ColumnDef::new(Game::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        // 2. Create the game_moves table in the smdb schema
        manager
            .create_table(
                Table::create()
                    .table((Smdb, GameMove::Table))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(GameMove::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(GameMove::GameId).uuid().not_null())
                    .col(ColumnDef::new(GameMove::MoveNumber).integer().not_null())
                    .col(ColumnDef::new(GameMove::San).string().not_null())
                    .col(ColumnDef::new(GameMove::Fen).string().not_null())
                    .col(
                        ColumnDef::new(GameMove::Timestamp)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_game_move_game_id")
                            .from(GameMove::Table, GameMove::GameId)
                            .to((Smdb, Game::Table), Game::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // 3. Create indexes on game_moves
        manager
            .create_index(
                Index::create()
                    .name("idx_game_moves_game_id")
                    .table((Smdb, GameMove::Table))
                    .col(GameMove::GameId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_game_moves_game_id_move_number")
                    .table((Smdb, GameMove::Table))
                    .col(GameMove::GameId)
                    .col(GameMove::MoveNumber)
                    .unique()
                    .to_owned(),
            )
            .await?;

        println!("Added created_at/updated_at to game table and created game_moves table.");
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 1. Drop indexes on game_moves
        manager
            .drop_index(
                Index::drop()
                    .name("idx_game_moves_game_id_move_number")
                    .table((Smdb, GameMove::Table))
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_game_moves_game_id")
                    .table((Smdb, GameMove::Table))
                    .to_owned(),
            )
            .await?;

        // 2. Drop game_moves table
        manager
            .drop_table(Table::drop().table((Smdb, GameMove::Table)).to_owned())
            .await?;

        // 3. Remove created_at and updated_at from game table
        manager
            .alter_table(
                Table::alter()
                    .table((Smdb, Game::Table))
                    .drop_column(Game::CreatedAt)
                    .drop_column(Game::UpdatedAt)
                    .to_owned(),
            )
            .await?;

        println!("Dropped game_moves table and removed created_at/updated_at from game table.");
        Ok(())
    }
}

#[derive(DeriveIden)]
enum Game {
    Table,
    Id,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum GameMove {
    Table,
    Id,
    GameId,
    MoveNumber,
    San,
    Fen,
    Timestamp,
}

#[derive(DeriveIden)]
struct Smdb;
