use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum VaultError {
    InvalidAmount          = 1,
    UnlockTimeNotInFuture  = 2,
    NoDepositFound         = 3,
    FundsStillLocked       = 4,
    DepositAlreadyExists   = 5,
    LockDurationTooLong    = 6,
    Unauthorized           = 7,
    AmountTooLarge         = 8,
    InvalidPenaltyBps      = 9,
    LockDurationTooShort   = 10,
    InvalidAdmin           = 11,
    ContractPaused         = 12,
    FundsAlreadyUnlocked   = 13,
    BatchTooLarge          = 14,
    DepositorFrozen        = 15,
    MigrationNotAllowed    = 16,
    TokenFrozen            = 17,
}
