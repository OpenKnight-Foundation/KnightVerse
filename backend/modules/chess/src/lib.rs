pub mod bitboard;
pub mod chess960;
pub mod fen_validator;
pub mod mandatory_draw;
pub mod pgn;
pub mod pgn_validator;
pub mod rating;
pub mod start_position;
pub mod time_control;

pub use chess960::{Chess960Generator, Chess960Library, Chess960Position, LibraryMetadata};
pub use fen_validator::{validate_fen_legality, FenValidationError};
pub use mandatory_draw::{
    check_mandatory_draw_conditions, update_position_tracker, MandatoryDrawResult, PositionTracker,
};
pub use pgn::{
    export_pgn, export_pgn_validated, parse_pgn, validate_game, ExportHeaders,
    GameResult as PgnGameResult, MoveAnnotation, ParsedGame, PgnBuilder, PgnError, PgnHeaders,
    ValidatedGame,
};
pub use pgn_validator::{
    is_valid_pgn_export, lint_pgn_export, validate_pgn_export, PgnValidationError,
    PgnValidationReport, ValidatedPgnExport, SEVEN_TAG_ROSTER,
};
pub use rating::{
    GameOutcome, RatingConfig, RatingService, K_FACTOR_BLITZ, K_FACTOR_BULLET, K_FACTOR_CLASSICAL,
    K_FACTOR_RAPID,
};
pub use start_position::{generate_starting_fen, GameVariant, STANDARD_START_FEN};
pub use time_control::{PlayerClock, TimeControl, TimeControlCategory};
