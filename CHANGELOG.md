# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-05-31

### Added

#### Contract Functions

**Initialization**
- `initialize(admin, fee_recipient, max_deposit, max_lock_secs)` — sets the admin and fee recipient addresses; optionally overrides compile-time limits for `MAX_DEPOSIT_AMOUNT` and `MAX_LOCK_DURATION_SECS`; can only be called once

**Core**
- `deposit(depositor, token, amount, unlock_time, penalty_bps)` — locks tokens until `unlock_time` (Unix seconds); returns a per-depositor `deposit_id`; fails with `ContractPaused` if the contract is paused
- `deposit_for(payer, depositor, token, amount, unlock_time, penalty_bps)` — third-party `payer` funds a vault on behalf of `depositor`; same validation as `deposit`; fails with `ContractPaused` if the contract is paused
- `deposit_by_ledger(depositor, token, amount, unlock_ledger, penalty_bps)` — locks tokens until ledger sequence `unlock_ledger` is reached; immune to timestamp manipulation; does **not** check the pause state
- `withdraw(depositor, deposit_id)` — returns the full locked amount to the depositor once the unlock condition is met; transparently handles both timestamp-based and ledger-based deposits
- `withdraw_to(depositor, deposit_id, recipient)` — same as `withdraw` but routes funds to a specified `recipient`; timestamp-based deposits only
- `cancel_deposit(depositor, deposit_id)` — early exit before unlock; applies `penalty_bps` penalty sent to `fee_recipient`, remainder returned to depositor; timestamp-based only

**Pause / Unpause**
- `pause(admin)` — admin-only; blocks new `deposit` and `deposit_for` calls with `ContractPaused`; withdrawals and queries remain operational
- `unpause(admin)` — admin-only; clears the paused state, re-enabling deposits
- `is_paused() -> bool` — returns the current pause state

**Admin**
- `emergency_withdraw(admin, depositor, deposit_id)` — admin-only; returns funds to the depositor regardless of lock time; timestamp-based deposits only; funds always go to the depositor, never to the admin
- `transfer_admin(admin, new_admin)` — step 1 of two-step admin transfer; nominates a pending admin
- `accept_admin(new_admin)` — step 2; pending admin accepts and becomes the active admin
- `cancel_transfer_admin(admin)` — cancels a pending admin transfer; emits `adm_xfr_cancel` event
- `renounce_admin(admin)` — permanently removes admin privileges; makes the vault fully trustless

**Read-only Queries**
- `get_vault(depositor, deposit_id) -> Option<VaultEntry>` — returns the timestamp-based vault entry without bumping TTL
- `get_vault_batch(depositors, deposit_id) -> Vec<Option<VaultEntry>>` — batch version of `get_vault`; up to `MAX_BATCH_SIZE` (20) depositors per call
- `get_deposit_ids(depositor) -> Vec<u32>` — returns all active deposit IDs for a depositor (both timestamp and ledger-based)
- `time_remaining(depositor, deposit_id) -> u64` — seconds until unlock; `0` if unlocked or not found
- `get_time() -> u64` — current ledger timestamp
- `get_admin() -> Option<Address>` — current admin, or `None` if renounced
- `get_pending_admin() -> Option<Address>` — pending admin during a transfer, or `None`
- `get_fee_recipient() -> Option<Address>` — fee recipient set at initialization
- `get_constants() -> (i128, u64)` — effective `(MAX_DEPOSIT_AMOUNT, MAX_LOCK_DURATION_SECS)` for this deployment
- `get_depositor_count() -> u32` — total number of addresses with an active deposit
- `get_depositors(offset, limit) -> Vec<Address>` — paginated list of active depositor addresses
- `is_initialized() -> bool` — whether `initialize` has been called

#### Protocol Constants

- `MAX_DEPOSIT_AMOUNT` — `1_000_000_000_000_000` (10^15 token base units)
- `MAX_LOCK_DURATION_SECS` — `157_788_000` (~5 years)
- `MIN_LOCK_DURATION_SECS` — `60` (1 minute)
- `MAX_BATCH_SIZE` — `20` (max depositors per `get_vault_batch` call)

#### Error Codes

| Code | Name | Meaning |
|------|------|---------|
| 1 | `InvalidAmount` | Amount <= 0 |
| 2 | `UnlockTimeNotInFuture` | `unlock_time` <= current ledger time, or `unlock_ledger` <= current sequence |
| 3 | `NoDepositFound` | No active deposit for this depositor/id |
| 4 | `FundsStillLocked` | Lock period not yet expired |
| 5 | `DepositAlreadyExists` | Reserved error code |
| 6 | `LockDurationTooLong` | Lock period exceeds `MAX_LOCK_DURATION_SECS` |
| 7 | `Unauthorized` | Caller is not the admin or pending admin |
| 8 | `AmountTooLarge` | Amount exceeds `MAX_DEPOSIT_AMOUNT` |
| 9 | `InvalidPenaltyBps` | `penalty_bps` > 10 000 |
| 10 | `InvalidAdmin` | Nominated admin is the same as the current admin |
| 11 | `LockDurationTooShort` | Lock period is shorter than `MIN_LOCK_DURATION_SECS` (60 s) |
| 12 | `ContractPaused` | Contract is paused; `deposit` and `deposit_for` are blocked |

#### Events

| Event | Topics | Data |
|-------|--------|------|
| `deposit` | `("deposit", depositor, token)` | `(amount, unlock_time)` |
| `withdraw` | `("withdraw", depositor, token)` | `amount` |
| `wdraw_to` | `("wdraw_to", depositor, token)` | `(recipient, amount)` |
| `emrg_wdraw` | `("emrg_wdraw", depositor)` | `(admin, token, amount)` |
| `dep_cancel` | `("dep_cancel", depositor, token)` | `(amount, penalty)` |
| `paused` | `("paused", admin)` | `()` |
| `unpaused` | `("unpaused", admin)` | `()` |
| `adm_xfr_init` | `("adm_xfr_init", current_admin)` | `pending_admin` |
| `adm_xfr_cancel` | `("adm_xfr_cancel", current_admin)` | `pending_admin` |
| `adm_xfr_done` | `("adm_xfr_done", new_admin)` | `()` |
| `adm_renounce` | `("adm_renounce", former_admin)` | `()` |

#### Security Properties

- Checks-Effects-Interactions pattern enforced on every withdrawal path — storage cleared before any token transfer
- `require_auth()` is the first statement in every mutating function
- No re-entrancy surface — state removed before external token calls
- Bounded inputs — amount capped at 10^15; lock duration capped at 5 years with a 60-second minimum
- Emergency withdraw always sends funds to the depositor, never to the admin
- Pause circuit-breaker allows blocking new deposits without affecting locked funds or withdrawals
- Two-step admin transfer prevents accidental key loss
- Admin can permanently renounce privileges for fully trustless operation
- Persistent storage entries bumped to ~5.2 years on every write; read-only queries skip TTL bump to avoid charging callers
- `features = ["testutils"]` only in `[dev-dependencies]` — testutils never compiled into production WASM

#### Storage

- All entries use Persistent Storage with TTL bump threshold ~30 days (`518_400` ledgers) and target ~5.2 years
- Storage keys: `Admin`, `PendingAdmin`, `Initialized`, `Paused`, `FeeRecipient`, `MaxDeposit`, `MaxLockSecs`, `DepositCounter(depositor)`, `Deposit(depositor, id)`, `DepositByLedger(depositor, id)`, `DepositorList`

#### Infrastructure

- Workspace Cargo setup with `soroban-sdk v22`
- Makefile targets: `build`, `test`, `check`, `optimize`, `check-wasm-size`, `audit`, `deny`, `deploy-testnet`, `smoke-test-local`
- CI workflow (GitHub Actions): lint → test → build WASM → check WASM size
- Testnet deploy script with atomic deploy+initialize to prevent front-running
- Local smoke test script against a Soroban standalone node
- `rust-toolchain.toml` pinning stable Rust with `wasm32-unknown-unknown` target
- `deny.toml` for license allowlist and dependency ban policy
- 48+ unit tests covering all functions, error codes, and boundary conditions

[Unreleased]: https://github.com/your-org/time-lock-vault/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/your-org/time-lock-vault/releases/tag/v0.1.0
