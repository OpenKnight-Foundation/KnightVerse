use soroban_sdk::contracterror;

/// Centralized error codes for the Gasless Meta-Transaction Relayer Contract (#1148 SC-48).
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum RelayerError {
    // ── Authorization & Governance Errors (1–9) ──
    /// Contract is already initialized
    AlreadyInitialized = 1,
    /// Contract has not been initialized yet
    NotInitialized = 2,
    /// Caller is not authorized for this operation
    Unauthorized = 3,
    /// Caller is not the contract admin
    NotAdmin = 4,
    /// Contract is paused for emergency stop
    ContractPaused = 5,
    /// Contract is not paused
    NotPaused = 6,
    /// The submitting relayer address is not authorized
    RelayerNotAuthorized = 7,

    // ── Cryptographic & Signature Verification Errors (10–19) ──
    /// Cryptographic signature verification failed
    InvalidSignature = 10,
    /// Public key or signer address format is invalid
    InvalidSigner = 11,
    /// Recovered signer does not match expected sender/player
    SignerMismatch = 12,
    /// Domain separator mismatch (wrong chain or contract)
    InvalidDomain = 13,
    /// Meta-transaction validity window has expired
    ExpiredTransaction = 14,

    // ── Nonce & Replay Protection Errors (20–29) ──
    /// The specified nonce is invalid or mismatched
    InvalidNonce = 20,
    /// The nonce has already been consumed (replay attempt)
    NonceAlreadyUsed = 21,
    /// The provided nonce does not match the expected sequential nonce
    NonceMismatch = 22,

    // ── Match Staking & Execution Errors (30–49) ──
    /// Staking or fee amount must be positive
    InvalidAmount = 30,
    /// Player has insufficient token balance
    InsufficientFunds = 31,
    /// Insufficient allowance granted to the forwarder contract
    InsufficientAllowance = 32,
    /// Match / game escrow record not found
    MatchNotFound = 33,
    /// Match with this ID already exists
    MatchAlreadyExists = 34,
    /// Match is already full (both players joined)
    MatchAlreadyFull = 35,
    /// Match is not in an active or joinable state
    MatchNotInProgress = 36,
    /// Match has already been settled
    MatchAlreadySettled = 37,
    /// Player 2 cannot be the same as Player 1
    SamePlayerJoining = 38,
    /// Downstream target contract invocation failed
    TargetCallFailed = 39,
    /// Batch request contains no transactions
    EmptyBatch = 40,
    /// Batch request exceeds maximum allowable size
    BatchTooLarge = 41,
    /// Length of batch requests, keys, and signatures mismatch
    InvalidBatchLengths = 42,
    /// Relayer gas fee compensation transfer failed
    FeeTransferFailed = 43,
    /// Reentrancy guard triggered
    ReentrantCall = 44,
}
