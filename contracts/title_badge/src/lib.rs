#![no_std]
//! Title Badge — an admin-gated registry of player titles/badges.
//!
//! A single admin (`init`) is the only address allowed to grant or revoke a
//! title. Titles are stored per player and readable via `get`. Re-granting a
//! player replaces their previous title; revoking marks the badge inactive
//! while keeping the record so history is not silently lost.

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, panic_with_error, symbol_short, Address,
    Env, Symbol,
};

/// Storage keys for the contract.
#[contracttype]
pub enum DataKey {
    /// The admin address allowed to grant and revoke titles.
    Admin,
    /// A player's badge record.
    Badge(Address),
}

/// A player's title badge.
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TitleBadge {
    /// Short title/rank symbol, e.g. `gm`, `im`, `fm`.
    pub title: Symbol,
    /// Ledger sequence at which the title was granted or last updated.
    pub granted_at: u64,
    /// False once an admin revokes the title.
    pub active: bool,
}

/// Structured error codes for the TitleBadge contract.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    /// Contract has already been initialized.
    AlreadyInitialized = 1,
    /// Contract has not been initialized.
    NotInitialized = 2,
    /// Caller is not the contract admin.
    NotAdmin = 3,
    /// Player has no badge record.
    BadgeNotFound = 4,
    /// Badge exists but is already revoked.
    AlreadyRevoked = 5,
}

#[contract]
pub struct TitleBadgeContract;

#[contractimpl]
impl TitleBadgeContract {
    /// Initialize the registry with a single admin. Can only be called once.
    pub fn init(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic_with_error!(&env, Error::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.events().publish((symbol_short!("init"),), admin);
    }

    /// Grant `player` the title `title`, replacing any previous title.
    ///
    /// Admin-gated. Emits a `grant` event.
    pub fn grant(env: Env, admin: Address, player: Address, title: Symbol) {
        admin.require_auth();
        Self::check_admin(&env, &admin);

        let badge = TitleBadge {
            title: title.clone(),
            granted_at: env.ledger().sequence() as u64,
            active: true,
        };
        env.storage()
            .persistent()
            .set(&DataKey::Badge(player.clone()), &badge);

        env.events()
            .publish((symbol_short!("grant"),), (player, title));
    }

    /// Alias for [`Self::grant`], kept for the original registry API.
    pub fn verify(env: Env, admin: Address, player: Address, title: Symbol) {
        Self::grant(env, admin, player, title);
    }

    /// Revoke `player`'s badge.
    ///
    /// Admin-gated. Panics if the player has no badge or theirs is already
    /// revoked, so a revocation cannot be silently repeated.
    pub fn revoke(env: Env, admin: Address, player: Address) {
        admin.require_auth();
        Self::check_admin(&env, &admin);

        let key = DataKey::Badge(player.clone());
        let mut badge: TitleBadge = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| panic_with_error!(&env, Error::BadgeNotFound));

        if !badge.active {
            panic_with_error!(&env, Error::AlreadyRevoked);
        }

        badge.active = false;
        env.storage().persistent().set(&key, &badge);

        env.events().publish((symbol_short!("revoke"),), player);
    }

    /// Return a player's badge record, if one exists.
    pub fn get(env: Env, player: Address) -> Option<TitleBadge> {
        env.storage().persistent().get(&DataKey::Badge(player))
    }

    /// Return whether a player currently holds an active badge.
    pub fn is_verified(env: Env, player: Address) -> bool {
        match Self::get(env.clone(), player) {
            Some(badge) => badge.active,
            None => false,
        }
    }

    /// Return the configured admin address.
    pub fn get_admin(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic_with_error!(&env, Error::NotInitialized))
    }

    /// Panic unless `caller` is the registered admin.
    fn check_admin(env: &Env, caller: &Address) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic_with_error!(env, Error::NotInitialized));
        if admin != *caller {
            panic_with_error!(env, Error::NotAdmin);
        }
    }
}

#[cfg(test)]
mod test;

// SC-105: timelocked upgrade path. The admin proposes new code with
// `propose_upgrade`; it can only be installed with `apply_upgrade` after
// `upgrade_timelock::UPGRADE_DELAY_SECS` (48 hours). Storage, timelock and
// events live in the shared `upgrade_timelock` crate.
#[contractimpl]
impl TitleBadgeContract {
    /// Admin only. Records `wasm_hash` as the pending upgrade (restarting the
    /// timelock if one was already pending) and returns the timestamp from
    /// which it can be applied. Emits `("upg_prop", wasm_hash) -> eta`.
    pub fn propose_upgrade(env: Env, wasm_hash: soroban_sdk::BytesN<32>) -> u64 {
        sc105_admin(&env).require_auth();
        upgrade_timelock::propose(&env, wasm_hash)
    }

    /// Admin only. Installs the pending code once the timelock has expired.
    /// Emits `("upgraded", wasm_hash)`.
    pub fn apply_upgrade(env: Env) {
        sc105_admin(&env).require_auth();
        upgrade_timelock::apply(&env);
    }

    /// Admin only. Withdraws the pending upgrade. Emits `("upg_cncl", wasm_hash)`.
    pub fn cancel_upgrade(env: Env) {
        sc105_admin(&env).require_auth();
        upgrade_timelock::cancel(&env);
    }

    /// The pending upgrade and the time it becomes applicable, if any.
    pub fn pending_upgrade(env: Env) -> Option<upgrade_timelock::PendingUpgrade> {
        upgrade_timelock::pending(&env)
    }
}

fn sc105_admin(env: &Env) -> Address {
    env.storage()
        .instance()
        .get(&DataKey::Admin)
        .expect("admin not set")
}

#[cfg(test)]
mod upgrade_test;
