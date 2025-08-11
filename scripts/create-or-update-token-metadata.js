import { readFileSync } from "fs";
import { Keypair, PublicKey } from "@solana/web3.js";
import { createUmi } from "@metaplex-foundation/umi-bundle-defaults";
import { keypairIdentity } from "@metaplex-foundation/umi";
import { createMetadataAccountV3 } from "@metaplex-foundation/mpl-token-metadata";

// CLI args: node script.js <KEYPAIR_PATH> <RPC_ENDPOINT> <POOL_MINT> <METADATA_JSON>
const [, , keypairPath, rpcEndpoint, poolMintStr, metadataPath] = process.argv;

if (!keypairPath || !rpcEndpoint || !poolMintStr || !metadataPath) {
  console.error(
    "Usage: node script.js <KEYPAIR_PATH> <RPC_ENDPOINT> <POOL_MINT> <METADATA_JSON>",
  );
  process.exit(1);
}

// 1) Load local keypair
const secretKey = Uint8Array.from(
  JSON.parse(readFileSync(keypairPath, "utf-8")),
);
const wallet = Keypair.fromSecretKey(secretKey);

// 2) Connect to the specified RPC endpoint
const umi = createUmi(rpcEndpoint);
umi.use(keypairIdentity(wallet));

// 3) Parse pool mint address
const poolMint = new PublicKey(poolMintStr);

// 4) Load metadata from JSON file
const metadata = JSON.parse(readFileSync(metadataPath, "utf-8"));

async function main() {
  console.log(`Connecting to ${rpcEndpoint}`);
  console.log(`Using wallet: ${wallet.publicKey.toBase58()}`);
  console.log(`Pool mint: ${poolMint.toBase58()}`);
  console.log(`Metadata:`, metadata);

  // Create metadata (will throw if already exists)
  const tx = await createMetadataAccountV3(umi, {
    mint: poolMint,
    updateAuthority: umi.identity.publicKey,
    data: metadata,
    isMutable: true,
  }).sendAndConfirm(umi);

  console.log("Transaction sent:", tx.signature);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
