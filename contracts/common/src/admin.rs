//! Admin storage and authorization helpers.
//!
//! `require_admin` both authenticates the caller and checks the stored admin,
//! so a contract cannot accidentally accept an unauthenticated admin address.

use crate::{CommonError, CommonKey};
use soroban_sdk::{panic_with_error, Address, Env, Error};

/// Whether an admin has been configured.
pub fn has(env: &Env) -> bool {
    env.storage().instance().has(&CommonKey::Admin)
}

/// Read the configured admin, if initialization has happened.
pub fn get(env: &Env) -> Option<Address> {
    env.storage().instance().get(&CommonKey::Admin)
}

/// Read the configured admin, panicking with `not_initialized` if unset.
pub fn get_required<E>(env: &Env, not_initialized: E) -> Address
where
    E: Into<Error>,
{
    get(env).unwrap_or_else(|| panic_with_error!(env, not_initialized))
}

/// Store `admin` as the contract admin.
pub fn set(env: &Env, admin: &Address) {
    env.storage().instance().set(&CommonKey::Admin, admin);
}

/// One-time initialization guard and admin write.
pub fn initialize<E>(env: &Env, admin: &Address, already_initialized: E)
where
    E: Into<Error>,
{
    if has(env) {
        panic_with_error!(env, already_initialized);
    }
    set(env, admin);
}

/// Require that `caller` has authenticated and is the current admin.
pub fn require_admin<E>(env: &Env, caller: &Address, not_admin: E)
where
    E: Into<Error>,
{
    caller.require_auth();
    let current = get_required(env, CommonError::NotInitialized);
    if current != *caller {
        panic_with_error!(env, not_admin);
    }
}
