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
  - ../../src/lib/review-window.ts
  - ../../src/lib/review-reuse.ts
  - ../../src/lib/branch-review.ts
  - ../../src/lib/window-label.ts
  - ../../src/router.ts
  - ../../src/pages/ReviewPage.tsx
  - ../../src/components/review/ReviewBackButton.tsx
  - ../../src/types/sub-window.ts
  - ../../src/hooks/useSessionFactStores.ts
  - ../../src/hooks/useReviewStatusRefresh.ts
  - ../../src/stores/activity.ts
  - ../../src-tauri/capabilities/sub-windows.json
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

Status: Draft (implemented: the targets and their read, the review of the kept branches, one window for a branch, a
whole stack, a kept branch and a list of commits)
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

### One window

- Every review but that of a pull request must be one window, `review` (`/review`, `ReviewPage`), whose data says
  what it shows (`ReviewWindowData`): a branch with its stack ancestors, the whole stack of a root or a kept branch
  (`kind: "branch"`, its chain with `wholeStack` or `kept`), or a commit of a list (`kind: "commits"`: the commits of
  the list as the opener had them, its name, the commit, the file). A list of commits shows its commits above the
  files (⌥↑/⌥↓) and the plan of each commit; it has no action and no "⋯". The window holds the session of its
  repository whatever it shows. Its size and place are those of the window state plugin under `review`: the sizes
  saved under the labels of the windows it replaced (`branch-review`, `commit-review`) are forgotten once.
  [@test] ../../tests/e2e/review-window/review-window.spec.ts (`should review a branch and then a commit of the unassigned list in the same window, and go back with ⌘[ and with Back`)
  [@test] ../../tests/e2e/commit-files/commit-review.spec.ts (`should review an unassigned commit in the review window of the commit`)
- The window must hold the session of its repository whatever it shows, and fill the stores of its facts from it as
  the main window does (`useSessionFactStores`: the robots, the pull requests, the readiness, the review requests),
  without telling their news: the toasts, the OS notifications and the records of a review requested, a robot that
  merged or an approval are the main window's (`useReviewStatusRefresh`). An error the window tells is recorded in
  Activity under the repository of the window, not the one the main window shows.
  [@test] ../../src/hooks/useSessionFactStores.test.ts (`fill in the review window from the patches of its session and tell no news there: the main window tells them`)
  [@test] ../../src/stores/activity.test.ts (`records the errors of a review window in the repository it was opened for, whatever the main window shows`)
- How an open goes on from the review on screen must be one rule for every subject (`planReviewOpen`): an open of the
  subject on screen only brings the window to the front, where the review is; a commit of it moves there (a commit of
  a branch of the chain, of the kept branch, of the list: `move`); anything else (another subject, a place such as a
  file of a list, a reviewer group or the whole stack of a branch, another repository) starts the window over
  (`startedAt`). The main window applies it to what the window shows as it knows it (`ShownReview`), an open from
  another window (the missing commits of a conflict) starts it over.
  [@test] ../../src/lib/review-reuse.test.ts (`keeps its review for a plain open of the branch it shows, and starts over for another branch or a place`, `moves to a commit of a branch of the chain on screen, and starts over for a commit of another branch`, `keeps a kept branch and a whole stack for a plain open of them, moves to a commit of the kept branch`, `moves to a commit of the list on screen, and starts over for another list, a file or a branch`)
  [@test] ../../tests/e2e/commit-files/commit-review.spec.ts (`should review a commit of a stacked branch with its stack and its actions, and move to a commit of the chain`)
- A start over must keep the review the window left in the history of the window, the latest last, at most 20, at
  the place it was (of a branch its part, its owner chip, its commit and its file; of a list its commit and its
  file). Back restores the latest: ⌘[ (Ctrl+[ on Windows and Linux; not in a dialog of the window) and the Back button
  at the start of the title bar (an arrow, "Back to login", its hover with the key), shown only while the history has
  a review. A restore starts that review over (it reads it again) and tells the main window what the window shows, so
  the main window sends the actions of that branch (`shows` of the link).
  [@test] ../../tests/e2e/review-window/review-window.spec.ts (`should review a branch and then a commit of the unassigned list in the same window, and go back with ⌘[ and with Back`)
  [@test] ../../src/lib/review-reuse.test.ts (`keeps the reviews it left, the latest last, at most the limit`, `starts a review of the history over at the place it was left`, `names the review it goes back to as its title does`)
- The native title of the window names what it shows: "Review login", "Review login · 1a2b3c4d" at a commit, "Review
  the whole air stack", of a list "Review a1b2c3d4 Sort the start-up report", retitled as it steps
  (`reviewWindowTitle`); the opener titles the window the same.
  [@test] ../../src/lib/review-reuse.test.ts (`names the review it goes back to as its title does`)

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
  only its "⋯" (its delete, the copies). A plain open of the kept branch on screen only brings the window to the front,
  a commit of it moves there ([One window](#one-window)).
  [@test] ../../tests/e2e/unapplied-branches/unapplied-branches.spec.ts (`should review an unapplied branch from its row, its menu and its commits, on its own base, with its actions`)
- After a Reapply the review must go on as the review of the branch it is again, once a sync built it; after a delete
  it keeps what it read, its actions disabled with why ("docs is no longer unapplied: it was reapplied or deleted").
  [@test] ../../tests/e2e/unapplied-branches/unapplied-branches.spec.ts (`should review an unapplied branch from its row, its menu and its commits, on its own base, with its actions`)
- The review plan of a kept branch reads its range; it has no Mark Reviewed and no "Since Your Review".

## References

- [Branch Review](../branch/branch-review.spec.md), [Commit Review](../commit/commit-review.spec.md)
- [Git Operations](../../docs/design/git-operations.md) ("Review of a target")
