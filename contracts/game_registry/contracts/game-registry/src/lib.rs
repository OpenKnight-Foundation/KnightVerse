#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, contracterror, panic_with_error, symbol_short, Address, Env, String, Symbol, token};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum RegistryError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    GameAlreadyRecorded = 3,
    GameNotFound = 4,
    TournamentFull = 5,
    AlreadyRegistered = 6,
    InsufficientEntryFee = 7,
    TournamentNotFound = 8,
    TournamentAlreadyExists = 9,
    /// Contract is paused for emergency halt (SC-11)
    ContractPaused = 10,
    /// Caller is not the contract admin
    NotAdmin = 11,
    /// Contract is already paused
    AlreadyPaused = 12,
    /// Contract is not currently paused
    NotPaused = 13,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GameResult {
    pub winner: Address,
    pub white: Address,
    pub black: Address,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Tournament {
    pub id: String,
    pub capacity: u32,
    pub entry_fee: i128,
    pub token_address: Address,
    pub participants: u32,
}

#[contracttype]
pub enum DataKey {
    Admin,
    Server,
    Game(String),
    Tournament(String),
    Registration(String, Address),
    Paused,
}

#[contract]
pub struct GameRegistry;

#[contractimpl]
impl GameRegistry {
    /// Initialize the contract with an admin and an authorized server address.
    pub fn initialize(env: Env, admin: Address, server: Address) -> Result<(), RegistryError> {
        if env.storage().persistent().has(&DataKey::Admin) {
            return Err(RegistryError::AlreadyInitialized);
        }
        env.storage().persistent().set(&DataKey::Admin, &admin);
        env.storage().persistent().set(&DataKey::Server, &server);

        // Extend TTL for Admin and Server keys to prevent expiration
        env.storage().persistent().extend_ttl(&DataKey::Admin, 100_000, 500_000);
        env.storage().persistent().extend_ttl(&DataKey::Server, 100_000, 500_000);
        Ok(())
    }

    // ── Pausable extension (SC-11) ────────────────────────────────────────────

    /// Pause the contract — blocks all state-mutating operations.
    /// Only the contract admin may call this.
    pub fn pause(env: Env, caller: Address) -> Result<(), RegistryError> {
        let admin: Address = env
            .storage()
            .persistent()
            .get(&DataKey::Admin)
            .ok_or(RegistryError::NotInitialized)?;
        caller.require_auth();
        if caller != admin {
            return Err(RegistryError::NotAdmin);
        }
        if env.storage().instance().get(&DataKey::Paused).unwrap_or(false) {
            return Err(RegistryError::AlreadyPaused);
        }
        env.storage().instance().set(&DataKey::Paused, &true);
        env.events()
            .publish((symbol_short!("paused"),), caller);
        Ok(())
    }

    /// Unpause the contract — resumes normal operations.
    /// Only the contract admin may call this.
    pub fn unpause(env: Env, caller: Address) -> Result<(), RegistryError> {
        let admin: Address = env
            .storage()
            .persistent()
            .get(&DataKey::Admin)
            .ok_or(RegistryError::NotInitialized)?;
        caller.require_auth();
        if caller != admin {
            return Err(RegistryError::NotAdmin);
        }
        if !env.storage().instance().get(&DataKey::Paused).unwrap_or(false) {
            return Err(RegistryError::NotPaused);
        }
        env.storage().instance().set(&DataKey::Paused, &false);
        env.events()
            .publish((symbol_short!("unpaused"),), caller);
        Ok(())
    }

    /// Returns `true` if the contract is currently paused.
    pub fn is_paused(env: Env) -> bool {
        env.storage().instance().get(&DataKey::Paused).unwrap_or(false)
    }

    /// Internal helper — returns `ContractError::ContractPaused` when the contract is paused.
    fn check_not_paused(env: &Env) {
        if env.storage().instance().get(&DataKey::Paused).unwrap_or(false) {
            panic_with_error!(env, RegistryError::ContractPaused);
        }
    }

    /// Records a game result. Only the authorized server can call this.
    pub fn record_game(
        env: Env,
        game_id: String,
        winner: Address,
        white: Address,
        black: Address,
        timestamp: u64,
    ) -> Result<(), RegistryError> {
        Self::check_not_paused(&env);
        let server: Address = env.storage().persistent().get(&DataKey::Server).ok_or(RegistryError::NotInitialized)?;
        server.require_auth();

        if env.storage().persistent().has(&DataKey::Game(game_id.clone())) {
            return Err(RegistryError::GameAlreadyRecorded);
        }

        let result = GameResult {
            winner: winner.clone(),
            white,
            black,
            timestamp,
        };

        let key = DataKey::Game(game_id.clone());
        env.storage().persistent().set(&key, &result);
        
        // Extend TTL for the game record to ensure it stays active.
        env.storage().persistent().extend_ttl(&key, 100_000, 500_000);

        // Emit GameFinalized event
        env.events().publish(
            (Symbol::new(&env, "GameFinalized"), game_id),
            (winner, timestamp),
        );
        Ok(())
    }

    /// Retrieves a recorded game result.
    pub fn get_game(env: Env, game_id: String) -> Result<GameResult, RegistryError> {
        env.storage()
            .persistent()
            .get(&DataKey::Game(game_id))
            .ok_or(RegistryError::GameNotFound)
    }

    /// Updates the authorized server address. Only the admin can call this.
    pub fn set_server(env: Env, new_server: Address) -> Result<(), RegistryError> {
        Self::check_not_paused(&env);
        let admin: Address = env.storage().persistent().get(&DataKey::Admin).ok_or(RegistryError::NotInitialized)?;
        admin.require_auth();
        env.storage().persistent().set(&DataKey::Server, &new_server);
        env.storage().persistent().extend_ttl(&DataKey::Server, 100_000, 500_000);
        Ok(())
    }

    /// Updates the admin address. Only the current admin can call this.
    pub fn set_admin(env: Env, new_admin: Address) -> Result<(), RegistryError> {
        Self::check_not_paused(&env);
        let admin: Address = env.storage().persistent().get(&DataKey::Admin).ok_or(RegistryError::NotInitialized)?;
        admin.require_auth();
        env.storage().persistent().set(&DataKey::Admin, &new_admin);
        env.storage().persistent().extend_ttl(&DataKey::Admin, 100_000, 500_000);
        Ok(())
    }

    /// Creates a new tournament. Only admin can call this.
    pub fn create_tournament(
        env: Env, 
        id: String, 
        capacity: u32, 
        entry_fee: i128, 
        token_address: Address
    ) -> Result<(), RegistryError> {
        Self::check_not_paused(&env);
        let admin: Address = env.storage().persistent().get(&DataKey::Admin).ok_or(RegistryError::NotInitialized)?;
        admin.require_auth();

        let t_key = DataKey::Tournament(id.clone());
        if env.storage().persistent().has(&t_key) {
            return Err(RegistryError::TournamentAlreadyExists);
        }

        let tournament = Tournament {
            id,
            capacity,
            entry_fee,
            token_address,
            participants: 0,
        };

        env.storage().persistent().set(&t_key, &tournament);
        env.storage().persistent().extend_ttl(&t_key, 100_000, 500_000);
        Ok(())
    }

    /// Registers a player for a tournament.
    pub fn register_tournament(
        env: Env, 
        player: Address, 
        tournament_id: String
    ) -> Result<(), RegistryError> {
        Self::check_not_paused(&env);
        player.require_auth();

        let t_key = DataKey::Tournament(tournament_id.clone());
        let mut tournament: Tournament = env.storage().persistent().get(&t_key).ok_or(RegistryError::TournamentNotFound)?;

        if tournament.participants >= tournament.capacity {
            return Err(RegistryError::TournamentFull);
        }

        let reg_key = DataKey::Registration(tournament_id.clone(), player.clone());
        if env.storage().persistent().has(&reg_key) {
            return Err(RegistryError::AlreadyRegistered);
        }

        if tournament.entry_fee > 0 {
            let token_client = token::Client::new(&env, &tournament.token_address);
            if token_client.balance(&player) < tournament.entry_fee {
                return Err(RegistryError::InsufficientEntryFee);
            }

            let admin: Address = env.storage().persistent().get(&DataKey::Admin).ok_or(RegistryError::NotInitialized)?;
            token_client.transfer(&player, &admin, &tournament.entry_fee);
        }

        tournament.participants += 1;
        env.storage().persistent().set(&t_key, &tournament);
        env.storage().persistent().set(&reg_key, &true);
        
        env.storage().persistent().extend_ttl(&t_key, 100_000, 500_000);
        env.storage().persistent().extend_ttl(&reg_key, 100_000, 500_000);

        Ok(())
    }
}

mod test;
