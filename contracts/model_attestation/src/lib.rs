#![no_std]
//! AI-41: On-Chain Model Checkpoint Hash Attestation
//!
//! Stores SHA-256 / Blake3 hashes of AI model weights in a Soroban registry
//! so tournament organizers can prove bots used identical, untampered models
//! throughout a tournament.

use soroban_sdk::{
    contract, contractimpl, contracttype, contracterror, panic_with_error,
    Address, Bytes, Env, String,
};

#[contracttype]
pub enum DataKey {
    Admin,
    /// model_id → ModelRecord
    Model(String),
}

/// A stored model checkpoint record.
#[contracttype]
#[derive(Clone, Debug)]
pub struct ModelRecord {
    /// Hex-encoded SHA-256 or Blake3 hash of the model weights
    pub hash: String,
    /// Tournament or version identifier
    pub tournament_id: String,
    /// Ledger timestamp at submission
    pub submitted_at: u64,
    /// Address that submitted this record
    pub submitted_by: Address,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Error {
    AlreadyInitialized = 1,
    NotAdmin = 2,
    /// Model ID already attested — records are immutable
    AlreadyAttested = 3,
    ModelNotFound = 4,
}

#[contract]
pub struct ModelAttestation;

#[contractimpl]
impl ModelAttestation {
    /// Initialise the registry with an admin address.
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic_with_error!(&env, Error::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
    }

    /// Submit a model weight hash for a given model_id. Immutable once stored.
    pub fn attest(
        env: Env,
        submitter: Address,
        model_id: String,
        hash: String,
        tournament_id: String,
    ) {
        submitter.require_auth();
        let key = DataKey::Model(model_id.clone());
        if env.storage().persistent().has(&key) {
            panic_with_error!(&env, Error::AlreadyAttested);
        }
        let record = ModelRecord {
            hash: hash.clone(),
            tournament_id: tournament_id.clone(),
            submitted_at: env.ledger().timestamp(),
            submitted_by: submitter,
        };
        env.storage().persistent().set(&key, &record);
        env.events().publish(
            (soroban_sdk::symbol_short!("attested"),),
            (model_id, hash, tournament_id),
        );
    }

    /// Look up a stored model record by model_id.
    pub fn get_record(env: Env, model_id: String) -> ModelRecord {
        env.storage()
            .persistent()
            .get(&DataKey::Model(model_id))
            .unwrap_or_else(|| panic_with_error!(&env, Error::ModelNotFound))
    }

    /// Verify a hash matches the stored attestation. Returns true if it matches.
    pub fn verify(env: Env, model_id: String, hash: String) -> bool {
        match env.storage().persistent().get::<_, ModelRecord>(&DataKey::Model(model_id)) {
            Some(record) => record.hash == hash,
            None => false,
        }
    }
}
