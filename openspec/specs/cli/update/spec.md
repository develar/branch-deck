---
name: brd update
description: Requirements for brd update, the agent's Update Project, its undo, and the resolve session in which an agent resolves a conflicting update commit by commit outside the working tree.
style: plain-1
targets:
  - crates/bd-update/src/update_command.rs
  - crates/bd-cli/src/agent_help.rs
  - crates/domain-types/src/error.rs
  - crates/bd-cli/src/schema.rs
  - crates/update-core/src/update_project.rs
  - crates/sync-core/src/fetch_lock.rs
  - crates/update-core/src/resolve_session.rs
  - crates/update-core/src/resolve_copies.rs
  - crates/sync-utils/src/paths.rs
  - crates/git-ops/src/update_resolve.rs
  - crates/git-ops/src/resolve_check.rs
  - crates/git-ops/src/merge_tree_output.rs
  - crates/git-ops/src/commit_writer.rs
  - crates/git-ops/src/update_branch.rs
  - crates/git-ops/src/revs.rs
  - crates/bd-cli/src/lib.rs
  - crates/update-core/src/error_cases.rs
  - crates/git-ops/src/word_replay.rs
---

# brd update

Status: Implemented
Date: 2026-10-15

## Purpose

`brd update` is the app's Update Project for a coding agent ([Update Project](../../repository/update-project/spec.md)):
it fetches the baseline and brings the checked out branch onto it. A conflict starts a resolve session. The session
replays the local commits outside the working tree and opens one round per conflicting commit. The agent edits copies
of the conflicting files, and `brd update --continue` commits each round. When nothing conflicts any more, the finish
moves the branch, and the files follow through the guarded update. All state of a session is in git; only the copies
that the agent edits are files. How it works: "Resolve session" in [Git Operations](../../../../docs/design/git-operations.md).
The rest of the CLI: [Agent CLI](../agent-cli/spec.md).

## Goals

- An agent updates and resolves conflicts in a working tree that other sessions share: no stash, no rebase in the
  working tree, no checkout.
- Peers may commit while the agent resolves: their commits become more rounds, not a failure.
- Every resolution is explicit and checked, and the user sees what each resolution dropped.
- A crash or a refusal at any step leaves a state that the next command completes or removes.

## Non-goals

- Binary, file/directory, rename shapes other than rename/rename 1→2, mode, symlink and submodule conflicts: a round refuses them.
- A partial `--continue`: a round needs a resolution for every conflicting file.
- Skipping a commit that the baseline likely has by its author, author time and subject only: brd offers `--landed`
  in its round. Only a proof (openspec/specs/repository/landed-commits/spec.md, "The proof") shows that the baseline has the
  change.
- Notes that follow a later rewrite of a resolved commit.

## Requirements

### Requirement: Update: `brd update` does what the app's Update Project does

`brd update` MUST do what the app's Update Project does: fetch the baseline, check the update, then, under the
repository lock, check that nothing moved and apply a clean update (`up_to_date`, `fast_forward`, `rebase`), the
files following through the guarded working-tree update.

#### Scenario: A fast forward
- **GIVEN** the baseline moved and the branch has no local commits
- **WHEN** `brd update` runs
- **THEN** the branch fast-forwards (`fast_forward`) and the files follow
  [@test] crates/bd-update/src/update_test.rs (`update_fast_forwards_and_the_undo_moves_the_files_back`)

#### Scenario: A rebase of the local commits
- **GIVEN** the branch has local commits and the baseline moved
- **WHEN** `brd update` runs
- **THEN** the local commits are replayed onto the baseline (`rebase`), the files following, and unrelated local
  changes stay
- **AND** a second update is `up_to_date`
  [@test] crates/bd-update/src/update_test.rs (`update_rebases_the_local_commits_and_keeps_unrelated_local_changes`)

### Requirement: Update: The output names the heads, the landed commits and the undo token

`brd update` MUST print the state, the previous and the new HEAD, the `replayed` count, the landed commits (`landed`)
and the undo token `update:<previous head>..<new head>` with full ids.

#### Scenario: The output of an update
- **WHEN** an update applies
- **THEN** it prints the state, the previous and the new HEAD, the `replayed` count and the landed commits (`landed`)
- **AND** the undo token `update:<previous head>..<new head>` with full ids
  [@test] crates/bd-update/src/update_test.rs (`update_fast_forwards_and_the_undo_moves_the_files_back`)

### Requirement: Update: Every local commit the update dropped is in `dropped`

Every local commit the update dropped MUST be in one list, `dropped`: `commit`, `subject`, the baseline commit it
landed as (`upstream`, when one is known) and its `proof`. `dropped_count` MUST count them.

#### Scenario: A commit the pairing left out
- **WHEN** the pairing left a local commit out
- **THEN** it is in `dropped` with its `proof`: `patch_id`, `replayed` or `changed_lines` (the pairing left it out)
  [@test] crates/bd-update/src/resolve_test.rs (`twins_in_another_order_are_left_out_by_their_proof_and_land`)

#### Scenario: A commit that its replay left without a change
- **WHEN** the replay of a local commit leaves it without a change
- **THEN** it is in `dropped` with the `proof` `empty` (its replay left it without a change, as `git rebase` drops it)
  [@test] crates/bd-update/src/update_test.rs (`a_commit_that_the_replay_leaves_without_a_change_is_dropped_as_empty`)

#### Scenario: A commit dropped as landed
- **WHEN** the agent drops a round's commit with `--landed`
- **THEN** it is in `dropped` with the `proof` `landed` (`--landed`)
  [@test] crates/bd-update/src/resolve_test.rs (`a_twin_that_differs_asks_the_user_and_drops_after_the_yes`)

#### Scenario: A commit the agent drops as empty
- **WHEN** the agent drops a round's commit with `--drop-empty`
- **THEN** it is in `dropped` with the `proof` `user` (`--drop-empty`)
  [@test] crates/bd-update/src/resolve_test.rs (`a_resolution_without_changes_is_refused_unless_the_agent_keeps_or_drops_it`)

### Requirement: Update: Which outputs print the list of dropped commits

The first output of a session, its repeat by `brd update`, `brd update --status`, the finish and an update that
applies at once MUST print the list; a round of `--continue` MUST print `dropped_count` only.

#### Scenario: The first output, the round of `--continue` and the finish
- **GIVEN** a session whose update left commits out
- **WHEN** its first output, a round of `--continue` and the finish print
- **THEN** the first output and the finish print the list
- **AND** the round of `--continue` prints `dropped_count` only
  [@test] crates/bd-update/src/resolve_test.rs (`twins_in_another_order_are_left_out_by_their_proof_and_land`)

#### Scenario: An update that applies at once
- **WHEN** an update that dropped a commit applies at once
- **THEN** it prints the list
  [@test] crates/bd-update/src/update_test.rs (`a_commit_that_the_replay_leaves_without_a_change_is_dropped_as_empty`)

### Requirement: Update: An output that needs a decision of the user asks for it

An output that needs a decision of the user MUST have `ask_user: true` and `reasons` (`code`, `message`):
`blocked_paths` in the first output of a session, `landed_differs` in the first output and in the round of a twin that
differs. Nothing else asks: a round of `--continue` MUST repeat `blocked_paths` without a question.

#### Scenario: Local changes that will block the finish
- **GIVEN** local changes on paths the update likely writes
- **WHEN** a session starts and `brd update` repeats its first output
- **THEN** that output has `ask_user: true` and the reason `blocked_paths`
- **AND** a round of `--continue` repeats `blocked_paths` without a question
  [@test] crates/bd-update/src/resolve_test.rs (`local_changes_that_will_block_the_finish_are_named_from_the_start`)

#### Scenario: A twin that differs
- **GIVEN** a candidate of the baseline whose change differs from the local commit
- **WHEN** the first output and the round of the twin print
- **THEN** they have `ask_user: true` and the reason `landed_differs`
  [@test] crates/bd-update/src/resolve_test.rs (`a_twin_that_differs_asks_the_user_and_drops_after_the_yes`)

### Requirement: Update: An error that waits for the user asks for it

An error whose code waits for the user (`uncommitted_overlap`, `empty_resolution`, `landed_differs`) MUST have
`ask_user: true`.

#### Scenario: An empty resolution
- **WHEN** a resolution leaves the commit without changes
- **THEN** the error `empty_resolution` has `ask_user: true`
  [@test] crates/bd-update/src/resolve_test.rs (`a_resolution_without_changes_is_refused_unless_the_agent_keeps_or_drops_it`)

#### Scenario: A drop of a twin that differs without the user's yes
- **WHEN** the agent drops a twin that differs without `--user`
- **THEN** the error `landed_differs` has `ask_user: true`
  [@test] crates/bd-update/src/resolve_test.rs (`a_twin_that_differs_asks_the_user_and_drops_after_the_yes`)

#### Scenario: Local changes in the way
- **WHEN** an update is refused with `uncommitted_overlap`
- **THEN** the error has `ask_user: true`

### Requirement: Update: `verify.paths` names the files to build and test

At exit 0 after a resolution, `verify.paths` MUST name the files to build and test: the files of the rounds that kept
their commit with `how` edited, commit, deleted or words, and the files of the commits an update resolved by words;
the first 100 (101 rather than one left out). A file of `onto` is the baseline's and is not in it; without such files
`verify` MUST be absent.

#### Scenario: An edited file
- **WHEN** a session finishes with a round resolved by an edit
- **THEN** `verify.paths` names its file
  [@test] crates/bd-update/src/resolve_test.rs (`a_conflict_opens_a_round_and_the_edited_copy_finishes_the_update`)

#### Scenario: An update resolved by words
- **WHEN** an update whose conflict resolves by words applies
- **THEN** `verify.paths` names the files of the commit the words resolved
  [@test] crates/bd-update/src/resolve_test.rs (`an_update_whose_conflict_resolves_by_words_applies_and_names_the_commit`)

#### Scenario: A drop leaves nothing to verify
- **WHEN** the only round drops its commit
- **THEN** `verify` is absent
  [@test] crates/bd-update/src/resolve_test.rs (`a_twin_with_every_line_of_the_commit_drops_without_a_question`)

### Requirement: Update: The fetch runs before the lock

The fetch MUST run before the lock and through the remote command policy; a failed fetch MUST be `network_error`
(exit 5, retryable). A held lock MUST be `locked` (exit 3) after the fetch.

#### Scenario: A failed fetch
- **WHEN** the fetch fails
- **THEN** the update is `network_error` (exit 5, retryable)
  [@test] crates/bd-update/src/update_test.rs (`a_failed_fetch_is_a_network_error`)

#### Scenario: A held lock
- **GIVEN** another process holds the repository lock
- **WHEN** `brd update` runs
- **THEN** the fetch runs, and the update is `locked` (exit 3) after it
  [@test] crates/bd-update/src/update_test.rs (`the_update_waits_for_the_repository_lock_after_its_fetch`)

### Requirement: Update: Two fetches of the baseline make one transfer

Beside another fetch of the baseline (the app's Update Project or Fetch, another `brd update`: one common dir), the two
fetches MUST make one transfer: the second one waits while the first one's process lives and takes its result once the
tracking ref moved; a first one that failed or was killed leaves the fetch to the second
([Git Operations](../../../../docs/design/git-operations.md#update-project), Fetch lock).

#### Scenario: The fetch of the app beside `brd update`
- **WHEN** the app fetches the baseline beside a `brd update`
- **THEN** the two make one transfer
  [@test] crates/bd-update/src/update_test.rs (`the_fetch_of_the_app_beside_bd_update_takes_its_transfer`)

#### Scenario: The second fetch takes the result of the first
- **WHEN** a fetch of the tracking ref runs beside another one
- **THEN** it waits while the first one's process lives and takes its result once the tracking ref moved
  [@test] crates/sync-core/src/fetch_lock_test.rs (`a_fetch_beside_another_fetch_of_its_tracking_ref_takes_its_result`)

#### Scenario: The first fetch failed
- **WHEN** the fetch that the second one waited for failed or was killed
- **THEN** the second one fetches itself
  [@test] crates/sync-core/src/fetch_lock_test.rs (`a_fetch_that_waited_for_one_that_failed_fetches_itself`)

### Requirement: Update: `--no-fetch` updates onto the last fetched baseline

`--no-fetch` MUST update onto the last fetched baseline.

#### Scenario: An update without a fetch
- **GIVEN** a baseline fetched before
- **WHEN** `brd update --no-fetch` runs
- **THEN** the branch is updated onto the last fetched baseline
  [@test] crates/bd-update/src/update_test.rs (`dry_run_neither_fetches_nor_writes`)

### Requirement: Update: `--dry-run` fetches nothing, locks nothing and writes at most objects

`--dry-run` MUST imply `--no-fetch`, take no lock and print what the update would do, writing at most git objects.

#### Scenario: A dry run of a clean update
- **WHEN** `brd update --dry-run` runs
- **THEN** it prints what the update would do, without a fetch and without a lock
- **AND** it writes at most git objects
  [@test] crates/bd-update/src/update_test.rs (`dry_run_neither_fetches_nor_writes`)

### Requirement: Update: A dry run of a conflict prints the first conflict and changes nothing

A dry run of a conflict MUST print `state: "conflict"` with the first conflict: the files of its merge, the commit
(`commits`) and the upstream commits that change the files; start no session, change nothing, and exit with 1.

#### Scenario: A dry run of a conflict
- **WHEN** `brd update --dry-run` meets a conflict
- **THEN** it prints `state: "conflict"` with the files of the merge, the commit (`commits`) and the upstream commits
  that change the files
- **AND** it starts no session, changes nothing, and exits with 1
  [@test] crates/bd-update/src/update_test.rs (`a_dry_run_reports_the_files_and_commits_of_a_conflict_and_changes_nothing`)

### Requirement: Update: A dry run of a wide conflict lists the first files

The output goes into an agent's context: `files` MUST list the first 50, the ones with their commits, and
`files_omitted` MUST count the rest; never one left out: 51 are all listed, with their commits.

#### Scenario: More than 50 files conflict
- **WHEN** the dry run of a conflict has more files than 51
- **THEN** `files` lists the first 50, with their commits, and `files_omitted` counts the rest
  [@test] crates/bd-update/src/update_test.rs (`a_dry_run_of_a_wide_conflict_lists_the_first_files_and_counts_the_rest`)

#### Scenario: 51 files conflict
- **WHEN** the dry run of a conflict has 51 files
- **THEN** all 51 are listed, with their commits

### Requirement: Update: An update whose every conflict resolves by words applies at once

With `branchdeck.resolveByWords` on (the default), an update whose every conflict of git resolves by words MUST apply
as a clean `rebase`, with no session. The commit that the words resolve MUST keep the author and the message of the
local commit.

#### Scenario: The words resolve every conflict
- **GIVEN** `branchdeck.resolveByWords` on
- **WHEN** every conflict of git of an update resolves by words
- **THEN** the update applies as a clean `rebase`, with no session
- **AND** the resolved commit keeps the author and the message of the local commit, and the rest replays onto it
  [@test] crates/git-ops/src/update_resolve_test.rs (`an_update_whose_every_conflict_resolves_by_words_applies_without_a_session`)
  [@test] crates/git-ops/src/word_replay_test.rs (`a_conflict_that_resolves_by_words_is_committed_and_the_rest_replays_onto_it`)

#### Scenario: The words of a commit conflict too
- **WHEN** the words of a conflicting commit do not resolve its conflict
- **THEN** the replay stops there, and the update is no clean `rebase`
  [@test] crates/git-ops/src/word_replay_test.rs (`a_commit_whose_words_conflict_too_stops_the_replay`)

### Requirement: Update: The output lists each commit the words resolved

The output MUST list each commit the words resolved in `resolved`: the `commit`, its `new_id` (omitted for a dry run),
`conflicts`, `file_count` and `files` with their conflicts.

#### Scenario: The output of an update resolved by words
- **WHEN** an update whose conflict resolves by words applies
- **THEN** `resolved` lists the `commit`, its `new_id`, `conflicts`, `file_count` and `files` with their conflicts
  [@test] crates/bd-update/src/resolve_test.rs (`an_update_whose_conflict_resolves_by_words_applies_and_names_the_commit`)

#### Scenario: A dry run
- **WHEN** a dry run of such an update prints `resolved`
- **THEN** each entry omits `new_id`

### Requirement: Update: Each commit the words resolve gets a resolve note

Each commit the words resolve MUST get a note in `refs/notes/branch-deck/resolve` with `resolved-by-words` and a
`words` line per file.

#### Scenario: The note of a commit resolved by words
- **WHEN** an update whose conflict resolves by words applies
- **THEN** its commit has a note in `refs/notes/branch-deck/resolve` with `resolved-by-words` and a `words` line per
  file
  [@test] crates/bd-update/src/resolve_test.rs (`an_update_whose_conflict_resolves_by_words_applies_and_names_the_commit`)

### Requirement: Update: An operation in progress refuses the update

A rebase, merge, cherry-pick or revert in progress MUST be `operation_in_progress` (exit 4) for the update and its dry
run, moving neither the branch nor the files.

#### Scenario: A cherry-pick in progress
- **GIVEN** a cherry-pick in progress
- **WHEN** `brd update` or `brd update --dry-run` runs
- **THEN** it is `operation_in_progress` (exit 4), and neither the branch nor the files move
  [@test] crates/bd-update/src/update_test.rs (`update_and_its_dry_run_are_refused_during_a_cherry_pick`)

### Requirement: Update: A checked out branch under the prefix is `managed_branch`

A checked out branch under the prefix (a virtual, unapplied, archived or stack branch) MUST be `managed_branch` (exit
4), before the fetch, moving neither the branch nor the tracking ref: the update would rebase a copy of one group
instead of the user's branch.

#### Scenario: An update of a branch under the prefix
- **GIVEN** a branch under the prefix checked out
- **WHEN** `brd update` runs
- **THEN** it is `managed_branch` (exit 4) before the fetch, and neither the branch nor the tracking ref moves
  [@test] crates/update-core/src/update_project_test.rs (`an_update_of_a_branch_under_the_prefix_is_refused_before_the_fetch`)

### Requirement: Update: A `git switch` between the check and the move moves no branch

A `git switch` between the check and the move MUST move no branch: `head_moved` for the update, `undo_stale` for the
undo.

#### Scenario: A switch before the move of the update or of the undo
- **WHEN** a `git switch` runs between the check and the move
- **THEN** no branch moves: the update is `head_moved`, the undo `undo_stale`
  [@test] crates/bd-update/src/update_test.rs (`a_switch_between_the_check_and_the_move_moves_no_branch`)

#### Scenario: A switch during the move
- **WHEN** a `git switch` runs while the update moves the branch
- **THEN** the switch fails on `HEAD.lock`, and the update moves its own branch only
  [@test] crates/bd-update/src/update_test.rs (`a_switch_during_the_move_fails_on_the_head_lock`)

### Requirement: Update: A lock file of git in the way of the move is `locked`

A lock file of git in the way of the move (`HEAD.lock` or the lock file of the branch), which a killed git can leave,
MUST be `locked` (exit 3, retryable) with the path of the file in the message, and move nothing.

#### Scenario: A stale `HEAD.lock`
- **GIVEN** a `HEAD.lock` that a killed git left
- **WHEN** `brd update` runs
- **THEN** it is `locked` (exit 3, retryable) with the path of the file in the message, and nothing moves
  [@test] crates/bd-update/src/update_test.rs (`a_stale_head_lock_is_locked_and_names_the_file`)

### Requirement: Update: A move killed before its files followed is finished first

A `brd update` or `brd undo` killed between the move of the branch and its files MUST leave the mark
`refs/branch-deck/update-pending/<worktree id>`, which the move creates in its ref transaction. The next `brd update`,
`brd undo` or Update Project of the app MUST let the files follow first.

#### Scenario: An update killed before its files followed
- **WHEN** a `brd update` is killed between the move of the branch and its files
- **THEN** the mark `refs/branch-deck/update-pending/<worktree id>` stays
- **AND** the next `brd update` lets the files follow first and removes the mark
  [@test] crates/bd-update/src/update_test.rs (`an_update_killed_before_its_files_followed_is_finished_by_the_next_update`)

#### Scenario: An undo killed before its files followed
- **WHEN** a `brd undo` is killed between the move of the branch and its files
- **THEN** the next `brd undo` lets the files follow first
  [@test] crates/bd-update/src/update_test.rs (`an_undo_killed_before_its_files_followed_is_finished_by_the_next_undo`)

### Requirement: Update: An interrupted move of a branch that moved since is `finish_pending`

When the branch moved since the interrupted move, the result MUST be `finish_pending` (exit 4), changing nothing, with
the command that removes the mark.

#### Scenario: The branch moved since
- **GIVEN** the mark of an interrupted move, and a branch that moved since
- **WHEN** `brd update` runs
- **THEN** it is `finish_pending` (exit 4), changes nothing, and names the command that removes the mark
  [@test] crates/bd-update/src/update_test.rs (`an_interrupted_update_of_a_branch_that_moved_since_is_reported`)

### Requirement: Update: `--status` prints the mark of an interrupted move

`brd update --status` MUST print the mark as `pending_update`, with `next`.

#### Scenario: The status of an interrupted update
- **GIVEN** the mark of an interrupted update
- **WHEN** `brd update --status` runs
- **THEN** it prints `pending_update`, with `next`
  [@test] crates/bd-update/src/update_test.rs (`an_update_killed_before_its_files_followed_is_finished_by_the_next_update`)

### Requirement: Update: With `core.autocrlf=true` the files get the line ends of a checkout

With `core.autocrlf=true`, the files of an update and of its undo MUST get the line ends of a checkout, and the status
MUST stay clean.

#### Scenario: An update and its undo with CRLF line ends
- **GIVEN** `core.autocrlf=true`
- **WHEN** an update and its undo run
- **THEN** the files get the line ends of a checkout and the status stays clean
  [@test] crates/bd-update/src/update_test.rs (`an_update_with_autocrlf_checks_out_crlf_and_leaves_a_clean_status`)

### Requirement: Update: Local changes on paths the update writes are `uncommitted_overlap`

Uncommitted, staged or untracked files on paths the update writes MUST be `uncommitted_overlap` (exit 4, not
retryable) with those `paths`, also for a dry run; brd MUST never stash, reset, check out or commit them.

#### Scenario: Local changes on an updated file
- **GIVEN** a local change on a file the update writes
- **WHEN** `brd update` or its dry run runs
- **THEN** it is `uncommitted_overlap` (exit 4, not retryable) with those `paths`
- **AND** the change stays, never stashed, reset, checked out or committed
  [@test] crates/bd-update/src/update_test.rs (`uncommitted_changes_on_updated_files_block_with_their_paths`)

### Requirement: Update: `paths` of an overlap lists the first 100

`paths` MUST list the first 100 (all 101 rather than one left out); with more, the message MUST say "`paths` lists the
first 100 of <n>: `git status` lists all".

#### Scenario: Many blocking paths
- **WHEN** more blocking paths than 101 refuse the update
- **THEN** `paths` lists the first 100 and the message says "`paths` lists the first 100 of <n>: `git status` lists all"
- **AND** 101 blocking paths are all listed
  [@test] crates/bd-update/src/update_test.rs (`many_blocking_paths_list_the_first_100_and_say_how_many`)

### Requirement: Update: `brd undo update:` moves the branch back with the files following

`brd undo update:<previous head>..<new head>` MUST take only full ids and move the branch back with the files
following: checked before the lock, which holds only the witness of HEAD and of the mark of an interrupted move, and
the move back; the landed report after it.

#### Scenario: The undo of an update
- **WHEN** `brd undo` runs with the token of an update
- **THEN** the branch moves back and the files follow
  [@test] crates/bd-update/src/update_test.rs (`update_fast_forwards_and_the_undo_moves_the_files_back`)

#### Scenario: Only the witness and the move under the lock
- **WHEN** the undo runs
- **THEN** its check runs before the lock, which holds only the witness of HEAD and of the mark and the move back
  [@test] crates/update-core/src/update_project_test.rs (`the_undo_of_an_update_locks_only_the_witness_and_the_move`)

#### Scenario: A token with short ids
- **WHEN** the token names a short id
- **THEN** the undo refuses it
  [@test] crates/update-core/src/update_project_test.rs (`undo_tokens_are_strict`)

### Requirement: Update: The undo refuses overlaps, a moved branch and a token of no last update

The undo MUST refuse with `uncommitted_overlap` when local changes overlap the paths it writes, `undo_stale` (exit 4,
not retryable) when the branch moved or HEAD switched since, and `invalid_arguments` when the token is not the last
update of the branch.

#### Scenario: Local changes overlap, or the branch moved
- **WHEN** local changes overlap the paths the undo writes, or the branch moved since
- **THEN** it is `uncommitted_overlap` with the paths, or `undo_stale` (exit 4, not retryable)
  [@test] crates/bd-update/src/update_test.rs (`the_undo_is_refused_when_local_changes_overlap_or_the_branch_moved`, `update_outputs_match_the_schema`)

#### Scenario: A token of an older update
- **WHEN** the token is not the last update of the branch
- **THEN** it is `invalid_arguments`
  [@test] crates/update-core/src/update_project_test.rs (`undo_needs_the_token_of_the_last_update`)

### Requirement: Update: The undo says whether the landed report is restored

The undo MUST print whether the landed report is the one from before the update again (`landed_restored`).

#### Scenario: The landed report of an undo
- **WHEN** an undo runs
- **THEN** it prints `landed_restored`
  [@test] crates/bd-update/src/update_test.rs (`update_outputs_match_the_schema`)

### Requirement: Update: An undo of an undone update exits 0

An undo of an undone update MUST exit 0 with `already_undone: true`.

#### Scenario: The same undo twice
- **WHEN** the same undo runs a second time
- **THEN** it exits 0 with `already_undone: true`
  [@test] crates/bd-update/src/update_test.rs (`update_fast_forwards_and_the_undo_moves_the_files_back`)

### Requirement: Update: The finish of a resolve session prints the same undo token

The finish of a resolve session MUST print the same token, and the same undo MUST take it back.

#### Scenario: The undo of a finished session
- **WHEN** a resolve session finishes
- **THEN** it prints the token `update:<previous head>..<new head>`, and `brd undo` with it takes the update back
  [@test] crates/bd-update/src/resolve_test.rs (`a_conflict_opens_a_round_and_the_edited_copy_finishes_the_update`)

### Requirement: In a lease: An update onto the baseline is `lease_follows_home`

In a lease of the pool `brd update` onto the baseline MUST be refused as `lease_follows_home` (exit 4), the message
naming `brd lease pick` and the owner of the shared checkout: the lease follows the branch it was cut from, and an
update onto a newer baseline would make its pick refuse it (`source_on_newer_baseline`).

#### Scenario: `brd update` in a lease
- **GIVEN** a worktree leased from the pool
- **WHEN** `brd update` runs there
- **THEN** it is `lease_follows_home` (exit 4), the message naming `brd lease pick` and the owner of the shared
  checkout
  [@test] crates/bd-lease/src/lease_pick_test.rs (`an_update_in_a_lease_is_refused_and_its_session_is_closed_with_abort`)

### Requirement: In a lease: The other modes of `brd update` go on in a lease

`--dry-run`, `--status`, `--continue` and `--abort` SHALL go on in a lease; a host slot of `brd-host` is no lease of a
session.

#### Scenario: A session in a lease is closed with `--abort`
- **GIVEN** a resolve session open in a lease
- **WHEN** `brd update --abort` runs there
- **THEN** the session is closed
  [@test] crates/bd-lease/src/lease_pick_test.rs (`an_update_in_a_lease_is_refused_and_its_session_is_closed_with_abort`)

#### Scenario: A host slot of `brd-host`
- **WHEN** `brd update` runs in a host slot of `brd-host`
- **THEN** it is not refused as in a lease of a session

### Requirement: In a lease: The update of a conflicted pick goes onto the branch the lease was cut from

The update that `brd lease pick` runs after its pick conflicted MUST go onto the branch the lease was cut from, a local
branch, without a fetch (`UpdateOptions::onto`): on the detached HEAD of the lease (`branch: "HEAD"`), the fork point
in the reflog of that branch seeing through a move that was rolled back (`branch_fork_point`).

#### Scenario: An update of a detached HEAD onto a local branch
- **GIVEN** the detached HEAD of a lease and a local branch with a new commit
- **WHEN** the update runs onto that branch
- **THEN** the lease's commits are replayed onto the branch, without a fetch, on `branch: "HEAD"`, and HEAD stays
  detached
  [@test] crates/update-core/src/update_project_test.rs (`an_update_onto_a_local_branch_moves_a_detached_head_and_its_undo_moves_it_back`)

### Requirement: In a lease: The update onto a local branch writes no landed report

The update onto the branch the lease was cut from MUST write no landed report, at its apply or at the finish of its
session: nothing reached the baseline.

#### Scenario: The apply writes no landed report
- **WHEN** the update onto a local branch applies
- **THEN** it lands nothing and writes no landed report
  [@test] crates/update-core/src/update_project_test.rs (`an_update_onto_a_local_branch_moves_a_detached_head_and_its_undo_moves_it_back`)

#### Scenario: The finish of its session writes no landed report
- **WHEN** the resolve session of such an update finishes
- **THEN** it writes no landed report

### Requirement: In a lease: A conflict of the update starts a resolve session in the lease

A conflict of the update that `brd lease pick` runs MUST start a resolve session as below, with `upstream` the branch;
the finish MUST print `next`, `brd -C <lease> lease pick`.

#### Scenario: A lease pick that conflicts
- **WHEN** the pick of a lease conflicts
- **THEN** a resolve session opens in the lease with `upstream` the branch
- **AND** its finish prints `next`, `brd -C <lease> lease pick`, which lands the lease
  [@test] crates/bd-lease/src/lease_pick_test.rs (`a_lease_pick_that_conflicts_opens_a_resolve_session_in_the_lease_and_lands_after_it`)

#### Scenario: The processes of a lease pick that conflicts
- **WHEN** the pick of a lease conflicts and its update opens the session
- **THEN** each runs the git processes of its budget
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_lease_pick_that_conflicts`)

### Requirement: In a lease: The undo moves the detached HEAD back by the reflog of `HEAD`

`brd undo update:` MUST move the detached HEAD of the lease back by the reflog of `HEAD`.

#### Scenario: The undo of an update of a detached HEAD
- **WHEN** the undo of an update onto a local branch runs
- **THEN** the detached HEAD moves back
  [@test] crates/update-core/src/update_project_test.rs (`an_update_onto_a_local_branch_moves_a_detached_head_and_its_undo_moves_it_back`)

### Requirement: Resolve session: A conflict starts a session and changes only its ref and copies

A conflict of `brd update` MUST start a resolve session, and change no ref but the session ref and no file but the
copies: HEAD, the index and the working tree stay. The output is `state: "conflict"` with a `session` object, and the
exit code MUST be 1.

#### Scenario: The start of a session
- **WHEN** `brd update` meets a conflict
- **THEN** it prints `state: "conflict"` with a `session` object and exits with 1
- **AND** only the session ref and the copies are written: HEAD, the index and the working tree stay
  [@test] crates/bd-update/src/resolve_test.rs (`a_conflict_opens_a_round_and_the_edited_copy_finishes_the_update`)

### Requirement: Resolve session: The first round is computed before the repository lock

The search for the first conflict and the first round (its replays, its merge, its objects) MUST run before the
repository lock, on the HEAD and the upstream the check read; under it only the witness of the update, the state with
the session ref and the copies of the round. A HEAD or an upstream that moved meanwhile (another process regrouped the
branch) MUST check and open the round once more under the lock, on what it finds there.

#### Scenario: Only the witness and the write under the lock
- **WHEN** a session starts
- **THEN** the lock holds only the witness of the update and the write of the state with the session ref and the copies
  [@test] crates/update-core/src/update_project_test.rs (`the_start_of_a_resolve_session_locks_only_the_witness_and_its_write`)

#### Scenario: A regroup during the search for the first conflict
- **WHEN** another process regroups the branch while the first conflict is searched
- **THEN** the regroup is done, and the session checks and opens its round once more under the lock, on what it finds
  [@test] crates/update-core/src/update_project_test.rs (`a_regroup_during_the_first_conflict_search_of_a_resolve_session_is_done_and_the_session_opens_on_it`)

### Requirement: Resolve session: A session belongs to one worktree and one branch

The session MUST belong to one worktree and one branch, and keep the baseline it started with. Its ref MUST be the
shared `refs/branch-deck/resolve/<worktree id>` (`main`, or the name of a linked worktree), so that a `gc --prune=now`
in another worktree keeps its objects. In another worktree, `--continue` MUST be `no_session`.

#### Scenario: A gc in another worktree
- **GIVEN** a session open in one worktree
- **WHEN** `gc --prune=now` runs in another worktree
- **THEN** the objects of the session are kept under `refs/branch-deck/resolve/<worktree id>`
  [@test] crates/bd-update/src/resolve_test.rs (`a_session_belongs_to_its_worktree_and_survives_a_gc_in_another`)

#### Scenario: `--continue` in another worktree
- **GIVEN** a session open in one worktree
- **WHEN** `brd update --continue` runs in another worktree
- **THEN** it is `no_session`
  [@test] crates/bd-update/src/resolve_test.rs (`a_session_belongs_to_its_worktree_and_survives_a_gc_in_another`)

### Requirement: Resolve session: The first round opens at the first conflict, found by a binary search

The first round MUST open at the first conflict of the check, after one replay of the commits before it: the check
found it with a binary search over the prefixes, each probe from the longest clean prefix, at most ⌈log₂ n⌉ replays
after its own replay. After a resolution, one replay of the rest MUST run, and a search only when it conflicts.

#### Scenario: A conflict in the middle of many commits
- **GIVEN** n local commits and a conflict at one of them
- **WHEN** `brd update` opens the first round
- **THEN** the round is at the first conflict, after at most ⌈log₂ n⌉ replays of the search after its own replay
  [@test] crates/bd-update/src/resolve_test.rs (`the_first_conflict_is_found_with_a_binary_search`)

### Requirement: Resolve session: Every commit of the session has the committer of its start

Every commit the session writes MUST have the committer of the session start (name, email and date), so the same input
gives the same ids; `--continue --dry-run` MUST report the `new_id` that `--continue` writes.

#### Scenario: The ids of a dry run and of the real `--continue`
- **WHEN** `--continue --dry-run` and then `--continue` run on the same resolution
- **THEN** the dry run reports the `new_id` that `--continue` writes
  [@test] crates/bd-update/src/resolve_test.rs (`a_dry_run_of_continue_checks_and_gives_the_ids_of_the_real_one`)

### Requirement: Resolve session: `brd update` with a session open prints it again

`brd update` with a session open MUST fetch nothing and change nothing: it prints the open round again and writes its
missing copies. With no round open (the finish is next), it MUST print `state: "pending"` (exit 1) and `session.next`
`<bd> update --continue`.

#### Scenario: A round is open
- **GIVEN** a session with an open round
- **WHEN** `brd update` runs
- **THEN** it fetches nothing, changes nothing and prints the open round again
  [@test] crates/bd-update/src/resolve_test.rs (`a_conflict_opens_a_round_and_the_edited_copy_finishes_the_update`)

#### Scenario: The finish is next
- **GIVEN** a session with no round open
- **WHEN** `brd update` runs
- **THEN** it prints `state: "pending"` (exit 1) and `session.next` `<bd> update --continue`
  [@test] crates/bd-update/src/resolve_test.rs (`local_changes_on_the_finish_keep_the_session_ready`)

### Requirement: Resolve session: Commits of peers on top join the session

Commits that peers make on top of the session's HEAD MUST join the session as more local commits, and a conflict among
them opens a round.

#### Scenario: Peers commit on top
- **GIVEN** an open session
- **WHEN** a peer commits on top of the session's HEAD, and the commit conflicts
- **THEN** the commit joins the session and its conflict opens a round
  [@test] crates/bd-update/src/resolve_test.rs (`peers_commits_on_top_become_more_rounds`)

### Requirement: Resolve session: A HEAD without the session's HEAD makes it `session_stale`

A HEAD that no longer contains the session's HEAD, or that left the branch, MUST be `session_stale` (exit 4, not
retryable); the session stays for `--abort`.

#### Scenario: A rewritten HEAD
- **GIVEN** an open session
- **WHEN** HEAD is rewritten so that it no longer contains the session's HEAD
- **THEN** `--continue` is `session_stale` (exit 4, not retryable), and `--abort` removes the session
  [@test] crates/bd-update/src/resolve_test.rs (`a_rewritten_head_makes_the_session_stale_and_abort_removes_it`)

### Requirement: Resolve session: A history git can't read is an error, never `session_stale`

A history that git can't read (an object that is not there) MUST be an error with git's message, never
`session_stale`: only exit 1 of `merge-base --is-ancestor` is "no".

#### Scenario: A session HEAD git cannot read
- **WHEN** an object of the session's history is not there
- **THEN** the result is an error with git's message, not `session_stale`
  [@test] crates/git-ops/src/update_resolve_test.rs (`a_session_head_git_cannot_read_is_an_error_not_a_rewrite`)

### Requirement: Rounds: A round prints its session, its commit, its files and `next`

A round MUST print `session` with the `worktree`, the `state` (`resolving`, `ready`, `applying`, `applied`), the
`round`, `replayed` and `total`, the `dir` of the copies, the `commit` (`commit`, `subject`, `body`, `author`, the
absolute path of its `patch`), the conflicting `files`, `marker_labels` (the full ids after the markers: `onto`,
`before` as `<commit>^`, and `commit`), and `next`, the command that continues.

#### Scenario: The output of a round
- **WHEN** a round opens
- **THEN** it prints `session` with the `round`, `replayed` and `total`, the `commit` with its `patch`, and the
  conflicting `files`
  [@test] crates/bd-update/src/resolve_test.rs (`a_conflict_opens_a_round_and_the_edited_copy_finishes_the_update`)

#### Scenario: The schema of a round
- **WHEN** the outputs of a session are checked against `brd schema`
- **THEN** every property of `session` is declared
  [@test] crates/bd-update/src/resolve_test.rs (`session_outputs_match_the_schema`)

#### Scenario: The marker labels
- **WHEN** a round opens
- **THEN** `marker_labels` names the full ids after the markers: `onto`, `before` as `<commit>^`, and `commit`
  [@test] crates/bd-update/src/resolve_test.rs (`markers_are_found_by_their_labels`)

### Requirement: Rounds: `next` names this binary in this repository

`next` MUST name this binary in this repository, as the app's prompt does
(`<bd> -C <repo> update --continue --round <n>`; `brd` when the binary is unknown).

#### Scenario: The command that continues
- **WHEN** a round opens
- **THEN** `next` is `<bd> -C <repo> update --continue --round <n>` with the path of this binary, or `brd` when the
  binary is unknown

### Requirement: Rounds: A round does not repeat the rounds resolved before it

A round MUST NOT repeat the rounds resolved before it: each output of a long session would grow with all of them in
the agent's context. `session.resolutions` (the rounds so far) MUST be printed by `brd update --status` only; the
finish prints all in `resolutions`.

#### Scenario: The second round
- **WHEN** the second round opens
- **THEN** it has no `session.resolutions`
- **AND** the finish prints both rounds in `resolutions`
  [@test] crates/bd-update/src/resolve_test.rs (`a_second_conflict_opens_a_second_round_with_its_own_copies`)

#### Scenario: `--status` prints the rounds so far
- **WHEN** `brd update --status` runs in a session with a resolved round
- **THEN** it prints `session.resolutions`
  [@test] crates/bd-update/src/resolve_test.rs (`local_changes_on_the_finish_keep_the_session_ready`)

### Requirement: Rounds: A round names the local changes that will block the finish

A round MUST print `session.blocked_paths`: the uncommitted, staged or untracked files on paths the update likely
writes (the paths the upstream changed since the merge base that differ between HEAD and the upstream, with the check
of every sync), the first 100 (101 rather than one left out), and `blocked_paths_omitted` the count of the rest.

#### Scenario: A local edit of a file the update writes
- **GIVEN** a local edit of a file the upstream changed
- **WHEN** a round prints
- **THEN** `session.blocked_paths` names it
  [@test] crates/bd-update/src/resolve_test.rs (`local_changes_that_will_block_the_finish_are_named_from_the_start`)

#### Scenario: More blocked paths than 101
- **WHEN** more blocked paths than 101 are found
- **THEN** `session.blocked_paths` lists the first 100 and `blocked_paths_omitted` counts the rest

### Requirement: Rounds: The finish runs the exact check of the blocked paths

The blocked paths block the finish, so the agent asks the user about them first. The finish MUST run the exact check.

#### Scenario: The finish checks the paths it writes
- **WHEN** the finish runs
- **THEN** it checks exactly the paths it writes, not the likely ones of `session.blocked_paths`

### Requirement: Rounds: Each file names its kind, its copies, its markers and its upstream commits

Each file MUST have its `path`, its `kind` (`content`, `modify_delete`), the absolute paths of `edit` (the merge result
with zdiff3 markers), `before` (the commit's parent), `onto` (the new parent: the baseline and the commits replayed so
far) and `commit` (the commit's version), a side left out when it has no such file, `markers` (the blocks left in
`edit` now), a `hint`, and the newest 10 `upstream_commits` that changed it since the merge base.

#### Scenario: A content conflict
- **WHEN** a round of a `content` conflict opens
- **THEN** its file has `kind`, the copies `edit`, `before`, `onto` and `commit`, `markers` and `upstream_commits`
  [@test] crates/bd-update/src/resolve_test.rs (`a_conflict_opens_a_round_and_the_edited_copy_finishes_the_update`)

#### Scenario: A side without the file
- **WHEN** a round of a `modify_delete` conflict opens
- **THEN** the side that has no such file is left out
  [@test] crates/bd-update/src/resolve_test.rs (`modify_delete_is_resolved_with_take_and_delete`)

### Requirement: Rounds: On Windows the path of a copy has backslashes

On Windows every path of a copy MUST have backslashes, and the `\\?\` form only from 260 characters on, which Windows
PowerShell 5.1 opens without long paths enabled.

#### Scenario: A short and a long path on Windows
- **WHEN** a path of a copy is printed on Windows
- **THEN** it has backslashes, and the `\\?\` form only from 260 characters on
  [@test] crates/sync-utils/src/paths_test.rs (`a_windows_path_has_backslashes_and_the_verbatim_form_only_from_260_characters`)

### Requirement: Rounds: A local commit that the baseline proves it has is left out

A local commit that the baseline proves it has (openspec/specs/repository/landed-commits/spec.md, "The proof") MUST be
left out of the replay, as `git rebase` leaves out a commit of the same patch id, and listed in `dropped` with that
baseline commit and its `proof` (`patch_id`, `replayed`, `changed_lines`); it is in `landed`.

#### Scenario: The same patch id
- **WHEN** the baseline has a commit of the same patch id
- **THEN** the local commit is left out and listed in `dropped` with the `proof` `patch_id`
  [@test] crates/bd-update/src/resolve_test.rs (`a_commit_the_upstream_has_with_the_same_patch_is_left_out`)

#### Scenario: The same author, author time and subject with a replay that proves it
- **WHEN** the baseline has a commit of the same author, author time and subject, and a replay writes the baseline
  commit's tree or the same changed lines
- **THEN** the local commit is left out, listed in `dropped` with the `proof` `replayed` or `changed_lines`, and in
  `landed`
  [@test] crates/bd-update/src/resolve_test.rs (`twins_in_another_order_are_left_out_by_their_proof_and_land`)

### Requirement: Rounds: The commits around a left-out commit replay as runs

The commits around a left-out commit MUST replay as runs, so a conflict after it still opens its round, and the session
pairs nothing again.

#### Scenario: A conflict after a left-out commit
- **WHEN** a commit after a left-out one conflicts
- **THEN** its round opens
  [@test] crates/bd-update/src/resolve_test.rs (`a_conflict_after_a_left_out_commit_opens_its_round`)

### Requirement: Rounds: A clean update leaves out a twin too

A clean update MUST leave out a twin too, and the same patch of its author under another key, whose replay would add
its change a second time.

#### Scenario: A twin whose merge adds its lines again
- **WHEN** a clean update meets a twin whose merge would add its lines again
- **THEN** the twin is dropped by its proof
  [@test] crates/bd-update/src/update_test.rs (`a_twin_whose_merge_adds_its_lines_again_is_dropped_by_its_proof`)

#### Scenario: The same patch under another key
- **WHEN** a clean update meets the same patch of its author under another key
- **THEN** it is dropped by its patch id
  [@test] crates/bd-update/src/update_test.rs (`the_same_patch_under_another_key_whose_merge_adds_its_lines_again_is_dropped_by_its_patch_id`)

### Requirement: Rounds: A candidate by its author, author time and subject is never skipped

A commit that the baseline likely has by its author, author time and subject only (a candidate) MUST never be skipped:
that key is no proof.

#### Scenario: A twin that differs
- **WHEN** the baseline has a commit of the same author, author time and subject that differs
- **THEN** the local commit opens its round and is not in `dropped`
  [@test] crates/bd-update/src/resolve_test.rs (`a_twin_that_differs_asks_the_user_and_drops_after_the_yes`)

### Requirement: Rounds: The round of a candidate prints `landed_as` with the interdiff

The round of a candidate, and the round of each of its fixups, MUST print `session.commit.landed_as`: the baseline
commit, its subject, and the interdiff of the two from their changed lines without context, the candidate and its
fixups summed: `local_only` (what a drop loses) and `upstream_only`, per path at most 20 lines, at most 20 paths.

#### Scenario: The round of a twin
- **WHEN** the round of a candidate opens
- **THEN** `session.commit.landed_as` has the baseline commit, its subject, `local_only` and `upstream_only`
  [@test] crates/bd-update/src/resolve_test.rs (`a_twin_that_differs_asks_the_user_and_drops_after_the_yes`, `a_twin_with_every_line_of_the_commit_drops_without_a_question`)

#### Scenario: The interdiff of a candidate with fixups
- **WHEN** the candidate has fixups
- **THEN** the interdiff sums the candidate and its fixups, and names a binary file
  [@test] crates/git-ops/src/landed_pairing_test.rs (`the_interdiff_of_a_twin_sums_the_fixups_and_names_a_binary_file`)

### Requirement: Rounds: The round of a candidate points at `--landed`

In the round of a candidate, `session.next` MUST be `<bd> update --continue --round <n> --landed <commit>`, and each
file `hint` MUST point at `session.commit.hint`.

#### Scenario: `next` and the hint of a twin round
- **WHEN** the round of a candidate opens
- **THEN** `session.next` ends with `--round <n> --landed <commit>`
- **AND** each file `hint` points at `session.commit.hint`
  [@test] crates/bd-update/src/resolve_test.rs (`a_twin_that_differs_asks_the_user_and_drops_after_the_yes`)

### Requirement: Rounds: The first output lists the candidates whose drop loses lines

The first output of a session MUST list in `session.landed_differs` (`commit`, `subject`, `landed_as`) every candidate
not replayed yet whose `local_only` is not empty.

#### Scenario: A candidate that differs
- **WHEN** a session starts with a candidate whose `local_only` is not empty
- **THEN** `session.landed_differs` lists it
  [@test] crates/bd-update/src/resolve_test.rs (`a_twin_that_differs_asks_the_user_and_drops_after_the_yes`)

#### Scenario: A candidate a drop loses nothing of
- **WHEN** a session starts with a candidate whose `local_only` is empty
- **THEN** `session.landed_differs` is absent
  [@test] crates/bd-update/src/resolve_test.rs (`a_twin_with_every_line_of_the_commit_drops_without_a_question`)

### Requirement: Rounds: `--landed` takes `onto` and drops the commit as landed

`--landed <commit>` (the round's commit, at least 4 characters of its id) MUST take `onto` for every conflicting file
and drop the commit as landed: `proof: landed` with its twin as `upstream`, in `landed` as that twin, the resolution
with `landed_as` (and `local_only` for a drop that lost lines).

#### Scenario: A drop after the user's yes
- **WHEN** `--landed <commit> --user` drops a twin that differs
- **THEN** `dropped` has `proof: landed` with its twin as `upstream`, `landed` names that twin
- **AND** the resolution has `landed_as` and `local_only`
  [@test] crates/bd-update/src/resolve_test.rs (`a_twin_that_differs_asks_the_user_and_drops_after_the_yes`)

#### Scenario: A drop that loses nothing
- **WHEN** `--landed <commit>` drops a twin with every line of the commit
- **THEN** `dropped` has `proof: landed` with its twin as `upstream`, in `landed` as that twin
  [@test] crates/bd-update/src/resolve_test.rs (`a_twin_with_every_line_of_the_commit_drops_without_a_question`)

### Requirement: Rounds: `--landed` refuses what it cannot drop

`--landed` MUST refuse, changing nothing, a round without a twin (`not_landed_round`, exit 2), a commit with
`local_only` without `--user`, the user's yes (`landed_differs`, exit 2, `ask_user`), and a commit that still changes
files with `onto` for every conflicting file (`landed_not_empty`, exit 2, with `paths`).

#### Scenario: A round without a twin
- **WHEN** `--landed` names the commit of a round without a twin
- **THEN** it is `not_landed_round` (exit 2), and nothing changes
  [@test] crates/bd-update/src/resolve_test.rs (`landed_refuses_a_round_without_a_twin_and_a_commit_that_keeps_a_change`)

#### Scenario: A drop that would lose lines, without the user's yes
- **WHEN** `--landed` drops a commit with `local_only` without `--user`
- **THEN** it is `landed_differs` (exit 2, `ask_user`), and nothing changes
  [@test] crates/bd-update/src/resolve_test.rs (`a_twin_that_differs_asks_the_user_and_drops_after_the_yes`, `landed_refuses_a_round_without_a_twin_and_a_commit_that_keeps_a_change`)

#### Scenario: A commit that keeps a change after `onto`
- **WHEN** `onto` for every conflicting file leaves the commit with a change
- **THEN** it is `landed_not_empty` (exit 2, with `paths`), even with `--user`
  [@test] crates/bd-update/src/resolve_test.rs (`landed_refuses_a_round_without_a_twin_and_a_commit_that_keeps_a_change`)

### Requirement: Rounds: `--landed` without `local_only` needs no question

Without `local_only`, `--landed` SHALL need no `--user` and no question.

#### Scenario: A twin with every line of the commit
- **WHEN** the round's `next` runs for a twin without `local_only`
- **THEN** the commit is dropped as landed, without `--user` and without a question
  [@test] crates/bd-update/src/resolve_test.rs (`a_twin_with_every_line_of_the_commit_drops_without_a_question`)

### Requirement: Rounds: Text, modify/delete, rename/delete and rename/rename conflicts open a round

Text conflicts (`CONFLICT (contents)`, an add on both sides as a `content` conflict without `before`), modify/delete,
rename/delete (kind `rename_delete`, with `renamed_from`; `--take` and `--delete` take either name), and rename/rename
(kind `rename_rename`) of one base path to two different names MUST open a round.

#### Scenario: The supported conflicts
- **WHEN** an update meets a text or a modify/delete conflict
- **THEN** a round opens for it
  [@test] crates/git-ops/src/update_resolve_test.rs (`only_text_and_modify_delete_conflicts_are_supported`)

#### Scenario: A rename and a delete
- **WHEN** one side renames a file and the other deletes it
- **THEN** a round opens with kind `rename_delete` and `renamed_from`, and `--take` takes either name
  [@test] crates/bd-update/src/resolve_test.rs (`a_rename_and_a_delete_resolve_with_a_take`)

#### Scenario: The output of `merge-tree`
- **WHEN** the merge of a commit conflicts, or merges cleanly
- **THEN** its tree, its stages and its messages are parsed; a clean merge has only the tree
  [@test] crates/git-ops/src/merge_tree_output.rs (`parses_the_tree_the_stages_and_the_messages`, `a_clean_merge_has_only_the_tree`)

### Requirement: Rounds: A rename/rename file names its three paths

A rename/rename file MUST have `path` as the onto name, `commit_path` as the commit name and `renamed_from` as the base
name; its before/onto/commit copies MUST be the original blobs from those three paths.

#### Scenario: A rename of one base path to two names
- **WHEN** both sides rename one base path to two different names
- **THEN** the round has one file with `path`, `commit_path` and `renamed_from`, its copies from those three paths
  [@test] crates/git-ops/src/update_resolve_test.rs (`a_rename_rename_conflict_is_one_file_with_all_three_names`)

#### Scenario: The schema and the state of a rename/rename round
- **WHEN** a rename/rename round is printed and stored
- **THEN** its output matches the schema, and its state reads back as written in state version three
  [@test] crates/bd-update/src/resolve_test.rs (`a_rename_rename_session_matches_the_output_schema`)
  [@test] crates/git-ops/src/update_resolve_test.rs (`a_rename_rename_round_round_trips_in_state_version_three`)

### Requirement: Rounds: Takes and the delete of a rename/rename file

`--take onto:<name>` MUST keep the onto name and delete the commit name, `--take commit:<name>` MUST do the reverse,
and `--delete <name>` MUST delete both; each accepts any of the three names. An edited copy alone MUST be
`missing_resolution` until it has a chosen name.

#### Scenario: A take of either side and a delete
- **WHEN** a rename/rename file is resolved with `--take onto:<name>`, `--take commit:<name>` or `--delete <name>`
- **THEN** the take keeps its side's name and deletes the other, the delete deletes both
- **AND** an edited copy alone is `missing_resolution`
  [@test] crates/bd-update/src/resolve_test.rs (`a_rename_rename_resolves_with_takes_and_delete`)

### Requirement: Rounds: `--rename-to` writes the checked edit copy to a new path

`--rename-to <path> <new-path>` MUST accept any of the three names and write the checked `edit` copy to `new-path`,
deleting both renamed paths unless either is the new path; a clean, unchanged edit is valid because the name is the
decision.

#### Scenario: A third name
- **WHEN** `--rename-to <path> <new-path>` names a third path
- **THEN** the checked `edit` copy is written to `new-path`, and both renamed paths are deleted
  [@test] crates/bd-update/src/resolve_test.rs (`a_rename_rename_writes_the_checked_merge_to_a_third_name`, `three_rename_rename_files_can_be_written_to_three_new_names`)

#### Scenario: One of the new names
- **WHEN** `--rename-to` names either new side's name
- **THEN** the copy is written there, and the other renamed path is deleted
  [@test] crates/bd-update/src/resolve_test.rs (`rename_to_accepts_either_new_side_name`)

### Requirement: Rounds: The destination of `--rename-to` is checked

A destination MUST be a valid tree path, free in the result tree other than the pair's two new names, and not overlap
another conflicting file; otherwise the flag is `invalid_arguments`. It MUST NOT resolve a non-rename/rename file or be
combined with a take or delete for the same file.

#### Scenario: Invalid names, collisions, other files and multiple resolutions
- **WHEN** `--rename-to` names an invalid or occupied path, a file of another kind, or a file with another resolution
- **THEN** it is `invalid_arguments`
  [@test] crates/bd-update/src/resolve_test.rs (`rename_to_rejects_non_rename_files_invalid_names_collisions_and_multiple_resolutions`)

### Requirement: Rounds: An edited rename/rename copy goes through the content checks

An edited `rename_rename` copy MUST go through the content checks (markers, outside-conflict, encoding and line ends);
dropped-line reports still compare the result with all three copies.

#### Scenario: Markers left in a copy written to a new name
- **WHEN** an edited `rename_rename` copy with markers left goes to a new name
- **THEN** the markers are refused before the new name is written
  [@test] crates/bd-update/src/resolve_test.rs (`an_overlapping_rename_rename_edit_checks_markers_before_writing_the_new_name`)

### Requirement: Rounds: The written path of a rename/rename file is named

When the written path differs from `path`, `resolutions[].files[].renamed_to` and the resolve note MUST name it, and
`verify.paths` MUST use the written path.

#### Scenario: A copy written to a new name
- **WHEN** a rename/rename file is written to another path than `path`
- **THEN** `resolutions[].files[].renamed_to` and the resolve note name it, and `verify.paths` has the written path
  [@test] crates/bd-update/src/resolve_test.rs (`a_rename_rename_writes_the_checked_merge_to_a_third_name`, `three_rename_rename_files_can_be_written_to_three_new_names`)

### Requirement: Rounds: A mode changed on one side merges as git does

A mode that one side changed MUST be merged as git does; two modes that differ on both sides are `distinct modes`.

#### Scenario: A mode changed by one side
- **WHEN** one side changed the mode of a conflicting file
- **THEN** the merge takes it as git does

#### Scenario: Modes changed on both sides
- **WHEN** both sides changed the mode to different ones
- **THEN** the file is `distinct modes`, an unsupported conflict

### Requirement: Rounds: Any other conflict refuses the whole round with `unsupported_conflict`

Any other type (binary, file/directory, other rename shapes, distinct modes, a symlink or a submodule, an invalid
path), and a `content` file whose merge result has no block of brd's labels, MUST refuse the whole round with
`unsupported_conflict` (exit 4) and its `paths`; at the start no session is created.

#### Scenario: A binary or a file/directory conflict
- **WHEN** an update meets a binary or a file/directory conflict
- **THEN** it is `unsupported_conflict` (exit 4) with its `paths`, and no session is created
  [@test] crates/bd-update/src/resolve_test.rs (`binary_and_file_directory_conflicts_are_refused_and_start_no_session`)

#### Scenario: Any type but text and modify/delete
- **WHEN** a conflict of another type is met
- **THEN** the round is refused
  [@test] crates/git-ops/src/update_resolve_test.rs (`only_text_and_modify_delete_conflicts_are_supported`)

#### Scenario: A rename/rename path in an unsupported message
- **WHEN** a message of git names a rename/rename path as unsupported
- **THEN** the whole round is refused
  [@test] crates/git-ops/src/update_resolve_test.rs (`a_rename_rename_path_in_an_unsupported_message_refuses_the_round`)

### Requirement: Rounds: The same rename on both sides opens no round

The same rename on both sides MUST merge cleanly and open no round.

#### Scenario: Both sides rename a file to the same name
- **WHEN** both sides rename a file to the same name
- **THEN** it merges cleanly and opens no round

### Requirement: Rounds: A round whose words resolve it resolves itself

With `branchdeck.resolveByWords` on, a conflicting commit whose conflict of git its words resolve MUST be a round that
resolves itself: it counts in `round`, and the agent gets the next round that does not resolve.

#### Scenario: The first round resolves by words
- **WHEN** the words resolve the conflict of the first conflicting commit
- **THEN** that round counts, and the agent gets the next round that does not resolve
  [@test] crates/git-ops/src/update_resolve_test.rs (`a_round_that_its_words_resolve_counts_and_the_agent_gets_the_next_one`)

#### Scenario: A later round resolves by words
- **WHEN** the words resolve the conflict of a later round
- **THEN** it resolves in the `--continue` before it, and the finish notes it
  [@test] crates/git-ops/src/update_resolve_test.rs (`a_later_round_that_its_words_resolve_resolves_in_the_continue_and_the_finish_notes_it`)

### Requirement: Rounds: Every round and `--status` list the rounds that resolved themselves

Every round and `--status` MUST list the rounds that resolved themselves in `session.resolved` (`round`, `commit`,
`new_id`, `conflicts`, `files`), so a round number never skips without a reason. Their resolution has `words` (the
conflicts), and each file `how: "words"` with its `conflicts`.

#### Scenario: The next round, the status and the finish
- **WHEN** a round resolved by words, and the next round, `--status` and the finish print
- **THEN** each names it in `session.resolved` (the finish in `resolved`)
- **AND** its resolution has `words`, and each file `how: "words"` with its `conflicts`
  [@test] crates/bd-update/src/resolve_test.rs (`a_round_that_resolves_by_words_is_named_in_the_next_round_the_status_and_the_finish`)

### Requirement: Rounds: A commit the words leave without a change is dropped

A commit that the words leave without a change MUST be dropped, as `git replay` drops one.

#### Scenario: The words leave the commit empty
- **WHEN** the words resolve a commit to no change
- **THEN** the commit is dropped

### Requirement: Rounds: A second conflicting commit opens a new round with its own directory

A second conflicting commit MUST open a new round with its own directory; the copies of the closed round MUST be
removed.

#### Scenario: The second round
- **WHEN** a `--continue` replays onto a second conflicting commit
- **THEN** round 2 opens with its own copies, and the copies of round 1 are gone
  [@test] crates/bd-update/src/resolve_test.rs (`a_second_conflict_opens_a_second_round_with_its_own_copies`)

### Requirement: Rounds: `--round` of another round is `round_moved`

`--round <n>` of another round MUST be `round_moved` (exit 3, retryable: read the open round and retry): its message
says that `brd update` prints the open round.

#### Scenario: A stale round number
- **WHEN** `--continue --round 1` runs while round 2 is open
- **THEN** it is `round_moved` (exit 3, retryable), the message saying that `brd update` prints the open round
  [@test] crates/bd-update/src/resolve_test.rs (`a_second_conflict_opens_a_second_round_with_its_own_copies`)

### Requirement: Copies: The copies are in `branch-deck/resolve/r<n>/` under flat names

The copies MUST be in `branch-deck/resolve/r<n>/` of the worktree's git dir: `edit/`, `before/`, `onto/` and
`commit/`, each file under the flat name `<key>-<basename>`, and `commit.patch`. Two paths that differ only in case
MUST get distinct names.

#### Scenario: Flat names that never collide
- **WHEN** the copies of a round are written
- **THEN** each file has the flat name `<key>-<basename>`, and two paths that differ only in case get distinct names
  [@test] crates/git-ops/src/update_resolve_test.rs (`copies_have_flat_names_that_never_collide`)

### Requirement: Copies: The tree paths are validated

The tree paths MUST be validated: no empty, `.` or `..` component, no `.git` in any case or a name the file system
takes for it (`GIT~1`, `.git.`, HFS+ ignorable code points, as `core.protectNTFS` and `core.protectHFS`), no NUL; on
Windows also no name Windows can't hold, which a tree made on Linux or macOS may have.

#### Scenario: A path that could leave a directory
- **WHEN** a tree path has an empty, `.` or `..` component, a `.git` in any case or a name taken for it, or a NUL
- **THEN** it is invalid
  [@test] crates/git-ops/src/update_resolve_test.rs (`tree_paths_that_could_leave_a_directory_are_invalid`)

#### Scenario: A name Windows can't hold
- **WHEN** on Windows a tree path has `\`, a drive prefix, a device name such as `CON`, `<>:"|?*` or a control
  character, a trailing dot or space
- **THEN** it is invalid

### Requirement: Copies: A long basename is cut and keeps its extension

A basename longer than 100 bytes MUST be cut on a character boundary, and keep its extension.

#### Scenario: A basename of more than 100 bytes
- **WHEN** the basename of a conflicting file is longer than 100 bytes
- **THEN** its copy's name is cut on a character boundary and keeps its extension
  [@test] crates/git-ops/src/update_resolve_test.rs (`a_long_basename_is_cut_and_keeps_its_extension`)

### Requirement: Copies: brd writes and reads the bytes as git stores them, never through a symlink

brd MUST write and read the bytes as git stores them, without filters, through a new temporary file and a rename, and
never through a symlink: a symlink at `edit` is no resolution, and a rewrite replaces it. Every directory from the git
dir (or `--dir`) down to the copy MUST be a real directory: brd neither reads nor deletes through a symlinked directory.

#### Scenario: A symlink at `edit`
- **WHEN** the agent puts a symlink at `edit`
- **THEN** it is no resolution, and a rewrite replaces it without following it
  [@test] crates/bd-update/src/resolve_test.rs (`a_symlink_at_edit_is_never_followed`)

#### Scenario: A symlinked round directory
- **WHEN** a directory of the copies is a symlink
- **THEN** brd neither reads nor deletes through it
  [@test] crates/bd-update/src/resolve_test.rs (`the_dir_of_another_worktree_or_session_is_refused_and_a_symlinked_round_is_never_read`)

### Requirement: Copies: The marker file names the git dir that owns the directory

The marker file `.branch-deck-resolve` MUST name the git dir of the worktree that owns the directory. brd MUST delete
only in a directory whose marker names its own git dir.

#### Scenario: The directory of another worktree
- **WHEN** the marker of a directory names the git dir of another worktree
- **THEN** brd deletes nothing in it
  [@test] crates/bd-update/src/resolve_test.rs (`the_dir_of_another_worktree_or_session_is_refused_and_a_symlinked_round_is_never_read`)

### Requirement: Copies: `--dir` puts the copies outside every worktree

`--dir <path>` MUST put the copies of a new session into an absolute directory that is not a symlink, not inside a
worktree, empty or marked as brd's for this git dir, and used by no other session; a refusal is `invalid_arguments`,
and a refused directory that brd created is removed. brd MUST delete only what it wrote, and remove only a directory
it created.

#### Scenario: A directory outside every worktree
- **WHEN** `--dir <path>` names an absolute directory outside every worktree
- **THEN** the copies go there, and a directory inside a worktree is `invalid_arguments`
  [@test] crates/bd-update/src/resolve_test.rs (`the_copies_go_to_dir_only_outside_every_worktree`)

#### Scenario: The directory of another worktree or session
- **WHEN** `--dir` names a directory marked for another git dir or used by another session
- **THEN** it is `invalid_arguments`
  [@test] crates/bd-update/src/resolve_test.rs (`the_dir_of_another_worktree_or_session_is_refused_and_a_symlinked_round_is_never_read`)

### Requirement: Copies: `--status --materialize` writes the missing copies

`brd update --status --materialize` MUST write the missing copies of the open round, and with `--overwrite-edited`
(`--force`, its name before) all of them.

#### Scenario: A missing copy and an edited one
- **WHEN** `brd update --status --materialize` runs with one copy missing and one edited
- **THEN** it writes the missing copy and keeps the edited one
- **AND** with `--overwrite-edited` it writes all of them
  [@test] crates/bd-update/src/resolve_test.rs (`markers_an_edit_outside_a_block_and_a_missing_file_are_refused`)

### Requirement: `--continue`: `--continue` commits the round and opens the next or applies

`brd update --continue` MUST check the resolution of every conflicting file of the open round, commit the round,
replay the rest, and open the next round (exit 1) or apply the update (exit 0).

#### Scenario: The next round opens
- **WHEN** `--continue` commits a round and the rest conflicts again
- **THEN** the next round opens (exit 1)
  [@test] crates/bd-update/src/resolve_test.rs (`a_second_conflict_opens_a_second_round_with_its_own_copies`)

#### Scenario: The last round applies the update
- **WHEN** `--continue` commits the last round
- **THEN** the update applies (exit 0)
  [@test] crates/bd-update/src/resolve_test.rs (`a_second_conflict_opens_a_second_round_with_its_own_copies`)

### Requirement: `--continue`: A refusal changes nothing and names the round and its commit

A refusal of `--continue` MUST change nothing, name the round and its commit (id and subject, also in
`error.commits`), and say that the round stays open and the edit copies keep the agent's edits; `--status` then MUST
show `edited` per file, whether its copy differs from brd's merge result.

#### Scenario: A refused resolution
- **WHEN** `--continue` refuses a resolution
- **THEN** nothing changes, and the message names the round and its commit, also in `error.commits`
- **AND** it says that the round stays open and the edit copies keep the agent's edits
  [@test] crates/bd-update/src/resolve_test.rs (`a_refusal_names_the_commit_and_keeps_the_round`, `markers_an_edit_outside_a_block_and_a_missing_file_are_refused`)

#### Scenario: The status after a refusal
- **WHEN** `--status` runs after the refusal
- **THEN** each file has `edited`, whether its copy differs from brd's merge result
  [@test] crates/bd-update/src/resolve_test.rs (`a_refusal_names_the_commit_and_keeps_the_round`)

### Requirement: `--continue`: A resolution is an edit, a take, a delete or a rename

A resolution MUST be an edited `edit` copy, `--take onto:<path>`, `--take commit:<path>`, `--delete <path>` or
`--rename-to <path> <new-path>`; a missing file is never a delete. No resolution MUST be `missing_resolution`; an
unchanged modify/delete copy is no resolution either. A take of a side without the file deletes it.

#### Scenario: A modify/delete resolved with a take or a delete
- **GIVEN** a `modify_delete` round
- **WHEN** it is resolved with `--take` or `--delete`, or its copy is left unchanged
- **THEN** a take of a side without the file deletes it, the delete deletes it
- **AND** an unchanged copy is `missing_resolution`
  [@test] crates/bd-update/src/resolve_test.rs (`modify_delete_is_resolved_with_take_and_delete`)

### Requirement: `--continue`: The resolutions of a rename/rename file

For `rename_rename`, `--take` and `--delete` MUST accept the base, onto or commit name and the delete removes both
renamed paths; an edited copy without a take, delete or `--rename-to` MUST be `missing_resolution` because it has no
chosen path.

#### Scenario: Takes and the delete by any of the three names
- **WHEN** a `rename_rename` file is resolved with `--take` or `--delete` by any of its names, or only edited
- **THEN** the take or the delete resolves it, and an edited copy alone is `missing_resolution`
  [@test] crates/bd-update/src/resolve_test.rs (`a_rename_rename_resolves_with_takes_and_delete`)

### Requirement: `--continue`: `--rename-to` checks the content rules of an edited file

`--rename-to` MUST check the same content rules as an edited `content` file, and SHALL allow the unchanged clean merge
under a new name.

#### Scenario: The unchanged merge under a new name
- **WHEN** `--rename-to` writes the unchanged clean merge under a new name
- **THEN** the resolution is valid
  [@test] crates/bd-update/src/resolve_test.rs (`a_rename_rename_writes_the_checked_merge_to_a_third_name`)

#### Scenario: Markers left in the renamed copy
- **WHEN** `--rename-to` names a copy with markers left
- **THEN** it is refused as an edited `content` file would be
  [@test] crates/bd-update/src/resolve_test.rs (`an_overlapping_rename_rename_edit_checks_markers_before_writing_the_new_name`)

### Requirement: `--continue`: A resolution of a wrong kind, path or side is `invalid_arguments`

A `--rename-to` on any other kind, to an invalid or occupied path, or combined with another resolution of the same file
MUST be `invalid_arguments`. A `--take` or `--delete` of a path the round does not have, or a side other than `onto`
and `commit`, MUST be `invalid_arguments`.

#### Scenario: A wrong `--rename-to`
- **WHEN** `--rename-to` names a file of another kind, an invalid or occupied path, or a file with another resolution
- **THEN** it is `invalid_arguments`
  [@test] crates/bd-update/src/resolve_test.rs (`rename_to_rejects_non_rename_files_invalid_names_collisions_and_multiple_resolutions`)

#### Scenario: A take of another path or side
- **WHEN** `--take` or `--delete` names a path the round does not have, or a side other than `onto` and `commit`
- **THEN** it is `invalid_arguments`

### Requirement: `--continue`: A block is recognised only by brd's labels

A block MUST be recognised only by the labels brd passed to the merge (`<<<<<<< <onto>`, `||||||| <commit>^`,
`>>>>>>> <commit>`), at any marker length (`conflict-marker-size`), with the separator of the same length only between
the base section and the end.

#### Scenario: Markers of brd's labels
- **WHEN** an `edit` copy has blocks of brd's labels
- **THEN** they are found by their labels
  [@test] crates/bd-update/src/resolve_test.rs (`markers_are_found_by_their_labels`)

#### Scenario: Longer markers
- **WHEN** `conflict-marker-size` makes the markers longer
- **THEN** the blocks are found by their labels, with the separator of the same length
  [@test] crates/git-ops/src/resolve_check_test.rs (`longer_markers_are_found_by_their_labels`)

### Requirement: `--continue`: The markers of a renamed file carry its path

With a rename, merge-ort adds `:<path>` after a label, and the block MUST count too: a renamed file with a conflict
resolves as any other.

#### Scenario: A renamed file with a conflict
- **WHEN** a renamed file conflicts and its markers carry `:<path>`
- **THEN** its blocks count, and an edit resolves it
  [@test] crates/bd-update/src/resolve_test.rs (`a_renamed_file_with_a_conflict_resolves_with_an_edit`)
  [@test] crates/git-ops/src/resolve_check_test.rs (`markers_of_a_renamed_file_carry_the_path`)

### Requirement: `--continue`: Any other marker-like line is text

Any other marker-like line, e.g. the `=======` of a Markdown setext heading, MUST be text.

#### Scenario: A setext heading
- **WHEN** an `edit` copy has the `=======` of a Markdown setext heading
- **THEN** it is text, no marker
  [@test] crates/git-ops/src/resolve_check_test.rs (`a_setext_heading_is_text`)

### Requirement: `--continue`: An edited content file changes only the blocks

An edited content file MUST equal brd's `edit` copy outside the conflict blocks (`resolution_outside_conflict`), and
have no marker line of brd's labels where a block was (`unresolved_markers`). When only the line ends differ (CRLF and
LF, or the newline at the end), the message MUST say so. These refusals, and the one of a byte order mark, are exit 2
with `paths`.

#### Scenario: A resolution inside the block
- **WHEN** the agent changes only the conflict blocks
- **THEN** the resolution passes
  [@test] crates/git-ops/src/resolve_check_test.rs (`a_resolution_inside_the_block_passes`)

#### Scenario: An edit outside the block
- **WHEN** the agent changes a line outside the blocks, or the order of the plain runs between two blocks
- **THEN** it is `resolution_outside_conflict` (exit 2, with `paths`)
  [@test] crates/bd-update/src/resolve_test.rs (`markers_an_edit_outside_a_block_and_a_missing_file_are_refused`)
  [@test] crates/git-ops/src/resolve_check_test.rs (`an_edit_outside_the_block_is_refused`, `the_plain_runs_between_two_blocks_must_stay_in_order`)

#### Scenario: Markers left
- **WHEN** a marker line of brd's labels is left where a block was
- **THEN** it is `unresolved_markers` (exit 2, with `paths`)
  [@test] crates/git-ops/src/resolve_check_test.rs (`markers_left_are_refused`)

#### Scenario: Only the line ends differ
- **WHEN** the copy differs only in its line ends (CRLF and LF, or the newline at the end)
- **THEN** the message says so
  [@test] crates/git-ops/src/resolve_check_test.rs (`changed_line_ends_are_named`)

### Requirement: `--continue`: A copy with a byte order mark or in UTF-16 is refused

A copy that starts with a byte order mark or is UTF-16, which `Set-Content` and `Out-File` of Windows PowerShell 5.1
write, MUST be `resolution_outside_conflict` with its own message: write the copy as UTF-8 without a BOM, with
`[IO.File]::WriteAllText`.

#### Scenario: A copy that Windows PowerShell 5.1 wrote
- **WHEN** the `edit` copy starts with a byte order mark or is UTF-16
- **THEN** it is refused with the message to write it as UTF-8 without a BOM, with `[IO.File]::WriteAllText`
  [@test] crates/bd-update/src/resolve_test.rs (`an_edit_copy_with_a_byte_order_mark_or_in_utf16_is_refused`)

### Requirement: `--continue`: A refusal names the copy and the paths changed in the working tree

The message of a refusal MUST name the absolute `edit` copy of each path. For leftover markers and a missing
resolution, it MUST also name the paths that changed in the working tree, where brd never reads.

#### Scenario: An edit in the working tree
- **WHEN** the agent edits the file in the working tree instead of its copy, and `--continue` runs
- **THEN** the refusal names the absolute `edit` copy and the path that changed in the working tree
  [@test] crates/bd-update/src/resolve_test.rs (`an_edit_in_the_working_tree_is_pointed_to_the_copy`)

### Requirement: `--continue`: A change outside the blocks goes into a `fixup!` commit

The refusal of a change outside the blocks MUST say that such a change goes into a `fixup! <subject>` commit after the
update, then `brd fold`.

#### Scenario: An edit outside a block
- **WHEN** `--continue` refuses a change outside the blocks
- **THEN** the message names the `fixup! <subject>` commit after the update and `brd fold`
  [@test] crates/bd-update/src/resolve_test.rs (`an_edit_in_the_working_tree_is_pointed_to_the_copy`)

### Requirement: `--continue`: The lines of a side that the result lost are reported

The lines of a side's change (`before` to `onto`, `before` to `commit`) that the result lost MUST be reported per file
as `dropped_upstream_lines` and `dropped_commit_lines`, also for `--take` and `--delete`, never refused.

#### Scenario: The lines a result lost
- **WHEN** a resolution loses lines of a side's change
- **THEN** they are reported per file as `dropped_upstream_lines` and `dropped_commit_lines`, and nothing is refused
  [@test] crates/git-ops/src/resolve_check_test.rs (`dropped_lines_are_the_lines_of_a_side_that_the_result_lost`)

#### Scenario: A take
- **WHEN** a `--take` resolves a file
- **THEN** the lines of the other side it drops are reported
  [@test] crates/bd-update/src/resolve_test.rs (`a_take_reports_the_lines_of_the_other_side_it_drops`)

### Requirement: `--continue`: A line counts as kept by its words

A line MUST count as kept when the result has it as it is, or when one line that replaced a conflict block (of an edit;
the whole result of a take) has every word that the side added to its nearest `before` line (the largest share of
words in common). So a lost change of one value (a mode, a version, a flag) is reported, also when its word is
elsewhere in the file, and a rewrite that keeps the change is not.

#### Scenario: A lost change of one value
- **WHEN** a resolution loses the change of one value of a line
- **THEN** the line is reported as dropped
  [@test] crates/git-ops/src/resolve_check_test.rs (`a_lost_change_of_one_value_is_reported`)
  [@test] crates/bd-update/src/resolve_test.rs (`field_shaped_conflicts_resolve_as_the_hints_say`)

#### Scenario: The lost word is elsewhere in the file
- **WHEN** the word of the lost change is elsewhere in the file
- **THEN** the line is still reported
  [@test] crates/git-ops/src/resolve_check_test.rs (`a_lost_value_is_reported_when_its_word_is_elsewhere_in_the_file`)

### Requirement: `--continue`: The hint of a deleting modify/delete says how to keep the delete

The hint of a modify/delete whose commit deletes the file MUST say how to keep the delete (`--take commit:<path>`) and
that a change of `onto` that belongs in another file now goes into a `fixup!` commit after the update, reported as
`dropped_upstream_lines`.

#### Scenario: A commit deletes a file the upstream changed
- **WHEN** the commit deletes a file the upstream changed
- **THEN** the hint names `--take commit:<path>` and the `fixup!` commit after the update
- **AND** the take reports the upstream change as `dropped_upstream_lines`
  [@test] crates/bd-update/src/resolve_test.rs (`field_shaped_conflicts_resolve_as_the_hints_say`)

### Requirement: `--continue`: The same line added on both sides opens no round

The same line added on both sides MUST merge cleanly and open no round.

#### Scenario: Both sides add the same line
- **WHEN** both sides add the same line
- **THEN** it merges cleanly and no round opens
  [@test] crates/bd-update/src/resolve_test.rs (`field_shaped_conflicts_resolve_as_the_hints_say`)

### Requirement: `--continue`: A resolution that leaves the commit empty is `empty_resolution`

A resolution that leaves the commit without changes MUST be `empty_resolution` (exit 2, `ask_user`), nothing written;
for a round with a twin, its message MUST name `--landed`.

#### Scenario: An empty resolution
- **WHEN** a resolution leaves the commit without changes
- **THEN** it is `empty_resolution` (exit 2, `ask_user`), and nothing is written
  [@test] crates/bd-update/src/resolve_test.rs (`a_resolution_without_changes_is_refused_unless_the_agent_keeps_or_drops_it`)

#### Scenario: An empty resolution of a twin round
- **WHEN** the round has a twin
- **THEN** the message of `empty_resolution` names `--landed`

### Requirement: `--continue`: `--allow-empty` keeps an empty commit

`--allow-empty` MUST keep the commit as an empty commit (`empty: "kept"`).

#### Scenario: An empty commit kept
- **WHEN** `--continue --allow-empty` runs on an empty resolution
- **THEN** the commit is kept, `empty: "kept"`
  [@test] crates/bd-update/src/resolve_test.rs (`a_resolution_without_changes_is_refused_unless_the_agent_keeps_or_drops_it`)

### Requirement: `--continue`: `--drop-empty` drops an empty commit

`--drop-empty <commit>` (the round's commit, at least 4 characters of its id) MUST drop it (`new_id` null, no note,
`proof: user` in `dropped`), in `landed` only when the baseline has a commit of its author, author time and subject (a
candidate).

#### Scenario: An empty commit dropped
- **WHEN** `--continue --drop-empty <commit>` runs on an empty resolution
- **THEN** the commit is dropped: `new_id` null, no note, `proof: user` in `dropped`
  [@test] crates/bd-update/src/resolve_test.rs (`a_resolution_without_changes_is_refused_unless_the_agent_keeps_or_drops_it`)

#### Scenario: A dropped candidate
- **WHEN** the baseline has a commit of the author, author time and subject of the dropped commit
- **THEN** the commit is in `landed`

### Requirement: `--continue`: `--continue --dry-run` checks and moves no ref

`--continue --dry-run` MUST run the checks, report the resolution with its `new_id`, exit 0, and move no ref.

#### Scenario: A dry run of `--continue`
- **WHEN** `--continue --dry-run` runs on a resolution
- **THEN** it reports the resolution with its `new_id`, exits 0 and moves no ref
  [@test] crates/bd-update/src/resolve_test.rs (`a_dry_run_of_continue_checks_and_gives_the_ids_of_the_real_one`)

### Requirement: `--continue`: `--continue --dry-run` takes no repository lock

`--continue --dry-run` MUST take no repository lock, so a check in a loop never makes an update or a rewrite wait; only
the real `--continue` is `locked` while another process holds the lock.

#### Scenario: Another process holds the lock
- **GIVEN** another process holds the repository lock
- **WHEN** `--continue --dry-run` runs
- **THEN** it answers, while the real `--continue` is `locked`
  [@test] crates/bd-update/src/resolve_test.rs (`a_dry_run_of_continue_answers_while_another_process_holds_the_lock`)

### Requirement: `--continue`: The finish releases the lock before the landed report

A `--continue` that finishes the session MUST release the repository lock once the branch, its files and the session
refs moved and the session's copies and marker are removed: what the update landed is a report, read and written to
`landed.json` without the lock, as after an update that applies cleanly.

#### Scenario: The landed report of a finished session
- **WHEN** a `--continue` finishes the session
- **THEN** the landed report is read and written to `landed.json` after the lock is released
  [@test] crates/bd-update/src/resolve_test.rs (`the_landed_report_of_a_finished_session_runs_after_the_lock`)

### Requirement: Finish: The finish moves the branch to the replayed tip and prints the resolutions

The finish MUST move the branch from the session's HEAD to the replayed tip with the reflog message
`branch-deck: update project`, the files following through the guarded update, and print `state: "rebase"`, the
`undo_token` and `resolutions`: per round the `commit`, its `new_id` and per file `how` (`edited`, `onto`, `commit`,
`deleted`, `words`) with the dropped lines. The rounds that resolved themselves are in `resolved` too.

#### Scenario: The finish of a session
- **WHEN** the last round is committed
- **THEN** the branch moves to the replayed tip and the files follow
- **AND** the output has `state: "rebase"`, the `undo_token` and `resolutions` with `how` per file
  [@test] crates/bd-update/src/resolve_test.rs (`a_conflict_opens_a_round_and_the_edited_copy_finishes_the_update`)

### Requirement: Finish: The session ref and the copies are gone after the finish

The session ref and the copies MUST be gone after the finish.

#### Scenario: Nothing of the session is left
- **WHEN** a session finishes
- **THEN** no ref of `refs/branch-deck/` and no copy is left
  [@test] crates/bd-update/src/resolve_test.rs (`a_conflict_opens_a_round_and_the_edited_copy_finishes_the_update`)

### Requirement: Finish: Each resolved commit gets a note, written by the finish

Each resolved commit MUST get a note in `refs/notes/branch-deck/resolve` (the session, the round, the original commit,
`resolved-by-words` for a round that resolved itself, the outcome per path), written only by the finish, in its ref
transaction.

#### Scenario: The note of an edited round
- **WHEN** a session finishes
- **THEN** each resolved commit has its note in `refs/notes/branch-deck/resolve` with the outcome per path
  [@test] crates/bd-update/src/resolve_test.rs (`a_conflict_opens_a_round_and_the_edited_copy_finishes_the_update`)

### Requirement: Finish: Local changes on the paths the finish writes keep the session ready

Local changes on paths the finish writes MUST be `uncommitted_overlap` (exit 4) with the `paths`, and keep the session
`ready` with its resolutions; a later `--continue` only finishes.

#### Scenario: A peer's edit of a file the finish writes
- **GIVEN** a local change on a path the finish writes
- **WHEN** the last `--continue` runs
- **THEN** it is `uncommitted_overlap` (exit 4) with the `paths`, and the session stays `ready` with its resolutions
- **AND** once the change is gone, a later `--continue` only finishes
  [@test] crates/bd-update/src/resolve_test.rs (`local_changes_on_the_finish_keep_the_session_ready`)

### Requirement: Finish: An interrupted finish is completed by the next `--continue`

A finish interrupted before the branch moved (`applying`), or refused by a `git switch` (`head_moved`), MUST be
completed by the next `--continue`.

#### Scenario: A finish interrupted or refused
- **WHEN** the finish is interrupted before the branch moved, or a `git switch` refuses it
- **THEN** the next `--continue` completes it
  [@test] crates/bd-update/src/resolve_test.rs (`an_interrupted_finish_is_completed_by_the_next_continue`)

### Requirement: Finish: A finish whose files did not follow refuses `--abort`

A finish whose branch moved but whose files did not follow (`applied`, HEAD at the tip) MUST refuse `--abort` with
`finish_pending` (exit 4), and the next `--continue` MUST let the files follow.

#### Scenario: A finish killed before the files followed
- **GIVEN** a finish whose branch moved and whose files did not follow
- **WHEN** `--abort` runs, then `--continue`
- **THEN** `--abort` is `finish_pending` (exit 4), and `--continue` lets the files follow
  [@test] crates/bd-update/src/resolve_test.rs (`a_finish_killed_before_the_files_followed_is_recovered`)

### Requirement: Finish: A finish that died after its files followed ends by `--continue` or `--abort`

When the files did follow and the branch still contains the tip (the finish died before it deleted the session ref,
also with peers' commits on top), both `--continue` and `--abort` MUST delete the session ref and exit 0.

#### Scenario: A peer committed on top
- **GIVEN** a finish that died after the files followed, and a peer's commit on top
- **WHEN** `--continue` or `--abort` runs
- **THEN** it deletes the session ref and exits 0
  [@test] crates/bd-update/src/resolve_test.rs (`a_session_whose_finish_died_after_the_files_followed_ends_after_a_peer_commit`)

### Requirement: Finish: A moved branch without its files is `finish_pending`

When the files did not follow and HEAD left the tip, `--continue` and `--abort` MUST be `finish_pending` (exit 4), and
the message MUST name `git update-ref -d <session ref> <commit>` as the way out.

#### Scenario: HEAD left the tip
- **GIVEN** a finish whose files did not follow, and a HEAD that left the tip
- **WHEN** `--continue` or `--abort` runs
- **THEN** it is `finish_pending` (exit 4), the message naming `git update-ref -d <session ref> <commit>`
  [@test] crates/bd-update/src/resolve_test.rs (`a_moved_branch_without_its_files_is_finish_pending`)

### Requirement: `--abort` and `--status`: `--abort` deletes the session ref, then the copies

`brd update --abort` MUST delete the session ref with compare-and-swap under the lock, then the copies brd wrote after
its release, and print the `worktree`, the `branch` and the `round`; it writes no note. Without a session it MUST be
`no_session` (exit 2).

#### Scenario: The abort of a stale session
- **GIVEN** a session that is `session_stale`
- **WHEN** `brd update --abort` runs
- **THEN** the session ref and the copies are gone, no note is written, and it prints the `worktree` and the `round`
- **AND** a `--continue` after it is `no_session` (exit 2)
  [@test] crates/bd-update/src/resolve_test.rs (`a_rewritten_head_makes_the_session_stale_and_abort_removes_it`)

### Requirement: `--abort` and `--status`: `--worktree` aborts the session of another worktree

`--worktree <id>` MUST abort the session of another worktree, e.g. an orphaned one. While that worktree exists, only
`--allow-live-session` aborts it (`--force`, its name before). An id MUST be letters, digits, `.`, `_` and `-`, else
`invalid_arguments`.

#### Scenario: An orphaned session
- **GIVEN** the session of a worktree that is gone
- **WHEN** `brd update --abort --worktree <id>` runs
- **THEN** that session is aborted
  [@test] crates/bd-update/src/resolve_test.rs (`an_orphaned_session_is_listed_and_aborted_by_its_worktree`)

#### Scenario: The live session of another worktree
- **GIVEN** the session of another worktree that exists
- **WHEN** `brd update --abort --worktree <id>` runs
- **THEN** only `--allow-live-session` aborts it
  [@test] crates/bd-update/src/resolve_test.rs (`aborting_the_live_session_of_another_worktree_needs_allow_live_session`)

#### Scenario: An invalid id
- **WHEN** the id of `--worktree` has another character than letters, digits, `.`, `_` and `-`
- **THEN** it is `invalid_arguments`

### Requirement: `--abort` and `--status`: Temporary refs older than an hour are deleted

Temporary refs older than an hour (by the time in their name) belong to a process that is gone; the start of a
session, `--abort` and a `--continue` of an `applying` or `applied` session MUST delete them.

#### Scenario: A session starts beside stale temporary refs
- **GIVEN** temporary refs older than an hour by the time in their name
- **WHEN** a session starts
- **THEN** they are deleted
  [@test] crates/bd-update/src/resolve_test.rs (`stale_temporary_refs_are_swept_when_a_session_starts`)

#### Scenario: `--abort` and a `--continue` of a finish
- **GIVEN** such refs
- **WHEN** `--abort` runs, or a `--continue` of an `applying` or `applied` session
- **THEN** they are deleted

### Requirement: `--abort` and `--status`: `--status` takes no lock and changes nothing

`brd update --status` MUST take no lock and change nothing: it prints the session of this worktree as a round prints
it.

#### Scenario: A read-only command
- **WHEN** `brd update --status` runs
- **THEN** it writes no ref, config, index or working tree
  [@test] crates/bd-cli/src/agent_help_test.rs (`read_only_commands_write_no_ref_config_index_or_working_tree`)

### Requirement: `--abort` and `--status`: `--status` lists every session of the repository

`brd update --status` MUST print `sessions`, every session of the repository with its `worktree`, `branch`, `git_dir`,
`state`, `round`, `orphaned` (its worktree is gone) and `own`.

#### Scenario: The session of another worktree
- **GIVEN** a session in another worktree
- **WHEN** `brd update --status` runs
- **THEN** `sessions` lists it with `own` false, and no `session` is printed
  [@test] crates/bd-update/src/resolve_test.rs (`a_session_belongs_to_its_worktree_and_survives_a_gc_in_another`)

#### Scenario: An orphaned session
- **GIVEN** a session whose worktree is gone
- **WHEN** `brd update --status` runs
- **THEN** `sessions` lists it with `orphaned` true
  [@test] crates/bd-update/src/resolve_test.rs (`an_orphaned_session_is_listed_and_aborted_by_its_worktree`)

### Requirement: `--abort` and `--status`: `brd schema` declares the outputs of the update

`brd schema` MUST have `update`, `update_status` and `update_abort`, and the real outputs MUST use only the properties
they declare.

#### Scenario: The outputs of a session against the schema
- **WHEN** the outputs of a session are checked against `brd schema`
- **THEN** they use only the properties of `update`, `update_status` and `update_abort`
  [@test] crates/bd-update/src/resolve_test.rs (`session_outputs_match_the_schema`)

#### Scenario: Every output is listed
- **WHEN** `brd schema` runs
- **THEN** it lists every output
  [@test] crates/bd-cli/src/schema_test.rs (`schema_lists_every_output`)

### Requirement: Error codes: Exit 1 is an open round or a pending finish, not an error

`brd update` MUST exit with 1 when a round is open (`conflict`) or the finish is next (`pending`): an output, not an
error.

#### Scenario: The row of exit 1
- **WHEN** `brd update` prints an open round or a pending finish
- **THEN** it exits with 1, as its row of the table says

  | Exit | Codes |
  |---|---|
  | 1 | a round is open (`conflict`) or the finish is next (`pending`): an output, not an error |

### Requirement: Error codes: Exit 2 is a refused resolution or argument

The codes `no_session`, `unresolved_markers`, `resolution_outside_conflict`, `missing_resolution`, `empty_resolution`,
`not_landed_round`, `landed_differs`, `landed_not_empty` and `invalid_arguments` MUST exit with 2.

#### Scenario: The row of exit 2
- **WHEN** `brd update` fails with a code of this row
- **THEN** it exits with 2

  | Exit | Codes |
  |---|---|
  | 2 | `no_session`, `unresolved_markers`, `resolution_outside_conflict`, `missing_resolution`, `empty_resolution`, `not_landed_round`, `landed_differs`, `landed_not_empty`, `invalid_arguments` |

### Requirement: Error codes: Exit 3 is a retryable move or lock

The codes `head_moved`, `round_moved` and `locked` MUST exit with 3, and they are retryable.

#### Scenario: The row of exit 3
- **WHEN** `brd update` fails with a code of this row
- **THEN** it exits with 3, retryable

  | Exit | Codes |
  |---|---|
  | 3 | `head_moved`, `round_moved`, `locked` (retryable) |

### Requirement: Error codes: Exit 4 is a state the agent cannot retry out of

The codes `session_stale`, `unsupported_conflict`, `uncommitted_overlap`, `finish_pending` and `lease_follows_home`
MUST exit with 4.

#### Scenario: The row of exit 4
- **WHEN** `brd update` fails with a code of this row
- **THEN** it exits with 4

  | Exit | Codes |
  |---|---|
  | 4 | `session_stale`, `unsupported_conflict`, `uncommitted_overlap`, `finish_pending`, `lease_follows_home` |

### Requirement: Error codes: The refusals list their files in `paths`

The refusals of a resolution and `unsupported_conflict` MUST list the files in `paths`, with each unsupported path's
conflict type in the message.

#### Scenario: A refused resolution
- **WHEN** a resolution is refused
- **THEN** the error lists its files in `paths`
  [@test] crates/bd-update/src/resolve_test.rs (`markers_an_edit_outside_a_block_and_a_missing_file_are_refused`)

#### Scenario: An unsupported conflict
- **WHEN** a round is `unsupported_conflict`
- **THEN** the error lists the files in `paths`, each unsupported path's conflict type in the message
  [@test] crates/bd-update/src/resolve_test.rs (`binary_and_file_directory_conflicts_are_refused_and_start_no_session`, `an_unsupported_mixed_rename_and_binary_conflict_names_the_pair`)

### Requirement: Error codes: An unsupported rename/rename names its pair

An unsupported rename/rename message MUST also list `from → onto / commit`, and the error output MUST include
`rename_pairs`.

#### Scenario: A rename/rename beside a binary conflict
- **WHEN** a round with a rename/rename and a binary conflict is `unsupported_conflict`
- **THEN** the message lists `from → onto / commit`, and the error has `rename_pairs`
  [@test] crates/bd-update/src/resolve_test.rs (`an_unsupported_mixed_rename_and_binary_conflict_names_the_pair`)

### Requirement: Error codes: An unsupported conflict offers the abort, then the unapply if it can run

The message of an unsupported conflict MUST offer `brd update --abort`, then `brd unapply <branch>` only when the
unapply dry run succeeds; otherwise it MUST say to abort and ask the user. It MUST NOT tell the agent to update in a
terminal.

#### Scenario: The unapply dry run succeeds
- **WHEN** a round is `unsupported_conflict` and the unapply dry run of its branch succeeds
- **THEN** the message offers `brd update --abort`, then `brd unapply <branch>`, and no update in a terminal
  [@test] crates/bd-update/src/resolve_test.rs (`binary_and_file_directory_conflicts_are_refused_and_start_no_session`)

#### Scenario: The unapply dry run refuses
- **WHEN** a round is `unsupported_conflict` and the unapply dry run refuses
- **THEN** the message says to abort and ask the user
  [@test] crates/bd-update/src/resolve_test.rs (`an_unsupported_conflict_asks_the_user_when_unapply_dry_run_refuses`)

### Requirement: `brd help update`: It prints the session workflow

`brd help update` MUST print the session workflow: `dropped` and its proofs, `ask_user` as the only reason to ask the
user, the round object, `pending`, edit only the files at `edit` (a change outside the blocks waits for a `fixup!`),
`--continue` with its resolutions, the exit codes, the report of the resolutions, what follows exit 0 or a failure,
`finish_pending`, `--abort`, `--status` and `--dir`, and that a copy is written as UTF-8 without a BOM.

#### Scenario: The text of `brd help update`
- **WHEN** `brd help update` runs
- **THEN** it names `dropped` and its proofs, `ask_user` as the only reason to ask the user, and `pending`
- **AND** the round object with `landed_as`, `landed_differs`, `blocked_paths`, `marker_labels` and `next`
- **AND** edit only the files at `edit` (a change outside the blocks waits for a `fixup!`)
- **AND** `--continue` with `--take`, `--delete`, `--allow-empty`, `--drop-empty`, `--landed` and `--user`
- **AND** the exit codes, the report of the resolutions, the build and the tests of `verify.paths` after exit 0 and a
  `fixup!` with `brd fold` or `brd undo` on a failure
- **AND** `finish_pending`, `--abort` (with `--allow-live-session` for a live session of another worktree), `--status`
  and `--dir`
- **AND** that a copy is written as UTF-8 without a BOM (`[IO.File]::WriteAllText` in Windows PowerShell)
  [@test] crates/bd-cli/src/agent_help_test.rs (`agent_help_names_every_command_and_option_that_exists`)

### Requirement: `brd help update`: `brd help agent` only names it, and the two name every option

`brd help agent` MUST only name `brd help update`; together the two texts MUST name every option of `brd update`.

#### Scenario: Every option of `brd update`
- **WHEN** the options of `brd update` are compared with `brd help agent` and `brd help update`
- **THEN** the two texts together name every option
  [@test] crates/bd-cli/src/agent_help_test.rs (`agent_help_names_every_command_and_option_that_exists`)

## Testing / Local Run

- `cargo nextest run -p bd-cli resolve_test` runs the session end to end on `TestRepo` fixtures; some use a `BD_GIT`
  wrapper that counts the replays, fails a step once or runs a `git switch` at a chosen moment.
- `cargo nextest run -p git-ops resolve_check update_resolve merge_tree_output` covers the checks, the conflict types
  and the parser.

## Open Questions / Risks

- An unsupported conflict in a later round stops the session: `--abort` drops the earlier rounds too.
- `session.blocked_paths` is the cheaper check of every sync; the exact check runs only at the finish.
- A note does not follow a later amend or reword of its commit: it is an audit record of one session.
- `--continue --dry-run` writes the resolved commit on a temporary ref and deletes it: no ref differs afterwards.

## References

- [Git Operations](../../../../docs/design/git-operations.md): "Resolve session" and "Update project".
- [Update Project](../../repository/update-project/spec.md): the app's update and the copied prompts.
- [Agent CLI guide](../../../../docs/contributing/agent-cli.md).
