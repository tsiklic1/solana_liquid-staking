mod test_helpers;

#[cfg(test)]
mod tests {
    use solana_program::example_mocks::solana_sdk::system_program;
    use solana_sdk::pubkey::Pubkey;
    use solana_sdk::signer::Signer;
    use solana_sdk::transaction::Transaction;

    use crate::test_helpers::test_helpers::{
        build_crank_merge_reserve_ix, print_transaction_logs, run_crank_initialize_reserve,
        run_crank_merge_reserve, run_initialize, setup_svm,
    };

    use solana_liquid_staking::instructions::helpers::STAKE_PROGRAM_ID;

    /// Helper: runs initialize + crank_initialize_reserve to get the pool into
    /// a state where crank_merge_reserve can be attempted.
    fn setup_merge_ready_pool(
        svm: &mut litesvm::LiteSVM,
    ) -> (
        solana_sdk::signature::Keypair,
        Pubkey,
        Pubkey,
        Pubkey,
        Pubkey,
    ) {
        let (initializer, _token_mint, _initializer_ata, config_pda, stake_account_main, stake_account_reserve, vote_pubkey) =
            run_initialize(svm);

        run_crank_initialize_reserve(
            svm,
            &initializer,
            &config_pda,
            &stake_account_reserve,
            &vote_pubkey,
        );

        (
            initializer,
            config_pda,
            stake_account_main,
            stake_account_reserve,
            vote_pubkey,
        )
    }

    #[test]
    fn test_crank_merge_reserve_success() {
        let mut svm = setup_svm();
        let (initializer, config_pda, stake_account_main, stake_account_reserve, _vote_pubkey) =
            setup_merge_ready_pool(&mut svm);

        run_crank_merge_reserve(
            &mut svm,
            &initializer,
            &config_pda,
            &stake_account_main,
            &stake_account_reserve,
        );
    }

    #[test]
    fn test_crank_merge_reserve_wrong_main_stake_account() {
        let mut svm = setup_svm();
        let (initializer, config_pda, _stake_account_main, stake_account_reserve, _vote_pubkey) =
            setup_merge_ready_pool(&mut svm);

        let wrong_main = Pubkey::new_unique();
        let ix = build_crank_merge_reserve_ix(
            &config_pda,
            &wrong_main,
            &stake_account_reserve,
            &system_program::ID,
            &Pubkey::from(STAKE_PROGRAM_ID),
        );

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&initializer.pubkey()),
            &[&initializer],
            svm.latest_blockhash(),
        );

        let result = svm.send_transaction(tx);
        print_transaction_logs(&result);
        assert!(result.is_err(), "Should fail with wrong main stake account");
    }

    #[test]
    fn test_crank_merge_reserve_double_invocation() {
        let mut svm = setup_svm();
        let (initializer, config_pda, stake_account_main, stake_account_reserve, _vote_pubkey) =
            setup_merge_ready_pool(&mut svm);

        // First invocation should succeed
        run_crank_merge_reserve(
            &mut svm,
            &initializer,
            &config_pda,
            &stake_account_main,
            &stake_account_reserve,
        );

        // Second invocation should fail (reserve stake state is no longer 2)
        let ix = build_crank_merge_reserve_ix(
            &config_pda,
            &stake_account_main,
            &stake_account_reserve,
            &system_program::ID,
            &Pubkey::from(STAKE_PROGRAM_ID),
        );

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&initializer.pubkey()),
            &[&initializer],
            svm.latest_blockhash(),
        );

        let result = svm.send_transaction(tx);
        print_transaction_logs(&result);
        assert!(result.is_err(), "Should fail on double invocation");
    }

    #[test]
    fn test_crank_merge_reserve_wrong_system_program() {
        let mut svm = setup_svm();
        let (initializer, config_pda, stake_account_main, stake_account_reserve, _vote_pubkey) =
            setup_merge_ready_pool(&mut svm);

        let wrong_system = Pubkey::new_unique();
        let ix = build_crank_merge_reserve_ix(
            &config_pda,
            &stake_account_main,
            &stake_account_reserve,
            &wrong_system,
            &Pubkey::from(STAKE_PROGRAM_ID),
        );

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&initializer.pubkey()),
            &[&initializer],
            svm.latest_blockhash(),
        );

        let result = svm.send_transaction(tx);
        print_transaction_logs(&result);
        assert!(result.is_err(), "Should fail with wrong system program");
    }

    #[test]
    fn test_crank_merge_reserve_wrong_stake_program() {
        let mut svm = setup_svm();
        let (initializer, config_pda, stake_account_main, stake_account_reserve, _vote_pubkey) =
            setup_merge_ready_pool(&mut svm);

        let wrong_stake = Pubkey::new_unique();
        let ix = build_crank_merge_reserve_ix(
            &config_pda,
            &stake_account_main,
            &stake_account_reserve,
            &system_program::ID,
            &wrong_stake,
        );

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&initializer.pubkey()),
            &[&initializer],
            svm.latest_blockhash(),
        );

        let result = svm.send_transaction(tx);
        print_transaction_logs(&result);
        assert!(result.is_err(), "Should fail with wrong stake program");
    }

    #[test]
    fn test_crank_merge_reserve_wrong_config_pda() {
        let mut svm = setup_svm();
        let (initializer, _config_pda, stake_account_main, stake_account_reserve, _vote_pubkey) =
            setup_merge_ready_pool(&mut svm);

        let wrong_config = Pubkey::new_unique();
        let ix = build_crank_merge_reserve_ix(
            &wrong_config,
            &stake_account_main,
            &stake_account_reserve,
            &system_program::ID,
            &Pubkey::from(STAKE_PROGRAM_ID),
        );

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&initializer.pubkey()),
            &[&initializer],
            svm.latest_blockhash(),
        );

        let result = svm.send_transaction(tx);
        print_transaction_logs(&result);
        assert!(result.is_err(), "Should fail with wrong config PDA");
    }
}
