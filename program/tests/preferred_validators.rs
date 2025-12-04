#![allow(clippy::arithmetic_side_effects)]
#![cfg(feature = "test-sbf")]

mod helpers;

use {
    helpers::*,
    solana_program::borsh1::try_from_slice_unchecked,
    solana_program_test::*,
    solana_sdk::{
        pubkey::Pubkey,
        signature::{Keypair, Signer},
        transaction::Transaction,
    },
    spl_stake_pool::{
        instruction::{self, PreferredValidatorType},
        state::StakeStatus,
        MINIMUM_RESERVE_LAMPORTS,
    },
};

/// Test that verifies preferred validators are properly reset when a validator is removed
/// from the stake pool through withdrawal. This test:
/// 1. Initializes a stake pool with zero fees
/// 2. Adds two validators to the pool
/// 3. Sets one validator as the preferred withdrawal validator
/// 4. Performs a withdrawal that removes the entire stake from the preferred validator
/// 5. Verifies that the preferred withdrawal validator is reset to None after the withdrawal
///
/// This ensures that when a validator is completely removed from the pool (via ValidatorRemoval
/// withdrawal source), any preferred validator settings for that validator are automatically cleared.
#[tokio::test]
async fn test_preferred_validator_removal() {
    let mut context = program_test().start_with_context().await;
    let mut stake_pool_accounts = StakePoolAccounts::default();

    // Set all fees to zero for simplicity
    let zero_fee = spl_stake_pool::state::Fee {
        denominator: 0,
        numerator: 0,
    };
    stake_pool_accounts.withdrawal_fee = zero_fee;
    stake_pool_accounts.deposit_fee = zero_fee;
    stake_pool_accounts.sol_deposit_fee = zero_fee;
    stake_pool_accounts.epoch_fee = zero_fee;

    // Step 1: Initialize stake pool with reserve
    let initial_reserve_lamports = TEST_STAKE_AMOUNT + MINIMUM_RESERVE_LAMPORTS;
    stake_pool_accounts
        .initialize_stake_pool(
            &mut context.banks_client,
            &context.payer,
            &context.last_blockhash,
            initial_reserve_lamports,
        )
        .await
        .unwrap();

    // Step 2: Add a validator to the pool
    let validator_stake_account = simple_add_validator_to_pool(
        &mut context.banks_client,
        &context.payer,
        &context.last_blockhash,
        &stake_pool_accounts,
        None,
    )
    .await;

    let _validator_stake_account_2 = simple_add_validator_to_pool(
        &mut context.banks_client,
        &context.payer,
        &context.last_blockhash,
        &stake_pool_accounts,
        None,
    )
    .await;

    stake_pool_accounts
        .set_preferred_validator(
            &mut context.banks_client,
            &context.payer,
            &context.last_blockhash,
            PreferredValidatorType::Withdraw,
            Some(validator_stake_account.vote.pubkey()),
        )
        .await;

    let current_slot = context.banks_client.get_root_slot().await.unwrap();
    context.warp_to_slot(current_slot + 5).unwrap();

    stake_pool_accounts
        .update_all(
            &mut context.banks_client,
            &context.payer,
            &context.last_blockhash,
            false,
        )
        .await;

    let stake_pool = stake_pool_accounts
        .get_stake_pool(&mut context.banks_client)
        .await;
    let total_lamports = stake_pool.total_lamports;

    println!("Total lamports after increase: {}", total_lamports);

    // Get updated validator stake account balance
    let validator_stake_info_after = get_account(
        &mut context.banks_client,
        &validator_stake_account.stake_account,
    )
    .await;
    let validator_balance = validator_stake_info_after.lamports;

    // Calculate withdrawal that would trigger ValidatorRemoval
    let withdrawal_amount = validator_balance;

    // Calculate pool tokens needed for this withdrawal, rounding UP to ensure we get at least this amount
    let lamports_per_pool_token = stake_pool.get_lamports_per_pool_token().unwrap();
    let pool_tokens_to_burn =
        ((withdrawal_amount as f64 / lamports_per_pool_token as f64).ceil()) as u64;
    println!("pool_tokens_to_burn: {}", pool_tokens_to_burn);
    println!("withdrawal_amount: {}", withdrawal_amount);
    println!("lamports_per_pool_token: {}", lamports_per_pool_token);

    println!(
        "Attempting to withdraw {} lamports using {} pool tokens",
        withdrawal_amount, pool_tokens_to_burn
    );
    println!("Lamports per pool token: {}", lamports_per_pool_token);

    // Debug: Check validator list state
    let validator_list_before_withdrawal = stake_pool_accounts
        .get_validator_list(&mut context.banks_client)
        .await;
    let validator_entry_before = validator_list_before_withdrawal
        .validators
        .iter()
        .find(|v| v.vote_account_address == validator_stake_account.vote.pubkey())
        .expect("Validator should be in list");
    println!(
        "Before withdrawal - Active: {}, Transient: {}",
        u64::from(validator_entry_before.active_stake_lamports),
        u64::from(validator_entry_before.transient_stake_lamports)
    );

    // Create pool token account for withdrawal
    let pool_token_account = Keypair::new();
    create_token_account(
        &mut context.banks_client,
        &context.payer,
        &context.last_blockhash,
        &stake_pool_accounts.token_program_id,
        &pool_token_account,
        &stake_pool_accounts.pool_mint.pubkey(),
        &context.payer,
        &[],
    )
    .await
    .unwrap();

    // Get pool tokens by depositing SOL - deposit extra to ensure we have enough
    let sol_to_deposit = 10_000_000_000;
    println!(
        "Depositing {} SOL to get enough pool tokens",
        sol_to_deposit
    );
    let error = stake_pool_accounts
        .deposit_sol(
            &mut context.banks_client,
            &context.payer,
            &context.last_blockhash,
            &pool_token_account.pubkey(),
            sol_to_deposit,
            None,
        )
        .await;
    assert!(
        error.is_none(),
        "Failed to deposit SOL for pool tokens: {:?}",
        error
    );

    // Create destination stake account
    let user_stake_recipient = Keypair::new();
    create_blank_stake_account(
        &mut context.banks_client,
        &context.payer,
        &context.last_blockhash,
        &user_stake_recipient,
    )
    .await;

    // Perform withdrawal - this should trigger ValidatorRemoval
    let error = stake_pool_accounts
        .withdraw_stake(
            &mut context.banks_client,
            &context.payer,
            &context.last_blockhash,
            &user_stake_recipient.pubkey(),
            &context.payer,
            &pool_token_account.pubkey(),
            &validator_stake_account.stake_account,
            &context.payer.pubkey(), // recipient_new_authority
            pool_tokens_to_burn,
        )
        .await;

    // This should succeed and trigger ValidatorRemoval
    assert!(error.is_none(), "Withdrawal failed: {:?}", error);

    // Check that the preferred withdraw validator is set to None at this point
    let stake_pool_after_withdrawal = stake_pool_accounts
        .get_stake_pool(&mut context.banks_client)
        .await;
    assert_eq!(
        stake_pool_after_withdrawal.preferred_withdraw_validator_vote_address, None,
        "Preferred withdraw validator should be None after withdrawal"
    );
}

/// Test that verifies preferred validators are reset when they reference validators that don't exist
/// in the validator list. This test:
/// 1. Initializes a stake pool
/// 2. Directly modifies the stake pool state to set a fake validator pubkey as preferred
/// 3. Calls the cleanup instruction to remove invalid validator entries
/// 4. Verifies that both preferred deposit and withdrawal validators are reset to None
///
/// This ensures that the cleanup process properly handles cases where preferred validators
/// reference non-existent validators, preventing invalid state.
#[tokio::test]
async fn test_preferred_validator_reset_on_cleanup() {
    let mut context = program_test().start_with_context().await;
    let stake_pool_accounts = StakePoolAccounts::default();

    // Initialize stake pool with reserve
    let initial_reserve_lamports = TEST_STAKE_AMOUNT + MINIMUM_RESERVE_LAMPORTS;
    stake_pool_accounts
        .initialize_stake_pool(
            &mut context.banks_client,
            &context.payer,
            &context.last_blockhash,
            initial_reserve_lamports,
        )
        .await
        .unwrap();

    // Create a fake validator pubkey that's not in the pool
    let fake_validator_pubkey = Pubkey::new_unique();

    // Now simulate setting the fake validator as preferred by directly writing to the stake pool account
    let mut stake_pool_data = context
        .banks_client
        .get_account(stake_pool_accounts.stake_pool.pubkey())
        .await
        .unwrap()
        .unwrap()
        .data;

    let mut stake_pool =
        try_from_slice_unchecked::<spl_stake_pool::state::StakePool>(&stake_pool_data).unwrap();

    // Set the fake validator as preferred (this shouldn't happen in normal flow)
    stake_pool.preferred_deposit_validator_vote_address = Some(fake_validator_pubkey);
    stake_pool.preferred_withdraw_validator_vote_address = Some(fake_validator_pubkey);

    // Write the modified stake pool back using the context's account modification
    let serialized_stake_pool = borsh::to_vec(&stake_pool).unwrap();
    stake_pool_data[..serialized_stake_pool.len()].copy_from_slice(&serialized_stake_pool);

    // Use the context to set the account data
    let account = solana_sdk::account::AccountSharedData::from(solana_sdk::account::Account {
        lamports: stake_pool_data.len() as u64,
        data: stake_pool_data,
        owner: spl_stake_pool::id(),
        executable: false,
        rent_epoch: 0,
    });
    context.set_account(&stake_pool_accounts.stake_pool.pubkey(), &account);

    // Verify the preferred validators are set to the fake validator
    let stake_pool_after_direct_write = stake_pool_accounts
        .get_stake_pool(&mut context.banks_client)
        .await;
    assert_eq!(
        stake_pool_after_direct_write.preferred_deposit_validator_vote_address,
        Some(fake_validator_pubkey)
    );
    assert_eq!(
        stake_pool_after_direct_write.preferred_withdraw_validator_vote_address,
        Some(fake_validator_pubkey)
    );

    // Call cleanup - this should reset the preferred validators since the validator doesn't exist in the list
    let cleanup_instruction = instruction::cleanup_removed_validator_entries(
        &spl_stake_pool::id(),
        &stake_pool_accounts.stake_pool.pubkey(),
        &stake_pool_accounts.validator_list.pubkey(),
    );

    let transaction = Transaction::new_signed_with_payer(
        &[cleanup_instruction],
        Some(&context.payer.pubkey()),
        &[&context.payer],
        context.last_blockhash,
    );

    context
        .banks_client
        .process_transaction(transaction)
        .await
        .unwrap();

    // Verify that the preferred validators were reset
    let stake_pool_after_cleanup = stake_pool_accounts
        .get_stake_pool(&mut context.banks_client)
        .await;
    assert_eq!(
        stake_pool_after_cleanup.preferred_deposit_validator_vote_address, None,
        "Preferred deposit validator should be reset"
    );
    assert_eq!(
        stake_pool_after_cleanup.preferred_withdraw_validator_vote_address, None,
        "Preferred withdrawal validator should be reset"
    );
}

/// Test that verifies preferred validators are reset when they reference validators that exist
/// in the validator list but are in an inactive state (ReadyForRemoval). This test:
/// 1. Initializes a stake pool and adds a validator
/// 2. Removes the validator by withdrawing all its stake (puts it in ReadyForRemoval state)
/// 3. Directly modifies the stake pool state to set the inactive validator as preferred
/// 4. Calls the cleanup instruction to remove inactive validator entries
/// 5. Verifies that both preferred deposit and withdrawal validators are reset to None
///
/// This ensures that the cleanup process properly handles cases where preferred validators
/// reference validators that are no longer active, maintaining pool integrity.
#[tokio::test]
async fn test_preferred_validator_reset_on_cleanup_inactive_validator() {
    let mut context = program_test().start_with_context().await;
    let mut stake_pool_accounts = StakePoolAccounts::default();

    // Set all fees to zero for simplicity
    let zero_fee = spl_stake_pool::state::Fee {
        denominator: 0,
        numerator: 0,
    };
    stake_pool_accounts.withdrawal_fee = zero_fee;
    stake_pool_accounts.deposit_fee = zero_fee;
    stake_pool_accounts.sol_deposit_fee = zero_fee;
    stake_pool_accounts.epoch_fee = zero_fee;

    // Step 1: Initialize stake pool with reserve
    let initial_reserve_lamports = TEST_STAKE_AMOUNT + MINIMUM_RESERVE_LAMPORTS;
    stake_pool_accounts
        .initialize_stake_pool(
            &mut context.banks_client,
            &context.payer,
            &context.last_blockhash,
            initial_reserve_lamports,
        )
        .await
        .unwrap();

    // Step 2: Add a validator to the pool
    let validator_stake_account = simple_add_validator_to_pool(
        &mut context.banks_client,
        &context.payer,
        &context.last_blockhash,
        &stake_pool_accounts,
        None,
    )
    .await;

    stake_pool_accounts
        .set_preferred_validator(
            &mut context.banks_client,
            &context.payer,
            &context.last_blockhash,
            PreferredValidatorType::Withdraw,
            Some(validator_stake_account.vote.pubkey()),
        )
        .await;

    let current_slot = context.banks_client.get_root_slot().await.unwrap();
    context.warp_to_slot(current_slot + 5).unwrap();

    stake_pool_accounts
        .update_all(
            &mut context.banks_client,
            &context.payer,
            &context.last_blockhash,
            false,
        )
        .await;

    let stake_pool = stake_pool_accounts
        .get_stake_pool(&mut context.banks_client)
        .await;

    // Get updated validator stake account balance
    let validator_stake_info_after = get_account(
        &mut context.banks_client,
        &validator_stake_account.stake_account,
    )
    .await;
    let validator_balance = validator_stake_info_after.lamports;

    // Calculate withdrawal that would trigger ValidatorRemoval
    let withdrawal_amount = validator_balance;

    // Calculate pool tokens needed for this withdrawal, rounding UP to ensure we get at least this amount
    let lamports_per_pool_token = stake_pool.get_lamports_per_pool_token().unwrap();
    let pool_tokens_to_burn =
        ((withdrawal_amount as f64 / lamports_per_pool_token as f64).ceil()) as u64;

    // Create pool token account for withdrawal
    let pool_token_account = Keypair::new();
    create_token_account(
        &mut context.banks_client,
        &context.payer,
        &context.last_blockhash,
        &stake_pool_accounts.token_program_id,
        &pool_token_account,
        &stake_pool_accounts.pool_mint.pubkey(),
        &context.payer,
        &[],
    )
    .await
    .unwrap();

    // Get pool tokens by depositing SOL - deposit extra to ensure we have enough
    let sol_to_deposit = 10_000_000_000;
    let error = stake_pool_accounts
        .deposit_sol(
            &mut context.banks_client,
            &context.payer,
            &context.last_blockhash,
            &pool_token_account.pubkey(),
            sol_to_deposit,
            None,
        )
        .await;
    assert!(
        error.is_none(),
        "Failed to deposit SOL for pool tokens: {:?}",
        error
    );

    // Create destination stake account
    let user_stake_recipient = Keypair::new();
    create_blank_stake_account(
        &mut context.banks_client,
        &context.payer,
        &context.last_blockhash,
        &user_stake_recipient,
    )
    .await;

    // Perform withdrawal - this should trigger ValidatorRemoval
    let error = stake_pool_accounts
        .withdraw_stake(
            &mut context.banks_client,
            &context.payer,
            &context.last_blockhash,
            &user_stake_recipient.pubkey(),
            &context.payer,
            &pool_token_account.pubkey(),
            &validator_stake_account.stake_account,
            &context.payer.pubkey(), // recipient_new_authority
            pool_tokens_to_burn,
        )
        .await;

    // This should succeed and trigger ValidatorRemoval
    assert!(error.is_none(), "Withdrawal failed: {:?}", error);

    // Check that the preferred withdraw validator is set to None at this point
    let stake_pool_after_withdrawal = stake_pool_accounts
        .get_stake_pool(&mut context.banks_client)
        .await;
    assert_eq!(
        stake_pool_after_withdrawal.preferred_withdraw_validator_vote_address, None,
        "Preferred withdraw validator should be None after withdrawal"
    );

    // Verify the validator is marked as ReadyForRemoval
    let validator_list = stake_pool_accounts
        .get_validator_list(&mut context.banks_client)
        .await;
    let validator_entry = validator_list
        .validators
        .iter()
        .find(|v| v.vote_account_address == validator_stake_account.vote.pubkey())
        .expect("Validator should be in list");
    assert_eq!(validator_entry.status, StakeStatus::ReadyForRemoval.into());

    // Now simulate setting the inactive validator as preferred by directly writing to the stake pool account
    let mut stake_pool_data = context
        .banks_client
        .get_account(stake_pool_accounts.stake_pool.pubkey())
        .await
        .unwrap()
        .unwrap()
        .data;

    let mut stake_pool =
        try_from_slice_unchecked::<spl_stake_pool::state::StakePool>(&stake_pool_data).unwrap();

    // Set the inactive validator as preferred (this shouldn't happen in normal flow)
    stake_pool.preferred_deposit_validator_vote_address =
        Some(validator_stake_account.vote.pubkey());
    stake_pool.preferred_withdraw_validator_vote_address =
        Some(validator_stake_account.vote.pubkey());

    // Write the modified stake pool back using the context's account modification
    let serialized_stake_pool = borsh::to_vec(&stake_pool).unwrap();
    stake_pool_data[..serialized_stake_pool.len()].copy_from_slice(&serialized_stake_pool);

    // Use the context to set the account data
    let account = solana_sdk::account::AccountSharedData::from(solana_sdk::account::Account {
        lamports: stake_pool_data.len() as u64,
        data: stake_pool_data,
        owner: spl_stake_pool::id(),
        executable: false,
        rent_epoch: 0,
    });
    context.set_account(&stake_pool_accounts.stake_pool.pubkey(), &account);

    // Verify the preferred validators are set to the inactive validator
    let stake_pool_after_direct_write = stake_pool_accounts
        .get_stake_pool(&mut context.banks_client)
        .await;
    assert_eq!(
        stake_pool_after_direct_write.preferred_deposit_validator_vote_address,
        Some(validator_stake_account.vote.pubkey())
    );
    assert_eq!(
        stake_pool_after_direct_write.preferred_withdraw_validator_vote_address,
        Some(validator_stake_account.vote.pubkey())
    );

    // Call cleanup - this should reset the preferred validators since the validator is not active
    let cleanup_instruction = instruction::cleanup_removed_validator_entries(
        &spl_stake_pool::id(),
        &stake_pool_accounts.stake_pool.pubkey(),
        &stake_pool_accounts.validator_list.pubkey(),
    );

    let transaction = Transaction::new_signed_with_payer(
        &[cleanup_instruction],
        Some(&context.payer.pubkey()),
        &[&context.payer],
        context.last_blockhash,
    );

    context
        .banks_client
        .process_transaction(transaction)
        .await
        .unwrap();

    // Verify that the preferred validators were reset
    let stake_pool_after_cleanup = stake_pool_accounts
        .get_stake_pool(&mut context.banks_client)
        .await;
    assert_eq!(
        stake_pool_after_cleanup.preferred_deposit_validator_vote_address, None,
        "Preferred deposit validator should be reset"
    );
    assert_eq!(
        stake_pool_after_cleanup.preferred_withdraw_validator_vote_address, None,
        "Preferred withdrawal validator should be reset"
    );
}
