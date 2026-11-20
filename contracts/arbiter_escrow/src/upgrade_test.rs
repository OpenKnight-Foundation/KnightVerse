#![cfg(test)]
//! SC-105: timelocked upgrade path.
extern crate std;

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger},
    Address, Env, IntoVal, Symbol,
};
use upgrade_timelock::{testutils::upload_target, UPGRADE_DELAY_SECS};

const T0: u64 = 1_700_000_000;

fn setup() -> (Env, ArbiterEscrowContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|l| l.timestamp = T0);
    let id = env.register_contract(None, ArbiterEscrowContract);
    let client = ArbiterEscrowContractClient::new(&env, &id);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    (env, client, admin)
}

#[test]
fn upgrade_applies_only_after_48h_timelock() {
    let (env, client, admin) = setup();
    let hash = upload_target(&env);

    assert_eq!(client.propose_upgrade(&hash), T0 + UPGRADE_DELAY_SECS);
    // the proposal was authorised by the stored admin
    assert_eq!(env.auths()[0].0, admin);
    let ev = env.events().all().last().unwrap();
    assert_eq!(
        ev.1,
        (Symbol::new(&env, "upg_prop"), hash.clone()).into_val(&env)
    );

    let p = client.pending_upgrade().unwrap();
    assert_eq!(
        (p.wasm_hash, p.eta),
        (hash.clone(), T0 + UPGRADE_DELAY_SECS)
    );

    // too early: immediately and one second before the timelock expires
    assert!(client.try_apply_upgrade().is_err());
    env.ledger()
        .with_mut(|l| l.timestamp = T0 + UPGRADE_DELAY_SECS - 1);
    assert!(client.try_apply_upgrade().is_err());
    assert!(client.pending_upgrade().is_some());

    env.ledger()
        .with_mut(|l| l.timestamp = T0 + UPGRADE_DELAY_SECS);
    client.apply_upgrade();
    assert_eq!(env.auths()[0].0, admin);
    let ev = env.events().all().last().unwrap();
    assert_eq!(ev.1, (Symbol::new(&env, "upgraded"), hash).into_val(&env));
    // the contract now runs the new (empty) code: its old entry points are gone
    assert!(client.try_pending_upgrade().is_err());
}

#[test]
fn unauthorised_callers_cannot_propose_apply_or_cancel() {
    let (env, client, _admin) = setup();
    let hash = upload_target(&env);
    env.set_auths(&[]); // no signatures from here on
    assert!(client.try_propose_upgrade(&hash).is_err());
    assert!(client.pending_upgrade().is_none());

    env.mock_all_auths();
    client.propose_upgrade(&hash);
    env.ledger()
        .with_mut(|l| l.timestamp = T0 + UPGRADE_DELAY_SECS);
    env.set_auths(&[]);
    assert!(client.try_apply_upgrade().is_err());
    assert!(client.try_cancel_upgrade().is_err());
    assert!(client.pending_upgrade().is_some());
}

#[test]
fn cancelled_upgrade_cannot_be_applied() {
    let (env, client, _admin) = setup();
    let hash = upload_target(&env);
    client.propose_upgrade(&hash);
    client.cancel_upgrade();
    assert!(client.pending_upgrade().is_none());
    env.ledger()
        .with_mut(|l| l.timestamp = T0 + UPGRADE_DELAY_SECS);
    assert!(client.try_apply_upgrade().is_err());
}

#[test]
fn no_upgrade_before_initialisation() {
    let env = Env::default();
    env.mock_all_auths();
    let id = env.register_contract(None, ArbiterEscrowContract);
    let client = ArbiterEscrowContractClient::new(&env, &id);
    let hash = upload_target(&env);
    assert!(client.try_propose_upgrade(&hash).is_err());
}
