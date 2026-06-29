//! # Persistent Storage Layout
//!
//! All entries use **Persistent Storage** with the TTL constants defined below.
//! Every *write* path calls `extend_ttl`; *read-only* helpers intentionally
//! skip the TTL bump to avoid charging callers extra fees.
//!
//! ## Key Space
//!
//! | `VaultKey` variant | Value type | Written by | Removed by | Notes |
//! |--------------------|-----------|------------|------------|-------|
//! | `Admin` | `Address` | `initialize`, `accept_admin` | `renounce_admin` | Absent after renounce → trustless mode |
//! | `PendingAdmin` | `Address` | `transfer_admin` | `accept_admin`, `cancel_transfer_admin`, `renounce_admin` | Absent when no transfer is in progress |
//! | `Initialized` | `bool` | `initialize` | never | Guards against re-initialization |
//! | `FeeRecipient` | `Address` | `initialize` | never | Receives penalty fees from `cancel_deposit` |
//! | `MaxDeposit` | `i128` | `initialize` (optional) | never | Absent → use compile-time `MAX_DEPOSIT_AMOUNT` |
//! | `MaxLockSecs` | `u64` | `initialize` (optional) | never | Absent → use compile-time `MAX_LOCK_DURATION_SECS` |
//! | `Paused` | `bool` | `pause`, `unpause` | never | Absent → treated as `false` (not paused) |
//! | `DepositorList` | `Vec<Address>` | `deposit`, `deposit_by_ledger` | entry removed when depositor's last deposit is withdrawn | Linear scan on read/write — see scalability note below |
//! | `DepositCounter(depositor)` | `u32` | every `deposit` / `deposit_by_ledger` call | never decremented | Monotonically increasing per-depositor sequence |
//! | `Deposit(depositor, id)` | `VaultEntry` | `deposit`, `deposit_for` | `withdraw`, `cancel_deposit`, `emergency_withdraw` | Timestamp-based lock; `id` comes from `DepositCounter` |
//! | `DepositByLedger(depositor, id)` | `LedgerVaultEntry` | `deposit_by_ledger` | `withdraw`, `emergency_withdraw` | Ledger-sequence-based lock; same `id` counter as above |
//!
//! ## TTL Policy
//!
//! - `BUMP_THRESHOLD` (`518_400` ledgers ≈ 30 days): TTL extension is triggered
//!   only when the remaining TTL falls below this value, avoiding redundant
//!   ledger writes on every call.
//! - `BUMP_TARGET` (≈ 5.2 years in ledgers): Derived from `MAX_LOCK_DURATION_SECS`
//!   divided by `LEDGER_SECONDS` (5 s/ledger). Guarantees that a max-duration
//!   deposit cannot expire before its unlock time without a manual TTL extension.
//!
//! ## Scalability Notes
//!
//! - **`get_deposit_ids`** performs a linear scan over `[0, DepositCounter)`,
//!   checking whether each `VaultKey::Deposit` key exists. Cost grows with the
//!   number of deposits a single address has ever made (not just active ones).
//!   Keep per-depositor deposit counts small for predictable instruction costs.
//! - **`DepositorList`** is a single `Vec<Address>` entry that is read and
//!   re-written on every `deposit` and `remove_depositor` call. For large
//!   numbers of concurrent depositors this entry grows proportionally and
//!   increases serialization cost. Use `get_depositors_page` for paginated
//!   off-chain reads rather than fetching the full list in a transaction.

use soroban_sdk::{Address, Env, Vec};

use crate::constants::MAX_LOCK_DURATION_SECS;
use crate::types::{LedgerVaultEntry, VaultEntry, VaultKey};

pub const LEDGER_SECONDS: u64 = 5;
pub const BUMP_THRESHOLD: u32 = 518_400;

/// Target TTL (in ledgers) applied by every `extend_ttl` call.
/// Derived from `MAX_LOCK_DURATION_SECS` so a max-duration deposit can never
/// expire before its unlock time. Approximately 5.2 years at 5 s/ledger.
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
//  Deposit counter / ID tracking
// ----------------------------------------------------------------

/// Returns the next deposit ID for `depositor` (starting at 0) and increments
/// the persisted counter. The counter is monotonically increasing and is never
/// decremented — withdrawn IDs are never reused.
///
/// **TTL:** bumped on every call (write path).
pub fn next_deposit_id(env: &Env, depositor: &Address) -> u32 {
    let key = VaultKey::DepositCounter(depositor.clone());
    let id: u32 = env.storage().persistent().get(&key).unwrap_or(0);
    env.storage().persistent().set(&key, &(id + 1));
    extend_ttl(env, &key);
    id
}

/// Returns all active timestamp-based deposit IDs for `depositor`.
///
/// Performs a linear scan over `[0, DepositCounter)`, checking `has` for each
/// `VaultKey::Deposit`. Instruction cost is proportional to the total number of
/// deposits ever made by this address, not just active ones. This function does
/// **not** include ledger-based deposit IDs (`VaultKey::DepositByLedger`).
///
/// **TTL:** not bumped (read-only path).
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
    extend_ttl(env, &key);
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
        extend_ttl(env, &key);
    }
}

fn inc_active_count(env: &Env, depositor: &Address) {
    let key = VaultKey::ActiveDepositCount(depositor.clone());
    let count: u32 = env.storage().persistent().get(&key).unwrap_or(0);
    env.storage().persistent().set(&key, &(count + 1));
    env.storage()
        .persistent()
        .extend_ttl(&key, BUMP_THRESHOLD, BUMP_TARGET);
}

fn dec_active_count(env: &Env, depositor: &Address) {
    let key = VaultKey::ActiveDepositCount(depositor.clone());
    let count: u32 = env.storage().persistent().get(&key).unwrap_or(0);
    env.storage()
        .persistent()
        .set(&key, &count.saturating_sub(1));
    env.storage()
        .persistent()
        .extend_ttl(&key, BUMP_THRESHOLD, BUMP_TARGET);
}

/// Returns true if `depositor` has at least one active deposit in either store.
pub fn has_any_deposit(env: &Env, depositor: &Address) -> bool {
    let key = VaultKey::ActiveDepositCount(depositor.clone());
    let count: u32 = env.storage().persistent().get(&key).unwrap_or(0);
    let new_count = count.saturating_sub(1);
    env.storage().persistent().set(&key, &new_count);
    env.storage()
        .persistent()
        .extend_ttl(&key, BUMP_THRESHOLD, BUMP_TARGET);
}

pub fn get_deposit_by_ledger_ids(env: &Env, depositor: &Address) -> Vec<u32> {
    let counter_key = VaultKey::DepositCounter(depositor.clone());
    let count: u32 = env.storage().persistent().get(&counter_key).unwrap_or(0);
    let mut ids = Vec::new(env);
    for id in 0..count {
        let key = VaultKey::DepositByLedger(depositor.clone(), id);
        if env.storage().persistent().has(&key) {
            ids.push_back(id);
        }
    }
    ids
}

/// Returns ledger-based deposit IDs with a limit to avoid unbounded scans.
/// This is a bounded alternative to get_deposit_by_ledger_ids for query operations.
pub fn get_deposit_by_ledger_ids_limited(env: &Env, depositor: &Address, max_results: u32) -> Vec<u32> {
    let counter_key = VaultKey::DepositCounter(depositor.clone());
    let count: u32 = env.storage().persistent().get(&counter_key).unwrap_or(0);
    let mut ids = Vec::new(env);
    let limit = count.min(max_results);
    for id in 0..limit {
        let key = VaultKey::DepositByLedger(depositor.clone(), id);
        if env.storage().persistent().has(&key) {
            ids.push_back(id);
            if ids.len() >= max_results {
                break;
            }
        }
    }
    ids
}

pub fn get_all_deposit_ids(env: &Env, depositor: &Address) -> Vec<u32> {
    let counter_key = VaultKey::DepositCounter(depositor.clone());
    let count: u32 = env.storage().persistent().get(&counter_key).unwrap_or(0);
    let mut ids = Vec::new(env);
    for id in 0..count {
        let key_ts = VaultKey::Deposit(depositor.clone(), id);
        let key_ledger = VaultKey::DepositByLedger(depositor.clone(), id);
        if env.storage().persistent().has(&key_ts) || env.storage().persistent().has(&key_ledger) {
            ids.push_back(id);
        }
    }
    ids
}

/// Returns all deposit IDs with a limit to avoid unbounded scans.
/// This is a bounded alternative to get_all_deposit_ids for query operations.
pub fn get_all_deposit_ids_limited(env: &Env, depositor: &Address, max_results: u32) -> Vec<u32> {
    let counter_key = VaultKey::DepositCounter(depositor.clone());
    let count: u32 = env.storage().persistent().get(&counter_key).unwrap_or(0);
    let mut ids = Vec::new(env);
    let limit = count.min(max_results);
    for id in 0..limit {
        let key_ts = VaultKey::Deposit(depositor.clone(), id);
        let key_ledger = VaultKey::DepositByLedger(depositor.clone(), id);
        if env.storage().persistent().has(&key_ts) || env.storage().persistent().has(&key_ledger) {
            ids.push_back(id);
            if ids.len() >= max_results {
                break;
            }
        }
    }
    ids
}

// ----------------------------------------------------------------
//  Deposit helpers  (timestamp-based: VaultKey::Deposit)
// ----------------------------------------------------------------

/// Persists a timestamp-based `VaultEntry` and bumps TTL.
/// Key: `VaultKey::Deposit(depositor, deposit_id)`.
pub fn set_deposit(env: &Env, depositor: &Address, deposit_id: u32, entry: &VaultEntry) {
    let key = VaultKey::Deposit(depositor.clone(), deposit_id);
    env.storage().persistent().set(&key, entry);
    extend_ttl(env, &key);
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

/// Retrieves a timestamp-based deposit **without** bumping TTL.
/// Used by view functions (`get_vault`, `time_remaining`) and by write paths
/// that will call `remove_deposit` immediately after — no TTL bump needed in
/// either case, keeping caller fees minimal.
pub fn get_deposit_readonly(env: &Env, depositor: &Address, deposit_id: u32) -> Option<VaultEntry> {
    env.storage()
        .persistent()
        .get(&VaultKey::Deposit(depositor.clone(), deposit_id))
}

/// Removes a timestamp-based deposit entry from persistent storage.
/// Called as the *effect* step of the CEI pattern before any token transfer.
pub fn remove_deposit(env: &Env, depositor: &Address, deposit_id: u32) {
    env.storage()
        .persistent()
        .remove(&VaultKey::Deposit(depositor.clone(), deposit_id));
    dec_active_count(env, depositor);
}

// ----------------------------------------------------------------
//  Ledger-based deposit helpers  (VaultKey::DepositByLedger)
//
//  These mirror the timestamp-based helpers above but use a separate key
//  variant so the two deposit types are unambiguously distinct in storage.
//  The same per-depositor `DepositCounter` sequence is shared, so a given
//  `deposit_id` belongs to exactly one type for a given depositor.
// ----------------------------------------------------------------

/// Persists a ledger-sequence-based `LedgerVaultEntry` and bumps TTL.
/// Key: `VaultKey::DepositByLedger(depositor, deposit_id)`.
pub fn set_deposit_by_ledger(env: &Env, depositor: &Address, deposit_id: u32, entry: &LedgerVaultEntry) {
    let key = VaultKey::DepositByLedger(depositor.clone(), deposit_id);
    env.storage().persistent().set(&key, entry);
    extend_ttl(env, &key);
    inc_active_count(env, depositor);
}

/// Retrieves a ledger-based deposit **without** bumping TTL.
/// Used by `withdraw` and `emergency_withdraw` immediately before removal.
pub fn get_deposit_by_ledger_readonly(env: &Env, depositor: &Address, deposit_id: u32) -> Option<LedgerVaultEntry> {
    env.storage()
        .persistent()
        .get(&VaultKey::DepositByLedger(depositor.clone(), deposit_id))
}

/// Removes a ledger-based deposit entry from persistent storage.
/// Called as the *effect* step of the CEI pattern before any token transfer.
pub fn remove_deposit_by_ledger(env: &Env, depositor: &Address, deposit_id: u32) {
    env.storage()
        .persistent()
        .remove(&VaultKey::DepositByLedger(depositor.clone(), deposit_id));
    dec_active_count(env, depositor);
}

// ----------------------------------------------------------------
//  Depositor index helpers (for paginated queries)
// ----------------------------------------------------------------

/// Load the full depositor index. Returns an empty Vec if not yet set.
pub fn get_depositor_index(env: &Env) -> Vec<Address> {
    env.storage()
        .persistent()
        .get(&VaultKey::DepositorIndex)
        .unwrap_or_else(|| Vec::new(env))
}

/// Append `depositor` to the index (called on first deposit).
pub fn add_to_depositor_index(env: &Env, depositor: &Address) {
    let mut list = get_depositor_index(env);
    list.push_back(depositor.clone());
    env.storage()
        .persistent()
        .set(&VaultKey::DepositorIndex, &list);
    env.storage()
        .persistent()
        .extend_ttl(&VaultKey::DepositorIndex, BUMP_THRESHOLD, BUMP_TARGET);
}

/// Remove `depositor` from the index (called on withdrawal/emergency_withdraw).
pub fn remove_from_depositor_index(env: &Env, depositor: &Address) {
    let list = get_depositor_index(env);
    let mut new_list: Vec<Address> = Vec::new(env);
    for addr in list.iter() {
        if &addr != depositor {
            new_list.push_back(addr);
        }
    }
    env.storage()
        .persistent()
        .set(&VaultKey::DepositorIndex, &new_list);
    env.storage()
        .persistent()
        .extend_ttl(&VaultKey::DepositorIndex, BUMP_THRESHOLD, BUMP_TARGET);
}

// ----------------------------------------------------------------
//  Admin helpers
// ----------------------------------------------------------------

/// Persists the admin address and bumps TTL.
/// Key: `VaultKey::Admin`. Overwritten by `accept_admin`; removed by `renounce_admin`.
pub fn set_admin(env: &Env, admin: &Address) {
    env.storage().persistent().set(&VaultKey::Admin, admin);
    extend_ttl(env, &VaultKey::Admin);
}

pub fn get_admin(env: &Env) -> Option<Address> {
    env.storage().persistent().get(&VaultKey::Admin)
}

pub fn remove_admin(env: &Env) {
    env.storage().persistent().remove(&VaultKey::Admin);
}

/// Sets the pending admin during a two-step transfer. Key: `VaultKey::PendingAdmin`.
pub fn set_pending_admin(env: &Env, pending: &Address) {
    env.storage()
        .persistent()
        .set(&VaultKey::PendingAdmin, pending);
    extend_ttl(env, &VaultKey::PendingAdmin);
}

/// Returns the pending admin address, or `None` when no transfer is in progress.
pub fn get_pending_admin(env: &Env) -> Option<Address> {
    env.storage().persistent().get(&VaultKey::PendingAdmin)
}

/// Clears the pending admin key. Called by `accept_admin`, `cancel_transfer_admin`,
/// and `renounce_admin`.
pub fn remove_pending_admin(env: &Env) {
    env.storage().persistent().remove(&VaultKey::PendingAdmin);
}

// ----------------------------------------------------------------
//  Initialized flag
// ----------------------------------------------------------------

/// Marks the contract as initialized. Written once by `initialize`; never removed.
/// Guards against re-initialization by a different caller.
pub fn set_initialized(env: &Env) {
    env.storage()
        .persistent()
        .set(&VaultKey::Initialized, &true);
    extend_ttl(env, &VaultKey::Initialized);
}

/// Returns `true` if `initialize` has been called, `false` otherwise.
/// Absent key is treated as `false`.
pub fn is_initialized(env: &Env) -> bool {
    env.storage()
        .persistent()
        .get::<VaultKey, bool>(&VaultKey::Initialized)
        .unwrap_or(false)
}

// ----------------------------------------------------------------
//  Runtime limit helpers
// ----------------------------------------------------------------

/// Persists a deployment-specific maximum deposit amount.
/// Key: `VaultKey::MaxDeposit`. Absent → callers fall back to `MAX_DEPOSIT_AMOUNT`.
pub fn set_max_deposit(env: &Env, v: i128) {
    env.storage().persistent().set(&VaultKey::MaxDeposit, &v);
    extend_ttl(env, &VaultKey::MaxDeposit);
}

/// Returns the runtime max deposit, or `None` to use the compile-time default.
pub fn get_max_deposit(env: &Env) -> Option<i128> {
    env.storage().persistent().get(&VaultKey::MaxDeposit)
}

/// Persists a deployment-specific maximum lock duration in seconds.
/// Key: `VaultKey::MaxLockSecs`. Absent → callers fall back to `MAX_LOCK_DURATION_SECS`.
pub fn set_max_lock_secs(env: &Env, v: u64) {
    env.storage().persistent().set(&VaultKey::MaxLockSecs, &v);
    extend_ttl(env, &VaultKey::MaxLockSecs);
}

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
//  Depositor index helpers
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
    extend_ttl(env, &VaultKey::DepositorCount);
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
        .set(&VaultKey::DepositorAt(slot), addr);
    extend_ttl(env, &VaultKey::DepositorAt(slot));
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
        .set(&VaultKey::DepositorIndex(addr.clone()), &slot);
    extend_ttl(env, &VaultKey::DepositorIndex(addr.clone()));
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
    if env.storage().persistent().has(&member_key) {
        return;
    }
    env.storage().persistent().set(&member_key, &true);
    env.storage()
        .persistent()
        .extend_ttl(&member_key, BUMP_THRESHOLD, BUMP_TARGET);

    let count = get_depositor_count_raw(env);
    set_depositor_at(env, count, depositor);
    set_depositor_slot(env, depositor, count);
    set_depositor_count(env, count + 1);
}

/// Removes `depositor` from the global depositor list.
/// Called when the depositor's last active deposit is withdrawn or cancelled.
pub fn remove_depositor(env: &Env, depositor: &Address) {
    // Guard: don't remove if deposits remain in either store.
    if has_any_deposit(env, depositor) {
        return;
    }

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

    let member_key = VaultKey::DepositorMember(depositor.clone());
    env.storage().persistent().remove(&member_key);
}

/// Returns the total number of addresses with at least one active deposit.
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
//  Pause state helpers
// ----------------------------------------------------------------

/// Sets the contract pause state. Key: `VaultKey::Paused`.
/// When `true`, `deposit` and `deposit_for` reject new deposits with `ContractPaused`.
pub fn set_paused(env: &Env, paused: bool) {
    env.storage()
        .persistent()
        .set(&VaultKey::Paused, &paused);
    env.storage()
        .persistent()
        .extend_ttl(&VaultKey::Paused, BUMP_THRESHOLD, BUMP_TARGET);
}

/// Returns `true` if the contract is paused. Absent key treated as `false`.
pub fn is_paused(env: &Env) -> bool {
    env.storage()
        .persistent()
        .get::<VaultKey, bool>(&VaultKey::Paused)
        .unwrap_or(false)
}

// ----------------------------------------------------------------
//  Freeze helpers
// ----------------------------------------------------------------

pub fn set_frozen(env: &Env, depositor: &Address) {
    let key = VaultKey::DepositorFrozen(depositor.clone());
    env.storage().persistent().set(&key, &true);
    env.storage()
        .persistent()
        .extend_ttl(&key, BUMP_THRESHOLD, BUMP_TARGET);
}

pub fn is_frozen(env: &Env, depositor: &Address) -> bool {
    env.storage()
        .persistent()
        .get::<VaultKey, bool>(&VaultKey::DepositorFrozen(depositor.clone()))
        .unwrap_or(false)
}

pub fn remove_frozen(env: &Env, depositor: &Address) {
    env.storage()
        .persistent()
        .remove(&VaultKey::DepositorFrozen(depositor.clone()));
}

// ----------------------------------------------------------------
//  Token freeze helpers
// ----------------------------------------------------------------

pub fn set_token_frozen(env: &Env, token: &Address) {
    let key = VaultKey::TokenFrozen(token.clone());
    env.storage().persistent().set(&key, &true);
    env.storage()
        .persistent()
        .extend_ttl(&key, BUMP_THRESHOLD, BUMP_TARGET);
}

pub fn is_token_frozen(env: &Env, token: &Address) -> bool {
    env.storage()
        .persistent()
        .get::<VaultKey, bool>(&VaultKey::TokenFrozen(token.clone()))
        .unwrap_or(false)
}

pub fn remove_token_frozen(env: &Env, token: &Address) {
    env.storage()
        .persistent()
        .remove(&VaultKey::TokenFrozen(token.clone()));
}

// ----------------------------------------------------------------
//  Penalty cap / fee rule helpers
// ----------------------------------------------------------------

pub fn set_max_penalty_bps(env: &Env, bps: u32) {
    env.storage()
        .persistent()
        .set(&VaultKey::MaxPenaltyBps, &bps);
    extend_ttl(env, &VaultKey::MaxPenaltyBps);
}

pub fn get_max_penalty_bps(env: &Env) -> Option<u32> {
    env.storage().persistent().get(&VaultKey::MaxPenaltyBps)
}

pub fn set_min_cancel_fee(env: &Env, fee: i128) {
    env.storage()
        .persistent()
        .set(&VaultKey::MinCancelFee, &fee);
    extend_ttl(env, &VaultKey::MinCancelFee);
}

pub fn get_min_cancel_fee(env: &Env) -> Option<i128> {
    env.storage().persistent().get(&VaultKey::MinCancelFee)
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
