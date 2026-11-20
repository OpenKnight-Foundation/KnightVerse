#![no_std]
//! AI-41: On-Chain Model Checkpoint Hash Attestation
//!
//! Stores SHA-256 / Blake3 hashes of AI model weights in a Soroban registry
//! so tournament organizers can prove bots used identical, untampered models
//! throughout a tournament.

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, panic_with_error, Address, Bytes, Env,
    String,
};

#[contracttype]
pub enum DataKey {
    Admin,
    /// model_id → ModelRecord
    Model(String),
    /// addresses allowed to attest on behalf of the organiser
    Attester(Address),
}

/// Algorithm used to produce the stored checkpoint hash (#1557).
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HashAlgorithm {
    Sha256,
    Blake3,
}

/// A stored model checkpoint record.
#[contracttype]
#[derive(Clone, Debug)]
pub struct ModelRecord {
    /// Canonical lower-case hex SHA-256 or Blake3 hash of the model weights
    pub hash: String,
    /// Algorithm that produced `hash`
    pub algorithm: HashAlgorithm,
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
    /// Submitter is neither the admin nor an approved attester
    NotAttester = 5,
    /// Hash is not a 32-byte hex digest
    InvalidHash = 6,
}

/// Reject malformed digests and store one canonical encoding (#1557).
fn normalize_hash(env: &Env, hash: &String) -> String {
    let bytes = hash.to_bytes();
    if bytes.len() != 64 {
        panic_with_error!(env, Error::InvalidHash);
    }
    let mut out = Bytes::new(env);
    for i in 0..bytes.len() {
        let mut b = bytes.get(i).unwrap();
        if b >= b'A' && b <= b'F' {
            b += b'a' - b'A';
        } else if !((b >= b'0' && b <= b'9') || (b >= b'a' && b <= b'f')) {
            panic_with_error!(env, Error::InvalidHash);
        }
        out.push(b);
    }
    String::from_bytes(env, &out)
}

fn require_admin(env: &Env, caller: &Address) {
    caller.require_auth();
    let admin: Address = env
        .storage()
        .instance()
        .get(&DataKey::Admin)
        .unwrap_or_else(|| panic_with_error!(env, Error::NotAdmin));
    if caller != &admin {
        panic_with_error!(env, Error::NotAdmin);
    }
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

    /// Approve or revoke an attester (#1556).
    pub fn set_attester(env: Env, admin: Address, attester: Address, approved: bool) {
        require_admin(&env, &admin);
        env.storage()
            .instance()
            .set(&DataKey::Attester(attester), &approved);
    }

    /// Submit a model weight hash for a given model_id. Immutable once stored.
    pub fn attest(
        env: Env,
        submitter: Address,
        model_id: String,
        hash: String,
        tournament_id: String,
        algorithm: HashAlgorithm,
    ) {
        submitter.require_auth();
        // #1556: immutable records must not be squat-able by arbitrary callers.
        let is_admin: bool = env
            .storage()
            .instance()
            .get::<_, Address>(&DataKey::Admin)
            .map(|admin| admin == submitter)
            .unwrap_or(false);
        let approved: bool = env
            .storage()
            .instance()
            .get(&DataKey::Attester(submitter.clone()))
            .unwrap_or(false);
        if !is_admin && !approved {
            panic_with_error!(&env, Error::NotAttester);
        }

        let hash = normalize_hash(&env, &hash);
        let key = DataKey::Model(model_id.clone());
        if env.storage().persistent().has(&key) {
            panic_with_error!(&env, Error::AlreadyAttested);
        }
        let record = ModelRecord {
            hash: hash.clone(),
            algorithm,
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
        match env
            .storage()
            .persistent()
            .get::<_, ModelRecord>(&DataKey::Model(model_id))
        {
            Some(record) => record.hash == normalize_hash(&env, &hash),
            None => false,
        }
    }
}
