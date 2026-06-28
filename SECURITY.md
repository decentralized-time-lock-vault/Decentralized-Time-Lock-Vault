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
- Pause mechanism bypass — calling `deposit` or `deposit_for` while the contract is paused without receiving `ContractPaused` (error code 12)
- Incorrect pause guard coverage — any mutating function that should check `is_paused()` but does not
- Ledger sequence manipulation — exploiting the fact that `deposit_by_ledger` does not check the pause flag to bypass an active pause in unintended ways

## Contract-specific guidance

- `initialize()` has no built-in deploy-time guard. Always initialize in the same transaction as deployment to prevent an attacker from seizing admin rights before the contract is initialized.
- `pause()` blocks `deposit` and `deposit_for`, but does not currently block `deposit_by_ledger`. Do not rely on pause as a complete halt for all deposit entry points.
- `emergency_withdraw()` only supports timestamp-based deposits. Ledger-based deposits cannot be recovered via the current emergency withdraw path.
- `get_vault()`, `get_deposit_ids()`, `time_remaining()`, and `withdraw_to()` operate only on timestamp-based deposits; they do not expose ledger-based deposits.
- The contract is immutable after deployment. Fixes require redeploying to a new contract and migrating funds.

## Out of Scope

- Stellar network-level issues (report to [Stellar](https://www.stellar.org/bug-bounty-program))
- Bugs in `soroban-sdk` itself (report to [Stellar/rs-soroban-sdk](https://github.com/stellar/rs-soroban-sdk))
- Issues requiring physical access to a user's private key
- Social engineering attacks

## Known Design Decisions (Not Vulnerabilities)

The following behaviors are intentional. Understanding them avoids false positives:

### `deposit_by_ledger` does not check the pause state

`deposit_by_ledger` accepts ledger-sequence-based locks and deliberately skips the `is_paused()` check. This is a documented trade-off: ledger-sequence deposits are intended for use cases that require deterministic block-count-based locks, and the pause mechanism is scoped to timestamp deposits (`deposit`, `deposit_for`). If you believe this asymmetry creates an exploitable attack surface in your deployment, the mitigation is to renounce the admin key so that `pause` can never be called, or to avoid offering `deposit_by_ledger` in your front-end while a pause is active.

### Ledger timestamp vs ledger sequence

Soroban's `env.ledger().timestamp()` reflects the validator-agreed close time of the current ledger. Validators can deviate from wall-clock time by a bounded amount (typically a few seconds per ledger). Timestamp-based deposits (`deposit`, `deposit_for`) are therefore subject to minor clock-skew. If sub-minute timing precision is critical, use `deposit_by_ledger` instead — the unlock condition (`env.ledger().sequence() >= unlock_ledger`) is fully deterministic and immune to timestamp manipulation. Note that ledger cadence on Stellar mainnet is approximately 5 seconds per ledger, so estimating a ledger count from a desired wall-clock duration is straightforward but may drift slightly over long periods.

### `ContractPaused` (error code 12)

Error code 12 (`ContractPaused`) is returned by `deposit` and `deposit_for` when the admin has called `pause`. This is not an error state — it is an intentional circuit-breaker for incident response or migrations. Withdrawals, cancellations, and all read-only queries remain operational while the contract is paused.

### Initialize front-running

`initialize()` contains no on-chain guard against a race condition: an attacker who observes the deploy transaction in the mempool before it is confirmed can submit a competing `initialize` call with their own admin address. **Mitigation:** always deploy and initialize in the same atomic transaction. The provided `scripts/deploy_testnet.sh` does this by default.

## Disclosure Policy

We follow coordinated disclosure. Please allow us reasonable time to patch before any public disclosure.

### Disclosure Timeline

| Step | Timeframe |
|------|-----------|
| Acknowledgement of your report | Within 72 hours of receipt |
| Initial triage and severity assessment | Within 5 business days |
| Status update to reporter | Every 7 days until resolved |
| Patch release for Critical/High issues | Within 14 days of confirmation |
| Patch release for Medium/Low issues | Within 60 days of confirmation |
| Public disclosure (coordinated with reporter) | After patch is available, or after 90 days maximum |

If a fix requires redeployment of the contract (because Soroban contracts are immutable), the disclosure timeline begins from when a migration path is communicated to users.

## Severity Guidelines

We use a four-level severity scale aligned with CVSS v3.

### Critical

Direct, exploitable fund loss or theft with no prerequisites.

Examples:
- Logic bug allowing an attacker to withdraw another depositor's funds
- Auth bypass on `emergency_withdraw` sending funds to a non-depositor address
- Re-entrancy enabling double-withdrawal of the same deposit

### High

Significant fund loss or lock-up that requires specific conditions or limited attacker control.

Examples:
- Depositor funds permanently locked due to a storage corruption bug
- Admin privilege escalation enabling unauthorized `emergency_withdraw` calls
- Incorrect penalty calculation causing material fund loss on `cancel_deposit`

### Medium

Contract misbehaviour that does not directly cause fund loss but degrades correctness or availability.

Examples:
- Incorrect event data emitted (wrong amount or deposit_id)
- TTL bump logic failing to extend storage, risking entry expiry before unlock time
- Edge-case overflow in `time_remaining` returning an incorrect value

### Low / Informational

Minor issues or improvements with negligible security impact.

Examples:
- Missing input validation that is harmless in practice
- Documentation inaccuracies about contract behaviour
- Gas/instruction inefficiencies with no exploitable consequence
