use db_entity::game::ResultSide;
use shakmaty::{fen::Fen, uci::Uci, CastlingMode, Chess, EnPassantMode, Position};
use thiserror::Error;

#[derive(Debug, Error, Clone)]
pub enum MoveValidationError {
    #[error("invalid position: {0}")]
    InvalidPosition(String),
    #[error("invalid move: {0}")]
    InvalidMove(String),
    #[error("illegal move: {0}")]
    IllegalMove(String),
}

/// Replays `played_ucis` from `start_fen`, applies `uci`, and returns the new FEN
/// plus an optional terminal result detected after the move.
pub fn apply_and_validate_uci_move(
    start_fen: &str,
    played_ucis: &[String],
    uci: &str,
) -> Result<(String, Option<ResultSide>), MoveValidationError> {
    let mut position = parse_position(start_fen)?;

    for (idx, mv) in played_ucis.iter().enumerate() {
        position = play_uci(&position, mv)
            .map_err(|e| MoveValidationError::InvalidPosition(format!("history move {}: {e}", idx + 1)))?;
    }

    position = play_uci(&position, uci)?;

    let fen = Fen::from_position(position.clone(), EnPassantMode::Legal).to_string();
    let result = if position.is_checkmate() {
        // After the move, side to move is the loser.
        Some(match position.turn() {
            shakmaty::Color::White => ResultSide::BlackWins,
            shakmaty::Color::Black => ResultSide::WhiteWins,
        })
    } else if position.is_stalemate() || position.is_insufficient_material() {
        Some(ResultSide::Draw)
    } else {
        None
    };

    Ok((fen, result))
}

/// Apply a single UCI move to the position described by `fen` (the stored
/// current position) and return the new FEN plus an optional terminal result.
pub fn apply_uci_to_fen(
    fen: &str,
    uci: &str,
) -> Result<(String, Option<ResultSide>), MoveValidationError> {
    let position = parse_position(fen)?;
    let position = play_uci(&position, uci)?;

    let new_fen = Fen::from_position(position.clone(), EnPassantMode::Legal).to_string();
    let result = if position.is_checkmate() {
        Some(match position.turn() {
            shakmaty::Color::White => ResultSide::BlackWins,
            shakmaty::Color::Black => ResultSide::WhiteWins,
        })
    } else if position.is_stalemate() || position.is_insufficient_material() {
        Some(ResultSide::Draw)
    } else {
        None
    };

    Ok((new_fen, result))
}

fn parse_position(fen: &str) -> Result<Chess, MoveValidationError> {
    let fen: Fen = fen
        .parse()
        .map_err(|e| MoveValidationError::InvalidPosition(e.to_string()))?;
    Chess::from_setup(fen.into_setup(), CastlingMode::Standard)
        .map_err(|e| MoveValidationError::InvalidPosition(e.to_string()))
}

fn play_uci(position: &Chess, uci: &str) -> Result<Chess, MoveValidationError> {
    let uci: Uci = uci
        .parse()
        .map_err(|e| MoveValidationError::InvalidMove(e.to_string()))?;
    let mv = uci
        .to_move(position)
        .map_err(|e| MoveValidationError::IllegalMove(e.to_string()))?;
    position
        .play(mv)
        .map_err(|e| MoveValidationError::IllegalMove(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const START: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

    #[test]
    fn rejects_illegal_move() {
        let err = apply_and_validate_uci_move(START, &[], "e2e5").unwrap_err();
        assert!(format!("{err}").contains("illegal") || format!("{err}").contains("invalid"));
    }

    #[test]
    fn detects_checkmate() {
        let moves = vec![
            "f2f3".to_string(),
            "e7e5".to_string(),
            "g2g4".to_string(),
        ];
        let (fen, result) = apply_and_validate_uci_move(START, &moves, "d8h4").unwrap();
        assert!(!fen.is_empty());
        assert_eq!(result, Some(ResultSide::BlackWins));
    }
}
