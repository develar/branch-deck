# A shell that wraps a heavy command is a heavy run

## Why

`brd-host run studio -- zsh -c '…; pnpm e2e …'` matched no pattern of `heavy`, which reads the command line alone, so a
full E2E run of an agent started beside the run that held the heavy turn of the Studio (2026-11-15).

## What Changes

- The client reads what a shell of the command runs: the script of `-c` (`-lc`, after `-o pipefail`), split at the
  separators of the shell, a shell in it, and a script file of the shipped working tree; a command in them that a
  pattern matches makes the run heavy (`by `<pattern>` of [host] in its script`).
- A script file that the client cannot read from the working tree counts as a heavy run, and says so.
- Capability `cli/host`.
