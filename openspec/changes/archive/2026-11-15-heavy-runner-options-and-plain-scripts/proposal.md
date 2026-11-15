# The options of a runner, pnpm by its path, and a script without a first line

## Why

The review of f092c87b: `npx -y playwright test`, `/opt/homebrew/bin/pnpm -C . e2e` and `pnpm --loglevel error e2e`
were light, a text file run by its path without a first line `#!` (which a shell runs itself) was light, and a file
read once as no script was not read again when a shell ran it.

## What Changes

- The options of a runner before `playwright` (`npx -y`, `npx --yes`) and `pnpm` by its path count as `pnpm`;
  `--loglevel` names its value.
- A text file run by its path without a first line `#!` and without a NUL is a script; a binary is none.
- Each path is read once and expanded once: a file run as no script and then by a shell is expanded by the shell.
- Capability `cli/host`.
