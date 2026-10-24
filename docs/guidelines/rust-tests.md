# Fast Rust Tests

**Status:** Active  
**Updated:** 2026-10-24  
**Tags:** [tests, nextest, fixtures, performance]

## Summary

The time of the Rust suite is the time of its git processes. A full run starts about 74,000 of them (2,620 tests, a
process each under nextest), and a test's time follows its count: a git process costs about 5 ms on a quiet Mac, 40 to
70 ms in a parallel run on the Studio (kernel time: one run spent 3,270 s in the kernel against 499 s in user code)
and about 80 ms on the Windows runner of CI. So a fixture spends no process it can do without, a test waits for a
state and never for time, and a change that adds processes to many tests is measured with
`pnpm perf:test-processes`, whose counts do not depend on the load of the machine.

## Fixtures

- **A repository is one process**: `TestRepo::new()` is a `git init` (no template: the sample hooks were 17 files per
  repository) and its config appended to `.git/config` at once (`append_config`: the test user, `zdiff3`, no signing,
  no `core.autocrlf`, no background maintenance). Never a `git config <key> <value>` per key in a fixture: append them
  with `append_config` right after the init or the clone, while no git uses the repository. A bare repository of a
  test is `git init --bare` and `disable_background_maintenance` (an append too).
- **A clone is one process**: `clone_from` passes its config as `git clone -c`.
- **A commit is two**: `create_commit*` and `commit_with` run `add` and `commit`, and the id comes from the summary line
  `git commit` prints with `core.abbrev=no`. Never a `rev-parse HEAD` after a commit of your own; `commit_with(args,
  env)` commits what a test staged itself.
- **Git is discovered once per process** (`DISCOVERED` of the executor, `discover_for_cli` without `BD_GIT`): a new
  `GitCommandExecutor::new()` costs nothing.
- **The guard**: `a_fixture_starts_one_git_process_per_step` (`crates/test-utils/src/git_test_utils_test.rs`) pins the
  processes of a repository, two commits and a clone. `test_utils::process_budget::git_processes_of(|| …)` lists the
  git processes of any closure by subcommand, a fixture's and the code's alike (trace2 of the whole test process); the
  budget tests of an operation count its own executor (`counting()`, `assert_processes`).

## Time

- **No wall-clock limit decides an assertion.** A limit of the code under test (the 1 s of a line comparison,
  `DIFF_LIMITS`) runs out on a loaded machine and turns the answer into another one; the test gives the code a limit
  that no run reaches (`unhurried()` of `crates/file-diffs/src/service_test.rs`) and tests the limit itself with a zero
  budget.
- **No sleep to wait**: wait for the state (a file a fake writes, a channel, `held`/`go` marks of a wrapped git as in
  `update_held_in_its_replay`). A wait of the code under test is a value of the test's environment
  (`Environment::lock_timeout` of the bd tests is 200 ms, the app's 10 s).
- **The order of a run**: `.config/nextest.toml` starts the binaries whose tests take longest first, so the end of a
  run is short tests.

## Measuring

```bash
pnpm perf:test-processes measure --out notes.local/tp-before                 # the whole suite, a trace2 directory per test
pnpm perf:test-processes measure --out notes.local/tp-after -- -p sync-core  # nextest arguments after --
pnpm perf:test-processes summarize notes.local/tp-after --top 40             # by subcommand, by binary, the costliest tests
pnpm perf:test-processes compare notes.local/tp-before notes.local/tp-after
```

`measure` runs the suite with `scripts/perf/test-git-runner.sh` as the cargo target runner, which gives each test
process its own `GIT_TRACE2_EVENT` directory (macOS and Linux). The times of its run mean little: a time is measured on
the Studio (`yonder run studio -- cargo nextest run --workspace`, [Studio Runs](studio-runs.md)) when nothing else runs there,
best as an A/B of two checkouts in turns. CI writes the 30 slowest tests of each platform and the time per binary into
the job summary (`scripts/slowest-tests.mjs`).

## Numbers

| Run | Git processes | Time | Where |
|---|---:|---|---|
| master `2e503b03` | 93,905 | 253 s (sum of the tests 4,908 s; user 499 s, sys 3,270 s) | the Studio, 20 slots, before monorepo lease checkouts loaded it |
| the fixtures of `50a26f0f` | 73,704 (−21.5%) | | traced on the developer Mac |
| A/B `09c39559` against `50a26f0f`…`6606cac4` | | CPU 1,048 / 1,024 s → 960 / 933 s (−9%); wall 302 / 244 s → 265 / 430 s | the developer Mac at load 60–190, two rounds in turns |

The wall times of the A/B say nothing: the load of the other sessions changed by a factor of three between the runs.
The process counts do not depend on it.

Of the 73,704, about three quarters are the code under test itself: each bd or sync operation reads the same
repository facts again (`git remote` three times per operation, `symbolic-ref HEAD` twice, three variants of
`rev-parse --git-dir`), which a session that reads them once per operation would cut (the rework of
[Rust as the Domain Authority](../design/rust-domain-authority.md)).
