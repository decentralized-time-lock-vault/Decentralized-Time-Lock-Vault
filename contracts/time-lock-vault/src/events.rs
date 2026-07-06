//! Event emission helpers for the Time-Lock Vault contract.
//!
//! # Topic conventions
//!
//! Soroban events are published via `env.events().publish(topics, data)`.
//! Topics are indexed on-chain and should uniquely identify **what** happened
//! and **who** is the primary actor.  Data carries the additional detail that
//! is useful off-chain but does not need to be filterable.
//!
//! All topic tuples in this module follow the pattern:
//!
//! ```text
//! (event_name: Symbol, primary_actor: Address [, secondary_actor: Address …])
//! ```
//!
//! Rules applied consistently across every event:
//!
//! * The first topic is always a short [`Symbol`] (≤ 9 ASCII bytes so it fits
//!   in a `symbol_short!`) that names the event.
//! * Addresses that act as natural filter keys (depositor, token) are placed in
//!   **topics** so callers can subscribe to a specific depositor or token.
//! * Addresses that are incidental context (e.g. the admin in an emergency
//!   withdrawal) are placed in the **data** payload to keep the topic stream
//!   clean and avoid leaking privileged addresses into the indexed layer.
//! * Numeric values (`amount`, `unlock_time`, `deposit_id`) are always in the
//!   data payload.
//!
//! # Event reference
//!
//! | Function | Topics | Data |
//! |---|---|---|
//! | [`deposit`] | `("deposit", depositor, token)` | `(deposit_id, amount, unlock_time)` |
//! | [`withdraw`] | `("withdraw", depositor, token)` | `(deposit_id, amount)` |
//! | [`withdraw_to`] | `("withdraw_to", depositor, recipient, token)` | `(deposit_id, amount)` |
//! | [`emergency_withdraw`] | `("emrg_wdraw", depositor)` | `(deposit_id, admin, token, amount)` |
//! | [`deposit_cancelled`] | `("dep_cancel", depositor, token)` | `(amount, penalty)` |
//! | [`lock_extended`] | `("lock_extended", depositor)` | `(old_unlock_time, new_unlock_time)` |
//! | [`paused`] | `("paused", admin)` | `()` |
//! | [`unpaused`] | `("unpaused", admin)` | `()` |
//! | [`admin_transfer_initiated`] | `("adm_xfr_init", current_admin)` | `pending_admin` |
//! | [`admin_transfer_cancelled`] | `("adm_xfr_cancel", current_admin)` | `pending_admin` |
//! | [`admin_transfer_accepted`] | `("adm_xfr_done", new_admin)` | `()` |
//! | [`admin_renounced`] | `("adm_renounce", former_admin)` | `()` |
//! | [`depositor_frozen`] | `("dep_frozen", depositor)` | `()` |
//! | [`depositor_unfrozen`] | `("dep_unfrozen", depositor)` | `()` |

use soroban_sdk::{contractevent, Address, Env};

#[contractevent]
pub struct Deposit {
    #[topic]
    depositor: Address,
    #[topic]
    token: Address,
    deposit_id: u32,
    amount: i128,
    unlock_time: u64,
}

#[contractevent]
pub struct TopUp {
    #[topic]
    depositor: Address,
    #[topic]
    token: Address,
    deposit_id: u32,
    added: i128,
    new_total: i128,
}

#[contractevent]
pub struct Withdraw {
    #[topic]
    depositor: Address,
    #[topic]
    token: Address,
    deposit_id: u32,
    amount: i128,
}

#[contractevent]
pub struct WithdrawTo {
    #[topic]
    depositor: Address,
    #[topic]
    recipient: Address,
    #[topic]
    token: Address,
    deposit_id: u32,
    amount: i128,
}

#[contractevent(topics = ["emrg_wdraw"])]
pub struct EmergencyWithdraw {
    #[topic]
    depositor: Address,
    deposit_id: u32,
    admin: Address,
    token: Address,
    amount: i128,
}

#[contractevent(topics = ["dep_cancel"])]
pub struct DepositCancelled {
    #[topic]
    depositor: Address,
    #[topic]
    token: Address,
    amount: i128,
    penalty: i128,
}

#[contractevent(topics = ["paused"])]
pub struct Paused {
    #[topic]
    admin: Address,
}

#[contractevent(topics = ["unpaused"])]
pub struct Unpaused {
    #[topic]
    admin: Address,
}

#[contractevent(topics = ["adm_xfr_init"])]
pub struct AdminTransferInitiated {
    #[topic]
    current_admin: Address,
    pending_admin: Address,
}

#[contractevent(topics = ["adm_xfr_cancel"])]
pub struct AdminTransferCancelled {
    #[topic]
    current_admin: Address,
    pending_admin: Address,
}

#[contractevent(topics = ["adm_xfr_done"])]
pub struct AdminTransferAccepted {
    #[topic]
    new_admin: Address,
}

#[contractevent(topics = ["adm_renounce"])]
pub struct AdminRenounced {
    #[topic]
    former_admin: Address,
}

#[contractevent]
pub struct Migrated {
    #[topic]
    depositor: Address,
    deposit_id: u32,
    to_ledger: bool,
    to_time: bool,
}

#[contractevent]
pub struct LockExtended {
    #[topic]
    depositor: Address,
    old_unlock_time: u64,
    new_unlock_time: u64,
}

#[contractevent(topics = ["dep_frozen"])]
pub struct DepositorFrozen {
    #[topic]
    depositor: Address,
}

#[contractevent(topics = ["dep_unfrozen"])]
pub struct DepositorUnfrozen {
    #[topic]
    depositor: Address,
}

#[contractevent(topics = ["batch_wdraw"])]
pub struct BatchWithdraw {
    #[topic]
    depositor: Address,
    count: u32,
}

pub fn deposit(
    env: &Env,
    depositor: &Address,
    token: &Address,
    deposit_id: u32,
    amount: i128,
    unlock_time: u64,
) {
    Deposit {
        depositor: depositor.clone(),
        token: token.clone(),
        deposit_id,
        amount,
        unlock_time,
    }
    .publish(env);
}

pub fn top_up(
    env: &Env,
    depositor: &Address,
    deposit_id: u32,
    token: &Address,
    added: i128,
    new_total: i128,
) {
    TopUp {
        depositor: depositor.clone(),
        token: token.clone(),
        deposit_id,
        added,
        new_total,
    }
    .publish(env);
}

pub fn withdraw(env: &Env, depositor: &Address, token: &Address, deposit_id: u32, amount: i128) {
    Withdraw {
        depositor: depositor.clone(),
        token: token.clone(),
        deposit_id,
        amount,
    }
    .publish(env);
}

pub fn withdraw_to(
    env: &Env,
    depositor: &Address,
    recipient: &Address,
    token: &Address,
    deposit_id: u32,
    amount: i128,
) {
    WithdrawTo {
        depositor: depositor.clone(),
        recipient: recipient.clone(),
        token: token.clone(),
        deposit_id,
        amount,
    }
    .publish(env);
}

pub fn emergency_withdraw(
    env: &Env,
    admin: &Address,
    depositor: &Address,
    deposit_id: u32,
    token: &Address,
    amount: i128,
) {
    EmergencyWithdraw {
        depositor: depositor.clone(),
        deposit_id,
        admin: admin.clone(),
        token: token.clone(),
        amount,
    }
    .publish(env);
}

pub fn deposit_cancelled(
    env: &Env,
    depositor: &Address,
    token: &Address,
    amount: i128,
    penalty: i128,
) {
    DepositCancelled {
        depositor: depositor.clone(),
        token: token.clone(),
        amount,
        penalty,
    }
    .publish(env);
}

pub fn paused(env: &Env, admin: &Address) {
    Paused {
        admin: admin.clone(),
    }
    .publish(env);
}

pub fn unpaused(env: &Env, admin: &Address) {
    Unpaused {
        admin: admin.clone(),
    }
    .publish(env);
}

pub fn admin_transfer_initiated(env: &Env, current_admin: &Address, pending_admin: &Address) {
    AdminTransferInitiated {
        current_admin: current_admin.clone(),
        pending_admin: pending_admin.clone(),
    }
    .publish(env);
}

pub fn admin_transfer_cancelled(env: &Env, current_admin: &Address, pending_admin: &Address) {
    AdminTransferCancelled {
        current_admin: current_admin.clone(),
        pending_admin: pending_admin.clone(),
    }
    .publish(env);
}

pub fn admin_transfer_accepted(env: &Env, new_admin: &Address) {
    AdminTransferAccepted {
        new_admin: new_admin.clone(),
    }
    .publish(env);
}

pub fn admin_renounced(env: &Env, former_admin: &Address) {
    AdminRenounced {
        former_admin: former_admin.clone(),
    }
    .publish(env);
}

pub fn migrated(env: &Env, depositor: &Address, deposit_id: u32, to_ledger: bool, to_time: bool) {
    Migrated {
        depositor: depositor.clone(),
        deposit_id,
        to_ledger,
        to_time,
    }
    .publish(env);
}

pub fn lock_extended(env: &Env, depositor: &Address, old_unlock_time: u64, new_unlock_time: u64) {
    LockExtended {
        depositor: depositor.clone(),
        old_unlock_time,
        new_unlock_time,
    }
    .publish(env);
}

pub fn batch_withdraw(env: &Env, depositor: &Address, count: u32) {
    BatchWithdraw {
        depositor: depositor.clone(),
        count,
    }
    .publish(env);
}

pub fn depositor_frozen(env: &Env, depositor: &Address) {
    DepositorFrozen {
        depositor: depositor.clone(),
    }
    .publish(env);
}

pub fn depositor_unfrozen(env: &Env, depositor: &Address) {
    DepositorUnfrozen {
        depositor: depositor.clone(),
    }
    .publish(env);
}
