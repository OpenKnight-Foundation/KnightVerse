use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Chess960Position {
    pub position_number: u16,
    pub fen: String,
    pub back_rank: String,
    pub white_king_pos: usize,
    pub white_rook_positions: [usize; 2],
    pub white_bishop_positions: [usize; 2],
    pub white_knight_positions: [usize; 2],
    pub white_queen_pos: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Chess960Library {
    pub positions: HashMap<u16, Chess960Position>,
    pub total_positions: u16,
    pub metadata: LibraryMetadata,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LibraryMetadata {
    pub version: String,
    pub generated_at: String,
    pub description: String,
}
