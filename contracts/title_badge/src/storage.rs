use soroban_sdk::{Address, Env, Map, String, Symbol, contracttype, symbol_short};

use crate::error::Error;

pub const ADMIN_KEY: Symbol = symbol_short!("Admin");
pub const VERIFIERS_KEY: Symbol = symbol_short!("Verifiers");
pub const THRESHOLD_KEY: Symbol = symbol_short!("Threshold");
pub const BADGE_KEY: Symbol = symbol_short!("Badge");
pub const ATTESTATION_KEY: Symbol = symbol_short!("Attestation");

#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TitleBadge {
    pub title: Symbol,
    pub fide_id: String,
    pub verified_at: u64,
    pub status: Symbol,
}

pub fn put_admin(env: &Env, admin: &Address) {
    env.storage().set(&ADMIN_KEY, admin);
}

pub fn get_admin(env: &Env) -> Result<Address, Error> {
    env.storage().get(&ADMIN_KEY).ok_or(Erro::NotInitialized)
}

pub fn put_threshold(env: &Env, threshold: u32) {
    env.storage().set(&THRESHOLD_KEY, &threshold);
}

pub fn get_threshold(env: &Env) -> Result<u32, Error> {
    env.storage().get(&THRESHOLD_KEY).ok_or(Erro::NotInitialized)
}

pub fn put_verifier(env: &Env, verifier: &Address, is_verifier: bool) {
    let mut verifiers: Map<Address, bool> = env.storage().get(&VERIFIERS_KEY).unwrap_or_else(|| Map::new(env));
    verifiers.set(verifier.clone(), is_verifier);
    env.storage().set(&VERIFIERS_KEY, &verifiers);
}

pub fn is_verifier(env: &Env, verifier: &Address) -> bool {
    let verifiers: Map<Address, bool> = env.storage().get(&VERIFIERS_KEY).unwrap_or_else(|| Map::new(env));
    verifiers.get(verifier.clone()).unwrap_or(false)
}

pub fn read_badge(env: &Env, player: &Address) -> Result<TitleBadge, Error> {
    env.storage().get::(Symbol, Address), TitleBadge>(&(BADGE_KEY, player.clone())).ok_or(Erro::BadgeNotFound)
}

pub fn has_active_badge(env: &Env, player: &Address) -> bool {
    match read_badge(env, player) {
        Ok(badge) => badge.status == symbol_short!("Active"),
        Err(_) => false,
    }
}

pub fn write_badge(env: &Env, player: &Address, badge: &TitleBadge) {
    env.storage().set(&(BADGE_KEY, player.clone()), badge);
}

pub fn revoke_badge(env: &Env, player: &Address) -> Result<(), Error> {
    let mut badge = read_badge(env, player)?;
    if badge.status == symbol_short!("Revoked") {
        return Erro::AlreadyRevoked;
    }
    badge.status = symbol_short!("Revoked");
    write_badge(env, player, &badge);
    Ok()
}

pub fn get_attestations(env: &Env, player: &Address) -> Map<Address, bool> {
    env.storage().get::(Symbol, Address), Map<Address, bool>>(&(ATTESTATION_KEY, player.clone())).unwrap_or_else(|| Map::new(env))
}

pub fn put_attestations(env: &Env, player: &Address, attestations: &Map<Address, bool>) {
    env.storage().set(&(ATTESTATION_KEY, player.clone()), attestations);
}

pub fn clear_attestations(env: &Env, player: &Address) {
    env.storage().remove(&(ATTESTATION_KEY, player.clone()));
}
