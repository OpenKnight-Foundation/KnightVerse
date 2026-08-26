use std::collections::HashMap;

/// FIDE 75-move rule threshold: 150 half-moves (75 full moves) without a pawn move or capture
const HALFMOVE_CLOCK_THRESHOLD: u32 = 150;

/// FIDE 5-fold repetition threshold
const REPETITION_THRESHOLD: u32 = 5;

/// Represents the result of a mandatory draw check
#[derive(Debug, Clone, PartialEq)]
pub enum MandatoryDrawResult {
    /// No mandatory draw condition met
    NoDraw,
    /// 75-move rule triggered
    SeventyFiveMoveRule { halfmove_clock: u32 },
    /// 5-fold repetition triggered
    FivefoldRepetition { position_hash: String, count: u32 },
}

/// Tracks position history for repetition detection
#[derive(Debug, Clone)]
pub struct PositionTracker {
    /// Map of position hash -> count of occurrences
    position_counts: HashMap<String, u32>,
    /// Current half-move clock (resets on pawn move or capture)
    halfmove_clock: u32,
}

impl PositionTracker {
    pub fn new() -> Self {
        Self {
            position_counts: HashMap::new(),
            halfmove_clock: 0,
        }
    }

    /// Record a position and check for repetition
    /// Returns the count of times this position has occurred
    pub fn record_position(&mut self, position_hash: &str) -> u32 {
        let count = self.position_counts
            .entry(position_hash.to_string())
            .and_modify(|c| *c += 1)
            .or_insert(1);
        *count
    }

    /// Increment the half-move clock (called after each move)
    pub fn increment_halfmove_clock(&mut self) {
        self.halfmove_clock += 1;
    }

    /// Reset the half-move clock (called after pawn move or capture)
    pub fn reset_halfmove_clock(&mut self) {
        self.halfmove_clock = 0;
    }

    /// Get current half-move clock
    pub fn halfmove_clock(&self) -> u32 {
        self.halfmove_clock
    }

    /// Check if a position has occurred the threshold number of times
    pub fn check_repetition(&self, position_hash: &str) -> Option<u32> {
        self.position_counts.get(position_hash).copied()
    }
}

/// Check for mandatory draw conditions (FIDE 75-move rule and 5-fold repetition)
///
/// This function should be called after every validated move in `apply_move()`.
/// Returns `MandatoryDrawResult::NoDraw` if no draw condition is met, otherwise
/// returns the specific draw condition that was triggered.
pub fn check_mandatory_draw_conditions(
    position_tracker: &PositionTracker,
    position_hash: &str,
    is_pawn_move: bool,
    is_capture: bool,
) -> MandatoryDrawResult {
    // Check 75-move rule (150 half-moves without pawn move or capture)
    if position_tracker.halfmove_clock() >= HALFMOVE_CLOCK_THRESHOLD {
        return MandatoryDrawResult::SeventyFiveMoveRule {
            halfmove_clock: position_tracker.halfmove_clock(),
        };
    }

    // Check 5-fold repetition
    if let Some(count) = position_tracker.check_repetition(position_hash) {
        if count >= REPETITION_THRESHOLD {
            return MandatoryDrawResult::FivefoldRepetition {
                position_hash: position_hash.to_string(),
                count,
            };
        }
    }

    MandatoryDrawResult::NoDraw
}

/// Update position tracker after a move
pub fn update_position_tracker(
    tracker: &mut PositionTracker,
    position_hash: &str,
    is_pawn_move: bool,
    is_capture: bool,
) -> MandatoryDrawResult {
    // Record the new position
    tracker.record_position(position_hash);

    // Update half-move clock
    if is_pawn_move || is_capture {
        tracker.reset_halfmove_clock();
    } else {
        tracker.increment_halfmove_clock();
    }

    // Check for mandatory draw conditions
    check_mandatory_draw_conditions(tracker, position_hash, is_pawn_move, is_capture)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_position_tracker_new() {
        let tracker = PositionTracker::new();
        assert_eq!(tracker.halfmove_clock(), 0);
        assert!(tracker.position_counts.is_empty());
    }

    #[test]
    fn test_record_position() {
        let mut tracker = PositionTracker::new();
        let count = tracker.record_position("pos1");
        assert_eq!(count, 1);

        let count = tracker.record_position("pos1");
        assert_eq!(count, 2);

        let count = tracker.record_position("pos2");
        assert_eq!(count, 1);
    }

    #[test]
    fn test_halfmove_clock() {
        let mut tracker = PositionTracker::new();

        // Normal move increments clock
        tracker.increment_halfmove_clock();
        assert_eq!(tracker.halfmove_clock(), 1);

        tracker.increment_halfmove_clock();
        assert_eq!(tracker.halfmove_clock(), 2);

        // Pawn move or capture resets clock
        tracker.reset_halfmove_clock();
        assert_eq!(tracker.halfmove_clock(), 0);
    }

    #[test]
    fn test_seventy_five_move_rule() {
        let mut tracker = PositionTracker::new();

        // Simulate 150 half-moves without pawn move or capture
        for _ in 0..150 {
            tracker.increment_halfmove_clock();
        }

        let result = check_mandatory_draw_conditions(&tracker, "pos1", false, false);
        assert_eq!(
            result,
            MandatoryDrawResult::SeventyFiveMoveRule {
                halfmove_clock: 150
            }
        );
    }

    #[test]
    fn test_seventy_five_move_rule_not_triggered() {
        let mut tracker = PositionTracker::new();

        // Simulate 149 half-moves
        for _ in 0..149 {
            tracker.increment_halfmove_clock();
        }

        let result = check_mandatory_draw_conditions(&tracker, "pos1", false, false);
        assert_eq!(result, MandatoryDrawResult::NoDraw);
    }

    #[test]
    fn test_fivefold_repetition() {
        let mut tracker = PositionTracker::new();

        // Record same position 5 times
        for _ in 0..5 {
            tracker.record_position("pos1");
        }

        let result = check_mandatory_draw_conditions(&tracker, "pos1", false, false);
        assert_eq!(
            result,
            MandatoryDrawResult::FivefoldRepetition {
                position_hash: "pos1".to_string(),
                count: 5
            }
        );
    }

    #[test]
    fn test_fivefold_repetition_not_triggered() {
        let mut tracker = PositionTracker::new();

        // Record same position 4 times
        for _ in 0..4 {
            tracker.record_position("pos1");
        }

        let result = check_mandatory_draw_conditions(&tracker, "pos1", false, false);
        assert_eq!(result, MandatoryDrawResult::NoDraw);
    }

    #[test]
    fn test_pawn_move_resets_halfmove_clock() {
        let mut tracker = PositionTracker::new();

        // Simulate some moves
        for _ in 0..50 {
            tracker.increment_halfmove_clock();
        }

        // Pawn move resets clock
        update_position_tracker(&mut tracker, "pos1", true, false);
        assert_eq!(tracker.halfmove_clock(), 0);
    }

    #[test]
    fn test_capture_resets_halfmove_clock() {
        let mut tracker = PositionTracker::new();

        // Simulate some moves
        for _ in 0..50 {
            tracker.increment_halfmove_clock();
        }

        // Capture resets clock
        update_position_tracker(&mut tracker, "pos1", false, true);
        assert_eq!(tracker.halfmove_clock(), 0);
    }

    #[test]
    fn test_update_tracker_with_pawn_move() {
        let mut tracker = PositionTracker::new();

        // Simulate 149 normal moves
        for _ in 0..149 {
            tracker.increment_halfmove_clock();
        }

        // 150th move is a pawn move - should not trigger 75-move rule
        let result = update_position_tracker(&mut tracker, "pos1", true, false);
        assert_eq!(result, MandatoryDrawResult::NoDraw);
        assert_eq!(tracker.halfmove_clock(), 0);
    }

    #[test]
    fn test_update_tracker_with_capture() {
        let mut tracker = PositionTracker::new();

        // Simulate 149 normal moves
        for _ in 0..149 {
            tracker.increment_halfmove_clock();
        }

        // 150th move is a capture - should not trigger 75-move rule
        let result = update_position_tracker(&mut tracker, "pos1", false, true);
        assert_eq!(result, MandatoryDrawResult::NoDraw);
        assert_eq!(tracker.halfmove_clock(), 0);
    }
}
