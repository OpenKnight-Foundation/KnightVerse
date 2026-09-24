# Gasless Meta-Transaction Relayer Contract (#1148 SC-48)

A Soroban smart contract providing a secure, high-throughput gasless meta-transaction forwarder and match staking relayer for KnightVerse.

## Overview

Web2 chess players often do not hold native XLM for transaction fees. The `GaslessRelayer` contract allows players to sign match stakes, moves, and contract calls off-chain with an Ed25519 keypair. Sponsored relayers submit these transactions on-chain, paying network gas fees on behalf of the player.

## Key Features

1. **Nonce-Based Replay Protection**:
   - Monotonic sequential nonces tracked per user address (`DataKey::UserNonce(Address)`).
   - Prevents duplicate execution of captured or intercepted transactions.
   - `bump_nonce(user)` allows players to revoke/invalidate any pending off-chain signed meta-transactions.

2. **EIP-712 / SEP Style Structured Typed Data Hashing**:
   - Domain separator incorporates contract address and network passphrase hash (`\x19\x01` prefix).
   - Protects against cross-contract and cross-network replay attacks.
   - Cryptographic verification via Stellar native `env.crypto().ed25519_verify(...)`.

3. **Gasless Match Staking & Escrow**:
   - Web2 players sign off-chain match creation (`is_creator = true`) and match joining (`is_creator = false`).
   - Tokens are pulled from the player's approved allowance into contract escrow.
   - Match lifecycle: `Created` → `Active` → `Settled` (or `Cancelled`).
   - Settle match disburses prize pot to winner (or 50/50 split on draw).

4. **Generic Meta-Transaction Forwarding & Batching**:
   - `execute_meta_transaction` forwards arbitrary contract calls.
   - `execute_meta_tx_batch` executes multiple meta-transactions in a single invocation.

5. **Relayer Access & Governance**:
   - Supports permissionless (`open_relayers = true`) and whitelisted relayer policies.
   - Emergency circuit breaker pausing (`pause` / `unpause`).

## Contract Interface

### Gasless Match Staking
```rust
pub fn gasless_stake_match(
    env: Env,
    relayer: Address,
    request: GaslessMatchStakeRequest,
    signer_pubkey: BytesN<32>,
    signature: BytesN<64>,
) -> Result<(), RelayerError>;
```

### Generic Meta-Transaction Forwarding
```rust
pub fn execute_meta_transaction(
    env: Env,
    relayer: Address,
    request: ForwardRequest,
    signer_pubkey: BytesN<32>,
    signature: BytesN<64>,
) -> Result<Val, RelayerError>;
```

### Batch Meta-Transactions
```rust
pub fn execute_meta_tx_batch(
    env: Env,
    relayer: Address,
    requests: Vec<ForwardRequest>,
    signer_pubkeys: Vec<BytesN<32>>,
    signatures: Vec<BytesN<64>>,
) -> Result<Vec<Val>, RelayerError>;
```

### Nonce & Key Management
```rust
pub fn get_nonce(env: Env, user: Address) -> u64;
pub fn bump_nonce(env: Env, user: Address) -> Result<u64, RelayerError>;
pub fn register_signer_key(env: Env, player: Address, signer_pubkey: BytesN<32>) -> Result<(), RelayerError>;
pub fn get_signer_key(env: Env, player: Address) -> Option<BytesN<32>>;
```

## Running Tests

```bash
cargo test -p gasless_relayer
```
