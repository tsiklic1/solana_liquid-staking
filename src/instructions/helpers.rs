use crate::errors::PinocchioError;
use pinocchio::cpi::invoke_signed;
use pinocchio::instruction::{AccountMeta, Instruction, Seed, Signer};
use pinocchio::pubkey::find_program_address;
use pinocchio::sysvars::Sysvar;
use pinocchio::{
    account_info::AccountInfo, program_error::ProgramError, sysvars::rent::Rent, ProgramResult,
};
use pinocchio_associated_token_account::instructions::Create;
use pinocchio_system::instructions::CreateAccount;
use pinocchio_token::instructions::{InitializeAccount3, InitializeMint2};

pub const TOKEN_2022_PROGRAM_ID: [u8; 32] = [
    0x06, 0xdd, 0xf6, 0xe1, 0xee, 0x75, 0x8f, 0xde, 0x18, 0x42, 0x5d, 0xbc, 0xe4, 0x6c, 0xcd, 0xda,
    0xb6, 0x1a, 0xfc, 0x4d, 0x83, 0xb9, 0x0d, 0x27, 0xfe, 0xbd, 0xf9, 0x28, 0xd8, 0xa1, 0x8b, 0xfc,
];

const TOKEN_2022_ACCOUNT_DISCRIMINATOR_OFFSET: usize = 165;
pub const TOKEN_2022_MINT_DISCRIMINATOR: u8 = 0x01;
pub const TOKEN_2022_TOKEN_ACCOUNT_DISCRIMINATOR: u8 = 0x02;

pub const STAKE_PROGRAM_ID: [u8; 32] = [
    6, 161, 216, 23, 145, 55, 84, 42, 152, 52, 55, 189, 254, 42, 122, 178, 85, 127, 83, 92, 138,
    120, 114, 43, 104, 164, 157, 192, 0, 0, 0, 0,
];

pub const VOTE_PROGRAM_ID: [u8; 32] = [
    7, 97, 72, 29, 53, 116, 116, 187, 124, 77, 118, 36, 235, 211, 189, 179, 216, 53, 94, 115, 209,
    16, 67, 252, 13, 163, 83, 128, 0, 0, 0, 0,
];

pub const LAMPORTS_PER_SOL: u64 = 1_000_000_000;
pub const STAKE_ACCOUNT_SPACE: usize = 200;

pub trait AccountCheck {
    fn check(account: &AccountInfo) -> Result<(), ProgramError>;
}

pub struct SignerAccount;

impl AccountCheck for SignerAccount {
    fn check(account: &AccountInfo) -> Result<(), ProgramError> {
        if !account.is_signer() {
            return Err(PinocchioError::NotSigner.into());
        }
        Ok(())
    }
}

pub struct SystemAccount;

impl AccountCheck for SystemAccount {
    fn check(account: &AccountInfo) -> Result<(), ProgramError> {
        if !account.is_owned_by(&pinocchio_system::ID) {
            return Err(PinocchioError::InvalidOwner.into());
        }

        Ok(())
    }
}

pub struct MintAccount;

impl AccountCheck for MintAccount {
    fn check(account: &AccountInfo) -> Result<(), ProgramError> {
        if !account.is_owned_by(&pinocchio_token::ID) {
            return Err(PinocchioError::InvalidOwner.into());
        }

        if account.data_len() != pinocchio_token::state::Mint::LEN {
            return Err(PinocchioError::InvalidAccountData.into());
        }

        Ok(())
    }
}

pub trait MintInit {
    fn init(
        account: &AccountInfo,
        payer: &AccountInfo,
        decimals: u8,
        mint_authority: &[u8; 32],
        freeze_authority: Option<&[u8; 32]>,
    ) -> ProgramResult;
    fn init_if_needed(
        account: &AccountInfo,
        payer: &AccountInfo,
        decimals: u8,
        mint_authority: &[u8; 32],
        freeze_authority: Option<&[u8; 32]>,
    ) -> ProgramResult;
}

impl MintInit for MintAccount {
    fn init(
        account: &AccountInfo,
        payer: &AccountInfo,
        decimals: u8,
        mint_authority: &[u8; 32],
        freeze_authority: Option<&[u8; 32]>,
    ) -> ProgramResult {
        let lamports = Rent::get()?.minimum_balance(pinocchio_token::state::Mint::LEN);

        CreateAccount {
            from: payer,
            to: account,
            lamports,
            space: pinocchio_token::state::Mint::LEN as u64,
            owner: &pinocchio_token::ID,
        }
        .invoke()?;

        InitializeMint2 {
            mint: account,
            decimals,
            mint_authority,
            freeze_authority,
        }
        .invoke()
    }

    fn init_if_needed(
        account: &AccountInfo,
        payer: &AccountInfo,
        decimals: u8,
        mint_authority: &[u8; 32],
        freeze_authority: Option<&[u8; 32]>,
    ) -> ProgramResult {
        match Self::check(account) {
            Ok(_) => Ok(()),
            Err(_) => Self::init(account, payer, decimals, mint_authority, freeze_authority),
        }
    }
}

pub struct TokenAccount;

impl AccountCheck for TokenAccount {
    fn check(account: &AccountInfo) -> Result<(), ProgramError> {
        if !account.is_owned_by(&pinocchio_token::ID) {
            return Err(PinocchioError::InvalidOwner.into());
        }

        if account
            .data_len()
            .ne(&pinocchio_token::state::TokenAccount::LEN)
        {
            return Err(PinocchioError::InvalidAccountData.into());
        }

        Ok(())
    }
}

pub trait AccountInit {
    fn init(
        account: &AccountInfo,
        mint: &AccountInfo,
        payer: &AccountInfo,
        owner: &[u8; 32],
    ) -> ProgramResult;
    fn init_if_needed(
        account: &AccountInfo,
        mint: &AccountInfo,
        payer: &AccountInfo,
        owner: &[u8; 32],
    ) -> ProgramResult;
}

impl AccountInit for TokenAccount {
    fn init(
        account: &AccountInfo,
        mint: &AccountInfo,
        payer: &AccountInfo,
        owner: &[u8; 32],
    ) -> ProgramResult {
        let lamports = Rent::get()?.minimum_balance(pinocchio_token::state::TokenAccount::LEN);

        CreateAccount {
            from: payer,
            to: account,
            lamports,
            space: pinocchio_token::state::TokenAccount::LEN as u64,
            owner: &pinocchio_token::ID,
        }
        .invoke()?;

        InitializeAccount3 {
            account,
            mint,
            owner,
        }
        .invoke()
    }

    fn init_if_needed(
        account: &AccountInfo,
        mint: &AccountInfo,
        payer: &AccountInfo,
        owner: &[u8; 32],
    ) -> ProgramResult {
        match Self::check(account) {
            Ok(_) => Ok(()),
            Err(_) => Self::init(account, mint, payer, owner),
        }
    }
}

pub struct Mint2022Account;

impl AccountCheck for Mint2022Account {
    fn check(account: &AccountInfo) -> Result<(), ProgramError> {
        if !account.is_owned_by(&TOKEN_2022_PROGRAM_ID) {
            return Err(PinocchioError::InvalidOwner.into());
        }

        let data = account.try_borrow_data()?;

        if data.len().ne(&pinocchio_token::state::Mint::LEN) {
            if data.len().le(&TOKEN_2022_ACCOUNT_DISCRIMINATOR_OFFSET) {
                return Err(PinocchioError::InvalidAccountData.into());
            }
            if data[TOKEN_2022_ACCOUNT_DISCRIMINATOR_OFFSET].ne(&TOKEN_2022_MINT_DISCRIMINATOR) {
                return Err(PinocchioError::InvalidAccountData.into());
            }
        }

        Ok(())
    }
}

impl MintInit for Mint2022Account {
    fn init(
        account: &AccountInfo,
        payer: &AccountInfo,
        decimals: u8,
        mint_authority: &[u8; 32],
        freeze_authority: Option<&[u8; 32]>,
    ) -> ProgramResult {
        let lamports = Rent::get()?.minimum_balance(pinocchio_token::state::Mint::LEN);

        CreateAccount {
            from: payer,
            to: account,
            lamports,
            space: pinocchio_token::state::Mint::LEN as u64,
            owner: &TOKEN_2022_PROGRAM_ID,
        }
        .invoke()?;

        InitializeMint2 {
            mint: account,
            decimals,
            mint_authority,
            freeze_authority,
        }
        .invoke()
    }

    fn init_if_needed(
        account: &AccountInfo,
        payer: &AccountInfo,
        decimals: u8,
        mint_authority: &[u8; 32],
        freeze_authority: Option<&[u8; 32]>,
    ) -> ProgramResult {
        match Self::check(account) {
            Ok(_) => Ok(()),
            Err(_) => Self::init(account, payer, decimals, mint_authority, freeze_authority),
        }
    }
}
pub struct TokenAccount2022Account;

impl AccountCheck for TokenAccount2022Account {
    fn check(account: &AccountInfo) -> Result<(), ProgramError> {
        if !account.is_owned_by(&TOKEN_2022_PROGRAM_ID) {
            return Err(PinocchioError::InvalidOwner.into());
        }

        let data = account.try_borrow_data()?;

        if data.len().ne(&pinocchio_token::state::TokenAccount::LEN) {
            if data.len().le(&TOKEN_2022_ACCOUNT_DISCRIMINATOR_OFFSET) {
                return Err(PinocchioError::InvalidAccountData.into());
            }
            if data[TOKEN_2022_ACCOUNT_DISCRIMINATOR_OFFSET]
                .ne(&TOKEN_2022_TOKEN_ACCOUNT_DISCRIMINATOR)
            {
                return Err(PinocchioError::InvalidAccountData.into());
            }
        }

        Ok(())
    }
}

impl AccountInit for TokenAccount2022Account {
    fn init(
        account: &AccountInfo,
        mint: &AccountInfo,
        payer: &AccountInfo,
        owner: &[u8; 32],
    ) -> ProgramResult {
        let lamports = Rent::get()?.minimum_balance(pinocchio_token::state::TokenAccount::LEN);

        CreateAccount {
            from: payer,
            to: account,
            lamports,
            space: pinocchio_token::state::TokenAccount::LEN as u64,
            owner: &TOKEN_2022_PROGRAM_ID,
        }
        .invoke()?;

        InitializeAccount3 {
            account,
            mint,
            owner,
        }
        .invoke()
    }

    fn init_if_needed(
        account: &AccountInfo,
        mint: &AccountInfo,
        payer: &AccountInfo,
        owner: &[u8; 32],
    ) -> ProgramResult {
        match Self::check(account) {
            Ok(_) => Ok(()),
            Err(_) => Self::init(account, mint, payer, owner),
        }
    }
}

pub struct MintInterface;

impl AccountCheck for MintInterface {
    fn check(account: &AccountInfo) -> Result<(), ProgramError> {
        if !account.is_owned_by(&TOKEN_2022_PROGRAM_ID) {
            if !account.is_owned_by(&pinocchio_token::ID) {
                return Err(PinocchioError::InvalidOwner.into());
            } else {
                if account.data_len().ne(&pinocchio_token::state::Mint::LEN) {
                    return Err(PinocchioError::InvalidAccountData.into());
                }
            }
        } else {
            let data = account.try_borrow_data()?;

            if data.len().ne(&pinocchio_token::state::Mint::LEN) {
                if data.len().le(&TOKEN_2022_ACCOUNT_DISCRIMINATOR_OFFSET) {
                    return Err(PinocchioError::InvalidAccountData.into());
                }
                if data[TOKEN_2022_ACCOUNT_DISCRIMINATOR_OFFSET].ne(&TOKEN_2022_MINT_DISCRIMINATOR)
                {
                    return Err(PinocchioError::InvalidAccountData.into());
                }
            }
        }

        Ok(())
    }
}

pub struct TokenAccountInterface;

impl AccountCheck for TokenAccountInterface {
    fn check(account: &AccountInfo) -> Result<(), ProgramError> {
        if !account.is_owned_by(&TOKEN_2022_PROGRAM_ID) {
            if !account.is_owned_by(&pinocchio_token::ID) {
                return Err(PinocchioError::InvalidOwner.into());
            } else {
                if account
                    .data_len()
                    .ne(&pinocchio_token::state::TokenAccount::LEN)
                {
                    return Err(PinocchioError::InvalidAccountData.into());
                }
            }
        } else {
            let data = account.try_borrow_data()?;

            if data.len().ne(&pinocchio_token::state::TokenAccount::LEN) {
                if data.len().le(&TOKEN_2022_ACCOUNT_DISCRIMINATOR_OFFSET) {
                    return Err(PinocchioError::InvalidAccountData.into());
                }
                if data[TOKEN_2022_ACCOUNT_DISCRIMINATOR_OFFSET]
                    .ne(&TOKEN_2022_TOKEN_ACCOUNT_DISCRIMINATOR)
                {
                    return Err(PinocchioError::InvalidAccountData.into());
                }
            }
        }

        Ok(())
    }
}

pub struct AssociatedTokenAccount;

pub trait AssociatedTokenAccountCheck {
    fn check(
        account: &AccountInfo,
        authority: &AccountInfo,
        mint: &AccountInfo,
        token_program: &AccountInfo,
    ) -> Result<(), ProgramError>;
}

impl AssociatedTokenAccountCheck for AssociatedTokenAccount {
    fn check(
        account: &AccountInfo,
        authority: &AccountInfo,
        mint: &AccountInfo,
        token_program: &AccountInfo,
    ) -> Result<(), ProgramError> {
        TokenAccount::check(account)?;

        if find_program_address(
            &[authority.key(), token_program.key(), mint.key()],
            &pinocchio_associated_token_account::ID,
        )
        .0
        .ne(account.key())
        {
            return Err(PinocchioError::InvalidAddress.into());
        }

        Ok(())
    }
}

pub trait AssociatedTokenAccountInit {
    fn init(
        account: &AccountInfo,
        mint: &AccountInfo,
        payer: &AccountInfo,
        owner: &AccountInfo,
        system_program: &AccountInfo,
        token_program: &AccountInfo,
    ) -> ProgramResult;
    fn init_if_needed(
        account: &AccountInfo,
        mint: &AccountInfo,
        payer: &AccountInfo,
        owner: &AccountInfo,
        system_program: &AccountInfo,
        token_program: &AccountInfo,
    ) -> ProgramResult;
}

impl AssociatedTokenAccountInit for AssociatedTokenAccount {
    fn init(
        account: &AccountInfo,
        mint: &AccountInfo,
        payer: &AccountInfo,
        owner: &AccountInfo,
        system_program: &AccountInfo,
        token_program: &AccountInfo,
    ) -> ProgramResult {
        Create {
            funding_account: payer,
            account,
            wallet: owner,
            mint,
            system_program,
            token_program,
        }
        .invoke()
    }

    fn init_if_needed(
        account: &AccountInfo,
        mint: &AccountInfo,
        payer: &AccountInfo,
        owner: &AccountInfo,
        system_program: &AccountInfo,
        token_program: &AccountInfo,
    ) -> ProgramResult {
        match Self::check(account, payer, mint, token_program) {
            Ok(_) => Ok(()),
            Err(_) => Self::init(account, mint, payer, owner, system_program, token_program),
        }
    }
}

pub struct ProgramAccount;

impl AccountCheck for ProgramAccount {
    fn check(account: &AccountInfo) -> Result<(), ProgramError> {
        if !account.is_owned_by(&crate::ID) {
            return Err(PinocchioError::InvalidOwner.into());
        }

        Ok(())
    }
}

pub trait ProgramAccountInit {
    fn init<'a, T: Sized>(
        payer: &AccountInfo,
        account: &AccountInfo,
        seeds: &[Seed<'a>],
        space: usize,
    ) -> ProgramResult;
}

impl ProgramAccountInit for ProgramAccount {
    fn init<'a, T: Sized>(
        payer: &AccountInfo,
        account: &AccountInfo,
        seeds: &[Seed<'a>],
        space: usize,
    ) -> ProgramResult {
        let lamports = Rent::get()?.minimum_balance(space);

        let signer = [Signer::from(seeds)];

        CreateAccount {
            from: payer,
            to: account,
            lamports,
            space: space as u64,
            owner: &crate::ID,
        }
        .invoke_signed(&signer)?;

        Ok(())
    }
}

pub trait AccountClose {
    fn close(account: &AccountInfo, destination: &AccountInfo) -> ProgramResult;
}

impl AccountClose for ProgramAccount {
    fn close(account: &AccountInfo, destination: &AccountInfo) -> ProgramResult {
        {
            let mut data = account.try_borrow_mut_data()?;
            data[0] = 0xff;
        }

        *destination.try_borrow_mut_lamports()? += *account.try_borrow_lamports()?;
        account.realloc(1, true)?;
        account.close()
    }
}

pub trait StakeAccountCreate {
    fn stake_account_create(
        payer: &AccountInfo,
        account: &AccountInfo,
        seeds: &[Seed],
    ) -> ProgramResult;
}

impl StakeAccountCreate for ProgramAccount {
    fn stake_account_create(
        payer: &AccountInfo,
        account: &AccountInfo,
        seeds: &[Seed],
    ) -> ProgramResult {
        let lamports = Rent::get()?.minimum_balance(STAKE_ACCOUNT_SPACE);

        let signer = [Signer::from(seeds)];

        CreateAccount {
            from: payer,
            to: account,
            lamports: lamports + LAMPORTS_PER_SOL,
            space: STAKE_ACCOUNT_SPACE as u64,
            owner: &STAKE_PROGRAM_ID,
        }
        .invoke_signed(&signer)?;

        Ok(())
    }
}

pub trait StakeAccountInitialize {
    fn initialize_stake_account_no_lockup(
        account: &AccountInfo,
        staker: &AccountInfo,
        withdrawer: &AccountInfo,
        rent_sysvar: &AccountInfo,
        seeds: &[Seed],
    ) -> ProgramResult;
}

impl StakeAccountInitialize for ProgramAccount {
    fn initialize_stake_account_no_lockup(
        account: &AccountInfo,
        staker: &AccountInfo,
        withdrawer: &AccountInfo,
        rent_sysvar: &AccountInfo,
        seeds: &[Seed],
    ) -> ProgramResult {
        let mut auth_buf = Vec::with_capacity(32 * 2);
        auth_buf.extend_from_slice(staker.key().as_ref()); // staker
        auth_buf.extend_from_slice(withdrawer.key().as_ref()); // withdrawer

        let mut initialize_stake_data = Vec::from(0u32.to_le_bytes());
        initialize_stake_data.extend_from_slice(&auth_buf);

        initialize_stake_data.extend_from_slice(&[0u8; 48]);

        let initialize_stake_ix = Instruction {
            program_id: &STAKE_PROGRAM_ID,
            data: &initialize_stake_data,
            accounts: &[account.into(), rent_sysvar.into()],
        };

        invoke_signed(
            &initialize_stake_ix,
            &[account, rent_sysvar],
            &[Signer::from(seeds)],
        )?;

        Ok(())
    }
}

pub trait StakeAccountDelegate {
    fn delegate_stake_account(
        account: &AccountInfo,
        vote_account: &AccountInfo,
        clock_sysvar: &AccountInfo,
        history_sysvar: &AccountInfo,
        unused_account: &AccountInfo,
        stake_authority: &AccountInfo,
        seeds: &[Seed],
    ) -> ProgramResult;
}

impl StakeAccountDelegate for ProgramAccount {
    fn delegate_stake_account(
        account: &AccountInfo,
        vote_account: &AccountInfo,
        clock_sysvar: &AccountInfo,
        history_sysvar: &AccountInfo,
        unused_account: &AccountInfo,
        stake_authority: &AccountInfo,
        seeds: &[Seed],
    ) -> ProgramResult {
        let delegate_stake_ix = Instruction {
            program_id: &STAKE_PROGRAM_ID,
            data: &Vec::from(2u32.to_le_bytes()),
            accounts: &[
                account.into(),
                vote_account.into(),
                clock_sysvar.into(),
                history_sysvar.into(),
                unused_account.into(),
                AccountMeta::new(stake_authority.key(), false, true),
            ],
        };

        invoke_signed(
            &delegate_stake_ix,
            &[
                account,
                vote_account,
                clock_sysvar,
                history_sysvar,
                unused_account,
                stake_authority,
            ],
            &[Signer::from(seeds)],
        )?;

        Ok(())
    }
}

pub trait StakeAccountMerge {
    fn merge_stake_account(
        destination: &AccountInfo,
        source: &AccountInfo,
        clock_sysvar: &AccountInfo,
        history_sysvar: &AccountInfo,
        stake_authority: &AccountInfo,
        seeds: &[Seed],
    ) -> ProgramResult;
}

impl StakeAccountMerge for ProgramAccount {
    fn merge_stake_account(
        destination: &AccountInfo,
        source: &AccountInfo,
        clock_sysvar: &AccountInfo,
        history_sysvar: &AccountInfo,
        stake_authority: &AccountInfo,
        seeds: &[Seed],
    ) -> ProgramResult {
        let merge_ix = Instruction {
            program_id: &STAKE_PROGRAM_ID,
            data: &Vec::from(7u32.to_le_bytes()),
            accounts: &[
                destination.into(),
                source.into(),
                clock_sysvar.into(),
                history_sysvar.into(),
                AccountMeta::new(stake_authority.key(), false, true),
            ],
        };

        invoke_signed(
            &merge_ix,
            &[
                destination,
                source,
                clock_sysvar,
                history_sysvar,
                stake_authority,
            ],
            &[Signer::from(seeds)],
        )?;

        Ok(())
    }
}

pub trait StakeAccountSplit {
    fn split_stake_account(
        source: &AccountInfo,
        destination: &AccountInfo,
        lamports: &u64,
        stake_authority: &AccountInfo,
        seeds: &[Seed],
    ) -> ProgramResult;
}

impl StakeAccountSplit for ProgramAccount {
    fn split_stake_account(
        source: &AccountInfo,
        destination: &AccountInfo,
        lamports_to_split: &u64,
        stake_authority: &AccountInfo,
        seeds: &[Seed],
    ) -> ProgramResult {
        let mut split_data = Vec::from(3u32.to_le_bytes());
        split_data.extend_from_slice(&lamports_to_split.to_le_bytes());

        let split_ix = Instruction {
            program_id: &STAKE_PROGRAM_ID,
            data: &split_data,
            accounts: &[
                source.into(),
                destination.into(),
                AccountMeta::readonly_signer(stake_authority.key()),
            ],
        };

        invoke_signed(
            &split_ix,
            &[source, destination, stake_authority],
            &[Signer::from(seeds)],
        )?;

        Ok(())
    }
}

pub trait StakeAccountDeactivate {
    fn deactivate_stake_account(
        account: &AccountInfo,
        clock_sysvar: &AccountInfo,
        stake_authority: &AccountInfo,
        seeds: &[Seed],
    ) -> ProgramResult;
}

impl StakeAccountDeactivate for ProgramAccount {
    fn deactivate_stake_account(
        account: &AccountInfo,
        clock_sysvar: &AccountInfo,
        stake_authority: &AccountInfo,
        seeds: &[Seed],
    ) -> ProgramResult {
        let deactivate_ix = Instruction {
            program_id: &STAKE_PROGRAM_ID,
            data: &Vec::from(5u32.to_le_bytes()),
            accounts: &[
                account.into(),
                clock_sysvar.into(),
                AccountMeta::readonly_signer(stake_authority.key()),
            ],
        };

        invoke_signed(
            &deactivate_ix,
            &[account, clock_sysvar, stake_authority],
            &[Signer::from(seeds)],
        )?;

        Ok(())
    }
}

pub trait StakeAccountWithdraw {
    fn withdraw_stake_account(
        account_to_withdraw_from: &AccountInfo,
        withdrawer: &AccountInfo,
        clock_sysvar: &AccountInfo,
        history_sysvar: &AccountInfo,
        withdraw_authority: &AccountInfo,
        seeds: &[Seed],
    ) -> ProgramResult;
}

impl StakeAccountWithdraw for ProgramAccount {
    fn withdraw_stake_account(
        account_to_withdraw_from: &AccountInfo,
        withdrawer: &AccountInfo,
        clock_sysvar: &AccountInfo,
        history_sysvar: &AccountInfo,
        withdraw_authority: &AccountInfo,
        seeds: &[Seed],
    ) -> ProgramResult {
        let mut withdraw_instruction_data = Vec::from(4u32.to_le_bytes());
        let lamports_on_account_to_withdraw_from = account_to_withdraw_from.lamports();
        withdraw_instruction_data
            .extend_from_slice(&lamports_on_account_to_withdraw_from.to_le_bytes());

        let withdraw_ix = Instruction {
            program_id: &STAKE_PROGRAM_ID,
            accounts: &[
                account_to_withdraw_from.into(),
                withdrawer.into(),
                clock_sysvar.into(),
                history_sysvar.into(),
                AccountMeta::readonly_signer(withdraw_authority.key()),
            ],
            data: &withdraw_instruction_data,
        };

        invoke_signed(
            &withdraw_ix,
            &[
                account_to_withdraw_from.into(),
                withdrawer.into(),
                clock_sysvar.into(),
                history_sysvar.into(),
                withdraw_authority.into(),
            ],
            &[Signer::from(seeds)],
        )?;

        Ok(())
    }
}
