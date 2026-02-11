use pinocchio::{
    account_info::AccountInfo, instruction::Seed, msg, program_error::ProgramError,
    pubkey::find_program_address,
};

use crate::{
    instructions::helpers::{
        ProgramAccount, StakeAccountDelegate, StakeAccountInitialize, STAKE_PROGRAM_ID,
        VOTE_PROGRAM_ID,
    },
    state::Config,
};

pub struct CrankInitializeReserveAccounts<'a> {
    pub config_pda: &'a AccountInfo,
    pub stake_account_reserve: &'a AccountInfo,
    pub validator_vote_account: &'a AccountInfo,
    pub unused_account: &'a AccountInfo,
    pub rent_sysvar: &'a AccountInfo,
    pub clock_sysvar: &'a AccountInfo,
    pub history_sysvar: &'a AccountInfo,
    pub system_program: &'a AccountInfo,
    pub stake_program: &'a AccountInfo,
}

impl<'a> TryFrom<&'a [AccountInfo]> for CrankInitializeReserveAccounts<'a> {
    type Error = ProgramError;

    fn try_from(accounts: &'a [AccountInfo]) -> Result<Self, ProgramError> {
        let [config_pda, stake_account_reserve, validator_vote_account, unused_account, rent_sysvar, clock_sysvar, history_sysvar, system_program, stake_program] =
            accounts
        else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };

        if system_program.key() != &pinocchio_system::ID {
            msg!("Invalid system program");
            return Err(ProgramError::IncorrectProgramId);
        }

        if stake_program.key() != &STAKE_PROGRAM_ID {
            msg!("Invalid stake program");
            return Err(ProgramError::IncorrectProgramId);
        }

        if !validator_vote_account.is_owned_by(&VOTE_PROGRAM_ID) {
            return Err(ProgramError::IncorrectProgramId);
        }

        Ok(Self {
            config_pda,
            stake_account_reserve,
            validator_vote_account,
            unused_account,
            rent_sysvar,
            clock_sysvar,
            history_sysvar,
            system_program,
            stake_program,
        })
    }
}

pub struct CrankInitializeReserve<'a> {
    pub accounts: CrankInitializeReserveAccounts<'a>,
}

impl<'a> TryFrom<&'a [AccountInfo]> for CrankInitializeReserve<'a> {
    type Error = ProgramError;

    fn try_from(accounts: &'a [AccountInfo]) -> Result<Self, ProgramError> {
        Ok(Self {
            accounts: CrankInitializeReserveAccounts::try_from(accounts)?,
        })
    }
}

impl<'a> CrankInitializeReserve<'a> {
    pub const DISCRIMINATOR: &'static u8 = &1;

    pub fn process(&self) -> Result<(), ProgramError> {
        //this prevents double invocation
        let reserve_data = self.accounts.stake_account_reserve.try_borrow_data()?;
        let stake_state = u32::from_le_bytes(reserve_data[0..4].try_into().unwrap());
        if stake_state != 0 {
            return Err(ProgramError::AccountAlreadyInitialized);
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

        if config.stake_account_reserve != *self.accounts.stake_account_reserve.key() {
            return Err(ProgramError::InvalidAccountData);
        }

        if config.validator_vote_pubkey != *self.accounts.validator_vote_account.key() {
            return Err(ProgramError::InvalidAccountData);
        }

        ProgramAccount::initialize_stake_account_no_lockup(
            self.accounts.stake_account_reserve,
            self.accounts.config_pda,
            self.accounts.config_pda,
            self.accounts.rent_sysvar,
            config_seeds,
        )?;

        ProgramAccount::delegate_stake_account(
            self.accounts.stake_account_reserve,
            self.accounts.validator_vote_account,
            self.accounts.clock_sysvar,
            self.accounts.history_sysvar,
            self.accounts.unused_account,
            self.accounts.config_pda,
            config_seeds,
        )?;

        Ok(())
    }
}
