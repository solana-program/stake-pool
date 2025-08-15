# stake-pool program

Full documentation is available at <https://spl.solana.com/stake-pool>

The command-line interface tool is available in the `./cli` directory.

Javascript bindings are available in the `./js` directory.

Python bindings are available in the `./py` directory.

## Audit

The repository [README](https://github.com/solana-labs/solana-program-library#audits)
contains information about program audits.

## Development

### Program

```
cd program
cargo build-sbf
solana program deploy -u $RPC_URL --program-id $PROGRAM_ID ../target/deploy/spl_stake_pool.so
```

## JS SDK

```
cd clients/js-legacy
pnpm run build
```

For local development, you need to import it as a dependency like this: `"@solana/spl-stake-pool": "file:../stake-pool-v2/clients/js-legacy",`

## CLI

```
cd clients/cli
cargo install --path . --locked
# deposit with session
spl-stake-pool --url https://testnet.fogo.io --program-id SPRe2ae9JQhySheYsSANX6M8tUZLt5bQonnBJ6Wu6Ud deposit-wsol-with-session 4yoj9HDiL2pujuh2ME5MJJ6roLseTAkFqLmA4SrG7Yi9 0.1
# withdraw with session
spl-stake-pool --url https://testnet.fogo.io --program-id SPRe2ae9JQhySheYsSANX6M8tUZLt5bQonnBJ6Wu6Ud withdraw-wsol-with-session 4yoj9HDiL2pujuh2ME5MJJ6roLseTAkFqLmA4SrG7Yi9 0.1
```