---
name: Agent CLI
description: Requirements for bd, the command line a coding agent uses to read the branches and commits Branch Deck sees and to regroup the commits, as JSON with a stable contract.
style: plain-1
targets:
  - crates/bd-core/src/text_input.rs
  - crates/bd-remote/src/operation_command.rs
  - crates/operations/src/journal.rs
  - crates/bd-core/src/operation_record.rs
  - src-tauri/src/main.rs
  - crates/bd-cli/src/main.rs
  - scripts/build-bd.mjs
  - scripts/external-bin.mjs
  - src-tauri/tauri.windows-bd.conf.json
  - crates/bd-cli/src/lib.rs
  - crates/bd-cli/src/flag_names.rs
  - crates/bd-core/src/context.rs
  - crates/sync-core/src/branch_prefix.rs
  - crates/app-settings/src/resolve.rs
  - crates/app-settings/src/file.rs
  - crates/bd-core/src/error.rs
  - crates/domain-types/src/error.rs
  - crates/bd-core/src/output.rs
  - crates/bd-remote/src/status.rs
  - crates/bd-core/src/branch_view.rs
  - crates/bd-remote/src/branch_stats.rs
  - crates/bd-rewrite/src/commits.rs
  - crates/commit-clustering/src/areas.rs
  - crates/trial-check/src/diff_scan.rs
  - crates/bd-remote/src/numstat.rs
  - crates/trial-check/src/dependencies.rs
  - crates/commit-clustering/src/hub_files.rs
  - crates/generated-files/src/attributes.rs
  - crates/generated-files/src/names.rs
  - crates/bd-cli/src/schema.rs
  - crates/bd-core/src/version.rs
  - crates/bd-core/build.rs
  - crates/bd-rewrite/src/rewrite.rs
  - crates/bd-update/src/undo_command.rs
  - crates/bd-rewrite/src/group_plan.rs
  - crates/bd-rewrite/src/describe.rs
  - crates/git-ops/src/branch_description.rs
  - crates/bd-core/src/sync_command.rs
  - crates/bd-core/src/stack_summary.rs
  - crates/bd-core/src/next_step_summary.rs
  - crates/sync-core/src/branch_processor.rs
  - crates/bd-rewrite/src/check_command.rs
  - crates/bd-core/src/check_summary.rs
  - crates/git-ops/src/tree_archive.rs
  - crates/bd-remote/src/robot_command.rs
  - crates/bd-remote/src/request_review_command.rs
  - crates/bd-rewrite/src/fold_command.rs
  - crates/bd-rewrite/src/squash_command.rs
  - crates/history-rewrite/src/squash.rs
  - crates/bd-rewrite/src/move_hunk_command.rs
  - crates/trial-check/src/move_hunk.rs
  - crates/git-ops/src/move_hunk.rs
  - crates/git-ops/src/merge_tree_output.rs
  - crates/history-rewrite/src/autosquash.rs
  - crates/bd-rewrite/src/suggest.rs
  - crates/bd-core/src/suggest_plan.rs
  - crates/commit-clustering/src/clustering.rs
  - crates/sync-utils/src/issue_pattern.rs
  - crates/sync-utils/src/list_cut.rs
  - crates/bd-remote/src/push_command.rs
  - crates/sync-core/src/remote_command.rs
  - crates/sync-core/src/push_lease.rs
  - crates/sync-core/src/push.rs
  - crates/operations/src/push.rs
  - crates/operations/src/pull_request.rs
  - crates/bd-remote/src/safe_push_command.rs
  - crates/bd-remote/src/safe_push_run.rs
  - crates/bd-remote/src/land_command.rs
  - crates/operations/src/land_execute.rs
  - crates/operations/src/robot_start.rs
  - crates/operations/src/registry_file.rs
  - crates/bd-core/src/review_summary.rs
  - crates/bd-remote/src/push_refusals.rs
  - crates/remote-facts/src/stack_pull_request.rs
  - crates/remote-facts/src/patronus_review.rs
  - crates/remote-facts/src/review_readiness.rs
  - crates/sync-core/src/patronus_start.rs
  - crates/domain-policy/src/robot_match.rs
  - crates/trial-check/src/check.rs
  - crates/git-ops/src/merge_memo.rs
  - crates/git-ops/src/cherry_pick.rs
  - crates/sync-core/src/cache_memo.rs
  - crates/object-cache/src/conflict_info.rs
  - crates/object-cache/src/remote_status.rs
  - crates/object-cache/src/conflict_chain.rs
  - crates/sync-core/src/conflict_chain.rs
  - crates/object-cache/src/build_result.rs
  - crates/history-rewrite/src/regroup.rs
  - crates/history-rewrite/src/reword.rs
  - crates/history-rewrite/src/fixup_targets.rs
  - crates/bd-rewrite/src/reword_command.rs
  - crates/history-rewrite/src/issue_groups.rs
  - crates/history-rewrite/src/commit_ids.rs
  - crates/history-rewrite/src/repository_lock.rs
  - crates/bd-cli/src/agent_help.rs
  - crates/bd-cli/src/install.rs
  - crates/bd-core/src/bd_command.rs
  - crates/app-api/src/app/agent_cli.rs
  - src-tauri/src/agent_cli.rs
  - src/hooks/useInstallAgentCli.ts
  - crates/sync-core/src/grouping.rs
  - crates/object-cache/src/cache.rs
  - crates/object-cache/src/commit_diff.rs
  - crates/git-ops/src/notes.rs
  - crates/git-ops/src/copy_commit.rs
  - crates/trial-check/src/dangling_references.rs
  - crates/trial-check/src/declaration_checks.rs
  - crates/trial-check/src/dangling_fix.rs
  - crates/trial-check/src/inverted_stack.rs
  - crates/trial-check/src/missing_declarations.rs
  - crates/trial-check/src/landing_gate.rs
  - crates/domain-types/src/landing.rs
  - crates/git-ops/src/object_texts.rs
  - crates/object-cache/src/commit_lines.rs
  - crates/git-executor/src/git_info.rs
  - crates/bd-remote/src/landed.rs
  - crates/bd-remote/src/robot_list.rs
  - crates/sync-core/src/robot_landing.rs
  - crates/domain-policy/src/admission.rs
  - crates/domain-policy/src/land.rs
  - crates/bd-rewrite/src/block_patch.rs
  - crates/bd-remote/src/session_facts.rs
  - crates/bd-core/src/services.rs
  - crates/operations/src/land.rs
  - crates/operations/src/witness.rs
  - crates/operations/src/needs.rs
  - crates/operations/src/facts.rs
  - crates/repository-model/src/branch_table.rs
  - crates/repository-model/src/one_shot.rs
---

# Agent CLI

Status: Implemented
Date: 2026-10-16

## Purpose

`brd` lets a coding agent read what Branch Deck sees: the branches the commits ahead of the baseline group into, the
unassigned commits, and every commit with its files and what ties it to the others. The agent clusters the commits
into branches from that, applies the grouping with one validated rewrite of the commit messages, syncs the virtual
branches and writes their pull request text. It is the `cli` subcommand of the app binary
(`branch-deck cli status`), or the binary symlinked as `brd`; on Windows a console `brd.exe` next to the app. Every output is one JSON object with a stable schema.
How it works and the workflow: [Agent CLI guide](../../../../docs/contributing/agent-cli.md).

## Goals

- Give an agent everything it needs to cluster commits, in a few compact calls.
- Keep the output contract stable: versioned JSON, fixed error codes and exit codes.
- Stay fast on hundreds of commits: one git process per phase, and a cache for the per-commit data.
- Reading changes nothing; a regroup changes commit messages only. A fold, a squash and `brd move-hunk` change the trees of
  commits ahead of the baseline, never HEAD's tree, the index or the working tree. Only `brd update`, `brd pick`, `brd unapply`, `brd reapply` and their undo write the working tree, through the guarded update. `brd lease` writes only the slots of its pool.

## Non-goals

- Reordering, dropping or splitting commits (`brd move-hunk` moves a file's change between two commits; it splits no
  commit into new ones), and a squash the user did not ask for (`brd squash` squashes the commits the user named).
- Moving a change across a rename, a mode change, a symlink or a submodule, or a part of a binary file.

## Concepts

### Flag names

An agent carries a flag from one command to another: it ran `brd pick` with the `--unchecked` of `brd lease pick`, and
clap answered `brd land --unchecked` with "a similar argument exists: '--check'", a flag of the opposite sense. So one
sense has one word in every command (`crates/bd-cli/src/flag_names.rs`).

## Requirements

### Requirement: Invocation: The app binary and `brd` run the CLI without Tauri

`branch-deck cli <command>` and a binary named `brd` MUST run the CLI without starting Tauri.

#### Scenario: The CLI starts without the shell of the app
- **WHEN** an agent runs `branch-deck cli <command>` or `brd`
- **THEN** the CLI runs and Tauri does not start

### Requirement: Invocation: Windows gets a console `brd.exe` next to the app

On Windows, where the app binary has no console, the installer MUST put a console `brd.exe` next to the app
(`crates/bd-cli/src/main.rs`, bundled as an `externalBin` by `src-tauri/tauri.windows-bd.conf.json`).

#### Scenario: The Windows installer
- **WHEN** the app is installed on Windows
- **THEN** a console `brd.exe` lies next to the app binary, which has no console

### Requirement: Invocation: `-C` selects the repository

`-C <repo>` MUST select the repository, default the current directory; a subdirectory MUST resolve to the top level.

#### Scenario: A subdirectory of the repository
- **WHEN** `-C <repo>` names a subdirectory of a repository, or `-C` is absent and the current directory is one
- **THEN** the repository is its top level
  [@test] crates/bd-remote/src/status_test.rs (`subdirectory_resolves_to_top_level`)

### Requirement: Invocation: The branch prefix comes from the specific source first

The branch prefix MUST come from `--prefix`, else the specific one first: `branchdeck.branchPrefix` of the
repository's local or worktree config or `git -c`, else the app setting `globalUserBranchPrefix`, else the global or
system `branchdeck.branchPrefix` (`app_settings::resolve`), each named as the branches are (`jane/` is `jane`).

#### Scenario: The prefix of the command line
- **WHEN** `--prefix` is given
- **THEN** it is the prefix, taken as the branches name it (`jane/` is `jane`)
  [@test] crates/app-settings/src/resolve_test.rs (`the_prefix_of_the_command_line_is_taken_as_it_is_named`)

#### Scenario: The config of the repository wins over the app setting
- **GIVEN** `branchdeck.branchPrefix` in the repository's local or worktree config, or in `git -c`
- **WHEN** `brd` resolves the prefix without `--prefix`
- **THEN** that prefix wins over the app setting and the global config, and stays scoped to that repository
  [@test] crates/bd-remote/src/status_test.rs (`prefix_from_git_config`)
  [@test] crates/app-settings/src/resolve_test.rs (`the_specific_prefix_wins`)
  [@test] crates/sync-core/src/branch_prefix_test.rs (`a_local_prefix_is_repository_scoped`)

#### Scenario: The app setting before the global config
- **GIVEN** no prefix in the repository's config
- **WHEN** `brd` resolves the prefix
- **THEN** the app setting `globalUserBranchPrefix` is taken, else the global or system config
- **AND** the global config of the process that runs a test never reaches the `brd` of that test
  [@test] crates/bd-remote/src/status_test.rs (`prefix_from_app_settings`, `the_global_config_of_the_process_never_reaches_the_bd_of_a_test`)

### Requirement: Invocation: Every output echoes the prefix and status names its source

Every output MUST echo the prefix, and `brd status` MUST name its source (`prefix_source`: `cli`,
`repository_config`, `app_setting`, `global_config`), so that a fallback to the global config is explicit.

#### Scenario: The source of the prefix in the status
- **WHEN** `brd status` runs
- **THEN** its output names the prefix and `prefix_source`, one of `cli`, `repository_config`, `app_setting` and
  `global_config`
  [@test] crates/bd-remote/src/status_test.rs (`prefix_source_names_where_the_prefix_came_from`)

### Requirement: Invocation: The settings of the app are read once through its loader

The settings of the app (its prefix, `patronusNewApi`) MUST be read once as `brd` starts, through the loader of the
app (`app_settings::file::SettingsFile::load`), with the same defaults: a missing file, one that does not parse and a
field of the wrong type are the defaults.

#### Scenario: The settings file of the app
- **WHEN** `brd` starts
- **THEN** it reads the prefix and `patronusNewApi` of the settings file of the app
  [@test] crates/bd-core/src/context_test.rs (`the_new_patronus_api_setting_of_the_app_is_read_from_its_settings_file`, `the_prefix_of_the_app_is_read_from_its_settings_file`)

#### Scenario: A settings file that does not parse
- **WHEN** the file is missing, does not parse (it was cut) or has a field of the wrong type
- **THEN** `brd` takes the defaults, as the app does
  [@test] crates/bd-core/src/context_test.rs (`a_settings_file_that_is_cut_gives_the_defaults`)

### Requirement: Invocation: A read of the settings beside the app's write gets whole settings

The app MUST write the file only by a rename, so a read beside that write gets the old or the new settings, never a
cut file, and never fails the write (on Windows the rename replaces a file that `brd` has open,
`sync_utils::atomic_file`); a sync of the app MUST write only `repository-state.json` beside it, so `brd` beside a sync
reads the same prefix.

#### Scenario: `brd` reads while the app writes
- **WHEN** `brd` reads the settings while the app writes them
- **THEN** it reads the old or the new settings, never a missing prefix, and the write of the app does not fail
  [@test] crates/app-settings/src/service_test.rs (`a_reader_of_the_file_never_sees_a_missing_prefix_while_the_service_writes_it`)

#### Scenario: `brd` beside a sync of the app
- **WHEN** the app syncs while `brd` starts
- **THEN** the sync writes only `repository-state.json` beside the settings, and `brd` reads the same prefix

### Requirement: Invocation: Git comes from `BD_GIT`, `PATH` or Homebrew and is 2.54 or newer

Git MUST come from `BD_GIT`, else `git` on `PATH`, else Homebrew's git, never from a login shell, and be 2.54 or
newer.

#### Scenario: An old git
- **WHEN** the git found is older than 2.54
- **THEN** it is below the minimum version of `brd`
  [@test] crates/git-executor/src/git_info.rs (`minimum_version`)

#### Scenario: A missing explicit git
- **WHEN** `BD_GIT` names a git that does not exist
- **THEN** git is not found, with the hint
  [@test] crates/git-executor/src/git_info.rs (`a_missing_explicit_git_is_not_found_with_the_hint`)

### Requirement: Invocation: Logs go to stderr and every line names its operation

Logs MUST go to stderr, and only with `-v` (debug) or `-vv` (trace). Every line of a run MUST name its operation, the
lines of the threads a sync builds its branches on too: the root span `operation{op=<command> operation_id=<ULID>}`
(`OperationId::span`), `op` the words of the command (`status`, `lease get`); the JSON output carries the same id
(`operation_id`).

#### Scenario: The log lines of a command
- **WHEN** a command runs with `-v`
- **THEN** every line on stderr names the root span of its operation, whose `operation_id` the JSON output carries
  [@test] crates/bd-cli/src/operation_test.rs (`every_log_line_of_a_command_names_its_operation_whose_id_its_json_carries`)

#### Scenario: A nested command
- **WHEN** a command of two words runs (`lease get`)
- **THEN** `op` names both words
  [@test] crates/bd-cli/src/operation_test.rs (`the_operation_of_a_nested_command_names_both_words`)

### Requirement: Invocation: A closure handed to another thread carries the span of its caller

Every closure or future the cores hand to another thread MUST carry the span of its caller.

#### Scenario: The work of another thread
- **WHEN** a core hands a closure or a future to another thread
- **THEN** it carries the span of its caller, so its log lines name the operation
  [@test] crates/git-executor/src/thread_span_scan_test.rs (`every_closure_or_future_handed_to_another_thread_carries_the_span_of_its_caller`)

### Requirement: Flag names: A preview is `--dry-run` in every command

A preview that writes at most git objects (and the sync its command runs first) MUST be `--dry-run`, in every
command: `brd safe-push`, `brd land` and `brd request-review` too. `--patronus-dry-run` is a Patronus Dry Run, a robot
that runs; with `--dry-run` it is the preview of one.

#### Scenario: The preview of a Safe Push and of a Patronus Dry Run
- **WHEN** an agent previews `brd safe-push`, `brd land` or `brd request-review`
- **THEN** the flag is `--dry-run`, as in every other command
- **AND** `--patronus-dry-run` starts a Patronus Dry Run, a robot that runs, which `--dry-run` previews

### Requirement: Flag names: A flag that accepts a refusal is `--allow-<what refused>`

A flag that accepts a refusal, given only once the user agreed, MUST be `--allow-<what refused>`, named after the
error code it lifts where one code is lifted.

#### Scenario: The flags that accept a refusal
- **WHEN** a command offers to go past a refusal on the user's word
- **THEN** its flag is one of `--allow-unchecked` (`unchecked_tree`), `--allow-checks-not-passed`
  (`checks_not_passed`), `--allow-dangling`, `--allow-missing`, `--allow-unknown-rules`, `--allow-unknown-review`,
  `--allow-live-session`

### Requirement: Flag names: An act keeps its own verb

An act MUST keep its own verb (`--force` of a push, `--cancel-running`, `--restart-running`); `--without-approvals` (a
mode of the start, with its profiles) and `--bypass-rules` (GitHub's word, three codes) SHALL stay as they are.

#### Scenario: A flag that acts
- **WHEN** a flag drops, cancels or restarts something, or names a mode of the start
- **THEN** it keeps its verb: `--force`, `--cancel-running`, `--restart-running`, `--without-approvals`,
  `--bypass-rules`

### Requirement: Flag names: One sense has one word, and one flag one sense

One sense MUST have one word in every command (`crates/bd-cli/src/flag_names.rs`), and one flag MUST have one sense:
`brd update --force` was two, now `--overwrite-edited` (with `--materialize`) and `--allow-live-session` (with
`--abort --worktree`).

#### Scenario: One sense in two commands
- **WHEN** two commands have a flag of the same sense
- **THEN** it is the same word in both: `--dry-run` in every command, `--allow-unchecked` of `brd lease pick` and
  `brd pick`

#### Scenario: The two senses of the old `--force` of `brd update`
- **WHEN** an agent overwrites edited files of a materialize, or aborts the update of a live session
- **THEN** the flag is `--overwrite-edited` (with `--materialize`), or `--allow-live-session` (with `--abort --worktree`)

### Requirement: Flag names: A renamed flag keeps its old name and says the new one

A renamed flag MUST keep its old name (an alias, `--force` of `brd update` a hidden flag), and a run that uses it MUST
print one line on stderr, `brd: <old> of brd <command> is now <new>`, its stdout unchanged.

#### Scenario: The renamed flags stay
- **WHEN** the flags of every command are read
- **THEN** each renamed flag stays and names a flag that exists: `--check` of `safe-push`, `land` and
  `request-review`, `--without-checks` of `land`, `--unchecked` of `lease pick` and `pick`, `--force` of `update`
  [@test] crates/bd-cli/src/flag_names_test.rs (`every_renamed_flag_stays_and_names_flags_that_exist`)

#### Scenario: A run with an old name
- **WHEN** a command runs with the old name of a flag
- **THEN** it runs as with the new one, its stdout unchanged
- **AND** stderr has one line, `brd: <old> of brd <command> is now <new>`
  [@test] crates/bd-cli/src/flag_names_test.rs (`a_renamed_flag_says_its_name_now`, `an_old_name_runs_as_the_new_one`)
  [@test] crates/bd-remote/src/land_test.rs (`lands_the_copy_of_a_branch_into_master_once_the_user_accepted_what_ci_did_not_check`)

### Requirement: Flag names: A flag of another command names the commands that have it

A flag the command lacks MUST be `invalid_arguments` (exit 2) whose message names the commands that have it, by its
visible name, then the visible flags of this command that preview or accept a refusal, then its usage. clap's similar
name SHALL stay only when no other command has the flag, as a visible name: a hidden alias MUST NOT be offered.

#### Scenario: A flag of another command
- **WHEN** a command runs with a flag that only other commands have, or an alias of one
- **THEN** it is `invalid_arguments` (exit 2) naming those commands by the visible name (an alias found too:
  `--unchecked` names `--allow-unchecked` of `brd lease pick` and `brd pick`)
- **AND** then the visible flags of this command that preview or accept a refusal, then its usage
  [@test] crates/bd-cli/src/flag_names_test.rs (`a_flag_of_another_command_is_named_with_that_command`)

### Requirement: Output contract: stdout holds one JSON object of schema 2

stdout MUST hold exactly one JSON object per run, and every object MUST have `"schema": 2` (the experimental
`brd review-plan` has 3, openspec/specs/branch/review-plan/spec.md).

#### Scenario: The object of a run
- **WHEN** a command runs, `brd version` and `brd --version` too
- **THEN** stdout holds one JSON object with `"schema": 2`
  [@test] crates/bd-cli/src/version_test.rs (`version_and_dash_dash_version`)
  [@test] crates/bd-cli/src/schema_test.rs (`schema_lists_every_output`)

### Requirement: Output contract: Every object carries the `operation_id` of its run

Every object MUST carry, after its own fields, the `operation_id` of the run (a ULID, new for each run, an error object
and `brd --version` included): the id of the root span of its log lines, and of its record in the operations registry
and its entry in the journal when it runs an operation of the remote, a rewrite, an update or a pick that writes.

#### Scenario: The id of the log lines and of the JSON
- **WHEN** a command runs
- **THEN** its JSON ends with an `operation_id` new for this run, the one its log lines name
  [@test] crates/bd-cli/src/operation_test.rs (`every_log_line_of_a_command_names_its_operation_whose_id_its_json_carries`)

### Requirement: Output contract: The JSON is ASCII

The JSON MUST be ASCII: every other character is a `\uXXXX` escape (a UTF-16 pair above the BMP), so a reader that
decodes stdout with a legacy code page, as Windows PowerShell 5.1 does, still gets every path and subject.

#### Scenario: A text that is not ASCII
- **WHEN** an output holds a character outside ASCII, from a body Windows PowerShell 5.1 passed
- **THEN** stdout has it as a `\uXXXX` escape, a UTF-16 pair above the BMP
  [@test] crates/bd-rewrite/src/describe_test.rs (`a_body_from_windows_powershell_is_read_and_printed_as_ascii`)

### Requirement: Output contract: A plan, a body and a message are UTF-8 or UTF-16 with a mark

A plan, a body and a message (`--plan`, `--body-file`, `--message-file`; `-` is stdin) MUST be read as UTF-8 without
a leading byte order mark, or as UTF-16 with one, which Windows PowerShell 5.1 writes.

#### Scenario: A byte order mark and UTF-16
- **WHEN** an input starts with a byte order mark, of UTF-8 or of UTF-16
- **THEN** the mark is dropped and UTF-16 is decoded
  [@test] crates/bd-core/src/text_input_test.rs (`a_bom_is_dropped_and_utf16_is_decoded`)

#### Scenario: A plan with a byte order mark
- **WHEN** `--plan` names a file with a byte order mark
- **THEN** the plan is read
  [@test] crates/bd-rewrite/src/check_test.rs (`a_plan_with_a_byte_order_mark_is_read`)

### Requirement: Output contract: `brd schema --all` prints the schema every output keeps

`brd schema --all` MUST print the JSON Schema of every output (the list is the test's), and the real outputs MUST use
only the properties it declares.

#### Scenario: Every output has a schema
- **WHEN** `brd schema --all` runs
- **THEN** it prints the JSON Schema of every output
  [@test] crates/bd-cli/src/schema_test.rs (`schema_lists_every_output`)

#### Scenario: The outputs of the commands match it
- **WHEN** a command prints its output, of an update, a fold or a move-hunk among them
- **THEN** it uses only the properties its schema declares
  [@test] crates/bd-cli/src/schema_test.rs (`outputs_match_schema`)
  [@test] crates/bd-update/src/update_test.rs (`update_outputs_match_the_schema`)
  [@test] crates/bd-rewrite/src/fold_test.rs (`fold_is_refused_on_another_head_or_while_the_lock_is_held`)
  [@test] crates/bd-rewrite/src/move_hunk_test.rs (`the_fix_moves_into_the_commit_it_belongs_to_and_the_sync_rebuilds_the_branch`, `a_commit_between_on_the_same_lines_conflicts_and_another_head_or_the_lock_refuse`)

### Requirement: Output contract: `brd schema` without a name is the index of the commands

`brd schema` without a name MUST print the index of every command and the names of its outputs, under 4 KB, so that
an agent never reads every schema for one field; every command but `help` and `schema` MUST name outputs that exist,
and together they every output but `error`.

#### Scenario: The index
- **WHEN** `brd schema` runs without a name
- **THEN** it prints every command with the names of its outputs, under 4 KB
- **AND** the commands but `help` and `schema` name outputs that exist, together every output but `error`
  [@test] crates/bd-cli/src/schema_test.rs (`schema_without_a_name_is_the_index_of_the_commands`)

### Requirement: Output contract: `brd schema` with names prints only those schemas

`brd schema <command or output>...` MUST print only those schemas (`group`: `rewrite`), and an unknown name MUST be
`invalid_arguments` naming every command and output.

#### Scenario: The schema of a command
- **WHEN** `brd schema` names a command
- **THEN** it prints only the schemas of its outputs (`group`: `rewrite`)
  [@test] crates/bd-cli/src/schema_test.rs (`schema_of_a_command_is_only_its_outputs`)

#### Scenario: An unknown name
- **WHEN** `brd schema` names no command and no output
- **THEN** it is `invalid_arguments` naming every command and output

### Requirement: Output contract: The read-only commands write at most git objects and the cache

`status`, `commits`, `suggest`, `check`, `robot`, `schema`, `version`, `describe` without a setter, `update --status`
without `--materialize`, `lease status`, and the `--dry-run` of `group`, `rename`, `restack`, `unstack`, `update`
(also with `--continue`), `pick`, `fold`, `squash`, `move-hunk` and `reword` MUST write no ref, config, index or
working tree: at most git objects (a trial sync, a replay, the trees of a move) and the object cache (none with
`--no-cache`), and HEAD MUST stay.

#### Scenario: A read-only command
- **GIVEN** staged and untracked work
- **WHEN** a read-only command or a dry run runs, with `--no-cache` and without
- **THEN** no ref, config, index or working tree changed and HEAD stayed
- **AND** with `--no-cache` the object cache was not written
  [@test] crates/bd-cli/src/agent_help_test.rs (`read_only_commands_write_no_ref_config_index_or_working_tree`)

#### Scenario: The dry run of an update
- **WHEN** `brd update --dry-run` runs
- **THEN** it neither fetches nor writes
  [@test] crates/bd-update/src/update_test.rs (`dry_run_neither_fetches_nor_writes`)

#### Scenario: A robot that failed
- **WHEN** `brd robot` reads a failed robot
- **THEN** it says why and which local commits it tested, writing nothing
  [@test] crates/bd-remote/src/robot_test.rs (`a_failed_robot_says_why_and_which_local_commits_it_tested`)

### Requirement: Output contract: The help says which commands only read

`brd --help` and `brd help agent` MUST say which commands write no ref, config, index or working tree, and that
`push --dry-run`, `safe-push --dry-run` and `request-review --dry-run` sync first.

#### Scenario: The read-only line of the help
- **WHEN** an agent reads `brd --help` or `brd help agent`
- **THEN** it has the line of the read-only commands, saying that `push --dry-run`, `safe-push --dry-run` and
  `request-review --dry-run` sync first
  [@test] crates/bd-cli/src/agent_help_test.rs (`read_only_commands_write_no_ref_config_index_or_working_tree`)

### Requirement: Output contract: A failure prints the error object with a stable code

A failure MUST print `{"schema":2,"error":{"code","message","commits","retryable"}}` with a stable snake_case code,
and `paths` for `uncommitted_overlap` (the local files) and for a refused resolution of `brd update --continue` (the
conflicting files).

#### Scenario: The error object
- **WHEN** a command fails
- **THEN** stdout holds the error object with its snake_case code, `message`, `commits` and `retryable`
- **AND** `paths` names the local files of `uncommitted_overlap` and the conflicting files of a refused resolution
  [@test] crates/bd-cli/src/schema_test.rs (`outputs_match_schema`)

### Requirement: Output contract: A write whose next step failed prints its output with `after_error`

A write whose step after the write failed MUST print its output with that error as `after_error`, never the error
object, so that the undo token stays.

#### Scenario: A failed step after the write
- **WHEN** a rewrite wrote its commits and the write of its descriptions failed after it
- **THEN** stdout holds the output of the rewrite with its undo token, and the error as `after_error`
  [@test] crates/bd-rewrite/src/rewrite_test.rs (`a_failed_description_write_is_the_error_of_the_output_and_keeps_the_undo_token`)

### Requirement: Output contract: The names of the outputs are one vocabulary

The names MUST be one vocabulary: a write prints `previous_head` and `new_head`, a commit and its copy are
`{commit, new_id}`, a state is `state`.

#### Scenario: The fields of the outputs
- **WHEN** outputs name the head before and after a write, a recreated commit or a state
- **THEN** they are `previous_head` and `new_head`, `{commit, new_id}` and `state` in every output
  [@test] crates/bd-cli/src/schema_test.rs (`outputs_match_schema`)

### Requirement: Output contract: A refusal names the condition of every flag past it

A refusal MUST name in its `message` the condition of every way past it that it offers, next to it: a flag that
starts anyway (`--allow-dangling`, `--allow-missing`: only when the user says it is a false alarm;
`--allow-unknown-rules`, `--without-approvals`: only when the user says so), and one that drops, cancels or replaces
(`--force`, `--cancel-running`, `--restart-running`, `--confirm-preserved`: once the user approved).

#### Scenario: A dangling reference
- **WHEN** a dangling reference refuses the Safe Push
- **THEN** the message describes the references and offers `--allow-dangling` only when the user says it is a false
  alarm
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_dangling_reference_refuses_the_safe_push_unless_allowed`)
  [@test] crates/bd-core/src/check_summary_test.rs (`a_dangling_reference_message_describes_two_whole_and_counts_from_three_on`)

#### Scenario: Rules GitHub does not answer
- **WHEN** the rules of GitHub have no answer
- **THEN** the message offers `--allow-unknown-rules` only when the user says so
  [@test] crates/bd-remote/src/safe_push_test.rs (`rules_github_does_not_answer_refuse_the_safe_push_unless_allowed`)

### Requirement: Output contract: A refusal by a limit of the user says never to raise it

A refusal by a limit that is the user's MUST say so in its `message`: `push_too_large`: never raise
`branchdeck.pushSizeLimit` yourself; `low_disk_space`: never pass `--min-free 0` on your own.

#### Scenario: A push over the size limit
- **WHEN** a push is over the size limit
- **THEN** it is `push_too_large` before the remote, and the message says never to raise the limit yourself
  [@test] crates/bd-remote/src/push_test.rs (`a_push_over_the_size_limit_is_push_too_large_before_the_remote`)

#### Scenario: A lease below the minimum free space
- **WHEN** `brd lease get` finds less free space than the minimum
- **THEN** it is `low_disk_space`, naming it, and the message says never to pass the flag that lowers it on your own
  [@test] crates/bd-lease/src/lease_test.rs (`a_get_below_the_minimum_free_space_refuses_and_names_it`)

### Requirement: Output contract: A refusal that no flag gets past says what to do

A refusal that no flag gets past MUST say what to do: show the user the answer of the remote, `brd sync` for a branch
not synced, `brd update` for a branch that landed, tell the user of a parked branch and never unpark it, move fewer
blocks of a `hunk_conflict`, sign in again for a token GitHub rejected.

#### Scenario: The remote refuses the push
- **WHEN** the remote refuses a push with its leases holding
- **THEN** it is `push_refused`, and the message says to show the user the answer of the remote
  [@test] crates/bd-remote/src/push_test.rs (`a_push_the_remote_refuses_with_its_leases_holding_is_push_refused`)

#### Scenario: A hunk conflict
- **WHEN** a move of blocks conflicts with a commit between
- **THEN** the message says to move fewer blocks
  [@test] crates/bd-rewrite/src/move_hunk_test.rs (`a_commit_between_on_the_same_lines_conflicts_and_another_head_or_the_lock_refuse`)

### Requirement: Output contract: The error object carries the `next_steps` ready to run

The error object MUST carry `next_steps`, the commands that act on the refusal, ready to run (`brd -C <repository>` as
the other printed commands), each `{command, needs_approval}`, and the same in each `refusals[]` of
`brd safe-push --dry-run`; it is left out when there is none.

#### Scenario: A refusal with steps
- **WHEN** a refusal has commands that act on it
- **THEN** its error object, or its entry of `refusals[]`, lists them in `next_steps`, each `{command, needs_approval}`
- **AND** without such a command `next_steps` is left out
  [@test] crates/bd-cli/src/error_test.rs (`a_next_step_that_writes_needs_the_approval_and_none_starts_anyway`)

### Requirement: Output contract: A next step that writes needs the approval of the user

A step that pushes, rewrites, updates or cancels or restarts a robot (`push`, `safe-push` and `request-review` without
`--dry-run`, `update`, `restack`, `--force`, `--cancel-running`, `--restart-running`) MUST have `needs_approval: true`,
and runs only once the user approved it; one that only reads (`brd robot`, `brd sync`, `safe-push --dry-run`) has it
false.

#### Scenario: A step that writes and one that reads
- **WHEN** the next steps of a refusal are printed
- **THEN** a step that writes has `needs_approval: true` and one that only reads has it false
  [@test] crates/bd-cli/src/error_test.rs (`a_next_step_that_writes_needs_the_approval_and_none_starts_anyway`)

### Requirement: Output contract: A suppression is never a next step

A suppression (`--allow-*`, `--without-approvals`, `--min-free`, a change of the config) MUST NOT be a step: it stays
in `message` with its condition.

#### Scenario: A refusal a flag could lift
- **WHEN** a refusal can be lifted by a flag that starts anyway
- **THEN** no next step has that flag, and the message names it with its condition
  [@test] crates/bd-cli/src/error_test.rs (`a_next_step_that_writes_needs_the_approval_and_none_starts_anyway`)

### Requirement: Output contract: The next steps of each refusal

The next steps of a refusal SHALL be the commands its code names below. The field MUST be named `next_steps`, never
`next`, whose commands (`brd workspace`, `brd update`, `brd pick`) run at once.

#### Scenario: A leased push that the remote rejects
- **WHEN** a push is `lease_rejected`
- **THEN** the step is the leased push that replaces the remote branch
  [@test] crates/bd-remote/src/push_test.rs (`a_moved_remote_branch_is_fetched_and_reported_and_force_replaces_it`)

#### Scenario: A push the remote refuses
- **WHEN** a push is `push_refused`
- **THEN** the step of a branch behind its remote is the push that replaces it, and of a branch not synced `brd sync`
  [@test] crates/bd-remote/src/push_test.rs (`all_needs_yes_and_unsynced_branches_are_refused`)

#### Scenario: A robot that runs
- **WHEN** a start is `robot_running`
- **THEN** the steps are `brd robot` of the robot and the `--cancel-running` or `--restart-running` of the same command
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_robot_on_the_last_commit_refuses_and_one_on_an_older_commit_is_canceled_when_told`, `push_refuses_a_branch_whose_safe_push_runs_and_restarts_it_when_told`)

#### Scenario: Missing approvals without a pull request
- **WHEN** a start is `approvals_missing` and the branch has no pull request
- **THEN** the step is the `--create-pull-request` of the start
  [@test] crates/bd-remote/src/safe_push_test.rs (`missing_approvals_refuse_the_safe_push_unless_told_to_start_anyway`, `the_approvals_count_in_the_pull_request_it_opens_and_the_safe_push_starts_from_it`)

#### Scenario: A refusal by the code a branch uses
- **WHEN** a start is `dangling_reference` or `missing_declaration`
- **THEN** the step of `dangling_reference` is the `fix.command` a trial build proved, of `missing_declaration` the
  `brd restack` it names
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_name_another_branch_adds_refuses_the_safe_push_unless_allowed`)

#### Scenario: A push that dismisses the approvals
- **WHEN** a start is `push_dismisses_approvals`
- **THEN** the steps are the push and then the review request
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_push_that_dismisses_the_approvals_patronus_counts_refuses_the_safe_push`)

#### Scenario: A branch that landed or whose target conflicts
- **WHEN** a refusal is `target_conflict` or `landed`
- **THEN** the step is `brd update`
  [@test] crates/bd-remote/src/landed_test.rs (`push_and_safe_push_refuse_a_branch_a_merged_safe_push_landed_whole`)

#### Scenario: A whole stack that conflicts or is not built
- **WHEN** a start is `stack_conflict` or `stack_not_built`
- **THEN** the step of `stack_conflict` is the `--dry-run` of the branch whose commit conflicts, of `stack_not_built`
  `brd sync`
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_whole_stack_that_conflicts_is_refused_with_the_conflict`)

### Requirement: Output contract: The exit code follows the code of the error

The exit code MUST follow the code: 0 ok, 1 conflict, 2 invalid input, 3 stale (retry), 4 refused, 5 git or network
error.

#### Scenario: The exit code of every code
- **WHEN** a command fails with an error code
- **THEN** it exits with the exit code of the class of that code
  [@test] crates/bd-cli/src/error_test.rs (`every_error_code_has_the_exit_code_and_the_retry_of_its_class`)

### Requirement: Output contract: Exit 1 is a conflict found, never an error object

Exit 1 MUST never be an error object: it is the output of a command that found a conflict (`state: "conflict"`, a
branch not built, `ok: false`); a conflict brd refuses to work around is exit 4.

#### Scenario: A conflict found and a conflict refused
- **WHEN** a command finds a conflict
- **THEN** it prints its output and exits with 1
- **AND** a conflict brd refuses to work around is an error that exits with 4

### Requirement: Output contract: `retryable` only where the same command can succeed

`retryable` MUST be true only where the same command again can succeed: never for `lease_rejected` (a push again with
`--force` would drop what the remote has now) nor for a stale undo token (`undo_stale`).

#### Scenario: Codes that are never retried
- **WHEN** a command fails with `lease_rejected` or `undo_stale`
- **THEN** `retryable` is false
  [@test] crates/bd-cli/src/error_test.rs (`every_error_code_has_the_exit_code_and_the_retry_of_its_class`)

### Requirement: Output contract: `retryable` is the `retry` class the app's `ApiError` carries

`retryable` MUST be the `retry` class of the code other than `no`, the one the app's `ApiError` carries: `same` for
`locked` and `network_error`, `after_reread` for the stale reads, `plan_changed` among them (a remote action planned
over facts that changed before its first effect did nothing, and its plan is read again).

#### Scenario: The retry of a code
- **WHEN** an error has a code
- **THEN** it carries the `retry` of that code, and `retryable` is true for every class but `no`
  [@test] crates/domain-types/src/error_test.rs (`an_error_carries_the_retry_of_its_code`)
  [@test] crates/bd-cli/src/error_test.rs (`every_error_code_has_the_exit_code_and_the_retry_of_its_class`)

### Requirement: Output contract: The codes are one registry with the app

The codes MUST be one registry with the app: `canceled` (an action the user stopped) exits with 4, `internal` (a
failure without a code of its own, which the app answers; `brd` prints a failure of git or IO as `git_error`) with 5.

#### Scenario: A canceled action and a failure without a code
- **WHEN** an action the user stopped fails, or a failure has no code of its own
- **THEN** the first is `canceled`, exit 4, the second `internal`, exit 5, in the app, and `git_error` in `brd`
  [@test] crates/domain-types/src/error_test.rs (`an_error_without_a_code_of_its_own_is_internal_and_canceled_is_a_code`)

### Requirement: Output contract: A push, a start or a review request is `busy` beside the same one

A push, a Safe Push or a Dry Run and a review request (`brd push`, `brd safe-push`, `brd request-review`) MUST be
recorded in the operations registry of the repository before its run, and MUST be `busy` (exit 3, retryable,
`retry: same`), nothing pushed or started, while an operation of another process (the app, another `brd`, in any
worktree of the repository) runs the same intent or holds what it needs: a branch it lands or pushes, a robot, a
field of a pull request.

#### Scenario: A Safe Push beside the Safe Push of another process
- **WHEN** `brd safe-push` starts while another process runs the Safe Push of the same branch
- **THEN** it is `busy`, exit 3, nothing pushed or started
  [@test] crates/bd-cli/src/cross_process_test.rs (`a_safe_push_beside_the_safe_push_of_another_process_is_busy_with_its_holder`)

#### Scenario: The registry while its holder lives
- **WHEN** the registry of the repository holds a record whose process lives
- **THEN** an admission of what it holds is `busy`, naming that holder
  [@test] crates/operations/src/registry_file_test.rs (`the_file_is_busy_while_the_holder_lives_and_an_entry_takes_its_record_out`)

#### Scenario: A registry of another schema
- **WHEN** the registry file of the repository is of another schema
- **THEN** an admission is `busy`, and the file is never written
  [@test] crates/operations/src/registry_file_test.rs (`a_registry_of_another_schema_is_busy_and_never_written`)

### Requirement: Output contract: An operation of another branch runs beside it

An operation of another branch MUST run beside such a push, start or review request.

#### Scenario: An operation of another branch
- **WHEN** an operation of another branch runs
- **THEN** the push, the start or the review request runs beside it
  [@test] crates/operations/src/registry_file_test.rs (`the_file_is_busy_while_the_holder_lives_and_an_entry_takes_its_record_out`)

### Requirement: Output contract: `error.holder` names the operation that holds it

`error.holder` MUST name that operation (`op`, `tool`, `pid`, `kind`, `subject`, `held_for`), the message who runs
what.

#### Scenario: The holder of a busy answer
- **WHEN** a command is `busy`
- **THEN** `error.holder` names the operation of the other process with `op`, `tool`, `pid`, `kind`, `subject` and
  `held_for`, and the message says who runs what
  [@test] crates/bd-cli/src/cross_process_test.rs (`a_safe_push_beside_the_safe_push_of_another_process_is_busy_with_its_holder`)

### Requirement: Output contract: The record of an operation leaves once it ended or its process is gone

The record of an operation MUST leave the registry once it ended, or at the next admission once its process is
gone.

#### Scenario: The record of an ended operation
- **WHEN** an operation ends
- **THEN** its entry takes its record out of the registry
  [@test] crates/operations/src/registry_file_test.rs (`the_file_is_busy_while_the_holder_lives_and_an_entry_takes_its_record_out`)

#### Scenario: The record of a gone process
- **WHEN** the process of a record is gone
- **THEN** the record leaves the registry at the next admission
  [@test] crates/operations/src/registry_file_test.rs (`a_gone_process_of_this_host_leaves_at_once_and_one_of_another_host_after_five_minutes`)

### Requirement: Output contract: The operation of the app holds `brd` and `brd`'s holds the app

The app's operation MUST be such a holder too, and the app MUST refuse its own the same way while `brd` holds what it
needs.

#### Scenario: `brd` beside the Safe Push of the app
- **WHEN** `brd safe-push` of a branch starts while the app runs its Safe Push
- **THEN** it is `busy`, naming the app
  [@test] crates/bd-cli/src/cross_process_test.rs (`bd_beside_the_apps_safe_push_of_the_same_branch_is_busy_naming_the_app`)

#### Scenario: The app beside the Safe Push of `brd`
- **WHEN** the app starts the Safe Push of a branch whose `brd safe-push` runs
- **THEN** the app's is `busy`, naming `brd`
  [@test] crates/bd-cli/src/cross_process_test.rs (`the_apps_safe_push_beside_bd_of_the_same_branch_is_busy_naming_bd`)

### Requirement: Output contract: A rewrite, an update and a pick are recorded before any lock

A rewrite that writes (`brd group`, `rename`, `restack`, `unstack`, `reword`, `fold`, `squash`, `park`, `unpark`,
`unapply`, `reapply`, `undo`, `move-hunk`), `brd update` and `brd pick` MUST be recorded in the operations registry of
the repository before the command takes any lock, as the intent of the app's family (`rewrite`, `updateProject`,
`pick`).

#### Scenario: A rewrite of `brd`
- **WHEN** a rewrite of `brd`, `brd update` or `brd pick` starts
- **THEN** it is recorded as a `rewrite`, `updateProject` or `pick` before it takes a lock

### Requirement: Output contract: The windows of the app show an operation of `brd` as `brd`'s

The app's windows MUST show such an operation on its rows as `brd`'s ("brd: Renaming auth to login" on the "⋯" of its
row, the name an item that an operation of `brd` keeps busy gives it too: "brd: Pushing branch auth…"); no item waits
for it.

#### Scenario: A rewrite of `brd` on its row
- **WHEN** `brd` runs a rewrite of a branch
- **THEN** the "⋯" of its row says it as `brd`'s while it runs
  [@test] tests/e2e/operations/foreign-operation.spec.ts (`should show the rewrite bd runs on the ⋯ of its row as bd's, and free the row once it ended`)

#### Scenario: The name of an operation of another process
- **WHEN** an item runs as an operation of another process, or is busy behind one
- **THEN** it is named by the one rule of its items, as its row, and names the one it is busy behind
  [@test] src/presenters/operations.test.ts (`names an operation by the one rule of its items: of another process as its row, of this window as it named it, else as the view`)
  [@test] src/presenters/actions.test.ts (`says on an item that runs as an operation its busy state as its row does, and names the one an item waits for or is busy behind`)

### Requirement: Output contract: The record of a rewrite leaves before its `--sync`

The record of a rewrite, an update or a pick MUST leave once the command ended, before its `--sync`, or at the next
admission once its process is gone.

#### Scenario: A `brd` killed in its rewrite
- **WHEN** a `brd` is killed while it rewrites
- **THEN** it leaves a record that the app drops once its process is gone
  [@test] crates/bd-cli/src/cross_process_test.rs (`a_bd_killed_in_its_rewrite_leaves_a_record_the_app_drops_once_its_process_is_gone`)

### Requirement: Output contract: HEAD makes no rewrite of another process `busy`

HEAD, which such a rewrite holds, MUST make no other process `busy`, nor the same rewrite of another process: the
repository lock orders them, and a rewrite of the app meanwhile is waited for.

#### Scenario: A rewrite of another process
- **WHEN** a rewrite starts while a rewrite of another process holds HEAD
- **THEN** it is not `busy`: it waits for the repository lock, while one of its own process is waited for
  [@test] crates/domain-policy/src/admission_test.rs (`a_rewrite_waits_for_the_rewrites_of_its_process_and_for_none_of_another`)

### Requirement: Output contract: Unstacks of the app and of `brd` beside them are one rewrite

Two Unstacks of a branch in the app and `brd unstack` of it beside them MUST be one rewrite of the app and nothing
more of `brd` (`invalid_plan` once it got the lock).

#### Scenario: Three unstacks of one branch
- **WHEN** the app unstacks a branch twice and `brd unstack` of it runs beside them
- **THEN** the app rewrites once, and `brd` rewrites nothing more: `invalid_plan` once it got the lock
  [@test] crates/bd-cli/src/cross_process_test.rs (`the_triple_unstack_of_the_app_and_bd_is_one_rewrite_journaled_once_by_each`)

### Requirement: Output contract: A dry run records nothing in the registry

A dry run, `brd update --status` and `brd check --fix` MUST record nothing.

#### Scenario: A command that only reads or previews
- **WHEN** a dry run, `brd update --status` or `brd check --fix` runs
- **THEN** the operations registry holds no record of it

### Requirement: Output contract: The run of an operation is journaled under its `operation_id`

The run of such an operation MUST be journaled once it ended, in the journal of the operations of the repository
(`$GIT_COMMON_DIR/branch-deck/journal.jsonl`, the app's operations in the same file) under the `operation_id` of the
JSON, with what it was, what it did and what it cost.

#### Scenario: A Safe Push and a rewrite in the journal
- **WHEN** a Safe Push or a rewrite of `brd` ended
- **THEN** the journal has its entry under the `operation_id` of its JSON
  [@test] crates/bd-cli/src/operation_test.rs (`a_safe_push_is_journaled_under_the_operation_id_of_its_json`, `a_rewrite_is_journaled_under_the_operation_id_of_its_json`)

#### Scenario: What the entry holds
- **WHEN** an operation of the app and one of `brd` ended, one in a slot of the lease pool
- **THEN** each is journaled once, the entry of `brd` with `tool: bd`, its pid, worktree and prefix, whether the
  worktree is a slot of the lease pool, the intent and its subject
- **AND** each effect with its status, what they answered or the error
- **AND** what the run cost from its admission (git processes by subcommand, HTTP requests, milliseconds)
  [@test] crates/operations/src/journal_test.rs (`an_operation_of_the_app_and_one_of_bd_are_journaled_once_each_with_what_they_cost`)
  [@test] crates/operations/src/journal_test.rs (`an_operation_in_a_slot_of_the_lease_pool_is_journaled_as_an_agents`)

### Requirement: Output contract: A run without an answer and a lost start are journaled

A run that ended without an answer MUST be journaled as `internal`; a start whose answer was lost MUST be journaled
again, under its id, once a read of Patronus settled it (`started`, `notStarted`) or it was forgotten (`forgotten`).

#### Scenario: A dropped operation
- **WHEN** an operation is dropped without an answer
- **THEN** it is journaled as `internal`
  [@test] crates/operations/src/journal_test.rs (`a_dropped_operation_is_journaled_as_internal_and_a_discarded_one_not_at_all`)

#### Scenario: A lost start settled later
- **WHEN** a start whose answer was lost is settled or forgotten
- **THEN** it is journaled again under its id as `started`, `notStarted` or `forgotten`
  [@test] crates/operations/src/journal_test.rs (`a_start_whose_answer_was_lost_is_journaled_again_once_settled_or_forgotten`)

### Requirement: Output contract: The journal keeps the newest entries of 30 days

The journal MUST keep the newest 200 entries of the last 30 days.

#### Scenario: An old or a 201st entry
- **WHEN** the journal has entries older than 30 days or more than 200
- **THEN** it keeps the newest 200 of the last 30 days
  [@test] crates/operations/src/journal_test.rs (`the_journal_keeps_the_newest_entries_of_the_last_thirty_days`)

### Requirement: Output contract: A start whose answer was lost is never sent again

A start of `brd safe-push` (and of the app) MUST record what it sends before its request goes out, and an answer lost
after it went out MUST leave it unknown, never sent again: the output says so in `start_unknown` (`operation`,
`reason`), and its record holds what it held for every process. A start of a process that is gone (killed between its
push and its start) is such a start too.

#### Scenario: A lost answer
- **WHEN** the answer of a start is lost after its request went out
- **THEN** the start is unknown once, never sent again, and the output says so in `start_unknown`
  [@test] crates/operations/src/reconcile_test.rs (`a_start_whose_answer_was_lost_is_unknown_once_and_holds_until_patronus_lists_its_robot`)

#### Scenario: A process killed between its push and its start
- **WHEN** a `brd` is killed between its push and its start
- **THEN** its start is such a lost start, which an admission settles
  [@test] crates/bd-cli/src/cross_process_test.rs (`a_bd_killed_between_its_push_and_its_start_leaves_a_lost_start_the_app_settles_and_starts_once`)

### Requirement: Output contract: An admission settles a lost start by a read of Patronus

An admission that a lost start holds MUST settle it first by a read of Patronus: a robot of it started
(`plan_changed`), none once 30 s passed since it was sent (it leaves, and the run goes as planned, pushing nothing
pushed already), else `busy` with `holder.unknown: true` and a message naming `brd operation forget <op>`.

#### Scenario: Patronus lists its robot
- **WHEN** an admission finds a lost start and Patronus lists a robot of it
- **THEN** it is `plan_changed`
  [@test] crates/operations/src/reconcile_test.rs (`a_start_whose_answer_was_lost_is_unknown_once_and_holds_until_patronus_lists_its_robot`)

#### Scenario: No robot yet
- **WHEN** Patronus lists no robot of it
- **THEN** once 30 s passed since it was sent it leaves and the run goes as planned, pushing nothing pushed already
- **AND** before that the command is `busy` with `holder.unknown: true`, its message naming `brd operation forget <op>`
  [@test] crates/bd-cli/src/cross_process_test.rs (`a_lost_start_holds_bd_until_bd_operation_forget_lets_it_go`)

### Requirement: Output contract: `brd operation forget` takes a lost start out on the user's word

`brd operation forget <op>` MUST take that record out, only when the user says so (`forgotten`, `subject`,
`source_ref`; `invalid_arguments` for an id the registry lacks or an operation that runs), and the next start MUST go.

#### Scenario: The user forgets a lost start
- **WHEN** the user says so and `brd operation forget` names the lost start
- **THEN** the record is taken out (`forgotten`, `subject`, `source_ref`) and the next start goes
  [@test] crates/bd-cli/src/cross_process_test.rs (`a_lost_start_holds_bd_until_bd_operation_forget_lets_it_go`)

#### Scenario: An id the registry lacks
- **WHEN** it names an id the registry lacks or an operation that runs
- **THEN** it is `invalid_arguments`

### Requirement: Output contract: A command that wrote touches the stamp of its repository

Every command of `brd` that succeeded and is no read (not in the read-only line of `brd --help`: a rewrite, a push, a
start, a sync, an update, a pick, a park, a description, the lease commands but `lease status`) MUST touch the stamp
of its repository once, after its run (`$GIT_COMMON_DIR/branch-deck/stamp`, its Unix time and pid): the app's windows
of the repository sync then without a focus.

#### Scenario: A mutation of `brd`
- **WHEN** a command of `brd` that is no read succeeded
- **THEN** the stamp of its repository holds its Unix time and pid, touched once after its run
  [@test] crates/bd-cli/src/cross_process_test.rs (`bd_moves_the_stamp_of_its_repository_after_a_mutation_only`)

### Requirement: Output contract: A read, a dry run and a failure leave the stamp

A read, a dry run and a command that failed MUST leave the stamp as it was.

#### Scenario: A read and a failed command
- **WHEN** a read, a dry run or a command that failed ran
- **THEN** the stamp is as it was
  [@test] crates/bd-cli/src/cross_process_test.rs (`bd_moves_the_stamp_of_its_repository_after_a_mutation_only`)

### Requirement: Output contract: A typed error of a core has one code in `brd` and the app

A typed error of a core MUST have one code in `brd` and in the app: the type names it (`ParkError::code`,
`GuardRefusal::code`, `UnapplyRefusal::code`, `ReapplyRefusal::code`), and local changes in the way of an unapply, a
reapply or their undo are `uncommitted_overlap`. Both sides SHALL be tested on one table of the typed errors and their
codes (`update_core::error_cases`).

#### Scenario: The refusals of a move, an unapply and a reapply
- **WHEN** a move, an unapply or a reapply is refused by a typed error
- **THEN** `brd` answers the code the app answers
  [@test] crates/bd-update/src/error_code_test.rs (`bd_answers_the_code_of_each_refusal_of_a_move_an_unapply_and_a_reapply_that_the_app_answers`)
  [@test] crates/app-api/src/api_error_test.rs (`api_error_answers_the_code_of_each_typed_error_that_bd_answers_and_an_undo_its_code_in_undo`)

#### Scenario: The refusals of a park and a need
- **WHEN** a park or a need is refused
- **THEN** `brd` answers the code the app answers
  [@test] crates/bd-rewrite/src/store_error_test.rs (`bd_answers_the_code_of_each_refusal_of_a_park_and_a_need_that_the_app_answers`)

#### Scenario: An error without a code
- **WHEN** a failure of a core or of a write of a store has no code of its own
- **THEN** `brd` answers `git_error`
  [@test] crates/bd-update/src/error_code_test.rs (`an_error_without_a_code_is_git_error_in_bd`)
  [@test] crates/bd-rewrite/src/store_error_test.rs (`a_write_of_a_store_that_fails_without_a_code_is_git_error_in_bd`)

### Requirement: Output contract: An undo answers `in_undo` and a moved ref is `undo_stale`

An undo MUST answer the code `in_undo` (`ErrorCode::in_undo`): a ref that moved on since the operation is
`undo_stale`, never retried, in `brd undo` and in the undo commands of the app alike, its words saying what moved and
that it can't be undone any more, never the advice to retry that a rewrite refused by the same move gives.

#### Scenario: The undo of `brd` and of the app
- **WHEN** an undo is refused
- **THEN** `brd undo` and the undo commands of the app answer the same code, by `in_undo`
  [@test] crates/bd-update/src/error_code_test.rs (`bd_undo_answers_the_code_of_each_refusal_that_the_app_answers_and_a_moved_head_makes_it_stale`)
  [@test] crates/app-api/src/api_error_test.rs (`api_error_answers_the_code_of_each_typed_error_that_bd_answers_and_an_undo_its_code_in_undo`)

#### Scenario: A ref that moved on since the operation
- **WHEN** a ref moved on since the operation an undo takes back, a new commit after a regroup or a squash among them
- **THEN** the undo is `undo_stale`, never retried, and says what moved and that it can't be undone any more
- **AND** it gives no advice to retry
  [@test] crates/app-api/src/api_error_test.rs (`a_stale_undo_says_it_cannot_be_undone_instead_of_advising_a_retry`)
  [@test] crates/bd-rewrite/src/rewrite_test.rs (`rename_restack_unstack_and_undo`)
  [@test] crates/domain-types/src/error_test.rs (`an_undo_that_a_moved_ref_refuses_is_stale_and_never_retried`)
  [@test] crates/test-server/src/reword_test.rs (`the_undo_of_a_regroup_after_a_new_commit_answers_undo_stale_never_retried`)
  [@test] crates/test-server/src/squash_test.rs (`the_undo_of_a_squash_after_a_new_commit_answers_undo_stale_never_retried`)

### Requirement: Output contract: Bad input exits with 2

An empty prefix MUST fail with `no_branch_prefix`, a path outside a repository with `not_a_repository`, bad arguments
with `invalid_arguments`, an unknown `--branch` with `unknown_branch`; all exit with 2.

#### Scenario: An empty prefix
- **WHEN** the prefix is empty
- **THEN** the command fails with `no_branch_prefix`, exit 2
  [@test] crates/bd-remote/src/status_test.rs (`empty_prefix_is_an_error`)

#### Scenario: A path outside a repository
- **WHEN** the path is outside a repository
- **THEN** the command fails with `not_a_repository`, exit 2
  [@test] crates/bd-remote/src/status_test.rs (`not_a_repository`)

#### Scenario: Bad arguments and an unknown branch
- **WHEN** the arguments are bad, or `--branch` names an unknown branch
- **THEN** the command fails with `invalid_arguments` or `unknown_branch`, exit 2
  [@test] crates/bd-remote/src/status_test.rs (`invalid_arguments`, `status_filters_and_pages`)

### Requirement: Output contract: A list cut at its limit leaves out two or more

A list cut at its limit MUST leave out two or more, never one: a list one longer than its limit is printed whole
(`sync_utils::list_cut::kept`), as the app cuts its lists and prompts (`cutList`).

#### Scenario: One longer than the limit, and longer
- **WHEN** a list is one longer than its limit
- **THEN** it is printed whole, and a longer one is cut at the limit
  [@test] crates/sync-utils/src/list_cut_test.rs (`a_list_one_longer_than_its_limit_is_kept_whole_and_a_longer_one_is_cut_at_the_limit`)

#### Scenario: The files and paths of an update and a pick
- **WHEN** `brd update` or `brd pick` lists its `files` and `files_omitted`, or `paths` and `blocked_paths_omitted`
- **THEN** a list one longer than its limit is whole
  [@test] crates/update-core/src/update_project_test.rs (`a_conflict_one_file_wider_than_the_list_reports_every_file_with_its_commits`, `many_blocking_paths_are_counted`)

#### Scenario: The lists of a robot
- **WHEN** `brd robot --full` lists the builds, test names and messages and their `omitted`, or a robot names its
  failed checks
- **THEN** a list one longer than its limit is whole
  [@test] crates/patronus/src/actions_test.rs (`a_list_of_the_full_failure_one_longer_than_its_limit_is_given_whole`)
  [@test] crates/patronus/src/actions_test.rs (`the_failed_checks_of_a_robot_are_named_whole_when_only_one_is_past_the_limit`)

#### Scenario: The failed checks of a pull request
- **WHEN** a pull request names its failed checks
- **THEN** a list one longer than its limit is whole
  [@test] crates/remote-facts/src/pull_requests_test.rs (`the_failed_checks_of_a_pull_request_are_named_whole_when_only_one_is_past_the_limit`)

#### Scenario: The dangling references of a refusal
- **WHEN** the refusal of `brd safe-push` describes its dangling references
- **THEN** a list one longer than its limit is whole
  [@test] crates/bd-core/src/check_summary_test.rs (`a_dangling_reference_message_describes_two_whole_and_counts_from_three_on`)

#### Scenario: The dropped commits of a remote
- **WHEN** a remote lists its `dropped_commits`
- **THEN** one more than the limit are listed whole
  [@test] crates/sync-core/src/detection_remote_status_test.rs (`test_dropped_commits_one_more_than_the_limit_are_listed_whole`)

### Requirement: Output contract: Commit ids are 8 characters unless two share them

Commit ids MUST be 8 characters, longer for the whole output when two of its ids share that prefix; `--full-ids` MUST
print full ids.

#### Scenario: Full ids
- **WHEN** a command runs with `--full-ids`
- **THEN** it prints full ids, else 8 characters, longer for the whole output when two ids share that prefix
  [@test] crates/bd-remote/src/status_test.rs (`full_ids`)

### Requirement: `brd status`: It prints HEAD, the baseline, the prefix and the branches

It MUST print HEAD, the baseline, the prefix and its source (`prefix_source`), the branches newest first by their
oldest commit, as the table orders them (name, ref, issue key, latest time, commit count, the tip of the synced
virtual branch or `null`, commits newest first without the prefix) and the unassigned commits.

#### Scenario: The shape of the status
- **WHEN** `brd status` runs
- **THEN** it prints HEAD, the baseline, the prefix with `prefix_source`, the branches in the order of the table and
  the unassigned commits
  [@test] crates/bd-remote/src/status_test.rs (`status_shape`)

### Requirement: `brd status`: Without a baseline it is `no_baseline`

Without a baseline (no upstream and no `origin/master` or `origin/main`, or no remote and no local `master` or
`main`) it MUST be `no_baseline`, the message naming each branch it tried once.

#### Scenario: No baseline
- **WHEN** the repository has no upstream and no `origin/master` or `origin/main`, or no remote and no local `master`
  or `main`
- **THEN** `brd status` is `no_baseline`, naming each branch it tried once
  [@test] crates/bd-remote/src/status_test.rs (`no_baseline_is_refused`)

### Requirement: `brd status`: A repository without commits is `no_baseline`

In a repository without commits (`git init`), every command that reads HEAD or the baseline (`status`, `commits`,
`suggest`, `check`, `sync`, `fold`, `update` and the rewrites) MUST be `no_baseline` with "The repository has no
commits yet: commit to the main branch, then sync", not a git error; telling it MUST cost no git process when HEAD
resolves.

#### Scenario: A repository just created
- **WHEN** a command that reads HEAD or the baseline runs in a repository without commits
- **THEN** it is `no_baseline` with "The repository has no commits yet: commit to the main branch, then sync"
  [@test] crates/bd-remote/src/status_test.rs (`no_commits_is_no_baseline`)

### Requirement: `brd status`: `--branch`, `--unassigned`, `--limit` and `--offset` filter and page

`--branch` MUST list one branch, `--unassigned` only the unassigned commits; `--limit`/`--offset` MUST page their
commits, and the branches otherwise.

#### Scenario: A filter and a page
- **WHEN** `brd status` runs with `--branch` or `--unassigned`, and `--limit`/`--offset`
- **THEN** it lists that branch or the unassigned commits, a page of their commits
- **AND** without a filter `--limit`/`--offset` page the branches
  [@test] crates/bd-remote/src/status_test.rs (`status_filters_and_pages`)

### Requirement: `brd status`: It runs no integration detection and no network command

It MUST NOT run integration detection, an update check or a network command of git.

#### Scenario: A status
- **WHEN** `brd status` runs
- **THEN** it runs no integration detection, no update check and no network command of git

### Requirement: `brd status`: A branch a merged Safe Push landed shows `landed_by`

On Space, with the Safe Push CLI logged in, each branch a merged Safe Push landed MUST show `landed_by` until the
update archives it, as the rows of the app say it ([Patronus](../../branch/patronus/spec.md#merged)), decided by
content (`robot_landings`: the commits the robot tested against the branch's); `--brief` MUST show the `branch`,
`whole_stack` and the count of `after`.

#### Scenario: A branch a merged Safe Push landed
- **GIVEN** Space, with the Safe Push CLI logged in
- **WHEN** a merged Safe Push landed a branch
- **THEN** `landed_by` names the `branch` whose Safe Push it was (the branch itself, a branch built on it, or the root
  with `whole_stack`), the `robot`, the `commits` it landed and `after`, the commits added since, the only work left
- **AND** `--brief` gives the `branch`, `whole_stack` and the count of `after`
  [@test] crates/bd-remote/src/landed_test.rs (`status_names_the_branches_a_merged_safe_push_landed_and_what_is_left_of_them`)

### Requirement: `brd status`: The landings are read from the cache, else once from Patronus

The push URL of `origin` (`remote get-url`) and the user's robots MUST be kept in the object cache while they hold,
else one list and one poll ([Patronus](../../branch/patronus/spec.md#load-on-patronus)); off Space, without a token or
an answer of Patronus nothing is landed.

#### Scenario: Robots kept in the cache
- **WHEN** `brd status` runs again while the kept robots hold
- **THEN** it asks Patronus once, and a status costs the git processes of its budget
  [@test] crates/bd-remote/src/robot_list_test.rs (`status_asks_patronus_once_while_the_kept_robots_hold`)
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_status`)

#### Scenario: Off Space or without an answer
- **WHEN** the repository is off Space, or there is no token or no answer of Patronus
- **THEN** nothing is landed

### Requirement: `brd status`: Each branch on Space shows its `landing_needs`

On Space (the push URL of `origin`), each branch MUST show `landing_needs`: what its Safe Push needs of a commit
outside what it lands, the refusals `brd safe-push` would give its start
(`trial_check::landing_gate::landing_needs`, one trial check of every branch, less what merged Safe Pushes landed), so
an agent sees them without `brd check`.

#### Scenario: A need of a branch outside it
- **GIVEN** Space
- **WHEN** a branch uses a commit of a branch outside what it lands
- **THEN** its `landing_needs` has per need the `code` (`missing_declaration`, `dangling_reference`), the
  `identifier`, `at` (`file:line` of the use), the commit it `needs` and its `needs_branch`
- **AND** the `fix` (`brd restack <root> <branch>`, which the user approves first, or `move <commit> into <branch>`)
  and `landing_in`, the robot of the running Safe Push that lands `needs_branch`
  [@test] crates/bd-remote/src/landed_test.rs (`status_names_what_the_safe_push_of_a_branch_needs_of_a_branch_outside_it`)
  [@test] crates/trial-check/src/landing_gate_test.rs (`the_needs_of_a_branch_are_the_refusals_of_its_start`)

### Requirement: `brd status`: An empty `landing_needs` needs nothing, an omitted one is unknown

An empty `landing_needs` MUST mean that nothing is needed, and a branch built on one that needs a branch needs it
too. It MUST be omitted off Space, with `--brief` or `--unassigned`, and when the check can't run: unknown, not empty.

#### Scenario: A branch built on one that needs a branch
- **WHEN** a branch is built on one that needs a branch
- **THEN** its `landing_needs` names that need too

#### Scenario: The check can't run
- **WHEN** the repository is off Space, `--brief` or `--unassigned` is given, or the check can't run
- **THEN** `landing_needs` is omitted, unknown rather than empty

### Requirement: `brd status`: `landing_needs` lists the declared needs first on every route

On every route `landing_needs` MUST also list, first, the order the user declared (`brd need`, [Declared
Needs](../../branch/declared-needs/spec.md)): `code: unlanded_need`, `needs_branch`, its `state` (`active`,
`unapplied`, `missing`, `companion_unreadable`), `inherited_from` and `fix` (`brd unstack <branch>`) of a need a
branch inherits from the branch it is built on; off Space only a branch such a need refuses has `landing_needs`.

#### Scenario: A declared need off Space
- **GIVEN** off Space
- **WHEN** a branch has a declared need
- **THEN** only that branch has `landing_needs`, with `code: unlanded_need`, `needs_branch` and the `state` of the
  branch it names
  [@test] crates/bd-remote/src/status_test.rs (`a_declared_need_is_in_the_status_off_space_with_the_state_of_the_branch_it_names`)

#### Scenario: A need inherited from the branch it is built on
- **WHEN** a branch is built on a branch with a declared need that has not landed
- **THEN** its `landing_needs` names the need with `inherited_from` and `fix`
  [@test] crates/bd-remote/src/landed_test.rs (`a_declared_need_refuses_the_start_of_the_branch_and_of_the_one_built_on_it_until_it_lands`)

### Requirement: `brd status`: Each branch and whole stack shows the `next_step` of its row

Each branch, and in `stacks` each whole stack of a stack that forks, MUST show `next_step`: the step the app's row
shows by the same rule (`domain_policy::next_step`, [Next Step](../../branch/next-step/spec.md#the-table)) over what
`brd status` read.

#### Scenario: The next step of a branch and of a whole stack
- **WHEN** `brd status` runs on Space
- **THEN** each branch and each whole stack in `stacks` has `next_step`: `row` of the table (`0` waits for a fact, `1`
  has nothing to start, `1″` `pick_remote_commits`: someone else pushed to the remote branch, `brd pick --remote`
  takes it in, … `11` a Safe Push), `step`, `waits_for` (the fact without an answer, or whose read failed) and
  `pull_request`
- **AND** why a Safe Push or a Dry Run can't start now (`safe_push_blocked`, `dry_run_blocked`, `null` when it can)
  [@test] crates/bd-remote/src/landed_test.rs (`status_gives_each_branch_and_whole_stack_the_next_step_of_the_apps_row`)
  [@test] crates/bd-remote/src/scenario_test.rs (`the_stack_and_whole_stack_cases_answer_alike_in_the_app_and_bd`)

### Requirement: `brd status`: Each branch shows the `status` its Status cell says

The `next_step` of a branch MUST show `status`, what its Status cell in the app says by the same rule
(`domain_policy::row_status`), the main fact first: `robot_running` before `blocked`, so a step settled by a running
robot says it.

#### Scenario: The status of a branch
- **WHEN** `brd status` gives the next step of a branch
- **THEN** its `status` is one of `robot_running`, `blocked`, `robot_finished`, `robot_failed`, `landed`,
  `sync_error`, `merge_conflict`, `analyzing_conflict`, `parked`, the main fact first
- **AND** it is omitted when only the push state is to say
  [@test] crates/bd-remote/src/landed_test.rs (`status_gives_each_branch_and_whole_stack_the_next_step_of_the_apps_row`)

### Requirement: `brd status`: The next step reads no sync, pull request or readiness

`brd status` reads the grouping without a sync, so it MUST name no failure of one: `brd sync` says it. `brd status`
MUST list no pull request and ask no readiness and no problem of a failed robot: a step that needs them waits for them
(row 0, `pull_requests`, `readiness`, `robot_problems`); `brd safe-push --dry-run` reads them.

#### Scenario: A step that needs a pull request
- **WHEN** the step of a branch needs its pull requests, its readiness or the problems of a failed robot
- **THEN** it waits for them (row 0, `pull_requests`, `readiness`, `robot_problems`)
- **AND** `brd safe-push --dry-run` reads them, and `brd sync` says a failure of the sync

### Requirement: `brd status`: The next step is omitted off Space and costs no git process

`next_step` MUST be omitted off Space (the row has no slot), with `--brief` and `--unassigned`, and MUST cost no git
process more.

#### Scenario: The cost of the next step
- **WHEN** `brd status` gives the next step of every branch
- **THEN** it runs no git process more than a status
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_status`)

#### Scenario: Off Space or a brief status
- **WHEN** the repository is off Space, or `--brief` or `--unassigned` is given
- **THEN** `next_step` is omitted

### Requirement: `brd status`: Each branch shows its origin and the issue groups are listed

Each branch MUST show its `origin` (`prefix`, `issue_group` or `issue`) and, for an issue group, its `issue_keys`;
the output MUST list `idle_issue_groups`, the keys of the issue groups that group no commit on this HEAD, and
`issue_group_problems`, the lines of the issue groups that are not used.

#### Scenario: Issue groups in the status
- **WHEN** `brd status` runs with issue groups
- **THEN** each branch has its `origin` and an issue group its `issue_keys`, and the output lists
  `idle_issue_groups` and `issue_group_problems`
  [@test] crates/bd-remote/src/status_test.rs (`issue_groups_in_status`)

### Requirement: `brd status`: Each branch shows its parent and what a sync builds it on

Each branch MUST show its declared `parent`, `parent_state` (`active`, `integrated`, `unapplied` or `missing`), the
`effective_base` or `blocked_by` a sync would use, `pr_base_changed` when a parent without commits is passed (the
branch is built on the nearest active ancestor or the baseline, never blocked by it), and its stack `problems` with
the commits that declare them.

#### Scenario: A stack in the status
- **WHEN** `brd status` runs with stacked branches
- **THEN** each branch has its `parent`, `parent_state`, `effective_base` or `blocked_by`, and its `problems`
- **AND** a parent without commits is passed with `pr_base_changed`
  [@test] crates/bd-remote/src/status_test.rs (`stacks_in_status`)

### Requirement: `brd status`: The output lists the forest, parents first

The output MUST list the `forest` order, parents first (omitted only without branches); `brd commits` MUST group by
the leaf as well.

#### Scenario: The forest
- **WHEN** `brd status` runs with branches
- **THEN** `forest` lists them parents first
  [@test] crates/bd-remote/src/status_test.rs (`stacks_in_status`)

### Requirement: `brd status`: The stack parents of the store are shown with their source

The `parent_source` of a parent MUST be `prefix` or `store` (the stack parents), and the output MUST list
`idle_stack_parents` and `stack_parent_problems` ([Stack Parents](../../branch/stack-parents/spec.md)).

#### Scenario: Stored parents in the status
- **WHEN** a parent comes from the stack parents
- **THEN** its `parent_source` is `store`, and the output lists `idle_stack_parents` and `stack_parent_problems`
  [@test] crates/bd-remote/src/status_test.rs (`stored_parents_in_status`)

### Requirement: `brd status`: `stacks` lists the stack branch of each forking stack

`stacks` MUST list the stack branch of each stack that forks (`root`, `branch` `{prefix}/stack/<root>`, `state`,
`tip`, `member` and `reason` of a block, `conflict`, `error`, `commits`, `members`) and the ones kept for their remote
branch, as the last sync left them, read without a sync and without a git process.

#### Scenario: A stack that forks
- **WHEN** `brd status` runs after a sync built the stack branch of a forking stack
- **THEN** `stacks` lists it as the sync left it, read without a git process
  [@test] crates/bd-remote/src/status_test.rs (`the_stack_branch_of_a_forking_stack_in_sync_and_status`)
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_status_of_a_forking_stack`)

### Requirement: `brd status`: The state of a stack branch comes from the object cache

The `state` of a stack branch MUST be `blocked` by a member's placement, else `built` when the object cache remembers
the build of the current commits at the ref's tip, `conflict` when it remembers their build conflicting, else
`stale`; no `remote` ([Whole Stack](../../branch/whole-stack/spec.md)).

#### Scenario: The states of a stack branch
- **WHEN** `stacks` lists a stack branch
- **THEN** its state is `blocked`, `built`, `conflict` or `stale` by what the object cache remembers, and it has no
  `remote`
  [@test] crates/bd-remote/src/status_test.rs (`the_stack_branch_of_a_forking_stack_in_sync_and_status`)

### Requirement: `brd status`: A branch with a description shows its title

Each branch with a `brd describe` text MUST show its title, read for all branches in one git process.

#### Scenario: A described branch
- **WHEN** a branch has a description
- **THEN** `brd status` shows its title, the titles of all branches read in one git process
  [@test] crates/bd-rewrite/src/describe_test.rs (`sets_prints_and_clears_a_description`)
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_status`)

### Requirement: `brd status`: `--stats` adds the stats of each branch

`--stats` MUST add per branch the distinct files, the generated files, added and deleted lines, the top three
directories, the authors, the author date range, the issue keys and a diffstat of the synced branch against the sync
base (`null` if it was never synced), for all branches in at most three more git processes.

#### Scenario: The stats of the branches
- **WHEN** `brd status --stats` runs
- **THEN** each branch has its stats, `null` for a branch never synced
- **AND** the files of each tip are listed against its base, for all branches in at most three more git processes
  [@test] crates/bd-remote/src/status_test.rs (`stats_per_branch`)
  [@test] crates/git-ops/src/commit_files_test.rs (`lists_the_files_of_each_tip_against_a_base`)

### Requirement: `brd status`: `--brief` prints the short status

`--brief` MUST print per branch only the name, the commit count, the issue key when it is not the name, and the newest
subject, the parent, `blocked_by`, `pr_base_changed`, the problems, the forest and the `stacks`, then the unassigned
commits; it MUST NOT go with `--branch` or `--stats`.

#### Scenario: A brief status
- **WHEN** `brd status --brief` runs
- **THEN** it prints the short fields of each branch, the forest and the `stacks` with their state, then the
  unassigned commits
  [@test] crates/bd-remote/src/status_test.rs (`brief_status`, `brief_status_keeps_the_stack_state`)

#### Scenario: Brief with a filter or stats
- **WHEN** `--brief` is given with `--branch` or `--stats`
- **THEN** the command is refused

### Requirement: `brd commits`: It prints every commit ahead of the baseline

It MUST print every commit ahead of the baseline, oldest first: id, subject, body, author, time, group (or `null`),
issue key, and for a fixup its kind and the target autosquash resolves.

#### Scenario: Entries, groups and fixups
- **WHEN** `brd commits` runs
- **THEN** it prints the commits ahead of the baseline oldest first with their group, issue key, and a fixup's kind
  and target
  [@test] crates/bd-rewrite/src/commits_test.rs (`entries_groups_and_fixups`)

### Requirement: `brd commits`: `--files` lists each changed file with its status and counts

`--files` MUST list each changed file with its status (`A`, `M`, `D`, `R` with the old path, `T`) and the counts of
the changed lines of the diff views; a binary file MUST have `"binary": true` and no counts.

#### Scenario: Renames, binaries and deletions
- **WHEN** `brd commits --files` lists a rename, a binary file and a deletion
- **THEN** each file has its status, a rename its old path, and a binary file `"binary": true` without counts
  [@test] crates/bd-rewrite/src/commits_test.rs (`files_with_renames_binaries_and_deletions`)

#### Scenario: The counts of the diff views
- **WHEN** the files of a commit are read
- **THEN** the counts are the changed lines of the blocks of the diff views, from the raw part of the diff
  [@test] crates/trial-check/src/diff_scan_test.rs (`parses_the_raw_part`, `read_commit_diffs_takes_the_blocks_of_the_views`)

### Requirement: `brd commits`: `--patch` adds the blocks of the diff views with their ids

`--patch` MUST add per changed file the blocks of the diff views under `Default`, each with its id for
`brd move-hunk --block` (the lines as in a git hunk header and a hash of them), its kind, a header with the label of
its scope, and its lines between up to three lines of context that stop at the next block; `--branch` MUST keep the
commits of one branch and the edges and overlaps that involve it.

#### Scenario: The blocks of a patch
- **WHEN** `brd commits --patch` runs
- **THEN** each block has its own context of up to three lines, stopped at the next block, a header naming its scope,
  and an id that applies to its commit
  [@test] crates/file-diffs/src/patch_test.rs (`each_block_has_its_own_context`, `an_insertion_and_a_final_line_end`, `the_header_names_the_scope`, `the_ids_of_a_commit_apply`)

#### Scenario: One branch
- **WHEN** `--branch` is given
- **THEN** the commits of that branch are kept with the edges and overlaps that involve it
  [@test] crates/bd-rewrite/src/commits_test.rs (`dependency_edges_and_cluster_overlap`)

### Requirement: `brd commits`: `dependencies` lists the edges by lines of the diff views

`dependencies` MUST list `{from, to, files}` where `from` touches lines `to` added, adjacent lines included, plus the
file-level rules for created, deleted, renamed and binary files; the lines are the blocks of the diff views, so the
user's `diff.algorithm`, `diff.interHunkContext` or `diff.indentHeuristic` changes no edge.

#### Scenario: Edges by lines
- **WHEN** a commit touches lines an earlier one added, overlapping, adjacent or moved by earlier insertions, in one
  hunk or several
- **THEN** `dependencies` has the edge, and created, renamed, deleted and binary files follow the file-level rules
  [@test] crates/trial-check/src/dependencies_test.rs (`overlapping_and_separate_hunks`, `lines_move_with_earlier_insertions`, `several_hunks_in_one_change`, `adjacent_changes_depend`, `created_renamed_deleted_and_binary_files`)

#### Scenario: The diff config of the user
- **WHEN** the user sets `diff.algorithm`, `diff.interHunkContext` or `diff.indentHeuristic`
- **THEN** neither the hunks nor the edges change
  [@test] crates/trial-check/src/diff_scan_test.rs (`the_diff_config_of_the_user_changes_neither_the_hunks_nor_the_edges`)

### Requirement: `brd commits`: `dependencies` lists the edges by symbols

Unless `branchdeck.symbols` is `off`, `dependencies` MUST also list the edges by symbols
([Symbol Dependencies](../../commit/symbol-dependencies/spec.md)): `via` `symbols` for an edge by symbols only (its
`files` the files of the uses) and `lines_and_symbols` for both, with `symbols` `{name, file}`; `via` MUST be absent
for an edge by lines only. `units` MUST NOT join by an edge by symbols only.

#### Scenario: An edge by symbols
- **WHEN** a commit uses a symbol another commit declares
- **THEN** the edge has `via` `symbols` and names the symbol, and it joins no unit
  [@test] crates/bd-rewrite/src/commits_test.rs (`an_edge_by_symbols_names_the_symbol_and_joins_no_unit`)

### Requirement: `brd commits`: `cluster_overlap` lists the files both branches change

`cluster_overlap` MUST list, for each pair of branches, the files both change.

#### Scenario: Two branches that change one file
- **WHEN** two branches change the same file
- **THEN** `cluster_overlap` lists it for that pair
  [@test] crates/bd-rewrite/src/commits_test.rs (`dependency_edges_and_cluster_overlap`)

### Requirement: `brd commits`: Commit ids and `--unassigned` filter the listing

`brd commits <id>…` MUST keep the commits with these ids (full, or abbreviated to at least 4 characters) and
`--unassigned` the unassigned commits, with the edges that involve them; an unknown id MUST be
`not_ahead_of_baseline`, an ambiguous one `invalid_arguments`, and the filters and `--branch` MUST exclude each other.

#### Scenario: Ids and unassigned commits
- **WHEN** `brd commits` names ids, or runs with `--unassigned`
- **THEN** it keeps those commits with the edges that involve them, and an unknown id is `not_ahead_of_baseline`
  [@test] crates/bd-rewrite/src/commits_test.rs (`commit_ids_and_unassigned_filter_the_listing`)

#### Scenario: An ambiguous id
- **WHEN** an abbreviated id matches several commits
- **THEN** it is `invalid_arguments`
  [@test] crates/bd-rewrite/src/commits_test.rs (`an_ambiguous_id_is_invalid`)

### Requirement: `brd commits`: `--brief` prints the short commits, their edges and units

`--brief` MUST print per commit only the id, the subject (without a prefix that is exactly the group; a stack path or
a sanitized prefix stays), the group, the issue key when it is not the group, the fixup kind and target, and the
areas; the dependencies as `{from, to, via}`; and `units`, the connected components of two or more commits of the
dependencies by lines and the fixups, over every commit, each oldest first.

#### Scenario: A brief view
- **WHEN** `brd commits --brief` runs
- **THEN** it prints the short commits with their areas, the dependencies as `{from, to, via}` and the `units`
  [@test] crates/bd-rewrite/src/commits_test.rs (`brief_view_with_units_and_areas`)

#### Scenario: A prefix that says more than the group
- **WHEN** the prefix of a subject is a stack path or a sanitized prefix
- **THEN** the subject keeps it, and only a prefix that is exactly the group is left out
  [@test] crates/bd-rewrite/src/commits_test.rs (`brief_keeps_a_prefix_that_says_more_than_the_group`)

### Requirement: `brd commits`: An area is the first directories of a path

An area MUST be the first `--depth` directories of a path (default 3; `.` at the top level); a commit MUST list at
most three areas by added plus deleted lines, without those under 5 % of its lines except the first, and without the
hub files of `HUB_FILES` (`crates/commit-clustering/src/hub_files.rs`). `--depth` MUST need `--brief`.

#### Scenario: The areas of a commit
- **WHEN** a commit changes files in several directories and hub files
- **THEN** each area keeps at most `--depth` directories, hub files match by name and are left out, and the top areas
  are by lines
  [@test] crates/commit-clustering/src/areas_test.rs (`an_area_keeps_at_most_depth_directories`, `hub_files_match_by_name`, `top_areas_by_lines_without_hub_files`)

### Requirement: Generated files: Generated files count in no edge, area or stats line

Lock files and the paths HEAD's attributes mark `linguist-generated` or `-diff` MUST NOT count in `dependencies`,
`cluster_overlap`, the areas and the stats lines; `--files` MUST still list them with `"generated": true`, and
`--include-generated` MUST count them.

#### Scenario: A generated file in a commit
- **WHEN** a commit changes a lock file or a path HEAD's attributes mark `linguist-generated` or `-diff`
- **THEN** it counts in no edge, overlap, area or stats line, and `--files` lists it with `"generated": true`
- **AND** with `--include-generated` it counts
  [@test] crates/bd-rewrite/src/commits_test.rs (`generated_files_are_flagged_and_unlinked`)
  [@test] crates/bd-remote/src/status_test.rs (`stats_per_branch`)

#### Scenario: The attributes of HEAD
- **WHEN** the attributes are read
- **THEN** they are those of the source tree
  [@test] crates/git-ops/src/attributes_test.rs (`reads_the_attributes_of_the_source_tree`)

### Requirement: Object cache: The changes of a commit are cached by its id

The per-commit changes MUST be cached by commit id in `$GIT_COMMON_DIR/branch-deck/cache/`, and git MUST run once for
the missing ids only. Cached and uncached outputs MUST be equal; `--no-cache` MUST neither read nor write it.

#### Scenario: A cached and an uncached run
- **WHEN** `brd commits` runs with the cache and with `--no-cache`
- **THEN** the outputs are equal
  [@test] crates/bd-rewrite/src/commits_test.rs (`cached_output_equals_uncached`)

### Requirement: Object cache: The cache is shared, cleared when full and a failure is a miss

Several processes MUST use the cache at once; a full cache MUST be cleared; the older versions of a store MUST be
dropped, and only of that store; any cache failure MUST be a miss.

#### Scenario: Threads and processes at once
- **WHEN** several threads or processes use the cache at once
- **THEN** each reads what was written
  [@test] crates/object-cache/src/cache_test.rs (`round_trip`, `parallel_threads`, `parallel_processes`)

#### Scenario: A new version of a store
- **WHEN** the version of a store is bumped
- **THEN** its older versions are dropped, and the other stores are kept
  [@test] crates/object-cache/src/cache_test.rs (`older_versions_of_a_store_are_dropped`, `a_bumped_store_keeps_the_other_stores`)

#### Scenario: A full or unusable cache
- **WHEN** the cache is full, or can't be used
- **THEN** a full cache is cleared, and an unusable one is a miss
  [@test] crates/object-cache/src/cache_test.rs (`unusable_cache_is_a_miss`, `full_cache_is_cleared`)

### Requirement: Object cache: An unchanged sync walks no branch

`brd sync` MUST walk no branch whose commits, remote branch and baseline did not move since a `brd` computed its
remote status: the status of the same commits comes from the cache (`remote_status`, read only by the status version
that wrote it), and the push times of all branches from one `reflog`.

#### Scenario: An unchanged sync of pushed branches
- **WHEN** `brd sync` runs in a new process and nothing moved
- **THEN** it walks no branch and reads the push times from one `reflog`
  [@test] crates/sync-core/src/process_budget_test.rs (`an_unchanged_sync_of_pushed_branches_in_a_new_process`)

#### Scenario: A remote status of another version
- **WHEN** the cache holds a remote status that another status version wrote
- **THEN** it is not read
  [@test] crates/object-cache/src/cache_test.rs (`remote_statuses_are_read_by_their_version`)

### Requirement: `brd group`, `rename`, `restack`, `unstack`, `stack-together`: `brd group` applies a plan in one rewrite

`brd group` MUST read a plan `{branches: {path: [ids]}, unassign: [ids], issue_groups: {name: [keys]}, ungroup_issues: [keys], unstack: [branches]}`
from `--plan` (`-` for stdin), set the prefixes of all its commits in one message-only rewrite, and change the issue
groups ([Issue Groups](../../branch/issue-groups/spec.md)) and the stack parents
([Stack Parents](../../branch/stack-parents/spec.md)) in the same ref transaction; it SHALL accept abbreviated ids.

#### Scenario: A plan of branches
- **WHEN** `brd group --plan` names the commits of each branch, by full or abbreviated ids
- **THEN** their prefixes are set in one message-only rewrite, and the issue groups and the stack parents change in
  the same ref transaction
  [@test] crates/bd-rewrite/src/rewrite_test.rs (`group_applies_a_plan`)
  [@test] crates/sync-core/src/regroup_test.rs (`applies_a_plan_in_one_rewrite`)

#### Scenario: An unassigned commit
- **WHEN** the plan unassigns a commit with an issue key
- **THEN** it falls back to the branch of its issue key
  [@test] crates/sync-core/src/regroup_test.rs (`unassign_falls_back_to_the_issue_key`)

### Requirement: `brd group`, `rename`, `restack`, `unstack`, `stack-together`: A plan of issue groups and stack parents rewrites no commit

A plan of issue groups and stack parents only MUST rewrite no commit.

#### Scenario: A plan without branches
- **WHEN** the plan only changes issue groups and stack parents
- **THEN** no commit is rewritten

### Requirement: `brd group`, `rename`, `restack`, `unstack`, `stack-together`: `brd group` without `--plan` is refused

`brd group` without `--plan` MUST be refused as `invalid_arguments` (exit 2), and stdin MUST NOT be read.

#### Scenario: No plan
- **WHEN** `brd group` runs without `--plan`
- **THEN** it is `invalid_arguments` (exit 2) and stdin is not read
  [@test] crates/bd-rewrite/src/rewrite_test.rs (`group_without_a_plan_is_refused`)

### Requirement: `brd group`, `rename`, `restack`, `unstack`, `stack-together`: A plan of the current grouping rewrites nothing

A commit without a prefix that the plan puts into the branch of its own issue key MUST keep its subject, so a plan that
names the current branch of each commit (its stack path) rewrites no commit without a prefix or with a canonical
prefix.

#### Scenario: The current grouping as a plan
- **WHEN** the plan names the current branch of each commit
- **THEN** no commit without a prefix or with a canonical prefix is rewritten
  [@test] crates/sync-core/src/regroup_test.rs (`the_current_grouping_as_a_plan_rewrites_nothing`)

### Requirement: `brd group`, `rename`, `restack`, `unstack`, `stack-together`: The plan is validated before anything is written

The plan MUST be validated against every subject ahead of the baseline before anything is written: unknown ids and ids
not ahead of the baseline are `not_ahead_of_baseline`; duplicate or contradicting ids, a malformed plan, invalid
names, a fixup separated from its target and a stack problem the plan introduces are `invalid_plan`, with the commits
named.

#### Scenario: A plan that can't be applied
- **WHEN** the plan has an unknown id, a duplicate or contradicting one, an invalid name or a malformed shape
- **THEN** it is refused with its code and exit code before anything is written, the commits named
  [@test] crates/sync-core/src/regroup_test.rs (`validates_the_plan_before_writing`)
  [@test] crates/bd-rewrite/src/rewrite_test.rs (`group_errors_and_exit_codes`)

#### Scenario: A stack problem the plan introduces
- **WHEN** the plan introduces a stack problem
- **THEN** it is `invalid_plan`
  [@test] crates/sync-core/src/regroup_test.rs (`refuses_problems_the_plan_introduces_and_keeps_existing_ones`)

### Requirement: `brd group`, `rename`, `restack`, `unstack`, `stack-together`: Problems that already existed are reported, not refused

Problems that already existed MUST be reported, not refused.

#### Scenario: An existing stack problem
- **WHEN** a stack problem existed before the plan
- **THEN** the plan is applied and the problem reported
  [@test] crates/sync-core/src/regroup_test.rs (`refuses_problems_the_plan_introduces_and_keeps_existing_ones`)

### Requirement: `brd group`, `rename`, `restack`, `unstack`, `stack-together`: Fixups follow their targets

Fixups not in the plan MUST follow their targets.

#### Scenario: A target that moves
- **WHEN** the plan moves the target of a fixup it does not name
- **THEN** the fixup moves with it
  [@test] crates/sync-core/src/regroup_test.rs (`fixups_follow_their_targets`)

### Requirement: `brd group`, `rename`, `restack`, `unstack`, `stack-together`: A rewrite that strands a fixup is `invalid_plan`

A rewrite whose fixup would fold into another target or none, or that recreates a target a fixup names by its id
(`fixup! <hash>`), MUST be `invalid_plan`: fold it first.

#### Scenario: A stranded fixup
- **WHEN** a rewrite would make a fixup fold into another target or none, or recreate a target named by its id
- **THEN** it is `invalid_plan`, saying to fold it first
  [@test] crates/sync-core/src/reword_test.rs (`a_reword_that_strands_a_fixup_is_refused`)

### Requirement: `brd group`, `rename`, `restack`, `unstack`, `stack-together`: The descriptions of a plan are validated with it

`descriptions` of the plan (`{branch: {title, body}}`, the leaf for a stacked branch) MUST be validated with it,
before anything is written: a title of one line that is not empty, and a branch that has commits after the plan, else
`invalid_plan`. `brd check` MUST validate them as well.

#### Scenario: A description that is not valid
- **WHEN** a description has an empty title or one of several lines, or names a branch without commits after the plan
- **THEN** `brd group`, its dry run and `brd check` are `invalid_plan`, and nothing is written
  [@test] crates/bd-rewrite/src/rewrite_test.rs (`descriptions_are_validated_with_the_plan`)

#### Scenario: A stacked branch
- **WHEN** the plan describes a stacked branch
- **THEN** the description is of its leaf
  [@test] crates/bd-rewrite/src/rewrite_test.rs (`a_stacked_branch_is_described_by_its_leaf`)

### Requirement: `brd group`, `rename`, `restack`, `unstack`, `stack-together`: The descriptions are written after the lock

brd MUST write the descriptions that differ after the ref transaction and the release of the repository lock, as
`brd describe` does, so the regroup holds the lock only for its witness and its write; it MUST list the written ones in
`described` and the texts they replaced in `replaced_descriptions`; a plan of descriptions only rewrites no commit.

#### Scenario: A plan with descriptions
- **WHEN** `brd group` applies a plan with descriptions
- **THEN** it writes those that differ after the lock, lists them in `described` and the old texts in
  `replaced_descriptions`
  [@test] crates/bd-rewrite/src/rewrite_test.rs (`group_writes_descriptions_and_syncs`)

#### Scenario: The span of the lock
- **WHEN** a group with descriptions runs
- **THEN** it holds the repository lock only for its witness and its write
  [@test] crates/bd-rewrite/src/rewrite_test.rs (`a_group_with_descriptions_locks_only_the_witness_and_the_write`)

#### Scenario: Descriptions only
- **WHEN** the plan only has descriptions
- **THEN** no commit is rewritten

### Requirement: `brd group`, `rename`, `restack`, `unstack`, `stack-together`: `--sync` syncs after the lock and adds its output

`--sync` MUST run the sync of `brd sync` after the writes and the release of the repository lock, so its merges never
make the app or another `brd` wait, and add its output as `sync`; a conflict MUST exit with 1 and keep the undo
token.

#### Scenario: A rewrite with a sync
- **WHEN** `brd group --sync` runs
- **THEN** the sync runs after the release of the lock, which it never takes again, and its output is `sync`
  [@test] crates/bd-rewrite/src/rewrite_test.rs (`group_writes_descriptions_and_syncs`, `the_sync_of_a_rewrite_runs_after_the_lock`)
  [@test] crates/sync-core/src/regroup_test.rs (`a_held_lock_is_not_taken_again`)

#### Scenario: A conflict of the sync
- **WHEN** the sync after the rewrite finds a conflict
- **THEN** the command exits with 1 and keeps the undo token
  [@test] crates/bd-rewrite/src/rewrite_test.rs (`a_sync_conflict_after_group_exits_with_1_and_keeps_the_undo_token`)

### Requirement: `brd group`, `rename`, `restack`, `unstack`, `stack-together`: A failed step after the rewrite keeps its output

A failed description write MUST skip the sync; it and a failed sync MUST keep the rewrite output, with the error as
`after_error` and its exit code.

#### Scenario: A failed description write
- **WHEN** the write of a description fails after the rewrite
- **THEN** the sync is skipped, and the output of the rewrite has the error as `after_error` with its exit code
  [@test] crates/bd-rewrite/src/rewrite_test.rs (`a_failed_description_write_is_the_error_of_the_output_and_keeps_the_undo_token`)

#### Scenario: A failed sync
- **WHEN** the sync after the rewrite fails
- **THEN** the output of the rewrite has the error as `after_error` with its exit code

### Requirement: `brd group`, `rename`, `restack`, `unstack`, `stack-together`: `--sync` does not go with `--dry-run`

`--sync` MUST NOT go with `--dry-run`.

#### Scenario: A dry run that would sync
- **WHEN** `--sync` is given with `--dry-run`
- **THEN** the command is `invalid_arguments`
  [@test] crates/bd-rewrite/src/rewrite_test.rs (`descriptions_are_validated_with_the_plan`)

### Requirement: `brd group`, `rename`, `restack`, `unstack`, `stack-together`: `--expect-head` is a compare-and-swap of HEAD

`--expect-head` MUST refuse with `head_moved` (exit 3, retryable) when HEAD is another commit, and the rewrite MUST
move the branch only from the HEAD the plan was computed for.

#### Scenario: Another HEAD than expected
- **WHEN** `--expect-head` names a commit and HEAD is another
- **THEN** the rewrite is `head_moved` (exit 3, retryable), and the branch moves only from the expected HEAD
  [@test] crates/sync-core/src/regroup_test.rs (`expect_head_is_a_compare_and_swap`)

### Requirement: `brd group`, `rename`, `restack`, `unstack`, `stack-together`: `--allow-append` accepts commits on top of the expected HEAD

With `--allow-append` (only with `--expect-head`) a HEAD with commits on top of the expected one MUST be accepted
while the expected one is still ahead of the baseline, and the output MUST list the commits on top as `appended` with
their new ids; any other HEAD stays `head_moved`.

#### Scenario: Commits on top of the expected HEAD
- **WHEN** commits were added on top of the expected HEAD and `--allow-append` is given
- **THEN** the rewrite is accepted and lists them as `appended` with their new ids
  [@test] crates/sync-core/src/regroup_test.rs (`allow_append_accepts_commits_on_top_and_refuses_a_rewritten_head`)
  [@test] crates/bd-rewrite/src/rewrite_test.rs (`group_with_allow_append_lists_the_appended_commits`)

#### Scenario: A rewritten HEAD
- **WHEN** HEAD was rewritten, not appended to
- **THEN** it stays `head_moved`
  [@test] crates/sync-core/src/regroup_test.rs (`allow_append_accepts_commits_on_top_and_refuses_a_rewritten_head`)

### Requirement: `brd group`, `rename`, `restack`, `unstack`, `stack-together`: The appended commits are regrouped like the others

The appended commits MUST be regrouped like the others: a rename, a restack or an unstack by name changes their prefix
too.

#### Scenario: A rename with appended commits
- **WHEN** a rename, a restack or an unstack by name accepts appended commits
- **THEN** their prefix changes too

### Requirement: `brd group`, `rename`, `restack`, `unstack`, `stack-together`: A rewrite waits a bounded time for the repository lock

Rewrites MUST hold `$GIT_COMMON_DIR/branch-deck.lock`, wait a bounded time for another holder, then fail with
`locked` (exit 3).

#### Scenario: Another holder of the lock
- **WHEN** another process holds the repository lock past the wait
- **THEN** the rewrite fails with `locked` (exit 3)
  [@test] crates/sync-core/src/regroup_test.rs (`waits_for_the_repository_lock`)

### Requirement: `brd group`, `rename`, `restack`, `unstack`, `stack-together`: A rewrite is refused during a rebase or on a detached HEAD

A rebase, merge, cherry-pick or revert in progress MUST be `operation_in_progress`, also for `--dry-run`, and a
detached HEAD `detached_head` (exit 4), but for `brd reword` in a lease (`brd reword` below).

#### Scenario: A rebase or a cherry-pick in progress
- **WHEN** a rebase, merge, cherry-pick or revert is in progress
- **THEN** the rewrite and its dry run are `operation_in_progress`
  [@test] crates/sync-core/src/regroup_test.rs (`refuses_while_a_rebase_is_in_progress`)
  [@test] crates/bd-rewrite/src/rewrite_test.rs (`group_and_its_dry_run_are_refused_during_a_cherry_pick`)

#### Scenario: A detached HEAD
- **WHEN** HEAD is detached, outside `brd reword` in a lease
- **THEN** the rewrite is `detached_head` (exit 4)

### Requirement: `brd group`, `rename`, `restack`, `unstack`, `stack-together`: A HEAD moved before the lock is computed once more under it

A rewrite without `--expect-head` (`brd group`, `brd reword`, `brd squash`, `brd fold`, `brd unapply`,
`brd move-hunk`) whose HEAD another process moves between two of its reads before the lock MUST compute once more
under the lock, on that HEAD, and keep the commits of the other process, never answer `head_moved`; with
`--expect-head` the moved HEAD MUST be `head_moved`.

#### Scenario: HEAD moved between two reads
- **WHEN** another process moves HEAD between two reads of a rewrite without `--expect-head`
- **THEN** the rewrite computes once more under the lock on that HEAD and keeps the commits of the other process
  [@test] crates/sync-core/src/repository_lock_test.rs (`a_head_moved_between_two_reads_of_a_rewrite_computes_it_once_more_under_the_lock`)
  [@test] crates/bd-rewrite/src/move_hunk_test.rs (`a_head_moved_between_two_reads_of_a_move_computes_it_once_more_under_the_lock`)
  [@test] crates/bd-rewrite/src/fold_test.rs (`a_head_moved_after_the_plan_of_the_fold_is_folded_as_its_core_finds_it`)

#### Scenario: The request named its HEAD
- **WHEN** HEAD moved and the request named its HEAD with `--expect-head`
- **THEN** it is `head_moved`, and without it the rewrite is computed once more under the lock
  [@test] crates/history-rewrite/src/repository_lock_test.rs (`a_head_moved_between_two_reads_before_the_lock_is_computed_once_more_under_it_unless_the_request_named_its_head`)

### Requirement: `brd group`, `rename`, `restack`, `unstack`, `stack-together`: The output lists the new groups, the new HEAD and an undo token

The output MUST list every commit's resulting group with its new id, the new HEAD and an undo token; `--map` MUST add
`{commit, new_id}` of every commit.

#### Scenario: The output of a rewrite
- **WHEN** a rewrite runs
- **THEN** it lists every commit's resulting group with its new id, the new HEAD and an undo token
- **AND** with `--map` the `{commit, new_id}` of every commit

### Requirement: `brd group`, `rename`, `restack`, `unstack`, `stack-together`: `--dry-run` writes nothing and prints the changes

`--dry-run` MUST write nothing and print the changed subjects (`changes`: the old and the new subject, `moved` with
the branch before and after when it changes) and the stack forest.

#### Scenario: A dry run of a rewrite
- **WHEN** a rewrite runs with `--dry-run`
- **THEN** nothing is written, and it prints `changes` and the stack forest
  [@test] crates/bd-rewrite/src/rewrite_test.rs (`dry_run_prints_the_changes_and_the_forest`)
  [@test] crates/sync-core/src/regroup_test.rs (`dry_run_writes_nothing`)

### Requirement: `brd group`, `rename`, `restack`, `unstack`, `stack-together`: `brd rename` renames a branch with its children

`brd rename` MUST replace the branch's level in every prefix path, children included, rename the issue groups and the
stack parents of that name without a rewrite, and name an issue branch by the issue groups; the new name MUST be a
branch name without `/`.

#### Scenario: A branch with its children
- **WHEN** `brd rename` renames a branch with children
- **THEN** its level changes in every prefix path, children included
  [@test] crates/sync-core/src/regroup_test.rs (`renames_a_branch_with_its_children`)

#### Scenario: A prefix branch and an issue branch
- **WHEN** it renames a prefix branch or an issue branch
- **THEN** a prefix branch is rewritten, and an issue branch is named by the issue groups without a rewrite
  [@test] crates/sync-core/src/regroup_test.rs (`renames_prefix_and_issue_branches`)

### Requirement: `brd group`, `rename`, `restack`, `unstack`, `stack-together`: `brd restack` and `brd unstack` set or clear the stack parent

`brd restack` and `brd unstack` MUST set or clear the stack parent of the branch, an issue branch and an issue group
included, and rewrite only the commits whose prefix declares another parent.

#### Scenario: A restack and an unstack
- **WHEN** `brd restack` or `brd unstack` runs on a branch
- **THEN** its stack parent is set or cleared, and only the commits whose prefix declares another parent are rewritten
  [@test] crates/sync-core/src/regroup_test.rs (`restacks_and_unstacks_a_branch`)
  [@test] crates/bd-rewrite/src/rewrite_test.rs (`rename_restack_unstack_and_undo`)

### Requirement: `brd group`, `rename`, `restack`, `unstack`, `stack-together`: `brd stack-together` chains the branches oldest first

`brd stack-together <branch> <branch>...` MUST put all selected branches into one oldest-first chain, preserve an
existing ancestor edge, move a selected branch out of its former stack while keeping its descendants, and write stack
parents without rewriting history when their prefixes declare no different parent.

#### Scenario: Branches stacked together
- **WHEN** `brd stack-together` names branches
- **THEN** they form one chain, the oldest at the bottom, written as stack parents without a rewrite
  [@test] crates/history-rewrite/src/regroup_test.rs (`stack_together_chains_branches_oldest_at_the_bottom_without_a_rewrite`)
  [@test] crates/bd-rewrite/src/rewrite_test.rs (`stack_together_stacks_branches_oldest_first_and_undoes_once`)

#### Scenario: A branch of another stack
- **WHEN** a selected branch has a parent and children in another stack
- **THEN** it moves out of its former stack while keeping its descendants, and an existing ancestor edge is preserved
  [@test] crates/sync-core/src/stack_operations_test.rs (`stack_together_moves_a_branch_and_keeps_its_existing_parent_and_children`)

### Requirement: `brd group`, `rename`, `restack`, `unstack`, `stack-together`: `brd stack-together` is one rewrite and refuses what it can't chain

`brd stack-together` MUST create one rewrite with one undo token, refuse too few branches, missing branches, a cycle
and branches already stacked together, and use no more git processes than `brd restack`.

#### Scenario: One rewrite with one undo
- **WHEN** `brd stack-together` runs
- **THEN** it is one rewrite of every named branch, undone at once by its one undo token
  [@test] crates/operations/src/rewrite_test.rs (`stack_together_is_one_rewrite_of_every_named_branch`)
  [@test] crates/bd-rewrite/src/rewrite_test.rs (`stack_together_stacks_branches_oldest_first_and_undoes_once`)

#### Scenario: Branches it can't chain
- **WHEN** it names too few branches, a missing one, branches that make a cycle or are already stacked together
- **THEN** it is refused
  [@test] crates/history-rewrite/src/regroup_test.rs (`stack_together_refuses_invalid_or_already_stacked_branches_and_cycles`)

#### Scenario: Its git processes
- **WHEN** `brd stack-together` runs
- **THEN** it uses no more git processes than `brd restack`
  [@test] crates/bd-cli/src/process_budget_test.rs (`stack_together_uses_the_same_number_of_git_processes_as_restack`)

### Requirement: `brd reword`: It replaces the message of a commit or of several by a plan

`brd reword <commit>` MUST replace the message of a commit ahead of the baseline with `--message`, `--message-file`
(`-` for stdin) or `--subject` (the subject line, its `(prefix)` included; the body stays), exactly one of them, or
the messages of several commits by `--plan` (`{rewrites: [{commit, message | subject}]}`, `-` for stdin), in one
message-only rewrite.

#### Scenario: A new subject
- **WHEN** `brd reword <commit> --subject` runs
- **THEN** only the subject of that commit changes, and its body stays
  [@test] crates/bd-rewrite/src/reword_test.rs (`reword_subject_changes_only_that_commit`)
  [@test] crates/sync-core/src/reword_test.rs (`a_subject_keeps_the_body_and_a_message_equal_after_cleanup_rewrites_nothing`)

#### Scenario: A message from stdin
- **WHEN** `--message-file -` passes a message on stdin
- **THEN** it replaces the whole message
  [@test] crates/bd-rewrite/src/reword_test.rs (`reword_with_a_message_file_from_stdin_replaces_the_whole_message`)

#### Scenario: Several messages in one rewrite
- **WHEN** `--plan` rewords several commits
- **THEN** their messages change in one message-only rewrite
  [@test] crates/sync-core/src/reword_test.rs (`rewords_messages_in_one_rewrite`)

### Requirement: `brd reword`: A reword changes no tree and keeps the older commits

In a reword every tree, the index and the working tree MUST stay, the older commits MUST keep their ids, the later ones
are recreated with their author and committer, and their mapping notes MUST follow them.

#### Scenario: The commits after the reworded one
- **WHEN** a commit is reworded
- **THEN** every tree, the index and the working tree stay, the older commits keep their ids
- **AND** the later ones are recreated with their author and committer, their mapping notes with them
  [@test] crates/sync-core/src/reword_test.rs (`rewords_messages_in_one_rewrite`)

### Requirement: `brd reword`: A message equal after the cleanup rewrites nothing

A message MUST be cleaned up as `git commit` does; one equal to the old message after the cleanup MUST rewrite nothing
and print no undo token.

#### Scenario: The same message after the cleanup
- **WHEN** the new message equals the old one after the cleanup of `git commit`
- **THEN** nothing is rewritten and no undo token is printed
  [@test] crates/bd-rewrite/src/reword_test.rs (`a_message_equal_after_cleanup_rewrites_nothing`)
  [@test] crates/sync-core/src/reword_test.rs (`a_subject_keeps_the_body_and_a_message_equal_after_cleanup_rewrites_nothing`)

### Requirement: `brd reword`: It is a rewrite of `brd group` with its flags and refusals

It MUST be a rewrite of `brd group`: under the repository lock (`locked`, exit 3), `--expect-head` (`head_moved`, exit
3), `--allow-append`, `--sync`, `--map`, `operation_in_progress` (exit 4) also for `--dry-run`.

#### Scenario: Another HEAD or the lock held
- **WHEN** HEAD is another than `--expect-head`, or another process holds the lock
- **THEN** the reword is `head_moved` or `locked`, exit 3
  [@test] crates/bd-rewrite/src/reword_test.rs (`reword_is_refused_on_another_head_or_while_the_lock_is_held`)

#### Scenario: A cherry-pick in progress
- **WHEN** a cherry-pick is in progress
- **THEN** the reword and its dry run are `operation_in_progress` (exit 4)
  [@test] crates/bd-rewrite/src/reword_test.rs (`reword_and_its_dry_run_are_refused_during_a_cherry_pick`)

### Requirement: `brd reword`: Its output has `changes` and an undo token for `brd undo`

Its output MUST be the output of `brd group` with `operation: reword` and `changes` (old id, `new_id`, old and new
subject, `body_changed`, `moved`) also without `--dry-run`, and an undo token `<previous head>..<new head>` that
`brd undo` takes back, the old commits with their notes.

#### Scenario: A plan and its undo
- **WHEN** a plan rewords two commits at once
- **THEN** the output has `operation: reword`, `changes` and an undo token
- **AND** `brd undo` of the token restores the old commits with their notes
  [@test] crates/bd-rewrite/src/reword_test.rs (`a_reword_plan_rewrites_two_commits_at_once_and_undo_restores_them`)

#### Scenario: A dry run
- **WHEN** `brd reword --dry-run` runs
- **THEN** it prints the `changes` and writes no ref
  [@test] crates/bd-rewrite/src/reword_test.rs (`reword_dry_run_writes_no_ref`)

### Requirement: `brd reword`: The issue groups and the stack parents stay

The issue groups and the stack parents MUST stay.

#### Scenario: A reword of a grouped commit
- **WHEN** a reword runs in a repository with issue groups and stack parents
- **THEN** they stay as they were
  [@test] crates/sync-core/src/reword_test.rs (`a_reword_keeps_the_issue_groups_and_the_stack_parents`)

### Requirement: `brd reword`: What it can't reword is refused with its code

A commit that is not ahead of the baseline (on the baseline, unknown) MUST be `not_ahead_of_baseline` (exit 2); a
revision that is no commit id (`HEAD~1`) or a commit without exactly one text `invalid_arguments`; a plan entry
without exactly one text, an empty plan, an empty subject, a subject of several lines and a commit named twice
`invalid_plan`.

#### Scenario: A commit not ahead of the baseline
- **WHEN** the commit is on the baseline or unknown
- **THEN** it is `not_ahead_of_baseline` (exit 2)
  [@test] crates/bd-rewrite/src/reword_test.rs (`reword_of_a_commit_not_ahead_of_the_baseline_is_refused`)

#### Scenario: Ids and texts it cannot reword
- **WHEN** a revision is no commit id, a commit has not exactly one text, or a plan is empty, names a commit twice or
  has an empty subject or one of several lines
- **THEN** it is `invalid_arguments` or `invalid_plan`
  [@test] crates/sync-core/src/reword_test.rs (`refuses_ids_and_texts_it_cannot_reword`)
  [@test] crates/bd-rewrite/src/reword_test.rs (`a_reword_plan_rewrites_two_commits_at_once_and_undo_restores_them`)

### Requirement: `brd reword`: In a lease the commits ahead of its home branch are reworded

In a lease of the pool, the commits ahead of its home branch (`refs/heads/<home>`) MUST take the place of the commits
ahead of the baseline: a commit that the home branch has (a pick landed it) is `not_ahead_of_baseline`.

#### Scenario: An unlanded commit of a lease
- **WHEN** a session rewords a commit of its lease that the home branch lacks
- **THEN** it is reworded
  [@test] crates/bd-lease/src/lease_test.rs (`a_session_rewords_its_unlanded_commit_in_the_detached_lease`)

#### Scenario: A commit the home branch has
- **WHEN** it names a commit that a pick landed on the home branch
- **THEN** it is `not_ahead_of_baseline`

### Requirement: `brd reword`: The reword moves the detached HEAD of the lease

The reword MUST move the detached HEAD of the lease itself, with compare-and-swap and `option no-deref`, and leave it
detached; its undo moves it back. Every other rewrite MUST stay `detached_head` there. So the agent fixes the message
of an unlanded commit before `brd lease pick`, in the lease.

#### Scenario: The detached HEAD of a lease
- **WHEN** a reword runs in a lease
- **THEN** it moves the detached HEAD with compare-and-swap and `option no-deref`, leaves it detached, and its undo
  moves it back
  [@test] crates/sync-core/src/reword_test.rs (`a_reword_in_a_lease_moves_its_detached_head`)
  [@test] crates/bd-lease/src/lease_test.rs (`a_session_rewords_its_unlanded_commit_in_the_detached_lease`)

#### Scenario: Its git processes in a lease
- **WHEN** a reword runs in a lease
- **THEN** its git processes are the pinned ones
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_reword_in_a_lease`)

#### Scenario: Another rewrite in a lease
- **WHEN** another rewrite runs on the detached HEAD of a lease
- **THEN** it is `detached_head`

### Requirement: `brd reword`: A new prefix moves the commit to that branch

A new `(prefix)` MUST move the commit to that branch as `brd group` does, its fixups with it, with the validation of a
plan: a stack problem it introduces is `invalid_plan`.

#### Scenario: A new prefix
- **WHEN** a reword gives a commit a new `(prefix)`
- **THEN** the commit moves to that branch with its fixups
  [@test] crates/bd-rewrite/src/reword_test.rs (`a_new_prefix_moves_the_commit_to_another_branch`)
  [@test] crates/sync-core/src/reword_test.rs (`a_new_prefix_moves_the_commit_with_its_fixups`)

#### Scenario: A prefix that breaks a stack
- **WHEN** the new prefix introduces a stack problem
- **THEN** it is `invalid_plan`
  [@test] crates/sync-core/src/reword_test.rs (`a_reword_that_breaks_a_stack_is_refused`)

### Requirement: `brd reword`: A reword that strands a fixup is `invalid_plan`

A fixup that would fold into another target or none, a `fixup!`, `squash!` or `amend!` marker added or removed, a
target a fixup names by its id, and a new text of a commit whose `amend!` waits MUST be `invalid_plan`; a plan that
rewords the fixup with its target SHALL pass.

#### Scenario: A stranded fixup
- **WHEN** a reword would strand a fixup, add or remove a marker, change a target named by its id, or a commit whose
  `amend!` waits
- **THEN** it is `invalid_plan`
  [@test] crates/bd-rewrite/src/reword_test.rs (`a_reword_that_strands_a_fixup_is_refused`)
  [@test] crates/sync-core/src/reword_test.rs (`a_reword_that_strands_a_fixup_is_refused`)

#### Scenario: A fixup reworded with its target
- **WHEN** a plan rewords the fixup with its target
- **THEN** it passes
  [@test] crates/sync-core/src/reword_test.rs (`a_plan_that_rewords_a_fixup_with_its_target_passes`)

### Requirement: `brd reword`: A reworded branch that was pushed needs a push

A reworded branch that was pushed MUST need a push: `brd safe-push --dry-run` with `push_needed: true` and
`remote.reworded`, `brd push --dry-run` a forced update.

#### Scenario: A pushed branch reworded
- **WHEN** `brd reword` rewords a branch that was pushed
- **THEN** `brd safe-push --dry-run` says `push_needed: true` with `remote.reworded`, and `brd push --dry-run` a
  forced update
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_branch_reworded_by_bd_reword_needs_a_push`)

### Requirement: `brd reword`: A session rewords its landed commit from its lease

A session MUST be able to reword its own landed commit from its lease with `-C <shared checkout>`; the lease needs no
move: a later pick lands only new commits and the return finds the old tip landed by content
([brd lease](../lease/spec.md)).

#### Scenario: A landed commit reworded from the lease
- **WHEN** a session rewords its landed commit with `-C <shared checkout>` from its lease
- **THEN** the commit is reworded in the shared checkout and the lease does not move
  [@test] crates/bd-lease/src/lease_test.rs (`a_session_rewords_its_landed_commit_from_its_lease`)

#### Scenario: A later pick
- **WHEN** the lease is picked later
- **THEN** the pick lands only new commits, finding the reworded commit present by content
  [@test] crates/bd-lease/src/lease_test.rs (`a_pick_without_follow_finds_the_reworded_commit_present_by_patch_id`)

### Requirement: `brd reword`: Its processes are pinned by subcommand

Its processes MUST be pinned by subcommand: the snapshot of a regroup, one `fast-import`, one `update-ref`.

#### Scenario: The git processes of a reword
- **WHEN** a reword runs
- **THEN** it runs the snapshot of a regroup, one `fast-import` and one `update-ref`
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_reword`)

### Requirement: `brd undo`: An undo token moves the branch and the stores back

`brd undo <undo_token>` MUST move the branch back to the token's previous HEAD with compare-and-swap. A token of the
issue groups (`groups:G1..G2`, or `H1..H2+groups:G1..G2` with a rewrite) MUST move them back in the same transaction,
the stack parents likewise with `+stacks:S1..S2` (alone `stacks:S1..S2`). The parts come in this order, each at most
once.

#### Scenario: The undo of a rewrite
- **WHEN** `brd undo` takes the token of a rewrite, a rename, a restack or an unstack
- **THEN** the branch moves back to the token's previous HEAD
  [@test] crates/sync-core/src/regroup_test.rs (`undo_restores_the_previous_head`)
  [@test] crates/bd-rewrite/src/rewrite_test.rs (`rename_restack_unstack_and_undo`)

#### Scenario: The undo of the issue groups and the stack parents
- **WHEN** the token has parts of the issue groups or of the stack parents, with a rewrite or alone
- **THEN** they move back with HEAD in one transaction
  [@test] crates/bd-rewrite/src/rewrite_test.rs (`issue_groups_plan_and_undo`, `stack_parents_plan_and_undo`)
  [@test] crates/sync-core/src/issue_groups_test.rs (`a_mixed_plan_moves_head_and_the_store_in_one_transaction`)

#### Scenario: A token out of order
- **WHEN** the parts of a token come in another order, or one comes twice
- **THEN** the token is not read
  [@test] crates/sync-core/src/issue_groups_test.rs (`an_undo_token_is_read_strictly`)

### Requirement: `brd undo`: A token whose branch or store changed again is `undo_stale`

Once the branch or a store changed again, the undo MUST be `undo_stale` (exit 4, not retryable): the token can never
succeed again, as for the tokens of `brd update` and `brd pick`.

#### Scenario: A branch that changed again
- **WHEN** the branch or a store changed again after the operation of the token
- **THEN** `brd undo` is `undo_stale` (exit 4, not retryable)

### Requirement: `brd update`: Its contract is the spec of brd update

`brd update`, `brd undo update:` and the resolve session of a conflicting update SHALL behave as [brd
update](../update/spec.md) requires.

#### Scenario: An update of the checked out branch
- **WHEN** an agent runs `brd update`, `brd undo update:` or resolves a conflicting update
- **THEN** it runs as that spec says

### Requirement: `brd pick`: Its contract is the spec of brd pick

`brd pick` (the commits of another worktree onto the checked out branch, `--follow`) and `brd undo pick:` SHALL behave
as [brd pick](../pick/spec.md) requires.

#### Scenario: A pick from another worktree
- **WHEN** an agent runs `brd pick` or `brd undo pick:`
- **THEN** it runs as that spec says

### Requirement: `brd need`: Its contract is the spec of Declared Needs

`brd need <branch> <needed>…` and `brd need <branch> --remove <needed>…` (what lands before a branch, in
`refs/branch-deck/needs`; `--allow-unlanded` of `brd safe-push` and `brd land` starts anyway on the user's word) SHALL
behave as [Declared Needs](../../branch/declared-needs/spec.md) requires.

#### Scenario: A declared need
- **WHEN** an agent runs `brd need` or its `--remove`
- **THEN** it runs as that spec says

### Requirement: `brd companion`: Its contract is the spec of companion repositories

`brd companion add <path> [--name]`, `brd companion remove <name>` and `brd companion list` (the repositories a need
names as `<companion>:<branch>`, written to both sides as Settings → Landing does), and `brd status --companions`, the
status of each, SHALL behave as [Declared Needs](../../branch/declared-needs/spec.md#companion-repositories) requires.

#### Scenario: A companion repository
- **WHEN** an agent adds, removes or lists a companion, or runs `brd status --companions`
- **THEN** it runs as that spec says

### Requirement: `brd unapply`, `brd reapply`: Their contract is the spec of Unapply and Reapply

`brd unapply` (a branch and its files out of the checked out branch), `brd reapply` (back on top) and
`brd undo unapply:`, `brd undo reapply:` SHALL behave as [Unapply and Reapply](../../branch/unapply/spec.md) requires.

#### Scenario: An unapply and a reapply
- **WHEN** an agent runs `brd unapply`, `brd reapply` or the undo of either
- **THEN** it runs as that spec says

### Requirement: `brd drop`: Its contract is the spec of Drop Commits

`brd drop` (commits and their files out of the checked out branch for good, a commit of a lease's home branch as
`drop_in_home`) and `brd undo drop:` SHALL behave as [Drop Commits](../../commit/drop/spec.md) requires.

#### Scenario: A drop
- **WHEN** an agent runs `brd drop` or `brd undo drop:`
- **THEN** it runs as that spec says

### Requirement: `brd lease`: Its contract is the spec of brd lease

`brd lease get`, `pick`, `return`, `status` and `adopt` (the pool of worktrees that agent sessions lease) SHALL behave
as [brd lease](../lease/spec.md) requires.

#### Scenario: A lease
- **WHEN** an agent runs a command of `brd lease`
- **THEN** it runs as that spec says

### Requirement: `brd fold`: It folds the fixups into their targets as Squash Fixups does

`brd fold` MUST fold the `fixup!`, `squash!` and `amend!` commits into their targets as the app's Squash Fixups does
([Squash Fixups](../../commit/fixups/spec.md)), under the repository lock, keeping HEAD's tree, the index and the
working tree, and print per target the fixups folded, the orphans, and an undo token `<previous head>..<new head>` for
`brd undo`.

#### Scenario: A fold and its undo
- **WHEN** `brd fold` runs
- **THEN** HEAD's tree stays, and the output names per target the fixups folded and the orphans
- **AND** `brd undo` of its token restores the head
  [@test] crates/bd-rewrite/src/fold_test.rs (`fold_keeps_the_tree_and_the_undo_restores_the_head`)

#### Scenario: A fixup with its marker after the prefix
- **WHEN** a fixup has its marker after a `(prefix)`
- **THEN** it is folded into its target
  [@test] crates/bd-rewrite/src/fold_test.rs (`fold_folds_a_fixup_with_its_marker_after_the_prefix`)

### Requirement: `brd fold`: `--branch` folds one branch and `--dry-run` writes nothing

`--branch` MUST fold only the fixups whose target is in that branch, and a name that is no branch of the commits ahead
of the baseline is `unknown_branch`; `--dry-run` MUST write nothing; nothing to fold MUST be no rewrite and no token.

#### Scenario: The fold of one branch and a dry run
- **WHEN** `brd fold --branch` or `brd fold --dry-run` runs
- **THEN** only the fixups whose target is in that branch are folded, and the dry run writes nothing
  [@test] crates/bd-rewrite/src/fold_test.rs (`fold_of_one_branch_and_a_dry_run`)

#### Scenario: An unknown branch or nothing to fold
- **WHEN** `--branch` names no branch of the commits ahead of the baseline, or there is nothing to fold
- **THEN** the first is `unknown_branch`, the second no rewrite and no token

### Requirement: `brd fold`: Without `--expect-head` it folds HEAD as its squash finds it

Without `--expect-head` the fold MUST fold the fixups of HEAD as its squash finds it, also when another process moved
HEAD after brd read its plan, and print what that squash folded: the targets with their fixups, the orphans, the head
it folded from and an undo token back to it; a HEAD without fixups of the scope there is nothing to fold.

#### Scenario: HEAD moved after the plan
- **WHEN** another process moves HEAD after brd read the plan of the fold
- **THEN** the fold folds HEAD as its core finds it, and prints the head it folded from with an undo token back to it
  [@test] crates/bd-rewrite/src/fold_test.rs (`a_head_moved_after_the_plan_of_the_fold_is_folded_as_its_core_finds_it`)

#### Scenario: The squash of the fold
- **WHEN** the squash folds interleaved groups, or the fixups of one branch or one fixup
- **THEN** it folds them in one rewrite
  [@test] crates/history-rewrite/src/autosquash_test.rs (`squashes_interleaved_groups_in_one_rewrite`, `squashes_only_the_fixups_of_one_branch_or_one_fixup`)

### Requirement: `brd fold`: `--expect-head` refuses another HEAD and a held lock is `locked`

`--expect-head` MUST refuse with `head_moved` on another HEAD, also one another process moved it to after the plan, and
the fold MUST check that HEAD, the planned commits and the history it rewrites all end at the planned head; a held lock
is `locked`.

#### Scenario: Another HEAD or the lock held
- **WHEN** HEAD is another than `--expect-head`, or another process holds the lock
- **THEN** the fold is `head_moved` or `locked`
  [@test] crates/bd-rewrite/src/fold_test.rs (`fold_is_refused_on_another_head_or_while_the_lock_is_held`, `a_head_moved_after_the_plan_of_the_fold_is_folded_as_its_core_finds_it`)

#### Scenario: The branch moved since the plan
- **WHEN** the history the fold rewrites no longer ends at the planned head
- **THEN** it is refused
  [@test] crates/history-rewrite/src/autosquash_test.rs (`refuses_when_the_branch_moved_since_the_plan`)

### Requirement: `brd fold`: An operation in progress in this worktree refuses it

A rebase, merge, cherry-pick or revert in progress in this worktree MUST be `operation_in_progress` (exit 4) for the
fold and its dry run, moving nothing; one in another worktree refuses nothing, since git keeps that state per worktree.

#### Scenario: A cherry-pick in progress here
- **WHEN** a cherry-pick is in progress in this worktree
- **THEN** the fold and its dry run are `operation_in_progress` (exit 4), moving nothing
  [@test] crates/bd-rewrite/src/fold_test.rs (`fold_and_its_dry_run_are_refused_during_a_cherry_pick`)

#### Scenario: A cherry-pick in another worktree
- **WHEN** a cherry-pick is in progress in another worktree
- **THEN** the fold is not refused
  [@test] crates/bd-rewrite/src/fold_test.rs (`a_cherry_pick_in_another_worktree_does_not_refuse_the_fold`)

### Requirement: `brd fold`: A fixup that conflicts or empties a commit folds nothing

A fixup that conflicts with a commit between it and its target or after it MUST be `squash_conflict` (exit 4) with the
commits and the files the merge names, one that leaves a later commit empty `squash_empty` (exit 4); both fold
nothing.

#### Scenario: A fixup that conflicts
- **WHEN** a fixup, with a prefix or without, conflicts with a commit between it and its target or after it
- **THEN** the fold is `squash_conflict` (exit 4) with the commits and the files, and folds nothing
  [@test] crates/bd-rewrite/src/fold_test.rs (`a_fold_whose_fixup_conflicts_is_refused_as_a_squash_conflict`)
  [@test] crates/bd-rewrite/src/fold_test.rs (`a_prefixed_fixup_that_conflicts_is_refused_as_a_squash_conflict`)

#### Scenario: A later commit left empty
- **WHEN** a fixup leaves a later commit empty
- **THEN** the fold is `squash_empty` (exit 4) and folds nothing

### Requirement: `brd fold`: Fixups that cancel their target drop it

Fixups that cancel their target MUST take it out of the history with them, HEAD's tree kept: its entry of `folded`
says `"dropped": true` and has no `new_id`. `--dry-run` MUST build the fold and drop it, with no ref, note or
temporary ref left, and say `dropped` as the fold would.

#### Scenario: A fixup that cancels its target
- **WHEN** fixups cancel their target
- **THEN** the fold drops the target with them, HEAD's tree kept, its entry of `folded` `"dropped": true` without
  `new_id`
- **AND** the dry run says `dropped` too, leaving no ref, note or temporary ref
  [@test] crates/bd-rewrite/src/fold_test.rs (`a_fixup_that_cancels_its_target_drops_it_and_the_dry_run_says_so`)

### Requirement: `brd squash`: It squashes the named commits into the oldest as Squash does

`brd squash <into> <commit>...` MUST squash the commits the user named into `<into>`, the oldest of them, as the app's
Squash does ([Squash Commits](../../commit/squash/spec.md)): one core (`squash_commits_core`) and one intent
(`RewriteIntent::Squash`), the commits in between kept, HEAD's tree, the index and the working tree kept.

#### Scenario: A squash of named commits
- **WHEN** `brd squash <into> <commit>...` runs
- **THEN** the commits are folded into the oldest, the commits in between kept, HEAD's tree kept
  [@test] crates/bd-rewrite/src/squash_test.rs (`squash_folds_the_commits_into_the_oldest_keeps_the_message_and_the_copies_after_them`)

#### Scenario: The intent of the squash
- **WHEN** the squash runs
- **THEN** it is journaled as the squash of its commits
  [@test] crates/bd-cli/src/operation_test.rs (`a_squash_is_journaled_as_the_squash_of_its_commits`)

### Requirement: `brd squash`: It keeps the message of `<into>` unless given one

It MUST keep the message of `<into>`; `--subject` MUST replace its subject and keep its body, `--message` and
`--message-file` (`-` for stdin) MUST be the whole message.

#### Scenario: The message of the squash
- **WHEN** no message is given
- **THEN** the squashed commit keeps the message of `<into>`
  [@test] crates/bd-rewrite/src/squash_test.rs (`squash_folds_the_commits_into_the_oldest_keeps_the_message_and_the_copies_after_them`)

#### Scenario: A message of the user
- **WHEN** `--subject`, `--message` or `--message-file` is given
- **THEN** `--subject` replaces the subject and keeps the body, the others are the whole message
  [@test] crates/bd-rewrite/src/squash_test.rs (`squash_dry_run_builds_and_drops_the_squash_and_a_message_is_the_users`)

### Requirement: `brd squash`: It prints the squash and an undo token

It MUST print `into`, `new_id`, `squashed` (oldest first), `subject`, `branch`, `rebuilt_branches` (the branch of the
squashed commit and those of the commits between it and the newest squashed one), and an undo token
`<previous head>..<new head>` for `brd undo`; `--sync` MUST sync after the record ended.

#### Scenario: The output of a squash
- **WHEN** a squash runs
- **THEN** it prints `into`, `new_id`, `squashed`, `subject`, `branch`, `rebuilt_branches` and an undo token
  [@test] crates/bd-rewrite/src/squash_test.rs (`squash_folds_the_commits_into_the_oldest_keeps_the_message_and_the_copies_after_them`)

#### Scenario: A squash with a sync
- **WHEN** `brd squash --sync` runs
- **THEN** the sync runs after the record of the squash ended

### Requirement: `brd squash`: The commits after the squash keep their mapping notes

The commits after the newest squashed one MUST keep their mapping notes, so a branch whose commits all come after it
keeps its virtual branch as it was after the next sync.

#### Scenario: A branch after the squash
- **WHEN** a branch has all its commits after the newest squashed one
- **THEN** its virtual branch is as it was after the next sync
  [@test] crates/bd-rewrite/src/squash_test.rs (`squash_folds_the_commits_into_the_oldest_keeps_the_message_and_the_copies_after_them`)

### Requirement: `brd squash`: `--dry-run` builds the squash and drops it

`--dry-run` MUST build the squash and drop it, refused as the squash would be, with no ref, note or temporary ref
written and no record.

#### Scenario: A dry run of a squash
- **WHEN** `brd squash --dry-run` runs
- **THEN** it builds the squash and drops it, writing no ref, note or temporary ref
  [@test] crates/bd-rewrite/src/squash_test.rs (`squash_dry_run_builds_and_drops_the_squash_and_a_message_is_the_users`)

#### Scenario: No record of a dry run
- **WHEN** the dry run ended
- **THEN** it left no record of an operation
  [@test] crates/bd-cli/src/operation_test.rs (`a_squash_is_journaled_as_the_squash_of_its_commits`)

### Requirement: `brd squash`: What is no squash of one branch is `invalid_arguments`

It MUST refuse, with nothing changed: a commit not ahead of the baseline (`not_ahead_of_baseline`), an ambiguous id,
fewer than two commits, an `<into>` that is not the oldest, a commit of another branch than `<into>`'s, a message that
moves the squashed commit to another branch (`invalid_arguments`).

#### Scenario: Commits that are no squash of one branch
- **WHEN** the commits are not ahead of the baseline, ambiguous, fewer than two, of another branch, `<into>` is not the
  oldest, or the message moves the squashed commit to another branch
- **THEN** the squash is refused with nothing changed
  [@test] crates/bd-rewrite/src/squash_test.rs (`squash_refuses_what_is_no_squash_of_one_branch`)

### Requirement: `brd squash`: A conflict or an empty result refuses the squash

It MUST refuse, with nothing changed: a conflict with a commit in between or after (`squash_conflict`, with `commits`
and `paths`), a result without changes or a later commit left empty (`squash_empty`).

#### Scenario: A conflict or an empty later commit
- **WHEN** the squash conflicts with a commit in between or after, or leaves a later commit empty
- **THEN** it is `squash_conflict` with `commits` and `paths`, or `squash_empty`, nothing changed
  [@test] crates/bd-rewrite/src/squash_test.rs (`squash_is_refused_by_a_conflict_or_an_empty_result_another_head_or_the_lock`)

#### Scenario: A result without changes
- **WHEN** the squashed commit would have no changes
- **THEN** it is `squash_empty`, nothing changed
  [@test] crates/bd-rewrite/src/squash_test.rs (`squash_refuses_a_result_without_changes`)

### Requirement: `brd squash`: Another HEAD, the lock or an operation in progress refuse it

It MUST refuse, with nothing changed: another HEAD than `--expect-head` (`head_moved`), a held lock (`locked`), and a
rebase, merge, cherry-pick or revert in progress (`operation_in_progress`), also for its dry run.

#### Scenario: Another HEAD or the lock held
- **WHEN** HEAD is another than `--expect-head`, or another process holds the lock
- **THEN** it is `head_moved` or `locked`, nothing changed
  [@test] crates/bd-rewrite/src/squash_test.rs (`squash_is_refused_by_a_conflict_or_an_empty_result_another_head_or_the_lock`)

#### Scenario: A cherry-pick in progress
- **WHEN** a cherry-pick is in progress
- **THEN** the squash and its dry run are `operation_in_progress`
  [@test] crates/bd-rewrite/src/squash_test.rs (`squash_and_its_dry_run_are_refused_during_a_cherry_pick`)

### Requirement: `brd move-hunk`: It moves the change of a file into an older or newer commit

`brd move-hunk <from> <to> <path>` MUST move the changes of the file `path` in commit `from` (all of them, or with
`--block <id>`, repeatable, the blocks with the ids of `brd commits <from> --patch`) into commit `to`, older or newer,
both ahead of the baseline on the checked out branch, in one rewrite under the repository lock.

#### Scenario: One block of three
- **WHEN** `--block` names one of the three blocks of a file, in a file of CRLF or lone CR line ends too
- **THEN** only that block moves
  [@test] crates/git-ops/src/move_hunk_test.rs (`moves_one_block_of_three`, `moves_a_block_of_a_crlf_file_and_of_a_lone_cr_file`)

#### Scenario: A fix moved into the commit it belongs to
- **WHEN** an agent moves a fix into the commit it belongs to
- **THEN** it moves in one rewrite, and the sync rebuilds the branch
  [@test] crates/bd-rewrite/src/move_hunk_test.rs (`the_fix_moves_into_the_commit_it_belongs_to_and_the_sync_rebuilds_the_branch`)

### Requirement: `brd move-hunk`: HEAD's tree, the index and the working tree stay

HEAD's tree MUST stay, and so the index and the working tree: into an older commit the change is added to the trees of
`[to, from)` and `from` keeps its tree; into a newer one it is taken out of the trees of `[from, to)` and `to` keeps
its tree. The commits above the newer one MUST get new parents only.

#### Scenario: Into an older commit
- **WHEN** the change moves into an older commit
- **THEN** it is added to the trees of `[to, from)`, and `from` keeps its tree
  [@test] crates/git-ops/src/move_hunk_test.rs (`moves_a_file_change_into_an_older_commit`)

#### Scenario: Into a newer commit
- **WHEN** the change moves into a newer commit
- **THEN** it is taken out of the trees of `[from, to)`, and `to` keeps its tree
  [@test] crates/git-ops/src/move_hunk_test.rs (`moves_a_file_change_into_a_newer_commit`)

#### Scenario: Local changes
- **GIVEN** dirty and staged changes
- **WHEN** a change moves
- **THEN** the index and the working tree stay byte for byte
  [@test] crates/git-ops/src/move_hunk_test.rs (`keeps_dirty_and_staged_changes_byte_for_byte`)

### Requirement: `brd move-hunk`: A block applies by its line ranges or is `hunk_conflict`

A block MUST apply by its line ranges to the file as each commit between has it, through one
`merge-tree --write-tree --stdin`: a commit that shifts the lines still takes it; one that changes the same lines, or
lacks the file, is `hunk_conflict` (exit 4) with `commits` and `paths`, and nothing is written but git objects.

#### Scenario: A commit between that shifts the lines
- **WHEN** a commit between shifts the lines of the block
- **THEN** the block still applies
  [@test] crates/git-ops/src/move_hunk_test.rs (`a_commit_between_that_shifts_the_lines_still_applies`)

#### Scenario: A commit between that changes the same lines
- **WHEN** a commit between changes the same lines, or lacks the file
- **THEN** it is `hunk_conflict` (exit 4) with `commits` and `paths`, and nothing is written but git objects
  [@test] crates/git-ops/src/move_hunk_test.rs (`a_commit_between_that_changes_the_same_lines_conflicts`)
  [@test] crates/git-ops/src/merge_tree_output.rs (`parses_the_merges_of_a_stdin_run`)

### Requirement: `brd move-hunk`: A conflict is never resolved by words

Such a conflict MUST never be resolved by words: the move MUST NOT change the user's commits in a way git does not.

#### Scenario: A conflict of a move
- **WHEN** a move conflicts
- **THEN** it is refused, and no commit is changed by a resolution git did not make

### Requirement: `brd move-hunk`: Only the changed commits get a new committer

`from`, `to` and the commits between whose diff of the file changed (`patch-id --stable`) MUST keep their author and
get the current user as committer and no mapping note (the sync copies them again); every other commit MUST keep
author and committer byte for byte, and its mapping note: with the new trees as its source for a commit between, as it
was above. So the sync rebuilds the branches of `from` and `to` and keeps the others.

#### Scenario: The commits of a move
- **WHEN** a change moves past commits between
- **THEN** the commits whose diff of the file changed get the current user as committer and no mapping note
- **AND** the others keep author, committer and their mapping note
  [@test] crates/git-ops/src/move_hunk_test.rs (`a_commit_between_that_shifts_the_lines_still_applies`, `moves_a_file_change_into_an_older_commit`)

### Requirement: `brd move-hunk`: It prints the moved blocks, the commits and an undo token

It MUST print the moved blocks (`moved.blocks`, as `brd commits --patch` prints them), `commits` (`{commit, new_id}`
of every recreated commit), `changed` (the commits whose changes differ) and an undo token
`<previous head>..<new head>` for `brd undo`.

#### Scenario: The output of a move
- **WHEN** a move of blocks named by their ids runs
- **THEN** `moved.blocks` has them as `brd commits --patch` prints them, with `commits`, `changed` and an undo token
  [@test] crates/bd-rewrite/src/move_hunk_test.rs (`the_block_id_is_the_one_of_bd_commits_patch`)

### Requirement: `brd move-hunk`: `--sync` syncs after the lock and `--dry-run` writes no ref

`--sync` MUST sync after the release of the lock (`sync`, or `after_error` with the token kept); `--dry-run` MUST print
the blocks and check the rewrite, writing nothing but git objects.

#### Scenario: A dry run
- **WHEN** `brd move-hunk --dry-run` runs
- **THEN** it prints the blocks and writes no ref and no note
  [@test] crates/git-ops/src/move_hunk_test.rs (`a_dry_run_writes_no_ref_and_no_note`)

#### Scenario: A move with a sync
- **WHEN** `brd move-hunk --sync` runs
- **THEN** the sync runs after the release of the lock, its output `sync`, or `after_error` with the token kept

### Requirement: `brd move-hunk`: What is no move of a file change is `invalid_arguments`

It MUST refuse with `invalid_arguments` (exit 2) the same commit twice, a file the commit doesn't change, a file it
renames, a mode change, a symlink or submodule, `--block` of a file without blocks, an id that does not parse, and a
move that leaves a commit without changes ("squash it instead").

#### Scenario: A move that names no change or leaves a commit empty
- **WHEN** the move names no change of the file, or leaves a commit without changes
- **THEN** it is `invalid_arguments` (exit 2), "squash it instead" for the empty commit
  [@test] crates/git-ops/src/move_hunk_test.rs (`refuses_a_move_that_leaves_a_commit_empty_or_names_no_change`)
  [@test] crates/bd-rewrite/src/move_hunk_test.rs (`a_fix_that_is_the_whole_commit_is_refused_as_a_squash`)

### Requirement: `brd move-hunk`: An unknown block, commit or HEAD and the lock refuse it

An id whose lines or hash match no block of the file now MUST be `block_not_found` (exit 2) with `blocks`, the ids of
now; an id that is no commit ahead of the baseline `not_ahead_of_baseline`; `--expect-head` `head_moved` on another
HEAD, a held lock `locked`.

#### Scenario: A block that is not there now
- **WHEN** a block id matches no block of the file now
- **THEN** it is `block_not_found` (exit 2) with `blocks`
  [@test] crates/bd-rewrite/src/move_hunk_test.rs (`the_block_id_is_the_one_of_bd_commits_patch`)

#### Scenario: Another HEAD or the lock held
- **WHEN** HEAD is another than `--expect-head`, or another process holds the lock
- **THEN** it is `head_moved` or `locked`
  [@test] crates/bd-rewrite/src/move_hunk_test.rs (`a_commit_between_on_the_same_lines_conflicts_and_another_head_or_the_lock_refuse`)

#### Scenario: A commit not ahead of the baseline
- **WHEN** an id is no commit ahead of the baseline
- **THEN** it is `not_ahead_of_baseline`

### Requirement: `brd move-hunk`: It runs a fixed number of git processes, most before the lock

It MUST run a fixed number of git processes, whatever the number of commits between, and compute every one of them but
the witness of HEAD and the move of the branch before the repository lock.

#### Scenario: The git processes of a move
- **WHEN** a move runs past any number of commits between
- **THEN** it runs a fixed number of git processes
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_move_hunk`)

#### Scenario: The span of the lock
- **WHEN** a move runs
- **THEN** it holds the repository lock only for the witness of HEAD and the move of the branch
  [@test] crates/bd-rewrite/src/move_hunk_test.rs (`a_move_hunk_locks_only_the_witness_and_the_write`)

### Requirement: `brd move-hunk`: A HEAD moved between its reads makes it compute once more

HEAD that another process moves between the commits it lists and the history it reads MUST make it compute once more
under the lock.

#### Scenario: HEAD moved between two reads
- **WHEN** another process moves HEAD between the commits the move lists and the history it reads
- **THEN** the move computes once more under the lock
  [@test] crates/bd-rewrite/src/move_hunk_test.rs (`a_head_moved_between_two_reads_of_a_move_computes_it_once_more_under_the_lock`)

### Requirement: `brd check`: A plan is applied in memory and built, moving no ref

`brd check` MUST apply a plan (`--plan`, `-` for stdin; the current grouping without one) in memory, validate it like a
regroup (exit 2 when invalid, `descriptions` included), place the branches as a stacked sync does and build each
one, parents first, writing git objects and the cache only: refs, notes, config and HEAD MUST NOT change.

#### Scenario: A commit moved away from its dependency
- **WHEN** the plan moves a commit away from the commit it depends on
- **THEN** its branch conflicts in the build
  [@test] crates/trial-check/src/check_test.rs (`a_commit_moved_away_from_its_dependency_conflicts`)

#### Scenario: Stacking resolves the conflict
- **WHEN** the plan stacks the branch on the branch it depends on
- **THEN** the branches are placed as a stacked sync does and build, parents first
  [@test] crates/trial-check/src/check_test.rs (`stacking_resolves_the_conflict`)

#### Scenario: The current grouping and a clean plan
- **WHEN** `brd check` runs without a plan, or with a plan that applies cleanly
- **THEN** each branch builds
- **AND** refs, notes, config and HEAD do not change
  [@test] crates/trial-check/src/check_test.rs (`the_current_grouping_and_a_clean_plan_build`)
  [@test] crates/bd-rewrite/src/check_test.rs (`the_current_grouping_and_invalid_plans`)

#### Scenario: An invalid plan
- **WHEN** the plan is invalid as a regroup would find it, `descriptions` included
- **THEN** it is refused with exit 2
  [@test] crates/trial-check/src/check_test.rs (`an_invalid_plan_is_refused`)
  [@test] crates/bd-rewrite/src/check_test.rs (`the_current_grouping_and_invalid_plans`)

### Requirement: `brd check`: Without `--plan` stdin is never read

Without `--plan`, `brd check` MUST check the current grouping and never read stdin, so the stdin of an agent, a CI
or an IDE, an open pipe that sends nothing, does not hold it. `--plan -` MUST read the plan from stdin, with the
result of `--plan <file>`.

#### Scenario: An open stdin without a plan
- **GIVEN** an open pipe on stdin that sends nothing
- **WHEN** `brd check` runs without `--plan`
- **THEN** it checks the current grouping and does not wait for stdin
  [@test] crates/bd-rewrite/src/check_test.rs (`without_a_plan_an_open_stdin_is_not_read`)

#### Scenario: A plan on stdin
- **WHEN** `brd check --plan -` reads a plan from stdin
- **THEN** the result is that of `--plan <file>` with the same plan
  [@test] crates/bd-rewrite/src/check_test.rs (`a_plan_on_stdin_with_plan_dash_is_the_plan_of_a_file`)

### Requirement: `brd check`: The branches build at the same time as a sync builds them

The branches MUST build at the same time as a sync builds them: a branch as soon as the one it is built on has its
head, the others at once, with the outcomes of a build one by one (a child on its parent's head, blocked by a failed
parent) and no more git processes.

#### Scenario: Independent branches
- **WHEN** the branches of the plan are not built on each other
- **THEN** they build at once
  [@test] crates/trial-check/src/check_test.rs (`independent_branches_build_at_the_same_time`)

#### Scenario: A child and its parent
- **WHEN** a branch is built on another
- **THEN** it builds as soon as its parent has its head, on that head
- **AND** a failed parent blocks it
  [@test] crates/trial-check/src/check_test.rs (`a_branch_waits_for_its_parent_and_is_blocked_by_its_failure`)

#### Scenario: No more git processes than one by one
- **WHEN** a check builds the branches, synced or not
- **THEN** it runs no more git processes than a build one by one
  [@test] crates/trial-check/src/process_budget_test.rs (`a_check`, `a_check_of_synced_branches`)

### Requirement: `brd check`: Each branch prints its result and the branch it is built on

Per branch it MUST print `ok`, a light conflict (commit, subject, files and the older commits outside the branch
and its ancestors that change those files), `blocked_by`, `problems` or `error`, and the branch it would be built
on; the exit code MUST be 1 unless every branch is `ok`.

#### Scenario: A conflicting plan
- **WHEN** a branch of the plan conflicts
- **THEN** its light conflict names the commit, its subject, the files and the older commits outside the branch and its ancestors that change those files
- **AND** the exit code is 1
  [@test] crates/bd-rewrite/src/check_test.rs (`a_conflicting_plan_names_the_commit_and_what_it_needs`, `stacking_resolves_the_conflict`)

#### Scenario: The children of a conflicting branch
- **WHEN** a branch conflicts
- **THEN** the branches built on it are `blocked_by` it
  [@test] crates/trial-check/src/check_test.rs (`children_of_a_conflicting_branch_are_blocked`)

### Requirement: `brd check`: `--fix` adds the regroup after which the branches apply

With `--fix` it MUST add `fix`, the regroup after which the conflicting branches apply cleanly as the trial builds of
[Conflict Fix](../../branch/conflict-fix/spec.md) find it, each component's `plan` the whole plan for `brd group`;
nothing MUST be applied. Each trial MUST be a line on stderr.

#### Scenario: A fix is found
- **WHEN** `brd check --fix` runs on a plan whose branches conflict
- **THEN** `fix` names the regroup after which they apply cleanly, each component's `plan` the whole plan for `brd group`
- **AND** nothing is applied
  [@test] crates/bd-rewrite/src/check_test.rs (`fix_finds_the_regroup_after_which_the_plan_applies_and_writes_nothing`)

### Requirement: `brd check`: `--max-trials` and `--time-budget` bound the search of `--fix`

`--max-trials N` (12) and `--time-budget SECONDS` (180, the whole command) MUST bound the search, each only with
`--fix`; a search they stop MUST say `search: budget` with the best plan so far.

#### Scenario: The search runs out of time
- **WHEN** the time budget stops the search
- **THEN** it says `search: budget` with the best plan so far
  [@test] crates/bd-rewrite/src/check_test.rs (`a_fix_out_of_time_says_budget_and_the_trial_limit_needs_fix`)

#### Scenario: A bound without `--fix`
- **WHEN** `--max-trials` or `--time-budget` is given without `--fix`
- **THEN** it is refused
  [@test] crates/bd-rewrite/src/check_test.rs (`a_fix_out_of_time_says_budget_and_the_trial_limit_needs_fix`)

### Requirement: `brd check`: A use of a name another branch adds is a missing declaration

With `branchdeck.symbols on`, `missing_declarations` MUST name each use, by a commit of a built branch, of a name
that a commit outside the branch and the branches it is built on adds or gives the form the use needs (an edge by
symbols), an override of it included, with the evidence. git merges such a branch and its build fails, so a branch
that builds otherwise MUST be `missing_declaration` (exit 1).

#### Scenario: A use of a name another branch adds
- **WHEN** a commit of a built branch uses a name that a commit outside it and its ancestors adds
- **THEN** `missing_declarations` names the use with the evidence
- **AND** the branch is `missing_declaration`, exit 1
  [@test] crates/bd-rewrite/src/check_test.rs (`a_branch_that_uses_a_name_another_branch_adds_is_a_missing_declaration`)

#### Scenario: An override of another branch's overload
- **WHEN** a commit of a built branch overrides an overload that another branch adds
- **THEN** the override is a missing declaration too
  [@test] crates/bd-rewrite/src/check_test.rs (`an_override_of_another_branchs_overload_is_a_missing_declaration`)

### Requirement: `brd check`: `--allow-missing` keeps the result and counts the branch as building

With `--allow-missing` a branch that is `missing_declaration` MUST keep that `result`, and only `ok` and the exit code
MUST count it as building, as `brd safe-push --allow-missing` starts anyway.

#### Scenario: A missing declaration allowed
- **WHEN** `brd check --allow-missing` finds a missing declaration and nothing else fails
- **THEN** `ok` is true and the exit code is 0
  [@test] crates/bd-rewrite/src/check_test.rs (`allow_dangling_keeps_the_reference_and_exits_0`)

#### Scenario: The result of an allowed missing declaration
- **WHEN** `brd check --allow-missing` finds a missing declaration
- **THEN** the branch keeps its `result` `missing_declaration`

### Requirement: `brd check`: A removed declaration the virtual tree still uses dangles

A built branch that removes a declaration its virtual tree still uses where HEAD's tree doesn't MUST be
`dangling_reference` (exit 1) with `{kind, identifier, file, line, removed_by, fixed_by, mutual, declared_in}`
(`declared_in` the `{file, line}` of the declaration before `removed_by`), when a commit outside
the branch and its ancestors changes the use (`fixed_by`).

#### Scenario: A branch removes a declaration that another branch's change still needs
- **GIVEN** a commit outside the branch and its ancestors changes the use (`fixed_by`)
- **WHEN** the branch removes the declaration its virtual tree still uses
- **THEN** the branch is `dangling_reference`, exit 1, with the fields of the reference
  [@test] crates/bd-rewrite/src/check_test.rs (`a_branch_that_removes_a_used_declaration_is_dangling_reference`)
  [@test] crates/trial-check/src/dangling_references_test.rs (`the_incident_a_moved_constant_whose_use_is_fixed_in_another_branch`, `a_removed_top_level_function_is_searched_by_its_name`)

### Requirement: `brd check`: A mutual dangling reference gets only a fix a trial proves

A dangling reference MUST be `mutual` when the change of `fixed_by` uses what the branch declares, with a `fix` only when a trial build of the plan
with it proves it (the branch builds without a proven dangling reference, no branch that built breaks): the move of
`fixed_by` into the branch when it changes only that file, else a `brd move-hunk` command, tried on commits written
as objects only that no ref names, at most two per check.

#### Scenario: A mutual reference fixed by a move of `fixed_by`
- **WHEN** the change of `fixed_by` uses what the branch declares and changes only that file
- **THEN** the reference is `mutual` and its `fix` moves `fixed_by` into the branch, and the plan with it builds
  [@test] crates/bd-rewrite/src/check_test.rs (`a_branch_that_removes_a_used_declaration_is_dangling_reference`)

#### Scenario: A mutual reference fixed by a move of a hunk
- **WHEN** the change of `fixed_by` uses what the branch declares and changes other files too
- **THEN** the reference is `mutual` and its `fix` is the `brd move-hunk` command that a trial build proves
  [@test] crates/bd-rewrite/src/check_test.rs (`a_mutual_dangling_reference_is_fixed_by_the_move_hunk_it_names`)

#### Scenario: A fix that breaks another branch
- **WHEN** the trial build of the plan with a fix breaks a branch that built
- **THEN** the fix is not offered
  [@test] crates/bd-rewrite/src/check_test.rs (`a_move_hunk_fix_that_breaks_another_branch_is_not_offered`)

#### Scenario: A trial writes objects only
- **WHEN** a `brd move-hunk` fix is tried
- **THEN** its commits are written as objects that no ref names
  [@test] crates/git-ops/src/move_hunk_test.rs (`a_trial_writes_the_commits_and_moves_no_ref`)

### Requirement: `brd check`: `--allow-dangling` keeps the result and counts the branch as building

With `--allow-dangling` the branch MUST keep its `result` `dangling_reference` and its references, and only `ok` and
the exit code MUST count it as building, as `brd safe-push --allow-dangling` starts anyway; any other failure MUST
still be exit 1.

#### Scenario: A dangling reference allowed
- **WHEN** `brd check --allow-dangling` finds a dangling reference and nothing else fails
- **THEN** the branch keeps its `result` and its references
- **AND** the exit code is 0
  [@test] crates/bd-rewrite/src/check_test.rs (`allow_dangling_keeps_the_reference_and_exits_0`)

#### Scenario: Another failure with `--allow-dangling`
- **WHEN** a branch fails otherwise, a missing declaration or a conflict
- **THEN** the exit code is still 1
  [@test] crates/bd-rewrite/src/check_test.rs (`allow_dangling_keeps_the_reference_and_exits_0`)

### Requirement: `brd check`: A use no commit changes is a possible dangling reference

A use no commit changes, and one the baseline got after the base, MUST be a `possible_dangling_reference` warning that
leaves the exit code alone.

#### Scenario: A use the baseline got after the base
- **WHEN** the use of a removed declaration landed on the baseline after the base
- **THEN** it is a `possible_dangling_reference` warning
  [@test] crates/trial-check/src/dangling_references_test.rs (`a_use_landed_on_the_baseline_is_a_possible_reference`)

#### Scenario: A use no commit changes
- **WHEN** no commit changes the use
- **THEN** it is a `possible_dangling_reference` warning and the exit code stays 0
  [@test] crates/bd-rewrite/src/check_test.rs (`a_possible_dangling_reference_is_a_warning_and_exit_0`)

### Requirement: `brd check`: Declarations and uses that are no dangling reference

A declaration still in its file, a private one used elsewhere, a use HEAD has as often, a use the branch changes
itself, a member moved with its class, a declaration moved within its package and a private one the branch declares
elsewhere MUST NOT count.

#### Scenario: A use the branch changes itself
- **WHEN** the branch changes the use itself
- **THEN** it is no dangling reference
  [@test] crates/trial-check/src/dangling_references_test.rs (`a_use_changed_by_the_branch_itself_is_not_dangling`)

#### Scenario: A declaration still in its file
- **WHEN** the declaration is changed in place, or a use is reformatted, so HEAD has it as often
- **THEN** it is no dangling reference
  [@test] crates/trial-check/src/dangling_references_test.rs (`a_declaration_changed_in_place_is_not_dangling`, `a_reformatted_use_is_not_dangling`)

#### Scenario: A private declaration
- **WHEN** a removed declaration is private, and a name of it is used elsewhere or the branch declares it elsewhere
- **THEN** it is no dangling reference
  [@test] crates/trial-check/src/dangling_references_test.rs (`a_private_declaration_is_not_searched_outside_its_file`, `a_private_declaration_moved_elsewhere_is_not_dangling`)

#### Scenario: A moved declaration
- **WHEN** a member moved with its class, or a declaration moved within its package
- **THEN** it is no dangling reference
  [@test] crates/trial-check/src/dangling_references_test.rs (`a_member_moved_with_its_class_is_not_dangling`, `a_top_level_function_moved_within_its_package_is_not_dangling`)

### Requirement: `brd check`: Words the syntax trees rule out are no use

By the syntax trees, a word of a comment or a string, a local of the same name, the name of an `override` member or a
name a `use` or an import alias still binds MUST NOT count either, nor a Rust name after a single `.`, nor the uses the
Rust crates rule out ([Symbol Dependencies](../../commit/symbol-dependencies/spec.md)).

#### Scenario: A word, an override line, a local, a name still bound
- **WHEN** the line search finds the name in a comment, on an `override` line, as a local of the same name, or as a name a `use` still binds
- **THEN** the syntax trees rule it out as a use
  [@test] crates/trial-check/src/dangling_references_test.rs (`a_word_in_kdoc_is_no_use`, `an_override_line_is_no_use`, `a_local_val_shadows_a_private_member_of_another_class`, `a_name_re_exported_by_a_use_is_not_removed`)

### Requirement: `brd check`: `via` says how the uses were found

`via` MUST say whether the trees confirmed the uses (`syntax_tree`) or the lines alone found them (`lines`). The check
MUST read no path but those where the trees differ.

#### Scenario: A dangling reference reported
- **WHEN** a dangling reference is reported
- **THEN** its `via` is `syntax_tree` when the trees confirmed the uses, `lines` when the lines alone found them
- **AND** only the paths where the trees differ were read
  [@test] crates/bd-rewrite/src/check_test.rs (`a_branch_that_removes_a_used_declaration_is_dangling_reference`)

### Requirement: `brd check`: The syntax summaries find a removed declaration

With `branchdeck.symbols on`, a dangling reference MUST also be found by the syntax summaries alone, with its `kind`
(`removed_declaration` for the line search above): `removed_declaration` for an instance or static member of a JVM
type or an enum entry that a commit of the branch removes with all its overloads, its key declared nowhere else in the
tree, where a use resolves to it (a receiver of its type, `this`, an override, an inherited call, its static path).

#### Scenario: A removed instance member
- **WHEN** a commit of the branch removes an instance member that a receiver of its type calls
- **THEN** it is a `removed_declaration`
  [@test] crates/trial-check/src/declaration_checks_test.rs (`a_removed_instance_member_dangles_where_its_receiver_calls_it`)

#### Scenario: A key declared elsewhere
- **WHEN** the tree declares the key of a removed member elsewhere
- **THEN** the member is kept only by its own key
  [@test] crates/trial-check/src/declaration_checks_test.rs (`a_removed_member_is_kept_only_by_its_own_key`)

#### Scenario: The checks of a session on sources
- **WHEN** a session checks plans on sources with the symbols on
- **THEN** the summaries find the declarations within the process budget
  [@test] crates/trial-check/src/process_budget_test.rs (`the_checks_of_a_session_on_sources`)

### Requirement: `brd check`: A changed form a use rejects is a changed declaration

A Kotlin or Java function, property or constant whose header, parameters (their types included) or reach a commit of
the branch changes, a type whose constructors it changes, or a key of which it removes an overload while another stays,
where the old form takes a use and no declaration of the key in the branch's tree does, MUST be `changed_declaration`.

#### Scenario: The proven cases
- **WHEN** the use is a call the new parameters reject, a call only the removed overload took, an added `suspend` outside a coroutine or a changed extension receiver
- **THEN** it is proven a `changed_declaration`, and only these are
  [@test] crates/trial-check/src/declaration_checks_test.rs (`a_function_made_suspend_dangles_where_another_branch_removed_its_call`, `a_changed_parameter_type_rejects_a_literal`, `a_removed_overload_dangles_where_the_one_left_rejects_the_call`, `a_changed_extension_receiver_breaks_its_call`, `a_new_parameter_dangles_only_where_the_call_does_not_fit_it`)
  [@test] crates/bd-rewrite/src/check_test.rs (`a_branch_that_changes_a_used_declaration_is_dangling_reference`)

### Requirement: `brd check`: A change of a form the summary cannot judge is only possible

A changed return type, nullability or type parameters and a suspend call in the lambda of an unknown function MUST be
only a `possible_changed_declaration` warning with its `fixed_by`. A possible one MUST be a warning of the kind
`possible_changed_declaration` or `possible_duplicate_declaration`.

#### Scenario: A header change the summary cannot judge
- **WHEN** a commit of the branch changes a return type, nullability or type parameters, or a use is a suspend call in the lambda of an unknown function
- **THEN** it is a `possible_changed_declaration` warning with its `fixed_by`
  [@test] crates/trial-check/src/declaration_checks_test.rs (`a_header_change_the_summary_cannot_judge_is_only_possible`)
  [@test] crates/bd-rewrite/src/check_test.rs (`a_changed_return_type_is_a_possible_changed_declaration`)

### Requirement: `brd check`: A type declared again in the same package is a duplicate

A JVM type or Kotlin top-level function a commit of the branch adds while the branch's tree, but not HEAD's, declares
the key in another file of the same package and module MUST be `duplicate_declaration`.

#### Scenario: A type declared again in the same package
- **WHEN** a commit of the branch adds a type that the branch's tree, but not HEAD's, declares in another file of the same package and module
- **THEN** it is a `duplicate_declaration`
  [@test] crates/trial-check/src/declaration_checks_test.rs (`a_type_declared_again_in_the_same_package_is_a_duplicate`)

### Requirement: `brd check`: Changes and uses that are no changed or duplicate declaration

A change of the body alone, a use the new form takes, an argument of no literal type, a call on a receiver of another
type, a bare call of a name in another class, a use the branch's own commits wrote (by the lines of their hunks, not
the text), a member that moves with its type, a change of a Rust or TypeScript signature, a declaration of another
package and a single declaration in a file another branch deletes MUST NOT count.

#### Scenario: A use the new form takes
- **WHEN** the use is a parameter with a default, a call in a suspend function or a coroutine builder, a `var` for a `val`, an explicit type of a property, `operator`
- **THEN** it is no changed declaration
  [@test] crates/trial-check/src/declaration_checks_test.rs (`a_function_made_suspend_fits_a_coroutine`)

#### Scenario: The body alone, another receiver
- **WHEN** a commit changes the body alone, or the use is a call on a receiver of another type
- **THEN** it is no changed declaration
  [@test] crates/trial-check/src/declaration_checks_test.rs (`a_change_of_the_body_alone_changes_no_form`, `a_call_on_a_receiver_of_another_type_is_no_use`)

#### Scenario: A use the branch's own commits wrote
- **WHEN** the branch's own commits wrote the use, by the lines of their hunks
- **THEN** it does not count
  [@test] crates/trial-check/src/declaration_checks_test.rs (`a_use_the_branch_writes_is_told_by_its_line_not_its_text`, `a_use_the_branch_writes_fits_its_form`)

#### Scenario: A Rust or TypeScript signature
- **WHEN** a commit changes a Rust or TypeScript signature
- **THEN** it does not count: the form is judged in the JVM languages only
  [@test] crates/trial-check/src/declaration_checks_test.rs (`a_changed_form_is_judged_in_the_jvm_languages_only`)

#### Scenario: Another package, a file another branch deletes
- **WHEN** the other declaration is of another package, or is a single declaration in a file another branch deletes
- **THEN** it is no duplicate
  [@test] crates/trial-check/src/declaration_checks_test.rs (`a_type_of_the_same_name_in_another_package_is_no_duplicate`, `a_declaration_another_branch_deletes_is_no_duplicate`)

### Requirement: `brd check`: A parent that needs its child is an inverted stack

A branch whose conflict or dangling reference needs a commit of a branch stacked on it, at any depth, MUST have
`inverted_stack {branch, child, fixes}` and keep its `result`; the fixes (`move_to_child`, `swap`) MUST be plan
parts for `brd group` that a trial build of the plan with them passes for both branches, and no `swap` for a mutual
dangling reference; with no fix that builds, a `reason` instead.

#### Scenario: A parent that needs its child
- **WHEN** the conflict of a branch needs a commit of a branch stacked on it
- **THEN** it has `inverted_stack` with the fixes that a trial build passes for both branches, and keeps its `result`
  [@test] crates/bd-rewrite/src/check_test.rs (`a_parent_that_needs_its_child_is_an_inverted_stack`)

#### Scenario: A mutual dangling reference of a stack
- **WHEN** the dangling reference of a stack is mutual
- **THEN** the fixes hold no `swap`
  [@test] crates/bd-rewrite/src/check_test.rs (`a_mutual_dangling_reference_of_a_stack_is_an_inverted_stack_without_a_swap`)

#### Scenario: No fix builds
- **WHEN** no fix builds
- **THEN** `inverted_stack` has a `reason` instead
  [@test] crates/bd-rewrite/src/check_test.rs (`an_inverted_stack_that_no_fix_builds_says_why`)

### Requirement: `brd check`: `--build` runs a command in the tree of each `ok` branch

`--build <command>` MUST run the command in the tree of each `ok` branch, extracted outside every working tree
(`--build-paths`: only those paths, names, never patterns), and make a failure `build_failed` with its exit code and
the end of its output; without `--build` the check MUST run no command.

#### Scenario: A build on each branch tree
- **WHEN** `brd check --build <command>` runs
- **THEN** the command runs in the extracted tree of each `ok` branch, and a failure is `build_failed` with its exit code and the end of its output
  [@test] crates/bd-rewrite/src/check_test.rs (`check_build_runs_the_command_on_each_branch_tree`, `a_build_that_passed_on_a_tree_is_not_run_again`)

#### Scenario: A path of `--build-paths`
- **WHEN** `--build-paths` names a path
- **THEN** it is a name, never a pattern
  [@test] crates/git-ops/src/tree_archive_test.rs (`a_path_is_a_name`)

#### Scenario: Outside every working tree
- **WHEN** the tree of a branch is extracted for the build
- **THEN** it is extracted outside every working tree, and no ref, config, index or working tree changes
  [@test] crates/bd-cli/src/agent_help_test.rs (`read_only_commands_write_no_ref_config_index_or_working_tree`)

### Requirement: `brd check`: The build command reaches the shell as written

The command of `--build` MUST reach the shell as written: `sh -c`, on Windows cmd with its quotes kept (a quoted path).

#### Scenario: A quoted path on Windows
- **WHEN** the command holds a quoted path on Windows
- **THEN** cmd gets it with its quotes kept; elsewhere `sh -c` gets the command as written

### Requirement: `brd check`: A build that passed does not run again on the same objects

A build that passed MUST NOT run again on the same command, branch and extracted objects (the tree, or the object of
each `--build-paths` path the tree has), also on a new commit of the same tree (`BD_COMMIT` is not in the key), and
MUST say `cached`; a failed build, another branch, a changed extracted object, `--rebuild` and `--no-cache` MUST run it.

#### Scenario: A passed build on the same tree
- **WHEN** a build passed on the same command, branch and extracted objects
- **THEN** it does not run again and says `cached`
  [@test] crates/bd-rewrite/src/check_test.rs (`a_build_that_passed_on_a_tree_is_not_run_again`)
  [@test] crates/object-cache/src/cache_test.rs (`passed_builds_are_read_by_their_version`)

#### Scenario: A failed build, another branch, `--rebuild`
- **WHEN** the build failed, or the branch is another, or `--rebuild` is given
- **THEN** the build runs again
  [@test] crates/bd-rewrite/src/check_test.rs (`a_failed_build_runs_again`, `a_build_of_another_branch_with_the_same_tree_runs`, `rebuild_runs_the_command_on_a_cached_tree`)

### Requirement: `brd check`: The key of a build costs one process for all branches

The key MUST cost one `cat-file --batch-check` for all branches, and a cached build no `archive`.

#### Scenario: A warm check with a build
- **WHEN** a check with a build runs, cold or warm
- **THEN** the key of all branches costs one `cat-file --batch-check`, and a cached build runs no `archive`
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_check_with_a_build`, `a_warm_check_with_a_build`)
  [@test] crates/git-ops/src/cat_file_batch_test.rs (`the_ids_of_a_path_with_a_space_a_tree_a_gitlink_and_missing_names`)

### Requirement: `brd check`: Merges are memoized by their trees

Merges MUST be memoized by their trees in the object cache, and a cached check MUST equal an uncached one.

#### Scenario: A cached check
- **WHEN** a check runs again with the cache
- **THEN** it gives the same report as an uncached one
  [@test] crates/trial-check/src/check_test.rs (`the_cache_gives_the_same_report`)
  [@test] crates/bd-rewrite/src/check_test.rs (`cached_check_equals_uncached`)

### Requirement: `brd check`: A check session checks many plans

A `CheckSession` MUST read the commits, the baseline and the branch tips once, and check any number of plans as
`check_plan` does.

#### Scenario: Many plans in one session
- **WHEN** a `CheckSession` checks several plans
- **THEN** each result is that of `check_plan`, and the commits, the baseline and the tips are read once
  [@test] crates/trial-check/src/check_test.rs (`a_session_checks_many_plans_as_check_plan_does`)

### Requirement: `brd suggest`: The plan holds only the commits whose branch changes

`brd suggest` MUST print a plan for `brd group` with only the commits whose branch changes (a fixup that follows its
target left out, a prefixed commit that goes to its own issue branch in `unassign`), the clusters with their name,
commit count, `change` against the groups now in `current_branches`, `why`, `weak`, issue keys and areas, the `parent`
and `restacked`, the subjects the plan rewrites, and the trial sync of the plan.

#### Scenario: A plan of what changes
- **WHEN** `brd suggest` runs
- **THEN** its plan holds only the commits whose branch changes, a regroup plan for `brd group`
- **AND** each cluster has its `change`: `kept`, `renamed`, `merged`, `split` or `new`
  [@test] crates/bd-rewrite/src/suggest_test.rs (`the_plan_moves_only_what_changes_and_is_a_regroup_plan`)

### Requirement: `brd suggest`: It writes objects only and its plan, applied, suggests nothing

`brd suggest` MUST write git objects and the cache only, and the plan, applied, MUST suggest nothing more.

#### Scenario: The plan applied
- **WHEN** the suggested plan is applied with `brd group` and `brd suggest` runs again
- **THEN** it suggests nothing more
  [@test] crates/bd-rewrite/src/suggest_test.rs (`suggests_a_checked_plan_that_is_stable_once_applied`)

### Requirement: `brd suggest`: Each cluster names its commits in three disjoint lists

Each cluster MUST name its commits in three disjoint lists that agree with the plan: `kept`, the ones it leaves as
they are; `moved`, the ones that change branch, each with `leaves`, the branch it leaves (`null` when unassigned);
and `new_path`, the ones whose branch stays but whose subject the plan gives another stacked path to the same leaf.
`moved` and `new_path` together MUST be what the plan changes of a commit (the top-level `rewritten` counts them).

#### Scenario: The branch a moved commit leaves
- **WHEN** a commit changes branch
- **THEN** it is in `moved`, its `leaves` by its group now against the leaf of the cluster's path
- **AND** a fixup that follows its target moves with it, and the commits an issue group takes in or lets go move
  [@test] crates/bd-rewrite/src/suggest_test.rs (`suggests_a_checked_plan_that_is_stable_once_applied`, `the_plan_moves_only_what_changes_and_is_a_regroup_plan`)

#### Scenario: A commit whose subject gets another path
- **WHEN** the branch of a commit stays and the plan gives its subject another stacked path to the same leaf
- **THEN** it is in `new_path`
  [@test] crates/bd-rewrite/src/suggest_test.rs (`a_plan_never_moves_a_commit_with_an_explicit_prefix`)

### Requirement: `brd suggest`: A commit with an explicit prefix never moves

Without `--fresh`, a plan MUST never move a commit with an explicit `(prefix)`, `--rewrite-issues` included: it stays
in `kept`. The one rewrite of such a subject SHALL be the stacked path it declares (`(gone/a)` to `a`): the commit
stays in its branch and is in `new_path`.

#### Scenario: A prefixed commit and a declared path that is gone
- **WHEN** `brd suggest` runs without `--fresh`, with `--rewrite-issues` too, on a commit with an explicit `(prefix)`
- **THEN** the commit stays in `kept`
- **AND** a subject that declares a stacked path whose parent is gone is rewritten to its leaf and is in `new_path`
  [@test] crates/bd-rewrite/src/suggest_test.rs (`a_plan_never_moves_a_commit_with_an_explicit_prefix`)

### Requirement: `brd suggest`: Hard units join fixups, prefixes and issue keys

Hard units MUST join a fixup and its target, the commits of one prefix (unless `--fresh`) and the commits without a
prefix of one issue key.

#### Scenario: The hard units
- **WHEN** the commits are clustered
- **THEN** a fixup and its target, the commits of one prefix and the commits without a prefix of one issue key are one unit each
  [@test] crates/commit-clustering/src/clustering_test.rs (`hard_units_join_dependencies_fixups_prefixes_and_keys`)

### Requirement: `brd suggest`: Two prefixes never join

Two prefixes MUST never join: a dependency between them stacks them, with no warning.

#### Scenario: A dependency between two prefixes
- **WHEN** a commit of one prefix depends on a commit of another
- **THEN** the two are stacked, not joined, with no warning
  [@test] crates/commit-clustering/src/clustering_test.rs (`an_edge_between_two_anchors_stacks_them_and_anchors_never_merge`)

### Requirement: `brd suggest`: A dependency is an edge between clusters, not a join

A prefix MUST be an anchor by the leaf of its path, and a dependency MUST join no units: it MUST be an edge between
two clusters, and only the dependencies of a cycle of units may join them.

#### Scenario: A dependency between two units
- **WHEN** a unit depends on another and they form no cycle
- **THEN** the dependency stacks their clusters instead of joining them
  [@test] crates/commit-clustering/src/clustering_test.rs (`with_stacked_branches_a_dependency_stacks_the_branches_instead_of_joining_them`)

### Requirement: `brd suggest`: A cycle between two prefixes stays a warning

A cycle between two prefixes MUST stay an `anchor_dependency` warning.

#### Scenario: Two prefixes that depend on each other
- **WHEN** the dependencies of two prefixes form a cycle
- **THEN** they stay apart with an `anchor_dependency` warning

### Requirement: `brd suggest`: Commits of an issue key a prefix names join that prefix

The commits without a prefix of an issue key that a prefix names MUST join that prefix, because they are one branch.

#### Scenario: A follow-up of an issue branch
- **WHEN** a commit without a prefix has the issue key that a prefix names
- **THEN** it joins that prefix, and stays there once the plan is applied
  [@test] crates/bd-rewrite/src/suggest_test.rs (`a_follow_up_that_joins_an_issue_branch_stays_there_once_applied`)

### Requirement: `brd suggest`: Every cluster a cluster depends on is its ancestor

Each cluster MUST get a parent so that every cluster it depends on is an ancestor: the parent its prefixes declare
when that parent covers them; else the deepest cluster it depends on that covers the others; else the stacks of the
clusters it needs MUST become one chain, in the order of the dependencies and the oldest cluster first, which only
adds ancestors.

#### Scenario: A branch needs another
- **WHEN** a cluster depends on another
- **THEN** it is stacked on the cluster it needs
  [@test] crates/commit-clustering/src/clustering_test.rs (`with_stacked_branches_a_dependency_stacks_the_branches_instead_of_joining_them`)
  [@test] crates/bd-rewrite/src/suggest_test.rs (`with_stacked_branches_a_branch_stacks_on_the_branch_it_needs`)

#### Scenario: Clusters it needs in different stacks
- **WHEN** no single cluster it depends on covers the others
- **THEN** the stacks of the clusters it needs become one chain, in the order of the dependencies, the oldest first
  [@test] crates/commit-clustering/src/clustering_test.rs (`stack_clusters_makes_every_needed_cluster_an_ancestor`)

### Requirement: `brd suggest`: A declared parent gives way in a cycle

A declared parent that is no cluster MUST go, a cluster MUST NOT be its own parent, `--fresh` MUST declare no parent,
and in a cycle the declared parent MUST give way first, then the oldest cluster of a cycle that waits on nothing
outside it MUST go first, never a cluster that only waits on the cycle.

#### Scenario: A parent without commits
- **WHEN** a declared parent is no cluster
- **THEN** it goes
  [@test] crates/commit-clustering/src/clustering_test.rs (`a_merge_that_closes_a_cycle_waits_and_a_parent_without_commits_goes`)

#### Scenario: A declared parent in a cycle
- **WHEN** a declared parent makes a cycle
- **THEN** the declared parent gives way first
  [@test] crates/commit-clustering/src/clustering_test.rs (`stack_clusters_keeps_a_declared_parent_and_gives_way_in_a_cycle`)

#### Scenario: A cycle in a source component
- **WHEN** clusters form a cycle
- **THEN** the oldest cluster of a cycle that waits on nothing outside it goes first, never a cluster that only waits on the cycle
  [@test] crates/commit-clustering/src/clustering_test.rs (`stack_order_breaks_a_cycle_in_a_source_component`)

### Requirement: `brd suggest`: The plan names a stacked cluster by its path

The plan MUST name a stacked cluster `parent/leaf` and a root by its leaf, so that a path of more levels declares no
parent that the placement moved. A stacked parent SHALL come from the prefixes, else from the stack parents; a
restacked cluster MUST name its path also when no commit moves (`"a/b": []`, the stack parents move), a cluster
unstacked from its stack parent MUST be in `unstack`, and a commit with the bare `(leaf)` stays where it is.

#### Scenario: A deeper path
- **WHEN** the plan stacks a cluster under a path of more levels
- **THEN** it names the cluster `parent/leaf`, and declares no parent that the placement moved
  [@test] crates/bd-rewrite/src/suggest_test.rs (`a_stacked_plan_stacks_an_issue_group_and_rewrites_a_deeper_path`)

#### Scenario: A restack where no commit moves
- **WHEN** a cluster is restacked and none of its commits moves
- **THEN** the plan names its path with no commits (`"a/b": []`)
  [@test] crates/bd-rewrite/src/suggest_test.rs (`stacked_branches_and_an_empty_range`)

### Requirement: `brd suggest`: A stored parent without need is reported and kept

A parent that stays, comes from the stack parents and holds no cluster the cluster depends on MUST be
`stacked_without_need`, and the plan MUST keep it.

#### Scenario: A stored parent the cluster does not need
- **WHEN** a stack parent holds no cluster the cluster depends on
- **THEN** the cluster is `stacked_without_need` and the plan keeps the parent
  [@test] crates/bd-rewrite/src/suggest_test.rs (`a_stored_parent_without_need_is_reported_and_kept`)

### Requirement: `brd suggest`: A cluster of whole issue branches stays an issue group

A cluster of whole issue branches MUST stay an issue group, stacked too. The trial MUST apply the plan as `brd group`
does, so `rewritten` counts only the commits that need a prefix.

#### Scenario: A stacked issue group
- **WHEN** a cluster of whole issue branches is stacked
- **THEN** it stays an issue group
- **AND** `rewritten` counts only the commits that need a prefix
  [@test] crates/bd-rewrite/src/suggest_test.rs (`a_stacked_plan_stacks_an_issue_group_and_rewrites_a_deeper_path`, `the_plan_moves_only_what_changes_and_is_a_regroup_plan`)

### Requirement: `brd suggest`: `why` names a new parent and its reason

`why` MUST name a new parent and its reason.

#### Scenario: A restack
- **WHEN** the plan gives a cluster a new parent
- **THEN** its `why` names the parent and the reason
  [@test] crates/commit-clustering/src/clustering_test.rs (`a_follow_up_of_two_stacked_branches_joins_the_upper_one_and_a_restack_names_its_reason`)

### Requirement: `brd suggest`: An edge by symbols orders and never joins

With `branchdeck.symbols on`, an edge by symbols only MUST never join two units: it MUST be an edge of the stack
outside the join of a cycle; a unit whose names three or more units use MUST order none of them; a branch the plan
doesn't build on the branch whose name it uses MUST be a `symbol_dependency` warning.

#### Scenario: An edge by symbols
- **WHEN** the only edge between two units is by symbols
- **THEN** it orders them in the stack and never joins them
- **AND** a unit whose names three or more units use orders none of them
  [@test] crates/commit-clustering/src/clustering_test.rs (`an_edge_by_symbols_orders_and_never_joins`)

#### Scenario: A use of another branch
- **WHEN** a branch uses a name another branch declares
- **THEN** the plan stacks it on that branch, else it is a `symbol_dependency` warning
  [@test] crates/bd-rewrite/src/suggest_test.rs (`with_the_symbols_on_a_use_of_another_branch_stacks`)

### Requirement: `brd suggest`: Soft merges go by a deterministic score

Soft merges MUST go by a deterministic integer score of issue key mentions, conventional scopes (`type(scope):` and
`type scope:`), shared paths weighted by IDF, the disagreement of issue projects and scopes, and log adjacency: the
best pair first, a tie to the oldest, under `--max-commits` (40) and down to the threshold 0.40.

#### Scenario: Adjacency alone
- **WHEN** only log adjacency lifts a pair over the threshold
- **THEN** the merge is `weak`
  [@test] crates/commit-clustering/src/clustering_test.rs (`a_merge_that_only_adjacency_lifts_over_the_threshold_is_weak`)

#### Scenario: Scopes
- **WHEN** commits without keys have the same conventional scope, `type(scope):` or `type scope:`
- **THEN** they merge, and another scope halves the weight of their shared paths
  [@test] crates/commit-clustering/src/clustering_test.rs (`the_same_scope_merges_commits_without_keys_and_another_scope_halves_paths`)
  [@test] crates/sync-utils/src/issue_pattern.rs (`semantic_prefixes_with_and_without_parentheses`)

#### Scenario: Mentions of issue keys
- **WHEN** a commit mentions an issue key anywhere in its text
- **THEN** the mention links keys of one project, and hardly across projects
  [@test] crates/commit-clustering/src/clustering_test.rs (`a_mention_links_keys_of_one_project_but_hardly_across_projects`)
  [@test] crates/sync-utils/src/issue_pattern.rs (`issue_keys_anywhere_in_a_text`)

#### Scenario: Shared paths
- **WHEN** commits share paths
- **THEN** they merge by their IDF weight, and a feature of many units or a peripheral file weighs little
  [@test] crates/commit-clustering/src/clustering_test.rs (`shared_paths_merge_but_a_feature_of_many_units_or_a_peripheral_file_weighs_little`)

#### Scenario: The cap and a tie
- **WHEN** a merge would pass `--max-commits`, or two pairs score the same
- **THEN** the cap stops the merge, and the tie goes to the oldest pair
  [@test] crates/commit-clustering/src/clustering_test.rs (`the_cap_stops_soft_merges_and_a_tie_goes_to_the_oldest_pair`)

### Requirement: `brd suggest`: A key of many units links nothing

A key that more than max(10, units / 3) units name MUST link nothing, and one body of a branch of a prefix that names
the key of a whole issue branch MUST NOT take the issue in without a second body or shared paths.

#### Scenario: A key many units name
- **WHEN** more than max(10, units / 3) units name a key
- **THEN** the key links nothing

#### Scenario: One mention of a whole issue
- **WHEN** one body of a branch of a prefix names the key of a whole issue branch, without a second body or shared paths
- **THEN** the branch does not take the issue in
  [@test] crates/commit-clustering/src/clustering_test.rs (`a_branch_of_a_prefix_takes_in_a_whole_issue_only_by_more_than_one_mention`)

### Requirement: `brd suggest`: A dependency links a cluster without a prefix to its need

A dependency MUST link a cluster without a prefix and an issue key to the cluster it depends on, and a merge MUST
NOT make a cycle of the dependencies and the declared parents.

#### Scenario: A follow-up without a prefix or a key
- **WHEN** a cluster without a prefix and an issue key depends on another
- **THEN** it is linked to the cluster it depends on
  [@test] crates/commit-clustering/src/clustering_test.rs (`with_stacked_branches_a_dependency_stacks_the_branches_instead_of_joining_them`, `a_follow_up_of_two_stacked_branches_joins_the_upper_one_and_a_restack_names_its_reason`)

#### Scenario: A merge that closes a cycle
- **WHEN** a merge would make a cycle of the dependencies and the declared parents
- **THEN** it waits
  [@test] crates/commit-clustering/src/clustering_test.rs (`a_merge_that_closes_a_cycle_waits_and_a_parent_without_commits_goes`)

### Requirement: `brd suggest`: The clusters of a large branch match a human grouping

With `--fresh`, on the anonymized input of a large monorepo branch, the clusters MUST match the grouping a human
made with a pairwise precision of at least 0.95 and a recall of at least 0.85, and keep the issue projects and the
named pairs of groups apart.

#### Scenario: The golden monorepo branch
- **WHEN** `--fresh` clusters the anonymized input of a large monorepo branch
- **THEN** the pairwise precision against the human grouping is at least 0.95 and the recall at least 0.85
- **AND** the issue projects and the named pairs of groups stay apart
  [@test] crates/commit-clustering/src/clustering_test.rs (`golden_monorepo_branch`)

### Requirement: `brd suggest`: A proven dangling reference is repaired like a conflict

A proven dangling reference MUST be repaired like a conflict, the removing commit needing `fixed_by` (it stacks on
its branch); a mutual one MUST NOT be stacked but named in an `unresolved_conflict` warning that says to move the
change into `removed_by`. A possible one SHALL be a `possible_dangling_reference` warning (or
`possible_changed_declaration`, `possible_duplicate_declaration`).

#### Scenario: A dangling reference
- **WHEN** a commit removes, or changes the form of, a declaration that `fixed_by` of another branch still needs
- **THEN** the removing commit's branch stacks on the branch of `fixed_by`
  [@test] crates/bd-rewrite/src/suggest_test.rs (`a_dangling_reference_stacks_the_branch_on_its_fix`, `a_changed_declaration_stacks_the_branch_on_its_fix`)

#### Scenario: A mutual dangling reference
- **WHEN** the dangling reference is mutual
- **THEN** nothing is stacked, and an `unresolved_conflict` warning says to move the change into `removed_by`
  [@test] crates/bd-rewrite/src/suggest_test.rs (`a_mutual_dangling_reference_suggests_moving_the_change`)

### Requirement: `brd suggest`: A conflict joins the conflicting commit to what it needs

A conflict of the check MUST join the hard unit of the conflicting commit to the units of the commits it needs by
its lines that its branch and the branches it is built on lack, by the conflicting files only when the lines name
none, also inside an issue group, and cluster again, at most 5 times, never joining two prefixes; the exit code MUST
be 1 when a branch still conflicts.

#### Scenario: A conflict repaired
- **WHEN** the check of the plan conflicts
- **THEN** the conflicting commit joins the units of the commits its lines need, and the clustering runs again
  [@test] crates/commit-clustering/src/clustering_test.rs (`repair_joins_a_conflicting_commit_to_what_it_needs`)
  [@test] crates/bd-rewrite/src/suggest_test.rs (`a_repair_stacks_on_what_the_lines_of_the_conflict_need_not_every_commit_of_the_file`)

#### Scenario: A lock file and an issue group
- **WHEN** the conflict is in a file the lines name nothing of, or inside an issue group
- **THEN** the repair goes by the conflicting files, also inside the issue group
  [@test] crates/bd-rewrite/src/suggest_test.rs (`a_conflict_in_a_lock_file_stacks_the_commits_and_fresh_ignores_prefixes`, `whole_issue_branches_become_an_issue_group`)

### Requirement: `brd suggest`: A conflict on another cluster's commit stacks first

A conflict on a commit of another cluster MUST first stack the two clusters, and join their units only when the same
conflict comes again.

#### Scenario: The conflict stays after the stack
- **WHEN** a conflict is on a commit of another cluster
- **THEN** the two clusters are stacked first
- **AND** their units join only when the same conflict comes again
  [@test] crates/commit-clustering/src/clustering_test.rs (`with_stacked_branches_a_repair_stacks_first_and_joins_when_the_conflict_stays`)

### Requirement: `brd suggest`: A name follows the order of preference

A name MUST be the cluster's prefix, the branch its commits come from when it holds the most of that branch, its
single issue key, a scope of at least 60 % of its commits, `<area leaf>-<word>`, its most frequent key or
`cluster-N`, in this order.

#### Scenario: The names in order
- **WHEN** a cluster is named
- **THEN** it takes the first of the names in the order of preference that it has
  [@test] crates/commit-clustering/src/clustering_test.rs (`names_in_order_of_preference_with_collisions`, `subject_words_drop_stopwords_and_endings`)

#### Scenario: The branch it holds most of
- **WHEN** a cluster holds the most of the branch its commits come from
- **THEN** it takes the name of that branch
  [@test] crates/commit-clustering/src/clustering_test.rs (`a_cluster_takes_the_name_of_the_branch_it_holds_most_of`)

### Requirement: `brd suggest`: A taken name gets a suffix

A name another cluster, a group now or a virtual branch has, in any case, MUST get `-2`, `-3`, unless the cluster's
commits come from it.

#### Scenario: A collision
- **WHEN** another cluster, a group now or a virtual branch has the name, in any case
- **THEN** the cluster gets `-2`, `-3`, unless its commits come from it
  [@test] crates/commit-clustering/src/clustering_test.rs (`names_in_order_of_preference_with_collisions`)
  [@test] crates/bd-rewrite/src/suggest_test.rs (`stacked_branches_and_an_empty_range`)

### Requirement: `brd suggest`: `kept` is one whole branch under its own name

`kept` MUST mean one whole branch under its own name.

#### Scenario: A branch that stays whole
- **WHEN** a cluster is one whole branch under its own name, with `--fresh` too
- **THEN** its `change` is `kept`
  [@test] crates/commit-clustering/src/clustering_test.rs (`changes_against_the_groups_now`)
  [@test] crates/bd-rewrite/src/suggest_test.rs (`fresh_keeps_the_name_of_a_branch_that_stays_whole`)

### Requirement: `brd suggest`: Whole issue branches become an issue group

A cluster of two or more whole issue branches MUST go into `issue_groups` of the plan, marked `"via": "issue_group"`,
so that the plan rewrites no commit; `--rewrite-issues` SHALL give the commits a prefix instead. An issue group MUST
be an anchor, as a prefix is.

#### Scenario: Two whole issue branches
- **WHEN** a cluster holds two or more whole issue branches
- **THEN** the plan has it in `issue_groups`, marked `"via": "issue_group"`, and rewrites no commit
  [@test] crates/bd-rewrite/src/suggest_test.rs (`suggests_a_checked_plan_that_is_stable_once_applied`, `whole_issue_branches_become_an_issue_group`)

### Requirement: `brd suggest`: The dump of the input hides every name

`--dump-input` (hidden) MUST print the input of the clustering with every name hashed by a keyed hash (SipHash) with
a random key that is not kept, and without subjects, bodies, ids and authors, so that the dump clusters as the
repository does; only the module markers of file paths may become one marker.

#### Scenario: The dump
- **WHEN** `brd suggest --dump-input` runs
- **THEN** no name, subject, body, id or author is in the dump
- **AND** the dump clusters as the repository does
  [@test] crates/commit-clustering/src/clustering_test.rs (`anonymize_keeps_the_clustering_and_no_name`)
  [@test] crates/bd-rewrite/src/suggest_test.rs (`the_dump_hides_every_name`)

### Requirement: `brd sync`: It runs the app's sync without the repository lock

`brd sync` MUST run the app's sync (`sync_core::sync::sync_branches`) and, like it, without the repository lock: its
refs move by compare-and-swap (below), so it never waits for a rewrite or Update Project, and a rewrite of the app or
of `brd` MUST never wait for it.

#### Scenario: Another process holds the lock
- **WHEN** another process holds the repository lock
- **THEN** `brd sync` syncs without waiting for it
  [@test] crates/bd-cli/src/sync_test.rs (`syncs_while_another_process_holds_the_lock`)

#### Scenario: A rewrite beside a running sync
- **WHEN** a rewrite starts while a sync runs
- **THEN** the rewrite does not wait for the sync
  [@test] crates/bd-cli/src/sync_test.rs (`a_rewrite_beside_a_running_sync_does_not_wait_for_it`)

### Requirement: `brd sync`: It prints the state of each branch

`brd sync` MUST print per branch its `state` (`created`, `updated`, `unchanged`, `error`, `conflict`, `blocked`), the
commit count, the new tip and the remote state, plus the unassigned count.

#### Scenario: A sync reports each branch
- **WHEN** `brd sync` runs
- **THEN** each branch has its `state`, its commit count, its new tip and its remote state
- **AND** the unassigned count is printed
  [@test] crates/bd-cli/src/sync_test.rs (`syncs_and_reports_each_branch`)

### Requirement: `brd sync`: It refuses during an operation, on a detached HEAD or a managed branch

`brd sync` MUST refuse, changing nothing, while a rebase, merge, cherry-pick or revert is in progress
(`operation_in_progress`), HEAD is detached (`detached_head`) or a branch under the prefix is checked out
(`managed_branch`; exit 4 all three): the sync would archive the branches the half-done history, or the history of
one group, lacks.

#### Scenario: A rebase in progress
- **WHEN** a rebase is in progress
- **THEN** `brd sync` is refused with `operation_in_progress`, exit 4, and nothing changes
  [@test] crates/bd-cli/src/sync_test.rs (`waits_while_a_rebase_is_in_progress`)

#### Scenario: A branch under the prefix checked out
- **WHEN** a branch under the prefix is checked out
- **THEN** the sync is refused with `managed_branch`, exit 4, and nothing changes
  [@test] crates/sync-core/src/sync_test.rs (`sync_waits_while_a_branch_under_the_prefix_is_checked_out`)

#### Scenario: A detached HEAD
- **WHEN** HEAD is detached
- **THEN** the sync is refused with `detached_head`, exit 4, and nothing changes

### Requirement: `brd sync`: The message of `locked` names the holder of the lock

The message of `locked` MUST name the holder of the lock, its command line, pid and how long it holds it, from the
record (`<lock>.holder`) a holder writes once it has the lock and deletes before it releases it; a record of a
process that is gone (or none, of an older Branch Deck) MUST name "Another Branch Deck operation".

#### Scenario: A lock timeout
- **WHEN** a wait for the lock times out
- **THEN** the message names the holder, its command line, pid and how long it holds the lock
- **AND** the holder deletes its record before it releases the lock
  [@test] crates/sync-core/src/repository_lock_test.rs (`a_lock_timeout_names_the_holder_and_its_release_deletes_the_record`)

#### Scenario: The record of a gone holder
- **WHEN** the record names a process that is gone, or there is no record
- **THEN** the message names "Another Branch Deck operation"
  [@test] crates/sync-core/src/repository_lock_test.rs (`the_record_of_a_gone_holder_names_nobody`)

### Requirement: `brd sync`: `stacks` lists the stack branch of each stack that forks

`stacks` MUST list the stack branch of each stack that forks, as the sync built it, with its `state` (`built`;
`blocked` with the `member` and the `reason`; `conflict` with the commit, its subject and files; `stale` with the
`error`), its `tip`, `commits`, `members` and the `remote` summary of its tip, then the ones kept for their remote
branch (`stale`, no members).

#### Scenario: A forking stack in sync and status
- **WHEN** a stack forks
- **THEN** `stacks` lists its stack branch with its `state`, `tip`, `commits`, `members` and `remote`, as the schema has it
  [@test] crates/bd-remote/src/status_test.rs (`the_stack_branch_of_a_forking_stack_in_sync_and_status`)
  [@test] crates/bd-cli/src/schema_test.rs (`outputs_match_schema`)

### Requirement: `brd sync`: A stack tip that is not built is never pushed

A tip of a stack branch in any state but `built` is not the stack's commits and MUST never be pushed
([Whole Stack](../../branch/whole-stack/spec.md)).

#### Scenario: A blocked, conflicting or stale stack branch
- **WHEN** a stack branch is `blocked`, `conflict` or `stale`
- **THEN** its tip is never pushed

### Requirement: `brd sync`: A branch moves only from the tip the sync listed

A sync (the app's and `brd sync`, neither of which takes the repository lock) MUST move a virtual branch only from the
tip it listed at its start, or create it only while no ref of its name exists: a branch another process moved to
another tip meanwhile MUST keep that tip and be `error` ("Not updated: another process moved …; sync again"), while
the other branches are written; one moved to the head the sync built (the copies are deterministic) is written.

#### Scenario: A branch moved since the listing
- **WHEN** another process moved a virtual branch to another tip since the sync listed it
- **THEN** the branch keeps that tip and is `error`, and the other branches are written
  [@test] crates/sync-core/src/branch_processor_test.rs (`a_branch_moved_since_the_listing_is_not_overwritten`)

#### Scenario: A branch created meanwhile
- **WHEN** a ref of the name of a new branch was created meanwhile
- **THEN** it is not overwritten
  [@test] crates/sync-core/src/branch_processor_test.rs (`a_new_branch_created_meanwhile_is_not_overwritten`)

#### Scenario: A branch moved to the head the sync built
- **WHEN** another process moved the branch to the head the sync built
- **THEN** the branch is written

### Requirement: `brd sync`: A conflict the words resolve is committed

With `branchdeck.resolveByWords` on (the default), a cherry-pick that git stops at MUST be committed when the words
resolve every conflicting file (IntelliJ's simple resolve under the default policy, all or nothing per commit; only
content conflicts of UTF-8 text), with the author, the date and the message of the commit, and the branch built.

#### Scenario: A conflict its words resolve
- **WHEN** git stops at a commit whose conflicts its words resolve
- **THEN** the copy is committed with the author, the date and the message of the commit, and the branch is built
  [@test] crates/bd-cli/src/sync_test.rs (`a_conflict_that_its_words_resolve_builds_and_says_so`)
  [@test] crates/trial-check/src/word_resolve_sync_test.rs (`a_conflict_that_its_words_resolve_is_copied_with_the_author_and_the_message_of_the_commit`, `crlf_lines_keep_their_ends`, `a_stack_of_commits_builds_on_the_resolved_copy`)

#### Scenario: A conflict that only ignoring whitespace resolves
- **WHEN** the words of a conflict resolve only when whitespace, the line ends too, does not count: lines joined into
  one that no side has, or a final line end removed beside a removed last line
- **THEN** the commit stays a conflict of git
  [@test] crates/git-ops/src/word_resolve_test.rs (`a_conflict_that_only_ignoring_whitespace_resolves_stays_the_conflict_of_git`)

#### Scenario: Only content conflicts of UTF-8 text
- **WHEN** a conflict is a modify/delete, an add/add, a binary file, a file of a merge driver or text that is not UTF-8
- **THEN** the commit stays a conflict of git
  [@test] crates/git-ops/src/word_resolve_test.rs (`a_modify_delete_beside_a_resolvable_file_keeps_the_commit_a_conflict`, `an_add_add_and_a_binary_file_stay_conflicts`, `a_merge_driver_keeps_the_conflict_of_git`, `text_that_is_not_utf8_stays_a_conflict`)

### Requirement: `brd sync`: The commits the words resolved are listed

`resolved` of the branch MUST list such commits with git's conflicts and the files; the mapping note keeps the counts,
and `brd status` and `brd commits` MUST show them as `resolved_by_words` of the commit. `brd check` MUST build alike.

#### Scenario: The sync says so
- **WHEN** a sync resolved a commit by its words
- **THEN** `resolved` of the branch lists it with git's conflicts and the files
  [@test] crates/bd-cli/src/sync_test.rs (`a_conflict_that_its_words_resolve_builds_and_says_so`)

#### Scenario: The next sync
- **WHEN** the next sync reuses the resolved copy
- **THEN** it says so from the counts of its mapping note
  [@test] crates/trial-check/src/word_resolve_sync_test.rs (`the_next_sync_reuses_the_resolved_copy_and_says_so_from_its_note`)

#### Scenario: A check
- **WHEN** `brd check` builds a branch with such a commit
- **THEN** it resolves it as the sync does
  [@test] crates/trial-check/src/word_resolve_sync_test.rs (`a_check_resolves_as_the_sync_does`)

### Requirement: `brd sync`: A copy of an older resolve by words is checked once

A copy whose mapping note names an older resolve by words MUST be checked once by its tree: the same tree keeps the
commit, with no new copy and no move of its branch, and its note MUST take the current resolve, so that no later sync
checks it again; another tree MUST copy the commit again, and a conflict that the current resolve leaves MUST be the
conflict of git.

#### Scenario: The same bytes
- **WHEN** the current resolve gives the tree of the copy
- **THEN** the copy stays, unchanged, and its note names the current resolve
- **AND** the next sync runs the processes of an unchanged sync
  [@test] crates/trial-check/src/word_resolve_sync_test.rs (`a_copy_of_an_older_resolve_with_the_same_bytes_keeps_its_commit_and_takes_this_resolve`)
  [@test] crates/sync-core/src/process_budget_test.rs (`a_copy_of_an_older_resolve_is_checked_once`)

#### Scenario: Other bytes
- **WHEN** the current resolve gives another tree
- **THEN** the commit is copied again with the tree of the current resolve, and its note names the new copy
  [@test] crates/trial-check/src/word_resolve_sync_test.rs (`a_copy_of_an_older_resolve_with_other_bytes_is_copied_again`)

#### Scenario: A conflict the current resolve leaves
- **WHEN** the current resolve leaves the conflict of git
- **THEN** the commit is that conflict, and a copy noted by the current resolve is taken as it is
  [@test] crates/trial-check/src/word_resolve_sync_test.rs (`a_copy_of_an_older_resolve_that_this_resolve_leaves_is_the_conflict_of_git`)

### Requirement: `brd sync`: Without the setting or a resolved file the conflict stays

With the setting off, or one file that does not resolve, the commit MUST stay a conflict of git; a copy that resolved
MUST be copied again and conflict once the setting is off.

#### Scenario: The setting off and on again
- **WHEN** `branchdeck.resolveByWords` is off
- **THEN** the commit is a conflict of git, and a copy that resolved is copied again and conflicts
- **AND** on again, the resolved copy comes back
  [@test] crates/trial-check/src/word_resolve_sync_test.rs (`the_setting_off_gives_the_conflict_of_git_and_on_again_the_resolved_copy`)

#### Scenario: One file its words leave
- **WHEN** one conflicting file does not resolve by its words
- **THEN** the commit stays a conflict of git
  [@test] crates/trial-check/src/word_resolve_sync_test.rs (`a_file_with_a_conflict_that_its_words_leave_keeps_the_commit_a_conflict`)

### Requirement: `brd sync`: The same commits give the same resolved copy everywhere

The same commits MUST give the same copy in every repository.

#### Scenario: Another repository
- **WHEN** the same commits are synced in another repository
- **THEN** the resolved copy is the same
  [@test] crates/trial-check/src/word_resolve_sync_test.rs (`the_same_commits_give_the_same_copy_in_another_repository`)

### Requirement: `brd sync`: The missing commits of a conflict leave out the ancestors

The `conflict` of a branch in `conflict` MUST list in `missing_commits` the older commits that change a conflicting
file and are neither in the branch nor in the branches it is built on, never the copies of its own or its ancestors'
commits.

#### Scenario: A conflict of a stacked branch
- **WHEN** a branch built on others conflicts
- **THEN** `missing_commits` names the older commits that change a conflicting file outside the branch and its ancestors, with their branch
  [@test] crates/trial-check/src/missing_commits_test.rs (`the_missing_commits_leave_out_the_branches_it_is_built_on_and_name_their_branch`)

### Requirement: `brd sync`: A virtual commit is reused only while its note fits

A virtual commit MUST be reused only while its mapping note fits the commit: the trees the note records as its source,
and the message and the author of the copy. A note carried to a changed commit (`git notes copy`,
`notes.rewriteRef`) MUST make the sync and the check copy the commit again.

#### Scenario: A stale note
- **WHEN** a note was copied to a commit with another tree, or carried by an amend with `notes.rewriteRef`
- **THEN** the sync and the check copy the commit again
  [@test] crates/bd-cli/src/sync_test.rs (`a_stale_note_rebuilds_the_branch`)
  [@test] crates/sync-core/src/branch_processor_test.rs (`a_note_copied_to_a_commit_with_another_tree_is_stale`, `an_amend_with_notes_rewrite_ref_rebuilds_the_copy`)
  [@test] crates/trial-check/src/check_test.rs (`check_does_not_reuse_a_stale_copy`)
  [@test] crates/git-ops/src/notes_test.rs (`the_lines_are_found_by_their_prefix`)

### Requirement: `brd sync`: A note without a source is checked against its copy

A note without a source MUST be checked against the tree of its copy, keep a copy that fits and get its source;
`--no-cache` MUST check every note that way.

#### Scenario: An older note
- **WHEN** a note has no source
- **THEN** a copy whose tree fits is kept and the note gets its source
  [@test] crates/sync-core/src/branch_processor_test.rs (`a_note_without_a_source_keeps_an_older_copy_and_gets_one`)

#### Scenario: `--no-cache`
- **WHEN** `brd sync --no-cache` runs
- **THEN** it checks the tree of every copy
  [@test] crates/bd-cli/src/sync_test.rs (`no_cache_verifies_each_note`)
  [@test] crates/sync-core/src/branch_processor_test.rs (`a_verifying_sync_checks_the_tree_of_each_copy`)

### Requirement: `brd sync`: The remote state counts what a push drops apart from older versions

The remote state MUST count the remote commits a push drops (`behind`, `dropped_commits`) apart from the older
versions of commits it has ([Outgoing Commits](../../repository/outgoing/spec.md#what-is-outgoing)): `amended`,
`restacked` and `landed`, and the commits it has in another version (`reworded`), each left out at 0.

#### Scenario: Amended and restacked commits
- **WHEN** the remote has older versions of commits amended here, or of commits of the branches it is built on
- **THEN** they count as `amended` (of commits amended here) and `restacked` (of commits of the branches it is built on), not as `behind`
  [@test] crates/bd-remote/src/push_test.rs (`amended_and_restacked_commits_push_without_force`)

#### Scenario: Reworded commits
- **WHEN** the remote has a commit of the branch in another version, another message or author
- **THEN** it counts as `reworded`, not as `behind`

#### Scenario: Landed commits
- **WHEN** the remote has commits the baseline has
- **THEN** they count as `landed` (of commits the baseline has, `landed_commits` with the `baseline_commit` of each)
- **AND** a count of 0 is left out
  [@test] crates/bd-remote/src/push_test.rs (`a_push_replaces_the_remote_commits_the_baseline_landed_without_force`)

### Requirement: `brd sync`: A conflict names the commit and exits with 1

A conflict MUST name the conflicting commit, its subject, the files and the commits the change likely needs, and the
exit code MUST be 1, as for every branch the sync could not build (`error`, `blocked`).

#### Scenario: A conflict
- **WHEN** a branch conflicts
- **THEN** the conflict names the commit, its subject, the files and the commits the change likely needs, and the exit code is 1
  [@test] crates/bd-cli/src/sync_test.rs (`a_conflict_exits_with_1`)

#### Scenario: A branch that is not built
- **WHEN** the sync could not build a branch, `error` or `blocked`
- **THEN** the exit code is 1
  [@test] crates/bd-cli/src/sync_test.rs (`a_branch_that_is_not_built_is_exit_1`)

### Requirement: `brd sync`: An unchanged conflict comes from the object cache

A sync of an unchanged conflicting branch MUST take that view from the object cache, shared with the app, and run
neither the merge nor the analysis of the conflict again; it MUST report the view the analysis made, with the branches
of the missing commits named as it groups them. `--no-cache` MUST analyse every conflict.

#### Scenario: An unchanged conflicting branch
- **WHEN** a conflicting branch is synced again unchanged
- **THEN** the view comes from the object cache, with neither the merge nor the analysis run again
  [@test] crates/sync-core/src/process_budget_test.rs (`an_unchanged_sync_of_a_conflicting_branch`)
  [@test] crates/object-cache/src/cache_test.rs (`conflict_chains_are_read_by_their_version`)

#### Scenario: The remembered conflict
- **WHEN** the view of a conflict comes from the cache
- **THEN** it is the one the analysis made, with the branches of the missing commits named as the sync groups them
  [@test] crates/sync-core/src/sync_test.rs (`a_remembered_conflict_is_the_one_the_analysis_made`)

### Requirement: `brd sync`: The clean copies before a conflict are not copied again

A branch that conflicts after commits that copy cleanly MUST NOT copy those again either: the copies the sync wrote
before the conflict are taken while the branch's commits before it are the same, also with a commit added after the
conflict, and it MUST say the same about every commit.

#### Scenario: A conflict after a clean commit
- **WHEN** a branch that conflicts after a clean commit is synced again
- **THEN** the copy of the clean commit is taken, not written again, also with a commit added after the conflict
  [@test] crates/sync-core/src/process_budget_test.rs (`an_unchanged_sync_of_a_conflict_after_a_clean_commit`)

### Requirement: `brd sync`: It names the push of each branch a push would change

Per branch whose remote branch a push would change it MUST print the `brd push` to run, as brd names itself
(`brd -C <repo>`): plain for unpushed, amended, reworded, restacked or landed commits; with
`--force --expect-remote <remote.head>`
when the push drops commits and no branch it is built on has commits for the remote; with `--force` alone when one
has. Run as written, it MUST push.

#### Scenario: A plain push
- **WHEN** a branch has unpushed, amended, reworded, restacked or landed commits for its remote branch
- **THEN** the sync names a plain `brd push`, and run as written it pushes
  [@test] crates/bd-remote/src/push_test.rs (`sync_names_the_push_of_each_branch`, `amended_and_restacked_commits_push_without_force`, `a_push_replaces_the_remote_commits_the_baseline_landed_without_force`)
  [@test] crates/bd-cli/src/sync_test.rs (`syncs_and_reports_each_branch`)

#### Scenario: A push that drops commits
- **WHEN** the push drops remote commits and no branch it is built on has commits for the remote
- **THEN** the sync names the push with `--force --expect-remote <remote.head>`
  [@test] crates/bd-remote/src/push_test.rs (`sync_names_the_push_of_each_branch`)

#### Scenario: A child behind its remote with a parent to push
- **WHEN** the push drops remote commits and a branch it is built on has commits for the remote
- **THEN** the sync names the push with `--force` alone
  [@test] crates/bd-remote/src/push_test.rs (`a_child_behind_its_remote_with_a_parent_to_push_forces_without_expect_remote`)

### Requirement: `brd sync`: It includes the integration results and the update check

The integration results of archived branches and the update check of the checked out branch MUST be included
compactly, with the landed commits (`update.landing`, `landed`).

#### Scenario: Commits the baseline already has
- **WHEN** the baseline already has commits of the checked out branch
- **THEN** the sync reports them in `update.landing` and `landed`
  [@test] crates/bd-cli/src/sync_test.rs (`reports_the_commits_the_baseline_already_has`)

### Requirement: `brd push`: It pushes each virtual branch to its own remote branch, never into master

`brd push` MUST push each virtual branch to its own remote branch, never into master (the app's Push Branch,
[Outgoing Commits](../../repository/outgoing/spec.md#push)), and its help and the skill MUST say so and point to
`brd land`, which lands a branch into master by the route of the repository.

#### Scenario: The help of `brd push`
- **WHEN** an agent reads the help of `brd push` or the skill
- **THEN** it says that `brd push` pushes the virtual branches, never into master, and points to `brd land`
  [@test] crates/bd-cli/src/agent_help_test.rs (`the_help_of_brd_push_says_it_pushes_the_virtual_branches_never_into_master`)

### Requirement: `brd push`: It syncs first and pushes the branches in one atomic push

`brd push <branch>…` MUST sync first, without the repository lock as `brd sync`, then push the branches in one atomic
`git push --porcelain` through the remote command policy, parents first, with an explicit lease per ref; `--all`
MUST need `--yes`.

#### Scenario: A push with a lease
- **WHEN** `brd push <branch>` runs
- **THEN** it syncs, then pushes the branch with an explicit lease, and reads git's `--porcelain` lines
  [@test] crates/bd-remote/src/push_test.rs (`pushes_with_a_lease_and_prints_the_pull_request`)
  [@test] crates/sync-core/src/push_lease_test.rs (`parses_porcelain_lines`)

#### Scenario: `--all` without `--yes`
- **WHEN** `brd push --all` runs without `--yes`
- **THEN** it is refused
  [@test] crates/bd-remote/src/push_test.rs (`all_needs_yes_and_unsynced_branches_are_refused`)

### Requirement: `brd push`: It is an adapter of the engine the app's push runs

`brd push` SHALL be an adapter of the engine the app's push runs (`operations::push`): an intent `Push` per named
branch, planned over the one-shot session of its sync (`push_planned`), the plans of all of them one push
(`push_together`), run as the app's run (`land_execute::run_planned`).

#### Scenario: Several named branches
- **WHEN** `brd push` names several branches
- **THEN** each is an intent `Push` planned over the session of the sync, and their plans are one push run as the app's run

### Requirement: `brd push`: The lock covers only the witness of the plan and the freeze

The lock MUST cover only the witness of the plan (the tips it pushes, read before the lock and under it) and the
freeze of its commits and leases: a branch another writer or sync moved since the sync MUST be `plan_changed`
(exit 3), nothing pushed.

#### Scenario: The lock of a push
- **WHEN** `brd push` takes the repository lock
- **THEN** it holds it only for the witness of the plan and the freeze of its commits and leases
- **AND** a branch moved since the sync is `plan_changed`, exit 3, and nothing is pushed
  [@test] crates/bd-remote/src/push_test.rs (`a_push_of_bd_locks_only_the_witness_and_the_freeze`)

### Requirement: `brd push`: The network push runs after the lock and sends the frozen commits

The retargets, the cancel and the restart of `--restart-running` and the network push MUST run after the lock, and the
network push MUST send exactly the frozen commits (`<commit>:refs/heads/<name>`).

#### Scenario: The network push
- **WHEN** the push goes to the remote
- **THEN** it holds no repository lock and sends exactly the frozen commits
  [@test] crates/bd-remote/src/push_test.rs (`the_network_push_holds_no_repository_lock`)

### Requirement: `brd push`: The robots and the pull requests are asked after the sync

The user's robots of the holds and the pull requests of the branches MUST be asked after the sync, side by side: no
other writer waits for the sync, GitHub, Patronus or the network.

#### Scenario: The robot list of a push
- **WHEN** `brd push` reads the user's robots
- **THEN** it reads them after the sync, outside the repository lock
  [@test] crates/bd-remote/src/push_test.rs (`the_robot_list_of_a_push_is_read_outside_the_repository_lock`)

### Requirement: `brd push`: It does not read again what its sync read

`brd push` MUST NOT read again what its sync read: the stack, the tips, the remote tips, the remotes, the descriptions
and the push size limit MUST come from the sync of the same run. Besides the push it SHALL read only the push URL of
`origin`, once, the Space settings for a GitHub mirror, the size check, the witness of its plan and the check of the
remote-tracking refs of a push that went.

#### Scenario: The processes of a push
- **WHEN** `brd push` runs
- **THEN** the size check is one `log`, one `cat-file`
- **AND** the witness of its plan is one `for-each-ref` before the lock and one under it; none for `--dry-run`, which moves nothing
- **AND** the check of the remote-tracking refs of a push that went is one `for-each-ref`, an `update-ref` only for one git left behind
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_push`, `a_push_over_a_moved_remote`)

### Requirement: `brd push`: A stacked branch takes its parents with commits for the remote along

A stacked branch MUST take the branches it is built on along, parents first, in the same atomic push, when they
have commits for the remote, as the app's push button does ([Stacked Branches](../../branch/stacked-branches/spec.md));
one on the remote as it is stays out, and one taken along brings no children with `--with-children`.

#### Scenario: Parents go first
- **WHEN** a stacked branch is pushed and its parents have commits for the remote
- **THEN** they go along in the same atomic push, parents first
  [@test] crates/bd-remote/src/push_test.rs (`parents_go_first_and_a_child_alone_carries_its_parents_commits`)

#### Scenario: A parent on the remote as it is
- **WHEN** a parent of the pushed branch is on the remote as it is
- **THEN** it stays out of the push
- **AND** with `--with-children`, a parent taken along brings no children of its own
  [@test] crates/bd-remote/src/push_test.rs (`a_parent_goes_along_only_with_commits_for_the_remote_and_brings_no_children`)

### Requirement: `brd push`: `--alone` and `--expect-remote` push only the named branches

`--alone` (named branches only) and `--expect-remote` (the lease of one branch) MUST push only the named branches: a
stacked branch carries the commits of the branches it is built on, its pull request into the baseline includes them
either way.

#### Scenario: A child alone
- **WHEN** `brd push --alone` names a stacked branch
- **THEN** only it is pushed, carrying the commits of its parents
  [@test] crates/bd-remote/src/push_test.rs (`parents_go_first_and_a_child_alone_carries_its_parents_commits`)

### Requirement: `brd push`: A branch that didn't sync or is behind its remote is refused

It MUST refuse (`push_refused`, exit 4, nothing pushed) a branch that didn't sync, and one behind its remote branch
without `--force` (a parent taken along too, with `--alone` as the way out).

#### Scenario: An unsynced branch
- **WHEN** a named branch didn't sync
- **THEN** the push is `push_refused`, exit 4, and nothing is pushed
  [@test] crates/bd-remote/src/push_test.rs (`all_needs_yes_and_unsynced_branches_are_refused`)

#### Scenario: A branch behind its remote
- **WHEN** a branch, or a parent taken along, is behind its remote branch and `--force` is not given
- **THEN** the push is refused, nothing is pushed, and `--alone` is the way out of a parent
  [@test] crates/bd-remote/src/push_test.rs (`a_moved_remote_branch_is_fetched_and_reported_and_force_replaces_it`)

### Requirement: `brd push`: Older versions of amended, restacked or landed commits are not behind

Older versions of commits amended, restacked or landed are not behind: such a branch MUST push without `--force`, and
a parent with only them MUST be taken along.

#### Scenario: Amended and restacked commits
- **WHEN** the remote has only older versions of commits amended or restacked
- **THEN** the branch pushes without `--force`, and a parent with only them is taken along
  [@test] crates/bd-remote/src/push_test.rs (`amended_and_restacked_commits_push_without_force`)

### Requirement: `brd push`: A member of a stack that landed whole pushes without `--force`

A member of a stack that landed whole, with a new commit since, MUST push without `--force`: its remote branch has its
commits as they were before the landing, which the baseline has by identity; `replaced_landed` of the pushed branch
(with `--dry-run` too) MUST name them with their baseline commits.

#### Scenario: A new commit after the landing
- **WHEN** a member of a stack that landed whole has a new commit
- **THEN** it pushes without `--force`, and `replaced_landed` names the remote commits with their baseline commits, with `--dry-run` too
  [@test] crates/bd-remote/src/push_test.rs (`a_push_replaces_the_remote_commits_the_baseline_landed_without_force`)

### Requirement: `brd push`: One remote commit the baseline lacks keeps the refusal

One remote commit the baseline lacks MUST keep the refusal, and `dropped_commits` MUST name only such commits.

#### Scenario: A remote commit the baseline lacks
- **WHEN** the remote branch has a commit the baseline lacks besides the landed ones
- **THEN** the push is refused, and `dropped_commits` names only that commit
  [@test] crates/bd-remote/src/push_test.rs (`a_remote_commit_the_baseline_lacks_still_refuses_the_push`)

### Requirement: `brd push`: A branch the app's push leaves to its row refuses the push

Where the app's push leaves a branch its scope covers out to its row (a child the sync didn't build or a Safe Push
holds, an ancestor or a child whose push drops remote commits), `brd push` MUST refuse the push, nothing pushed, in
the same words (`push_refusals`).

#### Scenario: A child the sync didn't build
- **WHEN** the scope of the push covers a child the sync didn't build, one a Safe Push holds, or an ancestor or child whose push drops remote commits
- **THEN** `brd push` refuses the push in the words of the app (`push_refusals`), and nothing is pushed

### Requirement: `brd push`: A branch the remote has as it is is listed up to date

A named branch, or a child `--with-children` covers, that the remote has as it is (by content: `remote.up_to_date`,
a branch only rebuilt on a newer baseline too) MUST be listed `up_to_date` with its lease and its pull request draft,
and nothing of it pushed, as the app's push sends nothing of it; its pull request MUST NOT be retargeted.

#### Scenario: A branch on the remote as it is
- **WHEN** a named branch is on the remote as it is
- **THEN** it is listed `up_to_date` with its lease and its pull request draft, and nothing of it is pushed
  [@test] crates/bd-remote/src/push_test.rs (`pushes_with_a_lease_and_prints_the_pull_request`)

#### Scenario: A branch only rebuilt on a newer baseline
- **WHEN** a branch was only rebuilt on a newer baseline
- **THEN** it is `up_to_date` by content and not pushed again
  [@test] crates/bd-remote/src/push_test.rs (`a_branch_only_rebuilt_on_a_newer_baseline_is_not_pushed_again`)

### Requirement: `brd push`: `--with-children` adds the branches stacked on the named ones

`--with-children` MUST add the branches stacked on the named ones by declared parent, directly or further down,
parents first, to the same atomic push, the branches the app's Push with Children covers (`domain_policy::stack::
descendants`, the steps of `push::push_plan`; [Stacked Branches](../../branch/stacked-branches/spec.md)). It MUST
need named branches and MUST NOT go with `--expect-remote`.

#### Scenario: The children of a named branch
- **WHEN** `brd push --with-children` names a branch
- **THEN** the branches stacked on it, directly or further down, go in the same atomic push, parents first
  [@test] crates/bd-remote/src/push_test.rs (`with_children_pushes_the_branches_stacked_on_the_named_ones`)

#### Scenario: Without named branches or with `--expect-remote`
- **WHEN** `--with-children` is given without named branches, or with `--expect-remote`
- **THEN** it is refused

### Requirement: `brd push`: A blocked child refuses `--with-children`

A blocked child MUST refuse the push as a named branch that didn't sync does, where the app's push leaves it out to
its row.

#### Scenario: A blocked child
- **WHEN** a child that `--with-children` covers is blocked
- **THEN** the push is refused, nothing pushed
  [@test] crates/bd-remote/src/push_test.rs (`with_children_refuses_a_blocked_child`)

### Requirement: `brd push`: `--with-children` of a forking stack's root pushes its stack branch

`--with-children` of a named root of a stack that forks MUST also push its stack branch
([Whole Stack](../../branch/whole-stack/spec.md)) in the same atomic push, after the branches, leased on the remote
head its remote status compared with, as the app's Push with Children does: once it is on the remote or the pull
request of the whole stack is open, only when this sync built it (`built`) and the remote lacks it by content.

#### Scenario: A stack branch on the remote
- **WHEN** `brd push --with-children` names the root of a stack that forks, and its stack branch is on the remote, `built` and lacked by the remote
- **THEN** the stack branch goes in the same atomic push, after the branches, leased on the remote head its remote status compared with
  [@test] crates/bd-remote/src/push_test.rs (`with_children_takes_the_stack_branch_along_once_it_is_on_the_remote`)

#### Scenario: The pull request of the whole stack is open
- **WHEN** the pull request of the whole stack is open, by the one GitHub search of the push, after the sync, its listing read by the retargets too
- **THEN** the stack branch goes along
  [@test] crates/bd-remote/src/safe_push_test.rs (`push_with_children_takes_the_stack_branch_along_while_its_pull_request_is_open`)

### Requirement: `brd push`: `stacks` lists a pushed stack branch as `branches` lists a branch

`stacks` MUST list a stack branch the push takes along as `branches` lists a branch, `name` the root, with the pull
request of the whole stack. A stack branch on no remote and in no pull request MUST stay local without a warning.

#### Scenario: A stack branch on no remote
- **WHEN** the stack branch is on no remote and in no pull request
- **THEN** it stays local, without a warning

#### Scenario: A pushed stack branch
- **WHEN** the stack branch goes along
- **THEN** `stacks` lists it, `name` the root, with the pull request of the whole stack

### Requirement: `brd push`: A stack branch in another state or on another remote is never pushed

A stack branch in another state, or pushed to another remote than `origin`, which its remote status compares with,
MUST never be pushed: `stack_warnings` names it, and so does one every branch of whose stack merged
Safe Pushes landed whole.

#### Scenario: A stack every branch of which landed
- **WHEN** every branch of the whole auth stack landed in master with the merged Safe Pushes of login and session
- **THEN** `stack_warnings` says "test-user/stack/auth was not pushed: every branch of the whole auth stack landed in master with the Safe Pushes of login and session: nothing of it is left to push"

#### Scenario: A stack branch that is not built
- **WHEN** the stack branch is in another state than `built`, or pushed to another remote than `origin`
- **THEN** it is not pushed, and `stack_warnings` names it

### Requirement: `brd push`: A child named with `--with-children` takes no stack branch

A child named with `--with-children` MUST take no stack branch.

#### Scenario: A named child of a forking stack
- **WHEN** `brd push --with-children` names a child of a stack that forks, not its root
- **THEN** the stack branch is not pushed

### Requirement: `brd push`: The lease is the remote head the sync compared with

The lease MUST be the remote head the sync of the same run compared with (`remote.head`), as the app leases on the
remote head its row showed ([Outgoing Commits](../../repository/outgoing/spec.md)): a remote commit the branch has by
content MUST go through on the first try, `--force` MUST drop what the sync counted as `behind` and nothing pushed
since, and either one's push MUST leave the other's next push valid.

#### Scenario: A remote commit the branch has by content
- **WHEN** the remote has a commit the branch has by content
- **THEN** a plain push replaces it on the first try
  [@test] crates/bd-remote/src/push_test.rs (`a_plain_push_replaces_a_remote_commit_the_branch_has_by_content`)

#### Scenario: `--force`
- **WHEN** `brd push --force` runs
- **THEN** it drops what the sync counted as `behind`, and nothing pushed since
  [@test] crates/bd-remote/src/push_test.rs (`force_drops_only_what_the_sync_saw_and_expect_remote_pins_the_lease`)

#### Scenario: The app and brd
- **WHEN** the app pushes a branch and then `brd push` does, or the other way round
- **THEN** the next push of either is valid
  [@test] crates/bd-remote/src/push_test.rs (`the_app_and_bd_share_the_lease`)

### Requirement: `brd push`: `--expect-remote` leases one branch on a commit

`--expect-remote <commit>` MUST lease one branch on that commit.

#### Scenario: A pinned lease
- **WHEN** `brd push --expect-remote <commit>` runs
- **THEN** the branch is leased on that commit
  [@test] crates/bd-remote/src/push_test.rs (`force_drops_only_what_the_sync_saw_and_expect_remote_pins_the_lease`)

### Requirement: `brd push`: Another remote is compared with as `origin` is

With `--remote` other than `origin`, which the sync compares with, the refusal, the lease and the errors MUST use
the remote status of the pushed branches against that remote, computed the same way; a `--remote` that is not a
configured remote MUST be `no_remote`.

#### Scenario: Another remote
- **WHEN** `brd push --remote` names another configured remote
- **THEN** the refusal, the lease and the errors use the remote status against that remote
  [@test] crates/bd-remote/src/push_test.rs (`another_remote_is_compared_with_as_origin_is`)

#### Scenario: A remote that is not configured
- **WHEN** `--remote` names no configured remote
- **THEN** the push is `no_remote`

### Requirement: `brd push`: A moved remote branch refuses the whole push and is fetched

A remote branch that is not where the lease expects it MUST refuse the push, the other branches of the push included
(it is atomic), and brd MUST fetch the branches of the push.

#### Scenario: A moved lease in a stack
- **WHEN** the remote branch of one branch of a stack moved
- **THEN** the whole push is refused, and brd fetches the branches of the push
  [@test] crates/bd-remote/src/push_test.rs (`a_moved_lease_refuses_the_whole_stack_and_names_the_branch_that_moved`, `a_moved_remote_branch_is_fetched_and_reported_and_force_replaces_it`)

### Requirement: `brd push`: It pushes again over a moved remote that lacks nothing of the branch

When each remote that moved has no commits its branch lacks and the branch has work for it (deleted, moved back to an
older commit of the branch, rebuilt older versions: `RemoteStatusUpdate::pushes_again`, the rule of the app's push),
brd MUST push again at once, once, leased on the fetched heads, and mark each such branch `pushed_over` (its `lease`
the head it went over, `null` for a deleted one), with `--expect-remote` too.

#### Scenario: A remote that moved to commits of the branch
- **WHEN** a remote branch was deleted, moved back to an older commit of the branch, or holds rebuilt older versions
- **THEN** brd pushes again at once, once, leased on the fetched head, and marks the branch `pushed_over`
  [@test] crates/bd-remote/src/push_test.rs (`pushes_again_over_a_moved_remote_that_lacks_nothing_of_the_branch`)
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_push_over_a_moved_remote`)
  [@test] crates/domain-types/src/remote_status_test.rs (`pushes_again_over_a_moved_remote_only_when_nothing_of_it_is_lost`)

### Requirement: `brd push`: The push over a moved remote is one rule with Force Push

The push again over a moved remote MUST be one rule with the app's Force Push, which leases on the head its row showed
and loses nothing of a remote that moved only to commits the branch has.

#### Scenario: Force Push of the app
- **WHEN** the app's Force Push finds the remote moved only to commits the branch has
- **THEN** it pushes over it as `brd push` does, losing nothing

### Requirement: `brd push`: A moved remote it does not push over is `lease_rejected`

A push over a moved remote that brd does not push again over (a remote with commits its branch lacks, or no work of
the branch for it), or one whose remote moved again, MUST be `lease_rejected` (exit 4, not retryable) with nothing pushed,
listing the branches whose remote moved in `error.branches` with their remote status as fetched (`head`, `behind`,
`dropped_commits`). A branch refused as behind MUST be listed the same way, and `brd sync` prints the same remote
status per branch.

#### Scenario: A remote with commits the branch lacks
- **WHEN** a remote that moved has commits its branch lacks, or moved again
- **THEN** the push is `lease_rejected`, exit 4, nothing pushed, and `error.branches` lists the branches whose remote moved with their remote status as fetched
  [@test] crates/bd-remote/src/push_test.rs (`a_moved_remote_branch_is_fetched_and_reported_and_force_replaces_it`, `a_moved_lease_refuses_the_whole_stack_and_names_the_branch_that_moved`)

#### Scenario: A branch refused as behind
- **WHEN** a branch is refused as behind
- **THEN** it is listed the same way, and `brd sync` prints the same remote status per branch
  [@test] crates/bd-remote/src/push_test.rs (`force_drops_only_what_the_sync_saw_and_expect_remote_pins_the_lease`)
  [@test] crates/bd-cli/src/sync_test.rs (`syncs_and_reports_each_branch`)

### Requirement: `brd push`: The app's stack push is the same push

The app's stack push MUST be the same push (`sync_core::push::PreparedPush::push_leased`).

#### Scenario: A stack pushed from the app
- **WHEN** the app pushes a stack
- **THEN** it runs the push `brd push` runs (`PreparedPush::push_leased`)

### Requirement: `brd push`: A push the remote refuses with every lease holding is `push_refused`

A push the remote refuses while every lease holds (a hook, a protected branch, a pack too large) MUST be
`push_refused` with what git said of each ref and what the remote said why (its `remote:` lines, git's `error:` lines,
not the progress), nothing pushed.

#### Scenario: A hook refuses the push
- **WHEN** the remote refuses the push while every lease holds
- **THEN** it is `push_refused` with what git said of each ref and the `remote:` and `error:` lines, not the progress, and nothing is pushed
  [@test] crates/bd-remote/src/push_test.rs (`a_push_the_remote_refuses_with_its_leases_holding_is_push_refused`)

### Requirement: `brd push`: A push over the size limit is `push_too_large` before the remote

A push whose commits add more files than the push size limit (`branchdeck.pushSizeLimit`, 50 MiB unless set, `0` or
`off` for none) MUST be `push_too_large` (exit 4) before brd asks the remote, a `--dry-run` too, with nothing pushed.
`brd safe-push` of a whole stack and `brd push --unapplied` MUST go through the same check.

#### Scenario: A push over the limit
- **WHEN** the commits of a push add more than the limit
- **THEN** it is `push_too_large`, exit 4, before brd asks the remote, and nothing is pushed
- **AND** the message says how much it adds, the directory that holds most of it and the largest files, and `paths` lists the directory (`dir/`) and the files ([Outgoing](../../repository/outgoing/spec.md#push))
  [@test] crates/bd-remote/src/push_test.rs (`a_push_over_the_size_limit_is_push_too_large_before_the_remote`)

### Requirement: `brd push`: A size limit that is neither a size nor off is invalid

A limit that is neither a size nor off MUST be `invalid_arguments`.

#### Scenario: A limit that is no size
- **WHEN** `branchdeck.pushSizeLimit` is neither a size nor off
- **THEN** the push is `invalid_arguments`

### Requirement: `brd push`: Each branch prints its pull request draft into the baseline

Per branch it MUST print the pull request base, the baseline without the remote (`master`) for every branch, stacked
or not ([Stacked Branches](../../branch/stacked-branches/spec.md#pull-requests)), the head, the title and the body as the
app's dialog writes them ([Pull Requests](../../branch/pull-requests/spec.md#creating-a-pull-request): the `brd describe`
text, else the text of the commits, from the sync's report).

#### Scenario: The draft of a pushed branch
- **WHEN** `brd push` pushes a branch, stacked or not, alone or with its parents or children
- **THEN** it prints the pull request base `master`, the head, and the title and the body the app's dialog writes
  [@test] crates/bd-remote/src/push_test.rs (`pushes_with_a_lease_and_prints_the_pull_request`, `parents_go_first_and_a_child_alone_carries_its_parents_commits`, `with_children_pushes_the_branches_stacked_on_the_named_ones`)

### Requirement: `brd push`: It prints a ready `gh pr create` command

Per branch it MUST print a ready `gh pr create --title … --body-file -` command, with `--repo` for a GitHub repository
`origin` doesn't push to (a mirror).

#### Scenario: A GitHub mirror
- **WHEN** `origin` pushes to another repository than the GitHub repository of the branches
- **THEN** the command names the GitHub repository with `--repo`
  [@test] crates/bd-remote/src/push_test.rs (`the_pull_request_command_names_a_github_mirror`)

### Requirement: `brd push`: The summaries of the issues come from YouTrack

The summaries of the issues MUST come from YouTrack as in the app, read with the token of `YOUTRACK_TOKEN` or of the
app's sign-in and never renewed.

#### Scenario: A branch of an issue
- **WHEN** the draft of a branch names an issue
- **THEN** its summary is read from YouTrack with the token of `YOUTRACK_TOKEN` or of the app's sign-in, which is never renewed

### Requirement: `brd push`: A layer of a stack of GitHub goes into the branch below it

On a route without Patronus the draft of a layer of a stack of GitHub MUST go into the branch below it, with only its
own commits, the bottom's into the baseline ([Stacks of GitHub](../../branch/stacked-branches/spec.md#stacks-of-github));
`github_stacks` MUST name each stack of the drafted branches, its `chain` bottom first and the `gh stack link` of their
remote branches, which opens the pull requests a layer lacks and links them.

#### Scenario: A stack of GitHub
- **WHEN** `brd push` drafts the layers of a stack on a route without Patronus
- **THEN** each layer's draft goes into the branch below it, the bottom's into the baseline
- **AND** `github_stacks` names the stack, its `chain` bottom first and the `gh stack link` of their remote branches
  [@test] crates/bd-remote/src/safe_push_test.rs (`push_drafts_the_layers_of_a_stack_of_github_into_the_branches_below_them`)

### Requirement: `brd push`: A pull request into a virtual branch is retargeted before the push

Before the push, with the lock released, it MUST retarget the open pull request of each branch it pushes whose base
is a virtual branch of the user (`{prefix}/virtual/…`) to the baseline, as the app's push does (one search of the
pull requests, one `PATCH pulls/<n>` each); `retargeted` MUST list them (`branch`, `number`, `url`, `old_base`,
`new_base`, and `approved_by`, whose approvals GitHub may dismiss).

#### Scenario: A pull request into a parent's virtual branch
- **WHEN** a pushed branch has an open pull request into a virtual branch of the user
- **THEN** it is retargeted to the baseline before the push, and `retargeted` lists it
- **AND** so GitHub never compares the pushed commits with a parent's branch the pull request no longer goes into (D3a)
  [@test] crates/bd-remote/src/safe_push_test.rs (`push_retargets_a_pull_request_into_a_virtual_branch_to_the_baseline`)

#### Scenario: A push the remote then refuses
- **WHEN** the remote refuses the push after the retarget
- **THEN** the retarget stays done
  [@test] crates/bd-remote/src/safe_push_test.rs (`push_retargets_before_it_pushes_and_a_refused_push_keeps_the_retarget`)

### Requirement: `brd push`: A pull request into another base stays

A pull request into another base (a release branch) MUST stay.

#### Scenario: A pull request into a release branch
- **WHEN** the open pull request of a pushed branch goes into a release branch
- **THEN** it is not retargeted

### Requirement: `brd push`: A retarget GitHub refuses is a warning and the push goes on

A retarget GitHub refuses, or a listing that fails for another reason than a missing token, MUST be a
`retarget_warnings` line, with the `gh pr edit` that retargets it; the push MUST go on.

#### Scenario: A refused retarget
- **WHEN** GitHub refuses a retarget, or the listing fails for another reason than a missing token
- **THEN** `retarget_warnings` has a line with the `gh pr edit` that retargets it, and the push goes on

### Requirement: `brd push`: `--dry-run` lists the retargets and changes nothing on GitHub

`--dry-run` MUST list the retargets a push would make and change nothing on GitHub. Without a GitHub repository of the
branches or a token, GitHub MUST NOT be asked.

#### Scenario: A dry run
- **WHEN** `brd push --dry-run` runs
- **THEN** it lists the retargets a push would make, and nothing changes on GitHub

#### Scenario: No GitHub repository or token
- **WHEN** the branches have no GitHub repository, or there is no token
- **THEN** GitHub is not asked

### Requirement: `brd push`: A pull request merged into another branch is named

A branch of `branches` whose pull request GitHub merged into another branch than the baseline, a parent's virtual
branch most of all (a child of a stack merged after its parent's pull request had landed), MUST be named in
`merged_elsewhere` (`branch`, `number`, `url`, `base`): the baseline lacks its commits, nothing of it landed, and its
`pull_request` draft opens a new one into the baseline; a merged pull request MUST never be retargeted.

#### Scenario: A child merged into its parent's branch
- **WHEN** the pull request of a child was merged into its parent's virtual branch after the parent's had landed
- **THEN** `merged_elsewhere` names it, its `pull_request` draft opens a new one into the baseline, and it is not retargeted
  [@test] crates/bd-remote/src/safe_push_test.rs (`push_names_a_pull_request_merged_into_another_branch_than_the_baseline`)

### Requirement: `brd push`: A branch whose Safe Push runs is held

On Space, with the Safe Push CLI logged in, it MUST hold the branches the app holds
([Patronus](../../branch/patronus/spec.md#pushing-into-a-branch-whose-safe-push-runs)): a named branch whose Safe Push
runs on it, or on a branch built on it whose commit has commits of it, is `robot_running` (exit 4, nothing pushed),
also with `--alone`, with the robot's page.

#### Scenario: A Safe Push runs on the branch
- **WHEN** a Safe Push runs on a named branch
- **THEN** the push is `robot_running`, exit 4, nothing pushed, with the robot's page, also with `--alone`
  [@test] crates/bd-remote/src/safe_push_test.rs (`push_refuses_a_branch_whose_safe_push_runs_and_restarts_it_when_told`)

#### Scenario: A Safe Push lands the commits of a parent
- **WHEN** a Safe Push runs on a branch built on the named branch and its commit has commits of it
- **THEN** the push is `robot_running`
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_push_of_a_parent_whose_commits_a_running_safe_push_lands_is_refused`)

### Requirement: `brd push`: The refusal of a running Safe Push names why

The refusal MUST name why: for its own robot the branch the push moves ("A Safe Push of auth is running on test-user/virtual/auth: a push moves the branch it took its commit from, so it won't land and fails the Safe Push at the end"), for another robot the commits of the branch it lands, counted, the first three by id, an amended one marked ("A Safe Push of login is running: it lands 2 commits of auth with it (1a2b3c4d, 5e6f7a8b amended since), a push won't land").

#### Scenario: Its own robot
- **WHEN** the Safe Push runs on the named branch itself
- **THEN** the refusal names the branch the push moves

#### Scenario: Another robot
- **WHEN** another branch's Safe Push lands commits of the named branch
- **THEN** the refusal counts them, names the first three by id and marks an amended one

### Requirement: `brd push`: What a robot lands is decided from its commit

What a robot lands MUST be decided from its commit (`robot_spans`, by content after a restack or an amend below it,
no process for a robot at the tip the sync left; the stack as it is now when the repository lacks its commit): a
parent a restack placed below the robot's branch after its start MUST be pushed.

#### Scenario: A parent restacked below a running Safe Push
- **WHEN** a restack placed a parent below the robot's branch after the robot started
- **THEN** the push of the parent goes on
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_push_of_a_parent_restacked_below_a_running_safe_push_goes_on`)
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_push_beside_a_robot_started_before_a_restack`)

#### Scenario: A robot of a temporary ref
- **WHEN** a robot runs on a commit the branch tips do not name
- **THEN** it holds only what its commit lands
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_robot_of_a_temporary_ref_holds_only_what_its_commit_lands`)

### Requirement: `brd push`: An ancestor a Safe Push lands stays as the robot took it

An ancestor taken along that a Safe Push lands MUST stay as the robot took it, and its child MUST be pushed alone,
carrying its commits. A canceling robot and a dry run MUST hold nothing.

#### Scenario: An ancestor a robot lands
- **WHEN** an ancestor that would be taken along is landed by a running Safe Push
- **THEN** it stays as the robot took it, and the child is pushed alone, carrying its commits

#### Scenario: A canceling robot, a dry run
- **WHEN** the robot is canceling, or the push is a dry run
- **THEN** nothing is held

### Requirement: `brd push`: `--restart-running` restarts the Safe Push on the pushed commit

`--restart-running` MUST cancel the Safe Push of a named branch after the lock, push the branch alone, the commits of
the ancestors that robot lands with it, and restart it on the pushed commit (`restarted`), as the app's restart that
pushes first; its children still go with `--with-children`.

#### Scenario: A restart when told
- **WHEN** `brd push --restart-running` names a branch whose Safe Push runs
- **THEN** it cancels the Safe Push, pushes the branch alone and restarts it on the pushed commit, `restarted`
  [@test] crates/bd-remote/src/safe_push_test.rs (`push_refuses_a_branch_whose_safe_push_runs_and_restarts_it_when_told`)

### Requirement: `brd push`: One robots request after the sync, nothing held off Space

There MUST be one robots request after the sync; off Space, without a token or an answer of Patronus nothing MUST be
held. `brd safe-push` pushes without the check: it cancels the robot it replaces itself.

#### Scenario: Off Space
- **WHEN** the repository is off Space, or there is no token or no answer of Patronus
- **THEN** nothing is held

#### Scenario: `brd safe-push`
- **WHEN** `brd safe-push` pushes a branch whose Safe Push runs
- **THEN** it pushes without the check and cancels the robot it replaces itself

### Requirement: `brd push`: A running Safe Push of the whole stack holds every branch

A running Safe Push of the whole stack MUST hold every branch of the stack its commit has, and `--with-children` of
its root MUST leave its stack branch as it is, a `stack_warnings` line: a push into it fails the robot at the end.

#### Scenario: The whole stack lands in one robot
- **WHEN** a Safe Push of the whole auth stack runs
- **THEN** the push of a branch of the stack its commit has is refused: "A Safe Push of the whole auth stack is running: it lands 1 commit of login with it (1a2b3c4d)"
  [@test] crates/bd-remote/src/safe_push_test.rs (`the_whole_stack_lands_in_one_robot_from_its_stack_branch`)

#### Scenario: `--with-children` of its root
- **WHEN** `brd push --with-children` names the root of the stack
- **THEN** its stack branch stays as it is, a `stack_warnings` line
  [@test] crates/bd-remote/src/safe_push_test.rs (`push_with_children_leaves_the_stack_branch_of_a_running_whole_stack_safe_push`)

### Requirement: `brd push`: A branch a merged Safe Push landed whole is left out

A named branch a merged Safe Push landed whole (`landed_by` of `brd status` without `after`), and with
`--with-children` one built on a named one, MUST be left out of the push, as the app's Push with Children leaves it,
and the others pushed: `skipped` names each with `code` `landed`, the `message` and `landed_by` as `brd status` has it.

#### Scenario: A named branch that landed whole
- **WHEN** a merged Safe Push landed a named branch whole
- **THEN** it is left out, the others are pushed, and `skipped` names it with `code` `landed` and the `message` ("login merged into master with its Safe Push (<robot>): nothing of it is left to push", "auth landed in master with the Safe Push of login (<robot>): …")
  [@test] crates/bd-remote/src/landed_test.rs (`push_leaves_out_a_named_branch_a_merged_safe_push_landed_whole_and_pushes_the_others`)

#### Scenario: A child that landed
- **WHEN** `--with-children` covers a branch built on a named one that a merged Safe Push landed whole
- **THEN** it is left out as the app's Push with Children leaves it
  [@test] crates/bd-remote/src/landed_test.rs (`push_with_children_leaves_out_what_merged_safe_pushes_landed_and_refuses_once_nothing_else_is_left`)

### Requirement: `brd push`: The push is `landed` only when nothing else is left

The push MUST be refused as `landed` (exit 4, nothing pushed, before any running robot) only when nothing else asked
for is left, the stack branch of `--with-children` included; the error MUST name the named branches.

#### Scenario: Nothing else is left
- **WHEN** everything the push asks for landed whole, the stack branch of `--with-children` included
- **THEN** the push is refused as `landed`, exit 4, before any running robot, and the error names the named branches
  [@test] crates/bd-remote/src/landed_test.rs (`push_and_safe_push_refuse_a_branch_a_merged_safe_push_landed_whole`, `push_with_children_leaves_out_what_merged_safe_pushes_landed_and_refuses_once_nothing_else_is_left`)

### Requirement: `brd push`: A landed ancestor stays and commits after the landing push

An ancestor taken along that landed MUST stay as it is. A branch with commits added after the landed ones MUST push
them.

#### Scenario: A landed ancestor
- **WHEN** an ancestor that would be taken along landed
- **THEN** it stays as it is

#### Scenario: Commits after the landed ones
- **WHEN** a branch has commits added after the landed ones
- **THEN** the push pushes them

### Requirement: `brd safe-push`: It starts a Safe Push of the virtual branch as the app does

`brd safe-push <branch>` MUST start a Patronus Safe Push of the virtual branch as the app's start does (the same
`domain_policy::land::decide`, [Patronus](../../branch/patronus/spec.md)): sync, then run the start through the engine
of the app (`operations::land_execute::execute`, [Push and land](../../../../docs/design/git-operations.md#push-and-land)),
the robot started on `refs/heads/<prefix>/virtual/<branch>`, never repeated.

#### Scenario: The start runs through the engine of the app
- **WHEN** `brd safe-push <branch>` starts a Safe Push
- **THEN** it syncs, then runs the retargets, the cancel of the Safe Push it replaces, the push of the branches it
  pushes with their commits frozen under the repository lock and the network push after it
- **AND** then the robot is started on `refs/heads/<prefix>/virtual/<branch>`, never repeated
  [@test] crates/operations/src/land_execute_test.rs (`a_start_cancels_the_robot_it_replaces_pushes_and_starts_in_the_order_of_its_plan`, `the_first_effect_that_fails_stops_the_start_and_the_ones_before_it_stay_done`)
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_safe_push_start`)
  [@test] crates/bd-remote/src/safe_push_test.rs (`missing_approvals_refuse_the_safe_push_unless_told_to_start_anyway`)

### Requirement: `brd safe-push`: `--patronus-dry-run` starts a Dry Run

With `--patronus-dry-run` it MUST start a Dry Run; `--dry-run` starts none, since it means "write nothing" for every
other command.

#### Scenario: A Dry Run is started by its own flag
- **WHEN** `brd safe-push <branch> --patronus-dry-run` runs
- **THEN** it starts a Dry Run of the virtual branch
- **AND** `--dry-run` would start no Dry Run but write nothing

### Requirement: `brd safe-push`: It holds the repository lock only for the witness and the freeze

It MUST hold the repository lock only for the witness of its refs and the freeze of its push: the sync (as
`brd sync`), the reads of the remote, the trial, the check against the target, the retargets, the cancel and the
network push MUST run without it, so they keep no other `brd` writer waiting. `--dry-run` MUST take no lock.

#### Scenario: Only the witness and the freeze run under the lock
- **WHEN** a Safe Push starts
- **THEN** the repository lock is held only for the witness of its refs and the freeze of its push
- **AND** the sync, the reads of the remote, the trial, the check against the target, the retargets, the cancel and
  the network push run without it
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_safe_push_locks_only_the_witness_and_the_freeze`)

#### Scenario: A dry run takes no lock
- **WHEN** `brd safe-push <branch> --dry-run` runs
- **THEN** it takes no lock

### Requirement: `brd safe-push`: `--dry-run` prints the object of the start and starts nothing

`--dry-run` MUST print the same object and push or start nothing, with the `refusals` a start would fail with.

#### Scenario: The check of a start
- **WHEN** `brd safe-push <branch> --dry-run` runs
- **THEN** it prints the object a start prints, with the `refusals` a start would fail with
- **AND** it pushes and starts nothing
  [@test] crates/bd-remote/src/safe_push_test.rs (`check_names_the_profiles_and_the_review_and_starts_nothing`)

### Requirement: `brd safe-push`: A rewrite before the witness fails the start as `plan_changed`

A rewrite of another session between the sync and the witness (a ref the plan moves or leases moved) MUST fail the
start as `plan_changed` (exit 3, `retry: after_reread`), nothing canceled, pushed or started.

#### Scenario: A ref the plan moves moved before the witness
- **GIVEN** another session rewrote a ref the plan moves or leases between the sync and the witness
- **WHEN** the start reaches the witness
- **THEN** it fails as `plan_changed` (exit 3, `retry: after_reread`)
- **AND** nothing is canceled, pushed or started
  [@test] crates/operations/src/land_execute_test.rs (`a_start_refused_before_its_first_effect_cancels_pushes_and_starts_nothing`)

### Requirement: `brd safe-push`: A rewrite while the push sends skips the start as `plan_changed`

A rewrite of another session while the push sends, after the lock, MUST skip the start as `plan_changed`, naming what
was pushed before it, with `brd safe-push <branch>` as the next step; the push sends the commits it froze.

#### Scenario: Another session rewrites while the push sends
- **GIVEN** another session rewrites a ref the plan moves while the push sends, after the lock
- **WHEN** the push ends
- **THEN** the push has sent the commits it froze
- **AND** the start is skipped as `plan_changed`, naming what was pushed before it, with `brd safe-push <branch>` as
  the next step
  [@test] crates/operations/src/land_execute_test.rs (`a_rewrite_while_the_push_sends_skips_the_start_and_the_outcome_tells_what_was_done`)

### Requirement: `brd safe-push`: `--dry-run` prints the next step the app's row shows

`--dry-run` MUST print `next_step`, the step the app's row shows of the subject (`domain_policy::next_step`, as
`brd status` prints it) over what the check read; a step that needs a fact the check didn't read MUST wait for it.

#### Scenario: The next step over what the check read
- **WHEN** `brd safe-push <branch> --dry-run` runs
- **THEN** `next_step` is the step of the app's row over the robots Patronus listed, the problems of the failed robot
  of the subject (whether it failed for the approvals, as the session reads them), the pull requests, the readiness
  when the start waits for it, and the landing gate of the start
  [@test] crates/bd-remote/src/safe_push_test.rs (`check_names_the_profiles_and_the_review_and_starts_nothing`)
  [@test] crates/bd-remote/src/scenario_test.rs (`the_next_step_row_cases_answer_alike_in_the_app_and_bd`)

#### Scenario: A fact the check didn't read
- **WHEN** the step needs a fact the check didn't read (the readiness of a Dry Run)
- **THEN** the step waits for it

### Requirement: `brd safe-push`: The rule of the app decides the start

The start MUST be decided by the rule of the app, `domain_policy::land::decide`, over the facts `brd` read: what the
table alone refuses (not synced, parked) MUST fail the command before Patronus or GitHub is asked, and the review and
the rules of the target MUST be read only when the decision waits for them.

#### Scenario: The table refuses before anything is asked
- **WHEN** the table alone refuses the start (not synced, parked)
- **THEN** the command fails before Patronus or GitHub is asked
  [@test] crates/domain-policy/src/land_test.rs (`the_table_refuses_first_then_the_target_then_a_push_that_drops_remote_commits_unless_allowed`)

#### Scenario: The review and the rules are read only when the decision waits for them
- **WHEN** the decision does not wait for the review or the rules of the target
- **THEN** neither is read
  [@test] crates/domain-policy/src/land_test.rs (`every_fact_without_an_answer_waits_or_confirms_and_never_refuses`)
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_safe_push_check`)

#### Scenario: The app and `brd` decide alike
- **WHEN** the same facts reach the app's start and `brd safe-push`
- **THEN** both decide the same mode and route
  [@test] crates/bd-remote/src/scenario_test.rs (`the_safe_push_mode_and_route_cases_answer_alike_in_the_app_and_bd`)

### Requirement: `brd safe-push`: The refusals and the effects are those of the decision

The `refusals` MUST be the blockers of the decision in their order, then each confirmation a flag of `brd` accepts
that the start lacks, named with its flag; its effects MUST be what the start does, in order (the retargets, the
cancel, the push, the start from a pull request or through the API with the profile).

#### Scenario: Blockers, then the confirmations of a flag
- **WHEN** the decision has blockers and lacks confirmations
- **THEN** the `refusals` are its blockers in their order, then each confirmation the start lacks, named with its
  flag: the cancel of the Safe Push it replaces, `--cancel-running`; a review Patronus couldn't answer,
  `--allow-unknown-review`; rules GitHub didn't answer, `--allow-unknown-rules`
  [@test] crates/domain-policy/src/land_test.rs (`bd_accepts_by_an_allowance_only_the_cancel_an_unknown_review_and_unknown_rules`, `the_landing_gate_refuses_unless_allowed_then_a_conflict_with_the_target`)

#### Scenario: The effects in order
- **WHEN** nothing is in the way of the start
- **THEN** its effects are the retargets, the cancel, the push, the start from a pull request or through the API with
  the profile, in this order
  [@test] crates/domain-policy/src/land_test.rs (`a_start_with_nothing_in_its_way_pushes_what_it_lands_root_first_then_starts`)

### Requirement: `brd safe-push`: The sync of the run and one read of `origin` give the facts

The target branch, the stack and the tips MUST come from the sync of the same run, and the Space and the GitHub
repository from one read of the push URL of `origin`, shared with the push.

#### Scenario: One sync and one read of the push URL
- **WHEN** `brd safe-push` checks a start
- **THEN** the target branch, the stack and the tips come from the sync of the same run
- **AND** the Space and the GitHub repository come from one read of the push URL of `origin`, shared with the push
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_safe_push_check`)

### Requirement: `brd safe-push`: The push is decided by content

The push MUST be decided by content, as the app's start decides it (the `Push` effect of
`domain_policy::land::decide`): `push_needed` only when the sync's remote status of the branch or of a branch it is
built on has work for the remote (`unpushed`, `behind`, `amended`, `reworded`, `restacked`, `landed`, or no remote
branch).

#### Scenario: Work for the remote
- **WHEN** the remote status of the branch or of a branch it is built on is `unpushed`, `behind`, `amended`,
  `reworded`, `restacked`, `landed`, or it has no remote branch
- **THEN** `push_needed` is set
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_branch_reworded_since_its_push_is_pushed_before_the_start`)

### Requirement: `brd safe-push`: The start pushes the branches of its push effect and no other

The start MUST push the branches of that effect and no other, each alone (`domain_policy::push::start_push`: the
ancestors with work up to the nearest one a running Safe Push holds, past one that landed), and `remotes` MUST name only
theirs: the remote branch of a held ancestor stays where its robot took it.

#### Scenario: An ancestor a running Safe Push holds
- **GIVEN** an ancestor of the branch is held by a running Safe Push
- **WHEN** the start pushes
- **THEN** it pushes the ancestors with work up to the nearest held one, past one that landed, each alone
- **AND** `remotes` names only theirs, and the remote branch of the held ancestor stays where its robot took it
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_start_pushes_what_its_decision_pushes_and_leaves_an_ancestor_a_running_safe_push_holds`)
  [@test] crates/domain-policy/src/land_test.rs (`a_safe_push_replaces_its_own_older_robot_a_dry_run_is_refused_its_push_and_the_push_stops_at_a_held_ancestor`)

### Requirement: `brd safe-push`: A branch only rebuilt on a newer baseline is not pushed

A branch whose remote branch has the same commits, only rebuilt since on a newer baseline, MUST NOT be pushed (no
`pushed`); one reworded since MUST be, as the robot would land the old messages: the robot starts from the remote
branch, and `--create-pull-request` opens the pull request of the remote branch as it is.

#### Scenario: Rebuilt on a newer baseline
- **GIVEN** the remote branch has the same commits, only rebuilt since on a newer baseline
- **WHEN** the Safe Push starts or `--create-pull-request` runs
- **THEN** nothing is pushed (no `pushed`)
- **AND** the robot starts from the remote branch, and the pull request of the remote branch is opened as it is
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_branch_only_rebuilt_on_a_newer_baseline_starts_from_its_remote_branch_without_a_push`, `the_pull_request_of_a_branch_only_rebuilt_on_a_newer_baseline_is_opened_without_a_push`)

#### Scenario: Reworded since its push
- **GIVEN** a commit of the branch was reworded since its push
- **WHEN** the Safe Push starts
- **THEN** the branch is pushed before the start
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_branch_reworded_since_its_push_is_pushed_before_the_start`)

### Requirement: `brd safe-push`: A robot running on the remote branch refuses the start

A robot running on the remote branch a start without a push starts from (a branch only rebuilt on a newer baseline)
MUST refuse the start as one on the last commit does.

#### Scenario: A robot on the remote branch the start would start from
- **GIVEN** a robot runs on the remote branch the robot would start from
- **WHEN** the Safe Push starts
- **THEN** it is refused as for a robot on the last commit
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_robot_on_the_last_commit_refuses_and_one_on_an_older_commit_is_canceled_when_told`)

### Requirement: `brd safe-push`: A Safe Push replaces a running one of an older commit when told

A running Safe Push of an older commit MUST be replaced as the app's start replaces it once its popover said so
(`replaces`, "Cancels the Safe Push of auth running on 1a2b3c4."): with nothing to push, a Safe Push start MUST refuse
with `robot_running` ("the start would run beside it") unless `--cancel-running`, which cancels it before the start
(`canceled`); a dry run and `--create-pull-request` leave it.

#### Scenario: An older commit's Safe Push runs
- **GIVEN** a Safe Push of an older commit of the branch runs
- **WHEN** the Safe Push starts with nothing to push, without `--cancel-running`
- **THEN** it refuses with `robot_running` ("the start would run beside it"), the `replaces` saying "Cancels the Safe
  Push of auth running on 1a2b3c4."
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_robot_on_the_last_commit_refuses_and_one_on_an_older_commit_is_canceled_when_told`)

#### Scenario: Told to cancel it
- **WHEN** the Safe Push starts with `--cancel-running`
- **THEN** the running one is canceled before the start (`canceled`)
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_robot_on_the_last_commit_refuses_and_one_on_an_older_commit_is_canceled_when_told`)
  [@test] crates/domain-policy/src/land_test.rs (`a_safe_push_replaces_its_own_older_robot_a_dry_run_is_refused_its_push_and_the_push_stops_at_a_held_ancestor`)

#### Scenario: A dry run or a pull request leaves it
- **WHEN** a dry run or `--create-pull-request` runs
- **THEN** the running Safe Push is left as it is

### Requirement: `brd safe-push`: A Dry Run replaces no Safe Push

A Dry Run MUST replace no Safe Push: its push into the branch of a running Safe Push MUST be refused (`robot_running`),
`--cancel-running` or not ([Patronus](../../branch/patronus/spec.md#pushing-into-a-branch-whose-safe-push-runs)).

#### Scenario: A Dry Run whose push goes into a running Safe Push
- **GIVEN** a Safe Push of the branch runs
- **WHEN** a Dry Run would push into its branch, with `--cancel-running` or without
- **THEN** it is refused with `robot_running`
  [@test] crates/domain-policy/src/land_test.rs (`a_safe_push_replaces_its_own_older_robot_a_dry_run_is_refused_its_push_and_the_push_stops_at_a_held_ancestor`)

### Requirement: `brd safe-push`: `remotes` names what the push replaces and drops

`remotes` MUST name each existing remote branch the start pushes with its remote state, as `brd sync` has it, so
`--dry-run` says beforehand what the push replaces (`amended`, `reworded`, `restacked`, `landed`) and drops.

#### Scenario: The remote state of each pushed remote branch
- **WHEN** `brd safe-push <branch> --dry-run` runs and the start would push existing remote branches
- **THEN** `remotes` names each with its remote state, as `brd sync` has it
  [@test] crates/bd-remote/src/safe_push_test.rs (`check_counts_the_remote_commits_the_baseline_landed_and_refuses_a_drop`, `a_branch_reworded_since_its_push_is_pushed_before_the_start`)

### Requirement: `brd safe-push`: A push that would drop commits refuses as `push_refused`

A branch whose push would drop commits (`behind`) MUST refuse the check and the start with `push_refused` and the
`brd push` that drops them after the user approves; the start's error carries the branch in `error.branches`, nothing
pushed or started. `--force`, once the user approved, MUST start with the drop pushed away.

#### Scenario: Remote commits the push would drop
- **GIVEN** the remote branch has commits the push would drop (`behind`)
- **WHEN** the check or the start runs
- **THEN** it refuses with `push_refused` and the `brd push` that drops them after the user approves
- **AND** the start's error carries the branch in `error.branches`, nothing pushed or started
  [@test] crates/bd-remote/src/safe_push_test.rs (`check_counts_the_remote_commits_the_baseline_landed_and_refuses_a_drop`)

#### Scenario: The user approved the drop
- **WHEN** the start runs with `--force`, once the user approved
- **THEN** the push replaces the commits of someone else, then the robot starts
  [@test] crates/bd-remote/src/safe_push_test.rs (`check_counts_the_remote_commits_the_baseline_landed_and_refuses_a_drop`)

### Requirement: `brd safe-push`: The profiles are the target's for the mode, never emergency ones

The profiles MUST be those of the target that allow the mode, never the emergency ones: `--profile emergency` is
`invalid_arguments`, as only the app's Safe Push without review offers one, in red.

#### Scenario: An emergency profile is asked for
- **WHEN** `brd safe-push <branch> --profile emergency` runs
- **THEN** it is `invalid_arguments`
  [@test] crates/bd-remote/src/scenario_test.rs (`the_safe_push_profile_cases_answer_alike_in_the_app_and_bd`)

### Requirement: `brd safe-push`: `--profile` takes an id, a name or `none`

`--profile` MUST take an id or a name (any case) or `none`, another value is `invalid_arguments` that lists them.
Without it the profile of the last robot of the branch MUST be the default.

#### Scenario: A profile by its name
- **WHEN** `--profile` names a profile by its name, in any case
- **THEN** that profile is used
- **AND** another value is `invalid_arguments` that lists the profiles
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_dry_run_takes_a_profile_by_name_and_checks_no_review`)

#### Scenario: No `--profile`
- **WHEN** no `--profile` is given
- **THEN** the profile of the last robot of the branch is the default
- **AND** `--profile none` drops it
  [@test] crates/bd-remote/src/safe_push_test.rs (`the_profile_of_the_last_robot_is_the_default_and_none_drops_it`)
  [@test] crates/bd-remote/src/scenario_test.rs (`the_safe_push_profile_cases_answer_alike_in_the_app_and_bd`)

### Requirement: `brd safe-push`: A Safe Push checks the code-owner review first

Before a Safe Push (never a dry run) it MUST check the code-owner review as the app's start does, with the pull
request the robot starts from, always asked again ([Patronus](../../branch/patronus/spec.md#code-owner-review)).

#### Scenario: A dry run checks no review
- **WHEN** a dry run starts
- **THEN** no review is checked
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_dry_run_takes_a_profile_by_name_and_checks_no_review`)

#### Scenario: A Safe Push asks for the review again
- **WHEN** a Safe Push starts
- **THEN** the review is asked again with the pull request the robot starts from
  [@test] crates/bd-remote/src/scenario_test.rs (`the_safe_push_review_cases_answer_alike_in_the_app_and_bd`)

### Requirement: `brd safe-push`: Missing approvals refuse the Safe Push with `approvals_missing`

A `missing` readiness MUST refuse the Safe Push with `approvals_missing` (exit 4), nothing pushed or started, `error.review`
naming the groups, their reviewers and files, as the app refuses the start (`domain_policy::land::decide`: "Patronus
checks the approvals when the robot starts and fails without them"), and its next step disables Safe Push.

#### Scenario: The readiness is `missing`
- **WHEN** a Safe Push starts and the readiness is `missing`
- **THEN** it is refused with `approvals_missing` (exit 4), nothing pushed or started
- **AND** `error.review` names the groups, their reviewers and files
  [@test] crates/bd-remote/src/safe_push_test.rs (`missing_approvals_refuse_the_safe_push_unless_told_to_start_anyway`)
  [@test] crates/domain-policy/src/land_test.rs (`missing_approvals_refuse_unless_skipped_and_approvals_judged_at_the_start_confirm`)

### Requirement: `brd safe-push`: A profile that skips the review starts without the approvals

A start through the API (no open pull request to start from) whose merge profile is known to skip the review (the
options of the last robot say `patronus.review.approval.check.skip`, or the list names it with the new API) MUST start
without them, as the app's start does (`skipsApprovals`).

#### Scenario: The merge profile skips the review
- **GIVEN** no open pull request to start from, and a merge profile known to skip the review
- **WHEN** the Safe Push starts
- **THEN** it starts through the API without the approvals
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_profile_that_skips_the_review_starts_without_the_approvals_through_the_api`)
  [@test] crates/domain-policy/src/land_test.rs (`the_default_profile_is_the_last_robots_while_offered_and_a_profile_is_confirmed`)

### Requirement: `brd safe-push`: `--without-approvals` starts anyway on either route

`--without-approvals` MUST start anyway on either route, the counterpart of the app's Safe Push without review: with a
`--profile` that skips the review (`--profile lsc`).

#### Scenario: The user says to start without the approvals
- **GIVEN** the readiness is `missing`
- **WHEN** the Safe Push starts with `--without-approvals` and a `--profile` that skips the review (`--profile lsc`)
- **THEN** it starts, from a pull request or through the API
  [@test] crates/bd-remote/src/safe_push_test.rs (`missing_approvals_refuse_the_safe_push_unless_told_to_start_anyway`)

### Requirement: `brd safe-push`: A review Patronus cannot answer refuses as `review_unknown`

A pre-check that failed MUST be no answer, never `approved` and never a failure of the command: it MUST refuse
the start with `review_unknown` (exit 4, also in `refusals`), the `review` saying `unavailable`, unless
`--allow-unknown-review`, which starts and leaves the approvals to the robot.

#### Scenario: The pre-check failed
- **WHEN** the pre-check of the review fails
- **THEN** the start is refused with `review_unknown` (exit 4, also in `refusals`), the `review` saying `unavailable`
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_review_patronus_cannot_answer_refuses_the_safe_push_unless_allowed`)

#### Scenario: Allowed by the user
- **WHEN** the start runs with `--allow-unknown-review`
- **THEN** it starts and leaves the approvals to the robot
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_review_patronus_cannot_answer_refuses_the_safe_push_unless_allowed`)

### Requirement: `brd safe-push`: The `review` says the readiness and who approved

The `review` MUST say `readiness` (`not_required`, `approved`, `missing`, `unknown`, `unavailable`) with its
`readiness_reason`, `skipped_by` (`profile` or `without_approvals`) when the approvals don't hold the start back, the
code owners asked who haven't approved (`waiting_for`), the approvals Patronus doesn't count (`not_counted`), and per
group `approved` and `needed`, per reviewer `approval_state`, when Patronus evaluated the pull request.

#### Scenario: Patronus evaluated the pull request
- **WHEN** Patronus evaluated the pull request
- **THEN** the `review` has `readiness`, `readiness_reason`, `waiting_for`, `not_counted`, per group `approved` and
  `needed`, and per reviewer `approval_state`
  [@test] crates/bd-remote/src/safe_push_test.rs (`missing_approvals_refuse_the_safe_push_unless_told_to_start_anyway`)

#### Scenario: The approvals don't hold the start back
- **WHEN** a profile or `--without-approvals` lets the start go without the approvals
- **THEN** `skipped_by` says `profile` or `without_approvals`
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_profile_that_skips_the_review_starts_without_the_approvals_through_the_api`)

### Requirement: `brd safe-push`: A push into an approved pull request reads the rules of the target

A Safe Push that pushes new commits into the open pull request the robot starts from, which someone approved, MUST read
the GitHub rules of the target first, as the app's start does
([Patronus](../../branch/patronus/spec.md#starting-a-safe-push-or-a-dry-run)): one `GET rules/branches/<target>`
(`read_branch_rules`), never for a dry run, `--create-pull-request`, a start without a push or a pull request nobody
approved.

#### Scenario: Only a push into an approved pull request reads the rules
- **WHEN** a Safe Push pushes new commits into the open pull request it starts from, which someone approved
- **THEN** it reads the rules of the target with one `GET rules/branches/<target>` (`read_branch_rules`) first
- **AND** a dry run, `--create-pull-request`, a start without a push and one into a pull request nobody approved read
  none
  [@test] crates/bd-remote/src/safe_push_test.rs (`the_rules_are_read_only_for_a_push_into_an_approved_pull_request`)
  [@test] crates/domain-policy/src/land_test.rs (`the_rules_of_a_push_into_an_approved_pull_request_refuse_confirm_or_wait`)

### Requirement: `brd safe-push`: Rules that dismiss counted approvals refuse the Safe Push

Rules that dismiss stale reviews on push with the readiness `approved` MUST refuse it with `push_dismisses_approvals`
(exit 4, nothing pushed or started), whose message says to push first (`brd push <branch>`), then to ask for the review
again (`brd request-review <branch>`); with another readiness they MUST let it start, the dismissed approvals said in
`rules.message`.

#### Scenario: The push would dismiss the approvals Patronus counts
- **GIVEN** the rules of the target dismiss stale reviews on push and the readiness is `approved`
- **WHEN** the Safe Push starts
- **THEN** it is refused with `push_dismisses_approvals` (exit 4), nothing pushed or started
- **AND** the message says to push first (`brd push <branch>`), then to ask for the review again
  (`brd request-review <branch>`)
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_push_that_dismisses_the_approvals_patronus_counts_refuses_the_safe_push`)
  [@test] crates/domain-policy/src/land_test.rs (`the_rules_of_a_push_into_an_approved_pull_request_refuse_confirm_or_wait`)

#### Scenario: The push would dismiss approvals Patronus doesn't count
- **GIVEN** the rules dismiss stale reviews on push and the readiness is another than `approved`
- **WHEN** the Safe Push starts
- **THEN** it starts, and `rules.message` says which approvals the push dismisses
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_push_that_dismisses_approvals_patronus_does_not_count_starts_and_says_so`)
  [@test] crates/bd-remote/src/scenario_test.rs (`the_discrepancy_cases_name_every_id_as_the_register_does`, `the_safe_push_review_cases_answer_alike_in_the_app_and_bd`)

### Requirement: `brd safe-push`: Rules GitHub doesn't answer refuse as `rules_unknown`

Rules GitHub doesn't answer (no token, no access, an error) MUST never read as "keeps the approvals": `rules_unknown`
(exit 4) MUST refuse the start unless `--allow-unknown-rules`, which starts with `rules.message` saying what is not
known.

#### Scenario: GitHub doesn't answer the rules
- **WHEN** GitHub doesn't answer the rules of the target (no token, no access, an error)
- **THEN** the start is refused with `rules_unknown` (exit 4)
  [@test] crates/bd-remote/src/safe_push_test.rs (`rules_github_does_not_answer_refuse_the_safe_push_unless_allowed`)

#### Scenario: Allowed by the user
- **WHEN** the start runs with `--allow-unknown-rules`
- **THEN** it starts, `rules.message` saying what is not known
  [@test] crates/bd-remote/src/safe_push_test.rs (`rules_github_does_not_answer_refuse_the_safe_push_unless_allowed`)

### Requirement: `brd safe-push`: `rules` says whether a push dismisses stale reviews

`rules` MUST carry `dismiss_stale_reviews_on_push` (`true`, `false`, `null` when GitHub didn't answer) and `message`,
and be absent when the rules were not read; `--dry-run` MUST list the refusal in `refusals`.

#### Scenario: The rules were read
- **WHEN** the start read the rules of the target
- **THEN** `rules` carries `dismiss_stale_reviews_on_push` (`true`, `false`, `null` when GitHub didn't answer) and
  `message`
- **AND** `--dry-run` lists the refusal of the rules in `refusals`
  [@test] crates/bd-remote/src/safe_push_test.rs (`rules_github_does_not_answer_refuse_the_safe_push_unless_allowed`)

#### Scenario: The rules were not read
- **WHEN** the start read no rules
- **THEN** there is no `rules`
  [@test] crates/bd-remote/src/safe_push_test.rs (`the_rules_are_read_only_for_a_push_into_an_approved_pull_request`)

### Requirement: `brd safe-push`: `--create-pull-request` opens the GitHub pull request of the branch

The approvals MUST be collected in the GitHub pull request of the branch, not in a Space merge request (deprecated):
`--create-pull-request` MUST push the branch and open it through the GitHub API as the `gh pr create` that `brd push`
prints would (with its `title` and `body`), as the app's dialog creates it
([Pull Requests](../../branch/pull-requests/spec.md#creating-a-pull-request)), without a start.

#### Scenario: The pull request is opened
- **WHEN** `brd safe-push <branch> --create-pull-request` runs
- **THEN** it pushes the branch and opens its pull request through the GitHub API, with the `title` and `body` of the
  `gh pr create` that `brd push` prints, and starts no robot
- **AND** the pull request is assigned to the user (`--no-assign` leaves it unassigned) and asks the `--reviewer <login>`
  logins, the author left out; a reviewer GitHub refuses is in `pull_request.warnings`
  [@test] crates/bd-remote/src/safe_push_test.rs (`the_pull_request_it_opens_asks_the_suggested_code_owner_by_the_github_login_patronus_names`)

#### Scenario: No GitHub repository
- **WHEN** `--create-pull-request` runs without a GitHub repository of the branches
- **THEN** it is `invalid_arguments`

### Requirement: `brd safe-push`: An existing pull request is found instead of opened

A pull request that already exists MUST be found instead: one into the target MUST be left as it is, one into a
virtual branch retargeted to the target (`retargeted`), and one into another base MUST refuse
`pull_request_other_base`, as its approvals would count for no Safe Push into the target.

#### Scenario: A pull request into the target exists
- **WHEN** `--create-pull-request` runs and a pull request of the branch into the target exists
- **THEN** it is found and left as it is
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_pull_request_that_already_exists_is_found_instead`)

#### Scenario: A pull request into a virtual branch or another base exists
- **WHEN** `--create-pull-request` runs and the pull request of the branch goes into a virtual branch or another base
- **THEN** one into a virtual branch is retargeted to the target (`retargeted`)
- **AND** one into another base refuses `pull_request_other_base`
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_pull_request_that_already_exists_is_found_instead`, `create_pull_request_retargets_the_pull_request_of_a_virtual_branch_and_refuses_one_into_another_base`)

### Requirement: `brd safe-push`: The reviewers of the `review` carry their GitHub login

The `review` reviewers MUST carry their `github_login` when Patronus names it, for `--reviewer`.

#### Scenario: Patronus names the GitHub login of a code owner
- **WHEN** Patronus names the GitHub login of a reviewer
- **THEN** the reviewer in `review` carries it as `github_login`, which `--reviewer` takes
  [@test] crates/bd-remote/src/safe_push_test.rs (`the_pull_request_it_opens_asks_the_suggested_code_owner_by_the_github_login_patronus_names`)

### Requirement: `brd safe-push`: The GitHub token is the first one found

The token SHALL be the first of `GH_TOKEN` (`GITHUB_TOKEN`), `gh auth token`, the app's sign-in (its token file) and
the git credentials of github.com ([Pull Requests](../../branch/pull-requests/spec.md#the-token)); without one GitHub
accepts, it MUST be `not_logged_in`.

#### Scenario: No token GitHub accepts
- **WHEN** a pull request is needed and no token GitHub accepts is found
- **THEN** the command is `not_logged_in`
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_pull_request_without_a_github_token_is_not_logged_in`)

### Requirement: `brd safe-push`: Patronus judges the approvals of the pull request

An approval no eligible code owner gave (a bot) MUST NOT let the Safe Push start: `approvals_missing` names who it waits
for and the approval it doesn't count, and `review.approved_on_github` MUST name only the approvals that count.

#### Scenario: An approval that doesn't count
- **GIVEN** the pull request has an approval no eligible code owner gave (a bot)
- **WHEN** the Safe Push starts
- **THEN** it is refused with `approvals_missing`, naming who it waits for and the approval it doesn't count
- **AND** `review.approved_on_github` names only the approvals that count
  [@test] crates/bd-remote/src/safe_push_test.rs (`patronus_judges_the_approvals_of_the_pull_request`)

### Requirement: `brd safe-push`: A Patronus that evaluates no pull request lets an approved one start

A Patronus that doesn't evaluate pull requests can't tell: a pull request someone approved MUST then let the Safe Push
start (`readiness` `unknown`), `review.approved_on_github` naming every approver; without any approval,
`approvals_missing` MUST name the pull request.

#### Scenario: Someone approved the pull request
- **GIVEN** Patronus doesn't evaluate pull requests and someone approved the pull request
- **WHEN** the Safe Push starts
- **THEN** it starts from the pull request (`readiness` `unknown`), `review.approved_on_github` naming every approver
  [@test] crates/bd-remote/src/safe_push_test.rs (`the_approvals_count_in_the_pull_request_it_opens_and_the_safe_push_starts_from_it`)

#### Scenario: Nobody approved the pull request
- **GIVEN** Patronus doesn't evaluate pull requests and nobody approved the pull request
- **WHEN** the Safe Push starts
- **THEN** it is refused with `approvals_missing` naming the pull request
  [@test] crates/bd-remote/src/safe_push_test.rs (`missing_approvals_refuse_the_safe_push_unless_told_to_start_anyway`)

### Requirement: `brd safe-push`: A branch a merged Safe Push landed whole is refused as `landed`

A branch a merged Safe Push landed whole MUST refuse the start with `landed` (exit 4, first of the `refusals` of
`--dry-run`); so MUST `--whole-stack` once every branch of the stack landed.

#### Scenario: The branch landed whole
- **GIVEN** a merged Safe Push landed the branch whole
- **WHEN** the Safe Push starts, or `--dry-run` runs
- **THEN** it is refused with `landed` (exit 4, first of the `refusals` of `--dry-run`): "Nothing was pushed or started:
  login merged into master with its Safe Push (<robot>): nothing of it is left to land"
  [@test] crates/bd-remote/src/landed_test.rs (`push_and_safe_push_refuse_a_branch_a_merged_safe_push_landed_whole`)

#### Scenario: Every branch of the whole stack landed
- **GIVEN** every branch of the stack landed
- **WHEN** `brd safe-push <branch> --whole-stack` runs
- **THEN** it is refused with `landed` ("every branch of the whole auth stack landed in master with the Safe Pushes of
  login and session")
  [@test] crates/bd-remote/src/landed_test.rs (`the_whole_stack_lands_what_is_left_of_it_and_refuses_once_every_branch_of_it_landed`)
  [@test] crates/bd-remote/src/scenario_test.rs (`the_stack_and_whole_stack_cases_answer_alike_in_the_app_and_bd`)

### Requirement: `brd safe-push`: `lands` leaves out the branches that landed whole

`lands` MUST leave out the branches merged Safe Pushes landed whole, the branch itself always named: the robot lands
the rest.

#### Scenario: Some branches of what lands landed whole
- **GIVEN** merged Safe Pushes landed some branches of what the start lands whole
- **WHEN** the Safe Push starts or `--dry-run` runs
- **THEN** `lands` leaves them out and always names the branch itself
  [@test] crates/bd-remote/src/landed_test.rs (`the_whole_stack_lands_what_is_left_of_it_and_refuses_once_every_branch_of_it_landed`)

### Requirement: `brd safe-push`: A branch landed otherwise is refused as `landed`

A branch landed otherwise MUST be refused as `landed` too, by the rule of the app
(`domain_policy::landed::landed_state`): the fetched baseline has every commit of it, or GitHub merged its pull request
with the commits it has, the head of the merged pull request of the listing at its tip or its remote branch up to date.

#### Scenario: The baseline has every commit of it
- **GIVEN** the fetched baseline has every commit of the branch
- **WHEN** the Safe Push starts
- **THEN** it is refused as `landed` ("auth is in origin/master: the update archives it")
  [@test] crates/bd-remote/src/scenario_test.rs (`the_next_step_row_cases_answer_alike_in_the_app_and_bd`)

#### Scenario: GitHub merged its pull request
- **GIVEN** GitHub merged the pull request of the branch with the commits it has (the head of the merged pull request of
  the listing at its tip, or its remote branch up to date)
- **WHEN** the Safe Push starts
- **THEN** it is refused as `landed` ("pull request #7 of auth is merged")
  [@test] crates/bd-remote/src/scenario_test.rs (`the_next_step_row_cases_answer_alike_in_the_app_and_bd`)

### Requirement: `brd safe-push`: A robot running on the commits of the branch refuses the start

A robot on the last commit of the branch, or a Safe Push whose commit has commits of it, of a branch of its stack
(decided as `brd push` decides it), MUST refuse the start with `robot_running`, in the words of the app, the commits
named by id; a running Safe Push of an older commit, which the push fails, too, unless `--cancel-running` cancels it
first.

#### Scenario: A robot on the last commit
- **GIVEN** a robot runs on the last commit of the branch
- **WHEN** the Safe Push starts
- **THEN** it is refused with `robot_running`, in the words of the app, the commits named by id
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_robot_on_the_last_commit_refuses_and_one_on_an_older_commit_is_canceled_when_told`)
  [@test] crates/sync-core/src/patronus_start_test.rs (`the_commits_a_robot_lands_are_named_by_their_number_and_ids`)

#### Scenario: A Safe Push whose commit has commits of the branch or of its stack
- **GIVEN** a running Safe Push whose commit has commits of the branch or of a branch of its stack, decided as `brd push`
  decides it
- **WHEN** the Safe Push starts
- **THEN** it is refused with `robot_running`
  [@test] crates/domain-policy/src/robot_match_test.rs (`the_robot_of_a_branch_is_the_running_one_then_the_newest_and_a_temporary_ref_by_its_revision`, `the_robots_match_their_branches_by_the_cases`)
  [@test] crates/repository-model/src/patronus_view_test.rs (`a_running_robot_at_its_tip_lands_the_branches_it_is_built_on`, `the_first_robot_of_the_table_lands_a_branch_and_only_a_running_one_holds_it`)
  [@test] crates/bd-remote/src/scenario_test.rs (`the_landing_cases_answer_alike_in_the_app_and_bd`, `the_stack_and_whole_stack_cases_answer_alike_in_the_app_and_bd`)

#### Scenario: A running Safe Push of an older commit
- **GIVEN** a Safe Push of an older commit runs, which the push fails
- **WHEN** the Safe Push starts without `--cancel-running`
- **THEN** it is refused with `robot_running`
- **AND** with `--cancel-running` that Safe Push is canceled first
  [@test] crates/domain-policy/src/land_test.rs (`a_safe_push_replaces_its_own_older_robot_a_dry_run_is_refused_its_push_and_the_push_stops_at_a_held_ancestor`, `the_whole_stack_says_its_build_first_and_its_own_robot_holds_or_is_replaced`)

### Requirement: `brd safe-push`: A parent restacked below a running Safe Push starts

A parent a restack placed below a running Safe Push after its start MUST start.

#### Scenario: A restack placed a parent below a running Safe Push
- **GIVEN** a restack placed a parent below a running Safe Push after its start
- **WHEN** the Safe Push of the parent starts
- **THEN** it starts, not refused as `robot_running`

### Requirement: `brd safe-push`: A proven dangling reference refuses as `dangling_reference`

A proven dangling reference (as `brd check` reports it) of the branch or a branch it is built on MUST refuse the start
with `dangling_reference` (exit 4), nothing pushed; the trial build that finds it MUST build only what lands (every
branch for a whole stack that forks).

#### Scenario: A proven dangling reference
- **GIVEN** a proven dangling reference of the branch or of a branch it is built on, as `brd check` reports it
- **WHEN** the Safe Push starts
- **THEN** it is refused with `dangling_reference` (exit 4), nothing pushed
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_dangling_reference_refuses_the_safe_push_unless_allowed`)
  [@test] crates/bd-remote/src/scenario_test.rs (`the_landing_cases_answer_alike_in_the_app_and_bd`)

#### Scenario: The trial build of a whole stack that forks
- **WHEN** the start is of a whole stack that forks
- **THEN** the trial build that finds the dangling references builds every branch of it, and otherwise only what lands

### Requirement: `brd safe-push`: The refusal of a dangling reference names the use and the fix

The refusal MUST name the use, `fixed_by` and the fix (of two references both, from three on of the first and
"(2 more: brd check)"); `--dry-run` MUST list it in `refusals`, `--allow-dangling` MUST start anyway, and the possible
ones MUST be `warnings`.

#### Scenario: The message of the refusal
- **WHEN** a dangling reference refuses the start
- **THEN** its message names the use, `fixed_by` and the fix, of two references both, from three on of the first and
  "(2 more: brd check)"
  [@test] crates/bd-core/src/check_summary_test.rs (`a_dangling_reference_message_names_the_kind`, `a_dangling_reference_message_describes_two_whole_and_counts_from_three_on`)

#### Scenario: Listed, allowed, or only possible
- **WHEN** `--dry-run` runs, or the start runs with `--allow-dangling`
- **THEN** `--dry-run` lists the refusal in `refusals`, and `--allow-dangling` starts anyway
- **AND** a possible dangling reference is one of the `warnings`
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_dangling_reference_refuses_the_safe_push_unless_allowed`)

### Requirement: `brd safe-push`: A name a commit outside what lands adds refuses as `missing_declaration`

With `branchdeck.symbols on`, a use by a commit of what lands of a name that a commit outside it adds (a missing
declaration of `brd check`), which no merged Safe Push landed, MUST refuse the start with `missing_declaration`
(exit 4), nothing pushed, in the same trial session (`trial_check::landing_gate`, the rule of the app's start too).

#### Scenario: A name another branch adds
- **GIVEN** `branchdeck.symbols on`, and a commit of what lands uses a name that a commit outside it adds, which no
  merged Safe Push landed
- **WHEN** the Safe Push starts
- **THEN** it is refused with `missing_declaration` (exit 4), nothing pushed
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_name_another_branch_adds_refuses_the_safe_push_unless_allowed`)
  [@test] crates/bd-remote/src/scenario_test.rs (`the_landing_cases_answer_alike_in_the_app_and_bd`)

#### Scenario: One trial session for both checks
- **WHEN** the check of a start runs, of a branch or of the whole stack
- **THEN** the missing declarations come from the same trial session as the dangling references
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_safe_push_check`, `a_safe_push_check_of_the_whole_stack`)

### Requirement: `brd safe-push`: A name the landing stack adds is no missing declaration

A name a branch of the landing stack adds MUST be no missing declaration, and neither MUST a name a sibling of a whole
stack that forks adds.

#### Scenario: A name a sibling adds
- **GIVEN** a sibling branch adds a name the branch uses
- **WHEN** the Safe Push of the branch alone starts, or that of the whole stack
- **THEN** the branch alone is refused with `missing_declaration`, the whole stack is not
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_name_a_sibling_adds_refuses_the_branch_alone_but_not_the_whole_stack`)

### Requirement: `brd safe-push`: The refusal of a missing declaration names the fix

The message MUST name the use, the commit that adds the name with its branch, and the fix: `brd restack <root> <branch>`,
which lands that branch too, or, for a commit unassigned or a restack that would close a cycle, the move of the commit;
"(N more: brd check)" for the others. `--dry-run` MUST list it in `refusals`, and `--allow-missing` MUST start anyway.

#### Scenario: The message, the dry run and the allowance
- **WHEN** a missing declaration refuses the start
- **THEN** the message names the use, the commit that adds the name with its branch, and `brd restack <root> <branch>`,
  or the move of the commit when it is unassigned or the restack would close a cycle, with "(N more: brd check)" for
  the others
- **AND** `--dry-run` lists it in `refusals`, and `--allow-missing` starts anyway
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_name_another_branch_adds_refuses_the_safe_push_unless_allowed`)

### Requirement: `brd safe-push`: It replays what lands onto the newest target before the start

Patronus rebases what a robot lands onto the newest target before any check, and a conflict fails the robot at once. So
before its sync, `brd safe-push` MUST fetch only the target's remote branch (as Update Project does), then replay what
the robot lands onto the tip that the sync read, as a plain `git rebase` replays it, with no resolution by words
(`sync_core::target_conflict`, the check of the app's start too).

#### Scenario: A target that moved without a conflict
- **GIVEN** the target moved after the last sync without a conflicting change
- **WHEN** the Safe Push starts
- **THEN** it fetches only the target's remote branch before its sync, the replay onto the tip is clean, and it starts
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_target_that_moved_without_a_conflict_starts`)
  [@test] crates/sync-core/src/target_conflict_test.rs (`a_target_that_moved_without_a_conflict_is_clean`)
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_safe_push_check`)
  [@test] crates/sync-core/src/process_budget_test.rs (`a_target_conflict_check`)

### Requirement: `brd safe-push`: A conflict with the target refuses as `target_conflict`

A conflict MUST refuse the start with `target_conflict` (exit 4), nothing pushed or started. The error names the commit
by the id of the checked out branch (`error.commits`), the files (`error.paths`), and the commits of the target after
the base of the branch that change these files (hash, author, subject); the message tells the fix: `brd update`
resolves it locally, then start again.

#### Scenario: A target that moved with a conflicting change
- **GIVEN** the target moved after the last sync with a change that conflicts with what the robot lands
- **WHEN** the Safe Push starts
- **THEN** it is refused with `target_conflict` (exit 4), nothing pushed or started
- **AND** the error names the commit (`error.commits`), the files (`error.paths`), the commits of the target that change
  them (hash, author, subject), and `brd update` as the fix before a new start
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_target_that_moved_with_a_conflicting_change_after_the_last_sync_refuses_the_start`)
  [@test] crates/sync-core/src/target_conflict_test.rs (`a_target_that_moved_with_a_conflicting_change_after_the_last_sync_conflicts`)

### Requirement: `brd safe-push`: `target_check` says the result of the check against the target

`--dry-run` MUST list a `target_conflict` in `refusals`, and `target_check` MUST say `result` (`clean`, `conflict`,
`not_run`) and the `target_tip`. A check that can't run MUST hold nothing back, and `--create-pull-request` starts no
robot and checks nothing.

#### Scenario: The check of a dry run
- **WHEN** `brd safe-push <branch> --dry-run` runs
- **THEN** `target_check` says `result` (`clean`, `conflict` or `not_run`) and the `target_tip`
- **AND** a conflict is listed in `refusals`
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_target_that_moved_with_a_conflicting_change_after_the_last_sync_refuses_the_start`)

#### Scenario: A check that can't run
- **WHEN** the check against the target can't run (a branch that does not resolve)
- **THEN** it holds nothing back
  [@test] crates/sync-core/src/target_conflict_test.rs (`a_branch_that_does_not_resolve_is_not_checked`)

#### Scenario: A pull request is only opened
- **WHEN** `--create-pull-request` runs
- **THEN** it starts no robot and checks nothing against the target

### Requirement: `brd safe-push`: A failed fetch of the target checks the last fetched tip

A failed fetch MUST NOT stop the check: it replays onto the last fetched tip, `fetched` is false, and `message` says why.

#### Scenario: The fetch of the target fails
- **WHEN** the fetch of the target's remote branch fails
- **THEN** the check replays onto the last fetched tip, `fetched` is false, and `message` says why
  [@test] crates/sync-core/src/target_conflict_test.rs (`a_failed_fetch_checks_the_last_fetched_target_and_says_so`)

### Requirement: `brd safe-push`: HEAD's branch read for the fetch does not outlive it

HEAD's branch read for the fetch MUST NOT outlive it: a HEAD the user detached while the fetch ran defers the sync after
it (`detached_head`, exit 4), nothing pushed or started. A HEAD switched while it ran to a branch of another baseline
MUST be synced as it is, the fetch, of the old baseline, counting as failed for the new one.

#### Scenario: HEAD detached during the fetch
- **WHEN** the user detaches HEAD while the fetch of the target runs
- **THEN** the sync after it is deferred (`detached_head`, exit 4), nothing pushed or started
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_head_detached_during_the_fetch_of_the_target_defers_the_sync`)
  [@test] crates/bd-cli/src/context_test.rs (`the_head_is_read_again_after_a_call_over_the_network`)

#### Scenario: HEAD switched to a branch of another baseline during the fetch
- **WHEN** HEAD is switched to a branch of another baseline while the fetch runs
- **THEN** that branch is synced as it is, and the check replays onto its last fetched tip
- **AND** `fetched` is false, and `message` names the baseline fetched
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_branch_switched_during_the_fetch_of_the_target_is_checked_as_not_fetched`)

### Requirement: `brd safe-push`: A stacked branch lands with every branch it is built on

A Safe Push of a stacked branch MUST land it with every branch it is built on, in one robot, as the app's start does
([Landing](../../../../docs/design/landing.md)): no refusal and no flag, the parents with commits for the remote pushed
first, the robot started on the branch's own virtual branch, which carries their commits.

#### Scenario: The Safe Push of a stacked branch
- **GIVEN** a branch stacked on parents with commits for the remote
- **WHEN** its Safe Push starts
- **THEN** the parents are pushed first, and the robot is started on the branch's own virtual branch, which carries
  their commits
- **AND** nothing refuses it and no flag is needed
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_stacked_branch_lands_its_parents_with_it`)

### Requirement: `brd safe-push`: `lands` names what the robot takes, root first

`lands` MUST name what the robot takes, root first, ending with the branch (only the branch for a root), in the start,
in `--dry-run` and in a dry run, which checks the same commits.

#### Scenario: A stacked branch and a root
- **WHEN** a start, `--dry-run` or a dry run runs
- **THEN** `lands` names what the robot takes, root first, ending with the branch
- **AND** for a root it names only the branch
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_stacked_branch_lands_its_parents_with_it`, `check_names_the_profiles_and_the_review_and_starts_nothing`)

### Requirement: `brd safe-push`: `--land-stack` is accepted and changes nothing

`--land-stack`, which an older brd needed, MUST be accepted, hidden, and change nothing.

#### Scenario: An older caller passes `--land-stack`
- **WHEN** `brd safe-push <branch> --land-stack` runs
- **THEN** the flag is accepted and the command does what it does without it
- **AND** the help does not show the flag

### Requirement: `brd safe-push`: `--dry-run` of a stacked branch says whether it lands alone

`--dry-run` of a stacked branch MUST add `alone`, the trial build of the branch as a root on the base the sync built
on: `builds` (with `uses`, the names of its stack it uses, and `symbols`, whether names were read) or `needs` (the
conflicting commit and its files, the commits of its ancestors it needs with their branch, and `movable`), with a
`message` naming `brd unstack <branch>` or the move; `unknown` when the trial fails.

#### Scenario: The branch builds alone
- **WHEN** `--dry-run` of a stacked branch runs and the branch builds as a root
- **THEN** `alone` says `builds`, with `uses` and `symbols`, and a `message` naming `brd unstack <branch>`
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_stacked_branch_lands_its_parents_with_it`)

#### Scenario: The branch needs a commit of its parent
- **WHEN** the branch does not build as a root
- **THEN** `alone` says `needs`: the conflicting commit and its files, the commits of its ancestors it needs with their
  branch, and `movable`, with a `message` naming the move
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_stacked_branch_that_needs_a_commit_of_its_parent_to_land_alone_names_it`)

#### Scenario: The trial fails
- **WHEN** the trial build fails
- **THEN** `alone` is `unknown`

### Requirement: `brd safe-push`: The trial of `alone` shares the session and changes nothing

The trial of `alone` MUST share the trial session of the dangling references, and a start MUST run no trial of it. It
MUST never unstack or move anything itself.

#### Scenario: One trial session, none at the start
- **WHEN** `--dry-run` of a stacked branch runs
- **THEN** `alone` comes from the trial session of the dangling references, and nothing is unstacked or moved
- **AND** a start runs no trial of it
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_safe_push_check_of_a_stacked_branch`)

### Requirement: `brd safe-push`: `--whole-stack` lands the whole stack from its stack branch

`--whole-stack` MUST land the whole stack of the branch (any branch of it) in one robot
([Whole Stack](../../branch/whole-stack/spec.md#the-safe-push-of-the-whole-stack)): the robot starts from the stack
branch of its top-level root, `refs/heads/<prefix>/stack/<root>`, the only branch pushed, leased on the remote head of
its remote status in the sync's report, with nothing read again.

#### Scenario: The Safe Push of the whole stack
- **WHEN** `brd safe-push <branch> --whole-stack` runs for any branch of the stack
- **THEN** one robot starts from `refs/heads/<prefix>/stack/<root>` of its top-level root, the only branch pushed
- **AND** the push is leased on the remote head of its remote status in the sync's report, nothing read again
  [@test] crates/bd-remote/src/safe_push_test.rs (`the_whole_stack_lands_in_one_robot_from_its_stack_branch`)
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_safe_push_check_of_the_whole_stack`)

### Requirement: `brd safe-push`: `lands` and `whole_stack` name the whole stack

`lands` MUST name every branch of the stack, and `whole_stack` the `root`, the `source` and the `state` of the stack
branch, in the start, in `--dry-run` and in a dry run.

#### Scenario: What the whole stack lands
- **WHEN** a start, `--dry-run` or a dry run of `--whole-stack` runs
- **THEN** `lands` names every branch of the stack
- **AND** `whole_stack` names the `root`, the `source` and the `state` of the stack branch
  [@test] crates/bd-remote/src/safe_push_test.rs (`the_whole_stack_lands_in_one_robot_from_its_stack_branch`)

### Requirement: `brd safe-push`: The whole stack of a straight chain starts from its last branch

A straight chain, which has no stack branch, MUST start from its last branch that is not parked (`source` its virtual
branch).

#### Scenario: A straight chain
- **GIVEN** the stack is a straight chain, without a stack branch
- **WHEN** `--whole-stack` starts
- **THEN** the robot starts from the last branch of the chain that is not parked, `source` its virtual branch
  [@test] crates/bd-remote/src/safe_push_test.rs (`the_whole_stack_of_a_straight_chain_lands_from_its_last_branch`)

### Requirement: `brd safe-push`: The whole stack is the app's, by declared parent

The whole stack MUST be the app's, by declared parent: a blocked branch whose declared parent has no commits heads
one of its own.

#### Scenario: A blocked branch with an inactive declared parent
- **GIVEN** a blocked branch whose declared parent has no commits
- **WHEN** the whole stack of the branch is taken
- **THEN** the branch heads a whole stack of its own
  [@test] crates/bd-remote/src/scenario_test.rs (`the_whole_stack_of_a_blocked_branch_with_an_inactive_declared_parent_is_d8`)

### Requirement: `brd safe-push`: An older build on the remote stack branch is pushed over once

A remote stack branch that moved since the sync without commits the stack branch lacks (an older build of the stack)
MUST be pushed over again at once, once, leased on the fetched head, as `brd push` does: `pushed[0].pushed_over`,
and the robot starts; one with commits the stack lacks, or that moves again, MUST be `lease_rejected`, nothing started.

#### Scenario: The remote moved to an older build of the stack
- **GIVEN** the remote stack branch moved since the sync without commits the stack branch lacks
- **WHEN** the Safe Push of the whole stack pushes
- **THEN** it pushes over it once more, leased on the fetched head, `pushed[0].pushed_over` set, and the robot starts
  [@test] crates/bd-remote/src/safe_push_test.rs (`the_whole_stack_pushes_again_over_a_moved_remote_that_lacks_nothing_of_its_stack_branch`)
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_safe_push_of_the_whole_stack_over_a_moved_remote`)

#### Scenario: The remote has commits the stack lacks, or moves again
- **WHEN** the moved remote stack branch has commits the stack lacks, or moves again
- **THEN** the push is `lease_rejected` and nothing is started

### Requirement: `brd safe-push`: A stack branch that is not built refuses the start

A stack branch that is not `built` MUST refuse the start, nothing pushed or started: `stack_conflict` (exit 4) with the
commit (`error.commits`) and the files (`error.paths`) when a commit of it conflicts with the commits of the stack
before it, `stack_not_built` (exit 4) naming the branch of the stack that is blocked or failed, or why it is stale; both
say to land it in pieces.

#### Scenario: A commit of the stack conflicts
- **WHEN** a commit of the stack branch conflicts with the commits of the stack before it
- **THEN** the start is refused with `stack_conflict` (exit 4), the commit in `error.commits` and the files in
  `error.paths`, saying to land it in pieces
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_whole_stack_that_conflicts_is_refused_with_the_conflict`)

#### Scenario: A branch of the stack is blocked or failed, or the stack branch is stale
- **WHEN** a branch of the stack is blocked or failed, or the stack branch is stale
- **THEN** the start is refused with `stack_not_built` (exit 4) naming that branch or why it is stale, saying to land it
  in pieces

### Requirement: `brd safe-push`: The dry run of a stack that is not built asks no review

`--dry-run` MUST list the refusal of a stack branch that is not built and ask Patronus no review of it.

#### Scenario: The dry run of a stack that is not built
- **WHEN** `--dry-run --whole-stack` runs and the stack branch is not `built`
- **THEN** it lists the refusal and asks Patronus no review

### Requirement: `brd safe-push`: A remote stack branch with commits the stack lacks is `push_refused`

A remote stack branch with commits the stack lacks MUST be `push_refused`, naming
`brd push <root> --with-children --force`.

#### Scenario: Someone else's commits on the remote stack branch
- **GIVEN** the remote stack branch has commits the stack lacks
- **WHEN** the Safe Push of the whole stack starts
- **THEN** it is `push_refused`, naming `brd push <root> --with-children --force`

### Requirement: `brd safe-push`: A Safe Push of a part and of the whole stack exclude each other

A running Safe Push of a branch of the stack lands a part of it and MUST refuse the start of the whole stack; the
running Safe Push of the whole stack lands every branch of it and MUST refuse the start of a branch and of the whole
stack on the same commits; a dry run lands nothing.

#### Scenario: A Safe Push of a branch of the stack runs
- **GIVEN** a Safe Push of a branch of the stack runs
- **WHEN** the Safe Push of the whole stack starts
- **THEN** it is refused ("A Safe Push of login is running: wait until it merges")
  [@test] crates/bd-remote/src/safe_push_test.rs (`the_whole_stack_lands_in_one_robot_from_its_stack_branch`)

#### Scenario: The Safe Push of the whole stack runs
- **GIVEN** the Safe Push of the whole stack runs
- **WHEN** the Safe Push of a branch of it, or of the whole stack on the same commits, starts
- **THEN** it is refused ("A Safe Push of the whole auth stack is running: it lands 1 commit of login with it
  (1a2b3c4d)")
  [@test] crates/repository-model/src/patronus_view_test.rs (`the_robot_of_the_whole_stack_lands_every_branch_of_it`)

#### Scenario: A running dry run
- **GIVEN** a dry run of the stack or of a branch of it runs
- **WHEN** the guards of a start read it
- **THEN** it lands nothing of the stack

### Requirement: `brd safe-push`: The robot of the stack branch is never a branch's

A robot of the stack branch MUST never be a branch's, not even by its revision.

#### Scenario: A robot of the stack branch at a branch's revision
- **WHEN** the robot of the stack branch has the revision of a branch of the stack
- **THEN** it is the robot of the root's whole stack, not of that branch
  [@test] crates/domain-policy/src/robot_match_test.rs (`the_robot_of_the_whole_stack_belongs_to_its_root_and_to_no_branch`)

### Requirement: `brd safe-push`: The approvals of the whole stack count in its stack branch's pull request

The approvals of the whole stack MUST count in the pull request of its stack branch, the stack's own (never a branch's,
never retargeted): `--create-pull-request --whole-stack` opens it into the target; the review asks Patronus about the
stack branch with that pull request, and a start comments on it.

#### Scenario: The pull request of the whole stack
- **WHEN** `brd safe-push <branch> --create-pull-request --whole-stack` runs
- **THEN** it opens the pull request of the stack branch into the target, titled by the root
- **AND** the review asks Patronus about the stack branch with that pull request, and a start comments on it
  [@test] crates/bd-remote/src/safe_push_test.rs (`the_pull_request_of_the_whole_stack_is_its_stack_branchs_and_the_start_comments_on_it`)

#### Scenario: Its description
- **WHEN** the pull request of the whole stack is opened
- **THEN** its body is the stack branch's own description, else a section per branch (its description, else the
  subjects of its commits), cut to what GitHub takes
  [@test] crates/remote-facts/src/stack_pull_request_test.rs (`the_pull_request_of_the_whole_stack_names_each_member_by_its_description_else_its_commits`, `a_text_too_long_for_github_is_cut_and_names_the_members_left_out`)

### Requirement: `brd safe-push`: An open pull request into the target starts from a comment

Branches reviewed on GitHub with an open pull request into the target MUST start from a `/patronus` comment on it,
reading the reply of the bot every 3 s for up to a minute (`operations::robot_start`), and MUST refuse `--profile`
there; no Space merge request is looked up.

#### Scenario: A branch with an open pull request
- **GIVEN** the branch has an open pull request into the target
- **WHEN** the Safe Push starts
- **THEN** it comments `/patronus` on it and reads the reply of the bot every 3 s for up to a minute
- **AND** `--profile` is refused there, and no Space merge request is looked up
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_branch_with_an_open_pull_request_starts_from_a_comment`)

### Requirement: `brd safe-push`: The robot of a comment is the newest one the comment started

The robot of the comment MUST be the newest robot of that pull request in the mode of the start that had not finished
before the comment (`comment_robot`), looked for once right after the bot accepted; a reply that never came MUST be
`reply: pending`, a rejection `patronus_refused` with the reason of the bot.

#### Scenario: The bot accepts
- **WHEN** the bot accepts the comment
- **THEN** the robot is looked for once, the newest of the pull request in the mode of the start that had not finished
  before the comment (`comment_robot`), never an earlier one
  [@test] crates/bd-remote/src/safe_push_test.rs (`the_robot_of_a_comment_is_never_an_earlier_robot_of_the_pull_request`)
  [@test] crates/operations/src/robot_start_test.rs (`a_comment_the_bot_accepts_starts_the_newest_robot_of_its_pull_request_nobody_knew`)

#### Scenario: The bot rejects it or never answers
- **WHEN** the bot rejects the comment, or never answers within the minute
- **THEN** a rejection is `patronus_refused` with the reason of the bot, and no reply is `reply: pending`
  [@test] crates/operations/src/robot_start_test.rs (`a_comment_the_bot_rejects_fails_with_its_reason_and_one_it_never_answers_is_pending_after_the_minute`)

### Requirement: `brd safe-push`: A pull request into a virtual branch is retargeted first

An open pull request of the branch, or of a branch its push takes along, whose base is a virtual branch of the user
MUST be retargeted to the target before anything is pushed or canceled, and the robot then starts from the branch's:
`retargeted` lists them as `brd push` does, `--dry-run` the ones a start would retarget.

#### Scenario: Pull requests into virtual branches
- **GIVEN** the pull requests of the branch and of a parent its push takes along go into virtual branches
- **WHEN** the Safe Push starts
- **THEN** they are retargeted to the target before anything is pushed or canceled, and `retargeted` lists them
- **AND** the robot starts from the branch's pull request; `--dry-run` lists the ones a start would retarget
  [@test] crates/bd-remote/src/safe_push_test.rs (`the_pull_requests_of_the_branch_and_of_the_parents_it_pushes_go_into_the_target_before_the_start`)

### Requirement: `brd safe-push`: A retarget GitHub refuses fails only the branch's own

A retarget of the branch's own pull request that GitHub refuses MUST fail the start with nothing pushed or started; one
of a branch taken along MUST be a `retarget_warnings` line (where a pull request goes is the rule of the app's,
`domain_policy::pull_request::base_of`).

#### Scenario: GitHub refuses a retarget
- **WHEN** GitHub refuses the retarget of a pull request
- **THEN** one of the branch's own fails the start, nothing pushed or started
- **AND** one of a branch taken along is a `retarget_warnings` line

### Requirement: `brd safe-push`: A pull request into another base refuses a Safe Push that counts it

A pull request into another base (a release branch) MUST stay as it is, never retargeted silently: its approvals count
for no Safe Push into the target, so a Safe Push that counts them, `--create-pull-request` too, MUST refuse
`pull_request_other_base` (exit 4, also in `refusals`) with the `gh pr edit … --base <target>` retarget as its next
step, as the app refuses by row 5′ ([Next Step](../../branch/next-step/spec.md#the-table)).

#### Scenario: A Safe Push whose approvals count
- **GIVEN** the open pull request of the branch goes into another base
- **WHEN** a Safe Push that counts its approvals starts, or `--create-pull-request` runs
- **THEN** it is refused with `pull_request_other_base` (exit 4, also in `refusals`), the next step
  `gh pr edit … --base <target>`
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_pull_request_into_another_base_refuses_only_a_safe_push_whose_approvals_count`)
  [@test] crates/domain-policy/src/land_test.rs (`the_pull_request_decides_the_route_a_draft_refuses_and_another_base_refuses_only_counted_approvals`)

#### Scenario: A start that counts no approvals
- **GIVEN** the open pull request of the branch goes into another base
- **WHEN** a Dry Run, a start with `--without-approvals` or one with a profile that skips the review starts
- **THEN** it counts no approvals and starts through the API
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_pull_request_into_another_base_refuses_only_a_safe_push_whose_approvals_count`)

### Requirement: `brd safe-push`: Without the Space token or outside Space nothing runs

Without the Space token of the Safe Push CLI it MUST be `not_logged_in`, and outside Space `not_on_space`, before a
request to Patronus.

#### Scenario: No token, or no Space
- **WHEN** `brd safe-push` runs without the Space token of the Safe Push CLI, or outside Space
- **THEN** it is `not_logged_in`, or `not_on_space`, before a request to Patronus
  [@test] crates/bd-remote/src/safe_push_test.rs (`without_a_token_or_space_nothing_runs`)

### Requirement: `brd land`: It lands the branch by the route of the repository

`brd land <branch>` MUST land the branch into master by the route of the repository (`branchdeck.landing`,
`domain_policy::landing_route::route`, [Landing Routes](../../branch/landing-routes/spec.md)): Patronus runs the Safe
Push of `brd safe-push`, a push pushes the copy the sync built into the target as a fast-forward, and a merge of the
pull request merges the open pull request of the subject on GitHub, or enables its auto-merge.

#### Scenario: The Patronus route
- **WHEN** `brd land <branch>` runs on the route `route: patronus`
- **THEN** Patronus runs the Safe Push of `brd safe-push`, its output in `safe_push`

#### Scenario: The push route
- **WHEN** `brd land <branch>` runs on the route `route: push`
- **THEN** the copy the sync built (`commit`) is pushed into the target as a fast-forward

#### Scenario: The pull request route
- **WHEN** `brd land <branch>` runs on the route `route: pull-request`
- **THEN** the open pull request of the subject is merged on GitHub, or its auto-merge is enabled
  ([Landing Routes](../../branch/landing-routes/spec.md#merge))

### Requirement: `brd land`: A merge of the pull request lands only where the pull request lands

The merge of the pull request MUST be the route only where no robot of Patronus lands the repository (on Space the
route is the Safe Push) and the pull request lands into the target: a repository off GitHub or a mirror `origin` doesn't
push to is `landing_route_unsupported`; Patronus named off Space MUST be `not_on_space`.

#### Scenario: The pull request route off GitHub
- **WHEN** the route is the merge of the pull request in a repository off GitHub, or a mirror `origin` doesn't push to
- **THEN** it is `landing_route_unsupported`, nothing merged
  [@test] crates/bd-remote/src/land_test.rs (`a_pull_request_route_off_github_merges_nothing_and_patronus_off_space_starts_nothing`)

#### Scenario: Patronus off Space
- **WHEN** the route names Patronus off Space
- **THEN** it is `not_on_space`, nothing started
  [@test] crates/bd-remote/src/land_test.rs (`a_pull_request_route_off_github_merges_nothing_and_patronus_off_space_starts_nothing`)

### Requirement: `brd land`: `--whole-stack` lands the whole stack, `--dry-run` nothing

`--whole-stack` MUST land the whole stack, and `--dry-run` MUST sync and push nothing, printing `lands`,
`confirmations`, `next_step` and the `refusals` a push or a merge would meet.

#### Scenario: A dry run of a landing
- **WHEN** `brd land <branch> --dry-run` runs
- **THEN** it syncs, pushes nothing and prints `lands`, `confirmations`, `next_step` and the `refusals` a push or a merge
  would meet

#### Scenario: The whole stack
- **WHEN** `brd land <branch> --whole-stack` runs
- **THEN** it lands the whole stack of the branch

### Requirement: `brd land`: A push runs the plan of the app's start through the engine

A push MUST run the plan of the app's start (`operations::land::prepare_one_shot` over the one-shot session of the
command: the pull requests with the permission of the token, the rules of the target, the landing gate of the subject)
through the engine (`operations::land_execute::execute`): no check against the newest target, the witness and the
freeze under the lock (its sync before it, as `brd sync`), the push by the push URL after it.

#### Scenario: The lock of a push
- **WHEN** `brd land` lands by a push
- **THEN** the sync runs before the lock, the witness and the freeze under it, and the push by the push URL after it
- **AND** no check against the newest target runs
  [@test] crates/bd-remote/src/land_test.rs (`a_land_by_a_push_locks_only_the_witness`)

#### Scenario: The app and `brd` plan alike
- **WHEN** the same facts reach the app's start and `brd land`
- **THEN** both plan the same route and refusals
  [@test] crates/bd-remote/src/scenario_test.rs (`the_safe_push_mode_and_route_cases_answer_alike_in_the_app_and_bd`)

### Requirement: `brd land`: What CI did not check and the gate of the route refuse a push

What CI did not check and the changes requested MUST refuse a push (`checks_not_passed`) unless
`--allow-checks-not-passed`; what the gate of the route asks MUST refuse it with the code of its reason
(`baseline_requires_pull_request`, `baseline_refused`, `push_refused`) unless `--bypass-rules`; both flags only once the
user agreed.

#### Scenario: Checks CI did not pass
- **WHEN** CI did not check the branch, or changes are requested
- **THEN** the push is refused with `checks_not_passed`
- **AND** with `--allow-checks-not-passed`, once the user accepted, it lands the copy into master
  [@test] crates/bd-remote/src/land_test.rs (`lands_the_copy_of_a_branch_into_master_once_the_user_accepted_what_ci_did_not_check`)

#### Scenario: The gate of the route
- **WHEN** the gate of the route asks for a pull request, or refuses the push
- **THEN** the push is refused with `baseline_requires_pull_request`, `baseline_refused` or `push_refused`, unless
  `--bypass-rules`

### Requirement: `brd land`: A moved target refuses the push as `baseline_moved`

A target that moved MUST be `baseline_moved` (exit 3, master fetched, next step `brd update`), and so MUST a fetched
master with commits the branch is not built on, refused before anything is sent (`--dry-run` names the step
`update_project`); a refusal of the server MUST be `baseline_refused` in its words; below write access
`no_write_access`.

#### Scenario: A fetched master ahead of the branch
- **GIVEN** the fetched master has commits the branch is not built on
- **WHEN** `brd land` pushes, or `--dry-run` runs
- **THEN** it is refused as `baseline_moved` before anything is sent, and `--dry-run` names the step `update_project`
  [@test] crates/bd-remote/src/land_test.rs (`a_fetched_target_ahead_of_the_branch_refuses_the_push_before_it_is_sent`)

#### Scenario: The target moved during the push, or the server refuses it
- **WHEN** the target moved, or the server refuses the push
- **THEN** a moved target is `baseline_moved` (exit 3, master fetched, next step `brd update`)
- **AND** a refusal of the server is `baseline_refused` in its words
  [@test] crates/bd-remote/src/land_test.rs (`a_moved_target_is_baseline_moved_and_a_refusal_of_the_server_baseline_refused`)

#### Scenario: Below write access
- **WHEN** the token has less than write access
- **THEN** the push is refused with `no_write_access`

### Requirement: `brd land`: A merge runs the plan of the app's start

A merge MUST run the plan of the app's start the same way: the pull requests with the merge state, the auto-merge and
the merge methods, the robots (off Space Patronus lands nothing), the landing gate. It MUST print `pull_request`;
`landed: true` after GitHub merged it, `auto_merge: true` once GitHub merges it when its checks pass.

#### Scenario: GitHub merges the pull request
- **WHEN** `brd land` merges the pull request
- **THEN** it prints `pull_request` with `landed: true` after GitHub merged it
  [@test] crates/bd-remote/src/scenario_test.rs (`the_safe_push_mode_and_route_cases_answer_alike_in_the_app_and_bd`)

#### Scenario: Auto-merge
- **WHEN** the auto-merge is enabled: now, after the push of the branch when its pull request showed another commit, or
  before
- **THEN** it prints `auto_merge: true`, as GitHub merges the pull request when its checks pass

### Requirement: `brd land`: What holds the merge back refuses it with the code of its reason

What holds the merge back MUST refuse it with the code of its reason: `no_pull_request` (create it first),
`merge_blocked` (approvals, required checks, changes requested, a draft), `not_mergeable` (a conflict, a head behind its
base with `brd update` next, a branch to push first with `brd push` next, a refusal of GitHub in its words); a head
GitHub moved meanwhile is `plan_changed`.

#### Scenario: The merge is held back
- **WHEN** the merge has no pull request, is blocked or is not mergeable
- **THEN** it is refused with `no_pull_request`, `merge_blocked` or `not_mergeable`, with `brd update` or `brd push` as
  the next step where they help
  [@test] crates/bd-remote/src/scenario_test.rs (`the_safe_push_mode_and_route_cases_answer_alike_in_the_app_and_bd`)

#### Scenario: Checks that fail and a moved head
- **WHEN** checks its base doesn't require fail, or GitHub moved the head meanwhile
- **THEN** failed checks are `checks_not_passed` until `--allow-checks-not-passed`
- **AND** a moved head is `plan_changed`

### Requirement: `brd land`: It never updates, and names `brd update` next

`brd land` MUST never update: a push that landed, and a merge, MUST print `landed: true` and `next: "brd update"`, which
brings the branch onto the target and drops the commits that landed.

#### Scenario: After a landing
- **WHEN** a push or a merge of `brd land` landed
- **THEN** it prints `landed: true` and `next: "brd update"`, and does not update the branch itself
  [@test] crates/bd-remote/src/land_test.rs (`lands_the_copy_of_a_branch_into_master_once_the_user_accepted_what_ci_did_not_check`)

### Requirement: `brd land`: It lists the pull requests its push closes and closes none

It MUST list the open pull requests its push closes as `closes` (`{branch, number}`: of what it landed, those that show
an older version of a branch, by the rule of the app, `domain_policy::landed::pull_requests_to_close`) and close none:
the app's session closes them with their comments
([Landing Routes](../../branch/landing-routes/spec.md#closing-what-landed)).

#### Scenario: Pull requests of an older version
- **GIVEN** open pull requests show an older version of a branch it landed
- **WHEN** the push lands
- **THEN** `closes` lists them as `{branch, number}`, and none is closed
  [@test] crates/bd-remote/src/land_test.rs (`lands_the_copy_of_a_branch_into_master_once_the_user_accepted_what_ci_did_not_check`)

### Requirement: `brd land`: The skill says that a push lands without a robot and needs approval

The agent skill MUST say that the push route lands without a robot and without CI and needs the user's approval like
any push.

#### Scenario: The skill of the push route
- **WHEN** an agent reads the agent help
- **THEN** it says that the push route lands without a robot and without CI, and needs the user's approval like any push
  [@test] crates/bd-cli/src/agent_help_test.rs (`the_agent_help_is_a_short_core_with_every_guard`)

### Requirement: `brd request-review`: It asks the missing code owners as the app's Request Review does

`brd request-review <branch>` MUST ask GitHub users to review the open pull request of the virtual branch, as the
app's Request Review… does ([Pull Requests](../../branch/pull-requests/spec.md#request-review)): without `--reviewer`,
one suggested code owner with a GitHub login of each group whose approval is missing and nobody is asked for yet, each
once, by the one rule of both, `domain_policy::review::candidates`.

#### Scenario: The suggested code owner of a missing group
- **GIVEN** a group whose approval is missing and nobody is asked for yet
- **WHEN** `brd request-review <branch>` runs without `--reviewer`
- **THEN** it asks one suggested code owner of the group with a GitHub login, each person once
- **AND** never the author, a member asked already or one whose approval counts (the ones the app's popover checks)
- **AND** the candidates come from the review readiness `brd safe-push` checks (the pre-check with the pull request,
  asked again)
  [@test] crates/bd-remote/src/request_review_test.rs (`asks_the_suggested_code_owner_of_the_missing_group_once`)

### Requirement: `brd request-review`: `--reviewer` asks the logins it is given

`--reviewer <login>` (repeatable) MUST ask those instead and not ask Patronus.

#### Scenario: Logins given
- **WHEN** `brd request-review <branch> --reviewer <login>` runs, the flag repeated
- **THEN** it asks those logins and doesn't ask Patronus
  [@test] crates/bd-remote/src/request_review_test.rs (`asks_the_logins_it_is_given_and_names_whom_github_refuses`)

### Requirement: `brd request-review`: It is the intent of the app's request

It MUST be the intent of the app's request (`RequestReview`, `operations::pull_request`), planned over the one-shot
session of the command (`operations::needs`) and run by the same engine.

#### Scenario: The request runs as the app's
- **WHEN** `brd request-review <branch>` runs
- **THEN** it plans `RequestReview` over the one-shot session of the command and runs it by the engine of the app
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_request_review_check`)

### Requirement: `brd request-review`: It prints whom it asked and whom GitHub refused

It MUST print `reviewers` (whom it asks), `requested`, `refused` with GitHub's reason, `waiting_for` (the reviewers the
pull request asks after the request), the `readiness` before it, each person with the full name Patronus knows, and
`pushed`, the branches it pushed first. `--dry-run` MUST print the same object and push and ask nothing.

#### Scenario: The output of a request
- **WHEN** a request ran
- **THEN** it prints `reviewers`, `requested`, `refused` with GitHub's reason, `waiting_for`, the `readiness` before it
  and `pushed`, each person with the full name Patronus knows
  [@test] crates/bd-remote/src/request_review_test.rs (`asks_the_logins_it_is_given_and_names_whom_github_refuses`)

#### Scenario: A dry run
- **WHEN** `brd request-review <branch> --dry-run` runs
- **THEN** it prints the same object and pushes and asks nothing

### Requirement: `brd request-review`: It holds the lock only for the witness and the freeze of its push

It MUST hold the repository lock only for the witness and the freeze of the push of `--push` (its sync as `brd sync`,
without it), and write nothing but the push of `--push` and the request on GitHub.

#### Scenario: The lock and the writes of a request
- **WHEN** `brd request-review <branch> --push` runs
- **THEN** the repository lock is held only for the witness and the freeze of its push, the sync running without it
- **AND** nothing is written but the push and the request on GitHub
  [@test] crates/bd-remote/src/request_review_test.rs (`a_request_locks_only_the_witness_and_the_freeze_of_its_push`)

### Requirement: `brd request-review`: Work the remote branch lacks is `needs_push`

A branch with work its remote branch lacks MUST be `needs_push` (exit 4) with the next step
`brd request-review <branch> --push` (`needs_approval: true`): the reviewers would not see the commits.

#### Scenario: Commits not pushed yet
- **GIVEN** the branch has work its remote branch lacks
- **WHEN** `brd request-review <branch>` runs without `--push`
- **THEN** it is `needs_push` (exit 4), the next step `brd request-review <branch> --push` with `needs_approval: true`
  [@test] crates/bd-remote/src/request_review_test.rs (`a_branch_its_remote_lacks_is_refused_without_push_and_pushed_first_with_it`)

### Requirement: `brd request-review`: `--push` pushes first as the app's request does

`--push` MUST push the commits first as the app's request does (the ancestors with work first, root first; an open
pull request of what it pushes into a virtual branch retargeted into the target before the push), then ask; a push the
user decides (one that drops remote commits) MUST refuse it as `brd push` does.

#### Scenario: Pushed first
- **WHEN** `brd request-review <branch> --push` runs for a branch with work its remote branch lacks
- **THEN** it pushes the ancestors with work first, root first, then the branch, and then asks
  [@test] crates/bd-remote/src/request_review_test.rs (`a_branch_its_remote_lacks_is_refused_without_push_and_pushed_first_with_it`)

#### Scenario: A push the user decides
- **WHEN** the push would drop remote commits
- **THEN** the request is refused as `brd push` refuses such a push

### Requirement: `brd request-review`: No pull request, no reviewer, or none asked refuse it

Without an open pull request, or with a draft, it MUST be `no_pull_request` (exit 4), naming
`brd safe-push <branch> --create-pull-request`; a missing group nobody is asked for whose code owners have no GitHub
login Patronus names, with nobody else to ask, `no_reviewer`; a request GitHub asked nobody in, `review_refused` naming
each with the reason.

#### Scenario: No open pull request, or no code owner to ask
- **WHEN** the branch has no open pull request, or only a draft, or a missing group has no code owner with a GitHub login
  Patronus names and nobody else to ask
- **THEN** it is `no_pull_request` (exit 4) naming `brd safe-push <branch> --create-pull-request`, or `no_reviewer`
  [@test] crates/bd-remote/src/request_review_test.rs (`a_branch_without_an_open_pull_request_or_a_code_owner_to_ask_is_refused`)

#### Scenario: GitHub asks nobody
- **WHEN** GitHub asked nobody of the request in
- **THEN** it is `review_refused`, naming each reviewer with the reason
  [@test] crates/bd-remote/src/request_review_test.rs (`asks_the_logins_it_is_given_and_names_whom_github_refuses`)

### Requirement: `brd robot`: It prints the robot of the virtual branch as `brd safe-push` finds it

`brd robot <branch>` MUST print the robot of the virtual branch as `brd safe-push` finds it (a running one first, else
the newest, by the branch or by the local or pushed tip of a temporary ref), or the one of `--robot`, with its status,
mode, profile and whether it still runs; `robot: null` with exit 0 when the branch has none, and `unknown_branch` for a
name that is neither a branch nor a branch with a robot.

#### Scenario: A branch with a robot
- **WHEN** `brd robot <branch>` runs for a branch with a robot
- **THEN** it prints the robot (a running one first, else the newest) with its status, mode, profile and whether it
  still runs
  [@test] crates/bd-remote/src/robot_test.rs (`a_successful_robot_has_no_failure_and_a_branch_without_one_has_no_robot`, `a_failed_robot_says_why_and_which_local_commits_it_tested`)

#### Scenario: No robot, or no branch
- **WHEN** the branch has no robot, or the name is neither a branch nor a branch with a robot
- **THEN** it prints `robot: null` with exit 0, or fails with `unknown_branch`
  [@test] crates/bd-remote/src/robot_test.rs (`a_successful_robot_has_no_failure_and_a_branch_without_one_has_no_robot`)

### Requirement: `brd robot`: It writes nothing

`brd robot` MUST write no ref, take no lock and run no sync.

#### Scenario: A read only
- **WHEN** `brd robot <branch>` runs
- **THEN** no ref is written, no lock taken and no sync run
  [@test] crates/bd-remote/src/robot_test.rs (`a_failed_robot_says_why_and_which_local_commits_it_tested`)

### Requirement: `brd robot`: A failed robot says why and which local commits it tested

For a failed robot, and for a running one whose required check failed already, it MUST print the `failure` the app's
Copy Prompt for Agent builds and the commit map by the ids of the checked-out branch, `virtual_behind_head` for
commits the last sync didn't build, and `head_branch`; no `failure` for any other robot.

#### Scenario: A failed robot
- **WHEN** the robot failed
- **THEN** `failure` has `kind`, the failed checks with their build ids and counts, the error problems, the test and
  target revisions
- **AND** the commit map by the ids of the checked-out branch says `tested`, `rebased`, `changed`, `added`, `removed`,
  `landed`, `unknown`, with `virtual_behind_head` and `head_branch`
  [@test] crates/bd-remote/src/robot_test.rs (`a_failed_robot_says_why_and_which_local_commits_it_tested`)

#### Scenario: A running robot whose required check failed
- **WHEN** the robot runs and a required check of it failed already
- **THEN** it prints its `failure` as for a failed robot
  [@test] crates/bd-remote/src/robot_test.rs (`a_running_robot_whose_required_check_failed_is_failing`)

#### Scenario: Any other robot
- **WHEN** the robot succeeded, or runs with no failed required check
- **THEN** there is no `failure`
  [@test] crates/bd-remote/src/robot_test.rs (`a_successful_robot_has_no_failure_and_a_branch_without_one_has_no_robot`)

### Requirement: `brd robot`: `--full` adds the failed builds and their log tails

`--full` MUST add the failed builds (no composite one) with their problems, failed tests and messages, and, through the
`teamcity` CLI when it serves the build, the log tail of a build that failed not only on tests and whether a test
failed before.

#### Scenario: The details of a failure
- **WHEN** `brd robot <branch> --full` runs for a failed robot
- **THEN** it adds the failed builds, no composite one, with their problems, failed tests and messages
- **AND** through the `teamcity` CLI, when it serves the build, the log tail of a build that failed not only on tests
  and whether a test failed before
  [@test] crates/bd-remote/src/robot_test.rs (`full_adds_the_failed_builds_their_tests_and_the_log_tails_of_the_teamcity_cli`)

### Requirement: `brd robot`: A newer robot of a branch stacked on it wins

A newer robot of a branch stacked on it MUST win over the branch's own, named by `robot_branch`: it built this branch
too. The commit map of a stacked branch's robot MUST have the commits of its ancestors too, each with its `branch`.

#### Scenario: A newer robot of a child
- **GIVEN** a branch stacked on the branch has a newer robot than the branch's own
- **WHEN** `brd robot <branch>` runs
- **THEN** it prints that robot, named by `robot_branch`
- **AND** its commit map has the commits of the ancestors too, each with its `branch`
  [@test] crates/bd-remote/src/robot_test.rs (`a_newer_robot_of_a_stacked_branch_tells_the_parent_with_the_commits_of_the_whole_stack`)

### Requirement: `brd robot`: `--robot` takes a robot of the branch only

`--robot` of a robot of another branch MUST be `invalid_arguments` naming that branch, unless that branch is stacked
on it.

#### Scenario: A robot of another branch
- **WHEN** `--robot` names a robot of another branch, not stacked on the branch
- **THEN** it is `invalid_arguments` naming that branch
  [@test] crates/bd-remote/src/robot_test.rs (`robot_takes_a_robot_by_id_of_the_branch_only`)

### Requirement: `brd robot`: The robot of the whole stack is found from any branch of it

A newer robot of the whole stack the branch is a branch of MUST win too, named by `stack_root`; `--whole-stack` MUST
print the robot of the whole stack, as `brd safe-push --whole-stack` starts it (of a straight chain, the robot of its
last branch), and refuse `--robot` of another one.

#### Scenario: From any branch of the stack
- **WHEN** `brd robot <branch>` runs for a branch of a whole stack with a newer robot, or with `--whole-stack`
- **THEN** it prints the robot of the whole stack, named by `stack_root`
- **AND** `--whole-stack` refuses `--robot` of another robot
  [@test] crates/bd-remote/src/robot_test.rs (`the_robot_of_the_whole_stack_is_found_from_any_branch_of_it`)

### Requirement: `brd robot`: A failure of the whole stack lists the commits of every branch

A failure of the whole stack's robot MUST list the commits of every branch of the stack, each with its branch, matched
by identity: the stack branch has no mapping notes.

#### Scenario: The whole stack's robot failed
- **WHEN** the robot of the whole stack failed
- **THEN** its commit map lists the commits of every branch of the stack, each with its branch, matched by identity
  [@test] crates/bd-remote/src/robot_test.rs (`the_robot_of_the_whole_stack_is_found_from_any_branch_of_it`)

### Requirement: `brd robot`: Without the Space token or outside Space nothing is asked

Without the Space token it MUST be `not_logged_in`, and outside Space `not_on_space`, before a request to Patronus.

#### Scenario: No token, or no Space
- **WHEN** `brd robot <branch>` runs without the Space token, or outside Space
- **THEN** it is `not_logged_in`, or `not_on_space`, before a request to Patronus
  [@test] crates/bd-remote/src/robot_test.rs (`robot_without_a_token_or_space_asks_nothing`)

### Requirement: `brd robot`: A merged Safe Push lists the pull requests its landing closes

For a merged Safe Push it MUST list `closes`: the open pull requests of the branches it landed whole besides its own (a
branch it is built on, every branch of its whole stack), each holding nothing the branch lacks, never the pull request
it started from: the ones the app closes, by the same rule (`domain_policy::landed::pull_requests_to_close`);
`brd` MUST close none.

#### Scenario: A merged robot
- **WHEN** `brd robot <branch>` prints a merged Safe Push
- **THEN** `closes` lists the open pull requests of the branches it landed whole besides its own, never the one it
  started from, compared with the tips the refs hold (no sync)
- **AND** `brd` closes none: it writes nothing to GitHub
  [@test] crates/bd-remote/src/robot_test.rs (`a_merged_robot_lists_the_pull_requests_its_landing_closes_and_closes_none`)

#### Scenario: No list
- **WHEN** the robot didn't merge, the repository is off GitHub, or GitHub doesn't list the pull requests
- **THEN** `closes` is left out

### Requirement: `brd robot-stats`: It prints the Safe Push statistics of the repository

`brd robot-stats` MUST print the Safe Push statistics of the repository from the core of the app: the user's robots of
the period (`--period 14d|30d|10w|6m`, 30 days by default) against everyone else's, the time lost, the failure causes
and flaky checks of the last 14 days and the start hours
([Safe Push Statistics](../../repository/safe-push-statistics/spec.md)).

#### Scenario: The period
- **WHEN** `brd robot-stats` runs with `--period 14d|30d|10w|6m`, or without it
- **THEN** the period is the days that count, 30 days by default
  [@test] crates/bd-remote/src/robot_stats_test.rs (`the_period_is_the_days_that_count`)

#### Scenario: The statistics
- **WHEN** `brd robot-stats` runs
- **THEN** it prints the user's robots of the period against everyone else's, the time lost, the failure causes and
  flaky checks of the last 14 days and the start hours
  [@test] crates/bd-remote/src/robot_stats_test.rs (`the_statistics_count_the_users_robots_against_everyone_elses_and_write_nothing`)

### Requirement: `brd robot-stats`: It only reads, from Patronus listed week by week and kept

`brd robot-stats` MUST only read, Patronus listed week by week and kept in the object cache.

#### Scenario: Nothing is written
- **WHEN** `brd robot-stats` runs
- **THEN** it writes nothing, and the weeks Patronus listed are kept in the object cache
  [@test] crates/bd-remote/src/robot_stats_test.rs (`the_statistics_count_the_users_robots_against_everyone_elses_and_write_nothing`)

#### Scenario: A failure after a read, or no statistics
- **WHEN** a read of Patronus fails after an earlier one, or there are no statistics
- **THEN** it prints the kept numbers with a warning, or says why there are none
  [@test] crates/bd-remote/src/robot_stats_test.rs (`a_failure_after_a_read_prints_the_kept_numbers_with_a_warning`, `the_statistics_say_why_there_are_none`)

### Requirement: `brd describe`: It prints the description of the branch

`brd describe <branch>` MUST print the branch's `branch.<name>.description` as `{title, body}`, or `null`. The
`descriptions` of a `brd group` plan SHALL set several at once.

#### Scenario: A branch with a description
- **WHEN** `brd describe <branch>` runs
- **THEN** it prints `branch.<name>.description` of the branch as `{title, body}`, or `null` without one

#### Scenario: Several at once
- **WHEN** a `brd group` plan has `descriptions`
- **THEN** they set the descriptions of several branches at once

### Requirement: `brd describe`: `--title`, `--body` and `--clear` set and remove it

`--title`, `--body` and `--body-file` MUST set it as `title`, a blank line and the body, keeping the part not given;
`--clear` MUST remove it. A description MUST have a title, and the branch MUST be a current group.

#### Scenario: Set, print and clear
- **WHEN** `--title`, `--body` or `--body-file` is given, or `--clear`
- **THEN** the description is the title, a blank line and the body, the part not given kept
- **AND** `--clear` removes it
  [@test] crates/bd-rewrite/src/describe_test.rs (`sets_prints_and_clears_a_description`)
  [@test] crates/git-ops/src/branch_description_test.rs (`writes_and_clears_a_description`)

#### Scenario: No title, or no current group
- **WHEN** the description would have no title, or the branch is no current group
- **THEN** it fails
  [@test] crates/bd-rewrite/src/describe_test.rs (`describe_errors`)

### Requirement: `brd version`: It prints the version of the app and of git

`brd version` and `brd --version` MUST print `{"schema":2,"version","git"}` without a repository: the version of the
app (the root `package.json`, which `tauri.conf.json` names), and the version of the git brd finds, `null` without one.

#### Scenario: The version outside a repository
- **WHEN** `brd version` or `brd --version` runs outside a repository
- **THEN** it prints `{"schema":2,"version","git"}`, the version of the app from the root `package.json` and that of
  the git brd finds, `null` without one
  [@test] crates/bd-cli/src/version_test.rs (`version_and_dash_dash_version`)

### Requirement: `brd install`: It puts a `brd` command where an agent finds it

`brd install` and Help → Install Agent CLI… MUST put a `brd` command where an agent finds it: on macOS and Linux a
symlink in `~/.local/bin` to the app binary, replacing a symlink there and refusing anything else (`install_refused`);
on Windows `brd.exe` next to the app.

#### Scenario: macOS and Linux
- **WHEN** `brd install` or Help → Install Agent CLI… runs on macOS or Linux
- **THEN** a symlink in `~/.local/bin` points at the app binary, replacing a symlink there
- **AND** anything else there is refused with `install_refused`, and left as it is
  [@test] crates/bd-cli/src/install_test.rs (`a_file_in_the_way_is_refused`)

#### Scenario: Windows
- **WHEN** it runs on Windows
- **THEN** `brd.exe` is put next to the app

### Requirement: `brd install`: The legacy `bd` link of this app is removed

If `~/.local/bin/bd` is a symlink to this app, `brd install` and `--uninstall` MUST remove it and list its path in
`removed_legacy`. A foreign file or symlink at that path MUST remain untouched, including a beads installation.

#### Scenario: The legacy link points at the app
- **GIVEN** `~/.local/bin/bd` is a symlink to this app
- **WHEN** `brd install` or `brd install --uninstall` runs
- **THEN** it is removed and its path listed in `removed_legacy`
  [@test] crates/bd-cli/src/install_test.rs (`install_removes_the_legacy_bd_link_only_when_it_points_at_the_app`, `installs_the_command_and_the_skills_and_uninstalls_them`)

#### Scenario: A foreign `bd`
- **GIVEN** a foreign file or symlink is at `~/.local/bin/bd`, a beads installation among them
- **WHEN** `brd install` or `brd install --uninstall` runs
- **THEN** it stays untouched
  [@test] crates/bd-cli/src/install_test.rs (`install_and_uninstall_leave_a_foreign_bd_file_alone`)

### Requirement: `brd install`: On macOS it links the `brd-host` of the app bundle

On macOS it MUST also link `~/.local/bin/brd-host` to the `brd-host` that the app bundle carries beside the app binary
([brd-host](../host/spec.md), "The bundle"), replacing a symlink there; a binary there MUST stay, because
`brd-host upgrade` wrote it and the `LaunchAgent` of a host runs it. `--uninstall` MUST remove only that symlink, or one
to a `brd-host` that is gone.

#### Scenario: The link of `brd-host`
- **WHEN** `brd install` runs on macOS
- **THEN** `~/.local/bin/brd-host` links to the `brd-host` of the app bundle, replacing a symlink there
- **AND** `bd_host` names the command, `null` when the app carries none
- **AND** a binary there stays
  [@test] crates/bd-cli/src/install_test.rs (`install_links_the_brd_host_of_the_app_bundle_and_keeps_a_binary_there`)

#### Scenario: Uninstall
- **WHEN** `brd install --uninstall` runs on macOS
- **THEN** it removes only that symlink, or one to a `brd-host` that is gone
  [@test] crates/bd-cli/src/install_test.rs (`install_links_the_brd_host_of_the_app_bundle_and_keeps_a_binary_there`)

### Requirement: `brd install`: It writes the `branch-deck` skill for the agents

It MUST write the skill `branch-deck` into `~/.claude/skills/` and `~/.agents/skills/`, with its full path, and
`brd help skill` MUST print the same text.

#### Scenario: The skill files
- **WHEN** `brd install` runs
- **THEN** the skill `branch-deck` is in `~/.claude/skills/` and `~/.agents/skills/`
- **AND** `brd help skill` prints the same text
  [@test] crates/bd-cli/src/install_test.rs (`installs_the_command_and_the_skills_and_uninstalls_them`)

### Requirement: `brd install`: The description of the skill says when to use brd

The description of the skill MUST say when to use brd: first of all before the first edit or commit in a Git
repository, for `brd workspace` ([brd workspace](../workspace/spec.md)); then grouping into branches and pull requests,
stacked when they build on each other, and updating onto the baseline with its conflicts, landing a worktree's commits,
fixups, a change in the wrong commit, a review request, why a robot failed.

#### Scenario: The triggers of the skill
- **WHEN** an agent reads the description of the skill
- **THEN** it names `brd workspace` before the first edit or commit, then the tasks of brd
- **AND** the landing of a worktree's commits onto a shared branch is among its triggers

### Requirement: `brd install`: The body of the skill sends the agent to the right command first

The body of the skill MUST name brd by its full path, `brd workspace` before the first edit with what each of its rules
asks, `brd help agent` as the workflow of grouping, pushing and Safe Push, `brd help update` for an update,
`brd help worktrees` when several sessions or worktrees work on one branch, and `brd <command> --help` for the outputs
and refusals of a command.

#### Scenario: What the agent reads next
- **WHEN** an agent reads the body of the skill
- **THEN** it finds `brd workspace` first, then `brd help agent`, `brd help update`, `brd help worktrees` and
  `brd <command> --help` for what each is for

### Requirement: `brd install`: The skill lands by the route of the repository

The skill MUST say that `brd lease pick` and `brd pick` land on the local branch only and `brd land <branch> --dry-run`
names the route of the repository, a Safe Push only on its `patronus` route (`not_on_space` never retried), and
`brd safe-push <branch> --dry-run` before a Safe Push, which lands a stacked branch with its parents, and
`--whole-stack`, which lands the whole stack.

#### Scenario: Landing in the skill
- **WHEN** an agent reads the skill
- **THEN** a pick is local, `brd land <branch> --dry-run` names the route, and a Safe Push is only for the `patronus`
  route, `not_on_space` never retried
- **AND** `brd safe-push <branch> --dry-run` comes before a Safe Push, with `--whole-stack` for the whole stack
  [@test] crates/bd-cli/src/agent_help_test.rs (`the_skill_lands_by_the_route_of_the_repository_and_a_pick_is_local`)

### Requirement: `brd install`: The skill holds the rules an agent must not miss

The skill MUST hold the rules an agent must not miss before it reads any help: approval before every push, never
reorder, squash or split commits and never drop one yourself, `brd pick` instead of a cherry-pick, `error.code`, parked
branches, unapply.

#### Scenario: The rules before any help
- **WHEN** an agent reads the skill
- **THEN** it reads the approval before every push, `brd pick` instead of a cherry-pick, `error.code`, parked branches
  and unapply
- **AND** never to reorder, squash, split or drop commits itself: brd drops a commit it proves landed, `--landed` one
  after the user's yes; `brd squash` squashes the commits the user asked to squash

### Requirement: `brd install`: The skill says the commit subject has the form of the repository

The skill MUST say that the commit subject has the form of the repository after its `(branch) ` prefix, also with the
rule off (its agent instructions, else its `git log`; a pick refuses what `[commit] check` rejects).

#### Scenario: The form of a subject
- **WHEN** an agent reads how to write a commit subject in the skill
- **THEN** the subject is the `(branch) ` prefix, then the form of the repository from its agent instructions, else its
  `git log`, also with the rule off
  [@test] crates/bd-cli/src/agent_help_test.rs (`the_skill_says_that_the_repository_form_of_a_subject_follows_the_prefix`)

### Requirement: `brd install`: The skill says how to write the commit body a review reads

Below the subject the skill MUST say, in five lines, the commit body a review reads: the diff shows what, the body why:
the lead with the new state and its cause, a paragraph per decision, code names in backticks that `brd review-plan`
resolves, `Look closely:`, the labels of the verification, the trailers last
([The Commit Body a Review Reads](../../../../docs/guide/commit-bodies.md)).

#### Scenario: The commit body
- **WHEN** an agent reads the skill below the commit subject
- **THEN** five lines say how to write the commit body a review reads
  [@test] crates/bd-cli/src/agent_help_test.rs (`the_skill_says_how_to_write_the_commit_body_a_review_reads_under_the_subject`)

### Requirement: `brd install`: The details of an output belong in the help of the binary

The details of an output MUST belong in the binary's help, not in the skill.

#### Scenario: The details of an output
- **WHEN** an agent needs the fields of an output
- **THEN** it finds them in the help of the binary, not in the skill

### Requirement: `brd install`: The skill allows every brd command

Its `allowed-tools` MUST be `Bash(<brd>:*)`, `<brd>` as the body names it, so that Claude Code runs every brd command
without a permission prompt.

#### Scenario: The allowed tools
- **WHEN** Claude Code loads the skill
- **THEN** its `allowed-tools` is `Bash(<brd>:*)` with `<brd>` as the body names it
  [@test] crates/bd-cli/src/agent_help_test.rs (`the_skill_allows_every_brd_command_as_named`)

### Requirement: `brd install`: It says whether brd is on `PATH` and how agents work

It MUST say whether `brd` on `PATH` is the installed one, and print `agents` `{lease, subject_branch}`, the rules of
`brd workspace` that the global git config gives every repository; `--uninstall` MUST remove the skills and only its own
symlink.

#### Scenario: Install and uninstall
- **WHEN** `brd install` runs, then `brd install --uninstall`
- **THEN** the install says whether `brd` on `PATH` is the installed one and prints `agents` `{lease, subject_branch}`
- **AND** the uninstall removes the skills and only its own symlink
  [@test] crates/bd-cli/src/install_test.rs (`installs_the_command_and_the_skills_and_uninstalls_them`)

#### Scenario: `brd` not on `PATH`
- **WHEN** `brd install` runs and `~/.local/bin` is not on `PATH`
- **THEN** it installs and says that `brd` is not on `PATH` (`on_path: false`)
  [@test] crates/bd-cli/src/install_test.rs (`a_file_in_the_way_is_refused`)

### Requirement: `brd install`: The toast of the app says where agents find brd

In the app the toast "Agent CLI installed" MUST say where agents find brd, how to run it and how agents work from now
on, paths under the home directory as `~`, with "Open Agent Settings" from the main window
([Onboarding](../../repository/onboarding/spec.md#agents)); a failure MUST be the error toast "Cannot install the Agent
CLI".

#### Scenario: The install succeeds in the app
- **WHEN** Help → Install Agent CLI… succeeds
- **THEN** the toast "Agent CLI installed" names the skills and brd as a command when it is on `PATH`, else its full
  path and the directory to add to `PATH`, paths under the home directory as `~`
- **AND** it says how agents work by the rules of every repository, with "Open Agent Settings" from the main window
  [@test] src/hooks/useInstallAgentCli.test.ts (`names the skills, and brd as a command when it is on PATH`, `names the full path and the directory to add to PATH when brd is not on it`, `says how agents work by the rules of every repository`)

#### Scenario: The install fails in the app
- **WHEN** the install fails
- **THEN** the error toast is "Cannot install the Agent CLI"

### Requirement: `brd install`: `agent_cli_status` names what is installed and writes nothing

`agent_cli_status` MUST name the skill files that exist and the command they name, `null` without one, writing nothing
(the row `brd` of Settings → Tools, the install line of Settings → Agents).

#### Scenario: The status in Settings
- **WHEN** Settings → Tools or Settings → Agents reads `agent_cli_status`
- **THEN** it names the skill files that exist and the command they name, `null` without one, and writes nothing

### Requirement: `brd install`: On Linux an AppImage is linked, not its mount

The symlink MUST outlive the running app: on Linux, with `APPIMAGE` set, it MUST point at the AppImage file, not at the
binary in its temporary mount.

#### Scenario: An AppImage
- **GIVEN** `APPIMAGE` is set on Linux
- **WHEN** `brd install` runs
- **THEN** the symlink points at the AppImage file, not at the binary in its temporary mount
  [@test] crates/bd-cli/src/install_test.rs (`the_link_target_is_the_appimage_not_its_mount`, `install_links_to_the_appimage`)

### Requirement: `brd install`: A translocated app on macOS is refused

On macOS an app run translocated (its path under `AppTranslocation/`) MUST be `install_refused` with "Branch Deck runs
from a temporary location: move it to Applications, then install the Agent CLI".

#### Scenario: A translocated app
- **GIVEN** the app runs from a path under `AppTranslocation/`
- **WHEN** `brd install` runs
- **THEN** it is `install_refused` with "Branch Deck runs from a temporary location: move it to Applications, then
  install the Agent CLI"
  [@test] crates/bd-cli/src/install_test.rs (`a_translocated_app_is_refused`)

### Requirement: `brd install`: Without the symlink the skill names the app binary with `cli`

Without the symlink to this binary, `brd help skill` MUST name the app binary with `cli`, never the bare app binary,
which starts the app, quoted for the shell and to be run as written.

#### Scenario: No symlink to this binary
- **GIVEN** no symlink points at this binary
- **WHEN** `brd help skill` runs
- **THEN** it names the app binary with `cli`, quoted for the shell, to be run as written
  [@test] crates/bd-cli/src/install_test.rs (`the_brd_command_is_the_link_to_this_app_or_the_app_with_cli`)

### Requirement: `brd install`: `agent_cli_command` returns the command with `-C <repository>`

The app's `agent_cli_command(repository)` MUST return the same command with `-C <repository>` without writing anything,
quoted for the shell (POSIX quotes; on Windows PowerShell's `& '<brd.exe>'` with single quotes, and `powershell: true`):
the symlink when it points at the running app, else the app binary with `cli` (`brd.exe` on Windows).

#### Scenario: The command of a repository
- **WHEN** the app asks `agent_cli_command(repository)`
- **THEN** it gets the symlink when it points at the running app, else the app binary with `cli`, with
  `-C <repository>`, quoted for the shell, nothing written
- **AND** on Windows it gets `brd.exe` as PowerShell's `& '<brd.exe>'` with single quotes, and `powershell: true`
  [@test] crates/bd-cli/src/install_test.rs (`the_brd_command_is_the_link_to_this_app_or_the_app_with_cli`)

### Requirement: `brd install`: A binary named `brd` is the CLI itself

A binary named `brd` (a cargo build) MUST be the CLI itself and get no `cli`.

#### Scenario: A cargo build
- **WHEN** the binary is named `brd`
- **THEN** its command is the binary alone, with no `cli`
  [@test] crates/bd-cli/src/install_test.rs (`the_brd_command_is_the_link_to_this_app_or_the_app_with_cli`)

### Requirement: `brd install`: `brd update` names its next command the same way

`brd update` MUST name its next command (`session.next`) as `agent_cli_command` names brd: the symlink when it points at
the running app, else the app binary with `cli`, a binary named `brd` alone.

#### Scenario: The next command of an update
- **WHEN** `brd update` prints its next command
- **THEN** `session.next` names brd as `agent_cli_command` does

### Requirement: `brd install`: A short path of the binary has no `\\?\` prefix

A path of the binary shorter than 260 characters MUST have no `\\?\` prefix in the command brd names.

#### Scenario: A path on Windows
- **WHEN** the path of the binary is shorter than 260 characters
- **THEN** the command names it without the `\\?\` prefix

### Requirement: `brd help agent`: It is a short core every agent reads first

It MUST be a core of at most 10 KB, which every agent reads before grouping or pushing, the rules first.

#### Scenario: The size and the order of the core
- **WHEN** `brd help agent` runs
- **THEN** it prints at most 10 KB, the rules first
  [@test] crates/bd-cli/src/agent_help_test.rs (`the_agent_help_is_a_short_core_with_every_guard`)

### Requirement: `brd help agent`: It names what needs the user's approval

The rules MUST name what needs the user's approval: every push, Safe Push and Patronus Dry Run, review request,
`gh pr create`, park, unpark, unapply, reapply and change of another session's commit, a reword of its message too; and
parked branches and unapply.

#### Scenario: The writes that wait for the user
- **WHEN** an agent reads the rules of the core
- **THEN** every push, Safe Push and Patronus Dry Run, review request, `gh pr create`, park, unpark, unapply, reapply and
  change of another session's commit, a reword of its message too, needs the user's approval

### Requirement: `brd help agent`: It says which commands change commits and the working tree

The rules MUST say never to reorder, squash or drop commits yourself (`brd fold` folds the fixups, `brd squash` the
commits the user asked to squash, `brd update` drops the landed ones), a change moved only with `brd move-hunk`, a
message changed only with `brd reword`, only `brd update`, `brd pick`, `brd unapply`, `brd reapply` and their undo
writing the working tree.

#### Scenario: The commands of a rewrite
- **WHEN** an agent reads the rules of the core
- **THEN** it reads that `brd fold`, `brd squash` and `brd update` fold, squash and drop, never the agent itself
- **AND** that `brd move-hunk` moves a change, `brd reword` changes a message, and only `brd update`, `brd pick`,
  `brd unapply`, `brd reapply` and their undo write the working tree

### Requirement: `brd help agent`: It says what each exit code and the error object ask for

The rules MUST say what each exit code asks for (exit 1 never an error object, exit 4 without a rule: stop and show
`message`), that `retryable` decides a retry whatever the exit code, and the error object with `paths`, `branches`,
`review`, `processes` and `next_steps`.

#### Scenario: A failed command
- **WHEN** a command fails
- **THEN** the core tells the agent what its exit code asks for, that `retryable` decides a retry, and the fields of the
  error object

### Requirement: `brd help agent`: It says which printed commands run at once and which wait

The rules MUST say which printed commands run at once (`next` of `brd workspace`, `brd update` and `brd pick`) and
which wait for the user's approval (`push` of `brd sync`, `fix`, `next_steps` with `needs_approval`), and name
`--expect-head`, `previous_head`, `new_head`, the `undo_token` and `after_error`, abbreviated ids compared by prefix.

#### Scenario: A printed command
- **WHEN** a command prints a next command
- **THEN** the core tells whether the agent runs it at once or waits for the user's approval
- **AND** it names `--expect-head`, `previous_head`, `new_head`, the `undo_token` and `after_error`, ids compared by
  prefix

### Requirement: `brd help agent`: It names every refusal that waits for the user, with its flag

After the rules, the core MUST name every refusal that waits for the user, with the flag that only the user's word
allows: `push_too_large` (never raise the limit), `parked` (never push or unpark), `lease_rejected` (show
`dropped_commits`), `robot_running`, `approvals_missing` (`--without-approvals` only when the user says so),
`dangling_reference` and `missing_declaration` (only a false alarm), `rules_unknown`, `low_disk_space` (never
`--min-free 0`).

#### Scenario: A refusal that waits for the user
- **WHEN** a command is refused by one of these refusals
- **THEN** the core names it with the flag only the user's word allows
  [@test] crates/bd-cli/src/agent_help_test.rs (`the_agent_help_is_a_short_core_with_every_guard`)

### Requirement: `brd help agent`: It gives the workflow in steps and the other tasks

Then the core MUST give the workflow in steps (look with `brd suggest`; plan and check with the `result` of each
branch; apply with `brd group --sync`; push; Safe Push with `--dry-run` first, `lands` and `--whole-stack`; review;
robot; statistics), each step sending the agent to the `--help` of its command, then the other tasks (`brd update`,
`brd pick`, `brd fold`, `brd squash`, `brd reword` with its `(prefix)` kept and never an empty `amend!` or plumbing,
`brd move-hunk`, `brd install`).

#### Scenario: The workflow
- **WHEN** an agent reads the workflow of the core
- **THEN** each step, from `brd suggest` to the statistics, sends it to the `--help` of its command
- **AND** the other tasks follow, `brd reword` keeping its `(prefix)`, never an empty `amend!` or plumbing

### Requirement: `brd help agent`: The helps name every command and option that exists, and no other

Together with `brd help update`, `brd help worktrees` and the `--help` of every command it MUST name every command and,
per command, every long option except a listed few, and name no command, and no option for a command, that doesn't
exist.

#### Scenario: Commands and options against the CLI
- **WHEN** the helps are compared with the commands and options of the CLI
- **THEN** every command and every long option, but a listed few, is named, and nothing that doesn't exist
  [@test] crates/bd-cli/src/agent_help_test.rs (`agent_help_names_every_command_and_option_that_exists`)

### Requirement: `brd help agent`: Its first line names the version

Its first line MUST name the version.

#### Scenario: The first line
- **WHEN** `brd help agent` runs
- **THEN** its first line names the version
  [@test] crates/bd-cli/src/version_test.rs (`agent_help_names_the_version`)

### Requirement: `brd <command> --help`: It prints the fields of the output and what each refusal asks

The `--help` of `status`, `suggest`, `check`, `group`, `push`, `safe-push`, `request-review`, `robot`, `robot-stats`,
`fold`, `squash`, `reword` and `move-hunk` MUST print, after the options, the fields of the command's output and what
each of its refusals asks.

#### Scenario: The help of a command
- **WHEN** `brd <command> --help` runs for one of these commands
- **THEN** it prints, after the options, the fields of its output and what each of its refusals asks
  [@test] crates/bd-cli/src/agent_help_test.rs (`every_refusal_is_in_the_help_of_its_command`)

### Requirement: `brd <command> --help`: `-h` names `--help`, the group commands name `brd group --help`

`-h` MUST end with the line that names `--help`, and `rename`, `restack` and `unstack` MUST name `brd group --help`.

#### Scenario: The short help and the commands of a group plan
- **WHEN** `brd <command> -h` runs, or the help of `rename`, `restack` or `unstack`
- **THEN** `-h` ends with the line that names `--help`
- **AND** `rename`, `restack` and `unstack` name `brd group --help`
  [@test] crates/bd-cli/src/agent_help_test.rs (`every_help_topic_prints_its_text`)

### Requirement: `brd <command> --help`: `brd help <command>` prints the same

`brd help <command>` MUST print the same as `brd <command> --help`, its usage `brd <command>`.

#### Scenario: The help by its topic
- **WHEN** `brd help <command>` runs
- **THEN** it prints what `brd <command> --help` prints, its usage `brd <command>`
  [@test] crates/bd-cli/src/agent_help_test.rs (`every_help_topic_prints_its_text`)

### Requirement: `brd <command> --help`: Every error code is placed in a help

Every error code MUST be placed (an exhaustive match, so a new code can't be forgotten): a refusal an agent acts on MUST
be named in the core, a topic or the `--help` of its command.

#### Scenario: A refusal an agent acts on
- **WHEN** an error code is a refusal an agent acts on
- **THEN** the core, a topic or the `--help` of its command names it
- **AND** a new code fails the exhaustive match until it is placed
  [@test] crates/bd-cli/src/agent_help_test.rs (`every_refusal_is_in_the_help_of_its_command`)

### Requirement: `brd help worktrees`: It says how sessions lease and return their worktrees

It MUST say how sessions that share a checkout work in worktrees of their own: the lease of a worktree with
`brd lease get` (its `home` and `next`) and its return with `brd lease return` and their refusals.

#### Scenario: A session needs a worktree of its own
- **WHEN** an agent reads `brd help worktrees`
- **THEN** it reads how to lease a worktree with `brd lease get`, its `home` and `next`, and to return it with
  `brd lease return`, with their refusals

### Requirement: `brd help worktrees`: It says to write the `(branch)` prefix when a commit is made

It MUST say that the `(branch)` prefix is written when a commit is made: `(name)` for a new branch, `(parent/leaf)` only
on an existing virtual branch, never the baseline, a readable name, an issue id never the name, no `fixup!` subject on
the shared branch, `brd rename`, `brd suggest` only for commits without a prefix.

#### Scenario: The prefix first
- **WHEN** an agent reads how to commit in `brd help worktrees`
- **THEN** it writes the prefix when it commits, `(name)` or `(parent/leaf)`, and `brd suggest` is only for commits
  without a prefix
  [@test] crates/bd-cli/src/agent_help_test.rs (`the_worktrees_help_writes_the_prefix_first_and_checks_by_content`)

### Requirement: `brd help worktrees`: It says that one owner rewrites and the others only add

It MUST say that one owner rewrites the shared branch (`brd reword` among its rewrites) and every other session only
adds commits with `brd lease pick` from its lease (its refusals `home_branch_switched`, `no_home_branch`, `no_lease`)
or `brd pick --from <worktree> --follow` (both with `unchecked_tree` and `--allow-unchecked`), never with
`git cherry-pick` and never with `brd update` in a lease.

#### Scenario: A session that does not own the shared branch
- **WHEN** an agent reads `brd help worktrees`
- **THEN** it adds commits only with `brd lease pick` or `brd pick --from <worktree> --follow`
- **AND** never with `git cherry-pick`, and never runs `brd update` in a lease

### Requirement: `brd help worktrees`: The one rewrite of a session is a reword of its landed commit

It MUST say that the one rewrite of a session that does not own the shared branch is a `brd reword` of its own landed commit with
`-C <shared checkout>` and `--expect-head`, without `--sync`; and that after a rewrite a session's next write is
`head_moved`, which `--allow-append` does not cure.

#### Scenario: A reword of its own landed commit
- **WHEN** a session rewords its own landed commit
- **THEN** it runs `brd reword` with `-C <shared checkout>` and `--expect-head`, without `--sync`
- **AND** after a rewrite, its next write is `head_moved`, which `--allow-append` does not cure

### Requirement: `brd help worktrees`: It says the outcomes of a pick and checks by content

It MUST say the outcomes of a pick (`grouped_by: none`, `already_present`, `conflict.next`, `uncommitted_overlap`,
`range_unknown`, `source_on_newer_baseline`, `followed.state`, the undo), `brd pick --dry-run` as the check after a
rewrite, never a comparison by date, the protocol of the owner, and the `branch-deck:` reflog lines.

#### Scenario: After a rewrite
- **WHEN** an agent checks what landed after a rewrite
- **THEN** it runs `brd pick --dry-run`, never a comparison by date
  [@test] crates/bd-cli/src/agent_help_test.rs (`the_worktrees_help_writes_the_prefix_first_and_checks_by_content`)

#### Scenario: The outcomes, the protocol and the reflog
- **WHEN** an agent reads `brd help worktrees`
- **THEN** it reads each outcome of a pick, the protocol of the owner and the `branch-deck:` reflog lines

### Requirement: `brd help worktrees`: It prints the topic, and an unknown topic names it

`brd help worktrees` MUST print it, and an unknown topic MUST name it.

#### Scenario: The topic and an unknown one
- **WHEN** `brd help worktrees` runs, or `brd help` with an unknown topic
- **THEN** the first prints the topic, the second names `worktrees` among the topics
  [@test] crates/bd-cli/src/agent_help_test.rs (`every_help_topic_prints_its_text`)

### Requirement: `brd help worktrees`: It names only commands and options that exist

Together with `brd help agent` and `brd help update` it MUST name only commands and options that exist.

#### Scenario: Against the CLI
- **WHEN** the helps are compared with the commands and options of the CLI
- **THEN** `brd help worktrees` names none that doesn't exist
  [@test] crates/bd-cli/src/agent_help_test.rs (`agent_help_names_every_command_and_option_that_exists`)
