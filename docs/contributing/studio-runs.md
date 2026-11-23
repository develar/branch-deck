# Runs on the Mac Studio (`brd-host run studio`)

**Status:** Active  
**Updated:** 2026-11-23  
**Tags:** [tests, e2e, nextest, studio, brd-host]

## Summary

This Mac runs several agents at once; under that load a full `cargo nextest run --workspace` takes minutes longer
and a full `pnpm e2e` fails on timeouts. The heavy checks run on the Mac Studio, the user's second Mac, through
`brd-host` ([brd-host](../../openspec/specs/cli/host/spec.md), [Host Runs](../design/host-runs.md)): the client
`~/.local/bin/brd-host` here, which `brd install` links to the app bundle, and `brd-host serve` there. A run ships the
working tree as it is, committed or not, runs the command in a warm host slot that the Studio lends to that run,
streams its output here and exits with the command's exit code. `brd-host skill install` writes the skill of the
binary, with the spec as its reference. The setup of a host is in [Runs on Another Mac](../guide/host-runs.md).

```bash
brd-host run studio -- cargo nextest run --workspace          # the Rust suite
brd-host run studio -- pnpm e2e                               # the whole E2E suite
brd-host run studio -- pnpm e2e tests/e2e/branch-row.spec.ts  # one spec
brd-host run studio -- env DEBUG_E2E=1 pnpm e2e tests/e2e/x.spec.ts
brd-host run studio -- pnpm lint:backend                      # cargo fmt --check and clippy: a check, edits nothing
brd-host run studio --apply -- pnpm fix:backend               # the edits of cargo fmt and clippy --fix come back here
brd-host run studio --apply -- pnpm e2e-up                    # the rewritten snapshots come back here
brd-host run studio --heavy -- sh -c 'cargo build --release'   # a heavy command that `heavy` of [host] does not name
brd-host run studio --fetch notes.local -- zsh -c 'mkdir -p notes.local; cargo nextest run -p host-link > notes.local/nt-1.log 2>&1'
```

yonder of the IntelliJ monorepo did these runs until 2026-10-28, when brd-host took them over and yonderd was retired.

## Use

- **Logs**: a run writes its logs into `notes.local/` of the slot, a name of its own per run (`nt-<sha>-<n>.log`), and
  `--fetch notes.local` brings them into the notes of the lease, which `notes.local` here links to: the one link a
  fetch writes through, and only to the notes of a lease of this repository.
- **Where**: in a lease (`brd lease get`), as every edit and build here. The Studio keeps four warm host slots of the
  repository ("Host slots" of [brd lease](../../openspec/specs/cli/lease/spec.md)) and lends one to each run for that run only:
  the slot this checkout used last, else the one used longest ago; with all four lent, a run waits and says so once a
  minute. Nothing is kept or given back by hand. `brd-host slots studio` shows the lend table.
- **What this repository asks of the host** is `[host]` of `.branchdeck.toml` in its root: the `outputs` that come back into the same paths here (`tests/results`,
  `playwright-report`, the nextest junit), the `heavy` runs (the whole Rust suite, `cargo clippy --workspace`, every
  E2E and Playwright run, the measurements, the release build, and the scripts of package.json that run them),
  `E2E_PORT` per slot, and `pnpm install` when the lockfile or package.json changed. A light iteration on the Studio
  names its crates (`cargo nextest run -p <crate>`, also with `-E` or `-R latest`); `-E` or `-R latest` without `-p`
  builds the whole workspace and waits in the heavy queue. A step and the command run in the login environment of the user there, with 65536 open files and `nice` 10
  (`--nice <n>`).
- **Heavy runs share the cores**: a run that `heavy` of `[host]` names, or one with `--heavy`, waits until the cores
  that its command used in its last runs (the 80th percentile of the last 5, by its samples) fit beside the heavy runs
  of the Studio and the live use of the others, whatever its repository, so two full suites never slow each other into
  timeouts. A command without a measure yet, or one that measured all 20 cores, runs alone; the queue is first come
  first, and no run overtakes the head, which starts on a free turn after 15 min in the queue whatever the load of the
  runs that are not heavy. The paths of the `--fetch` directories are no part of the key of the measures, so a run that
  writes to `out/air/light5` counts the measure of `out/air/light1`. The end of a run says `brd-host: the command used <mean> cores on average and
  <peak> at its peak, of the 20 of the host`, and `brd-host runs studio` shows both for each run. A pattern
  matches what a command runs, not how it is written: `playwright test*` every run of it (`npx`, `pnpm exec`,
  `npx -y`, `npx -p @playwright/test`, `pnpm playwright`, a runner or the binary by its path), `pnpm e2e*` also
  `pnpm -C web e2e`, `--dir`, `--filter`, `--registry <url>` and pnpm by its path, while a flag of pnpm takes no value
  (`pnpm --silent test:unit e2e` stays light): the value options and the flags are those of pnpm 12.18.0 and pnpm 10
  (`PNPM_VALUE_OPTIONS`, `PNPM_FLAG_OPTIONS` of `crates/host-link/src/config.rs`), and an option the two read
  otherwise counts both ways (`PNPM_DIVERGING_OPTIONS`: `pnpm --no-proxy e2e` is heavy). A shell
  that wraps a command counts by what it runs: `zsh -c 'pnpm install; pnpm e2e > e2e.log 2>&1'`, `bash -lc`,
  `env FOO=1 zsh -c`, a shell in it, a substitution `"$(…)"`, and a script file of the shipped working tree
  (`zsh probe/run.sh`, `./probe/run.sh` whose first line names a shell or that has no first line `#!`, `. ./lib.sh`)
  are heavy when any command in them
  is (`brd-host: a heavy run by `pnpm e2e*` of [host] in its script`); a shell's script file that the client cannot
  read from the working tree (outside of it, absent, over 256 KiB) counts as heavy. A polyglot wrapper that runs
  itself again under bash (`exec /bin/bash "$0" "$@"`) adds nothing: `$0` of a script file is that file and `$0` of a
  `-c` script the word after it, while a `$0` that names no file (the name of the shell, `$0` of the command itself)
  is a script the client cannot read. Before 2026-11-15 a wrapper went
  beside the run that held the turn. The wrapper itself counts too: a pattern that names it (`./probe/run.sh*`) matches.
  Each script is read once a run, 64 at most, past which, as past four nested shells or scripts, a script counts as
  heavy. `eval`, `xargs`, `sudo`, `$'…'`, `caffeinate` and a script by its path after a `cd` (`cd scripts &&
  ./e2e.sh`, read from the root) are not read: a heavy run through them says so with `--heavy`. Its wait
  says `brd-host: run … waits … for the heavy turn of the host, 7.5 of its 20 cores, … left: run … holds 12.0 for … (…,
  measured), …; a wait, not a hang: …` at once
  and once a minute. `brd-host runs studio` names the holder and the waiting runs. A wait past 2 h exits 75
  (`heavy_timeout`): never run it here then, as this Mac's load is what fails its tests; run it on the Studio again
  later. A run that is not heavy goes at once. Start a run that can take minutes as `brd-host run studio --max 9m -- <command>`
  and, while it exits 124 (`still_running`), `brd-host wait studio <run>`: each call says only the changes of the state
  of the run and ends before the 10 min of a tool call, the run goes on past it, and the end gives a summary with the
  path of the log of the whole output. SIGTERM and SIGHUP of `brd-host run` cancel the run there, as Ctrl+C does, so
  never a `timeout` wrapper, a run in the background with a marker file, or a `brd-host follow` loop.
- **One heavy run is one check**: `cargo nextest run --workspace`, `pnpm lint:backend`, `pnpm e2e` and each side of an
  A/B comparison are runs of their own, each with `--heavy` when `[host]` does not name it. A bundle (`zsh -c 'pnpm
  test:unit && for i in 1 2 3 4; do pnpm e2e …; done && pnpm e2e:webkit && …'`) held the turn for 1 h 19 min on
  2026-11-18 while the fast lanes of idea-1 and the checks of other sessions waited up to 75 min; between separate
  runs the turn goes round.
- **A later run supersedes a waiting one**: a heavy run of the same checkout and the same command (`nice`, `time` and
  the like before it aside, its environment not) takes the place of the one of that checkout that still waits for the
  turn, which ends with `superseded` (exit 75) before its lend: nothing of it ran, so never run it again; its message
  names `brd-host follow studio <later run>`. The later run says `brd-host: replaces run …, which waited …`, and
  `brd-host: run … of this checkout also waits for the heavy turn, …: <command>` when the checkout queued another
  command: cancel the one you no longer need. A run that holds the turn is never superseded. On 2026-11-18 one session
  queued the same hour-long A/B batch on each new commit, and each would have run.
- **The disk of the Studio**: the Studio has no app and no `brd`; never put either there. Its agent frees space by
  itself below `branchdeck.leaseMinFree` there; `brd-host disk studio` shows the pools, `brd-host trim studio --plan`
  what a trim would delete, and `brd-host trim studio` deletes it, after the user agrees.
- **Exit codes**: the command's own; 75 when the Studio did not run it (unreachable, a refusal before the command):
  run the same command here then, but never after `heavy_timeout` or `superseded`; 130 after Ctrl+C, which cancels the
  run there. `studio runs another build of brd-host … only after the user agrees` is a hint: the run goes on.
  `brd-host schema` lists every refusal code with its exit code.
- **Output**: stdout is only the stdout of the command, byte for byte, when the agent of the Studio is not older than
  the local brd-host. stderr has the lines of brd-host, and the run ends with `brd-host: phases: … command …, total …`:
  read that line before you guess where the time went (`heavy` is the wait for the other heavy runs).

## Measured

On 2026-10-24, with yonder, Studio load 4 to 22: `cargo nextest run --workspace` 4 min 7 s in a new slot (243.7 s of
tests), `pnpm e2e` 2 min 34 s, an `--apply` of `cargo fmt` 2.1 s, a rerun of the same tree sends no pack. On
2026-10-28, with brd-host: `cargo nextest run --workspace --no-fail-fast` 4 min 48 s in a warm slot (239.6 s of tests,
3605 tests), and 0.6 s of it before the command.
