use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum Player {
    Table,
    Id,
    RatingBullet,
    RatingBlitz,
    RatingRapid,
    RatingClassical,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add Glicko-2 columns (rating, rd, volatility) per time control.
        // Existing Elo values are carried over as the Glicko-2 rating (mu scale
        // is handled in the application layer); RD starts at the maximum 350
        // and volatility at the default 0.06, marking all players provisional.
        for (rating_col, rd_col, vol_col) in [
            ("rating_bullet", "rd_bullet", "volatility_bullet"),
            ("rating_blitz", "rd_blitz", "volatility_blitz"),
            ("rating_rapid", "rd_rapid", "volatility_rapid"),
            ("rating_classical", "rd_classical", "volatility_classical"),
        ] {
            let _ = rating_col;
            manager
                .alter_table(
                    Table::alter()
                        .table(Player::Table)
                        .add_column(
                            ColumnDef::new(Alias::new(rd_col))
                                .double()
                                .not_null()
                                .default(350.0),
                        )
                        .add_column(
                            ColumnDef::new(Alias::new(vol_col))
                                .double()
                                .not_null()
                                .default(0.06),
                        )
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for (rd_col, vol_col) in [
            ("rd_bullet", "volatility_bullet"),
            ("rd_blitz", "volatility_blitz"),
            ("rd_rapid", "volatility_rapid"),
            ("rd_classical", "volatility_classical"),
        ] {
            manager
                .alter_table(
                    Table::alter()
                        .table(Player::Table)
                        .drop_column(Alias::new(rd_col))
                        .drop_column(Alias::new(vol_col))
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }
}
