// ============================================================
//  Time-Lock Vault — Soroban Smart Contract
//  Stellar Blockchain | Soroban SDK v22
// ============================================================

#![no_std]

// Compile-time assertion: ensure u64 is 8 bytes (closes #82)
const _: () = assert!(core::mem::size_of::<u64>() == 8);

mod constants;
mod contract;
mod errors;
mod events;
mod storage;
mod types;

pub use constants::{
    MAX_BATCH_SIZE, MAX_DEPOSIT_AMOUNT, MAX_LOCK_DURATION_SECS, MIN_LOCK_DURATION_SECS,
};

pub use storage::{BUMP_TARGET, BUMP_THRESHOLD, LEDGER_SECONDS};

pub use contract::TimeLockVault;
pub use contract::TimeLockVaultClient;
pub use types::WithdrawResult;

pub use errors::VaultError;

pub use types::{LedgerVaultEntry, VaultEntry, VaultInfo, VaultKey, VaultStatus};

#[cfg(test)]
mod test;
