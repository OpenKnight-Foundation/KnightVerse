use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Player::Table)
                    .add_column(
                        ColumnDef::new(Player::EmailVerifiedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Player::Table)
                    .drop_column(Player::EmailVerifiedAt)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Player {
    Table,
    EmailVerifiedAt,
}
