//! BE-95: Dead-Letter Queue for failed archiving-pipeline jobs.
//!
//! Creates the `failed_archive_jobs` table used to persist archive jobs that
//! could not be completed due to IPFS/DB errors so they can be retried later.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table((Smdb, FailedArchiveJobs::Table))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(FailedArchiveJobs::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(FailedArchiveJobs::GameId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(FailedArchiveJobs::FailureReason)
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(FailedArchiveJobs::RetryCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(FailedArchiveJobs::FailedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        // Index for quick look-ups by game_id.
        manager
            .create_index(
                Index::create()
                    .name("idx_failed_archive_jobs_game_id")
                    .table((Smdb, FailedArchiveJobs::Table))
                    .col(FailedArchiveJobs::GameId)
                    .to_owned(),
            )
            .await?;

        println!("Created failed_archive_jobs table (BE-95 DLQ).");
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_failed_archive_jobs_game_id")
                    .table((Smdb, FailedArchiveJobs::Table))
                    .to_owned(),
            )
            .await?;

        manager
            .drop_table(
                Table::drop()
                    .table((Smdb, FailedArchiveJobs::Table))
                    .to_owned(),
            )
            .await?;

        println!("Dropped failed_archive_jobs table.");
        Ok(())
    }
}

#[derive(DeriveIden)]
enum FailedArchiveJobs {
    Table,
    Id,
    GameId,
    FailureReason,
    RetryCount,
    FailedAt,
}

#[derive(DeriveIden)]
struct Smdb;
