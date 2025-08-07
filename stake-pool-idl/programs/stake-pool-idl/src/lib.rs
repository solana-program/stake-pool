use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};

declare_id!("SPoo1Ku8WFXoNDMHPsrGSTSG1Y47rzgn41SLUNakuHy");

#[program]
pub mod stake_pool_idl {
    use super::*;

    /// Initialize a new stake pool
    pub fn initialize(
        _ctx: Context<Initialize>,
        _fee: Fee,
        _withdrawal_fee: Fee,
        _deposit_fee: Fee,
        _referral_fee: u8,
        _max_validators: u32,
    ) -> Result<()> {
        Ok(())
    }

    /// Deposit SOL into the stake pool in exchange for pool tokens
    pub fn deposit_sol(_ctx: Context<DepositSol>, _lamports: u64) -> Result<()> {
        Ok(())
    }

    /// Deposit wrapped SOL into the stake pool in exchange for pool tokens
    pub fn deposit_wsol(_ctx: Context<DepositWsol>, _lamports: u64) -> Result<()> {
        Ok(())
    }

    /// Withdraw SOL from the stake pool by burning pool tokens
    pub fn withdraw_sol(_ctx: Context<WithdrawSol>, _pool_tokens: u64) -> Result<()> {
        Ok(())
    }

    /// Deposit stake account into the pool in exchange for pool tokens
    pub fn deposit_stake(_ctx: Context<DepositStake>) -> Result<()> {
        Ok(())
    }

    /// Withdraw stake account from the pool by burning pool tokens
    pub fn withdraw_stake(_ctx: Context<WithdrawStake>, _pool_tokens: u64) -> Result<()> {
        Ok(())
    }

    /// Add a new validator to the pool (staker only)
    pub fn add_validator_to_pool(_ctx: Context<AddValidatorToPool>, _seed: u32) -> Result<()> {
        Ok(())
    }

    /// Remove validator from the pool (staker only)
    pub fn remove_validator_from_pool(_ctx: Context<RemoveValidatorFromPool>) -> Result<()> {
        Ok(())
    }

    /// Update the pool's balance and validator list balances
    pub fn update_validator_list_balance(
        _ctx: Context<UpdateValidatorListBalance>,
        _start_index: u32,
        _no_merge: bool,
    ) -> Result<()> {
        Ok(())
    }

    /// Set manager authority (manager only)
    pub fn set_manager(_ctx: Context<SetManager>) -> Result<()> {
        Ok(())
    }

    /// Set staker authority (manager only)
    pub fn set_staker(_ctx: Context<SetStaker>) -> Result<()> {
        Ok(())
    }

    /// Set fee (manager only)
    pub fn set_fee(_ctx: Context<SetFee>, _fee_type: FeeType, _fee: Fee) -> Result<()> {
        Ok(())
    }
}

// Account structures
#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    /// CHECK: Stake pool account to be initialized
    pub stake_pool: AccountInfo<'info>,
    pub manager: Signer<'info>,
    /// CHECK: Staker authority
    pub staker: AccountInfo<'info>,
    /// CHECK: Withdraw authority
    pub stake_pool_withdraw_authority: AccountInfo<'info>,
    #[account(mut)]
    /// CHECK: Validator list
    pub validator_list: AccountInfo<'info>,
    /// CHECK: Reserve stake account
    pub reserve_stake: AccountInfo<'info>,
    pub pool_mint: Account<'info, Mint>,
    #[account(mut)]
    pub manager_fee_account: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
    /// CHECK: Optional deposit authority
    pub deposit_authority: Option<AccountInfo<'info>>,
}

#[derive(Accounts)]
pub struct DepositSol<'info> {
    #[account(mut)]
    /// CHECK: Stake pool
    pub stake_pool: AccountInfo<'info>,
    /// CHECK: Stake pool withdraw authority
    pub stake_pool_withdraw_authority: AccountInfo<'info>,
    /// CHECK: Reserve stake account
    pub reserve_stake: AccountInfo<'info>,
    pub funding_account: Signer<'info>,
    #[account(mut)]
    pub destination: Account<'info, TokenAccount>,
    #[account(mut)]
    pub manager_fee_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub referrer_pool_tokens_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub pool_mint: Account<'info, Mint>,
    /// CHECK: System program
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    /// CHECK: Optional deposit authority
    pub sol_deposit_authority: Option<AccountInfo<'info>>,
}

#[derive(Accounts)]
pub struct WithdrawSol<'info> {
    #[account(mut)]
    /// CHECK: Stake pool
    pub stake_pool: AccountInfo<'info>,
    /// CHECK: Stake pool withdraw authority
    pub stake_pool_withdraw_authority: AccountInfo<'info>,
    #[account(mut)]
    pub source: Account<'info, TokenAccount>,
    #[account(mut)]
    /// CHECK: Reserve stake account
    pub reserve_stake: AccountInfo<'info>,
    #[account(mut)]
    /// CHECK: Destination lamports account
    pub destination: AccountInfo<'info>,
    #[account(mut)]
    pub manager_fee_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub pool_mint: Account<'info, Mint>,
    /// CHECK: Clock sysvar
    pub clock: Sysvar<'info, Clock>,
    /// CHECK: Stake history sysvar
    pub stake_history: AccountInfo<'info>,
    /// CHECK: Stake program
    pub stake_program: AccountInfo<'info>,
    pub token_program: Program<'info, Token>,
    /// CHECK: Optional withdraw authority
    pub sol_withdraw_authority: Option<AccountInfo<'info>>,
}

#[derive(Accounts)]
pub struct DepositWsol<'info> {
    #[account(mut)]
    /// CHECK: Stake pool
    pub stake_pool: AccountInfo<'info>,
    /// CHECK: Stake pool withdraw authority
    pub stake_pool_withdraw_authority: AccountInfo<'info>,
    #[account(mut)]
    /// CHECK: Reserve stake account
    pub reserve_stake: AccountInfo<'info>,
    #[account(mut)]
    /// CHECK: Wrapped SOL account to be deposited
    pub wsol_account: AccountInfo<'info>,
    /// CHECK: Authority of the wrapped SOL account
    pub wsol_authority: Signer<'info>,
    #[account(mut)]
    /// CHECK: Lamports destination account
    pub lamports_destination: AccountInfo<'info>,
    #[account(mut)]
    pub destination: Account<'info, TokenAccount>,
    #[account(mut)]
    pub manager_fee_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub referrer_pool_tokens_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub pool_mint: Account<'info, Mint>,
    /// CHECK: System program
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    /// CHECK: Wrapped SOL mint
    pub wsol_mint: Account<'info, Mint>,
    /// CHECK: Optional deposit authority
    pub sol_deposit_authority: Option<AccountInfo<'info>>,
}

#[derive(Accounts)]
pub struct DepositStake<'info> {
    #[account(mut)]
    /// CHECK: Stake pool
    pub stake_pool: AccountInfo<'info>,
    #[account(mut)]
    /// CHECK: Validator list
    pub validator_list: AccountInfo<'info>,
    /// CHECK: Deposit authority
    pub deposit_authority: AccountInfo<'info>,
    /// CHECK: Withdraw authority
    pub stake_pool_withdraw_authority: AccountInfo<'info>,
    #[account(mut)]
    /// CHECK: Deposit stake account
    pub deposit_stake: AccountInfo<'info>,
    #[account(mut)]
    /// CHECK: Validator stake account
    pub validator_stake: AccountInfo<'info>,
    #[account(mut)]
    /// CHECK: Reserve stake account
    pub reserve_stake: AccountInfo<'info>,
    #[account(mut)]
    pub destination_pool_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub manager_fee_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub referrer_pool_tokens_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub pool_mint: Account<'info, Mint>,
    /// CHECK: Clock sysvar
    pub clock: Sysvar<'info, Clock>,
    /// CHECK: Stake history sysvar
    pub stake_history: AccountInfo<'info>,
    pub token_program: Program<'info, Token>,
    /// CHECK: Stake program
    pub stake_program: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct WithdrawStake<'info> {
    #[account(mut)]
    /// CHECK: Stake pool
    pub stake_pool: AccountInfo<'info>,
    #[account(mut)]
    /// CHECK: Validator list
    pub validator_list: AccountInfo<'info>,
    /// CHECK: Withdraw authority
    pub stake_pool_withdraw_authority: AccountInfo<'info>,
    #[account(mut)]
    /// CHECK: Validator stake account
    pub validator_stake: AccountInfo<'info>,
    #[account(mut)]
    /// CHECK: Destination stake account
    pub destination_stake: AccountInfo<'info>,
    #[account(mut)]
    /// CHECK: Destination stake account authority
    pub destination_stake_authority: AccountInfo<'info>,
    #[account(mut)]
    /// CHECK: Source pool account
    pub source_pool_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub manager_fee_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub pool_mint: Account<'info, Mint>,
    /// CHECK: Clock sysvar
    pub clock: Sysvar<'info, Clock>,
    pub token_program: Program<'info, Token>,
    /// CHECK: Stake program
    pub stake_program: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct AddValidatorToPool<'info> {
    #[account(mut)]
    /// CHECK: Stake pool
    pub stake_pool: AccountInfo<'info>,
    pub staker: Signer<'info>,
    #[account(mut)]
    /// CHECK: Reserve stake account
    pub reserve_stake: AccountInfo<'info>,
    /// CHECK: Withdraw authority
    pub withdraw_authority: AccountInfo<'info>,
    #[account(mut)]
    /// CHECK: Validator list
    pub validator_list: AccountInfo<'info>,
    #[account(mut)]
    /// CHECK: Validator stake account
    pub validator_stake: AccountInfo<'info>,
    /// CHECK: Validator vote account
    pub validator_vote: AccountInfo<'info>,
    pub rent: Sysvar<'info, Rent>,
    pub clock: Sysvar<'info, Clock>,
    /// CHECK: Stake history sysvar
    pub stake_history: AccountInfo<'info>,
    /// CHECK: Stake config sysvar
    pub stake_config: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
    /// CHECK: Stake program
    pub stake_program: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct RemoveValidatorFromPool<'info> {
    #[account(mut)]
    /// CHECK: Stake pool
    pub stake_pool: AccountInfo<'info>,
    pub staker: Signer<'info>,
    /// CHECK: Withdraw authority
    pub withdraw_authority: AccountInfo<'info>,
    #[account(mut)]
    /// CHECK: Validator list
    pub validator_list: AccountInfo<'info>,
    #[account(mut)]
    /// CHECK: Validator stake account
    pub validator_stake: AccountInfo<'info>,
    #[account(mut)]
    /// CHECK: Transient stake account
    pub transient_stake: AccountInfo<'info>,
    pub clock: Sysvar<'info, Clock>,
    /// CHECK: Stake program
    pub stake_program: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct UpdateValidatorListBalance<'info> {
    #[account(mut)]
    /// CHECK: Stake pool
    pub stake_pool: AccountInfo<'info>,
    /// CHECK: Withdraw authority
    pub withdraw_authority: AccountInfo<'info>,
    #[account(mut)]
    /// CHECK: Validator list
    pub validator_list: AccountInfo<'info>,
    #[account(mut)]
    /// CHECK: Reserve stake account
    pub reserve_stake: AccountInfo<'info>,
    pub clock: Sysvar<'info, Clock>,
    /// CHECK: Stake history sysvar
    pub stake_history: AccountInfo<'info>,
    /// CHECK: Stake program
    pub stake_program: AccountInfo<'info>,
    // validator_stake_accounts would be remaining accounts
}

#[derive(Accounts)]
pub struct SetManager<'info> {
    #[account(mut)]
    /// CHECK: Stake pool
    pub stake_pool: AccountInfo<'info>,
    pub manager: Signer<'info>,
    /// CHECK: New manager
    pub new_manager: AccountInfo<'info>,
    /// CHECK: New fee account
    pub new_manager_fee_account: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct SetStaker<'info> {
    #[account(mut)]
    /// CHECK: Stake pool
    pub stake_pool: AccountInfo<'info>,
    pub manager: Signer<'info>,
    /// CHECK: New staker
    pub new_staker: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct SetFee<'info> {
    #[account(mut)]
    /// CHECK: Stake pool
    pub stake_pool: AccountInfo<'info>,
    pub manager: Signer<'info>,
    pub clock: Sysvar<'info, Clock>,
}

// Data structures
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq)]
pub struct Fee {
    pub denominator: u64,
    pub numerator: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq)]
pub struct Lockup {
    pub unix_timestamp: i64,
    pub epoch: u64,
    pub custodian: Pubkey,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq)]
pub enum FeeType {
    SolReferral(u8),
    StakeReferral(u8),
    Epoch(Fee),
    StakeWithdrawal(Fee),
    SolWithdrawal(Fee),
    StakeDeposit(Fee),
    SolDeposit(Fee),
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq)]
pub enum AccountType {
    Uninitialized,
    StakePool,
    ValidatorList,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct StakePool {
    pub account_type: AccountType,
    pub manager: Pubkey,
    pub staker: Pubkey,
    pub stake_deposit_authority: Pubkey,
    pub stake_withdraw_bump_seed: u8,
    pub validator_list: Pubkey,
    pub reserve_stake: Pubkey,
    pub pool_mint: Pubkey,
    pub manager_fee_account: Pubkey,
    pub token_program_id: Pubkey,
    pub total_lamports: u64,
    pub pool_token_supply: u64,
    pub last_update_epoch: u64,
    pub lockup: Lockup,
    pub epoch_fee: Fee,
    pub preferred_deposit_validator_vote_address: Option<Pubkey>,
    pub preferred_withdraw_validator_vote_address: Option<Pubkey>,
    pub stake_deposit_fee: Fee,
    pub stake_withdrawal_fee: Fee,
    pub stake_referral_fee: u8,
    pub sol_deposit_authority: Option<Pubkey>,
    pub sol_deposit_fee: Fee,
    pub sol_referral_fee: u8,
    pub sol_withdraw_authority: Option<Pubkey>,
    pub sol_withdrawal_fee: Fee,
    pub last_epoch_pool_token_supply: u64,
    pub last_epoch_total_lamports: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct ValidatorStakeInfo {
    pub status: StakeStatus,
    pub vote_account_address: Pubkey,
    pub active_stake_lamports: u64,
    pub transient_stake_lamports: u64,
    pub last_update_epoch: u64,
    pub transient_seed_suffix: u64,
    pub unused: u32,
    pub validator_seed_suffix: u32,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq)]
pub enum StakeStatus {
    Active,
    DeactivatingTransient,
    ReadyForRemoval,
}
