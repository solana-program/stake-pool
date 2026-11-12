use {
    bincode::deserialize,
    solana_account_decoder::UiAccountEncoding,
    solana_client::{
        client_error::ClientError,
        rpc_client::RpcClient,
        rpc_config::{RpcAccountInfoConfig, RpcProgramAccountsConfig},
        rpc_filter::{Memcmp, MemcmpEncodedBytes, RpcFilterType},
    },
    solana_compute_budget_interface::ComputeBudgetInstruction,
    solana_message::Message,
    solana_program::{
        borsh1::try_from_slice_unchecked, hash::Hash, instruction::Instruction, program_pack::Pack,
        pubkey::Pubkey,
    },
    solana_sdk::transaction::Transaction,
    solana_stake_interface as stake,
    spl_stake_pool::{
        find_ephemeral_stake_program_address, find_withdraw_authority_program_address,
        state::{StakePool, ValidatorList, ValidatorStakeInfo},
    },
    std::collections::HashSet,
};

pub(crate) type Error = Box<dyn std::error::Error>;

pub fn get_stake_pool(
    rpc_client: &RpcClient,
    stake_pool_address: &Pubkey,
) -> Result<StakePool, Error> {
    let account_data = rpc_client.get_account_data(stake_pool_address)?;
    let stake_pool = try_from_slice_unchecked::<StakePool>(account_data.as_slice())
        .map_err(|err| format!("Invalid stake pool {}: {}", stake_pool_address, err))?;
    Ok(stake_pool)
}

pub fn get_validator_list(
    rpc_client: &RpcClient,
    validator_list_address: &Pubkey,
) -> Result<ValidatorList, Error> {
    let account_data = rpc_client.get_account_data(validator_list_address)?;
    let validator_list = try_from_slice_unchecked::<ValidatorList>(account_data.as_slice())
        .map_err(|err| format!("Invalid validator list {}: {}", validator_list_address, err))?;
    Ok(validator_list)
}

pub fn get_token_account(
    rpc_client: &RpcClient,
    token_account_address: &Pubkey,
    expected_token_mint: &Pubkey,
) -> Result<spl_token_2022_interface::state::Account, Error> {
    let account_data = rpc_client.get_account_data(token_account_address)?;
    let token_account =
        spl_token_2022_interface::state::Account::unpack_from_slice(account_data.as_slice())
            .map_err(|err| format!("Invalid token account {}: {}", token_account_address, err))?;

    if token_account.mint != *expected_token_mint {
        Err(format!(
            "Invalid token mint for {}, expected mint is {}",
            token_account_address, expected_token_mint
        )
        .into())
    } else {
        Ok(token_account)
    }
}

pub fn get_token_mint(
    rpc_client: &RpcClient,
    token_mint_address: &Pubkey,
) -> Result<spl_token_2022_interface::state::Mint, Error> {
    let account_data = rpc_client.get_account_data(token_mint_address)?;
    let token_mint =
        spl_token_2022_interface::state::Mint::unpack_from_slice(account_data.as_slice())
            .map_err(|err| format!("Invalid token mint {}: {}", token_mint_address, err))?;

    Ok(token_mint)
}

pub(crate) fn get_stake_state(
    rpc_client: &RpcClient,
    stake_address: &Pubkey,
) -> Result<stake::state::StakeStateV2, Error> {
    let account_data = rpc_client.get_account_data(stake_address)?;
    let stake_state = deserialize(account_data.as_slice())
        .map_err(|err| format!("Invalid stake account {}: {}", stake_address, err))?;
    Ok(stake_state)
}

#[allow(clippy::result_large_err)]
pub(crate) fn get_stake_pools(
    rpc_client: &RpcClient,
    stake_pool_program_id: &Pubkey,
) -> Result<Vec<(Pubkey, StakePool, ValidatorList, Pubkey)>, ClientError> {
    rpc_client
        .get_program_ui_accounts_with_config(
            stake_pool_program_id,
            RpcProgramAccountsConfig {
                // 0 is the account type
                filters: Some(vec![RpcFilterType::Memcmp(Memcmp::new(
                    0,
                    MemcmpEncodedBytes::Base58("2".to_string()),
                ))]),
                account_config: RpcAccountInfoConfig {
                    encoding: Some(UiAccountEncoding::Base64),
                    ..RpcAccountInfoConfig::default()
                },
                ..RpcProgramAccountsConfig::default()
            },
        )
        .map(|accounts| {
            accounts
                .into_iter()
                .filter_map(|(address, account)| {
                    let pool_withdraw_authority =
                        find_withdraw_authority_program_address(stake_pool_program_id, &address).0;
                    let data = account.data.decode()?;
                    match try_from_slice_unchecked::<StakePool>(&data) {
                        Ok(stake_pool) => {
                            get_validator_list(rpc_client, &stake_pool.validator_list)
                                .map(|validator_list| {
                                    (address, stake_pool, validator_list, pool_withdraw_authority)
                                })
                                .ok()
                        }
                        Err(err) => {
                            eprintln!("Invalid stake pool data for {}: {}", address, err);
                            None
                        }
                    }
                })
                .collect()
        })
}

#[allow(clippy::result_large_err)]
pub(crate) fn get_all_stake(
    rpc_client: &RpcClient,
    authorized_staker: &Pubkey,
) -> Result<HashSet<Pubkey>, ClientError> {
    let all_stake_accounts = rpc_client.get_program_ui_accounts_with_config(
        &stake::program::id(),
        RpcProgramAccountsConfig {
            filters: Some(vec![
                // Filter by `Meta::authorized::staker`, which begins at byte offset 12
                RpcFilterType::Memcmp(Memcmp::new_base58_encoded(12, authorized_staker.as_ref())),
            ]),
            account_config: RpcAccountInfoConfig {
                encoding: Some(solana_account_decoder::UiAccountEncoding::Base64),
                commitment: Some(rpc_client.commitment()),
                ..RpcAccountInfoConfig::default()
            },
            ..RpcProgramAccountsConfig::default()
        },
    )?;

    Ok(all_stake_accounts
        .into_iter()
        .map(|(address, _)| address)
        .collect())
}

/// Helper function to add a compute unit limit instruction to a given set
/// of instructions
pub(crate) fn add_compute_unit_limit_from_simulation(
    rpc_client: &RpcClient,
    instructions: &mut Vec<Instruction>,
    payer: &Pubkey,
    blockhash: &Hash,
) -> Result<(), Error> {
    // add a max compute unit limit instruction for the simulation
    const MAX_COMPUTE_UNIT_LIMIT: u32 = 1_400_000;
    instructions.push(ComputeBudgetInstruction::set_compute_unit_limit(
        MAX_COMPUTE_UNIT_LIMIT,
    ));

    let transaction = Transaction::new_unsigned(Message::new_with_blockhash(
        instructions,
        Some(payer),
        blockhash,
    ));
    let simulation_result = rpc_client.simulate_transaction(&transaction)?.value;
    let units_consumed = simulation_result
        .units_consumed
        .ok_or("No units consumed on simulation")?;
    // Overwrite the compute unit limit instruction with the actual units consumed
    let compute_unit_limit = u32::try_from(units_consumed)?;
    instructions
        .last_mut()
        .expect("Compute budget instruction was added earlier")
        .data = ComputeBudgetInstruction::set_compute_unit_limit(compute_unit_limit).data;
    Ok(())
}

/// Helper function to find an unused ephemeral stake account by incrementing seeds
/// starting from 0 until an uninitialized account is found
pub(crate) fn find_unused_ephemeral_stake_seed(
    rpc_client: &RpcClient,
    stake_pool_program_id: &Pubkey,
    stake_pool_address: &Pubkey,
    max_attempts: u64,
) -> Result<u64, Error> {
    for seed in 0..max_attempts {
        let (ephemeral_stake_address, _) =
            find_ephemeral_stake_program_address(stake_pool_program_id, stake_pool_address, seed);

        // Check if the account exists and is initialized
        match rpc_client.get_account(&ephemeral_stake_address) {
            Ok(account) => {
                // Account exists - check if it's initialized (has non-zero data)
                if account.data.iter().all(|&x| x == 0) {
                    // Account exists but is uninitialized, can use this seed
                    return Ok(seed);
                }
                // Account is initialized, try next seed
                continue;
            }
            Err(_) => {
                // Account doesn't exist, can use this seed
                return Ok(seed);
            }
        }
    }

    Err(format!(
        "Could not find an unused ephemeral stake account after {} attempts. \
         All ephemeral seeds are in use. Wait for the next epoch for accounts to be cleaned up.",
        max_attempts
    )
    .into())
}

/// Check if a validator's transient stake account is currently in use
pub(crate) fn is_transient_stake_in_use(validator_stake_info: &ValidatorStakeInfo) -> bool {
    u64::from(validator_stake_info.transient_stake_lamports) > 0
}
