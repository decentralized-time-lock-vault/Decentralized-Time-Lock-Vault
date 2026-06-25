use soroban_sdk::{symbol_short, Address, Env, Symbol};

pub fn deposit(env: &Env, depositor: &Address, token: &Address, amount: i128, unlock_time: u64) {
    let topics = (symbol_short!("deposit"), depositor.clone(), token.clone());
    env.events().publish(topics, (amount, unlock_time));
}

pub fn withdraw(env: &Env, depositor: &Address, token: &Address, amount: i128) {
    let topics = (symbol_short!("withdraw"), depositor.clone(), token.clone());
    env.events().publish(topics, amount);
}

pub fn emergency_withdraw(
    env: &Env,
    admin: &Address,
    depositor: &Address,
    token: &Address,
    amount: i128,
) {
    // admin is placed in the data payload rather than topics to avoid
    // leaking the admin address in the publicly-indexed event topic stream.
    let topics = (Symbol::new(env, "emrg_wdraw"), depositor.clone());
    env.events()
        .publish(topics, (admin.clone(), token.clone(), amount));
}

pub fn admin_transfer_initiated(env: &Env, current_admin: &Address, pending_admin: &Address) {
    let topics = (Symbol::new(env, "adm_xfr_init"), current_admin.clone());
    env.events().publish(topics, pending_admin.clone());
}

pub fn admin_transfer_cancelled(env: &Env, current_admin: &Address, pending_admin: &Address) {
    let topics = (Symbol::new(env, "adm_xfr_cancel"), current_admin.clone());
    env.events().publish(topics, pending_admin.clone());
}

pub fn admin_transfer_accepted(env: &Env, new_admin: &Address) {
    let topics = (Symbol::new(env, "adm_xfr_done"), new_admin.clone());
    env.events().publish(topics, ());
}

pub fn admin_renounced(env: &Env, former_admin: &Address) {
    let topics = (Symbol::new(env, "adm_renounce"), former_admin.clone());
    env.events().publish(topics, ());
}

pub fn lock_extended(
    env: &Env,
    depositor: &Address,
    old_unlock_time: u64,
    new_unlock_time: u64,
) {
    let topics = (Symbol::new(env, "lock_extended"), depositor.clone());
    env.events().publish(topics, (old_unlock_time, new_unlock_time));
}

pub fn top_up(env: &Env, depositor: &Address, deposit_id: u32, token: &Address, added: i128, new_total: i128) {
    let topics = (Symbol::new(env, "top_up"), depositor.clone(), token.clone());
    env.events().publish(topics, (deposit_id, added, new_total));
}

pub fn deposit_cancelled(
    env: &Env,
    depositor: &Address,
    token: &Address,
    amount: i128,
    penalty: i128,
) {
    let topics = (
        Symbol::new(env, "dep_cancel"),
        depositor.clone(),
        token.clone(),
    );
    env.events().publish(topics, (amount, penalty));
}
