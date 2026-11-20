//! Consistent TTL bumps for persistent storage and the contract instance.
//!
//! Contracts must use these helpers instead of calling `extend_ttl` with local
//! magic numbers. Read/write/has wrappers bump the TTL only when the entry
//! exists, so writes to brand-new keys are not charged an extra extension.

use core::fmt::Debug;
use soroban_sdk::{Env, IntoVal, TryFromVal, Val};

/// Extend an entry only once its remaining TTL drops below this threshold.
pub const DEFAULT_PERSISTENT_TTL_THRESHOLD: u32 = 100_000;
/// TTL assigned to a persistent entry when it is bumped.
pub const DEFAULT_PERSISTENT_TTL_EXTEND_TO: u32 = 500_000;
/// Instance/code TTL threshold used by [`extend_instance`].
pub const DEFAULT_INSTANCE_TTL_THRESHOLD: u32 = 100_000;
/// Instance/code TTL assigned by [`extend_instance`].
pub const DEFAULT_INSTANCE_TTL_EXTEND_TO: u32 = 500_000;

/// Bump the current contract's instance and code TTL with canonical values.
pub fn extend_instance(env: &Env) {
    extend_instance_with(
        env,
        DEFAULT_INSTANCE_TTL_THRESHOLD,
        DEFAULT_INSTANCE_TTL_EXTEND_TO,
    );
}

/// Bump the instance/code TTL with explicit values (tests/modern networks).
pub fn extend_instance_with(env: &Env, threshold: u32, extend_to: u32) {
    env.storage().instance().extend_ttl(threshold, extend_to);
}

/// Read a persistent entry and bump its TTL when present.
pub fn get_persistent<K, V>(env: &Env, key: &K) -> Option<V>
where
    K: IntoVal<Env, Val>,
    V: TryFromVal<Env, Val>,
    V::Error: Debug,
{
    get_persistent_with(
        env,
        key,
        DEFAULT_PERSISTENT_TTL_THRESHOLD,
        DEFAULT_PERSISTENT_TTL_EXTEND_TO,
    )
}

/// Read a persistent entry with explicit TTL values.
pub fn get_persistent_with<K, V>(env: &Env, key: &K, threshold: u32, extend_to: u32) -> Option<V>
where
    K: IntoVal<Env, Val>,
    V: TryFromVal<Env, Val>,
    V::Error: Debug,
{
    let value = env.storage().persistent().get(key);
    if value.is_some() {
        env.storage()
            .persistent()
            .extend_ttl(key, threshold, extend_to);
    }
    value
}

/// Whether a persistent entry exists; bumps its TTL when present.
pub fn has_persistent<K>(env: &Env, key: &K) -> bool
where
    K: IntoVal<Env, Val>,
{
    has_persistent_with(
        env,
        key,
        DEFAULT_PERSISTENT_TTL_THRESHOLD,
        DEFAULT_PERSISTENT_TTL_EXTEND_TO,
    )
}

/// Whether a persistent entry exists, with explicit TTL values.
pub fn has_persistent_with<K>(env: &Env, key: &K, threshold: u32, extend_to: u32) -> bool
where
    K: IntoVal<Env, Val>,
{
    let exists = env.storage().persistent().has(key);
    if exists {
        env.storage()
            .persistent()
            .extend_ttl(key, threshold, extend_to);
    }
    exists
}

/// Write a persistent entry and bump its TTL.
pub fn set_persistent<K, V>(env: &Env, key: &K, value: &V)
where
    K: IntoVal<Env, Val>,
    V: IntoVal<Env, Val>,
{
    set_persistent_with(
        env,
        key,
        value,
        DEFAULT_PERSISTENT_TTL_THRESHOLD,
        DEFAULT_PERSISTENT_TTL_EXTEND_TO,
    )
}

/// Write a persistent entry with explicit TTL values.
pub fn set_persistent_with<K, V>(env: &Env, key: &K, value: &V, threshold: u32, extend_to: u32)
where
    K: IntoVal<Env, Val>,
    V: IntoVal<Env, Val>,
{
    env.storage().persistent().set(key, value);
    env.storage()
        .persistent()
        .extend_ttl(key, threshold, extend_to);
}
