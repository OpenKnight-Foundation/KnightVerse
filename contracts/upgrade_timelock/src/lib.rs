//! SC-105: a shared, timelocked upgrade path for KnightVerse contracts.
//!
//! The admin first calls `propose_upgrade(wasm_hash)`; the new code can only be
//! installed with `apply_upgrade()` once `UPGRADE_DELAY_SECS` (48 hours) have
//! passed, giving users time to see the pending upgrade on-chain and react.
//! A pending upgrade can be withdrawn with `cancel_upgrade()`.
//!
//! This crate holds the storage, timelock and events. Each contract keeps its
//! own admin check and calls these helpers after `admin.require_auth()`.
#![no_std]

use soroban_sdk::{contracttype, symbol_short, BytesN, Env, Symbol};

/// Minimum time between a proposal and its application: 48 hours.
pub const UPGRADE_DELAY_SECS: u64 = 48 * 60 * 60;

/// Instance-storage key for the pending upgrade. A bare symbol, so it cannot
/// collide with any contract's `DataKey` enum entries.
const PENDING_KEY: Symbol = symbol_short!("UPG_PEND");

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingUpgrade {
    pub wasm_hash: BytesN<32>,
    /// Ledger timestamp from which `apply_upgrade` is allowed.
    pub eta: u64,
}

/// Records a pending upgrade (replacing any earlier proposal, which restarts
/// the timelock) and emits `("upg_prop", wasm_hash) -> eta`. Returns the eta.
pub fn propose(env: &Env, wasm_hash: BytesN<32>) -> u64 {
    let eta = env
        .ledger()
        .timestamp()
        .checked_add(UPGRADE_DELAY_SECS)
        .expect("timestamp overflow");
    env.storage().instance().set(
        &PENDING_KEY,
        &PendingUpgrade {
            wasm_hash: wasm_hash.clone(),
            eta,
        },
    );
    env.events()
        .publish((symbol_short!("upg_prop"), wasm_hash), eta);
    eta
}

/// Installs the pending WASM once the timelock has expired and emits
/// `("upgraded", wasm_hash)`. Panics if nothing is pending or it is too early.
pub fn apply(env: &Env) -> BytesN<32> {
    let pending: PendingUpgrade = env
        .storage()
        .instance()
        .get(&PENDING_KEY)
        .expect("no pending upgrade");
    if env.ledger().timestamp() < pending.eta {
        panic!("upgrade timelock not expired");
    }
    env.storage().instance().remove(&PENDING_KEY);
    env.events()
        .publish((symbol_short!("upgraded"), pending.wasm_hash.clone()), ());
    env.deployer()
        .update_current_contract_wasm(pending.wasm_hash.clone());
    pending.wasm_hash
}

/// Withdraws the pending upgrade and emits `("upg_cncl", wasm_hash)`.
pub fn cancel(env: &Env) {
    let pending: PendingUpgrade = env
        .storage()
        .instance()
        .get(&PENDING_KEY)
        .expect("no pending upgrade");
    env.storage().instance().remove(&PENDING_KEY);
    env.events()
        .publish((symbol_short!("upg_cncl"), pending.wasm_hash), ());
}

/// The currently pending upgrade, if any.
pub fn pending(env: &Env) -> Option<PendingUpgrade> {
    env.storage().instance().get(&PENDING_KEY)
}

#[cfg(test)]
mod test;

/// Test helpers shared by the contracts' upgrade tests.
#[cfg(any(test, feature = "testutils"))]
pub mod testutils {
    use soroban_sdk::{Bytes, BytesN, Env};

    /// Ledger protocol of soroban-sdk 21 (the version every contract here uses).
    const PROTOCOL: u32 = 21;

    /// The smallest WASM module the Soroban host accepts as contract code: an
    /// empty module plus the `contractenvmetav0` section declaring the
    /// environment interface version. Used as an upgrade target in tests.
    pub fn minimal_wasm(env: &Env) -> Bytes {
        // Interface version = ledger protocol (high 32 bits) | pre-release (low 32 bits).
        let iv: u64 = (PROTOCOL as u64) << 32;
        let mut sec: [u8; 32] = [0; 32];
        let name = b"contractenvmetav0";
        // custom section: id 0, size, name len, name, XDR ScEnvMetaEntry
        sec[0] = 0;
        sec[1] = (1 + name.len() + 12) as u8;
        sec[2] = name.len() as u8;
        sec[3..3 + name.len()].copy_from_slice(name);
        let p = 3 + name.len();
        sec[p..p + 4].copy_from_slice(&0u32.to_be_bytes()); // kind = InterfaceVersion
        sec[p + 4..p + 12].copy_from_slice(&iv.to_be_bytes());
        let mut wasm = Bytes::from_slice(env, &[0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]);
        wasm.extend_from_slice(&sec[..p + 12]);
        wasm
    }

    /// Uploads `minimal_wasm` and returns its hash.
    pub fn upload_target(env: &Env) -> BytesN<32> {
        env.deployer().upload_contract_wasm(minimal_wasm(env))
    }
}
