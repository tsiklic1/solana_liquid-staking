mod test_helpers;

#[cfg(test)]
mod tests {
    use solana_sdk::pubkey::Pubkey;
    use solana_sdk::signature::Keypair;
    use solana_sdk::signer::Signer;
    use solana_sdk::transaction::Transaction;

    use solana_liquid_staking::instructions::helpers::STAKE_PROGRAM_ID;

    use crate::test_helpers::test_helpers::{
        build_withdraw_ix, print_transaction_logs, run_crank_initialize_reserve,
        run_crank_merge_reserve, run_crank_split, run_deposit, run_initialize, run_withdraw,
        setup_svm, PROGRAM_ID,
    };

    /// Sets up a pool ready for withdraw: initialize + deposit + crank_init_reserve + merge + split.
    /// Returns (initializer, token_mint, depositor, depositor_ata, config_pda,
    ///          stake_account_main, stake_account_reserve, depositor_stake_account, vote_pubkey).
    fn setup_withdraw_ready_pool(
        svm: &mut litesvm::LiteSVM,
        deposit_amount: u64,
        lamports_to_split: u64,
    ) -> (
        Keypair, // initializer
        Keypair, // token_mint
        Keypair, // depositor
        Pubkey,  // depositor_ata
        Pubkey,  // config_pda
        Pubkey,  // stake_account_main
        Pubkey,  // stake_account_reserve
        Pubkey,  // depositor_stake_account
        Pubkey,  // vote_pubkey
    ) {
        let (
            initializer,
            token_mint,
            _initializer_ata,
            config_pda,
            stake_account_main,
            stake_account_reserve,
            vote_pubkey,
        ) = run_initialize(svm);

        let (depositor, depositor_ata) = run_deposit(
            svm,
            &config_pda,
            &token_mint.pubkey(),
            &stake_account_main,
            &stake_account_reserve,
            deposit_amount,
        );

        run_crank_initialize_reserve(
            svm,
            &initializer,
            &config_pda,
            &stake_account_reserve,
            &vote_pubkey,
        );

        run_crank_merge_reserve(
            svm,
            &initializer,
            &config_pda,
            &stake_account_main,
            &stake_account_reserve,
        );

        let depositor_stake_account = run_crank_split(
            svm,
            &depositor,
            &depositor_ata,
            &config_pda,
            &stake_account_main,
            &stake_account_reserve,
            &token_mint.pubkey(),
            lamports_to_split,
            123,
        );

        (
            initializer,
            token_mint,
            depositor,
            depositor_ata,
            config_pda,
            stake_account_main,
            stake_account_reserve,
            depositor_stake_account,
            vote_pubkey,
        )
    }

    #[test]
    fn test_withdraw_success() {
        let mut svm = setup_svm();
        let (
            _initializer,
            _token_mint,
            depositor,
            _depositor_ata,
            config_pda,
            _stake_account_main,
            _stake_account_reserve,
            depositor_stake_account,
            _vote_pubkey,
        ) = setup_withdraw_ready_pool(&mut svm, 2_000_000_000, 1_500_000_000);

        run_withdraw(&mut svm, &depositor, &depositor_stake_account, &config_pda, 123);
    }

    #[test]
    fn test_withdraw_two_withdrawals() {
        let mut svm = setup_svm();
        let (
            initializer,
            token_mint,
            _initializer_ata,
            config_pda,
            stake_account_main,
            stake_account_reserve,
            vote_pubkey,
        ) = run_initialize(&mut svm);

        // Two depositors deposit before the crank cycle
        let deposit_amount = 2_000_000_000u64;
        let (depositor1, depositor1_ata) = run_deposit(
            &mut svm,
            &config_pda,
            &token_mint.pubkey(),
            &stake_account_main,
            &stake_account_reserve,
            deposit_amount,
        );
        let (depositor2, depositor2_ata) = run_deposit(
            &mut svm,
            &config_pda,
            &token_mint.pubkey(),
            &stake_account_main,
            &stake_account_reserve,
            deposit_amount,
        );

        // Single crank cycle
        run_crank_initialize_reserve(
            &mut svm,
            &initializer,
            &config_pda,
            &stake_account_reserve,
            &vote_pubkey,
        );
        run_crank_merge_reserve(
            &mut svm,
            &initializer,
            &config_pda,
            &stake_account_main,
            &stake_account_reserve,
        );

        // Both depositors split
        let depositor1_stake = run_crank_split(
            &mut svm,
            &depositor1,
            &depositor1_ata,
            &config_pda,
            &stake_account_main,
            &stake_account_reserve,
            &token_mint.pubkey(),
            1_500_000_000,
            123,
        );
        let depositor2_stake = run_crank_split(
            &mut svm,
            &depositor2,
            &depositor2_ata,
            &config_pda,
            &stake_account_main,
            &stake_account_reserve,
            &token_mint.pubkey(),
            1_500_000_000,
            123,
        );

        // Both withdraw successfully
        run_withdraw(&mut svm, &depositor1, &depositor1_stake, &config_pda, 123);
        run_withdraw(&mut svm, &depositor2, &depositor2_stake, &config_pda, 123);
    }

    #[test]
    fn test_withdraw_double_withdraw() {
        let mut svm = setup_svm();
        let (
            _initializer,
            _token_mint,
            depositor,
            _depositor_ata,
            config_pda,
            _stake_account_main,
            _stake_account_reserve,
            depositor_stake_account,
            _vote_pubkey,
        ) = setup_withdraw_ready_pool(&mut svm, 2_000_000_000, 1_500_000_000);

        // First withdraw should succeed
        run_withdraw(&mut svm, &depositor, &depositor_stake_account, &config_pda, 123);

        // Second withdraw from the same split account should fail
        let stake_program = Pubkey::from(STAKE_PROGRAM_ID);
        let ix = build_withdraw_ix(
            &depositor_stake_account,
            &depositor.pubkey(),
            &config_pda,
            &stake_program,
            123,
            true,
        );

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&depositor.pubkey()),
            &[&depositor],
            svm.latest_blockhash(),
        );

        let result = svm.send_transaction(tx);
        print_transaction_logs(&result);
        assert!(result.is_err(), "Should fail on double withdraw");
    }

    #[test]
    fn test_withdraw_wrong_withdrawer() {
        let mut svm = setup_svm();
        let (
            _initializer,
            _token_mint,
            _depositor,
            _depositor_ata,
            config_pda,
            _stake_account_main,
            _stake_account_reserve,
            depositor_stake_account,
            _vote_pubkey,
        ) = setup_withdraw_ready_pool(&mut svm, 2_000_000_000, 1_500_000_000);

        // A different user tries to withdraw from the depositor's split account
        let wrong_withdrawer = Keypair::new();
        svm.airdrop(&wrong_withdrawer.pubkey(), 10_000_000_000)
            .unwrap();

        let stake_program = Pubkey::from(STAKE_PROGRAM_ID);
        let ix = build_withdraw_ix(
            &depositor_stake_account,
            &wrong_withdrawer.pubkey(),
            &config_pda,
            &stake_program,
            123,
            true,
        );

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&wrong_withdrawer.pubkey()),
            &[&wrong_withdrawer],
            svm.latest_blockhash(),
        );

        let result = svm.send_transaction(tx);
        print_transaction_logs(&result);
        assert!(result.is_err(), "Should fail with wrong withdrawer");
    }

    #[test]
    fn test_withdraw_wrong_config_pda() {
        let mut svm = setup_svm();
        let (
            _initializer,
            _token_mint,
            depositor,
            _depositor_ata,
            _config_pda,
            _stake_account_main,
            _stake_account_reserve,
            depositor_stake_account,
            _vote_pubkey,
        ) = setup_withdraw_ready_pool(&mut svm, 2_000_000_000, 1_500_000_000);

        let wrong_config = Pubkey::new_unique();
        let stake_program = Pubkey::from(STAKE_PROGRAM_ID);
        let ix = build_withdraw_ix(
            &depositor_stake_account,
            &depositor.pubkey(),
            &wrong_config,
            &stake_program,
            123,
            true,
        );

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&depositor.pubkey()),
            &[&depositor],
            svm.latest_blockhash(),
        );

        let result = svm.send_transaction(tx);
        print_transaction_logs(&result);
        assert!(result.is_err(), "Should fail with wrong config PDA");
    }

    #[test]
    fn test_withdraw_wrong_stake_program() {
        let mut svm = setup_svm();
        let (
            _initializer,
            _token_mint,
            depositor,
            _depositor_ata,
            config_pda,
            _stake_account_main,
            _stake_account_reserve,
            depositor_stake_account,
            _vote_pubkey,
        ) = setup_withdraw_ready_pool(&mut svm, 2_000_000_000, 1_500_000_000);

        let wrong_stake_program = Pubkey::new_unique();
        let ix = build_withdraw_ix(
            &depositor_stake_account,
            &depositor.pubkey(),
            &config_pda,
            &wrong_stake_program,
            123,
            true,
        );

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&depositor.pubkey()),
            &[&depositor],
            svm.latest_blockhash(),
        );

        let result = svm.send_transaction(tx);
        print_transaction_logs(&result);
        assert!(result.is_err(), "Should fail with wrong stake program");
    }

    #[test]
    fn test_withdraw_nonexistent_split_account() {
        let mut svm = setup_svm();
        let (
            _initializer,
            _token_mint,
            _initializer_ata,
            config_pda,
            _stake_account_main,
            _stake_account_reserve,
            _vote_pubkey,
        ) = run_initialize(&mut svm);

        // A user that never did crank_split tries to withdraw
        let user = Keypair::new();
        svm.airdrop(&user.pubkey(), 10_000_000_000).unwrap();

        // Derive the split account PDA that was never created
        let nonce: u64 = 123;
        let nonce_bytes = nonce.to_le_bytes();
        let nonexistent_split = Pubkey::find_program_address(
            &[b"split_account", user.pubkey().as_ref(), &nonce_bytes],
            &PROGRAM_ID,
        )
        .0;

        let stake_program = Pubkey::from(STAKE_PROGRAM_ID);
        let ix = build_withdraw_ix(
            &nonexistent_split,
            &user.pubkey(),
            &config_pda,
            &stake_program,
            nonce,
            true,
        );

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&user.pubkey()),
            &[&user],
            svm.latest_blockhash(),
        );

        let result = svm.send_transaction(tx);
        print_transaction_logs(&result);
        assert!(
            result.is_err(),
            "Should fail when split account was never created"
        );
    }

    #[test]
    fn test_withdraw_two_splits_different_nonces() {
        let mut svm = setup_svm();
        let (
            initializer,
            token_mint,
            _initializer_ata,
            config_pda,
            stake_account_main,
            stake_account_reserve,
            vote_pubkey,
        ) = run_initialize(&mut svm);

        // Deposit enough to cover two splits
        let deposit_amount = 5_000_000_000u64;
        let (depositor, depositor_ata) = run_deposit(
            &mut svm,
            &config_pda,
            &token_mint.pubkey(),
            &stake_account_main,
            &stake_account_reserve,
            deposit_amount,
        );

        run_crank_initialize_reserve(
            &mut svm,
            &initializer,
            &config_pda,
            &stake_account_reserve,
            &vote_pubkey,
        );
        run_crank_merge_reserve(
            &mut svm,
            &initializer,
            &config_pda,
            &stake_account_main,
            &stake_account_reserve,
        );

        // First split with nonce 1
        let split_account_1 = run_crank_split(
            &mut svm,
            &depositor,
            &depositor_ata,
            &config_pda,
            &stake_account_main,
            &stake_account_reserve,
            &token_mint.pubkey(),
            1_500_000_000,
            1,
        );

        // Second split with nonce 2
        let split_account_2 = run_crank_split(
            &mut svm,
            &depositor,
            &depositor_ata,
            &config_pda,
            &stake_account_main,
            &stake_account_reserve,
            &token_mint.pubkey(),
            1_500_000_000,
            2,
        );

        // Both should be different PDAs
        assert_ne!(
            split_account_1, split_account_2,
            "Different nonces should produce different split account PDAs"
        );

        // Both withdrawals should succeed
        run_withdraw(&mut svm, &depositor, &split_account_1, &config_pda, 1);
        run_withdraw(&mut svm, &depositor, &split_account_2, &config_pda, 2);
    }

    #[test]
    fn test_withdraw_verify_sol_amount_received() {
        let mut svm = setup_svm();
        let (
            _initializer,
            _token_mint,
            depositor,
            _depositor_ata,
            config_pda,
            _stake_account_main,
            _stake_account_reserve,
            depositor_stake_account,
            _vote_pubkey,
        ) = setup_withdraw_ready_pool(&mut svm, 2_000_000_000, 1_500_000_000);

        let split_account_balance = svm
            .get_account(&depositor_stake_account)
            .unwrap()
            .lamports;
        let withdrawer_balance_before = svm.get_account(&depositor.pubkey()).unwrap().lamports;

        run_withdraw(&mut svm, &depositor, &depositor_stake_account, &config_pda, 123);

        let withdrawer_balance_after = svm.get_account(&depositor.pubkey()).unwrap().lamports;

        // The withdrawer pays a tx fee, so the increase should be
        // split_account_balance minus the transaction fee.
        let balance_increase = withdrawer_balance_after - withdrawer_balance_before;
        let tx_fee = 5000u64; // standard Solana tx fee

        assert_eq!(
            balance_increase,
            split_account_balance - tx_fee,
            "Withdrawer should receive the full split account balance minus tx fee. \
             Expected increase: {}, actual increase: {}",
            split_account_balance - tx_fee,
            balance_increase,
        );
    }
}
