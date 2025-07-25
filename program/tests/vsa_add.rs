#![allow(clippy::arithmetic_side_effects)]
#![cfg(feature = "test-sbf")]

mod helpers;

use {
    bincode::deserialize,
    helpers::*,
    solana_program::{
        borsh1::try_from_slice_unchecked,
        hash::Hash,
        instruction::{AccountMeta, Instruction, InstructionError},
        pubkey::Pubkey,
        sysvar,
    },
    solana_program_test::*,
    solana_sdk::{
        signature::{Keypair, Signer},
        transaction::{Transaction, TransactionError},
        transport::TransportError,
    },
    solana_stake_interface as stake,
    solana_system_interface::program as system_program,
    spl_stake_pool::{
        error::StakePoolError, find_stake_program_address, id, instruction, state,
        MINIMUM_RESERVE_LAMPORTS,
    },
};

async fn setup(
    num_validators: u64,
) -> (
    BanksClient,
    Keypair,
    Hash,
    StakePoolAccounts,
    ValidatorStakeAccount,
) {
    let (mut banks_client, payer, recent_blockhash) = program_test().start().await;
    let rent = banks_client.get_rent().await.unwrap();
    let stake_rent = rent.minimum_balance(std::mem::size_of::<stake::state::StakeStateV2>());
    let current_minimum_delegation =
        stake_pool_get_minimum_delegation(&mut banks_client, &payer, &recent_blockhash).await;
    let minimum_for_validator = stake_rent + current_minimum_delegation;

    let stake_pool_accounts = StakePoolAccounts::default();
    stake_pool_accounts
        .initialize_stake_pool(
            &mut banks_client,
            &payer,
            &recent_blockhash,
            MINIMUM_RESERVE_LAMPORTS + num_validators * minimum_for_validator,
        )
        .await
        .unwrap();

    let validator_stake =
        ValidatorStakeAccount::new(&stake_pool_accounts.stake_pool.pubkey(), None, 0);
    create_vote(
        &mut banks_client,
        &payer,
        &recent_blockhash,
        &validator_stake.validator,
        &validator_stake.vote,
    )
    .await;

    (
        banks_client,
        payer,
        recent_blockhash,
        stake_pool_accounts,
        validator_stake,
    )
}

#[tokio::test]
async fn success() {
    let (mut banks_client, payer, recent_blockhash, stake_pool_accounts, validator_stake) =
        setup(1).await;

    let error = stake_pool_accounts
        .add_validator_to_pool(
            &mut banks_client,
            &payer,
            &recent_blockhash,
            &validator_stake.stake_account,
            &validator_stake.vote.pubkey(),
            validator_stake.validator_stake_seed,
        )
        .await;
    assert!(error.is_none(), "{:?}", error);

    // Check if validator account was added to the list
    let validator_list = get_account(
        &mut banks_client,
        &stake_pool_accounts.validator_list.pubkey(),
    )
    .await;
    let validator_list =
        try_from_slice_unchecked::<state::ValidatorList>(validator_list.data.as_slice()).unwrap();
    let rent = banks_client.get_rent().await.unwrap();
    let stake_rent = rent.minimum_balance(std::mem::size_of::<stake::state::StakeStateV2>());
    let current_minimum_delegation =
        stake_pool_get_minimum_delegation(&mut banks_client, &payer, &recent_blockhash).await;
    assert_eq!(
        validator_list,
        state::ValidatorList {
            header: state::ValidatorListHeader {
                account_type: state::AccountType::ValidatorList,
                max_validators: stake_pool_accounts.max_validators,
            },
            validators: vec![state::ValidatorStakeInfo {
                status: state::StakeStatus::Active.into(),
                vote_account_address: validator_stake.vote.pubkey(),
                last_update_epoch: 0.into(),
                active_stake_lamports: (stake_rent + current_minimum_delegation).into(),
                transient_stake_lamports: 0.into(),
                transient_seed_suffix: 0.into(),
                unused: 0.into(),
                validator_seed_suffix: validator_stake
                    .validator_stake_seed
                    .map(|s| s.get())
                    .unwrap_or(0)
                    .into(),
            }]
        }
    );

    // Check stake account existence and authority
    let stake = get_account(&mut banks_client, &validator_stake.stake_account).await;
    let stake_state = deserialize::<stake::state::StakeStateV2>(&stake.data).unwrap();
    match stake_state {
        stake::state::StakeStateV2::Stake(meta, _, _) => {
            assert_eq!(
                &meta.authorized.staker,
                &stake_pool_accounts.withdraw_authority
            );
            assert_eq!(
                &meta.authorized.withdrawer,
                &stake_pool_accounts.withdraw_authority
            );
        }
        _ => panic!(),
    }
}

#[tokio::test]
async fn fail_with_wrong_validator_list_account() {
    let (banks_client, payer, recent_blockhash, stake_pool_accounts, validator_stake) =
        setup(1).await;

    let wrong_validator_list = Keypair::new();

    let mut transaction = Transaction::new_with_payer(
        &[instruction::add_validator_to_pool(
            &id(),
            &stake_pool_accounts.stake_pool.pubkey(),
            &stake_pool_accounts.staker.pubkey(),
            &stake_pool_accounts.reserve_stake.pubkey(),
            &stake_pool_accounts.withdraw_authority,
            &wrong_validator_list.pubkey(),
            &validator_stake.stake_account,
            &validator_stake.vote.pubkey(),
            validator_stake.validator_stake_seed,
        )],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[&payer, &stake_pool_accounts.staker], recent_blockhash);
    let transaction_error = banks_client
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
        _ => panic!("Wrong error occurs while try to add validator stake address with wrong validator stake list account"),
    }
}

#[tokio::test]
async fn fail_double_add() {
    let (mut banks_client, payer, recent_blockhash, stake_pool_accounts, validator_stake) =
        setup(2).await;

    stake_pool_accounts
        .add_validator_to_pool(
            &mut banks_client,
            &payer,
            &recent_blockhash,
            &validator_stake.stake_account,
            &validator_stake.vote.pubkey(),
            validator_stake.validator_stake_seed,
        )
        .await;

    let latest_blockhash = banks_client.get_latest_blockhash().await.unwrap();

    let transaction_error = stake_pool_accounts
        .add_validator_to_pool(
            &mut banks_client,
            &payer,
            &latest_blockhash,
            &validator_stake.stake_account,
            &validator_stake.vote.pubkey(),
            validator_stake.validator_stake_seed,
        )
        .await
        .unwrap();

    match transaction_error {
        TransportError::TransactionError(TransactionError::InstructionError(
            _,
            InstructionError::Custom(error_index),
        )) => {
            let program_error = StakePoolError::ValidatorAlreadyAdded as u32;
            assert_eq!(error_index, program_error);
        }
        _ => panic!("Wrong error occurs while try to add already added validator stake account"),
    }
}

#[tokio::test]
async fn fail_wrong_staker() {
    let (banks_client, payer, recent_blockhash, stake_pool_accounts, validator_stake) =
        setup(1).await;

    let malicious = Keypair::new();

    let mut transaction = Transaction::new_with_payer(
        &[instruction::add_validator_to_pool(
            &id(),
            &stake_pool_accounts.stake_pool.pubkey(),
            &malicious.pubkey(),
            &stake_pool_accounts.reserve_stake.pubkey(),
            &stake_pool_accounts.withdraw_authority,
            &stake_pool_accounts.validator_list.pubkey(),
            &validator_stake.stake_account,
            &validator_stake.vote.pubkey(),
            validator_stake.validator_stake_seed,
        )],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[&payer, &malicious], recent_blockhash);
    let transaction_error = banks_client
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
            let program_error = StakePoolError::WrongStaker as u32;
            assert_eq!(error_index, program_error);
        }
        _ => panic!("Wrong error occurs while malicious try to add validator stake account"),
    }
}

#[tokio::test]
async fn fail_without_signature() {
    let (banks_client, payer, recent_blockhash, stake_pool_accounts, validator_stake) =
        setup(1).await;

    let accounts = vec![
        AccountMeta::new(stake_pool_accounts.stake_pool.pubkey(), false),
        AccountMeta::new_readonly(stake_pool_accounts.staker.pubkey(), false),
        AccountMeta::new(payer.pubkey(), false),
        AccountMeta::new_readonly(stake_pool_accounts.withdraw_authority, false),
        AccountMeta::new(stake_pool_accounts.validator_list.pubkey(), false),
        AccountMeta::new(validator_stake.stake_account, false),
        AccountMeta::new(validator_stake.vote.pubkey(), false),
        AccountMeta::new_readonly(sysvar::rent::id(), false),
        AccountMeta::new_readonly(sysvar::clock::id(), false),
        AccountMeta::new_readonly(sysvar::stake_history::id(), false),
        #[allow(deprecated)]
        AccountMeta::new_readonly(stake::config::id(), false),
        AccountMeta::new_readonly(system_program::id(), false),
        AccountMeta::new_readonly(stake::program::id(), false),
    ];
    let instruction = Instruction {
        program_id: id(),
        accounts,
        data: borsh::to_vec(&instruction::StakePoolInstruction::AddValidatorToPool(
            validator_stake
                .validator_stake_seed
                .map(|s| s.get())
                .unwrap_or(0),
        ))
        .unwrap(),
    };

    let mut transaction = Transaction::new_with_payer(&[instruction], Some(&payer.pubkey()));
    transaction.sign(&[&payer], recent_blockhash);
    let transaction_error = banks_client
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
            let program_error = StakePoolError::SignatureMissing as u32;
            assert_eq!(error_index, program_error);
        }
        _ => panic!("Wrong error occurs while malicious try to add validator stake account without signing transaction"),
    }
}

#[tokio::test]
async fn fail_with_wrong_stake_program_id() {
    let (banks_client, payer, recent_blockhash, stake_pool_accounts, validator_stake) =
        setup(1).await;

    let wrong_stake_program = Pubkey::new_unique();
    let accounts = vec![
        AccountMeta::new(stake_pool_accounts.stake_pool.pubkey(), false),
        AccountMeta::new_readonly(stake_pool_accounts.staker.pubkey(), true),
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new_readonly(stake_pool_accounts.withdraw_authority, false),
        AccountMeta::new(stake_pool_accounts.validator_list.pubkey(), false),
        AccountMeta::new(validator_stake.stake_account, false),
        AccountMeta::new(validator_stake.vote.pubkey(), false),
        AccountMeta::new_readonly(sysvar::rent::id(), false),
        AccountMeta::new_readonly(sysvar::clock::id(), false),
        AccountMeta::new_readonly(sysvar::stake_history::id(), false),
        #[allow(deprecated)]
        AccountMeta::new_readonly(stake::config::id(), false),
        AccountMeta::new_readonly(system_program::id(), false),
        AccountMeta::new_readonly(wrong_stake_program, false),
    ];
    let instruction = Instruction {
        program_id: id(),
        accounts,
        data: borsh::to_vec(&instruction::StakePoolInstruction::AddValidatorToPool(
            validator_stake
                .validator_stake_seed
                .map(|s| s.get())
                .unwrap_or(0),
        ))
        .unwrap(),
    };
    let mut transaction = Transaction::new_with_payer(&[instruction], Some(&payer.pubkey()));
    transaction.sign(&[&payer, &stake_pool_accounts.staker], recent_blockhash);
    let transaction_error = banks_client
        .process_transaction(transaction)
        .await
        .err()
        .unwrap()
        .into();

    match transaction_error {
        TransportError::TransactionError(TransactionError::InstructionError(_, error)) => {
            assert_eq!(error, InstructionError::IncorrectProgramId);
        }
        _ => panic!(
            "Wrong error occurs while try to add validator stake account with wrong stake program ID"
        ),
    }
}

#[tokio::test]
async fn fail_with_wrong_system_program_id() {
    let (banks_client, payer, recent_blockhash, stake_pool_accounts, validator_stake) =
        setup(1).await;

    let wrong_system_program = Pubkey::new_unique();

    let accounts = vec![
        AccountMeta::new(stake_pool_accounts.stake_pool.pubkey(), false),
        AccountMeta::new_readonly(stake_pool_accounts.staker.pubkey(), true),
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new_readonly(stake_pool_accounts.withdraw_authority, false),
        AccountMeta::new(stake_pool_accounts.validator_list.pubkey(), false),
        AccountMeta::new(validator_stake.stake_account, false),
        AccountMeta::new(validator_stake.vote.pubkey(), false),
        AccountMeta::new_readonly(sysvar::rent::id(), false),
        AccountMeta::new_readonly(sysvar::clock::id(), false),
        AccountMeta::new_readonly(sysvar::stake_history::id(), false),
        #[allow(deprecated)]
        AccountMeta::new_readonly(stake::config::id(), false),
        AccountMeta::new_readonly(wrong_system_program, false),
        AccountMeta::new_readonly(stake::program::id(), false),
    ];
    let instruction = Instruction {
        program_id: id(),
        accounts,
        data: borsh::to_vec(&instruction::StakePoolInstruction::AddValidatorToPool(
            validator_stake
                .validator_stake_seed
                .map(|s| s.get())
                .unwrap_or(0),
        ))
        .unwrap(),
    };
    let mut transaction = Transaction::new_with_payer(&[instruction], Some(&payer.pubkey()));
    transaction.sign(&[&payer, &stake_pool_accounts.staker], recent_blockhash);
    let transaction_error = banks_client
        .process_transaction(transaction)
        .await
        .err()
        .unwrap()
        .into();

    match transaction_error {
        TransportError::TransactionError(TransactionError::InstructionError(_, error)) => {
            assert_eq!(error, InstructionError::IncorrectProgramId);
        }
        _ => panic!(
            "Wrong error occurs while try to add validator stake account with wrong stake program ID"
        ),
    }
}

#[tokio::test]
async fn fail_add_too_many_validator_stake_accounts() {
    let (mut banks_client, payer, recent_blockhash) = program_test().start().await;
    let rent = banks_client.get_rent().await.unwrap();
    let stake_rent = rent.minimum_balance(std::mem::size_of::<stake::state::StakeStateV2>());
    let current_minimum_delegation =
        stake_pool_get_minimum_delegation(&mut banks_client, &payer, &recent_blockhash).await;
    let minimum_for_validator = stake_rent + current_minimum_delegation;

    let stake_pool_accounts = StakePoolAccounts {
        max_validators: 1,
        ..Default::default()
    };
    stake_pool_accounts
        .initialize_stake_pool(
            &mut banks_client,
            &payer,
            &recent_blockhash,
            MINIMUM_RESERVE_LAMPORTS + 2 * minimum_for_validator,
        )
        .await
        .unwrap();

    let validator_stake =
        ValidatorStakeAccount::new(&stake_pool_accounts.stake_pool.pubkey(), None, 0);
    create_vote(
        &mut banks_client,
        &payer,
        &recent_blockhash,
        &validator_stake.validator,
        &validator_stake.vote,
    )
    .await;

    let error = stake_pool_accounts
        .add_validator_to_pool(
            &mut banks_client,
            &payer,
            &recent_blockhash,
            &validator_stake.stake_account,
            &validator_stake.vote.pubkey(),
            validator_stake.validator_stake_seed,
        )
        .await;
    assert!(error.is_none(), "{:?}", error);

    let validator_stake =
        ValidatorStakeAccount::new(&stake_pool_accounts.stake_pool.pubkey(), None, 0);
    create_vote(
        &mut banks_client,
        &payer,
        &recent_blockhash,
        &validator_stake.validator,
        &validator_stake.vote,
    )
    .await;
    let error = stake_pool_accounts
        .add_validator_to_pool(
            &mut banks_client,
            &payer,
            &recent_blockhash,
            &validator_stake.stake_account,
            &validator_stake.vote.pubkey(),
            validator_stake.validator_stake_seed,
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        error,
        TransactionError::InstructionError(0, InstructionError::AccountDataTooSmall),
    );
}

#[tokio::test]
async fn fail_with_unupdated_stake_pool() {} // TODO

#[tokio::test]
async fn fail_with_uninitialized_validator_list_account() {} // TODO

#[tokio::test]
async fn fail_on_non_vote_account() {
    let (mut banks_client, payer, recent_blockhash, stake_pool_accounts, _) = setup(1).await;

    let validator = Pubkey::new_unique();
    let (stake_account, _) = find_stake_program_address(
        &id(),
        &validator,
        &stake_pool_accounts.stake_pool.pubkey(),
        None,
    );

    let error = stake_pool_accounts
        .add_validator_to_pool(
            &mut banks_client,
            &payer,
            &recent_blockhash,
            &stake_account,
            &validator,
            None,
        )
        .await
        .unwrap()
        .unwrap();

    assert_eq!(
        error,
        TransactionError::InstructionError(0, InstructionError::IncorrectProgramId,)
    );
}

#[tokio::test]
async fn fail_on_incorrectly_derived_stake_account() {
    let (mut banks_client, payer, recent_blockhash, stake_pool_accounts, validator_stake) =
        setup(1).await;

    let bad_stake_account = Pubkey::new_unique();
    let error = stake_pool_accounts
        .add_validator_to_pool(
            &mut banks_client,
            &payer,
            &recent_blockhash,
            &bad_stake_account,
            &validator_stake.vote.pubkey(),
            validator_stake.validator_stake_seed,
        )
        .await
        .unwrap()
        .unwrap();

    assert_eq!(
        error,
        TransactionError::InstructionError(
            0,
            InstructionError::Custom(StakePoolError::InvalidStakeAccountAddress as u32),
        )
    );
}

#[tokio::test]
async fn success_with_lamports_in_account() {
    let (mut banks_client, payer, recent_blockhash, stake_pool_accounts, validator_stake) =
        setup(1).await;

    transfer(
        &mut banks_client,
        &payer,
        &recent_blockhash,
        &validator_stake.stake_account,
        1_000_000,
    )
    .await;

    let error = stake_pool_accounts
        .add_validator_to_pool(
            &mut banks_client,
            &payer,
            &recent_blockhash,
            &validator_stake.stake_account,
            &validator_stake.vote.pubkey(),
            validator_stake.validator_stake_seed,
        )
        .await;
    assert!(error.is_none(), "{:?}", error);

    // Check stake account existence and authority
    let stake = get_account(&mut banks_client, &validator_stake.stake_account).await;
    let stake_state = deserialize::<stake::state::StakeStateV2>(&stake.data).unwrap();
    match stake_state {
        stake::state::StakeStateV2::Stake(meta, _, _) => {
            assert_eq!(
                &meta.authorized.staker,
                &stake_pool_accounts.withdraw_authority
            );
            assert_eq!(
                &meta.authorized.withdrawer,
                &stake_pool_accounts.withdraw_authority
            );
        }
        _ => panic!(),
    }
}

#[tokio::test]
async fn fail_with_not_enough_reserve_lamports() {
    let (mut banks_client, payer, recent_blockhash, stake_pool_accounts, validator_stake) =
        setup(0).await;

    let error = stake_pool_accounts
        .add_validator_to_pool(
            &mut banks_client,
            &payer,
            &recent_blockhash,
            &validator_stake.stake_account,
            &validator_stake.vote.pubkey(),
            validator_stake.validator_stake_seed,
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        error,
        TransactionError::InstructionError(0, InstructionError::InsufficientFunds)
    );
}

#[tokio::test]
async fn fail_with_wrong_reserve() {
    let (banks_client, payer, recent_blockhash, stake_pool_accounts, validator_stake) =
        setup(1).await;

    let wrong_reserve = Pubkey::new_unique();

    let mut transaction = Transaction::new_with_payer(
        &[instruction::add_validator_to_pool(
            &id(),
            &stake_pool_accounts.stake_pool.pubkey(),
            &stake_pool_accounts.staker.pubkey(),
            &wrong_reserve,
            &stake_pool_accounts.withdraw_authority,
            &stake_pool_accounts.validator_list.pubkey(),
            &validator_stake.stake_account,
            &validator_stake.vote.pubkey(),
            validator_stake.validator_stake_seed,
        )],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[&payer, &stake_pool_accounts.staker], recent_blockhash);
    let transaction_error = banks_client
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
            let program_error = StakePoolError::InvalidProgramAddress as u32;
            assert_eq!(error_index, program_error);
        }
        _ => panic!("Wrong error occurs while try to add validator stake address with wrong validator stake list account"),
    }
}

#[tokio::test]
async fn fail_with_draining_reserve() {
    let (mut banks_client, payer, recent_blockhash) = program_test().start().await;
    let current_minimum_delegation =
        stake_pool_get_minimum_delegation(&mut banks_client, &payer, &recent_blockhash).await;

    let stake_pool_accounts = StakePoolAccounts::default();
    stake_pool_accounts
        .initialize_stake_pool(
            &mut banks_client,
            &payer,
            &recent_blockhash,
            current_minimum_delegation, // add exactly enough for a validator
        )
        .await
        .unwrap();

    let validator_stake =
        ValidatorStakeAccount::new(&stake_pool_accounts.stake_pool.pubkey(), None, 0);
    create_vote(
        &mut banks_client,
        &payer,
        &recent_blockhash,
        &validator_stake.validator,
        &validator_stake.vote,
    )
    .await;

    let error = stake_pool_accounts
        .add_validator_to_pool(
            &mut banks_client,
            &payer,
            &recent_blockhash,
            &validator_stake.stake_account,
            &validator_stake.vote.pubkey(),
            validator_stake.validator_stake_seed,
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        error,
        TransactionError::InstructionError(0, InstructionError::InsufficientFunds),
    );
}
