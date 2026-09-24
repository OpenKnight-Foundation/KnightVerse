#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bitboard(u64);

impl Bitboard {
    pub const EMPTY: Bitboard = Bitboard(0);
    pub const ALL: Bitboard = Bitboard(u64::MAX);
    pub const CENTER: Bitboard = Bitboard(0x1818000000);
    pub const FIRST_RANK: Bitboard = Bitboard(0xff);
    pub const LAST_RANK: Bitboard = Bitboard(0xff << 56);
    pub const LIGHT_SQUARES: Bitboard = Bitboard(0x55aa55aa55aa55aa);
    pub const DARK_SQUARES: Bitboard = Bitboard(0xaa55aa55aa55aa55);

    pub fn new(value: u64) -> Self {
        Bitboard(value)
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub fn non_empty(self) -> bool {
        !self.is_empty()
    }

    pub fn contains(self, square: u64) -> bool {
        (self.0 & (1 << square)) != 0
    }

    pub fn add(self, square: u64) -> Bitboard {
        Bitboard(self.0 | (1 << square))
    }

    pub fn remove(self, square: u64) -> Bitboard {
        Bitboard(self.0 & !(1 << square))
    }

    pub fn count(self) -> u32 {
        self.0.count_ones()
    }

    pub fn first(self) -> Option<u32> {
        if self.is_empty() {
            None
        } else {
            Some(self.0.trailing_zeros())
        }
    }

    pub fn last(self) -> Option<u32> {
        if self.is_empty() {
            None
        } else {
            Some(63 - self.0.leading_zeros())
        }
    }

    pub fn remove_first(self) -> Bitboard {
        Bitboard(self.0 & (self.0 - 1))
    }

    pub fn remove_last(self) -> Bitboard {
        Bitboard(self.0 & !(1 << self.last().unwrap_or(0)))
    }
}

impl std::ops::BitAnd for Bitboard {
    type Output = Bitboard;
    fn bitand(self, rhs: Bitboard) -> Bitboard {
        Bitboard(self.0 & rhs.0)
    }
}

impl std::ops::BitOr for Bitboard {
    type Output = Bitboard;
    fn bitor(self, rhs: Bitboard) -> Bitboard {
        Bitboard(self.0 | rhs.0)
    }
}

impl std::ops::BitXor for Bitboard {
    type Output = Bitboard;
    fn bitxor(self, rhs: Bitboard) -> Bitboard {
        Bitboard(self.0 ^ rhs.0)
    }
}

impl std::ops::Not for Bitboard {
    type Output = Bitboard;
    fn not(self) -> Bitboard {
        Bitboard(!self.0)
    }
}
