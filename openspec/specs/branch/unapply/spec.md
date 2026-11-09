---
name: Unapply and Reapply
description: Requirements for unapplying a virtual branch, which takes its commits and their files out of the current branch and keeps the branch in the unapplied namespace, atomically, and for reapplying it, which puts its commits back on top with their grouping prefix; both refused by local changes on the paths they write, both with an undo.
style: plain-1
targets:
  - crates/sync-core/src/unapply_branch.rs
  - crates/git-ops/src/unapply.rs
  - crates/git-ops/src/update_branch.rs
  - crates/git-ops/src/worktree_update.rs
  - crates/history-rewrite/src/repository_lock.rs
  - crates/git-ops/src/rewrite_plan.rs
  - crates/git-ops/src/replay.rs
  - crates/branch-integration/src/archive.rs
  - crates/operations/src/rewrite.rs
  - src/lib/rewrite-run.ts
  - src/lib/git/delete-kept-branch.ts
  - src/hooks/useUnapplyBranch.ts
  - src/components/branch/InlineUnapplyConfirm.tsx
  - crates/bd-update/src/unapply_command.rs
  - crates/bd-update/src/undo_command.rs
  - crates/domain-types/src/error.rs
  - crates/bd-cli/src/agent_help.rs
  - crates/sync-core/src/sync.rs
  - crates/domain-types/src/lib.rs
  - crates/sync-core/src/push.rs
  - crates/sync-core/src/delete_archived_branch.rs
  - crates/bd-remote/src/push_command.rs
  - crates/bd-remote/src/safe_push_command.rs
  - crates/bd-remote/src/status.rs
  - src/stores/unapplied-branches.ts
  - src/components/branch/UnappliedBranchTableCard.tsx
  - src/hooks/useKeptBranchMenu.ts
  - crates/domain-policy/src/kept.rs
  - crates/repository-model/src/action_view.rs
  - src/lib/push-run.ts
  - crates/repository-model/src/table.rs
  - crates/sync-core/src/reapply_branch.rs
  - crates/git-ops/src/reapply.rs
  - crates/git-ops/src/pick.rs
  - src/lib/git/reapply.ts
  - src/presenters/rewrite.ts
  - crates/operations/src/push.rs
---

# Unapply and Reapply

Status: Implemented
Date: 2026-10-21

## Purpose

Some work must leave the local branch for a while: the world is not ready for it, a team prepares a proper fix, a
change waits for a decision. Parking keeps it local but in the build; unapplying takes it out. The commits of the
virtual branch leave the current branch, the files they changed go back to how they are without them, and the virtual
branch is kept in the unapplied namespace (`{prefix}/unapplied/<name>`), which no sync archives or deletes. Reapply
puts its commits back on top of the current branch, their files and their grouping prefix with them.

## Goals

- Take a branch out of the current branch, its files too, without losing its work.
- Keep the user's work on other paths as it is, staged or not.
- All or nothing: HEAD, the files and the branch move together, or nothing does.
- Take it back at once with Undo, and let an agent do the same with `brd unapply` and `brd undo`.
- See what waits outside HEAD, keep a copy of it on the remote, and delete it once it is not needed.
- Bring it back when the world is ready, as the branch it was.

## Non-goals

- Dropping commits from a range that contains merge commits.
- Taking the children of a stacked branch out with it: they stay, built on the next branch below that is applied.

## Requirements

### Requirement: Confirmation: Unapply asks for confirmation below the row

"Unapply…" in the branch menu (right-click and "⋯") MUST expand the branch and open a confirmation below its row that
says how many commits leave HEAD with their changes, and that uncommitted changes to those files stop it. Its "Unapply
Branch" button MUST take the focus, so Enter confirms; Escape cancels.

#### Scenario: The confirmation opens below the row
- **WHEN** the user chooses "Unapply…" in the menu of a branch row, by right-click or "⋯"
- **THEN** the branch expands and a confirmation below its row says how many commits leave HEAD with their changes
- **AND** it says that uncommitted changes to those files stop it
  [@test] tests/e2e/unapplied-branches/unapplied-branches.spec.ts (`should unapply a branch from its menus: its commits and their files leave HEAD, the branch is kept`)

#### Scenario: Enter confirms, Escape cancels
- **WHEN** the confirmation is open
- **THEN** its "Unapply Branch" button has the focus, so Enter confirms
- **AND** Escape cancels it
  [@test] tests/e2e/unapplied-branches/unapplied-branches.spec.ts (`should keep the branch and say why when the unapply fails, and close the confirmation with Escape`)

### Requirement: Result: The commits of the branch leave the current branch

The commits of the branch, as they group now, MUST leave the current branch, and the commits after them MUST be
replayed in order. The app's request names the commits it shows.

#### Scenario: The commits leave HEAD and the later ones are replayed
- **WHEN** the unapply of a branch runs
- **THEN** the commits that group into it now leave the current branch
- **AND** the commits after them are replayed in order
  [@test] crates/sync-core/src/unapply_branch_test.rs (`unapply_takes_the_commits_of_the_branch_out_of_head_with_their_files`)

#### Scenario: The branch groups other commits than the request names
- **WHEN** the branch groups others now than the commits the app's request names
- **THEN** the unapply is refused, "sync and try again"
  [@test] crates/sync-core/src/unapply_branch_test.rs (`unapply_refuses_an_unknown_or_changed_branch_one_not_built_and_one_checked_out`)

### Requirement: Result: The files the commits changed follow them

The files the commits changed MUST follow them: a file they added is gone, a file they changed is as the remaining
commits leave it, and `git status` shows nothing of them.

#### Scenario: Added files are gone, changed files are as the remaining commits leave them
- **WHEN** the unapply runs
- **THEN** a file the commits added is gone
- **AND** a file they changed is as the remaining commits leave it
- **AND** `git status` shows nothing of them
  [@test] crates/git-ops/src/unapply_test.rs (`the_files_follow_the_unapply_and_the_virtual_branch_moves_in_the_same_transaction`)
  [@test] tests/e2e/unapplied-branches/unapplied-branches.spec.ts (`should unapply a branch from its menus: its commits and their files leave HEAD, the branch is kept`)

### Requirement: Result: The virtual branch is kept in the unapplied namespace

The virtual branch MUST move to the unapplied namespace, under a name that no ref takes or lies below (git refuses
`a` next to `a/b`); a conflicting name gets the `-N` suffix. A later sync MUST leave it as it is.

#### Scenario: The branch moves under its name and a sync leaves it
- **WHEN** the unapply runs and no ref takes the name of the branch
- **THEN** the virtual branch moves to the unapplied namespace under its name
- **AND** a later sync leaves it as it is
  [@test] crates/sync-core/src/unapply_branch_test.rs (`unapply_takes_the_commits_of_the_branch_out_of_head_with_their_files`)

#### Scenario: A ref takes the name or lies below it
- **WHEN** a ref takes the name or lies below it
- **THEN** the unapplied branch gets the `-N` suffix
  [@test] crates/sync-core/src/unapply_branch_test.rs (`test_unapply_avoids_a_ref_below_the_name`)

### Requirement: Result: HEAD, the branch and the files move together

The move of HEAD, the move of the virtual branch and the files MUST happen together: one ref transaction, and when the
files can't follow, HEAD and the branch move back and nothing is changed.

#### Scenario: A file changed since the check
- **WHEN** the files can't follow, as a file changed since the check
- **THEN** HEAD and the branch move back and nothing is changed
  [@test] crates/git-ops/src/unapply_test.rs (`a_file_changed_after_the_check_moves_head_and_the_branch_back`)

#### Scenario: A crash between the move and the files
- **WHEN** a crash comes between the move and the files
- **THEN** it leaves a mark that the next move of the files finishes
  [@test] crates/git-ops/src/unapply_test.rs (`an_interrupted_unapply_finishes_its_files`)

### Requirement: Result: The toast names the commits and the unapplied branch

The toast "Unapplied ‹branch›" MUST name the commits that left HEAD and the unapplied branch that keeps them.

#### Scenario: The toast of an unapply
- **WHEN** an unapply succeeds
- **THEN** the toast "Unapplied ‹branch›" names the commits that left HEAD and the unapplied branch that keeps them
  [@test] tests/e2e/unapplied-branches/unapplied-branches.spec.ts (`should unapply a branch from its menus: its commits and their files leave HEAD, the branch is kept`)

### Requirement: Working tree and index: Work on other paths stays as it is

Staged, unstaged and untracked work on paths the unapply does not write MUST stay as it is.

#### Scenario: Unrelated work, staged and not
- **GIVEN** staged, unstaged and untracked work on paths the unapply does not write
- **WHEN** the unapply runs
- **THEN** that work stays as it is
  [@test] crates/sync-core/src/unapply_branch_test.rs (`unapply_keeps_unrelated_work_staged_and_unstaged`)

### Requirement: Working tree and index: A local change on a path the unapply writes refuses it

A local change on a path the unapply writes, staged or not, MUST refuse it with the paths, and change nothing.

#### Scenario: A local change in the way
- **GIVEN** a local change, staged or not, on a path the unapply writes
- **WHEN** the unapply runs
- **THEN** it is refused with the paths, and nothing changes
  [@test] crates/sync-core/src/unapply_branch_test.rs (`unapply_refuses_a_local_change_on_a_path_it_writes_and_changes_nothing`)
  [@test] crates/git-ops/src/unapply_test.rs (`a_local_change_on_a_path_the_unapply_writes_refuses_it_staged_or_not`)

### Requirement: Refusals: The unapply is refused where it cannot drop the commits

The unapply MUST be refused, and change nothing, when no commit groups into the branch or it groups others than the
request names, when the virtual branch does not exist or is not the copies of its commits now, when a worktree has it
checked out, during a rebase, merge, cherry-pick or revert or with a detached HEAD, when the range from the oldest
dropped commit to HEAD contains a merge commit, and when the branch moved meanwhile.

#### Scenario: An unknown branch, or one that groups other commits
- **WHEN** no commit groups into the branch, or it groups others than the request names
- **THEN** the unapply is refused and nothing changes
  [@test] crates/sync-core/src/unapply_branch_test.rs (`unapply_refuses_an_unknown_or_changed_branch_one_not_built_and_one_checked_out`)

#### Scenario: A branch that is not built
- **WHEN** the virtual branch does not exist, or is not the copies of its commits now (a conflict, a sync that did not run)
- **THEN** the unapply is refused and nothing changes, as a commit would leave HEAD without a copy

#### Scenario: A branch checked out in a worktree
- **WHEN** a worktree has the virtual branch checked out
- **THEN** the unapply is refused and nothing changes

#### Scenario: An operation of git in progress
- **WHEN** a rebase, merge, cherry-pick or revert is in progress, or HEAD is detached
- **THEN** the unapply is refused and nothing changes
  [@test] crates/git-ops/src/unapply_test.rs (`a_drop_is_refused_during_a_merge`)

#### Scenario: A merge commit in the range
- **WHEN** the range from the oldest dropped commit to HEAD contains a merge commit
- **THEN** the unapply is refused and nothing changes

#### Scenario: The branch moved meanwhile
- **WHEN** the branch moved meanwhile
- **THEN** the unapply is refused and nothing changes

### Requirement: Refusals: Unapply, reapply and delete are rewrites of the engine

The unapply, the reapply, the delete of an unapplied branch and their undo MUST be rewrites of the engine (the intent
`rewrite`, `execute`): the session runs them one after the other with the other rewrites of the app, in the order they
came, each on what the one before it left, and a second start of the same one joins it.

#### Scenario: Rewrites run in the order they came
- **WHEN** the app starts an unapply, a reapply or the delete of an unapplied branch beside other rewrites of the app
- **THEN** the session runs them one after the other, in the order they came, each on what the one before it left
- **AND** a second start of the same one joins it
  [@test] crates/operations/src/rewrite_test.rs (`an_unapply_a_reapply_and_the_delete_of_the_unapplied_branch_run_through_the_engine`)

#### Scenario: The delete runs beside a rewrite and a sync
- **WHEN** the delete of an unapplied branch runs
- **THEN** it holds only the ref it drops, so it runs beside a rewrite and a sync

### Requirement: Refusals: A rewrite waits while a live holder of the repository lock lives

The unapply, the reapply and their undo MUST run under the repository lock: while another process holds it (an agent's
`brd pick --follow`, `brd`) or the app's own push, the app's wait as long as the holder lives, their surface saying
whom for with Stop Waiting (`cancel_operation` of the operation), which ends the wait with nothing changed.

#### Scenario: Another process holds the lock
- **WHEN** another process holds the repository lock (an agent's `brd pick --follow`, `brd`), or the app's own push
- **THEN** the unapply, the reapply or their undo of the app waits as long as the holder lives
- **AND** its surface (the confirmation, the "⋯" of the row) says whom for, with Stop Waiting
  [@test] crates/operations/src/rewrite_test.rs (`a_rewrite_waits_while_the_holder_of_the_lock_lives_and_stops_when_told`)
  [@test] src/lib/rewrite-run.test.ts (`shows whom a rewrite of the engine waits for and stops the wait by its operation`)
  [@test] crates/sync-core/src/repository_lock_test.rs (`a_rewrite_of_the_app_waits_while_a_live_holder_holds_the_lock_and_runs_after`)

#### Scenario: Stop Waiting
- **WHEN** the user chooses Stop Waiting (`cancel_operation` of the operation)
- **THEN** the wait ends with nothing changed
  [@test] crates/sync-core/src/repository_lock_test.rs (`a_rewrite_of_the_app_ends_its_wait_when_stopped_and_refuses_an_unnamed_holder`)

### Requirement: Refusals: A holder no record names refuses the rewrite after 10 s

Only a holder of the repository lock that no record names MUST refuse them after 10 s, with nothing changed and the
error toast saying that another operation holds the lock; `brd unapply` and `brd reapply` MUST wait 10 s, then answer
`locked`.

#### Scenario: A holder no record names
- **WHEN** a holder that no record names holds the repository lock for 10 s
- **THEN** the rewrite of the app is refused with nothing changed
- **AND** the error toast says that another operation holds the lock
  [@test] crates/sync-core/src/repository_lock_test.rs (`a_rewrite_of_the_app_ends_its_wait_when_stopped_and_refuses_an_unnamed_holder`)

#### Scenario: `brd` while another holds the lock
- **WHEN** `brd unapply` or `brd reapply` runs while another holds the repository lock
- **THEN** it waits 10 s, then answers `locked`
  [@test] crates/sync-core/src/repository_lock_test.rs (`every_rewrite_fails_with_locked_while_another_holds_the_lock`, `a_rewrite_waits_for_the_holder`)

### Requirement: Refusals: A queued unapply takes the commits the window showed by their new ids

An unapply MUST name the commits of its branch the window showed: queued behind another rewrite of the app, or after
another process moved HEAD, it takes them by the ids that rewrite gave them (else by their author, author time and
subject without the prefix), waits for the sync that builds their copies, and runs when its branch there has those
commits.

#### Scenario: Queued behind a reword of the branch's commit
- **WHEN** the unapply waits behind another rewrite of the app (a reword of the branch's commit)
- **THEN** it takes the commits by the ids that rewrite gave them, waits for the sync that builds their copies
- **AND** it runs when its branch there has those commits
  [@test] crates/operations/src/rewrite_test.rs (`an_unapply_queued_behind_a_reword_of_its_branch_unapplies_what_the_reword_left`)

#### Scenario: The branch has other commits there
- **WHEN** its branch there has other commits (a reword that took the commit to another branch)
- **THEN** the unapply is `plan_changed`, nothing written

#### Scenario: Another process moved HEAD while it waited for the lock
- **WHEN** another process moved HEAD while the unapply waited for the lock
- **THEN** it resolves and runs once more on that HEAD
  [@test] crates/operations/src/rewrite_test.rs (`an_unapply_and_a_reword_waiting_for_the_lock_of_another_process_run_on_the_head_it_left`)
  [@test] crates/sync-core/src/repository_lock_test.rs (`every_rewrite_refuses_another_head_than_the_expected_one`)

### Requirement: Refusals: Rewrites compute before the repository lock

The unapply, the reapply and their undo MUST read and compute before the repository lock (the grouping, the build of
the virtual branch, the drop or the pick, the dry run of the files, the reflog of an undo) and hold it only for the
witness of what they read (HEAD, its branch, the refs they computed on and the mark of an interrupted move) and the
move.

#### Scenario: An unapply and its undo
- **WHEN** an unapply or its undo runs
- **THEN** it holds the repository lock only for the witness and the move
  [@test] crates/sync-core/src/unapply_branch_test.rs (`an_unapply_and_its_undo_lock_only_the_witness_and_the_move`)

#### Scenario: A reapply and its undo
- **WHEN** a reapply or its undo runs
- **THEN** it holds the repository lock only for the witness and the move
  [@test] crates/sync-core/src/reapply_branch_test.rs (`a_reapply_and_its_undo_lock_only_the_witness_and_the_move`)

#### Scenario: A ref moved meanwhile
- **WHEN** a ref it computed on moved meanwhile
- **THEN** it releases the lock and computes it once more without it, the second time under it

#### Scenario: HEAD moved between two reads of the computation
- **WHEN** HEAD moved between two reads of the computation
- **THEN** it computes once more under the lock
  [@test] crates/sync-core/src/repository_lock_test.rs (`a_head_moved_between_two_reads_of_a_rewrite_computes_it_once_more_under_the_lock`)

### Requirement: Refusals: The files of an interrupted move follow first

A move of the files that a crash interrupted MUST have its files follow under the lock first, and never refuse the
next unapply as local changes.

#### Scenario: An unapply after a move whose files did not follow
- **GIVEN** a move whose files a crash interrupted
- **WHEN** the next unapply runs
- **THEN** the files of that move follow under the lock first, and the unapply is not refused as local changes
  [@test] crates/sync-core/src/unapply_branch_test.rs (`an_unapply_after_a_move_whose_files_did_not_follow_lets_them_follow_first`)

### Requirement: Refusals: The index lock of another process makes a rewrite wait

The index lock of another process MUST make the unapply, the reapply and their undo wait (at most 30 s per write) and
an ownerless one be removed, as for `brd pick`; one that stays MUST move HEAD back and answer `locked`. The wait MUST
come before the repository lock, so no other writer waits for it.

#### Scenario: Another process holds the index lock
- **WHEN** another process holds the index lock
- **THEN** the rewrite waits for it, at most 30 s per write, before the repository lock
- **AND** a lock that stays moves HEAD back and answers `locked`
  [@test] crates/history-rewrite/src/repository_lock_test.rs (`a_move_whose_files_follow_waits_for_the_index_lock_before_the_repository_lock`)

#### Scenario: An index lock that no process owns
- **WHEN** no process owns the index lock
- **THEN** it is removed and the files follow
  [@test] crates/git-ops/src/pick_test.rs (`a_move_removes_an_index_lock_that_no_process_owns_and_lets_the_files_follow`)

#### Scenario: A lock that appears while the move takes the repository lock
- **WHEN** an index lock appears while the move takes the repository lock
- **THEN** the move releases the repository lock and waits outside it

### Requirement: Refusals: A later commit that depends on a dropped one fails the unapply

A later commit that depends on a dropped commit MUST make the unapply fail with a conflict, and a ref, an index entry
or a file MUST NOT change.

#### Scenario: A dependent later commit
- **GIVEN** a later commit that depends on a dropped commit
- **WHEN** the unapply runs
- **THEN** it fails with a conflict, and no ref, index entry or file changes
  [@test] crates/sync-core/src/unapply_branch_test.rs (`unapply_conflict_changes_nothing`)
  [@test] crates/git-ops/src/unapply_test.rs (`a_conflict_of_the_drop_changes_nothing`)

### Requirement: Refusals: A later commit the unapply would empty refuses it

A later commit of another branch that holds only changes of the unapplied commits would become empty, and `git replay`
would leave it out with its message: it MUST refuse the unapply as a conflict does ("move it into that branch first"),
nothing changed.

#### Scenario: A later commit of only the unapplied changes
- **GIVEN** a later commit of another branch that holds only changes of the unapplied commits
- **WHEN** the unapply runs
- **THEN** it is refused as a conflict is, "move it into that branch first", and nothing changes
  [@test] crates/sync-core/src/unapply_branch_test.rs (`unapply_keeps_the_note_of_a_later_commit_whose_trees_stay_and_refuses_to_empty_one`)

### Requirement: Refusals: A later commit whose trees stay keeps its mapping note

A later commit whose trees the unapply keeps (the branch's commits cancel out) MUST keep its mapping note, written
before the move, so the next sync keeps its virtual copy.

#### Scenario: The commits of the branch cancel out
- **GIVEN** a later commit whose trees the unapply keeps, as the branch's commits cancel out
- **WHEN** the unapply runs
- **THEN** the commit keeps its mapping note, written before the move, and the next sync keeps its virtual copy
  [@test] crates/sync-core/src/unapply_branch_test.rs (`unapply_keeps_the_note_of_a_later_commit_whose_trees_stay_and_refuses_to_empty_one`)

### Requirement: Refusals: A failed unapply keeps the branch and says why

A failed unapply MUST keep the branch in the table and say why in the error toast "Cannot unapply ‹branch›".

#### Scenario: The unapply fails
- **WHEN** the unapply fails
- **THEN** the branch stays in the table and the error toast "Cannot unapply ‹branch›" says why
  [@test] tests/e2e/unapplied-branches/unapplied-branches.spec.ts (`should keep the branch and say why when the unapply fails, and close the confirmation with Escape`)

### Requirement: Reapply: Reapply puts the branch's own commits back on top of HEAD

Reapply (the menu of an unapplied row, `brd reapply <name>`) MUST put the branch's own commits (after its stack note,
else its merge base with the baseline) back on top of HEAD, the files following, and delete the unapplied branch in the
same transaction; author kept, committer the current user. The next sync builds the branch again.

#### Scenario: Reapply from the menus
- **WHEN** the user chooses Reapply in the menu of an unapplied row
- **THEN** the branch's own commits are back on top of HEAD, the files following, and the unapplied branch is deleted
- **AND** the branch is back in the table
  [@test] tests/e2e/unapplied-branches/unapplied-branches.spec.ts (`should reapply an unapplied branch from both menus, the branch back in the table, and take it back with Undo`)

#### Scenario: The next sync builds the branch again
- **WHEN** the next sync runs after a reapply
- **THEN** it builds the branch again; the authors are kept, the committer is the current user
  [@test] crates/sync-core/src/reapply_branch_test.rs (`reapply_puts_the_commits_back_on_top_with_their_prefix_and_files_and_the_sync_builds_the_branch_again`)

### Requirement: Reapply: A commit gets its grouping prefix back

A commit whose subject groups elsewhere MUST get `(<name>) ` back, since a virtual commit keeps no grouping prefix; an
issue branch comes back as it was.

#### Scenario: A subject that groups elsewhere
- **WHEN** the reapply puts back a commit whose subject groups elsewhere
- **THEN** its subject gets `(<name>) ` back
  [@test] crates/sync-core/src/reapply_branch_test.rs (`reapply_puts_the_commits_back_on_top_with_their_prefix_and_files_and_the_sync_builds_the_branch_again`)

#### Scenario: An issue branch
- **WHEN** the reapply puts back an issue branch
- **THEN** it comes back as it was, without a prefix
  [@test] crates/sync-core/src/reapply_branch_test.rs (`an_issue_branch_comes_back_without_a_prefix`)

### Requirement: Reapply: A commit HEAD has is left out and a dry run changes nothing

A commit HEAD has by its patch MUST be left out, one that becomes empty drops; a dry run MUST change nothing.

#### Scenario: A dry run
- **WHEN** the reapply runs as a dry run
- **THEN** it changes nothing and says what the reapply would do, a commit HEAD has by its patch left out
  [@test] crates/sync-core/src/reapply_branch_test.rs (`a_dry_run_changes_nothing_and_says_what_the_reapply_would_do`)

### Requirement: Reapply: The reapply is refused where it cannot put the commits back

The reapply MUST be refused, and change nothing, for an unknown branch, a commit that conflicts with HEAD (its files
named), local changes on a path it writes, a branch checked out in a worktree, and a HEAD with every commit of it
already; computed before the repository lock and written under it, as every rewrite.

#### Scenario: An unknown branch, a conflict, local changes in the way
- **WHEN** the branch is unknown, a commit of it conflicts with HEAD, or local changes sit on a path it writes
- **THEN** the reapply is refused and nothing changes; a conflict names its files
  [@test] crates/sync-core/src/reapply_branch_test.rs (`reapply_refuses_an_unknown_branch_a_conflict_and_local_changes_in_the_way_and_changes_nothing`)
  [@test] tests/e2e/unapplied-branches/unapplied-branches.spec.ts (`should keep an unapplied branch whose reapply conflicts and say why`)

#### Scenario: A branch checked out, or every commit in HEAD already
- **WHEN** a worktree has the branch checked out, or HEAD has every commit of it already
- **THEN** the reapply is refused and nothing changes

#### Scenario: Another holds the repository lock past its wait
- **WHEN** another holds the repository lock past the wait of the reapply (the 10 s of `brd reapply`)
- **THEN** the reapply, computed before the lock, fails with `locked`, and the unapplied branch stays
  [@test] crates/sync-core/src/reapply_branch_test.rs (`reapply_waits_for_the_repository_lock_and_fails_with_locked`)

### Requirement: Reapply: Undo of a reapply takes the commits out again

The toast "Reapplied ‹branch›" MUST offer Undo, which moves HEAD and the files back and creates the unapplied branch
again; the token `reapply:<previous head>..<new head>:<tip>:<unapplied branch>`; stale once HEAD moved on.

#### Scenario: Undo of the toast
- **WHEN** the user chooses Undo in the toast "Reapplied ‹branch›"
- **THEN** HEAD and the files move back and the unapplied branch is there again
  [@test] crates/sync-core/src/reapply_branch_test.rs (`the_undo_of_a_reapply_takes_the_commits_out_again_and_keeps_the_unapplied_branch`)
  [@test] tests/e2e/unapplied-branches/unapplied-branches.spec.ts (`should reapply an unapplied branch from both menus, the branch back in the table, and take it back with Undo`)

#### Scenario: The token of a reapply
- **WHEN** a reapply succeeds
- **THEN** its token names both heads and the tip by full ids, and the unapplied branch
- **AND** the token is stale once HEAD moved on
  [@test] crates/sync-core/src/reapply_branch_test.rs (`a_reapply_token_names_full_ids_and_the_unapplied_branch`)

### Requirement: Reapply: `brd reapply` prints one object and refuses by codes

`brd reapply <name> [--dry-run] [--expect-head <id>]` MUST print one object (`reapply` in `brd schema`) and refuse with
`reapply_conflict` (`commits`, `paths`), `uncommitted_overlap`, `nothing_to_reapply` and `unknown_branch`.

#### Scenario: `brd reapply` and its undo
- **WHEN** an agent runs `brd reapply <name>`
- **THEN** it prints one object, the branch back with its prefix, and `brd undo` of it takes it out again
  [@test] crates/bd-update/src/unapply_test.rs (`reapply_puts_the_branch_back_with_its_prefix_and_the_undo_takes_it_out_again`)

### Requirement: The Unapplied card: Every sync reports the unapplied branches

Every sync MUST report the unapplied branches once, also none, with the tip, its subject and the remote status of each
one whose remote branch exists; one without a remote branch costs no process.

#### Scenario: A pushed unapplied branch
- **WHEN** a sync runs and an unapplied branch has a remote branch
- **THEN** the sync reports it once with its tip, its subject and its remote status
- **AND** one without a remote branch costs no process
  [@test] crates/sync-core/src/unapply_branch_test.rs (`a_sync_reports_the_unapplied_branches_with_the_remote_status_of_a_pushed_one`)

### Requirement: The Unapplied card: The card lists the unapplied branches out of the branch table

The app MUST list them in the Unapplied card, after the branch table and the Parked card, before the archived
branches, open by default, its rows out of the branch table: the name, the subject, the Remote cell ("Not pushed",
"Pushed", "2 commits to push", "Remote differs"), and its commits below an expanded row.

#### Scenario: An unapplied branch in its card
- **WHEN** the repository has an unapplied branch
- **THEN** the Unapplied card lists it, out of the branch table, with its name, its subject and its Remote cell
- **AND** its commits show below its row when it is expanded, and a help popover says what an unapplied branch is
  [@test] tests/e2e/unapplied-branches/unapplied-branches.spec.ts (`should list an unapplied branch in its own card with its commits, out of the branch table`)

### Requirement: The Unapplied card: The menu of a row offers its review, reapply, push and delete

The menu of a row (right-click and "⋯") MUST offer Review Changes (D, its review: [Review
Window](../../review/review-window/spec.md#kept-branches)), Reapply, Push Branch (Force Push Branch when the remote has
another history, disabled when it has it as it is), Safe Push disabled with "Not in master: Reapply it to land it",
Copy, and Delete Unapplied Branch….

#### Scenario: The menu of an unapplied row
- **WHEN** the user opens the menu of an unapplied row, by right-click or "⋯"
- **THEN** it offers Review Changes, Reapply, Push Branch, Safe Push disabled with why, Copy and Delete Unapplied Branch…
  [@test] tests/e2e/unapplied-branches/unapplied-branches.spec.ts (`should push an unapplied branch under its own name from both menus, its status shown with no sync, and refuse its Safe Push with why`)
  [@test] crates/domain-policy/src/kept_test.rs (`an_unapplied_branch_offers_its_review_reapply_push_and_delete_and_lands_nothing`)

### Requirement: The Unapplied card: Rust decides the actions of a row

Rust MUST decide them (`domain_policy::kept`, the subject `unapplied:<full name>` of the view): a worktree that has the
branch checked out disables Reapply and the delete ("A worktree has docs checked out"), the cached frame of an earlier
run the push until the first sync; a push, a reapply or a delete that runs shows on its item, as on a branch row.

#### Scenario: A worktree has the branch checked out
- **WHEN** a worktree has the branch checked out
- **THEN** Reapply and the delete are disabled ("A worktree has docs checked out")
  [@test] crates/domain-policy/src/kept_test.rs (`a_checked_out_branch_keeps_its_reapply_and_delete_and_a_cached_frame_its_push`)

#### Scenario: The cached frame of an earlier run
- **WHEN** the window shows the cached frame of an earlier run
- **THEN** the push is disabled until the first sync
  [@test] crates/domain-policy/src/kept_test.rs (`a_checked_out_branch_keeps_its_reapply_and_delete_and_a_cached_frame_its_push`)

#### Scenario: An action that runs
- **WHEN** a push, a reapply or a delete of the branch runs
- **THEN** it shows on its item, as on a branch row
  [@test] crates/repository-model/src/derive_test.rs (`the_view_offers_the_actions_of_the_kept_branches_with_their_operations`)

### Requirement: The Unapplied card: Push pushes the branch under its own name

Push MUST push it under its own name (`{prefix}/unapplied/<name>`) with a lease on the remote head of its status, the
intent `Push` with the scope `unapplied` (`operations::push`): no robot read, no hold, no retarget (C3), "Force Push
Branch" of its menu the user's choice to drop what the remote has. A pull request of its virtual branch stays as it was.

#### Scenario: Push Branch of an unapplied row
- **WHEN** the user chooses Push Branch of an unapplied row
- **THEN** the branch is pushed under its own name with a lease on the remote head of its status
- **AND** a pull request of its virtual branch stays as it was
  [@test] crates/sync-core/src/push_test.rs (`an_unapplied_branch_is_pushed_under_its_own_name`)
  [@test] crates/operations/src/push_test.rs (`a_push_of_an_unapplied_branch_is_planned_and_run_by_the_engine`)

### Requirement: The Unapplied card: The push shows in the toast and the Remote cell with no sync

The toast "Pushed ‹branch›" MUST say the remote keeps it, and its Remote cell MUST show the status the push left from
the patch of the session, with no sync after it.

#### Scenario: After the push
- **WHEN** the push of an unapplied branch succeeds
- **THEN** the toast "Pushed ‹branch›" says the remote keeps it
- **AND** its Remote cell shows the status the push left, with no sync after it
  [@test] crates/repository-model/src/table_test.rs (`the_outcome_of_a_push_stays_over_the_remote_statuses_of_a_sync_that_started_before_it`)
  [@test] tests/e2e/unapplied-branches/unapplied-branches.spec.ts (`should push an unapplied branch under its own name from both menus, its status shown with no sync, and refuse its Safe Push with why`)

### Requirement: The Unapplied card: Delete Unapplied Branch… deletes it after its name is typed

Delete Unapplied Branch… MUST ask for its name typed, then delete it with compare-and-swap on its tip, refused while
a worktree has it; only its own rewrite deletes it (`deleteUnapplied`, by its full name). The form shows the delete
while it runs and stays with the typed name after a failure.

#### Scenario: The delete after the name is typed
- **WHEN** the user types the name of the branch in the form of Delete Unapplied Branch… and confirms
- **THEN** the branch is deleted with compare-and-swap on its tip, by its own rewrite
  [@test] crates/sync-core/src/delete_archived_branch_test.rs (`deletes_an_unapplied_branch_only_by_its_own_command`)
  [@test] crates/operations/src/rewrite_test.rs (`an_unapply_a_reapply_and_the_delete_of_the_unapplied_branch_run_through_the_engine`)
  [@test] tests/e2e/unapplied-branches/unapplied-branches.spec.ts (`should delete an unapplied branch after its name is typed, with no sync after it`)

#### Scenario: A worktree has the branch
- **WHEN** a worktree has the branch checked out
- **THEN** the delete is refused

### Requirement: The Unapplied card: A deleted branch stays deleted with no sync

Its row MUST leave the card as a patch of the session, with no sync after it; a sync that listed it before the
delete, or one that fails, does not bring it back.

#### Scenario: A sync that listed it before the delete, or one that fails
- **WHEN** the branch is deleted while a sync that listed it runs, or a sync fails after it
- **THEN** its row stays out of the card
  [@test] crates/repository-model/src/table_test.rs (`a_deleted_archived_or_unapplied_branch_stays_deleted_by_the_sync_that_listed_it_before_and_by_one_that_fails`)
  [@test] crates/repository-model/src/session_test.rs (`deleting_an_archived_or_unapplied_branch_answers_a_patch_and_runs_no_sync`)

### Requirement: Undo: Undo brings the commits, their files and the branch back

The toast "Unapplied ‹branch›" MUST offer Undo (and Activity the same: the rewrite `undo` of its token, in the
repository of the unapply), which brings the commits, their files and the virtual branch back as they were, in one
transaction; the next sync reuses the copies.

#### Scenario: Undo of the toast or of Activity
- **WHEN** the user chooses Undo in the toast "Unapplied ‹branch›" or in Activity
- **THEN** HEAD is at its previous commit, the virtual branch at its tip, the unapplied branch gone, the files following
- **AND** the next sync reuses the copies
  [@test] crates/sync-core/src/unapply_branch_test.rs (`the_undo_of_an_unapply_brings_the_commits_their_files_and_the_virtual_branch_back`)
  [@test] crates/git-ops/src/unapply_test.rs (`the_undo_brings_the_commits_their_files_and_the_virtual_branch_back`)
  [@test] tests/e2e/unapplied-branches/unapplied-branches.spec.ts (`should take the unapply back with the Undo of its toast: the commits, their files and the branch return`)

### Requirement: Undo: The undo token names full ids and both branches

The undo token MUST be `unapply:<previous head>..<new head>:<tip>:<virtual branch>:<unapplied branch>`, full ids and
full branch names; any other text is no token of an unapply.

#### Scenario: The token of an unapply
- **WHEN** an unapply succeeds
- **THEN** its token names both heads and the tip by full ids, and both branches by their full names
- **AND** any other text is no token of an unapply
  [@test] crates/sync-core/src/unapply_branch_test.rs (`an_unapply_token_names_full_ids_and_both_branches`)

### Requirement: Undo: An undo after another move is refused

The undo MUST be refused, and change nothing, when the unapply is not the last move of the branch (a commit since) or
a local change sits on a path it writes; undoing again is no error, nothing changes.

#### Scenario: A commit since the unapply
- **WHEN** a commit was made since the unapply
- **THEN** the undo is refused and nothing changes
  [@test] crates/sync-core/src/unapply_branch_test.rs (`the_undo_of_an_unapply_is_refused_after_a_new_commit_and_by_a_local_change_on_its_paths`)
  [@test] crates/git-ops/src/unapply_test.rs (`the_undo_of_an_unapply_that_is_not_the_last_move_is_refused`)

#### Scenario: A local change on a path the undo writes
- **WHEN** a local change, staged or not, sits on a path the undo writes
- **THEN** the undo is refused and nothing changes
  [@test] crates/sync-core/src/unapply_branch_test.rs (`the_undo_of_an_unapply_is_refused_after_a_new_commit_and_by_a_local_change_on_its_paths`)

#### Scenario: Undoing again
- **WHEN** an undone unapply is undone again
- **THEN** it is no error and nothing changes

### Requirement: `brd`: `brd unapply` unapplies as the app does

`brd unapply <branch> [--dry-run] [--expect-head <id>]` MUST unapply as the app does and print one object (`unapply`
in `brd schema`) with the unapplied branch, the commits, both heads and the `undo_token`; `brd undo` of it MUST undo it
(`restored_branch`, `already_undone`).

#### Scenario: `brd unapply` and `brd undo`
- **WHEN** an agent runs `brd unapply <branch>`, then `brd undo` of its token
- **THEN** the branch and its files leave HEAD, and the undo brings them back
- **AND** the outputs are the objects of `brd schema`
  [@test] crates/bd-update/src/unapply_test.rs (`unapply_takes_the_branch_and_its_files_out_of_head_and_the_undo_brings_them_back`, `unapply_outputs_match_the_schema`)

### Requirement: `brd`: A dry run of `brd unapply` writes nothing but git objects

`--dry-run` MUST print the object of the unapply it would do and write nothing but git objects: no ref, no mapping
note, no config, no index and no file, no lock and no record in the operations registry, as the dry run of `brd
reapply`; a refusal is the unapply's (`uncommitted_overlap` too).

#### Scenario: `brd unapply --dry-run`
- **WHEN** an agent runs `brd unapply <branch> --dry-run`
- **THEN** it prints the same object of the unapply it would do (`dry_run: true`, the name of the unapplied branch, the commits that would leave HEAD, the HEAD it would move to, no `undo_token`)
- **AND** it writes nothing but git objects
  [@test] crates/bd-update/src/unapply_test.rs (`a_dry_run_of_an_unapply_writes_nothing_and_says_what_it_would_do`)

### Requirement: `brd`: The refusals of `brd unapply` are codes

Its refusals MUST be codes: `unknown_branch` (exit 2), `uncommitted_overlap` with `paths`, `branch_not_built`,
`branch_checked_out` and `drop_conflict` (exit 4).

#### Scenario: An unknown branch, one not built, local changes in the way
- **WHEN** `brd unapply` names an unknown branch, one not built, or local changes sit in the way
- **THEN** it refuses with the code of each, `unknown_branch` (exit 2), `branch_not_built`, `uncommitted_overlap` with `paths`
  [@test] crates/bd-update/src/unapply_test.rs (`unapply_refuses_an_unknown_branch_one_not_built_and_local_changes_in_the_way`)

### Requirement: `brd`: `brd status` lists the unapplied branches

`brd status` MUST list the unapplied branches (`unapplied`: name, branch, tip, subject, the remote head and whether it
is up to date), from the listing it read.

#### Scenario: `brd status` with an unapplied branch
- **WHEN** an agent runs `brd status` in a repository with an unapplied branch
- **THEN** its `unapplied` lists it with its name, branch, tip, subject, remote head and whether it is up to date
  [@test] crates/bd-update/src/unapply_test.rs (`status_lists_an_unapplied_branch_push_refuses_it_by_its_name_and_pushes_it_with_unapplied`)

### Requirement: `brd`: `brd push --unapplied` pushes an unapplied branch under its own name

`brd push` and `brd safe-push` MUST refuse the name of one as `unapplied` (exit 4), and `brd push --unapplied <name>`
MUST push it under its own name, as the app's push of it (the intent `Push` with the scope `unapplied`, after a sync):
leased on the remote head the sync saw (or `--expect-remote`), a push that drops commits of its remote branch it lacks
refused as `push_refused` until `--force`.

#### Scenario: By its name and with `--unapplied`
- **WHEN** an agent runs `brd push <name>` of an unapplied branch, then `brd push --unapplied <name>`
- **THEN** the first is refused as `unapplied` (exit 4), the second pushes it under its own name
  [@test] crates/bd-update/src/unapply_test.rs (`status_lists_an_unapplied_branch_push_refuses_it_by_its_name_and_pushes_it_with_unapplied`)

#### Scenario: The remote branch has commits it lacks
- **WHEN** the remote branch has commits the unapplied branch lacks
- **THEN** `brd push --unapplied` refuses as `push_refused`, and pushes with `--force`
  [@test] crates/bd-remote/src/push_test.rs (`an_unapplied_branch_whose_remote_has_commits_it_lacks_is_pushed_only_with_force`)

### Requirement: `brd`: The agent help unapplies only when the user asks

`brd help agent` and the skill MUST tell an agent to unapply only when the user asks.

#### Scenario: The help of an agent
- **WHEN** an agent reads `brd help agent` or the skill
- **THEN** it says to unapply only when the user asks
  [@test] crates/bd-cli/src/agent_help_test.rs (`agent_help_names_every_command_and_option_that_exists`)

### Requirement: Process budget: An unapply costs the processes of a drop

An unapply MUST cost the processes `a_drop` pins: the grouping read once, one `replay` per kept run, one dry run of
the files and one move; its undo those of `an_undo_of_an_unapply`.

#### Scenario: An unapply and its undo
- **WHEN** an unapply runs, then its undo
- **THEN** they start the processes `a_drop` and `an_undo_of_an_unapply` pin
  [@test] crates/sync-core/src/process_budget_test.rs (`a_drop`, `an_undo_of_an_unapply`)

### Requirement: Process budget: A dry run of an unapply costs what a drop reads and computes

A dry run of an unapply MUST cost what `a_drop` reads and computes before the lock, without the witness, the move
and the mapping notes.

#### Scenario: A dry run of an unapply
- **WHEN** a dry run of an unapply runs
- **THEN** it starts the processes `a_dry_run_of_a_drop` pins
  [@test] crates/sync-core/src/process_budget_test.rs (`a_dry_run_of_a_drop`)

### Requirement: Process budget: A reapply costs the processes it pins

A reapply MUST cost the processes `a_reapply` pins (one `fast-import` for the restored messages, one `replay`, one
dry run of the files, one move); its undo those of `an_undo_of_a_reapply`.

#### Scenario: A reapply and its undo
- **WHEN** a reapply runs, then its undo
- **THEN** they start the processes `a_reapply` and `an_undo_of_a_reapply` pin
  [@test] crates/sync-core/src/process_budget_test.rs (`a_reapply`, `an_undo_of_a_reapply`)

## User Experience

- Unapply… → the confirmation below the row ("This removes 2 commits and their changes from HEAD"), Enter → the row
  leaves the table, the files with it → toast "Unapplied docs" · "Removed 1 commit from HEAD · kept in
  jane/unapplied/docs" · Undo.
- The Unapplied card: "Unapplied · 1 branch", the row "docs · Document the API · Not pushed"; Push Branch → toast "Pushed branch docs"
  · "The remote keeps it as jane/unapplied/docs"; Reapply → the row back in the branch table → toast "Reapplied docs"
  · "1 commit back on top of HEAD" · Undo.
- `brd unapply docs` → `{"unapplied_branch":"jane/unapplied/docs",…,"undo_token":"unapply:…"}`; `brd undo unapply:…`;
  `brd status` → `"unapplied":[{"name":"jane/unapplied/docs","branch":"docs",…}]`; `brd push --unapplied docs`;
  `brd reapply docs` → `{"reapplied":[…],"prefix_restored":1,"undo_token":"reapply:…"}`.

## Open Questions / Risks

- `git replay` reports a conflict of the unapply's drop only by its exit code, so its error cannot yet name the
  conflicting commit or files; the reapply's can (the binary search of the pick).
- A name that got a `-N` suffix at the unapply (`docs-1`) comes back under that name: the prefix restored is the one
  of the unapplied branch. A `(parent/child)` prefix comes back as `(child)`; the stack parents
  (`refs/branch-deck/stack-parents`) stack it again where they name its parent.
- A sync between a reapply and its undo builds the virtual branch; after the undo the next sync archives it.

## References

- [Git Operations](../../../../docs/design/git-operations.md) ("Drop and unapply")
- [Virtual Branches](../../../../docs/design/virtual-branches.md)
- [Parked Branches](../parked-branches/spec.md): the branches kept local in the build
