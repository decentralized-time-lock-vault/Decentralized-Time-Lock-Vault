# Bugfix Requirements Document

## Introduction

This document captures four distinct defects in the `time-lock-vault` Soroban smart contract. The bugs span missing validation in the ledger-deposit path, unnecessary storage/computation overhead, a stale duplicate-guard artifact in `storage.rs`, and ambiguous error handling during initialization. Together they create a risk of inconsistent deposit state, inflated on-chain resource costs, build failures, and confusing runtime errors for operators.

---

## Bug 1 — Missing Validation in the Ledger Deposit Path

### Current Behavior (Defect)

1.1 WHEN `deposit_by_ledger` is called while the contract is paused THEN the system accepts the deposit without returning `ContractPaused`.

1.2 WHEN `deposit_by_ledger` is called with a `lock_ledgers` duration that exceeds the configured `max_lock_secs`-derived ledger ceiling THEN the system accepts the deposit without returning `LockDurationTooLong`.

1.3 WHEN `deposit_by_ledger` is called with a `lock_ledgers` duration below the `MIN_LOCK_DURATION_SECS`-derived ledger floor THEN the system accepts the deposit without returning `LockDurationTooShort`.

### Expected Behavior (Correct)

2.1 WHEN `deposit_by_ledger` is called while the contract is paused THEN the system SHALL return `ContractPaused` and reject the deposit, matching the behavior of `deposit` and `deposit_for`.

2.2 WHEN `deposit_by_ledger` is called with a `lock_ledgers` duration that exceeds the configured `max_lock_secs`-derived ledger ceiling THEN the system SHALL return `LockDurationTooLong` and reject the deposit.

2.3 WHEN `deposit_by_ledger` is called with a `lock_ledgers` duration below the `MIN_LOCK_DURATION_SECS`-derived ledger floor THEN the system SHALL return `LockDurationTooShort` and reject the deposit.

### Unchanged Behavior (Regression Prevention)

3.1 WHEN `deposit_by_ledger` is called while the contract is not paused with valid lock duration bounds THEN the system SHALL CONTINUE TO accept the deposit and return a valid deposit ID.

3.2 WHEN `deposit` or `deposit_for` are called with any combination of pause state or lock duration THEN the system SHALL CONTINUE TO enforce their existing validation rules unchanged.

3.3 WHEN `deposit_by_ledger` is called by a frozen depositor THEN the system SHALL CONTINUE TO return `DepositorFrozen`.

---

## Bug 2 — Unnecessary Storage/Computation Overhead

### Current Behavior (Defect)

2.1 WHEN storage helpers perform a linear scan over all active deposit IDs to remove a single ID THEN the system traverses the entire `ActiveDepositIds` vector, creating O(n) cost that grows unboundedly with deposit count.

2.2 WHEN `get_depositors` is called without an enforced `limit` cap on the caller side THEN the system allows unbounded `limit` values to be passed into `get_depositors_page`, risking runaway computation costs.

2.3 WHEN the `has_any_deposit` helper checks for an active deposit THEN the system reads a separate `ActiveDepositCount` key instead of deriving the answer from the already-maintained `ActiveDepositIds` list, causing a redundant storage read that diverges from the inline comment stating the count is derived from the list.

### Expected Behavior (Correct)

2.1 WHEN a deposit ID is removed from `ActiveDepositIds` THEN the system SHALL perform this operation in O(1) using a swap-and-pop pattern rather than a linear scan.

2.2 WHEN `get_depositors` is called with any `limit` value THEN the system SHALL cap the effective limit at `MAX_DEPOSITORS_PAGE_SIZE` (100) before passing it to the storage layer, so the cap is enforced unconditionally.

2.3 WHEN `has_any_deposit` is called THEN the system SHALL derive the boolean result from the single `ActiveDepositCount` key that is already maintained, eliminating any redundant or mismatched reads.

### Unchanged Behavior (Regression Prevention)

3.1 WHEN a depositor still has remaining active deposits after a withdrawal or cancellation THEN the system SHALL CONTINUE TO report the correct non-zero deposit count and non-empty deposit ID list.

3.2 WHEN a depositor's last active deposit is removed THEN the system SHALL CONTINUE TO report zero active deposits and remove the depositor from the global depositor list.

3.3 WHEN `get_depositors` is called with a `limit` within the allowed page size THEN the system SHALL CONTINUE TO return the correct paginated slice of depositor addresses.

---

## Bug 3 — Stale Duplicate Artifact in `storage.rs`

### Current Behavior (Defect)

3.1 WHEN `storage.rs` is compiled THEN the system fails to compile because a leftover stub of `get_deposit_readonly(env, depositor)` (single-argument signature) is present immediately before the correct two-argument `get_deposit` function, creating a duplicate symbol and a dangling brace that causes a syntax/compile error.

3.2 WHEN `emergency_withdraw` in `contract.rs` is compiled THEN the system fails to compile because a dead-code call to `storage::get_deposit_readonly(&env, &depositor)` (single-argument, now non-existent) remains inside the function body after the timestamp-deposit branch, creating a type-mismatch compile error.

### Expected Behavior (Correct)

3.1 WHEN `storage.rs` is compiled THEN the system SHALL compile without errors because the leftover single-argument `get_deposit_readonly` stub and its dangling brace are removed.

3.2 WHEN `emergency_withdraw` in `contract.rs` is compiled THEN the system SHALL compile without errors because the dead-code single-argument `storage::get_deposit_readonly` call is removed.

### Unchanged Behavior (Regression Prevention)

3.1 WHEN any caller invokes `storage::get_deposit_readonly(env, depositor, deposit_id)` (two-argument form) THEN the system SHALL CONTINUE TO return the correct `Option<VaultEntry>` without bumping the TTL.

3.2 WHEN `emergency_withdraw` is called by the admin for a valid timestamp-based deposit THEN the system SHALL CONTINUE TO transfer funds back to the depositor and remove the deposit record.

---

## Bug 4 — Ambiguous Initialization Validation

### Current Behavior (Defect)

4.1 WHEN `initialize` is called with `max_lock_secs = Some(0)` THEN the system returns `VaultError::LockDurationTooLong`, which is semantically incorrect because a value of zero is an invalid configuration input, not a lock that is too long.

4.2 WHEN `initialize` is called with `max_lock_secs = Some(0)` THEN the system uses the same error code (`LockDurationTooLong`) that runtime deposit operations return for oversized lock durations, making it impossible for clients to distinguish a misconfigured deployment from a rejected deposit.

### Expected Behavior (Correct)

4.1 WHEN `initialize` is called with `max_lock_secs = Some(0)` THEN the system SHALL return a dedicated configuration error (e.g., `InvalidAmount` or a new `InvalidConfig` error variant) that is distinct from the runtime `LockDurationTooLong` error returned during deposits.

4.2 WHEN `initialize` is called with `max_lock_secs = Some(v)` where `v > 0` THEN the system SHALL accept the value and store it, with no change to normal initialization behavior.

### Unchanged Behavior (Regression Prevention)

4.1 WHEN `initialize` is called with `max_lock_secs = None` THEN the system SHALL CONTINUE TO initialize successfully using the default `MAX_LOCK_DURATION_SECS` constant.

4.2 WHEN `initialize` is called with `max_lock_secs = Some(v)` where `v > 0` THEN the system SHALL CONTINUE TO store the custom limit and enforce it during subsequent deposit calls.

4.3 WHEN `deposit` or `deposit_by_ledger` is called after a valid initialization and the lock duration exceeds the configured maximum THEN the system SHALL CONTINUE TO return `LockDurationTooLong`.

---

## Bug Condition Summary (Structured Pseudocode)

### Bug 1 — Ledger Deposit Missing Pause / Bounds Checks

```pascal
FUNCTION isBugCondition_B1(call)
  INPUT: call = deposit_by_ledger(depositor, token, amount, unlock_ledger, penalty_bps)
  OUTPUT: boolean
  RETURN is_paused()
      OR (unlock_ledger - current_ledger) > (max_lock_secs / LEDGER_SECONDS)
      OR (unlock_ledger - current_ledger) < (MIN_LOCK_DURATION_SECS / LEDGER_SECONDS)
END FUNCTION

// Fix Checking
FOR ALL call WHERE isBugCondition_B1(call) DO
  result ← deposit_by_ledger'(call)
  ASSERT result IN {ContractPaused, LockDurationTooLong, LockDurationTooShort}
END FOR

// Preservation Checking
FOR ALL call WHERE NOT isBugCondition_B1(call) DO
  ASSERT deposit_by_ledger(call) = deposit_by_ledger'(call)
END FOR
```

### Bug 2 — Storage Overhead

```pascal
FUNCTION isBugCondition_B2(op)
  INPUT: op = remove_active_deposit_id | get_depositors | has_any_deposit
  OUTPUT: boolean
  RETURN op = remove_active_deposit_id  // O(n) linear scan
      OR (op = get_depositors AND limit > MAX_DEPOSITORS_PAGE_SIZE)
      OR (op = has_any_deposit AND reads mismatched key)
END FUNCTION

// Fix Checking
FOR ALL op WHERE isBugCondition_B2(op) DO
  cost ← measure_ledger_reads(op')
  ASSERT cost = O(1)  // swap-and-pop; capped limit; single key read
END FOR

// Preservation Checking
FOR ALL op WHERE NOT isBugCondition_B2(op) DO
  ASSERT op(state) = op'(state)  // same results, reduced overhead
END FOR
```

### Bug 3 — Stale Duplicate Artifact

```pascal
FUNCTION isBugCondition_B3(source)
  INPUT: source = storage.rs OR contract.rs
  OUTPUT: boolean
  RETURN contains_duplicate_get_deposit_readonly_stub(source)
      OR contains_dead_single_arg_call(source)
END FUNCTION

// Fix Checking
FOR ALL source WHERE isBugCondition_B3(source) DO
  result ← compile(source')
  ASSERT result = Ok(())
END FOR

// Preservation Checking
FOR ALL call = get_deposit_readonly(env, depositor, deposit_id) DO
  ASSERT get_deposit_readonly(call) = get_deposit_readonly'(call)
END FOR
```

### Bug 4 — Ambiguous Initialization Validation

```pascal
FUNCTION isBugCondition_B4(call)
  INPUT: call = initialize(admin, fee_recipient, max_deposit, max_lock_secs)
  OUTPUT: boolean
  RETURN max_lock_secs = Some(0)
END FUNCTION

// Fix Checking
FOR ALL call WHERE isBugCondition_B4(call) DO
  result ← initialize'(call)
  ASSERT result = Err(InvalidConfig)  // or Err(InvalidAmount)
  ASSERT result ≠ Err(LockDurationTooLong)
END FOR

// Preservation Checking
FOR ALL call WHERE NOT isBugCondition_B4(call) DO
  ASSERT initialize(call) = initialize'(call)
END FOR
```
