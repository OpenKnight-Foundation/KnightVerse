use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use chess::pgn::ValidatedGame;
use chess::{RatingConfig, RatingService};
use chrono::{DateTime, TimeZone, Utc};
use db_entity::{game, prelude::Game};
use dto::games::{
    CreateGameRequest, GameDisplayDTO, GameResult, GameStatus, MakeMoveRequest,
};
use error::error::ApiError;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DbErr, EntityTrait, Order, QueryFilter, QueryOrder, QuerySelect,
    Set, TransactionTrait,
};
use sea_orm::{Condition, DatabaseConnection};
use uuid::Uuid;

/// Starting FEN for a standard chess game.
const STARTING_FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

pub struct GameService;

/// Nil UUID used as a placeholder when a game is waiting for a second player.
const NIL_UUID: Uuid = Uuid::nil();

impl GameService {
    /// Create a new game and persist it in the database.
    ///
    /// The creator is assigned as the white player.  The black player slot is
    /// initialised with a nil UUID until an opponent joins via [`join_game`].
    pub async fn create_game(
        db: &DatabaseConnection,
        creator_id: Uuid,
        request: CreateGameRequest,
    ) -> Result<GameDisplayDTO, ApiError> {
        let now = Utc::now();
        let game_id = Uuid::new_v4();

        let now_fixed = now.with_timezone(&chrono::FixedOffset::east_opt(0).unwrap());
        let active_model = game::ActiveModel {
            id: Set(game_id),
            white_player: Set(creator_id),
            black_player: Set(NIL_UUID),
            fen: Set(STARTING_FEN.to_string()),
            pgn: Set(serde_json::json!([])),
            result: Set(None),
            variant: Set(db_entity::game::GameVariant::Standard),
            started_at: Set(now_fixed),
            duration_sec: Set(request.time_control),
            created_at: Set(now_fixed),
            updated_at: Set(now_fixed),
            is_imported: Set(false),
            original_pgn: Set(None),
        };

        let model = active_model.insert(db).await.map_err(ApiError::from)?;
        Ok(Self::model_to_dto(model))
    }

    /// Fetch a single game by its UUID.
    pub async fn get_game(
        db: &DatabaseConnection,
        game_id: Uuid,
    ) -> Result<GameDisplayDTO, ApiError> {
        let model = game::Entity::find_by_id(game_id)
            .one(db)
            .await
            .map_err(ApiError::from)?
            .ok_or_else(|| ApiError::NotFound("Game not found".to_string()))?;

        Ok(Self::model_to_dto(model))
    }

    /// Record a move for the given game.
    ///
    /// Validates that the game is active, the caller is a participant, and it is
    /// the caller's turn.  The move string is appended to the PGN move history
    /// stored in the game's `pgn` JSON column.
    pub async fn make_move(
        db: &DatabaseConnection,
        game_id: Uuid,
        player_id: Uuid,
        move_request: MakeMoveRequest,
    ) -> Result<GameDisplayDTO, ApiError> {
        let model = game::Entity::find_by_id(game_id)
            .one(db)
            .await
            .map_err(ApiError::from)?
            .ok_or_else(|| ApiError::NotFound("Game not found".to_string()))?;

        // Game must be in progress (black_player must have joined).
        if model.black_player == NIL_UUID {
            return Err(ApiError::BadRequest(
                "Game has not started yet – waiting for opponent".to_string(),
            ));
        }

        // Game must not be completed.
        if model.result.is_some()
            && model.result.as_ref() != Some(&db_entity::game::ResultSide::Ongoing)
        {
            return Err(ApiError::BadRequest(
                "Game is already completed".to_string(),
            ));
        }

        // Determine whose turn it is from the move count.
        let moves: Vec<String> = model
            .pgn
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        let is_white_turn = moves.len() % 2 == 0;
        let is_white = model.white_player == player_id;

        if player_id != model.white_player && player_id != model.black_player {
            return Err(ApiError::Forbidden(
                "You are not a participant in this game".to_string(),
            ));
        }

        if is_white != is_white_turn {
            return Err(ApiError::Forbidden("It is not your turn".to_string()));
        }

        // Append the move to the PGN history.
        let mut new_moves = moves;
        new_moves.push(move_request.chess_move.clone());

        let mut active: game::ActiveModel = model.into();
        active.pgn = Set(serde_json::json!(new_moves));
        active.updated_at = Set(Utc::now().into());

        let updated = active.update(db).await.map_err(ApiError::from)?;
        Ok(Self::model_to_dto(updated))
    }

    /// Join an existing game as the black player.
    ///
    /// The game must exist, must not already be completed, and must not already
    /// have a second player.
    pub async fn join_game(
        db: &DatabaseConnection,
        game_id: Uuid,
        player_id: Uuid,
    ) -> Result<GameDisplayDTO, ApiError> {
        let model = game::Entity::find_by_id(game_id)
            .one(db)
            .await
            .map_err(ApiError::from)?
            .ok_or_else(|| ApiError::NotFound("Game not found".to_string()))?;

        // Cannot join a completed game.
        if model.result.is_some()
            && model.result.as_ref() != Some(&db_entity::game::ResultSide::Ongoing)
        {
            return Err(ApiError::BadRequest(
                "Game is already completed".to_string(),
            ));
        }

        // Black slot must be empty.
        if model.black_player != NIL_UUID {
            return Err(ApiError::BadRequest(
                "Game already has two players".to_string(),
            ));
        }

        // Creator cannot join their own game twice.
        if model.white_player == player_id {
            return Err(ApiError::BadRequest(
                "Cannot join your own game as opponent".to_string(),
            ));
        }

        let mut active: game::ActiveModel = model.into();
        active.black_player = Set(player_id);
        active.result = Set(Some(db_entity::game::ResultSide::Ongoing));
        active.started_at = Set(Utc::now().into());
        active.updated_at = Set(Utc::now().into());

        let updated = active.update(db).await.map_err(ApiError::from)?;
        Ok(Self::model_to_dto(updated))
    }

    /// Abandon (forfeit) a game.
    ///
    /// Only a participant may abandon a game.  The result is set to
    /// [`ResultSide::Abandoned`].
    pub async fn abandon_game(
        db: &DatabaseConnection,
        game_id: Uuid,
        player_id: Uuid,
    ) -> Result<GameDisplayDTO, ApiError> {
        let model = game::Entity::find_by_id(game_id)
            .one(db)
            .await
            .map_err(ApiError::from)?
            .ok_or_else(|| ApiError::NotFound("Game not found".to_string()))?;

        if player_id != model.white_player && player_id != model.black_player {
            return Err(ApiError::Forbidden(
                "You are not a participant in this game".to_string(),
            ));
        }

        // Already finished?
        if model.result.is_some()
            && model.result.as_ref() != Some(&db_entity::game::ResultSide::Ongoing)
        {
            return Err(ApiError::BadRequest(
                "Game is already completed".to_string(),
            ));
        }

        let mut active: game::ActiveModel = model.into();
        active.result = Set(Some(db_entity::game::ResultSide::Abandoned));
        active.updated_at = Set(Utc::now().into());

        let updated = active.update(db).await.map_err(ApiError::from)?;
        Ok(Self::model_to_dto(updated))
    }

    pub async fn import_game(
        db: &DatabaseConnection,
        _importer_id: Uuid,
        request: &ValidatedGame,
    ) -> Result<Uuid, ApiError> {
        let now = Utc::now();
        let game_id = Uuid::new_v4();

        // Collect moves from the validated game into a JSON array.
        let moves: Vec<String> = request
            .moves
            .iter()
            .map(|m| m.to_string())
            .collect();

        let result = match request.headers.result {
            chess::PgnGameResult::WhiteWins => Some(db_entity::game::ResultSide::WhiteWins),
            chess::PgnGameResult::BlackWins => Some(db_entity::game::ResultSide::BlackWins),
            chess::PgnGameResult::Draw => Some(db_entity::game::ResultSide::Draw),
            chess::PgnGameResult::Ongoing => None,
        };

        let now_fixed = now.with_timezone(&chrono::FixedOffset::east_opt(0).unwrap());
        let active_model = game::ActiveModel {
            id: Set(game_id),
            white_player: Set(Uuid::nil()), // unknown players for imports
            black_player: Set(Uuid::nil()),
            fen: Set(request.final_fen.clone()),
            pgn: Set(serde_json::json!(moves)),
            result: Set(result),
            variant: Set(db_entity::game::GameVariant::Standard),
            started_at: Set(now_fixed),
            duration_sec: Set(0),
            created_at: Set(now_fixed),
            updated_at: Set(now_fixed),
            is_imported: Set(true),
            original_pgn: Set(None),
        };

        active_model.insert(db).await.map_err(ApiError::from)?;
        Ok(game_id)
    }

    // -----------------------------------------------------------------------
    // Helper: convert a DB game::Model into the API DTO.
    // -----------------------------------------------------------------------
    fn model_to_dto(model: game::Model) -> GameDisplayDTO {
        let status = match &model.result {
            None => GameStatus::Waiting,
            Some(db_entity::game::ResultSide::Ongoing) => GameStatus::InProgress,
            Some(db_entity::game::ResultSide::Abandoned) => GameStatus::Aborted,
            _ => GameStatus::Completed,
        };

        let result = match &model.result {
            Some(db_entity::game::ResultSide::WhiteWins) => GameResult::WhiteWin,
            Some(db_entity::game::ResultSide::BlackWins) => GameResult::BlackWin,
            Some(db_entity::game::ResultSide::Draw) => GameResult::Draw,
            _ => GameResult::InProgress,
        };

        let move_history: Vec<String> = model
            .pgn
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        let black_player_id = if model.black_player == NIL_UUID {
            None
        } else {
            Some(model.black_player)
        };

        let started_at = model.started_at.with_timezone(&Utc);
        let created_at = model.created_at.with_timezone(&Utc);
        let updated_at = model.updated_at.with_timezone(&Utc);

        GameDisplayDTO {
            id: model.id,
            white_player_id: model.white_player,
            black_player_id,
            status,
            result,
            current_fen: model.fen,
            move_history,
            time_control: model.duration_sec,
            increment: 0,
            white_time_remaining: model.duration_sec,
            black_time_remaining: model.duration_sec,
            created_at,
            started_at: Some(started_at),
            updated_at,
        }
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
            return Err(ApiError::BadRequest(
                "Game is already completed".to_string(),
            ));
        }

        // Update game with result
        let mut game_active_model: game::ActiveModel = game_model.into();
        game_active_model.result = Set(Some(result.clone()));
        game_active_model.updated_at = Set(Utc::now().into());

        game_active_model
            .update(&txn)
            .await
            .map_err(ApiError::from)?;

        // 2. Update player ratings using the rating service
        let ratings_result =
            RatingService::update_ratings_in_transaction(&txn, game_id, &config).await;

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
                }
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
                            .add(game::Column::Id.lt(last_id)),
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
                next_cursor = Some(Self::encode_cursor(
                    last_game.created_at.into(),
                    last_game.id,
                ));
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
        let decoded_bytes = URL_SAFE_NO_PAD
            .decode(cursor)
            .map_err(|_| "Invalid base64".to_string())?;
        let raw = String::from_utf8(decoded_bytes).map_err(|_| "Invalid utf8".to_string())?;

        // Split once
        let parts: Vec<&str> = raw.splitn(2, ",").collect();
        if parts.len() != 2 {
            return Err("Invalid cursor format".to_string());
        }

        let ts_micros: i64 = parts[0]
            .parse()
            .map_err(|_| "Invalid timestamp".to_string())?;
        let id = Uuid::parse_str(parts[1]).map_err(|_| "Invalid UUID".to_string())?;

        let timestamp = Utc
            .timestamp_micros(ts_micros)
            .single()
            .ok_or("Invalid timestamp value".to_string())?;

        Ok((timestamp, id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::FixedOffset;
    use sea_orm::{DbBackend, MockDatabase};

    #[test]
    fn test_cursor_encoding_decoding() {
        let now = Utc::now();
        let id = Uuid::new_v4();

        let cursor = GameService::encode_cursor(now, id);
        let (decoded_ts, decoded_id) =
            GameService::decode_cursor(&cursor).expect("Decoding failed");

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

        let _result = GameService::list_games(&db, None, 10, Some(player_id), None).await;

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

        let _result = GameService::list_games(&db, Some(cursor), 10, None, None).await;

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
