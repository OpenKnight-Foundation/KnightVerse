pub mod db {
    use sea_orm::{ConnectOptions, Database, DatabaseConnection};

    pub async fn get_db() -> DatabaseConnection {
        dotenv::dotenv().ok();
        let connect_options = ConnectOptions::new(
            std::env::var("DATABASE_URL").expect("DATABASE_URL is not defined"),
        )
        .to_owned();

        let db: DatabaseConnection = Database::connect(connect_options).await.unwrap();

        db
    }
}
