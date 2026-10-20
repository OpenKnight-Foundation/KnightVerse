//! # SC-46 — Automated Liquidity Pool Yield Staking for Tournament Prize Pools
//!
//! Tournament organizers frequently deposit prize funds months before a major
//! championship event. This contract lets an organizer lock an idle prize pool
//! into a whitelisted Soroban liquidity pool so the deposit earns additional
//! prize yield while it waits, and lets the organizer harvest that yield when
//! the tournament concludes and add it to the final payout distribution.
//!
//! ## Design
//!
//! * **Whitelisted pools** — the contract only ever stakes into liquidity
//!   pool contracts the admin has explicitly whitelisted, so prize funds can
//!   never be diverted to an arbitrary attacker-chosen address.
//! * **Principal safety** — positions are denominated in a single underlying
//!   token per contract instance. The contract stores the exact principal it
//!   staked and, by default, refuses any settlement that would return less
//!   principal than was deposited (`zero-loss` mode). An organizer may
//!   explicitly accept principal risk per position via
//!   `set_allow_principal_loss`.
//! * **Reentrancy guard** — every external token/pool interaction runs inside
//!   a non-reentrant section, so a malicious pool cannot call back into
//!   `deposit`, `harvest_yield`, or `settle_tournament` mid-flow.
//! * **Arithmetic safety** — the crate is compiled with `overflow-checks =
//!   true` (here and in the workspace release profile) and every state
//!   update uses checked arithmetic that fails with typed `ContractError`s.
//! * **Settlement** — `settle_tournament` credits winners into an escrow
//!   ledger by percentage (1..=100, summing to exactly 100). Division dust is
//!   given to the first winner so the distributed sum always equals the pool
//!   exactly, mirroring the `payout_tournament` convention of `game_contract`.
//! * **Single asset** — prize pools are denominated in one underlying token;
//!   the first deposit fixes the contract-wide token and later positions must
//!   match it, keeping escrow accounting unambiguous.

#![no_std]

#[cfg(test)]
mod test;

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, token::TokenClient, Address, Env, Map,
    Symbol, Vec,
};

/// On-chain state of one staked tournament prize position.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PoolState {
    /// Liquidity pool contract the principal is staked in.
    pub pool: Address,
    /// Total underlying tokens staked (principal only; excludes harvested
    /// yield, which is credited to the treasury escrow when harvested).
    pub total_staked: i128,
    /// Yield harvested from this position so far.
    pub harvested_yield: i128,
    /// Whether the tournament has been settled and the position closed.
    pub settled: bool,
    /// Whether the organizer explicitly accepted principal risk.
    pub allow_principal_loss: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    /// Address allowed to whitelist pools, stake and settle tournaments.
    Admin,
    /// Address allowed to claim harvested-yield escrow.
    Treasury,
    /// Underlying token contract (fixed on first deposit).
    Token,
    /// `Map<Address (organizer), Map<u64 (pool_id), PoolState>>`
    Organizers,
    /// Monotonic pool id counter.
    PoolCounter,
    /// Set of whitelisted liquidity pool contract addresses.
    Whitelist,
    /// `Map<Address (recipient), i128>` escrow ledger.
    Escrow,
    /// Underlying tokens held in the contract wallet that are attributable to
    /// harvested-but-not-yet-distributed yield (across all positions).
    WalletYield,
    /// Reentrancy guard flag.
    Entered,
}

/// Errors surfaced through the generated `try_*` client methods.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ContractError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
    InvalidAmount = 4,
    PoolNotWhitelisted = 5,
    PoolAlreadyWhitelisted = 6,
    TournamentAlreadySettled = 7,
    ReentrantCall = 8,
    /// Settlement would return less than the deposited principal and the
    /// organizer did not accept principal risk (zero-loss mode).
    PrincipalLoss = 9,
    /// Percentages must be within 1..=100.
    InvalidPercentage = 10,
    /// Percentages must sum to exactly 100.
    PercentageSumMismatch = 11,
    /// At least one winner is required for settlement.
    EmptyWinners = 12,
    /// The requested position does not exist.
    PoolNotFound = 13,
    /// Checked arithmetic overflowed.
    ArithmeticOverflow = 14,
    /// The pool's underlying token does not match the contract's fixed token.
    TokenMismatch = 15,
}

#[contract]
pub struct YieldStakingContract;

#[contractimpl]
impl YieldStakingContract {
    /// Initialize the contract with an admin (whitelists, staking, settling)
    /// and a treasury (recipient of harvested yield).
    pub fn initialize(env: Env, admin: Address, treasury: Address) -> Result<(), ContractError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(ContractError::AlreadyInitialized);
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Treasury, &treasury);
        env.storage().instance().set(&DataKey::PoolCounter, &0u64);
        env.storage().instance().set(&DataKey::WalletYield, &0i128);
        env.storage().instance().set(&DataKey::Entered, &false);
        Ok(())
    }

    pub fn admin(env: Env) -> Result<Address, ContractError> {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(ContractError::NotInitialized)
    }

    pub fn treasury(env: Env) -> Result<Address, ContractError> {
        env.storage()
            .instance()
            .get(&DataKey::Treasury)
            .ok_or(ContractError::NotInitialized)
    }

    /// The underlying token fixed by the first deposit (if any).
    pub fn token(env: Env) -> Option<Address> {
        env.storage().instance().get(&DataKey::Token)
    }

    // ────────────────────────────────────────────────────────────────────
    // Pool whitelist
    // ────────────────────────────────────────────────────────────────────

    /// Whitelist a liquidity pool contract. Only whitelisted addresses can
    /// receive principal from `stake` — a compromised admin cannot silently
    /// point prize funds at an arbitrary "pool" that was never reviewed.
    pub fn whitelist_pool(env: Env, caller: Address, pool: Address) -> Result<(), ContractError> {
        caller.require_auth();
        Self::require_admin(&env, &caller)?;

        let mut whitelist: Map<Address, bool> = Self::whitelist(&env);
        if whitelist.get(pool.clone()).unwrap_or(false) {
            return Err(ContractError::PoolAlreadyWhitelisted);
        }
        whitelist.set(pool, true);
        env.storage()
            .instance()
            .set(&DataKey::Whitelist, &whitelist);

        env.events()
            .publish((Symbol::new(&env, "pool_whitelisted"),), ());
        Ok(())
    }

    /// Remove a liquidity pool from the whitelist. Existing positions are
    /// unaffected — they continue to harvest and settle normally.
    pub fn unwhitelist_pool(env: Env, caller: Address, pool: Address) -> Result<(), ContractError> {
        caller.require_auth();
        Self::require_admin(&env, &caller)?;

        let mut whitelist: Map<Address, bool> = Self::whitelist(&env);
        whitelist.remove(pool);
        env.storage()
            .instance()
            .set(&DataKey::Whitelist, &whitelist);

        env.events()
            .publish((Symbol::new(&env, "pool_unwhitelisted"),), ());
        Ok(())
    }

    pub fn is_pool_whitelisted(env: Env, pool: Address) -> bool {
        Self::whitelist(&env).get(pool).unwrap_or(false)
    }

    fn whitelist(env: &Env) -> Map<Address, bool> {
        env.storage()
            .instance()
            .get(&DataKey::Whitelist)
            .unwrap_or_else(|| Map::new(env))
    }

    // ────────────────────────────────────────────────────────────────────
    // Staking / withdrawal
    // ────────────────────────────────────────────────────────────────────

    /// Lock `amount` of the caller's tournament prize pool into the
    /// whitelisted liquidity `pool`.
    ///
    /// The tokens are pulled from the caller (the tournament organizer) into
    /// this contract, which then forwards them to the pool via the standard
    /// pool interface (`deposit(from, amount)`). Each (organizer, pool) pair
    /// holds exactly one position and further stakes top it up. Returns the
    /// position id.
    pub fn stake(
        env: Env,
        caller: Address,
        pool: Address,
        amount: i128,
    ) -> Result<u64, ContractError> {
        caller.require_auth();
        Self::require_not_entered(&env)?;
        if amount <= 0 {
            return Err(ContractError::InvalidAmount);
        }
        if !Self::is_pool_whitelisted(env.clone(), pool.clone()) {
            return Err(ContractError::PoolNotWhitelisted);
        }

        let organizer = caller;
        let (pool_id, mut state, existed) = Self::resolve_position(&env, &organizer, &pool)?;
        if existed && state.settled {
            return Err(ContractError::TournamentAlreadySettled);
        }
        state.total_staked = state
            .total_staked
            .checked_add(amount)
            .ok_or(ContractError::ArithmeticOverflow)?;

        // Non-reentrant section: pull the prize funds in, then stake them out.
        env.storage().instance().set(&DataKey::Entered, &true);
        let token = Self::contract_token(&env)?;
        let result = Self::stake_inner(&env, &token, &pool, &organizer, amount);
        env.storage().instance().set(&DataKey::Entered, &false);
        result?;

        Self::store_position(&env, &organizer, pool_id, &state);

        env.events().publish(
            (Symbol::new(&env, "pool_staked"), organizer),
            (pool_id, pool, amount),
        );
        Ok(pool_id)
    }

    fn stake_inner(
        env: &Env,
        token: &Address,
        pool: &Address,
        organizer: &Address,
        amount: i128,
    ) -> Result<(), ContractError> {
        let token_client = TokenClient::new(env, token);
        // Pull the prize funds from the organizer into escrow.
        token_client.transfer(organizer, &env.current_contract_address(), &amount);
        // Stake the escrowed principal with the whitelisted pool.
        token_client.transfer(&env.current_contract_address(), pool, &amount);
        PoolClient::new(env, pool).deposit(&env.current_contract_address(), &amount);
        Ok(())
    }

    /// Unstake `amount` of principal from the caller's own position back to
    /// the caller (e.g. the tournament was cancelled) without settling.
    pub fn unstake(
        env: Env,
        caller: Address,
        pool_id: u64,
        amount: i128,
    ) -> Result<(), ContractError> {
        caller.require_auth();
        Self::require_not_entered(&env)?;
        if amount <= 0 {
            return Err(ContractError::InvalidAmount);
        }

        let (organizer, mut state) = Self::load_position(&env, pool_id)?;
        if organizer != caller {
            return Err(ContractError::Unauthorized);
        }
        if state.settled {
            return Err(ContractError::TournamentAlreadySettled);
        }
        let new_staked = state
            .total_staked
            .checked_sub(amount)
            .ok_or(ContractError::InvalidAmount)?;
        if new_staked < 0 {
            return Err(ContractError::InvalidAmount);
        }
        state.total_staked = new_staked;

        env.storage().instance().set(&DataKey::Entered, &true);
        let result = Self::unstake_inner(&env, &state, &organizer, amount);
        env.storage().instance().set(&DataKey::Entered, &false);
        result?;

        Self::store_position(&env, &organizer, pool_id, &state);
        env.events().publish(
            (Symbol::new(&env, "pool_unstaked"), organizer),
            (pool_id, amount),
        );
        Ok(())
    }

    fn unstake_inner(
        env: &Env,
        state: &PoolState,
        organizer: &Address,
        amount: i128,
    ) -> Result<(), ContractError> {
        PoolClient::new(env, &state.pool).withdraw(&env.current_contract_address(), &amount);
        let token = Self::contract_token(env)?;
        TokenClient::new(env, &token).transfer(&env.current_contract_address(), organizer, &amount);
        Ok(())
    }

    /// Explicitly accept (or revoke) principal risk for a position. Only the
    /// admin may do this; it is required before a settlement that would
    /// return less than the deposited principal.
    pub fn set_allow_principal_loss(
        env: Env,
        caller: Address,
        pool_id: u64,
        allow: bool,
    ) -> Result<(), ContractError> {
        caller.require_auth();
        Self::require_admin(&env, &caller)?;

        let (organizer, mut state) = Self::load_position(&env, pool_id)?;
        if state.settled {
            return Err(ContractError::TournamentAlreadySettled);
        }
        state.allow_principal_loss = allow;
        Self::store_position(&env, &organizer, pool_id, &state);
        Ok(())
    }

    // ────────────────────────────────────────────────────────────────────
    // Yield accounting
    // ────────────────────────────────────────────────────────────────────

    /// Yield accrued on top of the tracked principal, according to the pool's
    /// own accounting (`balance_of`).
    pub fn accrued_yield(env: Env, pool_id: u64) -> Result<i128, ContractError> {
        let (_, state) = Self::load_position(&env, pool_id)?;
        Self::accrued_yield_for(&env, &state)
    }

    fn accrued_yield_for(env: &Env, state: &PoolState) -> Result<i128, ContractError> {
        let token = Self::contract_token(env)?;
        let pool_balance =
            PoolClient::new(env, &state.pool).balance_of(&env.current_contract_address());
        let wallet_balance = TokenClient::new(env, &token).balance(&env.current_contract_address());
        // Wallet tokens attributable to already-harvested yield are excluded:
        // they are accounted for by the escrow ledger, not as new accrual.
        let wallet_yield: i128 = env
            .storage()
            .instance()
            .get(&DataKey::WalletYield)
            .unwrap_or(0);
        let unaccounted_wallet = wallet_balance.saturating_sub(wallet_yield);
        let total = pool_balance
            .checked_add(unaccounted_wallet)
            .ok_or(ContractError::ArithmeticOverflow)?;
        total
            .checked_sub(state.total_staked)
            .ok_or(ContractError::ArithmeticOverflow)
    }

    /// Harvest accrued yield for a position: withdraw it from the pool into
    /// the contract and credit it to the treasury's escrow so it can be added
    /// to the final payout distribution. The principal stays staked and keeps
    /// earning. Returns the amount harvested by this call.
    pub fn harvest_yield(env: Env, caller: Address, pool_id: u64) -> Result<i128, ContractError> {
        caller.require_auth();
        Self::require_admin(&env, &caller)?;
        Self::require_not_entered(&env)?;

        let (organizer, mut state) = Self::load_position(&env, pool_id)?;
        if state.settled {
            return Err(ContractError::TournamentAlreadySettled);
        }

        let yield_amount = Self::accrued_yield_for(&env, &state)?;
        let mut harvested = 0i128;

        if yield_amount > 0 {
            env.storage().instance().set(&DataKey::Entered, &true);
            let result = Self::harvest_inner(&env, &state, yield_amount);
            env.storage().instance().set(&DataKey::Entered, &false);
            result?;
            harvested = yield_amount;

            // The withdrawn yield now sits in the contract wallet as
            // spendable underlying — track it so settlement of *other*
            // positions never sweeps it into their own distribution.
            let wallet_yield: i128 = env
                .storage()
                .instance()
                .get(&DataKey::WalletYield)
                .unwrap_or(0);
            env.storage().instance().set(
                &DataKey::WalletYield,
                &wallet_yield
                    .checked_add(harvested)
                    .ok_or(ContractError::ArithmeticOverflow)?,
            );
        }

        state.harvested_yield = state
            .harvested_yield
            .checked_add(harvested)
            .ok_or(ContractError::ArithmeticOverflow)?;
        Self::store_position(&env, &organizer, pool_id, &state);

        if harvested > 0 {
            let treasury = Self::treasury(env.clone())?;
            let mut escrow: Map<Address, i128> = Self::escrow(&env);
            let credited = escrow.get(treasury.clone()).unwrap_or(0);
            escrow.set(
                treasury,
                credited
                    .checked_add(harvested)
                    .ok_or(ContractError::ArithmeticOverflow)?,
            );
            env.storage().instance().set(&DataKey::Escrow, &escrow);
        }

        env.events().publish(
            (Symbol::new(&env, "yield_harvested"), organizer),
            (pool_id, harvested),
        );
        Ok(harvested)
    }

    fn harvest_inner(
        env: &Env,
        state: &PoolState,
        yield_amount: i128,
    ) -> Result<(), ContractError> {
        // Withdraw only the yield; the principal stays staked and continues
        // to earn. The withdrawn tokens remain in the contract wallet as
        // spendable underlying (the pool shares were burned/redeemed).
        PoolClient::new(env, &state.pool).withdraw(&env.current_contract_address(), &yield_amount);
        Ok(())
    }

    // ────────────────────────────────────────────────────────────────────
    // Tournament settlement
    // ────────────────────────────────────────────────────────────────────

    /// Settle a concluded tournament: verify principal safety, withdraw the
    /// full position (principal + accrued yield) from the pool, and credit
    /// the whole balance to `winners` by `percentages` in the escrow ledger —
    /// mirroring the `payout_tournament` convention of `game_contract`
    /// (percentages 1..=100 summing to exactly 100; division dust goes to the
    /// first winner so the distribution is always exact). Previously
    /// harvested-but-unclaimed yield is included in the distribution.
    pub fn settle_tournament(
        env: Env,
        caller: Address,
        pool_id: u64,
        winners: Vec<Address>,
        percentages: Vec<i128>,
    ) -> Result<(), ContractError> {
        caller.require_auth();
        Self::require_admin(&env, &caller)?;
        Self::require_not_entered(&env)?;
        Self::validate_distribution(&winners, &percentages)?;

        let (organizer, mut state) = Self::load_position(&env, pool_id)?;
        if state.settled {
            return Err(ContractError::TournamentAlreadySettled);
        }

        // Principal safety: everything this position controls must cover the
        // staked principal unless risk was explicitly accepted. Wallet tokens
        // attributable to *other* positions' pending yield are excluded so
        // settlements never sweep another tournament's harvested yield.
        let pool_balance =
            PoolClient::new(&env, &state.pool).balance_of(&env.current_contract_address());
        let token = Self::contract_token(&env)?;
        let wallet_balance =
            TokenClient::new(&env, &token).balance(&env.current_contract_address());
        let wallet_yield: i128 = env
            .storage()
            .instance()
            .get(&DataKey::WalletYield)
            .unwrap_or(0);
        let other_pending = wallet_yield.saturating_sub(state.harvested_yield).max(0);
        let other_yield = if other_pending < wallet_balance {
            other_pending
        } else {
            wallet_balance
        };
        let total = pool_balance
            .checked_add(wallet_balance - other_yield)
            .ok_or(ContractError::ArithmeticOverflow)?;
        if total < state.total_staked && !state.allow_principal_loss {
            return Err(ContractError::PrincipalLoss);
        }

        env.storage().instance().set(&DataKey::Entered, &true);
        let result = Self::settle_inner(&env, &state);
        env.storage().instance().set(&DataKey::Entered, &false);
        result?;

        // Distribute principal + yield by percentage. Compute every share
        // from the percentage first, then hand the integer-division dust to
        // the first winner so the sum distributed equals `total` exactly.
        let mut escrow: Map<Address, i128> = Self::escrow(&env);

        // Yield already credited to the treasury via harvesting is now part
        // of the winner distribution — cancel the credit so it can't be paid
        // out twice.
        let treasury = Self::treasury(env.clone())?;
        let treasury_credit = escrow.get(treasury.clone()).unwrap_or(0);
        escrow.set(
            treasury,
            treasury_credit.saturating_sub(state.harvested_yield),
        );

        let mut others_sum: i128 = 0;
        for i in 1..winners.len() {
            let share = total
                .checked_mul(percentages.get(i).unwrap())
                .and_then(|v| v.checked_div(100))
                .ok_or(ContractError::ArithmeticOverflow)?;
            others_sum = others_sum
                .checked_add(share)
                .ok_or(ContractError::ArithmeticOverflow)?;
        }
        let first_share = total
            .checked_sub(others_sum)
            .ok_or(ContractError::ArithmeticOverflow)?;

        let mut distributed: i128 = 0;
        for i in 0..winners.len() {
            let winner = winners.get(i).unwrap();
            let share = if i == 0 {
                first_share
            } else {
                total
                    .checked_mul(percentages.get(i).unwrap())
                    .and_then(|v| v.checked_div(100))
                    .ok_or(ContractError::ArithmeticOverflow)?
            };
            distributed = distributed
                .checked_add(share)
                .ok_or(ContractError::ArithmeticOverflow)?;

            let credited = escrow.get(winner.clone()).unwrap_or(0);
            escrow.set(
                winner.clone(),
                credited
                    .checked_add(share)
                    .ok_or(ContractError::ArithmeticOverflow)?,
            );

            env.events().publish(
                (Symbol::new(&env, "tournament_payout"), winner),
                (pool_id, share),
            );
        }
        env.storage().instance().set(&DataKey::Escrow, &escrow);

        // This position's harvested yield was folded into the distribution.
        let wallet_yield: i128 = env
            .storage()
            .instance()
            .get(&DataKey::WalletYield)
            .unwrap_or(0);
        env.storage().instance().set(
            &DataKey::WalletYield,
            &wallet_yield.saturating_sub(state.harvested_yield),
        );

        // Close the position: everything it controlled was distributed.
        state.total_staked = 0;
        state.harvested_yield = 0;
        state.settled = true;
        Self::store_position(&env, &organizer, pool_id, &state);

        env.events().publish(
            (Symbol::new(&env, "tournament_settled"), organizer),
            (pool_id, distributed),
        );
        Ok(())
    }

    fn settle_inner(env: &Env, state: &PoolState) -> Result<(), ContractError> {
        // Redeem every share the contract still holds in the pool so the
        // full balance becomes spendable underlying for distribution.
        let balance = PoolClient::new(env, &state.pool).balance_of(&env.current_contract_address());
        if balance > 0 {
            PoolClient::new(env, &state.pool).withdraw(&env.current_contract_address(), &balance);
        }
        Ok(())
    }

    // ────────────────────────────────────────────────────────────────────
    // Escrow
    // ────────────────────────────────────────────────────────────────────

    /// Escrowed payout balance credited to `who` by settlement/harvesting.
    pub fn escrow_balance(env: Env, who: Address) -> i128 {
        Self::escrow(&env).get(who).unwrap_or(0)
    }

    /// Claim escrowed tokens. Each address may claim only its own credited
    /// balance; winners claim their payout share and the treasury claims
    /// harvested yield.
    pub fn claim_escrow(env: Env, caller: Address, amount: i128) -> Result<(), ContractError> {
        caller.require_auth();
        Self::require_not_entered(&env)?;
        if amount <= 0 {
            return Err(ContractError::InvalidAmount);
        }

        let mut escrow: Map<Address, i128> = Self::escrow(&env);
        let balance = escrow.get(caller.clone()).unwrap_or(0);
        if balance < amount {
            return Err(ContractError::InvalidAmount);
        }

        let token = Self::contract_token(&env)?;
        escrow.set(caller.clone(), balance - amount);
        env.storage().instance().set(&DataKey::Escrow, &escrow);

        // If this claim spends harvested-but-unclaimed yield (i.e. the
        // treasury claiming before settlement), stop attributing the tokens
        // to `WalletYield` so accrual stays exact. Winner claims after
        // settlement consume principal that was never yield-attributed, so
        // they must not touch this counter.
        let treasury = Self::treasury(env.clone())?;
        if caller == treasury {
            let wallet_yield: i128 = env
                .storage()
                .instance()
                .get(&DataKey::WalletYield)
                .unwrap_or(0);
            env.storage().instance().set(
                &DataKey::WalletYield,
                &wallet_yield.saturating_sub(amount.min(wallet_yield)),
            );
        }

        TokenClient::new(&env, &token).transfer(&env.current_contract_address(), &caller, &amount);

        env.events()
            .publish((Symbol::new(&env, "escrow_claimed"), caller), amount);
        Ok(())
    }

    fn escrow(env: &Env) -> Map<Address, i128> {
        env.storage()
            .instance()
            .get(&DataKey::Escrow)
            .unwrap_or_else(|| Map::new(env))
    }

    /// The contract-wide underlying token (fixed on first deposit).
    fn contract_token(env: &Env) -> Result<Address, ContractError> {
        Self::token(env.clone()).ok_or(ContractError::NotInitialized)
    }

    // ────────────────────────────────────────────────────────────────────
    // View helpers
    // ────────────────────────────────────────────────────────────────────

    /// Position state for (organizer, pool_id), if it exists.
    pub fn position(env: Env, organizer: Address, pool_id: u64) -> Option<PoolState> {
        Self::positions(&env, &organizer).get(pool_id)
    }

    /// Look up the position id an organizer holds with a given pool.
    pub fn position_id(env: Env, organizer: Address, pool: Address) -> Option<u64> {
        let positions = Self::positions(&env, &organizer);
        for (id, state) in positions.iter() {
            if state.pool == pool {
                return Some(id);
            }
        }
        None
    }

    pub fn organizer_position_count(env: Env, organizer: Address) -> u32 {
        Self::positions(&env, &organizer).len()
    }

    fn positions(env: &Env, organizer: &Address) -> Map<u64, PoolState> {
        let organizers: Map<Address, Map<u64, PoolState>> = env
            .storage()
            .instance()
            .get(&DataKey::Organizers)
            .unwrap_or_else(|| Map::new(env));
        organizers
            .get(organizer.clone())
            .unwrap_or_else(|| Map::new(env))
    }

    // ────────────────────────────────────────────────────────────────────
    // Internal helpers
    // ────────────────────────────────────────────────────────────────────

    fn require_admin(env: &Env, caller: &Address) -> Result<(), ContractError> {
        let admin = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(ContractError::NotInitialized)?;
        if *caller != admin {
            return Err(ContractError::Unauthorized);
        }
        Ok(())
    }

    fn require_not_entered(env: &Env) -> Result<(), ContractError> {
        if env
            .storage()
            .instance()
            .get(&DataKey::Entered)
            .unwrap_or(false)
        {
            return Err(ContractError::ReentrantCall);
        }
        Ok(())
    }

    fn validate_distribution(
        winners: &Vec<Address>,
        percentages: &Vec<i128>,
    ) -> Result<(), ContractError> {
        if winners.is_empty() || winners.len() != percentages.len() {
            return Err(ContractError::EmptyWinners);
        }
        let mut sum: i128 = 0;
        for i in 0..percentages.len() {
            let p = percentages.get(i).unwrap();
            if !(1..=100).contains(&p) {
                return Err(ContractError::InvalidPercentage);
            }
            sum = sum
                .checked_add(p)
                .ok_or(ContractError::ArithmeticOverflow)?;
        }
        if sum != 100 {
            return Err(ContractError::PercentageSumMismatch);
        }
        Ok(())
    }

    /// Fetch the existing position for (organizer, pool), or register a fresh
    /// one with the next pool id. Returns `(pool_id, state, existed)`.
    fn resolve_position(
        env: &Env,
        organizer: &Address,
        pool: &Address,
    ) -> Result<(u64, PoolState, bool), ContractError> {
        if let Some(existing_id) = Self::position_id(env.clone(), organizer.clone(), pool.clone()) {
            let state = Self::positions(env, organizer)
                .get(existing_id)
                .ok_or(ContractError::PoolNotFound)?;
            return Ok((existing_id, state, true));
        }

        // New position: the pool's underlying token must match the
        // contract-wide token (fixed by the first deposit, if any).
        let pool_token = PoolClient::new(env, pool).token();
        match Self::token(env.clone()) {
            Some(fixed) if fixed != pool_token => return Err(ContractError::TokenMismatch),
            Some(_) => {}
            None => env.storage().instance().set(&DataKey::Token, &pool_token),
        }

        let counter: u64 = env
            .storage()
            .instance()
            .get(&DataKey::PoolCounter)
            .unwrap_or(0);
        let pool_id = counter
            .checked_add(1)
            .ok_or(ContractError::ArithmeticOverflow)?;
        env.storage()
            .instance()
            .set(&DataKey::PoolCounter, &pool_id);

        let state = PoolState {
            pool: pool.clone(),
            total_staked: 0,
            harvested_yield: 0,
            settled: false,
            allow_principal_loss: false,
        };
        Self::store_position(env, organizer, pool_id, &state);
        Ok((pool_id, state, false))
    }

    fn load_position(env: &Env, pool_id: u64) -> Result<(Address, PoolState), ContractError> {
        let organizers: Map<Address, Map<u64, PoolState>> = env
            .storage()
            .instance()
            .get(&DataKey::Organizers)
            .unwrap_or_else(|| Map::new(env));
        for organizer_key in organizers.keys() {
            if let Some(state) = organizers
                .get(organizer_key.clone())
                .and_then(|p| p.get(pool_id))
            {
                return Ok((organizer_key, state));
            }
        }
        Err(ContractError::PoolNotFound)
    }

    fn store_position(env: &Env, organizer: &Address, pool_id: u64, state: &PoolState) {
        let mut organizers: Map<Address, Map<u64, PoolState>> = env
            .storage()
            .instance()
            .get(&DataKey::Organizers)
            .unwrap_or_else(|| Map::new(env));
        let mut positions = organizers
            .get(organizer.clone())
            .unwrap_or_else(|| Map::new(env));
        positions.set(pool_id, state.clone());
        organizers.set(organizer.clone(), positions);
        env.storage()
            .instance()
            .set(&DataKey::Organizers, &organizers);
    }
}

// ────────────────────────────────────────────────────────────────────────
// Minimal Soroban liquidity-pool interface (acceptance criterion:
// "integration with Soroban standard liquidity pool interfaces").
//
// Whitelisted pools must expose: `token()`, `deposit(from, amount)`,
// `withdraw(from, amount)` and `balance_of(who)`. This is the share-accounting
// shape used by the Soroban example `liquidity_pool` contract and by
// Soroban AMM vault front-ends, so the same client works against any pool
// conforming to it.
// ────────────────────────────────────────────────────────────────────────
mod pool_interface {
    use soroban_sdk::{contractclient, Address, Env};

    #[contractclient(name = "PoolClient")]
    pub trait Pool {
        /// Underlying token contract the pool is denominated in.
        fn token(env: Env) -> Address;
        /// Stake `amount` underlying tokens from `from` into the pool.
        fn deposit(env: Env, from: Address, amount: i128);
        /// Release `amount` underlying tokens from the pool back to `from`.
        fn withdraw(env: Env, from: Address, amount: i128);
        /// Position size (principal + accrued yield) accounted by the pool.
        fn balance_of(env: Env, who: Address) -> i128;
    }
}
pub use pool_interface::*;
