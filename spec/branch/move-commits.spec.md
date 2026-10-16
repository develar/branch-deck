---
name: Move Commits
description: Requirements for moving commits and whole branches into one branch with the smallest change (a whole issue by the issue groups, the rest by the explicit prefix), from every gesture, renaming and ungrouping a branch, and nesting the branches of an issue.
style: plain-1
targets:
  - ../../crates/sync-core/src/move_commits.rs
  - ../../crates/sync-core/src/move_plan.rs
  - ../../crates/sync-core/src/stack_operations.rs
  - ../../crates/sync-core/src/regroup.rs
  - ../../crates/sync-core/src/repository_lock.rs
  - ../../crates/sync-core/src/reword_commit.rs
  - ../../crates/sync-core/src/commit_grouper.rs
  - ../../crates/sync-core/src/sync.rs
  - ../../src/hooks/useMoveCommits.ts
  - ../../src/hooks/useMovePreview.ts
  - ../../src/hooks/useStackActions.ts
  - ../../src/lib/move-summary.ts
  - ../../src/lib/branch-selection.ts
  - ../../src/hooks/useCommitMoveList.ts
  - ../../src/hooks/useCommitDrag.ts
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
  written, and with stacked branches on, refused if it introduces a stack problem.
  [@test] ../../crates/sync-core/src/regroup_test.rs (`validates_the_plan_before_writing`, `fixups_follow_their_targets`, `refuses_problems_the_plan_introduces_and_keeps_existing_ones`)
- Rewrites must hold the repository lock, so the app and the agent CLI don't rewrite the branch at the same time.
  [@test] ../../crates/sync-core/src/regroup_test.rs (`waits_for_the_repository_lock`)
- A commit already in the target branch must not be rewritten; the result lists it as `Unchanged`.
- All commits must be rewritten in one history rewrite that keeps the trees, the metadata and the working tree.
- The rewrite must keep the author and the committer of every recreated commit: name, email, date and timezone.
  [@test] ../../crates/sync-core/src/reword_commit_test.rs (`reword_preserves_committer`)
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
- The branch menu must offer "Move to…" with the key `M` and "Move to Existing ›", for all commits of the branch, or of
  every selected branch when the row is selected. `M` works where the item shows it: on the focused row without a
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

- A commit row must be draggable. Dragging a selected row must drag the selection; dragging another row must make it
  the selection, so what moves is what is selected.
- A branch row must be draggable: it drags the branch, or every selected branch, with all their commits.
  [@test] ../../tests/e2e/move-commits/combine-branches.spec.ts (`should drag an issue branch onto a branch: the issue joins it without a rewrite, and Undo moves it back`)
- Another branch must accept the drop on its row and on its rows below (the inline form, the expanded commits) and
  move the commits there. The source branch must not accept a drag that comes from it alone.
  [@test] ../../tests/e2e/move-commits/move-commits.spec.ts (`should drag the selection onto a branch, but not onto its own branch`)
  [@test] ../../tests/e2e/move-commits/combine-branches.spec.ts (`should drop a commit on the expanded commits of a branch, and drag an unselected row instead of the selection`)
  [@test] ../../src/stores/commit-move.test.ts (`drops anywhere but on the one list a drag comes from`)
- The unassigned commits card must accept the commits and branches that have a way back, and move them back.
- While commits are dragged, a "new branch" zone must appear above the branch table, also when there is no branch yet.
  A drop there must open the "Move to…" form for the dragged commits; the form keeps their ids and closes when a rewrite
  gave one of them another id.
  [@test] ../../tests/e2e/move-commits/move-commits.spec.ts (`should drag an unassigned commit onto the new branch zone`)
  [@test] ../../tests/e2e/move-commits/combine-branches.spec.ts (`should offer the new branch zone when there is no branch yet`)

### After a move

- A move must show a toast "Moved N commits to X" (N counts the commits that moved, not those already there) with what
  it did as the description ("IJPL-2 goes to docs by the issue key from now on, without a rewrite") and an Undo button.
- Undo of a move, a rename and a restack must move HEAD and the issue groups back in one ref transaction with
  compare-and-swap, under the repository lock (`undo_regroup` with the `undo_token` of the result).
  [@test] ../../tests/e2e/move-commits/move-commits.spec.ts (`should move a commit to an existing branch from the context menu and undo it`)
  [@test] ../../crates/sync-core/src/stack_operations_test.rs (`renaming_a_parent_renames_it_in_its_children`, `undo_refuses_a_bad_token_and_a_moved_head`)
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
- A droppable branch row has a dashed `outline-primary/30`. The row under the pointer gets `bg-primary/10` and a solid
  primary outline, its rows below `bg-primary/5`. The unassigned card gets a primary ring.
- The warning of the form is `text-warning`; the note about stacked children is muted.
- The chips of the form are `xs` buttons (`ChoiceChip`) in one style for the existing branches and the AI suggestions:
  the chip whose name the input holds is soft primary (`bg-primary/10 text-primary`, `aria-pressed`), the others
  `secondary`. A suggestion still being generated is a disabled `secondary` chip with a spinner and "Generating…".
- The new branch zone is a dashed `border-primary/50` nested panel (`rounded-lg`) with a `GitBranchPlus` icon.
- The drag image is a primary badge "N commits", or "2 branches · 5 commits".
- A nested branch row is indented under the issue's icon, behind a `border-l border-border` rule.
- The issue chip is an `outline` badge with the key as an issue link.

## Testing / Local Run

- `pnpm test:backend`: `move_commits_test.rs`, `reword_commit_test.rs`, `sync_test.rs`.
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
