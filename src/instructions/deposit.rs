use pinocchio::{
    account_info::AccountInfo,
    instruction::{Seed, Signer},
    program_error::ProgramError,
    pubkey::find_program_address,
};
use pinocchio_system::instructions::Transfer;
use pinocchio_token::{instructions::MintTo, state::Mint};

use crate::{
    errors::PinocchioError,
    instructions::helpers::{LAMPORTS_PER_SOL, STAKE_PROGRAM_ID},
    state::Config,
};

pub struct DepositAccounts<'a> {
    pub config_pda: &'a AccountInfo,
    pub depositor: &'a AccountInfo,
    pub depositor_ata: &'a AccountInfo,
    pub lst_mint: &'a AccountInfo,
    pub stake_account_main: &'a AccountInfo,
    pub stake_account_reserve: &'a AccountInfo,
    pub stake_program: &'a AccountInfo,
    pub token_program: &'a AccountInfo,
    pub system_program: &'a AccountInfo,
    pub rent_sysvar: &'a AccountInfo,
}

impl<'a> TryFrom<&'a [AccountInfo]> for DepositAccounts<'a> {
    type Error = pinocchio::program_error::ProgramError;

    fn try_from(accounts: &'a [AccountInfo]) -> Result<Self, Self::Error> {
        let [config_pda, depositor, depositor_ata, lst_mint, stake_account_main, stake_account_reserve, stake_program, token_program, system_program, rent_sysvar] =
            accounts
        else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };

        if !depositor.is_signer() {
            return Err(PinocchioError::NotSigner.into());
        }

        if system_program.key() != &pinocchio_system::ID {
            return Err(PinocchioError::InvalidSystemProgram.into());
        }

        if token_program.key() != &pinocchio_token::ID {
            return Err(PinocchioError::InvalidTokenProgram.into());
        }

        if stake_program.key() != &STAKE_PROGRAM_ID {
            return Err(PinocchioError::InvalidStakeProgram.into());
        }

        Ok(Self {
            config_pda,
            depositor,
            depositor_ata,
            lst_mint,
            stake_account_main,
            stake_account_reserve,
            stake_program,
            token_program,
            system_program,
            rent_sysvar,
        })
    }
}

pub struct DepositData {
    pub amount_in_lamports: u64,
}

impl TryFrom<&[u8]> for DepositData {
    type Error = ProgramError;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        if data.len() != 8 {
            return Err(ProgramError::InvalidInstructionData);
        }

        let amount_in_lamports = u64::from_le_bytes(data[0..8].try_into().unwrap());

        if amount_in_lamports < LAMPORTS_PER_SOL {
            return Err(PinocchioError::DepositBelowMinimum.into());
        }

        Ok(Self { amount_in_lamports })
    }
}

/// Deposits SOL to reserve and mints LST tokens.
///
/// Accounts expected:
///
/// 0. `[WRITE]` Config PDA
/// 1. `[WRITE, SIGNER]` Depositor
/// 2. `[WRITE]` Depositor ATA
/// 3. `[WRITE]` LST mint
/// 4. `[WRITE]` Stake account main
/// 5. `[WRITE]` Stake account reserve
/// 6. `[]` Stake program
/// 7. `[]` Token program
/// 8. `[]` System program
/// 9. `[]` Rent sysvar
pub struct Deposit<'a> {
    pub accounts: DepositAccounts<'a>,
    pub data: DepositData,
}

impl<'a> TryFrom<(&'a [u8], &'a [AccountInfo])> for Deposit<'a> {
    type Error = ProgramError;

    fn try_from((data, accounts): (&'a [u8], &'a [AccountInfo])) -> Result<Self, Self::Error> {
        Ok(Self {
            accounts: DepositAccounts::try_from(accounts)?,
            data: DepositData::try_from(data)?,
        })
    }
}

impl<'a> Deposit<'a> {
    pub const DISCRIMINATOR: &'static u8 = &3;

    pub fn process(&self) -> Result<(), ProgramError> {
        let (expected_config_pda, bump) = find_program_address(&[b"config"], &crate::ID);
        if expected_config_pda != *self.accounts.config_pda.key() {
            return Err(PinocchioError::InvalidConfigPda.into());
        }

        let bump_binding = [bump];
        let config_seeds = &[Seed::from(b"config"), Seed::from(&bump_binding)];
        let data = self.accounts.config_pda.try_borrow_data()?;
        let config = Config::load(&data)?;

        if !(*self.accounts.stake_account_reserve.key() == config.stake_account_reserve) {
            return Err(PinocchioError::InvalidStakeAccountReserve.into());
        }

        if !(*self.accounts.lst_mint.key() == config.lst_mint) {
            return Err(PinocchioError::InvalidLstMint.into());
        }

        let expected_ata = find_program_address(
            &[
                self.accounts.depositor.key(),
                self.accounts.token_program.key(),
                self.accounts.lst_mint.key(),
            ],
            &pinocchio_associated_token_account::ID,
        )
        .0;
        if expected_ata != *self.accounts.depositor_ata.key() {
            return Err(PinocchioError::InvalidDepositorAta.into());
        }

        let mint = Mint::from_account_info(self.accounts.lst_mint)?;
        let total_lst_supply = mint.supply();

        let total_sol_in_pool = self
            .accounts
            .stake_account_main
            .lamports()
            .checked_add(self.accounts.stake_account_reserve.lamports())
            .ok_or(ProgramError::ArithmeticOverflow)?;

        let lst_to_mint = if total_lst_supply == 0 || total_sol_in_pool == 0 {
            self.data.amount_in_lamports
        } else {
            (self.data.amount_in_lamports as u128)
                .checked_mul(total_lst_supply as u128)
                .ok_or(ProgramError::ArithmeticOverflow)?
                .checked_div(total_sol_in_pool as u128)
                .ok_or(ProgramError::ArithmeticOverflow)? as u64
        };

        drop(mint);

        Transfer {
            from: self.accounts.depositor,
            to: self.accounts.stake_account_reserve,
            lamports: self.data.amount_in_lamports,
        }
        .invoke()?;

        MintTo {
            mint: self.accounts.lst_mint,
            account: self.accounts.depositor_ata,
            mint_authority: self.accounts.config_pda,
            amount: lst_to_mint,
        }
        .invoke_signed(&[Signer::from(config_seeds)])?;
        Ok(())
    }
}
