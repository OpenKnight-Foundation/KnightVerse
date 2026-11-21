//! Canonical event names and payloads for shared contract controls.
//!
//! All events use a single topic symbol and put the affected address (plus
//! any extra data) in the body. Contracts must not publish their own variants
//! of these events.

use soroban_sdk::{symbol_short, Address, Env};

/// Emitted by `pause`. Payload: the admin that paused the contract.
pub fn paused(env: &Env, admin: &Address) {
    env.events()
        .publish((symbol_short!("paused"),), admin.clone());
}

/// Emitted by `unpause`. Payload: the admin that unpaused the contract.
pub fn unpaused(env: &Env, admin: &Address) {
    env.events()
        .publish((symbol_short!("unpaused"),), admin.clone());
}

/// Emitted when a timelocked admin handoff is proposed.
/// Payload: `(candidate, effective_at)`.
pub fn admin_transfer_proposed(env: &Env, candidate: &Address, effective_at: u64) {
    env.events().publish(
        (symbol_short!("adm_prop"),),
        (candidate.clone(), effective_at),
    );
}

/// Emitted when a pending admin handoff is accepted.
/// Payload: the new admin address.
pub fn admin_transfer_accepted(env: &Env, candidate: &Address) {
    env.events()
        .publish((symbol_short!("adm_xfer"),), candidate.clone());
}

/// Emitted when a pending admin handoff is cancelled.
/// Payload: the admin that cancelled it.
pub fn admin_transfer_cancelled(env: &Env, admin: &Address) {
    env.events()
        .publish((symbol_short!("adm_cncl"),), admin.clone());
}
