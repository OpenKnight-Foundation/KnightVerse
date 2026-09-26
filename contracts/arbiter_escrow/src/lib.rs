//! SC-45: Multi-Sig Arbiter Escrow for high-stakes chess match disputes.
//!
//! Two players stake an equal amount of a token into escrow. If the result of
//! the match is agreed, the admin can settle it immediately and the winner
//! takes the whole pot. If a player raises a cheating allegation,
//! [`ArbiterEscrowContract::dispute_match`] freezes the payout and opens a
//! 48-hour arbitration window during which the three appointed Fair Play
//! Arbiters vote. A 2-of-3 majority releases the pot to the rightful winner
//! or refunds both players.
//!
//! If the arbiters fail to reach a 2-of-3 majority, the funds stay frozen and
//! a time-locked fallback becomes available 7 days after the dispute was
//! raised, returning each player's original stake.
#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, panic_with_error, symbol_short, token,
    Address, Env,
};

// =============================================================================
// CONSTANTS
// =============================================================================

/// Arbitration window opened by [`ArbiterEscrowContract::dispute_match`].
///
/// Once a match is disputed, the payout is frozen and the appointed Fair Play
/// Arbiters have this long to record their votes.
pub const ARBITRATION_WINDOW_SECS: u64 = 48 * 60 * 60; // 48 hours

/// Time-locked fallback deadline, measured from the moment the match was
/// disputed. If the arbiters have not produced a 2-of-3 majority by then the
/// escrow can be unwound and both players receive their original stake back.
pub const FALLBACK_TIMEOUT_SECS: u64 = 7 * 24 * 60 * 60; // 7 days

/// Votes required from the arbiter panel to settle a disputed match (2-of-3).
pub const QUORUM: u32 = 2;

/// Number of Fair Play Arbiters appointed per match.
pub const ARBITER_COUNT: u32 = 3;

// =============================================================================
// ERRORS
// =============================================================================

/// Structured error codes for [`ArbiterEscrowContract`].
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum EscrowError {
    /// Contract has already been initialized
    AlreadyInitialized = 1,
    /// Contract has not been initialized yet
    NotInitialized = 2,
    /// Caller is not the contract admin
    NotAdmin = 3,
    /// An escrow for this match id already exists
    EscrowAlreadyExists = 4,
    /// No escrow exists for this match id
    EscrowNotFound = 5,
    /// Stake amount must be a positive integer
    InvalidAmount = 6,
    /// Winner / player address is not valid for this escrow
    InvalidPlayer = 7,
    /// The three arbiters must be distinct, non-player addresses
    InvalidArbiterPanel = 8,
    /// Operation is not valid for the escrow's current status
    InvalidStatus = 9,
    /// Caller is neither a player nor the admin
    NotParticipant = 10,
    /// This player has already funded their side of the escrow
    AlreadyFunded = 11,
    /// Caller is not one of the arbiters appointed for this match
    UnauthorizedArbiter = 12,
    /// This arbiter has already cast a vote for this match
    AlreadyVoted = 13,
    /// The 48-hour arbitration window has closed; no more votes are accepted
    ArbitrationWindowClosed = 14,
    /// The 2-of-3 arbiter threshold has not been reached yet
    ThresholdNotMet = 15,
    /// The 7-day fallback timelock has not elapsed yet
    FallbackNotElapsed = 16,
    /// A 2-of-3 majority exists, so the fallback cannot be used — resolve instead
    ResolutionAvailable = 17,
    /// Caller is not allowed to perform this operation
    Unauthorized = 18,
}

// =============================================================================
// TYPES
// =============================================================================

/// Lifecycle status of a match escrow.
#[contracttype]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum EscrowStatus {
    /// Created; waiting for both players to deposit their stake
    AwaitingFunding = 1,
    /// Both stakes are locked and the match can be settled
    Funded = 2,
    /// A dispute is open: payout is frozen for arbitration
    Disputed = 3,
    /// Funds have been released or refunded; the escrow is closed
    Resolved = 4,
}

/// The three ways an arbiter can vote on a disputed match.
#[contracttype]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum VoteChoice {
    /// Release the pot to player A
    PlayerA = 1,
    /// Release the pot to player B
    PlayerB = 2,
    /// Refund each player their original stake
    Refund = 3,
}

/// Full state of a single match escrow.
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchEscrow {
    pub match_id: u64,
    pub token: Address,
    pub player_a: Address,
    pub player_b: Address,
    /// Amount each player must deposit
    pub stake: i128,
    /// Total amount currently locked (2 * stake once fully funded)
    pub pot: i128,
    pub funded_a: bool,
    pub funded_b: bool,
    pub arbiter_1: Address,
    pub arbiter_2: Address,
    pub arbiter_3: Address,
    pub status: EscrowStatus,
    /// Ledger timestamp at which the dispute was raised (0 if never disputed)
    pub dispute_started_at: u64,
    /// End of the 48-hour arbitration window (0 if never disputed)
    pub arbitration_deadline: u64,
    /// Deadline of the 7-day time-locked fallback (0 if never disputed)
    pub fallback_deadline: u64,
    pub votes_for_a: u32,
    pub votes_for_b: u32,
    pub votes_for_refund: u32,
    /// Winning outcome once resolved
    pub winning_choice: Option<VoteChoice>,
    pub resolved_at: u64,
}

/// Storage keys for the contract.
#[contracttype]
pub enum DataKey {
    Admin,
    Escrow(u64),
    /// Recorded vote for a given match and arbiter (presence blocks double votes)
    ArbiterVote(u64, Address),
}

// =============================================================================
// CONTRACT
// =============================================================================

#[contract]
pub struct ArbiterEscrowContract;

#[contractimpl]
impl ArbiterEscrowContract {
    // =========================================================================
    // ADMIN
    // =========================================================================

    /// Initialize the contract with the escrow admin (the KnightVerse
    /// settlement authority that appoints Fair Play Arbiters per match).
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic_with_error!(&env, EscrowError::AlreadyInitialized);
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.events().publish((symbol_short!("init"),), admin);
    }

    /// Transfer the admin role.
    pub fn transfer_admin(env: Env, current_admin: Address, new_admin: Address) {
        current_admin.require_auth();
        Self::check_admin(&env, &current_admin);
        env.storage().instance().set(&DataKey::Admin, &new_admin);
        env.events()
            .publish((symbol_short!("adm_xfer"),), new_admin);
    }

    /// Get the contract admin.
    pub fn get_admin(env: Env) -> Address {
        Self::admin(&env)
    }

    // =========================================================================
    // ESCROW SETUP & FUNDING
    // =========================================================================

    /// Create a match escrow and appoint its three Fair Play Arbiters.
    ///
    /// Only the admin can open an escrow. The panel must consist of three
    /// distinct addresses that are not the two players.
    #[allow(clippy::too_many_arguments)]
    pub fn create_escrow(
        env: Env,
        admin: Address,
        match_id: u64,
        token: Address,
        player_a: Address,
        player_b: Address,
        stake: i128,
        arbiter_1: Address,
        arbiter_2: Address,
        arbiter_3: Address,
    ) {
        admin.require_auth();
        Self::check_admin(&env, &admin);

        if env.storage().instance().has(&DataKey::Escrow(match_id)) {
            panic_with_error!(&env, EscrowError::EscrowAlreadyExists);
        }
        if stake <= 0 {
            panic_with_error!(&env, EscrowError::InvalidAmount);
        }
        if player_a == player_b {
            panic_with_error!(&env, EscrowError::InvalidPlayer);
        }
        if arbiter_1 == arbiter_2 || arbiter_1 == arbiter_3 || arbiter_2 == arbiter_3 {
            panic_with_error!(&env, EscrowError::InvalidArbiterPanel);
        }
        if arbiter_1 == player_a
            || arbiter_1 == player_b
            || arbiter_2 == player_a
            || arbiter_2 == player_b
            || arbiter_3 == player_a
            || arbiter_3 == player_b
        {
            panic_with_error!(&env, EscrowError::InvalidArbiterPanel);
        }

        let escrow = MatchEscrow {
            match_id,
            token: token.clone(),
            player_a: player_a.clone(),
            player_b: player_b.clone(),
            stake,
            pot: 0,
            funded_a: false,
            funded_b: false,
            arbiter_1: arbiter_1.clone(),
            arbiter_2: arbiter_2.clone(),
            arbiter_3: arbiter_3.clone(),
            status: EscrowStatus::AwaitingFunding,
            dispute_started_at: 0,
            arbitration_deadline: 0,
            fallback_deadline: 0,
            votes_for_a: 0,
            votes_for_b: 0,
            votes_for_refund: 0,
            winning_choice: None,
            resolved_at: 0,
        };
        env.storage()
            .instance()
            .set(&DataKey::Escrow(match_id), &escrow);

        env.events().publish(
            (symbol_short!("created"), match_id),
            (player_a, player_b, stake, token),
        );
        env.events().publish(
            (symbol_short!("arbiters"), match_id),
            (arbiter_1, arbiter_2, arbiter_3),
        );
    }

    /// Deposit a player's stake into the escrow.
    ///
    /// Each player must call this once. When both stakes are locked the escrow
    /// becomes `Funded` and a `ready` event is emitted.
    pub fn fund_escrow(env: Env, match_id: u64, player: Address) {
        let mut escrow = Self::read_escrow(&env, match_id);
        if escrow.status != EscrowStatus::AwaitingFunding {
            panic_with_error!(&env, EscrowError::InvalidStatus);
        }

        let is_a = player == escrow.player_a;
        let is_b = player == escrow.player_b;
        if !is_a && !is_b {
            panic_with_error!(&env, EscrowError::NotParticipant);
        }
        if (is_a && escrow.funded_a) || (is_b && escrow.funded_b) {
            panic_with_error!(&env, EscrowError::AlreadyFunded);
        }

        player.require_auth();

        let token_client = token::Client::new(&env, &escrow.token);
        token_client.transfer(&player, &env.current_contract_address(), &escrow.stake);

        if is_a {
            escrow.funded_a = true;
        } else {
            escrow.funded_b = true;
        }
        escrow.pot = escrow.pot + escrow.stake;

        let both_funded = escrow.funded_a && escrow.funded_b;
        if both_funded {
            escrow.status = EscrowStatus::Funded;
        }
        env.storage()
            .instance()
            .set(&DataKey::Escrow(match_id), &escrow);

        env.events().publish(
            (symbol_short!("funded"), match_id, player),
            (escrow.stake, escrow.pot),
        );
        if both_funded {
            env.events()
                .publish((symbol_short!("ready"), match_id), escrow.pot);
        }
    }

    // =========================================================================
    // UNDISPUTED SETTLEMENT (FROZEN ONCE DISPUTED)
    // =========================================================================

    /// Settle an undisputed match by attesting the winner.
    ///
    /// This is the normal fast path and is deliberately unavailable once a
    /// dispute has been raised — at that point only the arbiter panel may move
    /// the funds.
    pub fn settle_match(env: Env, match_id: u64, admin: Address, winner: Address) {
        admin.require_auth();
        Self::check_admin(&env, &admin);

        let mut escrow = Self::read_escrow(&env, match_id);
        if escrow.status != EscrowStatus::Funded {
            panic_with_error!(&env, EscrowError::InvalidStatus);
        }

        let choice = if winner == escrow.player_a {
            VoteChoice::PlayerA
        } else if winner == escrow.player_b {
            VoteChoice::PlayerB
        } else {
            panic_with_error!(&env, EscrowError::InvalidPlayer);
        };

        Self::release(&env, &escrow, choice);
        escrow.status = EscrowStatus::Resolved;
        escrow.winning_choice = Some(choice);
        escrow.resolved_at = env.ledger().timestamp();
        env.storage()
            .instance()
            .set(&DataKey::Escrow(match_id), &escrow);

        env.events()
            .publish((symbol_short!("settle"), match_id), winner);
    }

    /// Settle an undisputed match as a draw/refund, returning each stake.
    pub fn settle_refund(env: Env, match_id: u64, admin: Address) {
        admin.require_auth();
        Self::check_admin(&env, &admin);

        let mut escrow = Self::read_escrow(&env, match_id);
        if escrow.status != EscrowStatus::Funded {
            panic_with_error!(&env, EscrowError::InvalidStatus);
        }

        Self::release(&env, &escrow, VoteChoice::Refund);
        escrow.status = EscrowStatus::Resolved;
        escrow.winning_choice = Some(VoteChoice::Refund);
        escrow.resolved_at = env.ledger().timestamp();
        env.storage()
            .instance()
            .set(&DataKey::Escrow(match_id), &escrow);

        env.events().publish(
            (symbol_short!("refund"), match_id),
            (escrow.player_a, escrow.player_b, escrow.stake),
        );
    }

    // =========================================================================
    // DISPUTE & ARBITRATION
    // =========================================================================

    /// Raise a cheating dispute and freeze the payout.
    ///
    /// Callable by either player or the admin. Opens the 48-hour arbitration
    /// window and arms the 7-day time-locked fallback.
    pub fn dispute_match(env: Env, match_id: u64, caller: Address) {
        caller.require_auth();

        let mut escrow = Self::read_escrow(&env, match_id);
        if escrow.status != EscrowStatus::Funded {
            panic_with_error!(&env, EscrowError::InvalidStatus);
        }

        let is_admin = Self::admin(&env) == caller;
        if caller != escrow.player_a && caller != escrow.player_b && !is_admin {
            panic_with_error!(&env, EscrowError::NotParticipant);
        }

        let now = env.ledger().timestamp();
        escrow.status = EscrowStatus::Disputed;
        escrow.dispute_started_at = now;
        escrow.arbitration_deadline = now + ARBITRATION_WINDOW_SECS;
        escrow.fallback_deadline = now + FALLBACK_TIMEOUT_SECS;
        env.storage()
            .instance()
            .set(&DataKey::Escrow(match_id), &escrow);

        env.events().publish(
            (symbol_short!("dispute"), match_id),
            (
                caller,
                escrow.arbitration_deadline,
                escrow.fallback_deadline,
            ),
        );
    }

    /// Cast a Fair Play Arbiter's vote on a disputed match.
    ///
    /// Each appointed arbiter may vote once, and only while the 48-hour
    /// arbitration window is open. The vote is a signed call
    /// ([`Address::require_auth`]) so a 2-of-3 majority is a 2-of-3
    /// multi-signature decision.
    pub fn vote(env: Env, match_id: u64, arbiter: Address, choice: VoteChoice) {
        arbiter.require_auth();

        let mut escrow = Self::read_escrow(&env, match_id);
        if escrow.status != EscrowStatus::Disputed {
            panic_with_error!(&env, EscrowError::InvalidStatus);
        }
        if !Self::is_arbiter(&escrow, &arbiter) {
            panic_with_error!(&env, EscrowError::UnauthorizedArbiter);
        }
        if env.ledger().timestamp() >= escrow.arbitration_deadline {
            panic_with_error!(&env, EscrowError::ArbitrationWindowClosed);
        }

        let vote_key = DataKey::ArbiterVote(match_id, arbiter.clone());
        if env.storage().instance().has(&vote_key) {
            panic_with_error!(&env, EscrowError::AlreadyVoted);
        }
        env.storage().instance().set(&vote_key, &choice);

        match choice {
            VoteChoice::PlayerA => escrow.votes_for_a = escrow.votes_for_a + 1,
            VoteChoice::PlayerB => escrow.votes_for_b = escrow.votes_for_b + 1,
            VoteChoice::Refund => escrow.votes_for_refund = escrow.votes_for_refund + 1,
        }
        env.storage()
            .instance()
            .set(&DataKey::Escrow(match_id), &escrow);

        env.events().publish(
            (symbol_short!("vote"), match_id, arbiter),
            (
                choice as u32,
                escrow.votes_for_a,
                escrow.votes_for_b,
                escrow.votes_for_refund,
            ),
        );
    }

    /// Execute the arbiters' decision once a 2-of-3 majority exists.
    ///
    /// Callable by the admin, either player, or one of the arbiters. Reverts
    /// with [`EscrowError::ThresholdNotMet`] while fewer than two arbiters
    /// agree, which is what prevents an early release.
    pub fn resolve_dispute(env: Env, match_id: u64, caller: Address) {
        caller.require_auth();

        let mut escrow = Self::read_escrow(&env, match_id);
        if escrow.status != EscrowStatus::Disputed {
            panic_with_error!(&env, EscrowError::InvalidStatus);
        }

        let is_admin = Self::admin(&env) == caller;
        let allowed = is_admin
            || caller == escrow.player_a
            || caller == escrow.player_b
            || Self::is_arbiter(&escrow, &caller);
        if !allowed {
            panic_with_error!(&env, EscrowError::Unauthorized);
        }

        let choice = match Self::quorum_choice(&escrow) {
            Some(choice) => choice,
            None => panic_with_error!(&env, EscrowError::ThresholdNotMet),
        };

        Self::release(&env, &escrow, choice);
        escrow.status = EscrowStatus::Resolved;
        escrow.winning_choice = Some(choice);
        escrow.resolved_at = env.ledger().timestamp();
        env.storage()
            .instance()
            .set(&DataKey::Escrow(match_id), &escrow);

        env.events()
            .publish((symbol_short!("resolved"), match_id), (choice as u32, caller));
    }

    /// Time-locked fallback: if the arbiters failed to reach a 2-of-3 majority
    /// within 7 days of the dispute, refund each player their original stake.
    ///
    /// Permissionless by design — anyone can unwind a stalled arbitration as
    /// long as the timelock has elapsed and no majority exists.
    pub fn execute_fallback(env: Env, match_id: u64, caller: Address) {
        caller.require_auth();

        let mut escrow = Self::read_escrow(&env, match_id);
        if escrow.status != EscrowStatus::Disputed {
            panic_with_error!(&env, EscrowError::InvalidStatus);
        }

        let now = env.ledger().timestamp();
        if now < escrow.fallback_deadline {
            panic_with_error!(&env, EscrowError::FallbackNotElapsed);
        }
        if Self::quorum_choice(&escrow).is_some() {
            panic_with_error!(&env, EscrowError::ResolutionAvailable);
        }

        Self::release(&env, &escrow, VoteChoice::Refund);
        escrow.status = EscrowStatus::Resolved;
        escrow.winning_choice = Some(VoteChoice::Refund);
        escrow.resolved_at = now;
        env.storage()
            .instance()
            .set(&DataKey::Escrow(match_id), &escrow);

        env.events().publish(
            (symbol_short!("fallback"), match_id),
            (escrow.player_a, escrow.player_b, escrow.stake),
        );
    }

    // =========================================================================
    // VIEWS
    // =========================================================================

    /// Get the full escrow record for a match.
    pub fn get_escrow(env: Env, match_id: u64) -> Option<MatchEscrow> {
        env.storage().instance().get(&DataKey::Escrow(match_id))
    }

    /// Get the vote an arbiter recorded for a match, if any.
    pub fn get_vote(env: Env, match_id: u64, arbiter: Address) -> Option<VoteChoice> {
        env.storage()
            .instance()
            .get(&DataKey::ArbiterVote(match_id, arbiter))
    }

    /// The 48-hour arbitration window, in seconds.
    pub fn arbitration_window(_env: Env) -> u64 {
        ARBITRATION_WINDOW_SECS
    }

    /// The 7-day fallback timelock, in seconds.
    pub fn fallback_timeout(_env: Env) -> u64 {
        FALLBACK_TIMEOUT_SECS
    }

    /// Votes required to settle a dispute (2-of-3).
    pub fn quorum(_env: Env) -> u32 {
        QUORUM
    }

    /// Number of arbiters appointed per match.
    pub fn arbiter_count(_env: Env) -> u32 {
        ARBITER_COUNT
    }

    // =========================================================================
    // INTERNAL HELPERS
    // =========================================================================

    fn admin(env: &Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic_with_error!(env, EscrowError::NotInitialized))
    }

    fn check_admin(env: &Env, caller: &Address) {
        let admin = Self::admin(env);
        if admin != *caller {
            panic_with_error!(env, EscrowError::NotAdmin);
        }
    }

    fn read_escrow(env: &Env, match_id: u64) -> MatchEscrow {
        env.storage()
            .instance()
            .get(&DataKey::Escrow(match_id))
            .unwrap_or_else(|| panic_with_error!(env, EscrowError::EscrowNotFound))
    }

    fn is_arbiter(escrow: &MatchEscrow, who: &Address) -> bool {
        who == &escrow.arbiter_1 || who == &escrow.arbiter_2 || who == &escrow.arbiter_3
    }

    /// Returns the outcome that has reached the 2-of-3 threshold, if any.
    fn quorum_choice(escrow: &MatchEscrow) -> Option<VoteChoice> {
        if escrow.votes_for_a >= QUORUM {
            Some(VoteChoice::PlayerA)
        } else if escrow.votes_for_b >= QUORUM {
            Some(VoteChoice::PlayerB)
        } else if escrow.votes_for_refund >= QUORUM {
            Some(VoteChoice::Refund)
        } else {
            None
        }
    }

    /// Move the escrowed funds according to `choice` and emit a `payout` event
    /// per recipient for on-chain transparency.
    fn release(env: &Env, escrow: &MatchEscrow, choice: VoteChoice) {
        let token_client = token::Client::new(env, &escrow.token);
        let contract = env.current_contract_address();

        match choice {
            VoteChoice::PlayerA => {
                token_client.transfer(&contract, &escrow.player_a, &escrow.pot);
                env.events().publish(
                    (symbol_short!("payout"), escrow.match_id),
                    (escrow.player_a.clone(), escrow.pot),
                );
            }
            VoteChoice::PlayerB => {
                token_client.transfer(&contract, &escrow.player_b, &escrow.pot);
                env.events().publish(
                    (symbol_short!("payout"), escrow.match_id),
                    (escrow.player_b.clone(), escrow.pot),
                );
            }
            VoteChoice::Refund => {
                token_client.transfer(&contract, &escrow.player_a, &escrow.stake);
                token_client.transfer(&contract, &escrow.player_b, &escrow.stake);
                env.events().publish(
                    (symbol_short!("payout"), escrow.match_id),
                    (escrow.player_a.clone(), escrow.stake),
                );
                env.events().publish(
                    (symbol_short!("payout"), escrow.match_id),
                    (escrow.player_b.clone(), escrow.stake),
                );
            }
        }
    }
}

#[cfg(test)]
mod test;
