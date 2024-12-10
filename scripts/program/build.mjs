#!/usr/bin/env zx
import 'zx/globals';
import {
  cliArguments,
  getProgramFolders,
  workingDirectory,
} from '../utils.mjs';

// Build the programs.
for (const folder of getProgramFolders()) {
  const manifestPath = path.join(workingDirectory, folder, 'Cargo.toml');
  await $`cargo-build-sbf --manifest-path ${manifestPath} ${cliArguments()}`;
}
