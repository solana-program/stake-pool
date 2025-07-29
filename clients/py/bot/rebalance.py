import argparse
import asyncio

import httpx
from solana.rpc.async_api import AsyncClient
from solana.rpc.commitment import Confirmed
from solana.rpc.providers import async_http
from solders.keypair import Keypair
from solders.pubkey import Pubkey
from stake.constants import LAMPORTS_PER_SOL, STAKE_LEN
from stake_pool.actions import (
    decrease_validator_stake,
    increase_validator_stake,
    update_stake_pool,
)
from stake_pool.constants import MINIMUM_ACTIVE_STAKE
from stake_pool.state import StakePool, ValidatorList


class InsecureAsyncHTTPProvider(async_http.AsyncHTTPProvider):
    def __init__(self, endpoint, timeout=10, extra_headers=None, proxy=None):
        super().__init__(endpoint, extra_headers=extra_headers)
        self.session = httpx.AsyncClient(timeout=timeout, proxy=proxy, verify=False)


class InsecureAsyncClient(AsyncClient):
    def __init__(
        self, endpoint, commitment=None, timeout=10, extra_headers=None, proxy=None
    ):
        super().__init__(endpoint, commitment, timeout, extra_headers, proxy)
        # Override the default provider with our custom one
        self._provider = InsecureAsyncHTTPProvider(
            endpoint, timeout, extra_headers, proxy
        )


async def get_client(endpoint: str) -> AsyncClient:
    print(f"Connecting to network at {endpoint}")
    async_client = InsecureAsyncClient(endpoint=endpoint, commitment=Confirmed)
    total_attempts = 10
    current_attempt = 0
    while not await async_client.is_connected():
        if current_attempt == total_attempts:
            raise Exception("Could not connect to test validator")
        else:
            current_attempt += 1
        await asyncio.sleep(1)
    return async_client


async def rebalance(
    endpoint: str,
    stake_pool_address: Pubkey,
    staker: Keypair,
    retained_reserve_amount: float,
):
    async_client = await get_client(endpoint)

    epoch_resp = await async_client.get_epoch_info(commitment=Confirmed)
    epoch = epoch_resp.value.epoch
    resp = await async_client.get_account_info(stake_pool_address, commitment=Confirmed)
    data = resp.value.data if resp.value else bytes()
    stake_pool = StakePool.decode(data)

    print(
        f"Stake pool last update epoch {stake_pool.last_update_epoch}, current epoch {epoch}"
    )
    if stake_pool.last_update_epoch != epoch:
        print("Updating stake pool")
        await update_stake_pool(async_client, staker, stake_pool_address)
        resp = await async_client.get_account_info(
            stake_pool_address, commitment=Confirmed
        )
        data = resp.value.data if resp.value else bytes()
        stake_pool = StakePool.decode(data)

    rent_resp = await async_client.get_minimum_balance_for_rent_exemption(STAKE_LEN)
    stake_rent_exemption = rent_resp.value
    retained_reserve_lamports = int(retained_reserve_amount * LAMPORTS_PER_SOL)

    val_resp = await async_client.get_account_info(
        stake_pool.validator_list, commitment=Confirmed
    )
    data = val_resp.value.data if val_resp.value else bytes()
    validator_list = ValidatorList.decode(data)

    print("Stake pool stats:")
    print(f"* {stake_pool.total_lamports} total lamports")
    num_validators = len(validator_list.validators)
    print(f"* {num_validators} validators")
    print(f"* Retaining {retained_reserve_lamports} lamports in the reserve")
    lamports_per_validator = (
        stake_pool.total_lamports - retained_reserve_lamports
    ) // num_validators
    num_increases = sum(
        [
            1
            for validator in validator_list.validators
            if validator.transient_stake_lamports == 0
            and validator.active_stake_lamports < lamports_per_validator
        ]
    )
    total_usable_lamports = (
        stake_pool.total_lamports
        - retained_reserve_lamports
        - num_increases * stake_rent_exemption
    )
    lamports_per_validator = total_usable_lamports // num_validators
    print(f"* {lamports_per_validator} lamports desired per validator")

    futures = []
    for validator in validator_list.validators:
        if validator.transient_stake_lamports != 0:
            print(
                f"Skipping {validator.vote_account_address}: {validator.transient_stake_lamports} transient lamports"
            )
        else:
            if validator.active_stake_lamports > lamports_per_validator:
                lamports_to_decrease = (
                    validator.active_stake_lamports - lamports_per_validator
                )
                if lamports_to_decrease <= stake_rent_exemption:
                    print(
                        f"Skipping decrease on {validator.vote_account_address}, \
currently at {validator.active_stake_lamports} lamports, \
decrease of {lamports_to_decrease} below the rent exmption"
                    )
                else:
                    futures.append(
                        decrease_validator_stake(
                            async_client,
                            staker,
                            staker,
                            stake_pool_address,
                            validator.vote_account_address,
                            lamports_to_decrease,
                        )
                    )
            elif validator.active_stake_lamports < lamports_per_validator:
                lamports_to_increase = (
                    lamports_per_validator - validator.active_stake_lamports
                )
                if lamports_to_increase < MINIMUM_ACTIVE_STAKE:
                    print(
                        f"Skipping increase on {validator.vote_account_address}, \
currently at {validator.active_stake_lamports} lamports, \
increase of {lamports_to_increase} less than the minimum of {MINIMUM_ACTIVE_STAKE}"
                    )
                else:
                    futures.append(
                        increase_validator_stake(
                            async_client,
                            staker,
                            staker,
                            stake_pool_address,
                            validator.vote_account_address,
                            lamports_to_increase,
                        )
                    )
            else:
                print(
                    f"{validator.vote_account_address}: already at {lamports_per_validator}"
                )

    print("Executing strategy")
    await asyncio.gather(*futures)
    print("Done")
    await async_client.close()


def keypair_from_file(keyfile_name: str) -> Keypair:
    with open(keyfile_name, "r") as keyfile:
        data = keyfile.read()
    return Keypair.from_json(data)


async def get_epoch_progress(async_client: AsyncClient) -> tuple[int, float]:
    epoch_resp = await async_client.get_epoch_info(commitment=Confirmed)
    epoch_info = epoch_resp.value
    progress = epoch_info.slot_index / epoch_info.slots_in_epoch
    return epoch_info.epoch, progress


async def service_mode(
    endpoint: str, stake_pool_address: Pubkey, staker: Keypair, reserve_amount: float
):
    async_client = await get_client(endpoint)
    current_epoch = None
    rebalanced_in_current_epoch = False
    print("Starting service mode - monitoring epoch progress...")

    while True:
        try:
            epoch, progress = await get_epoch_progress(async_client)

            print(f"Current epoch: {epoch}, progress: {progress:.2%}")

            if epoch != current_epoch:
                print(f"New epoch detected: {epoch} (previous: {current_epoch})")
                print(f"Updating stake pool for new epoch {epoch}")
                await update_stake_pool(async_client, staker, stake_pool_address)
                print(f"Updating stake pool for new epoch {epoch} done")
                current_epoch = epoch
                rebalanced_in_current_epoch = False

            if progress >= 0.95 and not rebalanced_in_current_epoch:
                print(f"Epoch {epoch} is {progress:.2%} complete - starting rebalance")
                await rebalance(endpoint, stake_pool_address, staker, reserve_amount)
                rebalanced_in_current_epoch = True
                print(f"Rebalance completed for epoch {epoch}")

            await asyncio.sleep(30)

        except Exception as e:
            print(f"Error in service mode: {e}")
            await asyncio.sleep(60)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(
        description="Rebalance stake evenly between all the validators in a stake pool."
    )
    parser.add_argument(
        "stake_pool",
        metavar="STAKE_POOL_ADDRESS",
        type=str,
        help="Stake pool to rebalance, given by a public key in base-58,\
                         e.g. Zg5YBPAk8RqBR9kaLLSoN5C8Uv7nErBz1WC63HTsCPR",
    )
    parser.add_argument(
        "staker",
        metavar="STAKER_KEYPAIR",
        type=str,
        help="Staker for the stake pool, given by a keypair file, e.g. staker.json",
    )
    parser.add_argument(
        "reserve_amount",
        metavar="RESERVE_AMOUNT",
        type=float,
        help="Amount of SOL to keep in the reserve, e.g. 10.5",
    )
    parser.add_argument(
        "--endpoint",
        metavar="ENDPOINT_URL",
        type=str,
        default="https://api.mainnet-beta.solana.com",
        help="RPC endpoint to use, e.g. https://api.mainnet-beta.solana.com",
    )
    parser.add_argument(
        "--service",
        action="store_true",
        help="Run in service mode with epoch-based periodic rebalancing",
    )

    args = parser.parse_args()
    stake_pool = Pubkey.from_string(args.stake_pool)
    staker = keypair_from_file(args.staker)
    print(f"Stake pool: {stake_pool}")
    print(f"Staker public key: {staker.pubkey()}")
    print(f"Amount to leave in the reserve: {args.reserve_amount} SOL")

    if args.service:
        print("Running in service mode")
        asyncio.run(
            service_mode(args.endpoint, stake_pool, staker, args.reserve_amount)
        )
    else:
        print("Running one-time rebalance")
        asyncio.run(rebalance(args.endpoint, stake_pool, staker, args.reserve_amount))
