use soroban_sdk::{contracttype, Address, Symbol, Val, Vec};

/// State of a gasless match staking escrow.
#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MatchState {
    /// Match created by Player 1 with initial stake locked.
    Created = 1,
    /// Player 2 joined with matching stake; match is in progress.
    Active = 2,
    /// Match resolved; funds disbursed to winner (or refunded on draw).
    Settled = 3,
    /// Match cancelled by creator prior to an opponent joining; stake refunded.
    Cancelled = 4,
}

/// Generic meta-transaction forward request for executing arbitrary contract invocations.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ForwardRequest {
    /// Address of the user authorizing the transaction.
    pub from: Address,
    /// Target smart contract address.
    pub target: Address,
    /// Function symbol to execute on the target contract.
    pub function: Symbol,
    /// Serialized arguments for the target function call.
    pub args: Vec<Val>,
    /// User's sequential nonce for replay protection.
    pub nonce: u64,
    /// Ledger sequence or timestamp after which this transaction is invalid (0 = no expiration).
    pub valid_until: u64,
    /// Optional token address used for compensating the relayer.
    pub fee_token: Option<Address>,
    /// Amount paid to the relayer as compensation for network fees.
    pub fee_amount: i128,
}

/// Specialized gasless match staking request for seamless Web2 player onboarding.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GaslessMatchStakeRequest {
    /// Web2 player address requesting the match stake.
    pub player: Address,
    /// Address of the token contract (e.g. SEP-41 token / SAC).
    pub token: Address,
    /// Amount of tokens to stake on the chess match.
    pub amount: i128,
    /// Unique match identifier.
    pub game_id: u64,
    /// True if creating a new match; false if joining an existing match.
    pub is_creator: bool,
    /// Sequential replay-protection nonce for this player.
    pub nonce: u64,
    /// Expiration ledger sequence (0 = no expiration).
    pub valid_until: u64,
}

/// On-chain escrow record tracking locked stakes for gasless matches.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MatchEscrow {
    /// Unique match identifier.
    pub game_id: u64,
    /// Token contract address.
    pub token: Address,
    /// Creator of the match (Player 1).
    pub player1: Address,
    /// Opponent who joined (Player 2), if any.
    pub player2: Option<Address>,
    /// Stake per player.
    pub wager_amount: i128,
    /// Total prize pot locked in escrow (player1 + player2 stakes).
    pub total_pot: i128,
    /// Current lifecycle state of the match.
    pub state: MatchState,
    /// Ledger sequence at which the match escrow was created.
    pub created_at: u64,
    /// Ledger sequence at which the match was settled, if applicable.
    pub settled_at: Option<u64>,
    /// Winning player address, if settled.
    pub winner: Option<Address>,
}

/// Storage keys for the meta-transaction forwarder contract.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    /// Contract administrator address.
    Admin,
    /// Emergency pause flag (bool).
    Paused,
    /// Stellar network passphrase SHA-256 hash.
    NetworkHash,
    /// Boolean flag allowing any relayer to submit (open vs whitelisted relayers).
    OpenRelayers,
    /// Map of authorized relayer addresses: Relayers(Address) -> bool.
    Relayers(Address),
    /// Registered Ed25519 signer public key for an address: PlayerSigner(Address) -> BytesN<32>.
    PlayerSigner(Address),
    /// Monotonic sequential replay prevention nonce: UserNonce(Address) -> u64.
    UserNonce(Address),
    /// Match escrow storage: Matches(u64) -> MatchEscrow.
    Matches(u64),
    /// Total count of successfully relayed meta-transactions.
    TotalRelayedCount,
    /// Total token stake volume relayed through the contract.
    TotalVolumeStaked,
    /// Reentrancy guard lock flag.
    ReentrancyGuard,
}
