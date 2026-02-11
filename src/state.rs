use pinocchio::{msg, program_error::ProgramError, pubkey::Pubkey};

#[repr(C, packed)]
pub struct Config {
    pub admin: [u8; 32],
    pub lst_mint: [u8; 32],
    pub stake_account_main: [u8; 32],
    pub stake_account_reserve: [u8; 32],
    pub validator_vote_pubkey: [u8; 32],
}

impl Config {
    pub const LEN: usize = 32 + 32 + 32 + 32 + 32;

    #[inline(always)]
    pub fn load_mut(bytes: &mut [u8]) -> Result<&mut Self, ProgramError> {
        if bytes.len() != Config::LEN {
            msg!(&bytes.len().to_string());
            msg!("Config invalid length");
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(unsafe { &mut *core::mem::transmute::<*mut u8, *mut Self>(bytes.as_mut_ptr()) })
    }

    #[inline(always)]
    pub fn load(bytes: &[u8]) -> Result<&Self, ProgramError> {
        if bytes.len() != Config::LEN {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(unsafe { &*core::mem::transmute::<*const u8, *const Self>(bytes.as_ptr()) })
    }

    #[inline(always)]
    pub fn set_inner(
        &mut self,
        admin: Pubkey,
        lst_mint: Pubkey,
        stake_account_main: Pubkey,
        stake_account_reserve: Pubkey,
        validator_vote_pubkey: Pubkey,
    ) {
        self.admin = admin;
        self.lst_mint = lst_mint;
        self.stake_account_main = stake_account_main;
        self.stake_account_reserve = stake_account_reserve;
        self.validator_vote_pubkey = validator_vote_pubkey;
    }
}
