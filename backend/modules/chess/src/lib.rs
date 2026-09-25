pub mod bitboard;
pub mod fen_validator;
pub mod mandatory_draw;
pub mod pgn;
pub mod rating;
pub mod time_control;

pub use fen_validator::{validate_fen_legality, FenValidationError};
pub use mandatory_draw::{
    check_mandatory_draw_conditions, update_position_tracker, MandatoryDrawResult, PositionTracker,
};
pub use pgn::{
    parse_pgn, validate_game, GameResult as PgnGameResult, ParsedGame, PgnError, PgnHeaders,
    ValidatedGame,
};
pub use rating::{
    GameOutcome, RatingConfig, RatingService, K_FACTOR_BLITZ, K_FACTOR_BULLET, K_FACTOR_CLASSICAL,
    K_FACTOR_RAPID,
};
pub use time_control::{PlayerClock, TimeControl, TimeControlCategory};
