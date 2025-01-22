from solders.pubkey import Pubkey
from solders.keypair import Keypair
import solders.system_program as sys
from solana.constants import SYSTEM_PROGRAM_ID
from solana.rpc.async_api import AsyncClient
from solana.rpc.commitment import Confirmed
from solana.rpc.types import TxOpts
from solders.sysvar import CLOCK, STAKE_HISTORY
from solders.transaction import Transaction

from stake.constants import STAKE_LEN, STAKE_PROGRAM_ID, SYSVAR_STAKE_CONFIG_ID
from stake.state import Authorized, Lockup, StakeAuthorize
import stake.instructions as st


OPTS = TxOpts(skip_confirmation=False, preflight_commitment=Confirmed)


async def create_stake(client: AsyncClient, payer: Keypair, stake: Keypair, authority: Pubkey, lamports: int):
    print(f"Creating stake {stake.pubkey()}")
    resp = await client.get_minimum_balance_for_rent_exemption(STAKE_LEN)
    recent_blockhash = (await client.get_latest_blockhash()).value.blockhash
    txn = Transaction.new_signed_with_payer(
        [
            sys.create_account(
                sys.CreateAccountParams(
                    from_pubkey=payer.pubkey(),
                    to_pubkey=stake.pubkey(),
                    lamports=resp.value + lamports,
                    space=STAKE_LEN,
                    owner=STAKE_PROGRAM_ID,
                )
            ),
            st.initialize(
                st.InitializeParams(
                    stake=stake.pubkey(),
                    authorized=Authorized(
                        staker=authority,
                        withdrawer=authority,
                    ),
                    lockup=Lockup(
                        unix_timestamp=0,
                        epoch=0,
                        custodian=SYSTEM_PROGRAM_ID,
                    )
                )
            )
        ],
        payer=payer.pubkey(),
        signing_keypairs=[payer, stake],
        recent_blockhash=recent_blockhash,
    )
    await client.send_transaction(txn, opts=OPTS)


async def delegate_stake(client: AsyncClient, payer: Keypair, staker: Keypair, stake: Pubkey, vote: Pubkey):
    recent_blockhash = (await client.get_latest_blockhash()).value.blockhash
    signers = [payer, staker] if payer.pubkey() != staker.pubkey() else [payer]
    txn = Transaction.new_signed_with_payer(
        [
            st.delegate_stake(
                st.DelegateStakeParams(
                    stake=stake,
                    vote=vote,
                    clock_sysvar=CLOCK,
                    stake_history_sysvar=STAKE_HISTORY,
                    stake_config_id=SYSVAR_STAKE_CONFIG_ID,
                    staker=staker.pubkey(),
                )
            )
        ],
        payer=payer.pubkey(),
        signing_keypairs=signers,
        recent_blockhash=recent_blockhash,
    )
    await client.send_transaction(txn, opts=OPTS)


async def authorize(
    client: AsyncClient, payer: Keypair, authority: Keypair, stake: Pubkey,
    new_authority: Pubkey, stake_authorize: StakeAuthorize
):
    signers = [payer, authority] if payer.pubkey() != authority.pubkey() else [payer]
    recent_blockhash = (await client.get_latest_blockhash()).value.blockhash
    txn = Transaction.new_signed_with_payer(
        [st.authorize(
            st.AuthorizeParams(
                stake=stake,
                clock_sysvar=CLOCK,
                authority=authority.pubkey(),
                new_authority=new_authority,
                stake_authorize=stake_authorize,
            )
        )],
        payer=payer.pubkey(),
        signing_keypairs=signers,
        recent_blockhash=recent_blockhash,
    )
    await client.send_transaction(txn, opts=OPTS)
