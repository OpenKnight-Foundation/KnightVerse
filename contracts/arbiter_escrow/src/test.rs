use super::*;
use soroban_sdk::{
    testutils::{Address as _, Events as _, Ledger as _},
    token::{Client as TokenClient, StellarAssetClient},
    Address, Env,
};

const STAKE: i128 = 1_000;
const START_TS: u64 = 1_000_000;

struct TestContext {
    env: Env,
    contract_id: Address,
    admin: Address,
    token: Address,
    player_a: Address,
    player_b: Address,
    arbiter_1: Address,
    arbiter_2: Address,
    arbiter_3: Address,
    outsider: Address,
}

fn client(ctx: &TestContext) -> ArbiterEscrowContractClient<'_> {
    ArbiterEscrowContractClient::new(&ctx.env, &ctx.contract_id)
}

fn token_client(ctx: &TestContext) -> TokenClient<'_> {
    TokenClient::new(&ctx.env, &ctx.token)
}

fn token_admin(ctx: &TestContext) -> StellarAssetClient<'_> {
    StellarAssetClient::new(&ctx.env, &ctx.token)
}

fn balance_of(ctx: &TestContext, who: &Address) -> i128 {
    token_client(ctx).balance(who)
}

fn setup() -> TestContext {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(START_TS);

    let contract_id = env.register_contract(None, ArbiterEscrowContract);
    let admin = Address::generate(&env);
    ArbiterEscrowContractClient::new(&env, &contract_id).initialize(&admin);

    let player_a = Address::generate(&env);
    let player_b = Address::generate(&env);
    let arbiter_1 = Address::generate(&env);
    let arbiter_2 = Address::generate(&env);
    let arbiter_3 = Address::generate(&env);
    let outsider = Address::generate(&env);

    let token_issuer = Address::generate(&env);
    let token_contract = env.register_stellar_asset_contract_v2(token_issuer);
    let token = token_contract.address();

    TestContext {
        env,
        contract_id,
        admin,
        token,
        player_a,
        player_b,
        arbiter_1,
        arbiter_2,
        arbiter_3,
        outsider,
    }
}

fn create_default_escrow(ctx: &TestContext, match_id: u64) {
    client(ctx).create_escrow(
        &ctx.admin,
        &match_id,
        &ctx.token,
        &ctx.player_a,
        &ctx.player_b,
        &STAKE,
        &ctx.arbiter_1,
        &ctx.arbiter_2,
        &ctx.arbiter_3,
    );
}

fn fund_default_escrow(ctx: &TestContext, match_id: u64) {
    token_admin(ctx).mint(&ctx.player_a, &STAKE);
    token_admin(ctx).mint(&ctx.player_b, &STAKE);
    client(ctx).fund_escrow(&match_id, &ctx.player_a);
    client(ctx).fund_escrow(&match_id, &ctx.player_b);
}

fn dispute_default_escrow(ctx: &TestContext, match_id: u64) {
    client(ctx).dispute_match(&match_id, &ctx.player_a);
}

// =============================================================================
// INITIALIZATION & CONFIG
// =============================================================================

#[test]
fn test_initialize_and_constants() {
    let ctx = setup();

    assert_eq!(client(&ctx).get_admin(), ctx.admin);
    assert_eq!(client(&ctx).arbitration_window(), ARBITRATION_WINDOW_SECS);
    assert_eq!(client(&ctx).fallback_timeout(), FALLBACK_TIMEOUT_SECS);
    assert_eq!(client(&ctx).quorum(), QUORUM);
    assert_eq!(client(&ctx).arbiter_count(), ARBITER_COUNT);

    // Guard the exact window lengths promised by the issue.
    assert_eq!(ARBITRATION_WINDOW_SECS, 172_800); // 48 hours
    assert_eq!(FALLBACK_TIMEOUT_SECS, 604_800); // 7 days
    assert_eq!(QUORUM, 2);
}

#[test]
fn test_double_initialize_rejected() {
    let ctx = setup();
    assert!(client(&ctx).try_initialize(&ctx.admin).is_err());
}

// =============================================================================
// ESCROW CREATION & FUNDING
// =============================================================================

#[test]
fn test_create_and_fund_escrow() {
    let ctx = setup();
    let match_id = 1u64;
    create_default_escrow(&ctx, match_id);

    let escrow = client(&ctx).get_escrow(&match_id).unwrap();
    assert_eq!(escrow.status, EscrowStatus::AwaitingFunding);
    assert_eq!(escrow.player_a, ctx.player_a);
    assert_eq!(escrow.player_b, ctx.player_b);
    assert_eq!(escrow.stake, STAKE);
    assert_eq!(escrow.pot, 0);
    assert_eq!(escrow.arbiter_1, ctx.arbiter_1);
    assert_eq!(escrow.arbiter_2, ctx.arbiter_2);
    assert_eq!(escrow.arbiter_3, ctx.arbiter_3);

    // Player A funds their side.
    token_admin(&ctx).mint(&ctx.player_a, &STAKE);
    client(&ctx).fund_escrow(&match_id, &ctx.player_a);

    let escrow = client(&ctx).get_escrow(&match_id).unwrap();
    assert!(escrow.funded_a);
    assert!(!escrow.funded_b);
    assert_eq!(escrow.status, EscrowStatus::AwaitingFunding);
    assert_eq!(escrow.pot, STAKE);
    assert_eq!(balance_of(&ctx, &ctx.contract_id), STAKE);

    // Player B funds; the escrow is now live.
    token_admin(&ctx).mint(&ctx.player_b, &STAKE);
    client(&ctx).fund_escrow(&match_id, &ctx.player_b);

    let escrow = client(&ctx).get_escrow(&match_id).unwrap();
    assert!(escrow.funded_a && escrow.funded_b);
    assert_eq!(escrow.status, EscrowStatus::Funded);
    assert_eq!(escrow.pot, STAKE * 2);
    assert_eq!(balance_of(&ctx, &ctx.contract_id), STAKE * 2);
    assert_eq!(balance_of(&ctx, &ctx.player_a), 0);
    assert_eq!(balance_of(&ctx, &ctx.player_b), 0);
}

#[test]
fn test_duplicate_escrow_rejected() {
    let ctx = setup();
    create_default_escrow(&ctx, 7);

    assert!(client(&ctx)
        .try_create_escrow(
            &ctx.admin,
            &7u64,
            &ctx.token,
            &ctx.player_a,
            &ctx.player_b,
            &STAKE,
            &ctx.arbiter_1,
            &ctx.arbiter_2,
            &ctx.arbiter_3,
        )
        .is_err());
}

#[test]
fn test_invalid_arbiter_panel_rejected() {
    let ctx = setup();

    // Two slots occupied by the same address.
    assert!(client(&ctx)
        .try_create_escrow(
            &ctx.admin,
            &11u64,
            &ctx.token,
            &ctx.player_a,
            &ctx.player_b,
            &STAKE,
            &ctx.arbiter_1,
            &ctx.arbiter_1,
            &ctx.arbiter_3,
        )
        .is_err());

    // An arbiter cannot also be a player.
    assert!(client(&ctx)
        .try_create_escrow(
            &ctx.admin,
            &12u64,
            &ctx.token,
            &ctx.player_a,
            &ctx.player_b,
            &STAKE,
            &ctx.player_a,
            &ctx.arbiter_2,
            &ctx.arbiter_3,
        )
        .is_err());
}

#[test]
fn test_non_admin_cannot_create_escrow() {
    let ctx = setup();

    assert!(client(&ctx)
        .try_create_escrow(
            &ctx.outsider,
            &13u64,
            &ctx.token,
            &ctx.player_a,
            &ctx.player_b,
            &STAKE,
            &ctx.arbiter_1,
            &ctx.arbiter_2,
            &ctx.arbiter_3,
        )
        .is_err());
}

#[test]
fn test_invalid_stake_rejected() {
    let ctx = setup();

    assert!(client(&ctx)
        .try_create_escrow(
            &ctx.admin,
            &14u64,
            &ctx.token,
            &ctx.player_a,
            &ctx.player_b,
            &0i128,
            &ctx.arbiter_1,
            &ctx.arbiter_2,
            &ctx.arbiter_3,
        )
        .is_err());
}

#[test]
fn test_fund_twice_rejected() {
    let ctx = setup();
    create_default_escrow(&ctx, 2);

    token_admin(&ctx).mint(&ctx.player_a, &(STAKE * 2));
    client(&ctx).fund_escrow(&2u64, &ctx.player_a);

    assert!(client(&ctx).try_fund_escrow(&2u64, &ctx.player_a).is_err());
    assert_eq!(balance_of(&ctx, &ctx.contract_id), STAKE);
}

#[test]
fn test_only_participant_can_fund() {
    let ctx = setup();
    create_default_escrow(&ctx, 3);

    assert!(client(&ctx).try_fund_escrow(&3u64, &ctx.outsider).is_err());
}

// =============================================================================
// DISPUTE FREEZE
// =============================================================================

#[test]
fn test_dispute_opens_arbitration_and_fallback_windows() {
    let ctx = setup();
    let match_id = 10u64;
    create_default_escrow(&ctx, match_id);
    fund_default_escrow(&ctx, match_id);

    let before = ctx.env.events().all().len();
    dispute_default_escrow(&ctx, match_id);
    let after = ctx.env.events().all().len();

    // Exactly one `dispute` event is emitted.
    assert_eq!(after, before + 1);

    let escrow = client(&ctx).get_escrow(&match_id).unwrap();
    assert_eq!(escrow.status, EscrowStatus::Disputed);
    assert_eq!(escrow.dispute_started_at, START_TS);
    assert_eq!(
        escrow.arbitration_deadline,
        START_TS + ARBITRATION_WINDOW_SECS
    );
    assert_eq!(escrow.fallback_deadline, START_TS + FALLBACK_TIMEOUT_SECS);
}

#[test]
fn test_only_participant_or_admin_can_dispute() {
    let ctx = setup();
    let match_id = 15u64;
    create_default_escrow(&ctx, match_id);
    fund_default_escrow(&ctx, match_id);

    assert!(client(&ctx)
        .try_dispute_match(&match_id, &ctx.outsider)
        .is_err());
    // Admin may raise the dispute as well.
    client(&ctx).dispute_match(&match_id, &ctx.admin);
    assert_eq!(
        client(&ctx).get_escrow(&match_id).unwrap().status,
        EscrowStatus::Disputed
    );
}

#[test]
fn test_dispute_requires_funded_escrow() {
    let ctx = setup();
    let match_id = 16u64;
    create_default_escrow(&ctx, match_id);

    // Not funded yet — cannot dispute.
    assert!(client(&ctx)
        .try_dispute_match(&match_id, &ctx.player_a)
        .is_err());
}

#[test]
fn test_settle_blocked_while_disputed() {
    let ctx = setup();
    let match_id = 20u64;
    create_default_escrow(&ctx, match_id);
    fund_default_escrow(&ctx, match_id);
    dispute_default_escrow(&ctx, match_id);

    // Payout is frozen: even the admin cannot use the fast path.
    assert!(client(&ctx)
        .try_settle_match(&match_id, &ctx.admin, &ctx.player_a)
        .is_err());
    assert!(client(&ctx).try_settle_refund(&match_id, &ctx.admin).is_err());
    assert_eq!(balance_of(&ctx, &ctx.contract_id), STAKE * 2);
    assert_eq!(
        client(&ctx).get_escrow(&match_id).unwrap().status,
        EscrowStatus::Disputed
    );
}

// =============================================================================
// UNDISPUTED FAST PATH
// =============================================================================

#[test]
fn test_admin_fast_path_settle_transfers_pot() {
    let ctx = setup();
    let match_id = 21u64;
    create_default_escrow(&ctx, match_id);
    fund_default_escrow(&ctx, match_id);

    client(&ctx).settle_match(&match_id, &ctx.admin, &ctx.player_b);

    assert_eq!(balance_of(&ctx, &ctx.player_b), STAKE * 2);
    assert_eq!(balance_of(&ctx, &ctx.player_a), 0);
    assert_eq!(balance_of(&ctx, &ctx.contract_id), 0);

    let escrow = client(&ctx).get_escrow(&match_id).unwrap();
    assert_eq!(escrow.status, EscrowStatus::Resolved);
    assert_eq!(escrow.winning_choice, Some(VoteChoice::PlayerB));

    // A resolved escrow cannot be settled twice.
    assert!(client(&ctx)
        .try_settle_match(&match_id, &ctx.admin, &ctx.player_a)
        .is_err());
}

#[test]
fn test_admin_fast_path_refund_returns_stakes() {
    let ctx = setup();
    let match_id = 22u64;
    create_default_escrow(&ctx, match_id);
    fund_default_escrow(&ctx, match_id);

    client(&ctx).settle_refund(&match_id, &ctx.admin);

    assert_eq!(balance_of(&ctx, &ctx.player_a), STAKE);
    assert_eq!(balance_of(&ctx, &ctx.player_b), STAKE);
    assert_eq!(balance_of(&ctx, &ctx.contract_id), 0);
    assert_eq!(
        client(&ctx).get_escrow(&match_id).unwrap().winning_choice,
        Some(VoteChoice::Refund)
    );
}

// =============================================================================
// ARBITER PANEL (2-OF-3)
// =============================================================================

#[test]
fn test_two_of_three_arbiters_release_to_winner() {
    let ctx = setup();
    let match_id = 30u64;
    create_default_escrow(&ctx, match_id);
    fund_default_escrow(&ctx, match_id);
    dispute_default_escrow(&ctx, match_id);

    client(&ctx).vote(&match_id, &ctx.arbiter_1, &VoteChoice::PlayerA);
    client(&ctx).vote(&match_id, &ctx.arbiter_3, &VoteChoice::PlayerA);

    let escrow = client(&ctx).get_escrow(&match_id).unwrap();
    assert_eq!(escrow.votes_for_a, 2);
    assert_eq!(
        client(&ctx).get_vote(&match_id, &ctx.arbiter_1),
        Some(VoteChoice::PlayerA)
    );

    // The non-voting arbiter (or anyone authorized) can execute the decision.
    client(&ctx).resolve_dispute(&match_id, &ctx.arbiter_2);

    assert_eq!(balance_of(&ctx, &ctx.player_a), STAKE * 2);
    assert_eq!(balance_of(&ctx, &ctx.player_b), 0);
    assert_eq!(balance_of(&ctx, &ctx.contract_id), 0);

    let escrow = client(&ctx).get_escrow(&match_id).unwrap();
    assert_eq!(escrow.status, EscrowStatus::Resolved);
    assert_eq!(escrow.winning_choice, Some(VoteChoice::PlayerA));
}

#[test]
fn test_arbiter_refund_returns_stakes() {
    let ctx = setup();
    let match_id = 33u64;
    create_default_escrow(&ctx, match_id);
    fund_default_escrow(&ctx, match_id);
    dispute_default_escrow(&ctx, match_id);

    client(&ctx).vote(&match_id, &ctx.arbiter_2, &VoteChoice::Refund);
    client(&ctx).vote(&match_id, &ctx.arbiter_3, &VoteChoice::Refund);
    client(&ctx).resolve_dispute(&match_id, &ctx.player_a);

    assert_eq!(balance_of(&ctx, &ctx.player_a), STAKE);
    assert_eq!(balance_of(&ctx, &ctx.player_b), STAKE);
    assert_eq!(balance_of(&ctx, &ctx.contract_id), 0);
    assert_eq!(
        client(&ctx).get_escrow(&match_id).unwrap().winning_choice,
        Some(VoteChoice::Refund)
    );
}

#[test]
fn test_split_votes_do_not_resolve() {
    let ctx = setup();
    let match_id = 34u64;
    create_default_escrow(&ctx, match_id);
    fund_default_escrow(&ctx, match_id);
    dispute_default_escrow(&ctx, match_id);

    // 1 / 1 / 1 — nobody reaches the 2-of-3 threshold.
    client(&ctx).vote(&match_id, &ctx.arbiter_1, &VoteChoice::PlayerA);
    client(&ctx).vote(&match_id, &ctx.arbiter_2, &VoteChoice::PlayerB);
    client(&ctx).vote(&match_id, &ctx.arbiter_3, &VoteChoice::Refund);

    assert!(client(&ctx)
        .try_resolve_dispute(&match_id, &ctx.arbiter_1)
        .is_err());
    assert_eq!(balance_of(&ctx, &ctx.contract_id), STAKE * 2);
}

#[test]
fn test_resolve_before_threshold_rejected() {
    let ctx = setup();
    let match_id = 31u64;
    create_default_escrow(&ctx, match_id);
    fund_default_escrow(&ctx, match_id);
    dispute_default_escrow(&ctx, match_id);

    // A single vote is not enough to release funds.
    client(&ctx).vote(&match_id, &ctx.arbiter_1, &VoteChoice::PlayerA);
    assert!(client(&ctx)
        .try_resolve_dispute(&match_id, &ctx.arbiter_1)
        .is_err());
    assert_eq!(balance_of(&ctx, &ctx.contract_id), STAKE * 2);

    // The second vote completes the 2-of-3 majority.
    client(&ctx).vote(&match_id, &ctx.arbiter_2, &VoteChoice::PlayerA);
    client(&ctx).resolve_dispute(&match_id, &ctx.player_a);
    assert_eq!(balance_of(&ctx, &ctx.player_a), STAKE * 2);
}

#[test]
fn test_double_vote_rejected() {
    let ctx = setup();
    let match_id = 35u64;
    create_default_escrow(&ctx, match_id);
    fund_default_escrow(&ctx, match_id);
    dispute_default_escrow(&ctx, match_id);

    client(&ctx).vote(&match_id, &ctx.arbiter_1, &VoteChoice::PlayerA);

    assert!(client(&ctx)
        .try_vote(&match_id, &ctx.arbiter_1, &VoteChoice::PlayerB)
        .is_err());

    let escrow = client(&ctx).get_escrow(&match_id).unwrap();
    assert_eq!(escrow.votes_for_a, 1);
    assert_eq!(escrow.votes_for_b, 0);
    assert_eq!(
        client(&ctx).get_vote(&match_id, &ctx.arbiter_1),
        Some(VoteChoice::PlayerA)
    );
}

#[test]
fn test_unauthorized_arbiter_vote_rejected() {
    let ctx = setup();
    let match_id = 36u64;
    create_default_escrow(&ctx, match_id);
    fund_default_escrow(&ctx, match_id);
    dispute_default_escrow(&ctx, match_id);

    // Outsider is not on the panel; the other player is not either.
    assert!(client(&ctx)
        .try_vote(&match_id, &ctx.outsider, &VoteChoice::PlayerA)
        .is_err());
    assert!(client(&ctx)
        .try_vote(&match_id, &ctx.player_a, &VoteChoice::PlayerA)
        .is_err());

    // An outsider cannot execute a resolution either.
    assert!(client(&ctx)
        .try_resolve_dispute(&match_id, &ctx.outsider)
        .is_err());
    assert_eq!(
        client(&ctx).get_escrow(&match_id).unwrap().votes_for_a,
        0
    );
}

#[test]
fn test_vote_after_arbitration_window_rejected() {
    let ctx = setup();
    let match_id = 32u64;
    create_default_escrow(&ctx, match_id);
    fund_default_escrow(&ctx, match_id);
    dispute_default_escrow(&ctx, match_id);

    client(&ctx).vote(&match_id, &ctx.arbiter_1, &VoteChoice::PlayerA);

    // One second before the deadline is still valid...
    ctx.env
        .ledger()
        .set_timestamp(START_TS + ARBITRATION_WINDOW_SECS - 1);
    client(&ctx).vote(&match_id, &ctx.arbiter_2, &VoteChoice::PlayerA);

    // ...but the window is closed at the deadline.
    ctx.env
        .ledger()
        .set_timestamp(START_TS + ARBITRATION_WINDOW_SECS);
    assert!(client(&ctx)
        .try_vote(&match_id, &ctx.arbiter_3, &VoteChoice::PlayerA)
        .is_err());
}

#[test]
fn test_vote_on_undisputed_match_rejected() {
    let ctx = setup();
    let match_id = 37u64;
    create_default_escrow(&ctx, match_id);
    fund_default_escrow(&ctx, match_id);

    assert!(client(&ctx)
        .try_vote(&match_id, &ctx.arbiter_1, &VoteChoice::PlayerA)
        .is_err());
}

// =============================================================================
// TIME-LOCKED FALLBACK
// =============================================================================

#[test]
fn test_fallback_before_timeout_rejected() {
    let ctx = setup();
    let match_id = 40u64;
    create_default_escrow(&ctx, match_id);
    fund_default_escrow(&ctx, match_id);
    dispute_default_escrow(&ctx, match_id);

    // Even at the very last second the timelock holds.
    ctx.env
        .ledger()
        .set_timestamp(START_TS + FALLBACK_TIMEOUT_SECS - 1);
    assert!(client(&ctx)
        .try_execute_fallback(&match_id, &ctx.outsider)
        .is_err());
    assert_eq!(balance_of(&ctx, &ctx.contract_id), STAKE * 2);
}

#[test]
fn test_fallback_after_seven_days_refunds_players() {
    let ctx = setup();
    let match_id = 41u64;
    create_default_escrow(&ctx, match_id);
    fund_default_escrow(&ctx, match_id);
    dispute_default_escrow(&ctx, match_id);

    // Arbiters fail to vote. After 7 days the permissionless fallback fires.
    ctx.env
        .ledger()
        .set_timestamp(START_TS + FALLBACK_TIMEOUT_SECS);
    client(&ctx).execute_fallback(&match_id, &ctx.outsider);

    assert_eq!(balance_of(&ctx, &ctx.player_a), STAKE);
    assert_eq!(balance_of(&ctx, &ctx.player_b), STAKE);
    assert_eq!(balance_of(&ctx, &ctx.contract_id), 0);

    let escrow = client(&ctx).get_escrow(&match_id).unwrap();
    assert_eq!(escrow.status, EscrowStatus::Resolved);
    assert_eq!(escrow.winning_choice, Some(VoteChoice::Refund));
    assert_eq!(escrow.resolved_at, START_TS + FALLBACK_TIMEOUT_SECS);
}

#[test]
fn test_fallback_blocked_when_quorum_reached() {
    let ctx = setup();
    let match_id = 42u64;
    create_default_escrow(&ctx, match_id);
    fund_default_escrow(&ctx, match_id);
    dispute_default_escrow(&ctx, match_id);

    client(&ctx).vote(&match_id, &ctx.arbiter_1, &VoteChoice::PlayerB);
    client(&ctx).vote(&match_id, &ctx.arbiter_2, &VoteChoice::PlayerB);

    // The majority exists, so the fallback must not be used to unwind it.
    ctx.env
        .ledger()
        .set_timestamp(START_TS + FALLBACK_TIMEOUT_SECS);
    assert!(client(&ctx)
        .try_execute_fallback(&match_id, &ctx.outsider)
        .is_err());

    // Resolution is still possible after the timelock.
    client(&ctx).resolve_dispute(&match_id, &ctx.arbiter_3);
    assert_eq!(balance_of(&ctx, &ctx.player_b), STAKE * 2);
    assert_eq!(balance_of(&ctx, &ctx.player_a), 0);
}

// =============================================================================
// EVENTS
// =============================================================================

#[test]
fn test_events_emitted_across_dispute_lifecycle() {
    let ctx = setup();
    let match_id = 50u64;

    let before_create = ctx.env.events().all().len();
    create_default_escrow(&ctx, match_id);
    // `created` + `arbiters`.
    assert_eq!(ctx.env.events().all().len(), before_create + 2);

    fund_default_escrow(&ctx, match_id);
    let after_fund = ctx.env.events().all().len();
    // At least the two `funded` events plus the `ready` event.
    assert!(after_fund >= before_create + 5);

    let before_dispute = ctx.env.events().all().len();
    dispute_default_escrow(&ctx, match_id);
    assert_eq!(ctx.env.events().all().len(), before_dispute + 1);

    let before_vote = ctx.env.events().all().len();
    client(&ctx).vote(&match_id, &ctx.arbiter_1, &VoteChoice::PlayerA);
    assert_eq!(ctx.env.events().all().len(), before_vote + 1);

    client(&ctx).vote(&match_id, &ctx.arbiter_2, &VoteChoice::PlayerA);
    let before_resolve = ctx.env.events().all().len();
    client(&ctx).resolve_dispute(&match_id, &ctx.player_b);
    // `payout` + `resolved` at minimum.
    assert!(ctx.env.events().all().len() >= before_resolve + 2);
}
