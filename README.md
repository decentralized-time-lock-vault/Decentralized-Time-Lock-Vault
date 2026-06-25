# 🔒 Decentralized Time-Lock Vault

[![Rust](https://img.shields.io/badge/Rust-1.81%2B-orange?logo=rust)](https://www.rust-lang.org/)
[![Soroban SDK](https://img.shields.io/badge/Soroban-SDK%20v22-blue?logo=stellar)](https://github.com/stellar/rs-soroban-sdk)
[![License](https://img.shields.io/badge/License-MIT-green)](./LICENSE)
[![Tests](https://github.com/kenedybok3/Decentralized-Time-Lock-Vault/actions/workflows/ci.yml/badge.svg)](https://github.com/kenedybok3/Decentralized-Time-Lock-Vault/actions)

A production-ready Soroban smart contract on the Stellar blockchain that locks XLM or any Stellar asset until a future timestamp or ledger sequence number is reached.

**Table of Contents**
- [Overview](#overview)
- [How It Works](#how-it-works)
- [Architecture](#architecture)
- [Contract API](#contract-api)
- [Security Properties](#security-properties)
- [Getting Started](#getting-started)
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

---

## How It Works

The deposit and withdrawal lifecycle:

1. **Deposit** — A user calls `deposit(depositor, token, amount, unlock_time, penalty_bps)` or `deposit_by_ledger(...)` → tokens transfer from their wallet into the contract. A per-depositor `deposit_id` is returned.
2. **Storage** — The contract stores a `VaultEntry` (timestamp-based) or `LedgerVaultEntry` (ledger-sequence-based) in **Persistent Storage** keyed by `(depositor, deposit_id)`.
3. **Verification** — When the user calls `withdraw(depositor, deposit_id)`, the contract checks `env.ledger().timestamp() >= unlock_time` (timestamp deposits) or `env.ledger().sequence() >= unlock_ledger` (ledger deposits).
4. **Unlock** — If the condition has been met, tokens are returned. Otherwise the call fails with `FundsStillLocked`.
5. **Admin Recovery** — An admin can perform emergency withdrawals (funds always return to the depositor, never to the admin).
6. **Pause / Unpause** — The admin can pause the contract, which blocks all new deposits (`ContractPaused`), allowing orderly migrations or incident response.
7. **Trustless Mode** — Admin rights can be transferred via a two-step process, or permanently renounced to make the vault fully trustless.

---

## Architecture

### Deposit / Withdraw Flow

```
Depositor
   │
   ├─► deposit(depositor, token, amount, unlock_time, penalty_bps) → deposit_id
   │       │
   │       ├─ validate amount, unlock_time, penalty_bps
   │       ├─ token.transfer(depositor → contract)
   │       ├─ storage::set_deposit(VaultKey::Deposit(depositor, id) → VaultEntry)
   │       └─ emit "deposit" event
   │
   └─► withdraw(depositor, deposit_id)
           │
           ├─ load VaultEntry (timestamp) or LedgerVaultEntry (ledger)
           ├─ assert now >= unlock_time  OR  sequence >= unlock_ledger
           ├─ storage::remove_deposit(depositor, id)   ← state cleared first (CEI)
           ├─ token.transfer(contract → depositor)
           └─ emit "withdraw" event
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
├── VaultKey::Admin                         → Address
│       (set once on initialize; removed on renounce_admin)
│
├── VaultKey::PendingAdmin                  → Address
│       (set by transfer_admin; cleared by accept_admin / cancel_transfer_admin)
│
├── VaultKey::Initialized                   → bool
│       (set once on initialize; never removed)
│
├── VaultKey::Paused                        → bool
│       (set by pause/unpause; absent means not paused)
│
├── VaultKey::FeeRecipient                  → Address
│
├── VaultKey::MaxDeposit                    → i128
├── VaultKey::MaxLockSecs                   → u64
│
├── VaultKey::DepositCounter(depositor)     → u32
│       (monotonically incremented; never decremented)
│
├── VaultKey::Deposit(depositor, id)        → VaultEntry
│       (timestamp-based; created on deposit; removed on withdraw/cancel/emergency_withdraw)
│
├── VaultKey::DepositByLedger(depositor, id) → LedgerVaultEntry
│       (ledger-sequence-based; created on deposit_by_ledger; removed on withdraw)
│
└── VaultKey::DepositorList                 → Vec<Address>
        (updated on deposit and final withdrawal)
```

All entries use TTL bump threshold ≈ 30 days and target ≈ 5.2 years so a max-duration deposit cannot expire before its unlock time.

---

## Project Structure

```
.
├── Cargo.toml                          # Workspace manifest
├── Makefile                            # Build / test / lint / deploy helpers
├── rust-toolchain.toml                 # Pins stable Rust + wasm32 target
├── .cargo/
│   └── config.toml                     # Documents --target trade-off (default target intentionally unset)
├── .gitignore
├── README.md
├── .github/
│   └── workflows/
│       └── ci.yml                      # CI: lint → test → build WASM
├── scripts/
│   └── deploy_testnet.sh               # Automated testnet deploy + smoke test
└── contracts/time-lock-vault/
    ├── Cargo.toml
    └── src/
        ├── lib.rs          # Crate root & module declarations
        ├── contract.rs     # All public entry points
        ├── types.rs        # VaultKey, VaultEntry, LedgerVaultEntry, protocol constants
        ├── constants.rs    # Protocol constants (MAX_DEPOSIT_AMOUNT, MAX_BATCH_SIZE, …)
        ├── errors.rs       # VaultError enum (12 typed codes)
        ├── events.rs       # Event emission helpers
        ├── storage.rs      # Persistent storage helpers + TTL bump logic
        └── test.rs         # Full unit test suite (48+ tests)
```

---

## Contract API

### 🔧 Initialization

#### `initialize(admin, fee_recipient, max_deposit, max_lock_secs)`
Sets the admin and fee recipient addresses. Optionally overrides compile-time limits for this deployment. Pass `None` to use the defaults (`10^15` and `5 years`). Must be called once after deployment; subsequent calls fail with `Unauthorized`.

| Param | Type | Description |
|---|---|---|
| `admin` | `Address` | Must sign. Becomes the contract admin. |
| `fee_recipient` | `Address` | Address that receives penalty fees from `cancel_deposit`. |
| `max_deposit` | `Option<i128>` | Override max deposit amount. `None` uses compile-time default (`10^15`). |
| `max_lock_secs` | `Option<u64>` | Override max lock duration. `None` uses compile-time default (~5 years). |

---

### 💰 Core Functions

#### `deposit(depositor, token, amount, unlock_time, penalty_bps) → u32`
Locks `amount` of `token` until `unlock_time` (Unix seconds). Returns a `deposit_id`.

Fails with `ContractPaused` if the contract is currently paused.

| Param | Type | Constraint |
|---|---|---|
| `depositor` | `Address` | Must sign |
| `token` | `Address` | SEP-41 token contract |
| `amount` | `i128` | `0 < amount ≤ MAX_DEPOSIT_AMOUNT` |
| `unlock_time` | `u64` | `now < unlock_time ≤ now + MAX_LOCK_DURATION_SECS` and `unlock_time - now ≥ 60` |
| `penalty_bps` | `u32` | `0–10000` (basis points for early-exit penalty) |

#### `deposit_for(payer, depositor, token, amount, unlock_time, penalty_bps) → u32`
Same as `deposit` but a third-party `payer` funds the vault on behalf of `depositor`. The `payer` must sign; the deposit is owned by `depositor`.

Fails with `ContractPaused` if the contract is currently paused.

#### `deposit_by_ledger(depositor, token, amount, unlock_ledger, penalty_bps) → u32`
Locks `amount` of `token` until ledger sequence `unlock_ledger` is reached. Returns a `deposit_id`. The unlock condition is `env.ledger().sequence() >= unlock_ledger`, making the lock immune to timestamp manipulation.

> **Note:** `deposit_by_ledger` does not check the pause state. Only `withdraw` (which handles both modes) releases the funds.

| Param | Type | Constraint |
|---|---|---|
| `depositor` | `Address` | Must sign |
| `token` | `Address` | SEP-41 token contract |
| `amount` | `i128` | `0 < amount ≤ MAX_DEPOSIT_AMOUNT` |
| `unlock_ledger` | `u32` | Must be > `env.ledger().sequence()` |
| `penalty_bps` | `u32` | `0–10000` |

#### `withdraw(depositor, deposit_id)`
Withdraws funds to the depositor once the unlock condition is met. Handles both timestamp-based and ledger-based deposits transparently — it checks the timestamp store first, then the ledger store.

| Param | Type | Description |
|---|---|---|
| `depositor` | `Address` | Must sign |
| `deposit_id` | `u32` | The ID returned by the original deposit call |

#### `withdraw_to(depositor, deposit_id, recipient)`
Same as `withdraw` but sends funds to a specified `recipient` instead of back to the depositor. Only supports timestamp-based deposits. The `depositor` must sign.

#### `cancel_deposit(depositor, deposit_id)`
Cancels an active timestamp-based deposit before the unlock time. The penalty (`penalty_bps` set at deposit time) is sent to the `fee_recipient`; the remainder is returned to the depositor.

Fails with `FundsStillLocked` if the vault is already past its unlock time (use `withdraw` instead).

---

### 🔐 Pause / Unpause

#### `pause(admin)`
Admin-only. Sets the contract into a paused state. While paused, `deposit` and `deposit_for` fail with `ContractPaused`. All withdrawal and query functions remain operational.

#### `unpause(admin)`
Admin-only. Clears the paused state, re-enabling deposits.

#### `is_paused() → bool`
Returns `true` if the contract is currently paused.

---

### 👨‍⚖️ Admin Functions

#### `emergency_withdraw(admin, depositor, deposit_id)`
Admin-only. Returns funds to the depositor regardless of lock time. Only works on timestamp-based deposits. Funds always go to the depositor — never to the admin.

#### `transfer_admin(admin, new_admin)`
Step 1 of a two-step admin transfer. Nominates `new_admin` as pending admin.

#### `accept_admin(new_admin)`
Step 2. The pending admin accepts and becomes the active admin.

#### `cancel_transfer_admin(admin)`
Cancels a pending admin transfer. Only the current admin can cancel.

#### `renounce_admin(admin)`
Permanently removes admin privileges. After this call, `emergency_withdraw`, `pause`, `unpause`, and all admin functions are disabled forever.

---

### 📖 Read-only Queries

#### `get_vault(depositor, deposit_id) → Option<VaultEntry>`
Returns the current timestamp-based vault entry. Does **not** bump storage TTL.

#### `get_vault_batch(depositors, deposit_id) → Vec<Option<VaultEntry>>`
Batch version of `get_vault`. Returns one `Option<VaultEntry>` per input address, all for the same `deposit_id`. Max `MAX_BATCH_SIZE` (20) addresses per call.

#### `get_deposit_ids(depositor) → Vec<u32>`
Returns all active deposit IDs for a depositor (both timestamp and ledger-based).

#### `time_remaining(depositor, deposit_id) → u64`
Returns seconds until unlock for a timestamp-based deposit. Returns `0` if unlocked or not found. Does **not** bump TTL.

#### `get_time() → u64`
Returns the current ledger timestamp.

#### `get_admin() → Option<Address>`
Returns the current admin, or `None` if renounced.

#### `get_pending_admin() → Option<Address>`
Returns the pending admin during a transfer, or `None`.

#### `get_fee_recipient() → Option<Address>`
Returns the fee recipient address set at initialization.

#### `get_constants() → (i128, u64)`
Returns `(MAX_DEPOSIT_AMOUNT, MAX_LOCK_DURATION_SECS)` for this deployment — runtime-configured values if set at `initialize`, otherwise compile-time defaults.

#### `get_depositor_count() → u32`
Returns the total number of addresses with at least one active deposit.

#### `get_depositors(offset, limit) → Vec<Address>`
Returns a paginated slice of active depositor addresses.

#### `is_initialized() → bool`
Returns `true` if `initialize` has been called.

---

## 📋 Events

All events are emitted via `env.events().publish(topics, data)`.

| Event | Topics | Data |
|---|---|---|
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

All `amount` and `penalty` values are `i128` token units. `deposit_id` is a `u32` per-depositor sequence number starting at `0`.

---

## 🗄️ Storage Layout

All entries use **Persistent Storage** with TTL bump threshold ≈ 30 days (`BUMP_THRESHOLD = 518_400` ledgers) and target ≈ 5.2 years (`BUMP_TARGET` derived from `MAX_LOCK_DURATION_SECS / 5s`).

| Key | Type | Lifetime |
|---|---|---|
| `VaultKey::Admin` | `Address` | Set on `initialize`; removed on `renounce_admin` |
| `VaultKey::PendingAdmin` | `Address` | Set by `transfer_admin`; cleared by `accept_admin` / `cancel_transfer_admin` |
| `VaultKey::Initialized` | `bool` | Set once on `initialize`; never removed |
| `VaultKey::Paused` | `bool` | Set by `pause`/`unpause`; absent means not paused |
| `VaultKey::FeeRecipient` | `Address` | Set on `initialize`; never removed |
| `VaultKey::MaxDeposit` | `i128` | Set on `initialize` if overridden; absent means use compile-time default |
| `VaultKey::MaxLockSecs` | `u64` | Set on `initialize` if overridden; absent means use compile-time default |
| `VaultKey::DepositCounter(depositor)` | `u32` | Incremented on each `deposit`; never decremented |
| `VaultKey::Deposit(depositor, id)` | `VaultEntry` | Created on `deposit`/`deposit_for`; removed on `withdraw` / `emergency_withdraw` / `cancel_deposit` |
| `VaultKey::DepositByLedger(depositor, id)` | `LedgerVaultEntry` | Created on `deposit_by_ledger`; removed on `withdraw` |
| `VaultKey::DepositorList` | `Vec<Address>` | Updated on `deposit` and final withdrawal |

`VaultEntry` fields: `token: Address`, `amount: i128`, `unlock_time: u64`, `depositor: Address`, `penalty_bps: u32`.
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
| 6 | `LockDurationTooLong` | Lock period exceeds `MAX_LOCK_DURATION_SECS` |
| 7 | `Unauthorized` | Caller is not the admin (or not the expected pending admin) |
| 8 | `AmountTooLarge` | Amount exceeds `MAX_DEPOSIT_AMOUNT` (10^15) |
| 9 | `InvalidPenaltyBps` | `penalty_bps` > 10000 |
| 10 | `InvalidAdmin` | Nominated admin is the same as the current admin |
| 11 | `LockDurationTooShort` | Lock period is shorter than the minimum (60 s) |
| 12 | `ContractPaused` | Contract is paused; deposits are blocked |

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
| Pause circuit-breaker | Admin can pause new deposits in an incident without touching locked funds |
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

# Install Soroban CLI
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

### 🔍 Full CI check (fmt + lint + test + audit + deny)

```bash
make check
```

### 🛡️ Security audit

```bash
make audit
```

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
```

---

## ✈️ Deployment Checklist

- [ ] Deploy and call `initialize` in the same transaction to prevent front-running
- [ ] Verify `get_admin` returns the expected admin address
- [ ] Verify `is_initialized` returns `true`
- [ ] Run `get_constants` to confirm `MAX_DEPOSIT_AMOUNT` and `MAX_LOCK_DURATION_SECS` match intended parameters
- [ ] Verify `get_fee_recipient` returns the correct fee recipient address
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
