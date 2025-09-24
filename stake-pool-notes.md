# Fogo Stake Pool Notes

## Compile

Two approaches, the vanilla program and our modified version that supports wSOL (wFOGO):

### Vanilla

Clone the repo:

```
git clone https://github.com/solana-program/stake-pool.git
```

Checkout the latest tag:

```
git checkout program@v2.0.2
```

Compile:

```
cd program
cargo build-sbf
```

Now we can find it in `../target/deploy`

### Modded version

Clone the repo:

```
git clone https://github.com/firstset/stake-pool.git
```

Compile:

```
cargo build-sbf
```

Now we can find it in `../target/deploy`

## Keypair

Use `solana keygen grind`. See our notes in the `stake-pool-vanity-keypair` folder for more context.

## Deploy

```
solana program deploy -u https://testnet.fogo.io --program-id ../../stake-pool-vanity-keypair/SPRe2ae9JQhySheYsSANX6M8tUZLt5bQonnBJ6Wu6Ud.json ../target/deploy/spl_stake_pool.so 
Program Id: SPRe2ae9JQhySheYsSANX6M8tUZLt5bQonnBJ6Wu6Ud

Signature: 2B5B78Ywfk2SLfixBryUt68gtoxa2UsaK7NtFJmrZqb1D1xbHveGo2XZyy5UwEMSnm2zhovdFZNK68L71dfDmghS
```
### Upgrades

If we have already deployed the program and we want to instead upgrade it, we simply re-deploy again to the same program id target. As long as we deploy with the same upgrade authority we should be able to do it. Since we didn't specify an explicit one, as long as we deploy with the same keypair, it should work. 

## Interact

Install the cli and configure with the program we deployed:

```
cd stake-pool/clients/cli
cargo install --path . --locked
spl-stake-pool --url https://testnet.fogo.io --program-id SPRe2ae9JQhySheYsSANX6M8tUZLt5bQonnBJ6Wu6Ud list-all
```

To create a pool we can do the following (not suitable for production, as we simplified the configuration here):

```
spl-stake-pool \
  --url https://testnet.fogo.io \
  --program-id SPRe2ae9JQhySheYsSANX6M8tUZLt5bQonnBJ6Wu6Ud \
  create-pool \
  --epoch-fee-numerator 3 \
  --epoch-fee-denominator 100 \
  --max-validators 100      \
  --unsafe-fees
```

Now we can list the pool:

```
spl-stake-pool --url https://testnet.fogo.io --program-id SPRe2ae9JQhySheYsSANX6M8tUZLt5bQonnBJ6Wu6Ud list-all

Address: 4yoj9HDiL2pujuh2ME5MJJ6roLseTAkFqLmA4SrG7Yi9	Manager: CB3zXZGXPVn2dDSJSy7Xr1ZK7fTvpa1545F3fLbFtXgV	Lamports: 0	Pool tokens: 0	Validators: 0
Total number of pools: 1
```

Now we need to deposit some FOGO in order to be able to perform other management actions (e.g. we found adding a validator while the pool had a balance of 0 failed):

```
spl-stake-pool --url https://testnet.fogo.io --program-id SPRe2ae9JQhySheYsSANX6M8tUZLt5bQonnBJ6Wu6Ud deposit-sol 4yoj9HDiL2pujuh2ME5MJJ6roLseTAkFqLmA4SrG7Yi9 1
Update not required
Using existing associated token account Eq2SMopCyS8TvBiWLoVs3D63vUH1LBDVHqukXB51LY57 to receive stake pool tokens of mint 6FzCV3CDRh7fkxdsJgevtVxU9t5bZ6jiJVYUNCk8eVU7, owned by CB3zXZGXPVn2dDSJSy7Xr1ZK7fTvpa1545F3fLbFtXgV
Signature: RM5nf3Hhd7fTmnW6mWUTpzGEKX5nairJtFEi7CTxv7aFJxkB5qEB6NifU9crjn3V6E5xps6FSzbJh8V97s1aYyU
```

And now let's add a validator:

```
spl-stake-pool --url https://testnet.fogo.io --program-id SPRe2ae9JQhySheYsSANX6M8tUZLt5bQonnBJ6Wu6Ud add-validator 4yoj9HDiL2pujuh2ME5MJJ6roLseTAkFqLmA4SrG7Yi9 GuyTHb4zpqQUh743qZhQKxrWKvuSQDEKn1vYC4WzQbfb
Update not required
Adding stake account 76hWMmYpzwRnQA6XbgUwSakmNHePgax9YF9BceTWKmq, delegated to GuyTHb4zpqQUh743qZhQKxrWKvuSQDEKn1vYC4WzQbfb
Signature: 54hehU64DjYmBQuWvxd4Xz9Myue7H3w8nF9UxfnoSpWdfNRM5x2WSBHPeiQtrp1Grn2KdswACuvF8n53wtfvQAwc
```

Now let's stake:

```
spl-stake-pool --url https://testnet.fogo.io --program-id SPRe2ae9JQhySheYsSANX6M8tUZLt5bQonnBJ6Wu6Ud deposit-wsol-with-session 4yoj9HDiL2pujuh2ME5MJJ6roLseTAkFqLmA4SrG7Yi9 1
```

## Update Pool Fees

To change the epoch fee after pool creation (e.g., from 3% to 5%):

```
spl-stake-pool --url https://testnet.fogo.io --program-id SPRe2ae9JQhySheYsSANX6M8tUZLt5bQonnBJ6Wu6Ud set-fee 4yoj9HDiL2pujuh2ME5MJJ6roLseTAkFqLmA4SrG7Yi9 epoch 5 100
```

Available fee types: `epoch`, `stake-deposit`, `sol-deposit`, `stake-withdrawal`, `sol-withdrawal`

## Wrapping SOL with External Keypairs

To wrap SOL from a wallet not configured in your CLI (using private key or mnemonic):

1. Create keypair file from mnemonic:
```
solana-keygen recover 'prompt://' --outfile temp_keypair.json
```

2. For existing wrapped SOL accounts, transfer native SOL and sync (most efficient):
```
solana transfer -u https://testnet.fogo.io -k temp_keypair.json <WSOL_TOKEN_ACCOUNT> <AMOUNT>
spl-token sync-native -u https://testnet.fogo.io --address <WSOL_TOKEN_ACCOUNT>
```

3. Clean up:
```
rm temp_keypair.json
```

**Alternative methods:**
- `spl-token wrap --create-aux-account <AMOUNT> temp_keypair.json` (creates new auxiliary account)
- Close existing account first: `spl-token close --address <WSOL_TOKEN_ACCOUNT> temp_keypair.json` then wrap

## Gathering pool info

Run `list`:

```
spl-stake-pool --url https://testnet.fogo.io --program-id SPRe2ae9JQhySheYsSANX6M8tUZLt5bQonnBJ6Wu6Ud list 4yoj9HDiL2pujuh2ME5MJJ6roLseTAkFqLmA4SrG7Yi9
```

Add the `-v` verbose option to print even more details.

## Deployments

| Network | Program ID | Pool ID |
|---------|------------|---------|
| testnet | SPRe2ae9JQhySheYsSANX6M8tUZLt5bQonnBJ6Wu6Ud | 4yoj9HDiL2pujuh2ME5MJJ6roLseTAkFqLmA4SrG7Yi9 |
| devnet  | SDbhNGbX66AFP9RPa3m8v1XooCCb5mbutk2NiVxdTw4 | 5wL3K4ACX3pZqg2Aq9cxxPCe9eSoc5c6dBGnhXaPkPMk |
