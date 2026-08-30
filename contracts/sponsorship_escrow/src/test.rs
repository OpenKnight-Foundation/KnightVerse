use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::{Client as TokenClient, StellarAssetClient},
    Address, Env, Vec,
};

#[allow(dead_code)]
struct TestContext {
    env: Env,
    contract_id: Address,
    client: SponsorshipEscrowContractClient<'static>,
    admin: Address,
    organizer: Address,
    oracle: Address,
    sponsor_a: Address,
    sponsor_b: Address,
    reg_recipient: Address,
    quarter_recipient: Address,
    grand_winner: Address,
    xlm_token: Address,
    xlm_admin: StellarAssetClient<'static>,
    xlm_client: TokenClient<'static>,
    usdc_token: Address,
    usdc_admin: StellarAssetClient<'static>,
    usdc_client: TokenClient<'static>,
}

fn setup_test_context() -> TestContext {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, SponsorshipEscrowContract);
    let client = SponsorshipEscrowContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let organizer = Address::generate(&env);
    let oracle = Address::generate(&env);
    let sponsor_a = Address::generate(&env);
    let sponsor_b = Address::generate(&env);
    let reg_recipient = Address::generate(&env);
    let quarter_recipient = Address::generate(&env);
    let grand_winner = Address::generate(&env);

    client.initialize(&admin);

    // Setup XLM mock token
    let xlm_admin_addr = Address::generate(&env);
    let xlm_contract = env.register_stellar_asset_contract_v2(xlm_admin_addr.clone());
    let xlm_token = xlm_contract.address();
    let xlm_admin = StellarAssetClient::new(&env, &xlm_token);
    let xlm_client = TokenClient::new(&env, &xlm_token);

    // Setup USDC mock token
    let usdc_admin_addr = Address::generate(&env);
    let usdc_contract = env.register_stellar_asset_contract_v2(usdc_admin_addr.clone());
    let usdc_token = usdc_contract.address();
    let usdc_admin = StellarAssetClient::new(&env, &usdc_token);
    let usdc_client = TokenClient::new(&env, &usdc_token);

    // Mint tokens to sponsors
    xlm_admin.mint(&sponsor_a, &100_000);
    xlm_admin.mint(&sponsor_b, &100_000);
    usdc_admin.mint(&sponsor_a, &50_000);
    usdc_admin.mint(&sponsor_b, &50_000);

    TestContext {
        env,
        contract_id,
        client,
        admin,
        organizer,
        oracle,
        sponsor_a,
        sponsor_b,
        reg_recipient,
        quarter_recipient,
        grand_winner,
        xlm_token,
        xlm_admin,
        xlm_client,
        usdc_token,
        usdc_admin,
        usdc_client,
    }
}

#[test]
fn test_initialize_and_admin_controls() {
    let ctx = setup_test_context();

    assert_eq!(ctx.client.get_admin(), ctx.admin);
    assert!(!ctx.client.is_contract_paused());

    // Pause contract
    ctx.client.pause(&ctx.admin);
    assert!(ctx.client.is_contract_paused());

    // Unpause contract
    ctx.client.unpause(&ctx.admin);
    assert!(!ctx.client.is_contract_paused());

    // Transfer admin
    let new_admin = Address::generate(&ctx.env);
    ctx.client.transfer_admin(&ctx.admin, &new_admin);
    assert_eq!(ctx.client.get_admin(), new_admin);
}

#[test]
fn test_standard_tournament_creation_and_view() {
    let ctx = setup_test_context();
    let tournament_id = 101u64;

    ctx.client.create_standard_tournament(
        &ctx.organizer,
        &tournament_id,
        &ctx.oracle,
        &500, // kickoff_deadline
        &Some(ctx.reg_recipient.clone()),
        &Some(ctx.quarter_recipient.clone()),
        &Some(ctx.grand_winner.clone()),
    );

    let t = ctx.client.get_tournament(&tournament_id).unwrap();
    assert_eq!(t.id, tournament_id);
    assert_eq!(t.organizer, ctx.organizer);
    assert_eq!(t.oracle, ctx.oracle);
    assert_eq!(t.status, TournamentStatus::AcceptingDeposits);
    assert_eq!(t.total_milestones, 3);
    assert_eq!(t.current_stage_index, 0);
    assert_eq!(t.kickoff_deadline, 500);

    let ms0 = ctx.client.get_milestone_config(&tournament_id, &0).unwrap();
    assert_eq!(ms0.stage_id, StandardStage::RegistrationComplete as u32);
    assert_eq!(ms0.basis_points, 2000); // 20%

    let ms1 = ctx.client.get_milestone_config(&tournament_id, &1).unwrap();
    assert_eq!(ms1.stage_id, StandardStage::QuarterFinals as u32);
    assert_eq!(ms1.basis_points, 3000); // 30%

    let ms2 = ctx.client.get_milestone_config(&tournament_id, &2).unwrap();
    assert_eq!(ms2.stage_id, StandardStage::GrandFinal as u32);
    assert_eq!(ms2.basis_points, 5000); // 50%

    let all_states = ctx.client.get_all_milestone_states(&tournament_id);
    assert_eq!(all_states.len(), 3);
    assert!(!all_states.get(0).unwrap().completed);
    assert!(!all_states.get(1).unwrap().completed);
    assert!(!all_states.get(2).unwrap().completed);
}

#[test]
fn test_multi_token_deposits_and_milestone_payouts() {
    let ctx = setup_test_context();
    let tournament_id = 201u64;

    // Create tournament with 3 stages: 20% (Reg), 30% (Quarter), 50% (Grand Final)
    ctx.client.create_standard_tournament(
        &ctx.organizer,
        &tournament_id,
        &ctx.oracle,
        &0,
        &Some(ctx.reg_recipient.clone()),
        &Some(ctx.quarter_recipient.clone()),
        &Some(ctx.grand_winner.clone()),
    );

    // Sponsor A deposits 10,000 XLM and 5,000 USDC
    ctx.client.deposit_sponsorship(
        &tournament_id,
        &ctx.sponsor_a,
        &ctx.xlm_token,
        &10_000,
    );
    ctx.client.deposit_sponsorship(
        &tournament_id,
        &ctx.sponsor_a,
        &ctx.usdc_token,
        &5_000,
    );

    // Sponsor B deposits 20,000 XLM and 15,000 USDC
    ctx.client.deposit_sponsorship(
        &tournament_id,
        &ctx.sponsor_b,
        &ctx.xlm_token,
        &20_000,
    );
    ctx.client.deposit_sponsorship(
        &tournament_id,
        &ctx.sponsor_b,
        &ctx.usdc_token,
        &15_000,
    );

    // Verify deposits: Total XLM = 30,000, Total USDC = 20,000
    assert_eq!(ctx.client.get_total_deposited(&tournament_id, &ctx.xlm_token), 30_000);
    assert_eq!(ctx.client.get_total_deposited(&tournament_id, &ctx.usdc_token), 20_000);
    assert_eq!(ctx.client.get_remaining_balance(&tournament_id, &ctx.xlm_token), 30_000);
    assert_eq!(ctx.client.get_remaining_balance(&tournament_id, &ctx.usdc_token), 20_000);

    assert_eq!(
        ctx.client.get_sponsor_deposit(&tournament_id, &ctx.sponsor_a, &ctx.xlm_token),
        10_000
    );
    assert_eq!(
        ctx.client.get_sponsor_deposit(&tournament_id, &ctx.sponsor_b, &ctx.usdc_token),
        15_000
    );

    let tokens = ctx.client.get_tournament_tokens(&tournament_id);
    assert_eq!(tokens.len(), 2);

    let sponsors = ctx.client.get_tournament_sponsors(&tournament_id);
    assert_eq!(sponsors.len(), 2);

    // =========================================================================
    // STAGE 0: Registration Complete (20% disbursement)
    // =========================================================================
    ctx.client.complete_stage(&tournament_id, &0, &None);

    let t_after_s0 = ctx.client.get_tournament(&tournament_id).unwrap();
    assert_eq!(t_after_s0.status, TournamentStatus::Active); // Transitioned to Active on kickoff
    assert_eq!(t_after_s0.current_stage_index, 1);

    // Reg recipient should receive 20% of 30,000 XLM = 6,000 XLM, and 20% of 20,000 USDC = 4,000 USDC
    assert_eq!(ctx.xlm_client.balance(&ctx.reg_recipient), 6_000);
    assert_eq!(ctx.usdc_client.balance(&ctx.reg_recipient), 4_000);

    assert_eq!(ctx.client.get_remaining_balance(&tournament_id, &ctx.xlm_token), 24_000);
    assert_eq!(ctx.client.get_remaining_balance(&tournament_id, &ctx.usdc_token), 16_000);
    assert_eq!(ctx.client.get_total_disbursed(&tournament_id, &ctx.xlm_token), 6_000);
    assert_eq!(ctx.client.get_total_disbursed(&tournament_id, &ctx.usdc_token), 4_000);

    let ms0_state = ctx.client.get_milestone_state(&tournament_id, &0).unwrap();
    assert!(ms0_state.completed);
    assert_eq!(ms0_state.recipient, Some(ctx.reg_recipient.clone()));

    // =========================================================================
    // STAGE 1: Quarter-Finals (30% disbursement)
    // =========================================================================
    ctx.client.complete_stage(&tournament_id, &1, &None);

    let t_after_s1 = ctx.client.get_tournament(&tournament_id).unwrap();
    assert_eq!(t_after_s1.status, TournamentStatus::Active);
    assert_eq!(t_after_s1.current_stage_index, 2);

    // Quarter recipient should receive 30% of 30,000 XLM = 9,000 XLM, and 30% of 20,000 USDC = 6,000 USDC
    assert_eq!(ctx.xlm_client.balance(&ctx.quarter_recipient), 9_000);
    assert_eq!(ctx.usdc_client.balance(&ctx.quarter_recipient), 6_000);

    assert_eq!(ctx.client.get_remaining_balance(&tournament_id, &ctx.xlm_token), 15_000);
    assert_eq!(ctx.client.get_remaining_balance(&tournament_id, &ctx.usdc_token), 10_000);

    // =========================================================================
    // STAGE 2: Grand Final (50% disbursement with dynamic winner override)
    // =========================================================================
    let champion = Address::generate(&ctx.env);
    ctx.client.complete_stage(&tournament_id, &2, &Some(champion.clone()));

    let t_after_s2 = ctx.client.get_tournament(&tournament_id).unwrap();
    assert_eq!(t_after_s2.status, TournamentStatus::Completed);
    assert_eq!(t_after_s2.current_stage_index, 3);

    // Champion should receive remaining 50% = 15,000 XLM and 10,000 USDC
    assert_eq!(ctx.xlm_client.balance(&champion), 15_000);
    assert_eq!(ctx.usdc_client.balance(&champion), 10_000);

    // Total escrow remaining balance should now be exactly 0
    assert_eq!(ctx.client.get_remaining_balance(&tournament_id, &ctx.xlm_token), 0);
    assert_eq!(ctx.client.get_remaining_balance(&tournament_id, &ctx.usdc_token), 0);
    assert_eq!(ctx.client.get_total_disbursed(&tournament_id, &ctx.xlm_token), 30_000);
    assert_eq!(ctx.client.get_total_disbursed(&tournament_id, &ctx.usdc_token), 20_000);

    let ms2_state = ctx.client.get_milestone_state(&tournament_id, &2).unwrap();
    assert!(ms2_state.completed);
    assert_eq!(ms2_state.recipient, Some(champion));
}

#[test]
fn test_sponsor_refund_when_cancelled_before_kickoff() {
    let ctx = setup_test_context();
    let tournament_id = 301u64;

    ctx.client.create_standard_tournament(
        &ctx.organizer,
        &tournament_id,
        &ctx.oracle,
        &1000,
        &Some(ctx.reg_recipient.clone()),
        &Some(ctx.quarter_recipient.clone()),
        &Some(ctx.grand_winner.clone()),
    );

    // Initial sponsor balances: A has 100k XLM, 50k USDC; B has 100k XLM, 50k USDC
    ctx.client.deposit_sponsorship(
        &tournament_id,
        &ctx.sponsor_a,
        &ctx.xlm_token,
        &12_000,
    );
    ctx.client.deposit_sponsorship(
        &tournament_id,
        &ctx.sponsor_a,
        &ctx.usdc_token,
        &8_000,
    );

    ctx.client.deposit_sponsorship(
        &tournament_id,
        &ctx.sponsor_b,
        &ctx.xlm_token,
        &25_000,
    );
    ctx.client.deposit_sponsorship(
        &tournament_id,
        &ctx.sponsor_b,
        &ctx.usdc_token,
        &15_000,
    );

    assert_eq!(ctx.xlm_client.balance(&ctx.sponsor_a), 88_000);
    assert_eq!(ctx.usdc_client.balance(&ctx.sponsor_a), 42_000);
    assert_eq!(ctx.xlm_client.balance(&ctx.sponsor_b), 75_000);
    assert_eq!(ctx.usdc_client.balance(&ctx.sponsor_b), 35_000);

    // Organizer cancels tournament before kickoff
    ctx.client.cancel_tournament(&tournament_id, &ctx.organizer);

    let t = ctx.client.get_tournament(&tournament_id).unwrap();
    assert_eq!(t.status, TournamentStatus::Cancelled);

    // Sponsor A claims refund
    ctx.client.claim_refund(&tournament_id, &ctx.sponsor_a);

    // Sponsor A should be fully restored to 100,000 XLM and 50,000 USDC
    assert_eq!(ctx.xlm_client.balance(&ctx.sponsor_a), 100_000);
    assert_eq!(ctx.usdc_client.balance(&ctx.sponsor_a), 50_000);

    // Sponsor B claims refund
    ctx.client.claim_refund(&tournament_id, &ctx.sponsor_b);

    // Sponsor B should be fully restored to 100,000 XLM and 50,000 USDC
    assert_eq!(ctx.xlm_client.balance(&ctx.sponsor_b), 100_000);
    assert_eq!(ctx.usdc_client.balance(&ctx.sponsor_b), 50_000);

    // Contract remaining balances should be 0
    assert_eq!(ctx.client.get_remaining_balance(&tournament_id, &ctx.xlm_token), 0);
    assert_eq!(ctx.client.get_remaining_balance(&tournament_id, &ctx.usdc_token), 0);
}

#[test]
fn test_admin_emergency_batch_refund_all_sponsors() {
    let ctx = setup_test_context();
    let tournament_id = 401u64;

    ctx.client.create_standard_tournament(
        &ctx.organizer,
        &tournament_id,
        &ctx.oracle,
        &0,
        &Some(ctx.reg_recipient.clone()),
        &Some(ctx.quarter_recipient.clone()),
        &Some(ctx.grand_winner.clone()),
    );

    ctx.client.deposit_sponsorship(
        &tournament_id,
        &ctx.sponsor_a,
        &ctx.xlm_token,
        &5_000,
    );
    ctx.client.deposit_sponsorship(
        &tournament_id,
        &ctx.sponsor_b,
        &ctx.usdc_token,
        &10_000,
    );

    // Admin cancels tournament
    ctx.client.cancel_tournament(&tournament_id, &ctx.admin);

    // Admin triggers batch refund for all sponsors
    ctx.client.admin_refund_all_sponsors(&tournament_id, &ctx.admin);

    assert_eq!(ctx.xlm_client.balance(&ctx.sponsor_a), 100_000);
    assert_eq!(ctx.usdc_client.balance(&ctx.sponsor_b), 50_000);
    assert_eq!(ctx.client.get_remaining_balance(&tournament_id, &ctx.xlm_token), 0);
    assert_eq!(ctx.client.get_remaining_balance(&tournament_id, &ctx.usdc_token), 0);
}

#[test]
fn test_kickoff_deadline_timeout_cancellation() {
    let ctx = setup_test_context();
    let tournament_id = 501u64;
    let deadline = 100u64;

    ctx.client.create_standard_tournament(
        &ctx.organizer,
        &tournament_id,
        &ctx.oracle,
        &deadline,
        &Some(ctx.reg_recipient.clone()),
        &Some(ctx.quarter_recipient.clone()),
        &Some(ctx.grand_winner.clone()),
    );

    ctx.client.deposit_sponsorship(
        &tournament_id,
        &ctx.sponsor_a,
        &ctx.xlm_token,
        &7_000,
    );

    // Advance ledger past kickoff deadline
    ctx.env.ledger().set_sequence_number(150);

    let random_caller = Address::generate(&ctx.env);
    ctx.client.cancel_if_deadline_passed(&tournament_id, &random_caller);

    let t = ctx.client.get_tournament(&tournament_id).unwrap();
    assert_eq!(t.status, TournamentStatus::Cancelled);

    // Sponsor A claims refund
    ctx.client.claim_refund(&tournament_id, &ctx.sponsor_a);
    assert_eq!(ctx.xlm_client.balance(&ctx.sponsor_a), 100_000);
}

#[test]
fn test_custom_milestone_tournament_creation() {
    let ctx = setup_test_context();
    let tournament_id = 601u64;

    let mut custom_milestones = Vec::new(&ctx.env);
    custom_milestones.push_back(MilestoneInput {
        stage_id: 1,
        name: symbol_short!("qualifier"),
        basis_points: 1000, // 10%
        default_recipient: Some(ctx.reg_recipient.clone()),
    });
    custom_milestones.push_back(MilestoneInput {
        stage_id: 2,
        name: symbol_short!("semis"),
        basis_points: 3000, // 30%
        default_recipient: Some(ctx.quarter_recipient.clone()),
    });
    custom_milestones.push_back(MilestoneInput {
        stage_id: 3,
        name: symbol_short!("finals"),
        basis_points: 6000, // 60%
        default_recipient: Some(ctx.grand_winner.clone()),
    });

    ctx.client.create_tournament(
        &ctx.organizer,
        &tournament_id,
        &ctx.oracle,
        &0,
        &custom_milestones,
    );

    let t = ctx.client.get_tournament(&tournament_id).unwrap();
    assert_eq!(t.total_milestones, 3);

    ctx.client.deposit_sponsorship(
        &tournament_id,
        &ctx.sponsor_a,
        &ctx.xlm_token,
        &10_000,
    );

    ctx.client.complete_stage(&tournament_id, &0, &None);
    assert_eq!(ctx.xlm_client.balance(&ctx.reg_recipient), 1_000); // 10%

    ctx.client.complete_stage(&tournament_id, &1, &None);
    assert_eq!(ctx.xlm_client.balance(&ctx.quarter_recipient), 3_000); // 30%

    ctx.client.complete_stage(&tournament_id, &2, &None);
    assert_eq!(ctx.xlm_client.balance(&ctx.grand_winner), 6_000); // 60%

    let t_done = ctx.client.get_tournament(&tournament_id).unwrap();
    assert_eq!(t_done.status, TournamentStatus::Completed);
}

#[test]
fn test_update_tournament_oracle() {
    let ctx = setup_test_context();
    let tournament_id = 701u64;

    ctx.client.create_standard_tournament(
        &ctx.organizer,
        &tournament_id,
        &ctx.oracle,
        &0,
        &Some(ctx.reg_recipient.clone()),
        &Some(ctx.quarter_recipient.clone()),
        &Some(ctx.grand_winner.clone()),
    );

    let new_oracle = Address::generate(&ctx.env);
    ctx.client.set_tournament_oracle(
        &tournament_id,
        &ctx.organizer,
        &new_oracle,
    );

    let t = ctx.client.get_tournament(&tournament_id).unwrap();
    assert_eq!(t.oracle, new_oracle);
}

#[test]
#[should_panic(expected = "Error(Contract, #1)")] // AlreadyInitialized
fn test_double_initialize_rejected() {
    let ctx = setup_test_context();
    ctx.client.initialize(&ctx.admin);
}

#[test]
#[should_panic(expected = "Error(Contract, #6)")] // TournamentAlreadyExists
fn test_duplicate_tournament_id_rejected() {
    let ctx = setup_test_context();
    let tournament_id = 777u64;

    ctx.client.create_standard_tournament(
        &ctx.organizer,
        &tournament_id,
        &ctx.oracle,
        &0,
        &Some(ctx.reg_recipient.clone()),
        &Some(ctx.quarter_recipient.clone()),
        &Some(ctx.grand_winner.clone()),
    );

    ctx.client.create_standard_tournament(
        &ctx.organizer,
        &tournament_id,
        &ctx.oracle,
        &0,
        &Some(ctx.reg_recipient.clone()),
        &Some(ctx.quarter_recipient.clone()),
        &Some(ctx.grand_winner.clone()),
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #10)")] // InvalidMilestoneConfig (empty milestones)
fn test_empty_milestones_rejected() {
    let ctx = setup_test_context();
    let empty_milestones = Vec::new(&ctx.env);

    ctx.client.create_tournament(
        &ctx.organizer,
        &888u64,
        &ctx.oracle,
        &0,
        &empty_milestones,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #9)")] // InvalidAmount (zero deposit)
fn test_zero_deposit_rejected() {
    let ctx = setup_test_context();
    let tournament_id = 999u64;

    ctx.client.create_standard_tournament(
        &ctx.organizer,
        &tournament_id,
        &ctx.oracle,
        &0,
        &Some(ctx.reg_recipient.clone()),
        &Some(ctx.quarter_recipient.clone()),
        &Some(ctx.grand_winner.clone()),
    );

    ctx.client.deposit_sponsorship(
        &tournament_id,
        &ctx.sponsor_a,
        &ctx.xlm_token,
        &0,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #11)")] // InvalidBasisPointsSum
fn test_invalid_basis_points_sum_rejected() {
    let ctx = setup_test_context();
    let tournament_id = 801u64;

    let mut bad_milestones = Vec::new(&ctx.env);
    bad_milestones.push_back(MilestoneInput {
        stage_id: 1,
        name: symbol_short!("stage1"),
        basis_points: 5000,
        default_recipient: Some(ctx.reg_recipient.clone()),
    });
    bad_milestones.push_back(MilestoneInput {
        stage_id: 2,
        name: symbol_short!("stage2"),
        basis_points: 4000, // Total = 9000 != 10000
        default_recipient: Some(ctx.grand_winner.clone()),
    });

    ctx.client.create_tournament(
        &ctx.organizer,
        &tournament_id,
        &ctx.oracle,
        &0,
        &bad_milestones,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #14)")] // KickoffAlreadyOccurred
fn test_cancel_after_kickoff_rejected() {
    let ctx = setup_test_context();
    let tournament_id = 901u64;

    ctx.client.create_standard_tournament(
        &ctx.organizer,
        &tournament_id,
        &ctx.oracle,
        &0,
        &Some(ctx.reg_recipient.clone()),
        &Some(ctx.quarter_recipient.clone()),
        &Some(ctx.grand_winner.clone()),
    );

    ctx.client.deposit_sponsorship(
        &tournament_id,
        &ctx.sponsor_a,
        &ctx.xlm_token,
        &10_000,
    );

    // Kickoff stage 0 is completed
    ctx.client.complete_stage(&tournament_id, &0, &None);

    // Trying to cancel after kickoff should panic with KickoffAlreadyOccurred
    ctx.client.cancel_tournament(&tournament_id, &ctx.organizer);
}

#[test]
#[should_panic(expected = "Error(Contract, #15)")] // TournamentNotCancelled
fn test_refund_before_cancellation_rejected() {
    let ctx = setup_test_context();
    let tournament_id = 1001u64;

    ctx.client.create_standard_tournament(
        &ctx.organizer,
        &tournament_id,
        &ctx.oracle,
        &0,
        &Some(ctx.reg_recipient.clone()),
        &Some(ctx.quarter_recipient.clone()),
        &Some(ctx.grand_winner.clone()),
    );

    ctx.client.deposit_sponsorship(
        &tournament_id,
        &ctx.sponsor_a,
        &ctx.xlm_token,
        &10_000,
    );

    // Attempting refund while tournament is still active / not cancelled
    ctx.client.claim_refund(&tournament_id, &ctx.sponsor_a);
}

#[test]
#[should_panic(expected = "Error(Contract, #12)")] // InvalidStageOrder
fn test_out_of_order_stage_signoff_rejected() {
    let ctx = setup_test_context();
    let tournament_id = 1101u64;

    ctx.client.create_standard_tournament(
        &ctx.organizer,
        &tournament_id,
        &ctx.oracle,
        &0,
        &Some(ctx.reg_recipient.clone()),
        &Some(ctx.quarter_recipient.clone()),
        &Some(ctx.grand_winner.clone()),
    );

    ctx.client.deposit_sponsorship(
        &tournament_id,
        &ctx.sponsor_a,
        &ctx.xlm_token,
        &10_000,
    );

    // Attempting stage 1 before stage 0
    ctx.client.complete_stage(&tournament_id, &1, &None);
}

#[test]
#[should_panic(expected = "Error(Contract, #16)")] // NothingToRefund (double refund prevention)
fn test_double_refund_rejected() {
    let ctx = setup_test_context();
    let tournament_id = 1201u64;

    ctx.client.create_standard_tournament(
        &ctx.organizer,
        &tournament_id,
        &ctx.oracle,
        &0,
        &Some(ctx.reg_recipient.clone()),
        &Some(ctx.quarter_recipient.clone()),
        &Some(ctx.grand_winner.clone()),
    );

    ctx.client.deposit_sponsorship(
        &tournament_id,
        &ctx.sponsor_a,
        &ctx.xlm_token,
        &10_000,
    );

    ctx.client.cancel_tournament(&tournament_id, &ctx.organizer);

    // First refund succeeds
    ctx.client.claim_refund(&tournament_id, &ctx.sponsor_a);

    // Second refund fails
    ctx.client.claim_refund(&tournament_id, &ctx.sponsor_a);
}

#[test]
#[should_panic(expected = "Error(Contract, #3)")] // ContractPaused
fn test_pause_blocks_deposits() {
    let ctx = setup_test_context();
    let tournament_id = 1301u64;

    ctx.client.create_standard_tournament(
        &ctx.organizer,
        &tournament_id,
        &ctx.oracle,
        &0,
        &Some(ctx.reg_recipient.clone()),
        &Some(ctx.quarter_recipient.clone()),
        &Some(ctx.grand_winner.clone()),
    );

    ctx.client.pause(&ctx.admin);

    // Deposit should fail when paused
    ctx.client.deposit_sponsorship(
        &tournament_id,
        &ctx.sponsor_a,
        &ctx.xlm_token,
        &10_000,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #8)")] // InvalidStatus (deposit after kickoff/completed)
fn test_deposit_after_tournament_completed_rejected() {
    let ctx = setup_test_context();
    let tournament_id = 1401u64;

    ctx.client.create_standard_tournament(
        &ctx.organizer,
        &tournament_id,
        &ctx.oracle,
        &0,
        &Some(ctx.reg_recipient.clone()),
        &Some(ctx.quarter_recipient.clone()),
        &Some(ctx.grand_winner.clone()),
    );

    ctx.client.deposit_sponsorship(
        &tournament_id,
        &ctx.sponsor_a,
        &ctx.xlm_token,
        &10_000,
    );

    ctx.client.complete_stage(&tournament_id, &0, &None);
    ctx.client.complete_stage(&tournament_id, &1, &None);
    ctx.client.complete_stage(&tournament_id, &2, &None);

    // Trying to deposit into completed tournament
    ctx.client.deposit_sponsorship(
        &tournament_id,
        &ctx.sponsor_b,
        &ctx.xlm_token,
        &5_000,
    );
}
