---
name: Squash Commits
description: Requirements for squashing selected commits of the current branch into the oldest of them, with an editable message and an undo, without touching the working tree.
style: plain-1
targets:
  - crates/git-ops/src/squash_commits.rs
  - crates/git-ops/src/rewrite_plan.rs
  - crates/git-ops/src/replay.rs
  - crates/history-rewrite/src/squash.rs
  - crates/git-ops/src/history.rs
  - crates/bd-rewrite/src/squash_command.rs
  - crates/history-rewrite/src/repository_lock.rs
  - crates/operations/src/rewrite.rs
  - src/lib/rewrite-run.ts
  - src/hooks/useSquashCommits.ts
  - src/hooks/useCommitMoveList.ts
  - src/components/commit/MovableCommitList.tsx
  - src/components/commit/SquashCommitsDialog.tsx
  - src/lib/squash-message.ts
  - src/presenters/rewrite.ts
---

# Squash Commits

Status: Implemented
Date: 2026-10-14

## Purpose

The user selects two or more commits of one list, a branch or the unassigned commits, and squashes them into one; an
agent squashes the commits the user named with `brd squash` ([Agent CLI](../../cli/agent-cli/spec.md)). The
squashed commit takes the place of the oldest selected commit, as `git rebase --autosquash` does with a fixup: the
newer commits are folded into it and leave the history, and the commits in between stay where they are. The message is
edited in a dialog, prefilled with the messages of all the commits. Squashing keeps the files of HEAD, so the working
tree and the index are not touched, and an undo is a move of the branch back.

## Goals

- Combine work-in-progress commits before a push without a terminal rebase.
- Squash commits that are not adjacent, for example a fix made after a commit of another branch.
- Undo a squash.

## Non-goals

- Squashing `fixup!`, `squash!` and `amend!` commits into their targets automatically ([Squash Fixups](../fixups/spec.md)).
- Reordering commits or resolving conflicts between the squashed commits and the commits in between.
- Squashing commits of different lists in one step.

## Requirements

### Requirement: Rewrite: The commits are squashed into the oldest selected commit

The commits MUST be squashed into the oldest selected commit, whatever the order of the selection. It MUST keep its
place in the history and its author; the committer becomes the current user.

#### Scenario: A selection newest first
- **WHEN** the commits are selected newer first
- **THEN** they are squashed into the oldest selected commit
- **AND** it keeps its author, and the committer becomes the current user
  [@test] crates/history-rewrite/src/squash_test.rs (`squashes_into_the_oldest_selected_commit_whatever_the_order`)

#### Scenario: The squashed commit keeps its place and its author
- **WHEN** adjacent commits are squashed
- **THEN** the squashed commit takes the place of the oldest one in the history
- **AND** it keeps the author of the oldest one
  [@test] crates/git-ops/src/squash_commits_test.rs (`squashes_adjacent_commits_into_the_oldest`)

### Requirement: Rewrite: The newer commits apply oldest first and the ones in between stay

The changes of the newer commits MUST be applied to it oldest first. The commits in between MUST stay, with their
messages.

#### Scenario: Adjacent commits
- **WHEN** the selected commits are adjacent
- **THEN** the changes of the newer ones are applied to the oldest
  [@test] crates/git-ops/src/squash_commits_test.rs (`squashes_adjacent_commits_into_the_oldest`)

#### Scenario: A commit of another branch in between
- **GIVEN** a commit of another branch between the selected commits
- **WHEN** they are squashed
- **THEN** the commit in between stays, with its message
  [@test] crates/history-rewrite/src/squash_test.rs (`squashes_into_the_oldest_selected_commit_whatever_the_order`)
  [@test] crates/git-ops/src/squash_commits_test.rs (`keeps_the_commit_in_between_of_another_branch`)

#### Scenario: HEAD into an older commit
- **WHEN** HEAD is squashed into an older commit
- **THEN** its changes are applied to that commit and the commits in between stay
  [@test] crates/git-ops/src/squash_commits_test.rs (`squashes_head_into_an_older_commit`)

#### Scenario: Several groups in one rewrite
- **WHEN** several groups are squashed in one rewrite
- **THEN** the changes of each group's newer commits are applied to its oldest
  [@test] crates/git-ops/src/squash_commits_test.rs (`squashes_several_groups_in_one_rewrite`)

### Requirement: Rewrite: The tree of HEAD and the working tree stay the same

The tree of HEAD MUST stay the same. The working tree, the index and the untracked files MUST stay byte for byte.

#### Scenario: Dirty and staged changes
- **GIVEN** dirty, staged and untracked changes
- **WHEN** the squash runs
- **THEN** the tree of HEAD stays the same
- **AND** the working tree, the index and the untracked files stay byte for byte
  [@test] crates/git-ops/src/squash_commits_test.rs (`keeps_dirty_and_staged_changes_byte_for_byte`)

### Requirement: Rewrite: The branch moves in one checked transaction

The branch MUST move in one transaction with an old-value check and the reflog message
`branch-deck: squash N commits`.

#### Scenario: The reflog message
- **WHEN** the squash moves the branch
- **THEN** the reflog says `branch-deck: squash N commits`
  [@test] crates/git-ops/src/squash_commits_test.rs (`records_the_reflog_message`)

#### Scenario: The branch moved after the prepare
- **WHEN** the branch moved after the squash was prepared
- **THEN** the old-value check fails and nothing changes
  [@test] crates/git-ops/src/squash_commits_test.rs (`branch_moved_after_prepare_changes_nothing`)

### Requirement: Rewrite: The squash is refused where it cannot rewrite, changing nothing

The squash MUST be refused, with no ref, index entry or file changed and no temporary ref left, for a selection or a
message it cannot take, a commit off the first-parent line of HEAD, a conflict with a commit in between, a result
without changes or a later commit that would become empty, a repository in a state no rewrite runs in, and a branch
that moved meanwhile.

#### Scenario: A selection or a message it cannot take
- **WHEN** fewer than two commits are given, a commit is given twice, or the message has no subject line
- **THEN** the squash is refused and nothing changes
  [@test] crates/git-ops/src/squash_commits_test.rs (`validates_the_groups`)
  [@test] crates/history-rewrite/src/squash_test.rs (`refuses_fewer_than_two_commits_a_duplicate_or_a_blank_subject`)

#### Scenario: A commit off the first-parent line of HEAD
- **WHEN** a commit is not on the first-parent line of HEAD
- **THEN** the squash is refused and nothing changes
  [@test] crates/git-ops/src/squash_commits_test.rs (`validates_the_groups`)
  [@test] crates/history-rewrite/src/squash_test.rs (`refuses_fewer_than_two_commits_a_duplicate_or_a_blank_subject`)

#### Scenario: A conflict with a commit in between
- **WHEN** a squashed commit conflicts with a commit in between
- **THEN** the squash is refused, nothing changes and no temporary ref is left
  [@test] crates/git-ops/src/rewrite_plan_test.rs (`squash_conflicting_with_commit_in_between_changes_nothing`, `refusal_after_writing_a_commit_leaves_no_temp_refs`)

#### Scenario: A result without changes
- **WHEN** the result has no changes, as the commits cancel each other
- **THEN** the squash is refused, "drop them instead": a squash gives one commit, so they are dropped instead
  [@test] crates/git-ops/src/rewrite_plan_test.rs (`squash_refuses_empty_result`)

#### Scenario: A later commit would become empty
- **WHEN** a later commit would become empty
- **THEN** the squash is refused and nothing changes
  [@test] crates/git-ops/src/rewrite_plan_test.rs (`squash_refuses_kept_commit_that_becomes_empty`)

#### Scenario: A state no rewrite runs in
- **WHEN** a rebase, merge, cherry-pick or revert is in progress, HEAD is detached, or the range contains a merge commit
- **THEN** the squash is refused and nothing changes
  [@test] crates/git-ops/src/squash_commits_test.rs (`refuses_a_range_with_merges`)

#### Scenario: The branch moved meanwhile
- **WHEN** the branch moved meanwhile
- **THEN** the squash is refused and nothing changes
  [@test] crates/git-ops/src/squash_commits_test.rs (`branch_moved_after_prepare_changes_nothing`)

### Requirement: Rewrite: A conflict and an empty result are refused by their own codes

A conflict MUST be refused as `squash_conflict`, with the commits it names and the files of a conflicting merge, and a
result without changes or a later commit left empty as `squash_empty`, in the app (`ApiError.code`, the `conflict`
details) and in `brd`; the refusal keeps its type also under the error of a core.

#### Scenario: A conflict
- **WHEN** a squashed commit conflicts with a commit in between
- **THEN** the refusal is `squash_conflict`, with the commits it names and the files of a conflicting merge
  [@test] crates/git-ops/src/rewrite_plan_test.rs (`squash_conflicting_with_commit_in_between_changes_nothing`)

#### Scenario: A result without changes
- **WHEN** the result has no changes, or a later commit is left empty
- **THEN** the refusal is `squash_empty`
  [@test] crates/git-ops/src/rewrite_plan_test.rs (`squash_refuses_empty_result`)

#### Scenario: The app answers the refusal by its kind
- **WHEN** a refused squash reaches the app under the error of a core
- **THEN** its `ApiError.code` is the code of the refusal, with the `conflict` details
  [@test] crates/app-api/src/api_error_test.rs (`api_error_answers_a_refused_squash_by_its_kind`)

### Requirement: Rewrite: A replayed commit whose trees stay keeps its mapping note

A replayed commit whose tree and parent tree stay the ones its mapping note records MUST keep that note, written before
the branch moves, so the next sync keeps its virtual copy; the squashed commit and the commits whose trees change get
none, and the original commits keep theirs. A history read without notes carries none.

#### Scenario: The replayed commits whose trees stay
- **WHEN** the squash replays commits whose tree and parent tree stay
- **THEN** they keep their mapping notes, written before the branch moves
- **AND** the squashed commit and the commits whose trees change get none
  [@test] crates/git-ops/src/squash_commits_test.rs (`the_replayed_commits_whose_trees_stay_keep_their_mapping_notes`)

#### Scenario: A history without notes
- **WHEN** the history is read without notes
- **THEN** the squash writes none
  [@test] crates/git-ops/src/squash_commits_test.rs (`a_squash_on_a_history_without_notes_writes_none`)

### Requirement: Rewrite: The squash and its undo are rewrites of the engine under the lock

The squash and its undo MUST be the rewrites `squash` and `undo` of the engine (the squash named on the table the
window shows) and run under the repository lock.

#### Scenario: A squash and its undo take the lock
- **WHEN** the squash or its undo runs
- **THEN** it runs as the rewrite `squash` or `undo` of the engine, under the repository lock
  [@test] crates/sync-core/src/repository_lock_test.rs (`a_rewrite_waits_for_the_holder`)

### Requirement: Rewrite: The squash waits while a named holder lives

While another process holds the lock (an agent's `brd pick --follow`, `brd`) or the app's own push, the squash and its
undo MUST wait as long as the holder lives; the squash dialog says whom for, the step of its operation, with Stop
Waiting (`cancel_operation`), which ends the wait with nothing changed, the dialog kept with its message for the next
press.

#### Scenario: Another process holds the lock
- **WHEN** another process holds the repository lock
- **THEN** the squash and its undo wait as long as the holder lives, and run once it released the lock
  [@test] crates/sync-core/src/repository_lock_test.rs (`a_rewrite_waits_for_the_holder`)
  [@test] crates/sync-core/src/repository_lock_test.rs (`a_rewrite_of_the_app_waits_while_a_live_holder_holds_the_lock_and_runs_after`)
  [@test] crates/operations/src/rewrite_test.rs (`a_rewrite_waits_while_the_holder_of_the_lock_lives_and_stops_when_told`)

#### Scenario: The dialog says whom for
- **WHEN** the squash waits for the holder
- **THEN** the squash dialog says whom for, the step of its operation: "Waiting for `brd pick` (pid 4242, for 12 s)"
- **AND** it squashes once the holder released the lock
  [@test] crates/sync-core/src/repository_lock_test.rs (`every_rewrite_fails_with_locked_while_another_holds_the_lock`)
  [@test] tests/e2e/repository/repository-lock.spec.ts (`should wait for the holder in the squash dialog, say whom for, and squash once it released the lock`)

#### Scenario: Stop Waiting
- **WHEN** the user presses Stop Waiting (`cancel_operation`) in the squash dialog
- **THEN** the wait ends with nothing changed: "Canceled squashing 2 commits before it started"
- **AND** the dialog is kept with its message, and the next press squashes
  [@test] crates/sync-core/src/repository_lock_test.rs (`a_rewrite_of_the_app_ends_its_wait_when_stopped_and_refuses_an_unnamed_holder`)
  [@test] tests/e2e/repository/repository-lock.spec.ts (`should stop the wait of a squash with Stop Waiting in its dialog, squash nothing, and squash on the next press`)
  [@test] crates/operations/src/rewrite_test.rs (`a_rewrite_waits_while_the_holder_of_the_lock_lives_and_stops_when_told`)

### Requirement: Rewrite: Only a holder no record names refuses the squash after 10 s

Only a holder no record names MUST refuse the squash and its undo, after 10 s, with nothing changed and the error toast
saying that another operation holds the lock.

#### Scenario: An unnamed holder
- **GIVEN** a holder of the lock that no record names
- **WHEN** the squash or its undo waits for it
- **THEN** it is refused after 10 s, with nothing changed
- **AND** the error toast says that another operation holds the lock
  [@test] crates/sync-core/src/repository_lock_test.rs (`a_rewrite_of_the_app_ends_its_wait_when_stopped_and_refuses_an_unnamed_holder`)

### Requirement: Rewrite: A HEAD the request was not made for refuses the squash

A HEAD the request was not made for (`expect_head`) MUST refuse the squash as `HeadMoved`.

#### Scenario: Another HEAD than the expected one
- **WHEN** HEAD is not the one the request was made for (`expect_head`)
- **THEN** the squash is refused as `HeadMoved`
  [@test] crates/sync-core/src/repository_lock_test.rs (`every_rewrite_refuses_another_head_than_the_expected_one`)

### Requirement: Rewrite: The squash computes before the lock and holds it only for the write

The squash and its undo MUST compute before the repository lock (the history, the merges and the replays, the squashed
commits on temporary refs; the checks of the undo) and hold it only for the witness of HEAD and the compare-and-swap
move of the branch.

#### Scenario: Only the witness and the write under the lock
- **WHEN** the squash or its undo runs
- **THEN** the history, the merges, the replays and the checks run before the lock
- **AND** only the witness of HEAD and the compare-and-swap move of the branch run under it
  [@test] crates/history-rewrite/src/squash_test.rs (`a_squash_locks_only_the_witness_and_the_write`)

### Requirement: Rewrite: A HEAD that moved before the lock computes the squash again

A HEAD that moved before the lock MUST compute the squash once more under it, where a HEAD the request was not made
for refuses it as `HeadMoved`; so does a HEAD that moved between two reads before the lock (the history and the guard).

#### Scenario: HEAD moved before the lock
- **WHEN** HEAD moved before the lock
- **THEN** the squash is computed once more under the lock
- **AND** a HEAD the request was not made for refuses it as `HeadMoved`

#### Scenario: HEAD moved between two reads before the lock
- **WHEN** HEAD moved between two reads before the lock (the history and the guard)
- **THEN** the squash is computed once more under the lock
  [@test] crates/sync-core/src/repository_lock_test.rs (`a_head_moved_between_two_reads_of_a_rewrite_computes_it_once_more_under_the_lock`)

### Requirement: Rewrite: An operation of the app itself never refuses the squash

An operation of the app itself MUST NOT refuse the squash: behind the app's own push or rewrite it waits in the line of
the app's process, as long as that one takes and past any bound, and the operations of the line get the lock in the
order they came; a wait stopped in the line leaves it, nothing changed.

#### Scenario: Behind a push of its own process
- **WHEN** the squash comes behind a push of the app's own process
- **THEN** it waits in the line of the app's process, past any bound, and is never refused as locked
  [@test] crates/history-rewrite/src/repository_lock_test.rs (`a_rewrite_behind_a_short_push_of_its_own_process_waits_in_its_line_and_is_never_locked`)

#### Scenario: The order of the line and a stopped wait
- **WHEN** several operations of the app wait in the line
- **THEN** they get the lock in the order they came
- **AND** a wait stopped in the line leaves it, nothing changed
  [@test] crates/history-rewrite/src/repository_lock_test.rs (`the_line_gives_the_turn_in_the_order_they_came_and_a_stopped_wait_leaves_it`)

### Requirement: Rewrite: A group that keeps the target's message keeps its bytes

A group that keeps the target's message (for fixups) MUST keep its bytes and its encoding.

#### Scenario: The target's message in another encoding
- **WHEN** a group keeps the target's message (for fixups)
- **THEN** the message keeps its bytes and its encoding
  [@test] crates/git-ops/src/squash_commits_test.rs (`target_message_keeps_the_bytes_and_the_encoding`)

### Requirement: Starting a squash: The selection actions show a Squash button

With two or more commits selected, the selection actions MUST show a ghost "Squash" button.

#### Scenario: Two commits selected
- **WHEN** two or more commits are selected
- **THEN** the selection actions show a ghost "Squash" button
  [@test] tests/e2e/squash-commits/squash-commits.spec.ts (`should squash selected branch commits into the oldest one and undo it`)

### Requirement: Starting a squash: The context menu offers Squash N Commits

The context menu of a selected row MUST offer "Squash N Commits…" with the key `S` when two or more commits are
selected, and MUST NOT offer it for a single commit.

#### Scenario: Two or more commits and a single one
- **WHEN** the context menu of a selected row opens with two or more commits selected
- **THEN** it offers "Squash N Commits…" with the key `S`
- **AND** for a single commit it offers no squash
  [@test] tests/e2e/squash-commits/squash-commits.spec.ts (`should offer squash in the context menu of two or more commits and cancel with Escape`)

### Requirement: Starting a squash: S opens the dialog in a branch and in the unassigned commits

`S` in a list with two or more selected commits MUST open the dialog, in a branch and in the unassigned commits.

#### Scenario: S in the unassigned commits
- **GIVEN** two or more selected commits in the unassigned commits
- **WHEN** the user presses `S`
- **THEN** the squash dialog opens
  [@test] tests/e2e/squash-commits/squash-commits.spec.ts (`should squash unassigned commits with the S key`)

#### Scenario: S in a branch
- **GIVEN** two or more selected commits of a branch
- **WHEN** the user presses `S`
- **THEN** the squash dialog opens

### Requirement: Dialog: The header says what is squashed

The header MUST say "Squash N commits", name the target and list the commits with their short hashes.

#### Scenario: The header of the dialog
- **WHEN** the squash dialog opens
- **THEN** its header says "Squash N commits" and names the target
- **AND** it lists the commits with their short hashes
  [@test] tests/e2e/squash-commits/squash-commits.spec.ts (`should squash selected branch commits into the oldest one and undo it`)

### Requirement: Dialog: The message is prefilled with the messages of the commits

The message MUST be prefilled with the oldest commit's message, then the message of each newer commit as a paragraph,
without its autosquash markers and `(prefix)`, or without the issue key it starts with if it is grouped by that key. A
message already included MUST be left out.

#### Scenario: The newer messages as paragraphs without their prefix
- **WHEN** the dialog opens
- **THEN** the message is the oldest commit's message, then the message of each newer commit as a paragraph, without
  its `(prefix)`
  [@test] src/lib/squash-message.test.ts (`keeps the oldest message and appends the others as paragraphs without their prefix`, `cleans up trailing whitespace and blank lines`)

#### Scenario: An autosquash marker and a message already included
- **WHEN** a newer message carries an autosquash marker, or is already included (e.g. `fixup! X` after `X`)
- **THEN** the marker is removed, and a message already included is left out
  [@test] src/lib/squash-message.test.ts (`removes the autosquash marker and leaves out a message already included`)

#### Scenario: The issue key a newer subject is grouped by
- **WHEN** a newer subject starts with the issue key it is grouped by
- **THEN** the key is removed (`ABC-1: Add tokens` becomes `Add tokens`)
  [@test] src/lib/squash-message.test.ts (`removes the issue key a newer subject is grouped by`)

#### Scenario: A newer subject grouped by its prefix
- **WHEN** a newer subject is grouped by its prefix and starts with an issue key
- **THEN** the prefix is removed and the issue key kept
  [@test] src/lib/squash-message.test.ts (`keeps the issue key of a newer subject grouped by its prefix`)

### Requirement: Dialog: The footer warns of a move to another branch

The footer MUST warn when the edited subject moves the squashed commit to another branch.

#### Scenario: A new prefix
- **WHEN** the edited subject moves the squashed commit to another branch
- **THEN** the footer warns of it
  [@test] tests/e2e/squash-commits/squash-commits.spec.ts (`should offer squash in the context menu of two or more commits and cancel with Escape`)

### Requirement: Dialog: The footer warns of pushed commits

The footer MUST warn "N squashed commits are already pushed; the next push replaces them" when the branch exists on
the remote and a selected commit is not ahead of it.

#### Scenario: Selected commits not ahead of the remote branch
- **GIVEN** the branch exists on the remote
- **WHEN** a selected commit is not ahead of it
- **THEN** the footer warns "N squashed commits are already pushed; the next push replaces them"
  [@test] src/lib/squash-message.test.ts (`counts the synced commits that are not ahead of the remote branch`)

#### Scenario: No remote branch or commits not synced yet
- **WHEN** the branch has no remote branch, or the commits are not synced yet
- **THEN** the footer warns of nothing pushed
  [@test] src/lib/squash-message.test.ts (`counts nothing without a remote branch or for commits not synced yet`)

### Requirement: Dialog: The keys squash and cancel, a click outside keeps the dialog

⌘↵ (Ctrl+Enter) or the Squash button MUST squash; Escape MUST cancel. A click outside MUST keep the dialog and the
edited message.

#### Scenario: ⌘↵ squashes
- **WHEN** the user presses ⌘↵ (Ctrl+Enter) in the dialog
- **THEN** the commits are squashed
  [@test] tests/e2e/squash-commits/squash-commits.spec.ts (`should squash selected branch commits into the oldest one and undo it`)

#### Scenario: The Squash button squashes
- **WHEN** the user presses the Squash button
- **THEN** the commits are squashed
  [@test] tests/e2e/squash-commits/squash-commits.spec.ts (`should squash unassigned commits with the S key`)

#### Scenario: Escape cancels
- **WHEN** the user presses Escape in the dialog
- **THEN** the dialog closes and nothing is squashed
  [@test] tests/e2e/squash-commits/squash-commits.spec.ts (`should offer squash in the context menu of two or more commits and cancel with Escape`)

#### Scenario: A click outside
- **WHEN** the user clicks outside the dialog
- **THEN** the dialog stays with the edited message

### Requirement: After a squash: The selection clears and the toast offers Undo

A squash MUST clear the selection, sync, and show the toast "Squashed N commits" with an Undo button.

#### Scenario: The toast of a squash
- **WHEN** a squash succeeds
- **THEN** the selection is cleared and the branch synced
- **AND** the toast "Squashed N commits" shows an Undo button
  [@test] tests/e2e/squash-commits/squash-commits.spec.ts (`should squash selected branch commits into the oldest one and undo it`)

### Requirement: After a squash: Undo moves the branch back while it holds the squash

Undo (the rewrite `undo` of the `squash` token of its outcome, `<previous>..<new>`) MUST move the branch back to the
commit before the squash in one transaction (`branch-deck: undo squash`), only if the branch still points at the
squashed history and both have the same files; otherwise it is refused.

#### Scenario: Undo right after the squash
- **WHEN** the user presses Undo while the branch still points at the squashed history
- **THEN** the branch moves back to the commit before the squash in one transaction (`branch-deck: undo squash`)
  [@test] crates/git-ops/src/squash_commits_test.rs (`undo_restores_the_previous_head`)
  [@test] crates/history-rewrite/src/squash_test.rs (`undo_restores_the_previous_head_and_refuses_after_a_new_commit`)
  [@test] tests/e2e/squash-commits/squash-commits.spec.ts (`should squash selected branch commits into the oldest one and undo it`)

#### Scenario: The branch no longer points at the squashed history
- **WHEN** the branch no longer points at the squashed history, or the two have other files
- **THEN** the undo is refused
  [@test] crates/git-ops/src/squash_commits_test.rs (`undo_is_refused_when_the_branch_moved`)

### Requirement: After a squash: An undo after the branch moved on is undo_stale

A branch that moved on since (a commit on top, also one made between the check and the move) MUST be refused with
`undo_stale` (`retry: no`), as every undo of the app and `brd undo` answer it, never `internal`, in words that say what
moved and that it can't be undone any more, never the advice to retry.

#### Scenario: A commit on top
- **WHEN** a commit was made on top since the squash
- **THEN** the undo is refused with `undo_stale` (`retry: no`), never `internal`
  [@test] crates/git-ops/src/squash_commits_test.rs (`undo_is_refused_when_the_branch_moved`)
  [@test] crates/history-rewrite/src/squash_test.rs (`undo_restores_the_previous_head_and_refuses_after_a_new_commit`)
  [@test] crates/test-server/src/squash_test.rs (`the_undo_of_a_squash_after_a_new_commit_answers_undo_stale_never_retried`)

#### Scenario: A commit between the check and the move
- **WHEN** a commit is made between the check of the undo and the move
- **THEN** the undo is refused with `undo_stale`
  [@test] crates/git-ops/src/squash_commits_test.rs (`undo_is_refused_when_the_branch_moved_during_the_transaction`)

#### Scenario: The words of the refusal
- **WHEN** a stale undo is refused
- **THEN** its words say what moved and that it can't be undone any more, never the advice to retry
  [@test] crates/app-api/src/api_error_test.rs (`a_stale_undo_says_it_cannot_be_undone_instead_of_advising_a_retry`)
  [@test] tests/e2e/squash-commits/squash-commits.spec.ts (`should refuse to undo a squash after a commit on top`)

### Requirement: After a squash: A failed transaction while the branch stays is no undo_stale

A transaction that fails while the branch stays (a ref that moves with it is elsewhere) MUST NOT be `undo_stale`.

#### Scenario: Another ref of the transaction moved
- **WHEN** the transaction of the undo fails because a ref that moves with the branch is elsewhere
- **THEN** the branch stays, and the refusal is no `undo_stale`
  [@test] crates/git-ops/src/squash_commits_test.rs (`undo_whose_other_ref_moved_is_not_head_moved`)

## User Experience

- The Squash button is a ghost `xs` button with the `Combine` icon, next to "Move to…".
- The dialog uses the preview dialog frame: the title and the commits above a rule, the message editor (monospace, as
  the commit preview edits a message), and a footer with the hints on the left and Esc Cancel, Squash ⌘↵ on the right.
- The pushed warning is `text-warning` with a triangle icon.

## Testing / Local Run

- `pnpm test:backend`: `squash_commits_test.rs`, `rewrite_plan_test.rs`, `squash_test.rs`.
- `pnpm test:unit`: `src/lib/squash-message.test.ts`.
- `pnpm e2e`: `tests/e2e/squash-commits` uses the `issue_split` and `unassigned` templates.

## Open Questions / Risks

- The undo uses the head ids of the squash. A commit made in a terminal between the squash and the undo makes the undo
  fail with an error toast (`undo_stale`).
- Squashing commits that are already pushed makes the next push replace them on the remote branch; the dialog warns.

## References

- [Git Operations](../../../../docs/design/git-operations.md)
- [Squash Fixups](../fixups/spec.md)
- [Move Commits](../../branch/move-commits/spec.md)
- [Commit Row](../commit-row/spec.md)
