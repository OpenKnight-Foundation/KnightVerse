pub mod db;

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use sea_orm::{ConnectionTrait, DatabaseBackend, DbConn, DbErr, Statement};

    use crate::db::db::get_db;

    const DATABASE_NAME: &str = "player";

    async fn table_exists(db: &DbConn, table_name: &str) -> Result<bool, DbErr> {
        let db_backend = db.get_database_backend();
        let (query, is_count) = match db_backend {
            DatabaseBackend::Postgres => (
                format!(
                    "SELECT EXISTS (
                        SELECT 1 FROM information_schema.tables 
                        WHERE table_name = '{}'
                    )",
                    table_name
                ),
                false,
            ),
            _ => (format!(""), false)
        };
    
        let result = db.query_one(Statement::from_string(db_backend, query)).await?;
        
        if is_count {
            Ok(result
                .map(|row| row.try_get_by_index::<i64>(0))
                .transpose()?
                .map(|count| count > 0)
                .unwrap_or(false))
        } else {
            Ok(result
                .map(|row| row.try_get_by_index::<bool>(0))
                .transpose()?
                .unwrap_or(false))
        }
    }
    
    #[async_std::test]
    async fn test_table_exists() -> Result<(), DbErr> {
        if std::env::var("DATABASE_URL").is_err() {
            return Ok(());
        }

        let db = get_db().await;
        
        assert!(
            table_exists(&db, DATABASE_NAME).await?,
            "Table '{}' should exist", 
            DATABASE_NAME
        );
        
        Ok(())
    }

    async fn get_column_type(
        db: &DbConn, 
        table_name: &str, 
        column_name: &str
    ) -> Result<Option<String>, DbErr> {
        let db_backend = db.get_database_backend();
        let query = match db_backend {
            DatabaseBackend::Postgres => format!(
                "SELECT data_type FROM information_schema.columns 
                 WHERE table_name = '{}' AND column_name = '{}'",
                table_name, column_name
            ),
            _ => format!("")
        };
    
        db.query_one(Statement::from_string(db_backend, query))
            .await?
            .map(|row| row.try_get_by_index::<String>(0))
            .transpose()
    }


    #[async_std::test]
    async fn accurate_column_types() -> Result<(), DbErr> {
        if std::env::var("DATABASE_URL").is_err() {
            return Ok(());
        }

        let db = get_db().await;

        let columns_and_types = HashMap::from([
            ("id","uuid"),
            ("username","character varying"),    
            ("email","character varying"),
            ("password_hash","bytea"),
            ("biography","text"),
            ("country","character varying"),
            ("flair","character varying"),
            ("real_name","character varying"),
            ("location","character varying"),
            ("fide_rating","integer"),
            ("social_links","ARRAY")
        ]);

        for (column, colunn_type) in columns_and_types.iter(){
            assert_eq!(
                get_column_type(&db, DATABASE_NAME, column).await?,
                Some(colunn_type.to_string())
            )
        }

        Ok(())
    }
}
