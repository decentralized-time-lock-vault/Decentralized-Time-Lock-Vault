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
    /// Maps depositor → VaultEntry
    Deposit(Address),
    /// Contract-level admin address
    Admin,
    PendingAdmin,
    /// Set to true once initialize() has been called; never removed
    Initialized,
    /// Global list of all active depositor addresses (Vec<Address>)
    DepositorList,
    /// Address that receives penalty fees on early cancellation
    FeeRecipient,
    /// Runtime-configurable max deposit amount (overrides compile-time constant).
    MaxDeposit,
    /// Runtime-configurable max lock duration in seconds (overrides compile-time constant).
    MaxLockSecs,
}

// ----------------------------------------------------------------
//  Data Structures
// ----------------------------------------------------------------

/// Represents a single vault deposit.
/// The depositor address is stored here for event emission.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VaultEntry {
    pub token: Address,
    pub amount: i128,
    pub unlock_time: u64,
    pub depositor: Address,

    /// Early-exit penalty in basis points (0–10000). Charged on cancel_deposit.
    /// 0 = free cancellation, 10000 = 100% penalty (all funds go to fee_recipient).
    pub penalty_bps: u32,
}
