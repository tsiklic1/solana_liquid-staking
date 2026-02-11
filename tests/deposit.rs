mod test_helpers;

#[cfg(test)]
mod tests {
    use solana_sdk::pubkey::Pubkey;
    use solana_sdk::signature::Keypair;
    use solana_sdk::signer::Signer;
    use solana_sdk::transaction::Transaction;

    use crate::test_helpers::test_helpers::{
        build_deposit_ix, create_and_fund_ata, print_transaction_logs, run_deposit,
        run_initialize, setup_svm,
    };

    #[test]
    fn test_deposit_success() {
        let mut svm = setup_svm();
        let (
            _initializer,
            token_mint,
            _initializer_ata,
            config_pda,
            stake_account_main,
            stake_account_reserve,
            _vote_pubkey,
        ) = run_initialize(&mut svm);

        let deposit_amount = 2_000_000_000u64;
        let (_depositor, _depositor_ata) = run_deposit(
            &mut svm,
            &config_pda,
            &token_mint.pubkey(),
            &stake_account_main,
            &stake_account_reserve,
            deposit_amount,
        );
    }

    #[test]
    fn test_deposit_less_than_minimum_amount() {
        let mut svm = setup_svm();
        let (
            _initializer,
            token_mint,
            _initializer_ata,
            config_pda,
            stake_account_main,
            stake_account_reserve,
            _vote_pubkey,
        ) = run_initialize(&mut svm);

        let depositor = Keypair::new();
        svm.airdrop(&depositor.pubkey(), 10_000_000_000).unwrap();
        let depositor_ata =
            create_and_fund_ata(&mut svm, &depositor.pubkey(), &token_mint.pubkey(), 0);

        // Less than 1 SOL (1_000_000_000 lamports)
        let small_amount = 500_000_000u64;
        let ix = build_deposit_ix(
            &config_pda,
            &depositor.pubkey(),
            &depositor_ata,
            &token_mint.pubkey(),
            &stake_account_main,
            &stake_account_reserve,
            small_amount,
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
        assert!(
            result.is_err(),
            "Should fail with deposit less than minimum"
        );
    }

    #[test]
    fn test_deposit_wrong_config_pda() {
        let mut svm = setup_svm();
        let (
            _initializer,
            token_mint,
            _initializer_ata,
            _config_pda,
            stake_account_main,
            stake_account_reserve,
            _vote_pubkey,
        ) = run_initialize(&mut svm);

        let depositor = Keypair::new();
        svm.airdrop(&depositor.pubkey(), 10_000_000_000).unwrap();
        let depositor_ata =
            create_and_fund_ata(&mut svm, &depositor.pubkey(), &token_mint.pubkey(), 0);

        let wrong_config = Pubkey::new_unique();
        let ix = build_deposit_ix(
            &wrong_config,
            &depositor.pubkey(),
            &depositor_ata,
            &token_mint.pubkey(),
            &stake_account_main,
            &stake_account_reserve,
            2_000_000_000,
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
    fn test_deposit_wrong_reserve_account() {
        let mut svm = setup_svm();
        let (
            _initializer,
            token_mint,
            _initializer_ata,
            config_pda,
            stake_account_main,
            _stake_account_reserve,
            _vote_pubkey,
        ) = run_initialize(&mut svm);

        let depositor = Keypair::new();
        svm.airdrop(&depositor.pubkey(), 10_000_000_000).unwrap();
        let depositor_ata =
            create_and_fund_ata(&mut svm, &depositor.pubkey(), &token_mint.pubkey(), 0);

        let wrong_reserve = Pubkey::new_unique();
        let ix = build_deposit_ix(
            &config_pda,
            &depositor.pubkey(),
            &depositor_ata,
            &token_mint.pubkey(),
            &stake_account_main,
            &wrong_reserve,
            2_000_000_000,
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
        assert!(result.is_err(), "Should fail with wrong reserve account");
    }

    #[test]
    fn test_deposit_wrong_lst_mint() {
        let mut svm = setup_svm();
        let (
            _initializer,
            _token_mint,
            _initializer_ata,
            config_pda,
            stake_account_main,
            stake_account_reserve,
            _vote_pubkey,
        ) = run_initialize(&mut svm);

        let depositor = Keypair::new();
        svm.airdrop(&depositor.pubkey(), 10_000_000_000).unwrap();

        // Create a different mint that doesn't match the config
        let wrong_mint =
            crate::test_helpers::test_helpers::create_mock_token_mint(&mut svm, &config_pda);
        let depositor_ata =
            create_and_fund_ata(&mut svm, &depositor.pubkey(), &wrong_mint.pubkey(), 0);

        let ix = build_deposit_ix(
            &config_pda,
            &depositor.pubkey(),
            &depositor_ata,
            &wrong_mint.pubkey(),
            &stake_account_main,
            &stake_account_reserve,
            2_000_000_000,
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
        assert!(result.is_err(), "Should fail with wrong LST mint");
    }

    #[test]
    fn test_deposit_missing_depositor_signature() {
        let mut svm = setup_svm();
        let (
            initializer,
            token_mint,
            _initializer_ata,
            config_pda,
            stake_account_main,
            stake_account_reserve,
            _vote_pubkey,
        ) = run_initialize(&mut svm);

        let depositor = Keypair::new();
        svm.airdrop(&depositor.pubkey(), 10_000_000_000).unwrap();
        let depositor_ata =
            create_and_fund_ata(&mut svm, &depositor.pubkey(), &token_mint.pubkey(), 0);

        // Build ix with depositor_is_signer = false
        let ix = build_deposit_ix(
            &config_pda,
            &depositor.pubkey(),
            &depositor_ata,
            &token_mint.pubkey(),
            &stake_account_main,
            &stake_account_reserve,
            2_000_000_000,
            false,
        );

        // Sign only with the initializer (fee payer), not the depositor
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&initializer.pubkey()),
            &[&initializer],
            svm.latest_blockhash(),
        );

        let result = svm.send_transaction(tx);
        print_transaction_logs(&result);
        assert!(
            result.is_err(),
            "Should fail with missing depositor signature"
        );
    }

    #[test]
    fn test_deposit_wrong_depositor_ata() {
        let mut svm = setup_svm();
        let (
            _initializer,
            token_mint,
            _initializer_ata,
            config_pda,
            stake_account_main,
            stake_account_reserve,
            _vote_pubkey,
        ) = run_initialize(&mut svm);

        let depositor = Keypair::new();
        svm.airdrop(&depositor.pubkey(), 10_000_000_000).unwrap();

        // Create an ATA owned by someone else (same mint, wrong owner)
        let other_owner = Keypair::new();
        let wrong_ata =
            create_and_fund_ata(&mut svm, &other_owner.pubkey(), &token_mint.pubkey(), 0);

        let ix = build_deposit_ix(
            &config_pda,
            &depositor.pubkey(),
            &wrong_ata,
            &token_mint.pubkey(),
            &stake_account_main,
            &stake_account_reserve,
            2_000_000_000,
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
        assert!(result.is_err(), "Should fail with wrong depositor ATA");
    }

    #[test]
    fn test_deposit_multiple_deposits() {
        let mut svm = setup_svm();
        let (
            _initializer,
            token_mint,
            _initializer_ata,
            config_pda,
            stake_account_main,
            stake_account_reserve,
            _vote_pubkey,
        ) = run_initialize(&mut svm);

        let deposit_amount = 2_000_000_000u64;

        // First deposit
        let (_depositor1, _depositor1_ata) = run_deposit(
            &mut svm,
            &config_pda,
            &token_mint.pubkey(),
            &stake_account_main,
            &stake_account_reserve,
            deposit_amount,
        );

        // Second deposit from a different user should also succeed
        let (_depositor2, _depositor2_ata) = run_deposit(
            &mut svm,
            &config_pda,
            &token_mint.pubkey(),
            &stake_account_main,
            &stake_account_reserve,
            deposit_amount,
        );

        // Third deposit from yet another user
        let (_depositor3, _depositor3_ata) = run_deposit(
            &mut svm,
            &config_pda,
            &token_mint.pubkey(),
            &stake_account_main,
            &stake_account_reserve,
            deposit_amount,
        );
    }
}
