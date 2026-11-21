use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Players::Table)
                    .add_column(
                        ColumnDef::new(Players::RatingBullet)
                            .double()
                            .not_null()
                            .default(1500.0),
                    )
                    .add_column(
                        ColumnDef::new(Players::RdBullet)
                            .double()
                            .not_null()
                            .default(350.0),
                    )
                    .add_column(
                        ColumnDef::new(Players::VolatilityBullet)
                            .double()
                            .not_null()
                            .default(0.06),
                    )
                    .add_column(
                        ColumnDef::new(Players::RatingBlitz)
                            .double()
                            .not_null()
                            .default(1500.0),
                    )
                    .add_column(
                        ColumnDef::new(Players::RdBlitz)
                            .double()
                            .not_null()
                            .default(350.0),
                    )
                    .add_column(
                        ColumnDef::new(Players::VolatilityBlitz)
                            .double()
                            .not_null()
                            .default(0.06),
                    )
                    .add_column(
                        ColumnDef::new(Players::RatingRapid)
                            .double()
                            .not_null()
                            .default(1500.0),
                    )
                    .add_column(
                        ColumnDef::new(Players::RdRapid)
                            .double()
                            .not_null()
                            .default(350.0),
                    )
                    .add_column(
                        ColumnDef::new(Players::VolatilityRapid)
                            .double()
                            .not_null()
                            .default(0.06),
                    )
                    .add_column(
                        ColumnDef::new(Players::RatingClassical)
                            .double()
                            .not_null()
                            .default(1500.0),
                    )
                    .add_column(
                        ColumnDef::new(Players::RdClassical)
                            .double()
                            .not_null()
                            .default(350.0),
                    )
                    .add_column(
                        ColumnDef::new(Players::VolatilityClassical)
                            .double()
                            .not_null()
                            .default(0.06),
                    )
                    .to_owned(),
            )
            .await?;

        // Migrate existing Elo ratings into the Glicko-2 rating fields.
        // Existing players keep their Elo value as the rating and receive a
        // high RD so they are treated as provisional until they play enough games.
        let db = manager.get_connection();
        db.execute_unprepared(
            "UPDATE players SET \
                rating_bullet = COALESCE(elo_bullet, 1500.0), \
                rating_blitz = COALESCE(elo_blitz, 1500.0), \
                rating_rapid = COALESCE(elo_rapid, 1500.0), \
                rating_classical = COALESCE(elo_classical, 1500.0)",
        )
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Players::Table)
                    .drop_column(Players::RatingBullet)
                    .drop_column(Players::RdBullet)
                    .drop_column(Players::VolatilityBullet)
                    .drop_column(Players::RatingBlitz)
                    .drop_column(Players::RdBlitz)
                    .drop_column(Players::VolatilityBlitz)
                    .drop_column(Players::RatingRapid)
                    .drop_column(Players::RdRapid)
                    .drop_column(Players::VolatilityRapid)
                    .drop_column(Players::RatingClassical)
                    .drop_column(Players::RdClassical)
                    .drop_column(Players::VolatilityClassical)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Players {
    Table,
    RatingBullet,
    RdBullet,
    VolatilityBullet,
    RatingBlitz,
    RdBlitz,
    VolatilityBlitz,
    RatingRapid,
    RdRapid,
    VolatilityRapid,
    RatingClassical,
    RdClassical,
    VolatilityClassical,
}
