
use std::collections::HashMap;
use std::ops::{BitAnd, BitOr, BitXor, Not};


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bitboard(pub u64);

impl Bitboard {
    pub const EMPTY: Bitboard = Bitboard(0);
    pub const ALL: Bitboard = Bitboard(u64::MAX);
    // Center squares: E4, D4, E5, D5.
    pub const CENTER: Bitboard = Bitboard(0x1818000000);
    pub const FIRST_RANK: Bitboard = Bitboard(0xff);
    pub const LAST_RANK: Bitboard = Bitboard(0xff << 56);
    pub const LIGHT_SQUARES: Bitboard = Bitboard(0x55aa55aa55aa55aa);
    pub const DARK_SQUARES: Bitboard = Bitboard(0xaa55aa55aa55aa55);

    pub fn new(value: u64) -> Self {
        Bitboard(value)
    }

    /// Count the number of bits set.
    pub fn count(self) -> u32 {
        self.0.count_ones()
    }

    /// Convert the bitboard to a vector of squares.
    pub fn to_squares(self) -> Vec<Square> {
        let mut squares = Vec::new();
        let mut bb = self.0;
        while bb != 0 {
            let ls = bb.trailing_zeros();
            squares.push(Square { value: ls as u8 });
            bb &= bb - 1;
        }
        squares
    }

    /// If exactly one bit is set, returns that square.
    pub fn single_square(self) -> Option<Square> {
        if self.0 != 0 && (self.0 & (self.0 - 1)) == 0 {
            Some(Square {
                value: self.0.trailing_zeros() as u8,
            })
        } else {
            None
        }
    }
}

// Bitwise operators for Bitboard.
impl BitAnd for Bitboard {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self::Output {
        Bitboard(self.0 & rhs.0)
    }
}

impl BitOr for Bitboard {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self::Output {
        Bitboard(self.0 | rhs.0)
    }
}

impl BitXor for Bitboard {
    type Output = Self;
    fn bitxor(self, rhs: Self) -> Self::Output {
        Bitboard(self.0 ^ rhs.0)
    }
}

impl Not for Bitboard {
    type Output = Self;
    fn not(self) -> Self::Output {
        Bitboard(!self.0)
    }
}

/// Placeholder types for Color, Role, Piece, and Square.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    White,
    Black,
}

impl Color {
    pub fn opposite(self) -> Color {
        match self {
            Color::White => Color::Black,
            Color::Black => Color::White,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Pawn,
    Knight,
    Bishop,
    Rook,
    Queen,
    King,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Square {
    pub value: u8, // 0..63 representing the square.
}

impl Square {
    /// Returns the bitboard corresponding to this square.
    pub fn bitboard(self) -> Bitboard {
        Bitboard(1u64 << self.value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Piece {
    //  Write a function that, given a square, an attacking color, and an occupied bitboard,
    //   returns a Bitboard representing all pieces of that color that can attack the square.
    // - Consider all piece attack patterns: rook, bishop, knight, king, and pawn.
    
    pub color: Color,
    pub role: Role,
}

/// A mapping of squares to pieces.
pub type PieceMap = HashMap<Square, Piece>;

/// Holds bitboards for each color.
#[derive(Debug, Clone, Copy)]
pub struct ByColor {
    pub white: Bitboard,
    pub black: Bitboard,
}

impl ByColor {
    pub fn new(white: Bitboard, black: Bitboard) -> Self {
        ByColor { white, black }
    }

    pub fn get(&self, color: Color) -> Bitboard {
        match color {
            Color::White => self.white,
            Color::Black => self.black,
        }
    }

    pub fn update<F>(&self, color: Color, f: F) -> ByColor
    where
        F: Fn(Bitboard) -> Bitboard,
    {
        match color {
            Color::White => ByColor {
                white: f(self.white),
                black: self.black,
            },
            Color::Black => ByColor {
                white: self.white,
                black: f(self.black),
            },
        }
    }

    pub fn foreach<F>(&self, mut f: F)
    where
        F: FnMut(Color, Bitboard),
    {
        f(Color::White, self.white);
        f(Color::Black, self.black);
    }

    pub fn fill(a: Bitboard) -> ByColor {
        ByColor { white: a, black: a }
    }

    /// Returns the first color for which the predicate holds.
    pub fn find_color<F>(&self, pred: F) -> Option<Color>
    where
        F: Fn(Bitboard) -> bool,
    {
        if pred(self.white) {
            Some(Color::White)
        } else if pred(self.black) {
            Some(Color::Black)
        } else {
            None
        }
    }
}

/// Holds bitboards for each role.
#[derive(Debug, Clone, Copy)]
pub struct ByRole {
    pub pawn: Bitboard,
    pub knight: Bitboard,
    pub bishop: Bitboard,
    pub rook: Bitboard,
    pub queen: Bitboard,
    pub king: Bitboard,
}

impl ByRole {
    pub fn new(
        pawn: Bitboard,
        knight: Bitboard,
        bishop: Bitboard,
        rook: Bitboard,
        queen: Bitboard,
        king: Bitboard,
    ) -> Self {
        ByRole {
            pawn,
            knight,
            bishop,
            rook,
            queen,
            king,
        }
    }

    pub fn get(&self, role: Role) -> Bitboard {
        match role {
            Role::Pawn => self.pawn,
            Role::Knight => self.knight,
            Role::Bishop => self.bishop,
            Role::Rook => self.rook,
            Role::Queen => self.queen,
            Role::King => self.king,
        }
    }

    pub fn update<F>(&self, role: Role, f: F) -> ByRole
    where
        F: Fn(Bitboard) -> Bitboard,
    {
        match role {
            Role::Pawn => ByRole {
                pawn: f(self.pawn),
                ..*self
            },
            Role::Knight => ByRole {
                knight: f(self.knight),
                ..*self
            },
            Role::Bishop => ByRole {
                bishop: f(self.bishop),
                ..*self
            },
            Role::Rook => ByRole {
                rook: f(self.rook),
                ..*self
            },
            Role::Queen => ByRole {
                queen: f(self.queen),
                ..*self
            },
            Role::King => ByRole {
                king: f(self.king),
                ..*self
            },
        }
    }

    pub fn foreach<F>(&self, mut f: F)
    where
        F: FnMut(Role, Bitboard),
    {
        f(Role::Pawn, self.pawn);
        f(Role::Knight, self.knight);
        f(Role::Bishop, self.bishop);
        f(Role::Rook, self.rook);
        f(Role::Queen, self.queen);
        f(Role::King, self.king);
    }

    pub fn find_role<F>(&self, pred: F) -> Option<Role>
    where
        F: Fn(Bitboard) -> bool,
    {
        if pred(self.pawn) {
            Some(Role::Pawn)
        } else if pred(self.knight) {
            Some(Role::Knight)
        } else if pred(self.bishop) {
            Some(Role::Bishop)
        } else if pred(self.rook) {
            Some(Role::Rook)
        } else if pred(self.queen) {
            Some(Role::Queen)
        } else if pred(self.king) {
            Some(Role::King)
        } else {
            None
        }
    }

    pub fn map<F>(&self, f: F) -> ByRole
    where
        F: Fn(Bitboard) -> Bitboard,
    {
        ByRole {
            pawn: f(self.pawn),
            knight: f(self.knight),
            bishop: f(self.bishop),
            rook: f(self.rook),
            queen: f(self.queen),
            king: f(self.king),
        }
    }

    pub fn fill(a: Bitboard) -> ByRole {
        ByRole {
            pawn: a,
            knight: a,
            bishop: a,
            rook: a,
            queen: a,
            king: a,
        }
    }
}

/// The main Board struct representing the chess board.
#[derive(Debug, Clone, Copy)]
pub struct Board {
    pub occupied: Bitboard,
    pub by_color: ByColor,
    pub by_role: ByRole,
}

impl Board {
    pub fn new(occupied: Bitboard, by_color: ByColor, by_role: ByRole) -> Self {
        Board {
            occupied,
            by_color,
            by_role,
        }
    }

    /// An empty board.
    pub fn empty() -> Board {
        Board {
            occupied: Bitboard::EMPTY,
            by_color: ByColor::fill(Bitboard::EMPTY),
            by_role: ByRole::fill(Bitboard::EMPTY),
        }
    }

    // Getters for various bitboards.
    pub fn white(&self) -> Bitboard {
        self.by_color.get(Color::White)
    }
    pub fn black(&self) -> Bitboard {
        self.by_color.get(Color::Black)
    }
    pub fn pawns(&self) -> Bitboard {
        self.by_role.get(Role::Pawn)
    }
    pub fn knights(&self) -> Bitboard {
        self.by_role.get(Role::Knight)
    }
    pub fn bishops(&self) -> Bitboard {
        self.by_role.get(Role::Bishop)
    }
    pub fn rooks(&self) -> Bitboard {
        self.by_role.get(Role::Rook)
    }
    pub fn queens(&self) -> Bitboard {
        self.by_role.get(Role::Queen)
    }
    pub fn kings(&self) -> Bitboard {
        self.by_role.get(Role::King)
    }

    /// Sliders are bishops, rooks, and queens.
    pub fn sliders(&self) -> Bitboard {
        self.bishops() ^ self.rooks() ^ self.queens()
    }

    /// Returns true if the given square is occupied.
    pub fn is_occupied_square(&self, s: Square) -> bool {
        (self.occupied.0 & s.bitboard().0) != 0
    }

    /// Total number of pieces.
    pub fn nb_pieces(&self) -> u32 {
        self.occupied.count()
    }

    /// Returns the bitboard for a given piece.
    pub fn by_piece(&self, piece: Piece) -> Bitboard {
        self.by_color.get(piece.color) & self.by_role.get(piece.role)
    }

    /// Returns the role at a square, if any.
    pub fn role_at(&self, s: Square) -> Option<Role> {
        self.by_role.find_role(|b| (b.0 & s.bitboard().0) != 0)
    }

    /// Returns the color at a square, if any.
    pub fn color_at(&self, s: Square) -> Option<Color> {
        self.by_color.find_color(|b| (b.0 & s.bitboard().0) != 0)
    }

    /// Returns the piece at a square, if any.
    pub fn piece_at(&self, s: Square) -> Option<Piece> {
        if let (Some(color), Some(role)) = (self.color_at(s), self.role_at(s)) {
            Some(Piece { color, role })
        } else {
            None
        }
    }

    pub fn white_at(&self, s: Square) -> bool {
        (self.white().0 & s.bitboard().0) != 0
    }
    pub fn black_at(&self, s: Square) -> bool {
        (self.black().0 & s.bitboard().0) != 0
    }

    /// Returns the king bitboard for a given color.
    pub fn king_of(&self, color: Color) -> Bitboard {
        self.kings() & self.by_color.get(color)
    }

    /// Returns the king position if there is exactly one king.
    pub fn king_pos_of(&self, color: Color) -> Option<Square> {
        self.king_of(color).single_square()
    }


    // ISSUE #1: Implement the `attackers` function.
    pub fn attackers() -> Bitboard {
        //Write your code here
        Bitboard::EMPTY // Temporary placeholder
    }

    /// Returns true if there is any attack on the square.
    pub fn attacks() -> bool {
         //Write your code here
         false // Temporary placeholder
    }

    // ISSUE #2: Implement the `slider_blockers` function.
    pub fn slider_blockers(&self, _our_king: Square, _us: Color) -> Bitboard {
        //Write your code here
        Bitboard::EMPTY // Temporary placeholder
    }

    /// Discards the piece on a given square.
    pub fn discard_by_square(&self, s: Square) -> Board {
        self.discard(s.bitboard())
    }

    /// Returns a new board with pieces in the provided mask removed.
    pub fn discard(&self, mask: Bitboard) -> Board {
        let not_mask = !mask;
        Board {
            occupied: self.occupied & not_mask,
            by_color: ByColor {
                white: self.by_color.white & not_mask,
                black: self.by_color.black & not_mask,
            },
            by_role: ByRole {
                pawn: self.by_role.pawn & not_mask,
                knight: self.by_role.knight & not_mask,
                bishop: self.by_role.bishop & not_mask,
                rook: self.by_role.rook & not_mask,
                queen: self.by_role.queen & not_mask,
                king: self.by_role.king & not_mask,
            },
        }
    }

    /// Returns the ByRole for pieces of a given color.
    pub fn by_role_of(&self, color: Color) -> ByRole {
        self.by_role.map(|b| b & self.by_color.get(color))
    }

    // ISSUE #3: Implement the `put` function.
    //
    // Description:
    // Write a function that places a piece on an empty square.
    pub fn put(&self, piece: Piece, at: Square) -> Option<Board> {
        if self.is_occupied_square(at) {
            None
        } else {
            Some(self.put_or_replace(piece, at))
        }
    }

    // ISSUE #4: Implement the `replace` function.
    //
    // Description:
    // Write a function that replaces the piece on an occupied square.
    pub fn replace(&self, piece: Piece, at: Square) -> Option<Board> {
        if self.is_occupied_square(at) {
            Some(self.put_or_replace(piece, at))
        } else {
            None
        }
    }

    /// Puts or replaces a piece on a square.
    pub fn put_or_replace(&self, piece: Piece, s: Square) -> Board {
        self.put_or_replace_details(s, piece.role, piece.color)
    }

    /// Helper for putting or replacing a piece.
    pub fn put_or_replace_details(&self, s: Square, role: Role, color: Color) -> Board {
        let b = self.discard_by_square(s);
        let m = s.bitboard();
        Board {
            occupied: b.occupied | m,
            by_color: b.by_color.update(color, |bb| bb | m),
            by_role: b.by_role.update(role, |bb| bb | m),
        }
    }

    /// Removes a piece from the board.
    pub fn take(&self, at: Square) -> Option<Board> {
        if self.is_occupied_square(at) {
            Some(self.discard_by_square(at))
        } else {
            None
        }
    }

    // ISSUE #3 Implement the `move_piece` function.
    //
    pub fn move_piece(&self, orig: Square, dest: Square) -> Option<Board> {
        // First check if the destination square is occupied
        if self.is_occupied_square(dest) {
            return None;
        }
        
        // Get the piece at the origin square
        let piece_opt = self.piece_at(orig);
        if piece_opt.is_none() {
            return None;
        }
        
        let piece = piece_opt.unwrap();
        let piece_color = piece.color;
        
        // Create a new board with the piece moved
        let new_board = self.discard_by_square(orig).put_or_replace(piece, dest);
        
        // Find our king's position
        let king_pos = new_board.king_pos_of(piece_color);
        if king_pos.is_none() {
            // If there's no king, just return the new board
            return Some(new_board);
        }
        
        let king_square = king_pos.unwrap();
        
        // Find all blockers between our king and attacking slider pieces
        let _blockers = Self::find_slider_blockers(&new_board, king_square, piece_color);
        
        // Store the blockers information somewhere or use it for move validation
        // For now, we'll just return the new board
        Some(new_board)
    }
    
    // Helper function to find slider blockers
    fn find_slider_blockers(board: &Board, our_king: Square, us: Color) -> Bitboard {
        let them = us.opposite();
        let mut blockers = Bitboard::EMPTY;
        
        // Get enemy bishops, rooks, and queens (all slider pieces)
        let enemy_bishops_and_queens = board.by_color.get(them) & (board.by_role.bishop | board.by_role.queen);
        let enemy_rooks_and_queens = board.by_color.get(them) & (board.by_role.rook | board.by_role.queen);
        
        // Get all pieces except our king
        let occupied_except_king = board.occupied ^ our_king.bitboard();
        
        // Check for potential bishop-like attackers (bishops and queens on diagonals)
        let mut potential_bishop_attackers = enemy_bishops_and_queens.0;
        while potential_bishop_attackers != 0 {
            // Get the square of the potential attacker
            let attacker_square = Square {
                value: potential_bishop_attackers.trailing_zeros() as u8,
            };
            
            // Check if the attacker is on the same diagonal or anti-diagonal as our king
            let king_file = our_king.value % 8;
            let king_rank = our_king.value / 8;
            let attacker_file = attacker_square.value % 8;
            let attacker_rank = attacker_square.value / 8;
            
            // Check if they're on the same diagonal or anti-diagonal
            let file_diff = (attacker_file as i8 - king_file as i8).abs();
            let rank_diff = (attacker_rank as i8 - king_rank as i8).abs();
            
            if file_diff == rank_diff {
                // They're on the same diagonal or anti-diagonal
                // Calculate the ray between them
                let mut ray = Bitboard::EMPTY;
                
                // Determine the direction
                let file_step = if attacker_file > king_file { 1 } else { -1 };
                let rank_step = if attacker_rank > king_rank { 1 } else { -1 };
                
                // Start from the king and move towards the attacker
                let mut current_file = king_file as i8 + file_step;
                let mut current_rank = king_rank as i8 + rank_step;
                
                // Add all squares between king and attacker to the ray
                while current_file >= 0 && current_file < 8 && 
                      current_rank >= 0 && current_rank < 8 && 
                      (current_file != attacker_file as i8 || current_rank != attacker_rank as i8) {
                    let square = Square {
                        value: ((current_rank as u8) * 8 + (current_file as u8)) as u8,
                    };
                    ray = ray | square.bitboard();
                    
                    current_file += file_step;
                    current_rank += rank_step;
                }
                
                // Check if there's exactly one piece on the ray
                let pieces_on_ray = ray & occupied_except_king;
                if pieces_on_ray.count() == 1 {
                    // There's exactly one blocker
                    let blocker = pieces_on_ray & board.by_color.get(us);
                    if blocker.0 != 0 {
                        // The blocker is our piece, so it's pinned
                        blockers = blockers | blocker;
                    }
                }
            }
            
            // Clear the least significant bit
            potential_bishop_attackers &= potential_bishop_attackers - 1;
        }
        
        // Check for potential rook-like attackers (rooks and queens on ranks/files)
        let mut potential_rook_attackers = enemy_rooks_and_queens.0;
        while potential_rook_attackers != 0 {
            // Get the square of the potential attacker
            let attacker_square = Square {
                value: potential_rook_attackers.trailing_zeros() as u8,
            };
            
            // Check if the attacker is on the same file or rank as our king
            let king_file = our_king.value % 8;
            let king_rank = our_king.value / 8;
            let attacker_file = attacker_square.value % 8;
            let attacker_rank = attacker_square.value / 8;
            
            // Check if they're on the same file or rank
            if king_file == attacker_file || king_rank == attacker_rank {
                // They're on the same file or rank
                // Calculate the ray between them
                let mut ray = Bitboard::EMPTY;
                
                if king_file == attacker_file {
                    // Same file
                    let rank_step = if attacker_rank > king_rank { 1 } else { -1 };
                    let mut current_rank = king_rank as i8 + rank_step;
                    
                    // Add all squares between king and attacker to the ray
                    while current_rank >= 0 && current_rank < 8 && current_rank != attacker_rank as i8 {
                        let square = Square {
                            value: ((current_rank as u8) * 8 + (king_file as u8)) as u8,
                        };
                        ray = ray | square.bitboard();
                        
                        current_rank += rank_step;
                    }
                } else {
                    // Same rank
                    let file_step = if attacker_file > king_file { 1 } else { -1 };
                    let mut current_file = king_file as i8 + file_step;
                    
                    // Add all squares between king and attacker to the ray
                    while current_file >= 0 && current_file < 8 && current_file != attacker_file as i8 {
                        let square = Square {
                            value: ((king_rank as u8) * 8 + (current_file as u8)) as u8,
                        };
                        ray = ray | square.bitboard();
                        
                        current_file += file_step;
                    }
                }
                
                // Check if there's exactly one piece on the ray
                let pieces_on_ray = ray & occupied_except_king;
                if pieces_on_ray.count() == 1 {
                    // There's exactly one blocker
                    let blocker = pieces_on_ray & board.by_color.get(us);
                    if blocker.0 != 0 {
                        // The blocker is our piece, so it's pinned
                        blockers = blockers | blocker;
                    }
                }
            }
            
            // Clear the least significant bit
            potential_rook_attackers &= potential_rook_attackers - 1;
        }
        
        blockers
    }

    // Implement the `taking` function.
    pub fn taking() -> Option<Board> {
        //Write your code here
        None // Temporary placeholder
    }

    /// Promotes a pawn.
    pub fn promote(&self, orig: Square, dest: Square, piece: Piece) -> Option<Board> {
        self.take(orig).map(|b| b.put_or_replace(piece, dest))
    }

    /// Returns true if the board has a piece matching the given piece.
    pub fn is_occupied_piece(&self, piece: Piece) -> bool {
        self.piece(piece).0 != 0
    }

    /// Returns the bitboard for the given piece.
    pub fn piece(&self, piece: Piece) -> Bitboard {
        self.by_color.get(piece.color) & self.by_role.get(piece.role)
    }

    // ISSUE #5: Implement the `piece_map` function.
    pub fn piece_map(&self) -> HashMap<Square, Piece> {
        let mut map_of_pieces = HashMap::new();
        
        for square in self.occupied.to_squares() {
            if let Some(piece) = self.piece_at(square) {
                map_of_pieces.insert(square, piece);
            }
        }
        
        map_of_pieces
    }

    /// Returns a mapping of pieces for a given color.
    pub fn pieces_of(&self, color: Color) -> HashMap<Square, Piece> {
        self.piece_map()
            .into_iter()
            .filter(|(_, p)| p.color == color)
            .collect()
    }

    /// Returns a list of pieces on the board.
    pub fn pieces(&self) -> Vec<Piece> {
        self.occupied
            .to_squares()
            .into_iter()
            .filter_map(|s| self.piece_at(s))
            .collect()
    }

    /// Returns the bitboard of pieces for the given color.
    pub fn color(&self, color: Color) -> Bitboard {
        self.by_color.get(color)
    }
}

