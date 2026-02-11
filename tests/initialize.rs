mod test_helpers;

#[cfg(test)]
mod tests {
    use solana_liquid_staking::instructions::helpers::STAKE_PROGRAM_ID;
    use solana_program::example_mocks::solana_sdk::system_program;
    use solana_pubkey::Pubkey;
    use solana_sdk::{
        account::Account,
        instruction::{AccountMeta, Instruction},
        signature::Keypair,
        signer::Signer,
        transaction::Transaction,
    };

    use crate::test_helpers::test_helpers::{
        build_initialize_ix, create_and_fund_ata, create_mock_token_mint, print_transaction_logs,
        setup_initialize_accounts, setup_svm, HISTORY_SYSVAR, PROGRAM_ID,
    };

    #[test]
    fn test_initialize_success() {
        let mut svm = setup_svm();
        let (initializer, token_mint, initializer_ata, config_pda, stake_account_main, stake_account_reserve, vote_pubkey) =
            setup_initialize_accounts(&mut svm);

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
        assert!(result.is_ok(), "Transaction should succeed");

        let account_opt = svm.get_account(&config_pda);
        assert!(account_opt.is_some(), "Account should exist");

        let account = account_opt.unwrap();
        assert_eq!(account.owner, PROGRAM_ID, "Should be owned by program");
        assert!(account.lamports > 0, "Should have lamports for rent");
    }

    #[test]
    fn test_initialize_fail_initializer_not_signer() {
        let mut svm = setup_svm();
        let (initializer, token_mint, initializer_ata, config_pda, stake_account_main, stake_account_reserve, vote_pubkey) =
            setup_initialize_accounts(&mut svm);

        // Use a separate fee payer so initializer is NOT automatically a signer
        let fee_payer = Keypair::new();
        svm.airdrop(&fee_payer.pubkey(), 10_000_000_000).unwrap();

        let rent_sysvar = solana_sdk::sysvar::rent::id();
        let clock_sysvar = solana_sdk::sysvar::clock::id();

        // SCREWING UP: initializer is_signer = false
        let ix = Instruction {
            program_id: PROGRAM_ID,
            data: vec![0u8],
            accounts: vec![
                AccountMeta::new(initializer.pubkey(), false), // <-- not a signer
                AccountMeta::new(initializer_ata, false),
                AccountMeta::new(config_pda, false),
                AccountMeta::new(stake_account_main, false),
                AccountMeta::new(stake_account_reserve, false),
                AccountMeta::new(token_mint.pubkey(), true),
                AccountMeta::new(vote_pubkey, false),
                AccountMeta::new(Pubkey::new_unique(), false),
                AccountMeta::new_readonly(system_program::ID, false),
                AccountMeta::new_readonly(Pubkey::from(STAKE_PROGRAM_ID), false),
                AccountMeta::new_readonly(spl_token::ID, false),
                AccountMeta::new_readonly(spl_associated_token_account::ID, false),
                AccountMeta::new_readonly(rent_sysvar, false),
                AccountMeta::new_readonly(clock_sysvar, false),
                AccountMeta::new_readonly(HISTORY_SYSVAR, false),
            ],
        };

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&fee_payer.pubkey()),        // <-- fee payer is NOT the initializer
            &[&fee_payer, &token_mint],        // <-- initializer not included as signer
            svm.latest_blockhash(),
        );

        let result = svm.send_transaction(tx);
        print_transaction_logs(&result);
        assert!(result.is_err(), "Should fail: initializer is not a signer");
    }

    #[test]
    fn test_initialize_fail_lst_mint_not_signer() {
        let mut svm = setup_svm();
        let (initializer, token_mint, initializer_ata, config_pda, stake_account_main, stake_account_reserve, vote_pubkey) =
            setup_initialize_accounts(&mut svm);

        // SCREWING UP: token_mint is_signer = false
        let ix = build_initialize_ix(
            &initializer.pubkey(),
            &initializer_ata,
            &config_pda,
            &stake_account_main,
            &stake_account_reserve,
            &token_mint.pubkey(),
            false, // <-- lst_mint not a signer
            &vote_pubkey,
            &system_program::ID,
            &Pubkey::from(STAKE_PROGRAM_ID),
            &spl_token::ID,
            &spl_associated_token_account::ID,
        );

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&initializer.pubkey()),
            &[&initializer], // <-- token_mint not included as signer
            svm.latest_blockhash(),
        );

        let result = svm.send_transaction(tx);
        print_transaction_logs(&result);
        assert!(result.is_err(), "Should fail: lst_mint is not a signer");
    }

    #[test]
    fn test_initialize_fail_wrong_config_pda() {
        let mut svm = setup_svm();
        let (initializer, token_mint, initializer_ata, _config_pda, stake_account_main, stake_account_reserve, vote_pubkey) =
            setup_initialize_accounts(&mut svm);

        // SCREWING UP: deriving config PDA with wrong seed
        let wrong_config_pda = Pubkey::find_program_address(&[b"wrong_config"], &PROGRAM_ID).0;

        let ix = build_initialize_ix(
            &initializer.pubkey(),
            &initializer_ata,
            &wrong_config_pda, // <-- wrong config PDA
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
        assert!(result.is_err(), "Should fail: wrong config PDA derivation");
    }

    #[test]
    fn test_initialize_fail_stake_account_main_already_initialized() {
        let mut svm = setup_svm();
        let (initializer, token_mint, initializer_ata, config_pda, stake_account_main, stake_account_reserve, vote_pubkey) =
            setup_initialize_accounts(&mut svm);

        // SCREWING UP: pre-initializing stake_account_main so it's not empty
        svm.set_account(
            stake_account_main,
            Account {
                lamports: 10_000_000,
                data: vec![0u8; 200], // <-- non-empty data, simulates already initialized
                owner: Pubkey::from(STAKE_PROGRAM_ID),
                executable: false,
                rent_epoch: 0,
            }
            .into(),
        )
        .unwrap();

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
        assert!(result.is_err(), "Should fail: stake_account_main already initialized");
    }

    #[test]
    fn test_initialize_fail_stake_account_reserve_already_initialized() {
        let mut svm = setup_svm();
        let (initializer, token_mint, initializer_ata, config_pda, stake_account_main, stake_account_reserve, vote_pubkey) =
            setup_initialize_accounts(&mut svm);

        // SCREWING UP: pre-initializing stake_account_reserve so it's not empty
        svm.set_account(
            stake_account_reserve,
            Account {
                lamports: 10_000_000,
                data: vec![0u8; 200], // <-- non-empty data, simulates already initialized
                owner: Pubkey::from(STAKE_PROGRAM_ID),
                executable: false,
                rent_epoch: 0,
            }
            .into(),
        )
        .unwrap();

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
        assert!(result.is_err(), "Should fail: stake_account_reserve already initialized");
    }

    #[test]
    fn test_initialize_fail_wrong_stake_account_main() {
        let mut svm = setup_svm();
        let (initializer, token_mint, initializer_ata, config_pda, _stake_account_main, stake_account_reserve, vote_pubkey) =
            setup_initialize_accounts(&mut svm);

        // SCREWING UP: deriving stake_account_main with wrong seed
        let wrong_stake_main = Pubkey::find_program_address(&[b"wrong_stake_main"], &PROGRAM_ID).0;

        let ix = build_initialize_ix(
            &initializer.pubkey(),
            &initializer_ata,
            &config_pda,
            &wrong_stake_main, // <-- wrong derivation
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
        assert!(result.is_err(), "Should fail: wrong stake_account_main derivation");
    }

    #[test]
    fn test_initialize_fail_wrong_stake_account_reserve() {
        let mut svm = setup_svm();
        let (initializer, token_mint, initializer_ata, config_pda, stake_account_main, _stake_account_reserve, vote_pubkey) =
            setup_initialize_accounts(&mut svm);

        // SCREWING UP: deriving stake_account_reserve with wrong seed
        let wrong_stake_reserve = Pubkey::find_program_address(&[b"wrong_reserve"], &PROGRAM_ID).0;

        let ix = build_initialize_ix(
            &initializer.pubkey(),
            &initializer_ata,
            &config_pda,
            &stake_account_main,
            &wrong_stake_reserve, // <-- wrong derivation
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
        assert!(result.is_err(), "Should fail: wrong stake_account_reserve derivation");
    }

    #[test]
    fn test_initialize_fail_wrong_system_program() {
        let mut svm = setup_svm();
        let (initializer, token_mint, initializer_ata, config_pda, stake_account_main, stake_account_reserve, vote_pubkey) =
            setup_initialize_accounts(&mut svm);

        // SCREWING UP: passing a fake system program
        let fake_system_program = Pubkey::new_unique();

        let ix = build_initialize_ix(
            &initializer.pubkey(),
            &initializer_ata,
            &config_pda,
            &stake_account_main,
            &stake_account_reserve,
            &token_mint.pubkey(),
            true,
            &vote_pubkey,
            &fake_system_program, // <-- wrong system program
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
        assert!(result.is_err(), "Should fail: wrong system program");
    }

    #[test]
    fn test_initialize_fail_wrong_token_program() {
        let mut svm = setup_svm();
        let (initializer, token_mint, initializer_ata, config_pda, stake_account_main, stake_account_reserve, vote_pubkey) =
            setup_initialize_accounts(&mut svm);

        // SCREWING UP: passing a fake token program
        let fake_token_program = Pubkey::new_unique();

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
            &fake_token_program, // <-- wrong token program
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
        assert!(result.is_err(), "Should fail: wrong token program");
    }

    #[test]
    fn test_initialize_fail_wrong_associated_token_program() {
        let mut svm = setup_svm();
        let (initializer, token_mint, initializer_ata, config_pda, stake_account_main, stake_account_reserve, vote_pubkey) =
            setup_initialize_accounts(&mut svm);

        // SCREWING UP: passing a fake associated token program
        let fake_ata_program = Pubkey::new_unique();

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
            &fake_ata_program, // <-- wrong associated token program
        );

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&initializer.pubkey()),
            &[&initializer, &token_mint],
            svm.latest_blockhash(),
        );

        let result = svm.send_transaction(tx);
        print_transaction_logs(&result);
        assert!(result.is_err(), "Should fail: wrong associated token program");
    }

    #[test]
    fn test_initialize_fail_wrong_stake_program() {
        let mut svm = setup_svm();
        let (initializer, token_mint, initializer_ata, config_pda, stake_account_main, stake_account_reserve, vote_pubkey) =
            setup_initialize_accounts(&mut svm);

        // SCREWING UP: passing a fake stake program
        let fake_stake_program = Pubkey::new_unique();

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
            &fake_stake_program, // <-- wrong stake program
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
        assert!(result.is_err(), "Should fail: wrong stake program");
    }

    #[test]
    fn test_initialize_fail_insufficient_sol() {
        let mut svm = setup_svm();
        let (_initializer, token_mint, initializer_ata, config_pda, stake_account_main, stake_account_reserve, vote_pubkey) =
            setup_initialize_accounts(&mut svm);

        // SCREWING UP: replace initializer with a new keypair that has almost no SOL
        let broke_initializer = Keypair::new();
        svm.airdrop(&broke_initializer.pubkey(), 1_000).unwrap(); // <-- only 1000 lamports

        let ix = build_initialize_ix(
            &broke_initializer.pubkey(), // <-- underfunded initializer
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
            Some(&broke_initializer.pubkey()),
            &[&broke_initializer, &token_mint],
            svm.latest_blockhash(),
        );

        let result = svm.send_transaction(tx);
        print_transaction_logs(&result);
        assert!(result.is_err(), "Should fail: initializer has insufficient SOL");
    }

    #[test]
    fn test_initialize_fail_wrong_mint_authority() {
        let mut svm = setup_svm();
        let (initializer, _token_mint, initializer_ata, config_pda, stake_account_main, stake_account_reserve, vote_pubkey) =
            setup_initialize_accounts(&mut svm);

        // SCREWING UP: creating mint with wrong authority (random key instead of config_pda)
        let wrong_authority = Pubkey::new_unique();
        let bad_mint = create_mock_token_mint(&mut svm, &wrong_authority); // <-- wrong mint authority

        let ix = build_initialize_ix(
            &initializer.pubkey(),
            &initializer_ata,
            &config_pda,
            &stake_account_main,
            &stake_account_reserve,
            &bad_mint.pubkey(), // <-- mint whose authority is not config_pda
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
            &[&initializer, &bad_mint],
            svm.latest_blockhash(),
        );

        let result = svm.send_transaction(tx);
        print_transaction_logs(&result);
        assert!(result.is_err(), "Should fail: mint has wrong authority");
    }

    #[test]
    fn test_initialize_fail_wrong_ata_owner() {
        let mut svm = setup_svm();
        let (initializer, token_mint, _initializer_ata, config_pda, stake_account_main, stake_account_reserve, vote_pubkey) =
            setup_initialize_accounts(&mut svm);

        // SCREWING UP: creating an ATA that belongs to a different owner
        let other_owner = Keypair::new();
        svm.airdrop(&other_owner.pubkey(), 1_000_000_000).unwrap();
        let wrong_ata = create_and_fund_ata(&mut svm, &other_owner.pubkey(), &token_mint.pubkey(), 0); // <-- ATA owned by someone else

        let ix = build_initialize_ix(
            &initializer.pubkey(),
            &wrong_ata, // <-- ATA belongs to other_owner, not initializer
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
        assert!(result.is_err(), "Should fail: ATA belongs to wrong owner");
    }
}
