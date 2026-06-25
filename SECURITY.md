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

## Pause Mechanism

The contract includes an admin-controlled pause mechanism. When the admin calls `pause()`:

- New `deposit` and `deposit_for` calls are blocked with `ContractPaused` (error 12)
- All existing locked funds are **unaffected** — depositors can still call `withdraw`, `withdraw_to`, and `cancel_deposit` normally
- The pause state is stored as `VaultKey::Paused` in persistent storage; absent means unpaused

**Security considerations for integrators:**
- Always check `is_paused()` before attempting a deposit in automation or off-chain tooling
- Monitoring the `paused` and `unpaused` events allows indexers and UIs to reflect contract state without polling
- Pause does not protect against a compromised admin — it is an operational safeguard, not a security boundary
- If `renounce_admin()` has been called the contract can never be paused again; consider this before renouncing

Pausing is **in-scope** for vulnerability reports if the mechanism can be bypassed or triggered by a non-admin caller.

## Out of Scope

- Stellar network-level issues (report to [Stellar](https://www.stellar.org/bug-bounty-program))
- Bugs in `soroban-sdk` itself (report to [Stellar/rs-soroban-sdk](https://github.com/stellar/rs-soroban-sdk))
- Issues requiring physical access to a user's private key
- Social engineering attacks

## Disclosure Policy

We follow coordinated disclosure. Please allow us reasonable time to patch before any public disclosure.
