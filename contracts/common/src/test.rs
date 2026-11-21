#![cfg(test)]

use super::*;
use soroban_sdk::{
    contract, contractimpl, symbol_short,
    testutils::{Address as _, Ledger},
    Address, Env,
};

#[contract]
struct Harness;

#[contractimpl]
impl Harness {
    pub fn noop(_env: Env) {}
}

fn harness() -> (Env, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let id = env.register_contract(None, Harness);
    (env, id)
}

#[test]
fn admin_initialize_and_authorize() {
    let (env, id) = harness();
    let admin = Address::generate(&env);

    env.as_contract(&id, || {
        admin::initialize(&env, &admin, CommonError::AlreadyInitialized);
    });
    env.as_contract(&id, || {
        assert!(admin::has(&env));
        assert_eq!(admin::get(&env), Some(admin.clone()));
        assert_eq!(
            admin::get_required(&env, CommonError::NotInitialized),
            admin
        );
        admin::require_admin(&env, &admin, CommonError::NotAdmin);
    });
}

#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn admin_rejects_non_admin() {
    let (env, id) = harness();
    let admin = Address::generate(&env);
    let other = Address::generate(&env);

    env.as_contract(&id, || {
        admin::initialize(&env, &admin, CommonError::AlreadyInitialized);
    });
    env.as_contract(&id, || {
        admin::require_admin(&env, &other, CommonError::NotAdmin);
    });
}

#[test]
#[should_panic(expected = "Error(Contract, #1)")]
fn admin_cannot_be_reinitialized() {
    let (env, id) = harness();
    let admin = Address::generate(&env);
    env.as_contract(&id, || {
        admin::initialize(&env, &admin, CommonError::AlreadyInitialized);
        admin::initialize(&env, &admin, CommonError::AlreadyInitialized);
    });
}

#[test]
fn pause_round_trip_and_guard() {
    let (env, id) = harness();
    let admin = Address::generate(&env);

    env.as_contract(&id, || {
        admin::initialize(&env, &admin, CommonError::AlreadyInitialized);
        pause::initialize(&env);
        assert!(!pause::is_paused(&env));
        pause::check_not_paused(&env, CommonError::ContractPaused);
    });
    env.as_contract(&id, || {
        pause::pause(
            &env,
            &admin,
            CommonError::NotAdmin,
            CommonError::AlreadyPaused,
        );
    });
    env.as_contract(&id, || {
        assert!(pause::is_paused(&env));
        pause::unpause(&env, &admin, CommonError::NotAdmin, CommonError::NotPaused);
        assert!(!pause::is_paused(&env));
    });
}

#[test]
#[should_panic(expected = "Error(Contract, #4)")]
fn pause_guard_blocks_when_paused() {
    let (env, id) = harness();
    let admin = Address::generate(&env);

    env.as_contract(&id, || {
        admin::initialize(&env, &admin, CommonError::AlreadyInitialized);
        pause::initialize(&env);
        env.storage().instance().set(&CommonKey::Paused, &true);
    });
    env.as_contract(&id, || {
        pause::check_not_paused(&env, CommonError::ContractPaused);
    });
}

#[test]
fn two_step_admin_transfer_lifecycle() {
    let (env, id) = harness();
    let admin = Address::generate(&env);
    let candidate = Address::generate(&env);

    env.as_contract(&id, || {
        admin::initialize(&env, &admin, CommonError::AlreadyInitialized);
    });
    env.as_contract(&id, || {
        two_step::propose(&env, &admin, &candidate, CommonError::NotAdmin);
    });
    env.as_contract(&id, || {
        let proposal = two_step::pending(&env).unwrap();
        assert_eq!(proposal.candidate, candidate);
        assert_eq!(
            proposal.effective_at,
            two_step::DEFAULT_ADMIN_TRANSFER_TIMELOCK_SECS
        );
        assert_eq!(admin::get(&env), Some(admin.clone()));
        assert_eq!(
            two_step::accept(
                &env,
                &candidate,
                CommonError::NoPendingAdminTransfer,
                CommonError::InvalidAdminCandidate,
                CommonError::TimelockNotElapsed,
            ),
            Err(CommonError::TimelockNotElapsed)
        );
    });

    env.ledger()
        .set_timestamp(two_step::DEFAULT_ADMIN_TRANSFER_TIMELOCK_SECS);
    env.as_contract(&id, || {
        two_step::accept(
            &env,
            &candidate,
            CommonError::NoPendingAdminTransfer,
            CommonError::InvalidAdminCandidate,
            CommonError::TimelockNotElapsed,
        )
        .unwrap();
    });
    env.as_contract(&id, || {
        assert_eq!(admin::get(&env), Some(candidate));
        assert_eq!(two_step::pending(&env), None);
    });
}

#[test]
fn two_step_admin_transfer_cancel() {
    let (env, id) = harness();
    let admin = Address::generate(&env);
    let candidate = Address::generate(&env);

    env.as_contract(&id, || {
        admin::initialize(&env, &admin, CommonError::AlreadyInitialized);
    });
    env.as_contract(&id, || {
        two_step::propose(&env, &admin, &candidate, CommonError::NotAdmin);
    });
    env.as_contract(&id, || {
        two_step::cancel(
            &env,
            &admin,
            CommonError::NotAdmin,
            CommonError::NoPendingAdminTransfer,
        )
        .unwrap();
    });
    env.as_contract(&id, || {
        assert_eq!(two_step::pending(&env), None);
        assert_eq!(admin::get(&env), Some(admin));
    });
}

#[test]
fn ttl_helpers_round_trip() {
    let (env, id) = harness();
    env.as_contract(&id, || {
        let key = symbol_short!("k");
        assert_eq!(ttl::get_persistent::<_, u64>(&env, &key), None);
        assert!(!ttl::has_persistent(&env, &key));

        ttl::set_persistent(&env, &key, &42u64);
        assert!(ttl::has_persistent(&env, &key));
        assert_eq!(ttl::get_persistent::<_, u64>(&env, &key), Some(42));

        ttl::extend_instance(&env);
    });
}
