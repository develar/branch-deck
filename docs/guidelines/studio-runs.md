# Runs on the Mac Studio (`bd-host run studio`)

**Status:** Active  
**Updated:** 2026-10-28  
**Tags:** [tests, e2e, nextest, studio, bd-host, yonder]

## Summary

This Mac runs several agents at once; under that load a full `cargo nextest run --workspace` takes minutes longer
and a full `pnpm e2e` fails on timeouts. The heavy checks run on the Mac Studio, the user's second Mac, through
`bd-host` ([bd-host](../../spec/cli/host.spec.md), [Host Runs](../design/host-runs.md)): the client
`~/.local/bin/bd-host` here, which `bd install` links to the app bundle, and `bd-host serve` there. A run ships the
working tree as it is, committed or not, runs the command in a warm host slot that the Studio lends to that run,
streams its output here and exits with the command's exit code. `bd-host skill install` writes the skill of the
binary, with the spec as its reference.

```bash
bd-host run studio -- cargo nextest run --workspace          # the Rust suite
bd-host run studio -- pnpm e2e                               # the whole E2E suite
bd-host run studio -- pnpm e2e tests/e2e/branch-row.spec.ts  # one spec
bd-host run studio -- env DEBUG_E2E=1 pnpm e2e tests/e2e/x.spec.ts
bd-host run studio --apply -- pnpm lint:backend              # the edits of cargo fmt and clippy --fix come back here
bd-host run studio --apply -- pnpm e2e-up                    # the rewritten snapshots come back here
```

## The migration from yonder

yonder (the client `~/.local/bin/yonder`, the agent `yonderd` on port 7741, `community/tools/yonder` of the IntelliJ
monorepo) did these runs before. Both agents serve the Studio side by side and lend from its one lend table, so a run
of either one waits for the same slots. Until the user ran `bd-host setup --import-yonder` on this Mac and on the
Studio, `bd-host run studio` refuses with `no_host` (75): run `yonder run studio` with the same arguments then, never
the heavy command here. The arguments are the same, but `--holder` is `--affinity` in bd-host.

## Use

- **Where**: in a lease (`bd lease get`), as every edit and build here. The Studio keeps four warm host slots of the
  repository ("Host slots" of [bd lease](../../spec/cli/lease.spec.md)) and lends one to each run for that run only:
  the slot this checkout used last, else the one used longest ago; with all four lent, a run waits and says so once a
  minute. Nothing is kept or given back by hand. `bd-host slots studio` shows the lend table.
- **What this repository asks of the host** is `.yonder.toml` in its root, which bd-host reads while
  `.branchdeck.toml` has no `[host]` table: the `outputs` that come back into the same paths here (`tests/results`,
  `playwright-report`, the nextest junit), `E2E_PORT` per slot, and `pnpm install` when the lockfile or package.json
  changed. A step and the command run in the login environment of the user there, with 65536 open files and `nice` 10
  (`--nice <n>`).
- **Exit codes**: the command's own; 75 when the Studio did not run it (unreachable, a refusal before the command):
  run the same command here then, except for `no_host` (above); 130 after Ctrl+C, which cancels the run there.
  `bd-host schema` lists every refusal code with its exit code.
- **Output**: stdout is only the stdout of the command. stderr has the lines of bd-host, and the run ends with
  `bd-host: phases: … command …, total …`: read that line before you guess where the time went.

## Measured

On 2026-10-24, with yonder, Studio load 4 to 22: `cargo nextest run --workspace` 4 min 7 s in a new slot (243.7 s of
tests), `pnpm e2e` 2 min 34 s, an `--apply` of `cargo fmt` 2.1 s, a rerun of the same tree sends no pack. bd-host has no
measurement on the Studio yet: no `bd-host serve` runs there.
