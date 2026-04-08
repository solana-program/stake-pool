#![allow(clippy::arithmetic_side_effects)]
#![cfg(feature = "test-sbf")]

mod helpers;

use {
    helpers::*,
    solana_program::{
        borsh1::try_from_slice_unchecked,
        hash::Hash,
        instruction::InstructionError,
        program_pack::Pack,
        pubkey::Pubkey,
        stake,
    },
    solana_program_test::*,
    solana_sdk::{
        signature::{Keypair, Signer},
        transaction::{Transaction, TransactionError},
        transport::TransportError,
    },
    spl_stake_pool::{
        error::StakePoolError, id, instruction, state, MINIMUM_RESERVE_LAMPORTS,
    },
};

async fn setup(
    max_validators: u32,
) -> (BanksClient, Keypair, Hash, StakePoolAccounts) {
    let (mut banks_client, payer, recent_blockhash) = program_test().start().await;
    let rent = banks_client.get_rent().await.unwrap();
    let stake_rent = rent.minimum_balance(std::mem::size_of::<stake::state::StakeStateV2>());
    let current_minimum_delegation =
        stake_pool_get_minimum_delegation(&mut banks_client, &payer, &recent_blockhash).await;
    let minimum_for_validator = stake_rent + current_minimum_delegation;

    let stake_pool_accounts = StakePoolAccounts {
        max_validators,
        ..Default::default()
    };
    stake_pool_accounts
        .initialize_stake_pool(
            &mut banks_client,
            &payer,
            &recent_blockhash,
            MINIMUM_RESERVE_LAMPORTS + minimum_for_validator,
        )
        .await
        .unwrap();

    (banks_client, payer, recent_blockhash, stake_pool_accounts)
}

#[tokio::test]
async fn success_realloc_validator_list() {
    let (mut banks_client, payer, recent_blockhash, stake_pool_accounts) = setup(5).await;

    let instruction = instruction::realloc_validator_list(
        &id(),
        &stake_pool_accounts.stake_pool.pubkey(),
        &stake_pool_accounts.manager.pubkey(),
        &stake_pool_accounts.validator_list.pubkey(),
        &payer.pubkey(),
        10,
    );
    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&payer.pubkey()),
        &[&payer, &stake_pool_accounts.manager],
        recent_blockhash,
    );
    banks_client
        .process_transaction(transaction)
        .await
        .unwrap();

    // Verify the validator list was updated
    let validator_list_account = get_account(
        &mut banks_client,
        &stake_pool_accounts.validator_list.pubkey(),
    )
    .await;
    let validator_list =
        try_from_slice_unchecked::<state::ValidatorList>(validator_list_account.data.as_slice())
            .unwrap();
    assert_eq!(validator_list.header.max_validators, 10);

    // Verify account size matches expected size for 10 validators
    let expected_size = std::mem::size_of::<state::ValidatorListHeader>()
        + 4
        + 10 * state::ValidatorStakeInfo::LEN;
    assert_eq!(validator_list_account.data.len(), expected_size);
}

#[tokio::test]
async fn fail_realloc_with_smaller_max() {
    let (banks_client, payer, recent_blockhash, stake_pool_accounts) = setup(10).await;

    let instruction = instruction::realloc_validator_list(
        &id(),
        &stake_pool_accounts.stake_pool.pubkey(),
        &stake_pool_accounts.manager.pubkey(),
        &stake_pool_accounts.validator_list.pubkey(),
        &payer.pubkey(),
        5,
    );
    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&payer.pubkey()),
        &[&payer, &stake_pool_accounts.manager],
        recent_blockhash,
    );
    let transaction_error: TransportError = banks_client
        .process_transaction(transaction)
        .await
        .err()
        .unwrap()
        .into();

    match transaction_error {
        TransportError::TransactionError(TransactionError::InstructionError(
            _,
            InstructionError::InvalidArgument,
        )) => {}
        _ => panic!("Wrong error: expected InvalidArgument, got {:?}", transaction_error),
    }
}

#[tokio::test]
async fn fail_realloc_with_same_max() {
    let (banks_client, payer, recent_blockhash, stake_pool_accounts) = setup(5).await;

    let instruction = instruction::realloc_validator_list(
        &id(),
        &stake_pool_accounts.stake_pool.pubkey(),
        &stake_pool_accounts.manager.pubkey(),
        &stake_pool_accounts.validator_list.pubkey(),
        &payer.pubkey(),
        5,
    );
    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&payer.pubkey()),
        &[&payer, &stake_pool_accounts.manager],
        recent_blockhash,
    );
    let transaction_error: TransportError = banks_client
        .process_transaction(transaction)
        .await
        .err()
        .unwrap()
        .into();

    match transaction_error {
        TransportError::TransactionError(TransactionError::InstructionError(
            _,
            InstructionError::InvalidArgument,
        )) => {}
        _ => panic!("Wrong error: expected InvalidArgument, got {:?}", transaction_error),
    }
}

#[tokio::test]
async fn fail_realloc_wrong_manager() {
    let (banks_client, payer, recent_blockhash, stake_pool_accounts) = setup(5).await;

    let wrong_manager = Keypair::new();
    let instruction = instruction::realloc_validator_list(
        &id(),
        &stake_pool_accounts.stake_pool.pubkey(),
        &wrong_manager.pubkey(),
        &stake_pool_accounts.validator_list.pubkey(),
        &payer.pubkey(),
        10,
    );
    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&payer.pubkey()),
        &[&payer, &wrong_manager],
        recent_blockhash,
    );
    let transaction_error: TransportError = banks_client
        .process_transaction(transaction)
        .await
        .err()
        .unwrap()
        .into();

    match transaction_error {
        TransportError::TransactionError(TransactionError::InstructionError(
            _,
            InstructionError::Custom(error_index),
        )) => {
            let program_error = StakePoolError::WrongManager as u32;
            assert_eq!(error_index, program_error);
        }
        _ => panic!("Wrong error: expected WrongManager, got {:?}", transaction_error),
    }
}

#[tokio::test]
async fn fail_realloc_wrong_validator_list() {
    let (banks_client, payer, recent_blockhash, stake_pool_accounts) = setup(5).await;

    let wrong_validator_list = Pubkey::new_unique();
    let instruction = instruction::realloc_validator_list(
        &id(),
        &stake_pool_accounts.stake_pool.pubkey(),
        &stake_pool_accounts.manager.pubkey(),
        &wrong_validator_list,
        &payer.pubkey(),
        10,
    );
    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&payer.pubkey()),
        &[&payer, &stake_pool_accounts.manager],
        recent_blockhash,
    );
    let transaction_error: TransportError = banks_client
        .process_transaction(transaction)
        .await
        .err()
        .unwrap()
        .into();

    match transaction_error {
        TransportError::TransactionError(TransactionError::InstructionError(
            _,
            InstructionError::Custom(error_index),
        )) => {
            let program_error = StakePoolError::InvalidValidatorStakeList as u32;
            assert_eq!(error_index, program_error);
        }
        _ => panic!(
            "Wrong error: expected InvalidValidatorStakeList, got {:?}",
            transaction_error
        ),
    }
}

#[tokio::test]
async fn success_realloc_preserves_existing_validators() {
    let (mut banks_client, payer, recent_blockhash, stake_pool_accounts) = setup(5).await;

    // Add a validator to the pool
    let validator_stake =
        simple_add_validator_to_pool(&mut banks_client, &payer, &recent_blockhash, &stake_pool_accounts, None).await;

    // Verify the validator is in the list before realloc
    let validator_list_account = get_account(
        &mut banks_client,
        &stake_pool_accounts.validator_list.pubkey(),
    )
    .await;
    let validator_list_before =
        try_from_slice_unchecked::<state::ValidatorList>(validator_list_account.data.as_slice())
            .unwrap();
    assert_eq!(validator_list_before.validators.len(), 1);
    assert_eq!(
        validator_list_before.validators[0].vote_account_address,
        validator_stake.vote.pubkey()
    );

    // Realloc to a larger size
    let instruction = instruction::realloc_validator_list(
        &id(),
        &stake_pool_accounts.stake_pool.pubkey(),
        &stake_pool_accounts.manager.pubkey(),
        &stake_pool_accounts.validator_list.pubkey(),
        &payer.pubkey(),
        10,
    );
    // Need a fresh blockhash since we already submitted transactions
    let recent_blockhash = banks_client.get_latest_blockhash().await.unwrap();
    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&payer.pubkey()),
        &[&payer, &stake_pool_accounts.manager],
        recent_blockhash,
    );
    banks_client
        .process_transaction(transaction)
        .await
        .unwrap();

    // Verify the validator is still present and the max was updated
    let validator_list_account = get_account(
        &mut banks_client,
        &stake_pool_accounts.validator_list.pubkey(),
    )
    .await;
    let validator_list_after =
        try_from_slice_unchecked::<state::ValidatorList>(validator_list_account.data.as_slice())
            .unwrap();
    assert_eq!(validator_list_after.header.max_validators, 10);
    assert_eq!(validator_list_after.validators.len(), 1);
    assert_eq!(
        validator_list_after.validators[0].vote_account_address,
        validator_stake.vote.pubkey()
    );
}
