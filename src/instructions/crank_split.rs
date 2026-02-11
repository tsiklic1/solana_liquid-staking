use pinocchio::{
    account_info::AccountInfo,
    instruction::{Seed, Signer},
    msg,
    program_error::ProgramError,
    pubkey::find_program_address,
    sysvars::{rent::Rent, Sysvar},
};
use pinocchio_token::{
    instructions::Burn,
    state::{Mint, TokenAccount},
};

use crate::{
    instructions::helpers::{
        AccountCheck, ProgramAccount, SignerAccount, StakeAccountCreate, StakeAccountDeactivate,
        StakeAccountSplit, STAKE_PROGRAM_ID,
    },
    state::Config,
};

pub struct CrankSplitAccounts<'a> {
    pub stake_account_main: &'a AccountInfo,
    pub stake_account_reserve: &'a AccountInfo,
    pub withdrawer: &'a AccountInfo,
    pub new_stake_account: &'a AccountInfo, //should be PDA derived like b"split_account" + withdrawer
    pub config_pda: &'a AccountInfo,
    pub withdrawer_ata: &'a AccountInfo,
    pub lst_mint: &'a AccountInfo,
    pub rent_sysvar: &'a AccountInfo,
    pub clock_sysvar: &'a AccountInfo,
    pub token_program: &'a AccountInfo,
    pub stake_program: &'a AccountInfo,
    pub system_program: &'a AccountInfo,
}

impl<'a> TryFrom<&'a [AccountInfo]> for CrankSplitAccounts<'a> {
    type Error = pinocchio::program_error::ProgramError;

    fn try_from(accounts: &'a [AccountInfo]) -> Result<Self, Self::Error> {
        let [stake_account_main, stake_account_reserve, withdrawer, new_stake_account, config_pda, withdrawer_ata, lst_mint, rent_sysvar, clock_sysvar, token_program, stake_program, system_program] =
            accounts
        else {
            return Err(pinocchio::program_error::ProgramError::NotEnoughAccountKeys);
        };

        SignerAccount::check(withdrawer)?;

        if system_program.key() != &pinocchio_system::ID {
            msg!("Invalid system program");
            return Err(pinocchio::program_error::ProgramError::IncorrectProgramId);
        }

        if stake_program.key() != &STAKE_PROGRAM_ID {
            msg!("Invalid stake program");
            return Err(pinocchio::program_error::ProgramError::IncorrectProgramId);
        }

        if token_program.key() != &pinocchio_token::ID {
            msg!("Invalid token program");
            return Err(pinocchio::program_error::ProgramError::IncorrectProgramId);
        }

        Ok(Self {
            stake_account_main,
            stake_account_reserve,
            withdrawer,
            new_stake_account,
            config_pda,
            withdrawer_ata,
            lst_mint,
            rent_sysvar,
            clock_sysvar,
            token_program,
            stake_program,
            system_program,
        })
    }
}

pub struct CrankSplitInstructionData {
    pub lamports_to_split: u64,
    pub nonce: u64,
}

impl TryFrom<&[u8]> for CrankSplitInstructionData {
    type Error = ProgramError;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        if data.len() != 8 + 8 {
            return Err(ProgramError::InvalidInstructionData);
        }

        let lamports_to_split = u64::from_le_bytes(data[0..8].try_into().unwrap());
        let nonce = u64::from_le_bytes(data[8..16].try_into().unwrap());

        let stake_account_length = 200;
        let mut minimum_lamports = Rent::get()?.minimum_balance(stake_account_length);
        minimum_lamports += 1_000_000_000;

        if lamports_to_split < minimum_lamports {
            return Err(ProgramError::InvalidInstructionData);
        }

        Ok(Self {
            lamports_to_split,
            nonce,
        })
    }
}

pub struct CrankSplit<'a> {
    pub accounts: CrankSplitAccounts<'a>,
    pub data: CrankSplitInstructionData,
}

impl<'a> TryFrom<(&'a [u8], &'a [AccountInfo])> for CrankSplit<'a> {
    type Error = ProgramError;

    fn try_from((data, accounts): (&'a [u8], &'a [AccountInfo])) -> Result<Self, Self::Error> {
        Ok(Self {
            accounts: CrankSplitAccounts::try_from(accounts)?,
            data: CrankSplitInstructionData::try_from(data)?,
        })
    }
}
impl<'a> CrankSplit<'a> {
    pub const DISCRIMINATOR: &'static u8 = &4;

    pub fn process(&self) -> Result<(), ProgramError> {
        let (expected_config_pda, bump) = find_program_address(&[b"config"], &crate::ID);
        if *self.accounts.config_pda.key() != expected_config_pda {
            return Err(ProgramError::InvalidAccountData);
        }

        let data = self.accounts.config_pda.try_borrow_data()?;
        let config = Config::load(&data)?;

        if config.stake_account_main != *self.accounts.stake_account_main.key() {
            return Err(ProgramError::InvalidAccountData);
        }

        if config.stake_account_reserve != *self.accounts.stake_account_reserve.key() {
            return Err(ProgramError::InvalidAccountData);
        }

        if config.lst_mint != *self.accounts.lst_mint.key() {
            return Err(ProgramError::InvalidAccountData);
        }

        let expected_ata = find_program_address(
            &[
                self.accounts.withdrawer.key(),
                self.accounts.token_program.key(),
                self.accounts.lst_mint.key(),
            ],
            &pinocchio_associated_token_account::ID,
        )
        .0;
        if expected_ata != *self.accounts.withdrawer_ata.key() {
            msg!("Invalid depositor ATA");
            return Err(ProgramError::InvalidAccountData);
        }

        let bump_binding = [bump];
        let config_seeds = &[Seed::from(b"config"), Seed::from(&bump_binding)];

        // let (_, new_stake_account_bump) = find_program_address(
        //     &[b"split_account", self.accounts.withdrawer.key()],
        //     &crate::ID,
        // );

        // let new_stake_account_bump_binding = [new_stake_account_bump];
        // let new_stake_seeds = &[
        //     Seed::from(b"split_account"),
        //     Seed::from(self.accounts.withdrawer.key()),
        //     Seed::from(&new_stake_account_bump_binding),
        // ];

        let nonce_bytes = self.data.nonce.to_le_bytes();
        let (expected_new_stake_account, new_stake_account_bump) = find_program_address(
            &[
                b"split_account",
                self.accounts.withdrawer.key(),
                &nonce_bytes,
            ],
            &crate::ID,
        );

        if expected_new_stake_account != *self.accounts.new_stake_account.key() {
            msg!("Invalid new stake account PDA");
            return Err(ProgramError::InvalidAccountData);
        }

        let new_stake_account_bump_binding = [new_stake_account_bump];
        let new_stake_seeds = &[
            Seed::from(b"split_account"),
            Seed::from(self.accounts.withdrawer.key()),
            Seed::from(&nonce_bytes),
            Seed::from(&new_stake_account_bump_binding),
        ];

        ProgramAccount::stake_account_create(
            self.accounts.withdrawer,
            self.accounts.new_stake_account,
            new_stake_seeds,
        )?;

        ProgramAccount::split_stake_account(
            self.accounts.stake_account_main,
            self.accounts.new_stake_account,
            &self.data.lamports_to_split,
            self.accounts.config_pda,
            config_seeds,
        )?;

        ProgramAccount::deactivate_stake_account(
            self.accounts.new_stake_account,
            self.accounts.clock_sysvar,
            self.accounts.config_pda,
            config_seeds,
        )?;

        //burn lst
        let mint = Mint::from_account_info(self.accounts.lst_mint)?;
        let total_supply_mint = mint.supply();

        // In process(), replace the exchange rate calculation (lines 155-166):
        let main_account_lamports = self.accounts.stake_account_main.lamports();
        let reserve_account_lamports = self.accounts.stake_account_reserve.lamports();
        let new_account_lamports = self.accounts.new_stake_account.lamports();

        let total_lamports_managed = main_account_lamports
            .checked_add(reserve_account_lamports)
            .ok_or(ProgramError::ArithmeticOverflow)?
            .checked_add(new_account_lamports)
            .ok_or(ProgramError::ArithmeticOverflow)?;

        // Also replace the f64 math with u128 integer math:
        let lst_to_burn = (self.data.lamports_to_split as u128)
            .checked_mul(total_supply_mint as u128)
            .ok_or(ProgramError::ArithmeticOverflow)?
            .checked_div(total_lamports_managed as u128)
            .ok_or(ProgramError::ArithmeticOverflow)? as u64;

        let withdrawer_ata_amount =
            TokenAccount::from_account_info(self.accounts.withdrawer_ata)?.amount();
        if withdrawer_ata_amount < lst_to_burn {
            return Err(ProgramError::InsufficientFunds);
        }

        drop(mint);

        Burn {
            account: self.accounts.withdrawer_ata,
            mint: self.accounts.lst_mint,
            authority: self.accounts.withdrawer,
            amount: lst_to_burn,
        }
        .invoke()?;
        msg!("5");

        Ok(())
    }
}
