use {
    pinocchio::{msg, program_error::ProgramError},
    thiserror::Error,
};

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum PinocchioError {
    // 0
    /// Lamport balance below rent-exempt threshold.
    #[error("Lamport balance below rent-exempt threshold")]
    NotRentExempt,
    // 1
    /// Not signer
    #[error("Account is not signer")]
    NotSigner,
    // 2
    /// Invalid owner
    #[error("Invalid owner")]
    InvalidOwner,
    // 3
    /// Invalid account data
    #[error("Invalid account data")]
    InvalidAccountData,
    // 4
    /// Invalid address
    #[error("Invalid address")]
    InvalidAddress,
    // 5
    /// Invalid system program
    #[error("Invalid system program")]
    InvalidSystemProgram,
    // 6
    /// Invalid token program
    #[error("Invalid token program")]
    InvalidTokenProgram,
    // 7
    /// Invalid stake program
    #[error("Invalid stake program")]
    InvalidStakeProgram,
    // 8
    /// Invalid associated token program
    #[error("Invalid associated token program")]
    InvalidAssociatedTokenProgram,
    // 9
    /// Invalid validator vote account
    #[error("Invalid validator vote account")]
    InvalidValidatorVoteAccount,
    // 10
    /// Invalid config PDA
    #[error("Invalid config PDA")]
    InvalidConfigPda,
    // 11
    /// Invalid stake account main
    #[error("Invalid stake account main")]
    InvalidStakeAccountMain,
    // 12
    /// Invalid stake account reserve
    #[error("Invalid stake account reserve")]
    InvalidStakeAccountReserve,
    // 13
    /// Invalid LST mint
    #[error("Invalid LST mint")]
    InvalidLstMint,
    // 14
    /// Invalid depositor ATA
    #[error("Invalid depositor ATA")]
    InvalidDepositorAta,
    // 15
    /// Invalid withdrawer ATA
    #[error("Invalid withdrawer ATA")]
    InvalidWithdrawerAta,
    // 16
    /// Invalid split account PDA
    #[error("Invalid split account PDA")]
    InvalidSplitAccountPda,
    // 17
    /// Deposit amount below minimum (1 SOL)
    #[error("Deposit amount below minimum (1 SOL)")]
    DepositBelowMinimum,
    // 18
    /// Split amount below minimum
    #[error("Split amount below minimum")]
    SplitBelowMinimum,
    // 19
    /// Reserve stake account already initialized
    #[error("Reserve stake account already initialized")]
    ReserveAlreadyInitialized,
    // 20
    /// Reserve stake not in staked state
    #[error("Reserve stake not in staked state")]
    ReserveNotStaked,
    // 21
    /// Insufficient LST balance for withdrawal
    #[error("Insufficient LST balance for withdrawal")]
    InsufficientLstBalance,
    // 22
    /// Invalid validator vote key
    #[error("Invalid validator vote key")]
    InvalidValidatorVoteKey,
}

impl From<PinocchioError> for ProgramError {
    fn from(e: PinocchioError) -> Self {
        msg!(&format!("LST-ERROR: {}", e));
        ProgramError::Custom(e as u32)
    }
}
