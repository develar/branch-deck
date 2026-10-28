---
name: Agent CLI
description: Requirements for bd, the command line a coding agent uses to read the branches and commits Branch Deck sees and to regroup the commits, as JSON with a stable contract.
style: plain-1
targets:
  - ../../crates/bd-core/src/text_input.rs
  - ../../crates/bd-remote/src/operation_command.rs
  - ../../crates/operations/src/journal.rs
  - ../../crates/bd-core/src/operation_record.rs
  - ../../src-tauri/src/main.rs
  - ../../crates/bd-cli/src/main.rs
  - ../../scripts/build-bd.mjs
  - ../../src-tauri/tauri.windows-bd.conf.json
  - ../../crates/bd-cli/src/lib.rs
  - ../../crates/bd-core/src/context.rs
  - ../../crates/sync-core/src/branch_prefix.rs
  - ../../crates/app-settings/src/resolve.rs
  - ../../crates/app-settings/src/file.rs
  - ../../crates/bd-core/src/error.rs
  - ../../crates/domain-types/src/error.rs
  - ../../crates/bd-core/src/output.rs
  - ../../crates/bd-remote/src/status.rs
  - ../../crates/bd-core/src/branch_view.rs
  - ../../crates/bd-remote/src/branch_stats.rs
  - ../../crates/bd-rewrite/src/commits.rs
  - ../../crates/commit-clustering/src/areas.rs
  - ../../crates/trial-check/src/diff_scan.rs
  - ../../crates/bd-remote/src/numstat.rs
  - ../../crates/trial-check/src/dependencies.rs
  - ../../crates/commit-clustering/src/generated.rs
  - ../../crates/bd-cli/src/schema.rs
  - ../../crates/bd-core/src/version.rs
  - ../../crates/bd-core/build.rs
  - ../../crates/bd-rewrite/src/rewrite.rs
  - ../../crates/bd-update/src/undo_command.rs
  - ../../crates/bd-rewrite/src/group_plan.rs
  - ../../crates/bd-rewrite/src/describe.rs
  - ../../crates/git-ops/src/branch_description.rs
  - ../../crates/bd-core/src/sync_command.rs
  - ../../crates/bd-core/src/stack_summary.rs
  - ../../crates/bd-core/src/next_step_summary.rs
  - ../../crates/sync-core/src/branch_processor.rs
  - ../../crates/bd-rewrite/src/check_command.rs
  - ../../crates/bd-core/src/check_summary.rs
  - ../../crates/git-ops/src/tree_archive.rs
  - ../../crates/bd-remote/src/robot_command.rs
  - ../../crates/bd-remote/src/request_review_command.rs
  - ../../crates/bd-rewrite/src/fold_command.rs
  - ../../crates/bd-rewrite/src/move_hunk_command.rs
  - ../../crates/trial-check/src/move_hunk.rs
  - ../../crates/git-ops/src/move_hunk.rs
  - ../../crates/git-ops/src/merge_tree_output.rs
  - ../../crates/history-rewrite/src/autosquash.rs
  - ../../crates/bd-rewrite/src/suggest.rs
  - ../../crates/bd-core/src/suggest_plan.rs
  - ../../crates/commit-clustering/src/clustering.rs
  - ../../crates/sync-utils/src/issue_pattern.rs
  - ../../crates/sync-utils/src/list_cut.rs
  - ../../crates/bd-remote/src/push_command.rs
  - ../../crates/sync-core/src/remote_command.rs
  - ../../crates/sync-core/src/push_lease.rs
  - ../../crates/sync-core/src/push.rs
  - ../../crates/operations/src/push.rs
  - ../../crates/operations/src/pull_request.rs
  - ../../crates/bd-remote/src/safe_push_command.rs
  - ../../crates/bd-remote/src/safe_push_run.rs
  - ../../crates/bd-remote/src/land_command.rs
  - ../../crates/operations/src/land_execute.rs
  - ../../crates/operations/src/robot_start.rs
  - ../../crates/operations/src/registry_file.rs
  - ../../crates/bd-core/src/review_summary.rs
  - ../../crates/bd-remote/src/push_refusals.rs
  - ../../crates/remote-facts/src/stack_pull_request.rs
  - ../../crates/remote-facts/src/patronus_review.rs
  - ../../crates/remote-facts/src/review_readiness.rs
  - ../../crates/sync-core/src/patronus_start.rs
  - ../../crates/domain-policy/src/robot_match.rs
  - ../../crates/trial-check/src/check.rs
  - ../../crates/git-ops/src/merge_memo.rs
  - ../../crates/git-ops/src/cherry_pick.rs
  - ../../crates/sync-core/src/cache_memo.rs
  - ../../crates/object-cache/src/conflict_info.rs
  - ../../crates/object-cache/src/remote_status.rs
  - ../../crates/object-cache/src/conflict_chain.rs
  - ../../crates/sync-core/src/conflict_chain.rs
  - ../../crates/object-cache/src/build_result.rs
  - ../../crates/history-rewrite/src/regroup.rs
  - ../../crates/history-rewrite/src/reword.rs
  - ../../crates/history-rewrite/src/fixup_targets.rs
  - ../../crates/bd-rewrite/src/reword_command.rs
  - ../../crates/history-rewrite/src/issue_groups.rs
  - ../../crates/history-rewrite/src/commit_ids.rs
  - ../../crates/history-rewrite/src/repository_lock.rs
  - ../../crates/bd-cli/src/agent_help.rs
  - ../../crates/bd-cli/src/install.rs
  - ../../crates/bd-core/src/bd_command.rs
  - ../../src-tauri/src/commands/agent_cli.rs
  - ../../src/hooks/useInstallAgentCli.ts
  - ../../crates/sync-core/src/grouping.rs
  - ../../crates/object-cache/src/cache.rs
  - ../../crates/object-cache/src/commit_diff.rs
  - ../../crates/git-ops/src/notes.rs
  - ../../crates/git-ops/src/copy_commit.rs
  - ../../crates/trial-check/src/dangling_references.rs
  - ../../crates/trial-check/src/declaration_checks.rs
  - ../../crates/trial-check/src/dangling_fix.rs
  - ../../crates/trial-check/src/inverted_stack.rs
  - ../../crates/trial-check/src/missing_declarations.rs
  - ../../crates/trial-check/src/landing_gate.rs
  - ../../crates/domain-types/src/landing.rs
  - ../../crates/git-ops/src/object_texts.rs
  - ../../crates/object-cache/src/commit_lines.rs
  - ../../crates/git-executor/src/git_info.rs
  - ../../crates/bd-remote/src/landed.rs
  - ../../crates/bd-remote/src/robot_list.rs
  - ../../crates/sync-core/src/robot_landing.rs
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
- Reading changes nothing; a regroup changes commit messages only. A fold and `bd move-hunk` change the trees of
  commits ahead of the baseline, never HEAD's tree, the index or the working tree. Only `bd update`, `bd pick`, `bd unapply`, `bd reapply` and their undo write the working tree, through the guarded update. `bd lease` writes only the slots of its pool.

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
  [@test] ../../crates/bd-remote/src/status_test.rs (`subdirectory_resolves_to_top_level`)
- The branch prefix must come from `--prefix`, else the specific one first: `branchdeck.branchPrefix` of the
  repository's local or worktree config or `git -c`, else the app setting `globalUserBranchPrefix`, else the global or system
  `branchdeck.branchPrefix` (`app_settings::resolve`), each named as the branches are (`jane/` is `jane`); every
  output must echo it, and `bd status` must name its source (`prefix_source`: `cli`, `repository_config`,
  `app_setting`, `global_config`), so that a fallback to the global config is explicit.
  [@test] ../../crates/bd-remote/src/status_test.rs (`prefix_from_app_settings`, `prefix_from_git_config`, `prefix_source_names_where_the_prefix_came_from`, `the_global_config_of_the_process_never_reaches_the_bd_of_a_test`)
  [@test] ../../crates/app-settings/src/resolve_test.rs (`the_specific_prefix_wins`, `the_prefix_of_the_command_line_is_taken_as_it_is_named`)
  [@test] ../../crates/sync-core/src/branch_prefix_test.rs (`a_local_prefix_is_repository_scoped`)
- The settings of the app (its prefix, `patronusNewApi`) must be read once as `bd` starts, through the loader of the
  app (`app_settings::file::SettingsFile::load`), with the same defaults: a missing file, one that does not parse and a
  field of the wrong type are the defaults. The app writes the file only by a rename, so a read beside that write gets
  the old or the new settings, never a cut file, and never fails the write (on Windows the rename replaces a file that
  `bd` has open, `sync_utils::atomic_file`); a sync of the app writes only `repository-state.json` beside it, so `bd`
  beside a sync reads the same prefix.
  [@test] ../../crates/bd-core/src/context_test.rs (`the_new_patronus_api_setting_of_the_app_is_read_from_its_settings_file`, `the_prefix_of_the_app_is_read_from_its_settings_file`, `a_settings_file_that_is_cut_gives_the_defaults`)
  [@test] ../../crates/app-settings/src/service_test.rs (`a_reader_of_the_file_never_sees_a_missing_prefix_while_the_service_writes_it`)
- Git must come from `BD_GIT`, else `git` on `PATH`, else Homebrew's git, never from a login shell, and be 2.54 or newer.
  [@test] ../../crates/git-executor/src/git_info.rs (`minimum_version`, `a_missing_explicit_git_is_not_found_with_the_hint`)
- Logs must go to stderr, and only with `-v` (debug) or `-vv` (trace). Every line of a run must name its operation,
  the lines of the threads a sync builds its branches on too: the root span
  `operation{op=<command> operation_id=<ULID>}` (`OperationId::span`), `op` the words of the command (`status`,
  `lease get`); the JSON output carries the same id (`operation_id`). Every closure or future the cores hand to another
  thread carries the span of its caller.
  [@test] ../../crates/bd-cli/src/operation_test.rs (`every_log_line_of_a_command_names_its_operation_whose_id_its_json_carries`, `the_operation_of_a_nested_command_names_both_words`)
  [@test] ../../crates/git-executor/src/thread_span_scan_test.rs (`every_closure_or_future_handed_to_another_thread_carries_the_span_of_its_caller`)

### Output contract

- stdout must hold exactly one JSON object per run, and every object must have `"schema": 2` and, after its own
  fields, the `operation_id` of the run (a ULID, new for each run, an error object and `bd --version` included): the
  id of the root span of its log lines, and of its record in the operations registry and its entry in the journal
  when it runs an operation of the remote, a rewrite, an update or a pick that writes.
  [@test] ../../crates/bd-cli/src/version_test.rs (`version_and_dash_dash_version`)
  [@test] ../../crates/bd-cli/src/schema_test.rs (`schema_lists_every_output`)
  [@test] ../../crates/bd-cli/src/operation_test.rs (`every_log_line_of_a_command_names_its_operation_whose_id_its_json_carries`)
- The JSON must be ASCII: every other character is a `\uXXXX` escape (a UTF-16 pair above the BMP), so a reader that
  decodes stdout with a legacy code page, as Windows PowerShell 5.1 does, still gets every path and subject.
  [@test] ../../crates/bd-rewrite/src/describe_test.rs (`a_body_from_windows_powershell_is_read_and_printed_as_ascii`)
- A plan, a body and a message (`--plan`, `--body-file`, `--message-file`; `-` is stdin) must be read as UTF-8 without a leading byte order
  mark, or as UTF-16 with one, which Windows PowerShell 5.1 writes.
  [@test] ../../crates/bd-core/src/text_input_test.rs (`a_bom_is_dropped_and_utf16_is_decoded`)
  [@test] ../../crates/bd-rewrite/src/check_test.rs (`a_plan_with_a_byte_order_mark_is_read`)
- `bd schema --all` must print the JSON Schema of every output (the list is the test's), and the real outputs must use
  only the properties it declares.
  [@test] ../../crates/bd-cli/src/schema_test.rs (`schema_lists_every_output`, `outputs_match_schema`)
  [@test] ../../crates/bd-update/src/update_test.rs (`update_outputs_match_the_schema`)
  [@test] ../../crates/bd-rewrite/src/fold_test.rs (`fold_is_refused_on_another_head_or_while_the_lock_is_held`)
  [@test] ../../crates/bd-rewrite/src/move_hunk_test.rs (`the_fix_moves_into_the_commit_it_belongs_to_and_the_sync_rebuilds_the_branch`, `a_commit_between_on_the_same_lines_conflicts_and_another_head_or_the_lock_refuse`)
- `bd schema` without a name must print the index of every command and the names of its outputs, under 4 KB, so that an
  agent never reads every schema for one field; every command but `help` and `schema` must name outputs that exist,
  and together they every output but `error`. `bd schema <command or output>...` must print only those schemas
  (`group`: `rewrite`), and an unknown name must be `invalid_arguments` naming every command and output.
  [@test] ../../crates/bd-cli/src/schema_test.rs (`schema_without_a_name_is_the_index_of_the_commands`, `schema_of_a_command_is_only_its_outputs`)
- `status`, `commits`, `suggest`, `check`, `robot`, `schema`, `version`, `describe` without a setter,
  `update --status` without `--materialize`, `lease status`, and the `--dry-run` of `group`, `rename`,
  `restack`, `unstack`, `update` (also with `--continue`), `pick`, `fold`, `move-hunk` and `reword` must write no ref,
  config,
  index or working tree: at most git objects (a trial sync, a replay, the trees of a move) and the object cache (none
  with `--no-cache`), and HEAD must stay; `bd --help` and `bd help agent` must say so, and that `push --dry-run`,
  `safe-push --check` and `request-review --check` sync first.
  [@test] ../../crates/bd-cli/src/agent_help_test.rs (`read_only_commands_write_no_ref_config_index_or_working_tree`)
  [@test] ../../crates/bd-update/src/update_test.rs (`dry_run_neither_fetches_nor_writes`)
  [@test] ../../crates/bd-remote/src/robot_test.rs (`a_failed_robot_says_why_and_which_local_commits_it_tested`)
- A failure must print `{"schema":2,"error":{"code","message","commits","retryable"}}` with a stable snake_case code,
  and `paths` for `uncommitted_overlap` (the local files) and for a refused resolution of `bd update --continue` (the
  conflicting files).
  A write whose step after the write failed must print its output with that error as `after_error`, never the error
  object, so that the undo token stays. The names must be one vocabulary: a write prints `previous_head` and
  `new_head`, a commit and its copy are `{commit, new_id}`, a state is `state`.
  [@test] ../../crates/bd-rewrite/src/rewrite_test.rs (`a_failed_description_write_is_the_error_of_the_output_and_keeps_the_undo_token`)
  [@test] ../../crates/bd-cli/src/schema_test.rs (`outputs_match_schema`)
- A refusal must name in its `message` the condition of every way past it that it offers, next to it: a flag that
  starts anyway (`--allow-dangling`, `--allow-missing`: only when the user says it is a false alarm;
  `--allow-unknown-rules`, `--without-approvals`: only when the user says so), one that drops, cancels or replaces
  (`--force`, `--cancel-running`, `--restart-running`, `--confirm-preserved`: once the user approved), and a limit
  that is the user's (`push_too_large`: never raise `branchdeck.pushSizeLimit` yourself; `low_disk_space`: never
  pass `--min-free 0` on your own). A refusal that no flag gets past must say what to do: show the user the answer of
  the remote, `bd sync` for a branch not synced, `bd update` for a branch that landed, tell the user of a parked
  branch and never unpark it, move fewer blocks of a `hunk_conflict`, sign in again for a token GitHub rejected.
  [@test] ../../crates/bd-remote/src/push_test.rs (`a_push_over_the_size_limit_is_push_too_large_before_the_remote`, `a_push_the_remote_refuses_with_its_leases_holding_is_push_refused`)
  [@test] ../../crates/bd-remote/src/safe_push_test.rs (`a_dangling_reference_refuses_the_safe_push_unless_allowed`, `rules_github_does_not_answer_refuse_the_safe_push_unless_allowed`)
  [@test] ../../crates/bd-lease/src/lease_test.rs (`a_get_below_the_minimum_free_space_refuses_and_names_it`)
  [@test] ../../crates/bd-rewrite/src/move_hunk_test.rs (`a_commit_between_on_the_same_lines_conflicts_and_another_head_or_the_lock_refuse`)
  [@test] ../../crates/bd-core/src/check_summary_test.rs (`a_dangling_reference_message_describes_two_whole_and_counts_from_three_on`)
- The error object must carry `next_steps`, the commands that act on the refusal, ready to run (`bd -C <repository>`
  as the other printed commands), each `{command, needs_approval}`, and the same in each `refusals[]` of
  `bd safe-push --check`; it is left out when there is none. A step that pushes, rewrites, updates or cancels or
  restarts a robot (`push`, `safe-push` and `request-review` without `--check`, `update`, `restack`, `--force`,
  `--cancel-running`, `--restart-running`) must have `needs_approval: true`, and runs only once the user approved it;
  one that only reads (`bd robot`, `bd sync`, `safe-push --check`) has it false. A suppression (`--allow-*`,
  `--without-approvals`, `--min-free`, a change of the config) must never be a step: it stays in `message` with its
  condition. The steps: `lease_rejected` the leased push that replaces the remote branch, `push_refused` of a branch
  behind its remote the push that replaces it and of a branch not synced `bd sync`, `robot_running` `bd robot` of the
  robot and the `--cancel-running` or `--restart-running` of the same command, `approvals_missing` without a pull
  request the `--create-pull-request` of the start, `dangling_reference` the `fix.command` a trial build proved,
  `missing_declaration` the `bd restack` it names, `push_dismisses_approvals` the push and then the review request,
  `target_conflict` and `landed` `bd update`, `stack_conflict` the `--check` of the branch whose commit conflicts,
  `stack_not_built` `bd sync`. It is named `next_steps`, never `next`, whose commands (`bd workspace`, `bd update`,
  `bd pick`) run at once.
  [@test] ../../crates/bd-cli/src/error_test.rs (`a_next_step_that_writes_needs_the_approval_and_none_starts_anyway`)
  [@test] ../../crates/bd-remote/src/push_test.rs (`a_moved_remote_branch_is_fetched_and_reported_and_force_replaces_it`, `all_needs_yes_and_unsynced_branches_are_refused`)
  [@test] ../../crates/bd-remote/src/safe_push_test.rs (`a_robot_on_the_last_commit_refuses_and_one_on_an_older_commit_is_canceled_when_told`, `push_refuses_a_branch_whose_safe_push_runs_and_restarts_it_when_told`, `missing_approvals_refuse_the_safe_push_unless_told_to_start_anyway`, `the_approvals_count_in_the_pull_request_it_opens_and_the_safe_push_starts_from_it`, `a_push_that_dismisses_the_approvals_patronus_counts_refuses_the_safe_push`, `a_name_another_branch_adds_refuses_the_safe_push_unless_allowed`, `a_whole_stack_that_conflicts_is_refused_with_the_conflict`)
  [@test] ../../crates/bd-remote/src/landed_test.rs (`push_and_safe_push_refuse_a_branch_a_merged_safe_push_landed_whole`)
- The exit code must follow the code: 0 ok, 1 conflict, 2 invalid input, 3 stale (retry), 4 refused, 5 git or network
  error. Exit 1 must never be an error object: it is the output of a command that found a conflict (`state:
  "conflict"`, a branch not built, `ok: false`); a conflict bd refuses to work around is exit 4. `retryable` must be
  true only where the same command again can succeed: never for `lease_rejected` (a push
  again with `--force` would drop what the remote has now) nor for a stale undo token (`undo_stale`). It must be the
  `retry` class of the code other than `no`, the one the app's `ApiError` carries: `same` for `locked` and
  `network_error`, `after_reread` for the stale reads, `plan_changed` among them (a remote action planned over facts
  that changed before its first effect did nothing, and its plan is read again). The codes are one registry with the app: `canceled` (an action
  the user stopped) exits with 4, `internal` (a failure without a code of its own, which the app answers; `bd` prints a
  failure of git or IO as `git_error`) with 5.
  [@test] ../../crates/bd-cli/src/error_test.rs (`every_error_code_has_the_exit_code_and_the_retry_of_its_class`)
  [@test] ../../crates/domain-types/src/error_test.rs (`an_error_without_a_code_of_its_own_is_internal_and_canceled_is_a_code`, `an_error_carries_the_retry_of_its_code`)
- A push, a Safe Push or a Dry Run and a review request (`bd push`, `bd safe-push`, `bd request-review`) must be
  recorded in the operations registry of the repository before its run, and must be `busy` (exit 3, retryable,
  `retry: same`), nothing pushed or started, while an operation of another process (the app, another `bd`, in any
  worktree of the repository) runs the same intent or holds what it needs: a branch it lands or pushes, a robot, a
  field of a pull request. `error.holder` must name that operation (`op`, `tool`, `pid`, `kind`, `subject`,
  `held_for`), the message who runs what. An operation of another branch must run beside it, and the record of an
  operation must leave the registry once it ended, or at the next admission once its process is gone. The app's
  operation is such a holder too, and the app refuses its own the same way while `bd` holds what it needs.
  [@test] ../../crates/bd-cli/src/cross_process_test.rs (`a_safe_push_beside_the_safe_push_of_another_process_is_busy_with_its_holder`)
  [@test] ../../crates/bd-cli/src/cross_process_test.rs (`bd_beside_the_apps_safe_push_of_the_same_branch_is_busy_naming_the_app`, `the_apps_safe_push_beside_bd_of_the_same_branch_is_busy_naming_bd`)
  [@test] ../../crates/operations/src/registry_file_test.rs (`the_file_is_busy_while_the_holder_lives_and_an_entry_takes_its_record_out`, `a_gone_process_of_this_host_leaves_at_once_and_one_of_another_host_after_five_minutes`, `a_registry_of_another_schema_is_busy_and_never_written`)
- A rewrite that writes (`bd group`, `rename`, `restack`, `unstack`, `reword`, `fold`, `park`, `unpark`, `unapply`,
  `reapply`, `undo`, `move-hunk`), `bd update` and `bd pick` must be recorded in the operations registry of the
  repository before the command takes any lock, as the intent of the app's family (`rewrite`, `updateProject`,
  `pick`), so the app's windows show it on its rows as `bd`'s; its record must leave once the command ended, before
  its `--sync`, or at the next admission once its process is gone. HEAD, which it holds, must make no other process
  `busy`, nor the same rewrite of another process: the repository lock orders them, and a rewrite of the app meanwhile
  is waited for. Two Unstacks of a branch in the app and `bd unstack` of it beside them must be one rewrite of the app
  and nothing more of `bd` (`invalid_plan` once it got the lock). A dry run, `bd update --status` and `bd check --fix`
  must record nothing.
  [@test] ../../crates/bd-cli/src/cross_process_test.rs (`the_triple_unstack_of_the_app_and_bd_is_one_rewrite_journaled_once_by_each`)
  [@test] ../../crates/bd-cli/src/cross_process_test.rs (`a_bd_killed_in_its_rewrite_leaves_a_record_the_app_drops_once_its_process_is_gone`)
  [@test] ../../crates/domain-policy/src/admission_test.rs (`a_rewrite_waits_for_the_rewrites_of_its_process_and_for_none_of_another`)
- The run of such an operation must be journaled once it ended, in the journal of the operations of the repository
  (`$GIT_COMMON_DIR/branch-deck/journal.jsonl`, the app's operations in the same file) under the `operation_id` of the
  JSON: `tool: bd`, its pid, worktree and prefix, whether the worktree is a slot of the lease pool, the intent and its
  subject, each effect with its status, what they answered or the error, and what the run cost from its admission (git
  processes by subcommand, HTTP requests, milliseconds). A run that ended without an answer is journaled as
  `internal`; a start whose answer was lost is journaled again, under its id, once a read of Patronus settled it
  (`started`, `notStarted`) or it was forgotten (`forgotten`). The journal keeps the newest 200 entries of the last
  30 days.
  [@test] ../../crates/bd-cli/src/operation_test.rs (`a_safe_push_is_journaled_under_the_operation_id_of_its_json`, `a_rewrite_is_journaled_under_the_operation_id_of_its_json`)
  [@test] ../../crates/operations/src/journal_test.rs (`an_operation_of_the_app_and_one_of_bd_are_journaled_once_each_with_what_they_cost`, `a_start_whose_answer_was_lost_is_journaled_again_once_settled_or_forgotten`, `a_dropped_operation_is_journaled_as_internal_and_a_discarded_one_not_at_all`)
  [@test] ../../crates/operations/src/journal_test.rs (`an_operation_in_a_slot_of_the_lease_pool_is_journaled_as_an_agents`, `the_journal_keeps_the_newest_entries_of_the_last_thirty_days`)
- A start of `bd safe-push` (and of the app) must record what it sends before its request goes out, and an answer
  lost after it went out must leave it unknown, never sent again: the output says so in `start_unknown` (`operation`,
  `reason`), and its record holds what it held for every process. A start of a process that is gone (killed between
  its push and its start) is such a start too. An admission that one holds must settle it first by a read of Patronus:
  a robot of it started (`plan_changed`), none once 30 s passed since it was sent (it leaves, and the run goes as
  planned, pushing nothing pushed already), else `busy` with `holder.unknown: true` and a message naming `bd
  operation forget <op>`. `bd operation forget <op>` must take that record out, only when the user says so
  (`forgotten`, `subject`, `source_ref`; `invalid_arguments` for an id the registry lacks or an operation that runs),
  and the next start must go.
  [@test] ../../crates/bd-cli/src/cross_process_test.rs (`a_lost_start_holds_bd_until_bd_operation_forget_lets_it_go`)
  [@test] ../../crates/bd-cli/src/cross_process_test.rs (`a_bd_killed_between_its_push_and_its_start_leaves_a_lost_start_the_app_settles_and_starts_once`)
  [@test] ../../crates/operations/src/reconcile_test.rs (`a_start_whose_answer_was_lost_is_unknown_once_and_holds_until_patronus_lists_its_robot`)
- Every command of `bd` that succeeded and is no read (not in the read-only line of `bd --help`: a rewrite, a push, a
  start, a sync, an update, a pick, a park, a description, the lease commands but `lease status`) must touch the stamp
  of its repository once, after its run (`$GIT_COMMON_DIR/branch-deck/stamp`, its Unix time and pid): the app's windows
  of the repository sync then without a focus. A read, a dry run and a command that failed must leave it as it was.
  [@test] ../../crates/bd-cli/src/cross_process_test.rs (`bd_moves_the_stamp_of_its_repository_after_a_mutation_only`)
- A typed error of a core must have one code in `bd` and in the app: the type names it (`ParkError::code`,
  `GuardRefusal::code`, `UnapplyRefusal::code`, `ReapplyRefusal::code`), local changes in the way of an unapply, a
  reapply or their undo are `uncommitted_overlap`, and an undo answers the code `in_undo` (`ErrorCode::in_undo`): a
  ref that moved on since the operation is `undo_stale`, never retried, in `bd undo` and in the undo commands of the
  app alike, its words saying what moved and that it can't be undone any more, never the advice to retry that a
  rewrite refused by the same move gives. Both sides are tested on one table of the typed errors and their codes
  (`update_core::error_cases`).
  [@test] ../../crates/bd-update/src/error_code_test.rs (`bd_answers_the_code_of_each_refusal_of_a_move_an_unapply_and_a_reapply_that_the_app_answers`, `bd_undo_answers_the_code_of_each_refusal_that_the_app_answers_and_a_moved_head_makes_it_stale`, `an_error_without_a_code_is_git_error_in_bd`)
  [@test] ../../crates/bd-rewrite/src/park_error_test.rs (`bd_answers_the_code_of_each_refusal_of_a_park_that_the_app_answers`, `a_park_that_fails_without_a_code_is_git_error_in_bd`)
  [@test] ../../crates/app-api/src/api_error_test.rs (`api_error_answers_the_code_of_each_typed_error_that_bd_answers_and_an_undo_its_code_in_undo`, `a_stale_undo_says_it_cannot_be_undone_instead_of_advising_a_retry`)
  [@test] ../../crates/bd-rewrite/src/rewrite_test.rs (`rename_restack_unstack_and_undo`)
  [@test] ../../crates/domain-types/src/error_test.rs (`an_undo_that_a_moved_ref_refuses_is_stale_and_never_retried`)
  [@test] ../../crates/test-server/src/reword_test.rs (`the_undo_of_a_regroup_after_a_new_commit_answers_undo_stale_never_retried`)
  [@test] ../../crates/test-server/src/squash_test.rs (`the_undo_of_a_squash_after_a_new_commit_answers_undo_stale_never_retried`)
- An empty prefix must fail with `no_branch_prefix`, a path outside a repository with `not_a_repository`, bad arguments
  with `invalid_arguments`, an unknown `--branch` with `unknown_branch`; all exit with 2.
  [@test] ../../crates/bd-remote/src/status_test.rs (`empty_prefix_is_an_error`, `not_a_repository`, `invalid_arguments`, `status_filters_and_pages`)
- A list cut at its limit must leave out two or more, never one: a list one longer than its limit is printed whole
  (`sync_utils::list_cut::kept`: `files` and `files_omitted` of `bd update` and `bd pick`, `paths` and
  `blocked_paths_omitted`, the builds, test names and messages of `bd robot --full` and their `omitted`, the dangling
  references the refusal of `bd safe-push` describes, the `dropped_commits` of a remote, the failed checks a robot or a
  pull request names), as the app cuts its lists and prompts (`cutList`).
  [@test] ../../crates/sync-utils/src/list_cut_test.rs (`a_list_one_longer_than_its_limit_is_kept_whole_and_a_longer_one_is_cut_at_the_limit`)
  [@test] ../../crates/bd-core/src/check_summary_test.rs (`a_dangling_reference_message_describes_two_whole_and_counts_from_three_on`)
  [@test] ../../crates/sync-core/src/detection_remote_status_test.rs (`test_dropped_commits_one_more_than_the_limit_are_listed_whole`)
  [@test] ../../crates/patronus/src/actions_test.rs (`the_failed_checks_of_a_robot_are_named_whole_when_only_one_is_past_the_limit`)
  [@test] ../../crates/remote-facts/src/pull_requests_test.rs (`the_failed_checks_of_a_pull_request_are_named_whole_when_only_one_is_past_the_limit`)
  [@test] ../../crates/update-core/src/update_project_test.rs (`a_conflict_one_file_wider_than_the_list_reports_every_file_with_its_commits`, `many_blocking_paths_are_counted`)
  [@test] ../../crates/patronus/src/actions_test.rs (`a_list_of_the_full_failure_one_longer_than_its_limit_is_given_whole`)
- Commit ids must be 8 characters, longer for the whole output when two of its ids share that prefix; `--full-ids`
  must print full ids.
  [@test] ../../crates/bd-remote/src/status_test.rs (`full_ids`)

### `bd status`

- It must print HEAD, the baseline, the prefix and its source (`prefix_source`), the branches newest first (name, ref, issue key, latest time, commit
  count, the tip of the synced virtual branch or `null`, commits newest first without the prefix) and the unassigned
  commits.
  [@test] ../../crates/bd-remote/src/status_test.rs (`status_shape`)
- Without a baseline (no upstream and no `origin/master` or `origin/main`, or no remote and no local `master` or
  `main`) it must be `no_baseline`, the message naming each branch it tried once.
  [@test] ../../crates/bd-remote/src/status_test.rs (`no_baseline_is_refused`)
- In a repository without commits (`git init`), every command that reads HEAD or the baseline (`status`, `commits`,
  `suggest`, `check`, `sync`, `fold`, `update` and the rewrites) must be `no_baseline` with "The repository has no
  commits yet: commit to the main branch, then sync", not a git error; telling it must cost no git process when HEAD
  resolves.
  [@test] ../../crates/bd-remote/src/status_test.rs (`no_commits_is_no_baseline`)
- `--branch` must list one branch, `--unassigned` only the unassigned commits; `--limit`/`--offset` must page their
  commits, and the branches otherwise.
  [@test] ../../crates/bd-remote/src/status_test.rs (`status_filters_and_pages`)
- It must not run integration detection, an update check or a network command of git.
- On Space, with the Safe Push CLI logged in, each branch a merged Safe Push landed must show `landed_by` until the
  update archives it, as the rows of the app say it ([Patronus](../branch/patronus.spec.md#merged)): the `branch` whose
  Safe Push it was (the branch itself, a branch built on it, or the root with `whole_stack`), the `robot`, the
  `commits` it landed and `after`, the commits added since, the only work left, decided by content
  (`robot_landings`: the commits the robot tested against the branch's); `--brief` the `branch`, `whole_stack` and the
  count of `after`. The push URL of `origin` (`remote get-url`) and the user's robots kept in the object cache while
  they hold, else one list and one poll ([Patronus](../branch/patronus.spec.md#load-on-patronus)); off Space, without
  a token or an answer of Patronus nothing is landed.
  [@test] ../../crates/bd-remote/src/landed_test.rs (`status_names_the_branches_a_merged_safe_push_landed_and_what_is_left_of_them`)
  [@test] ../../crates/bd-remote/src/robot_list_test.rs (`status_asks_patronus_once_while_the_kept_robots_hold`)
  [@test] ../../crates/bd-cli/src/process_budget_test.rs (`a_status`)
- On Space (the push URL of `origin`), each branch must show `landing_needs`: what its Safe Push needs of a commit
  outside what it lands, the refusals `bd safe-push` would give its start (`trial_check::landing_gate::landing_needs`,
  one trial check of every branch, less what merged Safe Pushes landed), so an agent sees them without `bd check`: per
  need the `code` (`missing_declaration`, `dangling_reference`), the `identifier`, `at` (`file:line` of the use), the
  commit it `needs` and its `needs_branch`, the `fix` (`bd restack <root> <branch>`, which the user approves first, or
  `move <commit> into <branch>`) and `landing_in`, the robot of the running Safe Push that lands `needs_branch`. Empty:
  nothing is needed. A branch built on one that needs a branch needs it too. Omitted off Space, with `--brief` or
  `--unassigned`, and when the check can't run: unknown, not empty.
  [@test] ../../crates/bd-remote/src/landed_test.rs (`status_names_what_the_safe_push_of_a_branch_needs_of_a_branch_outside_it`)
  [@test] ../../crates/trial-check/src/landing_gate_test.rs (`the_needs_of_a_branch_are_the_refusals_of_its_start`)
- Each branch, and in `stacks` each whole stack of a stack that forks, must show `next_step`: the step the app's row
  shows by the same rule (`domain_policy::next_step`, [Next Step](../branch/next-step.spec.md#the-table)) over what
  `bd status` read: `row` of the table (`0` waits for a fact, `1` has nothing to start, `1″` `pick_remote_commits`:
  someone else pushed to the remote branch, `bd pick --remote` takes it in, … `11` a Safe Push), `step`,
  `waits_for` (the fact without an answer, or whose read failed), `pull_request`, and why a Safe
  Push or a Dry Run can't start now (`safe_push_blocked`, `dry_run_blocked`, `null` when it can). `bd status` lists no
  pull request, asks no readiness and no problem of a failed robot: a step that needs them waits for them (row 0,
  `pull_requests`, `readiness`, `robot_problems`); `bd safe-push --check` reads them. Omitted off Space (the row has
  no slot), with `--brief` and `--unassigned`; no git process more.
  [@test] ../../crates/bd-remote/src/landed_test.rs (`status_gives_each_branch_and_whole_stack_the_next_step_of_the_apps_row`)
  [@test] ../../crates/bd-cli/src/process_budget_test.rs (`a_status`)
- Each branch must show its `origin` (`prefix`, `issue_group` or `issue`) and, for an issue group, its `issue_keys`;
  the output must list `idle_issue_groups`, the keys of the issue groups that group no commit on this HEAD, and
  `issue_group_problems`, the lines of the issue groups that are not used.
  [@test] ../../crates/bd-remote/src/status_test.rs (`issue_groups_in_status`)
- With stacked branches on, each branch must show its declared `parent`, `parent_state` (`active`, `integrated`,
  `unapplied` or `missing`), the `effective_base` or `blocked_by` a sync would use, `pr_base_changed` when a parent
  without commits is passed (the branch is built on the nearest active ancestor or the baseline, never blocked by it),
  and its stack `problems` with the commits that declare them, and the output must list the `forest` order, parents
  first; `bd commits` must group by the leaf as well. The `parent_source` of a parent is `prefix` or `store` (the stack
  parents), and the output lists `idle_stack_parents` and `stack_parent_problems` ([Stack Parents](../branch/stack-parents.spec.md)).
  [@test] ../../crates/bd-remote/src/status_test.rs (`stacks_in_status`, `stored_parents_in_status`)
- With stacked branches on, `stacks` must list the stack branch of each stack that forks (`root`, `branch`
  `{prefix}/stack/<root>`, `state`, `tip`, `member` and `reason` of a block, `conflict`, `error`, `commits`, `members`)
  and the ones kept for their remote branch, as the last sync left them, read without a sync and without a git
  process: `blocked` by a member's placement, else `built` when the object cache remembers the build of the current
  commits at the ref's tip, `conflict` when it remembers their build conflicting, else `stale`; no `remote`
  ([Whole Stack](../branch/whole-stack.spec.md)).
  [@test] ../../crates/bd-remote/src/status_test.rs (`the_stack_branch_of_a_forking_stack_in_sync_and_status`)
  [@test] ../../crates/bd-cli/src/process_budget_test.rs (`a_status_of_a_forking_stack`)
- Each branch with a `bd describe` text must show its title, read for all branches in one git process, the one that
  also gives the stack setting of the grouping.
  [@test] ../../crates/bd-rewrite/src/describe_test.rs (`sets_prints_and_clears_a_description`)
  [@test] ../../crates/bd-cli/src/process_budget_test.rs (`a_status`)
- `--stats` must add per branch the distinct files, the generated files, added and deleted lines, the top three
  directories, the authors, the author date range, the issue keys and a diffstat of the synced branch against the sync
  base (`null` if it was never synced), for all branches in at most three more git processes.
  [@test] ../../crates/bd-remote/src/status_test.rs (`stats_per_branch`)
  [@test] ../../crates/git-ops/src/commit_files_test.rs (`lists_the_files_of_each_tip_against_a_base`)
- `--brief` must print per branch only the name, the commit count, the issue key when it is not the name, and the
  newest subject, with stacked branches on also the parent, `blocked_by`, `pr_base_changed`, the problems, the
  forest and the `stacks`, then the unassigned commits; it must not go with `--branch` or `--stats`.
  [@test] ../../crates/bd-remote/src/status_test.rs (`brief_status`, `brief_status_keeps_the_stack_state`)

### `bd commits`

- It must print every commit ahead of the baseline, oldest first: id, subject, body, author, time, group (or `null`),
  issue key, and for a fixup its kind and the target autosquash resolves.
  [@test] ../../crates/bd-rewrite/src/commits_test.rs (`entries_groups_and_fixups`)
- `--files` must list each changed file with its status (`A`, `M`, `D`, `R` with the old path, `T`) and the counts of
  the changed lines of the diff views; a binary file must have `"binary": true` and no counts.
  [@test] ../../crates/bd-rewrite/src/commits_test.rs (`files_with_renames_binaries_and_deletions`)
  [@test] ../../crates/trial-check/src/diff_scan_test.rs (`parses_the_raw_part`, `read_commit_diffs_takes_the_blocks_of_the_views`)
- `--patch` must add per changed file the blocks of the diff views under `Default`, each with its id for
  `bd move-hunk --block` (the lines as in a git hunk header and a hash of them), its kind, a header with the label of
  its scope, and its lines between up to three lines of context that stop at the next block; `--branch` must keep the
  commits of one branch and the edges and overlaps that involve it.
  [@test] ../../crates/bd-rewrite/src/commits_test.rs (`dependency_edges_and_cluster_overlap`)
  [@test] ../../crates/file-diffs/src/patch_test.rs (`each_block_has_its_own_context`, `an_insertion_and_a_final_line_end`, `the_header_names_the_scope`, `the_ids_of_a_commit_apply`)
- `dependencies` must list `{from, to, files}` where `from` touches lines `to` added, adjacent lines included, plus the
  file-level rules for created, deleted, renamed and binary files; the lines are the blocks of the diff views, so the
  user's `diff.algorithm`, `diff.interHunkContext` or `diff.indentHeuristic` changes no edge.
  [@test] ../../crates/trial-check/src/dependencies_test.rs (`overlapping_and_separate_hunks`, `lines_move_with_earlier_insertions`, `several_hunks_in_one_change`, `adjacent_changes_depend`, `created_renamed_deleted_and_binary_files`)
  [@test] ../../crates/trial-check/src/diff_scan_test.rs (`the_diff_config_of_the_user_changes_neither_the_hunks_nor_the_edges`)
- Unless `branchdeck.symbols` is `off`, `dependencies` must also list the edges by symbols
  ([Symbol Dependencies](../commit/symbol-dependencies.spec.md)): `via` `symbols` for an edge by symbols only (its
  `files` the files of the uses) and `lines_and_symbols` for both, with `symbols` `{name, file}`; `via` must be absent
  for an edge by lines only. `units` must not join by an edge by symbols only.
  [@test] ../../crates/bd-rewrite/src/commits_test.rs (`an_edge_by_symbols_names_the_symbol_and_joins_no_unit`)
- `cluster_overlap` must list, for each pair of branches, the files both change.
  [@test] ../../crates/bd-rewrite/src/commits_test.rs (`dependency_edges_and_cluster_overlap`)
- `bd commits <id>…` must keep the commits with these ids (full, or abbreviated to at least 4 characters) and
  `--unassigned` the unassigned commits, with the edges that involve them; an unknown id must be
  `not_ahead_of_baseline`, an ambiguous one `invalid_arguments`, and the filters and `--branch` must exclude each other.
  [@test] ../../crates/bd-rewrite/src/commits_test.rs (`commit_ids_and_unassigned_filter_the_listing`, `an_ambiguous_id_is_invalid`)
- `--brief` must print per commit only the id, the subject (without a prefix that is exactly the group; a stack path
  or a sanitized prefix stays), the group, the issue key when it is not the group, the fixup kind and target, and the
  areas; the dependencies as `{from, to, via}`; and `units`, the connected components of two or more commits of the
  dependencies by lines and the fixups, over every commit, each oldest first.
  [@test] ../../crates/bd-rewrite/src/commits_test.rs (`brief_view_with_units_and_areas`, `brief_keeps_a_prefix_that_says_more_than_the_group`)
- An area must be the first `--depth` directories of a path (default 3; `.` at the top level); a commit must list at
  most three areas by added plus deleted lines, without those under 5 % of its lines except the first, and without
  the hub files of `HUB_FILES` (`crates/commit-clustering/src/generated.rs`). `--depth` must need `--brief`.
  [@test] ../../crates/commit-clustering/src/areas_test.rs (`an_area_keeps_at_most_depth_directories`, `hub_files_match_by_name`, `top_areas_by_lines_without_hub_files`)

### Generated files

- Lock files and the paths HEAD's attributes mark `linguist-generated` or `-diff` must not count in `dependencies`,
  `cluster_overlap`, the areas and the stats lines; `--files` must still list them with `"generated": true`, and
  `--include-generated` must count them.
  [@test] ../../crates/bd-rewrite/src/commits_test.rs (`generated_files_are_flagged_and_unlinked`)
  [@test] ../../crates/git-ops/src/attributes_test.rs (`reads_the_attributes_of_the_source_tree`)
  [@test] ../../crates/bd-remote/src/status_test.rs (`stats_per_branch`)

### Object cache

- The per-commit changes must be cached by commit id in `$GIT_COMMON_DIR/branch-deck/cache/`, and git must run once
  for the missing ids only. Cached and uncached outputs must be equal; `--no-cache` must neither read nor write it.
  [@test] ../../crates/bd-rewrite/src/commits_test.rs (`cached_output_equals_uncached`)
- Several processes must use the cache at once; a full cache must be cleared; the older versions of a store must be
  dropped, and only of that store; any cache failure must be a miss.
  [@test] ../../crates/object-cache/src/cache_test.rs (`round_trip`, `older_versions_of_a_store_are_dropped`, `a_bumped_store_keeps_the_other_stores`, `unusable_cache_is_a_miss`, `full_cache_is_cleared`, `parallel_threads`, `parallel_processes`)
- `bd sync` must walk no branch whose commits, remote branch and baseline did not move since a `bd` computed its remote
  status: the status of the same commits comes from the cache (`remote_status`, read only by the status version that
  wrote it), and the push times of all branches from one `reflog`.
  [@test] ../../crates/sync-core/src/process_budget_test.rs (`an_unchanged_sync_of_pushed_branches_in_a_new_process`)
  [@test] ../../crates/object-cache/src/cache_test.rs (`remote_statuses_are_read_by_their_version`)

### `bd group`, `rename`, `restack`, `unstack`

- `bd group` must read a plan `{branches: {path: [ids]}, unassign: [ids], issue_groups: {name: [keys]},
  ungroup_issues: [keys], unstack: [branches]}` from `--plan` (`-` for stdin), set the prefixes of all its commits in one
  message-only rewrite, and change the issue groups ([Issue Groups](../branch/issue-groups.spec.md)) and the stack
  parents ([Stack Parents](../branch/stack-parents.spec.md)) in the same ref transaction; ids may be abbreviated. A
  plan of issue groups and stack parents only rewrites no commit.
  [@test] ../../crates/bd-rewrite/src/rewrite_test.rs (`group_applies_a_plan`)
- `bd group` without `--plan` must be refused as `invalid_arguments` (exit 2), and stdin must not be read.
  [@test] ../../crates/bd-rewrite/src/rewrite_test.rs (`group_without_a_plan_is_refused`)
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
  [@test] ../../crates/bd-rewrite/src/rewrite_test.rs (`group_errors_and_exit_codes`)
- Fixups not in the plan must follow their targets. A rewrite whose fixup would fold into another target or none, or
  that recreates a target a fixup names by its id (`fixup! <hash>`), must be `invalid_plan`: fold it first.
  [@test] ../../crates/sync-core/src/regroup_test.rs (`fixups_follow_their_targets`)
  [@test] ../../crates/sync-core/src/reword_test.rs (`a_reword_that_strands_a_fixup_is_refused`)
- `descriptions` of the plan (`{branch: {title, body}}`, the leaf for a stacked branch) must be validated with it,
  before anything is written: a title of one line that is not empty, and a branch that has commits after the plan,
  else `invalid_plan`. bd must write the ones that differ after the ref transaction, under the same lock, as
  `bd describe` does, list the written ones in `described` and the texts they replaced in `replaced_descriptions`;
  a plan of descriptions only rewrites no commit. `bd check` must validate them as well.
  [@test] ../../crates/bd-rewrite/src/rewrite_test.rs (`group_writes_descriptions_and_syncs`, `descriptions_are_validated_with_the_plan`, `a_stacked_branch_is_described_by_its_leaf`)
- `--sync` must run the sync of `bd sync` after the writes and the release of the repository lock, so its merges never
  make the app or another `bd` wait, and add its output as `sync`; a conflict must exit with 1 and keep the undo token. A failed description write must skip the
  sync; it and a failed sync must keep the rewrite output, with the error as `after_error` and its exit code. `--sync` must not
  go with `--dry-run`.
  [@test] ../../crates/bd-rewrite/src/rewrite_test.rs (`group_writes_descriptions_and_syncs`, `a_sync_conflict_after_group_exits_with_1_and_keeps_the_undo_token`, `a_failed_description_write_is_the_error_of_the_output_and_keeps_the_undo_token`, `descriptions_are_validated_with_the_plan`, `the_sync_of_a_rewrite_runs_after_the_lock`)
  [@test] ../../crates/sync-core/src/regroup_test.rs (`a_held_lock_is_not_taken_again`)
- `--expect-head` must refuse with `head_moved` (exit 3, retryable) when HEAD is another commit, and the rewrite must
  move the branch only from the HEAD the plan was computed for.
  [@test] ../../crates/sync-core/src/regroup_test.rs (`expect_head_is_a_compare_and_swap`)
- With `--allow-append` (only with `--expect-head`) a HEAD with commits on top of the expected one must be accepted
  while the expected one is still ahead of the baseline, and the output must list the commits on top as `appended`
  with their new ids; any other HEAD stays `head_moved`. The appended commits are regrouped like the others: a rename,
  a restack or an unstack by name changes their prefix too.
  [@test] ../../crates/sync-core/src/regroup_test.rs (`allow_append_accepts_commits_on_top_and_refuses_a_rewritten_head`)
  [@test] ../../crates/bd-rewrite/src/rewrite_test.rs (`group_with_allow_append_lists_the_appended_commits`)
- Rewrites must hold `$GIT_COMMON_DIR/branch-deck.lock`, wait a bounded time for another holder, then fail with
  `locked` (exit 3). A rebase, merge, cherry-pick or revert in progress is `operation_in_progress`, also for
  `--dry-run`, and a detached HEAD `detached_head` (exit 4).
  [@test] ../../crates/sync-core/src/regroup_test.rs (`waits_for_the_repository_lock`, `refuses_while_a_rebase_is_in_progress`)
  [@test] ../../crates/bd-rewrite/src/rewrite_test.rs (`group_and_its_dry_run_are_refused_during_a_cherry_pick`)
- The output must list every commit's resulting group with its new id, the new HEAD and an undo token;
  `--dry-run` must write nothing and print the changed subjects (`changes`: the old and the new subject, `moved` with
  the branch before and after when it changes) and the stack forest; `--map` must add `{commit, new_id}` of every commit.
  [@test] ../../crates/bd-rewrite/src/rewrite_test.rs (`dry_run_prints_the_changes_and_the_forest`)
  [@test] ../../crates/sync-core/src/regroup_test.rs (`dry_run_writes_nothing`)
- `bd rename` must replace the branch's level in every prefix path, children included, rename the issue groups and
  the stack parents of that name without a rewrite, and name an issue branch by the issue groups; the new name must be
  a branch name without `/`.
  [@test] ../../crates/sync-core/src/regroup_test.rs (`renames_a_branch_with_its_children`, `renames_flat_and_issue_branches`)
- `bd restack` and `bd unstack` must set or clear the stack parent of the branch, an issue branch and an issue group
  included, rewrite only the commits whose prefix declares another parent, and need stacked branches.
  [@test] ../../crates/sync-core/src/regroup_test.rs (`restacks_and_unstacks_a_branch`, `restack_needs_stacked_branches`)
  [@test] ../../crates/bd-rewrite/src/rewrite_test.rs (`rename_restack_unstack_and_undo`)

### `bd reword`

- `bd reword <commit>` must replace the message of a commit ahead of the baseline with `--message`, `--message-file`
  (`-` for stdin) or `--subject` (the subject line, its `(prefix)` included; the body stays), exactly one of them, or
  the messages of several commits by `--plan` (`{rewrites: [{commit, message | subject}]}`, `-` for stdin), in one
  message-only rewrite: every tree, the index and the working tree stay, the older commits keep their ids, the later
  ones are recreated with their author and committer, and their mapping notes follow them. A message is cleaned up as
  `git commit` does; one equal to the old message after the cleanup rewrites nothing and prints no undo token.
  [@test] ../../crates/bd-rewrite/src/reword_test.rs (`reword_subject_changes_only_that_commit`, `reword_with_a_message_file_from_stdin_replaces_the_whole_message`, `a_message_equal_after_cleanup_rewrites_nothing`)
  [@test] ../../crates/sync-core/src/reword_test.rs (`rewords_messages_in_one_rewrite`, `a_subject_keeps_the_body_and_a_message_equal_after_cleanup_rewrites_nothing`)
- It must be a rewrite of `bd group`: under the repository lock (`locked`, exit 3), `--expect-head` (`head_moved`, exit
  3), `--allow-append`, `--sync`, `--map`, `operation_in_progress` (exit 4) also for `--dry-run`, the output of `bd group`
  with `operation: reword` and `changes` (old id, `new_id`, old and new subject, `body_changed`, `moved`) also without
  `--dry-run`, and an undo token `<previous head>..<new head>` that `bd undo` takes back, the old commits with their
  notes. The issue groups and the stack parents must stay.
  [@test] ../../crates/bd-rewrite/src/reword_test.rs (`reword_is_refused_on_another_head_or_while_the_lock_is_held`, `reword_and_its_dry_run_are_refused_during_a_cherry_pick`, `a_reword_plan_rewrites_two_commits_at_once_and_undo_restores_them`, `reword_dry_run_writes_no_ref`)
  [@test] ../../crates/sync-core/src/reword_test.rs (`a_reword_keeps_the_issue_groups_and_the_stack_parents`)
- A commit that is not ahead of the baseline (on the baseline, unknown) must be `not_ahead_of_baseline` (exit 2); a
  revision that is no commit id (`HEAD~1`) or a commit without exactly one text `invalid_arguments`; a plan entry
  without exactly one text, an empty plan, an empty subject, a subject of several lines and a commit named twice
  `invalid_plan`.
  [@test] ../../crates/bd-rewrite/src/reword_test.rs (`reword_of_a_commit_not_ahead_of_the_baseline_is_refused`, `a_reword_plan_rewrites_two_commits_at_once_and_undo_restores_them`)
  [@test] ../../crates/sync-core/src/reword_test.rs (`refuses_ids_and_texts_it_cannot_reword`)
- A new `(prefix)` must move the commit to that branch as `bd group` does, its fixups with it, with the validation of a
  plan: a stack problem it introduces is `invalid_plan`. A fixup that would fold into another target or none, a
  `fixup!`, `squash!` or `amend!` marker added or removed, a target a fixup names by its id, and a new text of a commit
  whose `amend!` waits must be `invalid_plan`; a plan that rewords the fixup with its target passes.
  [@test] ../../crates/bd-rewrite/src/reword_test.rs (`a_new_prefix_moves_the_commit_to_another_branch`, `a_reword_that_strands_a_fixup_is_refused`)
  [@test] ../../crates/sync-core/src/reword_test.rs (`a_new_prefix_moves_the_commit_with_its_fixups`, `a_reword_that_strands_a_fixup_is_refused`, `a_plan_that_rewords_a_fixup_with_its_target_passes`, `a_reword_that_breaks_a_stack_is_refused`)
- A reworded branch that was pushed must need a push: `bd safe-push --check` with `push_needed: true` and
  `remote.reworded`, `bd push --dry-run` a forced update.
  [@test] ../../crates/bd-remote/src/safe_push_test.rs (`a_branch_reworded_by_bd_reword_needs_a_push`)
- A session must be able to reword its own landed commit from its lease with `-C <shared checkout>`; the lease needs no
  move: a later pick lands only new commits and the return finds the old tip landed by content
  ([bd lease](lease.spec.md)).
  [@test] ../../crates/bd-lease/src/lease_test.rs (`a_session_rewords_its_landed_commit_from_its_lease`, `a_pick_without_follow_finds_the_reworded_commit_present_by_patch_id`)
- Its processes are pinned by subcommand: the snapshot of a regroup, one `fast-import`, one `update-ref`.
  [@test] ../../crates/bd-cli/src/process_budget_test.rs (`a_reword`)

### `bd undo`

- `bd undo <undo_token>` must move the branch back to the token's previous HEAD with compare-and-swap. A token of the
  issue groups (`groups:G1..G2`, or `H1..H2+groups:G1..G2` with a rewrite) must move them back in the same
  transaction, the stack parents likewise with `+stacks:S1..S2` (alone `stacks:S1..S2`). The parts come in this order,
  each at most once. Once the branch or a store changed again, the undo must be `undo_stale` (exit 4, not retryable):
  the token can never succeed again, as for the tokens of `bd update` and `bd pick`.
  [@test] ../../crates/sync-core/src/regroup_test.rs (`undo_restores_the_previous_head`)
  [@test] ../../crates/bd-rewrite/src/rewrite_test.rs (`rename_restack_unstack_and_undo`, `issue_groups_plan_and_undo`, `stack_parents_plan_and_undo`)
  [@test] ../../crates/sync-core/src/issue_groups_test.rs (`an_undo_token_is_read_strictly`)
  [@test] ../../crates/sync-core/src/issue_groups_test.rs (`a_mixed_plan_moves_head_and_the_store_in_one_transaction`)

### `bd update`

- `bd update`, `bd undo update:` and the resolve session of a conflicting update: [bd update](update.spec.md).

### `bd pick`

- `bd pick` (the commits of another worktree onto the checked out branch, `--follow`) and `bd undo pick:`:
  [bd pick](pick.spec.md).

### `bd unapply`, `bd reapply`

- `bd unapply` (a branch and its files out of the checked out branch), `bd reapply` (back on top) and `bd undo
  unapply:`, `bd undo reapply:`:
  [Unapply and Reapply](../branch/unapply.spec.md).

### `bd lease`

- `bd lease get`, `pick`, `return`, `status` and `adopt` (the pool of worktrees that agent sessions lease):
  [bd lease](lease.spec.md).

### `bd fold`

- `bd fold` must fold the `fixup!`, `squash!` and `amend!` commits into their targets as the app's Squash Fixups does
  ([Squash Fixups](../commit/fixups.spec.md)), under the repository lock, keeping HEAD's tree, the index and the working
  tree, and print per target the fixups folded, the orphans, and an undo token `<previous head>..<new head>` for
  `bd undo`. `--branch` must fold only the fixups whose target is in that branch, and a name that is no branch of the
  commits ahead of the baseline is `unknown_branch`; `--dry-run` must write nothing; nothing to fold is no rewrite and
  no token.
  [@test] ../../crates/bd-rewrite/src/fold_test.rs (`fold_keeps_the_tree_and_the_undo_restores_the_head`, `fold_of_one_branch_and_a_dry_run`)
- `--expect-head` must refuse with `head_moved` on another HEAD, and the fold must check that HEAD, the planned
  commits and the history it rewrites all end at the planned head; a held lock is `locked`.
  [@test] ../../crates/bd-rewrite/src/fold_test.rs (`fold_is_refused_on_another_head_or_while_the_lock_is_held`)
  [@test] ../../crates/history-rewrite/src/autosquash_test.rs (`refuses_when_the_branch_moved_since_the_plan`)
- A rebase, merge, cherry-pick or revert in progress in this worktree must be `operation_in_progress` (exit 4) for the
  fold and its dry run, moving nothing; one in another worktree refuses nothing, since git keeps that state per
  worktree.
  [@test] ../../crates/bd-rewrite/src/fold_test.rs (`fold_and_its_dry_run_are_refused_during_a_cherry_pick`, `a_cherry_pick_in_another_worktree_does_not_refuse_the_fold`)

### `bd move-hunk`

- `bd move-hunk <from> <to> <path>` must move the changes of the file `path` in commit `from` (all of them, or with
  `--block <id>`, repeatable, the blocks with the ids of `bd commits <from> --patch`) into commit `to`, older or
  newer, both ahead of the baseline on the checked out branch, in one rewrite under the repository lock. HEAD's tree
  must stay, and so the index and the working tree: into an older commit the change is added to the trees of
  `[to, from)` and `from` keeps its tree; into a newer one it is taken out of the trees of `[from, to)` and `to` keeps
  its tree. The commits above the newer one get new parents only.
  [@test] ../../crates/git-ops/src/move_hunk_test.rs (`moves_a_file_change_into_an_older_commit`, `moves_a_file_change_into_a_newer_commit`, `moves_one_block_of_three`, `moves_a_block_of_a_crlf_file_and_of_a_lone_cr_file`, `keeps_dirty_and_staged_changes_byte_for_byte`)
  [@test] ../../crates/bd-rewrite/src/move_hunk_test.rs (`the_fix_moves_into_the_commit_it_belongs_to_and_the_sync_rebuilds_the_branch`)
- A block must apply by its line ranges to the file as each commit between has it, through one
  `merge-tree --write-tree --stdin`: a commit that shifts the lines still takes it; one that changes the same lines, or
  lacks the file, is `hunk_conflict` (exit 4) with `commits` and `paths`, and nothing is written but git objects. Such
  a conflict is never resolved by words: the move must not change the user's commits in a way git does not.
  [@test] ../../crates/git-ops/src/move_hunk_test.rs (`a_commit_between_that_shifts_the_lines_still_applies`, `a_commit_between_that_changes_the_same_lines_conflicts`)
  [@test] ../../crates/git-ops/src/merge_tree_output.rs (`parses_the_merges_of_a_stdin_run`)
- `from`, `to` and the commits between whose diff of the file changed (`patch-id --stable`) must keep their author
  and get the current user as committer and no mapping note (the sync copies them again); every other commit must
  keep author and committer byte for byte, and its mapping note: with the new trees as its source for a commit
  between, as it was above. So the sync rebuilds the branches of `from` and `to` and keeps the others.
  [@test] ../../crates/git-ops/src/move_hunk_test.rs (`a_commit_between_that_shifts_the_lines_still_applies`, `moves_a_file_change_into_an_older_commit`)
- It must print the moved blocks (`moved.blocks`, as `bd commits --patch` prints them), `commits` (`{commit, new_id}`
  of every recreated commit), `changed` (the commits whose changes differ) and an undo token
  `<previous head>..<new head>` for `bd undo`; `--sync` must sync after the release of the lock (`sync`, or `after_error`
  with the token kept); `--dry-run` must print the blocks and check the rewrite, writing nothing but git objects.
  [@test] ../../crates/bd-rewrite/src/move_hunk_test.rs (`the_block_id_is_the_one_of_bd_commits_patch`)
  [@test] ../../crates/git-ops/src/move_hunk_test.rs (`a_dry_run_writes_no_ref_and_no_note`)
- It must refuse with `invalid_arguments` (exit 2) the same commit twice, a file the commit doesn't change, a file it
  renames, a mode change, a symlink or submodule, `--block` of a file without blocks, an id that does not parse,
  and a move that leaves a commit without changes ("squash it instead"); an id whose lines or hash match no block of
  the file now is `block_not_found` (exit 2) with `blocks`, the ids of now; an id that is no commit ahead of the
  baseline is `not_ahead_of_baseline`; `--expect-head` is `head_moved` on another HEAD, a held lock `locked`.
  [@test] ../../crates/git-ops/src/move_hunk_test.rs (`refuses_a_move_that_leaves_a_commit_empty_or_names_no_change`)
  [@test] ../../crates/bd-rewrite/src/move_hunk_test.rs (`the_block_id_is_the_one_of_bd_commits_patch`, `a_commit_between_on_the_same_lines_conflicts_and_another_head_or_the_lock_refuse`, `a_fix_that_is_the_whole_commit_is_refused_as_a_squash`)
- It must run a fixed number of git processes, whatever the number of commits between, and compute every one of them
  but the witness of HEAD and the move of the branch before the repository lock.
  [@test] ../../crates/bd-cli/src/process_budget_test.rs (`a_move_hunk`)
  [@test] ../../crates/bd-rewrite/src/move_hunk_test.rs (`a_move_hunk_locks_only_the_witness_and_the_write`)

### `bd check`

- `bd check` must apply a plan (`--plan`, `-` for stdin; the current grouping without one) in memory, validate it like a
  regroup (exit 2 when invalid, `descriptions` included), place the branches as a stacked sync does and build each
  one, parents first, writing git objects and the cache only: refs, notes, config and HEAD must not change.
  [@test] ../../crates/trial-check/src/check_test.rs (`a_commit_moved_away_from_its_dependency_conflicts`, `stacking_resolves_the_conflict`, `the_current_grouping_and_a_clean_plan_build`, `an_invalid_plan_is_refused`)
  [@test] ../../crates/bd-rewrite/src/check_test.rs (`the_current_grouping_and_invalid_plans`)
- Without `--plan`, `bd check` must check the current grouping and never read stdin, so the stdin of an agent, a CI
  or an IDE, an open pipe that sends nothing, does not hold it. `--plan -` must read the plan from stdin, with the
  result of `--plan <file>`.
  [@test] ../../crates/bd-rewrite/src/check_test.rs (`without_a_plan_an_open_stdin_is_not_read`, `a_plan_on_stdin_with_plan_dash_is_the_plan_of_a_file`)
- The branches must build at the same time as a sync builds them: a branch as soon as the one it is built on has its
  head, the others at once, with the outcomes of a build one by one (a child on its parent's head, blocked by a failed
  parent) and no more git processes.
  [@test] ../../crates/trial-check/src/check_test.rs (`independent_branches_build_at_the_same_time`, `a_branch_waits_for_its_parent_and_is_blocked_by_its_failure`)
  [@test] ../../crates/trial-check/src/process_budget_test.rs (`a_check`, `a_check_of_synced_branches`)
- Per branch it must print `ok`, a light conflict (commit, subject, files and the older commits outside the branch
  and its ancestors that change those files), `blocked_by`, `problems` or `error`, and the branch it would be built
  on; the exit code must be 1 unless every branch is `ok`.
  [@test] ../../crates/bd-rewrite/src/check_test.rs (`a_conflicting_plan_names_the_commit_and_what_it_needs`, `stacking_resolves_the_conflict`)
  [@test] ../../crates/trial-check/src/check_test.rs (`children_of_a_conflicting_branch_are_blocked`)
- With `--fix` it must add `fix`, the regroup after which the conflicting branches apply cleanly as the trial builds of
  [Conflict Fix](../branch/conflict-fix.spec.md) find it, each component's `plan` the whole plan for `bd group`;
  nothing is applied. `--max-trials N` (12) and `--time-budget SECONDS` (180, the whole command) bound the search,
  each only with `--fix`; a search they stop says `search: budget` with the best plan so far. Each trial is a line on
  stderr.
  [@test] ../../crates/bd-rewrite/src/check_test.rs (`fix_finds_the_regroup_after_which_the_plan_applies_and_writes_nothing`, `a_fix_out_of_time_says_budget_and_the_trial_limit_needs_fix`)
- With `branchdeck.symbols on`, `missing_declarations` must name each use, by a commit of a built branch, of a name
  that a commit outside the branch and the branches it is built on adds or gives the form the use needs (an edge by
  symbols), an override of it included, with the evidence. git merges such a branch and its build fails, so a branch
  that builds otherwise must be `missing_declaration` (exit 1); with `--allow-missing` it must keep that `result`, and
  only `ok` and the exit code must count it as building, as `bd safe-push --allow-missing` starts anyway.
  [@test] ../../crates/bd-rewrite/src/check_test.rs (`a_branch_that_uses_a_name_another_branch_adds_is_a_missing_declaration`, `an_override_of_another_branchs_overload_is_a_missing_declaration`, `allow_dangling_keeps_the_reference_and_exits_0`)
- A built branch that removes a declaration its virtual tree still uses where HEAD's tree doesn't must be
  `dangling_reference` (exit 1) with `{kind, identifier, file, line, removed_by, fixed_by, mutual, declared_in}`
  (`declared_in` the `{file, line}` of the declaration before `removed_by`), when a commit outside
  the branch and its ancestors changes the use (`fixed_by`); `mutual` when that change uses what the branch declares,
  with a `fix` only when a trial build of the plan with it proves it (the branch builds without a proven dangling
  reference, no branch that built breaks): the move of `fixed_by` into the branch when it changes only that file, else
  a `bd move-hunk` command, tried on commits written as objects only that no ref names, at most two per check.
  [@test] ../../crates/bd-rewrite/src/check_test.rs (`a_mutual_dangling_reference_is_fixed_by_the_move_hunk_it_names`, `a_move_hunk_fix_that_breaks_another_branch_is_not_offered`, `a_branch_that_removes_a_used_declaration_is_dangling_reference`)
  [@test] ../../crates/git-ops/src/move_hunk_test.rs (`a_trial_writes_the_commits_and_moves_no_ref`)
  With `--allow-dangling` the branch must keep its `result` `dangling_reference` and its references, and only `ok` and
  the exit code must count it as building, as `bd safe-push --allow-dangling` starts anyway; any other failure must
  still be exit 1.
  [@test] ../../crates/bd-rewrite/src/check_test.rs (`allow_dangling_keeps_the_reference_and_exits_0`)
  A use no commit changes, and one the baseline got after the base, must be a `possible_dangling_reference` warning that
  leaves the exit code alone. A declaration still in its file, a private one used elsewhere, a use HEAD has as often, a
  use the branch changes itself, a member moved with its class, a declaration moved within its package and a private one
  the branch declares elsewhere must not count, nor, by the syntax trees, a word of a comment or a string, a local of
  the same name, the name of an `override` member or a name a `use` or an import alias still binds, nor a Rust name after
  a single `.`, nor the uses the Rust crates rule out ([Symbol Dependencies](../commit/symbol-dependencies.spec.md)); `via` must say
  whether the trees confirmed the uses (`syntax_tree`) or the lines alone found them (`lines`). Only the paths where the
  trees differ may be read.
  [@test] ../../crates/trial-check/src/dangling_references_test.rs (`the_incident_a_moved_constant_whose_use_is_fixed_in_another_branch`, `a_use_changed_by_the_branch_itself_is_not_dangling`, `a_declaration_changed_in_place_is_not_dangling`, `a_reformatted_use_is_not_dangling`, `a_private_declaration_is_not_searched_outside_its_file`, `a_removed_top_level_function_is_searched_by_its_name`, `a_use_landed_on_the_baseline_is_a_possible_reference`, `a_member_moved_with_its_class_is_not_dangling`, `a_top_level_function_moved_within_its_package_is_not_dangling`, `a_private_declaration_moved_elsewhere_is_not_dangling`, `a_name_re_exported_by_a_use_is_not_removed`, `a_word_in_kdoc_is_no_use`, `an_override_line_is_no_use`, `a_local_val_shadows_a_private_member_of_another_class`)
  [@test] ../../crates/bd-rewrite/src/check_test.rs (`a_branch_that_removes_a_used_declaration_is_dangling_reference`, `a_possible_dangling_reference_is_a_warning_and_exit_0`)
- With `branchdeck.symbols on`, a dangling reference must also be found by the syntax summaries alone, with its `kind`
  (`removed_declaration` for the line search above): `removed_declaration` for an instance or static member of a JVM
  type or an enum entry that a commit of the branch removes with all its overloads, its key declared nowhere else in the
  tree, where a use resolves to it (a receiver of its type, `this`, an override, an inherited call, its static path);
  `changed_declaration` for a Kotlin or Java function, property or constant whose header, parameters (their types
  included) or reach a commit of the branch changes, a type whose constructors it changes, or a key of which it removes
  an overload while another stays, where the old form takes a use and no declaration of the key in the branch's tree
  does: proven only for a call the new parameters reject, a call only the removed overload took, an added `suspend`
  outside a coroutine and a changed extension receiver; a changed return type, nullability or type parameters and a
  suspend call in the lambda of an unknown function must be only a `possible_changed_declaration` warning with its
  `fixed_by`; `duplicate_declaration` for a JVM type or Kotlin top-level function a commit of the branch adds while the
  branch's tree, but not HEAD's, declares the key in another file of the same package and module. A change of the body
  alone, a use the new form takes (a parameter with a default, a call in a suspend function or a coroutine builder, a
  `var` for a `val`, an explicit type of a property, `operator`), an argument of no literal type, a call on a receiver of
  another type, a bare call of a name in another class, a use the branch's own commits wrote (by the lines of their
  hunks, not the text), a member that moves with its type, a change of a Rust or TypeScript signature, a declaration of
  another package and a single declaration in a file another branch deletes must not count. A possible one is a
  warning of the kind `possible_changed_declaration` or `possible_duplicate_declaration`.
  [@test] ../../crates/trial-check/src/declaration_checks_test.rs (`a_function_made_suspend_dangles_where_another_branch_removed_its_call`, `a_function_made_suspend_fits_a_coroutine`, `a_header_change_the_summary_cannot_judge_is_only_possible`, `a_changed_parameter_type_rejects_a_literal`, `a_removed_overload_dangles_where_the_one_left_rejects_the_call`, `a_changed_extension_receiver_breaks_its_call`, `a_changed_form_is_judged_in_the_jvm_languages_only`, `a_removed_member_is_kept_only_by_its_own_key`, `a_use_the_branch_writes_is_told_by_its_line_not_its_text`, `a_change_of_the_body_alone_changes_no_form`, `a_new_parameter_dangles_only_where_the_call_does_not_fit_it`, `a_call_on_a_receiver_of_another_type_is_no_use`, `a_use_the_branch_writes_fits_its_form`, `a_removed_instance_member_dangles_where_its_receiver_calls_it`, `a_type_declared_again_in_the_same_package_is_a_duplicate`, `a_type_of_the_same_name_in_another_package_is_no_duplicate`, `a_declaration_another_branch_deletes_is_no_duplicate`)
  [@test] ../../crates/bd-rewrite/src/check_test.rs (`a_branch_that_changes_a_used_declaration_is_dangling_reference`, `a_changed_return_type_is_a_possible_changed_declaration`)
  [@test] ../../crates/trial-check/src/process_budget_test.rs (`the_checks_of_a_session_on_sources`)
- A branch whose conflict or dangling reference needs a commit of a branch stacked on it, at any depth, must have
  `inverted_stack {branch, child, fixes}` and keep its `result`; the fixes (`move_to_child`, `swap`) must be plan
  parts for `bd group` that a trial build of the plan with them passes for both branches, and no `swap` for a mutual
  dangling reference; with no fix that builds, a `reason` instead.
  [@test] ../../crates/bd-rewrite/src/check_test.rs (`a_parent_that_needs_its_child_is_an_inverted_stack`, `a_mutual_dangling_reference_of_a_stack_is_an_inverted_stack_without_a_swap`, `an_inverted_stack_that_no_fix_builds_says_why`)
- `--build <command>` must run the command in the tree of each `ok` branch, extracted outside every working tree
  (`--build-paths`: only those paths, names, never patterns), and make a failure `build_failed` with its exit code and
  the end of its output; without it nothing runs. The command must reach the shell as written: `sh -c`, on Windows
  cmd with its quotes kept (a quoted path).
  [@test] ../../crates/bd-rewrite/src/check_test.rs (`check_build_runs_the_command_on_each_branch_tree`, `a_build_that_passed_on_a_tree_is_not_run_again`)
  [@test] ../../crates/git-ops/src/tree_archive_test.rs (`a_path_is_a_name`)
  [@test] ../../crates/bd-cli/src/agent_help_test.rs (`read_only_commands_write_no_ref_config_index_or_working_tree`)
- A build that passed must not run again on the same command, branch and extracted objects (the tree, or the object of
  each `--build-paths` path the tree has), also on a new commit of the same tree (`BD_COMMIT` is not in the key), and
  say `cached`; a failed build, another branch, a changed extracted object, `--rebuild` and `--no-cache` must run it.
  The key costs one `cat-file --batch-check` for all branches, and a cached build no `archive`.
  [@test] ../../crates/bd-rewrite/src/check_test.rs (`a_build_that_passed_on_a_tree_is_not_run_again`, `a_failed_build_runs_again`, `a_build_of_another_branch_with_the_same_tree_runs`, `rebuild_runs_the_command_on_a_cached_tree`)
  [@test] ../../crates/bd-cli/src/process_budget_test.rs (`a_check_with_a_build`, `a_warm_check_with_a_build`)
  [@test] ../../crates/object-cache/src/cache_test.rs (`passed_builds_are_read_by_their_version`)
  [@test] ../../crates/git-ops/src/cat_file_batch_test.rs (`the_ids_of_a_path_with_a_space_a_tree_a_gitlink_and_missing_names`)
- Merges must be memoized by their trees in the object cache, and a cached check must equal an uncached one.
  [@test] ../../crates/trial-check/src/check_test.rs (`the_cache_gives_the_same_report`)
  [@test] ../../crates/bd-rewrite/src/check_test.rs (`cached_check_equals_uncached`)
- A `CheckSession` must read the commits, the baseline and the branch tips once, and check any number of plans as
  `check_plan` does.
  [@test] ../../crates/trial-check/src/check_test.rs (`a_session_checks_many_plans_as_check_plan_does`)

### `bd suggest`

- `bd suggest` must print a plan for `bd group` with only the commits whose branch changes (a fixup that follows its
  target left out, a prefixed commit that goes to its own issue branch in `unassign`), the clusters with their name,
  commit count, `change` (`kept`, `renamed`, `merged`, `split`, `new`) against the groups now in `current_branches`, `why`, `weak`, issue keys
  and areas, with stacked branches the `parent` and `restacked`, the subjects the plan rewrites, and the trial sync of the plan. It must write git objects and the cache
  only, and the plan, applied, must suggest nothing more.
  [@test] ../../crates/bd-rewrite/src/suggest_test.rs (`suggests_a_checked_plan_that_is_stable_once_applied`, `the_plan_moves_only_what_changes_and_is_a_regroup_plan`)
- Each cluster must name its commits in three disjoint lists that agree with the plan: `kept`, the ones it leaves as
  they are; `moved`, the ones that change branch, each with `leaves`, the branch it leaves (`null` when unassigned),
  by its group now against the leaf of the cluster's path (a fixup that follows its target moves with it, the commits
  an issue group takes in or lets go move); and `new_path`, the ones whose branch stays but whose subject the plan
  gives another stacked path to the same leaf, so that `moved` and `new_path` together are what the plan changes of a
  commit (the top-level `rewritten` counts them).
  [@test] ../../crates/bd-rewrite/src/suggest_test.rs (`suggests_a_checked_plan_that_is_stable_once_applied`, `the_plan_moves_only_what_changes_and_is_a_regroup_plan`, `a_plan_never_moves_a_commit_with_an_explicit_prefix`)
- Without `--fresh`, a plan must never move a commit with an explicit `(prefix)`, `--rewrite-issues` included: it stays
  in `kept`. The one rewrite of such a subject is the stacked path it declares (`(gone/a)` to `a`): the commit stays
  in its branch and is in `new_path`.
  [@test] ../../crates/bd-rewrite/src/suggest_test.rs (`a_plan_never_moves_a_commit_with_an_explicit_prefix`)
- Hard units must join the dependencies, a fixup and its target, the commits of one prefix (unless `--fresh`) and the
  commits without a prefix of one issue key. Two prefixes must never join, and a dependency between them must be an
  `anchor_dependency` warning that names `git config branchdeck.stacks true`.
  [@test] ../../crates/commit-clustering/src/clustering_test.rs (`hard_units_join_dependencies_fixups_prefixes_and_keys`, `an_edge_between_two_anchors_is_a_warning_and_anchors_never_merge`)
  [@test] ../../crates/bd-rewrite/src/suggest_test.rs (`prefixes_that_depend_on_each_other_stay_apart_and_exit_with_1`)
- With stacked branches, a prefix must be an anchor by the leaf of its path, and a dependency must join no units: it
  must be an edge between two clusters, and only the dependencies of a cycle of units may join them. A cycle between
  two prefixes must stay an `anchor_dependency` warning. The commits without a prefix of an issue key that a prefix
  names must join that prefix, because they are one branch.
  [@test] ../../crates/commit-clustering/src/clustering_test.rs (`with_stacked_branches_a_dependency_stacks_the_branches_instead_of_joining_them`)
  [@test] ../../crates/bd-rewrite/src/suggest_test.rs (`a_follow_up_that_joins_an_issue_branch_stays_there_once_applied`)
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
  [@test] ../../crates/commit-clustering/src/clustering_test.rs (`with_stacked_branches_a_dependency_stacks_the_branches_instead_of_joining_them`, `a_merge_that_closes_a_cycle_waits_and_a_parent_without_commits_goes`, `stack_clusters_keeps_a_declared_parent_and_gives_way_in_a_cycle`, `stack_clusters_makes_every_needed_cluster_an_ancestor`, `stack_order_breaks_a_cycle_in_a_source_component`, `a_follow_up_of_two_stacked_branches_joins_the_upper_one_and_a_restack_names_its_reason`)
  [@test] ../../crates/bd-rewrite/src/suggest_test.rs (`with_stacked_branches_a_branch_stacks_on_the_branch_it_needs`, `a_stacked_plan_stacks_an_issue_group_and_rewrites_a_deeper_path`, `stacked_branches_and_an_empty_range`, `the_plan_moves_only_what_changes_and_is_a_regroup_plan`, `a_stored_parent_without_need_is_reported_and_kept`)
- With `branchdeck.symbols on`, an edge by symbols only must never join two units: with stacked branches it must be an
  edge of the stack outside the join of a cycle, without them it must link only a follow-up without a prefix and an
  issue key; a unit whose names three or more units use must order none of them; a branch the plan doesn't build on
  the branch whose name it uses must be a `symbol_dependency` warning.
  [@test] ../../crates/commit-clustering/src/clustering_test.rs (`an_edge_by_symbols_orders_and_never_joins`)
  [@test] ../../crates/bd-rewrite/src/suggest_test.rs (`with_the_symbols_on_a_use_of_another_branch_warns_or_stacks`)
- Soft merges must go by a deterministic integer score of issue key mentions, conventional scopes (`type(scope):` and
  `type scope:`), shared paths weighted by IDF, the disagreement of issue projects and scopes, and log adjacency: the
  best pair first, a tie to the oldest, under `--max-commits` (40) and down to the threshold 0.40. A key that more
  than max(10, units / 3) units name must link nothing, and one body of a branch of a prefix that names the key of a
  whole issue branch must not take the issue in without a second body or shared paths. With stacked branches, a dependency must link a cluster
  without a prefix and an issue key to the cluster it depends on, and no merge may make a cycle of the dependencies
  and the declared parents.
  [@test] ../../crates/commit-clustering/src/clustering_test.rs (`with_stacked_branches_a_dependency_stacks_the_branches_instead_of_joining_them`, `a_merge_that_closes_a_cycle_waits_and_a_parent_without_commits_goes`, `a_follow_up_of_two_stacked_branches_joins_the_upper_one_and_a_restack_names_its_reason`)
  [@test] ../../crates/commit-clustering/src/clustering_test.rs (`a_merge_that_only_adjacency_lifts_over_the_threshold_is_weak`, `the_same_scope_merges_commits_without_keys_and_another_scope_halves_paths`, `a_mention_links_keys_of_one_project_but_hardly_across_projects`, `a_branch_of_a_prefix_takes_in_a_whole_issue_only_by_more_than_one_mention`, `shared_paths_merge_but_a_feature_of_many_units_or_a_peripheral_file_weighs_little`, `the_cap_stops_soft_merges_and_a_tie_goes_to_the_oldest_pair`)
  [@test] ../../crates/sync-utils/src/issue_pattern.rs (`semantic_prefixes_with_and_without_parentheses`, `issue_keys_anywhere_in_a_text`)
- With `--fresh`, on the anonymized input of a large monorepo branch, the clusters must match the grouping a human
  made with a pairwise precision of at least 0.95 and a recall of at least 0.85, and keep the issue projects and the
  named pairs of groups apart.
  [@test] ../../crates/commit-clustering/src/clustering_test.rs (`golden_monorepo_branch`)
- A proven dangling reference must be repaired like a conflict, the removing commit needing `fixed_by` (with stacked
  branches it stacks on its branch); a mutual one must not be stacked but named in an `unresolved_conflict` warning
  that says to move the change into `removed_by`. A possible one is a `possible_dangling_reference` warning (or
  `possible_changed_declaration`, `possible_duplicate_declaration`).
  [@test] ../../crates/bd-rewrite/src/suggest_test.rs (`a_dangling_reference_stacks_the_branch_on_its_fix`, `a_mutual_dangling_reference_suggests_moving_the_change`, `a_changed_declaration_stacks_the_branch_on_its_fix`)
- A conflict of the check must join the hard unit of the conflicting commit to the units of the commits it needs by
  its lines that its branch and the branches it is built on lack, by the conflicting files only when the lines name
  none, also inside an issue group, and cluster again, at most 5 times, never joining two prefixes; the exit code must be 1 when a branch
  still conflicts. With stacked branches, a conflict on a commit of another cluster must first stack the two
  clusters, and join their units only when the same conflict comes again.
  [@test] ../../crates/commit-clustering/src/clustering_test.rs (`repair_joins_a_conflicting_commit_to_what_it_needs`, `with_stacked_branches_a_repair_stacks_first_and_joins_when_the_conflict_stays`)
  [@test] ../../crates/bd-rewrite/src/suggest_test.rs (`a_conflict_in_a_lock_file_joins_the_commits_and_fresh_ignores_prefixes`, `a_repair_joins_what_the_lines_of_the_conflict_need_not_every_commit_of_the_file`, `whole_issue_branches_become_an_issue_group`, `prefixes_that_depend_on_each_other_stay_apart_and_exit_with_1`)
- A name must be the cluster's prefix, the branch its commits come from when it holds the most of that branch, its
  single issue key, a scope of at least 60 % of its commits, `<area leaf>-<word>`, its most frequent key or
  `cluster-N`, in this order; a name another cluster, a group now or a virtual branch has, in any case, must get `-2`,
  `-3`, unless the cluster's commits come from it. `kept` must mean one whole branch under its own name.
  [@test] ../../crates/commit-clustering/src/clustering_test.rs (`names_in_order_of_preference_with_collisions`, `a_cluster_takes_the_name_of_the_branch_it_holds_most_of`, `changes_against_the_groups_now`, `subject_words_drop_stopwords_and_endings`)
  [@test] ../../crates/bd-rewrite/src/suggest_test.rs (`fresh_keeps_the_name_of_a_branch_that_stays_whole`, `stacked_branches_and_an_empty_range`)
- A cluster of two or more whole issue branches must go into `issue_groups` of the plan, marked `"via": "issue_group"`,
  so that the plan rewrites no commit; `--rewrite-issues` gives the commits a prefix instead. An issue group must be
  an anchor, as a prefix is.
  [@test] ../../crates/bd-rewrite/src/suggest_test.rs (`suggests_a_checked_plan_that_is_stable_once_applied`, `whole_issue_branches_become_an_issue_group`)
- `--dump-input` (hidden) must print the input of the clustering with every name hashed by a keyed hash (SipHash) with
  a random key that is not kept, and without subjects, bodies, ids and authors, so that the dump clusters as the
  repository does; only the module markers of file paths may become one marker.
  [@test] ../../crates/commit-clustering/src/clustering_test.rs (`anonymize_keeps_the_clustering_and_no_name`)
  [@test] ../../crates/bd-rewrite/src/suggest_test.rs (`the_dump_hides_every_name`)

### `bd sync`

- `bd sync` must run the app's sync (`sync_core::sync::sync_branches`) and, like it, without the repository lock: its refs move by
  compare-and-swap (below), so it never waits for a rewrite or Update Project, and a rewrite of the app or of `bd`
  never waits for it. It must print per branch its `state` (`created`, `updated`, `unchanged`, `error`, `conflict`, `blocked`),
  the commit count, the new tip and the remote state, plus the unassigned count. It must refuse, changing nothing,
  while a rebase, merge, cherry-pick or revert is in progress (`operation_in_progress`) or HEAD is detached
  (`detached_head`, exit 4 both): the sync would archive the branches the half-done history lacks.
  [@test] ../../crates/bd-cli/src/sync_test.rs (`syncs_and_reports_each_branch`, `syncs_while_another_process_holds_the_lock`, `a_rewrite_beside_a_running_sync_does_not_wait_for_it`, `waits_while_a_rebase_is_in_progress`)
- The message of `locked` must name the holder of the lock, its command line, pid and how long it holds it, from the
  record (`<lock>.holder`) a holder writes once it has the lock and deletes before it releases it; a record of a
  process that is gone (or none, of an older Branch Deck) names "Another Branch Deck operation".
  [@test] ../../crates/sync-core/src/repository_lock_test.rs (`a_lock_timeout_names_the_holder_and_its_release_deletes_the_record`, `the_record_of_a_gone_holder_names_nobody`)
- With stacked branches on, `stacks` must list the stack branch of each stack that forks, as the sync built it, with its
  `state` (`built`; `blocked` with the `member` and the `reason`; `conflict` with the commit, its subject and files;
  `stale` with the `error`), its `tip`, `commits`, `members` and the `remote` summary of its tip, then the ones kept
  for their remote branch (`stale`, no members); a tip in any state but `built` is not the stack's commits and must
  never be pushed ([Whole Stack](../branch/whole-stack.spec.md)).
  [@test] ../../crates/bd-remote/src/status_test.rs (`the_stack_branch_of_a_forking_stack_in_sync_and_status`)
  [@test] ../../crates/bd-cli/src/schema_test.rs (`outputs_match_schema`)
- A sync (the app's and `bd sync`, neither of which takes the repository lock) must move a virtual branch only from the tip
  it listed at its start, or create it only while no ref of its name exists: a branch another process moved to
  another tip meanwhile must keep that tip and be `error` ("Not updated: another process moved …; sync again"), while
  the other branches are written; one moved to the head the sync built (the copies are deterministic) is written.
  [@test] ../../crates/sync-core/src/branch_processor_test.rs (`a_branch_moved_since_the_listing_is_not_overwritten`, `a_new_branch_created_meanwhile_is_not_overwritten`)
- With `branchdeck.resolveByWords` on (the default), a cherry-pick that git stops at must be committed when the words
  resolve every conflicting file (IntelliJ's simple resolve, all or nothing per commit; only content conflicts of UTF-8
  text), with the author, the date and the message of the commit, and the branch built. `resolved` of the branch must
  list such commits with git's conflicts and the files; the mapping note keeps the counts, and `bd status` and
  `bd commits` must show them as `resolved_by_words` of the commit. `bd check` must build alike. With the setting off,
  or one file that does not resolve, the commit must stay a conflict of git; a copy that resolved must be copied again
  and conflict once the setting is off. The same commits must give the same copy in every repository.
  [@test] ../../crates/bd-cli/src/sync_test.rs (`a_conflict_that_its_words_resolve_builds_and_says_so`)
  [@test] ../../crates/trial-check/src/word_resolve_sync_test.rs (`a_conflict_that_its_words_resolve_is_copied_with_the_author_and_the_message_of_the_commit`, `the_next_sync_reuses_the_resolved_copy_and_says_so_from_its_note`, `the_setting_off_gives_the_conflict_of_git_and_on_again_the_resolved_copy`, `a_file_with_a_conflict_that_its_words_leave_keeps_the_commit_a_conflict`, `crlf_lines_keep_their_ends`, `a_stack_of_commits_builds_on_the_resolved_copy`, `the_same_commits_give_the_same_copy_in_another_repository`, `a_check_resolves_as_the_sync_does`)
  [@test] ../../crates/git-ops/src/word_resolve_test.rs (`a_modify_delete_beside_a_resolvable_file_keeps_the_commit_a_conflict`, `an_add_add_and_a_binary_file_stay_conflicts`, `a_merge_driver_keeps_the_conflict_of_git`, `text_that_is_not_utf8_stays_a_conflict`)
- The `conflict` of a branch in `conflict` must list in `missing_commits` the older commits that change a conflicting file
  and are neither in the branch nor in the branches it is built on, never the copies of its own or its ancestors'
  commits.
  [@test] ../../crates/trial-check/src/missing_commits_test.rs (`the_missing_commits_leave_out_the_branches_it_is_built_on_and_name_their_branch`)
- A virtual commit must be reused only while its mapping note fits the commit: the trees the note records as its
  source, and the message and the author of the copy. A note carried to a changed commit (`git notes copy`,
  `notes.rewriteRef`) must make the sync and the check copy the commit again. A note without a source must be checked
  against the tree of its copy, keep a copy that fits and get its source; `--no-cache` must check every note that way.
  [@test] ../../crates/bd-cli/src/sync_test.rs (`a_stale_note_rebuilds_the_branch`, `no_cache_verifies_each_note`)
  [@test] ../../crates/sync-core/src/branch_processor_test.rs (`a_note_copied_to_a_commit_with_another_tree_is_stale`, `an_amend_with_notes_rewrite_ref_rebuilds_the_copy`, `a_note_without_a_source_keeps_an_older_copy_and_gets_one`, `a_verifying_sync_checks_the_tree_of_each_copy`)
  [@test] ../../crates/trial-check/src/check_test.rs (`check_does_not_reuse_a_stale_copy`)
  [@test] ../../crates/git-ops/src/notes_test.rs (`the_lines_are_found_by_their_prefix`)
- The remote state must count the remote commits a push drops (`behind`, `dropped_commits`) apart from the older
  versions of commits it has ([Outgoing Commits](../repository/outgoing.spec.md#what-is-outgoing)): `amended` (of
  commits amended here), `restacked` (of commits of the branches it is built on) and `landed` (of commits the
  baseline has, `landed_commits` with the `baseline_commit` of each), and the commits it has in another version
  (`reworded`: another message or author), each left out at 0.
  [@test] ../../crates/bd-remote/src/push_test.rs (`amended_and_restacked_commits_push_without_force`, `a_push_replaces_the_remote_commits_the_baseline_landed_without_force`)
- A conflict must name the conflicting commit, its subject, the files and the commits the change likely needs, and the
  exit code must be 1, as for every branch the sync could not build (`error`, `blocked`). A sync of an unchanged conflicting branch must take that view from the object cache, shared
  with the app, and run neither the merge nor the analysis of the conflict again; it must report the view the analysis
  made, with the branches of the missing commits named as it groups them. A branch that conflicts after commits that
  copy cleanly must not copy those again either: the copies the sync wrote before the conflict are taken while the
  branch's commits before it are the same, also with a commit added after the conflict, and it says the same about
  every commit. `--no-cache` must analyse every conflict.
  [@test] ../../crates/sync-core/src/process_budget_test.rs (`an_unchanged_sync_of_a_conflicting_branch`, `an_unchanged_sync_of_a_conflict_after_a_clean_commit`)
  [@test] ../../crates/object-cache/src/cache_test.rs (`conflict_chains_are_read_by_their_version`)
  [@test] ../../crates/sync-core/src/sync_test.rs (`a_remembered_conflict_is_the_one_the_analysis_made`)
  [@test] ../../crates/bd-cli/src/sync_test.rs (`a_conflict_exits_with_1`, `a_branch_that_is_not_built_is_exit_1`)
- Per branch whose remote branch a push would change it must print the `bd push` to run, as bd names itself
  (`bd -C <repo>`): plain for unpushed, amended, reworded, restacked or landed commits; with
  `--force --expect-remote <remote.head>`
  when the push drops commits and no branch it is built on has commits for the remote; with `--force` alone when one
  has. Run as written, it must push.
  [@test] ../../crates/bd-remote/src/push_test.rs (`sync_names_the_push_of_each_branch`, `a_child_behind_its_remote_with_a_parent_to_push_forces_without_expect_remote`, `amended_and_restacked_commits_push_without_force`, `a_push_replaces_the_remote_commits_the_baseline_landed_without_force`)
  [@test] ../../crates/bd-cli/src/sync_test.rs (`syncs_and_reports_each_branch`)
- The integration results of archived branches and the update check of the checked out branch must be included
  compactly, with the landed commits (`update.landing`, `landed`).
  [@test] ../../crates/bd-cli/src/sync_test.rs (`reports_the_commits_the_baseline_already_has`)

### `bd push`

- `bd push <branch>…` must sync first under the repository lock, then push the branches in one atomic
  `git push --porcelain` through the remote command policy, parents first, with an explicit lease per ref; `--all`
  must need `--yes`. It is an adapter of the engine the app's push runs (`operations::push`): an intent `Push` per
  named branch, planned over the one-shot session of its sync (`push_planned`), the plans of all of them one push
  (`push_together`), run as the app's run (`land_execute::run_planned`). The lock covers only the sync, then again
  the witness of the plan (the tips it pushes, read before the lock and under it) and the freeze of its commits and
  leases: a branch another writer moved since the sync is `plan_changed` (exit 3), nothing pushed. The retargets, the
  cancel and the restart of `--restart-running` and the network push run after it, and the network push sends
  exactly the frozen commits (`<commit>:refs/heads/<name>`); the user's robots of the holds and the pull requests of
  the branches are asked after the sync, side by side: no other writer waits for GitHub, Patronus or the network.
  [@test] ../../crates/bd-remote/src/push_test.rs (`pushes_with_a_lease_and_prints_the_pull_request`, `all_needs_yes_and_unsynced_branches_are_refused`)
  [@test] ../../crates/bd-remote/src/push_test.rs (`the_network_push_holds_no_repository_lock`, `the_robot_list_of_a_push_is_read_outside_the_repository_lock`)
  [@test] ../../crates/sync-core/src/push_lease_test.rs (`parses_porcelain_lines`)
- It must not read again what its sync read: the stack, the tips, the remote tips, the remotes, the descriptions and
  the push size limit come from the sync of the same run; besides the push only the push URL of `origin`, once, the
  Space settings for a GitHub mirror, the size check (one `log`, one `cat-file`) and the witness of its plan (one
  `for-each-ref` before the lock and one under it; none for `--dry-run`, which moves nothing).
  [@test] ../../crates/bd-cli/src/process_budget_test.rs (`a_push`, `a_push_over_a_moved_remote`)
- A stacked branch must take the branches it is built on along, parents first, in the same atomic push, when they
  have commits for the remote, as the app's push button does ([Stacked Branches](../branch/stacked-branches.spec.md));
  one on the remote as it is stays out, and one taken along brings no children with `--with-children`. `--alone`
  (named branches only) and `--expect-remote` (the lease of one branch) push only the named branches: a stacked branch
  carries the commits of the branches it is built on, its pull request into the baseline includes them either way.
  [@test] ../../crates/bd-remote/src/push_test.rs (`parents_go_first_and_a_child_alone_carries_its_parents_commits`, `a_parent_goes_along_only_with_commits_for_the_remote_and_brings_no_children`)
- It must refuse (`push_refused`, exit 4, nothing pushed) a branch that didn't sync, and one behind its remote branch
  without `--force` (a parent taken along too, with `--alone` as the way out). Older versions of commits amended,
  restacked or landed are not behind: such a branch pushes without `--force`, and a parent with only them is taken
  along. A member of a stack that landed whole, with a new commit since, pushes without `--force`: its remote branch
  has its commits as they were before the landing, which the baseline has by identity; `replaced_landed` of the
  pushed branch (with `--dry-run` too) names them with their baseline commits. One remote commit the baseline lacks
  keeps the refusal, and `dropped_commits` names only such commits. Where the app's push leaves a branch its scope
  covers out to its row (a child the sync didn't build or a Safe Push holds, an ancestor or a child whose push drops
  remote commits), `bd push` refuses the push, nothing pushed, in the same words (`push_refusals`).
  [@test] ../../crates/bd-remote/src/push_test.rs (`all_needs_yes_and_unsynced_branches_are_refused`, `amended_and_restacked_commits_push_without_force`, `a_moved_remote_branch_is_fetched_and_reported_and_force_replaces_it`, `a_push_replaces_the_remote_commits_the_baseline_landed_without_force`, `a_remote_commit_the_baseline_lacks_still_refuses_the_push`)
- A named branch, or a child `--with-children` covers, that the remote has as it is (by content: `remote.up_to_date`,
  a branch only rebuilt on a newer baseline too) must be listed `up_to_date` with its lease and its pull request draft,
  and nothing of it pushed, as the app's push sends nothing of it; its pull request is not retargeted.
  [@test] ../../crates/bd-remote/src/push_test.rs (`pushes_with_a_lease_and_prints_the_pull_request`, `a_branch_only_rebuilt_on_a_newer_baseline_is_not_pushed_again`)
- `--with-children` must add the branches stacked on the named ones by declared parent, directly or further down,
  parents first, to the same atomic push, the branches the app's Push with Children covers (`domain_policy::stack::
  descendants`, the steps of `push::push_plan`; [Stacked Branches](../branch/stacked-branches.spec.md)); a blocked
  child must refuse the push as a named branch that didn't sync does, where the app's push leaves it out to its row. It must need named branches and must
  not go with `--expect-remote`.
  [@test] ../../crates/bd-remote/src/push_test.rs (`with_children_pushes_the_branches_stacked_on_the_named_ones`, `with_children_refuses_a_blocked_child`)
- `--with-children` of a named root of a stack that forks must also push its stack branch
  ([Whole Stack](../branch/whole-stack.spec.md)) in the same atomic push, after the branches, leased on the remote
  head its remote status compared with, as the app's Push with Children does: once it is on the remote or the pull
  request of the whole stack is open (the one GitHub search of the push, after the sync, its listing read by the
  retargets too), only when this sync built it (`built`) and the remote lacks it by content:
  `stacks` lists it as `branches` lists a branch, `name` the root, with the pull request of the whole stack. One on
  no remote and in no pull request stays local without a warning. Of the others, a stack branch in another state, or
  pushed to another remote than `origin`, which its remote status compares with, must never be pushed:
  `stack_warnings` names it, and so does one every branch of whose stack merged Safe Pushes landed whole
  ("test-user/stack/auth was not pushed: every branch of the whole auth stack landed in master with the Safe Pushes of
  login and session: nothing of it is left to push"). A child named with `--with-children` takes no stack branch.
  [@test] ../../crates/bd-remote/src/push_test.rs (`with_children_takes_the_stack_branch_along_once_it_is_on_the_remote`)
  [@test] ../../crates/bd-remote/src/safe_push_test.rs (`push_with_children_takes_the_stack_branch_along_while_its_pull_request_is_open`)
- The lease must be the remote head the sync of the same run compared with (`remote.head`), as the app leases on the
  remote head its row showed ([Outgoing Commits](../repository/outgoing.spec.md)): a remote commit the branch has by
  content must go through on the first try, `--force` must drop what the sync counted as `behind` and nothing pushed
  since, and either one's push must leave the other's next push valid. `--expect-remote <commit>` must lease one
  branch on that commit.
  [@test] ../../crates/bd-remote/src/push_test.rs (`a_plain_push_replaces_a_remote_commit_the_branch_has_by_content`, `force_drops_only_what_the_sync_saw_and_expect_remote_pins_the_lease`, `the_app_and_bd_share_the_lease`)
- With `--remote` other than `origin`, which the sync compares with, the refusal, the lease and the errors must use
  the remote status of the pushed branches against that remote, computed the same way; a `--remote` that is not a
  configured remote must be `no_remote`.
  [@test] ../../crates/bd-remote/src/push_test.rs (`another_remote_is_compared_with_as_origin_is`)
- A remote branch that is not where the lease expects it refuses the push, the other branches of the push included
  (it is atomic), and bd fetches the branches of the push. When each remote that moved has no commits its branch lacks
  and the branch has work for it (deleted, moved back to an older commit of the branch, rebuilt older versions:
  `domain_policy::push::over_moved_remote`, the rule of the app's push), bd must push again at once, once, leased on
  the fetched heads, and mark each such branch `pushed_over` (its `lease` the head it went over, `null` for a deleted
  one), with `--expect-remote` too: one rule with the app's Force Push, which leases on the head its row showed and
  loses nothing of a remote that moved only to commits the branch has. Otherwise, or when the remote moved again, it must be `lease_rejected`
  (exit 4, not retryable) with nothing pushed, listing the branches whose remote moved in `error.branches` with their
  remote status as fetched (`head`, `behind`, `dropped_commits`). A branch refused as behind is listed the same way,
  and `bd sync` prints the same remote status per branch. The app's stack push is the same push
  (`sync_core::push::PreparedPush::push_leased`). A push the remote refuses while every lease holds (a hook, a protected branch, a
  pack too large) must be `push_refused` with what git said of each ref and what the remote said why (its `remote:`
  lines, git's `error:` lines, not the progress), nothing pushed.
  [@test] ../../crates/bd-remote/src/push_test.rs (`a_moved_remote_branch_is_fetched_and_reported_and_force_replaces_it`, `force_drops_only_what_the_sync_saw_and_expect_remote_pins_the_lease`, `a_moved_lease_refuses_the_whole_stack_and_names_the_branch_that_moved`, `a_push_the_remote_refuses_with_its_leases_holding_is_push_refused`)
  [@test] ../../crates/bd-remote/src/push_test.rs (`pushes_again_over_a_moved_remote_that_lacks_nothing_of_the_branch`)
  [@test] ../../crates/bd-cli/src/process_budget_test.rs (`a_push_over_a_moved_remote`)
  [@test] ../../crates/domain-policy/src/push_test.rs (`pushes_again_over_a_moved_remote_only_when_nothing_of_it_is_lost`)
  [@test] ../../crates/bd-cli/src/sync_test.rs (`syncs_and_reports_each_branch`)
- A push whose commits add more files than the push size limit (`branchdeck.pushSizeLimit`, 50 MiB unless set, `0` or
  `off` for none) must be `push_too_large` (exit 4) before bd asks the remote, a `--dry-run` too, with nothing pushed:
  the message says how much it adds, the directory that holds most of it and the largest files, and `paths` lists the
  directory (`dir/`) and the files ([Outgoing](../repository/outgoing.spec.md#push)); `bd safe-push` of a whole stack
  and `bd push --unapplied` go through the same check. A limit that is neither a size nor off is
  `invalid_arguments`.
  [@test] ../../crates/bd-remote/src/push_test.rs (`a_push_over_the_size_limit_is_push_too_large_before_the_remote`)
- Per branch it must print the pull request base, the baseline without the remote (`master`) for every branch, stacked
  or not ([Stacked Branches](../branch/stacked-branches.spec.md#pull-requests)), the head, the title and the body as the
  app's dialog writes them ([Pull Requests](../branch/pull-requests.spec.md#creating-a-pull-request): the `bd describe`
  text, else the text of the commits, from the sync's report), and a ready `gh pr create --title … --body-file -`
  command, with `--repo` for a GitHub repository `origin` doesn't push to (a mirror). The summaries of the issues come
  from YouTrack as in the app, read with the token of `YOUTRACK_TOKEN` or of the app's sign-in and never renewed.
  [@test] ../../crates/bd-remote/src/push_test.rs (`pushes_with_a_lease_and_prints_the_pull_request`, `parents_go_first_and_a_child_alone_carries_its_parents_commits`, `with_children_pushes_the_branches_stacked_on_the_named_ones`, `the_pull_request_command_names_a_github_mirror`)
- Before the push, with the lock released, it must retarget the open pull request of each branch it pushes whose base
  is a virtual branch of the user (`{prefix}/virtual/…`) to the baseline, as the app's push does (one search of the
  pull requests, one `PATCH pulls/<n>` each; D3a: GitHub never compares the pushed commits with a parent's branch the
  pull request no longer goes into, and a push the remote then refuses leaves the retarget done): `retargeted` lists them (`branch`, `number`, `url`, `old_base`, `new_base`, and
  `approved_by`, whose approvals GitHub may dismiss). A pull request into another base (a release branch) stays. A
  retarget GitHub refuses, or a listing that fails for another reason than a missing token, is a `retarget_warnings`
  line, with the `gh pr edit` that retargets it; the push goes on. `--dry-run` lists the retargets a push would make
  and changes nothing on GitHub. Without a GitHub repository of the branches or a token, GitHub is not asked.
  [@test] ../../crates/bd-remote/src/safe_push_test.rs (`push_retargets_a_pull_request_into_a_virtual_branch_to_the_baseline`)
  [@test] ../../crates/bd-remote/src/safe_push_test.rs (`push_retargets_before_it_pushes_and_a_refused_push_keeps_the_retarget`)
- On Space, with the Safe Push CLI logged in, it must hold the branches the app holds
  ([Patronus](../branch/patronus.spec.md#pushing-into-a-branch-whose-safe-push-runs)): a named branch whose Safe Push
  runs on it, or on a branch built on it whose commit has commits of it, is `robot_running` (exit 4, nothing pushed),
  also with `--alone`, with the robot's page. The refusal names why: for its own robot the branch the push moves ("A
  Safe Push of auth is running on test-user/virtual/auth: a push moves the branch it took its commit from, so it won't
  land and fails the Safe Push at the end"), for another robot the commits of the branch it lands, counted, the first
  three by id, an amended one marked ("A Safe Push of login is running: it lands 2 commits of auth with it (1a2b3c4d,
  5e6f7a8b amended since), a push won't land"). What a robot lands is decided from its commit (`robot_spans`, by
  content after a restack or an amend below it, no process for a robot at the tip the sync left; the stack as it is
  now when the repository lacks its commit): a parent a restack placed below the robot's branch after its start is
  pushed. An ancestor taken along that a Safe Push lands stays as the robot took it, and its child is pushed alone,
  carrying its commits. A canceling robot and a dry run hold nothing. `--restart-running` cancels the Safe Push of
  a named branch after the lock, pushes the branch alone, the commits of the ancestors that robot lands with it, and
  restarts it on the pushed commit (`restarted`), as the app's restart that pushes first; its children still go with
  `--with-children`.
  One robots request after the sync; off Space, without a token or an answer of Patronus nothing is held. `bd
  safe-push` pushes without the check: it cancels the robot it replaces itself. A running Safe Push of the whole stack
  must hold every branch of the stack its commit has ("A Safe Push of the whole auth stack is running: it lands 1
  commit of login with it (1a2b3c4d)"), and `--with-children` of its root leaves its stack branch as it is, a
  `stack_warnings` line: a push into it fails the robot at the end.
  [@test] ../../crates/bd-remote/src/safe_push_test.rs (`push_refuses_a_branch_whose_safe_push_runs_and_restarts_it_when_told`, `the_whole_stack_lands_in_one_robot_from_its_stack_branch`)
  [@test] ../../crates/bd-remote/src/safe_push_test.rs (`a_push_of_a_parent_restacked_below_a_running_safe_push_goes_on`, `a_push_of_a_parent_whose_commits_a_running_safe_push_lands_is_refused`, `a_robot_of_a_temporary_ref_holds_only_what_its_commit_lands`, `push_with_children_leaves_the_stack_branch_of_a_running_whole_stack_safe_push`)
  [@test] ../../crates/bd-cli/src/process_budget_test.rs (`a_push_beside_a_robot_started_before_a_restack`)
- A named branch a merged Safe Push landed whole (`landed_by` of `bd status` without `after`), and with
  `--with-children` one built on a named one, must be left out of the push, as the app's Push with Children leaves it,
  and the others pushed: `skipped` names each with `code` `landed`, the `message` ("login merged into master with its
  Safe Push (<robot>): nothing of it is left to push", "auth landed in master with the Safe Push of login (<robot>):
  …") and `landed_by` as `bd status` has it. The push must be refused as `landed` (exit 4, nothing pushed, before any
  running robot) only when nothing else asked for is left, the stack branch of `--with-children` included; the error
  names the named branches. An ancestor taken along that landed stays as it is. A branch with commits added after the
  landed ones pushes them.
  [@test] ../../crates/bd-remote/src/landed_test.rs (`push_leaves_out_a_named_branch_a_merged_safe_push_landed_whole_and_pushes_the_others`, `push_and_safe_push_refuse_a_branch_a_merged_safe_push_landed_whole`, `push_with_children_leaves_out_what_merged_safe_pushes_landed_and_refuses_once_nothing_else_is_left`)

### `bd safe-push`

- `bd safe-push <branch>` must start a Patronus Safe Push of the virtual branch (with `--patronus-dry-run` a Dry Run: no
  `--dry-run`, which means "write nothing" for every other command) as the
  app's start does (the same `domain_policy::land::decide`, [Patronus](../branch/patronus.spec.md)): sync, then run the
  start through the engine of the app (`operations::land_execute::execute`, [Push and
  land](../../docs/design/git-operations.md#push-and-land)): the retargets, the cancel of the Safe Push it replaces, the
  push of the branches it pushes with their commits frozen under the repository lock and the network push after it,
  then the robot started on `refs/heads/<prefix>/virtual/<branch>`, never repeated. It must hold the repository lock
  for its sync, then again only for the witness of its refs and the freeze of its push: the reads of the remote, the
  trial, the check against the target, the retargets, the cancel, the network push and the start run without it, so
  they keep no other `bd` writer waiting. `--check` must print the same object and push or start nothing, with the
  `refusals` a start would fail with.
  [@test] ../../crates/bd-remote/src/safe_push_test.rs (`check_names_the_profiles_and_the_review_and_starts_nothing`, `missing_approvals_refuse_the_safe_push_unless_told_to_start_anyway`, `a_safe_push_frees_the_repository_lock_after_its_sync`)
  [@test] ../../crates/operations/src/land_execute_test.rs (`a_start_cancels_the_robot_it_replaces_pushes_and_starts_in_the_order_of_its_plan`, `the_first_effect_that_fails_stops_the_start_and_the_ones_before_it_stay_done`)
  [@test] ../../crates/bd-cli/src/process_budget_test.rs (`a_safe_push_start`)
- A rewrite of another session between the sync and the witness (a ref the plan moves or leases moved) must fail the
  start as `plan_changed` (exit 3, `retry: after_reread`), nothing canceled, pushed or started; one while the push sends,
  after the lock, must skip the start as `plan_changed`, naming what was pushed before it, with `bd safe-push <branch>`
  as the next step; the push sends the commits it froze.
  [@test] ../../crates/operations/src/land_execute_test.rs (`a_start_refused_before_its_first_effect_cancels_pushes_and_starts_nothing`, `a_rewrite_while_the_push_sends_skips_the_start_and_the_outcome_tells_what_was_done`)
- `--check` must print `next_step`, the step the app's row shows of the subject (`domain_policy::next_step`, as
  `bd status` prints it) over what the check read: the robots Patronus listed, the problems of the failed robot of the
  subject (whether it failed for the approvals, as the session reads them), the pull requests, the readiness when the
  start waits for it, the landing gate of the start; a step that needs a fact the check didn't read (the readiness of a
  Dry Run) waits for it.
  [@test] ../../crates/bd-remote/src/safe_push_test.rs (`check_names_the_profiles_and_the_review_and_starts_nothing`)
  [@test] ../../crates/bd-remote/src/scenario_test.rs (`the_next_step_row_cases_answer_alike_in_the_app_and_bd`)
- The start must be decided by the rule of the app, `domain_policy::land::decide`, over the facts `bd` read: what the
  table alone refuses (not synced, parked) fails the command before Patronus or GitHub is asked; the review and the
  rules of the target are read only when the decision waits for them; the `refusals` are its blockers in their order,
  then each confirmation a flag of `bd` accepts that the start lacks (the cancel of the Safe Push it replaces,
  `--cancel-running`; a review Patronus couldn't answer, `--allow-unknown-review`; rules GitHub didn't answer,
  `--allow-unknown-rules`), named with its flag; its effects are what the start does, in order (the retargets, the
  cancel, the push, the start from a pull request or through the API with the profile).
  [@test] ../../crates/domain-policy/src/land_test.rs (`a_start_with_nothing_in_its_way_pushes_what_it_lands_root_first_then_starts`, `every_fact_without_an_answer_waits_or_confirms_and_never_refuses`, `bd_accepts_by_an_allowance_only_the_cancel_an_unknown_review_and_unknown_rules`)
  [@test] ../../crates/domain-policy/src/land_test.rs (`the_table_refuses_first_then_the_target_then_a_push_that_drops_remote_commits_unless_allowed`, `the_landing_gate_refuses_unless_allowed_then_a_conflict_with_the_target`)
  [@test] ../../crates/bd-cli/src/process_budget_test.rs (`a_safe_push_check`)
  [@test] ../../crates/bd-remote/src/scenario_test.rs (`the_safe_push_mode_and_route_cases_answer_alike_in_the_app_and_bd`)
- The target branch, the stack and the tips must come from the sync of the same run, and the Space and the GitHub
  repository from one read of the push URL of `origin`, shared with the push.
  [@test] ../../crates/bd-cli/src/process_budget_test.rs (`a_safe_push_check`)
- The push must be decided by content, as the app's start decides it (the `Push` effect of `domain_policy::land::decide`): `push_needed` only when the sync's remote status of
  the branch or of a branch it is built on has work for the remote (`unpushed`, `behind`, `amended`, `reworded`,
  `restacked`, `landed`, or no remote branch). The start must push the branches of that effect and no other, each
  alone (`domain_policy::push::start_push`: the ancestors with work up to the nearest one a running Safe Push holds,
  past one that landed), and `remotes` name only theirs: the remote branch of a held ancestor stays where its robot
  took it. A branch whose remote branch has the same commits, only rebuilt since on
  a newer baseline, must not be pushed (no `pushed`); one reworded since must be, the robot would land the old
  messages: the robot starts from the remote branch, and `--create-pull-request` opens the pull
  request of the remote branch as it is. A robot running on that remote branch must refuse the start as one on the
  last commit does. A running Safe Push of an older commit must be replaced as the app's start replaces it once its
  popover said so (`replaces`, "Cancels the Safe Push of auth running on 1a2b3c4."): with nothing to push, a Safe Push start must refuse with `robot_running` ("the start would run beside it") unless
  `--cancel-running`, which cancels it before the start (`canceled`); a dry run and `--create-pull-request` leave it. A
  Dry Run replaces no Safe Push: its push into the branch of a running Safe Push is refused (`robot_running`),
  `--cancel-running` or not ([Patronus](../branch/patronus.spec.md#pushing-into-a-branch-whose-safe-push-runs)).
  [@test] ../../crates/bd-remote/src/safe_push_test.rs (`a_robot_on_the_last_commit_refuses_and_one_on_an_older_commit_is_canceled_when_told`)
  [@test] ../../crates/domain-policy/src/land_test.rs (`a_safe_push_replaces_its_own_older_robot_a_dry_run_is_refused_its_push_and_the_push_stops_at_a_held_ancestor`)
  [@test] ../../crates/bd-remote/src/safe_push_test.rs (`a_start_pushes_what_its_decision_pushes_and_leaves_an_ancestor_a_running_safe_push_holds`)
  [@test] ../../crates/bd-remote/src/safe_push_test.rs (`a_branch_only_rebuilt_on_a_newer_baseline_starts_from_its_remote_branch_without_a_push`, `the_pull_request_of_a_branch_only_rebuilt_on_a_newer_baseline_is_opened_without_a_push`, `a_branch_reworded_since_its_push_is_pushed_before_the_start`)
- `remotes` must name each existing remote branch the start pushes with its remote state, as `bd sync` has it, so
  `--check` says beforehand what the push replaces (`amended`, `reworded`, `restacked`, `landed`) and drops. A branch
  whose push would drop commits (`behind`) must refuse the check and the start with `push_refused` and the `bd push`
  that drops them after the user approves; the start's error carries the branch in `error.branches`, nothing pushed
  or started. `--force`, once the user approved, must start with the drop pushed away (D12): the push replaces the
  commits of someone else, then the robot starts.
  [@test] ../../crates/bd-remote/src/safe_push_test.rs (`check_counts_the_remote_commits_the_baseline_landed_and_refuses_a_drop`, `a_branch_reworded_since_its_push_is_pushed_before_the_start`)
- The profiles must be those of the target that allow the mode, never the emergency ones (`--profile emergency` is
  `invalid_arguments`: only the app's Safe Push without review offers one, in red); `--profile` takes an id or a name
  (any case) or `none`, another value is `invalid_arguments` that lists them. Without it the profile of the last
  robot of the branch is the default.
  [@test] ../../crates/bd-remote/src/safe_push_test.rs (`a_dry_run_takes_a_profile_by_name_and_checks_no_review`, `the_profile_of_the_last_robot_is_the_default_and_none_drops_it`)
  [@test] ../../crates/bd-remote/src/scenario_test.rs (`the_safe_push_profile_cases_answer_alike_in_the_app_and_bd`)
- Before a Safe Push (never a dry run) it must check the code-owner review as the app's start does, with the pull request
  the robot starts from, always asked again ([Patronus](../branch/patronus.spec.md#code-owner-review)), and a
  `missing` readiness must refuse it with `approvals_missing` (exit 4), nothing pushed or started, `error.review`
  naming the groups, their reviewers and files, as the app refuses the start (`domain_policy::land::decide`: "Patronus checks
  the approvals when the robot starts and fails without them") and its next step disables Safe Push. A start through the
  API (no open pull request to start from) whose merge profile is known to skip the review (the options of the last
  robot say `patronus.review.approval.check.skip`, or the list names it with the new API) must start without them, as
  the app's start does (`skipsApprovals`); `--without-approvals` must start anyway on either route, the counterpart of
  the app's Safe Push without review: with a `--profile` that skips the review (`--profile lsc`). A pre-check that
  failed is no answer, never `approved` and never a failure of the command (D14): it must refuse the start with
  `review_unknown` (exit 4, also in `refusals`), the `review` saying `unavailable`, unless `--allow-unknown-review`,
  which starts and leaves the approvals to the robot. The `review` says
  `readiness` (`not_required`, `approved`, `missing`, `unknown`, `unavailable`) with its `readiness_reason`, `skipped_by` (`profile` or
  `without_approvals`) when the approvals don't hold the start back, the code owners asked who haven't approved
  (`waiting_for`), the approvals Patronus doesn't count (`not_counted`), and per group `approved` and `needed`, per
  reviewer `approval_state`, when Patronus evaluated the pull request.
  [@test] ../../crates/bd-remote/src/safe_push_test.rs (`missing_approvals_refuse_the_safe_push_unless_told_to_start_anyway`, `a_dry_run_takes_a_profile_by_name_and_checks_no_review`)
  [@test] ../../crates/bd-remote/src/safe_push_test.rs (`a_profile_that_skips_the_review_starts_without_the_approvals_through_the_api`, `a_review_patronus_cannot_answer_refuses_the_safe_push_unless_allowed`)
  [@test] ../../crates/domain-policy/src/land_test.rs (`missing_approvals_refuse_unless_skipped_and_approvals_judged_at_the_start_confirm`, `the_default_profile_is_the_last_robots_while_offered_and_a_profile_is_confirmed`)
  [@test] ../../crates/bd-remote/src/scenario_test.rs (`the_safe_push_review_cases_answer_alike_in_the_app_and_bd`)
- A Safe Push that pushes new commits into the open pull request the robot starts from, which someone approved, must
  read the GitHub rules of the target first, as the app's start does ([Patronus](../branch/patronus.spec.md#starting-a-safe-push-or-a-dry-run)):
  one `GET rules/branches/<target>` (`read_branch_rules`), never for a dry run, `--create-pull-request`, a start
  without a push or a pull request nobody approved. Rules that dismiss stale reviews on push with the readiness
  `approved` must refuse it with `push_dismisses_approvals` (exit 4, nothing pushed or started), whose message says to
  push first (`bd push <branch>`), then to ask for the review again (`bd request-review <branch>`); with another
  readiness they must let it start, the dismissed approvals said in `rules.message`. Rules GitHub doesn't answer (no
  token, no access, an error) must never read as "keeps the approvals": `rules_unknown` (exit 4) refuses the start
  unless `--allow-unknown-rules`, which starts with `rules.message` saying what is not known. `rules` carries
  `dismiss_stale_reviews_on_push` (`true`, `false`, `null` when GitHub didn't answer) and `message`, absent when the
  rules were not read; `--check` lists the refusal in `refusals`.
  [@test] ../../crates/bd-remote/src/safe_push_test.rs (`a_push_that_dismisses_the_approvals_patronus_counts_refuses_the_safe_push`, `a_push_that_dismisses_approvals_patronus_does_not_count_starts_and_says_so`)
  [@test] ../../crates/bd-remote/src/safe_push_test.rs (`rules_github_does_not_answer_refuse_the_safe_push_unless_allowed`, `the_rules_are_read_only_for_a_push_into_an_approved_pull_request`)
  [@test] ../../crates/domain-policy/src/land_test.rs (`the_rules_of_a_push_into_an_approved_pull_request_refuse_confirm_or_wait`)
  [@test] ../../crates/bd-remote/src/scenario_test.rs (`the_discrepancy_cases_name_every_id_as_the_register_does`, `the_safe_push_review_cases_answer_alike_in_the_app_and_bd`)
- The approvals must be collected in the GitHub pull request of the branch, not in a Space merge request (deprecated):
  `--create-pull-request` must push the branch and open it through the GitHub API as the `gh pr create` that `bd push`
  prints would (with its `title` and `body`), assigned to the user (`--no-assign` leaves it unassigned) and with the
  `--reviewer <login>` logins asked (the author left out; a reviewer GitHub refuses is in `pull_request.warnings`), as
  the app's dialog creates it ([Pull Requests](../branch/pull-requests.spec.md#creating-a-pull-request)), without a
  start, and must be `invalid_arguments` without a GitHub repository of the branches; a pull request that already
  exists is found instead: one into the target is left as it is, one into a virtual branch is retargeted to the target
  (`retargeted`), one into another base refuses `pull_request_other_base` (its approvals would count for no Safe Push
  into the target). The
  `review` reviewers carry their `github_login` when Patronus names it, for `--reviewer`.
  [@test] ../../crates/bd-remote/src/safe_push_test.rs (`the_pull_request_it_opens_asks_the_suggested_code_owner_by_the_github_login_patronus_names`, `a_pull_request_that_already_exists_is_found_instead`)
- The token is the first of `GH_TOKEN` (`GITHUB_TOKEN`), `gh auth token`, the app's sign-in (its token file) and the git
  credentials of github.com ([Pull Requests](../branch/pull-requests.spec.md#the-token)); without one GitHub accepts,
  `not_logged_in`. Patronus judges the approvals of the pull request: an approval no eligible code owner gave (a bot)
  must not let the Safe Push start, `approvals_missing` naming who it waits for and the approval it doesn't count, and
  `review.approved_on_github` must name only the approvals that count. A Patronus that doesn't evaluate pull requests
  can't tell: then a pull request someone approved must let the Safe Push start (`readiness` `unknown`),
  `review.approved_on_github` naming every approver; without any approval, `approvals_missing` must name the pull
  request.
  [@test] ../../crates/bd-remote/src/safe_push_test.rs (`patronus_judges_the_approvals_of_the_pull_request`)
  [@test] ../../crates/bd-remote/src/safe_push_test.rs (`the_approvals_count_in_the_pull_request_it_opens_and_the_safe_push_starts_from_it`, `missing_approvals_refuse_the_safe_push_unless_told_to_start_anyway`)
  [@test] ../../crates/bd-remote/src/safe_push_test.rs (`a_pull_request_that_already_exists_is_found_instead`, `a_pull_request_without_a_github_token_is_not_logged_in`, `create_pull_request_retargets_the_pull_request_of_a_virtual_branch_and_refuses_one_into_another_base`)
- A branch a merged Safe Push landed whole must refuse the start with `landed` (exit 4, first of the `refusals` of
  `--check`): "Nothing was pushed or started: login merged into master with its Safe Push (<robot>): nothing of it is
  left to land"; so must `--whole-stack` once every branch of the stack landed ("every branch of the whole auth stack
  landed in master with the Safe Pushes of login and session"). `lands` must leave out the branches merged Safe
  Pushes landed whole, the branch itself always named: the robot lands the rest. A branch landed otherwise is refused
  as `landed` too, by the rule of the app (`domain_policy::landed::landed_state`): the fetched baseline has every commit
  of it ("auth is in origin/master: the update archives it"), or GitHub merged its pull request with the commits it has,
  the head of the merged pull request of the listing at its tip or its remote branch up to date ("pull request #7 of
  auth is merged", D13).
  [@test] ../../crates/bd-remote/src/landed_test.rs (`push_and_safe_push_refuse_a_branch_a_merged_safe_push_landed_whole`, `the_whole_stack_lands_what_is_left_of_it_and_refuses_once_every_branch_of_it_landed`)
  [@test] ../../crates/bd-remote/src/scenario_test.rs (`the_next_step_row_cases_answer_alike_in_the_app_and_bd`, `the_stack_and_whole_stack_cases_answer_alike_in_the_app_and_bd`)
- A robot on the last commit of the branch, or a Safe Push whose commit has commits of it, of a branch of its stack
  (decided as `bd push` decides it), must refuse the start with `robot_running`, in the words of the app, the commits
  named by id; a running Safe Push of an older commit, which the push fails, too, unless `--cancel-running` cancels it
  first. A parent a restack placed below a running Safe Push after its start starts.
  [@test] ../../crates/bd-remote/src/safe_push_test.rs (`a_robot_on_the_last_commit_refuses_and_one_on_an_older_commit_is_canceled_when_told`)
  [@test] ../../crates/sync-core/src/patronus_start_test.rs (`the_commits_a_robot_lands_are_named_by_their_number_and_ids`)
  [@test] ../../crates/domain-policy/src/land_test.rs (`a_safe_push_replaces_its_own_older_robot_a_dry_run_is_refused_its_push_and_the_push_stops_at_a_held_ancestor`, `the_whole_stack_says_its_build_first_and_its_own_robot_holds_or_is_replaced`)
  [@test] ../../crates/domain-policy/src/robot_match_test.rs (`the_robot_of_a_branch_is_the_running_one_then_the_newest_and_a_temporary_ref_by_its_revision`, `the_robots_match_their_branches_by_the_cases`)
  [@test] ../../crates/repository-model/src/patronus_view_test.rs (`a_running_robot_at_its_tip_lands_the_branches_it_is_built_on`, `the_first_robot_of_the_table_lands_a_branch_and_only_a_running_one_holds_it`)
  [@test] ../../crates/bd-remote/src/scenario_test.rs (`the_landing_cases_answer_alike_in_the_app_and_bd`, `the_stack_and_whole_stack_cases_answer_alike_in_the_app_and_bd`)
- A proven dangling reference (as `bd check` reports it) of the branch or a branch it is built on must refuse the start
  with `dangling_reference` (exit 4), nothing pushed; the trial build that finds it builds only what lands (every
  branch for a whole stack that forks), naming the use, `fixed_by` and the fix (of two references both,
  from three on of the first and "(2 more: bd check)"); `--check` must list it in `refusals`, `--allow-dangling` must
  start anyway, and the possible ones are `warnings`.
  [@test] ../../crates/bd-remote/src/safe_push_test.rs (`a_dangling_reference_refuses_the_safe_push_unless_allowed`)
  [@test] ../../crates/bd-core/src/check_summary_test.rs (`a_dangling_reference_message_names_the_kind`, `a_dangling_reference_message_describes_two_whole_and_counts_from_three_on`)
  [@test] ../../crates/bd-remote/src/scenario_test.rs (`the_landing_cases_answer_alike_in_the_app_and_bd`)
- With `branchdeck.symbols on`, a use by a commit of what lands of a name that a commit outside it adds (a missing
  declaration of `bd check`), which no merged Safe Push landed, must refuse the start with `missing_declaration`
  (exit 4), nothing pushed, in the same trial session (`trial_check::landing_gate`, the rule of the app's start too). A
  name a branch of the landing stack adds is none, a sibling's of a whole stack that forks too. The message must name
  the use, the commit that adds the name with its branch, and the fix: `bd restack <root> <branch>`, which lands that
  branch too, or, with stacked branches off, a commit unassigned or a restack that would close a cycle, the move of the
  commit; "(N more: bd check)" for the others. `--check` must list it in `refusals`, and `--allow-missing` must start
  anyway.
  [@test] ../../crates/bd-remote/src/safe_push_test.rs (`a_name_another_branch_adds_refuses_the_safe_push_unless_allowed`, `a_name_a_sibling_adds_refuses_the_branch_alone_but_not_the_whole_stack`)
  [@test] ../../crates/bd-cli/src/process_budget_test.rs (`a_safe_push_check`, `a_safe_push_check_of_the_whole_stack`)
  [@test] ../../crates/bd-remote/src/scenario_test.rs (`the_landing_cases_answer_alike_in_the_app_and_bd`)
- Patronus rebases what a robot lands onto the newest target before any check, and a conflict fails the robot at once.
  So before the lock, `bd safe-push` must fetch only the target's remote branch (as Update Project does). Then it must
  replay what the robot lands onto the tip that the sync read, as a plain `git rebase` replays it, with no resolution
  by words (`sync_core::target_conflict`, the check of the app's start too). A conflict must refuse the start with
  `target_conflict` (exit 4), nothing pushed or started. The error names the commit by the id of the checked out branch
  (`error.commits`), the files (`error.paths`), and the commits of the target after the base of the branch that change
  these files (hash, author, subject). The message tells the fix: `bd update` resolves it locally, then start again.
  `--check` must list it in `refusals`, and `target_check` must say `result` (`clean`, `conflict`, `not_run`) and the
  `target_tip`. A failed fetch must not stop the check: it replays onto the last fetched tip, `fetched` is false, and
  `message` says why. A check that can't run holds nothing back. `--create-pull-request` starts no robot and checks
  nothing.
  [@test] ../../crates/bd-remote/src/safe_push_test.rs (`a_target_that_moved_with_a_conflicting_change_after_the_last_sync_refuses_the_start`, `a_target_that_moved_without_a_conflict_starts`)
  [@test] ../../crates/sync-core/src/target_conflict_test.rs (`a_target_that_moved_with_a_conflicting_change_after_the_last_sync_conflicts`, `a_target_that_moved_without_a_conflict_is_clean`, `a_failed_fetch_checks_the_last_fetched_target_and_says_so`, `a_branch_that_does_not_resolve_is_not_checked`)
  [@test] ../../crates/bd-cli/src/process_budget_test.rs (`a_safe_push_check`)
  [@test] ../../crates/sync-core/src/process_budget_test.rs (`a_target_conflict_check`)
- A Safe Push of a stacked branch must land it with every branch it is built on, in one robot, as the app's start does
  ([Landing](../../docs/design/landing.md)): no refusal and no flag, the parents with commits for the remote pushed
  first, the robot started on the branch's own virtual branch, which carries their commits. `lands` must name what the
  robot takes, root first, ending with the branch (only the branch for a root), in the start, in `--check` and in a dry
  run, which checks the same commits. `--land-stack`, which an older bd needed, must be accepted, hidden, and change
  nothing.
  [@test] ../../crates/bd-remote/src/safe_push_test.rs (`a_stacked_branch_lands_its_parents_with_it`, `check_names_the_profiles_and_the_review_and_starts_nothing`)
- `--check` of a stacked branch must add `alone`, the trial build of the branch as a root on the base the sync built
  on: `builds` (with `uses`, the names of its stack it uses, and `symbols`, whether names were read) or `needs` (the
  conflicting commit and its files, the commits of its ancestors it needs with their branch, and `movable`), with a
  `message` naming `bd unstack <branch>` or the move; `unknown` when the trial fails. It must share the trial session
  of the dangling references, and a start must run no trial. It must never unstack or move anything itself.
  [@test] ../../crates/bd-remote/src/safe_push_test.rs (`a_stacked_branch_lands_its_parents_with_it`, `a_stacked_branch_that_needs_a_commit_of_its_parent_to_land_alone_names_it`)
  [@test] ../../crates/bd-cli/src/process_budget_test.rs (`a_safe_push_check_of_a_stacked_branch`)
- `--whole-stack` must land the whole stack of the branch (any branch of it) in one robot
  ([Whole Stack](../branch/whole-stack.spec.md#the-safe-push-of-the-whole-stack)): the robot starts from the stack
  branch of its top-level root, `refs/heads/<prefix>/stack/<root>`, the only branch pushed, leased on the remote head
  of its remote status in the sync's report, with nothing read again; `lands` must name every branch of the stack, and
  `whole_stack` the `root`, the `source` and the `state` of the stack branch, in the start, in `--check` and in a dry
  run. A straight chain, which has no stack branch, must start from its last branch that is not parked (`source` its
  virtual branch). The whole stack is the app's, by declared parent (D8): a blocked branch whose declared parent has no
  commits heads one of its own. A remote stack branch that moved since the sync without commits the stack branch lacks
  (an older build of the stack) must be pushed over again at once, once, leased on the fetched head, as `bd push`
  does (D4): `pushed[0].pushed_over`, and the robot starts; one with commits the stack lacks, or that moves again, is
  `lease_rejected`, nothing started.
  [@test] ../../crates/bd-remote/src/safe_push_test.rs (`the_whole_stack_lands_in_one_robot_from_its_stack_branch`, `the_whole_stack_of_a_straight_chain_lands_from_its_last_branch`)
  [@test] ../../crates/bd-remote/src/safe_push_test.rs (`the_whole_stack_pushes_again_over_a_moved_remote_that_lacks_nothing_of_its_stack_branch`)
  [@test] ../../crates/bd-remote/src/scenario_test.rs (`the_whole_stack_of_a_blocked_branch_with_an_inactive_declared_parent_is_d8`)
  [@test] ../../crates/bd-cli/src/process_budget_test.rs (`a_safe_push_check_of_the_whole_stack`, `a_safe_push_of_the_whole_stack_over_a_moved_remote`)
- A stack branch that is not `built` must refuse the start, nothing pushed or started: `stack_conflict` (exit 4) with the
  commit (`error.commits`) and the files (`error.paths`) when a commit of it conflicts with the commits of the stack
  before it, `stack_not_built` (exit 4) naming the branch of the stack that is blocked or failed, or why it is stale;
  both say to land it in pieces. `--check` lists the refusal and asks Patronus no review of it. A remote stack branch
  with commits the stack lacks must be `push_refused`, naming `bd push <root> --with-children --force`.
  [@test] ../../crates/bd-remote/src/safe_push_test.rs (`a_whole_stack_that_conflicts_is_refused_with_the_conflict`)
- The guards must go both ways: a running Safe Push of a branch of the stack lands a part of it and refuses the start
  of the whole stack ("A Safe Push of login is running: wait until it merges"); the running Safe Push of the whole
  stack lands every branch of it and refuses the start of a branch ("A Safe Push of the whole auth stack is running:
  it lands 1 commit of login with it (1a2b3c4d)") and of the whole stack on the same commits; a dry run lands nothing. A robot of the stack
  branch is never a branch's, not even by its revision.
  [@test] ../../crates/bd-remote/src/safe_push_test.rs (`the_whole_stack_lands_in_one_robot_from_its_stack_branch`)
  [@test] ../../crates/domain-policy/src/robot_match_test.rs (`the_robot_of_the_whole_stack_belongs_to_its_root_and_to_no_branch`)
  [@test] ../../crates/repository-model/src/patronus_view_test.rs (`the_robot_of_the_whole_stack_lands_every_branch_of_it`)
- The approvals of the whole stack must count in the pull request of its stack branch, the stack's own (never a
  branch's, never retargeted): `--create-pull-request --whole-stack` opens it into the target, titled by the root,
  with the stack branch's own description, else a section per branch (its description, else the subjects of its
  commits), cut to what GitHub takes; the review asks Patronus about the stack branch with that pull request, and a
  start comments on it.
  [@test] ../../crates/bd-remote/src/safe_push_test.rs (`the_pull_request_of_the_whole_stack_is_its_stack_branchs_and_the_start_comments_on_it`)
  [@test] ../../crates/remote-facts/src/stack_pull_request_test.rs (`the_pull_request_of_the_whole_stack_names_each_member_by_its_description_else_its_commits`, `a_text_too_long_for_github_is_cut_and_names_the_members_left_out`)
- Branches reviewed on GitHub with an open pull request into the target must start from a `/patronus` comment on it,
  reading the reply of the bot every 3 s for up to a minute (`operations::robot_start`), and must refuse `--profile`
  there; no Space merge request is looked up. The robot of the comment is the newest robot of that pull request in the
  mode of the start that had not finished before the comment (`comment_robot`), looked for once right after the bot
  accepted; a reply that never came is `reply: pending`, a rejection `patronus_refused` with the reason of the bot.
  [@test] ../../crates/bd-remote/src/safe_push_test.rs (`a_branch_with_an_open_pull_request_starts_from_a_comment`, `the_robot_of_a_comment_is_never_an_earlier_robot_of_the_pull_request`)
  [@test] ../../crates/operations/src/robot_start_test.rs (`a_comment_the_bot_accepts_starts_the_newest_robot_of_its_pull_request_nobody_knew`, `a_comment_the_bot_rejects_fails_with_its_reason_and_one_it_never_answers_is_pending_after_the_minute`)
- An open pull request of the branch, or of a branch its push takes along, whose base is a virtual branch of the user
  must be retargeted to the target before anything is pushed or canceled, and the robot then starts from the
  branch's: `retargeted` lists them as `bd push` does, `--check` the ones a start would retarget. A retarget of the
  branch's own pull request that GitHub refuses fails the start with nothing pushed or started; one of a branch taken
  along is a `retarget_warnings` line (where a pull request goes is the rule of the app's, `domain_policy::pull_request::
  base_of`). One into another base (a release branch) stays as it is, never retargeted silently: its approvals count
  for no Safe Push into the target, so a Safe Push that counts them, `--create-pull-request` too, must refuse
  `pull_request_other_base` (exit 4, also in `refusals`) with the `gh pr edit … --base <target>` retarget as its next
  step, as the app refuses by row 5′ ([Next Step](../branch/next-step.spec.md#the-table)); a Dry Run, `--without-approvals`
  and a profile that skips the review count none and start through the API.
  [@test] ../../crates/bd-remote/src/safe_push_test.rs (`the_pull_requests_of_the_branch_and_of_the_parents_it_pushes_go_into_the_target_before_the_start`)
  [@test] ../../crates/bd-remote/src/safe_push_test.rs (`a_pull_request_into_another_base_refuses_only_a_safe_push_whose_approvals_count`)
  [@test] ../../crates/domain-policy/src/land_test.rs (`the_pull_request_decides_the_route_a_draft_refuses_and_another_base_refuses_only_counted_approvals`)
- Without the Space token of the Safe Push CLI it must be `not_logged_in`, and outside Space `not_on_space`, before a
  request to Patronus.
  [@test] ../../crates/bd-remote/src/safe_push_test.rs (`without_a_token_or_space_nothing_runs`)

### `bd land`

- `bd land <branch>` must land the branch into master by the route of the repository (`branchdeck.landing`,
  `domain_policy::landing_route::route`, [Landing Routes](../branch/landing-routes.spec.md)): Patronus runs the Safe
  Push of `bd safe-push` (`route: patronus`, its output in `safe_push`); a push (`route: push`) pushes the copy the sync
  built (`commit`) into the target as a fast-forward; the merge of a pull request on GitHub is refused as not yet done
  (`landing_route_unsupported`), and Patronus named off Space as `not_on_space`. `--whole-stack` lands the whole stack,
  `--check` syncs and pushes nothing: `lands`, `confirmations`, `next_step` and the `refusals` a push would meet.
  [@test] ../../crates/bd-remote/src/land_test.rs (`a_repository_that_lands_through_its_pull_request_is_not_landed_by_bd_yet`)
- A push must run the plan of the app's start (`operations::land::prepare_one_shot` over the one-shot session of the
  command: the pull requests with the permission of the token, the rules of the target, the landing gate of the
  subject) through the engine (`operations::land_execute::execute`): no check against the newest target, the witness
  and the freeze under the lock, the push by the push URL after it. What CI did not check and the changes requested
  refuse it (`checks_not_passed`) unless `--without-checks`; what the gate of the route asks refuses it with the code of
  its reason (`baseline_requires_pull_request`, `baseline_refused`, `push_refused`) unless `--bypass-rules`; both only
  once the user agreed. A target that moved is `baseline_moved` (exit 3, master fetched, next step `bd update`), a
  refusal of the server `baseline_refused` in its words; below write access `no_write_access`.
  [@test] ../../crates/bd-remote/src/land_test.rs (`lands_the_copy_of_a_branch_into_master_once_the_user_accepted_what_ci_did_not_check`, `a_moved_target_is_baseline_moved_and_a_refusal_of_the_server_baseline_refused`)
  [@test] ../../crates/bd-remote/src/scenario_test.rs (`the_safe_push_mode_and_route_cases_answer_alike_in_the_app_and_bd`)
- `bd land` must never update: a push that landed prints `landed: true` and `next: "bd update"`, which brings the
  branch onto the target and drops the commits that landed. The agent skill says that the push route lands without a
  robot and without CI and needs the user's approval like any push.
  [@test] ../../crates/bd-remote/src/land_test.rs (`lands_the_copy_of_a_branch_into_master_once_the_user_accepted_what_ci_did_not_check`)
  [@test] ../../crates/bd-cli/src/agent_help_test.rs (`the_agent_help_is_a_short_core_with_every_guard`)

### `bd request-review`

- `bd request-review <branch>` must ask GitHub users to review the open pull request of the virtual branch, as the
  app's Request Review… does ([Pull Requests](../branch/pull-requests.spec.md#request-review)): without `--reviewer`,
  one suggested code owner with a GitHub login of each group whose approval is missing and nobody is asked for yet,
  each once, never the author, a member asked already or one whose approval counts (the ones the app's popover checks,
  by the one rule of both: `domain_policy::review::candidates`), from the review readiness
  `bd safe-push` checks (the pre-check with the pull request, asked again); `--reviewer <login>` (repeatable) asks
  those instead and doesn't ask Patronus. It is the intent of the app's request (`RequestReview`,
  `operations::pull_request`), planned over the one-shot session of the command (`operations::needs`) and run by the
  same engine. It must print `reviewers` (whom it asks), `requested`, `refused` with GitHub's reason, `waiting_for` (the
  reviewers the pull request asks after the request), the `readiness` before it, each person with the full name
  Patronus knows, and `pushed`, the branches it pushed first. `--check` must print the same object and push and ask
  nothing. It must hold the repository lock only for its sync and for the freeze of its push, and write nothing but
  the push of `--push` and the request on GitHub.
  [@test] ../../crates/bd-remote/src/request_review_test.rs (`asks_the_suggested_code_owner_of_the_missing_group_once`, `asks_the_logins_it_is_given_and_names_whom_github_refuses`)
  [@test] ../../crates/bd-cli/src/process_budget_test.rs (`a_request_review_check`)
- A branch with work its remote branch lacks must be `needs_push` (exit 4, D5) with the next step `bd request-review
  <branch> --push` (`needs_approval: true`): the reviewers would not see the commits. `--push` must push them first as
  the app's request does (the ancestors with work first, root first; an open pull request of what it pushes into a
  virtual branch retargeted into the target before the push), then ask; a push the user decides (one that drops remote
  commits) refuses it as `bd push` does.
  [@test] ../../crates/bd-remote/src/request_review_test.rs (`a_branch_its_remote_lacks_is_refused_without_push_and_pushed_first_with_it`)
- Without an open pull request, or with a draft, it must be `no_pull_request` (exit 4), naming `bd safe-push <branch>
  --create-pull-request`; a missing group nobody is asked for whose code owners have no GitHub login Patronus names,
  with nobody else to ask, `no_reviewer`; a request GitHub asked nobody in, `review_refused` naming each with the
  reason.
  [@test] ../../crates/bd-remote/src/request_review_test.rs (`a_branch_without_an_open_pull_request_or_a_code_owner_to_ask_is_refused`, `asks_the_logins_it_is_given_and_names_whom_github_refuses`)

### `bd robot`

- `bd robot <branch>` must print the robot of the virtual branch as `bd safe-push` finds it (a running one first, else
  the newest, by the branch or by the local or pushed tip of a temporary ref), or the one of `--robot`, with its
  status, mode, profile and whether it still runs; `robot: null` with exit 0 when the branch has none, and
  `unknown_branch` for a name that is neither a branch nor a branch with a robot. It must write no ref, take no lock
  and run no sync.
  [@test] ../../crates/bd-remote/src/robot_test.rs (`a_successful_robot_has_no_failure_and_a_branch_without_one_has_no_robot`, `a_failed_robot_says_why_and_which_local_commits_it_tested`)
- For a failed robot, and for a running one whose required check failed already, it must print the `failure` the
  app's Copy Prompt for Agent builds (`kind`, the failed checks with their build ids and counts, the error problems,
  the test and target revisions) and the commit map by the ids of the checked-out branch (`tested`, `rebased`,
  `changed`, `added`, `removed`, `landed`, `unknown`), `virtual_behind_head` for commits the last sync didn't build,
  and `head_branch`; no `failure` for any other robot.
  [@test] ../../crates/bd-remote/src/robot_test.rs (`a_failed_robot_says_why_and_which_local_commits_it_tested`, `a_running_robot_whose_required_check_failed_is_failing`, `a_successful_robot_has_no_failure_and_a_branch_without_one_has_no_robot`)
- `--full` must add the failed builds (no composite one) with their problems, failed tests and messages, and, through
  the `teamcity` CLI when it serves the build, the log tail of a build that failed not only on tests and whether a
  test failed before.
  [@test] ../../crates/bd-remote/src/robot_test.rs (`full_adds_the_failed_builds_their_tests_and_the_log_tails_of_the_teamcity_cli`)
- A newer robot of a branch stacked on it must win over the branch's own, named by `robot_branch`: it built this
  branch too. The commit map of a stacked branch's robot must have the commits of its ancestors too, each with its
  `branch`.
  [@test] ../../crates/bd-remote/src/robot_test.rs (`a_newer_robot_of_a_stacked_branch_tells_the_parent_with_the_commits_of_the_whole_stack`)
- `--robot` of a robot of another branch must be `invalid_arguments` naming that branch, unless that branch is stacked
  on it.
  [@test] ../../crates/bd-remote/src/robot_test.rs (`robot_takes_a_robot_by_id_of_the_branch_only`)
- A newer robot of the whole stack the branch is a branch of must win too, named by `stack_root`; `--whole-stack`
  must print the robot of the whole stack, as `bd safe-push --whole-stack` starts it (of a straight chain, the robot of
  its last branch), and refuse `--robot` of another one. A failure of the whole stack's robot must list the commits of
  every branch of the stack, each with its branch, matched by identity: the stack branch has no mapping notes.
  [@test] ../../crates/bd-remote/src/robot_test.rs (`the_robot_of_the_whole_stack_is_found_from_any_branch_of_it`)
- Without the Space token it must be `not_logged_in`, and outside Space `not_on_space`, before a request to Patronus.
  [@test] ../../crates/bd-remote/src/robot_test.rs (`robot_without_a_token_or_space_asks_nothing`)
- For a merged Safe Push it must list `closes`: the open pull requests of the branches it landed whole besides its own
  (a branch it is built on, every branch of its whole stack), each holding nothing the branch lacks, never the pull
  request it started from: the ones the app closes, by the same rule (`domain_policy::landed::pull_requests_to_close`,
  D7). `bd` closes none: it writes nothing to GitHub. Compared with the tips the refs hold (no sync); left out for a
  robot that didn't merge, off GitHub and while GitHub doesn't list the pull requests.
  [@test] ../../crates/bd-remote/src/robot_test.rs (`a_merged_robot_lists_the_pull_requests_its_landing_closes_and_closes_none`)

### `bd robot-stats`

- `bd robot-stats` must print the Safe Push statistics of the repository from the core of the app: the user's robots
  of the period (`--period 14d|30d|10w|6m`, 30 days by default) against everyone else's, the time lost, the failure
  causes and flaky checks of the last 14 days and the start hours
  ([Safe Push Statistics](../repository/safe-push-statistics.spec.md)); read only, Patronus listed week by week and
  kept in the object cache.
  [@test] ../../crates/bd-remote/src/robot_stats_test.rs (`the_period_is_the_days_that_count`)
  [@test] ../../crates/bd-remote/src/robot_stats_test.rs (`the_statistics_count_the_users_robots_against_everyone_elses_and_write_nothing`)
  [@test] ../../crates/bd-remote/src/robot_stats_test.rs (`a_failure_after_a_read_prints_the_kept_numbers_with_a_warning`, `the_statistics_say_why_there_are_none`)

### `bd describe`

- `bd describe <branch>` must print the branch's `branch.<name>.description` as `{title, body}`, or `null`. The
  `descriptions` of a `bd group` plan set several at once.
- `--title`, `--body` and `--body-file` must set it as `title`, a blank line and the body, keeping the part not
  given; `--clear` must remove it. A description needs a title, and the branch must be a current group.
  [@test] ../../crates/bd-rewrite/src/describe_test.rs (`sets_prints_and_clears_a_description`, `describe_errors`)
  [@test] ../../crates/git-ops/src/branch_description_test.rs (`writes_and_clears_a_description`)

### `bd version`

- `bd version` and `bd --version` must print `{"schema":2,"version","git"}` without a repository: the version of the
  app (the root `package.json`, which `tauri.conf.json` names), and the version of the git bd finds, `null` without one.
  [@test] ../../crates/bd-cli/src/version_test.rs (`version_and_dash_dash_version`)

### `bd install`

- `bd install` and Help → Install Agent CLI… must put a `bd` command where an agent finds it: on macOS and Linux a
  symlink in `~/.local/bin` to the app binary, replacing a symlink there and refusing anything else
  (`install_refused`); on Windows `bd.exe` next to the app.
- It must write the skill `branch-deck` into `~/.claude/skills/` and `~/.agents/skills/`: a description of when to use
  bd (first of all before the first edit or commit in a Git repository, for `bd workspace`
  ([bd workspace](workspace.spec.md)); then grouping into branches and pull requests, stacked when they build on each
  other, and updating onto the baseline with its conflicts, landing a worktree's commits, fixups, a change in the wrong
  commit, a review request, why a robot failed), its full path, `bd workspace` before the first edit with what each of
  its rules asks, `bd help agent` as the workflow of grouping, pushing and Safe Push, `bd help update` for an update,
  and `bd safe-push <branch> --check` before a Safe Push, which lands a stacked branch with its parents, and
  `--whole-stack`, which lands the whole stack, the landing of a worktree's commits onto a shared branch among its
  triggers, `bd help worktrees` when several sessions or worktrees work on one branch, `bd <command> --help` for the
  outputs and refusals of a command, and the rules an agent must not miss before it reads any help (approval before
  every push, never reorder, squash or split commits and never drop one yourself (bd drops a commit it proves landed,
  `--landed` one after the user's yes), `bd pick` instead of a cherry-pick, `error.code`, parked branches,
  unapply); the details of an output belong in the binary's help, not in the skill. `bd help skill` must print the
  same text. Its `allowed-tools` must be
  `Bash(<bd>:*)`, `<bd>` as the body names it, so that Claude Code runs every bd command without a permission prompt.
  [@test] ../../crates/bd-cli/src/agent_help_test.rs (`the_skill_allows_every_bd_command_as_named`)
  [@test] ../../crates/bd-cli/src/install_test.rs (`installs_the_command_and_the_skills_and_uninstalls_them`)
- It must say whether `bd` on `PATH` is the installed one, and print `agents` `{lease, subject_branch}`, the rules of
  `bd workspace` that the global git config gives every repository; `--uninstall` must remove the skills and only its
  own symlink. In the app the toast "Agent CLI installed" says where agents find bd, how to run it and how agents work
  from now on, paths under the home directory as `~`, with "Open Agent Settings" from the main window
  ([Onboarding](../repository/onboarding.spec.md#agents)); a failure is the error toast "Cannot install the Agent
  CLI". `agent_cli_status` must name the skill files that exist and the command they name, `null` without one, writing
  nothing (Settings → Agents).
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
  [@test] ../../src/hooks/useInstallAgentCli.test.ts (`names the skills, and bd as a command when it is on PATH`, `names the full path and the directory to add to PATH when bd is not on it`, `says how agents work by the rules of every repository`)
### `bd help agent`

- It must be a core of at most 10 KB, which every agent reads before grouping or pushing: the rules first, what needs
  the user's approval (every push, Safe Push and Patronus Dry Run, review request, `gh pr create`, park, unpark,
  unapply, reapply and change of another session's commit, a reword of its message too), parked branches and
  unapply, never reorder, squash or drop commits yourself (`bd fold` folds the fixups, `bd update` drops the landed
  ones), a change moved only with `bd move-hunk`, a
  message changed only with `bd reword`, only `bd update`, `bd pick`, `bd unapply`, `bd reapply` and their undo writing
  the working tree, what each exit code asks for (exit 1 never an error object, exit 4 without a rule: stop and show
  `message`), that `retryable` decides a retry whatever the exit code, the error object with `paths`, `branches`,
  `review`, `processes` and `next_steps`, which printed commands run at once (`next` of `bd workspace`, `bd update`
  and `bd pick`) and which wait for the user's approval (`push` of `bd sync`, `fix`, `next_steps` with
  `needs_approval`), `--expect-head`, `previous_head`, `new_head`, the `undo_token` and `after_error`, abbreviated ids
  compared by prefix. Then every refusal that waits for the user, with the flag that only the user's word allows:
  `push_too_large` (never raise the limit), `parked` (never push or unpark), `lease_rejected` (show
  `dropped_commits`), `robot_running`, `approvals_missing` (`--without-approvals` only when the user says so),
  `dangling_reference` and `missing_declaration` (only a false alarm), `rules_unknown`, `low_disk_space` (never
  `--min-free 0`). Then the workflow in steps (look with `bd suggest`; plan and check with the `result` of each
  branch; apply with `bd group --sync`; push; Safe Push with `--check` first, `lands` and `--whole-stack`; review;
  robot; statistics), each step sending the agent to the `--help` of its command, then the other tasks (`bd update`,
  `bd pick`, `bd fold`, `bd reword` with its `(prefix)` kept and never an empty `amend!` or plumbing, `bd move-hunk`,
  `bd install`). Together with `bd help update`, `bd help worktrees` and the `--help` of every command it must name
  every command and, per command, every long option except a listed few, and name no command, and no option for a
  command, that doesn't exist.
  [@test] ../../crates/bd-cli/src/agent_help_test.rs (`the_agent_help_is_a_short_core_with_every_guard`)
  [@test] ../../crates/bd-cli/src/agent_help_test.rs (`agent_help_names_every_command_and_option_that_exists`)
- Its first line must name the version.
  [@test] ../../crates/bd-cli/src/version_test.rs (`agent_help_names_the_version`)

### `bd <command> --help`

- The `--help` of `status`, `suggest`, `check`, `group`, `push`, `safe-push`, `request-review`, `robot`,
  `robot-stats`, `fold`, `reword` and `move-hunk` must print, after the options, the fields of the command's output
  and what each of its refusals asks; `-h` must end with the line that names `--help`, and `rename`, `restack` and
  `unstack` must name `bd group --help`. `bd help <command>` must print the same as `bd <command> --help`, its usage
  `bd <command>`. Every error code must be placed (an exhaustive match, so a new code can't be forgotten): a refusal an
  agent acts on must be named in the core, a topic or the `--help` of its command.
  [@test] ../../crates/bd-cli/src/agent_help_test.rs (`every_refusal_is_in_the_help_of_its_command`)
  [@test] ../../crates/bd-cli/src/agent_help_test.rs (`every_help_topic_prints_its_text`)

### `bd help worktrees`

- It must say how sessions that share a checkout work in worktrees of their own: the lease of a worktree with
  `bd lease get` (its `home` and `next`) and its return with `bd lease return` and their refusals, the `(branch)` prefix written
  when a commit is made (`(parent/leaf)` for a stacked branch, a readable name, an issue id never the name, no
  `fixup!` subject on the shared branch, `bd rename`, `bd suggest` only for commits without a prefix); one owner
  rewrites the shared branch (`bd reword` among its rewrites) and every other session only adds commits with
  `bd lease pick` from its lease (its refusals `home_branch_switched`, `no_home_branch`, `no_lease`, `unchecked_tree`
  and its `--unchecked`) or
  `bd pick --from <worktree> --follow`, never with `git cherry-pick` and never with `bd update` in a lease, its one rewrite a `bd reword` of its own landed
  commit with `-C <shared checkout>` and `--expect-head`, without `--sync`; after a rewrite a session's next write is
  `head_moved`, which `--allow-append` does not cure; the outcomes of a pick (`grouped_by: none`, `already_present`, `conflict.next`,
  `uncommitted_overlap`, `range_unknown`, `source_on_newer_baseline`, `followed.state`, the undo); `bd pick --dry-run`
  as the check after a rewrite, never a comparison by date; the protocol of the owner; and the `branch-deck:` reflog
  lines. `bd help worktrees` must print it, and an unknown topic must name it. Together with `bd help agent` and
  `bd help update` it must name only commands and options that exist.
  [@test] ../../crates/bd-cli/src/agent_help_test.rs (`every_help_topic_prints_its_text`, `the_worktrees_help_writes_the_prefix_first_and_checks_by_content`, `agent_help_names_every_command_and_option_that_exists`)

