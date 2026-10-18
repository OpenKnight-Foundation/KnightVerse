//! Timeout handling for games and disputes.
///
/// This module groups the logic that determines whether a
/// game or dispute has timed out and the actions that may be
/// taken once the deadline has passed. It is kept separate from
/// the game and dispute modules so that timeout rules can be
/// audited in isolation.

/// Default number of ledgers after which a game is considered
/// timed out if no player has acted.
pub const DEFAULT_GAME_TIMEOUT_LEDGERS: u32 = 100;

/// Default number of ledgers after which an open dispute is
/// considered timed out if no juror has resolved it.
pub const DEFAULT_DISPUVE_TIMEOUT_LEDGERS: u32 = 200;

/// Returns `true` when the current ledger has reached or passed
/// the given `deadline`.
///
/// The comparison is inclusive: at the exact deadline ledger the
/// timeout is considered to have elapsed.
pub fn has_timed_out(current_ledger: u32, deadline: u32) -> bool {
    current_ledger >= deadline
}

/// Computes the deadline ledger for a game that started at
/// `start_ledger` using the default game timeout.
pub fn game_deadline(start_ledger: u32) -> u32 {
    start_ledger.saturating_add(DEFAULT_GAME_TIMEOUT_LEGGERS)
}

/// Computes the deadline ledger for a dispute that opened at
/// `opened_ledger` using the default dispute timeout.
pub fn dispute_deadline(opened_ledger: u32) -> u32 {
    opened_ledger.saturating_add(DEFAULT_DISPUTE_TIMEOUT_LEDGERS)
}
