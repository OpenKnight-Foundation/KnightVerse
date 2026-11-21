//! Canonical error codes for shared admin, pause and admin-transfer behaviour.
//!
//! Keep these codes stable: they are part of the contract ABI. Contracts that
//! cannot change their published codes yet can pass their own error variants
//! to the helpers in this crate instead.

use soroban_sdk::contracterror;

/// Standard errors shared by every KnightVerse contract.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum CommonError {
    /// `initialize` was called more than once.
    AlreadyInitialized = 1,
    /// A privileged operation was attempted before initialization.
    NotInitialized = 2,
    /// Caller is not the contract admin.
    NotAdmin = 3,
    /// A state-mutating operation was attempted while paused.
    ContractPaused = 4,
    /// `pause` was called while already paused.
    AlreadyPaused = 5,
    /// `unpause` was called while not paused.
    NotPaused = 6,
    /// No admin-transfer proposal is pending.
    NoPendingAdminTransfer = 7,
    /// The admin-transfer timelock has not elapsed yet.
    TimelockNotElapsed = 8,
    /// The accepting address is not the pending admin candidate.
    InvalidAdminCandidate = 9,
}
