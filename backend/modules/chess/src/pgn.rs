//! PGN (Portable Game Notation) Parser Module
//!
//! This module provides functionality to parse and validate PGN strings,
//! enabling users to import games from other chess platforms.

use regex::Regex;
use shakmaty::{san::San, Chess, Position};
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

    // Filter out move numbers, results, and other non-move tokens
    // Match standalone move numbers like "1." or "1..." (spaced format)
    let standalone_move_number_regex = Regex::new(r"^\d+\.+$").unwrap();
    // Strip leading move-number prefix from tokens like "1.e4" or "10...Nf3"
    let move_number_prefix_regex = Regex::new(r"^\d+\.+").unwrap();
    let result_regex = Regex::new(r"^(1-0|0-1|1/2-1/2|\*)$").unwrap();

    tokens
        .into_iter()
        .filter(|token| !token.is_empty())
        .filter_map(|token| {
            // Skip standalone results
            if result_regex.is_match(token) {
                return None;
            }
            // Skip standalone move numbers (spaced PGN: "1." "2." etc.)
            if standalone_move_number_regex.is_match(token) {
                return None;
            }
            // Strip leading move-number prefix from compact tokens (e.g. "1.e4" → "e4")
            let cleaned = move_number_prefix_regex.replace(token, "");
            if cleaned.is_empty() {
                return None;
            }
            Some(cleaned.to_string())
        })
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
    fn test_parse_compact_pgn_with_glued_move_numbers() {
        let pgn = r#"[White "A"]
[Black "B"]
[Result "*"]

1.e4 e5 2.Nf3 Nc6 *"#;

        let result = parse_pgn(pgn);
        assert!(result.is_ok());

        let parsed = result.unwrap();
        assert_eq!(parsed.moves, vec!["e4", "e5", "Nf3", "Nc6"]);

        let validated = validate_game(&parsed);
        assert!(validated.is_ok());
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
}
