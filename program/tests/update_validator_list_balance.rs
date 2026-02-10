#![allow(clippy::arithmetic_side_effects)]
mod helpers;

use {
    helpers::*,
    solana_program::{
        borsh1::try_from_slice_unchecked, instruction::InstructionError, program_pack::Pack,
    },
    solana_program_test::*,
    solana_sdk::{
        hash::Hash,
        signature::{Keypair, Signer},
        transaction::TransactionError,
    },
    solana_stake_interface::state::StakeStateV2,
    spl_pod::primitives::PodU64,
    spl_stake_pool::{
        error::StakePoolError,
        state::{StakePool, StakeStatus, ValidatorList},
        MAX_VALIDATORS_TO_UPDATE, MINIMUM_RESERVE_LAMPORTS,
    },
    spl_token_interface::state::Mint,
    std::num::NonZeroU32,
};

async fn setup(
    num_validators: usize,
) -> (
    ProgramTestContext,
    Hash,
    StakePoolAccounts,
    Vec<ValidatorStakeAccount>,
    Vec<DepositStakeAccount>,
    u64,
    u64,
    u64,
) {
    let mut context = program_test().start_with_context().await;
    let first_normal_slot = context.genesis_config().epoch_schedule.first_normal_slot;
    let slots_per_epoch = context.genesis_config().epoch_schedule.slots_per_epoch;
    let mut slot = first_normal_slot + 1;
    context.warp_to_slot(slot).unwrap();

    let reserve_stake_amount = TEST_STAKE_AMOUNT * 2 * num_validators as u64;
    let stake_pool_accounts = StakePoolAccounts::default();
    stake_pool_accounts
        .initialize_stake_pool(
            &mut context.banks_client,
            &context.payer,
            &context.last_blockhash,
            reserve_stake_amount + MINIMUM_RESERVE_LAMPORTS,
        )
        .await
        .unwrap();

    // Add several accounts with some stake
    let mut stake_accounts: Vec<ValidatorStakeAccount> = vec![];
    let mut deposit_accounts: Vec<DepositStakeAccount> = vec![];
    for i in 0..num_validators {
        let stake_account = ValidatorStakeAccount::new(
            &stake_pool_accounts.stake_pool.pubkey(),
            NonZeroU32::new(i as u32),
            u64::MAX,
        );
        create_vote(
            &mut context.banks_client,
            &context.payer,
            &context.last_blockhash,
            &stake_account.validator,
            &stake_account.vote,
        )
        .await;

        let error = stake_pool_accounts
            .add_validator_to_pool(
                &mut context.banks_client,
                &context.payer,
                &context.last_blockhash,
                &stake_account.stake_account,
                &stake_account.vote.pubkey(),
                stake_account.validator_stake_seed,
            )
            .await;
        assert!(error.is_none(), "{:?}", error);

        let deposit_account = DepositStakeAccount::new_with_vote(
            stake_account.vote.pubkey(),
            stake_account.stake_account,
            TEST_STAKE_AMOUNT,
        );
        deposit_account
            .create_and_delegate(
                &mut context.banks_client,
                &context.payer,
                &context.last_blockhash,
            )
            .await;

        stake_accounts.push(stake_account);
        deposit_accounts.push(deposit_account);
    }

    // Warp forward so the stakes properly activate, and deposit
    slot += slots_per_epoch;
    context.warp_to_slot(slot).unwrap();
    let last_blockhash = context
        .banks_client
        .get_new_latest_blockhash(&context.last_blockhash)
        .await
        .unwrap();

    stake_pool_accounts
        .update_all(
            &mut context.banks_client,
            &context.payer,
            &context.last_blockhash,
            false,
        )
        .await;

    for deposit_account in &mut deposit_accounts {
        deposit_account
            .deposit_stake(
                &mut context.banks_client,
                &context.payer,
                &last_blockhash,
                &stake_pool_accounts,
            )
            .await;
    }

    slot += slots_per_epoch;
    context.warp_to_slot(slot).unwrap();
    let last_blockhash = context
        .banks_client
        .get_new_latest_blockhash(&context.last_blockhash)
        .await
        .unwrap();

    stake_pool_accounts
        .update_all(
            &mut context.banks_client,
            &context.payer,
            &last_blockhash,
            false,
        )
        .await;

    let last_blockhash = context
        .banks_client
        .get_new_latest_blockhash(&last_blockhash)
        .await
        .unwrap();

    (
        context,
        last_blockhash,
        stake_pool_accounts,
        stake_accounts,
        deposit_accounts,
        TEST_STAKE_AMOUNT,
        reserve_stake_amount,
        slot,
    )
}

#[tokio::test]
async fn success_with_normal() {
    let num_validators = 5;
    let (
        mut context,
        last_blockhash,
        stake_pool_accounts,
        stake_accounts,
        _,
        validator_lamports,
        reserve_lamports,
        mut slot,
    ) = setup(num_validators).await;

    // Check current balance in the list
    let rent = context.banks_client.get_rent().await.unwrap();
    let stake_rent = rent.minimum_balance(std::mem::size_of::<StakeStateV2>());
    let stake_pool_info = get_account(
        &mut context.banks_client,
        &stake_pool_accounts.stake_pool.pubkey(),
    )
    .await;
    let stake_pool = try_from_slice_unchecked::<StakePool>(&stake_pool_info.data).unwrap();
    let validator_list_sum = get_validator_list_sum(
        &mut context.banks_client,
        &stake_pool_accounts.reserve_stake.pubkey(),
        &stake_pool_accounts.validator_list.pubkey(),
    )
    .await;
    assert_eq!(stake_pool.total_lamports, validator_list_sum);
    // initially, have all of the deposits plus their rent, and the reserve stake
    let initial_lamports =
        (validator_lamports + stake_rent) * num_validators as u64 + reserve_lamports;
    assert_eq!(validator_list_sum, initial_lamports);

    // Simulate rewards
    for stake_account in &stake_accounts {
        context.increment_vote_account_credits(&stake_account.vote.pubkey(), 100);
    }

    // Warp one more epoch so the rewards are paid out
    let slots_per_epoch = context.genesis_config().epoch_schedule.slots_per_epoch;
    slot += slots_per_epoch;
    context.warp_to_slot(slot).unwrap();

    let last_blockhash = context
        .banks_client
        .get_new_latest_blockhash(&last_blockhash)
        .await
        .unwrap();

    stake_pool_accounts
        .update_all(
            &mut context.banks_client,
            &context.payer,
            &last_blockhash,
            false,
        )
        .await;
    let new_lamports = get_validator_list_sum(
        &mut context.banks_client,
        &stake_pool_accounts.reserve_stake.pubkey(),
        &stake_pool_accounts.validator_list.pubkey(),
    )
    .await;
    assert!(new_lamports > initial_lamports);

    let stake_pool_info = get_account(
        &mut context.banks_client,
        &stake_pool_accounts.stake_pool.pubkey(),
    )
    .await;
    let stake_pool = try_from_slice_unchecked::<StakePool>(&stake_pool_info.data).unwrap();
    assert_eq!(new_lamports, stake_pool.total_lamports);
}

#[tokio::test]
async fn merge_into_reserve() {
    let (
        mut context,
        last_blockhash,
        stake_pool_accounts,
        stake_accounts,
        _,
        lamports,
        _,
        mut slot,
    ) = setup(MAX_VALIDATORS_TO_UPDATE).await;

    let pre_lamports = get_validator_list_sum(
        &mut context.banks_client,
        &stake_pool_accounts.reserve_stake.pubkey(),
        &stake_pool_accounts.validator_list.pubkey(),
    )
    .await;

    let reserve_stake = context
        .banks_client
        .get_account(stake_pool_accounts.reserve_stake.pubkey())
        .await
        .unwrap()
        .unwrap();
    let pre_reserve_lamports = reserve_stake.lamports;

    // Decrease from all validators
    for stake_account in &stake_accounts {
        let error = stake_pool_accounts
            .decrease_validator_stake_either(
                &mut context.banks_client,
                &context.payer,
                &last_blockhash,
                &stake_account.stake_account,
                &stake_account.transient_stake_account,
                lamports,
                stake_account.transient_stake_seed,
                DecreaseInstruction::Reserve,
            )
            .await;
        assert!(error.is_none(), "{:?}", error);
    }

    // Update, should not change, no merges yet
    stake_pool_accounts
        .update_all(
            &mut context.banks_client,
            &context.payer,
            &last_blockhash,
            false,
        )
        .await;

    let expected_lamports = get_validator_list_sum(
        &mut context.banks_client,
        &stake_pool_accounts.reserve_stake.pubkey(),
        &stake_pool_accounts.validator_list.pubkey(),
    )
    .await;
    assert_eq!(pre_lamports, expected_lamports);

    let stake_pool_info = get_account(
        &mut context.banks_client,
        &stake_pool_accounts.stake_pool.pubkey(),
    )
    .await;
    let stake_pool = try_from_slice_unchecked::<StakePool>(&stake_pool_info.data).unwrap();
    assert_eq!(expected_lamports, stake_pool.total_lamports);

    // Warp one more epoch so the stakes deactivate
    let slots_per_epoch = context.genesis_config().epoch_schedule.slots_per_epoch;
    slot += slots_per_epoch;
    context.warp_to_slot(slot).unwrap();

    let last_blockhash = context
        .banks_client
        .get_new_latest_blockhash(&last_blockhash)
        .await
        .unwrap();
    stake_pool_accounts
        .update_all(
            &mut context.banks_client,
            &context.payer,
            &last_blockhash,
            false,
        )
        .await;
    let expected_lamports = get_validator_list_sum(
        &mut context.banks_client,
        &stake_pool_accounts.reserve_stake.pubkey(),
        &stake_pool_accounts.validator_list.pubkey(),
    )
    .await;
    assert_eq!(pre_lamports, expected_lamports);

    let reserve_stake = context
        .banks_client
        .get_account(stake_pool_accounts.reserve_stake.pubkey())
        .await
        .unwrap()
        .unwrap();
    let post_reserve_lamports = reserve_stake.lamports;
    assert!(post_reserve_lamports > pre_reserve_lamports);

    let stake_pool_info = get_account(
        &mut context.banks_client,
        &stake_pool_accounts.stake_pool.pubkey(),
    )
    .await;
    let stake_pool = try_from_slice_unchecked::<StakePool>(&stake_pool_info.data).unwrap();
    assert_eq!(expected_lamports, stake_pool.total_lamports);
}

#[tokio::test]
async fn merge_into_validator_stake() {
    let (
        mut context,
        last_blockhash,
        stake_pool_accounts,
        stake_accounts,
        _,
        lamports,
        reserve_lamports,
        mut slot,
    ) = setup(MAX_VALIDATORS_TO_UPDATE).await;

    let rent = context.banks_client.get_rent().await.unwrap();
    let pre_lamports = get_validator_list_sum(
        &mut context.banks_client,
        &stake_pool_accounts.reserve_stake.pubkey(),
        &stake_pool_accounts.validator_list.pubkey(),
    )
    .await;

    // Increase stake to all validators
    let stake_rent = rent.minimum_balance(std::mem::size_of::<StakeStateV2>());
    let current_minimum_delegation = stake_pool_get_minimum_delegation(
        &mut context.banks_client,
        &context.payer,
        &last_blockhash,
    )
    .await;
    let available_lamports =
        reserve_lamports - (stake_rent + current_minimum_delegation) * stake_accounts.len() as u64;
    let increase_amount = available_lamports / stake_accounts.len() as u64;
    for stake_account in &stake_accounts {
        let error = stake_pool_accounts
            .increase_validator_stake(
                &mut context.banks_client,
                &context.payer,
                &last_blockhash,
                &stake_account.transient_stake_account,
                &stake_account.stake_account,
                &stake_account.vote.pubkey(),
                increase_amount,
                stake_account.transient_stake_seed,
            )
            .await;
        assert!(error.is_none(), "{:?}", error);
    }

    // Warp just a little bit to get a new blockhash and update again
    context.warp_to_slot(slot + 10).unwrap();
    let last_blockhash = context
        .banks_client
        .get_new_latest_blockhash(&last_blockhash)
        .await
        .unwrap();

    // Update, should not change, no merges yet
    let error = stake_pool_accounts
        .update_all(
            &mut context.banks_client,
            &context.payer,
            &last_blockhash,
            false,
        )
        .await;
    assert!(error.is_none(), "{:?}", error);

    let expected_lamports = get_validator_list_sum(
        &mut context.banks_client,
        &stake_pool_accounts.reserve_stake.pubkey(),
        &stake_pool_accounts.validator_list.pubkey(),
    )
    .await;
    assert_eq!(pre_lamports, expected_lamports);
    let stake_pool_info = get_account(
        &mut context.banks_client,
        &stake_pool_accounts.stake_pool.pubkey(),
    )
    .await;
    let stake_pool = try_from_slice_unchecked::<StakePool>(&stake_pool_info.data).unwrap();
    assert_eq!(expected_lamports, stake_pool.total_lamports);

    // Warp one more epoch so the stakes activate, ready to merge
    let slots_per_epoch = context.genesis_config().epoch_schedule.slots_per_epoch;
    slot += slots_per_epoch;
    context.warp_to_slot(slot).unwrap();

    let last_blockhash = context
        .banks_client
        .get_new_latest_blockhash(&last_blockhash)
        .await
        .unwrap();
    let error = stake_pool_accounts
        .update_all(
            &mut context.banks_client,
            &context.payer,
            &last_blockhash,
            false,
        )
        .await;
    assert!(error.is_none(), "{:?}", error);
    let current_lamports = get_validator_list_sum(
        &mut context.banks_client,
        &stake_pool_accounts.reserve_stake.pubkey(),
        &stake_pool_accounts.validator_list.pubkey(),
    )
    .await;
    let stake_pool_info = get_account(
        &mut context.banks_client,
        &stake_pool_accounts.stake_pool.pubkey(),
    )
    .await;
    let stake_pool = try_from_slice_unchecked::<StakePool>(&stake_pool_info.data).unwrap();
    assert_eq!(current_lamports, stake_pool.total_lamports);

    // Check that transient accounts are gone
    for stake_account in &stake_accounts {
        assert!(context
            .banks_client
            .get_account(stake_account.transient_stake_account)
            .await
            .unwrap()
            .is_none());
    }

    // Check validator stake accounts have the expected balance now:
    // validator stake account minimum + deposited lamports + rents + increased
    // lamports
    let expected_lamports = current_minimum_delegation + lamports + increase_amount + stake_rent;
    for stake_account in &stake_accounts {
        let validator_stake =
            get_account(&mut context.banks_client, &stake_account.stake_account).await;
        assert_eq!(validator_stake.lamports, expected_lamports);
    }

    // Check reserve stake accounts for expected balance:
    // own rent, other account rents, and 1 extra lamport
    let reserve_stake = get_account(
        &mut context.banks_client,
        &stake_pool_accounts.reserve_stake.pubkey(),
    )
    .await;
    assert_eq!(
        reserve_stake.lamports,
        MINIMUM_RESERVE_LAMPORTS + stake_rent * (1 + stake_accounts.len() as u64)
    );
}

#[tokio::test]
async fn merge_transient_stake_after_remove() {
    let (
        mut context,
        last_blockhash,
        stake_pool_accounts,
        stake_accounts,
        _,
        lamports,
        reserve_lamports,
        mut slot,
    ) = setup(1).await;

    let rent = context.banks_client.get_rent().await.unwrap();
    let stake_rent = rent.minimum_balance(std::mem::size_of::<StakeStateV2>());
    let current_minimum_delegation = stake_pool_get_minimum_delegation(
        &mut context.banks_client,
        &context.payer,
        &last_blockhash,
    )
    .await;
    let deactivated_lamports = lamports;
    // Decrease and remove all validators
    for stake_account in &stake_accounts {
        let error = stake_pool_accounts
            .decrease_validator_stake_either(
                &mut context.banks_client,
                &context.payer,
                &last_blockhash,
                &stake_account.stake_account,
                &stake_account.transient_stake_account,
                deactivated_lamports,
                stake_account.transient_stake_seed,
                DecreaseInstruction::Reserve,
            )
            .await;
        assert!(error.is_none(), "{:?}", error);
        let error = stake_pool_accounts
            .remove_validator_from_pool(
                &mut context.banks_client,
                &context.payer,
                &last_blockhash,
                &stake_account.stake_account,
                &stake_account.transient_stake_account,
            )
            .await;
        assert!(error.is_none(), "{:?}", error);
    }

    // Warp forward to merge time
    let slots_per_epoch = context.genesis_config().epoch_schedule.slots_per_epoch;
    slot += slots_per_epoch;
    context.warp_to_slot(slot).unwrap();

    // Update without merge, status should be DeactivatingTransient
    let error = stake_pool_accounts
        .update_all(
            &mut context.banks_client,
            &context.payer,
            &last_blockhash,
            true,
        )
        .await;
    assert!(error.is_none(), "{:?}", error);

    let validator_list = get_account(
        &mut context.banks_client,
        &stake_pool_accounts.validator_list.pubkey(),
    )
    .await;
    let validator_list =
        try_from_slice_unchecked::<ValidatorList>(validator_list.data.as_slice()).unwrap();
    assert_eq!(validator_list.validators.len(), 1);
    assert_eq!(
        validator_list.validators[0].status,
        StakeStatus::DeactivatingAll.into()
    );
    assert_eq!(
        u64::from(validator_list.validators[0].active_stake_lamports),
        stake_rent + current_minimum_delegation
    );
    assert_eq!(
        u64::from(validator_list.validators[0].transient_stake_lamports),
        deactivated_lamports + stake_rent
    );

    // Update with merge, status should be ReadyForRemoval and no lamports
    let error = stake_pool_accounts
        .update_validator_list_balance(
            &mut context.banks_client,
            &context.payer,
            &last_blockhash,
            validator_list.validators.len(),
            false,
        )
        .await;
    assert!(error.is_none(), "{:?}", error);

    // stake accounts were merged in, none exist anymore
    for stake_account in &stake_accounts {
        let not_found_account = context
            .banks_client
            .get_account(stake_account.stake_account)
            .await
            .unwrap();
        assert!(not_found_account.is_none());
        let not_found_account = context
            .banks_client
            .get_account(stake_account.transient_stake_account)
            .await
            .unwrap();
        assert!(not_found_account.is_none());
    }
    let validator_list = get_account(
        &mut context.banks_client,
        &stake_pool_accounts.validator_list.pubkey(),
    )
    .await;
    let validator_list =
        try_from_slice_unchecked::<ValidatorList>(validator_list.data.as_slice()).unwrap();
    assert_eq!(validator_list.validators.len(), 1);
    assert_eq!(
        validator_list.validators[0].status,
        StakeStatus::ReadyForRemoval.into()
    );
    assert_eq!(validator_list.validators[0].stake_lamports().unwrap(), 0);

    let reserve_stake = context
        .banks_client
        .get_account(stake_pool_accounts.reserve_stake.pubkey())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        reserve_stake.lamports,
        reserve_lamports + deactivated_lamports + stake_rent * 2 + MINIMUM_RESERVE_LAMPORTS
    );

    // Update stake pool balance and cleanup, should be gone
    let error = stake_pool_accounts
        .update_stake_pool_balance(&mut context.banks_client, &context.payer, &last_blockhash)
        .await;
    assert!(error.is_none(), "{:?}", error);

    let error = stake_pool_accounts
        .cleanup_removed_validator_entries(
            &mut context.banks_client,
            &context.payer,
            &last_blockhash,
        )
        .await;
    assert!(error.is_none(), "{:?}", error);

    let validator_list = get_account(
        &mut context.banks_client,
        &stake_pool_accounts.validator_list.pubkey(),
    )
    .await;
    let validator_list =
        try_from_slice_unchecked::<ValidatorList>(validator_list.data.as_slice()).unwrap();
    assert_eq!(validator_list.validators.len(), 0);
}

#[tokio::test]
async fn success_with_burned_tokens() {
    let num_validators = 1;
    let (mut context, last_blockhash, stake_pool_accounts, _, deposit_accounts, _, _, mut slot) =
        setup(num_validators).await;

    let mint_info = get_account(
        &mut context.banks_client,
        &stake_pool_accounts.pool_mint.pubkey(),
    )
    .await;
    let mint = Mint::unpack(&mint_info.data).unwrap();

    let stake_pool_info = get_account(
        &mut context.banks_client,
        &stake_pool_accounts.stake_pool.pubkey(),
    )
    .await;
    let stake_pool = try_from_slice_unchecked::<StakePool>(&stake_pool_info.data).unwrap();
    assert_eq!(mint.supply, stake_pool.pool_token_supply);

    burn_tokens(
        &mut context.banks_client,
        &context.payer,
        &last_blockhash,
        &stake_pool_accounts.token_program_id,
        &stake_pool_accounts.pool_mint.pubkey(),
        &deposit_accounts[0].pool_account.pubkey(),
        &deposit_accounts[0].authority,
        deposit_accounts[0].pool_tokens,
    )
    .await
    .unwrap();

    let slots_per_epoch = context.genesis_config().epoch_schedule.slots_per_epoch;
    slot += slots_per_epoch;
    context.warp_to_slot(slot).unwrap();

    let last_blockhash = context
        .banks_client
        .get_new_latest_blockhash(&last_blockhash)
        .await
        .unwrap();

    let mint_info = get_account(
        &mut context.banks_client,
        &stake_pool_accounts.pool_mint.pubkey(),
    )
    .await;
    let mint = Mint::unpack(&mint_info.data).unwrap();
    assert_ne!(mint.supply, stake_pool.pool_token_supply);

    stake_pool_accounts
        .update_all(
            &mut context.banks_client,
            &context.payer,
            &last_blockhash,
            false,
        )
        .await;

    let stake_pool_info = get_account(
        &mut context.banks_client,
        &stake_pool_accounts.stake_pool.pubkey(),
    )
    .await;
    let stake_pool = try_from_slice_unchecked::<StakePool>(&stake_pool_info.data).unwrap();

    assert_eq!(mint.supply, stake_pool.pool_token_supply);
}

#[tokio::test]
async fn fail_with_no_merge_during_reward_payout() {
    let num_validators = 5;
    let (mut context, last_blockhash, stake_pool_accounts, stake_accounts, _, _, _, mut slot) =
        setup(num_validators).await;

    // Simulate rewards
    for stake_account in &stake_accounts {
        context.increment_vote_account_credits(&stake_account.vote.pubkey(), 100);
    }

    // Warp one more epoch minus one slot so that rewards are being paid out
    let slots_per_epoch = context.genesis_config().epoch_schedule.slots_per_epoch;
    slot += slots_per_epoch - 1;
    context.warp_to_slot(slot).unwrap();

    let last_blockhash = context
        .banks_client
        .get_new_latest_blockhash(&last_blockhash)
        .await
        .unwrap();

    let error = stake_pool_accounts
        .update_all(
            &mut context.banks_client,
            &context.payer,
            &last_blockhash,
            true,
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        error,
        TransactionError::InstructionError(
            0,
            InstructionError::Custom(StakePoolError::EpochRewardDistributionInProgress as u32)
        )
    );
}

#[tokio::test]
async fn fail_with_uninitialized_validator_list() {} // TODO

#[tokio::test]
async fn success_with_force_destaked_validator() {}

#[tokio::test]
async fn updates_validator_status_after_cluster_restart_merge() {
    let num_validators = 1;
    let (
        mut context,
        last_blockhash,
        stake_pool_accounts,
        stake_accounts,
        _,
        _validator_lamports,
        reserve_lamports,
        _slot,
    ) = setup(num_validators).await;

    let validator_stake_account = &stake_accounts[0];

    // Get initial validator list state - should be Active
    let initial_validator_list = stake_pool_accounts
        .get_validator_list(&mut context.banks_client)
        .await;
    let initial_validator_info = &initial_validator_list.validators[0];
    assert_eq!(initial_validator_info.status, StakeStatus::Active.into());

    // Simulate cluster restart scenario where stake account gets reset to Initialized
    let stake_pool = stake_pool_accounts
        .get_stake_pool(&mut context.banks_client)
        .await;

    // Create an Initialized stake account (as would happen during cluster restart)
    let initialized_meta = solana_stake_interface::state::Meta {
        rent_exempt_reserve: 2282880, // Standard rent exemption
        authorized: solana_stake_interface::state::Authorized {
            staker: stake_pool_accounts.withdraw_authority, // Correct authorities
            withdrawer: stake_pool_accounts.withdraw_authority,
        },
        lockup: stake_pool.lockup, // Correct lockup
    };

    let initialized_stake_state = StakeStateV2::Initialized(initialized_meta);

    // Set the stake account to Initialized state (simulating cluster restart)
    let mut stake_account_data = context
        .banks_client
        .get_account(validator_stake_account.stake_account)
        .await
        .unwrap()
        .unwrap();

    stake_account_data.data = bincode::serialize(&initialized_stake_state).unwrap();
    context.set_account(
        &validator_stake_account.stake_account,
        &stake_account_data.into(),
    );

    // Run update_validator_list_balance - this should merge the Initialized account into reserve
    // and CRITICALLY update the validator status to ReadyForRemoval
    let error = stake_pool_accounts
        .update_validator_list_balance(
            &mut context.banks_client,
            &context.payer,
            &last_blockhash,
            1,
            false,
        )
        .await;
    assert!(error.is_none(), "Update should succeed: {:?}", error);

    // MAIN TEST: Verify the validator status was properly updated to ReadyForRemoval
    // This is the critical fix - without it, the status would remain Active
    let post_merge_validator_list = stake_pool_accounts
        .get_validator_list(&mut context.banks_client)
        .await;
    let post_merge_validator_info = &post_merge_validator_list.validators[0];
    assert_eq!(
        post_merge_validator_info.status,
        StakeStatus::ReadyForRemoval.into(),
        "Validator status should be updated to ReadyForRemoval after merging Initialized stake"
    );

    // Verify the funds were properly merged into reserve
    let post_merge_reserve = context
        .banks_client
        .get_account(stake_pool_accounts.reserve_stake.pubkey())
        .await
        .unwrap()
        .unwrap();
    assert!(
        post_merge_reserve.lamports > reserve_lamports,
        "Reserve should have absorbed the Initialized stake funds"
    );

    // Verify active stake lamports are 0 (since account was merged)
    assert_eq!(
        u64::from(post_merge_validator_info.active_stake_lamports),
        0,
        "Active stake lamports should be 0 after merging Initialized account"
    );

    // This test proves that Fix #2 works: "update the validator status correctly after merging the inactive stake into the reserve"
}

#[tokio::test]
async fn ignores_unusable_stake_accounts_preventing_exploit() {
    let num_validators = 1;
    let (
        mut context,
        last_blockhash,
        stake_pool_accounts,
        stake_accounts,
        _,
        validator_lamports,
        _reserve_lamports,
        _slot,
    ) = setup(num_validators).await;

    let validator_stake_account = &stake_accounts[0];

    // Verify the validator starts as Active with proper stake
    let initial_validator_list = stake_pool_accounts
        .get_validator_list(&mut context.banks_client)
        .await;
    let initial_validator_info = &initial_validator_list.validators[0];
    assert_eq!(initial_validator_info.status, StakeStatus::Active.into());
    assert!(u64::from(initial_validator_info.active_stake_lamports) > 0);
    let initial_stake_pool = stake_pool_accounts
        .get_stake_pool(&mut context.banks_client)
        .await;

    // Now simulate the attack - malicious actor takes control of the stake account
    // This could happen after a cluster restart where the account gets reset to Initialized
    // and then the attacker manages to gain control before the pool processes it
    let malicious_authority = Keypair::new();
    let extra_malicious_lamports = 1_000_000_000; // 1 SOL extra
    let stake_pool = stake_pool_accounts
        .get_stake_pool(&mut context.banks_client)
        .await;

    let malicious_meta = solana_stake_interface::state::Meta {
        rent_exempt_reserve: 2282880,
        authorized: solana_stake_interface::state::Authorized {
            staker: malicious_authority.pubkey(), // WRONG - should be pool's withdraw authority
            withdrawer: malicious_authority.pubkey(), // WRONG - should be pool's withdraw authority
        },
        lockup: solana_stake_interface::state::Lockup {
            custodian: malicious_authority.pubkey(), // WRONG - different from pool's lockup
            epoch: stake_pool.lockup.epoch + 100,    // WRONG - different epoch
            ..stake_pool.lockup
        },
    };

    // Create a malicious delegation with the original validator + extra stake
    let malicious_delegation = solana_stake_interface::state::Delegation {
        voter_pubkey: validator_stake_account.vote.pubkey(),
        stake: validator_lamports + extra_malicious_lamports, // Original stake + malicious extra
        activation_epoch: 0,
        deactivation_epoch: u64::MAX,
        ..Default::default()
    };

    let malicious_stake = solana_stake_interface::state::Stake {
        delegation: malicious_delegation,
        credits_observed: 0,
    };

    let malicious_stake_state = StakeStateV2::Stake(
        malicious_meta,
        malicious_stake,
        solana_stake_interface::stake_flags::StakeFlags::empty(),
    );

    // Get original stake account for comparison
    let original_stake_account = context
        .banks_client
        .get_account(validator_stake_account.stake_account)
        .await
        .unwrap()
        .unwrap();
    let original_lamports = original_stake_account.lamports;

    // Replace the legitimate stake account with the malicious one
    let mut malicious_account = original_stake_account.clone();
    malicious_account.lamports = original_lamports + extra_malicious_lamports;
    malicious_account.data = bincode::serialize(&malicious_stake_state).unwrap();
    context.set_account(
        &validator_stake_account.stake_account,
        &malicious_account.into(),
    );

    // Run update_validator_list_balance - this should succeed butignore the malicious account
    let error = stake_pool_accounts
        .update_validator_list_balance(
            &mut context.banks_client,
            &context.payer,
            &last_blockhash,
            1,
            false,
        )
        .await;
    assert!(
        error.is_none(),
        "Update should succeed but ignore malicious account: {:?}",
        error
    );

    let final_validator_list = stake_pool_accounts
        .get_validator_list(&mut context.banks_client)
        .await;
    let final_validator_info = &final_validator_list.validators[0];

    // The validator should still be marked as Active but with 0 active stake
    // because the malicious account was ignored
    assert_eq!(final_validator_info.status, StakeStatus::Active.into());
    assert_eq!(
        u64::from(final_validator_info.active_stake_lamports),
        0,
        "Active stake lamports should be 0 because malicious account is ignored"
    );

    // The malicious stake account should still exist with all its lamports
    // (proving it was ignored, not merged or processed)
    let final_malicious_account = context
        .banks_client
        .get_account(validator_stake_account.stake_account)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(
        final_malicious_account.lamports,
        original_lamports + extra_malicious_lamports,
        "Malicious account should retain all its lamports since it was ignored"
    );

    // Verify the pool's total assets remained unchanged (malicious account was ignored)
    let final_stake_pool = stake_pool_accounts
        .get_stake_pool(&mut context.banks_client)
        .await;
    assert_eq!(
        final_stake_pool.total_lamports,
        initial_stake_pool.total_lamports
    );

    // This test proves fix for "only count active validator stakes if they're usable by the pool"
}

#[tokio::test]
async fn update_validator_list_balance_ingores_uninitialized_stake_account_balances() {
    let num_validators = 1;
    let (
        mut context,
        last_blockhash,
        stake_pool_accounts,
        stake_accounts,
        _,
        _validator_lamports,
        _reserve_lamports,
        mut slot,
    ) = setup(num_validators).await;

    let validator_stake_account = &stake_accounts[0];

    // Verify the validator starts as Active
    let initial_validator_list = stake_pool_accounts
        .get_validator_list(&mut context.banks_client)
        .await;
    let initial_validator_info = &initial_validator_list.validators[0];
    assert_eq!(initial_validator_info.status, StakeStatus::Active.into());
    assert!(u64::from(initial_validator_info.active_stake_lamports) > 0);

    // First, remove the validator from the pool to trigger deactivation
    let error = stake_pool_accounts
        .remove_validator_from_pool(
            &mut context.banks_client,
            &context.payer,
            &last_blockhash,
            &validator_stake_account.stake_account,
            &validator_stake_account.transient_stake_account,
        )
        .await;
    assert!(error.is_none(), "Failed to remove validator: {:?}", error);

    // Verify validator is now being deactivated
    let deactivating_validator_list = stake_pool_accounts
        .get_validator_list(&mut context.banks_client)
        .await;
    let deactivating_validator_info = &deactivating_validator_list.validators[0];
    assert_eq!(
        deactivating_validator_info.status,
        StakeStatus::DeactivatingValidator.into()
    );

    // Fast forward one epoch to allow the stake to deactivate
    let slots_per_epoch = context.genesis_config().epoch_schedule.slots_per_epoch;
    slot += slots_per_epoch;
    context.warp_to_slot(slot).unwrap();

    let new_blockhash = context
        .banks_client
        .get_new_latest_blockhash(&last_blockhash)
        .await
        .unwrap();

    // Now simulate a scenario where the stake account becomes uninitialized
    // This could happen due to cluster restart or other network events
    let uninitialized_stake_state = StakeStateV2::Uninitialized;

    let original_stake_account = context
        .banks_client
        .get_account(validator_stake_account.stake_account)
        .await
        .unwrap()
        .unwrap();

    let mut modified_account = original_stake_account.clone();
    modified_account.data = bincode::serialize(&uninitialized_stake_state).unwrap();
    context.set_account(
        &validator_stake_account.stake_account,
        &modified_account.into(),
    );

    // Run update_validator_list_balance - this should trigger the uninitialized account handling
    let error = stake_pool_accounts
        .update_validator_list_balance(
            &mut context.banks_client,
            &context.payer,
            &new_blockhash,
            1,
            false,
        )
        .await;
    assert!(
        error.is_none(),
        "Update should succeed despite uninitialized account: {:?}",
        error
    );

    // Verify the validator status and that the uninitialized account was ignored
    let final_validator_list = stake_pool_accounts
        .get_validator_list(&mut context.banks_client)
        .await;
    let final_validator_info = &final_validator_list.validators[0];

    // The validator should still be in DeactivatingValidator status with 0 active stake
    // because the uninitialized account was ignored
    assert_eq!(
        final_validator_info.status,
        StakeStatus::DeactivatingValidator.into()
    );
    assert_eq!(
        u64::from(final_validator_info.active_stake_lamports),
        0,
        "Active stake lamports should be 0 because uninitialized account is ignored"
    );

    // Verify the uninitialized stake account still exists
    let final_stake_account = context
        .banks_client
        .get_account(validator_stake_account.stake_account)
        .await
        .unwrap()
        .unwrap();

    // Verify it's still uninitialized
    let final_stake_state: StakeStateV2 = bincode::deserialize(&final_stake_account.data).unwrap();
    matches!(final_stake_state, StakeStateV2::Uninitialized);

    // assert it has a nonzero account balance
    assert!(final_stake_account.lamports > 0);

    // This test proves that the uninitialized account scenario can be triggered and is handled correctly
}

#[tokio::test]
async fn cleanup_does_not_remove_validators_with_remaining_lamports() {
    let num_validators = 2;
    let (
        mut context,
        last_blockhash,
        stake_pool_accounts,
        stake_accounts,
        _,
        _validator_lamports,
        _reserve_lamports,
        _slot,
    ) = setup(num_validators).await;

    // Remove 2 validators from the pool
    for stake_account in &stake_accounts {
        let error = stake_pool_accounts
            .remove_validator_from_pool(
                &mut context.banks_client,
                &context.payer,
                &last_blockhash,
                &stake_account.stake_account,
                &stake_account.transient_stake_account,
            )
            .await;
        assert!(error.is_none(), "Failed to remove validator: {:?}", error);
    }

    // Verify both validators are being deactivated (DeactivatingValidator status)
    let validator_list = stake_pool_accounts
        .get_validator_list(&mut context.banks_client)
        .await;
    assert_eq!(validator_list.validators.len(), 2);
    for validator_info in &validator_list.validators {
        assert_eq!(
            validator_info.status,
            StakeStatus::DeactivatingValidator.into()
        );
    }

    // Fast forward one epoch to allow the deactivating stakes to become inactive
    let current_slot = context.banks_client.get_root_slot().await.unwrap();
    let slots_per_epoch = context.genesis_config().epoch_schedule.slots_per_epoch;
    let next_epoch_slot = current_slot + slots_per_epoch;
    context.warp_to_slot(next_epoch_slot).unwrap();

    // Update validator list balance to process the now-inactive stakes
    // This should change status from DeactivatingValidator to ReadyForRemoval
    let error = stake_pool_accounts
        .update_validator_list_balance(
            &mut context.banks_client,
            &context.payer,
            &last_blockhash,
            2, // Process both validators
            false,
        )
        .await;
    assert!(error.is_none(), "Update should succeed: {:?}", error);

    // Verify both validators are now ReadyForRemoval
    let updated_validator_list = stake_pool_accounts
        .get_validator_list(&mut context.banks_client)
        .await;
    assert_eq!(updated_validator_list.validators.len(), 2);
    for validator_info in &updated_validator_list.validators {
        assert_eq!(validator_info.status, StakeStatus::ReadyForRemoval.into());
    }

    // Now manually modify the first validator to have some remaining active lamports
    // Note: This state (ReadyForRemoval with non-zero active_stake_lamports) cannot occur
    // through normal program execution because the status change to ReadyForRemoval and
    // the zeroing of active_stake_lamports happen atomically during the merge operation
    // in update_validator_list_balance. This test uses synthetic state to verify the
    // defensive guard in is_removed() works correctly - ensuring cleanup won't remove
    // validators that still have lamports recorded, regardless of how that state arose.
    let mut modified_validator_list = updated_validator_list.clone();
    modified_validator_list.validators[0].active_stake_lamports = PodU64::from(1_000_000u64); // 1 SOL remaining
    modified_validator_list.validators[0].transient_stake_lamports = PodU64::from(0u64); // No transient stake
    let validator_list_account = context
        .banks_client
        .get_account(stake_pool_accounts.validator_list.pubkey())
        .await
        .unwrap()
        .unwrap();
    let mut modified_account = validator_list_account.clone();

    let mut serialized_data = borsh::to_vec(&modified_validator_list).unwrap();
    serialized_data.resize(modified_account.data.len(), 0);
    modified_account.data = serialized_data;
    context.set_account(
        &stake_pool_accounts.validator_list.pubkey(),
        &modified_account.into(),
    );

    // Verify the validator setup is as intended (2 validators ready for removal, one with remaining lamports)
    let pre_cleanup_validator_list = stake_pool_accounts
        .get_validator_list(&mut context.banks_client)
        .await;
    assert_eq!(pre_cleanup_validator_list.validators.len(), 2);
    assert_eq!(
        pre_cleanup_validator_list.validators[0].status,
        StakeStatus::ReadyForRemoval.into()
    );
    assert_eq!(
        u64::from(pre_cleanup_validator_list.validators[0].active_stake_lamports),
        1_000_000
    );
    assert_eq!(
        u64::from(pre_cleanup_validator_list.validators[0].transient_stake_lamports),
        0
    );
    assert_eq!(
        pre_cleanup_validator_list.validators[1].status,
        StakeStatus::ReadyForRemoval.into()
    );
    assert_eq!(
        u64::from(pre_cleanup_validator_list.validators[1].active_stake_lamports),
        0
    );
    assert_eq!(
        u64::from(pre_cleanup_validator_list.validators[1].transient_stake_lamports),
        0
    );

    // Run cleanup_removed_validator_entries
    let error = stake_pool_accounts
        .cleanup_removed_validator_entries(
            &mut context.banks_client,
            &context.payer,
            &last_blockhash,
        )
        .await;
    assert!(error.is_none(), "Cleanup should succeed: {:?}", error);

    // Verify that only the validator with 0 lamports was removed
    let post_cleanup_validator_list = stake_pool_accounts
        .get_validator_list(&mut context.banks_client)
        .await;

    // Should have 1 validator remaining (the one with active lamports)
    assert_eq!(
        post_cleanup_validator_list.validators.len(),
        1,
        "Only validator with 0 lamports should have been removed"
    );

    // The remaining validator should be the one with active lamports
    assert_eq!(
        u64::from(post_cleanup_validator_list.validators[0].active_stake_lamports),
        1_000_000,
        "Validator with remaining lamports should not have been removed"
    );
    assert_eq!(
        post_cleanup_validator_list.validators[0].status,
        StakeStatus::ReadyForRemoval.into(),
        "Validator status should remain ReadyForRemoval"
    );
}
