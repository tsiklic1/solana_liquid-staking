use pinocchio::{
    account_info::AccountInfo, instruction::Seed, program_error::ProgramError,
    pubkey::find_program_address,
};

use crate::{
    errors::PinocchioError,
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
            return Err(PinocchioError::InvalidSystemProgram.into());
        }

        if stake_program.key() != &STAKE_PROGRAM_ID {
            return Err(PinocchioError::InvalidStakeProgram.into());
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

/// Merges reserve stake account into main stake account.
///
/// Accounts expected:
///
/// 0. `[WRITE]` Config PDA
/// 1. `[WRITE]` Stake account main
/// 2. `[WRITE]` Stake account reserve
/// 3. `[]` Clock sysvar
/// 4. `[]` History sysvar
/// 5. `[]` System program
/// 6. `[]` Stake program
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
            return Err(PinocchioError::ReserveNotStaked.into());
        }
        drop(reserve_data);

        let (expected_config_pda, bump) = find_program_address(&[b"config"], &crate::ID);
        let bump_binding = [bump];
        let config_seeds = &[Seed::from(b"config"), Seed::from(&bump_binding)];

        if expected_config_pda != *self.accounts.config_pda.key() {
            return Err(PinocchioError::InvalidConfigPda.into());
        }

        let config_data = self.accounts.config_pda.try_borrow_data()?;
        let config = Config::load(&config_data)?;

        if config.stake_account_main != *self.accounts.stake_account_main.key() {
            return Err(PinocchioError::InvalidStakeAccountMain.into());
        }

        if config.stake_account_reserve != *self.accounts.stake_account_reserve.key() {
            return Err(PinocchioError::InvalidStakeAccountReserve.into());
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
