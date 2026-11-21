//! Bitboard-based board representation.
//!
//! This module provides a compact board state built from 64-bit bitboards,
//! one per piece kind and color, plus helpers for querying occupancy.

/// Piece color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    White,
    Black,
}

/// Piece kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PieceKind {
    Pawn,
    Knight,
    Bishop,
    Rook,
    Queen,
    King,
}

/// A square index in the range 0..64 (a1 = 0, h8 = 63).
pub type Square = u8;

/// A bitboard with one bit per square.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Bitboard(pub u64);

impl Bitboard {
    pub const EMPTY: Bitboard = Bitboard(0);

    /// Returns a bitboard with only the given square set.
    pub fn from_square(sq: Square) -> Bitboard {
        debug_assert!(sq < 64);
        Bitboard(1u64 << sq)
    }

    /// Returns true if the given square is set.
    pub fn contains(self, sq: Square) -> bool {
        debug_assert!(sq < 64);
        (self.0 >> sq) & 1 == 1
    }

    /// Sets the given square.
    pub fn set(&mut self, sq: Square) {
        debug_assert!(sq < 64);
        self.0 |= 1u64 << sq;
    }

    /// Clears the given square.
    pub fn clear(&mut self, sq: Square) {
        debug_assert!(sq < 64);
        self.0 &= !(1u64 << sq);
    }

    /// Number of set squares.
    pub fn count(self) -> u32 {
        self.0.count_ones()
    }

    /// True if no squares are set.
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }
}

/// Board state: one bitboard per (color, piece kind) pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Board {
    white: [Bitboard; 6],
    black: [Bitboard; 6],
}

impl Default for Board {
    fn default() -> Self {
        Self::empty()
    }
}

fn kind_index(kind: PieceKind) -> usize {
    match kind {
        PieceKind::Pawn => 0,
        PieceKind::Knight => 1,
        PieceKind::Bishop => 2,
        PieceKind::Rook => 3,
        PieceKind::Queen => 4,
        PieceKind::King => 5,
    }
}

impl Board {
    /// Creates a board with no pieces.
    pub fn empty() -> Board {
        Board {
            white: [Bitboard::EMPTY; 6],
            black: [Bitboard::EMPTY; 6],
        }
    }

    /// Places a piece on a square, replacing any piece already there.
    pub fn place(&mut self, color: Color, kind: PieceKind, sq: Square) {
        self.remove(sq);
        let idx = kind_index(kind);
        match color {
            Color::White => self.white[idx].set(sq),
            Color::Black => self.black[idx].set(sq),
        }
    }

    /// Removes any piece from a square.
    pub fn remove(&mut self, sq: Square) {
        for bb in self.white.iter_mut().chain(self.black.iter_mut()) {
            bb.clear(sq);
        }
    }

    /// Returns the piece occupying a square, if any.
    pub fn piece_at(&self, sq: Square) -> Option<(Color, PieceKind)> {
        const KINDS: [PieceKind; 6] = [
            PieceKind::Pawn,
            PieceKind::Knight,
            PieceKind::Bishop,
            PieceKind::Rook,
            PieceKind::Queen,
            PieceKind::King,
        ];
        for (i, kind) in KINDS.iter().enumerate() {
            if self.white[i].contains(sq) {
                return Some((Color::White, *kind));
            }
            if self.black[i].contains(sq) {
                return Some((Color::Black, *kind));
            }
        }
        None
    }

    /// Bitboard of all pieces of one color.
    pub fn occupancy(&self, color: Color) -> Bitboard {
        let arr = match color {
            Color::White => &self.white,
            Color::Black => &self.black,
        };
        Bitboard(arr.iter().fold(0u64, |acc, bb| acc | bb.0))
    }

    /// Bitboard of all occupied squares.
    pub fn all_occupancy(&self) -> Bitboard {
        Bitboard(self.occupancy(Color::White).0 | self.occupancy(Color::Black).0)
    }

    /// Bitboard of a specific piece kind for one color.
    pub fn pieces(&self, color: Color, kind: PieceKind) -> Bitboard {
        let idx = kind_index(kind);
        match color {
            Color::White => self.white[idx],
            Color::Black => self.black[idx],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_board_has_no_pieces() {
        let b = Board::empty();
        assert!(b.all_occupancy().is_empty());
        assert_eq!(b.piece_at(0), None);
    }

    #[test]
    fn place_and_query() {
        let mut b = Board::empty();
        b.place(Color::White, PieceKind::King, 4);
        b.place(Color::Black, PieceKind::Queen, 59);
        assert_eq!(b.piece_at(4), Some((Color::White, PieceKind::King)));
        assert_eq!(b.piece_at(59), Some((Color::Black, PieceKind::Queen)));
        assert_eq!(b.occupancy(Color::White).count(), 1);
        assert_eq!(b.all_occupancy().count(), 2);
    }

    #[test]
    fn place_replaces_existing_piece() {
        let mut b = Board::empty();
        b.place(Color::White, PieceKind::Knight, 10);
        b.place(Color::Black, PieceKind::Bishop, 10);
        assert_eq!(b.piece_at(10), Some((Color::Black, PieceKind::Bishop)));
        assert_eq!(b.occupancy(Color::White).count(), 0);
    }

    #[test]
    fn remove_clears_square() {
        let mut b = Board::empty();
        b.place(Color::White, PieceKind::Rook, 7);
        b.remove(7);
        assert_eq!(b.piece_at(7), None);
        assert!(b.all_occupancy().is_empty());
    }
}
