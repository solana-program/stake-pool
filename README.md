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

## Create/Update Token Metadata

We save the token logo and the metadata json file in this repo's `static` folder (currently we use the branch `wsol-adaptor`, which might be changed in the future).

To create the metadata, run the following command:

```
# use the pool's manager identity
spl-stake-pool \
  --program-id SPRe2ae9JQhySheYsSANX6M8tUZLt5bQonnBJ6Wu6Ud \
  create-token-metadata 4yoj9HDiL2pujuh2ME5MJJ6roLseTAkFqLmA4SrG7Yi9 \
  "FOGO LST Token" \
  "stFOGO" \
  "https://raw.githubusercontent.com/Firstset/stake-pool-v2/refs/heads/wsol-adaptor/static/brasa-metadata.json"
```

To update the metadata, first edit the `brasa-metadata.json` file in `static` folder and remember to push the changes. Then run the following command:

```
spl-stake-pool \
  --program-id SPRe2ae9JQhySheYsSANX6M8tUZLt5bQonnBJ6Wu6Ud \
  update-token-metadata 4yoj9HDiL2pujuh2ME5MJJ6roLseTAkFqLmA4SrG7Yi9 \
  "FOGO LST Token" \
  "stFOGO" \
  "https://raw.githubusercontent.com/Firstset/stake-pool-v2/refs/heads/wsol-adaptor/static/brasa-metadata.json"
```

