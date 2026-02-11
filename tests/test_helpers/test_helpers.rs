use litesvm::LiteSVM;

use solana_sdk::{
    account::Account,
    clock::Clock,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
};
use spl_token::solana_program::program_option::COption;
use spl_token::solana_program::program_pack::Pack;
use spl_token::state::{Account as TokenAccount, Mint};
use spl_token::ID as TOKEN_PROGRAM_ID;

pub const PROGRAM_ID: Pubkey = Pubkey::new_from_array([
    0x0f, 0x1e, 0x6b, 0x14, 0x21, 0xc0, 0x4a, 0x07, 0x04, 0x31, 0x26, 0x5c, 0x19, 0xc5, 0xbb, 0xee,
    0x19, 0x92, 0xba, 0xe8, 0xaf, 0xd1, 0xcd, 0x07, 0x8e, 0xf8, 0xaf, 0x70, 0x47, 0xdc, 0x11, 0xf7,
]);

pub const HISTORY_SYSVAR: Pubkey = Pubkey::new_from_array([
    6, 167, 213, 23, 25, 53, 132, 208, 254, 237, 155, 179, 67, 29, 19, 32, 107, 229, 68, 40, 27,
    87, 184, 86, 108, 197, 55, 95, 244, 0, 0, 0,
]);

pub fn setup_svm() -> LiteSVM {
    let mut svm = LiteSVM::new().with_builtins().with_sigverify(false);

    svm.add_program_from_file(PROGRAM_ID, "target/deploy/solana_liquid_staking.so")
        .expect("Failed to load program");

    svm
}

pub fn print_transaction_logs(
    result: &Result<litesvm::types::TransactionMetadata, litesvm::types::FailedTransactionMetadata>,
) {
    if let Err(err) = result {
        println!("\n=== Transaction Failed ===");
        println!("Error: {:?}", err.err);
        println!("\nProgram Logs:");
        for log in &err.meta.logs {
            println!("  {}", log);
        }
        println!(
            "Compute units consumed: {}",
            err.meta.compute_units_consumed
        );
        println!("========================\n");
    } else if let Ok(meta) = result {
        println!("\n=== Transaction Succeeded ===");
        println!("\nProgram Logs:");
        for log in &meta.logs {
            println!("  {}", log);
        }
        println!("Compute units consumed: {}", meta.compute_units_consumed);
        println!("=============================\n");
    }
}

pub fn create_mock_token_mint(svm: &mut LiteSVM, authority: &Pubkey) -> Keypair {
    let mint_keypair = Keypair::new();
    let mint_pubkey = mint_keypair.pubkey();

    let mint_data = Mint {
        mint_authority: COption::Some(*authority),
        supply: 0,
        decimals: 9,
        is_initialized: true,
        freeze_authority: COption::None,
    };

    let mut data = vec![0u8; Mint::LEN];
    Mint::pack(mint_data, &mut data).unwrap();

    let mint_account = Account {
        lamports: 10_000_000,
        data,
        owner: TOKEN_PROGRAM_ID,
        executable: false,
        rent_epoch: 0,
    };

    let _ = svm.set_account(mint_pubkey, mint_account.into());
    mint_keypair
}

pub fn create_and_fund_ata(
    svm: &mut LiteSVM,
    owner: &Pubkey,
    mint: &Pubkey,
    amount: u64,
) -> Pubkey {
    let ata = spl_associated_token_account::get_associated_token_address(owner, mint);

    let token_account = TokenAccount {
        mint: *mint,
        owner: *owner,
        amount,
        delegate: COption::None,
        state: spl_token::state::AccountState::Initialized,
        is_native: COption::None,
        delegated_amount: 0,
        close_authority: COption::None,
    };

    let mut data = vec![0u8; TokenAccount::LEN];
    TokenAccount::pack(token_account, &mut data).unwrap();

    let account = Account {
        lamports: 10_000_000,
        data,
        owner: TOKEN_PROGRAM_ID,
        executable: false,
        rent_epoch: 0,
    };

    let _ = svm.set_account(ata, account.into());
    ata
}

pub fn warp_time(svm: &mut LiteSVM, new_timestamp: i64) {
    let mut clock = svm.get_sysvar::<Clock>();
    clock.unix_timestamp = new_timestamp;
    svm.set_sysvar(&clock);
}

/// Sets up common test state for the Initialize instruction and returns all the pieces needed.
pub fn setup_initialize_accounts(
    svm: &mut LiteSVM,
) -> (
    Keypair, // initializer
    Keypair, // token_mint
    Pubkey,  // initializer_ata
    Pubkey,  // config_pda
    Pubkey,  // stake_account_main
    Pubkey,  // stake_account_reserve
    Pubkey,  // vote_pubkey
) {
    use solana_liquid_staking::instructions::helpers::VOTE_PROGRAM_ID;

    let initializer = Keypair::new();
    svm.airdrop(&initializer.pubkey(), 10_000_000_000).unwrap();

    let config_pda = Pubkey::find_program_address(&[b"config"], &PROGRAM_ID).0;
    let token_mint = create_mock_token_mint(svm, &config_pda);
    let initializer_ata = create_and_fund_ata(svm, &initializer.pubkey(), &token_mint.pubkey(), 0);

    let stake_account_main = Pubkey::find_program_address(&[b"stake_main"], &PROGRAM_ID).0;
    let stake_account_reserve = Pubkey::find_program_address(&[b"stake_reserve"], &PROGRAM_ID).0;

    let validator_vote_account = Keypair::new();
    let vote_pubkey = validator_vote_account.pubkey();

    let mut data = vec![0u8; 3762];
    data[0..4].copy_from_slice(&1u32.to_le_bytes());
    data[4..36].copy_from_slice(vote_pubkey.as_ref());
    data[36..68].copy_from_slice(vote_pubkey.as_ref());

    svm.set_account(
        vote_pubkey,
        Account {
            lamports: 10_000_000_000,
            data,
            owner: Pubkey::from(VOTE_PROGRAM_ID),
            executable: false,
            rent_epoch: 0,
        }
        .into(),
    )
    .unwrap();

    (
        initializer,
        token_mint,
        initializer_ata,
        config_pda,
        stake_account_main,
        stake_account_reserve,
        vote_pubkey,
    )
}

/// Runs setup_initialize_accounts + sends the initialize transaction.
/// Returns (initializer, token_mint, initializer_ata, config_pda, stake_account_main, stake_account_reserve, vote_pubkey).
pub fn run_initialize(
    svm: &mut LiteSVM,
) -> (
    Keypair, // initializer
    Keypair, // token_mint
    Pubkey,  // initializer_ata
    Pubkey,  // config_pda
    Pubkey,  // stake_account_main
    Pubkey,  // stake_account_reserve
    Pubkey,  // vote_pubkey
) {
    use solana_liquid_staking::instructions::helpers::STAKE_PROGRAM_ID;
    use solana_program::example_mocks::solana_sdk::system_program;
    use solana_sdk::transaction::Transaction;

    let (
        initializer,
        token_mint,
        initializer_ata,
        config_pda,
        stake_account_main,
        stake_account_reserve,
        vote_pubkey,
    ) = setup_initialize_accounts(svm);

    let ix = build_initialize_ix(
        &initializer.pubkey(),
        &initializer_ata,
        &config_pda,
        &stake_account_main,
        &stake_account_reserve,
        &token_mint.pubkey(),
        true,
        &vote_pubkey,
        &system_program::ID,
        &Pubkey::from(STAKE_PROGRAM_ID),
        &spl_token::ID,
        &spl_associated_token_account::ID,
    );

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&initializer.pubkey()),
        &[&initializer, &token_mint],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    print_transaction_logs(&result);
    assert!(result.is_ok(), "Initialize transaction should succeed");

    (
        initializer,
        token_mint,
        initializer_ata,
        config_pda,
        stake_account_main,
        stake_account_reserve,
        vote_pubkey,
    )
}

/// Builds a Deposit instruction with the given accounts. The depositor must be
/// a signer in the transaction. `depositor_is_signer` controls the AccountMeta.
pub fn build_deposit_ix(
    config_pda: &Pubkey,
    depositor: &Pubkey,
    depositor_ata: &Pubkey,
    token_mint: &Pubkey,
    stake_account_main: &Pubkey,
    stake_account_reserve: &Pubkey,
    deposit_amount: u64,
    depositor_is_signer: bool,
) -> solana_sdk::instruction::Instruction {
    use solana_liquid_staking::instructions::helpers::STAKE_PROGRAM_ID;
    use solana_program::example_mocks::solana_sdk::system_program;
    use solana_sdk::instruction::{AccountMeta, Instruction};

    let rent_sysvar = solana_sdk::sysvar::rent::id();

    let mut data = vec![3u8];
    data.extend_from_slice(&deposit_amount.to_le_bytes());

    Instruction {
        program_id: PROGRAM_ID,
        data,
        accounts: vec![
            AccountMeta::new(*config_pda, false),
            AccountMeta::new(*depositor, depositor_is_signer),
            AccountMeta::new(*depositor_ata, false),
            AccountMeta::new(*token_mint, false),
            AccountMeta::new(*stake_account_main, false),
            AccountMeta::new(*stake_account_reserve, false),
            AccountMeta::new_readonly(Pubkey::from(STAKE_PROGRAM_ID), false),
            AccountMeta::new_readonly(spl_token::ID, false),
            AccountMeta::new_readonly(system_program::ID, false),
            AccountMeta::new_readonly(rent_sysvar, false),
        ],
    }
}

/// Sends a Deposit transaction. Returns the depositor keypair and depositor_ata.
pub fn run_deposit(
    svm: &mut LiteSVM,
    config_pda: &Pubkey,
    token_mint_pubkey: &Pubkey,
    stake_account_main: &Pubkey,
    stake_account_reserve: &Pubkey,
    deposit_amount: u64,
) -> (Keypair, Pubkey) {
    use solana_liquid_staking::instructions::helpers::STAKE_PROGRAM_ID;
    use solana_program::example_mocks::solana_sdk::system_program;
    use solana_sdk::instruction::{AccountMeta, Instruction};
    use solana_sdk::transaction::Transaction;

    let depositor = Keypair::new();
    svm.airdrop(&depositor.pubkey(), 10_000_000_000).unwrap();
    let depositor_ata = create_and_fund_ata(svm, &depositor.pubkey(), token_mint_pubkey, 0);

    let mut deposit_data = vec![3u8];
    deposit_data.extend_from_slice(&deposit_amount.to_le_bytes());

    let rent_sysvar = solana_sdk::sysvar::rent::id();

    let deposit_ix = Instruction {
        program_id: PROGRAM_ID,
        data: deposit_data,
        accounts: vec![
            AccountMeta::new(*config_pda, false),
            AccountMeta::new(depositor.pubkey(), true),
            AccountMeta::new(depositor_ata, false),
            AccountMeta::new(*token_mint_pubkey, false),
            AccountMeta::new(*stake_account_main, false),
            AccountMeta::new(*stake_account_reserve, false),
            AccountMeta::new_readonly(Pubkey::from(STAKE_PROGRAM_ID), false),
            AccountMeta::new_readonly(spl_token::ID, false),
            AccountMeta::new_readonly(system_program::ID, false),
            AccountMeta::new_readonly(rent_sysvar, false),
        ],
    };

    let deposit_tx = Transaction::new_signed_with_payer(
        &[deposit_ix],
        Some(&depositor.pubkey()),
        &[&depositor],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(deposit_tx);
    println!("PRINTING DEPOSIT TRANSACTION LOGS");
    print_transaction_logs(&result);
    assert!(result.is_ok(), "Deposit transaction should succeed");

    (depositor, depositor_ata)
}

/// Sends a CrankInitializeReserve transaction.
pub fn run_crank_initialize_reserve(
    svm: &mut LiteSVM,
    fee_payer: &Keypair,
    config_pda: &Pubkey,
    stake_account_reserve: &Pubkey,
    vote_pubkey: &Pubkey,
) {
    use solana_liquid_staking::instructions::helpers::STAKE_PROGRAM_ID;
    use solana_program::example_mocks::solana_sdk::system_program;
    use solana_sdk::instruction::{AccountMeta, Instruction};
    use solana_sdk::transaction::Transaction;

    let rent_sysvar = solana_sdk::sysvar::rent::id();
    let clock_sysvar = solana_sdk::sysvar::clock::id();

    let ix = Instruction {
        program_id: PROGRAM_ID,
        data: vec![1u8],
        accounts: vec![
            AccountMeta::new(*config_pda, false),
            AccountMeta::new(*stake_account_reserve, false),
            AccountMeta::new(*vote_pubkey, false),
            AccountMeta::new_readonly(Pubkey::from(STAKE_PROGRAM_ID), false),
            AccountMeta::new_readonly(rent_sysvar, false),
            AccountMeta::new_readonly(clock_sysvar, false),
            AccountMeta::new_readonly(HISTORY_SYSVAR, false),
            AccountMeta::new_readonly(system_program::ID, false),
            AccountMeta::new_readonly(Pubkey::from(STAKE_PROGRAM_ID), false),
        ],
    };

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&fee_payer.pubkey()),
        &[fee_payer],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    println!("PRINTING CRANK INITIALIZE RESERVE TRANSACTION LOGS");
    print_transaction_logs(&result);
    assert!(
        result.is_ok(),
        "CrankInitializeReserve transaction should succeed"
    );
}

/// Builds a CrankMergeReserve instruction with the given accounts.
pub fn build_crank_merge_reserve_ix(
    config_pda: &Pubkey,
    stake_account_main: &Pubkey,
    stake_account_reserve: &Pubkey,
    system_program_id: &Pubkey,
    stake_program_id: &Pubkey,
) -> solana_sdk::instruction::Instruction {
    use solana_sdk::instruction::{AccountMeta, Instruction};

    let clock_sysvar = solana_sdk::sysvar::clock::id();

    Instruction {
        program_id: PROGRAM_ID,
        data: vec![2u8],
        accounts: vec![
            AccountMeta::new(*config_pda, false),
            AccountMeta::new(*stake_account_main, false),
            AccountMeta::new(*stake_account_reserve, false),
            AccountMeta::new_readonly(clock_sysvar, false),
            AccountMeta::new_readonly(HISTORY_SYSVAR, false),
            AccountMeta::new_readonly(*system_program_id, false),
            AccountMeta::new_readonly(*stake_program_id, false),
        ],
    }
}

/// Sends a CrankMergeReserve transaction.
pub fn run_crank_merge_reserve(
    svm: &mut LiteSVM,
    fee_payer: &Keypair,
    config_pda: &Pubkey,
    stake_account_main: &Pubkey,
    stake_account_reserve: &Pubkey,
) {
    use solana_liquid_staking::instructions::helpers::STAKE_PROGRAM_ID;
    use solana_program::example_mocks::solana_sdk::system_program;
    use solana_sdk::instruction::{AccountMeta, Instruction};
    use solana_sdk::transaction::Transaction;

    let clock_sysvar = solana_sdk::sysvar::clock::id();

    let ix = Instruction {
        program_id: PROGRAM_ID,
        data: vec![2u8],
        accounts: vec![
            AccountMeta::new(*config_pda, false),
            AccountMeta::new(*stake_account_main, false),
            AccountMeta::new(*stake_account_reserve, false),
            AccountMeta::new_readonly(clock_sysvar, false),
            AccountMeta::new_readonly(HISTORY_SYSVAR, false),
            AccountMeta::new_readonly(system_program::ID, false),
            AccountMeta::new_readonly(Pubkey::from(STAKE_PROGRAM_ID), false),
        ],
    };

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&fee_payer.pubkey()),
        &[fee_payer],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    println!("PRINTING CRANK MERGE RESERVE TRANSACTION LOGS");
    print_transaction_logs(&result);
    assert!(
        result.is_ok(),
        "CrankMergeReserve transaction should succeed"
    );
}

/// Builds a CrankSplit instruction with the given accounts.
/// Returns (instruction, depositor_stake_account_pda).
pub fn build_crank_split_ix(
    depositor: &Pubkey,
    depositor_ata: &Pubkey,
    config_pda: &Pubkey,
    stake_account_main: &Pubkey,
    stake_account_reserve: &Pubkey,
    token_mint_pubkey: &Pubkey,
    lamports_to_split: u64,
    depositor_is_signer: bool,
    nonce: u64,
) -> (solana_sdk::instruction::Instruction, Pubkey) {
    use solana_liquid_staking::instructions::helpers::STAKE_PROGRAM_ID;
    use solana_program::example_mocks::solana_sdk::system_program;
    use solana_sdk::instruction::{AccountMeta, Instruction};

    let rent_sysvar = solana_sdk::sysvar::rent::id();
    let clock_sysvar = solana_sdk::sysvar::clock::id();

    let nonce_bytes = nonce.to_le_bytes();
    let depositor_stake_account = Pubkey::find_program_address(
        &[b"split_account", depositor.as_ref(), &nonce_bytes],
        &PROGRAM_ID,
    )
    .0;

    let mut data = vec![4u8];
    data.extend_from_slice(&lamports_to_split.to_le_bytes());
    data.extend_from_slice(&nonce_bytes);

    let ix = Instruction {
        program_id: PROGRAM_ID,
        data,
        accounts: vec![
            AccountMeta::new(*stake_account_main, false),
            AccountMeta::new(*stake_account_reserve, false),
            AccountMeta::new(*depositor, depositor_is_signer),
            AccountMeta::new(depositor_stake_account, false),
            AccountMeta::new(*config_pda, false),
            AccountMeta::new(*depositor_ata, false),
            AccountMeta::new(*token_mint_pubkey, false),
            AccountMeta::new_readonly(rent_sysvar, false),
            AccountMeta::new_readonly(clock_sysvar, false),
            AccountMeta::new_readonly(spl_token::ID, false),
            AccountMeta::new_readonly(Pubkey::from(STAKE_PROGRAM_ID), false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
    };

    (ix, depositor_stake_account)
}

/// Sends a CrankSplit transaction. Returns the depositor_stake_account PDA.
pub fn run_crank_split(
    svm: &mut LiteSVM,
    depositor: &Keypair,
    depositor_ata: &Pubkey,
    config_pda: &Pubkey,
    stake_account_main: &Pubkey,
    stake_account_reserve: &Pubkey,
    token_mint_pubkey: &Pubkey,
    lamports_to_split: u64,
    nonce: u64,
) -> Pubkey {
    use solana_liquid_staking::instructions::helpers::STAKE_PROGRAM_ID;
    use solana_program::example_mocks::solana_sdk::system_program;
    use solana_sdk::instruction::{AccountMeta, Instruction};
    use solana_sdk::transaction::Transaction;

    let rent_sysvar = solana_sdk::sysvar::rent::id();
    let clock_sysvar = solana_sdk::sysvar::clock::id();

    let nonce_bytes = nonce.to_le_bytes();
    let depositor_stake_account = Pubkey::find_program_address(
        &[b"split_account", depositor.pubkey().as_ref(), &nonce_bytes],
        &PROGRAM_ID,
    )
    .0;

    let mut crank_split_data = vec![4u8];

    crank_split_data.extend_from_slice(&lamports_to_split.to_le_bytes());
    crank_split_data.extend_from_slice(&nonce_bytes);

    let ix = Instruction {
        program_id: PROGRAM_ID,
        data: crank_split_data,
        accounts: vec![
            AccountMeta::new(*stake_account_main, false),
            AccountMeta::new(*stake_account_reserve, false),
            AccountMeta::new(depositor.pubkey(), true),
            AccountMeta::new(depositor_stake_account, false),
            AccountMeta::new(*config_pda, false),
            AccountMeta::new(*depositor_ata, false),
            AccountMeta::new(*token_mint_pubkey, false),
            AccountMeta::new_readonly(rent_sysvar, false),
            AccountMeta::new_readonly(clock_sysvar, false),
            AccountMeta::new_readonly(spl_token::ID, false),
            AccountMeta::new_readonly(Pubkey::from(STAKE_PROGRAM_ID), false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
    };

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&depositor.pubkey()),
        &[depositor],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    println!("PRINTING CRANK SPLIT TRANSACTION LOGS");
    print_transaction_logs(&result);
    assert!(result.is_ok(), "CrankSplit transaction should succeed");

    depositor_stake_account
}

/// Builds a Withdraw instruction with the given accounts.
pub fn build_withdraw_ix(
    depositor_stake_account: &Pubkey,
    withdrawer: &Pubkey,
    config_pda: &Pubkey,
    stake_program_id: &Pubkey,
    nonce: u64,
    withdrawer_is_signer: bool,
) -> solana_sdk::instruction::Instruction {
    use solana_sdk::instruction::{AccountMeta, Instruction};

    let clock_sysvar = solana_sdk::sysvar::clock::id();

    let mut data = vec![5u8];
    data.extend_from_slice(&nonce.to_le_bytes());

    Instruction {
        program_id: PROGRAM_ID,
        data,
        accounts: vec![
            AccountMeta::new(*depositor_stake_account, false),
            AccountMeta::new(*withdrawer, withdrawer_is_signer),
            AccountMeta::new_readonly(clock_sysvar, false),
            AccountMeta::new_readonly(HISTORY_SYSVAR, false),
            AccountMeta::new(*config_pda, false),
            AccountMeta::new_readonly(*stake_program_id, false),
        ],
    }
}

/// Sends a Withdraw transaction.
pub fn run_withdraw(
    svm: &mut LiteSVM,
    depositor: &Keypair,
    depositor_stake_account: &Pubkey,
    config_pda: &Pubkey,
    nonce: u64,
) {
    use solana_liquid_staking::instructions::helpers::STAKE_PROGRAM_ID;
    use solana_sdk::instruction::{AccountMeta, Instruction};
    use solana_sdk::transaction::Transaction;

    let clock_sysvar = solana_sdk::sysvar::clock::id();

    let nonce_bytes = nonce.to_le_bytes();
    let mut data = vec![5u8];
    data.extend_from_slice(&nonce_bytes);

    let ix = Instruction {
        program_id: PROGRAM_ID,
        data,
        accounts: vec![
            AccountMeta::new(*depositor_stake_account, false),
            AccountMeta::new(depositor.pubkey(), true),
            AccountMeta::new_readonly(clock_sysvar, false),
            AccountMeta::new_readonly(HISTORY_SYSVAR, false),
            AccountMeta::new(*config_pda, false),
            AccountMeta::new_readonly(Pubkey::from(STAKE_PROGRAM_ID), false),
        ],
    };

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&depositor.pubkey()),
        &[depositor],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    println!("PRINTING WITHDRAW TRANSACTION LOGS");
    print_transaction_logs(&result);
    assert!(result.is_ok(), "Withdraw transaction should succeed");
}

/// Builds a CrankInitializeReserve instruction with the given accounts.
pub fn build_crank_initialize_reserve_ix(
    config_pda: &Pubkey,
    stake_account_reserve: &Pubkey,
    vote_pubkey: &Pubkey,
    system_program_id: &Pubkey,
    stake_program_id: &Pubkey,
) -> solana_sdk::instruction::Instruction {
    use solana_sdk::instruction::{AccountMeta, Instruction};

    let rent_sysvar = solana_sdk::sysvar::rent::id();
    let clock_sysvar = solana_sdk::sysvar::clock::id();

    Instruction {
        program_id: PROGRAM_ID,
        data: vec![1u8],
        accounts: vec![
            AccountMeta::new(*config_pda, false),
            AccountMeta::new(*stake_account_reserve, false),
            AccountMeta::new(*vote_pubkey, false),
            AccountMeta::new_readonly(*stake_program_id, false),
            AccountMeta::new_readonly(rent_sysvar, false),
            AccountMeta::new_readonly(clock_sysvar, false),
            AccountMeta::new_readonly(HISTORY_SYSVAR, false),
            AccountMeta::new_readonly(*system_program_id, false),
            AccountMeta::new_readonly(*stake_program_id, false),
        ],
    }
}

/// Builds the Initialize instruction with the given accounts.
pub fn build_initialize_ix(
    initializer: &Pubkey,
    initializer_ata: &Pubkey,
    config_pda: &Pubkey,
    stake_account_main: &Pubkey,
    stake_account_reserve: &Pubkey,
    token_mint: &Pubkey,
    token_mint_is_signer: bool,
    vote_pubkey: &Pubkey,
    system_program_id: &Pubkey,
    stake_program_id: &Pubkey,
    token_program_id: &Pubkey,
    associated_token_program_id: &Pubkey,
) -> solana_sdk::instruction::Instruction {
    use solana_sdk::instruction::{AccountMeta, Instruction};

    let rent_sysvar = solana_sdk::sysvar::rent::id();
    let clock_sysvar = solana_sdk::sysvar::clock::id();

    Instruction {
        program_id: PROGRAM_ID,
        data: vec![0u8],
        accounts: vec![
            AccountMeta::new(*initializer, true),
            AccountMeta::new(*initializer_ata, false),
            AccountMeta::new(*config_pda, false),
            AccountMeta::new(*stake_account_main, false),
            AccountMeta::new(*stake_account_reserve, false),
            AccountMeta::new(*token_mint, token_mint_is_signer),
            AccountMeta::new(*vote_pubkey, false),
            AccountMeta::new(Pubkey::new_unique(), false),
            AccountMeta::new_readonly(*system_program_id, false),
            AccountMeta::new_readonly(*stake_program_id, false),
            AccountMeta::new_readonly(*token_program_id, false),
            AccountMeta::new_readonly(*associated_token_program_id, false),
            AccountMeta::new_readonly(rent_sysvar, false),
            AccountMeta::new_readonly(clock_sysvar, false),
            AccountMeta::new_readonly(HISTORY_SYSVAR, false),
        ],
    }
}
