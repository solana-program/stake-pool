from solders.pubkey import Pubkey
from solders.keypair import Keypair
from solana.rpc.async_api import AsyncClient
from solana.rpc.commitment import Confirmed
from solana.rpc.models import TxOpts
from solders.transaction import Transaction, VersionedTransaction
import solders.system_program as sys

from spl.token.constants import TOKEN_PROGRAM_ID
from spl.token._layouts import MINT_LAYOUT
from spl.token.models import InitializeMintParams
import spl.token.instructions as spl_token


OPTS = TxOpts(skip_confirmation=False, preflight_commitment=Confirmed)


async def create_associated_token_account(
    client: AsyncClient,
    payer: Keypair,
    owner: Pubkey,
    mint: Pubkey
) -> Pubkey:
    recent_blockhash = (await client.get_latest_blockhash()).value.blockhash
    ix = spl_token.create_associated_token_account(
            payer=payer.pubkey(), owner=owner, mint=mint
        )
    txn = VersionedTransaction.from_legacy(Transaction.new_signed_with_payer(
        [ix],
        signing_keypairs=[payer],
        payer=payer.pubkey(),
        recent_blockhash=recent_blockhash
    ))
    await client.send_transaction(txn, opts=OPTS)
    return ix.accounts[1].pubkey


async def create_mint(client: AsyncClient, payer: Keypair, mint: Keypair, mint_authority: Pubkey):
    print(f"Creating pool token mint {mint.pubkey()}")
    mint_balance = (await client.get_minimum_balance_for_rent_exemption(82)).value
    recent_blockhash = (await client.get_latest_blockhash()).value.blockhash
    txn = VersionedTransaction.from_legacy(Transaction.new_signed_with_payer(
        [
            sys.create_account(
                sys.CreateAccountParams(
                    from_pubkey=payer.pubkey(),
                    to_pubkey=mint.pubkey(),
                    lamports=mint_balance,
                    space=MINT_LAYOUT.sizeof(),
                    owner=TOKEN_PROGRAM_ID,
                )
            ),
            spl_token.initialize_mint(
                InitializeMintParams(
                    program_id=TOKEN_PROGRAM_ID,
                    mint=mint.pubkey(),
                    decimals=9,
                    mint_authority=mint_authority,
                    freeze_authority=None,
                )
            )
        ],
        payer=payer.pubkey(),
        recent_blockhash=recent_blockhash,
        signing_keypairs=[payer, mint],
    ))
    await client.send_transaction(txn, opts=OPTS)
