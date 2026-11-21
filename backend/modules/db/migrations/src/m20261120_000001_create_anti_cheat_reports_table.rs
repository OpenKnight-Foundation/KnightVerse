use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table((Smdb, AntiCheatReports::Table))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(AntiCheatReports::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(AntiCheatReports::GameId).uuid().not_null())
                    .col(ColumnDef::new(AntiCheatReports::PlayerId).uuid().not_null())
                    .col(
                        ColumnDef::new(AntiCheatReports::Flagged)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(ColumnDef::new(AntiCheatReports::FlagReason).text().null())
                    .col(
                        ColumnDef::new(AntiCheatReports::Metrics)
                            .json_binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AntiCheatReports::Report)
                            .json_binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AntiCheatReports::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_anti_cheat_reports_game_id")
                    .table((Smdb, AntiCheatReports::Table))
                    .col(AntiCheatReports::GameId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_anti_cheat_reports_player_id")
                    .table((Smdb, AntiCheatReports::Table))
                    .col(AntiCheatReports::PlayerId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_anti_cheat_reports_flagged")
                    .table((Smdb, AntiCheatReports::Table))
                    .col(AntiCheatReports::Flagged)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_anti_cheat_reports_created_at")
                    .table((Smdb, AntiCheatReports::Table))
                    .col(AntiCheatReports::CreatedAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_anti_cheat_reports_created_at")
                    .table((Smdb, AntiCheatReports::Table))
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_anti_cheat_reports_flagged")
                    .table((Smdb, AntiCheatReports::Table))
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_anti_cheat_reports_player_id")
                    .table((Smdb, AntiCheatReports::Table))
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_anti_cheat_reports_game_id")
                    .table((Smdb, AntiCheatReports::Table))
                    .to_owned(),
            )
            .await?;

        manager
            .drop_table(Table::drop().table((Smdb, AntiCheatReports::Table)).to_owned())
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum AntiCheatReports {
    Table,
    Id,
    GameId,
    PlayerId,
    Flagged,
    FlagReason,
    Metrics,
    Report,
    CreatedAt,
}

#[derive(DeriveIden)]
struct Smdb;
