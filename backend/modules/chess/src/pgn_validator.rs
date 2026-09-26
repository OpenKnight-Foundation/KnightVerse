//! Strict, automated validation of exported PGN documents.
//!
//! `pgn.rs` knows how to *serialize* a game into PGN. This module is the
//! independent gate that checks the serialized document against the PGN
//! specification before it is handed to third-party tools such as ChessBase,
//! Lichess or SCID. It is deliberately strict — an export either passes every
//! sanity check or it is rejected:
//!
//! * the Seven Tag Roster (`Event`, `Site`, `Date`, `Round`, `White`,
//!   `Black`, `Result`) must be present, syntactically valid, and must be the
//!   first seven tags of the document, in the specified order;
//! * a blank line must separate the tag section from the movetext;
//! * movetext must use canonical SAN with strictly sequential move numbers
//!   and the correct `+` / `#` suffixes, recomputed by replaying the game;
//! * the movetext must end in exactly one termination marker, which must
//!   match `[Result "..."]`, and `[Termination "..."]` must be consistent
//!   with the result and the final position;
//! * `[%eval ...]` and `[%clk ...]` annotations must use the standard formats;
//! * Chess960 and custom starting positions must declare `SetUp "1"` plus a
//!   legal `FEN`, and are validated by replaying from that position.
//!
//! [`lint_pgn_export`] collects *every* problem it finds; [`validate_pgn_export`]
//! returns the first one as a `Result`. Neither function ever panics, no
//! matter how malformed the input is.

use crate::pgn::GameResult;
use regex::Regex;
use shakmaty::fen::Fen;
use shakmaty::san::San;
use shakmaty::{CastlingMode, Chess, Color, EnPassantMode, FromSetup, Move, Position};
use thiserror::Error;

/// The seven mandatory tags, in the order the PGN specification requires.
pub const SEVEN_TAG_ROSTER: [&str; 7] =
    ["Event", "Site", "Date", "Round", "White", "Black", "Result"];

/// Termination values understood by the validator (case-insensitive). The
/// canonical PGN values are listed alongside the descriptive endings this
/// service reports (resignation, timeout, stalemate, 50-move rule, ...).
const TERMINATION_VALUES: [&str; 18] = [
    "normal",
    "time forfeit",
    "timeout",
    "resignation",
    "stalemate",
    "50-move rule",
    "fifty-move rule",
    "abandoned",
    "adjudication",
    "death",
    "emergency",
    "rules infraction",
    "unterminated",
    "insufficient material",
    "agreement",
    "repetition",
    "threefold repetition",
    "?",
];

/// A single reason a PGN export was rejected.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PgnValidationError {
    #[error("empty PGN document")]
    EmptyDocument,

    #[error("missing blank line between the tag section and the movetext")]
    MissingBlankLine,

    #[error("invalid tag syntax: {0}")]
    InvalidTagSyntax(String),

    #[error("invalid escape sequence in tag value: {0}")]
    InvalidTagEscapeSequence(String),

    #[error("duplicate tag: {0}")]
    DuplicateTag(String),

    #[error("missing Seven Tag Roster tag: {0}")]
    MissingSevenTagRosterTag(String),

    #[error("Seven Tag Roster out of order: expected {expected}, found {found}")]
    SevenTagRosterOutOfOrder { expected: String, found: String },

    #[error("invalid Date tag value: {0}")]
    InvalidDate(String),

    #[error("invalid Result value: {0}")]
    InvalidResultValue(String),

    #[error("Result header \"{header}\" does not match movetext terminator \"{terminator}\"")]
    ResultHeaderMismatch { header: String, terminator: String },

    #[error("missing Termination tag")]
    MissingTerminationTag,

    #[error("invalid Termination value: {0}")]
    InvalidTerminationValue(String),

    #[error("invalid SetUp value: {0} (must be \"1\")")]
    InvalidSetupValue(String),

    #[error("SetUp tag requires a FEN tag")]
    SetupWithoutFen,

    #[error("FEN tag requires SetUp \"1\"")]
    FenWithoutSetup,

    #[error("invalid start FEN: {0}")]
    InvalidStartFen(String),

    #[error("variant \"{0}\" requires SetUp/FEN start position tags")]
    VariantWithoutStartPosition(String),

    #[error("unbalanced comment braces in movetext")]
    UnbalancedComment,

    #[error("malformed annotation comment: {0}")]
    MalformedAnnotation(String),

    #[error("malformed eval annotation: {0}")]
    MalformedEvalAnnotation(String),

    #[error("malformed clock annotation: {0}")]
    MalformedClockAnnotation(String),

    #[error("variations are not allowed in a canonical PGN export")]
    VariationsNotAllowed,

    #[error("move number out of sequence at ply {ply}: expected {expected}, found {found}")]
    MoveNumberOutOfSequence {
        ply: usize,
        expected: u32,
        found: u32,
    },

    #[error("invalid move-number format at ply {ply}: {token}")]
    InvalidMoveNumberFormat { ply: usize, token: String },

    #[error("missing move number before the white move at ply {ply}")]
    MissingMoveNumber { ply: usize },

    #[error("move number without a following move at ply {ply}")]
    MissingMoveAfterNumber { ply: usize },

    #[error("unexpected move-number token at ply {ply}")]
    UnexpectedMoveNumber { ply: usize },

    #[error("syntactically invalid SAN \"{token}\" at ply {ply}")]
    InvalidMoveSyntax { ply: usize, token: String },

    #[error("illegal move \"{token}\" at ply {ply}: {reason}")]
    IllegalMove {
        ply: usize,
        token: String,
        reason: String,
    },

    #[error("non-canonical SAN at ply {ply}: expected \"{expected}\", found \"{found}\"")]
    NonCanonicalSan {
        ply: usize,
        expected: String,
        found: String,
    },

    #[error("incorrect check/checkmate suffix at ply {ply}: expected \"{expected}\", found \"{found}\"")]
    IncorrectCheckSuffix {
        ply: usize,
        expected: String,
        found: String,
    },

    #[error("moves continue after the game is already over (ply {ply})")]
    MovesAfterGameOver { ply: usize },

    #[error("missing game termination marker")]
    MissingTerminationMarker,

    #[error("multiple termination markers: {0}")]
    MultipleTerminationMarkers(String),

    #[error("unexpected token after the termination marker: {0}")]
    TokenAfterTermination(String),

    #[error("Termination \"{termination}\" is inconsistent with result {result}")]
    TerminationResultMismatch { termination: String, result: String },

    #[error("Termination \"50-move rule\" requires at least 100 halfmoves, found {halfmoves}")]
    FiftyMoveRuleNotSatisfied { halfmoves: u32 },

    #[error("Termination \"Stalemate\" but the final position is not stalemate")]
    StalemateClaimButNotStalemate,

    #[error("result {result} is inconsistent with the final position ({position})")]
    ResultPositionMismatch { result: String, position: String },
}

impl PgnValidationError {
    /// A stable, machine-readable error code suitable for API responses.
    pub fn code(&self) -> &'static str {
        match self {
            Self::EmptyDocument => "PGN_EMPTY_DOCUMENT",
            Self::MissingBlankLine => "PGN_MISSING_BLANK_LINE",
            Self::InvalidTagSyntax(_) => "PGN_INVALID_TAG_SYNTAX",
            Self::InvalidTagEscapeSequence(_) => "PGN_INVALID_TAG_ESCAPE",
            Self::DuplicateTag(_) => "PGN_DUPLICATE_TAG",
            Self::MissingSevenTagRosterTag(_) => "PGN_MISSING_ROSTER_TAG",
            Self::SevenTagRosterOutOfOrder { .. } => "PGN_ROSTER_OUT_OF_ORDER",
            Self::InvalidDate(_) => "PGN_INVALID_DATE",
            Self::InvalidResultValue(_) => "PGN_INVALID_RESULT",
            Self::ResultHeaderMismatch { .. } => "PGN_RESULT_MISMATCH",
            Self::MissingTerminationTag => "PGN_MISSING_TERMINATION_TAG",
            Self::InvalidTerminationValue(_) => "PGN_INVALID_TERMINATION",
            Self::InvalidSetupValue(_) => "PGN_INVALID_SETUP",
            Self::SetupWithoutFen => "PGN_SETUP_WITHOUT_FEN",
            Self::FenWithoutSetup => "PGN_FEN_WITHOUT_SETUP",
            Self::InvalidStartFen(_) => "PGN_INVALID_START_FEN",
            Self::VariantWithoutStartPosition(_) => "PGN_VARIANT_WITHOUT_START",
            Self::UnbalancedComment => "PGN_UNBALANCED_COMMENT",
            Self::MalformedAnnotation(_) => "PGN_MALFORMED_ANNOTATION",
            Self::MalformedEvalAnnotation(_) => "PGN_MALFORMED_EVAL",
            Self::MalformedClockAnnotation(_) => "PGN_MALFORMED_CLOCK",
            Self::VariationsNotAllowed => "PGN_VARIATIONS_NOT_ALLOWED",
            Self::MoveNumberOutOfSequence { .. } => "PGN_MOVE_NUMBER_SEQUENCE",
            Self::InvalidMoveNumberFormat { .. } => "PGN_MOVE_NUMBER_FORMAT",
            Self::MissingMoveNumber { .. } => "PGN_MISSING_MOVE_NUMBER",
            Self::MissingMoveAfterNumber { .. } => "PGN_MISSING_MOVE_AFTER_NUMBER",
            Self::UnexpectedMoveNumber { .. } => "PGN_UNEXPECTED_MOVE_NUMBER",
            Self::InvalidMoveSyntax { .. } => "PGN_INVALID_MOVE_SYNTAX",
            Self::IllegalMove { .. } => "PGN_ILLEGAL_MOVE",
            Self::NonCanonicalSan { .. } => "PGN_NON_CANONICAL_SAN",
            Self::IncorrectCheckSuffix { .. } => "PGN_INCORRECT_CHECK_SUFFIX",
            Self::MovesAfterGameOver { .. } => "PGN_MOVES_AFTER_GAME_OVER",
            Self::MissingTerminationMarker => "PGN_MISSING_TERMINATION_MARKER",
            Self::MultipleTerminationMarkers(_) => "PGN_MULTIPLE_TERMINATION_MARKERS",
            Self::TokenAfterTermination(_) => "PGN_TOKEN_AFTER_TERMINATION",
            Self::TerminationResultMismatch { .. } => "PGN_TERMINATION_RESULT_MISMATCH",
            Self::FiftyMoveRuleNotSatisfied { .. } => "PGN_FIFTY_MOVE_NOT_SATISFIED",
            Self::StalemateClaimButNotStalemate => "PGN_STALEMATE_CLAIM_INVALID",
            Self::ResultPositionMismatch { .. } => "PGN_RESULT_POSITION_MISMATCH",
        }
    }
}

/// A PGN export that passed every strict check.
#[derive(Debug, Clone)]
pub struct ValidatedPgnExport {
    /// Tag name/value pairs, in document order.
    pub tags: Vec<(String, String)>,
    /// Canonical SAN moves (suffixes included) as replayed by the validator.
    pub movetext: Vec<String>,
    pub result: GameResult,
    pub termination: String,
    pub final_fen: String,
    pub ply_count: usize,
    pub halfmoves: u32,
    pub variant: Option<String>,
    pub is_checkmate: bool,
    pub is_stalemate: bool,
}

/// The full result of linting a PGN export: every violation (if any) plus the
/// parsed form when the document is clean.
#[derive(Debug, Clone, Default)]
pub struct PgnValidationReport {
    pub errors: Vec<PgnValidationError>,
    pub validated: Option<ValidatedPgnExport>,
}

impl PgnValidationReport {
    /// `true` when the document satisfied every strict check.
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty() && self.validated.is_some()
    }
}

/// Lint a PGN export, collecting **all** violations.
pub fn lint_pgn_export(pgn: &str) -> PgnValidationReport {
    let normalized = pgn.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = normalized.split('\n').collect();

    let mut errors: Vec<PgnValidationError> = Vec::new();

    // ------------------------------------------------------------------
    // Split the tag section from the movetext.
    // ------------------------------------------------------------------
    let mut cursor = 0;
    while cursor < lines.len() && lines[cursor].trim().is_empty() {
        cursor += 1;
    }
    if cursor >= lines.len() {
        errors.push(PgnValidationError::EmptyDocument);
        return PgnValidationReport {
            errors,
            validated: None,
        };
    }

    let tag_start = cursor;
    while cursor < lines.len() && lines[cursor].trim_start().starts_with('[') {
        cursor += 1;
    }
    let tag_lines = &lines[tag_start..cursor];
    let has_tags = !tag_lines.is_empty();

    let movetext = if has_tags {
        let mut blank_seen = false;
        while cursor < lines.len() && lines[cursor].trim().is_empty() {
            cursor += 1;
            blank_seen = true;
        }
        if !blank_seen && cursor < lines.len() {
            errors.push(PgnValidationError::MissingBlankLine);
        }
        lines[cursor..].join("\n")
    } else {
        lines[cursor..].join("\n")
    };

    // ------------------------------------------------------------------
    // Tag section: syntax, duplicates, Seven Tag Roster presence + order.
    // ------------------------------------------------------------------
    let tags = parse_tag_lines(tag_lines, &mut errors);

    let names: Vec<String> = tags.iter().map(|(n, _)| n.clone()).collect();
    for required in SEVEN_TAG_ROSTER {
        if !names.iter().any(|n| n == required) {
            errors.push(PgnValidationError::MissingSevenTagRosterTag(
                required.to_string(),
            ));
        }
    }
    let roster_complete = SEVEN_TAG_ROSTER
        .iter()
        .all(|required| names.iter().any(|n| n.as_str() == *required));
    if roster_complete {
        let observed: Vec<String> = names
            .iter()
            .filter(|n| SEVEN_TAG_ROSTER.contains(&n.as_str()))
            .cloned()
            .collect();
        let expected: Vec<String> = SEVEN_TAG_ROSTER.iter().map(|s| s.to_string()).collect();
        let supplemental_before_roster_end = names
            .iter()
            .take(SEVEN_TAG_ROSTER.len())
            .any(|n| !SEVEN_TAG_ROSTER.contains(&n.as_str()));
        if observed != expected {
            let index = observed
                .iter()
                .zip(expected.iter())
                .position(|(a, b)| a != b)
                .unwrap_or(0);
            errors.push(PgnValidationError::SevenTagRosterOutOfOrder {
                expected: expected.get(index).cloned().unwrap_or_default(),
                found: observed.get(index).cloned().unwrap_or_default(),
            });
        } else if supplemental_before_roster_end {
            errors.push(PgnValidationError::SevenTagRosterOutOfOrder {
                expected: SEVEN_TAG_ROSTER[SEVEN_TAG_ROSTER.len() - 1].to_string(),
                found: names
                    .get(SEVEN_TAG_ROSTER.len() - 1)
                    .cloned()
                    .unwrap_or_default(),
            });
        }
    }

    if let Some(date) = get_tag(&tags, "Date") {
        if !is_valid_date(date) {
            errors.push(PgnValidationError::InvalidDate(date.to_string()));
        }
    }

    let result_from_header = match get_tag(&tags, "Result") {
        Some(value) => match GameResult::from_pgn_string(value) {
            Ok(result) => Some(result),
            Err(_) => {
                errors.push(PgnValidationError::InvalidResultValue(value.to_string()));
                None
            }
        },
        None => None,
    };

    // ------------------------------------------------------------------
    // Start position for Chess960 / custom variants.
    // ------------------------------------------------------------------
    let start = parse_start_position(&tags, &mut errors);
    if let Some(variant) = get_tag(&tags, "Variant") {
        let is_standard =
            variant.eq_ignore_ascii_case("standard") || variant.eq_ignore_ascii_case("chess");
        if !is_standard && start.is_none() {
            errors.push(PgnValidationError::VariantWithoutStartPosition(
                variant.to_string(),
            ));
        }
    }

    // ------------------------------------------------------------------
    // Movetext: move numbers, canonical SAN, check/checkmate suffixes.
    // ------------------------------------------------------------------
    let items = tokenize_movetext(&movetext, &mut errors);
    let mut position = start.unwrap_or_default();
    let mut fullmove = u32::from(position.fullmoves());
    let mut ply: usize = 0;
    let mut pending_number: Option<(u32, usize)> = None;
    let mut canonical_moves: Vec<String> = Vec::new();
    let mut result_token: Option<String> = None;

    let mut index = 0;
    while index < items.len() {
        match items[index].clone() {
            MovetextItem::Result(marker) => {
                if result_token.is_some() {
                    errors.push(PgnValidationError::MultipleTerminationMarkers(marker));
                } else {
                    result_token = Some(marker);
                }
                index += 1;
                if index < items.len() {
                    errors.push(PgnValidationError::TokenAfterTermination(
                        items[index].describe(),
                    ));
                    break;
                }
            }
            MovetextItem::Number { number, dots } => {
                if pending_number.is_some() {
                    errors.push(PgnValidationError::UnexpectedMoveNumber { ply });
                }
                pending_number = Some((number, dots));
                index += 1;
            }
            MovetextItem::San(token) => {
                let side = position.turn();

                match pending_number.take() {
                    Some((number, dots)) => {
                        if number != fullmove {
                            errors.push(PgnValidationError::MoveNumberOutOfSequence {
                                ply,
                                expected: fullmove,
                                found: number,
                            });
                        }
                        let expected_dots = if side == Color::White { 1 } else { 3 };
                        if dots != expected_dots {
                            errors.push(PgnValidationError::InvalidMoveNumberFormat {
                                ply,
                                token: format!("{}{}", number, ".".repeat(dots)),
                            });
                        }
                    }
                    None => {
                        if side == Color::White {
                            errors.push(PgnValidationError::MissingMoveNumber { ply });
                        }
                    }
                }

                if position.is_checkmate() || position.is_stalemate() {
                    errors.push(PgnValidationError::MovesAfterGameOver { ply });
                    break;
                }

                let (bare, suffix) = split_suffix(&token);
                if !is_san_syntax(bare) {
                    errors.push(PgnValidationError::InvalidMoveSyntax {
                        ply,
                        token: token.clone(),
                    });
                    index += 1;
                    continue;
                }
                let san: San = match bare.parse() {
                    Ok(san) => san,
                    Err(_) => {
                        errors.push(PgnValidationError::InvalidMoveSyntax {
                            ply,
                            token: token.clone(),
                        });
                        index += 1;
                        continue;
                    }
                };
                let chess_move: Move = match san.to_move(&position) {
                    Ok(mv) => mv,
                    Err(err) => {
                        errors.push(PgnValidationError::IllegalMove {
                            ply,
                            token: token.clone(),
                            reason: err.to_string(),
                        });
                        index += 1;
                        continue;
                    }
                };

                let expected_bare = San::from_move(&position, &chess_move).to_string();
                let next = match position.clone().play(&chess_move) {
                    Ok(next) => next,
                    Err(err) => {
                        errors.push(PgnValidationError::IllegalMove {
                            ply,
                            token: token.clone(),
                            reason: err.to_string(),
                        });
                        index += 1;
                        continue;
                    }
                };
                let expected_suffix = if next.is_checkmate() {
                    "#"
                } else if next.is_check() {
                    "+"
                } else {
                    ""
                };

                if bare != expected_bare.as_str() {
                    errors.push(PgnValidationError::NonCanonicalSan {
                        ply,
                        expected: expected_bare.clone(),
                        found: bare.to_string(),
                    });
                }
                if suffix != expected_suffix {
                    errors.push(PgnValidationError::IncorrectCheckSuffix {
                        ply,
                        expected: format!("{}{}", expected_bare, expected_suffix),
                        found: token.clone(),
                    });
                }

                canonical_moves.push(format!("{}{}", expected_bare, expected_suffix));
                position = next;
                if side == Color::Black {
                    fullmove += 1;
                }
                ply += 1;
                index += 1;
            }
        }
    }
    if pending_number.is_some() {
        errors.push(PgnValidationError::MissingMoveAfterNumber { ply });
    }

    // ------------------------------------------------------------------
    // Termination marker, result tag, and end-of-game consistency.
    // ------------------------------------------------------------------
    match &result_token {
        None => errors.push(PgnValidationError::MissingTerminationMarker),
        Some(marker) => {
            if let Some(header) = get_tag(&tags, "Result") {
                if header != marker.as_str() {
                    errors.push(PgnValidationError::ResultHeaderMismatch {
                        header: header.to_string(),
                        terminator: marker.clone(),
                    });
                }
            }
        }
    }

    if position.is_checkmate() {
        let winner = if position.turn() == Color::White {
            GameResult::BlackWins
        } else {
            GameResult::WhiteWins
        };
        if let Some(result) = &result_from_header {
            if *result != winner {
                errors.push(PgnValidationError::ResultPositionMismatch {
                    result: result.to_pgn_string().to_string(),
                    position: "checkmate".to_string(),
                });
            }
        }
    } else if position.is_stalemate() {
        if let Some(result) = &result_from_header {
            if *result != GameResult::Draw {
                errors.push(PgnValidationError::ResultPositionMismatch {
                    result: result.to_pgn_string().to_string(),
                    position: "stalemate".to_string(),
                });
            }
        }
    }

    let termination = get_tag(&tags, "Termination").map(|value| value.to_string());
    match &termination {
        None => errors.push(PgnValidationError::MissingTerminationTag),
        Some(value) => {
            let normalized = value.to_ascii_lowercase();
            if !TERMINATION_VALUES.contains(&normalized.as_str()) {
                errors.push(PgnValidationError::InvalidTerminationValue(value.clone()));
            }
            if let Some(result) = &result_from_header {
                let decisive = matches!(result, GameResult::WhiteWins | GameResult::BlackWins);
                let ongoing = matches!(result, GameResult::Ongoing);
                let mismatch = || PgnValidationError::TerminationResultMismatch {
                    termination: value.clone(),
                    result: result.to_pgn_string().to_string(),
                };
                match normalized.as_str() {
                    "resignation" | "time forfeit" | "timeout" => {
                        if !decisive {
                            errors.push(mismatch());
                        }
                    }
                    "stalemate" => {
                        if !matches!(result, GameResult::Draw) {
                            errors.push(mismatch());
                        } else if !position.is_stalemate() {
                            errors.push(PgnValidationError::StalemateClaimButNotStalemate);
                        }
                    }
                    "50-move rule" | "fifty-move rule" => {
                        if !matches!(result, GameResult::Draw) {
                            errors.push(mismatch());
                        } else if position.halfmoves() < 100 {
                            errors.push(PgnValidationError::FiftyMoveRuleNotSatisfied {
                                halfmoves: position.halfmoves(),
                            });
                        }
                    }
                    "unterminated" => {
                        if !ongoing {
                            errors.push(mismatch());
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    let variant = get_tag(&tags, "Variant").map(|value| value.to_string());
    let final_fen = Fen::from_position(position.clone(), EnPassantMode::Legal).to_string();
    let mut report = PgnValidationReport {
        errors,
        validated: None,
    };
    if report.errors.is_empty() {
        report.validated = Some(ValidatedPgnExport {
            tags,
            movetext: canonical_moves,
            result: result_from_header.clone().unwrap_or(GameResult::Ongoing),
            termination: termination.clone().unwrap_or_default(),
            final_fen,
            ply_count: ply,
            halfmoves: position.halfmoves(),
            variant,
            is_checkmate: position.is_checkmate(),
            is_stalemate: position.is_stalemate(),
        });
    }
    report
}

/// Validate a PGN export, returning the first violation if there is one.
pub fn validate_pgn_export(pgn: &str) -> Result<ValidatedPgnExport, PgnValidationError> {
    let report = lint_pgn_export(pgn);
    if let Some(validated) = report.validated {
        Ok(validated)
    } else {
        Err(report
            .errors
            .into_iter()
            .next()
            .unwrap_or(PgnValidationError::EmptyDocument))
    }
}

/// Convenience predicate: does `pgn` pass every strict check?
pub fn is_valid_pgn_export(pgn: &str) -> bool {
    lint_pgn_export(pgn).is_valid()
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// One meaningful piece of movetext (comments and NAGs are handled inline).
#[derive(Debug, Clone)]
enum MovetextItem {
    Number { number: u32, dots: usize },
    San(String),
    Result(String),
}

impl MovetextItem {
    fn describe(&self) -> String {
        match self {
            MovetextItem::Number { number, dots } => {
                format!("{}{}", number, ".".repeat(*dots))
            }
            MovetextItem::San(token) => token.clone(),
            MovetextItem::Result(marker) => marker.clone(),
        }
    }
}

fn get_tag<'a>(tags: &'a [(String, String)], name: &str) -> Option<&'a str> {
    tags.iter()
        .find(|(tag_name, _)| tag_name == name)
        .map(|(_, value)| value.as_str())
}

fn parse_tag_lines(
    lines: &[&str],
    errors: &mut Vec<PgnValidationError>,
) -> Vec<(String, String)> {
    let tag_re = Regex::new(r#"^\[([A-Za-z0-9_]+)\s+"((?:[^"\\]|\\.)*)"\]$"#).unwrap();
    let mut tags: Vec<(String, String)> = Vec::new();

    for raw_line in lines {
        let line = raw_line.trim_end();
        match tag_re.captures(line) {
            Some(captures) => {
                let name = captures[1].to_string();
                let raw_value = &captures[2];
                if !has_valid_escapes(raw_value) {
                    errors.push(PgnValidationError::InvalidTagEscapeSequence(
                        line.to_string(),
                    ));
                    continue;
                }
                if tags.iter().any(|(existing, _)| existing == &name) {
                    errors.push(PgnValidationError::DuplicateTag(name));
                    continue;
                }
                tags.push((name, unescape_tag_value(raw_value)));
            }
            None => errors.push(PgnValidationError::InvalidTagSyntax(line.to_string())),
        }
    }

    tags
}

fn has_valid_escapes(value: &str) -> bool {
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('"') | Some('\\') => {}
                _ => return false,
            }
        }
    }
    true
}

fn unescape_tag_value(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(next) = chars.next() {
                out.push(next);
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn is_valid_date(value: &str) -> bool {
    let date_re = Regex::new(r"^(\d{4}|\?{4})\.(\d{2}|\?{2})\.(\d{2}|\?{2})$").unwrap();
    let captures = match date_re.captures(value) {
        Some(captures) => captures,
        None => return false,
    };
    let month = &captures[2];
    if month != "??" {
        let parsed: u32 = month.parse().unwrap_or(0);
        if !(1..=12).contains(&parsed) {
            return false;
        }
    }
    let day = &captures[3];
    if day != "??" {
        let parsed: u32 = day.parse().unwrap_or(0);
        if !(1..=31).contains(&parsed) {
            return false;
        }
    }
    true
}

fn parse_start_position(
    tags: &[(String, String)],
    errors: &mut Vec<PgnValidationError>,
) -> Option<Chess> {
    let setup = get_tag(tags, "SetUp");
    let fen = get_tag(tags, "FEN");
    match (setup, fen) {
        (None, None) => None,
        (Some(_), None) => {
            errors.push(PgnValidationError::SetupWithoutFen);
            None
        }
        (None, Some(_)) => {
            errors.push(PgnValidationError::FenWithoutSetup);
            None
        }
        (Some(setup), Some(fen)) => {
            if setup != "1" {
                errors.push(PgnValidationError::InvalidSetupValue(setup.to_string()));
                return None;
            }
            match fen.parse::<Fen>() {
                Ok(parsed) => {
                    // Chess960 castling mode is the general form: it also
                    // accepts standard `KQkq` rights, so it covers Chess960
                    // and arbitrary custom start positions alike.
                    match Chess::from_setup(parsed.into_setup(), CastlingMode::Chess960) {
                        Ok(position) => Some(position),
                        Err(err) => {
                            errors.push(PgnValidationError::InvalidStartFen(err.to_string()));
                            None
                        }
                    }
                }
                Err(err) => {
                    errors.push(PgnValidationError::InvalidStartFen(err.to_string()));
                    None
                }
            }
        }
    }
}

fn split_suffix(token: &str) -> (&str, &str) {
    if let Some(rest) = token.strip_suffix('#') {
        (rest, "#")
    } else if let Some(rest) = token.strip_suffix('+') {
        (rest, "+")
    } else {
        (token, "")
    }
}

fn is_san_syntax(bare: &str) -> bool {
    let san_re =
        Regex::new(r"^(O-O(-O)?|0-0(-0)?|[KQRBN]?[a-h]?[1-8]?x?[a-h][1-8](=[QRBN])?)$").unwrap();
    san_re.is_match(bare)
}

fn tokenize_movetext(text: &str, errors: &mut Vec<PgnValidationError>) -> Vec<MovetextItem> {
    let number_re = Regex::new(r"^(\d+)\.{1,3}$").unwrap();
    let result_re = Regex::new(r"^(1-0|0-1|1/2-1/2|\*)$").unwrap();
    let classification_re = Regex::new(r"[!?]{1,2}$").unwrap();

    let chars: Vec<char> = text.chars().collect();
    let mut items: Vec<MovetextItem> = Vec::new();
    let mut variation_reported = false;
    let mut index = 0;

    while index < chars.len() {
        let current = chars[index];

        if current.is_whitespace() {
            index += 1;
            continue;
        }

        if current == '{' {
            let mut cursor = index + 1;
            let mut content = String::new();
            while cursor < chars.len() && chars[cursor] != '}' {
                content.push(chars[cursor]);
                cursor += 1;
            }
            if cursor >= chars.len() {
                errors.push(PgnValidationError::UnbalancedComment);
                break;
            }
            validate_comment(content.trim(), errors);
            index = cursor + 1;
            continue;
        }

        if current == '}' {
            errors.push(PgnValidationError::UnbalancedComment);
            index += 1;
            continue;
        }

        if current == '(' {
            if !variation_reported {
                errors.push(PgnValidationError::VariationsNotAllowed);
                variation_reported = true;
            }
            let mut depth = 1usize;
            let mut cursor = index + 1;
            while cursor < chars.len() && depth > 0 {
                match chars[cursor] {
                    '(' => depth += 1,
                    ')' => depth -= 1,
                    _ => {}
                }
                cursor += 1;
            }
            index = cursor;
            continue;
        }

        if current == ')' {
            errors.push(PgnValidationError::UnbalancedComment);
            index += 1;
            continue;
        }

        if current == ';' {
            while index < chars.len() && chars[index] != '\n' {
                index += 1;
            }
            continue;
        }

        if current == '$' {
            let mut cursor = index + 1;
            let mut digits = String::new();
            while cursor < chars.len() && chars[cursor].is_ascii_digit() {
                digits.push(chars[cursor]);
                cursor += 1;
            }
            if digits.is_empty() {
                errors.push(PgnValidationError::InvalidMoveSyntax {
                    ply: 0,
                    token: "$".to_string(),
                });
            }
            index = cursor;
            continue;
        }

        let mut cursor = index;
        let mut token = String::new();
        while cursor < chars.len() && !chars[cursor].is_whitespace() {
            token.push(chars[cursor]);
            cursor += 1;
        }
        index = cursor;

        let core = classification_re.replace(&token, "").into_owned();
        if core.is_empty() {
            continue;
        }
        if let Some(captures) = number_re.captures(&core) {
            let number = captures[1].parse::<u32>().unwrap_or(0);
            let dots = core.chars().filter(|c| *c == '.').count();
            items.push(MovetextItem::Number { number, dots });
        } else if result_re.is_match(&core) {
            items.push(MovetextItem::Result(core));
        } else {
            items.push(MovetextItem::San(core));
        }
    }

    items
}

fn validate_comment(content: &str, errors: &mut Vec<PgnValidationError>) {
    if content.is_empty() {
        return;
    }
    let annotation_re = Regex::new(r"\[%(\w+)([^\]]*)\]").unwrap();
    let eval_re = Regex::new(r"^\[%eval (#-?\d+|-?\d+(\.\d+)?)\]$").unwrap();
    let clock_re = Regex::new(r"^\[%clk \d+:\d{2}:\d{2}\]$").unwrap();

    let mut saw_annotation = false;
    for captures in annotation_re.captures_iter(content) {
        saw_annotation = true;
        let whole = captures.get(0).unwrap().as_str();
        match &captures[1] {
            "eval" => {
                if !eval_re.is_match(whole) {
                    errors.push(PgnValidationError::MalformedEvalAnnotation(
                        whole.to_string(),
                    ));
                }
            }
            "clk" => {
                if !clock_re.is_match(whole) {
                    errors.push(PgnValidationError::MalformedClockAnnotation(
                        whole.to_string(),
                    ));
                }
            }
            _ => {}
        }
    }

    if content.contains("[%") && !saw_annotation {
        errors.push(PgnValidationError::MalformedAnnotation(
            content.to_string(),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pgn::{export_pgn, ExportHeaders, MoveAnnotation};

    fn doc(result: &str, termination: &str, movetext: &str) -> String {
        format!(
            "[Event \"Test\"]\n[Site \"?\"]\n[Date \"2026.09.26\"]\n[Round \"1\"]\n\
             [White \"White\"]\n[Black \"Black\"]\n[Result \"{result}\"]\n\
             [Termination \"{termination}\"]\n\n{movetext}"
        )
    }

    fn errors_of(pgn: &str) -> Vec<PgnValidationError> {
        lint_pgn_export(pgn).errors
    }

    fn assert_ok(pgn: &str) {
        let report = lint_pgn_export(pgn);
        assert!(
            report.is_valid(),
            "expected a valid PGN export, got errors: {:?}",
            report.errors
        );
    }

    fn assert_error(pgn: &str, predicate: impl Fn(&PgnValidationError) -> bool, label: &str) {
        let errors = errors_of(pgn);
        assert!(
            errors.iter().any(|error| predicate(error)),
            "expected {label}, got errors: {errors:?}"
        );
    }

    fn headers(result: GameResult, termination: &str) -> ExportHeaders {
        ExportHeaders {
            event: "Test Event".to_string(),
            site: "test.example.com".to_string(),
            date: "2026.09.26".to_string(),
            round: "1".to_string(),
            white: "Alice".to_string(),
            black: "Bob".to_string(),
            result,
            termination: Some(termination.to_string()),
            ..Default::default()
        }
    }

    fn moves(sans: &[&str]) -> Vec<MoveAnnotation> {
        sans.iter()
            .map(|san| MoveAnnotation {
                san: (*san).to_string(),
                ..Default::default()
            })
            .collect()
    }

    // ---------------------------------------------------------------
    // Positive cases
    // ---------------------------------------------------------------

    #[test]
    fn accepts_minimal_valid_export() {
        assert_ok(&doc("*", "Unterminated", "1. e4 e5 2. Nf3 Nc6 *"));
    }

    #[test]
    fn accepts_check_and_checkmate_suffixes() {
        // Rossolimo check (not mate): "+" is required.
        assert_ok(&doc("*", "Unterminated", "1. e4 c5 2. Nf3 d6 3. Bb5+ *"));
        // Scholar's mate: "#" is required.
        assert_ok(&doc(
            "1-0",
            "Normal",
            "1. e4 e5 2. Bc4 Nc6 3. Qh5 Nf6 4. Qxf7# 1-0",
        ));
    }

    #[test]
    fn accepts_full_document_with_analysis_annotations() {
        let pgn = "[Event \"Rated Blitz Game\"]\n[Site \"chess.example.com\"]\n\
                   [Date \"2026.09.26\"]\n[Round \"1\"]\n[White \"Alice\"]\n[Black \"Bob\"]\n\
                   [Result \"1-0\"]\n[WhiteElo \"2839\"]\n[BlackElo \"2802\"]\n\
                   [TimeControl \"300+3\"]\n[Termination \"Resignation\"]\n\n\
                   1. e4 { [%eval 0.32] [%clk 0:04:58] } e5 { [%clk 0:04:57] } \
                   2. Nf3 { [%eval 0.28] } Nc6 3. Bb5 1-0";
        let validated = validate_pgn_export(pgn).expect("document should validate");
        assert_eq!(validated.ply_count, 5);
        assert_eq!(validated.result, GameResult::WhiteWins);
        assert_eq!(validated.termination, "Resignation");
    }

    #[test]
    fn accepts_builder_export_for_resignation() {
        let pgn = export_pgn(
            headers(GameResult::WhiteWins, "Resignation"),
            moves(&["e4", "e5", "Nf3", "Nc6", "Bb5"]),
            false,
        )
        .unwrap();
        let validated = validate_pgn_export(&pgn).expect("export should validate");
        assert_eq!(validated.ply_count, 5);
        assert_eq!(validated.result, GameResult::WhiteWins);
    }

    #[test]
    fn accepts_builder_export_for_timeout() {
        let pgn = export_pgn(
            headers(GameResult::BlackWins, "Time forfeit"),
            moves(&["e4", "e5", "Nf3", "Nc6", "Bb5"]),
            false,
        )
        .unwrap();
        let validated = validate_pgn_export(&pgn).expect("export should validate");
        assert_eq!(validated.result, GameResult::BlackWins);
        assert_eq!(validated.termination, "Time forfeit");
    }

    #[test]
    fn accepts_builder_export_for_stalemate() {
        let mut export_headers = headers(GameResult::Draw, "Stalemate");
        export_headers.start_fen = Some("k7/2K5/8/8/8/8/8/1Q6 w - - 0 1".to_string());
        let pgn = export_pgn(export_headers, moves(&["Qb6"]), false).unwrap();
        assert!(pgn.contains("[SetUp \"1\"]"));
        assert!(pgn.contains("[FEN \"k7/2K5/8/8/8/8/8/1Q6 w - - 0 1\"]"));

        let validated = validate_pgn_export(&pgn).expect("stalemate export should validate");
        assert!(validated.is_stalemate);
        assert_eq!(validated.result, GameResult::Draw);
    }

    #[test]
    fn accepts_builder_export_for_fifty_move_rule() {
        let mut export_headers = headers(GameResult::Draw, "50-move rule");
        export_headers.start_fen = Some("8/8/8/4k3/8/8/4P3/4K3 w - - 99 1".to_string());
        let pgn = export_pgn(export_headers, moves(&["Kd1"]), false).unwrap();

        let validated = validate_pgn_export(&pgn).expect("50-move export should validate");
        assert_eq!(validated.halfmoves, 100);
        assert_eq!(validated.result, GameResult::Draw);
    }

    #[test]
    fn accepts_builder_export_with_analysis_annotations() {
        let mut annotated = moves(&["e4", "e5", "Nf3"]);
        annotated[0].clock = Some("0:04:58".to_string());
        annotated[0].eval_centipawns = Some(32);
        annotated[1].eval_mate = Some(3);

        let pgn = export_pgn(
            headers(GameResult::Ongoing, "Unterminated"),
            annotated,
            true,
        )
        .unwrap();
        let validated = validate_pgn_export(&pgn).expect("annotated export should validate");
        assert_eq!(validated.ply_count, 3);
    }

    #[test]
    fn accepts_chess960_export() {
        let mut export_headers = headers(GameResult::Ongoing, "Unterminated");
        export_headers.start_fen =
            Some("rnkbbqnr/pppppppp/8/8/8/8/PPPPPPPP/RNKBBQNR w KQkq - 0 1".to_string());
        export_headers.variant = Some("Chess960".to_string());
        let pgn = export_pgn(export_headers, moves(&["Nc3"]), false).unwrap();
        assert!(pgn.contains("[Variant \"Chess960\"]"));

        let validated = validate_pgn_export(&pgn).expect("Chess960 export should validate");
        assert_eq!(validated.variant.as_deref(), Some("Chess960"));
        assert_eq!(validated.ply_count, 1);
    }

    #[test]
    fn accepts_custom_variant_export() {
        let mut export_headers = headers(GameResult::Ongoing, "Unterminated");
        export_headers.start_fen = Some("8/8/8/4k3/8/8/4P3/4K3 w - - 0 1".to_string());
        export_headers.variant = Some("Custom".to_string());
        let pgn = export_pgn(export_headers, moves(&["Kd1"]), false).unwrap();

        let validated = validate_pgn_export(&pgn).expect("custom export should validate");
        assert_eq!(validated.variant.as_deref(), Some("Custom"));
    }

    // ---------------------------------------------------------------
    // Negative cases: structure and tags
    // ---------------------------------------------------------------

    #[test]
    fn rejects_empty_document() {
        assert_error("", |error| {
            matches!(error, PgnValidationError::EmptyDocument)
        }, "EmptyDocument");
    }

    #[test]
    fn rejects_missing_blank_line() {
        let pgn = "[Event \"Test\"]\n[Site \"?\"]\n[Date \"2026.09.26\"]\n[Round \"1\"]\n\
                   [White \"White\"]\n[Black \"Black\"]\n[Result \"*\"]\n\
                   [Termination \"Unterminated\"]\n1. e4 *";
        assert_error(pgn, |error| {
            matches!(error, PgnValidationError::MissingBlankLine)
        }, "MissingBlankLine");
    }

    #[test]
    fn rejects_missing_seven_tag_roster_tag() {
        let pgn = "[Event \"Test\"]\n[Site \"?\"]\n[Round \"1\"]\n[White \"White\"]\n\
                   [Black \"Black\"]\n[Result \"*\"]\n[Termination \"Unterminated\"]\n\n1. e4 *";
        assert_error(pgn, |error| {
            matches!(error, PgnValidationError::MissingSevenTagRosterTag(tag) if tag == "Date")
        }, "MissingSevenTagRosterTag(Date)");
    }

    #[test]
    fn rejects_seven_tag_roster_out_of_order() {
        let pgn = "[Event \"Test\"]\n[Site \"?\"]\n[Date \"2026.09.26\"]\n[Round \"1\"]\n\
                   [Black \"Black\"]\n[White \"White\"]\n[Result \"*\"]\n\
                   [Termination \"Unterminated\"]\n\n1. e4 *";
        assert_error(pgn, |error| {
            matches!(error, PgnValidationError::SevenTagRosterOutOfOrder { .. })
        }, "SevenTagRosterOutOfOrder");
    }

    #[test]
    fn rejects_duplicate_tag() {
        let pgn = "[Event \"Test\"]\n[Event \"Again\"]\n[Site \"?\"]\n[Date \"2026.09.26\"]\n\
                   [Round \"1\"]\n[White \"White\"]\n[Black \"Black\"]\n[Result \"*\"]\n\
                   [Termination \"Unterminated\"]\n\n1. e4 *";
        assert_error(pgn, |error| {
            matches!(error, PgnValidationError::DuplicateTag(tag) if tag == "Event")
        }, "DuplicateTag(Event)");
    }

    #[test]
    fn rejects_missing_termination_tag() {
        let pgn = "[Event \"Test\"]\n[Site \"?\"]\n[Date \"2026.09.26\"]\n[Round \"1\"]\n\
                   [White \"White\"]\n[Black \"Black\"]\n[Result \"*\"]\n\n1. e4 *";
        assert_error(pgn, |error| {
            matches!(error, PgnValidationError::MissingTerminationTag)
        }, "MissingTerminationTag");
    }

    #[test]
    fn rejects_invalid_termination_value() {
        assert_error(&doc("*", "Something else", "1. e4 e5 *"), |error| {
            matches!(error, PgnValidationError::InvalidTerminationValue(_))
        }, "InvalidTerminationValue");
    }

    #[test]
    fn rejects_result_header_mismatch() {
        assert_error(&doc("1-0", "Normal", "1. e4 e5 2. Nf3 Nc6 *"), |error| {
            matches!(error, PgnValidationError::ResultHeaderMismatch { .. })
        }, "ResultHeaderMismatch");
    }

    #[test]
    fn rejects_setup_without_fen() {
        let pgn = "[Event \"Test\"]\n[Site \"?\"]\n[Date \"2026.09.26\"]\n[Round \"1\"]\n\
                   [White \"White\"]\n[Black \"Black\"]\n[Result \"*\"]\n[SetUp \"1\"]\n\
                   [Termination \"Unterminated\"]\n\n1. e4 *";
        assert_error(pgn, |error| {
            matches!(error, PgnValidationError::SetupWithoutFen)
        }, "SetupWithoutFen");
    }

    #[test]
    fn rejects_fen_without_setup() {
        let pgn = "[Event \"Test\"]\n[Site \"?\"]\n[Date \"2026.09.26\"]\n[Round \"1\"]\n\
                   [White \"White\"]\n[Black \"Black\"]\n[Result \"*\"]\n\
                   [FEN \"8/8/8/4k3/8/8/4P3/4K3 w - - 0 1\"]\n\
                   [Termination \"Unterminated\"]\n\n1. Kd1 *";
        assert_error(pgn, |error| {
            matches!(error, PgnValidationError::FenWithoutSetup)
        }, "FenWithoutSetup");
    }

    #[test]
    fn rejects_variant_without_start_position() {
        let mut export_headers = headers(GameResult::Ongoing, "Unterminated");
        export_headers.variant = Some("Chess960".to_string());
        let pgn = export_pgn(export_headers, vec![], false).unwrap();
        assert_error(&pgn, |error| {
            matches!(error, PgnValidationError::VariantWithoutStartPosition(variant) if variant == "Chess960")
        }, "VariantWithoutStartPosition");
    }

    // ---------------------------------------------------------------
    // Negative cases: movetext
    // ---------------------------------------------------------------

    #[test]
    fn rejects_malformed_move_syntax() {
        assert_error(&doc("*", "Unterminated", "1. e4 e5 2. Nf3 Zq9 *"), |error| {
            matches!(error, PgnValidationError::InvalidMoveSyntax { .. })
        }, "InvalidMoveSyntax");
    }

    #[test]
    fn rejects_illegal_move() {
        assert_error(&doc("*", "Unterminated", "1. e4 e5 2. Ke3 *"), |error| {
            matches!(error, PgnValidationError::IllegalMove { .. })
        }, "IllegalMove");
    }

    #[test]
    fn rejects_non_canonical_long_algebraic_san() {
        let errors = errors_of(&doc("*", "Unterminated", "1. e4 e5 2. Ng1f3 *"));
        assert!(
            errors.iter().any(|error| matches!(
                error,
                PgnValidationError::NonCanonicalSan { .. } | PgnValidationError::IllegalMove { .. }
            )),
            "expected long algebraic SAN to be rejected, got {errors:?}"
        );
    }

    #[test]
    fn rejects_missing_move_number() {
        assert_error(&doc("*", "Unterminated", "e4 e5 *"), |error| {
            matches!(error, PgnValidationError::MissingMoveNumber { .. })
        }, "MissingMoveNumber");
    }

    #[test]
    fn rejects_move_number_out_of_sequence() {
        assert_error(&doc("*", "Unterminated", "1. e4 e5 3. Nf3 *"), |error| {
            matches!(error, PgnValidationError::MoveNumberOutOfSequence { .. })
        }, "MoveNumberOutOfSequence");
    }

    #[test]
    fn rejects_missing_check_suffix() {
        // 3. Bb5 gives check but is exported without the required "+".
        assert_error(&doc("*", "Unterminated", "1. e4 c5 2. Nf3 d6 3. Bb5 *"), |error| {
            matches!(error, PgnValidationError::IncorrectCheckSuffix { .. })
        }, "IncorrectCheckSuffix");
    }

    #[test]
    fn rejects_missing_checkmate_suffix() {
        // 4. Qxf7 is mate but is exported without the required "#".
        assert_error(&doc("1-0", "Normal", "1. e4 e5 2. Bc4 Nc6 3. Qh5 Nf6 4. Qxf7 1-0"), |error| {
            matches!(error, PgnValidationError::IncorrectCheckSuffix { .. })
        }, "IncorrectCheckSuffix");
    }

    #[test]
    fn rejects_check_suffix_on_a_non_checking_move() {
        assert_error(&doc("*", "Unterminated", "1. e4+ *"), |error| {
            matches!(error, PgnValidationError::IncorrectCheckSuffix { .. })
        }, "IncorrectCheckSuffix");
    }

    #[test]
    fn rejects_moves_after_checkmate() {
        assert_error(&doc(
            "*",
            "Unterminated",
            "1. e4 e5 2. Bc4 Nc6 3. Qh5 Nf6 4. Qxf7# 5. Kg1 *",
        ), |error| {
            matches!(error, PgnValidationError::MovesAfterGameOver { .. })
        }, "MovesAfterGameOver");
    }

    #[test]
    fn rejects_missing_termination_marker() {
        assert_error(&doc("*", "Unterminated", "1. e4 e5 2. Nf3 Nc6"), |error| {
            matches!(error, PgnValidationError::MissingTerminationMarker)
        }, "MissingTerminationMarker");
    }

    #[test]
    fn rejects_tokens_after_termination_marker() {
        assert_error(&doc("*", "Unterminated", "1. e4 e5 * 2. Nf3"), |error| {
            matches!(error, PgnValidationError::TokenAfterTermination(_))
        }, "TokenAfterTermination");
    }

    // ---------------------------------------------------------------
    // Negative cases: annotations and end-of-game consistency
    // ---------------------------------------------------------------

    #[test]
    fn rejects_malformed_eval_annotation() {
        assert_error(&doc("*", "Unterminated", "1. e4 {[%eval 0.3.2]} *"), |error| {
            matches!(error, PgnValidationError::MalformedEvalAnnotation(_))
        }, "MalformedEvalAnnotation");
    }

    #[test]
    fn rejects_malformed_clock_annotation() {
        assert_error(&doc("*", "Unterminated", "1. e4 {[%clk 4:58]} *"), |error| {
            matches!(error, PgnValidationError::MalformedClockAnnotation(_))
        }, "MalformedClockAnnotation");
    }

    #[test]
    fn rejects_unbalanced_comment() {
        assert_error(&doc("*", "Unterminated", "1. e4 {unterminated comment *"), |error| {
            matches!(error, PgnValidationError::UnbalancedComment)
        }, "UnbalancedComment");
    }

    #[test]
    fn rejects_termination_result_mismatch() {
        assert_error(&doc("*", "Time forfeit", "1. e4 e5 *"), |error| {
            matches!(error, PgnValidationError::TerminationResultMismatch { .. })
        }, "TerminationResultMismatch");
    }

    #[test]
    fn rejects_stalemate_claim_when_position_is_not_stalemate() {
        assert_error(&doc("1/2-1/2", "Stalemate", "1. e4 e5 2. Nf3 1/2-1/2"), |error| {
            matches!(error, PgnValidationError::StalemateClaimButNotStalemate)
        }, "StalemateClaimButNotStalemate");
    }

    #[test]
    fn rejects_fifty_move_claim_not_satisfied() {
        assert_error(&doc("1/2-1/2", "50-move rule", "1. e4 e5 1/2-1/2"), |error| {
            matches!(error, PgnValidationError::FiftyMoveRuleNotSatisfied { .. })
        }, "FiftyMoveRuleNotSatisfied");
    }

    #[test]
    fn rejects_result_inconsistent_with_checkmate() {
        assert_error(&doc("1/2-1/2", "Normal", "1. e4 e5 2. Bc4 Nc6 3. Qh5 Nf6 4. Qxf7# 1/2-1/2"), |error| {
            matches!(error, PgnValidationError::ResultPositionMismatch { .. })
        }, "ResultPositionMismatch");
    }

    #[test]
    fn error_codes_are_stable_and_machine_readable() {
        assert_eq!(
            PgnValidationError::MissingTerminationTag.code(),
            "PGN_MISSING_TERMINATION_TAG"
        );
        assert_eq!(
            PgnValidationError::IncorrectCheckSuffix {
                ply: 0,
                expected: "Qxf7#".to_string(),
                found: "Qxf7".to_string(),
            }
            .code(),
            "PGN_INCORRECT_CHECK_SUFFIX"
        );
    }
}
