#![no_std]
//! SC-54: Decentralized Referral & Affiliate Commission Splitter
//!
//! Records permanent on-chain referee→referrer bindings and automatically
//! splits a configurable fee percentage to the referrer on every wager.
//! Self-referral loops are rejected at registration time.

use soroban_sdk::{
    contract, contractimpl, contracttype, contracterror, panic_with_error,
    Address, Env, Vec,
};

/// Fee denominator: commission_bps / 10_000 = commission fraction.
const FEE_DENOMINATOR: i128 = 10_000;

#[contracttype]
pub enum DataKey {
    /// admin address
    Admin,
    /// referrer for a given referee address
    Referrer(Address),
    /// cumulative earnings for a referrer
    Earnings(Address),
    /// configurable commission in basis points (e.g. 1000 = 10%)
    CommissionBps,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Error {
    AlreadyInitialized = 1,
    NotAdmin = 2,
    /// Referee already has a referrer registered
    AlreadyReferred = 3,
    /// Self-referral is forbidden
    SelfReferral = 4,
    InvalidAmount = 5,
    InvalidCommission = 6,
}

#[contract]
pub struct ReferralSplitter;

#[contractimpl]
impl ReferralSplitter {
    /// Initialise the contract; sets admin and commission in basis points.
    pub fn initialize(env: Env, admin: Address, commission_bps: u32) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic_with_error!(&env, Error::AlreadyInitialized);
        }
        if commission_bps > 10_000 {
            panic_with_error!(&env, Error::InvalidCommission);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::CommissionBps, &commission_bps);
    }

    /// Register a referee→referrer binding. Permanent once set; self-referral rejected.
    pub fn register_referral(env: Env, referee: Address, referrer: Address) {
        referee.require_auth();
        if referee == referrer {
            panic_with_error!(&env, Error::SelfReferral);
        }
        let key = DataKey::Referrer(referee.clone());
        if env.storage().persistent().has(&key) {
            panic_with_error!(&env, Error::AlreadyReferred);
        }
        env.storage().persistent().set(&key, &referrer);
    }

    /// Settle a wager of `amount` stroops. Splits commission to the referrer
    /// (if one exists) and returns the referrer's cut. Emits a referral_earnings event.
    pub fn settle_wager(env: Env, referee: Address, amount: i128) -> i128 {
        if amount <= 0 {
            panic_with_error!(&env, Error::InvalidAmount);
        }
        let referrer: Option<Address> = env
            .storage()
            .persistent()
            .get(&DataKey::Referrer(referee));
        if let Some(ref r) = referrer {
            let bps: u32 = env
                .storage()
                .instance()
                .get(&DataKey::CommissionBps)
                .unwrap_or(1_000);
            let cut = amount * (bps as i128) / FEE_DENOMINATOR;
            if cut > 0 {
                let prev: i128 = env
                    .storage()
                    .persistent()
                    .get(&DataKey::Earnings(r.clone()))
                    .unwrap_or(0);
                env.storage()
                    .persistent()
                    .set(&DataKey::Earnings(r.clone()), &(prev + cut));
                env.events().publish(
                    (soroban_sdk::symbol_short!("ref_earn"),),
                    (r.clone(), cut),
                );
                return cut;
            }
        }
        0
    }

    /// Returns cumulative earnings for a referrer.
    pub fn get_earnings(env: Env, referrer: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::Earnings(referrer))
            .unwrap_or(0)
    }

    /// Returns the referrer registered for a referee, if any.
    pub fn get_referrer(env: Env, referee: Address) -> Option<Address> {
        env.storage()
            .persistent()
            .get(&DataKey::Referrer(referee))
    }
}
