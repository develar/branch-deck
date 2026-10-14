---
name: Agent CLI
description: Requirements for bd, the command line a coding agent uses to read the branches and commits Branch Deck sees and to regroup the commits, as JSON with a stable contract.
style: plain-1
targets:
  - ../../src-tauri/src/main.rs
  - ../../crates/bd-cli/src/lib.rs
  - ../../crates/bd-cli/src/context.rs
  - ../../crates/bd-cli/src/error.rs
  - ../../crates/bd-cli/src/output.rs
  - ../../crates/bd-cli/src/status.rs
  - ../../crates/bd-cli/src/branch_stats.rs
  - ../../crates/bd-cli/src/commits.rs
  - ../../crates/bd-cli/src/diff_scan.rs
  - ../../crates/bd-cli/src/numstat.rs
  - ../../crates/bd-cli/src/dependencies.rs
  - ../../crates/bd-cli/src/generated.rs
  - ../../crates/bd-cli/src/schema.rs
  - ../../crates/bd-cli/src/rewrite.rs
  - ../../crates/sync-core/src/regroup.rs
  - ../../crates/sync-core/src/repository_lock.rs
  - ../../crates/bd-cli/src/agent_help.rs
  - ../../crates/sync-core/src/grouping.rs
  - ../../crates/object-cache/src/cache.rs
  - ../../crates/object-cache/src/commit_diff.rs
  - ../../crates/git-executor/src/git_info.rs
---

# Agent CLI

Status: Implemented (read and regroup; check, sync, describe and push come later)
Date: 2026-10-14

## Summary

`bd` lets a coding agent read what Branch Deck sees: the branches the commits ahead of the baseline group into, the
unassigned commits, and every commit with its files and what ties it to the others. The agent clusters the commits
into branches from that and applies the grouping with one validated rewrite of the commit messages. It is the `cli` subcommand of the app binary
(`branch-deck cli status`), or the binary symlinked as `bd`. Every output is one JSON object with a stable schema.
How it works and the workflow: [Agent CLI guide](../../docs/guidelines/agent-cli.md).

## Goals

- Give an agent everything it needs to cluster commits, in a few compact calls.
- Keep the output contract stable: versioned JSON, fixed error codes and exit codes.
- Stay fast on hundreds of commits: one git process per phase, and a cache for the per-commit data.
- Reading changes nothing; a regroup changes commit messages only, never trees, the index or the working tree.

## Non-goals

- Syncing, describing and pushing branches from the CLI: they come later.
- Reordering, squashing, dropping or splitting commits.
- A console binary on Windows; the CLI runs from the macOS app binary first.

## Requirements

### Invocation

- `branch-deck cli <command>` and a binary named `bd` must run the CLI without starting Tauri.
- `-C <repo>` must select the repository, default the current directory; a subdirectory must resolve to the top level.
  [@test] ../../crates/bd-cli/src/status_test.rs (`subdirectory_resolves_to_top_level`)
- The branch prefix must come from `--prefix`, else the app setting `globalUserBranchPrefix`, else
  `git config branchdeck.branchPrefix`, and every output must echo it.
  [@test] ../../crates/bd-cli/src/status_test.rs (`prefix_from_app_settings`, `prefix_from_git_config`)
- Git must come from `BD_GIT`, else `git` on `PATH`, else Homebrew's git, never from a login shell, and be 2.54 or newer.
- Logs must go to stderr, and only with `-v` (debug) or `-vv` (trace).

### Output contract

- stdout must hold exactly one JSON object per run, and every object must have `"schema": 1`.
- `bd schema` must print the JSON Schema of the `status`, `commits`, `rewrite`, `undo` and `error` outputs, and the
  real outputs must use only the properties it declares.
  [@test] ../../crates/bd-cli/src/schema_test.rs (`schema_lists_every_output`, `outputs_match_schema`)
- A failure must print `{"schema":1,"error":{"code","message","commits","retryable"}}` with a stable snake_case code.
- The exit code must follow the code: 0 ok, 1 conflict, 2 invalid input, 3 stale (retry), 4 refused, 5 git or network
  error.
- An empty prefix must fail with `no_branch_prefix`, a path outside a repository with `not_a_repository`, bad arguments
  with `invalid_arguments`, an unknown `--branch` with `unknown_branch`; all exit with 2.
  [@test] ../../crates/bd-cli/src/status_test.rs (`empty_prefix_is_an_error`, `not_a_repository`, `invalid_arguments`, `status_filters_and_pages`)
- Commit ids must be 8 characters, longer for the whole output when two of its ids share that prefix; `--full-ids`
  must print full ids.
  [@test] ../../crates/bd-cli/src/status_test.rs (`full_ids`)

### `bd status`

- It must print HEAD, the baseline, the prefix, the branches newest first (name, ref, issue key, latest time, commit
  count, the tip of the synced virtual branch or `null`, commits newest first without the prefix) and the unassigned
  commits.
  [@test] ../../crates/bd-cli/src/status_test.rs (`status_shape`)
- `--branch` must list one branch, `--unassigned` only the unassigned commits; `--limit`/`--offset` must page their
  commits, and the branches otherwise.
  [@test] ../../crates/bd-cli/src/status_test.rs (`status_filters_and_pages`)
- It must not run integration detection, an update check or a network command.
- `--stats` must add per branch the distinct files, the generated files, added and deleted lines, the top three
  directories, the authors, the author date range, the issue keys and a diffstat of the synced branch against the sync
  base (`null` if it was never synced), for all branches in at most three more git processes.
  [@test] ../../crates/bd-cli/src/status_test.rs (`stats_per_branch`)

### `bd commits`

- It must print every commit ahead of the baseline, oldest first: id, subject, body, author, time, group (or `null`),
  issue key, and for a fixup its kind and the target autosquash resolves.
  [@test] ../../crates/bd-cli/src/commits_test.rs (`entries_groups_and_fixups`)
- `--files` must list each changed file with its status (`A`, `M`, `D`, `R` with the old path, `T`) and the counts
  `--numstat` reports; a binary file must have `"binary": true` and no counts.
  [@test] ../../crates/bd-cli/src/commits_test.rs (`files_with_renames_binaries_and_deletions`)
  [@test] ../../crates/bd-cli/src/diff_scan_test.rs (`parses_raw_and_patch_parts`, `parses_log_and_diff_tree_numstats`)
- `--patch` must add each commit's patch with three lines of context; `--branch` must keep the commits of one branch
  and the edges and overlaps that involve it.
- `dependencies` must list `{from, to, files}` where `from` touches lines `to` added, adjacent lines included, plus the
  file-level rules for created, deleted, renamed and binary files.
  [@test] ../../crates/bd-cli/src/dependencies_test.rs (`overlapping_and_separate_hunks`, `lines_move_with_earlier_insertions`, `several_hunks_in_one_change`, `adjacent_changes_depend`, `created_renamed_deleted_and_binary_files`)
- `cluster_overlap` must list, for each pair of branches, the files both change.
  [@test] ../../crates/bd-cli/src/commits_test.rs (`dependency_edges_and_cluster_overlap`)

### Generated files

- Lock files and the paths HEAD's attributes mark `linguist-generated` or `-diff` must not count in `dependencies`,
  `cluster_overlap` and the stats lines; `--files` must still list them with `"generated": true`, and
  `--include-generated` must count them.
  [@test] ../../crates/bd-cli/src/commits_test.rs (`generated_files_are_flagged_and_unlinked`)
  [@test] ../../crates/bd-cli/src/status_test.rs (`stats_per_branch`)

### Object cache

- The per-commit changes must be cached by commit id in `$GIT_COMMON_DIR/branch-deck/cache/`, and git must run once
  for the missing ids only. Cached and uncached outputs must be equal; `--no-cache` must neither read nor write it.
  [@test] ../../crates/bd-cli/src/commits_test.rs (`cached_output_equals_uncached`)
- Several processes must use the cache at once; a full cache must be cleared; older schema versions must be dropped;
  any cache failure must be a miss.
  [@test] ../../crates/object-cache/src/cache_test.rs (`round_trip`, `older_schema_versions_are_dropped`, `unusable_cache_is_a_miss`, `full_cache_is_cleared`, `parallel_threads`, `parallel_processes`)

### `bd group`, `rename`, `restack`, `unstack`

- `bd group` must read a plan `{branches: {path: [ids]}, unassign: [ids]}` from stdin or `--plan`, and set the
  prefixes of all its commits in one message-only rewrite; ids may be abbreviated.
  [@test] ../../crates/bd-cli/src/rewrite_test.rs (`group_applies_a_plan`)
  [@test] ../../crates/sync-core/src/regroup_test.rs (`applies_a_plan_in_one_rewrite`, `unassign_falls_back_to_the_issue_key`)
- The plan must be validated against every subject ahead of the baseline before anything is written: unknown ids and
  ids not ahead of the baseline are `not_ahead_of_baseline`; duplicate or contradicting ids, a malformed plan, invalid
  names, a fixup separated from its target and a stack problem the plan introduces are `invalid_plan`, with the
  commits named. Problems that already existed must be reported, not refused.
  [@test] ../../crates/sync-core/src/regroup_test.rs (`validates_the_plan_before_writing`, `refuses_problems_the_plan_introduces_and_keeps_existing_ones`)
  [@test] ../../crates/bd-cli/src/rewrite_test.rs (`group_errors_and_exit_codes`)
- Fixups not in the plan must follow their targets.
  [@test] ../../crates/sync-core/src/regroup_test.rs (`fixups_follow_their_targets`)
- `--expect-head` must refuse with `head_moved` (exit 3, retryable) when HEAD is another commit, and the rewrite must
  move the branch only from the HEAD the plan was computed for.
  [@test] ../../crates/sync-core/src/regroup_test.rs (`expect_head_is_a_compare_and_swap`)
- Rewrites must hold `$GIT_COMMON_DIR/branch-deck.lock`, wait a bounded time for another holder, then fail with
  `locked` (exit 3). A rebase or merge in progress is `operation_in_progress` and a detached HEAD `detached_head`
  (exit 4).
  [@test] ../../crates/sync-core/src/regroup_test.rs (`waits_for_the_repository_lock`, `refuses_while_a_rebase_is_in_progress`)
- The output must list every commit's resulting group with its new id, the new HEAD and an undo token;
  `--dry-run` must write nothing and print the changed subjects and the stack forest; `--map` must add old → new ids.
  [@test] ../../crates/bd-cli/src/rewrite_test.rs (`dry_run_prints_the_changes_and_the_forest`)
  [@test] ../../crates/sync-core/src/regroup_test.rs (`dry_run_writes_nothing`)
- `bd rename` must replace the branch's level in every prefix path, children included, and give an issue branch's
  commits a prefix; the new name must be a branch name without `/`.
  [@test] ../../crates/sync-core/src/regroup_test.rs (`renames_a_branch_with_its_children`, `renames_flat_and_issue_branches`)
- `bd restack` and `bd unstack` must set or clear the parent on all commits of the branch, and need stacked branches.
  [@test] ../../crates/sync-core/src/regroup_test.rs (`restacks_and_unstacks_a_branch`, `restack_needs_stacked_branches`)
  [@test] ../../crates/bd-cli/src/rewrite_test.rs (`rename_restack_unstack_and_undo`)

### `bd undo`

- `bd undo <undo_token>` must move the branch back to the token's previous HEAD with compare-and-swap, and refuse
  with `head_moved` once the branch changed again.
  [@test] ../../crates/sync-core/src/regroup_test.rs (`undo_restores_the_previous_head`)
  [@test] ../../crates/bd-cli/src/rewrite_test.rs (`rename_restack_unstack_and_undo`)

### `bd help agent`

- It must print the agent workflow as text and mark the commands that don't exist yet.
