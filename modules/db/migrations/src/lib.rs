pub use sea_orm_migration::prelude::*;

mod m20261201_000001_add_player_glicko_ratings;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(m20261201_000001_add_player_glicko_ratings::Migration)]
    }
}
