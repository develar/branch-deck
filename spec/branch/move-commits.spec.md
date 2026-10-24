---
name: Move Commits
description: Requirements for moving commits and whole branches into one branch with the smallest change (a whole issue by the issue groups, the rest by the explicit prefix), from every gesture, renaming and ungrouping a branch, and nesting the branches of an issue.
style: plain-1
targets:
  - ../../crates/history-rewrite/src/move_commits.rs
  - ../../crates/history-rewrite/src/move_plan.rs
  - ../../crates/sync-core/src/stack_operations.rs
  - ../../crates/history-rewrite/src/regroup.rs
  - ../../crates/history-rewrite/src/snapshot.rs
  - ../../crates/history-rewrite/src/repository_lock.rs
  - ../../crates/history-rewrite/src/reword.rs
  - ../../crates/history-rewrite/src/commit_grouper.rs
  - ../../crates/sync-core/src/sync.rs
  - ../../src/hooks/useMoveCommits.ts
  - ../../src/hooks/useMovePreview.ts
  - ../../src/hooks/useStackActions.ts
  - ../../src/lib/stack.ts
  - ../../src/lib/move-summary.ts
  - ../../src/lib/branch-selection.ts
  - ../../src/hooks/useCommitMoveList.ts
  - ../../src/hooks/useCommitDrag.ts
  - ../../src/hooks/useBranchDropTarget.ts
  - ../../src/lib/branch-drop.ts
  - ../../src/components/branch/BranchDropZones.tsx
  - ../../src/components/branch/DragTargetBar.tsx
  - ../../src/pages/IndexPage.tsx
  - ../../src/hooks/useBranchContextActions.ts
  - ../../src/hooks/useBranchRowKeys.ts
  - ../../src/components/commit/MovableCommitList.tsx
  - ../../src/components/shared/InlineBranchTarget.tsx
  - ../../src/components/shared/ChoiceChip.tsx
  - ../../src/components/ai/BranchNameSuggestions.tsx
  - ../../src/components/branch/InlineRenameBranchInput.tsx
  - ../../src/components/branch/InlineUngroupConfirm.tsx
  - ../../src/stores/inline-row-action.ts
  - ../../src/components/branch/BranchTableCard.tsx
  - ../../src/components/branch/BranchSelectionBar.tsx
  - ../../src/components/branch/BranchRow.tsx
  - ../../src/components/branch/BranchContextMenuRow.tsx
  - ../../src/components/unassigned/UnassignedCommitListCard.tsx
  - ../../src/components/branch/IssueFamilyRow.tsx
  - ../../src/stores/commit-move.ts
  - ../../src/stores/branch-sync.ts
---

# Move Commits

Status: Implemented
Date: 2026-10-15

## Summary

A branch is what ships together: one push, one Safe Push, one pull request. A commit belongs to a branch through its
subject: an explicit `(name)` prefix wins, then the [issue group](issue-groups.spec.md) of its issue key, then the key
(`ABC-123: …`). One operation, "move these commits to branch X", serves every list, the branch rows, the context menus,
the keyboard and drag and drop, and it makes the smallest change:

- the whole of an issue (every commit of its key without a prefix) moves by the issue groups, without a rewrite, so the
  next commit of the issue goes there too: `IJPL-1`, `IJPL-2` and `IJPL-3` dragged onto `maps` ship as one branch,
  also when `maps` is stacked;
- a part of an issue and a commit without a key get the prefix, so `ABC-123: Add lexer` becomes
  `(lexer) ABC-123: Add lexer` (the issue key stays in the pushed message); a target path `auth/lexer` stacks `lexer` on
  `auth` in the stack parents ([Stack Parents](stack-parents.spec.md)), so the prefix is the bare `(lexer)`;
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

### Move operation

- A move must make the smallest regroup (`plan_move`), computed from the commits read under the repository lock. An
  issue is whole when the move takes every commit of its key without a prefix, and there is at least one; the fixups
  of a moved commit count as moved.
  [@test] ../../crates/sync-core/src/move_commits_test.rs (`whole_issues_join_a_branch_without_a_rewrite_and_their_next_commits_follow`, `the_fixups_of_a_whole_issue_move_with_it`)
- A whole issue must move by the issue groups (`KEY -> X`), without a rewrite, also when its key was in another issue
  group; a whole issue moved to the branch of its own key leaves its issue group. A part of an issue and a commit
  without a key must get the prefix `(X)`, in the same history rewrite and ref transaction.
  [@test] ../../crates/sync-core/src/move_commits_test.rs (`a_part_of_an_issue_and_a_commit_without_a_key_get_the_prefix_in_the_same_transaction`, `moving_an_issue_group_reassigns_its_keys_and_moving_it_back_ungroups_them`)
- A commit without a key moved into the branch of an issue (its prefix is the key) must be pushed with the key, where
  the issue key of a subject is read (after a `[category]` and a `type(scope):`); a subject with a key of its own is
  pushed as is. Its Move Back item must move it to the unassigned commits.
  [@test] ../../crates/history-rewrite/src/commit_grouper_test.rs (`a_prefix_that_is_an_issue_key_stays_in_the_virtual_subject_as_the_key`, `a_key_prefix_adds_no_key_to_a_subject_with_a_key_of_its_own`, `the_leaf_of_a_stack_prefix_that_is_an_issue_key_stays_as_the_key`)
  [@test] ../../crates/sync-core/src/branch_processor_test.rs (`a_commit_moved_into_the_branch_of_an_issue_is_copied_with_the_key`)
  [@test] ../../src/hooks/useCommitMoveList.test.ts (`moves a commit of an issue prefix without a key of its own to the unassigned commits`)
- A commit that its key puts into X without a prefix (an issue branch named X, an issue group X) must lose its prefix,
  not get `(X)`.
  [@test] ../../crates/sync-core/src/move_commits_test.rs (`a_prefixed_commit_that_its_key_puts_into_the_target_loses_the_prefix`)
- A stacked target (a path `auth/login`, or a branch with a declared parent) must take a whole issue by the issue
  groups, not by the prefix; a path writes its stack parent, and a commit of the branch whose prefix declares another
  parent gets the bare leaf.
  [@test] ../../crates/sync-core/src/move_commits_test.rs (`a_whole_issue_joins_a_stacked_target_without_a_rewrite`)
- A prefix must replace an existing one and keep the rest of the subject and the body.
- A move without a target (back) must remove the explicit prefix: the commit goes back to its issue group, its issue
  branch, or to the unassigned commits. A whole issue in an issue group must leave it; a part of one must get the
  prefix of its key.
- A dry run must say what the move would do and write nothing, without the lock; a move may name the HEAD it expects.
  [@test] ../../crates/sync-core/src/move_commits_test.rs (`a_dry_run_writes_nothing_and_expect_head_refuses_a_moved_head`)
- An autosquash marker must stay in front: `fixup! (a) X` becomes `fixup! (b) X`.
- The fixups of a moved commit must move with it, and be listed after the requested commits for the undo.
  [@test] ../../crates/sync-core/src/move_commits_test.rs (`moves_the_fixups_with_their_target`)
- A move is a regroup plan: it must be validated against every subject ahead of the baseline before anything is
  written, and with stacked branches on, refused if it introduces a stack problem; also when it recreates a commit
  that a `fixup! <id>` names by its id, which would leave the fixup without a target.
  [@test] ../../crates/sync-core/src/regroup_test.rs (`validates_the_plan_before_writing`, `fixups_follow_their_targets`, `refuses_problems_the_plan_introduces_and_keeps_existing_ones`)
  [@test] ../../crates/sync-core/src/reword_test.rs (`a_reword_that_strands_a_fixup_is_refused`)
- Rewrites must hold the repository lock, so the app and the agent CLI don't rewrite the branch at the same time.
  [@test] ../../crates/sync-core/src/regroup_test.rs (`waits_for_the_repository_lock`)
- A move, a rename, a restack and their undo of the app must wait for the lock as long as its holder lives (an
  agent's `bd pick --follow`, `bd`, the app's own push), never refused after 10 s: a move started from a menu shows
  its spinner on the "⋯" of its row, whose hover says whom it waits for ("Waiting for `bd pick` (pid 4242, for 12
  s)") and whose click offers Stop Waiting, which ends the wait with nothing moved ("Canceled moving 1 commit to docs
  before it started"); the surface that started it says the same. Once the holder released the lock the move runs.
  Only a holder no record names refuses it after 10 s.
  [@test] ../../tests/e2e/repository/repository-lock.spec.ts (`should show the wait of a move started from a menu on the row's ⋯, whose menu stops it, nothing moved`)
  [@test] ../../crates/test-server/src/channel_commands_test.rs (`a_rewrite_says_whom_it_waits_for_and_runs_once_the_holder_released_the_lock`, `stop_waiting_ends_the_wait_of_a_rewrite_with_nothing_changed`)
  [@test] ../../crates/history-rewrite/src/lock_wait_test.rs (`a_wait_is_stopped_by_its_channel_and_leaves_the_registry_with_its_ticket`)
  [@test] ../../src/stores/operations.test.ts (`says whom the command of a running rewrite waits for, offers Stop Waiting on its channel, and says its own state again once the wait ended`)
  [@test] ../../src/lib/lock-wait.test.ts (`says that an action stopped while it waited for the repository lock changed nothing`, `tells a stopped wait from a failure by the code of the command's error`)
- A commit already in the target branch must not be rewritten; the result lists it as `Unchanged`.
- All commits must be rewritten in one history rewrite that keeps the trees, the metadata and the working tree: a
  move of ten commits runs the git processes of a move of one.
  [@test] ../../crates/sync-core/src/move_commits_test.rs (`moves_many_commits_in_one_rewrite`)
- The rewrite must keep the author and the committer of every recreated commit: name, email, date and timezone.
  [@test] ../../crates/sync-core/src/reword_test.rs (`reword_preserves_committer`)
- The target name must be a name the grouper keeps as is: letters, numbers, `-`, `_`, `.` and `/`, not at the ends.
  [@test] ../../crates/sync-core/src/move_commits_test.rs (`sets_replaces_and_removes_the_prefix`, `keeps_the_autosquash_marker`, `validates_the_branch_name`, `moves_unassigned_commits_to_a_new_branch`, `moves_to_another_branch_and_back_to_the_issue`, `skips_commits_already_in_the_target_branch`, `keeps_the_body_and_the_metadata`, `keeps_the_checked_out_branch`, `refuses_an_invalid_target_or_an_unknown_commit`, `moves_many_commits_in_one_rewrite`)
- A split prefix branch must keep the issue key of its commits.
  [@test] ../../crates/sync-core/src/move_commits_test.rs (`splits_commits_out_of_an_issue_branch`)

### Selection and the "Move to…" form

- The commits of a branch table row and of the unassigned commits card must be selectable.
- The branch rows must be selectable: Cmd/Ctrl-click toggles a row, Shift-click selects the rows from the last clicked
  one, Cmd+A selects all, Escape clears; a plain click expands the row and clears the selection. Under the filter of the
  table, Shift-click takes the shown rows, Cmd+A the rows the filter matched (not the dimmed ones), and a selected
  branch the filter hides stays selected: the bar adds "· 1 hidden" ([Branch Filter](branch-filter.spec.md)). The rows
  a collapsed stack hides count as hidden too: Shift-click and Cmd+A skip them, a selected one stays selected.
  [@test] ../../src/lib/branch-selection.test.ts (`selects a range of rows from the anchor, in table order`)
  [@test] ../../tests/e2e/move-commits/combine-branches.spec.ts (`should move the selected branches to a new branch with Move to…`)
  [@test] ../../tests/e2e/branch-filter/branch-filter.spec.ts (`should select only the matches with Cmd+A and keep a hidden selected branch`, `should not select the branches of a collapsed stack with Cmd+A, and count them as hidden`)
- Only one list must hold a selection, the branch table included: selecting in another list must clear it.
  [@test] ../../tests/e2e/commit-message/commit-message.spec.ts (`should select commit rows of a branch, one list at a time`)
  [@test] ../../src/stores/commit-move.test.ts (`clears the selected branches and their form when a commit list takes the selection`)
- A selection must show "N selected" and a "Move to…" button, above the branch commits or in the unassigned card header;
  selected branches "N branches selected", "Move to…" and a clear button above the branch table.
- A selection with a way back must also offer it: "Move Back to ABC-123" (the issue group of the key when it has
  one, see [Issue Groups](issue-groups.spec.md); the issue branch of the key for a commit an issue group holds), "Move
  to Unassigned", "Remove Prefix", or "Move Back" when they go back to several branches.
  [@test] ../../src/hooks/useCommitMoveList.test.ts (`moves the commits that the issue groups put into a branch back to the issue branch of their key`)
- "Move to…", Enter or M must open the form, for the selected commits or for all commits of the selected branches. One
  name input decides: an existing branch name moves the commits there, a new name creates the branch. The help text and
  the submit button ("Move" or "Create") must say which.
- The help text must say what the move does, from its dry run: the issues that go there by their key, without a
  rewrite, and the commits that get the prefix ("rewrites them to start with (lexer)" when all do). A refused plan must
  show its reason and disable the submit; commits all in the branch already say so.
- The form must warn when it empties a pushed branch: its remote branch and pull request stay. A child whose parent the
  move empties is a muted note: it stays stacked on the parent and is built on the baseline until the parent has
  commits again.
- The form must offer the existing branches as chips, the branches of the same issue first, then the AI suggestions
  ([Branch Name Suggestions](branch-name-suggestions.spec.md)).
  [@test] ../../tests/e2e/move-commits/move-commits.spec.ts (`should split commits out of an issue branch into a new branch nested under the issue`)
- With [stacked branches](stacked-branches.spec.md) on, the input may name a parent (`auth/login`), the target is
  looked up by its leaf, the help text names the parent ("Moves 2 commits to login (on auth)", "Creates login on
  auth"), says that a parent other than the existing branch's restacks it ("Moves 2 commits to login and stacks it on
  auth instead of docs"), and the branches of the same stack come first. A bare name takes the branch's stack parent.
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should move commits to a new branch on a parent with Move to…`)
  [@test] ../../src/stores/branch-table-rows.test.ts (`offers the branches of the same stack first as move targets`)

### Context menus

- The commit menu must offer "Move to…" with the key `M` (the label of the selection button, for one commit or many),
  a "Move to Existing" submenu with the branches and "New Branch…", and the way back of the commits.
  [@test] ../../tests/e2e/move-commits/move-commits.spec.ts (`should move a commit to an existing branch from the context menu and undo it`, `should move a split commit back to its issue and a prefix commit to the unassigned commits`)
  [@test] ../../tests/e2e/unassigned-commits/unassigned-commits.spec.ts (`should act on all selected commits from the context menu of a selected row`, `should act on that commit only from the context menu of a row that is not selected`)
- The branch menu must offer "Move to…" with the key `M`, for all commits of the branch, or of every selected branch
  when the row is selected; it has no "Move to Existing ›": the form offers the existing branches. `M` works where the item shows it: on the focused row without a
  selection it selects the row and opens "Move to…" for it, as the item does; with selected rows it opens it for them.
  [@test] ../../tests/e2e/move-commits/combine-branches.spec.ts (`should open Move to… for the focused row with M, without a selection`, `should move the selected branches to a new branch with Move to…`)
- The branch menu must offer "Rename…" for a prefix branch and "Name This Branch…" for an issue branch, through
  `rename_branch`; an existing name merges the branches. "Name This Branch…" and "Rename…" of an issue group change the
  issue groups only, without a rewrite, also when the issue branch is named after a stacked one.
  [@test] ../../crates/sync-core/src/regroup_test.rs (`renames_flat_and_issue_branches`, `an_issue_branch_renamed_into_a_stacked_branch_joins_it_by_the_issue_groups`)
- The branch menu of a prefix branch and of an issue group must offer "Ungroup": it moves all its commits back, the
  prefixes go and the issues leave the issue group.
  [@test] ../../tests/e2e/move-commits/move-commits.spec.ts (`should rename a branch and name an issue branch`)
- While a rename or an ungroup runs, its form must show the progress row ("Renaming docs to documentation…"); it closes
  when the commits moved. A failure must keep the form with the typed name (the error is a toast).
- With [stacked branches](stacked-branches.spec.md) on, Rename must rename the branch in the prefixes of the branches
  stacked on it too, and in the stack parents (`rename_branch`), and the new name must be a branch name without `/`. Ungroup and Unapply of a
  branch that others stack on must confirm and name them: they stay stacked on it and are built on the baseline until
  it has commits again. The
  confirmation focuses its button, so Enter confirms.
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should rename a parent with its children following`, `should build a child on the baseline once its parent has no commits, and unstack it`)

### Drag and drop

- A commit row must be draggable. Dragging a selected row must drag the selection; dragging another row must drag it
  alone and leave the selection and its form as they are.
- A branch row must be draggable, also while a sync runs: it drags the selected branches when it is one of them, else
  itself alone, with all their commits; a row of the Parked card too. The dragged rows fade (`data-dragging`).
  [@test] ../../tests/e2e/move-commits/combine-branches.spec.ts (`should drag an issue branch onto a branch: the issue joins it without a rewrite, and Undo moves it back`)
  [@test] ../../tests/e2e/parked-branches/parked-branches.spec.ts (`should park a branch dragged onto the park zone of the drag bar, unpark it dragged onto the unpark zone, take both back with Undo, and drag the commits of a parked row onto a branch`)
  [@test] ../../tests/e2e/parked-branches/parked-branches.spec.ts (`should park an unselected branch dropped on the collapsed Parked card and keep the selection, and park and unpark the selection with its buttons, of both kinds by the part each takes`)
- A drag must start without a change of the layout: nothing shows or goes above the dragged row, so it stays under the
  pointer.
  [@test] ../../tests/e2e/move-commits/combine-branches.spec.ts (`should drop a commit on the expanded commits of a branch, and drag an unselected row alone, the selection as it was`)
  [@test] ../../tests/e2e/parked-branches/parked-branches.spec.ts (`should keep the rows in place when a drag starts, show the drag bar in view at the bottom of a long list, and take no drop on the bar from a drag that started under it`)
- Another branch must accept the drop on its row and on its rows below (the inline form, the expanded commits) and
  move the commits there. The source branch must not accept a drag that comes from it alone.
  [@test] ../../tests/e2e/move-commits/move-commits.spec.ts (`should drag the selection onto a branch, but not onto its own branch`)
  [@test] ../../src/stores/commit-move.test.ts (`drops anywhere but on the one list a drag comes from`)
- The unassigned commits card must accept the commits and branches that have a way back, and move them back.
- A target must wait as the menu of the same action does, and say why while the pointer is over it: a drag of whole
  branches while a sync runs or one of them failed to sync, as Move to… of a branch; a drag of commit rows moves during
  a sync, as Move to… of a commit. A target takes a drop while a rewrite of the repository runs: its move waits for
  that rewrite in the queue ([Running Operations](../../docs/design/operations.md)) and then moves the commits as the
  rewrite left them, by the ids it gave them. Its answer follows the sync and the rewrites while the drag goes on, and
  a drop asks it once more and takes the commits as the last sync left them (a commit a rewrite gave another id is
  found by its author, author time and subject): gone ones refuse the drop with an info toast.
  [@test] ../../tests/e2e/move-commits/move-commits.spec.ts (`should take a second drag while the first move runs, wait for it in the queue and move the commit as the first move left it`)
  [@test] ../../tests/e2e/move-commits/combine-branches.spec.ts (`should drag a branch while a sync runs, say why the targets wait for it, and take the drop once it ended`)
  [@test] ../../src/lib/branch-drop.test.ts (`moves dragged commits onto a branch, leaves a drag of branches that land to the Parked card from its rows, and waits as Move to… does`)
- While commits or branches are dragged, the drag bar must show over the bottom of the window, in view wherever the
  page is scrolled and out of the flow of the content, which gets room at its end so that every row scrolls out from
  under it; it leaves the column of the toasts free. It holds the targets that change the shape of what is dragged,
  each while the drag is for it, and above them why the target under the pointer refuses the drag. A drag that starts
  under the bar takes no drop on it until the pointer left it (`data-armed`). The drag ends with the `dragend` of its
  row also when a sync remounted the row, else with the next mouse move, press or key.
  [@test] ../../tests/e2e/parked-branches/parked-branches.spec.ts (`should keep the rows in place when a drag starts, show the drag bar in view at the bottom of a long list, and take no drop on the bar from a drag that started under it`)
- Move to New Branch in the drag bar shows also when there is no branch yet. A drop there must open the "Move to…" form
  for the dragged commits above the branch table, in view and focused; the form keeps their ids and closes when a
  rewrite gave one of them another id. It waits for nothing: the form's move does.
  [@test] ../../tests/e2e/move-commits/move-commits.spec.ts (`should drag an unassigned commit onto the new branch zone`)
  [@test] ../../tests/e2e/move-commits/combine-branches.spec.ts (`should offer the new branch zone when there is no branch yet`)
- A drag of whole branches must also change their shape, where the target says so, at once, with a toast and Undo
  (`branch-drop.ts`); the label of a zone names what its drop takes ("Park docs and api"); a drag of commit rows passes
  these targets by:
  - The park zone of the drag bar and the whole Parked card, also collapsed (its header, its rows), park the dragged
    branches that are not parked, without a note, in one write ([Parked Branches](parked-branches.spec.md)); those
    parked already stay as they are. A row of the Parked card leaves such a drag to the card; it takes a drag of
    commit rows and of parked branches as any row.
  - The unpark zone of the drag bar unparks the dragged parked branches; a branch parked with its parent goes along
    with the parent; those that land stay as they are.
  - With stacked branches on, a "Stack on ‹name›" chip over the Actions cell of the row under one dragged branch
    stacks it there instead of moving its commits; the unstack zone of the drag bar makes a stacked branch a root
    ([Stack Parents](stack-parents.spec.md#app)).
  - After the drop the row is scrolled into view where it went (the Parked card, the table).
  [@test] ../../src/lib/branch-drop.test.ts (`parks the dragged branches that land and leaves the parked ones, refuses one a robot lands, and waits for the sync`)
  [@test] ../../src/lib/branch-drop.test.ts (`unparks the dragged branches with a line of their own, the ones parked with them along, never one alone, and leaves those that land`)
  [@test] ../../src/lib/branch-drop.test.ts (`stacks one dragged branch on another, never on itself, its parent or a branch stacked on it`)
  [@test] ../../src/lib/branch-drop.test.ts (`unstacks one dragged branch that is stacked, and shows no zone for a root`)
- A target that can't take the drag now must show and say why instead of its label, with the reason of the menu
  ("Cannot park docs: A Safe Push of docs is running…", "Cannot stack auth on oauth: oauth is stacked on it"); the
  pointer shows no drop there (`data-drop-target="refused"`), and a row or a card that refuses is not highlighted.
  [@test] ../../tests/e2e/parked-branches/parked-branches.spec.ts (`should park the selected branches dropped on a row or the header of the Parked card at once with one Undo, and unpark a stack by its root, never by a branch parked with it`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should stack a dragged branch dropped on the Stack on chip of a row, refuse a cycle there, and unstack it on the unstack zone`)
- An issue row, the archived table and the header of a collapsed stack island are no targets.

### After a move

- A move must show a toast "Moved N commits to X" (N counts the commits that moved, not those already there) with what
  it did as the description ("IJPL-2 goes to docs by the issue key from now on, without a rewrite") and an Undo button.
- Undo of a move, a rename and a restack must move HEAD and the issue groups back in one ref transaction with
  compare-and-swap, under the repository lock (`undo_regroup` with the `undo_token` of the result), in the repository
  the move was made in: a move, a rename, a restack, a reword and a conflict fix name the repository of the click, and
  their Undo clicked after a switch of the repository undoes there, without a sync of the repository shown.
  [@test] ../../src/hooks/useRewordCommit.test.ts (`undoes the reword in its repository when the toast's Undo comes after a switch of the repository`)
  [@test] ../../tests/e2e/move-commits/move-commits.spec.ts (`should move a commit to an existing branch from the context menu and undo it`)
  [@test] ../../crates/sync-core/src/stack_operations_test.rs (`renaming_a_parent_renames_it_in_its_children`, `undo_refuses_a_bad_token_and_a_moved_head`)
- After the toast shows, the trial build of the stacked branches of the move runs, the target first, then the source
  branches ([Stack Parents](stack-parents.spec.md#a-stacked-branch-that-builds-alone)). The first one that builds
  alone now gives the toast a second action: "Unstack from IJPL-257337" for the target, which the toast names, and
  "Unstack a from r" for a source branch. The move never unstacks a branch by itself.
  [@test] ../../src/lib/stack.test.ts (`names the branch it unstacks unless it is the target the toast names`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should offer the move of the commit a stacked branch needs to land it alone, then its unstack, and review the branch alone`)
- The target branch must expand and scroll into view, with the moved commits selected.
  [@test] ../../tests/e2e/move-commits/move-commits.spec.ts (`should split commits out of an issue branch into a new branch nested under the issue`)

### Issue families

- A prefix branch whose commits all carry the same issue key belongs to that issue.
- The branches of an issue must follow the issue branch, nested under it, wherever they are in the order.
- Without an issue branch, two or more branches of an issue must be nested under an issue row with the key and the
  branch count.
- A single branch of an issue without an issue branch must show the key as a chip next to its name.
  [@test] ../../src/stores/branch-sync.test.ts (`nests the branches of an issue under the issue branch, wherever it is in the order`, `heads the branches of an issue without a branch of its own with an issue row`, `shows a single branch of an issue without a branch of its own with a chip`)
  [@test] ../../tests/e2e/move-commits/move-commits.spec.ts (`should rename a branch and name an issue branch`)
- A prefix branch of an issue must show the issue summary: the subject after the issue key.
- With stacked branches on, issue families are formed from the stack roots only; a stacked branch stays under its
  parent and shows its issue as a chip when the parent's issue is another one.
  [@test] ../../src/stores/branch-table-rows.test.ts (`forms issue families from the stack roots only and marks a child of another issue`)

## User Experience

- The selection actions sit right-aligned above the branch commits, in the card header for the unassigned commits.
- "Move to…" is an outline `xs` button in the primary colour. The way back is a ghost `xs` button.
- The branch selection bar sits right-aligned above the branch table header, with a ghost `icon-xs` clear button; a
  selected branch row is `bg-primary/15`, like a selected commit row.
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
- `pnpm test:unit`: the grouping rule (`src/lib/commit.test.ts`) and the issue families (`src/stores/branch-sync.test.ts`).
- `pnpm e2e`: `tests/e2e/move-commits` uses the `issue_split` and `issue_combine` templates.

## Open Questions / Risks

- A split can create a conflict when a moved commit depends on a commit left behind. The sync shows it, and Undo
  restores the previous grouping.
- The undo needs HEAD and the issue groups where the move left them: a commit or a rewrite in a terminal between the move
  and the undo makes it fail with an error toast.
- The issue groups are shared by every worktree: a whole issue moved here also moves its commits in another worktree.
- A `fixup! <id>` names its target by hash: a prefix in front of the id leaves it without a target.

## References

- [Virtual Branches](../../docs/design/virtual-branches.md)
- [Commit Row](../commit/commit-row.spec.md)
- [Branch Name Suggestions](branch-name-suggestions.spec.md): the AI suggestions of the form
