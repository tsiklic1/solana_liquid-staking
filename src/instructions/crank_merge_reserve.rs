use pinocchio::{
    account_info::AccountInfo, instruction::Seed, msg, program_error::ProgramError,
    pubkey::find_program_address,
};

use crate::{
    instructions::helpers::{ProgramAccount, StakeAccountMerge, STAKE_PROGRAM_ID},
    state::Config,
};

pub struct CrankMergeReserveAccounts<'a> {
    pub config_pda: &'a AccountInfo,
    pub stake_account_main: &'a AccountInfo,
    pub stake_account_reserve: &'a AccountInfo,
    pub clock_sysvar: &'a AccountInfo,
    pub history_sysvar: &'a AccountInfo,
    pub system_program: &'a AccountInfo,
    pub stake_program: &'a AccountInfo,
}

impl<'a> TryFrom<&'a [AccountInfo]> for CrankMergeReserveAccounts<'a> {
    type Error = ProgramError;

    fn try_from(accounts: &'a [AccountInfo]) -> Result<Self, Self::Error> {
        let [config_pda, stake_account_main, stake_account_reserve, clock_sysvar, history_sysvar, system_program, stake_program] =
            accounts
        else {
            return Err(pinocchio::program_error::ProgramError::NotEnoughAccountKeys);
        };

        if system_program.key() != &pinocchio_system::ID {
            msg!("Invalid system program");
            return Err(ProgramError::IncorrectProgramId);
        }

        if stake_program.key() != &STAKE_PROGRAM_ID {
            msg!("Invalid stake program");
            return Err(ProgramError::IncorrectProgramId);
        }

        Ok(Self {
            config_pda,
            stake_account_main,
            stake_account_reserve,
            clock_sysvar,
            history_sysvar,
            system_program,
            stake_program,
        })
    }
}

pub struct CrankMergeReserve<'a> {
    pub accounts: CrankMergeReserveAccounts<'a>,
}
impl<'a> TryFrom<&'a [AccountInfo]> for CrankMergeReserve<'a> {
    type Error = ProgramError;

    fn try_from(accounts: &'a [AccountInfo]) -> Result<Self, ProgramError> {
        Ok(Self {
            accounts: CrankMergeReserveAccounts::try_from(accounts)?,
        })
    }
}
impl<'a> CrankMergeReserve<'a> {
    pub const DISCRIMINATOR: &'static u8 = &2;

    pub fn process(&self) -> Result<(), ProgramError> {
        let reserve_data = self.accounts.stake_account_reserve.try_borrow_data()?;
        let stake_state = u32::from_le_bytes(reserve_data[0..4].try_into().unwrap());
        if stake_state != 2 {
            return Err(ProgramError::InvalidAccountData);
        }
        drop(reserve_data);

        let (expected_config_pda, bump) = find_program_address(&[b"config"], &crate::ID);
        let bump_binding = [bump];
        let config_seeds = &[Seed::from(b"config"), Seed::from(&bump_binding)];

        if expected_config_pda != *self.accounts.config_pda.key() {
            return Err(ProgramError::InvalidAccountData);
        }

        let config_data = self.accounts.config_pda.try_borrow_data()?;
        let config = Config::load(&config_data)?;

        if config.stake_account_main != *self.accounts.stake_account_main.key() {
            return Err(ProgramError::InvalidAccountData);
        }

        if config.stake_account_reserve != *self.accounts.stake_account_reserve.key() {
            return Err(ProgramError::InvalidAccountData);
        }

        ProgramAccount::merge_stake_account(
            self.accounts.stake_account_main,
            self.accounts.stake_account_reserve,
            self.accounts.clock_sysvar,
            self.accounts.history_sysvar,
            self.accounts.config_pda,
            config_seeds,
        )?;

        Ok(())
    }
}
