#![cfg(test)]
extern crate std;

use super::*;
use soroban_sdk::token::{StellarAssetClient, TokenClient as SdkTokenClient};
use soroban_sdk::{testutils::Address as _, vec, Address, Env, Symbol, TryIntoVal, Val};

// ────────────────────────────────────────────────────────────────────────
// Mock whitelisted liquidity pool implementing the SC-46 pool interface:
//   token() / deposit(from, amount) / withdraw(from, amount) / balance_of(who)
// It accrues a configurable yield on deposits (simulating swap fees) and
// supports two attack simulations: principal loss and reentrant callbacks.
// ────────────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
enum MockKey {
    Token,
    /// (depositor, shares) outstanding per depositor.
    Shares,
    /// Yield rate in basis points applied at deposit time.
    YieldBips,
    /// Target contract + function to call back into during deposit.
    Callback,
}

#[contract]
struct MockPool;

#[contractimpl]
impl MockPool {
    pub fn init(env: Env, token: Address) {
        env.storage().instance().set(&MockKey::Token, &token);
        env.storage().instance().set(&MockKey::YieldBips, &0u32);
    }

    pub fn set_yield_bips(env: Env, caller: Address, bips: u32) {
        caller.require_auth();
        env.storage().instance().set(&MockKey::YieldBips, &bips);
    }

    /// Point the pool at a target contract + function to call back into
    /// during `deposit` (reentrancy attack simulation). Pass zero addresses
    /// to clear; the guard only checks well-formed targets.
    pub fn set_callback(env: Env, caller: Address, target: Address, func: Symbol) {
        caller.require_auth();
        env.storage()
            .instance()
            .set(&MockKey::Callback, &(target, func));
    }

    pub fn token(env: Env) -> Address {
        env.storage().instance().get(&MockKey::Token).unwrap()
    }

    pub fn deposit(env: Env, from: Address, amount: i128) {
        if amount <= 0 {
            panic!("amount must be positive");
        }
        // Simulated yield: shares minted = amount * (1 + bips/10000).
        let bips: u32 = env
            .storage()
            .instance()
            .get(&MockKey::YieldBips)
            .unwrap_or(0);
        let shares = amount + (amount * bips as i128) / 10_000;

        // Reentrancy attack hook, if configured. The nested call mirrors the
        // real signature: stake(caller, pool, amount).
        if let Some((target, func)) = env
            .storage()
            .instance()
            .get::<_, (Address, Symbol)>(&MockKey::Callback)
        {
            let amount_val: Val = amount.try_into_val(&env).unwrap();
            let pool_self = env.current_contract_address();
            let _: Val = env.invoke_contract(
                &target,
                &func,
                vec![&env, from.to_val(), pool_self.to_val(), amount_val],
            );
        }

        let mut shares_map: soroban_sdk::Map<Address, i128> = env
            .storage()
            .instance()
            .get(&MockKey::Shares)
            .unwrap_or_else(|| soroban_sdk::Map::new(&env));
        let cur = shares_map.get(from.clone()).unwrap_or(0);
        shares_map.set(from, cur + shares);
        env.storage().instance().set(&MockKey::Shares, &shares_map);
    }

    pub fn withdraw(env: Env, from: Address, amount: i128) {
        if amount <= 0 {
            panic!("amount must be positive");
        }
        let mut shares_map: soroban_sdk::Map<Address, i128> = env
            .storage()
            .instance()
            .get(&MockKey::Shares)
            .unwrap_or_else(|| soroban_sdk::Map::new(&env));
        let cur = shares_map.get(from.clone()).unwrap_or(0);
        if cur < amount {
            panic!("insufficient shares");
        }
        shares_map.set(from.clone(), cur - amount);
        env.storage().instance().set(&MockKey::Shares, &shares_map);

        // Pay out the underlying tokens from the pool's own balance, like a
        // real liquidity pool redeeming shares.
        let token: Address = env.storage().instance().get(&MockKey::Token).unwrap();
        TokenClient::new(&env, &token).transfer(&env.current_contract_address(), &from, &amount);
    }

    /// Simulate an impermanent-loss event: destroy `amount` of `holder`'s
    /// shares and move the corresponding underlying out of reach, so the
    /// holder's controllable total drops below its principal.
    pub fn simulate_loss(env: Env, caller: Address, holder: Address, amount: i128) {
        caller.require_auth();
        let mut shares_map: soroban_sdk::Map<Address, i128> = env
            .storage()
            .instance()
            .get(&MockKey::Shares)
            .unwrap_or_else(|| soroban_sdk::Map::new(&env));
        let cur = shares_map.get(holder.clone()).unwrap_or(0);
        if cur < amount {
            panic!("insufficient shares");
        }
        shares_map.set(holder, cur - amount);
        env.storage().instance().set(&MockKey::Shares, &shares_map);

        let token: Address = env.storage().instance().get(&MockKey::Token).unwrap();
        TokenClient::new(&env, &token).transfer(&env.current_contract_address(), &caller, &amount);
    }

    pub fn balance_of(env: Env, who: Address) -> i128 {
        let shares_map: soroban_sdk::Map<Address, i128> = env
            .storage()
            .instance()
            .get(&MockKey::Shares)
            .unwrap_or_else(|| soroban_sdk::Map::new(&env));
        shares_map.get(who).unwrap_or(0)
    }
}

// ────────────────────────────────────────────────────────────────────────
// Test fixture
// ────────────────────────────────────────────────────────────────────────

struct Ctx<'a> {
    env: Env,
    ys_id: Address,
    ys: YieldStakingContractClient<'a>,
    token_id: Address,
    token: SdkTokenClient<'a>,
    pool_id: Address,
    pool: MockPoolClient<'a>,
    admin: Address,
    treasury: Address,
    organizer: Address,
}

fn setup() -> Ctx<'static> {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);
    let organizer = Address::generate(&env);

    let ys_id = env.register_contract(None, YieldStakingContract);
    let ys = YieldStakingContractClient::new(&env, &ys_id);

    let token_id = env.register_stellar_asset_contract_v2(admin.clone());
    let token_id = token_id.address();
    let token = SdkTokenClient::new(&env, &token_id);

    ys.initialize(&admin, &treasury);

    let pool_id = env.register_contract(None, MockPool);
    let pool = MockPoolClient::new(&env, &pool_id);
    pool.init(&token_id);
    pool.set_yield_bips(&admin, &1_000); // 10 % simulated yield

    // Fund the pool with an underlying buffer that backs the simulated
    // yield, like swap-fee revenue accumulated by a real AMM pool.
    StellarAssetClient::new(&env, &token_id).mint(&pool_id, &10_000_000);

    // Fund the organizer with prize-pool funds.
    StellarAssetClient::new(&env, &token_id).mint(&organizer, &1_000_000);

    // SAFETY: each test holds the Ctx (and therefore the Env) for its whole
    // body, so the clients never outlive the Env they borrow.
    unsafe {
        std::mem::transmute::<Ctx<'_>, Ctx<'static>>(Ctx {
            env,
            ys_id,
            ys,
            token_id,
            token,
            pool_id,
            pool,
            admin,
            treasury,
            organizer,
        })
    }
}

// ────────────────────────────────────────────────────────────────────────
// Initialization and whitelist
// ────────────────────────────────────────────────────────────────────────

#[test]
fn test_initialize_once() {
    let f = setup();
    assert_eq!(f.ys.admin(), f.admin);
    assert_eq!(f.ys.treasury(), f.treasury);

    let res = f.ys.try_initialize(&f.admin, &f.treasury);
    assert_eq!(res, Err(Ok(ContractError::AlreadyInitialized)));
}

#[test]
fn test_uninitialized_rejects_admin_query() {
    let env = Env::default();
    env.mock_all_auths();
    let ys_id = env.register_contract(None, YieldStakingContract);
    let ys = YieldStakingContractClient::new(&env, &ys_id);
    let res = ys.try_admin();
    assert_eq!(res, Err(Ok(ContractError::NotInitialized)));
}

#[test]
fn test_whitelist_roundtrip() {
    let f = setup();
    assert!(!f.ys.is_pool_whitelisted(&f.pool_id));
    f.ys.whitelist_pool(&f.admin, &f.pool_id);
    assert!(f.ys.is_pool_whitelisted(&f.pool_id));
    f.ys.unwhitelist_pool(&f.admin, &f.pool_id);
    assert!(!f.ys.is_pool_whitelisted(&f.pool_id));
}

#[test]
fn test_whitelist_rejects_non_admin_and_duplicates() {
    let f = setup();
    let attacker = Address::generate(&f.env);
    let res = f.ys.try_whitelist_pool(&attacker, &f.pool_id);
    assert_eq!(res, Err(Ok(ContractError::Unauthorized)));

    f.ys.whitelist_pool(&f.admin, &f.pool_id);
    let res = f.ys.try_whitelist_pool(&f.admin, &f.pool_id);
    assert_eq!(res, Err(Ok(ContractError::PoolAlreadyWhitelisted)));
}

// ────────────────────────────────────────────────────────────────────────
// Staking (deposit)
// ────────────────────────────────────────────────────────────────────────

#[test]
fn test_stake_requires_whitelisted_pool() {
    let f = setup();
    let res = f.ys.try_stake(&f.organizer, &f.pool_id, &1_000);
    assert_eq!(res, Err(Ok(ContractError::PoolNotWhitelisted)));

    f.ys.whitelist_pool(&f.admin, &f.pool_id);
    let id = f.ys.stake(&f.organizer, &f.pool_id, &1_000);
    assert_eq!(id, 1);
}

#[test]
fn test_stake_rejects_bad_amount() {
    let f = setup();
    f.ys.whitelist_pool(&f.admin, &f.pool_id);

    let res = f.ys.try_stake(&f.organizer, &f.pool_id, &0);
    assert_eq!(res, Err(Ok(ContractError::InvalidAmount)));
    let res = f.ys.try_stake(&f.organizer, &f.pool_id, &-5);
    assert_eq!(res, Err(Ok(ContractError::InvalidAmount)));
}

#[test]
fn test_stake_moves_principal_into_pool() {
    let f = setup();
    f.ys.whitelist_pool(&f.admin, &f.pool_id);

    let before = f.token.balance(&f.organizer);
    f.ys.stake(&f.organizer, &f.pool_id, &50_000);

    // Organizer paid exactly the staked amount; the pool holds it as 110 %
    // shares (principal + simulated yield) and the contract wallet is empty.
    assert_eq!(before - f.token.balance(&f.organizer), 50_000);
    assert_eq!(f.token.balance(&f.ys_id), 0);
    assert_eq!(
        f.ys.position(&f.organizer, &1).unwrap().total_staked,
        50_000
    );
    assert_eq!(f.pool.balance_of(&f.ys_id), 55_000);
}

#[test]
fn test_stake_topup_same_position() {
    let f = setup();
    f.ys.whitelist_pool(&f.admin, &f.pool_id);
    f.ys.stake(&f.organizer, &f.pool_id, &10_000);
    f.ys.stake(&f.organizer, &f.pool_id, &5_000);

    assert_eq!(f.ys.organizer_position_count(&f.organizer), 1);
    assert_eq!(
        f.ys.position(&f.organizer, &1).unwrap().total_staked,
        15_000
    );
}

#[test]
fn test_stake_into_settled_position_rejected() {
    let f = setup();
    f.ys.whitelist_pool(&f.admin, &f.pool_id);
    f.ys.stake(&f.organizer, &f.pool_id, &10_000);

    let w1 = Address::generate(&f.env);
    f.ys.settle_tournament(&f.admin, &1, &vec![&f.env, w1], &vec![&f.env, 100]);

    let res = f.ys.try_stake(&f.organizer, &f.pool_id, &1_000);
    assert_eq!(res, Err(Ok(ContractError::TournamentAlreadySettled)));
}

#[test]
fn test_stake_token_mismatch_rejected() {
    let f = setup();
    f.ys.whitelist_pool(&f.admin, &f.pool_id);
    f.ys.stake(&f.organizer, &f.pool_id, &1_000);

    // A second pool denominated in a different token must be rejected.
    let issuer = Address::generate(&f.env);
    let other_token = f.env.register_stellar_asset_contract_v2(issuer).address();
    let pool2_id = f.env.register_contract(None, MockPool);
    MockPoolClient::new(&f.env, &pool2_id).init(&other_token);
    f.ys.whitelist_pool(&f.admin, &pool2_id);

    let res = f.ys.try_stake(&f.organizer, &pool2_id, &1_000);
    assert_eq!(res, Err(Ok(ContractError::TokenMismatch)));
}

// ────────────────────────────────────────────────────────────────────────
// Yield accrual and harvesting
// ────────────────────────────────────────────────────────────────────────

#[test]
fn test_yield_accrues_and_harvests_to_treasury_escrow() {
    let f = setup();
    f.ys.whitelist_pool(&f.admin, &f.pool_id);
    f.ys.stake(&f.organizer, &f.pool_id, &100_000);

    // 10 % simulated yield accrued on the deposit.
    assert_eq!(f.ys.accrued_yield(&1), 10_000);

    let harvested = f.ys.harvest_yield(&f.admin, &1);
    assert_eq!(harvested, 10_000);
    assert_eq!(f.ys.escrow_balance(&f.treasury), 10_000);

    // Principal remains staked; accrued yield resets without double counting.
    assert_eq!(
        f.ys.position(&f.organizer, &1).unwrap().total_staked,
        100_000
    );
    assert_eq!(f.pool.balance_of(&f.ys_id), 100_000);
    assert_eq!(f.ys.accrued_yield(&1), 0);

    // Harvesting again with no new yield is a no-op.
    let again = f.ys.harvest_yield(&f.admin, &1);
    assert_eq!(again, 0);
    assert_eq!(f.ys.escrow_balance(&f.treasury), 10_000);
}

#[test]
fn test_harvest_accumulates_across_deposits() {
    let f = setup();
    f.ys.whitelist_pool(&f.admin, &f.pool_id);
    f.ys.stake(&f.organizer, &f.pool_id, &10_000);
    f.ys.harvest_yield(&f.admin, &1); // +1 000

    // Additional deposit produces additional yield.
    f.ys.stake(&f.organizer, &f.pool_id, &20_000);
    assert_eq!(f.ys.accrued_yield(&1), 2_000);
    f.ys.harvest_yield(&f.admin, &1); // +2 000

    assert_eq!(f.ys.escrow_balance(&f.treasury), 3_000);
    assert_eq!(
        f.ys.position(&f.organizer, &1).unwrap().harvested_yield,
        3_000
    );
    assert_eq!(f.ys.accrued_yield(&1), 0);
}

#[test]
fn test_harvest_rejects_non_admin() {
    let f = setup();
    f.ys.whitelist_pool(&f.admin, &f.pool_id);
    f.ys.stake(&f.organizer, &f.pool_id, &10_000);

    let stranger = Address::generate(&f.env);
    let res = f.ys.try_harvest_yield(&stranger, &1);
    assert_eq!(res, Err(Ok(ContractError::Unauthorized)));
}

#[test]
fn test_harvest_unknown_position() {
    let f = setup();
    let res = f.ys.try_harvest_yield(&f.admin, &99);
    assert_eq!(res, Err(Ok(ContractError::PoolNotFound)));
}

// ────────────────────────────────────────────────────────────────────────
// Settlement
// ────────────────────────────────────────────────────────────────────────

#[test]
fn test_settle_distributes_principal_plus_yield() {
    let f = setup();
    f.ys.whitelist_pool(&f.admin, &f.pool_id);
    f.ys.stake(&f.organizer, &f.pool_id, &100_000);
    f.ys.harvest_yield(&f.admin, &1); // treasury escrow +10 000

    let w1 = Address::generate(&f.env);
    let w2 = Address::generate(&f.env);
    let w3 = Address::generate(&f.env);
    let winners = vec![&f.env, w1.clone(), w2.clone(), w3.clone()];
    let pcts = vec![&f.env, 50, 30, 20];

    f.ys.settle_tournament(&f.admin, &1, &winners, &pcts);

    // Pool = 110 000 (principal 100 000 + yield 10 000). The treasury's
    // harvested-yield credit is folded into the distribution (no double pay).
    assert_eq!(f.ys.escrow_balance(&w1), 55_000);
    assert_eq!(f.ys.escrow_balance(&w2), 33_000);
    assert_eq!(f.ys.escrow_balance(&w3), 22_000);
    assert_eq!(f.ys.escrow_balance(&f.treasury), 0);

    // Position closed.
    let pos = f.ys.position(&f.organizer, &1).unwrap();
    assert!(pos.settled);
    assert_eq!(pos.total_staked, 0);
    assert_eq!(pos.harvested_yield, 0);

    // Double settlement rejected.
    let res = f.ys.try_settle_tournament(&f.admin, &1, &winners, &pcts);
    assert_eq!(res, Err(Ok(ContractError::TournamentAlreadySettled)));
}

#[test]
fn test_settle_includes_unharvested_yield_with_dust_to_first() {
    let f = setup();
    f.ys.whitelist_pool(&f.admin, &f.pool_id);
    f.ys.stake(&f.organizer, &f.pool_id, &33_300);

    let w1 = Address::generate(&f.env);
    let w2 = Address::generate(&f.env);
    let winners = vec![&f.env, w1.clone(), w2.clone()];
    // Total 36 630 at 60 / 40 → 21 978 and 14 652 exactly (no dust); vary the
    // stake slightly to exercise the dust path in the next test.
    let pcts = vec![&f.env, 60, 40];

    f.ys.settle_tournament(&f.admin, &1, &winners, &pcts);

    assert_eq!(f.ys.escrow_balance(&w1), 21_978);
    assert_eq!(f.ys.escrow_balance(&w2), 14_652);
}

#[test]
fn test_settle_dust_goes_to_first_winner() {
    let f = setup();
    f.ys.whitelist_pool(&f.admin, &f.pool_id);
    f.ys.stake(&f.organizer, &f.pool_id, &33_333);

    let w1 = Address::generate(&f.env);
    let w2 = Address::generate(&f.env);
    let winners = vec![&f.env, w1.clone(), w2.clone()];
    // Total 36 666 at 33/67: w2 = 24 566, remainder 12 100 → w1.
    let pcts = vec![&f.env, 33, 67];

    f.ys.settle_tournament(&f.admin, &1, &winners, &pcts);
    assert_eq!(f.ys.escrow_balance(&w1), 12_100);
    assert_eq!(f.ys.escrow_balance(&w2), 24_566);
}

#[test]
fn test_settle_rejects_non_admin_and_bad_percentages() {
    let f = setup();
    f.ys.whitelist_pool(&f.admin, &f.pool_id);
    f.ys.stake(&f.organizer, &f.pool_id, &10_000);

    let w1 = Address::generate(&f.env);
    let winners = vec![&f.env, w1.clone()];

    let stranger = Address::generate(&f.env);
    let res =
        f.ys.try_settle_tournament(&stranger, &1, &winners, &vec![&f.env, 100]);
    assert_eq!(res, Err(Ok(ContractError::Unauthorized)));

    // Zero winners.
    let res =
        f.ys.try_settle_tournament(&f.admin, &1, &vec![&f.env], &vec![&f.env]);
    assert_eq!(res, Err(Ok(ContractError::EmptyWinners)));

    // Percentages out of range.
    let res =
        f.ys.try_settle_tournament(&f.admin, &1, &winners, &vec![&f.env, 0]);
    assert_eq!(res, Err(Ok(ContractError::InvalidPercentage)));
    let res =
        f.ys.try_settle_tournament(&f.admin, &1, &winners, &vec![&f.env, 101]);
    assert_eq!(res, Err(Ok(ContractError::InvalidPercentage)));

    // Sum mismatch.
    let res =
        f.ys.try_settle_tournament(&f.admin, &1, &winners, &vec![&f.env, 99]);
    assert_eq!(res, Err(Ok(ContractError::PercentageSumMismatch)));

    // Length mismatch between winners and percentages.
    let res =
        f.ys.try_settle_tournament(&f.admin, &1, &winners, &vec![&f.env, 50, 50]);
    assert_eq!(res, Err(Ok(ContractError::EmptyWinners)));
}

#[test]
fn test_settle_principal_safety_zero_loss_mode() {
    let f = setup();
    f.ys.whitelist_pool(&f.admin, &f.pool_id);
    f.ys.stake(&f.organizer, &f.pool_id, &100_000);

    // Simulate an impermanent-loss event: 30 000 of the position's 110 000
    // shares (100 000 principal + 10 000 yield) are destroyed along with
    // their underlying, leaving only 80 000 controllable.
    f.pool.set_yield_bips(&f.admin, &0);
    f.pool.simulate_loss(&f.admin, &f.ys_id, &30_000);

    let w1 = Address::generate(&f.env);
    let winners = vec![&f.env, w1.clone()];
    let res =
        f.ys.try_settle_tournament(&f.admin, &1, &winners, &vec![&f.env, 100]);
    assert_eq!(res, Err(Ok(ContractError::PrincipalLoss)));

    // After explicitly accepting the loss, settlement proceeds and pays out
    // only what remains (80 000).
    f.ys.set_allow_principal_loss(&f.admin, &1, &true);
    f.ys.settle_tournament(&f.admin, &1, &winners, &vec![&f.env, 100]);
    assert_eq!(f.ys.escrow_balance(&w1), 80_000);
}

#[test]
fn test_settle_unknown_position() {
    let f = setup();
    let w1 = Address::generate(&f.env);
    let winners = vec![&f.env, w1.clone()];
    let res =
        f.ys.try_settle_tournament(&f.admin, &42, &winners, &vec![&f.env, 100]);
    assert_eq!(res, Err(Ok(ContractError::PoolNotFound)));
}

// ────────────────────────────────────────────────────────────────────────
// Escrow claims
// ────────────────────────────────────────────────────────────────────────

#[test]
fn test_claim_escrow_pays_out_winners_and_treasury() {
    let f = setup();
    f.ys.whitelist_pool(&f.admin, &f.pool_id);
    f.ys.stake(&f.organizer, &f.pool_id, &100_000);
    f.ys.harvest_yield(&f.admin, &1);

    let w1 = Address::generate(&f.env);
    let winners = vec![&f.env, w1.clone()];
    f.ys.settle_tournament(&f.admin, &1, &winners, &vec![&f.env, 100]);

    // Winner claims the full payout (principal + yield; the harvested-yield
    // credit was folded into the winner distribution at settlement).
    let before = f.token.balance(&w1);
    f.ys.claim_escrow(&w1, &110_000);
    assert_eq!(f.token.balance(&w1) - before, 110_000);
    assert_eq!(f.ys.escrow_balance(&w1), 0);
    assert_eq!(f.ys.escrow_balance(&f.treasury), 0);

    // Over-claiming is rejected.
    let res = f.ys.try_claim_escrow(&w1, &1);
    assert_eq!(res, Err(Ok(ContractError::InvalidAmount)));
}

#[test]
fn test_treasury_claims_harvested_yield_before_settlement() {
    let f = setup();
    f.ys.whitelist_pool(&f.admin, &f.pool_id);
    f.ys.stake(&f.organizer, &f.pool_id, &100_000);
    f.ys.harvest_yield(&f.admin, &1); // escrow +10 000, wallet holds 10 000

    // Treasury claims the yield right away; principal stays staked.
    let before = f.token.balance(&f.treasury);
    f.ys.claim_escrow(&f.treasury, &10_000);
    assert_eq!(f.token.balance(&f.treasury) - before, 10_000);

    // Accrual stays exact after the claim (no phantom 10 000).
    assert_eq!(f.ys.accrued_yield(&1), 0);
    assert_eq!(f.pool.balance_of(&f.ys_id), 100_000);

    // Settlement then distributes principal only.
    let w1 = Address::generate(&f.env);
    std::eprintln!("PROBE pool_shares={}", f.pool.balance_of(&f.ys_id));
    std::eprintln!("PROBE wallet={}", f.token.balance(&f.ys_id));
    std::eprintln!("PROBE accrued={}", f.ys.accrued_yield(&1));
    f.ys.settle_tournament(&f.admin, &1, &vec![&f.env, w1.clone()], &vec![&f.env, 100]);
    std::eprintln!("PROBE w1_escrow={}", f.ys.escrow_balance(&w1));
    assert_eq!(f.ys.escrow_balance(&w1), 100_000);
}

#[test]
fn test_claim_escrow_rejects_zero_and_unknown() {
    let f = setup();
    let stranger = Address::generate(&f.env);
    let res = f.ys.try_claim_escrow(&stranger, &0);
    assert_eq!(res, Err(Ok(ContractError::InvalidAmount)));

    let res = f.ys.try_claim_escrow(&stranger, &10);
    assert_eq!(res, Err(Ok(ContractError::InvalidAmount)));
}

// ────────────────────────────────────────────────────────────────────────
// Unstake (early exit)
// ────────────────────────────────────────────────────────────────────────

#[test]
fn test_unstake_returns_principal() {
    let f = setup();
    f.ys.whitelist_pool(&f.admin, &f.pool_id);
    f.ys.stake(&f.organizer, &f.pool_id, &50_000);

    let before = f.token.balance(&f.organizer);
    f.ys.unstake(&f.organizer, &1, &20_000);
    assert_eq!(f.token.balance(&f.organizer) - before, 20_000);
    assert_eq!(
        f.ys.position(&f.organizer, &1).unwrap().total_staked,
        30_000
    );

    // Overdraw rejected.
    let res = f.ys.try_unstake(&f.organizer, &1, &31_000);
    assert_eq!(res, Err(Ok(ContractError::InvalidAmount)));

    // Only the position owner may unstake.
    let stranger = Address::generate(&f.env);
    let res = f.ys.try_unstake(&stranger, &1, &1);
    assert_eq!(res, Err(Ok(ContractError::Unauthorized)));
}

// ────────────────────────────────────────────────────────────────────────
// Reentrancy guard
// ────────────────────────────────────────────────────────────────────────

#[test]
fn test_reentrant_stake_rejected() {
    let f = setup();
    f.ys.whitelist_pool(&f.admin, &f.pool_id);

    // The pool re-enters YieldStakingContract::stake during its own deposit.
    f.pool
        .set_callback(&f.admin, &f.ys_id, &Symbol::new(&f.env, "stake"));

    // The nested call must trip the guard and abort the whole outer stake —
    // the typed error is swallowed by the nested frame (guest abort), so we
    // assert the outer call failed and left no phantom state behind.
    let res = f.ys.try_stake(&f.organizer, &f.pool_id, &1_000);
    assert!(
        res.is_err(),
        "nested reentrant call must abort the outer stake"
    );
    assert!(f.ys.position(&f.organizer, &1).is_none());
    // Organizer's funds were rolled back with the aborted transaction.
    assert_eq!(f.token.balance(&f.organizer), 1_000_000);

    // Directly verify the guard trips with the typed error when the contract
    // is marked as entered (i.e. mid-external-call).
    f.env.as_contract(&f.ys_id, || {
        f.env.storage().instance().set(&DataKey::Entered, &true);
        let result = YieldStakingContract::stake(
            f.env.clone(),
            f.organizer.clone(),
            f.pool_id.clone(),
            1_000,
        );
        assert_eq!(result, Err(ContractError::ReentrantCall));
        f.env.storage().instance().set(&DataKey::Entered, &false);
    });
}

// ────────────────────────────────────────────────────────────────────────
// Arithmetic overflow
// ────────────────────────────────────────────────────────────────────────

#[test]
fn test_stake_amount_overflow_rejected() {
    let f = setup();
    f.ys.whitelist_pool(&f.admin, &f.pool_id);
    f.pool.set_yield_bips(&f.admin, &0); // avoid overflow inside the mock

    // Seed a position directly at the i128 ceiling (bypassing token
    // plumbing, as in game_contract's seed_* test helpers) so that the next
    // stake must overflow the checked add.
    f.env.as_contract(&f.ys_id, || {
        let state = PoolState {
            pool: f.pool_id.clone(),
            total_staked: i128::MAX - 1,
            harvested_yield: 0,
            settled: false,
            allow_principal_loss: false,
        };
        let mut organizers: Map<Address, Map<u64, PoolState>> = f
            .env
            .storage()
            .instance()
            .get(&DataKey::Organizers)
            .unwrap_or_else(|| Map::new(&f.env));
        let mut positions = organizers
            .get(f.organizer.clone())
            .unwrap_or_else(|| Map::new(&f.env));
        positions.set(1, state);
        organizers.set(f.organizer.clone(), positions);
        f.env
            .storage()
            .instance()
            .set(&DataKey::Organizers, &organizers);
        f.env.storage().instance().set(&DataKey::Token, &f.token_id);
        f.env.storage().instance().set(&DataKey::PoolCounter, &1u64);
    });

    // A stake of 2 would overflow `total_staked`; the checked add must
    // reject it before any token transfer happens.
    let res = f.ys.try_stake(&f.organizer, &f.pool_id, &2);
    assert_eq!(res, Err(Ok(ContractError::ArithmeticOverflow)));

    // Sanity: a stake of 1 still succeeds and lands exactly at i128::MAX.
    f.ys.stake(&f.organizer, &f.pool_id, &1);
    assert_eq!(
        f.ys.position(&f.organizer, &1).unwrap().total_staked,
        i128::MAX
    );
}

// ────────────────────────────────────────────────────────────────────────
// Helpers
// ────────────────────────────────────────────────────────────────────────

#[allow(dead_code)]
fn env_register_token(env: &Env) -> Address {
    let issuer = Address::generate(env);
    env.register_stellar_asset_contract_v2(issuer).address()
}
