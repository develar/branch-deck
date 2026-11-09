---
name: Parked Branches
description: Requirements for parked branches, the branches a user keeps on the local branch without pushing or landing them, stored in `refs/branch-deck/parked`, refused by every push and kept out of Your Move.
style: plain-1
targets:
  - crates/history-rewrite/src/parked.rs
  - crates/history-rewrite/src/store_lines.rs
  - crates/history-rewrite/src/snapshot.rs
  - crates/history-rewrite/src/regroup.rs
  - crates/sync-core/src/parked.rs
  - crates/domain-policy/src/parked.rs
  - crates/domain-policy/src/facts.rs
  - crates/repository-model/src/derive.rs
  - crates/domain-types/src/session.rs
  - crates/sync-core/src/sync.rs
  - crates/sync-core/src/sync_inputs.rs
  - crates/domain-types/src/lib.rs
  - crates/bd-rewrite/src/park_command.rs
  - crates/bd-remote/src/status.rs
  - crates/bd-remote/src/push_command.rs
  - crates/bd-remote/src/safe_push_command.rs
  - crates/repository-model/src/push_guard.rs
  - crates/domain-types/src/error.rs
  - crates/bd-cli/src/agent_help.rs
  - crates/operations/src/rewrite.rs
  - src/lib/rewrite-run.ts
  - src/lib/parked.ts
  - src/stores/branch-sync.ts
  - src/model/structure.ts
  - src/presenters/next-step.ts
  - src/presenters/landing-node.ts
  - src/lib/branch-filter.ts
  - src/hooks/useLandingOverview.ts
  - src/presenters/push-button.ts
  - src/hooks/useBranchPushes.ts
  - src/hooks/useBranchContextActions.ts
  - src/hooks/useParkBranch.ts
  - src/components/branch/ParkedBranchesCard.tsx
  - src/components/branch/BranchDropZones.tsx
  - src/components/branch/DragTargetBar.tsx
  - src/components/branch/BranchSelectionBar.tsx
  - src/hooks/useBranchDropTarget.ts
  - src/lib/branch-drop.ts
  - src/components/branch/InlineParkBranchInput.tsx
  - src/components/branch/BranchRowStatus.tsx
  - src/presenters/rewrite.ts
  - crates/operations/src/push.rs
---

# Parked Branches

Status: Implemented
Date: 2026-10-20

## Purpose

Some commits stay on the local branch only because they must not land yet: they wait for a proper fix elsewhere, a
workaround the user needs locally, a change kept for a later review. Before, such a branch looked like any other: a
filled node, a move in Your Move, a Safe Push offered on its row. A user had to remember not to press it, and an agent
running `brd push --all` pushed it.

A parked branch is a virtual branch the user keeps local. Its commits stay where they are, and the sync builds it as
before, so its conflicts still show. No push sends it, no robot lands it, and it never counts as the user's move. A
branch built on it carries its commits, so it is parked with it.

## Goals

- Keep a branch local by one action, with an optional note that says why, and let it go by another.
- Make no push, Safe Push or Dry Run send it, in the app and in `brd`, and so no agent.
- Keep it out of every count of the user's moves.
- Store it as the other per-branch facts of Branch Deck: shared by every worktree, versioned, never pushed.

## Non-goals

- Park one commit. The unit is the virtual branch; a commit is moved into a branch of its own first (Move to Branch).
- Take the commits out of HEAD. That is unapply ([Unapply](../unapply/spec.md)); a parked branch stays in the build.
- Stop a merge on GitHub of a pull request that is open already, or a robot that runs: parking is no remote state.
- Push or fetch the store. A new clone or another machine does not see the parked branches.

## Requirements

### Requirement: Store: The parked branches live in `refs/branch-deck/parked`

The parked branches MUST live in `refs/branch-deck/parked`: a commit whose tree holds the file `parked`, one
`branch<TAB>note` line per branch, sorted, the note one line or empty.

#### Scenario: A park writes the line of its branch into the store
- **WHEN** a branch is parked with a note
- **THEN** `refs/branch-deck/parked` is a commit whose tree holds the file `parked`
- **AND** the file holds one `branch<TAB>note` line per branch, sorted, the note one line or empty
  [@test] crates/sync-core/src/parked_test.rs (`park_writes_a_line_without_a_rewrite_and_unpark_drops_it`)

### Requirement: Store: Each write is a new commit on the previous one

Each write of the store MUST be a new commit on the previous one.

#### Scenario: Three writes are three commits of the store
- **WHEN** the store is written three times
- **THEN** each write is a new commit on the previous one
  [@test] crates/sync-core/src/parked_test.rs (`park_writes_a_line_without_a_rewrite_and_unpark_drops_it`)

### Requirement: Store: The store is read with the issue groups and the stack parents

The store MUST be read in the `cat-file --batch` of the issue groups and the stack parents, so the grouping inputs
still cost two processes, and the sync SHALL send the map in `BranchesGrouped`.

#### Scenario: A sync reads the store in the batch of the other stores
- **WHEN** a sync that changes nothing runs
- **THEN** the store is read in the `cat-file --batch` of the issue groups and the stack parents
- **AND** the grouping inputs still cost two processes
  [@test] crates/sync-core/src/process_budget_test.rs (`an_unchanged_sync`)

#### Scenario: A park reads the stores in one batch
- **WHEN** a park or an unpark writes the store
- **THEN** the stores are read in one `cat-file --batch`
  [@test] crates/sync-core/src/process_budget_test.rs (`a_park_and_an_unpark`)

#### Scenario: The sync sends the map
- **WHEN** a sync groups the branches
- **THEN** it sends the map of the parked branches in `BranchesGrouped`

### Requirement: Store: A line that is no branch name or a repeated branch is a problem

A line whose key is not a branch name, or a branch listed twice, MUST be a problem: never used, and kept as it is by a
write of another key.

#### Scenario: A write of another key keeps the problem line
- **GIVEN** the store holds a line that is not a branch line
- **WHEN** another branch is unparked
- **THEN** the line is a problem, never used
- **AND** the write keeps it as it is
  [@test] crates/sync-core/src/parked_test.rs (`unpark_drops_the_line_of_a_branch_without_commits_and_keeps_other_lines_as_they_are`)

### Requirement: Store: The store decides no group

The store MUST decide no group: a sync builds a parked branch as any other.

#### Scenario: A sync builds a parked branch
- **GIVEN** a parked branch
- **WHEN** a sync runs
- **THEN** it builds the virtual branch of the parked branch as any other
  [@test] crates/sync-core/src/parked_test.rs (`a_sync_keeps_every_line_it_can_not_prove_dead`)

### Requirement: Store: The ref is in the inputs of a sync

The ref MUST be in the inputs of a sync, so a park in `brd` syncs the window on its next focus.

#### Scenario: A park in `brd` changes the inputs of a sync
- **WHEN** `brd` parks a branch
- **THEN** the inputs of a sync change, so the window syncs on its next focus
  [@test] crates/sync-core/src/sync_inputs_test.rs (`changes_with_what_a_sync_reads_and_not_with_what_it_writes`)

### Requirement: Park and unpark: A park writes the line of a branch and rewrites no commit

A park MUST write the line of a branch that groups commits ahead of the baseline, and MUST rewrite no commit.

#### Scenario: A park writes the line, HEAD stays
- **GIVEN** a branch that groups commits ahead of the baseline
- **WHEN** it is parked
- **THEN** its line is written into the store
- **AND** no commit is rewritten, HEAD stays where it was
  [@test] crates/sync-core/src/parked_test.rs (`park_writes_a_line_without_a_rewrite_and_unpark_drops_it`)

#### Scenario: A branch that groups no commit is refused
- **WHEN** a park names a branch that groups no commits ahead of the baseline
- **THEN** it is refused and the store is not written
  [@test] crates/sync-core/src/parked_test.rs (`park_refuses_an_unknown_branch_and_a_note_of_two_lines_unpark_one_not_parked`)

### Requirement: Park and unpark: A park holds the repository lock only for its one ref

A park MUST write its store commit before the repository lock and move it under the lock with compare-and-swap on the
commit it read, the one ref it writes.

#### Scenario: Only the move of the store ref runs under the lock
- **WHEN** a park or an unpark writes the store
- **THEN** its store commit is written before the repository lock
- **AND** under the lock only the store ref moves, with compare-and-swap on the commit it read
  [@test] crates/history-rewrite/src/regroup_test.rs (`a_park_and_an_unpark_lock_only_the_write`)

### Requirement: Park and unpark: A store a sync changed after the read is written once more

A store a sync dropped lines of after the read MUST be read and written once more under the lock.

#### Scenario: A sync drops a line between the read and the lock
- **GIVEN** a sync dropped lines of the store after the park read it
- **WHEN** the park takes the lock and its compare-and-swap fails
- **THEN** the store is read and written once more under the lock, on the commit of the drop
  [@test] crates/history-rewrite/src/regroup_test.rs (`a_park_over_a_drop_after_its_read_is_written_once_more_under_the_lock`)

### Requirement: Park and unpark: A park of a parked branch changes its note

A park of a parked branch MUST change its note; the same line again MUST write nothing.

#### Scenario: A new note replaces the old one
- **GIVEN** a parked branch
- **WHEN** it is parked with another note
- **THEN** the note changes
  [@test] crates/sync-core/src/parked_test.rs (`park_writes_a_line_without_a_rewrite_and_unpark_drops_it`)

#### Scenario: The same line again writes nothing
- **GIVEN** a parked branch
- **WHEN** it is parked with the same note
- **THEN** nothing is written
  [@test] crates/sync-core/src/parked_test.rs (`park_writes_a_line_without_a_rewrite_and_unpark_drops_it`)

### Requirement: Park and unpark: The note is one trimmed line of 500 characters at most

The note MUST be trimmed; a note of two lines or of more than 500 characters MUST be refused.

#### Scenario: The note is trimmed
- **WHEN** a park gives a note with spaces around it
- **THEN** the stored note is trimmed
  [@test] crates/sync-core/src/parked_test.rs (`park_writes_a_line_without_a_rewrite_and_unpark_drops_it`)

#### Scenario: Two lines or more than 500 characters are refused
- **WHEN** a park gives a note of two lines or of more than 500 characters
- **THEN** it is refused and the store is not written
  [@test] crates/sync-core/src/parked_test.rs (`park_refuses_an_unknown_branch_and_a_note_of_two_lines_unpark_one_not_parked`)

### Requirement: Park and unpark: Several branches are written in one store commit or none

A park or an unpark of several branches (the app's drag of a selection, an undo) MUST write all their lines in one
store commit, or none: an unknown branch refuses the park of all, one that is not parked the unpark of all. The
processes MUST NOT grow with the number of branches.

#### Scenario: Several branches park and unpark in one store commit
- **WHEN** several branches are parked, or unparked, together
- **THEN** all their lines are written in one store commit
  [@test] crates/sync-core/src/parked_test.rs (`several_branches_park_and_unpark_in_one_store_commit_or_not_at_all`)

#### Scenario: An unknown branch refuses the park of all, one not parked the unpark of all
- **WHEN** a park of several names an unknown branch, or an unpark of several names one that is not parked
- **THEN** no line is written
  [@test] crates/sync-core/src/parked_test.rs (`several_branches_park_and_unpark_in_one_store_commit_or_not_at_all`)

#### Scenario: The processes don't grow with the number of branches
- **WHEN** two branches are parked, then unparked
- **THEN** each write runs the processes of a store write of one branch
  [@test] crates/sync-core/src/process_budget_test.rs (`a_park_and_an_unpark`)

### Requirement: Park and unpark: `brd park` waits for the repository lock up to 10 s

A park MUST wait for the repository lock as a rewrite does: `brd park` up to 10 s, then `locked`.

#### Scenario: Another process holds the lock past the wait
- **GIVEN** another process holds the repository lock
- **WHEN** `brd park` waits for it past its bound
- **THEN** it fails with `locked` and the store is not written
  [@test] crates/sync-core/src/parked_test.rs (`park_waits_for_the_repository_lock`)

### Requirement: Park and unpark: The app's park waits as long as the holder lives

The app's park and unpark (the rewrites `park` and `unpark` of the engine, `operations::rewrite`) MUST wait for the
repository lock as long as the holder lives, the row's "⋯" saying whom for with Stop Waiting (`cancel_operation`),
which ends the wait with nothing changed.

#### Scenario: The rewrite says whom it waits for and runs after the release
- **GIVEN** another process holds the repository lock
- **WHEN** the app parks or unparks a branch
- **THEN** the row's "⋯" says whom it waits for
- **AND** the rewrite runs once the holder released the lock
  [@test] crates/test-server/src/invoke_test.rs (`a_rewrite_says_whom_it_waits_for_and_runs_once_the_holder_released_the_lock`)

#### Scenario: Stop Waiting ends the wait
- **WHEN** the user chooses Stop Waiting (`cancel_operation`) while the park waits
- **THEN** the wait ends with nothing changed
  [@test] crates/test-server/src/invoke_test.rs (`stop_waiting_ends_the_wait_of_a_rewrite_with_nothing_changed`)

### Requirement: Park and unpark: An unpark drops the line

An unpark MUST drop the line, also of a branch that groups no commit here; a branch that is not parked MUST be
`not_parked`.

#### Scenario: The line of a branch without commits here is dropped
- **GIVEN** the line of a branch that groups no commit here (one of another worktree)
- **WHEN** it is unparked
- **THEN** its line is dropped, every other line kept as it is
  [@test] crates/sync-core/src/parked_test.rs (`unpark_drops_the_line_of_a_branch_without_commits_and_keeps_other_lines_as_they_are`)

#### Scenario: A branch that is not parked
- **WHEN** an unpark names a branch that is not parked
- **THEN** it is refused with `not_parked`
  [@test] crates/sync-core/src/parked_test.rs (`park_refuses_an_unknown_branch_and_a_note_of_two_lines_unpark_one_not_parked`)

### Requirement: Park and unpark: The rewrites report the notes before and after

The rewrites `park` and `unpark` MUST report the note before and after.

#### Scenario: A park and an unpark report the notes
- **WHEN** a branch is parked with a note, then unparked
- **THEN** each result names the note before and the note after
  [@test] crates/sync-core/src/parked_test.rs (`the_result_of_a_park_reports_the_notes`)

### Requirement: Park and unpark: The undo token moves the store back with compare-and-swap

The rewrites `park` and `unpark` MUST report the undo token of what they wrote: the store back to the commit it was
written on (`parked:P1..P2`, compare-and-swap), so a later write of the store makes the Undo stale instead of losing
that write; only the drops of a sync after it are passed over (`undo_regroup_core`, as the undo of a rename does).

#### Scenario: The Undo of a park moves the store back
- **WHEN** a park is undone by its token
- **THEN** the token is `parked:P1..P2`
- **AND** the store moves back to the commit it was written on
  [@test] crates/operations/src/rewrite_test.rs (`an_undo_during_a_rewrite_waits_behind_it_and_runs_after_it`)

#### Scenario: A later write makes the Undo stale
- **GIVEN** the store was written again after the write a token moves back, by a park and not by the drops of a sync
- **WHEN** the token is undone
- **THEN** the compare-and-swap refuses it with `parked_moved`, which an undo answers as stale (`undo_stale`), never
  retried, and the later write is kept
  [@test] crates/sync-core/src/parked_test.rs (`an_undo_of_a_rename_passes_over_the_drops_of_a_sync`)
  [@test] crates/domain-types/src/error_test.rs (`an_undo_that_a_moved_ref_refuses_is_stale_and_never_retried`)

### Requirement: Parked with a parent: A branch built on a parked branch is parked too

A branch built on a parked branch (its effective base, or the declared parent of a blocked branch, up the chain) MUST
be parked too, with the note of the nearest parked ancestor, which it names. Only the parked branch itself SHALL have a
line.

#### Scenario: The branches up the chain are parked with the note of the nearest parked ancestor
- **GIVEN** a parked branch, a branch built on it, a branch built on that one, and a blocked branch whose declared
  parent is in the chain
- **WHEN** the parked state of each is read
- **THEN** each is parked with the note of the nearest parked ancestor, which it names
- **AND** only the parked branch itself has a line; a branch apart is not parked
  [@test] crates/domain-policy/src/parked_test.rs (`a_branch_built_on_a_parked_one_is_parked_with_it`)

### Requirement: Parked with a parent: The chain is the one the commits are built on

The chain MUST be the one its commits are built on, not the declared one: a branch whose declared parent has no commits
is built on that parent's parent, carries its commits, and is parked with it; a chain that loops MUST park nothing.

#### Scenario: A declared parent without commits
- **GIVEN** a branch whose declared parent has no commits, built on that parent's parent, which is parked
- **WHEN** its parked state is read
- **THEN** it carries the commits of that parent's parent and is parked with it
  [@test] crates/domain-policy/src/parked_test.rs (`a_branch_is_parked_by_the_chain_it_is_built_on_not_the_declared_one`)

#### Scenario: A chain that loops
- **GIVEN** two branches built on each other
- **WHEN** their parked state is read
- **THEN** the loop parks nothing
  [@test] crates/domain-policy/src/parked_test.rs (`a_branch_is_parked_by_the_chain_it_is_built_on_not_the_declared_one`)

### Requirement: Parked with a parent: The app and `brd` read one rule

The app and `brd` MUST read one rule (`domain_policy::parked`): `brd status`, `brd push` and `brd safe-push` and the
app's checks in Rust ask the same rule over their grouping.

#### Scenario: `brd` and the app's checks ask the same rule
- **WHEN** `brd status`, `brd push`, `brd safe-push` or a check of the app in Rust needs the parked state of a branch
- **THEN** it asks `domain_policy::parked` over its grouping

### Requirement: Parked with a parent: The session's view places each branch with its parked state

The session's view MUST place each branch with its parked state and its section (`BranchView.structure`), and the
stack branch of a whole stack with the first of its members that is parked (`stackParked` of its root).

#### Scenario: Every branch is placed with its section
- **WHEN** the session derives the view of a grouping, or a patch of it
- **THEN** each branch is placed with its parked state and its section in `BranchView.structure`
  [@test] crates/repository-model/src/derive_test.rs (`a_grouping_places_every_branch_with_its_section`, `a_patch_carries_only_the_structures_that_changed`)

#### Scenario: The first parked member keeps a stack branch local
- **GIVEN** a whole stack one of whose members is parked
- **WHEN** the parked state of its stack branch is read
- **THEN** it is the first of its members that is parked (`stackParked` of its root)
  [@test] crates/domain-policy/src/parked_test.rs (`the_first_parked_member_keeps_a_stack_branch_local`)

### Requirement: Parked with a parent: The page splits the rows by the section and decides nothing

The page MUST split the order of the table into the rows of each section by the section of each branch's structure,
and MUST decide nothing.

#### Scenario: The rows of each section come from the one order
- **WHEN** the view sends the order of the table and the structure of each branch
- **THEN** the page splits the order into the rows of each section by it, and the view sends no second list
  [@test] src/model/store.test.ts (`splits the order into the rows of each section by the section of each branch's structure, the view sends no second list`)

### Requirement: Rename and cleanup: A rename carries the line to the new name

A rename MUST carry the line to the new name in its regroup transaction; the undo token gets `parked:P1..P2` after
`stacks:`, and its undo MUST move the line back.

#### Scenario: A rename of a parked branch and its undo
- **GIVEN** a parked branch
- **WHEN** it is renamed
- **THEN** its line moves to the new name in the regroup transaction
- **AND** the undo token gets `parked:P1..P2` after `stacks:`, and its undo moves the line back
  [@test] crates/sync-core/src/parked_test.rs (`a_rename_carries_the_line_and_its_undo_moves_it_back`)

### Requirement: Rename and cleanup: A merge into a parked branch keeps the note of the target

A rename that merges a branch into a parked branch MUST keep the note of the target.

#### Scenario: A rename into a parked branch
- **GIVEN** two parked branches
- **WHEN** one is renamed to the name of the other
- **THEN** the store keeps the line of the target with its note
  [@test] crates/sync-core/src/parked_test.rs (`a_rename_carries_the_line_and_its_undo_moves_it_back`)

### Requirement: Rename and cleanup: A rename of a branch that is not parked writes no line

A rename of a branch that is not parked MUST write no line.

#### Scenario: A rename of an unparked branch
- **WHEN** a branch that is not parked is renamed
- **THEN** the store is not written
  [@test] crates/sync-core/src/parked_test.rs (`a_rename_carries_the_line_and_its_undo_moves_it_back`)

### Requirement: Rename and cleanup: A sync drops the line of an integrated branch

A sync MUST drop the line of a branch without commits here once its newest archived branch is integrated, as the stack
parents (`integrated_children`), best effort and with its own drop message; every other line MUST stay.

#### Scenario: A sync keeps every line it cannot prove dead
- **GIVEN** the store holds the line of a branch here and of one without commits here
- **WHEN** a sync runs
- **THEN** every line stays
  [@test] crates/sync-core/src/parked_test.rs (`a_sync_keeps_every_line_it_can_not_prove_dead`)

#### Scenario: The line of an integrated branch goes
- **GIVEN** a line of a branch without commits here whose newest archived branch is integrated
- **WHEN** the sync drops the lines of the integrated branches
- **THEN** that line goes in a write with its own drop message, every other line stays
  [@test] crates/sync-core/src/parked_test.rs (`a_sync_keeps_every_line_it_can_not_prove_dead`)

### Requirement: Rename and cleanup: An undo of a rename passes over the drops of a sync

An undo of a rename MUST pass over the drops of a sync; a park after the rename MUST make it `parked_moved`.

#### Scenario: A sync dropped a line after the rename
- **GIVEN** a rename of a parked branch, then a sync that dropped another line
- **WHEN** the rename is undone
- **THEN** the rename is undone and the drop stays
  [@test] crates/sync-core/src/parked_test.rs (`an_undo_of_a_rename_passes_over_the_drops_of_a_sync`)

#### Scenario: A park after the rename
- **GIVEN** a rename of a parked branch, then a park of it
- **WHEN** the rename is undone
- **THEN** the undo is refused with `parked_moved`
  [@test] crates/sync-core/src/parked_test.rs (`an_undo_of_a_rename_passes_over_the_drops_of_a_sync`)

### Requirement: `brd`: `brd park` and `brd unpark` print one object

`brd park <branch> [--note]` and `brd unpark <branch>` MUST print one object (`park` in `brd schema`).

#### Scenario: A park and an unpark print the note
- **WHEN** an agent runs `brd park <branch> [--note]`, then `brd unpark <branch>`
- **THEN** each prints one object, `park` in `brd schema`, with the note
  [@test] crates/bd-rewrite/src/park_test.rs (`park_and_unpark_print_the_note_and_status_names_the_parked_branches`)

### Requirement: `brd`: `brd status` names the parked branches

`brd status` MUST name `parked: {note, with}` per branch and `idle_parked`, `--brief` too.

#### Scenario: The status of a parked branch
- **GIVEN** a parked branch
- **WHEN** an agent runs `brd status`, with `--brief` or without
- **THEN** it names `parked: {note, with}` of the branch and `idle_parked`
  [@test] crates/bd-rewrite/src/park_test.rs (`park_and_unpark_print_the_note_and_status_names_the_parked_branches`)

### Requirement: `brd`: `brd push` refuses a named parked branch

`brd push` MUST refuse a named parked branch, or one built on a parked branch, with `parked` (exit 4), nothing pushed.

#### Scenario: A push of a parked branch or one built on it
- **WHEN** `brd push` names a parked branch, or one built on a parked branch
- **THEN** it is refused with `parked` (exit 4) and nothing is pushed
  [@test] crates/bd-remote/src/push_test.rs (`a_parked_branch_and_what_is_built_on_it_are_never_pushed`)

### Requirement: `brd`: `brd push --all` leaves the parked branches out

With `--all` or `--with-children`, `brd push` MUST leave a parked branch out (`skipped`, `code: parked`) and push the
others, and refuse only when nothing else is left.

#### Scenario: The others are pushed
- **WHEN** `brd push --all` or `--with-children` reaches a parked branch
- **THEN** it leaves it out (`skipped`, `code: parked`) and pushes the others
  [@test] crates/bd-remote/src/push_test.rs (`a_parked_branch_and_what_is_built_on_it_are_never_pushed`)

#### Scenario: Nothing else is left
- **WHEN** every branch `brd push --all` or `--with-children` reaches is parked
- **THEN** it refuses
  [@test] crates/bd-remote/src/push_test.rs (`a_parked_branch_and_what_is_built_on_it_are_never_pushed`)

### Requirement: `brd`: The stack branch of a stack with a parked member stays local

`brd push` MUST keep the stack branch of a stack with a parked member local.

#### Scenario: A push of a stack that holds a parked member
- **GIVEN** a stack with a parked member
- **WHEN** `brd push` reaches its stack branch
- **THEN** the stack branch stays local: its parked member holds it (`StackParked`)
  [@test] crates/domain-policy/src/holds_test.rs (`a_stack_branch_is_held_by_a_parked_member_then_landed_members_then_its_robot_then_no_build`)

### Requirement: `brd`: `brd safe-push` refuses what is parked before Patronus is asked

`brd safe-push` MUST refuse a parked branch, a branch built on one and a whole stack with a parked member with
`parked`, `--dry-run` and `--patronus-dry-run` included, before Patronus is asked.

#### Scenario: A parked branch and a branch built on it start nothing
- **WHEN** `brd safe-push` names a parked branch or a branch built on one, also with `--dry-run` or
  `--patronus-dry-run`
- **THEN** it is refused with `parked` before Patronus is asked, and nothing is pushed
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_parked_branch_and_a_branch_built_on_it_start_nothing`)
  [@test] crates/bd-remote/src/scenario_test.rs (`the_next_step_row_cases_answer_alike_in_the_app_and_bd`)

#### Scenario: A whole stack with a parked member
- **WHEN** `brd safe-push` lands a whole stack with a parked member
- **THEN** it is refused with `parked` before Patronus is asked
  [@test] crates/bd-remote/src/scenario_test.rs (`the_landing_cases_answer_alike_in_the_app_and_bd`)

### Requirement: `brd`: An agent never pushes or unparks a parked branch on its own

`brd help agent` and the skill MUST tell an agent never to push or unpark a parked branch on its own.

#### Scenario: The agent help names park and unpark
- **WHEN** an agent reads `brd help agent` or the skill
- **THEN** it is told never to push or unpark a parked branch on its own
  [@test] crates/bd-cli/src/agent_help_test.rs (`agent_help_names_every_command_and_option_that_exists`)

### Requirement: The app: A parked branch leaves the table for the Parked card

A parked branch, and every branch parked with it, MUST leave the branch table and its stack islands for the Parked card
after them: its rows as in the table (expand, review, the conflict view, the keys and the preview), a stack graph of
their own, a parked child of a branch to land a root there.

#### Scenario: A parked stack root moves into the card with the branches built on it
- **WHEN** a branch is parked
- **THEN** it and every branch parked with it leave the branch table and its stack islands for the Parked card after
  them
- **AND** their rows are as in the table, in a stack graph of their own, a parked child of a branch to land a root there
  [@test] src/stores/branch-table-rows.test.ts (`moves a parked branch and the branches built on it into the Parked card, a stack graph of their own`)
  [@test] tests/e2e/parked-branches/parked-branches.spec.ts (`should park a stack root with the branches built on it, edit its note and unpark it from the menus`)

### Requirement: The app: The Parked card is open by default and its collapse is kept

The Parked card MUST be open by default; its collapse MUST be kept per repository and hide its rows from the keys.

#### Scenario: A collapsed card keeps its rows out of the table
- **GIVEN** the Parked card holds a parked branch
- **WHEN** the user collapses the card
- **THEN** its rows are hidden, from the keys too, and stay out of the table
  [@test] tests/e2e/parked-branches/parked-branches.spec.ts (`should collapse the Parked card and keep its rows out of the table`)

### Requirement: The app: The node of a parked row is `parked`

The node of a parked row MUST be `parked`, hollow and neutral, never a move, also with a conflict; a running robot, a
Safe Push that lands it or a landing SHALL come first.

#### Scenario: A parked row, also with a conflict
- **GIVEN** a parked branch, with a conflict or without
- **WHEN** its node is shown
- **THEN** it is `parked`, hollow and neutral, never a move
  [@test] crates/domain-policy/src/landing_node_test.rs (`the_node_of_the_subject_itself_parked_failing_landed_blocked_or_its_robot_running_stays_through_a_move`)
  [@test] src/presenters/landing-node.test.ts (`words the state of the subject itself: parked, a failure, landed, blocked, its robot running, which a move doesn't replace`)
  [@test] tests/e2e/parked-branches/parked-branches.spec.ts (`should park a branch from its menu with a note, leave it out of every move, and take the park back with Undo`)

#### Scenario: A robot, a Safe Push that lands it or a landing comes first
- **GIVEN** a parked branch whose robot runs, that a Safe Push lands, or that landed
- **WHEN** its node is shown
- **THEN** that state shows instead of `parked`
  [@test] crates/domain-policy/src/landing_node_test.rs (`the_node_of_the_subject_itself_parked_failing_landed_blocked_or_its_robot_running_stays_through_a_move`)

### Requirement: The app: A parked branch is in no unit, no chip but All, and no move

The header, the landing overview, the chips and F2 MUST leave a parked branch out: a parked branch is in no unit, no
chip but All, and no move.

#### Scenario: The counts, the chips and F2 leave it out
- **WHEN** a branch is parked
- **THEN** the header and the landing overview count it in no unit and no move
- **AND** it is in no chip but All, and F2 never stops at it
  [@test] src/lib/branch-filter.test.ts (`puts a parked branch into All only: it has a card of its own, no chip, no move for F2`)
  [@test] tests/e2e/parked-branches/parked-branches.spec.ts (`should park a branch from its menu with a note, leave it out of every move, and take the park back with Undo`)

### Requirement: The app: Nothing of a parked branch starts

A parked branch MUST start nothing: its next step is row 1 of [Next Step](../next-step/spec.md), Safe Push and
Dry Run disabled with "Parked: Unpark it to push" (a branch parked with its parent: "Built on parked auth: Unpark auth
to push"; the whole stack: "Holds parked auth: land the stack in pieces").

#### Scenario: A parked branch and one built on it
- **WHEN** the next step of a parked branch, or of a branch parked with its parent, is shown
- **THEN** it is row 1 of [Next Step](../next-step/spec.md), its step none on every surface
- **AND** Safe Push and Dry Run are disabled with "Parked: Unpark it to push", or "Built on parked auth: Unpark auth to
  push" of a branch parked with its parent
  [@test] src/lib/parked.test.ts (`starts no Safe Push and no Dry Run of a parked branch: its step is none, on every surface`, `says why nothing starts, of a branch and of one built on it`)
  [@test] crates/bd-remote/src/scenario_test.rs (`the_next_step_row_cases_answer_alike_in_the_app_and_bd`)
  [@test] crates/domain-policy/src/next_step_test.rs (`row_1_offers_nothing_while_the_status_cell_says_why_and_disables_both_starts`)

#### Scenario: A start the page did not check is refused in Rust
- **GIVEN** a parked branch, or a branch parked with its parent
- **WHEN** a Safe Push or a Dry Run of it is started
- **THEN** the decision of the start refuses it with `parked` before its first effect, and nothing is pushed or started
  [@test] crates/domain-policy/src/land_test.rs (`the_table_refuses_first_then_the_target_then_a_push_that_drops_remote_commits_unless_allowed`)

#### Scenario: A whole stack that holds a parked branch
- **WHEN** the Safe Push of a whole stack that holds a parked branch is shown
- **THEN** it is disabled with "Holds parked auth: land the stack in pieces"
  [@test] src/presenters/reasons.test.ts (`words every reason of the catalog Rust writes as the page says it`)

### Requirement: The app: The push of a parked branch is disabled with the same reason

The push of a parked branch MUST be disabled with the same reason on the row, in the menu, the preview and the review,
and so MUST Push Up to Here of its commits, their push arrow and their menu
([Stacked Branches](../stacked-branches/spec.md#push-up-to-here)).

#### Scenario: The push and Push Up to Here say why
- **GIVEN** a parked branch, or one parked with its parent
- **WHEN** the user opens its menu, the push arrow of one of its commits, or the menu of the commit
- **THEN** the push and Push Up to Here are disabled with the same reason
  [@test] tests/e2e/parked-branches/parked-branches.spec.ts (`should park a branch from its menu with a note, leave it out of every move, and take the park back with Undo`, `should park a stack root with the branches built on it, edit its note and unpark it from the menus`)

### Requirement: The app: Push with Children leaves the parked children out

Push with Children of a branch to land MUST leave its parked children out, and the stack branch of a stack that holds
one MUST stay local.

#### Scenario: A branch to land with a parked child
- **GIVEN** a branch to land with a parked child
- **WHEN** the user pushes it with its children
- **THEN** the parked child is left out
  [@test] crates/domain-policy/src/push_test.rs (`push_with_children_covers_the_branch_and_its_descendants`)

#### Scenario: The stack branch of a stack that holds a parked child
- **GIVEN** a root to land whose whole stack holds a parked branch
- **WHEN** the user pushes the root with its children
- **THEN** the stack branch of the stack does not go along: its parked member holds it (`StackParked`) and it stays local
  [@test] crates/domain-policy/src/push_test.rs (`the_stack_branch_goes_along_once_published_built_not_held_and_behind`)

### Requirement: The app: Patronus is not asked for the readiness of a parked branch

Patronus MUST NOT be asked for the readiness of a parked branch.

#### Scenario: The step of a parked branch needs no readiness
- **WHEN** the next step of a parked branch is shown
- **THEN** Patronus is not asked for its readiness
  [@test] src/lib/parked.test.ts (`starts no Safe Push and no Dry Run of a parked branch: its step is none, on every surface`)

### Requirement: The app: The app's push refuses a parked branch in Rust too

The app's push MUST refuse it in Rust too, as `brd push` does: the run plans the push again over the table of its
session, after the rewrites admitted before it and the sync (`operations::push::execute_push`), and what keeps the
branch out of a push refuses it before the first effect, parked first. A push of a parked branch, of a branch built on
one, or of the stack branch of a stack that holds one pushes nothing, so a push the page did not check (a race, IPC)
never sends it.

#### Scenario: A push of a parked branch or one built on it
- **WHEN** the app pushes a parked branch, or a branch built on one, the page not checking it
- **THEN** nothing is pushed: "Nothing was pushed: login is parked: IJPL-1", code `parked`
  [@test] crates/domain-policy/src/push_test.rs (`a_branch_is_kept_out_parked_then_landed_then_held_unless_replaced_then_not_built`)

#### Scenario: A push of the stack branch of a stack that holds one
- **WHEN** the app pushes the stack branch of a stack that holds a parked branch
- **THEN** nothing is pushed: "… holds parked login; land the stack in pieces"

### Requirement: The app: An unapplied branch under the name of a parked one pushes

An unapplied branch under the name of a parked one is no branch of the table, and its push MUST NOT be refused as
parked.

#### Scenario: The push of an unapplied branch named as a parked one
- **GIVEN** an unapplied branch under the name of a parked one
- **WHEN** it is pushed
- **THEN** it pushes

### Requirement: The app: The Status cell says "Parked" instead of the push state

The row MUST say it: the Status cell "Parked" or "Parked with auth" (the note in its hover) instead of the push state,
its error or conflict still shown.

#### Scenario: A parked branch and a branch parked with it
- **GIVEN** a parked branch and a branch parked with it
- **WHEN** their rows are shown
- **THEN** the Status cell says "Parked", or "Parked with auth" of the branch parked with it, instead of the push state
- **AND** its hover tells the note; an error or a conflict is still shown
  [@test] tests/e2e/parked-branches/parked-branches.spec.ts (`should park a branch from its menu with a note, leave it out of every move, and take the park back with Undo`, `should park a stack root with the branches built on it, edit its note and unpark it from the menus`)

### Requirement: The app: The name cell of a parked branch shows its note

The name cell of the parked branch MUST show its note instead of the summary, issue keys as links.

#### Scenario: The note instead of the summary
- **GIVEN** a branch parked with a note
- **WHEN** its row is shown
- **THEN** its name cell shows the note instead of the summary, issue keys as links
  [@test] tests/e2e/parked-branches/parked-branches.spec.ts (`should park a branch from its menu with a note, leave it out of every move, and take the park back with Undo`, `should park a stack root with the branches built on it, edit its note and unpark it from the menus`)

### Requirement: The app: The branch menu offers "Park Branch…" beside "Unapply…"

The branch menu (right-click and "⋯") MUST offer "Park Branch…" (a form with the note, one line, optional; Enter
parks) beside "Unapply…", the other way to set a branch aside.

#### Scenario: Park Branch… parks with a note
- **WHEN** the user chooses "Park Branch…" in the menu of a branch row, by right-click or "⋯"
- **THEN** a form with the note opens, one line, optional
- **AND** Enter parks the branch
  [@test] tests/e2e/parked-branches/parked-branches.spec.ts (`should park a branch from its menu with a note, leave it out of every move, and take the park back with Undo`, `should park a stack root with the branches built on it, edit its note and unpark it from the menus`)

### Requirement: The app: The menu of a parked branch offers its note and its unpark

The branch menu of a parked branch MUST offer "Edit Park Note…" and "Unpark Branch"; of a branch parked with its parent
"Unpark ‹parent›".

#### Scenario: A parked branch and a branch parked with its parent
- **GIVEN** a parked stack root and a branch parked with it
- **WHEN** the user opens their menus
- **THEN** the root offers "Edit Park Note…" and "Unpark Branch"
- **AND** the branch parked with it offers "Unpark ‹parent›"
  [@test] tests/e2e/parked-branches/parked-branches.spec.ts (`should park a stack root with the branches built on it, edit its note and unpark it from the menus`)

### Requirement: The app: "Park Branch…" is disabled while a robot lands the branch

"Park Branch…" MUST be disabled while a robot lands the branch; a merged robot that landed it disables nothing.

#### Scenario: A robot lands the branch
- **GIVEN** a robot that runs lands the branch
- **WHEN** the user opens its menu
- **THEN** "Park Branch…" is disabled with the hold of the robot
  [@test] crates/domain-policy/src/actions_test.rs (`a_park_waits_for_a_robot_that_lands_the_branch_and_a_parked_branch_unparks`)

#### Scenario: A merged robot landed the branch
- **GIVEN** a merged robot landed the branch
- **WHEN** the user opens its menu
- **THEN** "Park Branch…" is enabled
  [@test] crates/domain-policy/src/actions_test.rs (`a_park_waits_for_a_robot_that_lands_the_branch_and_a_parked_branch_unparks`)

### Requirement: The app: "Park Branch…" says that an open pull request stays open

"Park Branch…" of a branch with an open pull request MUST say that the pull request stays open: "Keeps it local; #7
stays open, a merge on GitHub still lands it".

#### Scenario: A branch with an open pull request
- **GIVEN** a branch to land whose pull request #7 is open
- **WHEN** the user opens its menu
- **THEN** "Park Branch…" says "Keeps it local; #7 stays open, a merge on GitHub still lands it"

### Requirement: The app: A park is a write of the repository with a toast and its Undo

A park or an unpark MUST be a write of the repository (the session queues it behind a rewrite that runs), followed by a
sync, then a toast "Parked auth" or "Unparked auth" whose Undo moves the store back as it was, by the token of the
park.

#### Scenario: A park, its toast and its Undo
- **WHEN** a branch is parked or unparked from its menu
- **THEN** a sync follows the write, then a toast "Parked auth" or "Unparked auth"
- **AND** its Undo moves the store back as it was, by the token of the park
  [@test] tests/e2e/parked-branches/parked-branches.spec.ts (`should park a branch from its menu with a note, leave it out of every move, and take the park back with Undo`, `should park a stack root with the branches built on it, edit its note and unpark it from the menus`)

#### Scenario: A park while a rewrite runs
- **GIVEN** a rewrite of the repository runs
- **WHEN** the user parks or unparks a branch
- **THEN** the session queues the write behind the rewrite

### Requirement: The app: A park and its Undo belong to the repository of the click

The park, the unpark and their Undo MUST belong to the repository of the click: an Undo clicked after a switch of the
repository moves the store back there (synced by its session), never in the repository shown.

#### Scenario: The Undo clicked after a switch of the repository
- **GIVEN** a branch parked in one repository, then a switch to another
- **WHEN** the user clicks the Undo of the toast
- **THEN** the store moves back in the repository of the park, synced by its session, never in the repository shown
  [@test] tests/e2e/parked-branches/parked-branches.spec.ts (`should take the park back in its repository with the Undo of its toast clicked after a switch of the repository`)

### Requirement: The app: Branches dropped on the park zone or the Parked card are parked at once

Branches dragged from the table onto the park zone of the drag bar, or onto the Parked card, also collapsed (its
header, its rows), MUST be parked at once, without a note, in one write of the store and one toast "Parked core and
docs", whose Undo takes back every line.

#### Scenario: A branch dropped on the park zone of the drag bar
- **WHEN** the user drags a branch from the table onto the park zone of the drag bar
- **THEN** it is parked at once, without a note, and a toast says so
  [@test] tests/e2e/parked-branches/parked-branches.spec.ts (`should park a branch dragged onto the park zone of the drag bar, unpark it dragged onto the unpark zone, take both back with Undo, and drag the commits of a parked row onto a branch`)
  [@test] src/lib/branch-drop.test.ts (`parks the dragged branches that land and leaves the parked ones, refuses one a robot lands, and never waits for a sync`)

#### Scenario: Selected branches dropped on the Parked card, also collapsed
- **WHEN** the user drags selected branches onto a row or the header of the Parked card, open or collapsed
- **THEN** they are parked at once in one write of the store and one toast "Parked core and docs"
- **AND** its Undo takes back every line
  [@test] tests/e2e/parked-branches/parked-branches.spec.ts (`should park the selected branches dropped on a row or the header of the Parked card at once with one Undo, and unpark a stack by its root, never by a branch parked with it`)
  [@test] tests/e2e/parked-branches/parked-branches.spec.ts (`should park an unselected branch dropped on the collapsed Parked card and keep the selection, and park and unpark the selection with its buttons, of both kinds by the part each takes`)

### Requirement: The app: Parked branches dropped on the unpark zone are unparked at once

Parked branches dragged onto the unpark zone of the drag bar MUST be unparked the same way.

#### Scenario: A parked branch dropped on the unpark zone
- **WHEN** the user drags a parked branch onto the unpark zone of the drag bar
- **THEN** it is unparked at once, with a toast whose Undo parks it again
  [@test] tests/e2e/parked-branches/parked-branches.spec.ts (`should park a branch dragged onto the park zone of the drag bar, unpark it dragged onto the unpark zone, take both back with Undo, and drag the commits of a parked row onto a branch`)
  [@test] src/lib/branch-drop.test.ts (`unparks the dragged branches with a line of their own, the ones parked with them along, never one alone, and leaves those that land`)

### Requirement: The app: A branch parked with its parent is unparked only with it

A branch parked with its parent MUST be unparked only with it: dragged alone, the zone refuses it and says why.

#### Scenario: A branch parked with its parent dragged alone
- **GIVEN** a parked stack root and a branch parked with it
- **WHEN** the user drags the branch parked with it alone onto the unpark zone
- **THEN** the zone refuses it and says why
- **AND** the root dragged there unparks its stack
  [@test] tests/e2e/parked-branches/parked-branches.spec.ts (`should park the selected branches dropped on a row or the header of the Parked card at once with one Undo, and unpark a stack by its root, never by a branch parked with it`)
  [@test] src/lib/branch-drop.test.ts (`unparks the dragged branches with a line of their own, the ones parked with them along, never one alone, and leaves those that land`)

### Requirement: The app: A drag of both kinds parks and unparks each on its own zone

A drag of both kinds MUST park the ones that land and unpark the parked ones, each on its own zone.

#### Scenario: A drag of a branch to land and a parked one
- **WHEN** the user drags a branch to land and a parked branch together
- **THEN** the park zone parks the one that lands and leaves the parked one
- **AND** the unpark zone unparks the parked one and leaves the one that lands
  [@test] src/lib/branch-drop.test.ts (`parks the dragged branches that land and leaves the parked ones, refuses one a robot lands, and never waits for a sync`, `unparks the dragged branches with a line of their own, the ones parked with them along, never one alone, and leaves those that land`)

### Requirement: The app: A drop target refuses with the reason of the menu

A target MUST refuse with the reason of the menu ([Move Commits](../move-commits/spec.md#drag-and-drop)).

#### Scenario: A branch a robot lands dropped on the park zone
- **GIVEN** a robot lands the branch
- **WHEN** the user drags it onto the park zone
- **THEN** the zone refuses it with the reason "Park Branch…" of its menu gives
  [@test] src/lib/branch-drop.test.ts (`parks the dragged branches that land and leaves the parked ones, refuses one a robot lands, and never waits for a sync`)

### Requirement: The app: The selection bar parks and unparks the selected branches

The branch selection bar MUST offer Park and Unpark of the selected branches, by the same rules as the zones: Park
while some of them land, Unpark while some are parked, disabled with the reason while they can't now.

#### Scenario: Park and Unpark of a selection of both kinds
- **GIVEN** a selection of a branch to land and a parked branch
- **WHEN** the user presses Park, then Unpark, in the selection bar
- **THEN** Park parks the one that lands and Unpark unparks the parked one
  [@test] tests/e2e/parked-branches/parked-branches.spec.ts (`should park an unselected branch dropped on the collapsed Parked card and keep the selection, and park and unpark the selection with its buttons, of both kinds by the part each takes`)

#### Scenario: A selection with nothing parked offers no Unpark
- **GIVEN** a selection of branches to land only
- **WHEN** the selection bar shows
- **THEN** it offers Park and no Unpark
  [@test] tests/e2e/parked-branches/parked-branches.spec.ts (`should park an unselected branch dropped on the collapsed Parked card and keep the selection, and park and unpark the selection with its buttons, of both kinds by the part each takes`)

#### Scenario: The selected branches can't be parked now
- **WHEN** the selected branches can't be parked or unparked now
- **THEN** the button is disabled with the reason

## User Experience

- Park Branch… → the inline note form in the row ("Why it stays local, e.g. waits for the fix of IJPL-123
  (optional)"), Enter parks → the row moves into the Parked card → toast "Parked docs" · Undo.
- The Parked card: "Parked · 2 branches", a help popover, the rows; the node hollow and grey, Status "Parked", the
  note beside the name.
- Drag a branch row → "Park docs" in the drag bar at the bottom of the window, the Parked card ringed → drop on either
  → toast "Parked docs" · Undo, the row scrolled into view in the card; drag a parked row → "Unpark docs" in the bar.
- Select branches → Park or Unpark in the selection bar.
- `brd park docs --note '…'`, `brd unpark docs`; an agent never parks or unparks on its own.

## Open Questions / Risks

- A park does not convert an open pull request into a draft: a merge on GitHub still lands the branch. The menu says
  so; GitHub has no draft conversion in `crates/github` yet.
- The key of a line is the branch name as the grouping makes it (the leaf of a stacked branch): a renamed issue group
  is carried only by Rename.
