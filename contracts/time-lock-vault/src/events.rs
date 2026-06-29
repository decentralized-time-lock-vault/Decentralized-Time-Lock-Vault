use soroban_sdk::{symbol_short, Address, Env, Symbol};

pub fn deposit(
    env: &Env,
    depositor: &Address,
    token: &Address,
    deposit_id: u32,
    amount: i128,
    unlock_time: u64,
) {
    env.events().publish(
        (symbol_short!("deposit"), depositor.clone(), token.clone()),
        (deposit_id, amount, unlock_time),
    );
}

pub fn top_up(
    env: &Env,
    depositor: &Address,
    deposit_id: u32,
    token: &Address,
    added: i128,
    new_total: i128,
) {
    env.events().publish(
        (symbol_short!("top_up"), depositor.clone(), token.clone()),
        (deposit_id, added, new_total),
    );
}

pub fn withdraw(
    env: &Env,
    depositor: &Address,
    token: &Address,
    deposit_id: u32,
    amount: i128,
) {
    env.events().publish(
        (symbol_short!("withdraw"), depositor.clone(), token.clone()),
        (deposit_id, amount),
    );
}

pub fn withdraw_to(
    env: &Env,
    depositor: &Address,
    recipient: &Address,
    token: &Address,
    deposit_id: u32,
    amount: i128,
) {
    env.events().publish(
        (
            Symbol::new(env, "withdraw_to"),
            depositor.clone(),
            recipient.clone(),
            token.clone(),
        ),
        (deposit_id, amount),
    );
}

pub fn emergency_withdraw(
    env: &Env,
    admin: &Address,
    depositor: &Address,
    token: &Address,
    deposit_id: u32,
    amount: i128,
) {
    env.events().publish(
        (Symbol::new(env, "emrg_wdraw"), depositor.clone()),
        (deposit_id, admin.clone(), token.clone(), amount),
    );
}

pub fn deposit_cancelled(
    env: &Env,
    depositor: &Address,
    token: &Address,
    amount: i128,
    penalty: i128,
) {
    env.events().publish(
        (
            Symbol::new(env, "dep_cancel"),
            depositor.clone(),
            token.clone(),
        ),
        (amount, penalty),
    );
}

pub fn paused(env: &Env, admin: &Address) {
    env.events().publish(
        (Symbol::new(env, "paused"), admin.clone()),
        (),
    );
}

pub fn admin_transfer_accepted(env: &Env, new_admin: &Address) {
    let topics = (Symbol::new(env, "adm_xfr_done"),);
    env.events().publish(topics, new_admin.clone());
}

pub fn admin_renounced(env: &Env, former_admin: &Address) {
    let topics = (Symbol::new(env, "adm_renounce"),);
    env.events().publish(topics, former_admin.clone());
}

pub fn unfrozen(env: &Env, admin: &Address, depositor: &Address) {
    env.events().publish(
        (symbol_short!("unfrozen"), admin.clone(), depositor.clone()),
        (),
    );
}

pub fn migrated(
    env: &Env,
    depositor: &Address,
    deposit_id: u32,
    to_ledger: bool,
    to_time: bool,
) {
    env.events().publish(
        (symbol_short!("migrated"), depositor.clone()),
        (deposit_id, to_ledger, to_time),
    );
}

pub fn lock_extended(
    env: &Env,
    depositor: &Address,
    old_unlock_time: u64,
    new_unlock_time: u64,
) {
    env.events().publish(
        (Symbol::new(env, "lock_extended"), depositor.clone()),
        (old_unlock_time, new_unlock_time),
    );
}

pub fn admin_transfer_initiated(env: &Env, current_admin: &Address, pending_admin: &Address) {
    env.events().publish(
        (Symbol::new(env, "adm_xfr_init"), current_admin.clone()),
        pending_admin.clone(),
    );
}

pub fn admin_transfer_cancelled(env: &Env, current_admin: &Address, pending_admin: &Address) {
    env.events().publish(
        (Symbol::new(env, "adm_xfr_cancel"), current_admin.clone()),
        pending_admin.clone(),
    );
}

pub fn admin_transfer_accepted(env: &Env, new_admin: &Address) {
    env.events().publish(
        (Symbol::new(env, "adm_xfr_done"), new_admin.clone()),
        (),
    );
}

pub fn admin_renounced(env: &Env, former_admin: &Address) {
    env.events().publish(
        (Symbol::new(env, "adm_renounce"), former_admin.clone()),
        (),
    );
}

pub fn batch_withdraw(env: &Env, depositor: &Address, count: u32) {
    env.events().publish(
        (Symbol::new(env, "batch_wdraw"), depositor.clone()),
        count,
    );
}
