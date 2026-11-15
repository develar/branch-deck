# A pattern of heavy matches what a command runs

## Why

The header of `heavy` of this repository names every Playwright run heavy, but `npx playwright test x.spec.ts`,
`pnpm playwright test` and `pnpm -C web e2e` matched no pattern; a pattern per way of writing them is a list that the
next way slips by.

## What Changes

- A pattern matches a command by what it runs as well as by what it says: `pnpm` without its options before its
  script (`-C`, `--dir`, `--filter`, `-r`), and every run of `playwright test` (`npx`, `pnpm exec`, `pnpm`, `yarn`, a
  path of the binary) as `playwright test …`. A pattern written with its runner matches the line as written.
- `heavy` of `.branchdeck.toml` has `playwright test*` in place of `pnpm exec playwright test*`.
- Capability `cli/host`.
