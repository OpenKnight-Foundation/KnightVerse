use soroban_sdk::contracterror;

/// Centralized error codes for the KnightVerse game contract.
///
/// All contract functions return structured errors instead of panicking with
/// string messages. This makes frontend error handling and debugging
/// deterministic — each variant maps to a distinct `u32` value that the SDK
/// exposes via `SCError::Contract`.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum ContractError {
    // ── Game-state errors (1–23) ──────────────────────────────────────────
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
    /// Total prize pool would exceed the configured limit
    PrizePoolLimitExceeded = 36,
    /// claim_puzzle_rewards_batch called with an empty proof list
    EmptyBatch = 37,
    /// claim_puzzle_rewards_batch called with more proofs than MAX_BATCH_SIZE
    BatchTooLarge = 38,
    /// Contract is paused for emergency halt (SC-11)
    ContractPaused = 39,

    // ── Initialization & admin errors (40–49) ────────────────────────────
    /// Contract (or a sub-module) has already been initialized
    AlreadyInitialized = 40,
    /// Contract has not been initialized yet
    NotInitialized = 41,
    /// Contract admin address has already been set
    AdminAlreadySet = 42,
    /// Caller is not the contract admin
    NotAdmin = 43,
    /// Contract is already paused
    AlreadyPaused = 44,
    /// Contract is not currently paused
    NotPaused = 45,
    /// One or more configuration parameters are invalid
    InvalidConfig = 46,
    /// Treasury has insufficient funds to cover the payout
    InsufficientTreasury = 47,
    /// Token address is not in the admin-maintained whitelist (SC-17)
    TokenNotWhitelisted = 48,
    /// Player has reached the maximum number of active tournament escrows (SC-20)
    MaxActiveEscrowsExceeded = 49,
    /// Reentrant call detected (#860)
    ReentrantCall = 50,
    /// Circuit breaker vote not found
    CircuitBreakerVoteNotFound = 51,
}
