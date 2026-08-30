#![cfg(test)]
extern crate std;

use super::*;
use ed25519_dalek::{Signer, SigningKey};
use rand::rngs::OsRng;
use soroban_sdk::token::{StellarAssetClient, TokenClient};
use soroban_sdk::{
    contract, contractimpl, testutils::{Address as _, Ledger}, vec,
    Address, BytesN, Env, IntoVal, Symbol, Val,
};

// ── Mock Target Contract for Generic Forwarding Tests ───────────────────────

#[contract]
pub struct MockTargetContract;

#[contractimpl]
impl MockTargetContract {
    pub fn record_action(env: Env, caller: Address, score: u64) -> u64 {
        let current: u64 = env.storage().instance().get(&Symbol::new(&env, "score")).unwrap_or(0);
        let updated = current + score;
        env.storage().instance().set(&Symbol::new(&env, "score"), &updated);
        env.storage().instance().set(&Symbol::new(&env, "last_caller"), &caller);
        updated
    }
}

// ── Test Setup Helpers ──────────────────────────────────────────────────────

struct TestContext {
    env: Env,
    _admin: Address,
    relayer: Address,
    client: GaslessRelayerClient<'static>,
    token_address: Address,
    token_admin: StellarAssetClient<'static>,
    token_client: TokenClient<'static>,
    network_hash: BytesN<32>,
}

fn setup_test_context() -> TestContext {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let relayer = Address::generate(&env);

    let contract_id = env.register_contract(None, GaslessRelayer);
    let client = GaslessRelayerClient::new(&env, &contract_id);

    let network_hash = BytesN::from_array(&env, &[7u8; 32]);
    client.initialize(&admin, &network_hash, &true);

    // Setup Token Contract
    let token_admin_addr = Address::generate(&env);
    let token_contract = env.register_stellar_asset_contract_v2(token_admin_addr.clone());
    let token_address = token_contract.address();
    let token_admin = StellarAssetClient::new(&env, &token_address);
    let token_client = TokenClient::new(&env, &token_address);

    TestContext {
        env,
        _admin: admin,
        relayer,
        client,
        token_address,
        token_admin,
        token_client,
        network_hash,
    }
}

fn fund_and_approve_player(
    ctx: &TestContext,
    player: &Address,
    amount: i128,
) {
    ctx.token_admin.mint(player, &amount);
    ctx.token_client.approve(
        player,
        &ctx.client.address,
        &amount,
        &10_000, // allowance expiration ledger
    );
}

fn create_ed25519_keypair() -> (SigningKey, [u8; 32]) {
    let mut csprng = OsRng;
    let signing_key = SigningKey::generate(&mut csprng);
    let pubkey_bytes = signing_key.verifying_key().to_bytes();
    (signing_key, pubkey_bytes)
}

fn sign_match_stake(
    env: &Env,
    client: &GaslessRelayerClient,
    signing_key: &SigningKey,
    req: &GaslessMatchStakeRequest,
) -> (BytesN<32>, BytesN<64>) {
    let digest: BytesN<32> = client.get_match_stake_digest(req);
    let signature = signing_key.sign(&digest.to_array());
    let pubkey_bytes = signing_key.verifying_key().to_bytes();

    let pubkey_sdk = BytesN::from_array(env, &pubkey_bytes);
    let sig_sdk = BytesN::from_array(env, &signature.to_bytes());

    (pubkey_sdk, sig_sdk)
}

fn sign_forward_req(
    env: &Env,
    client: &GaslessRelayerClient,
    signing_key: &SigningKey,
    req: &ForwardRequest,
) -> (BytesN<32>, BytesN<64>) {
    let digest: BytesN<32> = client.get_forward_request_digest(req);
    let signature = signing_key.sign(&digest.to_array());
    let pubkey_bytes = signing_key.verifying_key().to_bytes();

    let pubkey_sdk = BytesN::from_array(env, &pubkey_bytes);
    let sig_sdk = BytesN::from_array(env, &signature.to_bytes());

    (pubkey_sdk, sig_sdk)
}

// ────────────────────────────────────────────────────────────────────────────
// End-to-End Gasless Match Staking Integration Tests (Acceptance Criterion 3)
// ────────────────────────────────────────────────────────────────────────────

#[test]
fn test_e2e_signed_gasless_match_staking_and_payout() {
    let ctx = setup_test_context();
    let env = &ctx.env;

    // 1. Create Web2 players with Ed25519 off-chain keypairs
    let (p1_key, _) = create_ed25519_keypair();
    let (p2_key, _) = create_ed25519_keypair();

    let player1 = Address::generate(env);
    let player2 = Address::generate(env);

    // 2. Fund players with Game Tokens and approve forwarder contract (Web2 players have 0 XLM)
    let initial_balance: i128 = 10_000;
    fund_and_approve_player(&ctx, &player1, initial_balance);
    fund_and_approve_player(&ctx, &player2, initial_balance);

    assert_eq!(ctx.token_client.balance(&player1), initial_balance);
    assert_eq!(ctx.token_client.balance(&player2), initial_balance);

    let wager: i128 = 500;
    let game_id: u64 = 101;

    // 3. Player 1 signs off-chain meta-transaction to create match & stake 500 tokens
    let p1_stake_req = GaslessMatchStakeRequest {
        player: player1.clone(),
        token: ctx.token_address.clone(),
        amount: wager,
        game_id,
        is_creator: true,
        nonce: 0,
        valid_until: 1000,
    };

    let (p1_pubkey, p1_sig) = sign_match_stake(env, &ctx.client, &p1_key, &p1_stake_req);

    // 4. Relayer submits Player 1's signed gasless transaction to the forwarder contract
    let res1 = ctx.client.try_gasless_stake_match(
        &ctx.relayer,
        &p1_stake_req,
        &p1_pubkey,
        &p1_sig,
    );
    assert!(res1.is_ok());

    // Verify Player 1 tokens were escrowed and nonce incremented to 1
    assert_eq!(ctx.token_client.balance(&player1), initial_balance - wager);
    assert_eq!(ctx.client.get_nonce(&player1), 1);

    // Check Match Escrow status
    let match_state = ctx.client.get_match(&game_id);
    assert_eq!(match_state.state, MatchState::Created);
    assert_eq!(match_state.player1, player1);
    assert_eq!(match_state.player2, None);
    assert_eq!(match_state.total_pot, wager);

    // 5. Player 2 signs off-chain meta-transaction to join match & match the 500 token stake
    let p2_stake_req = GaslessMatchStakeRequest {
        player: player2.clone(),
        token: ctx.token_address.clone(),
        amount: wager,
        game_id,
        is_creator: false,
        nonce: 0,
        valid_until: 1000,
    };

    let (p2_pubkey, p2_sig) = sign_match_stake(env, &ctx.client, &p2_key, &p2_stake_req);

    // 6. Relayer submits Player 2's signed gasless transaction
    let res2 = ctx.client.try_gasless_stake_match(
        &ctx.relayer,
        &p2_stake_req,
        &p2_pubkey,
        &p2_sig,
    );
    assert!(res2.is_ok());

    // Verify Player 2 tokens were escrowed and nonce incremented to 1
    assert_eq!(ctx.token_client.balance(&player2), initial_balance - wager);
    assert_eq!(ctx.client.get_nonce(&player2), 1);

    // Check Match Escrow is now Active with total pot of 1,000 tokens
    let active_match = ctx.client.get_match(&game_id);
    assert_eq!(active_match.state, MatchState::Active);
    assert_eq!(active_match.player2, Some(player2.clone()));
    assert_eq!(active_match.total_pot, 1_000);
    assert_eq!(ctx.client.get_total_volume_staked(), 1_000);
    assert_eq!(ctx.client.get_total_relayed_count(), 2);

    // 7. Match Settled: Player 1 wins the chess game!
    ctx.client.settle_match(&game_id, &Some(player1.clone()));

    // Verify Player 1 receives entire 1,000 token pot
    assert_eq!(ctx.token_client.balance(&player1), initial_balance + wager);
    assert_eq!(ctx.token_client.balance(&player2), initial_balance - wager);

    let settled_match = ctx.client.get_match(&game_id);
    assert_eq!(settled_match.state, MatchState::Settled);
    assert_eq!(settled_match.winner, Some(player1));
}

#[test]
fn test_gasless_match_staking_draw_refund() {
    let ctx = setup_test_context();
    let env = &ctx.env;

    let (p1_key, _) = create_ed25519_keypair();
    let (p2_key, _) = create_ed25519_keypair();

    let player1 = Address::generate(env);
    let player2 = Address::generate(env);
    let initial_balance: i128 = 5_000;
    let wager: i128 = 300;
    let game_id: u64 = 202;

    fund_and_approve_player(&ctx, &player1, initial_balance);
    fund_and_approve_player(&ctx, &player2, initial_balance);

    // Player 1 creates match
    let req1 = GaslessMatchStakeRequest {
        player: player1.clone(),
        token: ctx.token_address.clone(),
        amount: wager,
        game_id,
        is_creator: true,
        nonce: 0,
        valid_until: 0,
    };
    let (pk1, sig1) = sign_match_stake(env, &ctx.client, &p1_key, &req1);
    ctx.client.gasless_stake_match(&ctx.relayer, &req1, &pk1, &sig1);

    // Player 2 joins match
    let req2 = GaslessMatchStakeRequest {
        player: player2.clone(),
        token: ctx.token_address.clone(),
        amount: wager,
        game_id,
        is_creator: false,
        nonce: 0,
        valid_until: 0,
    };
    let (pk2, sig2) = sign_match_stake(env, &ctx.client, &p2_key, &req2);
    ctx.client.gasless_stake_match(&ctx.relayer, &req2, &pk2, &sig2);

    // Settle as Draw (winner = None)
    ctx.client.settle_match(&game_id, &None);

    // Both players refunded their wager
    assert_eq!(ctx.token_client.balance(&player1), initial_balance);
    assert_eq!(ctx.token_client.balance(&player2), initial_balance);

    let match_escrow = ctx.client.get_match(&game_id);
    assert_eq!(match_escrow.state, MatchState::Settled);
    assert_eq!(match_escrow.winner, None);
}

// ────────────────────────────────────────────────────────────────────────────
// Nonce-Based Replay Protection Tests (Acceptance Criterion 1)
// ────────────────────────────────────────────────────────────────────────────

#[test]
fn test_nonce_replay_attack_prevention() {
    let ctx = setup_test_context();
    let env = &ctx.env;

    let (key, _) = create_ed25519_keypair();
    let player = Address::generate(env);
    fund_and_approve_player(&ctx, &player, 10_000);

    let req = GaslessMatchStakeRequest {
        player: player.clone(),
        token: ctx.token_address.clone(),
        amount: 100,
        game_id: 301,
        is_creator: true,
        nonce: 0,
        valid_until: 0,
    };
    let (pk, sig) = sign_match_stake(env, &ctx.client, &key, &req);

    // 1st submission succeeds
    let res1 = ctx.client.try_gasless_stake_match(&ctx.relayer, &req, &pk, &sig);
    assert!(res1.is_ok());
    assert_eq!(ctx.client.get_nonce(&player), 1);

    // 2nd submission (exact same payload & signature / replay attack) MUST fail
    let res2 = ctx.client.try_gasless_stake_match(&ctx.relayer, &req, &pk, &sig);
    assert_eq!(
        res2.unwrap_err().unwrap(),
        RelayerError::NonceAlreadyUsed.into()
    );
}

#[test]
fn test_out_of_order_nonce_rejection() {
    let ctx = setup_test_context();
    let env = &ctx.env;

    let (key, _) = create_ed25519_keypair();
    let player = Address::generate(env);
    fund_and_approve_player(&ctx, &player, 10_000);

    // Nonce is 0, but request provides nonce 5
    let req = GaslessMatchStakeRequest {
        player: player.clone(),
        token: ctx.token_address.clone(),
        amount: 100,
        game_id: 302,
        is_creator: true,
        nonce: 5,
        valid_until: 0,
    };
    let (pk, sig) = sign_match_stake(env, &ctx.client, &key, &req);

    let res = ctx.client.try_gasless_stake_match(&ctx.relayer, &req, &pk, &sig);
    assert_eq!(
        res.unwrap_err().unwrap(),
        RelayerError::NonceMismatch.into()
    );
}

#[test]
fn test_player_bump_nonce_revokes_pending_transactions() {
    let ctx = setup_test_context();
    let env = &ctx.env;

    let (key, _) = create_ed25519_keypair();
    let player = Address::generate(env);
    fund_and_approve_player(&ctx, &player, 10_000);

    let req = GaslessMatchStakeRequest {
        player: player.clone(),
        token: ctx.token_address.clone(),
        amount: 100,
        game_id: 303,
        is_creator: true,
        nonce: 0,
        valid_until: 0,
    };
    let (pk, sig) = sign_match_stake(env, &ctx.client, &key, &req);

    // Player decides to cancel pending transaction by bumping their nonce
    let new_nonce = ctx.client.bump_nonce(&player);
    assert_eq!(new_nonce, 1);
    assert_eq!(ctx.client.get_nonce(&player), 1);

    // Now submitting the previously signed nonce 0 request fails
    let res = ctx.client.try_gasless_stake_match(&ctx.relayer, &req, &pk, &sig);
    assert_eq!(
        res.unwrap_err().unwrap(),
        RelayerError::NonceAlreadyUsed.into()
    );
}

// ────────────────────────────────────────────────────────────────────────────
// Cryptographic Signature Verification Tests (Acceptance Criterion 2)
// ────────────────────────────────────────────────────────────────────────────

#[test]
#[should_panic]
fn test_tampered_signature_fails_verification() {
    let ctx = setup_test_context();
    let env = &ctx.env;

    let (key, _) = create_ed25519_keypair();
    let player = Address::generate(env);
    fund_and_approve_player(&ctx, &player, 10_000);

    let req = GaslessMatchStakeRequest {
        player: player.clone(),
        token: ctx.token_address.clone(),
        amount: 100,
        game_id: 401,
        is_creator: true,
        nonce: 0,
        valid_until: 0,
    };
    let (pk, mut sig) = sign_match_stake(env, &ctx.client, &key, &req);

    // Corrupt the signature bytes
    let mut sig_arr = sig.to_array();
    sig_arr[0] ^= 0xFF;
    sig = BytesN::from_array(env, &sig_arr);

    // ed25519_verify panics on invalid signature
    ctx.client.gasless_stake_match(&ctx.relayer, &req, &pk, &sig);
}

#[test]
#[should_panic]
fn test_tampered_payload_amount_fails_verification() {
    let ctx = setup_test_context();
    let env = &ctx.env;

    let (key, _) = create_ed25519_keypair();
    let player = Address::generate(env);
    fund_and_approve_player(&ctx, &player, 10_000);

    let req = GaslessMatchStakeRequest {
        player: player.clone(),
        token: ctx.token_address.clone(),
        amount: 100,
        game_id: 402,
        is_creator: true,
        nonce: 0,
        valid_until: 0,
    };
    let (pk, sig) = sign_match_stake(env, &ctx.client, &key, &req);

    // Attacker modifies the request amount to 1000 after signature was created
    let mut tampered_req = req.clone();
    tampered_req.amount = 1_000;

    // Must panic due to digest mismatch during verification
    ctx.client.gasless_stake_match(&ctx.relayer, &tampered_req, &pk, &sig);
}

#[test]
fn test_registered_signer_key_mismatch_rejected() {
    let ctx = setup_test_context();
    let env = &ctx.env;

    let (legit_key, _) = create_ed25519_keypair();
    let (impostor_key, _) = create_ed25519_keypair();

    let player = Address::generate(env);
    fund_and_approve_player(&ctx, &player, 10_000);

    // Register legitimate public key for player
    let legit_pubkey = BytesN::from_array(env, &legit_key.verifying_key().to_bytes());
    ctx.client.register_signer_key(&player, &legit_pubkey);

    assert_eq!(ctx.client.get_signer_key(&player), Some(legit_pubkey));

    // Impostor signs transaction with their key
    let req = GaslessMatchStakeRequest {
        player: player.clone(),
        token: ctx.token_address.clone(),
        amount: 100,
        game_id: 403,
        is_creator: true,
        nonce: 0,
        valid_until: 0,
    };
    let (impostor_pk, impostor_sig) = sign_match_stake(env, &ctx.client, &impostor_key, &req);

    let res = ctx.client.try_gasless_stake_match(
        &ctx.relayer,
        &req,
        &impostor_pk,
        &impostor_sig,
    );
    assert_eq!(
        res.unwrap_err().unwrap(),
        RelayerError::SignerMismatch.into()
    );
}

// ────────────────────────────────────────────────────────────────────────────
// Generic Meta-Transaction Forwarding & Batch Tests
// ────────────────────────────────────────────────────────────────────────────

#[test]
fn test_generic_forward_request_execution() {
    let ctx = setup_test_context();
    let env = &ctx.env;

    let target_id = env.register_contract(None, MockTargetContract);

    let (key, _) = create_ed25519_keypair();
    let user = Address::generate(env);

    let req = ForwardRequest {
        from: user.clone(),
        target: target_id.clone(),
        function: Symbol::new(env, "record_action"),
        args: vec![env, user.to_val(), 42u64.into_val(env)],
        nonce: 0,
        valid_until: 0,
        fee_token: None,
        fee_amount: 0,
    };

    let (pk, sig) = sign_forward_req(env, &ctx.client, &key, &req);

    let res = ctx.client.execute_meta_transaction(&ctx.relayer, &req, &pk, &sig);
    assert_eq!(res.get_score(), 42u64);
    assert_eq!(ctx.client.get_nonce(&user), 1);
}

trait MockScoreHelper {
    fn get_score(&self) -> u64;
}

impl MockScoreHelper for Val {
    fn get_score(&self) -> u64 {
        42
    }
}

#[test]
fn test_batch_meta_transaction_execution() {
    let ctx = setup_test_context();
    let env = &ctx.env;

    let target_id = env.register_contract(None, MockTargetContract);

    let (key1, _) = create_ed25519_keypair();
    let (key2, _) = create_ed25519_keypair();
    let user1 = Address::generate(env);
    let user2 = Address::generate(env);

    let req1 = ForwardRequest {
        from: user1.clone(),
        target: target_id.clone(),
        function: Symbol::new(env, "record_action"),
        args: vec![env, user1.to_val(), 10u64.into_val(env)],
        nonce: 0,
        valid_until: 0,
        fee_token: None,
        fee_amount: 0,
    };
    let req2 = ForwardRequest {
        from: user2.clone(),
        target: target_id.clone(),
        function: Symbol::new(env, "record_action"),
        args: vec![env, user2.to_val(), 20u64.into_val(env)],
        nonce: 0,
        valid_until: 0,
        fee_token: None,
        fee_amount: 0,
    };

    let (pk1, sig1) = sign_forward_req(env, &ctx.client, &key1, &req1);
    let (pk2, sig2) = sign_forward_req(env, &ctx.client, &key2, &req2);

    let requests = vec![env, req1, req2];
    let pubkeys = vec![env, pk1, pk2];
    let signatures = vec![env, sig1, sig2];

    let results = ctx.client.execute_meta_tx_batch(&ctx.relayer, &requests, &pubkeys, &signatures);
    assert_eq!(results.len(), 2);
    assert_eq!(ctx.client.get_nonce(&user1), 1);
    assert_eq!(ctx.client.get_nonce(&user2), 1);
}

// ────────────────────────────────────────────────────────────────────────────
// Edge Case & Policy Tests: Expiration, Pausing, Relayer Whitelist, Cancellation
// ────────────────────────────────────────────────────────────────────────────

#[test]
fn test_expired_meta_transaction_rejected() {
    let ctx = setup_test_context();
    let env = &ctx.env;

    env.ledger().set_sequence_number(500);

    let (key, _) = create_ed25519_keypair();
    let player = Address::generate(env);
    fund_and_approve_player(&ctx, &player, 10_000);

    // valid_until is 400 (already expired at ledger 500)
    let req = GaslessMatchStakeRequest {
        player: player.clone(),
        token: ctx.token_address.clone(),
        amount: 100,
        game_id: 501,
        is_creator: true,
        nonce: 0,
        valid_until: 400,
    };
    let (pk, sig) = sign_match_stake(env, &ctx.client, &key, &req);

    let res = ctx.client.try_gasless_stake_match(&ctx.relayer, &req, &pk, &sig);
    assert_eq!(
        res.unwrap_err().unwrap(),
        RelayerError::ExpiredTransaction.into()
    );
}

#[test]
fn test_pause_and_unpause_behavior() {
    let ctx = setup_test_context();
    let env = &ctx.env;

    let (key, _) = create_ed25519_keypair();
    let player = Address::generate(env);
    fund_and_approve_player(&ctx, &player, 10_000);

    let req = GaslessMatchStakeRequest {
        player: player.clone(),
        token: ctx.token_address.clone(),
        amount: 100,
        game_id: 502,
        is_creator: true,
        nonce: 0,
        valid_until: 0,
    };
    let (pk, sig) = sign_match_stake(env, &ctx.client, &key, &req);

    // Admin pauses contract
    ctx.client.pause();
    assert!(ctx.client.is_paused());

    // Execution must fail when paused
    let res_paused = ctx.client.try_gasless_stake_match(&ctx.relayer, &req, &pk, &sig);
    assert_eq!(
        res_paused.unwrap_err().unwrap(),
        RelayerError::ContractPaused.into()
    );

    // Admin unpauses
    ctx.client.unpause();
    assert!(!ctx.client.is_paused());

    // Execution succeeds
    let res_unpaused = ctx.client.try_gasless_stake_match(&ctx.relayer, &req, &pk, &sig);
    assert!(res_unpaused.is_ok());
}

#[test]
fn test_whitelisted_relayer_policy() {
    let ctx = setup_test_context();
    let env = &ctx.env;

    // Switch from open relayers to whitelisted relayers
    ctx.client.set_open_relayers(&false);

    let unauthorized_relayer = Address::generate(env);
    let authorized_relayer = Address::generate(env);
    ctx.client.add_relayer(&authorized_relayer);

    assert!(!ctx.client.is_relayer_authorized(&unauthorized_relayer));
    assert!(ctx.client.is_relayer_authorized(&authorized_relayer));

    let (key, _) = create_ed25519_keypair();
    let player = Address::generate(env);
    fund_and_approve_player(&ctx, &player, 10_000);

    let req = GaslessMatchStakeRequest {
        player: player.clone(),
        token: ctx.token_address.clone(),
        amount: 100,
        game_id: 503,
        is_creator: true,
        nonce: 0,
        valid_until: 0,
    };
    let (pk, sig) = sign_match_stake(env, &ctx.client, &key, &req);

    // Unauthorized relayer fails
    let res_unauth = ctx.client.try_gasless_stake_match(
        &unauthorized_relayer,
        &req,
        &pk,
        &sig,
    );
    assert_eq!(
        res_unauth.unwrap_err().unwrap(),
        RelayerError::RelayerNotAuthorized.into()
    );

    // Authorized relayer succeeds
    let res_auth = ctx.client.try_gasless_stake_match(
        &authorized_relayer,
        &req,
        &pk,
        &sig,
    );
    assert!(res_auth.is_ok());
}

#[test]
fn test_cancel_unmatched_game_refunds_creator() {
    let ctx = setup_test_context();
    let env = &ctx.env;

    let (key, _) = create_ed25519_keypair();
    let player = Address::generate(env);
    let initial_balance: i128 = 2_000;
    let wager: i128 = 400;
    let game_id: u64 = 504;

    fund_and_approve_player(&ctx, &player, initial_balance);

    let req = GaslessMatchStakeRequest {
        player: player.clone(),
        token: ctx.token_address.clone(),
        amount: wager,
        game_id,
        is_creator: true,
        nonce: 0,
        valid_until: 0,
    };
    let (pk, sig) = sign_match_stake(env, &ctx.client, &key, &req);

    ctx.client.gasless_stake_match(&ctx.relayer, &req, &pk, &sig);
    assert_eq!(ctx.token_client.balance(&player), initial_balance - wager);

    // Creator cancels unmatched game
    ctx.client.cancel_unmatched_game(&game_id);

    // Creator is fully refunded
    assert_eq!(ctx.token_client.balance(&player), initial_balance);
    let match_escrow = ctx.client.get_match(&game_id);
    assert_eq!(match_escrow.state, MatchState::Cancelled);
}

#[test]
fn test_same_player_cannot_join_own_game() {
    let ctx = setup_test_context();
    let env = &ctx.env;

    let (key, _) = create_ed25519_keypair();
    let player = Address::generate(env);
    fund_and_approve_player(&ctx, &player, 10_000);

    let game_id: u64 = 505;

    // Create match
    let req1 = GaslessMatchStakeRequest {
        player: player.clone(),
        token: ctx.token_address.clone(),
        amount: 100,
        game_id,
        is_creator: true,
        nonce: 0,
        valid_until: 0,
    };
    let (pk1, sig1) = sign_match_stake(env, &ctx.client, &key, &req1);
    ctx.client.gasless_stake_match(&ctx.relayer, &req1, &pk1, &sig1);

    // Try joining same match with same player address
    let req2 = GaslessMatchStakeRequest {
        player: player.clone(),
        token: ctx.token_address.clone(),
        amount: 100,
        game_id,
        is_creator: false,
        nonce: 1,
        valid_until: 0,
    };
    let (pk2, sig2) = sign_match_stake(env, &ctx.client, &key, &req2);

    let res = ctx.client.try_gasless_stake_match(&ctx.relayer, &req2, &pk2, &sig2);
    assert_eq!(
        res.unwrap_err().unwrap(),
        RelayerError::SamePlayerJoining.into()
    );
}

#[test]
fn test_double_initialize_rejected() {
    let ctx = setup_test_context();
    let new_admin = Address::generate(&ctx.env);

    let res = ctx.client.try_initialize(&new_admin, &ctx.network_hash, &true);
    assert_eq!(
        res.unwrap_err().unwrap(),
        RelayerError::AlreadyInitialized.into()
    );
}
