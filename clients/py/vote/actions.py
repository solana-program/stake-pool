from solders.pubkey import Pubkey
from solders.keypair import Keypair
from solana.rpc.async_api import AsyncClient
from solana.rpc.commitment import Confirmed
from solana.rpc.types import TxOpts
from solders.sysvar import CLOCK, RENT
from solders.transaction import Transaction
import solders.system_program as sys

from vote.constants import VOTE_PROGRAM_ID, VOTE_STATE_LEN
from vote.instructions import initialize, InitializeParams


async def create_vote(
        client: AsyncClient, payer: Keypair, vote: Keypair, node: Keypair,
        voter: Pubkey, withdrawer: Pubkey, commission: int):
    print(f"Creating vote account {vote.pubkey()}")
    resp = await client.get_minimum_balance_for_rent_exemption(VOTE_STATE_LEN)
    recent_blockhash = (await client.get_latest_blockhash()).value.blockhash
    txn = Transaction.new_signed_with_payer(
        [
            sys.create_account(
                sys.CreateAccountParams(
                    from_pubkey=payer.pubkey(),
                    to_pubkey=vote.pubkey(),
                    lamports=resp.value,
                    space=VOTE_STATE_LEN,
                    owner=VOTE_PROGRAM_ID,
                )
            ),
            initialize(
                InitializeParams(
                    vote=vote.pubkey(),
                    rent_sysvar=RENT,
                    clock_sysvar=CLOCK,
                    node=node.pubkey(),
                    authorized_voter=voter,
                    authorized_withdrawer=withdrawer,
                    commission=commission,
                )
            )
        ],
        payer=payer.pubkey(),
        signing_keypairs=[payer, vote, node],
        recent_blockhash=recent_blockhash,
    )
    await client.send_transaction(
        txn,
        opts=TxOpts(skip_confirmation=False, preflight_commitment=Confirmed))
