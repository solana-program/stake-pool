#!/usr/bin/env zx
import 'zx/globals';
import {
  cliArguments,
  getProgramFolders,
  workingDirectory,
} from '../utils.mjs';

const hasSolfmt = await which('solfmt', { nothrow: true });

// Test the programs.
for (const folder of getProgramFolders()) {
  const manifestPath = path.join(workingDirectory, folder, 'Cargo.toml');

  if (hasSolfmt) {
    await $`RUST_LOG=error cargo test-sbf --manifest-path ${manifestPath} ${cliArguments()} 2>&1 | solfmt`;
  } else {
    await $`RUST_LOG=error cargo test-sbf --manifest-path ${manifestPath} ${cliArguments()}`;
  }
}
