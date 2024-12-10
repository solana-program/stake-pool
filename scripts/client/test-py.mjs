#!/usr/bin/env zx
import 'zx/globals';
import { cliArguments, workingDirectory } from '../utils.mjs';

// Build the client and run the tests.
cd(path.join(workingDirectory, 'clients', 'py'));
await $`python3 -m venv venv`;
await $`./venv/bin/pip3 install -r requirements.txt`;
await $`./venv/bin/pip3 install -r optional-requirements.txt`;
const checkDirs = [
  'bot',
  'spl_token',
  'stake',
  'stake_pool',
  'system',
  'tests',
  'vote',
];
await $`./venv/bin/flake8 ${checkDirs}`;
await $`./venv/bin/mypy ${checkDirs}`;
await $`./venv/bin/python3 -m pytest`;
