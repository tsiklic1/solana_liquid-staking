use pinocchio::{
    account_info::AccountInfo, instruction::Seed, program_error::ProgramError,
    pubkey::find_program_address,
};

use crate::{
    errors::PinocchioError,
    instructions::helpers::{
        AccountCheck, ProgramAccount, SignerAccount, StakeAccountWithdraw, STAKE_PROGRAM_ID,
    },
};

pub struct WithdrawAccounts<'a> {
    pub account_to_withdraw_from: &'a AccountInfo,
    pub withdrawer: &'a AccountInfo,
    pub clock_sysvar: &'a AccountInfo,
    pub history_sysvar: &'a AccountInfo,
    pub config_pda: &'a AccountInfo,
    pub stake_program: &'a AccountInfo,
}

impl<'a> TryFrom<&'a [AccountInfo]> for WithdrawAccounts<'a> {
    type Error = ProgramError;

    fn try_from(accounts: &'a [AccountInfo]) -> Result<Self, Self::Error> {
        let [account_to_withdraw_from, withdrawer, clock_sysvar, history_sysvar, config_pda, stake_program] =
            accounts
        else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };

        SignerAccount::check(withdrawer)?;

        if stake_program.key() != &STAKE_PROGRAM_ID {
            return Err(PinocchioError::InvalidStakeProgram.into());
        }

        Ok(Self {
            account_to_withdraw_from,
            withdrawer,
            clock_sysvar,
            history_sysvar,
            config_pda,
            stake_program,
        })
    }
}

pub struct WithdrawInstructionData {
    pub nonce: u64,
}

impl TryFrom<&[u8]> for WithdrawInstructionData {
    type Error = ProgramError;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        if data.len() != 8 {
            return Err(ProgramError::InvalidInstructionData);
        }

        let nonce = u64::from_le_bytes(data[0..8].try_into().unwrap());

        Ok(Self { nonce })
    }
}

/// Withdraws SOL from deactivated split stake account to user.
///
/// Accounts expected:
///
/// 0. `[WRITE]` Account to withdraw from (split PDA)
/// 1. `[WRITE, SIGNER]` Withdrawer
/// 2. `[]` Clock sysvar
/// 3. `[]` History sysvar
/// 4. `[WRITE]` Config PDA
/// 5. `[]` Stake program
pub struct Withdraw<'a> {
    pub accounts: WithdrawAccounts<'a>,
    pub data: WithdrawInstructionData,
}

impl<'a> TryFrom<(&'a [u8], &'a [AccountInfo])> for Withdraw<'a> {
    type Error = ProgramError;

    fn try_from((data, accounts): (&'a [u8], &'a [AccountInfo])) -> Result<Self, Self::Error> {
        Ok(Self {
            accounts: WithdrawAccounts::try_from(accounts)?,
            data: WithdrawInstructionData::try_from(data)?,
        })
    }
}

impl<'a> Withdraw<'a> {
    pub const DISCRIMINATOR: &'static u8 = &5;

    pub fn process(&self) -> Result<(), ProgramError> {
        let (expected_config_pda, bump) = find_program_address(&[b"config"], &crate::ID);
        if *self.accounts.config_pda.key() != expected_config_pda {
            return Err(PinocchioError::InvalidConfigPda.into());
        }

        let nonce_bytes = self.data.nonce.to_le_bytes();
        let expected_split_account = find_program_address(
            &[
                b"split_account",
                self.accounts.withdrawer.key(),
                &nonce_bytes,
            ],
            &crate::ID,
        )
        .0;

        if *self.accounts.account_to_withdraw_from.key() != expected_split_account {
            return Err(PinocchioError::InvalidSplitAccountPda.into());
        }

        let bump_binding = [bump];
        let config_seeds = &[Seed::from(b"config"), Seed::from(&bump_binding)];

        ProgramAccount::withdraw_stake_account(
            self.accounts.account_to_withdraw_from,
            self.accounts.withdrawer,
            self.accounts.clock_sysvar,
            self.accounts.history_sysvar,
            self.accounts.config_pda,
            config_seeds,
        )?;

        Ok(())
    }
}
