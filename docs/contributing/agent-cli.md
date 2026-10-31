# Agent CLI (`bd`)

**Status:** Active  
**Updated:** 2026-10-28  
**Tags:** [cli, agents, json]

## Summary

`bd` lets an agent read what Branch Deck sees (the commits ahead of the baseline, their groups, and what ties them
together), regroup the commits and sync them. It reads with `status`, `commits`, `schema` and `version`, suggests a
checked grouping with `suggest`, rewrites commit messages with `group`, `rename`, `restack`, `unstack` and `undo`,
rebuilds the virtual branches with `sync` and keeps their pull request text with `describe`, tries a plan without
writing refs with `check`, pushes with `push`, asks the code owners to review a pull request with `request-review`,
starts a Patronus Safe Push with `safe-push` and reads its robot, and why it failed, with `robot`, and how the Safe
Pushes of the repository went with `robot-stats`. `group --sync`
applies a plan, its pull request texts and a sync in one call. `update` brings the checked out branch onto its
baseline as the app's Update Project does, and resolves a conflict in a resolve session outside the working tree;
`pick` lands the commits of another worktree on the checked out branch, `unapply` takes a branch and its files out of
it and `reapply` puts it back, `lease` keeps the pool of worktrees that
agent sessions work in, and `workspace` tells an agent before its first edit whether to lease one and how to write a
commit subject; `fold` folds the fixup commits, `squash` squashes the commits the user asked to squash, and
`move-hunk` moves the change of a file from one commit into another. `install` puts `bd` and its skill where agents
find them; `help agent`, `help update` and `help worktrees` print the texts an agent follows, `help <command>` the
`--help` of a command.

`push`, `safe-push` and `request-review` write to the remote as the app's actions do: an intent of the engine of the
operations (`crates/operations`), its plan prepared over the facts it needs (`needs`, a one-shot session of the sync),
its effects executed after its witness, with the same plans, refusals and outcomes as the app. `push` plans an intent
`Push` per branch it names and runs their plans as one atomic push (`operations::push::push_together`); where the
app's push leaves a covered branch out to its row, `push` refuses (`push_refusals`). A plan whose facts changed before
its first effect fails as `plan_changed` (exit 3, nothing done). The run is the operation of the command: it is
recorded in the operations registry of the repository under the `operation_id` of the JSON (the id of the root span
below), `busy` while another process runs what it needs, and once it ended it goes into the journal of the operations
of the repository under that id, beside the app's (what it did, how it ended, its git processes, its HTTP requests and
its time from the admission; [Journal](../design/git-operations.md#journal)). A rewrite (`group`, `rename`,
`restack`, `unstack`, `reword`, `fold`, `squash`, `park`, `unpark`, `unapply`, `reapply`, `undo`, `move-hunk`),
`update` and `pick` that write are recorded too, before they take any lock (`bd_core::operation_record::recorded`): the app's
windows show them on their rows ("bd: Unstacking beta…") and Activity lists them, but HEAD, which they hold, makes no
other process `busy`: the repository lock orders the rewrites of the app and of every `bd`, and a rewrite of the app
meanwhile is waited for (`locked` after the bounded wait). A dry run, `bd update --status` (without `--materialize`)
and `bd check --fix`, which write nothing, are no record. The other commands that write are no operation of the
repository either: `sync` (a sync takes no lock and moves each ref by compare-and-swap, as the app's), `describe` (the
`branch.<name>.description` of git config, no ref), `lease` and `workspace` (the pool's own `leases.json`, under its
own lock), `operation forget` (it writes the registry itself) and `install` (the skill files, no repository).

On macOS and Linux it is the `cli` subcommand of the app binary: `branch-deck cli status`. If the binary is symlinked
as `bd`, the CLI runs without the subcommand. `src-tauri/src/main.rs` dispatches before Tauri starts, so no window or
plugin is created. On Windows the app binary is a GUI-subsystem program without a console, so the installer puts a
console `bd.exe` next to `branch-deck.exe`: the `bd` binary of `crates/bd-cli`, which `scripts/build-bd.mjs` builds
for the Tauri `externalBin` of `src-tauri/tauri.windows-bd.conf.json` (passed with `--config` by the Windows CI jobs
only, so other builds don't need the file). The macOS release build carries `bd-host` the same way
(`scripts/build-bd-host.mjs`, `src-tauri/tauri.macos-bd-host.conf.json`), and `bd install` links
`~/.local/bin/bd-host` to it. The code is in `crates/bd-cli` (the `Cli`, the dispatch, `bd schema`, the help texts) and the crates it
builds on: `bd-core`, `bd-lease`, `bd-update`, `bd-rewrite` and `bd-remote`, and the library `commit-clustering`; the requirements are in the
[Agent CLI spec](../../spec/cli/agent-cli.spec.md).

## Invocation

```bash
branch-deck cli [-C <repo>] [--prefix <p>] [--full-ids] [--no-cache] [-v] <command> [options]
cargo run -p branch-deck -- cli status -C .   # from a checkout
```

- `-C <repo>` sets the repository. It defaults to the current directory, and any subdirectory resolves to the
  top level of the working tree.
- `--prefix <p>` sets the branch prefix. Without it the specific prefix wins: `branchdeck.branchPrefix` of the
  repository's local or worktree config (`git config branchdeck.branchPrefix <p>` in the repository) or `git -c`, else the app
  setting `globalUserBranchPrefix`, read from `settings.json` in the app data directory
  (`~/Library/Application Support/org.develar.branch-deck/` on macOS, `%APPDATA%\org.develar.branch-deck\` on
  Windows), else the global or system `branchdeck.branchPrefix` (`resolve_branch_prefix`). An empty prefix is the error
  `no_branch_prefix`. Every output echoes the prefix, and `bd status` names where it came from (`prefix_source`:
  `cli`, `repository_config`, `app_setting`, `global_config`).
- The settings of the app are read once as bd starts, by the app's own loader (`app_settings::file::SettingsFile`),
  with its defaults: without the file, or when it does not parse, bd runs with the defaults (no prefix of the app,
  `patronusNewApi` off). The app writes the file only by a rename, so bd never reads a cut file.
- `--full-ids` prints full commit ids. By default an id is cut to 8 characters, as in Branch Deck's messages
  (`git_ops::revs::short_id`). When two ids of one output share that prefix, all ids in it get longer.
- `--no-cache` neither reads nor writes the object cache (see below).
- `-v` logs debug output to stderr and `-vv` logs trace output. Without either, stderr has only the progress lines of
  the commands that take minutes (the trials of `bd check --fix`, the steps of `bd lease get`). Each line starts
  with the root span of the run, `operation{op=status operation_id=01K…}` (a ULID, `sync_utils::operation_id`), also
  the lines of the threads a sync builds its branches on, so the lines of two runs that interleave stay apart.
- Git comes from `BD_GIT`, else `git` on `PATH`, else Homebrew's git on macOS. It must be 2.54 or newer. Unlike the
  app, the CLI never asks a login shell for git. Without one, the error is `git_not_found`: a line "Cannot find Git:
  <reason>" or "Git 2.39 is too old: Branch Deck requires Git 2.54 or newer", then how to install Git.

## The JSON contract

- stdout gets exactly one JSON object per run, on one line. Only `bd help` prints text.
- The JSON is ASCII: every other character is a `\uXXXX` escape, so a shell that decodes stdout with a legacy code
  page (Windows PowerShell 5.1) still reads every path and subject. A plan or a body that bd reads (`--plan`,
  `--body-file`; `-` is stdin) is UTF-8, with or without a byte order mark, or UTF-16 with one. A pipe of Windows
  PowerShell 5.1 turns other characters into `?` before bd reads them, so pass a file there.
- Every object starts with `"schema": 2`. The experimental `bd review-plan` has a version of its own, 3. The version
  goes up only when a field changes meaning or disappears. New optional fields don't change it.
- Every object ends with the `operation_id` of the run, an error object and `bd --version` too: a ULID, new for each
  run, the id of the root span of its log lines (`-v`), and of its record in the operations registry and its entry in
  the journal of the operations when the command runs an operation of the remote (`push`, `safe-push`,
  `request-review`), a rewrite, an update or a pick that writes. `bd operation forget` prints its own and the one
  it forgot (`forgotten`). `bd schema` declares it in every output (`schema.rs`, `with_operation_id`).
- Optional fields are left out when they are empty. `group` is the exception: it is always present and is `null` for
  an unassigned commit. In a `--brief` view a field is also left out when another field says it: `issue_key` when it
  is the `group` (`bd commits --brief`) or the `name` (`bd status --brief`) of an issue branch, and the `(prefix)` of a
  subject when it is exactly the `group`.
- `bd schema <command>` prints the JSON Schema of the outputs of a command, `bd schema` their index (see `bd schema`
  below). The schemas are generated from the Rust
  types with `schemars`, and `schema_test.rs` checks the real outputs against them.
- Read only: `status`, `commits`, `suggest`, `check`, `robot`, `robot-stats`, `schema`, `version`, `update --dry-run`, `update --status`,
  `lease status`, `workspace`, `pick --dry-run`, `fold --dry-run`, `squash --dry-run`, `move-hunk --dry-run` and `reword --dry-run` write no ref, config, index or working tree. They write at most git objects (the trial sync of `check` and
  `suggest`, the replay of `update --dry-run` and `pick --dry-run`, the merged trees of `move-hunk --dry-run`, the
  squashed commit of `squash --dry-run`) and the object cache (none with `--no-cache`). `bd --help` and
  `bd help agent` print the same line (`READ_ONLY` in `agent_help.rs`), and `agent_help_test.rs` checks the refs, the
  config, the index and the working tree around each of them.
- The working tree: only `bd update`, `bd pick`, `bd unapply`, `bd reapply` and their undo write the working tree, through the guarded update of
  the app's Update Project: only the files the move changes, and in a resolve session only at the finish. `bd lease`
  writes only the slots of its pool, which are not the user's working tree (see `bd lease`).

An error looks like this:

```json
{"schema":2,"error":{"code":"no_branch_prefix","message":"No branch prefix: ...","commits":[],"retryable":false}}
```

`uncommitted_overlap` also has `paths`, the local files that block an update, a pick or their undo:

```json
{"schema":2,"error":{"code":"uncommitted_overlap","message":"…","commits":[],"paths":["src/app.txt"],"retryable":false}}
```

A refused resolution of `bd update --continue` has `paths` too, the conflicting files; `lease_rejected` and
`push_refused` have `branches`, the branches with their remote status; `push_too_large` has `paths`, the directory that
holds most of the push (`dir/`) and its largest files.

`approvals_missing` of `bd safe-push` has `review`, the code-owner review the Safe Push lacks (see `bd safe-push`).
`target_conflict` has `commits` and `paths`: the commit that Patronus can't rebase onto the newest target and its files.

A refusal that a command acts on has `next_steps`, the commands ready to run in their order, each with
`needs_approval` (the `refusals` of `bd safe-push --check` carry them too):

```json
{"schema":2,"error":{"code":"lease_rejected","message":"The remote rejected auth: …","commits":[],"branches":[…],
 "next_steps":[{"command":"bd push auth --force --expect-remote 1a2b3c4d","needs_approval":true}],"retryable":false}}
```

A step with `needs_approval` pushes, rewrites, updates or cancels or restarts a robot: run it only once the user
approved it. One without it only reads (`bd robot`, `bd sync`, `bd safe-push <branch> --check`). The field is
`next_steps`, not `next`: the `next` of `bd workspace`, `bd update` and `bd pick` runs at once, a step of an error
does not. A way to start anyway (`--allow-*`, `--without-approvals`), `--min-free` and a change of the config are
never a step: the message names each with its condition ("only when the user says it is a false alarm", "never raise
`branchdeck.pushSizeLimit` yourself"), and so it names the condition of `--force`, `--cancel-running`,
`--restart-running` and `--confirm-preserved` ("once the user approved"). The steps, by code: `lease_rejected` the
leased push that replaces the remote branch (`--with-children --force` for a stack branch); `push_refused` the push
that replaces a branch behind its remote, `bd sync` for one not synced; `robot_running` `bd robot` of the robot, and
the same command with `--cancel-running` (`safe-push`) or `--restart-running` (`push`) where it applies;
`approvals_missing` the start with `--create-pull-request` when the branch has no pull request; `dangling_reference`
the `fix.command` a trial build proved; `missing_declaration` the `bd restack` it names; `push_dismisses_approvals`
the push, then `bd request-review`; `target_conflict` and `landed` `bd update`; `stack_conflict` the `--check` of the
branch whose commit conflicts; `stack_not_built` `bd sync`. `NextStep` is in `crates/bd-core/src/error.rs`.

| Exit | Meaning | Codes |
|---|---|---|
| 0 | ok | |
| 1 | conflict | never an error object: the command's output with `state: "conflict"` (update, pick), a branch not built (sync, `--sync`), or `ok: false` (check, suggest) |
| 2 | invalid input | `invalid_arguments`, `block_not_found`, `not_a_repository`, `no_branch_prefix`, `unknown_branch`, `invalid_plan`, `not_ahead_of_baseline`, `no_session`, `unresolved_markers`, `resolution_outside_conflict`, `missing_resolution`, `empty_resolution`, `no_lease`, `lease_mismatch`, `not_a_slot`, `slot_in_pool`, `not_parked`, `no_identity` (git cannot form the identity of the user: set `user.name` and `user.email`) |
| 3 | stale, read again and retry | `head_moved`, `groups_moved`, `stacks_moved`, `parked_moved`, `prefix_changed` (the app's sync only), `round_moved`, `plan_changed` (the facts a remote action was planned on changed before its first effect; nothing was done), `locked`, `busy` (an operation of another process, the app or another `bd`, runs what the command needs: `error.holder`; nothing was done) |
| 4 | refused | `no_baseline`, `operation_in_progress`, `detached_head`, `uncommitted_overlap`, `undo_stale`, `lease_rejected`, `hunk_conflict`, `stack_conflict`, `stack_not_built`, `target_conflict`, `range_unknown`, `source_on_newer_baseline`, `home_branch_switched`, `no_home_branch`, `unchecked_tree`, `lease_follows_home`, `resolve_in_progress`, `session_stale`, `unsupported_conflict`, `finish_pending`, `push_refused`, `push_too_large`, `no_remote`, `install_refused`, `approvals_missing`, `push_dismisses_approvals`, `rules_unknown`, `review_unknown`, `pull_request_other_base`, `robot_running`, `landed`, `parked`, `dangling_reference`, `missing_declaration`, `not_on_space`, `not_logged_in`, `patronus_refused`, `no_pull_request`, `no_reviewer`, `needs_push`, `review_refused`, `slot_busy`, `host_slot`, `unlanded_commits`, `dirty_worktree`, `branch_exists`, `other_volume`, `low_disk_space`, `lease_reclaimed`, `canceled` |
| 5 | git or network error | `git_not_found`, `git_error`, `network_error`, `patronus_error`, `internal` |

`no_baseline` is also a repository without commits (`git init`): every command that reads HEAD or the baseline says
"The repository has no commits yet: commit to the main branch, then sync".

The codes are `ErrorCode` of `crates/domain-types/src/error.rs` (its `exit_code`, `retry` and `retryable`), one registry
for `bd` and the app: a command of the app fails with `ApiError` of the same file, whose `code` is one of these. `CliError`,
the error object of `bd`, is in `crates/bd-core/src/error.rs`. A typed error of a core names its code itself
(`RegroupError::code`, `SyncDeferred::code`), so `bd` and the app read one mapping. The app's conversion is
`crates/app-api/src/api_error.rs`: the app and the e2e server turn a core's error into `ApiError` with its one function,
`app_api::api_error`, which also names the holder of a held lock (`details`) and gives the other typed errors
the code the `bd` command gives them: the types name it (`ParkError::code`, `GuardRefusal::code`,
`UnapplyRefusal::code`, `ReapplyRefusal::code`), which the commands of `bd` (`park_error`, `unapply_error`,
`reapply_error`, `update_command::core_error`) read too. An undo answers `ErrorCode::in_undo` of the code, in `bd undo`
and in the undo commands of the app (`app_api::api_error::undo_error`): a ref that moved on since the operation is
`undo_stale`, in the words of `HeadMoved::undo_message` or `StoreMoved::undo_message` (`RegroupError::undo_message`):
what moved and that it can't be undone any more, not the advice to retry. One table of the typed errors with their codes, `crates/update-core/src/error_cases.rs`, is what both
sides are tested on (`error_code_test.rs` of `bd-update`, `park_error_test.rs` of `bd-rewrite`, `api_error_test.rs` of
`app-api`): no crate calls both, as only the app and the e2e server depend on `app-api`. A lock file of git in the way of a ref move (`RefLocked`) is `locked` without
`details`: its message names the file. So is the index lock of another process that stayed for the 30 s a move waits
for it (`IndexLocked`; a lock no process owns is removed): its message names the file, its age and the processes that
may own it, which bd lists in `processes` too. Both adapters answer the JSON of
`crates/app-api/fixtures/api_errors.json` for each of its cases (`api_error_contract_test.rs` of `src-tauri` and of
`test-server`). `canceled` is an action the user stopped (the wait for a lock, a sign-in, an action of a CLI tool, a
review fetch: `LockWaitCanceled`, `SignInError::Canceled`, `DeviceFlowError::Canceled`, `ActionError::Canceled`,
`StoppedCommand::Cancelled`); `internal` is a failure without a code of its own, which the app answers, while `bd`
prints a failure of git or IO as `git_error`.

Agents branch on the code and the exit code, never on the message. `commits` names the commits an error is about.
`retryable` is true for `head_moved`, `groups_moved`, `stacks_moved`, `parked_moved`, `round_moved`, `plan_changed`, `locked`, `busy` and `network_error`: retry when it is
true, whatever the exit code. It is the `retry` class of the code other than `no`: `same` (`locked`, `busy`, `network_error`) runs
the same command again, `after_reread` (`head_moved` and the other `*_moved`, `plan_changed`) builds it again from a new read. The message of `locked` names the holder when it wrote its record (`` `bd check --fix` (pid 4242, for 12
min 3 s) holds …/branch-deck.lock (waited 10.0 s); retry when it is done``), else "Another Branch Deck operation".
`busy` (`bd push`, `bd safe-push`, `bd request-review`; of a rewrite only `bd reapply` while another process pushes
the unapplied branch) names the operation of another process that holds what the command needs, from the operations
registry of the repository (docs/design/git-operations.md, "Operations registry"):
its `error.holder` is `{op, tool, pid, kind, subject, held_for, unknown}` (`kind` `safe_push`, `dry_run`, `push`, …;
`op` the `operation_id` of its log lines), left out for a registry of another Branch Deck, whose operations can't be
read. `unknown: true` is a start whose answer was lost (its request went out and no answer came, or its process was
killed after it went out): it holds until Patronus lists its robot, which an admission reads first (a robot of it
started: `plan_changed`; none once 30 s passed: the command goes on), or until `bd operation forget <op>`.

A write whose step after the write failed (the descriptions or the sync of `bd group --sync`, the sync of
`bd move-hunk --sync`) prints its output with `after_error`, never the error object: the write stands, its
`undo_token` reverts it, and the exit code is that error's. Retry that step (`bd sync`), never the write.

The names are one vocabulary across the commands: a write prints `previous_head` and `new_head`; a commit and its
copy are `{commit, new_id}`; a state is `state`; a list of commits names each by `commit`.

A list too long to print whole is cut at its limit with the count of the rest (`files_omitted`,
`blocked_paths_omitted`, the `omitted` of `bd robot --full`, the "(2 more: bd check)" of the dangling references a
refusal describes), never leaving out only one: a list one longer than its limit is printed whole
(`sync_utils::list_cut::kept`, the rule of the app's lists and prompts, `cutList`), so a count is 0 or at least 2.

## Commands

### `bd status [--branch <b>] [--unassigned] [--limit N] [--offset M] [--stats [--include-generated]] [--brief]`

Prints HEAD, the baseline (as `detect_baseline_branch` finds it), the prefix, the branches and the unassigned commits. Without a
baseline (no upstream and no `origin/master` or `origin/main`, or no remote and no local `master` or `main`) it is
`no_baseline`. It groups
the commits the same way a sync does (`sync_core::grouping`, the first phase of a sync without its events). It does no
integration detection (except for a stacked parent whose commits left HEAD, below), no update check and no network
command of git. It runs about eight short git processes, with one `log` for all the commits: HEAD, then the baseline
and the branches under the prefix in one listing, as a sync reads them, and the push URL of `origin`, which tells
whether the repository is on Space; on Space it reads the user's robots (`landed_by`, below) from the object cache
while they hold (a minute while one of them runs, five minutes while none does), else asks Patronus for the robots
started since and polls the running ones, as the app does; `bd push`, `bd safe-push` and `bd robot` ask fresh and keep
what they listed.

```json
{"schema":2,"repository":"/repo","prefix":"develar","prefix_source":"global_config",
 "head":{"id":"ff366ae0","branch":"master"},"baseline":"origin/master","branch_count":2,"unassigned_count":1,
 "branches":[{"name":"ABC-12","ref":"refs/heads/develar/virtual/ABC-12","issue_key":"ABC-12", "...": "..."},
  {"name":"auth","ref":"refs/heads/develar/virtual/auth","latest_time":1700000400,"commit_count":2,
   "tip":"2c1d0e93","commits":[{"id":"9f3a2b1c","subject":"Add logout"},{"id":"41c7d00e","subject":"Add login"}]}],
 "unassigned":[{"id":"77e01aa4","subject":"Tidy up"}]}
```

- Branches are listed newest first by where each begins in the history, the same order as the UI: the branch whose
  oldest commit is the newest first, so no rewrite of the commits of a branch and no commit added to it moves it
  ([Stacked Branches](../../spec/branch/stacked-branches.spec.md#ui)). The commits of a branch are newest first, and
  their subjects have the `(prefix)` stripped, as the virtual branch commits do (a prefix that is an issue key stays as
  the key). `resolved_by_words` (`{"conflicts","file_count"}`, from the mapping
  note) marks a commit whose virtual copy resolved a conflict of git by words (see `bd sync`).
- `tip` is the tip of the existing virtual branch, or `null` if the branch was never synced.
- `origin` says what groups the commits: `prefix` (a commit has the `(prefix)`), `issue_group` (the issue groups put
  commits of other keys into it) or `issue`. `issue_keys` lists the keys whose issue group the branch is.
- `idle_issue_groups` lists the keys of the issue groups that group no commit ahead of the baseline on this branch, and
  `issue_group_problems` (`{line, message}`) the lines of the store that are not used; both are left out when empty.
  The map is shared by every branch and worktree, so an idle key may group the commits of another one: leave it,
  unless you know that no branch needs it (then `ungroup_issues`).
- With stacked branches on, `parent_source` says where `parent` comes from: `prefix` (a parent level of a prefix) or
  `store` (the stack parents, `refs/branch-deck/stack-parents`, see [Stack Parents](../../spec/branch/stack-parents.spec.md)).
  `idle_stack_parents` lists the children of the stack parents that stack no branch ahead of the baseline here, and
  `stack_parent_problems` the lines of that store that are not used. An idle child may stack a branch of another
  worktree; a sync drops its line once its branch is integrated, `bd group` with `unstack` drops it at once.
- `--branch` lists that one branch and `--unassigned` lists only the unassigned commits. `--limit`/`--offset` page
  the commits in both of these modes, and page the branches otherwise.
- With stacked branches on (`git config branchdeck.stacks true`), each branch is placed as a sync places it:
  - `parent`, the parent its commits declare, and `parent_state`: `active` (it has commits), `integrated` (its branch
    is integrated), `unapplied` or `missing`. A parent without commits, in any of these states, doesn't block: the
    branch goes on the nearest active ancestor or the baseline and has `pr_base_changed: true` (it is not built on the
    declared parent; its pull request goes into the baseline as every one). A parent whose commits left HEAD since the last sync is detected now, as the next
    sync detects it, without writing the detection note;
  - `effective_base`, the branch it is built on (omitted for the baseline), or `blocked_by` `{parent, reason}` when the
    parent can't be built: `problem` (a stack problem), `error` (a failed build), `conflict` or `blocked`;
  - `problems` (`{message, commits}`): conflicting parents, a cycle, a self parent, a case collision, an empty level.
    The message ends with its fixes as commands, the same fixes the app offers as buttons: `fix: `bd restack login
    auth`, `bd restack login docs` or `bd unstack login`` (`bd rename` for a case collision, `bd reword` of the
    commits for an empty level and for a self parent beside another parent).

  The output has `forest`: the branches with commits, each after its parent.
- With stacked branches on, `stacks` lists the stack branch of each stack that forks: one more virtual branch,
  `{prefix}/stack/<root>` (`branch`), of all the stack's commits in the order of the history (see `bd sync`), as the
  last sync left it; left out without one, and with `--branch` or `--unassigned`. Its `state` comes without a sync and
  without a git process: `blocked` (with `member` and `reason`) when a member has a stack problem or is blocked, else
  `built` when the object cache remembers the build of the current commits and the ref is at it, `conflict` when it
  remembers their build conflicting, else `stale` (`error` says why: a new commit since the sync, or never built). With
  `--no-cache` a built one reads `stale`. `bd status` has no `remote`: `bd sync` says what a push sends.

  ```json
  "stacks":[{"root":"auth","branch":"develar/stack/auth","state":"built","tip":"5e6f7a8b","commits":5,
    "members":["auth","login","session"]}]
  ```
- `landed_by` names the merged Safe Push that landed the branch, until the update archives it, as the rows of the
  app say it: `branch`, whose Safe Push it was (the branch itself, a branch built on it, or the root of the whole
  stack with `whole_stack`), `robot` (`id`, `state`, `url`, …), `commits`, the commits of the branch it landed, and
  `after`, the commits added since, the only work left of the branch, which its push and its Safe Push send. It is
  decided by content (`sync_core::robot_landing`: the commits the robot tested, the same commit, a patch-equivalent
  copy or a copy with the same changes on a newer base, against the branch's), so a new commit or a reword of a branch
  it is built on keeps it; an amend of a landed commit leaves the branch without it. A branch with no `after` is all
  in the target: `bd push` leaves it out (`skipped`) and `bd safe-push` refuses it (`landed`). Off Space, without the
  Safe Push CLI's token or an answer of Patronus there is none. A robot at the tip the sync left needs no git; one
  whose branches got new copies costs two `rev-list` and at most a `diff-tree` and a `patch-id`.

  ```json
  "landed_by":{"branch":"oauth","robot":{"id":"7f…","name":"…","state":"success","mode":"safe_push",
    "commit":"9a8b7c6d","url":"https://patronus…/robot/7f…"},"commits":["1a2b3c4d"],"after":["5e6f7a8b"]}
  ```
- `landing_needs` (on Space) lists what the Safe Push of the branch needs of a commit outside what it lands: the
  refusals `bd safe-push` would give its start, from one trial check of every branch in one session
  (`trial_check::landing_gate::landing_needs`, the rule of the gate; about the processes of a `bd check`), less what
  merged Safe Pushes landed. Each need has its `code` (`missing_declaration`: a commit uses a name only that commit
  adds; `dangling_reference`: a commit removes or changes a declaration a use still has, which only that commit
  changes), the `identifier`, `at` (`file:line`), the commit it `needs` and its `needs_branch`, the `fix` and
  `landing_in`, the robot of the running Safe Push that lands `needs_branch` (its robot tests that branch without this
  one). A branch built on one that needs a branch needs it too. `[]`: nothing; absent off Space, with `--brief` and
  when the check can't run, which is unknown, not nothing. Show the user the need and the fix; restack only after they
  approve.

  ```json
  "landing_needs":[{"code":"missing_declaration","identifier":"Stills","at":"lane/src/BuildLog.kt:3",
    "needs":"ecebf000","needs_branch":"stills-codec","fix":"bd restack lane-run stills-codec"}]
  ```
- `parked` says the user keeps the branch local (`bd park`, the app's Park Branch): `{note}`, and `with` when a branch
  it is built on is the parked one, whose commits it carries. `bd push` and `bd safe-push` refuse it (`parked`).
  `idle_parked` lists the parked names that group no commit here (another worktree's, or gone); a sync drops one once
  its archived branch is integrated, `bd unpark` any.
- `description` is `{title}` of the branch's `bd describe` text, read for all branches with one
  `git config --get-regexp`.
- `--stats` adds `stats` to each listed branch:

  ```json
  "stats":{"files":4,"generated_files":1,"additions":7,"deletions":1,
    "top_dirs":[{"dir":"src/api","lines":7},{"dir":"docs","lines":1}],
    "authors":[{"author":"Name <mail>","commits":3}],"first_time":1700000100,"last_time":1700000300,
    "issue_keys":["PROJ-7"],"diffstat":{"base":"3c0ffee1","files":4,"additions":6,"deletions":0}}
  ```

  - `files` counts the distinct files the commits change, and `generated_files` how many of them are lock or
    generated files (see `bd commits`).
  - `additions`, `deletions` and `top_dirs` (at most three directories, by added plus deleted lines) are summed over
    the commits. Generated files don't count without `--include-generated`.
  - `first_time` and `last_time` are author times. `issue_keys` are the distinct keys of the subjects.
  - `diffstat` is the net change of the synced virtual branch: its tip against `base`, the parent of the oldest
    commit ahead of the baseline, which a sync rebuilds every branch on. It is exact while the tip is synced with
    the current commits, and `null` for a branch that was never synced.
  - It costs at most three more git processes for all branches together: the commit diffs the cache misses (the same
    `log --raw` and blob pass and cache as `bd commits`), one `diff-tree --stdin --raw --numstat -z` with a `<tip> <base>` line per
    synced branch (`git_ops::commit_files::read_files_from_base`, the parser of the commit files), and one
    `check-attr` (`git_ops::attributes`).
- `--brief` prints `StatusBrief` (`status_brief` in `bd schema`), small enough to read whole: about 2 KB for the 12
  branches and 74 commits of a large monorepo branch, where `bd status` prints 11.5 KB.
  - Per branch it lists `name`, `commit_count`, `issue_key` (left out when it is the `name`) and `newest`, the subject
    of the newest commit, `issue_keys` of an issue group, `landed_by` as `{branch, whole_stack, after}`, `after`
    the count of the commits left, and `parked`. With stacked branches on, it keeps `parent`,
    `parent_source`, `blocked_by`, `pr_base_changed`, `problems`, `forest` and `stacks`. It keeps `idle_issue_groups`,
    `issue_group_problems`, `idle_stack_parents` and `stack_parent_problems`.
  - `unassigned` lists the ids and subjects.
  - It reads what `bd status` reads, except the virtual branch tips and the descriptions.
  - It works with `--unassigned`, `--limit` and `--offset`, but not with `--branch` or `--stats`.

  ```json
  {"schema":2,…,"head":{"id":"ff366ae0","branch":"master"},"baseline":"origin/master","branch_count":2,
   "unassigned_count":1,"branches":[{"name":"auth","commit_count":2,"newest":"Add logout"},
    {"name":"ABC-12","commit_count":1,"newest":"ABC-12 Fix crash"}],
   "unassigned":[{"id":"77e01aa4","subject":"Tidy up"}]}
  ```

### `bd commits [<id>…] [--files] [--patch] [--branch <b> | --unassigned] [--include-generated] [--brief [--depth N]]`

Prints every commit ahead of the baseline, oldest first, which is the order they are applied in.

```json
{"schema":2,"repository":"/repo","prefix":"develar","head":"77e01aa4","baseline":"origin/master",
 "commits":[{"id":"41c7d00e","subject":"(auth) Add login","body":"...","author":"Name <mail>","time":1700000100,
   "group":"auth","files":[{"path":"auth.rs","status":"A","additions":12,"deletions":0}]},
  {"id":"5e2f9c3b","subject":"fixup! (auth) Add login","group":"auth",
   "fixup":{"kind":"fixup","target":"41c7d00e","target_subject":"(auth) Add login"}, "...": "..."}],
 "dependencies":[{"from":"5e2f9c3b","to":"41c7d00e","files":["auth.rs"]}],
 "cluster_overlap":[{"branches":["auth","login"],"files":["auth.rs"]}]}
```

- `group` is the branch the commit is grouped into, or `null`. `issue_key` is the issue key of the subject.
- `resolved_by_words` marks a commit whose virtual copy resolved a conflict of git by words, as in `bd status`.
- `fixup` is set on a `fixup!`, `squash!` or `amend!` commit. Its `target` is resolved with `git rebase --autosquash`
  rules (`plan_autosquash`), and is `null` when no older commit matches.
- `--files` lists the changed files of each commit. `status` is `A`, `M`, `D`, `R` (`old_path` is set) or `T`.
  `additions` and `deletions` count the changed lines of the diff views. They differ from `--numstat` for a change of
  the line ends alone, which counts no line. A binary file has `"binary": true` and no counts.
- `--patch` adds `patch`, the changed files of each listed commit with their blocks: the blocks of the diff views of
  the app, never merged with a neighbour.

  ```json
  "patch":[{"path":"auth.rs","status":"M","blocks":[{"id":"27,1:27,1@656914d1","old_start":27,"old_lines":1,
    "new_start":27,"new_lines":1,"kind":"changed","header":"@@ -24,7 +24,7 @@ Auth › login",
    "text":" line 24\n line 25\n line 26\n-old\n+new\n line 28\n line 29\n line 30"}]}]
  ```

  - `id` is what `bd move-hunk --block` takes: the lines of the block as in a git hunk header, and a hash of its lines.
    An id of a block that changed since does not match.
  - `old_start` and `new_start` are 1-based. A side without lines names the line before them, as git does.
  - `kind` is `changed`, or `whitespace_only` for lines that differ in whitespace or in the final line end only.
  - `header` counts the lines of `text` and names the declaration that holds the block, when the language is known.
  - `text` has the lines of the block between up to three lines of context. The context stops at the next block.
    A final line end added or removed is a block on the empty last line.
  - A file without text (`binary`: a binary file, a symlink, a submodule, a type change, over 8 MiB) has no blocks.
- `--branch` keeps only the commits of that branch, and the edges and overlaps that involve it. `--unassigned` keeps
  the unassigned commits, and `<id>…` the commits with these ids: full, or abbreviated to at least 4 characters, as in
  a plan. An id that matches no commit ahead of the baseline is `not_ahead_of_baseline`, one that matches several is
  `invalid_arguments`. The three filters exclude each other. The dependencies are still computed over every commit;
  the overlaps kept are those of the branches the listed commits are in.
- `dependencies` are edges where `from` depends on `to`, an older commit, through `files`. An edge by symbols (see
  "How dependencies are approximated") has `via`: `symbols` when only the names link the two (its `files` are the
  files of the uses), `lines_and_symbols` when the lines do too, and `symbols` `[{name, file}]`.
- `cluster_overlap` lists, for each pair of current branches, the files both change.
- Lock files and generated files don't count in `dependencies` and `cluster_overlap` (see below);
  `--include-generated` keeps them. `--files` still lists them, with `"generated": true`.
- `--brief` prints `CommitsBrief` (`commits_brief` in `bd schema`): what an agent needs to cluster, small enough to
  read whole. Measured on the 74 commits of a large monorepo branch, it is 19.5 KB, where `--files` is 278 KB.
  `--files` and `--patch` don't go with it; the filters do.

  ```json
  {"schema":2,…,"head":"77e01aa4","baseline":"origin/master",
   "commits":[{"id":"41c7d00e","subject":"Add login","group":"auth","areas":[{"dir":"src/auth","lines":12}]},
    {"id":"5e2f9c3b","subject":"fixup! Add login","group":"auth","fixup":{"kind":"fixup","target":"41c7d00e"},
     "areas":[{"dir":"src/auth","lines":2}]},
    {"id":"9f3a2b1c","subject":"ABC-1 Add parser","group":"ABC-1","areas":[{"dir":"src/parser","lines":30}]}],
   "dependencies":[{"from":"5e2f9c3b","to":"41c7d00e"}],
   "units":[["41c7d00e","5e2f9c3b"]]}
  ```

  - Per commit it lists `id`, `subject`, `group`, `issue_key`, `fixup` and `areas`, without the body, the author and
    the files. It leaves out what another field says: the subject drops a `(prefix)` that is exactly the `group`, and
    `issue_key` is left out when it is the `group`. A stack path, whose leaf is the `group`, and a prefix the grouper
    sanitizes (`(Foo Bar)` is in `Foo-Bar`) stay in the subject, so a plan made from the brief view keeps them.
  - `dependencies` are `{from, to, via}`, without the files.
  - `units` are the commits that must stay together: the connected components of the dependencies and of the fixups
    with their targets, over every commit. Only components of two or more commits are listed, ordered by their oldest
    commit, each oldest first. With a filter, the units that hold a listed commit are kept whole.

#### Areas

`areas` (`crates/commit-clustering/src/areas.rs`) say where a commit belongs without its file list:

- The area of a path is its first `--depth` directories (3 by default), or all of them for a shorter path, and `.`
  for a file at the top level: `plugins/air/shared/src/Foo.kt` is in `plugins/air/shared`.
- Per commit, at most three areas with the most added plus deleted lines. An area under 5 % of the commit's lines is
  left out, unless it is the first. An area of renames or binary files only has 0 lines, so it shows only when the
  commit has no other lines.
- Lock files and generated files don't count without `--include-generated`. Hub files never count: changes in many
  unrelated areas touch them, so they say nothing about where a commit belongs. `HUB_FILES` in
  `crates/commit-clustering/src/hub_files.rs` lists them: module and build registries, file lists and top-level docs. It matches
  by file name, and a pattern with a `/` (`.idea/modules.xml`) by the end of the path.
- `top_dirs` of `bd status --stats` keeps its meaning: the full directories of the files.

All changes, the file lists and the dependencies come from one `git log --no-walk=unsorted --stdin -z --raw
--no-abbrev` process and one `cat-file --batch` of the blobs, for the commits the object cache misses. The raw part
gives the full ids of each file's blobs before and after. The line counts and the line ranges of the dependencies are
the blocks of the diff views under `Default`, so `bd commits` and the app count the same lines. `--patch` adds one
`cat-file --batch` of the blobs of the listed commits. The log passes explicit diff options (`-M`, `--no-ext-diff`,
`--no-textconv`), and the blocks come from the diff engine of the app, so the user's diff config doesn't change the
output.

#### Lock files and generated files

Nobody edits them by hand, and they link every commit that touches them, so by default they are left out. A file is
generated when:

- its name is a lock file: `Cargo.lock`, `pnpm-lock.yaml`, `package-lock.json`, `yarn.lock`, `go.sum`,
  `poetry.lock`, `Gemfile.lock` and the others in `LOCK_FILES` (`crates/generated-files/src/names.rs`);
- or the attributes of HEAD mark it `linguist-generated` or `-diff` (which `binary` implies). One
  `git check-attr --source=HEAD --stdin -z` process checks all paths (`crates/generated-files/src/attributes.rs`).

Other generated files, such as bindings or snapshots, count until `.gitattributes` marks them, e.g.
`src/lib/bindings.ts linguist-generated`.

#### How dependencies are approximated

This is in `crates/trial-check/src/dependencies.rs`.

- Every line a commit adds is owned by that commit. The owned line ranges of each file are carried forward through
  the later commits.
- A commit depends on an earlier one when one of its blocks (the blocks of the diff views) replaces or deletes an owned
  line, or is adjacent to one. Git's three-way merge conflicts on adjacent changes too.
  - `-a,b` with `b > 0` touches lines `a-1` to `a+b`.
  - A pure insertion after line `a` touches lines `a` and `a+1`.
- A change to a file created in the range depends on the commit that created it.
- Re-adding a deleted file depends on the commit that deleted it.
- A binary file or a type change depends on the previous commit that changed the file.
- Renames are followed. Copies are not.

The approximation sees only text. It misses semantic dependencies, such as a call to a function that another commit
adds in another file. Edits next to each other do conflict in git's merge. Rarely, an edge is one git's merge would
not conflict on: the blocks come from the diff engine of the app, git's merge uses histogram, and in a large rewrite
the two can disagree about which commit added a line. Treat an edge as
"these can't be cherry-picked apart without a conflict", not as a proof.

The edges by symbols (`crates/trial-check/src/symbol_dependencies.rs`, unless `branchdeck.symbols` is `off`) add those: a
commit uses a name another commit of the range adds, with evidence that the use means that declaration (an import, a
qualified name, the same file or Rust module, a type of the same package and module, an override in a subtype whose
supertype resolves, a call in a subclass of a member its one supertype has, a use on a receiver of the member's type, a
Java use through the accessor or the file class of a Kotlin declaration), or removes a declaration whose uses another
commit removed. Of a Kotlin or Java type a declaration is each overload and each constructor, and a new form of one (a
wider reach, `open`, a default, a new or renamed parameter) counts as added for the uses that only it accepts; a call
links to the overloads its arguments select. They come from the syntax trees of Kotlin, Java, Rust, TypeScript and
JavaScript, cached by blob id. Such an edge (`via: "symbols"`) means "this doesn't compile without that", which a stack
or the order of two Safe Pushes satisfies; it never joins `units`.
See [Symbol Dependencies](../design/symbol-dependencies.md).

### The object cache

The changes of a commit never change, so `bd commits` and `bd status --stats` keep them in a disk cache keyed by the
commit id (`crates/object-cache`, LMDB through heed):

- It lives in `$GIT_COMMON_DIR/branch-deck/cache/`, so all worktrees share it. The app and parallel `bd` calls can use
  it at once: LMDB serializes writers through its lock file, and readers never wait.
- git runs only for the ids the cache misses, in one batch process, and the results are written back in one
  transaction.
- The cap is the LMDB map size, 1 GiB by default. A write that finds the map full clears the cache and is retried.
- Each store's version is part of its database name (`commit_diff.v1`). Opening drops the older versions of a store,
  so a new value type of one store keeps the entries of the others.
- Any cache failure is a miss, logged with `-v`; it never fails a command. `--no-cache` bypasses it.
- `bd check` memoizes its merges in `merge_result` ((base, commit, target) trees → result tree or conflict files) and
  commit trees in `commit_tree`. Sync doesn't use them yet.

### `bd group [--dry-run | --sync] [--expect-head <id> [--allow-append]] [--map] --plan <file | ->`

Moves commits between branches by their subject prefixes, in one message-only history rewrite
(`history_rewrite::regroup`, see "Regroup, rename, restack" in [Git Operations](../design/git-operations.md)), and changes
the issue groups and the stack parents in the same ref transaction. Then it sets the pull request texts of the plan under
the same repository lock and, with `--sync`, rebuilds the virtual branches after its release, as `bd sync` does: another
rewrite may come in between, and the sync then shows the newer HEAD. The plan is the file of `--plan`, or stdin for
`--plan -`; without `--plan` it is `invalid_arguments`, and stdin is never read.

```json
{"branches": {"auth": ["41c7d00e", "5e2f9c3b"], "auth/login": ["9f3a2b1c"]}, "unassign": ["77e01aa4"],
 "issue_groups": {"parser": ["ABC-1", "ABC-2"]}, "ungroup_issues": ["XYZ-7"], "unstack": ["docs"],
 "descriptions": {"auth": {"title": "Add login", "body": "The login form."}}}
```

- A path is a branch name, or `parent/leaf` with stacked branches on (`git config branchdeck.stacks true`). Ids may
  be abbreviated. `unassign` removes the prefix: the commit falls back to its issue branch or becomes unassigned.
- A path `parent/leaf` stacks `leaf` on `parent` in the stack parents (`refs/branch-deck/stack-parents`), not in the
  subjects: a commit that needs a prefix gets the bare `(leaf)`, and a commit already in `leaf` by its issue key or
  issue group keeps its subject. So stacking whole issue branches (`{"IJAI-3992/IJAI-4047": [ids]}`, or an empty list)
  rewrites nothing. A commit whose prefix declares another parent for a branch the plan stacks loses that level.
  `unstack` makes branches roots: their line goes, and so do the parent levels of their commits; a bare path never
  unstacks.
- A commit without a prefix that the plan puts into the branch of its own issue key keeps its subject: it is in that
  branch already. So a plan that names the current branch of each commit (its stack path with stacked branches on, not
  the leaf) rewrites no commit that has no prefix or a canonical one. A prefix the grouper sanitizes, such as
  `(Foo Bar)` in `Foo-Bar`, is rewritten to the name.
- `issue_groups` maps an issue group name (a branch name without `/`) to issue keys: the commits without a prefix that
  carry one of the keys go into that branch, and no subject changes. `ungroup_issues` removes the issue group of a
  key, so its commits go back to the issue branch of the key. The map lives in `refs/branch-deck/issue-groups`; see
  [Issue Groups](../../spec/branch/issue-groups.spec.md). A plan of issue groups only rewrites nothing
  (`rewritten` 0) and moves only that ref, still with `--expect-head` and a check that HEAD did not move.
- Before anything is written, the plan is applied in memory to every subject ahead of the baseline and validated,
  with the issue groups as they are after the plan:
  - unknown ids and ids not ahead of the baseline are `not_ahead_of_baseline`;
  - duplicate ids, ids both in a branch and in `unassign`, invalid names, a fixup sent away from its target and a
    stack problem the plan introduces (a cycle, conflicting parents, a self parent, a case collision) are
    `invalid_plan`; problems that already existed are listed in `problems`;
  - a key in two issue groups, or in an issue group and in `ungroup_issues`, a key to ungroup that is in none (and in
    no line of the store), an invalid key or name, a key that no commit without a prefix carries, and a fixup the issue
    groups would separate from its target (`fixup! <id>` has no issue key) are `invalid_plan` too;
  - a path whose leaf is its parent, two paths that give a branch two parents, a branch both in a path and in
    `unstack`, and an `unstack` of a branch that is not stacked (or with stacked branches off) are `invalid_plan`.
- `descriptions` maps a branch (the group name, as for `bd describe`: the leaf of a stacked branch) to its pull
  request text `{title, body}`. bd
  validates it with the plan: a title of one line that is not empty, and a branch that has commits after the plan (a
  prefix branch, an issue branch or an issue group); else `invalid_plan`, and nothing is written. After the ref
  transaction bd writes the texts that differ from the current ones, as `bd describe` does: one `git config` process
  per branch, because git has no batch writer for config. A plan of descriptions only rewrites nothing, and
  `--expect-head` stays optional, as for any plan.
- Fixups not in the plan follow their targets.
- `--expect-head` refuses with `head_moved` (exit 3) unless HEAD is still that commit; the rewrite itself only moves
  the branch from the HEAD it read (compare-and-swap). With `--allow-append` a HEAD with commits on top of it is
  fine while that commit is still ahead of the baseline (a peer appended, nothing the plan was made for changed): the
  commits on top are regrouped with the rest and listed in `appended` (`{commit, subject}`, new ids). `rename`, `restack`
  and `unstack` work by name, so they rename the prefix of an appended commit too.
- The repository lock serializes rewrites with the app and other `bd` calls; after 10 s of waiting it is `locked`
  (exit 3). A rebase, merge, cherry-pick or revert in progress is `operation_in_progress`, a detached HEAD
  `detached_head` (exit 4).

```json
{"schema":2,"repository":"/repo","prefix":"develar","operation":"group","dry_run":false,
 "previous_head":"77e01aa4","new_head":"c0ffee12","rewritten":3,"undo_token":"<full previous>..<full new>",
 "groups":{"auth":["1b2c3d4e","5f6a7b8c"],"login":["9d0e1f2a"]},"unassigned":["c0ffee12"]}
```

- `groups` and `unassigned` list every commit ahead of the baseline afterwards, with the new ids, oldest first.
- `issue_groups` (only when they change) lists every issue group afterwards with its keys. A write changes only the
  keys the plan names; the keys of other branches and the lines bd can't read stay.
- `stack_parents` (only when they change) lists every stack parent afterwards, child -> parent, those of other
  worktrees included.
- `undo_token` holds full ids, the parts of the refs that moved joined by `+` in this order: `<previous head>..<new
  head>` for a rewrite, `groups:<previous>..<new>` (commits of the issue groups) and `stacks:<previous>..<new>`
  (commits of the stack parents). A previous store commit is zeros when there was no store before. Pass the token to
  `bd undo` as printed.
- `--dry-run` writes nothing and adds `changes` (`{commit, old_subject, new_subject}`) and, with stacked branches on,
  `forest` (`{name, parent, parent_source, has_commits, problems}` per branch). `--map` adds `map`, `{commit, new_id}`
  of every commit.
- `described` lists the branches whose description the plan wrote (with `--dry-run`: would write), and
  `replaced_descriptions` the texts they replaced, by branch. `bd undo` doesn't restore a description; `bd describe`
  can, with the replaced text.
- `--sync` runs `bd sync` after the writes, once the rewrite released the repository lock (the sync takes none, so its
  merges never make the app or another `bd` wait; a rewrite between them only makes the sync stale, never wrong:
  every ref moves by compare-and-swap), and adds its output as `sync`. The exit code is 1 when a branch was not built, as for `bd sync`. `--sync` doesn't go with
  `--dry-run` (`invalid_arguments`): a dry run writes nothing, and `bd check` is the trial sync of a plan.
- `error` (`{code, message, commits, retryable}`, as in an error output) is set when a step after the rewrite fails:
  a description write, which also skips the sync and leaves the later descriptions unwritten, or the sync. The
  rewrite stands, `undo_token` reverts it, and the exit code is that error's.
- `bd undo` moves back only HEAD, the issue groups and the stack parents: the descriptions stay, and the virtual
  branches stay as the sync built them until the next `bd sync`.

### `bd rename <old> <new>`, `bd restack <branch> <parent>`, `bd unstack <branch>`

Plans that `bd` computes, validated and written like `bd group`, with the same options (`--sync` too) and output:

- `rename` replaces the level `old` in every prefix path, so a parent's children follow, renames the issue groups
  named `old` and names an issue branch by the issue groups, without a rewrite, and renames `old` in the stack parents
  (as a child, and as the parent of a branch with commits here). The new name must not contain `/`. A name that exists
  merges the branches.
- `restack` writes the stack parent of the branch, `unstack` removes it; both rewrite only the commits whose prefix
  declares another parent. An issue branch and an issue group are stacked like any branch, without a rewrite. Both
  need stacked branches on; `unstack` of a root is `invalid_plan`.

### `bd park <branch> [--note <why>]`, `bd unpark <branch>`

Keep a branch local, or let it be pushed again ([Parked Branches](../../spec/branch/parked-branches.spec.md)): only when
the user asks. No rewrite: one line `branch<TAB>note` in `refs/branch-deck/parked`, shared by every worktree, under
the repository lock with compare-and-swap (`parked_moved`, exit 3, when another writer moved it). `park` needs a branch
that groups commits here (`unknown_branch`); a second park changes the note (one line, at most 500 characters,
`invalid_arguments`). `unpark` drops the line, also of a branch without commits here (`idle_parked`); `not_parked`
(exit 2) when there is none. A rename carries the line to the new name.

```json
{"schema":2,"repository":"/repo","branch":"build-wrappers","parked":true,"note":"waits for the fix of IJPL-1",
  "changed":true}
```

### `bd mark-reviewed <branch> [--base <parent>]`

Keeps the base and the tip a branch has now as reviewed, experimental ([Review Plan](../../spec/branch/review-plan.spec.md)):
only when the user says they reviewed the branch. The branch is a virtual branch by its name, else a local branch by
its name, or `refs/heads/<name>` (`unknown_branch`, exit 2, for none); the base is its merge base with the baseline of
the checked out branch, or with `--base` (a branch named the same way, else any commit): for a branch stacked on
another, `--base <parent>` keeps only its own commits as reviewed. No rewrite:
`refs/branch-deck/reviewed/<sha256 of the branch>/{base,tip}`, in one ref
transaction with compare-and-swap that also deletes the pairs of the branches that are gone (`pruned`); no repository
lock ("Review plan" in [Git Operations](../design/git-operations.md#review-plan)). `previous` is the pair it replaced,
`changed` false when the branch was marked at this base and tip already.

```json
{"schema":2,"repository":"/repo","branch":"jane/virtual/auth","ref":"refs/heads/jane/virtual/auth","base":"1a2b3c4d",
  "tip":"5e6f7a8b","previous":{"base":"1a2b3c4d","tip":"9c0d1e2f"},"changed":true,"pruned":0}
```

- `unknown_branch` (exit 2): no branch has the name.
- `invalid_arguments` (exit 2): `--base` names no branch and no commit, or shares no history with the branch.
- `no_baseline` (exit 4): no baseline, or one that shares no history with the branch.
- Each refusal writes no ref.

### `bd review-plan <branch | base..head> [--base <parent>] [--since-reviewed]`

The plan of a review, experimental ([Review Plan](../../spec/branch/review-plan.spec.md)); moves no ref. The branch is
named as for `bd mark-reviewed`, on the same base (`--base <parent>` for a stacked branch); `base..head` is a range.
For a branch marked reviewed, `interdiff` pairs each commit of `base..head` with a reviewed one: `unchanged` (the same
commit, or a copy with the same identity and the same changed lines, or a reword), `rewritten` (the same identity,
other lines: `new_only` and `reviewed_only` per path), `added` or `dropped` (listed last). `scope` is what changed
since the review as one diff, `git diff <base> <head>`: the plain range of the commits added on top when nothing else
changed; else (`carried: true`) from the reviewed change carried onto the base, a commit on no ref, so an amend shows
against its reviewed version and a rebase adds nothing; `null` when that carry conflicts. `--since-reviewed` lists
only the added and the rewritten commits, and reads the API and the files from the diff of the scope. A branch never marked and a range have `"interdiff": null` ("Review plan" in
[Git Operations](../design/git-operations.md#review-plan)).

`api` lists the public declarations the reviewed commits add, remove or change in form (`kind`, `changes`, the form
`before` and `after`), no override, the most usages left outside the change first. `usages` comes from `accio
references` at the head, asked by `Owner.name` for a member and by `package.name` for a top-level Kotlin or Java
declaration, for the first 20 queries: `inside` counts the uses on the lines the commits changed, `outside` the others (also on a line of a changed file that the change left), with the
first 5 as `outside_sites`, and `complete: false` past 100 hits. An import, a doc mention and a hit in another
language family are no use, and of a type whose constructors are its only change only a call and a subtype are. A
name that nearly
every type declares (`equals`, `toString`, `get`, `run`, `new`, `invoke` and others) is never asked without an
owner: its `usages` is `null`. A member of that name counts only the lines that name its owner, as a lower bound
(`complete: false`). A removal names the file and line before the commit that removed
it. `files` gives per changed file (a deleted one too) its `old_path`, `mechanical` (`buildModel`, `importsOnly`,
`whitespace`, `renamed`, `generated`, or `null` for real work, a deletion too), `generated` (why, for a `generated` file:
`{"kind":"name","name":"lockFile"}` for a lock file, a minified file or a source map, `{"kind":"attribute"}` for
`linguist-generated` or `-diff` at the head, `{"kind":"header","line":"Generated by …"}` for the leading comment a
tool wrote; [Review Plan](../../spec/branch/review-plan.spec.md#mechanical-changes)), `uses` and `used_by` (the links among the changed files
after the commits), `new_links`, `breaks` (the usages left outside of the API it removes or changes), `breaks_complete` (false makes
`breaks` a lower bound: a usage count of that API is `null` or not `complete`), `untested` (a source file
changed while none of its tests did), `repeated_edit` and `repeated_edit_only` (every hunk of it is that edit), the most
`breaks` first; a generated file is in no repeated edit. `links` lists which changed file
uses which (`from`, `to`, `symbols` with the line of the first use, `state` `added`, `removed` or `kept`,
`cross_module`, `new_between_modules` when it is also added); `dependencies` the entries the commits add to, remove from or change in the build files: the
`orderEntry` of an `*.iml`, the `depends`, `dependencies` and `content` of a plugin descriptor, the dependency tables of
a `Cargo.toml` and the dependencies of a `package.json` (`manifest`, `module`, `kind`, `name`, `change` `added`,
`removed` or `changed`, `version_before`, `version_after`, `scope`, `scope_before`), a new or deleted build file as one
`newModule` entry; a build file that changed only in these entries is `buildModel`. `repeated_edits` lists the groups
of 3 or more files that get the same edit. The reviewed commits are those of `base..head`, with
`--since-reviewed` only the added and rewritten ones. Without accio or its index, `usages` is `null` and `index_note`
says why; a failing accio never fails the plan. `order` lists every changed path once (a deleted one too) in
dependency order: a file that declares what another imports or uses before it, a test right after its subject, the
tree order inside a cycle and between files with no edge (`"schema": 10` since the `steps` of the story and the `brief`
([Review Tour](../../spec/branch/review-tour.spec.md)), 9 the name `file` of a concern, 8 the
main row of a file of several concerns, 7 the concerns, 6 a deletion as real
work, 5 the generated files). `units` (since `"schema": 4`) cuts the diff of the review
into the changes a reviewer reads once, in that order ([Review Units](../../spec/branch/review-units.spec.md)): a
change of the form of a declaration (`shape` with `changes`), of its body alone (`behaviour`), a new or removed
declaration (`added`, `removed`), a new or a deleted file (`newFile`, `deletedFile`; `deletedFiles` with its
`directory` for 3 or more deleted from one directory), one unit per kind of mechanical change and per repeated edit
over all of their files (`mechanical`, `repeatedEdit`, last, the generated files last of all), and a change of no declaration (`other` with its
`reason`); each with its `slices`: the file, its `blocks` by `index` with their `old` and `new` lines from 1, and a
`fingerprint` of the change that a rebase or an amend keeps. `concerns` (since `"schema": 7`) joins the units into the
tasks of the range by evidence ([Review Concerns](../../spec/branch/review-concerns.spec.md)): each with its `name` by
its `source` (`token`, `replacement`, `declaration`, `file`, `directory`), its `parts` (`unit`, `path`, `blocks`), its
`paths`, its `elsewhere` (since `"schema": 8`: its files whose main row is in another concern, the one whose parts
change the most of their lines) and its `evidence` (`uses`, `group`, `token`, `rest`), what no evidence joined last
(`rest`).

```json
{"schema":5,"repository":"/repo","branch":"jane/virtual/auth","base":"1a2b3c4d","head":"7a8b9c0d",
  "interdiff":{"reviewed_base":"1a2b3c4d","reviewed_tip":"5e6f7a8b",
    "commits":[{"id":"7a8b9c0d","reviewed_id":null,"subject":"Add logout","state":"added"}],
    "scope":{"base":"5e6f7a8b","head":"7a8b9c0d"}},
  "api":[{"path":"src/a/Session.kt","line":3,"name":"logout","kind":"changed","changes":["parameters"],
    "before":"fun logout() : Unit","after":"fun logout(all:Boolean) : Unit",
    "usages":{"inside":1,"outside":12,"outside_sites":[{"path":"src/c/Menu.kt","line":40},…],"complete":true,"updated":false}}],
  "files":[{"path":"src/a/Session.kt","old_path":null,"mechanical":null,"generated":null,"uses":0,"used_by":1,
    "new_links":0,"breaks":12,"breaks_complete":true,"untested":true,"repeated_edit":null,"repeated_edit_only":false},
    {"path":"src/b/Logout.kt","old_path":null,"mechanical":null,"generated":null,"uses":1,"used_by":0,"new_links":1,
    "breaks":0,"breaks_complete":true,"untested":true,"repeated_edit":null,"repeated_edit_only":false}],
  "links":[{"from":"src/b/Logout.kt","to":"src/a/Session.kt","symbols":[{"name":"logout","line":5}],"state":"added",
    "cross_module":false,"new_between_modules":false}],
  "dependencies":[{"manifest":"src/b/intellij.b.iml","module":"intellij.b","kind":"module","name":"intellij.a",
    "change":"added","version_before":null,"version_after":null,"scope":"main","scope_before":null}],
  "repeated_edits":[],
  "units":[{"id":"declaration:src/a/Session.kt:a.logout:all:Boolean","kind":"shape","changes":["parameters"],
    "name":"logout","key":"a.logout","signature":"all:Boolean","public":true,"line":3,"slices":[{"path":"src/a/Session.kt",
    "old_path":null,"whole":false,"blocks":[{"index":0,"old":[3,3],"new":[3,3]}],"fingerprint":"5f0c…"}],
    "fingerprint":"5f0c…"},
    {"id":"new:src/b/Logout.kt","kind":"newFile","name":"Logout.kt","key":null,"signature":null,"public":false,
    "line":null,"slices":[{"path":"src/b/Logout.kt","old_path":null,"whole":true,"blocks":[],"fingerprint":"a1d9…"}],
    "fingerprint":"a1d9…"}],
  "order":["src/a/Session.kt","src/b/Logout.kt"],"index_note":null}
```

- `unknown_branch` (exit 2): no branch has the name.
- `invalid_arguments` (exit 2): `--base` names no branch and no commit or shares no history with the branch; a rev of
  the range names no commit; `--since-reviewed` or `--base` of a range.
- `no_baseline` (exit 4): no baseline, or one that shares no history with the branch.

### `bd unapply <branch> [--expect-head <id>]`

Takes the commits of a branch and their files out of the checked out branch and keeps the branch as
`{prefix}/unapplied/<name>`, which no sync archives or deletes (`sync_core::unapply_branch`, "Drop and unapply" in
[Git Operations](../design/git-operations.md), the contract [Unapply and Reapply](../../spec/branch/unapply.spec.md)): only
when the user asks. Computed before the repository lock, which holds only its witness and the move; the files follow
through the guarded update of Update Project, so a local change on a path it writes is `uncommitted_overlap` (exit 4,
`paths`) and changes nothing.

```json
{"schema":2,"repository":"/repo","prefix":"jane","branch":"docs","unapplied_branch":"jane/unapplied/docs",
  "commits":["1a2b3c4d","5e6f7a8b"],"previous_head":"9c0d1e2f","new_head":"3a4b5c6d",
  "undo_token":"unapply:<full previous>..<full new>:<full tip>:jane/virtual/docs:jane/unapplied/docs"}
```

- `unknown_branch` (exit 2): no commit groups into the branch.
- `branch_not_built` (exit 4): the virtual branch is not the copies of its commits now (a conflict, a commit no sync
  copied yet): a commit would leave HEAD without a copy. `bd sync` says why.
- `branch_checked_out` (exit 4): a worktree has the virtual branch checked out.
- `drop_conflict` (exit 4): a later commit depends on the changes of a commit of the branch, or the range holds a merge.
- `bd undo <undo_token>` brings the commits, their files and the virtual branch back (`undo_stale` once the branch
  moved on); a repeated undo exits 0 with `already_undone: true`.
- `unapplied` of `bd status` lists the unapplied branches (`name`, `branch`, `tip`, `subject`, `remote.head` and
  `remote.up_to_date` once pushed). They land nothing: `bd push <name>` and `bd safe-push <name>` refuse one as
  `unapplied` (exit 4); `bd push --unapplied <name>` syncs and pushes it to `origin` under its own name, leased on the
  remote head the sync saw or `--expect-remote`, with no pull request draft (`pushed_over` over a remote that moved
  without commits it lacks, `lease_rejected` when the remote moved otherwise); a remote branch with commits it lacks
  is `push_refused` until `--force`, as the app's push of it asks first.

### `bd reapply <name> [--dry-run] [--expect-head <id>]`

Puts the commits of the unapplied branch `{prefix}/unapplied/<name>` back on top of the checked out branch, their files
following, and deletes it (`sync_core::reapply_branch`, "Reapply" in [Git Operations](../design/git-operations.md)): only
when the user asks. A commit whose subject groups elsewhere gets its `(<name>)` prefix back (`prefix_restored`); one
HEAD has is left out (`present`).

```json
{"schema":2,"repository":"/repo","prefix":"jane","branch":"docs","unapplied_branch":"jane/unapplied/docs","dry_run":false,
  "reapplied":[{"commit":"1a2b3c4d","new_id":"5e6f7a8b","subject":"(docs) Document the API"}],"prefix_restored":1,
  "previous_head":"9c0d1e2f","new_head":"5e6f7a8b","undo_token":"reapply:<full previous>..<full new>:<full tip>:jane/unapplied/docs"}
```

- `reapply_conflict` (exit 4): a commit conflicts with HEAD (`commits`, `paths`); nothing was changed.
- `nothing_to_reapply` (exit 4): HEAD has every commit of it, or it has no common history with the baseline.
- `uncommitted_overlap` (exit 4, `paths`), `branch_checked_out` (exit 4), `unknown_branch` (exit 2).
- `bd undo <undo_token>` takes the commits out again with their files and creates the unapplied branch again
  (`restored_branch`).

### `bd reword <commit> (--subject <s> | --message <m> | --message-file <file>)`, `bd reword --plan <file>`

A new message for commits ahead of the baseline, as a regroup (`history_rewrite::reword`), validated and written like
`bd group`, with the same options (`--expect-head`, `--allow-append`, `--dry-run`, `--sync`, `--map`) and output, its
`operation` `reword`:

```json
{"schema":2,"operation":"reword","dry_run":false,"previous_head":"8df5c602","new_head":"26245b67","rewritten":1,
 "undo_token":"8df5c602…..26245b67…","groups":{"feat":["fc2a8ace","26245b67"],"other":["57c7fc85"]},"unassigned":[],
 "changes":[{"commit":"2ebfe814","new_id":"57c7fc85","old_subject":"(feat) Commit 2 with a very long subject",
             "new_subject":"(other) Commit 2","moved":{"from":"feat","to":"other"}}]}
```

- `--subject` replaces the subject line, the `(prefix)` included, and keeps the body; `--message` and
  `--message-file` (`-` for stdin) the whole message. A message is cleaned up as `git commit` does (trailing
  whitespace, blank lines at the ends); an empty subject, a subject of several lines and a commit named twice are
  `invalid_plan`.
- `--plan` rewords several commits in one rewrite: `{"rewrites": [{"commit": "<id>", "subject": "…"}, {"commit":
  "<id>", "message": "…"}]}`, each entry with exactly one of the two.
- The commit is an id that `bd status` printed (abbreviated or full); a revision such as `HEAD~1` is
  `invalid_arguments`, a commit on the baseline or unknown `not_ahead_of_baseline`.
- Every tree stays, so the index and the working tree do; the older commits keep their ids, the later ones are
  recreated with their author and committer, and their mapping notes follow them. The note of a commit whose text
  changed is dropped: the next sync copies it anew, with the new message, so the branch needs a push
  (`remote.reworded`, and `remote.restacked` on the branches stacked on it).
- A new `(prefix)` moves the commit to that branch, its fixups with it (`changes[].moved`). A fixup that would fold
  into another target or none, a marker added or removed, a target a fixup names by its id, and a new text of a
  commit whose `amend!` waits are `invalid_plan`: fold first, or reword the fixups in the same plan.
- `changes` is printed also without `--dry-run`, each with `new_id`, `body_changed` when the body changed, and
  `moved`. A message that equals the old one after the cleanup rewrites nothing and prints no `undo_token`.
- From a lease: `bd -C <shared checkout> reword <commit> --expect-head <head> --subject '…'` fixes a session's own
  landed commit; the lease needs no move (`bd help worktrees`).

### `bd undo <undo_token>`

Moves the branch back from the token's new HEAD to its previous HEAD, and the issue groups and the stack parents back
from the token's new commit to its previous one (or away), in one transaction with CAS on each, under the lock:
`undo_stale` (exit 4) once anything else changed the branch or a store, since the token can never succeed again. Give the token as it was printed: a token with abbreviated ids, or whose store commit is not a
write on its previous one, is `invalid_arguments`. Prints
`{"schema":2,…,"previous_head":"<undone>","new_head":"<restored>","undone_issue_groups":"<undone>","undone_stack_parents":"<undone>"}`;
`previous_head` and each `undone…` are left out for a token without that part.

An `update:<previous>..<new>` token of `bd update` moves the branch back and the files with it (see `bd update`); a
`pick:<previous>..<new>[+follow:<worktree>:<previous>..<new>]` token of `bd pick` moves the branch and the worktree
that followed back (see `bd pick`); an `unapply:<previous>..<new>:<tip>:<branch>:<unapplied branch>` token of
`bd unapply` moves the branch back with the files and the unapplied branch back to the virtual one (`restored_branch`);
a `reapply:<previous>..<new>:<tip>:<unapplied branch>` token of `bd reapply` takes its commits out again and creates the
unapplied branch again.

### `bd update [--no-fetch] [--dry-run] [--dir <path>]`

The app's Update Project (`update_core::update_project::update_project_with`, "Update project" in
[Git Operations](../design/git-operations.md)): it fetches the baseline's remote branch, then takes the repository
lock, checks again and applies a clean update. The fetch runs before the lock, so a slow fetch blocks no other writer;
beside another fetch of the baseline (the app's, another agent's) it waits for that one and takes its result.

```json
{"schema":2,…,"branch":"master","upstream":"origin/master","state":"rebase","dry_run":false,"fetched_at":1760500000,
 "previous_head":"77e01aa4","new_head":"c0ffee12","upstream_head":"5f6a7b8c","replayed":2,"dropped_count":1,
 "dropped":[{"commit":"1b2c3d4e","subject":"(auth) Add login","upstream":"9d0e1f2a","proof":"patch_id"}],
 "undo_token":"update:<full previous>..<full new>",
 "landed":[{"branch":"auth","commits":[{"commit":"1b2c3d4e","subject":"Add login","upstream":"9d0e1f2a"}],"remaining":0}]}
```

- `state` is `up_to_date`, `fast_forward` or `rebase`; `replayed` and `undo_token` come only with a change. `dropped`
  names each local commit the update dropped, with its `proof` (`patch_id`, `replayed`, `changed_lines`, `empty`; in a
  resolve session also `landed` and `user`), and `dropped_count` counts them. `landed` lists the local commits the
  baseline has, by branch. `verify.paths` names the files to build and test after a resolution.
- An output or an error with `ask_user: true` waits for the user's decision: `reasons` of the output, `message` of the
  error. bd asks only for a decision of the user, and an agent asks only then.
- `resolved` lists the local commits whose conflict of git the update resolved by words (`branchdeck.resolveByWords`,
  as in `bd sync`): `commit`, `new_id` (not for a dry run), `conflicts`, `file_count` and `files`. An update whose
  every conflict resolves so is a `rebase` without a session.
- A conflict starts a resolve session (below): `state` `conflict`, exit 1. With `--dry-run`, nothing starts: `conflict`
  names the first conflict: its `files` (the first 50; `files_omitted` counts the rest), the commit in `commits` and
  the `upstream_commits` that change the files (`commit`, `subject`, `author`, `files`).
- `uncommitted_overlap` (exit 4): uncommitted, staged or untracked files sit on paths the update writes; `paths`
  lists them, the first 100, and the message says how many there are beyond. bd never stashes, resets or commits
  them: the agent asks the user, then runs `bd update` again.
- `--no-fetch` updates onto the last fetched baseline. `--dry-run` implies it, takes no lock and prints what the
  update would do; it writes at most git objects (the replay).
- A failed fetch is `network_error` (exit 5), a held lock `locked` (exit 3), and `operation_in_progress`,
  `detached_head` and `no_baseline` (a local baseline) refuse (exit 4).
- `bd undo <undo_token>` takes the update back: the branch moves back, and the files follow through the same
  guarded update with the trees swapped, under the lock. Only the last update of the branch can be undone (its
  reflog entry proves it; the update writes it with `--create-reflog`), with full ids. Local changes on a path it
  writes are `uncommitted_overlap`; a branch moved, or a HEAD switched, since is `undo_stale` (exit 4: a retry can't
  succeed). The landed report is the one from before the update again (`landed_restored`). A repeated undo exits 0
  with `already_undone: true` and changes nothing.
- The move updates `HEAD` and commits only while git holds its lock, after a check that HEAD is still on the checked
  branch; a `git switch` meanwhile is `head_moved` with nothing changed (see "Ref moves" in
  [Git Operations](../design/git-operations.md)). A lock file of git that is in the way (`HEAD.lock`, or one that a killed
  git left) is `locked` with its path.
- A `bd update` or `bd undo` killed after the move and before the files followed leaves the mark
  `refs/branch-deck/update-pending/<worktree id>`. The next `bd update`, `bd undo` or Update Project lets the files
  follow first; `bd update --status` shows it as `pending_update`. When the branch moved since, `finish_pending`
  (exit 4) names the command that removes the mark.

### The resolve session of `bd update`

`bd help update` prints this workflow for an agent; the contract is [bd update](../../spec/cli/update.spec.md), the
design "Resolve session" in [Git Operations](../design/git-operations.md). The session replays the local commits
outside the working tree (`update_core::resolve_session`, `git_ops::update_resolve`) and opens one round per
conflicting commit. It belongs to the worktree and the branch, keeps the baseline it started with, and lives in the
ref `refs/branch-deck/resolve/<worktree id>`. Peers' commits on top of HEAD join it as more rounds. `bd update` with a
session open prints the open round again, fetches nothing and writes the missing copies.

```json
{"schema":2,…,"state":"conflict","session":{"worktree":"main","state":"resolving","round":1,"replayed":0,"total":1,
 "dir":"/repo/.git/branch-deck/resolve",
 "commit":{"commit":"d88f5726","subject":"(auth) Change line 2","author":"Me","patch":"/repo/.git/branch-deck/resolve/r1/commit.patch"},
 "files":[{"path":"shared.txt","kind":"content","edit":"/repo/.git/branch-deck/resolve/r1/edit/1-shared.txt",
   "before":"…/r1/before/1-shared.txt","onto":"…/r1/onto/1-shared.txt","commit":"…/r1/commit/1-shared.txt",
   "markers":1,"hint":"…","upstream_commits":[{"commit":"c9631487","subject":"Change line 2 upstream","author":"Dana Lee"}]}],
 "blocked_paths":[],"marker_labels":{"onto":"c9631487…","before":"d88f5726…^","commit":"d88f5726…"},
 "next":"/Users/me/.local/bin/bd -C /repo update --continue --round 1"}}
```

What an agent does with a round, each refusal and the finish is `bd help update` (`UPDATE_HELP`, the runtime text);
the contract is the spec. The mechanics behind it:

- bd finds a block by the labels it wrote after the markers (`marker_labels`, the full ids, followed by `:<path>` for a
  renamed file), at any marker length (`conflict-marker-size`); a `=======` outside the blocks, e.g. under a Markdown
  heading, is text.
- A round prints its own files only: the rounds resolved so far are in the `resolutions` of the finish and of
  `bd update --status`, not repeated in every round, so a long session stays as small as its open round.
- A round that resolves by words resolves itself (see "Resolve by words" in [Git Operations](../design/git-operations.md)):
  it counts in `round`, and `session.resolved` of every round lists it, so the number of the open round is never a
  surprise. Its resolution has `words`, and its files `how: "words"`.
- A refusal names the round and its commit and changes nothing: the round stays open, and the copies keep the edits;
  `edited` per file says whether a copy differs from bd's merge result.
- A round whose commit has a twin in the baseline (a candidate: the same author, author time and subject, but no
  proof) has `session.commit.landed_as` with the interdiff of the two (`local_only`, `upstream_only`), and its `next`
  ends with `--landed <commit>`. A drop that loses nothing (`local_only` empty) asks nobody; one that loses lines waits
  for `--user`. The first output lists every such twin in `session.landed_differs`, so the user answers once.
- The first output of a session lists `dropped`; a round of `--continue` prints `dropped_count` only, so the list of
  a long session is printed once.
- The copies live in `branch-deck/resolve/r<n>/{edit,before,onto,commit}/<key>-<basename>` of the worktree's git dir,
  or under `--dir <path>` (absolute, outside every worktree, empty or bd's for this worktree, used by no other
  session), for a sandbox that keeps `.git` read only. On Windows the paths have backslashes, and the `\\?\` form
  from 260 characters on.

### `bd pick (--from <dir> [--follow] | <rev> | <a>..<b>) [--expect-head <id>] [--dry-run]`

Lands the commits of another worktree on the branch checked out at `-C`, without `git cherry-pick`
(`update_core::pick::pick_core`, "Pick" in [Git Operations](../design/git-operations.md), the contract
[bd pick](../../spec/cli/pick.spec.md)). Agent sessions that share a checkout commit in worktrees of their own (a
`bd lease` worktree, detached HEAD) and land each commit on the shared branch this way: the commits are replayed in memory
with `git replay`, and under the repository lock, which every worktree shares, the branch moves and the files follow
through the guarded update. `--follow` moves the source worktree onto the new tip the same way. Inside a lease of the
pool, `bd lease pick` is this pick, run in the checkout the lease was cut from (below).

```json
{"schema":2,…,"branch":"master","state":"picked","dry_run":false,"source_worktree":"/Users/me/.branch-deck/pool/repo-1a2b3c4d/3/repo","base":"5f6a7b8c",
 "previous_head":"77e01aa4","new_head":"c0ffee12",
 "picked":[{"commit":"1b2c3d4e","new_id":"9d0e1f2a","subject":"(lane) Add the probe","group":"lane","grouped_by":"prefix"}],
 "already_present":[{"commit":"2c3d4e5f","subject":"(lane) Fix the build","on":"aa11bb22","by":"patch_id"}],
 "followed":{"worktree":"/Users/me/.branch-deck/pool/repo-1a2b3c4d/3/repo","previous_head":"3d4e5f60","new_head":"c0ffee12","state":"moved"},
 "undo_token":"pick:<full previous>..<full new>+follow:3:<full previous>..<full new>"}
```

- The range of `--from` is what the worktree's HEAD has that the branch lacks: `<source HEAD> ^<branch> ^<upstream>
  ^<base>`, the base being the fork point of the source in the branch's reflog. It sees through a fold, a regroup or
  new dates of the branch after the worktree was cut from it. Without a fork point, a local shared history is
  `range_unknown` (exit 4): name the commits, `bd pick <first>^..<last>`. A worktree on a newer baseline than the
  branch is `source_on_newer_baseline` (exit 4): the owner of the branch updates it first.
- `state` is `picked`, `up_to_date` (the branch has every commit) or `conflict` (exit 1, nothing changed: `conflict`
  names the first commit that does not apply, the `clean` ones before it, its `files` and `next`, what to do: move the
  worktree onto the branch, redo the commit there, regenerate a generated file, never keep one side of a code hunk).
- `already_present`: the commits whose change the branch has, with `on`, its commit of the same patch id (`null`: the
  change is there in another form), and `by`: `patch_id`, a commit of the branch above the base has its patch id and
  it is left out before the replay (the commits after it replay onto the copy before it), or `empty`, the replay
  dropped it; never an empty commit, never a `CHERRY_PICK_HEAD`. A commit that reached the upstream in another form is
  not found by patch id (the upstream is outside the range): it drops as `empty` or conflicts. With `conflict`, only
  the `patch_id` ones are listed. Never resolve a conflict with `git cherry-pick` in the shared checkout: it leaves
  `CHERRY_PICK_HEAD`, which blocks every rewrite. `--dry-run` takes no lock and
  writes at most git objects: it answers "does the branch hold my commits?" by content, never by date.
- `picked[].group` is the branch Branch Deck groups the copy into, `grouped_by` `prefix`, `issue_group`, `issue` or
  `none`: a subject without a prefix or an issue key lands unassigned. Write the `(branch)` prefix when you commit.
- `dependencies` (also with `--dry-run`, so the prefix can be fixed in the worktree before the pick): a picked commit
  whose prefix names a branch the branch has already (`prefix_branch`), which is not built on every branch whose
  commits its hunks depend on (`depends_on`; the edges by lines of `bd commits`, without symbols), so that branch
  can't sync. `plan` is the `bd group` plan that moves the commit into the branch of the stack that holds all of them
  (merge the `branches` of several plans into one `bd group`: the rewrite gives the commits new ids); `restack`, for a
  commit that needs its own branch too, is the `bd restack <prefix_branch> <parent>` onto a branch that holds them and
  its parent; neither when no branch holds them all. The pick never applies it.

  ```json
  "dependencies":[{"commit":"9d0e1f2a","subject":"(base/teamcity) Watch the monitor","prefix_branch":"teamcity",
   "depends_on":["hang"],"plan":{"branches":{"hang":["9d0e1f2a"]},"unassign":[]}}]
  ```
- `reparented` (`{"leaf": "parent"}`): a `(parent/leaf)` prefix of a picked commit stacked a branch the branch had
  already, a root or on another parent before: the whole branch is built on `parent` now (a bare `(leaf)` declares
  no parent). Two parents for one branch are conflicting parents, which name the commits of each and the
  `bd restack <leaf> <parent>` that keeps one, or the `bd unstack <leaf>` that keeps none.
- `followed.state`: `moved`, `at_tip` (nothing to move), `would_move` (`--dry-run`), or, after the branch moved,
  `head_moved` or `blocked` with a `message`: the same `bd pick` again finds every commit present and moves only the
  worktree. A detached HEAD stays detached (`option no-deref`); a rebase, merge or cherry-pick in progress in the
  worktree refuses `--follow` (`operation_in_progress`).
- Local changes on a path the pick or the follow writes are `uncommitted_overlap` (exit 4, the message names the
  worktree); both are checked before either moves. A branch that moved since the check is `head_moved` (exit 3).
- `bd undo <undo_token>` moves the branch and the worktree that followed back, each with its files, after checking
  both (`undo_stale` once one moved on); `follow_undone` says whether the worktree moved back. A repeated undo exits 0
  with `already_undone: true`.
- A pick or a follow killed between the move and its files leaves the mark `refs/branch-deck/update-pending/<worktree
  id>`, as an update does; the next pick, update or undo lets the files follow first.

### `bd lease get [--holder <label>] [-b <branch>] [--min-free <size>]`, `pick`, `return`, `status`, `adopt`

The pool of worktrees that agent sessions work in, one session per slot (`lease_pool::lease`, "Lease" in
[Git Operations](../design/git-operations.md), the contract [bd lease](../../spec/cli/lease.spec.md)). A lease needs no
branch prefix. The state is `$GIT_COMMON_DIR/branch-deck/leases.json`, under the pool lock `leases.lock`.

```json
{"schema":2,"repository":"/Users/me/idea-1","pool":"/Users/me/.branch-deck/pool/idea-1-3f2a9c01",
 "path":"/Users/me/.branch-deck/pool/idea-1-3f2a9c01/2/idea-1","lease_id":"<uuid>","holder":"lane-1","slot":"idea-12",
 "source_head":"<full id>","home":{"checkout":"/Users/me/idea-1","branch":"master"},
 "next":["bd -C /Users/me/.branch-deck/pool/idea-1-3f2a9c01/2/idea-1 lease pick","bd lease return --lease-id <uuid>"],
 "leased_at":1792411982,"reused":true,"seeded":[".claude/settings.local.json"],
 "skipped":[{"slot":"idea-1","path":"…/1/idea-1","reason":"dirty","paths":["left.txt"]}],"processes_checked":true}
```

- `bd lease get` first checks the free space of the volume of the pool against `--min-free <size>`, else
  `git config branchdeck.leaseMinFree <size>` (`200G`, binary units as `df -h`; `0` or no setting checks nothing):
  less is `low_disk_space` (exit 4), the message naming both sizes, the free slots and the leases whose holder
  process is gone. It leases a free slot at the exact HEAD of `-C`, detached (`-b <branch>` creates the branch
  there): one `status` of the slot, then `checkout --force`, only the diff written (`reused: true`). A free slot with
  local changes is skipped and marked dirty, one with a process of another session in it is skipped as busy
  (`skipped`, `processes`); the Bazel servers of a free slot are stopped with SIGTERM (`bazel shutdown` on Windows;
  `stopped_servers`). No free slot: an idle lease, its holder process gone for an hour, is taken over when every commit its home branch lacks is
  proven there (the landed rule of the return) and its tree is clean (`took_over`: `lease_id`, `holder`, `branch`,
  which stays, `landed` with the proof of each commit); one with work is skipped (`reason: "idle_with_work"`, `idle`
  with `commits`, `paths`, `operation`) and never touched, the user decides. Else a new one, `<pool>/<n>/<repository name>` (`reused: false`): on macOS an APFS clone of a free slot or of the
  source without what the `delete` clean rules delete, then the diff checked out (`created_by: "clone"`,
  `cloned_from`), else a full checkout (`created_by: "checkout"`); with a new slot, `idle_leases` names the leases
  whose holder process is gone, each with its `lease_id` and `idle` (`idle` or `idle_with_work`). The pool is `~/.branch-deck/pool/<name>-<hash>`, or
  `git config branchdeck.leasePool <dir>`. The ignored files `.worktreeinclude` names are copied in (`seeded`).
  `free_bytes` is the free space before the get.
- A pool adds one slot at a time (`clones.lock` beside the state): a get that must add a slot while another adds one
  waits, and a slot a return frees meanwhile ends the wait and is leased instead (`reused: true`), within about a
  second, while the other clone still runs; a get whose turn comes while a slot is being returned or cleaned waits for
  that slot rather than clone one; the free space is checked again once its turn comes. `waited_for_clone_ms` says how long it waited (omitted when it waited for none). The wait lasts
  while the holder lives; a killed holder frees the lock. Each step goes to stderr as it starts, the wait with its
  holder:

  ```
  bd lease get: reserving a free slot
  bd lease get: no free slot; waiting for `bd lease get --holder lane-3` (pid 4242, for 35 s), which adds /Users/me/.branch-deck/pool/idea-1-3f2a9c01/8/idea-1 for lane-3: the pool adds one slot at a time, and a slot freed meanwhile is leased instead
  bd lease get: reserving a slot freed meanwhile, else a new one
  bd lease get: cloning /Users/me/.branch-deck/pool/idea-1-3f2a9c01/2/idea-1 into /Users/me/.branch-deck/pool/idea-1-3f2a9c01/9/idea-1
  bd lease get: refreshing the index of /Users/me/.branch-deck/pool/idea-1-3f2a9c01/9/idea-1
  bd lease get: checking out 3f2a9c01 in /Users/me/.branch-deck/pool/idea-1-3f2a9c01/9/idea-1
  bd lease get: looking for local changes in /Users/me/.branch-deck/pool/idea-1-3f2a9c01/9/idea-1
  ```
- `home` names where the commits of the lease land: `checkout`, the source of the get, and `branch`, the branch it
  had checked out (`null` for a detached HEAD: nothing to pick onto, and `next` is only the return). `next` is what the
  session runs, in order: `bd -C <path> lease pick` after each commit that builds and passes its tests, and the
  return once the task is done.
- `bd lease pick [--dry-run]` inside the lease is `bd pick --from <lease> --follow` in `home.checkout`, onto
  `home.branch`, and prints what `bd pick` prints: the commits replayed onto the tip as it is then, whatever other
  sessions landed meanwhile, and the lease moved onto the new tip; with nothing to land the lease only follows
  (`up_to_date`, `followed.state: moved`). A conflict lands nothing (exit 1) and comes back to the lease: `update` is
  the update of the lease onto the tip of `home.branch` without a fetch, as `bd update` prints it, and its conflict
  opens a resolve session in the lease (`bd help update`); the lease moves when `bd update --continue` finishes it,
  whose `next` is `bd lease pick` again, and `bd update --abort` closes it; while it is open `bd lease pick` is
  `resolve_in_progress` (exit 4). The output is `lease_pick`.
- In a lease `bd update` onto the baseline is `lease_follows_home` (exit 4): the lease follows its home branch, and the
  owner of the shared checkout updates that; `--dry-run` and the steps of a session go on. Refused with nothing changed: `home_branch_switched` (exit 4; another branch is checked out there now:
  ask the user), `no_home_branch` (exit 4; cut from a detached HEAD), `no_lease` (exit 2; not inside a lease), and the
  refusals of `bd pick`.
- `bd lease return [--workspace <dir>] [--lease-id <id>] [--confirm-preserved]` frees the slot of `--workspace`, `-C`
  or the current directory, or, with `--lease-id` and no `--workspace`, the slot of that lease from any checkout of the
  repository: the seeded paths go, a branch is detached (`detached_branch`; it stays), the slot stays at
  its commit. Refused with nothing changed: `no_lease`, `lease_mismatch`, `resolve_in_progress` (a resolve session of the slot is
  open: finish it with `bd update --continue` or close it with `--abort`), `slot_busy` (a process of another session
  in it: `processes` [{`pid`, `command`, `argv`, `kind`: `bazel_server` or `other`, `workspace`, `stop_error`}]), `unlanded_commits`
  (`commits` that no branch, tag or remote branch holds and that no proof finds on the branch of the source: a dry run
  of the pick onto it, a patch id, the identity and the twin of `bd update`, the mapping note of a sync; land them with
  `bd -C <dir> lease pick`),
  `dirty_worktree` (`paths`). Not counted: the session of bd (bd, its ancestors and the processes of their process
  groups, such as the `jq` of a pipeline), git's background maintenance (`git maintenance`, `git gc --auto` and the
  gits they start: `maintenance` names them; a detach of HEAD waits up to 10 s for a `HEAD.lock` they hold;
  with `git config branchdeck.leaseMaintenance true` the return starts `git maintenance run --auto --detach` once the
  slot is free, `maintenance_started`, for a repository whose `maintenance.auto` is false) and
  `caffeinate`. A Bazel server of the worktree (`--workspace_directory` in it) is asked to exit with SIGTERM once no
  other process is there, up to 30 s, and named in `stopped_servers`; one that does not exit is `slot_busy`. On
  Windows, which has no process scan, the server is found by the pid file of the output base its `bazel-out` link
  leads to, and asked with `bazel shutdown` (`bazel` of the `PATH` with the extensions of `PATHEXT`); one whose
  `bazel shutdown` can't run or fails is `slot_busy` at once, its `stop_error` naming the program and the error.
  `landed_check` says which check found the work landed: `reachability` or `content`
  (`landed_branch`), and `landed` names each commit the content check proved, with its `proof` (`picked`, `patch_id`,
  `replayed`, `changed_lines`, `twin`, `twin_removed_before` (the twin lacks only removals of lines its parent no
  longer had), `subject_twin` (a commit of the same author and subject at another author time with the same lines),
  `note` with its `virtual_commit`). `--confirm-preserved` gives the commits up
  (`given_up`) and discards the changes (`discarded`) with `reset --hard` and `clean -fd`. A repeat joins: a return
  retried while the first still runs waits for it, and a return that another return of the lease overtook changes
  nothing; both answer `joined: true`, with empty lists and no `landed_check` when they checked nothing.
- `bd lease status [--processes]` lists the slots from the state alone: `state` `free`, `preparing`, `ready`,
  `templating` (a get clones it into a new slot), `returning` (its return runs), `cleaning` (`bd lease clean` cleans
  it) or `abandoned` (the get, the return or the clean of it died), `dirty`, `missing`, `lease` (`holder_pid`,
  `holder_alive`: whether the process the get ran for still runs; `idle` of a ready lease whose holder is gone:
  `idle`, or `idle_with_work` with the `commits`, `paths` or `operation` a check found; `taking_over` while a get
  checks one), `reclaimed` (the lease a takeover or a reclaim took from its dead holder), `free_bytes`,
  `trash_bytes`; no `git status`. `--processes` adds the processes in each slot.
- `bd lease reclaim --idle [--dry-run] [--keep <n>] [--age <duration>]` returns every idle lease of the pool whose
  work landed, as a takeover would (`reclaimed`, with `landed`, `stopped_servers`, `cleaned`), keeps the ones with
  work (`kept`, as `idle_with_work`) and names the rest (`skipped`: `not_quiet`, `busy`, `changed`, `failed`);
  `--dry-run` writes nothing. Output `lease_reclaim`.
- In a slot whose lease was taken over, a write of another session than the one that holds it now (a resumed
  session of the old holder) and its `bd workspace` are `lease_reclaimed` (exit 4): the message names both leases, the
  home branch and the old branch, which stays; `next_steps` is `bd lease get --holder <old holder> -C <home>`.
- `bd lease adopt [--move] <dir>...` adds clean linked worktrees of this repository as free slots where they are
  (`adopted`), and lists the refused ones with their code (`refused`: `not_a_slot`, `slot_in_pool`, `slot_busy`,
  `dirty_worktree`; exit 4). `--move` moves each into the pool directory with `git worktree move` (`moved_from`),
  only on the same volume (`other_volume`), and removes its old parent directory when it is left empty. A moved slot
  has a new path, so Bazel gives it a new, cold output base.
- The return applies the clean rules of `.branchdeck.toml` (`[lease] clean`: `cargo`, the stale units of a target
  dir; `delete`, a path or a link, with `except` the entries of a directory but the named ones) of the source
  checkout before it frees the slot: a `delete` moves its paths into the trash (`$GIT_COMMON_DIR/branch-deck/trash`),
  but deletes a link (a junction of Windows too) in place as the link, a `cargo` rule each stale unit as a whole, and
  they are deleted and counted once the slot is free: `cleaned`
  [{`rule`, `freed_bytes`, `ms`, `error`}], `trash_left`. What a process keeps (on Windows: works in it, has a file of
  it open) stays whole, and the rule's `error` names the path and says that a process holds it; the next clean deletes
  it once it is free. A rule with an unsafe path (out of the slot, `.git`, tracked, a symlink on its way, a `delete` of
  a cargo rule's dir) is skipped with its `error`; `clean_error` when the file does not parse, the return succeeds
  anyway; `--no-clean` applies none. A get leaves out of a cloned slot what the `delete` rules delete.
  `bd lease clean [--free] [--dry-run] [<dir>...]` applies the rules now to the free slots, the given worktrees, or
  the checkout of `-C`; it deletes by default, `--dry-run` only reports (`dry_run`, `freed_bytes`); `freed_bytes`
  counts blocks an APFS clone shares too, `volume_freed_bytes` is what the volume gained; a slot with a
  lease or a process is `skipped`; a file that does not parse is `invalid_arguments`. On both commands `--keep <n>`
  and `--age <duration>` set `keep` and `age` of every cargo rule for that call.
- `processes_checked: false`: the platform has no process scan (Windows), and nothing was refused for a process.

### `bd fold [--branch <b>] [--expect-head <id>] [--dry-run]`

Folds the `fixup!`, `squash!` and `amend!` commits into their targets, as the app's Squash Fixups does
(`history_rewrite::autosquash`), under the repository lock. One rewrite that keeps HEAD's tree, so the index and the working
tree stay as they are.

```json
{"schema":2,…,"dry_run":false,"previous_head":"77e01aa4","new_head":"c0ffee12",
 "folded":[{"commit":"1b2c3d4e","subject":"(auth) Add login","new_id":"2c3d4e5f","fixups":["9d0e1f2a"]}],
 "orphans":["5f6a7b8c"],"undo_token":"<full previous>..<full new>"}
```

- `--branch` folds only the fixups whose target is in that branch; a name that is no branch is `unknown_branch`.
  `orphans` are fixups without an older target: they stay. Nothing to fold prints `folded: []` without a token.
- Without `--expect-head` it folds the fixups of HEAD as the squash finds it, also after another process moved HEAD
  since bd read the plan (a rewrite of the app, a commit): `folded`, `orphans` and `previous_head` are what the squash
  folded there, the commit of the other process kept; a HEAD without fixups there is nothing to fold.
- `--expect-head` refuses with `head_moved` unless HEAD is still that commit; the fold also checks that the planned
  commits and the history it rewrites end at the planned head, and moves the branch only from it (compare-and-swap).
- `bd undo <undo_token>` restores the previous head.
- A fixup that conflicts with a commit between it and its target or after it is `squash_conflict` (exit 4, `commits`,
  `paths`), one that leaves its target or a later commit empty `squash_empty`; nothing is folded.

### `bd squash <into> <commit>… [--subject <s> | --message <m> | --message-file <f>] [--expect-head <id>] [--dry-run | --sync]`

Squashes the commits the user asked to squash into `<into>`, the oldest of them, as the app's Squash does: the same
core (`history_rewrite::squash::squash_commits_core`) and the same intent (`RewriteIntent::Squash`), recorded before
any lock. `select_squash` finds the commits among the commits ahead of the baseline (one `log`, the grouping stores) and
checks the selection; the squash reads its history again, computes before the repository lock and holds it for the
witness of HEAD and the move only. One rewrite that keeps HEAD's tree, so the index and the working tree stay as they
are.

```json
{"schema":2,…,"dry_run":false,"previous_head":"77e01aa4","new_head":"c0ffee12","into":"1b2c3d4e",
 "new_id":"2c3d4e5f","squashed":["9d0e1f2a","5f6a7b8c"],"subject":"(lenses) Add lenses","branch":"lenses",
 "rebuilt_branches":["lenses","ui"],"undo_token":"<full previous>..<full new>"}
```

- The message of `<into>` stays; `--subject` replaces its subject and keeps its body; `--message` and
  `--message-file` (`-` for stdin) are the whole message.
- `rebuilt_branches`: the branch of the squashed commit and those of the commits between it and the newest squashed
  one, whose files change, so the next sync builds them anew (a pushed one needs a push again). The commits after
  the newest squashed one keep their mapping notes, so a branch of only those keeps its virtual branch.
- `--dry-run` builds the squash and drops it (`check_squash_commits_core`): a conflict shows before the user approves.
- Refused with nothing changed: `not_ahead_of_baseline`; `invalid_arguments` for an ambiguous id, fewer than two
  commits, an `<into>` that is not the oldest, a commit of another branch, a message that moves the commit to another
  branch; `squash_conflict` (exit 4, `commits`, `paths`), `squash_empty` (exit 4); `head_moved`, `locked`,
  `operation_in_progress`.
- `bd undo <undo_token>` restores the previous head.

### `bd move-hunk <from> <to> <path> [--block <id>]… [--expect-head <id>] [--dry-run | --sync]`

Moves the changes of the file `path` from commit `from` into commit `to`, older or newer, both ahead of the baseline:
the fix for a change that landed in the wrong commit, without `commit-tree` plumbing. Without `--block` the whole
change of the file moves (also of a binary file, an added or a deleted one). `--block <id>` (repeatable) moves only
the blocks with the ids that `bd commits <from> --patch` prints. One rewrite that keeps HEAD's tree, so the index and
the working tree stay as they are, computed before the repository lock and written under it after the witness of HEAD
([Git Operations](../design/git-operations.md#move-a-hunk)).

```json
{"schema":2,…,"dry_run":false,"previous_head":"77e01aa4","new_head":"c0ffee12",
 "moved":{"path":"src/Host.kt","from":"9d0e1f2a","to":"1b2c3d4e","blocks":[{"id":"2,1:2,1@1a2b3c4d","old_start":2,
   "old_lines":1,"new_start":2,"new_lines":1,"kind":"changed","header":"@@ -1,3 +1,3 @@","text":" a\n-b\n+B\n c"}]},
 "commits":[{"commit":"1b2c3d4e","new_id":"2c3d4e5f"},…],"changed":2,"undo_token":"<full previous>..<full new>"}
```

- Into an older commit, the change is added to the trees of `[to, from)`, and `from` keeps its tree, so it holds only
  the rest; into a newer one, it is taken out of the trees of `[from, to)`, and `to` keeps its tree. A block applies by
  its line ranges, so a commit between that only shifts the lines takes it; one that changes the same lines, or lacks
  the file, is `hunk_conflict` (exit 4, `commits` and `paths` name it), and nothing changed: bd can't move it.
- An id holds a hash of the lines of its block. When the file changed since `bd commits --patch`, an id matches no
  block: `block_not_found` (exit 2), with `error.blocks`, the ids of the file's blocks now.
- `commits` maps every recreated commit, oldest first. `changed` counts the commits whose changes differ: `from`,
  `to` and the commits between whose diff of the file changed. They get the current user as committer and lose their
  mapping note, so the next sync copies them again; the others keep author, committer and note, and their virtual
  commits stay.
- Refused with `invalid_arguments`: the same commit twice, a file `from` doesn't change, a rename, a mode change, a
  symlink or submodule, `--block` of a file without blocks (binary, or only the mode or the line ends changed), an id
  that does not parse, and a move that leaves a commit without changes (squash it instead). All blocks named is the
  whole file. `not_ahead_of_baseline` names a commit id.
- `--expect-head` refuses with `head_moved` unless HEAD is still that commit; a held lock is `locked`.
- `--dry-run` prints the blocks and checks the rewrite (the merges and the empty commits); it writes only git objects.
  `--sync` syncs after the release of the lock, as `bd group --sync` does. `bd undo <undo_token>` restores the previous head.

### `bd check [--plan <file | ->] [--build COMMAND [--build-paths PATH…] [--rebuild]] [--fix [--max-trials N] [--time-budget SECONDS]] [--allow-dangling] [--allow-missing]`

A trial sync of a plan (`trial_check::check`), or of the current grouping without `--plan`: stdin is read only for
`--plan -`, so the open stdin of an agent or a CI never holds it. The plan is applied in memory
and validated as `bd group --dry-run` does, `descriptions` included (an invalid plan is exit 2), the branches are
placed as a stacked sync places them, and each one is built with the sync's `build_branch`, parents first. It writes
commit objects and the object cache only: no ref, note, config, HEAD or working tree changes, no remote status and no
repository lock.

```json
{"schema":2,…,"head":"77e01aa4","baseline":"origin/master","base_commit":"3c0ffee1","rewritten":1,"ok":false,
 "branches":[{"name":"a","built_on":null,"commit_count":1,"result":"ok"},
  {"name":"b","built_on":null,"commit_count":1,"result":"conflict",
   "conflict":{"commit":"5e2f9c3b","subject":"(b) Change shared","files":["shared.txt"],"missing_commits":["41c7d00e"]}}],
 "unassigned_count":0}
```

- `result` is `ok`, `conflict`, `dangling_reference`, `missing_declaration`, `blocked` (with `blocked_by` `{parent, reason}`: the parent has a
  problem, conflicts or is blocked; a parent without commits is passed), `problems`, `error` or, with `--build`,
  `build_failed`. `built_on` is the branch it would be built on,
  `null` for `base_commit`.
- `missing_commits` are the older commits outside the branch and the branches it is built on that change the
  conflicting files, computed from the cached commit diffs: move them into the branch, or stack the branch on the
  one that has them.
- `dangling_reference`: git merges the branch, but its build fails. It removes a declaration (a top-level type or
  function, a static member) that a file of its virtual tree still uses, and a commit outside it, `fixed_by`, changes
  that use in HEAD. `dangling_references` lists `{kind, identifier, file, line, removed_by, fixed_by, mutual,
  declared_in}`, `declared_in` the `{file, line}` of the declaration before `removed_by`, for a reader to judge whether
  the use means it: move
  `fixed_by` into the branch or stack the branch on the one that has it; `mutual`, when the change in `fixed_by` uses
  what the branch adds (as in a move of a constant into a new class), means no stack builds both, and `fix` says how
  the change gets into the branch: `{kind: "move_commit", branches}` when `fixed_by` changes only `file` (add
  `branches` to the plan), else `{kind: "move_hunk", command}`, a ready `bd move-hunk <fixed_by> <removed_by> <file>`
  that moves the whole change of the file (add `--block <id>` to move less). A `fix` is there only when a trial build of
  the plan with it proves it: the branch builds and no branch that built breaks (a hunk move is tried on commits
  written as objects only, at most two per check); without one, `--fix` searches the regroups. A use that no commit here changes, or one that the baseline got after the base (Patronus
  merges onto its tip), is `warnings[]` `{kind: "possible_dangling_reference", branch, identifier, file, line,
  removed_by, on_baseline, via, declared_in}` and doesn't count for the exit code. The lines find the candidates (Java,
  Kotlin, Groovy, Scala, Rust, TypeScript, JavaScript, Python, Go): instance members are not searched, and a use through
  an alias or a star import is not seen. A Rust name after a single `.` is a field or a method, no use; with
  `branchdeck.symbols on` a Rust file of another crate uses a declaration only through its crate (its name, or a `use`
  of it that names the declaration, a glob or the module of the path), and a `pub(crate)` one not at all.
  `--allow-dangling` counts a branch whose
  only failure is a dangling reference in `ok` and the exit code (its `result` stays), for a false alarm, as
  `bd safe-push --allow-dangling` starts anyway. The syntax trees of Kotlin, Java, Rust, TypeScript and JavaScript then take away
  a word of a comment or a string, a local of the same name, a name a `use` or an import alias still binds and a call
  on an expression of a Kotlin top-level function that is no extension: `via`
  is `syntax_tree` where they confirmed the uses, `lines` where the lines alone found them. With `branchdeck.symbols
  on` the syntax summaries find three more kinds by themselves (`kind`; the lines' are `removed_declaration`):
  `removed_declaration` of an instance or static member, or an enum entry, used on a receiver of its type, by `this`,
  an override, an inherited call or its static path (not by a bare name elsewhere); `changed_declaration`, a Kotlin or
  Java function, property or constant whose header, parameters or reach the branch changes (`removed_by`), the
  constructors of a type or an overload it removes, used where the old form fits and no form in the branch's tree does:
  a call the new parameters reject, an added `suspend` outside a coroutine, a changed extension receiver (a parameter
  with a default, a call in a coroutine, a `var` for a `val` take the old uses; a change of the body is none). A change
  the summary can't judge a use by (a return type, nullability, type parameters, a suspend call in the lambda of an
  unknown function) is only a `possible_changed_declaration` warning, with the `fixed_by` to stack on if the build
  fails; `duplicate_declaration`, a type or a
  Kotlin top-level function the branch adds while its tree declares the key in another file of the package and module
  (`file`, `line`) that `fixed_by` deletes: a move whose deletion is in another branch. Their possible ones are
  `possible_changed_declaration` and `possible_duplicate_declaration`. It searches only the paths where the branch's tree and HEAD's differ,
  so it costs a `diff-tree` and a `cat-file`, plus one `log --raw` and one `cat-file` for the commits the cache lacks.
- `inverted_stack` `{branch, child, fixes}`: the branch conflicts, or has a dangling reference, because it needs a
  commit (`missing_commits`, `fixed_by`) of `child`, a branch stacked on it: no order of the stack builds both. Each
  fix is `{kind, branches, unstack}` to add to the plan, kept only when a trial build of the plan with it builds both
  branches (`trial_check::inverted_stack`; bd only shortens the ids and words the `reason`): `move_to_child` moves the branch's commits that need the child into it, `swap` stacks the branch on the
  child (`unstack` the child, `"child/branch": []`); no `swap` when the child's change uses what the branch adds
  (`mutual`). With no fix that builds, `fixes` is empty and `reason` says why. The `result` stays.
- `--build COMMAND` runs a shell command (`sh -c`, on Windows `cmd /D /S /C "COMMAND"`, the command line as written)
  in the tree of each `ok` branch, extracted with `git archive` into a temporary directory outside every working tree
  (`BD_BRANCH`, `BD_COMMIT` set), and removes it afterwards; `build` is `{exit_code, output}` (the last 50 lines of a
  failure), and a failure is `build_failed`. `--build-paths` extracts only those paths (files or directories, names,
  not patterns): the whole tree of a large repository is gigabytes per branch.
- A build that passed is not run again on the same command, branch and extracted objects (the tree, or with
  `--build-paths` the object of each path the tree has; one `cat-file --batch-check` reads their ids): `build` is
  `{exit_code: 0, cached: true}`, also after a regroup that wrote new commits of the same tree, since `BD_COMMIT` is not
  part of the key. A failure always runs again: it may be by chance. The key knows nothing outside the tree (a JDK, a
  toolchain, the environment) and nothing that depends on the commit (`export-subst` of `git archive`): `--rebuild`
  runs the command anyway, `--no-cache` skips the whole object cache.
- Unless `git config branchdeck.symbols` is `report` or `off`, `missing_declarations` lists
  `{branch, identifier, file, line, used_by, declared_by, declared_in, evidence}`: a commit of a built branch uses a
  name that a commit outside the branch and the branches it is built on adds or gives the form the use needs (an
  overload, a constructor parameter, a wider reach), overrides it (`evidence: "override"`) or calls it as an inherited
  member (`evidence: "inherited"`), on a receiver of its type (`evidence: "receiver"`) or through the class of its
  Kotlin file (`evidence: "facade"`), by the edges by symbols. git merges it and the build would fail; stack the branch
  on `declared_in` or move one of the two. The branch's `result` is `missing_declaration`, which fails `ok` and exits 1,
  as it refuses `bd safe-push`; names, not a compiler, so `--allow-missing` counts it as building for a name the user
  says is a false alarm.
- `--fix` adds `fix`, the regroup after which the branches that conflict (or have a proven dangling reference) apply
  cleanly, found by trial builds on top of the plan (`trial_check::conflict_fix`, the search of the app's Fix): nothing is
  applied. `fix.components` are independent parts, each `{steps, plan, fixed, unblocked, emptied, orphaned_children,
  rewritten}`; `plan` is the whole plan (the `--plan` included) for `bd group --plan … --expect-head <head> --sync`.
  A step is `move` (`commits` leave `from` for `to`, the branch whose stack has what they need), `pull_in` (what `to`
  needs comes into it from `from`; none for unassigned commits) or `restack` (`fixes` is stacked on `to`). `unresolved`
  names the branches no candidate fixed; `search` is `complete`, or `budget` when the trial or time limit stopped it,
  with the best plan so far (run it again: the merges of the first run are cached). `--max-trials N` (12) and
  `--time-budget SECONDS` (180, from the start of the command) set the limits. A branch that conflicts as a whole is
  restacked first, in one trial, on the branch (or the chain of branches) that holds what it needs; the dangling
  references come after the conflicts. The search prints a line per trial on stderr (`trial 3/12: restack
  control-runtime-peer-cancel on cloud-spawner-exit … verified (41 s)`). See
  [Conflict Fix](../../spec/branch/conflict-fix.spec.md).

```json
{"schema":2,…,"ok":false,"branches":[…],
 "fix":{"search":"complete","trials":3,"components":[{"steps":[{"kind":"move","commits":["e1b77cc1"],
   "from":["IJAI-3852"],"to":"IJAI-4047","fixes":"IJAI-3852"}],
   "plan":{"branches":{"IJAI-4047":["e1b77cc1"]},"unassign":[]},"fixed":["IJAI-3852"],"rewritten":1}]}}
```

- Conflicts are light: the files only, without the contents and the analysis a conflict view needs.
- Merges are memoized in the object cache by `(parent tree, commit tree, target tree)` (`merge_result`), and commit
  trees by commit (`commit_tree`): trying variants of a plan repeats no `merge-tree`. On the grouped 169-commit clone,
  a check takes 3.4 s cold and 0.2 s warm.
- The exit code is 1 when any branch is not `ok`.

### `bd suggest [--fresh] [--max-commits N] [--rewrite-issues]`

Suggests a grouping of the commits ahead of the baseline into branches, as a plan for `bd group`, and checks the plan
with the trial sync of `bd check`. It writes git objects and the cache only: no ref, config, index or working tree
changes, and no repository lock.

It reads the commits once with a `CheckSession` (`trial_check::check`: the snapshot of a regroup, the baseline and the
branch tips), their changes from the object cache without lock files, generated files and hub files
(`read_linked_diffs`), the dependencies of `bd commits` and the fixup targets. The clustering
(`crates/commit-clustering/src/clustering.rs`) runs no git:

1. Hard units, joined with union-find: the dependency edges, a fixup and its target, the commits with the same
   `(prefix)` or of the same issue group (an anchor), and the commits without a prefix that have the same issue key.
   `--fresh` ignores the anchors. A dependency between two anchors joins nothing: it is an `anchor_dependency`
   warning. With stacked branches (`branchdeck.stacks`), the anchor of a prefix is the leaf of its path, and a
   dependency joins nothing either: it stays an edge between two units, for the stack (step 4). Only the edges of a
   cycle of units join them (Kosaraju), because no stack builds a cycle. The commits without a prefix of an issue key
   that a prefix names join that prefix: `(K-1) X` and `K-1 Y` are one branch. With `branchdeck.symbols on` (the
   default), an edge by symbols only (a use of a name another commit adds, no line between them) joins nothing, ever: with stacked
   branches it is an edge for the stack, outside the cycle join; without them it only links a follow-up (R, step 2);
   a unit whose names three or more units use is a shared helper, and its edges order nothing.
2. A score between two clusters, `S = max(R, P × Kp) × K + adj`, in integer units of 1/10 000 so that ties are exact:
   - R, the links: with stacked branches, a dependency between the two sides when a side without an anchor has no
     issue key (0.95): a follow-up joins the branch it needs, and a commit with its own key stacks on it instead; a
     body names an issue key of the other side (0.9); both sides name a third key (0.7); the same conventional scope
     (0.8 without issue keys, 0.5 with them). A key that more than max(10, units / 3) units name links nothing: it is
     the key of a series. A branch of a prefix takes in a whole issue branch by the mention only when two of its bodies
     name the key or they share paths (P of at least 0.2): one body that names it is a reference, not the same work. A scope is read from `type(scope):` and from `type scope:`, as in `setup vm:`
     (`parse_semantic_prefix`).
   - P, the paths: the IDF-weighted Jaccard similarity of the areas (the first 3 directories, weight 0.5), the modules
     (the directories before the first `src`, `testSrc`, `test`, `tests`, `resources` or `gen`), the packages (the
     directories after it) and the files. A feature of more than max(4, units / 4) units weighs 0. A file under 5 % of
     the commit's lines and outside its main area weighs 0.2.
   - K: 0.3 when the issue keys of the two sides are of different projects (`ABC-1` and `XYZ-2`), 0.5 when both sides
     have scopes and none in common. Kp: 0.7 when the keys differ in the same project.
   - adj: 0.05 when the two sides have neighbouring commits in the log and R or P is not 0.
3. Merges, the best pair first and on a tie the oldest, until the best score is under 0.40 (`MERGE_THRESHOLD`). Two
   anchors never merge, no merge makes a cluster of more than `--max-commits` commits (default 40), and no merge
   makes a cycle: a pair where one side reaches the other through a third, over the dependencies and the declared
   parents, waits. So a follow-up of a branch and its parent joins the branch, not the parent. Each row of
   the score matrix keeps its best column, so a merge rescores one row and one column. On synthetic input of commits
   that all merge, 300 commits take 0.3 s and 1000 take 6.9 s in a debug build (1.1 s in a release build); commits
   that don't merge take 16 ms and 0.14 s.

4. With stacked branches, `place_clusters` gives each cluster a parent, so that every cluster it depends on is an
   ancestor (`stack_clusters`). The clusters go in a topological order of their dependencies and declared parents,
   the oldest first on a tie. Each takes the parent its prefixes declare when that parent covers what it needs, else
   the deepest cluster it needs that covers the others. When the clusters it needs lie on two stacks, those stacks
   become one chain in that order, which only adds ancestors, so the clusters placed before keep theirs. In a cycle the
   declared parent gives way first, then the oldest cluster of a cycle that waits on nothing outside it goes first: a
   cluster that only waits on a cycle stacks on it instead of going first and losing what it needs. A declared parent that is no cluster goes,
   because the sync would build the branch on the baseline anyway, and `--fresh` declares no parent.

Each cluster then gets a name, the first of: its prefix; the branch its commits come from, when it holds the most
commits of that branch (so `--fresh` keeps the names of the branches it finds again); its single issue key; a scope of
at least 60 % of its commits; `<area leaf>-<word>`, for the subject word of at least half its subjects with the highest
TF-IDF (issue keys and the conventional prefix are not words); its most frequent key; `cluster-N`. When another cluster, a group now or an existing virtual branch has the name (in any case), and the
cluster's commits don't come from it, the name gets `-2`, `-3`, and so on.

A prefixed commit whose own issue key names its cluster is in `unassign`: the prefix goes, and the key puts the commit
into the issue branch of the cluster, so it stays in the cluster, not without a branch.

The name of a stacked cluster is its leaf, and the plan names it `parent/leaf`, and a root by its leaf. A prefix path of
more levels, as `(a/b/c)`, also declares the parent of its parent, which the placement may have moved, so the plan
rewrites it to `b/c`. The keys of a stacked cluster leave their issue group (`ungroup_issues`), because an issue
group can't name a parent.

The trial sync builds the plan. A conflict joins the conflicting commit, as hard edges, to the commits it needs by its
lines (the closure over the dependencies of all its files, lock and generated ones included) that its branch and the
branches it is built on lack; to its `missing_commits` (every older commit there that changes a conflicting file) only
when the lines name none, else to the newest older commit outside its branch that changes a conflicting file. Then the
clustering runs again, at most 5 times. An edge joins two hard units, also two of one cluster that build apart (an issue group,
below): the joined unit then goes into the plan. A repair never joins two anchors, so a conflict between two prefixes
stays. With stacked branches, a conflict on a commit of another cluster first stacks the two clusters; only when the
same conflict comes again does the next round join their units.

```json
{"schema":2,…,"head":"77e01aa4","baseline":"origin/master",
 "plan":{"branches":{"vm":["41c7d00e","5e2f9c3b"]},"unassign":[],"issue_groups":{"parser-parse":["ABC-1","ABC-2"]}},
 "clusters":[{"name":"vm","commit_count":2,"change":"new","current_branches":[],"kept":[],"new_path":[],
   "moved":[{"commit":"41c7d00e","leaves":null},{"commit":"5e2f9c3b","leaves":null}],
   "why":"2 units; scope vm in 2 subjects; area vm","weak":false,"issue_keys":[],"areas":[{"dir":"vm","lines":2}]},
  {"name":"parser-parse","commit_count":2,"change":"merged","current_branches":["ABC-1","ABC-2"],"kept":[],"new_path":[],
   "moved":[{"commit":"9c0d1e2f","leaves":"ABC-1"},{"commit":"a3b4c5d6","leaves":"ABC-2"}],
   "why":"2 issue units; ABC-1 named in 1 body; area parser","weak":false,"issue_keys":["ABC-1","ABC-2"],
   "areas":[{"dir":"parser","lines":2}],"via":"issue_group"}],
 "rewritten":2,"check":{"ok":true,"repair_rounds":0,"branches":[…]}}
```

- `plan` lists only the commits whose branch changes. A fixup that follows its target is left out, and a commit with a
  prefix that goes to the branch of its own issue key is in `unassign`. Give the plan to `bd group` as it is, or edit
  it and run `bd check`.
- Per cluster `kept`, `moved` and `new_path` name its commits, each once: `kept` those the plan leaves as they are,
  `moved` those that change branch, each with `leaves`, the branch it leaves (`null` for an unassigned commit), and
  `new_path` those whose branch stays but whose subject the plan gives another stacked path (`(gone/a)` to `a`, a
  restack: in `plan`, with a new id). `moved` holds the commits of the plan that go to the cluster, a fixup that follows its target (which
  the plan leaves out) and the commits an issue group takes in or lets go. `moved` and `new_path` together are what
  the plan changes (the top-level `rewritten` counts them): refuse a plan that changes a commit of another session. Without `--fresh` a commit with an
  explicit prefix is never in `moved`: its prefix is an anchor, `--rewrite-issues` included.
- Per cluster: `change` against the groups now in `current_branches` (`kept`: one whole branch under its name; `renamed`: one
  whole branch under another name; `merged`: two or more whole branches, or one with unassigned commits; `split`: a
  part of one branch; `new`: anything else), `why` (one line to
  quote to the user, with the repair rounds and a new parent), `weak` (a merge by paths near the threshold, or one
  that adjacency decided: review these first), `issue_keys` and `areas` (as in `bd commits --brief`). With stacked
  branches also `parent` (the branch it builds on) and `restacked` (the parent changes). `stacked_without_need`: the
  stack parents stack it on `parent`, a line with no prefix behind it, and it depends on no commit of `parent` or of
  its ancestors. The plan keeps the parent: the stack parents don't say why a branch is stacked, and "Stack On…"
  writes the same line. `bd unstack` makes it a root.
- A stacked cluster keeps its issue group: the plan names its path with an empty list (`"a/grp": []`), which writes
  the stack parent without a rewrite. A cluster unstacked from its stack parent is in `unstack`.
- A cluster of two or more whole issue branches goes into `issue_groups` of the plan, marked `"via":"issue_group"`:
  its subjects keep their keys, and `rewritten` stays 0 for it. `--rewrite-issues` gives its commits a prefix instead.
  An issue group is an anchor, as a prefix is: it stays one branch unless `--fresh`.
- `rewritten` counts the subjects the plan changes. `check` has the `bd check` result of each branch and the
  `repair_rounds`. `warnings` lists `anchor_dependency` (with stacked branches only a cycle between two prefixes),
  `unresolved_conflict` and, with `branchdeck.symbols on`, `symbol_dependency` (a branch uses a name another branch
  adds and the plan doesn't build it on that branch: git builds them apart, a build would fail), with the branches.
- The exit code is 1 when a branch of the plan still conflicts, as for `bd check`.
- On the 74 commits and 12 branches of a large monorepo branch it keeps the 12 branches with an empty plan, in 0.27 s
  warm (a debug build). With `--fresh` it finds the grouping a human made there with a pairwise precision of 1.0 and
  a recall of 0.945, and its plan rewrites 6 subjects. The golden test of `clustering_test.rs` checks its anonymized
  input: `--dump-input` (hidden) prints the input with every name hashed by SipHash with a random key of the
  operating system, which is not kept.

### `bd sync`

Rebuilds the virtual branches with `sync_core::sync::sync_branches`, the app's sync: it includes the integration
detection of archived branches and the update check of the checked out branch. Its events fold into the table of the
app's session as they come (`repository_model::table::TableState`), and the output reads that table: each branch says
what its row in the app says. It takes no repository lock, as the
app's sync takes none: every ref it moves is a compare-and-swap on the tip it listed, and a rewrite or an Update Project
of the app or of another `bd` can land while it runs, so its output is the branches as they were when it read them;
the next sync shows the change. While a rebase, merge, cherry-pick or revert is in progress it is `operation_in_progress`, on a detached HEAD `detached_head`
(exit 4), and changes nothing: a half-done history would archive the branches it lacks; finish the operation first.

```json
{"schema":2,"repository":"/repo","prefix":"develar","baseline":"origin/master","unassigned_count":3,
 "branches":[{"name":"auth","ref":"refs/heads/develar/virtual/auth","state":"created","commit_count":2,
   "tip":"1b2c3d4e","remote":{"exists":false,"unpushed":2,"behind":0,"head":null},"push":"bd -C /repo push auth"},
  {"name":"login","ref":"…","state":"conflict","commit_count":1,"tip":null,
   "conflict":{"commit":"9d0e1f2a","subject":"(login) Use the session","files":["src/session.rs"],
     "missing_commits":["5f6a7b8c"]}}],
 "archived":[{"name":"old-fix","state":"integrated","integrated_at":1700000000}],
 "update":{"upstream":"origin/master","state":"up_to_date"}}
```

- `state` is `created`, `updated`, `unchanged`, `blocked` (a stacked branch whose parent can't be built on, with the
  reason in `error`), `error` (with `error`) or `conflict` (with `conflict`: the
  commit whose cherry-pick conflicts, the files, and the commits the change likely needs: the older commits that change
  a conflicting file and are neither in the branch nor in the branches it is built on, as `bd check` names them).
  `blocked` lists the commits not synced after an earlier failure.
- `resolved` lists the commits whose conflict of git the sync resolved by words, oldest first:
  `{"commit","conflicts","file_count","files":[{"path","conflicts"}]}`, the conflicts counted as git counts them (its
  `<<<<<<<` lines). With `git config branchdeck.resolveByWords` on (the default), a cherry-pick that git stops at is
  committed with the result of the engine when every conflicting file resolves: a content conflict of UTF-8 text whose
  blocks each take the side that changed or resolve by words (IntelliJ's simple resolve, never the greedy one). One
  file that does not, or another kind of conflict (modify/delete, add/add, rename, binary, a merge driver), and the
  commit stays a conflict of git. The copy keeps the author, the date and the message; its mapping note records the
  counts, so a reused copy has `files` left out. `bd check` builds alike, and `bd move-hunk` never resolves.
- `remote` compares with the remote-tracking branch of `origin` by content: `unpushed` commits it lacks, `behind`
  commits it has that the branch doesn't (a push drops them), and `head`, the remote commit the counts compare with
  (`null` without a remote branch). `dropped_commits` lists the newest ten of the `behind` commits, newest first:
  `{"commit","subject","author","author_time"}`. Older versions of commits the branch or the baseline has are not
  `behind`: `amended` counts those of commits amended here, `restacked` those of commits of the branches it is built
  on (amended or rebuilt since it was pushed), `landed` those of commits the baseline has above the remote branch (a
  Safe Push landed them, also rebuilt with other changes: the remote branch of a member after its whole stack landed),
  with the newest ten in `landed_commits`, `{"commit","subject","baseline_commit"}`; same author, author time and
  subject without the prefix, a push replaces them. `reworded` counts the commits the remote has by content in another
  version (a reword, a new author): not `unpushed`, yet a push replaces their remote copies, which a Safe Push of the
  remote branch would land as they are. Each is left out at 0.
- `push` is the `bd push` that brings the remote branch up to date, quoted for the shell, to run only after the user
  approves: `bd push <branch>` (older versions of amended, restacked or landed commits and reworded copies need no
  `--force`),
  `bd push <branch> --force --expect-remote <remote.head>` when the push drops `dropped_commits`, and
  `bd push <branch> --force` when a branch it is built on has commits for the remote too (`--expect-remote` would push
  the branch alone). It is left out for a branch whose remote branch is up to date, one that didn't sync, and without
  a remote.
- `update.state` is `up_to_date`, `fast_forward`, `rebase`, `conflict` or `blocked` (with `files`: at most 50
  conflicting files or 100 blocking paths, `files_omitted` counts the rest), or `error`.
- `update.landing` lists the local commits the fetched baseline already has (the update drops them), and `landed` what
  landed with an update and the app hasn't dismissed: per branch (omitted for unassigned commits) the commits with
  `commit`, `subject` and the baseline commit `upstream` (omitted when squashed upstream), and `remaining`, the commits
  that stay local.
- With stacked branches on, `stacks` lists the stack branch of each stack that forks (a branch of it has two children
  or more), then those kept for their remote branch: `{prefix}/stack/<root>` (`branch`), all the commits of the
  stack's branches (`members`, the root first, each after its parent; `commits` counts them) in the order of the
  history, on the base of the sync, as a flat virtual branch of them. It is what a Safe Push of the whole stack lands;
  a straight chain has none, its leaf is the whole stack. `state`:
  - `built`: `tip` is the build of the stack's commits now;
  - `conflict`: a commit conflicts with the commits of the stack before it (`conflict`: `commit`, `subject`, `files`);
  - `blocked`: `member` failed or is blocked (`reason`: `problem`, `error`, `conflict` or `blocked`, as its branch says), the
    first in stack order, while the stack branch itself builds;
  - `stale`: not rebuilt for another reason (`error`): another process moved the ref, or the stack no longer forks and
    the ref stays for its remote branch (then `members` is empty).

  In every state but `built` the ref keeps its tip: never push it. `remote` is the remote summary of `tip`, as for a
  branch. The stack branch writes no mapping note; its build is remembered by its commits (the object cache), so an
  unchanged one costs no git process. A stack that no longer forks loses its stack branch unless it has a remote
  branch or a worktree has it checked out.

  ```json
  "stacks":[{"root":"auth","branch":"develar/stack/auth","state":"blocked","tip":"5e6f7a8b","member":"login",
    "reason":"conflict","commits":5,"members":["auth","login","session"],
    "remote":{"exists":false,"unpushed":5,"behind":0,"head":null}}]
  ```
- A virtual commit is reused only while its mapping note still fits the commit: the note names the trees it was copied
  from, and the copy must have the message and the author of the commit. A note that `git notes copy`, `git rebase` or
  `git commit --amend` (`notes.rewriteRef`) carried to a changed commit is stale; the sync copies the commit again and
  logs it (`-v`). With `--no-cache` every note is checked against the tree of its copy.
- The exit code is 1 when any branch was not built (`conflict`, `error`, `blocked`); the summary is printed
  either way.

### `bd push <branch>… [--with-children] [--alone] | --all --yes [--force] [--expect-remote COMMIT] [--remote R] [--dry-run] [--restart-running]`, `bd push --unapplied <name>`

Syncs, then pushes the branches in one atomic `git push --porcelain` with a lease per ref, parents first: the sync
without the repository lock, as `bd sync`, the witness of the plan of the push (its refs, their commits and leases)
under it, the retargets, the cancel and the restart of `--restart-running` and the network push after it (see "Push and land" and "Agent push" in
[Git Operations](../design/git-operations.md)). Every pull request of a virtual
branch goes into the baseline, a stacked branch's with the commits of the branches it is built on
([Landing](../design/landing.md)). A stacked branch takes those branches along when they have commits for the remote,
as the app's push button does, so their remote branches stay what it is built on. `--alone` pushes only the named
branches, which carry their parents' commits; `--expect-remote` does too. Before the push bd retargets the open pull
requests of the branches it pushes whose base is a virtual branch to the baseline (`retargeted`).

```json
{"schema":2,…,"remote":"origin","dry_run":false,
 "branches":[{"name":"auth","remote_branch":"develar/virtual/auth","commit":"1b2c3d4e","state":"new",
   "lease":null,"lease_source":"absent",
   "pull_request":{"base":"master","head":"develar/virtual/auth","title":"Add login","body":"The login form.",
     "command":"gh pr create --base master --head develar/virtual/auth --assignee @me --title 'Add login' --body-file -"}}],
 "retargeted":[{"branch":"login","number":8,"url":"https://github.com/acme/monorepo/pull/8","old_base":"develar/virtual/auth",
   "new_base":"master","approved_by":["alice"]}]}
```

- Refused with `push_refused` (exit 4), nothing pushed: a branch that didn't sync, and one whose remote branch has
  commits it lacks (`behind`) without `--force` (also a parent taken along: `--alone` leaves it). `amended`,
  `reworded`, `restacked` and `landed` need no `--force`: the push replaces older versions of commits the branch or
  the baseline has, and a parent with only them goes along. A pushed branch names the landed remote commits it
  replaced in `replaced_landed` (`{"commit","subject","baseline_commit"}`, with `--dry-run` the ones it replaces): tell
  the user they are in the baseline, nothing of them is lost. `--all` needs `--yes`. A
  branch refused as behind is in `error.branches` with its remote status, as below.
- Refused with `robot_running` (exit 4), nothing pushed: on Space, a named branch whose Safe Push runs on it ("A Safe
  Push of auth is running on develar/virtual/auth: a push moves the branch it took its commit from, …"), or on a branch
  built on it whose commit has commits of it, also with `--alone` ("A Safe Push of oauth is running: it lands 2 commits
  of auth with it (1a2b3c4d, 5e6f7a8b), a push won't land"): a push won't land, and a push into the robot's branch fails
  it at the end. What a robot lands is what its commit has: a parent a restack placed below its branch after it started
  is pushed. An ancestor taken along that a Safe Push lands stays as the robot took it, and its child goes alone, with
  its commits. The Safe Push of the whole stack holds every branch of it its commit has ("A Safe Push of the whole auth
  stack is running: it lands 1 commit of login with it (1a2b3c4d)"), and `--with-children` of the root leaves the stack
  branch it runs on out (a `stack_warnings` line). Tell the user; only after the user approves, `bd push auth --restart-running` cancels the Safe Push of
  `auth`, pushes and restarts it on the pushed commit (`"restarted":[{"branch":"auth","url":…}]`). One robots request
  after the sync; off Space or without the Safe Push CLI's token nothing is held. A dry run or a canceling robot holds
  nothing.
- A parked branch (`parked` in `bd status`) stays local, and so does a branch built on it, which carries its commits:
  named, the push is refused with `parked` (exit 4), nothing pushed; with `--all` or `--with-children` it is left out
  and the others push, `skipped` naming it with `code` `parked` and the reason (`"login is built on parked auth"`);
  the stack branch of a stack with a parked member is left out (`stack_warnings`). Nothing but parked branches left is
  `parked` too. Tell the user; never unpark it on your own.
- A named branch a merged Safe Push landed all of (`landed_by` without `after` in `bd status`), and with
  `--with-children` one built on a named one, is left out and the others push: `skipped` names each, with `code`
  `landed`, the `message` and `landed_by` as `bd status` has it. Tell the user who landed it. An ancestor taken along
  that landed stays as it is. A branch with commits added after the landed ones pushes them.

  ```json
  "skipped":[{"name":"login","code":"landed",
    "message":"login merged into master with its Safe Push (https://patronus…/robot/7f…): nothing of it is left to push",
    "landed_by":{"branch":"login","robot":{"id":"7f…","name":"…","state":"success","mode":"safe_push",
      "commit":"9a8b7c6d","url":"https://patronus…/robot/7f…"},"commits":["1a2b3c4d"]}}]
  ```
- Refused with `landed` (exit 4), nothing pushed, before `robot_running`: nothing asked for is left but what merged
  Safe Pushes landed, the stack branch of `--with-children` included. The message names the named branches: "login
  merged into master with its Safe Push (<robot>): nothing of it is left to push", "auth landed in master with the Safe
  Push of login (<robot>): nothing of it is left to push".
- `--with-children` pushes the branches stacked on the named ones too, directly or further down, in the same atomic
  push; a blocked or conflicting child refuses it, as a named branch would. A parent taken along brings no children.
  Not with `--expect-remote`. A named root of a stack that forks also takes its stack branch along
  (`{prefix}/stack/<root>`, [Whole Stack](../../spec/branch/whole-stack.spec.md)), as the app's Push with Children
  does: once it is on the remote or the pull request of the whole stack is open (bd asks GitHub once for a stack branch
  the remote lacks), leased on the remote head its remote status compared with, when this sync built it and the
  remote lacks it: `stacks` lists it as `branches` lists a branch, `name` the root, its `pull_request` the one of the
  whole stack (the stack branch's own description, else a section per branch: its description, else its commits). One
  on no remote and in no pull request stays local and needs no word: `bd safe-push --whole-stack` (or its
  `--create-pull-request`) puts it there. Of one on the remote or in an open pull request, a stack branch in any other
  state than `built` is never pushed, and neither is one to a `--remote` other than `origin`, which its remote status
  compares with: `stack_warnings` names each one left out, and so does one every branch of whose stack merged Safe
  Pushes landed whole (it carries only commits that are in the target). Its remote branch with commits it lacks refuses
  the push without `--force`, as a branch's does.
- `state` is `new`, `updated`, `forced` or `up_to_date`. The lease is the remote head the sync of the same run
  compared with (`remote.head`, `lease_source: expected`), or none without a remote branch (`absent`: it must not
  exist). The push replaces exactly that remote state: a remote commit the branch has by content goes through,
  `--force` drops the `behind` commits of that sync, and nothing pushed since is lost. `--expect-remote COMMIT` leases
  one branch on the given remote commit, a `remote.head` bd printed: the remote state the user saw.
- With `--remote` other than `origin`, which the sync compares with, bd computes the same remote status of each
  pushed branch against that remote's remote-tracking branch, and the refusal, the lease and the errors use it. A
  `--remote` that is not a configured remote is `no_remote`.
- A remote branch that moved since the sync without commits its branch lacks (deleted, moved back to an older commit
  of it, rebuilt older versions) loses nothing: bd fetches it and pushes again at once, once, leased on what it has now,
  as the app's push does, and marks the branch `"pushed_over": true` (its `lease` the head it went over). Tell the
  user the remote had moved.
- Otherwise a remote branch that is not where the lease expects it is `lease_rejected` (exit 4, not retryable: a push
  again with `--force` would drop what the remote has now), nothing pushed: the push is
  atomic, so the other branches of it stay as they are on the remote too. bd fetches the branches of the push and
  lists the ones that moved in `error.branches` with their remote status as fetched:
  `{"name":"auth","remote":{"exists":true,"unpushed":1,"behind":1,"head":"7c8d9e0f","dropped_commits":[…]}}`.
  Show the `dropped_commits` to the user; only after the user
  approves, `bd push auth --force --expect-remote 7c8d9e0f`.
- A push the remote refuses while every lease holds (a hook, a protected branch, a pack too large) is
  `push_refused` with what git said of each ref and what the remote said why; nothing was pushed. Tell the user: a
  retry doesn't help.
- A push whose commits add more files than the push size limit (`branchdeck.pushSizeLimit`, 50 MiB unless set) is
  `push_too_large` before bd asks the remote, `--dry-run` too: the message says how much and where, `paths` lists the
  directory and the largest files. It is mostly build output committed by mistake (a `target/`, `node_modules/`): show
  it to the user, who takes the files out of the commits or raises the limit (Settings → Landing); never raise it
  yourself.
- `pull_request.base` is the baseline without the remote (`master`), for every branch. `title` and `body` are the
  `bd describe` text, else the text of the commits: the issue key and its YouTrack summary as the title (the summary
  only when the link of `.idea/vcs.xml` is an issue page of the YouTrack you are signed in to), the issues linked, an
  item per commit with its body as paragraphs, and for a stacked branch the branches it is built on, whose commits it
  holds too. `command` takes the title and reads the body from stdin (`--body-file -`).
- `retargeted`: before the push, with the lock released, the open pull requests of the pushed branches whose base is a
  virtual branch of the user (`<prefix>/virtual/…`: a parent's branch, opened when pull requests went there) go into
  the baseline (one search of the pull requests, one `PATCH pulls/<n>` each), as the app's push retargets them. Each
  has `branch`, `number`, `url`, `old_base`, `new_base` and `approved_by`: GitHub may dismiss the approvals of a
  pull request whose base changes, so name them to the user. A pull request into another base (a release branch)
  stays. A retarget GitHub refuses, or a listing that fails for another reason than a missing token, is a
  `retarget_warnings` line with the `gh pr edit` that does it; the push goes on. With `--dry-run`, `retargeted` lists
  what a push would retarget and GitHub is only asked. Without a GitHub repository of the branches or a token, GitHub
  is not asked.

### `bd safe-push <branch> [--whole-stack] [--patronus-dry-run] [--profile P] [--check] [--without-approvals] [--create-pull-request [--reviewer LOGIN]… [--no-assign]] [--cancel-running] [--force] [--allow-dangling] [--allow-missing] [--allow-unknown-rules] [--allow-unknown-review]`

Starts a Patronus robot on the virtual branch, as the Safe Push of the app ([Patronus](../../spec/branch/patronus.spec.md)):
a Safe Push (Patronus rebases the branch onto the target, runs the checks and pushes it into the target when they
pass) or, with `--patronus-dry-run`, a Patronus Dry Run (the checks only; it pushes and starts a robot as a Safe Push
does, so it needs the user's approval too; `--check` is the preview that starts nothing). The repository must be on Space (`origin` on
`git.jetbrains.team`, else `not_on_space`), and the Space token of the Safe Push CLI must be there: run
`./safePush.cmd` once, or set `SPACE_TOKEN` (else `not_logged_in`). `SAFEPUSH_PATRONUS_URL` picks another Patronus
instance, as for the Safe Push CLI. The app's setting Settings → Landing → "New Patronus API"
(`appSettings.patronusNewApi` of its settings file) holds for bd too: `bd robot` reads the progress of a running
robot in one request, and the robot lists name the merge profiles, so `bd safe-push` needs no request for the profile
of the last robot; off, or while Patronus has the flags off, the requests are the ones of before.

A stacked branch lands with every branch it is built on, in one robot, as the app's Safe Push of a piece
([Landing](../design/landing.md)): its virtual branch carries their commits, so the robot starts on it and Patronus
lands them all. The branches built on it stay; after the merge the next sync builds them on the target.

A parked branch (`parked` in `bd status`), a branch built on one and a `--whole-stack` with a parked member start
nothing, a Dry Run and `--check` included: `parked` (exit 4) right after the sync, before Patronus is asked.

```json
{"schema":2,…,"branch":"login","mode":"safe_push","target":"master","lands":["auth","login"],
 "profile":{"id":"lsc","name":"Large Scale Change","description":"All the tests"},
 "profiles":[{"id":"default","name":"Default"},{"id":"lsc","name":"Large Scale Change","description":"All the tests"}],
 "review":{"required":false,"skipped":false,"counts_in":"space_merge_request",
   "merge_request":{"key":"IJ-CR-7","url":"https://jetbrains.team/p/ij/reviews/7"},"groups":[]},
 "push_needed":true,"pushed":[{"name":"auth",…,"state":"new",…},{"name":"login",…,"state":"new",…}],
 "started_via":"api","robot":{"id":"r1","name":"Add login","state":"running","mode":"safe_push","commit":"1b2c3d4e",
   "url":"https://patronus.labs.jb.gg/robot/r1"}}
```

- `--whole-stack` lands the whole stack of the branch (any branch of it) in one robot, its side branches included
  ([Landing](../design/landing.md), [Whole Stack](../../spec/branch/whole-stack.spec.md)): the robot starts from the
  stack branch of its top-level root, `{prefix}/stack/<root>`, which the sync builds of every commit of the stack, and
  only that branch is pushed, leased (its branches stay as they are on the remote). `whole_stack` names the `root`,
  the `source` the robot starts from and the `state` of the stack branch (`built` starts); `lands` names every branch
  of the stack, each after its parent. A straight chain has no stack branch: its last branch carries the whole stack,
  and the robot starts from it (`source` is its virtual branch, no `state`). The approvals count in the pull request
  of the stack branch, the stack's own (`--create-pull-request` opens it, titled by the root, a section per branch in
  its body; never retargeted), the review asks Patronus about the stack branch with that pull request, and a start
  comments on it. Refused with `stack_conflict` (exit 4, `error.commits` and `error.paths`): a commit of the stack branch
  conflicts with the commits of the stack before it, though each branch builds; with `stack_not_built` (exit 4): a
  branch of the stack is blocked or failed, or the stack branch is stale; both say to land the stack in pieces
  (`bd safe-push <branch>` lands a branch with what it is built on). A remote stack branch with commits the stack
  lacks is `push_refused` (`bd push <root> --with-children --force` replaces them, after the user approves).
  `robot_running` both ways: a Safe Push of a branch of the stack lands a part of it ("A Safe Push of login is
  running: wait until it merges"), and the Safe Push of the whole stack lands every branch of it, so a branch's start
  waits for it ("A Safe Push of the whole auth stack is running: it lands 1 commit of login with it (1a2b3c4d)").

  ```json
  {"schema":2,…,"branch":"login","mode":"safe_push","target":"master","lands":["auth","login","session"],
   "whole_stack":{"root":"auth","source":"develar/stack/auth","state":"built"},
   "push_needed":true,"pushed":[{"name":"auth","remote_branch":"develar/stack/auth","state":"new",…}],
   "started_via":"api","robot":{"id":"r2",…}}
  ```

- `--check` syncs and prints the same object without pushing or starting anything: what it `lands`, the profiles, the
  review, `push_needed`, `remotes`, `target_check`, and `refusals`, each `{code, message}` a start would fail with. The
  agent shows it to the user and starts only after the user approves.
- The decision is the app's rule, `domain_policy::land::decide` ([Rust as the Domain Authority](../design/rust-domain-authority.md#s8-policy)),
  over what bd read: the `refusals` are its blockers, in their order, then each confirmation a flag accepts that the
  command lacks (`--cancel-running`, `--allow-unknown-review`, `--allow-unknown-rules`), named with the flag. What the
  table alone refuses (a branch that didn't sync, a parked one) fails before Patronus or GitHub is asked; the review
  and the rules of the target are asked only when the decision waits for them.
- `remotes` names each existing remote branch the start pushes, `{name, remote}` with the `remote` of `bd sync`: what
  the push replaces (`amended`, `reworded`, `restacked`, `landed`) and drops (`behind`, `dropped_commits`). One that
  drops commits refuses the check and the start with `push_refused` and the `bd push` that drops them, to run only
  after the user approves; the start's error has it in `error.branches`. `--force`, once the user approved, starts with
  the drop pushed away (D12). `push_needed` is true for a branch reworded
  since its push: the robot would land the old messages of the remote branch.
- The target conflict check (`target_check`, the core of the app's start too, `sync_core::target_conflict`): Patronus
  rebases what it lands onto the newest target before any check, and a conflict fails the robot at once. A branch is
  built on the base of the checked out branch, not on the newest target, and the target of a monorepo moves every
  minute. So before the lock bd fetches only the target's remote branch (as `bd update` does), then replays what lands
  onto its tip as a plain `git rebase` does, with no resolution by words. `result` is `clean`, `conflict` or `not_run`
  (the check did not run to its end; it holds nothing back), with the `target_tip`. A failed fetch doesn't stop the
  check: `fetched` is false, the check used the last fetched tip, and `message` says why. A conflict refuses the start
  with `target_conflict` (exit 4), nothing pushed or started: `error.commits` names the commit by the id of the checked
  out branch, `error.paths` the files, and the message names the commits of the target after the base of the branch
  that change them (hash, author, subject). The fix is `bd update`, which resolves it locally (a resolve session); then
  the agent starts again. `--create-pull-request` starts no robot and checks nothing.

  ```json
  "target_check":{"result":"conflict","target_tip":"faebba53","fetched":true,
    "message":"Nothing was pushed or started: Patronus can't rebase composer-kind onto origin/master: e90c2bf6 (Add the composer kind) conflicts in AgentPromptPalettePopup.kt, which origin/master changed after the base of composer-kind: 6fddca13 (Alice: IJAI-4233 Change the palette popup). `bd update` (the app's Update Project) brings the checked out branch onto origin/master and resolves the conflict locally; then run `bd safe-push composer-kind` again"}
  ```

- `lands` is what the robot takes, root first, ending with the branch: the branches it is built on, then the branch
  (only the branch for a root). A Safe Push lands them all into the target; a dry run checks the same commits; with
  `--create-pull-request` its pull request holds them. There is no refusal and no flag for it: the obsolete
  `--land-stack` of an older bd is accepted, hidden, and changes nothing.
- `alone` (only with `--check`, only for a stacked branch) is the trial build of the branch as a root, on the base the
  sync builds on, not on the newest target (`stack_independence` in `crates/trial-check/src/check.rs`): the second way,
  landing the branch without what it is built on, for the user to choose; bd never unstacks or moves anything for it.
  `result` is `builds` (it applies alone: `bd unstack <branch>` makes it a root; `uses` lists the names of its stack
  that its commits use, `{identifier, used_by, declared_by, declared_in}`, so it applies but would not compile;
  `symbols: false` means lines only), `needs` (`conflict` `{commit, subject, files}` is its first commit that conflicts
  alone; `needs` the commits of its ancestors it needs, `{commit, subject, branch}`, and what these need, by the line
  dependencies, so a compile dependency is not seen; `movable`: they can move into the branch, then `bd unstack`;
  otherwise other branches need them too, and a move would make those need this one) or `unknown` (the trial failed).
  `message` says it in one sentence:

  ```json
  "alone":{"result":"needs","parent":"auth","conflict":{"commit":"5e6f7a8b","subject":"Use auth","files":["auth.txt"]},
    "needs":[{"commit":"1b2c3d4e","subject":"Add auth","branch":"auth"}],"movable":true,
    "message":"login needs 1b2c3d4e (auth): move it into the branch, then `bd unstack login`"}
  ```

  It runs in the trial session of the dangling references; a start runs no trial.
- The profile: `--profile` takes an id or a name (any case), `none` for the default checks of the target; without it,
  the profile of the last robot of the branch, as the app's start does. `profiles` lists those of the target that allow the
  mode, never the emergency ones; another name is `invalid_arguments`, listing them.
- The code-owner review (never for a dry run): the commits of the branch go to the pre-check of Patronus. Missing
  approvals refuse a Safe Push with `approvals_missing` (exit 4), nothing pushed or started, as the app refuses the start, and
  an agent shall not send a robot that Patronus fails with "Changes require review approval". `error.review` lists the
  `groups` with their `reviewers` (`suggested`, `available`) and `paths` (`branch_wide` for all files).
- The approvals count in the GitHub pull request of the branch (Space merge requests are deprecated; one found open,
  `review.merge_request`, still counts for a start through the API). `--create-pull-request` pushes the branch and
  opens its pull request through the GitHub API, as the `gh pr create` that `bd push` prints would (into the target,
  with the text of `pull_request` of `bd push`), assigned to the user (`--no-assign`
  leaves it unassigned), with `--reviewer <login>` (repeatable) asked to review it: the `github_login` of the
  suggested `review.groups[].reviewers`, once the user chose them. `pull_request` has `created: true`, `reviewers`
  and `assignees` as GitHub took them and `warnings` for what it refused; or the open one of the branch, whatever its
  base, also one that already existed, left as it is (a warning says so): one into a virtual branch is retargeted to
  the target first (`retargeted`), one into another base is taken with a warning that Patronus merges it there; no
  robot starts. The GitHub token is the first of `GH_TOKEN`
  (`GITHUB_TOKEN`), `gh auth token`, the app's GitHub sign-in (Settings) and the git credentials of github.com;
  without one GitHub accepts it is `not_logged_in`. The
  GitHub repository is the mirror of the Space settings or `branchdeck.githubRepository`; without one it is
  `invalid_arguments`, and `approvals_missing` says how to set it. The pre-check takes the pull request the robot
  starts from, and Patronus judges its approvals as that robot would: `review.readiness` is `missing` (refused),
  `approved`, `not_required`, or `unknown` while Patronus doesn't evaluate pull requests and the pull request has an
  approval (the start goes on: the robot decides). `review.waiting_for` names the code owners asked who haven't
  approved, `review.not_counted` the approvals Patronus doesn't count (a bot), `review.approved_on_github` the ones
  that count. A start through the API whose merge profile is known to skip the code-owner review (the last robot's,
  or `--profile`) is not refused for `missing` approvals, as the app's start is not: `review.skipped_by` is `profile`.
  `--without-approvals` starts anyway, only when the user says so (`review.skipped_by`: `without_approvals`). A branch
  of more than 500 commits is not checked (`review.skipped`).
- The branch is pushed first, as `bd push <branch>` pushes it (`pushed`; the branches it is built on that have
  commits for the remote go along, parents first, with the lease and the refusals of `bd push`): only its witness and
  freeze run under the repository lock, the sync and the checks before it, the start after it. A start is never
  repeated.
- bd closes no pull request after the merge. The app closes the open pull requests of the ancestors a merged robot
  landed, with a comment, when it sees the robot merge ([Patronus](../../spec/branch/patronus.spec.md#merged)); a
  robot bd started is seen too while the app runs. Otherwise they stay open: the agent tells the user.
- A start through the API (`started_via: api`) starts the robot on `refs/heads/<prefix>/virtual/<branch>`, kept
  after the merge. Branches reviewed on GitHub (the mirror of the Space settings, or `branchdeck.githubRepository`)
  with an open pull request into the target start from it (`started_via: pull_request`). One into a virtual branch, of
  the branch or of a branch its push takes along, is retargeted to the target first, before anything is pushed
  (`retargeted`, as `bd push` lists them; `--check` lists the ones a start would retarget): a refused retarget of the
  branch's own fails the start, nothing pushed or started; one of a branch taken along is a `retarget_warnings` line.
  One into another base starts through the API. For a start from the pull request bd posts
  `/patronus merge --keep-source-branch` (`/patronus dry-run`) through the GitHub API, waits up to a minute for the answer of
  Patronus, and finds the robot, so the approvals of the pull request count (`review.counts_in: pull_request`). A
  comment can't choose a profile: `--profile` is `invalid_arguments` there. `pull_request.reply` is `pending` when
  Patronus didn't answer within the minute (the robot is `null`, and `start_unknown` says the start is not known yet);
  a rejected comment or a draft pull request is `patronus_refused`. A start whose request went out and whose answer was
  lost (a timeout, a connection that broke off) is `start_unknown` (`operation`, `reason`), never sent again: its record
  holds the branch until Patronus lists its robot or `bd operation forget <operation>`.
- `robot_running` (exit 4): a robot runs on the last commit of the branch ("A Safe Push of the last commit is
  running"), a Safe Push lands it with a branch of its stack ("A Safe Push of oauth is running: it lands login with
  it"), or a Safe Push of a branch it is built on runs, whose commits it would land a second time ("A Safe Push of
  auth is running: wait until it merges"). A running Safe Push of an older commit fails once the push changes its
  branch: `--cancel-running` cancels it first (`canceled`). A Dry Run replaces no Safe Push: its push into the branch
  of a running Safe Push is refused, `--cancel-running` or not; one without a push starts beside it.
- `landed` (exit 4, the first of `refusals` with `--check`): a merged Safe Push landed all of the branch ("Nothing was
  pushed or started: login merged into master with its Safe Push (<robot>): nothing of it is left to land"), or, with
  `--whole-stack`, every branch of the stack ("every branch of the whole auth stack landed in master with the Safe
  Pushes of login and session"). `lands` leaves out the branches merged Safe Pushes landed all of: the robot lands the
  rest (the branch itself is always named). The same refusal for a branch the fetched baseline has all of ("auth is in
  origin/master: the update archives it") and for one whose pull request GitHub merged with the commits it has ("pull
  request #7 of auth is merged"): one rule with the app (`domain_policy::landed::landed_state`); `bd update` picks it up.
- `dangling_reference` (exit 4, nothing pushed): the branch or a branch it is built on removes, changes or declares
  again a declaration whose use (or other declaration) a commit of another branch changes, as `bd check` reports it
  (`dangling_references`, with its `kind`); the message
  names the use, the commits and the fix, of two references both, from three on of the first, with the count of the
  rest ("(2 more: bd check)", which lists them). `--check` lists it in `refusals`; `--allow-dangling` starts anyway, when
  the user says it is a false alarm. A possible one (`bd check` `warnings`) is in `warnings` and refuses nothing.
- `missing_declaration` (exit 4, nothing pushed, `branchdeck.symbols on`): a commit of what lands uses a name that a
  commit outside it adds, which no merged Safe Push landed (`missing_declarations` of `bd check`; a name of a branch of
  the landing stack is none). The landing gate (`trial_check::landing_gate`, the same rule as the app's start) checks it
  in the trial session of the dangling references. The message names the use, the commit that adds the name with its
  branch, and the fix: `bd restack <root> <branch>` lands that branch too (after the user approves); with stacked
  branches off, an unassigned commit or a restack that would close a cycle, the commit to move. `--check` lists it in
  `refusals`; `--allow-missing` starts anyway, when the user says it is a false alarm.
- `push_dismisses_approvals` (exit 4, nothing pushed or started): the start pushes new commits into the pull request
  the robot starts from, which someone approved, and the GitHub rules of the target dismiss stale reviews on push
  (one `GET rules/branches/<target>`, as the app asks): the approvals Patronus counts would be gone and the robot
  fails. Push first (`bd push <branch>`), then `bd request-review <branch>`. Approvals Patronus doesn't count are only
  said in `rules.message`. `rules_unknown` (exit 4): GitHub didn't answer the rules (no token, no access, an error);
  `--allow-unknown-rules` starts anyway when the user says so. `rules` is `{dismiss_stale_reviews_on_push: true | false
  | null, message}`, absent when they were not read; `--check` lists either refusal in `refusals`.
- `review_unknown` (exit 4, nothing pushed or started, also in `refusals`): Patronus couldn't answer the code-owner
  review (its pre-check failed; `review.readiness` `unavailable`), which is never "approved" (D14);
  `--allow-unknown-review` starts anyway when the user says so, and the robot checks the approvals when it starts.
- `pull_request_other_base` (exit 4, nothing pushed or started): the open pull request of the branch goes into another
  base than the target (a release branch, `domain_policy::pull_request::base_of`), and Patronus merges a pull request
  into its base: its approvals count for no Safe Push into the target. The next step is the `gh pr edit <n> --base
  <target>` retarget, the user's to approve; `--without-approvals` (a start through the API) only when the user says so.
  A Dry Run and a profile that skips the review count no approval and start. `--create-pull-request` refuses it too.
- `patronus_refused`: the target is not protected by Patronus, the pull request is a draft, or Patronus rejected the
  comment; `patronus_error` (exit 5): a request to Patronus failed, `message` is its answer.

### `bd land <branch> [--whole-stack] [--check] [--without-checks] [--bypass-rules] [--allow-dangling] [--allow-missing]`

Lands the branch into master by the route of the repository (`branchdeck.landing`,
[Landing Routes](../../spec/branch/landing-routes.spec.md)), `route` in the output:

- `patronus`: the Safe Push of `bd safe-push`, its whole output in `safe_push`.
- `push`: the sync without the repository lock, then the plan of the app's start over the one-shot session of the
  command (the pull requests with `viewerPermission`, the rules of the target, the landing gate of the subject), then
  its run through the engine: the witness under the lock, the push of the copy (`commit`: the virtual branch of the
  piece, the stack branch of a whole stack) into `target` by the push URL after it, a fast-forward
  ([Push into the baseline](../design/git-operations.md#push-into-the-baseline)). `--check` pushes nothing and prints
  `lands`, `confirmations`, `next_step` and `refusals`.
- `pull-request`: the same frame, GitHub merging the open pull request of the branch (`pull_request`) by the first
  method the repository allows of rebase, a merge commit and squash, while it shows the commit the copy is
  (`expectedHeadOid`); while its required checks run, and after the push of the branch when it shows another commit,
  the auto-merge instead (`auto_merge: true`: GitHub merges it once they pass, nothing landed yet). On Space this route
  is the Safe Push; a repository off GitHub or a mirror is `landing_route_unsupported`. Refusals: `no_pull_request`
  (none open into the target), `merge_blocked` (approvals, required checks, changes requested, a draft),
  `not_mergeable` (a conflict, a head behind its base: `bd update` then `bd push`; GitHub's refusal in its words).

A push into master lands without a robot and without the CI of a pull request: what CI did not pass on the commit
and the changes requested on its pull request refuse it as `checks_not_passed` until `--without-checks`; what the gate
of the route asks (a rule a maintainer may bypass, the required status checks, a refusal of the server before) refuses
it with the code of its reason until `--bypass-rules`. Both flags are the user's word, never the agent's. Below write
access it is `no_write_access`, a rule no push passes `baseline_refused`, a server that refused it `baseline_refused`
with what the server said, a target that moved `baseline_moved` (exit 3; master is fetched, `bd update` comes next),
and so is a fetched master with commits the branch is not built on, before anything is sent.
It never updates: a push that landed, and a merge, print `landed: true` and `next: "bd update"`; a push `closes`: the open pull requests
of what it landed that show an older version of a branch, which the app closes with a comment (`bd` closes none; one
whose head the push put into master GitHub merges by itself). Name them to the user.

### `bd request-review <branch> [--reviewer LOGIN]… [--push] [--check]`

Asks GitHub users to review the open pull request of the virtual branch, as the app's Request Review… does
([Pull Requests](../../spec/branch/pull-requests.spec.md#request-review)): the same intent of the engine
(`RequestReview`, `operations::pull_request`), planned over the one-shot session of the command. Patronus counts the
approvals of the code owners in that pull request, so one that asks no member of a missing group waits for nobody.

```json
{"schema":2,…,"branch":"auth","pull_request":{"number":7,"url":"https://github.com/acme/monorepo/pull/7"},
 "readiness":"missing","reviewers":[{"login":"ikorennoy","full_name":"Ilya Korennoy"}],
 "requested":[{"login":"ikorennoy","full_name":"Ilya Korennoy"}],
 "waiting_for":[{"login":"ikorennoy","full_name":"Ilya Korennoy"}]}
```

- Without `--reviewer` it asks Patronus as `bd safe-push` does (the pre-check with the pull request when it merges into
  the target, asked again; the Space token of the Safe Push CLI, else `not_logged_in`), and asks one suggested code
  owner with a GitHub login of each missing group nobody is asked for yet, each once: never the author, a member asked
  already or one whose approval counts. A group someone is asked for adds nobody, so a second run asks nobody.
  `--reviewer <login>` (repeatable) asks those instead and doesn't ask Patronus (`readiness: null`).
- `reviewers` is whom it asks; `--check` prints the same object and asks nobody (`requested` empty): the agent shows it
  to the user and asks only after the user approves. `requested` is whom GitHub asked, `refused` whom it refused with
  its reason (the author, a user without access; a refused list is asked one by one to tell), `waiting_for` the
  reviewers the pull request asks after the request.
- It syncs first, without the repository lock (`unknown_branch` for a branch the sync has not), so it knows whether the
  branch has work its remote lacks: then the reviewers would not see the commits, and it is `needs_push` (exit 4)
  with the next step `bd request-review <branch> --push`, the user's to approve. `--push` pushes them first as the app's
  request does (the ancestors with work first; an open pull request of what it pushes into a virtual branch is
  retargeted into the target before the push), `pushed` naming the branches, then asks; `--check` names what it would
  push. The writes take the lock only for the witness and the freeze of the push: the push, then
  `POST pulls/<n>/requested_reviewers`, users only (no team).
- `no_pull_request` (exit 4): no open pull request, or a draft; `bd safe-push <branch> --create-pull-request` opens
  one with `--reviewer`. `no_reviewer` (exit 4): a missing group nobody is asked for has no code owner with a GitHub
  login Patronus names, and nobody else is to ask: ask the user whom, then `--reviewer`. `review_refused` (exit 4):
  GitHub asked nobody; the message names each with the reason.

### `bd robot <branch> [--full] [--robot ID] [--whole-stack]`

Prints the Patronus robot of the virtual branch, as `bd safe-push` finds it: a running one first, else the newest of
the user's robots of the last 7 days, started on the branch or on a temporary ref at its local or pushed tip; with
`--robot`, that robot ([Safe Push Failure Prompt](../../spec/branch/safe-push-failure-prompt.spec.md)). It is how an
agent watches a robot the user started or restarted (`running`: run it again later), and, after a failed Safe Push,
learns why and which local commits it tested. Read only: no repository lock, no sync, no ref written. The Space
token and the repository on Space are needed as for `bd safe-push` (`not_logged_in`, `not_on_space`).

```json
{"schema":2,…,"branch":"auth",
 "robot":{"id":"r1","name":"Add login","state":"failure","mode":"safe_push","commit":"1b2c3d4e",
   "url":"https://patronus.labs.jb.gg/robot/r1","running":false,"profile":{"id":"lsc","name":"Large Scale Change"},
   "target":"master","started_at":1791970200,"finished_at":1791973800},
 "failure":{"kind":"checks","test_revision":"9f8e…","target_revision":"4a5b…",
   "checks":[{"name":"Fast Tests","build_configuration_id":"Fast_Tests","attempt":1,"attempt_limit":1,"build_id":"100",
     "build_url":"https://ci.example/build/100","failed_tests":3,"failed_builds":2,"failed_to_start":0}],
   "problems":[{"title":"Changes require review approval","subsystem":"OWNERSHIP_REVIEW_APPROVAL_CHECK"}],
   "builds":[{"name":"Compile","project":"IntelliJ / Build","build_id":"300","build_url":"https://ci.example/build/300",
     "problems":[{"kind":"TC_COMPILATION_ERROR","details":"Foo.kt:12: unresolved reference: bar"}],
     "log_tail":"…"}],
   "omitted":{"builds":0,"tests":0,"messages":0,"truncated":false}},
 "commits":[{"commit":"5c6d7e8f","subject":"(auth) Fix login","author_time":1700000900,"branch":"auth","state":"added"},
   {"commit":"2b3c4d5e","subject":"(auth) Add login","author_time":1700000100,"branch":"auth","state":"tested"}],
 "head_branch":"master"}
```

- `robot` is `null` (exit 0) when the branch has no robot; a name that is neither a branch ahead of the baseline nor
  a branch with a robot is `unknown_branch`. `--robot` of a robot of another branch is `invalid_arguments`, naming it,
  unless that branch is stacked on this one.
- A newer robot of a branch stacked on this one wins over the branch's own: it built this branch too, and
  `robot_branch` names its branch. So does a newer robot of the whole stack the branch is a branch of, named by
  `stack_root`; `--whole-stack` prints the robot of the whole stack, as `bd safe-push --whole-stack` starts it (of a
  straight chain, the robot of its last branch). `--robot` of a robot of the whole stack is that of a branch of it;
  with `--whole-stack`, only the whole stack's. A failure of the whole stack's robot lists the `commits` of every branch
  of the stack, each with its `branch`; the stack branch has no mapping notes, so they are matched by identity.
- `failure` is there when the robot failed, or runs while a required check failed already (Patronus' "will fail",
  or a running check with a failed build). `kind`: `checks`; `infrastructure` (every failed check only failed to
  start its builds: the user restarts it, nothing to fix); `massive` (Patronus detected a massive failure: the branch
  may break a core module, or the target is broken); `patronus` (no check failed, Patronus reports an error such as
  a conflict of the rebase onto `target_revision`: `bd update`); `unknown`. `checks` are the failed checks by their
  last attempt (`running` for a running one), `build_id` the TeamCity build id (`teamcity run log <build_id>`);
  `problems` those of severity error. `test_revision` is the commit TeamCity built (the robot's commit rebased onto
  `target_revision`), a full id and not a local commit.
- `--full` adds `builds`: the failed builds of the failed checks (never a composite one), those that failed not only
  on tests first, with their problems and failed tests (`message`, `first_failed_before` when the test failed in an
  earlier build); through the `teamcity` CLI, when it is installed and logged in to the server of the build, the
  `log_tail` of a build that failed not only on tests. A few seconds; `omitted` counts what the limits left out (5
  builds, 40 test names, 20 messages).
- `commits`, newest first, are the commits of the branch against those the robot tested, by the ids of the
  checked-out branch (`head_branch`), as the other commands print them (`--full-ids`): `tested`, `rebased` (a copy
  on a newer baseline with the same changes), `changed` (amended since), `added`, `removed` and `landed` (the robot
  had it, the branch no longer has it, or the baseline has it now; no `commit`). `source_missing`: the robot's commit
  is not in the repository, every commit is `unknown`. `virtual_behind_head`: the checked-out branch has commits of
  the branch that the last sync didn't build; they are not listed. The map compares the virtual branch as the last
  sync built it, the same map as the app's Copy Prompt for Agent. The robot of a stacked branch builds the whole
  stack: the map has the commits of its ancestors too, and `branch` says whose each commit is (a failure may come from
  a parent's commit).
- The agent fixes and checks locally; the user pushes and restarts the Safe Push.
- `closes` (a merged Safe Push): the open pull requests of the branches it landed whole besides its own, each holding
  nothing the branch lacks, never the one it started from (`[{"branch":"auth","number":5}]`): the app closes them with
  a comment, by the same rule (`domain_policy::landed::pull_requests_to_close`); `bd` closes none. Compared with the
  tips the refs hold, no sync; left out for a robot that didn't merge, off GitHub and while GitHub doesn't list the
  pull requests.
- `patronus_error` (exit 5): a request to Patronus failed, `message` is its answer.

### `bd operation forget <op>`

A start whose answer was lost (`busy` with `error.holder.unknown`, or `start_unknown` of a `bd safe-push`) holds its
branch in the operations registry of the repository until Patronus lists its robot. `bd operation forget <op>`, only
when the user says so, takes its record out (`operations::forget`, the core of Forget This Start of the app), so the
branch can start again; Patronus is not asked and may still run the robot it sent (`bd robot <branch>` shows it once
listed). Output `{schema, repository, forgotten, subject, source_ref}`; `invalid_arguments` (exit 2) for an id the
registry lacks (it ended, or a read of Patronus settled it) or of an operation whose process still runs it; `locked`
when another process keeps the lock of the registry.

### `bd describe <branch> [--title T] [--body B | --body-file F] [--clear]`

Prints or sets the pull request text of a branch in git's own `branch.<prefix>/virtual/<branch>.description`, stored
as `title`, a blank line and the body, as `git branch --edit-description` writes it. Without options it prints
`{…,"branch","ref","description":{"title","body"}|null,"changed":false}`. `--title` or `--body` replace only that part;
`--body-file -` reads stdin; `--clear` removes it. The branch must be a current group. The `descriptions` of a
`bd group` plan set the texts of several branches in the same call as the rewrite. The app shows the title after the
branch name, in the branch table and the branch preview.

### `bd schema`

Without a name it prints the index `{"schema":2,"commands":{"<command>":["<output>",…]}}` (under 1 KB), because every
schema together is about 350 KB, some 90k tokens that an agent would read for one field. `bd schema <name>...` prints
`{"schema":2,"outputs":{"<output>":<JSON Schema>,…}}` of those commands or outputs, `--all` of every output, each named
by the test `schema_lists_every_output`. `COMMAND_OUTPUTS` in `schema.rs` is the index: `rewrite` is the output of
`group`, `rename`, `restack`, `unstack` and `reword`, `commits_brief` and `status_brief` the outputs of `--brief`,
`update_status` and `update_abort` those of `bd update --status` and `--abort`, `lease_get` … `lease_clean` those of
`bd lease`; `error` is the error of any command. An unknown name is `invalid_arguments` with every name.

### `bd workspace`

Run by an agent before its first edit or commit (the skill says so; the contract
[bd workspace](../../spec/cli/workspace.spec.md)). Read only: `lease_pool::agent_workspace`, one `rev-parse`, one
`git config` of the `branchdeck.*` keys, in the main worktree one more of the pool config, in the main worktree or a
lease one `status` of the shared checkout, and in a lease one more `rev-parse` of the branch the shared checkout has
checked out now. Two rules of the git config, each `{on, source, invalid, reason}`:

- `lease`: `branchdeck.agentLease`, on by default. Work in a lease of the pool, land each commit that builds and passes
  its tests at once with `bd lease pick`, return the lease when done. Off with a `reason` in a linked worktree outside the
  pool (a checkout of its own already), outside a working tree, and when the pool directory can't be written (a
  sandbox; a probe file in its first existing parent, removed at once).
- `subject_branch`: `branchdeck.agentSubjectBranch`, on by default. Start each commit subject with `(branch) `: the
  prefix groups the commit into its branch, and the copies of the virtual branches drop it. A repository whose commits
  land as they are turns it off, since there the prefix reaches the remote.

`source` is the scope that sets the rule (`local`, `worktree`, `command`, `global`, `system`) or `default`; the user
sets the global value and the repository's own in Settings → Agents, or with `git config [--global]`. `kind` is
`main`, `linked`, `lease` or `none`; `subdir` where bd runs below the top level; `shared_checkout` the checkout the
commits land on (the main worktree, or the source of this lease); in a lease `home` as `bd lease get` prints it, with
`switched_to` when the shared checkout has another branch checked out now (`null`: detached); `lease_id` and `holder` this slot's lease;
`local_changes` the number of local changes of the shared checkout, which a lease lacks; `active_leases` every ready
lease of the pool (`lease_id`, `holder`, `path`, `holder_alive`, `idle` for one whose holder is gone); in a slot taken
over from a dead holder, another session's `bd workspace` is `lease_reclaimed`; `next` the commands to run, in order:

```json
{"schema":2,"kind":"main","lease":{"on":true,"source":"default"},"subject_branch":{"on":true,"source":"default"},
 "stacks":false,"top_level":"/work/app","subdir":"crates/x","shared_checkout":"/work/app","local_changes":1,
 "active_leases":[],"next":["bd lease get --holder <your name> -C /work/app"]}
```

Inside the lease `next` is `bd -C <slot> lease pick` and `bd lease return --lease-id <id>`, as `bd lease get` printed it.

### `bd version`, `bd --version`

Both print `{"schema":2,"version":"0.11.0","git":"2.56.0"}` and need no repository. `version` is the version of
Branch Deck, the one the app shows: `build.rs` of `crates/bd-core` (bd-cli runs it too) reads it from the root `package.json`, which
release-please updates and `src-tauri/tauri.conf.json` names (`"version": "../package.json"`). The workspace version of the crates stays 0.1.0, so `CARGO_PKG_VERSION` is not it. `git` is
the version of the git bd finds (see Invocation), also of one older than 2.54, and `null` without one: never an
error. The first line of `bd help agent` names the version too.

### `bd install [--uninstall]`, `bd help skill`

How an agent learns that bd exists. `bd install`, or Help → Install Agent CLI… in the app
(`install_agent_cli` of `crates/app-api/src/app/agent_cli.rs`, through `src-tauri/src/agent_cli.rs`: the same code), writes:

- on macOS and Linux a `bd` symlink in `~/.local/bin` to the app binary, which runs the CLI under that name. A symlink
  there is replaced; a file that is not a symlink is `install_refused` (exit 4). On Linux an app run from an AppImage
  (`APPIMAGE` is set) is linked by the AppImage file, not by the binary in its mount under `/tmp/.mount_*`, which goes
  when the app quits. On macOS an app that Gatekeeper translocated (started from Downloads or a disk image, its path
  contains `/AppTranslocation/`) is `install_refused`: "Branch Deck runs from a temporary location: move it to
  Applications, then install the Agent CLI". On Windows `bd.exe` is next to the app already, and a symlink would need
  developer mode;
- the skill `branch-deck` (`SKILL.md`, the Agent Skills format) into `~/.claude/skills/branch-deck/` (Claude Code) and
  `~/.agents/skills/branch-deck/` (Codex and others). Its description tells an agent when to use bd (`bd help skill`
  prints it), first of all before the first edit or commit in a Git repository, for `bd workspace`; then grouping or
  splitting commits into branches or pull requests, stacked branches, fixups, a change in the wrong commit, an update
  or rebase onto the baseline and its conflicts, landing the commits of a worktree onto a shared branch, the lease of a
  worktree, a Safe Push or Dry Run and why it failed, a review request, Branch Deck. The description stays within the
  1024 characters of the Agent Skills format. Its body names bd by its full path, so it works without `PATH`; it starts
  with `bd workspace` and what each of its rules asks (the lease: work only in its `path`, land each commit at once,
  return only its own lease; the subject branch: `(branch) ` first, else the subject as the repository writes it) and
  the isolation rules a repository would otherwise repeat (no `git worktree add` or clone of the agent's own, also
  when bd fails). The rule of the lease names no agent. A note for each kind of agent follows it: in Claude Code a
  `cd` into `path`, else `EnterWorktree` with it; in a Codex sandbox, write access to exactly the lease's `path`; with
  a shell that does not keep the current directory (Junie, for example), an absolute path in `path` for each file
  tool and `cd <path> && ` before each shell command. The body then sends the agent to `bd help agent` for grouping, pushing
  and Safe Push, to `bd help update` for an update, to `bd help worktrees` when several sessions or worktrees work
  on one branch, and to `bd <command> --help` for the outputs and refusals of a command; it asks for
  `bd safe-push <branch> --check` before a Safe Push, as the Safe Push of a stacked branch lands its parents too, and
  keeps the rules an agent must not miss before it reads any help (approval, no reorder, `bd pick`, `error.code`,
  parked, unapply). The workflow and the fields stay in the binary, so the skill can't fall out of date with it. Its `allowed-tools` is `Bash(<bd>:*)`: while the skill is active, Claude Code
  runs every bd command without a permission prompt, also `bd group`, which rewrites the checked out branch, and
  `bd push`. The approval of a push or a Safe Push is the rule of the skill's body, not a prompt.

It prints `{"schema":2,"bd":"/Users/me/.local/bin/bd","on_path":true,"skills":[…],"uninstalled":false,
"agents":{"lease":true,"subject_branch":true}}`. `agents` is what `bd workspace` answers in a repository without its
own value (one `git config --global`), which the app's toast tells the user. `on_path`
says whether `bd` on `PATH` is this one; the app asks the user's login shell for `PATH`, since an app started from the
Finder has a minimal one. `--uninstall` removes the skills and the symlink if it points at this app (or at a removed
`branch-deck`). `bd help skill` prints the skill as text, for an agent that reads skills from elsewhere. Without the
symlink it names the app binary with `cli` (`'/Applications/Branch Deck.app/…/branch-deck' cli`), quoted for the shell
and to be run as written: the bare app binary starts the app.

The app's prompts name bd the same way without an install: the Tauri command `agent_cli_command(repository)`
(`bd_command` and `for_shell_in` in `install.rs`) returns `<bd> -C <repository>`, with the `bd` symlink when it points
at the running app, else the app binary with `cli` (on Windows `bd.exe` next to the app; a binary named `bd`, such as
a cargo build, needs no `cli`). It is quoted for a POSIX
shell, or for PowerShell on Windows (`& '<bd.exe>' -C '<repository>'`, single quotes, in which `$` does not expand, `powershell: true`). It writes nothing, so the
bd of a prompt is always the version of the app that copied it. When the app can't tell its binary (no `bd.exe`), the
blocked prompt names no command: the user updates in the app.

### `bd help agent`

Prints the agent workflow as plain text (`AGENT_HELP`, a core of at most 10 KB, about 2,500 tokens): the rules first,
the refusals that wait for the user with the flag only the user's word allows, then the steps, then the other tasks.
Every agent reads it before grouping or pushing, so it carries only what an agent must know before it acts; a step
sends the agent to the `--help` of its command for the fields of the output and the other refusals. It only names the
resolve session, which `bd help update` prints (`UPDATE_HELP`), and the work beside other sessions, which
`bd help worktrees` prints (`WORKTREES_HELP`). Agents act on the error they get and seldom read a `--help` first: so a
rule that waits for the user stays in the core and in the message of the error, never only in a `--help`.

`agent_help_test.rs` keeps the texts true. Together with the `after_long_help` of every command they must name every
command and, for each command, every long option, unless the test lists it in `NOT_IN_AGENT_HELP` (as
`status --limit`). An option counts for the command named last before it (in the `--help` of a command, that command
before the first), and it must exist for that command. `documented_in`, an exhaustive match over `ErrorCode`, places
every code: the core, a topic or the `--help` of a command must name it.

### `bd <command> --help`

The text after the options (`after_long_help`, constants beside `AGENT_HELP`) of `status`, `suggest`, `check`, `group`,
`push`, `safe-push`, `request-review`, `robot`, `robot-stats`, `fold`, `squash`, `reword` and `move-hunk`: the fields of the
output and what each refusal asks. `-h` ends with one line that names `--help`; `bd help <command>` prints the same
as `--help`, because an agent tries it first.

### `bd help worktrees`

Prints how agent sessions that share a checkout work beside each other, each committing in a worktree of its own
(`WORKTREES_HELP`, about 55 lines): the lease of that worktree with `bd lease get` when `bd workspace` says
`lease.on`, and its return; the `(branch)` prefix, when `bd workspace` says `subject_branch.on`, is written when a
commit is made, never guessed later; one
owner rewrites the shared branch (`bd update`, `bd fold`, `bd group`, `bd rename`) and every other session only adds
commits with `bd pick --from <worktree> --follow`, never a cherry-pick in the shared checkout; the outcomes of a pick
and what to do with each; `bd pick --dry-run` as the check after a rewrite, never a comparison by date; the protocol
of the owner (the lock, `operation_in_progress`, the new HEAD told to the others); and the `branch-deck: <operation>`
reflog lines. The skill sends an agent there when several sessions or worktrees work on one branch.

## Agent workflow

The workflow is `bd help agent` (`AGENT_HELP` in `crates/bd-cli/src/agent_help.rs`): the rules first (what needs the
user's approval, what each exit code asks for, when to retry, how to undo), then the steps from `bd suggest` to the
robot of a Safe Push, then the other tasks (`bd update`, `bd pick`, `bd fold`, `bd squash`, `bd move-hunk`). It is the one text an
agent follows, versioned with the binary and checked by `agent_help_test.rs`; this guide is its reference and does not
repeat it. `bd help update` and `bd help worktrees` hold the resolve session and the work beside other sessions, the
`--help` of a command its outputs and refusals.

What an agent reads, by cost: the skill (about 5 KB, loaded whenever it triggers), `bd help agent` (at most 10 KB,
before grouping or pushing), then only the `--help` of the command it runs, or the topic of its task; `bd schema
<command>` for the exact fields. Nothing in this guide is installed.

The app's Update Project hand-off prompt (`src/lib/agent-prompt.ts`) states the same rules for an agent that rebases:
no reordered, squashed or dropped commit, no push, no stash. For an update blocked by uncommitted changes it sends the
agent to the user about each file and then to `bd update --no-fetch`; for a conflict, to `bd update --no-fetch` and
its resolve session; without a bd command, to the git steps, which ask the user about uncommitted changes instead of
stashing them.
