#![no_std]

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, panic_with_error, Address, Env};

// Storage keys
#[contracttype]
pub enum DataKey {
    Paused,
    Admin,
    Balance(Address),
    TotalSupply,
    MatchEscrow(u64),
    /// SC-66: the pending admin-transfer proposal, if any.
    PendingAdmin,
}

#[contracttype]
pub struct MatchEscrowData {
    pub player: Address,
    pub amount: i128,
}

/// SC-66: a proposed admin handoff, timelocked so a compromised or
/// fat-fingered `transfer_admin` call can be noticed and cancelled before
/// it takes effect.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingAdmin {
    pub candidate: Address,
    pub effective_at: u64,
}

/// Delay between proposing an admin transfer and it taking effect.
pub const ADMIN_TRANSFER_TIMELOCK_SECS: u64 = 48 * 60 * 60;

/// Structured error codes for the PausableContract.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum ContractError {
    /// Contract has already been initialized
    AlreadyInitialized = 1,
    /// Caller is not the contract admin
    NotAdmin = 2,
    /// Contract is already paused
    AlreadyPaused = 3,
    /// Contract is not currently paused
    NotPaused = 4,
    /// Contract is paused — state-changing operations are blocked
    ContractPaused = 5,
    /// Amount must be a positive integer
    InvalidAmount = 6,
    /// Account has insufficient balance for the requested operation
    InsufficientBalance = 7,
    /// Match escrow with this ID already exists
    EscrowAlreadyExists = 8,
    /// SC-66: no admin-transfer proposal is currently pending
    NoPendingAdminTransfer = 9,
    /// SC-66: the timelock delay has not yet elapsed
    TimelockNotElapsed = 10,
}

#[contract]
pub struct PausableContract;

#[contractimpl]
impl PausableContract {
    /// Initialize the contract with an admin address
    pub fn initialize(env: Env, admin: Address) {
        // Ensure contract hasn't been initialized
        if env.storage().instance().has(&DataKey::Admin) {
            panic_with_error!(&env, ContractError::AlreadyInitialized);
        }

        // Set admin
        env.storage().instance().set(&DataKey::Admin, &admin);

        // Set initial paused state to false
        env.storage().instance().set(&DataKey::Paused, &false);

        // Initialize total supply
        env.storage().instance().set(&DataKey::TotalSupply, &0i128);
    }

    // ============================================
    // PAUSE/UNPAUSE FUNCTIONS (ADMIN ONLY)
    // ============================================

    /// Pause the contract - blocks all state-changing operations
    /// Only callable by admin
    pub fn pause(env: Env, caller: Address) {
        caller.require_auth();

        // Check if caller is admin
        Self::check_admin(&env, &caller);

        // Check if already paused
        let is_paused = Self::is_paused(&env);
        if is_paused {
            panic_with_error!(&env, ContractError::AlreadyPaused);
        }

        // Set paused state to true
        env.storage().instance().set(&DataKey::Paused, &true);

        // Emit event
        env.events()
            .publish((soroban_sdk::symbol_short!("paused"),), caller);
    }

    /// Unpause the contract - resumes normal operations
    /// Only callable by admin
    pub fn unpause(env: Env, caller: Address) {
        caller.require_auth();

        // Check if caller is admin
        Self::check_admin(&env, &caller);

        // Check if not paused
        let is_paused = Self::is_paused(&env);
        if !is_paused {
            panic_with_error!(&env, ContractError::NotPaused);
        }

        // Set paused state to false
        env.storage().instance().set(&DataKey::Paused, &false);

        // Emit event
        env.events()
            .publish((soroban_sdk::symbol_short!("unpaused"),), caller);
    }

    /// Check if contract is currently paused
    pub fn is_paused(env: &Env) -> bool {
        env.storage()
            .instance()
            .get(&DataKey::Paused)
            .unwrap_or(false)
    }

    // ============================================
    // STATE-CHANGING FUNCTIONS (PAUSABLE)
    // ============================================

    /// Deposit tokens (guarded by pause check)
    pub fn deposit(env: Env, from: Address, amount: i128) {
        from.require_auth();

        // CHECK PAUSED - This is the circuit breaker in action!
        Self::check_not_paused(&env);

        if amount <= 0 {
            panic_with_error!(&env, ContractError::InvalidAmount);
        }

        // Get current balance
        let balance_key = DataKey::Balance(from.clone());
        let current_balance: i128 = env.storage().instance().get(&balance_key).unwrap_or(0);

        // Update balance
        let new_balance = current_balance + amount;
        env.storage().instance().set(&balance_key, &new_balance);

        // Update total supply
        let total_supply: i128 = env
            .storage()
            .instance()
            .get(&DataKey::TotalSupply)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&DataKey::TotalSupply, &(total_supply + amount));

        // Emit event
        env.events().publish(
            (soroban_sdk::symbol_short!("deposit"), from.clone()),
            amount,
        );
    }

    /// Claim/withdraw tokens (guarded by pause check)
    pub fn claim(env: Env, to: Address, amount: i128) {
        to.require_auth();

        // CHECK PAUSED - Circuit breaker!
        Self::check_not_paused(&env);

        if amount <= 0 {
            panic_with_error!(&env, ContractError::InvalidAmount);
        }

        // Get current balance
        let balance_key = DataKey::Balance(to.clone());
        let current_balance: i128 = env.storage().instance().get(&balance_key).unwrap_or(0);

        // Check sufficient balance
        if current_balance < amount {
            panic_with_error!(&env, ContractError::InsufficientBalance);
        }

        // Update balance
        let new_balance = current_balance - amount;
        env.storage().instance().set(&balance_key, &new_balance);

        // Update total supply
        let total_supply: i128 = env
            .storage()
            .instance()
            .get(&DataKey::TotalSupply)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&DataKey::TotalSupply, &(total_supply - amount));

        // Emit event
        env.events()
            .publish((soroban_sdk::symbol_short!("claim"), to.clone()), amount);
    }

    /// Mint new tokens (guarded by pause check)
    pub fn mint(env: Env, caller: Address, to: Address, amount: i128) {
        caller.require_auth();

        // Only admin can mint
        Self::check_admin(&env, &caller);

        // CHECK PAUSED - Circuit breaker!
        Self::check_not_paused(&env);

        if amount <= 0 {
            panic_with_error!(&env, ContractError::InvalidAmount);
        }

        // Get current balance
        let balance_key = DataKey::Balance(to.clone());
        let current_balance: i128 = env.storage().instance().get(&balance_key).unwrap_or(0);

        // Update balance
        let new_balance = current_balance + amount;
        env.storage().instance().set(&balance_key, &new_balance);

        // Update total supply
        let total_supply: i128 = env
            .storage()
            .instance()
            .get(&DataKey::TotalSupply)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&DataKey::TotalSupply, &(total_supply + amount));

        // Emit event
        env.events()
            .publish((soroban_sdk::symbol_short!("mint"), to.clone()), amount);
    }

    /// Deposit into match escrow (guarded by pause check)
    pub fn deposit_match(env: Env, player: Address, match_id: u64, amount: i128) {
        player.require_auth();

        // CHECK PAUSED - Circuit breaker!
        Self::check_not_paused(&env);

        if amount <= 0 {
            panic_with_error!(&env, ContractError::InvalidAmount);
        }

        // Check if match escrow already exists
        let escrow_key = DataKey::MatchEscrow(match_id);
        if env.storage().instance().has(&escrow_key) {
            panic_with_error!(&env, ContractError::EscrowAlreadyExists);
        }

        // Get player's balance
        let balance_key = DataKey::Balance(player.clone());
        let current_balance: i128 = env.storage().instance().get(&balance_key).unwrap_or(0);

        // Check sufficient balance
        if current_balance < amount {
            panic_with_error!(&env, ContractError::InsufficientBalance);
        }

        // Deduct from player's balance
        let new_balance = current_balance - amount;
        env.storage().instance().set(&balance_key, &new_balance);

        // Update total supply
        let total_supply: i128 = env
            .storage()
            .instance()
            .get(&DataKey::TotalSupply)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&DataKey::TotalSupply, &(total_supply - amount));

        // Create match escrow
        let escrow_data = MatchEscrowData {
            player: player.clone(),
            amount,
        };
        env.storage().instance().set(&escrow_key, &escrow_data);

        // Emit event
        env.events().publish(
            (
                soroban_sdk::symbol_short!("dep_mt"),
                match_id,
                player.clone(),
            ),
            amount,
        );
    }

    // ============================================
    // ADMIN RESCUE FUNCTIONS (WORKS EVEN WHEN PAUSED)
    // ============================================

    /// Emergency rescue function - Admin can rescue funds even when paused
    /// This is crucial for recovering from critical bugs
    pub fn emergency_rescue(env: Env, admin: Address, to: Address, amount: i128) {
        admin.require_auth();

        // Check if caller is admin
        Self::check_admin(&env, &admin);

        // NOTE: No pause check here! This works even when paused.

        if amount <= 0 {
            panic_with_error!(&env, ContractError::InvalidAmount);
        }

        // Get current balance
        let balance_key = DataKey::Balance(to.clone());
        let current_balance: i128 = env.storage().instance().get(&balance_key).unwrap_or(0);

        // Update balance
        let new_balance = current_balance + amount;
        env.storage().instance().set(&balance_key, &new_balance);

        // Emit event
        env.events()
            .publish((soroban_sdk::symbol_short!("rescue"), to.clone()), amount);
    }

    /// Admin refund match escrow - Manually unlock escrow and return funds
    /// Only callable by admin, works even when paused
    pub fn admin_refund_match(env: Env, admin: Address, match_id: u64) {
        admin.require_auth();

        // Check if caller is admin
        Self::check_admin(&env, &admin);

        // NOTE: No pause check here! This works even when paused.

        // Get match escrow data
        let escrow_key = DataKey::MatchEscrow(match_id);
        let escrow_data: MatchEscrowData = env
            .storage()
            .instance()
            .get(&escrow_key)
            .expect("Match escrow not found");

        let player = escrow_data.player;
        let amount = escrow_data.amount;

        // Remove the escrow data
        env.storage().instance().remove(&escrow_key);

        // Update player's balance
        let balance_key = DataKey::Balance(player.clone());
        let current_balance: i128 = env.storage().instance().get(&balance_key).unwrap_or(0);
        let new_balance = current_balance + amount;
        env.storage().instance().set(&balance_key, &new_balance);

        // Update total supply
        let total_supply: i128 = env
            .storage()
            .instance()
            .get(&DataKey::TotalSupply)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&DataKey::TotalSupply, &(total_supply + amount));

        // Emit event
        env.events().publish(
            (
                soroban_sdk::symbol_short!("refund_mt"),
                match_id,
                player.clone(),
            ),
            amount,
        );
    }

    /// Propose an admin transfer (works even when paused). Does not switch
    /// admin immediately — records a `PendingAdmin` that only takes effect
    /// after `ADMIN_TRANSFER_TIMELOCK_SECS` via [`accept_admin_transfer`],
    /// so a compromised or fat-fingered handoff has a window to be
    /// noticed and cancelled (SC-66). Calling this again while a proposal
    /// is already pending overwrites it with a new candidate and resets
    /// the timelock.
    pub fn transfer_admin(env: Env, current_admin: Address, new_admin: Address) {
        current_admin.require_auth();

        // Check if caller is current admin
        Self::check_admin(&env, &current_admin);

        // NOTE: No pause check - admin transfer should work even when paused

        let effective_at = env.ledger().timestamp() + ADMIN_TRANSFER_TIMELOCK_SECS;
        env.storage().instance().set(
            &DataKey::PendingAdmin,
            &PendingAdmin {
                candidate: new_admin.clone(),
                effective_at,
            },
        );

        // Emit event
        env.events().publish(
            (soroban_sdk::symbol_short!("adminprop"),),
            (new_admin, effective_at),
        );
    }

    /// Finalizes a pending admin transfer once the timelock has elapsed.
    /// Requires the candidate's own auth, so a bad/unreachable address
    /// can't be force-finalized by anyone else (SC-66).
    pub fn accept_admin_transfer(env: Env, candidate: Address) -> Result<(), ContractError> {
        candidate.require_auth();

        let pending: PendingAdmin = env
            .storage()
            .instance()
            .get(&DataKey::PendingAdmin)
            .ok_or(ContractError::NoPendingAdminTransfer)?;

        if pending.candidate != candidate {
            panic_with_error!(&env, ContractError::NotAdmin);
        }
        if env.ledger().timestamp() < pending.effective_at {
            return Err(ContractError::TimelockNotElapsed);
        }

        env.storage().instance().set(&DataKey::Admin, &candidate);
        env.storage().instance().remove(&DataKey::PendingAdmin);

        env.events()
            .publish((soroban_sdk::symbol_short!("adminxfer"),), candidate);
        Ok(())
    }

    /// The current admin cancels a pending admin transfer before it takes
    /// effect (SC-66) — e.g. because `transfer_admin` was called with the
    /// wrong address, or the current admin key recovers control before a
    /// compromised-key handoff finalizes.
    pub fn cancel_admin_transfer(env: Env, current_admin: Address) -> Result<(), ContractError> {
        current_admin.require_auth();
        Self::check_admin(&env, &current_admin);

        if !env.storage().instance().has(&DataKey::PendingAdmin) {
            return Err(ContractError::NoPendingAdminTransfer);
        }
        env.storage().instance().remove(&DataKey::PendingAdmin);

        env.events()
            .publish((soroban_sdk::symbol_short!("admincncl"),), current_admin);
        Ok(())
    }

    /// Returns the pending admin-transfer proposal, if any.
    pub fn get_pending_admin_transfer(env: Env) -> Option<PendingAdmin> {
        env.storage().instance().get(&DataKey::PendingAdmin)
    }

    // ============================================
    // VIEW FUNCTIONS (READ-ONLY, ALWAYS AVAILABLE)
    // ============================================

    /// Get balance of an address
    pub fn balance_of(env: Env, address: Address) -> i128 {
        let balance_key = DataKey::Balance(address);
        env.storage().instance().get(&balance_key).unwrap_or(0)
    }

    /// Get total supply
    pub fn total_supply(env: Env) -> i128 {
        env.storage()
            .instance()
            .get(&DataKey::TotalSupply)
            .unwrap_or(0)
    }

    /// Get admin address
    pub fn get_admin(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Admin not set")
    }

    /// Get match escrow data
    pub fn get_match_escrow(env: Env, match_id: u64) -> Option<MatchEscrowData> {
        let escrow_key = DataKey::MatchEscrow(match_id);
        env.storage().instance().get(&escrow_key)
    }

    /// Get paused status
    pub fn paused(env: Env) -> bool {
        Self::is_paused(&env)
    }

    // ============================================
    // HELPER FUNCTIONS (INTERNAL)
    // ============================================

    /// Check if contract is paused, panic with `ContractError::ContractPaused` if it is.
    /// This is the core circuit breaker check.
    fn check_not_paused(env: &Env) {
        if Self::is_paused(env) {
            panic_with_error!(env, ContractError::ContractPaused);
        }
    }

    /// Check if caller is admin, panic with `ContractError::NotAdmin` if not.
    fn check_admin(env: &Env, caller: &Address) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Admin not set");

        if admin != *caller {
            panic_with_error!(env, ContractError::NotAdmin);
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        Address, Env,
    };

    #[test]
    fn test_initialize() {
        let env = Env::default();
        let contract_id = env.register_contract(None, PausableContract);
        let client = PausableContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.initialize(&admin);

        assert!(!client.paused());
        assert_eq!(client.get_admin(), admin);
    }

    #[test]
    fn test_pause_unpause() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, PausableContract);
        let client = PausableContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.initialize(&admin);

        // Pause
        client.pause(&admin);
        assert!(client.paused());

        // Unpause
        client.unpause(&admin);
        assert!(!client.paused());
    }

    #[test]
    #[should_panic]
    fn test_deposit_when_paused() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, PausableContract);
        let client = PausableContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let user = Address::generate(&env);

        client.initialize(&admin);
        client.pause(&admin);

        // This should return ContractError::ContractPaused
        client.deposit(&user, &1000);
    }

    #[test]
    fn test_deposit_when_not_paused() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, PausableContract);
        let client = PausableContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let user = Address::generate(&env);

        client.initialize(&admin);

        // Deposit should work when not paused
        client.deposit(&user, &1000);
        assert_eq!(client.balance_of(&user), 1000);
    }

    #[test]
    #[should_panic]
    fn test_claim_when_paused() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, PausableContract);
        let client = PausableContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let user = Address::generate(&env);

        client.initialize(&admin);
        client.deposit(&user, &1000);
        client.pause(&admin);

        // This should return ContractError::ContractPaused
        client.claim(&user, &500);
    }

    #[test]
    #[should_panic]
    fn test_mint_when_paused() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, PausableContract);
        let client = PausableContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let user = Address::generate(&env);

        client.initialize(&admin);
        client.pause(&admin);

        // This should return ContractError::ContractPaused
        client.mint(&admin, &user, &1000);
    }

    #[test]
    fn test_emergency_rescue_when_paused() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, PausableContract);
        let client = PausableContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let user = Address::generate(&env);

        client.initialize(&admin);
        client.pause(&admin);

        // Emergency rescue should work even when paused!
        client.emergency_rescue(&admin, &user, &5000);
        assert_eq!(client.balance_of(&user), 5000);
    }

    #[test]
    #[should_panic]
    fn test_pause_non_admin() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, PausableContract);
        let client = PausableContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let non_admin = Address::generate(&env);

        client.initialize(&admin);

        // Non-admin trying to pause should return ContractError::NotAdmin
        client.pause(&non_admin);
    }

    #[test]
    fn test_transfer_admin_when_paused() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, PausableContract);
        let client = PausableContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let new_admin = Address::generate(&env);

        client.initialize(&admin);
        client.pause(&admin);

        // Admin transfer should work even when paused, but only after the timelock
        client.transfer_admin(&admin, &new_admin);
        assert_eq!(client.get_admin(), admin);

        env.ledger().set_timestamp(ADMIN_TRANSFER_TIMELOCK_SECS);
        client.accept_admin_transfer(&new_admin);
        assert_eq!(client.get_admin(), new_admin);
        assert_eq!(client.get_pending_admin_transfer(), None);
    }

    #[test]
    fn test_admin_transfer_waits_for_timelock() {
        let env = Env::default();
        env.mock_all_auths();
        env.ledger().set_timestamp(1_000);

        let contract_id = env.register_contract(None, PausableContract);
        let client = PausableContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let new_admin = Address::generate(&env);
        client.initialize(&admin);

        client.transfer_admin(&admin, &new_admin);
        let effective_at = 1_000 + ADMIN_TRANSFER_TIMELOCK_SECS;
        assert_eq!(
            client.get_pending_admin_transfer(),
            Some(PendingAdmin {
                candidate: new_admin.clone(),
                effective_at,
            })
        );

        env.ledger().set_timestamp(effective_at - 1);
        assert_eq!(
            client.try_accept_admin_transfer(&new_admin),
            Err(Ok(ContractError::TimelockNotElapsed))
        );
        assert_eq!(client.get_admin(), admin);

        env.ledger().set_timestamp(effective_at);
        client.accept_admin_transfer(&new_admin);
        assert_eq!(client.get_admin(), new_admin);
    }

    #[test]
    fn test_admin_transfer_rejects_other_candidate() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, PausableContract);
        let client = PausableContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let new_admin = Address::generate(&env);
        let intruder = Address::generate(&env);
        client.initialize(&admin);

        client.transfer_admin(&admin, &new_admin);
        env.ledger().set_timestamp(ADMIN_TRANSFER_TIMELOCK_SECS);

        assert!(client.try_accept_admin_transfer(&intruder).is_err());
        assert_eq!(client.get_admin(), admin);
    }

    #[test]
    fn test_cancel_admin_transfer() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, PausableContract);
        let client = PausableContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let new_admin = Address::generate(&env);
        client.initialize(&admin);

        assert_eq!(
            client.try_cancel_admin_transfer(&admin),
            Err(Ok(ContractError::NoPendingAdminTransfer))
        );

        client.transfer_admin(&admin, &new_admin);
        client.cancel_admin_transfer(&admin);
        assert_eq!(client.get_pending_admin_transfer(), None);

        env.ledger().set_timestamp(ADMIN_TRANSFER_TIMELOCK_SECS);
        assert_eq!(
            client.try_accept_admin_transfer(&new_admin),
            Err(Ok(ContractError::NoPendingAdminTransfer))
        );
        assert_eq!(client.get_admin(), admin);
    }

    #[test]
    fn test_view_functions_when_paused() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, PausableContract);
        let client = PausableContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let user = Address::generate(&env);

        client.initialize(&admin);
        client.deposit(&user, &1000);
        client.pause(&admin);

        // View functions should work even when paused
        assert_eq!(client.balance_of(&user), 1000);
        assert_eq!(client.total_supply(), 1000);
        assert!(client.paused());
        assert_eq!(client.get_admin(), admin);
    }

    #[test]
    fn test_deposit_match() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, PausableContract);
        let client = PausableContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let player = Address::generate(&env);

        client.initialize(&admin);
        client.mint(&admin, &player, &1000);

        // Deposit into match escrow
        client.deposit_match(&player, &123, &500);

        // Check balances
        assert_eq!(client.balance_of(&player), 500);
        assert_eq!(client.total_supply(), 500);

        // Check escrow data
        let escrow = client.get_match_escrow(&123).unwrap();
        assert_eq!(escrow.player, player);
        assert_eq!(escrow.amount, 500);
    }

    #[test]
    fn test_admin_refund_match() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, PausableContract);
        let client = PausableContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let player = Address::generate(&env);

        client.initialize(&admin);
        client.mint(&admin, &player, &1000);
        client.deposit_match(&player, &123, &500);

        // Admin refund match
        client.admin_refund_match(&admin, &123);

        // Check balances - player should have funds back
        assert_eq!(client.balance_of(&player), 1000);
        assert_eq!(client.total_supply(), 1000);

        // Check escrow data is removed
        assert!(client.get_match_escrow(&123).is_none());
    }

    #[test]
    #[should_panic]
    fn test_admin_refund_match_non_admin() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, PausableContract);
        let client = PausableContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let non_admin = Address::generate(&env);
        let player = Address::generate(&env);

        client.initialize(&admin);
        client.mint(&admin, &player, &1000);
        client.deposit_match(&player, &123, &500);

        // Non-admin trying to refund should return ContractError::NotAdmin
        client.admin_refund_match(&non_admin, &123);
    }

    #[test]
    #[should_panic(expected = "Match escrow not found")]
    fn test_admin_refund_match_nonexistent() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, PausableContract);
        let client = PausableContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);

        client.initialize(&admin);

        // Trying to refund non-existent match should fail
        client.admin_refund_match(&admin, &999);
    }
}
