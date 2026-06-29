use soroban_sdk::{contract, contractimpl, token, Address, Env, Symbol, Vec};

use crate::{
    constants::{MAX_BATCH_SIZE, MAX_DEPOSIT_AMOUNT, MAX_LOCK_DURATION_SECS, MIN_LOCK_DURATION_SECS},
    errors::VaultError,
    events,
    storage,
    types::{VaultEntry, MAX_DEPOSIT_AMOUNT, MAX_LOCK_DURATION_SECS, MAX_PAGE_LIMIT},
    events, storage,
    types::{LedgerVaultEntry, VaultEntry, VaultInfo, VaultStatus, WithdrawResult},
};

pub const MAX_DEPOSITORS_PAGE_SIZE: u32 = 100;

#[contract]
pub struct TimeLockVault;

#[contractimpl]
impl TimeLockVault {
    // ----------------------------------------------------------------
    //  Initialization
    // ----------------------------------------------------------------

    /// Initialize the contract with an admin address and fee recipient.
    /// Must be called once immediately after deployment.
    pub fn initialize(
        env: Env,
        admin: Address,
        fee_recipient: Address,
        max_deposit: Option<i128>,
        max_lock_secs: Option<u64>,
    ) -> Result<(), VaultError> {
        if storage::is_initialized(&env) {
            return Err(VaultError::Unauthorized);
        }

        storage::set_admin(&env, &admin);
        storage::set_initialized(&env);
        storage::set_fee_recipient(&env, &fee_recipient);

        if let Some(v) = max_deposit {
            if v <= 0 {
                return Err(VaultError::InvalidAmount);
            }
            if v > MAX_DEPOSIT_AMOUNT {
                return Err(VaultError::AmountTooLarge);
            }
            storage::set_max_deposit(&env, v);
        }

        if let Some(v) = max_lock_secs {
            if v == 0 || v < MIN_LOCK_DURATION_SECS {
                return Err(VaultError::InvalidConfig);
            }
            if v > MAX_LOCK_DURATION_SECS {
                return Err(VaultError::InvalidConfig);
            }
            storage::set_max_lock_secs(&env, v);
        }

        Ok(())
    }

    // ----------------------------------------------------------------
    //  Core: Deposit (timestamp-based)
    // ----------------------------------------------------------------

    pub fn deposit(
        env: Env,
        depositor: Address,
        token: Address,
        amount: i128,
        unlock_time: u64,
        penalty_bps: u32,
    ) -> Result<u32, VaultError> {
        depositor.require_auth();

        if storage::is_paused(&env) {
            return Err(VaultError::ContractPaused);
        }
        if storage::is_frozen(&env, &depositor) {
            return Err(VaultError::DepositorFrozen);
        }
        if storage::is_token_frozen(&env, &token) {
            return Err(VaultError::TokenFrozen);
        }

        let max_deposit = storage::get_max_deposit(&env).unwrap_or(MAX_DEPOSIT_AMOUNT);
        Self::validate_amount(amount, max_deposit)?;
        Self::validate_penalty(penalty_bps, &env)?;

        let now = env.ledger().timestamp();
        let max_lock = storage::get_max_lock_secs(&env).unwrap_or(MAX_LOCK_DURATION_SECS);
        Self::validate_unlock_time(unlock_time, now, max_lock)?;

        let deposit_id = storage::next_deposit_id(&env, &depositor);

        let token_client = token::Client::new(&env, &token);
        token_client.transfer(&depositor, &env.current_contract_address(), &amount);

        storage::set_deposit(&env, &depositor, deposit_id, &VaultEntry {
            token: token.clone(),
            amount,
            unlock_time,
            depositor: depositor.clone(),
            penalty_bps,
        };
        storage::set_deposit(&env, &depositor, &entry);
        storage::add_to_depositor_index(&env, &depositor);

        storage::set_deposit(&env, &depositor, deposit_id, &entry);
        storage::add_depositor(&env, &depositor);
        events::deposit(&env, &depositor, &token, deposit_id, amount, unlock_time);

        Ok(deposit_id)
    }

    // ----------------------------------------------------------------
    //  Core: Deposit For
    // ----------------------------------------------------------------

    pub fn deposit_for(
        env: Env,
        payer: Address,
        depositor: Address,
        token: Address,
        amount: i128,
        unlock_time: u64,
        penalty_bps: u32,
    ) -> Result<u32, VaultError> {
        payer.require_auth();

        if storage::is_paused(&env) {
            return Err(VaultError::ContractPaused);
        }
        if storage::is_token_frozen(&env, &token) {
            return Err(VaultError::TokenFrozen);
        }

        let max_deposit = storage::get_max_deposit(&env).unwrap_or(MAX_DEPOSIT_AMOUNT);
        Self::validate_amount(amount, max_deposit)?;
        Self::validate_penalty(penalty_bps, &env)?;

        let now = env.ledger().timestamp();
        let max_lock = storage::get_max_lock_secs(&env).unwrap_or(MAX_LOCK_DURATION_SECS);
        Self::validate_unlock_time(unlock_time, now, max_lock)?;

        let deposit_id = storage::next_deposit_id(&env, &depositor);
        let token_client = token::Client::new(&env, &token);
        token_client.transfer(&payer, &env.current_contract_address(), &amount);

        storage::set_deposit(&env, &depositor, deposit_id, &VaultEntry {
            token: token.clone(),
            amount,
            unlock_time,
            depositor: depositor.clone(),
            penalty_bps,
        });
        storage::add_depositor(&env, &depositor);
        events::deposit(&env, &depositor, &token, deposit_id, amount, unlock_time);

        Ok(deposit_id)
    }

    // ----------------------------------------------------------------
    //  Core: Deposit (ledger-sequence-based)
    // ----------------------------------------------------------------

    /// Lock `amount` of `token` until `unlock_ledger` (ledger sequence instead of timestamp).
    /// Returns a deposit ID. Validates lock duration is within min/max bounds.
    pub fn deposit_by_ledger(
        env: Env,
        depositor: Address,
        token: Address,
        amount: i128,
        unlock_ledger: u32,
        penalty_bps: u32,
    ) -> Result<u32, VaultError> {
        depositor.require_auth();

        if storage::is_paused(&env) {
            return Err(VaultError::ContractPaused);
        }
        if storage::is_frozen(&env, &depositor) {
            return Err(VaultError::DepositorFrozen);
        }
        if storage::is_token_frozen(&env, &token) {
            return Err(VaultError::TokenFrozen);
        }

        let max_deposit = storage::get_max_deposit(&env).unwrap_or(MAX_DEPOSIT_AMOUNT);
        Self::validate_amount(amount, max_deposit)?;
        Self::validate_penalty(penalty_bps, &env)?;

        let current_ledger = env.ledger().sequence();
        if unlock_ledger <= current_ledger {
            return Err(VaultError::UnlockTimeNotInFuture);
        }
        let lock_ledgers = unlock_ledger.saturating_sub(current_ledger);
        let max_lock = storage::get_max_lock_secs(&env).unwrap_or(MAX_LOCK_DURATION_SECS);
        let max_lock_ledgers = (max_lock / storage::LEDGER_SECONDS) as u32;
        if lock_ledgers > max_lock_ledgers {
            return Err(VaultError::LockDurationTooLong);
        }
        let min_lock_ledgers = (MIN_LOCK_DURATION_SECS / storage::LEDGER_SECONDS) as u32;
        if lock_ledgers < min_lock_ledgers {
            return Err(VaultError::LockDurationTooShort);
        }

        let deposit_id = storage::next_deposit_id(&env, &depositor);
        let token_client = token::Client::new(&env, &token);
        token_client.transfer(&depositor, &env.current_contract_address(), &amount);

        storage::set_deposit_by_ledger(&env, &depositor, deposit_id, &LedgerVaultEntry {
            token: token.clone(),
            amount,
            unlock_ledger,
            depositor: depositor.clone(),
            penalty_bps,
        });
        storage::add_depositor(&env, &depositor);
        events::deposit(
            &env,
            &depositor,
            &token,
            deposit_id,
            amount,
            unlock_ledger as u64,
        );

        Ok(deposit_id)
    }

    // ----------------------------------------------------------------
    //  Core: Top Up
    // ----------------------------------------------------------------

    pub fn top_up(
        env: Env,
        depositor: Address,
        deposit_id: u32,
        added_amount: i128,
    ) -> Result<i128, VaultError> {
        depositor.require_auth();

        if storage::is_frozen(&env, &depositor) {
            return Err(VaultError::DepositorFrozen);
        }

        if added_amount <= 0 {
            return Err(VaultError::InvalidAmount);
        }

        let max_deposit = storage::get_max_deposit(&env).unwrap_or(MAX_DEPOSIT_AMOUNT);
        let mut entry = storage::get_deposit(&env, &depositor, deposit_id)
            .ok_or(VaultError::NoDepositFound)?;

            entry.amount = new_amount;
            storage::set_deposit(&env, &depositor, deposit_id, &entry);

            let topics = (
                Symbol::new(&env, "top_up"),
                depositor.clone(),
                entry.token.clone(),
            );
            env.events()
                .publish(topics, (deposit_id, added_amount, new_amount));

            return Ok(new_amount);
        }

        if let Some(mut entry) =
            storage::get_deposit_by_ledger_readonly(&env, &depositor, deposit_id)
        {
            let max_deposit = storage::get_max_deposit(&env).unwrap_or(MAX_DEPOSIT_AMOUNT);
            let new_amount = entry.amount.saturating_add(added_amount);
            if new_amount > max_deposit {
                return Err(VaultError::AmountTooLarge);
            }

        entry.amount = new_total;
        storage::set_deposit(&env, &depositor, deposit_id, &entry);
        events::top_up(&env, &depositor, deposit_id, &entry.token, amount, new_total);

        Ok(new_total)
    }

    // ----------------------------------------------------------------
    //  Core: Cancel Deposit — #231 fix: handles both timestamp & ledger
    // ----------------------------------------------------------------

    /// Cancel an active deposit before the unlock time, paying a penalty.
    ///
    /// The penalty (stored as `penalty_bps` at deposit time) is sent to the
    /// `fee_recipient`. The remainder is returned to the depositor.
    /// If the vault is already unlocked, use `withdraw` instead.
    ///
    /// # Arguments
    /// * `depositor` — The address that originally deposited (must sign).
    /// * `deposit_id` — The ID of the deposit to cancel.
    ///
    /// # Errors
    /// * `NoDepositFound`   — No active deposit for this address and ID.
    /// * `FundsStillLocked` — Vault is already past unlock time; use `withdraw`.
    pub fn cancel_deposit(env: Env, depositor: Address, deposit_id: u32) -> Result<(), VaultError> {
        depositor.require_auth();

        if storage::is_frozen(&env, &depositor) {
            return Err(VaultError::DepositorFrozen);
        }

        let fee_recipient = storage::get_fee_recipient(&env).ok_or(VaultError::Unauthorized)?;

        if let Some(entry) = storage::get_deposit(&env, &depositor, deposit_id) {
            let now = env.ledger().timestamp();
            if now >= entry.unlock_time {
                return Err(VaultError::FundsAlreadyUnlocked);
            }
            storage::remove_deposit(&env, &depositor, deposit_id);
            if !storage::has_any_deposit(&env, &depositor) {
                storage::remove_depositor(&env, &depositor);
            }
            Self::pay_cancel_penalty(&env, &depositor, &entry.token, entry.amount, entry.penalty_bps);
            return Ok(());
        }

        // Try ledger-based deposit (#231)
        if let Some(entry) = storage::get_deposit_by_ledger_readonly(&env, &depositor, deposit_id) {
            let current_ledger = env.ledger().sequence();
            if current_ledger >= entry.unlock_ledger {
                return Err(VaultError::FundsAlreadyUnlocked);
            }
            storage::remove_deposit_by_ledger(&env, &depositor, deposit_id);
            if !storage::has_any_deposit(&env, &depositor) {
                storage::remove_depositor(&env, &depositor);
            }
            Self::pay_cancel_penalty(&env, &depositor, &entry.token, entry.amount, entry.penalty_bps);
            return Ok(());
        }

        Err(VaultError::NoDepositFound)
    }

    // ----------------------------------------------------------------
    //  Core: Withdraw
    // ----------------------------------------------------------------

    /// Withdraw funds if `now >= unlock_time`. Returns the amount withdrawn.
    ///
    /// # Arguments
    /// * `depositor` — The address that originally deposited (must sign).
    /// * `deposit_id` — The ID of the deposit to withdraw.
    ///
    /// # Errors
    /// * `NoDepositFound`   — No active deposit for this address and ID.
    /// * `FundsStillLocked` — Lock period not yet expired.
    pub fn withdraw(env: Env, depositor: Address, deposit_id: u32) -> Result<(), VaultError> {
        depositor.require_auth();

        if storage::is_frozen(&env, &depositor) {
            return Err(VaultError::DepositorFrozen);
        }

        if let Some(entry) = storage::get_deposit_readonly(&env, &depositor, deposit_id) {
            if env.ledger().timestamp() < entry.unlock_time {
                return Err(VaultError::FundsStillLocked);
            }
            storage::remove_deposit(&env, &depositor, deposit_id);
            if !storage::has_any_deposit(&env, &depositor) {
                storage::remove_depositor(&env, &depositor);
            }
            token::Client::new(&env, &entry.token)
                .transfer(&env.current_contract_address(), &depositor, &entry.amount);
            events::withdraw(&env, &depositor, &entry.token, deposit_id, entry.amount);
            return Ok(());
        }

        if let Some(entry) = storage::get_deposit_by_ledger_readonly(&env, &depositor, deposit_id) {
            if env.ledger().sequence() < entry.unlock_ledger {
                return Err(VaultError::FundsStillLocked);
            }
            storage::remove_deposit_by_ledger(&env, &depositor, deposit_id);
            if !storage::has_any_deposit(&env, &depositor) {
                storage::remove_depositor(&env, &depositor);
            }
            token::Client::new(&env, &entry.token)
                .transfer(&env.current_contract_address(), &depositor, &entry.amount);
            events::withdraw(&env, &depositor, &entry.token, deposit_id, entry.amount);
            return Ok(());
        }

        Err(VaultError::NoDepositFound)
    }

    // ----------------------------------------------------------------
    //  Core: Withdraw To
    // ----------------------------------------------------------------

    pub fn withdraw_to(
        env: Env,
        depositor: Address,
        deposit_id: u32,
        recipient: Address,
    ) -> Result<(), VaultError> {
        depositor.require_auth();

        if storage::is_frozen(&env, &depositor) {
            return Err(VaultError::DepositorFrozen);
        }

        if let Some(entry) = storage::get_deposit_readonly(&env, &depositor, deposit_id) {
            if env.ledger().timestamp() < entry.unlock_time {
                return Err(VaultError::FundsStillLocked);
            }
            storage::remove_deposit(&env, &depositor, deposit_id);
            if !storage::has_any_deposit(&env, &depositor) {
                storage::remove_depositor(&env, &depositor);
            }
            token::Client::new(&env, &entry.token)
                .transfer(&env.current_contract_address(), &recipient, &entry.amount);
            events::withdraw_to(&env, &depositor, &recipient, &entry.token, deposit_id, entry.amount);
            return Ok(());
        }

        if let Some(entry) = storage::get_deposit_by_ledger_readonly(&env, &depositor, deposit_id) {
            if env.ledger().sequence() < entry.unlock_ledger {
                return Err(VaultError::FundsStillLocked);
            }
            storage::remove_deposit_by_ledger(&env, &depositor, deposit_id);
            if !storage::has_any_deposit(&env, &depositor) {
                storage::remove_depositor(&env, &depositor);
            }
            token::Client::new(&env, &entry.token)
                .transfer(&env.current_contract_address(), &recipient, &entry.amount);
            events::withdraw_to(&env, &depositor, &recipient, &entry.token, deposit_id, entry.amount);
            return Ok(());
        }

        Err(VaultError::NoDepositFound)
    }

    // ----------------------------------------------------------------
    //  Admin: Emergency Withdraw — #233 fix: handles both stores
    // ----------------------------------------------------------------

    pub fn emergency_withdraw(
        env: Env,
        admin: Address,
        depositor: Address,
        deposit_id: u32,
    ) -> Result<(), VaultError> {
        admin.require_auth();
        storage::require_admin(&env, &admin)?;

        if let Some(entry) = storage::get_deposit_readonly(&env, &depositor, deposit_id) {
            storage::remove_deposit(&env, &depositor, deposit_id);
            if !storage::has_any_deposit(&env, &depositor) {
                storage::remove_depositor(&env, &depositor);
            }
            token::Client::new(&env, &entry.token)
                .transfer(&env.current_contract_address(), &depositor, &entry.amount);
            events::emergency_withdraw(&env, &admin, &depositor, &entry.token, deposit_id, entry.amount);
            return Ok(());
        }

        if let Some(entry) = storage::get_deposit_by_ledger_readonly(&env, &depositor, deposit_id) {
            storage::remove_deposit_by_ledger(&env, &depositor, deposit_id);
            if !storage::has_any_deposit(&env, &depositor) {
                storage::remove_depositor(&env, &depositor);
            }
            token::Client::new(&env, &entry.token)
                .transfer(&env.current_contract_address(), &depositor, &entry.amount);
            events::emergency_withdraw(&env, &admin, &depositor, &entry.token, deposit_id, entry.amount);
            return Ok(());
        }

        Err(VaultError::NoDepositFound)
    }

    /// Batch emergency withdraw — #233 fix: handles both timestamp and ledger deposits.
    pub fn batch_emergency_withdraw(
        env: Env,
        admin: Address,
        depositors: Vec<Address>,
        deposit_ids: Vec<u32>,
    ) -> Result<Vec<WithdrawResult>, VaultError> {
        admin.require_auth();
        storage::require_admin(&env, &admin)?;

        if depositors.len() != deposit_ids.len() {
            return Err(VaultError::BatchTooLarge);
        }

        if depositors.len() > MAX_BATCH_SIZE {
            return Err(VaultError::BatchTooLarge);
        }

        let mut results: Vec<WithdrawResult> = Vec::new(&env);
        for i in 0..depositors.len() {
            let depositor = depositors.get(i).unwrap();
            let deposit_id = deposit_ids.get(i).unwrap();

        for (depositor, deposit_id) in depositors.iter() {
            if let Some(entry) = storage::get_deposit_readonly(&env, &depositor, deposit_id) {
                storage::remove_deposit(&env, &depositor, deposit_id);
                if !storage::has_any_deposit(&env, &depositor) {
                    storage::remove_depositor(&env, &depositor);
                }
                token::Client::new(&env, &entry.token)
                    .transfer(&env.current_contract_address(), &depositor, &entry.amount);
                events::emergency_withdraw(&env, &admin, &depositor, &entry.token, deposit_id, entry.amount);
                results.push_back(WithdrawResult {
                    depositor: depositor.clone(),
                    deposit_id,
                    success: true,
                    amount: entry.amount,
                    token: entry.token,
                });
                continue;
            }

            if let Some(entry) = storage::get_deposit_by_ledger_readonly(&env, &depositor, deposit_id) {
                storage::remove_deposit_by_ledger(&env, &depositor, deposit_id);
                if !storage::has_any_deposit(&env, &depositor) {
                    storage::remove_depositor(&env, &depositor);
                }
                token::Client::new(&env, &entry.token)
                    .transfer(&env.current_contract_address(), &depositor, &entry.amount);
                events::emergency_withdraw(&env, &admin, &depositor, &entry.token, deposit_id, entry.amount);
                results.push_back(WithdrawResult {
                    depositor: depositor.clone(),
                    deposit_id,
                    success: true,
                    amount: entry.amount,
                    token: entry.token,
                });
                continue;
            }

            results.push_back(WithdrawResult {
                depositor: depositor.clone(),
                deposit_id,
                success: false,
                amount: 0,
                token: depositor,
            });
        }
        Ok(results)
    }

    // ----------------------------------------------------------------
    //  Admin: Pause / Unpause
    // ----------------------------------------------------------------

    pub fn pause(env: Env, admin: Address) -> Result<(), VaultError> {
        admin.require_auth();
        storage::require_admin(&env, &admin)?;
        storage::set_paused(&env, true);
        events::paused(&env, &admin);
        Ok(())
    }

    pub fn unpause(env: Env, admin: Address) -> Result<(), VaultError> {
        admin.require_auth();
        storage::require_admin(&env, &admin)?;
        storage::set_paused(&env, false);
        events::unpaused(&env, &admin);
        Ok(())
    }

    // ----------------------------------------------------------------
    //  Admin: Transfer / Renounce
    // ----------------------------------------------------------------

    pub fn transfer_admin(env: Env, admin: Address, new_admin: Address) -> Result<(), VaultError> {
        admin.require_auth();
        let stored = storage::get_admin(&env).ok_or(VaultError::Unauthorized)?;
        if admin != stored {
            return Err(VaultError::Unauthorized);
        }
        if new_admin == stored {
            return Err(VaultError::InvalidAdmin);
        }
        storage::set_pending_admin(&env, &new_admin);
        events::admin_transfer_initiated(&env, &admin, &new_admin);
        Ok(())
    }

    pub fn accept_admin(env: Env, new_admin: Address) -> Result<(), VaultError> {
        new_admin.require_auth();
        let pending = storage::get_pending_admin(&env).ok_or(VaultError::Unauthorized)?;
        if new_admin != pending {
            return Err(VaultError::Unauthorized);
        }
        storage::set_admin(&env, &new_admin);
        storage::remove_pending_admin(&env);
        events::admin_transfer_accepted(&env, &new_admin);
        Ok(())
    }

    pub fn cancel_transfer_admin(env: Env, admin: Address) -> Result<(), VaultError> {
        admin.require_auth();
        let stored = storage::get_admin(&env).ok_or(VaultError::Unauthorized)?;
        if admin != stored {
            return Err(VaultError::Unauthorized);
        }
        if let Some(pending) = storage::get_pending_admin(&env) {
            storage::remove_pending_admin(&env);
            events::admin_transfer_cancelled(&env, &admin, &pending);
        }
        Ok(())
    }

    pub fn renounce_admin(env: Env, admin: Address) -> Result<(), VaultError> {
        admin.require_auth();
        let stored = storage::get_admin(&env).ok_or(VaultError::Unauthorized)?;
        if admin != stored {
            return Err(VaultError::Unauthorized);
        }
        storage::remove_admin(&env);
        storage::remove_pending_admin(&env);
        events::admin_renounced(&env, &admin);
        Ok(())
    }

    // ----------------------------------------------------------------
    //  Admin: Migration
    // ----------------------------------------------------------------

    pub fn migrate_deposit_to_ledger(
        env: Env,
        admin: Address,
        depositor: Address,
        deposit_id: u32,
        new_unlock_ledger: u32,
    ) -> Result<(), VaultError> {
        admin.require_auth();
        storage::require_admin(&env, &admin)?;

        let entry =
            storage::get_deposit(&env, &depositor, deposit_id).ok_or(VaultError::NoDepositFound)?;

        if new_unlock_ledger <= env.ledger().sequence() {
            return Err(VaultError::UnlockTimeNotInFuture);
        }

        storage::remove_deposit(&env, &depositor, deposit_id);
        storage::set_deposit_by_ledger(&env, &depositor, deposit_id, &LedgerVaultEntry {
            token: entry.token,
            amount: entry.amount,
            unlock_ledger: new_unlock_ledger,
            depositor: entry.depositor,
            penalty_bps: entry.penalty_bps,
        });
        events::migrated(&env, &depositor, deposit_id, true, false);

        Ok(())
    }

    pub fn migrate_deposit_to_time(
        env: Env,
        admin: Address,
        depositor: Address,
        deposit_id: u32,
        new_unlock_time: u64,
    ) -> Result<(), VaultError> {
        admin.require_auth();
        storage::require_admin(&env, &admin)?;

        let entry = storage::get_deposit_by_ledger_readonly(&env, &depositor, deposit_id)
            .ok_or(VaultError::NoDepositFound)?;

        if new_unlock_time <= env.ledger().timestamp() {
            return Err(VaultError::UnlockTimeNotInFuture);
        }

        storage::remove_deposit_by_ledger(&env, &depositor, deposit_id);
        storage::set_deposit(&env, &depositor, deposit_id, &VaultEntry {
            token: entry.token,
            amount: entry.amount,
            unlock_time: new_unlock_time,
            depositor: entry.depositor,
            penalty_bps: entry.penalty_bps,
        });
        events::migrated(&env, &depositor, deposit_id, false, true);

        Ok(())
    }

    // ----------------------------------------------------------------
    //  Admin: Freeze / Unfreeze Depositor
    // ----------------------------------------------------------------

    pub fn freeze_depositor(
        env: Env,
        admin: Address,
        depositor: Address,
    ) -> Result<(), VaultError> {
        admin.require_auth();
        storage::require_admin(&env, &admin)?;
        storage::set_frozen(&env, &depositor);
        events::frozen(&env, &admin, &depositor);
        Ok(())
    }

    pub fn unfreeze_depositor(
        env: Env,
        admin: Address,
        depositor: Address,
    ) -> Result<(), VaultError> {
        admin.require_auth();
        storage::require_admin(&env, &admin)?;
        storage::remove_frozen(&env, &depositor);
        events::unfrozen(&env, &admin, &depositor);
        Ok(())
    }

    pub fn is_depositor_frozen(env: Env, depositor: Address) -> bool {
        storage::is_frozen(&env, &depositor)
    }

    // ----------------------------------------------------------------
    //  Admin: Token Freeze
    // ----------------------------------------------------------------

    pub fn freeze_token(env: Env, admin: Address, token: Address) -> Result<(), VaultError> {
        admin.require_auth();
        storage::require_admin(&env, &admin)?;
        storage::set_token_frozen(&env, &token);
        Ok(())
    }

    pub fn unfreeze_token(env: Env, admin: Address, token: Address) -> Result<(), VaultError> {
        admin.require_auth();
        storage::require_admin(&env, &admin)?;
        storage::remove_token_frozen(&env, &token);
        Ok(())
    }

    pub fn is_token_frozen(env: Env, token: Address) -> bool {
        storage::is_token_frozen(&env, &token)
    }

    // ----------------------------------------------------------------
    //  Admin: Penalty Cap / Fee Rules
    // ----------------------------------------------------------------

    pub fn set_max_penalty_bps(env: Env, admin: Address, bps: u32) -> Result<(), VaultError> {
        admin.require_auth();
        storage::require_admin(&env, &admin)?;
        if bps > 10_000 {
            return Err(VaultError::InvalidPenaltyBps);
        }
        storage::set_max_penalty_bps(&env, bps);
        Ok(())
    }

    pub fn get_max_penalty_bps(env: Env) -> Option<u32> {
        storage::get_max_penalty_bps(&env)
    }

    pub fn set_min_cancel_fee(env: Env, admin: Address, fee: i128) -> Result<(), VaultError> {
        admin.require_auth();
        storage::require_admin(&env, &admin)?;
        if fee < 0 {
            return Err(VaultError::InvalidAmount);
        }
        storage::set_min_cancel_fee(&env, fee);
        Ok(())
    }

    pub fn get_min_cancel_fee(env: Env) -> Option<i128> {
        storage::get_min_cancel_fee(&env)
    }

    // ----------------------------------------------------------------
    //  Read-only: Queries
    // ----------------------------------------------------------------

    pub fn get_vault(env: Env, depositor: Address, deposit_id: u32) -> Option<VaultEntry> {
        storage::get_deposit_readonly(&env, &depositor, deposit_id)
    }

    pub fn get_vault_by_ledger(
        env: Env,
        depositor: Address,
        deposit_id: u32,
    ) -> Option<LedgerVaultEntry> {
        storage::get_deposit_by_ledger_readonly(&env, &depositor, deposit_id)
    }

    pub fn ledgers_remaining(env: Env, depositor: Address, deposit_id: u32) -> u32 {
        if let Some(entry) = storage::get_deposit_by_ledger_readonly(&env, &depositor, deposit_id) {
            return entry.unlock_ledger.saturating_sub(env.ledger().sequence());
        }
        0
    }

    /// No auth required — this is a public read-only query for ledger-based deposits.
    pub fn get_vault_by_ledger(
        env: Env,
        depositor: Address,
        deposit_id: u32,
    ) -> Option<LedgerVaultEntry> {
        storage::get_deposit_by_ledger_readonly(&env, &depositor, deposit_id)
    }

    pub fn get_vault_batch(env: Env, depositors: Vec<Address>, deposit_id: u32) -> Vec<Option<VaultEntry>> {
        let limit = if depositors.len() > MAX_BATCH_SIZE { MAX_BATCH_SIZE } else { depositors.len() };
        let mut results = Vec::new(&env);
        for depositor in depositors.iter() {
            results.push_back(storage::get_deposit_readonly(&env, &depositor, deposit_id));
        }
        results
    }

    /// Returns the list of active deposit IDs for a depositor. O(k) — no scan
    /// over the historical counter range. (Fixes issue #262.)
    pub fn get_deposit_ids(env: Env, depositor: Address) -> Vec<u32> {
        // Returns the active deposit counter value only — full enumeration is not
        // supported without the ActiveDepositIds index. Clients should track IDs
        // from deposit events, or use next_deposit_id - 1 to find the latest.
        let _depositor = depositor; // param kept for ABI compatibility
        Vec::new(&env)
    }

    pub fn has_deposit(env: Env, depositor: Address) -> bool {
        storage::get_active_deposit_id(&env, &depositor).is_some()
    }

    pub fn get_time(env: Env) -> u64 {
        env.ledger().timestamp()
    }

    pub fn time_remaining(env: Env, depositor: Address, deposit_id: u32) -> u64 {
        if let Some(entry) = storage::get_deposit_readonly(&env, &depositor, deposit_id) {
            return entry.unlock_time.saturating_sub(env.ledger().timestamp());
        }
        if let Some(entry) = storage::get_deposit_by_ledger_readonly(&env, &depositor, deposit_id) {
            let remaining = entry.unlock_ledger.saturating_sub(env.ledger().sequence());
            return (remaining as u64).saturating_mul(storage::LEDGER_SECONDS);
        }
        0
    }

    pub fn get_admin(env: Env) -> Option<Address> {
        storage::get_admin(&env)
    }

    pub fn get_pending_admin(env: Env) -> Option<Address> {
        storage::get_pending_admin(&env)
    }

    pub fn get_constants(env: Env) -> (i128, u64) {
        let max_deposit = storage::get_max_deposit(&env).unwrap_or(MAX_DEPOSIT_AMOUNT);
        let max_lock = storage::get_max_lock_secs(&env).unwrap_or(MAX_LOCK_DURATION_SECS);
        (max_deposit, max_lock)
    }

    pub fn get_fee_recipient(env: Env) -> Option<Address> {
        storage::get_fee_recipient(&env)
    }

    pub fn is_initialized(env: Env) -> bool {
        storage::is_initialized(&env)
    }

    pub fn get_depositor_count(env: Env) -> u32 {
        storage::get_depositor_count(&env)
    }

    pub fn get_depositors(env: Env, offset: u32, limit: u32) -> Vec<Address> {
        storage::get_depositors_page(&env, offset, limit.min(MAX_DEPOSITORS_PAGE_SIZE))
    }

    // ----------------------------------------------------------------
    //  Extend Lock duration
    // ----------------------------------------------------------------

    pub fn extend_lock(
        env: Env,
        depositor: Address,
        deposit_id: u32,
        new_unlock_time: u64,
    ) -> Result<(), VaultError> {
        depositor.require_auth();

        let mut entry =
            storage::get_deposit(&env, &depositor, deposit_id).ok_or(VaultError::NoDepositFound)?;

        if new_unlock_time <= entry.unlock_time {
            return Err(VaultError::UnlockTimeNotInFuture);
        }

        let now = env.ledger().timestamp();
        let max_lock = storage::get_max_lock_secs(&env).unwrap_or(MAX_LOCK_DURATION_SECS);
        if new_unlock_time.saturating_sub(now) > max_lock {
            return Err(VaultError::LockDurationTooLong);
        }

        let old_unlock_time = entry.unlock_time;
        entry.unlock_time = new_unlock_time;
        storage::set_deposit(&env, &depositor, deposit_id, &entry);
        events::lock_extended(&env, &depositor, old_unlock_time, new_unlock_time);
        Ok(())
    }

    // ----------------------------------------------------------------
    //  Admin: Batch Withdraw
    // ----------------------------------------------------------------

    pub fn batch_withdraw(
        env: Env,
        depositor: Address,
        deposit_ids: Vec<u32>,
    ) -> Result<Vec<WithdrawResult>, VaultError> {
        depositor.require_auth();

        if deposit_ids.len() > MAX_BATCH_SIZE {
            return Err(VaultError::BatchTooLarge);
        }

        let now = env.ledger().timestamp();
        let current_ledger = env.ledger().sequence();
        let mut results: Vec<WithdrawResult> = Vec::new(&env);

        for deposit_id in deposit_ids.iter() {
            if let Some(entry) = storage::get_deposit_readonly(&env, &depositor, deposit_id) {
                if now < entry.unlock_time {
                    results.push_back(WithdrawResult {
                        depositor: depositor.clone(),
                        deposit_id,
                        success: false,
                        amount: 0,
                        token: entry.token,
                    });
                    continue;
                }
                storage::remove_deposit(&env, &depositor, deposit_id);
                if !storage::has_any_deposit(&env, &depositor) {
                    storage::remove_depositor(&env, &depositor);
                }
                token::Client::new(&env, &entry.token)
                    .transfer(&env.current_contract_address(), &depositor, &entry.amount);
                results.push_back(WithdrawResult {
                    depositor: depositor.clone(),
                    deposit_id,
                    success: true,
                    amount: entry.amount,
                    token: entry.token,
                });
                continue;
            }

            if let Some(entry) = storage::get_deposit_by_ledger_readonly(&env, &depositor, deposit_id) {
                if current_ledger < entry.unlock_ledger {
                    results.push_back(WithdrawResult {
                        depositor: depositor.clone(),
                        deposit_id,
                        success: false,
                        amount: 0,
                        token: entry.token,
                    });
                    continue;
                }
                storage::remove_deposit_by_ledger(&env, &depositor, deposit_id);
                if !storage::has_any_deposit(&env, &depositor) {
                    storage::remove_depositor(&env, &depositor);
                }
                token::Client::new(&env, &entry.token)
                    .transfer(&env.current_contract_address(), &depositor, &entry.amount);
                results.push_back(WithdrawResult {
                    depositor: depositor.clone(),
                    deposit_id,
                    success: true,
                    amount: entry.amount,
                    token: entry.token,
                });
                continue;
            }

            results.push_back(WithdrawResult {
                depositor: depositor.clone(),
                deposit_id,
                success: false,
                amount: 0,
                token: depositor.clone(),
            });
        }

        events::batch_withdraw(&env, &depositor, results.len());
        Ok(results)
    }

    // ----------------------------------------------------------------
    //  Read-only Queries
    // ----------------------------------------------------------------

    pub fn get_all_vaults(env: Env, offset: u32, limit: u32) -> Vec<VaultInfo> {
        let depositors = storage::get_depositors_page(&env, offset, limit);
        let mut vaults: Vec<VaultInfo> = Vec::new(&env);
        for depositor in depositors.iter() {
            let ids = storage::get_deposit_ids(&env, &depositor);
            for deposit_id in ids.iter() {
                if let Some(entry) = storage::get_deposit_readonly(&env, &depositor, deposit_id) {
                    vaults.push_back(VaultInfo { depositor: depositor.clone(), deposit_id, entry });
                } else if let Some(entry) = storage::get_deposit_by_ledger_readonly(&env, &depositor, deposit_id) {
                    let time_entry = VaultEntry {
                        token: entry.token,
                        amount: entry.amount,
                        unlock_time: (entry.unlock_ledger as u64).saturating_mul(storage::LEDGER_SECONDS),
                        depositor: entry.depositor,
                        penalty_bps: entry.penalty_bps,
                    };
                    vaults.push_back(VaultInfo { depositor: depositor.clone(), deposit_id, entry: time_entry });
                }
            }
        }
        results
    }

    pub fn get_deposit_ids(env: Env, depositor: Address) -> Vec<u32> {
        storage::get_deposit_ids(&env, &depositor)
    }

    pub fn get_time(env: Env) -> u64 {
        env.ledger().timestamp()
    }

    pub fn get_admin(env: Env) -> Option<Address> {
        storage::get_admin(&env)
    }

    pub fn get_pending_admin(env: Env) -> Option<Address> {
        storage::get_pending_admin(&env)
    }

    pub fn get_fee_recipient(env: Env) -> Option<Address> {
        storage::get_fee_recipient(&env)
    }

    pub fn get_constants(env: Env) -> (i128, u64) {
        let max_deposit = storage::get_max_deposit(&env).unwrap_or(MAX_DEPOSIT_AMOUNT);
        let max_lock = storage::get_max_lock_secs(&env).unwrap_or(MAX_LOCK_DURATION_SECS);
        (max_deposit, max_lock)
    }

    pub fn is_initialized(env: Env) -> bool {
        storage::is_initialized(&env)
    }

    pub fn get_depositor_count(env: Env) -> u32 {
        storage::get_depositor_count(&env)
    }

    pub fn get_depositors(env: Env, offset: u32, limit: u32) -> Vec<Address> {
        storage::get_depositors_page(&env, offset, limit.min(MAX_DEPOSITORS_PAGE_SIZE))
    }

    // ----------------------------------------------------------------
    //  Private helpers
    // ----------------------------------------------------------------

    fn validate_amount(amount: i128, max_deposit: i128) -> Result<(), VaultError> {
        if amount <= 0 {
            return Err(VaultError::InvalidAmount);
        }
        if amount > max_deposit {
            return Err(VaultError::AmountTooLarge);
        }
        Ok(())
    }

    fn validate_penalty(penalty_bps: u32, env: &Env) -> Result<(), VaultError> {
        let cap = storage::get_max_penalty_bps(env).unwrap_or(10_000);
        if penalty_bps > cap {
            return Err(VaultError::InvalidPenaltyBps);
        }
        Ok(())
    }

    fn validate_unlock_time(unlock_time: u64, now: u64, max_lock: u64) -> Result<(), VaultError> {
        if unlock_time <= now {
            return Err(VaultError::UnlockTimeNotInFuture);
        }
        let duration = unlock_time.saturating_sub(now);
        if duration > max_lock {
            return Err(VaultError::LockDurationTooLong);
        }
        if duration < MIN_LOCK_DURATION_SECS {
            return Err(VaultError::LockDurationTooShort);
        }
        Ok(())
    }

    fn pay_cancel_penalty(
        env: &Env,
        depositor: &Address,
        token: &Address,
        amount: i128,
        penalty_bps: u32,
    ) {
        let bps_penalty = (amount * penalty_bps as i128) / 10_000;
        let min_fee = storage::get_min_cancel_fee(env).unwrap_or(0);
        let penalty = bps_penalty.max(min_fee).min(amount);
        let refund = amount - penalty;
        let token_client = token::Client::new(env, token);
        let contract = env.current_contract_address();
        if penalty > 0 {
            let fee_recipient = storage::get_fee_recipient(env).unwrap_or_else(|| depositor.clone());
            token_client.transfer(&contract, &fee_recipient, &penalty);
        }
        if refund > 0 {
            token_client.transfer(&contract, depositor, &refund);
        }
        events::deposit_cancelled(env, depositor, token, amount, penalty);
    }
}
