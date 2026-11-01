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
  - ../../src/pages/PullRequestReviewWindow.tsx
  - ../../src/components/review/ReviewBackButton.tsx
  - ../../src/types/sub-window.ts
  - ../../src/hooks/useSessionFactStores.ts
  - ../../src/hooks/useReviewStatusRefresh.ts
  - ../../src/stores/activity.ts
  - ../../src/hooks/useReviewActions.ts
  - ../../src/lib/review-actions.ts
  - ../../src/lib/review-runs.ts
  - ../../src/hooks/useReviewRuns.ts
  - ../../src/stores/repository.ts
  - ../../crates/domain-types/src/actions.rs
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
  - ../../src/components/branch/KeptBranchPreviewDialog.tsx
  - ../../src/components/preview/CommitsPreviewBody.tsx
---

# Review Window

Status: Implemented
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

- Every review must be one window, `review` (`/review`, `ReviewPage`), whose data says what it shows
  (`ReviewWindowData`): a branch with its stack ancestors, the whole stack of a root or a kept branch (`kind:
  "branch"`, its chain with `wholeStack` or `kept`), a commit of a list (`kind: "commits"`: the commits of the list as
  the opener had them, its name, the commit, the file), or a pull request waiting for the user's review (`kind:
  "pullRequest"`: the request, its place in Your Turn or Reviews, the write the row asked for;
  [Review Requests](../repository/review-requests.spec.md#review-window)), whose review is a chunk of its own the page
  loads only for it, so the first frame of the others keeps its size. A list of commits shows its commits above the
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
  file of a list, a reviewer group, the whole stack of a branch, another pull request or a write of one, another
  repository) starts the window over
  (`startedAt`). The page of the window applies it to every open it gets, from any window (`takeOpen`): an opener only
  says what to show. A start over while the "Create pull request" dialog of the window holds what the user typed is
  refused, an `info` toast in the window ("Cannot review auth"); a move waits until the dialog closes.
  [@test] ../../src/lib/review-reuse.test.ts (`keeps its review for a plain open of the request it shows, and starts over for another request or an intent`, `keeps its review for a plain open of the branch it shows, and starts over for another branch or a place`, `moves to a commit of a branch of the chain on screen, and starts over for a commit of another branch`, `keeps a kept branch and a whole stack for a plain open of them, moves to a commit of the kept branch`, `moves to a commit of the list on screen, and starts over for another list, a file or a branch`, `keeps the review on screen for a plain open of it, moves to a commit of it, and starts over for anything else`)
  [@test] ../../tests/e2e/branch-review/branch-review.spec.ts (`should keep the Create pull request dialog of the window while a commit is reviewed, and move to the commit once it closes`)
  [@test] ../../tests/e2e/commit-files/commit-review.spec.ts (`should review a commit of a stacked branch with its stack and its actions, and move to a commit of the chain`)
- A start over must keep the review the window left in the history of the window, the latest last, at most 20, at
  the place it was (of a branch its part, its owner chip, its commit and its file; of a list its commit and its
  file; of a pull request the request ↑/↓ walked to, which it fetches again). Back restores the latest: ⌘[ (Ctrl+[ on Windows and Linux; not in a dialog of the window) and the Back button
  at the start of the title bar (an arrow, "Back to login", its hover with the key), shown only while the history has
  a review. A restore starts that review over: it reads it again, and its actions are those of the window's session.
  [@test] ../../tests/e2e/review-window/review-window.spec.ts (`should review a branch and then a commit of the unassigned list in the same window, and go back with ⌘[ and with Back`)
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should go back to the pull request with ⌘[ after the review of a branch in the same window, and fetch it again`)
  [@test] ../../src/lib/review-reuse.test.ts (`keeps the reviews it left, the latest last, at most the limit`, `starts a review of the history over at the place it was left`, `names the review it goes back to as its title does`)
- The native title of the window names what it shows: "Review login", "Review login · 1a2b3c4d" at a commit, "Review
  the whole air stack", of a list "Review a1b2c3d4 Sort the start-up report", retitled as it steps
  (`reviewWindowTitle`); the opener titles the window the same.
  [@test] ../../src/lib/review-reuse.test.ts (`names the review it goes back to as its title does`)

### Actions

- The window must build the actions of what it shows from its own session, by the builders of the rows
  (`useReviewActions`: the title bar and "⋯" of a branch, of a whole stack, of a kept branch, the menu of each commit),
  in the repository of the review whatever the main window shows (`getSelectedProject` of the review window), and run
  each itself (an intent, a dialog of the window, a copy, the next review), but one that opens a form under the row of
  its subject: those Rust names (`ActionId::opens_row_form`, `ROW_FORM_ACTIONS` of the bindings: a rename, a move, a
  stack, an ungroup, the amend, the issue reference, the squash, unapply, park, the typed delete of a kept branch, the
  popover of a Safe Push and of its rarer starts, Request Review and Start in Air, a push, which asks at the row when
  it would drop remote commits) and the fix of a conflict, the forms and the rewrites of a commit.
  [@test] ../../tests/e2e/branch-review/branch-review.spec.ts (`should build the actions of the title bar of each part from the window's own session, and open the pull request itself`, `should show the Unstack the row runs as running in the window's menu, from the window's own session, and run it once`, `should create the pull request in the window, with the context of its own session`)
  [@test] ../../tests/e2e/unapplied-branches/unapplied-branches.spec.ts (`should review an unapplied branch from its row, its menu and its commits, on its own base, with its actions`)
  [@test] ../../crates/domain-policy/src/action_catalog_test.rs (`every_action_and_every_blocker_code_is_in_the_action_catalog_the_page_reads`)
- A form under a row must be one request to the main window, `review/run` (`{ id, repositoryPath, subject, itemId,
  commitIds? }`: the subject a branch, `stack:<root>`, `unapplied:<full>`, `archived:<full>`; the commits of a commit's
  item), answered by `review/reply` and, refused, `review/feedback`: the main window switches to the repository of
  the request (`showRepository`), finds the item by the same builders (`findReviewItem`), comes to the front, reveals
  the row and runs it; one it can't run (the repository is gone from its list, the subject left, the item is disabled)
  is refused with the reason, shown in the window and recorded in Activity. While the main window shows another
  repository, the hover of such an item says "Switches the main window to app".
  [@test] ../../tests/e2e/branch-review/branch-review.spec.ts (`should run the Safe Push and the push of the title bar at the rows of the main window, switched to the repository of the review first`, `should run an item of the branch's menu from the window's ⋯, in the main window`, `should run the menu of a commit of the review in the main window, its forms at the row, its copies in the window`)
  [@test] ../../tests/e2e/archived-branches/archived-branches.spec.ts (`should review an archived branch from its menu, its focused row and its commits`)
  [@test] ../../tests/e2e/outgoing/outgoing.spec.ts (`should push a branch a review window asks for during a sync once that sync ended, in the main window`)

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
  its push, its "⋯" the rest of its menu, run where they run ([Actions](#actions): Reapply in the window, the push and
  the typed delete at the row); an archived branch has only its "⋯" (its delete, the copies). A plain open of the kept branch on screen only brings the window to the front,
  a commit of it moves there ([One window](#one-window)).
  [@test] ../../tests/e2e/unapplied-branches/unapplied-branches.spec.ts (`should review an unapplied branch from its row, its menu and its commits, on its own base, with its actions`)
- Space on an unapplied or archived row (focused, or under the pointer) must open its preview
  (`KeptBranchPreviewDialog`): its name and where it is kept with the count of its commits, then its own commits as
  its review reads them, newest first, with their whole messages, as the preview of a branch reads its commits
  (`CommitsPreviewBody`); ↑/↓ show the previous or next row of the card, D and the footer's `D Review Changes` review
  it (a commit's button at that commit), the preview closing once the window opened; Space and Escape close it.
  [@test] ../../tests/e2e/unapplied-branches/unapplied-branches.spec.ts (`should preview an unapplied branch with Space, its own commits with their messages, and review it from the preview`)
- After a Reapply the review must go on as the review of the branch it is again, once a sync built it (the window
  opens it itself, the kept branch into its history); after a delete
  it keeps what it read, its actions disabled with why ("docs is no longer unapplied: it was reapplied or deleted").
  [@test] ../../tests/e2e/unapplied-branches/unapplied-branches.spec.ts (`should review an unapplied branch from its row, its menu and its commits, on its own base, with its actions`)
- The review plan of a kept branch reads its range; it has no Mark Reviewed and no "Since Your Review".

## References

- [Branch Review](../branch/branch-review.spec.md), [Commit Review](../commit/commit-review.spec.md)
- [Git Operations](../../docs/design/git-operations.md) ("Review of a target")
