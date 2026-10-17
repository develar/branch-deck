---
name: Agent CLI
description: Requirements for bd, the command line a coding agent uses to read the branches and commits Branch Deck sees and to regroup the commits, as JSON with a stable contract.
style: plain-1
targets:
  - ../../src-tauri/src/main.rs
  - ../../crates/bd-cli/src/main.rs
  - ../../scripts/build-bd.mjs
  - ../../src-tauri/tauri.windows-bd.conf.json
  - ../../crates/bd-cli/src/lib.rs
  - ../../crates/bd-cli/src/context.rs
  - ../../crates/sync-core/src/branch_prefix.rs
  - ../../crates/bd-cli/src/error.rs
  - ../../crates/bd-cli/src/output.rs
  - ../../crates/bd-cli/src/status.rs
  - ../../crates/bd-cli/src/branch_stats.rs
  - ../../crates/bd-cli/src/commits.rs
  - ../../crates/bd-cli/src/areas.rs
  - ../../crates/sync-core/src/diff_scan.rs
  - ../../crates/bd-cli/src/numstat.rs
  - ../../crates/sync-core/src/dependencies.rs
  - ../../crates/bd-cli/src/generated.rs
  - ../../crates/bd-cli/src/schema.rs
  - ../../crates/bd-cli/src/version.rs
  - ../../crates/bd-cli/build.rs
  - ../../crates/bd-cli/src/rewrite.rs
  - ../../crates/bd-cli/src/group_plan.rs
  - ../../crates/bd-cli/src/describe.rs
  - ../../crates/git-ops/src/branch_description.rs
  - ../../crates/bd-cli/src/sync_command.rs
  - ../../crates/sync-core/src/branch_processor.rs
  - ../../crates/bd-cli/src/check_command.rs
  - ../../crates/bd-cli/src/robot_command.rs
  - ../../crates/bd-cli/src/request_review_command.rs
  - ../../crates/bd-cli/src/fold_command.rs
  - ../../crates/bd-cli/src/move_hunk_command.rs
  - ../../crates/sync-core/src/move_hunk.rs
  - ../../crates/git-ops/src/move_hunk.rs
  - ../../crates/git-ops/src/merge_tree_output.rs
  - ../../crates/sync-core/src/autosquash.rs
  - ../../crates/bd-cli/src/suggest.rs
  - ../../crates/bd-cli/src/clustering.rs
  - ../../crates/sync-utils/src/issue_pattern.rs
  - ../../crates/bd-cli/src/push_command.rs
  - ../../crates/sync-core/src/remote_command.rs
  - ../../crates/sync-core/src/push_lease.rs
  - ../../crates/sync-core/src/push.rs
  - ../../crates/bd-cli/src/safe_push_command.rs
  - ../../crates/sync-core/src/patronus_review.rs
  - ../../crates/sync-core/src/review_readiness.rs
  - ../../crates/sync-core/src/patronus_start.rs
  - ../../crates/sync-core/src/check.rs
  - ../../crates/git-ops/src/merge_memo.rs
  - ../../crates/git-ops/src/cherry_pick.rs
  - ../../crates/sync-core/src/cache_memo.rs
  - ../../crates/object-cache/src/conflict_info.rs
  - ../../crates/sync-core/src/regroup.rs
  - ../../crates/sync-core/src/issue_groups.rs
  - ../../crates/sync-core/src/commit_ids.rs
  - ../../crates/sync-core/src/repository_lock.rs
  - ../../crates/bd-cli/src/agent_help.rs
  - ../../crates/bd-cli/src/install.rs
  - ../../src-tauri/src/commands/agent_cli.rs
  - ../../src/hooks/useInstallAgentCli.ts
  - ../../crates/sync-core/src/grouping.rs
  - ../../crates/object-cache/src/cache.rs
  - ../../crates/object-cache/src/commit_diff.rs
  - ../../crates/git-ops/src/notes.rs
  - ../../crates/git-ops/src/copy_commit.rs
  - ../../crates/sync-core/src/dangling_references.rs
  - ../../crates/sync-core/src/dangling_fix.rs
  - ../../crates/sync-core/src/inverted_stack.rs
  - ../../crates/sync-core/src/missing_declarations.rs
  - ../../crates/git-ops/src/object_texts.rs
  - ../../crates/object-cache/src/commit_lines.rs
  - ../../crates/git-executor/src/git_info.rs
---

# Agent CLI

Status: Implemented
Date: 2026-10-16

## Summary

`bd` lets a coding agent read what Branch Deck sees: the branches the commits ahead of the baseline group into, the
unassigned commits, and every commit with its files and what ties it to the others. The agent clusters the commits
into branches from that, applies the grouping with one validated rewrite of the commit messages, syncs the virtual
branches and writes their pull request text. It is the `cli` subcommand of the app binary
(`branch-deck cli status`), or the binary symlinked as `bd`; on Windows a console `bd.exe` next to the app. Every output is one JSON object with a stable schema.
How it works and the workflow: [Agent CLI guide](../../docs/guidelines/agent-cli.md).

## Goals

- Give an agent everything it needs to cluster commits, in a few compact calls.
- Keep the output contract stable: versioned JSON, fixed error codes and exit codes.
- Stay fast on hundreds of commits: one git process per phase, and a cache for the per-commit data.
- Reading changes nothing; a regroup or a fold changes commit messages only, never trees, the index or the working
  tree. `bd move-hunk` changes the trees of the commits between two commits only, never HEAD's tree, the index or the
  working tree. Only `bd update` and its undo write the working tree, through the guarded update.

## Non-goals

- Reordering, squashing, dropping or splitting commits (`bd move-hunk` moves a file's change between two commits;
  it splits no commit into new ones).
- Moving a change across a rename, a mode change, a symlink or a submodule, or a part of a binary file.

## Requirements

### Invocation

- `branch-deck cli <command>` and a binary named `bd` must run the CLI without starting Tauri.
- On Windows, where the app binary has no console, the installer must put a console `bd.exe` next to the app
  (`crates/bd-cli/src/main.rs`, bundled as an `externalBin` by `src-tauri/tauri.windows-bd.conf.json`).
- `-C <repo>` must select the repository, default the current directory; a subdirectory must resolve to the top level.
  [@test] ../../crates/bd-cli/src/status_test.rs (`subdirectory_resolves_to_top_level`)
- The branch prefix must come from `--prefix`, else the specific one first: `branchdeck.branchPrefix` of the
  repository's local or worktree config, else the app setting `globalUserBranchPrefix`, else the global or system
  `branchdeck.branchPrefix` (`resolve_branch_prefix`); every output must echo it.
  [@test] ../../crates/bd-cli/src/status_test.rs (`prefix_from_app_settings`, `prefix_from_git_config`)
  [@test] ../../crates/sync-core/src/branch_prefix_test.rs (`the_specific_prefix_wins`, `a_local_prefix_is_repository_scoped`)
- Git must come from `BD_GIT`, else `git` on `PATH`, else Homebrew's git, never from a login shell, and be 2.54 or newer.
- Logs must go to stderr, and only with `-v` (debug) or `-vv` (trace).

### Output contract

- stdout must hold exactly one JSON object per run, and every object must have `"schema": 1`.
- The JSON must be ASCII: every other character is a `\uXXXX` escape (a UTF-16 pair above the BMP), so a reader that
  decodes stdout with a legacy code page, as Windows PowerShell 5.1 does, still gets every path and subject.
  [@test] ../../crates/bd-cli/src/describe_test.rs (`a_body_from_windows_powershell_is_read_and_printed_as_ascii`)
- A plan and a body (`--plan`, stdin, `--body-file`; `-` is stdin) must be read as UTF-8 without a leading byte order
  mark, or as UTF-16 with one, which Windows PowerShell 5.1 writes.
  [@test] ../../crates/bd-cli/src/text_input_test.rs (`a_bom_is_dropped_and_utf16_is_decoded`)
  [@test] ../../crates/bd-cli/src/check_test.rs (`a_plan_with_a_byte_order_mark_is_read`)
- `bd schema` must print the JSON Schema of the `status`, `status_brief`, `commits`, `commits_brief`, `rewrite`,
  `undo`, `check`, `suggest`, `sync`, `push`, `safe_push`, `robot`, `describe`, `install`, `update`, `update_status`, `update_abort`, `fold`,
  `move_hunk`, `version` and `error` outputs, and the real outputs must use only the properties it declares.
  [@test] ../../crates/bd-cli/src/schema_test.rs (`schema_lists_every_output`, `outputs_match_schema`)
  [@test] ../../crates/bd-cli/src/update_test.rs (`update_outputs_match_the_schema`)
  [@test] ../../crates/bd-cli/src/fold_test.rs (`fold_is_refused_on_another_head_or_while_the_lock_is_held`)
  [@test] ../../crates/bd-cli/src/move_hunk_test.rs (`the_fix_moves_into_the_commit_it_belongs_to_and_the_sync_rebuilds_the_branch`, `a_commit_between_on_the_same_lines_conflicts_and_another_head_or_the_lock_refuse`)
- `status`, `commits`, `suggest`, `check`, `robot`, `schema`, `version`, `update --dry-run`, `update --status`,
  `fold --dry-run` and `move-hunk --dry-run` must write no ref, config, index or working tree: at most git objects (`update --dry-run` replays, `move-hunk --dry-run` merges trees) and the object cache (none with
  `--no-cache`), and HEAD must stay; `bd --help` and `bd help agent` must say so in one sentence.
  [@test] ../../crates/bd-cli/src/agent_help_test.rs (`read_only_commands_write_no_ref_config_index_or_working_tree`)
  [@test] ../../crates/bd-cli/src/update_test.rs (`dry_run_neither_fetches_nor_writes`)
  [@test] ../../crates/bd-cli/src/robot_test.rs (`a_failed_robot_says_why_and_which_local_commits_it_tested`)
- A failure must print `{"schema":1,"error":{"code","message","commits","retryable"}}` with a stable snake_case code,
  and `paths` for `uncommitted_overlap` (the local files) and for a refused resolution of `bd update --continue` (the
  conflicting files).
  A rewrite whose step after the rewrite failed must print its output with that `error` in it, so that the undo
  token stays.
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
- Without a baseline (no upstream and no `origin/master` or `origin/main`, or no remote and no local `master` or
  `main`) it must be `no_baseline`, the message naming each branch it tried once.
  [@test] ../../crates/bd-cli/src/status_test.rs (`no_baseline_is_refused`)
- In a repository without commits (`git init`), every command that reads HEAD or the baseline (`status`, `commits`,
  `suggest`, `check`, `sync`, `fold`, `update` and the rewrites) must be `no_baseline` with "The repository has no
  commits yet: commit to the main branch, then sync", not a git error; telling it must cost no git process when HEAD
  resolves.
  [@test] ../../crates/bd-cli/src/status_test.rs (`no_commits_is_no_baseline`)
- `--branch` must list one branch, `--unassigned` only the unassigned commits; `--limit`/`--offset` must page their
  commits, and the branches otherwise.
  [@test] ../../crates/bd-cli/src/status_test.rs (`status_filters_and_pages`)
- It must not run integration detection, an update check or a network command.
- Each branch must show its `origin` (`prefix`, `issue_group` or `issue`) and, for an issue group, its `issue_keys`;
  the output must list `idle_issue_groups`, the keys of the issue groups that group no commit on this HEAD, and
  `issue_group_problems`, the lines of the issue groups that are not used.
  [@test] ../../crates/bd-cli/src/status_test.rs (`issue_groups_in_status`)
- With stacked branches on, each branch must show its declared `parent`, `parent_state` (`active`, `integrated`,
  `unapplied` or `missing`), the `effective_base` or `blocked_by` a sync would use, `pr_base_changed` when a parent
  without commits is passed (the branch is built on the nearest active ancestor or the baseline, never blocked by it),
  and its stack `problems` with the commits that declare them, and the output must list the `forest` order, parents
  first; `bd commits` must group by the leaf as well. The `parent_source` of a parent is `prefix` or `store` (the stack
  parents), and the output lists `idle_stack_parents` and `stack_parent_problems` ([Stack Parents](../branch/stack-parents.spec.md)).
  [@test] ../../crates/bd-cli/src/status_test.rs (`stacks_in_status`, `stored_parents_in_status`)
- Each branch with a `bd describe` text must show its title, read for all branches in one git process.
  [@test] ../../crates/bd-cli/src/describe_test.rs (`sets_prints_and_clears_a_description`)
- `--stats` must add per branch the distinct files, the generated files, added and deleted lines, the top three
  directories, the authors, the author date range, the issue keys and a diffstat of the synced branch against the sync
  base (`null` if it was never synced), for all branches in at most three more git processes.
  [@test] ../../crates/bd-cli/src/status_test.rs (`stats_per_branch`)
- `--brief` must print per branch only the name, the commit count, the issue key when it is not the name, and the
  newest subject, with stacked branches on also the parent, `blocked_by`, `pr_base_changed`, the problems and the
  forest, then the unassigned commits; it must not go with `--branch` or `--stats`.
  [@test] ../../crates/bd-cli/src/status_test.rs (`brief_status`, `brief_status_keeps_the_stack_state`)

### `bd commits`

- It must print every commit ahead of the baseline, oldest first: id, subject, body, author, time, group (or `null`),
  issue key, and for a fixup its kind and the target autosquash resolves.
  [@test] ../../crates/bd-cli/src/commits_test.rs (`entries_groups_and_fixups`)
- `--files` must list each changed file with its status (`A`, `M`, `D`, `R` with the old path, `T`) and the counts
  `--numstat` reports; a binary file must have `"binary": true` and no counts.
  [@test] ../../crates/bd-cli/src/commits_test.rs (`files_with_renames_binaries_and_deletions`)
  [@test] ../../crates/sync-core/src/diff_scan_test.rs (`parses_raw_and_patch_parts`)
  [@test] ../../crates/bd-cli/src/numstat_test.rs (`parses_log_and_diff_tree_numstats`)
- `--patch` must add each commit's patch with three lines of context, its hunks fixed whatever the user's diff config
  (`--diff-algorithm=myers --inter-hunk-context=0 --indent-heuristic`, no external diff or textconv), so that hunk N
  of a file is hunk N of `bd move-hunk --hunk N`; `--branch` must keep the commits of one branch and the edges and
  overlaps that involve it.
  [@test] ../../crates/sync-core/src/diff_scan_test.rs (`file_patch_numbers_the_hunks_of_one_file`, `file_patch_tells_renames_binaries_added_files_and_modes`)
- `dependencies` must list `{from, to, files}` where `from` touches lines `to` added, adjacent lines included, plus the
  file-level rules for created, deleted, renamed and binary files; the hunks fixed as for `--patch`, so the user's
  `diff.algorithm`, `diff.interHunkContext` or `diff.indentHeuristic` changes no edge.
  [@test] ../../crates/sync-core/src/dependencies_test.rs (`overlapping_and_separate_hunks`, `lines_move_with_earlier_insertions`, `several_hunks_in_one_change`, `adjacent_changes_depend`, `created_renamed_deleted_and_binary_files`)
  [@test] ../../crates/sync-core/src/diff_scan_test.rs (`the_diff_config_of_the_user_changes_neither_the_hunks_nor_the_edges`)
- Unless `branchdeck.symbols` is `off`, `dependencies` must also list the edges by symbols
  ([Symbol Dependencies](../commit/symbol-dependencies.spec.md)): `via` `symbols` for an edge by symbols only (its
  `files` the files of the uses) and `lines_and_symbols` for both, with `symbols` `{name, file}`; `via` must be absent
  for an edge by lines only. `units` must not join by an edge by symbols only.
  [@test] ../../crates/bd-cli/src/commits_test.rs (`an_edge_by_symbols_names_the_symbol_and_joins_no_unit`)
- `cluster_overlap` must list, for each pair of branches, the files both change.
  [@test] ../../crates/bd-cli/src/commits_test.rs (`dependency_edges_and_cluster_overlap`)
- `bd commits <id>…` must keep the commits with these ids (full, or abbreviated to at least 4 characters) and
  `--unassigned` the unassigned commits, with the edges that involve them; an unknown id must be
  `not_ahead_of_baseline`, an ambiguous one `invalid_arguments`, and the filters and `--branch` must exclude each other.
  [@test] ../../crates/bd-cli/src/commits_test.rs (`commit_ids_and_unassigned_filter_the_listing`, `an_ambiguous_id_is_invalid`)
- `--brief` must print per commit only the id, the subject (without a prefix that is exactly the group; a stack path
  or a sanitized prefix stays), the group, the issue key when it is not the group, the fixup kind and target, and the
  areas; the dependencies as `{from, to, via}`; and `units`, the connected components of two or more commits of the
  dependencies by lines and the fixups, over every commit, each oldest first.
  [@test] ../../crates/bd-cli/src/commits_test.rs (`brief_view_with_units_and_areas`, `brief_keeps_a_prefix_that_says_more_than_the_group`)
- An area must be the first `--depth` directories of a path (default 3; `.` at the top level); a commit must list at
  most three areas by added plus deleted lines, without those under 5 % of its lines except the first, and without
  the hub files of `HUB_FILES` (`crates/bd-cli/src/generated.rs`). `--depth` must need `--brief`.
  [@test] ../../crates/bd-cli/src/areas_test.rs (`an_area_keeps_at_most_depth_directories`, `hub_files_match_by_name`, `top_areas_by_lines_without_hub_files`)

### Generated files

- Lock files and the paths HEAD's attributes mark `linguist-generated` or `-diff` must not count in `dependencies`,
  `cluster_overlap`, the areas and the stats lines; `--files` must still list them with `"generated": true`, and
  `--include-generated` must count them.
  [@test] ../../crates/bd-cli/src/commits_test.rs (`generated_files_are_flagged_and_unlinked`)
  [@test] ../../crates/bd-cli/src/status_test.rs (`stats_per_branch`)

### Object cache

- The per-commit changes must be cached by commit id in `$GIT_COMMON_DIR/branch-deck/cache/`, and git must run once
  for the missing ids only. Cached and uncached outputs must be equal; `--no-cache` must neither read nor write it.
  [@test] ../../crates/bd-cli/src/commits_test.rs (`cached_output_equals_uncached`)
- Several processes must use the cache at once; a full cache must be cleared; the older versions of a store must be
  dropped, and only of that store; any cache failure must be a miss.
  [@test] ../../crates/object-cache/src/cache_test.rs (`round_trip`, `older_versions_of_a_store_are_dropped`, `a_bumped_store_keeps_the_other_stores`, `unusable_cache_is_a_miss`, `full_cache_is_cleared`, `parallel_threads`, `parallel_processes`)

### `bd group`, `rename`, `restack`, `unstack`

- `bd group` must read a plan `{branches: {path: [ids]}, unassign: [ids], issue_groups: {name: [keys]},
  ungroup_issues: [keys], unstack: [branches]}` from stdin or `--plan`, set the prefixes of all its commits in one
  message-only rewrite, and change the issue groups ([Issue Groups](../branch/issue-groups.spec.md)) and the stack
  parents ([Stack Parents](../branch/stack-parents.spec.md)) in the same ref transaction; ids may be abbreviated. A
  plan of issue groups and stack parents only rewrites no commit.
  [@test] ../../crates/bd-cli/src/rewrite_test.rs (`group_applies_a_plan`)
  [@test] ../../crates/sync-core/src/regroup_test.rs (`applies_a_plan_in_one_rewrite`, `unassign_falls_back_to_the_issue_key`)
- A commit without a prefix that the plan puts into the branch of its own issue key must keep its subject, so a plan
  that names the current branch of each commit (its stack path with stacked branches on) rewrites no commit without a
  prefix or with a canonical prefix.
  [@test] ../../crates/sync-core/src/regroup_test.rs (`the_current_grouping_as_a_plan_rewrites_nothing`)
- The plan must be validated against every subject ahead of the baseline before anything is written: unknown ids and
  ids not ahead of the baseline are `not_ahead_of_baseline`; duplicate or contradicting ids, a malformed plan, invalid
  names, a fixup separated from its target and a stack problem the plan introduces are `invalid_plan`, with the
  commits named. Problems that already existed must be reported, not refused.
  [@test] ../../crates/sync-core/src/regroup_test.rs (`validates_the_plan_before_writing`, `refuses_problems_the_plan_introduces_and_keeps_existing_ones`)
  [@test] ../../crates/bd-cli/src/rewrite_test.rs (`group_errors_and_exit_codes`)
- Fixups not in the plan must follow their targets.
  [@test] ../../crates/sync-core/src/regroup_test.rs (`fixups_follow_their_targets`)
- `descriptions` of the plan (`{branch: {title, body}}`, the leaf for a stacked branch) must be validated with it,
  before anything is written: a title of one line that is not empty, and a branch that has commits after the plan,
  else `invalid_plan`. bd must write the ones that differ after the ref transaction, under the same lock, as
  `bd describe` does, list the written ones in `described` and the texts they replaced in `replaced_descriptions`;
  a plan of descriptions only rewrites no commit. `bd check` must validate them as well.
  [@test] ../../crates/bd-cli/src/rewrite_test.rs (`group_writes_descriptions_and_syncs`, `descriptions_are_validated_with_the_plan`, `a_stacked_branch_is_described_by_its_leaf`)
- `--sync` must run the sync of `bd sync` after the writes, without releasing the repository lock in between, and add
  its output as `sync`; a conflict must exit with 1 and keep the undo token. A failed description write must skip the
  sync; it and a failed sync must keep the rewrite output, with the `error` in it and its exit code. `--sync` must not
  go with `--dry-run`.
  [@test] ../../crates/bd-cli/src/rewrite_test.rs (`group_writes_descriptions_and_syncs`, `a_sync_conflict_after_group_exits_with_1_and_keeps_the_undo_token`, `a_failed_description_write_is_the_error_of_the_output_and_keeps_the_undo_token`, `descriptions_are_validated_with_the_plan`)
  [@test] ../../crates/sync-core/src/regroup_test.rs (`a_held_lock_is_not_taken_again`)
- `--expect-head` must refuse with `head_moved` (exit 3, retryable) when HEAD is another commit, and the rewrite must
  move the branch only from the HEAD the plan was computed for.
  [@test] ../../crates/sync-core/src/regroup_test.rs (`expect_head_is_a_compare_and_swap`)
- With `--allow-append` (only with `--expect-head`) a HEAD with commits on top of the expected one must be accepted
  while the expected one is still ahead of the baseline, and the output must list the commits on top as `appended`
  with their new ids; any other HEAD stays `head_moved`. The appended commits are regrouped like the others: a rename,
  a restack or an unstack by name changes their prefix too.
  [@test] ../../crates/sync-core/src/regroup_test.rs (`allow_append_accepts_commits_on_top_and_refuses_a_rewritten_head`)
  [@test] ../../crates/bd-cli/src/rewrite_test.rs (`group_with_allow_append_lists_the_appended_commits`)
- Rewrites must hold `$GIT_COMMON_DIR/branch-deck.lock`, wait a bounded time for another holder, then fail with
  `locked` (exit 3). A rebase or merge in progress is `operation_in_progress` and a detached HEAD `detached_head`
  (exit 4).
  [@test] ../../crates/sync-core/src/regroup_test.rs (`waits_for_the_repository_lock`, `refuses_while_a_rebase_is_in_progress`)
- The output must list every commit's resulting group with its new id, the new HEAD and an undo token;
  `--dry-run` must write nothing and print the changed subjects and the stack forest; `--map` must add old → new ids.
  [@test] ../../crates/bd-cli/src/rewrite_test.rs (`dry_run_prints_the_changes_and_the_forest`)
  [@test] ../../crates/sync-core/src/regroup_test.rs (`dry_run_writes_nothing`)
- `bd rename` must replace the branch's level in every prefix path, children included, rename the issue groups and
  the stack parents of that name without a rewrite, and name an issue branch by the issue groups; the new name must be
  a branch name without `/`.
  [@test] ../../crates/sync-core/src/regroup_test.rs (`renames_a_branch_with_its_children`, `renames_flat_and_issue_branches`)
- `bd restack` and `bd unstack` must set or clear the stack parent of the branch, an issue branch and an issue group
  included, rewrite only the commits whose prefix declares another parent, and need stacked branches.
  [@test] ../../crates/sync-core/src/regroup_test.rs (`restacks_and_unstacks_a_branch`, `restack_needs_stacked_branches`)
  [@test] ../../crates/bd-cli/src/rewrite_test.rs (`rename_restack_unstack_and_undo`)

### `bd undo`

- `bd undo <undo_token>` must move the branch back to the token's previous HEAD with compare-and-swap, and refuse
  with `head_moved` once the branch changed again. A token of the issue groups (`groups:G1..G2`, or
  `H1..H2+groups:G1..G2` with a rewrite) must move them back in the same transaction, and refuse with `groups_moved`
  (exit 3, retryable) once they changed again; the stack parents likewise with `+stacks:S1..S2` (alone `stacks:S1..S2`)
  and `stacks_moved`. The parts come in this order, each at most once.
  [@test] ../../crates/sync-core/src/regroup_test.rs (`undo_restores_the_previous_head`)
  [@test] ../../crates/bd-cli/src/rewrite_test.rs (`rename_restack_unstack_and_undo`, `issue_groups_plan_and_undo`, `stack_parents_plan_and_undo`)
  [@test] ../../crates/sync-core/src/issue_groups_test.rs (`an_undo_token_is_read_strictly`)
  [@test] ../../crates/sync-core/src/issue_groups_test.rs (`a_mixed_plan_moves_head_and_the_store_in_one_transaction`)

### `bd update`

- `bd update`, `bd undo update:` and the resolve session of a conflicting update: [bd update](update.spec.md).

### `bd fold`

- `bd fold` must fold the `fixup!`, `squash!` and `amend!` commits into their targets as the app's Squash Fixups does
  ([Squash Fixups](../commit/fixups.spec.md)), under the repository lock, keeping HEAD's tree, the index and the working
  tree, and print per target the fixups folded, the orphans, and an undo token `<previous head>..<new head>` for
  `bd undo`. `--branch` must fold only the fixups whose target is in that branch, and a name that is no branch of the
  commits ahead of the baseline is `unknown_branch`; `--dry-run` must write nothing; nothing to fold is no rewrite and
  no token.
  [@test] ../../crates/bd-cli/src/fold_test.rs (`fold_keeps_the_tree_and_the_undo_restores_the_head`, `fold_of_one_branch_and_a_dry_run`)
- `--expect-head` must refuse with `head_moved` on another HEAD, and the fold must check that HEAD, the planned
  commits and the history it rewrites all end at the planned head; a held lock is `locked`.
  [@test] ../../crates/bd-cli/src/fold_test.rs (`fold_is_refused_on_another_head_or_while_the_lock_is_held`)
  [@test] ../../crates/sync-core/src/autosquash_test.rs (`refuses_when_the_branch_moved_since_the_plan`)

### `bd move-hunk`

- `bd move-hunk <from> <to> <path>` must move the changes of the file `path` in commit `from` (all of them, or with
  `--hunk N`, repeatable, the hunks numbered from 1 as in `bd commits <from> --patch`) into commit `to`, older or
  newer, both ahead of the baseline on the checked out branch, in one rewrite under the repository lock. HEAD's tree
  must stay, and so the index and the working tree: into an older commit the change is added to the trees of
  `[to, from)` and `from` keeps its tree; into a newer one it is taken out of the trees of `[from, to)` and `to` keeps
  its tree. The commits above the newer one get new parents only.
  [@test] ../../crates/git-ops/src/move_hunk_test.rs (`moves_a_file_change_into_an_older_commit`, `moves_a_file_change_into_a_newer_commit`, `moves_one_hunk_of_three`, `keeps_dirty_and_staged_changes_byte_for_byte`)
  [@test] ../../crates/bd-cli/src/move_hunk_test.rs (`the_fix_moves_into_the_commit_it_belongs_to_and_the_sync_rebuilds_the_branch`)
- A hunk must apply by its line ranges to the file as each commit between has it, through one
  `merge-tree --write-tree --stdin`: a commit that shifts the lines still takes it; one that changes the same lines, or
  lacks the file, is `hunk_conflict` (exit 1) with `commits` and `paths`, and nothing is written but git objects.
  [@test] ../../crates/git-ops/src/move_hunk_test.rs (`a_commit_between_that_shifts_the_lines_still_applies`, `a_commit_between_that_changes_the_same_lines_conflicts`)
  [@test] ../../crates/git-ops/src/merge_tree_output.rs (`parses_the_merges_of_a_stdin_run`)
- `from`, `to` and the commits between whose diff of the file changed (`patch-id --stable`) must keep their author
  and get the current user as committer and no mapping note (the sync copies them again); every other commit must
  keep author and committer byte for byte, and its mapping note: with the new trees as its source for a commit
  between, as it was above. So the sync rebuilds the branches of `from` and `to` and keeps the others.
  [@test] ../../crates/git-ops/src/move_hunk_test.rs (`a_commit_between_that_shifts_the_lines_still_applies`, `moves_a_file_change_into_an_older_commit`)
- It must print the moved hunks (`moved.hunks`, header and text), `commits` (old -> new id of every recreated commit),
  `rewritten` (the commits whose changes differ) and an undo token `<previous head>..<new head>` for `bd undo`;
  `--sync` must sync under the same lock (`sync`, or `error` with the token kept); `--dry-run` must print the hunks
  and check the rewrite, writing nothing but git objects.
  [@test] ../../crates/bd-cli/src/move_hunk_test.rs (`the_hunk_number_is_the_one_of_bd_commits_patch`)
  [@test] ../../crates/git-ops/src/move_hunk_test.rs (`a_dry_run_writes_no_ref_and_no_note`)
- It must refuse with `invalid_arguments` (exit 2) the same commit twice, a file the commit doesn't change, a file it
  renames or copies, a mode change, a symlink or submodule, `--hunk` of a binary file or of an added or deleted
  file, and a move that leaves a commit without changes ("squash it instead"); an unknown hunk number is
  `hunk_not_found` (exit 2), an id that is no commit ahead of the baseline `not_ahead_of_baseline`; `--expect-head`
  is `head_moved` on another HEAD, a held lock `locked`.
  [@test] ../../crates/git-ops/src/move_hunk_test.rs (`refuses_a_move_that_leaves_a_commit_empty_or_names_no_change`)
  [@test] ../../crates/bd-cli/src/move_hunk_test.rs (`the_hunk_number_is_the_one_of_bd_commits_patch`, `a_commit_between_on_the_same_lines_conflicts_and_another_head_or_the_lock_refuse`, `a_fix_that_is_the_whole_commit_is_refused_as_a_squash`)
- It must run a fixed number of git processes, whatever the number of commits between.

### `bd check`

- `bd check` must apply a plan (stdin or `--plan`; the current grouping without one) in memory, validate it like a
  regroup (exit 2 when invalid, `descriptions` included), place the branches as a stacked sync does and build each
  one, parents first, writing git objects and the cache only: refs, notes, config and HEAD must not change.
  [@test] ../../crates/sync-core/src/check_test.rs (`a_commit_moved_away_from_its_dependency_conflicts`, `stacking_resolves_the_conflict`, `the_current_grouping_and_a_clean_plan_build`, `an_invalid_plan_is_refused`)
  [@test] ../../crates/bd-cli/src/check_test.rs (`the_current_grouping_and_invalid_plans`)
- Per branch it must print `ok`, a light conflict (commit, subject, files and the older commits outside the branch
  and its ancestors that change those files), `blocked_by`, `problems` or `error`, and the branch it would be built
  on; the exit code must be 1 unless every branch is `ok`.
  [@test] ../../crates/bd-cli/src/check_test.rs (`a_conflicting_plan_names_the_commit_and_what_it_needs`, `stacking_resolves_the_conflict`)
  [@test] ../../crates/sync-core/src/check_test.rs (`children_of_a_conflicting_branch_are_blocked`)
- With `--fix` it must add `fix`, the regroup after which the conflicting branches apply cleanly as the trial builds of
  [Conflict Fix](../branch/conflict-fix.spec.md) find it, each component's `plan` the whole plan for `bd group`;
  nothing is applied.
  [@test] ../../crates/bd-cli/src/check_test.rs (`fix_finds_the_regroup_after_which_the_plan_applies_and_writes_nothing`)
- With `branchdeck.symbols on`, `missing_declarations` must name each use, by a commit of a built branch, of a name
  that a commit outside the branch and the branches it is built on adds (an edge by symbols), with the evidence; it
  must not change `ok` or the exit code.
  [@test] ../../crates/bd-cli/src/check_test.rs (`a_branch_that_uses_a_name_another_branch_adds_is_a_missing_declaration`)
- A built branch that removes a declaration its virtual tree still uses where HEAD's tree doesn't must be
  `dangling_reference` (exit 1) with `{identifier, file, line, removed_by, fixed_by, mutual}`, when a commit outside
  the branch and its ancestors changes the use (`fixed_by`); `mutual` when that change uses what the branch declares,
  with a `fix` only when a trial build of the plan with it proves it (the branch builds without a proven dangling
  reference, no branch that built breaks): the move of `fixed_by` into the branch when it changes only that file, else
  a `bd move-hunk` command, tried on commits written as objects only that no ref names, at most two per check.
  [@test] ../../crates/bd-cli/src/check_test.rs (`a_mutual_dangling_reference_is_fixed_by_the_move_hunk_it_names`, `a_move_hunk_fix_that_breaks_another_branch_is_not_offered`, `a_branch_that_removes_a_used_declaration_is_dangling_reference`)
  [@test] ../../crates/git-ops/src/move_hunk_test.rs (`a_trial_writes_the_commits_and_moves_no_ref`)
  A use no commit changes, and one the baseline got after the base, must be a `possible_dangling_reference` warning
  that leaves the exit code alone. A declaration still in its file, a private one used elsewhere, a use HEAD has as
  often, a use the branch changes itself, a member moved with its class, a declaration moved within its package and a
  private one the branch declares elsewhere must not count, nor, by the syntax trees, a word of a comment or a string,
  a local of the same name or a name a `use` or an import alias still binds; `via` must say whether the trees
  confirmed the uses (`syntax_tree`) or the lines alone found them (`lines`). Only the paths where the trees differ may
  be read.
  [@test] ../../crates/sync-core/src/dangling_references_test.rs (`the_incident_a_moved_constant_whose_use_is_fixed_in_another_branch`, `a_use_changed_by_the_branch_itself_is_not_dangling`, `a_declaration_changed_in_place_is_not_dangling`, `a_reformatted_use_is_not_dangling`, `a_private_declaration_is_not_searched_outside_its_file`, `a_removed_top_level_function_is_searched_by_its_name`, `a_use_landed_on_the_baseline_is_a_possible_reference`, `a_member_moved_with_its_class_is_not_dangling`, `a_top_level_function_moved_within_its_package_is_not_dangling`, `a_private_declaration_moved_elsewhere_is_not_dangling`, `a_name_re_exported_by_a_use_is_not_removed`, `a_word_in_kdoc_is_no_use`, `a_local_val_shadows_a_private_member_of_another_class`)
  [@test] ../../crates/bd-cli/src/check_test.rs (`a_branch_that_removes_a_used_declaration_is_dangling_reference`, `a_possible_dangling_reference_is_a_warning_and_exit_0`)
- A branch whose conflict or dangling reference needs a commit of a branch stacked on it, at any depth, must have
  `inverted_stack {branch, child, fixes}` and keep its `result`; the fixes (`move_to_child`, `swap`) must be plan
  parts for `bd group` that a trial build of the plan with them passes for both branches, and no `swap` for a mutual
  dangling reference; with no fix that builds, a `reason` instead.
  [@test] ../../crates/bd-cli/src/check_test.rs (`a_parent_that_needs_its_child_is_an_inverted_stack`, `a_mutual_dangling_reference_of_a_stack_is_an_inverted_stack_without_a_swap`, `an_inverted_stack_that_no_fix_builds_says_why`)
- `--build <command>` must run the command in the tree of each `ok` branch, extracted outside every working tree
  (`--build-paths`: only those paths), and make a failure `build_failed` with its exit code and the end of its
  output; without it nothing runs.
  [@test] ../../crates/bd-cli/src/check_test.rs (`check_build_runs_the_command_on_each_branch_tree`)
  [@test] ../../crates/bd-cli/src/agent_help_test.rs (`read_only_commands_write_no_ref_config_index_or_working_tree`)
- Merges must be memoized by their trees in the object cache, and a cached check must equal an uncached one.
  [@test] ../../crates/sync-core/src/check_test.rs (`the_cache_gives_the_same_report`)
  [@test] ../../crates/bd-cli/src/check_test.rs (`cached_check_equals_uncached`)
- A `CheckSession` must read the commits, the baseline and the branch tips once, and check any number of plans as
  `check_plan` does.
  [@test] ../../crates/sync-core/src/check_test.rs (`a_session_checks_many_plans_as_check_plan_does`)

### `bd suggest`

- `bd suggest` must print a plan for `bd group` with only the commits whose branch changes (a fixup that follows its
  target left out, a prefixed commit that goes to its own issue branch in `unassign`), the clusters with their name,
  commit count, `change` (`kept`, `renamed`, `merged`, `split`, `new`) against the groups now in `from`, `why`, `weak`, issue keys
  and areas, with stacked branches the `parent` and `restacked`, the subjects the plan rewrites, and the trial sync of the plan. It must write git objects and the cache
  only, and the plan, applied, must suggest nothing more.
  [@test] ../../crates/bd-cli/src/suggest_test.rs (`suggests_a_checked_plan_that_is_stable_once_applied`, `the_plan_moves_only_what_changes_and_is_a_regroup_plan`)
- Hard units must join the dependencies, a fixup and its target, the commits of one prefix (unless `--fresh`) and the
  commits without a prefix of one issue key. Two prefixes must never join, and a dependency between them must be an
  `anchor_dependency` warning that names `git config branchdeck.stacks true`.
  [@test] ../../crates/bd-cli/src/clustering_test.rs (`hard_units_join_dependencies_fixups_prefixes_and_keys`, `an_edge_between_two_anchors_is_a_warning_and_anchors_never_merge`)
  [@test] ../../crates/bd-cli/src/suggest_test.rs (`prefixes_that_depend_on_each_other_stay_apart_and_exit_with_1`)
- With stacked branches, a prefix must be an anchor by the leaf of its path, and a dependency must join no units: it
  must be an edge between two clusters, and only the dependencies of a cycle of units may join them. A cycle between
  two prefixes must stay an `anchor_dependency` warning. The commits without a prefix of an issue key that a prefix
  names must join that prefix, because they are one branch.
  [@test] ../../crates/bd-cli/src/clustering_test.rs (`with_stacked_branches_a_dependency_stacks_the_branches_instead_of_joining_them`)
  [@test] ../../crates/bd-cli/src/suggest_test.rs (`a_follow_up_that_joins_an_issue_branch_stays_there_once_applied`)
- With stacked branches, each cluster must get a parent so that every cluster it depends on is an ancestor: the parent
  its prefixes declare when that parent covers them; else the deepest cluster it depends on that covers the others;
  else the stacks of the clusters it needs must become one chain, in the order of the dependencies and the oldest
  cluster first, which only adds ancestors. A declared parent that is no cluster must go, a cluster must not be its own
  parent, `--fresh` must declare no parent, and in a cycle the declared parent must give way first, then the oldest
  cluster of a cycle that waits on nothing outside it must go first, never a cluster that only waits on the cycle. The
  plan must name a stacked cluster `parent/leaf` and a root by its leaf, so that a path of more levels declares no
  parent that the placement moved. A stacked parent comes from the prefixes, else from the stack parents; a restacked cluster names its
  path also when no commit moves (`"a/b": []`, the stack parents move), a cluster unstacked from its stack parent is in
  `unstack`, and a commit with the bare `(leaf)` stays where it is. A parent that stays, comes from the stack parents
  and holds no cluster the cluster depends on must be `stacked_without_need`, and the plan must keep it. A cluster of whole issue branches stays an issue
  group, stacked too. The trial applies the plan as `bd group` does, so `rewritten` counts only the commits that need
  a prefix. `why` must name a new parent and its reason.
  [@test] ../../crates/bd-cli/src/clustering_test.rs (`with_stacked_branches_a_dependency_stacks_the_branches_instead_of_joining_them`, `a_merge_that_closes_a_cycle_waits_and_a_parent_without_commits_goes`, `stack_clusters_keeps_a_declared_parent_and_gives_way_in_a_cycle`, `stack_order_breaks_a_cycle_in_a_source_component`, `a_follow_up_of_two_stacked_branches_joins_the_upper_one_and_a_restack_names_its_reason`)
  [@test] ../../crates/bd-cli/src/suggest_test.rs (`with_stacked_branches_a_branch_stacks_on_the_branch_it_needs`, `a_stacked_plan_stacks_an_issue_group_and_rewrites_a_deeper_path`, `stacked_branches_and_an_empty_range`, `the_plan_moves_only_what_changes_and_is_a_regroup_plan`, `a_stored_parent_without_need_is_reported_and_kept`)
- With `branchdeck.symbols on`, an edge by symbols only must never join two units: with stacked branches it must be an
  edge of the stack outside the join of a cycle, without them it must link only a follow-up without a prefix and an
  issue key; a unit whose names three or more units use must order none of them; a branch the plan doesn't build on
  the branch whose name it uses must be a `symbol_dependency` warning.
  [@test] ../../crates/bd-cli/src/clustering_test.rs (`an_edge_by_symbols_orders_and_never_joins`)
  [@test] ../../crates/bd-cli/src/suggest_test.rs (`with_the_symbols_on_a_use_of_another_branch_warns_or_stacks`)
- Soft merges must go by a deterministic integer score of issue key mentions, conventional scopes (`type(scope):` and
  `type scope:`), shared paths weighted by IDF, the disagreement of issue projects and scopes, and log adjacency: the
  best pair first, a tie to the oldest, under `--max-commits` (40) and down to the threshold 0.40. A key that more
  than max(10, units / 3) units name must link nothing, and one body of a branch of a prefix that names the key of a
  whole issue branch must not take the issue in without a second body or shared paths. With stacked branches, a dependency must link a cluster
  without a prefix and an issue key to the cluster it depends on, and no merge may make a cycle of the dependencies
  and the declared parents.
  [@test] ../../crates/bd-cli/src/clustering_test.rs (`with_stacked_branches_a_dependency_stacks_the_branches_instead_of_joining_them`, `a_merge_that_closes_a_cycle_waits_and_a_parent_without_commits_goes`, `a_follow_up_of_two_stacked_branches_joins_the_upper_one_and_a_restack_names_its_reason`)
  [@test] ../../crates/bd-cli/src/clustering_test.rs (`a_merge_that_only_adjacency_lifts_over_the_threshold_is_weak`, `the_same_scope_merges_commits_without_keys_and_another_scope_halves_paths`, `a_mention_links_keys_of_one_project_but_hardly_across_projects`, `a_branch_of_a_prefix_takes_in_a_whole_issue_only_by_more_than_one_mention`, `shared_paths_merge_but_a_feature_of_many_units_or_a_peripheral_file_weighs_little`, `the_cap_stops_soft_merges_and_a_tie_goes_to_the_oldest_pair`)
  [@test] ../../crates/sync-utils/src/issue_pattern.rs (`semantic_prefixes_with_and_without_parentheses`, `issue_keys_anywhere_in_a_text`)
- With `--fresh`, on the anonymized input of a large monorepo branch, the clusters must match the grouping a human
  made with a pairwise precision of at least 0.95 and a recall of at least 0.85, and keep the issue projects and the
  named pairs of groups apart.
  [@test] ../../crates/bd-cli/src/clustering_test.rs (`golden_monorepo_branch`)
- A proven dangling reference must be repaired like a conflict, the removing commit needing `fixed_by` (with stacked
  branches it stacks on its branch); a mutual one must not be stacked but named in an `unresolved_conflict` warning
  that says to move the change into `removed_by`. A possible one is a `possible_dangling_reference` warning.
  [@test] ../../crates/bd-cli/src/suggest_test.rs (`a_dangling_reference_stacks_the_branch_on_its_fix`, `a_mutual_dangling_reference_suggests_moving_the_change`)
- A conflict of the check must join the hard unit of the conflicting commit to the units of the commits it needs by
  its lines that its branch and the branches it is built on lack, by the conflicting files only when the lines name
  none, also inside an issue group, and cluster again, at most 5 times, never joining two prefixes; the exit code must be 1 when a branch
  still conflicts. With stacked branches, a conflict on a commit of another cluster must first stack the two
  clusters, and join their units only when the same conflict comes again.
  [@test] ../../crates/bd-cli/src/clustering_test.rs (`repair_joins_a_conflicting_commit_to_what_it_needs`, `with_stacked_branches_a_repair_stacks_first_and_joins_when_the_conflict_stays`)
  [@test] ../../crates/bd-cli/src/suggest_test.rs (`a_conflict_in_a_lock_file_joins_the_commits_and_fresh_ignores_prefixes`, `a_repair_joins_what_the_lines_of_the_conflict_need_not_every_commit_of_the_file`, `whole_issue_branches_become_an_issue_group`, `prefixes_that_depend_on_each_other_stay_apart_and_exit_with_1`)
- A name must be the cluster's prefix, the branch its commits come from when it holds the most of that branch, its
  single issue key, a scope of at least 60 % of its commits, `<area leaf>-<word>`, its most frequent key or
  `cluster-N`, in this order; a name another cluster, a group now or a virtual branch has, in any case, must get `-2`,
  `-3`, unless the cluster's commits come from it. `kept` must mean one whole branch under its own name.
  [@test] ../../crates/bd-cli/src/clustering_test.rs (`names_in_order_of_preference_with_collisions`, `a_cluster_takes_the_name_of_the_branch_it_holds_most_of`, `changes_against_the_groups_now`, `subject_words_drop_stopwords_and_endings`)
  [@test] ../../crates/bd-cli/src/suggest_test.rs (`fresh_keeps_the_name_of_a_branch_that_stays_whole`, `stacked_branches_and_an_empty_range`)
- A cluster of two or more whole issue branches must go into `issue_groups` of the plan, marked `"via": "issue_group"`,
  so that the plan rewrites no commit; `--rewrite-issues` gives the commits a prefix instead. An issue group must be
  an anchor, as a prefix is.
  [@test] ../../crates/bd-cli/src/suggest_test.rs (`suggests_a_checked_plan_that_is_stable_once_applied`, `whole_issue_branches_become_an_issue_group`)
- `--dump-input` (hidden) must print the input of the clustering with every name hashed by a keyed hash (SipHash) with
  a random key that is not kept, and without subjects, bodies, ids and authors, so that the dump clusters as the
  repository does; only the module markers of file paths may become one marker.
  [@test] ../../crates/bd-cli/src/clustering_test.rs (`anonymize_keeps_the_clustering_and_no_name`)
  [@test] ../../crates/bd-cli/src/suggest_test.rs (`the_dump_hides_every_name`)

### `bd sync`

- `bd sync` must run the app's sync (`sync_branches`) under the repository lock (`locked`, exit 3, when another
  process holds it) and print per branch the status (`created`, `updated`, `unchanged`, `error`, `merge_conflict`),
  the commit count, the new tip and the remote state, plus the unassigned count. It must refuse, changing nothing,
  while a rebase, merge, cherry-pick or revert is in progress (`operation_in_progress`) or HEAD is detached
  (`detached_head`, exit 4 both): the sync would archive the branches the half-done history lacks.
  [@test] ../../crates/bd-cli/src/sync_test.rs (`syncs_and_reports_each_branch`, `waits_for_the_lock`, `waits_while_a_rebase_is_in_progress`)
- A sync (the app's and `bd sync`, which don't share the repository lock) must move a virtual branch only from the tip
  it listed at its start, or create it only while no ref of its name exists: a branch another process moved to
  another tip meanwhile must keep that tip and be `error` ("Not updated: another process moved …; sync again"), while
  the other branches are written; one moved to the head the sync built (the copies are deterministic) is written.
  [@test] ../../crates/sync-core/src/branch_processor_test.rs (`a_branch_moved_since_the_listing_is_not_overwritten`, `a_new_branch_created_meanwhile_is_not_overwritten`)
- The `conflict` of a `merge_conflict` must list in `missing_commits` the older commits that change a conflicting file
  and are neither in the branch nor in the branches it is built on, never the copies of its own or its ancestors'
  commits.
  [@test] ../../crates/sync-core/src/missing_commits_test.rs (`the_missing_commits_leave_out_the_branches_it_is_built_on_and_name_their_branch`)
- A virtual commit must be reused only while its mapping note fits the commit: the trees the note records as its
  source, and the message and the author of the copy. A note carried to a changed commit (`git notes copy`,
  `notes.rewriteRef`) must make the sync and the check copy the commit again. A note without a source must be checked
  against the tree of its copy, keep a copy that fits and get its source; `--no-cache` must check every note that way.
  [@test] ../../crates/bd-cli/src/sync_test.rs (`a_stale_note_rebuilds_the_branch`, `no_cache_verifies_each_note`)
  [@test] ../../crates/sync-core/src/branch_processor_test.rs (`a_note_copied_to_a_commit_with_another_tree_is_stale`, `an_amend_with_notes_rewrite_ref_rebuilds_the_copy`, `a_note_without_a_source_keeps_an_older_copy_and_gets_one`, `a_verifying_sync_checks_the_tree_of_each_copy`)
  [@test] ../../crates/sync-core/src/check_test.rs (`check_does_not_reuse_a_stale_copy`)
  [@test] ../../crates/git-ops/src/notes_test.rs (`the_lines_are_found_by_their_prefix`)
- The remote state must count the remote commits a push drops (`behind`, `dropped_commits`) apart from the older
  versions of commits it has ([Outgoing Commits](../repository/outgoing.spec.md#what-is-outgoing)): `amended` (of
  commits amended here) and `restacked` (of commits of the branches it is built on), each left out at 0.
  [@test] ../../crates/bd-cli/src/push_test.rs (`amended_and_restacked_commits_push_without_force`)
- A conflict must name the conflicting commit, its subject, the files and the commits the change likely needs, and the
  exit code must be 1. A sync of an unchanged conflicting branch must take that view from the object cache, shared
  with the app, and run neither the merge nor the analysis of the conflict again; it must report the view the analysis
  made, with the branches of the missing commits named as it groups them. `--no-cache` must analyse every conflict.
  [@test] ../../crates/sync-core/src/process_budget_test.rs (`an_unchanged_sync_of_a_conflicting_branch`)
  [@test] ../../crates/sync-core/src/sync_test.rs (`a_remembered_conflict_is_the_one_the_analysis_made`)
  [@test] ../../crates/bd-cli/src/sync_test.rs (`a_conflict_exits_with_1`)
- Per branch whose remote branch a push would change it must print the `bd push` to run, as bd names itself
  (`bd -C <repo>`): plain for unpushed, amended or restacked commits; with `--force --expect-remote <remote.head>`
  when the push drops commits and no branch it is built on has commits for the remote; with `--force` alone when one
  has. Run as written, it must push.
  [@test] ../../crates/bd-cli/src/push_test.rs (`sync_names_the_push_of_each_branch`, `a_child_behind_its_remote_with_a_parent_to_push_forces_without_expect_remote`, `amended_and_restacked_commits_push_without_force`)
  [@test] ../../crates/bd-cli/src/sync_test.rs (`syncs_and_reports_each_branch`)
- The integration results of archived branches and the update check of the checked out branch must be included
  compactly, with the landed commits (`update.landing`, `landed`).
  [@test] ../../crates/bd-cli/src/sync_test.rs (`reports_the_commits_the_baseline_already_has`)

### `bd push`

- `bd push <branch>…` must sync first under the repository lock, then push the branches in one atomic
  `git push --porcelain` through the remote command policy, parents first, with an explicit lease per ref; `--all`
  must need `--yes`.
  [@test] ../../crates/bd-cli/src/push_test.rs (`pushes_with_a_lease_and_prints_the_pull_request`, `all_needs_yes_and_unsynced_branches_are_refused`)
  [@test] ../../crates/sync-core/src/push_lease_test.rs (`parses_porcelain_lines`)
- It must not read again what its sync read: the stack, the tips, the remote tips, the remotes and the descriptions
  come from the sync of the same run; besides the push only the push URL of `origin`, once, and the Space settings
  for a GitHub mirror.
  [@test] ../../crates/bd-cli/src/process_budget_test.rs (`a_push`)
- A stacked branch must take the branches it is built on along, parents first, in the same atomic push, when they
  have commits for the remote, as the app's push button does ([Stacked Branches](../branch/stacked-branches.spec.md));
  one on the remote as it is stays out, and one taken along brings no children with `--with-children`. `--alone`
  (named branches only) and `--expect-remote` (the lease of one branch) push only the named branches.
  [@test] ../../crates/bd-cli/src/push_test.rs (`parents_go_first_and_a_child_alone_needs_its_parent_on_the_remote`, `a_parent_goes_along_only_with_commits_for_the_remote_and_brings_no_children`)
- It must refuse (`push_refused`, exit 4, nothing pushed) a branch that didn't sync, one behind its remote branch
  without `--force` (a parent taken along too, with `--alone` as the way out), and with `--alone` a stacked branch
  whose parent is neither on the remote nor pushed with it. Older versions of commits amended or restacked here are
  not behind: such a branch pushes without `--force`, and a parent with only them is taken along.
  [@test] ../../crates/bd-cli/src/push_test.rs (`parents_go_first_and_a_child_alone_needs_its_parent_on_the_remote`, `all_needs_yes_and_unsynced_branches_are_refused`, `amended_and_restacked_commits_push_without_force`, `a_moved_remote_branch_is_fetched_and_reported_and_force_replaces_it`)
- `--with-children` must add the branches stacked on the named ones, directly or further down, parents first, to the
  same atomic push, as the app's Push with Children does ([Stacked Branches](../branch/stacked-branches.spec.md)); a
  blocked child must refuse the push as a named branch that didn't sync does. It must need named branches and must
  not go with `--expect-remote`.
  [@test] ../../crates/bd-cli/src/push_test.rs (`with_children_pushes_the_branches_stacked_on_the_named_ones`, `with_children_refuses_a_blocked_child`)
- The lease must be the remote head the sync of the same run compared with (`remote.head`), as the app leases on the
  remote head its row showed ([Outgoing Commits](../repository/outgoing.spec.md)): a remote commit the branch has by
  content must go through on the first try, `--force` must drop what the sync counted as `behind` and nothing pushed
  since, and either one's push must leave the other's next push valid. `--expect-remote <commit>` must lease one
  branch on that commit.
  [@test] ../../crates/bd-cli/src/push_test.rs (`a_plain_push_replaces_a_remote_commit_the_branch_has_by_content`, `force_drops_only_what_the_sync_saw_and_expect_remote_pins_the_lease`, `the_app_and_bd_share_the_lease`)
- With `--remote` other than `origin`, which the sync compares with, the refusal, the lease and the errors must use
  the remote status of the pushed branches against that remote, computed the same way; a `--remote` that is not a
  configured remote must be `no_remote`.
  [@test] ../../crates/bd-cli/src/push_test.rs (`another_remote_is_compared_with_as_origin_is`)
- A remote branch that is not where the lease expects it must be `lease_rejected` (exit 3) with nothing pushed; bd
  must fetch the rejected branches and list them in `error.branches` with their remote status as fetched
  (`head`, `behind`, `dropped_commits`). A branch refused as behind is listed the same way, and `bd sync` prints the
  same remote status per branch.
  [@test] ../../crates/bd-cli/src/push_test.rs (`a_moved_remote_branch_is_fetched_and_reported_and_force_replaces_it`, `force_drops_only_what_the_sync_saw_and_expect_remote_pins_the_lease`)
  [@test] ../../crates/bd-cli/src/sync_test.rs (`syncs_and_reports_each_branch`)
- Per branch it must print the pull request base (the parent's remote branch, or the baseline without the remote),
  the head, the `bd describe` title and body, and a ready `gh pr create` command, with `--repo` for a GitHub
  repository `origin` doesn't push to (a mirror).
  [@test] ../../crates/bd-cli/src/push_test.rs (`pushes_with_a_lease_and_prints_the_pull_request`, `parents_go_first_and_a_child_alone_needs_its_parent_on_the_remote`, `the_pull_request_command_names_a_github_mirror`)
- On Space, with the Safe Push CLI logged in, it must hold the branches the app holds
  ([Patronus](../branch/patronus.spec.md#pushing-into-a-branch-whose-safe-push-runs)): a named branch whose Safe Push
  runs on it, or on a branch built on it that lands it, is `robot_running` (exit 4, nothing pushed), with the robot's
  page; an ancestor taken along that a Safe Push lands stays as the robot took it, so a child that needs it on the
  remote is `push_refused`. A canceling robot and a dry run hold nothing. `--restart-running` cancels the Safe Push of
  a named branch, pushes it with the ancestors that robot lands, and restarts it on the pushed commit (`restarted`).
  One robots request after the sync; off Space, without a token or an answer of Patronus nothing is held. `bd
  safe-push` pushes without the check: it cancels the robot it replaces itself.
  [@test] ../../crates/bd-cli/src/safe_push_test.rs (`push_refuses_a_branch_whose_safe_push_runs_and_restarts_it_when_told`)

### `bd safe-push`

- `bd safe-push <branch>` must start a Patronus Safe Push of the virtual branch (with `--dry-run` a Dry Run) as the
  app's Safe Push form does ([Patronus](../branch/patronus.spec.md)): sync, then push the branch as `bd push <branch>`
  pushes it, under the repository lock, then start the robot on `refs/heads/<prefix>/virtual/<branch>` after the
  lock, never repeated. `--check` must print the same object and push or start nothing, with the `refusals` a start
  would fail with.
  [@test] ../../crates/bd-cli/src/safe_push_test.rs (`check_names_the_profiles_and_the_review_and_starts_nothing`, `missing_approvals_refuse_the_safe_push_unless_told_to_start_anyway`)
- The target branch, the stack and the tips must come from the sync of the same run, and the Space and the GitHub
  repository from one read of the push URL of `origin`, shared with the push.
  [@test] ../../crates/bd-cli/src/process_budget_test.rs (`a_safe_push_check`)
- The push must be decided by content, as the form decides it: `push_needed` only when the sync's remote status of
  the branch or of a branch it is built on has work for the remote (`unpushed`, `behind`, `amended`, `restacked`, or
  no remote branch). A branch whose remote branch has the same commits, only rebuilt since on a newer baseline, must
  not be pushed (no `pushed`): the robot starts from the remote branch, and `--create-pull-request` opens the pull
  request of the remote branch as it is. A robot running on that remote branch must refuse the start as one on the
  last commit does. A running Safe Push of an older commit must be replaced as the app's form replaces it: with nothing
  to push, a Safe Push start must refuse with `robot_running` ("the start would run beside it") unless
  `--cancel-running`, which cancels it before the start (`canceled`); a dry run and `--create-pull-request` leave it.
  [@test] ../../crates/bd-cli/src/safe_push_test.rs (`a_branch_only_rebuilt_on_a_newer_baseline_starts_from_its_remote_branch_without_a_push`, `the_pull_request_of_a_branch_only_rebuilt_on_a_newer_baseline_is_opened_without_a_push`)
- The profiles must be those of the target that allow the mode, never the emergency ones (`--profile emergency` is
  `invalid_arguments`: only the app's Safe Push without review offers one, in red); `--profile` takes an id or a name
  (any case) or `none`, another value is `invalid_arguments` that lists them. Without it the profile of the last
  robot of the branch is the default.
  [@test] ../../crates/bd-cli/src/safe_push_test.rs (`a_dry_run_takes_a_profile_by_name_and_checks_no_review`, `the_profile_of_the_last_robot_is_the_default_and_none_drops_it`)
- Before a Safe Push (never a dry run) it must check the code-owner review as the form does, with the pull request
  the robot starts from, always asked again ([Patronus](../branch/patronus.spec.md#code-owner-review)), and a
  `missing` readiness must refuse it with `approvals_missing` (exit 4), nothing pushed or started, `error.review`
  naming the groups, their reviewers and files, as the form disables Start. `--without-approvals` must start anyway,
  the counterpart of the app's Safe Push without review: with a `--profile` that skips the review (`--profile lsc`). The
  `review` says `readiness` (`not_required`, `approved`, `missing`, `unknown`) with its `readiness_reason`, the code
  owners asked who haven't approved (`waiting_for`), the approvals Patronus doesn't count (`not_counted`), and per
  group `approved` and `needed`, per reviewer `approval_state`, when Patronus evaluated the pull request.
  [@test] ../../crates/bd-cli/src/safe_push_test.rs (`missing_approvals_refuse_the_safe_push_unless_told_to_start_anyway`, `a_dry_run_takes_a_profile_by_name_and_checks_no_review`)
- The approvals must be collected in the GitHub pull request of the branch, not in a Space merge request (deprecated):
  `--create-pull-request` must push the branch and open it through the GitHub API as the `gh pr create` that `bd push`
  prints would (the `bd describe` text, else filled as `--fill` does: one commit's subject and body, or the branch
  name and the subjects of several), assigned to the user (`--no-assign` leaves it unassigned) and with the
  `--reviewer <login>` logins asked (the author left out; a reviewer GitHub refuses is in `pull_request.warnings`), as
  the app's dialog creates it ([Pull Requests](../branch/pull-requests.spec.md#creating-a-pull-request)), without a
  start, and must be `invalid_arguments` without a GitHub repository of the branches; a pull request that already
  exists is found instead and left as it is, with a warning. The `review` reviewers carry their `github_login` when
  Patronus names it, for `--reviewer`.
  [@test] ../../crates/bd-cli/src/safe_push_test.rs (`the_pull_request_it_opens_asks_the_suggested_code_owner_by_the_github_login_patronus_names`, `a_pull_request_that_already_exists_is_found_instead`) The token is the first of `GH_TOKEN`
  (`GITHUB_TOKEN`), `gh auth token`, the app's sign-in (its token file) and the git credentials of github.com ([Pull
  Requests](../branch/pull-requests.spec.md#the-token)); without one GitHub accepts, `not_logged_in`. Patronus judges
  the approvals of the pull request: an approval no eligible code owner gave (a bot) must not let the Safe Push start,
  `approvals_missing` naming who it waits for and the approval it doesn't count, and `review.approved_on_github` must
  name only the approvals that count. A Patronus that doesn't evaluate pull requests can't tell: then a pull request
  someone approved must let the Safe Push start (`readiness` `unknown`), `review.approved_on_github` naming every
  approver; without any approval, `approvals_missing` must name the pull request.
  [@test] ../../crates/bd-cli/src/safe_push_test.rs (`patronus_judges_the_approvals_of_the_pull_request`)
  [@test] ../../crates/bd-cli/src/safe_push_test.rs (`the_approvals_count_in_the_pull_request_it_opens_and_the_safe_push_starts_from_it`, `missing_approvals_refuse_the_safe_push_unless_told_to_start_anyway`)
  [@test] ../../crates/bd-cli/src/safe_push_test.rs (`a_pull_request_that_already_exists_is_found_instead`, `a_pull_request_without_a_github_token_is_not_logged_in`)
- A robot on the last commit of the branch, or a Safe Push that lands it with a branch of its stack, must refuse the
  start with `robot_running`, in the words of the app; a running Safe Push of an older commit, which the push fails,
  too, unless `--cancel-running` cancels it first.
  [@test] ../../crates/bd-cli/src/safe_push_test.rs (`a_robot_on_the_last_commit_refuses_and_one_on_an_older_commit_is_canceled_when_told`)
  [@test] ../../crates/sync-core/src/patronus_start_test.rs (`the_robot_of_a_branch_is_the_running_one_then_the_newest_and_a_temporary_ref_by_its_revision`, `a_start_waits_for_a_robot_on_the_last_commit_and_a_push_guards_a_running_safe_push`, `a_safe_push_waits_for_a_robot_that_lands_an_ancestor_or_the_branch_itself`, `a_profile_allows_the_modes_it_lists_and_any_when_it_lists_none`)
- A proven dangling reference (as `bd check` reports it) of the branch or a branch it is built on must refuse the start
  with `dangling_reference` (exit 4), nothing pushed, naming the use, `fixed_by` and the fix; `--check` must list it
  in `refusals`, `--allow-dangling` must start anyway, and the possible ones are `warnings`.
  [@test] ../../crates/bd-cli/src/safe_push_test.rs (`a_dangling_reference_refuses_the_safe_push_unless_allowed`)
- A Safe Push of a stacked branch whose parents aren't in the target yet must be `stack_not_landed`, naming the root to
  start first, unless `--land-stack`, which pushes the parents first and lands them with it; a dry run needs no flag.
  [@test] ../../crates/bd-cli/src/safe_push_test.rs (`a_stacked_branch_leads_root_first_or_lands_its_parents_when_told`)
- The `stack_not_landed` message, also as a refusal of `--check`, must end with the trial build of the branch as a
  root: that it builds alone with `bd unstack <branch>`, or the commits of its ancestors it needs, with their branch.
  It must never unstack the branch itself.
  [@test] ../../crates/bd-cli/src/safe_push_test.rs (`a_stacked_branch_leads_root_first_or_lands_its_parents_when_told`, `a_stacked_branch_that_needs_a_commit_of_its_parent_names_it`)
- Branches reviewed on GitHub with an open pull request into the target must start from a `/patronus` comment on it,
  waiting up to a minute for the answer, and must refuse `--profile` there; no Space merge request is looked up.
  [@test] ../../crates/bd-cli/src/safe_push_test.rs (`a_branch_with_an_open_pull_request_starts_from_a_comment`)
- Without the Space token of the Safe Push CLI it must be `not_logged_in`, and outside Space `not_on_space`, before a
  request to Patronus.
  [@test] ../../crates/bd-cli/src/safe_push_test.rs (`without_a_token_or_space_nothing_runs`)

### `bd request-review`

- `bd request-review <branch>` must ask GitHub users to review the open pull request of the virtual branch, as the
  app's Request Review… does ([Pull Requests](../branch/pull-requests.spec.md#request-review)): without `--reviewer`,
  one suggested code owner with a GitHub login of each group whose approval is missing and nobody is asked for yet,
  each once, never the author, a member asked already or one whose approval counts, from the review readiness
  `bd safe-push` checks (the pre-check with the pull request, asked again); `--reviewer <login>` (repeatable) asks
  those instead and doesn't ask Patronus. It must print `reviewers` (whom it asks), `requested`, `refused` with
  GitHub's reason, `waiting_for` (the reviewers the pull request asks after the request) and the `readiness` before it,
  each person with the full name Patronus knows. `--check` must print the same object and ask nobody. It must take no
  lock, run no sync and write nothing but the request on GitHub.
  [@test] ../../crates/bd-cli/src/request_review_test.rs (`asks_the_suggested_code_owner_of_the_missing_group_once`, `asks_the_logins_it_is_given_and_names_whom_github_refuses`)
- Without an open pull request, or with a draft, it must be `no_pull_request` (exit 4), naming `bd safe-push <branch>
  --create-pull-request`; a missing group nobody is asked for whose code owners have no GitHub login Patronus names,
  with nobody else to ask, `no_reviewer`; a request GitHub asked nobody in, `review_refused` naming each with the
  reason.
  [@test] ../../crates/bd-cli/src/request_review_test.rs (`a_branch_without_an_open_pull_request_or_a_code_owner_to_ask_is_refused`, `asks_the_logins_it_is_given_and_names_whom_github_refuses`)

### `bd robot`

- `bd robot <branch>` must print the robot of the virtual branch as `bd safe-push` finds it (a running one first, else
  the newest, by the branch or by the local or pushed tip of a temporary ref), or the one of `--robot`, with its
  status, mode, profile and whether it still runs; `robot: null` with exit 0 when the branch has none, and
  `unknown_branch` for a name that is neither a branch nor a branch with a robot. It must write no ref, take no lock
  and run no sync.
  [@test] ../../crates/bd-cli/src/robot_test.rs (`a_successful_robot_has_no_failure_and_a_branch_without_one_has_no_robot`, `a_failed_robot_says_why_and_which_local_commits_it_tested`)
- For a failed robot, and for a running one whose required check failed already, it must print the `failure` the
  app's Copy Prompt for Agent builds (`kind`, the failed checks with their build ids and counts, the error problems,
  the test and target revisions) and the commit map by the ids of the checked-out branch (`tested`, `rebased`,
  `changed`, `added`, `removed`, `landed`, `unknown`), `virtual_behind_head` for commits the last sync didn't build,
  and `head_branch`; no `failure` for any other robot.
  [@test] ../../crates/bd-cli/src/robot_test.rs (`a_failed_robot_says_why_and_which_local_commits_it_tested`, `a_running_robot_whose_required_check_failed_is_failing`, `a_successful_robot_has_no_failure_and_a_branch_without_one_has_no_robot`)
- `--full` must add the failed builds (no composite one) with their problems, failed tests and messages, and, through
  the `teamcity` CLI when it serves the build, the log tail of a build that failed not only on tests and whether a
  test failed before.
  [@test] ../../crates/bd-cli/src/robot_test.rs (`full_adds_the_failed_builds_their_tests_and_the_log_tails_of_the_teamcity_cli`)
- A newer robot of a branch stacked on it must win over the branch's own, named by `robot_branch`: it built this
  branch too. The commit map of a stacked branch's robot must have the commits of its ancestors too, each with its
  `branch`.
  [@test] ../../crates/bd-cli/src/robot_test.rs (`a_newer_robot_of_a_stacked_branch_tells_the_parent_with_the_commits_of_the_whole_stack`)
- `--robot` of a robot of another branch must be `invalid_arguments` naming that branch, unless that branch is stacked
  on it.
  [@test] ../../crates/bd-cli/src/robot_test.rs (`robot_takes_a_robot_by_id_of_the_branch_only`)
- Without the Space token it must be `not_logged_in`, and outside Space `not_on_space`, before a request to Patronus.
  [@test] ../../crates/bd-cli/src/robot_test.rs (`robot_without_a_token_or_space_asks_nothing`)

### `bd describe`

- `bd describe <branch>` must print the branch's `branch.<name>.description` as `{title, body}`, or `null`. The
  `descriptions` of a `bd group` plan set several at once.
- `--title`, `--body` and `--body-file` must set it as `title`, a blank line and the body, keeping the part not
  given; `--clear` must remove it. A description needs a title, and the branch must be a current group.
  [@test] ../../crates/bd-cli/src/describe_test.rs (`sets_prints_and_clears_a_description`, `describe_errors`)

### `bd version`

- `bd version` and `bd --version` must print `{"schema":1,"version","git"}` without a repository: the version of the
  app (the root `package.json`, which `tauri.conf.json` names), and the version of the git bd finds, `null` without one.
  [@test] ../../crates/bd-cli/src/version_test.rs (`version_and_dash_dash_version`)

### `bd help agent`

- It must print the agent workflow as text: `bd suggest`, `bd check` only after an edit of the plan, `bd group
  --sync` with descriptions for the branches that have none, `bd push` after the user approves, and `bd safe-push`
  with `--check` first and the start after the user approves, then `bd robot --full` after a failed Safe Push; one
  line names
  `bd update` and `bd fold` and sends a conflict to `bd help update`, one `bd move-hunk` for a change in the wrong
  commit, and the rules say that a change moves between commits only with `bd move-hunk`, and only `bd update` and its
  undo write the working tree, through the guarded update. Together with `bd help update` it must name every command
  and, per command, every long option except a listed few, and name no command, and no option for a command, that
  doesn't exist.
  [@test] ../../crates/bd-cli/src/agent_help_test.rs (`agent_help_names_every_command_and_option_that_exists`)
- Its first line must name the version.
  [@test] ../../crates/bd-cli/src/version_test.rs (`agent_help_names_the_version`)

### `bd install`

- `bd install` and Help → Install Agent CLI… must put a `bd` command where an agent finds it: on macOS and Linux a
  symlink in `~/.local/bin` to the app binary, replacing a symlink there and refusing anything else
  (`install_refused`); on Windows `bd.exe` next to the app.
- It must write the skill `branch-deck` into `~/.claude/skills/` and `~/.agents/skills/`: a description of when to use
  bd (grouping, and updating onto the baseline with its conflicts), its full path, `bd help agent` as the workflow and
  `bd help update` for an update; `bd help skill` must print the same text. Its `allowed-tools` must be
  `Bash(<bd>:*)`, `<bd>` as the body names it, so that Claude Code runs every bd command without a permission prompt.
  [@test] ../../crates/bd-cli/src/agent_help_test.rs (`the_skill_allows_every_bd_command_as_named`)
- It must say whether `bd` on `PATH` is the installed one; `--uninstall` must remove the skills and only its own
  symlink. In the app the toast "Agent CLI installed" says where agents find bd and how to run it, paths under the
  home directory as `~`, with "Copy Path" (the full path of `bd`); a failure is the error toast "Cannot install the
  Agent CLI".
  [@test] ../../crates/bd-cli/src/install_test.rs (`installs_the_command_and_the_skills_and_uninstalls_them`, `a_file_in_the_way_is_refused`)
- The symlink must outlive the running app: on Linux, with `APPIMAGE` set, it must point at the AppImage file, not at
  the binary in its temporary mount; on macOS an app run translocated (its path under `AppTranslocation/`) must be
  `install_refused` with "Branch Deck runs from a temporary location: move it to Applications, then install the Agent
  CLI".
  [@test] ../../crates/bd-cli/src/install_test.rs (`the_link_target_is_the_appimage_not_its_mount`, `install_links_to_the_appimage`, `a_translocated_app_is_refused`)
- Without the symlink to this binary, `bd help skill` must name the app binary with `cli`, never the bare app binary,
  which starts the app, quoted for the shell and to be run as written. The app's `agent_cli_command(repository)` must
  return the same command with `-C <repository>` without writing anything, quoted for the shell (POSIX quotes; on
  Windows PowerShell's `& '<bd.exe>'` with single quotes, and `powershell: true`): the symlink when it points at the running app, else
  the app binary with `cli` (`bd.exe` on Windows). A binary named `bd` (a cargo build) is the CLI itself and gets no
  `cli`. `bd update` names its next command (`session.next`) the same way. A path of the binary shorter than 260
  characters has no `\\?\` prefix.
  [@test] ../../crates/bd-cli/src/install_test.rs (`the_bd_command_is_the_link_to_this_app_or_the_app_with_cli`)
  [@test] ../../src/hooks/useInstallAgentCli.test.ts (`names the skills, and bd as a command when it is on PATH`, `names the full path and the directory to add to PATH when bd is not on it`)
