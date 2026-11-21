#![cfg(test)]
extern crate std;

use crate::{testutils::upload_target, UPGRADE_DELAY_SECS};
use soroban_sdk::{
    contract, contractimpl,
    testutils::{Events, Ledger},
    BytesN, Env,
};

#[contract]
struct Host;

#[contractimpl]
impl Host {
    pub fn propose(env: Env, wasm_hash: BytesN<32>) -> u64 {
        crate::propose(&env, wasm_hash)
    }
    pub fn apply(env: Env) -> BytesN<32> {
        crate::apply(&env)
    }
    pub fn cancel(env: Env) {
        crate::cancel(&env)
    }
    pub fn pending(env: Env) -> Option<crate::PendingUpgrade> {
        crate::pending(&env)
    }
}

fn setup() -> (Env, HostClient<'static>) {
    let env = Env::default();
    env.ledger().with_mut(|l| l.timestamp = 1_000);
    let id = env.register_contract(None, Host);
    let client = HostClient::new(&env, &id);
    (env, client)
}

#[test]
fn propose_sets_eta_48h_ahead_and_emits_event() {
    let (env, c) = setup();
    let hash = upload_target(&env);
    let eta = c.propose(&hash);
    assert_eq!(eta, 1_000 + UPGRADE_DELAY_SECS);
    assert_eq!(c.pending().unwrap().wasm_hash, hash);
    assert_eq!(env.events().all().len(), 1);
}

#[test]
fn apply_before_timelock_fails_and_keeps_pending() {
    let (env, c) = setup();
    let hash = upload_target(&env);
    c.propose(&hash);
    assert!(c.try_apply().is_err());
    env.ledger()
        .with_mut(|l| l.timestamp = 1_000 + UPGRADE_DELAY_SECS - 1);
    assert!(c.try_apply().is_err());
    assert!(c.pending().is_some());
}

#[test]
fn apply_after_timelock_installs_wasm_and_clears_pending() {
    let (env, c) = setup();
    let hash = upload_target(&env);
    c.propose(&hash);
    env.ledger()
        .with_mut(|l| l.timestamp = 1_000 + UPGRADE_DELAY_SECS);
    assert_eq!(c.apply(), hash);
    // the contract now runs the (empty) target module, so its old functions are gone
    assert!(c.try_pending().is_err());
}

#[test]
fn apply_without_proposal_fails() {
    let (_env, c) = setup();
    assert!(c.try_apply().is_err());
}

#[test]
fn reproposal_restarts_the_timelock() {
    let (env, c) = setup();
    let hash = upload_target(&env);
    c.propose(&hash);
    env.ledger()
        .with_mut(|l| l.timestamp = 1_000 + UPGRADE_DELAY_SECS - 10);
    let eta = c.propose(&hash);
    assert_eq!(eta, 1_000 + 2 * UPGRADE_DELAY_SECS - 10);
    env.ledger()
        .with_mut(|l| l.timestamp = 1_000 + UPGRADE_DELAY_SECS);
    assert!(c.try_apply().is_err());
}

#[test]
fn cancel_removes_pending_upgrade() {
    let (env, c) = setup();
    let hash = upload_target(&env);
    c.propose(&hash);
    c.cancel();
    assert!(c.pending().is_none());
    env.ledger()
        .with_mut(|l| l.timestamp = 1_000 + UPGRADE_DELAY_SECS);
    assert!(c.try_apply().is_err());
    assert!(c.try_cancel().is_err());
}
