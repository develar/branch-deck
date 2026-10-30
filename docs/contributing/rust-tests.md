# Fast Rust Tests

**Status:** Active  
**Updated:** 2026-10-30  
**Tags:** [tests, nextest, fixtures, performance]

## Summary

The time of the Rust suite is the time of its git processes. A full run starts about 89,000 of them (3,799 tests, a
process each under nextest), and a test's time follows its count: a git process costs about 5 ms on a quiet Mac, 40 to
70 ms in a parallel run on the Studio (kernel time: one run spent 3,270 s in the kernel against 499 s in user code;
on 2026-10-29 the tests' time, 4,633 s, was 49 ms for each of their 93,839 processes) and about 80 ms on the Windows
runner of CI. The 20 slots of a run on the Studio stay full to its last seconds, so the suite takes the sum of its
tests' time divided by the slots: a class of tests counts by its sum ([Classes](#classes)), not by its slowest test.
So a fixture spends no process it can do without, a test waits for a state and never for time, and a change that adds
processes to many tests is measured with `pnpm perf:test-processes`, whose counts do not depend on the load of the
machine.

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
- **A history is two**: `TestRepo::with_history` (a new repository) and `commit_history` (on top of a branch) write
  any number of commits with one `git fast-import` and check HEAD out with one `read-tree` (`test_utils::history`):
  the commits `add` and `commit` would write, the same ids at the same times. `with_commits` and `commits_on` take an
  array and give the ids back as one (`let (repo, [base, fix]) = TestRepo::with_commits(&[…])`), `one_file_each`
  writes the array of commits of one file each; a `HistoryCommit` is `by` another author and `deleting` paths, as a
  `git rm` before it. `RepoTemplate::build` writes a template so: its
  commits, its stores and `origin/master` in one stream; another ref at the root, as `main` of `setup_test_repos`, is a
  `reset_to_root` of the stream, a ref at a later commit (a baseline moved between two commits) a `reset_at`. A test of
  100 commits went 203 → 5 processes. A commit between two steps of a test (a peer's commit during a resolve session,
  the commit after a sync) stays a commit.
- **The repository of a `bd` test is one history** (`crates/bd-cli/src/fixture_test.rs`): `repo_with_baseline`,
  `repo_on_baseline`, `repo_with_commits` and `repo_with_subjects` write the initial commit, `origin/master` at it and
  the commits a test makes before its first command in one stream, `origin` appended to the config: three processes
  for any number of commits. `repo_with_remote` and `Space::with_history` add one `git init --bare` (the URL of Space
  and `core.sshCommand` appended too), a case of the scenario table is one `Space::with_history` (`space_of`), and the
  Safe Push failures of `commit_dangling_reference` and its siblings are one stream on top of master
  (`on_a_new_baseline`). A `Space` of one commit went 11 → 4 processes.
- **A git answers its version once per process**, whoever asks (`checked` of `git_executor::git_info`: `DISCOVERED` of
  the executor, `discover_for_cli` of `bd`, with or without `BD_GIT`), and the environment of the `bd` tests runs the
  git the fixtures discovered (`Environment::git`, Homebrew's first on macOS): one `git version` per test process,
  where a `bd` test asked two, three when `PATH` names an older git first. A new `GitCommandExecutor::new()` costs
  nothing.
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
  `update_held_in_its_replay`, the exit of a process by its pid and start, `process_scan::is_running`). A wait of the code under test is a value of the test's environment
  (`Environment::lock_timeout` of the bd tests is 200 ms, the app's 10 s).
- **A FIFO releases a waiting reader only while its writer stays**: on macOS a reader that waits in its `open` of a FIFO
  misses a writer that opens and closes before the reader runs again, and waits for the next writer (about 1 in 200
  such releases on a loaded machine; the test hangs until nextest kills it). A test that holds a read on a FIFO, as a
  disk that hangs, keeps the writer open until the reader took a part of what it writes: `answer_hung_read` of
  `crates/operations/src/journal_test.rs` writes past what a pipe holds, and `during_fast_import` of a wrapped git
  keeps its writer until git read the stream.
- **Commit times are the fixture's**: commits that must fall into different seconds, or a rewrite that must have a
  later committer time, get their dates (`create_commit_with_timestamp`, `GIT_COMMITTER_DATE` of the rewrite's git)
  instead of a wait for the clock. Two commits of one author with one subject share a `CommitKey` only within one
  second, so a fixture gives them subjects of their own unless the pairing is what it tests
- **A remote failure is asked again at once**: the clients of GitHub and Patronus wait a second before they ask again
  after a transient failure, and bd's environment of the tests sets no wait (`Environment::remote_retry_delay`); a test
  that fakes a 5xx still sees the retry.
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
the Studio (`bd-host run studio -- cargo nextest run --workspace`, [Studio Runs](studio-runs.md)) when nothing else runs there,
best as an A/B of two checkouts in turns. CI writes the 30 slowest tests of each platform and the time per binary into
the job summary (`scripts/slowest-tests.mjs`).

## Numbers

| Run | Git processes | Time | Where |
|---|---:|---|---|
| master `2e503b03` | 93,905 | 253 s (sum of the tests 4,908 s; user 499 s, sys 3,270 s) | the Studio, 20 slots, before monorepo lease checkouts loaded it |
| the fixtures of `50a26f0f` | 73,704 (−21.5%) | | traced on the developer Mac |
| A/B `09c39559` against `50a26f0f`…`6606cac4` | | CPU 1,048 / 1,024 s → 960 / 933 s (−9%); wall 302 / 244 s → 265 / 430 s | the developer Mac at load 60–190, two rounds in turns |
| a fetch and an update read the baseline once (`c4748d00`) | 75,265 → 74,157 | | traced |
| an operation reads its directories once (`650e296d`) | 74,299 → 71,600 | `bd sync` on the monorepo: full 157 → 156, unchanged 14 → 13 | traced; `pnpm perf:monorepo` |
| master `15bcb360` against `2e503b03` | 93,914 → 70,293 (−25%) | 232 / 279 s → 208 / 220 s, user CPU 482 / 479 s → 390 / 394 s | the Studio, two rounds in turns, the new runs at the higher load (11–14 / 41–65 against 17–24 / 41–79) |
| s16-4: `6d4194fa`, 3,740 tests (the tree of `s0a`, `66938306`, there: 227.7 s, 2,241 tests, load 3.7) | 93,839 | 234.9 s, the tests' time 4,633 s (bd-remote 954, sync-core 897, bd-update 427, bd-rewrite 284, git-ops 284, operations 280, bd-lease 246, update-core 228); again 268.4 s, 5,303 s (user 590 s, sys 3,227 s); beside the clippy of another checkout 465.1 s, 9,235 s | the Studio, `--profile ci --retries 0`; load at the start 13 (the `pnpm lint:backend` of another checkout from the first minute), 8 (31 over five minutes, no other run), 32; traced at load 27–122 |
| s16-6: the six binaries of `bd` and update-core (`-p bd-remote -p bd-update -p update-core -p bd-rewrite -p bd-lease -p bd-cli`), 637 tests, on `f431ca76` and with the fixtures of s16-6 | 48,962 → 44,688 (−8.7 %): bd-remote 21,246 → 19,050, bd-update 8,730 → 8,537, bd-lease 5,428 → 4,889, bd-rewrite 5,395 → 4,612, bd-cli 4,551 → 3,991, update-core 3,612 → 3,609; the fixtures' own processes before the first command of a test 7,611 → 3,833, `version` 1,242 → 753 | | the Studio, traced (`GIT_TRACE2_ENV_VARS=GIT_CONFIG_GLOBAL` tells the git of `bd` from the fixtures'), load 13–32 |
| s16-8: A/B of `abc5b20a` (3,799 tests) against the tree of `s0a` (`66938306`, 2,241 tests) | 88,459 → 88,897 (23.4 a test against 39.5); from s16-4 93,839 → 88,897 (−5.3 %): `add` 6,953 → 4,227, `commit` 6,058 → 4,297, `remote` 5,378 → 4,383, `config` 5,737 → 5,153, `version` 2,682 → 2,181, `fast-import` 3,745 → 4,560 | 219.3 / 231.8 s → 255.8 / 257.4 / 255.5 s (+10–17 %); the tests' time 4,003 / 4,263 s → 5,107 / 5,138 / 5,101 s, user 461 s → 568 s, sys 2,318 / 2,504 s → 3,215–3,287 s; traced, 48 → 60 ms of the tests' time per git process | the Studio, `--profile ci --retries 0`, the trees in turns (A B B A A B), load at the start 21–31 with no other run; two runs of `s0a` beside the full e2e of another checkout (293.8, 288.4 s) left out; traced at load 6.5 / 3.7 with the script of `abc5b20a` |

The wall times of a loaded machine say little: the load of the other sessions changed by a factor of three between
the runs of the first A/B. The process counts do not depend on it. What is left is mostly the code under test: about
three quarters of the processes are the operations' own git, of which the sync reads the facts of its fetch again (the
session of s7 of the rework is to own them). The last A/B shows where the count stops telling the time: the suite of
s16 starts as many git processes as the tree of `s0a` for 70 % more tests and still takes 10–17 % longer, because its
tests spend 60 ms of their time per process against 48 ms: in the kernel and the unoptimized code beside git (the
engine of `operations`, the scenario table, the crates of `bd-host`). The open classes are the reads of a `bd` command,
the CPU of the unoptimized build and the threads of the scenario table ([Classes](#classes)).

## Classes

What the suite spends, by class (B16 of [Rust as the Domain Authority](../design/rust-domain-authority.md): a full
run on a machine takes no longer than the tree of `s0a` there). Measured on `6d4194fa` (s16-4): the junit of the
first Studio run of the last row of [Numbers](#numbers) (234.9 s), `pnpm perf:test-processes` of the whole suite and
the argv of ten tests traced alone; the raw outputs are in `notes.local/s16-rust-budgets/` of the lease. The slots of
each run were full until its last 5 s, so a class costs what its processes cost, wherever its tests run; a fix is
made for a whole class and measured by `compare` before and after, never by weakening what a test checks.

| Class | Tests | Cost | Fix |
|---|---|---|---|
| **Commits one by one**: `create_commit*` and `commit_with` write a commit with two processes, `add` and `commit` | `each_restack_a_stack_problem_offers_removes_it` (`crates/sync-core/src/stack_operations_test.rs:136`, 30 commits, 223 processes); `a_free_call_in_another_crate_without_a_use_is_no_use` (`crates/trial-check/src/dangling_references_test.rs:511`, 18); `one_walk_tells_what_is_behind_and_the_fast_forward` (`crates/sync-core/src/detection_remote_status_test.rs:1009`, 15). The candidates are confirmed: commits in `detection_remote_status_test` 135 (24 tests), `update_project_test` 196 (52), `resolve_test` of bd-update 181 (44), `pick_test` of git-ops 96 (27), `dangling_references_test` 92 (23), `stack_sync_test` 81 (16), `sync_test` 72 (24), `branch_processor_test` 52 (15) | 13,011 processes (13.9 %): 6,058 commits; 1,138 tests write three or more (5,275 commits), 8,274 processes fewer at most if each wrote one history | s16-5: `with_commits` or `commits_on` for the commits a test writes before its operation, a commit between two steps stays. Traced before → after (`pnpm perf:test-processes`, the processes of a file): `dangling_references_test` 925 → 794, `detection_remote_status_test` 1,105 → 948, `stack_sync_test` 941 → 828, `sync_test` 665 → 610, `branch_processor_test` 813 → 752, `pick_test` 1,252 → 1,232 (and no `rev-parse HEAD` after a commit of `commit_at`), `resolve_test` 4,761 → 4,737; `setup_test_repos` writes `main` in its stream, one process fewer in each test of the seven other `detection_*_test` files (1,798 → 1,759) |
| **A process per file or per key** | `UpdateTemplate::build` adds each file alone (`crates/test-utils/src/repo_template.rs:1340`): 517 `add` of 770 processes in `e2e_templates_have_the_expected_update_states` (`crates/update-core/src/update_project_test.rs:528`); `configure_author` (`detection_remote_status_test.rs:74`) sets `user.email` and `user.name` per repository, 14 of the 115 processes of `one_walk…`; `Space::new` (`crates/bd-cli/src/remote_fixture_test.rs:38`) sets `core.sshCommand` and the URL of `origin` twice after its init | `config` 5,737 processes (6.1 %), with the one read of each operation among them | s16-5: one `add` per commit and the config, the remote and the upstream of `UpdateTemplate` appended: `update_project_test` 2,867 → 2,316, `templates_test` of test-server 871 → 356; the authors of `detection_remote_status_test` and the settings a test writes before its operation appended (`branchdeck.stacks`, `branchdeck.resolveByWords`, `branchdeck.symbols` of a new repository, branch descriptions) |
| **Several repositories in one test** | `every_template_builds` (`crates/test-server/src/templates_test.rs:38`): 90 `init`, 861 processes, 18.5 s; `e2e_templates_have_the_expected_update_states`: 10, 770, 33.9 s; `the_rules_are_read_only_for_a_push_into_an_approved_pull_request` (`crates/bd-remote/src/safe_push_test.rs:779`): five `Space`s, 248, 36.9 s, the slowest test; `a_head_moved_between_two_reads_of_a_rewrite_computes_it_once_more_under_the_lock` (`crates/sync-core/src/repository_lock_test.rs:331`): six in a loop, 341, 18.0 s | 36 tests with four `init` or `clone` or more beside the scenario table: 5,835 processes (6.2 %), 319 s | s16-5: each repository from one stream (a history, a template); a case of a loop becomes a test of its own only where it shares nothing with the others: a split alone moves the time to another slot |
| **The scenario table** | `crates/bd-remote/src/scenario_test.rs`: 9 tests over the 120 cases of `crates/domain-policy/cases/`, each case in a thread of its own (`assert_group`, :1569) with its own `Space` and the commands of `bd` beside the session of the app; `the_next_step_row_cases_answer_alike_in_the_app_and_bd` (:1613): 35 cases, 3,270 processes, 28.8 s | 10,347 processes (11.0 %), 181 s. bd-remote starts first (a priority binary), and its 120 threads beside the 20 slots made a git process cost 70–159 ms in the 20–60 s of the run against 41–57 ms in the rest: the 22 tests of `status_test.rs` (19–38 processes each; `brief_status`, :407, 11.2 s) ran then | s16-6: the repository of a case is one history (`space_of`: its commits in the stream of its `Space`, its settings appended), the fixtures' processes of the six tests 1,636 → 631, `the_next_step_row…` 3,271 → 2,975. The rest is what the table compares: the sync and the reads of the app's session and of `bd` over one repository, and the commands that put the case's branch on its remote or its robots on the built tips. The threads of a group stay: the slots are full until the end of a run, so a bound moves the time of the cases into the test without lowering the sum, and the neighbours slowed at the start are load, not waits (alone, `-j 1` at load 10: `the_robot_of_a_comment_is_never_an_earlier_robot_of_the_pull_request` 1.0 s, `brief_status` 0.31 s, against 15.6 and 8.6 s in a full run) |
| **The reads of a `bd` command before its operation** | each command resolves the repository (`rev-parse --show-toplevel`), reads `config -z --show-scope --get-regexp`, `symbolic-ref HEAD`, `remote -v`, the upstreams (`for-each-ref`), the commit list, `remote get-url --push origin` and `.space/repository-settings.json`, and a command of the remote syncs first: `another_remote_is_compared_with_as_origin_is` (`crates/bd-remote/src/push_test.rs:565`) runs six commands in 150 processes, five or six of each read. Some read one fact more than once: `symbolic-ref HEAD` 17 times of 194 processes in `a_rename_and_a_delete_resolve_with_a_take` (`crates/bd-update/src/resolve_test.rs:1178`), 19 of 171 in `a_lease_pick_lands_the_commits_on_the_home_branch_and_the_lease_follows` (`crates/bd-lease/src/lease_pick_test.rs:50`) | the tests of bd-remote, bd-update, bd-rewrite, bd-lease and bd-cli beside the scenario table: 555 tests, 35,021 processes (37.3 %), 1,927 s of the 4,633 | s16-6, traced with the git of `bd` told apart (`GIT_TRACE2_ENV_VARS=GIT_CONFIG_GLOBAL`: only the environment of `bd` sets it): of the 48,962 processes of the six binaries 35,374 were `bd`'s, 2,074 commands in 564 tests, 17 a command; 13,588 the fixtures' and the tests' own checks. The fixtures' part went to 9,332, `bd`'s stayed (35,356): the repositories as one history (the previous rows) and one `git version` (the next row). What a command reads more than once is the code under test and stays the measured floor: 11,180 of its processes repeat a read of the same command, mostly the guard before the lock, the witness under it and the check before each ref move of a pick or an update (`HeadRef::ensure`, `files_follow`: `symbolic-ref HEAD` 1,066 of 2,129), which [Lock spans](../design/git-operations.md#lock-spans) require, and a second sync where the scenario table runs the app's session beside `bd` on one context. No test spends a wait of its environment: a lock waits its 200 ms only where a test holds it, a retry of GitHub or Patronus waits none, the fakes start no process |
| **Waits for time** | `a_pick_waits_for_the_index_lock_of_another_process_and_then_lands_with_its_files` (`crates/git-ops/src/pick_test.rs:604`: the lock goes after 500 ms); the fakes of `crates/host-client/src/run_test.rs` (`heartbeats_keep_a_waiting_request_alive`, :992: four heartbeats a second apart, 4 processes, 4.5 s; `a_silent_agent_times_out_after_the_reconnect_window`, :595: 5.0 s); `crates/host-agent/tests/serve.rs:530` (0.5 s), `serve/lend.rs:102` and `:142` (0.6 and 1 s); `a_stopped_git_ends_every_process_it_started` (`crates/git-executor/src/git_command_executor_test.rs:378-382`, 0.7 s, Windows only). The 10 ms polls of `crates/bd-lease/src/clones_lock_test.rs` wait for a state and stay | with the next class, the 48 tests of 20 processes or fewer that took 2 s or more: 179 s (3.9 %) | s16-5: the wait for a mark of the fake or for the state (`held` / `go`); a window of the code under test a value of the test's environment. Done: the lock of `a_pick_waits…` is held open, so the wait never takes it for one that no process owns, however late its holder lets go (the old lock nobody had open went after 500 ms; a thread late by 2 s under load let the wait remove it, and the test's own removal failed), and it goes 100 ms after the wait starts; `a_stopped_git_ends_every_process_it_started` waits for the exit of the alias's shell, whose Windows pid and start it reads at its first line (`/proc/$$/winpid`, `process_scan::is_running`), with a limit of 60 s that no run reaches, where it slept 0.7 s and counted beats (a stop that ends the launcher alone fails it, checked in the Windows VM) |
| **CPU of the unoptimized build** | `test_large_stdin_and_stdout_do_not_deadlock` (`crates/git-ops/src/git_command_test.rs:60`): 5 processes, 12.5 s; `a_diff_costs_one_list_and_one_cat_file_then_nothing` (`crates/file-diffs/src/service_test.rs:160`): 13, 12.3 s; `fetch_reports_progress_until_the_last_stage_ends` (`crates/update-core/src/update_project_test.rs:716`, 150 files of 64 KiB of noise): 32, 12.5 s; `the_patches_of_a_large_table_cost_less_than_its_events` (`crates/repository-model/src/state_test.rs:252`): 9.1 s | in the 179 s above | s16-5, to confirm first: the time of each alone on a quiet Studio; then `opt-level` for the dependencies of the test profile, against the build time it adds |
| **Races under load** | the traced run at load 27–122 (three suites on the Studio at once) failed four: `a_lock_that_a_process_has_open_stays_and_the_answer_names_the_process` (`crates/git-ops/src/index_lock_test.rs:106`: no holder named within the `retry` of 1 s), the two other lock tests of that file (24 and 46 s), `the_chunks_of_a_pack_keep_its_run_waiting_and_a_start_without_them_ends_at_the_stall` (`crates/host-agent/tests/serve/pack.rs:82`); in s16-3 at load 42.7, `a_window_that_watches_a_kept_session_anew_syncs_it_when_the_inputs_changed` (`crates/repository-model/src/session_test.rs:1526`: 2000 polls of 5 ms) | a failed run (in CI a pass on a retry fails it too) | s16-5: reproduced under `taskpolicy` or a parallel load, then no wall-clock limit decides an assertion ([Time](#time)) |
| **The discovery of git in each test process** | every test process that runs git asks `git version` once (`DISCOVERED` of the executor) | 2,682 processes (2.9 %); a `bd` test asked two (the executor's `DISCOVERED`, then `discover_for_cli` of `bd`) | s16-6: each git answers its version once per process (`checked`, `a_git_answers_its_version_once_per_process_and_a_failure_is_asked_again` in `crates/git-executor/src/git_info.rs`) and `bd` of a test runs the fixtures' git: `version` of the six binaries 1,242 → 753 |

## Temporary files on a RAM disk (macOS)

The repositories of the tests live in the temporary directory, and their files cost on the APFS of the disk: with
`TMPDIR` on a RAM disk the full suite took 142 s instead of 164 s on the developer Mac (load about 100), `-p sync-core
-p git-ops` 38 and 42 s instead of 45 and 56 s; a run never held more than 75 MB there.

```bash
device=$(diskutil image attach --noMount ram://1048576)   # 512 MB
diskutil erasevolume APFS bd-tests "$device"
TMPDIR=/Volumes/bd-tests/ cargo nextest run --workspace
diskutil eject "$device"
```

Neither CI nor `bd-host run studio` does it: on a RAM disk the temporary directory of a test is no symlink, and the tests of
the macOS symlink of `/var` (`git_canonical_path`) would pass without one.

Of the 73,704, about three quarters are the code under test itself: each bd or sync operation reads the same
repository facts again (`git remote` three times per operation, `symbolic-ref HEAD` twice, three variants of
`rev-parse --git-dir`), which a session that reads them once per operation would cut (the rework of
[Rust as the Domain Authority](../design/rust-domain-authority.md)).
