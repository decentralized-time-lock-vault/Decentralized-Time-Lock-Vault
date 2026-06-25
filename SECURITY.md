# Security Policy

## Supported Versions

| Version | Supported |
|---------|-----------|
| 0.1.x   | ✅        |

## Reporting a Vulnerability

**Please do NOT open a public GitHub issue for security vulnerabilities.**

- **Email:** security@example.com *(replace with actual contact before going to production)*
- **Response time:** within 72 hours

Please include:
- A clear description of the vulnerability
- Steps to reproduce or a proof-of-concept
- The potential impact (fund loss, unauthorized access, etc.)

We will acknowledge receipt within 72 hours and aim to release a fix within 14 days for critical issues.

## Scope

The following are considered in-scope vulnerabilities:

- Smart contract logic bugs that could result in fund loss or theft
- Unauthorized access to locked funds (auth bypass)
- Storage manipulation vulnerabilities
- Re-entrancy or checks-effects-interactions violations
- Admin privilege escalation
- Pause bypass — calling `deposit` or `deposit_for` while the contract is paused
- `deposit_by_ledger` unlock condition bypass (ledger sequence check)

## Out of Scope

- Stellar network-level issues (report to [Stellar](https://www.stellar.org/bug-bounty-program))
- Bugs in `soroban-sdk` itself (report to [Stellar/rs-soroban-sdk](https://github.com/stellar/rs-soroban-sdk))
- Issues requiring physical access to a user's private key
- Social engineering attacks

## Disclosure Policy

We follow coordinated disclosure. Please allow us reasonable time to patch before any public disclosure.

## Security Properties

The following properties are enforced by the contract:

| Property | Detail |
|---|---|
| Checks-Effects-Interactions | Storage is cleared before any token transfer on every withdrawal path |
| Auth-first | `require_auth()` is the first statement in every mutating function |
| No admin fund theft | `emergency_withdraw` always transfers to the depositor, never to the admin |
| Two-step admin transfer | Prevents accidental admin key loss |
| Trustless mode | Admin can permanently renounce via `renounce_admin()` |
| Bounded inputs | Amount ≤ `MAX_DEPOSIT_AMOUNT` (10^15); lock ≤ `MAX_LOCK_DURATION_SECS` (~5 years); lock ≥ 60 s |
| Pause surface | `pause()` blocks `deposit` and `deposit_for` only. `deposit_by_ledger`, `withdraw`, `withdraw_to`, `cancel_deposit`, and `emergency_withdraw` are never blocked by the pause flag. |

## Error Codes

These are the typed error codes returned by the contract (`VaultError`):

| Code | Name | Trigger |
|------|------|---------|
| 1 | `InvalidAmount` | Amount ≤ 0 |
| 2 | `UnlockTimeNotInFuture` | `unlock_time`/`unlock_ledger` ≤ current time/sequence |
| 3 | `NoDepositFound` | No active deposit for this depositor/id |
| 4 | `FundsStillLocked` | Withdraw before unlock time; or cancel after unlock time |
| 5 | `DepositAlreadyExists` | Reserved — not currently triggered |
| 6 | `LockDurationTooLong` | Lock duration exceeds `MAX_LOCK_DURATION_SECS` |
| 7 | `Unauthorized` | Caller is not the admin or pending admin |
| 8 | `AmountTooLarge` | Amount exceeds `MAX_DEPOSIT_AMOUNT` |
| 9 | `InvalidPenaltyBps` | `penalty_bps` > 10 000 |
| 10 | `InvalidAdmin` | `new_admin` is the same as the current admin |
| 11 | `LockDurationTooShort` | Lock duration < `MIN_LOCK_DURATION_SECS` (60 s) |
| 12 | `ContractPaused` | `deposit` or `deposit_for` called while the contract is paused |

## Initialize Front-Running

`initialize()` has no on-chain guard against a front-running race. An attacker who observes the deploy transaction in the mempool can call `initialize` first with their own admin address. **Mitigation:** always call `initialize` in the same transaction as `deploy` (atomic deploy+init). The provided deploy script (`scripts/deploy_testnet.sh`) does this by default.
