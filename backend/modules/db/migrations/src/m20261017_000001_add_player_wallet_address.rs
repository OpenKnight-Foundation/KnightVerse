use sea_orm_migration::prelude::*;

/// Adds the Stellar account linked to each player (FE-97). Nullable, and
/// unique so a wallet can only belong to one player (Postgres allows many NULLs).
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Player::Table)
                    .add_column(ColumnDef::new(Player::WalletAddress).string_len(56).null())
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_player_wallet_address_unique")
                    .table(Player::Table)
                    .col(Player::WalletAddress)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_player_wallet_address_unique")
                    .table(Player::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Player::Table)
                    .drop_column(Player::WalletAddress)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Player {
    Table,
    WalletAddress,
}
