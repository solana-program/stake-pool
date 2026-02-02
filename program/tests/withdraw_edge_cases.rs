#![allow(clippy::arithmetic_side_effects)]
#![allow(clippy::items_after_test_module)]
mod helpers;

use {
    bincode::deserialize,
    helpers::*,
    solana_program::{
        borsh1::try_from_slice_unchecked, instruction::InstructionError, pubkey::Pubkey,
    },
    solana_program_test::*,
    solana_sdk::{
        signature::{keypair::Keypair, Signer},
        transaction::TransactionError,
    },
    solana_stake_interface as stake,
    spl_stake_pool::{error::StakePoolError, instruction, state, MINIMUM_RESERVE_LAMPORTS},
    test_case::test_case,
};

#[tokio::test]
async fn fail_remove_validator_blocked_by_transient_stake() {
    let (
        mut context,
        stake_pool_accounts,
        validator_stake,
        deposit_info,
        user_transfer_authority,
        user_stake_recipient,
        _,
    ) = setup_for_withdraw(spl_token::id(), STAKE_ACCOUNT_RENT_EXEMPTION).await;

    // Step 1: Create transient stake by decreasing some validator stake
    let decrease_amount = deposit_info.stake_lamports / 3; // Decrease 1/3 of the stake
    let error = stake_pool_accounts
        .decrease_validator_stake_either(
            &mut context.banks_client,
            &context.payer,
            &context.last_blockhash,
            &validator_stake.stake_account,
            &validator_stake.transient_stake_account,
            decrease_amount,
            validator_stake.transient_stake_seed,
            DecreaseInstruction::Additional, // This creates transient stake that won't merge back
        )
        .await;
    assert!(error.is_none(), "{:?}", error);

    // Step 2: Check the state after creating transient stake
    let _validator_stake_account_after_decrease =
        get_account(&mut context.banks_client, &validator_stake.stake_account).await;
    let _transient_stake_account_after_decrease = get_account(
        &mut context.banks_client,
        &validator_stake.transient_stake_account,
    )
    .await;

    // Validator stake after decrease: validator_stake_account_after_decrease.lamports
    // Transient stake after decrease: transient_stake_account_after_decrease.lamports

    // Step 3: Warp forward to deactivation epoch
    let first_normal_slot = context.genesis_config().epoch_schedule.first_normal_slot;
    let slot = first_normal_slot + 1;
    context.warp_to_slot(slot).unwrap();

    // Step 4: Update with no_merge=true to keep transient stake separate
    let error = stake_pool_accounts
        .update_all(
            &mut context.banks_client,
            &context.payer,
            &context.last_blockhash,
            true, // no_merge = true to prevent merging transient stake back
        )
        .await;
    assert!(error.is_none(), "{:?}", error);

    // Step 5: Check final state - validator should have active stake, transient should remain separate
    let validator_stake_account_final =
        get_account(&mut context.banks_client, &validator_stake.stake_account).await;
    let transient_stake_account_final = context
        .banks_client
        .get_account(validator_stake.transient_stake_account)
        .await
        .unwrap();

    // Validator stake: validator_stake_account_final.lamports

    // Verify transient stake still exists
    let _transient_account =
        transient_stake_account_final.expect("Transient stake account should still exist");
    // Transient stake: _transient_account.lamports

    // Step 6: Try to withdraw ALL active stake - this should FAIL because transient stake blocks removal
    let rent = context.banks_client.get_rent().await.unwrap();
    let stake_rent = rent.minimum_balance(std::mem::size_of::<stake::state::StakeStateV2>());

    let stake_pool = stake_pool_accounts
        .get_stake_pool(&mut context.banks_client)
        .await;

    // Try to withdraw everything except rent (this should trigger the validator removal check)
    let active_stake_to_withdraw = validator_stake_account_final
        .lamports
        .saturating_sub(stake_rent);

    // Testing complete withdrawal of active stake (should fail due to transient stake)

    let pool_tokens_all = (active_stake_to_withdraw * stake_pool.pool_token_supply)
        .checked_div(stake_pool.total_lamports)
        .unwrap();
    let pool_tokens_all_with_fee =
        stake_pool_accounts.calculate_inverse_withdrawal_fee(pool_tokens_all);

    let new_user_authority_all = Pubkey::new_unique();
    let error_all = stake_pool_accounts
        .withdraw_stake(
            &mut context.banks_client,
            &context.payer,
            &context.last_blockhash,
            &user_stake_recipient.pubkey(),
            &user_transfer_authority,
            &deposit_info.pool_account.pubkey(),
            &validator_stake.stake_account, // Withdraw from active stake account
            &new_user_authority_all,
            pool_tokens_all_with_fee,
        )
        .await;

    // Step 7: Verify that the complete withdrawal fails because transient stake prevents validator removal
    assert!(
        error_all.is_some(),
        "Complete withdrawal should fail because validator has transient stake"
    );
    let transaction_error = error_all.unwrap().unwrap();

    // Complete withdrawal correctly failed with expected error
    // The error should be StakeLamportsNotEqualToMinimum because the program detects
    // that this validator cannot be removed due to associated transient lamports
    assert_eq!(
        transaction_error,
        TransactionError::InstructionError(
            0,
            InstructionError::Custom(StakePoolError::StakeLamportsNotEqualToMinimum as u32)
        )
    );

    // Step 8: Verify that the transient stake is still there and blocking removal
    let final_transient_check = context
        .banks_client
        .get_account(validator_stake.transient_stake_account)
        .await
        .unwrap();

    assert!(
        final_transient_check.is_some(),
        "Transient stake account should still exist"
    );
    let final_transient = final_transient_check.unwrap();
    assert!(
        final_transient.lamports > 0,
        "Transient stake should still have lamports"
    );
}

#[tokio::test]
async fn fail_remove_validator() {
    let (
        mut context,
        stake_pool_accounts,
        validator_stake,
        deposit_info,
        user_transfer_authority,
        user_stake_recipient,
        _,
    ) = setup_for_withdraw(spl_token::id(), STAKE_ACCOUNT_RENT_EXEMPTION).await;

    // decrease a little stake, not all
    let error = stake_pool_accounts
        .decrease_validator_stake_either(
            &mut context.banks_client,
            &context.payer,
            &context.last_blockhash,
            &validator_stake.stake_account,
            &validator_stake.transient_stake_account,
            deposit_info.stake_lamports / 2,
            validator_stake.transient_stake_seed,
            DecreaseInstruction::Reserve,
        )
        .await;
    assert!(error.is_none(), "{:?}", error);

    // warp forward to deactivation
    let first_normal_slot = context.genesis_config().epoch_schedule.first_normal_slot;
    context.warp_to_slot(first_normal_slot + 1).unwrap();

    // update to merge deactivated stake into reserve
    let error = stake_pool_accounts
        .update_all(
            &mut context.banks_client,
            &context.payer,
            &context.last_blockhash,
            false,
        )
        .await;
    assert!(error.is_none(), "{:?}", error);

    // Withdraw entire account, fail because some stake left
    let validator_stake_account =
        get_account(&mut context.banks_client, &validator_stake.stake_account).await;
    let remaining_lamports = validator_stake_account.lamports;
    let new_user_authority = Pubkey::new_unique();
    let error = stake_pool_accounts
        .withdraw_stake(
            &mut context.banks_client,
            &context.payer,
            &context.last_blockhash,
            &user_stake_recipient.pubkey(),
            &user_transfer_authority,
            &deposit_info.pool_account.pubkey(),
            &validator_stake.stake_account,
            &new_user_authority,
            remaining_lamports,
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        error,
        TransactionError::InstructionError(
            0,
            InstructionError::Custom(StakePoolError::StakeLamportsNotEqualToMinimum as u32)
        )
    );
}

#[test_case(0; "equal")]
#[test_case(5; "big")]
#[test_case(11; "bigger")]
#[test_case(29; "biggest")]
#[tokio::test]
async fn success_remove_validator(multiple: u64) {
    let (
        mut context,
        stake_pool_accounts,
        validator_stake,
        deposit_info,
        user_transfer_authority,
        user_stake_recipient,
        _,
    ) = setup_for_withdraw(spl_token::id(), STAKE_ACCOUNT_RENT_EXEMPTION).await;

    // make pool tokens very valuable, so it isn't possible to exactly get down to
    // the minimum
    transfer(
        &mut context.banks_client,
        &context.payer,
        &context.last_blockhash,
        &stake_pool_accounts.reserve_stake.pubkey(),
        deposit_info.stake_lamports * multiple, // each pool token is worth more than one lamport
    )
    .await;

    // warp forward to after reward payout
    let first_normal_slot = context.genesis_config().epoch_schedule.first_normal_slot;
    let mut slot = first_normal_slot + 1;
    context.warp_to_slot(slot).unwrap();

    let error = stake_pool_accounts
        .update_all(
            &mut context.banks_client,
            &context.payer,
            &context.last_blockhash,
            false,
        )
        .await;
    assert!(error.is_none(), "{:?}", error);

    let rent = context.banks_client.get_rent().await.unwrap();
    let stake_rent = rent.minimum_balance(std::mem::size_of::<stake::state::StakeStateV2>());
    let stake_pool = stake_pool_accounts
        .get_stake_pool(&mut context.banks_client)
        .await;
    let lamports_per_pool_token = stake_pool.get_lamports_per_pool_token().unwrap();

    // decrease all of stake except for lamports_per_pool_token lamports, must be
    // withdrawable
    let error = stake_pool_accounts
        .decrease_validator_stake_either(
            &mut context.banks_client,
            &context.payer,
            &context.last_blockhash,
            &validator_stake.stake_account,
            &validator_stake.transient_stake_account,
            deposit_info.stake_lamports + stake_rent - lamports_per_pool_token,
            validator_stake.transient_stake_seed,
            DecreaseInstruction::Reserve,
        )
        .await;
    assert!(error.is_none(), "{:?}", error);

    // warp forward to deactivation
    slot += context.genesis_config().epoch_schedule.slots_per_epoch;
    context.warp_to_slot(slot).unwrap();

    let last_blockhash = context
        .banks_client
        .get_new_latest_blockhash(&context.last_blockhash)
        .await
        .unwrap();

    // update to merge deactivated stake into reserve
    let error = stake_pool_accounts
        .update_all(
            &mut context.banks_client,
            &context.payer,
            &last_blockhash,
            false,
        )
        .await;
    assert!(error.is_none(), "{:?}", error);

    let validator_stake_account =
        get_account(&mut context.banks_client, &validator_stake.stake_account).await;
    let remaining_lamports = validator_stake_account.lamports;
    let stake_minimum_delegation =
        stake_get_minimum_delegation(&mut context.banks_client, &context.payer, &last_blockhash)
            .await;
    // make sure it's actually more than the minimum
    assert!(remaining_lamports > stake_rent + stake_minimum_delegation);

    // round up to force one more pool token if needed
    let pool_tokens_post_fee =
        (remaining_lamports * stake_pool.pool_token_supply).div_ceil(stake_pool.total_lamports);
    let new_user_authority = Pubkey::new_unique();
    let pool_tokens = stake_pool_accounts.calculate_inverse_withdrawal_fee(pool_tokens_post_fee);
    let error = stake_pool_accounts
        .withdraw_stake(
            &mut context.banks_client,
            &context.payer,
            &last_blockhash,
            &user_stake_recipient.pubkey(),
            &user_transfer_authority,
            &deposit_info.pool_account.pubkey(),
            &validator_stake.stake_account,
            &new_user_authority,
            pool_tokens,
        )
        .await;
    assert!(error.is_none(), "{:?}", error);

    // Check validator stake account gone
    let validator_stake_account = context
        .banks_client
        .get_account(validator_stake.stake_account)
        .await
        .unwrap();
    assert!(validator_stake_account.is_none());

    // Check user recipient stake account balance
    let user_stake_recipient_account =
        get_account(&mut context.banks_client, &user_stake_recipient.pubkey()).await;
    assert_eq!(
        user_stake_recipient_account.lamports,
        remaining_lamports + stake_rent
    );

    // Check that cleanup happens correctly
    stake_pool_accounts
        .cleanup_removed_validator_entries(
            &mut context.banks_client,
            &context.payer,
            &last_blockhash,
        )
        .await;

    let validator_list = get_account(
        &mut context.banks_client,
        &stake_pool_accounts.validator_list.pubkey(),
    )
    .await;
    let validator_list =
        try_from_slice_unchecked::<state::ValidatorList>(validator_list.data.as_slice()).unwrap();
    let validator_stake_item = validator_list.find(&validator_stake.vote.pubkey());
    assert!(validator_stake_item.is_none());
}

#[tokio::test]
async fn fail_with_reserve() {
    let (
        mut context,
        stake_pool_accounts,
        validator_stake,
        deposit_info,
        user_transfer_authority,
        user_stake_recipient,
        tokens_to_burn,
    ) = setup_for_withdraw(spl_token::id(), STAKE_ACCOUNT_RENT_EXEMPTION).await;

    // decrease a little stake, not all
    let error = stake_pool_accounts
        .decrease_validator_stake_either(
            &mut context.banks_client,
            &context.payer,
            &context.last_blockhash,
            &validator_stake.stake_account,
            &validator_stake.transient_stake_account,
            deposit_info.stake_lamports / 2,
            validator_stake.transient_stake_seed,
            DecreaseInstruction::Reserve,
        )
        .await;
    assert!(error.is_none(), "{:?}", error);

    // warp forward to deactivation
    let first_normal_slot = context.genesis_config().epoch_schedule.first_normal_slot;
    context.warp_to_slot(first_normal_slot + 1).unwrap();

    // update to merge deactivated stake into reserve
    stake_pool_accounts
        .update_all(
            &mut context.banks_client,
            &context.payer,
            &context.last_blockhash,
            false,
        )
        .await;

    // Withdraw directly from reserve, fail because some stake left
    let new_user_authority = Pubkey::new_unique();
    let error = stake_pool_accounts
        .withdraw_stake(
            &mut context.banks_client,
            &context.payer,
            &context.last_blockhash,
            &user_stake_recipient.pubkey(),
            &user_transfer_authority,
            &deposit_info.pool_account.pubkey(),
            &stake_pool_accounts.reserve_stake.pubkey(),
            &new_user_authority,
            tokens_to_burn,
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        error,
        TransactionError::InstructionError(
            0,
            InstructionError::Custom(StakePoolError::StakeLamportsNotEqualToMinimum as u32)
        )
    );
}

#[tokio::test]
async fn fail_with_transient() {
    let (
        mut context,
        stake_pool_accounts,
        validator_stake,
        deposit_info,
        user_transfer_authority,
        user_stake_recipient,
        _,
    ) = setup_for_withdraw(spl_token::id(), STAKE_ACCOUNT_RENT_EXEMPTION).await;

    // warp forward to after reward payout
    let first_normal_slot = context.genesis_config().epoch_schedule.first_normal_slot;
    let mut slot = first_normal_slot + 1;
    context.warp_to_slot(slot).unwrap();

    let error = stake_pool_accounts
        .update_all(
            &mut context.banks_client,
            &context.payer,
            &context.last_blockhash,
            false,
        )
        .await;
    assert!(error.is_none(), "{:?}", error);

    let rent = context.banks_client.get_rent().await.unwrap();
    let stake_rent = rent.minimum_balance(std::mem::size_of::<stake::state::StakeStateV2>());
    let stake_minimum_delegation = stake_get_minimum_delegation(
        &mut context.banks_client,
        &context.payer,
        &context.last_blockhash,
    )
    .await;

    // Calculate how much to decrease to leave only rent + minimum delegation in validator stake account
    // This will create a transient stake account with the decreased amount
    let validator_stake_account_before =
        get_account(&mut context.banks_client, &validator_stake.stake_account).await;
    let current_validator_lamports = validator_stake_account_before.lamports;
    let target_validator_lamports = stake_rent + stake_minimum_delegation;
    let amount_to_decrease = current_validator_lamports - target_validator_lamports;

    // Current validator stake: current_validator_lamports
    // Target validator stake: target_validator_lamports (rent + min delegation)
    // Amount to decrease: amount_to_decrease

    // Decrease validator stake, creating transient stake
    let error = stake_pool_accounts
        .decrease_validator_stake_either(
            &mut context.banks_client,
            &context.payer,
            &context.last_blockhash,
            &validator_stake.stake_account,
            &validator_stake.transient_stake_account,
            amount_to_decrease,
            validator_stake.transient_stake_seed,
            DecreaseInstruction::Additional,
        )
        .await;
    assert!(error.is_none(), "{:?}", error);

    // Check the state after decrease - validator should have minimum, transient should have the decreased amount
    let _validator_stake_account_after =
        get_account(&mut context.banks_client, &validator_stake.stake_account).await;
    let _transient_stake_account = get_account(
        &mut context.banks_client,
        &validator_stake.transient_stake_account,
    )
    .await;

    // Validator stake after decrease: validator_stake_account_after.lamports
    // Transient stake after decrease: transient_stake_account.lamports

    // warp forward to deactivation epoch
    slot += context.genesis_config().epoch_schedule.slots_per_epoch;
    context.warp_to_slot(slot).unwrap();

    let last_blockhash = context
        .banks_client
        .get_new_latest_blockhash(&context.last_blockhash)
        .await
        .unwrap();

    // update to merge deactivated stake into reserve, but use no_merge=true to keep transient stake separate
    let error = stake_pool_accounts
        .update_all(
            &mut context.banks_client,
            &context.payer,
            &last_blockhash,
            true, // no_merge = true to prevent merging transient stake back
        )
        .await;
    assert!(error.is_none(), "{:?}", error);

    // Check final state before withdrawal
    let _validator_stake_account_final =
        get_account(&mut context.banks_client, &validator_stake.stake_account).await;

    // Check if transient stake account still exists (it might have been merged back)
    let transient_stake_account_final = context
        .banks_client
        .get_account(validator_stake.transient_stake_account)
        .await
        .unwrap();

    // Validator stake after epoch change: validator_stake_account_final.lamports

    // Verify transient stake account still exists and has lamports
    let transient_account =
        transient_stake_account_final.expect("Transient stake account should still exist");
    // Transient stake after epoch change: transient_account.lamports

    // Calculate pool tokens needed to withdraw EXACTLY the transient stake amount
    let stake_pool = stake_pool_accounts
        .get_stake_pool(&mut context.banks_client)
        .await;

    // We want to withdraw exactly what's in the transient account (all of it)
    let exact_transient_lamports = transient_account.lamports;

    // Calculate the exact pool tokens needed for this withdrawal amount
    // Using the same formula as the stake pool: pool_tokens = (lamports * pool_token_supply) / total_lamports
    let pool_tokens_post_fee = (exact_transient_lamports * stake_pool.pool_token_supply)
        .checked_div(stake_pool.total_lamports)
        .unwrap();
    let pool_tokens = stake_pool_accounts.calculate_inverse_withdrawal_fee(pool_tokens_post_fee);

    // Transient account has exactly: exact_transient_lamports lamports
    // Attempting to withdraw exactly: exact_transient_lamports lamports from transient stake
    // Pool tokens calculated: pool_tokens (post-fee: pool_tokens_post_fee)
    // Stake pool total lamports: stake_pool.total_lamports, pool token supply: stake_pool.pool_token_supply

    let new_user_authority = Pubkey::new_unique();

    // Try to withdraw from transient stake account - this should FAIL
    let error = stake_pool_accounts
        .withdraw_stake(
            &mut context.banks_client,
            &context.payer,
            &last_blockhash,
            &user_stake_recipient.pubkey(),
            &user_transfer_authority,
            &deposit_info.pool_account.pubkey(),
            &validator_stake.transient_stake_account,
            &new_user_authority,
            pool_tokens,
        )
        .await;

    // Verify that the withdrawal fails with the expected error
    assert!(error.is_some(), "Withdrawal should fail");
    let transaction_error = error.unwrap().unwrap();
    assert_eq!(
        transaction_error,
        TransactionError::InstructionError(
            0,
            InstructionError::Custom(StakePoolError::StakeLamportsNotEqualToMinimum as u32)
        )
    );
}

#[tokio::test]
async fn success_with_reserve() {
    let (
        mut context,
        stake_pool_accounts,
        validator_stake,
        deposit_info,
        user_transfer_authority,
        user_stake_recipient,
        _,
    ) = setup_for_withdraw(spl_token::id(), STAKE_ACCOUNT_RENT_EXEMPTION).await;

    let rent = context.banks_client.get_rent().await.unwrap();
    let stake_rent = rent.minimum_balance(std::mem::size_of::<stake::state::StakeStateV2>());

    // decrease all of stake
    let error = stake_pool_accounts
        .decrease_validator_stake_either(
            &mut context.banks_client,
            &context.payer,
            &context.last_blockhash,
            &validator_stake.stake_account,
            &validator_stake.transient_stake_account,
            deposit_info.stake_lamports + stake_rent,
            validator_stake.transient_stake_seed,
            DecreaseInstruction::Reserve,
        )
        .await;
    assert!(error.is_none(), "{:?}", error);

    // warp forward to deactivation
    let first_normal_slot = context.genesis_config().epoch_schedule.first_normal_slot;
    context.warp_to_slot(first_normal_slot + 1).unwrap();

    // update to merge deactivated stake into reserve
    stake_pool_accounts
        .update_all(
            &mut context.banks_client,
            &context.payer,
            &context.last_blockhash,
            false,
        )
        .await;

    // now it works
    let new_user_authority = Pubkey::new_unique();
    let error = stake_pool_accounts
        .withdraw_stake(
            &mut context.banks_client,
            &context.payer,
            &context.last_blockhash,
            &user_stake_recipient.pubkey(),
            &user_transfer_authority,
            &deposit_info.pool_account.pubkey(),
            &stake_pool_accounts.reserve_stake.pubkey(),
            &new_user_authority,
            deposit_info.pool_tokens,
        )
        .await;
    assert!(error.is_none(), "{:?}", error);

    // first and only deposit, lamports:pool 1:1
    let stake_pool = get_account(
        &mut context.banks_client,
        &stake_pool_accounts.stake_pool.pubkey(),
    )
    .await;
    let stake_pool =
        try_from_slice_unchecked::<state::StakePool>(stake_pool.data.as_slice()).unwrap();
    // the entire deposit is actually stake since it isn't activated, so only
    // the stake deposit fee is charged
    let deposit_fee = stake_pool
        .calc_pool_tokens_stake_deposit_fee(stake_rent + deposit_info.stake_lamports)
        .unwrap();
    assert_eq!(
        deposit_info.stake_lamports + stake_rent - deposit_fee,
        deposit_info.pool_tokens,
        "stake {} rent {} deposit fee {} pool tokens {}",
        deposit_info.stake_lamports,
        stake_rent,
        deposit_fee,
        deposit_info.pool_tokens
    );

    let withdrawal_fee = stake_pool_accounts.calculate_withdrawal_fee(deposit_info.pool_tokens);

    // Check tokens used
    let user_token_balance = get_token_balance(
        &mut context.banks_client,
        &deposit_info.pool_account.pubkey(),
    )
    .await;
    assert_eq!(user_token_balance, 0);

    // Check reserve stake account balance
    let reserve_stake_account = get_account(
        &mut context.banks_client,
        &stake_pool_accounts.reserve_stake.pubkey(),
    )
    .await;
    let stake_state =
        deserialize::<stake::state::StakeStateV2>(&reserve_stake_account.data).unwrap();
    let meta = stake_state.meta().unwrap();
    assert_eq!(
        meta.rent_exempt_reserve + withdrawal_fee + deposit_fee + stake_rent,
        reserve_stake_account.lamports
    );

    // Check user recipient stake account balance
    let user_stake_recipient_account =
        get_account(&mut context.banks_client, &user_stake_recipient.pubkey()).await;
    assert_eq!(
        user_stake_recipient_account.lamports,
        deposit_info.stake_lamports + stake_rent * 2 - withdrawal_fee - deposit_fee
    );
}

#[tokio::test]
async fn success_with_empty_preferred_withdraw() {
    let (
        mut context,
        stake_pool_accounts,
        validator_stake,
        deposit_info,
        user_transfer_authority,
        user_stake_recipient,
        tokens_to_burn,
    ) = setup_for_withdraw(spl_token::id(), 0).await;

    let preferred_validator = simple_add_validator_to_pool(
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
            instruction::PreferredValidatorType::Withdraw,
            Some(preferred_validator.vote.pubkey()),
        )
        .await;

    // preferred is empty, withdrawing from non-preferred works
    let new_authority = Pubkey::new_unique();
    let error = stake_pool_accounts
        .withdraw_stake(
            &mut context.banks_client,
            &context.payer,
            &context.last_blockhash,
            &user_stake_recipient.pubkey(),
            &user_transfer_authority,
            &deposit_info.pool_account.pubkey(),
            &validator_stake.stake_account,
            &new_authority,
            tokens_to_burn / 2,
        )
        .await;
    assert!(error.is_none(), "{:?}", error);
}

#[tokio::test]
async fn success_and_fail_with_preferred_withdraw() {
    let (
        mut context,
        stake_pool_accounts,
        validator_stake,
        deposit_info,
        user_transfer_authority,
        user_stake_recipient,
        tokens_to_burn,
    ) = setup_for_withdraw(spl_token::id(), 0).await;

    let last_blockhash = context
        .banks_client
        .get_new_latest_blockhash(&context.last_blockhash)
        .await
        .unwrap();

    let preferred_validator = simple_add_validator_to_pool(
        &mut context.banks_client,
        &context.payer,
        &last_blockhash,
        &stake_pool_accounts,
        None,
    )
    .await;

    stake_pool_accounts
        .set_preferred_validator(
            &mut context.banks_client,
            &context.payer,
            &last_blockhash,
            instruction::PreferredValidatorType::Withdraw,
            Some(preferred_validator.vote.pubkey()),
        )
        .await;

    let _preferred_deposit = simple_deposit_stake(
        &mut context.banks_client,
        &context.payer,
        &last_blockhash,
        &stake_pool_accounts,
        &preferred_validator,
        TEST_STAKE_AMOUNT,
    )
    .await
    .unwrap();

    let new_authority = Pubkey::new_unique();
    let error = stake_pool_accounts
        .withdraw_stake(
            &mut context.banks_client,
            &context.payer,
            &last_blockhash,
            &user_stake_recipient.pubkey(),
            &user_transfer_authority,
            &deposit_info.pool_account.pubkey(),
            &validator_stake.stake_account,
            &new_authority,
            tokens_to_burn / 2 + 1,
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        error,
        TransactionError::InstructionError(
            0,
            InstructionError::Custom(StakePoolError::IncorrectWithdrawVoteAddress as u32)
        )
    );

    let last_blockhash = context
        .banks_client
        .get_new_latest_blockhash(&last_blockhash)
        .await
        .unwrap();

    // success from preferred
    let new_authority = Pubkey::new_unique();
    let error = stake_pool_accounts
        .withdraw_stake(
            &mut context.banks_client,
            &context.payer,
            &last_blockhash,
            &user_stake_recipient.pubkey(),
            &user_transfer_authority,
            &deposit_info.pool_account.pubkey(),
            &preferred_validator.stake_account,
            &new_authority,
            tokens_to_burn / 2,
        )
        .await;
    assert!(error.is_none(), "{:?}", error);
}

#[tokio::test]
async fn fail_withdraw_from_transient() {
    let (
        mut context,
        stake_pool_accounts,
        validator_stake_account,
        deposit_info,
        user_transfer_authority,
        user_stake_recipient,
        tokens_to_withdraw,
    ) = setup_for_withdraw(spl_token::id(), STAKE_ACCOUNT_RENT_EXEMPTION).await;

    let last_blockhash = context
        .banks_client
        .get_new_latest_blockhash(&context.last_blockhash)
        .await
        .unwrap();

    // add a preferred withdraw validator, keep it empty, to be sure that this works
    let preferred_validator = simple_add_validator_to_pool(
        &mut context.banks_client,
        &context.payer,
        &last_blockhash,
        &stake_pool_accounts,
        None,
    )
    .await;

    stake_pool_accounts
        .set_preferred_validator(
            &mut context.banks_client,
            &context.payer,
            &last_blockhash,
            instruction::PreferredValidatorType::Withdraw,
            Some(preferred_validator.vote.pubkey()),
        )
        .await;

    let last_blockhash = context
        .banks_client
        .get_new_latest_blockhash(&last_blockhash)
        .await
        .unwrap();

    let rent = context.banks_client.get_rent().await.unwrap();
    let stake_rent = rent.minimum_balance(std::mem::size_of::<stake::state::StakeStateV2>());

    // decrease to minimum stake + 2 lamports
    let error = stake_pool_accounts
        .decrease_validator_stake_either(
            &mut context.banks_client,
            &context.payer,
            &last_blockhash,
            &validator_stake_account.stake_account,
            &validator_stake_account.transient_stake_account,
            deposit_info.stake_lamports + stake_rent - 2,
            validator_stake_account.transient_stake_seed,
            DecreaseInstruction::Reserve,
        )
        .await;
    assert!(error.is_none(), "{:?}", error);

    // fail withdrawing from transient, still a lamport in the validator stake
    // account
    let new_user_authority = Pubkey::new_unique();
    let error = stake_pool_accounts
        .withdraw_stake(
            &mut context.banks_client,
            &context.payer,
            &last_blockhash,
            &user_stake_recipient.pubkey(),
            &user_transfer_authority,
            &deposit_info.pool_account.pubkey(),
            &validator_stake_account.transient_stake_account,
            &new_user_authority,
            tokens_to_withdraw,
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        error,
        TransactionError::InstructionError(
            0,
            InstructionError::Custom(StakePoolError::InvalidStakeAccountAddress as u32)
        )
    );
}

#[tokio::test]
async fn success_withdraw_from_transient() {
    let (
        mut context,
        stake_pool_accounts,
        validator_stake_account,
        deposit_info,
        user_transfer_authority,
        user_stake_recipient,
        tokens_to_withdraw,
    ) = setup_for_withdraw(spl_token::id(), STAKE_ACCOUNT_RENT_EXEMPTION).await;

    let last_blockhash = context
        .banks_client
        .get_new_latest_blockhash(&context.last_blockhash)
        .await
        .unwrap();

    // add a preferred withdraw validator, keep it empty, to be sure that this works
    let preferred_validator = simple_add_validator_to_pool(
        &mut context.banks_client,
        &context.payer,
        &last_blockhash,
        &stake_pool_accounts,
        None,
    )
    .await;

    stake_pool_accounts
        .set_preferred_validator(
            &mut context.banks_client,
            &context.payer,
            &context.last_blockhash,
            instruction::PreferredValidatorType::Withdraw,
            Some(preferred_validator.vote.pubkey()),
        )
        .await;

    let rent = context.banks_client.get_rent().await.unwrap();
    let stake_rent = rent.minimum_balance(std::mem::size_of::<stake::state::StakeStateV2>());

    let last_blockhash = context
        .banks_client
        .get_new_latest_blockhash(&last_blockhash)
        .await
        .unwrap();

    // decrease all of stake
    let error = stake_pool_accounts
        .decrease_validator_stake_either(
            &mut context.banks_client,
            &context.payer,
            &last_blockhash,
            &validator_stake_account.stake_account,
            &validator_stake_account.transient_stake_account,
            deposit_info.stake_lamports + stake_rent,
            validator_stake_account.transient_stake_seed,
            DecreaseInstruction::Reserve,
        )
        .await;
    assert!(error.is_none(), "{:?}", error);

    // nothing left in the validator stake account (or any others), so withdrawing
    // from the transient account is ok!
    let new_user_authority = Pubkey::new_unique();
    let error = stake_pool_accounts
        .withdraw_stake(
            &mut context.banks_client,
            &context.payer,
            &last_blockhash,
            &user_stake_recipient.pubkey(),
            &user_transfer_authority,
            &deposit_info.pool_account.pubkey(),
            &validator_stake_account.transient_stake_account,
            &new_user_authority,
            tokens_to_withdraw / 2,
        )
        .await;
    assert!(error.is_none(), "{:?}", error);
}

#[tokio::test]
async fn success_with_small_preferred_withdraw() {
    let (
        mut context,
        stake_pool_accounts,
        validator_stake,
        deposit_info,
        user_transfer_authority,
        user_stake_recipient,
        tokens_to_burn,
    ) = setup_for_withdraw(spl_token::id(), 0).await;

    let last_blockhash = context
        .banks_client
        .get_new_latest_blockhash(&context.last_blockhash)
        .await
        .unwrap();

    // make pool tokens very valuable, so it isn't possible to exactly get down to
    // the minimum
    transfer(
        &mut context.banks_client,
        &context.payer,
        &last_blockhash,
        &stake_pool_accounts.reserve_stake.pubkey(),
        deposit_info.stake_lamports * 5, // each pool token is worth more than one lamport
    )
    .await;
    stake_pool_accounts
        .update_all(
            &mut context.banks_client,
            &context.payer,
            &last_blockhash,
            false,
        )
        .await;

    let preferred_validator = simple_add_validator_to_pool(
        &mut context.banks_client,
        &context.payer,
        &last_blockhash,
        &stake_pool_accounts,
        None,
    )
    .await;

    stake_pool_accounts
        .set_preferred_validator(
            &mut context.banks_client,
            &context.payer,
            &last_blockhash,
            instruction::PreferredValidatorType::Withdraw,
            Some(preferred_validator.vote.pubkey()),
        )
        .await;

    let last_blockhash = context
        .banks_client
        .get_new_latest_blockhash(&last_blockhash)
        .await
        .unwrap();

    // add a tiny bit of stake, less than lamports per pool token to preferred
    // validator
    let rent = context.banks_client.get_rent().await.unwrap();
    let rent_exempt = rent.minimum_balance(std::mem::size_of::<stake::state::StakeStateV2>());
    let stake_minimum_delegation =
        stake_get_minimum_delegation(&mut context.banks_client, &context.payer, &last_blockhash)
            .await;
    let minimum_lamports = stake_minimum_delegation + rent_exempt;

    simple_deposit_stake(
        &mut context.banks_client,
        &context.payer,
        &last_blockhash,
        &stake_pool_accounts,
        &preferred_validator,
        stake_minimum_delegation + 1, // stake_rent gets deposited too
    )
    .await
    .unwrap();

    // decrease all stake except for 1 lamport
    let error = stake_pool_accounts
        .decrease_validator_stake_either(
            &mut context.banks_client,
            &context.payer,
            &last_blockhash,
            &preferred_validator.stake_account,
            &preferred_validator.transient_stake_account,
            minimum_lamports,
            preferred_validator.transient_stake_seed,
            DecreaseInstruction::Reserve,
        )
        .await;
    assert!(error.is_none(), "{:?}", error);

    // warp forward to deactivation
    let first_normal_slot = context.genesis_config().epoch_schedule.first_normal_slot;
    context.warp_to_slot(first_normal_slot + 1).unwrap();

    // update to merge deactivated stake into reserve
    stake_pool_accounts
        .update_all(
            &mut context.banks_client,
            &context.payer,
            &last_blockhash,
            false,
        )
        .await;

    // withdraw from preferred fails
    let new_authority = Pubkey::new_unique();
    let error = stake_pool_accounts
        .withdraw_stake(
            &mut context.banks_client,
            &context.payer,
            &last_blockhash,
            &user_stake_recipient.pubkey(),
            &user_transfer_authority,
            &deposit_info.pool_account.pubkey(),
            &preferred_validator.stake_account,
            &new_authority,
            1,
        )
        .await;
    assert!(error.is_some());

    // preferred is empty, withdrawing from non-preferred works
    let new_authority = Pubkey::new_unique();
    let error = stake_pool_accounts
        .withdraw_stake(
            &mut context.banks_client,
            &context.payer,
            &last_blockhash,
            &user_stake_recipient.pubkey(),
            &user_transfer_authority,
            &deposit_info.pool_account.pubkey(),
            &validator_stake.stake_account,
            &new_authority,
            tokens_to_burn / 6,
        )
        .await;
    assert!(error.is_none(), "{:?}", error);
}

#[tokio::test]
async fn fail_overdraw_reserve() {
    let mut context = program_test().start_with_context().await;
    let mut stake_pool_accounts = StakePoolAccounts::new_with_token_program(spl_token::id());

    // Set withdrawal fees to zero for easier calculation
    stake_pool_accounts.withdrawal_fee = spl_stake_pool::state::Fee {
        numerator: 0,
        denominator: 1,
    };
    stake_pool_accounts.sol_deposit_fee = spl_stake_pool::state::Fee {
        numerator: 0,
        denominator: 1,
    };

    // Initialize stake pool with minimal reserve
    let rent = context.banks_client.get_rent().await.unwrap();
    let stake_rent = rent.minimum_balance(std::mem::size_of::<stake::state::StakeStateV2>());
    let reserve_lamports = stake_rent + MINIMUM_RESERVE_LAMPORTS;

    stake_pool_accounts
        .initialize_stake_pool(
            &mut context.banks_client,
            &context.payer,
            &context.last_blockhash,
            reserve_lamports,
        )
        .await
        .unwrap();

    // Create pool token account for deposits
    let user_pool_account = Keypair::new();
    create_token_account(
        &mut context.banks_client,
        &context.payer,
        &context.last_blockhash,
        &stake_pool_accounts.token_program_id,
        &user_pool_account,
        &stake_pool_accounts.pool_mint.pubkey(),
        &context.payer,
        &[],
    )
    .await
    .unwrap();

    // Deposit 5 SOL into the pool (this will mint pool tokens to the user)
    let deposit_amount = 5_000_000_000;
    let error = stake_pool_accounts
        .deposit_sol(
            &mut context.banks_client,
            &context.payer,
            &context.last_blockhash,
            &user_pool_account.pubkey(),
            deposit_amount,
            None, // No SOL deposit authority
        )
        .await;
    assert!(error.is_none(), "SOL deposit failed: {:?}", error);

    // Add a validator to the pool
    let validator_stake_account = ValidatorStakeAccount::new(
        &stake_pool_accounts.stake_pool.pubkey(),
        DEFAULT_VALIDATOR_STAKE_SEED,
        DEFAULT_TRANSIENT_STAKE_SEED,
    );

    // Create vote account for the validator
    create_vote(
        &mut context.banks_client,
        &context.payer,
        &context.last_blockhash,
        &validator_stake_account.validator,
        &validator_stake_account.vote,
    )
    .await;

    let error = stake_pool_accounts
        .add_validator_to_pool(
            &mut context.banks_client,
            &context.payer,
            &context.last_blockhash,
            &validator_stake_account.stake_account,
            &validator_stake_account.vote.pubkey(),
            validator_stake_account.validator_stake_seed,
        )
        .await;
    assert!(error.is_none(), "Add validator failed: {:?}", error);

    let reserve_account = get_account(
        &mut context.banks_client,
        &stake_pool_accounts.reserve_stake.pubkey(),
    )
    .await;
    let withdrawable_amount = reserve_account.lamports;

    // Get current stake pool state
    let stake_pool_before = stake_pool_accounts
        .get_stake_pool(&mut context.banks_client)
        .await;

    // Calculate how many pool tokens we need to withdraw the full withdrawable reserve amount
    let pool_tokens_needed = stake_pool_before
        .calc_pool_tokens_for_deposit(withdrawable_amount)
        .unwrap();

    // Get the pool tokens that were minted from our SOL deposit
    let user_pool_tokens =
        get_token_balance(&mut context.banks_client, &user_pool_account.pubkey()).await;

    // We need to use the amount that allows us to withdraw the full reserve
    let pool_tokens_to_use = pool_tokens_needed.min(user_pool_tokens);

    // Create destination stake account for withdrawal
    let destination_stake_account = Keypair::new();
    create_blank_stake_account(
        &mut context.banks_client,
        &context.payer,
        &context.last_blockhash,
        &destination_stake_account,
    )
    .await;

    let new_user_authority = Pubkey::new_unique();
    let error = stake_pool_accounts
        .withdraw_stake(
            &mut context.banks_client,
            &context.payer,
            &context.last_blockhash,
            &destination_stake_account.pubkey(),
            &context.payer,
            &user_pool_account.pubkey(),
            &stake_pool_accounts.reserve_stake.pubkey(),
            &new_user_authority,
            pool_tokens_to_use,
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        error,
        TransactionError::InstructionError(
            0,
            InstructionError::Custom(StakePoolError::SolWithdrawalTooLarge as u32)
        )
    );
}

#[tokio::test]
async fn success_remove_preferred_validator_resets_preference() {
    let (
        mut context,
        stake_pool_accounts,
        validator_stake,
        deposit_info,
        user_transfer_authority,
        user_stake_recipient,
        _,
    ) = setup_for_withdraw(spl_token::id(), STAKE_ACCOUNT_RENT_EXEMPTION).await;

    let last_blockhash = context
        .banks_client
        .get_new_latest_blockhash(&context.last_blockhash)
        .await
        .unwrap();

    // Set the validator as the preferred withdraw validator
    stake_pool_accounts
        .set_preferred_validator(
            &mut context.banks_client,
            &context.payer,
            &last_blockhash,
            instruction::PreferredValidatorType::Withdraw,
            Some(validator_stake.vote.pubkey()),
        )
        .await;

    // Also set it as preferred deposit validator to test both reset paths
    stake_pool_accounts
        .set_preferred_validator(
            &mut context.banks_client,
            &context.payer,
            &last_blockhash,
            instruction::PreferredValidatorType::Deposit,
            Some(validator_stake.vote.pubkey()),
        )
        .await;

    // Verify the preferred deposit and withdraw validators are set
    let stake_pool_before = stake_pool_accounts
        .get_stake_pool(&mut context.banks_client)
        .await;
    assert_eq!(
        stake_pool_before.preferred_withdraw_validator_vote_address,
        Some(validator_stake.vote.pubkey())
    );
    assert_eq!(
        stake_pool_before.preferred_deposit_validator_vote_address,
        Some(validator_stake.vote.pubkey())
    );

    // Preferred validators set to: validator_stake.vote.pubkey()

    // Warp forward to after reward payout
    let first_normal_slot = context.genesis_config().epoch_schedule.first_normal_slot;
    let mut slot = first_normal_slot + 1;
    context.warp_to_slot(slot).unwrap();

    let error = stake_pool_accounts
        .update_all(
            &mut context.banks_client,
            &context.payer,
            &last_blockhash,
            false,
        )
        .await;
    assert!(error.is_none(), "{:?}", error);

    let rent = context.banks_client.get_rent().await.unwrap();
    let stake_rent = rent.minimum_balance(std::mem::size_of::<stake::state::StakeStateV2>());
    let stake_pool = stake_pool_accounts
        .get_stake_pool(&mut context.banks_client)
        .await;
    let lamports_per_pool_token = stake_pool.get_lamports_per_pool_token().unwrap();

    // Decrease all of stake except for exactly lamports_per_pool_token lamports
    // This will leave the minimum amount that can be withdrawn completely
    let error = stake_pool_accounts
        .decrease_validator_stake_either(
            &mut context.banks_client,
            &context.payer,
            &last_blockhash,
            &validator_stake.stake_account,
            &validator_stake.transient_stake_account,
            deposit_info.stake_lamports + stake_rent - lamports_per_pool_token,
            validator_stake.transient_stake_seed,
            DecreaseInstruction::Reserve,
        )
        .await;
    assert!(error.is_none(), "{:?}", error);

    // Warp forward to deactivation
    slot += context.genesis_config().epoch_schedule.slots_per_epoch;
    context.warp_to_slot(slot).unwrap();

    let last_blockhash = context
        .banks_client
        .get_new_latest_blockhash(&last_blockhash)
        .await
        .unwrap();

    // Update to merge deactivated stake into reserve
    let error = stake_pool_accounts
        .update_all(
            &mut context.banks_client,
            &context.payer,
            &last_blockhash,
            false,
        )
        .await;
    assert!(error.is_none(), "{:?}", error);

    let validator_stake_account =
        get_account(&mut context.banks_client, &validator_stake.stake_account).await;
    let remaining_lamports = validator_stake_account.lamports;
    let stake_minimum_delegation =
        stake_get_minimum_delegation(&mut context.banks_client, &context.payer, &last_blockhash)
            .await;

    // Remaining lamports in validator: remaining_lamports
    // Stake rent: stake_rent
    // Minimum delegation: stake_minimum_delegation
    // Make sure it's actually more than the minimum (should be exactly lamports_per_pool_token)
    assert!(remaining_lamports > stake_rent + stake_minimum_delegation);

    // Calculate pool tokens needed to withdraw everything (this should remove the validator completely)
    let stake_pool_updated = stake_pool_accounts
        .get_stake_pool(&mut context.banks_client)
        .await;
    let pool_tokens_post_fee = (remaining_lamports * stake_pool_updated.pool_token_supply)
        .div_ceil(stake_pool_updated.total_lamports);
    let pool_tokens = stake_pool_accounts.calculate_inverse_withdrawal_fee(pool_tokens_post_fee);

    // Pool tokens needed for complete withdrawal: pool_tokens

    let new_user_authority = Pubkey::new_unique();

    // Perform the complete withdrawal - this should trigger validator removal and reset preferred validators
    let error = stake_pool_accounts
        .withdraw_stake(
            &mut context.banks_client,
            &context.payer,
            &last_blockhash,
            &user_stake_recipient.pubkey(),
            &user_transfer_authority,
            &deposit_info.pool_account.pubkey(),
            &validator_stake.stake_account,
            &new_user_authority,
            pool_tokens,
        )
        .await;
    assert!(
        error.is_none(),
        "Complete withdrawal should succeed: {:?}",
        error
    );

    // Complete withdrawal successful - validator should be removed

    // Verify validator stake account is gone
    let validator_stake_account_after = context
        .banks_client
        .get_account(validator_stake.stake_account)
        .await
        .unwrap();
    assert!(
        validator_stake_account_after.is_none(),
        "Validator stake account should be removed"
    );

    // Verify that preferred validators have been reset to None
    let stake_pool_after = stake_pool_accounts
        .get_stake_pool(&mut context.banks_client)
        .await;

    assert_eq!(
        stake_pool_after.preferred_withdraw_validator_vote_address, None,
        "Preferred withdraw validator should be reset to None"
    );
    assert_eq!(
        stake_pool_after.preferred_deposit_validator_vote_address, None,
        "Preferred deposit validator should be reset to None"
    );

    // Verify user received the stake
    let user_stake_recipient_account =
        get_account(&mut context.banks_client, &user_stake_recipient.pubkey()).await;
    assert_eq!(
        user_stake_recipient_account.lamports,
        remaining_lamports + stake_rent,
        "User should receive all lamports from removed validator"
    );
}
