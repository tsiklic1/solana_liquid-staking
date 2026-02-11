mod test_helpers;

#[cfg(test)]
mod tests {
    use solana_program::example_mocks::solana_sdk::system_program;
    use solana_sdk::pubkey::Pubkey;
    use solana_sdk::signer::Signer;
    use solana_sdk::transaction::Transaction;

    use crate::test_helpers::test_helpers::{
        build_crank_initialize_reserve_ix, print_transaction_logs, run_crank_initialize_reserve,
        run_initialize, setup_svm,
    };

    use solana_liquid_staking::instructions::helpers::STAKE_PROGRAM_ID;

    #[test]
    fn test_crank_initialize_reserve_success() {
        let mut svm = setup_svm();
        let (initializer, _token_mint, _initializer_ata, config_pda, _stake_account_main, stake_account_reserve, vote_pubkey) =
            run_initialize(&mut svm);

        run_crank_initialize_reserve(
            &mut svm,
            &initializer,
            &config_pda,
            &stake_account_reserve,
            &vote_pubkey,
        );
    }

    #[test]
    fn test_crank_initialize_reserve_wrong_reserve_stake_account() {
        let mut svm = setup_svm();
        let (initializer, _token_mint, _initializer_ata, config_pda, _stake_account_main, _stake_account_reserve, vote_pubkey) =
            run_initialize(&mut svm);

        let wrong_reserve = Pubkey::new_unique();
        let system_program = system_program::ID;
        let stake_program = Pubkey::from(STAKE_PROGRAM_ID);

        let ix = build_crank_initialize_reserve_ix(
            &config_pda,
            &wrong_reserve,
            &vote_pubkey,
            &system_program,
            &stake_program,
        );

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&initializer.pubkey()),
            &[&initializer],
            svm.latest_blockhash(),
        );

        let result = svm.send_transaction(tx);
        print_transaction_logs(&result);
        assert!(result.is_err(), "Should fail with wrong reserve stake account");
    }

    #[test]
    fn test_crank_initialize_reserve_wrong_validator_vote_account() {
        let mut svm = setup_svm();
        let (initializer, _token_mint, _initializer_ata, config_pda, _stake_account_main, stake_account_reserve, _vote_pubkey) =
            run_initialize(&mut svm);

        let wrong_vote = Pubkey::new_unique();
        let system_program = system_program::ID;
        let stake_program = Pubkey::from(STAKE_PROGRAM_ID);

        let ix = build_crank_initialize_reserve_ix(
            &config_pda,
            &stake_account_reserve,
            &wrong_vote,
            &system_program,
            &stake_program,
        );

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&initializer.pubkey()),
            &[&initializer],
            svm.latest_blockhash(),
        );

        let result = svm.send_transaction(tx);
        print_transaction_logs(&result);
        assert!(result.is_err(), "Should fail with wrong validator vote account");
    }

    #[test]
    fn test_crank_initialize_reserve_double_invocation() {
        let mut svm = setup_svm();
        let (initializer, _token_mint, _initializer_ata, config_pda, _stake_account_main, stake_account_reserve, vote_pubkey) =
            run_initialize(&mut svm);

        // First invocation should succeed
        run_crank_initialize_reserve(
            &mut svm,
            &initializer,
            &config_pda,
            &stake_account_reserve,
            &vote_pubkey,
        );

        // Second invocation should fail
        let system_program = system_program::ID;
        let stake_program = Pubkey::from(STAKE_PROGRAM_ID);

        let ix = build_crank_initialize_reserve_ix(
            &config_pda,
            &stake_account_reserve,
            &vote_pubkey,
            &system_program,
            &stake_program,
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
    fn test_crank_initialize_reserve_empty_reserve() {
        let mut svm = setup_svm();
        let (initializer, _token_mint, _initializer_ata, config_pda, _stake_account_main, stake_account_reserve, vote_pubkey) =
            run_initialize(&mut svm);

        // Do NOT deposit anything — the reserve has no extra SOL beyond what
        // initialize left (which should be below the 1 SOL + rent threshold
        // needed for stake delegation).
        // The happy-path test works because initialize already funds the
        // reserve with enough lamports. We need to drain it.
        // Actually, let's check: after initialize the reserve may already have
        // enough. Instead, we test by calling crank on a reserve that has
        // insufficient balance. We'll create a fresh pool where the reserve
        // has minimal lamports by not depositing.
        //
        // Looking at the instruction: it calls initialize_stake_account then
        // delegate. The stake account needs at least rent + 1 SOL for delegation.
        // After initialize, the reserve PDA is created with system account
        // lamports. If we drain it to below the threshold, it should fail.

        // Drain the reserve to leave very little
        // We can't easily drain a PDA, so instead let's just attempt the crank
        // without a deposit and see if it fails. If the initialize instruction
        // funds the reserve adequately, we need a different approach.

        // Actually looking at the happy path: run_initialize creates the pool,
        // and the test immediately calls crank_initialize_reserve without any
        // deposit. So the reserve must already have enough from initialize.
        // For the "empty reserve" test, I need to set up the state such that
        // the reserve has less than required.
        // Let me set the reserve account balance directly to a small amount.

        let reserve_account = svm.get_account(&stake_account_reserve).unwrap();
        let mut drained = reserve_account.clone();
        drained.lamports = 100_000; // well below 1 SOL + rent for stake
        svm.set_account(stake_account_reserve, drained.into()).unwrap();

        let system_program = system_program::ID;
        let stake_program = Pubkey::from(STAKE_PROGRAM_ID);

        let ix = build_crank_initialize_reserve_ix(
            &config_pda,
            &stake_account_reserve,
            &vote_pubkey,
            &system_program,
            &stake_program,
        );

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&initializer.pubkey()),
            &[&initializer],
            svm.latest_blockhash(),
        );

        let result = svm.send_transaction(tx);
        print_transaction_logs(&result);
        assert!(result.is_err(), "Should fail with empty/underfunded reserve");
    }

    #[test]
    fn test_crank_initialize_reserve_wrong_system_program() {
        let mut svm = setup_svm();
        let (initializer, _token_mint, _initializer_ata, config_pda, _stake_account_main, stake_account_reserve, vote_pubkey) =
            run_initialize(&mut svm);

        let wrong_system_program = Pubkey::new_unique();
        let stake_program = Pubkey::from(STAKE_PROGRAM_ID);

        let ix = build_crank_initialize_reserve_ix(
            &config_pda,
            &stake_account_reserve,
            &vote_pubkey,
            &wrong_system_program,
            &stake_program,
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
    fn test_crank_initialize_reserve_wrong_stake_program() {
        let mut svm = setup_svm();
        let (initializer, _token_mint, _initializer_ata, config_pda, _stake_account_main, stake_account_reserve, vote_pubkey) =
            run_initialize(&mut svm);

        let system_program = system_program::ID;
        let wrong_stake_program = Pubkey::new_unique();

        let ix = build_crank_initialize_reserve_ix(
            &config_pda,
            &stake_account_reserve,
            &vote_pubkey,
            &system_program,
            &wrong_stake_program,
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
}
