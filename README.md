# 🔒 Decentralized Time-Lock Vault

[![Rust](https://img.shields.io/badge/Rust-1.81%2B-orange?logo=rust)](https://www.rust-lang.org/)
[![Soroban SDK](https://img.shields.io/badge/Soroban-SDK%20v22-blue?logo=stellar)](https://github.com/stellar/rs-soroban-sdk)
[![License](https://img.shields.io/badge/License-MIT-green)](./LICENSE)
[![Tests](https://github.com/kenedybok3/Decentralized-Time-Lock-Vault/actions/workflows/ci.yml/badge.svg)](https://github.com/kenedybok3/Decentralized-Time-Lock-Vault/actions)

A production-ready Soroban smart contract on the Stellar blockchain that locks XLM or any Stellar asset until a future timestamp or ledger sequence number is reached.

**Table of Contents**
- [Overview](#overview)
- [How It Works](#how-it-works)
- [Ledger vs Timestamp Deposits](#ledger-vs-timestamp-deposits)
- [Pause Semantics](#pause-semantics)
- [Architecture](#architecture)
- [Contract API](#contract-api)
- [Security Properties](#security-properties)
- [Getting Started](#getting-started)
- [Local Standalone Node Integration Testing](#local-standalone-node-integration-testing)
- [Deployment Checklist](#deployment-checklist)
- [Known Limitations](#known-limitations)

---

## Overview

| Property | Value |
|---|---|
| Network | Stellar (Soroban) |
| Language | Rust |
| SDK | soroban-sdk v22 |
| Storage | Persistent (per-depositor, per-deposit-id) |
| Max deposit | 10^15 units (1 quadrillion) |
| Max lock duration | 5 years |
| Min lock duration | 60 seconds |
| Max batch size | 20 depositors per batch call |

---

## How It Works

The contract supports two independent lock mechanisms:

1. **Deposit** — A user calls `deposit(token, amount, unlock_time)` → tokens transfer from their wallet into the contract
2. **Storage** — The contract stores a `VaultEntry` in **Persistent Storage** keyed by the depositor's address and deposit id
3. **Verification** — When the user calls `withdraw()`, the contract checks `env.ledger().timestamp() >= unlock_time`
4. **Unlock** — If the time has passed, tokens are returned. Otherwise the call fails with `FundsStillLocked`
5. **Admin Recovery** — An admin can perform emergency withdrawals (funds always return to the depositor, never to the admin)
6. **Trustless Mode** — Admin rights can be transferred via a two-step process, or permanently renounced to make the vault fully trustless

---

## Architecture

### Deposit / Withdraw Flow

```
Depositor
   |
   |-> deposit(depositor, token, amount, unlock_time, penalty_bps)
   |       |
   |       |- validate pause / freeze / amount / unlock_time
   |       |- token.transfer(depositor -> contract)
   |       |- storage::set_deposit(VaultKey::Deposit(depositor, id))
   |       `- emit "deposit" event
   |
   |-> deposit_by_ledger(depositor, token, amount, unlock_ledger, penalty_bps)
   |       |
   |       |- validate pause / freeze / amount / ledger sequence
   |       |- token.transfer(depositor -> contract)
   |       |- storage::set_deposit_by_ledger(VaultKey::DepositByLedger(depositor, id))
   |       `- emit "deposit" event
   |
   `-> withdraw(depositor, deposit_id)
           |
           |- load VaultEntry or LedgerVaultEntry
           |- assert unlock condition met (timestamp or ledger sequence)
           |- storage::remove_deposit*(depositor, id)   <- CEI: state cleared first
           |- token.transfer(contract -> depositor)
           `- emit "withdraw" event
```

### Timestamp vs Ledger Deposits

The contract supports two deposit modes:

| Mode | Function | Unlock condition | Entry type |
|---|---|---|---|
| Timestamp | `deposit`, `deposit_for` | `env.ledger().timestamp() >= unlock_time` | `VaultEntry` (unlock_time: u64 Unix seconds) |
| Ledger sequence | `deposit_by_ledger` | `env.ledger().sequence() >= unlock_ledger` | `LedgerVaultEntry` (unlock_ledger: u32) |

Use timestamp mode for human-readable calendar deadlines. Use ledger-sequence mode when you need deterministic block-count-based locks without clock-skew exposure. Note that `deposit_by_ledger` does **not** check the contract pause state.

`withdraw` transparently handles both modes: it first looks for a timestamp entry, then a ledger entry for the same `(depositor, deposit_id)` key.

### Storage Layout

```
Persistent Storage
├── VaultKey::Admin                    → Address
│       (set once on initialize; removed on renounce_admin)
│
├── VaultKey::PendingAdmin             → Address
│       (set by transfer_admin; cleared by accept_admin / cancel_transfer_admin)
│
├── VaultKey::Deposit(depositor: Address, deposit_id: u32) → VaultEntry
│       token:       Address   (SEP-41 token contract)
│       amount:      i128      (locked units)
│       unlock_time: u64       (Unix seconds)
│       depositor:   Address   (owner; stored for event emission)
│       penalty_bps: u32       (early-exit penalty basis points)
│
├── VaultKey::DepositByLedger(depositor: Address, deposit_id: u32) → LedgerVaultEntry
│       token:         Address
│       amount:        i128
│       unlock_ledger: u32
│       depositor:     Address
│       penalty_bps:   u32
│
├── VaultKey::ActiveDepositIds(depositor: Address) → Vec<u32>
│       (active deposit ids for a depositor)
│
├── VaultKey::ActiveDepositCount(depositor: Address) → u32
│       (active deposit count for a depositor)
```
All entries use TTL bump threshold â‰ˆ 30 days and target â‰ˆ 5.2 years so a max-duration deposit cannot expire before its unlock time.

---

## Project Structure

```
.
â”œâ”€â”€ Cargo.toml                          # Workspace manifest
â”œâ”€â”€ Makefile                            # Build / test / lint / deploy helpers
â”œâ”€â”€ rust-toolchain.toml                 # Pins stable Rust + wasm32 target
â”œâ”€â”€ .cargo/
â”‚   â””â”€â”€ config.toml                     # Documents --target trade-off (default target intentionally unset)
â”œâ”€â”€ .gitignore
â”œâ”€â”€ README.md
â”œâ”€â”€ .github/
â”‚   â””â”€â”€ workflows/
â”‚       â””â”€â”€ ci.yml                      # CI: lint â†’ test â†’ build WASM
â”œâ”€â”€ scripts/
â”‚   â””â”€â”€ deploy_testnet.sh               # Automated testnet deploy + smoke test
â””â”€â”€ contracts/time-lock-vault/
    â”œâ”€â”€ Cargo.toml
    â””â”€â”€ src/
        â”œâ”€â”€ lib.rs          # Crate root & module declarations
        â”œâ”€â”€ contract.rs     # All public entry points
        â”œâ”€â”€ types.rs        # VaultKey, VaultEntry, protocol constants
        â”œâ”€â”€ errors.rs       # VaultError enum (16 typed codes)
        â”œâ”€â”€ events.rs       # Event emission helpers
        â”œâ”€â”€ storage.rs      # Persistent storage helpers + TTL bump logic
        â””â”€â”€ test.rs         # Full unit test suite (60+ tests)
```

---

## Contract API

### Initialization

#### `initialize(admin, fee_recipient, max_deposit, max_lock_secs)`

Sets the admin and fee-recipient addresses. Optionally overrides compile-time limits. Must be called once after deployment.

| Param | Type | Description |
|---|---|---|
| `admin` | `Address` | Contract administrator |
| `fee_recipient` | `Address` | Receives early-exit penalty fees |
| `max_deposit` | `Option<i128>` | Override max deposit amount; `None` uses default (10^15) |
| `max_lock_secs` | `Option<u64>` | Override max lock duration; `None` uses default (5 years) |

---

### Core Functions

#### `deposit(depositor, token, amount, unlock_time, penalty_bps) -> u32`

#### `deposit(depositor, token, amount, unlock_time, penalty_bps)`
Locks `amount` of `token` until `unlock_time` (Unix seconds) and returns a unique `deposit_id` for the depositor.

| Param | Type | Constraint |
|---|---|---|
| `depositor` | `Address` | Must sign |
| `token` | `Address` | SEP-41 token contract |
| `amount` | `i128` | `0 < amount <= 10^15` |
| `unlock_time` | `u64` | `now + 60s < unlock_time <= now + 5 years` |
| `penalty_bps` | `u32` | `0-10000` (basis points for early-exit penalty) |

Each depositor can create multiple active deposits. The returned `deposit_id` must be used for later `withdraw`, `cancel_deposit`, or `get_vault` calls.

#### `deposit_for(payer, depositor, token, amount, unlock_time, penalty_bps)`
Locks `amount` of `token` on behalf of `depositor`. The authenticated `payer` funds the deposit, while `depositor` remains the beneficiary.

Same as `deposit` but unlocks at `unlock_ledger` (ledger sequence number) instead of a Unix timestamp. Returns a `deposit_id`. Subject to the same pause/freeze and amount/penalty validations.

| Param | Type | Constraint |
|---|---|---|
| `depositor` | `Address` | Must sign |
| `token` | `Address` | SEP-41 token contract |
| `amount` | `i128` | `0 < amount <= 10^15` |
| `unlock_ledger` | `u32` | `current + min_ledgers < unlock_ledger <= current + max_ledgers` |
| `penalty_bps` | `u32` | `0-10000` |

Ledger duration bounds are derived from the same `MIN_LOCK_DURATION_SECS` / `MAX_LOCK_DURATION_SECS` limits using `LEDGER_SECONDS = 5`.

#### `deposit_for(payer, depositor, token, amount, unlock_time, penalty_bps) -> u32`

Identical to `deposit` but the `payer` transfers the tokens and `depositor` receives the vault entry. `payer` must sign; `depositor` does not need to be present.

#### `withdraw(depositor, deposit_id)`

Withdraws funds if the unlock condition is met. Works for both timestamp and ledger deposits. Blocked while the depositor is frozen.

#### `withdraw_to(depositor, deposit_id, recipient)`

Same as `withdraw` but transfers funds to `recipient` instead of `depositor`. Useful for routing unlocked funds directly to a different address.

#### `cancel_deposit(depositor, deposit_id)`

Cancels an active deposit before its unlock time. The `penalty_bps` fraction goes to the `fee_recipient`; the remainder is returned to the depositor. Blocked while the depositor is frozen. Fails with `FundsAlreadyUnlocked` if the vault is past its unlock time (use `withdraw` instead).

| Param | Type | Constraint |
|---|---|---|
| `depositor` | `Address` | Must sign |
| `token` | `Address` | SEP-41 token contract |
| `amount` | `i128` | `0 < amount ≤ 10^15` |
| `unlock_ledger` | `u32` | `current_ledger < unlock_ledger` |
| `penalty_bps` | `u32` | `0–10000` |

### Admin Functions

#### `emergency_withdraw(admin, depositor, deposit_id)`

Admin-only. Returns funds to the depositor regardless of lock time. Works for both timestamp and ledger deposits. Funds always go to the depositor — never to the admin. Works even when the depositor is frozen.

#### `batch_emergency_withdraw(admin, depositors) -> Vec<WithdrawResult>`

Admin-only. Processes emergency withdrawals for multiple `(depositor, deposit_id)` pairs in one transaction.

| Param | Type | Description |
|---|---|---|
| `admin` | `Address` | Must be the current admin |
| `depositors` | `Vec<(Address, u32)>` | `(depositor, deposit_id)` pairs. Max `MAX_BATCH_SIZE` (20) entries |

Returns `Vec<WithdrawResult>` — one entry per input pair:

| Field | Type | Meaning |
|---|---|---|
| `depositor` | `Address` | The input address |
| `deposit_id` | `u32` | The input deposit ID |
| `success` | `bool` | `true` = funds transferred; `false` = no deposit found, skipped |

#### `pause(admin)` / `unpause(admin)`

Pauses or resumes the contract. While paused, `deposit` and `deposit_by_ledger` fail with `ContractPaused`. Withdrawals, cancellations, and emergency withdrawals still work.

#### `freeze_depositor(admin, depositor)` / `unfreeze_depositor(admin, depositor)`

Freezes or unfreezes a specific depositor. While frozen, the depositor cannot call `deposit`, `deposit_by_ledger`, `withdraw`, `withdraw_to`, or `cancel_deposit`. Emergency withdrawal by the admin is still allowed.

#### `migrate_deposit_to_ledger(admin, depositor, deposit_id, new_unlock_ledger)`

Admin-only. Converts an existing timestamp-based deposit to a ledger-sequence deposit.

#### `migrate_deposit_to_time(admin, depositor, deposit_id, new_unlock_time)`

Admin-only. Converts an existing ledger-sequence deposit to a timestamp-based deposit.

#### `transfer_admin(admin, new_admin)`

Step 1 of a two-step admin transfer. Nominates `new_admin` as pending admin.

#### `accept_admin(new_admin)`

Step 2. The pending admin accepts and becomes the active admin.

#### `cancel_transfer_admin(admin)`

Cancels a pending admin transfer. Only the current admin can cancel.

#### `renounce_admin(admin)`

Permanently removes admin privileges. After this call, all admin functions are disabled forever.

#### `freeze_depositor(admin, depositor)` — emergency freeze (#331)
Admin-only. Blocks `depositor` from making new deposits and from calling `withdraw`. Use `emergency_withdraw` to return their funds while frozen.

#### `unfreeze_depositor(admin, depositor)` — (#331)
Admin-only. Lifts the freeze on `depositor`.

#### `is_depositor_frozen(depositor) → bool`
Returns `true` if the depositor is currently frozen.

#### `freeze_token(admin, token)` — emergency freeze (#331)
Admin-only. Prevents **new deposits** of the specified token contract address. Existing deposits are unaffected and can still be withdrawn normally.

```
# Example: block new USDC deposits after a security incident
freeze_token(admin=ADMIN_ADDR, token=USDC_CONTRACT)
```

#### `unfreeze_token(admin, token)` — (#331)
Admin-only. Re-enables deposits for a previously frozen token.

#### `is_token_frozen(token) → bool`
Returns `true` if new deposits of this token are blocked.

#### `set_max_penalty_bps(admin, bps)` — penalty cap (#332)
Admin-only. Sets the global upper bound on `penalty_bps` for new deposits (0–10000).
Any deposit whose `penalty_bps` exceeds this value is rejected with `InvalidPenaltyBps`.
Pass `10000` to effectively remove the cap.

```
# Restrict all new deposits to a maximum 20% early-exit penalty
set_max_penalty_bps(admin=ADMIN_ADDR, bps=2000)
```

#### `get_max_penalty_bps() → Option<u32>` — (#332)
Returns the configured penalty cap in basis points, or `None` if unset (defaults to 10000).

#### `set_min_cancel_fee(admin, fee)` — minimum cancel fee (#332)
Admin-only. Sets a minimum flat fee (in token units) charged on every `cancel_deposit` call.
Effective penalty = `max(bps_penalty, min_cancel_fee)`, capped at the full deposit amount.
Set to `0` to disable.

```
# Require at least 100 stroops fee on every early cancellation
set_min_cancel_fee(admin=ADMIN_ADDR, fee=100)
```

#### `get_min_cancel_fee() → Option<i128>` — (#332)
Returns the configured minimum cancel fee, or `None` if unset (defaults to 0).

---

### Read-only Queries

#### `get_vault(depositor, deposit_id) -> Option<VaultEntry>`

Returns the timestamp-based vault entry. Does **not** bump storage TTL.

#### `get_vault_by_ledger(depositor, deposit_id) -> Option<LedgerVaultEntry>`

Returns the ledger-based vault entry. Does **not** bump TTL.

#### `get_vault_batch(depositors, deposit_id) -> Vec<Option<VaultEntry>>`

Returns timestamp entries for multiple depositors at once (max 20).

#### `get_deposit_ids(depositor) -> Vec<u32>`

#### `get_vault(depositor, deposit_id) â†’ Option<VaultEntry>`
Returns the current vault entry for the given `deposit_id`. Does **not** bump storage TTL (no extra fees).

#### `time_remaining(depositor, deposit_id) -> u64`

Returns seconds until unlock for either deposit type. For ledger deposits, remaining ledgers are converted using `LEDGER_SECONDS = 5`. Returns `0` if unlocked or not found.

#### `ledgers_remaining(depositor, deposit_id) -> u32`

Returns remaining ledgers for a ledger-based deposit. Returns `0` if unlocked or not found.

#### `get_time() -> u64`

Returns the current ledger timestamp.

#### `get_admin() -> Option<Address>`

Returns the current admin, or `None` if renounced.

#### `get_pending_admin() -> Option<Address>`

Returns the pending admin during a transfer, or `None`.

#### `get_fee_recipient() -> Option<Address>`

Returns the fee recipient address.

#### `get_constants() -> (i128, u64)`

Returns the effective `(MAX_DEPOSIT_AMOUNT, MAX_LOCK_DURATION_SECS)` for this deployment.

#### `get_depositor_count() -> u32`

Returns the number of addresses with at least one active deposit.

#### `get_depositors(offset, limit) -> Vec<Address>`

Returns a paginated slice of active depositor addresses. `limit` is capped at 100.

#### `is_paused() -> bool`

Returns `true` if the contract is currently paused.

#### `is_depositor_frozen(depositor) -> bool`

Returns `true` if the depositor is currently frozen.

#### `is_initialized() -> bool`

Returns `true` if `initialize` has been called.
---

## 📋 Events

All events are emitted via `env.events().publish(topics, data)`.

| Event | Topics | Data |
|---|---|---|
| `deposit` | `("deposit", depositor, token)` | `(deposit_id, amount, unlock_time)` |
| `top_up` | `("top_up", depositor, token)` | `(deposit_id, added, new_total)` |
| `withdraw` | `("withdraw", depositor, token)` | `(deposit_id, amount)` |
| `withdraw_to` | `("withdraw_to", depositor, recipient, token)` | `(deposit_id, amount)` |
| `emrg_wdraw` | `("emrg_wdraw", depositor)` | `(deposit_id, admin, token, amount)` |
| `dep_cancel` | `("dep_cancel", depositor, token)` | `(amount, penalty)` |
| `paused` | `("paused", admin)` | `()` |
| `unpaused` | `("unpaused", admin)` | `()` |
| `frozen` | `("frozen", admin, depositor)` | `()` |
| `unfrozen` | `("unfrozen", admin, depositor)` | `()` |
| `migrated` | `("migrated", depositor)` | `(deposit_id, to_ledger, to_time)` |
| `adm_xfr_init` | `("adm_xfr_init", current_admin)` | `pending_admin` |
| `adm_xfr_cancel` | `("adm_xfr_cancel", current_admin)` | `pending_admin` |
| `adm_xfr_done` | `("adm_xfr_done", new_admin)` | `()` |
| `adm_xfr_cancel` | `("adm_xfr_cancel", admin)` | `pending_admin` |
| `adm_renounce` | `("adm_renounce", former_admin)` | `()` |
| `paused` | `("paused", admin)` | `()` |
| `unpaused` | `("unpaused", admin)` | `()` |
| `lock_extended` | `("lock_extended", depositor)` | `(old_unlock_time, new_unlock_time)` |

All `amount` and `penalty` values are `i128` token units. `deposit_id` is a `u32` per-depositor sequence number. For `deposit_by_ledger`, the `unlock_time` field in the `deposit` event carries `unlock_ledger` cast to `u64`.
---

## 🗄️ Storage Layout

All entries use **Persistent Storage** with TTL bump threshold ≈ 30 days (`BUMP_THRESHOLD = 518_400` ledgers) and target ≈ 5.2 years (`BUMP_TARGET` derived from `MAX_LOCK_DURATION_SECS / 5s`).

| Key | Value Type | Lifetime |
|---|---|---|
| `VaultKey::Admin` | `Address` | Set on `initialize`; removed on `renounce_admin` |
| `VaultKey::PendingAdmin` | `Address` | Set by `transfer_admin`; cleared by `accept_admin` / `cancel_transfer_admin` |
| `VaultKey::Initialized` | `bool` | Set once on `initialize`; never removed |
| `VaultKey::Paused` | `bool` | Set by `pause`/`unpause`; absent means not paused |
| `VaultKey::FeeRecipient` | `Address` | Set on `initialize`; never removed |
| `VaultKey::Paused` | `bool` | Toggled by `pause`/`unpause`; absent → false |
| `VaultKey::MaxDeposit` | `i128` | Set on `initialize` if overridden; absent → compile-time default |
| `VaultKey::MaxLockSecs` | `u64` | Set on `initialize` if overridden; absent → compile-time default |
| `VaultKey::DepositCounter(depositor)` | `u32` | Incremented on each deposit; never decremented |
| `VaultKey::ActiveDepositIds(depositor)` | `Vec<u32>` | Updated on deposit and removal; absent → empty |
| `VaultKey::Deposit(depositor, id)` | `VaultEntry` | Created on `deposit`; removed on `withdraw` / `emergency_withdraw` / `cancel_deposit` |
| `VaultKey::ActiveDepositIds(depositor)` | `Vec<u32>` | Active deposit IDs for a depositor |
| `VaultKey::ActiveDepositCount(depositor)` | `u32` | Active deposit count for a depositor |
| `VaultKey::DepositorAt(slot)` | `Address` | Depositor index used for pagination |
| `VaultKey::DepositorIndex(depositor)` | `u32` | Slot index for an active depositor |

`VaultEntry` fields: `token: Address`, `amount: i128`, `unlock_time: u64`, `depositor: Address`, `penalty_bps: u32`.
`LedgerVaultEntry` fields: `token: Address`, `amount: i128`, `unlock_ledger: u32`, `depositor: Address`, `penalty_bps: u32`.

`LedgerVaultEntry` fields: `token: Address`, `amount: i128`, `unlock_ledger: u32`, `depositor: Address`, `penalty_bps: u32`.

TTL is bumped on every **write**. Read-only query functions skip the TTL bump to avoid charging callers extra fees.

---

## ❌ Error Codes

| Code | Name | Meaning |
|---|---|---|
| 1 | `InvalidAmount` | Amount ≤ 0 |
| 2 | `UnlockTimeNotInFuture` | `unlock_time` ≤ current ledger time (or `unlock_ledger` ≤ current sequence) |
| 3 | `NoDepositFound` | No active deposit for this depositor/id |
| 4 | `FundsStillLocked` | Lock period not yet expired |
| 5 | `DepositAlreadyExists` | Reserved error code |
| 6 | `LockDurationTooLong` | Lock period exceeds 5 years |
| 7 | `Unauthorized` | Caller is not the admin |
| 8 | `AmountTooLarge` | Amount exceeds 10^15 |
| 9 | `InvalidPenaltyBps` | `penalty_bps` > 10000 |
| 10 | `LockDurationTooShort` | Lock period is shorter than the minimum (60 s) |
| 11 | `InvalidAdmin` | Nominated admin is the same as the current admin |
| 12 | `BatchTooLarge` | `depositors.len()` exceeds `MAX_BATCH_SIZE` (25) |
| 13 | `FundsAlreadyUnlocked` | Funds are already past unlock time; use `withdraw` |
| 14 | `DepositorFrozen` | Depositor is frozen; contact admin |
| 15 | `MigrationNotAllowed` | Migration precondition failed |
| 16 | `TokenFrozen` | Token is frozen; new deposits blocked (#331) |

---

## 🔐 Security Properties

| Property | Implementation |
|---|---|
| Checks-Effects-Interactions | Storage cleared before token transfer on every withdrawal path |
| Auth-first ordering | `require_auth()` is always the first statement in every mutating function |
| No re-entrancy surface | State removed before any external token call |
| Bounded inputs | Amount capped at 10^15; lock duration capped at 5 years with a 60 s minimum |
| No admin fund theft | Emergency withdraw always sends to depositor, never to admin |
| Trustless mode | Admin can permanently renounce via `renounce_admin()` |
| Safe admin transfer | Two-step transfer prevents accidental key loss |
| Pause mechanism | Admin can pause new deposits via `pause()` without affecting existing locked funds; `unpause()` resumes normal operation |
| Pause mechanism | Admin can pause new deposits via `pause()` without affecting existing locked funds; `unpause()` resumes normal operation |
| TTL management | Persistent entries bumped to ~5.2 years on every write; view functions skip TTL bump |
| No testutils in production | `features = ["testutils"]` only in `[dev-dependencies]` |
| Initialize front-running | `initialize()` has no on-chain guard against a race: an attacker who observes the deploy transaction in the mempool can call `initialize` first with their own address. **Mitigation:** always call `initialize` in the same transaction as `deploy` (atomic deploy+init). The deploy script does this by default. |

---

## 🔄 Upgradeability

Soroban contracts are **immutable by default** — once deployed, the contract code cannot be changed or patched.

| Implication | Detail |
|---|---|
| No in-place upgrades | There is no `upgrade` or `set_code` function; the deployed WASM is fixed forever |
| Bug fixes require redeployment | A new contract must be deployed and users must migrate their funds to it |
| Migration path | The admin can call `emergency_withdraw(admin, depositor, deposit_id)` for each active deposit to return funds, who can then re-deposit into the new contract. Use `get_depositors` + `get_deposit_ids` to enumerate all live deposits. |
| Pause during migration | Call `pause` before starting migration to prevent new deposits during the transition. |
| Trustless trade-off | If `renounce_admin()` has been called, no migration is possible — the contract is fully trustless but also fully immutable with no escape hatch |

---

## 🚀 Getting Started

### 📋 Prerequisites

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Add WASM target
rustup target add wasm32-unknown-unknown

# Install Soroban CLI (also installs the stellar CLI)
cargo install --locked soroban-cli

# Install cargo-watch (optional, for make watch)
cargo install cargo-watch
```

### 🔨 Build

```bash
make build
```

> **Why not just `cargo build`?**
> Running `cargo build` without `--target wasm32-unknown-unknown` produces a native binary, not a WASM contract. The Makefile's `build` target always passes the correct flag.

### ✅ Test

```bash
make test
```

> Tests run natively (no `--target` flag) so that `soroban-sdk`'s `testutils` feature works.

### 🔍 Full CI check (fmt + lint + test + build + audit + deny)

```bash
make check
```

### 🛡️ Security audit (CI + local)

The CI pipeline runs `cargo audit` on every push and pull request via the `security-audit` job
in `.github/workflows/ci.yml` (using `rustsec/audit-check`). This checks all dependencies
against the [RustSec Advisory Database](https://rustsec.org/) and fails the build if any
known-vulnerable dependency is detected.

Run the same check locally:

```bash
make audit
# or directly:
cargo audit
```

`cargo-audit` is installed automatically as part of `make check` (which mirrors the full CI
pipeline: `fmt-check → lint → test → audit → deny`).

**GitHub Release WASM builds** — pushing a version tag (`v*`) also triggers
`.github/workflows/release.yml`, which builds an optimized WASM binary and attaches it to the
GitHub Release as `time_lock_vault.optimized.wasm`.

### 📦 License & dependency policy

```bash
make deny
```

### ⚡ Optimize WASM

```bash
make optimize
```

### 📊 Check WASM size

```bash
make check-wasm-size
```

Fails if the optimized WASM exceeds `MAX_WASM_BYTES` (default **65 536 bytes / 64 KB**).

### 🌐 Deploy to Testnet

```bash
export SOROBAN_SECRET_KEY=S...
make deploy-testnet
```

### 🧪 Smoke Test (local node)

```bash
make smoke-test-local

# Or invoke the script directly:
bash scripts/smoke_test_local.sh
```

### What the smoke test does

The script (`scripts/smoke_test_local.sh`) exercises the full deposit → query → withdraw lifecycle:

| Step | Action | What is verified |
|---|---|---|
| 1 | Check WASM exists | Fails fast if `make build` was not run |
| 2 | `stellar network start local --background` | Local node is up and listening |
| 3 | `stellar keys generate --fund` | A funded test identity is created |
| 4 | `stellar contract deploy` | Contract deploys successfully; a contract ID is returned |
| 5 | `initialize(admin, ...)` | Contract accepts the init call without error |
| 6 | `stellar contract asset deploy --asset native` | Native XLM is wrapped as a SEP-41 token |
| 7 | `deposit(depositor, token, 1000, now+120s, penalty_bps=0)` | Deposit returns a `deposit_id`; token balance decreases |
| 8 | `get_vault(depositor, 0)` | Returned entry contains `amount = 1000` |
| 9 | `time_remaining(depositor, 0)` | Returns > 0 (lock has not expired) |
| 10 | `withdraw(depositor, 0)` | Fails with `FundsStillLocked` (asserts error string) |
| EXIT | `stellar network stop local` | Node is shut down cleanly via `trap` |

### Expected output

```
==> Checking WASM...
  ✓ WASM found: target/wasm32-unknown-unknown/release/time_lock_vault.wasm
==> Starting local Soroban node...
  ✓ Local node started
==> Setting up identity...
  ✓ Identity: GABC...XYZ
==> Deploying contract...
  ✓ Contract deployed: CCCC...AAAA
==> Calling initialize...
  ✓ initialize OK
==> Wrapping native XLM...
  ✓ Token: CDDD...BBBB
==> Calling deposit...
  ✓ deposit OK
==> Calling get_vault...
  ✓ get_vault returns amount 1000
==> Calling time_remaining...
  ✓ time_remaining > 0 (119)
==> Calling withdraw (expect FundsStillLocked)...
  ✓ withdraw fails while locked

All smoke tests passed.
==> Stopping local node...
```

### Extending the smoke test

To add assertions for additional contract functions, edit `scripts/smoke_test_local.sh`. The `assert_contains` helper makes it easy:

---

## ✈️ Deployment Checklist

- [ ] Deploy and call `initialize` in the same transaction to prevent front-running
- [ ] Pass the correct `fee_recipient` address to `initialize` (receives early-exit penalties)
- [ ] Verify `get_admin` returns the expected admin address
- [ ] Verify `is_initialized` returns `true`
- [ ] Run `get_constants` to confirm `MAX_DEPOSIT_AMOUNT` and `MAX_LOCK_DURATION_SECS` match intended parameters
- [ ] Verify `get_fee_recipient` returns the correct fee recipient address
- [ ] Run `is_initialized` to confirm the contract initialized successfully
- [ ] Consider calling `renounce_admin` for fully trustless operation once setup is complete
- [ ] Monitor storage TTL for long-duration vaults — entries are bumped on write but not on read
- [ ] Confirm the optimized WASM size is within the Stellar network limit (`make check-wasm-size`)

---

## ⚠️ Known Limitations

| Limitation | Detail |
|---|---|
| Multiple deposits per address supported | Each `deposit` call returns a new `deposit_id`. Use `get_deposit_ids` to list all active IDs. |
| No partial withdrawals | The full locked amount is returned in one call. |
| `cancel_deposit` / `withdraw_to` / `emergency_withdraw` are timestamp-only | These functions do not handle ledger-based deposits. Use `withdraw` for ledger deposits. |
| `is_paused` check only on timestamp deposits | `deposit_by_ledger` does not check the pause flag. |
| Single admin address | Admin is one key — no multisig or DAO governance. Use `renounce_admin` to go fully trustless. |
| `MAX_BATCH_SIZE` = 20 | `get_vault_batch` accepts at most 20 depositors per call. |

---

## 🧬 Testing

### Run all tests

```bash
make test
```

### Run a specific test

```bash
cargo test test_deposit_success --features testutils -- --nocapture
```

### Test categories

The suite (`contracts/time-lock-vault/src/test.rs`) contains 48+ tests covering:

| Category | What is tested |
|---|---|
| Deposit | Valid deposits, `deposit_for`, `deposit_by_ledger`, amount/time boundary checks |
| Withdraw | Successful withdrawal, early withdrawal rejection, `withdraw_to`, ledger-based withdraw |
| Cancel deposit | Penalty calculation, fee recipient transfer, post-unlock rejection |
| Pause / Unpause | Deposit blocked while paused; withdraw unaffected |
| Admin | `transfer_admin`, `accept_admin`, `cancel_transfer_admin`, `renounce_admin` |
| Pause / Unpause | Deposit blocked while paused, withdrawal unaffected, `is_paused` query |
| Emergency withdraw | Admin-only access, funds always go to depositor |
| Read-only queries | `get_vault`, `get_vault_batch`, `get_deposit_ids`, `time_remaining`, `is_initialized`, pagination |
| Error codes | Every `VaultError` variant is exercised |

---

## Use Cases

- **Savings accounts** — Lock funds for a fixed period to enforce saving discipline.
- **Token vesting** — Team or investor tokens released on a schedule.
- **HODL challenges** — Commit to not selling until a future date or block height.
- **Escrow** — Time-gated release of payment.
- **Third-party funding** — Use `deposit_for` to fund a vault on behalf of another address.

---

## Contributing

See [CONTRIBUTING.md](./CONTRIBUTING.md) for guidelines.

See [CHANGELOG.md](./CHANGELOG.md) for the full version history.

---

## License

MIT
