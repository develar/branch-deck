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
- **Wait for the answer, not for a count before it**: a fake counts a call before the code under test has its answer, so a test that goes on at the count races the code (`counted` of `crates/repository-model/src/session_test.rs` saw the read of the inputs, and the next open joined the check that had not answered). The test waits for the outcome of its request, and a fake that answers late after its count (`late_answer`) makes the race happen in every run.
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
process its own `GIT_TRACE2_EVENT` directory (macOS and Linux). It also splits the time of each test (JUnit) into its
time in git and beside git: a git process lasts from its `start` event to its first `atexit` by git's own clock
(`t_abs`), the time in git of a test is the union of those spans (several at once count once), and what is left beside
it is the test's own: its code, the start of the test process, the start of each git before git's clock runs (exec and
the loader) and its reap, and the test's waits. `summarize` prints both by subcommand and by binary, the tests with the
most processes and those with the most time beside git; `compare` the same of two runs. `pnpm test:perf` tests the script on trace lines of its own. On the Studio, a
`--fetch` of the output directory brings back `summary.json`, `junit.xml` and `run.log` (the trace files, named by the
test, do not come: `rm -rf <out>/traces` after the measure). The times of its run mean little: a time is measured on
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
| **The scenario table** | `crates/bd-remote/src/scenario_test.rs`: 9 tests over the 120 cases of `crates/domain-policy/cases/`, each case in a thread of its own (`assert_group`, :1569) with its own `Space` and the commands of `bd` beside the session of the app; `the_next_step_row_cases_answer_alike_in_the_app_and_bd` (:1613): 35 cases, 3,270 processes, 28.8 s | 10,347 processes (11.0 %), 181 s. bd-remote starts first (a priority binary), and its 120 threads beside the 20 slots made a git process cost 70–159 ms in the 20–60 s of the run against 41–57 ms in the rest: the 22 tests of `status_test.rs` (19–38 processes each; `brief_status`, :407, 11.2 s) ran then | s16-6: the repository of a case is one history (`space_of`: its commits in the stream of its `Space`, its settings appended), the fixtures' processes of the six tests 1,636 → 631, `the_next_step_row…` 3,271 → 2,975. The rest is what the table compares: the sync and the reads of the app's session and of `bd` over one repository, and the commands that put the case's branch on its remote or its robots on the built tips. s17-6 measured the width ([Beyond the processes](#beyond-the-processes), the five priority binaries on the Studio): the table costs its neighbours 265 s of junit and the five 21 s of wall (s17-2, load 17–29), after s17-3…s17-5 233 s and 20 s (1,912 s of the others with it against 1,679 s without, 106 / 86 s wall, sys 1,457 / 1,195 s, load 6–34); its nine tests take 164 s in that run against 100 s alone, `the_next_step_row…` 37.5 against 17.9 s, far under the slow-timeout of 60 s. 4 cases at once give the neighbours 70 s back and the table takes them (the sum −1.4 %, the wall the same), and `worker_threads(2)` of `RepoContext::runtime()` changes nothing, with the bound or alone: the threads of a group stay, and so does the runtime. The priorities of `.config/nextest.toml` stay too: a full run keeps its 20 slots full to its last second, and its last test takes 4.4 s |
| **The reads of a `bd` command before its operation** | each command resolves the repository (`rev-parse --show-toplevel`), reads `config -z --show-scope --get-regexp`, `symbolic-ref HEAD`, `remote -v`, the upstreams (`for-each-ref`), the commit list, `remote get-url --push origin` and `.space/repository-settings.json`, and a command of the remote syncs first: `another_remote_is_compared_with_as_origin_is` (`crates/bd-remote/src/push_test.rs:565`) runs six commands in 150 processes, five or six of each read. Some read one fact more than once: `symbolic-ref HEAD` 17 times of 194 processes in `a_rename_and_a_delete_resolve_with_a_take` (`crates/bd-update/src/resolve_test.rs:1178`), 19 of 171 in `a_lease_pick_lands_the_commits_on_the_home_branch_and_the_lease_follows` (`crates/bd-lease/src/lease_pick_test.rs:50`) | the tests of bd-remote, bd-update, bd-rewrite, bd-lease and bd-cli beside the scenario table: 555 tests, 35,021 processes (37.3 %), 1,927 s of the 4,633 | s16-6, traced with the git of `bd` told apart (`GIT_TRACE2_ENV_VARS=GIT_CONFIG_GLOBAL`: only the environment of `bd` sets it): of the 48,962 processes of the six binaries 35,374 were `bd`'s, 2,074 commands in 564 tests, 17 a command; 13,588 the fixtures' and the tests' own checks. The fixtures' part went to 9,332, `bd`'s stayed (35,356): the repositories as one history (the previous rows) and one `git version` (the next row). What a command reads more than once is the code under test and stays the measured floor: 11,180 of its processes repeat a read of the same command, mostly the guard before the lock, the witness under it and the check before each ref move of a pick or an update (`HeadRef::ensure`, `files_follow`: `symbolic-ref HEAD` 1,066 of 2,129), which [Lock spans](../design/git-operations.md#lock-spans) require, and a second sync where the scenario table runs the app's session beside `bd` on one context. No test spends a wait of its environment: a lock waits its 200 ms only where a test holds it, a retry of GitHub or Patronus waits none, the fakes start no process. s17-3 (the six binaries traced as above on this tree, before and after, 650 and 651 tests; counts only, on the MacBook beside other agents' runs): a command reads HEAD's branch, the remotes with their push URLs and the baseline once (`RepoContext::command_facts`, `baseline`, forgotten after a wait for the lock) and passes them to its sync (`SyncOptions::facts`), its grouping and the fetch of the target; the push URL of `origin` comes from the same `remote -v`. The git processes of the six binaries 45,369 → 44,028, `bd`'s 35,882 → 34,545 (16.9 → 16.3 a command); the repeated reads 11,570 → 10,632: `remote get-url --push origin` 208 → 1, `remote -v` 639 → 326, `symbolic-ref HEAD` 1,113 → 816. Of the repeats 5,998 (5,327 after) are the scenario table's, where the cases of one test run in threads of one process and the segments of their commands interleave in its trace: no command repeats them; the rest, 5,572 → 5,305, is the guard, the witness and the check before each ref move of a pick or an update (s17-4), a transaction per step and the `fast-import` per branch. s17-4 (traced the same way, 651 tests, the MacBook at load 22–28 beside other agents' runs, counts only): a move of HEAD reads it only under git's locks (`symbolic-ref` inside `RefTransaction::commit_while`, once more only when `prepare` fails) and refreshes the index only before it (`read-tree -u` records the stat data of what it writes), and `bd fold` hands the range of its plan to its core (`FoldRange`). The git processes of the six binaries 44,028 → 43,607, `bd`'s 34,545 → 34,196 (16.3 → 16.1 a command), the repeated reads 10,632 → 10,350: `symbolic-ref HEAD` 1,842 → 1,709 in all, `update-index --refresh` 416 → 294. What is left is the floor by design, not a second listing: the `rev-list` walks of the remote status are one per branch and range (`sync_core::remote_status`), the marks of interrupted moves one `rev-parse` per worktree, the guard of the check after the fetch of an update reads HEAD again across the network, the witness under the lock and the `symbolic-ref` under git's locks are what [Lock spans](../design/git-operations.md#lock-spans) require, then a transaction per step, the `fast-import` per branch and a `cat-file --batch` per phase; `log -1 --format=%T %ct` repeats only in the scenario table, whose cases interleave in one trace |
| **Waits for time** | `a_pick_waits_for_the_index_lock_of_another_process_and_then_lands_with_its_files` (`crates/git-ops/src/pick_test.rs:604`: the lock goes after 500 ms); the fakes of `crates/host-client/src/run_test.rs` (`heartbeats_keep_a_waiting_request_alive`, :992: four heartbeats a second apart, 4 processes, 4.5 s; `a_silent_agent_times_out_after_the_reconnect_window`, :595: 5.0 s); `crates/host-agent/tests/serve.rs:530` (0.5 s), `serve/lend.rs:102` and `:142` (0.6 and 1 s); `a_stopped_git_ends_every_process_it_started` (`crates/git-executor/src/git_command_executor_test.rs:378-382`, 0.7 s, Windows only). The 10 ms polls of `crates/bd-lease/src/clones_lock_test.rs` wait for a state and stay | with the next class, the 48 tests of 20 processes or fewer that took 2 s or more: 179 s (3.9 %) | s16-5: the wait for a mark of the fake or for the state (`held` / `go`); a window of the code under test a value of the test's environment. Done: the lock of `a_pick_waits…` is held open, so the wait never takes it for one that no process owns, however late its holder lets go (the old lock nobody had open went after 500 ms; a thread late by 2 s under load let the wait remove it, and the test's own removal failed), and it goes 100 ms after the wait starts; `a_stopped_git_ends_every_process_it_started` waits for the exit of the alias's shell, whose Windows pid and start it reads at its first line (`/proc/$$/winpid`, `process_scan::is_running`), with a limit of 60 s that no run reaches, where it slept 0.7 s and counted beats (a stop that ends the launcher alone fails it, checked in the Windows VM) |
| **CPU of the unoptimized build** | `test_large_stdin_and_stdout_do_not_deadlock` (`crates/git-ops/src/git_command_test.rs:60`): 5 processes, 12.5 s; `a_diff_costs_one_list_and_one_cat_file_then_nothing` (`crates/file-diffs/src/service_test.rs:160`): 13, 12.3 s; `fetch_reports_progress_until_the_last_stage_ends` (`crates/update-core/src/update_project_test.rs:716`, 150 files of 64 KiB of noise): 32, 12.5 s; `the_patches_of_a_large_table_cost_less_than_its_events` (`crates/repository-model/src/state_test.rs:252`): 9.1 s | in the 179 s above | s16-5, to confirm first: the time of each alone on a quiet Studio; then `opt-level` for the dependencies of the test profile, against the build time it adds |
| **Races under load** | the traced run at load 27–122 (three suites on the Studio at once) failed four: `a_lock_that_a_process_has_open_stays_and_the_answer_names_the_process` (`crates/git-ops/src/index_lock_test.rs:106`: no holder named within the `retry` of 1 s), the two other lock tests of that file (24 and 46 s), `the_chunks_of_a_pack_keep_its_run_waiting_and_a_start_without_them_ends_at_the_stall` (`crates/host-agent/tests/serve/pack.rs:82`); in s16-3 at load 42.7, `a_window_that_watches_a_kept_session_anew_syncs_it_when_the_inputs_changed` (`crates/repository-model/src/session_test.rs:1526`: 2000 polls of 5 ms) | a failed run (in CI a pass on a retry fails it too) | Done (the review of s16): no limit of the clock decides them ([Time](#time)). The lock tests failed on a limit of the code: the `lsof` that names who has the lock open took more than its 10 s at that load (160 ms alone), so no holder was named and no ownerless lock removed; the holders are the file descriptors of each process, read through libproc in the process (`processes_with_open_file` of `crates/sync-utils/src/process_scan.rs`, 35 ms for 1000 processes), with no limit. The waits of the tests end by their state, at a limit no run reaches (`UNHURRIED`), and the holder of `a_step_blocked_…` lets go after the log line of the wait: before it, the refused step finds no lock and stands refused. The pack test sends its chunks until the run says that it waits, which a run says only after twice the stall time (`BD_HOST_TEST_PACK_SAY_MS`), so its line is the agent's word that the chunks kept the wait alive, however late the wait started (s17-7: chunks for the stall time after a line said at the start still lost to a late start of the wait in a full run); the end at the stall is told by its message, not by 20 s. `a_have_sends_heartbeats_while_it_waits_for_an_upgrade_of_git` ends the upgrade of git at the third heartbeat of its `Have`, where a thread of the test ended it after 600 ms and the test counted at least three heartbeats of 100 ms: under load the reply came before the third one. `counted` of the model saw the count of a check before the session had its answer, so the next open joined that check with the inputs before the commit: the test waits for the answer of an open, its host answers 50 ms after the count (`late_answer`), so a wait for the count fails every run, and the polls end at 1 min, not after 2000 of 5 ms. Beside 40 `yes` on the Studio (load 7 → 38), these tests, the lock tests of `pick_test` and the holders of `process_scan_test`, `--stress-count 20`: 220 of 220, the slowest lock test 2.6 s (it failed at 46 s before) |
| **The discovery of git in each test process** | every test process that runs git asks `git version` once (`DISCOVERED` of the executor) | 2,682 processes (2.9 %); a `bd` test asked two (the executor's `DISCOVERED`, then `discover_for_cli` of `bd`) | s16-6: each git answers its version once per process (`checked`, `a_git_answers_its_version_once_per_process_and_a_failure_is_asked_again` in `crates/git-executor/src/git_info.rs`) and `bd` of a test runs the fixtures' git: `version` of the six binaries 1,242 → 753 |

## Beyond the processes

The tree of s16 starts as many git processes as the tree of `s0a` and still takes longer (B16 of [Rust as the Domain
Authority](../design/rust-domain-authority.md)), so s17-2 split the time of every test into its time in git and beside
it ([Measuring](#measuring)) and measured each suspect on the Studio with throwaway builds (empty tests, switches read
from the environment; never committed). The raw outputs are in `notes.local/s17-test-suite/s17-2/` of the lease.

**Both trees traced** on 2026-10-30, one after the other with no other run on the Studio (`--profile ci --retries 0`;
the load at the start 24.8 for this tree, after the E2E of another checkout, and 17.3 for `s0a`, after this tree):

| | `s0a` (`66938306`) | s17-2 (`a81a9835`) |
|---|---:|---:|
| tests; git processes | 2,241; 88,459 | 3,800; 88,918 |
| wall, user, sys | 239.2 s, 474 s, 2,481 s | 278.1 s, 584 s, 3,048 s |
| the tests' time | 4,233 s | 5,452 s |
| in git (while one of its processes ran) | 2,214 s, 25.0 ms a process | 2,654 s, 29.8 ms a process |
| beside git | 2,019 s, 22.8 ms a process | 2,798 s, 31.5 ms a process |
| beside git: the tests without git | 1,035 tests, 142 s (137 ms a test) | 1,966 tests, 236 s (120 ms a test) |
| beside git: the tests of 1 to 19 processes | 146 tests, 69 s (34 ms a process) | 587 tests, 322 s (51 ms a process) |
| beside git: the tests of 20 processes or more | 1,060 tests, 1,809 s (20.9 ms a process) | 1,247 tests, 2,241 s (27.1 ms a process) |

Both halves grew: git itself runs slower in the run of this tree (by git's own clock `rev-parse` 6.9 → 10.9 ms,
`symbolic-ref` 7.0 → 13.9, `for-each-ref` 14.0 → 25.0, `log` 14.4 → 27.7, `version` 1.3 → 2.7), and every test spends
more beside its processes. Neither is a burst at the start: a git process costs 48–67 ms of test time in every 30 s of
this tree's run, 40–53 ms in the run of `s0a`. The binaries of `bd`, in git and beside it per process:

| Binary | Tests | Processes | Test s | In git, ms a process | Beside git, ms a process |
|---|---:|---:|---:|---:|---:|
| bd-remote | 99 → 141 | 10,150 → 19,037 | 462 → 1,089 | 21.8 → 33.7 | 23.7 → 23.5 |
| bd-update | 72 → 100 | 9,387 → 8,537 | 369 → 504 | 15.9 → 28.5 | 23.3 → 30.6 |
| bd-rewrite | 98 → 116 | 7,023 → 4,935 | 285 → 364 | 17.6 → 39.4 | 22.9 → 34.3 |
| bd-lease | 37 → 106 | 2,170 → 4,876 | 107 → 282 | 16.8 → 21.2 | 32.6 → 36.7 |
| bd-cli | 56 → 113 | 2,954 → 4,182 | 116 → 251 | 16.1 → 30.0 | 23.3 → 29.9 |
| operations (new) | 146 | 6,535 | 335 | 26.0 | 25.2 |
| sync-core | 813 → 513 | 29,403 → 18,596 | 1,452 → 1,005 | 24.5 → 27.1 | 24.9 → 27.0 |
| git-ops | 270 → 322 | 7,467 → 6,032 | 320 → 334 | 19.1 → 26.4 | 23.8 → 28.9 |

Inside a test its git processes run one after another (the sum of their spans over their union is 1.0–1.2 in every
binary but bd-remote 2.7, repository-model 2.9 and test-server 2.4; in `s0a` test-server ran 13,688 processes 26 at
once), so a git that runs longer by its own clock is not a test that runs several at once: it is the machine. Each
suspect, measured on its own (the Studio, `--profile ci --retries 0`, the load at the start named; nothing else ran
through `bd-host` unless said):

| Hypothesis | Measurement | Verdict | Step |
|---|---|---|---|
| The start of a git process costs more from a large test binary, from one with many threads or much memory | a test spawns `git --version` 200 times: bd-remote (190.8 MB) 5.5 ms, sync-utils (7.9 MB) 6.0 ms; beside 120 parked threads 5.3 / 6.2 ms; with 256 MiB touched 5.6 / 6.0 ms; the same test inside a full run 12.5–31.4 ms (load 22–41) | no: the binary, its threads and its memory do not change a spawn, the load of the run triples it. The kernel time per process is the contention of 20 full slots, which grows with all that the tests do | none of its own: every class below lowers it |
| A test process costs more without git | 50 empty tests: bd-remote 22 ms a test, sync-utils 16 ms (junit); beside a run of sync-core 60 and 40 ms | the floor of a process per test; 1,559 tests more than `s0a` are 25–95 s of the tests' time | none: nextest runs a process per test |
| The scenario table's threads slow the binaries that start with it | the five priority binaries (load 17–29): as they are 2,177 / 2,155 s of junit (the others 2,005 / 1,997 s), 112 / 111 s wall, sys 1,296 / 1,311 s; without the table the others 1,736 s, 90 s wall, sys 1,114 s; with 4 cases at once 2,135 s (others 1,930), 110 s; with 4 and `worker_threads(2)` 2,153 s (1,945), 111 s; with `worker_threads(2)` alone 2,146 s (1,984), 110 s | the table costs its neighbours 265 s and the five 21 s of wall; a bound gives the neighbours 70 s back and the table takes them: the sum −1.4 %, the wall the same; the workers of the runtime change nothing | s17-6 records the numbers in [the class](#classes) and changes nothing |
| LMDB flushes the drive at each commit (`F_FULLFSYNC`), and so does each durable write (`sync_all`) | a full run with both and without (throwaway `NO_SYNC \| NO_META_SYNC` and no `sync_all`), in turns, load 22–41: 257.3 / 256.8 s against 240.2 / 243.5 s; 2,915 / 2,920 commits of the cache took 66 / 67 s of the tests' time (23 ms each, p90 50 ms) against 1.2 / 1.5 s; 2,836 durable writes 28.8 / 28.2 s against 0.3 / 0.4 s, of them 1,904 of `operations.json` (21.4 s), 310 `repository-state.json`, 226 `studio.json`, 205 `settings.json`. object-cache and the sync tests of sync-core: 193 / 179 s of junit against 135 / 133 s, 10.9 / 10.3 s wall against 8.0 / 7.9 s | yes: 95 s of the tests' time and 13–17 s of a run, at the device, where the 20 slots' flushes queue. For the user a command writes at most 7 transactions (~30 ms, [Object cache](../design/git-operations.md#object-cache)) | the product keeps both: the sync of the cache is what keeps a power loss from leaving a wrong tree id (`an_environment_syncs_every_commit`), the durable write of the registry its state across a crash ([Operations registry](../design/git-operations.md#operations-registry)). s17-5 took both out of the tests only, by a value the product never has: `CacheConfig::sync_commits` off (`the_environments_of_a_test_commit_without_a_flush`) and `ProcessRegistry::unflushed` (`the_registry_flushes_each_write_but_in_a_test`), set by `bd` of the fixtures (`Environment::flush_to_drive`), `empty_cache`, the tests of the crates and the E2E server. The other 932 durable writes, 7 s (`settings.json` and `repository-state.json` of `SettingsService`, `studio.json` of a host), stay flushed |
| `ProcessScan::scan()` of `bd lease` walks every process of a loaded machine | 23–24 ms a scan alone and in a full run; 339 scans in bd-lease (8.1 s of its 253 s), 468 in a full run (10.9 s) | no: the scan does not grow with the load. bd-lease takes 2.4 s a test against 2.3 s in `s0a`: its time is its 69 new tests | none |
| A priority binary costs the kernel more per test than in `s0a`, or git runs slower in it | each binary alone under `/usr/bin/time -l`, `s0a` (load 30–43) against this tree (load 12–22 beside the `pnpm lint:backend` of another checkout; again at 32–40 beside a full run of another, the same numbers): bd-remote 21.7 → 46.7 s wall, sys 230 → 395 s, involuntary context switches 0.65 M → 5.6 M, page reclaims 7.8 M → 15.2 M; bd-update 22.5 → 25.0 s, sys 224 → 250 s; bd-rewrite 13.2 → 13.6 s, 113 → 110 s; bd-lease 6.5 → 13.3 s, 57 → 127 s. Traced alone, git per process: bd-update 14.8 → 21.4 ms, bd-rewrite 17.0 → 27.2 ms; the same subcommand `rev-parse` 4.8 → 7.3, `cat-file` 9.0 → 11.0, `update-ref` 32 → 33, `commit` 46 → 52 ms, while `config` 1,074 → 281 and `version` 570 → 0 processes left and `fast-import` of bd-rewrite 317 → 506 processes, 52 → 93 ms (longer streams) | no: the mean per process is the mix (the cheap processes went, the streams that replaced them stay), a subcommand alone costs about what it cost. bd-remote doubles with its processes (10,150 → 19,037) and the threads of the table (above); bd-lease with its tests. Debug logging costs nothing: the five with `RUST_LOG=off` 100.0 s against 100.4 s, sys 917 against 963 s | none of its own: s17-3 and s17-4 lower the processes |

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
