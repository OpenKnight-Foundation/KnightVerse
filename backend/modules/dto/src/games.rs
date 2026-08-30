use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use validator::{Validate, ValidationError};

// Define a regex for validating chess moves in algebraic notation
static CHESS_MOVE_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^[a-h][1-8][a-h][1-8][qrbnQRBN]?$").unwrap());

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub enum PlayerColor {
    #[serde(rename = "white")]
    White,
    #[serde(rename = "black")]
    Black,
    #[serde(rename = "random")]
    Random,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub enum GameStatus {
    #[serde(rename = "waiting")]
    Waiting,
    #[serde(rename = "in_progress")]
    InProgress,
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "aborted")]
    Aborted,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub enum GameResult {
    #[serde(rename = "white_win")]
    WhiteWin,
    #[serde(rename = "black_win")]
    BlackWin,
    #[serde(rename = "draw")]
    Draw,
    #[serde(rename = "in_progress")]
    InProgress,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, Validate)]
pub struct CreateGameRequest {
    #[validate(range(
        min = 60,
        max = 7200,
        message = "Time control must be between 1 minute and 2 hours"
    ))]
    pub time_control: i32,

    #[validate(range(
        min = 0,
        max = 60,
        message = "Increment must be between 0 and 60 seconds"
    ))]
    pub increment: i32,

    pub player_color: Option<PlayerColor>,
    pub opponent_id: Option<Uuid>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct GameDisplayDTO {
    #[schema(value_type = String, format = "uuid", example = "123e4567-e89b-12d3-a456-426614174000")]
    pub id: Uuid,

    #[schema(value_type = String, format = "uuid", example = "123e4567-e89b-12d3-a456-426614174001")]
    pub white_player_id: Uuid,

    #[schema(value_type = Option<String>, format = "uuid", example = "123e4567-e89b-12d3-a456-426614174002")]
    pub black_player_id: Option<Uuid>,

    pub status: GameStatus,
    pub result: GameResult,

    #[schema(example = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1")]
    pub current_fen: String,

    pub move_history: Vec<String>,
    pub time_control: i32,
    pub increment: i32,
    pub white_time_remaining: i32,
    pub black_time_remaining: i32,

    #[schema(value_type = String, format = "date-time")]
    pub created_at: DateTime<Utc>,

    #[schema(value_type = Option<String>, format = "date-time")]
    pub started_at: Option<DateTime<Utc>>,

    #[schema(value_type = String, format = "date-time")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, Validate)]
pub struct MakeMoveRequest {
    #[validate(regex(
        path = "CHESS_MOVE_REGEX",
        message = "Move must be in valid algebraic notation (e.g., 'e2e4', 'g7g8q')"
    ))]
    #[schema(example = "e2e4")]
    pub chess_move: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, Validate)]
pub struct JoinGameRequest {
    // player_id is now resolved exclusively from the validated JWT.
    // The struct is kept (rather than removed) so the OpenAPI schema and
    // route signature remain stable; clients may send an empty body `{}`.
}

// UUID validation function
pub fn validate_uuid(uuid: &Uuid) -> Result<(), ValidationError> {
    if uuid.is_nil() {
        return Err(ValidationError::new("Nil UUID is not allowed"));
    }
    Ok(())
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct ExportPgnQuery {
    #[schema(example = false)]
    pub include_analysis: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct ListGamesQuery {
    #[schema(example = "waiting")]
    pub status: Option<String>,

    #[schema(value_type = Option<String>, format = "uuid", example = "123e4567-e89b-12d3-a456-426614174000")]
    pub player_id: Option<Uuid>,

    #[schema(default = 1, example = 1)]
    /// Deprecated: Use cursor-based pagination
    pub page: Option<i32>,

    #[schema(default = 10, example = 10)]
    pub limit: Option<u64>,

    #[schema(
        example = "MjAyNS0wNS0zMVQxMDowMDowMC4wMDAwMDBaLDEyM2U0NTY3LWU4OWItMTJkMy1hNDU2LTQyNjYxNDE3NDAwMA=="
    )]
    pub cursor: Option<String>,
}

/// Request body for importing a game from PGN format
#[derive(Debug, Serialize, Deserialize, ToSchema, Validate)]
pub struct ImportGameRequest {
    #[validate(length(
        min = 10,
        max = 50000,
        message = "PGN must be between 10 and 50000 characters"
    ))]
    #[schema(
        example = "[White \"Magnus Carlsen\"]\n[Black \"Hikaru Nakamura\"]\n[Result \"1-0\"]\n\n1. e4 e5 2. Nf3 Nc6 3. Bb5 1-0"
    )]
    pub pgn: String,
}

/// Response for a successful game import
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ImportGameResponse {
    pub success: bool,

    #[schema(value_type = Option<String>, format = "uuid")]
    pub game_id: Option<Uuid>,

    #[schema(example = "Magnus Carlsen")]
    pub white_player: String,

    #[schema(example = "Hikaru Nakamura")]
    pub black_player: String,

    #[schema(example = "white_win")]
    pub result: String,

    #[schema(example = 42)]
    pub move_count: usize,

    #[schema(example = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1")]
    pub final_fen: Option<String>,

    pub error: Option<String>,
}

/// Request body for completing a game and updating player ratings
#[derive(Debug, Serialize, Deserialize, ToSchema, Validate)]
pub struct CompleteGameRequest {
    #[schema(example = "white_wins")]
    pub result: String,

    #[validate(range(min = 16, max = 64, message = "K-factor must be between 16 and 64"))]
    #[schema(default = 32, example = 32)]
    pub k_factor: Option<u32>,
}

/// Response for game completion with updated ratings
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CompleteGameResponse {
    pub success: bool,

    #[schema(value_type = String, format = "uuid")]
    pub game_id: Uuid,

    #[schema(example = "white_wins")]
    pub result: String,

    #[schema(example = 1532)]
    pub white_new_rating: i32,

    #[schema(example = 1468)]
    pub black_new_rating: i32,

    #[schema(example = 16)]
    pub rating_change_white: i32,

    #[schema(example = -16)]
    pub rating_change_black: i32,

    pub error: Option<String>,
}
