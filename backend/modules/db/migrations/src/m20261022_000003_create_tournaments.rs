//! BE-114: Persist tournaments and their registered players.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("CREATE SCHEMA IF NOT EXISTS \"smdb\"")
            .await?;

        manager
            .create_table(
                Table::create()
                    .table((Smdb, Tournaments::Table))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Tournaments::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Tournaments::Name).string().not_null())
                    .col(ColumnDef::new(Tournaments::Format).string().not_null())
                    .col(ColumnDef::new(Tournaments::Status).string().not_null())
                    .col(ColumnDef::new(Tournaments::BracketData).json_binary().null())
                    .col(
                        ColumnDef::new(Tournaments::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table((Smdb, TournamentParticipants::Table))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(TournamentParticipants::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(TournamentParticipants::TournamentId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TournamentParticipants::PlayerId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TournamentParticipants::DisplayName)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TournamentParticipants::Elo)
                            .integer()
                            .not_null()
                            .default(1200),
                    )
                    .col(
                        ColumnDef::new(TournamentParticipants::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_tournament_participants_tournament_id")
                            .from((Smdb, TournamentParticipants::Table), TournamentParticipants::TournamentId)
                            .to((Smdb, Tournaments::Table), Tournaments::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uq_tournament_participants_tournament_player")
                    .table((Smdb, TournamentParticipants::Table))
                    .col(TournamentParticipants::TournamentId)
                    .col(TournamentParticipants::PlayerId)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table((Smdb, TournamentParticipants::Table))
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table((Smdb, Tournaments::Table)).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Tournaments {
    Table,
    Id,
    Name,
    Format,
    Status,
    BracketData,
    CreatedAt,
}

#[derive(DeriveIden)]
enum TournamentParticipants {
    Table,
    Id,
    TournamentId,
    PlayerId,
    DisplayName,
    Elo,
    CreatedAt,
}

#[derive(DeriveIden)]
struct Smdb;
