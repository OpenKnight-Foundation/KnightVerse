//! PGN (Portable Game Notation) Parser Module
//!
//! This module provides functionality to parse and validate PGN strings,
//! enabling users to import games from other chess platforms, and to build
//! PGN strings from stored game data for export.

use regex::Regex;
use shakmaty::{san::San, Chess, Move, Position};
use std::collections::HashMap;
use thiserror::Error;

/// Errors that can occur during PGN parsing and validation
#[derive(Debug, Error, Clone)]
pub enum PgnError {
    #[error("Invalid PGN format: {0}")]
    InvalidFormat(String),

    #[error("Missing required header: {0}")]
    MissingHeader(String),

    #[error("Invalid header format: {0}")]
    InvalidHeader(String),

    #[error("Illegal move at move {move_number}: '{move_text}' - {reason}")]
    IllegalMove {
        move_number: usize,
        move_text: String,
        reason: String,
    },

    #[error("Invalid result format: {0}")]
    InvalidResult(String),

    #[error("Empty PGN string")]
    EmptyPgn,
}

/// Represents the result of a chess game
#[derive(Debug, Clone, PartialEq)]
#[derive(Default)]
pub enum GameResult {
    WhiteWins,
    BlackWins,
    Draw,
    #[default]
    Ongoing,
}

impl GameResult {
    /// Parse a result string from PGN format
    pub fn from_pgn_string(s: &str) -> Result<Self, PgnError> {
        match s.trim() {
            "1-0" => Ok(GameResult::WhiteWins),
            "0-1" => Ok(GameResult::BlackWins),
            "1/2-1/2" => Ok(GameResult::Draw),
            "*" => Ok(GameResult::Ongoing),
            other => Err(PgnError::InvalidResult(other.to_string())),
        }
    }

    /// Convert to PGN result string
    pub fn to_pgn_string(&self) -> &'static str {
        match self {
            GameResult::WhiteWins => "1-0",
            GameResult::BlackWins => "0-1",
            GameResult::Draw => "1/2-1/2",
            GameResult::Ongoing => "*",
        }
    }
}

/// Headers extracted from a PGN string
#[derive(Debug, Clone, Default)]
pub struct PgnHeaders {
    pub event: Option<String>,
    pub site: Option<String>,
    pub date: Option<String>,
    pub round: Option<String>,
    pub white: String,
    pub black: String,
    pub result: GameResult,
    /// Any additional headers not explicitly parsed
    pub other: HashMap<String, String>,
}

/// Represents a fully parsed PGN game
#[derive(Debug, Clone)]
pub struct ParsedGame {
    pub headers: PgnHeaders,
    /// Moves in SAN notation
    pub moves: Vec<String>,
    /// The final FEN position after all moves
    pub final_fen: String,
    /// Total number of half-moves (plies)
    pub ply_count: usize,
}

/// Represents a validated game ready for storage
#[derive(Debug, Clone)]
pub struct ValidatedGame {
    pub headers: PgnHeaders,
    pub moves: Vec<String>,
    pub final_fen: String,
    pub ply_count: usize,
    pub is_valid: bool,
}

/// Parse PGN headers from the input string
fn parse_headers(pgn: &str) -> Result<(PgnHeaders, &str), PgnError> {
    let header_regex = Regex::new(r#"\[(\w+)\s+"([^"]+)"\]"#).unwrap();

    let mut headers = PgnHeaders::default();
    let mut last_header_end = 0;

    for cap in header_regex.captures_iter(pgn) {
        let full_match = cap.get(0).unwrap();
        last_header_end = full_match.end();

        let key = cap.get(1).unwrap().as_str();
        let value = cap.get(2).unwrap().as_str().to_string();

        match key.to_lowercase().as_str() {
            "event" => headers.event = Some(value),
            "site" => headers.site = Some(value),
            "date" => headers.date = Some(value),
            "round" => headers.round = Some(value),
            "white" => headers.white = value,
            "black" => headers.black = value,
            "result" => headers.result = GameResult::from_pgn_string(&value)?,
            _ => {
                headers.other.insert(key.to_string(), value);
            }
        }
    }

    // Validate required headers
    if headers.white.is_empty() {
        return Err(PgnError::MissingHeader("White".to_string()));
    }
    if headers.black.is_empty() {
        return Err(PgnError::MissingHeader("Black".to_string()));
    }

    // Get the move text (everything after headers)
    let move_text = &pgn[last_header_end..];

    Ok((headers, move_text))
}

/// Parse move text into individual SAN moves
fn parse_moves(move_text: &str) -> Vec<String> {
    // Remove comments (both curly brace and semicolon style)
    let without_curly_comments = Regex::new(r"\{[^}]*\}")
        .unwrap()
        .replace_all(move_text, " ");
    let without_semicolon_comments = Regex::new(r";[^\n]*")
        .unwrap()
        .replace_all(&without_curly_comments, " ");

    // Remove NAGs (Numeric Annotation Glyphs like $1, $2, etc.)
    let without_nags = Regex::new(r"\$\d+")
        .unwrap()
        .replace_all(&without_semicolon_comments, " ");

    // Remove variations (recursive parentheses - simplified, only top-level)
    let without_variations = Regex::new(r"\([^()]*\)")
        .unwrap()
        .replace_all(&without_nags, " ");

    // Split into tokens
    let tokens: Vec<&str> = without_variations.split_whitespace().collect();

    // Strip a leading move-number prefix from each token, then drop results and
    // empty tokens. The prefix regex has no trailing `$` anchor, so it also handles
    // the compact form where the number is glued to the move (e.g. `1.e4`,
    // `10...Nf6`), not just the spaced form (`1. e4`).
    let move_number_prefix = Regex::new(r"^\d+\.+").unwrap();
    let result_regex = Regex::new(r"^(1-0|0-1|1/2-1/2|\*)$").unwrap();

    tokens
        .into_iter()
        .map(|token| move_number_prefix.replace(token, "").into_owned())
        .filter(|token| !token.is_empty() && !result_regex.is_match(token))
        .collect()
}

/// Parse a PGN string into a ParsedGame
pub fn parse_pgn(pgn_string: &str) -> Result<ParsedGame, PgnError> {
    let pgn = pgn_string.trim();

    if pgn.is_empty() {
        return Err(PgnError::EmptyPgn);
    }

    let (headers, move_text) = parse_headers(pgn)?;
    let moves = parse_moves(move_text);

    Ok(ParsedGame {
        headers,
        moves,
        final_fen: String::new(), // Will be filled during validation
        ply_count: 0,
    })
}

/// Validate a parsed game by replaying all moves
pub fn validate_game(parsed: &ParsedGame) -> Result<ValidatedGame, PgnError> {
    let mut position: Chess = Chess::default();
    let mut validated_moves = Vec::new();

    for (idx, move_san) in parsed.moves.iter().enumerate() {
        let move_number = (idx / 2) + 1;

        // Parse the SAN move
        let san: San = move_san.parse().map_err(|_| PgnError::IllegalMove {
            move_number,
            move_text: move_san.clone(),
            reason: "Invalid move notation".to_string(),
        })?;

        // Try to play the move
        let chess_move = san.to_move(&position).map_err(|_| PgnError::IllegalMove {
            move_number,
            move_text: move_san.clone(),
            reason: "Move is not legal in this position".to_string(),
        })?;

        position = position
            .play(&chess_move)
            .map_err(|_| PgnError::IllegalMove {
                move_number,
                move_text: move_san.clone(),
                reason: "Move leaves king in check".to_string(),
            })?;

        validated_moves.push(move_san.clone());
    }

    // Get final FEN
    let final_fen =
        shakmaty::fen::Fen::from_position(position.clone(), shakmaty::EnPassantMode::Legal)
            .to_string();

    Ok(ValidatedGame {
        headers: parsed.headers.clone(),
        moves: validated_moves,
        final_fen,
        ply_count: parsed.moves.len(),
        is_valid: true,
    })
}

// ---------------------------------------------------------------------------
// Export support
// ---------------------------------------------------------------------------

/// Headers required to export a game as PGN. Distinct from `PgnHeaders`
/// (which is populated while *parsing* an incoming PGN) because export needs
/// FIDE-recommended supplemental tags that we don't require on import.
#[derive(Debug, Clone)]
pub struct ExportHeaders {
    pub event: String,
    pub site: String,
    /// "YYYY.MM.DD" per the PGN spec, or "????.??.??" if unknown.
    pub date: String,
    /// Round number, or "?" if not applicable.
    pub round: String,
    pub white: String,
    pub black: String,
    pub result: GameResult,
    pub white_elo: Option<i32>,
    pub black_elo: Option<i32>,
    /// e.g. "300+3" (300s base + 3s increment), or "-" if untimed.
    pub time_control: Option<String>,
    /// e.g. "Normal", "Time forfeit", "Abandoned".
    pub termination: Option<String>,
}

impl Default for ExportHeaders {
    fn default() -> Self {
        Self {
            event: "?".to_string(),
            site: "?".to_string(),
            date: "????.??.??".to_string(),
            round: "?".to_string(),
            white: String::new(),
            black: String::new(),
            result: GameResult::default(),
            white_elo: None,
            black_elo: None,
            time_control: None,
            termination: None,
        }
    }
}

/// A single played move plus whatever analysis annotations are available for
/// it. `san` should be the *bare* SAN (no check/mate suffix) — the builder
/// recomputes `+`/`#` itself from the replayed position so we never emit an
/// incorrect suffix regardless of what was originally stored.
#[derive(Debug, Clone, Default)]
pub struct MoveAnnotation {
    pub san: String,
    /// Clock remaining *after* this move, formatted "H:MM:SS" (PGN `%clk`).
    pub clock: Option<String>,
    /// Engine evaluation in centipawns from White's perspective.
    pub eval_centipawns: Option<i32>,
    /// Mate-in-N eval; takes precedence over `eval_centipawns` when present.
    pub eval_mate: Option<i32>,
    /// Move classification annotation glyph, e.g. "?!", "??", "!!".
    pub classification: Option<String>,
}

/// Builds a spec-compliant PGN string from headers + annotated moves.
pub struct PgnBuilder {
    headers: ExportHeaders,
    moves: Vec<MoveAnnotation>,
    include_analysis: bool,
}

const MOVETEXT_LINE_WIDTH: usize = 80;

impl PgnBuilder {
    pub fn new(headers: ExportHeaders, moves: Vec<MoveAnnotation>, include_analysis: bool) -> Self {
        Self {
            headers,
            moves,
            include_analysis,
        }
    }

    /// Escapes `"` and `\` inside a tag value, per the PGN spec.
    fn escape_tag_value(value: &str) -> String {
        value.replace('\\', "\\\\").replace('"', "\\\"")
    }

    fn tag(name: &str, value: &str) -> String {
        format!("[{} \"{}\"]\n", name, Self::escape_tag_value(value))
    }

    /// Seven Tag Roster first, then FIDE-recommended supplemental tags.
    fn format_headers(&self) -> String {
        let mut out = String::new();
        out.push_str(&Self::tag("Event", &self.headers.event));
        out.push_str(&Self::tag("Site", &self.headers.site));
        out.push_str(&Self::tag("Date", &self.headers.date));
        out.push_str(&Self::tag("Round", &self.headers.round));
        out.push_str(&Self::tag("White", &self.headers.white));
        out.push_str(&Self::tag("Black", &self.headers.black));
        out.push_str(&Self::tag("Result", self.headers.result.to_pgn_string()));

        if let Some(elo) = self.headers.white_elo {
            out.push_str(&Self::tag("WhiteElo", &elo.to_string()));
        }
        if let Some(elo) = self.headers.black_elo {
            out.push_str(&Self::tag("BlackElo", &elo.to_string()));
        }
        if let Some(tc) = &self.headers.time_control {
            out.push_str(&Self::tag("TimeControl", tc));
        }
        if let Some(term) = &self.headers.termination {
            out.push_str(&Self::tag("Termination", term));
        }

        out
    }

    /// Replays every move from the start position, recomputing the correct
    /// check (`+`) / checkmate (`#`) suffix instead of trusting whatever
    /// suffix (if any) was stored, so export can never emit invalid SAN.
    fn recompute_check_suffixes(moves: &[MoveAnnotation]) -> Result<Vec<String>, PgnError> {
        let mut position: Chess = Chess::default();
        let mut out = Vec::with_capacity(moves.len());

        for (idx, ann) in moves.iter().enumerate() {
            let move_number = (idx / 2) + 1;

            let san: San = ann.san.parse().map_err(|_| PgnError::IllegalMove {
                move_number,
                move_text: ann.san.clone(),
                reason: "Invalid move notation".to_string(),
            })?;

            let chess_move: Move = san.to_move(&position).map_err(|_| PgnError::IllegalMove {
                move_number,
                move_text: ann.san.clone(),
                reason: "Move is not legal in this position".to_string(),
            })?;

            position = position
                .play(&chess_move)
                .map_err(|_| PgnError::IllegalMove {
                    move_number,
                    move_text: ann.san.clone(),
                    reason: "Move leaves king in check".to_string(),
                })?;

            let suffix = if position.is_checkmate() {
                "#"
            } else if position.is_check() {
                "+"
            } else {
                ""
            };

            out.push(format!("{}{}", San::from(&chess_move), suffix));
        }

        Ok(out)
    }

    fn format_eval_tag(ann: &MoveAnnotation) -> Option<String> {
        if let Some(mate) = ann.eval_mate {
            Some(format!("[%eval #{}]", mate))
        } else {
            ann.eval_centipawns
                .map(|cp| format!("[%eval {:.2}]", cp as f64 / 100.0))
        }
    }

    fn format_clk_tag(clock: &str) -> String {
        format!("[%clk {}]", clock)
    }

    /// Builds the movetext, wrapping at `MOVETEXT_LINE_WIDTH` columns as
    /// recommended by the PGN spec.
    fn format_movetext(&self) -> Result<String, PgnError> {
        let sans = Self::recompute_check_suffixes(&self.moves)?;

        let mut tokens: Vec<String> = Vec::new();

        for (idx, (san, ann)) in sans.iter().zip(self.moves.iter()).enumerate() {
            let is_white = idx % 2 == 0;
            let move_number = (idx / 2) + 1;

            if is_white {
                tokens.push(format!("{}.", move_number));
            } else if idx == 0 {
                // Game starting on a black move (rare, but handle it).
                tokens.push(format!("{}...", move_number));
            }

            let mut move_token = san.clone();
            if let Some(class) = &ann.classification {
                move_token.push_str(class);
            }
            tokens.push(move_token);

            if self.include_analysis {
                let mut annotation_parts = Vec::new();
                if let Some(eval_tag) = Self::format_eval_tag(ann) {
                    annotation_parts.push(eval_tag);
                }
                if let Some(clk) = &ann.clock {
                    annotation_parts.push(Self::format_clk_tag(clk));
                }
                if !annotation_parts.is_empty() {
                    tokens.push(format!("{{ {} }}", annotation_parts.join(" ")));
                }
            }
        }

        tokens.push(self.headers.result.to_pgn_string().to_string());

        // Wrap tokens into lines no wider than MOVETEXT_LINE_WIDTH.
        let mut lines = Vec::new();
        let mut current_line = String::new();
        for token in tokens {
            if current_line.is_empty() {
                current_line = token;
            } else if current_line.len() + 1 + token.len() <= MOVETEXT_LINE_WIDTH {
                current_line.push(' ');
                current_line.push_str(&token);
            } else {
                lines.push(current_line);
                current_line = token;
            }
        }
        if !current_line.is_empty() {
            lines.push(current_line);
        }

        Ok(lines.join("\n"))
    }

    pub fn build(&self) -> Result<String, PgnError> {
        let headers = self.format_headers();
        let movetext = self.format_movetext()?;
        // Blank line separates the tag section from the movetext, per spec.
        Ok(format!("{}\n{}\n", headers, movetext))
    }
}

/// Formats a full PGN export string from headers + annotated moves.
///
/// This is the pure, DB-free formatting entry point — `GameService`
/// (service crate) is responsible for loading the game and its move
/// history and converting them into `ExportHeaders` / `MoveAnnotation`
/// before calling this.
pub fn export_pgn(
    headers: ExportHeaders,
    moves: Vec<MoveAnnotation>,
    include_analysis: bool,
) -> Result<String, PgnError> {
    if headers.white.is_empty() {
        return Err(PgnError::MissingHeader("White".to_string()));
    }
    if headers.black.is_empty() {
        return Err(PgnError::MissingHeader("Black".to_string()));
    }

    PgnBuilder::new(headers, moves, include_analysis).build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_pgn() {
        let pgn = r#"[White "Magnus Carlsen"]
[Black "Hikaru Nakamura"]
[Result "1-0"]

1. e4 e5 2. Nf3 Nc6 3. Bb5 1-0"#;

        let result = parse_pgn(pgn);
        assert!(result.is_ok());

        let parsed = result.unwrap();
        assert_eq!(parsed.headers.white, "Magnus Carlsen");
        assert_eq!(parsed.headers.black, "Hikaru Nakamura");
        assert_eq!(parsed.headers.result, GameResult::WhiteWins);
        assert_eq!(parsed.moves.len(), 5);
    }

    #[test]
    fn test_validate_legal_game() {
        let pgn = r#"[White "Player1"]
[Black "Player2"]
[Result "1-0"]

1. e4 e5 2. Nf3 Nc6 1-0"#;

        let parsed = parse_pgn(pgn).unwrap();
        let validated = validate_game(&parsed);

        assert!(validated.is_ok());
        let game = validated.unwrap();
        assert!(game.is_valid);
        assert_eq!(game.ply_count, 4);
    }

    #[test]
    fn test_reject_illegal_move() {
        // Ke3 is illegal because the king cannot move to e3 from e1 in one move
        // (it would need to pass through e2)
        let pgn = r#"[White "Player1"]
[Black "Player2"]
[Result "*"]

1. e4 e5 2. Ke3 *"#;

        let parsed = parse_pgn(pgn).unwrap();
        let validated = validate_game(&parsed);

        assert!(validated.is_err());
        if let Err(PgnError::IllegalMove { move_text, .. }) = validated {
            assert_eq!(move_text, "Ke3");
        }
    }

    #[test]
    fn test_missing_white_header() {
        let pgn = r#"[Black "Player2"]
[Result "1-0"]

1. e4 1-0"#;

        let result = parse_pgn(pgn);
        assert!(matches!(result, Err(PgnError::MissingHeader(_))));
    }

    #[test]
    fn test_parse_headers_with_comments() {
        let pgn = r#"[White "Player1"]
[Black "Player2"]
[Result "1/2-1/2"]

1. e4 {Opening move} e5 2. Nf3 Nc6 1/2-1/2"#;

        let parsed = parse_pgn(pgn).unwrap();
        assert_eq!(parsed.moves.len(), 4);
        assert_eq!(parsed.headers.result, GameResult::Draw);
    }

    #[test]
    fn test_parse_compact_move_numbers() {
        // Compact form: move number glued to the move (`1.e4`), as produced by
        // many exporters. Previously these tokens slipped through the move-number
        // filter and failed SAN validation.
        let pgn = r#"[White "Player1"]
[Black "Player2"]
[Result "*"]

1.e4 e5 2.Nf3 Nc6 3.Bb5 *"#;

        let parsed = parse_pgn(pgn).unwrap();
        assert_eq!(parsed.moves, vec!["e4", "e5", "Nf3", "Nc6", "Bb5"]);
        assert!(validate_game(&parsed).is_ok());
    }

    #[test]
    fn test_parse_compact_black_continuation() {
        // Black move-number continuation glued to the move (`1...e5`).
        let pgn = r#"[White "Player1"]
[Black "Player2"]
[Result "1-0"]

1.e4 {comment} 1...e5 2.Nf3 1-0"#;

        let parsed = parse_pgn(pgn).unwrap();
        assert_eq!(parsed.moves, vec!["e4", "e5", "Nf3"]);
    }

    #[test]
    fn test_parse_two_digit_move_numbers() {
        let pgn = r#"[White "Player1"]
[Black "Player2"]
[Result "*"]

10.Ba2 Bb7 11.Qe2 *"#;

        let parsed = parse_pgn(pgn).unwrap();
        assert_eq!(parsed.moves, vec!["Ba2", "Bb7", "Qe2"]);
    }

    #[test]
    fn test_game_result_parsing() {
        assert_eq!(
            GameResult::from_pgn_string("1-0").unwrap(),
            GameResult::WhiteWins
        );
        assert_eq!(
            GameResult::from_pgn_string("0-1").unwrap(),
            GameResult::BlackWins
        );
        assert_eq!(
            GameResult::from_pgn_string("1/2-1/2").unwrap(),
            GameResult::Draw
        );
        assert_eq!(
            GameResult::from_pgn_string("*").unwrap(),
            GameResult::Ongoing
        );
    }

    // -----------------------------------------------------------------
    // Export tests
    // -----------------------------------------------------------------

    fn sample_export_headers() -> ExportHeaders {
        ExportHeaders {
            event: "Rated Blitz Game".to_string(),
            site: "chess.example.com".to_string(),
            date: "2026.08.27".to_string(),
            round: "1".to_string(),
            white: "Magnus Carlsen".to_string(),
            black: "Hikaru Nakamura".to_string(),
            result: GameResult::WhiteWins,
            white_elo: Some(2839),
            black_elo: Some(2802),
            time_control: Some("300+3".to_string()),
            termination: Some("Normal".to_string()),
        }
    }

    fn sample_moves() -> Vec<MoveAnnotation> {
        vec![
            MoveAnnotation {
                san: "e4".to_string(),
                clock: Some("0:04:58".to_string()),
                ..Default::default()
            },
            MoveAnnotation {
                san: "e5".to_string(),
                clock: Some("0:04:57".to_string()),
                ..Default::default()
            },
            MoveAnnotation {
                san: "Nf3".to_string(),
                clock: Some("0:04:55".to_string()),
                eval_centipawns: Some(32),
                ..Default::default()
            },
            MoveAnnotation {
                san: "Nc6".to_string(),
                clock: Some("0:04:53".to_string()),
                eval_centipawns: Some(28),
                ..Default::default()
            },
            MoveAnnotation {
                san: "Bb5".to_string(),
                clock: Some("0:04:50".to_string()),
                classification: Some("!".to_string()),
                ..Default::default()
            },
        ]
    }

    #[test]
    fn test_export_pgn_headers_present() {
        let pgn = export_pgn(sample_export_headers(), sample_moves(), false).unwrap();
        assert!(pgn.contains("[Event \"Rated Blitz Game\"]"));
        assert!(pgn.contains("[Site \"chess.example.com\"]"));
        assert!(pgn.contains("[Date \"2026.08.27\"]"));
        assert!(pgn.contains("[Round \"1\"]"));
        assert!(pgn.contains("[White \"Magnus Carlsen\"]"));
        assert!(pgn.contains("[Black \"Hikaru Nakamura\"]"));
        assert!(pgn.contains("[WhiteElo \"2839\"]"));
        assert!(pgn.contains("[BlackElo \"2802\"]"));
        assert!(pgn.contains("[TimeControl \"300+3\"]"));
        assert!(pgn.contains("[Termination \"Normal\"]"));
        assert!(pgn.contains("[Result \"1-0\"]"));
    }

    #[test]
    fn test_export_pgn_roundtrips_through_parser() {
        let pgn = export_pgn(sample_export_headers(), sample_moves(), false).unwrap();
        let parsed = parse_pgn(&pgn).unwrap();
        let validated = validate_game(&parsed).unwrap();
        assert!(validated.is_valid);
        assert_eq!(validated.moves, vec!["e4", "e5", "Nf3", "Nc6", "Bb5"]);
    }

    #[test]
    fn test_export_pgn_includes_clock_and_eval_when_requested() {
        let pgn = export_pgn(sample_export_headers(), sample_moves(), true).unwrap();
        assert!(pgn.contains("[%clk 0:04:58]"));
        assert!(pgn.contains("[%eval 0.32]"));
        // Result string still parses cleanly with annotations present.
        let parsed = parse_pgn(&pgn).unwrap();
        assert!(validate_game(&parsed).is_ok());
    }

    #[test]
    fn test_export_pgn_omits_analysis_when_not_requested() {
        let pgn = export_pgn(sample_export_headers(), sample_moves(), false).unwrap();
        assert!(!pgn.contains("%clk"));
        assert!(!pgn.contains("%eval"));
    }

    #[test]
    fn test_export_pgn_recomputes_check_suffix() {
        // Scholar's mate — bare SANs supplied with no +/# suffix; the
        // builder must recompute "Qxf7#" itself.
        let moves = vec![
            MoveAnnotation { san: "e4".to_string(), ..Default::default() },
            MoveAnnotation { san: "e5".to_string(), ..Default::default() },
            MoveAnnotation { san: "Bc4".to_string(), ..Default::default() },
            MoveAnnotation { san: "Nc6".to_string(), ..Default::default() },
            MoveAnnotation { san: "Qh5".to_string(), ..Default::default() },
            MoveAnnotation { san: "Nf6".to_string(), ..Default::default() },
            MoveAnnotation { san: "Qxf7".to_string(), ..Default::default() },
        ];
        let mut headers = sample_export_headers();
        headers.result = GameResult::WhiteWins;

        let pgn = export_pgn(headers, moves, false).unwrap();
        assert!(pgn.contains("Qxf7#"));
    }

    #[test]
    fn test_export_pgn_check_suffix_non_mating() {
        // A mid-game check that is not mate should get "+", never "#".
        let moves = vec![
            MoveAnnotation { san: "e4".to_string(), ..Default::default() },
            MoveAnnotation { san: "e6".to_string(), ..Default::default() },
            MoveAnnotation { san: "Bb5".to_string(), ..Default::default() },
        ];
        let mut headers = sample_export_headers();
        headers.result = GameResult::Ongoing;

        let pgn = export_pgn(headers, moves, false).unwrap();
        assert!(pgn.contains("Bb5+"));
        assert!(!pgn.contains("Bb5#"));
    }

    #[test]
    fn test_export_pgn_rejects_missing_white() {
        let mut headers = sample_export_headers();
        headers.white = String::new();
        let result = export_pgn(headers, sample_moves(), false);
        assert!(matches!(result, Err(PgnError::MissingHeader(_))));
    }

    #[test]
    fn test_export_pgn_rejects_missing_black() {
        let mut headers = sample_export_headers();
        headers.black = String::new();
        let result = export_pgn(headers, sample_moves(), false);
        assert!(matches!(result, Err(PgnError::MissingHeader(_))));
    }

    #[test]
    fn test_export_pgn_rejects_illegal_move_sequence() {
        let moves = vec![MoveAnnotation {
            san: "Ke3".to_string(),
            ..Default::default()
        }];
        let result = export_pgn(sample_export_headers(), moves, false);
        assert!(matches!(result, Err(PgnError::IllegalMove { .. })));
    }

    #[test]
    fn test_export_pgn_handles_no_moves() {
        let mut headers = sample_export_headers();
        headers.result = GameResult::Ongoing;
        let pgn = export_pgn(headers, vec![], false).unwrap();
        assert!(pgn.contains("[Result \"*\"]"));
        assert!(pgn.trim_end().ends_with('*'));
    }
}