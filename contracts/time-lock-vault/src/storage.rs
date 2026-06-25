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

use crate::types::{VaultEntry, VaultKey, LedgerVaultEntry, MAX_LOCK_DURATION_SECS};

// Number of seconds per ledger — Soroban ledgers are ~5 seconds apart.
pub const LEDGER_SECONDS: u64 = 5;

/// Minimum remaining TTL (in ledgers) below which `extend_ttl` is triggered.
/// Equivalent to ~30 days at 5 s/ledger. Avoids redundant writes when TTL is healthy.
pub const BUMP_THRESHOLD: u32 = 518_400;

/// Target TTL (in ledgers) applied by every `extend_ttl` call.
/// Derived from `MAX_LOCK_DURATION_SECS` so a max-duration deposit can never
/// expire before its unlock time. Approximately 5.2 years at 5 s/ledger.
pub const BUMP_TARGET: u32 = ((MAX_LOCK_DURATION_SECS + LEDGER_SECONDS - 1) / LEDGER_SECONDS) as u32;

// ----------------------------------------------------------------
//  Deposit counter helpers
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
    env.storage()
        .persistent()
        .extend_ttl(&key, BUMP_THRESHOLD, BUMP_TARGET);
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
    let counter_key = VaultKey::DepositCounter(depositor.clone());
    let count: u32 = env.storage().persistent().get(&counter_key).unwrap_or(0);
    let mut ids = Vec::new(env);
    for id in 0..count {
        let key = VaultKey::Deposit(depositor.clone(), id);
        if env.storage().persistent().has(&key) {
            ids.push_back(id);
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
    env.storage()
        .persistent()
        .extend_ttl(&key, BUMP_THRESHOLD, BUMP_TARGET);
}

/// Retrieves a timestamp-based deposit and bumps TTL on hit.
/// Use this variant when the caller's transaction itself constitutes a
/// "keep-alive" access that should refresh the entry's lifetime.
pub fn get_deposit(env: &Env, depositor: &Address, deposit_id: u32) -> Option<VaultEntry> {
    let key = VaultKey::Deposit(depositor.clone(), deposit_id);
    let entry: Option<VaultEntry> = env.storage().persistent().get(&key);
    if entry.is_some() {
        env.storage()
            .persistent()
            .extend_ttl(&key, BUMP_THRESHOLD, BUMP_TARGET);
    }
    entry
}

/// Retrieves a timestamp-based deposit **without** bumping TTL.
/// Used by view functions (`get_vault`, `time_remaining`) and by write paths
/// that will call `remove_deposit` immediately after — no TTL bump needed in
/// either case, keeping caller fees minimal.
pub fn get_deposit_readonly(env: &Env, depositor: &Address, deposit_id: u32) -> Option<VaultEntry> {
    let key = VaultKey::Deposit(depositor.clone(), deposit_id);
    env.storage().persistent().get(&key)
}

/// Removes a timestamp-based deposit entry from persistent storage.
/// Called as the *effect* step of the CEI pattern before any token transfer.
pub fn remove_deposit(env: &Env, depositor: &Address, deposit_id: u32) {
    let key = VaultKey::Deposit(depositor.clone(), deposit_id);
    env.storage().persistent().remove(&key);
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
    env.storage()
        .persistent()
        .extend_ttl(&key, BUMP_THRESHOLD, BUMP_TARGET);
}

/// Retrieves a ledger-based deposit **without** bumping TTL.
/// Used by `withdraw` and `emergency_withdraw` immediately before removal.
pub fn get_deposit_by_ledger_readonly(env: &Env, depositor: &Address, deposit_id: u32) -> Option<LedgerVaultEntry> {
    let key = VaultKey::DepositByLedger(depositor.clone(), deposit_id);
    env.storage().persistent().get(&key)
}

/// Removes a ledger-based deposit entry from persistent storage.
/// Called as the *effect* step of the CEI pattern before any token transfer.
pub fn remove_deposit_by_ledger(env: &Env, depositor: &Address, deposit_id: u32) {
    let key = VaultKey::DepositByLedger(depositor.clone(), deposit_id);
    env.storage().persistent().remove(&key);
}

// ----------------------------------------------------------------
//  Admin helpers
// ----------------------------------------------------------------

/// Persists the admin address and bumps TTL.
/// Key: `VaultKey::Admin`. Overwritten by `accept_admin`; removed by `renounce_admin`.
pub fn set_admin(env: &Env, admin: &Address) {
    env.storage().persistent().set(&VaultKey::Admin, admin);
    env.storage()
        .persistent()
        .extend_ttl(&VaultKey::Admin, BUMP_THRESHOLD, BUMP_TARGET);
}

/// Returns the current admin address, or `None` if admin has been renounced.
pub fn get_admin(env: &Env) -> Option<Address> {
    env.storage().persistent().get(&VaultKey::Admin)
}

/// Removes the admin key permanently. Called by `renounce_admin` to enter trustless mode.
pub fn remove_admin(env: &Env) {
    env.storage().persistent().remove(&VaultKey::Admin);
}

/// Sets the pending admin during a two-step transfer. Key: `VaultKey::PendingAdmin`.
pub fn set_pending_admin(env: &Env, pending: &Address) {
    env.storage()
        .persistent()
        .set(&VaultKey::PendingAdmin, pending);
    env.storage()
        .persistent()
        .extend_ttl(&VaultKey::PendingAdmin, BUMP_THRESHOLD, BUMP_TARGET);
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
    env.storage()
        .persistent()
        .extend_ttl(&VaultKey::Initialized, BUMP_THRESHOLD, BUMP_TARGET);
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
//  Runtime limits helpers
// ----------------------------------------------------------------

/// Persists a deployment-specific maximum deposit amount.
/// Key: `VaultKey::MaxDeposit`. Absent → callers fall back to `MAX_DEPOSIT_AMOUNT`.
pub fn set_max_deposit(env: &Env, v: i128) {
    env.storage().persistent().set(&VaultKey::MaxDeposit, &v);
    env.storage()
        .persistent()
        .extend_ttl(&VaultKey::MaxDeposit, BUMP_THRESHOLD, BUMP_TARGET);
}

/// Returns the runtime max deposit, or `None` to use the compile-time default.
pub fn get_max_deposit(env: &Env) -> Option<i128> {
    env.storage().persistent().get(&VaultKey::MaxDeposit)
}

/// Persists a deployment-specific maximum lock duration in seconds.
/// Key: `VaultKey::MaxLockSecs`. Absent → callers fall back to `MAX_LOCK_DURATION_SECS`.
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

pub fn get_fee_recipient(env: &Env) -> Option<Address> {
    env.storage().persistent().get(&VaultKey::FeeRecipient)
}

// ----------------------------------------------------------------
//  Depositor list helpers
//
//  A single `VaultKey::DepositorList` entry holds a `Vec<Address>` of every
//  address that currently has at least one active deposit. The list is updated
//  on every deposit (add) and when a depositor's last deposit is removed.
//
//  Scalability: reads and writes deserialize/serialize the entire vector.
//  For contracts with many concurrent depositors this entry grows large.
//  Off-chain consumers should use `get_depositors_page` to paginate rather
//  than reading the full list in a single transaction.
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

/// Adds `depositor` to the global depositor list if not already present.
/// No-op if the address is already in the list (deduplication via linear scan).
pub fn add_depositor(env: &Env, depositor: &Address) {
    let mut list = get_depositor_list(env);
    for addr in list.iter() {
        if &addr == depositor {
            return;
        }
    }
    list.push_back(depositor.clone());
    save_depositor_list(env, &list);
}

/// Removes `depositor` from the global depositor list.
/// Called when the depositor's last active deposit is withdrawn or cancelled.
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

/// Returns the total number of addresses with at least one active deposit.
pub fn get_depositor_count(env: &Env) -> u32 {
    get_depositor_list(env).len()
}

// ----------------------------------------------------------------
//  Paused flag helpers
// ----------------------------------------------------------------

/// Sets the contract pause state. Key: `VaultKey::Paused`.
/// When `true`, `deposit` and `deposit_for` reject new deposits with `ContractPaused`.
pub fn set_paused(env: &Env, paused: bool) {
    env.storage().persistent().set(&VaultKey::Paused, &paused);
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

/// Returns a paginated slice of active depositor addresses.
///
/// - `offset`: zero-based start index into the `DepositorList`.
/// - `limit`: maximum number of addresses to return.
///
/// Returns an empty `Vec` when `offset >= list.len()`. Safe to call with any
/// `offset`/`limit` combination — never panics on out-of-range values.
///
/// **TTL:** not bumped (read-only path).
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
