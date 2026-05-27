#![cfg(test)]

extern crate std;

use soroban_sdk::{
    testutils::{Address as _, Ledger, LedgerInfo},
    token::{Client as TokenClient, StellarAssetClient},
    Address, Env,
};

use crate::{
    contract::{TimeLockVault, TimeLockVaultClient},
    errors::VaultError,
    types::{MAX_DEPOSIT_AMOUNT, MAX_LOCK_DURATION_SECS},
};

// ================================================================
//  Test helpers
// ================================================================

fn setup() -> (Env, TimeLockVaultClient<'static>, Address, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let vault_id = env.register(TimeLockVault, ());
    let vault = TimeLockVaultClient::new(&env, &vault_id);

    let admin: Address = Address::generate(&env);
    let alice: Address = Address::generate(&env);

    let token_id = env.register_stellar_asset_contract_v2(admin.clone());
    let token_address = token_id.address();

    let asset_client = StellarAssetClient::new(&env, &token_address);
    asset_client.mint(&alice, &10_000);

    vault.initialize(&admin);

    (env, vault, token_address, admin, alice)
}

fn advance_time(env: &Env, seconds: u64) {
    env.ledger().set(LedgerInfo {
        timestamp: env.ledger().timestamp() + seconds,
        protocol_version: env.ledger().protocol_version(),
        sequence_number: env.ledger().sequence(),
        network_id: Default::default(),
        base_reserve: 10,
        min_temp_entry_ttl: 16,
        min_persistent_entry_ttl: 4096,
        max_entry_ttl: 33_000_000,
    });
}

// ================================================================
//  Initialization
// ================================================================

#[test]
fn test_initialize_sets_admin() {
    let (_env, vault, _token, admin, _alice) = setup();
    assert_eq!(vault.get_admin(), Some(admin));
}

#[test]
fn test_double_initialize_fails() {
    let (_env, vault, _token, admin, _alice) = setup();
    let result = vault.try_initialize(&admin);
    assert_eq!(result, Err(Ok(VaultError::Unauthorized)));
}

#[test]
fn test_is_initialized() {
    let env = Env::default();
    env.mock_all_auths();

    let vault_id = env.register(TimeLockVault, ());
    let vault = TimeLockVaultClient::new(&env, &vault_id);
    let admin: Address = Address::generate(&env);

    assert!(!vault.is_initialized());
    vault.initialize(&admin);
    assert!(vault.is_initialized());

    vault.renounce_admin(&admin);
    assert!(vault.is_initialized());
}

// ================================================================
//  Deposit — happy path
// ================================================================

#[test]
fn test_deposit_success() {
    let (env, vault, token, _admin, alice) = setup();

    let unlock_time = env.ledger().timestamp() + 3600;
    let id = vault.deposit(&alice, &token, &1_000, &unlock_time);

    assert_eq!(id, 0);
    let entry = vault.get_vault(&alice, &id).expect("entry should exist");
    assert_eq!(entry.amount, 1_000);
    assert_eq!(entry.unlock_time, unlock_time);
    assert_eq!(entry.token, token);
    assert_eq!(entry.depositor, alice);
}

#[test]
fn test_deposit_transfers_tokens_to_contract() {
    let (env, vault, token, _admin, alice) = setup();
    let token_client = TokenClient::new(&env, &token);

    let unlock_time = env.ledger().timestamp() + 3600;
    vault.deposit(&alice, &token, &1_000, &unlock_time);

    assert_eq!(token_client.balance(&alice), 9_000);
}

// ================================================================
//  Deposit — validation errors
// ================================================================

#[test]
fn test_deposit_zero_amount_fails() {
    let (env, vault, token, _admin, alice) = setup();
    let unlock_time = env.ledger().timestamp() + 3600;
    let result = vault.try_deposit(&alice, &token, &0, &unlock_time);
    assert_eq!(result, Err(Ok(VaultError::InvalidAmount)));
}

#[test]
fn test_deposit_negative_amount_fails() {
    let (env, vault, token, _admin, alice) = setup();
    let unlock_time = env.ledger().timestamp() + 3600;
    let result = vault.try_deposit(&alice, &token, &-1, &unlock_time);
    assert_eq!(result, Err(Ok(VaultError::InvalidAmount)));
}

#[test]
fn test_deposit_amount_exceeds_max_fails() {
    let (env, vault, token, _admin, alice) = setup();
    let asset_client = StellarAssetClient::new(&env, &token);
    asset_client.mint(&alice, &MAX_DEPOSIT_AMOUNT);

    let unlock_time = env.ledger().timestamp() + 3600;
    let result = vault.try_deposit(&alice, &token, &(MAX_DEPOSIT_AMOUNT + 1), &unlock_time);
    assert_eq!(result, Err(Ok(VaultError::AmountTooLarge)));
}

#[test]
fn test_deposit_at_max_amount_succeeds() {
    let (env, vault, token, _admin, alice) = setup();
    let asset_client = StellarAssetClient::new(&env, &token);
    asset_client.mint(&alice, &MAX_DEPOSIT_AMOUNT);

    let unlock_time = env.ledger().timestamp() + 3600;
    vault.deposit(&alice, &token, &MAX_DEPOSIT_AMOUNT, &unlock_time);

    let entry = vault.get_vault(&alice, &0).expect("entry should exist");
    assert_eq!(entry.amount, MAX_DEPOSIT_AMOUNT);
}

#[test]
fn test_deposit_past_unlock_time_fails() {
    let (env, vault, token, _admin, alice) = setup();
    let unlock_time = env.ledger().timestamp();
    let result = vault.try_deposit(&alice, &token, &1_000, &unlock_time);
    assert_eq!(result, Err(Ok(VaultError::UnlockTimeNotInFuture)));
}

#[test]
fn test_deposit_unlock_time_in_past_fails() {
    let (env, vault, token, _admin, alice) = setup();
    let unlock_time = env.ledger().timestamp().saturating_sub(1);
    let result = vault.try_deposit(&alice, &token, &1_000, &unlock_time);
    assert_eq!(result, Err(Ok(VaultError::UnlockTimeNotInFuture)));
}

#[test]
fn test_deposit_lock_duration_too_long_fails() {
    let (env, vault, token, _admin, alice) = setup();
    let unlock_time = env.ledger().timestamp() + MAX_LOCK_DURATION_SECS + 1;
    let result = vault.try_deposit(&alice, &token, &1_000, &unlock_time);
    assert_eq!(result, Err(Ok(VaultError::LockDurationTooLong)));
}

#[test]
fn test_deposit_at_max_duration_succeeds() {
    let (env, vault, token, _admin, alice) = setup();
    let unlock_time = env.ledger().timestamp() + MAX_LOCK_DURATION_SECS;
    vault.deposit(&alice, &token, &1_000, &unlock_time);
    assert!(vault.get_vault(&alice, &0).is_some());
}

// ================================================================
//  Multiple concurrent deposits
// ================================================================

#[test]
fn test_multiple_deposits_same_address() {
    let (env, vault, token, _admin, alice) = setup();
    let asset_client = StellarAssetClient::new(&env, &token);
    asset_client.mint(&alice, &5_000);

    let t1 = env.ledger().timestamp() + 3600;
    let t2 = env.ledger().timestamp() + 7200;
    let t3 = env.ledger().timestamp() + 10800;

    let id0 = vault.deposit(&alice, &token, &1_000, &t1);
    let id1 = vault.deposit(&alice, &token, &2_000, &t2);
    let id2 = vault.deposit(&alice, &token, &3_000, &t3);

    assert_eq!(id0, 0);
    assert_eq!(id1, 1);
    assert_eq!(id2, 2);

    assert_eq!(vault.get_vault(&alice, &0).unwrap().amount, 1_000);
    assert_eq!(vault.get_vault(&alice, &1).unwrap().amount, 2_000);
    assert_eq!(vault.get_vault(&alice, &2).unwrap().amount, 3_000);
}

#[test]
fn test_get_deposit_ids_returns_active_ids() {
    let (env, vault, token, _admin, alice) = setup();
    let asset_client = StellarAssetClient::new(&env, &token);
    asset_client.mint(&alice, &3_000);

    let t1 = env.ledger().timestamp() + 3600;
    let t2 = env.ledger().timestamp() + 7200;

    vault.deposit(&alice, &token, &1_000, &t1);
    vault.deposit(&alice, &token, &2_000, &t2);

    let ids = vault.get_deposit_ids(&alice);
    assert_eq!(ids.len(), 2);
    assert_eq!(ids.get(0).unwrap(), 0);
    assert_eq!(ids.get(1).unwrap(), 1);
}

#[test]
fn test_partial_withdrawal_leaves_other_deposits_intact() {
    let (env, vault, token, _admin, alice) = setup();
    let asset_client = StellarAssetClient::new(&env, &token);
    asset_client.mint(&alice, &3_000);
    let token_client = TokenClient::new(&env, &token);

    let t1 = env.ledger().timestamp() + 3600;
    let t2 = env.ledger().timestamp() + 7200;

    vault.deposit(&alice, &token, &1_000, &t1);
    vault.deposit(&alice, &token, &2_000, &t2);

    // Withdraw only deposit 0
    advance_time(&env, 3601);
    vault.withdraw(&alice, &0);

    // Deposit 0 gone, deposit 1 still there
    assert!(vault.get_vault(&alice, &0).is_none());
    assert!(vault.get_vault(&alice, &1).is_some());
    assert_eq!(vault.get_vault(&alice, &1).unwrap().amount, 2_000);

    // get_deposit_ids only returns active ones
    let ids = vault.get_deposit_ids(&alice);
    assert_eq!(ids.len(), 1);
    assert_eq!(ids.get(0).unwrap(), 1);

    // Alice got back 1_000 (started with 10_000, minted 3_000, deposited 3_000, withdrew 1_000)
    assert_eq!(token_client.balance(&alice), 10_000 + 3_000 - 3_000 + 1_000);
}

#[test]
fn test_deposits_have_independent_unlock_times() {
    let (env, vault, token, _admin, alice) = setup();
    let asset_client = StellarAssetClient::new(&env, &token);
    asset_client.mint(&alice, &2_000);

    let t1 = env.ledger().timestamp() + 3600;
    let t2 = env.ledger().timestamp() + 7200;

    vault.deposit(&alice, &token, &1_000, &t1);
    vault.deposit(&alice, &token, &1_000, &t2);

    advance_time(&env, 3601);

    // Deposit 0 unlocked, deposit 1 still locked
    vault.withdraw(&alice, &0);
    let result = vault.try_withdraw(&alice, &1);
    assert_eq!(result, Err(Ok(VaultError::FundsStillLocked)));
}

#[test]
fn test_deposit_ids_increment_after_withdrawal() {
    let (env, vault, token, _admin, alice) = setup();
    let asset_client = StellarAssetClient::new(&env, &token);
    asset_client.mint(&alice, &3_000);

    let t1 = env.ledger().timestamp() + 3600;
    let id0 = vault.deposit(&alice, &token, &1_000, &t1);
    assert_eq!(id0, 0);

    advance_time(&env, 3601);
    vault.withdraw(&alice, &0);

    // New deposit gets id=1 (counter never resets)
    let t2 = env.ledger().timestamp() + 3600;
    let id1 = vault.deposit(&alice, &token, &1_000, &t2);
    assert_eq!(id1, 1);
}

// ================================================================
//  Withdraw — happy path
// ================================================================

#[test]
fn test_withdraw_after_unlock_succeeds() {
    let (env, vault, token, _admin, alice) = setup();
    let token_client = TokenClient::new(&env, &token);

    let unlock_time = env.ledger().timestamp() + 3600;
    vault.deposit(&alice, &token, &1_000, &unlock_time);
    assert_eq!(token_client.balance(&alice), 9_000);

    advance_time(&env, 3601);
    vault.withdraw(&alice, &0);

    assert!(vault.get_vault(&alice, &0).is_none());
    assert_eq!(token_client.balance(&alice), 10_000);
}

#[test]
fn test_withdraw_exactly_at_unlock_time_succeeds() {
    let (env, vault, token, _admin, alice) = setup();
    let unlock_time = env.ledger().timestamp() + 3600;
    vault.deposit(&alice, &token, &1_000, &unlock_time);

    advance_time(&env, 3600);
    vault.withdraw(&alice, &0);

    assert!(vault.get_vault(&alice, &0).is_none());
}

// ================================================================
//  Withdraw — error paths
// ================================================================

#[test]
fn test_withdraw_before_unlock_fails() {
    let (env, vault, token, _admin, alice) = setup();
    let unlock_time = env.ledger().timestamp() + 3600;
    vault.deposit(&alice, &token, &1_000, &unlock_time);

    advance_time(&env, 1800);

    let result = vault.try_withdraw(&alice, &0);
    assert_eq!(result, Err(Ok(VaultError::FundsStillLocked)));
}

#[test]
fn test_withdraw_no_deposit_fails() {
    let (_env, vault, _token, _admin, alice) = setup();
    let result = vault.try_withdraw(&alice, &0);
    assert_eq!(result, Err(Ok(VaultError::NoDepositFound)));
}

// ================================================================
//  Time helpers
// ================================================================

#[test]
fn test_time_remaining_before_unlock() {
    let (env, vault, token, _admin, alice) = setup();
    let unlock_time = env.ledger().timestamp() + 3600;
    vault.deposit(&alice, &token, &1_000, &unlock_time);

    advance_time(&env, 1800);
    assert_eq!(vault.time_remaining(&alice, &0), 1800);
}

#[test]
fn test_time_remaining_after_unlock_is_zero() {
    let (env, vault, token, _admin, alice) = setup();
    let unlock_time = env.ledger().timestamp() + 3600;
    vault.deposit(&alice, &token, &1_000, &unlock_time);

    advance_time(&env, 7200);
    assert_eq!(vault.time_remaining(&alice, &0), 0);
}

#[test]
fn test_time_remaining_no_deposit_is_zero() {
    let (_env, vault, _token, _admin, alice) = setup();
    assert_eq!(vault.time_remaining(&alice, &0), 0);
}

#[test]
fn test_get_time_returns_ledger_timestamp() {
    let (env, vault, _token, _admin, _alice) = setup();
    assert_eq!(vault.get_time(), env.ledger().timestamp());
}

#[test]
fn test_get_constants_returns_correct_values() {
    let (_env, vault, _token, _admin, _alice) = setup();
    let (max_amount, max_duration) = vault.get_constants();
    assert_eq!(max_amount, MAX_DEPOSIT_AMOUNT);
    assert_eq!(max_duration, MAX_LOCK_DURATION_SECS);
}

// ================================================================
//  Emergency Withdrawal
// ================================================================

#[test]
fn test_emergency_withdraw_by_admin_before_unlock_succeeds() {
    let (env, vault, token, admin, alice) = setup();
    let token_client = TokenClient::new(&env, &token);

    let unlock_time = env.ledger().timestamp() + 86400;
    vault.deposit(&alice, &token, &2_000, &unlock_time);

    vault.emergency_withdraw(&admin, &alice, &0);

    assert!(vault.get_vault(&alice, &0).is_none());
    assert_eq!(token_client.balance(&alice), 10_000);
}

#[test]
fn test_emergency_withdraw_by_non_admin_fails() {
    let (env, vault, token, _admin, alice) = setup();
    let bob: Address = Address::generate(&env);

    let unlock_time = env.ledger().timestamp() + 86400;
    vault.deposit(&alice, &token, &2_000, &unlock_time);

    let result = vault.try_emergency_withdraw(&bob, &alice, &0);
    assert_eq!(result, Err(Ok(VaultError::Unauthorized)));
}

#[test]
fn test_emergency_withdraw_no_deposit_fails() {
    let (_env, vault, _token, admin, alice) = setup();
    let result = vault.try_emergency_withdraw(&admin, &alice, &0);
    assert_eq!(result, Err(Ok(VaultError::NoDepositFound)));
}

// ================================================================
//  Admin Transfer — two-step
// ================================================================

#[test]
fn test_transfer_admin_two_step_succeeds() {
    let (env, vault, _token, admin, _alice) = setup();
    let new_admin: Address = Address::generate(&env);

    vault.transfer_admin(&admin, &new_admin);
    assert_eq!(vault.get_pending_admin(), Some(new_admin.clone()));
    assert_eq!(vault.get_admin(), Some(admin.clone()));

    vault.accept_admin(&new_admin);
    assert_eq!(vault.get_admin(), Some(new_admin.clone()));
    assert_eq!(vault.get_pending_admin(), None);
}

#[test]
fn test_transfer_admin_non_admin_cannot_initiate() {
    let (env, vault, _token, _admin, _alice) = setup();
    let bob: Address = Address::generate(&env);
    let carol: Address = Address::generate(&env);

    let result = vault.try_transfer_admin(&bob, &carol);
    assert_eq!(result, Err(Ok(VaultError::Unauthorized)));
}

#[test]
fn test_accept_admin_wrong_address_fails() {
    let (env, vault, _token, admin, _alice) = setup();
    let new_admin: Address = Address::generate(&env);
    let impostor: Address = Address::generate(&env);

    vault.transfer_admin(&admin, &new_admin);

    let result = vault.try_accept_admin(&impostor);
    assert_eq!(result, Err(Ok(VaultError::Unauthorized)));
    assert_eq!(vault.get_admin(), Some(admin));
}

#[test]
fn test_accept_admin_with_no_pending_fails() {
    let (env, vault, _token, _admin, _alice) = setup();
    let bob: Address = Address::generate(&env);

    let result = vault.try_accept_admin(&bob);
    assert_eq!(result, Err(Ok(VaultError::Unauthorized)));
}

#[test]
fn test_cancel_transfer_admin_clears_pending() {
    let (env, vault, _token, admin, _alice) = setup();
    let new_admin: Address = Address::generate(&env);

    vault.transfer_admin(&admin, &new_admin);
    assert_eq!(vault.get_pending_admin(), Some(new_admin.clone()));

    vault.cancel_transfer_admin(&admin);
    assert_eq!(vault.get_pending_admin(), None);
    assert_eq!(vault.get_admin(), Some(admin));
}

#[test]
fn test_cancel_transfer_admin_by_non_admin_fails() {
    let (env, vault, _token, admin, _alice) = setup();
    let new_admin: Address = Address::generate(&env);
    let bob: Address = Address::generate(&env);

    vault.transfer_admin(&admin, &new_admin);

    let result = vault.try_cancel_transfer_admin(&bob);
    assert_eq!(result, Err(Ok(VaultError::Unauthorized)));
}

#[test]
fn test_accept_admin_by_admin_with_no_pending_fails() {
    let (env, vault, _token, admin, _alice) = setup();

    // Admin tries to accept without any prior transfer_admin
    let result = vault.try_accept_admin(&admin);
    assert_eq!(result, Err(Ok(VaultError::Unauthorized)));
}

#[test]
fn test_accept_admin_after_cancel_fails() {
    let (env, vault, _token, admin, _alice) = setup();
    let new_admin: Address = Address::generate(&env);

    vault.transfer_admin(&admin, &new_admin);
    vault.cancel_transfer_admin(&admin);

    // Pending is cleared; previously-nominated address must now fail
    let result = vault.try_accept_admin(&new_admin);
    assert_eq!(result, Err(Ok(VaultError::Unauthorized)));
    assert_eq!(vault.get_pending_admin(), None);
}

#[test]
fn test_new_admin_can_emergency_withdraw_after_transfer() {
    let (env, vault, token, admin, alice) = setup();
    let new_admin: Address = Address::generate(&env);
    let token_client = TokenClient::new(&env, &token);

    let unlock_time = env.ledger().timestamp() + 86400;
    vault.deposit(&alice, &token, &1_000, &unlock_time);

    vault.transfer_admin(&admin, &new_admin);
    vault.accept_admin(&new_admin);

    let result = vault.try_emergency_withdraw(&admin, &alice, &0);
    assert_eq!(result, Err(Ok(VaultError::Unauthorized)));

    vault.emergency_withdraw(&new_admin, &alice, &0);
    assert_eq!(token_client.balance(&alice), 10_000);
}

// ================================================================
//  Admin Renounce
// ================================================================

#[test]
fn test_renounce_admin_removes_admin() {
    let (_env, vault, _token, admin, _alice) = setup();
    vault.renounce_admin(&admin);
    assert_eq!(vault.get_admin(), None);
}

#[test]
fn test_renounce_admin_disables_emergency_withdraw() {
    let (env, vault, token, admin, alice) = setup();

    let unlock_time = env.ledger().timestamp() + 86400;
    vault.deposit(&alice, &token, &1_000, &unlock_time);

    vault.renounce_admin(&admin);

    let result = vault.try_emergency_withdraw(&admin, &alice, &0);
    assert_eq!(result, Err(Ok(VaultError::Unauthorized)));
}

#[test]
fn test_renounce_admin_by_non_admin_fails() {
    let (env, vault, _token, _admin, _alice) = setup();
    let bob: Address = Address::generate(&env);

    let result = vault.try_renounce_admin(&bob);
    assert_eq!(result, Err(Ok(VaultError::Unauthorized)));
}

#[test]
fn test_renounce_admin_clears_pending_transfer() {
    let (env, vault, _token, admin, _alice) = setup();
    let new_admin: Address = Address::generate(&env);

    vault.transfer_admin(&admin, &new_admin);
    assert_eq!(vault.get_pending_admin(), Some(new_admin));

    vault.renounce_admin(&admin);
    assert_eq!(vault.get_admin(), None);
    assert_eq!(vault.get_pending_admin(), None);
}

// ================================================================
//  Re-deposit after withdrawal
// ================================================================

#[test]
fn test_redeposit_after_withdraw_succeeds() {
    let (env, vault, token, _admin, alice) = setup();

    let unlock_time = env.ledger().timestamp() + 3600;
    vault.deposit(&alice, &token, &1_000, &unlock_time);

    advance_time(&env, 3601);
    vault.withdraw(&alice, &0);

    let new_unlock = env.ledger().timestamp() + 7200;
    let id = vault.deposit(&alice, &token, &500, &new_unlock);

    // Counter increments — new deposit gets id=1
    assert_eq!(id, 1);
    let entry = vault.get_vault(&alice, &1).expect("entry should exist");
    assert_eq!(entry.amount, 500);
}

// ================================================================
//  TTL / storage constants
// ================================================================

#[test]
fn test_bump_target_covers_max_lock_duration() {
    // At 5 s/ledger, MAX_LOCK_DURATION_SECS converts to ledgers.
    // BUMP_TARGET must be >= that value so a max-duration deposit
    // cannot expire before its unlock time.
    use crate::storage::BUMP_TARGET;
    const LEDGER_INTERVAL_SECS: u64 = 5;
    let max_lock_ledgers = MAX_LOCK_DURATION_SECS / LEDGER_INTERVAL_SECS;
    assert!(
        BUMP_TARGET as u64 >= max_lock_ledgers,
        "BUMP_TARGET ({}) must be >= max lock duration in ledgers ({})",
        BUMP_TARGET,
        max_lock_ledgers,
    );
}

// ================================================================
//  View functions do not mutate state
// ================================================================

#[test]
fn test_get_vault_is_readonly() {
    let (_env, vault, _token, _admin, alice) = setup();
    assert!(vault.get_vault(&alice, &0).is_none());
    assert!(vault.get_vault(&alice, &0).is_none());
}

#[test]
fn test_time_remaining_is_readonly() {
    let (_env, vault, _token, _admin, alice) = setup();
    assert_eq!(vault.time_remaining(&alice, &0), 0);
    assert_eq!(vault.time_remaining(&alice, &0), 0);
}
