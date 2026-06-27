use soroban_sdk::{Address, Env, Vec};

use crate::constants::MAX_LOCK_DURATION_SECS;
use crate::errors::VaultError;
use crate::types::{LedgerVaultEntry, VaultEntry, VaultKey};

// Number of seconds per ledger — Soroban ledgers are ~5 seconds apart.
pub const LEDGER_SECONDS: u64 = 5;

// TTL constants: threshold ≈ 30 days, target ≈ 5.2 years.
pub const BUMP_THRESHOLD: u32 = 518_400;
pub const BUMP_TARGET: u32 = ((MAX_LOCK_DURATION_SECS + LEDGER_SECONDS - 1) / LEDGER_SECONDS) as u32;

// ----------------------------------------------------------------
//  TTL helper
// ----------------------------------------------------------------

fn extend_ttl(env: &Env, key: &VaultKey) {
    env.storage()
        .persistent()
        .extend_ttl(key, BUMP_THRESHOLD, BUMP_TARGET);
}

// ----------------------------------------------------------------
//  Deposit counter helpers
// ----------------------------------------------------------------

pub fn next_deposit_id(env: &Env, depositor: &Address) -> u32 {
    let key = VaultKey::DepositCounter(depositor.clone());
    let id: u32 = env.storage().persistent().get(&key).unwrap_or(0);
    let next = id.saturating_add(1);
    env.storage().persistent().set(&key, &next);
    extend_ttl(env, &key);
    id
}

// ----------------------------------------------------------------
//  Active deposit ID list (Issue #262 — O(k) not O(counter))
// ----------------------------------------------------------------

/// Returns the list of active deposit IDs for a depositor — O(k) where
/// k = active deposits, with no scan over the entire historical counter range.
pub fn get_deposit_ids(env: &Env, depositor: &Address) -> Vec<u32> {
    let key = VaultKey::ActiveDepositIds(depositor.clone());
    env.storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|| Vec::new(env))
}

fn add_active_deposit_id(env: &Env, depositor: &Address, deposit_id: u32) {
    let key = VaultKey::ActiveDepositIds(depositor.clone());
    let mut ids: Vec<u32> = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|| Vec::new(env));
    ids.push_back(deposit_id);
    env.storage().persistent().set(&key, &ids);
    env.storage()
        .persistent()
        .extend_ttl(&key, BUMP_THRESHOLD, BUMP_TARGET);
}

fn remove_active_deposit_id(env: &Env, depositor: &Address, deposit_id: u32) {
    let key = VaultKey::ActiveDepositIds(depositor.clone());
    let ids: Vec<u32> = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|| Vec::new(env));
    let mut new_ids: Vec<u32> = Vec::new(env);
    for id in ids.iter() {
        if id != deposit_id {
            new_ids.push_back(id);
        }
    }
    if new_ids.is_empty() {
        env.storage().persistent().remove(&key);
    } else {
        env.storage().persistent().set(&key, &new_ids);
        env.storage()
            .persistent()
            .extend_ttl(&key, BUMP_THRESHOLD, BUMP_TARGET);
    }
}

// ----------------------------------------------------------------
//  Active deposit counter (Issue #262 — O(1) has_any_deposit)
// ----------------------------------------------------------------

/// O(1) check: returns true if the depositor has at least one active deposit.
pub fn has_any_deposit(env: &Env, depositor: &Address) -> bool {
    let key = VaultKey::ActiveDepositCount(depositor.clone());
    let count: u32 = env.storage().persistent().get(&key).unwrap_or(0);
    count > 0
}

/// Increment the active-deposit counter for `depositor`.
pub fn inc_active_count(env: &Env, depositor: &Address) {
    let key = VaultKey::ActiveDepositCount(depositor.clone());
    let count: u32 = env.storage().persistent().get(&key).unwrap_or(0);
    env.storage()
        .persistent()
        .set(&key, &count.saturating_add(1));
    env.storage()
        .persistent()
        .extend_ttl(&key, BUMP_THRESHOLD, BUMP_TARGET);
}

/// Decrement the active-deposit counter for `depositor` (saturating at 0).
pub fn dec_active_count(env: &Env, depositor: &Address) {
    let key = VaultKey::ActiveDepositCount(depositor.clone());
    let count: u32 = env.storage().persistent().get(&key).unwrap_or(0);
    let new_count = count.saturating_sub(1);
    env.storage().persistent().set(&key, &new_count);
    env.storage()
        .persistent()
        .extend_ttl(&key, BUMP_THRESHOLD, BUMP_TARGET);
}

// ----------------------------------------------------------------
//  Deposit helpers
// ----------------------------------------------------------------

pub fn set_deposit(env: &Env, depositor: &Address, deposit_id: u32, entry: &VaultEntry) {
    let key = VaultKey::Deposit(depositor.clone(), deposit_id);
    env.storage().persistent().set(&key, entry);
    env.storage()
        .persistent()
        .extend_ttl(&key, BUMP_THRESHOLD, BUMP_TARGET);
    add_active_deposit_id(env, depositor, deposit_id);
    inc_active_count(env, depositor);
}

pub fn get_deposit(env: &Env, depositor: &Address, deposit_id: u32) -> Option<VaultEntry> {
    let key = VaultKey::Deposit(depositor.clone(), deposit_id);
    let entry: Option<VaultEntry> = env.storage().persistent().get(&key);
    if entry.is_some() {
        extend_ttl(env, &key);
    }
    entry
}

pub fn get_deposit_readonly(env: &Env, depositor: &Address, deposit_id: u32) -> Option<VaultEntry> {
    let key = VaultKey::Deposit(depositor.clone(), deposit_id);
    env.storage().persistent().get(&key)
}

pub fn remove_deposit(env: &Env, depositor: &Address, deposit_id: u32) {
    let key = VaultKey::Deposit(depositor.clone(), deposit_id);
    env.storage().persistent().remove(&key);
    remove_active_deposit_id(env, depositor, deposit_id);
    dec_active_count(env, depositor);
}

// ----------------------------------------------------------------
//  Ledger-based deposit helpers
// ----------------------------------------------------------------

pub fn set_deposit_by_ledger(
    env: &Env,
    depositor: &Address,
    deposit_id: u32,
    entry: &LedgerVaultEntry,
) {
    let key = VaultKey::DepositByLedger(depositor.clone(), deposit_id);
    env.storage().persistent().set(&key, entry);
    env.storage()
        .persistent()
        .extend_ttl(&key, BUMP_THRESHOLD, BUMP_TARGET);
    add_active_deposit_id(env, depositor, deposit_id);
    inc_active_count(env, depositor);
}

pub fn get_deposit_by_ledger_readonly(
    env: &Env,
    depositor: &Address,
    deposit_id: u32,
) -> Option<LedgerVaultEntry> {
    let key = VaultKey::DepositByLedger(depositor.clone(), deposit_id);
    env.storage().persistent().get(&key)
}

pub fn remove_deposit_by_ledger(env: &Env, depositor: &Address, deposit_id: u32) {
    let key = VaultKey::DepositByLedger(depositor.clone(), deposit_id);
    env.storage().persistent().remove(&key);
    remove_active_deposit_id(env, depositor, deposit_id);
    dec_active_count(env, depositor);
}

// ----------------------------------------------------------------
//  Admin helpers
// ----------------------------------------------------------------

pub fn set_admin(env: &Env, admin: &Address) {
    env.storage().persistent().set(&VaultKey::Admin, admin);
    extend_ttl(env, &VaultKey::Admin);
}

pub fn get_admin(env: &Env) -> Option<Address> {
    env.storage().persistent().get(&VaultKey::Admin)
}

/// Asserts that `caller` is the current admin; returns `Unauthorized` otherwise.
pub fn require_admin(env: &Env, caller: &Address) -> Result<(), VaultError> {
    match get_admin(env) {
        Some(admin) if admin == *caller => Ok(()),
        _ => Err(VaultError::Unauthorized),
    }
}

pub fn remove_admin(env: &Env) {
    env.storage().persistent().remove(&VaultKey::Admin);
}

pub fn set_pending_admin(env: &Env, pending: &Address) {
    env.storage()
        .persistent()
        .set(&VaultKey::PendingAdmin, pending);
    extend_ttl(env, &VaultKey::PendingAdmin);
}

pub fn get_pending_admin(env: &Env) -> Option<Address> {
    env.storage().persistent().get(&VaultKey::PendingAdmin)
}

pub fn remove_pending_admin(env: &Env) {
    env.storage().persistent().remove(&VaultKey::PendingAdmin);
}

// ----------------------------------------------------------------
//  Initialized flag
// ----------------------------------------------------------------

pub fn set_initialized(env: &Env) {
    env.storage()
        .persistent()
        .set(&VaultKey::Initialized, &true);
    extend_ttl(env, &VaultKey::Initialized);
}

pub fn is_initialized(env: &Env) -> bool {
    env.storage()
        .persistent()
        .get::<VaultKey, bool>(&VaultKey::Initialized)
        .unwrap_or(false)
}

// ----------------------------------------------------------------
//  Runtime limits helpers
// ----------------------------------------------------------------

pub fn set_max_deposit(env: &Env, v: i128) {
    env.storage().persistent().set(&VaultKey::MaxDeposit, &v);
    env.storage()
        .persistent()
        .extend_ttl(&VaultKey::MaxDeposit, BUMP_THRESHOLD, BUMP_TARGET);
}

pub fn get_max_deposit(env: &Env) -> Option<i128> {
    env.storage().persistent().get(&VaultKey::MaxDeposit)
}

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

pub fn set_fee_recipient(env: &Env, recipient: &Address) {
    env.storage()
        .persistent()
        .set(&VaultKey::FeeRecipient, recipient);
    extend_ttl(env, &VaultKey::FeeRecipient);
}

pub fn get_fee_recipient(env: &Env) -> Option<Address> {
    env.storage().persistent().get(&VaultKey::FeeRecipient)
}

// ----------------------------------------------------------------
//  Depositor index helpers — O(1) add / O(1) remove via swap-remove
//  (Issues #260 and #261)
// ----------------------------------------------------------------

fn get_depositor_count_raw(env: &Env) -> u32 {
    env.storage()
        .persistent()
        .get(&VaultKey::DepositorCount)
        .unwrap_or(0)
}

fn set_depositor_count(env: &Env, count: u32) {
    env.storage()
        .persistent()
        .set(&VaultKey::DepositorCount, &count);
    env.storage()
        .persistent()
        .extend_ttl(&VaultKey::DepositorCount, BUMP_THRESHOLD, BUMP_TARGET);
}

fn get_depositor_at(env: &Env, slot: u32) -> Address {
    env.storage()
        .persistent()
        .get(&VaultKey::DepositorAt(slot))
        .unwrap()
}

fn set_depositor_at(env: &Env, slot: u32, addr: &Address) {
    let key = VaultKey::DepositorAt(slot);
    env.storage().persistent().set(&key, addr);
    env.storage()
        .persistent()
        .extend_ttl(&key, BUMP_THRESHOLD, BUMP_TARGET);
}

fn remove_depositor_at(env: &Env, slot: u32) {
    env.storage()
        .persistent()
        .remove(&VaultKey::DepositorAt(slot));
}

fn get_depositor_slot(env: &Env, addr: &Address) -> Option<u32> {
    env.storage()
        .persistent()
        .get(&VaultKey::DepositorIndex(addr.clone()))
}

fn set_depositor_slot(env: &Env, addr: &Address, slot: u32) {
    let key = VaultKey::DepositorIndex(addr.clone());
    env.storage().persistent().set(&key, &slot);
    env.storage()
        .persistent()
        .extend_ttl(&key, BUMP_THRESHOLD, BUMP_TARGET);
}

fn remove_depositor_slot(env: &Env, addr: &Address) {
    env.storage()
        .persistent()
        .remove(&VaultKey::DepositorIndex(addr.clone()));
}

/// O(1) add: uses `DepositorMember` flag for duplicate check, then appends to
/// the slot array. No list scan. (Fixes issue #260.)
pub fn add_depositor(env: &Env, depositor: &Address) {
    let member_key = VaultKey::DepositorMember(depositor.clone());
    // O(1) membership check — no full-list scan
    if env.storage().persistent().has(&member_key) {
        return;
    }
    env.storage().persistent().set(&member_key, &true);
    env.storage()
        .persistent()
        .extend_ttl(&member_key, BUMP_THRESHOLD, BUMP_TARGET);

    let slot = get_depositor_count_raw(env);
    set_depositor_at(env, slot, depositor);
    set_depositor_slot(env, depositor, slot);
    set_depositor_count(env, slot.saturating_add(1));
}

/// O(1) swap-remove: moves the last element into the vacated slot.
/// (Fixes issue #261.)
pub fn remove_depositor(env: &Env, depositor: &Address) {
    let count = get_depositor_count_raw(env);
    if count == 0 {
        return;
    }
    let slot = match get_depositor_slot(env, depositor) {
        Some(s) => s,
        None => return,
    };
    let last = count.saturating_sub(1);
    if slot != last {
        // Move last element into the freed slot
        let last_addr = get_depositor_at(env, last);
        set_depositor_at(env, slot, &last_addr);
        set_depositor_slot(env, &last_addr, slot);
    }
    remove_depositor_at(env, last);
    remove_depositor_slot(env, depositor);

    // Remove membership flag
    env.storage()
        .persistent()
        .remove(&VaultKey::DepositorMember(depositor.clone()));

    set_depositor_count(env, last);
}

pub fn get_depositor_count(env: &Env) -> u32 {
    get_depositor_count_raw(env)
}

pub fn get_depositors_page(env: &Env, offset: u32, limit: u32) -> Vec<Address> {
    let count = get_depositor_count_raw(env);
    let mut page: Vec<Address> = Vec::new(env);
    let end = (offset.saturating_add(limit)).min(count);
    for i in offset..end {
        page.push_back(get_depositor_at(env, i));
    }
    page
}

// ----------------------------------------------------------------
//  Paused flag helpers
// ----------------------------------------------------------------

pub fn set_paused(env: &Env, paused: bool) {
    env.storage().persistent().set(&VaultKey::Paused, &paused);
    env.storage()
        .persistent()
        .extend_ttl(&VaultKey::Paused, BUMP_THRESHOLD, BUMP_TARGET);
}

pub fn is_paused(env: &Env) -> bool {
    env.storage()
        .persistent()
        .get::<VaultKey, bool>(&VaultKey::Paused)
        .unwrap_or(false)
}
