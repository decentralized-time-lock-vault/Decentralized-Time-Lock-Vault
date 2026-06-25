# ðŸ”’ Decentralized Time-Lock Vault

[![Rust](https://img.shields.io/badge/Rust-1.81%2B-orange?logo=rust)](https://www.rust-lang.org/)
[![Soroban SDK](https://img.shields.io/badge/Soroban-SDK%20v22-blue?logo=stellar)](https://github.com/stellar/rs-soroban-sdk)
[![License](https://img.shields.io/badge/License-MIT-green)](./LICENSE)
[![Tests](https://github.com/kenedybok3/Decentralized-Time-Lock-Vault/actions/workflows/ci.yml/badge.svg)](https://github.com/kenedybok3/Decentralized-Time-Lock-Vault/actions)

A production-ready Soroban smart contract on the Stellar blockchain that locks XLM or any Stellar asset until a future timestamp is reached.

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
| Storage | Persistent (per-depositor) |
| Max deposit | 10^15 units (1 quadrillion) |
| Max lock duration | 5 years |

---

## How It Works

The deposit and withdrawal lifecycle:

1. **Deposit** — A user calls `deposit(depositor, token, amount, unlock_time, penalty_bps)` → tokens transfer from their wallet into the contract and a `deposit_id` is returned. Multiple active deposits per address are supported.
2. **Storage** — The contract stores a `VaultEntry` in **Persistent Storage** keyed by `(depositor, deposit_id)`.
3. **Unlock modes** — Two unlock modes are available:
   - **Timestamp-based** (`deposit`/`deposit_for`): unlock when `env.ledger().timestamp() >= unlock_time`
   - **Ledger-sequence-based** (`deposit_by_ledger`): unlock when `env.ledger().sequence() >= unlock_ledger`
4. **Withdrawal** — `withdraw(depositor, deposit_id)` checks the appropriate condition and returns funds. `withdraw_to` sends them to a custom recipient.
5. **Pause** — The admin can call `pause()` to temporarily halt all new deposits. Existing withdrawals are unaffected.
6. **Admin Recovery** — An admin can perform emergency withdrawals (funds always return to the depositor, never to the admin).
7. **Trustless Mode** — Admin rights can be transferred via a two-step process, or permanently renounced to make the vault fully trustless.

---

## Architecture

### Deposit / Withdraw Flow

```
Depositor
   |
   +-> deposit(depositor, token, amount, unlock_time, penalty_bps)      <- timestamp unlock
   |   deposit_for(payer, depositor, token, amount, unlock_time, penalty_bps)
   |   deposit_by_ledger(depositor, token, amount, unlock_ledger, penalty_bps) <- ledger unlock
   |       |
   |       +- check paused, validate inputs
   |       +- token.transfer(depositor/payer -> contract)
   |       +- storage::set_deposit(VaultKey::Deposit(depositor, id) -> VaultEntry)
   |       +- emit "deposit" event; return deposit_id
   |
   +-> withdraw(depositor, deposit_id)             <- returns funds to depositor
   |   withdraw_to(depositor, deposit_id, recipient) <- returns funds to custom recipient
   |       |
   |       +- load VaultEntry (timestamp-based or ledger-based)
   |       +- assert unlock condition met
   |       +- storage::remove_deposit(...)         <- state cleared first (CEI)
   |       +- token.transfer(contract -> depositor/recipient)
   |       +- emit "withdraw" / "withdraw_to" event
   |
   +-> cancel_deposit(depositor, deposit_id)       <- early exit with penalty
           |
           +- assert now < unlock_time (must still be locked)
           +- storage::remove_deposit(...)
           +- token.transfer(contract -> fee_recipient, penalty)
           +- token.transfer(contract -> depositor, refund)
           +- emit "dep_cancel" event
```

### Storage Layout

```
Persistent Storage
+-- VaultKey::Admin                           -> Address
|       (set on initialize; removed on renounce_admin)
+-- VaultKey::PendingAdmin                    -> Address
|       (set by transfer_admin; cleared by accept_admin / cancel_transfer_admin)
+-- VaultKey::Initialized                     -> bool
+-- VaultKey::FeeRecipient                    -> Address
+-- VaultKey::MaxDeposit                      -> i128  (absent = use compile-time default)
+-- VaultKey::MaxLockSecs                     -> u64   (absent = use compile-time default)
+-- VaultKey::Paused                          -> bool
+-- VaultKey::DepositCounter(depositor)       -> u32   (monotonically incrementing deposit_id)
+-- VaultKey::Deposit(depositor, id)          -> VaultEntry
|       +-- token:       Address   (SEP-41 token contract)
|       +-- amount:      i128      (locked units)
|       +-- unlock_time: u64       (Unix seconds)
|       +-- depositor:   Address
|       +-- penalty_bps: u32
+-- VaultKey::DepositByLedger(depositor, id)  -> LedgerVaultEntry
|       +-- token:         Address
|       +-- amount:        i128
|       +-- unlock_ledger: u32    (ledger sequence number)
|       +-- depositor:     Address
|       +-- penalty_bps:   u32
+-- VaultKey::DepositorList                   -> Vec<Address>
```

All entries use TTL bump threshold ~30 days (`518_400` ledgers) and target ~5.2 years (`33_000_000` ledgers) so a max-duration deposit cannot expire before its unlock time.
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
        â”œâ”€â”€ errors.rs       # VaultError enum (9 typed codes)
        â”œâ”€â”€ events.rs       # Event emission helpers
        â”œâ”€â”€ storage.rs      # Persistent storage helpers + TTL bump logic
        â””â”€â”€ test.rs         # Full unit test suite (48+ tests)
```

---

## Contract API

### 🔧 Initialization

#### `initialize(admin, fee_recipient, max_deposit, max_lock_secs)`

| Param | Type | Description |
|---|---|---|
| `admin` | `Address` | Must sign; becomes the contract admin |
| `fee_recipient` | `Address` | Receives penalty fees from `cancel_deposit` |
| `max_deposit` | `Option<i128>` | Override for `MAX_DEPOSIT_AMOUNT`; `None` uses compile-time default |
| `max_lock_secs` | `Option<u64>` | Override for `MAX_LOCK_DURATION_SECS`; `None` uses compile-time default |

Must be called once after deployment. Fails with `Unauthorized` if called again.

---

### 💰 Core Functions

#### `deposit(depositor, token, amount, unlock_time, penalty_bps) → u32`
Locks `amount` of `token` until `unlock_time` (Unix timestamp in seconds). Returns a `deposit_id`.
Fails with `ContractPaused` if the admin has paused the contract.

| Param | Type | Constraint |
|---|---|---|
| `depositor` | `Address` | Must sign |
| `token` | `Address` | SEP-41 token contract |
| `amount` | `i128` | `0 < amount ≤ MAX_DEPOSIT_AMOUNT` |
| `unlock_time` | `u64` | `now + 60s ≤ unlock_time ≤ now + MAX_LOCK_DURATION_SECS` |
| `penalty_bps` | `u32` | `0–10000` (basis points for early-exit penalty) |

#### `deposit_for(payer, depositor, token, amount, unlock_time, penalty_bps) → u32`
Same validation as `deposit`, but `payer` signs and funds the vault on behalf of `depositor`. The deposit is credited to `depositor`'s account.
Fails with `ContractPaused` if the contract is paused.

#### `deposit_by_ledger(depositor, token, amount, unlock_ledger, penalty_bps) → u32`
Locks tokens until a specific **ledger sequence number** is reached instead of a timestamp.
Unlock condition: `env.ledger().sequence() >= unlock_ledger`.
Not subject to the pause check.

| Param | Type | Constraint |
|---|---|---|
| `depositor` | `Address` | Must sign |
| `unlock_ledger` | `u32` | Must be greater than the current ledger sequence |

The returned `deposit_id` is shared with timestamp-based deposits (same counter per depositor).

#### `withdraw(depositor, deposit_id)`
Withdraws funds to `depositor`. Checks timestamp condition for `VaultEntry`, or ledger condition for `LedgerVaultEntry`. Fails with `FundsStillLocked` if not yet unlocked, `NoDepositFound` if the id does not exist.

#### `withdraw_to(depositor, deposit_id, recipient)`
Same as `withdraw` but sends funds to `recipient` instead of `depositor`. Only supports timestamp-based deposits. `depositor` must sign.

#### `cancel_deposit(depositor, deposit_id)`
Early exit before the unlock time. Applies the `penalty_bps` fee (sent to `fee_recipient`), refunds the remainder to `depositor`. Fails with `FundsStillLocked` (repurposed: means "already unlocked — use withdraw instead") if past the unlock time.

---

### 🔒 Pause Functions

#### `pause(admin)`
Admin-only. Prevents new `deposit` and `deposit_for` calls. Active deposits and withdrawals are not affected. Emits a `"paused"` event.

#### `unpause(admin)`
Admin-only. Re-enables new deposits. Emits an `"unpaused"` event.

#### `is_paused() → bool`
Returns `true` if the contract is currently paused.

---

### 👨‍⚖️ Admin Functions

#### `emergency_withdraw(admin, depositor, deposit_id)`
Admin-only. Returns funds to the depositor regardless of lock time. Funds always go to the depositor — never to the admin.

#### `transfer_admin(admin, new_admin)`
Step 1 of a two-step admin transfer. Nominates `new_admin` as pending admin. Fails with `InvalidAdmin` if `new_admin` is the same as the current admin.

#### `accept_admin(new_admin)`
Step 2. The pending admin accepts and becomes the active admin.

#### `cancel_transfer_admin(admin)`
Cancels a pending admin transfer. Only the current admin can call this.

#### `renounce_admin(admin)`
Permanently removes admin privileges. After this call, `emergency_withdraw` and all admin functions are disabled forever. Makes the vault fully trustless.

---

### 📖 Read-only Queries

#### `get_vault(depositor, deposit_id) → Option<VaultEntry>`
Returns the vault entry for a timestamp-based deposit. Does **not** bump storage TTL.

#### `get_vault_batch(depositors, deposit_id) → Vec<Option<VaultEntry>>`
Returns vault entries for up to `MAX_BATCH_SIZE` depositors in one call. Each element is `Some(entry)` or `None` if not found.

#### `get_deposit_ids(depositor) → Vec<u32>`
Returns all active deposit IDs for a depositor (both timestamp and ledger types).

#### `time_remaining(depositor, deposit_id) → u64`
Returns seconds until unlock for a timestamp-based deposit. Returns `0` if unlocked or not found.

#### `get_time() → u64`
Returns the current ledger timestamp.

#### `get_admin() → Option<Address>`
Returns the current admin, or `None` if renounced.

#### `get_pending_admin() → Option<Address>`
Returns the pending admin during a transfer, or `None`.

#### `get_fee_recipient() → Option<Address>`
Returns the fee recipient address.

#### `get_constants() → (i128, u64)`
Returns `(MAX_DEPOSIT_AMOUNT, MAX_LOCK_DURATION_SECS)` — runtime-configured values if set at `initialize`, otherwise the compile-time defaults.

#### `get_depositor_count() → u32`
Returns the total number of addresses with at least one active deposit.

#### `get_depositors(offset, limit) → Vec<Address>`
Returns a paginated slice of active depositor addresses. Use `offset=0, limit=N` for the first page, then increment `offset` by `N`.

#### `is_initialized() → bool`
Returns `true` if `initialize` has been called.

---

## 📋 Events

All events are emitted via `env.events().publish(topics, data)`.

| Event | Topics | Data |
|---|---|---|
| `deposit` | `("deposit", depositor, token)` | `(amount, unlock_time)` |
| `withdraw` | `("withdraw", depositor, token)` | `amount` |
| `withdraw_to` | `("withdraw_to", depositor, recipient, token)` | `amount` |
| `emrg_wdraw` | `("emrg_wdraw", depositor)` | `(admin, token, amount)` |
| `dep_cancel` | `("dep_cancel", depositor, token)` | `(amount, penalty)` |
| `adm_xfr_init` | `("adm_xfr_init", current_admin)` | `pending_admin` |
| `adm_xfr_cancel` | `("adm_xfr_cancel", current_admin)` | `pending_admin` |
| `adm_xfr_done` | `("adm_xfr_done", new_admin)` | `()` |
| `adm_renounce` | `("adm_renounce", former_admin)` | `()` |
| `paused` | `("paused", admin)` | `()` |
| `unpaused` | `("unpaused", admin)` | `()` |

All `amount` and `penalty` values are `i128` token units.

---

## 🗄️ Storage Layout

All entries use **Persistent Storage** with TTL bump threshold ≈ 30 days (`BUMP_THRESHOLD = 518_400` ledgers) and target ≈ 5.2 years, ensuring a max-duration deposit cannot expire before its unlock time.

| Key | Type | Lifetime |
|---|---|---|
| `VaultKey::Admin` | `Address` | Set on `initialize`; removed on `renounce_admin` |
| `VaultKey::PendingAdmin` | `Address` | Set by `transfer_admin`; cleared by `accept_admin` / `cancel_transfer_admin` |
| `VaultKey::Initialized` | `bool` | Set once on `initialize`; never removed |
| `VaultKey::FeeRecipient` | `Address` | Set on `initialize`; never removed |
| `VaultKey::MaxDeposit` | `i128` | Set on `initialize` if overridden; absent means use compile-time default |
| `VaultKey::MaxLockSecs` | `u64` | Set on `initialize` if overridden; absent means use compile-time default |
| `VaultKey::Paused` | `bool` | Set by `pause`/`unpause`; absent means not paused |
| `VaultKey::DepositCounter(depositor)` | `u32` | Incremented on each deposit; never decremented |
| `VaultKey::Deposit(depositor, id)` | `VaultEntry` | Created on `deposit`/`deposit_for`; removed on `withdraw` / `emergency_withdraw` / `cancel_deposit` |
| `VaultKey::DepositByLedger(depositor, id)` | `LedgerVaultEntry` | Created on `deposit_by_ledger`; removed on `withdraw` |
| `VaultKey::DepositorList` | `Vec<Address>` | Updated on deposit and when last deposit is withdrawn |

`VaultEntry` fields: `token: Address`, `amount: i128`, `unlock_time: u64`, `depositor: Address`, `penalty_bps: u32`.
`LedgerVaultEntry` fields: `token: Address`, `amount: i128`, `unlock_ledger: u32`, `depositor: Address`, `penalty_bps: u32`.

TTL is bumped on every **write**. Read-only query functions skip the TTL bump to avoid charging callers extra fees.

---

## ❌ Error Codes

| Code | Name | Meaning |
|---|---|---|
| 1 | `InvalidAmount` | Amount ≤ 0 |
| 2 | `UnlockTimeNotInFuture` | `unlock_time`/`unlock_ledger` ≤ current ledger time/sequence |
| 3 | `NoDepositFound` | No active deposit for this depositor/id |
| 4 | `FundsStillLocked` | Lock period not yet expired (withdraw); or already expired (cancel) |
| 5 | `DepositAlreadyExists` | Reserved error code |
| 6 | `LockDurationTooLong` | Lock period exceeds `MAX_LOCK_DURATION_SECS` |
| 7 | `Unauthorized` | Caller is not the admin or pending admin |
| 8 | `AmountTooLarge` | Amount exceeds `MAX_DEPOSIT_AMOUNT` |
| 9 | `InvalidPenaltyBps` | `penalty_bps` > 10000 |
| 10 | `InvalidAdmin` | Nominated admin is the same as the current admin |
| 11 | `LockDurationTooShort` | Lock period is shorter than `MIN_LOCK_DURATION_SECS` (60 s) |
| 12 | `ContractPaused` | `deposit` or `deposit_for` called while contract is paused |


---

## 🔐 Security Properties

| Property | Implementation |
|---|---|
| Checks-Effects-Interactions | Storage cleared before token transfer on every withdrawal |
| Auth-first ordering | `require_auth()` is always the first statement in every mutating function |
| No re-entrancy surface | State removed before any external token call |
| Bounded inputs | Amount capped at 10^15; lock duration capped at 5 years with a 60-second minimum |
| No admin fund theft | Emergency withdraw always sends to depositor, never to admin |
| Trustless mode | Admin can permanently renounce via `renounce_admin()` |
| Safe admin transfer | Two-step transfer prevents accidental key loss |
| Pause surface | `pause()` blocks `deposit` and `deposit_for`; `deposit_by_ledger`, withdrawals, and cancellations are unaffected |
| TTL management | Persistent entries bumped to ~5.2 years on every write; view functions skip TTL bump to avoid charging callers |
| No testutils in production | `features = ["testutils"]` only in `[dev-dependencies]` |
| Initialize front-running | `initialize()` has no on-chain guard against a race: an attacker who observes the deploy transaction in the mempool can call `initialize` first with their own address. **Mitigation:** always call `initialize` in the same transaction as `deploy` (atomic deploy+init). The deploy script does this by default. |

---

## 🔄 Upgradeability

Soroban contracts are **immutable by default** â€” once deployed, the contract code cannot be changed or patched.

| Implication | Detail |
|---|---|
| No in-place upgrades | There is no `upgrade` or `set_code` function; the deployed WASM is fixed forever |
| Bug fixes require redeployment | A new contract must be deployed and users must migrate their funds to it |
| Migration path | The admin can call `emergency_withdraw(admin, depositor)` for each active deposit to return funds to depositors, who can then re-deposit into the new contract |
| Trustless trade-off | If `renounce_admin()` has been called, no migration is possible â€” the contract is fully trustless but also fully immutable with no escape hatch |

Plan deployments carefully. Audit the contract before going to mainnet, because there is no way to patch a live deployment.

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
> Running `cargo build` without `--target wasm32-unknown-unknown` produces a native binary, not a WASM contract. The Makefile's `build` target always passes the correct flag. A `.cargo/config.toml` is included in the repo that documents this trade-off â€” the default target is intentionally left commented out because setting it would break `cargo test` (tests must run natively to use Soroban testutils).

### ✅ Test

```bash
make test
```

> Tests run natively (no `--target` flag) so that `soroban-sdk`'s `testutils` feature works. Never run `cargo test --target wasm32-unknown-unknown`.

### 🔍 Full CI check (fmt + lint + test + audit + deny)

```bash
make check
```

### 🛡️ Security audit

```bash
make audit
```

Runs `cargo audit` to check all dependencies against the [RustSec Advisory Database](https://rustsec.org/).

### 📦 License & dependency policy

```bash
make deny
```

Runs `cargo deny check` to enforce license allowlists and ban policies defined in `deny.toml`.

### ⚡ Optimize WASM

```bash
make optimize
```

### 📊 Check WASM size

```bash
make check-wasm-size
```

Fails if the optimized WASM exceeds `MAX_WASM_BYTES` (default **65 536 bytes / 64 KB**).
Override the threshold at the command line:

```bash
make check-wasm-size MAX_WASM_BYTES=81920   # 80 KB
```

The same threshold is enforced in CI via the `Check WASM size` step in `.github/workflows/ci.yml`.
To update the limit, change `MAX_WASM_BYTES` in both places (or only in `ci.yml` if you don't use the Makefile target locally).

### 🌐 Deploy to Testnet

```bash
export SOROBAN_SECRET_KEY=S...
make deploy-testnet
```

### 🎯 Release Deployment (CI)

Pushing a version tag triggers the `deploy-testnet` CI job automatically:

```bash
git tag v1.0.0
git push origin v1.0.0
```

The job requires the `SOROBAN_SECRET_KEY` secret to be set in the repository's **testnet** environment (`Settings â†’ Environments â†’ testnet â†’ Secrets`). After the run, the deployed contract ID appears in the job's summary tab.

### 🧪 Smoke Test (local node)

Runs a quick end-to-end test against a local Soroban standalone node â€” no funded account or testnet access required.

```bash
# Build the WASM first, then run the smoke test
make smoke-test-local
```

The script (`scripts/smoke_test_local.sh`):
1. Starts a local node via `stellar network start local`
2. Generates a funded test identity
3. Deploys the contract and calls `initialize`, `deposit`, `get_vault`, `time_remaining`, and `withdraw`
4. Asserts expected outputs at each step
5. Stops the local node on exit

---

## 📝 Updating the Stellar CLI Version

`STELLAR_CLI_VERSION` is defined as a top-level `env` variable in `.github/workflows/ci.yml`. Dependabot keeps GitHub Actions versions up to date automatically, but it does not track arbitrary binary downloads. When a new `stellar-cli` release is published at https://github.com/stellar/stellar-cli/releases, update the variable manually:

```yaml
# .github/workflows/ci.yml
env:
  STELLAR_CLI_VERSION: "<new-version>"
```

## ✈️ Deployment Checklist

Use this checklist when deploying to production.

- [ ] Deploy and call `initialize` in the same transaction to prevent front-running
- [ ] Verify `get_admin` returns the expected admin address
- [ ] Run `get_constants` to confirm `MAX_DEPOSIT_AMOUNT` and `MAX_LOCK_DURATION_SECS` match your intended parameters
- [ ] Verify `get_fee_recipient` returns the correct fee recipient address
- [ ] Consider calling `renounce_admin` for fully trustless operation once setup is complete
- [ ] Monitor storage TTL for long-duration vaults â€” entries are bumped on write but not on read
- [ ] Confirm the optimized WASM size is within the Stellar network limit (`make check-wasm-size`)

---

## 💡 Fee Estimation

Soroban charges fees for persistent storage operations. Here is what each call costs at a high level:

| Operation | Storage effect |
|---|---|
| `deposit` | Creates a new persistent entry + pays for initial TTL bump (~30-day threshold, ~5.2-year target) |
| `withdraw` / `cancel_deposit` / `emergency_withdraw` | Removes the persistent entry (storage freed) |
| `get_vault`, `time_remaining`, `get_time` | Read-only â€” **no TTL bump**, no extra storage fee |
| `initialize` | Writes admin / fee-recipient entries once |

Key points:
- The depositor pays the storage-creation fee on `deposit`.
- View functions intentionally skip TTL bumps to avoid charging callers for reads.
- For very long locks (approaching 5 years) the TTL is set well beyond the unlock time, so no manual TTL extension is needed.

For current fee rates see the [Stellar fee documentation](https://developers.stellar.org/docs/learn/fundamentals/fees-resource-limits-metering).

---

## ⚠️ Known Limitations

| Limitation | Detail |
|---|---|
| No partial withdrawals | The full locked amount is returned in one call; partial releases are not supported. |
| No early withdrawal without penalty | Only `cancel_deposit` (with a penalty) or an admin `emergency_withdraw` can release funds before the unlock time. |
| `withdraw_to` timestamp-only | `withdraw_to` only works for timestamp-based (`deposit`/`deposit_for`) vaults; ledger-based deposits must be withdrawn via `withdraw`. |
| `cancel_deposit` timestamp-only | `cancel_deposit` only supports timestamp-based vaults; there is no early-cancel path for `deposit_by_ledger`. |
| Single admin address | Admin is one key — no multisig or DAO governance. Use `renounce_admin` to go fully trustless. |
| Pause scope | `pause` halts `deposit` and `deposit_for` only; `deposit_by_ledger` is not affected by the pause flag. |
| Storage TTL | Persistent entries are bumped to ~5.2 years on every write. Deposits longer than that would require a TTL extension call (current max lock is 5 years, so this is not an issue in practice). |

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

### Run all tests with output

```bash
cargo test --features testutils -- --nocapture
```

> Tests run natively (without `--target wasm32-unknown-unknown`) so that `soroban-sdk`'s `testutils` feature works correctly.

### Test categories

The suite (`contracts/time-lock-vault/src/test.rs`) contains 48+ tests covering:

| Category | What is tested |
|---|---|
| Deposit | Valid deposits, duplicate deposits, amount/time boundary checks |
| Withdraw | Successful withdrawal, early withdrawal rejection, missing deposit |
| Cancel deposit | Penalty calculation, fee recipient transfer, post-unlock rejection |
| Admin | `transfer_admin`, `accept_admin`, `cancel_transfer_admin`, `renounce_admin` |
| Emergency withdraw | Admin-only access, funds always go to depositor |
| Read-only queries | `get_vault`, `time_remaining`, `get_time`, `get_constants`, pagination |
| Error codes | Every `VaultError` variant is exercised |

---

## Use Cases

- **Savings accounts** â€” Lock funds for a fixed period to enforce saving discipline.
- **Token vesting** â€” Team or investor tokens released on a schedule.
- **HODL challenges** â€” Commit to not selling until a future date.
- **Escrow** â€” Time-gated release of payment.

---

## Contributing



See [CHANGELOG.md](./CHANGELOG.md) for the full version history.

---

## License

MIT
