//! Game service — business logic layer for chess games.
//!
//! ## Read/Write pool routing
//!
//! All methods that only fetch data accept `db: &DatabaseConnection` so that
//! callers may supply either the replica or the primary.  Write/transactional
//! methods always take the primary explicitly.
//!
//! The public API surface uses [`db::DbPool`] as the entry-point; each method
//! routes internally:
//!
//! | Method            | Pool used   | Reason                          |
//! |-------------------|-------------|----------------------------------|
//! | `get_game`        | replica     | single-row read                 |
//! | `get_game_history`| replica     | analytic read (potentially heavy)|
//! | `list_games`      | replica     | pagination + count read         |
//! | `create_game`     | primary     | INSERT                          |
//! | `make_move`       | primary     | UPDATE                          |
//! | `join_game`       | primary     | UPDATE                          |
//! | `abandon_game`    | primary     | UPDATE                          |
//! | `import_game`     | primary     | INSERT                          |
//! | `complete_game`   | primary     | transaction (UPDATE × 2)        |

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use chess::pgn::ValidatedGame;
use chess::{RatingConfig, RatingService};
use chrono::{DateTime, TimeZone, Utc};
use db::DbPool;
use db_entity::{game, prelude::Game};
use dto::games::{
    CreateGameRequest, GameDisplayDTO, GameResult, GameStatus, MakeMoveRequest,
};
use error::error::ApiError;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DbErr, EntityTrait, Order, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect, Set, TransactionTrait,
};
use sea_orm::{Condition, DatabaseConnection};
use uuid::Uuid;

/// Starting FEN for a standard chess game.
const STARTING_FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

pub struct GameService;

/// Nil UUID used as a placeholder when a game is waiting for a second player.
const NIL_UUID: Uuid = Uuid::nil();

impl GameService {
    // =========================================================================
    // WRITE operations — always hit primary
    // =========================================================================

    /// Create a new game and persist it in the database.
    ///
    /// Routes to the **primary** pool (INSERT).
    pub async fn create_game(
        pool: &DbPool,
        creator_id: Uuid,
        request: CreateGameRequest,
    ) -> Result<GameDisplayDTO, ApiError> {
        Self::create_game_on(pool.primary(), creator_id, request).await
    }

    /// Record a move for the given game.
    ///
    /// Routes to the **primary** pool (UPDATE).
    pub async fn make_move(
        pool: &DbPool,
        game_id: Uuid,
        player_id: Uuid,
        move_request: MakeMoveRequest,
    ) -> Result<GameDisplayDTO, ApiError> {
        Self::make_move_on(pool.primary(), game_id, player_id, move_request).await
    }

    /// Join an existing game as the black player.
    ///
    /// Routes to the **primary** pool (UPDATE).
    pub async fn join_game(
        pool: &DbPool,
        game_id: Uuid,
        player_id: Uuid,
    ) -> Result<GameDisplayDTO, ApiError> {
        Self::join_game_on(pool.primary(), game_id, player_id).await
    }

    /// Abandon (forfeit) a game.
    ///
    /// Routes to the **primary** pool (UPDATE).
    pub async fn abandon_game(
        pool: &DbPool,
        game_id: Uuid,
        player_id: Uuid,
    ) -> Result<GameDisplayDTO, ApiError> {
        Self::abandon_game_on(pool.primary(), game_id, player_id).await
    }

    /// Import a previously played game from PGN.
    ///
    /// Routes to the **primary** pool (INSERT).
    pub async fn import_game(
        pool: &DbPool,
        importer_id: Uuid,
        request: &ValidatedGame,
    ) -> Result<Uuid, ApiError> {
        Self::import_game_on(pool.primary(), importer_id, request).await
    }

    /// Complete a game atomically: set result + update ratings.
    ///
    /// Routes to the **primary** pool (transaction).
    pub async fn complete_game(
        pool: &DbPool,
        game_id: Uuid,
        result: db_entity::game::ResultSide,
        rating_config: Option<RatingConfig>,
    ) -> Result<(i32, i32), ApiError> {
        Self::complete_game_on(pool.primary(), game_id, result, rating_config).await
    }

    // =========================================================================
    // READ operations — routed to replica (falls back to primary transparently)
    // =========================================================================

    /// Fetch a single game by its UUID.
    ///
    /// Routes to the **replica** pool (SELECT).
    pub async fn get_game(
        pool: &DbPool,
        game_id: Uuid,
    ) -> Result<GameDisplayDTO, ApiError> {
        Self::get_game_on(pool.replica(), game_id).await
    }

    /// Return a player's full game history (all finished games they participated in).
    ///
    /// This is a potentially large analytic read — routes to the **replica** pool.
    pub async fn get_game_history(
        pool: &DbPool,
        player_id: Uuid,
        limit: u64,
        offset: Option<u64>,
    ) -> Result<(Vec<game::Model>, u64), DbErr> {
        Self::get_game_history_on(pool.replica(), player_id, limit, offset).await
    }

    /// List games with keyset or offset pagination.
    ///
    /// Routes to the **replica** pool (SELECT + COUNT).
    pub async fn list_games(
        pool: &DbPool,
        cursor: Option<String>,
        offset: Option<u64>,
        limit: u64,
        player_id: Option<Uuid>,
        status: Option<GameStatus>,
    ) -> Result<(Vec<game::Model>, Option<String>, u64), DbErr> {
        Self::list_games_on(pool.replica(), cursor, offset, limit, player_id, status).await
    }

    /// Get a player's rating for a specific game (helper for rating calculations).
    ///
    /// Routes to the **replica** pool (SELECT).
    pub async fn get_player_rating_for_game(
        pool: &DbPool,
        game_id: Uuid,
        is_white: bool,
    ) -> Result<i32, ApiError> {
        Self::get_player_rating_for_game_on(pool.replica(), game_id, is_white).await
    }

    // =========================================================================
    // Low-level implementations that accept a raw `&DatabaseConnection`.
    //
    // These are `pub(crate)` so integration tests can target specific pools
    // directly without going through the `DbPool` facade.
    // =========================================================================

    pub(crate) async fn create_game_on(
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

    pub(crate) async fn get_game_on(
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

    pub(crate) async fn make_move_on(
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

        if model.black_player == NIL_UUID {
            return Err(ApiError::BadRequest(
                "Game has not started yet – waiting for opponent".to_string(),
            ));
        }

        if model.result.is_some()
            && model.result.as_ref() != Some(&db_entity::game::ResultSide::Ongoing)
        {
            return Err(ApiError::BadRequest(
                "Game is already completed".to_string(),
            ));
        }

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

        let mut new_moves = moves;
        new_moves.push(move_request.chess_move.clone());

        let mut active: game::ActiveModel = model.into();
        active.pgn = Set(serde_json::json!(new_moves));
        active.updated_at = Set(Utc::now().into());

        let updated = active.update(db).await.map_err(ApiError::from)?;
        Ok(Self::model_to_dto(updated))
    }

    pub(crate) async fn join_game_on(
        db: &DatabaseConnection,
        game_id: Uuid,
        player_id: Uuid,
    ) -> Result<GameDisplayDTO, ApiError> {
        let model = game::Entity::find_by_id(game_id)
            .one(db)
            .await
            .map_err(ApiError::from)?
            .ok_or_else(|| ApiError::NotFound("Game not found".to_string()))?;

        if model.result.is_some()
            && model.result.as_ref() != Some(&db_entity::game::ResultSide::Ongoing)
        {
            return Err(ApiError::BadRequest(
                "Game is already completed".to_string(),
            ));
        }

        if model.black_player != NIL_UUID {
            return Err(ApiError::BadRequest(
                "Game already has two players".to_string(),
            ));
        }

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

    pub(crate) async fn abandon_game_on(
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

    pub(crate) async fn import_game_on(
        db: &DatabaseConnection,
        _importer_id: Uuid,
        request: &ValidatedGame,
    ) -> Result<Uuid, ApiError> {
        let now = Utc::now();
        let game_id = Uuid::new_v4();

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
            white_player: Set(Uuid::nil()),
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

    pub(crate) async fn complete_game_on(
        db: &DatabaseConnection,
        game_id: Uuid,
        result: db_entity::game::ResultSide,
        rating_config: Option<RatingConfig>,
    ) -> Result<(i32, i32), ApiError> {
        let config = rating_config.unwrap_or_default();
        let txn = db.begin().await.map_err(ApiError::from)?;

        let game_model = game::Entity::find_by_id(game_id)
            .one(&txn)
            .await
            .map_err(ApiError::from)?
            .ok_or_else(|| ApiError::NotFound("Game not found".to_string()))?;

        if game_model.result.is_some() {
            let _ = txn.rollback().await;
            return Err(ApiError::BadRequest(
                "Game is already completed".to_string(),
            ));
        }

        let mut game_active_model: game::ActiveModel = game_model.into();
        game_active_model.result = Set(Some(result.clone()));
        game_active_model.updated_at = Set(Utc::now().into());
        game_active_model
            .update(&txn)
            .await
            .map_err(ApiError::from)?;

        let ratings_result =
            RatingService::update_ratings_in_transaction(&txn, game_id, &config).await;

        match ratings_result {
            Ok(ratings) => {
                txn.commit().await.map_err(ApiError::from)?;
                Ok(ratings)
            }
            Err(e) => {
                let _ = txn.rollback().await;
                Err(e)
            }
        }
    }

    pub(crate) async fn get_game_history_on(
        db: &DatabaseConnection,
        player_id: Uuid,
        limit: u64,
        offset: Option<u64>,
    ) -> Result<(Vec<game::Model>, u64), DbErr> {
        let condition = Condition::any()
            .add(game::Column::WhitePlayer.eq(player_id))
            .add(game::Column::BlackPlayer.eq(player_id));

        let total = Game::find().filter(condition.clone()).count(db).await?;

        let mut query = Game::find()
            .filter(condition)
            .order_by(game::Column::UpdatedAt, Order::Desc)
            .limit(limit);

        if let Some(off) = offset {
            query = query.offset(off);
        }

        let games = query.all(db).await?;
        Ok((games, total))
    }

    pub(crate) async fn get_player_rating_for_game_on(
        db: &DatabaseConnection,
        game_id: Uuid,
        is_white: bool,
    ) -> Result<i32, ApiError> {
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

        chess::RatingService::get_player_rating(db, player_id).await
    }

    pub(crate) async fn list_games_on(
        db: &DatabaseConnection,
        cursor: Option<String>,
        offset: Option<u64>,
        limit: u64,
        player_id: Option<Uuid>,
        status: Option<GameStatus>,
    ) -> Result<(Vec<game::Model>, Option<String>, u64), DbErr> {
        let filter_condition = Self::build_filter_condition(player_id, &status);

        let mut query = Game::find();
        if let Some(ref cond) = filter_condition {
            query = query.filter(cond.clone());
        }

        query = query
            .order_by(game::Column::CreatedAt, Order::Desc)
            .order_by(game::Column::Id, Order::Desc);

        if let Some(cursor_str) = cursor {
            if let Ok((last_created_at, last_id)) = Self::decode_cursor(&cursor_str) {
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

        let mut count_query = Game::find();
        if let Some(ref cond) = filter_condition {
            count_query = count_query.filter(cond.clone());
        }
        let total_count = count_query.count(db).await?;

        if let Some(off) = offset {
            query = query.offset(off);
        }

        let results = query.limit(limit + 1).all(db).await?;

        let mut games = results;
        let mut next_cursor: Option<String> = None;

        if games.len() as u64 > limit {
            games.truncate(limit as usize);
            if let Some(last_game) = games.last() {
                next_cursor = Some(Self::encode_cursor(
                    last_game.created_at.into(),
                    last_game.id,
                ));
            }
        }

        Ok((games, next_cursor, total_count))
    }

    // =========================================================================
    // Shared helpers
    // =========================================================================

    fn build_filter_condition(
        player_id: Option<Uuid>,
        status: &Option<GameStatus>,
    ) -> Option<Condition> {
        let mut conditions: Vec<Condition> = Vec::new();

        if let Some(pid) = player_id {
            let player_cond = Condition::any()
                .add(game::Column::WhitePlayer.eq(pid))
                .add(game::Column::BlackPlayer.eq(pid));
            conditions.push(player_cond);
        }

        if let Some(s) = status.as_ref() {
            match s {
                GameStatus::Waiting | GameStatus::InProgress => {
                    conditions.push(Condition::all().add(game::Column::Result.is_null()));
                }
                GameStatus::Completed | GameStatus::Aborted => {
                    conditions.push(Condition::all().add(game::Column::Result.is_not_null()));
                }
            }
        }

        if conditions.is_empty() {
            None
        } else {
            Some(conditions.into_iter().reduce(|acc, c| acc.add(c)).unwrap())
        }
    }

    fn encode_cursor(timestamp: DateTime<Utc>, id: Uuid) -> String {
        let ts_part = timestamp.timestamp_micros();
        let raw = format!("{},{}", ts_part, id);
        URL_SAFE_NO_PAD.encode(raw)
    }

    fn decode_cursor(cursor: &str) -> Result<(DateTime<Utc>, Uuid), String> {
        let decoded_bytes = URL_SAFE_NO_PAD
            .decode(cursor)
            .map_err(|_| "Invalid base64".to_string())?;
        let raw = String::from_utf8(decoded_bytes).map_err(|_| "Invalid utf8".to_string())?;

        let parts: Vec<&str> = raw.splitn(2, ',').collect();
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
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::FixedOffset;
    use db::db::db::DbPool;
    use sea_orm::{DbBackend, MockDatabase};

    // -------------------------------------------------------------------------
    // Unit tests (no real DB required)
    // -------------------------------------------------------------------------

    #[test]
    fn cursor_encode_decode_roundtrip() {
        let now = Utc::now();
        let id = Uuid::new_v4();

        let cursor = GameService::encode_cursor(now, id);
        let (decoded_ts, decoded_id) =
            GameService::decode_cursor(&cursor).expect("Decoding should not fail");

        // Microsecond precision roundtrip
        assert_eq!(decoded_ts.timestamp_micros(), now.timestamp_micros());
        assert_eq!(decoded_id, id);
    }

    #[test]
    fn decode_cursor_invalid_base64() {
        assert!(GameService::decode_cursor("!!!").is_err());
    }

    #[test]
    fn decode_cursor_missing_separator() {
        // Valid base64 but no comma inside
        let encoded = URL_SAFE_NO_PAD.encode("noseparatorhere");
        assert!(GameService::decode_cursor(&encoded).is_err());
    }

    // -------------------------------------------------------------------------
    // Pool-routing unit test: verifies that DbPool.primary() / .replica()
    // are wired correctly without a live database.
    // -------------------------------------------------------------------------

    /// Mock-based test that proves `list_games` issues its queries against the
    /// connection we supply (i.e. the replica in production).
    #[tokio::test]
    async fn test_list_games_query_structure() {
        // Create Mock Database to verify the generated SQL
        // We need two query result sets: one for count, one for the main query
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results(vec![
                // First query result (count) — empty set; typed so `T: IntoMockRow`
                // can be inferred. count() on no rows resolves to 0 and execution
                // continues to the data query below.
                Vec::<game::Model>::new(),
            ])
            .append_query_results(vec![
                // Second query result (main data)
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
        let result = GameService::list_games_on(
            &mock_db,
            None,
            None,
            10,
            Some(player_id),
            None
        ).await;
        
        // Get transaction log to verify SQL
        let transaction_log = db.into_transaction_log();
        
        // We expect two queries (count + data)
        assert_eq!(transaction_log.len(), 2);
        
        // Inspect the data query (index 1); index 0 is the COUNT query, which
        // carries neither the ORDER BY / LIMIT nor the keyset cursor predicate.
        let log = &transaction_log[1];
        let log_str = format!("{:?}", log);
        println!("Log: {}", log_str);

        let (games, _cursor, _total) = result;
        assert_eq!(games.len(), 1);

        // Inspect generated SQL to verify player filter and sort direction
        let log = mock_db.into_transaction_log();
        assert_eq!(log.len(), 2, "expected count + data queries");

        let count_sql = format!("{:?}", &log[0]);
        assert!(
            count_sql.contains(r#"\"game\".\"white_player\" = $1"#)
                || count_sql.contains("white_player"),
            "count query should filter by player"
        );

        let data_sql = format!("{:?}", &log[1]);
        assert!(
            data_sql.contains("DESC"),
            "data query should sort DESC for keyset pagination"
        );
    }

    /// Verifies that `create_game` issues an INSERT (write) by checking the
    /// transaction log of the supplied mock connection.
    #[tokio::test]
    async fn create_game_issues_insert() {
        let mock_game = make_mock_game();
        let creator_id = mock_game.white_player;
        let mock_game_clone = mock_game.clone();

        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results(vec![
                // First query result (count) — empty set; typed so `T: IntoMockRow`
                // can be inferred. count() on no rows resolves to 0 and execution
                // continues to the data query below.
                Vec::<game::Model>::new(),
            ])
            .append_query_results(vec![
                // Second query result (main data)
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
            }]])
            .into_connection();
            
        let _result = GameService::list_games(
            &db,
            Some(cursor),
            None,
            10,
            None,
            None
        ).await;
        
        let transaction_log = db.into_transaction_log();
        // Inspect the data query (index 1); index 0 is the COUNT query, which
        // carries neither the ORDER BY / LIMIT nor the keyset cursor predicate.
        let log = &transaction_log[1];
        let log_str = format!("{:?}", log);
        println!("Log with cursor: {}", log_str);

        let request = CreateGameRequest {
            time_control: 600,
            variant: None,
        };
        let _ = GameService::create_game_on(&mock_db, creator_id, request).await;

        let log = mock_db.into_transaction_log();
        assert!(!log.is_empty(), "at least one query should have been issued");
        let sql = format!("{:?}", &log[0]);
        assert!(
            sql.contains("INSERT") || sql.contains("insert"),
            "create_game should issue an INSERT, got: {}",
            sql
        );
    }

    // -------------------------------------------------------------------------
    // Pool-routing integration: DbPool routes reads to replica, writes to primary
    // -------------------------------------------------------------------------

    /// Constructs two separate mock connections and wraps them in a DbPool.
    /// Calls a read method and verifies the query hits the replica connection,
    /// then calls a write method and verifies it hits the primary.
    #[tokio::test]
    async fn pool_routes_reads_to_replica_and_writes_to_primary() {
        let game = make_mock_game();
        let game_id = game.id;

        // --- Replica mock: serves get_game (SELECT)
        let replica_mock = MockDatabase::new(DbBackend::Postgres)
            .append_query_results(vec![vec![game.clone()]])
            .into_connection();

        // --- Primary mock: serves create_game (INSERT)
        let primary_mock = MockDatabase::new(DbBackend::Postgres)
            .append_query_results(vec![vec![game.clone()]])
            .into_connection();

        // Wire them into a DbPool directly
        let pool = DbPool::from_connections(
            std::sync::Arc::new(primary_mock),
            std::sync::Arc::new(replica_mock),
            true,
        );

        // READ — should route to replica
        let _dto = GameService::get_game(&pool, game_id).await;

        // WRITE — should route to primary
        let request = CreateGameRequest {
            time_control: 300,
            variant: None,
        };
        let _create_result = GameService::create_game(&pool, game.white_player, request).await;

        // Inspect both pools' transaction logs
        let (primary_conn, replica_conn) = pool.into_connections();

        let replica_log = replica_conn.into_transaction_log();
        let primary_log = primary_conn.into_transaction_log();

        assert!(
            !replica_log.is_empty(),
            "replica should have received the get_game query"
        );
        assert!(
            !primary_log.is_empty(),
            "primary should have received the create_game query"
        );

        let replica_sql = format!("{:?}", &replica_log[0]);
        assert!(
            replica_sql.contains("SELECT") || replica_sql.contains("select"),
            "replica query should be a SELECT, got: {}",
            replica_sql
        );

        let primary_sql = format!("{:?}", &primary_log[0]);
        assert!(
            primary_sql.contains("INSERT") || primary_sql.contains("insert"),
            "primary query should be an INSERT, got: {}",
            primary_sql
        );
    }

    // -------------------------------------------------------------------------
    // Helpers
    // -------------------------------------------------------------------------

    fn make_mock_game() -> game::Model {
        let now = Utc::now().with_timezone(&FixedOffset::east_opt(0).unwrap());
        game::Model {
            id: Uuid::new_v4(),
            white_player: Uuid::new_v4(),
            black_player: Uuid::new_v4(),
            fen: STARTING_FEN.to_string(),
            pgn: serde_json::json!([]),
            result: None,
            variant: db_entity::game::GameVariant::Standard,
            started_at: now,
            duration_sec: 600,
            created_at: now,
            updated_at: now,
            is_imported: false,
            original_pgn: None,
        }
    }
}
