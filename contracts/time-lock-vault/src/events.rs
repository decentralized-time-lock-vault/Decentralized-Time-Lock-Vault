use soroban_sdk::{Address, Env, Symbol, symbol_short};

// ----------------------------------------------------------------
//  Event helpers
// ----------------------------------------------------------------
// symbol_short! enforces the 32-byte topic limit at compile time.
// Symbol::new is used for strings > 9 chars that still fit in 32 bytes.

/// Emitted when a user successfully locks funds.
pub fn deposit(env: &Env, depositor: &Address, token: &Address, amount: i128, unlock_time: u64) {
    // Keep topics small: use a short symbol only. Put addresses into the payload
    // to avoid bloating the topic area with full Address values.
    let topics = (symbol_short!("deposit"),);
    env.events()
        .publish(topics, (depositor.clone(), token.clone(), amount, unlock_time));
}

/// Emitted when a user successfully withdraws unlocked funds.
pub fn withdraw(env: &Env, depositor: &Address, token: &Address, amount: i128) {
    let topics = (symbol_short!("withdraw"),);
    env.events().publish(topics, (depositor.clone(), token.clone(), amount));
}

/// Emitted when the admin performs an emergency withdrawal.
pub fn emergency_withdraw(
    env: &Env,
    admin: &Address,
    depositor: &Address,
    token: &Address,
    amount: i128,
) {
    let topics = (Symbol::new(env, "emrg_wdraw"),);
    env.events().publish(topics, (admin.clone(), depositor.clone(), token.clone(), amount));
}

/// Emitted when the current admin initiates an admin transfer.
pub fn admin_transfer_initiated(env: &Env, current_admin: &Address, pending_admin: &Address) {
    let topics = (Symbol::new(env, "adm_xfr_init"),);
    env.events().publish(topics, (current_admin.clone(), pending_admin.clone()));
}

/// Emitted when the pending admin accepts and becomes the new admin.
pub fn admin_transfer_accepted(env: &Env, new_admin: &Address) {
    let topics = (Symbol::new(env, "adm_xfr_done"),);
    env.events().publish(topics, new_admin.clone());
}

/// Emitted when the admin renounces their role (sets admin to a dead address).
pub fn admin_renounced(env: &Env, former_admin: &Address) {
    let topics = (Symbol::new(env, "adm_renounce"),);
    env.events().publish(topics, former_admin.clone());
}
