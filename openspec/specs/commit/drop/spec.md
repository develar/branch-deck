---
name: Drop Commits
description: Requirements for dropping commits ahead of the baseline from the current branch for good, their files following, as the agent's way to take back its own commit instead of a revert that lands both into master; refused by a later commit that depends on a dropped one or would become empty and by local changes on the paths it writes, with an undo, also for a commit a lease already picked onto its home branch.
style: plain-1
targets:
  - crates/history-rewrite/src/drop_commits.rs
  - crates/git-ops/src/unapply.rs
  - crates/git-ops/src/rewrite_plan.rs
  - crates/git-ops/src/update_branch.rs
  - crates/domain-types/src/rewrite.rs
  - crates/domain-types/src/error.rs
  - crates/operations/src/rewrite.rs
  - crates/app-api/src/api_error.rs
  - crates/bd-update/src/drop_command.rs
  - crates/bd-update/src/undo_command.rs
  - crates/bd-lease/src/lease_command.rs
  - crates/bd-cli/src/lib.rs
  - crates/bd-cli/src/agent_help.rs
  - src/presenters/rewrite.ts
  - src/presenters/operations.ts
  - src/presenters/drop.ts
  - src/hooks/useDropCommits.ts
  - src/hooks/useCommitMoveList.ts
  - src/components/commit/InlineDropConfirm.tsx
  - src/components/commit/MovableCommitList.tsx
  - src/lib/review-actions.ts
---

# Drop Commits

Status: Implemented
Date: 2026-11-03

## Purpose

The work of a branch is done when it is in master, so a commit that must not land must leave the branch before it
lands, not be answered by a revert: a commit and its revert land into master together, two commits that change
nothing. A drop takes commits ahead of the baseline out of the current branch for good; the commits after them are
replayed, and the files follow, as an unapply's do ([Unapply and Reapply](../../branch/unapply/spec.md)), through the same
check and the same guarded move. A commit and its revert dropped together leave every file as it is. An agent drops its
own commit with `brd drop`; a commit its lease already picked onto the shared branch is dropped there.

## Goals

- Take back a commit that must not land, its files with it, without a revert in master.
- Clean up a commit and a revert of it that are both ahead of the baseline.
- Undo the drop, files included.

## Non-goals

- Dropping a commit on the baseline: it landed, a revert of it is the user's to decide.
- Recognizing a revert (`Revert "X"`, "This reverts commit …") by its message: a fold of it would rewrite X under its
  own message when the revert is partial, and the revert groups into no branch (its subject has no prefix). Both are
  named to the drop instead.
- Refusing a drop on a branch that is pushed or under a robot: no rewrite of the branch refuses that; the agent asks the
  user ([Agent CLI](../../cli/agent-cli/spec.md)), and the output names the branches the next sync builds anew.

## Requirements

### Requirement: Result: The named commits leave the current branch

The commits named MUST leave the current branch, and the commits after them MUST be replayed in order.

#### Scenario: A drop of a commit with later ones
- **WHEN** a drop of a commit that later commits follow runs
- **THEN** the commit leaves the current branch
- **AND** the later commits are replayed in order
  [@test] crates/git-ops/src/unapply_test.rs (`a_drop_takes_the_commits_and_their_files_out_of_head_and_its_undo_brings_them_back`)
  [@test] crates/history-rewrite/src/drop_commits_test.rs (`a_drop_takes_the_commits_and_their_files_out_and_names_what_the_next_sync_builds_anew`)
  [@test] crates/bd-update/src/drop_test.rs (`drop_takes_the_commit_and_its_file_out_of_head_and_the_undo_brings_them_back`)

### Requirement: Result: The files the commits changed follow them

The files the commits changed MUST follow: a file they added is gone, a file they changed is as the remaining commits
leave it, and `git status` shows nothing of them.

#### Scenario: Added files are gone, changed files are as the remaining commits leave them
- **WHEN** the drop runs
- **THEN** a file the commits added is gone
- **AND** a file they changed is as the remaining commits leave it
- **AND** `git status` shows nothing of them
  [@test] crates/git-ops/src/unapply_test.rs (`a_drop_takes_the_commits_and_their_files_out_of_head_and_its_undo_brings_them_back`)
  [@test] crates/history-rewrite/src/drop_commits_test.rs (`a_drop_takes_the_commits_and_their_files_out_and_names_what_the_next_sync_builds_anew`)
  [@test] crates/bd-update/src/drop_test.rs (`drop_takes_the_commit_and_its_file_out_of_head_and_the_undo_brings_them_back`)

### Requirement: Result: Work on other paths stays as it is

Work on the paths the drop does not write MUST stay as it is.

#### Scenario: A local change on another path
- **GIVEN** a local change on a path the drop does not write
- **WHEN** the drop runs
- **THEN** that change stays as it is
  [@test] crates/git-ops/src/unapply_test.rs (`a_drop_takes_the_commits_and_their_files_out_of_head_and_its_undo_brings_them_back`)

### Requirement: Result: The branch moves with the reflog message `branch-deck: drop`

The branch MUST move with the reflog message `branch-deck: drop`.

#### Scenario: The reflog of a drop
- **WHEN** a drop moves the branch
- **THEN** its reflog entry is `branch-deck: drop`
  [@test] crates/git-ops/src/unapply_test.rs (`a_drop_takes_the_commits_and_their_files_out_of_head_and_its_undo_brings_them_back`)

### Requirement: Result: A commit and its revert dropped together move no file

A commit and its revert dropped together MUST leave HEAD's tree and every file as they are (`files_changed: false`).

#### Scenario: A commit and its revert
- **WHEN** a commit and its revert are dropped together
- **THEN** HEAD's tree and every file stay as they are, and the result says `files_changed: false`
  [@test] crates/git-ops/src/unapply_test.rs (`a_drop_of_a_commit_and_its_revert_moves_no_file_and_a_later_commit_keeps_its_note`)
  [@test] crates/history-rewrite/src/drop_commits_test.rs (`a_commit_and_its_revert_drop_without_a_file_moving_and_empty_their_branch`)
  [@test] crates/bd-update/src/drop_test.rs (`drop_of_a_commit_and_its_revert_moves_no_file`)

### Requirement: Result: A later commit whose trees stay keeps its mapping note

A later commit whose trees stay MUST keep its mapping note, so the next sync keeps its virtual copy.

#### Scenario: A commit after a dropped pair
- **GIVEN** a commit after a commit and its revert
- **WHEN** both are dropped and the trees of the later commit stay
- **THEN** it keeps its mapping note, and the next sync keeps its virtual copy
  [@test] crates/git-ops/src/unapply_test.rs (`a_drop_of_a_commit_and_its_revert_moves_no_file_and_a_later_commit_keeps_its_note`)

### Requirement: Result: The result names what dropped and what the next sync builds

The result MUST name each dropped commit with its subject and its branch, whether the files changed, the branches the
next sync builds anew (those of the dropped commits and of the later commits whose files change: a pushed one needs a
push again) and the branches left without a commit, which the next sync archives.

#### Scenario: A drop whose later commits lose a file
- **WHEN** a drop of a commit runs whose file later commits of another branch lose
- **THEN** the result names the commit with its subject and its branch, that the files changed, and both branches as built anew
  [@test] crates/history-rewrite/src/drop_commits_test.rs (`a_drop_takes_the_commits_and_their_files_out_and_names_what_the_next_sync_builds_anew`)

#### Scenario: A drop that leaves a branch without a commit
- **WHEN** a commit and its revert drop and their branch keeps no commit
- **THEN** the result says that the files did not change, and names that branch as left without a commit
  [@test] crates/history-rewrite/src/drop_commits_test.rs (`a_commit_and_its_revert_drop_without_a_file_moving_and_empty_their_branch`)

### Requirement: Result: HEAD and the files move together

The move of HEAD and the files MUST happen together: when the files can't follow, HEAD moves back and nothing is
changed; a crash between the move and the files leaves the mark that the next move of the files finishes.

#### Scenario: The files can't follow
- **WHEN** the files can't follow
- **THEN** HEAD moves back and nothing is changed

#### Scenario: A crash between the move and the files
- **WHEN** a crash comes between the move and the files
- **THEN** it leaves the mark that the next move of the files finishes
  [@test] crates/git-ops/src/unapply_test.rs (`an_interrupted_drop_finishes_its_files`)

### Requirement: Refusals: The drop is refused where it cannot take the commits out

The drop MUST be refused, and change nothing, no temporary ref left, for an id that names no commit ahead of the
baseline, an ambiguous one or none, a later commit that depends on a dropped one or would become empty, the root
commit, a local change on a path the drop writes, a rebase, merge, cherry-pick or revert in progress, a detached HEAD,
a merge in the range, and another HEAD than the request was made for.

#### Scenario: An id that names no commit ahead of the baseline
- **WHEN** an id names no commit ahead of the baseline
- **THEN** the drop is refused with `not_ahead_of_baseline`, with `commits`, and nothing changes
  [@test] crates/history-rewrite/src/drop_commits_test.rs (`a_drop_refuses_commits_that_are_not_ahead_and_another_head`)
  [@test] crates/bd-update/src/drop_test.rs (`drop_refuses_what_is_not_ahead_a_dependent_later_commit_local_changes_and_another_head`)

#### Scenario: An ambiguous id or none
- **WHEN** an id is ambiguous, or no id is given
- **THEN** the drop is refused with `invalid_arguments`, and nothing changes
  [@test] crates/history-rewrite/src/drop_commits_test.rs (`a_drop_refuses_commits_that_are_not_ahead_and_another_head`)

#### Scenario: A later commit depends on a dropped one, would become empty, or the root
- **WHEN** a later commit depends on a dropped one (a conflict), or holds only changes of the dropped commits and would become empty ("drop it too"), or the root commit is named
- **THEN** the drop is refused with `drop_conflict`, and nothing changes
  [@test] crates/git-ops/src/unapply_test.rs (`a_drop_is_refused_by_a_later_commit_that_depends_on_it_would_become_empty_or_by_the_root`)
  [@test] crates/bd-update/src/drop_test.rs (`drop_refuses_what_is_not_ahead_a_dependent_later_commit_local_changes_and_another_head`)

#### Scenario: A local change on a path the drop writes
- **GIVEN** a local change, staged or not, on a path the drop writes
- **WHEN** the drop runs
- **THEN** it is refused with `uncommitted_overlap`, with `paths`, nothing changes and no temporary ref is left
  [@test] crates/git-ops/src/unapply_test.rs (`a_drop_under_a_staged_change_names_the_path`)
  [@test] crates/bd-update/src/drop_test.rs (`drop_refuses_what_is_not_ahead_a_dependent_later_commit_local_changes_and_another_head`)

#### Scenario: An operation in progress, a detached HEAD, a merge in the range
- **WHEN** a rebase, merge, cherry-pick or revert is in progress, HEAD is detached, or the range holds a merge
- **THEN** the drop is refused and nothing changes
  [@test] crates/git-ops/src/unapply_test.rs (`a_drop_is_refused_during_a_merge`)

#### Scenario: Another HEAD than the request was made for
- **WHEN** HEAD is another than the request was made for
- **THEN** the drop is refused with `head_moved`, and nothing changes
  [@test] crates/history-rewrite/src/drop_commits_test.rs (`a_drop_refuses_commits_that_are_not_ahead_and_another_head`)
  [@test] crates/bd-update/src/drop_test.rs (`drop_refuses_what_is_not_ahead_a_dependent_later_commit_local_changes_and_another_head`)

### Requirement: Refusals: The app and `brd` answer each refusal with the same code

The app and `brd` MUST answer each refusal with the same code.

#### Scenario: A refusal of a drop
- **WHEN** a drop is refused
- **THEN** `brd` answers the code that the app answers for it
  [@test] crates/bd-update/src/error_code_test.rs (`bd_answers_the_code_of_each_refusal_of_a_move_an_unapply_and_a_reapply_that_the_app_answers`)

### Requirement: Lock and engine: The drop and its undo hold the lock only for the witness and the move

The drop and its undo MUST compute before the repository lock (the commits ahead and their grouping, the replay, the
count of the new history, the dry run of the files, the carried mapping notes; the reflog and the dry run of the
undo) and hold it only for the witness of HEAD and of the mark of an interrupted move, and the guarded move.

#### Scenario: A drop and its undo under the lock
- **WHEN** a drop and its undo run
- **THEN** they compute before the repository lock, and hold it only for the witness of HEAD and of the mark of an interrupted move, and the guarded move
  [@test] crates/history-rewrite/src/drop_commits_test.rs (`a_drop_and_its_undo_lock_only_the_witness_and_the_move`)
  [@test] crates/sync-core/src/process_budget_test.rs (`a_drop_of_commits`)

### Requirement: Lock and engine: The drop and its undo are rewrites of the engine

The drop MUST be the rewrite `drop` of the engine and its undo the rewrite `undo` of its `drop:<previous>..<new>`
token.

#### Scenario: A drop and its undo through the engine
- **WHEN** the app drops a commit and undoes the drop
- **THEN** the drop runs as the rewrite `drop` of the engine and its undo as the rewrite `undo` of its token
  [@test] crates/operations/src/rewrite_test.rs (`a_drop_and_its_undo_run_through_the_engine_and_a_queued_drop_follows_a_reword`)
  [@test] src/presenters/rewrite.test.ts (`words a rewrite by what it does to what it names`)

### Requirement: Lock and engine: A queued drop follows a rewrite of its commit

A drop queued behind a rewrite of its commit MUST name it through the new id that rewrite answered.

#### Scenario: A drop queued behind a reword
- **WHEN** a drop is queued behind a reword of its commit
- **THEN** it drops the commit by the new id the reword answered
  [@test] crates/operations/src/rewrite_test.rs (`a_drop_and_its_undo_run_through_the_engine_and_a_queued_drop_follows_a_reword`)

### Requirement: Undo: The undo brings the commits back with their ids, files and notes

The undo MUST bring the commits back with their ids, their files and their mapping notes.

#### Scenario: The undo of a drop
- **WHEN** the undo of a drop runs
- **THEN** HEAD is the one before the drop, so the commits are back with their ids
- **AND** their files and their mapping notes are back
  [@test] crates/history-rewrite/src/drop_commits_test.rs (`a_drop_takes_the_commits_and_their_files_out_and_names_what_the_next_sync_builds_anew`)
  [@test] crates/git-ops/src/unapply_test.rs (`a_drop_takes_the_commits_and_their_files_out_of_head_and_its_undo_brings_them_back`)

### Requirement: Undo: The undo is refused as the undo of an unapply is

The undo MUST be refused as the undo of an unapply is (a new commit on top: `undo_stale`; local changes on its paths).

#### Scenario: A new commit on top
- **WHEN** a new commit is on top of the drop
- **THEN** the undo is refused with `undo_stale`

#### Scenario: Local changes on the paths of the undo
- **WHEN** local changes are on the paths the undo writes
- **THEN** the undo is refused

### Requirement: Undo: Undoing again is no error

Undoing again MUST be no error (`already_undone`).

#### Scenario: A second undo of the same drop
- **GIVEN** a drop that was undone
- **WHEN** its undo runs again
- **THEN** it is no error and answers `already_undone`
  [@test] crates/history-rewrite/src/drop_commits_test.rs (`a_drop_takes_the_commits_and_their_files_out_and_names_what_the_next_sync_builds_anew`)

### Requirement: Undo: The token of a drop is `drop:` with two full ids

The token of a drop MUST be `drop:` with two full ids, and only the undo of a drop takes it.

#### Scenario: The token read strictly
- **WHEN** a token is read
- **THEN** `drop:` with two full ids is a drop token, and short ids, the same id twice or no `drop:` are not
  [@test] crates/history-rewrite/src/drop_commits_test.rs (`a_drop_token_names_two_full_ids`)

#### Scenario: An undo of an unapply given a drop
- **WHEN** the undo of an unapply is asked to take a drop back
- **THEN** it refuses; only the undo of a drop takes it
  [@test] crates/git-ops/src/unapply_test.rs (`a_drop_takes_the_commits_and_their_files_out_of_head_and_its_undo_brings_them_back`)

### Requirement: App: The context menu of commits offers "Drop Commit…"

The context menu of commits (a branch, the unassigned commits, the preview, the review window) MUST offer "Drop
Commit…", and "Drop N Commits…" for a selection.

#### Scenario: The menu of a commit
- **WHEN** the user opens the context menu of a commit of a branch, of the unassigned commits, of the preview or of the review window
- **THEN** it offers "Drop Commit…", and "Drop N Commits…" for a selection
  [@test] tests/e2e/drop-commits/drop-commits.spec.ts (`should drop a commit and its file from its menu and bring them back with the Undo of its toast`)

### Requirement: App: Backspace or Delete opens the drop of the selected rows

The key of the drop, Backspace or Delete (⌫ on a Mac, Del elsewhere), MUST open the same with rows selected.

#### Scenario: The key on a selected commit
- **GIVEN** a selected commit row
- **WHEN** the user presses Backspace or Delete (⌫ on a Mac, Del elsewhere)
- **THEN** the same confirmation opens
  [@test] tests/e2e/drop-commits/drop-commits.spec.ts (`should drop the selected commit with the key, and say which branch it leaves without a commit`)

### Requirement: App: The drop opens a confirmation below the lead row

The drop MUST open a confirmation below the lead row of the commits, as "Move to…" does: the commits, "This removes N
commits and their changes from HEAD" with what follows (the files, a commit and its revert, local changes, the Undo, a
pushed branch).

#### Scenario: The confirmation of a drop
- **WHEN** the user chooses "Drop Commit…"
- **THEN** a confirmation below the lead row lists the commits and says "This removes N commits and their changes from HEAD" with what follows
  [@test] tests/e2e/drop-commits/drop-commits.spec.ts (`should drop a commit and its file from its menu and bring them back with the Undo of its toast`)

### Requirement: App: Enter drops, Escape and Cancel close the confirmation

The confirmation MUST focus "Drop Commit" ("Drop Commits"), so Enter drops; Escape and Cancel MUST close it.

#### Scenario: Enter drops
- **WHEN** the confirmation opens
- **THEN** "Drop Commit" ("Drop Commits") has the focus, and Enter drops
  [@test] tests/e2e/drop-commits/drop-commits.spec.ts (`should drop a commit and its file from its menu and bring them back with the Undo of its toast`)

#### Scenario: Escape and Cancel close it
- **WHEN** the user presses Escape or Cancel in the confirmation
- **THEN** it closes and nothing is dropped
  [@test] tests/e2e/drop-commits/drop-commits.spec.ts (`should drop the selected commit with the key, and say which branch it leaves without a commit`)

### Requirement: App: Success shows "Dropped N commits" with Undo

Success MUST show "Dropped N commits" with Undo, which brings them back with their files, its description saying that
the files did not change (a commit and its revert) and which branches have no commits left.

#### Scenario: Undo of the toast
- **WHEN** a drop succeeds
- **THEN** "Dropped N commits" shows with Undo, and Undo brings the commits back with their files
  [@test] tests/e2e/drop-commits/drop-commits.spec.ts (`should drop a commit and its file from its menu and bring them back with the Undo of its toast`)

#### Scenario: The description of the toast
- **WHEN** a drop leaves the files as they were, or leaves a branch without commits
- **THEN** the description of the toast says that the files did not change, and which branches have no commits left
  [@test] tests/e2e/drop-commits/drop-commits.spec.ts (`should drop the selected commit with the key, and say which branch it leaves without a commit`)
  [@test] src/presenters/drop.test.ts (`says that the files stayed and names the branches left without a commit`)

### Requirement: App: A refused drop keeps the commits and says why

A refusal MUST keep the commits and show "Cannot drop the commits" with why.

#### Scenario: A refused drop
- **WHEN** the drop is refused
- **THEN** the commits stay and "Cannot drop the commits" says why
  [@test] tests/e2e/drop-commits/drop-commits.spec.ts (`should keep the commits and say why when the drop is refused`)

### Requirement: `brd drop`: `brd drop` drops the commits and prints the result

`brd drop <commit>... [--expect-head <id>] [--dry-run]` MUST drop the commits as above and print `dropped`,
`files_changed`, `rebuilt_branches`, `emptied_branches`, `previous_head`, `new_head` and its `undo_token`.

#### Scenario: `brd drop` of a commit
- **WHEN** `brd drop <commit>` runs
- **THEN** the commit and its file leave HEAD
- **AND** it prints `dropped`, `files_changed`, `rebuilt_branches`, `emptied_branches`, `previous_head`, `new_head` and its `undo_token`, as its schema says
  [@test] crates/bd-update/src/drop_test.rs (`drop_takes_the_commit_and_its_file_out_of_head_and_the_undo_brings_them_back`, `drop_outputs_match_the_schema`)

### Requirement: `brd drop`: `--dry-run` changes nothing

`--dry-run` SHALL build the drop and drop it, with no ref, note or file changed and no record.

#### Scenario: `brd drop --dry-run`
- **WHEN** `brd drop --dry-run` runs
- **THEN** it builds the drop and drops it, with no ref, note or file changed and no record
  [@test] crates/bd-update/src/drop_test.rs (`drop_takes_the_commit_and_its_file_out_of_head_and_the_undo_brings_them_back`)

### Requirement: `brd drop`: A commit the home branch has is dropped there

In a lease of the pool, a commit its home branch has (a pick landed it) MUST be refused as `drop_in_home` (exit 4)
with the two next steps: the drop in the shared checkout, then `brd lease pick`, which lands nothing and moves the
lease past the dropped commit, so the lease can be returned.

#### Scenario: A drop of a picked commit in its lease
- **GIVEN** a commit of the lease that a pick landed on its home branch
- **WHEN** `brd drop` of it runs in the lease
- **THEN** it is refused as `drop_in_home` (exit 4), nothing changed, with the two next steps: the drop in the shared checkout, then `brd lease pick`
- **AND** after the drop in the shared checkout, `brd lease pick` lands nothing and moves the lease past the dropped commit, so the lease can be returned
  [@test] crates/bd-lease/src/lease_test.rs (`a_session_drops_its_landed_commit_in_the_shared_checkout_and_its_lease_follows`)

### Requirement: `brd drop`: The agent's rules name `brd drop`, never `git revert`

The agent's rules MUST name `brd drop` for a commit of its own that must not land, never `git revert`, both dropped
when a revert exists, and a commit of another session or of the user only after the user asked.

#### Scenario: The help of the agent
- **WHEN** the agent reads its rules
- **THEN** they name `brd drop` for a commit of its own that must not land, never `git revert`, both dropped when a revert exists, and a commit of another session or of the user only after the user asked
  [@test] crates/bd-cli/src/agent_help_test.rs (`every_refusal_is_in_the_help_of_its_command`)

## References

- [Unapply and Reapply](../../branch/unapply/spec.md): the shared check and move
- [Squash Fixups](../fixups/spec.md): a fixup that cancels its target drops it too
- [Git Operations](../../../../docs/design/git-operations.md), "Drop and unapply"
