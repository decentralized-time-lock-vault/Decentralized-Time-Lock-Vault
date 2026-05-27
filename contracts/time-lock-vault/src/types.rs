use soroban_sdk::{contracttype, Address};

// ----------------------------------------------------------------
//  Protocol constants
// ----------------------------------------------------------------

pub const MAX_DEPOSIT_AMOUNT: i128 = 1_000_000_000_000_000;
pub const MAX_LOCK_DURATION_SECS: u64 = 157_788_000;

/// Minimum lock duration: prevent trivial, pointless vaults that waste storage.
/// Set to 60 seconds to avoid very short-lived deposits.
pub const MIN_LOCK_DURATION_SECS: u64 = 60;

// ----------------------------------------------------------------
//  Storage Keys
// ----------------------------------------------------------------

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VaultKey {
    /// Maps (depositor, deposit_id) → VaultEntry
    Deposit(Address, u32),
    /// Maps depositor → next deposit ID counter
    DepositCounter(Address),
    /// Contract-level admin address
    Admin,
    /// Pending admin address during a two-step admin transfer
    PendingAdmin,
    /// Set to true once initialize() has been called; never removed
    Initialized,
}

// ----------------------------------------------------------------
//  Data Structures
// ----------------------------------------------------------------

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VaultEntry {
    pub token: Address,
    pub amount: i128,
    pub unlock_time: u64,
    pub depositor: Address,
}
