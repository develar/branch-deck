# Proposal

## Why

51d1a445 read every long option of pnpm without `=` that its value options lack as a setting of `.npmrc` with its value,
the documented flags included. A flag then took the script as its value, and the first argument of a light script
became the script: `pnpm --silent test:unit e2e-pins` matched `pnpm e2e*`, `pnpm --no-bail test:unit perf:diff`
matched `pnpm perf:*`, and a false heavy run held or waited for the heavy turn of the host.

## What Changes

- The value options and the flags of pnpm come from its own help: `pnpm --help`, `pnpm run --help` and
  `pnpm exec --help` of pnpm 12.18.0, the `packageManager` of package.json, and the run options of pnpm 10
  (https://pnpm.io/10.x/cli/run) for a repository of a host on an older pnpm (`--silent`, `--resume-from`).
- A flag (`--silent`, `--if-present`, `--color`, `--yes`, `--dry-run`, …) and each `--no-…` but `--no-proxy <NO_PROXY>`
  take no value; only a long option outside both lists counts also as a setting with its value.
- `PNPM_OPTIONS_CHECKED_WITH` names the version of pnpm the tables were checked against, and a test holds it equal to
  `packageManager`, so an update of pnpm asks for the tables to be checked again; another test keeps the two tables
  apart.
- Capability `cli/host`.
