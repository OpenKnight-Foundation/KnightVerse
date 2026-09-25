//! Unit tests for the TitleBadge registry.

use super::*;
use soroban_sdk::{testutils::Address as _, symbol_short, Address, Env};

fn setup() -> (Env, TitleBadgeContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, TitleBadgeContract);
    let client = TitleBadgeContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.init(&admin);

    (env, client, admin)
}

#[test]
fn test_init_sets_admin() {
    let (_env, client, admin) = setup();
    assert_eq!(client.get_admin(), admin);
}

#[test]
#[should_panic(expected = "Error(Contract, #1)")]
fn test_double_initialization_panics() {
    let (_env, client, admin) = setup();
    client.init(&admin);
}

#[test]
fn test_grant_stores_active_badge() {
    let (env, client, admin) = setup();
    let player = Address::generate(&env);

    client.grant(&admin, &player, &symbol_short!("gm"));

    let badge = client.get(&player).expect("badge should exist");
    assert_eq!(badge.title, symbol_short!("gm"));
    assert!(badge.active);
    assert!(client.is_verified(&player));
}

#[test]
fn test_verify_is_an_alias_for_grant() {
    let (env, client, admin) = setup();
    let player = Address::generate(&env);

    client.verify(&admin, &player, &symbol_short!("im"));

    assert_eq!(client.get(&player).unwrap().title, symbol_short!("im"));
}

#[test]
fn test_grant_replaces_previous_title() {
    let (env, client, admin) = setup();
    let player = Address::generate(&env);

    client.grant(&admin, &player, &symbol_short!("fm"));
    client.grant(&admin, &player, &symbol_short!("gm"));

    let badge = client.get(&player).unwrap();
    assert_eq!(badge.title, symbol_short!("gm"));
    assert!(badge.active);
}

#[test]
fn test_get_returns_none_for_unknown_player() {
    let (env, client, _admin) = setup();
    let player = Address::generate(&env);

    assert!(client.get(&player).is_none());
    assert!(!client.is_verified(&player));
}

#[test]
fn test_revoke_by_admin_deactivates_badge() {
    let (env, client, admin) = setup();
    let player = Address::generate(&env);

    client.grant(&admin, &player, &symbol_short!("gm"));
    client.revoke(&admin, &player);

    let badge = client.get(&player).expect("record is kept");
    assert!(!badge.active);
    assert!(!client.is_verified(&player));
}

#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn test_revoke_rejected_for_non_admin() {
    let (env, client, admin) = setup();
    let player = Address::generate(&env);
    let non_admin = Address::generate(&env);

    client.grant(&admin, &player, &symbol_short!("gm"));
    client.revoke(&non_admin, &player);
}

#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn test_grant_rejected_for_non_admin() {
    let (env, client, _admin) = setup();
    let player = Address::generate(&env);
    let non_admin = Address::generate(&env);

    client.grant(&non_admin, &player, &symbol_short!("gm"));
}

#[test]
#[should_panic(expected = "Error(Contract, #4)")]
fn test_revoke_unknown_player_panics() {
    let (env, client, admin) = setup();
    let player = Address::generate(&env);

    client.revoke(&admin, &player);
}

#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn test_revoke_twice_panics() {
    let (env, client, admin) = setup();
    let player = Address::generate(&env);

    client.grant(&admin, &player, &symbol_short!("gm"));
    client.revoke(&admin, &player);
    client.revoke(&admin, &player);
}

#[test]
fn test_grant_after_revoke_reactivates_badge() {
    let (env, client, admin) = setup();
    let player = Address::generate(&env);

    client.grant(&admin, &player, &symbol_short!("gm"));
    client.revoke(&admin, &player);
    client.grant(&admin, &player, &symbol_short!("gm"));

    assert!(client.is_verified(&player));
}
