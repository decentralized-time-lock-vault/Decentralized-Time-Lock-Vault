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

use soroban_sdk::{symbol_short, Address, Env, Symbol};

pub fn deposit(env: &Env, depositor: &Address, token: &Address, deposit_id: u32, amount: i128, unlock_time: u64) {
    let topics = (symbol_short!("deposit"), depositor.clone(), token.clone());
    env.events().publish(topics, (deposit_id, amount, unlock_time));
}

pub fn withdraw(env: &Env, depositor: &Address, token: &Address, deposit_id: u32, amount: i128) {
    let topics = (symbol_short!("withdraw"), depositor.clone(), token.clone());
    env.events().publish(topics, (deposit_id, amount));
}

pub fn emergency_withdraw(
    env: &Env,
    admin: &Address,
    depositor: &Address,
    token: &Address,
    deposit_id: u32,
    amount: i128,
) {
    // admin is placed in the data payload rather than topics to avoid
    // leaking the admin address in the publicly-indexed event topic stream.
    let topics = (Symbol::new(env, "emrg_wdraw"), depositor.clone());
    env.events()
        .publish(topics, (deposit_id, admin.clone(), token.clone(), amount));
}

pub fn admin_transfer_initiated(env: &Env, current_admin: &Address, pending_admin: &Address) {
    let topics = (Symbol::new(env, "adm_xfr_init"), current_admin.clone());
    env.events().publish(topics, pending_admin.clone());
}

pub fn admin_transfer_cancelled(env: &Env, current_admin: &Address, pending_admin: &Address) {
    let topics = (Symbol::new(env, "adm_xfr_cancel"), current_admin.clone());
    env.events().publish(topics, pending_admin.clone());
}

pub fn admin_transfer_accepted(env: &Env, new_admin: &Address) {
    let topics = (Symbol::new(env, "adm_xfr_done"), new_admin.clone());
    env.events().publish(topics, ());
}

pub fn admin_renounced(env: &Env, former_admin: &Address) {
    let topics = (Symbol::new(env, "adm_renounce"), former_admin.clone());
    env.events().publish(topics, ());
}

pub fn lock_extended(
    env: &Env,
    depositor: &Address,
    old_unlock_time: u64,
    new_unlock_time: u64,
) {
    let topics = (Symbol::new(env, "lock_extended"), depositor.clone());
    env.events().publish(topics, (old_unlock_time, new_unlock_time));
}

pub fn paused(env: &Env, admin: &Address) {
    let topics = (Symbol::new(env, "paused"), admin.clone());
    env.events().publish(topics, ());
}

pub fn unpaused(env: &Env, admin: &Address) {
    let topics = (Symbol::new(env, "unpaused"), admin.clone());
    env.events().publish(topics, ());
}

pub fn withdraw_to(
    env: &Env,
    depositor: &Address,
    recipient: &Address,
    token: &Address,
    deposit_id: u32,
    amount: i128,
) {
    let topics = (
        Symbol::new(env, "withdraw_to"),
        depositor.clone(),
        recipient.clone(),
        token.clone(),
    );
    env.events().publish(topics, (deposit_id, amount));
}

pub fn deposit_cancelled(
    env: &Env,
    depositor: &Address,
    token: &Address,
    amount: i128,
    penalty: i128,
) {
    let topics = (
        Symbol::new(env, "dep_cancel"),
        depositor.clone(),
        token.clone(),
    );
    env.events().publish(topics, (amount, penalty));
}
