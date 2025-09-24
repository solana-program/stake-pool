import {
  PublicKey,
  STAKE_CONFIG_ID,
  SYSVAR_CLOCK_PUBKEY,
  SYSVAR_RENT_PUBKEY,
  SYSVAR_STAKE_HISTORY_PUBKEY,
  StakeProgram,
  SystemProgram,
  TransactionInstruction,
} from '@solana/web3.js';
import * as BufferLayout from '@solana/buffer-layout';
import { TOKEN_PROGRAM_ID } from '@solana/spl-token';
import { InstructionType, encodeData, decodeData } from './utils';
import {
  METADATA_MAX_NAME_LENGTH,
  METADATA_MAX_SYMBOL_LENGTH,
  METADATA_MAX_URI_LENGTH,
  METADATA_PROGRAM_ID,
  STAKE_POOL_PROGRAM_ID,
  DEVNET_STAKE_POOL_PROGRAM_ID,
} from './constants';

/**
 * An enumeration of valid StakePoolInstructionType's
 */
export type StakePoolInstructionType =
  | 'IncreaseValidatorStake'
  | 'DecreaseValidatorStake'
  | 'UpdateValidatorListBalance'
  | 'UpdateStakePoolBalance'
  | 'CleanupRemovedValidatorEntries'
  | 'DepositStake'
  | 'DepositSol'
  | 'WithdrawStake'
  | 'WithdrawSol'
  | 'IncreaseAdditionalValidatorStake'
  | 'DecreaseAdditionalValidatorStake'
  | 'DecreaseValidatorStakeWithReserve'
  | 'Redelegate'
  | 'AddValidatorToPool'
  | 'RemoveValidatorFromPool'
  | 'DepositWsolWithSession'
  | 'WithdrawWsolWithSession';

// 'UpdateTokenMetadata' and 'CreateTokenMetadata' have dynamic layouts

const MOVE_STAKE_LAYOUT = BufferLayout.struct<any>([
  BufferLayout.u8('instruction'),
  BufferLayout.ns64('lamports'),
  BufferLayout.ns64('transientStakeSeed'),
]);

const UPDATE_VALIDATOR_LIST_BALANCE_LAYOUT = BufferLayout.struct<any>([
  BufferLayout.u8('instruction'),
  BufferLayout.u32('startIndex'),
  BufferLayout.u8('noMerge'),
]);

export function tokenMetadataLayout(
  instruction: number,
  nameLength: number,
  symbolLength: number,
  uriLength: number,
) {
  if (nameLength > METADATA_MAX_NAME_LENGTH) {
    throw 'maximum token name length is 32 characters';
  }

  if (symbolLength > METADATA_MAX_SYMBOL_LENGTH) {
    throw 'maximum token symbol length is 10 characters';
  }

  if (uriLength > METADATA_MAX_URI_LENGTH) {
    throw 'maximum token uri length is 200 characters';
  }

  return {
    index: instruction,
    layout: BufferLayout.struct<any>([
      BufferLayout.u8('instruction'),
      BufferLayout.u32('nameLen'),
      BufferLayout.blob(nameLength, 'name'),
      BufferLayout.u32('symbolLen'),
      BufferLayout.blob(symbolLength, 'symbol'),
      BufferLayout.u32('uriLen'),
      BufferLayout.blob(uriLength, 'uri'),
    ]),
  };
}

/**
 * An enumeration of valid stake InstructionType's
 * @internal
 */
export const STAKE_POOL_INSTRUCTION_LAYOUTS: {
  /* eslint-disable-next-line @typescript-eslint/no-unused-vars */
  [type in StakePoolInstructionType]: InstructionType;
} = Object.freeze({
  AddValidatorToPool: {
    index: 1,
    layout: BufferLayout.struct<any>([BufferLayout.u8('instruction'), BufferLayout.u32('seed')]),
  },
  RemoveValidatorFromPool: {
    index: 2,
    layout: BufferLayout.struct<any>([BufferLayout.u8('instruction')]),
  },
  DecreaseValidatorStake: {
    index: 3,
    layout: MOVE_STAKE_LAYOUT,
  },
  IncreaseValidatorStake: {
    index: 4,
    layout: MOVE_STAKE_LAYOUT,
  },
  UpdateValidatorListBalance: {
    index: 6,
    layout: UPDATE_VALIDATOR_LIST_BALANCE_LAYOUT,
  },
  UpdateStakePoolBalance: {
    index: 7,
    layout: BufferLayout.struct<any>([BufferLayout.u8('instruction')]),
  },
  CleanupRemovedValidatorEntries: {
    index: 8,
    layout: BufferLayout.struct<any>([BufferLayout.u8('instruction')]),
  },
  DepositStake: {
    index: 9,
    layout: BufferLayout.struct<any>([BufferLayout.u8('instruction')]),
  },
  /// Withdraw the token from the pool at the current ratio.
  WithdrawStake: {
    index: 10,
    layout: BufferLayout.struct<any>([
      BufferLayout.u8('instruction'),
      BufferLayout.ns64('poolTokens'),
    ]),
  },
  /// Deposit SOL directly into the pool's reserve account. The output is a "pool" token
  /// representing ownership into the pool. Inputs are converted to the current ratio.
  DepositSol: {
    index: 14,
    layout: BufferLayout.struct<any>([
      BufferLayout.u8('instruction'),
      BufferLayout.ns64('lamports'),
    ]),
  },
  /// Withdraw SOL directly from the pool's reserve account. Fails if the
  /// reserve does not have enough SOL.
  WithdrawSol: {
    index: 16,
    layout: BufferLayout.struct<any>([
      BufferLayout.u8('instruction'),
      BufferLayout.ns64('poolTokens'),
    ]),
  },
  IncreaseAdditionalValidatorStake: {
    index: 19,
    layout: BufferLayout.struct<any>([
      BufferLayout.u8('instruction'),
      BufferLayout.ns64('lamports'),
      BufferLayout.ns64('transientStakeSeed'),
      BufferLayout.ns64('ephemeralStakeSeed'),
    ]),
  },
  DecreaseAdditionalValidatorStake: {
    index: 20,
    layout: BufferLayout.struct<any>([
      BufferLayout.u8('instruction'),
      BufferLayout.ns64('lamports'),
      BufferLayout.ns64('transientStakeSeed'),
      BufferLayout.ns64('ephemeralStakeSeed'),
    ]),
  },
  DecreaseValidatorStakeWithReserve: {
    index: 21,
    layout: MOVE_STAKE_LAYOUT,
  },
  Redelegate: {
    index: 22,
    layout: BufferLayout.struct<any>([BufferLayout.u8('instruction')]),
  },
  DepositStakeWithSlippage: {
    index: 23,
    layout: BufferLayout.struct<any>([
      BufferLayout.u8('instruction'),
      BufferLayout.ns64('lamports'),
    ]),
  },
  WithdrawStakeWithSlippage: {
    index: 24,
    layout: BufferLayout.struct<any>([
      BufferLayout.u8('instruction'),
      BufferLayout.ns64('lamports'),
    ]),
  },
  DepositSolWithSlippage: {
    index: 25,
    layout: BufferLayout.struct<any>([
      BufferLayout.u8('instruction'),
      BufferLayout.ns64('lamports'),
    ]),
  },
  WithdrawSolWithSlippage: {
    index: 26,
    layout: BufferLayout.struct<any>([
      BufferLayout.u8('instruction'),
      BufferLayout.ns64('lamports'),
    ]),
  },
  DepositWsolWithSession: {
    index: 27,
    layout: BufferLayout.struct<any>([
      BufferLayout.u8('instruction'),
      BufferLayout.ns64('lamports'),
    ]),
  },
  WithdrawWsolWithSession: {
    index: 28,
    layout: BufferLayout.struct<any>([
      BufferLayout.u8('instruction'),
      BufferLayout.ns64('poolTokens'),
    ]),
  },
});

/**
 * Cleans up validator stake account entries marked as `ReadyForRemoval`
 */
export type CleanupRemovedValidatorEntriesParams = {
  programId?: PublicKey | undefined;
  stakePool: PublicKey;
  validatorList: PublicKey;
};

/**
 * Updates balances of validator and transient stake accounts in the pool.
 */
export type UpdateValidatorListBalanceParams = {
  programId?: PublicKey | undefined;
  stakePool: PublicKey;
  withdrawAuthority: PublicKey;
  validatorList: PublicKey;
  reserveStake: PublicKey;
  validatorAndTransientStakePairs: PublicKey[];
  startIndex: number;
  noMerge: boolean;
};

/**
 * Updates total pool balance based on balances in the reserve and validator list.
 */
export type UpdateStakePoolBalanceParams = {
  programId?: PublicKey | undefined;
  stakePool: PublicKey;
  withdrawAuthority: PublicKey;
  validatorList: PublicKey;
  reserveStake: PublicKey;
  managerFeeAccount: PublicKey;
  poolMint: PublicKey;
};

/**
 * (Staker only) Decrease active stake on a validator, eventually moving it to the reserve
 */
export type DecreaseValidatorStakeParams = {
  programId?: PublicKey | undefined;
  stakePool: PublicKey;
  staker: PublicKey;
  withdrawAuthority: PublicKey;
  validatorList: PublicKey;
  validatorStake: PublicKey;
  transientStake: PublicKey;
  // Amount of lamports to split into the transient stake account
  lamports: number;
  // Seed to used to create the transient stake account
  transientStakeSeed: number;
};

export interface DecreaseValidatorStakeWithReserveParams extends DecreaseValidatorStakeParams {
  reserveStake: PublicKey;
}

export interface DecreaseAdditionalValidatorStakeParams extends DecreaseValidatorStakeParams {
  reserveStake: PublicKey;
  ephemeralStake: PublicKey;
  ephemeralStakeSeed: number;
}

/**
 * (Staker only) Increase stake on a validator from the reserve account.
 */
export type IncreaseValidatorStakeParams = {
  programId?: PublicKey | undefined;
  stakePool: PublicKey;
  staker: PublicKey;
  withdrawAuthority: PublicKey;
  validatorList: PublicKey;
  reserveStake: PublicKey;
  transientStake: PublicKey;
  validatorStake: PublicKey;
  validatorVote: PublicKey;
  // Amount of lamports to split into the transient stake account
  lamports: number;
  // Seed to used to create the transient stake account
  transientStakeSeed: number;
};

export interface IncreaseAdditionalValidatorStakeParams extends IncreaseValidatorStakeParams {
  ephemeralStake: PublicKey;
  ephemeralStakeSeed: number;
}

/**
 * Deposits a stake account into the pool in exchange for pool tokens
 */
export type DepositStakeParams = {
  programId?: PublicKey | undefined;
  stakePool: PublicKey;
  validatorList: PublicKey;
  depositAuthority: PublicKey;
  withdrawAuthority: PublicKey;
  depositStake: PublicKey;
  validatorStake: PublicKey;
  reserveStake: PublicKey;
  destinationPoolAccount: PublicKey;
  managerFeeAccount: PublicKey;
  referralPoolAccount: PublicKey;
  poolMint: PublicKey;
};

/**
 * Withdraws a stake account from the pool in exchange for pool tokens
 */
export type WithdrawStakeParams = {
  programId?: PublicKey | undefined;
  stakePool: PublicKey;
  validatorList: PublicKey;
  withdrawAuthority: PublicKey;
  validatorStake: PublicKey;
  destinationStake: PublicKey;
  destinationStakeAuthority: PublicKey;
  sourceTransferAuthority: PublicKey;
  sourcePoolAccount: PublicKey;
  managerFeeAccount: PublicKey;
  poolMint: PublicKey;
  poolTokens: number;
};

/**
 * Withdraw sol instruction params
 */
export type WithdrawSolParams = {
  programId?: PublicKey | undefined;
  stakePool: PublicKey;
  sourcePoolAccount: PublicKey;
  withdrawAuthority: PublicKey;
  reserveStake: PublicKey;
  destinationSystemAccount: PublicKey;
  sourceTransferAuthority: PublicKey;
  solWithdrawAuthority?: PublicKey | undefined;
  managerFeeAccount: PublicKey;
  poolMint: PublicKey;
  poolTokens: number;
};

/**
 * Withdraw WSOL with session instruction params
 */
export type WithdrawWsolWithSessionParams = {
  programId: PublicKey;
  stakePool: PublicKey;
  withdrawAuthority: PublicKey;
  userTransferAuthority: PublicKey;
  poolTokensFrom: PublicKey;
  reserveStake: PublicKey;
  userWsolAccount: PublicKey;
  managerFeeAccount: PublicKey;
  poolMint: PublicKey;
  tokenProgramId: PublicKey;
  solWithdrawAuthority?: PublicKey;
  wsolMint: PublicKey;
  feePayer: PublicKey;
  userOwner: PublicKey;
  programSigner: PublicKey;
  poolTokens: number;
};

/**
 * Deposit SOL directly into the pool's reserve account. The output is a "pool" token
 * representing ownership into the pool. Inputs are converted to the current ratio.
 */
export type DepositSolParams = {
  programId?: PublicKey | undefined;
  stakePool: PublicKey;
  depositAuthority?: PublicKey | undefined;
  withdrawAuthority: PublicKey;
  reserveStake: PublicKey;
  fundingAccount: PublicKey;
  destinationPoolAccount: PublicKey;
  managerFeeAccount: PublicKey;
  referralPoolAccount: PublicKey;
  poolMint: PublicKey;
  lamports: number;
};

export type CreateTokenMetadataParams = {
  programId?: PublicKey | undefined;
  stakePool: PublicKey;
  manager: PublicKey;
  tokenMetadata: PublicKey;
  withdrawAuthority: PublicKey;
  poolMint: PublicKey;
  payer: PublicKey;
  name: string;
  symbol: string;
  uri: string;
};

export type UpdateTokenMetadataParams = {
  programId?: PublicKey | undefined;
  stakePool: PublicKey;
  manager: PublicKey;
  tokenMetadata: PublicKey;
  withdrawAuthority: PublicKey;
  name: string;
  symbol: string;
  uri: string;
};

export type AddValidatorToPoolParams = {
  programId?: PublicKey | undefined;
  stakePool: PublicKey;
  staker: PublicKey;
  reserveStake: PublicKey;
  withdrawAuthority: PublicKey;
  validatorList: PublicKey;
  validatorStake: PublicKey;
  validatorVote: PublicKey;
  seed?: number;
};

export type RemoveValidatorFromPoolParams = {
  programId?: PublicKey | undefined;
  stakePool: PublicKey;
  staker: PublicKey;
  withdrawAuthority: PublicKey;
  validatorList: PublicKey;
  validatorStake: PublicKey;
  transientStake: PublicKey;
};

/**
 * Stake Pool Instruction class
 */
export class StakePoolInstruction {
  /**
   * Creates instruction to add a validator into the stake pool.
   */
  static addValidatorToPool(params: AddValidatorToPoolParams): TransactionInstruction {
    const {
      programId,
      stakePool,
      staker,
      reserveStake,
      withdrawAuthority,
      validatorList,
      validatorStake,
      validatorVote,
      seed,
    } = params;
    const type = STAKE_POOL_INSTRUCTION_LAYOUTS.AddValidatorToPool;
    const data = encodeData(type, { seed: seed == undefined ? 0 : seed });

    const keys = [
      { pubkey: stakePool, isSigner: false, isWritable: true },
      { pubkey: staker, isSigner: true, isWritable: false },
      { pubkey: reserveStake, isSigner: false, isWritable: true },
      { pubkey: withdrawAuthority, isSigner: false, isWritable: false },
      { pubkey: validatorList, isSigner: false, isWritable: true },
      { pubkey: validatorStake, isSigner: false, isWritable: true },
      { pubkey: validatorVote, isSigner: false, isWritable: false },
      { pubkey: SYSVAR_RENT_PUBKEY, isSigner: false, isWritable: false },
      { pubkey: SYSVAR_CLOCK_PUBKEY, isSigner: false, isWritable: false },
      { pubkey: SYSVAR_STAKE_HISTORY_PUBKEY, isSigner: false, isWritable: false },
      { pubkey: STAKE_CONFIG_ID, isSigner: false, isWritable: false },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      { pubkey: StakeProgram.programId, isSigner: false, isWritable: false },
    ];

    return new TransactionInstruction({
      programId: programId ?? STAKE_POOL_PROGRAM_ID,
      keys,
      data,
    });
  }

  /**
   * Creates instruction to remove a validator from the stake pool.
   */
  static removeValidatorFromPool(params: RemoveValidatorFromPoolParams): TransactionInstruction {
    const {
      programId,
      stakePool,
      staker,
      withdrawAuthority,
      validatorList,
      validatorStake,
      transientStake,
    } = params;
    const type = STAKE_POOL_INSTRUCTION_LAYOUTS.RemoveValidatorFromPool;
    const data = encodeData(type);

    const keys = [
      { pubkey: stakePool, isSigner: false, isWritable: true },
      { pubkey: staker, isSigner: true, isWritable: false },
      { pubkey: withdrawAuthority, isSigner: false, isWritable: false },
      { pubkey: validatorList, isSigner: false, isWritable: true },
      { pubkey: validatorStake, isSigner: false, isWritable: true },
      { pubkey: transientStake, isSigner: false, isWritable: true },
      { pubkey: SYSVAR_CLOCK_PUBKEY, isSigner: false, isWritable: false },
      { pubkey: StakeProgram.programId, isSigner: false, isWritable: false },
    ];

    return new TransactionInstruction({
      programId: programId ?? STAKE_POOL_PROGRAM_ID,
      keys,
      data,
    });
  }

  /**
   * Creates instruction to update a set of validators in the stake pool.
   */
  static updateValidatorListBalance(
    params: UpdateValidatorListBalanceParams,
  ): TransactionInstruction {
    const {
      programId,
      stakePool,
      withdrawAuthority,
      validatorList,
      reserveStake,
      startIndex,
      noMerge,
      validatorAndTransientStakePairs,
    } = params;

    const type = STAKE_POOL_INSTRUCTION_LAYOUTS.UpdateValidatorListBalance;
    const data = encodeData(type, { startIndex, noMerge: noMerge ? 1 : 0 });

    const keys = [
      { pubkey: stakePool, isSigner: false, isWritable: false },
      { pubkey: withdrawAuthority, isSigner: false, isWritable: false },
      { pubkey: validatorList, isSigner: false, isWritable: true },
      { pubkey: reserveStake, isSigner: false, isWritable: true },
      { pubkey: SYSVAR_CLOCK_PUBKEY, isSigner: false, isWritable: false },
      { pubkey: SYSVAR_STAKE_HISTORY_PUBKEY, isSigner: false, isWritable: false },
      { pubkey: StakeProgram.programId, isSigner: false, isWritable: false },
      ...validatorAndTransientStakePairs.map((pubkey) => ({
        pubkey,
        isSigner: false,
        isWritable: true,
      })),
    ];

    return new TransactionInstruction({
      programId: programId ?? STAKE_POOL_PROGRAM_ID,
      keys,
      data,
    });
  }

  /**
   * Creates instruction to update the overall stake pool balance.
   */
  static updateStakePoolBalance(params: UpdateStakePoolBalanceParams): TransactionInstruction {
    const {
      programId,
      stakePool,
      withdrawAuthority,
      validatorList,
      reserveStake,
      managerFeeAccount,
      poolMint,
    } = params;

    const type = STAKE_POOL_INSTRUCTION_LAYOUTS.UpdateStakePoolBalance;
    const data = encodeData(type);

    const keys = [
      { pubkey: stakePool, isSigner: false, isWritable: true },
      { pubkey: withdrawAuthority, isSigner: false, isWritable: false },
      { pubkey: validatorList, isSigner: false, isWritable: true },
      { pubkey: reserveStake, isSigner: false, isWritable: false },
      { pubkey: managerFeeAccount, isSigner: false, isWritable: true },
      { pubkey: poolMint, isSigner: false, isWritable: true },
      { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
    ];

    return new TransactionInstruction({
      programId: programId ?? STAKE_POOL_PROGRAM_ID,
      keys,
      data,
    });
  }

  /**
   * Creates instruction to cleanup removed validator entries.
   */
  static cleanupRemovedValidatorEntries(
    params: CleanupRemovedValidatorEntriesParams,
  ): TransactionInstruction {
    const { programId, stakePool, validatorList } = params;

    const type = STAKE_POOL_INSTRUCTION_LAYOUTS.CleanupRemovedValidatorEntries;
    const data = encodeData(type);

    const keys = [
      { pubkey: stakePool, isSigner: false, isWritable: false },
      { pubkey: validatorList, isSigner: false, isWritable: true },
    ];

    return new TransactionInstruction({
      programId: programId ?? STAKE_POOL_PROGRAM_ID,
      keys,
      data,
    });
  }

  /**
   * Creates `IncreaseValidatorStake` instruction (rebalance from reserve account to
   * transient account)
   */
  static increaseValidatorStake(params: IncreaseValidatorStakeParams): TransactionInstruction {
    const {
      programId,
      stakePool,
      staker,
      withdrawAuthority,
      validatorList,
      reserveStake,
      transientStake,
      validatorStake,
      validatorVote,
      lamports,
      transientStakeSeed,
    } = params;

    const type = STAKE_POOL_INSTRUCTION_LAYOUTS.IncreaseValidatorStake;
    const data = encodeData(type, { lamports, transientStakeSeed });

    const keys = [
      { pubkey: stakePool, isSigner: false, isWritable: false },
      { pubkey: staker, isSigner: true, isWritable: false },
      { pubkey: withdrawAuthority, isSigner: false, isWritable: false },
      { pubkey: validatorList, isSigner: false, isWritable: true },
      { pubkey: reserveStake, isSigner: false, isWritable: true },
      { pubkey: transientStake, isSigner: false, isWritable: true },
      { pubkey: validatorStake, isSigner: false, isWritable: false },
      { pubkey: validatorVote, isSigner: false, isWritable: false },
      { pubkey: SYSVAR_CLOCK_PUBKEY, isSigner: false, isWritable: false },
      { pubkey: SYSVAR_RENT_PUBKEY, isSigner: false, isWritable: false },
      { pubkey: SYSVAR_STAKE_HISTORY_PUBKEY, isSigner: false, isWritable: false },
      { pubkey: STAKE_CONFIG_ID, isSigner: false, isWritable: false },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      { pubkey: StakeProgram.programId, isSigner: false, isWritable: false },
    ];

    return new TransactionInstruction({
      programId: programId ?? STAKE_POOL_PROGRAM_ID,
      keys,
      data,
    });
  }

  /**
   * Creates `IncreaseAdditionalValidatorStake` instruction (rebalance from reserve account to
   * transient account)
   */
  static increaseAdditionalValidatorStake(
    params: IncreaseAdditionalValidatorStakeParams,
  ): TransactionInstruction {
    const {
      programId,
      stakePool,
      staker,
      withdrawAuthority,
      validatorList,
      reserveStake,
      transientStake,
      validatorStake,
      validatorVote,
      lamports,
      transientStakeSeed,
      ephemeralStake,
      ephemeralStakeSeed,
    } = params;

    const type = STAKE_POOL_INSTRUCTION_LAYOUTS.IncreaseAdditionalValidatorStake;
    const data = encodeData(type, { lamports, transientStakeSeed, ephemeralStakeSeed });

    const keys = [
      { pubkey: stakePool, isSigner: false, isWritable: false },
      { pubkey: staker, isSigner: true, isWritable: false },
      { pubkey: withdrawAuthority, isSigner: false, isWritable: false },
      { pubkey: validatorList, isSigner: false, isWritable: true },
      { pubkey: reserveStake, isSigner: false, isWritable: true },
      { pubkey: ephemeralStake, isSigner: false, isWritable: true },
      { pubkey: transientStake, isSigner: false, isWritable: true },
      { pubkey: validatorStake, isSigner: false, isWritable: false },
      { pubkey: validatorVote, isSigner: false, isWritable: false },
      { pubkey: SYSVAR_CLOCK_PUBKEY, isSigner: false, isWritable: false },
      { pubkey: SYSVAR_STAKE_HISTORY_PUBKEY, isSigner: false, isWritable: false },
      { pubkey: STAKE_CONFIG_ID, isSigner: false, isWritable: false },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      { pubkey: StakeProgram.programId, isSigner: false, isWritable: false },
    ];

    return new TransactionInstruction({
      programId: programId ?? STAKE_POOL_PROGRAM_ID,
      keys,
      data,
    });
  }

  /**
   * Creates `DecreaseValidatorStake` instruction (rebalance from validator account to
   * transient account)
   */
  static decreaseValidatorStake(params: DecreaseValidatorStakeParams): TransactionInstruction {
    const {
      programId,
      stakePool,
      staker,
      withdrawAuthority,
      validatorList,
      validatorStake,
      transientStake,
      lamports,
      transientStakeSeed,
    } = params;

    const type = STAKE_POOL_INSTRUCTION_LAYOUTS.DecreaseValidatorStake;
    const data = encodeData(type, { lamports, transientStakeSeed });

    const keys = [
      { pubkey: stakePool, isSigner: false, isWritable: false },
      { pubkey: staker, isSigner: true, isWritable: false },
      { pubkey: withdrawAuthority, isSigner: false, isWritable: false },
      { pubkey: validatorList, isSigner: false, isWritable: true },
      { pubkey: validatorStake, isSigner: false, isWritable: true },
      { pubkey: transientStake, isSigner: false, isWritable: true },
      { pubkey: SYSVAR_CLOCK_PUBKEY, isSigner: false, isWritable: false },
      { pubkey: SYSVAR_RENT_PUBKEY, isSigner: false, isWritable: false },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      { pubkey: StakeProgram.programId, isSigner: false, isWritable: false },
    ];

    return new TransactionInstruction({
      programId: programId ?? STAKE_POOL_PROGRAM_ID,
      keys,
      data,
    });
  }

  /**
   * Creates `DecreaseValidatorStakeWithReserve` instruction (rebalance from
   * validator account to transient account)
   */
  static decreaseValidatorStakeWithReserve(
    params: DecreaseValidatorStakeWithReserveParams,
  ): TransactionInstruction {
    const {
      programId,
      stakePool,
      staker,
      withdrawAuthority,
      validatorList,
      reserveStake,
      validatorStake,
      transientStake,
      lamports,
      transientStakeSeed,
    } = params;

    const type = STAKE_POOL_INSTRUCTION_LAYOUTS.DecreaseValidatorStakeWithReserve;
    const data = encodeData(type, { lamports, transientStakeSeed });

    const keys = [
      { pubkey: stakePool, isSigner: false, isWritable: false },
      { pubkey: staker, isSigner: true, isWritable: false },
      { pubkey: withdrawAuthority, isSigner: false, isWritable: false },
      { pubkey: validatorList, isSigner: false, isWritable: true },
      { pubkey: reserveStake, isSigner: false, isWritable: true },
      { pubkey: validatorStake, isSigner: false, isWritable: true },
      { pubkey: transientStake, isSigner: false, isWritable: true },
      { pubkey: SYSVAR_CLOCK_PUBKEY, isSigner: false, isWritable: false },
      { pubkey: SYSVAR_STAKE_HISTORY_PUBKEY, isSigner: false, isWritable: false },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      { pubkey: StakeProgram.programId, isSigner: false, isWritable: false },
    ];

    return new TransactionInstruction({
      programId: programId ?? STAKE_POOL_PROGRAM_ID,
      keys,
      data,
    });
  }

  /**
   * Creates `DecreaseAdditionalValidatorStake` instruction (rebalance from
   * validator account to transient account)
   */
  static decreaseAdditionalValidatorStake(
    params: DecreaseAdditionalValidatorStakeParams,
  ): TransactionInstruction {
    const {
      programId,
      stakePool,
      staker,
      withdrawAuthority,
      validatorList,
      reserveStake,
      validatorStake,
      transientStake,
      lamports,
      transientStakeSeed,
      ephemeralStakeSeed,
      ephemeralStake,
    } = params;

    const type = STAKE_POOL_INSTRUCTION_LAYOUTS.DecreaseAdditionalValidatorStake;
    const data = encodeData(type, { lamports, transientStakeSeed, ephemeralStakeSeed });

    const keys = [
      { pubkey: stakePool, isSigner: false, isWritable: false },
      { pubkey: staker, isSigner: true, isWritable: false },
      { pubkey: withdrawAuthority, isSigner: false, isWritable: false },
      { pubkey: validatorList, isSigner: false, isWritable: true },
      { pubkey: reserveStake, isSigner: false, isWritable: true },
      { pubkey: validatorStake, isSigner: false, isWritable: true },
      { pubkey: ephemeralStake, isSigner: false, isWritable: true },
      { pubkey: transientStake, isSigner: false, isWritable: true },
      { pubkey: SYSVAR_CLOCK_PUBKEY, isSigner: false, isWritable: false },
      { pubkey: SYSVAR_STAKE_HISTORY_PUBKEY, isSigner: false, isWritable: false },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      { pubkey: StakeProgram.programId, isSigner: false, isWritable: false },
    ];

    return new TransactionInstruction({
      programId: programId ?? STAKE_POOL_PROGRAM_ID,
      keys,
      data,
    });
  }

  /**
   * Creates a transaction instruction to deposit a stake account into a stake pool.
   */
  static depositStake(params: DepositStakeParams): TransactionInstruction {
    const {
      programId,
      stakePool,
      validatorList,
      depositAuthority,
      withdrawAuthority,
      depositStake,
      validatorStake,
      reserveStake,
      destinationPoolAccount,
      managerFeeAccount,
      referralPoolAccount,
      poolMint,
    } = params;

    const type = STAKE_POOL_INSTRUCTION_LAYOUTS.DepositStake;
    const data = encodeData(type);

    const keys = [
      { pubkey: stakePool, isSigner: false, isWritable: true },
      { pubkey: validatorList, isSigner: false, isWritable: true },
      { pubkey: depositAuthority, isSigner: false, isWritable: false },
      { pubkey: withdrawAuthority, isSigner: false, isWritable: false },
      { pubkey: depositStake, isSigner: false, isWritable: true },
      { pubkey: validatorStake, isSigner: false, isWritable: true },
      { pubkey: reserveStake, isSigner: false, isWritable: true },
      { pubkey: destinationPoolAccount, isSigner: false, isWritable: true },
      { pubkey: managerFeeAccount, isSigner: false, isWritable: true },
      { pubkey: referralPoolAccount, isSigner: false, isWritable: true },
      { pubkey: poolMint, isSigner: false, isWritable: true },
      { pubkey: SYSVAR_CLOCK_PUBKEY, isSigner: false, isWritable: false },
      { pubkey: SYSVAR_STAKE_HISTORY_PUBKEY, isSigner: false, isWritable: false },
      { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
      { pubkey: StakeProgram.programId, isSigner: false, isWritable: false },
    ];

    return new TransactionInstruction({
      programId: programId ?? STAKE_POOL_PROGRAM_ID,
      keys,
      data,
    });
  }

  /**
   * Creates a transaction instruction to deposit SOL into a stake pool.
   */
  static depositSol(params: DepositSolParams): TransactionInstruction {
    const {
      programId,
      stakePool,
      withdrawAuthority,
      depositAuthority,
      reserveStake,
      fundingAccount,
      destinationPoolAccount,
      managerFeeAccount,
      referralPoolAccount,
      poolMint,
      lamports,
    } = params;

    const type = STAKE_POOL_INSTRUCTION_LAYOUTS.DepositSol;
    const data = encodeData(type, { lamports });

    const keys = [
      { pubkey: stakePool, isSigner: false, isWritable: true },
      { pubkey: withdrawAuthority, isSigner: false, isWritable: false },
      { pubkey: reserveStake, isSigner: false, isWritable: true },
      { pubkey: fundingAccount, isSigner: true, isWritable: true },
      { pubkey: destinationPoolAccount, isSigner: false, isWritable: true },
      { pubkey: managerFeeAccount, isSigner: false, isWritable: true },
      { pubkey: referralPoolAccount, isSigner: false, isWritable: true },
      { pubkey: poolMint, isSigner: false, isWritable: true },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
    ];

    if (depositAuthority) {
      keys.push({
        pubkey: depositAuthority,
        isSigner: true,
        isWritable: false,
      });
    }

    return new TransactionInstruction({
      programId: programId ?? STAKE_POOL_PROGRAM_ID,
      keys,
      data,
    });
  }

  /**
   * Helper function to build the deposit_wsol_with_session instruction
   */
  static buildDepositWsolWithSessionInstruction(params: {
    programId: PublicKey;
    signerOrSession: PublicKey;
    feePayer: PublicKey;
    programSigner: PublicKey;
    userWsolAccount: PublicKey;
    transientWsolPda: PublicKey;
    wsolMint: PublicKey;
    stakePool: PublicKey;
    withdrawAuthority: PublicKey;
    solDepositAuthority?: PublicKey;
    reserveStake: PublicKey;
    destinationPoolAccount: PublicKey;
    managerFeeAccount: PublicKey;
    referralPoolAccount: PublicKey;
    poolMint: PublicKey;
    tokenProgramId: PublicKey;
    lamports: number;
  }): TransactionInstruction {
    const keys = [
      // --- Core deposit SOL accounts (match DepositSol order) -----------
      /* 0  */ { pubkey: params.stakePool, isSigner: false, isWritable: true },
      /* 1  */ { pubkey: params.withdrawAuthority, isSigner: false, isWritable: false },
      /* 2  */ { pubkey: params.reserveStake, isSigner: false, isWritable: true },
      /* 3  */ { pubkey: params.destinationPoolAccount, isSigner: false, isWritable: true },
      /* 4  */ { pubkey: params.managerFeeAccount, isSigner: false, isWritable: true },
      /* 5  */ { pubkey: params.referralPoolAccount, isSigner: false, isWritable: true },
      /* 6  */ { pubkey: params.poolMint, isSigner: false, isWritable: true },
      /* 7  */ { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      /* 8  */ { pubkey: params.tokenProgramId, isSigner: false, isWritable: false },
      
      // --- Extra accounts for WSOL ATA creation / validation ------------
      /* 9  */ { pubkey: params.signerOrSession, isSigner: true, isWritable: true },
      /* 10 */ { pubkey: params.wsolMint, isSigner: false, isWritable: false },
      /* 11 */ { pubkey: params.feePayer, isSigner: true, isWritable: true },
      /* 12 */ { pubkey: params.userWsolAccount, isSigner: false, isWritable: true },
      /* 13 */ { pubkey: params.transientWsolPda, isSigner: false, isWritable: true },
      /* 14 */ { pubkey: params.programSigner, isSigner: false, isWritable: true },
    ];

    if (params.solDepositAuthority) {
      keys.push({
        pubkey: params.solDepositAuthority,
        isSigner: true,
        isWritable: false,
      });
    }

    // Encode the instruction data
    // This should match your Rust enum variant for DepositWsolWithSession
    // Adjust the discriminator based on your actual instruction enum
    const type = STAKE_POOL_INSTRUCTION_LAYOUTS.DepositWsolWithSession;
    const data = encodeData(type, { lamports: params.lamports });

    return new TransactionInstruction({
      keys,
      programId: params.programId,
      data,
    });
  }
  /**
   * Creates a transaction instruction to withdraw active stake from a stake pool.
   */
  static withdrawStake(params: WithdrawStakeParams): TransactionInstruction {
    const {
      programId,
      stakePool,
      validatorList,
      withdrawAuthority,
      validatorStake,
      destinationStake,
      destinationStakeAuthority,
      sourceTransferAuthority,
      sourcePoolAccount,
      managerFeeAccount,
      poolMint,
      poolTokens,
    } = params;

    const type = STAKE_POOL_INSTRUCTION_LAYOUTS.WithdrawStake;
    const data = encodeData(type, { poolTokens });

    const keys = [
      { pubkey: stakePool, isSigner: false, isWritable: true },
      { pubkey: validatorList, isSigner: false, isWritable: true },
      { pubkey: withdrawAuthority, isSigner: false, isWritable: false },
      { pubkey: validatorStake, isSigner: false, isWritable: true },
      { pubkey: destinationStake, isSigner: false, isWritable: true },
      { pubkey: destinationStakeAuthority, isSigner: false, isWritable: false },
      { pubkey: sourceTransferAuthority, isSigner: true, isWritable: false },
      { pubkey: sourcePoolAccount, isSigner: false, isWritable: true },
      { pubkey: managerFeeAccount, isSigner: false, isWritable: true },
      { pubkey: poolMint, isSigner: false, isWritable: true },
      { pubkey: SYSVAR_CLOCK_PUBKEY, isSigner: false, isWritable: false },
      { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
      { pubkey: StakeProgram.programId, isSigner: false, isWritable: false },
    ];

    return new TransactionInstruction({
      programId: programId ?? STAKE_POOL_PROGRAM_ID,
      keys,
      data,
    });
  }

  /**
   * Creates a transaction instruction to withdraw SOL from a stake pool.
   */
  static withdrawSol(params: WithdrawSolParams): TransactionInstruction {
    const {
      programId,
      stakePool,
      withdrawAuthority,
      sourceTransferAuthority,
      sourcePoolAccount,
      reserveStake,
      destinationSystemAccount,
      managerFeeAccount,
      solWithdrawAuthority,
      poolMint,
      poolTokens,
    } = params;

    const type = STAKE_POOL_INSTRUCTION_LAYOUTS.WithdrawSol;
    const data = encodeData(type, { poolTokens });

    const keys = [
      { pubkey: stakePool, isSigner: false, isWritable: true },
      { pubkey: withdrawAuthority, isSigner: false, isWritable: false },
      { pubkey: sourceTransferAuthority, isSigner: true, isWritable: false },
      { pubkey: sourcePoolAccount, isSigner: false, isWritable: true },
      { pubkey: reserveStake, isSigner: false, isWritable: true },
      { pubkey: destinationSystemAccount, isSigner: false, isWritable: true },
      { pubkey: managerFeeAccount, isSigner: false, isWritable: true },
      { pubkey: poolMint, isSigner: false, isWritable: true },
      { pubkey: SYSVAR_CLOCK_PUBKEY, isSigner: false, isWritable: false },
      { pubkey: SYSVAR_STAKE_HISTORY_PUBKEY, isSigner: false, isWritable: false },
      { pubkey: StakeProgram.programId, isSigner: false, isWritable: false },
      { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
    ];

    if (solWithdrawAuthority) {
      keys.push({
        pubkey: solWithdrawAuthority,
        isSigner: true,
        isWritable: false,
      });
    }

    return new TransactionInstruction({
      programId: programId ?? STAKE_POOL_PROGRAM_ID,
      keys,
      data,
    });
  }

  /**
   * Helper function to build the withdraw_wsol_with_session instruction
   */
  static buildWithdrawWsolWithSessionInstruction(
    params: WithdrawWsolWithSessionParams,
  ): TransactionInstruction {
    const keys = [
      { pubkey: params.stakePool, isSigner: false, isWritable: true },
      { pubkey: params.withdrawAuthority, isSigner: false, isWritable: false },
      { pubkey: params.userTransferAuthority, isSigner: true, isWritable: true },
      { pubkey: params.poolTokensFrom, isSigner: false, isWritable: true },
      { pubkey: params.reserveStake, isSigner: false, isWritable: true },
      { pubkey: params.userWsolAccount, isSigner: false, isWritable: true },
      { pubkey: params.managerFeeAccount, isSigner: false, isWritable: true },
      { pubkey: params.poolMint, isSigner: false, isWritable: true },
      { pubkey: SYSVAR_CLOCK_PUBKEY, isSigner: false, isWritable: false },
      { pubkey: SYSVAR_STAKE_HISTORY_PUBKEY, isSigner: false, isWritable: false },
      { pubkey: StakeProgram.programId, isSigner: false, isWritable: false },
      { pubkey: params.tokenProgramId, isSigner: false, isWritable: false },
    ];

    keys.push(
      { pubkey: params.wsolMint, isSigner: false, isWritable: false },
      { pubkey: params.feePayer, isSigner: true, isWritable: true },
      { pubkey: params.userOwner, isSigner: false, isWritable: false },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      { pubkey: params.programSigner, isSigner: false, isWritable: true },
    );

    // Optional SOL withdraw authority (needs to be at the end since it is not always present)
    if (params.solWithdrawAuthority) {
      keys.push({
        pubkey: params.solWithdrawAuthority,
        isSigner: true,
        isWritable: false,
      });
    }

    const type = STAKE_POOL_INSTRUCTION_LAYOUTS.WithdrawWsolWithSession;
    const data = encodeData(type, { poolTokens: params.poolTokens });

    return new TransactionInstruction({
      keys,
      programId: params.programId,
      data,
    });
  }

  /**
   * Creates an instruction to create metadata
   * using the mpl token metadata program for the pool token
   */
  static createTokenMetadata(params: CreateTokenMetadataParams): TransactionInstruction {
    const {
      programId,
      stakePool,
      withdrawAuthority,
      tokenMetadata,
      manager,
      payer,
      poolMint,
      name,
      symbol,
      uri,
    } = params;

    const keys = [
      { pubkey: stakePool, isSigner: false, isWritable: false },
      { pubkey: manager, isSigner: true, isWritable: false },
      { pubkey: withdrawAuthority, isSigner: false, isWritable: false },
      { pubkey: poolMint, isSigner: false, isWritable: false },
      { pubkey: payer, isSigner: true, isWritable: true },
      { pubkey: tokenMetadata, isSigner: false, isWritable: true },
      { pubkey: METADATA_PROGRAM_ID, isSigner: false, isWritable: false },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      { pubkey: SYSVAR_RENT_PUBKEY, isSigner: false, isWritable: false },
    ];

    const type = tokenMetadataLayout(17, name.length, symbol.length, uri.length);
    const data = encodeData(type, {
      nameLen: name.length,
      name: Buffer.from(name),
      symbolLen: symbol.length,
      symbol: Buffer.from(symbol),
      uriLen: uri.length,
      uri: Buffer.from(uri),
    });

    return new TransactionInstruction({
      programId: programId ?? STAKE_POOL_PROGRAM_ID,
      keys,
      data,
    });
  }

  /**
   * Creates an instruction to update metadata
   * in the mpl token metadata program account for the pool token
   */
  static updateTokenMetadata(params: UpdateTokenMetadataParams): TransactionInstruction {
    const { programId, stakePool, withdrawAuthority, tokenMetadata, manager, name, symbol, uri } =
      params;

    const keys = [
      { pubkey: stakePool, isSigner: false, isWritable: false },
      { pubkey: manager, isSigner: true, isWritable: false },
      { pubkey: withdrawAuthority, isSigner: false, isWritable: false },
      { pubkey: tokenMetadata, isSigner: false, isWritable: true },
      { pubkey: METADATA_PROGRAM_ID, isSigner: false, isWritable: false },
    ];

    const type = tokenMetadataLayout(18, name.length, symbol.length, uri.length);
    const data = encodeData(type, {
      nameLen: name.length,
      name: Buffer.from(name),
      symbolLen: symbol.length,
      symbol: Buffer.from(symbol),
      uriLen: uri.length,
      uri: Buffer.from(uri),
    });

    return new TransactionInstruction({
      programId: programId ?? STAKE_POOL_PROGRAM_ID,
      keys,
      data,
    });
  }

  /**
   * Decode a deposit stake pool instruction and retrieve the instruction params.
   */
  static decodeDepositStake(instruction: TransactionInstruction): DepositStakeParams {
    this.checkProgramId(instruction.programId);
    this.checkKeyLength(instruction.keys, 11);

    decodeData(STAKE_POOL_INSTRUCTION_LAYOUTS.DepositStake, instruction.data);

    return {
      programId: instruction.programId,
      stakePool: instruction.keys[0].pubkey,
      validatorList: instruction.keys[1].pubkey,
      depositAuthority: instruction.keys[2].pubkey,
      withdrawAuthority: instruction.keys[3].pubkey,
      depositStake: instruction.keys[4].pubkey,
      validatorStake: instruction.keys[5].pubkey,
      reserveStake: instruction.keys[6].pubkey,
      destinationPoolAccount: instruction.keys[7].pubkey,
      managerFeeAccount: instruction.keys[8].pubkey,
      referralPoolAccount: instruction.keys[9].pubkey,
      poolMint: instruction.keys[10].pubkey,
    };
  }

  /**
   * Decode a deposit sol instruction and retrieve the instruction params.
   */
  static decodeDepositSol(instruction: TransactionInstruction): DepositSolParams {
    this.checkProgramId(instruction.programId);
    this.checkKeyLength(instruction.keys, 9);

    const { amount } = decodeData(STAKE_POOL_INSTRUCTION_LAYOUTS.DepositSol, instruction.data);

    return {
      programId: instruction.programId,
      stakePool: instruction.keys[0].pubkey,
      depositAuthority: instruction.keys[1].pubkey,
      withdrawAuthority: instruction.keys[2].pubkey,
      reserveStake: instruction.keys[3].pubkey,
      fundingAccount: instruction.keys[4].pubkey,
      destinationPoolAccount: instruction.keys[5].pubkey,
      managerFeeAccount: instruction.keys[6].pubkey,
      referralPoolAccount: instruction.keys[7].pubkey,
      poolMint: instruction.keys[8].pubkey,
      lamports: amount,
    };
  }

  /**
   * @internal
   */
  private static checkProgramId(programId: PublicKey) {
    if (
      !programId.equals(STAKE_POOL_PROGRAM_ID) &&
      !programId.equals(DEVNET_STAKE_POOL_PROGRAM_ID)
    ) {
      throw new Error('Invalid instruction; programId is not the stake pool program');
    }
  }

  /**
   * @internal
   */
  private static checkKeyLength(keys: Array<any>, expectedLength: number) {
    if (keys.length < expectedLength) {
      throw new Error(
        `Invalid instruction; found ${keys.length} keys, expected at least ${expectedLength}`,
      );
    }
  }
}
