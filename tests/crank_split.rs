mod test_helpers;

#[cfg(test)]
mod tests {
    use solana_sdk::pubkey::Pubkey;
    use solana_sdk::signature::Keypair;
    use solana_sdk::signer::Signer;
    use solana_sdk::transaction::Transaction;

    use crate::test_helpers::test_helpers::{
        build_crank_split_ix, create_and_fund_ata, print_transaction_logs,
        run_crank_initialize_reserve, run_crank_merge_reserve, run_crank_split, run_deposit,
        run_initialize, setup_svm,
    };

    /// Sets up a pool ready for crank_split: initialize + deposit + crank_init_reserve + merge.
    /// Returns (initializer, token_mint, depositor, depositor_ata, config_pda,
    ///          stake_account_main, stake_account_reserve, vote_pubkey).
    fn setup_split_ready_pool(
        svm: &mut litesvm::LiteSVM,
        deposit_amount: u64,
    ) -> (
        Keypair, // initializer
        Keypair, // token_mint
        Keypair, // depositor
        Pubkey,  // depositor_ata
        Pubkey,  // config_pda
        Pubkey,  // stake_account_main
        Pubkey,  // stake_account_reserve
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

        (
            initializer,
            token_mint,
            depositor,
            depositor_ata,
            config_pda,
            stake_account_main,
            stake_account_reserve,
            vote_pubkey,
        )
    }

    #[test]
    fn test_crank_split_success() {
        let mut svm = setup_svm();
        let (
            _initializer,
            token_mint,
            depositor,
            depositor_ata,
            config_pda,
            stake_account_main,
            stake_account_reserve,
            _vote_pubkey,
        ) = setup_split_ready_pool(&mut svm, 2_000_000_000);

        let lamports_to_split = 1_500_000_000u64;
        let _depositor_stake_account = run_crank_split(
            &mut svm,
            &depositor,
            &depositor_ata,
            &config_pda,
            &stake_account_main,
            &stake_account_reserve,
            &token_mint.pubkey(),
            lamports_to_split,
            123,
        );
    }

    #[test]
    fn test_crank_split_wrong_config_pda() {
        let mut svm = setup_svm();
        let (
            _initializer,
            token_mint,
            depositor,
            depositor_ata,
            _config_pda,
            stake_account_main,
            stake_account_reserve,
            _vote_pubkey,
        ) = setup_split_ready_pool(&mut svm, 2_000_000_000);

        let wrong_config = Pubkey::new_unique();
        let (ix, _) = build_crank_split_ix(
            &depositor.pubkey(),
            &depositor_ata,
            &wrong_config,
            &stake_account_main,
            &stake_account_reserve,
            &token_mint.pubkey(),
            1_500_000_000,
            true,
            123,
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
    fn test_crank_split_wrong_stake_account_main() {
        let mut svm = setup_svm();
        let (
            _initializer,
            token_mint,
            depositor,
            depositor_ata,
            config_pda,
            _stake_account_main,
            stake_account_reserve,
            _vote_pubkey,
        ) = setup_split_ready_pool(&mut svm, 2_000_000_000);

        let wrong_main = Pubkey::new_unique();
        let (ix, _) = build_crank_split_ix(
            &depositor.pubkey(),
            &depositor_ata,
            &config_pda,
            &wrong_main,
            &stake_account_reserve,
            &token_mint.pubkey(),
            1_500_000_000,
            true,
            123,
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
            "Should fail with wrong stake account main"
        );
    }

    #[test]
    fn test_crank_split_wrong_lst_mint() {
        let mut svm = setup_svm();
        let (
            _initializer,
            _token_mint,
            depositor,
            _depositor_ata,
            config_pda,
            stake_account_main,
            stake_account_reserve,
            _vote_pubkey,
        ) = setup_split_ready_pool(&mut svm, 2_000_000_000);

        let wrong_mint =
            crate::test_helpers::test_helpers::create_mock_token_mint(&mut svm, &config_pda);
        let wrong_ata =
            create_and_fund_ata(&mut svm, &depositor.pubkey(), &wrong_mint.pubkey(), 0);

        let (ix, _) = build_crank_split_ix(
            &depositor.pubkey(),
            &wrong_ata,
            &config_pda,
            &stake_account_main,
            &stake_account_reserve,
            &wrong_mint.pubkey(),
            1_500_000_000,
            true,
            123,
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
    fn test_crank_split_wrong_reserve_account() {
        let mut svm = setup_svm();
        let (
            _initializer,
            token_mint,
            depositor,
            depositor_ata,
            config_pda,
            stake_account_main,
            _stake_account_reserve,
            _vote_pubkey,
        ) = setup_split_ready_pool(&mut svm, 2_000_000_000);

        let wrong_reserve = Pubkey::new_unique();
        let (ix, _) = build_crank_split_ix(
            &depositor.pubkey(),
            &depositor_ata,
            &config_pda,
            &stake_account_main,
            &wrong_reserve,
            &token_mint.pubkey(),
            1_500_000_000,
            true,
            123,
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
    fn test_crank_split_double_withdrawal() {
        let mut svm = setup_svm();
        let (
            _initializer,
            token_mint,
            depositor,
            depositor_ata,
            config_pda,
            stake_account_main,
            stake_account_reserve,
            _vote_pubkey,
        ) = setup_split_ready_pool(&mut svm, 2_000_000_000);

        // First split should succeed
        let _depositor_stake_account = run_crank_split(
            &mut svm,
            &depositor,
            &depositor_ata,
            &config_pda,
            &stake_account_main,
            &stake_account_reserve,
            &token_mint.pubkey(),
            1_500_000_000,
            123,
        );

        // Second split with the same nonce should fail (PDA already created)
        let (ix, _) = build_crank_split_ix(
            &depositor.pubkey(),
            &depositor_ata,
            &config_pda,
            &stake_account_main,
            &stake_account_reserve,
            &token_mint.pubkey(),
            1_500_000_000,
            true,
            123,
        );

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&depositor.pubkey()),
            &[&depositor],
            svm.latest_blockhash(),
        );

        let result = svm.send_transaction(tx);
        print_transaction_logs(&result);
        assert!(result.is_err(), "Should fail on double withdrawal (same nonce)");
    }

    #[test]
    fn test_crank_split_more_than_available() {
        let mut svm = setup_svm();
        let (
            _initializer,
            token_mint,
            depositor,
            depositor_ata,
            config_pda,
            stake_account_main,
            stake_account_reserve,
            _vote_pubkey,
        ) = setup_split_ready_pool(&mut svm, 2_000_000_000);

        // Try to split way more than is in the main stake account
        let excessive_amount = 100_000_000_000u64;
        let (ix, _) = build_crank_split_ix(
            &depositor.pubkey(),
            &depositor_ata,
            &config_pda,
            &stake_account_main,
            &stake_account_reserve,
            &token_mint.pubkey(),
            excessive_amount,
            true,
            123,
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
            "Should fail when splitting more than available"
        );
    }

    #[test]
    fn test_crank_split_withdrawer_insufficient_lst() {
        let mut svm = setup_svm();
        let (
            _initializer,
            token_mint,
            _depositor,
            _depositor_ata,
            config_pda,
            stake_account_main,
            stake_account_reserve,
            _vote_pubkey,
        ) = setup_split_ready_pool(&mut svm, 2_000_000_000);

        // Create a new user with no LST
        let poor_user = Keypair::new();
        svm.airdrop(&poor_user.pubkey(), 10_000_000_000).unwrap();
        let poor_user_ata =
            create_and_fund_ata(&mut svm, &poor_user.pubkey(), &token_mint.pubkey(), 0);

        let (ix, _) = build_crank_split_ix(
            &poor_user.pubkey(),
            &poor_user_ata,
            &config_pda,
            &stake_account_main,
            &stake_account_reserve,
            &token_mint.pubkey(),
            1_500_000_000,
            true,
            123,
        );

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&poor_user.pubkey()),
            &[&poor_user],
            svm.latest_blockhash(),
        );

        let result = svm.send_transaction(tx);
        print_transaction_logs(&result);
        assert!(
            result.is_err(),
            "Should fail when withdrawer has insufficient LST"
        );
    }
}
