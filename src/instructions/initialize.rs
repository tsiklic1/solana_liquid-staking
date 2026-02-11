use pinocchio::{
    account_info::AccountInfo,
    instruction::{Seed, Signer},
    msg,
    program_error::ProgramError,
    pubkey::find_program_address,
};
use pinocchio_token::instructions::MintTo;

use crate::{
    errors::PinocchioError,
    instructions::helpers::{
        AccountCheck, AssociatedTokenAccount, AssociatedTokenAccountInit, MintAccount, MintInit,
        ProgramAccount, ProgramAccountInit, SignerAccount, StakeAccountCreate,
        StakeAccountDelegate, StakeAccountInitialize, SystemAccount, STAKE_PROGRAM_ID,
        VOTE_PROGRAM_ID,
    },
    state::Config,
};

pub struct InitializeAccounts<'a> {
    pub initializer: &'a AccountInfo,
    pub initializer_ata: &'a AccountInfo,
    pub config_pda: &'a AccountInfo,
    pub stake_account_main: &'a AccountInfo,
    pub stake_account_reserve: &'a AccountInfo,
    pub lst_mint: &'a AccountInfo,
    pub validator_vote_account: &'a AccountInfo,
    pub unused_account: &'a AccountInfo,
    pub system_program: &'a AccountInfo,
    pub stake_program: &'a AccountInfo,
    pub token_program: &'a AccountInfo,
    pub associated_token_program: &'a AccountInfo,
    pub rent_sysvar: &'a AccountInfo,
    pub clock_sysvar: &'a AccountInfo,
    pub history_sysvar: &'a AccountInfo,
}

impl<'a> TryFrom<&'a [AccountInfo]> for InitializeAccounts<'a> {
    type Error = ProgramError;

    fn try_from(accounts: &'a [AccountInfo]) -> Result<Self, Self::Error> {
        let [initializer, initializer_ata, config_pda, stake_account_main, stake_account_reserve, lst_mint, validator_vote_account, unused_account, system_program, stake_program, token_program, associated_token_program, rent_sysvar, clock_sysvar, history_sysvar] =
            accounts
        else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };

        SignerAccount::check(initializer)?;
        SignerAccount::check(lst_mint)?;

        if system_program.key() != &pinocchio_system::ID {
            return Err(PinocchioError::InvalidSystemProgram.into());
        }

        if token_program.key() != &pinocchio_token::ID {
            return Err(PinocchioError::InvalidTokenProgram.into());
        }

        SystemAccount::check(config_pda)?;

        if !config_pda.data_is_empty() {
            return Err(ProgramError::AccountAlreadyInitialized);
        }

        SystemAccount::check(stake_account_main)?;

        if !stake_account_main.data_is_empty() {
            return Err(ProgramError::AccountAlreadyInitialized);
        }

        SystemAccount::check(stake_account_reserve)?;

        if !stake_account_reserve.data_is_empty() {
            return Err(ProgramError::AccountAlreadyInitialized);
        }

        MintAccount::check(lst_mint)?;

        if !validator_vote_account.is_owned_by(&VOTE_PROGRAM_ID) {
            return Err(PinocchioError::InvalidValidatorVoteAccount.into());
        }

        if stake_program.key() != &STAKE_PROGRAM_ID {
            return Err(PinocchioError::InvalidStakeProgram.into());
        }

        if associated_token_program.key() != &pinocchio_associated_token_account::ID {
            return Err(PinocchioError::InvalidAssociatedTokenProgram.into());
        }

        Ok(Self {
            initializer,
            initializer_ata,
            config_pda,
            stake_account_main,
            stake_account_reserve,
            lst_mint,
            validator_vote_account,
            unused_account,
            system_program,
            stake_program,
            token_program,
            associated_token_program,
            rent_sysvar,
            clock_sysvar,
            history_sysvar,
        })
    }
}
/// Sets up liquid staking pool and mints initial LST.
///
/// Accounts expected:
///
/// 0. `[WRITE, SIGNER]` Initializer
/// 1. `[WRITE]` Initializer ATA
/// 2. `[WRITE]` Config PDA
/// 3. `[WRITE]` Stake account main
/// 4. `[WRITE]` Stake account reserve
/// 5. `[WRITE, SIGNER]` LST mint
/// 6. `[WRITE]` Validator vote account
/// 7. `[WRITE]` Unused account
/// 8. `[]` System program
/// 9. `[]` Stake program
/// 10. `[]` Token program
/// 11. `[]` Associated token program
/// 12. `[]` Rent sysvar
/// 13. `[]` Clock sysvar
/// 14. `[]` History sysvar
pub struct Initialize<'a> {
    pub accounts: InitializeAccounts<'a>,
}
impl<'a> TryFrom<&'a [AccountInfo]> for Initialize<'a> {
    type Error = ProgramError;

    fn try_from(accounts: &'a [AccountInfo]) -> Result<Self, Self::Error> {
        msg!("Initialize::try_from accounts");
        Ok(Self {
            accounts: InitializeAccounts::try_from(accounts)?,
        })
    }
}
impl<'a> Initialize<'a> {
    pub const DISCRIMINATOR: &'static u8 = &0;

    pub fn process(&mut self) -> Result<(), ProgramError> {
        let (expected_config_pda, bump) = find_program_address(&[b"config"], &crate::ID);
        if expected_config_pda != *self.accounts.config_pda.key() {
            return Err(PinocchioError::InvalidConfigPda.into());
        }
        let bump_binding = [bump];
        let config_seeds = &[Seed::from(b"config"), Seed::from(&bump_binding)];
        ProgramAccount::init::<Config>(
            self.accounts.initializer,
            self.accounts.config_pda,
            config_seeds,
            Config::LEN,
        )?;
        let mut data = self.accounts.config_pda.try_borrow_mut_data()?;
        let config = Config::load_mut(data.as_mut())?;

        config.set_inner(
            *self.accounts.initializer.key(),
            *self.accounts.lst_mint.key(),
            *self.accounts.stake_account_main.key(),
            *self.accounts.stake_account_reserve.key(),
            *self.accounts.validator_vote_account.key(),
        );

        //make and fund stake account main
        let (expected_stake_account_main, stake_main_bump) =
            find_program_address(&[b"stake_main"], &crate::ID);

        if expected_stake_account_main != *self.accounts.stake_account_main.key() {
            return Err(PinocchioError::InvalidStakeAccountMain.into());
        }

        let stake_main_bump_binding = [stake_main_bump];
        let stake_main_seeds = &[
            Seed::from(b"stake_main"),
            Seed::from(&stake_main_bump_binding),
        ];

        ProgramAccount::stake_account_create(
            self.accounts.initializer,
            self.accounts.stake_account_main,
            stake_main_seeds,
        )?;

        ProgramAccount::initialize_stake_account_no_lockup(
            self.accounts.stake_account_main,
            self.accounts.config_pda,
            self.accounts.config_pda,
            self.accounts.rent_sysvar,
            config_seeds,
        )?;

        drop(data);

        ProgramAccount::delegate_stake_account(
            self.accounts.stake_account_main,
            self.accounts.validator_vote_account,
            self.accounts.clock_sysvar,
            self.accounts.history_sysvar,
            self.accounts.unused_account,
            self.accounts.config_pda,
            config_seeds,
        )?;

        let (expected_stake_account_reserve, stake_reserve_bump) =
            find_program_address(&[b"stake_reserve"], &crate::ID);

        if expected_stake_account_reserve != *self.accounts.stake_account_reserve.key() {
            return Err(PinocchioError::InvalidStakeAccountReserve.into());
        }

        let stake_reserve_bump_binding = [stake_reserve_bump];

        let stake_reserve_seeds = &[
            Seed::from(b"stake_reserve"),
            Seed::from(&stake_reserve_bump_binding),
        ];

        ProgramAccount::stake_account_create(
            self.accounts.initializer,
            self.accounts.stake_account_reserve,
            stake_reserve_seeds,
        )?;
        let signer = [Signer::from(config_seeds)];

        MintAccount::init_if_needed(
            self.accounts.lst_mint,
            self.accounts.initializer,
            9,
            self.accounts.config_pda.key(),
            None,
        )?;

        AssociatedTokenAccount::init_if_needed(
            self.accounts.initializer_ata,
            self.accounts.lst_mint,
            self.accounts.initializer,
            self.accounts.initializer,
            self.accounts.system_program,
            self.accounts.token_program,
        )?;

        MintTo {
            mint: self.accounts.lst_mint,
            account: self.accounts.initializer_ata,
            mint_authority: self.accounts.config_pda,
            amount: 1 * 10u64.pow(9),
        }
        .invoke_signed(&signer)?;

        Ok(())
    }
}
