use sea_orm_migration::prelude::*;

/// Adds `player.glicko_ratings` (BE-187): Glicko-2 rating, RD and volatility
/// per time control as JSON, e.g. `{ "rapid": { "rating", "rd", "volatility" } }`.
/// Players start with `{}`; a time control without an entry is provisional.
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
                        ColumnDef::new(Player::GlickoRatings)
                            .json_binary()
                            .not_null()
                            .default(Expr::cust("'{}'::jsonb")),
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
                    .drop_column(Player::GlickoRatings)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Player {
    Table,
    GlickoRatings,
}
