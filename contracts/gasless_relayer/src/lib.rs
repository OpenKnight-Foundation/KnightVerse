#![no_std]

pub mod error;
pub mod types;

#[cfg(test)]
mod test;

use error::RelayerError;
use soroban_sdk::token::TokenClient;
use soroban_sdk::{
    contract, contractimpl, panic_with_error, Address, Bytes, BytesN, Env, Symbol, Val, Vec,
};
use types::{DataKey, ForwardRequest, GaslessMatchStakeRequest, MatchEscrow, MatchState};

const MAX_BATCH_SIZE: u32 = 20;

#[contract]
pub struct GaslessRelayer;

#[contractimpl]
impl GaslessRelayer {
    // ────────────────────────────────────────────────────────────────────────
    // Initialization & Admin Governance
    // ────────────────────────────────────────────────────────────────────────

    /// Initialize the Gasless Meta-Transaction Relayer contract.
    ///
    /// # Parameters
    /// - `admin`: Governance address with administrative privileges.
    /// - `network_passphrase_hash`: SHA-256 hash of the Stellar network passphrase
    ///   (used in domain separator calculation to prevent cross-network replays).
    /// - `open_relayers`: If true, any relayer can submit meta-transactions. If false,
    ///   only whitelisted relayers can submit.
    pub fn initialize(
        env: Env,
        admin: Address,
        network_passphrase_hash: BytesN<32>,
        open_relayers: bool,
    ) -> Result<(), RelayerError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(RelayerError::AlreadyInitialized);
        }

        admin.require_auth();

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::NetworkHash, &network_passphrase_hash);
        env.storage()
            .instance()
            .set(&DataKey::OpenRelayers, &open_relayers);
        env.storage().instance().set(&DataKey::Paused, &false);
        env.storage()
            .instance()
            .set(&DataKey::TotalRelayedCount, &0u64);
        env.storage()
            .instance()
            .set(&DataKey::TotalVolumeStaked, &0i128);

        env.events().publish(
            (
                Symbol::new(&env, "gasless"),
                Symbol::new(&env, "init"),
            ),
            (admin, network_passphrase_hash, open_relayers),
        );

        Ok(())
    }

    /// Update the contract administrator.
    pub fn set_admin(env: Env, new_admin: Address) -> Result<(), RelayerError> {
        Self::require_admin(&env)?;
        new_admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &new_admin);
        env.events().publish(
            (
                Symbol::new(&env, "gasless"),
                Symbol::new(&env, "admin_changed"),
            ),
            new_admin,
        );
        Ok(())
    }

    /// Add an authorized relayer to the whitelist.
    pub fn add_relayer(env: Env, relayer: Address) -> Result<(), RelayerError> {
        Self::require_admin(&env)?;
        env.storage()
            .instance()
            .set(&DataKey::Relayers(relayer.clone()), &true);
        env.events().publish(
            (
                Symbol::new(&env, "gasless"),
                Symbol::new(&env, "relayer_added"),
            ),
            relayer,
        );
        Ok(())
    }

    /// Remove a relayer from the whitelist.
    pub fn remove_relayer(env: Env, relayer: Address) -> Result<(), RelayerError> {
        Self::require_admin(&env)?;
        env.storage()
            .instance()
            .remove(&DataKey::Relayers(relayer.clone()));
        env.events().publish(
            (
                Symbol::new(&env, "gasless"),
                Symbol::new(&env, "relayer_removed"),
            ),
            relayer,
        );
        Ok(())
    }

    /// Toggle open relayer policy (permissionless vs whitelisted relaying).
    pub fn set_open_relayers(env: Env, is_open: bool) -> Result<(), RelayerError> {
        Self::require_admin(&env)?;
        env.storage()
            .instance()
            .set(&DataKey::OpenRelayers, &is_open);
        env.events().publish(
            (
                Symbol::new(&env, "gasless"),
                Symbol::new(&env, "open_relayers_set"),
            ),
            is_open,
        );
        Ok(())
    }

    /// Emergency pause all meta-transaction relaying and match staking.
    pub fn pause(env: Env) -> Result<(), RelayerError> {
        Self::require_admin(&env)?;
        env.storage().instance().set(&DataKey::Paused, &true);
        env.events().publish(
            (
                Symbol::new(&env, "gasless"),
                Symbol::new(&env, "paused"),
            ),
            (),
        );
        Ok(())
    }

    /// Unpause contract operations.
    pub fn unpause(env: Env) -> Result<(), RelayerError> {
        Self::require_admin(&env)?;
        env.storage().instance().set(&DataKey::Paused, &false);
        env.events().publish(
            (
                Symbol::new(&env, "gasless"),
                Symbol::new(&env, "unpaused"),
            ),
            (),
        );
        Ok(())
    }

    /// Returns true if the contract is paused.
    pub fn is_paused(env: Env) -> bool {
        env.storage()
            .instance()
            .get(&DataKey::Paused)
            .unwrap_or(false)
    }

    /// Checks if a relayer is authorized to submit transactions.
    pub fn is_relayer_authorized(env: Env, relayer: Address) -> bool {
        let is_open: bool = env
            .storage()
            .instance()
            .get(&DataKey::OpenRelayers)
            .unwrap_or(false);
        if is_open {
            return true;
        }
        env.storage()
            .instance()
            .get(&DataKey::Relayers(relayer))
            .unwrap_or(false)
    }

    // ────────────────────────────────────────────────────────────────────────
    // Player Signer Key Registration & Nonce Tracking
    // ────────────────────────────────────────────────────────────────────────

    /// Register or update the Ed25519 signer public key for a player address.
    pub fn register_signer_key(
        env: Env,
        player: Address,
        signer_pubkey: BytesN<32>,
    ) -> Result<(), RelayerError> {
        player.require_auth();
        env.storage()
            .instance()
            .set(&DataKey::PlayerSigner(player.clone()), &signer_pubkey);
        env.events().publish(
            (
                Symbol::new(&env, "gasless"),
                Symbol::new(&env, "signer_registered"),
            ),
            (player, signer_pubkey),
        );
        Ok(())
    }

    /// Get registered Ed25519 signer public key for a player address.
    pub fn get_signer_key(env: Env, player: Address) -> Option<BytesN<32>> {
        env.storage()
            .instance()
            .get(&DataKey::PlayerSigner(player))
    }

    /// Retrieve the current expected replay-prevention nonce for a user.
    pub fn get_nonce(env: Env, user: Address) -> u64 {
        env.storage()
            .instance()
            .get(&DataKey::UserNonce(user))
            .unwrap_or(0u64)
    }

    /// Allow a player to increment/bump their nonce, invalidating any pending off-chain signed requests.
    pub fn bump_nonce(env: Env, user: Address) -> Result<u64, RelayerError> {
        user.require_auth();
        let current_nonce: u64 = env
            .storage()
            .instance()
            .get(&DataKey::UserNonce(user.clone()))
            .unwrap_or(0u64);
        let new_nonce = current_nonce + 1;
        env.storage()
            .instance()
            .set(&DataKey::UserNonce(user.clone()), &new_nonce);

        env.events().publish(
            (
                Symbol::new(&env, "gasless"),
                Symbol::new(&env, "nonce_bumped"),
            ),
            (user, new_nonce),
        );
        Ok(new_nonce)
    }

    // ────────────────────────────────────────────────────────────────────────
    // Domain Separator & Structured Typed Data Hashing (EIP-712 / SEP Style)
    // ────────────────────────────────────────────────────────────────────────

    /// Calculate the EIP-712 / SEP style domain separator for this forwarder instance.
    pub fn get_domain_separator(env: Env) -> BytesN<32> {
        Self::compute_domain_separator(&env)
    }

    /// Compute the structured typed data digest for a gasless match stake request.
    pub fn get_match_stake_digest(
        env: Env,
        request: GaslessMatchStakeRequest,
    ) -> BytesN<32> {
        Self::compute_match_stake_digest(&env, &request)
    }

    /// Compute the structured typed data digest for a generic forward request.
    pub fn get_forward_request_digest(
        env: Env,
        request: ForwardRequest,
    ) -> BytesN<32> {
        Self::compute_forward_request_digest(&env, &request)
    }

    // ────────────────────────────────────────────────────────────────────────
    // Core Execution: Gasless Match Staking
    // ────────────────────────────────────────────────────────────────────────

    /// Execute a gasless chess match stake on behalf of a Web2 player.
    ///
    /// The submitting `relayer` pays the Stellar network gas fee on behalf of the player.
    /// The player's off-chain Ed25519 signature over structured typed data is verified
    /// with strict sequential nonce replay protection.
    ///
    /// # Parameters
    /// - `relayer`: Account submitting the transaction and paying network gas.
    /// - `request`: Staking payload containing match ID, token, stake amount, is_creator flag, nonce, and expiration.
    /// - `signer_pubkey`: Ed25519 public key of the player.
    /// - `signature`: 64-byte Ed25519 signature over the EIP-712 / SEP style typed digest.
    pub fn gasless_stake_match(
        env: Env,
        relayer: Address,
        request: GaslessMatchStakeRequest,
        signer_pubkey: BytesN<32>,
        signature: BytesN<64>,
    ) -> Result<(), RelayerError> {
        Self::check_not_paused(&env)?;

        relayer.require_auth();
        if !Self::is_relayer_authorized(env.clone(), relayer.clone()) {
            return Err(RelayerError::RelayerNotAuthorized);
        }

        // 1. Verify expiration window (valid_until = 0 means no expiration)
        if request.valid_until > 0 && (env.ledger().sequence() as u64) > request.valid_until {
            return Err(RelayerError::ExpiredTransaction);
        }

        // 2. Validate amount
        if request.amount <= 0 {
            return Err(RelayerError::InvalidAmount);
        }

        // 3. Verify registered signer mapping if one exists for the player
        if let Some(stored_pk) = env
            .storage()
            .instance()
            .get::<DataKey, BytesN<32>>(&DataKey::PlayerSigner(request.player.clone()))
        {
            if stored_pk != signer_pubkey {
                return Err(RelayerError::SignerMismatch);
            }
        }

        // 4. Nonce-based replay protection
        let current_nonce: u64 = env
            .storage()
            .instance()
            .get(&DataKey::UserNonce(request.player.clone()))
            .unwrap_or(0u64);

        if request.nonce != current_nonce {
            if request.nonce < current_nonce {
                return Err(RelayerError::NonceAlreadyUsed);
            } else {
                return Err(RelayerError::NonceMismatch);
            }
        }

        // 5. Cryptographic signature verification over EIP-712 / SEP style typed data
        let digest: BytesN<32> = Self::compute_match_stake_digest(&env, &request);
        let digest_bytes: Bytes = digest.into();
        env.crypto()
            .ed25519_verify(&signer_pubkey, &digest_bytes, &signature);

        // 6. State update before external interactions (checks-effects-interactions)
        env.storage()
            .instance()
            .set(&DataKey::UserNonce(request.player.clone()), &(current_nonce + 1));

        Self::reentrancy_enter(&env)?;

        // 7. Transfer staked tokens from player to forwarder escrow
        let contract_address = env.current_contract_address();
        let token_client = TokenClient::new(&env, &request.token);

        token_client.transfer_from(
            &contract_address,
            &request.player,
            &contract_address,
            &request.amount,
        );

        // 8. Escrow state management
        let game_key = DataKey::Matches(request.game_id);
        if request.is_creator {
            if env.storage().instance().has(&game_key) {
                Self::reentrancy_exit(&env);
                return Err(RelayerError::MatchAlreadyExists);
            }

            let escrow = MatchEscrow {
                game_id: request.game_id,
                token: request.token.clone(),
                player1: request.player.clone(),
                player2: None,
                wager_amount: request.amount,
                total_pot: request.amount,
                state: MatchState::Created,
                created_at: env.ledger().sequence() as u64,
                settled_at: None,
                winner: None,
            };
            env.storage().instance().set(&game_key, &escrow);
        } else {
            let mut escrow: MatchEscrow = env
                .storage()
                .instance()
                .get(&game_key)
                .ok_or_else(|| {
                    Self::reentrancy_exit(&env);
                    RelayerError::MatchNotFound
                })?;

            if escrow.state != MatchState::Created {
                Self::reentrancy_exit(&env);
                return Err(RelayerError::MatchNotInProgress);
            }
            if escrow.player2.is_some() {
                Self::reentrancy_exit(&env);
                return Err(RelayerError::MatchAlreadyFull);
            }
            if escrow.player1 == request.player {
                Self::reentrancy_exit(&env);
                return Err(RelayerError::SamePlayerJoining);
            }
            if escrow.token != request.token || escrow.wager_amount != request.amount {
                Self::reentrancy_exit(&env);
                return Err(RelayerError::InvalidAmount);
            }

            escrow.player2 = Some(request.player.clone());
            escrow.total_pot += request.amount;
            escrow.state = MatchState::Active;
            env.storage().instance().set(&game_key, &escrow);
        }

        // 9. Update metrics
        let total_relayed: u64 = env
            .storage()
            .instance()
            .get(&DataKey::TotalRelayedCount)
            .unwrap_or(0u64);
        env.storage()
            .instance()
            .set(&DataKey::TotalRelayedCount, &(total_relayed + 1));

        let total_volume: i128 = env
            .storage()
            .instance()
            .get(&DataKey::TotalVolumeStaked)
            .unwrap_or(0i128);
        env.storage()
            .instance()
            .set(&DataKey::TotalVolumeStaked, &(total_volume + request.amount));

        Self::reentrancy_exit(&env);

        // 10. Emit event
        env.events().publish(
            (
                Symbol::new(&env, "gasless"),
                Symbol::new(&env, "match_staked"),
            ),
            (
                request.game_id,
                request.player,
                request.amount,
                request.is_creator,
                relayer,
            ),
        );

        Ok(())
    }

    // ────────────────────────────────────────────────────────────────────────
    // Core Execution: Generic Meta-Transaction Forwarding
    // ────────────────────────────────────────────────────────────────────────

    /// Forward and execute an arbitrary smart contract invocation signed off-chain by a player.
    ///
    /// # Parameters
    /// - `relayer`: Submitting relayer account paying network gas fees.
    /// - `request`: Generic forward request including target contract, function, arguments, nonce, fee, and expiration.
    /// - `signer_pubkey`: Player's Ed25519 public key.
    /// - `signature`: 64-byte Ed25519 signature over the forward request typed digest.
    pub fn execute_meta_transaction(
        env: Env,
        relayer: Address,
        request: ForwardRequest,
        signer_pubkey: BytesN<32>,
        signature: BytesN<64>,
    ) -> Result<Val, RelayerError> {
        Self::check_not_paused(&env)?;

        relayer.require_auth();
        if !Self::is_relayer_authorized(env.clone(), relayer.clone()) {
            return Err(RelayerError::RelayerNotAuthorized);
        }

        Self::execute_meta_tx_internal(&env, &relayer, &request, &signer_pubkey, &signature)
    }

    /// Execute a batch of meta-transactions in a single on-chain invocation.
    pub fn execute_meta_tx_batch(
        env: Env,
        relayer: Address,
        requests: Vec<ForwardRequest>,
        signer_pubkeys: Vec<BytesN<32>>,
        signatures: Vec<BytesN<64>>,
    ) -> Result<Vec<Val>, RelayerError> {
        Self::check_not_paused(&env)?;

        relayer.require_auth();
        if !Self::is_relayer_authorized(env.clone(), relayer.clone()) {
            return Err(RelayerError::RelayerNotAuthorized);
        }

        let count = requests.len();
        if count == 0 {
            return Err(RelayerError::EmptyBatch);
        }
        if count > MAX_BATCH_SIZE {
            return Err(RelayerError::BatchTooLarge);
        }
        if signer_pubkeys.len() != count || signatures.len() != count {
            return Err(RelayerError::InvalidBatchLengths);
        }

        let mut results = Vec::new(&env);
        for i in 0..count {
            let req = requests.get(i).unwrap();
            let pk = signer_pubkeys.get(i).unwrap();
            let sig = signatures.get(i).unwrap();

            let val = Self::execute_meta_tx_internal(
                &env,
                &relayer,
                &req,
                &pk,
                &sig,
            )?;
            results.push_back(val);
        }

        Ok(results)
    }

    fn execute_meta_tx_internal(
        env: &Env,
        relayer: &Address,
        request: &ForwardRequest,
        signer_pubkey: &BytesN<32>,
        signature: &BytesN<64>,
    ) -> Result<Val, RelayerError> {
        // 1. Expiration check
        if request.valid_until > 0 && (env.ledger().sequence() as u64) > request.valid_until {
            return Err(RelayerError::ExpiredTransaction);
        }

        // 2. Signer mapping check
        if let Some(stored_pk) = env
            .storage()
            .instance()
            .get::<DataKey, BytesN<32>>(&DataKey::PlayerSigner(request.from.clone()))
        {
            if stored_pk != *signer_pubkey {
                return Err(RelayerError::SignerMismatch);
            }
        }

        // 3. Replay protection
        let current_nonce: u64 = env
            .storage()
            .instance()
            .get(&DataKey::UserNonce(request.from.clone()))
            .unwrap_or(0u64);

        if request.nonce != current_nonce {
            if request.nonce < current_nonce {
                return Err(RelayerError::NonceAlreadyUsed);
            } else {
                return Err(RelayerError::NonceMismatch);
            }
        }

        // 4. Signature verification
        let digest: BytesN<32> = Self::compute_forward_request_digest(env, request);
        let digest_bytes: Bytes = digest.into();
        env.crypto()
            .ed25519_verify(signer_pubkey, &digest_bytes, signature);

        // 5. Update nonce
        env.storage()
            .instance()
            .set(&DataKey::UserNonce(request.from.clone()), &(current_nonce + 1));

        Self::reentrancy_enter(env)?;

        // 6. Execute target invocation
        let result: Val = env.invoke_contract(&request.target, &request.function, request.args.clone());

        // 7. Optional relayer fee compensation in tokens
        if request.fee_amount > 0 {
            if let Some(fee_token_addr) = &request.fee_token {
                let token_client = TokenClient::new(env, fee_token_addr);
                token_client.transfer_from(
                    &env.current_contract_address(),
                    &request.from,
                    relayer,
                    &request.fee_amount,
                );
            }
        }

        // 8. Update metrics
        let total_relayed: u64 = env
            .storage()
            .instance()
            .get(&DataKey::TotalRelayedCount)
            .unwrap_or(0u64);
        env.storage()
            .instance()
            .set(&DataKey::TotalRelayedCount, &(total_relayed + 1));

        Self::reentrancy_exit(env);

        // 9. Event
        env.events().publish(
            (
                Symbol::new(env, "gasless"),
                Symbol::new(env, "forwarded"),
            ),
            (
                request.from.clone(),
                request.target.clone(),
                request.function.clone(),
                request.nonce,
                relayer.clone(),
            ),
        );

        Ok(result)
    }

    // ────────────────────────────────────────────────────────────────────────
    // Match Settlement & Cancellation
    // ────────────────────────────────────────────────────────────────────────

    /// Settle a completed match and disburse the escrowed prize pot.
    ///
    /// # Parameters
    /// - `winner`: `Some(address)` to award the pot to the winner, or `None` on draw (refunds 50/50).
    pub fn settle_match(
        env: Env,
        game_id: u64,
        winner: Option<Address>,
    ) -> Result<(), RelayerError> {
        Self::require_admin(&env)?;

        let game_key = DataKey::Matches(game_id);
        let mut escrow: MatchEscrow = env
            .storage()
            .instance()
            .get(&game_key)
            .ok_or(RelayerError::MatchNotFound)?;

        if escrow.state != MatchState::Active {
            return Err(RelayerError::MatchNotInProgress);
        }

        let player2 = escrow.player2.clone().ok_or(RelayerError::MatchNotInProgress)?;
        let token_client = TokenClient::new(&env, &escrow.token);
        let contract_address = env.current_contract_address();

        Self::reentrancy_enter(&env)?;

        match &winner {
            Some(w) => {
                if *w != escrow.player1 && *w != player2 {
                    Self::reentrancy_exit(&env);
                    return Err(RelayerError::Unauthorized);
                }
                token_client.transfer(&contract_address, w, &escrow.total_pot);
            }
            None => {
                // Draw: split pot evenly back to both players
                token_client.transfer(&contract_address, &escrow.player1, &escrow.wager_amount);
                token_client.transfer(&contract_address, &player2, &escrow.wager_amount);
            }
        }

        escrow.state = MatchState::Settled;
        escrow.settled_at = Some(env.ledger().sequence() as u64);
        escrow.winner = winner.clone();
        env.storage().instance().set(&game_key, &escrow);

        Self::reentrancy_exit(&env);

        env.events().publish(
            (
                Symbol::new(&env, "gasless"),
                Symbol::new(&env, "match_settled"),
            ),
            (game_id, winner, escrow.total_pot),
        );

        Ok(())
    }

    /// Cancel an unmatched match and refund Player 1's escrowed stake.
    pub fn cancel_unmatched_game(env: Env, game_id: u64) -> Result<(), RelayerError> {
        let game_key = DataKey::Matches(game_id);
        let mut escrow: MatchEscrow = env
            .storage()
            .instance()
            .get(&game_key)
            .ok_or(RelayerError::MatchNotFound)?;

        escrow.player1.require_auth();

        if escrow.state != MatchState::Created || escrow.player2.is_some() {
            return Err(RelayerError::MatchNotInProgress);
        }

        let contract_address = env.current_contract_address();
        let token_client = TokenClient::new(&env, &escrow.token);

        Self::reentrancy_enter(&env)?;

        token_client.transfer(&contract_address, &escrow.player1, &escrow.wager_amount);

        escrow.state = MatchState::Cancelled;
        escrow.settled_at = Some(env.ledger().sequence() as u64);
        env.storage().instance().set(&game_key, &escrow);

        Self::reentrancy_exit(&env);

        env.events().publish(
            (
                Symbol::new(&env, "gasless"),
                Symbol::new(&env, "match_cancelled"),
            ),
            (game_id, escrow.player1, escrow.wager_amount),
        );

        Ok(())
    }

    /// Query match escrow status and details.
    pub fn get_match(env: Env, game_id: u64) -> Result<MatchEscrow, RelayerError> {
        env.storage()
            .instance()
            .get(&DataKey::Matches(game_id))
            .ok_or(RelayerError::MatchNotFound)
    }

    /// Total count of all meta-transactions successfully executed.
    pub fn get_total_relayed_count(env: Env) -> u64 {
        env.storage()
            .instance()
            .get(&DataKey::TotalRelayedCount)
            .unwrap_or(0u64)
    }

    /// Total token stake volume relayed through the forwarder contract.
    pub fn get_total_volume_staked(env: Env) -> i128 {
        env.storage()
            .instance()
            .get(&DataKey::TotalVolumeStaked)
            .unwrap_or(0i128)
    }

    // ────────────────────────────────────────────────────────────────────────
    // Internal Helper Functions
    // ────────────────────────────────────────────────────────────────────────

    fn require_admin(env: &Env) -> Result<Address, RelayerError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(RelayerError::NotInitialized)?;
        admin.require_auth();
        Ok(admin)
    }

    fn check_not_paused(env: &Env) -> Result<(), RelayerError> {
        if Self::is_paused(env.clone()) {
            Err(RelayerError::ContractPaused)
        } else {
            Ok(())
        }
    }

    fn reentrancy_enter(env: &Env) -> Result<(), RelayerError> {
        let is_locked: bool = env
            .storage()
            .instance()
            .get(&DataKey::ReentrancyGuard)
            .unwrap_or(false);
        if is_locked {
            panic_with_error!(env, RelayerError::ReentrantCall);
        }
        env.storage()
            .instance()
            .set(&DataKey::ReentrancyGuard, &true);
        Ok(())
    }

    fn reentrancy_exit(env: &Env) {
        env.storage()
            .instance()
            .set(&DataKey::ReentrancyGuard, &false);
    }

    fn compute_domain_separator(env: &Env) -> BytesN<32> {
        let network_hash: BytesN<32> = env
            .storage()
            .instance()
            .get(&DataKey::NetworkHash)
            .unwrap_or_else(|| BytesN::from_array(env, &[0u8; 32]));

        let mut domain_data = Bytes::new(env);
        domain_data.append(&Bytes::from_slice(
            env,
            b"EIP712Domain(string name,string version,bytes32 network_hash)",
        ));
        domain_data.append(&Bytes::from_slice(env, b"KnightVerseForwarder"));
        domain_data.append(&Bytes::from_slice(env, b"1"));
        domain_data.append(&Bytes::from_array(env, &network_hash.to_array()));

        env.crypto().sha256(&domain_data).into()
    }

    fn compute_match_stake_digest(
        env: &Env,
        req: &GaslessMatchStakeRequest,
    ) -> BytesN<32> {
        let domain_sep = Self::compute_domain_separator(env);

        let mut msg_data = Bytes::new(env);
        msg_data.append(&Bytes::from_slice(
            env,
            b"GaslessMatchStake(int128 amount,uint64 game_id,bool is_creator,uint64 nonce,uint64 valid_until)",
        ));

        let amount_bytes = req.amount.to_be_bytes();
        let game_id_bytes = req.game_id.to_be_bytes();
        let is_creator_byte = if req.is_creator { [1u8] } else { [0u8] };
        let nonce_bytes = req.nonce.to_be_bytes();
        let valid_until_bytes = req.valid_until.to_be_bytes();

        msg_data.append(&Bytes::from_slice(env, &amount_bytes));
        msg_data.append(&Bytes::from_slice(env, &game_id_bytes));
        msg_data.append(&Bytes::from_slice(env, &is_creator_byte));
        msg_data.append(&Bytes::from_slice(env, &nonce_bytes));
        msg_data.append(&Bytes::from_slice(env, &valid_until_bytes));

        let msg_hash: BytesN<32> = env.crypto().sha256(&msg_data).into();

        let mut typed_data = Bytes::new(env);
        typed_data.append(&Bytes::from_slice(env, b"\x19\x01"));
        typed_data.append(&Bytes::from_array(env, &domain_sep.to_array()));
        typed_data.append(&Bytes::from_array(env, &msg_hash.to_array()));

        env.crypto().sha256(&typed_data).into()
    }

    fn compute_forward_request_digest(
        env: &Env,
        req: &ForwardRequest,
    ) -> BytesN<32> {
        let domain_sep = Self::compute_domain_separator(env);

        let mut msg_data = Bytes::new(env);
        msg_data.append(&Bytes::from_slice(
            env,
            b"ForwardRequest(uint64 nonce,uint64 valid_until,int128 fee_amount)",
        ));

        let nonce_bytes = req.nonce.to_be_bytes();
        let valid_until_bytes = req.valid_until.to_be_bytes();
        let fee_amount_bytes = req.fee_amount.to_be_bytes();

        msg_data.append(&Bytes::from_slice(env, &nonce_bytes));
        msg_data.append(&Bytes::from_slice(env, &valid_until_bytes));
        msg_data.append(&Bytes::from_slice(env, &fee_amount_bytes));

        let msg_hash: BytesN<32> = env.crypto().sha256(&msg_data).into();

        let mut typed_data = Bytes::new(env);
        typed_data.append(&Bytes::from_slice(env, b"\x19\x01"));
        typed_data.append(&Bytes::from_array(env, &domain_sep.to_array()));
        typed_data.append(&Bytes::from_array(env, &msg_hash.to_array()));

        env.crypto().sha256(&typed_data).into()
    }
}
