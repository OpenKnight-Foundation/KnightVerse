#![cfg(test)]
extern crate std;

use super::*;
use ed25519_dalek::{Signer, SigningKey};
use rand::rngs::OsRng;
use soroban_sdk::token::StellarAssetClient;
use soroban_sdk::{Address, Bytes, BytesN, Env, Map, Vec, testutils::Address as _};

/// Helper: seed a completed game directly into contract storage, bypassing
/// token transfers and auth checks.  Returns the game_id (always 1).
fn seed_completed_game(
    env: &Env,
    contract_id: &Address,
    player1: &Address,
    player2: &Address,
    wager: i128,
) -> u64 {
    let game_id: u64 = 1;
    env.as_contract(contract_id, || {
        // Write game counter
        env.storage().instance().set(&GAME_COUNTER, &game_id);

        // Build a completed game
        let game = Game {
            id: game_id,
            player1: player1.clone(),
            player2: Some(player2.clone()),
            state: GameState::Completed,
            wager_amount: wager,
            current_turn: 1,
            moves: Vec::new(env),
            created_at: 0,
            winner: None,
            last_move_at: 0,
        };
        let mut games: Map<u64, Game> = Map::new(env);
        games.set(game_id, game);
        env.storage().instance().set(&GAMES, &games);

        // Seed escrow so payout_tournament can debit both players
        let mut escrow: Map<Address, i128> = Map::new(env);
        escrow.set(player1.clone(), wager);
        escrow.set(player2.clone(), wager);
        env.storage().instance().set(&ESCROW, &escrow);
    });
    game_id
}

#[test]
fn test_payout_tournament() {
    let env = Env::default();
    let contract_id = env.register_contract(None, GameContract);
    let client = GameContractClient::new(&env, &contract_id);

    let player1 = Address::generate(&env);
    let player2 = Address::generate(&env);
    let wager: i128 = 1000;

    let game_id = seed_completed_game(&env, &contract_id, &player1, &player2, wager);

    let winner1 = Address::generate(&env);
    let winner2 = Address::generate(&env);
    let winner3 = Address::generate(&env);

    let mut winners = Vec::new(&env);
    winners.push_back(winner1.clone());
    winners.push_back(winner2.clone());
    winners.push_back(winner3.clone());

    let mut percentages = Vec::new(&env);
    percentages.push_back(50);
    percentages.push_back(30);
    percentages.push_back(20);

    // Call payout_tournament
    client
        .mock_all_auths()
        .payout_tournament(&game_id, &winners, &percentages);

    // Total pool should be wager * 2 = 2000
    // Expected payouts: 50% = 1000, 30% = 600, 20% = 400
    env.as_contract(&contract_id, || {
        let escrow: Map<Address, i128> = env.storage().instance().get(&ESCROW).unwrap();

        // Assert sum precisely equals total pool
        let w1_escrow = escrow.get(winner1.clone()).unwrap_or(0);
        let w2_escrow = escrow.get(winner2.clone()).unwrap_or(0);
        let w3_escrow = escrow.get(winner3.clone()).unwrap_or(0);

        assert_eq!(w1_escrow, 1000);
        assert_eq!(w2_escrow, 600);
        assert_eq!(w3_escrow, 400);

        // Calculate total sum of payouts
        let total_distributed = w1_escrow + w2_escrow + w3_escrow;
        assert_eq!(total_distributed, (wager * 2) as i128);

        // Player1 and Player2 escrows should be subtracted by wager amount
        let p1_escrow = escrow.get(player1.clone()).unwrap_or(0);
        let p2_escrow = escrow.get(player2.clone()).unwrap_or(0);
        assert_eq!(p1_escrow, 0); // Started as 1000, subtracted 1000
        assert_eq!(p2_escrow, 0); // Started as 1000, subtracted 1000
    });
}

#[test]
fn test_payout_tournament_dust() {
    let env = Env::default();
    let contract_id = env.register_contract(None, GameContract);
    let client = GameContractClient::new(&env, &contract_id);

    let player1 = Address::generate(&env);
    let player2 = Address::generate(&env);

    // An amount that creates an uneven division for testing "precision" remainder distribution
    let wager: i128 = 333; // total pool = 666

    let game_id = seed_completed_game(&env, &contract_id, &player1, &player2, wager);

    let winner1 = Address::generate(&env);
    let winner2 = Address::generate(&env);
    let winner3 = Address::generate(&env);

    let mut winners = Vec::new(&env);
    winners.push_back(winner1.clone());
    winners.push_back(winner2.clone());
    winners.push_back(winner3.clone());

    let mut percentages = Vec::new(&env);
    percentages.push_back(50); // 333
    percentages.push_back(30); // 199.8 -> 199
    percentages.push_back(20); // 133.2 -> 133
    // Sum without remainder distribution: 333 + 199 + 133 = 665
    // Remainder: 666 - 665 = 1
    // With remainder to first place: w1 gets 333 + 1 = 334.

    client
        .mock_all_auths()
        .payout_tournament(&game_id, &winners, &percentages);

    env.as_contract(&contract_id, || {
        let escrow: Map<Address, i128> = env.storage().instance().get(&ESCROW).unwrap();

        let w1_escrow = escrow.get(winner1.clone()).unwrap_or(0);
        let w2_escrow = escrow.get(winner2.clone()).unwrap_or(0);
        let w3_escrow = escrow.get(winner3.clone()).unwrap_or(0);

        assert_eq!(w1_escrow, 334);
        assert_eq!(w2_escrow, 199);
        assert_eq!(w3_escrow, 133);

        let total_distributed = w1_escrow + w2_escrow + w3_escrow;
        assert_eq!(total_distributed, (wager * 2) as i128); // 666
    });
}

#[test]
fn test_payout_tournament_invalid_percentage() {
    let env = Env::default();
    let contract_id = env.register_contract(None, GameContract);
    let client = GameContractClient::new(&env, &contract_id);

    let player1 = Address::generate(&env);
    let player2 = Address::generate(&env);
    let wager: i128 = 1000;

    let game_id = seed_completed_game(&env, &contract_id, &player1, &player2, wager);

    let winner1 = Address::generate(&env);

    let mut winners = Vec::new(&env);
    winners.push_back(winner1.clone());

    let mut percentages = Vec::new(&env);
    percentages.push_back(90); // Does not equal 100

    let res = client
        .mock_all_auths()
        .try_payout_tournament(&game_id, &winners, &percentages);

    // Result should be Err matching InvalidPercentage (12)
    assert!(res.is_err());
}

#[test]
fn test_create_game_exceeds_max_stake() {
    let env = Env::default();
    let contract_id = env.register_contract(None, GameContract);
    let client = GameContractClient::new(&env, &contract_id);

    let player1 = Address::generate(&env);
    let wager = 1001; // Exceeds default 1000

    let res = client.try_create_game(&player1, &wager);
    assert!(res.is_err());

    // The error should be StakeLimitExceeded (15)
    // We can check the error code if we want to be precise:
    // let err = res.err().unwrap();
    // assert!(err.get_code() == 15);
}

#[test]
fn test_set_max_stake() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, GameContract);
    let client = GameContractClient::new(&env, &contract_id);

    let issuer = Address::generate(&env);
    let player1 = Address::generate(&env);

    // Setup token
    let stellar_token = env.register_stellar_asset_contract_v2(issuer.clone());
    let token_address = stellar_token.address();
    let stellar_asset_client = soroban_sdk::token::StellarAssetClient::new(&env, &token_address);

    // Mint player balance
    stellar_asset_client.mint(&player1, &1000);

    // Initialize game contract with token
    let admin = Address::generate(&env);
    let treasury_addr = Address::generate(&env);
    client.initialize_token(&admin, &token_address);
    let admin_key = Bytes::from_slice(&env, &[0u8; 32]);
    client.initialize_puzzle_rewards(&admin, &admin_key, &0i128, &0u32, &treasury_addr);

    // Set limit to 500
    client.set_max_stake(&admin, &500);

    // Try to create game with 600
    let res = client.try_create_game(&player1, &600);
    assert!(res.is_err());

    // Try to create game with 500
    let game_id_res = client.try_create_game(&player1, &500);
    assert!(game_id_res.is_ok());
}

#[test]
fn test_set_max_stake_rejects_non_admin() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, GameContract);
    let client = GameContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let attacker = Address::generate(&env);
    let treasury_addr = Address::generate(&env);
    let admin_key = Bytes::from_slice(&env, &[0u8; 32]);

    client.initialize_puzzle_rewards(&admin, &admin_key, &0i128, &0u32, &treasury_addr);
    let res = client.try_set_max_stake(&attacker, &1_000i128);
    assert!(res.is_err());
}

#[test]
fn test_payout_with_fee() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, GameContract);
    let client = GameContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let issuer = Address::generate(&env);
    let player1 = Address::generate(&env);
    let player2 = Address::generate(&env);
    let treasury_addr = Address::generate(&env);

    // Register token contract
    let stellar_token = env.register_stellar_asset_contract_v2(issuer);
    let token_address = stellar_token.address();
    let stellar_asset_client = StellarAssetClient::new(&env, &token_address);

    // Initialize Game Contract with token
    client.initialize_token(&admin, &token_address);

    // Initialize Puzzle Rewards/Fees
    let admin_key = Bytes::from_slice(&env, &[0u8; 32]);
    client.initialize_puzzle_rewards(&admin, &admin_key, &0i128, &20u32, &treasury_addr); // 2% fee (20 bips)

    let wager = 500; // Total pool 1000
    stellar_asset_client.mint(&player1, &wager);
    stellar_asset_client.mint(&player2, &wager);

    let game_id = client.create_game(&player1, &wager);
    client.join_game(&game_id, &player2);

    // Force complete the game and set winner
    env.as_contract(&contract_id, || {
        let mut games: Map<u64, Game> = env.storage().instance().get(&GAMES).unwrap();
        let mut game = games.get(game_id).unwrap();
        game.state = GameState::Completed;
        game.winner = Some(player1.clone());
        games.set(game_id, game);
        env.storage().instance().set(&GAMES, &games);
    });

    client.payout(&game_id, &player1);

    env.as_contract(&contract_id, || {
        let escrow: Map<Address, i128> = env.storage().instance().get(&ESCROW).unwrap();
        let winner_escrow = escrow.get(player1.clone()).unwrap_or(0);
        let treasury_escrow = escrow.get(treasury_addr.clone()).unwrap_or(0);
        let loser_escrow = escrow.get(player2.clone()).unwrap_or(0);

        assert_eq!(winner_escrow, 980);
        assert_eq!(treasury_escrow, 20);
        assert_eq!(loser_escrow, 0);
    });
}

#[test]
fn test_configure_fees_permissioned() {
    let env = Env::default();
    let contract_id = env.register_contract(None, GameContract);
    let client = GameContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let treasury_addr = Address::generate(&env);
    let admin_key = Bytes::from_slice(&env, &[0u8; 32]);

    env.mock_all_auths();
    client.initialize_puzzle_rewards(&admin, &admin_key, &0i128, &0u32, &treasury_addr);

    // Update fees as admin
    let new_treasury = Address::generate(&env);
    let mut admins = Vec::new(&env);
    admins.push_back(admin.clone());
    client.configure_fees(&admins, &50, &new_treasury); // 5% fee

    // Verify update
    // (In a real test we'd check storage or run a payout, but here we just ensure it doesn't panic)

    // Attempt update as someone else should panic
    let stranger = Address::generate(&env);
    let mut stranger_admins = Vec::new(&env);
    stranger_admins.push_back(stranger.clone());
    let res = client.try_configure_fees(&stranger_admins, &100, &new_treasury);
    assert!(res.is_err());
}

#[test]
fn test_upgrade_admin_logic() {
    let env = Env::default();
    let contract_id = env.register_contract(None, GameContract);
    let client = GameContractClient::new(&env, &contract_id);

    let admin_key = Bytes::from_slice(&env, &[0u8; 32]);

    // Manually set ADMIN_KEY to simulate old initialization (pre-CONTRACT_ADMIN)
    env.as_contract(&contract_id, || {
        env.storage()
            .instance()
            .set(&symbol_short!("ADMIN_KEY"), &admin_key);
    });

    let admin = Address::generate(&env);
    env.mock_all_auths();

    // upgrade_admin should allow setting the admin for the first time
    client.upgrade_admin(&admin);

    // Further calls to upgrade_admin should panic
    let stranger = Address::generate(&env);
    let res = client.try_upgrade_admin(&stranger);
    assert!(res.is_err());
}

// ── SEP-10 Challenge Verification Tests (#529) ────────────────────────────

/// Helper: initialise the contract with a zeroed admin key (sufficient for
/// storage-only tests that don't exercise ed25519_verify).
fn init_contract(env: &Env, contract_id: &Address) -> (Address, Address) {
    let client = GameContractClient::new(env, contract_id);
    let admin = Address::generate(env);
    let treasury = Address::generate(env);
    let admin_key = Bytes::from_slice(env, &[0u8; 32]);
    env.mock_all_auths();
    client.initialize_puzzle_rewards(&admin, &admin_key, &0i128, &0u32, &treasury);
    (admin, treasury)
}

/// Helper: initialise the contract with a real ed25519 signing key.
/// Returns (admin, treasury, signing_key).
fn init_contract_with_key(
    env: &Env,
    contract_id: &Address,
) -> (Address, Address, SigningKey) {
    let client = GameContractClient::new(env, contract_id);
    let admin = Address::generate(env);
    let treasury = Address::generate(env);
    let signing_key = SigningKey::generate(&mut OsRng);
    let admin_key = Bytes::from_slice(env, &signing_key.verifying_key().to_bytes());
    env.mock_all_auths();
    client.initialize_puzzle_rewards(&admin, &admin_key, &0i128, &0u32, &treasury);
    (admin, treasury, signing_key)
}

/// Sign the SEP-10 payload: SHA256(address_bytes || nonce_bytes || expiry_le8)
fn sign_sep10_payload(
    env: &Env,
    signing_key: &SigningKey,
    account: &Address,
    nonce: &BytesN<32>,
    expiry: u64,
) -> BytesN<64> {
    let mut payload = Bytes::new(env);

    let account_str = account.clone().to_string();
    let str_len = account_str.len() as usize;
    let mut addr_buf = [0u8; 64];
    account_str.copy_into_slice(&mut addr_buf[..str_len]);
    payload.append(&Bytes::from_slice(env, &addr_buf[..str_len]));

    let nonce_bytes: Bytes = nonce.clone().into();
    payload.append(&nonce_bytes);

    payload.append(&Bytes::from_slice(env, &expiry.to_le_bytes()));

    let digest: BytesN<32> = env.crypto().sha256(&payload).into();
    let mut digest_raw = [0u8; 32];
    digest.copy_into_slice(&mut digest_raw);

    BytesN::from_array(env, &signing_key.sign(&digest_raw).to_bytes())
}

#[test]
fn test_sep10_issue_and_verify_marks_account_verified() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, GameContract);
    let client = GameContractClient::new(&env, &contract_id);
    let (admin, _, signing_key) = init_contract_with_key(&env, &contract_id);

    let account = Address::generate(&env);
    let nonce = BytesN::from_array(&env, &[1u8; 32]);
    let expiry: u64 = env.ledger().sequence() as u64 + 1000;

    client.issue_sep10_challenge(&admin, &account, &nonce, &expiry);

    assert!(!client.is_sep10_verified(&account));

    let sig = sign_sep10_payload(&env, &signing_key, &account, &nonce, expiry);
    client.verify_sep10_challenge(&account, &nonce, &sig);

    assert!(client.is_sep10_verified(&account));
}

#[test]
fn test_sep10_challenge_cannot_be_reused() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, GameContract);
    let client = GameContractClient::new(&env, &contract_id);
    let (admin, _, signing_key) = init_contract_with_key(&env, &contract_id);

    let account = Address::generate(&env);
    let nonce = BytesN::from_array(&env, &[2u8; 32]);
    let expiry: u64 = env.ledger().sequence() as u64 + 1000;

    client.issue_sep10_challenge(&admin, &account, &nonce, &expiry);

    let sig = sign_sep10_payload(&env, &signing_key, &account, &nonce, expiry);
    client.verify_sep10_challenge(&account, &nonce, &sig);

    // Second verification with the same nonce must fail (challenge consumed)
    let res = client.try_verify_sep10_challenge(&account, &nonce, &sig);
    assert!(res.is_err());
}

#[test]
fn test_sep10_duplicate_nonce_rejected_on_issue() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, GameContract);
    let client = GameContractClient::new(&env, &contract_id);
    let (admin, _) = init_contract(&env, &contract_id);

    let account = Address::generate(&env);
    let nonce = BytesN::from_array(&env, &[3u8; 32]);
    let expiry: u64 = env.ledger().sequence() as u64 + 1000;

    client.issue_sep10_challenge(&admin, &account, &nonce, &expiry);

    // Issuing the same nonce again must fail
    let res = client.try_issue_sep10_challenge(&admin, &account, &nonce, &expiry);
    assert!(res.is_err());
}

// ── Multi-Sig Fee Control Tests (#535) ────────────────────────────────────

#[test]
fn test_multisig_fee_change_requires_threshold() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, GameContract);
    let client = GameContractClient::new(&env, &contract_id);
    let (admin, _) = init_contract(&env, &contract_id);

    let signer1 = Address::generate(&env);
    let signer2 = Address::generate(&env);
    let signer3 = Address::generate(&env);

    let mut signers = Vec::new(&env);
    signers.push_back(signer1.clone());
    signers.push_back(signer2.clone());
    signers.push_back(signer3.clone());

    // 2-of-3 threshold
    client.configure_multisig(&admin, &signers, &2u32);

    let new_treasury = Address::generate(&env);

    // signer1 proposes (auto-approved for proposer)
    client.propose_fee_change(&signer1, &50u32, &new_treasury);

    // One approval so far — proposal should NOT be executed yet
    let executed = client.approve_fee_proposal(&signer2);
    assert!(executed); // threshold reached with signer1 (proposer) + signer2

    // Proposal should be cleared after execution
    let proposal = client.get_fee_proposal();
    assert!(proposal.is_none());
}

#[test]
fn test_multisig_single_approval_not_enough_for_threshold_3() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, GameContract);
    let client = GameContractClient::new(&env, &contract_id);
    let (admin, _) = init_contract(&env, &contract_id);

    let signer1 = Address::generate(&env);
    let signer2 = Address::generate(&env);
    let signer3 = Address::generate(&env);

    let mut signers = Vec::new(&env);
    signers.push_back(signer1.clone());
    signers.push_back(signer2.clone());
    signers.push_back(signer3.clone());

    // 3-of-3 threshold
    client.configure_multisig(&admin, &signers, &3u32);

    let new_treasury = Address::generate(&env);
    client.propose_fee_change(&signer1, &30u32, &new_treasury);

    // signer2 approves — still 2/3, not executed
    let executed = client.approve_fee_proposal(&signer2);
    assert!(!executed);

    // Proposal still pending
    assert!(client.get_fee_proposal().is_some());
    assert_eq!(client.get_approval_count(), 2u32);
}

#[test]
fn test_multisig_cancel_clears_proposal() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, GameContract);
    let client = GameContractClient::new(&env, &contract_id);
    let (admin, _) = init_contract(&env, &contract_id);

    let signer1 = Address::generate(&env);
    let mut signers = Vec::new(&env);
    signers.push_back(signer1.clone());
    client.configure_multisig(&admin, &signers, &1u32);

    let new_treasury = Address::generate(&env);
    client.propose_fee_change(&signer1, &10u32, &new_treasury);
    assert!(client.get_fee_proposal().is_some());

    client.cancel_fee_proposal(&signer1);
    assert!(client.get_fee_proposal().is_none());
}

#[test]
fn test_multisig_non_signer_cannot_propose() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, GameContract);
    let client = GameContractClient::new(&env, &contract_id);
    let (admin, _) = init_contract(&env, &contract_id);

    let signer1 = Address::generate(&env);
    let mut signers = Vec::new(&env);
    signers.push_back(signer1.clone());
    client.configure_multisig(&admin, &signers, &1u32);

    let attacker = Address::generate(&env);
    let new_treasury = Address::generate(&env);
    let res = client.try_propose_fee_change(&attacker, &10u32, &new_treasury);
    assert!(res.is_err());
}

#[test]
fn test_multisig_invalid_threshold_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, GameContract);
    let client = GameContractClient::new(&env, &contract_id);
    let (admin, _) = init_contract(&env, &contract_id);

    let signer1 = Address::generate(&env);
    let mut signers = Vec::new(&env);
    signers.push_back(signer1.clone());

    // threshold > signers.len() must fail
    let res = client.try_configure_multisig(&admin, &signers, &5u32);
    assert!(res.is_err());

    // threshold = 0 must fail
    let res2 = client.try_configure_multisig(&admin, &signers, &0u32);
    assert!(res2.is_err());
}

// ── Issue #534: Move Sequence Unit Tests ──────────────────────────────────────

fn setup_in_progress_game<'a>(
    env: &'a Env,
    contract_id: &'a Address,
) -> (GameContractClient<'a>, Address, Address, u64) {
    let client = GameContractClient::new(env, contract_id);
    let admin = Address::generate(env);
    let issuer = Address::generate(env);
    let player1 = Address::generate(env);
    let player2 = Address::generate(env);
    let treasury_addr = Address::generate(env);

    let stellar_token = env.register_stellar_asset_contract_v2(issuer);
    let token_address = stellar_token.address();
    let stellar_asset_client = StellarAssetClient::new(env, &token_address);

    client.initialize_token(&admin, &token_address);
    client.initialize_puzzle_rewards(
        &admin,
        &Bytes::from_slice(env, &[0u8; 32]),
        &0i128,
        &0u32,
        &treasury_addr,
    );

    let wager = 100;
    stellar_asset_client.mint(&player1, &wager);
    stellar_asset_client.mint(&player2, &wager);

    let game_id = client.create_game(&player1, &wager);
    client.join_game(&game_id, &player2);

    (client, player1, player2, game_id)
}

#[test]
fn test_submit_move_normal() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, GameContract);
    let (client, player1, _player2, game_id) = setup_in_progress_game(&env, &contract_id);

    let move_data = Vec::from_array(&env, [1u32, 2u32, 3u32]);
    client.submit_move(&game_id, &player1, &move_data);

    let game = client.get_game(&game_id);
    assert_eq!(game.moves.len(), 1);
    assert_eq!(game.current_turn, 2);
}

#[test]
fn test_submit_move_wrong_turn() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, GameContract);
    let (client, _player1, player2, game_id) = setup_in_progress_game(&env, &contract_id);

    let move_data = Vec::from_array(&env, [1u32, 2u32]);
    let res = client.try_submit_move(&game_id, &player2, &move_data);
    assert_eq!(res, Err(Ok(ContractError::NotYourTurn)));
}

#[test]
fn test_submit_move_not_a_player() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, GameContract);
    let (client, _player1, _player2, game_id) = setup_in_progress_game(&env, &contract_id);

    let outsider = Address::generate(&env);
    let move_data = Vec::from_array(&env, [1u32, 2u32]);
    let res = client.try_submit_move(&game_id, &outsider, &move_data);
    assert_eq!(res, Err(Ok(ContractError::NotPlayer)));
}

#[test]
fn test_submit_move_empty_move_data() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, GameContract);
    let (client, player1, _player2, game_id) = setup_in_progress_game(&env, &contract_id);

    let empty_move: Vec<u32> = Vec::new(&env);
    let res = client.try_submit_move(&game_id, &player1, &empty_move);
    assert_eq!(res, Err(Ok(ContractError::InvalidMove)));
}

#[test]
fn test_submit_move_game_not_in_progress() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, GameContract);
    let client = GameContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let issuer = Address::generate(&env);
    let player1 = Address::generate(&env);
    let treasury_addr = Address::generate(&env);

    let stellar_token = env.register_stellar_asset_contract_v2(issuer);
    let token_address = stellar_token.address();
    let stellar_asset_client = StellarAssetClient::new(&env, &token_address);

    client.initialize_token(&admin, &token_address);
    client.initialize_puzzle_rewards(
        &admin,
        &Bytes::from_slice(&env, &[0u8; 32]),
        &0i128,
        &0u32,
        &treasury_addr,
    );

    let wager = 100;
    stellar_asset_client.mint(&player1, &wager);
    let game_id = client.create_game(&player1, &wager);

    let move_data = Vec::from_array(&env, [1u32, 2u32]);
    let res = client.try_submit_move(&game_id, &player1, &move_data);
    assert_eq!(res, Err(Ok(ContractError::GameNotInProgress)));
}

#[test]
fn test_submit_move_sequence_alternation() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, GameContract);
    let (client, player1, player2, game_id) = setup_in_progress_game(&env, &contract_id);

    client.submit_move(&game_id, &player1, &Vec::from_array(&env, [1u32]));
    client.submit_move(&game_id, &player2, &Vec::from_array(&env, [2u32]));
    client.submit_move(&game_id, &player1, &Vec::from_array(&env, [3u32]));
    client.submit_move(&game_id, &player2, &Vec::from_array(&env, [4u32]));

    let game = client.get_game(&game_id);
    assert_eq!(game.moves.len(), 4);
    assert_eq!(game.current_turn, 1);
}

#[test]
fn test_submit_move_player1_cannot_move_twice() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, GameContract);
    let (client, player1, _player2, game_id) = setup_in_progress_game(&env, &contract_id);

    client.submit_move(&game_id, &player1, &Vec::from_array(&env, [1u32]));
    let res = client.try_submit_move(&game_id, &player1, &Vec::from_array(&env, [2u32]));
    assert_eq!(res, Err(Ok(ContractError::NotYourTurn)));
}
