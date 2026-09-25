#![no_std]
//! SC-54: Decentralized Referral & Affiliate Commission Splitter
//!
//! Records permanent on-chain referee→referrer bindings and automatically
//! splits a configurable fee percentage to the referrer on every wager.
//! Self-referral loops are rejected at registration time.
//!
//! Commissions are accrued by [`ReferralSplitter::settle_wager`] and paid out
//! on-chain by [`ReferralSplitter::withdraw_earnings`], which performs a real
//! `token::Client` transfer of the referrer's full balance and zeroes it.

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, panic_with_error, symbol_short, token,
    Address, Env,
};

/// Fee denominator: commission_bps / 10_000 = commission fraction.
const FEE_DENOMINATOR: i128 = 10_000;

#[contracttype]
pub enum DataKey {
    /// admin address
    Admin,
    /// referrer for a given referee address
    Referrer(Address),
    /// withdrawable commission balance for a referrer
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
    /// (if one exists) and returns the referrer's cut, which is credited to
    /// their withdrawable balance. Emits a referral_earnings event.
    ///
    /// This is accounting only: the accrued balance is moved on-chain by
    /// [`ReferralSplitter::withdraw_earnings`].
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
                    (symbol_short!("ref_earn"),),
                    (r.clone(), cut),
                );
                return cut;
            }
        }
        0
    }

    /// Withdraw a referrer's entire accrued commission balance in `token`.
    ///
    /// Requires the referrer's authorisation and transfers the full
    /// [`ReferralSplitter::get_earnings`] balance from this contract to the
    /// referrer using a real `token::Client` transfer. The balance is zeroed
    /// *before* the transfer (checks-effects-interactions) so a repeated or
    /// re-entrant call cannot double-pay.
    ///
    /// Withdrawing with nothing accrued is a no-op that returns `0` — it does
    /// not panic and does not move any funds. Returns the amount paid out.
    pub fn withdraw_earnings(env: Env, referrer: Address, token: Address) -> i128 {
        referrer.require_auth();

        let key = DataKey::Earnings(referrer.clone());
        let balance: i128 = env.storage().persistent().get(&key).unwrap_or(0);
        if balance <= 0 {
            return 0;
        }

        // Effects before interactions: zero the balance so the external call
        // can never be replayed into a second payout.
        env.storage().persistent().set(&key, &0i128);

        token::Client::new(&env, &token).transfer(
            &env.current_contract_address(),
            &referrer,
            &balance,
        );

        env.events().publish(
            (symbol_short!("ref_wd"),),
            (referrer, token, balance),
        );

        balance
    }

    /// Returns the withdrawable commission balance for a referrer.
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

    /// Returns the configured commission in basis points.
    pub fn get_commission_bps(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::CommissionBps)
            .unwrap_or(1_000)
    }
}

#[cfg(test)]
mod test;
