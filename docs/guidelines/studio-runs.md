# Runs on the Mac Studio (`yonder run studio`)

**Status:** Active  
**Updated:** 2026-10-27  
**Tags:** [tests, e2e, nextest, studio, yonder]

## Summary

This Mac runs several agents at once; under that load a full `cargo nextest run --workspace` takes minutes longer
and a full `pnpm e2e` fails on timeouts. The heavy checks run on the Mac Studio, the user's second Mac, through
yonder (the client `~/.local/bin/yonder`, the agent `yonderd` there; its source and the format of `.yonder.toml` are
in `community/tools/yonder` of the IntelliJ monorepo, and the `mac-studio` skill says how it is reached). A run ships
the working tree as it is, committed or not, runs the command in a warm host slot that the Studio lends to that run, streams its
output here and exits with the command's exit code.

```bash
yonder run studio -- cargo nextest run --workspace          # the Rust suite
yonder run studio -- pnpm e2e                               # the whole E2E suite
yonder run studio -- pnpm e2e tests/e2e/branch-row.spec.ts  # one spec
yonder run studio -- env DEBUG_E2E=1 pnpm e2e tests/e2e/x.spec.ts
yonder run studio --apply -- pnpm lint:backend              # the edits of cargo fmt and clippy --fix come back here
yonder run studio --apply -- pnpm e2e-up                    # the rewritten snapshots come back here
```

## Use

- **Where**: in a lease (`bd lease get`), as every edit and build here. The Studio keeps four warm host slots of the
  repository (`bd lease host`, "Host slots" of [bd lease](../../spec/cli/lease.spec.md)) and lends one to each run for
  that run only: the slot this checkout used last, else the one used longest ago; with all four lent, a run waits
  and says so once a minute. Nothing is kept or given back by hand.
- **What this repository asks of the host** is `.yonder.toml` in its root: the `outputs` that come back into the same
  paths here (`tests/results`, `playwright-report`, the nextest junit), `E2E_PORT` per slot, and `pnpm install` when
  the lockfile or package.json changed. A step and the command run in the login environment of the user there, with
  65536 open files and `nice` 10 (`--nice <n>`).
- **Exit codes**: the command's own; 75 when the Studio did not run it (unreachable, a refusal before the command):
  run the same command here then; 130 after Ctrl+C, which cancels the run there.
- **Output**: stderr and stdout stream as they come, and the run ends with `yonder: phases: … command …, total …`:
  read that line before you guess where the time went.

## Measured

On 2026-10-24, Studio load 4 to 22: `cargo nextest run --workspace` 4 min 7 s in a new slot (243.7 s of tests),
`pnpm e2e` 2 min 34 s, an `--apply` of `cargo fmt` 2.1 s, a rerun of the same tree sends no pack.
