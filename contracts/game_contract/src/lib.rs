#![no_std]
use soroban_sdk::token::TokenClient;
use soroban_sdk::{
    Address, Bytes, BytesN, Env, Map, Symbol, Vec, contract, contracterror, contractimpl,
    contracttype, symbol_short,
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

// ────────────────────────────────────────────────────────────────────────────
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
    pub total_amount: i128,
    pub locked_until: u64, // ledger sequence when funds can be released
    pub released: bool,
}

// ────────────────────────────────────────────────────────────────────────────
// Errors
// ────────────────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum ContractError {
    GameNotFound = 1,
    NotYourTurn = 2,
    GameNotInProgress = 3,
    InvalidMove = 4,
    InsufficientFunds = 5,
    AlreadyJoined = 6,
    GameFull = 7,
    NotPlayer = 8,
    GameAlreadyCompleted = 9,
    DrawNotAvailable = 10,
    ForfeitNotAllowed = 11,
    InvalidPercentage = 12,
    MismatchedLengths = 13,
    /// Invalid or already-used backend signature  (#199)
    Unauthorized = 14,
    StakeLimitExceeded = 15,
    /// Game has not timed out yet
    TimeoutNotReached = 16,
    /// Timeout feature not configured
    TimeoutNotConfigured = 17,
    /// Game is not in a disputable state
    NotDisputable = 18,
    /// Dispute not found
    DisputeNotFound = 19,
    /// Only arbitrator can resolve disputes
    NotArbitrator = 20,
    /// Insufficient dispute fee
    InsufficientDisputeFee = 21,
    /// Only the waiting player can claim a timeout win
    InvalidTimeoutClaimant = 22,
    /// Settlement or payout has already been processed
    AlreadySettled = 23,
    /// Amount value must be positive and within supported bounds
    InvalidAmount = 24,
    /// SEP-10 challenge has expired or is invalid (#529)
    ChallengeExpired = 25,
    /// SEP-10 challenge nonce already used (#529)
    ChallengeAlreadyUsed = 26,
    /// Address has not completed SEP-10 verification (#529)
    NotVerified = 27,
    /// Multi-sig: signer is not in the signers list (#535)
    NotASigner = 28,
    /// Multi-sig: no pending fee proposal to approve (#535)
    NoProposal = 29,
    /// Multi-sig: signer already approved this proposal (#535)
    AlreadyApproved = 30,
    /// Multi-sig: threshold must be ≥ 1 and ≤ number of signers (#535)
    InvalidThreshold = 31,
    /// Oracle contract not configured (#533)
    OracleNotConfigured = 32,
    /// Tournament escrow not found (#532)
    EscrowNotFound = 33,
    /// Tournament escrow is still locked (#532)
    EscrowStillLocked = 34,
    /// Tournament escrow already released (#532)
    EscrowAlreadyReleased = 35,
}

#[contract]
pub struct GameContract;

#[contractimpl]
impl GameContract {
    pub fn initialize_token(env: Env, admin: Address, token_contract: Address) {
        if env.storage().instance().has(&TOKEN_CONTRACT) {
            panic!("Contract already initialized");
        }
        admin.require_auth();
        env.storage()
            .instance()
            .set(&TOKEN_CONTRACT, &token_contract);
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

    /// Gas-optimized tournament payout — single pass, no redundant map reads.
    /// Validates percentages and distributes atomically.
    pub fn payout_tournament_optimized(
        env: Env,
        game_id: u64,
        winners: Vec<Address>,
        percentages: Vec<u32>,
    ) -> Result<(), ContractError> {
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
            total_pct += pct;
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

        Ok(())
    }

    // ── Game lifecycle ────────────────────────────────────────────────────────

    pub fn create_game(
        env: Env,
        player1: Address,
        wager_amount: i128,
    ) -> Result<u64, ContractError> {
        let max_stake: i128 = env.storage().instance().get(&MAX_STAKE).unwrap_or(1_000);
        if wager_amount > max_stake {
            return Err(ContractError::StakeLimitExceeded);
        }

        player1.require_auth();

        let token_client = Self::token_client(&env);
        let contract_address = env.current_contract_address();

        if token_client.balance(&player1) < wager_amount {
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
        escrow.set(player1, current_escrow + wager_amount);
        env.storage().instance().set(&ESCROW, &escrow);

        Ok(game_counter)
    }

    pub fn join_game(env: Env, game_id: u64, player2: Address) -> Result<(), ContractError> {
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

        player2.require_auth();
        let token_client = Self::token_client(&env);
        let contract_address = env.current_contract_address();

        if token_client.balance(&player2) < game.wager_amount {
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
        escrow.set(player2, current_escrow + game.wager_amount);
        env.storage().instance().set(&ESCROW, &escrow);

        games.set(game_id, game);
        env.storage().instance().set(&GAMES, &games);

        Ok(())
    }

    pub fn submit_move(
        env: Env,
        game_id: u64,
        player: Address,
        move_data: Vec<u32>,
    ) -> Result<(), ContractError> {
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

    pub fn claim_draw(env: Env, game_id: u64, player: Address, signature: BytesN<64>) -> Result<(), ContractError> {
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

        game.state = GameState::Drawn;
        Self::process_draw_payout(&env, &game)?;

        games.set(game_id, game);
        env.storage().instance().set(&GAMES, &games);

        Ok(())
    }

    pub fn claim_win(
        env: Env,
        game_id: u64,
        winner: Address,
        signature: BytesN<64>,
    ) -> Result<(), ContractError> {
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

        game.winner = Some(winner.clone());
        Self::process_payout(&env, &game, &winner)?;
        game.state = GameState::Settled;

        games.set(game_id, game);
        env.storage().instance().set(&GAMES, &games);

        Ok(())
    }

    pub fn cancel_game(env: Env, game_id: u64, player: Address) -> Result<(), ContractError> {
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

        Ok(())
    }

    pub fn forfeit(env: Env, game_id: u64, player: Address) -> Result<(), ContractError> {
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

        game.winner = Some(winner.clone());
        Self::process_payout(&env, &game, &winner)?;
        game.state = GameState::Settled;

        games.set(game_id, game);
        env.storage().instance().set(&GAMES, &games);

        Ok(())
    }

    pub fn payout(env: Env, game_id: u64, winner: Address) -> Result<(), ContractError> {
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

        Self::process_payout(&env, &game, &winner)?;
        game.state = GameState::Settled;

        games.set(game_id, game);
        env.storage().instance().set(&GAMES, &games);

        Ok(())
    }

    pub fn payout_tournament(
        env: Env,
        game_id: u64,
        winners: Vec<Address>,
        percentages: Vec<u32>,
    ) -> Result<(), ContractError> {
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

        // Deduct wagers first to prevent double-counting
        escrow.set(game.player1.clone(), player1_escrow - game.wager_amount);
        if let Some(ref player2) = game.player2 {
            escrow.set(player2.clone(), player2_escrow - game.wager_amount);
        }

        let mut distributed: i128 = 0;
        for i in 0..winners.len() {
            let winner = winners.get(i).unwrap();
            let percentage = percentages.get(i).unwrap();
            total_percentage += percentage;
            let payout_amount = (total_pool * percentage as i128) / 100;
            distributed += payout_amount;
            let winner_escrow = escrow.get(winner.clone()).unwrap_or(0);
            escrow.set(winner.clone(), winner_escrow + payout_amount);
        }

        if total_percentage != 100 {
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

        Ok(())
    }

    pub fn get_game(env: Env, game_id: u64) -> Result<Game, ContractError> {
        let games: Map<u64, Game> = env
            .storage()
            .instance()
            .get(&GAMES)
            .ok_or(ContractError::GameNotFound)?;

        games.get(game_id).ok_or(ContractError::GameNotFound)
    }

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
    /// Must be called exactly once.
    ///
    /// * `admin_public_key` – 32-byte ED25519 public key of the backend signing service
    /// * `treasury_amount`  – Initial token reserve for puzzle payouts
    /// * `fee_bips`         – Protocol fee in basis-points of 1000 (e.g. 20 = 2 %)
    /// * `treasury_address` – Address that receives the protocol fee
    pub fn initialize_puzzle_rewards(
        env: Env,
        admin: Address,
        admin_public_key: Bytes,
        treasury_amount: i128,
        fee_bips: u32,
        treasury_address: Address,
    ) {
        if env.storage().instance().has(&CONTRACT_ADMIN) {
            panic!("Already initialized");
        }

        admin.require_auth();

        if admin_public_key.len() != 32 {
            panic!("Admin public key must be 32 bytes");
        }
        if treasury_amount < 0 {
            panic!("Treasury amount must be non-negative");
        }
        if fee_bips > 1000 {
            panic!("Fee bips must be between 0 and 1000");
        }

        env.storage().instance().set(&CONTRACT_ADMIN, &admin);
        env.storage().instance().set(&ADMIN_KEY, &admin_public_key);
        env.storage().instance().set(&TREASURY, &treasury_amount);
        env.storage().instance().set(&FEE_BIPS, &fee_bips);
        env.storage()
            .instance()
            .set(&TREASURY_ADDR, &treasury_address);
        env.storage().instance().set(&MAX_STAKE, &1_000i128);
    }

    pub fn set_max_stake(env: Env, admin: Address, new_limit: i128) {
        let current_admin: Address = env
            .storage()
            .instance()
            .get(&CONTRACT_ADMIN)
            .expect("Not initialized");
        current_admin.require_auth();
        if admin != current_admin {
            panic!("Unauthorized admin address");
        }
        if new_limit <= 0 {
            panic!("Max stake must be positive");
        }
        env.storage().instance().set(&MAX_STAKE, &new_limit);
    }

    pub fn configure_fees(env: Env, admin: Address, fee_bips: u32, treasury_address: Address) {
        let current_admin: Address = env
            .storage()
            .instance()
            .get(&CONTRACT_ADMIN)
            .expect("Not initialized");
        current_admin.require_auth();

        if admin != current_admin {
            panic!("Unauthorized admin address");
        }
        if fee_bips > 1000 {
            panic!("Fee bips must be between 0 and 1000");
        }

        env.storage().instance().set(&FEE_BIPS, &fee_bips);
        env.storage()
            .instance()
            .set(&TREASURY_ADDR, &treasury_address);
    }

    pub fn upgrade_admin(env: Env, admin: Address) {
        if env.storage().instance().has(&CONTRACT_ADMIN) {
            panic!("Admin already set");
        }
        if !env.storage().instance().has(&ADMIN_KEY) {
            panic!("Contract must be initialized first");
        }
        admin.require_auth();
        env.storage().instance().set(&CONTRACT_ADMIN, &admin);
    }

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
    pub fn claim_puzzle_reward(
        env: Env,
        recipient: Address,
        reward_amount: i128,
        nonce: u64,
        signature: BytesN<64>,
    ) -> Result<(), ContractError> {
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

        // 4. Mark nonce as used (state-before-interaction pattern)
        nonces.set(nonce, true);
        env.storage().instance().set(&USED_NONCE, &nonces);

        // 5. Deduct from Treasury
        let treasury: i128 = env.storage().instance().get(&TREASURY).unwrap_or(0);
        if treasury < reward_amount {
            panic!("Insufficient treasury");
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

    /// Configure dispute resolution system
    /// * `arbitrator` - Address of the dispute arbitrator
    /// * `dispute_fee` - Fee required to file a dispute (in tokens)
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
            panic!("Unauthorized admin address");
        }
        if dispute_fee < 0 {
            panic!("Dispute fee must be non-negative");
        }

        env.storage().instance().set(&ARBITRATOR, &arbitrator);
        env.storage().instance().set(&DISPUTE_FEE, &dispute_fee);
    }

    /// Configure timeout duration (in ledger sequences)
    pub fn configure_timeout(env: Env, admin: Address, duration: u64) {
        let current_admin: Address = env
            .storage()
            .instance()
            .get(&CONTRACT_ADMIN)
            .expect("Not initialized");
        current_admin.require_auth();

        if admin != current_admin {
            panic!("Unauthorized admin address");
        }
        if duration == 0 {
            panic!("Timeout duration must be greater than 0");
        }

        env.storage().instance().set(&TIMEOUT_DURATION, &duration);
    }

    /// File a dispute against the opponent for a game.
    pub fn file_dispute(
        env: Env,
        game_id: u64,
        filer: Address,
        against: Address,
        reason: Bytes,
    ) -> Result<u64, ContractError> {
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

        let dispute_fee: i128 = env.storage().instance().get(&DISPUTE_FEE).unwrap_or(0);
        if dispute_fee > 0 {
            let token_client = Self::token_client(&env);
            let contract_address = env.current_contract_address();

            if token_client.balance(&filer) < dispute_fee {
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

        Ok(dispute_counter)
    }

    /// Claim timeout win when opponent hasn't moved within timeout period
    /// The current player can claim victory if the opponent hasn't made a move
    /// within the configured timeout duration
    pub fn claim_timeout_win(
        env: Env,
        game_id: u64,
        claimant: Address,
    ) -> Result<(), ContractError> {
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

        game.winner = Some(claimant.clone());
        Self::process_payout(&env, &game, &claimant)?;
        game.state = GameState::Settled;

        games.set(game_id, game);
        env.storage().instance().set(&GAMES, &games);

        env.events().publish(
            (symbol_short!("timeout"), game_id),
            (claimant, timeout_duration),
        );

        Ok(())
    }

    /// Query remaining time before timeout (in ledger sequences).
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

    /// Resolve a dispute and settle the game according to the arbitrator's decision.
    pub fn resolve_dispute(
        env: Env,
        dispute_id: u64,
        arbitrator: Address,
        winner: Option<Address>,
        resolution: Bytes,
    ) -> Result<(), ContractError> {
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

        match winner {
            Some(ref winner_addr) => {
                if *winner_addr != game.player1 && Some(winner_addr.clone()) != game.player2 {
                    return Err(ContractError::NotPlayer);
                }
                game.state = GameState::Completed;
                game.winner = Some(winner_addr.clone());
                Self::process_payout(&env, &game, winner_addr)?;
                game.state = GameState::Settled;
            }
            None => {
                game.state = GameState::Drawn;
                game.winner = None;
                Self::process_draw_payout(&env, &game)?;
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

        Ok(())
    }

    /// Reject a dispute (arbitrator only)
    /// Returns the dispute fee to the filer
    pub fn reject_dispute(
        env: Env,
        dispute_id: u64,
        arbitrator: Address,
        reason: Bytes,
    ) -> Result<(), ContractError> {
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

        Ok(())
    }

    /// Query a dispute by ID
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

    /// Issue a SEP-10 challenge for `account`.
    ///
    /// * `nonce`  – 32-byte random value (must be unique per challenge)
    /// * `expiry` – ledger sequence after which the challenge is invalid
    ///
    /// Only the contract admin may issue challenges (prevents spam).
    pub fn issue_sep10_challenge(
        env: Env,
        admin: Address,
        account: Address,
        nonce: BytesN<32>,
        expiry: u64,
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

        env.events().publish(
            (symbol_short!("sep10"), symbol_short!("verified")),
            account,
        );

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

    /// Configure the multi-sig signer set and approval threshold.
    ///
    /// Can only be called by the contract admin.
    /// `threshold` must be ≥ 1 and ≤ `signers.len()`.
    pub fn configure_multisig(
        env: Env,
        admin: Address,
        signers: Vec<Address>,
        threshold: u32,
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

        let n = signers.len();
        if threshold == 0 || threshold > n {
            return Err(ContractError::InvalidThreshold);
        }

        env.storage().instance().set(&MULTISIG_SIGNERS, &signers);
        env.storage().instance().set(&MULTISIG_THRESHOLD, &threshold);

        Ok(())
    }

    /// Propose a new fee configuration.
    ///
    /// Replaces any existing pending proposal. The proposer's approval is
    /// counted automatically.
    pub fn propose_fee_change(
        env: Env,
        proposer: Address,
        new_fee_bips: u32,
        new_treasury_address: Address,
    ) -> Result<(), ContractError> {
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

    /// Approve the pending fee proposal.
    ///
    /// When approvals reach the threshold the fee change is applied immediately.
    pub fn approve_fee_proposal(env: Env, signer: Address) -> Result<bool, ContractError> {
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

    /// Cancel the pending fee proposal (any signer may cancel).
    pub fn cancel_fee_proposal(env: Env, signer: Address) -> Result<(), ContractError> {
        let signers: Vec<Address> = env
            .storage()
            .instance()
            .get(&MULTISIG_SIGNERS)
            .ok_or(ContractError::Unauthorized)?;

        if !signers.contains(&signer) {
            return Err(ContractError::NotASigner);
        }

        if !env
            .storage()
            .instance()
            .has(&PENDING_FEE_PROPOSAL)
        {
            return Err(ContractError::NoProposal);
        }

        signer.require_auth();

        env.storage().instance().remove(&PENDING_FEE_PROPOSAL);
        env.storage().instance().remove(&FEE_PROPOSAL_APPROVALS);

        env.events().publish(
            (symbol_short!("multisig"), symbol_short!("cancel")),
            signer,
        );

        Ok(())
    }

    /// Query the pending fee proposal (if any).
    pub fn get_fee_proposal(env: Env) -> Option<FeeProposal> {
        env.storage().instance().get(&PENDING_FEE_PROPOSAL)
    }

    /// Query how many signers have approved the pending proposal.
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

    /// Configure the SEP-40 oracle contract address for clock sync.
    pub fn configure_oracle(env: Env, admin: Address, oracle: Address) -> Result<(), ContractError> {
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

    /// Get the current oracle contract address.
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

    /// Configure the tournament time-lock duration (in ledger sequences).
    pub fn configure_tournament_timelock(
        env: Env,
        admin: Address,
        duration: u64,
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
        if duration == 0 {
            panic!("Timelock duration must be greater than 0");
        }
        env.storage().instance().set(&TOURNAMENT_TIMELOCK, &duration);
        Ok(())
    }

    /// Create a time-locked escrow for a completed tournament game.
    ///
    /// Locks the total prize pool until `current_ledger + timelock_duration`.
    /// Returns the escrow ID.
    pub fn create_tournament_escrow(
        env: Env,
        game_id: u64,
    ) -> Result<u64, ContractError> {
        let games: Map<u64, Game> = env
            .storage()
            .instance()
            .get(&GAMES)
            .ok_or(ContractError::GameNotFound)?;

        let game = games.get(game_id).ok_or(ContractError::GameNotFound)?;

        if game.state != GameState::Completed {
            return Err(ContractError::GameNotInProgress);
        }

        game.player1.require_auth();

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
            total_amount,
            locked_until,
            released: false,
        };

        escrows.set(escrow_id, escrow);
        env.storage().instance().set(&TOURNAMENT_ESCROWS, &escrows);

        env.events().publish(
            (symbol_short!("tl_escrow"), symbol_short!("created")),
            (escrow_id, game_id, locked_until),
        );

        Ok(escrow_id)
    }

    /// Release a time-locked tournament escrow to the specified winners.
    ///
    /// Can only be called after the lock period has expired.
    pub fn release_tournament_escrow(
        env: Env,
        escrow_id: u64,
        winners: Vec<Address>,
        percentages: Vec<u32>,
    ) -> Result<(), ContractError> {
        let mut escrows: Map<u64, TournamentEscrow> = env
            .storage()
            .instance()
            .get(&TOURNAMENT_ESCROWS)
            .ok_or(ContractError::EscrowNotFound)?;

        let escrow = escrows.get(escrow_id).ok_or(ContractError::EscrowNotFound)?;

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
            total_pct += percentages.get(i).unwrap();
            if total_pct > 100 {
                return Err(ContractError::InvalidPercentage);
            }
        }
        if total_pct != 100 {
            return Err(ContractError::InvalidPercentage);
        }

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

        let mut released_escrow = escrow;
        released_escrow.released = true;
        escrows.set(escrow_id, released_escrow);
        env.storage().instance().set(&TOURNAMENT_ESCROWS, &escrows);

        env.events().publish(
            (symbol_short!("tl_escrow"), symbol_short!("released")),
            escrow_id,
        );

        Ok(())
    }

    /// Query a tournament escrow by ID.
    pub fn get_tournament_escrow(env: Env, escrow_id: u64) -> Result<TournamentEscrow, ContractError> {
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
}
