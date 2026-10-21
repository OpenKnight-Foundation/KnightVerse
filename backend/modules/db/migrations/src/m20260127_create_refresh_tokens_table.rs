use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(RefreshTokens::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(RefreshTokens::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(RefreshTokens::PlayerId).integer().not_null())
                    .col(ColumnDef::new(RefreshTokens::FamilyId).uuid().not_null())
                    .col(
                        ColumnDef::new(RefreshTokens::TokenHash)
                            .string()
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(RefreshTokens::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(RefreshTokens::UsedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(RefreshTokens::ExpiresAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RefreshTokens::IsRevoked)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_refresh_tokens_player_id")
                            .from(RefreshTokens::Table, RefreshTokens::PlayerId)
                            .to(Players::Table, Players::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Postgres has no inline INDEX clause in CREATE TABLE, so the lookup
        // indexes are separate statements (token_hash is already UNIQUE).
        manager
            .create_index(
                Index::create()
                    .name("idx_refresh_tokens_family_id")
                    .table(RefreshTokens::Table)
                    .col(RefreshTokens::FamilyId)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_refresh_tokens_player_id")
                    .table(RefreshTokens::Table)
                    .col(RefreshTokens::PlayerId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(RefreshTokens::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum RefreshTokens {
    Table,
    Id,
    PlayerId,
    FamilyId,
    TokenHash,
    CreatedAt,
    UsedAt,
    ExpiresAt,
    IsRevoked,
}

#[derive(Iden)]
enum Players {
    // player_id is the integer account id (refresh_token::Model::player_id is
    // i32), so it references users(id), not the UUID-keyed player table.
    #[iden = "users"]
    Table,
    Id,
}
