//! Bitboard primitives used by the chess engine.
//!
//! Note: this file is created as part of the Glicko-2 rating work (BE-187)
//! only so the chess crate's module tree is well-formed; the rating logic
//! itself lives in `rating.rs`.

/// A 64-bit bitboard where bit `i` corresponds to square `i`
/// (a1 = 0, h8 = 63).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Bitboard(pub u64);

impl Bitboard {
    /// Empty bitboard.
    pub const EMPTY: Bitboard = Bitboard(0);

    /// Creates a bitboard from a raw `u64`.
    #[inline]
    pub const fn new(bits: u64) -> Self {
        Bitboard(bits)
    }

    /// Returns `true` if no squares are set.
    #[inline]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Returns the number of set squares.
    #[inline]
    pub const fn count(self) -> u32 {
        self.0.count_ones()
    }

    /// Returns `true` if the square index (0..64) is set.
    #[inline]
    pub const fn contains(self, square: u8) -> bool {
        (self.0 >> square) & 1 == 1
    }

    /// Sets the square index (0..64).
    #[inline]
    pub fn set(&mut self, square: u8) {
        self.0 |= 1u64 << square;
    }

    /// Clears the square index (0..64).
    #[inline]
    pub fn clear(&mut self, square: u8) {
        self.0 &= !(1u64 << square);
    }

    /// Returns the index of the least significant set square, if any.
    #[inline]
    pub const fn lsb(self) -> Option<u8> {
        if self.0 == 0 {
            None
        } else {
            Some(self.0.trailing_zeros() as u8)
        }
    }
}

impl std::ops::BitAnd for Bitboard {
    type Output = Bitboard;
    #[inline]
    fn bitand(self, rhs: Self) -> Self::Output {
        Bitboard(self.0 & rhs.0)
    }
}

impl std::ops::BitOr for Bitboard {
    type Output = Bitboard;
    #[inline]
    fn bitor(self, rhs: Self) -> Self::Output {
        Bitboard(self.0 | rhs.0)
    }
}

impl std::ops::BitXor for Bitboard {
    type Output = Bitboard;
    #[inline]
    fn bitxor(self, rhs: Self) -> Self::Output {
        Bitboard(self.0 ^ rhs.0)
    }
}

impl std::ops::Not for Bitboard {
    type Output = Bitboard;
    #[inline]
    fn not(self) -> Self::Output {
        Bitboard(!self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_clear_contains() {
        let mut bb = Bitboard::EMPTY;
        assert!(bb.is_empty());
        bb.set(0);
        bb.set(63);
        assert!(bb.contains(0));
        assert!(bb.contains(63));
        assert_eq!(bb.count(), 2);
        bb.clear(0);
        assert!(!bb.contains(0));
        assert_eq!(bb.lsb(), Some(63));
    }

    #[test]
    fn bitwise_ops() {
        let a = Bitboard::new(0b1100);
        let b = Bitboard::new(0b1010);
        assert_eq!((a & b).0, 0b1000);
        assert_eq!((a | b).0, 0b1110);
        assert_eq!((a ^ b).0, 0b0110);
        assert_eq!((!Bitboard::EMPTY).0, u64::MAX);
    }
}
