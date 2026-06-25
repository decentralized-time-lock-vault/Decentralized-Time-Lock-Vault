// ----------------------------------------------------------------
//  Protocol Constants
// ----------------------------------------------------------------

/// Maximum deposit amount (in stroops or token base units).
pub const MAX_DEPOSIT_AMOUNT: i128 = 1_000_000_000_000_000;

/// Maximum lock duration in seconds (~5 years).
pub const MAX_LOCK_DURATION_SECS: u64 = 157_788_000;

/// Minimum lock duration: prevent trivial, pointless vaults that waste storage.
pub const MIN_LOCK_DURATION_SECS: u64 = 60;

/// Maximum depositors per `get_vault_batch` call.
///
/// Soroban's per-transaction instruction budget is ~100M instructions.
/// Each iteration reads one persistent storage entry — roughly 0.5–1M instructions.
/// 20 leaves comfortable headroom.
///
/// NOTE: This constant is also defined in `types.rs` (value = 20).
/// The `contract.rs` imports both; the `types` import takes precedence.
/// The authoritative value used at runtime is 20.
pub const MAX_BATCH_SIZE: u32 = 20;
