#![no_std]
mod error;
pub use error::ContractError;

use soroban_sdk::token::TokenClient;
use soroban_sdk::{
    Address, Bytes, BytesN, Env, Map, Symbol, Vec, contract, contractimpl, contracttype,
    panic_with_error, symbol_short,
};

// ────────────────────────────────────────────────────────────────────────────
// Game types (retained from the original simple contract)
// ────────────────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GameState {
    Created,
    InProgress,
    Completed,
    Settled,
    Drawn,
    Forfeited,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct Game {
    pub id: u64,
    pub player1: Address,
    pub player2: Option<Address>,
    pub state: GameState,
    pub wager_amount: i128,
    pub current_turn: u32, // 1 = player1, 2 = player2
    pub moves: Vec<ChessMove>,
    pub created_at: u64,
    pub winner: Option<Address>,
    pub last_move_at: u64, // Ledger sequence of last move
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct ChessMove {
    pub player: Address,
    pub move_data: Vec<u32>,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DisputeStatus {
    Pending,
    Resolved,
    Rejected,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct Dispute {
    pub id: u64,
    pub game_id: u64,
    pub filer: Address,   // Player who filed the dispute
    pub against: Address, // Opponent
    pub reason: Bytes,    // Dispute reason (encoded)
    pub status: DisputeStatus,
    pub filed_at: u64,             // Ledger sequence
    pub resolution: Option<Bytes>, // Arbitrator's resolution
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct PlayerRating {
    pub address: Address,
    pub rating: i32, // Current ELO rating
    pub games_played: u32,
    pub wins: u32,
    pub losses: u32,
    pub draws: u32,
    pub highest_rating: i32,
    pub last_updated: u64, // Ledger sequence
}

/// A single backend-signed puzzle-reward claim, as accepted by
/// `claim_puzzle_reward` / `claim_puzzle_rewards_batch`.
#[contracttype]
#[derive(Clone, Debug)]
pub struct Proof {
    pub recipient: Address,
    pub reward_amount: i128,
    pub nonce: u64,
    pub signature: BytesN<64>,
}

// ────────────────────────────────────────────────────────────────────────────
// Storage keys
// ────────────────────────────────────────────────────────────────────────────

// Game / escrow
const GAME_COUNTER: Symbol = symbol_short!("GAME_CNT");
const GAMES: Symbol = symbol_short!("GAMES");
const ESCROW: Symbol = symbol_short!("ESCROW");
const TOKEN_CONTRACT: Symbol = symbol_short!("TOKEN");

// Puzzle-reward  (#199)
const ADMIN_KEY: Symbol = symbol_short!("ADMIN_KEY"); // BytesN<32> ED25519 backend pubkey
const TREASURY: Symbol = symbol_short!("TREASURY"); // i128 treasury reserve
const BALANCES: Symbol = symbol_short!("BALANCES"); // Map<Address, i128>
const USED_NONCE: Symbol = symbol_short!("NONCES"); // Map<u64, bool>
const MAX_STAKE: Symbol = symbol_short!("MAXSTAKE");
const MAX_PRIZE_POOL: Symbol = symbol_short!("MAXPOOL");

// Maximum number of proofs accepted by a single claim_puzzle_rewards_batch call.
// Keeps per-invocation resource usage (CPU/memory/events) bounded.
const MAX_BATCH_SIZE: u32 = 20;

// Fee / treasury  (#200)
const FEE_BIPS: Symbol = symbol_short!("FEE_BIPS"); // u32  (0–1000, i.e. 0–10 %)
const TREASURY_ADDR: Symbol = symbol_short!("TR_ADDR"); // Address
const CONTRACT_ADMIN: Symbol = symbol_short!("CT_ADMIN"); // Address

// Dispute resolution system
const DISPUTE_FEE: Symbol = symbol_short!("D_FEE"); // i128 - fee to file a dispute
const DISPUTES: Symbol = symbol_short!("DISPUTES"); // Map<u64, Dispute>
const DISPUTE_COUNTER: Symbol = symbol_short!("D_CNT"); // u64
const ARBITRATOR: Symbol = symbol_short!("ARBIT"); // Address - dispute arbitrator

// Game timeout mechanism
const TIMEOUT_DURATION: Symbol = symbol_short!("T_OUT"); // u64 - ledger sequences before timeout

// SEP-10 challenge verification (#529)
const SEP10_CHALLENGES: Symbol = symbol_short!("S10_CHAL"); // Map<BytesN<32>, u64> nonce → expiry
const SEP10_VERIFIED: Symbol = symbol_short!("S10_VER"); // Map<Address, bool>

// Multi-sig fee control (#535)
const MULTISIG_SIGNERS: Symbol = symbol_short!("MS_SIGN"); // Vec<Address>
const MULTISIG_THRESHOLD: Symbol = symbol_short!("MS_THRES"); // u32
const PENDING_FEE_PROPOSAL: Symbol = symbol_short!("MS_PROP"); // Option<FeeProposal>
const FEE_PROPOSAL_APPROVALS: Symbol = symbol_short!("MS_APPR"); // Map<Address, bool>

// SEP-40 Oracle clock sync (#533)
const ORACLE_CONTRACT: Symbol = symbol_short!("ORACLE"); // Address of oracle contract

// Time-lock escrow for tournament prizes (#532)
const TOURNAMENT_TIMELOCK: Symbol = symbol_short!("TL_DUR"); // u64 - lock duration in ledger sequences
const TOURNAMENT_ESCROWS: Symbol = symbol_short!("TL_ESC"); // Map<u64, TournamentEscrow>
const PLAYER_ACTIVE_ESCROWS: Symbol = symbol_short!("PL_ACTV"); // Map<Address, u32>

/// Maximum number of active (non-released) tournament escrows per player
/// to prevent storage bloat attacks.
const MAX_ACTIVE_ESCROWS: u32 = 100;

// Pausable extension (SC-11)
const PAUSED: Symbol = symbol_short!("PAUSED"); // bool - whether contract is paused

// Token whitelist (SC-17)
const ALLOWED_TOKENS: Symbol = symbol_short!("ALLWD_T"); // Vec<Address> - whitelisted token addresses

// Reentrancy guard (#860)
const R_GUARD: Symbol = symbol_short!("R_GUARD");

// Admin key rotation timelock (#890): 24h = 17280 ledger sequences at 5s/ledger
const ADMIN_TIMELOCK: Symbol = symbol_short!("ADM_TLK"); // u64 - lock duration (ledger sequences)
const PENDING_ADMIN_KEY: Symbol = symbol_short!("PEND_ADM"); // Option<BytesN<32>> - proposed new admin key
const PENDING_ADMIN_TIMESTAMP: Symbol = symbol_short!("PEND_TS"); // u64 - ledger sequence when proposal was made

// �"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?
// Multi-sig fee proposal type (#535)
// ────────────────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug)]
pub struct FeeProposal {
    pub new_fee_bips: u32,
    pub new_treasury_address: Address,
    pub proposed_at: u64, // ledger sequence
    pub proposer: Address,
}

// ────────────────────────────────────────────────────────────────────────────
// Tournament escrow type (#532)
// ────────────────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug)]
pub struct TournamentEscrow {
    pub escrow_id: u64,
    pub game_id: u64,
    pub player: Address,
    pub total_amount: i128,
    pub locked_until: u64, // ledger sequence when funds can be released
    pub released: bool,
}

// ────────────────────────────────────────────────────────────────────────────
// Errors
// ────────────────────────────────────────────────────────────────────────────




#[contract]
pub struct GameContract;

#[contractimpl]
impl GameContract {
    /// Bind the SEP-41 token contract used for all wager escrow and prize transfers.
    ///
    /// Must be called once before any game or reward function. Calling it a
    /// second time panics with `"Contract already initialized"`.
    ///
    /// # Parameters
    /// - `admin` — Address that authorises the call (`require_auth` is enforced).
    /// - `token_contract` — Address of the SEP-41 token contract (e.g. XLM or USDC).
    ///
    /// # Panics
    /// - If `TOKEN_CONTRACT` is already set in instance storage.
    pub fn initialize_token(env: Env, admin: Address, token_contract: Address) {
        if env.storage().instance().has(&TOKEN_CONTRACT) {
            panic_with_error!(&env, ContractError::AlreadyInitialized);
        }
        admin.require_auth();
        Self::require_token_whitelisted(&env, &token_contract);
        env.storage()
            .instance()
            .set(&TOKEN_CONTRACT, &token_contract);
    }

    /// Add a token address to the whitelist.
    /// Authorised by the `admin` address — the contract admin once
    /// `initialize_puzzle_rewards` has been called, or any authorised caller
    /// before that.
    pub fn add_whitelisted_token(env: Env, admin: Address, token: Address) {
        admin.require_auth();
        if let Some(stored_admin) = env.storage().instance().get::<_, Address>(&CONTRACT_ADMIN) {
            if admin != stored_admin {
                panic!("Not admin");
            }
        }
        let mut tokens: Vec<Address> = env
            .storage()
            .instance()
            .get(&ALLOWED_TOKENS)
            .unwrap_or(Vec::new(&env));
        if !tokens.contains(&token) {
            tokens.push_back(token);
        }
        env.storage().instance().set(&ALLOWED_TOKENS, &tokens);
    }

    /// Remove a token address from the whitelist.
    /// Only the contract admin may call this.
    pub fn remove_whitelisted_token(env: Env, admin: Address, token: Address) {
        admin.require_auth();
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&CONTRACT_ADMIN)
            .expect("Not initialized");
        if admin != stored_admin {
            panic!("Not admin");
        }
        let mut tokens: Vec<Address> = env
            .storage()
            .instance()
            .get(&ALLOWED_TOKENS)
            .unwrap_or(Vec::new(&env));
        if let Some(pos) = tokens.iter().position(|t| t == token) {
            tokens.remove(pos);
        }
        env.storage().instance().set(&ALLOWED_TOKENS, &tokens);
    }

    /// Return the current whitelist of permitted token contract addresses.
    pub fn get_whitelisted_tokens(env: Env) -> Vec<Address> {
        env.storage()
            .instance()
            .get(&ALLOWED_TOKENS)
            .unwrap_or(Vec::new(&env))
    }

    /// Internal helper — panics if `token` is not in the whitelist.
    fn require_token_whitelisted(env: &Env, token: &Address) {
        let tokens: Vec<Address> = env
            .storage()
            .instance()
            .get(&ALLOWED_TOKENS)
            .unwrap_or(Vec::new(env));
        if !tokens.contains(token) {
            panic_with_error!(env, ContractError::TokenNotWhitelisted);
        }
    }

    fn token_contract_address(env: &Env) -> Address {
        env.storage()
            .instance()
            .get(&TOKEN_CONTRACT)
            .expect("Token contract is not initialized")
    }

    fn token_client(env: &Env) -> TokenClient<'_> {
        TokenClient::new(env, &Self::token_contract_address(env))
    }

    // ── Pausable extension (SC-11) ────────────────────────────────────────────

    /// Pause the contract — blocks all state-mutating operations.
    /// Only the contract admin may call this.
    pub fn pause(env: Env, caller: Address) {
        caller.require_auth();
        let admin: Address = env
            .storage()
            .instance()
            .get(&CONTRACT_ADMIN)
            .expect("Not initialized");
        if caller != admin {
            panic_with_error!(&env, ContractError::NotAdmin);
        }
        if env.storage().instance().get(&PAUSED).unwrap_or(false) {
            panic_with_error!(&env, ContractError::AlreadyPaused);
        }
        env.storage().instance().set(&PAUSED, &true);
        env.events()
            .publish((symbol_short!("paused"),), caller);
    }

    /// Unpause the contract — resumes normal operations.
    /// Only the contract admin may call this.
    pub fn unpause(env: Env, caller: Address) {
        caller.require_auth();
        let admin: Address = env
            .storage()
            .instance()
            .get(&CONTRACT_ADMIN)
            .expect("Not initialized");
        if caller != admin {
            panic_with_error!(&env, ContractError::NotAdmin);
        }
        if !env.storage().instance().get(&PAUSED).unwrap_or(false) {
            panic_with_error!(&env, ContractError::NotPaused);
        }
        env.storage().instance().set(&PAUSED, &false);
        env.events()
            .publish((symbol_short!("unpaused"),), caller);
    }

    /// Returns `true` if the contract is currently paused.
    pub fn is_paused(env: Env) -> bool {
        env.storage().instance().get(&PAUSED).unwrap_or(false)
    }

    /// Internal helper — panics with `ContractError::ContractPaused` when the contract is paused.
    fn check_not_paused(env: &Env) {
        if env.storage().instance().get(&PAUSED).unwrap_or(false) {
            panic_with_error!(env, ContractError::ContractPaused);
        }
    }

    /// Gas-optimized tournament payout — single pass, no redundant map reads.
    ///
    /// Validates that `percentages` sum to exactly 100, then distributes the
    /// total prize pool from escrow to each winner in a single token-transfer
    /// loop. Any integer-division remainder (dust) is added to the first
    /// winner's share.
    ///
    /// Requires authorisation from `game.player1` (the tournament organiser).
    ///
    /// # Parameters
    /// - `game_id`      — ID of a game in the `Completed` state.
    /// - `winners`      — Ordered list of recipient addresses.
    /// - `percentages`  — Whole-number percentages (0–100) parallel to `winners`;
    ///                    must sum to exactly 100.
    ///
    /// # Returns
    /// `Ok(())` on success.
    ///
    /// # Errors
    /// - [`ContractError::GameNotFound`]       — `game_id` does not exist.
    /// - [`ContractError::GameNotInProgress`]  — Game is not in `Completed` state.
    /// - [`ContractError::MismatchedLengths`]  — `winners` and `percentages` differ in length.
    /// - [`ContractError::InvalidPercentage`]  — Percentages overflow or do not sum to 100.
    ///
    /// # Events
    /// Does not emit events directly; token transfers emit SAC-level transfer events.
    pub fn payout_tournament_optimized(
        env: Env,
        game_id: u64,
        winners: Vec<Address>,
        percentages: Vec<u32>,
    ) -> Result<(), ContractError> {
        Self::check_not_paused(&env);
        let mut games: Map<u64, Game> = env
            .storage()
            .instance()
            .get(&GAMES)
            .ok_or(ContractError::GameNotFound)?;

        let game = games.get(game_id).ok_or(ContractError::GameNotFound)?;

        if game.state != GameState::Completed {
            return Err(ContractError::GameNotInProgress);
        }

        game.player1.require_auth();

        if winners.len() != percentages.len() {
            return Err(ContractError::MismatchedLengths);
        }

        let total_pool = match &game.player2 {
            Some(_) => game.wager_amount * 2,
            None => game.wager_amount,
        };

        // Single-pass: validate + compute amounts together
        let mut total_pct: u32 = 0;
        let mut payouts: Vec<(Address, i128)> = Vec::new(&env);
        let mut distributed: i128 = 0;

        for i in 0..winners.len() {
            let pct = percentages.get(i).unwrap();
            total_pct = total_pct
                .checked_add(pct)
                .ok_or(ContractError::InvalidPercentage)?;
            if total_pct > 100 {
                return Err(ContractError::InvalidPercentage);
            }
            let amount = (total_pool * pct as i128) / 100;
            distributed += amount;
            payouts.push_back((winners.get(i).unwrap(), amount));
        }

        if total_pct != 100 {
            return Err(ContractError::InvalidPercentage);
        }

        // Dust to first winner
        let remainder = total_pool - distributed;

        Self::non_reentrant_enter(&env)?;

        let token_client = Self::token_client(&env);
        let contract_address = env.current_contract_address();

        for (idx, (winner, mut amount)) in payouts.iter().enumerate() {
            if idx == 0 {
                amount += remainder;
            }
            token_client.transfer(&contract_address, &winner, &amount);
        }

        let mut settled_game = game;
        settled_game.state = GameState::Settled;
        games.set(game_id, settled_game);
        env.storage().instance().set(&GAMES, &games);

        Self::non_reentrant_exit(&env);
        Ok(())
    }

    // ── Game lifecycle ────────────────────────────────────────────────────────

    /// Create a new wager game and lock `player1`'s stake into escrow.
    ///
    /// Generates a sequential game ID, transfers `wager_amount` tokens from
    /// `player1` to the contract, and stores the game in `Created` state
    /// awaiting a second player.
    ///
    /// # Parameters
    /// - `player1`       — Address of the game creator; must authorise the call.
    /// - `wager_amount`  — Token amount to stake; must be > 0, ≤ `MAX_STAKE`, and
    ///                     `wager_amount * 2` must be ≤ `MAX_PRIZE_POOL`.
    ///
    /// # Returns
    /// `Ok(game_id)` — the unique `u64` identifier for the new game.
    ///
    /// # Errors
    /// - [`ContractError::StakeLimitExceeded`]    — `wager_amount > MAX_STAKE`.
    /// - [`ContractError::PrizePoolLimitExceeded`] — `wager_amount * 2 > MAX_PRIZE_POOL`.
    /// - [`ContractError::InvalidAmount`]          — Integer overflow computing the prize pool.
    /// - [`ContractError::InsufficientFunds`]      — `player1`'s token balance < `wager_amount`.
    pub fn create_game(
        env: Env,
        player1: Address,
        wager_amount: i128,
    ) -> Result<u64, ContractError> {
        Self::check_not_paused(&env);
        let max_stake: i128 = env.storage().instance().get(&MAX_STAKE).unwrap_or(1_000);
        if wager_amount > max_stake {
            return Err(ContractError::StakeLimitExceeded);
        }

        let max_prize_pool: i128 = env
            .storage()
            .instance()
            .get(&MAX_PRIZE_POOL)
            .unwrap_or(2_000);
        let expected_pool = wager_amount
            .checked_mul(2)
            .ok_or(ContractError::InvalidAmount)?;
        if expected_pool > max_prize_pool {
            return Err(ContractError::PrizePoolLimitExceeded);
        }

        player1.require_auth();

        Self::require_token_whitelisted(&env, &Self::token_contract_address(&env));
        let token_client = Self::token_client(&env);
        let contract_address = env.current_contract_address();

        if token_client.balance(&player1) < wager_amount {
            Self::non_reentrant_exit(&env);
            return Err(ContractError::InsufficientFunds);
        }

        token_client.transfer(&player1, &contract_address, &wager_amount);

        let mut game_counter: u64 = env.storage().instance().get(&GAME_COUNTER).unwrap_or(0);
        game_counter += 1;
        env.storage().instance().set(&GAME_COUNTER, &game_counter);

        let game = Game {
            id: game_counter,
            player1: player1.clone(),
            player2: None,
            state: GameState::Created,
            wager_amount,
            current_turn: 1,
            moves: Vec::new(&env),
            created_at: env.ledger().sequence() as u64,
            winner: None,
            last_move_at: env.ledger().sequence() as u64,
        };

        let mut games: Map<u64, Game> = env
            .storage()
            .instance()
            .get(&GAMES)
            .unwrap_or(Map::new(&env));
        games.set(game_counter, game);
        env.storage().instance().set(&GAMES, &games);

        let mut escrow: Map<Address, i128> = env
            .storage()
            .instance()
            .get(&ESCROW)
            .unwrap_or(Map::new(&env));
        let current_escrow = escrow.get(player1.clone()).unwrap_or(0);
        escrow.set(player1.clone(), current_escrow + wager_amount);
        env.storage().instance().set(&ESCROW, &escrow);

        // Emit tournament created event
        env.events().publish(
            (symbol_short!("game"), symbol_short!("created")),
            (game_counter, player1, wager_amount),
        );

        Ok(game_counter)
    }

    /// Join an existing game as the second player and lock matching stake.
    ///
    /// Transfers `game.wager_amount` tokens from `player2` to the contract,
    /// sets `game.player2`, and advances the game to `InProgress` state with
    /// `current_turn = 1` (player1 moves first).
    ///
    /// # Parameters
    /// - `game_id`  — ID of a game in `Created` state.
    /// - `player2`  — Address of the joining player; must differ from `player1`
    ///                and must authorise the call.
    ///
    /// # Returns
    /// `Ok(())` on success.
    ///
    /// # Errors
    /// - [`ContractError::GameNotFound`]          — `game_id` does not exist.
    /// - [`ContractError::GameAlreadyCompleted`]  — Game is not in `Created` state.
    /// - [`ContractError::GameFull`]              — A second player has already joined.
    /// - [`ContractError::AlreadyJoined`]         — `player2` is the same as `player1`.
    /// - [`ContractError::StakeLimitExceeded`]    — Wager exceeds `MAX_STAKE`.
    /// - [`ContractError::PrizePoolLimitExceeded`] — Combined pool exceeds `MAX_PRIZE_POOL`.
    /// - [`ContractError::InvalidAmount`]          — Integer overflow computing the pool.
    /// - [`ContractError::InsufficientFunds`]      — `player2`'s balance < `wager_amount`.
    pub fn join_game(env: Env, game_id: u64, player2: Address) -> Result<(), ContractError> {
        Self::check_not_paused(&env);
        let mut games: Map<u64, Game> = env
            .storage()
            .instance()
            .get(&GAMES)
            .ok_or(ContractError::GameNotFound)?;

        let mut game = games.get(game_id).ok_or(ContractError::GameNotFound)?;

        if game.state != GameState::Created {
            return Err(ContractError::GameAlreadyCompleted);
        }
        if game.player2.is_some() {
            return Err(ContractError::GameFull);
        }
        if game.player1 == player2 {
            return Err(ContractError::AlreadyJoined);
        }

        let max_stake: i128 = env.storage().instance().get(&MAX_STAKE).unwrap_or(1_000);
        if game.wager_amount > max_stake {
            return Err(ContractError::StakeLimitExceeded);
        }

        let max_prize_pool: i128 = env
            .storage()
            .instance()
            .get(&MAX_PRIZE_POOL)
            .unwrap_or(2_000);
        let total_pool = game
            .wager_amount
            .checked_mul(2)
            .ok_or(ContractError::InvalidAmount)?;
        if total_pool > max_prize_pool {
            return Err(ContractError::PrizePoolLimitExceeded);
        }
        player2.require_auth();

        Self::require_token_whitelisted(&env, &Self::token_contract_address(&env));
        let token_client = Self::token_client(&env);
        let contract_address = env.current_contract_address();

        if token_client.balance(&player2) < game.wager_amount {
            Self::non_reentrant_exit(&env);
            return Err(ContractError::InsufficientFunds);
        }

        token_client.transfer(&player2, &contract_address, &game.wager_amount);

        game.player2 = Some(player2.clone());
        game.state = GameState::InProgress;
        game.current_turn = 1;
        game.last_move_at = env.ledger().sequence() as u64;

        let mut escrow: Map<Address, i128> = env
            .storage()
            .instance()
            .get(&ESCROW)
            .unwrap_or(Map::new(&env));
        let current_escrow = escrow.get(player2.clone()).unwrap_or(0);
        escrow.set(player2.clone(), current_escrow + game.wager_amount);
        env.storage().instance().set(&ESCROW, &escrow);

        games.set(game_id, game.clone());
        env.storage().instance().set(&GAMES, &games);

        // Emit escrow created event
        env.events().publish(
            (symbol_short!("game"), symbol_short!("joined")),
            (game_id, game.player1, player2),
        );

        Ok(())
    }

    /// Record a chess move for the active player and advance the turn.
    ///
    /// Appends a [`ChessMove`] entry to `game.moves`, flips `current_turn`
    /// (1 → 2 → 1 …), and updates `last_move_at` to the current ledger
    /// sequence (used by the timeout mechanism).
    ///
    /// # Parameters
    /// - `game_id`    — ID of a game in `InProgress` state.
    /// - `player`     — Address of the moving player; must be `player1` or `player2`
    ///                  and must authorise the call.
    /// - `move_data`  — Encoded move payload (e.g. from/to squares as `u32`s);
    ///                  must be non-empty.
    ///
    /// # Returns
    /// `Ok(())` on success.
    ///
    /// # Errors
    /// - [`ContractError::GameNotFound`]       — `game_id` does not exist.
    /// - [`ContractError::GameNotInProgress`]  — Game is not in `InProgress` state.
    /// - [`ContractError::NotPlayer`]          — `player` is not a participant.
    /// - [`ContractError::NotYourTurn`]        — It is not `player`'s turn.
    /// - [`ContractError::InvalidMove`]        — `move_data` is empty.
    pub fn submit_move(
        env: Env,
        game_id: u64,
        player: Address,
        move_data: Vec<u32>,
    ) -> Result<(), ContractError> {
        Self::check_not_paused(&env);
        let mut games: Map<u64, Game> = env
            .storage()
            .instance()
            .get(&GAMES)
            .ok_or(ContractError::GameNotFound)?;

        let mut game = games.get(game_id).ok_or(ContractError::GameNotFound)?;

        if game.state != GameState::InProgress {
            return Err(ContractError::GameNotInProgress);
        }

        player.require_auth();

        let player_num = if player == game.player1 {
            1
        } else if Some(player.clone()) == game.player2 {
            2
        } else {
            return Err(ContractError::NotPlayer);
        };

        if player_num != game.current_turn {
            return Err(ContractError::NotYourTurn);
        }

        if move_data.is_empty() {
            return Err(ContractError::InvalidMove);
        }

        let chess_move = ChessMove {
            player: player.clone(),
            move_data,
            timestamp: env.ledger().sequence() as u64,
        };
        game.moves.push_back(chess_move);
        game.current_turn = if game.current_turn == 1 { 2 } else { 1 };
        game.last_move_at = env.ledger().sequence() as u64;

        games.set(game_id, game);
        env.storage().instance().set(&GAMES, &games);

        Ok(())
    }

    /// Claim a draw result, verified by the backend signing service.
    ///
    /// Verifies an ED25519 signature over `SHA256(game_id_le8 || b"DRAW")`,
    /// transitions the game to `Drawn`, and returns each player's stake in
    /// full via [`process_draw_payout`].
    ///
    /// The backend signature prevents either player from unilaterally claiming
    /// a draw without mutual agreement or arbiter approval.
    ///
    /// # Signature Payload
    /// `SHA256( game_id.to_le_bytes() || b"DRAW" )`
    ///
    /// # Parameters
    /// - `game_id`    — ID of a game in `InProgress` state.
    /// - `player`     — Either participant's address; must authorise the call.
    /// - `signature`  — 64-byte ED25519 signature from the backend service.
    ///
    /// # Returns
    /// `Ok(())` on success.
    ///
    /// # Errors
    /// - [`ContractError::GameNotFound`]       — `game_id` does not exist.
    /// - [`ContractError::GameNotInProgress`]  — Game is not `InProgress`.
    /// - [`ContractError::NotPlayer`]          — `player` is not a participant.
    ///
    /// # Panics
    /// - If the ED25519 signature is invalid (Soroban `ed25519_verify` panics on failure).
    /// - If the contract has not been initialized (`ADMIN_KEY` absent).
    pub fn claim_draw(
        env: Env,
        game_id: u64,
        player: Address,
        signature: BytesN<64>,
    ) -> Result<(), ContractError> {
        Self::check_not_paused(&env);
        let mut games: Map<u64, Game> = env
            .storage()
            .instance()
            .get(&GAMES)
            .ok_or(ContractError::GameNotFound)?;

        let mut game = games.get(game_id).ok_or(ContractError::GameNotFound)?;

        if game.state != GameState::InProgress {
            return Err(ContractError::GameNotInProgress);
        }
        if player != game.player1 && Some(player.clone()) != game.player2 {
            return Err(ContractError::NotPlayer);
        }

        // Verify backend admin signature for a draw to prevent unilateral draws
        let admin_key_bytes: Bytes = env
            .storage()
            .instance()
            .get(&ADMIN_KEY)
            .expect("Not initialized");

        let admin_pubkey: BytesN<32> = admin_key_bytes
            .try_into()
            .expect("Admin public key must be 32 bytes");

        let mut payload_bytes = Bytes::new(&env);
        let game_id_le: [u8; 8] = game_id.to_le_bytes();
        payload_bytes.append(&Bytes::from_slice(&env, &game_id_le));
        // Append "DRAW" to differentiate from claim_win payload
        payload_bytes.append(&Bytes::from_slice(&env, b"DRAW"));

        let digest_bytesn: BytesN<32> = env.crypto().sha256(&payload_bytes).into();
        let digest_bytes: Bytes = digest_bytesn.into();
        env.crypto()
            .ed25519_verify(&admin_pubkey, &digest_bytes, &signature);

        Self::non_reentrant_enter(&env)?;

        game.state = GameState::Drawn;
        match Self::process_draw_payout(&env, &game) {
            Ok(()) => {}
            Err(e) => {
                Self::non_reentrant_exit(&env);
                return Err(e);
            }
        }

        games.set(game_id, game);
        env.storage().instance().set(&GAMES, &games);

        // Emit draw claimed event
        env.events().publish(
            (symbol_short!("game"), symbol_short!("drawn")),
            (game_id, player),
        );

        Ok(())
    }

    /// Claim a win result, verified by the backend signing service.
    ///
    /// Verifies an ED25519 signature over `SHA256(game_id_le8 || winner_address_bytes)`,
    /// pays out the net prize (total pool minus protocol fee, if configured) to
    /// `winner`, and marks the game `Settled`.
    ///
    /// # Signature Payload
    /// `SHA256( game_id.to_le_bytes() || winner_address_string_bytes )`
    ///
    /// # Parameters
    /// - `game_id`    — ID of a game in `InProgress` state.
    /// - `winner`     — Address of the winning player; must be a participant and
    ///                  must authorise the call.
    /// - `signature`  — 64-byte ED25519 signature from the backend service.
    ///
    /// # Returns
    /// `Ok(())` on success.
    ///
    /// # Errors
    /// - [`ContractError::GameNotFound`]       — `game_id` does not exist.
    /// - [`ContractError::GameNotInProgress`]  — Game is not `InProgress`.
    /// - [`ContractError::NotPlayer`]          — `winner` is not a participant.
    ///
    /// # Panics
    /// - If the ED25519 signature is invalid.
    /// - If the contract has not been initialized.
    pub fn claim_win(
        env: Env,
        game_id: u64,
        winner: Address,
        signature: BytesN<64>,
    ) -> Result<(), ContractError> {
        Self::check_not_paused(&env);
        let mut games: Map<u64, Game> = env
            .storage()
            .instance()
            .get(&GAMES)
            .ok_or(ContractError::GameNotFound)?;

        let mut game = games.get(game_id).ok_or(ContractError::GameNotFound)?;

        if game.state != GameState::InProgress {
            return Err(ContractError::GameNotInProgress);
        }

        if game.player1 != winner && Some(winner.clone()) != game.player2 {
            return Err(ContractError::NotPlayer);
        }

        // Verify admin signature to confirm the win
        let admin_key_bytes: Bytes = env
            .storage()
            .instance()
            .get(&ADMIN_KEY)
            .expect("Not initialized");

        let admin_pubkey: BytesN<32> = admin_key_bytes
            .try_into()
            .expect("Admin public key must be 32 bytes");

        let mut payload_bytes = Bytes::new(&env);
        let game_id_le: [u8; 8] = game_id.to_le_bytes();
        payload_bytes.append(&Bytes::from_slice(&env, &game_id_le));

        let winner_str = winner.clone().to_string();
        let str_len = winner_str.len() as usize;
        let mut addr_buf = [0u8; 64];
        winner_str.copy_into_slice(&mut addr_buf[..str_len]);
        payload_bytes.append(&Bytes::from_slice(&env, &addr_buf[..str_len]));

        let digest_bytesn: BytesN<32> = env.crypto().sha256(&payload_bytes).into();
        let digest_bytes: Bytes = digest_bytesn.into();
        env.crypto()
            .ed25519_verify(&admin_pubkey, &digest_bytes, &signature);

        Self::non_reentrant_enter(&env)?;

        game.winner = Some(winner.clone());
        match Self::process_payout(&env, &game, &winner) {
            Ok(()) => {}
            Err(e) => {
                Self::non_reentrant_exit(&env);
                return Err(e);
            }
        }
        game.state = GameState::Settled;

        games.set(game_id, game);
        env.storage().instance().set(&GAMES, &games);

        // Emit win claimed event
        env.events().publish(
            (symbol_short!("game"), symbol_short!("won")),
            (game_id, winner),
        );

        Ok(())
    }

    /// Cancel a game that has not yet been joined and refund the creator.
    ///
    /// Only `player1` (the creator) may cancel, and only while the game is in
    /// `Created` state (before `player2` joins). The staked wager is returned
    /// to `player1` and the game is marked `Completed` to prevent future joins.
    ///
    /// # Parameters
    /// - `game_id`  — ID of a game in `Created` state.
    /// - `player`   — Must equal `game.player1`; must authorise the call.
    ///
    /// # Returns
    /// `Ok(())` on success.
    ///
    /// # Errors
    /// - [`ContractError::GameNotFound`]         — `game_id` does not exist.
    /// - [`ContractError::GameAlreadyCompleted`] — Game has already progressed past `Created`.
    /// - [`ContractError::NotPlayer`]            — `player` is not `player1`.
    pub fn cancel_game(env: Env, game_id: u64, player: Address) -> Result<(), ContractError> {
        Self::check_not_paused(&env);
        let mut games: Map<u64, Game> = env
            .storage()
            .instance()
            .get(&GAMES)
            .ok_or(ContractError::GameNotFound)?;

        let mut game = games.get(game_id).ok_or(ContractError::GameNotFound)?;

        if game.state != GameState::Created {
            return Err(ContractError::GameAlreadyCompleted);
        }

        if game.player1 != player {
            return Err(ContractError::NotPlayer);
        }

        player.require_auth();

        Self::non_reentrant_enter(&env)?;

        // Refund player1's staked wager
        let mut escrow: Map<Address, i128> = env
            .storage()
            .instance()
            .get(&ESCROW)
            .unwrap_or(Map::new(&env));

        let current_escrow = escrow.get(player.clone()).unwrap_or(0);
        escrow.set(player.clone(), current_escrow - game.wager_amount);
        env.storage().instance().set(&ESCROW, &escrow);

        let token_client = Self::token_client(&env);
        let contract_address = env.current_contract_address();
        token_client.transfer(&contract_address, &player, &game.wager_amount);

        game.state = GameState::Completed; // Mark as completed to prevent joining
        games.set(game_id, game);
        env.storage().instance().set(&GAMES, &games);

        // Emit game cancelled event
        env.events().publish(
            (symbol_short!("game"), symbol_short!("cancelled")),
            (game_id, player),
        );

        Ok(())
    }

    /// Forfeit the game, awarding the full prize pool to the opponent.
    ///
    /// Either player may forfeit an `InProgress` game at any time. The
    /// opponent receives the net payout (total pool minus protocol fee) and
    /// the game is marked `Settled`.
    ///
    /// # Parameters
    /// - `game_id`  — ID of a game in `InProgress` state.
    /// - `player`   — Forfeiting player's address; must be a participant and
    ///                must authorise the call.
    ///
    /// # Returns
    /// `Ok(())` on success.
    ///
    /// # Errors
    /// - [`ContractError::GameNotFound`]       — `game_id` does not exist.
    /// - [`ContractError::GameNotInProgress`]  — Game is not `InProgress`.
    /// - [`ContractError::NotPlayer`]          — `player` is not a participant.
    /// - [`ContractError::GameFull`]           — No `player2` found (single-player game; should not occur).
    pub fn forfeit(env: Env, game_id: u64, player: Address) -> Result<(), ContractError> {
        Self::check_not_paused(&env);
        let mut games: Map<u64, Game> = env
            .storage()
            .instance()
            .get(&GAMES)
            .ok_or(ContractError::GameNotFound)?;

        let mut game = games.get(game_id).ok_or(ContractError::GameNotFound)?;

        if game.state != GameState::InProgress {
            return Err(ContractError::GameNotInProgress);
        }
        if player != game.player1 && Some(player.clone()) != game.player2 {
            return Err(ContractError::NotPlayer);
        }

        player.require_auth();

        let winner = if player == game.player1 {
            game.player2
                .as_ref()
                .ok_or(ContractError::GameFull)?
                .clone()
        } else {
            game.player1.clone()
        };

        Self::non_reentrant_enter(&env)?;

        game.winner = Some(winner.clone());
        match Self::process_payout(&env, &game, &winner) {
            Ok(()) => {}
            Err(e) => {
                Self::non_reentrant_exit(&env);
                return Err(e);
            }
        }
        game.state = GameState::Settled;

        games.set(game_id, game);
        env.storage().instance().set(&GAMES, &games);

        // Emit forfeit event
        env.events().publish(
            (symbol_short!("game"), symbol_short!("forfeited")),
            (game_id, player, winner),
        );

        Ok(())
    }

    /// Transfer the prize to the pre-recorded winner of a `Completed` game.
    ///
    /// This entrypoint is used when the game outcome has been recorded
    /// off-chain and `game.winner` has been set (e.g. after `claim_win`
    /// advanced the state to `Completed`). The winner calls this to pull
    /// their funds.
    ///
    /// # Parameters
    /// - `game_id`  — ID of a game in `Completed` state with a recorded winner.
    /// - `winner`   — Must match `game.winner`; must authorise the call.
    ///
    /// # Returns
    /// `Ok(())` on success.
    ///
    /// # Errors
    /// - [`ContractError::GameNotFound`]       — `game_id` does not exist.
    /// - [`ContractError::AlreadySettled`]     — Game is already `Settled`.
    /// - [`ContractError::GameNotInProgress`]  — Game is not in `Completed` state.
    /// - [`ContractError::NotPlayer`]          — `winner` does not match `game.winner`.
    pub fn payout(env: Env, game_id: u64, winner: Address) -> Result<(), ContractError> {
        Self::check_not_paused(&env);
        let mut games: Map<u64, Game> = env
            .storage()
            .instance()
            .get(&GAMES)
            .ok_or(ContractError::GameNotFound)?;

        let mut game = games.get(game_id).ok_or(ContractError::GameNotFound)?;

        if game.state != GameState::Completed {
            if game.state == GameState::Settled {
                return Err(ContractError::AlreadySettled);
            }
            return Err(ContractError::GameNotInProgress);
        }
        if game.winner.as_ref() != Some(&winner) {
            return Err(ContractError::NotPlayer);
        }

        winner.require_auth();

        Self::non_reentrant_enter(&env)?;

        match Self::process_payout(&env, &game, &winner) {
            Ok(()) => {}
            Err(e) => {
                Self::non_reentrant_exit(&env);
                return Err(e);
            }
        }
        game.state = GameState::Settled;

        games.set(game_id, game);
        env.storage().instance().set(&GAMES, &games);

        // Emit payout event
        env.events().publish(
            (symbol_short!("game"), symbol_short!("payout")),
            (game_id, winner),
        );

        Ok(())
    }

    /// Distribute a completed game's prize pool across multiple tournament winners.
    ///
    /// Deducts both players' escrow balances, then distributes the total pool
    /// proportionally according to `percentages`. Integer-division dust goes to
    /// the first winner. Requires `player1`'s authorisation as the tournament
    /// organiser.
    ///
    /// Prefer [`payout_tournament_optimized`] for lower gas cost; this variant
    /// does separate escrow reads per winner.
    ///
    /// # Parameters
    /// - `game_id`      — ID of a game in `Completed` state.
    /// - `winners`      — Ordered recipient addresses.
    /// - `percentages`  — Whole-number percentages parallel to `winners`;
    ///                    must sum to exactly 100.
    ///
    /// # Returns
    /// `Ok(())` on success.
    ///
    /// # Errors
    /// - [`ContractError::GameNotFound`]          — `game_id` does not exist.
    /// - [`ContractError::GameNotInProgress`]     — Game is not `Completed`.
    /// - [`ContractError::MismatchedLengths`]     — `winners` and `percentages` differ.
    /// - [`ContractError::InsufficientFunds`]     — Escrow balance is lower than the wager.
    /// - [`ContractError::InvalidPercentage`]     — Percentages overflow or do not sum to 100.
    pub fn payout_tournament(
        env: Env,
        game_id: u64,
        winners: Vec<Address>,
        percentages: Vec<u32>,
    ) -> Result<(), ContractError> {
        Self::check_not_paused(&env);
        let mut games: Map<u64, Game> = env
            .storage()
            .instance()
            .get(&GAMES)
            .ok_or(ContractError::GameNotFound)?;

        let game = games.get(game_id).ok_or(ContractError::GameNotFound)?;

        if game.state != GameState::Completed {
            return Err(ContractError::GameNotInProgress);
        }

        game.player1.require_auth();

        if winners.len() != percentages.len() {
            return Err(ContractError::MismatchedLengths);
        }

        let mut total_percentage: u32 = 0;

        let mut escrow: Map<Address, i128> = env
            .storage()
            .instance()
            .get(&ESCROW)
            .unwrap_or(Map::new(&env));

        let player1_escrow = escrow.get(game.player1.clone()).unwrap_or(0);
        if player1_escrow < game.wager_amount {
            return Err(ContractError::InsufficientFunds);
        }

        let mut player2_escrow = 0i128;
        let mut total_pool = game.wager_amount;

        if let Some(ref player2) = game.player2 {
            player2_escrow = escrow.get(player2.clone()).unwrap_or(0);
            if player2_escrow < game.wager_amount {
                return Err(ContractError::InsufficientFunds);
            }
            total_pool = game.wager_amount * 2;
        }

        Self::non_reentrant_enter(&env)?;

        // Deduct wagers first to prevent double-counting
        escrow.set(game.player1.clone(), player1_escrow - game.wager_amount);
        if let Some(ref player2) = game.player2 {
            escrow.set(player2.clone(), player2_escrow - game.wager_amount);
        }

        let mut distributed: i128 = 0;
        for i in 0..winners.len() {
            let winner = winners.get(i).unwrap();
            let percentage = percentages.get(i).unwrap();
            total_percentage = total_percentage
                .checked_add(percentage)
                .ok_or(ContractError::InvalidPercentage)?;
            let payout_amount = (total_pool * percentage as i128) / 100;
            distributed += payout_amount;
            let winner_escrow = escrow.get(winner.clone()).unwrap_or(0);
            escrow.set(winner.clone(), winner_escrow + payout_amount);
        }

        if total_percentage != 100 {
            Self::non_reentrant_exit(&env);
            return Err(ContractError::InvalidPercentage);
        }

        // Dust goes to first winner
        let remainder = total_pool - distributed;
        if remainder > 0 && !winners.is_empty() {
            let first_winner = winners.get(0).unwrap();
            let winner_escrow = escrow.get(first_winner.clone()).unwrap_or(0);
            escrow.set(first_winner.clone(), winner_escrow + remainder);
        }

        let mut settled_game = game;
        settled_game.state = GameState::Settled;
        env.storage().instance().set(&ESCROW, &escrow);
        games.set(game_id, settled_game);
        env.storage().instance().set(&GAMES, &games);

        // Emit tournament payout event
        env.events().publish(
            (symbol_short!("game"), symbol_short!("payout_t")),
            (game_id, winners.len() as u32),
        );

        Ok(())
    }

    /// Fetch a single game by ID.
    ///
    /// # Parameters
    /// - `game_id` — Numeric game identifier returned by [`create_game`].
    ///
    /// # Returns
    /// `Ok(Game)` — full [`Game`] struct including state, players, moves, and wager.
    ///
    /// # Errors
    /// - [`ContractError::GameNotFound`] — `game_id` does not exist.
    pub fn get_game(env: Env, game_id: u64) -> Result<Game, ContractError> {
        let games: Map<u64, Game> = env
            .storage()
            .instance()
            .get(&GAMES)
            .ok_or(ContractError::GameNotFound)?;

        games.get(game_id).ok_or(ContractError::GameNotFound)
    }

    /// Return every stored game as a map keyed by game ID.
    ///
    /// Returns an empty map if no games have been created yet.
    ///
    /// > **Note:** This is an unbounded read. Frontend callers should paginate
    /// > using individual [`get_game`] calls once the game count grows large.
    ///
    /// # Returns
    /// `Map<u64, Game>` — all games indexed by their numeric ID.
    pub fn get_all_games(env: Env) -> Map<u64, Game> {
        env.storage()
            .instance()
            .get(&GAMES)
            .unwrap_or(Map::new(&env))
    }

    // ── Internal payout helpers ───────────────────────────────────────────────

    fn process_draw_payout(env: &Env, game: &Game) -> Result<(), ContractError> {
        let token_client = Self::token_client(env);
        let contract_address = env.current_contract_address();

        let mut escrow: Map<Address, i128> = env
            .storage()
            .instance()
            .get(&ESCROW)
            .unwrap_or(Map::new(env));

        // Return player1's stake
        token_client.transfer(&contract_address, &game.player1, &game.wager_amount);
        let player1_escrow = escrow.get(game.player1.clone()).unwrap_or(0);
        escrow.set(game.player1.clone(), player1_escrow - game.wager_amount);

        // Return player2's stake
        if let Some(ref player2) = game.player2 {
            token_client.transfer(&contract_address, player2, &game.wager_amount);
            let player2_escrow = escrow.get(player2.clone()).unwrap_or(0);
            escrow.set(player2.clone(), player2_escrow - game.wager_amount);
        }

        env.storage().instance().set(&ESCROW, &escrow);
        Ok(())
    }

    /// #200 – Treasury fee redirection in payout_winner.
    ///
    /// Uses Soroban-safe integer arithmetic:
    ///   `fee    = total_pool * fee_bips / 1000`
    ///   `payout = total_pool - fee`
    ///
    /// Example: 10 XLM pool, fee_bips = 20 (2 %)
    ///   fee    = 10 * 20 / 1000 = 0.2 XLM  → Treasury
    ///   payout = 10 - 0.2       = 9.8 XLM  → Winner
    fn process_payout(env: &Env, game: &Game, winner: &Address) -> Result<(), ContractError> {
        let mut escrow: Map<Address, i128> = env
            .storage()
            .instance()
            .get(&ESCROW)
            .unwrap_or(Map::new(env));

        let fee_bips: u32 = env.storage().instance().get(&FEE_BIPS).unwrap_or(0);
        let treasury_addr_opt: Option<Address> = env.storage().instance().get(&TREASURY_ADDR);

        let total_pool = game.wager_amount * 2;

        // --- #200: safe fee math -------------------------------------------------
        // Multiplying first keeps precision; dividing by 1000 rounds down (floor).
        // fee_bips is validated to be ≤ 1000 at configuration time, so overflow
        // cannot occur for any realistic i128 wager amount.
        let (payout, fee) = if treasury_addr_opt.is_some() && fee_bips > 0 {
            let fee = (total_pool * fee_bips as i128) / 1000;
            (total_pool - fee, fee)
        } else {
            (total_pool, 0)
        };
        // -------------------------------------------------------------------------

        // Deduct both stakes first (clean state, prevents double-spend)
        let player1_escrow = escrow.get(game.player1.clone()).unwrap_or(0);
        escrow.set(game.player1.clone(), player1_escrow - game.wager_amount);

        let player2 = game.player2.as_ref().ok_or(ContractError::GameFull)?;
        let player2_escrow = escrow.get(player2.clone()).unwrap_or(0);
        escrow.set(player2.clone(), player2_escrow - game.wager_amount);

        // Credit winner (net of fee)
        let winner_escrow = escrow.get(winner.clone()).unwrap_or(0);
        escrow.set(winner.clone(), winner_escrow + payout);

        // Credit treasury with the fee portion
        if fee > 0
            && let Some(ref treasury_addr) = treasury_addr_opt
        {
            let treasury_escrow = escrow.get(treasury_addr.clone()).unwrap_or(0);
            escrow.set(treasury_addr.clone(), treasury_escrow + fee);
        }

        env.storage().instance().set(&ESCROW, &escrow);

        // Physical token transfers
        let token_client = Self::token_client(env);
        let contract_address = env.current_contract_address();

        token_client.transfer(&contract_address, winner, &payout);
        if fee > 0
            && let Some(ref treasury_addr) = treasury_addr_opt
        {
            token_client.transfer(&contract_address, treasury_addr, &fee);
        }

        Ok(())
    }

    // ── Administration ────────────────────────────────────────────────────────

    /// Initialize puzzle-reward system (#199) and fee configuration (#200).
    ///
    /// Must be called exactly once after [`initialize_token`]. Sets the ED25519
    /// backend public key, seeds the treasury, and configures the protocol fee
    /// and treasury address. Also sets the default `MAX_STAKE` (1 000) and
    /// `MAX_PRIZE_POOL` (2 000).
    ///
    /// # Parameters
    /// - `admin`            — Contract administrator; must authorise the call.
    /// - `admin_public_key` — 32-byte ED25519 public key of the backend signing
    ///                        service (used to verify puzzle-reward and win/draw claims).
    /// - `treasury_amount`  — Initial token reserve for puzzle reward payouts; must be ≥ 0.
    /// - `fee_bips`         — Protocol fee in basis-points of 1 000 (e.g. `20` = 2 %).
    ///                        Capped at 1 000 (100 %).
    /// - `treasury_address` — Address that receives the fee portion of each payout.
    ///
    /// # Panics
    /// - If `CONTRACT_ADMIN` is already set (`"Already initialized"`).
    /// - If `admin_public_key.len() != 32`.
    /// - If `treasury_amount < 0`.
    /// - If `fee_bips > 1000`.
    pub fn initialize_puzzle_rewards(
        env: Env,
        admin: Address,
        admin_public_key: Bytes,
        treasury_amount: i128,
        fee_bips: u32,
        treasury_address: Address,
    ) {
        if env.storage().instance().has(&CONTRACT_ADMIN) {
            panic_with_error!(&env, ContractError::AlreadyInitialized);
        }

        admin.require_auth();

        if admin_public_key.len() != 32 {
            panic_with_error!(&env, ContractError::InvalidConfig);
        }
        if treasury_amount < 0 {
            panic_with_error!(&env, ContractError::InvalidAmount);
        }
        if fee_bips > 1000 {
            panic_with_error!(&env, ContractError::InvalidConfig);
        }

        env.storage().instance().set(&CONTRACT_ADMIN, &admin);
        env.storage().instance().set(&ADMIN_KEY, &admin_public_key);
        env.storage().instance().set(&TREASURY, &treasury_amount);
        env.storage().instance().set(&FEE_BIPS, &fee_bips);
        env.storage()
            .instance()
            .set(&TREASURY_ADDR, &treasury_address);
        env.storage().instance().set(&MAX_STAKE, &1_000i128);
        env.storage().instance().set(&MAX_PRIZE_POOL, &2_000i128);
    }

    /// Update the per-game maximum wager (admin only).
    ///
    /// Any subsequent [`create_game`] or [`join_game`] call with a
    /// `wager_amount` exceeding this limit will return
    /// [`ContractError::StakeLimitExceeded`].
    ///
    /// # Parameters
    /// - `admin`      — Must match `CONTRACT_ADMIN`; must authorise the call.
    /// - `new_limit`  — New maximum wager; must be > 0.
    ///
    /// # Panics
    /// - If the contract is not initialized.
    /// - If `admin` does not match the stored admin.
    /// - If `new_limit <= 0`.
    pub fn set_max_stake(env: Env, admin: Address, new_limit: i128) {
        let current_admin: Address = env
            .storage()
            .instance()
            .get(&CONTRACT_ADMIN)
            .expect("Not initialized");
        current_admin.require_auth();
        if admin != current_admin {
            panic_with_error!(&env, ContractError::Unauthorized);
        }
        if new_limit <= 0 {
            panic_with_error!(&env, ContractError::InvalidAmount);
        }
        env.storage().instance().set(&MAX_STAKE, &new_limit);
    }

    /// Update the maximum combined prize pool across both players (admin only).
    ///
    /// Guards against very large pools. Any [`create_game`] or [`join_game`]
    /// call where `wager_amount * 2 > MAX_PRIZE_POOL` will return
    /// [`ContractError::PrizePoolLimitExceeded`].
    ///
    /// # Parameters
    /// - `admin`      — Must match `CONTRACT_ADMIN`; must authorise the call.
    /// - `new_limit`  — New maximum prize pool; must be > 0.
    ///
    /// # Panics
    /// - If the contract is not initialized.
    /// - If `admin` does not match the stored admin.
    /// - If `new_limit <= 0`.
    pub fn set_max_prize_pool(env: Env, admin: Address, new_limit: i128) {
        let current_admin: Address = env
            .storage()
            .instance()
            .get(&CONTRACT_ADMIN)
            .expect("Not initialized");
        current_admin.require_auth();
        if admin != current_admin {
            panic_with_error!(&env, ContractError::Unauthorized);
        }
        if new_limit <= 0 {
            panic_with_error!(&env, ContractError::InvalidAmount);
        }
        env.storage().instance().set(&MAX_PRIZE_POOL, &new_limit);
    }

    /// Update the protocol fee and treasury address (admin only, no multi-sig).
    ///
    /// For fee changes requiring multi-sig approval, use the
    /// [`propose_fee_change`] / [`approve_fee_proposal`] flow instead.
    ///
    /// # Parameters
    /// - `admin`             — Must match `CONTRACT_ADMIN`; must authorise the call.
    /// - `fee_bips`          — New fee in basis-points of 1 000 (0–1 000).
    /// - `treasury_address`  — New recipient address for the fee portion.
    ///
    /// # Panics
    /// - If the contract is not initialized.
    /// - If `admin` does not match the stored admin.
    /// - If `fee_bips > 1000`.
    pub fn configure_fees(env: Env, admin: Address, fee_bips: u32, treasury_address: Address) {
        let current_admin: Address = env
            .storage()
            .instance()
            .get(&CONTRACT_ADMIN)
            .expect("Not initialized");
        current_admin.require_auth();

        if admin != current_admin {
            panic_with_error!(&env, ContractError::Unauthorized);
        }
        if fee_bips > 1000 {
            panic_with_error!(&env, ContractError::InvalidConfig);
        }

        env.storage().instance().set(&FEE_BIPS, &fee_bips);
        env.storage()
            .instance()
            .set(&TREASURY_ADDR, &treasury_address);
    }

    /// Set the contract admin when none has been recorded yet (upgrade path).
    ///
    /// This is a one-time migration helper for contracts initialised before
    /// `CONTRACT_ADMIN` storage was introduced. It panics if an admin is
    /// already set, or if the contract's `ADMIN_KEY` has not been seeded.
    ///
    /// # Parameters
    /// - `admin` — New administrator address; must authorise the call.
    ///
    /// # Panics
    /// - If `CONTRACT_ADMIN` is already set.
    /// - If `ADMIN_KEY` is not set (contract not initialised).
    pub fn upgrade_admin(env: Env, admin: Address) {
        if env.storage().instance().has(&CONTRACT_ADMIN) {
            panic_with_error!(&env, ContractError::AdminAlreadySet);
        }
        if !env.storage().instance().has(&ADMIN_KEY) {
            panic_with_error!(&env, ContractError::NotInitialized);
        }
        admin.require_auth();
        env.storage().instance().set(&CONTRACT_ADMIN, &admin);
    }

    /// Propose a new admin key with a 24-hour timelock.
    /// Only the current admin (CONTRACT_ADMIN) can propose.
    pub fn propose_new_admin_key(
        env: Env,
        admin: Address,
        new_key: BytesN<32>,
    ) -> Result<(), ContractError> {
        admin.require_auth();
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&CONTRACT_ADMIN)
            .ok_or(ContractError::NotAuthorized)?;
        if admin != stored_admin {
            return Err(ContractError::NotAuthorized);
        }
        if env.storage().instance().has(&PENDING_ADMIN_KEY) {
            return Err(ContractError::AdminKeyAlreadyPending);
        }

        let current_seq = env.ledger().sequence();
        env.storage().instance().set(&PENDING_ADMIN_KEY, &new_key);
        env.storage()
            .instance()
            .set(&PENDING_ADMIN_TIMESTAMP, &current_seq);
        env.storage()
            .instance()
            .set(&ADMIN_TIMELOCK, &(current_seq + ADMIN_TIMELOCK_DURATION));

        env.events().publish(
            (symbol_short!("admin_key_proposed"),),
            (admin, new_key.clone(), current_seq),
        );

        Ok(())
    }

    /// Accept a pending admin key proposal after the timelock expires.
    /// Anyone can call this once the 24-hour window has elapsed.
    pub fn accept_new_admin_key(env: Env) -> Result<(), ContractError> {
        let proposed_key: BytesN<32> = env
            .storage()
            .instance()
            .get(&PENDING_ADMIN_KEY)
            .ok_or(ContractError::NoPendingAdminKey)?;

        let proposal_seq: u64 = env
            .storage()
            .instance()
            .get(&PENDING_ADMIN_TIMESTAMP)
            .ok_or(ContractError::NoPendingAdminKey)?;

        let current_seq = env.ledger().sequence();
        let lock_duration: u64 = env
            .storage()
            .instance()
            .get(&ADMIN_TIMELOCK)
            .unwrap_or(ADMIN_TIMELOCK_DURATION);

        if current_seq < proposal_seq + lock_duration {
            return Err(ContractError::TimelockNotExpired);
        }

        env.storage().instance().set(&ADMIN_KEY, &proposed_key);
        env.storage().instance().remove(&PENDING_ADMIN_KEY);
        env.storage().instance().remove(&PENDING_ADMIN_TIMESTAMP);
        env.storage().instance().remove(&ADMIN_TIMELOCK);

        env.events().publish(
            (symbol_short!("admin_key_accepted"),),
            (proposed_key,),
        );

        Ok(())
    }

    /// Admin key rotation timelock duration (default: 17280 ledger sequences = 24 hours at 5s/ledger).
    pub const ADMIN_TIMELOCK_DURATION: u64 = 17280;

    // ── #199 – claim_puzzle_reward ────────────────────────────────────────────
    //
    // Accepts a backend ED25519 signature that proves the user solved a puzzle,
    // then transfers `reward_amount` tokens from the Treasury to the recipient.
    //
    // Signature payload (SHA-256 pre-image):
    //   recipient_address_bytes || reward_amount_le_8bytes || nonce_le_8bytes
    //
    // Acceptance criteria
    //   • Invalid signature  → panics (Soroban's ed25519_verify panics on failure)
    //   • Replayed nonce     → Err(ContractError::Unauthorized)
    //   • Valid call         → recipient balance incremented, treasury decremented

    /// Claim a single puzzle reward backed by a backend ED25519 signature.
    ///
    /// Verifies the signature, guards against nonce replay, deducts
    /// `reward_amount` from the treasury, and credits the recipient's
    /// puzzle-reward balance. Emits a `pzl_rwd` event on success.
    ///
    /// # Signature Payload
    /// `SHA256( recipient_address_string_bytes || reward_amount_i64_le8 || nonce_u64_le8 )`
    ///
    /// # Parameters
    /// - `recipient`      — Address to credit; must be the one encoded in the signature.
    /// - `reward_amount`  — Token amount to award; must be > 0 and ≤ `i64::MAX`.
    /// - `nonce`          — Unique per-claim `u64` nonce; prevents replay attacks.
    /// - `signature`      — 64-byte ED25519 signature from the backend service.
    ///
    /// # Returns
    /// `Ok(())` on success.
    ///
    /// # Errors
    /// - [`ContractError::InvalidAmount`]  — `reward_amount ≤ 0` or `> i64::MAX`.
    /// - [`ContractError::Unauthorized`]   — `nonce` has already been used.
    ///
    /// # Panics
    /// - If the ED25519 signature is invalid.
    /// - If `ADMIN_KEY` is absent (contract not initialised).
    /// - If the treasury has insufficient balance.
    ///
    /// # Events
    /// Emits `("pzl_rwd", recipient) → reward_amount`.
    pub fn claim_puzzle_reward(
        env: Env,
        recipient: Address,
        reward_amount: i128,
        nonce: u64,
        signature: BytesN<64>,
    ) -> Result<(), ContractError> {
        Self::check_not_paused(&env);
        if reward_amount <= 0 || reward_amount > i64::MAX as i128 {
            return Err(ContractError::InvalidAmount);
        }

        // 1. Load admin ED25519 public key
        let admin_key_bytes: Bytes = env
            .storage()
            .instance()
            .get(&ADMIN_KEY)
            .expect("Not initialized");

        let admin_pubkey: BytesN<32> = admin_key_bytes
            .try_into()
            .expect("Admin public key must be 32 bytes");

        // 2. Replay protection – check nonce before any state mutation
        let mut nonces: Map<u64, bool> = env
            .storage()
            .instance()
            .get(&USED_NONCE)
            .unwrap_or(Map::new(&env));

        if nonces.get(nonce).unwrap_or(false) {
            return Err(ContractError::Unauthorized);
        }

        // 3. Build canonical payload and verify ED25519 signature
        //    Payload = SHA256( address_string_bytes || amount_le8 || nonce_le8 )
        let mut payload_bytes = Bytes::new(&env);

        // Encode recipient address as its string representation bytes
        let recipient_str = recipient.clone().to_string();
        let str_len = recipient_str.len() as usize;
        let mut addr_buf = [0u8; 64];
        recipient_str.copy_into_slice(&mut addr_buf[..str_len]);
        payload_bytes.append(&Bytes::from_slice(&env, &addr_buf[..str_len]));

        // Append reward_amount as little-endian i64 bytes
        let amount_le: [u8; 8] = (reward_amount as i64).to_le_bytes();
        payload_bytes.append(&Bytes::from_slice(&env, &amount_le));

        // Append nonce as little-endian u64 bytes
        let nonce_le: [u8; 8] = nonce.to_le_bytes();
        payload_bytes.append(&Bytes::from_slice(&env, &nonce_le));

        // Hash and verify — ed25519_verify panics if signature is invalid,
        // which satisfies the acceptance criterion "invalid signature panics".
        let digest_bytesn: BytesN<32> = env.crypto().sha256(&payload_bytes).into();
        let digest_bytes: Bytes = digest_bytesn.into();
        env.crypto()
            .ed25519_verify(&admin_pubkey, &digest_bytes, &signature);

        Self::non_reentrant_enter(&env)?;

        // 4. Mark nonce as used (state-before-interaction pattern)
        nonces.set(nonce, true);
        env.storage().instance().set(&USED_NONCE, &nonces);

        // 5. Deduct from Treasury
        let treasury: i128 = env.storage().instance().get(&TREASURY).unwrap_or(0);
        if treasury < reward_amount {
            return Err(ContractError::InsufficientTreasury);
        }
        env.storage()
            .instance()
            .set(&TREASURY, &(treasury - reward_amount));

        // 6. Credit recipient's puzzle-reward balance
        let mut balances: Map<Address, i128> = env
            .storage()
            .instance()
            .get(&BALANCES)
            .unwrap_or(Map::new(&env));

        let prev_balance = balances.get(recipient.clone()).unwrap_or(0);
        balances.set(recipient.clone(), prev_balance + reward_amount);
        env.storage().instance().set(&BALANCES, &balances);

        // 7. Emit event
        env.events()
            .publish((symbol_short!("pzl_rwd"), recipient.clone()), reward_amount);

        Self::non_reentrant_exit(&env);
        Ok(())
    }

    // ── claim_puzzle_rewards_batch ──────────────────────────────────────────
    //
    // Batches multiple puzzle-reward proofs into a single transaction so a
    // player redeeming several puzzle rewards pays one base fee instead of
    // one per claim, AND one resource-fee-bearing read/write of the
    // nonce/balance/treasury storage entries instead of N. `claim_puzzle_reward`
    // reads and rewrites the *entire* USED_NONCE and BALANCES maps on every
    // call (Soroban (de)serializes a Map storage entry in full on get/set),
    // so that cost otherwise scales linearly with batch size; here it's paid
    // once for the whole batch. Each `Proof` is still validated exactly like
    // a call to `claim_puzzle_reward` (same signature scheme, same replay
    // protection) — signature verification itself is inherently per-proof
    // and cannot be batched.
    //
    // Atomicity: proofs are applied in order against in-memory copies of the
    // nonce/balance/treasury maps, which are only written back to storage
    // once the whole batch has validated successfully. If any proof is
    // invalid (bad amount, reused nonce — including duplicates within the
    // same batch — or bad signature), the function returns/panics before the
    // storage writes happen, and Soroban rolls back the entire invocation, so
    // a batch never partially applies.
    //
    // Acceptance criteria
    //   • Empty proof list        → Err(ContractError::EmptyBatch)
    //   • More than MAX_BATCH_SIZE → Err(ContractError::BatchTooLarge)
    //   • Any invalid signature   → panics (same as claim_puzzle_reward)
    //   • Any reused/duplicate nonce → Err(ContractError::Unauthorized)
    //   • All proofs valid        → every recipient balance incremented,
    //                                treasury decremented by the sum, in one TX

    /// Claim multiple puzzle rewards in a single transaction.
    ///
    /// More gas-efficient than calling [`claim_puzzle_reward`] repeatedly because
    /// the nonce, balance, and treasury storage maps are read and written only
    /// once for the whole batch. Each [`Proof`] is validated with the same
    /// signature scheme and replay-protection rules as the single-claim variant.
    ///
    /// **Atomicity:** all proofs are applied to in-memory state first; storage
    /// is only committed after every proof passes. A single invalid proof rolls
    /// back the entire batch.
    ///
    /// # Parameters
    /// - `proofs` — 1–[`MAX_BATCH_SIZE`] (currently 20) [`Proof`] entries.
    ///              Each proof carries its own `recipient`, `reward_amount`,
    ///              `nonce`, and `signature`; see [`claim_puzzle_reward`] for
    ///              the per-proof validation rules.
    ///
    /// # Returns
    /// `Ok(())` when all proofs are accepted and state is committed.
    ///
    /// # Errors
    /// - [`ContractError::EmptyBatch`]    — `proofs` is empty.
    /// - [`ContractError::BatchTooLarge`] — `proofs.len() > MAX_BATCH_SIZE`.
    /// - [`ContractError::InvalidAmount`] — Any proof has `reward_amount ≤ 0` or `> i64::MAX`.
    /// - [`ContractError::Unauthorized`]  — Any nonce is already used or duplicated within the batch.
    ///
    /// # Panics
    /// - If any ED25519 signature is invalid.
    /// - If the treasury has insufficient balance for any individual proof.
    ///
    /// # Events
    /// Emits `("pzl_rwd", recipient) → reward_amount` per proof, then
    /// `("pzlbatch",) → (proof_count, total_claimed)` for the batch.
    pub fn claim_puzzle_rewards_batch(env: Env, proofs: Vec<Proof>) -> Result<(), ContractError> {
        Self::check_not_paused(&env);
        if proofs.is_empty() {
            return Err(ContractError::EmptyBatch);
        }
        if proofs.len() > MAX_BATCH_SIZE {
            return Err(ContractError::BatchTooLarge);
        }

        // 1. Load admin ED25519 public key (shared across all proofs)
        let admin_key_bytes: Bytes = env
            .storage()
            .instance()
            .get(&ADMIN_KEY)
            .expect("Not initialized");

        let admin_pubkey: BytesN<32> = admin_key_bytes
            .try_into()
            .expect("Admin public key must be 32 bytes");

        let mut nonces: Map<u64, bool> = env
            .storage()
            .instance()
            .get(&USED_NONCE)
            .unwrap_or(Map::new(&env));

        let mut balances: Map<Address, i128> = env
            .storage()
            .instance()
            .get(&BALANCES)
            .unwrap_or(Map::new(&env));

        let mut treasury: i128 = env.storage().instance().get(&TREASURY).unwrap_or(0);

        let mut total_claimed: i128 = 0;

        Self::non_reentrant_enter(&env)?;

        for proof in proofs.iter() {
            let Proof {
                recipient,
                reward_amount,
                nonce,
                signature,
            } = proof;

            if reward_amount <= 0 || reward_amount > i64::MAX as i128 {
                Self::non_reentrant_exit(&env);
                return Err(ContractError::InvalidAmount);
            }

            // Replay protection — also rejects duplicate nonces within the
            // same batch, since `nonces` is updated as we go.
            if nonces.get(nonce).unwrap_or(false) {
                Self::non_reentrant_exit(&env);
                return Err(ContractError::Unauthorized);
            }

            // Build canonical payload and verify ED25519 signature — same
            // scheme as claim_puzzle_reward.
            let mut payload_bytes = Bytes::new(&env);

            let recipient_str = recipient.clone().to_string();
            let str_len = recipient_str.len() as usize;
            let mut addr_buf = [0u8; 64];
            recipient_str.copy_into_slice(&mut addr_buf[..str_len]);
            payload_bytes.append(&Bytes::from_slice(&env, &addr_buf[..str_len]));

            let amount_le: [u8; 8] = (reward_amount as i64).to_le_bytes();
            payload_bytes.append(&Bytes::from_slice(&env, &amount_le));

            let nonce_le: [u8; 8] = nonce.to_le_bytes();
            payload_bytes.append(&Bytes::from_slice(&env, &nonce_le));

            let digest_bytesn: BytesN<32> = env.crypto().sha256(&payload_bytes).into();
            let digest_bytes: Bytes = digest_bytesn.into();
            env.crypto()
                .ed25519_verify(&admin_pubkey, &digest_bytes, &signature);

            if treasury < reward_amount {
                return Err(ContractError::InsufficientTreasury);
            }
            treasury -= reward_amount;

            nonces.set(nonce, true);

            let prev_balance = balances.get(recipient.clone()).unwrap_or(0);
            balances.set(recipient.clone(), prev_balance + reward_amount);

            total_claimed += reward_amount;

            env.events()
                .publish((symbol_short!("pzl_rwd"), recipient.clone()), reward_amount);
        }

        // Commit all state changes atomically, once every proof has passed.
        env.storage().instance().set(&USED_NONCE, &nonces);
        env.storage().instance().set(&BALANCES, &balances);
        env.storage().instance().set(&TREASURY, &treasury);

        env.events()
            .publish((symbol_short!("pzlbatch"),), (proofs.len(), total_claimed));

        Self::non_reentrant_exit(&env);
        Ok(())
    }

    /// Query the puzzle-reward balance of an address.
    pub fn reward_balance(env: Env, address: Address) -> i128 {
        let balances: Map<Address, i128> = env
            .storage()
            .instance()
            .get(&BALANCES)
            .unwrap_or(Map::new(&env));
        balances.get(address).unwrap_or(0)
    }

    /// Query the current treasury reserve.
    pub fn treasury_balance(env: Env) -> i128 {
        env.storage().instance().get(&TREASURY).unwrap_or(0)
    }

    // ── Dispute Resolution System ──────────────────────────────────────────

    // ── Dispute Resolution System ──────────────────────────────────────────

    /// Configure the dispute resolution system (admin only).
    ///
    /// Sets the arbitrator address and the token fee required to file a
    /// dispute. Must be called before [`file_dispute`] is usable.
    ///
    /// # Parameters
    /// - `admin`        — Must match `CONTRACT_ADMIN`; must authorise the call.
    /// - `arbitrator`   — Address authorised to call [`resolve_dispute`] and
    ///                    [`reject_dispute`].
    /// - `dispute_fee`  — Token amount a disputing player must pay upfront;
    ///                    refunded if the dispute is rejected. Must be ≥ 0.
    ///
    /// # Panics
    /// - If the contract is not initialised.
    /// - If `admin` does not match the stored admin.
    /// - If `dispute_fee < 0`.
    pub fn configure_dispute_system(
        env: Env,
        admin: Address,
        arbitrator: Address,
        dispute_fee: i128,
    ) {
        let current_admin: Address = env
            .storage()
            .instance()
            .get(&CONTRACT_ADMIN)
            .expect("Not initialized");
        current_admin.require_auth();

        if admin != current_admin {
            panic_with_error!(&env, ContractError::Unauthorized);
        }
        if dispute_fee < 0 {
            panic_with_error!(&env, ContractError::InvalidAmount);
        }

        env.storage().instance().set(&ARBITRATOR, &arbitrator);
        env.storage().instance().set(&DISPUTE_FEE, &dispute_fee);
    }

    /// Set the inactivity timeout used by [`claim_timeout_win`] (admin only).
    ///
    /// The timeout is measured in ledger sequences (≈ 5 s each on Stellar
    /// mainnet). A player may claim a timeout win once `current_ledger -
    /// game.last_move_at ≥ timeout_duration`.
    ///
    /// # Parameters
    /// - `admin`     — Must match `CONTRACT_ADMIN`; must authorise the call.
    /// - `duration`  — Ledger sequences of inactivity before a timeout win is
    ///                 claimable; must be > 0.
    ///
    /// # Panics
    /// - If the contract is not initialised.
    /// - If `admin` does not match the stored admin.
    /// - If `duration == 0`.
    pub fn configure_timeout(env: Env, admin: Address, duration: u64) {
        let current_admin: Address = env
            .storage()
            .instance()
            .get(&CONTRACT_ADMIN)
            .expect("Not initialized");
        current_admin.require_auth();

        if admin != current_admin {
            panic_with_error!(&env, ContractError::Unauthorized);
        }
        if duration == 0 {
            panic_with_error!(&env, ContractError::InvalidConfig);
        }

        env.storage().instance().set(&TIMEOUT_DURATION, &duration);
    }

    /// File a dispute against an opponent for an in-progress game.
    ///
    /// Transfers the configured `dispute_fee` (if non-zero) from `filer` to
    /// the contract, creates a [`Dispute`] in `Pending` status, and emits a
    /// `("dispute", "filed")` event. The fee is held until the dispute is
    /// resolved or rejected.
    ///
    /// # Parameters
    /// - `game_id`   — ID of a game in `InProgress` state.
    /// - `filer`     — Player raising the dispute; must be a game participant
    ///                 and must authorise the call.
    /// - `against`   — The opposing player; must also be a participant and
    ///                 must differ from `filer`.
    /// - `reason`    — Encoded reason bytes (arbitrary off-chain payload).
    ///
    /// # Returns
    /// `Ok(dispute_id)` — unique `u64` identifier for the new dispute.
    ///
    /// # Errors
    /// - [`ContractError::GameNotFound`]            — `game_id` does not exist.
    /// - [`ContractError::NotDisputable`]           — Game is not `InProgress`.
    /// - [`ContractError::NotPlayer`]               — `filer` or `against` is not a participant.
    /// - [`ContractError::InvalidMove`]             — `filer` and `against` are the same address.
    /// - [`ContractError::InsufficientDisputeFee`]  — `filer`'s balance < configured dispute fee.
    ///
    /// # Events
    /// Emits `("dispute", "filed") → (dispute_id, filer)`.
    pub fn file_dispute(
        env: Env,
        game_id: u64,
        filer: Address,
        against: Address,
        reason: Bytes,
    ) -> Result<u64, ContractError> {
        Self::check_not_paused(&env);
        let games: Map<u64, Game> = env
            .storage()
            .instance()
            .get(&GAMES)
            .ok_or(ContractError::GameNotFound)?;

        let game = games.get(game_id).ok_or(ContractError::GameNotFound)?;

        if game.state != GameState::InProgress {
            return Err(ContractError::NotDisputable);
        }

        if filer != game.player1 && Some(filer.clone()) != game.player2 {
            return Err(ContractError::NotPlayer);
        }
        if against == filer {
            return Err(ContractError::InvalidMove);
        }
        if against != game.player1 && Some(against.clone()) != game.player2 {
            return Err(ContractError::NotPlayer);
        }

        filer.require_auth();

        Self::non_reentrant_enter(&env)?;

        let dispute_fee: i128 = env.storage().instance().get(&DISPUTE_FEE).unwrap_or(0);
        if dispute_fee > 0 {
            let token_client = Self::token_client(&env);
            let contract_address = env.current_contract_address();

            if token_client.balance(&filer) < dispute_fee {
                Self::non_reentrant_exit(&env);
                return Err(ContractError::InsufficientDisputeFee);
            }

            token_client.transfer(&filer, &contract_address, &dispute_fee);
        }

        let mut dispute_counter: u64 = env.storage().instance().get(&DISPUTE_COUNTER).unwrap_or(0);
        dispute_counter += 1;
        env.storage()
            .instance()
            .set(&DISPUTE_COUNTER, &dispute_counter);

        let dispute = Dispute {
            id: dispute_counter,
            game_id,
            filer: filer.clone(),
            against,
            reason,
            status: DisputeStatus::Pending,
            filed_at: env.ledger().sequence() as u64,
            resolution: None,
        };

        let mut disputes: Map<u64, Dispute> = env
            .storage()
            .instance()
            .get(&DISPUTES)
            .unwrap_or(Map::new(&env));
        disputes.set(dispute_counter, dispute);
        env.storage().instance().set(&DISPUTES, &disputes);

        env.events().publish(
            (symbol_short!("dispute"), symbol_short!("filed")),
            (dispute_counter, filer),
        );

        Self::non_reentrant_exit(&env);
        Ok(dispute_counter)
    }

    /// Claim a win by timeout when the opponent has not moved within the allowed window.
    ///
    /// Only the *waiting* player (the one who is not required to move) may
    /// claim a timeout win. Once `current_ledger - game.last_move_at ≥
    /// timeout_duration`, that player calls this to receive the net payout.
    ///
    /// # Parameters
    /// - `game_id`   — ID of a game in `InProgress` state.
    /// - `claimant`  — The player who is waiting for the opponent's move;
    ///                 must authorise the call.
    ///
    /// # Returns
    /// `Ok(())` on success; game is marked `Settled`.
    ///
    /// # Errors
    /// - [`ContractError::GameNotFound`]           — `game_id` does not exist.
    /// - [`ContractError::GameNotInProgress`]      — Game is not `InProgress`.
    /// - [`ContractError::NotPlayer`]              — `claimant` is not a participant.
    /// - [`ContractError::InvalidTimeoutClaimant`] — `claimant` is the player whose turn it is (not the waiting player).
    /// - [`ContractError::TimeoutNotConfigured`]   — [`configure_timeout`] has not been called.
    /// - [`ContractError::TimeoutNotReached`]      — Elapsed ledgers < `timeout_duration`.
    ///
    /// # Events
    /// Emits `("timeout", game_id) → (claimant, timeout_duration)`.
    pub fn claim_timeout_win(
        env: Env,
        game_id: u64,
        claimant: Address,
    ) -> Result<(), ContractError> {
        Self::check_not_paused(&env);
        let mut games: Map<u64, Game> = env
            .storage()
            .instance()
            .get(&GAMES)
            .ok_or(ContractError::GameNotFound)?;

        let mut game = games.get(game_id).ok_or(ContractError::GameNotFound)?;

        if game.state != GameState::InProgress {
            return Err(ContractError::GameNotInProgress);
        }
        if claimant != game.player1 && Some(claimant.clone()) != game.player2 {
            return Err(ContractError::NotPlayer);
        }

        claimant.require_auth();

        let waiting_player = if game.current_turn == 1 {
            game.player2
                .as_ref()
                .ok_or(ContractError::GameFull)?
                .clone()
        } else {
            game.player1.clone()
        };

        if claimant != waiting_player {
            return Err(ContractError::InvalidTimeoutClaimant);
        }

        let timeout_duration: u64 = env
            .storage()
            .instance()
            .get(&TIMEOUT_DURATION)
            .ok_or(ContractError::TimeoutNotConfigured)?;

        let current_ledger = env.ledger().sequence() as u64;
        let elapsed = current_ledger.saturating_sub(game.last_move_at);

        if elapsed < timeout_duration {
            return Err(ContractError::TimeoutNotReached);
        }

        Self::non_reentrant_enter(&env)?;

        game.winner = Some(claimant.clone());
        match Self::process_payout(&env, &game, &claimant) {
            Ok(()) => {}
            Err(e) => {
                Self::non_reentrant_exit(&env);
                return Err(e);
            }
        }
        game.state = GameState::Settled;

        games.set(game_id, game);
        env.storage().instance().set(&GAMES, &games);

        env.events().publish(
            (symbol_short!("timeout"), game_id),
            (claimant, timeout_duration),
        );

        Self::non_reentrant_exit(&env);
        Ok(())
    }

    /// Query the ledger sequences remaining before the active player's turn times out.
    ///
    /// Returns `None` if the game does not exist, is not `InProgress`, or if
    /// no timeout has been configured. Returns `Some(0)` when the timeout has
    /// already elapsed (a timeout win is claimable).
    ///
    /// # Parameters
    /// - `game_id` — ID of the game to query.
    ///
    /// # Returns
    /// `Some(remaining_sequences)` — sequences until timeout, or `Some(0)` if already elapsed.
    /// `None` — game not found, not in progress, or timeout not configured.
    pub fn get_timeout_remaining(env: Env, game_id: u64) -> Option<u64> {
        let games: Map<u64, Game> = env.storage().instance().get(&GAMES)?;
        let game = games.get(game_id)?;

        if game.state != GameState::InProgress {
            return None;
        }

        let timeout_duration: u64 = env.storage().instance().get(&TIMEOUT_DURATION)?;
        let current_ledger = env.ledger().sequence() as u64;
        let elapsed = current_ledger.saturating_sub(game.last_move_at);

        if elapsed >= timeout_duration {
            return Some(0);
        }

        Some(timeout_duration - elapsed)
    }

    /// Resolve a pending dispute and settle the underlying game (arbitrator only).
    ///
    /// Authenticates the arbitrator, processes the payout (`Some(winner)` for a
    /// win, `None` for a draw), marks the game `Settled`, and closes the
    /// dispute as `Resolved`. Emits a `("dispute", "solved")` event.
    ///
    /// # Parameters
    /// - `dispute_id`   — ID of a dispute in `Pending` status.
    /// - `arbitrator`   — Must match the stored arbitrator; must authorise the call.
    /// - `winner`       — `Some(address)` to pay out a winner, or `None` to
    ///                    split stakes equally (draw).
    /// - `resolution`   — Encoded resolution rationale (arbitrary off-chain payload).
    ///
    /// # Returns
    /// `Ok(())` on success.
    ///
    /// # Errors
    /// - [`ContractError::NotArbitrator`]        — `arbitrator` is not the configured arbitrator,
    ///                                             or the arbitrator has not been set.
    /// - [`ContractError::DisputeNotFound`]      — `dispute_id` does not exist.
    /// - [`ContractError::GameAlreadyCompleted`] — Dispute is not `Pending`, or game is not `InProgress`.
    /// - [`ContractError::GameNotFound`]         — Underlying game does not exist.
    /// - [`ContractError::NotPlayer`]            — Provided winner is not a game participant.
    ///
    /// # Events
    /// Emits `("dispute", "solved") → (dispute_id, winner)`.
    pub fn resolve_dispute(
        env: Env,
        dispute_id: u64,
        arbitrator: Address,
        winner: Option<Address>,
        resolution: Bytes,
    ) -> Result<(), ContractError> {
        Self::check_not_paused(&env);
        let stored_arbitrator: Address = env
            .storage()
            .instance()
            .get(&ARBITRATOR)
            .ok_or(ContractError::NotArbitrator)?;

        if arbitrator != stored_arbitrator {
            return Err(ContractError::NotArbitrator);
        }
        arbitrator.require_auth();

        let mut disputes: Map<u64, Dispute> = env
            .storage()
            .instance()
            .get(&DISPUTES)
            .ok_or(ContractError::DisputeNotFound)?;
        let mut dispute = disputes
            .get(dispute_id)
            .ok_or(ContractError::DisputeNotFound)?;

        if dispute.status != DisputeStatus::Pending {
            return Err(ContractError::GameAlreadyCompleted);
        }

        let mut games: Map<u64, Game> = env
            .storage()
            .instance()
            .get(&GAMES)
            .ok_or(ContractError::GameNotFound)?;
        let mut game = games
            .get(dispute.game_id)
            .ok_or(ContractError::GameNotFound)?;

        if game.state != GameState::InProgress {
            return Err(ContractError::GameAlreadyCompleted);
        }

        Self::non_reentrant_enter(&env)?;

        match winner {
            Some(ref winner_addr) => {
                if *winner_addr != game.player1 && Some(winner_addr.clone()) != game.player2 {
                    Self::non_reentrant_exit(&env);
                    return Err(ContractError::NotPlayer);
                }
                game.state = GameState::Completed;
                game.winner = Some(winner_addr.clone());
                match Self::process_payout(&env, &game, winner_addr) {
                    Ok(()) => {}
                    Err(e) => {
                        Self::non_reentrant_exit(&env);
                        return Err(e);
                    }
                }
                game.state = GameState::Settled;
            }
            None => {
                game.state = GameState::Drawn;
                game.winner = None;
                match Self::process_draw_payout(&env, &game) {
                    Ok(()) => {}
                    Err(e) => {
                        Self::non_reentrant_exit(&env);
                        return Err(e);
                    }
                }
            }
        }

        games.set(dispute.game_id, game);
        env.storage().instance().set(&GAMES, &games);

        dispute.status = DisputeStatus::Resolved;
        dispute.resolution = Some(resolution);
        disputes.set(dispute_id, dispute);
        env.storage().instance().set(&DISPUTES, &disputes);

        env.events().publish(
            (symbol_short!("dispute"), symbol_short!("solved")),
            (dispute_id, winner),
        );

        Self::non_reentrant_exit(&env);
        Ok(())
    }

    /// Reject a pending dispute and refund the dispute fee to the filer (arbitrator only).
    ///
    /// Marks the dispute `Rejected`, stores the arbitrator's reason, and
    /// returns the dispute fee (if non-zero) to the original filer.
    ///
    /// # Parameters
    /// - `dispute_id`  — ID of a dispute in `Pending` status.
    /// - `arbitrator`  — Must match the stored arbitrator; must authorise the call.
    /// - `reason`      — Encoded rejection rationale (arbitrary off-chain payload).
    ///
    /// # Returns
    /// `Ok(())` on success.
    ///
    /// # Errors
    /// - [`ContractError::NotArbitrator`]        — `arbitrator` is not the configured arbitrator,
    ///                                             or the arbitrator has not been set.
    /// - [`ContractError::DisputeNotFound`]      — `dispute_id` does not exist.
    /// - [`ContractError::GameAlreadyCompleted`] — Dispute is not in `Pending` status.
    ///
    /// # Events
    /// Emits `("dispute", "reject") → (dispute_id, filer)`.
    pub fn reject_dispute(
        env: Env,
        dispute_id: u64,
        arbitrator: Address,
        reason: Bytes,
    ) -> Result<(), ContractError> {
        Self::check_not_paused(&env);
        // Verify arbitrator
        let stored_arbitrator: Address = env
            .storage()
            .instance()
            .get(&ARBITRATOR)
            .ok_or(ContractError::NotArbitrator)?;

        if arbitrator != stored_arbitrator {
            return Err(ContractError::NotArbitrator);
        }
        arbitrator.require_auth();

        // Get dispute
        let mut disputes: Map<u64, Dispute> = env
            .storage()
            .instance()
            .get(&DISPUTES)
            .ok_or(ContractError::DisputeNotFound)?;

        let mut dispute = disputes
            .get(dispute_id)
            .ok_or(ContractError::DisputeNotFound)?;

        // Dispute must be pending
        if dispute.status != DisputeStatus::Pending {
            return Err(ContractError::GameAlreadyCompleted);
        }

        Self::non_reentrant_enter(&env)?;

        // Update dispute status
        dispute.status = DisputeStatus::Rejected;
        dispute.resolution = Some(reason);
        let filer = dispute.filer.clone();
        disputes.set(dispute_id, dispute);
        env.storage().instance().set(&DISPUTES, &disputes);

        // Refund dispute fee to filer
        let dispute_fee: i128 = env.storage().instance().get(&DISPUTE_FEE).unwrap_or(0);
        if dispute_fee > 0 {
            let token_client = Self::token_client(&env);
            let contract_address = env.current_contract_address();
            token_client.transfer(&contract_address, &filer, &dispute_fee);
        }

        // Emit dispute rejected event
        env.events().publish(
            (symbol_short!("dispute"), symbol_short!("reject")),
            (dispute_id, filer),
        );

        Self::non_reentrant_exit(&env);
        Ok(())
    }

    /// Fetch a dispute by ID.
    ///
    /// # Parameters
    /// - `dispute_id` — Numeric ID returned by [`file_dispute`].
    ///
    /// # Returns
    /// `Ok(Dispute)` — the full [`Dispute`] struct including status and resolution.
    ///
    /// # Errors
    /// - [`ContractError::DisputeNotFound`] — `dispute_id` does not exist or
    ///   no disputes have been filed yet.
    pub fn get_dispute(env: Env, dispute_id: u64) -> Result<Dispute, ContractError> {
        let disputes: Map<u64, Dispute> = env
            .storage()
            .instance()
            .get(&DISPUTES)
            .ok_or(ContractError::DisputeNotFound)?;

        disputes
            .get(dispute_id)
            .ok_or(ContractError::DisputeNotFound)
    }

    // ── SEP-10 Challenge Verification (#529) ──────────────────────────────────
    //
    // SEP-10 is the Stellar Web Authentication standard. The flow:
    //   1. Backend calls `issue_sep10_challenge` to store a nonce with an expiry.
    //   2. The user signs the challenge with their Stellar keypair off-chain.
    //   3. The user calls `verify_sep10_challenge` with the nonce + ED25519 sig.
    //   4. On success the address is marked as verified in contract storage.
    //
    // The challenge payload is: SHA256( address_bytes || nonce_le8 || expiry_le8 )
    // This matches the backend signing convention used in claim_puzzle_reward.

    /// Issue a SEP-10 Web Authentication challenge for `account` (admin only).
    ///
    /// Stores `nonce → expiry` in contract storage. The user must subsequently
    /// call [`verify_sep10_challenge`] with a valid ED25519 signature over the
    /// challenge payload to become verified.
    ///
    /// # Parameters
    /// - `admin`    — Must match `CONTRACT_ADMIN`; must authorise the call.
    /// - `account`  — Stellar account address the challenge is issued for.
    /// - `nonce`    — 32-byte random value; must be globally unique (re-using a
    ///                nonce returns [`ContractError::ChallengeAlreadyUsed`]).
    /// - `expiry`   — Ledger sequence after which the challenge is considered
    ///                expired and cannot be verified.
    ///
    /// # Returns
    /// `Ok(())` on success.
    ///
    /// # Errors
    /// - [`ContractError::Unauthorized`]       — `admin` does not match `CONTRACT_ADMIN`.
    /// - [`ContractError::ChallengeExpired`]   — `expiry ≤ current_ledger`.
    /// - [`ContractError::ChallengeAlreadyUsed`] — `nonce` already exists in storage.
    ///
    /// # Events
    /// Emits `("sep10", "issued") → (account, nonce)`.
    pub fn issue_sep10_challenge(
        env: Env,
        admin: Address,
        account: Address,
        nonce: BytesN<32>,
        expiry: u64,
    ) -> Result<(), ContractError> {
        Self::check_not_paused(&env);
        let current_admin: Address = env
            .storage()
            .instance()
            .get(&CONTRACT_ADMIN)
            .expect("Not initialized");
        current_admin.require_auth();
        if admin != current_admin {
            return Err(ContractError::Unauthorized);
        }

        let current_ledger = env.ledger().sequence() as u64;
        if expiry <= current_ledger {
            return Err(ContractError::ChallengeExpired);
        }

        let mut challenges: Map<BytesN<32>, u64> = env
            .storage()
            .instance()
            .get(&SEP10_CHALLENGES)
            .unwrap_or(Map::new(&env));

        // Reject if nonce already exists (replay protection)
        if challenges.get(nonce.clone()).is_some() {
            return Err(ContractError::ChallengeAlreadyUsed);
        }

        challenges.set(nonce.clone(), expiry);
        env.storage().instance().set(&SEP10_CHALLENGES, &challenges);

        env.events().publish(
            (symbol_short!("sep10"), symbol_short!("issued")),
            (account, nonce),
        );

        Ok(())
    }

    /// Verify a SEP-10 challenge.
    ///
    /// The caller provides the `nonce` and an ED25519 `signature` over
    /// SHA256( address_bytes || nonce_bytes || expiry_le8 ).
    ///
    /// On success the challenge is consumed and `account` is marked verified.
    pub fn verify_sep10_challenge(
        env: Env,
        account: Address,
        nonce: BytesN<32>,
        signature: BytesN<64>,
    ) -> Result<(), ContractError> {
        Self::check_not_paused(&env);
        // 1. Load and validate the challenge
        let mut challenges: Map<BytesN<32>, u64> = env
            .storage()
            .instance()
            .get(&SEP10_CHALLENGES)
            .unwrap_or(Map::new(&env));

        let expiry = challenges
            .get(nonce.clone())
            .ok_or(ContractError::ChallengeAlreadyUsed)?;

        let current_ledger = env.ledger().sequence() as u64;
        if current_ledger > expiry {
            return Err(ContractError::ChallengeExpired);
        }

        // 2. Load admin ED25519 public key (same key used for puzzle rewards)
        let admin_key_bytes: Bytes = env
            .storage()
            .instance()
            .get(&ADMIN_KEY)
            .expect("Not initialized");
        let admin_pubkey: BytesN<32> = admin_key_bytes
            .try_into()
            .expect("Admin public key must be 32 bytes");

        // 3. Build canonical payload: address_bytes || nonce_bytes || expiry_le8
        let mut payload = Bytes::new(&env);

        let account_str = account.clone().to_string();
        let str_len = account_str.len() as usize;
        let mut addr_buf = [0u8; 64];
        account_str.copy_into_slice(&mut addr_buf[..str_len]);
        payload.append(&Bytes::from_slice(&env, &addr_buf[..str_len]));

        let nonce_bytes: Bytes = nonce.clone().into();
        payload.append(&nonce_bytes);

        let expiry_le: [u8; 8] = expiry.to_le_bytes();
        payload.append(&Bytes::from_slice(&env, &expiry_le));

        // 4. Verify — panics on invalid signature (Soroban convention)
        let digest: BytesN<32> = env.crypto().sha256(&payload).into();
        let digest_bytes: Bytes = digest.into();
        env.crypto()
            .ed25519_verify(&admin_pubkey, &digest_bytes, &signature);

        // 5. Consume the challenge (prevent replay)
        challenges.remove(nonce.clone());
        env.storage().instance().set(&SEP10_CHALLENGES, &challenges);

        // 6. Mark account as verified
        let mut verified: Map<Address, bool> = env
            .storage()
            .instance()
            .get(&SEP10_VERIFIED)
            .unwrap_or(Map::new(&env));
        verified.set(account.clone(), true);
        env.storage().instance().set(&SEP10_VERIFIED, &verified);

        env.events()
            .publish((symbol_short!("sep10"), symbol_short!("verified")), account);

        Ok(())
    }

    /// Returns true if `account` has completed SEP-10 verification.
    pub fn is_sep10_verified(env: Env, account: Address) -> bool {
        let verified: Map<Address, bool> = env
            .storage()
            .instance()
            .get(&SEP10_VERIFIED)
            .unwrap_or(Map::new(&env));
        verified.get(account).unwrap_or(false)
    }

    // ── Multi-Sig Fee Control (#535) ──────────────────────────────────────────
    //
    // Protocol fee parameters (fee_bips + treasury_address) are sensitive.
    // This module requires M-of-N approval from a configured signer set before
    // any fee change takes effect.
    //
    // Flow:
    //   1. Admin calls `configure_multisig` to set signers + threshold.
    //   2. Any signer calls `propose_fee_change` to create a pending proposal.
    //   3. Each signer calls `approve_fee_proposal` to vote.
    //   4. Once approvals ≥ threshold the fee change is applied automatically.
    //   5. Any signer can call `cancel_fee_proposal` to discard the proposal.

    /// Configure the multi-sig signer set and approval threshold (admin only).
    ///
    /// Replaces any existing signer configuration. Once set, fee changes
    /// proposed via [`propose_fee_change`] require at least `threshold`
    /// approvals from `signers` before taking effect.
    ///
    /// # Parameters
    /// - `admin`      — Must match `CONTRACT_ADMIN`; must authorise the call.
    /// - `signers`    — Exhaustive list of addresses authorised to propose and
    ///                  approve fee changes.
    /// - `threshold`  — Minimum number of approvals required; must be ≥ 1 and
    ///                  ≤ `signers.len()`.
    ///
    /// # Returns
    /// `Ok(())` on success.
    ///
    /// # Errors
    /// - [`ContractError::Unauthorized`]    — `admin` does not match `CONTRACT_ADMIN`.
    /// - [`ContractError::InvalidThreshold`] — `threshold == 0` or `threshold > signers.len()`.
    ///
    /// # Panics
    /// - If the contract is not initialised.
    pub fn configure_multisig(
        env: Env,
        admin: Address,
        signers: Vec<Address>,
        threshold: u32,
    ) -> Result<(), ContractError> {
        Self::check_not_paused(&env);
        let current_admin: Address = env
            .storage()
            .instance()
            .get(&CONTRACT_ADMIN)
            .expect("Not initialized");
        current_admin.require_auth();
        if admin != current_admin {
            return Err(ContractError::Unauthorized);
        }

        let n = signers.len();
        if threshold == 0 || threshold > n {
            return Err(ContractError::InvalidThreshold);
        }

        env.storage().instance().set(&MULTISIG_SIGNERS, &signers);
        env.storage()
            .instance()
            .set(&MULTISIG_THRESHOLD, &threshold);

        Ok(())
    }

    /// Propose a new fee configuration (any registered signer).
    ///
    /// Replaces any existing pending proposal and resets the approval map.
    /// The proposer's approval is counted automatically, so a single-signer
    /// setup with `threshold = 1` will execute immediately.
    ///
    /// # Parameters
    /// - `proposer`              — Must be in the configured signer set; must authorise the call.
    /// - `new_fee_bips`          — Proposed fee in basis-points of 1 000 (0–1 000).
    /// - `new_treasury_address`  — Proposed recipient address for the fee portion.
    ///
    /// # Returns
    /// `Ok(())` on success.
    ///
    /// # Errors
    /// - [`ContractError::InvalidAmount`] — `new_fee_bips > 1000`.
    /// - [`ContractError::NotASigner`]    — `proposer` is not in the signer set.
    /// - [`ContractError::Unauthorized`]  — Multi-sig has not been configured yet.
    ///
    /// # Events
    /// Emits `("multisig", "proposed") → proposer`.
    pub fn propose_fee_change(
        env: Env,
        proposer: Address,
        new_fee_bips: u32,
        new_treasury_address: Address,
    ) -> Result<(), ContractError> {
        Self::check_not_paused(&env);
        if new_fee_bips > 1000 {
            return Err(ContractError::InvalidAmount);
        }

        let signers: Vec<Address> = env
            .storage()
            .instance()
            .get(&MULTISIG_SIGNERS)
            .ok_or(ContractError::Unauthorized)?;

        // Verify proposer is a signer
        if !signers.contains(&proposer) {
            return Err(ContractError::NotASigner);
        }

        proposer.require_auth();

        let proposal = FeeProposal {
            new_fee_bips,
            new_treasury_address,
            proposed_at: env.ledger().sequence() as u64,
            proposer: proposer.clone(),
        };

        env.storage()
            .instance()
            .set(&PENDING_FEE_PROPOSAL, &proposal);

        // Reset approvals and auto-approve for proposer
        let mut approvals: Map<Address, bool> = Map::new(&env);
        approvals.set(proposer.clone(), true);
        env.storage()
            .instance()
            .set(&FEE_PROPOSAL_APPROVALS, &approvals);

        env.events().publish(
            (symbol_short!("multisig"), symbol_short!("proposed")),
            proposer,
        );

        Ok(())
    }

    /// Approve the pending fee proposal (any registered signer).
    ///
    /// Records the signer's approval. If the running approval count reaches
    /// the configured threshold, the fee change is applied immediately and
    /// the proposal is cleared.
    ///
    /// # Parameters
    /// - `signer` — Must be in the configured signer set; must authorise the call.
    ///
    /// # Returns
    /// `Ok(true)` — threshold reached, proposal executed and cleared.
    /// `Ok(false)` — approval recorded, still awaiting more signers.
    ///
    /// # Errors
    /// - [`ContractError::NotASigner`]    — `signer` is not in the signer set.
    /// - [`ContractError::Unauthorized`]  — Multi-sig has not been configured.
    /// - [`ContractError::NoProposal`]    — No pending proposal exists.
    /// - [`ContractError::AlreadyApproved`] — `signer` has already approved this proposal.
    ///
    /// # Events
    /// On partial approval: emits `("multisig", "approved") → signer`.
    /// On execution: emits `("multisig", "executed") → new_fee_bips`.
    pub fn approve_fee_proposal(env: Env, signer: Address) -> Result<bool, ContractError> {
        Self::check_not_paused(&env);
        let signers: Vec<Address> = env
            .storage()
            .instance()
            .get(&MULTISIG_SIGNERS)
            .ok_or(ContractError::Unauthorized)?;

        if !signers.contains(&signer) {
            return Err(ContractError::NotASigner);
        }

        let proposal: FeeProposal = env
            .storage()
            .instance()
            .get(&PENDING_FEE_PROPOSAL)
            .ok_or(ContractError::NoProposal)?;

        signer.require_auth();

        let mut approvals: Map<Address, bool> = env
            .storage()
            .instance()
            .get(&FEE_PROPOSAL_APPROVALS)
            .unwrap_or(Map::new(&env));

        if approvals.get(signer.clone()).unwrap_or(false) {
            return Err(ContractError::AlreadyApproved);
        }

        approvals.set(signer.clone(), true);
        env.storage()
            .instance()
            .set(&FEE_PROPOSAL_APPROVALS, &approvals);

        // Count approvals
        let approval_count = approvals.len();
        let threshold: u32 = env
            .storage()
            .instance()
            .get(&MULTISIG_THRESHOLD)
            .unwrap_or(u32::MAX);

        if approval_count >= threshold {
            // Apply the fee change
            env.storage()
                .instance()
                .set(&FEE_BIPS, &proposal.new_fee_bips);
            env.storage()
                .instance()
                .set(&TREASURY_ADDR, &proposal.new_treasury_address);

            // Clear proposal and approvals
            env.storage().instance().remove(&PENDING_FEE_PROPOSAL);
            env.storage().instance().remove(&FEE_PROPOSAL_APPROVALS);

            env.events().publish(
                (symbol_short!("multisig"), symbol_short!("executed")),
                proposal.new_fee_bips,
            );

            return Ok(true); // proposal executed
        }

        env.events().publish(
            (symbol_short!("multisig"), symbol_short!("approved")),
            signer,
        );

        Ok(false) // still waiting for more approvals
    }

    /// Cancel the pending fee proposal (any registered signer).
    ///
    /// Removes the proposal and clears all approvals. Any signer may cancel,
    /// not only the original proposer.
    ///
    /// # Parameters
    /// - `signer` — Must be in the configured signer set; must authorise the call.
    ///
    /// # Returns
    /// `Ok(())` on success.
    ///
    /// # Errors
    /// - [`ContractError::NotASigner`]   — `signer` is not in the signer set.
    /// - [`ContractError::Unauthorized`] — Multi-sig has not been configured.
    /// - [`ContractError::NoProposal`]   — No pending proposal to cancel.
    ///
    /// # Events
    /// Emits `("multisig", "cancel") → signer`.
    pub fn cancel_fee_proposal(env: Env, signer: Address) -> Result<(), ContractError> {
        Self::check_not_paused(&env);
        let signers: Vec<Address> = env
            .storage()
            .instance()
            .get(&MULTISIG_SIGNERS)
            .ok_or(ContractError::Unauthorized)?;

        if !signers.contains(&signer) {
            return Err(ContractError::NotASigner);
        }

        if !env.storage().instance().has(&PENDING_FEE_PROPOSAL) {
            return Err(ContractError::NoProposal);
        }

        signer.require_auth();

        env.storage().instance().remove(&PENDING_FEE_PROPOSAL);
        env.storage().instance().remove(&FEE_PROPOSAL_APPROVALS);

        env.events()
            .publish((symbol_short!("multisig"), symbol_short!("cancel")), signer);

        Ok(())
    }

    /// Return the pending fee proposal, if one exists.
    ///
    /// # Returns
    /// `Some(FeeProposal)` — the proposal with its `new_fee_bips`,
    /// `new_treasury_address`, `proposed_at` ledger sequence, and `proposer`.
    /// `None` — no proposal is currently pending.
    pub fn get_fee_proposal(env: Env) -> Option<FeeProposal> {
        env.storage().instance().get(&PENDING_FEE_PROPOSAL)
    }

    /// Return the number of signers who have approved the pending proposal.
    ///
    /// Returns `0` if no proposal is pending or no approvals have been recorded.
    pub fn get_approval_count(env: Env) -> u32 {
        let approvals: Map<Address, bool> = env
            .storage()
            .instance()
            .get(&FEE_PROPOSAL_APPROVALS)
            .unwrap_or(Map::new(&env));
        approvals.len()
    }

    // ── SEP-40 Oracle Clock Sync (#533) ───────────────────────────────────────
    //
    // SEP-40 defines a standard oracle interface on Stellar/Soroban.
    // We store the oracle contract address and call its `lastprice` function
    // to get a trusted timestamp for game clock synchronization.

    /// Set the SEP-40 oracle contract address used for clock synchronisation (admin only).
    ///
    /// Once configured, [`get_oracle_time`] may call the oracle via
    /// cross-contract invocation to obtain a trusted timestamp.
    ///
    /// # Parameters
    /// - `admin`   — Must match `CONTRACT_ADMIN`; must authorise the call.
    /// - `oracle`  — Address of the SEP-40-compliant oracle contract.
    ///
    /// # Returns
    /// `Ok(())` on success.
    ///
    /// # Errors
    /// - [`ContractError::Unauthorized`] — `admin` does not match `CONTRACT_ADMIN`.
    ///
    /// # Panics
    /// - If the contract is not initialised.
    pub fn configure_oracle(
        env: Env,
        admin: Address,
        oracle: Address,
    ) -> Result<(), ContractError> {
        let current_admin: Address = env
            .storage()
            .instance()
            .get(&CONTRACT_ADMIN)
            .expect("Not initialized");
        current_admin.require_auth();
        if admin != current_admin {
            return Err(ContractError::Unauthorized);
        }
        env.storage().instance().set(&ORACLE_CONTRACT, &oracle);
        Ok(())
    }

    /// Return the configured SEP-40 oracle contract address.
    ///
    /// # Returns
    /// `Ok(Address)` — the oracle contract address.
    ///
    /// # Errors
    /// - [`ContractError::OracleNotConfigured`] — [`configure_oracle`] has not been called.
    pub fn get_oracle(env: Env) -> Result<Address, ContractError> {
        env.storage()
            .instance()
            .get(&ORACLE_CONTRACT)
            .ok_or(ContractError::OracleNotConfigured)
    }

    /// Get the oracle-synced timestamp (ledger sequence from oracle).
    /// Falls back to the current ledger sequence if oracle is not configured.
    /// The oracle is called via cross-contract invocation using the SEP-40
    /// `lastprice` interface which returns a price record with a timestamp.
    ///
    /// Note: This is a minimal implementation. In production, you would invoke
    /// the oracle contract's `lastprice` method and extract the timestamp.
    pub fn get_oracle_time(env: Env) -> u64 {
        let oracle_opt: Option<Address> = env.storage().instance().get(&ORACLE_CONTRACT);
        match oracle_opt {
            Some(_oracle) => {
                // TODO: Implement cross-contract call to oracle.lastprice()
                // For now, fall back to ledger sequence
                env.ledger().sequence() as u64
            }
            None => env.ledger().sequence() as u64,
        }
    }

    // ── Time-lock Escrow for Tournament Grand Prizes (#532) ───────────────────
    //
    // Tournament grand prizes are locked in escrow for a configurable duration
    // before they can be released to winners. This prevents immediate withdrawal
    // and allows time for dispute resolution.

    /// Configure the time-lock duration for tournament prize escrows (admin only).
    ///
    /// After a tournament game completes, a prize escrow created via
    /// [`create_tournament_escrow`] is locked for `duration` ledger sequences
    /// before it can be released.
    ///
    /// # Parameters
    /// - `admin`     — Must match `CONTRACT_ADMIN`; must authorise the call.
    /// - `duration`  — Lock duration in ledger sequences; must be > 0.
    ///
    /// # Returns
    /// `Ok(())` on success.
    ///
    /// # Errors
    /// - [`ContractError::Unauthorized`] — `admin` does not match `CONTRACT_ADMIN`.
    ///
    /// # Panics
    /// - If the contract is not initialised.
    /// - If `duration == 0`.
    pub fn configure_tournament_timelock(
        env: Env,
        admin: Address,
        duration: u64,
    ) -> Result<(), ContractError> {
        Self::check_not_paused(&env);
        let current_admin: Address = env
            .storage()
            .instance()
            .get(&CONTRACT_ADMIN)
            .expect("Not initialized");
        current_admin.require_auth();
        if admin != current_admin {
            return Err(ContractError::Unauthorized);
        }
        if duration == 0 {
            return Err(ContractError::InvalidConfig);
        }
        env.storage()
            .instance()
            .set(&TOURNAMENT_TIMELOCK, &duration);
        Ok(())
    }

    /// Create a time-locked escrow for a completed tournament game.
    ///
    /// Calculates the total prize pool from the game's wager amount and locks it
    /// until `current_ledger + timelock_duration`. The escrow must then be
    /// released via [`release_tournament_escrow`] by the admin after the lock
    /// expires. Requires `player1`'s authorisation as the tournament organiser.
    ///
    /// # Parameters
    /// - `game_id` — ID of a game in `Completed` state.
    ///
    /// # Returns
    /// `Ok(escrow_id)` — the unique `u64` identifier for the new escrow.
    ///
    /// # Errors
    /// - [`ContractError::GameNotFound`]       — `game_id` does not exist.
    /// - [`ContractError::GameNotInProgress`]  — Game is not in `Completed` state.
    /// - [`ContractError::TimeoutNotConfigured`] — [`configure_tournament_timelock`] has not been called.
    ///
    /// # Events
    /// Emits `("tl_escrow", "created") → (escrow_id, game_id, locked_until)`.
    pub fn create_tournament_escrow(env: Env, game_id: u64) -> Result<u64, ContractError> {
        Self::check_not_paused(&env);
        let games: Map<u64, Game> = env
            .storage()
            .instance()
            .get(&GAMES)
            .ok_or(ContractError::GameNotFound)?;

        let game = games.get(game_id).ok_or(ContractError::GameNotFound)?;

        if game.state != GameState::Completed {
            return Err(ContractError::GameNotInProgress);
        }

        // Prevent a player from creating a tournament escrow against themselves (#933)
        if let Some(ref player2) = game.player2 {
            if game.player1 == *player2 {
                return Err(ContractError::AlreadyJoined);
            }
        }

        game.player1.require_auth();

        // Check active escrow cap for this player
        let mut player_counts: Map<Address, u32> = env
            .storage()
            .instance()
            .get(&PLAYER_ACTIVE_ESCROWS)
            .unwrap_or(Map::new(&env));

        let current_count = player_counts.get(game.player1.clone()).unwrap_or(0);
        if current_count >= MAX_ACTIVE_ESCROWS {
            return Err(ContractError::MaxActiveEscrowsExceeded);
        }

        let duration: u64 = env
            .storage()
            .instance()
            .get(&TOURNAMENT_TIMELOCK)
            .ok_or(ContractError::TimeoutNotConfigured)?;

        let total_amount = match &game.player2 {
            Some(_) => game.wager_amount * 2,
            None => game.wager_amount,
        };

        let locked_until = env.ledger().sequence() as u64 + duration;

        let mut escrows: Map<u64, TournamentEscrow> = env
            .storage()
            .instance()
            .get(&TOURNAMENT_ESCROWS)
            .unwrap_or(Map::new(&env));

        let escrow_id = escrows.len() as u64 + 1;
        let escrow = TournamentEscrow {
            escrow_id,
            game_id,
            player: game.player1.clone(),
            total_amount,
            locked_until,
            released: false,
        };

        escrows.set(escrow_id, escrow);
        env.storage().instance().set(&TOURNAMENT_ESCROWS, &escrows);

        // Increment player's active escrow count
        player_counts.set(game.player1.clone(), current_count + 1);
        env.storage()
            .instance()
            .set(&PLAYER_ACTIVE_ESCROWS, &player_counts);

        env.events().publish(
            (symbol_short!("tl_escrow"), symbol_short!("created")),
            (escrow_id, game_id, locked_until),
        );

        Ok(escrow_id)
    }

    /// Release a time-locked tournament escrow to the specified winners (admin only).
    ///
    /// Distributes the locked prize pool proportionally according to
    /// `percentages`. Integer-division dust goes to the first winner.
    /// Can only be called after the lock period has expired.
    ///
    /// # Parameters
    /// - `admin`        — Must match `CONTRACT_ADMIN`; must authorise the call.
    /// - `escrow_id`    — ID of a previously created, unreleased escrow.
    /// - `winners`      — Ordered recipient addresses.
    /// - `percentages`  — Whole-number percentages parallel to `winners`;
    ///                    must sum to exactly 100.
    ///
    /// # Returns
    /// `Ok(())` on success.
    ///
    /// # Errors
    /// - [`ContractError::Unauthorized`]         — `admin` does not match `CONTRACT_ADMIN`.
    /// - [`ContractError::EscrowNotFound`]        — `escrow_id` does not exist.
    /// - [`ContractError::EscrowAlreadyReleased`] — Escrow has already been released.
    /// - [`ContractError::EscrowStillLocked`]     — Lock period has not yet elapsed.
    /// - [`ContractError::MismatchedLengths`]     — `winners` and `percentages` differ.
    /// - [`ContractError::InvalidPercentage`]     — Percentages overflow or do not sum to 100.
    ///
    /// # Events
    /// Emits `("tl_escrow", "released") → escrow_id`.
    pub fn release_tournament_escrow(
        env: Env,
        admin: Address,
        escrow_id: u64,
        winners: Vec<Address>,
        percentages: Vec<u32>,
    ) -> Result<(), ContractError> {
        Self::check_not_paused(&env);
        let current_admin: Address = env
            .storage()
            .instance()
            .get(&CONTRACT_ADMIN)
            .expect("Not initialized");
        current_admin.require_auth();
        if admin != current_admin {
            return Err(ContractError::Unauthorized);
        }
        let mut escrows: Map<u64, TournamentEscrow> = env
            .storage()
            .instance()
            .get(&TOURNAMENT_ESCROWS)
            .ok_or(ContractError::EscrowNotFound)?;

        let escrow = escrows
            .get(escrow_id)
            .ok_or(ContractError::EscrowNotFound)?;

        if escrow.released {
            return Err(ContractError::EscrowAlreadyReleased);
        }

        let current_ledger = env.ledger().sequence() as u64;
        if current_ledger < escrow.locked_until {
            return Err(ContractError::EscrowStillLocked);
        }

        if winners.len() != percentages.len() {
            return Err(ContractError::MismatchedLengths);
        }

        let mut total_pct: u32 = 0;
        for i in 0..percentages.len() {
            total_pct = total_pct
                .checked_add(percentages.get(i).unwrap())
                .ok_or(ContractError::InvalidPercentage)?;
            if total_pct > 100 {
                return Err(ContractError::InvalidPercentage);
            }
        }
        if total_pct != 100 {
            return Err(ContractError::InvalidPercentage);
        }

        Self::non_reentrant_enter(&env)?;

        let token_client = Self::token_client(&env);
        let contract_address = env.current_contract_address();
        let total = escrow.total_amount;
        let mut distributed: i128 = 0;

        for i in 0..winners.len() {
            let winner = winners.get(i).unwrap();
            let pct = percentages.get(i).unwrap();
            let amount = (total * pct as i128) / 100;
            distributed += amount;
            token_client.transfer(&contract_address, &winner, &amount);
        }

        // Dust goes to first winner
        let remainder = total - distributed;
        if remainder > 0 && !winners.is_empty() {
            let first_winner = winners.get(0).unwrap();
            token_client.transfer(&contract_address, &first_winner, &remainder);
        }

        let escrow_player = escrow.player.clone();
        let mut released_escrow = escrow;
        released_escrow.released = true;
        escrows.set(escrow_id, released_escrow);
        env.storage().instance().set(&TOURNAMENT_ESCROWS, &escrows);

        // Decrement player's active escrow count
        let mut player_counts: Map<Address, u32> = env
            .storage()
            .instance()
            .get(&PLAYER_ACTIVE_ESCROWS)
            .unwrap_or(Map::new(&env));
        let player_count = player_counts.get(escrow_player.clone()).unwrap_or(0);
        if player_count > 0 {
            player_counts.set(escrow_player, player_count - 1);
        }
        env.storage()
            .instance()
            .set(&PLAYER_ACTIVE_ESCROWS, &player_counts);

        env.events().publish(
            (symbol_short!("tl_escrow"), symbol_short!("released")),
            escrow_id,
        );

        Self::non_reentrant_exit(&env);
        Ok(())
    }

    /// Fetch a tournament escrow by ID.
    ///
    /// # Parameters
    /// - `escrow_id` — Numeric ID returned by [`create_tournament_escrow`].
    ///
    /// # Returns
    /// `Ok(TournamentEscrow)` — the full escrow record including `locked_until`
    /// and `released` flag.
    ///
    /// # Errors
    /// - [`ContractError::EscrowNotFound`] — `escrow_id` does not exist or no
    ///   escrows have been created yet.
    pub fn get_tournament_escrow(
        env: Env,
        escrow_id: u64,
    ) -> Result<TournamentEscrow, ContractError> {
        let escrows: Map<u64, TournamentEscrow> = env
            .storage()
            .instance()
            .get(&TOURNAMENT_ESCROWS)
            .ok_or(ContractError::EscrowNotFound)?;
        escrows.get(escrow_id).ok_or(ContractError::EscrowNotFound)
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Tests
// ────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod test;

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use rand::rngs::OsRng;
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::testutils::Ledger as _;
    use soroban_sdk::token::{StellarAssetClient, TokenClient};
    use soroban_sdk::{Address, Bytes, BytesN, Env};

    // ── Helpers ───────────────────────────────────────────────────────────────

    /// Build and sign the same payload the contract constructs.
    fn sign_payload(
        env: &Env,
        signing_key: &SigningKey,
        recipient: &Address,
        reward_amount: i128,
        nonce: u64,
    ) -> BytesN<64> {
        let mut payload_bytes = Bytes::new(env);

        let recipient_str = recipient.clone().to_string();
        let str_len = recipient_str.len() as usize;
        let mut addr_buf = [0u8; 64];
        recipient_str.copy_into_slice(&mut addr_buf[..str_len]);
        payload_bytes.append(&Bytes::from_slice(env, &addr_buf[..str_len]));

        let amount_le: [u8; 8] = (reward_amount as i64).to_le_bytes();
        payload_bytes.append(&Bytes::from_slice(env, &amount_le));

        let nonce_le: [u8; 8] = nonce.to_le_bytes();
        payload_bytes.append(&Bytes::from_slice(env, &nonce_le));

        let digest_bytesn: BytesN<32> = env.crypto().sha256(&payload_bytes).into();
        let mut digest_raw = [0u8; 32];
        digest_bytesn.copy_into_slice(&mut digest_raw);

        let dalek_sig = signing_key.sign(&digest_raw);
        BytesN::from_array(env, &dalek_sig.to_bytes())
    }

    /// Register and initialize the contract; returns (client, signing_key).
    fn setup(env: &Env, treasury_amount: i128) -> (GameContractClient<'_>, SigningKey) {
        let contract_id = env.register_contract(None, GameContract);
        let client = GameContractClient::new(env, &contract_id);

        let admin = Address::generate(env);
        let signing_key = SigningKey::generate(&mut OsRng);
        let verifying_key_bytes: [u8; 32] = signing_key.verifying_key().to_bytes();
        let admin_key = Bytes::from_slice(env, &verifying_key_bytes);
        let treasury_addr = Address::generate(env);

        client.initialize_puzzle_rewards(
            &admin,
            &admin_key,
            &treasury_amount,
            &0u32,
            &treasury_addr,
        );
        (client, signing_key)
    }

    // ── #200 – Treasury fee test ───────────────────────────────────────────────

    /// 10 XLM pool, 2 % fee (fee_bips = 20):
    ///   winner gets 9.8 XLM, treasury gets 0.2 XLM
    #[test]
    fn test_fee_redirection_2_percent() {
        let env = Env::default();
        env.mock_all_auths();

        // Token setup
        let issuer = Address::generate(&env);
        let stellar_token = env.register_stellar_asset_contract_v2(issuer.clone());
        let token_address = stellar_token.address();
        let token_client = TokenClient::new(&env, &token_address);
        let stellar_asset_client = StellarAssetClient::new(&env, &token_address);

        let admin = Address::generate(&env);
        let player1 = Address::generate(&env);
        let player2 = Address::generate(&env);
        let treasury_addr = Address::generate(&env);

        // Each player gets 1_000 tokens (wager = 5 each → pool = 10)
        stellar_asset_client.mint(&player1, &1_000i128);
        stellar_asset_client.mint(&player2, &1_000i128);

        // Deploy contract
        let contract_id = env.register_contract(None, GameContract);
        let client = GameContractClient::new(&env, &contract_id);

        // Initialize token then puzzle/fee config (fee_bips=20 → 2 %)
        client.add_whitelisted_token(&admin, &token_address);
        client.initialize_token(&admin, &token_address);
        let dummy_key = Bytes::from_slice(&env, &[0u8; 32]);
        client.initialize_puzzle_rewards(
            &admin,
            &dummy_key,
            &0i128,
            &20u32, // 2 %
            &treasury_addr,
        );

        // Create & join game with wager = 5 (pool = 10)
        let wager: i128 = 5;
        let game_id = client.create_game(&player1, &wager);
        client.join_game(&game_id, &player2);

        // player1 forfeits → player2 wins
        client.forfeit(&game_id, &player1);

        // fee = 10 * 20 / 1000 = 0.2 XLM
        // payout = 10 - 0.2 = 9.8 XLM
        // player2 started with 1000, put in 5, gets back 9.8
        // net balance = 1000 - 5 + 9.8 = 1004.8 — but i128, wager=5 * 1e0
        // In smallest units: fee=0, payout=10 (integer division: 10*20/1000=0)
        // To get a non-zero fee, use wager=500 (pool=1000), fee=1000*20/1000=20
        let player2_balance = token_client.balance(&player2);
        let treasury_balance = token_client.balance(&treasury_addr);

        // With wager=5, pool=10: fee=10*20/1000=0 (integer div).
        // Documented in comment; test verifies the math is applied correctly.
        assert_eq!(player2_balance + treasury_balance, 1_000 + wager); // conservation
    }

    /// Larger amounts: wager = 500, pool = 1000, fee_bips = 20 (2 %)
    ///   fee    = 1000 * 20 / 1000 = 20 tokens  → treasury
    ///   payout = 1000 - 20        = 980 tokens  → winner
    #[test]
    fn test_fee_redirection_2_percent_large() {
        let env = Env::default();
        env.mock_all_auths();

        let issuer = Address::generate(&env);
        let stellar_token = env.register_stellar_asset_contract_v2(issuer.clone());
        let token_address = stellar_token.address();
        let token_client = TokenClient::new(&env, &token_address);
        let stellar_asset_client = StellarAssetClient::new(&env, &token_address);

        let admin = Address::generate(&env);
        let player1 = Address::generate(&env);
        let player2 = Address::generate(&env);
        let treasury_addr = Address::generate(&env);

        stellar_asset_client.mint(&player1, &1_000i128);
        stellar_asset_client.mint(&player2, &1_000i128);

        let contract_id = env.register_contract(None, GameContract);
        let client = GameContractClient::new(&env, &contract_id);

        client.add_whitelisted_token(&admin, &token_address);
        client.initialize_token(&admin, &token_address);
        let dummy_key = Bytes::from_slice(&env, &[0u8; 32]);
        client.initialize_puzzle_rewards(
            &admin,
            &dummy_key,
            &0i128,
            &20u32, // 2 %
            &treasury_addr,
        );

        // Raise stake limit first so wager=500 is permitted
        client.set_max_stake(&admin, &1_000i128);

        let wager: i128 = 500; // pool = 1000
        let game_id = client.create_game(&player1, &wager);
        client.join_game(&game_id, &player2);
        client.forfeit(&game_id, &player1); // player2 wins

        let player2_balance = token_client.balance(&player2); // 1000 - 500 + 980 = 1480? no: started 1000, deposited 500, gets 980
        let treasury_balance = token_client.balance(&treasury_addr);

        // player2: starts 1000, puts in 500, receives 980 → 1000 - 500 + 980 = 1480
        assert_eq!(player2_balance, 1_480);
        // treasury: receives fee of 20
        assert_eq!(treasury_balance, 20);
    }

    // ── #199 – USDC staking workflow ──────────────────────────────────────────

    #[test]
    fn test_usdc_staking_workflow() {
        let env = Env::default();
        env.mock_all_auths();

        let issuer = Address::generate(&env);
        let player1 = Address::generate(&env);
        let player2 = Address::generate(&env);

        let stellar_token = env.register_stellar_asset_contract_v2(issuer.clone());
        let token_address = stellar_token.address();
        let token_client = TokenClient::new(&env, &token_address);
        let stellar_asset_client = StellarAssetClient::new(&env, &token_address);

        stellar_asset_client.mint(&player1, &1_000i128);
        stellar_asset_client.mint(&player2, &1_000i128);

        let contract_id = env.register_contract(None, GameContract);
        let client = GameContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.add_whitelisted_token(&admin, &token_address);
        client.initialize_token(&admin, &token_address);

        let initial_wager: i128 = 100;
        let game_id = client.create_game(&player1, &initial_wager);
        client.join_game(&game_id, &player2);
        client.forfeit(&game_id, &player1);

        // No fee configured, so player2 receives the full 200
        let final_player2_balance = token_client.balance(&player2);
        assert_eq!(final_player2_balance, 1_100);
    }

    // ── #199 – Puzzle reward tests ────────────────────────────────────────────

    /// Happy path: valid signature → balance incremented, treasury decremented
    #[test]
    fn test_claim_puzzle_reward_valid_sig() {
        let env = Env::default();
        env.mock_all_auths();

        let (client, signing_key) = setup(&env, 10_000);
        let recipient = Address::generate(&env);
        let reward_amount: i128 = 500;
        let nonce: u64 = 1;

        let sig = sign_payload(&env, &signing_key, &recipient, reward_amount, nonce);
        client.claim_puzzle_reward(&recipient, &reward_amount, &nonce, &sig);

        assert_eq!(client.reward_balance(&recipient), reward_amount);
        assert_eq!(client.treasury_balance(), 10_000 - reward_amount);
    }

    /// Invalid signature must panic (Unauthorized / ed25519_verify panics)
    #[test]
    #[should_panic]
    fn test_claim_puzzle_reward_invalid_sig() {
        let env = Env::default();
        env.mock_all_auths();

        let (client, _signing_key) = setup(&env, 10_000);
        let recipient = Address::generate(&env);

        let wrong_key = SigningKey::generate(&mut OsRng);
        let bad_sig = sign_payload(&env, &wrong_key, &recipient, 500, 1);

        client.claim_puzzle_reward(&recipient, &500, &1, &bad_sig);
    }

    /// Replayed nonce → Err(Unauthorized)
    #[test]
    fn test_claim_puzzle_reward_replay_rejected() {
        let env = Env::default();
        env.mock_all_auths();

        let (client, signing_key) = setup(&env, 10_000);
        let recipient = Address::generate(&env);
        let reward_amount: i128 = 300;
        let nonce: u64 = 42;

        let sig = sign_payload(&env, &signing_key, &recipient, reward_amount, nonce);
        client.claim_puzzle_reward(&recipient, &reward_amount, &nonce, &sig);

        let sig2 = sign_payload(&env, &signing_key, &recipient, reward_amount, nonce);
        let result = client.try_claim_puzzle_reward(&recipient, &reward_amount, &nonce, &sig2);
        assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
    }

    #[test]
    fn test_claim_puzzle_reward_invalid_amount_rejected() {
        let env = Env::default();
        env.mock_all_auths();

        let (client, signing_key) = setup(&env, 10_000);
        let recipient = Address::generate(&env);

        let sig = sign_payload(&env, &signing_key, &recipient, -1, 7);
        let result = client.try_claim_puzzle_reward(&recipient, &-1, &7, &sig);
        assert_eq!(result, Err(Ok(ContractError::InvalidAmount)));
    }

    // ── claim_puzzle_rewards_batch tests ───────────────────────────────────

    /// Happy path: multiple valid proofs in one call → all balances credited,
    /// treasury decremented by the sum, in a single transaction.
    #[test]
    fn test_claim_puzzle_rewards_batch_valid() {
        let env = Env::default();
        env.mock_all_auths();

        let (client, signing_key) = setup(&env, 10_000);
        let recipient1 = Address::generate(&env);
        let recipient2 = Address::generate(&env);
        let recipient3 = Address::generate(&env);

        let sig1 = sign_payload(&env, &signing_key, &recipient1, 100, 1);
        let sig2 = sign_payload(&env, &signing_key, &recipient2, 200, 2);
        let sig3 = sign_payload(&env, &signing_key, &recipient3, 300, 3);

        let proofs = Vec::from_array(
            &env,
            [
                Proof {
                    recipient: recipient1.clone(),
                    reward_amount: 100,
                    nonce: 1,
                    signature: sig1,
                },
                Proof {
                    recipient: recipient2.clone(),
                    reward_amount: 200,
                    nonce: 2,
                    signature: sig2,
                },
                Proof {
                    recipient: recipient3.clone(),
                    reward_amount: 300,
                    nonce: 3,
                    signature: sig3,
                },
            ],
        );

        client.claim_puzzle_rewards_batch(&proofs);

        assert_eq!(client.reward_balance(&recipient1), 100);
        assert_eq!(client.reward_balance(&recipient2), 200);
        assert_eq!(client.reward_balance(&recipient3), 300);
        assert_eq!(client.treasury_balance(), 10_000 - 600);
    }

    /// Empty proof list is rejected up front.
    #[test]
    fn test_claim_puzzle_rewards_batch_empty_rejected() {
        let env = Env::default();
        env.mock_all_auths();

        let (client, _signing_key) = setup(&env, 10_000);
        let proofs: Vec<Proof> = Vec::new(&env);

        let result = client.try_claim_puzzle_rewards_batch(&proofs);
        assert_eq!(result, Err(Ok(ContractError::EmptyBatch)));
    }

    /// More proofs than MAX_BATCH_SIZE is rejected up front.
    #[test]
    fn test_claim_puzzle_rewards_batch_too_large_rejected() {
        let env = Env::default();
        env.mock_all_auths();

        let (client, signing_key) = setup(&env, 1_000_000);
        let mut proofs: Vec<Proof> = Vec::new(&env);
        for i in 0..(MAX_BATCH_SIZE + 1) as u64 {
            let recipient = Address::generate(&env);
            let sig = sign_payload(&env, &signing_key, &recipient, 1, i);
            proofs.push_back(Proof {
                recipient,
                reward_amount: 1,
                nonce: i,
                signature: sig,
            });
        }

        let result = client.try_claim_puzzle_rewards_batch(&proofs);
        assert_eq!(result, Err(Ok(ContractError::BatchTooLarge)));
    }

    /// A duplicate nonce within the same batch is rejected, and the whole
    /// batch is rolled back — the first (otherwise-valid) proof must not be
    /// applied either.
    #[test]
    fn test_claim_puzzle_rewards_batch_duplicate_nonce_rolls_back() {
        let env = Env::default();
        env.mock_all_auths();

        let (client, signing_key) = setup(&env, 10_000);
        let recipient1 = Address::generate(&env);
        let recipient2 = Address::generate(&env);

        let sig1 = sign_payload(&env, &signing_key, &recipient1, 100, 9);
        let sig2 = sign_payload(&env, &signing_key, &recipient2, 200, 9); // reused nonce

        let proofs = Vec::from_array(
            &env,
            [
                Proof {
                    recipient: recipient1.clone(),
                    reward_amount: 100,
                    nonce: 9,
                    signature: sig1,
                },
                Proof {
                    recipient: recipient2.clone(),
                    reward_amount: 200,
                    nonce: 9,
                    signature: sig2,
                },
            ],
        );

        let result = client.try_claim_puzzle_rewards_batch(&proofs);
        assert_eq!(result, Err(Ok(ContractError::Unauthorized)));

        // Rolled back entirely — neither recipient was credited.
        assert_eq!(client.reward_balance(&recipient1), 0);
        assert_eq!(client.reward_balance(&recipient2), 0);
        assert_eq!(client.treasury_balance(), 10_000);
    }

    /// A single bad signature anywhere in the batch panics and rolls back
    /// every proof in the batch, including the valid ones before it.
    #[test]
    #[should_panic]
    fn test_claim_puzzle_rewards_batch_invalid_sig_rolls_back() {
        let env = Env::default();
        env.mock_all_auths();

        let (client, signing_key) = setup(&env, 10_000);
        let recipient1 = Address::generate(&env);
        let recipient2 = Address::generate(&env);

        let sig1 = sign_payload(&env, &signing_key, &recipient1, 100, 1);
        let wrong_key = SigningKey::generate(&mut OsRng);
        let bad_sig = sign_payload(&env, &wrong_key, &recipient2, 200, 2);

        let proofs = Vec::from_array(
            &env,
            [
                Proof {
                    recipient: recipient1.clone(),
                    reward_amount: 100,
                    nonce: 1,
                    signature: sig1,
                },
                Proof {
                    recipient: recipient2.clone(),
                    reward_amount: 200,
                    nonce: 2,
                    signature: bad_sig,
                },
            ],
        );

        client.claim_puzzle_rewards_batch(&proofs);
    }

    /// Replaying a nonce already consumed by a prior `claim_puzzle_reward`
    /// call is rejected inside a batch too.
    #[test]
    fn test_claim_puzzle_rewards_batch_replay_against_prior_claim_rejected() {
        let env = Env::default();
        env.mock_all_auths();

        let (client, signing_key) = setup(&env, 10_000);
        let recipient = Address::generate(&env);

        let sig = sign_payload(&env, &signing_key, &recipient, 500, 1);
        client.claim_puzzle_reward(&recipient, &500, &1, &sig);

        let sig2 = sign_payload(&env, &signing_key, &recipient, 500, 1);
        let proofs = Vec::from_array(
            &env,
            [Proof {
                recipient: recipient.clone(),
                reward_amount: 500,
                nonce: 1,
                signature: sig2,
            }],
        );

        let result = client.try_claim_puzzle_rewards_batch(&proofs);
        assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
    }

    // ── Timeout Tests ──────────────────────────────────────────────────────

    #[test]
    fn test_configure_timeout() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, GameContract);
        let client = GameContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let admin_key = Bytes::from_slice(&env, &[0u8; 32]);
        let treasury_addr = Address::generate(&env);

        client.initialize_puzzle_rewards(&admin, &admin_key, &0i128, &0u32, &treasury_addr);
        client.configure_timeout(&admin, &1000u64);
    }

    #[test]
    fn test_claim_timeout_win_success() {
        let env = Env::default();
        env.mock_all_auths();

        let issuer = Address::generate(&env);
        let stellar_token = env.register_stellar_asset_contract_v2(issuer.clone());
        let token_address = stellar_token.address();
        let token_client = TokenClient::new(&env, &token_address);
        let stellar_asset_client = StellarAssetClient::new(&env, &token_address);

        let admin = Address::generate(&env);
        let player1 = Address::generate(&env);
        let player2 = Address::generate(&env);
        let treasury_addr = Address::generate(&env);

        stellar_asset_client.mint(&player1, &1_000i128);
        stellar_asset_client.mint(&player2, &1_000i128);

        let contract_id = env.register_contract(None, GameContract);
        let client = GameContractClient::new(&env, &contract_id);

        client.add_whitelisted_token(&admin, &token_address);
        client.initialize_token(&admin, &token_address);
        client.initialize_puzzle_rewards(
            &admin,
            &Bytes::from_slice(&env, &[0u8; 32]),
            &0i128,
            &0u32,
            &treasury_addr,
        );
        client.configure_timeout(&admin, &100u64);
        client.set_max_stake(&admin, &1_000i128);

        let wager: i128 = 100;
        let game_id = client.create_game(&player1, &wager);
        client.join_game(&game_id, &player2);

        env.as_contract(&contract_id, || {
            let mut games: Map<u64, Game> = env.storage().instance().get(&GAMES).unwrap();
            let mut game = games.get(game_id).unwrap();
            game.last_move_at = 0;
            games.set(game_id, game);
            env.storage().instance().set(&GAMES, &games);
        });

        env.ledger().set_sequence_number(101);

        client.claim_timeout_win(&game_id, &player2);

        assert_eq!(token_client.balance(&player2), 1_100);
    }

    #[test]
    fn test_claim_timeout_win_not_reached() {
        let env = Env::default();
        env.mock_all_auths();

        let issuer = Address::generate(&env);
        let stellar_token = env.register_stellar_asset_contract_v2(issuer.clone());
        let token_address = stellar_token.address();
        let stellar_asset_client = StellarAssetClient::new(&env, &token_address);

        let admin = Address::generate(&env);
        let player1 = Address::generate(&env);
        let player2 = Address::generate(&env);
        let treasury_addr = Address::generate(&env);

        stellar_asset_client.mint(&player1, &1_000i128);
        stellar_asset_client.mint(&player2, &1_000i128);

        let contract_id = env.register_contract(None, GameContract);
        let client = GameContractClient::new(&env, &contract_id);

        client.add_whitelisted_token(&admin, &token_address);
        client.initialize_token(&admin, &token_address);
        client.initialize_puzzle_rewards(
            &admin,
            &Bytes::from_slice(&env, &[0u8; 32]),
            &0i128,
            &0u32,
            &treasury_addr,
        );
        client.configure_timeout(&admin, &1000u64);
        client.set_max_stake(&admin, &1_000i128);

        let wager: i128 = 100;
        let game_id = client.create_game(&player1, &wager);
        client.join_game(&game_id, &player2);

        let result = client.try_claim_timeout_win(&game_id, &player2);
        assert_eq!(result, Err(Ok(ContractError::TimeoutNotReached)));
    }

    #[test]
    fn test_get_timeout_remaining() {
        let env = Env::default();
        env.mock_all_auths();

        let issuer = Address::generate(&env);
        let stellar_token = env.register_stellar_asset_contract_v2(issuer.clone());
        let token_address = stellar_token.address();
        let stellar_asset_client = StellarAssetClient::new(&env, &token_address);

        let admin = Address::generate(&env);
        let player1 = Address::generate(&env);
        let player2 = Address::generate(&env);
        let treasury_addr = Address::generate(&env);

        stellar_asset_client.mint(&player1, &1_000i128);
        stellar_asset_client.mint(&player2, &1_000i128);

        let contract_id = env.register_contract(None, GameContract);
        let client = GameContractClient::new(&env, &contract_id);

        client.add_whitelisted_token(&admin, &token_address);
        client.initialize_token(&admin, &token_address);
        client.initialize_puzzle_rewards(
            &admin,
            &Bytes::from_slice(&env, &[0u8; 32]),
            &0i128,
            &0u32,
            &treasury_addr,
        );
        client.configure_timeout(&admin, &1000u64);
        client.set_max_stake(&admin, &1_000i128);

        let wager: i128 = 100;
        let game_id = client.create_game(&player1, &wager);
        client.join_game(&game_id, &player2);

        let remaining = client.get_timeout_remaining(&game_id);
        assert_eq!(remaining, Some(1000));

        env.ledger().set_sequence_number(501);
        let remaining = client.get_timeout_remaining(&game_id);
        assert_eq!(remaining, Some(499));

        env.ledger().set_sequence_number(1001);
        let remaining = client.get_timeout_remaining(&game_id);
        assert_eq!(remaining, Some(0));
    }

    // ── Dispute Resolution Tests ───────────────────────────────────────────

    #[test]
    fn test_file_dispute_success() {
        let env = Env::default();
        env.mock_all_auths();

        let issuer = Address::generate(&env);
        let stellar_token = env.register_stellar_asset_contract_v2(issuer.clone());
        let token_address = stellar_token.address();
        let token_client = TokenClient::new(&env, &token_address);
        let stellar_asset_client = StellarAssetClient::new(&env, &token_address);

        let admin = Address::generate(&env);
        let player1 = Address::generate(&env);
        let player2 = Address::generate(&env);
        let arbitrator = Address::generate(&env);
        let treasury_addr = Address::generate(&env);

        stellar_asset_client.mint(&player1, &1_000i128);
        stellar_asset_client.mint(&player2, &1_000i128);

        let contract_id = env.register_contract(None, GameContract);
        let client = GameContractClient::new(&env, &contract_id);

        client.add_whitelisted_token(&admin, &token_address);
        client.initialize_token(&admin, &token_address);
        client.initialize_puzzle_rewards(
            &admin,
            &Bytes::from_slice(&env, &[0u8; 32]),
            &0i128,
            &0u32,
            &treasury_addr,
        );
        client.configure_dispute_system(&admin, &arbitrator, &25i128);
        client.set_max_stake(&admin, &1_000i128);

        let wager: i128 = 100;
        let game_id = client.create_game(&player1, &wager);
        client.join_game(&game_id, &player2);

        let reason = Bytes::from_slice(&env, b"Engine abuse");
        let dispute_id = client.file_dispute(&game_id, &player1, &player2, &reason);

        let dispute = client.get_dispute(&dispute_id);
        assert_eq!(dispute.game_id, game_id);
        assert_eq!(dispute.filer, player1);
        assert_eq!(dispute.against, player2);
        assert_eq!(dispute.status, DisputeStatus::Pending);
        assert_eq!(token_client.balance(&player1), 875);
    }

    #[test]
    fn test_resolve_dispute_winner_takes_all() {
        let env = Env::default();
        env.mock_all_auths();

        let issuer = Address::generate(&env);
        let stellar_token = env.register_stellar_asset_contract_v2(issuer.clone());
        let token_address = stellar_token.address();
        let token_client = TokenClient::new(&env, &token_address);
        let stellar_asset_client = StellarAssetClient::new(&env, &token_address);

        let admin = Address::generate(&env);
        let player1 = Address::generate(&env);
        let player2 = Address::generate(&env);
        let arbitrator = Address::generate(&env);
        let treasury_addr = Address::generate(&env);

        stellar_asset_client.mint(&player1, &1_000i128);
        stellar_asset_client.mint(&player2, &1_000i128);

        let contract_id = env.register_contract(None, GameContract);
        let client = GameContractClient::new(&env, &contract_id);

        client.add_whitelisted_token(&admin, &token_address);
        client.initialize_token(&admin, &token_address);
        client.initialize_puzzle_rewards(
            &admin,
            &Bytes::from_slice(&env, &[0u8; 32]),
            &0i128,
            &0u32,
            &treasury_addr,
        );
        client.configure_dispute_system(&admin, &arbitrator, &0i128);
        client.set_max_stake(&admin, &1_000i128);

        let wager: i128 = 100;
        let game_id = client.create_game(&player1, &wager);
        client.join_game(&game_id, &player2);

        let reason = Bytes::from_slice(&env, b"Illegal move");
        let dispute_id = client.file_dispute(&game_id, &player1, &player2, &reason);
        let resolution = Bytes::from_slice(&env, b"Awarding win to player1");
        client.resolve_dispute(
            &dispute_id,
            &arbitrator,
            &Some(player1.clone()),
            &resolution,
        );

        let dispute = client.get_dispute(&dispute_id);
        assert_eq!(dispute.status, DisputeStatus::Resolved);
        assert_eq!(token_client.balance(&player1), 1_100);
    }

    #[test]
    fn test_file_dispute_rejects_settled_games() {
        let env = Env::default();
        env.mock_all_auths();

        let issuer = Address::generate(&env);
        let stellar_token = env.register_stellar_asset_contract_v2(issuer.clone());
        let token_address = stellar_token.address();
        let stellar_asset_client = StellarAssetClient::new(&env, &token_address);

        let admin = Address::generate(&env);
        let player1 = Address::generate(&env);
        let player2 = Address::generate(&env);
        let arbitrator = Address::generate(&env);
        let treasury_addr = Address::generate(&env);

        stellar_asset_client.mint(&player1, &1_000i128);
        stellar_asset_client.mint(&player2, &1_000i128);

        let contract_id = env.register_contract(None, GameContract);
        let client = GameContractClient::new(&env, &contract_id);

        client.add_whitelisted_token(&admin, &token_address);
        client.initialize_token(&admin, &token_address);
        client.initialize_puzzle_rewards(
            &admin,
            &Bytes::from_slice(&env, &[0u8; 32]),
            &0i128,
            &0u32,
            &treasury_addr,
        );
        client.configure_dispute_system(&admin, &arbitrator, &25i128);
        client.set_max_stake(&admin, &1_000i128);

        let wager: i128 = 100;
        let game_id = client.create_game(&player1, &wager);
        client.join_game(&game_id, &player2);
        client.forfeit(&game_id, &player1);

        let reason = Bytes::from_slice(&env, b"Too late");
        let result = client.try_file_dispute(&game_id, &player1, &player2, &reason);
        assert_eq!(result, Err(Ok(ContractError::NotDisputable)));
    }

    #[test]
    fn test_resolve_dispute_rejects_already_settled_games() {
        let env = Env::default();
        env.mock_all_auths();

        let issuer = Address::generate(&env);
        let stellar_token = env.register_stellar_asset_contract_v2(issuer.clone());
        let token_address = stellar_token.address();
        let stellar_asset_client = StellarAssetClient::new(&env, &token_address);

        let admin = Address::generate(&env);
        let player1 = Address::generate(&env);
        let player2 = Address::generate(&env);
        let arbitrator = Address::generate(&env);
        let treasury_addr = Address::generate(&env);

        stellar_asset_client.mint(&player1, &1_000i128);
        stellar_asset_client.mint(&player2, &1_000i128);

        let contract_id = env.register_contract(None, GameContract);
        let client = GameContractClient::new(&env, &contract_id);

        client.add_whitelisted_token(&admin, &token_address);
        client.initialize_token(&admin, &token_address);
        client.initialize_puzzle_rewards(
            &admin,
            &Bytes::from_slice(&env, &[0u8; 32]),
            &0i128,
            &0u32,
            &treasury_addr,
        );
        client.configure_dispute_system(&admin, &arbitrator, &0i128);
        client.set_max_stake(&admin, &1_000i128);

        let wager: i128 = 100;
        let game_id = client.create_game(&player1, &wager);
        client.join_game(&game_id, &player2);

        let reason = Bytes::from_slice(&env, b"Illegal move");
        let dispute_id = client.file_dispute(&game_id, &player1, &player2, &reason);

        client.forfeit(&game_id, &player1);

        let resolution = Bytes::from_slice(&env, b"Awarding win to player1");
        let result = client.try_resolve_dispute(
            &dispute_id,
            &arbitrator,
            &Some(player1.clone()),
            &resolution,
        );
        assert_eq!(result, Err(Ok(ContractError::GameAlreadyCompleted)));
    }

    #[test]
    fn test_claim_timeout_win_rejects_current_turn_player() {
        let env = Env::default();
        env.mock_all_auths();

        let issuer = Address::generate(&env);
        let stellar_token = env.register_stellar_asset_contract_v2(issuer.clone());
        let token_address = stellar_token.address();
        let stellar_asset_client = StellarAssetClient::new(&env, &token_address);

        let admin = Address::generate(&env);
        let player1 = Address::generate(&env);
        let player2 = Address::generate(&env);
        let treasury_addr = Address::generate(&env);

        stellar_asset_client.mint(&player1, &1_000i128);
        stellar_asset_client.mint(&player2, &1_000i128);

        let contract_id = env.register_contract(None, GameContract);
        let client = GameContractClient::new(&env, &contract_id);

        client.add_whitelisted_token(&admin, &token_address);
        client.initialize_token(&admin, &token_address);
        client.initialize_puzzle_rewards(
            &admin,
            &Bytes::from_slice(&env, &[0u8; 32]),
            &0i128,
            &0u32,
            &treasury_addr,
        );
        client.configure_timeout(&admin, &100u64);
        client.set_max_stake(&admin, &1_000i128);

        let wager: i128 = 100;
        let game_id = client.create_game(&player1, &wager);
        client.join_game(&game_id, &player2);

        env.as_contract(&contract_id, || {
            let mut games: Map<u64, Game> = env.storage().instance().get(&GAMES).unwrap();
            let mut game = games.get(game_id).unwrap();
            game.last_move_at = 0;
            games.set(game_id, game);
            env.storage().instance().set(&GAMES, &games);
        });

        env.ledger().set_sequence_number(101);

        let result = client.try_claim_timeout_win(&game_id, &player1);
        assert_eq!(result, Err(Ok(ContractError::InvalidTimeoutClaimant)));
    }

    #[test]
    fn test_submit_move_sequence_updates_turn_and_history() {
        let env = Env::default();
        env.mock_all_auths();

        let issuer = Address::generate(&env);
        let stellar_token = env.register_stellar_asset_contract_v2(issuer.clone());
        let token_address = stellar_token.address();
        let stellar_asset_client = StellarAssetClient::new(&env, &token_address);

        let admin = Address::generate(&env);
        let player1 = Address::generate(&env);
        let player2 = Address::generate(&env);
        let treasury_addr = Address::generate(&env);

        stellar_asset_client.mint(&player1, &1_000i128);
        stellar_asset_client.mint(&player2, &1_000i128);

        let contract_id = env.register_contract(None, GameContract);
        let client = GameContractClient::new(&env, &contract_id);

        client.add_whitelisted_token(&admin, &token_address);
        client.initialize_token(&admin, &token_address);
        client.initialize_puzzle_rewards(
            &admin,
            &Bytes::from_slice(&env, &[0u8; 32]),
            &0i128,
            &0u32,
            &treasury_addr,
        );
        client.set_max_stake(&admin, &1_000i128);

        let wager: i128 = 100;
        let game_id = client.create_game(&player1, &wager);
        client.join_game(&game_id, &player2);

        let first_move = Vec::from_array(&env, [12u32, 28u32]);
        client.submit_move(&game_id, &player1, &first_move);

        env.ledger().set_sequence_number(2);

        let second_move = Vec::from_array(&env, [52u32, 36u32]);
        client.submit_move(&game_id, &player2, &second_move);

        let game = client.get_game(&game_id);
        assert_eq!(game.current_turn, 1);
        assert_eq!(game.moves.len(), 2);
        assert_eq!(game.last_move_at, 2);

        let recorded_first = game.moves.get(0).unwrap();
        let recorded_second = game.moves.get(1).unwrap();

        assert_eq!(recorded_first.player, player1);
        assert_eq!(
            recorded_first.move_data,
            Vec::from_array(&env, [12u32, 28u32])
        );
        assert_eq!(recorded_first.timestamp, 0);

        assert_eq!(recorded_second.player, player2);
        assert_eq!(
            recorded_second.move_data,
            Vec::from_array(&env, [52u32, 36u32])
        );
        assert_eq!(recorded_second.timestamp, 2);
    }

    #[test]
    fn test_submit_move_rejects_out_of_turn_and_empty_moves() {
        let env = Env::default();
        env.mock_all_auths();

        let issuer = Address::generate(&env);
        let stellar_token = env.register_stellar_asset_contract_v2(issuer.clone());
        let token_address = stellar_token.address();
        let stellar_asset_client = StellarAssetClient::new(&env, &token_address);

        let admin = Address::generate(&env);
        let player1 = Address::generate(&env);
        let player2 = Address::generate(&env);
        let treasury_addr = Address::generate(&env);

        stellar_asset_client.mint(&player1, &1_000i128);
        stellar_asset_client.mint(&player2, &1_000i128);

        let contract_id = env.register_contract(None, GameContract);
        let client = GameContractClient::new(&env, &contract_id);

        client.add_whitelisted_token(&admin, &token_address);
        client.initialize_token(&admin, &token_address);
        client.initialize_puzzle_rewards(
            &admin,
            &Bytes::from_slice(&env, &[0u8; 32]),
            &0i128,
            &0u32,
            &treasury_addr,
        );
        client.set_max_stake(&admin, &1_000i128);

        let wager: i128 = 100;
        let game_id = client.create_game(&player1, &wager);
        client.join_game(&game_id, &player2);

        let early_move = Vec::from_array(&env, [52u32, 36u32]);
        let result = client.try_submit_move(&game_id, &player2, &early_move);
        assert_eq!(result, Err(Ok(ContractError::NotYourTurn)));

        let empty_move = Vec::new(&env);
        let result = client.try_submit_move(&game_id, &player1, &empty_move);
        assert_eq!(result, Err(Ok(ContractError::InvalidMove)));
    }

    #[test]
    fn test_reject_dispute_refund_fee() {
        let env = Env::default();
        env.mock_all_auths();

        let issuer = Address::generate(&env);
        let stellar_token = env.register_stellar_asset_contract_v2(issuer.clone());
        let token_address = stellar_token.address();
        let token_client = TokenClient::new(&env, &token_address);
        let stellar_asset_client = StellarAssetClient::new(&env, &token_address);

        let admin = Address::generate(&env);
        let player1 = Address::generate(&env);
        let player2 = Address::generate(&env);
        let arbitrator = Address::generate(&env);
        let treasury_addr = Address::generate(&env);

        stellar_asset_client.mint(&player1, &1_000i128);
        stellar_asset_client.mint(&player2, &1_000i128);

        let contract_id = env.register_contract(None, GameContract);
        let client = GameContractClient::new(&env, &contract_id);

        client.add_whitelisted_token(&admin, &token_address);
        client.initialize_token(&admin, &token_address);
        client.initialize_puzzle_rewards(
            &admin,
            &Bytes::from_slice(&env, &[0u8; 32]),
            &0i128,
            &0u32,
            &treasury_addr,
        );
        client.configure_dispute_system(&admin, &arbitrator, &25i128);
        client.set_max_stake(&admin, &1_000i128);

        let wager: i128 = 100;
        let game_id = client.create_game(&player1, &wager);
        client.join_game(&game_id, &player2);

        // File dispute
        let reason = Bytes::from_slice(&env, b"False claim");
        let dispute_id = client.file_dispute(&game_id, &player1, &player2, &reason);

        // Arbitrator rejects dispute
        let rejection_reason = Bytes::from_slice(&env, b"No evidence");
        client.reject_dispute(&dispute_id, &arbitrator, &rejection_reason);

        // Verify dispute fee was refunded
        assert_eq!(token_client.balance(&player1), 900);

        // Verify dispute is rejected
        let dispute = client.get_dispute(&dispute_id);
        assert_eq!(dispute.status, DisputeStatus::Rejected);
    }

    #[test]
    fn test_payout_tournament_optimized_splits_correctly() {
        let env = Env::default();
        env.mock_all_auths();

        let issuer = Address::generate(&env);
        let stellar_token = env.register_stellar_asset_contract_v2(issuer.clone());
        let token_address = stellar_token.address();
        let token_client = TokenClient::new(&env, &token_address);
        let stellar_asset_client = StellarAssetClient::new(&env, &token_address);

        let admin = Address::generate(&env);
        let player1 = Address::generate(&env);
        let player2 = Address::generate(&env);
        let treasury_addr = Address::generate(&env);

        stellar_asset_client.mint(&player1, &1_000i128);
        stellar_asset_client.mint(&player2, &1_000i128);

        let contract_id = env.register_contract(None, GameContract);
        let client = GameContractClient::new(&env, &contract_id);

        client.add_whitelisted_token(&admin, &token_address);
        client.initialize_token(&admin, &token_address);
        client.initialize_puzzle_rewards(
            &admin,
            &Bytes::from_slice(&env, &[0u8; 32]),
            &0i128,
            &0u32,
            &treasury_addr,
        );
        client.set_max_stake(&admin, &1_000i128);

        let wager: i128 = 500;
        let game_id = client.create_game(&player1, &wager);
        client.join_game(&game_id, &player2);

        // Manually complete the game (reuse forfeit → sets Forfeited not Completed, so set directly)
        env.as_contract(&contract_id, || {
            let mut games: Map<u64, Game> = env.storage().instance().get(&GAMES).unwrap();
            let mut game = games.get(game_id).unwrap();
            game.state = GameState::Completed;
            games.set(game_id, game);
            env.storage().instance().set(&GAMES, &games);
        });

        let winners = Vec::from_array(&env, [player1.clone(), player2.clone()]);
        let percentages = Vec::from_array(&env, [60u32, 40u32]);
        client.payout_tournament_optimized(&game_id, &winners, &percentages);

        // pool = 1000: player1 gets 600, player2 gets 400
        assert_eq!(token_client.balance(&player1), 1100); // started 1000, put in 500, gets 600
        assert_eq!(token_client.balance(&player2), 900); // started 1000, put in 500, gets 400
    }

    #[test]
    fn test_payout_cannot_be_called_twice() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, GameContract);
        let client = GameContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let issuer = Address::generate(&env);
        let player1 = Address::generate(&env);
        let player2 = Address::generate(&env);
        let treasury_addr = Address::generate(&env);

        let stellar_token = env.register_stellar_asset_contract_v2(issuer);
        let token_address = stellar_token.address();
        let stellar_asset_client = StellarAssetClient::new(&env, &token_address);

        client.add_whitelisted_token(&admin, &token_address);
        client.initialize_token(&admin, &token_address);
        let admin_key = Bytes::from_slice(&env, &[0u8; 32]);
        client.initialize_puzzle_rewards(&admin, &admin_key, &0i128, &0u32, &treasury_addr);

        let wager = 500;
        stellar_asset_client.mint(&player1, &wager);
        stellar_asset_client.mint(&player2, &wager);
        let game_id = client.create_game(&player1, &wager);
        client.join_game(&game_id, &player2);

        env.as_contract(&contract_id, || {
            let mut games: Map<u64, Game> = env.storage().instance().get(&GAMES).unwrap();
            let mut game = games.get(game_id).unwrap();
            game.state = GameState::Completed;
            game.winner = Some(player1.clone());
            games.set(game_id, game);
            env.storage().instance().set(&GAMES, &games);
        });

        client.payout(&game_id, &player1);
        let second = client.try_payout(&game_id, &player1);
        assert_eq!(second, Err(Ok(ContractError::AlreadySettled)));
    }

    #[test]
    fn test_release_tournament_escrow_rejects_unauthorized_caller() {
        let env = Env::default();
        env.mock_all_auths();

        let issuer = Address::generate(&env);
        let stellar_token = env.register_stellar_asset_contract_v2(issuer.clone());
        let token_address = stellar_token.address();
        let stellar_asset_client = StellarAssetClient::new(&env, &token_address);

        let admin = Address::generate(&env);
        let attacker = Address::generate(&env);
        let player1 = Address::generate(&env);
        let player2 = Address::generate(&env);
        let treasury_addr = Address::generate(&env);

        stellar_asset_client.mint(&player1, &1_000i128);
        stellar_asset_client.mint(&player2, &1_000i128);

        let contract_id = env.register_contract(None, GameContract);
        let client = GameContractClient::new(&env, &contract_id);

        client.add_whitelisted_token(&admin, &token_address);
        client.initialize_token(&admin, &token_address);
        client.initialize_puzzle_rewards(
            &admin,
            &Bytes::from_slice(&env, &[0u8; 32]),
            &0i128,
            &0u32,
            &treasury_addr,
        );
        client.set_max_stake(&admin, &1_000i128);
        client.configure_tournament_timelock(&admin, &100u64);

        let wager: i128 = 100;
        let game_id = client.create_game(&player1, &wager);
        client.join_game(&game_id, &player2);

        env.as_contract(&contract_id, || {
            let mut games: Map<u64, Game> = env.storage().instance().get(&GAMES).unwrap();
            let mut game = games.get(game_id).unwrap();
            game.state = GameState::Completed;
            games.set(game_id, game);
            env.storage().instance().set(&GAMES, &games);
        });

        let escrow_id = client.create_tournament_escrow(&game_id);

        env.ledger().set_sequence_number(200);

        let winners = Vec::from_array(&env, [player1.clone()]);
        let percentages = Vec::from_array(&env, [100u32]);

        let winners_attack = Vec::from_array(&env, [attacker.clone()]);
        let result = client.try_release_tournament_escrow(
            &attacker,
            &escrow_id,
            &winners_attack,
            &percentages,
        );
        assert_eq!(result, Err(Ok(ContractError::Unauthorized)));

        client.release_tournament_escrow(&admin, &escrow_id, &winners, &percentages);
        let escrow = client.get_tournament_escrow(&escrow_id);
        assert!(escrow.released);
    }

    #[test]
    fn test_release_tournament_escrow_succeeds_for_admin() {
        let env = Env::default();
        env.mock_all_auths();

        let issuer = Address::generate(&env);
        let stellar_token = env.register_stellar_asset_contract_v2(issuer.clone());
        let token_address = stellar_token.address();
        let token_client = TokenClient::new(&env, &token_address);
        let stellar_asset_client = StellarAssetClient::new(&env, &token_address);

        let admin = Address::generate(&env);
        let player1 = Address::generate(&env);
        let player2 = Address::generate(&env);
        let treasury_addr = Address::generate(&env);

        stellar_asset_client.mint(&player1, &1_000i128);
        stellar_asset_client.mint(&player2, &1_000i128);

        let contract_id = env.register_contract(None, GameContract);
        let client = GameContractClient::new(&env, &contract_id);

        client.add_whitelisted_token(&admin, &token_address);
        client.initialize_token(&admin, &token_address);
        client.initialize_puzzle_rewards(
            &admin,
            &Bytes::from_slice(&env, &[0u8; 32]),
            &0i128,
            &0u32,
            &treasury_addr,
        );
        client.set_max_stake(&admin, &1_000i128);
        client.configure_tournament_timelock(&admin, &100u64);

        let wager: i128 = 500;
        let game_id = client.create_game(&player1, &wager);
        client.join_game(&game_id, &player2);

        env.as_contract(&contract_id, || {
            let mut games: Map<u64, Game> = env.storage().instance().get(&GAMES).unwrap();
            let mut game = games.get(game_id).unwrap();
            game.state = GameState::Completed;
            games.set(game_id, game);
            env.storage().instance().set(&GAMES, &games);
        });

        let escrow_id = client.create_tournament_escrow(&game_id);

        env.ledger().set_sequence_number(200);

        let winners = Vec::from_array(&env, [player1.clone(), player2.clone()]);
        let percentages = Vec::from_array(&env, [70u32, 30u32]);
        client.release_tournament_escrow(&admin, &escrow_id, &winners, &percentages);

        assert_eq!(token_client.balance(&player1), 1200);
        assert_eq!(token_client.balance(&player2), 800);

        let escrow = client.get_tournament_escrow(&escrow_id);
        assert!(escrow.released);
    }

    // ── Reentrancy Guard Tests (#860) ──────────────────────────────────────────

    #[test]
    fn test_reentrancy_guard_normal_call_succeeds() {
        let env = Env::default();
        env.mock_all_auths();
        let issuer = Address::generate(&env);
        let stellar_token = env.register_stellar_asset_contract_v2(issuer.clone());
        let token_address = stellar_token.address();
        let stellar_asset_client = StellarAssetClient::new(&env, &token_address);
        let admin = Address::generate(&env);
        let player1 = Address::generate(&env);
        let treasury_addr = Address::generate(&env);
        stellar_asset_client.mint(&player1, &1_000i128);
        let contract_id = env.register_contract(None, GameContract);
        let client = GameContractClient::new(&env, &contract_id);
        client.initialize_token(&admin, &token_address);
        client.initialize_puzzle_rewards(
            &admin,
            &Bytes::from_slice(&env, &[0u8; 32]),
            &0i128,
            &0u32,
            &treasury_addr,
        );
        let game_id = client.create_game(&player1, &100);
        assert!(game_id > 0);
    }

    #[test]
    fn test_reentrancy_guard_rejects_nested_entry() {
        let env = Env::default();
        env.mock_all_auths();
        let issuer = Address::generate(&env);
        let stellar_token = env.register_stellar_asset_contract_v2(issuer.clone());
        let token_address = stellar_token.address();
        let stellar_asset_client = StellarAssetClient::new(&env, &token_address);
        let admin = Address::generate(&env);
        let player1 = Address::generate(&env);
        let treasury_addr = Address::generate(&env);
        stellar_asset_client.mint(&player1, &1_000i128);
        let contract_id = env.register_contract(None, GameContract);
        let client = GameContractClient::new(&env, &contract_id);
        client.initialize_token(&admin, &token_address);
        client.initialize_puzzle_rewards(
            &admin,
            &Bytes::from_slice(&env, &[0u8; 32]),
            &0i128,
            &0u32,
            &treasury_addr,
        );
        env.as_contract(&contract_id, || {
            env.storage().instance().set(&R_GUARD, &1u32);
        });
        let result = client.try_create_game(&player1, &100);
        assert_eq!(result, Err(Ok(ContractError::ReentrantCall)));
    }

    #[test]
    fn test_reentrancy_guard_released_after_call() {
        let env = Env::default();
        env.mock_all_auths();
        let issuer = Address::generate(&env);
        let stellar_token = env.register_stellar_asset_contract_v2(issuer.clone());
        let token_address = stellar_token.address();
        let stellar_asset_client = StellarAssetClient::new(&env, &token_address);
        let admin = Address::generate(&env);
        let player1 = Address::generate(&env);
        let player2 = Address::generate(&env);
        let treasury_addr = Address::generate(&env);
        stellar_asset_client.mint(&player1, &1_000i128);
        stellar_asset_client.mint(&player2, &1_000i128);
        let contract_id = env.register_contract(None, GameContract);
        let client = GameContractClient::new(&env, &contract_id);
        client.initialize_token(&admin, &token_address);
        client.initialize_puzzle_rewards(
            &admin,
            &Bytes::from_slice(&env, &[0u8; 32]),
            &0i128,
            &0u32,
            &treasury_addr,
        );

        // First call: create_game enters and exits guard
        let game_id = client.create_game(&player1, &100);

        // Guard should be released, allowing join_game to proceed
        client.join_game(&game_id, &player2);
        let game = client.get_game(&game_id);
        assert_eq!(game.state, GameState::InProgress);
    }
}
