---
name: Amend Changes
description: Requirements for amending selected uncommitted changes into a commit of the current branch without touching the working tree or the user's staged work.
style: plain-1
targets:
  - crates/git-ops/src/amend_operations.rs
  - crates/git-ops/src/replay.rs
  - crates/git-ops/src/rewrite_guard.rs
  - crates/sync-core/src/amend_to_branch.rs
  - crates/history-rewrite/src/repository_lock.rs
  - crates/operations/src/rewrite.rs
  - src/hooks/useAmendChanges.ts
  - src/components/branch/InlineAmendChangesInput.tsx
  - src/components/branch/UncommittedChangesCard.tsx
  - src/components/conflict/SubjectFileDiffList.tsx
  - src/presenters/rewrite.ts
  - crates/sync-core/src/uncommitted_changes.rs
  - crates/git-ops/src/index_lock.rs
---

# Amend Changes

Status: Draft
Date: 2026-10-14

## Purpose

The user selects files from the uncommitted changes and amends them into a commit of the current branch. The target
can be HEAD, an older commit or the root commit. Branch Deck rewrites the branch on objects and refs only. The working
tree is never written, and the user's other staged work stays staged.

## Goals

- Put exactly the selected changes into the target commit, whatever commits follow it.
- Keep the working tree byte for byte, and keep the staged changes that the amend does not touch.
- Fail without any change when the amend cannot be done cleanly.

## Non-goals

- Amending into a range that contains merge commits.
- Resolving conflicts between the amended change and later commits.

## Requirements

### Requirement: Content: The amend takes only the selected files

The amend MUST take only the selected files from the working tree. Other modified files MUST stay uncommitted.

#### Scenario: Two modified files, one selected
- **WHEN** the user amends one of two modified files into a commit
- **THEN** the commit takes the selected file from the working tree
- **AND** the other modified file stays uncommitted
  [@test] crates/git-ops/src/amend_operations_test.rs (`amend_only_selected_files`)

### Requirement: Content: An older commit gets only the working-tree delta

Amending a commit other than HEAD MUST apply only the working-tree delta relative to HEAD. Changes that later commits
made to the same file MUST NOT move into the target commit.

#### Scenario: A later commit changed the same file
- **GIVEN** a later commit changed another part of the file
- **WHEN** the user amends the file into the older commit
- **THEN** the target commit gets only the working-tree delta relative to HEAD
- **AND** the change of the later commit does not move into it
  [@test] crates/git-ops/src/amend_operations_test.rs (`amend_non_head_applies_only_delta`)

### Requirement: Content: The tip tree is HEAD's tree plus the selected files

After the amend, the tip tree MUST equal HEAD's tree plus the selected files.

#### Scenario: The tip after the amend
- **WHEN** the amend ends
- **THEN** the tree of the tip equals HEAD's tree plus the selected files
  [@test] crates/git-ops/src/amend_operations_test.rs (`amend_non_head_applies_only_delta`)

### Requirement: Content: The amended commit keeps its author and its message

The amended commit MUST keep its author and its message. Its committer and the committer of the commits after it
become the current user.

#### Scenario: The metadata of the amended commit
- **WHEN** the amend rewrites a commit
- **THEN** the amended commit keeps its author and its message
- **AND** its committer and the committer of the commits after it are the current user
  [@test] crates/git-ops/src/amend_operations_test.rs (`amend_keeps_the_author_and_message_and_records_the_reflog`)

### Requirement: The diffs of the uncommitted changes: The card shows the engine's diff of each file

The Uncommitted Changes card MUST show the diff of each changed file as the diff engine computes it, the working tree
against HEAD under the whitespace policy, with the engine's `+N −M` in its header.

#### Scenario: A changed file in the card
- **WHEN** the card shows a changed file
- **THEN** its diff is the working tree against HEAD under the whitespace policy, as the diff engine computes it
- **AND** its header has the engine's `+N −M`

### Requirement: The diffs of the uncommitted changes: A new answer reads the diffs again

A new answer of the uncommitted changes MUST read the diffs again; the shown ones MUST stay meanwhile.

#### Scenario: A refresh of the uncommitted changes
- **WHEN** a new answer of the uncommitted changes comes
- **THEN** the diffs are read again
- **AND** the shown ones stay until the new read ends
  [@test] src/stores/file-diffs.test.ts (`reads the uncommitted changes again on a refresh, and keeps the loaded ones until the new read ends`)

### Requirement: The diffs of the uncommitted changes: A new answer during a read is never dropped

A new answer during a read MUST read the diffs once more after it, never dropped, however many came.

#### Scenario: Answers that come during a read
- **WHEN** new answers of the uncommitted changes come while the diffs are read
- **THEN** the diffs are read once more after that read, however many came
  [@test] src/stores/file-diffs.test.ts (`reads the uncommitted changes once more after a refresh asked during a read, never dropping it`)

### Requirement: The diffs of the uncommitted changes: The diffs are of their repository's working tree

The diffs MUST be of the working tree of their repository: those of another repository are another entry, never shown
for this one.

#### Scenario: Two repositories
- **WHEN** the uncommitted changes of two repositories are read
- **THEN** each is read from its own working tree, another entry
- **AND** those of the other repository are never shown for this one
  [@test] src/stores/file-diffs.test.ts (`keeps the uncommitted changes of two repositories apart, each read from its own working tree`)

### Requirement: Working tree and index: The amend does not write to the working tree

The amend MUST NOT write to the working tree.

#### Scenario: Files of the working tree during an amend
- **WHEN** the amend runs
- **THEN** the working tree is not written, an uncommitted file stays as it was
  [@test] crates/git-ops/src/amend_operations_test.rs (`test_working_directory_preserved`)

### Requirement: Working tree and index: Staged changes the amend does not touch stay staged

Staged changes on paths that the amend does not touch MUST stay staged.

#### Scenario: A staged change on another path
- **GIVEN** a staged change on a path that the amend does not touch
- **WHEN** the amend runs
- **THEN** that change stays staged
  [@test] crates/git-ops/src/amend_operations_test.rs (`amend_keeps_unrelated_staged_changes`)

### Requirement: Working tree and index: The amended files show as unmodified afterwards

The amended files MUST show as unmodified afterwards: their working-tree versions are staged, like
`git commit -- <paths>` does, replacing a partly staged version.

#### Scenario: The status after the amend
- **WHEN** the amend ends
- **THEN** the amended files show as unmodified, their working-tree versions staged like `git commit -- <paths>` does
- **AND** a partly staged version is replaced
  [@test] crates/git-ops/src/amend_operations_test.rs (`amend_keeps_unrelated_staged_changes`)

### Requirement: Working tree and index: An index that cannot follow answers it, the amend runs once

An amend whose index cannot follow the branch it moved MUST answer that failure, with the advice to run `git reset -q`:
the branch was rewritten once, and the amend never runs again on the HEAD it wrote itself.

#### Scenario: The index cannot follow the moved branch
- **WHEN** the index cannot follow the branch the amend moved (a file unreadable since the amend was computed, the
  index lock of another process past its wait)
- **THEN** the amend answers that failure, with the advice to run `git reset -q`
- **AND** the branch was rewritten once, and the amend never runs again on the HEAD it wrote itself
  [@test] crates/operations/src/rewrite_test.rs (`an_amend_whose_index_cannot_follow_the_branch_it_moved_answers_that_and_runs_once`)

### Requirement: Refusals: The amend is refused and changes nothing in these cases

The amend MUST be refused, and change nothing, when a rebase, merge, cherry-pick or revert is in progress, when HEAD
is detached, when the range from the target to HEAD contains a merge commit, or when the branch moved meanwhile.

#### Scenario: An operation of git in progress
- **WHEN** a rebase, merge, cherry-pick or revert is in progress
- **THEN** the amend is refused and nothing changes
  [@test] crates/git-ops/src/rewrite_plan_test.rs (`rewrite_refused_during_merge`)

#### Scenario: HEAD is detached
- **WHEN** HEAD is detached
- **THEN** the amend is refused and nothing changes

#### Scenario: A merge commit in the range
- **WHEN** the range from the target to HEAD contains a merge commit
- **THEN** the amend is refused and nothing changes
  [@test] crates/git-ops/src/amend_operations_test.rs (`amend_refuses_range_with_merges`)

#### Scenario: The branch moved meanwhile
- **WHEN** the branch moved meanwhile
- **THEN** the amend is refused and nothing changes

### Requirement: Refusals: The amend writes under the repository lock only what it must

The amend of the app is the rewrite `amend` of the engine, named on the table the window shows, and MUST write under
the repository lock: the history, the merge, the amended commit and the replay of the commits above it come before the
lock, and under it only the witness of HEAD, the move of the branch and the staging of the amended files (one
`update-index`, no refresh).

#### Scenario: What runs under the lock
- **WHEN** the amend of the app runs as the rewrite `amend` of the engine
- **THEN** the history, the merge, the amended commit and the replay of the commits above it come before the lock
- **AND** under it only the witness of HEAD, the move of the branch and the staging of the amended files (one
  `update-index`, no refresh)
  [@test] crates/sync-core/src/repository_lock_test.rs (`an_amend_and_an_issue_reference_lock_only_the_witness_and_the_write`)

### Requirement: Refusals: A HEAD another process moved is refused before the lookup

A HEAD another process moved meanwhile (`brd update`, a `brd` rewrite of a commit below, a commit on top) MUST be
refused (`head_moved`) before the amended commit is looked for on it, under the lock by one `rev-parse` before the
history is read again.

#### Scenario: Another process moved HEAD
- **WHEN** another process moved HEAD meanwhile (`brd update`, a `brd` rewrite of a commit below, a commit on top)
- **THEN** the amend is refused (`head_moved`) by one `rev-parse` under the lock, before the history is read again and
  the amended commit is looked for on it
  [@test] crates/sync-core/src/repository_lock_test.rs (`every_rewrite_refuses_another_head_than_the_expected_one`, `every_rewrite_refuses_another_head_that_has_none_of_its_commits`)

### Requirement: Refusals: The engine amends the copy of the commit on HEAD as it is now

After `head_moved`, the engine MUST resolve the amend again on HEAD as it is now and amend the copy of the commit
there, found by its author, author time and subject.

#### Scenario: Another process rewrote below the commit during the wait
- **WHEN** the amend is refused `head_moved` because another process rewrote a commit below it
- **THEN** the engine resolves the amend again on HEAD as it is now
- **AND** amends the copy of the commit there, found by its author, author time and subject
  [@test] crates/operations/src/rewrite_test.rs (`an_amend_waiting_for_the_lock_of_another_process_that_rewrites_below_its_commit_amends_the_copy`)

### Requirement: Refusals: The amend waits while a live holder holds the lock

While another process holds the lock (an agent's `brd pick --follow`, `brd`) or the app's own push, the amend MUST
wait as long as the holder lives, its inline form saying whom for, the step of its operation, with Stop Waiting
(`cancel_operation`), which ends the wait with nothing changed and the form kept.

#### Scenario: A live holder
- **WHEN** another process holds the lock (an agent's `brd pick --follow`, `brd`) or the app's own push does
- **THEN** the amend waits as long as the holder lives, then runs
- **AND** its inline form says whom for, the step of its operation ("Waiting for `brd pick` (pid 4242, for 12 s)")
  [@test] crates/sync-core/src/repository_lock_test.rs (`a_rewrite_waits_for_the_holder`, `a_rewrite_of_the_app_waits_while_a_live_holder_holds_the_lock_and_runs_after`)
  [@test] crates/operations/src/rewrite_test.rs (`a_rewrite_waits_while_the_holder_of_the_lock_lives_and_stops_when_told`)

#### Scenario: Stop Waiting
- **WHEN** the user chooses Stop Waiting (`cancel_operation`)
- **THEN** the wait ends with nothing changed and the form kept
  [@test] crates/sync-core/src/repository_lock_test.rs (`a_rewrite_of_the_app_ends_its_wait_when_stopped_and_refuses_an_unnamed_holder`)
  [@test] crates/operations/src/rewrite_test.rs (`a_rewrite_waits_while_the_holder_of_the_lock_lives_and_stops_when_told`)

### Requirement: Refusals: A holder no record names refuses the amend after 10 s

Only a holder no record names MUST refuse the amend after 10 s, with nothing changed and the error toast saying that
another operation holds the lock.

#### Scenario: A holder without a record
- **WHEN** a holder no record names holds the lock past 10 s
- **THEN** the amend is refused with nothing changed
- **AND** the error toast says that another operation holds the lock
  [@test] crates/sync-core/src/repository_lock_test.rs (`every_rewrite_fails_with_locked_while_another_holds_the_lock`, `a_rewrite_of_the_app_ends_its_wait_when_stopped_and_refuses_an_unnamed_holder`)

### Requirement: Refusals: A HEAD the request was not made for refuses the amend

A HEAD the request was not made for MUST refuse the amend too.

#### Scenario: Another HEAD than the request's
- **WHEN** HEAD is not the one the request was made for
- **THEN** the amend is refused
  [@test] crates/sync-core/src/repository_lock_test.rs (`every_rewrite_refuses_another_head_than_the_expected_one`)

### Requirement: Refusals: An error keeps the form, a conflict opens the conflict viewer

An error MUST keep the form; a conflict MUST open the conflict viewer below the row in its place.

#### Scenario: The amend fails
- **WHEN** the amend fails with an error
- **THEN** the form is kept

#### Scenario: The amend conflicts
- **WHEN** the amend fails with a conflict
- **THEN** the conflict viewer opens below the row in place of the form

### Requirement: Refusals: A conflict with a later commit changes nothing

A conflict between the amended change and a later commit MUST refuse the amend and change no ref, index entry or
file.

#### Scenario: A later commit changed the same lines
- **WHEN** the amended change conflicts with a later commit
- **THEN** the amend is refused
- **AND** no ref, index entry or file changes
  [@test] crates/git-ops/src/amend_operations_test.rs (`test_amend_conflict_changes_nothing`, `test_amend_is_refused_when_later_commits_changed_the_same_lines`)

## Testing / Local Run

- `pnpm test:backend`: `crates/git-ops/src/amend_operations_test.rs`.
- `pnpm e2e`: `tests/e2e/amend-changes` covers the form.

## Open Questions / Risks

- The index update after the branch moved waits for the index lock of another process (at most 30 s) and removes one
  that no process owns, as the moves do ([Index lock](../../../../docs/design/git-operations.md#index-lock),
  `the_index_of_a_rewrite_follows_past_a_lock_that_no_process_owns`). If it still fails, the error suggests
  `git reset -q` and is not retryable: the branch was rewritten already.

## References

- [Git Operations](../../../../docs/design/git-operations.md)
- [Commit Row](../commit-row/spec.md)
