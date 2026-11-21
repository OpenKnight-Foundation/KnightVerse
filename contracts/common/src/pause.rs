//! Pause/unpause helpers, including the circuit-breaker guard.
//!
//! `pause` and `unpause` authenticate and authorize the caller through
//! [`crate::admin::require_admin`]; contract entry points should simply
//! delegate to them.

use crate::{admin, events, CommonKey};
use soroban_sdk::{panic_with_error, Address, Env, Error};

/// Current pause flag. Defaults to `false` before initialization.
pub fn is_paused(env: &Env) -> bool {
    env.storage()
        .instance()
        .get(&CommonKey::Paused)
        .unwrap_or(false)
}

/// Set the initial pause flag to `false`. Call from `initialize`.
pub fn initialize(env: &Env) {
    env.storage().instance().set(&CommonKey::Paused, &false);
}

/// Admin-only pause, with the canonical `paused` event.
pub fn pause<E1, E2>(env: &Env, caller: &Address, not_admin: E1, already_paused: E2)
where
    E1: Into<Error>,
    E2: Into<Error>,
{
    admin::require_admin(env, caller, not_admin);
    if is_paused(env) {
        panic_with_error!(env, already_paused);
    }
    env.storage().instance().set(&CommonKey::Paused, &true);
    events::paused(env, caller);
}

/// Admin-only unpause, with the canonical `unpaused` event.
pub fn unpause<E1, E2>(env: &Env, caller: &Address, not_admin: E1, not_paused: E2)
where
    E1: Into<Error>,
    E2: Into<Error>,
{
    admin::require_admin(env, caller, not_admin);
    if !is_paused(env) {
        panic_with_error!(env, not_paused);
    }
    env.storage().instance().set(&CommonKey::Paused, &false);
    events::unpaused(env, caller);
}

/// Circuit-breaker guard for state-mutating entry points.
pub fn check_not_paused<E>(env: &Env, contract_paused: E)
where
    E: Into<Error>,
{
    if is_paused(env) {
        panic_with_error!(env, contract_paused);
    }
}
