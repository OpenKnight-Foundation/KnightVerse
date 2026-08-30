#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, panic_with_error, symbol_short,
    token, Address, Env, Symbol, Vec,
};

/// Structured error codes for SponsorshipEscrowContract.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum EscrowError {
    /// Contract has already been initialized
    AlreadyInitialized = 1,
    /// Caller is not authorized to perform this operation
    Unauthorized = 2,
    /// Contract is currently paused
    ContractPaused = 3,
    /// Contract is already paused
    AlreadyPaused = 4,
    /// Contract is not paused
    NotPaused = 5,
    /// Tournament with the given ID already exists
    TournamentAlreadyExists = 6,
    /// Tournament not found
    TournamentNotFound = 7,
    /// Tournament is in an invalid status for this operation
    InvalidStatus = 8,
    /// Deposit or disbursement amount must be positive
    InvalidAmount = 9,
    /// Milestone configuration is invalid (e.g. empty or invalid stage parameters)
    InvalidMilestoneConfig = 10,
    /// Sum of milestone basis points must be exactly 10,000 (100%)
    InvalidBasisPointsSum = 11,
    /// Stages must be completed in sequential order
    InvalidStageOrder = 12,
    /// Stage has already been completed and disbursed
    StageAlreadyCompleted = 13,
    /// Kickoff has already occurred; cancellation before kickoff is no longer possible
    KickoffAlreadyOccurred = 14,
    /// Tournament has not been cancelled
    TournamentNotCancelled = 15,
    /// No funds available to refund for the sponsor
    NothingToRefund = 16,
    /// No recipient address specified for the milestone payout
    NoRecipientSpecified = 17,
    /// Kickoff deadline has not passed yet
    DeadlineNotPassed = 18,
    /// Insufficient escrow token balance for requested operation
    InsufficientBalance = 19,
    /// Milestone does not exist for the tournament
    MilestoneNotFound = 20,
    /// Recipient address is invalid
    InvalidRecipient = 21,
}

/// Lifecycle statuses of a tournament escrow.
#[contracttype]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum TournamentStatus {
    /// Created & accepting multi-token deposits from sponsors before kickoff
    AcceptingDeposits = 1,
    /// Tournament kicked off / in progress across milestones
    Active = 2,
    /// All milestones completed and fully disbursed
    Completed = 3,
    /// Cancelled before kickoff (sponsors are guaranteed 100% refund)
    Cancelled = 4,
}

/// Standard stage definitions for tournament milestones.
#[contracttype]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum StandardStage {
    RegistrationComplete = 1,
    QuarterFinals = 2,
    GrandFinal = 3,
}

/// Input configuration for defining a tournament stage milestone.
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MilestoneInput {
    /// Stage identifier (e.g. 1 for Registration Complete, 2 for Quarter-Finals, 3 for Grand Final)
    pub stage_id: u32,
    /// Descriptive short name (max 9 chars for Symbol, e.g. "reg_cmp", "quarter", "grd_fnl")
    pub name: Symbol,
    /// Percentage share of total locked pool in basis points (100 bps = 1%, 10,000 bps = 100%)
    pub basis_points: u32,
    /// Optional pre-configured default recipient (e.g. organizer or stage winner address)
    pub default_recipient: Option<Address>,
}

/// Recorded execution state of a tournament milestone.
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MilestoneState {
    pub stage_id: u32,
    pub name: Symbol,
    pub basis_points: u32,
    pub recipient: Option<Address>,
    pub completed: bool,
    pub disbursed_ledger: u64,
}

/// Core metadata and state for a tournament escrow.
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tournament {
    pub id: u64,
    pub organizer: Address,
    pub oracle: Address,
    pub status: TournamentStatus,
    pub current_stage_index: u32,
    pub total_milestones: u32,
    pub created_at_ledger: u64,
    pub kickoff_deadline: u64,
}

/// Storage data keys for the contract.
#[contracttype]
pub enum DataKey {
    Admin,
    Paused,
    Tournament(u64),
    MilestoneInput(u64, u32),
    MilestoneState(u64, u32),
    TournamentTokens(u64),
    TournamentSponsors(u64),
    TotalDeposited(u64, Address),
    RemainingBalance(u64, Address),
    TotalDisbursed(u64, Address),
    SponsorDeposit(u64, Address, Address),
}

pub const TOTAL_BASIS_POINTS: u32 = 10000;

#[contract]
pub struct SponsorshipEscrowContract;

#[contractimpl]
impl SponsorshipEscrowContract {
    /// Initialize the contract with a designated admin address.
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic_with_error!(&env, EscrowError::AlreadyInitialized);
        }

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Paused, &false);
    }

    // =========================================================================
    // ADMIN & CIRCUIT BREAKER / PAUSE CONTROLS
    // =========================================================================

    /// Pause the contract, blocking state-changing escrow operations.
    pub fn pause(env: Env, caller: Address) {
        caller.require_auth();
        Self::check_admin(&env, &caller);

        if Self::is_paused(&env) {
            panic_with_error!(&env, EscrowError::AlreadyPaused);
        }

        env.storage().instance().set(&DataKey::Paused, &true);
        env.events().publish((symbol_short!("paused"),), caller);
    }

    /// Unpause the contract, restoring normal operations.
    pub fn unpause(env: Env, caller: Address) {
        caller.require_auth();
        Self::check_admin(&env, &caller);

        if !Self::is_paused(&env) {
            panic_with_error!(&env, EscrowError::NotPaused);
        }

        env.storage().instance().set(&DataKey::Paused, &false);
        env.events().publish((symbol_short!("unpaused"),), caller);
    }

    /// Transfer contract admin role.
    pub fn transfer_admin(env: Env, current_admin: Address, new_admin: Address) {
        current_admin.require_auth();
        Self::check_admin(&env, &current_admin);

        env.storage().instance().set(&DataKey::Admin, &new_admin);
        env.events()
            .publish((symbol_short!("adm_xfer"),), new_admin);
    }

    /// Update tournament oracle address before tournament completion.
    pub fn set_tournament_oracle(
        env: Env,
        tournament_id: u64,
        caller: Address,
        new_oracle: Address,
    ) {
        caller.require_auth();
        Self::check_not_paused(&env);

        let mut tournament = Self::get_tournament_internal(&env, tournament_id);
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();

        if caller != tournament.organizer && caller != admin {
            panic_with_error!(&env, EscrowError::Unauthorized);
        }

        if tournament.status == TournamentStatus::Completed
            || tournament.status == TournamentStatus::Cancelled
        {
            panic_with_error!(&env, EscrowError::InvalidStatus);
        }

        tournament.oracle = new_oracle.clone();
        env.storage()
            .instance()
            .set(&DataKey::Tournament(tournament_id), &tournament);

        env.events().publish(
            (symbol_short!("set_orc"), tournament_id),
            (caller, new_oracle),
        );
    }

    // =========================================================================
    // TOURNAMENT CREATION & MILESTONE DEFINITION
    // =========================================================================

    /// Create a tournament with custom stage milestones.
    ///
    /// Validates that:
    /// - Milestones list is non-empty.
    /// - Sum of milestone basis points equals exactly 10,000 (100%).
    /// - Each milestone has basis points > 0.
    pub fn create_tournament(
        env: Env,
        organizer: Address,
        tournament_id: u64,
        oracle: Address,
        kickoff_deadline: u64,
        milestones: Vec<MilestoneInput>,
    ) {
        organizer.require_auth();
        Self::check_not_paused(&env);

        let tourn_key = DataKey::Tournament(tournament_id);
        if env.storage().instance().has(&tourn_key) {
            panic_with_error!(&env, EscrowError::TournamentAlreadyExists);
        }

        let total_milestones = milestones.len();
        if total_milestones == 0 {
            panic_with_error!(&env, EscrowError::InvalidMilestoneConfig);
        }

        let mut sum_bps: u32 = 0;
        for i in 0..total_milestones {
            let m = milestones.get(i).unwrap();
            if m.basis_points == 0 {
                panic_with_error!(&env, EscrowError::InvalidMilestoneConfig);
            }
            sum_bps = sum_bps
                .checked_add(m.basis_points)
                .unwrap_or_else(|| panic_with_error!(&env, EscrowError::InvalidBasisPointsSum));

            // Store milestone config & initial state
            env.storage()
                .instance()
                .set(&DataKey::MilestoneInput(tournament_id, i), &m);

            let initial_state = MilestoneState {
                stage_id: m.stage_id,
                name: m.name,
                basis_points: m.basis_points,
                recipient: m.default_recipient.clone(),
                completed: false,
                disbursed_ledger: 0,
            };
            env.storage()
                .instance()
                .set(&DataKey::MilestoneState(tournament_id, i), &initial_state);
        }

        if sum_bps != TOTAL_BASIS_POINTS {
            panic_with_error!(&env, EscrowError::InvalidBasisPointsSum);
        }

        let tournament = Tournament {
            id: tournament_id,
            organizer: organizer.clone(),
            oracle: oracle.clone(),
            status: TournamentStatus::AcceptingDeposits,
            current_stage_index: 0,
            total_milestones,
            created_at_ledger: env.ledger().sequence() as u64,
            kickoff_deadline,
        };

        env.storage().instance().set(&tourn_key, &tournament);

        let empty_tokens: Vec<Address> = Vec::new(&env);
        env.storage()
            .instance()
            .set(&DataKey::TournamentTokens(tournament_id), &empty_tokens);

        let empty_sponsors: Vec<Address> = Vec::new(&env);
        env.storage()
            .instance()
            .set(&DataKey::TournamentSponsors(tournament_id), &empty_sponsors);

        env.events().publish(
            (symbol_short!("created"), tournament_id),
            (organizer, oracle, total_milestones),
        );
    }

    /// Convenience helper to create a standard 3-stage tournament:
    /// - Stage 1: Registration Complete (default 2000 bps = 20%)
    /// - Stage 2: Quarter-Finals (default 3000 bps = 30%)
    /// - Stage 3: Grand Final (default 5000 bps = 50%)
    pub fn create_standard_tournament(
        env: Env,
        organizer: Address,
        tournament_id: u64,
        oracle: Address,
        kickoff_deadline: u64,
        reg_recipient: Option<Address>,
        quarter_recipient: Option<Address>,
        grand_recipient: Option<Address>,
    ) {
        let mut milestones: Vec<MilestoneInput> = Vec::new(&env);

        milestones.push_back(MilestoneInput {
            stage_id: StandardStage::RegistrationComplete as u32,
            name: symbol_short!("reg_cmp"),
            basis_points: 2000,
            default_recipient: reg_recipient,
        });

        milestones.push_back(MilestoneInput {
            stage_id: StandardStage::QuarterFinals as u32,
            name: symbol_short!("quarter"),
            basis_points: 3000,
            default_recipient: quarter_recipient,
        });

        milestones.push_back(MilestoneInput {
            stage_id: StandardStage::GrandFinal as u32,
            name: symbol_short!("grd_fnl"),
            basis_points: 5000,
            default_recipient: grand_recipient,
        });

        Self::create_tournament(
            env,
            organizer,
            tournament_id,
            oracle,
            kickoff_deadline,
            milestones,
        );
    }

    // =========================================================================
    // MULTI-TOKEN DEPOSIT MECHANISM
    // =========================================================================

    /// Deposit sponsorship funds in any token (e.g. XLM, USDC).
    ///
    /// Can only be called while the tournament is accepting deposits (before kickoff).
    /// Transfers tokens from the sponsor to this escrow contract.
    pub fn deposit_sponsorship(
        env: Env,
        tournament_id: u64,
        sponsor: Address,
        token: Address,
        amount: i128,
    ) {
        sponsor.require_auth();
        Self::check_not_paused(&env);

        if amount <= 0 {
            panic_with_error!(&env, EscrowError::InvalidAmount);
        }

        let tournament = Self::get_tournament_internal(&env, tournament_id);
        if tournament.status != TournamentStatus::AcceptingDeposits {
            panic_with_error!(&env, EscrowError::InvalidStatus);
        }

        // Perform token transfer from sponsor to contract
        let token_client = token::Client::new(&env, &token);
        token_client.transfer(&sponsor, &env.current_contract_address(), &amount);

        // Update total deposited for this token
        let total_key = DataKey::TotalDeposited(tournament_id, token.clone());
        let current_total: i128 = env.storage().instance().get(&total_key).unwrap_or(0);
        let new_total = current_total
            .checked_add(amount)
            .unwrap_or_else(|| panic_with_error!(&env, EscrowError::InvalidAmount));
        env.storage().instance().set(&total_key, &new_total);

        // Update remaining balance for this token
        let rem_key = DataKey::RemainingBalance(tournament_id, token.clone());
        let current_rem: i128 = env.storage().instance().get(&rem_key).unwrap_or(0);
        let new_rem = current_rem
            .checked_add(amount)
            .unwrap_or_else(|| panic_with_error!(&env, EscrowError::InvalidAmount));
        env.storage().instance().set(&rem_key, &new_rem);

        // Update sponsor's individual deposit for this token
        let sponsor_key = DataKey::SponsorDeposit(tournament_id, sponsor.clone(), token.clone());
        let current_sponsor_dep: i128 = env.storage().instance().get(&sponsor_key).unwrap_or(0);
        let new_sponsor_dep = current_sponsor_dep
            .checked_add(amount)
            .unwrap_or_else(|| panic_with_error!(&env, EscrowError::InvalidAmount));
        env.storage()
            .instance()
            .set(&sponsor_key, &new_sponsor_dep);

        // Record token in tournament's token list if not present
        let tokens_key = DataKey::TournamentTokens(tournament_id);
        let mut tokens: Vec<Address> = env
            .storage()
            .instance()
            .get(&tokens_key)
            .unwrap_or(Vec::new(&env));

        let mut token_found = false;
        for i in 0..tokens.len() {
            if tokens.get(i).unwrap() == token {
                token_found = true;
                break;
            }
        }
        if !token_found {
            tokens.push_back(token.clone());
            env.storage().instance().set(&tokens_key, &tokens);
        }

        // Record sponsor in tournament's sponsors list if not present
        let sponsors_key = DataKey::TournamentSponsors(tournament_id);
        let mut sponsors: Vec<Address> = env
            .storage()
            .instance()
            .get(&sponsors_key)
            .unwrap_or(Vec::new(&env));

        let mut sponsor_found = false;
        for i in 0..sponsors.len() {
            if sponsors.get(i).unwrap() == sponsor {
                sponsor_found = true;
                break;
            }
        }
        if !sponsor_found {
            sponsors.push_back(sponsor.clone());
            env.storage().instance().set(&sponsors_key, &sponsors);
        }

        // Emit deposit event
        env.events().publish(
            (symbol_short!("deposit"), tournament_id, sponsor),
            (token, amount),
        );
    }

    // =========================================================================
    // ORACLE SIGN-OFF & AUTOMATIC MILESTONE DISBURSEMENT
    // =========================================================================

    /// Sign off on the completion of a tournament stage milestone and trigger
    /// automatic proportional disbursement across all deposited tokens (XLM, USDC, etc.).
    ///
    /// Requirements:
    /// - Only the authorized oracle can invoke this.
    /// - Stages must be completed sequentially (0 -> 1 -> 2 ...).
    /// - Completing stage 0 transitions tournament from AcceptingDeposits to Active (Kickoff).
    /// - Milestone recipient is resolved from override_recipient or default_recipient.
    /// - All deposited tokens are disbursed in exact proportion to the milestone's basis points.
    /// - Final stage milestone disburses 100% of remaining undistributed token balances.
    pub fn complete_stage(
        env: Env,
        tournament_id: u64,
        stage_index: u32,
        override_recipient: Option<Address>,
    ) {
        Self::check_not_paused(&env);

        let mut tournament = Self::get_tournament_internal(&env, tournament_id);
        tournament.oracle.require_auth();

        if tournament.status != TournamentStatus::AcceptingDeposits
            && tournament.status != TournamentStatus::Active
        {
            panic_with_error!(&env, EscrowError::InvalidStatus);
        }

        if stage_index != tournament.current_stage_index {
            panic_with_error!(&env, EscrowError::InvalidStageOrder);
        }

        if stage_index >= tournament.total_milestones {
            panic_with_error!(&env, EscrowError::MilestoneNotFound);
        }

        let milestone_state_key = DataKey::MilestoneState(tournament_id, stage_index);
        let mut milestone_state: MilestoneState = env
            .storage()
            .instance()
            .get(&milestone_state_key)
            .unwrap_or_else(|| panic_with_error!(&env, EscrowError::MilestoneNotFound));

        if milestone_state.completed {
            panic_with_error!(&env, EscrowError::StageAlreadyCompleted);
        }

        // Determine payout recipient
        let recipient = match override_recipient {
            Some(addr) => addr,
            None => match milestone_state.recipient {
                Some(addr) => addr,
                None => panic_with_error!(&env, EscrowError::NoRecipientSpecified),
            },
        };

        if recipient == env.current_contract_address() {
            panic_with_error!(&env, EscrowError::InvalidRecipient);
        }

        // If tournament was accepting deposits, transition to Active (Kickoff occurred)
        if tournament.status == TournamentStatus::AcceptingDeposits {
            tournament.status = TournamentStatus::Active;
        }

        // Execute disbursements across all deposited tokens
        let tokens_key = DataKey::TournamentTokens(tournament_id);
        let tokens: Vec<Address> = env
            .storage()
            .instance()
            .get(&tokens_key)
            .unwrap_or(Vec::new(&env));

        let is_last_stage = (stage_index + 1) == tournament.total_milestones;

        for i in 0..tokens.len() {
            let token = tokens.get(i).unwrap();
            let total_dep_key = DataKey::TotalDeposited(tournament_id, token.clone());
            let total_dep: i128 = env.storage().instance().get(&total_dep_key).unwrap_or(0);

            let rem_key = DataKey::RemainingBalance(tournament_id, token.clone());
            let rem_balance: i128 = env.storage().instance().get(&rem_key).unwrap_or(0);

            let payout_amount = if is_last_stage {
                // Disburse full remaining balance on final stage (guarantees zero leftover dust)
                rem_balance
            } else {
                // (total_dep * basis_points) / 10000
                (total_dep
                    .checked_mul(milestone_state.basis_points as i128)
                    .unwrap_or(0))
                    / (TOTAL_BASIS_POINTS as i128)
            };

            if payout_amount > 0 {
                if payout_amount > rem_balance {
                    panic_with_error!(&env, EscrowError::InsufficientBalance);
                }

                // Update remaining balance & total disbursed
                let new_rem = rem_balance - payout_amount;
                env.storage().instance().set(&rem_key, &new_rem);

                let disb_key = DataKey::TotalDisbursed(tournament_id, token.clone());
                let total_disb: i128 = env.storage().instance().get(&disb_key).unwrap_or(0);
                env.storage()
                    .instance()
                    .set(&disb_key, &(total_disb + payout_amount));

                // Transfer tokens to the verified recipient
                let token_client = token::Client::new(&env, &token);
                token_client.transfer(&env.current_contract_address(), &recipient, &payout_amount);

                env.events().publish(
                    (symbol_short!("disburse"), tournament_id, stage_index),
                    (token, recipient.clone(), payout_amount),
                );
            }
        }

        // Update milestone state
        milestone_state.completed = true;
        milestone_state.recipient = Some(recipient.clone());
        milestone_state.disbursed_ledger = env.ledger().sequence() as u64;
        env.storage()
            .instance()
            .set(&milestone_state_key, &milestone_state);

        // Advance tournament stage
        tournament.current_stage_index = stage_index + 1;
        if tournament.current_stage_index == tournament.total_milestones {
            tournament.status = TournamentStatus::Completed;
            env.events()
                .publish((symbol_short!("tourn_end"), tournament_id), stage_index);
        }

        // Save updated tournament
        env.storage()
            .instance()
            .set(&DataKey::Tournament(tournament_id), &tournament);

        env.events().publish(
            (symbol_short!("stg_done"), tournament_id, stage_index),
            (milestone_state.name, recipient),
        );
    }

    // =========================================================================
    // REFUND MECHANISM (IF CANCELLED BEFORE KICKOFF)
    // =========================================================================

    /// Cancel a tournament before kickoff.
    ///
    /// Can only be called by organizer, contract admin, or oracle while the tournament
    /// is still in `AcceptingDeposits` state (before stage 0 sign-off / kickoff).
    pub fn cancel_tournament(env: Env, tournament_id: u64, caller: Address) {
        caller.require_auth();

        let mut tournament = Self::get_tournament_internal(&env, tournament_id);
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();

        if caller != tournament.organizer && caller != admin && caller != tournament.oracle {
            panic_with_error!(&env, EscrowError::Unauthorized);
        }

        if tournament.status != TournamentStatus::AcceptingDeposits
            || tournament.current_stage_index > 0
        {
            panic_with_error!(&env, EscrowError::KickoffAlreadyOccurred);
        }

        tournament.status = TournamentStatus::Cancelled;
        env.storage()
            .instance()
            .set(&DataKey::Tournament(tournament_id), &tournament);

        env.events()
            .publish((symbol_short!("cancel"), tournament_id), caller);
    }

    /// Cancel a tournament if its kickoff deadline has passed without kickoff.
    ///
    /// Any participant/sponsor/organizer can call this once the deadline sequence is exceeded.
    pub fn cancel_if_deadline_passed(env: Env, tournament_id: u64, caller: Address) {
        caller.require_auth();

        let mut tournament = Self::get_tournament_internal(&env, tournament_id);
        if tournament.status != TournamentStatus::AcceptingDeposits {
            panic_with_error!(&env, EscrowError::InvalidStatus);
        }

        if tournament.kickoff_deadline == 0
            || (env.ledger().sequence() as u64) <= tournament.kickoff_deadline
        {
            panic_with_error!(&env, EscrowError::DeadlineNotPassed);
        }

        tournament.status = TournamentStatus::Cancelled;
        env.storage()
            .instance()
            .set(&DataKey::Tournament(tournament_id), &tournament);

        env.events()
            .publish((symbol_short!("timeout"), tournament_id), caller);
    }

    /// Sponsor claims 100% refund of all deposited tokens if tournament is cancelled.
    pub fn claim_refund(env: Env, tournament_id: u64, sponsor: Address) {
        sponsor.require_auth();

        let tournament = Self::get_tournament_internal(&env, tournament_id);
        if tournament.status != TournamentStatus::Cancelled {
            panic_with_error!(&env, EscrowError::TournamentNotCancelled);
        }

        let tokens_key = DataKey::TournamentTokens(tournament_id);
        let tokens: Vec<Address> = env
            .storage()
            .instance()
            .get(&tokens_key)
            .unwrap_or(Vec::new(&env));

        let mut total_refunded_any = false;

        for i in 0..tokens.len() {
            let token = tokens.get(i).unwrap();
            let sponsor_key =
                DataKey::SponsorDeposit(tournament_id, sponsor.clone(), token.clone());
            let deposit_amount: i128 = env.storage().instance().get(&sponsor_key).unwrap_or(0);

            if deposit_amount > 0 {
                total_refunded_any = true;

                // Reset sponsor's recorded deposit to prevent double refunds
                env.storage().instance().set(&sponsor_key, &0i128);

                // Decrement remaining balance
                let rem_key = DataKey::RemainingBalance(tournament_id, token.clone());
                let rem_balance: i128 = env.storage().instance().get(&rem_key).unwrap_or(0);
                let new_rem = rem_balance.saturating_sub(deposit_amount);
                env.storage().instance().set(&rem_key, &new_rem);

                // Transfer tokens back to sponsor
                let token_client = token::Client::new(&env, &token);
                token_client.transfer(
                    &env.current_contract_address(),
                    &sponsor,
                    &deposit_amount,
                );

                env.events().publish(
                    (symbol_short!("refund"), tournament_id, sponsor.clone()),
                    (token, deposit_amount),
                );
            }
        }

        if !total_refunded_any {
            panic_with_error!(&env, EscrowError::NothingToRefund);
        }
    }

    /// Admin emergency batch refund for all sponsors in a cancelled tournament.
    pub fn admin_refund_all_sponsors(env: Env, tournament_id: u64, admin: Address) {
        admin.require_auth();
        Self::check_admin(&env, &admin);

        let tournament = Self::get_tournament_internal(&env, tournament_id);
        if tournament.status != TournamentStatus::Cancelled {
            panic_with_error!(&env, EscrowError::TournamentNotCancelled);
        }

        let sponsors_key = DataKey::TournamentSponsors(tournament_id);
        let sponsors: Vec<Address> = env
            .storage()
            .instance()
            .get(&sponsors_key)
            .unwrap_or(Vec::new(&env));

        let tokens_key = DataKey::TournamentTokens(tournament_id);
        let tokens: Vec<Address> = env
            .storage()
            .instance()
            .get(&tokens_key)
            .unwrap_or(Vec::new(&env));

        for s_idx in 0..sponsors.len() {
            let sponsor = sponsors.get(s_idx).unwrap();

            for t_idx in 0..tokens.len() {
                let token = tokens.get(t_idx).unwrap();
                let sponsor_key =
                    DataKey::SponsorDeposit(tournament_id, sponsor.clone(), token.clone());
                let deposit_amount: i128 =
                    env.storage().instance().get(&sponsor_key).unwrap_or(0);

                if deposit_amount > 0 {
                    env.storage().instance().set(&sponsor_key, &0i128);

                    let rem_key = DataKey::RemainingBalance(tournament_id, token.clone());
                    let rem_balance: i128 = env.storage().instance().get(&rem_key).unwrap_or(0);
                    let new_rem = rem_balance.saturating_sub(deposit_amount);
                    env.storage().instance().set(&rem_key, &new_rem);

                    let token_client = token::Client::new(&env, &token);
                    token_client.transfer(
                        &env.current_contract_address(),
                        &sponsor,
                        &deposit_amount,
                    );

                    env.events().publish(
                        (symbol_short!("adm_ref"), tournament_id, sponsor.clone()),
                        (token, deposit_amount),
                    );
                }
            }
        }
    }

    // =========================================================================
    // VIEW / QUERY FUNCTIONS
    // =========================================================================

    /// Get tournament details.
    pub fn get_tournament(env: Env, tournament_id: u64) -> Option<Tournament> {
        env.storage()
            .instance()
            .get(&DataKey::Tournament(tournament_id))
    }

    /// Get milestone configuration for a stage index.
    pub fn get_milestone_config(
        env: Env,
        tournament_id: u64,
        stage_index: u32,
    ) -> Option<MilestoneInput> {
        env.storage()
            .instance()
            .get(&DataKey::MilestoneInput(tournament_id, stage_index))
    }

    /// Get milestone runtime state for a stage index.
    pub fn get_milestone_state(
        env: Env,
        tournament_id: u64,
        stage_index: u32,
    ) -> Option<MilestoneState> {
        env.storage()
            .instance()
            .get(&DataKey::MilestoneState(tournament_id, stage_index))
    }

    /// Get all milestone states for a tournament.
    pub fn get_all_milestone_states(env: Env, tournament_id: u64) -> Vec<MilestoneState> {
        let tournament = Self::get_tournament(env.clone(), tournament_id);
        let mut list = Vec::new(&env);
        if let Some(t) = tournament {
            for i in 0..t.total_milestones {
                if let Some(ms) = Self::get_milestone_state(env.clone(), tournament_id, i) {
                    list.push_back(ms);
                }
            }
        }
        list
    }

    /// Get total deposited amount for a tournament in a specific token.
    pub fn get_total_deposited(env: Env, tournament_id: u64, token: Address) -> i128 {
        env.storage()
            .instance()
            .get(&DataKey::TotalDeposited(tournament_id, token))
            .unwrap_or(0)
    }

    /// Get remaining undistributed balance for a tournament in a specific token.
    pub fn get_remaining_balance(env: Env, tournament_id: u64, token: Address) -> i128 {
        env.storage()
            .instance()
            .get(&DataKey::RemainingBalance(tournament_id, token))
            .unwrap_or(0)
    }

    /// Get total disbursed amount for a tournament in a specific token.
    pub fn get_total_disbursed(env: Env, tournament_id: u64, token: Address) -> i128 {
        env.storage()
            .instance()
            .get(&DataKey::TotalDisbursed(tournament_id, token))
            .unwrap_or(0)
    }

    /// Get a sponsor's recorded deposit for a tournament in a specific token.
    pub fn get_sponsor_deposit(
        env: Env,
        tournament_id: u64,
        sponsor: Address,
        token: Address,
    ) -> i128 {
        env.storage()
            .instance()
            .get(&DataKey::SponsorDeposit(tournament_id, sponsor, token))
            .unwrap_or(0)
    }

    /// Get all distinct token addresses deposited into a tournament.
    pub fn get_tournament_tokens(env: Env, tournament_id: u64) -> Vec<Address> {
        env.storage()
            .instance()
            .get(&DataKey::TournamentTokens(tournament_id))
            .unwrap_or(Vec::new(&env))
    }

    /// Get all distinct sponsors who deposited into a tournament.
    pub fn get_tournament_sponsors(env: Env, tournament_id: u64) -> Vec<Address> {
        env.storage()
            .instance()
            .get(&DataKey::TournamentSponsors(tournament_id))
            .unwrap_or(Vec::new(&env))
    }

    /// Get the contract admin address.
    pub fn get_admin(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Contract not initialized")
    }

    /// Get current paused status.
    pub fn is_contract_paused(env: Env) -> bool {
        Self::is_paused(&env)
    }

    // =========================================================================
    // INTERNAL HELPERS
    // =========================================================================

    fn is_paused(env: &Env) -> bool {
        env.storage()
            .instance()
            .get(&DataKey::Paused)
            .unwrap_or(false)
    }

    fn check_not_paused(env: &Env) {
        if Self::is_paused(env) {
            panic_with_error!(env, EscrowError::ContractPaused);
        }
    }

    fn check_admin(env: &Env, caller: &Address) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Contract not initialized");
        if admin != *caller {
            panic_with_error!(env, EscrowError::Unauthorized);
        }
    }

    fn get_tournament_internal(env: &Env, tournament_id: u64) -> Tournament {
        env.storage()
            .instance()
            .get(&DataKey::Tournament(tournament_id))
            .unwrap_or_else(|| panic_with_error!(env, EscrowError::TournamentNotFound))
    }
}

#[cfg(test)]
mod test;
