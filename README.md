# Decentralized Time-Lock Vault

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

1. **Deposit (time-based)** - A user calls `deposit(depositor, token, amount, unlock_time, penalty_bps)` -> tokens transfer from their wallet into the contract
2. **Deposit (ledger-based)** - A user calls `deposit_by_ledger(depositor, token, amount, unlock_ledger, penalty_bps)` using a ledger sequence number instead of a timestamp
3. **Deposit for third party** - `deposit_for(payer, depositor, token, amount, unlock_time, penalty_bps)` allows a payer to fund a vault for another beneficiary
4. **Storage** - The contract stores a `VaultEntry` (time-based) or `LedgerVaultEntry` (ledger-based) in **Persistent Storage** keyed by the depositor's address and deposit ID
5. **Verification** - When the user calls `withdraw(depositor, deposit_id)`, the contract checks `env.ledger().timestamp() >= unlock_time` (or `env.ledger().sequence() >= unlock_ledger` for ledger-based deposits)
6. **Unlock** - If the condition has passed, tokens are returned. Otherwise the call fails with `FundsStillLocked`
7. **Early Exit** - The depositor can call `cancel_deposit(depositor, deposit_id)` before unlock time to exit early, paying a penalty (`penalty_bps`) to the fee recipient
8. **Admin Recovery** - An admin can perform emergency withdrawals (funds always return to the depositor, never to the admin)
9. **Trustless Mode** - Admin rights can be transferred via a two-step process, or permanently renounced to make the vault fully trustless

---

## Contract API

### Initialization

#### `initialize(admin: Address, fee_recipient: Address, max_deposit: Option<i128>, max_lock_secs: Option<u64>)`
Sets the admin and fee recipient addresses. Optionally overrides the compile-time limits for this deployment. Pass `None` to use the defaults (`10^15` and `5 years`). Must be called once after deployment. Add `fee_recipient` as the second parameter to designate where early-exit penalties are sent.

---

### Core Functions

#### `deposit(depositor, token, amount, unlock_time, penalty_bps)`
Locks `amount` of `token` until `unlock_time` (Unix seconds).

| Param | Type | Constraint |
|---|---|---|
| `depositor` | `Address` | Must sign |
| `token` | `Address` | SEP-41 token contract |
| `amount` | `i128` | `0 < amount <= 10^15` |
| `unlock_time` | `u64` | `now < unlock_time <= now + 5 years` |
| `penalty_bps` | `u32` | `0-10000` (basis points for early-exit penalty) |

Returns a per-depositor `deposit_id` (starts at 0, increments on each call; never reused).

#### `deposit_for(payer, depositor, token, amount, unlock_time, penalty_bps)`
Same as `deposit` but a third-party `payer` funds the vault on behalf of `depositor`. The `payer` must sign the transaction. The beneficiary (`depositor`) is the sole authorized recipient on withdrawal or emergency-withdrawal.

| Param | Type | Description |
|---|---|---|
| `payer` | `Address` | Must sign; funds transferred from this address |
| `depositor` | `Address` | Beneficiary who can withdraw / cancel |

#### `deposit_by_ledger(depositor, token, amount, unlock_ledger, penalty_bps)`
Ledger-based variant of `deposit`. Uses a ledger sequence number instead of a Unix timestamp for the unlock condition. Useful for deposits that should unlock after a known number of ledger closes.

| Param | Type | Constraint |
|---|---|---|
| `unlock_ledger` | `u32` | `current_ledger < unlock_ledger <= current_ledger + max_lock_ledgers` |

#### `cancel_deposit(depositor, deposit_id)`
Cancels an active deposit before the unlock time. The penalty (`penalty_bps` set at deposit time) is sent to the `fee_recipient`; the remainder is returned to the depositor. Fails with `FundsAlreadyUnlocked` if the vault is already past its unlock time (use `withdraw` instead).

#### `withdraw(depositor, deposit_id)`
Withdraws funds if `now >= unlock_time` (or `current_ledger >= unlock_ledger` for ledger-based deposits). Works for both time-based and ledger-based deposits. Fails with `FundsStillLocked` otherwise.

#### `withdraw_to(depositor, deposit_id, recipient)`
Same as `withdraw` but sends the unlocked funds to an arbitrary `recipient` address instead of the depositor. The depositor must sign and is the only authorized caller.

| Param | Type | Description |
|---|---|---|
| `recipient` | `Address` | Recipient of the unlocked funds |

---

### Admin Functions

#### `emergency_withdraw(admin, depositor, deposit_id)`
Admin-only. Returns funds to the depositor regardless of lock time. Funds always go to the depositor -- never to the admin.

#### `batch_emergency_withdraw(admin, depositors) -> Vec<WithdrawResult>`
Admin-only. Processes emergency withdrawals for multiple depositors in a single transaction -- useful for contract migrations where many depositors need recovery at once.

| Param | Type | Description |
|---|---|---|
| `admin` | `Address` | Must be the current admin. Signs **once** for the entire batch. |
| `depositors` | `Vec<(Address, u32)>` | `(depositor, deposit_id)` pairs to process. Max `MAX_BATCH_SIZE` (20) entries. |

**Best-effort**: entries with no active deposit are skipped and recorded as `success: false` in the result -- the call never aborts due to a missing deposit.

**Returns** `Vec<WithdrawResult>` -- one entry per input:

| Field | Type | Meaning |
|---|---|---|
| `depositor` | `Address` | The input address |
| `deposit_id` | `u32` | The deposit ID |
| `success` | `bool` | `true` = funds transferred; `false` = no deposit found, skipped |

**Instruction budget**: Soroban caps each transaction at ~100M instructions. Each iteration costs roughly 1-2M instructions. The hard cap of 20 keeps the batch well within budget.

#### `pause(admin)` / `unpause(admin)`
Admin-only pause/unpause. When paused, all new deposits are rejected with `ContractPaused`. Withdrawals and cancellations of existing deposits are still allowed.

#### `is_paused() -> bool`
Returns whether the contract is currently paused.

#### `transfer_admin(admin, new_admin)`
Step 1 of a two-step admin transfer. Nominates `new_admin` as pending admin.

#### `accept_admin(new_admin)`
Step 2. The pending admin accepts and becomes the active admin.

#### `cancel_transfer_admin(admin)`
Cancels a pending admin transfer. Only the current admin can cancel.

#### `renounce_admin(admin)`
Permanently removes admin privileges. After this call, `emergency_withdraw` and all admin functions are disabled forever. Makes the vault fully trustless.

---

### Read-only Queries

#### `get_vault(depositor, deposit_id) -> Option<VaultEntry>`
Returns the current time-based vault entry. Does **not** bump storage TTL (no extra fees).

#### `get_vault_by_ledger(depositor, deposit_id) -> Option<LedgerVaultEntry>`
Returns the current ledger-based vault entry. Read-only -- does not bump TTL.

#### `get_vault_batch(depositors: Vec<Address>, deposit_id: u32) -> Vec<Option<VaultEntry>>`
Batch query for multiple depositors. Returns `None` for depositors with no deposit under the given ID. Capped at `MAX_BATCH_SIZE` (20) entries.

#### `get_deposit_ids(depositor) -> Vec<u32>`
Returns all active deposit IDs for a depositor. O(k) where k = number of active deposits (no full-counter scan).

#### `ledgers_remaining(depositor, deposit_id) -> u32`
Returns ledgers remaining until the ledger-based deposit unlocks. Returns `0` if already unlocked or no deposit exists. Does **not** bump TTL.

#### `time_remaining(depositor, deposit_id) -> u64`
Returns seconds until unlock for a time-based deposit. Returns `0` if unlocked or no deposit exists. Does **not** bump TTL.

#### `get_time() -> u64`
Returns the current ledger timestamp.

#### `get_admin() -> Option<Address>`
Returns the current admin, or `None` if renounced.

#### `get_pending_admin() -> Option<Address>`
Returns the pending admin during a transfer, or `None`.

#### `get_fee_recipient() -> Option<Address>`
Returns the fee recipient address set at initialization.

#### `get_constants() -> (i128, u64)`
Returns the effective `(MAX_DEPOSIT_AMOUNT, MAX_LOCK_DURATION_SECS)` for this deployment -- runtime-configured values if set at `initialize`, otherwise the compile-time defaults.

#### `get_depositor_count() -> u32`
Returns the total number of addresses with an active deposit.

#### `get_depositors(offset: u32, limit: u32) -> Vec<Address>`
Returns a paginated slice of active depositor addresses.

| Param | Type | Description |
|---|---|---|
| `offset` | `u32` | Zero-based start index |
| `limit` | `u32` | Maximum number of addresses to return (capped at 100) |

Use `offset=0, limit=N` for the first page, then increment `offset` by `N` for subsequent pages.

#### `is_initialized() -> bool`
Returns whether `initialize` has been called.

---

## Events

All events are emitted via `env.events().publish(topics, data)`.

| Event | Topics | Data |
|---|---|---|
| `deposit` | `("deposit", depositor, token)` | `(deposit_id, amount, unlock_time)` |
| `withdraw` | `("withdraw", depositor, token)` | `(deposit_id, amount)` |
| `withdraw_to` | `("withdraw_to", depositor, recipient, token)` | `(deposit_id, amount)` |
| `emrg_wdraw` | `("emrg_wdraw", depositor)` | `(deposit_id, admin, token, amount)` |
| `dep_cancel` | `("dep_cancel", depositor, token)` | `(amount, penalty)` |
| `adm_xfr_init` | `("adm_xfr_init", current_admin)` | `pending_admin` |
| `adm_xfr_cancel` | `("adm_xfr_cancel", current_admin)` | `pending_admin` |
| `adm_xfr_done` | `("adm_xfr_done", new_admin)` | `()` |
| `adm_renounce` | `("adm_renounce", former_admin)` | `()` |
| `lock_extended` | `("lock_extended", depositor)` | `(old_unlock_time, new_unlock_time)` |
| `paused` | `("paused", admin)` | `()` |
| `unpaused` | `("unpaused", admin)` | `()` |

All `amount` and `penalty` values are `i128` token units. `deposit_id` is a `u32` per-depositor sequence number.

---

## Storage Layout

All entries use **Persistent Storage** with TTL bump threshold ~30 days (`BUMP_THRESHOLD = 518_400` ledgers) and target ~5.2 years (`BUMP_TARGET = 33_000_000` ledgers), ensuring a max-duration deposit cannot expire before its unlock time.

| Key | Type | Lifetime |
|---|---|---|
| `VaultKey::Admin` | `Address` | Set on `initialize`; removed on `renounce_admin` |
| `VaultKey::PendingAdmin` | `Address` | Set by `transfer_admin`; cleared by `accept_admin` / `cancel_transfer_admin` |
| `VaultKey::Initialized` | `bool` | Set once on `initialize`; never removed |
| `VaultKey::FeeRecipient` | `Address` | Set on `initialize`; never removed |
| `VaultKey::MaxDeposit` | `i128` | Set on `initialize` if overridden; absent means use compile-time default |
| `VaultKey::MaxLockSecs` | `u64` | Set on `initialize` if overridden; absent means use compile-time default |
| `VaultKey::Paused` | `bool` | Set by `pause` / `unpause` |
| `VaultKey::DepositCounter(depositor)` | `u32` | Incremented on each `deposit`; never decremented |
| `VaultKey::Deposit(depositor, id)` | `VaultEntry` | Created on `deposit`; removed on `withdraw` / `emergency_withdraw` / `cancel_deposit` |
| `VaultKey::DepositByLedger(depositor, id)` | `LedgerVaultEntry` | Created on `deposit_by_ledger`; removed on `withdraw` / `emergency_withdraw` |
| `VaultKey::ActiveDepositIds(depositor)` | `Vec<u32>` | Active deposit IDs per depositor; updated on create/remove |
| `VaultKey::DepositorMember(depositor)` | `bool` | O(1) membership flag for depositor index |
| `VaultKey::DepositorCount` | `u32` | Total number of unique depositors |
| `VaultKey::DepositorIndex(depositor)` | `u32` | Slot index for swap-remove |
| `VaultKey::DepositorAt(slot)` | `Address` | Address at slot in depositor array |

`VaultEntry` fields: `token: Address`, `amount: i128`, `unlock_time: u64`, `depositor: Address`, `penalty_bps: u32`.

`LedgerVaultEntry` fields: `token: Address`, `amount: i128`, `unlock_ledger: u32`, `depositor: Address`, `penalty_bps: u32`.

TTL is bumped on every **write**. Read-only query functions (`get_vault`, `time_remaining`, `get_time`, etc.) skip the TTL bump to avoid charging callers extra fees.

---

## Error Codes

| Code | Name | Meaning |
|---|---|---|
| 1 | `InvalidAmount` | Amount <= 0 |
| 2 | `UnlockTimeNotInFuture` | `unlock_time` <= current ledger time (or `unlock_ledger` <= current ledger) |
| 3 | `NoDepositFound` | No active deposit for this depositor/id |
| 4 | `FundsStillLocked` | Lock period not yet expired |
| 5 | `DepositAlreadyExists` | Reserved error code |
| 6 | `LockDurationTooLong` | Lock period exceeds 5 years |
| 7 | `Unauthorized` | Caller is not the admin or pending admin |
| 8 | `AmountTooLarge` | Amount exceeds 10^15 |
| 9 | `InvalidPenaltyBps` | `penalty_bps` > 10000 |
| 10 | `InvalidAdmin` | Nominated admin is the same as the current admin |
| 11 | `LockDurationTooShort` | Lock period is shorter than the minimum (60 s) |
| 12 | `ContractPaused` | Contract is paused; deposits rejected |
| 13 | `FundsAlreadyUnlocked` | Cancel attempted after unlock time (use `withdraw` instead) |
| 14 | `BatchTooLarge` | `depositors.len()` exceeds `MAX_BATCH_SIZE` (20) |

---

## Security Properties

| Property | Implementation |
|---|---|
| Checks-Effects-Interactions | Storage cleared before token transfer on every withdrawal |
| Auth-first ordering | `require_auth()` is always the first statement in every mutating function |
| No re-entrancy surface | State removed before any external token call |
| Bounded inputs | Amount capped at 10^15; lock duration capped at 5 years |
| No admin fund theft | Emergency withdraw always sends to depositor, never to admin |
| Trustless mode | Admin can permanently renounce via `renounce_admin()` |
| Safe admin transfer | Two-step transfer prevents accidental key loss |
| TTL management | Persistent entries bumped to ~5.2 years on every write; view functions skip TTL bump |
| No testutils in production | `features = ["testutils"]` only in `[dev-dependencies]` |
| Initialize front-running | `initialize()` has no on-chain guard against a race: an attacker who observes the deploy transaction in the mempool can call `initialize` first with their own address. **Mitigation:** always call `initialize` in the same transaction as `deploy` (atomic deploy+init) so no intermediate state is visible. The deploy script does this by default. |
| View functions are read-only | `get_vault`, `time_remaining`, `ledgers_remaining`, `get_time` do not bump TTL or mutate storage |

---

## Upgradeability

Soroban contracts are **immutable by default** -- once deployed, the contract code cannot be changed or patched.

| Implication | Detail |
|---|---|
| No in-place upgrades | There is no `upgrade` or `set_code` function; the deployed WASM is fixed forever |
| Bug fixes require redeployment | A new contract must be deployed and users must migrate their funds to it |
| Migration path | The admin can call `emergency_withdraw(admin, depositor)` for each active deposit to return funds to depositors, who can then re-deposit into the new contract |
| Trustless trade-off | If `renounce_admin()` has been called, no migration is possible -- the contract is fully trustless but also fully immutable with no escape hatch |

Plan deployments carefully. Audit the contract before going to mainnet, because there is no way to patch a live deployment.

---

## Getting Started

### Prerequisites

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

### Build

```bash
make build
```

> **Why not just `cargo build`?**
> Running `cargo build` without `--target wasm32-unknown-unknown` produces a native binary, not a WASM contract. The Makefile's `build` target always passes the correct flag. A `.cargo/config.toml` is included in the repo that documents this trade-off -- the default target is intentionally left commented out because setting it would break `cargo test` (tests must run natively to use Soroban testutils).

### Test

```bash
make test
```

> Tests run natively (no `--target` flag) so that `soroban-sdk`'s `testutils` feature works. Never run `cargo test --target wasm32-unknown-unknown`.

### Full CI check (fmt + lint + test + audit + deny)

```bash
make check
```

### Security audit

```bash
make audit
```

Runs `cargo audit` to check all dependencies against the [RustSec Advisory Database](https://rustsec.org/).

### License & dependency policy

```bash
make deny
```

Runs `cargo deny check` to enforce license allowlists and ban policies defined in `deny.toml`.

### Optimize WASM

```bash
make optimize
```

### Check WASM size

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

### Deploy to Testnet

```bash
export SOROBAN_SECRET_KEY=S...
make deploy-testnet
```

### Release Deployment (CI)

Pushing a version tag triggers the `deploy-testnet` CI job automatically:

```bash
git tag v1.0.0
git push origin v1.0.0
```

The job requires the `SOROBAN_SECRET_KEY` secret to be set in the repository's **testnet** environment (`Settings > Environments > testnet > Secrets`). After the run, the deployed contract ID appears in the job's summary tab.

### Smoke Test (local node)

Runs a quick end-to-end test against a local Soroban standalone node -- no funded account or testnet access required.

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

## Updating the Stellar CLI Version

`STELLAR_CLI_VERSION` is defined as a top-level `env` variable in `.github/workflows/ci.yml`. Dependabot keeps GitHub Actions versions up to date automatically, but it does not track arbitrary binary downloads. When a new `stellar-cli` release is published at https://github.com/stellar/stellar-cli/releases, update the variable manually:

```yaml
# .github/workflows/ci.yml
env:
  STELLAR_CLI_VERSION: "<new-version>"
```

## Deployment Checklist

Use this checklist when deploying to production.

- [ ] Deploy and call `initialize` in the same transaction to prevent front-running
- [ ] Verify `get_admin` returns the expected admin address
- [ ] Verify `get_fee_recipient` returns the correct fee recipient address
- [ ] Run `get_constants` to confirm `MAX_DEPOSIT_AMOUNT` and `MAX_LOCK_DURATION_SECS` match your intended parameters
- [ ] Verify `is_initialized()` returns `true`
- [ ] Verify `is_paused()` returns `false`
- [ ] Consider calling `renounce_admin` for fully trustless operation once setup is complete
- [ ] Monitor storage TTL for long-duration vaults -- entries are bumped on write but not on read
- [ ] Confirm the optimized WASM size is within the Stellar network limit (`make check-wasm-size`)

---

## Fee Estimation

Soroban charges fees for persistent storage operations. Here is what each call costs at a high level:

| Operation | Storage effect |
|---|---|
| `deposit` / `deposit_for` / `deposit_by_ledger` | Creates persistent entries (VaultEntry/LedgerVaultEntry + ActiveDepositIds + depositor index) + initial TTL bump |
| `withdraw` / `cancel_deposit` / `emergency_withdraw` | Removes persistent entries (storage freed) |
| `get_vault`, `time_remaining`, `ledgers_remaining`, `get_time` | Read-only -- **no TTL bump**, no extra storage fee |
| `initialize` | Writes admin / fee-recipient / limits entries once |

Key points:
- The depositor pays the storage-creation fee on `deposit`.
- View functions intentionally skip TTL bumps to avoid charging callers for reads.
- For very long locks (approaching 5 years) the TTL is set well beyond the unlock time, so no manual TTL extension is needed.

For current fee rates see the [Stellar fee documentation](https://developers.stellar.org/docs/learn/fundamentals/fees-resource-limits-metering).

---

## Known Limitations

| Limitation | Detail |
|---|---|
| No partial withdrawals | The full locked amount is returned in one call; partial releases are not supported. |
| No early withdrawal without admin | Only `cancel_deposit` (with a penalty) or an admin `emergency_withdraw` can release funds before the unlock time. |
| Single admin address | Admin is one key -- no multisig or DAO governance. Use `renounce_admin` to go fully trustless. |
| Storage TTL | Persistent entries are bumped to ~5.2 years on every write. Deposits longer than that would require a TTL extension call (current max lock is 5 years, so this is not an issue in practice). |
| Ledger-based duration granularity | Ledger-based deposits use 5-second intervals. The minimum lock of 60 seconds translates to 12 ledgers. |

---

## Testing

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

The suite (`contracts/time-lock-vault/src/test.rs`) contains 70+ tests covering:

| Category | What is tested |
|---|---|
| Deposit | Valid deposits, multiple deposits per address, amount/time boundary checks |
| Deposit for | Third-party payer, beneficiary withdrawal, access control |
| Deposit by ledger | Ledger-based timing, duration bounds, pause check, full lifecycle |
| Withdraw | Successful withdrawal, early withdrawal rejection, missing deposit, exact unlock time |
| Withdraw to | Funds sent to recipient, early withdrawal rejection, depositor removal |
| Cancel deposit | Penalty calculation (0%, partial, 100%), fee recipient transfer, post-unlock rejection |
| Admin | `transfer_admin`, `accept_admin`, `cancel_transfer_admin`, `renounce_admin`, access control |
| Emergency withdraw | Admin-only access, funds always go to depositor, batch processing |
| Pause / Unpause | Admin-only control, deposit rejection while paused, withdrawal still allowed |
| Read-only queries | `get_vault`, `get_vault_by_ledger`, `get_vault_batch`, `get_deposit_ids`, `time_remaining`, `ledgers_remaining`, `get_time`, `get_constants`, pagination, `is_initialized`, `is_paused` |
| Depositor list | Count, pagination, offset/limit, removal on withdraw/emergency withdraw, consistency |
| Storage | TTL constants, `has_any_deposit` correctness, XDR serialization snapshots |
| Boundary & lifecycle | Max lock duration, exact unlock time, full deposit/withdraw/re-deposit lifecycle |
| Auth assertion | Every mutating function requires correct caller auth |

---

## Use Cases

- **Savings accounts** -- Lock funds for a fixed period to enforce saving discipline.
- **Token vesting** -- Team or investor tokens released on a schedule.
- **HODL challenges** -- Commit to not selling until a future date.
- **Escrow** -- Time-gated release of payment.

---

## Contributing



See [CHANGELOG.md](./CHANGELOG.md) for the full version history.

---

## License

MIT
