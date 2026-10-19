# Adding a Git Operation

**Status:** Active  
**Updated:** 2026-10-19  
**Tags:** [git, rust, bd, tauri]

## Summary

Read this before you add or change a git operation in `crates/git-ops`, `crates/sync-core`, `crates/bd-cli` or
`src-tauri/src/commands`. The checklist is in [AGENTS.md](../../AGENTS.md) ("Adding a git operation"), the hard rules
in its "Git Operations", the flows and the reasons in [Git Operations](../design/git-operations.md).

## Layers

An operation keeps one name across the layers (`git_ops::move_hunk`, `sync_core::move_hunk`, `bd move-hunk`). The `//!`
headers of `crates/git-ops/src/lib.rs` and `crates/sync-core/src/lib.rs` map the modules by area; the layers are the
[Architecture](../design/git-operations.md#architecture) of Git Operations.
- **git-ops** (`crates/git-ops/src/<op>.rs`): the git, functions of `(git, repo_path, …)`, one process per phase (many
  ids or paths on stdin, `cat-file --batch`, `fast-import`, one `RefTransaction`), through `git.cmd(&args, repo)` with
  one finisher, or the shorthands `execute_command`, `execute_command_lines`, `execute_command_with_status`. Read `-z`
  output (paths as bytes), take full ids (`is_object_id` for an id from outside), put a revision from outside after
  `--end-of-options` and a path after `--` as a literal pathspec (`--literal-pathspecs` first in the argument list).
  A rewrite of the checked-out branch runs the guards on the history it read (`History::check_rewrite`;
  `check_rewrite_range` for a range it did not read) and moves the branch with CAS on the HEAD it read. Reuse a helper
  below before writing a parser; no lock and no policy here.
- **The core** (`crates/sync-core/src/<op>.rs`): `<op>_core(git, params, …)`, the one entry of every caller. A rewrite
  of the checked-out branch takes `&RewriteOptions`: `options.lock(..)` before it reads anything, then the expected
  HEAD (`ensure_head`, `expected_full_head`), then git-ops. A step of a sync, or of a `bd` command after its sync, takes
  the facts `RepoFacts`, `ConfigSnapshot`, `Snapshot` or the `SyncReport` hold instead of reading them again; a remote
  only through `remote_command`. The params and results the app passes derive `specta::Type` (feature `specta`).
- **bd** (usually `crates/bd-cli/src/<op>_command.rs`, a variant of `Command` in `lib.rs`): clap arguments, one JSON object with
  `schema` (`bd schema`, `schema_test.rs`), errors as `CliError` with an `ErrorCode` (its exit code in
  `docs/guidelines/agent-cli.md`), a read-only command in `read_only!` (`agent_help.rs`); bd's own lock goes to the core
  as `held_lock` only when bd writes more under it. No git of its own.
- **The app** (`src-tauri/src/commands/<op>.rs`): `#[tauri::command]` `#[specta::specta]` that calls
  `run_blocking(move || <op>_core(..))`, registered in `src-tauri/src/lib.rs`, `src/lib/bindings.ts` regenerated
  (`bindings_test.rs`); the E2E server routes the command to the same core (`crates/test-server/src/lib.rs`,
  `tauri_command_bridge.rs`).
- **Same commit**: the spec, the flow's section in [Git Operations](../design/git-operations.md), and a test of the
  operation in `process_budget_test.rs` (`crates/sync-core/src/` for a core, `crates/bd-cli/src/` for a `bd` command)
  that pins its processes by subcommand; the scan tests stay green (`worktree_safety_test.rs`,
  `git_usage_scan_test.rs`, "Scan tests" of AGENTS.md), an exemption added only with its reason and its number of hits.

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
| `read_notes`, `notes_ref`, `write_commit_notes` | `git_ops::notes` | the notes of many commits from one `log --no-walk --stdin`; the mapping notes |
| `get_commits_info`, `find_commit`, `empty_tree`, `blob_id`, `object_id_of` | `git_ops::commit_utils` | commits by id from one `log --no-walk --stdin`; ids without a process |
| `get_commit_list_with_handler` | `git_ops::commit_list` | the commits ahead of the baseline, streamed |
| `read_commit_files_cached`, `read_diff_files` | `git_ops::commit_files` | files and line counts of many commits from one `diff-tree --stdin`, of a range from one `diff-tree` |
| `read_attributes` | `git_ops::attributes` | the attributes of many paths, one `check-attr --stdin -z` |
| `resolve_by_words`, `RESOLVE_VERSION` | `git_ops::word_resolve` | a conflicting cherry-pick that its words resolve, all or nothing: the files and the tree |
| `read_descriptions`, `write_description`, `clear_description` | `git_ops::branch_description` | the pull request text of a branch |
| `to_final_branch_name`, `qualified_branch_name`, `strip_branch_namespace`, `sanitize_branch_name` | `git_ops::branch_names` | `{prefix}/virtual/<name>` and the other namespaces under the prefix; a group name valid in a ref |
| `RepoFacts`, `read_baseline_listing` | `sync_core::repo_facts` | the baseline, HEAD, upstream and the branches under the prefix, read once per sync |
| `plan_stack_branches`, `StackBuilds`, `read_stack_branches` | `sync_core::stack_branch` | the stack branch of a stack that forks: its members and commits, its remembered builds, its state without a sync |
| `ConfigSnapshot` | `sync_core::config_snapshot` | the `branchdeck.*` settings and the descriptions from one `git config` |
| `Snapshot`, `CommitsAhead`, `read_grouping_inputs` | `sync_core::snapshot` | the commits ahead of the baseline and what groups them, once per operation |
| `LineStore`; `ISSUE_GROUPS`, `STACK_PARENTS` | `sync_core::store_lines`; `issue_groups`, `stack_parents` | a `key<TAB>value` map in a ref of its own: read, CAS write |
| `walk_commits`, `CommitKey` | `sync_core::commit_walk`, `sync_core::commit_key` | a `rev-list` walk matched across a rebase |
| `resolve_commit_ids` | `sync_core::commit_ids` | the ids of a request, abbreviated or full, among the commits ahead of the baseline |
| `CacheMemo`, `ConflictMemo` | `sync_core::cache_memo` (private) | merges, commit trees and conflict views in the object cache |
| `lock_repository`, `RewriteOptions`, `ensure_expected_head` | `sync_core::repository_lock` | the repository lock and the expected HEAD |
| `execute_remote_command` (`_accepting`, `_with_progress`, `_with_progress_until`) | `sync_core::remote_command` | every push and fetch |
| `push_refs` | `sync_core::push_lease` | one leased `push --porcelain` of many refs |
| `detect_baseline_branch`; `check_sync_guards` | `sync_core::baseline`; `sync_core::sync` | the baseline outside a sync; the guards of a sync |
| `ProcessPermits` | `git_executor::concurrency` | the slots of an operation's parallel git work |
| `with_process_stats`, `GitCommandExecutor::process_stats`; `ProcessStats::by_command` | `git_executor::git_command_executor`; `git_executor::process_stats` | the processes of an operation, in a test or in the log |
