---
name: Update Project
description: Requirements for Update Project, which fetches the baseline and rebases the checked out branch onto it without touching unrelated local changes, the header state of the update, and the hand-off of a conflict to a coding agent.
style: plain-1
targets:
  - crates/git-ops/src/update_branch.rs
  - crates/git-ops/src/paths.rs
  - crates/git-ops/src/replay.rs
  - crates/git-ops/src/revs.rs
  - crates/git-ops/src/worktree_update.rs
  - crates/update-core/src/update_project.rs
  - crates/app-api/src/api_error.rs
  - crates/sync-core/src/baseline.rs
  - crates/sync-core/src/fetch_lock.rs
  - crates/sync-core/src/transfer_progress.rs
  - crates/sync-core/src/remote_command.rs
  - crates/sync-core/src/sync.rs
  - crates/history-rewrite/src/repository_lock.rs
  - crates/git-executor/src/worktree_safety_test.rs
  - crates/operations/src/update.rs
  - src-tauri/src/menu.rs
  - src/components/repository/UpdateProjectButton.tsx
  - src/components/repository/UpdateBlockedDialog.tsx
  - src/components/repository/ConfigurationHeader.tsx
  - src/components/repository/SyncButton.tsx
  - src/components/repository/RepositorySelector.tsx
  - src/lib/home-path.ts
  - src-tauri/src/lib.rs
  - src/components/shared/RelativeTime.tsx
  - src/hooks/useNow.ts
  - src/stores/update-project.ts
  - src/lib/errors.ts
  - src/stores/branch-sync.ts
  - src/lib/update-state.ts
  - src/lib/update-progress.ts
  - src/hooks/useRunProgress.ts
  - src/components/shared/ProgressLine.tsx
  - src/components/repository/DockStatus.tsx
  - src/lib/transfer-progress.ts
  - src/lib/agent-prompt.ts
  - crates/app-api/src/app/agent_cli.rs
  - src-tauri/src/agent_cli.rs
  - src/lib/update-conflict.ts
  - src/lib/text.ts
  - crates/sync-utils/src/list_cut.rs
  - crates/operations/src/events.rs
  - crates/operations/src/run_frame.rs
  - src/model/intents.ts
  - crates/update-core/src/error_cases.rs
  - crates/git-ops/src/word_replay.rs
  - crates/git-ops/src/index_lock.rs
  - crates/domain-types/src/update.rs
  - crates/domain-types/src/update_project.rs
  - src/presenters/lock-wait.ts
  - src/presenters/operations.ts
  - src/presenters/outcome.ts
---

# Update Project

Status: Implemented
Date: 2026-10-28

## Purpose

Update Project (`⌘T`, as in IntelliJ) fetches the baseline (e.g. `origin/master`) and brings the checked out branch
onto it: a fast-forward without local commits, otherwise a rebase of the local commits. It works with uncommitted and
staged changes as long as the upstream doesn't change those files. A conflict or an overlap changes nothing; a dialog
explains it and copies a ready prompt for a coding agent. With the app's bd, the agent resolves a conflict in a resolve
session of `brd update`, outside the working tree ([brd update](../../cli/update/spec.md)).

The header shows whether an update would apply cleanly, from a check that every sync runs against the last fetch.

## Goals

- Update without leaving the app, and without stashing local work that the update doesn't touch.
- Know before updating whether it conflicts.
- Never leave a half-done rebase: either the update applies as a whole, or nothing changes.

## Non-goals

- No count of incoming commits anywhere: a monorepo gets hundreds, and only conflicts matter. The state is up to date,
  applies cleanly, conflicts in N files, or blocked by uncommitted changes. The user's own commits that the baseline
  already has are named, not counted as incoming ([Landed Commits](../landed-commits/spec.md)).
- The app does not resolve conflicts and does not launch an agent; it copies a prompt. The agent resolves a conflict
  with `brd update` and its resolve session, and a blocked update goes to the user, then to `brd update`; neither
  stashes. Without a brd command, a conflict gets the git steps, which never stash either.
- No merge of the upstream (`pull --no-rebase`), no update of submodules.

## Concepts

### Conflict and blocked dialog

The brd conflict prompt leaves the protocol to brd because a prompt that restated it drifted from brd: it put
`round_moved`, exit 3, under exit 2, and its "exit 3 retries" then sent the agent back to the same refused round, and
its "ask the user" on an empty result made the agent ask about every commit that upstream had already.

A conflict prompt is a map, not the record: hundreds of files with 50 upstream commits each were
hundreds of kilobytes.

## Requirements

### Requirement: Check: The check uses no network

The check MUST NOT use the network: it compares the branch with the tracking ref of the last fetch.

#### Scenario: A check compares with the last fetch
- **WHEN** the check runs
- **THEN** it uses no network and compares the branch with the tracking ref of the last fetch

### Requirement: Check: The check refuses during an operation, a detached HEAD or a merge

The check MUST refuse (an error, not a state) while a rebase, merge, cherry-pick or revert is in progress, on a
detached HEAD, and when the local commits contain a merge.

#### Scenario: A rebase in progress
- **GIVEN** a rebase, merge, cherry-pick or revert is in progress
- **WHEN** the check runs
- **THEN** it refuses with an error, not a state
  [@test] crates/update-core/src/update_project_test.rs (`check_refuses_while_a_rebase_is_in_progress`)

#### Scenario: A detached HEAD
- **GIVEN** HEAD is detached
- **WHEN** the check runs
- **THEN** it refuses with an error, not a state

#### Scenario: A merge in the local commits
- **GIVEN** the local commits contain a merge
- **WHEN** the check runs
- **THEN** it refuses with an error, not a state
  [@test] crates/update-core/src/update_project_test.rs (`merges_in_the_local_commits_are_refused`)

### Requirement: Check: A local baseline has nothing to update from

A local baseline has nothing to update from: the system SHALL run no check for it, and an update SHALL say so.

#### Scenario: The baseline is a local branch
- **GIVEN** the baseline is local
- **WHEN** a sync or an update runs
- **THEN** no check runs, and an update says that there is nothing to update from
  [@test] crates/update-core/src/update_project_test.rs (`local_baseline_has_nothing_to_update`)

### Requirement: Check: Unrelated histories are refused as such

A branch and an upstream without a common history MUST be refused as such.

#### Scenario: No common history
- **GIVEN** the branch and the upstream have no common history
- **WHEN** the check runs
- **THEN** it is refused as "no common history"
  [@test] crates/git-ops/src/update_branch_test.rs (`unrelated_histories_have_no_common_history`)

### Requirement: Check: A history git cannot read is an error with its message

A history that git can't read (an object that is not there, a tracking ref that is gone) MUST be an error with git's
message, never "no common history" or "no fork point".

#### Scenario: A missing object or a tracking ref that is gone
- **WHEN** the check meets an object that is not there, or a tracking ref that is gone
- **THEN** it is an error with git's message
- **AND** never "no common history" or "no fork point"
  [@test] crates/git-ops/src/update_branch_test.rs (`a_history_git_cannot_read_is_an_error_with_its_message_not_unrelated_histories`, `the_fork_point_of_a_tracking_ref_that_is_gone_is_an_error`)

### Requirement: Check: The state is up to date, clean, conflict or blocked

The state MUST be one of: up to date, the upstream has nothing the branch lacks; clean, as a fast-forward or a rebase;
conflict, with the first conflict (`firstConflict`); blocked, with the local paths that overlap the update.

#### Scenario: Up to date
- **WHEN** the upstream has nothing the branch lacks
- **THEN** the state is up to date

#### Scenario: Clean
- **WHEN** the update applies without a conflict
- **THEN** the state is clean, as a fast-forward or a rebase

#### Scenario: Conflict
- **WHEN** a local commit conflicts with the upstream
- **THEN** the state is conflict, with the first conflict (`firstConflict`), and nothing changes
  [@test] crates/update-core/src/update_project_test.rs (`same_line_conflict_reports_the_first_conflict_and_changes_nothing`)

#### Scenario: Blocked
- **WHEN** local paths overlap the update
- **THEN** the state is blocked, with those paths, and nothing changes
  [@test] crates/update-core/src/update_project_test.rs (`dirty_touched_path_blocks_and_changes_nothing`)

### Requirement: Check: The first conflict names its commit, its files and the upstream commits

The first conflict MUST be the first local commit whose replay conflicts after the commits the update leaves out
because the upstream has them, with the files of its merge onto the commits before it (the first 50, and the count of
all), and the upstream commits that change those files (the newest 50, oldest first).

#### Scenario: A wide conflict
- **WHEN** the first conflict has more than 50 files, and more than 50 upstream commits change them
- **THEN** it names the first 50 files and the count of all
- **AND** the upstream commits that change those files, the newest 50, oldest first
  [@test] crates/update-core/src/update_project_test.rs (`wide_conflict_reports_the_first_files_with_their_commits_and_counts_all`)

### Requirement: Check: Each commit of the first conflict has its people, times and files

Each commit of the first conflict MUST have its author and author time, its committer and committer time (for an
upstream commit, about when it reached the upstream, e.g. when a robot landed it) and the conflicting files it changes.

#### Scenario: The commits of a conflict
- **WHEN** the state is conflict
- **THEN** each of its commits has its author and author time, its committer and committer time and the conflicting files it changes
- **AND** an upstream commit's committer time is about when it reached the upstream, e.g. when a robot landed it
  [@test] crates/update-core/src/update_project_test.rs (`wide_conflict_reports_the_first_files_with_their_commits_and_counts_all`)

### Requirement: Check: The search for the first conflict replays each commit about once

The search for the first conflict MUST replay one run of commits at a time, and each probe of its binary search MUST
start from the longest clean prefix, so a run of n commits costs about n replayed commits, not ⌈log₂ n⌉ × n/2.

#### Scenario: A probe starts from the longest clean prefix
- **WHEN** the binary search probes a run of commits
- **THEN** each probe starts from the longest clean prefix
  [@test] crates/git-ops/src/replay_test.rs (`each_probe_starts_from_the_longest_clean_prefix`)

#### Scenario: The runs replay until the first that conflicts
- **WHEN** the search replays the local commits
- **THEN** it replays one run at a time, until the first run that conflicts
  [@test] crates/git-ops/src/replay_test.rs (`the_runs_replay_until_the_first_that_conflicts`)

### Requirement: Check: With the resolve by words, the first conflict is where its replay stopped

With the resolve by words on, the first conflict MUST be the commit where that replay stopped.

#### Scenario: A commit whose words conflict too
- **GIVEN** the resolve by words is on
- **WHEN** the replay by words stops at a commit whose words conflict too
- **THEN** that commit is the first conflict
  [@test] crates/git-ops/src/word_replay_test.rs (`a_commit_whose_words_conflict_too_stops_the_replay`)

### Requirement: Check: A resolve session opens its first round at the first conflict

A resolve session MUST open its first round at the commit of the first conflict, without a search of its own.

#### Scenario: The first round of a resolve session
- **WHEN** a resolve session starts on a conflict
- **THEN** its first round opens at the commit of the first conflict, without a search of its own
  [@test] crates/bd-update/src/resolve_test.rs (`the_first_conflict_is_found_with_a_binary_search`)

### Requirement: Check: The search and the first round run before the repository lock

The search for the first conflict and the first round of a resolve session MUST run before the repository lock, which
holds only the witness, the state of the session and the copies of the round.

#### Scenario: A regroup during the search
- **WHEN** a regroup runs while a resolve session searches for its first conflict
- **THEN** the regroup is done, as the search holds no lock
- **AND** the session opens on the regrouped HEAD
  [@test] crates/update-core/src/update_project_test.rs (`a_regroup_during_the_first_conflict_search_of_a_resolve_session_is_done_and_the_session_opens_on_it`)

### Requirement: Check: Blocked names the local paths that overlap the update

Blocked MUST name the local paths that overlap the update (the first 100, and the count of all): uncommitted or staged
changes on a file the upstream changes, an untracked file where it adds one, an ignored file where it adds one.

#### Scenario: An uncommitted change on a file the upstream changes
- **GIVEN** an uncommitted change on a file the upstream changes
- **WHEN** the check runs
- **THEN** the state is blocked by that path
  [@test] crates/update-core/src/update_project_test.rs (`dirty_touched_path_blocks_and_changes_nothing`)

#### Scenario: A staged change on a file the upstream changes
- **GIVEN** a staged change on a file the upstream changes
- **WHEN** the check runs
- **THEN** the state is blocked by that path
  [@test] crates/update-core/src/update_project_test.rs (`staged_touched_path_blocks`)

#### Scenario: An untracked file where the upstream adds one
- **GIVEN** an untracked file where the upstream adds one
- **WHEN** the check runs
- **THEN** the state is blocked by that path
  [@test] crates/update-core/src/update_project_test.rs (`untracked_file_that_upstream_adds_blocks`)

#### Scenario: An ignored file where the upstream adds one
- **GIVEN** an ignored file where the upstream adds one
- **WHEN** the check runs
- **THEN** the state is blocked by that path
  [@test] crates/update-core/src/update_project_test.rs (`ignored_file_that_upstream_adds_blocks`)

#### Scenario: Many blocking paths
- **GIVEN** more than 100 local paths overlap the update
- **WHEN** the check runs
- **THEN** the state names the first 100 and the count of all
  [@test] crates/update-core/src/update_project_test.rs (`many_blocking_paths_are_counted`)

### Requirement: Check: A file that was only touched does not block

A file that was only touched (same content, new modification time) MUST NOT block.

#### Scenario: A touched file the upstream changes
- **GIVEN** a file the upstream changes has the same content and a new modification time
- **WHEN** the check runs
- **THEN** it does not block
  [@test] crates/update-core/src/update_project_test.rs (`touched_only_file_passes_thanks_to_the_refresh`)

### Requirement: Check: A path that is not UTF-8 is checked by its bytes

A path that is not UTF-8 MUST be checked by its bytes, its index entry and its file; the state SHALL show it with
U+FFFD for the bytes that are not UTF-8.

#### Scenario: A path with bytes that are not UTF-8
- **GIVEN** a path that is not UTF-8
- **WHEN** the check runs
- **THEN** it checks the path by its bytes, its index entry and its file
- **AND** the state shows it with U+FFFD for the bytes that are not UTF-8
  [@test] crates/git-ops/src/update_branch_test.rs (`a_path_that_is_not_utf8_names_its_index_entry`)

### Requirement: Check: Commits an upstream force-push removed are not replayed

Commits that an upstream force-push removed MUST NOT be replayed: the old tip of a forced fetch and the fork point of
the tracking ref's reflog are excluded, as `git pull --rebase` does.

#### Scenario: The old tip of a forced fetch
- **GIVEN** a forced fetch removed upstream commits
- **WHEN** the check runs
- **THEN** the old tip of the forced fetch is excluded and the removed commits are not replayed
  [@test] crates/update-core/src/update_project_test.rs (`upstream_force_push_does_not_replay_the_removed_commits`)
  [@test] crates/update-core/src/update_project_test.rs (`parses_the_forced_update_line_of_fetch`)

#### Scenario: A forced fetch elsewhere
- **GIVEN** a forced fetch ran elsewhere
- **WHEN** the check runs
- **THEN** the fork point of the tracking ref's reflog is excluded
  [@test] crates/update-core/src/update_project_test.rs (`check_after_a_forced_fetch_elsewhere_uses_the_fork_point`)

### Requirement: Check: Every sync runs the check and reports it once

Every sync MUST run the check and report it once, beside the rest of the sync (the event can come at any point); a
failed check is reported with its reason and MUST NOT fail the sync.

#### Scenario: A sync reports the baseline status
- **WHEN** a sync runs
- **THEN** it runs the check and reports it once, beside the rest of the sync
  [@test] crates/update-core/src/update_project_test.rs (`sync_reports_the_baseline_status`)

#### Scenario: The event can come at any point
- **WHEN** a sync reports its events
- **THEN** the event of the check can come at any point among them
  [@test] crates/sync-core/src/sync_test.rs (`test_branches_grouped_precedes_per_branch_events`)

#### Scenario: A failed check
- **WHEN** the check of a sync fails
- **THEN** it is reported with its reason, and the sync does not fail

### Requirement: Check: A deferred sync runs no check

A sync deferred for a detached HEAD, an operation in progress or a checked out branch under the prefix
(`SyncDeferred`, [Onboarding](../onboarding/spec.md)) MUST run no check and report nothing.

#### Scenario: A sync on a detached HEAD
- **GIVEN** HEAD is detached, an operation is in progress or a branch under the prefix is checked out
- **WHEN** a sync is deferred (`SyncDeferred`)
- **THEN** it runs no check and reports nothing
  [@test] crates/update-core/src/update_project_test.rs (`sync_on_a_detached_head_is_deferred_before_the_update_check`)

### Requirement: Check: Repeated checks write no new objects, a sync's check can't be applied

Repeated checks of the same state MUST NOT write new objects; a check made for a sync MUST NOT be one that an update
can apply.

#### Scenario: The same state checked again
- **WHEN** the same state is checked again
- **THEN** no new objects are written
  [@test] crates/update-core/src/update_project_test.rs (`preview_check_cannot_be_applied`)

#### Scenario: Applying the check of a sync
- **WHEN** an update is given a check made for a sync
- **THEN** it can't apply it
  [@test] crates/update-core/src/update_project_test.rs (`preview_check_cannot_be_applied`)

### Requirement: Check: The check of a sync costs in proportion to the paths the update changes

The check inside a sync MUST cost in proportion to the paths the update changes, not the size of the index: it never
copies, refreshes or writes the index, reuses the replay of an unchanged HEAD and upstream, and rereads only files
whose stat data changed.

#### Scenario: An unchanged state
- **GIVEN** HEAD and the upstream did not change since the last check
- **WHEN** a sync checks again
- **THEN** it reuses the replay
  [@test] crates/update-core/src/update_project_test.rs (`preview_reuses_the_replay_of_an_unchanged_state`)

#### Scenario: The index stays as it is
- **WHEN** a sync checks
- **THEN** it never copies, refreshes or writes the index, and rereads only files whose stat data changed
  [@test] crates/update-core/src/update_project_test.rs (`preview_blocks_dirty_and_staged_touched_paths_without_touching_the_index`)

### Requirement: Check: The check of a sync blocks as the update does

The check inside a sync MUST still find dirty and staged touched paths, MUST NOT block on a file that was only touched,
and MUST block on untracked and ignored files where the upstream adds one.

#### Scenario: Dirty and staged touched paths
- **GIVEN** dirty and staged changes on files the upstream changes
- **WHEN** a sync checks
- **THEN** they block, and the index is not touched
  [@test] crates/update-core/src/update_project_test.rs (`preview_blocks_dirty_and_staged_touched_paths_without_touching_the_index`)

#### Scenario: A file that was only touched
- **GIVEN** a file that was only touched
- **WHEN** a sync checks
- **THEN** it does not block
  [@test] crates/update-core/src/update_project_test.rs (`preview_does_not_block_a_touched_only_file`)

#### Scenario: Untracked and ignored files where the upstream adds one
- **GIVEN** untracked and ignored files where the upstream adds one
- **WHEN** a sync checks
- **THEN** they block
  [@test] crates/update-core/src/update_project_test.rs (`preview_blocks_untracked_and_ignored_files_that_upstream_adds`)

### Requirement: Check: A conflict found once is not replayed again

A conflict found once MUST NOT be replayed again for the same HEAD, upstream and previous tips: the update after the
sync that found it, and the sync after a conflicting update, reuse it (a conflict doesn't depend on the committer
date; a clean outcome is never shared).

#### Scenario: The update after the sync that found the conflict
- **GIVEN** a sync found a conflict
- **WHEN** an update runs for the same HEAD, upstream and previous tips
- **THEN** it reuses the conflict and does not replay again
  [@test] crates/update-core/src/update_project_test.rs (`update_after_the_sync_found_a_conflict_does_not_replay_again`)

#### Scenario: The sync after a conflicting update
- **GIVEN** an update conflicted
- **WHEN** a sync runs for the same HEAD, upstream and previous tips
- **THEN** it reuses the conflict and does not replay again
  [@test] crates/update-core/src/update_project_test.rs (`sync_after_a_conflicting_update_does_not_replay_again`)

#### Scenario: A clean outcome
- **WHEN** a replay is clean
- **THEN** its outcome is never shared between the sync and the update

### Requirement: Check: A conflicting replay is tried again with the resolve by words

With `branchdeck.resolveByWords` on (the default), a replay that conflicts MUST be tried again commit by commit, each
conflict of git resolved by words as the sync resolves it ([Git Operations](../../../../docs/design/git-operations.md),
"Resolve by words").

#### Scenario: A conflict that resolves by words
- **GIVEN** `branchdeck.resolveByWords` is on
- **WHEN** a commit's replay conflicts and its conflict resolves by words
- **THEN** the resolved commit is committed and the rest replays onto it
  [@test] crates/git-ops/src/word_replay_test.rs (`a_conflict_that_resolves_by_words_is_committed_and_the_rest_replays_onto_it`)

#### Scenario: A commit that the words leave without a change
- **WHEN** the resolve by words leaves a commit without a change
- **THEN** that commit is dropped
  [@test] crates/git-ops/src/word_replay_test.rs (`a_commit_that_the_words_leave_without_a_change_is_dropped`)

### Requirement: Check: The resolve by words is all or nothing

All or nothing: the state MUST be a clean rebase only when every conflicting commit resolves by words, and it lists them
(`resolvedByWords`: the commit, its subject, the conflicts and the files). The replay of the sync's check and of the
update are cached apart by the mode.

#### Scenario: Every conflict resolves by words
- **WHEN** every conflicting commit resolves by words
- **THEN** the state is a clean rebase that lists them (`resolvedByWords`), and the update applies without a session
  [@test] crates/git-ops/src/update_resolve_test.rs (`an_update_whose_every_conflict_resolves_by_words_applies_without_a_session`)

#### Scenario: A commit whose words conflict too
- **WHEN** a conflicting commit does not resolve by words
- **THEN** the replay stops there and the state is not a clean rebase
  [@test] crates/git-ops/src/word_replay_test.rs (`a_commit_whose_words_conflict_too_stops_the_replay`)

#### Scenario: The sync's check and the update in different modes
- **WHEN** the sync's check and the update replay the same state
- **THEN** their replays are cached apart by the mode

### Requirement: Check: A file the upstream replaces by a directory does not block

A file that the upstream replaces by a directory (or the reverse) MUST NOT block.

#### Scenario: A file replaced by a directory upstream
- **GIVEN** the upstream replaces a file by a directory, or the reverse
- **WHEN** the check runs
- **THEN** it is not in the way
  [@test] crates/update-core/src/update_project_test.rs (`file_replaced_by_a_directory_upstream_is_not_in_the_way`)

### Requirement: Update: The update fetches only the baseline's remote branch

The update MUST fetch only the baseline's remote branch, so the push lease of the virtual branches still compares
against what the last push saw, then check again and apply a clean result.

#### Scenario: The fetch of an update
- **WHEN** the update runs
- **THEN** it fetches only the baseline's remote branch, so the push lease of the virtual branches still compares against what the last push saw

#### Scenario: After the fetch
- **WHEN** the fetch is done
- **THEN** the update checks again and applies a clean result

### Requirement: Update: A failed fetch changes nothing

A failed fetch MUST change nothing.

#### Scenario: The remote is gone
- **WHEN** the fetch of the update fails
- **THEN** nothing changes, and the error is a failed fetch of the upstream
  [@test] crates/update-core/src/update_project_test.rs (`a_failed_fetch_is_fetch_failed`)

#### Scenario: A failed fetch runs once
- **WHEN** the fetch fails
- **THEN** it is not repeated
  [@test] crates/sync-core/src/baseline_fetch_test.rs (`a_failed_fetch_is_not_repeated`)

### Requirement: Update: A branch under the prefix is refused before the fetch

With a branch under the prefix checked out (a virtual, unapplied, archived or stack branch, switched to by hand), the
update MUST refuse before the fetch (`managed_branch`) and move neither the branch nor the tracking ref: it would
rebase a copy of one group instead of the user's branch. Only a branch with a `/` costs the read of the prefix.

#### Scenario: A virtual branch checked out by hand
- **GIVEN** a branch under the prefix is checked out
- **WHEN** the update runs
- **THEN** it refuses before the fetch (`managed_branch`)
- **AND** neither the branch nor the tracking ref moves
  [@test] crates/update-core/src/update_project_test.rs (`an_update_of_a_branch_under_the_prefix_is_refused_before_the_fetch`)

#### Scenario: A branch without a slash
- **GIVEN** the checked out branch has no `/` in its name
- **WHEN** the update runs
- **THEN** it does not read the prefix

### Requirement: Update: A fetch beside another fetch of its tracking ref takes its result

A fetch beside another fetch of the same tracking ref, of any process (`brd update` in a lease while the app updates
or fetches: one common dir), MUST wait for it while its process lives and take its result once it ended well, also
when the remote had nothing new, or once the ref moved: one transfer, both succeed, and a forced update gives both the
old tip.

#### Scenario: Another fetch of the same tracking ref
- **GIVEN** another process fetches the same tracking ref
- **WHEN** a fetch starts beside it
- **THEN** it waits while that process lives and takes its result once it ended well: one transfer, both succeed
  [@test] crates/sync-core/src/fetch_lock_test.rs (`a_fetch_beside_another_fetch_of_its_tracking_ref_takes_its_result`)

#### Scenario: The other fetch found nothing new
- **WHEN** the other fetch ended well and the remote had nothing new
- **THEN** the waiting fetch takes its result too
  [@test] crates/sync-core/src/fetch_lock_test.rs (`a_fetch_beside_another_that_found_nothing_new_takes_its_result`)

#### Scenario: The other fetch was a forced update
- **WHEN** the other fetch was a forced update
- **THEN** both get the old tip
  [@test] crates/sync-core/src/fetch_lock_test.rs (`a_fetch_that_joins_a_forced_update_returns_the_old_tip`)

#### Scenario: The app's fetch beside `brd update`
- **GIVEN** `brd update` in a lease fetches while the app updates or fetches (one common dir)
- **WHEN** the app's fetch starts beside it
- **THEN** it takes the transfer of `brd update`
  [@test] crates/bd-update/src/update_test.rs (`the_fetch_of_the_app_beside_bd_update_takes_its_transfer`)

### Requirement: Update: A fetch whose holder failed fetches itself

A fetch whose holder failed or was killed, or fetched another tracking ref, MUST fetch itself once the other one
ended; neither fails on the other's ref lock.

#### Scenario: The holder failed
- **WHEN** the fetch it waited for failed
- **THEN** it fetches itself
  [@test] crates/sync-core/src/fetch_lock_test.rs (`a_fetch_that_waited_for_one_that_failed_fetches_itself`)

#### Scenario: The holder fetched another tracking ref
- **WHEN** the other fetch is of another tracking ref
- **THEN** it waits, then fetches itself, and neither fails on the other's ref lock
  [@test] crates/sync-core/src/fetch_lock_test.rs (`a_fetch_of_another_tracking_ref_waits_and_fetches_itself`)

#### Scenario: The holder was killed
- **WHEN** the holder of the fetch was killed
- **THEN** the fetch fetches at once
  [@test] crates/sync-core/src/fetch_lock_test.rs (`a_fetch_after_a_killed_holder_fetches_at_once`)

### Requirement: Update: The user's stop ends the wait for another fetch

The user's stop of the app's update MUST end its wait for another fetch.

#### Scenario: The user stops the update while it waits
- **GIVEN** the app's update waits for another fetch
- **WHEN** the user stops it
- **THEN** its wait ends without a transfer
  [@test] crates/sync-core/src/fetch_lock_test.rs (`a_canceled_wait_for_another_fetch_ends_without_a_transfer`)

### Requirement: Update: The check runs without the lock, which holds only the move

After the fetch, the update MUST check (the replay, the resolution by words, the landed candidates) without the
repository lock, on the HEAD and the upstream it read once, then take the lock only for the witness, the local changes
on the paths it writes and the move, so the app, `brd update` and the rewrites of either never interleave with the
move, while a regroup, a push or an agent's `brd` goes on during the check.

#### Scenario: The lock is taken after the fetch and the check
- **GIVEN** another process holds the repository lock
- **WHEN** the update runs
- **THEN** the fetch and the check run before the lock, then the update waits for it as a step of its own
  [@test] crates/update-core/src/update_project_test.rs (`update_takes_the_repository_lock_after_the_fetch_and_says_it_waits`)
  [@test] crates/bd-update/src/update_test.rs (`the_update_waits_for_the_repository_lock_after_its_fetch`)

#### Scenario: What runs under the lock
- **WHEN** the app's update applies
- **THEN** the replay runs before the lock, and the lock spans only the witness, the local changes and the move
  [@test] crates/update-core/src/update_project_test.rs (`the_lock_of_the_apps_update_spans_only_the_witness_the_local_changes_and_the_move`)
  [@test] crates/bd-update/src/update_test.rs (`the_check_of_an_update_runs_before_the_lock`)

### Requirement: Update: The fetch, the counts, the notes and the landed record run outside the lock

The fetch, the counts and notes of what the update did and the record of what landed MUST never run under the lock.

#### Scenario: The counts, the notes and the landed report after the lock
- **WHEN** an update applies, also one that resolves conflicts by words
- **THEN** the counts of what it did, the notes of its resolved commits and the report of what landed are written after the lock
  [@test] crates/bd-update/src/update_test.rs (`the_check_of_an_update_runs_before_the_lock`)
  [@test] crates/update-core/src/update_project_test.rs (`the_lock_of_the_apps_update_spans_only_the_witness_the_local_changes_and_the_move`)

### Requirement: Update: A late record of an update only adds what it landed

A late record of an update that another update already moved past MUST only add what it landed: the state keeps naming
the later update, whose undo restores the report from before it.

#### Scenario: Two updates whose records come out of order
- **GIVEN** another update already moved past an update
- **WHEN** the record of the earlier update comes late
- **THEN** it only adds what it landed, and the state keeps naming the later update
- **AND** the undo of the later update restores the report from before it
  [@test] crates/update-core/src/landed_test.rs (`a_late_record_of_an_update_keeps_the_record_of_the_update_after_it`)

### Requirement: Update: A check that moves nothing answers without waiting for the lock

A check that moves nothing (up to date, or a conflict, which starts no session in the app) MUST be the answer at once,
without waiting for the lock, unless an interrupted update left files to follow.

#### Scenario: Up to date or a conflict while the lock is held
- **GIVEN** another process holds the repository lock
- **WHEN** the check of an update is up to date or a conflict
- **THEN** it is the answer at once, without waiting for the lock
  [@test] crates/update-core/src/update_project_test.rs (`an_update_that_moves_nothing_answers_without_waiting_for_the_lock`)

#### Scenario: An interrupted update left files to follow
- **GIVEN** an interrupted update left files to follow
- **WHEN** a check moves nothing
- **THEN** the update still takes the lock

### Requirement: Update: A branch or upstream moved during the check is checked again

A branch or an upstream moved during the check MUST be checked once more without the lock, so no other writer waits
for its replay, and the second time under the lock; a file the update writes, edited during the check, MUST block it.

#### Scenario: A regroup during the replay
- **WHEN** a regroup moves the branch during the replay of an update
- **THEN** the regroup is done, and the update checks once more without the lock, then the second time under it
  [@test] crates/bd-update/src/update_test.rs (`a_regroup_during_the_replay_of_an_update_is_done_and_the_update_checks_again_without_the_lock`)

#### Scenario: A file edited during the replay
- **WHEN** a file the update writes is edited during the check
- **THEN** it blocks the update
  [@test] crates/bd-update/src/update_test.rs (`a_file_edited_during_the_replay_of_an_update_blocks_it`)

### Requirement: Update: The app's update reads the stat data once under the lock

The app's update MUST read the stat data of the whole worktree once under the lock: the refresh of the index that its
check of the local changes copies and its move takes as it is.

#### Scenario: One stat pass under the lock
- **WHEN** the app's update applies
- **THEN** it refreshes the index once under the lock, and its check of the local changes and its move use that refresh
  [@test] crates/update-core/src/update_project_test.rs (`the_lock_of_the_apps_update_spans_only_the_witness_the_local_changes_and_the_move`)

### Requirement: Update: A lock held for the whole wait is `locked` with its holder

A lock held for the whole wait of `brd update` (10 s), or in the app one that no record names a live holder of for
10 s, MUST be an error with only the fetch and the check done, `locked` with the holder its record names
(`details.holder`: `brd` or the app, its command, pid and how long it has held the lock; none once the record is gone).

#### Scenario: Another process holds the lock for the whole wait
- **GIVEN** another process holds the repository lock for the whole bounded wait of `brd update`
- **WHEN** the update runs
- **THEN** it fails `locked` with the holder its record names, with only the fetch and the check done
  [@test] crates/update-core/src/update_project_test.rs (`update_takes_the_repository_lock_after_the_fetch_and_says_it_waits`)
  [@test] crates/bd-update/src/update_test.rs (`the_update_waits_for_the_repository_lock_after_its_fetch`)

#### Scenario: The holder of the lock
- **WHEN** the error is `locked`
- **THEN** `details.holder` names `brd` or the app, its command, pid and how long it has held the lock, and none once the record is gone
  [@test] crates/app-api/src/api_error_test.rs (`api_error_names_the_holder_of_a_lock_with_how_long_it_has_held_it`)

### Requirement: Update: The app says who holds the repository and keeps the fetch time

The app MUST show "Cannot update the project" with the holder, worded by the page from the code and its data, never
the lock file path, and MUST keep the time of the fetch.

#### Scenario: The holder has a record
- **WHEN** the update fails `locked` with a holder
- **THEN** the app shows "Cannot update the project" with "`brd push auth` (pid 4242) has held the repository for 40 s"
- **AND** the app's own holder as "Branch Deck (pid 7) …"
  [@test] tests/e2e/update-project/update-project.spec.ts (`should name the holder of the repository lock and retry the update without a fetch`)
  [@test] src/lib/errors.test.ts (`names the holder of a held repository lock and how long it has held it, as the lock record says it`)
  [@test] src/lib/errors.test.ts (`toasts a held repository lock with its holder, without the holder the other operation, and a plain Error with its message`)

#### Scenario: The holder left no record
- **WHEN** the update fails `locked` without a record
- **THEN** the app says "Another Branch Deck operation holds the repository", never the lock file path
  [@test] src/lib/errors.test.ts (`says another operation holds the repository when the holder left no record, never the lock file`)

#### Scenario: The time of the fetch
- **WHEN** the update fails after its fetch, e.g. while brd holds the repository
- **THEN** the app keeps the time of the fetch
  [@test] src/stores/update-project.test.ts (`keeps the fetch time when the update fails after its fetch, e.g. while brd holds the repository`)

### Requirement: Update: An update refused by a held lock offers Retry Without Fetch

An update refused after its fetch by an error that the same request may pass (`retry: same`, a held lock) MUST offer
"Retry Without Fetch" on its toast: the check and the apply against the baseline that fetch got (the intent
`updateProject` with `fetch: false`, no second fetch), which a second retry joins, and only while the main window shows
the repository.

#### Scenario: A held lock after the fetch
- **WHEN** an update is refused after its fetch by a held lock (`retry: same`)
- **THEN** its toast offers "Retry Without Fetch", which checks and applies without a second fetch (`updateProject` with `fetch: false`)
- **AND** a second retry joins it
  [@test] src/stores/update-project.test.ts (`offers Retry Without Fetch for a held lock after the fetch, which checks and applies without a second fetch; a moved branch gets none`)
  [@test] tests/e2e/update-project/update-project.spec.ts (`should name the holder of the repository lock and retry the update without a fetch`)

#### Scenario: Another repository in the main window
- **WHEN** the main window shows another repository
- **THEN** the retry does not run
  [@test] src/stores/update-project.test.ts (`retries without a fetch only the repository the main window shows`)

### Requirement: Update: A retry refused by the same holder updates its toast

The retry refused by the same holder MUST update that toast in place (an error toast is one per title and backend
message, which says no time).

#### Scenario: The same holder refuses the retry
- **WHEN** the retry is refused by the same holder
- **THEN** that toast is updated in place, not shown a second time

### Requirement: Update: An error to read again offers no retry

An error to read again (`retry: after_reread`, the branch moved) MUST offer no retry: a new Update reads it again.

#### Scenario: The branch moved under the update
- **WHEN** the update is refused because the branch moved (`retry: after_reread`)
- **THEN** its toast offers no retry, and a new Update reads it again
  [@test] src/stores/update-project.test.ts (`offers Retry Without Fetch for a held lock after the fetch, which checks and applies without a second fetch; a moved branch gets none`)
  [@test] tests/e2e/update-project/update-project.spec.ts (`should offer no retry when the branch moved under the update`)

### Requirement: Update: `brd update --no-fetch` updates onto the last fetched baseline

`brd update --no-fetch` MUST update onto the last fetched baseline ([brd update](../../cli/update/spec.md)).

#### Scenario: An update without a fetch
- **WHEN** `brd update --no-fetch` runs
- **THEN** it updates onto the last fetched baseline, without a fetch
  [@test] crates/update-core/src/update_project_test.rs (`update_without_a_fetch_goes_onto_the_last_fetched_state`)

### Requirement: Update: A fast-forward moves the branch and updates the files

A fast-forward MUST move the branch and update the files.

#### Scenario: A branch without local commits
- **WHEN** the update is a fast-forward
- **THEN** the branch moves and the files are updated
  [@test] crates/update-core/src/update_project_test.rs (`fast_forward_updates_the_working_tree`)

### Requirement: Update: A rebase keeps the authors and the current user commits

A rebase MUST keep the author of every local commit; the committer SHALL be the current user.

#### Scenario: The local commits of a rebase
- **WHEN** the update rebases the local commits
- **THEN** each keeps its author, and the committer is the current user
  [@test] crates/update-core/src/update_project_test.rs (`rebase_keeps_the_author_and_makes_the_current_user_the_committer`)

### Requirement: Update: Changes on paths the update doesn't change survive byte for byte

Uncommitted, staged and untracked changes on paths the update doesn't change MUST survive byte for byte, the staged
version staged.

#### Scenario: Local work on other paths
- **GIVEN** uncommitted, staged and untracked changes on paths the update doesn't change
- **WHEN** the update applies
- **THEN** they survive byte for byte, the staged version staged
  [@test] crates/update-core/src/update_project_test.rs (`dirty_and_staged_files_on_untouched_paths_survive_byte_identical`)

### Requirement: Update: A rebase resolved by words keeps its commits and writes their notes

A rebase that resolved conflicts of git by words MUST keep the author and the message of each resolved commit, and
write a note per resolved commit on `refs/notes/branch-deck/resolve`. Undo SHALL take it back as any update.

#### Scenario: An update whose every conflict resolves by words
- **WHEN** an update's every conflict of git resolves by words
- **THEN** it applies without a session, each resolved commit keeps its author and its message
- **AND** a note per resolved commit is written on `refs/notes/branch-deck/resolve`
  [@test] crates/git-ops/src/update_resolve_test.rs (`an_update_whose_every_conflict_resolves_by_words_applies_without_a_session`)

#### Scenario: Undo of an update resolved by words
- **WHEN** the user undoes an update that resolved conflicts by words
- **THEN** Undo takes it back as any update

### Requirement: Update: A rebase resolved by words says so

The toast and the Activity entry of a rebase that resolved conflicts of git by words MUST end with "Resolved 1 conflict
by words in “<subject>”" (or "… in 2 commits"); the tooltip and the footer of the check SHALL say "Rebases onto
origin/master, resolving 1 conflict by words".

#### Scenario: The toast and the Activity entry
- **WHEN** an update resolved conflicts of git by words
- **THEN** its toast and its Activity entry end with "Resolved 1 conflict by words in “<subject>”", or "… in 2 commits"
  [@test] src/lib/landed.test.ts (`ends with the conflicts of git that the update resolved by words`)
  [@test] tests/e2e/update-project/update-project.spec.ts (`should rebase an update whose conflict of git resolves by words, and say so`)

#### Scenario: The tooltip and the footer of the check
- **WHEN** the check is a rebase that resolves conflicts by words
- **THEN** the tooltip and the footer say "Rebases onto origin/master, resolving 1 conflict by words": it is not clean to git
  [@test] src/lib/update-state.test.ts (`says that a rebase resolves conflicts of git by words: it is not clean to git`)

### Requirement: Update: A local commit already upstream is dropped and reported

A local commit whose changes are already upstream MUST be dropped and reported, not an error.

#### Scenario: A commit already upstream
- **GIVEN** the changes of a local commit are already upstream
- **WHEN** the update rebases
- **THEN** that commit is dropped and reported, not an error
  [@test] crates/update-core/src/update_project_test.rs (`commit_already_upstream_is_dropped_and_reported`)

### Requirement: Update: A twin the baseline has is left out before the replay

A twin the baseline proves it has, or the same patch of its author under another key, MUST be left out before the
replay, so a clean merge never adds its change a second time.

#### Scenario: A twin of the same patch
- **GIVEN** the baseline proves it has a twin of a local commit
- **WHEN** the update replays
- **THEN** that commit is left out before the replay, so a clean replay does not add its lines again
  [@test] crates/update-core/src/landed_test.rs (`a_twin_of_the_same_patch_is_left_out_of_a_clean_replay_that_would_add_its_lines_again`)

#### Scenario: The same patch under another key
- **GIVEN** the baseline has the same patch of its author under another key
- **WHEN** the update replays
- **THEN** that commit is left out before the replay
  [@test] crates/update-core/src/landed_test.rs (`the_same_patch_under_another_key_is_left_out_of_a_clean_replay_that_would_add_its_lines_again`)

### Requirement: Update: Landed commits a robot folded are left out of a conflicting replay

When the replay conflicts because a robot folded fixups into their target, the landed commits MUST be left out if
nothing local is lost ([Landed Commits](../landed-commits/spec.md)).

#### Scenario: A robot folded fixups into their target
- **GIVEN** a robot folded local fixups into their target
- **WHEN** the replay conflicts because of it
- **THEN** the landed commits are left out if nothing local is lost

### Requirement: Update: Conflict and blocked change nothing

Conflict and blocked are results: the branch, the index, the working tree and the reflog MUST stay as they were.

#### Scenario: An update that conflicts or is blocked
- **WHEN** the update's check is conflict or blocked
- **THEN** the branch, the index, the working tree and the reflog stay as they were

### Requirement: Update: Up to date writes no reflog entry

Up to date MUST write no reflog entry.

#### Scenario: An update with nothing to do
- **WHEN** the update is up to date
- **THEN** no reflog entry is written
  [@test] crates/update-core/src/update_project_test.rs (`up_to_date_writes_no_reflog_entry`)

### Requirement: Update: The branch moves with a CAS ref transaction before the files

The branch MUST move with a CAS ref transaction (`branch-deck: update project`) before the files; if the files can't
follow (one changed since the check), the branch moves back (`branch-deck: update project rolled back`) and the update
reports that nothing was changed.

#### Scenario: A file changed between the check and the update
- **WHEN** a file changed since the check, so the files can't follow
- **THEN** the branch moves back (`branch-deck: update project rolled back`)
- **AND** the update reports that nothing was changed
  [@test] crates/update-core/src/update_project_test.rs (`rolls_back_when_a_file_changes_between_the_check_and_the_update`)

### Requirement: Update: The transaction holds the lock of HEAD

The transaction MUST update HEAD and commit only while git holds the lock of HEAD, after a check that HEAD is still on
the checked branch.

#### Scenario: The move of an update
- **WHEN** the move commits
- **THEN** git holds the lock of HEAD, and HEAD was checked to be still on the checked branch

### Requirement: Update: The index lock of another process makes the move wait

The index lock of another process MUST make the move wait (at most 30 s per write) and an ownerless one be removed,
as for `brd pick`; one that stays MUST move the branch back and answer `locked`, nothing changed.

#### Scenario: An index lock no process owns
- **GIVEN** an index lock that no process owns
- **WHEN** the move runs
- **THEN** the lock is removed and the files follow
  [@test] crates/git-ops/src/pick_test.rs (`a_move_removes_an_index_lock_that_no_process_owns_and_lets_the_files_follow`)

#### Scenario: An index lock of another process
- **GIVEN** another process holds the index lock
- **WHEN** the move writes the index
- **THEN** it waits, at most 30 s per write

#### Scenario: An index lock that stays
- **GIVEN** the index lock stays for the whole wait
- **WHEN** the move runs
- **THEN** the branch moves back and the update answers `locked`, nothing changed

### Requirement: Update: The wait for the index lock comes before the repository lock

The wait for the index lock MUST come before the repository lock, so no other writer waits for it; a lock that appears
while the update takes the repository lock MUST release it and wait outside it.

#### Scenario: A move whose files follow
- **GIVEN** another process holds the index lock
- **WHEN** the update moves the branch and its files follow
- **THEN** it waits for the index lock before the repository lock
- **AND** an index lock that appears while it takes the repository lock makes it release the repository lock and wait outside it
  [@test] crates/history-rewrite/src/repository_lock_test.rs (`a_move_whose_files_follow_waits_for_the_index_lock_before_the_repository_lock`)

### Requirement: Update: HEAD switched since the check refuses the move

HEAD switched to another branch since the check, even one at the same commit, and also right before the move, MUST
refuse the move (and its undo) with nothing changed.

#### Scenario: A switch to another branch at the same commit
- **WHEN** HEAD was switched since the check to another branch at the same commit
- **THEN** the move is refused with nothing changed
  [@test] crates/update-core/src/update_project_test.rs (`a_switch_to_another_branch_at_the_same_commit_since_the_check_is_refused`)

#### Scenario: A switch to a branch at another commit
- **WHEN** HEAD was switched since the check to a branch at another commit, or from a detached HEAD to one
- **THEN** the move is refused with nothing changed
  [@test] crates/update-core/src/update_project_test.rs (`a_switch_to_a_branch_at_another_commit_since_the_check_is_refused`, `a_detached_head_switched_to_a_branch_at_another_commit_since_the_check_is_refused`)

#### Scenario: A switch right before the move
- **WHEN** HEAD is switched between the check and the move
- **THEN** no branch moves
  [@test] crates/bd-update/src/update_test.rs (`a_switch_between_the_check_and_the_move_moves_no_branch`)

#### Scenario: The undo after a switch
- **WHEN** HEAD was switched to another branch since the update
- **THEN** its undo is refused with nothing changed

### Requirement: Update: A `git switch` during the move fails on the lock of HEAD

A `git switch` while the move holds the lock of HEAD MUST fail on `HEAD.lock`; the move SHALL commit on the checked
branch.

#### Scenario: A switch during the move
- **WHEN** a `git switch` runs while the move holds the lock of HEAD
- **THEN** it fails on `HEAD.lock`, and the move commits on the checked branch
  [@test] crates/bd-update/src/update_test.rs (`a_switch_during_the_move_fails_on_the_head_lock`)

### Requirement: Update: The working tree is written by `worktree_update.rs` only

The working tree MUST be written by `crates/git-ops/src/worktree_update.rs` only, called only from `update_branch.rs`
(the update, the undo that only `check_update_undo` builds, and the finish of a resolve session, whose tip
`update_resolve.rs` reads from the session ref); the safety scan MUST fail on any other `read-tree -u`, `--reset` or
`checkout-index`, and on a change of the number of exempt argument lists.

#### Scenario: The production code
- **WHEN** the safety scan reads the production code
- **THEN** no code but `worktree_update.rs` writes the working tree, and the number of exempt argument lists is as it was
  [@test] crates/git-executor/src/worktree_safety_test.rs (`production_code_never_writes_the_working_tree`)

#### Scenario: A `read-tree -u` elsewhere
- **WHEN** other code runs `read-tree -u`
- **THEN** the scanner finds it
  [@test] crates/git-executor/src/worktree_safety_test.rs (`scanner_finds_read_tree_updates`)

### Requirement: Undo: The undo moves the branch and the files back

`brd undo update:<previous head>..<new head>` MUST move the branch back and let the files follow through the same
guarded two-way update with the trees swapped (`branch-deck: undo update`); local changes on the other paths SHALL
stay.

#### Scenario: The undo of an update
- **WHEN** `brd undo update:<previous head>..<new head>` runs
- **THEN** the branch moves back (`branch-deck: undo update`) and the files follow through the guarded two-way update with the trees swapped
- **AND** local changes on the other paths stay
  [@test] crates/update-core/src/update_project_test.rs (`undo_moves_the_branch_and_the_files_back`)

### Requirement: Undo: The check of the undo runs before the repository lock

The check of the undo (the guards, the reflog, the dry run of the files) MUST run before the repository lock, which
holds only the witness of HEAD and of the mark of an interrupted move, and the move back; the landed report SHALL come
after it.

#### Scenario: What the undo runs under the lock
- **WHEN** the undo runs
- **THEN** its guards, the reflog and the dry run of the files run before the lock
- **AND** the lock holds only the witness of HEAD and of the mark, and the move back, and the landed report comes after it
  [@test] crates/update-core/src/update_project_test.rs (`the_undo_of_an_update_locks_only_the_witness_and_the_move`)

### Requirement: Undo: The undo refuses local changes in its way and a moved branch

The undo MUST refuse, changing nothing, when local changes sit on a path it writes, an ignored file where a file that
the update deleted comes back included, and when the branch moved since the update.

#### Scenario: Local changes on a path the undo writes
- **GIVEN** local changes on a path the undo writes, or an ignored file where a file that the update deleted comes back
- **WHEN** the undo runs
- **THEN** it is refused, and nothing changes
  [@test] crates/update-core/src/update_project_test.rs (`undo_is_refused_when_local_changes_overlap`)

#### Scenario: The branch moved since the update
- **GIVEN** the branch moved since the update
- **WHEN** the undo runs
- **THEN** it is refused, and nothing changes
  [@test] crates/update-core/src/update_project_test.rs (`undo_is_refused_after_the_branch_moved`)

### Requirement: Undo: The undo recovers an update killed before the files followed

After a process was killed between the move and the files (HEAD at the new head, the files at the previous one), the
undo MUST succeed and be the recovery.

#### Scenario: An update killed between the move and the files
- **GIVEN** HEAD is at the new head and the files at the previous one, as the process was killed between them
- **WHEN** the undo runs
- **THEN** it succeeds and is the recovery
  [@test] crates/update-core/src/update_project_test.rs (`the_undo_recovers_an_update_killed_before_the_files_followed`)

### Requirement: Undo: The move marks an interrupted update until the files followed

The move MUST create the mark `refs/branch-deck/update-pending/<worktree id>` in its ref transaction, and delete it
once the files followed.

#### Scenario: A move whose files followed
- **WHEN** the move runs
- **THEN** its ref transaction creates `refs/branch-deck/update-pending/<worktree id>`
- **AND** the mark is deleted once the files followed

### Requirement: Undo: The next update or undo finishes the files of a killed move

With the mark left by a killed process, the next update or undo, the app's included, MUST let the files follow first,
under the lock, or change nothing with `finish_pending` when the branch moved since; an undo whose own run was killed
finds itself done and lets the files follow before it answers `already_undone`.

#### Scenario: An update killed before its files followed
- **GIVEN** the mark of an update killed before its files followed
- **WHEN** the next update runs
- **THEN** it lets the files follow first, under the lock
  [@test] crates/bd-update/src/update_test.rs (`an_update_killed_before_its_files_followed_is_finished_by_the_next_update`)

#### Scenario: An undo killed before its files followed
- **GIVEN** the mark of an undo killed before its files followed
- **WHEN** the next undo runs
- **THEN** it finds itself done, lets the files follow and answers `already_undone`
  [@test] crates/bd-update/src/update_test.rs (`an_undo_killed_before_its_files_followed_is_finished_by_the_next_undo`)

#### Scenario: The branch moved since the interrupted update
- **GIVEN** the mark of an interrupted update, and the branch moved since
- **WHEN** the next update or undo runs
- **THEN** it changes nothing, with `finish_pending`
  [@test] crates/bd-update/src/update_test.rs (`an_interrupted_update_of_a_branch_that_moved_since_is_reported`)

### Requirement: Undo: The outcome of the app's update carries the undo token

The outcome of the app's update MUST carry the same token (`Outcome.undo`, `UndoKind::Update`), whose undo the engine
runs with the core's short lock (`Rewrite(Undo)`).

#### Scenario: The app's update applies
- **WHEN** the app's update fetches, checks and moves
- **THEN** its outcome carries the token (`Outcome.undo`, `UndoKind::Update`)
- **AND** the engine runs its undo with the core's short lock (`Rewrite(Undo)`)
  [@test] crates/operations/src/update_test.rs (`an_update_fetches_checks_and_moves_then_asks_its_sync_with_its_undo`)

### Requirement: Undo: An undo of an undone update changes nothing

An undo of an undone update MUST be no error and change nothing (`already_undone`).

#### Scenario: The same undo twice
- **WHEN** the undo of an update that is undone runs again
- **THEN** it is no error and changes nothing (`already_undone`)
  [@test] crates/update-core/src/update_project_test.rs (`undo_moves_the_branch_and_the_files_back`)

### Requirement: Undo: Only the last update of the branch can be undone

Only the last update of the branch SHALL be undoable: the last reflog entry MUST be that update, so a token can't move
the branch to another commit. A token MUST take full ids only.

#### Scenario: A token of an earlier update
- **WHEN** the last reflog entry is not the update the token names
- **THEN** the undo is refused
  [@test] crates/update-core/src/update_project_test.rs (`undo_needs_the_token_of_the_last_update`)

#### Scenario: A token with short ids
- **WHEN** a token names a commit by a short id
- **THEN** it is refused
  [@test] crates/update-core/src/update_project_test.rs (`undo_tokens_are_strict`)

### Requirement: Undo: The update writes its reflog entry without reflogs too

The update MUST write that reflog entry also with `core.logAllRefUpdates=false`.

#### Scenario: Reflogs off
- **GIVEN** `core.logAllRefUpdates=false`
- **WHEN** an update applies
- **THEN** it writes its reflog entry, and the undo works
  [@test] crates/update-core/src/update_project_test.rs (`the_undo_works_without_reflogs_because_the_update_writes_one`)

### Requirement: Undo: The landed report is the one from before the update again

After the undo, the landed report MUST be the one from before the update again
([Landed Commits](../landed-commits/spec.md)).

#### Scenario: The undo of an update that landed commits
- **WHEN** an update is undone
- **THEN** the landed report is the one from before it
  [@test] crates/update-core/src/landed_test.rs (`undo_of_an_update_restores_the_report_from_before_it`)

### Requirement: Header: The Update button sits before the landing indicator

The Update button SHALL sit in the header before the landing indicator: incoming and the way into master together, the
time of the last sync next to Sync. It MUST be hidden for a local baseline and before the first sync of the
repository, which runs when the window opens a repository synced before ([Onboarding](../onboarding/spec.md)).

#### Scenario: A repository with a remote baseline, synced
- **WHEN** the header shows a repository whose baseline is remote, after its first sync
- **THEN** the Update button sits before the landing indicator, and the time of the last sync next to Sync

#### Scenario: A local baseline, or before the first sync
- **WHEN** the baseline is local, or the repository has not had its first sync yet
- **THEN** the Update button is hidden

### Requirement: Header: The header stays one line down to the minimum window width

The header MUST stay one line down to the minimum window width, 960 px (`min_inner_size`), its controls never
overlapping: the repository selector gives way first; the branch count shows while the filter bar is hidden, from
448 px (`@md`) of the room of the repository; the time of the last sync from 768 px (`@3xl`) of the header; Update,
the landing indicator, Activity ([Activity](../activity/spec.md), there with an empty log too), Sync and its settings
never shrink.

#### Scenario: The minimum window width
- **GIVEN** the window is 960 px wide (`min_inner_size`)
- **WHEN** the header is shown
- **THEN** it stays one line, its controls never overlapping, and the repository selector gives way first
- **AND** Update, the landing indicator, Activity (there with an empty log too), Sync and its settings never shrink
  [@test] tests/e2e/repository/header.spec.ts (`should keep its controls apart on one line at the minimum window width`)

#### Scenario: The branch count
- **GIVEN** the filter bar is hidden: below 8 branches, as from 8 branches on its "All 35" counts them
- **WHEN** the room of the repository (the selector, the views and the count) is 448 px (`@md`) or more
- **THEN** the header shows the branch count

#### Scenario: The time of the last sync
- **WHEN** the header is 768 px (`@3xl`) wide or more
- **THEN** it shows the time of the last sync

### Requirement: Header: The selector cuts its path only at a separator

The selector MUST show its path cut at a separator, never in a segment: the whole path where it fits, else as many of
its last segments as fit behind one "…" (`…/scratch/demo`, then `…/demo`), at last the name alone, whose end a name
wider than the room cuts (`pathForms`). A cut path SHALL be whole in the tooltip of the trigger, its lines broken after
a separator.

#### Scenario: The forms of a path
- **WHEN** the forms of a path are computed (`pathForms`)
- **THEN** they drop whole leading segments behind one "…" (`…/scratch/demo`, then `…/demo`), the name alone last
- **AND** a path without a segment to drop stays whole, and no form takes a trailing separator
  [@test] src/lib/home-path.test.ts (`cuts a path only at its separators, dropping whole leading segments behind one …, the name alone last`, `keeps a path without a segment to drop whole, and drops no trailing separator into a form`)

#### Scenario: The room of the selector changes
- **WHEN** the room of the selector grows or shrinks
- **THEN** each form is measured in the font of the trigger, and the one shown is picked on every resize of its room, more of the path as the room grows
- **AND** a name wider than the room is cut at its end
  [@test] tests/e2e/repository/header.spec.ts (`should cut the path at its separators behind one …, more of it as the room grows`)

#### Scenario: The tooltip of a cut path
- **WHEN** the path is shown cut
- **THEN** the tooltip of the trigger has it whole, its lines broken after a separator

### Requirement: Header: The time of the last sync keeps its width as it ages

The time of the last sync MUST take the room of its widest text, "Last synced 00 minutes ago" in digits of one width,
so its aging ("just now", "1 minute ago", "Checked just now") moves nothing: the selector, which gives way to it at the
minimum width beside the traffic lights, keeps its width as the time ages.

#### Scenario: The time ages
- **WHEN** the time of the last sync ages, "just now", "1 minute ago", "Checked just now"
- **THEN** it keeps the room of "Last synced 00 minutes ago", and the selector keeps its width, the whole path in its tooltip
  [@test] tests/e2e/repository/header.spec.ts (`should keep its width while the time of the last sync ages, the whole path in its tooltip`)

### Requirement: Header: While a sync runs, Sync shows its spinner and nothing moves

While a sync runs, Sync MUST show its spinner, the one indicator of the sync, and its tooltip names the step,
"Syncing branches…", without the shortcut, which does nothing then; the time of the last sync SHALL stay as it is, so
nothing in the header moves. Before the first sync there is no time.

#### Scenario: A sync runs
- **WHEN** a sync runs
- **THEN** Sync shows its spinner and its tooltip "Syncing branches…", without the shortcut
- **AND** the time of the last sync stays as it is, and nothing in the header moves
  [@test] tests/e2e/repository/header.spec.ts (`should not move while a sync runs`)

#### Scenario: Before the first sync
- **WHEN** the repository has not been synced yet
- **THEN** the header shows no time of the last sync

### Requirement: Header: One sync runs at a time in the session of a repository

One sync MUST run at a time in the session of a repository, whichever window or command asks for it. A resync after a
change, its Undo or a fetch asked while one runs SHALL share one quiet sync after it, which reads what changed
meanwhile.

#### Scenario: Requests during a run
- **GIVEN** a sync runs
- **WHEN** a resync after a change, its Undo or a fetch is asked
- **THEN** they share one quiet sync after the run, which reads what changed meanwhile
  [@test] crates/repository-model/src/sync_scheduler_test.rs (`a_ticket_resolves_when_the_run_that_serves_it_ends`)

### Requirement: Header: A focus or ⌘R during a run checks once after it

A focus or ⌘R while a sync runs MUST check once after it and sync only if what a sync reads changed.

#### Scenario: A focus during a run
- **GIVEN** a sync runs
- **WHEN** the window gets the focus, or the user presses ⌘R
- **THEN** the inputs are checked once after the run, and a sync runs only if what a sync reads changed
  [@test] crates/repository-model/src/session_test.rs (`a_focus_during_a_run_is_one_check_after_it`)
  [@test] crates/repository-model/src/sync_scheduler_test.rs (`a_focus_during_a_run_checks_the_inputs_once_after_the_chain`)

### Requirement: Header: A request resolves at the end of the run that serves it

A request MUST resolve at the end of the run that serves it, never of the follow-ups after it.

#### Scenario: A follow-up after the run
- **GIVEN** a request is served by a run, and a follow-up is queued after it
- **WHEN** the run ends
- **THEN** the request resolves, without waiting for the follow-up
  [@test] crates/repository-model/src/session_test.rs (`a_ticket_resolves_when_its_run_ends_not_the_chain`)
  [@test] crates/repository-model/src/sync_scheduler_test.rs (`a_ticket_resolves_when_the_run_that_serves_it_ends`)

### Requirement: Header: Sync keeps its spinner until the last patch of its run

Sync MUST keep its spinner from the request until the last patch of the run that serves it reached the window, and a
push pressed meanwhile SHALL wait for it.

#### Scenario: The run's last patch reaches the window
- **WHEN** a sync is requested
- **THEN** Sync is busy from the request until the last patch of the run that serves it reached the window
  [@test] src/model/store.test.ts (`keeps Sync busy from the request to the last patch of the run that serves it, and counts the runs that ended`)

#### Scenario: A push pressed meanwhile
- **WHEN** a push is pressed while Sync keeps its spinner
- **THEN** the push waits for that run

### Requirement: Header: The events of an ended run are dropped

The events of a run that ended MUST be dropped, and a sync MUST record its time on its own repository.

#### Scenario: Late events of an ended run
- **WHEN** an event of a run that ended comes late
- **THEN** it is dropped
  [@test] crates/repository-model/src/session_test.rs (`the_late_events_of_an_ended_run_are_dropped`)

#### Scenario: The window left the repository
- **GIVEN** the window left the repository of a request to its session
- **WHEN** that sync ends
- **THEN** Sync of the next repository is not busy, and nothing of it is recorded there
  [@test] src/model/store.test.ts (`leaves a request of the session the window left to it: Sync of the next one is not busy, nothing of it is recorded`)

### Requirement: Header: Sync names its shortcut, and the menu item and ⌘R do what it does

Sync MUST have the tooltip "Sync branches ⌘R" (`Ctrl+R` off macOS) and its settings (auto-sync on focus) next to it;
the menu item and ⌘R SHALL do what the button does, nothing while it is disabled for a reason.

#### Scenario: The tooltip of Sync
- **WHEN** the user hovers Sync
- **THEN** its tooltip is "Sync branches ⌘R" (`Ctrl+R` off macOS), and its settings (auto-sync on focus) are next to it
  [@test] tests/e2e/repository/header.spec.ts (`should name the shortcut of Sync and keep the folder button of the selected repository without a tooltip`)

#### Scenario: The menu item and ⌘R
- **WHEN** the user chooses the menu item or presses ⌘R
- **THEN** it does what the button does, and nothing while the button is disabled for a reason

### Requirement: Header: A failed sync turns Sync `destructive`

A failed sync MUST turn Sync `destructive` with the error in its tooltip; with the page alert, the error SHALL show
twice at most.

#### Scenario: A sync fails
- **WHEN** a sync fails
- **THEN** Sync turns `destructive` with the error in its tooltip
- **AND** with the page alert, the error shows twice at most

### Requirement: Header: The selector has a tooltip only where it says something

The repository selector MUST have a tooltip only without a repository ("Select a Git repository to manage branches"),
for its error, and on its trigger for a path it shows cut; its folder button MUST have none.

#### Scenario: No repository
- **WHEN** no repository is selected
- **THEN** the selector's tooltip is "Select a Git repository to manage branches"

#### Scenario: A selected repository
- **WHEN** a repository is selected
- **THEN** the folder button has no tooltip, and the trigger has one only for its error or a path it shows cut
  [@test] tests/e2e/repository/header.spec.ts (`should name the shortcut of Sync and keep the folder button of the selected repository without a tooltip`)

### Requirement: Header: The Update button shows the state, never a count

The Update button MUST show the state, never a count: up to date, clean, conflict, blocked or a failed check, each
with its icon and its tooltip.

#### Scenario: Up to date
- **WHEN** the state is up to date
- **THEN** the button is muted, without a state icon, its tooltip "Up to date with origin/master"
  [@test] tests/e2e/update-project/update-project.spec.ts (`should be muted when up to date`)
  [@test] src/lib/update-state.test.ts (`names the state, never a count of upstream commits`)

#### Scenario: Clean
- **WHEN** the state is clean
- **THEN** the button shows a `success` check, its tooltip "Rebases cleanly onto origin/master" ("Fast-forwards to origin/master")
  [@test] src/lib/update-state.test.ts (`names the state, never a count of upstream commits`)

#### Scenario: Conflict
- **WHEN** the state is conflict
- **THEN** the button shows a `warning` icon, its tooltip "Conflicts in N files: …" (at most three names)
  [@test] src/lib/update-state.test.ts (`names the state, never a count of upstream commits`, `names at most three files, four of four: never 'and 1 more'`)

#### Scenario: Blocked
- **WHEN** the state is blocked
- **THEN** the button shows a `warning` icon, its tooltip "Your uncommitted changes in … overlap origin/master"
  [@test] src/lib/update-state.test.ts (`names the state, never a count of upstream commits`)

#### Scenario: A failed check
- **WHEN** the check failed
- **THEN** the button is muted with a question mark, its tooltip "Cannot check the update: <reason>"
  [@test] src/lib/update-state.test.ts (`names the state, never a count of upstream commits`)

### Requirement: Header: The tooltip says when the baseline was last fetched

The tooltip MUST say when the baseline was last fetched: Branch Deck's own fetch or one elsewhere (`FETCH_HEAD`),
whichever is newer, and the shortcut: "Fetched 5 minutes ago · fetch and update ⌘T".

#### Scenario: A fetch elsewhere
- **GIVEN** Branch Deck fetched the baseline, and a fetch elsewhere wrote `FETCH_HEAD`
- **WHEN** the user hovers the Update button
- **THEN** its tooltip names the newer of the two and the shortcut, "Fetched 5 minutes ago · fetch and update ⌘T"
  [@test] src/lib/update-state.test.ts (`takes the newer of Branch Deck's fetch and one elsewhere`)

### Requirement: Header: Before the first fetch the button claims no state

Before the first fetch, a state would claim what nobody asked the remote: the button MUST be muted without a state
icon, its tooltip "origin/master not fetched yet" over "fetch and update ⌘T", and the footer of the landing overview
"Incoming from origin/master: not fetched yet".

#### Scenario: The baseline was not fetched yet
- **GIVEN** the baseline was never fetched
- **WHEN** the header shows the Update button
- **THEN** the button is muted without a state icon, its tooltip "origin/master not fetched yet" over "fetch and update ⌘T"
  [@test] src/lib/update-state.test.ts (`says the baseline was not fetched yet instead of a state nobody asked the remote for`)
  [@test] tests/e2e/update-project/update-project.spec.ts (`should be muted when up to date`)

#### Scenario: The footer of the landing overview
- **GIVEN** the baseline was never fetched
- **WHEN** the landing overview is shown
- **THEN** its footer says "Incoming from origin/master: not fetched yet"
  [@test] src/lib/update-state.test.ts (`says the same in the footer of the landing overview`)

### Requirement: Header: A conflict found before the first fetch keeps its words

A conflict or a blocked update, found against the copy of the clone before the first fetch, MUST keep its words and
its icon, then "· origin/master not fetched yet".

#### Scenario: A conflict against the copy of the clone
- **GIVEN** the baseline was never fetched
- **WHEN** the check finds a conflict or a blocked update against the copy of the clone
- **THEN** the button keeps its words and its icon, then "· origin/master not fetched yet"
  [@test] src/lib/update-state.test.ts (`says the baseline was not fetched yet instead of a state nobody asked the remote for`)

### Requirement: Header: A relative time ages while it is shown

A relative time MUST age while it is shown, without a sync or a fetch: "Last synced 1 minute ago" next to the button,
the fetch time of the tooltip and of the footer line, "Pull requests checked" and "Patronus checked" in the overview,
the times of the Landed island and chips, and of the commits a force push drops.

#### Scenario: The last sync next to the Update button
- **GIVEN** nothing syncs or fetches
- **WHEN** a minute passes while the header is shown
- **THEN** "Last synced just now" becomes "Last synced 1 minute ago"
  [@test] src/hooks/useNow.test.ts (`ages a relative time without anything else changing: "Last synced just now" becomes "1 minute ago"`)
  [@test] tests/e2e/update-project/update-project.spec.ts (`should age the last sync next to the Update button while it is shown`)

#### Scenario: The other relative times
- **WHEN** time passes while they are shown
- **THEN** the fetch time of the tooltip and of the footer line, "Pull requests checked" and "Patronus checked" in the overview, the times of the Landed island and chips, and of the commits a force push drops age too

### Requirement: Header: One interval per interval length renews the relative times

One interval per interval length MUST renew the relative times every 15 s (`useNow`), so "1 minute ago" is at most a
tick late; only the text SHALL re-render.

#### Scenario: Several listeners of one interval length
- **GIVEN** several relative times of one interval length are shown
- **WHEN** the interval ticks
- **THEN** every listener ticks at once, with one interval that stops with the last listener
  [@test] src/hooks/useNow.test.ts (`ticks every listener of an interval length at once, with one interval that stops with the last listener`)

#### Scenario: Only the text re-renders
- **WHEN** a relative time renews
- **THEN** only its text re-renders, at most a tick late

### Requirement: Header: A click or ⌘T runs the update

A click, or File → Update Project (`⌘T`), MUST run the update.

#### Scenario: The Update Project menu item
- **WHEN** the user chooses File → Update Project, presses `⌘T` or clicks the button
- **THEN** the update runs
  [@test] tests/e2e/update-project/update-project.spec.ts (`should run from the Update Project menu item`)

### Requirement: Header: The steps of an update are one progress from the click to its end

The steps of the update MUST be one progress from the click to its end: the fetch, the check, the wait for the
repository lock (only while another operation holds it and the check found something to apply), and, for an update
that applies, the update of the files and the sync that rebuilds the rebased branches.

#### Scenario: An update that applies
- **WHEN** an update that applies runs
- **THEN** one progress spans the fetch, the check, the update of the files and the sync that rebuilds the rebased branches, its steps in the tooltip
  [@test] tests/e2e/update-project/update-project.spec.ts (`should show one progress from the click to the end of its sync, its steps in the tooltip`)

#### Scenario: The wait for the repository lock
- **WHEN** another operation holds the repository lock and the check found something to apply
- **THEN** the wait for the lock is a step of that progress, and only then

### Requirement: Header: The button shows one word and the percent of the whole run

Meanwhile the button MUST show a spinner and one word for the whole run with its percent, "Updating 62%" ("Updating…"
below 1 %), never a word per step; the Fetch of a merge toast says "Fetching 30%". The label MUST keep the width of
the widest one, "Updating 99%", from the start of the run to its end, so the header doesn't move per step or tick.

#### Scenario: The label of a run
- **WHEN** the update runs
- **THEN** the button shows a spinner and "Updating 62%", or "Updating…" below 1 %, never a word per step
- **AND** the Fetch of a merge toast says "Fetching 30%"
  [@test] src/lib/update-state.test.ts (`names the whole run with one word and its percent, the toast's Fetch as a fetch`)

#### Scenario: The header does not move
- **WHEN** the steps and the percent of a run change
- **THEN** the label keeps the width of "Updating 99%" from the start of the run to its end, and the controls stay apart
  [@test] tests/e2e/repository/header.spec.ts (`should keep its controls apart while an update runs`)

### Requirement: Header: The update waits for a live holder of the repository lock

While another process holds the repository lock (an agent's `brd pick --follow`, `brd`) or the app's own push, the
update MUST wait after its fetch and its check as long as the holder lives, never refused after 10 s: its tooltip
step says whom for ("Waiting for `brd pick` (pid 4242, for 12 s)…").

#### Scenario: Another holder has the repository lock
- **GIVEN** another process or the app's own push holds the repository lock
- **WHEN** the update asks for the lock after its fetch and its check
- **THEN** it waits as long as the holder lives, and its tooltip step says "Waiting for `brd pick` (pid 4242, for 12 s)…", until its next step
  [@test] src/lib/update-state.test.ts (`names whom an update waits for while another holder has the repository lock, with Stop Waiting, until its next step`)
  [@test] src/stores/update-project.test.ts (`names whom it waits for while another holder has the repository lock, and Stop Waiting cancels its operation`)

### Requirement: Header: Stop Waiting ends the wait with nothing changed but the fetch

Stop Waiting beside the Update button MUST end the wait with nothing changed but the fetch ("Canceled the update
before it started"): `cancel_operation` of its operation, whose update is the failed effect of its outcome, `canceled`.

#### Scenario: The user stops the wait
- **GIVEN** the update waits for the repository lock
- **WHEN** the user clicks Stop Waiting beside the Update button
- **THEN** `cancel_operation` of its operation ends the wait, its update the failed effect of its outcome, `canceled`
- **AND** nothing changed but the fetch, "Canceled the update before it started"
  [@test] src/stores/update-project.test.ts (`names whom it waits for while another holder has the repository lock, and Stop Waiting cancels its operation`)
  [@test] crates/operations/src/update_test.rs (`stop_waiting_ends_the_wait_of_an_update_for_the_lock_with_nothing_moved`)
  [@test] tests/e2e/update-project/update-project.spec.ts (`should say whom an update waits for after its check, whose Stop Waiting changes nothing but the fetch`)
  [@test] src/lib/update-state.test.ts (`names whom an update waits for while another holder has the repository lock, with Stop Waiting, until its next step`)

### Requirement: Header: Only a holder no record names refuses the update after 10 s

Only a holder no record names MUST refuse the update after 10 s; `brd update` SHALL wait 10 s, then fail `locked`.

#### Scenario: A holder without a record
- **GIVEN** a holder that no record names holds the repository lock
- **WHEN** the app's update waits for it for 10 s
- **THEN** the update is refused

#### Scenario: `brd update` while the lock is held
- **GIVEN** another holds the repository lock
- **WHEN** `brd update` waits for it for 10 s
- **THEN** it fails `locked`

### Requirement: Header: A bar along the bottom edge of the button shows the progress

A bar of 2 px along the bottom edge of the button MUST show the progress, a sibling of the button (`progressbar`,
`aria-valuenow`, `aria-valuetext` with the step, "Checking the update onto origin/master… 62%"); while the run is on,
the name of the button SHALL be "Update Project: updating", without the percent.

#### Scenario: The bar of a running update
- **WHEN** the update runs
- **THEN** a bar of 2 px along the bottom edge of the button, its sibling (`progressbar`), has `aria-valuenow` and `aria-valuetext` with the step, "Checking the update onto origin/master… 62%"
- **AND** the name of the button is "Update Project: updating", without the percent

### Requirement: Header: The button works again at once when the run ends

Once the run ends, the button MUST work again at once and the bar SHALL show 100 % for 300 ms.

#### Scenario: The run ends
- **WHEN** the run of the update ends, through the sync of an applied update
- **THEN** the button works again at once, and the bar shows 100 % for 300 ms
  [@test] src/stores/update-project.test.ts (`is busy through the sync of an applied update, its own run of it; the toast doesn't wait for the sync`)

### Requirement: Header: The Dock shows the progress of the update

While the update runs in the main window, the Dock (the taskbar on Windows) MUST show the progress of the update
instead of the robots' ([Robots in the Dock](../robots-indicator/spec.md#dock)).

#### Scenario: An update in the main window
- **WHEN** an update runs in the main window
- **THEN** the Dock, the taskbar on Windows, shows its progress instead of the robots'

### Requirement: Header: The share of a step is its usual time in the repository

The share of each step MUST be its usual time in the repository: the median of its last 5 durations (fetch, check,
update of the files; the sync learns from every sync of the repository, the wait for the lock never), kept in the
state of the repository (`repositoryState`, beside the settings, never in `settings.json`), 3 s, 1.5 s, 0.8 s and 2 s
before the first.

#### Scenario: The median of the last five
- **WHEN** the bar is shared among the steps
- **THEN** each step's share is the median of its last 5 durations in the repository, and 3 s, 1.5 s, 0.8 s and 2 s before the first
  [@test] src/lib/update-progress.test.ts (`shares the bar by the usual time of each step in the repository: the median of the last five`)

#### Scenario: What the steps took is learnt
- **WHEN** an update ends
- **THEN** what its fetch, check and update of the files took is recorded in the state of the repository (`repositoryState`), never in `settings.json`
- **AND** the wait for the lock is not learnt, and the sync learns from every sync of the repository
  [@test] src/lib/update-progress.test.ts (`learns what the steps took, without the wait for the lock and the sync, which learns from every sync`)
  [@test] src/stores/update-project.test.ts (`changes the step only when the operation does, and records what the steps took in the state of the repository once the update ends`)

### Requirement: Header: Inside a step the progress follows git, the sync or the time

Inside a step the progress MUST be the fetch's percent of git, or the synced commits of the update's own sync run,
when it is ahead of the time; else the time, `1 − e^(−t/usual)`: 63 % of the step at its usual time, never still.

#### Scenario: git's percent in the fetch
- **WHEN** git's percent of the fetch is ahead of the time
- **THEN** the progress of the step takes it
  [@test] src/lib/update-progress.test.ts (`takes git's percent in the fetch when it is ahead of the time`)

#### Scenario: The commits of its own sync
- **WHEN** the update's own sync run reports its synced commits
- **THEN** the progress takes them, capped until that sync run ends
  [@test] src/lib/update-progress.test.ts (`takes the commits of its own sync, capped until the sync ends`)

#### Scenario: A step that reports nothing
- **WHEN** a step reports nothing
- **THEN** the progress moves with the time, `1 − e^(−t/usual)`, 63 % of the step at its usual time, never still
  [@test] src/lib/update-progress.test.ts (`moves with the time in a step that reports nothing, never standing still, and keeps the end of a step for its end`)

### Requirement: Header: The bar never jumps nor goes back

A step MUST reach at most 95 % of its share before it ends, and at each change of step the rest of the bar MUST be
shared again among the steps left, from where it stands: the bar never jumps nor goes back.

#### Scenario: The end of a step is kept for its end
- **WHEN** a step runs longer than its usual time
- **THEN** it reaches at most 95 % of its share before it ends
  [@test] src/lib/update-progress.test.ts (`moves with the time in a step that reports nothing, never standing still, and keeps the end of a step for its end`)

#### Scenario: A change of step
- **WHEN** the step changes after a slow or a fast step
- **THEN** the rest of the bar is shared again among the steps left from where it stands: no jump after a slow fetch, no stall after a fast one
  [@test] src/lib/update-progress.test.ts (`goes on from where it stands at each change of step: no jump after a slow fetch, no stall after a fast one`)

### Requirement: Header: The bar stands still while the update waits for the lock

While the update waits for the lock, the bar MUST stand still.

#### Scenario: A wait for the lock
- **WHEN** the update waits for the lock, or behind a rewrite
- **THEN** the bar stands still, the wait placed after the steps done: behind a rewrite before the check, for the lock before the apply
  [@test] src/lib/update-progress.test.ts (`stands still while it waits for the lock`)
  [@test] src/lib/update-progress.test.ts (`places a wait after the steps done: behind a rewrite before the check, for the lock before the apply`)

### Requirement: Header: The update shares the bar among all its steps from its start

The update MUST share the bar among all its steps from its start, also when the last check found the branch up to
date; a check that finds nothing to apply SHALL end the run with its result, and the bar shows 100 %.

#### Scenario: The last check found the branch up to date
- **GIVEN** the last check found the branch up to date
- **WHEN** an update starts
- **THEN** the bar is shared among every step of an update from its start
  [@test] src/stores/update-project.test.ts (`lists every step of an update from its start, also when the last check found the branch up to date`)

#### Scenario: Nothing to apply
- **WHEN** the check of the update finds nothing to apply
- **THEN** the run ends with its result, and the bar shows 100 %

### Requirement: Header: While the update runs, its tooltip lists its steps

While the update runs, its tooltip MUST list its steps instead of the state of the last check and "fetch and update":
those done ("Fetched origin/master · 5.79 MiB", "Checked the update onto origin/master"), the running one, and those
left, muted ("Check the update", "Update to origin/master", "Sync branches").

#### Scenario: The steps done, the running one and those left
- **WHEN** the user hovers the Update button while the update runs
- **THEN** the tooltip lists the steps done ("Fetched origin/master · 5.79 MiB", "Checked the update onto origin/master"), the running one and those left, muted ("Check the update", "Update to origin/master", "Sync branches"), never the last state
  [@test] src/lib/update-state.test.ts (`names the running step in the tooltip, never the last state`)
  [@test] src/lib/update-state.test.ts (`lists the steps done, the running one and those left, without a wait that ended, a step that ran again once`)
  [@test] tests/e2e/update-project/update-project.spec.ts (`should show one progress from the click to the end of its sync, its steps in the tooltip`)

### Requirement: Header: The running step says what the update does now

The running step MUST say what the update does now: "Fetching origin/master…", or git's last progress line once git
reports a stage, or whom the fetch waits for while another fetch of the baseline runs; "Waiting for Branch Deck or brd
to finish…", "Checking the update onto origin/master…", "Updating to origin/master…", "Syncing branches…".

#### Scenario: The running step
- **WHEN** the update fetches, waits, checks, updates or syncs
- **THEN** the running step is "Fetching origin/master…" or git's last progress line once git reports a stage, "Waiting for Branch Deck or brd to finish…", "Checking the update onto origin/master…", "Updating to origin/master…" or "Syncing branches…"
  [@test] src/lib/update-state.test.ts (`names the running step in the tooltip, never the last state`)

#### Scenario: Another fetch of the baseline runs
- **GIVEN** another fetch of the baseline runs
- **WHEN** the fetch of the update waits for it
- **THEN** the running step names whom it waits for, "Fetching origin/master with `brd update` (pid 4242, for 3 s)…"
  [@test] crates/sync-core/src/fetch_lock_test.rs (`a_fetch_beside_another_fetch_of_its_tracking_ref_takes_its_result`)

### Requirement: Header: Every step is listed from the start of the run

Every step MUST be listed from the start of the run, the update of the files and the sync too, so one look shows the
whole run: a check that finds nothing to apply ends the run before them. A wait that ended is no step of its own, and
a step that ran again (the check under the lock after a wait) is listed once.

#### Scenario: A wait that ended and a step that ran again
- **WHEN** a wait ended, and the check ran again under the lock after it
- **THEN** the wait is no step of its own, and the check is listed once
  [@test] src/lib/update-state.test.ts (`lists the steps done, the running one and those left, without a wait that ended, a step that ran again once`)

#### Scenario: Nothing to apply
- **WHEN** the check finds nothing to apply
- **THEN** the run ends before the update of the files and the sync, at its result, and its sync shows on Sync
  [@test] tests/e2e/update-project/update-project.spec.ts (`should end the progress at the result when there is nothing to apply, its sync on Sync`)

### Requirement: Header: The session tells the steps of the update's operation

The session MUST tell the steps of the operation of the update on the channel of its `execute`: `fetching` with the
progress of the fetch, `queued` behind a rewrite that holds HEAD when its fetch ended, `checking` (before the lock),
`waitingForLock` with the holder when another holds the lock as the update asks for it, `updating`; the run SHALL show
a wait as its step `waiting`, after the steps done.

#### Scenario: The steps of an update that applies
- **WHEN** an update fetches, checks and moves
- **THEN** its operation tells `fetching`, `checking` and `updating` on the channel of its `execute`
  [@test] crates/operations/src/update_test.rs (`an_update_fetches_checks_and_moves_then_asks_its_sync_with_its_undo`)

#### Scenario: A fetch with git's progress, then a wait behind a rewrite
- **WHEN** the fetch reports git's progress, and a rewrite holds HEAD when the fetch ended
- **THEN** the steps are `fetching` with the progress, `queued`, then `checking`, and the run shows the wait as `waiting`
  [@test] src/stores/update-project.test.ts (`shows the steps of its operation: the fetch with git's progress, the wait behind a rewrite, then the check`)

#### Scenario: Another holds the lock
- **WHEN** another holds the lock as the update asks for it
- **THEN** the step is `waitingForLock` with the holder
  [@test] crates/update-core/src/update_project_test.rs (`update_takes_the_repository_lock_after_the_fetch_and_says_it_waits`)

### Requirement: Header: The check under the lock is `checking` again

When the branch or the upstream moved meanwhile, the check once more under the lock MUST be `checking` again, so the
step no longer names another holder.

#### Scenario: The check again after the wait
- **GIVEN** the branch or the upstream moved while the update waited for the lock
- **WHEN** the update checks once more under the lock
- **THEN** the step is `checking` again, without the holder
  [@test] crates/update-core/src/update_project_test.rs (`the_check_again_after_the_wait_for_the_lock_is_the_check_step`)

### Requirement: Header: A step after the outcome changes nothing

A step after the outcome MUST change nothing: the steps after it are the frontend's.

#### Scenario: A late step
- **WHEN** a step arrives after the outcome of the update
- **THEN** it is ignored
  [@test] src/stores/update-project.test.ts (`ignores a step that arrives after its outcome`)

### Requirement: Header: The update after a push into the target is part of the landing

Update Project that follows a successful push into the target MUST be part of that landing's operation, not a second
start: the push's CAS already moved the target tracking ref, so the update has no network fetch; it holds the target
until its sync ends.

#### Scenario: A push into the target
- **WHEN** a push into the target succeeds
- **THEN** an update of its own follows in the same operation, without a network fetch, and drops the original whose copy landed
  [@test] crates/operations/src/update_test.rs (`a_push_into_the_target_is_followed_by_an_update_of_its_own_that_drops_the_original_whose_copy_landed`)

#### Scenario: Two pushes into the target
- **WHEN** two pushes into the target start
- **THEN** the update of the first holds the target until its sync ends, and the second runs after it, replanned
  [@test] crates/operations/src/update_test.rs (`two_pushes_into_the_target_run_one_after_the_other_with_the_second_replanned`)

### Requirement: Header: The push and its update are one operation in the journal

The start and update outcomes MUST use the same event channel and operation id, and the journal MUST contain one
entry. A target that refused the push as `baseline_moved` SHALL still take the fetch path.

#### Scenario: The journal of a push and its update
- **WHEN** a push into the target is followed by its update
- **THEN** the start and the update outcomes come on the same event channel with the same operation id, and the journal has one entry
  [@test] crates/operations/src/update_test.rs (`a_push_into_the_target_is_followed_by_an_update_of_its_own_that_drops_the_original_whose_copy_landed`)

#### Scenario: The target refused the push
- **WHEN** the target refused the push as `baseline_moved`
- **THEN** the update takes the fetch path

### Requirement: Header: The step and the fetch progress belong to their repository

The step and the fetch progress MUST belong to the repository the update (or the Fetch of a merge toast) runs in:
after a switch, the buttons show the selected repository's, never another one's.

#### Scenario: A switch of repositories during an update
- **GIVEN** an update runs in one repository
- **WHEN** the user switches to another repository
- **THEN** the buttons show the selected repository's step, never the other repository's
  [@test] src/stores/update-project.test.ts (`belongs to its repository: switching repositories never shows the other repository's phase`)

### Requirement: Header: One update or fetch runs at a time in a repository, none dropped

One update or fetch MUST run at a time in a repository, and none is dropped: the session joins a second Update (⌘T, a
toast) into the running update, the fetch of an Update during the Fetch of a toast into that fetch, which the update
goes on from with the check and the apply (no second fetch), and a Fetch during the fetch of an update into it;
another repository SHALL be able to update meanwhile.

#### Scenario: A second Update and a Fetch during an update
- **GIVEN** an update runs
- **WHEN** a second Update (⌘T, a toast) or a Fetch starts in the same repository
- **THEN** the second Update joins the running update, and the Fetch joins its fetch
  [@test] crates/operations/src/update_test.rs (`a_second_update_joins_the_update_and_a_fetch_joins_its_fetch`)
  [@test] src/stores/update-project.test.ts (`runs one run at a time in a repository: a second start joins it, and an update during a Fetch goes on from that fetch with its check`)

#### Scenario: An Update during the Fetch of a toast
- **GIVEN** the Fetch of a merge toast runs
- **WHEN** the user presses Update
- **THEN** its fetch joins that fetch, and the update goes on from it with the check and the apply, without a second fetch
  [@test] crates/operations/src/update_test.rs (`an_update_during_a_fetch_goes_on_from_that_fetch`)
  [@test] src/stores/update-project.test.ts (`runs one run at a time in a repository: a second start joins it, and an update during a Fetch goes on from that fetch with its check`)
  [@test] tests/e2e/update-project/landing-toast.spec.ts (`should go on from the Fetch of the toast with an update pressed during it: the update checks and moves on that fetch`)

#### Scenario: Another repository
- **WHEN** an update runs in one repository
- **THEN** another repository can update meanwhile

### Requirement: Header: The sync after an update is the one of its own session

The sync after an update MUST be the one of its own session, never the selected repository's; the outcome of an
operation MUST be told once, also to a start that joined it.

#### Scenario: The selected repository changed
- **WHEN** an update ends after the user selected another repository
- **THEN** its sync runs in its own session, never in the selected repository's

#### Scenario: A start that joined the operation
- **WHEN** a start joined a running update
- **THEN** the outcome of the operation is told once, also to that start

### Requirement: Header: The fetch of an update holds no HEAD

The fetch of an update MUST hold no HEAD: a move, a park, an Unstack, a reword or an Undo during the fetch runs at
once.

#### Scenario: A rewrite during the fetch
- **GIVEN** an update fetches
- **WHEN** a move, a park, an Unstack, a reword or an Undo starts
- **THEN** it runs at once, and the update checks and moves what it left
  [@test] crates/operations/src/update_test.rs (`a_rewrite_during_the_fetch_of_an_update_runs_at_once_and_the_update_moves_what_it_left`)
  [@test] tests/e2e/update-project/update-project.spec.ts (`should let a rewrite run during the fetch of an update, which checks and applies after it`)

### Requirement: Header: The check and the move of the update hold HEAD

The check and the move of the update MUST hold HEAD ([Running Operations](../../../../docs/design/operations.md#update-project)):
while a regroup, a squash or another rewrite holds it, Update stays enabled with the busy state it would wait for on
hover ("Unstacking oauth…"); a start fetches, then its check waits for that rewrite in the queue of the session (the
step "waiting"), never refused, and runs on what it left.

#### Scenario: Update pressed during a rewrite
- **GIVEN** a regroup, a squash or another rewrite holds HEAD
- **WHEN** the user hovers Update, then presses it
- **THEN** Update is enabled, its hover names the busy state it would wait for ("Unstacking oauth…")
- **AND** the update fetches, then its check waits for the rewrite in the queue of the session (the step "waiting"), never refused, and runs on what it left
  [@test] crates/operations/src/update_test.rs (`an_update_waits_behind_a_rewrite_and_a_rewrite_after_it_behind_the_update`)
  [@test] tests/e2e/update-project/update-project.spec.ts (`should run an update pressed during a rewrite after it, waiting for it in the queue of the session`)

### Requirement: Header: A rewrite during the check and the move waits behind them

A rewrite started while the check and the move of the update run MUST wait behind them in the queue of the session,
and run on what the update left.

#### Scenario: A rewrite during the check
- **GIVEN** the update checks or moves
- **WHEN** a rewrite starts
- **THEN** it waits behind the update in the queue of the session, and runs on what the update left
  [@test] crates/operations/src/update_test.rs (`a_rewrite_during_the_check_of_an_update_waits_behind_it_and_runs_on_what_it_left`, `an_update_waits_behind_a_rewrite_and_a_rewrite_after_it_behind_the_update`)
  [@test] tests/e2e/update-project/update-project.spec.ts (`should run a rewrite pressed during the check of an update after it, on what the update left`)

### Requirement: Header: Update pressed during a sync goes to the session at once

Update pressed during a sync MUST go to the session at once, never refused: it fetches, checks and moves, and answers
with the sync it asked, after the running one; no sync of its own besides that one.

#### Scenario: Update during a held sync
- **GIVEN** a sync runs
- **WHEN** the user presses Update
- **THEN** the update fetches, checks and moves at once, and answers with the sync it asked, once the running one ended
- **AND** it runs no sync of its own besides that one
  [@test] tests/e2e/update-project/update-project.spec.ts (`should run an update pressed during a held sync at once, its result once that sync ended, one sync for the update`)

### Requirement: Header: git's stages of the fetch move the progress of the update

Once git reports a stage of the fetch, its percent of the whole fetch MUST move the progress of the update, and the
tooltip SHALL show the last progress line of git, as IDEA shows a fetch. A push reports its progress the same way
([Outgoing](../outgoing/spec.md)).

#### Scenario: A fetch reports its stages
- **WHEN** git reports the stages of the fetch of an update
- **THEN** the percent of the whole fetch moves the progress, until the last stage ends
- **AND** the tooltip shows the last progress line of git
  [@test] crates/update-core/src/update_project_test.rs (`fetch_reports_progress_until_the_last_stage_ends`)

### Requirement: Header: The percent weighs the stages as IDEA does

The percent MUST weigh the stages as IDEA's `GitStandardProgressAnalyzer` does (counting 5 %, compressing 10 %,
receiving or unpacking 80 %, resolving deltas 5 %), never decrease, and reach the window at most every 100 ms; the end
of a stage always does.

#### Scenario: The shares of the stages
- **WHEN** git reports its stages
- **THEN** counting weighs 5 %, compressing 10 %, receiving 80 % and resolving deltas 5 %
  [@test] crates/sync-core/src/transfer_progress_test.rs (`stages_have_the_shares_of_idea`)

#### Scenario: A small fetch
- **WHEN** a small fetch unpacks instead of receiving
- **THEN** unpacking takes the share of receiving, 80 %
  [@test] crates/sync-core/src/transfer_progress_test.rs (`small_fetch_unpacks_instead_of_receiving`)

#### Scenario: A later stage reports less
- **WHEN** a later line would give a lower percent
- **THEN** the percent never decreases
  [@test] crates/sync-core/src/transfer_progress_test.rs (`percent_never_decreases`)

#### Scenario: The progress reaches the window
- **WHEN** git reports progress faster than every 100 ms
- **THEN** the first progress and every stage end reach the window, the rest at most every 100 ms
  [@test] crates/sync-core/src/transfer_progress_test.rs (`reporter_sends_the_first_progress_and_every_stage_end`)

### Requirement: Header: The progress is read from git's `--progress` lines, best effort

The progress MUST be read from `--progress` lines on stderr in the C locale (git has no machine-readable progress),
best effort: a line it doesn't know changes nothing, and nothing but the progress depends on it. The error of a failed
fetch MUST keep git's messages without the progress updates.

#### Scenario: A line it doesn't know
- **WHEN** stderr has a line that is no known progress line
- **THEN** it changes nothing
  [@test] crates/sync-core/src/transfer_progress_test.rs (`other_lines_change_nothing`)

#### Scenario: The C locale
- **WHEN** the fetch runs
- **THEN** git runs untranslated, in the C locale
  [@test] crates/sync-core/src/remote_command_test.rs (`remote_command_runs_untranslated`)

#### Scenario: A failed fetch
- **WHEN** the fetch fails
- **THEN** its error keeps git's messages without the progress updates
  [@test] crates/git-executor/src/git_command_executor_test.rs (`failed_command_reports_messages_without_progress_updates`)

### Requirement: Header: A toast says what an applied or up-to-date update did

After an applied update a toast MUST say "Updated master: rebased 2 commits onto origin/master" or "…: fast-forwarded
to origin/master". When commits landed, the toast SHALL lead with them instead
([Landed Commits](../landed-commits/spec.md)). Up to date SHALL say "master is already up to date with origin/master".

#### Scenario: A clean update, then up to date
- **WHEN** a clean update applies, and the next one is up to date
- **THEN** the toast says "Updated master: rebased 2 commits onto origin/master" or "…: fast-forwarded to origin/master", then "master is already up to date with origin/master"
  [@test] tests/e2e/update-project/update-project.spec.ts (`should show a clean update, apply it and then be up to date`)

#### Scenario: Commits landed
- **WHEN** the update brought in commits that landed
- **THEN** the toast leads with them instead

### Requirement: Header: The toast of an update takes the place of the merge toasts

Either toast MUST take the place of the open toasts of the merges of the repository, whose Update Project it did, and
every surface of a running update SHALL show it there: the Update Project of such a toast says "Updating…" while it
runs ([Patronus](../../branch/patronus/spec.md#merged), [One toast per event](../../../../docs/contributing/ux-writing.md#one-toast-per-event)).

#### Scenario: The toasts of the merges it brought in
- **GIVEN** toasts of merges of the repository are open
- **WHEN** an update brings those merges in, landed or up to date
- **THEN** its toast takes their place
  [@test] src/stores/update-project.test.ts (`takes the place of the toasts of the merges of its repository it brought in: what landed, or up to date`)

#### Scenario: The update starts from the header
- **GIVEN** the toast of a merge is open
- **WHEN** the update starts from the header
- **THEN** that toast turns into what the update landed, in place, and its Update Project says "Updating…" while it runs
  [@test] tests/e2e/update-project/landing-toast.spec.ts (`should turn the toast of a merge into what the update landed, in place, when the update starts from the header`)

### Requirement: Header: A sync follows every update whose fetch finished

A sync MUST follow every update whose fetch finished, without expanding or scrolling: applied, up to date,
conflicting, blocked, or failed after the fetch; the session asks it after the move, and the operation ends with that
request. The fetch moved the baseline, so integration, the remote status and what landed are built against it
although the branch itself didn't change.

#### Scenario: An update with nothing to apply
- **WHEN** an update is up to date, conflicting or blocked
- **THEN** a sync follows it without expanding or scrolling, and builds integration, the remote status and what landed against the fetched baseline
  [@test] tests/e2e/update-project/update-project.spec.ts (`should end the progress at the result when there is nothing to apply, its sync on Sync`)

#### Scenario: An update that failed after its fetch
- **WHEN** an update fails or is stopped after its fetch
- **THEN** the operation still ends with the request of a sync
  [@test] crates/operations/src/update_test.rs (`stop_waiting_ends_the_wait_of_an_update_for_the_lock_with_nothing_moved`)

### Requirement: Header: The sync of an applied update is the last step of its progress

The sync of an applied update MUST be the last step of its progress: the Update button stays busy until that sync run
ends (a ⌘R meanwhile asks for a follow-up that is Sync's only), and Sync shows its spinner as for any sync. Up to
date, refused or failed, the update SHALL end with its result, and its sync shows on Sync alone.

#### Scenario: An applied update
- **WHEN** an update applies
- **THEN** the Update button stays busy until its sync run ends, and Sync shows its spinner
- **AND** a ⌘R meanwhile asks for a follow-up that is Sync's only: the request of the update resolves when its own run ends
  [@test] src/stores/update-project.test.ts (`is busy through the sync of an applied update, its own run of it; the toast doesn't wait for the sync`)
  [@test] crates/repository-model/src/session_test.rs (`a_ticket_resolves_when_its_run_ends_not_the_chain`)

#### Scenario: Up to date, refused or failed
- **WHEN** an update is up to date, refused or failed
- **THEN** it ends with its result, and its sync shows on Sync alone
  [@test] tests/e2e/update-project/update-project.spec.ts (`should end the progress at the result when there is nothing to apply, its sync on Sync`)
  [@test] src/stores/update-project.test.ts (`keeps the fetch time when the update fails after its fetch, e.g. while brd holds the repository`)

### Requirement: Header: The toast and the dialog come with the result

The toast and the dialog MUST come with the result, never after the sync: they say what happened, the sync only
rebuilds the rows.

#### Scenario: The result of an update
- **WHEN** an update has its result
- **THEN** its toast or its dialog shows at once, without waiting for the sync
  [@test] src/stores/update-project.test.ts (`is busy through the sync of an applied update, its own run of it; the toast doesn't wait for the sync`)

### Requirement: Header: The footer of the landing overview shows the same state

The footer of the landing overview MUST show the same state in one line, with the state icon of the button, named as
incoming and with the fetch time, "Incoming from origin/master: rebases cleanly · fetched 5 minutes ago" (up to date:
"nothing new"), with an Update button. Below the rows of the work on its way, a bare "origin/master: up to date"
reads as a state of that work.

#### Scenario: The footer of a clean update
- **WHEN** the landing overview is shown
- **THEN** its footer says "Incoming from origin/master: rebases cleanly · fetched 5 minutes ago", or "nothing new" when up to date, with the state icon of the button and an Update button
- **AND** never a bare "origin/master: up to date"
  [@test] tests/e2e/update-project/update-project.spec.ts (`should show the update state in the footer of the landing overview`)

### Requirement: Header: While the update runs, the footer names its step

While the update runs, the footer text MUST name its step in lower case, "Incoming from origin/master: checking the
update…", and while the fetch receives it has the amount and the rate as git prints them, "Incoming from
origin/master: 5.79 MiB · 32.57 MiB/s".

#### Scenario: The step in the footer
- **WHEN** the update checks
- **THEN** the footer says "Incoming from origin/master: checking the update…"
  [@test] src/lib/update-state.test.ts (`says the step in the footer, the amount and the rate while the fetch receives`)

#### Scenario: The fetch receives
- **WHEN** the fetch receives
- **THEN** the footer says "Incoming from origin/master: 5.79 MiB · 32.57 MiB/s", the amount and the rate as git prints them
  [@test] src/lib/transfer-progress.test.ts (`shows the amount and the rate as git prints them`)
  [@test] src/lib/update-state.test.ts (`says the step in the footer, the amount and the rate while the fetch receives`)

### Requirement: Header: The footer's Update button has the label of the header button

The footer's button MUST have the label of the header button, without a second bar (the header shows above the
overview), and keep its full colour while the update runs, as the one in the header.

#### Scenario: The footer's button during an update
- **WHEN** the update runs
- **THEN** the footer's button has the label of the header button, without a second bar, in its full colour

### Requirement: Conflict and blocked dialog: A conflict or a block opens a dialog

A conflict or a block after a click MUST open a dialog in the frame of the previews: it says nothing was changed and
when the baseline was fetched.

#### Scenario: An update that conflicts or is blocked
- **WHEN** the user clicks Update and the update conflicts or is blocked
- **THEN** a dialog opens in the frame of the previews
- **AND** it says nothing was changed and when the baseline was fetched

### Requirement: Conflict and blocked dialog: The dialog reads the brd command as it opens

The dialog MUST read the app's brd command in the repository when it opens (`agent_cli_command`), so the copy needs no
wait; Copy SHALL wait until it is read.

#### Scenario: The dialog opens
- **WHEN** the dialog opens
- **THEN** it reads the app's brd command in the repository (`agent_cli_command`), so the copy needs no wait

#### Scenario: Copy before the command is read
- **WHEN** the user clicks Copy before the command is read
- **THEN** Copy waits until it is read

### Requirement: Conflict and blocked dialog: The conflict title counts all files of the first conflict

The conflict title MUST count all files of the first conflict, "1 file conflicts with origin/master" or "6 files
conflict with origin/master"; without known files it SHALL say "Your commits conflict with origin/master".

#### Scenario: Known files
- **WHEN** the first conflict has known files
- **THEN** the title is "1 file conflicts with origin/master" or "6 files conflict with origin/master", counting all of them

#### Scenario: No known files
- **WHEN** the first conflict has no known files
- **THEN** the title is "Your commits conflict with origin/master"

### Requirement: Conflict and blocked dialog: A conflict names the first conflict and its files

A conflict MUST name the first conflict, "First conflict <id> <subject>": the commit where the rebase stops first.
Below it the files of its merge, each with that commit ("yours", with its author time) and the upstream commits that
change it. A file that only a merge of the whole branch would name MUST never be listed.

#### Scenario: The files of the first conflict
- **WHEN** the dialog shows a conflict
- **THEN** it names "First conflict <id> <subject>", the commit where the rebase stops first
- **AND** below it each file of its merge, with that commit ("yours", with its author time) and the upstream commits that change it, in their order
  [@test] src/lib/update-conflict.test.ts (`gives each file the commits of each side that change it, in their order`)
  [@test] tests/e2e/update-project/update-project.spec.ts (`should explain a conflict and copy a prompt for an agent`)

#### Scenario: A file only a merge of the whole branch would name
- **WHEN** a file would conflict only in a merge of the whole branch
- **THEN** it is never listed

### Requirement: Conflict and blocked dialog: An upstream commit shows both times when they differ

An upstream commit MUST show its author and both times when they differ by more than an hour, "Daniil Bubnov ·
authored 13 days ago · landed 2 hours ago", else one, "Daniil Bubnov, 13 days ago": a commit written long ago can
have reached the upstream just now.

#### Scenario: Landed more than an hour after it was written
- **WHEN** an upstream commit landed more than an hour after it was written
- **THEN** it shows "Daniil Bubnov · authored 13 days ago · landed 2 hours ago"
  [@test] src/lib/update-conflict.test.ts (`names when the commit was written and when it landed, if that was more than an hour later`)

#### Scenario: Landed within an hour
- **WHEN** an upstream commit landed within an hour of being written
- **THEN** it shows one time, "Daniil Bubnov, 13 days ago"
  [@test] src/lib/update-conflict.test.ts (`gives one time when the commit landed within an hour of being written`)

### Requirement: Conflict and blocked dialog: The line above the conflict says what to do

The line above SHALL say to rebase in a terminal or the IDE, or, with a brd command, that the prompt resolves each
commit with brd, outside the working tree.

#### Scenario: With a brd command
- **WHEN** the app has a brd command in the repository
- **THEN** the line above says that the prompt resolves each commit with brd, outside the working tree

#### Scenario: Without a brd command
- **WHEN** the app has no brd command
- **THEN** the line above says to rebase in a terminal or the IDE

### Requirement: Conflict and blocked dialog: A large conflict stays a short list

A large conflict MUST stay a short list: the files the check reported (50), then "… and 10 more files"; per file at
most 5 commits a side, yours the oldest (the first to stop the rebase) and the upstream's the newest, then "… and 2
more".

#### Scenario: A wide conflict
- **WHEN** the conflict has more files than the check reported
- **THEN** the dialog lists the files the check reported (50), then "… and 10 more files"
- **AND** per file at most 5 commits a side, yours the oldest and the upstream's the newest, then "… and 2 more"
  [@test] tests/e2e/update-project/update-project.spec.ts (`should list the first files of a wide conflict and count the rest`)

### Requirement: Conflict and blocked dialog: The tooltip names three files of all

The tooltip MUST name three files of all, "Conflicts in 60 files: a, b, c and 57 more" (four of four).

#### Scenario: The tooltip of a wide conflict
- **WHEN** the conflict has 60 files, more than the backend listed
- **THEN** the tooltip says "Conflicts in 60 files: a, b, c and 57 more", counting the files the backend left out of its list
- **AND** four files of four are all named
  [@test] src/lib/update-state.test.ts (`counts the files the backend left out of its list`)

### Requirement: Conflict and blocked dialog: A block lists the paths the check reported

A block MUST list the paths the check reported (100), then "… and 20 more files".

#### Scenario: Many blocking paths
- **WHEN** more paths block than the check reported
- **THEN** the dialog lists the paths the check reported (100), then "… and 20 more files"

### Requirement: Conflict and blocked dialog: No list leaves out only one

Every list MUST be cut so that the count of the rest is never 1: one more than the limit is listed whole (51 files, 6
commits a side, 101 paths; `cutList`, and the check's `list_cut::kept`).

#### Scenario: One file wider than the list
- **WHEN** the conflict has 51 files
- **THEN** the check reports every file with its commits (`list_cut::kept`)
  [@test] crates/update-core/src/update_project_test.rs (`a_conflict_one_file_wider_than_the_list_reports_every_file_with_its_commits`)

#### Scenario: One path more than the list
- **WHEN** 101 paths block the update
- **THEN** the check reports all 101 (`list_cut::kept`), never a count of one
  [@test] crates/update-core/src/update_project_test.rs (`many_blocking_paths_are_counted`)

#### Scenario: A list one longer than its limit
- **WHEN** a list is one longer than its limit, 51 files, 6 commits a side or 101 paths
- **THEN** it is listed whole (`cutList`), and a longer one is cut at the limit, counting the rest of all
  [@test] src/lib/text.test.ts (`keeps a list one longer than its limit whole and cuts a longer one at the limit, counting the rest of all`)

### Requirement: Conflict and blocked dialog: A block lists the overlapping paths

A block MUST list the overlapping paths and say to commit or stash them; changes to other files don't block. It SHALL
say that the prompt asks about each file, then updates with brd, or, without a brd command, that you update again.

#### Scenario: Uncommitted changes block the update
- **WHEN** the dialog shows a block
- **THEN** it lists the overlapping paths and says to commit or stash them, and changes to other files are not listed
- **AND** it says that the prompt asks about each file, then updates with brd
  [@test] tests/e2e/update-project/update-project.spec.ts (`should explain uncommitted changes that block the update`)

#### Scenario: A block without a brd command
- **WHEN** the app has no brd command
- **THEN** the dialog says that you update again

### Requirement: Conflict and blocked dialog: "Copy Prompt for Agent" copies and confirms in place

The primary button "Copy Prompt for Agent" MUST copy a prompt and confirm in place: it says "Copied" (`CopyCheckIcon`)
for a moment, as a copy button does, and no toast covers the dialog. Both cases SHALL offer it: an overlap of
uncommitted changes with the upstream needs the same merge as a conflict.

#### Scenario: Copy the prompt of a conflict
- **WHEN** the user clicks "Copy Prompt for Agent"
- **THEN** a prompt is copied, and the button says "Copied" (`CopyCheckIcon`) for a moment, with no toast over the dialog
  [@test] tests/e2e/update-project/update-project.spec.ts (`should explain a conflict and copy a prompt for an agent`)

#### Scenario: A blocked update
- **WHEN** the dialog shows a block
- **THEN** it offers "Copy Prompt for Agent" too

### Requirement: Conflict and blocked dialog: With brd, the conflict prompt sends the agent to brd's session

With a brd command, the conflict prompt MUST send the agent to brd's resolve session and keep it out of the working
tree.

#### Scenario: A conflict with a brd command
- **GIVEN** the app has a brd command in the repository
- **WHEN** the user copies the prompt of a conflict
- **THEN** it sends the agent to brd's resolve session, outside the working tree
  [@test] src/lib/agent-prompt.test.ts (`with brd, resolves a conflict in brd's session, outside the working tree`)
  [@test] tests/e2e/update-project/update-project.spec.ts (`should explain a conflict and copy a prompt for an agent`)

### Requirement: Conflict and blocked dialog: The brd prompt leaves the protocol to brd

The brd conflict prompt MUST hold only what brd does not know and leave the protocol to brd (`brd help update`, the
`hint` of each file and of the commit, `session.next`, `ask_user`, `retryable`).

#### Scenario: What the prompt holds
- **WHEN** the prompt of a conflict with brd is copied
- **THEN** it holds only what brd does not know, and leaves the protocol to `brd help update`, the `hint` of each file and of the commit, `session.next`, `ask_user` and `retryable`
  [@test] src/lib/agent-prompt.test.ts (`with brd, resolves a conflict in brd's session, outside the working tree`)

### Requirement: Conflict and blocked dialog: The brd prompt names the repository and brd once

The brd conflict prompt MUST name the repository path, the branch and the upstream; brd once, as the command to run as
written (in PowerShell on Windows), with "brd is `<command>`; below, `brd` stands for it", "Run `brd help update`
first and follow it", one JSON object per run.

#### Scenario: brd named once
- **WHEN** the prompt of a conflict with brd is copied
- **THEN** it names the repository path, the branch and the upstream
- **AND** "brd is `<command>`; below, `brd` stands for it", "Run `brd help update` first and follow it", one JSON object per run
  [@test] src/lib/agent-prompt.test.ts (`with brd, resolves a conflict in brd's session, outside the working tree`)

#### Scenario: A conflict on Windows
- **WHEN** the prompt is copied on Windows
- **THEN** it names the command to run as written in PowerShell
  [@test] src/lib/agent-prompt.test.ts (`names PowerShell for a conflict on Windows`)

### Requirement: Conflict and blocked dialog: The brd prompt names the state the user saw and the first conflict

The brd conflict prompt MUST give the state the user saw on one line ("State (fetched <time>): HEAD (<branch>) <id>,
<upstream> <id>."), then "First conflict: <id> <subject>: <files>." (the commit where brd opens its first round, with
the files of its merge; each round names the upstream commits of its files, so the prompt describes none).

#### Scenario: The state and the first conflict
- **WHEN** the prompt of a conflict with brd is copied
- **THEN** it says "State (fetched <time>): HEAD (<branch>) <id>, <upstream> <id>." on one line
- **AND** then "First conflict: <id> <subject>: <files>.", the commit where brd opens its first round, and describes no upstream commit
  [@test] src/lib/agent-prompt.test.ts (`with brd, resolves a conflict in brd's session, outside the working tree`)

### Requirement: Conflict and blocked dialog: Step 1 of the brd prompt runs `brd update --no-fetch`

The first step of the brd conflict prompt MUST be `brd update --no-fetch` (the upstream the user saw, no
`--expect-head`, because peers commit meanwhile); when an output or its `error` has `ask_user: true`, show the user its
`reasons` (or the error's `message`) with `session.blocked_paths` and `session.landed_differs`, and ask once; a later
round of a commit decided in `landed_differs` takes that answer.

#### Scenario: Step 1
- **WHEN** the prompt of a conflict with brd is copied
- **THEN** its step 1 is `brd update --no-fetch`, without `--expect-head`
- **AND** on `ask_user: true` in an output or its `error`, the agent shows the user its `reasons` (or the error's `message`) with `session.blocked_paths` and `session.landed_differs` and asks once, and a later round of a commit decided in `landed_differs` takes that answer
  [@test] src/lib/agent-prompt.test.ts (`with brd, resolves a conflict in brd's session, outside the working tree`)

### Requirement: Conflict and blocked dialog: Step 2 of the brd prompt follows the hints

The second step of the brd conflict prompt MUST be: while it exits 1, follow the `hint` of each file and of
`session.commit`, edit only the copies at `edit`, and run `session.next`.

#### Scenario: Step 2
- **WHEN** the prompt of a conflict with brd is copied
- **THEN** its step 2 says: while it exits 1, follow the `hint` of each file and of `session.commit`, edit only the copies at `edit`, and run `session.next`
  [@test] src/lib/agent-prompt.test.ts (`with brd, resolves a conflict in brd's session, outside the working tree`)

### Requirement: Conflict and blocked dialog: Step 3 of the brd prompt builds and tests

The third step of the brd conflict prompt MUST be: on exit 0, build, and run the tests of `verify.paths` (without
`verify`, the build is enough), and on a failure a `fixup! <subject>` commit of only the fixed files with `brd fold`,
or `brd undo <undo_token>`.

#### Scenario: Step 3
- **WHEN** the prompt of a conflict with brd is copied
- **THEN** its step 3 says: on exit 0, build, and run the tests of `verify.paths`, the build alone without `verify`
- **AND** on a failure, a `fixup! <subject>` commit of only the fixed files with `brd fold`, or `brd undo <undo_token>`
  [@test] src/lib/agent-prompt.test.ts (`with brd, resolves a conflict in brd's session, outside the working tree`)

#### Scenario: The fixup commit
- **WHEN** step 3 names the fixup commit
- **THEN** it is `git -C <repository> commit -m 'fixup! <subject>' -- <paths>`, the repository quoted for the shell, never the changes of other sessions
- **AND** in PowerShell each `'` of the subject is doubled
  [@test] src/lib/agent-prompt.test.ts (`names PowerShell for a conflict on Windows`)

### Requirement: Conflict and blocked dialog: Step 4 of the brd prompt reports the resolutions

The fourth step of the brd conflict prompt MUST be: report each edited resolution of `resolutions` and `resolved` with
its dropped lines, each drop the user decided (`landed_as` with `local_only`), and `dropped` as a count per `proof`.

#### Scenario: Step 4
- **WHEN** the prompt of a conflict with brd is copied
- **THEN** its step 4 says to report each edited resolution of `resolutions` and `resolved` with its dropped lines, each drop the user decided (`landed_as` with `local_only`), and `dropped` as a count per `proof`
  [@test] src/lib/agent-prompt.test.ts (`with brd, resolves a conflict in brd's session, outside the working tree`)

### Requirement: Conflict and blocked dialog: The brd prompt keeps the agent off the tree and the history

The rules of the brd conflict prompt MUST say: other sessions work in the tree, so never `git rebase`, `git stash`,
`git checkout` or `git reset`, and never edit a conflicting file in the working tree; keep every message with its
`(prefix)`; don't squash, reorder or split commits, and don't drop a commit yourself, because brd drops a commit it
proves landed and `--landed` drops one after the user's yes (`UPDATE_RULES`); don't push.

#### Scenario: The rules with brd
- **WHEN** the prompt of a conflict with brd is copied
- **THEN** it forbids `git rebase`, `git stash`, `git checkout`, `git reset` and an edit of a conflicting file in the working tree, as other sessions work in the tree
- **AND** it says to keep every message with its `(prefix)`, not to squash, reorder or split commits nor drop one itself (`UPDATE_RULES`), and not to push
  [@test] src/lib/agent-prompt.test.ts (`with brd, resolves a conflict in brd's session, outside the working tree`)

#### Scenario: The prompt without brd and the robot prompt
- **WHEN** the prompt without brd or the robot prompt is copied
- **THEN** it keeps "don't squash, reorder, split or drop commits"

### Requirement: Conflict and blocked dialog: On an error the brd prompt follows `retryable` or the message

The brd conflict prompt MUST say: on an error, `retryable` goes back to step 1, else the agent does what its `message`
says or stops and shows the error.

#### Scenario: An error of brd
- **WHEN** the prompt of a conflict with brd is copied
- **THEN** it says that on an error `retryable` goes back to step 1, else the agent does what its `message` says or stops and shows the error
  [@test] src/lib/agent-prompt.test.ts (`with brd, resolves a conflict in brd's session, outside the working tree`)

### Requirement: Conflict and blocked dialog: The brd prompt names no exit code above 1

The brd conflict prompt MUST name no exit code above 1, no error code and no rule of the markers: `brd help update`
has them, the PowerShell way to write a copy too.

#### Scenario: What the prompt leaves to brd
- **WHEN** the prompt of a conflict with brd is copied
- **THEN** it names no exit code above 1, no error code and no rule of the markers, which `brd help update` has, with the PowerShell way to write a copy
  [@test] src/lib/agent-prompt.test.ts (`with brd, resolves a conflict in brd's session, outside the working tree`)
  [@test] src/lib/agent-prompt.test.ts (`names PowerShell for a conflict on Windows`)

### Requirement: Conflict and blocked dialog: A conflict prompt stays a few kilobytes

A conflict prompt MUST stay a few kilobytes however large the conflict: it is a map, not the record; brd names the
files and the upstream commits of each round, git the rest.

#### Scenario: A conflict of hundreds of files
- **GIVEN** a conflict of hundreds of files with 50 upstream commits each
- **WHEN** the prompt is copied, with brd or without
- **THEN** it stays a few kilobytes
  [@test] src/lib/agent-prompt.test.ts (`with brd, names the first conflict with its first files`, `without brd, names the first conflict and the commits of its files`)

### Requirement: Conflict and blocked dialog: With brd the first conflict names at most 20 files

With brd the first conflict MUST name at most 20 files, then ", and <n> more files", counted as in the title.

#### Scenario: A wide conflict with brd
- **WHEN** the first conflict has more than 20 files
- **THEN** the brd prompt names 20 of them, then ", and <n> more files", counted as in the title, and describes no upstream commit
  [@test] src/lib/agent-prompt.test.ts (`with brd, names the first conflict with its first files`)

#### Scenario: No known files with brd
- **WHEN** the first conflict has no known files
- **THEN** the brd prompt names the first conflict without files
  [@test] src/lib/agent-prompt.test.ts (`names the first conflict without known files`)

### Requirement: Conflict and blocked dialog: Without brd the prompt lists 20 files and 3 commits a side

Without brd the prompt MUST list at most 20 files, then "… and <n> more files: `git status` names them when the rebase
stops"; per file at most 3 commits a side, by id: mine the oldest (the first to stop a replay), upstream the newest,
then "+<n> more", "+<n> or more" when the backend kept only the newest 50 upstream commits.

#### Scenario: A wide conflict without brd
- **WHEN** the first conflict has more than 20 files, and the backend kept only the newest 50 upstream commits
- **THEN** the prompt lists 20 files, then "… and <n> more files: `git status` names them when the rebase stops"
- **AND** per file at most 3 commits a side by id, mine the oldest and upstream the newest, then "+<n> or more"
  [@test] src/lib/agent-prompt.test.ts (`without brd, names the first conflict and the commits of its files`)

#### Scenario: The backend did not cut the upstream list
- **WHEN** the backend kept every upstream commit of a file
- **THEN** the rest is counted exactly, "+<n> more"
  [@test] src/lib/agent-prompt.test.ts (`counts the rest exactly when the backend did not cut the upstream list`)

### Requirement: Conflict and blocked dialog: Without brd each upstream commit is described once

Without brd each upstream commit MUST be described once, at most the 10 newest, then "… and <n> more:" with where the
rest are (`git log --oneline <base>..<upstream> -- <path>`).

#### Scenario: More than 10 upstream commits
- **WHEN** the files of the first conflict have more than 10 upstream commits
- **THEN** the prompt describes the 10 newest, each once, then "… and <n> more:" with `git log --oneline <base>..<upstream> -- <path>`
  [@test] src/lib/agent-prompt.test.ts (`without brd, names the first conflict and the commits of its files`)

### Requirement: Conflict and blocked dialog: A blocked prompt lists at most 50 paths

A blocked prompt MUST list at most 50 paths, then "… and <n> more: `git status` lists them", and ask about "these
files".

#### Scenario: Many blocking paths
- **WHEN** more than 50 paths block the update
- **THEN** the prompt lists 50, then "… and <n> more: `git status` lists them", and asks about "these files"
  [@test] src/lib/agent-prompt.test.ts (`caps the paths of a blocked update`)

### Requirement: Conflict and blocked dialog: No list of a prompt leaves out only one

No list of a prompt MUST leave out only one: one more than its limit is given whole (`cutList`), so the prompt never
says "… and 1 more files".

#### Scenario: A list one longer than its limit
- **WHEN** a list of the prompt is one longer than its limit: 21 files, 4 upstream commits, 51 paths
- **THEN** it is given whole (`cutList`), and the prompt never says "… and 1 more files"
  [@test] src/lib/agent-prompt.test.ts (`never counts one where the item would stand: a list one longer than its limit is given whole`)

### Requirement: Conflict and blocked dialog: Without brd the prompt names the repository and the exact ids

Without a brd command, the conflict prompt MUST contain the repository path, the branch, the upstream with the ids of
HEAD, the upstream and the merge base and the fetch time.

#### Scenario: The repository and the ids
- **GIVEN** the app has no brd command
- **WHEN** the prompt of a conflict is copied
- **THEN** it names the repository path, the branch, the upstream, the ids of HEAD, the upstream and the merge base, and the fetch time
  [@test] src/lib/agent-prompt.test.ts (`names the repository, the branch, the upstream and the exact ids`)

### Requirement: Conflict and blocked dialog: Without brd the prompt names the first conflict and its files

Without a brd command, the conflict prompt MUST contain the first conflict ("First conflict: my commit <id> <subject>,
where the rebase stops first", with `git log --reverse --oneline <base>..<head>` for the other commits) and the files
of the first conflict ("Files of the first conflict").

#### Scenario: The first conflict without brd
- **WHEN** the prompt of a conflict without brd is copied
- **THEN** it says "First conflict: my commit <id> <subject>, where the rebase stops first", with `git log --reverse --oneline <base>..<head>` for the other commits
  [@test] src/lib/agent-prompt.test.ts (`names the first commit that conflicts and how to list the others`)

#### Scenario: The files of the first conflict
- **WHEN** the files of the first conflict are known
- **THEN** the prompt lists them under "Files of the first conflict"
  [@test] src/lib/agent-prompt.test.ts (`names the commits on both sides of each likely conflicting file`)

#### Scenario: The conflicting files are unknown
- **WHEN** the conflicting files are unknown
- **THEN** the prompt leaves out the file list
  [@test] src/lib/agent-prompt.test.ts (`leaves out the file list when the conflicting files are unknown`)

### Requirement: Conflict and blocked dialog: Without brd the prompt states the rules and the steps

Without a brd command, the conflict prompt MUST contain the rules (keep every message with its `(prefix)`, don't
squash, reorder, split or drop commits, don't push; other sessions may work in the tree, so uncommitted changes stop
the agent, which asks the user and never stashes, resets, checks out or commits them) and the steps (rebase, resolve
keeping the upstream change and the intent of the commit, build and test, continue, report each resolution).

#### Scenario: The rules and the steps without brd
- **WHEN** the prompt of a conflict without brd is copied
- **THEN** it states the rules: keep every message with its `(prefix)`, don't squash, reorder, split or drop commits, don't push, and stop on uncommitted changes, ask the user and never stash, reset, check out or commit them
- **AND** the steps: rebase, resolve keeping the upstream change and the intent of the commit, build and test, continue, report each resolution
  [@test] src/lib/agent-prompt.test.ts (`states the rules and the steps`)

### Requirement: Conflict and blocked dialog: Without brd each file names the commits of both sides

Without a brd command, each file of the first conflict MUST name the ids of the local commits and the upstream commits
that change it; "Upstream commits of these files" SHALL describe each upstream one once, with its subject, author and
both dates: "(Daniil Bubnov, authored 2026-10-01, in origin/master since 2026-10-14)".

#### Scenario: The commits of each file
- **WHEN** the prompt of a conflict without brd is copied
- **THEN** each file names the ids of the local and the upstream commits that change it
- **AND** "Upstream commits of these files" describes each upstream one once, "(Daniil Bubnov, authored 2026-10-01, in origin/master since 2026-10-14)"
  [@test] src/lib/agent-prompt.test.ts (`names the commits on both sides of each likely conflicting file`)

### Requirement: Conflict and blocked dialog: The blocked prompt never stashes

The blocked prompt MUST never stash: it lists the files, says that other sessions work in the tree, forbids to stash,
reset, check out or commit them, and asks the agent to ask the user about each file.

#### Scenario: A blocked update
- **WHEN** the prompt of a blocked update is copied
- **THEN** it lists the files, says that other sessions work in the tree, forbids to stash, reset, check out or commit them, and asks the agent to ask the user about each file
  [@test] src/lib/agent-prompt.test.ts (`with brd, sends a blocked update to the user and then to brd, never to a stash`, `without brd, sends a blocked update to the user and back to Branch Deck, never to a stash`)
  [@test] tests/e2e/update-project/update-project.spec.ts (`should explain uncommitted changes that block the update`)

### Requirement: Conflict and blocked dialog: With bd the blocked prompt then runs the update

With the app's bd, the blocked prompt MUST then run `<bd> -C <repository> update --no-fetch` (quoted for the shell,
`& '…'` in PowerShell on Windows) and continue as its output says; if that prints `state: conflict`, the agent runs
`<bd> help update`, follows it and edits only the files at `edit`. Without one, it SHALL tell the user to update in
Branch Deck again.

#### Scenario: With bd
- **GIVEN** the app has its bd
- **WHEN** the prompt of a blocked update is copied
- **THEN** after the user's answers it runs `<bd> -C <repository> update --no-fetch`, quoted for the shell, and continues as its output says
- **AND** on `state: conflict` the agent runs `<bd> help update`, follows it and edits only the files at `edit`
  [@test] src/lib/agent-prompt.test.ts (`with brd, sends a blocked update to the user and then to brd, never to a stash`)

#### Scenario: On Windows
- **WHEN** the prompt of a blocked update is copied on Windows
- **THEN** the command is `& '…'` in PowerShell
  [@test] src/lib/agent-prompt.test.ts (`names PowerShell for the command of Windows`)

#### Scenario: Without bd
- **WHEN** the app has no bd command
- **THEN** the prompt tells the user to update in Branch Deck again
  [@test] src/lib/agent-prompt.test.ts (`without brd, sends a blocked update to the user and back to Branch Deck, never to a stash`)

## User Experience

- The button is `ghost` `sm` with `ArrowDownIcon` and "Update" (a `Spinner` and the run's label while it runs, its
  `ProgressLine` along the bottom edge in `primary`); the state icon follows the label (`CheckIcon`
  `text-success`, `PackageCheckIcon` `text-info` when the user's commits landed, `TriangleAlertIcon` `text-warning`,
  `CircleHelpIcon` muted for a failed check). Up to date mutes the label instead of adding an icon. The footer line
  starts with the same icon at `size-3`.
- Sync is the `ghost` `sm` button with `RefreshCwIcon`, as Update (a `Spinner` at full colour while it runs), the
  shortcut a `Kbd` in its tooltip; after it, in a group of its own and in every state (no repository, an invalid
  path), Settings, the `ghost` `icon-sm` `SettingsIcon` ("Settings" and ⌘, / Ctrl+, in its tooltip,
  [Settings](../settings/spec.md)). The header has no
  primary button: Sync runs by itself when the window opens the repository and after each change.
- The dialog uses `PreviewDialogFrame`: the title says what is wrong, the body starts with one line on what to do,
  then lists the files, each as its name and then its muted directory, which is clipped from its start on a narrow
  window, with a copy button on hover; the commits of each side sit below in aligned columns (side, hash, subject,
  then the time for yours and the author with the times for the upstream's, muted); the full list is a `Collapsible` with a `TriangleAlertIcon` `text-warning` mark. The footer shows
  `master ← origin/master`, `Esc Close` and the primary button.

## Testing / Local Run

- `cargo nextest run --workspace` covers the check, the update and the scan
  (`crates/update-core/src/update_project_test.rs`, `crates/git-executor/src/worktree_safety_test.rs`).
- `pnpm test:unit` covers the state words and the prompt.
- `pnpm e2e` covers the header, the dialog and the update with the templates `behind_upstream`, `update_conflict`,
  `update_conflict_wide` (60 files, 7 upstream commits each) and `update_blocked`: a clone with a bare upstream at
  `.git/upstream.git`, already fetched, so the update works offline.
  The upstream commit that conflicts landed 15 hours after it was written, so the dialog and the prompt show both
  times (`e2e_templates_have_the_expected_update_states` checks it); every template time is before the fixed clock of
  the e2e pages, 2024-01-02 10:00 UTC.

## Open Questions / Risks

- A write can still fail midway (a Git LFS smudge filter, a full disk) after git validated every path; the branch moves
  back and the written files show as uncommitted changes. See "Update project" in the design doc.
- The first check after a start or a fetch hashes the changed files the probe hasn't seen: about 60–130 µs a file on a
  loaded machine, so an update that changes tens of thousands of files costs seconds once.
- The usual time of a step is a guess from the last runs: a fetch that is much slower than usual moves slowly past its
  63 %, and the first update of a repository shares the bar by defaults. The test server forwards no progress either:
  the e2e tests send the events of a held update to the page themselves (`holdIntent`).

## References

- [Git Operations](../../../../docs/design/git-operations.md): "Update project", the exception to invariant 1, and "Resolve
  session".
- [brd update](../../cli/update/spec.md): the agent's update and its resolve session.
- [Outgoing Commits](../outgoing/spec.md): the overview whose footer shows the update state.
