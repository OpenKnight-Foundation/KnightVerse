//! Two-step, timelocked admin transfer.
//!
//! The current admin proposes a candidate; the candidate must authenticate and
//! accept after the timelock elapses. The current admin can cancel at any time
//! before acceptance. Proposal, acceptance and cancellation are all paused-safe
//! (they never call `pause::check_not_paused`), so control can be recovered
//! while an incident is ongoing.

use crate::{admin, events, CommonKey};
use soroban_sdk::{contracttype, panic_with_error, Address, Env, Error};

/// Default delay between proposing and accepting an admin handoff (48 hours).
pub const DEFAULT_ADMIN_TRANSFER_TIMELOCK_SECS: u64 = 48 * 60 * 60;

/// A proposed admin handoff stored under [`CommonKey::PendingAdmin`].
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingAdmin {
    /// Address that will become admin on acceptance.
    pub candidate: Address,
    /// Ledger timestamp (seconds) at which acceptance becomes possible.
    pub effective_at: u64,
}

/// The pending handoff, if any.
pub fn pending(env: &Env) -> Option<PendingAdmin> {
    env.storage().instance().get(&CommonKey::PendingAdmin)
}

/// Propose a handoff with the default timelock. Overwrites any prior proposal.
pub fn propose<E>(env: &Env, current_admin: &Address, candidate: &Address, not_admin: E)
where
    E: Into<Error>,
{
    propose_with_timelock(
        env,
        current_admin,
        candidate,
        DEFAULT_ADMIN_TRANSFER_TIMELOCK_SECS,
        not_admin,
    );
}

/// Propose a handoff with a custom timelock. Overwrites any prior proposal.
pub fn propose_with_timelock<E>(
    env: &Env,
    current_admin: &Address,
    candidate: &Address,
    timelock_secs: u64,
    not_admin: E,
) where
    E: Into<Error>,
{
    admin::require_admin(env, current_admin, not_admin);
    let effective_at = env.ledger().timestamp().saturating_add(timelock_secs);
    env.storage().instance().set(
        &CommonKey::PendingAdmin,
        &PendingAdmin {
            candidate: candidate.clone(),
            effective_at,
        },
    );
    events::admin_transfer_proposed(env, candidate, effective_at);
}

/// Complete a pending handoff. The candidate must authenticate, match the
/// proposal and wait for the timelock.
pub fn accept<E>(
    env: &Env,
    candidate: &Address,
    no_pending: E,
    wrong_candidate: E,
    timelock_not_elapsed: E,
) -> Result<(), E>
where
    E: Into<Error>,
{
    candidate.require_auth();
    let proposal = pending(env).ok_or(no_pending)?;
    if proposal.candidate != *candidate {
        panic_with_error!(env, wrong_candidate);
    }
    if env.ledger().timestamp() < proposal.effective_at {
        return Err(timelock_not_elapsed);
    }

    admin::set(env, candidate);
    env.storage().instance().remove(&CommonKey::PendingAdmin);
    events::admin_transfer_accepted(env, candidate);
    Ok(())
}

/// Cancel a pending handoff. Only the current admin may do so.
pub fn cancel<E>(env: &Env, current_admin: &Address, not_admin: E, no_pending: E) -> Result<(), E>
where
    E: Into<Error>,
{
    admin::require_admin(env, current_admin, not_admin);
    if pending(env).is_none() {
        return Err(no_pending);
    }
    env.storage().instance().remove(&CommonKey::PendingAdmin);
    events::admin_transfer_cancelled(env, current_admin);
    Ok(())
}
