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
    //1
    /// Not signer
    #[error("Account is not signer")]
    NotSigner,

    //2
    /// Invalid owner
    #[error("Invalid owner")]
    InvalidOwner,

    //3
    /// Invalid account data
    #[error("Invalid account data")]
    InvalidAccountData,

    //4
    /// Invalid address
    #[error("Invalid address")]
    InvalidAddress,
}

impl From<PinocchioError> for ProgramError {
    fn from(e: PinocchioError) -> Self {
        msg!(&format!("LST-ERROR: {}", e));
        ProgramError::Custom(e as u32)
    }
}
