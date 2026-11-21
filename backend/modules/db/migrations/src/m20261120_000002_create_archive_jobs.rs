//! BE-152: persisted archive job records so `get_archive_status` can report
//! the real state of a game's archival instead of fabricating "Completed".

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table((Smdb, ArchiveJobs::Table))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ArchiveJobs::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ArchiveJobs::GameId)
                            .uuid()
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(ArchiveJobs::Status)
                            .string()
                            .not_null()
                            .default("pending"),
                    )
                    .col(
                        ColumnDef::new(ArchiveJobs::Progress)
                            .double()
                            .not_null()
                            .default(0.0),
                    )
                    .col(ColumnDef::new(ArchiveJobs::ErrorMessage).text().null())
                    .col(
                        ColumnDef::new(ArchiveJobs::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ArchiveJobs::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table((Smdb, ArchiveJobs::Table)).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ArchiveJobs {
    Table,
    Id,
    GameId,
    Status,
    Progress,
    ErrorMessage,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
struct Smdb;
