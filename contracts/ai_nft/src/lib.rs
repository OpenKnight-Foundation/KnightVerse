#![no_std]
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, panic_with_error, symbol_short, Address,
    BytesN, Env, Map, String, Symbol,
};

// AI NFT metadata structure
#[contracttype]
#[derive(Clone, Debug)]
pub struct AINFTMetadata {
    pub owner: Address,
    pub nft_id: u64,
    pub metadata_hash: BytesN<32>,  // IPFS/content hash
    pub personality_traits: String, // JSON describing personality
    pub created_at: u64,
    pub minter: Address, // Original minting user
}

// Contract storage keys
const ADMIN: Symbol = symbol_short!("ADMIN");
const NFT_COUNTER: Symbol = symbol_short!("NFT_CNT");
const NFT_OWNERS: Symbol = symbol_short!("OWNERS");
const NFT_METADATA: Symbol = symbol_short!("METADATA");
const MINTER_REGISTRY: Symbol = symbol_short!("MINTER");
// Pausable extension (SC-11)
const PAUSED: Symbol = symbol_short!("PAUSED");

// Contract errors
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum ContractError {
    NotAuthorized = 1,
    NFTNotFound = 2,
    InvalidMetadataHash = 3,
    AlreadyTransferred = 4,
    InvalidOwner = 5,
    MinterMismatch = 6,
    /// Contract is paused for emergency halt (SC-11)
    ContractPaused = 7,
    /// Contract has already been initialized
    AlreadyInitialized = 8,
    /// Caller is not the contract admin
    NotAdmin = 9,
    /// Contract is already paused
    AlreadyPaused = 10,
    /// Contract is not currently paused
    NotPaused = 11,
}

#[contract]
pub struct AINFTContract;

#[contractimpl]
impl AINFTContract {
    /// Initialize the AI NFT contract with an admin address
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&ADMIN) {
            panic_with_error!(&env, ContractError::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage().instance().set(&ADMIN, &admin);
        env.storage().instance().set(&NFT_COUNTER, &0u64);
    }

    /// Get the current admin
    pub fn admin(env: Env) -> Address {
        env.storage().instance().get(&ADMIN).expect("Admin not set")
    }

    // ── Pausable extension (SC-11) ────────────────────────────────────────────

    /// Pause the contract — blocks all state-mutating operations.
    /// Only the contract admin may call this.
    pub fn pause(env: Env, caller: Address) {
        caller.require_auth();
        let admin: Address = env.storage().instance().get(&ADMIN).expect("Admin not set");
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
        let admin: Address = env.storage().instance().get(&ADMIN).expect("Admin not set");
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

    /// Mint a new AI NFT with metadata hash
    pub fn mint(
        env: Env,
        minter: Address,
        metadata_hash: BytesN<32>,
        personality_traits: String,
    ) -> u64 {
        Self::check_not_paused(&env);
        let admin = Self::admin(env.clone());
        admin.require_auth();
        minter.require_auth();

        Self::require_not_paused(&env)?;

        // Increment NFT counter
        let mut nft_counter: u64 = env.storage().instance().get(&NFT_COUNTER).unwrap_or(0);
        nft_counter += 1;
        env.storage().instance().set(&NFT_COUNTER, &nft_counter);

        // Create NFT metadata
        let nft = AINFTMetadata {
            owner: minter.clone(),
            nft_id: nft_counter,
            metadata_hash: metadata_hash.clone(),
            personality_traits: personality_traits.clone(),
            created_at: env.ledger().sequence() as u64,
            minter: minter.clone(),
        };

        // Store metadata with owner association
        let mut nft_metadata: Map<u64, AINFTMetadata> = env
            .storage()
            .instance()
            .get(&NFT_METADATA)
            .unwrap_or(Map::new(&env));
        nft_metadata.set(nft_counter, nft);
        env.storage().instance().set(&NFT_METADATA, &nft_metadata);

        // Record owner
        let mut owners: Map<u64, Address> = env
            .storage()
            .instance()
            .get(&NFT_OWNERS)
            .unwrap_or(Map::new(&env));
        owners.set(nft_counter, minter.clone());
        env.storage().instance().set(&NFT_OWNERS, &owners);

        // Record minter for this NFT
        let mut minter_registry: Map<u64, Address> = env
            .storage()
            .instance()
            .get(&MINTER_REGISTRY)
            .unwrap_or(Map::new(&env));
        minter_registry.set(nft_counter, minter.clone());
        env.storage()
            .instance()
            .set(&MINTER_REGISTRY, &minter_registry);

        // Emit NFT minted event
        env.events().publish(
            (symbol_short!("ai_nft"), symbol_short!("mint")),
            (nft_counter, minter, metadata_hash),
        );

        nft_counter
    }

    /// Transfer NFT from current owner to a new owner
    pub fn transfer(env: Env, nft_id: u64, to: Address) -> Result<(), ContractError> {
        Self::check_not_paused(&env);
        let mut owners: Map<u64, Address> = env
            .storage()
            .instance()
            .get(&NFT_OWNERS)
            .ok_or(ContractError::NFTNotFound)?;

        let current_owner = owners.get(nft_id).ok_or(ContractError::NFTNotFound)?;
        current_owner.require_auth();

        Self::require_not_paused(&env)?;

        // Update owner in NFT_OWNERS
        owners.set(nft_id, to.clone());
        env.storage().instance().set(&NFT_OWNERS, &owners);

        // Update owner in NFT_METADATA
        let mut nft_metadata: Map<u64, AINFTMetadata> = env
            .storage()
            .instance()
            .get(&NFT_METADATA)
            .ok_or(ContractError::NFTNotFound)?;
        let mut nft = nft_metadata.get(nft_id).ok_or(ContractError::NFTNotFound)?;
        nft.owner = to.clone();
        nft_metadata.set(nft_id, nft);
        env.storage().instance().set(&NFT_METADATA, &nft_metadata);

        // Emit NFT transferred event
        env.events().publish(
            (symbol_short!("ai_nft"), symbol_short!("transfer")),
            (nft_id, current_owner, to),
        );

        Ok(())
    }

    /// Get the current owner of an NFT
    pub fn owner_of(env: Env, nft_id: u64) -> Result<Address, ContractError> {
        let owners: Map<u64, Address> = env
            .storage()
            .instance()
            .get(&NFT_OWNERS)
            .ok_or(ContractError::NFTNotFound)?;

        owners.get(nft_id).ok_or(ContractError::NFTNotFound)
    }

    /// Get the minter of an NFT (original creator)
    pub fn minter_of(env: Env, nft_id: u64) -> Result<Address, ContractError> {
        let minter_registry: Map<u64, Address> = env
            .storage()
            .instance()
            .get(&MINTER_REGISTRY)
            .ok_or(ContractError::NFTNotFound)?;

        minter_registry
            .get(nft_id)
            .ok_or(ContractError::MinterMismatch)
    }

    /// Get full metadata of an NFT
    pub fn metadata(env: Env, nft_id: u64) -> Result<AINFTMetadata, ContractError> {
        let nft_metadata: Map<u64, AINFTMetadata> = env
            .storage()
            .instance()
            .get(&NFT_METADATA)
            .ok_or(ContractError::NFTNotFound)?;

        nft_metadata.get(nft_id).ok_or(ContractError::NFTNotFound)
    }

    /// Get total number of NFTs minted
    pub fn total_supply(env: Env) -> u64 {
        env.storage().instance().get(&NFT_COUNTER).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::{Address, BytesN, Env};

    #[test]
    fn test_ai_nft_mint_and_transfer() {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let minter = Address::generate(&env);
        let new_owner = Address::generate(&env);

        // Initialize contract
        let contract_id = env.register_contract(None, AINFTContract);
        let client = AINFTContractClient::new(&env, &contract_id);
        client.initialize(&admin);

        // Create metadata hash
        let metadata_hash: BytesN<32> = BytesN::from_array(&env, &[1u8; 32]);
        let personality = String::from_str(&env, "creative_artist_robot");

        // Mint NFT
        let nft_id = client.mint(&minter, &metadata_hash, &personality);
        assert_eq!(nft_id, 1u64);

        // Verify owner
        let owner = client.owner_of(&nft_id);
        assert_eq!(owner, minter);

        // Verify minter is recorded
        let minter_addr = client.minter_of(&nft_id);
        assert_eq!(minter_addr, minter);

        // Verify metadata includes minter
        let nft_meta = client.metadata(&nft_id);
        assert_eq!(nft_meta.minter, minter);
        assert_eq!(nft_meta.metadata_hash, metadata_hash);
        assert_eq!(nft_meta.personality_traits, personality);

        // Transfer to new owner
        client.transfer(&nft_id, &new_owner);

        // Verify new owner
        let new_owner_check = client.owner_of(&nft_id);
        assert_eq!(new_owner_check, new_owner);

        // Verify metadata owner is also updated
        let nft_meta_after = client.metadata(&nft_id);
        assert_eq!(nft_meta_after.owner, new_owner);

        // Verify minter is still the original minter
        let minter_check = client.minter_of(&nft_id);
        assert_eq!(minter_check, minter);

        // Verify total supply
        assert_eq!(client.total_supply(), 1u64);
    }

    #[test]
    fn test_multiple_nft_minting() {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let minter1 = Address::generate(&env);
        let minter2 = Address::generate(&env);

        let contract_id = env.register_contract(None, AINFTContract);
        let client = AINFTContractClient::new(&env, &contract_id);
        client.initialize(&admin);

        let metadata_hash1: BytesN<32> = BytesN::from_array(&env, &[1u8; 32]);
        let metadata_hash2: BytesN<32> = BytesN::from_array(&env, &[2u8; 32]);

        // Mint first NFT
        let nft_id1 = client.mint(&minter1, &metadata_hash1, &String::from_str(&env, "bot1"));

        // Mint second NFT with different minter
        let nft_id2 = client.mint(&minter2, &metadata_hash2, &String::from_str(&env, "bot2"));

        assert_eq!(nft_id1, 1u64);
        assert_eq!(nft_id2, 2u64);

        // Verify each NFT has correct minter and metadata
        let minter1_check = client.minter_of(&nft_id1);
        assert_eq!(minter1_check, minter1);

        let minter2_check = client.minter_of(&nft_id2);
        assert_eq!(minter2_check, minter2);

        let meta1 = client.metadata(&nft_id1);
        let meta2 = client.metadata(&nft_id2);

        assert_eq!(meta1.minter, minter1);
        assert_eq!(meta1.metadata_hash, metadata_hash1);

        assert_eq!(meta2.minter, minter2);
        assert_eq!(meta2.metadata_hash, metadata_hash2);

        // Total supply should be 2
        assert_eq!(client.total_supply(), 2u64);
    }

    #[test]
    fn test_metadata_hash_association() {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let minter = Address::generate(&env);

        let contract_id = env.register_contract(None, AINFTContract);
        let client = AINFTContractClient::new(&env, &contract_id);
        client.initialize(&admin);

        // Create specific metadata hash
        let metadata_hash: BytesN<32> = BytesN::from_array(&env, &[42u8; 32]);
        let personality = String::from_str(&env, "philosophical_ai");

        // Mint with specific hash
        let nft_id = client.mint(&minter, &metadata_hash, &personality);

        // Verify metadata hash is correctly stored and associated with minter
        let retrieved = client.metadata(&nft_id);
        assert_eq!(retrieved.metadata_hash, metadata_hash);
        assert_eq!(retrieved.minter, minter);
        assert_eq!(retrieved.owner, minter);
        assert_eq!(retrieved.personality_traits, personality);
    }

    // ── Circuit Breaker Tests ─────────────────────────────────────────────────

    fn setup_breaker<'a>(
        env: &'a Env,
        client: &AINFTContractClient<'a>,
        admin: &Address,
    ) -> emergency_circuit_breaker::PausableContractClient<'a> {
        use emergency_circuit_breaker::PausableContract;
        let cb_id = env.register_contract(None, PausableContract);
        let cb_client = emergency_circuit_breaker::PausableContractClient::new(env, &cb_id);
        cb_client.initialize(admin);
        client.initialize_circuit_breaker(admin, &cb_id);
        cb_client
    }

    /// mint is blocked when circuit breaker is paused.
    #[test]
    fn test_circuit_breaker_paused_blocks_mint() {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let minter = Address::generate(&env);

        let contract_id = env.register_contract(None, AINFTContract);
        let client = AINFTContractClient::new(&env, &contract_id);
        client.initialize(&admin);

        let cb_client = setup_breaker(&env, &client, &admin);
        cb_client.pause(&admin);

        let metadata_hash: BytesN<32> = BytesN::from_array(&env, &[1u8; 32]);
        let personality = String::from_str(&env, "blocked_bot");

        let result = client.try_mint(&minter, &metadata_hash, &personality);
        assert_eq!(result, Err(Ok(ContractError::CircuitBreakerTripped)));
    }

    /// transfer is blocked when circuit breaker is paused.
    #[test]
    fn test_circuit_breaker_paused_blocks_transfer() {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let minter = Address::generate(&env);
        let new_owner = Address::generate(&env);

        let contract_id = env.register_contract(None, AINFTContract);
        let client = AINFTContractClient::new(&env, &contract_id);
        client.initialize(&admin);

        // Mint while breaker is not yet paused
        let metadata_hash: BytesN<32> = BytesN::from_array(&env, &[2u8; 32]);
        let nft_id = client.mint(&minter, &metadata_hash, &String::from_str(&env, "live_bot"));

        // Now wire up and trip the breaker
        let cb_client = setup_breaker(&env, &client, &admin);
        cb_client.pause(&admin);

        let result = client.try_transfer(&nft_id, &new_owner);
        assert_eq!(result, Err(Ok(ContractError::CircuitBreakerTripped)));
    }

    /// After unpausing, mint and transfer work again.
    #[test]
    fn test_circuit_breaker_unpause_resumes_mint_and_transfer() {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let minter = Address::generate(&env);
        let new_owner = Address::generate(&env);

        let contract_id = env.register_contract(None, AINFTContract);
        let client = AINFTContractClient::new(&env, &contract_id);
        client.initialize(&admin);

        let cb_client = setup_breaker(&env, &client, &admin);
        cb_client.pause(&admin);

        let metadata_hash: BytesN<32> = BytesN::from_array(&env, &[3u8; 32]);
        // mint blocked while paused
        assert!(client
            .try_mint(&minter, &metadata_hash, &String::from_str(&env, "bot"))
            .is_err());

        cb_client.unpause(&admin);

        // Now mint succeeds
        let nft_id = client.mint(&minter, &metadata_hash, &String::from_str(&env, "bot"));
        assert_eq!(client.owner_of(&nft_id), minter);

        // And transfer succeeds
        client.transfer(&nft_id, &new_owner);
        assert_eq!(client.owner_of(&nft_id), new_owner);
    }
}
