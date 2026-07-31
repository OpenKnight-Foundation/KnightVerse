pub use sea_orm_migration::prelude::*;

mod m20250123_000001_create_users_table;
mod m20250324_add_elo_rating_to_player;
mod m20250428_121011_create_players_table;
mod m20250429_163843_create_games_table;
mod m20250429_192832_add_common_indexes;
mod m20250604_160341_create_games_and_moves;
mod m20250605_090000_add_game_search_indexes;
mod m20260127_180000_add_game_imported_flag;
mod m20260127_create_refresh_tokens_table;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20250123_000001_create_users_table::Migration),
            Box::new(m20250428_121011_create_players_table::Migration),
            Box::new(m20250429_163843_create_games_table::Migration),
            Box::new(m20250429_192832_add_common_indexes::Migration),
            Box::new(m20250604_160341_create_games_and_moves::Migration),
            Box::new(m20250605_090000_add_game_search_indexes::Migration),
            Box::new(m20260127_create_refresh_tokens_table::Migration),
            Box::new(m20260127_180000_add_game_imported_flag::Migration),
            Box::new(m20250324_add_elo_rating_to_player::Migration),
        ]
    }
}
