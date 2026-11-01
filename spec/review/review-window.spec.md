---
name: Review Window
description: Requirements for the one review of everything with changes, a branch with its stack, a whole stack, an unapplied or archived branch, a list of commits and a pull request: one target that Rust resolves into its parts and commits, read ahead as the window opens and memoised by the tips it read.
style: plain-1
targets:
  - ../../crates/domain-types/src/review.rs
  - ../../crates/sync-core/src/review_target.rs
  - ../../crates/app-api/src/remote/review.rs
  - ../../src/lib/review-target.ts
  - ../../src/lib/review-window-reads.ts
  - ../../src/lib/branch-review-window.ts
  - ../../src/lib/branch-review.ts
  - ../../src/pages/BranchReviewPage.tsx
  - ../../src/hooks/useBranchReview.ts
  - ../../src/components/review/BranchReview.tsx
  - ../../src/components/branch/KeptBranchCommits.tsx
  - ../../src/components/branch/UnappliedBranchTableCard.tsx
  - ../../src/components/branch/ArchivedBranchTableCard.tsx
  - ../../src/components/branch/ArchivedBranchExpanded.tsx
  - ../../src/hooks/useKeptBranchMenu.ts
  - ../../src/components/commit/CommitList.tsx
  - ../../src/hooks/usePreviewHotkeys.ts
---

# Review Window

Status: Draft (implemented: the targets and their read, the review of the kept branches)
Date: 2026-11-01

## Summary

Everything in Branch Deck that has changes can be reviewed the same way: a virtual branch with its stack, the whole
stack of a root, an unapplied or archived branch, the commits of a list no branch owns, a pull request. "I don't
remember what is in that unapplied branch" has the same answer as "what does this branch land": Review Changes (D)
opens its changes. A review has a target, which Rust resolves into what the window shows: the parts of a range, each
with its base, its head and its commits, or the commits of a list. A pull request is no target: its fetch
([Review Requests](../repository/review-requests.spec.md#review-window)) answers a range of the same shape.

## Goals

- One answer for every object with changes: what it changes, read the same way.
- One read per target, as cheap as the review of a branch: one `for-each-ref` and one `log`, nothing again for what
  did not move.

## Non-goals

- No review of uncommitted changes: the amend card shows them ([Amend Changes](../commit/amend.spec.md)).

## Requirements

### Targets

- A target must be one of: a branch with its stack ancestors (the chain from the root), the whole stack of a root, a
  kept branch by its full name (`{prefix}/unapplied/<name>`, `{prefix}/archived/<date>/<name>`), a list of commit
  ids (`ReviewTarget`); `get_review` answers the parts of a range (`segments`: name, base, head, commits newest
  first) or the commits of a list (`commits`, in its order).
  [@test] ../../crates/sync-core/src/review_target_test.rs (`splits_a_stack_at_the_heads_of_the_parents`, `the_whole_stack_is_one_part_of_its_stack_branch`)
- A kept branch must be one part of its own commits: from its stack base, else from where it left the baseline, as its
  row lists them, whatever the baseline has since; its name without the namespace (and the date of an archived one).
  A name outside the kept namespaces of the prefix is refused; a branch that is gone says so.
  [@test] ../../crates/sync-core/src/review_target_test.rs (`a_kept_branch_is_one_part_of_its_own_commits`)
- A list of commits must keep its order, each commit against its first parent.
  [@test] ../../crates/sync-core/src/review_target_test.rs (`a_list_of_commits_keeps_its_order`)
- A review read again must run the `for-each-ref` of its refs alone while its tips and the baseline stay, and a list
  of commits nothing: a branch is one `for-each-ref` and one `log`, a kept branch adds the read of where its commits
  start (the stack notes, the merge base), a list is one `log --no-walk`.
  [@test] ../../crates/sync-core/src/review_target_test.rs (`a_memoised_review_runs_no_log_for_the_same_tips`, `a_moved_tip_or_baseline_is_read_again`, `a_memoised_kept_branch_is_read_again_only_when_its_tip_moves`)
  [@test] ../../crates/sync-core/src/process_budget_test.rs (`a_review_of_a_branch_a_kept_branch_and_a_list_of_commits`)
- The window must start the read of its target with its page, and its first read take it (`readReview`).
  [@test] ../../src/lib/review-window-reads.test.ts (`gives the first read of the page the read its load started, once, and only for the same review`)

### Kept branches

- "Review Changes" (D), the first item of the menu of an unapplied or archived row (right-click and "⋯"), and D on its
  focused row or the row under the pointer must open the review window of the kept branch
  (`openKeptReviewWindow`, titled "Review docs"): one part of its own commits, the stack bar naming where it is kept and
  the commit they start on ("Unapplied on 1a2b3c4d") instead of the baseline, its diff against that commit, not against
  today's baseline. D on a commit of its expanded row opens the same review at the commit ("Review docs · 1a2b3c4d").
  [@test] ../../tests/e2e/unapplied-branches/unapplied-branches.spec.ts (`should review an unapplied branch from its row, its menu and its commits, on its own base, with its actions`)
  [@test] ../../tests/e2e/archived-branches/archived-branches.spec.ts (`should review an archived branch from its menu, its focused row and its commits`)
- The title bar must hold the actions Rust offers for it (`domain_policy::kept`): of an unapplied branch Reapply and
  its push, its "⋯" the rest of its menu, which the main window runs as the row's menu would; an archived branch has
  only its "⋯" (its delete, the copies). A plain open of the kept branch on screen only brings the window to the front;
  another one starts it over.
  [@test] ../../tests/e2e/unapplied-branches/unapplied-branches.spec.ts (`should review an unapplied branch from its row, its menu and its commits, on its own base, with its actions`)
- After a Reapply the review must go on as the review of the branch it is again, once a sync built it; after a delete
  it keeps what it read, its actions disabled with why ("docs is no longer unapplied: it was reapplied or deleted").
  [@test] ../../tests/e2e/unapplied-branches/unapplied-branches.spec.ts (`should review an unapplied branch from its row, its menu and its commits, on its own base, with its actions`)
- The review plan of a kept branch reads its range; it has no Mark Reviewed and no "Since Your Review".

## References

- [Branch Review](../branch/branch-review.spec.md), [Commit Review](../commit/commit-review.spec.md)
- [Git Operations](../../docs/design/git-operations.md) ("Review of a target")
