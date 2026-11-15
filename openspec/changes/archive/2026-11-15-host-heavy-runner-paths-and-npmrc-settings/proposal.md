# Proposal

## Why

The review of the heavy classifier found three ways of writing a run of the heavy suites that it took for light ones:
a runner by its path (`/usr/local/bin/npx playwright test`), a runner option with its value
(`npx -p @playwright/test playwright test`), and a setting of `.npmrc` passed to pnpm with its value in the next word
(`pnpm --registry https://… e2e`), whose value it read as the script.

## What Changes

- The runners before `playwright test` count by their base name, with their options and the values of those.
- A long option of pnpm that the list of its documented value options lacks counts both ways: as a flag, and as a
  setting with its value in the next word; a pattern that matches either reading makes the run heavy.
- Capability `cli/host`.
