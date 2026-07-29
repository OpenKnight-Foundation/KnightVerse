use chess::bitboard::board::{Bitboard, Board, ByColor, ByRole, Color, Piece, Role, Square};
use std::collections::HashMap;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_piece_map() {
        // Create an empty board
        let mut board = Board::empty();

        // Add some pieces to specific squares
        let e2 = Square { value: 12 }; // e2 square
        let e4 = Square { value: 28 }; // e4 square
        let d8 = Square { value: 59 }; // d8 square

        let white_pawn = Piece {
            color: Color::White,
            role: Role::Pawn,
        };
        let white_king = Piece {
            color: Color::White,
            role: Role::King,
        };
        let black_queen = Piece {
            color: Color::Black,
            role: Role::Queen,
        };

        // Place pieces on the board
        board = board.put_or_replace(white_pawn, e2);
        board = board.put_or_replace(white_king, e4);
        board = board.put_or_replace(black_queen, d8);

        // Get the piece map
        let piece_map = board.piece_map();

        // Verify the map contains our pieces at the correct squares
        assert_eq!(piece_map.len(), 3);
        assert_eq!(piece_map.get(&e2), Some(&white_pawn));
        assert_eq!(piece_map.get(&e4), Some(&white_king));
        assert_eq!(piece_map.get(&d8), Some(&black_queen));

        // Test with an empty board
        let empty_board = Board::empty();
        let empty_map = empty_board.piece_map();
        assert_eq!(empty_map.len(), 0);
    }
}
