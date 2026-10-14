---
name: Move Commits
description: Requirements for moving commits between branches by their explicit prefix, renaming and ungrouping a branch, and nesting the branches of an issue.
style: plain-1
targets:
  - ../../crates/sync-core/src/move_commits.rs
  - ../../crates/sync-core/src/regroup.rs
  - ../../crates/sync-core/src/repository_lock.rs
  - ../../crates/sync-core/src/reword_commit.rs
  - ../../crates/sync-core/src/commit_grouper.rs
  - ../../crates/sync-core/src/sync.rs
  - ../../src/hooks/useMoveCommits.ts
  - ../../src/hooks/useCommitMoveList.ts
  - ../../src/hooks/useCommitDrag.ts
  - ../../src/hooks/useBranchContextActions.ts
  - ../../src/components/commit/MovableCommitList.tsx
  - ../../src/components/shared/InlineBranchTarget.tsx
  - ../../src/components/branch/InlineRenameBranchInput.tsx
  - ../../src/components/branch/InlineUngroupConfirm.tsx
  - ../../src/stores/inline-row-action.ts
  - ../../src/components/branch/BranchTableCard.tsx
  - ../../src/components/branch/BranchRow.tsx
  - ../../src/components/branch/IssueFamilyRow.tsx
  - ../../src/stores/commit-move.ts
  - ../../src/stores/branch-sync.ts
---

# Move Commits

Status: Implemented
Date: 2026-10-14

## Summary

A commit belongs to a branch through its subject. An explicit `(name)` prefix wins, then the issue key
(`ABC-123: …`). To push part of an issue on its own, the user moves some of its commits to another branch. The move sets
the explicit prefix of the commits, so `ABC-123: Add lexer` becomes `(lexer) ABC-123: Add lexer`. The issue key stays
in the pushed message. One operation, "move commits to branch X", serves every list, the context menus, the keyboard
and drag and drop. Renaming and ungrouping a branch are moves of all its commits.

## Goals

- Split an issue branch into branches that can be pushed separately, without editing messages one by one.
- Move commits to an existing branch or to a new one with the same gesture.
- Undo a move.
- Keep the branches of one issue together in the branch table.

## Non-goals

- Reordering commits. A move rewrites messages only; the commit order and the trees stay the same.
- Detecting before the move that a commit depends on a commit left behind. The next sync shows the conflict.

## Requirements

### Move operation

- A move must set the explicit prefix of each commit, replacing an existing one, and keep the rest of the subject and
  the body.
- A move without a target must remove the explicit prefix: the commit goes back to its issue branch, or to the
  unassigned commits.
- An autosquash marker must stay in front: `fixup! (a) X` becomes `fixup! (b) X`.
- The fixups of a moved commit must move with it, and be listed after the requested commits for the undo.
  [@test] ../../crates/sync-core/src/move_commits_test.rs (`moves_the_fixups_with_their_target`)
- A move is a regroup plan: it must be validated against every subject ahead of the baseline before anything is
  written, and with stacked branches on, refused if it introduces a stack problem.
  [@test] ../../crates/sync-core/src/regroup_test.rs (`validates_the_plan_before_writing`, `fixups_follow_their_targets`, `refuses_problems_the_plan_introduces_and_keeps_existing_ones`)
- Rewrites must hold the repository lock, so the app and the agent CLI don't rewrite the branch at the same time.
  [@test] ../../crates/sync-core/src/regroup_test.rs (`waits_for_the_repository_lock`)
- A commit already in the target branch must not be rewritten.
- All commits must be rewritten in one history rewrite that keeps the trees, the metadata and the working tree.
- The rewrite must keep the author and the committer of every recreated commit: name, email, date and timezone.
  [@test] ../../crates/sync-core/src/reword_commit_test.rs (`reword_preserves_committer`)
- The target name must be a name the grouper keeps as is: letters, numbers, `-`, `_`, `.` and `/`, not at the ends.
  [@test] ../../crates/sync-core/src/move_commits_test.rs (`sets_replaces_and_removes_the_prefix`, `keeps_the_autosquash_marker`, `validates_the_branch_name`, `moves_unassigned_commits_to_a_new_branch`, `moves_to_another_branch_and_back_to_the_issue`, `skips_commits_already_in_the_target_branch`, `keeps_the_body_and_the_metadata`, `keeps_the_checked_out_branch`, `refuses_an_invalid_target_or_an_unknown_commit`, `moves_many_commits_in_one_rewrite`)
- A split prefix branch must keep the issue key of its commits.
  [@test] ../../crates/sync-core/src/move_commits_test.rs (`splits_commits_out_of_an_issue_branch`)

### Selection and the "Move to…" form

- The commits of a branch table row and of the unassigned commits card must be selectable.
- Only one list must hold a selection: selecting in another list must clear it.
  [@test] ../../tests/e2e/commit-message/commit-message.spec.ts (`should select commit rows of a branch, one list at a time`)
- A selection must show "N selected" and a "Move to…" button, above the branch commits or in the unassigned card header.
- A selection with an explicit prefix must also offer "Move Back to ABC-123", or "Move to Unassigned".
- "Move to…", Enter or M must open the form. One name input decides: an existing branch name moves the commits there,
  a new name creates the branch. The help text and the submit button ("Move" or "Create") must say which.
- The form must offer the existing branches as chips, the branches of the same issue first, then the AI suggestions.
  [@test] ../../tests/e2e/move-commits/move-commits.spec.ts (`should split commits out of an issue branch into a new branch nested under the issue`)
- With [stacked branches](stacked-branches.spec.md) on, the input may name a parent (`auth/login`), the target is
  looked up by its leaf, the help text names the parent ("Moves 2 commits to login (on auth)", "Creates login on
  auth") and warns when the parent differs from the existing branch's, and the branches of the same stack come first.
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should move commits to a new branch on a parent with Move to…`)
  [@test] ../../src/stores/branch-table-rows.test.ts (`offers the branches of the same stack first as move targets`)

### Context menus

- The commit menu must offer "Move to…" with the key `M` (the label of the selection button, for one commit or many),
  a "Move to Existing" submenu with the branches and "New Branch…", and "Move Back to ABC-123", "Move to Unassigned" or
  "Remove Prefix" for commits with an explicit prefix.
  [@test] ../../tests/e2e/move-commits/move-commits.spec.ts (`should move a commit to an existing branch from the context menu and undo it`, `should move a split commit back to its issue and a prefix commit to the unassigned commits`)
  [@test] ../../tests/e2e/unassigned-commits/unassigned-commits.spec.ts (`should act on all selected commits from the context menu of a selected row`, `should act on that commit only from the context menu of a row that is not selected`)
- The branch menu must offer "Rename…" for a prefix branch and "Name This Branch…" for an issue branch. Both move all
  commits of the branch to the new name; an existing name merges the branches.
- The branch menu of a prefix branch must offer "Ungroup", which removes the prefix of all its commits.
  [@test] ../../tests/e2e/move-commits/move-commits.spec.ts (`should rename a branch and name an issue branch`)
- While a rename or an ungroup runs, its form must show the progress row ("Renaming docs to documentation…"); it closes
  when the commits moved. A failure must keep the form with the typed name (the error is a toast).
- With [stacked branches](stacked-branches.spec.md) on, Rename must rename the branch in the prefixes of the branches
  stacked on it too (`rename_branch`), and the new name must be a branch name without `/`. Ungroup and Unapply of a
  branch that others stack on must confirm and name them: they are blocked until it has commits again. The
  confirmation focuses its button, so Enter confirms.
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should rename a parent with its children following`, `should confirm ungrouping a parent and list its children`)

### Drag and drop

- A commit row must be draggable. Dragging a selected row must drag the selection.
- A branch row of another branch must accept the drop and move the commits there.
- The source branch must not accept the drop.
- The unassigned commits card must accept commits with an explicit prefix and remove it.
  [@test] ../../tests/e2e/move-commits/move-commits.spec.ts (`should drag the selection onto a branch, but not onto its own branch`)
- While commits are dragged, a "new branch" zone must appear above the branch table. A drop there must open the
  "Move to…" form for the dragged commits.
  [@test] ../../tests/e2e/move-commits/move-commits.spec.ts (`should drag an unassigned commit onto the new branch zone`)

### After a move

- A move must show a toast "Moved N commits to X" with an Undo button. Undo must restore the previous messages in one
  history rewrite.
  [@test] ../../tests/e2e/move-commits/move-commits.spec.ts (`should move a commit to an existing branch from the context menu and undo it`)
  [@test] ../../crates/sync-core/src/reword_commit_test.rs (`rewords_several_commits_in_one_pass`)
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
- "Move to…" is an outline `xs` button in the primary colour. The remove-prefix action is a ghost `xs` button.
- A droppable branch row has a dashed `outline-primary/30`. The row under the pointer gets `bg-primary/10` and a solid
  primary outline. The unassigned card gets a primary ring.
- The new branch zone is a dashed `border-primary/50` nested panel (`rounded-lg`) with a `GitBranchPlus` icon.
- The drag image is a primary badge "N commits".
- A nested branch row is indented under the issue's icon, behind a `border-l border-border` rule.
- The issue chip is an `outline` badge with the key as an issue link.

## Testing / Local Run

- `pnpm test:backend`: `move_commits_test.rs`, `reword_commit_test.rs`, `sync_test.rs`.
- `pnpm test:unit`: the grouping rule (`src/lib/commit.test.ts`) and the issue families (`src/stores/branch-sync.test.ts`).
- `pnpm e2e`: `tests/e2e/move-commits` uses the `issue_split` template.

## Open Questions / Risks

- A split can create a conflict when a moved commit depends on a commit left behind. The sync shows it, and Undo
  restores the previous grouping.
- The undo uses the new commit ids. A history rewrite in a terminal between the move and the undo makes the undo fail
  with an error toast.

## References

- [Virtual Branches](../../docs/design/virtual-branches.md)
- [Commit Row](../commit/commit-row.spec.md)
