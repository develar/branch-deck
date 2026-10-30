# Adding a Git Operation

**Status:** Active  
**Updated:** 2026-10-26  
**Tags:** [git, rust, bd, tauri]

## Summary

Read this before you add or change a git operation in `crates/git-ops`, `crates/history-rewrite`, `crates/sync-core`,
`crates/update-core`, `crates/trial-check`, `crates/lease-pool`, `crates/remote-facts`, `crates/repository-model`,
`crates/operations`, `crates/bd-*` or `crates/app-api`. The checklist is [below](#checklist), the hard rules
are [Git Rules](git-rules.md), the flows and the reasons in [Git Operations](../design/git-operations.md).

## Checklist

- One name across the layers (`git_ops::move_hunk`, `trial_check::move_hunk`, `bd move-hunk`); the `//!` headers of
  `crates/git-ops/src/lib.rs`, `crates/history-rewrite/src/lib.rs`, `crates/sync-core/src/lib.rs`,
  `crates/update-core/src/lib.rs`, `crates/trial-check/src/lib.rs`, `crates/lease-pool/src/lib.rs`,
  `crates/remote-facts/src/lib.rs`, `crates/repository-model/src/lib.rs` and `crates/operations/src/lib.rs` map the
  modules by area.
- **git-ops** has the git: one process per phase, `-z` output, full ids, `--end-of-options` before a revision from
  outside, `--literal-pathspecs` for paths after `--`; no lock and no policy.
- **The core** `<op>_core` in `crates/history-rewrite/` (a rewrite that needs no sync, GitHub or Patronus),
  `crates/update-core/` (Update Project, `bd pick`, the resolve sessions), `crates/trial-check/` (the trial checks,
  `bd move-hunk`, whatever reads the diffs or the symbols of the commits), `crates/lease-pool/` (`bd lease`,
  `bd workspace`), `crates/remote-facts/` (what GitHub and Patronus answer over HTTP, and the writes to them),
  `crates/repository-model/` (the branch table Rust derives, the guards of the app's push and robot actions) or
  `crates/sync-core/` (the sync, the pushes and the rest, with no remote service but git's) is the one
  entry of every caller: a rewrite of the checked-out branch
  takes `&RewriteOptions`, reads and computes on the oids it read once (checking the expected HEAD there), then takes
  the lock for the witness of those refs and the CAS write (`RewriteOptions::lock_prepared`).
- **operations**: an action on the remote, a rewrite of the checked-out branch and Update Project are an intent of its
  family in `crates/operations/` (`prepare` and `execute` of `engine`; a rewrite and an update are `execute` alone): it
  runs in `engine::run`, admitted by `registry` over the resources it names (`domain_policy::admission`), its steps the
  `operations` part of the view, and checks its `witness` before the first effect (`plan_changed`). A write
  to Patronus, GitHub or YouTrack and the leased push are effects of a plan, called only from `crates/operations`
  (`remote_write_scan_test.rs`, exact counts per file). What
  its plan reads of the remote is `needs::needs(intent)`, read by `facts::fill` through the one reader of every host
  (`remote_facts::service_sources::ServiceFactSources`) into its session; `bd`'s is a one-shot session of its sync
  (`repository_model::one_shot::OneShot`, its services `bd_core::services::BdServices`), never a reader of its own.
- **bd** and **the app** parse, call the core and present its result; no git of their own. A `bd` command answers one
  JSON object with `schema`, errors as `CliError` with an `ErrorCode`, and records a rewrite, an update, a pick and an
  operation of the remote in the operations registry (`bd_core::operation_record`); a command of the app is one entry
  of the registry of app-api (`crates/app-api/src/commands.rs`): a read `run_blocking` of the core, a write `execute`
  of its intent or a command of the app, `app(<reason>)` (`AppReason`: what has no repository as its subject, or what
  has a writer of its own that no session reads, as the reviewed refs); the
  app's Tauri wrapper and the E2E server's route come from the entry and run the same body.
- **Reuse a helper** before writing a parser or a process: `ref_transaction`, `commit_writer`, `commit_chain`,
  `cat_file_batch`, `object_texts`, `history`, `rewrite_guard` and `revs` in `git_ops`; `snapshot`, `repository_lock`
  and `regroup` in `history_rewrite`; `repo_facts`, `baseline` and `remote_command` in `sync_core`.
- **Same commit**: the spec, the flow's section in [Git Operations](../design/git-operations.md), and a test in
  `process_budget_test.rs` that pins its processes by subcommand; a core that takes the repository lock, its row of
  [Lock spans](../design/git-operations.md#lock-spans) and the test of its span; the scan tests stay green, the
  concurrency scan beside the git scans (`concurrency_scan_test.rs`: no guard of an async mutex across another await,
  no repository lock across a network call).

## Layers

An operation keeps one name across the layers (`git_ops::move_hunk`, `trial_check::move_hunk`, `bd move-hunk`). The `//!`
headers of `crates/git-ops/src/lib.rs`, `crates/history-rewrite/src/lib.rs`, `crates/sync-core/src/lib.rs`,
`crates/update-core/src/lib.rs`, `crates/trial-check/src/lib.rs`, `crates/lease-pool/src/lib.rs`,
`crates/remote-facts/src/lib.rs`, `crates/repository-model/src/lib.rs` and `crates/operations/src/lib.rs` map the
modules by area; the layers are the
[Architecture](../design/git-operations.md#architecture) of Git Operations.
- **git-ops** (`crates/git-ops/src/<op>.rs`): the git, functions of `(git, repo_path, …)`, one process per phase (many
  ids or paths on stdin, `cat-file --batch`, `fast-import`, one `RefTransaction`), through `git.cmd(&args, repo)` with
  one finisher, or the shorthands `execute_command`, `execute_command_lines`, `execute_command_with_status`. Read `-z`
  output (paths as bytes), take full ids (`is_object_id` for an id from outside), put a revision from outside after
  `--end-of-options` and a path after `--` as a literal pathspec (`--literal-pathspecs` first in the argument list).
  A rewrite of the checked-out branch runs the guards on the history it read (`History::check_rewrite`;
  `check_rewrite_range` for a range it did not read) and moves the branch with CAS on the HEAD it read. Reuse a helper
  below before writing a parser; no lock and no policy here.
- **The core** (`crates/history-rewrite/src/<op>.rs` for a rewrite of the checked-out branch that needs no sync, GitHub
  or Patronus; `crates/update-core/src/<op>.rs` for a move of the checked-out branch onto commits from elsewhere: Update
  Project, `bd pick`, the resolve sessions; `crates/trial-check/src/<op>.rs` for a trial check, `bd move-hunk` and
  whatever reads the diffs or the symbols of the commits; `crates/lease-pool/src/<op>.rs` for the lease pool and the agent
  workspace; `crates/remote-facts/src/<op>.rs` for a read or a write of GitHub or Patronus over HTTP, with the git it
  needs through sync-core and git-ops; `crates/repository-model/src/<op>.rs` for what Rust derives of the branch table
  and the guards of the app's actions on the remote (the push, the start of a robot), on what the others read; else
  `crates/sync-core/src/<op>.rs`: the sync, the pushes and the other cores, which reach no remote service but through
  git): `<op>_core(git, params, …)`, the one entry of every caller. A rewrite
  of the checked-out branch takes `&RewriteOptions`: it reads the history or the snapshot once and computes on those
  oids (the expected HEAD checked there, `ensure_head`; merge-tree, replay, the `fast-import` of the new objects on
  temporary refs) before the lock, then `options.lock_prepared(..)` takes the lock for the witness of the refs it read
  (`Prepared::witness`: HEAD, its branch and the store tips, one `for-each-ref`), computes once more under the lock when
  one moved, and the core writes under it: the CAS transaction, then the move of the index. It passes what it computed
  as a `Result` (`options.lock_prepared(git, repo, prepare(), prepare)`): HEAD that another process moved between two
  reads of the computation is its `HeadMoved`, computed once more under the lock too unless the request named its HEAD;
  a move that the files follow takes `options.lock_prepared_move(..)`, and `bd pick` checks again under the lock
  (`recheck_pick`). An operation reads a repository fact once and passes it
  down: a step of a sync, or of a `bd` command after its sync, takes the facts `RepoFacts`, `ConfigSnapshot`,
  `Snapshot` or the `SyncReport` hold instead of reading them again, a `bd` command takes HEAD's branch, the remotes
  (and the push URL of `origin` of them) and the baseline of its context (`RepoContext::command_facts`, `baseline`,
  forgotten after a wait for the lock) and passes them to its sync (`SyncOptions::facts`) and its cores, the
  baseline with its upstream comes from one `read_baseline` (with its commit, HEAD's tip and the branch tips from one `read_listed_baseline`), and an operation that reads the directories in several steps takes them once at its entry
  (`with_dirs_of`); under the lock only the witness of what it read is read again. A remote only through `remote_command`. The params and results the app passes derive `specta::Type` (feature `specta`).
- **The operation** (`crates/operations/src/<family>.rs`, for an action on the remote: a push, a start, a restart or a
  cancel of a robot, a write to a pull request or an issue): the intent of its family (`domain_types::operation::Intent`)
  runs in the frame of `operations::engine::run`, admitted by the in-process registry (`operations::registry`, by the
  rule `domain_policy::admission` over the resources it names: the refs it pushes, what its robot lands, the robots and
  the fields of the pull requests it writes) and ended with its `Outcome`; its steps go into the `operations` part of the
  view, its progress to the caller (`EventSink`). The cores it orders are the ones above; before its first effect it
  checks the facts its plan was prepared on (`operations::witness`: one `for-each-ref` of the refs, the pull requests
  and the robots from the session, `plan_changed` when they moved). The families move here one by one
  (docs/design/rust-domain-authority.md, s9-engine-remote); the app and `bd` call the operation, never the cores of a
  family that has one.
- **bd** (usually `<op>_command.rs` in the crate of its area, e.g. `crates/bd-rewrite/src/`, and a variant of `Command`
  in `crates/bd-cli/src/lib.rs`): clap arguments, one JSON object with
  `schema` (`bd schema`, `schema_test.rs`), errors as `CliError` with an `ErrorCode` (its exit code in
  `docs/contributing/agent-cli.md`), a read-only command in `read_only!` (`agent_help.rs`); bd's own lock goes to the core
  as `held_lock` only when bd writes more under it. No git of its own.
- **The app**: a read is one entry of the registry of app-api (`for_each_command!` in `crates/app-api/src/commands.rs`,
  the grammar in `registry.rs`) with its body in a family module (`crates/app-api/src/reads/`), `async fn(&AppContext,
  params)` that calls `run_blocking(move || <op>_core(..))`; nothing else is written: the app's `#[tauri::command]`
  wrapper and its registration (`src-tauri/src/api.rs`), the E2E server's route (`crates/test-server/src/invoke.rs`)
  and the entry of the page mock's table (`crates/app-api/commands.json`, `UPDATE_COMMANDS=1` of
  `commands_json_test.rs`) come from the entry, and `COMMAND_COUNT` of `registry_test.rs` goes up by one. A write of a
  repository is an intent of `execute`, never an entry of its own; a write no session admits is a command of the app
  with its reason, `app(<AppReason>)`. Every command is an entry: the app has no handwritten command
  (`command_scan_test.rs` of app-api); what only the app can do (a window, a dialog, the updater) is a host trait of
  `crates/app-api/src/transport.rs`, which the app and the E2E server implement. `src/lib/bindings.ts` is regenerated
  (`UPDATE_BINDINGS=1` on `the_bindings_are_what_the_export_writes`, `bindings_test.rs`).
- **Same commit**: the spec, the flow's section in [Git Operations](../design/git-operations.md), and a test of the
  operation in `process_budget_test.rs` (`crates/sync-core/src/` for a core, of `history-rewrite` too;
  `crates/update-core/src/`, `crates/trial-check/src/`, `crates/remote-facts/src/`, `crates/operations/src/` and
  `crates/branch-integration/src/` for theirs; `crates/bd-cli/src/` for a `bd` command)
  that pins its processes by subcommand; a core that takes the repository lock, its row of
  [Lock spans](../design/git-operations.md#lock-spans) and the test of its span (`test_utils::lock_span::LockSpan`, a
  `bd` command on `bd_cli::fixture_test::LockSpanGit`); the scan tests stay green (`worktree_safety_test.rs`,
  `git_usage_scan_test.rs`, [Scan tests](git-rules.md#scan-tests)), and the concurrency scan beside them
  (`concurrency_scan_test.rs`: no guard of an async mutex across another `.await`, no guard of the repository lock
  across `execute_remote_command*`, `fetch_baseline`, `push_baseline`, HTTP or `block_on`); an exemption added only
  with its reason and its number of hits. All of them: `cargo nextest run -p git-executor`.

## Helpers

Reuse one of these before writing a parser or a process of your own.

| Helper | Module | For |
|---|---|---|
| `RefTransaction` (`update`, `create`, `delete`, `verify`, `commit`, `commit_while`) | `git_ops::ref_transaction` | every ref move: CAS, reflog message, related refs in one transaction |
| `read_raw_commits`, `write_commits_with_notes`, `write_store_updates`, `write_notes_to_refs`, `complete_line` | `git_ops::commit_writer` | commits, store commits and notes from one `fast-import`, exact metadata, temporary refs deleted with CAS |
| `CommitChain` | `git_ops::commit_chain` | dependent merges and commits of one branch: one `merge-tree --stdin` and one `fast-import` |
| `parse_batch_merge`, `parse_merge_tree_output`; `MergeOutcome` | `git_ops::merge_tree_output`, `git_ops::merge_memo` | the `-z` output of `merge-tree --write-tree`, a `--stdin` run merge by merge |
| `for_each_object`, `read_objects`, `object_types` | `git_ops::cat_file_batch` | many objects from one `cat-file --batch --buffer -Z`, streamed, over-limit contents skipped |
| `read_blob_bytes`, `read_texts`, `read_trees` | `git_ops::object_texts` | blob bytes or texts and tree entries of many names |
| `History::read_to`, `read_ahead_of`, `check_rewrite` | `git_ops::history` | the first-parent history a rewrite works on, never the whole one, and its guards |
| `check_rewrite_range`, `ensure_no_operation_in_progress`, `IndexUpdate`, `HeadMoved` | `git_ops::rewrite_guard` | the guards of a range not read; the user's index after a rewrite |
| `short_id`, `is_object_id`, `current_branch`, `checked_out_branch`, `read_head`, `resolve_commit`, `resolve_tree_id`, `ref_exists`, `is_ancestor`, `merge_base`, `fork_point`, `ref_tips`, `tips_under`, `current_worktree`, `repository_dirs` | `git_ops::revs` | rev, branch and worktree facts; a full id; `merge-base` exit 1 is "no", any other an error |
| `changed_paths`, `commits_touching` | `git_ops::paths` | the paths two trees differ in, as bytes; the commits that change some paths |
| `replay_range`, `replay_range_with_env`, `merge_trees` | `git_ops::replay` | `git replay` of a range, a merge of three trees, objects only |
| `read_notes`, `read_noted_commits`, `notes_ref`, `write_commit_notes` | `git_ops::notes` | the notes of many commits from one `log --no-walk --stdin`, with their committer times and subjects; the mapping notes |
| `get_commits_info`, `find_commit`, `empty_tree`, `blob_id`, `object_id_of` | `git_ops::commit_utils` | commits by id from one `log --no-walk --stdin`; ids without a process |
| `get_commit_list_with_handler` | `git_ops::commit_list` | the commits ahead of the baseline (`domain_types::commit::Commit`), streamed |
| `read_commit_files_cached`, `read_diff_files` | `git_ops::commit_files` | files and line counts of many commits from one `diff-tree --stdin`, of a range from one `diff-tree` |
| `read_attributes` | `git_ops::attributes` | the attributes of many paths, one `check-attr --stdin -z` |
| `resolve_by_words`, `RESOLVE_VERSION` | `git_ops::word_resolve` | a conflicting cherry-pick that its words resolve, all or nothing: the files and the tree |
| `read_descriptions`, `write_description`, `clear_description` | `git_ops::branch_description` | the pull request text of a branch |
| `to_final_branch_name`, `qualified_branch_name`, `strip_branch_namespace`, `sanitize_branch_name` | `domain_types::branch_names` | `{prefix}/virtual/<name>` and the other namespaces under the prefix; a group name valid in a ref |
| `RepoFacts`, `read_baseline_listing`; `read_baseline`; `read_listed_baseline` | `sync_core::repo_facts` | the baseline, HEAD, upstream and the branches under the prefix, read once per sync; the baseline and its upstream outside a sync, in one read (a fetch, Update Project); the baseline with its commit, HEAD's tip and the branch tips in one read (a check session, `bd move-hunk`) |
| `resolve_baseline_upstream`, `fetch_baseline`, `check_baseline_update`, `baseline_tracking_ref` | `sync_core::baseline` | the remote branch the baseline tracks, its fetch, the update check every sync runs |
| `resolve_github_repository`, `parse_github_repository` | `sync_core::github_repository` | the GitHub repository of the branches from the config, `origin` or the Space mirror, without HTTP |
| `read_commit_diffs_cached`, `read_commit_changes` | `trial_check::diff_scan` | the changes of many commits from one `log --raw -z`; only a trial check or a pick reads diffs, never a sync |
| `plan_stack_branches`, `StackBuilds`, `read_stack_branches` | `sync_core::stack_branch` | the stack branch of a stack that forks: its members and commits, its remembered builds, its state without a sync |
| `ConfigSnapshot` | `sync_core::config_snapshot` | the `branchdeck.*` settings and the descriptions from one `git config` |
| `Snapshot`, `CommitsAhead`, `read_grouping_inputs` | `history_rewrite::snapshot` | the commits ahead of the baseline and what groups them, once per operation; `Snapshot::read_with` with the stack setting and the commits ahead a caller read |
| `LineStore`; `ISSUE_GROUPS`, `STACK_PARENTS` | `history_rewrite::store_lines`; `issue_groups`, `stack_parents` | a `key<TAB>value` map in a ref of its own: read, CAS write |
| `walk_commits`, `CommitKey`, `same_changes`, `pair_by_patch_id` | `git_ops::landed_pairing` | a `rev-list` walk matched across a rebase; the patch ids of commits, without context or with it |
| `parse_subject`, `stripped_subject` | `sync_utils::commit_subject` | the autosquash markers and the `(prefix)` of a subject |
| `resolve_commit_ids` | `history_rewrite::commit_ids` | the ids of a request, abbreviated or full, among the commits ahead of the baseline |
| `CacheMemo`, `ConflictMemo` | `sync_core::cache_memo` | merges, commit trees and conflict views in the object cache |
| `lock_repository`, `RewriteOptions`, `ensure_expected_head` | `history_rewrite::repository_lock` | the repository lock and the expected HEAD |
| `execute_remote_command` (`_accepting`, `_with_progress`, `_with_progress_until`) | `sync_core::remote_command` | every push and fetch |
| `push_refs` | `sync_core::push_lease` | one leased `push --porcelain` of many refs |
| `detect_baseline_branch`; `check_sync_guards` | `history_rewrite::baseline`; `sync_core::sync` | the baseline outside a sync; the guards of a sync |
| `with_dirs_of`, `known_dirs`, `git_common_dir`, `RepositoryDirs::git_path` | `git_ops::revs` | the directories of the repository, read once at the entry of an operation and carried by its executor: a path in the git directory with no `rev-parse` |
| `ProcessPermits` | `git_executor::concurrency` | the slots of an operation's parallel git work |
| `with_process_stats`, `GitCommandExecutor::process_stats`; `ProcessStats::by_command` | `git_executor::git_command_executor`; `git_executor::process_stats` | the processes of an operation, in a test or in the log |
