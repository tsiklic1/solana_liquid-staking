use pinocchio::{
    account_info::AccountInfo, entrypoint, msg, program_error::ProgramError, pubkey::Pubkey,
    ProgramResult,
};

use crate::instructions::{
    crank_initialize_reserve::CrankInitializeReserve, crank_merge_reserve::CrankMergeReserve,
    crank_split::CrankSplit, deposit::Deposit, initialize::Initialize, withdraw::Withdraw,
};

entrypoint!(process_instruction);

pub mod errors;

pub mod instructions;

pub mod state;

// 22222222222222222222222222222222222222222222
pub const ID: Pubkey = [
    0x0f, 0x1e, 0x6b, 0x14, 0x21, 0xc0, 0x4a, 0x07, 0x04, 0x31, 0x26, 0x5c, 0x19, 0xc5, 0xbb, 0xee,
    0x19, 0x92, 0xba, 0xe8, 0xaf, 0xd1, 0xcd, 0x07, 0x8e, 0xf8, 0xaf, 0x70, 0x47, 0xdc, 0x11, 0xf7,
];

fn process_instruction(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    match instruction_data.split_first() {
        Some((Initialize::DISCRIMINATOR, _data)) => {
            msg!("Initialize instruction called");
            Initialize::try_from(accounts)?.process()
        }
        Some((CrankInitializeReserve::DISCRIMINATOR, _data)) => {
            msg!("CrankInitializeReserve instruction called");
            CrankInitializeReserve::try_from(accounts)?.process()
        }
        Some((CrankMergeReserve::DISCRIMINATOR, _data)) => {
            msg!("CrankMergeReserve instruction called");
            CrankMergeReserve::try_from(accounts)?.process()
        }
        Some((Deposit::DISCRIMINATOR, data)) => {
            msg!("Deposit instruction called");
            Deposit::try_from((data, accounts))?.process()
        }
        Some((CrankSplit::DISCRIMINATOR, data)) => {
            msg!("CrankSplit instruction called");
            CrankSplit::try_from((data, accounts))?.process()
        }
        Some((Withdraw::DISCRIMINATOR, data)) => {
            msg!("Withdraw instruction called");
            Withdraw::try_from((data, accounts))?.process()
        }
        _ => Err(ProgramError::InvalidInstructionData),
    }
}
