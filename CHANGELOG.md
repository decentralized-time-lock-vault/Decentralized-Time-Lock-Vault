# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- `deposit_by_ledger(depositor, token, amount, unlock_ledger, penalty_bps)` - ledger-sequence-based deposit variant
- `deposit_for(payer, depositor, token, amount, unlock_time, penalty_bps)` - third-party funded deposits
- `withdraw_to(depositor, deposit_id, recipient)` - withdraw unlocked funds to an arbitrary address
- `get_vault_by_ledger(depositor, deposit_id)` - read-only query for ledger-based entries
- `get_vault_batch(depositors, deposit_id)` - batch query for multiple depositors
- `get_deposit_ids(depositor)` - list all active deposit IDs per depositor
- `ledgers_remaining(depositor, deposit_id)` - ledgers until ledger-based deposit unlocks
- `pause(admin)` / `unpause(admin)` - admin-controlled pause/unpause of new deposits
- `is_paused()` / `is_initialized()` - read-only query functions
- `batch_emergency_withdraw(admin, depositors)` - batch emergency withdraw for contract migrations
- `cancel_transfer_admin(admin)` - cancel a pending admin transfer
- Multiple deposits per address (O(1) active deposit ID tracking via `ActiveDepositIds`)
- Slot-based O(1) depositor index with `DepositorMember`, `DepositorCount`, `DepositorIndex`, `DepositorAt`
- `VaultKey::DepositByLedger`, `VaultKey::Paused` storage keys
- Events: `withdraw_to`, `dep_cancel`, `adm_xfr_cancel`, `lock_extended`, `paused`, `unpaused`
- Error codes: `ContractPaused` (12), `FundsAlreadyUnlocked` (13), `BatchTooLarge` (14)

### Changed
- `initialize` now requires `fee_recipient: Address` as second parameter
- `deposit` and `withdraw` now take a `deposit_id` parameter (per-depositor sequence number)
- Storage layout uses `ActiveDepositIds` instead of per-depositor counter scan for O(1) ID queries
- `has_any_deposit` uses `ActiveDepositIds` list check (O(1)) instead of unused counter
- `add_depositor` uses slot-based index matching `remove_depositor`'s swap-remove pattern
- View functions are explicitly read-only (no TTL bump, no state mutation)
- `emergency_withdraw` correctly returns `NoDepositFound` when neither deposit type exists
- `withdraw_to` properly handles both time-based and ledger-based deposits with token transfers

### Fixed
- `withdraw_to` function had broken control flow with duplicated/overwritten remove logic and missing token transfers
- `emergency_withdraw` referenced out-of-scope `entry` variable in dead code path
- `initialize` had a type-error `if let Some(r) = fee_recipient` on a non-optional `Address`
- Duplicate `require_admin` definition in storage.rs removed
- Duplicate function definitions for `withdraw_to`, `paused`, `unpaused` in events.rs removed
- `add_depositor` called non-existent `get_depositor_list`/`save_depositor_list` functions
- `has_any_deposit` used `ActiveDepositCount` key that was never written to
- Missing `DepositorCount`, `DepositorIndex`, `DepositorAt` variants in `VaultKey` enum
- Removed unused `DepositorList` and `ActiveDepositCount` enum variants
- Removed dead `inc_active_count` / `dec_active_count` functions

## [0.1.0] - 2026-05-31

### Added

#### Contract Functions

**Initialization**
- `initialize(admin, fee_recipient, max_deposit, max_lock_secs)`  sets the admin and fee recipient addresses; optionally overrides compile-time limits for `MAX_DEPOSIT_AMOUNT` and `MAX_LOCK_DURATION_SECS`; can only be called once

**Core**
- `deposit(depositor, token, amount, unlock_time, penalty_bps)`  locks tokens until `unlock_time` (Unix seconds derived from the ledger clock via `env.ledger().timestamp()`); returns a per-depositor `deposit_id`
- `deposit_for(payer, depositor, token, amount, unlock_time, penalty_bps)`  same as `deposit` but a third-party `payer` funds the vault on behalf of `depositor`; the `payer` must sign the transaction and the depositor's address is stored as the beneficiary
- `withdraw(depositor, deposit_id)`  returns the full locked amount to the depositor once `unlock_time` has passed
- `cancel_deposit(depositor, deposit_id)`  early exit before unlock; applies `penalty_bps` penalty sent to `fee_recipient`, remainder returned to depositor

**Admin**
- `emergency_withdraw(admin, depositor, deposit_id)`  admin-only; returns funds to the depositor regardless of lock time; funds always go to the depositor, never to the admin
- `transfer_admin(admin, new_admin)`  step 1 of two-step admin transfer; nominates a pending admin
- `accept_admin(new_admin)`  step 2; pending admin accepts and becomes the active admin
- `cancel_transfer_admin(admin)`  cancels a pending admin transfer
- `renounce_admin(admin)`  permanently removes admin privileges; makes the vault fully trustless

**Read-only Queries**
- `get_vault(depositor, deposit_id) -> Option<VaultEntry>`  returns the vault entry without bumping TTL
- `get_deposit_ids(depositor) -> Vec<u32>`  returns all active deposit IDs for a depositor
- `time_remaining(depositor, deposit_id) -> u64`  seconds until unlock; `0` if unlocked or not found
- `get_time() -> u64`  current ledger timestamp
- `get_admin() -> Option<Address>`  current admin, or `None` if renounced
- `get_pending_admin() -> Option<Address>`  pending admin during a transfer, or `None`
- `get_fee_recipient() -> Option<Address>`  fee recipient set at initialization
- `get_constants() -> (i128, u64)`  effective `(MAX_DEPOSIT_AMOUNT, MAX_LOCK_DURATION_SECS)` for this deployment
- `get_depositor_count() -> u32`  total number of addresses with an active deposit
- `get_depositors(offset, limit) -> Vec<Address>`  paginated list of active depositor addresses
- `is_initialized() -> bool`  whether `initialize` has been called

#### Ledger-Based Deposit Timing

Unlock times are validated and enforced using the **Soroban ledger clock** (`env.ledger().timestamp()`), not wall-clock time. Key implications:

- `unlock_time` must be supplied as a Unix timestamp (seconds since the Unix epoch).
- The contract reads `env.ledger().timestamp()` once per invocation and caches the value locally to avoid repeated host-function calls.
- A deposit is accepted only when `unlock_time > now` (strictly future).
- A `withdraw` succeeds only when `env.ledger().timestamp() >= unlock_time`.
- Ledger close times on Stellar advance roughly every 5–6 seconds. For short lock durations, callers should account for this granularity when choosing `unlock_time`.

Example — depositing with a 1-hour lock:
```
let now: u64 = env.ledger().timestamp(); // e.g. 1_700_000_000
let one_hour = 3_600_u64;
contract.deposit(&depositor, &token, &amount, &(now + one_hour), &0_u32);
```

The `deposit_for` function follows the same ledger-time semantics:
```
// payer funds the vault; depositor is the beneficiary
contract.deposit_for(&payer, &depositor, &token, &amount, &(now + one_hour), &0_u32);
```

#### Protocol Constants

- `MAX_DEPOSIT_AMOUNT`  `1_000_000_000_000_000` (10^15 token base units)
- `MAX_LOCK_DURATION_SECS`  `157_788_000` (~5 years)
- `MIN_LOCK_DURATION_SECS`  `60` (1 minute)

#### Error Codes

| Code | Name | Meaning |
|------|------|---------|
| 1 | `InvalidAmount` | Amount <= 0 |
| 2 | `UnlockTimeNotInFuture` | `unlock_time` <= current ledger time |
| 3 | `NoDepositFound` | No active deposit for this depositor/id |
| 4 | `FundsStillLocked` | Lock period not yet expired |
| 5 | `DepositAlreadyExists` | Reserved error code |
| 6 | `LockDurationTooLong` | Lock period exceeds `MAX_LOCK_DURATION_SECS` |
| 7 | `Unauthorized` | Caller is not the admin or pending admin |
| 8 | `AmountTooLarge` | Amount exceeds `MAX_DEPOSIT_AMOUNT` |
| 9 | `InvalidPenaltyBps` | `penalty_bps` > 10 000 |
| 10 | `InvalidAdmin` | Nominated admin is the same as the current admin |
| 11 | `LockDurationTooShort` | Lock period is shorter than `MIN_LOCK_DURATION_SECS` |

#### Events

| Event | Topics | Data |
|-------|--------|------|
| `deposit` | `("deposit", depositor, token)` | `(deposit_id, amount, unlock_time)` |
| `withdraw` | `("withdraw", depositor, token)` | `(deposit_id, amount)` |
| `emrg_wdraw` | `("emrg_wdraw", depositor)` | `(deposit_id, admin, token, amount)` |
| `dep_cancel` | `("dep_cancel", depositor, token)` | `(amount, penalty)` |
| `adm_xfr_init` | `("adm_xfr_init", current_admin)` | `pending_admin` |
| `adm_xfr_done` | `("adm_xfr_done", new_admin)` | `()` |
| `adm_renounce` | `("adm_renounce", former_admin)` | `()` |

#### Security Properties

- Checks-Effects-Interactions pattern enforced on every withdrawal path  storage cleared before any token transfer
- `require_auth()` is the first statement in every mutating function
- No re-entrancy surface  state removed before external token calls
- Bounded inputs  amount capped at 10^15; lock duration capped at 5 years with a 60-second minimum
- Emergency withdraw always sends funds to the depositor, never to the admin
- Two-step admin transfer prevents accidental key loss
- Admin can permanently renounce privileges for fully trustless operation
- Persistent storage entries bumped to ~5.2 years on every write; read-only queries skip TTL bump to avoid charging callers
- `features = ["testutils"]` only in `[dev-dependencies]`  testutils never compiled into production WASM

#### Storage

- All entries use Persistent Storage with TTL bump threshold ~30 days (`518_400` ledgers) and target ~5.2 years (`33_000_000` ledgers)
- Storage keys: `Admin`, `PendingAdmin`, `Initialized`, `FeeRecipient`, `MaxDeposit`, `MaxLockSecs`, `DepositCounter(depositor)`, `Deposit(depositor, id)`, `DepositorList`

#### Infrastructure

- Workspace Cargo setup with `soroban-sdk v22`
- Makefile targets: `build`, `test`, `check`, `optimize`, `check-wasm-size`, `audit`, `deny`, `deploy-testnet`, `smoke-test-local`
- CI workflow (GitHub Actions): lint -> test -> build WASM -> check WASM size
- Testnet deploy script with atomic deploy+initialize to prevent front-running
- Local smoke test script against a Soroban standalone node
- `rust-toolchain.toml` pinning stable Rust with `wasm32-unknown-unknown` target
- `deny.toml` for license allowlist and dependency ban policy
- 48+ unit tests covering all functions, error codes, and boundary conditions

[Unreleased]: https://github.com/your-org/time-lock-vault/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/your-org/time-lock-vault/releases/tag/v0.1.0
