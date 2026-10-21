import { Buffer } from "buffer";
import { Address } from "@stellar/stellar-sdk";
import {
  AssembledTransaction,
  Client as ContractClient,
  ClientOptions as ContractClientOptions,
  MethodOptions,
  Result,
  Spec as ContractSpec,
} from "@stellar/stellar-sdk/contract";
import type {
  u32,
  i32,
  u64,
  i64,
  u128,
  i128,
  u256,
  i256,
  Option,
  Timepoint,
  Duration,
} from "@stellar/stellar-sdk/contract";
export * from "@stellar/stellar-sdk";
export * as contract from "@stellar/stellar-sdk/contract";
export * as rpc from "@stellar/stellar-sdk/rpc";

if (typeof window !== "undefined") {
  //@ts-ignore Buffer exists
  window.Buffer = window.Buffer || Buffer;
}





export interface Game {
  board_fen: Buffer;
  created_at: u64;
  current_turn: u32;
  id: u64;
  /**
 * Ledger *timestamp* (Unix seconds) of the last move, or of `join_game`
 * if no move has been made yet. Used by `claim_timeout_victory` (SC-50)
 * for strict wall-clock deadline validation, distinct from
 * `last_move_at` which is ledger-*sequence*-based and backs the older
 * `claim_timeout_win` mechanism.
 */
last_activity_ts: u64;
  last_move_at: u64;
  moves: Array<ChessMove>;
  player1: string;
  player2: Option<string>;
  proof_of_game: Buffer;
  state: GameState;
  wager_amount: i128;
  winner: Option<string>;
}


/**
 * A single backend-signed puzzle-reward claim, as accepted by
 * `claim_puzzle_reward` / `claim_puzzle_rewards_batch`.
 */
export interface Proof {
  nonce: u64;
  recipient: string;
  reward_amount: i128;
  signature: Buffer;
}


export interface Trophy {
  earned_at: u64;
  metadata_uri: string;
  owner: string;
  trophy_id: u64;
  trophy_type: TrophyType;
}


export interface Dispute {
  against: string;
  filed_at: u64;
  filer: string;
  game_id: u64;
  id: u64;
  reason: Buffer;
  resolution: Option<Buffer>;
  status: DisputeStatus;
}

export type VipTier = {tag: "None", values: void} | {tag: "Bronze", values: void} | {tag: "Silver", values: void} | {tag: "Gold", values: void} | {tag: "Diamond", values: void};


export interface ChessMove {
  move_data: Array<u32>;
  player: string;
  timestamp: u64;
}

export type GameState = {tag: "Created", values: void} | {tag: "InProgress", values: void} | {tag: "Completed", values: void} | {tag: "Escrowed", values: void} | {tag: "Settled", values: void} | {tag: "Drawn", values: void} | {tag: "Forfeited", values: void};


export interface VipConfig {
  bronze_threshold: i128;
  diamond_threshold: i128;
  fee_discount_bronze: u32;
  fee_discount_diamond: u32;
  fee_discount_gold: u32;
  fee_discount_silver: u32;
  gold_threshold: i128;
  silver_threshold: i128;
}


export interface VipHolder {
  address: string;
  expires_at: Option<u64>;
  joined_at: u64;
  tier: VipTier;
  token_balance: i128;
}

export type TrophyType = {tag: "FirstWin", values: void} | {tag: "WinStreak", values: readonly [u32]} | {tag: "PuzzleMaster", values: void} | {tag: "TournamentChampion", values: void} | {tag: "RatingMilestone", values: readonly [i32]} | {tag: "Custom", values: readonly [Buffer]};


export interface DecayConfig {
  ban_threshold: i128;
  base_decay_rate_bips: u32;
  max_decay_duration: u64;
  min_penalty_threshold: i128;
}


export interface FeeProposal {
  new_fee_bips: u32;
  new_treasury_address: string;
  proposed_at: u64;
  proposer: string;
}


export interface PlayerRating {
  address: string;
  draws: u32;
  games_played: u32;
  highest_rating: i32;
  last_updated: u64;
  losses: u32;
  rating: i32;
  wins: u32;
}


export interface PuzzleBounty {
  bounty_id: u64;
  claimed: boolean;
  creator: string;
  deadline: u64;
  max_winners: u32;
  puzzle_id: u64;
  reward_amount: i128;
  total_submissions: u32;
  winner: Option<string>;
}

export type DisputeStatus = {tag: "Pending", values: void} | {tag: "Resolved", values: void} | {tag: "Rejected", values: void};


export interface PenaltyRecord {
  decay_rate_bips: u32;
  expires_at: u64;
  is_banned: boolean;
  issued_at: u64;
  original_amount: i128;
  penalty_amount: i128;
  player: string;
  reason: Buffer;
}


export interface TrophyMetadata {
  description: string;
  image_uri: string;
  name: string;
  total_minted: u32;
  trophy_id: u64;
  trophy_type: TrophyType;
}


export interface BountySubmission {
  puzzle_id: u64;
  solution_hash: Buffer;
  solver: string;
  submitted_at: u64;
}


export interface TournamentEscrow {
  escrow_id: u64;
  game_id: u64;
  locked_until: u64;
  player: string;
  released: boolean;
  total_amount: i128;
}

/**
 * Centralized error codes for the KnightVerse game contract.
 * 
 * All contract functions return structured errors instead of panicking with
 * string messages. This makes frontend error handling and debugging
 * deterministic — each variant maps to a distinct `u32` value that the SDK
 * exposes via `SCError::Contract`.
 */
export const ContractError = {
  1: {message:"GameNotFound"},
  2: {message:"NotYourTurn"},
  3: {message:"GameNotInProgress"},
  4: {message:"InvalidMove"},
  5: {message:"InsufficientFunds"},
  6: {message:"AlreadyJoined"},
  7: {message:"GameFull"},
  8: {message:"NotPlayer"},
  9: {message:"GameAlreadyCompleted"},
  12: {message:"InvalidPercentage"},
  13: {message:"MismatchedLengths"},
  /**
   * Invalid or already-used backend signature  (#199)
   */
  14: {message:"Unauthorized"},
  15: {message:"StakeLimitExceeded"},
  /**
   * Game has not timed out yet
   */
  16: {message:"TimeoutNotReached"},
  /**
   * Timeout feature not configured
   */
  17: {message:"TimeoutNotConfigured"},
  /**
   * Game is not in a disputable state
   */
  18: {message:"NotDisputable"},
  /**
   * Dispute not found
   */
  19: {message:"DisputeNotFound"},
  /**
   * Only arbitrator can resolve disputes
   */
  20: {message:"NotArbitrator"},
  /**
   * Insufficient dispute fee
   */
  21: {message:"InsufficientDisputeFee"},
  /**
   * Only the waiting player can claim a timeout win
   */
  22: {message:"InvalidTimeoutClaimant"},
  /**
   * Settlement or payout has already been processed
   */
  23: {message:"AlreadySettled"},
  /**
   * Amount value must be positive and within supported bounds
   */
  24: {message:"InvalidAmount"},
  /**
   * SEP-10 challenge has expired or is invalid (#529)
   */
  25: {message:"ChallengeExpired"},
  /**
   * SEP-10 challenge nonce already used (#529)
   */
  26: {message:"ChallengeAlreadyUsed"},
  /**
   * Multi-sig: signer is not in the signers list (#535)
   */
  28: {message:"NotASigner"},
  /**
   * Multi-sig: no pending fee proposal to approve (#535)
   */
  29: {message:"NoProposal"},
  /**
   * Multi-sig: signer already approved this proposal (#535)
   */
  30: {message:"AlreadyApproved"},
  /**
   * Multi-sig: threshold must be ≥ 1 and ≤ number of signers (#535)
   */
  31: {message:"InvalidThreshold"},
  /**
   * Oracle contract not configured (#533)
   */
  32: {message:"OracleNotConfigured"},
  /**
   * Tournament escrow not found (#532)
   */
  33: {message:"EscrowNotFound"},
  /**
   * Tournament escrow is still locked (#532)
   */
  34: {message:"EscrowStillLocked"},
  /**
   * Tournament escrow already released (#532)
   */
  35: {message:"EscrowAlreadyReleased"},
  /**
   * Total prize pool would exceed the configured limit
   */
  36: {message:"PrizePoolLimitExceeded"},
  /**
   * claim_puzzle_rewards_batch called with an empty proof list
   */
  37: {message:"EmptyBatch"},
  /**
   * claim_puzzle_rewards_batch called with more proofs than MAX_BATCH_SIZE
   */
  38: {message:"BatchTooLarge"},
  /**
   * Contract is paused for emergency halt (SC-11)
   */
  39: {message:"ContractPaused"},
  /**
   * Contract (or a sub-module) has already been initialized
   */
  40: {message:"AlreadyInitialized"},
  /**
   * Contract has not been initialized yet
   */
  41: {message:"NotInitialized"},
  /**
   * Contract admin address has already been set
   */
  42: {message:"AdminAlreadySet"},
  /**
   * Caller is not the contract admin
   */
  43: {message:"NotAdmin"},
  /**
   * Contract is already paused
   */
  44: {message:"AlreadyPaused"},
  /**
   * Contract is not currently paused
   */
  45: {message:"NotPaused"},
  /**
   * One or more configuration parameters are invalid
   */
  46: {message:"InvalidConfig"},
  /**
   * Treasury has insufficient funds to cover the payout
   */
  47: {message:"InsufficientTreasury"},
  /**
   * Token address is not in the admin-maintained whitelist (SC-17)
   */
  48: {message:"TokenNotWhitelisted"},
  /**
   * Player has reached the maximum number of active tournament escrows (SC-20)
   */
  49: {message:"MaxActiveEscrowsExceeded"},
  /**
   * Reentrant call detected (#860)
   */
  50: {message:"ReentrantCall"},
  /**
   * Circuit breaker vote not found
   */
  51: {message:"CircuitBreakerVoteNotFound"},
  /**
   * Admin key rotation already has a pending proposal (#890)
   */
  52: {message:"AdminKeyAlreadyPending"},
  /**
   * Stored admin public key is not a 32-byte ED25519 key.
   */
  53: {message:"InvalidAdminKey"}
}

export interface Client {
  /**
   * Construct and simulate a pause transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Pause the contract — blocks all state-mutating operations.
   * Only the contract admin may call this.
   */
  pause: ({caller}: {caller: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a payout transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Transfer the prize to the pre-recorded winner of a `Completed` game.
   * 
   * This entrypoint is used when the game outcome has been recorded
   * off-chain and `game.winner` has been set (e.g. after `claim_win`
   * advanced the state to `Completed`). The winner calls this to pull
   * their funds.
   * 
   * # Parameters
   * - `game_id`  — ID of a game in `Completed` state with a recorded winner.
   * - `winner`   — Must match `game.winner`; must authorise the call.
   * 
   * # Returns
   * `Ok(())` on success.
   * 
   * # Errors
   * - [`ContractError::GameNotFound`]       — `game_id` does not exist.
   * - [`ContractError::AlreadySettled`]     — Game is already `Settled`.
   * - [`ContractError::GameNotInProgress`]  — Game is not in `Completed` state.
   * - [`ContractError::NotPlayer`]          — `winner` does not match `game.winner`.
   */
  payout: ({game_id, winner}: {game_id: u64, winner: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a forfeit transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Forfeit the game, awarding the full prize pool to the opponent.
   * 
   * Either player may forfeit an `InProgress` game at any time. The
   * opponent receives the net payout (total pool minus protocol fee) and
   * the game is marked `Settled`.
   * 
   * # Parameters
   * - `game_id`  — ID of a game in `InProgress` state.
   * - `player`   — Forfeiting player's address; must be a participant and
   * must authorise the call.
   * 
   * # Returns
   * `Ok(())` on success.
   * 
   * # Errors
   * - [`ContractError::GameNotFound`]       — `game_id` does not exist.
   * - [`ContractError::GameNotInProgress`]  — Game is not `InProgress`.
   * - [`ContractError::NotPlayer`]          — `player` is not a participant.
   * - [`ContractError::GameFull`]           — No `player2` found (single-player game; should not occur).
   */
  forfeit: ({game_id, player}: {game_id: u64, player: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a unpause transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Unpause the contract — resumes normal operations.
   * Only the contract admin may call this.
   */
  unpause: ({caller}: {caller: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a upgrade transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Upgrade this contract's WASM. Restricted to the ADMIN_KEY holder.
   * 
   * The caller must provide an ED25519 signature (from the backend signing
   * service) over `SHA256(wasm_hash)`.
   */
  upgrade: ({wasm_hash, signature}: {wasm_hash: Buffer, signature: Buffer}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a get_game transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Fetch a single game by ID.
   * 
   * # Parameters
   * - `game_id` — Numeric game identifier returned by [`create_game`].
   * 
   * # Returns
   * `Ok(Game)` — full [`Game`] struct including state, players, moves, and wager.
   * 
   * # Errors
   * - [`ContractError::GameNotFound`] — `game_id` does not exist.
   */
  get_game: ({game_id}: {game_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<Game>>>

  /**
   * Construct and simulate a claim_win transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Claim a win result, verified by the backend signing service.
   * 
   * Verifies an ED25519 signature over `SHA256(game_id_le8 || winner_address_bytes)`,
   * pays out the net prize (total pool minus protocol fee, if configured) to
   * `winner`, and marks the game `Settled`.
   * 
   * # Signature Payload
   * `SHA256( game_id.to_le_bytes() || winner_address_string_bytes )`
   * 
   * # Parameters
   * - `game_id`    — ID of a game in `InProgress` state.
   * - `winner`     — Address of the winning player; must be a participant and
   * must authorise the call.
   * - `signature`  — 64-byte ED25519 signature from the backend service.
   * 
   * # Returns
   * `Ok(())` on success.
   * 
   * # Errors
   * - [`ContractError::GameNotFound`]       — `game_id` does not exist.
   * - [`ContractError::GameNotInProgress`]  — Game is not `InProgress`.
   * - [`ContractError::NotPlayer`]          — `winner` is not a participant.
   * 
   * # Panics
   * - If the ED25519 signature is invalid.
   * - If the contract has not been initialized.
   */
  claim_win: ({game_id, winner, signature}: {game_id: u64, winner: string, signature: Buffer}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a is_paused transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns `true` if the contract is currently paused.
   */
  is_paused: (options?: MethodOptions) => Promise<AssembledTransaction<boolean>>

  /**
   * Construct and simulate a join_game transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Join an existing game as the second player and lock matching stake.
   * 
   * Transfers `game.wager_amount` tokens from `player2` to the contract,
   * sets `game.player2`, and advances the game to `InProgress` state with
   * `current_turn = 1` (player1 moves first).
   * 
   * # Parameters
   * - `game_id`  — ID of a game in `Created` state.
   * - `player2`  — Address of the joining player; must differ from `player1`
   * and must authorise the call.
   * 
   * # Returns
   * `Ok(())` on success.
   * 
   * # Errors
   * - [`ContractError::GameNotFound`]          — `game_id` does not exist.
   * - [`ContractError::GameAlreadyCompleted`]  — Game is not in `Created` state.
   * - [`ContractError::GameFull`]              — A second player has already joined.
   * - [`ContractError::AlreadyJoined`]         — `player2` is the same as `player1`.
   * - [`ContractError::StakeLimitExceeded`]    — Wager exceeds `MAX_STAKE`.
   * - [`ContractError::PrizePoolLimitExceeded`] — Combined pool exceeds `MAX_PRIZE_POOL`.
   * - [`ContractError::InvalidAmount`]          — Integer overflow computing the pool.
   */
  join_game: ({game_id, player2}: {game_id: u64, player2: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a claim_draw transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Claim a draw result, verified by the backend signing service.
   * 
   * Verifies an ED25519 signature over `SHA256(game_id_le8 || b"DRAW")`,
   * transitions the game to `Drawn`, and returns each player's stake in
   * full via [`process_draw_payout`].
   * 
   * The backend signature prevents either player from unilaterally claiming
   * a draw without mutual agreement or arbiter approval.
   * 
   * # Signature Payload
   * `SHA256( game_id.to_le_bytes() || b"DRAW" )`
   * 
   * # Parameters
   * - `game_id`    — ID of a game in `InProgress` state.
   * - `player`     — Either participant's address; must authorise the call.
   * - `signature`  — 64-byte ED25519 signature from the backend service.
   * 
   * # Returns
   * `Ok(())` on success.
   * 
   * # Errors
   * - [`ContractError::GameNotFound`]       — `game_id` does not exist.
   * - [`ContractError::GameNotInProgress`]  — Game is not `InProgress`.
   * - [`ContractError::NotPlayer`]          — `player` is not a participant.
   * 
   * # Panics
   * - If the ED25519 signature is invalid (Soroban `ed25519_verify` panics on failure).
   * - If the contract has not been in
   */
  claim_draw: ({game_id, player, signature}: {game_id: u64, player: string, signature: Buffer}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a get_oracle transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Return the configured SEP-40 oracle contract address.
   * 
   * # Returns
   * `Ok(Address)` — the oracle contract address.
   * 
   * # Errors
   * - [`ContractError::OracleNotConfigured`] — [`configure_oracle`] has not been called.
   */
  get_oracle: (options?: MethodOptions) => Promise<AssembledTransaction<Result<string>>>

  /**
   * Construct and simulate a cancel_game transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Cancel a game that has not yet been joined and refund the creator.
   * 
   * Only `player1` (the creator) may cancel, and only while the game is in
   * `Created` state (before `player2` joins). The staked wager is returned
   * to `player1` and the game is marked `Completed` to prevent future joins.
   * 
   * # Parameters
   * - `game_id`  — ID of a game in `Created` state.
   * - `player`   — Must equal `game.player1`; must authorise the call.
   * 
   * # Returns
   * `Ok(())` on success.
   * 
   * # Errors
   * - [`ContractError::GameNotFound`]         — `game_id` does not exist.
   * - [`ContractError::GameAlreadyCompleted`] — Game has already progressed past `Created`.
   * - [`ContractError::NotPlayer`]            — `player` is not `player1`.
   */
  cancel_game: ({game_id, player}: {game_id: u64, player: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a create_game transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Create a new wager game and lock `player1`'s stake into escrow.
   * 
   * Generates a sequential game ID, transfers `wager_amount` tokens from
   * `player1` to the contract, and stores the game in `Created` state
   * awaiting a second player.
   * 
   * # Parameters
   * - `player1`       — Address of the game creator; must authorise the call.
   * - `wager_amount`  — Token amount to stake; must be > 0, ≤ `MAX_STAKE`, and
   * `wager_amount * 2` must be ≤ `MAX_PRIZE_POOL`.
   * 
   * # Returns
   * `Ok(game_id)` — the unique `u64` identifier for the new game.
   * 
   * # Errors
   * - [`ContractError::StakeLimitExceeded`]    — `wager_amount > MAX_STAKE`.
   * - [`ContractError::PrizePoolLimitExceeded`] — `wager_amount * 2 > MAX_PRIZE_POOL`.
   * - [`ContractError::InvalidAmount`]          — Integer overflow computing the prize pool.
   * - [`ContractError::InsufficientFunds`]      — `player1`'s token balance < `wager_amount`.
   */
  create_game: ({player1, wager_amount, initial_board}: {player1: string, wager_amount: i128, initial_board: Buffer}, options?: MethodOptions) => Promise<AssembledTransaction<Result<u64>>>

  /**
   * Construct and simulate a get_dispute transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Fetch a dispute by ID.
   * 
   * # Parameters
   * - `dispute_id` — Numeric ID returned by [`file_dispute`].
   * 
   * # Returns
   * `Ok(Dispute)` — the full [`Dispute`] struct including status and resolution.
   * 
   * # Errors
   * - [`ContractError::DisputeNotFound`] — `dispute_id` does not exist or
   * no disputes have been filed yet.
   */
  get_dispute: ({dispute_id}: {dispute_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<Dispute>>>

  /**
   * Construct and simulate a submit_move transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Record a chess move for the active player and advance the turn.
   * 
   * Appends a [`ChessMove`] entry to `game.moves`, flips `current_turn`
   * (1 → 2 → 1 …), and updates `last_move_at` to the current ledger
   * sequence (used by the timeout mechanism).
   * 
   * # Parameters
   * - `game_id`    — ID of a game in `InProgress` state.
   * - `player`     — Address of the moving player; must be `player1` or `player2`
   * and must authorise the call.
   * - `move_data`  — Encoded move payload (e.g. from/to squares as `u32`s);
   * must be non-empty.
   * 
   * # Returns
   * `Ok(())` on success.
   * 
   * # Errors
   * - [`ContractError::GameNotFound`]       — `game_id` does not exist.
   * - [`ContractError::GameNotInProgress`]  — Game is not in `InProgress` state.
   * - [`ContractError::NotPlayer`]          — `player` is not a participant.
   * - [`ContractError::NotYourTurn`]        — It is not `player`'s turn.
   * - [`ContractError::InvalidMove`]        — `move_data` is empty.
   */
  submit_move: ({game_id, player, move_data, new_board, game_status}: {game_id: u64, player: string, move_data: Array<u32>, new_board: Buffer, game_status: u32}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a file_dispute transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * File a dispute against an opponent for an in-progress game.
   * 
   * Transfers the configured `dispute_fee` (if non-zero) from `filer` to
   * the contract, creates a [`Dispute`] in `Pending` status, and emits a
   * `("dispute", "filed")` event. The fee is held until the dispute is
   * resolved or rejected.
   * 
   * # Parameters
   * - `game_id`   — ID of a game in `InProgress` state.
   * - `filer`     — Player raising the dispute; must be a game participant
   * and must authorise the call.
   * - `against`   — The opposing player; must also be a participant and
   * must differ from `filer`.
   * - `reason`    — Encoded reason bytes (arbitrary off-chain payload).
   * 
   * # Returns
   * `Ok(dispute_id)` — unique `u64` identifier for the new dispute.
   * 
   * # Errors
   * - [`ContractError::GameNotFound`]            — `game_id` does not exist.
   * - [`ContractError::NotDisputable`]           — Game is not `InProgress`.
   * - [`ContractError::NotPlayer`]               — `filer` or `against` is not a participant.
   * - [`ContractError::InvalidMove`]             — `filer` and `against` 
   */
  file_dispute: ({game_id, filer, against, reason}: {game_id: u64, filer: string, against: string, reason: Buffer}, options?: MethodOptions) => Promise<AssembledTransaction<Result<u64>>>

  /**
   * Construct and simulate a get_all_games transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Return every stored game as a map keyed by game ID.
   * 
   * Returns an empty map if no games have been created yet.
   * 
   * > **Note:** This is an unbounded read. Frontend callers should paginate
   * > using individual [`get_game`] calls once the game count grows large.
   * 
   * # Returns
   * `Map<u64, Game>` — all games indexed by their numeric ID.
   */
  get_all_games: (options?: MethodOptions) => Promise<AssembledTransaction<Map<u64, Game>>>

  /**
   * Construct and simulate a set_max_stake transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Update the per-game maximum wager (admin only).
   * 
   * Any subsequent [`create_game`] or [`join_game`] call with a
   * `wager_amount` exceeding this limit will return
   * [`ContractError::StakeLimitExceeded`].
   * 
   * # Parameters
   * - `admin`      — Must match `CONTRACT_ADMIN`; must authorise the call.
   * - `new_limit`  — New maximum wager; must be > 0.
   * 
   * # Panics
   * - If the contract is not initialized.
   * - If `admin` does not match the stored admin.
   * - If `new_limit <= 0`.
   */
  set_max_stake: ({admin, new_limit}: {admin: string, new_limit: i128}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a upgrade_admin transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Set the contract admin when none has been recorded yet (upgrade path).
   * 
   * This is a one-time migration helper for contracts initialised before
   * `CONTRACT_ADMIN` storage was introduced. It panics if an admin is
   * already set, or if the contract's `ADMIN_KEY` has not been seeded.
   * 
   * # Parameters
   * - `admin` — New administrator address; must authorise the call.
   * 
   * # Panics
   * - If `CONTRACT_ADMIN` is already set.
   * - If `ADMIN_KEY` is not set (contract not initialised).
   */
  upgrade_admin: ({admin}: {admin: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a configure_fees transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Update the protocol fee and treasury address (admin only, no multi-sig).
   * 
   * For fee changes requiring multi-sig approval, use the
   * [`propose_fee_change`] / [`approve_fee_proposal`] flow instead.
   * 
   * # Parameters
   * - `admin`             — Must match `CONTRACT_ADMIN`; must authorise the call.
   * - `fee_bips`          — New fee in basis-points of 1 000 (0–1 000).
   * - `treasury_address`  — New recipient address for the fee portion.
   * 
   * # Panics
   * - If the contract is not initialized.
   * - If `admin` does not match the stored admin.
   * - If `fee_bips > 1000`.
   */
  configure_fees: ({admin, fee_bips, treasury_address}: {admin: string, fee_bips: u32, treasury_address: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a reject_dispute transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Reject a pending dispute and refund the dispute fee to the filer (arbitrator only).
   * 
   * Marks the dispute `Rejected`, stores the arbitrator's reason, and
   * returns the dispute fee (if non-zero) to the original filer.
   * 
   * # Parameters
   * - `dispute_id`  — ID of a dispute in `Pending` status.
   * - `arbitrator`  — Must match the stored arbitrator; must authorise the call.
   * - `reason`      — Encoded rejection rationale (arbitrary off-chain payload).
   * 
   * # Returns
   * `Ok(())` on success.
   * 
   * # Errors
   * - [`ContractError::NotArbitrator`]        — `arbitrator` is not the configured arbitrator,
   * or the arbitrator has not been set.
   * - [`ContractError::DisputeNotFound`]      — `dispute_id` does not exist.
   * - [`ContractError::GameAlreadyCompleted`] — Dispute is not in `Pending` status.
   * 
   * # Events
   * Emits `("dispute", "reject") → (dispute_id, filer)`.
   */
  reject_dispute: ({dispute_id, arbitrator, reason}: {dispute_id: u64, arbitrator: string, reason: Buffer}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a reward_balance transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Query the puzzle-reward balance of an address.
   */
  reward_balance: ({address}: {address: string}, options?: MethodOptions) => Promise<AssembledTransaction<i128>>

  /**
   * Construct and simulate a get_oracle_time transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Get the oracle-synced timestamp (ledger sequence from oracle).
   * Falls back to the current ledger sequence if oracle is not configured.
   * The oracle is called via cross-contract invocation using the SEP-40
   * `lastprice` interface which returns a price record with a timestamp.
   * 
   * Note: This is a minimal implementation. In production, you would invoke
   * the oracle contract's `lastprice` method and extract the timestamp.
   */
  get_oracle_time: (options?: MethodOptions) => Promise<AssembledTransaction<u64>>

  /**
   * Construct and simulate a resolve_dispute transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Resolve a pending dispute and settle the underlying game (arbitrator only).
   * 
   * Authenticates the arbitrator, processes the payout (`Some(winner)` for a
   * win, `None` for a draw), marks the game `Settled`, and closes the
   * dispute as `Resolved`. Emits a `("dispute", "solved")` event.
   * 
   * # Parameters
   * - `dispute_id`   — ID of a dispute in `Pending` status.
   * - `arbitrator`   — Must match the stored arbitrator; must authorise the call.
   * - `winner`       — `Some(address)` to pay out a winner, or `None` to
   * split stakes equally (draw).
   * - `resolution`   — Encoded resolution rationale (arbitrary off-chain payload).
   * 
   * # Returns
   * `Ok(())` on success.
   * 
   * # Errors
   * - [`ContractError::NotArbitrator`]        — `arbitrator` is not the configured arbitrator,
   * or the arbitrator has not been set.
   * - [`ContractError::DisputeNotFound`]      — `dispute_id` does not exist.
   * - [`ContractError::GameAlreadyCompleted`] — Dispute is not `Pending`, or game is not `InProgress`.
   * - [`ContractError::GameNotFound`]         — Underlying game does
   */
  resolve_dispute: ({dispute_id, arbitrator, winner, resolution}: {dispute_id: u64, arbitrator: string, winner: Option<string>, resolution: Buffer}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a configure_oracle transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Set the SEP-40 oracle contract address used for clock synchronisation (admin only).
   * 
   * Once configured, [`get_oracle_time`] may call the oracle via
   * cross-contract invocation to obtain a trusted timestamp.
   * 
   * # Parameters
   * - `admin`   — Must match `CONTRACT_ADMIN`; must authorise the call.
   * - `oracle`  — Address of the SEP-40-compliant oracle contract.
   * 
   * # Returns
   * `Ok(())` on success.
   * 
   * # Errors
   * - [`ContractError::Unauthorized`] — `admin` does not match `CONTRACT_ADMIN`.
   * 
   * # Panics
   * - If the contract is not initialised.
   */
  configure_oracle: ({admin, oracle}: {admin: string, oracle: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a get_fee_proposal transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Return the pending fee proposal, if one exists.
   * 
   * # Returns
   * `Some(FeeProposal)` — the proposal with its `new_fee_bips`,
   * `new_treasury_address`, `proposed_at` ledger sequence, and `proposer`.
   * `None` — no proposal is currently pending.
   */
  get_fee_proposal: (options?: MethodOptions) => Promise<AssembledTransaction<Option<FeeProposal>>>

  /**
   * Construct and simulate a initialize_token transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Bind the SEP-41 token contract used for all wager escrow and prize transfers.
   * 
   * Must be called once before any game or reward function. Calling it a
   * second time panics with `"Contract already initialized"`.
   * 
   * # Parameters
   * - `admin` — Address that authorises the call (`require_auth` is enforced).
   * - `token_contract` — Address of the SEP-41 token contract (e.g. XLM or USDC).
   * 
   * # Panics
   * - If `TOKEN_CONTRACT` is already set in instance storage.
   */
  initialize_token: ({admin, token_contract}: {admin: string, token_contract: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a treasury_balance transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Query the current treasury reserve.
   */
  treasury_balance: (options?: MethodOptions) => Promise<AssembledTransaction<i128>>

  /**
   * Construct and simulate a claim_timeout_win transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Claim a win by timeout when the opponent has not moved within the allowed window.
   * 
   * Only the *waiting* player (the one who is not required to move) may
   * claim a timeout win. Once `current_ledger - game.last_move_at ≥
   * timeout_duration`, that player calls this to receive the net payout.
   * 
   * # Parameters
   * - `game_id`   — ID of a game in `InProgress` state.
   * - `claimant`  — The player who is waiting for the opponent's move;
   * must authorise the call.
   * 
   * # Returns
   * `Ok(())` on success; game is marked `Settled`.
   * 
   * # Errors
   * - [`ContractError::GameNotFound`]           — `game_id` does not exist.
   * - [`ContractError::GameNotInProgress`]      — Game is not `InProgress`.
   * - [`ContractError::NotPlayer`]              — `claimant` is not a participant.
   * - [`ContractError::InvalidTimeoutClaimant`] — `claimant` is the player whose turn it is (not the waiting player).
   * - [`ContractError::TimeoutNotConfigured`]   — [`configure_timeout`] has not been called.
   * - [`ContractError::TimeoutNotReached`]      — Elapsed ledgers < `timeo
   */
  claim_timeout_win: ({game_id, claimant}: {game_id: u64, claimant: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a configure_timeout transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Set the inactivity timeout used by [`claim_timeout_win`] (admin only).
   * 
   * The timeout is measured in ledger sequences (≈ 5 s each on Stellar
   * mainnet). A player may claim a timeout win once `current_ledger -
   * game.last_move_at ≥ timeout_duration`.
   * 
   * # Parameters
   * - `admin`     — Must match `CONTRACT_ADMIN`; must authorise the call.
   * - `duration`  — Ledger sequences of inactivity before a timeout win is
   * claimable; must be > 0.
   * 
   * # Panics
   * - If the contract is not initialised.
   * - If `admin` does not match the stored admin.
   * - If `duration == 0`.
   */
  configure_timeout: ({admin, duration}: {admin: string, duration: u64}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a is_sep10_verified transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns true if `account` has completed SEP-10 verification.
   */
  is_sep10_verified: ({account}: {account: string}, options?: MethodOptions) => Promise<AssembledTransaction<boolean>>

  /**
   * Construct and simulate a payout_tournament transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Distribute a completed game's prize pool across multiple tournament winners.
   * 
   * Deducts both players' escrow balances, then distributes the total pool
   * proportionally according to `percentages`. Integer-division dust goes to
   * the first winner. Requires `player1`'s authorisation as the tournament
   * organiser.
   * 
   * Prefer [`payout_tournament_optimized`] for lower gas cost; this variant
   * does separate escrow reads per winner.
   * 
   * # Parameters
   * - `game_id`      — ID of a game in `Completed` state.
   * - `winners`      — Ordered recipient addresses.
   * - `percentages`  — Whole-number percentages parallel to `winners`;
   * must sum to exactly 100.
   * 
   * # Returns
   * `Ok(())` on success.
   * 
   * # Errors
   * - [`ContractError::GameNotFound`]          — `game_id` does not exist.
   * - [`ContractError::GameNotInProgress`]     — Game is not `Completed`.
   * - [`ContractError::MismatchedLengths`]     — `winners` and `percentages` differ.
   * - [`ContractError::InsufficientFunds`]     — Escrow balance is lower than the wager.
   * - [`ContractError::InvalidPercentage`
   */
  payout_tournament: ({game_id, winners, percentages}: {game_id: u64, winners: Array<string>, percentages: Array<u32>}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a set_player_rating transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Manually set a player's rating (admin only).
   * 
   * This is useful for:
   * - Correcting erroneous ratings
   * - Resetting new accounts
   * - Administrative corrections
   */
  set_player_rating: ({admin, player, new_rating}: {admin: string, player: string, new_rating: i32}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a configure_multisig transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Configure the multi-sig signer set and approval threshold (admin only).
   * 
   * Replaces any existing signer configuration. Once set, fee changes
   * proposed via [`propose_fee_change`] require at least `threshold`
   * approvals from `signers` before taking effect.
   * 
   * # Parameters
   * - `admin`      — Must match `CONTRACT_ADMIN`; must authorise the call.
   * - `signers`    — Exhaustive list of addresses authorised to propose and
   * approve fee changes.
   * - `threshold`  — Minimum number of approvals required; must be ≥ 1 and
   * ≤ `signers.len()`.
   * 
   * # Returns
   * `Ok(())` on success.
   * 
   * # Errors
   * - [`ContractError::Unauthorized`]    — `admin` does not match `CONTRACT_ADMIN`.
   * - [`ContractError::InvalidThreshold`] — `threshold == 0` or `threshold > signers.len()`.
   * 
   * # Panics
   * - If the contract is not initialised.
   */
  configure_multisig: ({admin, signers, threshold}: {admin: string, signers: Array<string>, threshold: u32}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a get_approval_count transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Return the number of signers who have approved the pending proposal.
   * 
   * Returns `0` if no proposal is pending or no approvals have been recorded.
   */
  get_approval_count: (options?: MethodOptions) => Promise<AssembledTransaction<u32>>

  /**
   * Construct and simulate a get_player_profile transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Retrieve a player profile from persistent storage.
   * 
   * Returns None if player has no profile yet.
   */
  get_player_profile: ({player}: {player: string}, options?: MethodOptions) => Promise<AssembledTransaction<Option<PlayerRating>>>

  /**
   * Construct and simulate a propose_fee_change transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Propose a new fee configuration (any registered signer).
   * 
   * Replaces any existing pending proposal and resets the approval map.
   * The proposer's approval is counted automatically, so a single-signer
   * setup with `threshold = 1` will execute immediately.
   * 
   * # Parameters
   * - `proposer`              — Must be in the configured signer set; must authorise the call.
   * - `new_fee_bips`          — Proposed fee in basis-points of 1 000 (0–1 000).
   * - `new_treasury_address`  — Proposed recipient address for the fee portion.
   * 
   * # Returns
   * `Ok(())` on success.
   * 
   * # Errors
   * - [`ContractError::InvalidAmount`] — `new_fee_bips > 1000`.
   * - [`ContractError::NotASigner`]    — `proposer` is not in the signer set.
   * - [`ContractError::Unauthorized`]  — Multi-sig has not been configured yet.
   * 
   * # Events
   * Emits `("multisig", "proposed") → proposer`.
   */
  propose_fee_change: ({proposer, new_fee_bips, new_treasury_address}: {proposer: string, new_fee_bips: u32, new_treasury_address: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a set_max_prize_pool transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Update the maximum combined prize pool across both players (admin only).
   * 
   * Guards against very large pools. Any [`create_game`] or [`join_game`]
   * call where `wager_amount * 2 > MAX_PRIZE_POOL` will return
   * [`ContractError::PrizePoolLimitExceeded`].
   * 
   * # Parameters
   * - `admin`      — Must match `CONTRACT_ADMIN`; must authorise the call.
   * - `new_limit`  — New maximum prize pool; must be > 0.
   * 
   * # Panics
   * - If the contract is not initialized.
   * - If `admin` does not match the stored admin.
   * - If `new_limit <= 0`.
   */
  set_max_prize_pool: ({admin, new_limit}: {admin: string, new_limit: i128}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a cancel_fee_proposal transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Cancel the pending fee proposal (any registered signer).
   * 
   * Removes the proposal and clears all approvals. Any signer may cancel,
   * not only the original proposer.
   * 
   * # Parameters
   * - `signer` — Must be in the configured signer set; must authorise the call.
   * 
   * # Returns
   * `Ok(())` on success.
   * 
   * # Errors
   * - [`ContractError::NotASigner`]   — `signer` is not in the signer set.
   * - [`ContractError::Unauthorized`] — Multi-sig has not been configured.
   * - [`ContractError::NoProposal`]   — No pending proposal to cancel.
   * 
   * # Events
   * Emits `("multisig", "cancel") → signer`.
   */
  cancel_fee_proposal: ({signer}: {signer: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a claim_puzzle_reward transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Claim a single puzzle reward backed by a backend ED25519 signature.
   * 
   * Verifies the signature, guards against nonce replay, deducts
   * `reward_amount` from the treasury, and credits the recipient's
   * puzzle-reward balance. Emits a `pzl_rwd` event on success.
   * 
   * # Signature Payload
   * `SHA256( recipient_address_string_bytes || reward_amount_i64_le8 || nonce_u64_le8 )`
   * 
   * # Parameters
   * - `recipient`      — Address to credit; must be the one encoded in the signature.
   * - `reward_amount`  — Token amount to award; must be > 0 and ≤ `i64::MAX`.
   * - `nonce`          — Unique per-claim `u64` nonce; prevents replay attacks.
   * - `signature`      — 64-byte ED25519 signature from the backend service.
   * 
   * # Returns
   * `Ok(())` on success.
   * 
   * # Errors
   * - [`ContractError::InvalidAmount`]  — `reward_amount ≤ 0` or `> i64::MAX`.
   * - [`ContractError::Unauthorized`]   — `nonce` has already been used.
   * 
   * # Panics
   * - If the ED25519 signature is invalid.
   * - If `ADMIN_KEY` is absent (contract not initialised).
   * - If the treasury has insufficient balanc
   */
  claim_puzzle_reward: ({recipient, reward_amount, nonce, signature}: {recipient: string, reward_amount: i128, nonce: u64, signature: Buffer}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a accept_new_admin_key transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Accept a pending admin key proposal after the timelock expires.
   * Anyone can call this once the 24-hour window has elapsed.
   */
  accept_new_admin_key: (options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a approve_fee_proposal transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Approve the pending fee proposal (any registered signer).
   * 
   * Records the signer's approval. If the running approval count reaches
   * the configured threshold, the fee change is applied immediately and
   * the proposal is cleared.
   * 
   * # Parameters
   * - `signer` — Must be in the configured signer set; must authorise the call.
   * 
   * # Returns
   * `Ok(true)` — threshold reached, proposal executed and cleared.
   * `Ok(false)` — approval recorded, still awaiting more signers.
   * 
   * # Errors
   * - [`ContractError::NotASigner`]    — `signer` is not in the signer set.
   * - [`ContractError::Unauthorized`]  — Multi-sig has not been configured.
   * - [`ContractError::NoProposal`]    — No pending proposal exists.
   * - [`ContractError::AlreadyApproved`] — `signer` has already approved this proposal.
   * 
   * # Events
   * On partial approval: emits `("multisig", "approved") → signer`.
   * On execution: emits `("multisig", "executed") → new_fee_bips`.
   */
  approve_fee_proposal: ({signer}: {signer: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<boolean>>>

  /**
   * Construct and simulate a add_whitelisted_token transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Add a token address to the whitelist.
   * Authorised by the `admin` address — the contract admin once
   * `initialize_puzzle_rewards` has been called, or any authorised caller
   * before that.
   */
  add_whitelisted_token: ({admin, token}: {admin: string, token: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a claim_timeout_victory transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Claim victory when the opponent has failed to respond within the
   * configured deadline **plus** grace buffer (SC-50).
   * 
   * This is the strict, ledger-*timestamp*-based sibling of
   * [`claim_timeout_win`]. It exists so that a player who is still
   * actively engaged cannot be held hostage by an opponent who realizes
   * they're losing and simply goes offline instead of resigning: once
   * `env.ledger().timestamp() - game.last_activity_ts >= deadline_secs +
   * buffer_secs`, the waiting player may settle the game and receive the
   * full escrowed wager **immediately — no backend signature required**.
   * 
   * The `buffer_secs` grace period specifically covers a **temporary
   * backend/server outage**: even if the off-chain service that would
   * normally arbitrate a `claim_win`/`claim_draw` is briefly down, a
   * forced forfeit only becomes claimable after `deadline_secs` has
   * *also* elapsed on top of that, giving the server a fair window to
   * recover before the trustless on-chain path opens up. A claim
   * attempted after `deadline_secs` alone but before the buff
   */
  claim_timeout_victory: ({game_id, claimant}: {game_id: u64, claimant: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a get_timeout_remaining transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Query the ledger sequences remaining before the active player's turn times out.
   * 
   * Returns `None` if the game does not exist, is not `InProgress`, or if
   * no timeout has been configured. Returns `Some(0)` when the timeout has
   * already elapsed (a timeout win is claimable).
   * 
   * # Parameters
   * - `game_id` — ID of the game to query.
   * 
   * # Returns
   * `Some(remaining_sequences)` — sequences until timeout, or `Some(0)` if already elapsed.
   * `None` — game not found, not in progress, or timeout not configured.
   */
  get_timeout_remaining: ({game_id}: {game_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Option<u64>>>

  /**
   * Construct and simulate a get_tournament_escrow transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Fetch a tournament escrow by ID.
   * 
   * # Parameters
   * - `escrow_id` — Numeric ID returned by [`create_tournament_escrow`].
   * 
   * # Returns
   * `Ok(TournamentEscrow)` — the full escrow record including `locked_until`
   * and `released` flag.
   * 
   * # Errors
   * - [`ContractError::EscrowNotFound`] — `escrow_id` does not exist or no
   * escrows have been created yet.
   */
  get_tournament_escrow: ({escrow_id}: {escrow_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<TournamentEscrow>>>

  /**
   * Construct and simulate a issue_sep10_challenge transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Issue a SEP-10 Web Authentication challenge for `account` (admin only).
   * 
   * Stores `nonce → expiry` in contract storage. The user must subsequently
   * call [`verify_sep10_challenge`] with a valid ED25519 signature over the
   * challenge payload to become verified.
   * 
   * # Parameters
   * - `admin`    — Must match `CONTRACT_ADMIN`; must authorise the call.
   * - `account`  — Stellar account address the challenge is issued for.
   * - `nonce`    — 32-byte random value; must be globally unique (re-using a
   * nonce returns [`ContractError::ChallengeAlreadyUsed`]).
   * - `expiry`   — Ledger sequence after which the challenge is considered
   * expired and cannot be verified.
   * 
   * # Returns
   * `Ok(())` on success.
   * 
   * # Errors
   * - [`ContractError::Unauthorized`]       — `admin` does not match `CONTRACT_ADMIN`.
   * - [`ContractError::ChallengeExpired`]   — `expiry ≤ current_ledger`.
   * - [`ContractError::ChallengeAlreadyUsed`] — `nonce` already exists in storage.
   * 
   * # Events
   * Emits `("sep10", "issued") → (account, nonce)`.
   */
  issue_sep10_challenge: ({admin, account, nonce, expiry}: {admin: string, account: string, nonce: Buffer, expiry: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a propose_new_admin_key transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Propose a new admin key with a 24-hour timelock.
   * Only the current admin (CONTRACT_ADMIN) can propose.
   */
  propose_new_admin_key: ({admin, new_key}: {admin: string, new_key: Buffer}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a get_whitelisted_tokens transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Return the current whitelist of permitted token contract addresses.
   */
  get_whitelisted_tokens: (options?: MethodOptions) => Promise<AssembledTransaction<Array<string>>>

  /**
   * Construct and simulate a verify_sep10_challenge transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Verify a SEP-10 challenge.
   * 
   * The caller provides the `nonce` and an ED25519 `signature` over
   * SHA256( address_bytes || nonce_bytes || expiry_le8 ).
   * 
   * On success the challenge is consumed and `account` is marked verified.
   */
  verify_sep10_challenge: ({account, nonce, signature}: {account: string, nonce: Buffer, signature: Buffer}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a configure_dispute_system transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Configure the dispute resolution system (admin only).
   * 
   * Sets the arbitrator address and the token fee required to file a
   * dispute. Must be called before [`file_dispute`] is usable.
   * 
   * # Parameters
   * - `admin`        — Must match `CONTRACT_ADMIN`; must authorise the call.
   * - `arbitrator`   — Address authorised to call [`resolve_dispute`] and
   * [`reject_dispute`].
   * - `dispute_fee`  — Token amount a disputing player must pay upfront;
   * refunded if the dispute is rejected. Must be ≥ 0.
   * 
   * # Panics
   * - If the contract is not initialised.
   * - If `admin` does not match the stored admin.
   * - If `dispute_fee < 0`.
   */
  configure_dispute_system: ({admin, arbitrator, dispute_fee}: {admin: string, arbitrator: string, dispute_fee: i128}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a create_tournament_escrow transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Create a time-locked escrow for a completed tournament game.
   * 
   * Calculates the total prize pool from the game's wager amount and locks it
   * until `current_ledger + timelock_duration`. The escrow must then be
   * released via [`release_tournament_escrow`] by the admin after the lock
   * expires. Requires `player1`'s authorisation as the tournament organiser.
   * 
   * # Parameters
   * - `game_id` — ID of a game in `Completed` state.
   * 
   * # Returns
   * `Ok(escrow_id)` — the unique `u64` identifier for the new escrow.
   * 
   * # Errors
   * - [`ContractError::GameNotFound`]       — `game_id` does not exist.
   * - [`ContractError::GameNotInProgress`]  — Game is not in `Completed` state.
   * - [`ContractError::TimeoutNotConfigured`] — [`configure_tournament_timelock`] has not been called.
   * 
   * # Events
   * Emits `("tl_escrow", "created") → (escrow_id, game_id, locked_until)`.
   */
  create_tournament_escrow: ({game_id}: {game_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<u64>>>

  /**
   * Construct and simulate a remove_whitelisted_token transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Remove a token address from the whitelist.
   * Only the contract admin may call this.
   */
  remove_whitelisted_token: ({admin, token}: {admin: string, token: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a initialize_puzzle_rewards transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Initialize puzzle-reward system (#199) and fee configuration (#200).
   * 
   * Must be called exactly once after [`initialize_token`]. Sets the ED25519
   * backend public key, seeds the treasury, and configures the protocol fee
   * and treasury address. Also sets the default `MAX_STAKE` (1 000) and
   * `MAX_PRIZE_POOL` (2 000).
   * 
   * # Parameters
   * - `admin`            — Contract administrator; must authorise the call.
   * - `admin_public_key` — 32-byte ED25519 public key of the backend signing
   * service (used to verify puzzle-reward and win/draw claims).
   * - `treasury_amount`  — Initial token reserve for puzzle reward payouts; must be ≥ 0.
   * - `fee_bips`         — Protocol fee in basis-points of 1 000 (e.g. `20` = 2 %).
   * Capped at 1 000 (100 %).
   * - `treasury_address` — Address that receives the fee portion of each payout.
   * 
   * # Panics
   * - If `CONTRACT_ADMIN` is already set (`"Already initialized"`).
   * - If `admin_public_key.len() != 32`.
   * - If `treasury_amount < 0`.
   * - If `fee_bips > 1000`.
   */
  initialize_puzzle_rewards: ({admin, admin_public_key, treasury_amount, fee_bips, treasury_address}: {admin: string, admin_public_key: Buffer, treasury_amount: i128, fee_bips: u32, treasury_address: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a release_tournament_escrow transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Release a time-locked tournament escrow to the specified winners (admin only).
   * 
   * Distributes the locked prize pool proportionally according to
   * `percentages`. Integer-division dust goes to the first winner.
   * Can only be called after the lock period has expired.
   * 
   * # Parameters
   * - `admin`        — Must match `CONTRACT_ADMIN`; must authorise the call.
   * - `escrow_id`    — ID of a previously created, unreleased escrow.
   * - `winners`      — Ordered recipient addresses.
   * - `percentages`  — Whole-number percentages parallel to `winners`;
   * must sum to exactly 100.
   * 
   * # Returns
   * `Ok(())` on success.
   * 
   * # Errors
   * - [`ContractError::Unauthorized`]         — `admin` does not match `CONTRACT_ADMIN`.
   * - [`ContractError::EscrowNotFound`]        — `escrow_id` does not exist.
   * - [`ContractError::EscrowAlreadyReleased`] — Escrow has already been released.
   * - [`ContractError::EscrowStillLocked`]     — Lock period has not yet elapsed.
   * - [`ContractError::MismatchedLengths`]     — `winners` and `percentages` differ.
   * - [`ContractErro
   */
  release_tournament_escrow: ({admin, escrow_id, winners, percentages}: {admin: string, escrow_id: u64, winners: Array<string>, percentages: Array<u32>}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a claim_puzzle_rewards_batch transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Claim multiple puzzle rewards in a single transaction.
   * 
   * More gas-efficient than calling [`claim_puzzle_reward`] repeatedly because
   * the nonce, balance, and treasury storage maps are read and written only
   * once for the whole batch. Each [`Proof`] is validated with the same
   * signature scheme and replay-protection rules as the single-claim variant.
   * 
   * **Atomicity:** all proofs are applied to in-memory state first; storage
   * is only committed after every proof passes. A single invalid proof rolls
   * back the entire batch.
   * 
   * # Parameters
   * - `proofs` — 1–[`MAX_BATCH_SIZE`] (currently 20) [`Proof`] entries.
   * Each proof carries its own `recipient`, `reward_amount`,
   * `nonce`, and `signature`; see [`claim_puzzle_reward`] for
   * the per-proof validation rules.
   * 
   * # Returns
   * `Ok(())` when all proofs are accepted and state is committed.
   * 
   * # Errors
   * - [`ContractError::EmptyBatch`]    — `proofs` is empty.
   * - [`ContractError::BatchTooLarge`] — `proofs.len() > MAX_BATCH_SIZE`.
   * - [`ContractError::InvalidAmount`] — Any proof has `reward_amo
   */
  claim_puzzle_rewards_batch: ({proofs}: {proofs: Array<Proof>}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a configure_timeout_deadline transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Configure the timestamp-based match deadline and grace buffer used by
   * [`claim_timeout_victory`] (admin only).
   * 
   * # Parameters
   * - `admin`          — Must match `CONTRACT_ADMIN`; must authorise the call.
   * - `deadline_secs`  — Seconds a player has to respond to their opponent's
   * last move before the opponent may force a timeout
   * victory. Must be > 0.
   * - `buffer_secs`    — Additional grace seconds layered on top of
   * `deadline_secs` — e.g. to cover a brief backend
   * outage that would otherwise have arbitrated the
   * game via `claim_win`/`claim_draw`. May be `0` to
   * disable the extra grace window.
   * 
   * # Errors
   * - [`ContractError::NotInitialized`] — Contract has not been initialized.
   * - [`ContractError::Unauthorized`]   — `admin` does not match the stored admin.
   * - [`ContractError::InvalidConfig`]  — `deadline_secs == 0`.
   */
  configure_timeout_deadline: ({admin, deadline_secs, buffer_secs}: {admin: string, deadline_secs: u64, buffer_secs: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a payout_tournament_optimized transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Gas-optimized tournament payout — single pass, no redundant map reads.
   * 
   * Validates that `percentages` sum to exactly 100, then distributes the
   * total prize pool from escrow to each winner in a single token-transfer
   * loop. Any integer-division remainder (dust) is added to the first
   * winner's share.
   * 
   * Requires authorisation from `game.player1` (the tournament organiser).
   * 
   * # Parameters
   * - `game_id`      — ID of a game in the `Completed` state.
   * - `winners`      — Ordered list of recipient addresses.
   * - `percentages`  — Whole-number percentages (0–100) parallel to `winners`;
   * must sum to exactly 100.
   * 
   * # Returns
   * `Ok(())` on success.
   * 
   * # Errors
   * - [`ContractError::GameNotFound`]       — `game_id` does not exist.
   * - [`ContractError::GameNotInProgress`]  — Game is not in `Completed` state.
   * - [`ContractError::MismatchedLengths`]  — `winners` and `percentages` differ in length.
   * - [`ContractError::InvalidPercentage`]  — Percentages overflow or do not sum to 100.
   * 
   * # Events
   * Does not emit events directly; token trans
   */
  payout_tournament_optimized: ({game_id, winners, percentages}: {game_id: u64, winners: Array<string>, percentages: Array<u32>}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a configure_tournament_timelock transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Configure the time-lock duration for tournament prize escrows (admin only).
   * 
   * After a tournament game completes, a prize escrow created via
   * [`create_tournament_escrow`] is locked for `duration` ledger sequences
   * before it can be released.
   * 
   * # Parameters
   * - `admin`     — Must match `CONTRACT_ADMIN`; must authorise the call.
   * - `duration`  — Lock duration in ledger sequences; must be > 0.
   * 
   * # Returns
   * `Ok(())` on success.
   * 
   * # Errors
   * - [`ContractError::Unauthorized`] — `admin` does not match `CONTRACT_ADMIN`.
   * 
   * # Panics
   * - If the contract is not initialised.
   * - If `duration == 0`.
   */
  configure_tournament_timelock: ({admin, duration}: {admin: string, duration: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a get_player_profiles_paginated transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Get all player profiles (paginated for efficiency).
   * 
   * Returns up to `limit` profiles starting from the given offset.
   * This is useful for leaderboards and statistics queries.
   */
  get_player_profiles_paginated: ({offset, limit}: {offset: u32, limit: u32}, options?: MethodOptions) => Promise<AssembledTransaction<Array<PlayerRating>>>

  /**
   * Construct and simulate a get_timeout_victory_remaining transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Query the seconds remaining before [`claim_timeout_victory`] becomes
   * claimable by the player currently waiting on their opponent's move.
   * 
   * Returns `None` if the game doesn't exist, isn't `InProgress`, or no
   * deadline has been configured. Returns `Some(0)` once the deadline +
   * buffer has already elapsed (a timeout victory is claimable now).
   * 
   * # Parameters
   * - `game_id` — ID of the game to query.
   */
  get_timeout_victory_remaining: ({game_id}: {game_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Option<u64>>>

}
export class Client extends ContractClient {
  static async deploy<T = Client>(
    /** Options for initializing a Client as well as for calling a method, with extras specific to deploying. */
    options: MethodOptions &
      Omit<ContractClientOptions, "contractId"> & {
        /** The hash of the Wasm blob, which must already be installed on-chain. */
        wasmHash: Buffer | string;
        /** Salt used to generate the contract's ID. Passed through to {@link Operation.createCustomContract}. Default: random. */
        salt?: Buffer | Uint8Array;
        /** The format used to decode `wasmHash`, if it's provided as a string. */
        format?: "hex" | "base64";
      }
  ): Promise<AssembledTransaction<T>> {
    return ContractClient.deploy(null, options)
  }
  constructor(public readonly options: ContractClientOptions) {
    super(
      new ContractSpec([ "AAAAAAAAAGNQYXVzZSB0aGUgY29udHJhY3Qg4oCUIGJsb2NrcyBhbGwgc3RhdGUtbXV0YXRpbmcgb3BlcmF0aW9ucy4KT25seSB0aGUgY29udHJhY3QgYWRtaW4gbWF5IGNhbGwgdGhpcy4AAAAABXBhdXNlAAAAAAAAAQAAAAAAAAAGY2FsbGVyAAAAAAATAAAAAA==",
        "AAAAAAAAAwpUcmFuc2ZlciB0aGUgcHJpemUgdG8gdGhlIHByZS1yZWNvcmRlZCB3aW5uZXIgb2YgYSBgQ29tcGxldGVkYCBnYW1lLgoKVGhpcyBlbnRyeXBvaW50IGlzIHVzZWQgd2hlbiB0aGUgZ2FtZSBvdXRjb21lIGhhcyBiZWVuIHJlY29yZGVkCm9mZi1jaGFpbiBhbmQgYGdhbWUud2lubmVyYCBoYXMgYmVlbiBzZXQgKGUuZy4gYWZ0ZXIgYGNsYWltX3dpbmAKYWR2YW5jZWQgdGhlIHN0YXRlIHRvIGBDb21wbGV0ZWRgKS4gVGhlIHdpbm5lciBjYWxscyB0aGlzIHRvIHB1bGwKdGhlaXIgZnVuZHMuCgojIFBhcmFtZXRlcnMKLSBgZ2FtZV9pZGAgIOKAlCBJRCBvZiBhIGdhbWUgaW4gYENvbXBsZXRlZGAgc3RhdGUgd2l0aCBhIHJlY29yZGVkIHdpbm5lci4KLSBgd2lubmVyYCAgIOKAlCBNdXN0IG1hdGNoIGBnYW1lLndpbm5lcmA7IG11c3QgYXV0aG9yaXNlIHRoZSBjYWxsLgoKIyBSZXR1cm5zCmBPaygoKSlgIG9uIHN1Y2Nlc3MuCgojIEVycm9ycwotIFtgQ29udHJhY3RFcnJvcjo6R2FtZU5vdEZvdW5kYF0gICAgICAg4oCUIGBnYW1lX2lkYCBkb2VzIG5vdCBleGlzdC4KLSBbYENvbnRyYWN0RXJyb3I6OkFscmVhZHlTZXR0bGVkYF0gICAgIOKAlCBHYW1lIGlzIGFscmVhZHkgYFNldHRsZWRgLgotIFtgQ29udHJhY3RFcnJvcjo6R2FtZU5vdEluUHJvZ3Jlc3NgXSAg4oCUIEdhbWUgaXMgbm90IGluIGBDb21wbGV0ZWRgIHN0YXRlLgotIFtgQ29udHJhY3RFcnJvcjo6Tm90UGxheWVyYF0gICAgICAgICAg4oCUIGB3aW5uZXJgIGRvZXMgbm90IG1hdGNoIGBnYW1lLndpbm5lcmAuAAAAAAAGcGF5b3V0AAAAAAACAAAAAAAAAAdnYW1lX2lkAAAAAAYAAAAAAAAABndpbm5lcgAAAAAAEwAAAAEAAAPpAAAD7QAAAAAAAAfQAAAADUNvbnRyYWN0RXJyb3IAAAA=",
        "AAAAAQAAAAAAAAAAAAAABEdhbWUAAAANAAAAAAAAAAlib2FyZF9mZW4AAAAAAAAOAAAAAAAAAApjcmVhdGVkX2F0AAAAAAAGAAAAAAAAAAxjdXJyZW50X3R1cm4AAAAEAAAAAAAAAAJpZAAAAAAABgAAASdMZWRnZXIgKnRpbWVzdGFtcCogKFVuaXggc2Vjb25kcykgb2YgdGhlIGxhc3QgbW92ZSwgb3Igb2YgYGpvaW5fZ2FtZWAKaWYgbm8gbW92ZSBoYXMgYmVlbiBtYWRlIHlldC4gVXNlZCBieSBgY2xhaW1fdGltZW91dF92aWN0b3J5YCAoU0MtNTApCmZvciBzdHJpY3Qgd2FsbC1jbG9jayBkZWFkbGluZSB2YWxpZGF0aW9uLCBkaXN0aW5jdCBmcm9tCmBsYXN0X21vdmVfYXRgIHdoaWNoIGlzIGxlZGdlci0qc2VxdWVuY2UqLWJhc2VkIGFuZCBiYWNrcyB0aGUgb2xkZXIKYGNsYWltX3RpbWVvdXRfd2luYCBtZWNoYW5pc20uAAAAABBsYXN0X2FjdGl2aXR5X3RzAAAABgAAAAAAAAAMbGFzdF9tb3ZlX2F0AAAABgAAAAAAAAAFbW92ZXMAAAAAAAPqAAAH0AAAAAlDaGVzc01vdmUAAAAAAAAAAAAAB3BsYXllcjEAAAAAEwAAAAAAAAAHcGxheWVyMgAAAAPoAAAAEwAAAAAAAAANcHJvb2Zfb2ZfZ2FtZQAAAAAAA+4AAAAgAAAAAAAAAAVzdGF0ZQAAAAAAB9AAAAAJR2FtZVN0YXRlAAAAAAAAAAAAAAx3YWdlcl9hbW91bnQAAAALAAAAAAAAAAZ3aW5uZXIAAAAAA+gAAAAT",
        "AAAAAAAAAu9Gb3JmZWl0IHRoZSBnYW1lLCBhd2FyZGluZyB0aGUgZnVsbCBwcml6ZSBwb29sIHRvIHRoZSBvcHBvbmVudC4KCkVpdGhlciBwbGF5ZXIgbWF5IGZvcmZlaXQgYW4gYEluUHJvZ3Jlc3NgIGdhbWUgYXQgYW55IHRpbWUuIFRoZQpvcHBvbmVudCByZWNlaXZlcyB0aGUgbmV0IHBheW91dCAodG90YWwgcG9vbCBtaW51cyBwcm90b2NvbCBmZWUpIGFuZAp0aGUgZ2FtZSBpcyBtYXJrZWQgYFNldHRsZWRgLgoKIyBQYXJhbWV0ZXJzCi0gYGdhbWVfaWRgICDigJQgSUQgb2YgYSBnYW1lIGluIGBJblByb2dyZXNzYCBzdGF0ZS4KLSBgcGxheWVyYCAgIOKAlCBGb3JmZWl0aW5nIHBsYXllcidzIGFkZHJlc3M7IG11c3QgYmUgYSBwYXJ0aWNpcGFudCBhbmQKbXVzdCBhdXRob3Jpc2UgdGhlIGNhbGwuCgojIFJldHVybnMKYE9rKCgpKWAgb24gc3VjY2Vzcy4KCiMgRXJyb3JzCi0gW2BDb250cmFjdEVycm9yOjpHYW1lTm90Rm91bmRgXSAgICAgICDigJQgYGdhbWVfaWRgIGRvZXMgbm90IGV4aXN0LgotIFtgQ29udHJhY3RFcnJvcjo6R2FtZU5vdEluUHJvZ3Jlc3NgXSAg4oCUIEdhbWUgaXMgbm90IGBJblByb2dyZXNzYC4KLSBbYENvbnRyYWN0RXJyb3I6Ok5vdFBsYXllcmBdICAgICAgICAgIOKAlCBgcGxheWVyYCBpcyBub3QgYSBwYXJ0aWNpcGFudC4KLSBbYENvbnRyYWN0RXJyb3I6OkdhbWVGdWxsYF0gICAgICAgICAgIOKAlCBObyBgcGxheWVyMmAgZm91bmQgKHNpbmdsZS1wbGF5ZXIgZ2FtZTsgc2hvdWxkIG5vdCBvY2N1cikuAAAAAAdmb3JmZWl0AAAAAAIAAAAAAAAAB2dhbWVfaWQAAAAABgAAAAAAAAAGcGxheWVyAAAAAAATAAAAAQAAA+kAAAPtAAAAAAAAB9AAAAANQ29udHJhY3RFcnJvcgAAAA==",
        "AAAAAAAAAFpVbnBhdXNlIHRoZSBjb250cmFjdCDigJQgcmVzdW1lcyBub3JtYWwgb3BlcmF0aW9ucy4KT25seSB0aGUgY29udHJhY3QgYWRtaW4gbWF5IGNhbGwgdGhpcy4AAAAAAAd1bnBhdXNlAAAAAAEAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAA=",
        "AAAAAAAAAKxVcGdyYWRlIHRoaXMgY29udHJhY3QncyBXQVNNLiBSZXN0cmljdGVkIHRvIHRoZSBBRE1JTl9LRVkgaG9sZGVyLgoKVGhlIGNhbGxlciBtdXN0IHByb3ZpZGUgYW4gRUQyNTUxOSBzaWduYXR1cmUgKGZyb20gdGhlIGJhY2tlbmQgc2lnbmluZwpzZXJ2aWNlKSBvdmVyIGBTSEEyNTYod2FzbV9oYXNoKWAuAAAAB3VwZ3JhZGUAAAAAAgAAAAAAAAAJd2FzbV9oYXNoAAAAAAAD7gAAACAAAAAAAAAACXNpZ25hdHVyZQAAAAAAA+4AAABAAAAAAA==",
        "AAAAAQAAAHFBIHNpbmdsZSBiYWNrZW5kLXNpZ25lZCBwdXp6bGUtcmV3YXJkIGNsYWltLCBhcyBhY2NlcHRlZCBieQpgY2xhaW1fcHV6emxlX3Jld2FyZGAgLyBgY2xhaW1fcHV6emxlX3Jld2FyZHNfYmF0Y2hgLgAAAAAAAAAAAAAFUHJvb2YAAAAAAAAEAAAAAAAAAAVub25jZQAAAAAAAAYAAAAAAAAACXJlY2lwaWVudAAAAAAAABMAAAAAAAAADXJld2FyZF9hbW91bnQAAAAAAAALAAAAAAAAAAlzaWduYXR1cmUAAAAAAAPuAAAAQA==",
        "AAAAAAAAARJGZXRjaCBhIHNpbmdsZSBnYW1lIGJ5IElELgoKIyBQYXJhbWV0ZXJzCi0gYGdhbWVfaWRgIOKAlCBOdW1lcmljIGdhbWUgaWRlbnRpZmllciByZXR1cm5lZCBieSBbYGNyZWF0ZV9nYW1lYF0uCgojIFJldHVybnMKYE9rKEdhbWUpYCDigJQgZnVsbCBbYEdhbWVgXSBzdHJ1Y3QgaW5jbHVkaW5nIHN0YXRlLCBwbGF5ZXJzLCBtb3ZlcywgYW5kIHdhZ2VyLgoKIyBFcnJvcnMKLSBbYENvbnRyYWN0RXJyb3I6OkdhbWVOb3RGb3VuZGBdIOKAlCBgZ2FtZV9pZGAgZG9lcyBub3QgZXhpc3QuAAAAAAAIZ2V0X2dhbWUAAAABAAAAAAAAAAdnYW1lX2lkAAAAAAYAAAABAAAD6QAAB9AAAAAER2FtZQAAB9AAAAANQ29udHJhY3RFcnJvcgAAAA==",
        "AAAAAQAAAAAAAAAAAAAABlRyb3BoeQAAAAAABQAAAAAAAAAJZWFybmVkX2F0AAAAAAAABgAAAAAAAAAMbWV0YWRhdGFfdXJpAAAAEAAAAAAAAAAFb3duZXIAAAAAAAATAAAAAAAAAAl0cm9waHlfaWQAAAAAAAAGAAAAAAAAAAt0cm9waHlfdHlwZQAAAAfQAAAAClRyb3BoeVR5cGUAAA==",
        "AAAAAAAAA6VDbGFpbSBhIHdpbiByZXN1bHQsIHZlcmlmaWVkIGJ5IHRoZSBiYWNrZW5kIHNpZ25pbmcgc2VydmljZS4KClZlcmlmaWVzIGFuIEVEMjU1MTkgc2lnbmF0dXJlIG92ZXIgYFNIQTI1NihnYW1lX2lkX2xlOCB8fCB3aW5uZXJfYWRkcmVzc19ieXRlcylgLApwYXlzIG91dCB0aGUgbmV0IHByaXplICh0b3RhbCBwb29sIG1pbnVzIHByb3RvY29sIGZlZSwgaWYgY29uZmlndXJlZCkgdG8KYHdpbm5lcmAsIGFuZCBtYXJrcyB0aGUgZ2FtZSBgU2V0dGxlZGAuCgojIFNpZ25hdHVyZSBQYXlsb2FkCmBTSEEyNTYoIGdhbWVfaWQudG9fbGVfYnl0ZXMoKSB8fCB3aW5uZXJfYWRkcmVzc19zdHJpbmdfYnl0ZXMgKWAKCiMgUGFyYW1ldGVycwotIGBnYW1lX2lkYCAgICDigJQgSUQgb2YgYSBnYW1lIGluIGBJblByb2dyZXNzYCBzdGF0ZS4KLSBgd2lubmVyYCAgICAg4oCUIEFkZHJlc3Mgb2YgdGhlIHdpbm5pbmcgcGxheWVyOyBtdXN0IGJlIGEgcGFydGljaXBhbnQgYW5kCm11c3QgYXV0aG9yaXNlIHRoZSBjYWxsLgotIGBzaWduYXR1cmVgICDigJQgNjQtYnl0ZSBFRDI1NTE5IHNpZ25hdHVyZSBmcm9tIHRoZSBiYWNrZW5kIHNlcnZpY2UuCgojIFJldHVybnMKYE9rKCgpKWAgb24gc3VjY2Vzcy4KCiMgRXJyb3JzCi0gW2BDb250cmFjdEVycm9yOjpHYW1lTm90Rm91bmRgXSAgICAgICDigJQgYGdhbWVfaWRgIGRvZXMgbm90IGV4aXN0LgotIFtgQ29udHJhY3RFcnJvcjo6R2FtZU5vdEluUHJvZ3Jlc3NgXSAg4oCUIEdhbWUgaXMgbm90IGBJblByb2dyZXNzYC4KLSBbYENvbnRyYWN0RXJyb3I6Ok5vdFBsYXllcmBdICAgICAgICAgIOKAlCBgd2lubmVyYCBpcyBub3QgYSBwYXJ0aWNpcGFudC4KCiMgUGFuaWNzCi0gSWYgdGhlIEVEMjU1MTkgc2lnbmF0dXJlIGlzIGludmFsaWQuCi0gSWYgdGhlIGNvbnRyYWN0IGhhcyBub3QgYmVlbiBpbml0aWFsaXplZC4AAAAAAAAJY2xhaW1fd2luAAAAAAAAAwAAAAAAAAAHZ2FtZV9pZAAAAAAGAAAAAAAAAAZ3aW5uZXIAAAAAABMAAAAAAAAACXNpZ25hdHVyZQAAAAAAA+4AAABAAAAAAQAAA+kAAAPtAAAAAAAAB9AAAAANQ29udHJhY3RFcnJvcgAAAA==",
        "AAAAAAAAADNSZXR1cm5zIGB0cnVlYCBpZiB0aGUgY29udHJhY3QgaXMgY3VycmVudGx5IHBhdXNlZC4AAAAACWlzX3BhdXNlZAAAAAAAAAAAAAABAAAAAQ==",
        "AAAAAAAABABKb2luIGFuIGV4aXN0aW5nIGdhbWUgYXMgdGhlIHNlY29uZCBwbGF5ZXIgYW5kIGxvY2sgbWF0Y2hpbmcgc3Rha2UuCgpUcmFuc2ZlcnMgYGdhbWUud2FnZXJfYW1vdW50YCB0b2tlbnMgZnJvbSBgcGxheWVyMmAgdG8gdGhlIGNvbnRyYWN0LApzZXRzIGBnYW1lLnBsYXllcjJgLCBhbmQgYWR2YW5jZXMgdGhlIGdhbWUgdG8gYEluUHJvZ3Jlc3NgIHN0YXRlIHdpdGgKYGN1cnJlbnRfdHVybiA9IDFgIChwbGF5ZXIxIG1vdmVzIGZpcnN0KS4KCiMgUGFyYW1ldGVycwotIGBnYW1lX2lkYCAg4oCUIElEIG9mIGEgZ2FtZSBpbiBgQ3JlYXRlZGAgc3RhdGUuCi0gYHBsYXllcjJgICDigJQgQWRkcmVzcyBvZiB0aGUgam9pbmluZyBwbGF5ZXI7IG11c3QgZGlmZmVyIGZyb20gYHBsYXllcjFgCmFuZCBtdXN0IGF1dGhvcmlzZSB0aGUgY2FsbC4KCiMgUmV0dXJucwpgT2soKCkpYCBvbiBzdWNjZXNzLgoKIyBFcnJvcnMKLSBbYENvbnRyYWN0RXJyb3I6OkdhbWVOb3RGb3VuZGBdICAgICAgICAgIOKAlCBgZ2FtZV9pZGAgZG9lcyBub3QgZXhpc3QuCi0gW2BDb250cmFjdEVycm9yOjpHYW1lQWxyZWFkeUNvbXBsZXRlZGBdICDigJQgR2FtZSBpcyBub3QgaW4gYENyZWF0ZWRgIHN0YXRlLgotIFtgQ29udHJhY3RFcnJvcjo6R2FtZUZ1bGxgXSAgICAgICAgICAgICAg4oCUIEEgc2Vjb25kIHBsYXllciBoYXMgYWxyZWFkeSBqb2luZWQuCi0gW2BDb250cmFjdEVycm9yOjpBbHJlYWR5Sm9pbmVkYF0gICAgICAgICDigJQgYHBsYXllcjJgIGlzIHRoZSBzYW1lIGFzIGBwbGF5ZXIxYC4KLSBbYENvbnRyYWN0RXJyb3I6OlN0YWtlTGltaXRFeGNlZWRlZGBdICAgIOKAlCBXYWdlciBleGNlZWRzIGBNQVhfU1RBS0VgLgotIFtgQ29udHJhY3RFcnJvcjo6UHJpemVQb29sTGltaXRFeGNlZWRlZGBdIOKAlCBDb21iaW5lZCBwb29sIGV4Y2VlZHMgYE1BWF9QUklaRV9QT09MYC4KLSBbYENvbnRyYWN0RXJyb3I6OkludmFsaWRBbW91bnRgXSAgICAgICAgICDigJQgSW50ZWdlciBvdmVyZmxvdyBjb21wdXRpbmcgdGhlIHBvb2wuAAAACWpvaW5fZ2FtZQAAAAAAAAIAAAAAAAAAB2dhbWVfaWQAAAAABgAAAAAAAAAHcGxheWVyMgAAAAATAAAAAQAAA+kAAAPtAAAAAAAAB9AAAAANQ29udHJhY3RFcnJvcgAAAA==",
        "AAAAAQAAAAAAAAAAAAAAB0Rpc3B1dGUAAAAACAAAAAAAAAAHYWdhaW5zdAAAAAATAAAAAAAAAAhmaWxlZF9hdAAAAAYAAAAAAAAABWZpbGVyAAAAAAAAEwAAAAAAAAAHZ2FtZV9pZAAAAAAGAAAAAAAAAAJpZAAAAAAABgAAAAAAAAAGcmVhc29uAAAAAAAOAAAAAAAAAApyZXNvbHV0aW9uAAAAAAPoAAAADgAAAAAAAAAGc3RhdHVzAAAAAAfQAAAADURpc3B1dGVTdGF0dXMAAAA=",
        "AAAAAgAAAAAAAAAAAAAAB1ZpcFRpZXIAAAAABQAAAAAAAAAAAAAABE5vbmUAAAAAAAAAAAAAAAZCcm9uemUAAAAAAAAAAAAAAAAABlNpbHZlcgAAAAAAAAAAAAAAAAAER29sZAAAAAAAAAAAAAAAB0RpYW1vbmQA",
        "AAAAAAAABABDbGFpbSBhIGRyYXcgcmVzdWx0LCB2ZXJpZmllZCBieSB0aGUgYmFja2VuZCBzaWduaW5nIHNlcnZpY2UuCgpWZXJpZmllcyBhbiBFRDI1NTE5IHNpZ25hdHVyZSBvdmVyIGBTSEEyNTYoZ2FtZV9pZF9sZTggfHwgYiJEUkFXIilgLAp0cmFuc2l0aW9ucyB0aGUgZ2FtZSB0byBgRHJhd25gLCBhbmQgcmV0dXJucyBlYWNoIHBsYXllcidzIHN0YWtlIGluCmZ1bGwgdmlhIFtgcHJvY2Vzc19kcmF3X3BheW91dGBdLgoKVGhlIGJhY2tlbmQgc2lnbmF0dXJlIHByZXZlbnRzIGVpdGhlciBwbGF5ZXIgZnJvbSB1bmlsYXRlcmFsbHkgY2xhaW1pbmcKYSBkcmF3IHdpdGhvdXQgbXV0dWFsIGFncmVlbWVudCBvciBhcmJpdGVyIGFwcHJvdmFsLgoKIyBTaWduYXR1cmUgUGF5bG9hZApgU0hBMjU2KCBnYW1lX2lkLnRvX2xlX2J5dGVzKCkgfHwgYiJEUkFXIiApYAoKIyBQYXJhbWV0ZXJzCi0gYGdhbWVfaWRgICAgIOKAlCBJRCBvZiBhIGdhbWUgaW4gYEluUHJvZ3Jlc3NgIHN0YXRlLgotIGBwbGF5ZXJgICAgICDigJQgRWl0aGVyIHBhcnRpY2lwYW50J3MgYWRkcmVzczsgbXVzdCBhdXRob3Jpc2UgdGhlIGNhbGwuCi0gYHNpZ25hdHVyZWAgIOKAlCA2NC1ieXRlIEVEMjU1MTkgc2lnbmF0dXJlIGZyb20gdGhlIGJhY2tlbmQgc2VydmljZS4KCiMgUmV0dXJucwpgT2soKCkpYCBvbiBzdWNjZXNzLgoKIyBFcnJvcnMKLSBbYENvbnRyYWN0RXJyb3I6OkdhbWVOb3RGb3VuZGBdICAgICAgIOKAlCBgZ2FtZV9pZGAgZG9lcyBub3QgZXhpc3QuCi0gW2BDb250cmFjdEVycm9yOjpHYW1lTm90SW5Qcm9ncmVzc2BdICDigJQgR2FtZSBpcyBub3QgYEluUHJvZ3Jlc3NgLgotIFtgQ29udHJhY3RFcnJvcjo6Tm90UGxheWVyYF0gICAgICAgICAg4oCUIGBwbGF5ZXJgIGlzIG5vdCBhIHBhcnRpY2lwYW50LgoKIyBQYW5pY3MKLSBJZiB0aGUgRUQyNTUxOSBzaWduYXR1cmUgaXMgaW52YWxpZCAoU29yb2JhbiBgZWQyNTUxOV92ZXJpZnlgIHBhbmljcyBvbiBmYWlsdXJlKS4KLSBJZiB0aGUgY29udHJhY3QgaGFzIG5vdCBiZWVuIGluAAAACmNsYWltX2RyYXcAAAAAAAMAAAAAAAAAB2dhbWVfaWQAAAAABgAAAAAAAAAGcGxheWVyAAAAAAATAAAAAAAAAAlzaWduYXR1cmUAAAAAAAPuAAAAQAAAAAEAAAPpAAAD7QAAAAAAAAfQAAAADUNvbnRyYWN0RXJyb3IAAAA=",
        "AAAAAAAAANBSZXR1cm4gdGhlIGNvbmZpZ3VyZWQgU0VQLTQwIG9yYWNsZSBjb250cmFjdCBhZGRyZXNzLgoKIyBSZXR1cm5zCmBPayhBZGRyZXNzKWAg4oCUIHRoZSBvcmFjbGUgY29udHJhY3QgYWRkcmVzcy4KCiMgRXJyb3JzCi0gW2BDb250cmFjdEVycm9yOjpPcmFjbGVOb3RDb25maWd1cmVkYF0g4oCUIFtgY29uZmlndXJlX29yYWNsZWBdIGhhcyBub3QgYmVlbiBjYWxsZWQuAAAACmdldF9vcmFjbGUAAAAAAAAAAAABAAAD6QAAABMAAAfQAAAADUNvbnRyYWN0RXJyb3IAAAA=",
        "AAAAAAAAArRDYW5jZWwgYSBnYW1lIHRoYXQgaGFzIG5vdCB5ZXQgYmVlbiBqb2luZWQgYW5kIHJlZnVuZCB0aGUgY3JlYXRvci4KCk9ubHkgYHBsYXllcjFgICh0aGUgY3JlYXRvcikgbWF5IGNhbmNlbCwgYW5kIG9ubHkgd2hpbGUgdGhlIGdhbWUgaXMgaW4KYENyZWF0ZWRgIHN0YXRlIChiZWZvcmUgYHBsYXllcjJgIGpvaW5zKS4gVGhlIHN0YWtlZCB3YWdlciBpcyByZXR1cm5lZAp0byBgcGxheWVyMWAgYW5kIHRoZSBnYW1lIGlzIG1hcmtlZCBgQ29tcGxldGVkYCB0byBwcmV2ZW50IGZ1dHVyZSBqb2lucy4KCiMgUGFyYW1ldGVycwotIGBnYW1lX2lkYCAg4oCUIElEIG9mIGEgZ2FtZSBpbiBgQ3JlYXRlZGAgc3RhdGUuCi0gYHBsYXllcmAgICDigJQgTXVzdCBlcXVhbCBgZ2FtZS5wbGF5ZXIxYDsgbXVzdCBhdXRob3Jpc2UgdGhlIGNhbGwuCgojIFJldHVybnMKYE9rKCgpKWAgb24gc3VjY2Vzcy4KCiMgRXJyb3JzCi0gW2BDb250cmFjdEVycm9yOjpHYW1lTm90Rm91bmRgXSAgICAgICAgIOKAlCBgZ2FtZV9pZGAgZG9lcyBub3QgZXhpc3QuCi0gW2BDb250cmFjdEVycm9yOjpHYW1lQWxyZWFkeUNvbXBsZXRlZGBdIOKAlCBHYW1lIGhhcyBhbHJlYWR5IHByb2dyZXNzZWQgcGFzdCBgQ3JlYXRlZGAuCi0gW2BDb250cmFjdEVycm9yOjpOb3RQbGF5ZXJgXSAgICAgICAgICAgIOKAlCBgcGxheWVyYCBpcyBub3QgYHBsYXllcjFgLgAAAAtjYW5jZWxfZ2FtZQAAAAACAAAAAAAAAAdnYW1lX2lkAAAAAAYAAAAAAAAABnBsYXllcgAAAAAAEwAAAAEAAAPpAAAD7QAAAAAAAAfQAAAADUNvbnRyYWN0RXJyb3IAAAA=",
        "AAAAAAAAA2dDcmVhdGUgYSBuZXcgd2FnZXIgZ2FtZSBhbmQgbG9jayBgcGxheWVyMWAncyBzdGFrZSBpbnRvIGVzY3Jvdy4KCkdlbmVyYXRlcyBhIHNlcXVlbnRpYWwgZ2FtZSBJRCwgdHJhbnNmZXJzIGB3YWdlcl9hbW91bnRgIHRva2VucyBmcm9tCmBwbGF5ZXIxYCB0byB0aGUgY29udHJhY3QsIGFuZCBzdG9yZXMgdGhlIGdhbWUgaW4gYENyZWF0ZWRgIHN0YXRlCmF3YWl0aW5nIGEgc2Vjb25kIHBsYXllci4KCiMgUGFyYW1ldGVycwotIGBwbGF5ZXIxYCAgICAgICDigJQgQWRkcmVzcyBvZiB0aGUgZ2FtZSBjcmVhdG9yOyBtdXN0IGF1dGhvcmlzZSB0aGUgY2FsbC4KLSBgd2FnZXJfYW1vdW50YCAg4oCUIFRva2VuIGFtb3VudCB0byBzdGFrZTsgbXVzdCBiZSA+IDAsIOKJpCBgTUFYX1NUQUtFYCwgYW5kCmB3YWdlcl9hbW91bnQgKiAyYCBtdXN0IGJlIOKJpCBgTUFYX1BSSVpFX1BPT0xgLgoKIyBSZXR1cm5zCmBPayhnYW1lX2lkKWAg4oCUIHRoZSB1bmlxdWUgYHU2NGAgaWRlbnRpZmllciBmb3IgdGhlIG5ldyBnYW1lLgoKIyBFcnJvcnMKLSBbYENvbnRyYWN0RXJyb3I6OlN0YWtlTGltaXRFeGNlZWRlZGBdICAgIOKAlCBgd2FnZXJfYW1vdW50ID4gTUFYX1NUQUtFYC4KLSBbYENvbnRyYWN0RXJyb3I6OlByaXplUG9vbExpbWl0RXhjZWVkZWRgXSDigJQgYHdhZ2VyX2Ftb3VudCAqIDIgPiBNQVhfUFJJWkVfUE9PTGAuCi0gW2BDb250cmFjdEVycm9yOjpJbnZhbGlkQW1vdW50YF0gICAgICAgICAg4oCUIEludGVnZXIgb3ZlcmZsb3cgY29tcHV0aW5nIHRoZSBwcml6ZSBwb29sLgotIFtgQ29udHJhY3RFcnJvcjo6SW5zdWZmaWNpZW50RnVuZHNgXSAgICAgIOKAlCBgcGxheWVyMWAncyB0b2tlbiBiYWxhbmNlIDwgYHdhZ2VyX2Ftb3VudGAuAAAAAAtjcmVhdGVfZ2FtZQAAAAADAAAAAAAAAAdwbGF5ZXIxAAAAABMAAAAAAAAADHdhZ2VyX2Ftb3VudAAAAAsAAAAAAAAADWluaXRpYWxfYm9hcmQAAAAAAAAOAAAAAQAAA+kAAAAGAAAH0AAAAA1Db250cmFjdEVycm9yAAAA",
        "AAAAAAAAAS1GZXRjaCBhIGRpc3B1dGUgYnkgSUQuCgojIFBhcmFtZXRlcnMKLSBgZGlzcHV0ZV9pZGAg4oCUIE51bWVyaWMgSUQgcmV0dXJuZWQgYnkgW2BmaWxlX2Rpc3B1dGVgXS4KCiMgUmV0dXJucwpgT2soRGlzcHV0ZSlgIOKAlCB0aGUgZnVsbCBbYERpc3B1dGVgXSBzdHJ1Y3QgaW5jbHVkaW5nIHN0YXR1cyBhbmQgcmVzb2x1dGlvbi4KCiMgRXJyb3JzCi0gW2BDb250cmFjdEVycm9yOjpEaXNwdXRlTm90Rm91bmRgXSDigJQgYGRpc3B1dGVfaWRgIGRvZXMgbm90IGV4aXN0IG9yCm5vIGRpc3B1dGVzIGhhdmUgYmVlbiBmaWxlZCB5ZXQuAAAAAAAAC2dldF9kaXNwdXRlAAAAAAEAAAAAAAAACmRpc3B1dGVfaWQAAAAAAAYAAAABAAAD6QAAB9AAAAAHRGlzcHV0ZQAAAAfQAAAADUNvbnRyYWN0RXJyb3IAAAA=",
        "AAAAAAAAA5ZSZWNvcmQgYSBjaGVzcyBtb3ZlIGZvciB0aGUgYWN0aXZlIHBsYXllciBhbmQgYWR2YW5jZSB0aGUgdHVybi4KCkFwcGVuZHMgYSBbYENoZXNzTW92ZWBdIGVudHJ5IHRvIGBnYW1lLm1vdmVzYCwgZmxpcHMgYGN1cnJlbnRfdHVybmAKKDEg4oaSIDIg4oaSIDEg4oCmKSwgYW5kIHVwZGF0ZXMgYGxhc3RfbW92ZV9hdGAgdG8gdGhlIGN1cnJlbnQgbGVkZ2VyCnNlcXVlbmNlICh1c2VkIGJ5IHRoZSB0aW1lb3V0IG1lY2hhbmlzbSkuCgojIFBhcmFtZXRlcnMKLSBgZ2FtZV9pZGAgICAg4oCUIElEIG9mIGEgZ2FtZSBpbiBgSW5Qcm9ncmVzc2Agc3RhdGUuCi0gYHBsYXllcmAgICAgIOKAlCBBZGRyZXNzIG9mIHRoZSBtb3ZpbmcgcGxheWVyOyBtdXN0IGJlIGBwbGF5ZXIxYCBvciBgcGxheWVyMmAKYW5kIG11c3QgYXV0aG9yaXNlIHRoZSBjYWxsLgotIGBtb3ZlX2RhdGFgICDigJQgRW5jb2RlZCBtb3ZlIHBheWxvYWQgKGUuZy4gZnJvbS90byBzcXVhcmVzIGFzIGB1MzJgcyk7Cm11c3QgYmUgbm9uLWVtcHR5LgoKIyBSZXR1cm5zCmBPaygoKSlgIG9uIHN1Y2Nlc3MuCgojIEVycm9ycwotIFtgQ29udHJhY3RFcnJvcjo6R2FtZU5vdEZvdW5kYF0gICAgICAg4oCUIGBnYW1lX2lkYCBkb2VzIG5vdCBleGlzdC4KLSBbYENvbnRyYWN0RXJyb3I6OkdhbWVOb3RJblByb2dyZXNzYF0gIOKAlCBHYW1lIGlzIG5vdCBpbiBgSW5Qcm9ncmVzc2Agc3RhdGUuCi0gW2BDb250cmFjdEVycm9yOjpOb3RQbGF5ZXJgXSAgICAgICAgICDigJQgYHBsYXllcmAgaXMgbm90IGEgcGFydGljaXBhbnQuCi0gW2BDb250cmFjdEVycm9yOjpOb3RZb3VyVHVybmBdICAgICAgICDigJQgSXQgaXMgbm90IGBwbGF5ZXJgJ3MgdHVybi4KLSBbYENvbnRyYWN0RXJyb3I6OkludmFsaWRNb3ZlYF0gICAgICAgIOKAlCBgbW92ZV9kYXRhYCBpcyBlbXB0eS4AAAAAAAtzdWJtaXRfbW92ZQAAAAAFAAAAAAAAAAdnYW1lX2lkAAAAAAYAAAAAAAAABnBsYXllcgAAAAAAEwAAAAAAAAAJbW92ZV9kYXRhAAAAAAAD6gAAAAQAAAAAAAAACW5ld19ib2FyZAAAAAAAAA4AAAAAAAAAC2dhbWVfc3RhdHVzAAAAAAQAAAABAAAD6QAAA+0AAAAAAAAH0AAAAA1Db250cmFjdEVycm9yAAAA",
        "AAAAAQAAAAAAAAAAAAAACUNoZXNzTW92ZQAAAAAAAAMAAAAAAAAACW1vdmVfZGF0YQAAAAAAA+oAAAAEAAAAAAAAAAZwbGF5ZXIAAAAAABMAAAAAAAAACXRpbWVzdGFtcAAAAAAAAAY=",
        "AAAAAgAAAAAAAAAAAAAACUdhbWVTdGF0ZQAAAAAAAAcAAAAAAAAAAAAAAAdDcmVhdGVkAAAAAAAAAAAAAAAACkluUHJvZ3Jlc3MAAAAAAAAAAAAAAAAACUNvbXBsZXRlZAAAAAAAAAAAAAAAAAAACEVzY3Jvd2VkAAAAAAAAAAAAAAAHU2V0dGxlZAAAAAAAAAAAAAAAAAVEcmF3bgAAAAAAAAAAAAAAAAAACUZvcmZlaXRlZAAAAA==",
        "AAAAAQAAAAAAAAAAAAAACVZpcENvbmZpZwAAAAAAAAgAAAAAAAAAEGJyb256ZV90aHJlc2hvbGQAAAALAAAAAAAAABFkaWFtb25kX3RocmVzaG9sZAAAAAAAAAsAAAAAAAAAE2ZlZV9kaXNjb3VudF9icm9uemUAAAAABAAAAAAAAAAUZmVlX2Rpc2NvdW50X2RpYW1vbmQAAAAEAAAAAAAAABFmZWVfZGlzY291bnRfZ29sZAAAAAAAAAQAAAAAAAAAE2ZlZV9kaXNjb3VudF9zaWx2ZXIAAAAABAAAAAAAAAAOZ29sZF90aHJlc2hvbGQAAAAAAAsAAAAAAAAAEHNpbHZlcl90aHJlc2hvbGQAAAAL",
        "AAAAAQAAAAAAAAAAAAAACVZpcEhvbGRlcgAAAAAAAAUAAAAAAAAAB2FkZHJlc3MAAAAAEwAAAAAAAAAKZXhwaXJlc19hdAAAAAAD6AAAAAYAAAAAAAAACWpvaW5lZF9hdAAAAAAAAAYAAAAAAAAABHRpZXIAAAfQAAAAB1ZpcFRpZXIAAAAAAAAAAA10b2tlbl9iYWxhbmNlAAAAAAAACw==",
        "AAAAAAAABABGaWxlIGEgZGlzcHV0ZSBhZ2FpbnN0IGFuIG9wcG9uZW50IGZvciBhbiBpbi1wcm9ncmVzcyBnYW1lLgoKVHJhbnNmZXJzIHRoZSBjb25maWd1cmVkIGBkaXNwdXRlX2ZlZWAgKGlmIG5vbi16ZXJvKSBmcm9tIGBmaWxlcmAgdG8KdGhlIGNvbnRyYWN0LCBjcmVhdGVzIGEgW2BEaXNwdXRlYF0gaW4gYFBlbmRpbmdgIHN0YXR1cywgYW5kIGVtaXRzIGEKYCgiZGlzcHV0ZSIsICJmaWxlZCIpYCBldmVudC4gVGhlIGZlZSBpcyBoZWxkIHVudGlsIHRoZSBkaXNwdXRlIGlzCnJlc29sdmVkIG9yIHJlamVjdGVkLgoKIyBQYXJhbWV0ZXJzCi0gYGdhbWVfaWRgICAg4oCUIElEIG9mIGEgZ2FtZSBpbiBgSW5Qcm9ncmVzc2Agc3RhdGUuCi0gYGZpbGVyYCAgICAg4oCUIFBsYXllciByYWlzaW5nIHRoZSBkaXNwdXRlOyBtdXN0IGJlIGEgZ2FtZSBwYXJ0aWNpcGFudAphbmQgbXVzdCBhdXRob3Jpc2UgdGhlIGNhbGwuCi0gYGFnYWluc3RgICAg4oCUIFRoZSBvcHBvc2luZyBwbGF5ZXI7IG11c3QgYWxzbyBiZSBhIHBhcnRpY2lwYW50IGFuZAptdXN0IGRpZmZlciBmcm9tIGBmaWxlcmAuCi0gYHJlYXNvbmAgICAg4oCUIEVuY29kZWQgcmVhc29uIGJ5dGVzIChhcmJpdHJhcnkgb2ZmLWNoYWluIHBheWxvYWQpLgoKIyBSZXR1cm5zCmBPayhkaXNwdXRlX2lkKWAg4oCUIHVuaXF1ZSBgdTY0YCBpZGVudGlmaWVyIGZvciB0aGUgbmV3IGRpc3B1dGUuCgojIEVycm9ycwotIFtgQ29udHJhY3RFcnJvcjo6R2FtZU5vdEZvdW5kYF0gICAgICAgICAgICDigJQgYGdhbWVfaWRgIGRvZXMgbm90IGV4aXN0LgotIFtgQ29udHJhY3RFcnJvcjo6Tm90RGlzcHV0YWJsZWBdICAgICAgICAgICDigJQgR2FtZSBpcyBub3QgYEluUHJvZ3Jlc3NgLgotIFtgQ29udHJhY3RFcnJvcjo6Tm90UGxheWVyYF0gICAgICAgICAgICAgICDigJQgYGZpbGVyYCBvciBgYWdhaW5zdGAgaXMgbm90IGEgcGFydGljaXBhbnQuCi0gW2BDb250cmFjdEVycm9yOjpJbnZhbGlkTW92ZWBdICAgICAgICAgICAgIOKAlCBgZmlsZXJgIGFuZCBgYWdhaW5zdGAgAAAADGZpbGVfZGlzcHV0ZQAAAAQAAAAAAAAAB2dhbWVfaWQAAAAABgAAAAAAAAAFZmlsZXIAAAAAAAATAAAAAAAAAAdhZ2FpbnN0AAAAABMAAAAAAAAABnJlYXNvbgAAAAAADgAAAAEAAAPpAAAABgAAB9AAAAANQ29udHJhY3RFcnJvcgAAAA==",
        "AAAAAgAAAAAAAAAAAAAAClRyb3BoeVR5cGUAAAAAAAYAAAAAAAAAAAAAAAhGaXJzdFdpbgAAAAEAAAAAAAAACVdpblN0cmVhawAAAAAAAAEAAAAEAAAAAAAAAAAAAAAMUHV6emxlTWFzdGVyAAAAAAAAAAAAAAASVG91cm5hbWVudENoYW1waW9uAAAAAAABAAAAAAAAAA9SYXRpbmdNaWxlc3RvbmUAAAAAAQAAAAUAAAABAAAAAAAAAAZDdXN0b20AAAAAAAEAAAPuAAAAIA==",
        "AAAAAAAAAUNSZXR1cm4gZXZlcnkgc3RvcmVkIGdhbWUgYXMgYSBtYXAga2V5ZWQgYnkgZ2FtZSBJRC4KClJldHVybnMgYW4gZW1wdHkgbWFwIGlmIG5vIGdhbWVzIGhhdmUgYmVlbiBjcmVhdGVkIHlldC4KCj4gKipOb3RlOioqIFRoaXMgaXMgYW4gdW5ib3VuZGVkIHJlYWQuIEZyb250ZW5kIGNhbGxlcnMgc2hvdWxkIHBhZ2luYXRlCj4gdXNpbmcgaW5kaXZpZHVhbCBbYGdldF9nYW1lYF0gY2FsbHMgb25jZSB0aGUgZ2FtZSBjb3VudCBncm93cyBsYXJnZS4KCiMgUmV0dXJucwpgTWFwPHU2NCwgR2FtZT5gIOKAlCBhbGwgZ2FtZXMgaW5kZXhlZCBieSB0aGVpciBudW1lcmljIElELgAAAAANZ2V0X2FsbF9nYW1lcwAAAAAAAAAAAAABAAAD7AAAAAYAAAfQAAAABEdhbWU=",
        "AAAAAAAAAcJVcGRhdGUgdGhlIHBlci1nYW1lIG1heGltdW0gd2FnZXIgKGFkbWluIG9ubHkpLgoKQW55IHN1YnNlcXVlbnQgW2BjcmVhdGVfZ2FtZWBdIG9yIFtgam9pbl9nYW1lYF0gY2FsbCB3aXRoIGEKYHdhZ2VyX2Ftb3VudGAgZXhjZWVkaW5nIHRoaXMgbGltaXQgd2lsbCByZXR1cm4KW2BDb250cmFjdEVycm9yOjpTdGFrZUxpbWl0RXhjZWVkZWRgXS4KCiMgUGFyYW1ldGVycwotIGBhZG1pbmAgICAgICDigJQgTXVzdCBtYXRjaCBgQ09OVFJBQ1RfQURNSU5gOyBtdXN0IGF1dGhvcmlzZSB0aGUgY2FsbC4KLSBgbmV3X2xpbWl0YCAg4oCUIE5ldyBtYXhpbXVtIHdhZ2VyOyBtdXN0IGJlID4gMC4KCiMgUGFuaWNzCi0gSWYgdGhlIGNvbnRyYWN0IGlzIG5vdCBpbml0aWFsaXplZC4KLSBJZiBgYWRtaW5gIGRvZXMgbm90IG1hdGNoIHRoZSBzdG9yZWQgYWRtaW4uCi0gSWYgYG5ld19saW1pdCA8PSAwYC4AAAAAAA1zZXRfbWF4X3N0YWtlAAAAAAAAAgAAAAAAAAAFYWRtaW4AAAAAAAATAAAAAAAAAAluZXdfbGltaXQAAAAAAAALAAAAAA==",
        "AAAAAAAAAclTZXQgdGhlIGNvbnRyYWN0IGFkbWluIHdoZW4gbm9uZSBoYXMgYmVlbiByZWNvcmRlZCB5ZXQgKHVwZ3JhZGUgcGF0aCkuCgpUaGlzIGlzIGEgb25lLXRpbWUgbWlncmF0aW9uIGhlbHBlciBmb3IgY29udHJhY3RzIGluaXRpYWxpc2VkIGJlZm9yZQpgQ09OVFJBQ1RfQURNSU5gIHN0b3JhZ2Ugd2FzIGludHJvZHVjZWQuIEl0IHBhbmljcyBpZiBhbiBhZG1pbiBpcwphbHJlYWR5IHNldCwgb3IgaWYgdGhlIGNvbnRyYWN0J3MgYEFETUlOX0tFWWAgaGFzIG5vdCBiZWVuIHNlZWRlZC4KCiMgUGFyYW1ldGVycwotIGBhZG1pbmAg4oCUIE5ldyBhZG1pbmlzdHJhdG9yIGFkZHJlc3M7IG11c3QgYXV0aG9yaXNlIHRoZSBjYWxsLgoKIyBQYW5pY3MKLSBJZiBgQ09OVFJBQ1RfQURNSU5gIGlzIGFscmVhZHkgc2V0LgotIElmIGBBRE1JTl9LRVlgIGlzIG5vdCBzZXQgKGNvbnRyYWN0IG5vdCBpbml0aWFsaXNlZCkuAAAAAAAADXVwZ3JhZGVfYWRtaW4AAAAAAAABAAAAAAAAAAVhZG1pbgAAAAAAABMAAAAA",
        "AAAAAQAAAAAAAAAAAAAAC0RlY2F5Q29uZmlnAAAAAAQAAAAAAAAADWJhbl90aHJlc2hvbGQAAAAAAAALAAAAAAAAABRiYXNlX2RlY2F5X3JhdGVfYmlwcwAAAAQAAAAAAAAAEm1heF9kZWNheV9kdXJhdGlvbgAAAAAABgAAAAAAAAAVbWluX3BlbmFsdHlfdGhyZXNob2xkAAAAAAAACw==",
        "AAAAAQAAAAAAAAAAAAAAC0ZlZVByb3Bvc2FsAAAAAAQAAAAAAAAADG5ld19mZWVfYmlwcwAAAAQAAAAAAAAAFG5ld190cmVhc3VyeV9hZGRyZXNzAAAAEwAAAAAAAAALcHJvcG9zZWRfYXQAAAAABgAAAAAAAAAIcHJvcG9zZXIAAAAT",
        "AAAAAAAAAiBVcGRhdGUgdGhlIHByb3RvY29sIGZlZSBhbmQgdHJlYXN1cnkgYWRkcmVzcyAoYWRtaW4gb25seSwgbm8gbXVsdGktc2lnKS4KCkZvciBmZWUgY2hhbmdlcyByZXF1aXJpbmcgbXVsdGktc2lnIGFwcHJvdmFsLCB1c2UgdGhlCltgcHJvcG9zZV9mZWVfY2hhbmdlYF0gLyBbYGFwcHJvdmVfZmVlX3Byb3Bvc2FsYF0gZmxvdyBpbnN0ZWFkLgoKIyBQYXJhbWV0ZXJzCi0gYGFkbWluYCAgICAgICAgICAgICDigJQgTXVzdCBtYXRjaCBgQ09OVFJBQ1RfQURNSU5gOyBtdXN0IGF1dGhvcmlzZSB0aGUgY2FsbC4KLSBgZmVlX2JpcHNgICAgICAgICAgIOKAlCBOZXcgZmVlIGluIGJhc2lzLXBvaW50cyBvZiAxIDAwMCAoMOKAkzEgMDAwKS4KLSBgdHJlYXN1cnlfYWRkcmVzc2AgIOKAlCBOZXcgcmVjaXBpZW50IGFkZHJlc3MgZm9yIHRoZSBmZWUgcG9ydGlvbi4KCiMgUGFuaWNzCi0gSWYgdGhlIGNvbnRyYWN0IGlzIG5vdCBpbml0aWFsaXplZC4KLSBJZiBgYWRtaW5gIGRvZXMgbm90IG1hdGNoIHRoZSBzdG9yZWQgYWRtaW4uCi0gSWYgYGZlZV9iaXBzID4gMTAwMGAuAAAADmNvbmZpZ3VyZV9mZWVzAAAAAAADAAAAAAAAAAVhZG1pbgAAAAAAABMAAAAAAAAACGZlZV9iaXBzAAAABAAAAAAAAAAQdHJlYXN1cnlfYWRkcmVzcwAAABMAAAAA",
        "AAAAAAAAA0FSZWplY3QgYSBwZW5kaW5nIGRpc3B1dGUgYW5kIHJlZnVuZCB0aGUgZGlzcHV0ZSBmZWUgdG8gdGhlIGZpbGVyIChhcmJpdHJhdG9yIG9ubHkpLgoKTWFya3MgdGhlIGRpc3B1dGUgYFJlamVjdGVkYCwgc3RvcmVzIHRoZSBhcmJpdHJhdG9yJ3MgcmVhc29uLCBhbmQKcmV0dXJucyB0aGUgZGlzcHV0ZSBmZWUgKGlmIG5vbi16ZXJvKSB0byB0aGUgb3JpZ2luYWwgZmlsZXIuCgojIFBhcmFtZXRlcnMKLSBgZGlzcHV0ZV9pZGAgIOKAlCBJRCBvZiBhIGRpc3B1dGUgaW4gYFBlbmRpbmdgIHN0YXR1cy4KLSBgYXJiaXRyYXRvcmAgIOKAlCBNdXN0IG1hdGNoIHRoZSBzdG9yZWQgYXJiaXRyYXRvcjsgbXVzdCBhdXRob3Jpc2UgdGhlIGNhbGwuCi0gYHJlYXNvbmAgICAgICDigJQgRW5jb2RlZCByZWplY3Rpb24gcmF0aW9uYWxlIChhcmJpdHJhcnkgb2ZmLWNoYWluIHBheWxvYWQpLgoKIyBSZXR1cm5zCmBPaygoKSlgIG9uIHN1Y2Nlc3MuCgojIEVycm9ycwotIFtgQ29udHJhY3RFcnJvcjo6Tm90QXJiaXRyYXRvcmBdICAgICAgICDigJQgYGFyYml0cmF0b3JgIGlzIG5vdCB0aGUgY29uZmlndXJlZCBhcmJpdHJhdG9yLApvciB0aGUgYXJiaXRyYXRvciBoYXMgbm90IGJlZW4gc2V0LgotIFtgQ29udHJhY3RFcnJvcjo6RGlzcHV0ZU5vdEZvdW5kYF0gICAgICDigJQgYGRpc3B1dGVfaWRgIGRvZXMgbm90IGV4aXN0LgotIFtgQ29udHJhY3RFcnJvcjo6R2FtZUFscmVhZHlDb21wbGV0ZWRgXSDigJQgRGlzcHV0ZSBpcyBub3QgaW4gYFBlbmRpbmdgIHN0YXR1cy4KCiMgRXZlbnRzCkVtaXRzIGAoImRpc3B1dGUiLCAicmVqZWN0Iikg4oaSIChkaXNwdXRlX2lkLCBmaWxlcilgLgAAAAAAAA5yZWplY3RfZGlzcHV0ZQAAAAAAAwAAAAAAAAAKZGlzcHV0ZV9pZAAAAAAABgAAAAAAAAAKYXJiaXRyYXRvcgAAAAAAEwAAAAAAAAAGcmVhc29uAAAAAAAOAAAAAQAAA+kAAAPtAAAAAAAAB9AAAAANQ29udHJhY3RFcnJvcgAAAA==",
        "AAAAAAAAAC5RdWVyeSB0aGUgcHV6emxlLXJld2FyZCBiYWxhbmNlIG9mIGFuIGFkZHJlc3MuAAAAAAAOcmV3YXJkX2JhbGFuY2UAAAAAAAEAAAAAAAAAB2FkZHJlc3MAAAAAEwAAAAEAAAAL",
        "AAAAAQAAAAAAAAAAAAAADFBsYXllclJhdGluZwAAAAgAAAAAAAAAB2FkZHJlc3MAAAAAEwAAAAAAAAAFZHJhd3MAAAAAAAAEAAAAAAAAAAxnYW1lc19wbGF5ZWQAAAAEAAAAAAAAAA5oaWdoZXN0X3JhdGluZwAAAAAABQAAAAAAAAAMbGFzdF91cGRhdGVkAAAABgAAAAAAAAAGbG9zc2VzAAAAAAAEAAAAAAAAAAZyYXRpbmcAAAAAAAUAAAAAAAAABHdpbnMAAAAE",
        "AAAAAQAAAAAAAAAAAAAADFB1enpsZUJvdW50eQAAAAkAAAAAAAAACWJvdW50eV9pZAAAAAAAAAYAAAAAAAAAB2NsYWltZWQAAAAAAQAAAAAAAAAHY3JlYXRvcgAAAAATAAAAAAAAAAhkZWFkbGluZQAAAAYAAAAAAAAAC21heF93aW5uZXJzAAAAAAQAAAAAAAAACXB1enpsZV9pZAAAAAAAAAYAAAAAAAAADXJld2FyZF9hbW91bnQAAAAAAAALAAAAAAAAABF0b3RhbF9zdWJtaXNzaW9ucwAAAAAAAAQAAAAAAAAABndpbm5lcgAAAAAD6AAAABM=",
        "AAAAAAAAAZtHZXQgdGhlIG9yYWNsZS1zeW5jZWQgdGltZXN0YW1wIChsZWRnZXIgc2VxdWVuY2UgZnJvbSBvcmFjbGUpLgpGYWxscyBiYWNrIHRvIHRoZSBjdXJyZW50IGxlZGdlciBzZXF1ZW5jZSBpZiBvcmFjbGUgaXMgbm90IGNvbmZpZ3VyZWQuClRoZSBvcmFjbGUgaXMgY2FsbGVkIHZpYSBjcm9zcy1jb250cmFjdCBpbnZvY2F0aW9uIHVzaW5nIHRoZSBTRVAtNDAKYGxhc3RwcmljZWAgaW50ZXJmYWNlIHdoaWNoIHJldHVybnMgYSBwcmljZSByZWNvcmQgd2l0aCBhIHRpbWVzdGFtcC4KCk5vdGU6IFRoaXMgaXMgYSBtaW5pbWFsIGltcGxlbWVudGF0aW9uLiBJbiBwcm9kdWN0aW9uLCB5b3Ugd291bGQgaW52b2tlCnRoZSBvcmFjbGUgY29udHJhY3QncyBgbGFzdHByaWNlYCBtZXRob2QgYW5kIGV4dHJhY3QgdGhlIHRpbWVzdGFtcC4AAAAAD2dldF9vcmFjbGVfdGltZQAAAAAAAAAAAQAAAAY=",
        "AAAAAAAABABSZXNvbHZlIGEgcGVuZGluZyBkaXNwdXRlIGFuZCBzZXR0bGUgdGhlIHVuZGVybHlpbmcgZ2FtZSAoYXJiaXRyYXRvciBvbmx5KS4KCkF1dGhlbnRpY2F0ZXMgdGhlIGFyYml0cmF0b3IsIHByb2Nlc3NlcyB0aGUgcGF5b3V0IChgU29tZSh3aW5uZXIpYCBmb3IgYQp3aW4sIGBOb25lYCBmb3IgYSBkcmF3KSwgbWFya3MgdGhlIGdhbWUgYFNldHRsZWRgLCBhbmQgY2xvc2VzIHRoZQpkaXNwdXRlIGFzIGBSZXNvbHZlZGAuIEVtaXRzIGEgYCgiZGlzcHV0ZSIsICJzb2x2ZWQiKWAgZXZlbnQuCgojIFBhcmFtZXRlcnMKLSBgZGlzcHV0ZV9pZGAgICDigJQgSUQgb2YgYSBkaXNwdXRlIGluIGBQZW5kaW5nYCBzdGF0dXMuCi0gYGFyYml0cmF0b3JgICAg4oCUIE11c3QgbWF0Y2ggdGhlIHN0b3JlZCBhcmJpdHJhdG9yOyBtdXN0IGF1dGhvcmlzZSB0aGUgY2FsbC4KLSBgd2lubmVyYCAgICAgICDigJQgYFNvbWUoYWRkcmVzcylgIHRvIHBheSBvdXQgYSB3aW5uZXIsIG9yIGBOb25lYCB0bwpzcGxpdCBzdGFrZXMgZXF1YWxseSAoZHJhdykuCi0gYHJlc29sdXRpb25gICAg4oCUIEVuY29kZWQgcmVzb2x1dGlvbiByYXRpb25hbGUgKGFyYml0cmFyeSBvZmYtY2hhaW4gcGF5bG9hZCkuCgojIFJldHVybnMKYE9rKCgpKWAgb24gc3VjY2Vzcy4KCiMgRXJyb3JzCi0gW2BDb250cmFjdEVycm9yOjpOb3RBcmJpdHJhdG9yYF0gICAgICAgIOKAlCBgYXJiaXRyYXRvcmAgaXMgbm90IHRoZSBjb25maWd1cmVkIGFyYml0cmF0b3IsCm9yIHRoZSBhcmJpdHJhdG9yIGhhcyBub3QgYmVlbiBzZXQuCi0gW2BDb250cmFjdEVycm9yOjpEaXNwdXRlTm90Rm91bmRgXSAgICAgIOKAlCBgZGlzcHV0ZV9pZGAgZG9lcyBub3QgZXhpc3QuCi0gW2BDb250cmFjdEVycm9yOjpHYW1lQWxyZWFkeUNvbXBsZXRlZGBdIOKAlCBEaXNwdXRlIGlzIG5vdCBgUGVuZGluZ2AsIG9yIGdhbWUgaXMgbm90IGBJblByb2dyZXNzYC4KLSBbYENvbnRyYWN0RXJyb3I6OkdhbWVOb3RGb3VuZGBdICAgICAgICAg4oCUIFVuZGVybHlpbmcgZ2FtZSBkb2VzAAAAD3Jlc29sdmVfZGlzcHV0ZQAAAAAEAAAAAAAAAApkaXNwdXRlX2lkAAAAAAAGAAAAAAAAAAphcmJpdHJhdG9yAAAAAAATAAAAAAAAAAZ3aW5uZXIAAAAAA+gAAAATAAAAAAAAAApyZXNvbHV0aW9uAAAAAAAOAAAAAQAAA+kAAAPtAAAAAAAAB9AAAAANQ29udHJhY3RFcnJvcgAAAA==",
        "AAAAAgAAAAAAAAAAAAAADURpc3B1dGVTdGF0dXMAAAAAAAADAAAAAAAAAAAAAAAHUGVuZGluZwAAAAAAAAAAAAAAAAhSZXNvbHZlZAAAAAAAAAAAAAAACFJlamVjdGVk",
        "AAAAAQAAAAAAAAAAAAAADVBlbmFsdHlSZWNvcmQAAAAAAAAIAAAAAAAAAA9kZWNheV9yYXRlX2JpcHMAAAAABAAAAAAAAAAKZXhwaXJlc19hdAAAAAAABgAAAAAAAAAJaXNfYmFubmVkAAAAAAAAAQAAAAAAAAAJaXNzdWVkX2F0AAAAAAAABgAAAAAAAAAPb3JpZ2luYWxfYW1vdW50AAAAAAsAAAAAAAAADnBlbmFsdHlfYW1vdW50AAAAAAALAAAAAAAAAAZwbGF5ZXIAAAAAABMAAAAAAAAABnJlYXNvbgAAAAAADg==",
        "AAAAAAAAAghTZXQgdGhlIFNFUC00MCBvcmFjbGUgY29udHJhY3QgYWRkcmVzcyB1c2VkIGZvciBjbG9jayBzeW5jaHJvbmlzYXRpb24gKGFkbWluIG9ubHkpLgoKT25jZSBjb25maWd1cmVkLCBbYGdldF9vcmFjbGVfdGltZWBdIG1heSBjYWxsIHRoZSBvcmFjbGUgdmlhCmNyb3NzLWNvbnRyYWN0IGludm9jYXRpb24gdG8gb2J0YWluIGEgdHJ1c3RlZCB0aW1lc3RhbXAuCgojIFBhcmFtZXRlcnMKLSBgYWRtaW5gICAg4oCUIE11c3QgbWF0Y2ggYENPTlRSQUNUX0FETUlOYDsgbXVzdCBhdXRob3Jpc2UgdGhlIGNhbGwuCi0gYG9yYWNsZWAgIOKAlCBBZGRyZXNzIG9mIHRoZSBTRVAtNDAtY29tcGxpYW50IG9yYWNsZSBjb250cmFjdC4KCiMgUmV0dXJucwpgT2soKCkpYCBvbiBzdWNjZXNzLgoKIyBFcnJvcnMKLSBbYENvbnRyYWN0RXJyb3I6OlVuYXV0aG9yaXplZGBdIOKAlCBgYWRtaW5gIGRvZXMgbm90IG1hdGNoIGBDT05UUkFDVF9BRE1JTmAuCgojIFBhbmljcwotIElmIHRoZSBjb250cmFjdCBpcyBub3QgaW5pdGlhbGlzZWQuAAAAEGNvbmZpZ3VyZV9vcmFjbGUAAAACAAAAAAAAAAVhZG1pbgAAAAAAABMAAAAAAAAABm9yYWNsZQAAAAAAEwAAAAEAAAPpAAAD7QAAAAAAAAfQAAAADUNvbnRyYWN0RXJyb3IAAAA=",
        "AAAAAAAAAOxSZXR1cm4gdGhlIHBlbmRpbmcgZmVlIHByb3Bvc2FsLCBpZiBvbmUgZXhpc3RzLgoKIyBSZXR1cm5zCmBTb21lKEZlZVByb3Bvc2FsKWAg4oCUIHRoZSBwcm9wb3NhbCB3aXRoIGl0cyBgbmV3X2ZlZV9iaXBzYCwKYG5ld190cmVhc3VyeV9hZGRyZXNzYCwgYHByb3Bvc2VkX2F0YCBsZWRnZXIgc2VxdWVuY2UsIGFuZCBgcHJvcG9zZXJgLgpgTm9uZWAg4oCUIG5vIHByb3Bvc2FsIGlzIGN1cnJlbnRseSBwZW5kaW5nLgAAABBnZXRfZmVlX3Byb3Bvc2FsAAAAAAAAAAEAAAPoAAAH0AAAAAtGZWVQcm9wb3NhbAA=",
        "AAAAAAAAAbxCaW5kIHRoZSBTRVAtNDEgdG9rZW4gY29udHJhY3QgdXNlZCBmb3IgYWxsIHdhZ2VyIGVzY3JvdyBhbmQgcHJpemUgdHJhbnNmZXJzLgoKTXVzdCBiZSBjYWxsZWQgb25jZSBiZWZvcmUgYW55IGdhbWUgb3IgcmV3YXJkIGZ1bmN0aW9uLiBDYWxsaW5nIGl0IGEKc2Vjb25kIHRpbWUgcGFuaWNzIHdpdGggYCJDb250cmFjdCBhbHJlYWR5IGluaXRpYWxpemVkImAuCgojIFBhcmFtZXRlcnMKLSBgYWRtaW5gIOKAlCBBZGRyZXNzIHRoYXQgYXV0aG9yaXNlcyB0aGUgY2FsbCAoYHJlcXVpcmVfYXV0aGAgaXMgZW5mb3JjZWQpLgotIGB0b2tlbl9jb250cmFjdGAg4oCUIEFkZHJlc3Mgb2YgdGhlIFNFUC00MSB0b2tlbiBjb250cmFjdCAoZS5nLiBYTE0gb3IgVVNEQykuCgojIFBhbmljcwotIElmIGBUT0tFTl9DT05UUkFDVGAgaXMgYWxyZWFkeSBzZXQgaW4gaW5zdGFuY2Ugc3RvcmFnZS4AAAAQaW5pdGlhbGl6ZV90b2tlbgAAAAIAAAAAAAAABWFkbWluAAAAAAAAEwAAAAAAAAAOdG9rZW5fY29udHJhY3QAAAAAABMAAAAA",
        "AAAAAAAAACNRdWVyeSB0aGUgY3VycmVudCB0cmVhc3VyeSByZXNlcnZlLgAAAAAQdHJlYXN1cnlfYmFsYW5jZQAAAAAAAAABAAAACw==",
        "AAAAAQAAAAAAAAAAAAAADlRyb3BoeU1ldGFkYXRhAAAAAAAGAAAAAAAAAAtkZXNjcmlwdGlvbgAAAAAQAAAAAAAAAAlpbWFnZV91cmkAAAAAAAAQAAAAAAAAAARuYW1lAAAAEAAAAAAAAAAMdG90YWxfbWludGVkAAAABAAAAAAAAAAJdHJvcGh5X2lkAAAAAAAABgAAAAAAAAALdHJvcGh5X3R5cGUAAAAH0AAAAApUcm9waHlUeXBlAAA=",
        "AAAAAAAABABDbGFpbSBhIHdpbiBieSB0aW1lb3V0IHdoZW4gdGhlIG9wcG9uZW50IGhhcyBub3QgbW92ZWQgd2l0aGluIHRoZSBhbGxvd2VkIHdpbmRvdy4KCk9ubHkgdGhlICp3YWl0aW5nKiBwbGF5ZXIgKHRoZSBvbmUgd2hvIGlzIG5vdCByZXF1aXJlZCB0byBtb3ZlKSBtYXkKY2xhaW0gYSB0aW1lb3V0IHdpbi4gT25jZSBgY3VycmVudF9sZWRnZXIgLSBnYW1lLmxhc3RfbW92ZV9hdCDiiaUKdGltZW91dF9kdXJhdGlvbmAsIHRoYXQgcGxheWVyIGNhbGxzIHRoaXMgdG8gcmVjZWl2ZSB0aGUgbmV0IHBheW91dC4KCiMgUGFyYW1ldGVycwotIGBnYW1lX2lkYCAgIOKAlCBJRCBvZiBhIGdhbWUgaW4gYEluUHJvZ3Jlc3NgIHN0YXRlLgotIGBjbGFpbWFudGAgIOKAlCBUaGUgcGxheWVyIHdobyBpcyB3YWl0aW5nIGZvciB0aGUgb3Bwb25lbnQncyBtb3ZlOwptdXN0IGF1dGhvcmlzZSB0aGUgY2FsbC4KCiMgUmV0dXJucwpgT2soKCkpYCBvbiBzdWNjZXNzOyBnYW1lIGlzIG1hcmtlZCBgU2V0dGxlZGAuCgojIEVycm9ycwotIFtgQ29udHJhY3RFcnJvcjo6R2FtZU5vdEZvdW5kYF0gICAgICAgICAgIOKAlCBgZ2FtZV9pZGAgZG9lcyBub3QgZXhpc3QuCi0gW2BDb250cmFjdEVycm9yOjpHYW1lTm90SW5Qcm9ncmVzc2BdICAgICAg4oCUIEdhbWUgaXMgbm90IGBJblByb2dyZXNzYC4KLSBbYENvbnRyYWN0RXJyb3I6Ok5vdFBsYXllcmBdICAgICAgICAgICAgICDigJQgYGNsYWltYW50YCBpcyBub3QgYSBwYXJ0aWNpcGFudC4KLSBbYENvbnRyYWN0RXJyb3I6OkludmFsaWRUaW1lb3V0Q2xhaW1hbnRgXSDigJQgYGNsYWltYW50YCBpcyB0aGUgcGxheWVyIHdob3NlIHR1cm4gaXQgaXMgKG5vdCB0aGUgd2FpdGluZyBwbGF5ZXIpLgotIFtgQ29udHJhY3RFcnJvcjo6VGltZW91dE5vdENvbmZpZ3VyZWRgXSAgIOKAlCBbYGNvbmZpZ3VyZV90aW1lb3V0YF0gaGFzIG5vdCBiZWVuIGNhbGxlZC4KLSBbYENvbnRyYWN0RXJyb3I6OlRpbWVvdXROb3RSZWFjaGVkYF0gICAgICDigJQgRWxhcHNlZCBsZWRnZXJzIDwgYHRpbWVvAAAAEWNsYWltX3RpbWVvdXRfd2luAAAAAAAAAgAAAAAAAAAHZ2FtZV9pZAAAAAAGAAAAAAAAAAhjbGFpbWFudAAAABMAAAABAAAD6QAAA+0AAAAAAAAH0AAAAA1Db250cmFjdEVycm9yAAAA",
        "AAAAAAAAAiJTZXQgdGhlIGluYWN0aXZpdHkgdGltZW91dCB1c2VkIGJ5IFtgY2xhaW1fdGltZW91dF93aW5gXSAoYWRtaW4gb25seSkuCgpUaGUgdGltZW91dCBpcyBtZWFzdXJlZCBpbiBsZWRnZXIgc2VxdWVuY2VzICjiiYggNSBzIGVhY2ggb24gU3RlbGxhcgptYWlubmV0KS4gQSBwbGF5ZXIgbWF5IGNsYWltIGEgdGltZW91dCB3aW4gb25jZSBgY3VycmVudF9sZWRnZXIgLQpnYW1lLmxhc3RfbW92ZV9hdCDiiaUgdGltZW91dF9kdXJhdGlvbmAuCgojIFBhcmFtZXRlcnMKLSBgYWRtaW5gICAgICDigJQgTXVzdCBtYXRjaCBgQ09OVFJBQ1RfQURNSU5gOyBtdXN0IGF1dGhvcmlzZSB0aGUgY2FsbC4KLSBgZHVyYXRpb25gICDigJQgTGVkZ2VyIHNlcXVlbmNlcyBvZiBpbmFjdGl2aXR5IGJlZm9yZSBhIHRpbWVvdXQgd2luIGlzCmNsYWltYWJsZTsgbXVzdCBiZSA+IDAuCgojIFBhbmljcwotIElmIHRoZSBjb250cmFjdCBpcyBub3QgaW5pdGlhbGlzZWQuCi0gSWYgYGFkbWluYCBkb2VzIG5vdCBtYXRjaCB0aGUgc3RvcmVkIGFkbWluLgotIElmIGBkdXJhdGlvbiA9PSAwYC4AAAAAABFjb25maWd1cmVfdGltZW91dAAAAAAAAAIAAAAAAAAABWFkbWluAAAAAAAAEwAAAAAAAAAIZHVyYXRpb24AAAAGAAAAAA==",
        "AAAAAAAAADxSZXR1cm5zIHRydWUgaWYgYGFjY291bnRgIGhhcyBjb21wbGV0ZWQgU0VQLTEwIHZlcmlmaWNhdGlvbi4AAAARaXNfc2VwMTBfdmVyaWZpZWQAAAAAAAABAAAAAAAAAAdhY2NvdW50AAAAABMAAAABAAAAAQ==",
        "AAAAAAAABABEaXN0cmlidXRlIGEgY29tcGxldGVkIGdhbWUncyBwcml6ZSBwb29sIGFjcm9zcyBtdWx0aXBsZSB0b3VybmFtZW50IHdpbm5lcnMuCgpEZWR1Y3RzIGJvdGggcGxheWVycycgZXNjcm93IGJhbGFuY2VzLCB0aGVuIGRpc3RyaWJ1dGVzIHRoZSB0b3RhbCBwb29sCnByb3BvcnRpb25hbGx5IGFjY29yZGluZyB0byBgcGVyY2VudGFnZXNgLiBJbnRlZ2VyLWRpdmlzaW9uIGR1c3QgZ29lcyB0bwp0aGUgZmlyc3Qgd2lubmVyLiBSZXF1aXJlcyBgcGxheWVyMWAncyBhdXRob3Jpc2F0aW9uIGFzIHRoZSB0b3VybmFtZW50Cm9yZ2FuaXNlci4KClByZWZlciBbYHBheW91dF90b3VybmFtZW50X29wdGltaXplZGBdIGZvciBsb3dlciBnYXMgY29zdDsgdGhpcyB2YXJpYW50CmRvZXMgc2VwYXJhdGUgZXNjcm93IHJlYWRzIHBlciB3aW5uZXIuCgojIFBhcmFtZXRlcnMKLSBgZ2FtZV9pZGAgICAgICDigJQgSUQgb2YgYSBnYW1lIGluIGBDb21wbGV0ZWRgIHN0YXRlLgotIGB3aW5uZXJzYCAgICAgIOKAlCBPcmRlcmVkIHJlY2lwaWVudCBhZGRyZXNzZXMuCi0gYHBlcmNlbnRhZ2VzYCAg4oCUIFdob2xlLW51bWJlciBwZXJjZW50YWdlcyBwYXJhbGxlbCB0byBgd2lubmVyc2A7Cm11c3Qgc3VtIHRvIGV4YWN0bHkgMTAwLgoKIyBSZXR1cm5zCmBPaygoKSlgIG9uIHN1Y2Nlc3MuCgojIEVycm9ycwotIFtgQ29udHJhY3RFcnJvcjo6R2FtZU5vdEZvdW5kYF0gICAgICAgICAg4oCUIGBnYW1lX2lkYCBkb2VzIG5vdCBleGlzdC4KLSBbYENvbnRyYWN0RXJyb3I6OkdhbWVOb3RJblByb2dyZXNzYF0gICAgIOKAlCBHYW1lIGlzIG5vdCBgQ29tcGxldGVkYC4KLSBbYENvbnRyYWN0RXJyb3I6Ok1pc21hdGNoZWRMZW5ndGhzYF0gICAgIOKAlCBgd2lubmVyc2AgYW5kIGBwZXJjZW50YWdlc2AgZGlmZmVyLgotIFtgQ29udHJhY3RFcnJvcjo6SW5zdWZmaWNpZW50RnVuZHNgXSAgICAg4oCUIEVzY3JvdyBiYWxhbmNlIGlzIGxvd2VyIHRoYW4gdGhlIHdhZ2VyLgotIFtgQ29udHJhY3RFcnJvcjo6SW52YWxpZFBlcmNlbnRhZ2VgAAAAEXBheW91dF90b3VybmFtZW50AAAAAAAAAwAAAAAAAAAHZ2FtZV9pZAAAAAAGAAAAAAAAAAd3aW5uZXJzAAAAA+oAAAATAAAAAAAAAAtwZXJjZW50YWdlcwAAAAPqAAAABAAAAAEAAAPpAAAD7QAAAAAAAAfQAAAADUNvbnRyYWN0RXJyb3IAAAA=",
        "AAAAAAAAAJZNYW51YWxseSBzZXQgYSBwbGF5ZXIncyByYXRpbmcgKGFkbWluIG9ubHkpLgoKVGhpcyBpcyB1c2VmdWwgZm9yOgotIENvcnJlY3RpbmcgZXJyb25lb3VzIHJhdGluZ3MKLSBSZXNldHRpbmcgbmV3IGFjY291bnRzCi0gQWRtaW5pc3RyYXRpdmUgY29ycmVjdGlvbnMAAAAAABFzZXRfcGxheWVyX3JhdGluZwAAAAAAAAMAAAAAAAAABWFkbWluAAAAAAAAEwAAAAAAAAAGcGxheWVyAAAAAAATAAAAAAAAAApuZXdfcmF0aW5nAAAAAAAFAAAAAQAAA+kAAAPtAAAAAAAAB9AAAAANQ29udHJhY3RFcnJvcgAAAA==",
        "AAAAAAAAAxdDb25maWd1cmUgdGhlIG11bHRpLXNpZyBzaWduZXIgc2V0IGFuZCBhcHByb3ZhbCB0aHJlc2hvbGQgKGFkbWluIG9ubHkpLgoKUmVwbGFjZXMgYW55IGV4aXN0aW5nIHNpZ25lciBjb25maWd1cmF0aW9uLiBPbmNlIHNldCwgZmVlIGNoYW5nZXMKcHJvcG9zZWQgdmlhIFtgcHJvcG9zZV9mZWVfY2hhbmdlYF0gcmVxdWlyZSBhdCBsZWFzdCBgdGhyZXNob2xkYAphcHByb3ZhbHMgZnJvbSBgc2lnbmVyc2AgYmVmb3JlIHRha2luZyBlZmZlY3QuCgojIFBhcmFtZXRlcnMKLSBgYWRtaW5gICAgICAg4oCUIE11c3QgbWF0Y2ggYENPTlRSQUNUX0FETUlOYDsgbXVzdCBhdXRob3Jpc2UgdGhlIGNhbGwuCi0gYHNpZ25lcnNgICAgIOKAlCBFeGhhdXN0aXZlIGxpc3Qgb2YgYWRkcmVzc2VzIGF1dGhvcmlzZWQgdG8gcHJvcG9zZSBhbmQKYXBwcm92ZSBmZWUgY2hhbmdlcy4KLSBgdGhyZXNob2xkYCAg4oCUIE1pbmltdW0gbnVtYmVyIG9mIGFwcHJvdmFscyByZXF1aXJlZDsgbXVzdCBiZSDiiaUgMSBhbmQK4omkIGBzaWduZXJzLmxlbigpYC4KCiMgUmV0dXJucwpgT2soKCkpYCBvbiBzdWNjZXNzLgoKIyBFcnJvcnMKLSBbYENvbnRyYWN0RXJyb3I6OlVuYXV0aG9yaXplZGBdICAgIOKAlCBgYWRtaW5gIGRvZXMgbm90IG1hdGNoIGBDT05UUkFDVF9BRE1JTmAuCi0gW2BDb250cmFjdEVycm9yOjpJbnZhbGlkVGhyZXNob2xkYF0g4oCUIGB0aHJlc2hvbGQgPT0gMGAgb3IgYHRocmVzaG9sZCA+IHNpZ25lcnMubGVuKClgLgoKIyBQYW5pY3MKLSBJZiB0aGUgY29udHJhY3QgaXMgbm90IGluaXRpYWxpc2VkLgAAAAASY29uZmlndXJlX211bHRpc2lnAAAAAAADAAAAAAAAAAVhZG1pbgAAAAAAABMAAAAAAAAAB3NpZ25lcnMAAAAD6gAAABMAAAAAAAAACXRocmVzaG9sZAAAAAAAAAQAAAABAAAD6QAAA+0AAAAAAAAH0AAAAA1Db250cmFjdEVycm9yAAAA",
        "AAAAAAAAAI9SZXR1cm4gdGhlIG51bWJlciBvZiBzaWduZXJzIHdobyBoYXZlIGFwcHJvdmVkIHRoZSBwZW5kaW5nIHByb3Bvc2FsLgoKUmV0dXJucyBgMGAgaWYgbm8gcHJvcG9zYWwgaXMgcGVuZGluZyBvciBubyBhcHByb3ZhbHMgaGF2ZSBiZWVuIHJlY29yZGVkLgAAAAASZ2V0X2FwcHJvdmFsX2NvdW50AAAAAAAAAAAAAQAAAAQ=",
        "AAAAAAAAAF5SZXRyaWV2ZSBhIHBsYXllciBwcm9maWxlIGZyb20gcGVyc2lzdGVudCBzdG9yYWdlLgoKUmV0dXJucyBOb25lIGlmIHBsYXllciBoYXMgbm8gcHJvZmlsZSB5ZXQuAAAAAAASZ2V0X3BsYXllcl9wcm9maWxlAAAAAAABAAAAAAAAAAZwbGF5ZXIAAAAAABMAAAABAAAD6AAAB9AAAAAMUGxheWVyUmF0aW5n",
        "AAAAAAAAAzxQcm9wb3NlIGEgbmV3IGZlZSBjb25maWd1cmF0aW9uIChhbnkgcmVnaXN0ZXJlZCBzaWduZXIpLgoKUmVwbGFjZXMgYW55IGV4aXN0aW5nIHBlbmRpbmcgcHJvcG9zYWwgYW5kIHJlc2V0cyB0aGUgYXBwcm92YWwgbWFwLgpUaGUgcHJvcG9zZXIncyBhcHByb3ZhbCBpcyBjb3VudGVkIGF1dG9tYXRpY2FsbHksIHNvIGEgc2luZ2xlLXNpZ25lcgpzZXR1cCB3aXRoIGB0aHJlc2hvbGQgPSAxYCB3aWxsIGV4ZWN1dGUgaW1tZWRpYXRlbHkuCgojIFBhcmFtZXRlcnMKLSBgcHJvcG9zZXJgICAgICAgICAgICAgICDigJQgTXVzdCBiZSBpbiB0aGUgY29uZmlndXJlZCBzaWduZXIgc2V0OyBtdXN0IGF1dGhvcmlzZSB0aGUgY2FsbC4KLSBgbmV3X2ZlZV9iaXBzYCAgICAgICAgICDigJQgUHJvcG9zZWQgZmVlIGluIGJhc2lzLXBvaW50cyBvZiAxIDAwMCAoMOKAkzEgMDAwKS4KLSBgbmV3X3RyZWFzdXJ5X2FkZHJlc3NgICDigJQgUHJvcG9zZWQgcmVjaXBpZW50IGFkZHJlc3MgZm9yIHRoZSBmZWUgcG9ydGlvbi4KCiMgUmV0dXJucwpgT2soKCkpYCBvbiBzdWNjZXNzLgoKIyBFcnJvcnMKLSBbYENvbnRyYWN0RXJyb3I6OkludmFsaWRBbW91bnRgXSDigJQgYG5ld19mZWVfYmlwcyA+IDEwMDBgLgotIFtgQ29udHJhY3RFcnJvcjo6Tm90QVNpZ25lcmBdICAgIOKAlCBgcHJvcG9zZXJgIGlzIG5vdCBpbiB0aGUgc2lnbmVyIHNldC4KLSBbYENvbnRyYWN0RXJyb3I6OlVuYXV0aG9yaXplZGBdICDigJQgTXVsdGktc2lnIGhhcyBub3QgYmVlbiBjb25maWd1cmVkIHlldC4KCiMgRXZlbnRzCkVtaXRzIGAoIm11bHRpc2lnIiwgInByb3Bvc2VkIikg4oaSIHByb3Bvc2VyYC4AAAAScHJvcG9zZV9mZWVfY2hhbmdlAAAAAAADAAAAAAAAAAhwcm9wb3NlcgAAABMAAAAAAAAADG5ld19mZWVfYmlwcwAAAAQAAAAAAAAAFG5ld190cmVhc3VyeV9hZGRyZXNzAAAAEwAAAAEAAAPpAAAD7QAAAAAAAAfQAAAADUNvbnRyYWN0RXJyb3IAAAA=",
        "AAAAAAAAAflVcGRhdGUgdGhlIG1heGltdW0gY29tYmluZWQgcHJpemUgcG9vbCBhY3Jvc3MgYm90aCBwbGF5ZXJzIChhZG1pbiBvbmx5KS4KCkd1YXJkcyBhZ2FpbnN0IHZlcnkgbGFyZ2UgcG9vbHMuIEFueSBbYGNyZWF0ZV9nYW1lYF0gb3IgW2Bqb2luX2dhbWVgXQpjYWxsIHdoZXJlIGB3YWdlcl9hbW91bnQgKiAyID4gTUFYX1BSSVpFX1BPT0xgIHdpbGwgcmV0dXJuCltgQ29udHJhY3RFcnJvcjo6UHJpemVQb29sTGltaXRFeGNlZWRlZGBdLgoKIyBQYXJhbWV0ZXJzCi0gYGFkbWluYCAgICAgIOKAlCBNdXN0IG1hdGNoIGBDT05UUkFDVF9BRE1JTmA7IG11c3QgYXV0aG9yaXNlIHRoZSBjYWxsLgotIGBuZXdfbGltaXRgICDigJQgTmV3IG1heGltdW0gcHJpemUgcG9vbDsgbXVzdCBiZSA+IDAuCgojIFBhbmljcwotIElmIHRoZSBjb250cmFjdCBpcyBub3QgaW5pdGlhbGl6ZWQuCi0gSWYgYGFkbWluYCBkb2VzIG5vdCBtYXRjaCB0aGUgc3RvcmVkIGFkbWluLgotIElmIGBuZXdfbGltaXQgPD0gMGAuAAAAAAAAEnNldF9tYXhfcHJpemVfcG9vbAAAAAAAAgAAAAAAAAAFYWRtaW4AAAAAAAATAAAAAAAAAAluZXdfbGltaXQAAAAAAAALAAAAAA==",
        "AAAAAQAAAAAAAAAAAAAAEEJvdW50eVN1Ym1pc3Npb24AAAAEAAAAAAAAAAlwdXp6bGVfaWQAAAAAAAAGAAAAAAAAAA1zb2x1dGlvbl9oYXNoAAAAAAAD7gAAACAAAAAAAAAABnNvbHZlcgAAAAAAEwAAAAAAAAAMc3VibWl0dGVkX2F0AAAABg==",
        "AAAAAQAAAAAAAAAAAAAAEFRvdXJuYW1lbnRFc2Nyb3cAAAAGAAAAAAAAAAllc2Nyb3dfaWQAAAAAAAAGAAAAAAAAAAdnYW1lX2lkAAAAAAYAAAAAAAAADGxvY2tlZF91bnRpbAAAAAYAAAAAAAAABnBsYXllcgAAAAAAEwAAAAAAAAAIcmVsZWFzZWQAAAABAAAAAAAAAAx0b3RhbF9hbW91bnQAAAAL",
        "AAAAAAAAAjFDYW5jZWwgdGhlIHBlbmRpbmcgZmVlIHByb3Bvc2FsIChhbnkgcmVnaXN0ZXJlZCBzaWduZXIpLgoKUmVtb3ZlcyB0aGUgcHJvcG9zYWwgYW5kIGNsZWFycyBhbGwgYXBwcm92YWxzLiBBbnkgc2lnbmVyIG1heSBjYW5jZWwsCm5vdCBvbmx5IHRoZSBvcmlnaW5hbCBwcm9wb3Nlci4KCiMgUGFyYW1ldGVycwotIGBzaWduZXJgIOKAlCBNdXN0IGJlIGluIHRoZSBjb25maWd1cmVkIHNpZ25lciBzZXQ7IG11c3QgYXV0aG9yaXNlIHRoZSBjYWxsLgoKIyBSZXR1cm5zCmBPaygoKSlgIG9uIHN1Y2Nlc3MuCgojIEVycm9ycwotIFtgQ29udHJhY3RFcnJvcjo6Tm90QVNpZ25lcmBdICAg4oCUIGBzaWduZXJgIGlzIG5vdCBpbiB0aGUgc2lnbmVyIHNldC4KLSBbYENvbnRyYWN0RXJyb3I6OlVuYXV0aG9yaXplZGBdIOKAlCBNdWx0aS1zaWcgaGFzIG5vdCBiZWVuIGNvbmZpZ3VyZWQuCi0gW2BDb250cmFjdEVycm9yOjpOb1Byb3Bvc2FsYF0gICDigJQgTm8gcGVuZGluZyBwcm9wb3NhbCB0byBjYW5jZWwuCgojIEV2ZW50cwpFbWl0cyBgKCJtdWx0aXNpZyIsICJjYW5jZWwiKSDihpIgc2lnbmVyYC4AAAAAAAATY2FuY2VsX2ZlZV9wcm9wb3NhbAAAAAABAAAAAAAAAAZzaWduZXIAAAAAABMAAAABAAAD6QAAA+0AAAAAAAAH0AAAAA1Db250cmFjdEVycm9yAAAA",
        "AAAAAAAABABDbGFpbSBhIHNpbmdsZSBwdXp6bGUgcmV3YXJkIGJhY2tlZCBieSBhIGJhY2tlbmQgRUQyNTUxOSBzaWduYXR1cmUuCgpWZXJpZmllcyB0aGUgc2lnbmF0dXJlLCBndWFyZHMgYWdhaW5zdCBub25jZSByZXBsYXksIGRlZHVjdHMKYHJld2FyZF9hbW91bnRgIGZyb20gdGhlIHRyZWFzdXJ5LCBhbmQgY3JlZGl0cyB0aGUgcmVjaXBpZW50J3MKcHV6emxlLXJld2FyZCBiYWxhbmNlLiBFbWl0cyBhIGBwemxfcndkYCBldmVudCBvbiBzdWNjZXNzLgoKIyBTaWduYXR1cmUgUGF5bG9hZApgU0hBMjU2KCByZWNpcGllbnRfYWRkcmVzc19zdHJpbmdfYnl0ZXMgfHwgcmV3YXJkX2Ftb3VudF9pNjRfbGU4IHx8IG5vbmNlX3U2NF9sZTggKWAKCiMgUGFyYW1ldGVycwotIGByZWNpcGllbnRgICAgICAg4oCUIEFkZHJlc3MgdG8gY3JlZGl0OyBtdXN0IGJlIHRoZSBvbmUgZW5jb2RlZCBpbiB0aGUgc2lnbmF0dXJlLgotIGByZXdhcmRfYW1vdW50YCAg4oCUIFRva2VuIGFtb3VudCB0byBhd2FyZDsgbXVzdCBiZSA+IDAgYW5kIOKJpCBgaTY0OjpNQVhgLgotIGBub25jZWAgICAgICAgICAg4oCUIFVuaXF1ZSBwZXItY2xhaW0gYHU2NGAgbm9uY2U7IHByZXZlbnRzIHJlcGxheSBhdHRhY2tzLgotIGBzaWduYXR1cmVgICAgICAg4oCUIDY0LWJ5dGUgRUQyNTUxOSBzaWduYXR1cmUgZnJvbSB0aGUgYmFja2VuZCBzZXJ2aWNlLgoKIyBSZXR1cm5zCmBPaygoKSlgIG9uIHN1Y2Nlc3MuCgojIEVycm9ycwotIFtgQ29udHJhY3RFcnJvcjo6SW52YWxpZEFtb3VudGBdICDigJQgYHJld2FyZF9hbW91bnQg4omkIDBgIG9yIGA+IGk2NDo6TUFYYC4KLSBbYENvbnRyYWN0RXJyb3I6OlVuYXV0aG9yaXplZGBdICAg4oCUIGBub25jZWAgaGFzIGFscmVhZHkgYmVlbiB1c2VkLgoKIyBQYW5pY3MKLSBJZiB0aGUgRUQyNTUxOSBzaWduYXR1cmUgaXMgaW52YWxpZC4KLSBJZiBgQURNSU5fS0VZYCBpcyBhYnNlbnQgKGNvbnRyYWN0IG5vdCBpbml0aWFsaXNlZCkuCi0gSWYgdGhlIHRyZWFzdXJ5IGhhcyBpbnN1ZmZpY2llbnQgYmFsYW5jAAAAE2NsYWltX3B1enpsZV9yZXdhcmQAAAAABAAAAAAAAAAJcmVjaXBpZW50AAAAAAAAEwAAAAAAAAANcmV3YXJkX2Ftb3VudAAAAAAAAAsAAAAAAAAABW5vbmNlAAAAAAAABgAAAAAAAAAJc2lnbmF0dXJlAAAAAAAD7gAAAEAAAAABAAAD6QAAA+0AAAAAAAAH0AAAAA1Db250cmFjdEVycm9yAAAA",
        "AAAAAAAAAHlBY2NlcHQgYSBwZW5kaW5nIGFkbWluIGtleSBwcm9wb3NhbCBhZnRlciB0aGUgdGltZWxvY2sgZXhwaXJlcy4KQW55b25lIGNhbiBjYWxsIHRoaXMgb25jZSB0aGUgMjQtaG91ciB3aW5kb3cgaGFzIGVsYXBzZWQuAAAAAAAAFGFjY2VwdF9uZXdfYWRtaW5fa2V5AAAAAAAAAAEAAAPpAAAD7QAAAAAAAAfQAAAADUNvbnRyYWN0RXJyb3IAAAA=",
        "AAAAAAAAA4hBcHByb3ZlIHRoZSBwZW5kaW5nIGZlZSBwcm9wb3NhbCAoYW55IHJlZ2lzdGVyZWQgc2lnbmVyKS4KClJlY29yZHMgdGhlIHNpZ25lcidzIGFwcHJvdmFsLiBJZiB0aGUgcnVubmluZyBhcHByb3ZhbCBjb3VudCByZWFjaGVzCnRoZSBjb25maWd1cmVkIHRocmVzaG9sZCwgdGhlIGZlZSBjaGFuZ2UgaXMgYXBwbGllZCBpbW1lZGlhdGVseSBhbmQKdGhlIHByb3Bvc2FsIGlzIGNsZWFyZWQuCgojIFBhcmFtZXRlcnMKLSBgc2lnbmVyYCDigJQgTXVzdCBiZSBpbiB0aGUgY29uZmlndXJlZCBzaWduZXIgc2V0OyBtdXN0IGF1dGhvcmlzZSB0aGUgY2FsbC4KCiMgUmV0dXJucwpgT2sodHJ1ZSlgIOKAlCB0aHJlc2hvbGQgcmVhY2hlZCwgcHJvcG9zYWwgZXhlY3V0ZWQgYW5kIGNsZWFyZWQuCmBPayhmYWxzZSlgIOKAlCBhcHByb3ZhbCByZWNvcmRlZCwgc3RpbGwgYXdhaXRpbmcgbW9yZSBzaWduZXJzLgoKIyBFcnJvcnMKLSBbYENvbnRyYWN0RXJyb3I6Ok5vdEFTaWduZXJgXSAgICDigJQgYHNpZ25lcmAgaXMgbm90IGluIHRoZSBzaWduZXIgc2V0LgotIFtgQ29udHJhY3RFcnJvcjo6VW5hdXRob3JpemVkYF0gIOKAlCBNdWx0aS1zaWcgaGFzIG5vdCBiZWVuIGNvbmZpZ3VyZWQuCi0gW2BDb250cmFjdEVycm9yOjpOb1Byb3Bvc2FsYF0gICAg4oCUIE5vIHBlbmRpbmcgcHJvcG9zYWwgZXhpc3RzLgotIFtgQ29udHJhY3RFcnJvcjo6QWxyZWFkeUFwcHJvdmVkYF0g4oCUIGBzaWduZXJgIGhhcyBhbHJlYWR5IGFwcHJvdmVkIHRoaXMgcHJvcG9zYWwuCgojIEV2ZW50cwpPbiBwYXJ0aWFsIGFwcHJvdmFsOiBlbWl0cyBgKCJtdWx0aXNpZyIsICJhcHByb3ZlZCIpIOKGkiBzaWduZXJgLgpPbiBleGVjdXRpb246IGVtaXRzIGAoIm11bHRpc2lnIiwgImV4ZWN1dGVkIikg4oaSIG5ld19mZWVfYmlwc2AuAAAAFGFwcHJvdmVfZmVlX3Byb3Bvc2FsAAAAAQAAAAAAAAAGc2lnbmVyAAAAAAATAAAAAQAAA+kAAAABAAAH0AAAAA1Db250cmFjdEVycm9yAAAA",
        "AAAAAAAAALZBZGQgYSB0b2tlbiBhZGRyZXNzIHRvIHRoZSB3aGl0ZWxpc3QuCkF1dGhvcmlzZWQgYnkgdGhlIGBhZG1pbmAgYWRkcmVzcyDigJQgdGhlIGNvbnRyYWN0IGFkbWluIG9uY2UKYGluaXRpYWxpemVfcHV6emxlX3Jld2FyZHNgIGhhcyBiZWVuIGNhbGxlZCwgb3IgYW55IGF1dGhvcmlzZWQgY2FsbGVyCmJlZm9yZSB0aGF0LgAAAAAAFWFkZF93aGl0ZWxpc3RlZF90b2tlbgAAAAAAAAIAAAAAAAAABWFkbWluAAAAAAAAEwAAAAAAAAAFdG9rZW4AAAAAAAATAAAAAA==",
        "AAAAAAAABABDbGFpbSB2aWN0b3J5IHdoZW4gdGhlIG9wcG9uZW50IGhhcyBmYWlsZWQgdG8gcmVzcG9uZCB3aXRoaW4gdGhlCmNvbmZpZ3VyZWQgZGVhZGxpbmUgKipwbHVzKiogZ3JhY2UgYnVmZmVyIChTQy01MCkuCgpUaGlzIGlzIHRoZSBzdHJpY3QsIGxlZGdlci0qdGltZXN0YW1wKi1iYXNlZCBzaWJsaW5nIG9mCltgY2xhaW1fdGltZW91dF93aW5gXS4gSXQgZXhpc3RzIHNvIHRoYXQgYSBwbGF5ZXIgd2hvIGlzIHN0aWxsCmFjdGl2ZWx5IGVuZ2FnZWQgY2Fubm90IGJlIGhlbGQgaG9zdGFnZSBieSBhbiBvcHBvbmVudCB3aG8gcmVhbGl6ZXMKdGhleSdyZSBsb3NpbmcgYW5kIHNpbXBseSBnb2VzIG9mZmxpbmUgaW5zdGVhZCBvZiByZXNpZ25pbmc6IG9uY2UKYGVudi5sZWRnZXIoKS50aW1lc3RhbXAoKSAtIGdhbWUubGFzdF9hY3Rpdml0eV90cyA+PSBkZWFkbGluZV9zZWNzICsKYnVmZmVyX3NlY3NgLCB0aGUgd2FpdGluZyBwbGF5ZXIgbWF5IHNldHRsZSB0aGUgZ2FtZSBhbmQgcmVjZWl2ZSB0aGUKZnVsbCBlc2Nyb3dlZCB3YWdlciAqKmltbWVkaWF0ZWx5IOKAlCBubyBiYWNrZW5kIHNpZ25hdHVyZSByZXF1aXJlZCoqLgoKVGhlIGBidWZmZXJfc2Vjc2AgZ3JhY2UgcGVyaW9kIHNwZWNpZmljYWxseSBjb3ZlcnMgYSAqKnRlbXBvcmFyeQpiYWNrZW5kL3NlcnZlciBvdXRhZ2UqKjogZXZlbiBpZiB0aGUgb2ZmLWNoYWluIHNlcnZpY2UgdGhhdCB3b3VsZApub3JtYWxseSBhcmJpdHJhdGUgYSBgY2xhaW1fd2luYC9gY2xhaW1fZHJhd2AgaXMgYnJpZWZseSBkb3duLCBhCmZvcmNlZCBmb3JmZWl0IG9ubHkgYmVjb21lcyBjbGFpbWFibGUgYWZ0ZXIgYGRlYWRsaW5lX3NlY3NgIGhhcwoqYWxzbyogZWxhcHNlZCBvbiB0b3Agb2YgdGhhdCwgZ2l2aW5nIHRoZSBzZXJ2ZXIgYSBmYWlyIHdpbmRvdyB0bwpyZWNvdmVyIGJlZm9yZSB0aGUgdHJ1c3RsZXNzIG9uLWNoYWluIHBhdGggb3BlbnMgdXAuIEEgY2xhaW0KYXR0ZW1wdGVkIGFmdGVyIGBkZWFkbGluZV9zZWNzYCBhbG9uZSBidXQgYmVmb3JlIHRoZSBidWZmAAAAFWNsYWltX3RpbWVvdXRfdmljdG9yeQAAAAAAAAIAAAAAAAAAB2dhbWVfaWQAAAAABgAAAAAAAAAIY2xhaW1hbnQAAAATAAAAAQAAA+kAAAPtAAAAAAAAB9AAAAANQ29udHJhY3RFcnJvcgAAAA==",
        "AAAAAAAAAe5RdWVyeSB0aGUgbGVkZ2VyIHNlcXVlbmNlcyByZW1haW5pbmcgYmVmb3JlIHRoZSBhY3RpdmUgcGxheWVyJ3MgdHVybiB0aW1lcyBvdXQuCgpSZXR1cm5zIGBOb25lYCBpZiB0aGUgZ2FtZSBkb2VzIG5vdCBleGlzdCwgaXMgbm90IGBJblByb2dyZXNzYCwgb3IgaWYKbm8gdGltZW91dCBoYXMgYmVlbiBjb25maWd1cmVkLiBSZXR1cm5zIGBTb21lKDApYCB3aGVuIHRoZSB0aW1lb3V0IGhhcwphbHJlYWR5IGVsYXBzZWQgKGEgdGltZW91dCB3aW4gaXMgY2xhaW1hYmxlKS4KCiMgUGFyYW1ldGVycwotIGBnYW1lX2lkYCDigJQgSUQgb2YgdGhlIGdhbWUgdG8gcXVlcnkuCgojIFJldHVybnMKYFNvbWUocmVtYWluaW5nX3NlcXVlbmNlcylgIOKAlCBzZXF1ZW5jZXMgdW50aWwgdGltZW91dCwgb3IgYFNvbWUoMClgIGlmIGFscmVhZHkgZWxhcHNlZC4KYE5vbmVgIOKAlCBnYW1lIG5vdCBmb3VuZCwgbm90IGluIHByb2dyZXNzLCBvciB0aW1lb3V0IG5vdCBjb25maWd1cmVkLgAAAAAAFWdldF90aW1lb3V0X3JlbWFpbmluZwAAAAAAAAEAAAAAAAAAB2dhbWVfaWQAAAAABgAAAAEAAAPoAAAABg==",
        "AAAAAAAAAVJGZXRjaCBhIHRvdXJuYW1lbnQgZXNjcm93IGJ5IElELgoKIyBQYXJhbWV0ZXJzCi0gYGVzY3Jvd19pZGAg4oCUIE51bWVyaWMgSUQgcmV0dXJuZWQgYnkgW2BjcmVhdGVfdG91cm5hbWVudF9lc2Nyb3dgXS4KCiMgUmV0dXJucwpgT2soVG91cm5hbWVudEVzY3JvdylgIOKAlCB0aGUgZnVsbCBlc2Nyb3cgcmVjb3JkIGluY2x1ZGluZyBgbG9ja2VkX3VudGlsYAphbmQgYHJlbGVhc2VkYCBmbGFnLgoKIyBFcnJvcnMKLSBbYENvbnRyYWN0RXJyb3I6OkVzY3Jvd05vdEZvdW5kYF0g4oCUIGBlc2Nyb3dfaWRgIGRvZXMgbm90IGV4aXN0IG9yIG5vCmVzY3Jvd3MgaGF2ZSBiZWVuIGNyZWF0ZWQgeWV0LgAAAAAAFWdldF90b3VybmFtZW50X2VzY3JvdwAAAAAAAAEAAAAAAAAACWVzY3Jvd19pZAAAAAAAAAYAAAABAAAD6QAAB9AAAAAQVG91cm5hbWVudEVzY3JvdwAAB9AAAAANQ29udHJhY3RFcnJvcgAAAA==",
        "AAAAAAAAA9xJc3N1ZSBhIFNFUC0xMCBXZWIgQXV0aGVudGljYXRpb24gY2hhbGxlbmdlIGZvciBgYWNjb3VudGAgKGFkbWluIG9ubHkpLgoKU3RvcmVzIGBub25jZSDihpIgZXhwaXJ5YCBpbiBjb250cmFjdCBzdG9yYWdlLiBUaGUgdXNlciBtdXN0IHN1YnNlcXVlbnRseQpjYWxsIFtgdmVyaWZ5X3NlcDEwX2NoYWxsZW5nZWBdIHdpdGggYSB2YWxpZCBFRDI1NTE5IHNpZ25hdHVyZSBvdmVyIHRoZQpjaGFsbGVuZ2UgcGF5bG9hZCB0byBiZWNvbWUgdmVyaWZpZWQuCgojIFBhcmFtZXRlcnMKLSBgYWRtaW5gICAgIOKAlCBNdXN0IG1hdGNoIGBDT05UUkFDVF9BRE1JTmA7IG11c3QgYXV0aG9yaXNlIHRoZSBjYWxsLgotIGBhY2NvdW50YCAg4oCUIFN0ZWxsYXIgYWNjb3VudCBhZGRyZXNzIHRoZSBjaGFsbGVuZ2UgaXMgaXNzdWVkIGZvci4KLSBgbm9uY2VgICAgIOKAlCAzMi1ieXRlIHJhbmRvbSB2YWx1ZTsgbXVzdCBiZSBnbG9iYWxseSB1bmlxdWUgKHJlLXVzaW5nIGEKbm9uY2UgcmV0dXJucyBbYENvbnRyYWN0RXJyb3I6OkNoYWxsZW5nZUFscmVhZHlVc2VkYF0pLgotIGBleHBpcnlgICAg4oCUIExlZGdlciBzZXF1ZW5jZSBhZnRlciB3aGljaCB0aGUgY2hhbGxlbmdlIGlzIGNvbnNpZGVyZWQKZXhwaXJlZCBhbmQgY2Fubm90IGJlIHZlcmlmaWVkLgoKIyBSZXR1cm5zCmBPaygoKSlgIG9uIHN1Y2Nlc3MuCgojIEVycm9ycwotIFtgQ29udHJhY3RFcnJvcjo6VW5hdXRob3JpemVkYF0gICAgICAg4oCUIGBhZG1pbmAgZG9lcyBub3QgbWF0Y2ggYENPTlRSQUNUX0FETUlOYC4KLSBbYENvbnRyYWN0RXJyb3I6OkNoYWxsZW5nZUV4cGlyZWRgXSAgIOKAlCBgZXhwaXJ5IOKJpCBjdXJyZW50X2xlZGdlcmAuCi0gW2BDb250cmFjdEVycm9yOjpDaGFsbGVuZ2VBbHJlYWR5VXNlZGBdIOKAlCBgbm9uY2VgIGFscmVhZHkgZXhpc3RzIGluIHN0b3JhZ2UuCgojIEV2ZW50cwpFbWl0cyBgKCJzZXAxMCIsICJpc3N1ZWQiKSDihpIgKGFjY291bnQsIG5vbmNlKWAuAAAAFWlzc3VlX3NlcDEwX2NoYWxsZW5nZQAAAAAAAAQAAAAAAAAABWFkbWluAAAAAAAAEwAAAAAAAAAHYWNjb3VudAAAAAATAAAAAAAAAAVub25jZQAAAAAAA+4AAAAgAAAAAAAAAAZleHBpcnkAAAAAAAYAAAABAAAD6QAAA+0AAAAAAAAH0AAAAA1Db250cmFjdEVycm9yAAAA",
        "AAAAAAAAAGVQcm9wb3NlIGEgbmV3IGFkbWluIGtleSB3aXRoIGEgMjQtaG91ciB0aW1lbG9jay4KT25seSB0aGUgY3VycmVudCBhZG1pbiAoQ09OVFJBQ1RfQURNSU4pIGNhbiBwcm9wb3NlLgAAAAAAABVwcm9wb3NlX25ld19hZG1pbl9rZXkAAAAAAAACAAAAAAAAAAVhZG1pbgAAAAAAABMAAAAAAAAAB25ld19rZXkAAAAD7gAAACAAAAABAAAD6QAAA+0AAAAAAAAH0AAAAA1Db250cmFjdEVycm9yAAAA",
        "AAAAAAAAAENSZXR1cm4gdGhlIGN1cnJlbnQgd2hpdGVsaXN0IG9mIHBlcm1pdHRlZCB0b2tlbiBjb250cmFjdCBhZGRyZXNzZXMuAAAAABZnZXRfd2hpdGVsaXN0ZWRfdG9rZW5zAAAAAAAAAAAAAQAAA+oAAAAT",
        "AAAAAAAAANlWZXJpZnkgYSBTRVAtMTAgY2hhbGxlbmdlLgoKVGhlIGNhbGxlciBwcm92aWRlcyB0aGUgYG5vbmNlYCBhbmQgYW4gRUQyNTUxOSBgc2lnbmF0dXJlYCBvdmVyClNIQTI1NiggYWRkcmVzc19ieXRlcyB8fCBub25jZV9ieXRlcyB8fCBleHBpcnlfbGU4ICkuCgpPbiBzdWNjZXNzIHRoZSBjaGFsbGVuZ2UgaXMgY29uc3VtZWQgYW5kIGBhY2NvdW50YCBpcyBtYXJrZWQgdmVyaWZpZWQuAAAAAAAAFnZlcmlmeV9zZXAxMF9jaGFsbGVuZ2UAAAAAAAMAAAAAAAAAB2FjY291bnQAAAAAEwAAAAAAAAAFbm9uY2UAAAAAAAPuAAAAIAAAAAAAAAAJc2lnbmF0dXJlAAAAAAAD7gAAAEAAAAABAAAD6QAAA+0AAAAAAAAH0AAAAA1Db250cmFjdEVycm9yAAAA",
        "AAAAAAAAAlhDb25maWd1cmUgdGhlIGRpc3B1dGUgcmVzb2x1dGlvbiBzeXN0ZW0gKGFkbWluIG9ubHkpLgoKU2V0cyB0aGUgYXJiaXRyYXRvciBhZGRyZXNzIGFuZCB0aGUgdG9rZW4gZmVlIHJlcXVpcmVkIHRvIGZpbGUgYQpkaXNwdXRlLiBNdXN0IGJlIGNhbGxlZCBiZWZvcmUgW2BmaWxlX2Rpc3B1dGVgXSBpcyB1c2FibGUuCgojIFBhcmFtZXRlcnMKLSBgYWRtaW5gICAgICAgICDigJQgTXVzdCBtYXRjaCBgQ09OVFJBQ1RfQURNSU5gOyBtdXN0IGF1dGhvcmlzZSB0aGUgY2FsbC4KLSBgYXJiaXRyYXRvcmAgICDigJQgQWRkcmVzcyBhdXRob3Jpc2VkIHRvIGNhbGwgW2ByZXNvbHZlX2Rpc3B1dGVgXSBhbmQKW2ByZWplY3RfZGlzcHV0ZWBdLgotIGBkaXNwdXRlX2ZlZWAgIOKAlCBUb2tlbiBhbW91bnQgYSBkaXNwdXRpbmcgcGxheWVyIG11c3QgcGF5IHVwZnJvbnQ7CnJlZnVuZGVkIGlmIHRoZSBkaXNwdXRlIGlzIHJlamVjdGVkLiBNdXN0IGJlIOKJpSAwLgoKIyBQYW5pY3MKLSBJZiB0aGUgY29udHJhY3QgaXMgbm90IGluaXRpYWxpc2VkLgotIElmIGBhZG1pbmAgZG9lcyBub3QgbWF0Y2ggdGhlIHN0b3JlZCBhZG1pbi4KLSBJZiBgZGlzcHV0ZV9mZWUgPCAwYC4AAAAYY29uZmlndXJlX2Rpc3B1dGVfc3lzdGVtAAAAAwAAAAAAAAAFYWRtaW4AAAAAAAATAAAAAAAAAAphcmJpdHJhdG9yAAAAAAATAAAAAAAAAAtkaXNwdXRlX2ZlZQAAAAALAAAAAA==",
        "AAAAAAAAA0FDcmVhdGUgYSB0aW1lLWxvY2tlZCBlc2Nyb3cgZm9yIGEgY29tcGxldGVkIHRvdXJuYW1lbnQgZ2FtZS4KCkNhbGN1bGF0ZXMgdGhlIHRvdGFsIHByaXplIHBvb2wgZnJvbSB0aGUgZ2FtZSdzIHdhZ2VyIGFtb3VudCBhbmQgbG9ja3MgaXQKdW50aWwgYGN1cnJlbnRfbGVkZ2VyICsgdGltZWxvY2tfZHVyYXRpb25gLiBUaGUgZXNjcm93IG11c3QgdGhlbiBiZQpyZWxlYXNlZCB2aWEgW2ByZWxlYXNlX3RvdXJuYW1lbnRfZXNjcm93YF0gYnkgdGhlIGFkbWluIGFmdGVyIHRoZSBsb2NrCmV4cGlyZXMuIFJlcXVpcmVzIGBwbGF5ZXIxYCdzIGF1dGhvcmlzYXRpb24gYXMgdGhlIHRvdXJuYW1lbnQgb3JnYW5pc2VyLgoKIyBQYXJhbWV0ZXJzCi0gYGdhbWVfaWRgIOKAlCBJRCBvZiBhIGdhbWUgaW4gYENvbXBsZXRlZGAgc3RhdGUuCgojIFJldHVybnMKYE9rKGVzY3Jvd19pZClgIOKAlCB0aGUgdW5pcXVlIGB1NjRgIGlkZW50aWZpZXIgZm9yIHRoZSBuZXcgZXNjcm93LgoKIyBFcnJvcnMKLSBbYENvbnRyYWN0RXJyb3I6OkdhbWVOb3RGb3VuZGBdICAgICAgIOKAlCBgZ2FtZV9pZGAgZG9lcyBub3QgZXhpc3QuCi0gW2BDb250cmFjdEVycm9yOjpHYW1lTm90SW5Qcm9ncmVzc2BdICDigJQgR2FtZSBpcyBub3QgaW4gYENvbXBsZXRlZGAgc3RhdGUuCi0gW2BDb250cmFjdEVycm9yOjpUaW1lb3V0Tm90Q29uZmlndXJlZGBdIOKAlCBbYGNvbmZpZ3VyZV90b3VybmFtZW50X3RpbWVsb2NrYF0gaGFzIG5vdCBiZWVuIGNhbGxlZC4KCiMgRXZlbnRzCkVtaXRzIGAoInRsX2VzY3JvdyIsICJjcmVhdGVkIikg4oaSIChlc2Nyb3dfaWQsIGdhbWVfaWQsIGxvY2tlZF91bnRpbClgLgAAAAAAABhjcmVhdGVfdG91cm5hbWVudF9lc2Nyb3cAAAABAAAAAAAAAAdnYW1lX2lkAAAAAAYAAAABAAAD6QAAAAYAAAfQAAAADUNvbnRyYWN0RXJyb3IAAAA=",
        "AAAAAAAAAFFSZW1vdmUgYSB0b2tlbiBhZGRyZXNzIGZyb20gdGhlIHdoaXRlbGlzdC4KT25seSB0aGUgY29udHJhY3QgYWRtaW4gbWF5IGNhbGwgdGhpcy4AAAAAAAAYcmVtb3ZlX3doaXRlbGlzdGVkX3Rva2VuAAAAAgAAAAAAAAAFYWRtaW4AAAAAAAATAAAAAAAAAAV0b2tlbgAAAAAAABMAAAAA",
        "AAAAAAAAA8lJbml0aWFsaXplIHB1enpsZS1yZXdhcmQgc3lzdGVtICgjMTk5KSBhbmQgZmVlIGNvbmZpZ3VyYXRpb24gKCMyMDApLgoKTXVzdCBiZSBjYWxsZWQgZXhhY3RseSBvbmNlIGFmdGVyIFtgaW5pdGlhbGl6ZV90b2tlbmBdLiBTZXRzIHRoZSBFRDI1NTE5CmJhY2tlbmQgcHVibGljIGtleSwgc2VlZHMgdGhlIHRyZWFzdXJ5LCBhbmQgY29uZmlndXJlcyB0aGUgcHJvdG9jb2wgZmVlCmFuZCB0cmVhc3VyeSBhZGRyZXNzLiBBbHNvIHNldHMgdGhlIGRlZmF1bHQgYE1BWF9TVEFLRWAgKDEgMDAwKSBhbmQKYE1BWF9QUklaRV9QT09MYCAoMiAwMDApLgoKIyBQYXJhbWV0ZXJzCi0gYGFkbWluYCAgICAgICAgICAgIOKAlCBDb250cmFjdCBhZG1pbmlzdHJhdG9yOyBtdXN0IGF1dGhvcmlzZSB0aGUgY2FsbC4KLSBgYWRtaW5fcHVibGljX2tleWAg4oCUIDMyLWJ5dGUgRUQyNTUxOSBwdWJsaWMga2V5IG9mIHRoZSBiYWNrZW5kIHNpZ25pbmcKc2VydmljZSAodXNlZCB0byB2ZXJpZnkgcHV6emxlLXJld2FyZCBhbmQgd2luL2RyYXcgY2xhaW1zKS4KLSBgdHJlYXN1cnlfYW1vdW50YCAg4oCUIEluaXRpYWwgdG9rZW4gcmVzZXJ2ZSBmb3IgcHV6emxlIHJld2FyZCBwYXlvdXRzOyBtdXN0IGJlIOKJpSAwLgotIGBmZWVfYmlwc2AgICAgICAgICDigJQgUHJvdG9jb2wgZmVlIGluIGJhc2lzLXBvaW50cyBvZiAxIDAwMCAoZS5nLiBgMjBgID0gMiAlKS4KQ2FwcGVkIGF0IDEgMDAwICgxMDAgJSkuCi0gYHRyZWFzdXJ5X2FkZHJlc3NgIOKAlCBBZGRyZXNzIHRoYXQgcmVjZWl2ZXMgdGhlIGZlZSBwb3J0aW9uIG9mIGVhY2ggcGF5b3V0LgoKIyBQYW5pY3MKLSBJZiBgQ09OVFJBQ1RfQURNSU5gIGlzIGFscmVhZHkgc2V0IChgIkFscmVhZHkgaW5pdGlhbGl6ZWQiYCkuCi0gSWYgYGFkbWluX3B1YmxpY19rZXkubGVuKCkgIT0gMzJgLgotIElmIGB0cmVhc3VyeV9hbW91bnQgPCAwYC4KLSBJZiBgZmVlX2JpcHMgPiAxMDAwYC4AAAAAAAAZaW5pdGlhbGl6ZV9wdXp6bGVfcmV3YXJkcwAAAAAAAAUAAAAAAAAABWFkbWluAAAAAAAAEwAAAAAAAAAQYWRtaW5fcHVibGljX2tleQAAAA4AAAAAAAAAD3RyZWFzdXJ5X2Ftb3VudAAAAAALAAAAAAAAAAhmZWVfYmlwcwAAAAQAAAAAAAAAEHRyZWFzdXJ5X2FkZHJlc3MAAAATAAAAAA==",
        "AAAAAAAABABSZWxlYXNlIGEgdGltZS1sb2NrZWQgdG91cm5hbWVudCBlc2Nyb3cgdG8gdGhlIHNwZWNpZmllZCB3aW5uZXJzIChhZG1pbiBvbmx5KS4KCkRpc3RyaWJ1dGVzIHRoZSBsb2NrZWQgcHJpemUgcG9vbCBwcm9wb3J0aW9uYWxseSBhY2NvcmRpbmcgdG8KYHBlcmNlbnRhZ2VzYC4gSW50ZWdlci1kaXZpc2lvbiBkdXN0IGdvZXMgdG8gdGhlIGZpcnN0IHdpbm5lci4KQ2FuIG9ubHkgYmUgY2FsbGVkIGFmdGVyIHRoZSBsb2NrIHBlcmlvZCBoYXMgZXhwaXJlZC4KCiMgUGFyYW1ldGVycwotIGBhZG1pbmAgICAgICAgIOKAlCBNdXN0IG1hdGNoIGBDT05UUkFDVF9BRE1JTmA7IG11c3QgYXV0aG9yaXNlIHRoZSBjYWxsLgotIGBlc2Nyb3dfaWRgICAgIOKAlCBJRCBvZiBhIHByZXZpb3VzbHkgY3JlYXRlZCwgdW5yZWxlYXNlZCBlc2Nyb3cuCi0gYHdpbm5lcnNgICAgICAg4oCUIE9yZGVyZWQgcmVjaXBpZW50IGFkZHJlc3Nlcy4KLSBgcGVyY2VudGFnZXNgICDigJQgV2hvbGUtbnVtYmVyIHBlcmNlbnRhZ2VzIHBhcmFsbGVsIHRvIGB3aW5uZXJzYDsKbXVzdCBzdW0gdG8gZXhhY3RseSAxMDAuCgojIFJldHVybnMKYE9rKCgpKWAgb24gc3VjY2Vzcy4KCiMgRXJyb3JzCi0gW2BDb250cmFjdEVycm9yOjpVbmF1dGhvcml6ZWRgXSAgICAgICAgIOKAlCBgYWRtaW5gIGRvZXMgbm90IG1hdGNoIGBDT05UUkFDVF9BRE1JTmAuCi0gW2BDb250cmFjdEVycm9yOjpFc2Nyb3dOb3RGb3VuZGBdICAgICAgICDigJQgYGVzY3Jvd19pZGAgZG9lcyBub3QgZXhpc3QuCi0gW2BDb250cmFjdEVycm9yOjpFc2Nyb3dBbHJlYWR5UmVsZWFzZWRgXSDigJQgRXNjcm93IGhhcyBhbHJlYWR5IGJlZW4gcmVsZWFzZWQuCi0gW2BDb250cmFjdEVycm9yOjpFc2Nyb3dTdGlsbExvY2tlZGBdICAgICDigJQgTG9jayBwZXJpb2QgaGFzIG5vdCB5ZXQgZWxhcHNlZC4KLSBbYENvbnRyYWN0RXJyb3I6Ok1pc21hdGNoZWRMZW5ndGhzYF0gICAgIOKAlCBgd2lubmVyc2AgYW5kIGBwZXJjZW50YWdlc2AgZGlmZmVyLgotIFtgQ29udHJhY3RFcnJvAAAAGXJlbGVhc2VfdG91cm5hbWVudF9lc2Nyb3cAAAAAAAAEAAAAAAAAAAVhZG1pbgAAAAAAABMAAAAAAAAACWVzY3Jvd19pZAAAAAAAAAYAAAAAAAAAB3dpbm5lcnMAAAAD6gAAABMAAAAAAAAAC3BlcmNlbnRhZ2VzAAAAA+oAAAAEAAAAAQAAA+kAAAPtAAAAAAAAB9AAAAANQ29udHJhY3RFcnJvcgAAAA==",
        "AAAAAAAABABDbGFpbSBtdWx0aXBsZSBwdXp6bGUgcmV3YXJkcyBpbiBhIHNpbmdsZSB0cmFuc2FjdGlvbi4KCk1vcmUgZ2FzLWVmZmljaWVudCB0aGFuIGNhbGxpbmcgW2BjbGFpbV9wdXp6bGVfcmV3YXJkYF0gcmVwZWF0ZWRseSBiZWNhdXNlCnRoZSBub25jZSwgYmFsYW5jZSwgYW5kIHRyZWFzdXJ5IHN0b3JhZ2UgbWFwcyBhcmUgcmVhZCBhbmQgd3JpdHRlbiBvbmx5Cm9uY2UgZm9yIHRoZSB3aG9sZSBiYXRjaC4gRWFjaCBbYFByb29mYF0gaXMgdmFsaWRhdGVkIHdpdGggdGhlIHNhbWUKc2lnbmF0dXJlIHNjaGVtZSBhbmQgcmVwbGF5LXByb3RlY3Rpb24gcnVsZXMgYXMgdGhlIHNpbmdsZS1jbGFpbSB2YXJpYW50LgoKKipBdG9taWNpdHk6KiogYWxsIHByb29mcyBhcmUgYXBwbGllZCB0byBpbi1tZW1vcnkgc3RhdGUgZmlyc3Q7IHN0b3JhZ2UKaXMgb25seSBjb21taXR0ZWQgYWZ0ZXIgZXZlcnkgcHJvb2YgcGFzc2VzLiBBIHNpbmdsZSBpbnZhbGlkIHByb29mIHJvbGxzCmJhY2sgdGhlIGVudGlyZSBiYXRjaC4KCiMgUGFyYW1ldGVycwotIGBwcm9vZnNgIOKAlCAx4oCTW2BNQVhfQkFUQ0hfU0laRWBdIChjdXJyZW50bHkgMjApIFtgUHJvb2ZgXSBlbnRyaWVzLgpFYWNoIHByb29mIGNhcnJpZXMgaXRzIG93biBgcmVjaXBpZW50YCwgYHJld2FyZF9hbW91bnRgLApgbm9uY2VgLCBhbmQgYHNpZ25hdHVyZWA7IHNlZSBbYGNsYWltX3B1enpsZV9yZXdhcmRgXSBmb3IKdGhlIHBlci1wcm9vZiB2YWxpZGF0aW9uIHJ1bGVzLgoKIyBSZXR1cm5zCmBPaygoKSlgIHdoZW4gYWxsIHByb29mcyBhcmUgYWNjZXB0ZWQgYW5kIHN0YXRlIGlzIGNvbW1pdHRlZC4KCiMgRXJyb3JzCi0gW2BDb250cmFjdEVycm9yOjpFbXB0eUJhdGNoYF0gICAg4oCUIGBwcm9vZnNgIGlzIGVtcHR5LgotIFtgQ29udHJhY3RFcnJvcjo6QmF0Y2hUb29MYXJnZWBdIOKAlCBgcHJvb2ZzLmxlbigpID4gTUFYX0JBVENIX1NJWkVgLgotIFtgQ29udHJhY3RFcnJvcjo6SW52YWxpZEFtb3VudGBdIOKAlCBBbnkgcHJvb2YgaGFzIGByZXdhcmRfYW1vAAAAGmNsYWltX3B1enpsZV9yZXdhcmRzX2JhdGNoAAAAAAABAAAAAAAAAAZwcm9vZnMAAAAAA+oAAAfQAAAABVByb29mAAAAAAAAAQAAA+kAAAPtAAAAAAAAB9AAAAANQ29udHJhY3RFcnJvcgAAAA==",
        "AAAAAAAAAzRDb25maWd1cmUgdGhlIHRpbWVzdGFtcC1iYXNlZCBtYXRjaCBkZWFkbGluZSBhbmQgZ3JhY2UgYnVmZmVyIHVzZWQgYnkKW2BjbGFpbV90aW1lb3V0X3ZpY3RvcnlgXSAoYWRtaW4gb25seSkuCgojIFBhcmFtZXRlcnMKLSBgYWRtaW5gICAgICAgICAgIOKAlCBNdXN0IG1hdGNoIGBDT05UUkFDVF9BRE1JTmA7IG11c3QgYXV0aG9yaXNlIHRoZSBjYWxsLgotIGBkZWFkbGluZV9zZWNzYCAg4oCUIFNlY29uZHMgYSBwbGF5ZXIgaGFzIHRvIHJlc3BvbmQgdG8gdGhlaXIgb3Bwb25lbnQncwpsYXN0IG1vdmUgYmVmb3JlIHRoZSBvcHBvbmVudCBtYXkgZm9yY2UgYSB0aW1lb3V0CnZpY3RvcnkuIE11c3QgYmUgPiAwLgotIGBidWZmZXJfc2Vjc2AgICAg4oCUIEFkZGl0aW9uYWwgZ3JhY2Ugc2Vjb25kcyBsYXllcmVkIG9uIHRvcCBvZgpgZGVhZGxpbmVfc2Vjc2Ag4oCUIGUuZy4gdG8gY292ZXIgYSBicmllZiBiYWNrZW5kCm91dGFnZSB0aGF0IHdvdWxkIG90aGVyd2lzZSBoYXZlIGFyYml0cmF0ZWQgdGhlCmdhbWUgdmlhIGBjbGFpbV93aW5gL2BjbGFpbV9kcmF3YC4gTWF5IGJlIGAwYCB0bwpkaXNhYmxlIHRoZSBleHRyYSBncmFjZSB3aW5kb3cuCgojIEVycm9ycwotIFtgQ29udHJhY3RFcnJvcjo6Tm90SW5pdGlhbGl6ZWRgXSDigJQgQ29udHJhY3QgaGFzIG5vdCBiZWVuIGluaXRpYWxpemVkLgotIFtgQ29udHJhY3RFcnJvcjo6VW5hdXRob3JpemVkYF0gICDigJQgYGFkbWluYCBkb2VzIG5vdCBtYXRjaCB0aGUgc3RvcmVkIGFkbWluLgotIFtgQ29udHJhY3RFcnJvcjo6SW52YWxpZENvbmZpZ2BdICDigJQgYGRlYWRsaW5lX3NlY3MgPT0gMGAuAAAAGmNvbmZpZ3VyZV90aW1lb3V0X2RlYWRsaW5lAAAAAAADAAAAAAAAAAVhZG1pbgAAAAAAABMAAAAAAAAADWRlYWRsaW5lX3NlY3MAAAAAAAAGAAAAAAAAAAtidWZmZXJfc2VjcwAAAAAGAAAAAQAAA+kAAAPtAAAAAAAAB9AAAAANQ29udHJhY3RFcnJvcgAAAA==",
        "AAAAAAAABABHYXMtb3B0aW1pemVkIHRvdXJuYW1lbnQgcGF5b3V0IOKAlCBzaW5nbGUgcGFzcywgbm8gcmVkdW5kYW50IG1hcCByZWFkcy4KClZhbGlkYXRlcyB0aGF0IGBwZXJjZW50YWdlc2Agc3VtIHRvIGV4YWN0bHkgMTAwLCB0aGVuIGRpc3RyaWJ1dGVzIHRoZQp0b3RhbCBwcml6ZSBwb29sIGZyb20gZXNjcm93IHRvIGVhY2ggd2lubmVyIGluIGEgc2luZ2xlIHRva2VuLXRyYW5zZmVyCmxvb3AuIEFueSBpbnRlZ2VyLWRpdmlzaW9uIHJlbWFpbmRlciAoZHVzdCkgaXMgYWRkZWQgdG8gdGhlIGZpcnN0Cndpbm5lcidzIHNoYXJlLgoKUmVxdWlyZXMgYXV0aG9yaXNhdGlvbiBmcm9tIGBnYW1lLnBsYXllcjFgICh0aGUgdG91cm5hbWVudCBvcmdhbmlzZXIpLgoKIyBQYXJhbWV0ZXJzCi0gYGdhbWVfaWRgICAgICAg4oCUIElEIG9mIGEgZ2FtZSBpbiB0aGUgYENvbXBsZXRlZGAgc3RhdGUuCi0gYHdpbm5lcnNgICAgICAg4oCUIE9yZGVyZWQgbGlzdCBvZiByZWNpcGllbnQgYWRkcmVzc2VzLgotIGBwZXJjZW50YWdlc2AgIOKAlCBXaG9sZS1udW1iZXIgcGVyY2VudGFnZXMgKDDigJMxMDApIHBhcmFsbGVsIHRvIGB3aW5uZXJzYDsKbXVzdCBzdW0gdG8gZXhhY3RseSAxMDAuCgojIFJldHVybnMKYE9rKCgpKWAgb24gc3VjY2Vzcy4KCiMgRXJyb3JzCi0gW2BDb250cmFjdEVycm9yOjpHYW1lTm90Rm91bmRgXSAgICAgICDigJQgYGdhbWVfaWRgIGRvZXMgbm90IGV4aXN0LgotIFtgQ29udHJhY3RFcnJvcjo6R2FtZU5vdEluUHJvZ3Jlc3NgXSAg4oCUIEdhbWUgaXMgbm90IGluIGBDb21wbGV0ZWRgIHN0YXRlLgotIFtgQ29udHJhY3RFcnJvcjo6TWlzbWF0Y2hlZExlbmd0aHNgXSAg4oCUIGB3aW5uZXJzYCBhbmQgYHBlcmNlbnRhZ2VzYCBkaWZmZXIgaW4gbGVuZ3RoLgotIFtgQ29udHJhY3RFcnJvcjo6SW52YWxpZFBlcmNlbnRhZ2VgXSAg4oCUIFBlcmNlbnRhZ2VzIG92ZXJmbG93IG9yIGRvIG5vdCBzdW0gdG8gMTAwLgoKIyBFdmVudHMKRG9lcyBub3QgZW1pdCBldmVudHMgZGlyZWN0bHk7IHRva2VuIHRyYW5zAAAAG3BheW91dF90b3VybmFtZW50X29wdGltaXplZAAAAAADAAAAAAAAAAdnYW1lX2lkAAAAAAYAAAAAAAAAB3dpbm5lcnMAAAAD6gAAABMAAAAAAAAAC3BlcmNlbnRhZ2VzAAAAA+oAAAAEAAAAAQAAA+kAAAPtAAAAAAAAB9AAAAANQ29udHJhY3RFcnJvcgAAAA==",
        "AAAAAAAAAkNDb25maWd1cmUgdGhlIHRpbWUtbG9jayBkdXJhdGlvbiBmb3IgdG91cm5hbWVudCBwcml6ZSBlc2Nyb3dzIChhZG1pbiBvbmx5KS4KCkFmdGVyIGEgdG91cm5hbWVudCBnYW1lIGNvbXBsZXRlcywgYSBwcml6ZSBlc2Nyb3cgY3JlYXRlZCB2aWEKW2BjcmVhdGVfdG91cm5hbWVudF9lc2Nyb3dgXSBpcyBsb2NrZWQgZm9yIGBkdXJhdGlvbmAgbGVkZ2VyIHNlcXVlbmNlcwpiZWZvcmUgaXQgY2FuIGJlIHJlbGVhc2VkLgoKIyBQYXJhbWV0ZXJzCi0gYGFkbWluYCAgICAg4oCUIE11c3QgbWF0Y2ggYENPTlRSQUNUX0FETUlOYDsgbXVzdCBhdXRob3Jpc2UgdGhlIGNhbGwuCi0gYGR1cmF0aW9uYCAg4oCUIExvY2sgZHVyYXRpb24gaW4gbGVkZ2VyIHNlcXVlbmNlczsgbXVzdCBiZSA+IDAuCgojIFJldHVybnMKYE9rKCgpKWAgb24gc3VjY2Vzcy4KCiMgRXJyb3JzCi0gW2BDb250cmFjdEVycm9yOjpVbmF1dGhvcml6ZWRgXSDigJQgYGFkbWluYCBkb2VzIG5vdCBtYXRjaCBgQ09OVFJBQ1RfQURNSU5gLgoKIyBQYW5pY3MKLSBJZiB0aGUgY29udHJhY3QgaXMgbm90IGluaXRpYWxpc2VkLgotIElmIGBkdXJhdGlvbiA9PSAwYC4AAAAAHWNvbmZpZ3VyZV90b3VybmFtZW50X3RpbWVsb2NrAAAAAAAAAgAAAAAAAAAFYWRtaW4AAAAAAAATAAAAAAAAAAhkdXJhdGlvbgAAAAYAAAABAAAD6QAAA+0AAAAAAAAH0AAAAA1Db250cmFjdEVycm9yAAAA",
        "AAAAAAAAAKtHZXQgYWxsIHBsYXllciBwcm9maWxlcyAocGFnaW5hdGVkIGZvciBlZmZpY2llbmN5KS4KClJldHVybnMgdXAgdG8gYGxpbWl0YCBwcm9maWxlcyBzdGFydGluZyBmcm9tIHRoZSBnaXZlbiBvZmZzZXQuClRoaXMgaXMgdXNlZnVsIGZvciBsZWFkZXJib2FyZHMgYW5kIHN0YXRpc3RpY3MgcXVlcmllcy4AAAAAHWdldF9wbGF5ZXJfcHJvZmlsZXNfcGFnaW5hdGVkAAAAAAAAAgAAAAAAAAAGb2Zmc2V0AAAAAAAEAAAAAAAAAAVsaW1pdAAAAAAAAAQAAAABAAAD6gAAB9AAAAAMUGxheWVyUmF0aW5n",
        "AAAAAAAAAYlRdWVyeSB0aGUgc2Vjb25kcyByZW1haW5pbmcgYmVmb3JlIFtgY2xhaW1fdGltZW91dF92aWN0b3J5YF0gYmVjb21lcwpjbGFpbWFibGUgYnkgdGhlIHBsYXllciBjdXJyZW50bHkgd2FpdGluZyBvbiB0aGVpciBvcHBvbmVudCdzIG1vdmUuCgpSZXR1cm5zIGBOb25lYCBpZiB0aGUgZ2FtZSBkb2Vzbid0IGV4aXN0LCBpc24ndCBgSW5Qcm9ncmVzc2AsIG9yIG5vCmRlYWRsaW5lIGhhcyBiZWVuIGNvbmZpZ3VyZWQuIFJldHVybnMgYFNvbWUoMClgIG9uY2UgdGhlIGRlYWRsaW5lICsKYnVmZmVyIGhhcyBhbHJlYWR5IGVsYXBzZWQgKGEgdGltZW91dCB2aWN0b3J5IGlzIGNsYWltYWJsZSBub3cpLgoKIyBQYXJhbWV0ZXJzCi0gYGdhbWVfaWRgIOKAlCBJRCBvZiB0aGUgZ2FtZSB0byBxdWVyeS4AAAAAAAAdZ2V0X3RpbWVvdXRfdmljdG9yeV9yZW1haW5pbmcAAAAAAAABAAAAAAAAAAdnYW1lX2lkAAAAAAYAAAABAAAD6AAAAAY=",
        "AAAABAAAATNDZW50cmFsaXplZCBlcnJvciBjb2RlcyBmb3IgdGhlIEtuaWdodFZlcnNlIGdhbWUgY29udHJhY3QuCgpBbGwgY29udHJhY3QgZnVuY3Rpb25zIHJldHVybiBzdHJ1Y3R1cmVkIGVycm9ycyBpbnN0ZWFkIG9mIHBhbmlja2luZyB3aXRoCnN0cmluZyBtZXNzYWdlcy4gVGhpcyBtYWtlcyBmcm9udGVuZCBlcnJvciBoYW5kbGluZyBhbmQgZGVidWdnaW5nCmRldGVybWluaXN0aWMg4oCUIGVhY2ggdmFyaWFudCBtYXBzIHRvIGEgZGlzdGluY3QgYHUzMmAgdmFsdWUgdGhhdCB0aGUgU0RLCmV4cG9zZXMgdmlhIGBTQ0Vycm9yOjpDb250cmFjdGAuAAAAAAAAAAANQ29udHJhY3RFcnJvcgAAAAAAADIAAAAAAAAADEdhbWVOb3RGb3VuZAAAAAEAAAAAAAAAC05vdFlvdXJUdXJuAAAAAAIAAAAAAAAAEUdhbWVOb3RJblByb2dyZXNzAAAAAAAAAwAAAAAAAAALSW52YWxpZE1vdmUAAAAABAAAAAAAAAARSW5zdWZmaWNpZW50RnVuZHMAAAAAAAAFAAAAAAAAAA1BbHJlYWR5Sm9pbmVkAAAAAAAABgAAAAAAAAAIR2FtZUZ1bGwAAAAHAAAAAAAAAAlOb3RQbGF5ZXIAAAAAAAAIAAAAAAAAABRHYW1lQWxyZWFkeUNvbXBsZXRlZAAAAAkAAAAAAAAAEUludmFsaWRQZXJjZW50YWdlAAAAAAAADAAAAAAAAAARTWlzbWF0Y2hlZExlbmd0aHMAAAAAAAANAAAAMUludmFsaWQgb3IgYWxyZWFkeS11c2VkIGJhY2tlbmQgc2lnbmF0dXJlICAoIzE5OSkAAAAAAAAMVW5hdXRob3JpemVkAAAADgAAAAAAAAASU3Rha2VMaW1pdEV4Y2VlZGVkAAAAAAAPAAAAGkdhbWUgaGFzIG5vdCB0aW1lZCBvdXQgeWV0AAAAAAARVGltZW91dE5vdFJlYWNoZWQAAAAAAAAQAAAAHlRpbWVvdXQgZmVhdHVyZSBub3QgY29uZmlndXJlZAAAAAAAFFRpbWVvdXROb3RDb25maWd1cmVkAAAAEQAAACFHYW1lIGlzIG5vdCBpbiBhIGRpc3B1dGFibGUgc3RhdGUAAAAAAAANTm90RGlzcHV0YWJsZQAAAAAAABIAAAARRGlzcHV0ZSBub3QgZm91bmQAAAAAAAAPRGlzcHV0ZU5vdEZvdW5kAAAAABMAAAAkT25seSBhcmJpdHJhdG9yIGNhbiByZXNvbHZlIGRpc3B1dGVzAAAADU5vdEFyYml0cmF0b3IAAAAAAAAUAAAAGEluc3VmZmljaWVudCBkaXNwdXRlIGZlZQAAABZJbnN1ZmZpY2llbnREaXNwdXRlRmVlAAAAAAAVAAAAL09ubHkgdGhlIHdhaXRpbmcgcGxheWVyIGNhbiBjbGFpbSBhIHRpbWVvdXQgd2luAAAAABZJbnZhbGlkVGltZW91dENsYWltYW50AAAAAAAWAAAAL1NldHRsZW1lbnQgb3IgcGF5b3V0IGhhcyBhbHJlYWR5IGJlZW4gcHJvY2Vzc2VkAAAAAA5BbHJlYWR5U2V0dGxlZAAAAAAAFwAAADlBbW91bnQgdmFsdWUgbXVzdCBiZSBwb3NpdGl2ZSBhbmQgd2l0aGluIHN1cHBvcnRlZCBib3VuZHMAAAAAAAANSW52YWxpZEFtb3VudAAAAAAAABgAAAAxU0VQLTEwIGNoYWxsZW5nZSBoYXMgZXhwaXJlZCBvciBpcyBpbnZhbGlkICgjNTI5KQAAAAAAABBDaGFsbGVuZ2VFeHBpcmVkAAAAGQAAACpTRVAtMTAgY2hhbGxlbmdlIG5vbmNlIGFscmVhZHkgdXNlZCAoIzUyOSkAAAAAABRDaGFsbGVuZ2VBbHJlYWR5VXNlZAAAABoAAAAzTXVsdGktc2lnOiBzaWduZXIgaXMgbm90IGluIHRoZSBzaWduZXJzIGxpc3QgKCM1MzUpAAAAAApOb3RBU2lnbmVyAAAAAAAcAAAANE11bHRpLXNpZzogbm8gcGVuZGluZyBmZWUgcHJvcG9zYWwgdG8gYXBwcm92ZSAoIzUzNSkAAAAKTm9Qcm9wb3NhbAAAAAAAHQAAADdNdWx0aS1zaWc6IHNpZ25lciBhbHJlYWR5IGFwcHJvdmVkIHRoaXMgcHJvcG9zYWwgKCM1MzUpAAAAAA9BbHJlYWR5QXBwcm92ZWQAAAAAHgAAAENNdWx0aS1zaWc6IHRocmVzaG9sZCBtdXN0IGJlIOKJpSAxIGFuZCDiiaQgbnVtYmVyIG9mIHNpZ25lcnMgKCM1MzUpAAAAABBJbnZhbGlkVGhyZXNob2xkAAAAHwAAACVPcmFjbGUgY29udHJhY3Qgbm90IGNvbmZpZ3VyZWQgKCM1MzMpAAAAAAAAE09yYWNsZU5vdENvbmZpZ3VyZWQAAAAAIAAAACJUb3VybmFtZW50IGVzY3JvdyBub3QgZm91bmQgKCM1MzIpAAAAAAAORXNjcm93Tm90Rm91bmQAAAAAACEAAAAoVG91cm5hbWVudCBlc2Nyb3cgaXMgc3RpbGwgbG9ja2VkICgjNTMyKQAAABFFc2Nyb3dTdGlsbExvY2tlZAAAAAAAACIAAAApVG91cm5hbWVudCBlc2Nyb3cgYWxyZWFkeSByZWxlYXNlZCAoIzUzMikAAAAAAAAVRXNjcm93QWxyZWFkeVJlbGVhc2VkAAAAAAAAIwAAADJUb3RhbCBwcml6ZSBwb29sIHdvdWxkIGV4Y2VlZCB0aGUgY29uZmlndXJlZCBsaW1pdAAAAAAAFlByaXplUG9vbExpbWl0RXhjZWVkZWQAAAAAACQAAAA6Y2xhaW1fcHV6emxlX3Jld2FyZHNfYmF0Y2ggY2FsbGVkIHdpdGggYW4gZW1wdHkgcHJvb2YgbGlzdAAAAAAACkVtcHR5QmF0Y2gAAAAAACUAAABGY2xhaW1fcHV6emxlX3Jld2FyZHNfYmF0Y2ggY2FsbGVkIHdpdGggbW9yZSBwcm9vZnMgdGhhbiBNQVhfQkFUQ0hfU0laRQAAAAAADUJhdGNoVG9vTGFyZ2UAAAAAAAAmAAAALUNvbnRyYWN0IGlzIHBhdXNlZCBmb3IgZW1lcmdlbmN5IGhhbHQgKFNDLTExKQAAAAAAAA5Db250cmFjdFBhdXNlZAAAAAAAJwAAADdDb250cmFjdCAob3IgYSBzdWItbW9kdWxlKSBoYXMgYWxyZWFkeSBiZWVuIGluaXRpYWxpemVkAAAAABJBbHJlYWR5SW5pdGlhbGl6ZWQAAAAAACgAAAAlQ29udHJhY3QgaGFzIG5vdCBiZWVuIGluaXRpYWxpemVkIHlldAAAAAAAAA5Ob3RJbml0aWFsaXplZAAAAAAAKQAAACtDb250cmFjdCBhZG1pbiBhZGRyZXNzIGhhcyBhbHJlYWR5IGJlZW4gc2V0AAAAAA9BZG1pbkFscmVhZHlTZXQAAAAAKgAAACBDYWxsZXIgaXMgbm90IHRoZSBjb250cmFjdCBhZG1pbgAAAAhOb3RBZG1pbgAAACsAAAAaQ29udHJhY3QgaXMgYWxyZWFkeSBwYXVzZWQAAAAAAA1BbHJlYWR5UGF1c2VkAAAAAAAALAAAACBDb250cmFjdCBpcyBub3QgY3VycmVudGx5IHBhdXNlZAAAAAlOb3RQYXVzZWQAAAAAAAAtAAAAME9uZSBvciBtb3JlIGNvbmZpZ3VyYXRpb24gcGFyYW1ldGVycyBhcmUgaW52YWxpZAAAAA1JbnZhbGlkQ29uZmlnAAAAAAAALgAAADNUcmVhc3VyeSBoYXMgaW5zdWZmaWNpZW50IGZ1bmRzIHRvIGNvdmVyIHRoZSBwYXlvdXQAAAAAFEluc3VmZmljaWVudFRyZWFzdXJ5AAAALwAAAD5Ub2tlbiBhZGRyZXNzIGlzIG5vdCBpbiB0aGUgYWRtaW4tbWFpbnRhaW5lZCB3aGl0ZWxpc3QgKFNDLTE3KQAAAAAAE1Rva2VuTm90V2hpdGVsaXN0ZWQAAAAAMAAAAEpQbGF5ZXIgaGFzIHJlYWNoZWQgdGhlIG1heGltdW0gbnVtYmVyIG9mIGFjdGl2ZSB0b3VybmFtZW50IGVzY3Jvd3MgKFNDLTIwKQAAAAAAGE1heEFjdGl2ZUVzY3Jvd3NFeGNlZWRlZAAAADEAAAAeUmVlbnRyYW50IGNhbGwgZGV0ZWN0ZWQgKCM4NjApAAAAAAANUmVlbnRyYW50Q2FsbAAAAAAAADIAAAAeQ2lyY3VpdCBicmVha2VyIHZvdGUgbm90IGZvdW5kAAAAAAAaQ2lyY3VpdEJyZWFrZXJWb3RlTm90Rm91bmQAAAAAADMAAAA4QWRtaW4ga2V5IHJvdGF0aW9uIGFscmVhZHkgaGFzIGEgcGVuZGluZyBwcm9wb3NhbCAoIzg5MCkAAAAWQWRtaW5LZXlBbHJlYWR5UGVuZGluZwAAAAAANAAAADVTdG9yZWQgYWRtaW4gcHVibGljIGtleSBpcyBub3QgYSAzMi1ieXRlIEVEMjU1MTkga2V5LgAAAAAAAA9JbnZhbGlkQWRtaW5LZXkAAAAANQ==" ]),
      options
    )
  }
  public readonly fromJSON = {
    pause: this.txFromJSON<null>,
        payout: this.txFromJSON<Result<void>>,
        forfeit: this.txFromJSON<Result<void>>,
        unpause: this.txFromJSON<null>,
        upgrade: this.txFromJSON<null>,
        get_game: this.txFromJSON<Result<Game>>,
        claim_win: this.txFromJSON<Result<void>>,
        is_paused: this.txFromJSON<boolean>,
        join_game: this.txFromJSON<Result<void>>,
        claim_draw: this.txFromJSON<Result<void>>,
        get_oracle: this.txFromJSON<Result<string>>,
        cancel_game: this.txFromJSON<Result<void>>,
        create_game: this.txFromJSON<Result<u64>>,
        get_dispute: this.txFromJSON<Result<Dispute>>,
        submit_move: this.txFromJSON<Result<void>>,
        file_dispute: this.txFromJSON<Result<u64>>,
        get_all_games: this.txFromJSON<Map<u64, Game>>,
        set_max_stake: this.txFromJSON<null>,
        upgrade_admin: this.txFromJSON<null>,
        configure_fees: this.txFromJSON<null>,
        reject_dispute: this.txFromJSON<Result<void>>,
        reward_balance: this.txFromJSON<i128>,
        get_oracle_time: this.txFromJSON<u64>,
        resolve_dispute: this.txFromJSON<Result<void>>,
        configure_oracle: this.txFromJSON<Result<void>>,
        get_fee_proposal: this.txFromJSON<Option<FeeProposal>>,
        initialize_token: this.txFromJSON<null>,
        treasury_balance: this.txFromJSON<i128>,
        claim_timeout_win: this.txFromJSON<Result<void>>,
        configure_timeout: this.txFromJSON<null>,
        is_sep10_verified: this.txFromJSON<boolean>,
        payout_tournament: this.txFromJSON<Result<void>>,
        set_player_rating: this.txFromJSON<Result<void>>,
        configure_multisig: this.txFromJSON<Result<void>>,
        get_approval_count: this.txFromJSON<u32>,
        get_player_profile: this.txFromJSON<Option<PlayerRating>>,
        propose_fee_change: this.txFromJSON<Result<void>>,
        set_max_prize_pool: this.txFromJSON<null>,
        cancel_fee_proposal: this.txFromJSON<Result<void>>,
        claim_puzzle_reward: this.txFromJSON<Result<void>>,
        accept_new_admin_key: this.txFromJSON<Result<void>>,
        approve_fee_proposal: this.txFromJSON<Result<boolean>>,
        add_whitelisted_token: this.txFromJSON<null>,
        claim_timeout_victory: this.txFromJSON<Result<void>>,
        get_timeout_remaining: this.txFromJSON<Option<u64>>,
        get_tournament_escrow: this.txFromJSON<Result<TournamentEscrow>>,
        issue_sep10_challenge: this.txFromJSON<Result<void>>,
        propose_new_admin_key: this.txFromJSON<Result<void>>,
        get_whitelisted_tokens: this.txFromJSON<Array<string>>,
        verify_sep10_challenge: this.txFromJSON<Result<void>>,
        configure_dispute_system: this.txFromJSON<null>,
        create_tournament_escrow: this.txFromJSON<Result<u64>>,
        remove_whitelisted_token: this.txFromJSON<null>,
        initialize_puzzle_rewards: this.txFromJSON<null>,
        release_tournament_escrow: this.txFromJSON<Result<void>>,
        claim_puzzle_rewards_batch: this.txFromJSON<Result<void>>,
        configure_timeout_deadline: this.txFromJSON<Result<void>>,
        payout_tournament_optimized: this.txFromJSON<Result<void>>,
        configure_tournament_timelock: this.txFromJSON<Result<void>>,
        get_player_profiles_paginated: this.txFromJSON<Array<PlayerRating>>,
        get_timeout_victory_remaining: this.txFromJSON<Option<u64>>
  }
}