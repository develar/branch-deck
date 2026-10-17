---
name: Squash Commits
description: Requirements for squashing selected commits of the current branch into the oldest of them, with an editable message and an undo, without touching the working tree.
style: plain-1
targets:
  - ../../crates/git-ops/src/squash_commits.rs
  - ../../crates/git-ops/src/rewrite_plan.rs
  - ../../crates/git-ops/src/replay.rs
  - ../../crates/sync-core/src/squash.rs
  - ../../crates/sync-core/src/repository_lock.rs
  - ../../src-tauri/src/commands/squash_commits.rs
  - ../../src/hooks/useSquashCommits.ts
  - ../../src/hooks/useCommitMoveList.ts
  - ../../src/components/commit/MovableCommitList.tsx
  - ../../src/components/commit/SquashCommitsDialog.tsx
  - ../../src/lib/squash-message.ts
---

# Squash Commits

Status: Implemented
Date: 2026-10-14

## Summary

The user selects two or more commits of one list, a branch or the unassigned commits, and squashes them into one. The
squashed commit takes the place of the oldest selected commit, as `git rebase --autosquash` does with a fixup: the
newer commits are folded into it and leave the history, and the commits in between stay where they are. The message is
edited in a dialog, prefilled with the messages of all the commits. Squashing keeps the files of HEAD, so the working
tree and the index are not touched, and an undo is a move of the branch back.

## Goals

- Combine work-in-progress commits before a push without a terminal rebase.
- Squash commits that are not adjacent, for example a fix made after a commit of another branch.
- Undo a squash.

## Non-goals

- Squashing `fixup!`, `squash!` and `amend!` commits into their targets automatically ([Squash Fixups](fixups.spec.md)).
- Reordering commits or resolving conflicts between the squashed commits and the commits in between.
- Squashing commits of different lists in one step.

## Requirements

### Rewrite

- The commits must be squashed into the oldest selected commit, whatever the order of the selection. It must keep its
  place in the history and its author; the committer becomes the current user.
- The changes of the newer commits must be applied to it oldest first. The commits in between must stay, with their
  messages.
  [@test] ../../crates/sync-core/src/squash_test.rs (`squashes_into_the_oldest_selected_commit_whatever_the_order`)
  [@test] ../../crates/git-ops/src/squash_commits_test.rs (`squashes_adjacent_commits_into_the_oldest`, `keeps_the_commit_in_between_of_another_branch`, `squashes_head_into_an_older_commit`, `squashes_several_groups_in_one_rewrite`)
- The tree of HEAD must stay the same. The working tree, the index and the untracked files must stay byte for byte.
  [@test] ../../crates/git-ops/src/squash_commits_test.rs (`keeps_dirty_and_staged_changes_byte_for_byte`)
- The branch must move in one transaction with an old-value check and the reflog message
  `branch-deck: squash N commits`.
  [@test] ../../crates/git-ops/src/squash_commits_test.rs (`records_the_reflog_message`, `branch_moved_after_prepare_changes_nothing`)
- The squash must be refused, with no ref, index entry or file changed and no temporary ref left, in these cases:
  - fewer than two commits, a commit given twice, or a message without a subject line;
  - a commit that is not on the first-parent line of HEAD;
  - a squashed commit conflicts with a commit in between;
  - the result has no changes, or a later commit would become empty;
  - a rebase, merge, cherry-pick or revert is in progress, HEAD is detached, or the range contains a merge commit;
  - the branch moved meanwhile.
  [@test] ../../crates/git-ops/src/squash_commits_test.rs (`validates_the_groups`, `refuses_a_range_with_merges`)
  [@test] ../../crates/sync-core/src/squash_test.rs (`refuses_fewer_than_two_commits_a_duplicate_or_a_blank_subject`)
  [@test] ../../crates/git-ops/src/rewrite_plan_test.rs (`squash_conflicting_with_commit_in_between_changes_nothing`, `refusal_after_writing_a_commit_leaves_no_temp_refs`, `squash_refuses_empty_result`, `squash_refuses_kept_commit_that_becomes_empty`)
- The squash and its undo must run under the repository lock: while another Branch Deck operation holds it, they wait
  up to 10 s, then they are refused with nothing changed and the error toast says that another operation holds the
  lock. A HEAD the request was not made for (`expect_head`) refuses the squash as `HeadMoved`.
  [@test] ../../crates/sync-core/src/repository_lock_test.rs (`every_rewrite_fails_with_locked_while_another_holds_the_lock`, `a_rewrite_waits_for_the_holder`)
  [@test] ../../crates/sync-core/src/repository_lock_test.rs (`every_rewrite_refuses_another_head_than_the_expected_one`)
- A group that keeps the target's message (for fixups) must keep its bytes and its encoding.
  [@test] ../../crates/git-ops/src/squash_commits_test.rs (`target_message_keeps_the_bytes_and_the_encoding`)

### Starting a squash

- With two or more commits selected, the selection actions must show a ghost "Squash" button.
  [@test] ../../tests/e2e/squash-commits/squash-commits.spec.ts (`should squash selected branch commits into the oldest one and undo it`)
- The context menu of a selected row must offer "Squash N Commits…" with the key `S` when two or more commits are
  selected, and not for a single commit.
  [@test] ../../tests/e2e/squash-commits/squash-commits.spec.ts (`should offer squash in the context menu of two or more commits and cancel with Escape`)
- `S` in a list with two or more selected commits must open the dialog, in a branch and in the unassigned commits.
  [@test] ../../tests/e2e/squash-commits/squash-commits.spec.ts (`should squash unassigned commits with the S key`)

### Dialog

- The header must say "Squash N commits", name the target and list the commits with their short hashes.
- The message must be prefilled with the oldest commit's message, then the message of each newer commit as a paragraph,
  without its autosquash markers and `(prefix)`, or without the issue key it starts with if it is grouped by that key
  (`ABC-1: Add tokens` becomes `Add tokens`). A message already included (e.g. `fixup! X` after `X`) is left out.
  [@test] ../../src/lib/squash-message.test.ts (`keeps the oldest message and appends the others as paragraphs without their prefix`, `removes the autosquash marker and leaves out a message already included`, `cleans up trailing whitespace and blank lines`, `keeps the issue key of a newer subject grouped by its prefix`, `removes the issue key a newer subject is grouped by`)
- The footer must warn when the edited subject moves the squashed commit to another branch.
  [@test] ../../tests/e2e/squash-commits/squash-commits.spec.ts (`should offer squash in the context menu of two or more commits and cancel with Escape`)
- The footer must warn "N squashed commits are already pushed; the next push replaces them" when the branch exists on
  the remote and a selected commit is not ahead of it.
  [@test] ../../src/lib/squash-message.test.ts (`counts the synced commits that are not ahead of the remote branch`, `counts nothing without a remote branch or for commits not synced yet`)
- ⌘↵ (Ctrl+Enter) or the Squash button must squash; Escape must cancel. A click outside must keep the dialog and the
  edited message.

### After a squash

- A squash must clear the selection, sync, and show the toast "Squashed N commits" with an Undo button.
- Undo must move the branch back to the commit before the squash in one transaction (`branch-deck: undo squash`),
  only if the branch still points at the squashed history and both have the same files; otherwise it is refused.
  [@test] ../../crates/git-ops/src/squash_commits_test.rs (`undo_restores_the_previous_head`, `undo_is_refused_when_the_branch_moved`)
  [@test] ../../crates/sync-core/src/squash_test.rs (`undo_restores_the_previous_head_and_refuses_after_a_new_commit`)
  [@test] ../../tests/e2e/squash-commits/squash-commits.spec.ts (`should squash selected branch commits into the oldest one and undo it`)

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
  fail with an error toast.
- Squashing commits that are already pushed makes the next push replace them on the remote branch; the dialog warns.

## References

- [Git Operations](../../docs/design/git-operations.md)
- [Squash Fixups](fixups.spec.md)
- [Move Commits](../branch/move-commits.spec.md)
- [Commit Row](commit-row.spec.md)
