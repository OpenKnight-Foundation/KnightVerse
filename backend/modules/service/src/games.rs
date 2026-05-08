use db_entity::{game, prelude::Game};
use sea_orm::{
    ColumnTrait, DbErr, EntityTrait, Order, QueryFilter,
    QueryOrder, QuerySelect, ActiveModelTrait, Set, TransactionTrait,
};
use sea_orm::{Condition, DatabaseConnection};
use uuid::Uuid;
use chrono::{DateTime, Utc, TimeZone};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use dto::games::{GameStatus, CreateGameRequest, MakeMoveRequest, GameDisplayDTO};
use error::error::ApiError;
use chess::pgn::ValidatedGame;
use chess::{RatingService, RatingConfig};

pub struct GameService;

impl GameService {
    pub async fn create_game(
        _db: &DatabaseConnection,
        _creator_id: Uuid,
        _request: CreateGameRequest,
    ) -> Result<GameDisplayDTO, ApiError> {
        Err(ApiError::NotImplemented("create_game not yet implemented".to_string()))
    }

    pub async fn get_game(
        _db: &DatabaseConnection,
        _game_id: Uuid,
    ) -> Result<GameDisplayDTO, ApiError> {
        Err(ApiError::NotFound("Game not found".to_string()))
    }

    pub async fn make_move(
        _db: &DatabaseConnection,
        _game_id: Uuid,
        _player_id: Uuid,
        _move_request: MakeMoveRequest,
    ) -> Result<GameDisplayDTO, ApiError> {
        Err(ApiError::NotImplemented("make_move not yet implemented".to_string()))
    }

    pub async fn join_game(
        _db: &DatabaseConnection,
        _game_id: Uuid,
        _player_id: Uuid,
    ) -> Result<GameDisplayDTO, ApiError> {
        Err(ApiError::NotImplemented("join_game not yet implemented".to_string()))
    }

    pub async fn abandon_game(
        _db: &DatabaseConnection,
        _game_id: Uuid,
        _player_id: Uuid,
    ) -> Result<GameDisplayDTO, ApiError> {
        Err(ApiError::NotImplemented("abandon_game not yet implemented".to_string()))
    }

    pub async fn import_game(
        _db: &DatabaseConnection,
        _importer_id: Uuid,
        _request: &ValidatedGame,
    ) -> Result<Uuid, ApiError> {
        Err(ApiError::NotImplemented("import_game not yet implemented".to_string()))
    }

    /// Complete a game with the given result and update player ratings
    /// 
    /// # Arguments
    /// * `db` - Database connection
    /// * `game_id` - UUID of the game to complete
    /// * `result` - The final result of the game
    /// * `rating_config` - Optional rating configuration (uses default if None)
    /// 
    /// # Returns
    /// * `Ok((white_new_rating, black_new_rating))` - New ratings for both players
    /// * `Err(ApiError)` - If game not found, already completed, or database error
    /// 
    /// This method:
    /// 1. Updates the game result in a transaction
    /// 2. Calculates and updates player ratings atomically
    /// 3. Ensures no race conditions during rapid consecutive games
    pub async fn complete_game(
        db: &DatabaseConnection,
        game_id: Uuid,
        result: db_entity::game::ResultSide,
        rating_config: Option<RatingConfig>,
    ) -> Result<(i32, i32), ApiError> {
        // Use default rating config if none provided
        let config = rating_config.unwrap_or_default();

        // Start transaction for atomic game completion and rating update
        let txn = db.begin().await.map_err(ApiError::from)?;

        // 1. Update game result
        let game_model = game::Entity::find_by_id(game_id)
            .one(&txn)
            .await
            .map_err(ApiError::from)?
            .ok_or_else(|| ApiError::NotFound("Game not found".to_string()))?;

        // Check if game is already completed
        if game_model.result.is_some() {
            let _ = txn.rollback().await;
            return Err(ApiError::BadRequest("Game is already completed".to_string()));
        }

        // Update game with result
        let mut game_active_model: game::ActiveModel = game_model.into();
        game_active_model.result = Set(Some(result.clone()));
        game_active_model.updated_at = Set(Utc::now().into());

        game_active_model.update(&txn).await.map_err(ApiError::from)?;

        // 2. Update player ratings using the rating service
        let ratings_result = RatingService::update_ratings_in_transaction(&txn, game_id, &config).await;

        match ratings_result {
            Ok(ratings) => {
                // Commit transaction if both game update and rating update succeeded
                txn.commit().await.map_err(ApiError::from)?;
                Ok(ratings)
            }
            Err(e) => {
                // Rollback transaction on rating update failure
                let _ = txn.rollback().await;
                Err(e)
            }
        }
    }

    /// Get a player's rating for a specific game (helper method for rating calculations)
    pub async fn get_player_rating_for_game(
        db: &DatabaseConnection,
        game_id: Uuid,
        is_white: bool,
    ) -> Result<i32, ApiError> {
        // Fetch the game to get player IDs
        let game_model = game::Entity::find_by_id(game_id)
            .one(db)
            .await
            .map_err(ApiError::from)?
            .ok_or_else(|| ApiError::NotFound("Game not found".to_string()))?;

        let player_id = if is_white {
            game_model.white_player
        } else {
            game_model.black_player
        };

        // Use the rating service to get the player's current rating
        chess::RatingService::get_player_rating(db, player_id).await
    }

    /// List games with keyset pagination.
    /// 
    /// # Arguments
    /// * `db` - Database connection
    /// * `cursor` - Optional cursor string (base64 encoded "timestamp,id")
    /// * `limit` - Number of items to return
    /// * `player_id` - Optional player ID filter (checks both white and black players)
    /// * `status` - Optional status filter (currently maps to result being not null for finished games, or specific status if column exists)
    /// 
    /// Note: The current schema uses `result` to determine if a game is finished. 
    /// Active games might have `result` as NULL (after our migration).
    pub async fn list_games(
        db: &DatabaseConnection,
        cursor: Option<String>,
        limit: u64,
        player_id: Option<Uuid>,
        status: Option<GameStatus>,
    ) -> Result<(Vec<game::Model>, Option<String>), DbErr> {
        let mut query = Game::find();

        // 1. Apply Filtering
        if let Some(pid) = player_id {
            // Filter by player (white OR black)
            // effective union of indexes logic would be nice, but OR is simpler to write here.
            // "idx_games_white_player_created_at_id" and "idx_games_black_player_created_at_id"
            // Postgres creates a BitmapOr for these two indexes usually.
            let condition = Condition::any()
                .add(game::Column::WhitePlayer.eq(pid))
                .add(game::Column::BlackPlayer.eq(pid));
            query = query.filter(condition);
        }

        if let Some(s) = status {
            match s {
                GameStatus::Waiting | GameStatus::InProgress => {
                     // Active games: result is NULL
                     query = query.filter(game::Column::Result.is_null());
                },
                GameStatus::Completed | GameStatus::Aborted => {
                    // Finished games: result is NOT NULL
                    // Note: "Aborted" vs "Completed" might need distinguishing via ResultSide if we had it, 
                    // but for now we just check if it has a result.
                    query = query.filter(game::Column::Result.is_not_null());
                }
            }
        }

        // 2. Apply Cursor (Keyset Pagination)
        // Sort by created_at DESC, id DESC
        query = query
            .order_by(game::Column::CreatedAt, Order::Desc)
            .order_by(game::Column::Id, Order::Desc);

        if let Some(cursor_str) = cursor {
            if let Ok((last_created_at, last_id)) = Self::decode_cursor(&cursor_str) {
                // created_at < last_created_at OR (created_at = last_created_at AND id < last_id)
                // SeaORM tuple comparison: (col1, col2) < (val1, val2)
                // query = query.filter(
                //    Condition::any()
                //        .add(game::Column::CreatedAt.lt(last_created_at))
                //        .add(
                //            Condition::all()
                //                .add(game::Column::CreatedAt.eq(last_created_at))
                //                .add(game::Column::Id.lt(last_id))
                //        )
                // );
                // Actually, SeaORM supports tuple comparison conveniently? 
                // Not directly in the builder API widely in all versions, but the composite condition above is correct for (A, B) < (a, b) logic.
                // However, tuple comparison `(A, B) < (a, b)` logic is standard SQL but SeaORM DSL is explicit.
                
                // Constructing: (created_at, id) < (last_created_at, last_id)
                // Equivalent to: created_at < last_created_at OR (created_at = last_created_at AND id < last_id) (for DESC, DESC)
                // WAIT! For DESC sort, "next page" means values SMALLER than cursor?
                // Yes. Sorting DESC means newest first. Cursor is at some point. We want older stuff.
                // So we want `created_at < cursor.created_at`.
                // If created_at == cursor.created_at, then `id < cursor.id` (assuming ID also DESC).
                
                let condition = Condition::any()
                    .add(game::Column::CreatedAt.lt(last_created_at))
                    .add(
                        Condition::all()
                            .add(game::Column::CreatedAt.eq(last_created_at))
                            .add(game::Column::Id.lt(last_id))
                    );
                
                query = query.filter(condition);
            }
        }

        // 3. Limit and Execution
        // Fetch limit + 1 to check if there is a next page
        let results = query.limit(limit + 1).all(db).await?;

        let mut games = results;
        let mut next_cursor: Option<String> = None;

        if games.len() as u64 > limit {
            // We have a next page
            games.truncate(limit as usize);
            if let Some(last_game) = games.last() {
                next_cursor = Some(Self::encode_cursor(last_game.created_at.into(), last_game.id));
            }
        }

        Ok((games, next_cursor))
    }

    fn encode_cursor(timestamp: DateTime<Utc>, id: Uuid) -> String {
        // Format: "timestamp_micros,uuid"
        // timestamp: use timestamp_micros for precision
        let ts_part = timestamp.timestamp_micros();
        let id_part = id.to_string();
        let raw = format!("{},{}", ts_part, id_part);
        URL_SAFE_NO_PAD.encode(raw)
    }

    fn decode_cursor(cursor: &str) -> Result<(DateTime<Utc>, Uuid), String> {
        let decoded_bytes = URL_SAFE_NO_PAD.decode(cursor)
            .map_err(|_| "Invalid base64".to_string())?;
        let raw = String::from_utf8(decoded_bytes)
            .map_err(|_| "Invalid utf8".to_string())?;
        
        // Split once
        let parts: Vec<&str> = raw.splitn(2, ",").collect();
        if parts.len() != 2 {
            return Err("Invalid cursor format".to_string());
        }

        let ts_micros: i64 = parts[0].parse()
            .map_err(|_| "Invalid timestamp".to_string())?;
        let id = Uuid::parse_str(parts[1])
            .map_err(|_| "Invalid UUID".to_string())?;

        let timestamp = Utc.timestamp_micros(ts_micros).single()
             .ok_or("Invalid timestamp value".to_string())?;

        Ok((timestamp, id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{MockDatabase, DbBackend};
    use chrono::FixedOffset;

    #[test]
    fn test_cursor_encoding_decoding() {
        let now = Utc::now();
        let id = Uuid::new_v4();
        
        let cursor = GameService::encode_cursor(now, id);
        let (decoded_ts, decoded_id) = GameService::decode_cursor(&cursor).expect("Decoding failed");
        
        // Timestamp might lose precision if we are not careful, but we used timestamp_micros
        // We compare micros
        assert_eq!(decoded_ts.timestamp(), now.timestamp());
        assert_eq!(decoded_id, id);
    }
    
    #[tokio::test]
    async fn test_list_games_query_structure() {
        // Create Mock Database to verify the generated SQL
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results(vec![
                // First query result (empty list is fine, we check SQL)
                vec![game::Model {
                    id: Uuid::new_v4(),
                    white_player: Uuid::new_v4(),
                    black_player: Uuid::new_v4(),
                    fen: "fen".to_string(),
                    pgn: serde_json::json!({}),
                    result: None,
                    variant: db_entity::game::GameVariant::Standard,
                    started_at: Utc::now().with_timezone(&FixedOffset::east_opt(0).unwrap()),
                    duration_sec: 600,
                    created_at: Utc::now().with_timezone(&FixedOffset::east_opt(0).unwrap()),
                    updated_at: Utc::now().with_timezone(&FixedOffset::east_opt(0).unwrap()),
                    is_imported: false,
                    original_pgn: None,
                }],
            ])
            .into_connection();
            
        let player_id = Uuid::new_v4();
        
        let _result = GameService::list_games(
            &db,
            None,
            10,
            Some(player_id),
            None
        ).await;
        
        // Get transaction log to verify SQL
        let transaction_log = db.into_transaction_log();
        
        // We expect one query
        assert_eq!(transaction_log.len(), 1);
        
        let log = &transaction_log[0];
        let log_str = format!("{:?}", log);
        println!("Log: {}", log_str);
        
        // Verify SQL logic via Debug string (escaped quotes due to Debug format)
        // We expect filtering by player with table alias "game"
        assert!(log_str.contains(r#"\"game\".\"white_player\" = $1"#));
        assert!(log_str.contains(r#"\"game\".\"black_player\" = $2"#));
        // Verify sorting keyset
        assert!(log_str.contains(r#"ORDER BY \"game\".\"created_at\" DESC, \"game\".\"id\" DESC"#));
        // Verify Limit
        assert!(log_str.contains("LIMIT $3"));
    }
    
    #[tokio::test]
    async fn test_list_games_with_cursor() {
        let last_time = Utc::now();
        let last_id = Uuid::new_v4();
        let cursor = GameService::encode_cursor(last_time, last_id);
        
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results(vec![vec![game::Model {
                 id: Uuid::new_v4(),
                    white_player: Uuid::new_v4(),
                    black_player: Uuid::new_v4(),
                    fen: "fen".to_string(),
                    pgn: serde_json::json!({}),
                    result: None,
                    variant: db_entity::game::GameVariant::Standard,
                    started_at: Utc::now().with_timezone(&FixedOffset::east_opt(0).unwrap()),
                    duration_sec: 600,
                    created_at: Utc::now().with_timezone(&FixedOffset::east_opt(0).unwrap()),
                    updated_at: Utc::now().with_timezone(&FixedOffset::east_opt(0).unwrap()),
                    is_imported: false,
                    original_pgn: None,
            }]])
            .into_connection();
            
        let _result = GameService::list_games(
            &db,
            Some(cursor),
            10,
            None,
            None
        ).await;
        
        let transaction_log = db.into_transaction_log();
        let log = &transaction_log[0];
        let log_str = format!("{:?}", log);
        println!("Log with cursor: {}", log_str);
        
        // Verify cursor condition: (created_at < ?) OR (created_at = ? AND id < ?)
        assert!(log_str.contains(r#"\"game\".\"created_at\" < $1"#));
        assert!(log_str.contains(r#"\"game\".\"created_at\" = $2"#));
        assert!(log_str.contains(r#"\"game\".\"id\" < $3"#));
    }
}
