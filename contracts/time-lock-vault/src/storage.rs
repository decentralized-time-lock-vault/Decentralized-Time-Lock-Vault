use soroban_sdk::{Address, Env, Vec};

use crate::errors::VaultError;
use crate::types::{VaultEntry, VaultKey};

// ----------------------------------------------------------------
//  Persistent storage TTL constants
// ----------------------------------------------------------------

/// Minimum remaining TTL (in ledgers) before a bump is applied (≈ 30 days at 5s/ledger).
pub const BUMP_THRESHOLD: u32 = 518_400;

/// Target TTL after a bump (≈ 5.2 years at 5s/ledger).
/// Must exceed MAX_LOCK_DURATION_SECS in ledger units so a max-duration deposit
/// cannot expire before its unlock time.
pub const BUMP_TARGET: u32 = 33_000_000;

// ----------------------------------------------------------------
//  Deposit count helpers (Instance storage)
// ----------------------------------------------------------------

pub fn increment_deposit_count(env: &Env) {
    let count: u64 = env.storage().instance().get(&VaultKey::DepositCount).unwrap_or(0);
    env.storage().instance().set(&VaultKey::DepositCount, &(count + 1));
}

pub fn decrement_deposit_count(env: &Env) {
    let count: u64 = env.storage().instance().get(&VaultKey::DepositCount).unwrap_or(0);
    env.storage().instance().set(&VaultKey::DepositCount, &count.saturating_sub(1));
}

pub fn get_deposit_count(env: &Env) -> u64 {
    env.storage().instance().get(&VaultKey::DepositCount).unwrap_or(0)
}

// ----------------------------------------------------------------
//  Deposit helpers
// ----------------------------------------------------------------

/// Persists `entry` under `VaultKey::Deposit(depositor)` and bumps its TTL.
/// Also increments the global deposit count.
pub fn set_deposit(env: &Env, depositor: &Address, entry: &VaultEntry) {
    let key = VaultKey::Deposit(depositor.clone());
    env.storage().persistent().set(&key, entry);
    env.storage()
        .persistent()
        .extend_ttl(&key, BUMP_THRESHOLD, BUMP_TARGET);
    increment_deposit_count(env);
}

/// Loads a deposit entry and bumps its TTL if found. Use for mutating call paths.
pub fn get_deposit(env: &Env, depositor: &Address) -> Option<VaultEntry> {
    let key = VaultKey::Deposit(depositor.clone());
    let entry: Option<VaultEntry> = env.storage().persistent().get(&key);
    if entry.is_some() {
        env.storage()
            .persistent()
            .extend_ttl(&key, BUMP_THRESHOLD, BUMP_TARGET);
    }
    entry
}

/// Loads a deposit entry without bumping TTL. Use for read-only queries to avoid extra fees.
pub fn get_deposit_readonly(env: &Env, depositor: &Address) -> Option<VaultEntry> {
    let key = VaultKey::Deposit(depositor.clone());
    env.storage().persistent().get(&key)
}

/// Removes the deposit entry from persistent storage and decrements the global deposit count.
pub fn remove_deposit(env: &Env, depositor: &Address) {
    env.storage()
        .persistent()
        .remove(&VaultKey::Deposit(depositor.clone()));
    decrement_deposit_count(env);
}

// ----------------------------------------------------------------
//  Admin helpers
// ----------------------------------------------------------------

/// Persists `admin` under `VaultKey::Admin`.
pub fn set_admin(env: &Env, admin: &Address) {
    env.storage().instance().set(&VaultKey::Admin, admin);
}

/// Returns the current admin address, or `None` if admin has been renounced.
pub fn get_admin(env: &Env) -> Option<Address> {
    env.storage().instance().get(&VaultKey::Admin)
}

/// Stores the nominated `pending` admin during a two-step transfer.
pub fn set_pending_admin(env: &Env, pending: &Address) {
    env.storage().instance().set(&VaultKey::PendingAdmin, pending);
}

/// Returns the pending admin address, or `None` if no transfer is in progress.
pub fn get_pending_admin(env: &Env) -> Option<Address> {
    env.storage().instance().get(&VaultKey::PendingAdmin)
}

/// Clears the pending admin entry (called on accept or cancel of a transfer).
pub fn remove_pending_admin(env: &Env) {
    env.storage().instance().remove(&VaultKey::PendingAdmin);
}

/// Removes the admin entry (called when admin renounces).
pub fn remove_admin(env: &Env) {
    env.storage().instance().remove(&VaultKey::Admin);
}

pub fn require_admin(env: &Env, caller: &Address) -> Result<(), VaultError> {
    let stored = get_admin(env).ok_or(VaultError::Unauthorized)?;
    if caller != &stored {
        return Err(VaultError::Unauthorized);
    }
    Ok(())
}

// ----------------------------------------------------------------
//  Initialized flag
// ----------------------------------------------------------------

/// Marks the contract as initialized. Called once during `initialize`.
pub fn set_initialized(env: &Env) {
    env.storage()
        .persistent()
        .set(&VaultKey::Initialized, &true);
    env.storage()
        .persistent()
        .extend_ttl(&VaultKey::Initialized, BUMP_THRESHOLD, BUMP_TARGET);
}

/// Returns `true` if `initialize` has already been called.
pub fn is_initialized(env: &Env) -> bool {
    env.storage()
        .persistent()
        .get::<VaultKey, bool>(&VaultKey::Initialized)
        .unwrap_or(false)
}

// ----------------------------------------------------------------
//  Runtime limits helpers
// ----------------------------------------------------------------

/// Persists a runtime override for the maximum deposit amount and bumps TTL.
pub fn set_max_deposit(env: &Env, v: i128) {
    env.storage().persistent().set(&VaultKey::MaxDeposit, &v);
    env.storage()
        .persistent()
        .extend_ttl(&VaultKey::MaxDeposit, BUMP_THRESHOLD, BUMP_TARGET);
}

/// Returns the runtime-configured max deposit amount, or `None` to use the compile-time default.
pub fn get_max_deposit(env: &Env) -> Option<i128> {
    env.storage().persistent().get(&VaultKey::MaxDeposit)
}

/// Persists a runtime override for the maximum lock duration (seconds) and bumps TTL.
pub fn set_max_lock_secs(env: &Env, v: u64) {
    env.storage().persistent().set(&VaultKey::MaxLockSecs, &v);
    env.storage()
        .persistent()
        .extend_ttl(&VaultKey::MaxLockSecs, BUMP_THRESHOLD, BUMP_TARGET);
}

/// Returns the runtime-configured max lock duration, or `None` to use the compile-time default.
pub fn get_max_lock_secs(env: &Env) -> Option<u64> {
    env.storage().persistent().get(&VaultKey::MaxLockSecs)
}

// ----------------------------------------------------------------
//  Fee recipient helpers
// ----------------------------------------------------------------

/// Persists the `fee_recipient` address and bumps TTL. Called once during `initialize`.
pub fn set_fee_recipient(env: &Env, recipient: &Address) {
    env.storage()
        .persistent()
        .set(&VaultKey::FeeRecipient, recipient);
    env.storage()
        .persistent()
        .extend_ttl(&VaultKey::FeeRecipient, BUMP_THRESHOLD, BUMP_TARGET);
}

/// Returns the fee recipient address, or `None` if not set.
pub fn get_fee_recipient(env: &Env) -> Option<Address> {
    env.storage().persistent().get(&VaultKey::FeeRecipient)
}

// ----------------------------------------------------------------
//  Depositor list helpers
// ----------------------------------------------------------------

fn get_depositor_list(env: &Env) -> Vec<Address> {
    env.storage()
        .persistent()
        .get(&VaultKey::DepositorList)
        .unwrap_or_else(|| Vec::new(env))
}

fn save_depositor_list(env: &Env, list: &Vec<Address>) {
    env.storage()
        .persistent()
        .set(&VaultKey::DepositorList, list);
    env.storage()
        .persistent()
        .extend_ttl(&VaultKey::DepositorList, BUMP_THRESHOLD, BUMP_TARGET);
}

pub fn add_depositor(env: &Env, depositor: &Address) {
    let mut list = get_depositor_list(env);
    list.push_back(depositor.clone());
    save_depositor_list(env, &list);
}

pub fn remove_depositor(env: &Env, depositor: &Address) {
    let list = get_depositor_list(env);
    let mut new_list: Vec<Address> = Vec::new(env);
    for addr in list.iter() {
        if &addr != depositor {
            new_list.push_back(addr);
        }
    }
    save_depositor_list(env, &new_list);
}

pub fn get_depositor_count(env: &Env) -> u32 {
    get_depositor_list(env).len()
}

pub fn get_depositors_page(env: &Env, offset: u32, limit: u32) -> Vec<Address> {
    let list = get_depositor_list(env);
    let len = list.len();
    let mut page: Vec<Address> = Vec::new(env);
    let end = (offset + limit).min(len);
    for i in offset..end {
        page.push_back(list.get(i).unwrap());
    }
    page
}
