#![no_std]
//! Shared primitives for KnightVerse Soroban contracts.
//!
//! Every contract used to re-implement admin checks, pause flags, admin
//! handoffs and TTL bumps. That produced different errors and event names for
//! the same concept. This crate is the single place where those conventions
//! live. See `contracts/README.md` for the full convention table.
//!
//! # Storage
//!
//! [`CommonKey`] holds the canonical instance-storage keys. Unit enum variants
//! are serialized by Soroban as their name (`Admin`, `Paused`, `PendingAdmin`),
//! so contracts migrating from a local `DataKey` with the same variants keep
//! reading their existing on-chain state.
//!
//! # Errors and events
//!
//! [`CommonError`] defines the canonical numeric codes, and [`events`] defines
//! the canonical topic/data shapes. Helpers take the error value to raise so
//! an existing contract can keep its published error codes while migrating
//! incrementally; new contracts should pass [`CommonError`] variants.

use soroban_sdk::contracttype;

pub mod admin;
pub mod error;
pub mod events;
pub mod pause;
pub mod ttl;
pub mod two_step;

#[cfg(test)]
mod test;

pub use error::CommonError;
pub use soroban_sdk::Error;

/// Canonical instance-storage keys used by the shared helpers.
#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum CommonKey {
    /// Address allowed to perform privileged operations.
    Admin,
    /// Whether state-mutating operations are currently blocked.
    Paused,
    /// Timelocked admin handoff proposed by the current admin.
    PendingAdmin,
}
