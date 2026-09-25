//! BE-91: Durable audit log of moderator/admin actions.
//!
//! Creates the `admin_actions` table: who (actor_id) did what (action_type) to
//! whom (target_id), why (reason), with free-form structured context
//! (metadata), and when (created_at). This is what makes disputed moderation
//! decisions auditable after the fact.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table((Smdb, AdminActions::Table))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(AdminActions::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(AdminActions::ActorId).uuid().not_null())
                    .col(
                        ColumnDef::new(AdminActions::ActionType)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(AdminActions::TargetId).uuid().null())
                    .col(ColumnDef::new(AdminActions::Reason).text().null())
                    .col(
                        ColumnDef::new(AdminActions::Metadata)
                            .json_binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AdminActions::CreatedAt)
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
                    .name("idx_admin_actions_actor_id")
                    .table((Smdb, AdminActions::Table))
                    .col(AdminActions::ActorId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_admin_actions_target_id")
                    .table((Smdb, AdminActions::Table))
                    .col(AdminActions::TargetId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_admin_actions_created_at")
                    .table((Smdb, AdminActions::Table))
                    .col(AdminActions::CreatedAt)
                    .to_owned(),
            )
            .await?;

        println!("Created admin_actions table (BE-91).");
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_admin_actions_created_at")
                    .table((Smdb, AdminActions::Table))
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_admin_actions_target_id")
                    .table((Smdb, AdminActions::Table))
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_admin_actions_actor_id")
                    .table((Smdb, AdminActions::Table))
                    .to_owned(),
            )
            .await?;

        manager
            .drop_table(Table::drop().table((Smdb, AdminActions::Table)).to_owned())
            .await?;

        println!("Dropped admin_actions table.");
        Ok(())
    }
}

#[derive(DeriveIden)]
enum AdminActions {
    Table,
    Id,
    ActorId,
    ActionType,
    TargetId,
    Reason,
    Metadata,
    CreatedAt,
}

#[derive(DeriveIden)]
struct Smdb;
