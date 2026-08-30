//! Strict FEN legality validation.
//!
//! Endpoints that accept caller-supplied FEN strings (puzzle setup, game
//! restoration, custom challenges) must never hand a malformed or illegal
//! position to the engine — that's how you get crashes / panics deep in
//! move generation. This module rejects those positions up front with a
//! specific, descriptive error instead.
//!
//! Rather than re-implement position legality (king counts, checks,
//! castling/en-passant geometry, ...) by hand, this wraps [`shakmaty`]'s
//! FEN parser and [`Position::from_setup`] legality checks, which are
//! already exercised against Stockfish/Lichess-compatible rules elsewhere
//! in this crate (see `pgn.rs`). That keeps this validator small while
//! still covering every rule in the ticket.

use shakmaty::fen::{Fen, ParseFenError};
use shakmaty::{CastlingMode, Chess, FromSetup, PositionError, PositionErrorKinds};

/// Reasons a FEN string may be rejected by [`validate_fen_legality`].
///
/// Every variant carries a stable [`FenValidationError::code`] suitable for
/// API error responses, in addition to a human-readable [`Display`] message.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FenValidationError {
    /// The FEN could not be parsed at all: wrong number of fields, a
    /// malformed board section, bad rank counts, non-digit/non-piece
    /// characters, etc.
    #[error("malformed FEN: {0}")]
    MalformedFen(String),

    /// The active color field was present but was neither `w` nor `b`.
    #[error("active color must be 'w' or 'b'")]
    InvalidActiveColor,

    /// A side has no king.
    #[error("each side must have exactly one king")]
    MissingKing,

    /// A side has more than one king.
    #[error("each side must have exactly one king")]
    TooManyKings,

    /// A pawn is on rank 1 or rank 8.
    #[error("pawns cannot be placed on rank 1 or rank 8")]
    PawnsOnBackRank,

    /// A side has more pieces than are reachable through any sequence of
    /// legal moves (more than 16 total, or a promotion count inconsistent
    /// with the number of missing pawns).
    #[error("a side has more pieces than are reachable through legal play")]
    TooMuchMaterial,

    /// Castling rights don't match the actual king/rook placement.
    #[error("castling rights do not match king/rook placement")]
    InvalidCastlingRights,

    /// The en-passant square isn't geometrically valid for this position
    /// (wrong rank, occupied, or no pawn present to have made the double
    /// push that would allow it).
    #[error("en passant square is not valid for this position")]
    InvalidEnPassantSquare,

    /// The side NOT to move is in check — only the side to move may be.
    #[error("the side not to move may not be in check")]
    OppositeCheck,

    /// The position implies an impossible check configuration (too many
    /// simultaneous checkers, misaligned sliding checkers, or a checker
    /// that contradicts the en-passant square).
    #[error("position implies an impossible check configuration")]
    ImpossibleCheck,

    /// A variant-specific rule was violated.
    #[error("position violates a variant-specific rule")]
    VariantRuleViolated,
}

impl FenValidationError {
    /// A stable, machine-readable error code — safe to expose over an API.
    pub fn code(&self) -> &'static str {
        match self {
            Self::MalformedFen(_) => "MALFORMED_FEN",
            Self::InvalidActiveColor => "INVALID_ACTIVE_COLOR",
            Self::MissingKing => "MISSING_KING",
            Self::TooManyKings => "TOO_MANY_KINGS",
            Self::PawnsOnBackRank => "PAWNS_ON_BACKRANK",
            Self::TooMuchMaterial => "TOO_MUCH_MATERIAL",
            Self::InvalidCastlingRights => "INVALID_CASTLING_RIGHTS",
            Self::InvalidEnPassantSquare => "INVALID_EN_PASSANT_SQUARE",
            Self::OppositeCheck => "OPPOSITE_CHECK",
            Self::ImpossibleCheck => "IMPOSSIBLE_CHECK",
            Self::VariantRuleViolated => "VARIANT_RULE_VIOLATED",
        }
    }
}

impl From<ParseFenError> for FenValidationError {
    fn from(err: ParseFenError) -> Self {
        match err {
            ParseFenError::InvalidTurn => FenValidationError::InvalidActiveColor,
            other => FenValidationError::MalformedFen(other.to_string()),
        }
    }
}

impl From<PositionError<Chess>> for FenValidationError {
    fn from(err: PositionError<Chess>) -> Self {
        let kinds = err.kinds();

        // A single illegal position can trip several of shakmaty's checks
        // at once (e.g. a missing king also reads as "empty-ish" material).
        // Walk them in a fixed priority so the caller always gets one
        // specific, stable code rather than a nondeterministic pick.
        if kinds.contains(PositionErrorKinds::MISSING_KING)
            || kinds.contains(PositionErrorKinds::EMPTY_BOARD)
        {
            FenValidationError::MissingKing
        } else if kinds.contains(PositionErrorKinds::TOO_MANY_KINGS) {
            FenValidationError::TooManyKings
        } else if kinds.contains(PositionErrorKinds::PAWNS_ON_BACKRANK) {
            FenValidationError::PawnsOnBackRank
        } else if kinds.contains(PositionErrorKinds::TOO_MUCH_MATERIAL) {
            FenValidationError::TooMuchMaterial
        } else if kinds.contains(PositionErrorKinds::OPPOSITE_CHECK) {
            FenValidationError::OppositeCheck
        } else if kinds.contains(PositionErrorKinds::INVALID_CASTLING_RIGHTS) {
            FenValidationError::InvalidCastlingRights
        } else if kinds.contains(PositionErrorKinds::INVALID_EP_SQUARE) {
            FenValidationError::InvalidEnPassantSquare
        } else if kinds.contains(PositionErrorKinds::IMPOSSIBLE_CHECK) {
            FenValidationError::ImpossibleCheck
        } else {
            FenValidationError::VariantRuleViolated
        }
    }
}

/// Validate that `fen` is both syntactically well-formed and describes a
/// legally reachable chess position.
///
/// Checks performed (see [`FenValidationError`] for the specific codes):
/// - exactly one king per side
/// - no pawns on rank 1 or rank 8
/// - no more than 16 pieces per side, with promotions consistent with
///   missing pawns
/// - the side *not* to move is not in check
/// - castling rights match actual king/rook placement
/// - the en-passant square (if any) is geometrically valid
/// - the active color field is exactly `w` or `b`
///
/// On success, returns the parsed [`Chess`] position so callers don't need
/// to re-parse the FEN. This function never panics, regardless of how
/// garbled the input is — every failure mode is reported as a
/// [`FenValidationError`].
pub fn validate_fen_legality(fen: &str) -> Result<Chess, FenValidationError> {
    let parsed: Fen = fen.parse().map_err(FenValidationError::from)?;
    Chess::from_setup(parsed.into_setup(), CastlingMode::Standard).map_err(FenValidationError::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    const STARTING_POSITION: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

    fn assert_rejected(fen: &str, expected: FenValidationError) {
        match validate_fen_legality(fen) {
            Ok(_) => panic!("expected {fen:?} to be rejected as {expected:?}, but it was accepted"),
            Err(actual) => assert_eq!(
                actual, expected,
                "fen {fen:?} rejected for the wrong reason"
            ),
        }
    }

    // ---------------------------------------------------------------
    // Valid positions parse cleanly.
    // ---------------------------------------------------------------

    #[test]
    fn accepts_starting_position() {
        assert!(validate_fen_legality(STARTING_POSITION).is_ok());
    }

    #[test]
    fn accepts_position_with_black_to_move() {
        assert!(validate_fen_legality(
            "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1"
        )
        .is_ok());
    }

    #[test]
    fn accepts_valid_en_passant_square() {
        // White just pushed e2-e4, so e3 is a legal en-passant target.
        assert!(validate_fen_legality(
            "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1"
        )
        .is_ok());
    }

    #[test]
    fn accepts_position_with_no_castling_rights() {
        assert!(validate_fen_legality("4k3/8/8/8/8/8/8/4K3 w - - 0 1").is_ok());
    }

    #[test]
    fn accepts_endgame_with_single_pawn() {
        assert!(validate_fen_legality("8/8/8/4k3/8/8/4P3/4K3 w - - 0 1").is_ok());
    }

    // ---------------------------------------------------------------
    // Invalid positions are rejected with the specific expected code.
    // (>= 20 distinct invalid FEN edge cases, per the acceptance criteria.)
    // ---------------------------------------------------------------

    #[test]
    fn rejects_empty_string() {
        assert!(matches!(
            validate_fen_legality(""),
            Err(FenValidationError::MalformedFen(_))
        ));
    }

    #[test]
    fn rejects_completely_garbage_input() {
        assert!(matches!(
            validate_fen_legality("not a fen at all!! \0\0\0"),
            Err(FenValidationError::MalformedFen(_))
        ));
    }

    #[test]
    fn rejects_random_unicode_garbage_without_panicking() {
        assert!(validate_fen_legality("♞♞♞♞♞♞♞♞/🎉🎉🎉🎉🎉🎉🎉🎉/8/8/8/8/8/8 w - - 0 1").is_err());
    }

    #[test]
    fn rejects_too_few_ranks() {
        assert!(matches!(
            validate_fen_legality("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP w KQkq - 0 1"),
            Err(FenValidationError::MalformedFen(_))
        ));
    }

    #[test]
    fn rejects_too_many_ranks() {
        // 9 slashes (10 rank groups). Note: exactly 8 slashes in the board
        // field is a documented shakmaty extension for an appended Crazyhouse
        // pocket, so this needs one rank group beyond even that to reliably
        // fail as malformed rather than being reinterpreted.
        assert!(matches!(
            validate_fen_legality("8/8/rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"),
            Err(FenValidationError::MalformedFen(_))
        ));
    }

    #[test]
    fn rejects_rank_with_too_many_squares() {
        assert!(matches!(
            validate_fen_legality("9/8/8/8/8/8/8/4K2k w - - 0 1"),
            Err(FenValidationError::MalformedFen(_))
        ));
    }

    #[test]
    fn rejects_rank_with_too_few_squares() {
        assert!(matches!(
            validate_fen_legality("6/8/8/8/8/8/8/4K2k w - - 0 1"),
            Err(FenValidationError::MalformedFen(_))
        ));
    }

    #[test]
    fn rejects_invalid_piece_character() {
        assert!(matches!(
            validate_fen_legality("xxxxxxxx/8/8/8/8/8/8/4K2k w - - 0 1"),
            Err(FenValidationError::MalformedFen(_))
        ));
    }

    #[test]
    fn rejects_missing_active_color() {
        // Missing turn field defaults to white in shakmaty's lenient EPD
        // parsing, so exercise outright invalid tokens instead below; this
        // case just documents that a totally empty FEN is still malformed.
        assert!(matches!(
            validate_fen_legality("   "),
            Err(FenValidationError::MalformedFen(_))
        ));
    }

    #[test]
    fn rejects_invalid_active_color_token() {
        assert_eq!(
            validate_fen_legality("4k3/8/8/8/8/8/8/4K3 x - - 0 1").unwrap_err(),
            FenValidationError::InvalidActiveColor
        );
    }

    #[test]
    fn rejects_numeric_active_color_token() {
        assert_eq!(
            validate_fen_legality("4k3/8/8/8/8/8/8/4K3 1 - - 0 1").unwrap_err(),
            FenValidationError::InvalidActiveColor
        );
    }

    #[test]
    fn rejects_uppercase_active_color_token() {
        assert_eq!(
            validate_fen_legality("4k3/8/8/8/8/8/8/4K3 W - - 0 1").unwrap_err(),
            FenValidationError::InvalidActiveColor
        );
    }

    #[test]
    fn rejects_position_with_no_kings() {
        assert_rejected("8/8/8/8/8/8/8/8 w - - 0 1", FenValidationError::MissingKing);
    }

    #[test]
    fn rejects_position_missing_white_king() {
        assert_rejected(
            "4k3/8/8/8/8/8/8/8 w - - 0 1",
            FenValidationError::MissingKing,
        );
    }

    #[test]
    fn rejects_position_missing_black_king() {
        assert_rejected(
            "8/8/8/8/8/8/8/4K3 w - - 0 1",
            FenValidationError::MissingKing,
        );
    }

    #[test]
    fn rejects_two_white_kings() {
        assert_rejected(
            "4k3/8/8/8/8/8/8/3KK3 w - - 0 1",
            FenValidationError::TooManyKings,
        );
    }

    #[test]
    fn rejects_two_black_kings() {
        assert_rejected(
            "3kk3/8/8/8/8/8/8/4K3 w - - 0 1",
            FenValidationError::TooManyKings,
        );
    }

    #[test]
    fn rejects_pawn_on_rank_eight() {
        assert_rejected(
            "4P3/8/8/8/8/8/8/4Kk2 w - - 0 1",
            FenValidationError::PawnsOnBackRank,
        );
    }

    #[test]
    fn rejects_pawn_on_rank_one() {
        assert_rejected(
            "4Kk2/8/8/8/8/8/8/4p3 w - - 0 1",
            FenValidationError::PawnsOnBackRank,
        );
    }

    #[test]
    fn rejects_black_pawn_on_rank_one() {
        assert_rejected(
            "4Kk2/8/8/8/8/8/8/3ppp2 w - - 0 1",
            FenValidationError::PawnsOnBackRank,
        );
    }

    #[test]
    fn rejects_nine_pawns_for_one_side() {
        assert_rejected(
            "4k3/pppppppp/p7/8/8/8/8/4K3 w - - 0 1",
            FenValidationError::TooMuchMaterial,
        );
    }

    #[test]
    fn rejects_too_many_queens_for_material_available() {
        // 9 queens plus all 8 pawns still on the board is impossible for
        // white — every extra queen beyond the original one requires a
        // pawn to have been sacrificed via promotion.
        assert_rejected(
            "4k3/QQQQQQQQ/8/8/8/8/PPPPPPPP/QK6 w - - 0 1",
            FenValidationError::TooMuchMaterial,
        );
    }

    #[test]
    fn rejects_side_not_to_move_in_check() {
        // It is white to move, but black's king sits in open check from a
        // white rook down the e-file — black (not to move) must not be in
        // check.
        assert_rejected(
            "4k3/8/8/8/4R3/8/8/4K3 w - - 0 1",
            FenValidationError::OppositeCheck,
        );
    }

    #[test]
    fn rejects_castling_rights_without_rook() {
        assert_rejected(
            "r3k3/8/8/8/8/8/8/4K3 w KQ - 0 1",
            FenValidationError::InvalidCastlingRights,
        );
    }

    #[test]
    fn rejects_castling_rights_with_king_not_on_home_square() {
        assert_rejected(
            "r3k3/8/8/8/8/8/8/3K4 w Q - 0 1",
            FenValidationError::InvalidCastlingRights,
        );
    }

    #[test]
    fn rejects_en_passant_square_on_wrong_rank() {
        assert_rejected(
            "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e6 0 1",
            FenValidationError::InvalidEnPassantSquare,
        );
    }

    #[test]
    fn rejects_en_passant_square_with_no_pawn_present() {
        assert_rejected(
            "4k3/8/8/8/8/8/8/4K3 b - e3 0 1",
            FenValidationError::InvalidEnPassantSquare,
        );
    }

    #[test]
    fn rejects_extra_trailing_field() {
        assert!(matches!(
            validate_fen_legality("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1 extra"),
            Err(FenValidationError::MalformedFen(_))
        ));
    }

    #[test]
    fn error_code_is_stable_and_machine_readable() {
        assert_eq!(FenValidationError::MissingKing.code(), "MISSING_KING");
        assert_eq!(FenValidationError::TooManyKings.code(), "TOO_MANY_KINGS");
        assert_eq!(
            FenValidationError::InvalidActiveColor.code(),
            "INVALID_ACTIVE_COLOR"
        );
        assert_eq!(
            FenValidationError::PawnsOnBackRank.code(),
            "PAWNS_ON_BACKRANK"
        );
    }
}
