// ----------------------------------------------------------------
//  Protocol Constants
// ----------------------------------------------------------------

/// Maximum deposit amount expressed in the token's **base units** (the smallest
/// indivisible denomination — e.g. stroops for XLM, or the equivalent for any
/// SEP-41 token).
///
/// Value: `1_000_000_000_000_000` = 10^15
///
/// Scale note: this is **one quadrillion** in the short scale (used in the US,
/// UK since 1974, and most English-speaking countries) — i.e. 10^15.  In the
/// long scale (historically used in parts of continental Europe) the same number
/// is called *one billiard*.  All documentation in this project uses the
/// **short-scale** convention.
///
/// XLM context: XLM has 7 decimal places, so 1 XLM = 10 000 000 stroops.
/// The cap therefore represents 10^15 / 10^7 = **100 000 000 XLM** (100 million
/// XLM) as the maximum single deposit.  For tokens with different decimal
/// precision the effective cap in "whole tokens" scales accordingly.
pub const MAX_DEPOSIT_AMOUNT: i128 = 1_000_000_000_000_000;

/// Maximum lock duration in seconds (~5 years).
pub const MAX_LOCK_DURATION_SECS: u64 = 157_788_000;

/// Minimum lock duration: prevent trivial, pointless vaults that waste storage.
pub const MIN_LOCK_DURATION_SECS: u64 = 60;

/// Maximum depositors per `batch_emergency_withdraw` call.
///
/// Soroban's per-transaction instruction budget is ~100M instructions.
/// Each iteration performs two persistent-storage removes, one token transfer,
/// and one event publish — roughly 1–2M instructions each.
/// 25 leaves comfortable headroom for the common migration use-case.
pub const MAX_BATCH_SIZE: u32 = 25;
