---
name: Review Window
description: Requirements for the one review of everything with changes, a branch with its stack, a whole stack, an unapplied or archived branch, a list of commits and a pull request: one target that Rust resolves into its parts and commits, read ahead as the window opens and memoised by the tips it read.
style: plain-1
targets:
  - crates/domain-types/src/review.rs
  - crates/sync-core/src/review_target.rs
  - crates/app-api/src/remote/review.rs
  - src/lib/review-target.ts
  - src/lib/review-window-reads.ts
  - src/lib/review-window.ts
  - src/lib/review-reuse.ts
  - src/lib/branch-review.ts
  - src/lib/window-label.ts
  - src/router.ts
  - src/pages/ReviewPage.tsx
  - src/pages/PullRequestReviewWindow.tsx
  - src/components/review/ReviewBackButton.tsx
  - src/types/sub-window.ts
  - src/hooks/useSessionFactStores.ts
  - src/hooks/useReviewStatusRefresh.ts
  - src/stores/activity.ts
  - src/hooks/useReviewActions.ts
  - src/lib/review-actions.ts
  - src/lib/review-runs.ts
  - src/hooks/useReviewRuns.ts
  - src/stores/repository.ts
  - crates/domain-types/src/actions.rs
  - src-tauri/capabilities/sub-windows.json
  - src/hooks/useBranchReview.ts
  - src/components/review/BranchReview.tsx
  - src/components/branch/KeptBranchCommits.tsx
  - src/components/branch/UnappliedBranchTableCard.tsx
  - src/components/branch/ArchivedBranchTableCard.tsx
  - src/components/branch/ArchivedBranchExpanded.tsx
  - src/hooks/useKeptBranchMenu.ts
  - src/components/commit/CommitList.tsx
  - src/hooks/usePreviewHotkeys.ts
  - src/components/branch/KeptBranchPreviewDialog.tsx
  - src/components/preview/CommitsPreviewBody.tsx
---

# Review Window

Status: Implemented
Date: 2026-11-01

## Purpose

Everything in Branch Deck that has changes can be reviewed the same way: a virtual branch with its stack, the whole
stack of a root, an unapplied or archived branch, the commits of a list no branch owns, a pull request. "I don't
remember what is in that unapplied branch" has the same answer as "what does this branch land": Review Changes (D)
opens its changes. A review has a target, which Rust resolves into what the window shows: the parts of a range, each
with its base, its head and its commits, or the commits of a list. A pull request is no target: its fetch
([Review Requests](../../repository/review-requests/spec.md#review-window)) answers a range of the same shape.

## Goals

- One answer for every object with changes: what it changes, read the same way.
- One read per target, as cheap as the review of a branch: one `for-each-ref` and one `log`, nothing again for what
  did not move.

## Non-goals

- No review of uncommitted changes: the amend card shows them ([Amend Changes](../../commit/amend/spec.md)).

## Requirements

### Requirement: One window: Every review is one window whose data says what it shows

Every review MUST be one window, `review` (`/review`, `ReviewPage`), whose data says what it shows
(`ReviewWindowData`): a branch with its stack ancestors, the whole stack of a root or a kept branch, a commit of a
list, or a pull request waiting for the user's review.

#### Scenario: A branch, the whole stack of a root or a kept branch
- **WHEN** the window shows a branch with its stack ancestors, the whole stack of a root or a kept branch
- **THEN** its data is `kind: "branch"`, its chain with `wholeStack` or `kept`

#### Scenario: A commit of a list
- **WHEN** the window shows a commit of a list
- **THEN** its data is `kind: "commits"`: the commits of the list as the opener had them, its name, the commit, the file
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should review an unassigned commit in the review window of the commit`)

#### Scenario: A branch and then a commit of a list in the same window
- **GIVEN** the window shows a branch
- **WHEN** a commit of the unassigned list is reviewed
- **THEN** the same window shows the commit of the list
  [@test] tests/e2e/review-window/review-window.spec.ts (`should review a branch and then a commit of the unassigned list in the same window, and go back with ⌘[ and with Back`)

#### Scenario: A pull request waiting for the user's review
- **WHEN** the window shows a pull request waiting for the user's review
- **THEN** its data is `kind: "pullRequest"`: the request, its place in Your Turn or Reviews, the write the row asked
  for ([Review Requests](../../repository/review-requests/spec.md#review-window))

### Requirement: One window: The review of a pull request is a chunk of its own

The review of a pull request MUST be a chunk of its own that the page loads only for it, so the first frame of the
others keeps its size.

#### Scenario: The window shows no pull request
- **WHEN** the window shows a branch or a list of commits
- **THEN** the page does not load the chunk of the review of a pull request, and its first frame keeps its size

### Requirement: One window: A list of commits shows its commits above the files

A list of commits SHALL show its commits above the files (⌥↑/⌥↓) and the plan of each commit; it MUST have no action
and no "⋯".

#### Scenario: The review of a list of commits
- **WHEN** the window shows a commit of a list
- **THEN** its commits stand above the files, ⌥↑/⌥↓ step through them, and each commit shows its plan
- **AND** it has no action and no "⋯"

### Requirement: One window: The size and place of the window are kept under its label

Its size and place MUST be those of the window state plugin under `review`: the sizes saved under the labels of the
windows it replaced (`branch-review`, `commit-review`) are forgotten once.

#### Scenario: Sizes saved under the old labels
- **GIVEN** sizes saved under `branch-review` and `commit-review`
- **WHEN** the window opens
- **THEN** its size and place are those saved under `review`, and the old sizes are forgotten once

### Requirement: One window: The window fills its fact stores from its own session

The window MUST hold the session of its repository whatever it shows, and fill the stores of its facts from it as the
main window does (`useSessionFactStores`: the robots, the pull requests, the readiness, the review requests), without
telling their news.

#### Scenario: News of the session in the review window
- **WHEN** the patches of the window's session bring a review requested, a robot that merged or an approval
- **THEN** the window fills its stores from them
- **AND** the toasts, the OS notifications and the records of them are the main window's (`useReviewStatusRefresh`)
  [@test] src/hooks/useSessionFactStores.test.ts (`fill in the review window from the patches of its session and tell no news there: the main window tells them`)

### Requirement: One window: An error of the window is recorded under its repository

An error the window tells MUST be recorded in Activity under the repository of the window, not the one the main
window shows.

#### Scenario: The main window shows another repository
- **GIVEN** the main window shows another repository than the review window
- **WHEN** the review window tells an error
- **THEN** Activity records it under the repository of the review window
  [@test] src/stores/activity.test.ts (`records the errors of a review window in the repository it was opened for, whatever the main window shows`)

### Requirement: One window: One rule says how an open goes on from the review on screen

How an open goes on from the review on screen MUST be one rule for every subject (`planReviewOpen`): an open of the
subject on screen only brings the window to the front, where the review is; a commit of it moves there (`move`);
anything else starts the window over (`startedAt`). The page of the window MUST apply it to every open it gets, from
any window (`takeOpen`): an opener only says what to show.

#### Scenario: A pull request
- **WHEN** the window shows a pull request and gets an open
- **THEN** a plain open of the request it shows keeps its review
- **AND** another request or an intent starts it over
  [@test] src/lib/review-reuse.test.ts (`keeps its review for a plain open of the request it shows, and starts over for another request or an intent`)

#### Scenario: A branch
- **WHEN** the window shows a branch and gets an open
- **THEN** a plain open of the branch it shows keeps its review
- **AND** another branch or a place starts it over
  [@test] src/lib/review-reuse.test.ts (`keeps its review for a plain open of the branch it shows, and starts over for another branch or a place`)

#### Scenario: A commit of a branch of the chain
- **WHEN** the window shows a chain and gets an open of a commit
- **THEN** a commit of a branch of the chain on screen moves there (`move`)
- **AND** a commit of another branch starts it over
  [@test] src/lib/review-reuse.test.ts (`moves to a commit of a branch of the chain on screen, and starts over for a commit of another branch`)
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should review a commit of a stacked branch with its stack and its actions, and move to a commit of the chain`)

#### Scenario: A kept branch and a whole stack
- **WHEN** the window shows a kept branch or a whole stack and gets an open
- **THEN** a plain open of it keeps it
- **AND** a commit of the kept branch moves there
  [@test] src/lib/review-reuse.test.ts (`keeps a kept branch and a whole stack for a plain open of them, moves to a commit of the kept branch`)

#### Scenario: A list of commits
- **WHEN** the window shows a list of commits and gets an open
- **THEN** a commit of the list on screen moves there
- **AND** another list, a file or a branch starts it over
  [@test] src/lib/review-reuse.test.ts (`moves to a commit of the list on screen, and starts over for another list, a file or a branch`)

#### Scenario: Anything else starts the window over
- **WHEN** the open is another subject, a place such as a file of a list, a reviewer group, the whole stack of a
  branch, another pull request or a write of one, or another repository
- **THEN** the window starts over (`startedAt`)
  [@test] src/lib/review-reuse.test.ts (`keeps the review on screen for a plain open of it, moves to a commit of it, and starts over for anything else`)

### Requirement: One window: A start over waits for the Create pull request dialog

A start over while the "Create pull request" dialog of the window holds what the user typed MUST be refused, an
`info` toast in the window ("Cannot review auth"); a move MUST wait until the dialog closes.

#### Scenario: The dialog holds what the user typed
- **GIVEN** the "Create pull request" dialog of the window holds what the user typed
- **WHEN** an open would start the window over
- **THEN** it is refused, an `info` toast in the window ("Cannot review auth")
- **AND** a move to a commit waits until the dialog closes
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should keep the Create pull request dialog of the window while a commit is reviewed, and move to the commit once it closes`)

### Requirement: One window: A start over keeps the review it left in the history

A start over MUST keep the review the window left in the history of the window, the latest last, at most 20, at the
place it was (of a branch its part, its owner chip, its commit and its file; of a list its commit and its file; of a
pull request the request ↑/↓ walked to, which it fetches again).

#### Scenario: The history keeps the latest last, at most 20
- **WHEN** the window starts over more often than the limit
- **THEN** its history holds the reviews it left, the latest last, at most 20
  [@test] src/lib/review-reuse.test.ts (`keeps the reviews it left, the latest last, at most the limit`)

#### Scenario: A review of the history starts at the place it was left
- **WHEN** a review of the history starts over
- **THEN** it starts at the place it was: of a branch its part, its owner chip, its commit and its file; of a list its
  commit and its file; of a pull request the request ↑/↓ walked to
  [@test] src/lib/review-reuse.test.ts (`starts a review of the history over at the place it was left`)

### Requirement: One window: Back restores the latest review of the history

Back MUST restore the latest: ⌘[ (Ctrl+[ on Windows and Linux; not in a dialog of the window) and the Back button at
the start of the title bar (an arrow, "Back to login", its hover with the key), shown only while the history has a
review. A restore MUST start that review over: it reads it again, and its actions are those of the window's session.

#### Scenario: ⌘[ and the Back button
- **GIVEN** the window started over from a branch to a commit of a list
- **WHEN** the user presses ⌘[ (Ctrl+[ on Windows and Linux), or clicks the Back button ("Back to login")
- **THEN** the branch is read again, with the actions of the window's session
- **AND** the Back button is shown only while the history has a review
  [@test] tests/e2e/review-window/review-window.spec.ts (`should review a branch and then a commit of the unassigned list in the same window, and go back with ⌘[ and with Back`)

#### Scenario: Back to a pull request
- **GIVEN** the window started over from a pull request to the review of a branch
- **WHEN** the user presses ⌘[
- **THEN** the window shows the pull request again and fetches it again
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should go back to the pull request with ⌘[ after the review of a branch in the same window, and fetch it again`)

#### Scenario: The Back button names the review it goes back to
- **WHEN** the history has a review
- **THEN** the Back button names it as its title does
  [@test] src/lib/review-reuse.test.ts (`names the review it goes back to as its title does`)

### Requirement: One window: The native title names what the window shows

The native title of the window MUST name what it shows, retitled as it steps (`reviewWindowTitle`); the opener SHALL
title the window the same.

#### Scenario: The titles of a branch, a commit, a whole stack and a list
- **WHEN** the window shows a branch, a commit of it, a whole stack or a commit of a list
- **THEN** it is titled "Review login", "Review login · 1a2b3c4d" at a commit, "Review the whole air stack", of a list
  "Review a1b2c3d4 Sort the start-up report"
- **AND** it is retitled as it steps
  [@test] src/lib/review-reuse.test.ts (`names the review it goes back to as its title does`)

### Requirement: Actions: The window builds the actions of what it shows from its own session

The window MUST build the actions of what it shows from its own session, by the builders of the rows
(`useReviewActions`: the title bar and "⋯" of a branch, of a whole stack, of a kept branch, the menu of each commit),
in the repository of the review whatever the main window shows (`getSelectedProject` of the review window).

#### Scenario: The title bar of each part of a branch
- **WHEN** the window reviews a branch of a stack
- **THEN** the title bar and "⋯" of each part are the actions its builders give from the window's own session
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should build the actions of the title bar of each part from the window's own session, and open the pull request itself`)

#### Scenario: An action the row runs is running in the window's menu
- **WHEN** the row of the main window runs an action of the branch the window shows
- **THEN** the window's menu, from the window's own session, shows it as running, and it runs once
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should show the Unstack the row runs as running in the window's menu, from the window's own session, and run it once`)

#### Scenario: The actions of a kept branch
- **WHEN** the window reviews an unapplied branch
- **THEN** its title bar and "⋯" hold the actions of the kept branch from the window's own session
  [@test] tests/e2e/unapplied-branches/unapplied-branches.spec.ts (`should review an unapplied branch on its own base with its actions, its Reapply going on as the review of the branch it is again`)

### Requirement: Actions: The window runs each action itself but a form under a row

The window MUST run each action itself (an intent, a dialog of the window, a copy, the next review), but one that
opens a form under the row of its subject: those Rust names (`ActionId::opens_row_form`, `ROW_FORM_ACTIONS` of the
bindings), the fix of a conflict, the forms and the rewrites of a commit.

#### Scenario: A dialog of the window and an open of the pull request
- **WHEN** the user creates or opens the pull request from the window
- **THEN** the window runs it itself, with the context of its own session, and asks nothing of the main window
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should create the pull request in the window, with the context of its own session`, `should build the actions of the title bar of each part from the window's own session, and open the pull request itself`)

#### Scenario: The actions Rust names as forms under a row
- **WHEN** an action opens a form under the row of its subject
- **THEN** the catalog the page reads (`ROW_FORM_ACTIONS` of the bindings) names it as Rust does
  (`ActionId::opens_row_form`): a rename, a move, a stack, an ungroup, the amend, the issue reference, the squash,
  unapply, park, a declared need, the typed delete of a kept branch, the popover of a Safe Push and of its rarer starts,
  Request Review and Start in Air, a push (also with its children), which asks at the row when it would drop remote
  commits, the pick of remote commits, the push into the target and the merge of the pull request of a landing, the
  close of the pull request of a stack
- **AND** the window does not run it itself
  [@test] crates/domain-policy/src/action_catalog_test.rs (`every_action_and_every_blocker_code_is_in_the_action_catalog_the_page_reads`)

### Requirement: Actions: A form under a row is one request to the main window

A form under a row MUST be one request to the main window, `review/run` (`{ id, repositoryPath, subject, itemId,
commitIds?, request? }`), answered by `review/reply` and, refused, `review/feedback`.

#### Scenario: An item of a branch
- **WHEN** the window asks the main window to run the push or the Safe Push of a branch
- **THEN** it sends one `review/run` per item, with the repository of the review, the branch as its subject and the
  id of the item
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should run the Safe Push and the push of the title bar at the rows of the main window, switched to the repository of the review first`)

#### Scenario: The subjects and the commits of a request
- **WHEN** the window sends `review/run`
- **THEN** its subject is a branch, `stack:<root>`, `unapplied:<full>`, `archived:<full>`, or a pull request
  (`pull:<number>`, with the whole `request`;
  [Review Requests](../../repository/review-requests/spec.md#review-window))
- **AND** the item of a commit carries the commits of the item, of a pull request the reviewed commit

### Requirement: Actions: The main window runs the request at the row of its repository

For a request the main window MUST switch to the repository of the request (`showRepository`), find the item by the
same builders (`findReviewItem`), come to the front, reveal the row and run it.

#### Scenario: The Safe Push and the push of the title bar
- **GIVEN** the main window shows another repository
- **WHEN** the user runs the Safe Push or the push of the title bar of the window
- **THEN** the main window switches to the repository of the review first, comes to the front and runs it at the row
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should run the Safe Push and the push of the title bar at the rows of the main window, switched to the repository of the review first`)

#### Scenario: An item of the branch's menu from the window's "⋯"
- **WHEN** the user chooses an item of the branch's menu that opens a form from the window's "⋯"
- **THEN** the main window runs it at the row
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should run an item of the branch's menu from the window's ⋯, in the main window`)

#### Scenario: The menu of a commit of the review
- **WHEN** the user chooses an item of the menu of a commit of the review
- **THEN** its forms run at the row of the main window, its copies in the window
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should run the menu of a commit of the review in the main window, its forms at the row, its copies in the window`)

#### Scenario: The typed delete of an archived branch
- **WHEN** the user chooses the delete of an archived branch in the window's "⋯"
- **THEN** its form opens at the row of the main window
  [@test] tests/e2e/archived-branches/archived-branches.spec.ts (`should review an archived branch from its menu, its focused row and its commits`)

#### Scenario: A request during a sync
- **WHEN** the window asks for a push while a sync runs in the main window
- **THEN** the main window pushes the branch once that sync ended
  [@test] tests/e2e/outgoing/outgoing.spec.ts (`should push a branch a review window asks for during a sync once that sync ended, in the main window`)

### Requirement: Actions: A request the main window can't run is refused with the reason

A request the main window can't run (the repository is gone from its list, the subject left, the item is disabled)
MUST be refused with the reason, shown in the window and recorded in Activity.

#### Scenario: The repository is gone from the list of the main window
- **WHEN** a request names a repository the main window can't switch to
- **THEN** it is refused (`review/reply` not ok) with the reason, shown in the window
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should run the Safe Push and the push of the title bar at the rows of the main window, switched to the repository of the review first`)

#### Scenario: The subject left or the item is disabled
- **WHEN** the subject of the request left or its item is disabled
- **THEN** it is refused with the reason, shown in the window and recorded in Activity

### Requirement: Actions: The hover of a row item says the main window switches

While the main window shows another repository, the hover of an item that opens a form under a row MUST say
"Switches the main window to app".

#### Scenario: The main window shows another repository
- **GIVEN** the main window shows another repository than the review
- **WHEN** the user hovers an item the main window runs at a row
- **THEN** its hover says "Switches the main window to app"
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should run the Safe Push and the push of the title bar at the rows of the main window, switched to the repository of the review first`)

### Requirement: Targets: A target is a chain, a whole stack, a kept branch or a list

A target MUST be one of: a branch with its stack ancestors (the chain from the root), the whole stack of a root, a
kept branch by its full name (`{prefix}/unapplied/<name>`, `{prefix}/archived/<date>/<name>`), a list of commit ids
(`ReviewTarget`); `get_review` MUST answer the parts of a range (`segments`: name, base, head, commits newest first)
or the commits of a list (`commits`, in its order).

#### Scenario: A branch with its stack ancestors
- **WHEN** the target is a branch of a stack
- **THEN** `get_review` answers the chain from the root, split at the heads of the parents, each part its name, base,
  head and commits newest first
  [@test] crates/sync-core/src/review_target_test.rs (`splits_a_stack_at_the_heads_of_the_parents`)

#### Scenario: The whole stack of a root
- **WHEN** the target is the whole stack of a root
- **THEN** `get_review` answers one part, that of its stack branch
  [@test] crates/sync-core/src/review_target_test.rs (`the_whole_stack_is_one_part_of_its_stack_branch`)

#### Scenario: A list of commit ids
- **WHEN** the target is a list of commit ids
- **THEN** `get_review` answers the commits of the list (`commits`, in its order) and no part

### Requirement: Targets: A kept branch is one part of its own commits

A kept branch MUST be one part of its own commits: from its stack base, else from where it left the baseline, as its
row lists them, whatever the baseline has since; its name without the namespace (and the date of an archived one).

#### Scenario: The baseline moved on since the branch left it
- **GIVEN** an unapplied and an archived branch, and the baseline moved on since they left it
- **WHEN** each is reviewed
- **THEN** it is one part of its own commits on its own base
- **AND** the part is named without the namespace, and without the date of the archived one
  [@test] crates/sync-core/src/review_target_test.rs (`a_kept_branch_is_one_part_of_its_own_commits`)

### Requirement: Targets: A name outside the kept namespaces is refused

A name outside the kept namespaces of the prefix MUST be refused; a branch that is gone MUST say so.

#### Scenario: A virtual branch, another prefix, an archived name without its date
- **WHEN** the kept branch named is outside the kept namespaces of the prefix
- **THEN** the review is refused: it is no unapplied or archived branch of the prefix
  [@test] crates/sync-core/src/review_target_test.rs (`a_kept_branch_is_one_part_of_its_own_commits`)

#### Scenario: A kept branch that is gone
- **WHEN** the kept branch named no longer exists
- **THEN** the review says so
  [@test] crates/sync-core/src/review_target_test.rs (`a_kept_branch_is_one_part_of_its_own_commits`)

### Requirement: Targets: A list of commits keeps its order

A list of commits MUST keep its order, each commit against its first parent.

#### Scenario: A list of two commits
- **WHEN** a list of commits is reviewed
- **THEN** its commits come in the order of the list, each against its first parent
  [@test] crates/sync-core/src/review_target_test.rs (`a_list_of_commits_keeps_its_order`)

### Requirement: Targets: A review read again reads only its refs while they stay

A review read again MUST run the `for-each-ref` of its refs alone while its tips and the baseline stay, and a list of
commits nothing; a review whose baseline names no ref (a commit id) is read whole each time.

#### Scenario: The same tips
- **WHEN** a review is read again and its tips and the baseline stay
- **THEN** it runs the `for-each-ref` of its refs alone, no `log`
  [@test] crates/sync-core/src/review_target_test.rs (`a_memoised_review_runs_no_log_for_the_same_tips`)

#### Scenario: A moved tip or baseline
- **WHEN** a tip of the review or the baseline moved
- **THEN** the review is read again
  [@test] crates/sync-core/src/review_target_test.rs (`a_moved_tip_or_baseline_is_read_again`)

#### Scenario: A kept branch
- **WHEN** a kept branch is read again
- **THEN** the review it read answers while its tip stays, and a moved tip reads its commits again
  [@test] crates/sync-core/src/review_target_test.rs (`a_memoised_kept_branch_is_read_again_only_when_its_tip_moves`)

#### Scenario: A list of commits
- **WHEN** a list of commits is read again
- **THEN** it runs nothing
  [@test] crates/sync-core/src/review_target_test.rs (`a_list_of_commits_keeps_its_order`)

#### Scenario: A branch, a kept branch and a list of commits read again
- **GIVEN** a branch, a kept branch and a list of commits were reviewed
- **WHEN** each is read again, nothing moved
- **THEN** the branch and the kept branch run one `for-each-ref` each, the list nothing
  [@test] crates/sync-core/src/process_budget_test.rs (`a_review_of_a_branch_a_kept_branch_and_a_list_of_commits`)

### Requirement: Targets: The first read of a review is one for-each-ref and one log

The first read of a review (nothing memoised for its tips and baseline) MUST run one `for-each-ref` and one `log` for
a branch; a kept branch adds the read of where its commits start (a `log` of the stack notes, and the merge base with
the baseline when they name no stack base); a list MUST be one `log --no-walk`.

#### Scenario: A branch, a kept branch and a list of commits
- **WHEN** a branch, a kept branch without a stack base and a list of commits are reviewed the first time
- **THEN** a branch is one `for-each-ref` and one `log`, a kept branch adds the read of where its commits start (the
  stack notes, the merge base), a list is one `log --no-walk`
  [@test] crates/sync-core/src/process_budget_test.rs (`a_review_of_a_branch_a_kept_branch_and_a_list_of_commits`)

### Requirement: Targets: The window starts the read of its target with its page

The window MUST start the read of its target with its page, and its first read MUST take it (`readReview`).

#### Scenario: The first read of the page
- **WHEN** the page of the window loads
- **THEN** the read of its target starts with it
- **AND** the first read of the page takes it, once, and only for the same review
  [@test] src/lib/review-window-reads.test.ts (`gives the first read of the page the read its load started, once, and only for the same review`)

### Requirement: Kept branches: Review Changes opens the review window of the kept branch

"Review Changes" (D), the first item of the menu of an unapplied or archived row (right-click and "⋯"), and D on its
focused row or the row under the pointer MUST open the review window of the kept branch (`openKeptReviewWindow`,
titled "Review docs").

#### Scenario: The menu and the focused row of an unapplied branch
- **WHEN** the user chooses "Review Changes" in the menu of an unapplied row, or presses D on its focused row
- **THEN** the review window of the kept branch opens, titled "Review docs"
  [@test] tests/e2e/unapplied-branches/unapplied-branches.spec.ts (`should open the review of an unapplied branch from its menu, from D on its row and from D on a commit of it`)

#### Scenario: The menu and the focused row of an archived branch
- **WHEN** the user chooses "Review Changes" in the menu of an archived row, or presses D on its focused row
- **THEN** the review window of the kept branch opens
  [@test] tests/e2e/archived-branches/archived-branches.spec.ts (`should review an archived branch from its menu, its focused row and its commits`)

#### Scenario: The row under the pointer
- **WHEN** the user presses D with the pointer over an unapplied or archived row
- **THEN** the review window of the kept branch opens

### Requirement: Kept branches: The review shows the commits on the commit they start on

The review of a kept branch MUST be one part of its own commits, the stack bar naming where it is kept and the commit
they start on ("Unapplied on 1a2b3c4d") instead of the baseline, its diff against that commit, not against today's
baseline.

#### Scenario: The review of an unapplied branch
- **WHEN** the review window of an unapplied branch opens
- **THEN** it shows one part of its own commits, the stack bar saying "Unapplied on 1a2b3c4d"
- **AND** its diff is against that commit, not against today's baseline
  [@test] tests/e2e/unapplied-branches/unapplied-branches.spec.ts (`should review an unapplied branch on its own base with its actions, its Reapply going on as the review of the branch it is again`)

### Requirement: Kept branches: D on a commit of the expanded row opens the review at it

D on a commit of the expanded row of a kept branch MUST open the same review at the commit ("Review docs · 1a2b3c4d").

#### Scenario: D on a commit of an unapplied branch
- **WHEN** the user presses D on a commit of the expanded row of an unapplied branch
- **THEN** the review of the branch opens at the commit, titled "Review docs · 1a2b3c4d"
  [@test] tests/e2e/unapplied-branches/unapplied-branches.spec.ts (`should open the review of an unapplied branch from its menu, from D on its row and from D on a commit of it`)

#### Scenario: D on a commit of an archived branch
- **WHEN** the user presses D on a commit of the expanded row of an archived branch
- **THEN** the review of the branch opens at the commit
  [@test] tests/e2e/archived-branches/archived-branches.spec.ts (`should review an archived branch from its menu, its focused row and its commits`)

### Requirement: Kept branches: The title bar holds the actions Rust offers for it

The title bar MUST hold the actions Rust offers for it (`domain_policy::kept`): of an unapplied branch Reapply and its
push, its "⋯" the rest of its menu, run where they run ([Actions](#actions): Reapply in the window, the push and the
typed delete at the row); an archived branch has only its "⋯" (its delete, the copies).

#### Scenario: An unapplied branch
- **WHEN** the window reviews an unapplied branch
- **THEN** its title bar holds Reapply and its push, its "⋯" the rest of its menu
- **AND** Reapply runs in the window
  [@test] tests/e2e/unapplied-branches/unapplied-branches.spec.ts (`should review an unapplied branch on its own base with its actions, its Reapply going on as the review of the branch it is again`)

#### Scenario: An archived branch
- **WHEN** the window reviews an archived branch
- **THEN** its title bar has only its "⋯": its delete, which opens its form at the row, and the copies
  [@test] tests/e2e/archived-branches/archived-branches.spec.ts (`should review an archived branch from its menu, its focused row and its commits`)

### Requirement: Kept branches: A plain open of the kept branch on screen brings the window up

A plain open of the kept branch on screen MUST only bring the window to the front, and a commit of it MUST move there
([One window](#one-window)).

#### Scenario: An open of the kept branch on screen or of its commit
- **GIVEN** the window shows a kept branch
- **WHEN** it gets a plain open of that branch, or of a commit of it
- **THEN** the plain open only brings the window to the front, and the commit moves there
  [@test] src/lib/review-reuse.test.ts (`keeps a kept branch and a whole stack for a plain open of them, moves to a commit of the kept branch`)

### Requirement: Kept branches: Space on a kept row opens its preview

Space on an unapplied or archived row (focused, or under the pointer) MUST open its preview (`KeptBranchPreviewDialog`):
its name and where it is kept with the count of its commits, then its own commits as its review reads them, newest
first, with their whole messages, as the preview of a branch reads its commits (`CommitsPreviewBody`).

#### Scenario: Space on a focused unapplied row
- **WHEN** the user presses Space on a focused unapplied row
- **THEN** its preview shows its name, where it is kept with the count of its commits, and its own commits with their
  whole messages, newest first
  [@test] tests/e2e/unapplied-branches/unapplied-branches.spec.ts (`should preview an unapplied branch with Space, its own commits with their messages, and review it from the preview`)

#### Scenario: Space on an archived row or a row under the pointer
- **WHEN** the user presses Space on an archived row, or with the pointer over a kept row
- **THEN** its preview opens the same way

### Requirement: Kept branches: The keys of the preview of a kept branch

In the preview of a kept branch, ↑/↓ MUST show the previous or next row of the card, D and the footer's
`D Review Changes` MUST review it (a commit's button at that commit), the preview closing once the window opened;
Space and Escape MUST close it.

#### Scenario: D and the review button of a commit
- **WHEN** the user presses D in the preview, or clicks the review button of a commit
- **THEN** the review of the kept branch opens, at that commit for the commit's button
- **AND** the preview closes once the window opened
  [@test] tests/e2e/unapplied-branches/unapplied-branches.spec.ts (`should preview an unapplied branch with Space, its own commits with their messages, and review it from the preview`)

#### Scenario: The footer's button
- **WHEN** the user clicks the footer's `D Review Changes`
- **THEN** the review of the kept branch opens

#### Scenario: ↑/↓, Space and Escape
- **WHEN** the user presses ↑/↓ in the preview
- **THEN** it shows the previous or next row of the card
- **AND** Space and Escape close it

### Requirement: Kept branches: After a Reapply the review goes on as that of the branch

After a Reapply the review MUST go on as the review of the branch it is again, once a sync built it (the window opens
it itself, the kept branch into its history).

#### Scenario: Reapply from the window
- **WHEN** the user runs Reapply from the title bar of the window, and a sync builds the branch
- **THEN** the window opens the review of the branch it is again itself, the kept branch into its history
  [@test] tests/e2e/unapplied-branches/unapplied-branches.spec.ts (`should review an unapplied branch on its own base with its actions, its Reapply going on as the review of the branch it is again`)

### Requirement: Kept branches: After a delete the review keeps what it read

After a delete the review MUST keep what it read, its actions disabled with why ("docs is no longer unapplied: it was
reapplied or deleted"; of an archived branch "The archived branch auth was deleted").

#### Scenario: The kept branch is deleted
- **WHEN** the kept branch the window shows is deleted
- **THEN** the window keeps what it read
- **AND** its actions are disabled with why: of an unapplied branch "docs is no longer unapplied: it was reapplied or
  deleted", of an archived one "The archived branch auth was deleted"

### Requirement: Kept branches: The review plan of a kept branch reads its range

The review plan of a kept branch MUST read its range; it MUST have no Mark Reviewed and no "Since Your Review".

#### Scenario: The plan of a kept branch
- **WHEN** the window reviews a kept branch with the review plan
- **THEN** the plan reads the range of the branch
- **AND** there is no Mark Reviewed and no "Since Your Review"

### Requirement: Kept branches: A scope whose commits change no file together says so

A scope of a range ("All Changes", "Since Your Review") whose commits change no file together, as a commit and the
fixup that takes it back, MUST say so instead of "No changed files" ("The 2 commits cancel each other out: together
they change no file", `formatRangeWithoutChanges`) and offer "Show First Commit", which shows the oldest commit of the
scope alone with its files.

#### Scenario: A commit and the fixup that takes it back
- **GIVEN** a kept branch of a commit and the fixup that takes it back
- **WHEN** its review shows "All Changes"
- **THEN** it says "The 2 commits cancel each other out: together they change no file"
- **AND** "Show First Commit" shows the oldest commit alone with its files
  [@test] tests/e2e/unapplied-branches/unapplied-branches.spec.ts (`should say that the commits of an unapplied branch cancel each other out when together they change no file, and show the first`)

#### Scenario: The words of a scope that changes no file
- **WHEN** the commits of a scope change no file together
- **THEN** `formatRangeWithoutChanges` says how many cancel each other out
  [@test] src/lib/range-review.test.ts (`says that the commits of a scope that changes no file cancel each other out`)

### Requirement: Kept branches: A commit that changes no file says No changed files

A commit that changes no file MUST still say "No changed files".

#### Scenario: A commit without changed files
- **WHEN** the review shows a commit that changes no file
- **THEN** it says "No changed files"

## References

- [Branch Review](../../branch/branch-review/spec.md), [Commit Review](../../commit/commit-review/spec.md)
- [Git Operations](../../../../docs/design/git-operations.md) ("Review of a target")
