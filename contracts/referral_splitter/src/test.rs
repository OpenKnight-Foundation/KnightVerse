//! Unit tests for the referral splitter: commission accrual and the
//! on-chain `withdraw_earnings` payout.

use super::*;
use soroban_sdk::{
    testutils::Address as _,
    token::{Client as TokenClient, StellarAssetClient},
    Address, Env,
};

#[allow(dead_code)]
struct TestContext {
    env: Env,
    contract_id: Address,
    client: ReferralSplitterClient<'static>,
    admin: Address,
    referrer: Address,
    referee_a: Address,
    referee_b: Address,
    token: Address,
    token_client: TokenClient<'static>,
    token_admin: StellarAssetClient<'static>,
}

/// Deploys the splitter with a 10% (1000 bps) commission and a funded mock
/// token. The splitter itself is minted 1_000_000 stroops so it can honour
/// withdrawals, standing in for the wager pool it settles from.
fn setup() -> TestContext {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, ReferralSplitter);
    let client = ReferralSplitterClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let referrer = Address::generate(&env);
    let referee_a = Address::generate(&env);
    let referee_b = Address::generate(&env);

    client.initialize(&admin, &1_000u32);

    let token_admin_addr = Address::generate(&env);
    let token_contract = env.register_stellar_asset_contract_v2(token_admin_addr.clone());
    let token = token_contract.address();
    let token_client = TokenClient::new(&env, &token);
    let token_admin = StellarAssetClient::new(&env, &token);

    token_admin.mint(&contract_id, &1_000_000);

    TestContext {
        env,
        contract_id,
        client,
        admin,
        referrer,
        referee_a,
        referee_b,
        token,
        token_client,
        token_admin,
    }
}

#[test]
fn test_initialize_sets_admin_and_commission() {
    let ctx = setup();
    assert_eq!(ctx.client.get_commission_bps(), 1_000);
}

#[test]
fn test_earnings_accrue_across_multiple_wagers() {
    let ctx = setup();

    ctx.client.register_referral(&ctx.referee_a, &ctx.referrer);
    ctx.client.register_referral(&ctx.referee_b, &ctx.referrer);

    assert_eq!(ctx.client.settle_wager(&ctx.referee_a, &1_000), 100);
    assert_eq!(ctx.client.settle_wager(&ctx.referee_b, &2_000), 200);
    assert_eq!(ctx.client.settle_wager(&ctx.referee_a, &5_000), 500);

    // 10% of 1_000 + 2_000 + 5_000
    assert_eq!(ctx.client.get_earnings(&ctx.referrer), 800);
}

#[test]
fn test_settle_wager_without_referrer_accrues_nothing() {
    let ctx = setup();

    assert_eq!(ctx.client.settle_wager(&ctx.referee_a, &1_000), 0);
    assert_eq!(ctx.client.get_earnings(&ctx.referrer), 0);
}

#[test]
fn test_withdraw_earnings_pays_out_and_resets_balance() {
    let ctx = setup();

    ctx.client.register_referral(&ctx.referee_a, &ctx.referrer);
    ctx.client.settle_wager(&ctx.referee_a, &10_000); // 10% -> 1_000

    let contract_before = ctx.token_client.balance(&ctx.contract_id);
    assert_eq!(ctx.token_client.balance(&ctx.referrer), 0);

    let paid = ctx.client.withdraw_earnings(&ctx.referrer, &ctx.token);

    assert_eq!(paid, 1_000);
    assert_eq!(ctx.client.get_earnings(&ctx.referrer), 0);
    assert_eq!(ctx.token_client.balance(&ctx.referrer), 1_000);
    assert_eq!(ctx.token_client.balance(&ctx.contract_id), contract_before - 1_000);
}

#[test]
fn test_second_withdraw_is_a_noop() {
    let ctx = setup();

    ctx.client.register_referral(&ctx.referee_a, &ctx.referrer);
    ctx.client.settle_wager(&ctx.referee_a, &10_000); // 10% -> 1_000

    assert_eq!(ctx.client.withdraw_earnings(&ctx.referrer, &ctx.token), 1_000);

    let referrer_after = ctx.token_client.balance(&ctx.referrer);
    let contract_after = ctx.token_client.balance(&ctx.contract_id);

    // Nothing left to withdraw: no panic, no double-pay, no state change.
    assert_eq!(ctx.client.withdraw_earnings(&ctx.referrer, &ctx.token), 0);
    assert_eq!(ctx.client.get_earnings(&ctx.referrer), 0);
    assert_eq!(ctx.token_client.balance(&ctx.referrer), referrer_after);
    assert_eq!(ctx.token_client.balance(&ctx.contract_id), contract_after);
}

#[test]
fn test_withdraw_with_no_accrual_is_a_noop() {
    let ctx = setup();

    assert_eq!(ctx.client.withdraw_earnings(&ctx.referrer, &ctx.token), 0);
    assert_eq!(ctx.token_client.balance(&ctx.referrer), 0);
}

#[test]
fn test_withdraw_requires_referrer_auth() {
    let ctx = setup();

    ctx.client.register_referral(&ctx.referee_a, &ctx.referrer);
    ctx.client.settle_wager(&ctx.referee_a, &10_000);

    // Drop the blanket auth mocking so `require_auth` must be satisfied for real.
    ctx.env.mock_auths(&[]);

    let result = ctx
        .client
        .try_withdraw_earnings(&ctx.referrer, &ctx.token);

    assert!(result.is_err(), "withdrawal must require the referrer's auth");
    // The balance is untouched by the rejected call.
    assert_eq!(ctx.client.get_earnings(&ctx.referrer), 1_000);
}

#[test]
#[should_panic(expected = "Error(Contract, #1)")]
fn test_double_initialize_panics() {
    let ctx = setup();
    ctx.client.initialize(&ctx.admin, &1_000u32);
}

#[test]
#[should_panic(expected = "Error(Contract, #6)")]
fn test_invalid_commission_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, ReferralSplitter);
    let client = ReferralSplitterClient::new(&env, &contract_id);

    client.initialize(&Address::generate(&env), &10_001u32);
}

#[test]
#[should_panic(expected = "Error(Contract, #4)")]
fn test_self_referral_panics() {
    let ctx = setup();
    ctx.client.register_referral(&ctx.referee_a, &ctx.referee_a);
}

#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn test_duplicate_referral_panics() {
    let ctx = setup();

    ctx.client.register_referral(&ctx.referee_a, &ctx.referrer);
    ctx.client.register_referral(&ctx.referee_a, &ctx.referee_b);
}

#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn test_settle_wager_rejects_non_positive_amount() {
    let ctx = setup();
    ctx.client.settle_wager(&ctx.referee_a, &0);
}

#[test]
fn test_get_referrer_returns_binding() {
    let ctx = setup();

    assert!(ctx.client.get_referrer(&ctx.referee_a).is_none());
    ctx.client.register_referral(&ctx.referee_a, &ctx.referrer);
    assert_eq!(
        ctx.client.get_referrer(&ctx.referee_a),
        Some(ctx.referrer.clone())
    );
}
