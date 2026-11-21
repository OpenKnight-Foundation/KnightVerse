//! Mandatory draw detection (FIDE Article 5.2.2) and rating-related draw helpers.
//!
//! This module is intentionally self-contained and operates on a position
//! summary rather than the full board representation so it can be used both
//! by the game engine and by rating/result post-processing.

/// Material summary of one side, counted by piece type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MaterialCount {
    pub pawns: u8,
    pub knights: u8,
    pub bishops_light: u8,
    pub bishops_dark: u8,
    pub rooks: u8,
    pub queens: u8,
}

impl MaterialCount {
    /// True if this side has any pawn, rook or queen (sufficient mating material
    /// in combination with a king, barring pathological blockades).
    pub fn has_major_or_pawn(&self) -> bool {
        self.pawns > 0 || self.rooks > 0 || self.queens > 0
    }

    /// Total number of minor pieces (knights and bishops).
    pub fn minor_count(&self) -> u8 {
        self.knights + self.bishops_light + self.bishops_dark
    }
}

/// Returns true if the position is a dead position under FIDE rules: neither
/// side can possibly checkmate the other by any sequence of legal moves.
///
/// Covered cases:
/// - King vs king
/// - King and a single minor piece vs king
/// - King and bishop(s) vs king and bishop(s) where all bishops occupy
///   squares of the same colour
/// - King and knight vs king and knight (single knight each side)
pub fn is_dead_position(white: &MaterialCount, black: &MaterialCount) -> bool {
    if white.has_major_or_pawn() || black.has_major_or_pawn() {
        return false;
    }

    let w_minor = white.minor_count();
    let b_minor = black.minor_count();

    // King vs king.
    if w_minor == 0 && b_minor == 0 {
        return true;
    }

    // King + single minor vs king.
    if (w_minor == 1 && b_minor == 0) || (w_minor == 0 && b_minor == 1) {
        return true;
    }

    // Bishops only, all on the same colour complex.
    let only_bishops = white.knights == 0 && black.knights == 0;
    if only_bishops {
        let any_light = white.bishops_light > 0 || black.bishops_light > 0;
        let any_dark = white.bishops_dark > 0 || black.bishops_dark > 0;
        if !(any_light && any_dark) {
            return true;
        }
    }

    // Single knight each side, no bishops.
    if white.knights == 1
        && black.knights == 1
        && white.bishops_light + white.bishops_dark == 0
        && black.bishops_light + black.bishops_dark == 0
    {
        return true;
    }

    false
}

/// Returns true if the game must be declared drawn because the 75-move rule
/// has been reached: 75 consecutive moves by each side without any pawn move
/// or capture. `plies_since_last_capture_or_pawn_move` counts half-moves.
///
/// Note: checkmate delivered on the 75th move takes precedence and is not a
/// draw; callers must check for checkmate before applying this rule.
pub fn is_seventy_five_move_draw(plies_since_last_capture_or_pawn_move: u32) -> bool {
    plies_since_last_capture_or_pawn_move >= 150
}

/// Returns true if the fivefold repetition rule applies: the same position
/// (with the same side to move and same castling/en-passant possibilities)
/// has occurred at least five times. Repetition count is supplied by the caller.
pub fn is_fivefold_repetition(occurrences: u32) -> bool {
    occurrences >= 5
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kings_only() -> MaterialCount {
        MaterialCount::default()
    }

    #[test]
    fn king_vs_king_is_dead() {
        assert!(is_dead_position(&kings_only(), &kings_only()));
    }

    #[test]
    fn king_and_knight_vs_king_is_dead() {
        let white = MaterialCount { knights: 1, ..Default::default() };
        assert!(is_dead_position(&white, &kings_only()));
    }

    #[test]
    fn king_and_bishop_vs_king_is_dead() {
        let black = MaterialCount { bishops_dark: 1, ..Default::default() };
        assert!(is_dead_position(&kings_only(), &black));
    }

    #[test]
    fn same_colour_bishops_are_dead() {
        let white = MaterialCount { bishops_light: 1, ..Default::default() };
        let black = MaterialCount { bishops_light: 1, ..Default::default() };
        assert!(is_dead_position(&white, &black));
    }

    #[test]
    fn opposite_colour_bishops_are_not_dead() {
        let white = MaterialCount { bishops_light: 1, ..Default::default() };
        let black = MaterialCount { bishops_dark: 1, ..Default::default() };
        assert!(!is_dead_position(&white, &black));
    }

    #[test]
    fn single_knight_each_side_is_dead() {
        let knight = MaterialCount { knights: 1, ..Default::default() };
        assert!(is_dead_position(&knight, &knight));
    }

    #[test]
    fn pawn_on_board_is_not_dead() {
        let white = MaterialCount { pawns: 1, ..Default::default() };
        assert!(!is_dead_position(&white, &kings_only()));
    }

    #[test]
    fn seventy_five_move_rule_threshold() {
        assert!(!is_seventy_five_move_draw(149));
        assert!(is_seventy_five_move_draw(150));
    }

    #[test]
    fn fivefold_repetition_threshold() {
        assert!(!is_fivefold_repetition(4));
        assert!(is_fivefold_repetition(5));
    }
}
