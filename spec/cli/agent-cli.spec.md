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
  - ../../crates/bd-cli/src/error.rs
  - ../../crates/bd-cli/src/output.rs
  - ../../crates/bd-cli/src/status.rs
  - ../../crates/bd-cli/src/branch_stats.rs
  - ../../crates/bd-cli/src/commits.rs
  - ../../crates/bd-cli/src/areas.rs
  - ../../crates/bd-cli/src/diff_scan.rs
  - ../../crates/bd-cli/src/numstat.rs
  - ../../crates/bd-cli/src/dependencies.rs
  - ../../crates/bd-cli/src/generated.rs
  - ../../crates/bd-cli/src/schema.rs
  - ../../crates/bd-cli/src/version.rs
  - ../../crates/bd-cli/build.rs
  - ../../crates/bd-cli/src/rewrite.rs
  - ../../crates/bd-cli/src/group_plan.rs
  - ../../crates/bd-cli/src/describe.rs
  - ../../crates/bd-cli/src/sync_command.rs
  - ../../crates/bd-cli/src/check_command.rs
  - ../../crates/bd-cli/src/update_command.rs
  - ../../crates/bd-cli/src/fold_command.rs
  - ../../crates/sync-core/src/update_project.rs
  - ../../crates/sync-core/src/autosquash.rs
  - ../../crates/bd-cli/src/suggest.rs
  - ../../crates/bd-cli/src/clustering.rs
  - ../../crates/sync-utils/src/issue_pattern.rs
  - ../../crates/bd-cli/src/push_command.rs
  - ../../crates/sync-core/src/remote_command.rs
  - ../../crates/sync-core/src/push_lease.rs
  - ../../crates/sync-core/src/push.rs
  - ../../crates/sync-core/src/check.rs
  - ../../crates/git-ops/src/merge_memo.rs
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
  - ../../crates/git-executor/src/git_info.rs
---

# Agent CLI

Status: Implemented
Date: 2026-10-15

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
  tree. Only `bd update` and its undo write the working tree, and only the files the update changes.

## Non-goals

- Reordering, squashing, dropping or splitting commits.

## Requirements

### Invocation

- `branch-deck cli <command>` and a binary named `bd` must run the CLI without starting Tauri.
- On Windows, where the app binary has no console, the installer must put a console `bd.exe` next to the app
  (`crates/bd-cli/src/main.rs`, bundled as an `externalBin` by `src-tauri/tauri.windows-bd.conf.json`).
- `-C <repo>` must select the repository, default the current directory; a subdirectory must resolve to the top level.
  [@test] ../../crates/bd-cli/src/status_test.rs (`subdirectory_resolves_to_top_level`)
- The branch prefix must come from `--prefix`, else the app setting `globalUserBranchPrefix`, else
  `git config branchdeck.branchPrefix`, and every output must echo it.
  [@test] ../../crates/bd-cli/src/status_test.rs (`prefix_from_app_settings`, `prefix_from_git_config`)
- Git must come from `BD_GIT`, else `git` on `PATH`, else Homebrew's git, never from a login shell, and be 2.54 or newer.
- Logs must go to stderr, and only with `-v` (debug) or `-vv` (trace).

### Output contract

- stdout must hold exactly one JSON object per run, and every object must have `"schema": 1`.
- `bd schema` must print the JSON Schema of the `status`, `status_brief`, `commits`, `commits_brief`, `rewrite`,
  `undo`, `check`, `suggest`, `sync`, `push`, `describe`, `install`, `update`, `fold`, `version` and `error` outputs,
  and the real outputs must use only the properties it declares.
  [@test] ../../crates/bd-cli/src/schema_test.rs (`schema_lists_every_output`, `outputs_match_schema`)
  [@test] ../../crates/bd-cli/src/update_test.rs (`update_outputs_match_the_schema`)
  [@test] ../../crates/bd-cli/src/fold_test.rs (`fold_is_refused_on_another_head_or_while_the_lock_is_held`)
- `status`, `commits`, `suggest`, `check`, `schema`, `version`, `update --dry-run` and `fold --dry-run` must write no
  ref, config, index or working tree: at most git objects (`update --dry-run` replays) and the object cache (none with
  `--no-cache`), and HEAD must stay; `bd --help` and `bd help agent` must say so in one sentence.
  [@test] ../../crates/bd-cli/src/agent_help_test.rs (`read_only_commands_write_no_ref_config_index_or_working_tree`)
  [@test] ../../crates/bd-cli/src/update_test.rs (`dry_run_neither_fetches_nor_writes`)
- A failure must print `{"schema":1,"error":{"code","message","commits","retryable"}}` with a stable snake_case code,
  and `paths` (the local files) for `uncommitted_overlap`.
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
- `--branch` must list one branch, `--unassigned` only the unassigned commits; `--limit`/`--offset` must page their
  commits, and the branches otherwise.
  [@test] ../../crates/bd-cli/src/status_test.rs (`status_filters_and_pages`)
- It must not run integration detection, an update check or a network command.
- Each branch must show its `origin` (`prefix`, `issue_group` or `issue`) and, for an issue group, its `issue_keys`;
  the output must list `idle_issue_groups`, the keys of the issue groups that group no commit on this HEAD, and
  `issue_group_problems`, the lines of the issue groups that are not used.
  [@test] ../../crates/bd-cli/src/status_test.rs (`issue_groups_in_status`)
- With stacked branches on, each branch must show its declared `parent`, `parent_state` (`active`, `integrated`,
  `unapplied` or `missing`), the `effective_base` or `blocked_by` a sync would use, and its stack `problems` with the
  commits that declare them, and the output must list the `forest` order, parents first; `bd commits` must group by
  the leaf as well.
  [@test] ../../crates/bd-cli/src/status_test.rs (`stacks_in_status`)
- Each branch with a `bd describe` text must show its title, read for all branches in one git process.
  [@test] ../../crates/bd-cli/src/describe_test.rs (`sets_prints_and_clears_a_description`)
- `--stats` must add per branch the distinct files, the generated files, added and deleted lines, the top three
  directories, the authors, the author date range, the issue keys and a diffstat of the synced branch against the sync
  base (`null` if it was never synced), for all branches in at most three more git processes.
  [@test] ../../crates/bd-cli/src/status_test.rs (`stats_per_branch`)
- `--brief` must print per branch only the name, the commit count, the issue key when it is not the name, and the
  newest subject, with stacked branches on also the parent, `blocked_by`, the problems and the forest, then the
  unassigned commits; it must not go with `--branch` or `--stats`.
  [@test] ../../crates/bd-cli/src/status_test.rs (`brief_status`, `brief_status_keeps_the_stack_state`)

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
- `bd commits <id>…` must keep the commits with these ids (full, or abbreviated to at least 4 characters) and
  `--unassigned` the unassigned commits, with the edges that involve them; an unknown id must be
  `not_ahead_of_baseline`, an ambiguous one `invalid_arguments`, and the filters and `--branch` must exclude each other.
  [@test] ../../crates/bd-cli/src/commits_test.rs (`commit_ids_and_unassigned_filter_the_listing`, `an_ambiguous_id_is_invalid`)
- `--brief` must print per commit only the id, the subject (without a prefix that is exactly the group; a stack path
  or a sanitized prefix stays), the group, the issue key when it is not the group, the fixup kind and target, and the
  areas; the dependencies as `{from, to}`; and `units`, the connected components of two or more commits of the
  dependencies and the fixups, over every commit, each oldest first.
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
- Several processes must use the cache at once; a full cache must be cleared; older schema versions must be dropped;
  any cache failure must be a miss.
  [@test] ../../crates/object-cache/src/cache_test.rs (`round_trip`, `older_schema_versions_are_dropped`, `unusable_cache_is_a_miss`, `full_cache_is_cleared`, `parallel_threads`, `parallel_processes`)

### `bd group`, `rename`, `restack`, `unstack`

- `bd group` must read a plan `{branches: {path: [ids]}, unassign: [ids], issue_groups: {name: [keys]},
  ungroup_issues: [keys]}` from stdin or `--plan`, set the prefixes of all its commits in one message-only rewrite,
  and change the issue groups in the same ref transaction ([Issue Groups](../branch/issue-groups.spec.md)); ids may
  be abbreviated. A plan of issue groups only rewrites no commit.
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
- Rewrites must hold `$GIT_COMMON_DIR/branch-deck.lock`, wait a bounded time for another holder, then fail with
  `locked` (exit 3). A rebase or merge in progress is `operation_in_progress` and a detached HEAD `detached_head`
  (exit 4).
  [@test] ../../crates/sync-core/src/regroup_test.rs (`waits_for_the_repository_lock`, `refuses_while_a_rebase_is_in_progress`)
- The output must list every commit's resulting group with its new id, the new HEAD and an undo token;
  `--dry-run` must write nothing and print the changed subjects and the stack forest; `--map` must add old → new ids.
  [@test] ../../crates/bd-cli/src/rewrite_test.rs (`dry_run_prints_the_changes_and_the_forest`)
  [@test] ../../crates/sync-core/src/regroup_test.rs (`dry_run_writes_nothing`)
- `bd rename` must replace the branch's level in every prefix path, children included, rename the issue groups of
  that name without a rewrite, and give an issue branch's commits a prefix; the new name must be a branch name without
  `/`.
  [@test] ../../crates/sync-core/src/regroup_test.rs (`renames_a_branch_with_its_children`, `renames_flat_and_issue_branches`)
- `bd restack` and `bd unstack` must set or clear the parent on all commits of the branch, and need stacked branches;
  an issue group is `invalid_plan`.
  [@test] ../../crates/sync-core/src/regroup_test.rs (`restacks_and_unstacks_a_branch`, `restack_needs_stacked_branches`)
  [@test] ../../crates/bd-cli/src/rewrite_test.rs (`rename_restack_unstack_and_undo`)

### `bd undo`

- `bd undo <undo_token>` must move the branch back to the token's previous HEAD with compare-and-swap, and refuse
  with `head_moved` once the branch changed again. A token of the issue groups (`groups:G1..G2`, or
  `H1..H2+groups:G1..G2` with a rewrite) must move them back in the same transaction, and refuse with `groups_moved`
  (exit 3, retryable) once they changed again.
  [@test] ../../crates/sync-core/src/regroup_test.rs (`undo_restores_the_previous_head`)
  [@test] ../../crates/bd-cli/src/rewrite_test.rs (`rename_restack_unstack_and_undo`, `issue_groups_plan_and_undo`)
  [@test] ../../crates/sync-core/src/issue_groups_test.rs (`a_mixed_plan_moves_head_and_the_store_in_one_transaction`)

### `bd update`

- `bd update` must do what the app's Update Project does ([Update Project](../repository/update-project.spec.md)):
  fetch the baseline, then, under the repository lock, check again and apply a clean update (`up_to_date`,
  `fast_forward`, `rebase`), the files following through the guarded working-tree update. It must print the state,
  the previous and the new HEAD, the `replayed` and `dropped_already_upstream` counts, the landed commits (`landed`)
  and the undo token `update:<previous head>..<new head>` with full ids.
  [@test] ../../crates/bd-cli/src/update_test.rs (`update_fast_forwards_and_the_undo_moves_the_files_back`, `update_rebases_the_local_commits_and_keeps_unrelated_local_changes`)
- The fetch must run before the lock and through the remote command policy; a failed fetch is `network_error`
  (exit 5, retryable). A held lock is `locked` (exit 3) after the fetch.
  [@test] ../../crates/bd-cli/src/update_test.rs (`a_failed_fetch_is_a_network_error`, `the_update_waits_for_the_repository_lock_after_its_fetch`)
- `--no-fetch` must update onto the last fetched baseline. `--dry-run` must imply `--no-fetch`, take no lock and
  print what the update would do, writing at most git objects.
  [@test] ../../crates/bd-cli/src/update_test.rs (`dry_run_neither_fetches_nor_writes`)
- A conflict must print `state: "conflict"` with the likely conflicting files, the local commits and the upstream
  commits that change them, change nothing, and exit with 1.
  [@test] ../../crates/bd-cli/src/update_test.rs (`a_conflict_reports_the_files_and_commits_and_changes_nothing`)
- Uncommitted, staged or untracked files on paths the update writes must be `uncommitted_overlap` (exit 4, not
  retryable) with those `paths`, also for a dry run; bd must never stash, reset, check out or commit them.
  [@test] ../../crates/bd-cli/src/update_test.rs (`uncommitted_changes_on_updated_files_block_with_their_paths`)
- `bd undo update:<previous head>..<new head>` must take only full ids and move the branch back with the files
  following, under the lock; it must refuse with `uncommitted_overlap` when local changes overlap the paths it writes,
  `head_moved` when the branch moved since, and `invalid_arguments` when the token is not the last update of the
  branch. It must print whether the landed report is the one from before the update again (`landed_restored`).
  [@test] ../../crates/bd-cli/src/update_test.rs (`the_undo_is_refused_when_local_changes_overlap_or_the_branch_moved`, `update_outputs_match_the_schema`)
  [@test] ../../crates/sync-core/src/update_project_test.rs (`undo_needs_the_token_of_the_last_update`, `undo_tokens_are_strict`)

### `bd fold`

- `bd fold` must fold the `fixup!`, `squash!` and `amend!` commits into their targets as the app's Squash Fixups does
  ([Squash Fixups](../commit/fixups.spec.md)), under the repository lock, keeping HEAD's tree, the index and the working
  tree, and print per target the fixups folded, the orphans, and an undo token `<previous head>..<new head>` for
  `bd undo`. `--branch` must fold only the fixups whose target is in that branch; `--dry-run` must write nothing;
  nothing to fold is no rewrite and no token.
  [@test] ../../crates/bd-cli/src/fold_test.rs (`fold_keeps_the_tree_and_the_undo_restores_the_head`, `fold_of_one_branch_and_a_dry_run`)
- `--expect-head` must refuse with `head_moved` on another HEAD, and the fold must check that HEAD, the planned
  commits and the history it rewrites all end at the planned head; a held lock is `locked`.
  [@test] ../../crates/bd-cli/src/fold_test.rs (`fold_is_refused_on_another_head_or_while_the_lock_is_held`)
  [@test] ../../crates/sync-core/src/autosquash_test.rs (`refuses_when_the_branch_moved_since_the_plan`)

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
  and areas, the subjects the plan rewrites, and the trial sync of the plan. It must write git objects and the cache
  only, and the plan, applied, must suggest nothing more.
  [@test] ../../crates/bd-cli/src/suggest_test.rs (`suggests_a_checked_plan_that_is_stable_once_applied`, `the_plan_moves_only_what_changes_and_is_a_regroup_plan`)
- Hard units must join the dependencies, a fixup and its target, the commits of one prefix (unless `--fresh`) and the
  commits without a prefix of one issue key. Two prefixes must never join, and a dependency between them must be an
  `anchor_dependency` warning.
  [@test] ../../crates/bd-cli/src/clustering_test.rs (`hard_units_join_dependencies_fixups_prefixes_and_keys`, `an_edge_between_two_anchors_is_a_warning_and_anchors_never_merge`)
  [@test] ../../crates/bd-cli/src/suggest_test.rs (`prefixes_that_depend_on_each_other_stay_apart_and_exit_with_1`)
- Soft merges must go by a deterministic integer score of issue key mentions, conventional scopes (`type(scope):` and
  `type scope:`), shared paths weighted by IDF, the disagreement of issue projects and scopes, and log adjacency: the
  best pair first, a tie to the oldest, under `--max-commits` (40) and down to the threshold 0.40.
  [@test] ../../crates/bd-cli/src/clustering_test.rs (`a_merge_that_only_adjacency_lifts_over_the_threshold_is_weak`, `the_same_scope_merges_commits_without_keys_and_another_scope_halves_paths`, `a_mention_links_keys_of_one_project_but_hardly_across_projects`, `shared_paths_merge_but_a_feature_of_many_units_or_a_peripheral_file_weighs_little`, `the_cap_stops_soft_merges_and_a_tie_goes_to_the_oldest_pair`)
  [@test] ../../crates/sync-utils/src/issue_pattern.rs (`semantic_prefixes_with_and_without_parentheses`, `issue_keys_anywhere_in_a_text`)
- With `--fresh`, on the anonymized input of a large monorepo branch, the clusters must match the grouping a human
  made with a pairwise precision of at least 0.95 and a recall of at least 0.85, and keep the issue projects and the
  named pairs of groups apart.
  [@test] ../../crates/bd-cli/src/clustering_test.rs (`golden_monorepo_branch`)
- A conflict of the check must join the hard unit of the conflicting commit to the units it needs, also inside an
  issue group, and cluster again, at most 5 times, never joining two prefixes; the exit code must be 1 when a branch
  still conflicts.
  [@test] ../../crates/bd-cli/src/clustering_test.rs (`repair_joins_a_conflicting_commit_to_what_it_needs`)
  [@test] ../../crates/bd-cli/src/suggest_test.rs (`a_conflict_in_a_lock_file_joins_the_commits_and_fresh_ignores_prefixes`, `a_conflict_inside_an_issue_group_puts_it_into_the_plan`, `prefixes_that_depend_on_each_other_stay_apart_and_exit_with_1`)
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
  the commit count, the new tip and the remote state, plus the unassigned count.
  [@test] ../../crates/bd-cli/src/sync_test.rs (`syncs_and_reports_each_branch`, `waits_for_the_lock`)
- A conflict must name the conflicting commit, its subject, the files and the commits the change likely needs, and the
  exit code must be 1.
  [@test] ../../crates/bd-cli/src/sync_test.rs (`a_conflict_exits_with_1`)
- The integration results of archived branches and the update check of the checked out branch must be included
  compactly, with the landed commits (`update.landing`, `landed`).
  [@test] ../../crates/bd-cli/src/sync_test.rs (`reports_the_commits_the_baseline_already_has`)

### `bd push`

- `bd push <branch>…` must sync first under the repository lock, then push the branches in one atomic
  `git push --porcelain` through the remote command policy, parents first, with an explicit lease per ref; `--all`
  must need `--yes`.
  [@test] ../../crates/bd-cli/src/push_test.rs (`pushes_with_a_lease_and_prints_the_pull_request`, `all_needs_yes_and_unsynced_branches_are_refused`)
  [@test] ../../crates/sync-core/src/push_lease_test.rs (`parses_porcelain_lines`)
- It must refuse (`push_refused`, exit 4, nothing pushed) a branch that didn't sync, one behind its remote branch
  without `--force`, and a stacked branch whose parent is neither on the remote nor pushed with it.
  [@test] ../../crates/bd-cli/src/push_test.rs (`parents_go_first_and_a_child_needs_its_parent_on_the_remote`, `all_needs_yes_and_unsynced_branches_are_refused`)
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
  the head, the `bd describe` title and body, and a ready `gh pr create` command.
  [@test] ../../crates/bd-cli/src/push_test.rs (`pushes_with_a_lease_and_prints_the_pull_request`, `parents_go_first_and_a_child_needs_its_parent_on_the_remote`)

### `bd describe`

- `bd describe <branch>` must print the branch's `branch.<name>.description` as `{title, body}`, or `null`. The
  `descriptions` of a `bd group` plan set several at once.
- `--title`, `--body` and `--body-file` must set it as `title`, a blank line and the body, keeping the part not
  given; `--clear` must remove it. A description needs a title, and the branch must be a current group.
  [@test] ../../crates/bd-cli/src/describe_test.rs (`sets_prints_and_clears_a_description`, `describe_errors`)

### `bd version`

- `bd version` and `bd --version` must print `{"schema":1,"version","git"}` without a repository: the version of the
  app (`src-tauri/tauri.conf.json`), and the version of the git bd finds, `null` without one.
  [@test] ../../crates/bd-cli/src/version_test.rs (`version_and_dash_dash_version`)

### `bd help agent`

- It must print the agent workflow as text: `bd suggest`, `bd check` only after an edit of the plan, `bd group
  --sync` with descriptions for the branches that have none, and `bd push` after the user approves; one line names
  `bd update` and `bd fold`, and the rules say that only `bd update` and its undo write the working tree. It must name
  every command and, per command, every long option except a listed few, and name no command, and no option for a
  command, that doesn't exist.
  [@test] ../../crates/bd-cli/src/agent_help_test.rs (`agent_help_names_every_command_and_option_that_exists`)
- Its first line must name the version.
  [@test] ../../crates/bd-cli/src/version_test.rs (`agent_help_names_the_version`)

### `bd install`

- `bd install` and Help → Install Agent CLI… must put a `bd` command where an agent finds it: on macOS and Linux a
  symlink in `~/.local/bin` to the app binary, replacing a symlink there and refusing anything else
  (`install_refused`); on Windows `bd.exe` next to the app.
- It must write the skill `branch-deck` into `~/.claude/skills/` and `~/.agents/skills/`: a description of when to use
  bd, its full path, and `bd help agent` as the workflow; `bd help skill` must print the same text.
- It must say whether `bd` on `PATH` is the installed one; `--uninstall` must remove the skills and only its own
  symlink. In the app the toast "Agent CLI installed" says where agents find bd and how to run it, with "Copy Path"
  (the full path of `bd`); a failure is the error toast "Cannot install the Agent CLI".
  [@test] ../../crates/bd-cli/src/install_test.rs (`installs_the_command_and_the_skills_and_uninstalls_them`, `a_file_in_the_way_is_refused`)
- Without the symlink to this binary, `bd help skill` must name the app binary with `cli`, never the bare app binary,
  which starts the app. The app's `agent_cli_command` must return the same command without writing anything, quoted
  for a shell: the symlink when it points at the running app, else the app binary with `cli` (`bd.exe` on Windows).
  [@test] ../../crates/bd-cli/src/install_test.rs (`the_bd_command_is_the_link_to_this_app_or_the_app_with_cli`)
  [@test] ../../src/hooks/useInstallAgentCli.test.ts (`names the skills, and bd as a command when it is on PATH`, `names the full path and the directory to add to PATH when bd is not on it`)
