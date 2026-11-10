---
name: Move Commits
description: Requirements for moving commits and whole branches into one branch with the smallest change (a whole issue by the issue groups, the rest by the explicit prefix), from every gesture, renaming and ungrouping a branch, and nesting the branches of an issue.
style: plain-1
targets:
  - crates/history-rewrite/src/move_commits.rs
  - crates/domain-types/src/branch_names.rs
  - src/hooks/useBranchNameValidation.ts
  - src/presenters/branch-name.ts
  - crates/history-rewrite/src/move_plan.rs
  - crates/operations/src/rewrite.rs
  - crates/history-rewrite/src/regroup.rs
  - crates/history-rewrite/src/snapshot.rs
  - crates/history-rewrite/src/repository_lock.rs
  - crates/history-rewrite/src/reword.rs
  - crates/history-rewrite/src/commit_grouper.rs
  - crates/sync-core/src/sync.rs
  - src/hooks/useMoveCommits.ts
  - src/hooks/useMovePreview.ts
  - src/hooks/useStackActions.ts
  - src/lib/rewrite-run.ts
  - src/lib/stack.ts
  - src/lib/move-summary.ts
  - src/lib/branch-selection.ts
  - src/lib/row-selection.ts
  - src/hooks/useRowSelection.ts
  - src/hooks/useCommitMoveList.ts
  - src/hooks/useCommitDrag.ts
  - src/hooks/useBranchDropTarget.ts
  - src/lib/branch-drop.ts
  - src/components/branch/BranchDropZones.tsx
  - src/components/branch/DragTargetBar.tsx
  - src/pages/IndexPage.tsx
  - src/hooks/useBranchContextActions.ts
  - src/hooks/useBranchRowKeys.ts
  - src/components/commit/MovableCommitList.tsx
  - src/components/shared/InlineBranchTarget.tsx
  - src/components/shared/ChoiceChip.tsx
  - src/components/ai/BranchNameSuggestions.tsx
  - src/components/branch/InlineRenameBranchInput.tsx
  - src/components/branch/InlineUngroupConfirm.tsx
  - src/stores/inline-row-action.ts
  - src/components/branch/BranchTableCard.tsx
  - src/components/branch/BranchSelectionBar.tsx
  - src/components/branch/BranchRow.tsx
  - src/components/branch/BranchRowElement.tsx
  - src/components/unassigned/UnassignedCommitListCard.tsx
  - src/components/branch/IssueFamilyRow.tsx
  - src/stores/commit-move.ts
  - src/stores/branch-sync.ts
  - src/presenters/rewrite.ts
---

# Move Commits

Status: Implemented
Date: 2026-10-15

## Purpose

A branch is what ships together: one push, one Safe Push, one pull request. A commit belongs to a branch through its
subject: an explicit `(name)` prefix wins, then the [issue group](../issue-groups/spec.md) of its issue key, then the key
(`ABC-123: …`). One operation, "move these commits to branch X", serves every list, the branch rows, the context menus,
the keyboard and drag and drop, and it makes the smallest change:

- the whole of an issue (every commit of its key without a prefix) moves by the issue groups, without a rewrite, so the
  next commit of the issue goes there too: `IJPL-1`, `IJPL-2` and `IJPL-3` dragged onto `maps` ship as one branch,
  also when `maps` is stacked;
- a part of an issue and a commit without a key get the prefix, so `ABC-123: Add lexer` becomes
  `(lexer) ABC-123: Add lexer` (the issue key stays in the pushed message); a target path `auth/lexer` stacks `lexer` on
  `auth` in the stack parents ([Stack Parents](../stack-parents/spec.md)), so the prefix is the bare `(lexer)`;
- a commit that its key puts into X anyway loses its prefix.

Renaming and ungrouping a branch are moves of all its commits.

## Goals

- Ship several issues as one branch with one gesture, and keep it so for their next commits.
- Split an issue branch into branches that can be pushed separately, without editing messages one by one.
- Move commits, or whole branches, to an existing branch or to a new one with the same gesture.
- Rewrite history only where the grouping needs it.
- Say before the move what it does, and undo it in one step.
- Keep the branches of one issue together in the branch table.

## Non-goals

- Reordering commits. A move rewrites messages only; the commit order and the trees stay the same.
- Detecting before the move that a commit depends on a commit left behind. The next sync shows the conflict.

## Requirements

### Requirement: Move operation: A move makes the smallest regroup

A move MUST make the smallest regroup (`plan_move`), computed from the commits it read before the repository lock,
which the witness under the lock confirms. An issue is whole when the move takes every commit of its key without a prefix, and there is at least one; the fixups of
a moved commit count as moved.

#### Scenario: Whole issues join a branch
- **WHEN** a move takes every commit of one or more issue keys, none of them with a prefix
- **THEN** those issues are whole, and the move is the smallest regroup that puts them into the target
  [@test] crates/sync-core/src/move_commits_test.rs (`whole_issues_join_a_branch_without_a_rewrite_and_their_next_commits_follow`)

#### Scenario: The fixups of a moved commit count as moved
- **GIVEN** a commit of an issue key and a fixup of it
- **WHEN** the move takes the commit, not the fixup
- **THEN** the fixup counts as moved, so the issue is whole
  [@test] crates/sync-core/src/move_commits_test.rs (`the_fixups_of_a_whole_issue_move_with_it`)

### Requirement: Move operation: A whole issue moves by the issue groups

A whole issue MUST move by the issue groups (`KEY -> X`), without a rewrite, also when its key was in another issue
group; a whole issue moved to the branch of its own key leaves its issue group.

#### Scenario: An issue group moves to another branch
- **GIVEN** the keys of an issue group
- **WHEN** their whole issues move to another branch X
- **THEN** their keys get `KEY -> X`, without a rewrite
- **AND** a whole issue moved to the branch of its own key leaves its issue group
  [@test] crates/sync-core/src/move_commits_test.rs (`moving_an_issue_group_reassigns_its_keys_and_moving_it_back_ungroups_them`)

### Requirement: Move operation: A part of an issue and a commit without a key get the prefix

A part of an issue and a commit without a key MUST get the prefix `(X)`, in the same history rewrite and ref
transaction as the issue groups of the move.

#### Scenario: A part of an issue, a whole issue and a commit without a key in one move
- **WHEN** a move to X takes a part of one issue, the whole of another and a commit without a key
- **THEN** the part of the issue and the commit without a key get the prefix `(X)`, the whole issue moves by the issue groups
- **AND** it is one history rewrite and ref transaction, which one undo takes back
  [@test] crates/sync-core/src/move_commits_test.rs (`a_part_of_an_issue_and_a_commit_without_a_key_get_the_prefix_in_the_same_transaction`)

#### Scenario: Commits without a key move to a new branch
- **WHEN** commits without a key move to a new branch
- **THEN** each gets its prefix
  [@test] crates/sync-core/src/move_commits_test.rs (`moves_unassigned_commits_to_a_new_branch`)

### Requirement: Move operation: A commit moved into the branch of an issue is pushed with the key

A commit without a key moved into the branch of an issue (its prefix is the key) MUST be pushed with the key, where
the issue key of a subject is read (after a `[category]` and a `type(scope):`); a subject with a key of its own is
pushed as is.

#### Scenario: A prefix that is an issue key
- **GIVEN** a commit without a key whose prefix is an issue key
- **WHEN** it is copied into the virtual branch
- **THEN** its subject there carries the key, after a `[category]` and a `type(scope):`
  [@test] crates/history-rewrite/src/commit_grouper_test.rs (`a_prefix_that_is_an_issue_key_stays_in_the_virtual_subject_as_the_key`, `the_leaf_of_a_stack_prefix_that_is_an_issue_key_stays_as_the_key`)
  [@test] crates/sync-core/src/branch_processor_test.rs (`a_commit_moved_into_the_branch_of_an_issue_is_copied_with_the_key`)

#### Scenario: A subject with a key of its own
- **GIVEN** a commit with a key of its own whose prefix is another issue key
- **WHEN** it is copied into the virtual branch
- **THEN** its subject is pushed as is
  [@test] crates/history-rewrite/src/commit_grouper_test.rs (`a_key_prefix_adds_no_key_to_a_subject_with_a_key_of_its_own`)

### Requirement: Move operation: Move Back of a commit in an issue branch goes to the unassigned commits

The Move Back item of a commit without a key moved into the branch of an issue MUST move it to the unassigned
commits.

#### Scenario: Move Back of a commit with an issue prefix
- **GIVEN** a commit without a key of its own in the branch of an issue by its prefix
- **WHEN** the user chooses its Move Back item
- **THEN** it moves to the unassigned commits
  [@test] src/hooks/useCommitMoveList.test.ts (`moves a commit of an issue prefix without a key of its own to the unassigned commits`)

### Requirement: Move operation: A commit that its key puts into the target loses its prefix

A commit that its key puts into X without a prefix (an issue branch named X, an issue group X) MUST lose its prefix,
not get `(X)`.

#### Scenario: A prefixed commit moved to the branch of its key
- **GIVEN** a prefixed commit whose key puts it into X without a prefix
- **WHEN** it moves to X
- **THEN** it loses its prefix and does not get `(X)`
  [@test] crates/sync-core/src/move_commits_test.rs (`a_prefixed_commit_that_its_key_puts_into_the_target_loses_the_prefix`)

### Requirement: Move operation: A stacked target takes a whole issue by the issue groups

A stacked target (a path `auth/login`, or a branch with a declared parent) MUST take a whole issue by the issue
groups, not by the prefix; a path writes its stack parent, and a commit of the branch whose prefix declares another
parent gets the bare leaf.

#### Scenario: A whole issue joins a stacked branch, a path writes the stack parent
- **WHEN** a whole issue moves to a branch with a declared parent
- **THEN** it joins it by the issue groups, without a rewrite
- **AND** a move to a path writes its stack parent, and a commit of the branch whose prefix declares another parent gets the bare leaf
  [@test] crates/sync-core/src/move_commits_test.rs (`a_whole_issue_joins_a_stacked_target_without_a_rewrite`)

### Requirement: Move operation: A prefix replaces an existing one

A prefix MUST replace an existing one and keep the rest of the subject and the body.

#### Scenario: A prefixed commit gets another prefix
- **GIVEN** a commit with a prefix and a body
- **WHEN** it gets another prefix
- **THEN** the new prefix replaces the old one, and the rest of the subject and the body stay
  [@test] crates/sync-core/src/move_commits_test.rs (`sets_replaces_and_removes_the_prefix`)

### Requirement: Move operation: A move back removes the explicit prefix

A move without a target (back) MUST remove the explicit prefix: the commit goes back to its issue group, its issue
branch, or to the unassigned commits. A whole issue in an issue group MUST leave it; a part of one MUST get the prefix
of its key.

#### Scenario: A prefixed commit moves back
- **WHEN** a commit with an explicit prefix moves without a target
- **THEN** it loses the prefix and goes back to its issue group, its issue branch, or to the unassigned commits
  [@test] crates/sync-core/src/move_commits_test.rs (`moves_to_another_branch_and_back_to_the_issue`, `sets_replaces_and_removes_the_prefix`)

#### Scenario: A whole issue and a part of one move back out of an issue group
- **GIVEN** an issue in an issue group
- **WHEN** the whole issue moves back
- **THEN** it leaves the issue group
- **AND** when only a part of one moves back, the part gets the prefix of its key
  [@test] crates/sync-core/src/move_commits_test.rs (`moving_an_issue_group_reassigns_its_keys_and_moving_it_back_ungroups_them`)

### Requirement: Move operation: A dry run writes nothing

A dry run MUST say what the move would do and write nothing, without the lock (`preview_move`, the preview of the
Move to… form as the user types).

#### Scenario: A dry run
- **WHEN** a move runs as a dry run
- **THEN** it says what it would do and writes nothing
  [@test] crates/sync-core/src/move_commits_test.rs (`a_dry_run_writes_nothing_and_expect_head_refuses_a_moved_head`)

### Requirement: Move operation: A move may name the HEAD it expects

A move SHALL allow naming the HEAD it expects (`expect_head`); another HEAD MUST refuse it as `HeadMoved`, nothing
written.

#### Scenario: A move names the HEAD it expects
- **GIVEN** a move that names the HEAD it expects
- **WHEN** HEAD is another one
- **THEN** the move is refused as `HeadMoved`, and nothing is written
  [@test] crates/sync-core/src/move_commits_test.rs (`a_dry_run_writes_nothing_and_expect_head_refuses_a_moved_head`)

### Requirement: Move operation: An autosquash marker stays in front

An autosquash marker MUST stay in front: `fixup! (a) X` becomes `fixup! (b) X`.

#### Scenario: A fixup gets another prefix
- **WHEN** `fixup! (a) X` moves to b
- **THEN** it becomes `fixup! (b) X`
  [@test] crates/sync-core/src/move_commits_test.rs (`keeps_the_autosquash_marker`)

### Requirement: Move operation: The fixups of a moved commit move with it

The fixups of a moved commit MUST move with it, and be listed after the requested commits for the undo.

#### Scenario: A commit with fixups moves
- **WHEN** a commit with fixups moves
- **THEN** its fixups move with it
- **AND** the result lists them after the requested commits, for the undo
  [@test] crates/sync-core/src/move_commits_test.rs (`moves_the_fixups_with_their_target`)

### Requirement: Move operation: A move is validated before anything is written

A move is a regroup plan: it MUST be validated against every subject ahead of the baseline before anything is written,
and refused if it introduces a stack problem; also when it recreates a commit that a `fixup! <id>` names by its id,
which would leave the fixup without a target.

#### Scenario: An invalid plan writes nothing
- **WHEN** the plan is invalid against the subjects ahead of the baseline
- **THEN** it is refused before anything is written
  [@test] crates/sync-core/src/regroup_test.rs (`validates_the_plan_before_writing`, `fixups_follow_their_targets`)

#### Scenario: A plan that introduces a stack problem
- **WHEN** the plan introduces a stack problem
- **THEN** it is refused
  [@test] crates/sync-core/src/regroup_test.rs (`refuses_problems_the_plan_introduces_and_keeps_existing_ones`)

#### Scenario: A plan that recreates the target of a fixup by id
- **WHEN** the plan recreates a commit that a `fixup! <id>` names by its id
- **THEN** it is refused, as the fixup would be left without a target
  [@test] crates/sync-core/src/reword_test.rs (`a_reword_that_strands_a_fixup_is_refused`)

### Requirement: Move operation: Rewrites hold the repository lock

Rewrites MUST hold the repository lock, so the app and the agent CLI don't rewrite the branch at the same time.

#### Scenario: Another process holds the lock
- **WHEN** a rewrite starts while another process holds the repository lock
- **THEN** it waits for the lock
  [@test] crates/sync-core/src/regroup_test.rs (`waits_for_the_repository_lock`)

### Requirement: Move operation: A rewrite holds the lock only for the witness and the write

A move, a rename, a restack, an unstack, a reword and their undo MUST read and compute before the lock (the snapshot,
the plan validated, the new commits and store commits written on temporary refs) and hold it only for the witness of
what they read (HEAD, its branch and the store commits) and the compare-and-swap transaction, so a long history never
makes another writer wait.

#### Scenario: Each rewrite locks only the witness and the write
- **WHEN** a move, a rename, a reword, a restack or an undo runs
- **THEN** it holds the lock only for the witness of what it read and the compare-and-swap transaction
  [@test] crates/history-rewrite/src/regroup_test.rs (`a_move_locks_only_the_witness_and_the_write`, `a_rename_locks_only_the_witness_and_the_write`, `a_reword_locks_only_the_witness_and_the_write`, `a_restack_locks_only_the_witness_and_the_write`, `an_undo_locks_only_the_witness_and_the_write`)

### Requirement: Move operation: A ref that moved after the read computes the rewrite once more

A ref that moved after the read (another process rewrote HEAD, a sync dropped the lines of integrated branches) MUST
release the lock and compute the rewrite once more without it, the second time under it: a HEAD the request was not
made for refuses it as `HeadMoved`, a store a sync dropped lines of is read as the sync left it, and no temporary ref
is left.

#### Scenario: The witness missed
- **WHEN** a ref moved after the read
- **THEN** the lock is released and the rewrite is computed once more without it, the second time under it
  [@test] crates/history-rewrite/src/repository_lock_test.rs (`a_witness_that_missed_computes_once_more_without_the_lock`)
  [@test] crates/sync-core/src/repository_lock_test.rs (`a_head_moved_between_two_reads_of_a_rewrite_computes_it_once_more_under_the_lock`)

#### Scenario: Another process rewrote HEAD
- **WHEN** another process rewrote HEAD after the read, and the request was not made for that HEAD
- **THEN** the rewrite is refused as `HeadMoved`
- **AND** no temporary ref is left
  [@test] crates/history-rewrite/src/regroup_test.rs (`a_head_moved_before_the_lock_is_read_once_more`)

#### Scenario: A sync dropped lines of a store
- **WHEN** a sync dropped the lines of integrated branches from a store after the read
- **THEN** the store is read as the sync left it, and the rewrite is written on it
- **AND** no temporary ref is left
  [@test] crates/history-rewrite/src/regroup_test.rs (`a_drop_commit_between_the_snapshot_and_the_transaction_is_read_once_more`, `an_undo_over_a_drop_after_its_checks_is_checked_once_more_under_the_lock`)

### Requirement: Move operation: The moves of the app are rewrites of the engine

A move, a rename, a restack, an unstack and their undo of the app MUST be rewrites of the engine (`execute` of the
intent `rewrite`, `operations::rewrite`), named on the table the window shows (`TableWitness`).

#### Scenario: A move of the app
- **WHEN** the app starts a move, a rename, a restack, an unstack or their undo
- **THEN** it runs as `execute` of the intent `rewrite` (`operations::rewrite`), named on the table the window shows (`TableWitness`)

### Requirement: Move operation: A rewrite waits for the lock as long as its holder lives

Each rewrite of the engine MUST wait for the lock as long as its holder lives (an agent's `brd pick --follow`, `brd`,
the app's own push), never refused after 10 s: its operation's step is `waitingForLock` with the holder. Only a holder
no record names MUST refuse it after 10 s.

#### Scenario: A live holder holds the lock
- **GIVEN** a live holder of the lock (an agent's `brd pick --follow`, `brd`, the app's own push)
- **WHEN** a rewrite of the engine starts
- **THEN** it waits as long as the holder lives, its operation's step `waitingForLock` with the holder
- **AND** it runs once the holder released the lock
  [@test] crates/test-server/src/invoke_test.rs (`a_rewrite_says_whom_it_waits_for_and_runs_once_the_holder_released_the_lock`)
  [@test] crates/operations/src/rewrite_test.rs (`a_rewrite_waits_while_the_holder_of_the_lock_lives_and_stops_when_told`)

#### Scenario: A holder no record names
- **GIVEN** a holder of the lock that no record names
- **WHEN** a rewrite of the engine waits for it
- **THEN** it is refused after 10 s

### Requirement: Move operation: The wait of a move shows on the row and Stop Waiting ends it

A move started from a menu MUST show its spinner on the "⋯" of its row, whose hover says whom it waits for and whose
click offers Stop Waiting (`cancel_operation`), which ends the wait with nothing moved; the surface that started it
MUST say the same.

#### Scenario: The wait of a move started from a menu
- **WHEN** a move started from a menu waits for the lock
- **THEN** the "⋯" of its row shows its spinner, whose hover says whom it waits for ("Waiting for `brd pick` (pid 4242, for 12 s)")
- **AND** its click offers Stop Waiting (`cancel_operation`), which ends the wait with nothing moved ("Canceled moving 1 commit to docs before it started")
  [@test] tests/e2e/repository/repository-lock.spec.ts (`should show the wait of a move started from a menu on the row's ⋯, whose menu stops it, nothing moved`)
  [@test] crates/test-server/src/invoke_test.rs (`stop_waiting_ends_the_wait_of_a_rewrite_with_nothing_changed`)
  [@test] src/presenters/lock-wait.test.ts (`offers Stop Waiting of an operation of the session only while it waits for a holder, and stops it by its id`, `says that an action stopped while it waited for the repository lock changed nothing`)

#### Scenario: The surface that started it says the same
- **WHEN** a rewrite of the engine waits
- **THEN** the surface that started it says whom it waits for, and its Stop Waiting stops the wait by its operation
  [@test] src/lib/rewrite-run.test.ts (`shows whom a rewrite of the engine waits for and stops the wait by its operation`)
  [@test] src/lib/rewrite-run.test.ts (`says behind which rewrite of this window it waits in the queue of the session`)

### Requirement: Move operation: Once the holder released the lock the move runs on what it left

Once the holder released the lock, the move MUST run on what the holder left. A holder that moved HEAD meanwhile makes
its core refuse the HEAD it expected, and the move MUST be resolved again on HEAD as it is now and run once more (at
most three runs); a commit the holder took away is `plan_changed`, nothing written.

#### Scenario: The holder moved HEAD
- **GIVEN** a holder that moved HEAD meanwhile (`brd update`, `brd pick`, a `brd` rewrite, a commit)
- **WHEN** it released the lock
- **THEN** the core refuses the HEAD the move expected, and the move is resolved again on HEAD as it is now and runs once more, at most three runs
- **AND** a branch is resolved by its name, a commit by its author, author time and subject without the prefix once the sync shows that HEAD
  [@test] crates/operations/src/rewrite_test.rs (`a_rewrite_waiting_for_the_lock_of_another_process_runs_on_the_head_it_left`)

#### Scenario: The holder took the commit away
- **WHEN** the holder took away a commit the move names
- **THEN** the move is `plan_changed`, nothing written
  [@test] crates/operations/src/rewrite_test.rs (`a_rewrite_waiting_for_the_lock_of_another_process_runs_on_the_head_it_left`)

### Requirement: Move operation: A Stop Waiting as the holder lets the lock go stops the move

A Stop Waiting that came as the holder moved HEAD and released the lock MUST stop the move too, `canceled` also when
the holder took the commit away: no resolve and no run on the HEAD the holder left, nothing moved.

#### Scenario: Stop Waiting as the holder moves HEAD
- **WHEN** the user stops the wait as the holder moves HEAD and releases the lock
- **THEN** the move is stopped, with no resolve and no run on the HEAD the holder left, nothing moved
  [@test] crates/operations/src/rewrite_test.rs (`a_stop_waiting_as_the_holder_moves_head_and_lets_the_lock_go_writes_nothing`)

#### Scenario: Stop Waiting as the holder takes the commit away
- **WHEN** the user stops the wait as the holder takes the named commit away
- **THEN** the move is `canceled`, without a resolve, nothing moved
  [@test] crates/operations/src/rewrite_test.rs (`a_stop_waiting_as_the_holder_squashes_the_named_commit_away_is_canceled_without_a_resolve`)

### Requirement: Move operation: A commit already in the target is not rewritten

A commit already in the target branch MUST NOT be rewritten; the result lists it as `Unchanged`.

#### Scenario: A commit already in the target
- **WHEN** a move takes a commit already in the target branch
- **THEN** it is not rewritten, and the result lists it as `Unchanged`
  [@test] crates/sync-core/src/move_commits_test.rs (`skips_commits_already_in_the_target_branch`)

### Requirement: Move operation: All commits are rewritten in one history rewrite

All commits MUST be rewritten in one history rewrite that keeps the trees, the metadata and the working tree: a move of
ten commits runs the git processes of a move of one.

#### Scenario: The trees, the metadata and the checked-out branch stay
- **WHEN** a move rewrites commits
- **THEN** their trees and metadata stay, and the working tree with its checked-out branch is kept
  [@test] crates/sync-core/src/move_commits_test.rs (`keeps_the_body_and_the_metadata`, `keeps_the_checked_out_branch`)

#### Scenario: Many commits move in one rewrite
- **WHEN** many commits move
- **THEN** one history rewrite rewrites them all
  [@test] crates/sync-core/src/move_commits_test.rs (`moves_many_commits_in_one_rewrite`)

#### Scenario: A move of ten commits
- **WHEN** ten commits move
- **THEN** the move runs the git processes of a move of one
  [@test] crates/sync-core/src/move_commits_test.rs (`moves_many_commits_in_one_rewrite`)

### Requirement: Move operation: The rewrite keeps the author and the committer

The rewrite MUST keep the author and the committer of every recreated commit: name, email, date and timezone.

#### Scenario: A recreated commit
- **WHEN** a rewrite recreates a commit
- **THEN** its author and committer keep their name, email, date and timezone
  [@test] crates/sync-core/src/reword_test.rs (`reword_preserves_committer`)

### Requirement: Move operation: The target name is one the grouper keeps as is

The target name MUST be a name the grouper keeps as is: letters, numbers, `-`, `_`, `.` and `/`, not at the ends.

#### Scenario: An invalid target name
- **WHEN** a move names a target with other characters, or `-`, `_`, `.` or `/` at the ends
- **THEN** the name is refused
  [@test] crates/sync-core/src/move_commits_test.rs (`validates_the_branch_name`, `refuses_an_invalid_target_or_an_unknown_commit`)

### Requirement: Move operation: The form and the rename ask Rust what a typed name is

The "Move to…" form and the rename SHALL ask Rust what a typed name is (`validate_branch_name`, `check_branch_name`)
once the typing pauses, keep the answer of each name, and word it (`src/presenters/branch-name.ts`); Enter right after
typing waits for the answer of the name as typed.

#### Scenario: The answers of a typed name
- **WHEN** the typing of a name pauses
- **THEN** Rust answers what it is: empty, valid, too short, invalid characters, a `/` where only a branch name goes, a name the grouper would rename
- **AND** the field words the answer
  [@test] crates/domain-types/src/branch_names_test.rs (`a_typed_name_is_what_the_grouping_keeps_as_it_is`)
  [@test] tests/e2e/inline-branch-creator/inline-branch-creator.spec.ts (`should validate branch name format`)

#### Scenario: The words are checked against the oracle Rust writes
- **WHEN** a Rust test writes the oracle `src/test-support/branch-names.cases.json` (`UPDATE_BRANCH_NAME_CASES=1`)
- **THEN** the words of the presenter are checked against it
  [@test] crates/domain-policy/src/branch_name_cases_test.rs (`the_branch_name_cases_are_what_the_check_answers`)
  [@test] src/presenters/branch-name.test.ts (`words every answer of the oracle Rust writes as the field says it`, `marks only a valid name valid, and every name with words invalid`)

### Requirement: Move operation: A split prefix branch keeps the issue key

A split prefix branch MUST keep the issue key of its commits.

#### Scenario: Commits split out of an issue branch
- **WHEN** commits of an issue branch move to a new prefix branch
- **THEN** the issue key stays in their messages, and the prefix branch keeps it
  [@test] crates/sync-core/src/move_commits_test.rs (`splits_commits_out_of_an_issue_branch`)

### Requirement: Selection and the "Move to…" form: The commits of a list are selectable

The commits of a branch table row and of the unassigned commits card MUST be selectable: a click selects the row
alone, Cmd/Ctrl-click toggles it, Shift-click selects the rows from the anchor (the row of the last plain or Cmd/Ctrl
click, which a Shift-click leaves in place), Cmd+A selects all, Escape clears.

#### Scenario: Click, Cmd/Ctrl-click and Shift-click
- **WHEN** the user clicks a commit row, Cmd/Ctrl-clicks it or Shift-clicks it
- **THEN** a click selects the row alone, Cmd/Ctrl-click toggles it, and Shift-click selects the rows from the anchor
- **AND** the anchor is the row of the last plain or Cmd/Ctrl click, which a Shift-click leaves in place
  [@test] src/lib/row-selection.test.ts (`ranges every Shift+click from the row of the last plain click, and leads with the row clicked last`, `adds a range to the selection with Cmd/Ctrl+Shift and moves the anchor with a Cmd/Ctrl click`)

#### Scenario: Cmd+A and Escape
- **WHEN** the user presses Cmd+A in a commit list, or Escape
- **THEN** Cmd+A selects all, and Escape clears the selection

### Requirement: Selection and the "Move to…" form: The row clicked last leads the selection

The row clicked last MUST lead the selection; when a Cmd/Ctrl-click took it out, the selected row nearest to it leads,
and a selection the app made (the commits just moved) is led by its first row.

#### Scenario: The row clicked last
- **WHEN** the user clicks rows of a commit list
- **THEN** the row clicked last leads the selection
  [@test] src/lib/row-selection.test.ts (`ranges every Shift+click from the row of the last plain click, and leads with the row clicked last`)

#### Scenario: A Cmd/Ctrl-click took the lead out
- **WHEN** a Cmd/Ctrl-click takes the leading row out of the selection
- **THEN** the selected row nearest to it leads
  [@test] src/lib/row-selection.test.ts (`is the selected row nearest to the lead once a Cmd/Ctrl click took the lead out, the earlier of two as near`)

#### Scenario: A selection the app made
- **WHEN** the app makes a selection (the commits just moved)
- **THEN** its first row leads it
  [@test] src/lib/row-selection.test.ts (`is the first selected row of a selection made by its ids, and nothing without a selection`)

### Requirement: Selection and the "Move to…" form: The branch rows are selectable

The branch rows MUST be selectable: Cmd/Ctrl-click toggles a row, Shift-click selects the rows from the last clicked
one, Cmd+A selects all, Escape clears; a plain click expands the row and clears the selection.

#### Scenario: Selecting branch rows
- **WHEN** the user Cmd/Ctrl-clicks branch rows, or Shift-clicks one
- **THEN** Cmd/Ctrl-click toggles a row, and Shift-click selects the rows from the last clicked one, in table order
  [@test] src/lib/branch-selection.test.ts (`selects a range of rows from the anchor, in table order`)
  [@test] tests/e2e/move-commits/combine-branches.spec.ts (`should move the selected branches to a new branch with Move to…`)

#### Scenario: Cmd+A, Escape and a plain click
- **WHEN** the user presses Cmd+A or Escape in the branch table, or clicks a row plainly
- **THEN** Cmd+A selects all, Escape clears, and a plain click expands the row and clears the selection

### Requirement: Selection and the "Move to…" form: The selection of branches under the filter

Under the filter of the table, Shift-click MUST take the shown rows, Cmd+A the rows the filter matched (not the dimmed
ones), and a selected branch the filter hides MUST stay selected: the bar adds "· 1 hidden"
([Branch Filter](../branch-filter/spec.md)).

#### Scenario: Cmd+A under the filter
- **GIVEN** a filter of the table and a selected branch the filter hides
- **WHEN** the user presses Cmd+A, or Shift-clicks a row
- **THEN** Cmd+A selects the rows the filter matched, not the dimmed ones, and Shift-click takes the shown rows
- **AND** the hidden branch stays selected, and the bar adds "· 1 hidden"
  [@test] tests/e2e/branch-filter/branch-filter.spec.ts (`should select only the matches with Cmd+A and keep a hidden selected branch`)

### Requirement: Selection and the "Move to…" form: The rows a collapsed stack hides count as hidden

The rows a collapsed stack hides MUST count as hidden too: Shift-click and Cmd+A skip them, a selected one stays
selected.

#### Scenario: Cmd+A with a collapsed stack
- **GIVEN** a collapsed stack
- **WHEN** the user presses Cmd+A, or Shift-clicks across it
- **THEN** its branches are skipped, and a selected one of them stays selected and counts as hidden
  [@test] tests/e2e/branch-filter/branch-filter.spec.ts (`should not select the branches of a collapsed stack with Cmd+A, and count them as hidden`)

### Requirement: Selection and the "Move to…" form: Only one list holds a selection

Only one list MUST hold a selection, the branch table included: selecting in another list MUST clear it.

#### Scenario: Selecting in another list
- **GIVEN** a selection in one list
- **WHEN** the user selects in another list
- **THEN** the selection of the first list clears
  [@test] tests/e2e/commit-message/commit-message.spec.ts (`should select commit rows of a branch, one list at a time`)

#### Scenario: A commit list takes the selection from the branch table
- **GIVEN** selected branches and their form
- **WHEN** a commit list takes the selection
- **THEN** the selected branches and their form clear
  [@test] src/stores/commit-move.test.ts (`clears the selected branches and their form when a commit list takes the selection`)

### Requirement: Selection and the "Move to…" form: The actions of a commit selection are on its lead row

A selection of commits MUST show "N selected" and a "Move to…" button at the end of the meta line of its lead row, in a
branch and in the unassigned card alike. The selection's actions never get a line of their own: they take the place
that holds nothing else, the meta line of a commit.

#### Scenario: Commits selected
- **WHEN** commits are selected, in a branch or in the unassigned card
- **THEN** the meta line of the lead row ends with "N selected" and a "Move to…" button, on no line of their own

### Requirement: Selection and the "Move to…" form: The actions of a branch selection are over the column heads

Selected branches MUST show "N branches selected", "Move to…" and a clear button over the column heads of the branch
table. The selection's actions never get a line of their own: they take the place that holds nothing else, the column
heads of the branches.

#### Scenario: Branches selected
- **WHEN** branches are selected
- **THEN** "N branches selected", "Move to…" and a clear button stand over the column heads of the branch table, on no line of their own

### Requirement: Selection and the "Move to…" form: Selecting a commit moves no row

Selecting or deselecting a commit MUST NOT move a row: no line above the commits holds the actions or a hint, and the
meta line of the lead row stays one line with them. Their press keeps the selection. The "Move to…" form opens as a row
right below the lead row, which says "N selected" alone while it is open.

#### Scenario: Commits selected and deselected
- **WHEN** commits are selected and deselected
- **THEN** no row moves, and the meta line of the lead row stays one line with the actions
- **AND** a press of an action keeps the selection
  [@test] tests/e2e/move-commits/move-commits.spec.ts (`should keep the rows in place as commits are selected and deselected`)

#### Scenario: The form opens below the lead row
- **WHEN** the "Move to…" form opens for selected commits
- **THEN** it is a row right below the lead row, which says "N selected" alone while it is open

### Requirement: Selection and the "Move to…" form: The keys of the actions and of the form are theirs

The keys of the selection's actions and of the form MUST be theirs: Enter presses the focused button, a letter typed
into the name is a letter of the name. Escape on a button of the actions MUST clear the selection and give the focus
back to the list.

#### Scenario: Enter on Squash, a letter in the name, Escape on a button
- **WHEN** the user presses Enter on a focused button of the actions, types a letter into the name, or presses Escape on a button of the actions
- **THEN** Enter presses that button (Squash opens the squash, not the form of the list's Enter), the letter is a letter of the name
- **AND** Escape clears the selection and gives the focus back to the list
  [@test] tests/e2e/move-commits/move-commits.spec.ts (`should leave the keys of the selection's actions and of its form to them`)

### Requirement: Selection and the "Move to…" form: Selecting a branch moves no row

Selecting or deselecting a branch MUST NOT move a row: the branch selection bar takes the place of the column heads of
the branch table, as high as they are, and says "N branches selected" alone while the "Move to…" form is open above the
table.

#### Scenario: Branches selected and deselected
- **WHEN** branches are selected and deselected
- **THEN** no row moves: the branch selection bar takes the place of the column heads, as high as they are
- **AND** while the "Move to…" form is open above the table, the bar says "N branches selected" alone
  [@test] tests/e2e/move-commits/combine-branches.spec.ts (`should keep the rows in place as branches are selected and deselected`)

#### Scenario: A table that shows no rows
- **GIVEN** a table that shows no rows (the branches in islands or parked, or a filter that matched none)
- **WHEN** branches are selected
- **THEN** the table has no heads, and the bar is a line above the islands

### Requirement: Selection and the "Move to…" form: The selection bar offers Stack Together

With [stacked branches](../stacked-branches/spec.md) on, the selection bar MUST offer "Stack Together" for two or more
selected branches; the oldest commit goes at the bottom and the action sends one rewrite intent.

#### Scenario: Two selected branches stacked together
- **GIVEN** stacked branches on and two or more selected branches
- **WHEN** the user chooses "Stack Together" in the selection bar
- **THEN** the branches are stacked, the oldest commit at the bottom, by one rewrite intent
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should stack the selected branches together from the selection bar, the oldest at the bottom, and undo it`)

### Requirement: Selection and the "Move to…" form: A selection with a way back offers it

A selection with a way back MUST also offer it: "Move Back to ABC-123" (the issue group of the key when it has one, see
[Issue Groups](../issue-groups/spec.md); the issue branch of the key for a commit an issue group holds), "Move to
Unassigned", "Remove Prefix", or "Move Back" when they go back to several branches.

#### Scenario: Commits an issue group put into a branch
- **GIVEN** selected commits that the issue groups put into a branch
- **WHEN** the selection offers its way back
- **THEN** it is "Move Back to ABC-123", which moves them to the issue branch of their key
  [@test] src/hooks/useCommitMoveList.test.ts (`moves the commits that the issue groups put into a branch back to the issue branch of their key`)

#### Scenario: The other ways back
- **WHEN** the selected commits go back to the unassigned commits, lose only a prefix, or go back to several branches
- **THEN** the selection offers "Move to Unassigned", "Remove Prefix", or "Move Back"

### Requirement: Selection and the "Move to…" form: "Move to…", Enter or M opens the form

"Move to…", Enter or M MUST open the form, for the selected commits or for all commits of the selected branches. One
name input decides: an existing branch name moves the commits there, a new name creates the branch; the help text and
the submit button ("Move" or "Create") MUST say which.

#### Scenario: An existing name and a new one
- **WHEN** the user opens the form with "Move to…", Enter or M and types a name
- **THEN** an existing branch name moves the commits there and the submit button says "Move"
- **AND** a new name creates the branch and the submit button says "Create", the help text saying which

### Requirement: Selection and the "Move to…" form: The help text says what the move does

The help text MUST say what the move does, from its dry run: the issues that go there by their key, without a rewrite,
and the commits that get the prefix. A refused plan MUST show its reason and disable the submit; commits all in the
branch already say so.

#### Scenario: The help text of a move
- **WHEN** the dry run of the typed move answers
- **THEN** the help text names the issues that go there by their key, without a rewrite, and the commits that get the prefix ("rewrites them to start with (lexer)" when all do)

#### Scenario: A refused plan and commits already in the branch
- **WHEN** the plan is refused, or all commits are in the branch already
- **THEN** a refused plan shows its reason and disables the submit, and commits all in the branch already say so

### Requirement: Selection and the "Move to…" form: The form warns when it empties a pushed branch

The form MUST warn when it empties a pushed branch: its remote branch and pull request stay. A child whose parent the
move empties is a muted note: it stays stacked on the parent and is built on the baseline until the parent has commits
again.

#### Scenario: A move empties a pushed branch
- **WHEN** the move empties a pushed branch
- **THEN** the form warns that its remote branch and pull request stay

#### Scenario: A move empties the parent of a child
- **WHEN** the move empties the parent of a child
- **THEN** a muted note says the child stays stacked on the parent and is built on the baseline until the parent has commits again

### Requirement: Selection and the "Move to…" form: The form offers the existing branches as chips

The form MUST offer the existing branches as chips, the branches of the same issue first, then the AI suggestions
([Branch Name Suggestions](../branch-name-suggestions/spec.md)).

#### Scenario: The chips of the form
- **WHEN** the form opens for commits of an issue branch
- **THEN** it offers the existing branches as chips, those of the same issue first, then the AI suggestions
  [@test] tests/e2e/move-commits/move-commits.spec.ts (`should split commits out of an issue branch into a new branch nested under the issue`)

### Requirement: Selection and the "Move to…" form: The input may name a parent

With [stacked branches](../stacked-branches/spec.md) on, the input SHALL allow naming a parent (`auth/login`): the
target is looked up by its leaf, the help text names the parent and says that a parent other than the existing
branch's restacks it, and the branches of the same stack come first. A bare name takes the branch's stack parent.

#### Scenario: A move to a branch on a parent
- **GIVEN** stacked branches on
- **WHEN** the user types `auth/login`
- **THEN** the target is looked up by its leaf, and the help text names the parent ("Moves 2 commits to login (on auth)", "Creates login on auth")
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should move commits to a new branch on a parent with Move to…`)

#### Scenario: A parent other than the existing branch's
- **WHEN** the typed parent is other than the existing branch's
- **THEN** the help text says that it restacks it ("Moves 2 commits to login and stacks it on auth instead of docs")

#### Scenario: A bare name
- **WHEN** the user types a bare name of a stacked branch
- **THEN** it takes the branch's stack parent

#### Scenario: The branches of the same stack first
- **WHEN** the form offers its move targets
- **THEN** the branches of the same stack come first
  [@test] src/stores/branch-table-rows.test.ts (`offers the branches of the same stack first as move targets`)

### Requirement: Context menus: The commit menu offers Move to…, Move to Existing and the way back

The commit menu MUST offer "Move to…" with the key `M` (the label of the selection button, for one commit or many), a
"Move to Existing" submenu with the branches and "New Branch…", and the way back of the commits.

#### Scenario: A commit moves to an existing branch from its menu
- **WHEN** the user opens the menu of a commit
- **THEN** it offers "Move to…" with the key `M`, and a "Move to Existing" submenu with the branches and "New Branch…"
- **AND** a branch of the submenu moves the commit there
  [@test] tests/e2e/move-commits/move-commits.spec.ts (`should move a commit to an existing branch from the context menu and undo it`)

#### Scenario: The way back of the commits
- **WHEN** the user opens the menu of a split commit or of a prefix commit
- **THEN** it offers their way back, which moves them back to their issue or to the unassigned commits
  [@test] tests/e2e/move-commits/move-commits.spec.ts (`should move a split commit back to its issue and a prefix commit to the unassigned commits`)

#### Scenario: One commit or many
- **WHEN** the user opens the menu of a selected row, or of a row that is not selected
- **THEN** the menu acts on all selected commits, or on that commit only
  [@test] tests/e2e/unassigned-commits/unassigned-commits.spec.ts (`should act on all selected commits from the context menu of a selected row`, `should act on that commit only from the context menu of a row that is not selected`)

### Requirement: Context menus: The branch menu offers Move to… for its commits

The branch menu MUST offer "Move to…" with the key `M`, for all commits of the branch, or of every selected branch when
the row is selected; it has no "Move to Existing ›": the form offers the existing branches.

#### Scenario: Move to… of selected branches
- **GIVEN** selected branch rows
- **WHEN** the user chooses "Move to…" in the menu of a selected row
- **THEN** the form opens for all commits of every selected branch, and offers the existing branches; the menu has no "Move to Existing ›"
  [@test] tests/e2e/move-commits/combine-branches.spec.ts (`should move the selected branches to a new branch with Move to…`)

### Requirement: Context menus: `M` works where the item shows it

`M` MUST work where the item shows it: on the focused row without a selection it selects the row and opens "Move to…"
for it, as the item does; with selected rows it opens it for them.

#### Scenario: `M` on the focused row without a selection
- **GIVEN** a focused branch row and no selection
- **WHEN** the user presses `M`
- **THEN** the row is selected and "Move to…" opens for it, as the item does
  [@test] tests/e2e/move-commits/combine-branches.spec.ts (`should open Move to… for the focused row with M, without a selection`)

#### Scenario: `M` with selected rows
- **GIVEN** selected branch rows
- **WHEN** the user presses `M`
- **THEN** "Move to…" opens for them

### Requirement: Context menus: The branch menu offers Rename… and Name This Branch…

The branch menu MUST offer "Rename…" for a prefix branch and "Name This Branch…" for an issue branch, through the
rewrite `rename` of the engine; an existing name merges the branches.

#### Scenario: A prefix branch renamed and an issue branch named
- **WHEN** the user renames a prefix branch with "Rename…" or names an issue branch with "Name This Branch…"
- **THEN** the branch takes the name, through the rewrite `rename` of the engine
- **AND** an existing name merges the branches
  [@test] crates/sync-core/src/regroup_test.rs (`renames_prefix_and_issue_branches`)
  [@test] tests/e2e/move-commits/move-commits.spec.ts (`should rename a branch and name an issue branch`)

### Requirement: Context menus: Naming an issue group changes only the issue groups

"Name This Branch…" and "Rename…" of an issue group MUST change the issue groups only, without a rewrite, also when the
issue branch is named after a stacked one.

#### Scenario: An issue branch named after a stacked branch
- **WHEN** an issue branch is named after a stacked branch
- **THEN** it joins it by the issue groups, without a rewrite
  [@test] crates/sync-core/src/regroup_test.rs (`an_issue_branch_renamed_into_a_stacked_branch_joins_it_by_the_issue_groups`)

#### Scenario: An issue branch named
- **WHEN** the user names an issue branch with "Name This Branch…"
- **THEN** the issue groups change, without a rewrite
  [@test] tests/e2e/move-commits/move-commits.spec.ts (`should rename a branch and name an issue branch`)

### Requirement: Context menus: The branch menu offers Ungroup

The branch menu of a prefix branch and of an issue group MUST offer "Ungroup": it moves all its commits back, the
prefixes go and the issues leave the issue group.

#### Scenario: A prefix branch ungrouped
- **WHEN** the user chooses "Ungroup" in the menu of a prefix branch
- **THEN** all its commits move back and their prefixes go
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should build a child on the baseline once its parent has no commits, and unstack it`)

#### Scenario: An issue group ungrouped
- **WHEN** the user chooses "Ungroup" in the menu of an issue group
- **THEN** its issues leave the issue group

### Requirement: Context menus: A rename or an ungroup shows its progress row

While a rename or an ungroup runs, its form MUST show the progress row ("Renaming docs to documentation…"); it closes
when the commits moved. A failure MUST keep the form with the typed name (the error is a toast).

#### Scenario: A rename runs
- **WHEN** a rename runs
- **THEN** its form shows the progress row ("Renaming docs to documentation…"), and closes when the commits moved

#### Scenario: A rename fails
- **WHEN** a rename or an ungroup fails
- **THEN** the form stays with the typed name, and the error is a toast

### Requirement: Context menus: Rename renames the branch in its children and the stack parents

Rename MUST rename the branch in the prefixes of the branches [stacked](../stacked-branches/spec.md) on it too, and in
the stack parents (`rename_branch_core`), and the new name MUST be a branch name without `/`.

#### Scenario: A parent renamed
- **GIVEN** a parent that other branches are stacked on
- **WHEN** the user renames it
- **THEN** its children follow, in their prefixes and in the stack parents
- **AND** a new name with `/` is refused
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should rename a parent with its children following`)
  [@test] crates/sync-core/src/stack_operations_test.rs (`renaming_a_parent_renames_it_in_its_children`)

### Requirement: Context menus: Ungroup and Unapply of a parent confirm and name its children

Ungroup and Unapply of a branch that others stack on MUST confirm and name them: they stay stacked on it and are built
on the baseline until it has commits again. The confirmation focuses its button, so Enter confirms.

#### Scenario: A parent ungrouped
- **GIVEN** a branch that others stack on
- **WHEN** the user chooses Ungroup or Unapply
- **THEN** the confirmation names them: they stay stacked on it and are built on the baseline until it has commits again
- **AND** it focuses its button, so Enter confirms
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should build a child on the baseline once its parent has no commits, and unstack it`)

### Requirement: Drag and drop: A commit row is draggable

A commit row MUST be draggable. Dragging a selected row MUST drag the selection; dragging another row MUST drag it
alone and leave the selection and its form as they are.

#### Scenario: A selected row dragged
- **GIVEN** selected commit rows
- **WHEN** the user drags one of them
- **THEN** the drag takes the selection

#### Scenario: A row that is not selected dragged
- **GIVEN** selected commit rows and their form
- **WHEN** the user drags a row that is not selected
- **THEN** the drag takes it alone, and the selection and its form stay as they are
  [@test] tests/e2e/move-commits/combine-branches.spec.ts (`should drop a commit on the expanded commits of a branch, and drag an unselected row alone, the selection as it was`)

### Requirement: Drag and drop: A branch row is draggable

A branch row MUST be draggable, also while a sync runs: it drags the selected branches when it is one of them, else
itself alone, with all their commits; a row of the Parked card too. The dragged rows fade (`data-dragging`).

#### Scenario: A branch row dragged onto a branch
- **WHEN** the user drags a branch row onto another branch
- **THEN** the drag takes it with all its commits, and the dragged rows fade (`data-dragging`)
  [@test] tests/e2e/move-commits/combine-branches.spec.ts (`should drag an issue branch onto a branch: the issue joins it without a rewrite, and Undo moves it back`)

#### Scenario: A row of the Parked card dragged
- **WHEN** the user drags a row of the Parked card, selected or not
- **THEN** the drag takes the selected branches when it is one of them, else that row alone
  [@test] tests/e2e/parked-branches/parked-branches.spec.ts (`should park a branch dragged onto the park zone of the drag bar, unpark it dragged onto the unpark zone, take both back with Undo, and drag the commits of a parked row onto a branch`)
  [@test] tests/e2e/parked-branches/parked-branches.spec.ts (`should park an unselected branch dropped on the collapsed Parked card and keep the selection, and park and unpark the selection with its buttons, of both kinds by the part each takes`)

#### Scenario: A branch row dragged while a sync runs
- **WHEN** a sync runs
- **THEN** a branch row is still draggable
  [@test] tests/e2e/move-commits/combine-branches.spec.ts (`should take the drop of a branch while a sync runs: its move goes to the session at once`)

### Requirement: Drag and drop: A drag starts without a change of the layout

A drag MUST start without a change of the layout: nothing shows or goes above the dragged row, so it stays under the
pointer.

#### Scenario: A drag starts
- **WHEN** the user starts to drag a row
- **THEN** nothing shows or goes above it, and it stays under the pointer
  [@test] tests/e2e/move-commits/combine-branches.spec.ts (`should drop a commit on the expanded commits of a branch, and drag an unselected row alone, the selection as it was`)
  [@test] tests/e2e/parked-branches/parked-branches.spec.ts (`should keep the rows in place when a drag starts, show the drag bar in view at the bottom of a long list, and take no drop on the bar from a drag that started under it`)

### Requirement: Drag and drop: Another branch accepts the drop

Another branch MUST accept the drop on its row and on its rows below (the inline form, the expanded commits) and move
the commits there. The source branch MUST NOT accept a drag that comes from it alone.

#### Scenario: A drop on another branch
- **WHEN** the user drops commits on the row of another branch, or on its rows below
- **THEN** the commits move there
  [@test] tests/e2e/move-commits/move-commits.spec.ts (`should drag the selection onto a branch, but not onto its own branch`)
  [@test] tests/e2e/move-commits/combine-branches.spec.ts (`should drop a commit on the expanded commits of a branch, and drag an unselected row alone, the selection as it was`)

#### Scenario: A drag from the source branch alone
- **WHEN** a drag comes from one branch alone
- **THEN** that branch does not accept it
  [@test] tests/e2e/move-commits/move-commits.spec.ts (`should drag the selection onto a branch, but not onto its own branch`)
  [@test] src/stores/commit-move.test.ts (`drops anywhere but on the one list a drag comes from`)

### Requirement: Drag and drop: The unassigned commits card takes what has a way back

The unassigned commits card MUST accept the commits and branches that have a way back, and move them back.

#### Scenario: A drop on the unassigned commits card
- **WHEN** the user drops commits or branches that have a way back on the unassigned commits card
- **THEN** they move back

### Requirement: Drag and drop: A target refuses as the menu of the same action does

A target MUST refuse as the menu of the same action does, and say why while the pointer is over it.

#### Scenario: Whole branches one of which failed to sync
- **WHEN** a drag of whole branches, one of which failed to sync, is over a target
- **THEN** the target refuses it as Move to… of a branch does, and says why

### Requirement: Drag and drop: A target takes a drop while a sync runs

A target MUST take a drop while a sync runs, as a sync disables no rewrite
([Running Operations](../../../../docs/design/operations.md#conflicts)): its move goes to the session at once, named on
the table the drag started on.

#### Scenario: A drop of a branch during a sync
- **GIVEN** a sync that runs
- **WHEN** the user drops a branch on a target
- **THEN** its move goes to the session at once, named on the table the drag started on
  [@test] tests/e2e/move-commits/combine-branches.spec.ts (`should take the drop of a branch while a sync runs: its move goes to the session at once`)

### Requirement: Drag and drop: A drop during a rewrite waits for it in the queue

A target MUST take a drop while a rewrite of the repository runs: its move waits for that rewrite in the queue of the
session ([Running Operations](../../../../docs/design/operations.md)), its row saying behind which, and then moves the
commits as the rewrite left them, by the ids it gave them.

#### Scenario: A second drag while the first move runs
- **GIVEN** a move that runs
- **WHEN** the user drops a second drag
- **THEN** its move waits for the first in the queue of the session, its row saying behind which ("Waits for Moving 1 commit to docs…")
- **AND** then it moves the commit as the first move left it, by the id it gave it
  [@test] tests/e2e/move-commits/move-commits.spec.ts (`should take a second drag while the first move runs, wait for it in the queue and move the commit as the first move left it`)
  [@test] crates/operations/src/rewrite_test.rs (`a_move_queued_behind_a_move_names_its_commit_through_the_new_id_the_first_answered`)
  [@test] crates/operations/src/registry_test.rs (`the_caller_of_a_waiting_operation_hears_behind_which_it_waits_and_where_it_starts`)
  [@test] src/lib/branch-drop.test.ts (`moves dragged commits onto a branch, leaves a drag of branches that land to the Parked card from its rows, and waits as Move to… does`)

### Requirement: Drag and drop: Two drags into different branches both land

A queued move MUST run beside the sync of the one before, whose syncs fold into two runs at most, and two drags into
different branches MUST both land where they were dropped.

#### Scenario: Two drags into different branches
- **WHEN** the user drops two drags into different branches, the first waiting for the repository lock
- **THEN** the second runs beside the sync of the first, their syncs fold into two runs at most
- **AND** both land where they were dropped
  [@test] tests/e2e/move-commits/move-commits.spec.ts (`should move two drags into different branches, the first waiting for the repository lock, both in place`)

### Requirement: Drag and drop: A drop asks the target once more

The answer of a target MUST follow the sync and the rewrites while the drag goes on, and a drop MUST ask it once more
and take the commits as the last sync left them (a commit a rewrite gave another id is found by its author, author
time and subject): gone ones refuse the drop with an info toast.

#### Scenario: A rewrite gave a dragged commit another id
- **WHEN** a rewrite gave a dragged commit another id before the drop
- **THEN** the drop finds it by its author, author time and subject, as the last sync left it
  [@test] crates/operations/src/rewrite_test.rs (`a_move_queued_behind_a_move_of_another_commit_finds_its_commit_by_its_key_after_the_sync`)

#### Scenario: A dragged commit is gone
- **WHEN** a dragged commit is gone at the drop
- **THEN** the drop is refused with an info toast

### Requirement: Drag and drop: The drag bar shows over the bottom of the window

While commits or branches are dragged, the drag bar MUST show over the bottom of the window, in view wherever the page
is scrolled and out of the flow of the content, which gets room at its end so that every row scrolls out from under
it; it leaves the column of the toasts free.

#### Scenario: A drag in a long list
- **WHEN** the user drags a row of a long list
- **THEN** the drag bar shows in view at the bottom of the window, out of the flow of the content, wherever the page is scrolled
- **AND** the content gets room at its end so that every row scrolls out from under it, and the column of the toasts stays free
  [@test] tests/e2e/parked-branches/parked-branches.spec.ts (`should keep the rows in place when a drag starts, show the drag bar in view at the bottom of a long list, and take no drop on the bar from a drag that started under it`)

### Requirement: Drag and drop: The drag bar holds the targets that change the shape

The drag bar MUST hold the targets that change the shape of what is dragged, each while the drag is for it, and above
them why the target under the pointer refuses the drag.

#### Scenario: The targets of the drag bar
- **WHEN** a drag is for a target that changes its shape
- **THEN** the drag bar holds that target
- **AND** above the targets, it says why the target under the pointer refuses the drag

### Requirement: Drag and drop: A drag that starts under the bar takes no drop on it

A drag that starts under the bar MUST take no drop on it until the pointer left it (`data-armed`).

#### Scenario: A drag started under the bar
- **WHEN** a drag starts under the drag bar
- **THEN** the bar takes no drop until the pointer left it (`data-armed`)
  [@test] tests/e2e/parked-branches/parked-branches.spec.ts (`should keep the rows in place when a drag starts, show the drag bar in view at the bottom of a long list, and take no drop on the bar from a drag that started under it`)

### Requirement: Drag and drop: A drag ends with the dragend of its row

The drag MUST end with the `dragend` of its row also when a sync remounted the row, else with the next mouse move,
press or key.

#### Scenario: A sync remounted the dragged row
- **WHEN** a sync remounted the dragged row and the drag ends
- **THEN** the drag ends with the `dragend` of its row, else with the next mouse move, press or key

### Requirement: Drag and drop: Move to New Branch opens the Move to… form

Move to New Branch in the drag bar SHALL show also when there is no branch yet. A drop there MUST open the "Move to…"
form for the dragged commits above the branch table, in view and focused; the form keeps their ids and closes when a
rewrite gave one of them another id. It waits for nothing: the form's move does.

#### Scenario: An unassigned commit dropped on the new branch zone
- **WHEN** the user drops an unassigned commit on Move to New Branch
- **THEN** the "Move to…" form opens for it above the branch table, in view and focused
  [@test] tests/e2e/move-commits/move-commits.spec.ts (`should drag an unassigned commit onto the new branch zone`)

#### Scenario: No branch yet
- **GIVEN** no branch yet
- **WHEN** the user drags a commit
- **THEN** the drag bar shows Move to New Branch
  [@test] tests/e2e/move-commits/combine-branches.spec.ts (`should offer the new branch zone when there is no branch yet`)

#### Scenario: A rewrite gave a commit of the form another id
- **GIVEN** the form open for dragged commits, which keeps their ids
- **WHEN** a rewrite gave one of them another id
- **THEN** the form closes

### Requirement: Drag and drop: A drag of whole branches changes their shape where the target says so

A drag of whole branches MUST also change their shape, where the target says so, at once, with a toast and Undo
(`branch-drop.ts`); the label of a zone names what its drop takes ("Park docs and api"); a drag of commit rows passes
these targets by.

#### Scenario: The label of a zone
- **WHEN** whole branches are dragged over a zone that changes their shape
- **THEN** its label names what its drop takes ("Park docs and api"), and the drop changes their shape at once, with a toast and Undo

#### Scenario: A drag of commit rows
- **WHEN** commit rows are dragged
- **THEN** they pass these targets by

### Requirement: Drag and drop: The park zone and the Parked card park the dragged branches

The park zone of the drag bar and the whole Parked card, also collapsed (its header, its rows), MUST park the dragged
branches that are not parked, without a note, in one write ([Parked Branches](../parked-branches/spec.md)); those
parked already stay as they are. A row of the Parked card leaves such a drag to the card; it takes a drag of commit
rows and of parked branches as any row.

#### Scenario: Branches dropped on the park zone
- **WHEN** the user drops branches on the park zone or the Parked card
- **THEN** those that are not parked are parked, without a note, in one write, and those parked already stay as they are
  [@test] src/lib/branch-drop.test.ts (`parks the dragged branches that land and leaves the parked ones, refuses one a robot lands, and never waits for a sync`)

#### Scenario: A drop on a row of the Parked card
- **WHEN** the user drops whole branches, commit rows or parked branches on a row of the Parked card
- **THEN** it leaves the drag of branches to the card, and takes the commit rows and the parked branches as any row
  [@test] src/lib/branch-drop.test.ts (`moves dragged commits onto a branch, leaves a drag of branches that land to the Parked card from its rows, and waits as Move to… does`)

### Requirement: Drag and drop: The unpark zone unparks the dragged parked branches

The unpark zone of the drag bar MUST unpark the dragged parked branches; a branch parked with its parent goes along
with the parent; those that land stay as they are.

#### Scenario: Parked branches dropped on the unpark zone
- **WHEN** the user drops parked branches on the unpark zone
- **THEN** they are unparked, a branch parked with its parent along with the parent
- **AND** those that land stay as they are
  [@test] src/lib/branch-drop.test.ts (`unparks the dragged branches with a line of their own, the ones parked with them along, never one alone, and leaves those that land`)

### Requirement: Drag and drop: A Stack on chip stacks one dragged branch

A "Stack on ‹name›" chip over the Actions cell of the row under one dragged branch MUST stack it there instead of
moving its commits.

#### Scenario: One branch dropped on the Stack on chip
- **WHEN** the user drops one dragged branch on the "Stack on ‹name›" chip of a row
- **THEN** it is stacked there, its commits not moved
  [@test] src/lib/branch-drop.test.ts (`stacks one dragged branch on another, never on itself, its parent or a branch stacked on it`)

### Requirement: Drag and drop: The unstack zone makes a stacked branch a root

The unstack zone of the drag bar MUST make a stacked branch a root ([Stack Parents](../stack-parents/spec.md#app)).

#### Scenario: A stacked branch dropped on the unstack zone
- **WHEN** the user drops a stacked branch on the unstack zone
- **THEN** it becomes a root
  [@test] src/lib/branch-drop.test.ts (`unstacks one dragged branch that is stacked, and shows no zone for a root`)

### Requirement: Drag and drop: After the drop the row is scrolled into view

After a drop of whole branches that changes their shape, the row MUST be scrolled into view where it went (the Parked
card, the table).

#### Scenario: A branch parked by a drop
- **WHEN** a drop parks, unparks or stacks a branch
- **THEN** its row is scrolled into view where it went, the Parked card or the table

### Requirement: Drag and drop: A target that can't take the drag says why

A target that can't take the drag now MUST show and say why instead of its label, with the reason of the menu; the
pointer shows no drop there (`data-drop-target="refused"`), and a row or a card that refuses is not highlighted.

#### Scenario: A branch that can't be parked now
- **WHEN** a dragged branch can't be parked now
- **THEN** the park target says why instead of its label ("Cannot park docs: A Safe Push of docs is running…")
- **AND** the pointer shows no drop there (`data-drop-target="refused"`), and a row or a card that refuses is not highlighted
  [@test] tests/e2e/parked-branches/parked-branches.spec.ts (`should park a branch dragged onto the header of the Parked card, and unpark a stack by its root, never by a branch parked with it`)

#### Scenario: A stack that would be a cycle
- **WHEN** a dragged branch would be stacked on a branch stacked on it
- **THEN** the chip says why ("Cannot stack auth on oauth: oauth is stacked on it")
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should stack a dragged branch dropped on the Stack on chip of a row, refuse a cycle there, and unstack it on the unstack zone`)

### Requirement: Drag and drop: An issue row, the archived table and a collapsed island header are no targets

An issue row, the archived table and the header of a collapsed stack island MUST NOT be drop targets.

#### Scenario: A drag over them
- **WHEN** the user drags over an issue row, the archived table or the header of a collapsed stack island
- **THEN** none of them takes the drop

### Requirement: After a move: A move shows a toast with what it did and Undo

A move MUST show a toast "Moved N commits to X" (N counts the commits that moved, not those already there) with what it
did as the description and an Undo button, once the sync the rewrite asked showed it.

#### Scenario: The toast of a move
- **WHEN** a move ends and the sync it asked showed it
- **THEN** the toast says "Moved N commits to X", N the commits that moved, not those already there
- **AND** its description says what it did ("IJPL-2 goes to docs by the issue key from now on, without a rewrite"), and it has an Undo button

### Requirement: After a move: The operation of a move ends with the request of its sync

The operation of a move MUST end with the request of the sync the rewrite asked (a rewrite behind it runs meanwhile),
and its answer MUST come once the sync ended, with the version of its last patch (`Outcome.versionAfter`).

#### Scenario: A rewrite asks its sync
- **WHEN** the core of a rewrite wrote it and asked its sync
- **THEN** the operation ends with the request of that sync, and a rewrite behind it runs meanwhile
- **AND** its answer comes once the sync ended, with `Outcome.versionAfter` and its undo
  [@test] crates/operations/src/rewrite_test.rs (`a_rewrite_is_run_by_its_core_and_ends_once_it_asked_its_sync_with_its_undo`)

### Requirement: After a move: The toast of a move back says what it did

A move back (Ungroup, Move to Unassigned) MUST be "Removed the prefix of N commits" when it only removed prefixes, else
"Moved N commits back". The description never says what the title says.

#### Scenario: A move back that only removed prefixes
- **WHEN** a move back only removed prefixes
- **THEN** its toast is "Removed the prefix of N commits"
  [@test] tests/e2e/move-commits/move-commits.spec.ts (`should move a split commit back to its issue and a prefix commit to the unassigned commits`)

#### Scenario: Any other move back
- **WHEN** a move back did more than remove prefixes
- **THEN** its toast is "Moved N commits back"

#### Scenario: An Ungroup of a branch of one commit
- **WHEN** the user ungroups a branch of one commit
- **THEN** its toast is "Removed the prefix of 1 commit" alone, the description not saying it again
  [@test] src/lib/move-summary.test.ts (`titles the toast of a move with what it did and describes only the rest, never the title again`)

### Requirement: After a move: Undo moves HEAD and the issue groups back in one transaction

Undo of a move, a rename and a restack MUST move HEAD and the issue groups back in one ref transaction with
compare-and-swap, under the repository lock (the rewrite `undo` with the token of the outcome, `Outcome.undo`).

#### Scenario: A move undone
- **WHEN** the user presses Undo of a move
- **THEN** HEAD and the issue groups move back
  [@test] tests/e2e/move-commits/move-commits.spec.ts (`should move a commit to an existing branch from the context menu and undo it`)

#### Scenario: The rewrite undo with the token of the outcome
- **GIVEN** a rewrite that answered its undo token (`Outcome.undo`)
- **WHEN** the rewrite `undo` runs with that token
- **THEN** HEAD is the old one again
  [@test] crates/test-server/src/reword_test.rs (`a_reword_answers_the_undo_that_brings_the_old_head_back`)

#### Scenario: A rename undone
- **WHEN** a rename of a parent is undone
- **THEN** one undo moves HEAD back
  [@test] crates/sync-core/src/stack_operations_test.rs (`renaming_a_parent_renames_it_in_its_children`)

#### Scenario: A bad token or a moved HEAD
- **WHEN** an undo names a bad token, or HEAD moved since the outcome
- **THEN** the compare-and-swap refuses it
  [@test] crates/sync-core/src/stack_operations_test.rs (`undo_refuses_a_bad_token_and_a_moved_head`)

### Requirement: After a move: Undo runs in the repository the move was made in

Undo MUST run in the repository the move was made in: a move, a rename, a restack, a reword and a conflict fix name the
repository of the click, and their Undo clicked after a switch of the repository undoes there, without a sync of the
repository shown.

#### Scenario: Undo after a switch of the repository
- **GIVEN** a reword in one repository
- **WHEN** the user switches the repository and then presses the toast's Undo
- **THEN** it undoes in the repository of the click, without a sync of the repository shown
  [@test] src/hooks/useRewordCommit.test.ts (`undoes the reword in its repository when the toast's Undo comes after a switch of the repository`)

### Requirement: After a move: An Undo pressed during another rewrite waits behind it

An Undo pressed while another rewrite runs MUST wait behind it in the queue of the session and run after it.

#### Scenario: Undo while another rewrite runs
- **WHEN** the user presses Undo of a move while another rewrite runs
- **THEN** it waits behind it in the queue of the session and runs after it, the toast and its Undo kept until it undid
  [@test] tests/e2e/move-commits/move-commits.spec.ts (`should run the Undo of a move pressed while another rewrite runs after it, the toast and its Undo kept until it undid`)

### Requirement: After a move: An Undo pressed during a sync goes to the session at once

An Undo pressed during a sync MUST go to the session at once, never refused: it answers with the sync it asked, after
the running one, its toast kept until then.

#### Scenario: Undo during a held sync
- **GIVEN** a sync that runs
- **WHEN** the user presses Undo of a move
- **THEN** it is not refused, and it answers with the sync it asked, after the running one, its toast kept until then
  [@test] tests/e2e/move-commits/move-commits.spec.ts (`should undo a move pressed during a held sync once that sync ended, without a refusal, one sync for the undo`)

### Requirement: After a move: The same Undo asked three times is one undo

The same Undo asked three times (its toast, Activity, the Activity window) MUST be one undo with one outcome.

#### Scenario: Undo from the toast, Activity and the Activity window
- **WHEN** the same Undo is asked from its toast, Activity and the Activity window
- **THEN** it is one undo with one outcome
  [@test] crates/operations/src/rewrite_test.rs (`a_triple_undo_is_one_undo_with_one_outcome`)

### Requirement: After a move: An Undo after another process committed on top is stale

An Undo that waited for the lock of another process that committed on top meanwhile MUST be stale (`undo_stale`) and
write nothing.

#### Scenario: Another process committed on top while the Undo waited
- **GIVEN** an Undo that waits for the lock of another process
- **WHEN** that process commits on top meanwhile
- **THEN** the Undo is stale (`undo_stale`) and writes nothing
  [@test] crates/operations/src/rewrite_test.rs (`an_undo_waiting_for_the_lock_of_another_process_that_commits_on_top_is_stale_and_writes_nothing`)

### Requirement: After a move: The trial build offers to unstack a branch that builds alone

After the toast shows, the trial build of the stacked branches of the move MUST run, the target first, then the source
branches ([Stack Parents](../stack-parents/spec.md#a-stacked-branch-that-builds-alone)). The first one that builds alone
now gives the toast a second action. The move MUST NOT unstack a branch by itself.

#### Scenario: The target builds alone
- **WHEN** the target, which the toast names, builds alone now
- **THEN** the toast's second action is "Unstack from IJPL-257337"
  [@test] src/lib/stack.test.ts (`names the branch it unstacks unless it is the target the toast names`)
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should offer the move of the commit a stacked branch needs to land it alone, then its unstack, and review the branch alone`)

#### Scenario: A source branch builds alone
- **WHEN** a source branch builds alone now
- **THEN** the toast's second action is "Unstack a from r", which names it
- **AND** nothing is unstacked until the user chooses it
  [@test] src/lib/stack.test.ts (`names the branch it unstacks unless it is the target the toast names`)

### Requirement: After a move: The target branch expands with the moved commits selected

The target branch MUST expand and scroll into view, with the moved commits selected.

#### Scenario: Commits moved to a new branch
- **WHEN** a move ends
- **THEN** the target branch expands and scrolls into view, with the moved commits selected
  [@test] tests/e2e/move-commits/move-commits.spec.ts (`should split commits out of an issue branch into a new branch nested under the issue`)

### Requirement: Issue families: A prefix branch whose commits carry one issue key belongs to that issue

A prefix branch whose commits all carry the same issue key SHALL belong to that issue.

#### Scenario: A prefix branch of one issue
- **GIVEN** a prefix branch whose commits all carry the same issue key
- **WHEN** the branch table shows it
- **THEN** it belongs to that issue
  [@test] crates/sync-core/src/move_commits_test.rs (`splits_commits_out_of_an_issue_branch`)

### Requirement: Issue families: The branches of an issue follow the issue branch

The branches of an issue MUST follow the issue branch, nested under it, wherever they are in the order.

#### Scenario: An issue branch and its branches
- **GIVEN** an issue branch and other branches of the issue
- **WHEN** the branch table shows them
- **THEN** the other branches follow the issue branch, nested under it, wherever they are in the order
  [@test] src/stores/branch-sync.test.ts (`nests the branches of an issue under the issue branch, wherever it is in the order`)

### Requirement: Issue families: Branches of an issue without an issue branch nest under an issue row

Without an issue branch, two or more branches of an issue MUST be nested under an issue row with the key and the branch
count.

#### Scenario: Two branches of an issue without an issue branch
- **GIVEN** two or more branches of an issue and no issue branch
- **WHEN** the branch table shows them
- **THEN** they are nested under an issue row with the key and the branch count
  [@test] src/stores/branch-sync.test.ts (`heads the branches of an issue without a branch of its own with an issue row`)

### Requirement: Issue families: A single branch of an issue shows the key as a chip

A single branch of an issue without an issue branch MUST show the key as a chip next to its name.

#### Scenario: One branch of an issue without an issue branch
- **GIVEN** a single branch of an issue and no issue branch
- **WHEN** the branch table shows it
- **THEN** the key shows as a chip next to its name
  [@test] src/stores/branch-sync.test.ts (`shows a single branch of an issue without a branch of its own with a chip`)
  [@test] tests/e2e/move-commits/move-commits.spec.ts (`should rename a branch and name an issue branch`)

### Requirement: Issue families: A prefix branch of an issue shows the issue summary

A prefix branch of an issue MUST show the issue summary: the subject after the issue key.

#### Scenario: The summary of a prefix branch of an issue
- **GIVEN** a prefix branch of an issue
- **WHEN** the branch table shows it
- **THEN** its summary is the subject after the issue key
  [@test] crates/sync-core/src/move_commits_test.rs (`splits_commits_out_of_an_issue_branch`)

### Requirement: Issue families: Issue families are formed from the stack roots only

Issue families MUST be formed from the stack roots only; a stacked branch stays under its parent and shows its issue as
a chip when the parent's issue is another one.

#### Scenario: A child of another issue
- **GIVEN** a stacked branch whose issue is another than its parent's
- **WHEN** the branch table forms the issue families
- **THEN** it stays under its parent and shows its issue as a chip
  [@test] src/stores/branch-table-rows.test.ts (`forms issue families from the stack roots only and marks a child of another issue`)

## User Experience

- The selection's actions sit at the right end of the meta line of the lead row (`leadActions` of `CommitList`), "N
  selected" muted `text-xs` like the meta line; their `xs` buttons overhang its 16px line (`-my-1`, as the "⋯" of a
  card), and the meta line keeps to one line beside them (`singleLine`), its end clipped where the row is narrow. The
  "Move to…" form is a row of the table below the lead row (`leadForm`).
- "Move to…" is an outline `xs` button in the primary colour. The way back is a ghost `xs` button.
- The branch selection bar sits right-aligned over the column heads of the branch table (`absolute`, `h-8` like the
  heads, `bg-card`; the heads are `invisible` under it, so the columns keep their widths), with a ghost `icon-xs`
  clear button and "Stack Together" when at least two branches are selected and stacks are enabled; a selected branch
  row is `bg-primary/15`, like a selected commit row.
- Of the droppable branch rows only the one under the pointer shows: `bg-primary/10` and a solid primary outline, its
  rows below `bg-primary/5`; a dragged row is at half opacity. The unassigned card and the Parked card get a primary
  ring.
- The warning of the form is `text-warning`; the note about stacked children is muted.
- The chips of the form are `xs` buttons (`ChoiceChip`) in one style for the existing branches and the AI suggestions:
  the chip whose name the input holds is soft primary (`bg-primary/10 text-primary`, `aria-pressed`), the others
  `secondary`. A suggestion still being generated is a disabled `secondary` chip with a spinner and "Generating…".
- The drag bar is a popover surface (`bg-popover`, `shadow-lg`, `rounded-xl`) centred over the bottom of the window,
  24 px above its edge. Its zones are dashed `border-primary/50` panels (`rounded-lg`) in one row: Move to New Branch
  (`GitBranchPlus`), Park ‹names› (`CirclePause`), Unpark ‹names› (`CirclePlay`), Unstack ‹name› (`Layers2`). A refused
  zone is `border-border` and `text-muted-foreground`; the reason of the target under the pointer is a muted `xs` line
  above the zones.
- The "Stack on ‹name›" chip is a dashed `border-primary` `xs` chip with a `Layers` icon over the Actions cell, solid
  `bg-primary` under the pointer, muted with the reason when refused.
- The drag image is a primary badge at the pointer: "N commits", "docs · 3 commits", or "2 branches · 5 commits".
- A nested branch row is indented under the issue's icon, behind a `border-l border-border` rule.
- The issue chip is an `outline` badge with the key as an issue link.

## Testing / Local Run

- `pnpm test:backend`: `move_commits_test.rs`, `reword_test.rs`, `sync_test.rs`.
- `pnpm test:unit`: the words of a typed name (`src/presenters/branch-name.test.ts`, against the oracle Rust writes) and
  the issue families (`src/stores/branch-sync.test.ts`); the grouping rule is `commit_grouper_test.rs`.
- `pnpm e2e`: `tests/e2e/move-commits` uses the `issue_split` and `issue_combine` templates.

## Open Questions / Risks

- A split can create a conflict when a moved commit depends on a commit left behind. The sync shows it, and Undo
  restores the previous grouping.
- The undo needs HEAD and the issue groups where the move left them: a commit or a rewrite in a terminal between the move
  and the undo makes it fail with an error toast.
- The issue groups are shared by every worktree: a whole issue moved here also moves its commits in another worktree.
- A `fixup! <id>` names its target by hash: a prefix in front of the id leaves it without a target.

## References

- [Virtual Branches](../../../../docs/design/virtual-branches.md)
- [Commit Row](../../commit/commit-row/spec.md)
- [Branch Name Suggestions](../branch-name-suggestions/spec.md): the AI suggestions of the form
