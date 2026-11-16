# Proposal

## Why

2ce46283 read eight options of pnpm with a value, as the help of pnpm 12.18.0 gives them: `--registry`, `--store-dir`,
`--state-dir`, `--npmrc-auth-file`, `--userconfig`, `--https-proxy`, `--http-proxy`, `--no-proxy`. Before a script
pnpm 10.33.0 reads each as a flag: `pnpm --no-proxy e2e` runs `e2e` there, and the classifier took it for a light run
on a host whose repository runs pnpm 10. Its comment said that no option diverges between the two.

## What Changes

- The eight leave both tables for `PNPM_DIVERGING_OPTIONS`, read both ways: as a flag and with a value; the reading
  that makes the run heavy counts.
- The test that keeps the tables apart covers the three of them.
- Capability `cli/host`.
