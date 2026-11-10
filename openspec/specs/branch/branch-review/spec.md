---
name: Branch Review
description: Requirements for the review of a branch in its window: its diff from its parent or from the baseline, the parts of its stack, its commits, and its files by code owner, with the files each reviewer group of the Patronus code-owner check has to review.
style: plain-1
targets:
  - src/pages/ReviewPage.tsx
  - src/components/review/BranchReview.tsx
  - src/hooks/useBranchReview.ts
  - src/components/branch/BranchPreviewActions.tsx
  - src/stores/branch-preview.ts
  - src/components/review/BranchReviewStackBar.tsx
  - src/components/review/ReviewRobotStatus.tsx
  - src/components/ui/toggle-group.tsx
  - src/components/shared/FilterChips.tsx
  - src/components/shared/ActionButton.tsx
  - src/components/shared/WindowPage.tsx
  - src/lib/branch-review.ts
  - src/lib/review-window.ts
  - src/hooks/usePreviewHotkeys.ts
  - crates/app-api/src/remote/review.rs
  - src/lib/review-owners.ts
  - src/components/review/ReviewOwnerChips.tsx
  - src/hooks/useFileOwners.ts
  - src/hooks/useReviewOwnerFilter.ts
  - src/stores/settings/repositorySettings.ts
  - src/lib/range-review.ts
  - src/components/review/RangeReview.tsx
  - src/components/commit/CommitReview.tsx
  - src/lib/review-position.ts
  - src/hooks/useReviewActions.ts
  - src/hooks/useReviewRuns.ts
  - src/lib/review-actions.ts
  - src/lib/review-runs.ts
  - src/hooks/useMainWindowLink.ts
  - src/lib/sub-window-link.ts
  - src/model/store.ts
  - src/pages/IndexPage.tsx
  - src/hooks/useBranchContextActions.ts
  - src/hooks/useBranchRowKeys.ts
  - src/hooks/useCommitMoveList.ts
  - src/hooks/useCommitCopyActions.ts
  - src/stores/commit-move.ts
  - src/components/branch/BranchPreviewDialog.tsx
  - src/components/repository/ReviewRequestRow.tsx
  - src/components/branch/SafePushPopover.tsx
  - src/components/branch/StackIslandCard.tsx
  - src/hooks/useWholeStackActions.ts
  - src/stores/repository.ts
  - src/types/sub-window.ts
  - src/lib/pull-request-dialog.ts
  - src/lib/pull-request-run.ts
  - src/components/branch/CreatePullRequestDialog.tsx
  - src/hooks/useCreatePullRequestContext.ts
  - src/lib/sub-window.ts
  - src/router.ts
  - src-tauri/capabilities/sub-windows.json
  - src-tauri/capabilities/desktop.json
  - crates/sync-core/src/review_target.rs
  - crates/git-ops/src/ownership.rs
  - crates/app-api/src/reads/commits.rs
---

# Branch Review

Status: Implemented
Date: 2026-10-15

## Purpose

"Review Changes" (D) opens the changes of a branch in a window of its own, the only review of a branch: what the
branch adds on top of its parent, the diff of all its files in the [commit review](../../commit/commit-review/spec.md),
its commits above the files in the sidebar (`BranchReview`); the review of a pull request has the same commits
([Review Requests](../../repository/review-requests/spec.md#review-window)). A review is a task of minutes beside the
branch table: the window stays beside it (the Safe Push popover too), where a dialog covered it. A
stacked branch shows its stack from the baseline as a bar of segments, one per branch; a click shows one branch on its
parent, Shift+click extends the part, "From Root" shows everything from the baseline to the branch, root..branch, which
is what its Safe Push lands and Patronus checks; "Whole Stack" after it goes up to every branch of the stack. The whole stack, every branch of a stack that forks
([Landing](../../../../docs/design/landing.md)), is reviewed in the same window as its stack branch: "Review Whole Stack";
each of its commits names its branch, and the segments of the chain it came from go back down.
The files are labelled with their code owners by the `OWNERSHIP` files of the baseline, and chips mark them by the
reviewer groups the code-owner check of a Safe Push asks for and by owner: the story keeps every file, the others are
muted, and the keys move through the chosen owner's files. The Safe Push popover names the files each
group has to review, and its "Review Changes" opens the review on them. The title bar of the window holds the stack, the robot that lands the
branch while it runs, the diff view and the actions on the branch: Open Pull Request #42, Safe Push (Push to master or Merge #7 in its place, by the route of the
repository, [Landing Routes](../landing-routes/spec.md)) and Push Branch, built and run by the window from its own
session, a form of a row by the main window at that row; the footer holds the keys.

## Goals

- Look at what a branch changes before a push, a Safe Push or a pull request, without leaving the app.
- See a stacked branch alone and with its parents: a reviewer reads the child on its parent, Patronus checks all.
- Know which files need whose approval, and look at exactly those, with the declarations and uses around them.

## Non-goals

- No comments and no approvals: the review happens in the pull request.
- No form of a row in the window: a rename, a move, the popover of a Safe Push, the question of a push that drops
  remote commits open at the row of the branch in the main window ([Review Window](../../review/review-window/spec.md#actions)).
- No review of the children of a branch in its window: each has its own, with this branch as a segment; "Whole Stack"
  reviews every branch of the stack, the children too.
- No group membership: "my files" needs the members of a group, which the repository doesn't hold.

## Requirements

### Requirement: Opening: Review Changes opens the review window of the branch

"Review Changes" of the branch menu (key D, its only review item), D on a branch row, "Review Changes" of the Safe
Push popover (on its line of the approvals a repository only on Space needs, and on its line of the approvals a
profile skips) and D or the footer button of the branch preview MUST open the review window of the branch
(`openBranchReviewWindow`); there is no review dialog, no W and no "Open in Window".

#### Scenario: D on a branch row and Review Changes of its menu
- **WHEN** the user presses D on a branch row, or chooses "Review Changes" of the branch menu, its only review item
- **THEN** the review window of the branch opens (`openBranchReviewWindow`), no review dialog
- **AND** the menu has no "Open in Window"
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should open the review of a branch in its window from its row, its menu and its preview`)

#### Scenario: D or the footer button of the branch preview
- **WHEN** the user presses D in the branch preview, or clicks the button of its footer
- **THEN** the review window of the branch opens
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should open the review of a branch in its window from its row, its menu and its preview`)

#### Scenario: Review Changes of the Safe Push popover
- **GIVEN** the Safe Push popover shows its line of the approvals a repository only on Space needs, or its line of the approvals a profile skips
- **WHEN** the user clicks "Review Changes" of that line
- **THEN** the review window of the branch opens
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should say on Space that the approvals count in the GitHub pull request, and open Settings at its repository`)

### Requirement: Opening: The preview closes once the window opened

The branch preview MUST close once the review window opened, and stay (the error told) when it could not; the Safe
Push popover MUST stay open in the main window.

#### Scenario: The window opened from the preview
- **WHEN** D or the footer button of the branch preview opened the review window
- **THEN** the preview closes
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should open the review of a branch in its window from its row, its menu and its preview`)

#### Scenario: The window could not open
- **WHEN** the review window could not open from the preview
- **THEN** the preview stays, the error told

#### Scenario: The window opened from the Safe Push popover
- **WHEN** "Review Changes" of the Safe Push popover opened the review window
- **THEN** the Safe Push popover stays open in the main window

### Requirement: Opening: Review Changes is disabled where the branch cannot be reviewed

"Review Changes" MUST be disabled, with the reason, for a branch without commits, one that failed to sync and a
blocked one (its virtual branch is not built on its parent).

#### Scenario: A branch without commits, one that failed to sync, a blocked one
- **WHEN** a branch has no commits, failed to sync, or is blocked (its virtual branch is not built on its parent)
- **THEN** "Review Changes" is disabled, with the reason, in the words of its row
  [@test] src/lib/branch-review.test.ts (`says why Rust refuses the review of a branch, in the words of its row`)
  [@test] crates/domain-policy/src/actions_test.rs (`the_review_the_pull_request_actions_and_the_rarer_starts_follow_their_facts`)

### Requirement: Opening: Review Whole Stack opens the window of the whole stack

"Review Whole Stack" (the "⋯" of a stack island and "Whole Stack ›" of the menu of every branch of the stack) MUST
open the same window for the whole stack of a stack that forks (`openWholeStackReviewWindow`, titled "Review the whole
air stack"). Its title bar holds the actions of the whole stack ([below](#title-bar-footer-and-actions)).

#### Scenario: From the island's menu, the root's menu and the menu of a branch of the stack
- **GIVEN** a stack that forks
- **WHEN** the user chooses "Review Whole Stack" in the "⋯" of its stack island, or in "Whole Stack ›" of the menu of a branch of the stack
- **THEN** the review window opens for the whole stack (`openWholeStackReviewWindow`), titled "Review the whole air stack"
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should review the whole stack in the window of its stack branch, from the island's menu, the root's menu and the menu of a branch of it`)
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should keep the popover of a row of the island at its slot, and review the whole stack in the window of its stack branch`)

### Requirement: Opening: The whole stack is one part of its stack branch

The review of the whole stack MUST be one part, its stack branch `{prefix}/stack/<root>` on the baseline (the target
`wholeStack`, `ReviewTarget::WholeStack`), every commit of the stack in the order of the history, its stack bar the
main line and "Whole Stack" ([below](#the-stack-and-the-commits)); it MUST be read again when a sync moves the stack
branch.

#### Scenario: The part of the whole stack
- **WHEN** the review of the whole stack is read
- **THEN** it is one part, its stack branch `{prefix}/stack/<root>` on the baseline, with every commit of the stack in the order of the history
- **AND** its stack bar is the main line and "Whole Stack"
  [@test] crates/sync-core/src/review_target_test.rs (`the_whole_stack_is_one_part_of_its_stack_branch`)
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should review the whole stack in the window of its stack branch, from the island's menu, the root's menu and the menu of a branch of it`)

#### Scenario: A sync moves the stack branch
- **WHEN** a sync moves the stack branch while its review is open
- **THEN** the review of the whole stack is read again

### Requirement: Opening: Review Whole Stack is disabled while the stack branch has no tip

"Review Whole Stack" MUST be disabled, with the reason, while the stack branch has no tip.

#### Scenario: A stack branch without a tip
- **WHEN** the stack branch of a stack that forks has no tip
- **THEN** "Review Whole Stack" is disabled, with the reason

### Requirement: Opening: Review Whole Stack of a straight chain reviews its last branch

Of a straight chain, which has no stack branch, "Review Whole Stack" MUST open the review of its last branch from the
root (`range: "all"`).

#### Scenario: A straight chain
- **GIVEN** a straight chain, which has no stack branch
- **WHEN** the user chooses "Review Whole Stack"
- **THEN** the review of its last branch opens from the root (`range: "all"`)

### Requirement: Opening: D on a commit row opens the window at the commit

D on a commit row of a branch, the "Review Changes" of its menu and the D, the footer button and Enter in the Files
tree of its preview MUST open the window at the commit (`reviewBranchCommit`, `focus.commitId` the copy of the commit
in the virtual branch), its branch's own part, titled "Review <branch> · a1b2c3d"
([Commit Review](../../commit/commit-review/spec.md#opening-and-closing): a commit the branch review can't show opens
the review of its list).

#### Scenario: The review of a commit of a stacked branch
- **WHEN** the user presses D on a commit row of a branch, chooses "Review Changes" of its menu, or presses D, the footer button or Enter in the Files tree of its preview
- **THEN** the window opens at the copy of the commit in the virtual branch (`focus.commitId`), its branch's own part
- **AND** it is titled "Review <branch> · a1b2c3d"
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should review a commit of a stacked branch with its stack and its actions, and move to a commit of the chain`)

#### Scenario: A commit the branch review can't show
- **WHEN** the branch review can't show the commit
- **THEN** the review of its list opens

### Requirement: Opening: Review Changes of a commit of the preview opens the window at it

The "Review Changes" button of each commit of the branch preview MUST open the window at that commit, and the preview
MUST close once the window opened.

#### Scenario: A commit of the branch preview
- **WHEN** the user clicks the "Review Changes" button of a commit of the branch preview
- **THEN** the window opens at that commit in the review of the branch
- **AND** the preview closes once the window opened
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should review a commit of the branch preview in the review of the branch, at the commit`)

### Requirement: Opening: D on an own pull request handed back opens the review of its branch

D on a row of Your Turn's own pull requests with changes requested MUST open the review of its branch
([Review Requests](../../repository/review-requests/spec.md)).

#### Scenario: An own pull request with changes requested
- **WHEN** the user presses D on a row of Your Turn's own pull requests with changes requested
- **THEN** the review of its branch opens
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should list the own pull requests a reviewer handed back after the requests, show the branch and review it`)

### Requirement: Opening: The window is the one review window

The window MUST be the one review window, titled "Review <branch>" ([Review Window](../../review/review-window/spec.md#one-window):
how an open goes on from the review on screen, Back; [Title Bar](../../repository/title-bar/spec.md) for how it
opens).

#### Scenario: An open of the review of a branch
- **WHEN** the review of a branch opens
- **THEN** it opens in the one review window, titled "Review <branch>", going on from the review on screen as the review window does

### Requirement: Opening: An open is refused while the window's dialog is open

An open that would start the window over while its "Create pull request" dialog is open MUST be refused with "Cannot
review login" and the reason, an `info` toast: the dialog holds what the user typed.

#### Scenario: The Create pull request dialog of the window is open
- **GIVEN** the "Create pull request" dialog of the review window is open
- **WHEN** an open would start the window over
- **THEN** it is refused with "Cannot review login" and the reason, an `info` toast
- **AND** the dialog keeps what the user typed

### Requirement: Opening: The window reads the repository it was opened for

The window MUST read the repository it was opened for, not the one selected later in the main window
(`getDiffRepositoryPath`).

#### Scenario: Another repository selected in the main window
- **GIVEN** the review window is open for a repository
- **WHEN** the user selects another repository in the main window
- **THEN** the window still reads the repository it was opened for (`getDiffRepositoryPath`)

### Requirement: Opening: A move to a commit of the chain keeps the part that has it

A move to a commit of the chain on screen (`move` of the window data) MUST keep the part when it has the commit, else
it is the commit's segment; the owner chip MUST stay and apply to the new range.

#### Scenario: The part has the commit
- **WHEN** the window moves to a commit of the chain on screen and the part has the commit
- **THEN** the part stays
  [@test] src/lib/branch-review.test.ts (`keeps the part that has the commit, else shows the commit's segment`)
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should review a commit of a stacked branch with its stack and its actions, and move to a commit of the chain`)

#### Scenario: The part lacks the commit
- **WHEN** the window moves to a commit of the chain on screen that the part lacks
- **THEN** the part is the commit's segment
  [@test] src/lib/branch-review.test.ts (`keeps the part that has the commit, else shows the commit's segment`, `shows all changes of the commit's branch when the segments don't have the commit`)

#### Scenario: The owner chip of a move
- **GIVEN** an owner chip is chosen
- **WHEN** the window moves to a commit of the chain
- **THEN** the owner chip stays and applies to the new range

### Requirement: Opening: A move shows the file of its path when the shown set has it

On a move, the file of `path` MUST show when it is in the shown set; otherwise the selection MUST follow the first file
the chip keeps.

#### Scenario: The file of the move in the shown set
- **WHEN** a move names a `path` that is in the shown set
- **THEN** that file shows

#### Scenario: The file of the move outside the shown set
- **WHEN** a move names a `path` that is not in the shown set
- **THEN** the selection follows the first file the chip keeps

### Requirement: Opening: A move waits while the Create pull request dialog is open

A move MUST wait while the "Create pull request" dialog of the window is open, and the dialog MUST keep what was typed.

#### Scenario: A commit reviewed while the dialog is open
- **GIVEN** the "Create pull request" dialog of the window is open with what the user typed
- **WHEN** a commit is reviewed, moving the window
- **THEN** the dialog keeps what was typed
- **AND** the window moves to the commit once the dialog closes
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should keep the Create pull request dialog of the window while a commit is reviewed, and move to the commit once it closes`)

### Requirement: Opening: The review gets the chain, the baseline, the focus and the groups

The review SHALL get the chain of the branch (its stack ancestors by `effectiveBase`, root first), the baseline, where
to start (`BranchReviewFocus`: the branch itself, or the whole stack from the Safe Push popover and the island; the
owner chip, the commit, the file), and the reviewer groups of the last code-owner check of the branch at its current
head.

#### Scenario: The data of an open
- **WHEN** the review of a branch opens
- **THEN** it gets the chain of the branch by `effectiveBase`, root first, and the baseline
- **AND** where to start (`BranchReviewFocus`): the branch itself or the whole stack, the owner chip, the commit, the file
- **AND** the reviewer groups of the last code-owner check of the branch at its current head

### Requirement: Opening: The app reads the review ahead as its window opens

The app MUST read the review ahead as its window opens, beside the start of the window: the backend reads the segments
and the files and the patch of the part the window shows first (the branch on its parent, or the whole stack) into its
memory while the window starts, so its first read finds them read (`prefetch_review`).

#### Scenario: The open of a review window
- **WHEN** the review window of a branch opens
- **THEN** the backend reads the segments, the files and the patch of the part the window shows first into its memory while the window starts (`prefetch_review`)
- **AND** the first read of the window finds them read
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should read the review of a branch ahead as its window opens, once per state, and nothing on a hover or a focus of its row`)

### Requirement: Opening: The read ahead runs once per state of the branch

The read ahead MUST run once per state of the branch: its chain and their heads; an open of the branch the window
shows MUST read nothing again.

#### Scenario: A second open of the same state
- **GIVEN** the window shows the review of a branch, its chain and heads unchanged
- **WHEN** the review of that branch opens again
- **THEN** nothing is read again
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should read the review of a branch ahead as its window opens, once per state, and nothing on a hover or a focus of its row`)

### Requirement: Opening: A hover or a focus of a branch row reads nothing

A hover or a focus of a branch row MUST read nothing: no IPC for a pointer that passes over the table.

#### Scenario: The pointer passes over the table
- **WHEN** the pointer hovers a branch row, or a branch row takes the focus
- **THEN** nothing is read, no IPC
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should read the review of a branch ahead as its window opens, once per state, and nothing on a hover or a focus of its row`)
  [@test] tests/e2e/patronus/patronus.spec.ts (`should ask the session nothing for the hover and the menu of a row, and tell it only the details of the robot whose popover opens`)

### Requirement: Opening: The window starts the read of its segments with its page

The window MUST start the read of its segments with its page, not after the chunk of its page loaded; its first read
takes that read when it asks for the same review, a later one reads again.

#### Scenario: The first read of the page
- **WHEN** the page of the window loads and its first read asks for the same review
- **THEN** the first read takes the read its load started, once
- **AND** a later read, or a read of another review, reads again
  [@test] src/lib/review-window-reads.test.ts (`gives the first read of the page the read its load started, once, and only for the same review`)

### Requirement: Opening: The keys of the window are the review's, never a push

In the window ↑/↓, Space and P MUST be the review's (P is the previous change), never a push.

#### Scenario: P in the window
- **WHEN** the user presses P in the review window
- **THEN** it goes to the previous change and nothing is pushed
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should open the review of a branch in its window from its row, its menu and its preview`)

### Requirement: The stack and the commits: The stack bar shows the baseline and a segment per branch

The stack bar MUST show the baseline, then a segment per branch of the chain with its number of commits. The branch
itself MUST be selected at first.

#### Scenario: The stack bar of a stacked branch
- **WHEN** the review of a stacked branch opens
- **THEN** the stack bar shows the baseline, then a segment per branch of the chain with its number of commits
- **AND** the branch itself is selected
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should review a stacked branch on its parent, then the whole stack, by owner`)

### Requirement: The stack and the commits: A click selects a segment, Shift+click extends the part

A click MUST select one segment, Shift+click MUST extend the part to the clicked segment; "From Root" MUST select all of
them (root..branch, not the whole stack).

#### Scenario: Click, Shift+click and From Root
- **WHEN** the user clicks a segment
- **THEN** that segment alone is selected
- **AND** Shift+click extends the part to the clicked segment, and "From Root" selects all of them, root..branch, not the whole stack
  [@test] src/lib/branch-review.test.ts (`selects one segment, or extends the range to the clicked one with Shift`)
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should review a stacked branch on its parent, then the whole stack, by owner`)

### Requirement: The stack and the commits: The diff runs from the base of the part to its last head

The diff MUST be from the base of the first segment of the part (the head of its parent, the commit the root is built
on) to the head of its last one.

#### Scenario: The diff of a part
- **WHEN** a part of the stack is selected
- **THEN** the diff is from the base of its first segment (the head of its parent, the commit the root is built on) to the head of its last one
  [@test] src/lib/branch-review.test.ts (`diffs the base of the first segment to the head of the last one`)

### Requirement: The stack and the commits: The stack bar stays on the one line of the title bar

The stack bar MUST stay on one line, the title bar's, never running under its actions: the baseline and the segments
outside the part give way first (cut with "…", the name in their hover), the selected segments and the last one last.

#### Scenario: The stack bar in the title bar
- **WHEN** the review window shows a stacked branch
- **THEN** the stack bar stays on the one line of the title bar, never running under its actions
  [@test] tests/e2e/repository/title-bar.spec.ts (`should start the row of a review window after the traffic lights, its empty parts dragging the window`)

#### Scenario: A stack bar wider than its room
- **WHEN** the stack bar is wider than its room
- **THEN** the baseline and the segments outside the part give way first, cut with "…", the name in their hover
- **AND** the selected segments and the last one give way last

### Requirement: The stack and the commits: Whole Stack goes up to every branch of the root's stack

After "From Root", "Whole Stack" MUST go up to every branch of the stack of the chain's root while the stack has
branches outside the chain (`WholeStackScope`, `wholeStackScopeOf`); the window opens that review itself
(`openWholeStackReviewWindow`), the chain as the way back.

#### Scenario: A branch whose stack has branches outside its chain
- **GIVEN** the stack of the chain's root has branches outside the chain
- **WHEN** the user presses "Whole Stack" after "From Root"
- **THEN** the window opens the review of the whole stack itself, the chain as the way back
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should move the review of a branch up to the whole stack, and down to a branch of it and to the branch of a commit`)

### Requirement: The stack and the commits: From Root and Whole Stack are one option set

"From Root" and "Whole Stack" MUST be one option set "Scope" of the two (the stock `ToggleGroup`, so they read as
controls, not as two words of the title), one pressed at most; a press of the pressed one MUST keep it.

#### Scenario: A press of the pressed scope
- **WHEN** the user presses the scope that is pressed
- **THEN** it stays pressed, and at most one of the two is pressed
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should move the review of a branch up to the whole stack, and down to a branch of it and to the branch of a commit`)

### Requirement: The stack and the commits: Whole Stack of a stack that forks is its stack branch

Of a stack that forks "Whole Stack" MUST be the review of its stack branch, its hover "Every branch of the whole air
stack, its side branches too, on origin/master: 7 branches · 14 commits, what its Safe Push lands" (what is left of
it, `StackLanding.rest`).

#### Scenario: The Whole Stack of a stack that forks
- **GIVEN** a stack that forks
- **WHEN** the user hovers or presses "Whole Stack"
- **THEN** its hover says "Every branch of the whole air stack, its side branches too, on origin/master: 7 branches · 14 commits, what its Safe Push lands", of what is left of it (`StackLanding.rest`)
- **AND** a press opens the review of its stack branch
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should move the review of a branch up to the whole stack, and down to a branch of it and to the branch of a commit`)

### Requirement: The stack and the commits: Whole Stack of a straight chain is its last branch

Of a straight chain "Whole Stack" MUST be the review of its last branch from the root ("IJPL-257337 to perf-part-2 on
origin/master: 2 branches · 3 commits, what the Safe Push of perf-part-2 lands"), and its last branch MUST have none:
From Root is the whole stack.

#### Scenario: A branch of a straight chain with branches built on it
- **GIVEN** a straight chain, and the review of a branch with branches built on it
- **WHEN** the user hovers "Whole Stack"
- **THEN** it says "IJPL-257337 to perf-part-2 on origin/master: 2 branches · 3 commits, what the Safe Push of perf-part-2 lands", the review of its last branch from the root
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should offer the whole stack of a straight chain on a branch with branches built on it, as the last one from the root`)

#### Scenario: The last branch of a straight chain
- **WHEN** the review shows the last branch of a straight chain
- **THEN** it has no "Whole Stack": From Root is the whole stack
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should offer the whole stack of a straight chain on a branch with branches built on it, as the last one from the root`)

### Requirement: The stack and the commits: Whole Stack is disabled with the reason Rust gives

"Whole Stack" MUST be disabled with the reason Rust gives "Review Whole Stack" (`review-whole-stack` of `stack:<root>`:
the stack branch not built, the review of the last branch blocked).

#### Scenario: A Whole Stack that can't open
- **WHEN** the stack branch is not built, or the review of the last branch is blocked
- **THEN** "Whole Stack" stays in the scopes, disabled with the reason of `review-whole-stack` of `stack:<root>` on hover
- **AND** a press changes nothing
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should keep an unavailable Whole Stack in the scopes, announced as unavailable, its reason on hover, a press changing nothing`)
  [@test] crates/domain-policy/src/actions_test.rs (`the_whole_stack_starts_as_its_step_says_and_lands_in_pieces_while_its_stack_branch_is_not_built`)

### Requirement: The stack and the commits: The stack bar of the whole stack shows the chain it came from

The stack bar of the review of the whole stack MUST show the chain it came from (`from` of the window data; the main
line from the island and the menus), each segment with its number of commits, none pressed, then "Whole Stack",
pressed.

#### Scenario: The review of the whole stack the island opens
- **WHEN** the review of the whole stack opens from the island or a menu
- **THEN** its stack bar shows the main line, each segment with its number of commits, none pressed
- **AND** then "Whole Stack", pressed
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should go down from the review of the whole stack the island opens, along its main line`)

### Requirement: The stack and the commits: A segment of the whole stack reviews its branch

A segment of the stack bar of the whole stack MUST review its branch, which the window opens itself, once it holds the
table of its session whole: a press before its first view, or while the session builds the branches, waits for it and
MUST NOT say the branch is gone.

#### Scenario: A press of a segment
- **WHEN** the user presses a segment of the stack bar of the whole stack
- **THEN** the window opens the review of its branch itself
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should move the review of a branch up to the whole stack, and down to a branch of it and to the branch of a commit`, `should go down from the review of the whole stack the island opens, along its main line`)

#### Scenario: A press before the table of the session is whole
- **WHEN** the press comes before the first view of the window, or while the session builds the branches
- **THEN** it waits for the table of the session whole, and never says the branch is gone
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should go down from the review of the whole stack the island opens, along its main line`)

### Requirement: The stack and the commits: Each commit of the whole stack names its branch

Each commit of the column of the review of the whole stack (with the plan, each commit row of the tree) MUST name its
branch, muted after its subject, matched across the copies by author, author time and subject (`reviewCommitKey`: the
stack branch has no mapping notes; `stackCommits` of the table the window reads).

#### Scenario: The commits of the whole stack
- **WHEN** the review of the whole stack shows its commits, in the column or, with the plan, as commit rows of the tree
- **THEN** each names its branch, muted after its subject, matched across the copies by author, author time and subject
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should move the review of a branch up to the whole stack, and down to a branch of it and to the branch of a commit`, `should name the branch of each commit row of the plan's tree, from the root and in the whole stack, and review the branch of a commit shown alone`)

### Requirement: The stack and the commits: The heading of a commit of the whole stack reviews its branch

In the review of the whole stack the heading of the shown commit MUST end with "Review side-fix", the review of its
branch at the commit (`{ review: "commit" }`).

#### Scenario: Review side-fix
- **GIVEN** the review of the whole stack shows a commit of the branch side-fix alone
- **WHEN** the user clicks "Review side-fix" at the end of its heading
- **THEN** the review of side-fix opens at the commit (`{ review: "commit" }`)
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should move the review of a branch up to the whole stack, and down to a branch of it and to the branch of a commit`, `should name the branch of each commit row of the plan's tree, from the root and in the whole stack, and review the branch of a commit shown alone`)

### Requirement: The stack and the commits: A commit row of a part of several branches names its branch

With the plan a commit row of the tree of a part of several branches MUST name its branch too, which the column heads
by branch.

#### Scenario: The tree of a part from the root
- **GIVEN** the plan, and a part of several branches
- **WHEN** the tree shows a commit row
- **THEN** the row names its branch, as the column heads its commits by branch
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should name the branch of each commit row of the plan's tree, from the root and in the whole stack, and review the branch of a commit shown alone`)

### Requirement: The stack and the commits: The stack bar of a long stack folds its middle

The stack bar of a stack of more than 6 branches MUST stay short: it shows the root, the ends of the selected part and
the last two segments (the parent and the branch), and folds each run of two or more others into a ghost "… 14 more"
button (in `text-primary` while the part covers it) with "14 branches · 120 commits · Click to show them" as its
tooltip; a click MUST show every segment.

#### Scenario: A stack of more than 6 branches
- **WHEN** the stack has more than 6 branches
- **THEN** the stack bar shows the root, the ends of the selected part and the last two segments
- **AND** each run of two or more others folds into a ghost "… 14 more" button, its tooltip "14 branches · 120 commits · Click to show them", in `text-primary` while the part covers it
  [@test] src/lib/branch-review.test.ts (`folds the middle of a long stack, keeping the root, the ends of the range and the last two segments`)

#### Scenario: A short stack, or one unfolded
- **WHEN** the stack has 6 branches or fewer, or the user clicked the fold
- **THEN** the stack bar shows every segment
  [@test] src/lib/branch-review.test.ts (`shows every segment of a short stack, or once unfolded`)

### Requirement: The stack and the commits: The segments come from the virtual branches

The segments MUST come from the virtual branches: one `for-each-ref` for the tips, one `git log --first-parent` from
the baseline to the last tip, cut at the tips of the parents. A parent whose head is not below its child, or a branch
without a virtual branch, MUST be an error that asks for a sync.

#### Scenario: A stack split at the heads of its parents
- **WHEN** the segments of a stack are read
- **THEN** one `for-each-ref` reads the tips, one `git log --first-parent` from the baseline to the last tip, cut at the tips of the parents
  [@test] crates/sync-core/src/review_target_test.rs (`splits_a_stack_at_the_heads_of_the_parents`)

#### Scenario: A branch outside a stack
- **WHEN** the segments of a branch outside a stack are read
- **THEN** it is one segment from the baseline
  [@test] crates/sync-core/src/review_target_test.rs (`a_branch_outside_a_stack_is_one_segment_from_the_baseline`)

#### Scenario: A child not built on the head of its parent
- **WHEN** a parent's head is not below its child, or a branch has no virtual branch
- **THEN** the read is an error that asks for a sync
  [@test] crates/sync-core/src/review_target_test.rs (`a_child_not_built_on_the_head_of_its_parent_needs_a_sync`)

### Requirement: The stack and the commits: The segments are remembered by the tips and the baseline

The app MUST remember the segments by the tips and the baseline's refs that the `for-each-ref` reads (it lists the
refs the baseline's name may stand for too), and run the `log` again only when a tip or the baseline moved.

#### Scenario: The same tips
- **WHEN** the segments are read again for the same tips and baseline
- **THEN** no `log` runs
  [@test] crates/sync-core/src/review_target_test.rs (`a_memoised_review_runs_no_log_for_the_same_tips`)

#### Scenario: A tip or the baseline moved
- **WHEN** a tip or one of the refs the baseline's name may stand for moved
- **THEN** the `log` runs again
  [@test] crates/sync-core/src/review_target_test.rs (`a_moved_tip_or_baseline_is_read_again`)

### Requirement: The stack and the commits: The commits are above the files in the sidebar

The commits (`RangeReview`, the ones of a pull request too) MUST be above the files in the sidebar of the commit
review ([Commit Review](../../commit/commit-review/spec.md#the-sidebar-and-the-diffs)): "All Changes" with the number of
commits of the part, then its commits newest first, under the name of their branch when the part has several, the
subject taking the width of the row and the hash only on hover (the heading of the shown commit has it).

#### Scenario: The commits of a part
- **WHEN** the review shows a part of several commits
- **THEN** the sidebar lists "All Changes" with the number of commits of the part, then its commits newest first, above the files
- **AND** under the name of their branch when the part has several, the subject taking the width of the row, the hash only on hover
  [@test] src/lib/range-review.test.ts (`lists all changes, then the commits of the reviewed branch first, each branch headed by its name`)
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should review a stacked branch on its parent, then the whole stack, by owner`)

### Requirement: The stack and the commits: A commit shows only its own changes

A commit MUST show only its own changes, its subject, author, time and whole message at the top of its document, above
them.

#### Scenario: A commit chosen in the list
- **WHEN** the user chooses a commit of the list
- **THEN** the review shows only its own changes, its subject, author, time and whole message at the top of its document, above them

### Requirement: The stack and the commits: Option+arrows step through the commits from anywhere

⌥↑/⌥↓ MUST step through the commits from anywhere in the review, ↑/↓ inside them (one tab stop, a listbox), which MUST
keep the focus while the diffs of the next commit mount.

#### Scenario: Stepping through the commits
- **WHEN** the user presses ⌥↑/⌥↓ anywhere in the review, or ↑/↓ in the list of the commits
- **THEN** the review steps to the next or previous commit, the list one tab stop, a listbox
- **AND** the list keeps the focus while the diffs of the next commit mount

### Requirement: The stack and the commits: A part of one commit has no list

A part of one commit MUST have no list of the commits: it shows that commit.

#### Scenario: A part of one commit
- **WHEN** the part has one commit
- **THEN** the sidebar has no list of the commits and the review shows that commit

### Requirement: The stack and the commits: A sync that rebuilds the commits keeps the shown one

A sync that rebuilds the commits MUST keep the shown one by its branch, subject and author time.

#### Scenario: A commit a sync rebuilt
- **GIVEN** the review shows a commit
- **WHEN** a sync rebuilds the commits
- **THEN** the review keeps the commit by its id, else the one the sync rebuilt in its place, by its branch, subject and author time
  [@test] src/lib/range-review.test.ts (`keeps the commit by its id, else the one a sync rebuilt in its place`)

### Requirement: The stack and the commits: Another commit, chip or sync keeps the file on screen

Another commit, another owner chip or a sync that reads the stack again MUST keep the file on screen when the new
changes have it; an owner chip MUST keep every file, the selection and the collapsed directories.

#### Scenario: Another commit, another chip, a sync
- **WHEN** the user chooses another commit or another owner chip, or a sync reads the stack again, and the new changes have the file on screen
- **THEN** that file stays on screen
- **AND** an owner chip keeps every file, the selection and the collapsed directories
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should review a stacked branch on its parent, then the whole stack, by owner`)

### Requirement: The stack and the commits: The file on screen is kept while the chip lacks it for a while

The file on screen MUST be kept by its path also while the files of the chosen chip lack it for a while: the first of
them stands in for it (no place of the reader: the review read again starts at the file on screen), and the review
selects that file again once the files of the chip show it, unless the reader moved.

#### Scenario: A sync moves the head before the code-owner check of the new head answers
- **GIVEN** the chip of a group is chosen
- **WHEN** a sync moves the head before the code-owner check of the new head answers, which makes the chip of the group the owner of its name by the `OWNERSHIP` files, with fewer files
- **THEN** the first of them stands in for the file on screen, and no file shows as the place of the reader
- **AND** once the files of the chip show it, the review selects the file on screen again, unless the reader moved
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should stay at the file on screen by its path when a sync moves the head while the chip of its group shows fewer files, until the check of the new head answers`)
  [@test] src/lib/review-position.test.ts (`shows no file while one stands in for the file on screen the chip lacks`)

#### Scenario: The reader chooses another chip or search
- **WHEN** the reader chooses another chip or search
- **THEN** the review selects the first file of what it keeps

### Requirement: The stack and the commits: A moved head or another part keeps the review on screen

A sync that moves a head of the part, or another part chosen in the stack bar, MUST keep the review on screen (the
tree with the steps of its plan and their commits, the column, the files, the diffs, the file on screen and the
position of the footer) until the files, the diffs and the plan of the new changes are read, which the window reads
ahead as it reads the neighbours of a commit.

#### Scenario: A sync moved the head
- **WHEN** a sync moves a head of the part, or the user chooses another part in the stack bar
- **THEN** the review on screen stays (the tree with the steps of its plan and their commits, the column, the files, the diffs, the file on screen, the position of the footer)
- **AND** it stays until the files, the diffs and the plan of the new changes are read
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should keep the review on screen after a sync that moved the head until the files, the diffs and the plan of the new head are read, then show them at once`)
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should review a stacked branch on its parent, then the whole stack, by owner`)
  [@test] src/lib/review-plan-answer.test.ts (`ends the read of a plan with its usages, disabled, a failure or no plan, never with a plan of other changes or one without its usages`)

### Requirement: The stack and the commits: The new review shows in one change

Once the new changes are read, the new review MUST show in one change, at the file on screen when the new changes have
it. Meanwhile the body of the review is `aria-busy`, and the stack bar MUST show the part chosen at once.

#### Scenario: The new changes are read
- **WHEN** the files, the diffs and the plan of the new changes are read
- **THEN** the new review shows in one change, at the file on screen when the new changes have it
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should keep the review on screen after a sync that moved the head until the files, the diffs and the plan of the new head are read, then show them at once`)

#### Scenario: While the new changes are read
- **WHEN** the new changes are being read
- **THEN** the body of the review is `aria-busy`
- **AND** the stack bar shows the part chosen at once
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should keep the review on screen after a sync that moved the head until the files, the diffs and the plan of the new head are read, then show them at once`)

### Requirement: The stack and the commits: A commit chosen in the column shows at once

A commit chosen in the column (a click, ⌥↑/⌥↓, D on a commit row) MUST show at once; a review restored by Back starts
as an open does: the window has no review of it to keep.

#### Scenario: A commit chosen in the column
- **WHEN** the user chooses a commit in the column by a click or ⌥↑/⌥↓, or presses D on a commit row
- **THEN** the commit shows at once

#### Scenario: A review restored by Back
- **WHEN** Back restores a review
- **THEN** it starts as an open does: the window has no review of it to keep

### Requirement: The stack and the commits: The window reads the tips of its own subscription

The window MUST read the tips of its own subscription: it subscribes to the session of its repository itself
(`ensureSession` of its own `src/model/store.ts`, `subscribe_view` under its label, beside the main window's
subscription and independent of it).

#### Scenario: A sync of the main window moves a tip
- **GIVEN** the review window subscribed to the session of its repository under its label
- **WHEN** a sync of the main window moves a tip of the chain
- **THEN** the window reads the stack again from its own session
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should read the stack again from the window's own session when a sync of the main window moves a tip of it, and keep a branch that left`)

#### Scenario: Two windows subscribed
- **WHEN** a second window subscribes to the session beside the main window
- **THEN** its subscription is independent of the first's: its own view, its own count of the runs
  [@test] src/model/store.test.ts (`keeps the subscription of a second window independent of the first's: its own view, its own count of the runs`)

### Requirement: The stack and the commits: The window takes the heads and the baseline from its view

The window MUST take from its view the head of each branch of the chain (the copy of its newest commit, else that
commit), of the review of the whole stack the tip of its stack branch and the branch of each of its commits by
`reviewCommitKey`, and the baseline (`settledReviewTable`).

#### Scenario: The heads of the chain
- **WHEN** the view of the window settled
- **THEN** the window takes the head of each branch of the chain, the copy of its newest commit, else that commit, and the baseline (`settledReviewTable`)
  [@test] src/lib/branch-review.test.ts (`reads the heads of the chain once the view settled, never from a running sync or a table no sync grouped`)

#### Scenario: The review of the whole stack
- **WHEN** the window shows the review of the whole stack and its view settled
- **THEN** it takes the tip of the stack branch and the branch of each of its commits by `reviewCommitKey`
  [@test] src/lib/branch-review.test.ts (`reads the tip of the stack branch of a whole stack and the branch of each of its commits`)

### Requirement: The stack and the commits: The window takes the heads once the view settled

The window MUST take the heads and the baseline once the view settled: once per finished sync of the session, whichever window asked for it,
also while the main window shows another repository, never during a run, and not before a sync grouped the branches.

#### Scenario: A running sync, a table no sync grouped
- **WHEN** a sync of the session is running, or no sync grouped the branches yet
- **THEN** the window takes no heads from the view
  [@test] src/lib/branch-review.test.ts (`reads the heads of the chain once the view settled, never from a running sync or a table no sync grouped`)

#### Scenario: A finished sync
- **WHEN** a sync of the session finished, whichever window asked for it, also while the main window shows another repository
- **THEN** the window takes the heads once
- **AND** a view that changed nothing the review reads keeps its table
  [@test] src/lib/branch-review.test.ts (`keeps the table of a view that changed nothing the review reads`)

### Requirement: The stack and the commits: The window carries the version of its view

The window MUST carry the version of its view (`data-view-version` of `review-window`); the main window MUST send none
of it.

#### Scenario: The version of the view of the window
- **WHEN** the view of the window changes
- **THEN** `review-window` carries its version as `data-view-version`, and the main window sends none of it

### Requirement: The stack and the commits: The read at the open uses the baseline of the window data

The read at the open MUST use the baseline of the window data, which the page starts at its load.

#### Scenario: The first read of the window
- **WHEN** the window opens
- **THEN** its first read uses the baseline of the window data, started at the load of the page

### Requirement: The stack and the commits: A moved tip of the chain reads the stack again

A sync that moves a tip of the chain MUST read the stack again, once per set of heads, and keep the part by the names
of its ends (the reviewed branch alone when an end left the chain).

#### Scenario: A sync moved a tip
- **WHEN** a sync moves a tip of the chain
- **THEN** the window reads the stack again, once per set of heads
- **AND** it keeps the part by the names of its ends
  [@test] src/lib/branch-review.test.ts (`keeps the range by the names of its ends, or falls back to the reviewed branch`)
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should read the stack again from the window's own session when a sync of the main window moves a tip of it, and keep a branch that left`)

#### Scenario: An end left the chain
- **WHEN** an end of the part left the chain
- **THEN** the part is the reviewed branch alone
  [@test] src/lib/branch-review.test.ts (`keeps the range by the names of its ends, or falls back to the reviewed branch`)

### Requirement: The stack and the commits: A branch that left keeps the last view

A branch of the chain no longer in the table of the window's session (it landed, it was renamed) MUST keep the last
view with "login is no longer a branch: it landed or was renamed": an info `Alert` above the review in the window; its
Safe Push and Push Branch MUST be disabled with that reason, its pull request still opens, and the stack is not read
again.

#### Scenario: A branch that landed or was renamed
- **WHEN** a branch of the chain is no longer in the table of the window's session
- **THEN** the window keeps the last view with "login is no longer a branch: it landed or was renamed", an info `Alert` above the review
- **AND** its Safe Push and Push Branch are disabled with that reason, its pull request still opens, and the stack is not read again
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should read the stack again from the window's own session when a sync of the main window moves a tip of it, and keep a branch that left`)

### Requirement: Title bar, footer and actions: The title bar shows the stack bar, the diff view and the actions

The title bar of the window ([Title Bar](../../repository/title-bar/spec.md)) MUST show the stack bar and after it the
robot that lands the target branch while it runs, then at its other end the Split/Unified/Inline toggle and the
actions on the target branch.

#### Scenario: The title bar of the review window
- **WHEN** the review window of a branch shows
- **THEN** its title bar shows the stack bar, then at its other end the Split/Unified/Inline toggle and the actions on the target branch
- **AND** after the stack bar the robot that lands the target branch while it runs
  [@test] tests/e2e/repository/title-bar.spec.ts (`should start the row of a review window after the traffic lights, its empty parts dragging the window`)
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should build the actions of the title bar of each part from the window's own session, and open the pull request itself`)
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should show the running Safe Push of the branch after its stack bar as its row does, its details with Open in Patronus on hover, a click opening it`)

### Requirement: Title bar, footer and actions: The title bar shows the robot that lands the branch

The title bar MUST show after the stack bar the robot that lands what the review shows while it runs, as the Status
cell of its row does: the ring, the time, the line under it; its hover the popover of its details with "Open in
Patronus", a click opening it. Of a branch it is its own robot, else the running Safe Push that lands it with more
("lands with session"); of the whole stack its robot. Nothing shows while none runs, nor for a kept branch, a list of
commits or a pull request.

#### Scenario: The running Safe Push of the branch
- **WHEN** the review window shows a branch whose Safe Push runs
- **THEN** its title bar shows after the stack bar the ring, "18m · ~22m" and the line, as the row of the branch
- **AND** its hover is the popover of its details, whose "Open in Patronus" opens it, as a click on it does
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should show the running Safe Push of the branch after its stack bar as its row does, its details with Open in Patronus on hover, a click opening it`)

#### Scenario: A robot that will fail
- **WHEN** a required check of the running robot failed
- **THEN** its ring and its line are `destructive`, its time "18m · Failing"
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should show the robot that will fail in the colour of a failure, Failing in place of the time left`)

#### Scenario: The Safe Push of a branch built on it lands it
- **WHEN** the review shows a branch that the running Safe Push of a branch built on it lands
- **THEN** the title bar shows that robot, "lands with session" after it, its details on hover
- **AND** a branch that no running robot lands shows none
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should show the running Safe Push of a branch built on the branch that lands it, and nothing on a branch it doesn't land`)

#### Scenario: The robot of the whole stack
- **WHEN** the robot of the whole stack runs
- **THEN** the review of the whole stack shows it, and the review of a branch of it with "lands with the whole air stack"
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should show the running Safe Push of the whole stack after the stack bar of its review, and of each branch of it, which it lands`)

#### Scenario: The narrowest window
- **WHEN** the stack bar and the actions leave the robot little room
- **THEN** the branches keep their names: the robot shows how long it runs first, "lands with session" before the time
  left, and with no room the ring and the line alone, the name of the robot saying its time
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should show the running Safe Push of the branch after its stack bar as its row does, its details with Open in Patronus on hover, a click opening it`)

### Requirement: Title bar, footer and actions: The footer shows the position and the keys

The footer MUST show the position in the review ("All Changes · File 2 / 5", "Commit 1 / 3 · File 1 / 2") and the keys
of the commit review (`J K Files`, `N P Changes`, `⌥↑ ⌥↓ Commits` with several commits), `Esc Close` (Escape closes a sub-window).

#### Scenario: The footer of a review of several commits
- **WHEN** the review shows a part of several commits
- **THEN** the footer shows the position, as "All Changes · File 2 / 5" or "Commit 1 / 3 · File 1 / 2"
- **AND** the keys `J K Files`, `N P Changes`, `⌥↑ ⌥↓ Commits` and `Esc Close`

#### Scenario: Escape
- **WHEN** the user presses Escape in the window
- **THEN** the sub-window closes

### Requirement: Title bar, footer and actions: The actions are the items of the branch's menu

The actions MUST be the items of the branch's menu (`getBranchReviewItems`), their keys working in the whole window.

#### Scenario: The actions of each part
- **WHEN** the review window shows a part
- **THEN** its title bar holds the items of the menu of the target branch, their keys working in the whole window
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should build the actions of the title bar of each part from the window's own session, and open the pull request itself`)

### Requirement: Title bar, footer and actions: Open Pull Request is an action of a branch with one

`O Open Pull Request #42` MUST be an action of the title bar when the branch has a pull request: the item of its
menu, the number saying which one it opens.

#### Scenario: A branch with a pull request
- **WHEN** the target branch has a pull request
- **THEN** the title bar offers `O Open Pull Request #42`
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should build the actions of the title bar of each part from the window's own session, and open the pull request itself`)

### Requirement: Title bar, footer and actions: Create Pull Request is an action of a branch without one

`Create Pull Request…` MUST be an action of the title bar for a branch on GitHub without a live one, pushed or not (its
dialog opens in the window, [below](#the-create-pull-request-dialog), and the main window pushes the branch on Create or
"Continue on GitHub").

#### Scenario: A branch on GitHub without a live pull request
- **WHEN** the target branch is on GitHub without a live pull request, pushed or not
- **THEN** the title bar offers `Create Pull Request…`, whose dialog opens in the window
- **AND** the main window pushes the branch on Create or "Continue on GitHub"

### Requirement: Title bar, footer and actions: Safe Push is an action where Patronus is on

`S Safe Push` MUST be an action of the title bar where Patronus is on the repository and the branch has work.

#### Scenario: A branch with work on a repository of Patronus
- **WHEN** Patronus is on the repository and the target branch has work
- **THEN** the title bar offers `S Safe Push`
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should build the actions of the title bar of each part from the window's own session, and open the pull request itself`)

### Requirement: Title bar, footer and actions: The push is named as the push button of the row

The push MUST be an action of the title bar for a branch with work, named as the push button of its row (`Push Branch (2)`, `Push Stack (2 branches)`), without a key: P is the previous change.

#### Scenario: The push of a branch with work
- **WHEN** the target branch has work
- **THEN** the title bar offers its push, named as the push button of its row (`Push Branch (2)`, `Push Stack (2 branches)`)
- **AND** the push has no key: P is the previous change
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should build the actions of the title bar of each part from the window's own session, and open the pull request itself`)

### Requirement: Title bar, footer and actions: A first push of a landing stays disabled in its place

A first push whose landing pushes the branch (`isFirstPushInLanding`) is a step of Create Pull Request… and Safe Push:
the push MUST stay in its place, disabled with why ("Safe Push and Create Pull Request… push auth first: "⋯" pushes it
alone"), and so it is while Patronus or GitHub haven't answered whether they push it (`firstPushWait`), so their
answers move no button of the title bar.

#### Scenario: A first push the landing pushes, with Patronus
- **GIVEN** Patronus is on the repository
- **WHEN** the landing of the branch auth pushes it (`isFirstPushInLanding`)
- **THEN** the push stays in its place, disabled with why: "Safe Push and Create Pull Request… push auth first: "⋯" pushes it alone"

#### Scenario: A first push the landing pushes, without Patronus
- **GIVEN** Patronus is not on the repository
- **WHEN** the landing of the branch auth pushes it (`isFirstPushInLanding`)
- **THEN** the push stays in its place, disabled with why: "Create Pull Request… pushes auth first: "⋯" pushes it alone"

#### Scenario: Patronus or GitHub haven't answered
- **WHEN** Patronus or GitHub haven't answered whether they push the branch (`firstPushWait`)
- **THEN** the push stays in its place, disabled, and their answers move no button of the title bar

### Requirement: Title bar, footer and actions: The ⋯ of the title bar is the whole menu of the row

"⋯" after the actions MUST be the whole menu of the branch's row (`branchReviewMenu`, its items without their submenus,
the collapse of the stack and the review it is, without key hints): an item that opens a form of the row (a rename, an
amend) MUST run in the main window, at the row ([Review Window](../../review/review-window/spec.md#actions)); disabled
with the reason once the branch left.

#### Scenario: An item of the ⋯ that opens a form of the row
- **WHEN** the user chooses an item of the "⋯" of the title bar that opens a form of the row, as a rename or an amend
- **THEN** it runs in the main window, at the row
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should run an item of the branch's menu from the window's ⋯, in the main window`)

#### Scenario: The items of the ⋯
- **WHEN** the user opens the "⋯" of the title bar
- **THEN** it holds the items of the menu of the branch's row without their submenus, without the collapse of the stack and the review it is, without key hints
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should run an item of the branch's menu from the window's ⋯, in the main window`)

#### Scenario: The ⋯ of a branch that left
- **WHEN** the branch left the table of the session
- **THEN** the items of the "⋯" are disabled with the reason

### Requirement: Title bar, footer and actions: The whole stack has its actions in the title bar

The title bar of the review of the whole stack MUST hold the actions of the whole stack, from the one builder of its
menus (`getWholeStackMenuItems`, `getWholeStackReviewItems`), in the order of a branch's.

#### Scenario: The title bar of the review of the whole stack
- **WHEN** the window shows the review of the whole stack
- **THEN** its title bar holds the actions of the whole stack in the order of a branch's, from the one builder of its menus
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should start the Safe Push of the whole stack with S in its review, its popover at the header, and its pull request in the window's dialog`, `should create the pull request of the whole stack in its review, in its window, its Safe Push disabled until then`)

### Requirement: Title bar, footer and actions: The pull request of the whole stack opens or is created in the window

The title bar of the review of the whole stack MUST offer `O Open Pull Request #12` of its pull request (opened by the window) or "Create Pull Request of the Stack…" (its dialog in the window, the
context of the whole stack, `getStackPullRequestContext`; Create pushes its stack branch alone, then creates it, the
toast in the window).

#### Scenario: The whole stack without a pull request
- **WHEN** the user chooses "Create Pull Request of the Stack…" in the review of the whole stack
- **THEN** its dialog opens in the window, with the context of the whole stack (`getStackPullRequestContext`)
- **AND** its Create pushes the stack branch alone, then creates the pull request, the toast in the window
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should create the pull request of the whole stack in its review, in its window, its Safe Push disabled until then`)
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should start the Safe Push of the whole stack with S in its review, its popover at the header, and its pull request in the window's dialog`)

#### Scenario: The whole stack with a pull request
- **WHEN** the whole stack has a pull request
- **THEN** the title bar offers `O Open Pull Request #12`, opened by the window

### Requirement: Title bar, footer and actions: S starts the Safe Push of the whole stack at the header

The title bar of the review of the whole stack MUST offer `S Safe Push Whole Stack` (its popover at the header of the
island, else at the root's row; disabled with the reason of the next step of the whole stack).

#### Scenario: S in the review of the whole stack
- **WHEN** the user presses S in the review of the whole stack
- **THEN** the Safe Push popover of the whole stack opens at the header of the island, else at the root's row
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should start the Safe Push of the whole stack with S in its review, its popover at the header, and its pull request in the window's dialog`)

#### Scenario: A whole stack whose next step is no Safe Push
- **WHEN** the next step of the whole stack is no Safe Push, as before its pull request exists
- **THEN** `S Safe Push Whole Stack` is disabled with the reason of that step
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should create the pull request of the whole stack in its review, in its window, its Safe Push disabled until then`)

### Requirement: Title bar, footer and actions: The whole stack offers Push with Children of the root

The title bar of the review of the whole stack MUST offer Push with Children of the root, without a key, and its "⋯"
MUST be the whole menu of the whole stack but "Review Whole Stack".

#### Scenario: The push and the ⋯ of the whole stack
- **WHEN** the window shows the review of the whole stack
- **THEN** its title bar offers Push with Children of the root, without a key
- **AND** its "⋯" holds the whole menu of the whole stack but "Review Whole Stack"

### Requirement: Title bar, footer and actions: A start of the whole stack names it at the header

A start the main window runs at the header MUST name the whole stack (the subject `stack:<root>` of `review/run`);
while the window shows it, it MUST ask for the readiness of `stack:<root>`.

#### Scenario: A start the window asks of the main window
- **WHEN** the review of the whole stack asks the main window for a start
- **THEN** `review/run` names the subject `stack:<root>`, and the main window runs it at the header
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should start the Safe Push of the whole stack with S in its review, its popover at the header, and its pull request in the window's dialog`)

#### Scenario: The readiness of the whole stack
- **WHEN** the window shows the review of the whole stack
- **THEN** it asks for the readiness of `stack:<root>`

### Requirement: Title bar, footer and actions: The whole stack from the popover gets its reviewer groups

Opened from the Safe Push popover, the review of the whole stack MUST get the reviewer groups of the code-owner check of
the whole stack.

#### Scenario: Review Changes of the Safe Push popover of the whole stack
- **WHEN** the review of the whole stack opens from its Safe Push popover
- **THEN** it gets the reviewer groups of the code-owner check of the whole stack

### Requirement: Title bar, footer and actions: A commit of the review has the menu of its row

A commit of the review MUST have the menu of its row: its right-click in the column, on its row in the steps of the
tree after Show Commit Alone ([Review Tour](../review-tour/spec.md#in-the-review-window)), and "⋯" at the end of its
heading (a part of one commit has no column, its heading has it; open while the diffs of the commit load, it stays
open as the heading moves into the diffs), built when it opens, without key hints and without submenus.

#### Scenario: The right-click of a commit in the column
- **WHEN** the user right-clicks a commit in the column, or its row in the steps of the tree after Show Commit Alone, or "⋯" at the end of its heading
- **THEN** the menu of its row opens, built when it opens, without key hints and without submenus
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should run the menu of a commit of the review in the main window, its forms at the row, its copies in the window`)

#### Scenario: A part of one commit
- **WHEN** the part has one commit, and so no column
- **THEN** the "⋯" at the end of the heading of the commit has the menu of its row

#### Scenario: The ⋯ of the heading while the diffs load
- **WHEN** the "⋯" of the heading is open while the diffs of the commit load
- **THEN** it stays open as the heading moves into the diffs

### Requirement: Title bar, footer and actions: The menu of a commit holds the items of its row

The menu of a commit of the review MUST hold "Reword…", "Move to…", the way back ("Move to Unassigned", "Move Back to
…"), the squash into its target, "Push Up to Here", "Drop Commit…" (`getCommitMenuGroups`, the one builder of the menus
of commits, every item by a stable id).

#### Scenario: The items of the menu of a commit
- **WHEN** the menu of a commit of the review opens
- **THEN** it holds "Reword…", "Move to…", the way back, the squash into its target, "Push Up to Here" and "Drop Commit…", from `getCommitMenuGroups`
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should run the menu of a commit of the review in the main window, its forms at the row, its copies in the window`)

### Requirement: Title bar, footer and actions: The main window runs an item of a commit's menu at its row

The window MUST build the menu of a commit from its own session by the id of the commit in the virtual branch and ask
the main window to run its items (`review/run` with `commitIds`): it comes to the front, reveals the row, and a form
opens at the row (`requestCommitForm`: Move to…, the squash and the confirmation of the drop in the list, Reword… in
the preview of the commit), a push pushes.

#### Scenario: An item that opens a form
- **WHEN** the user chooses Move to…, the squash, "Drop Commit…" or Reword… in the menu of a commit of the review
- **THEN** the main window comes to the front, reveals the row, and the form opens at the row (`requestCommitForm`), Reword… in the preview of the commit
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should run the menu of a commit of the review in the main window, its forms at the row, its copies in the window`)

#### Scenario: Push Up to Here
- **WHEN** the user chooses "Push Up to Here" in the menu of a commit of the review
- **THEN** the main window pushes up to that commit
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should push up to a commit of the review from its menu in the main window`)

### Requirement: Title bar, footer and actions: The copies of a commit run in the window

The copies ("Copy Commit Hash", "Copy Commit Message", "Copy Changed Paths") MUST run in the window, of the commit as it
shows it.

#### Scenario: A copy of the menu of a commit
- **WHEN** the user chooses a copy in the menu of a commit of the review
- **THEN** it runs in the window, of the commit as the window shows it
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should run the menu of a commit of the review in the main window, its forms at the row, its copies in the window`)

### Requirement: Title bar, footer and actions: A commit of the whole stack has the copies only

The menu of a commit in the review of the whole stack MUST hold the copies only: "Review side-fix" leads to the menu.

#### Scenario: The menu of a commit of the whole stack
- **WHEN** the user opens the menu of a commit in the review of the whole stack
- **THEN** it holds the copies only, and "Review side-fix" leads to the menu

### Requirement: Title bar, footer and actions: The diff view is saved by the writer of the backend

Split/Unified/Inline MUST be the setting of every diff view, saved by the writer of the backend, which owns
`settings.json` ([Distributed Settings](../../../../docs/design/distributed-settings.md)): the window sends it the
change, and keeps the choice on its answer.

#### Scenario: The window chooses a diff view
- **WHEN** the user chooses Split, Unified or Inline in the window
- **THEN** the window sends the change to the writer of the backend, which saves it in `settings.json`
- **AND** the window keeps the choice on its answer
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should save the diff view the window chooses through the writer of the backend`)

### Requirement: Title bar, footer and actions: The target branch is the reviewed end of the part

The target branch MUST be the branch of the last selected segment, the reviewed end of the part: for "From Root", the
reviewed branch; for a parent alone, the parent.

#### Scenario: From Root and a parent alone
- **WHEN** the part is "From Root", or a parent alone
- **THEN** the actions are of the reviewed branch, or of the parent
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should build the actions of the title bar of each part from the window's own session, and open the pull request itself`)

### Requirement: Title bar, footer and actions: The window builds its actions from its own session

What the window offers it MUST build from its own session (`useReviewActions`), by the builders of the rows
(`getBranchReviewItems`, `branchReviewMenu`), again whenever a store they read changes (the branches, a sync or a
push, the pull requests, the robots, the operations of the session).

#### Scenario: The actions of each part
- **WHEN** the window shows a part
- **THEN** it builds the actions of the title bar from its own session, by the builders of the rows
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should build the actions of the title bar of each part from the window's own session, and open the pull request itself`)

#### Scenario: An operation of the session runs on the row
- **WHEN** an operation of the session runs on the row, as the Unstack the row runs
- **THEN** the window builds its menu again and shows it as running, and runs it once
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should show the Unstack the row runs as running in the window's menu, from the window's own session, and run it once`)

### Requirement: Title bar, footer and actions: An unavailable action stays, disabled with the reason

An unavailable Safe Push or Push Branch MUST stay, disabled, with the reason of the menu on hover ("A Safe Push of the
last commit is running", "Wait for the sync to finish"): one blocker for the menu, S on the row, the branch preview and
the review (the `safe-push` and `push` actions Rust decides, `domain_policy::actions`). The actions of a branch that
left MUST stay, disabled with why.

#### Scenario: A Safe Push or push that can't start
- **WHEN** the Safe Push or the push of the target branch can't start
- **THEN** it stays in the title bar, disabled, the reason of the menu on hover
- **AND** the reason is the one of the menu, S on the row and the branch preview: the `safe-push` and `push` actions Rust decides
  [@test] src/hooks/useBranchContextActions.test.ts (`says why a branch can't start a Safe Push in the words of its next step, and what runs on the row over it`)
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should build the actions of the title bar of each part from the window's own session, and open the pull request itself`)

#### Scenario: The actions of a branch that left
- **WHEN** the target branch left the table of the session
- **THEN** its actions stay, disabled with why

### Requirement: Title bar, footer and actions: O opens the pull request from the window

O (and its button) MUST open the pull request from the window.

#### Scenario: O in the window
- **WHEN** the user presses O, or clicks its button
- **THEN** the window opens the pull request itself
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should build the actions of the title bar of each part from the window's own session, and open the pull request itself`)

### Requirement: Title bar, footer and actions: Safe Push and the push run at the row of the main window

Safe Push (S) and Push Branch MUST open their popover or their question at the row: the main window runs them there
([Review Window](../../review/review-window/spec.md#actions)), switched to the repository of the review first, the
hover of each saying so meanwhile ("Switches the main window to app").

#### Scenario: S and Push Branch of the title bar
- **WHEN** the user presses S or Push Branch in the title bar
- **THEN** the main window, switched to the repository of the review first, opens their popover or their question at the row
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should run the Safe Push and the push of the title bar at the rows of the main window, switched to the repository of the review first`)

#### Scenario: The main window shows another repository
- **WHEN** the main window shows another repository than the review's
- **THEN** the hover of each says so ("Switches the main window to app")
- **AND** the main window switches to the repository of the review first
  [@test] src/lib/show-repository.test.ts (`switches the main window to another repository and waits for its first sync`, `says why a repository that is no recent one any more can't be shown`)
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should run the Safe Push and the push of the title bar at the rows of the main window, switched to the repository of the review first`)

### Requirement: The Create pull request dialog: The dialog opens in the window with its own context

"Create Pull Request…" MUST open the dialog of [Pull Requests](../pull-requests/spec.md#creating-a-pull-request) in
the window, the dialog of the main window (`CreatePullRequestDialog`, loaded on the first open), with the context the
window builds from its own session (the head and the base, the live pull request, the GitHub availability, the code
owners, whose readiness it asks for while it is open); the main window stays where it is.

#### Scenario: Create Pull Request… in the window
- **WHEN** the user chooses "Create Pull Request…" in the review window
- **THEN** the dialog of the main window (`CreatePullRequestDialog`) opens in the window, with the head and the base, the live pull request, the GitHub availability and the code owners of the window's own session
- **AND** the window asks for the readiness of the code owners while the dialog is open, and the main window stays where it is
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should create the pull request in the window, with the context of its own session`)

### Requirement: The Create pull request dialog: The keys of the review never see what is typed

The frame of the dialog MUST be outside the review, so the keys of the review never see what is typed; they MUST be
off while it is open.

#### Scenario: Typing into the dialog
- **GIVEN** the "Create pull request" dialog is open in the window
- **WHEN** the user types into it
- **THEN** the keys of the review are off and see nothing of what is typed
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should create the pull request in the window, with the context of its own session`)

### Requirement: The Create pull request dialog: Escape closes the dialog, not the window

Escape in the dialog MUST close the dialog, not the window.

#### Scenario: Escape in the dialog
- **WHEN** the user presses Escape in the dialog
- **THEN** the dialog closes and the window stays
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should create the pull request in the window, with the context of its own session`)

### Requirement: The Create pull request dialog: Create creates the pull request in the window

Create MUST create the pull request in the window, in the repository of the review, as the dialog of the main window
does (the push first, Activity, the toast with "Open Pull Request" in the window).

#### Scenario: Create in the window's dialog
- **WHEN** the user presses Create in the dialog of the window
- **THEN** the pull request is created in the repository of the review, the push first, as the dialog of the main window does
- **AND** Activity has it, and the toast with "Open Pull Request" shows in the window
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should create the pull request in the window, with the context of its own session`)

### Requirement: The Create pull request dialog: Safe Push Without Review opens its popover in the main window

"Safe Push Without Review…" MUST close the dialog and ask the main window, which comes to the front, reveals the row
and opens the Safe Push popover without review at its slot ([Patronus](../patronus/spec.md#safe-push-without-review)).

#### Scenario: Safe Push Without Review… of the window's dialog
- **WHEN** the user chooses "Safe Push Without Review…" in the dialog of the window
- **THEN** the dialog closes
- **AND** the main window comes to the front, reveals the row and opens the Safe Push popover without review at its slot
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should start the Safe Push without review the window's dialog offers in the main window`)

### Requirement: Code owners: The owner of a file is the last matching rule of the OWNERSHIP files

The owner of a file MUST be the last matching rule of the `OWNERSHIP` files of the baseline in the ancestor
directories of the file, ordered by the depth of the file, then its path, then the order in it.

#### Scenario: Rules of several OWNERSHIP files match
- **WHEN** rules of several `OWNERSHIP` files of the ancestor directories match a file
- **THEN** the owner is the last match, by the depth of the file, then its path, then the order in it
  [@test] crates/git-ops/src/ownership_test.rs (`the_last_match_wins_by_depth_then_file_then_order`)

#### Scenario: The OWNERSHIP files of the ancestor directories of a tree
- **WHEN** the owners of changed paths are read at the baseline
- **THEN** only the `OWNERSHIP` files of their ancestor directories are read, and each path gets the owner of the last matching rule
  [@test] crates/git-ops/src/ownership_test.rs (`reads_the_ownership_files_of_the_ancestor_directories_of_a_tree`)

### Requirement: Code owners: A pattern of an OWNERSHIP file has gitignore semantics

A `/pattern` line MUST be relative to its directory with gitignore semantics, `owner 'Name'` MUST close a section, and
a section without patterns MUST own `/**`.

#### Scenario: The patterns of a section
- **WHEN** a `/pattern` line of an `OWNERSHIP` file is matched against a path
- **THEN** it matches relative to its directory with gitignore semantics
  [@test] crates/git-ops/src/ownership_test.rs (`patterns_match_as_jgit_fast_ignore_rule_with_path_match_false`)

#### Scenario: A section closed by its owner, a section without patterns
- **WHEN** an `OWNERSHIP` file has `owner 'Name'` after its patterns, or a section without patterns
- **THEN** `owner 'Name'` closes the section of its patterns, and a section without patterns owns `/**`
  [@test] crates/git-ops/src/ownership_test.rs (`reads_the_ownership_files_of_the_ancestor_directories_of_a_tree`)

### Requirement: Code owners: The owners are read once per path in one batch

The owners MUST be read once per path for the window, in one `cat-file --batch`.

#### Scenario: The owners of the files of the window
- **WHEN** the window needs the owners of its files
- **THEN** the owners are read once per path, in one `cat-file --batch`

### Requirement: Code owners: A repository without OWNERSHIP files shows no owners

A repository without `OWNERSHIP` files MUST show no owners and no owner chips.

#### Scenario: No OWNERSHIP files
- **WHEN** the repository has no `OWNERSHIP` files
- **THEN** the review shows no owners and no owner chips

### Requirement: Code owners: The header of a file shows its owner

The header of a file MUST show its owner, muted; the tree row MUST keep its width for the name and show the owner in its
hover ("Owned by Design").

#### Scenario: A file with an owner
- **WHEN** the review shows a file with an owner
- **THEN** its header shows the owner, muted
- **AND** its tree row keeps its width for the name and shows the owner in its hover ("Owned by Design")

### Requirement: Code owners: The chips mark the groups, the owners and the files without one

The chips MUST be in the header of the files in the sidebar, above the tree they mark: "All", the reviewer groups the
code-owner check asks for (a warning shield; the number of their files in the part, all of them for a branch-wide
group, disabled when nobody tells which files), the other owners most files first, and "No Owner".

#### Scenario: The chips of a part
- **WHEN** the review shows the files of a part with owners and reviewer groups
- **THEN** the header of the files shows "All", the reviewer groups the code-owner check asks for with a warning shield and the number of their files, the other owners most files first, and "No Owner"
- **AND** a branch-wide group counts all the files, a group whose files nobody tells is disabled
  [@test] src/lib/review-owners.test.ts (`lists all files, the groups the check asks for, then the other owners and the files without one`)

### Requirement: Code owners: A file of a group matches by its path or its old path

A file of a group MUST match by its path or by the old path of a rename.

#### Scenario: A renamed file of a group
- **WHEN** a group names the old path of a renamed file
- **THEN** the file is a file of the group
  [@test] src/lib/review-owners.test.ts (`marks the files of a group (a rename by its old path), of an owner, or without an owner`)

### Requirement: Code owners: A chosen chip filters the files the review shows

A chosen chip MUST filter the files by their current or old path: a `ReviewFilter` holds the tree, the diffs, the file
count and J/K/N/P to the same shown files. The text search intersects that set; the selection and collapsed
directories follow the same rules as search.

#### Scenario: The chip of an owner
- **WHEN** the user chooses the chip of an owner
- **THEN** the tree, the diffs, the file count and J/K/N/P hold to the same shown files of the chip, by their current or old path
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should filter tree, diffs and navigation to the files of an owner chip`)

#### Scenario: A text search under a chip
- **WHEN** the user searches the files while a chip is chosen
- **THEN** the search intersects the files of the chip, and the selection and collapsed directories follow the same rules as search

### Requirement: Code owners: A chip hides the steps and groups it empties

Under a chosen chip, empty steps and groups MUST disappear. A partial step MUST say "4 of 7" on its row, with the
accessible description "4 of 7 files, Go Engineering"; the files header says "Files 4 of 22" and offers "Show All
Files".

#### Scenario: The steps of the story under an owner chip
- **WHEN** the user chooses an owner chip in a review with the steps of its plan
- **THEN** only the steps and files of the chip show, and the keys navigate within its story
- **AND** a partial step says "4 of 7" on its row, its accessible description "4 of 7 files, Go Engineering"
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should show only the steps and files of an owner chip, and navigate within its story`)
  [@test] src/presenters/review-steps.test.ts (`counts the files of the chosen owner and all files each step shows`)

#### Scenario: The files header under a chip
- **WHEN** a chip narrows the range
- **THEN** the files header says "Files 4 of 22" and offers "Show All Files"
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should filter tree, diffs and navigation to the files of an owner chip`)

### Requirement: Code owners: A chip whose paths are empty does not narrow the range

A chip whose paths are empty MUST NOT narrow the range: its note says "No files of Design in this range" and the files
remain visible.

#### Scenario: The chip of an owner without files in the range
- **WHEN** the user chooses a chip whose paths in the range are empty
- **THEN** its note says "No files of Design in this range" and the files remain visible
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should keep the chip of an owner with no files in the range and show no chips when one owner has them all`)

### Requirement: Code owners: A link to a file the chip hides clears the chip

A link from a mark or the overview to a file hidden by the chip MUST clear the chip and show the file; it MUST also
clear the text search only when that hides the target.

#### Scenario: A linked file of another owner
- **WHEN** the user follows a link from a mark or the overview to a file the chip hides
- **THEN** the chip clears and the file shows
- **AND** a text search that hides the target clears too
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should reveal a linked file from another owner by clearing its chip and clear a search that hides the target`)

#### Scenario: An overview file under a text search
- **WHEN** the user follows a link from the overview to a file the chip keeps, under a text search
- **THEN** the file shows and the owner chip stays
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should reveal an overview file under a text search while retaining its owner chip`)

### Requirement: Code owners: A long card names the next file of the shown set

A long card MUST name the next file in the shown set, the one J shows ([Commit Review](../../commit/commit-review/spec.md#requirements)).

#### Scenario: A long card under an owner chip
- **GIVEN** an owner chip is chosen
- **WHEN** the review shows a long card
- **THEN** the card names the next file of the chip's shown set, the one J shows
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should navigate the long card to the next visible file of the chosen owner chip`)

### Requirement: Code owners: A chosen chip stays among the chips while it has no files

While a chip is chosen, the chips MUST show, its own too when the shown changes have none of its files ("Design 0"), so
it can be turned off; an empty path set does not filter, the keys move as without it, and the header of the files says
"No files of Design in this range" ("Every file in this range has an owner" for No Owner).

#### Scenario: The chosen chip has no files in the shown changes
- **WHEN** the shown changes have none of the files of the chosen chip
- **THEN** the chips show, its own too ("Design 0"), so it can be turned off
- **AND** nothing is filtered, the keys move as without it, and the header of the files says "No files of Design in this range"
  [@test] src/lib/review-owners.test.ts (`keeps the chosen chip when the files have none of its own, so it can be turned off`, `names the chosen owner, and says when it has no files in the part`)
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should keep the chip of an owner with no files in the range and show no chips when one owner has them all`)

#### Scenario: No Owner without files
- **WHEN** No Owner is chosen and every file of the shown changes has an owner
- **THEN** the header of the files says "Every file in this range has an owner"
  [@test] src/lib/review-owners.test.ts (`names the chosen owner, and says when it has no files in the part`)

### Requirement: Code owners: One owner of every file shows no chips

With All chosen there MUST be no chips when one owner (or none) has every file and no group asks for an approval: the
header of the files says "1 file · Auth Team".

#### Scenario: One owner has every file
- **GIVEN** All is chosen
- **WHEN** one owner, or none, has every file and no group asks for an approval
- **THEN** there are no chips, and the header of the files says "1 file · Auth Team"
  [@test] src/lib/review-owners.test.ts (`is just All when one owner has every file, unless a group asks for an approval`)
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should keep the chip of an owner with no files in the range and show no chips when one owner has them all`)

#### Scenario: A group asks for an approval
- **WHEN** one owner has every file but a group asks for an approval
- **THEN** the chips show
  [@test] src/lib/review-owners.test.ts (`is just All when one owner has every file, unless a group asks for an approval`)

### Requirement: Code owners: The footer names the owner of the chosen chip

While a chip is chosen, the footer MUST name its owner after the position of the shown file, without repeating the
owner's count: "All Changes · File 3 / 12 · Design". A chip at 0, a group whose files nobody tells and All add no owner
name. The file count is the same filtered set the tree and diffs show.

#### Scenario: The footer under an owner chip
- **WHEN** the chip of an owner is chosen
- **THEN** the footer says "All Changes · File 3 / 12 · Design", its count the filtered set the tree and diffs show, without repeating the owner's count
  [@test] src/lib/review-position.test.ts (`names the chosen owner chip without duplicating the file count`)
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should filter tree, diffs and navigation to the files of an owner chip`, `should review a stacked branch on its parent, then the whole stack, by owner`)

#### Scenario: The footer under a branch-wide group
- **WHEN** the chip of a branch-wide group is chosen
- **THEN** every file is in its filtered set, and the footer names the group after a count of all the files
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should keep every file visible for a branch-wide group and count it in Approve`)
  [@test] src/lib/review-owners.test.ts (`never opens on a group whose files Patronus doesn't tell, and opens on a branch-wide one`)

#### Scenario: A chip at 0, a group whose files nobody tells, All
- **WHEN** the chosen chip is at 0, or is a group whose files nobody tells, or is All
- **THEN** the footer names no owner
  [@test] src/lib/review-position.test.ts (`names the chosen owner chip without duplicating the file count`)

### Requirement: Code owners: The chips are the same in the review of a pull request

The chips, their marks and their keys MUST be the same in the review window of a pull request
([Review Requests](../../repository/review-requests/spec.md#owner-chips)): one header of the files and one reading of
the owners for both windows.

#### Scenario: The review window of a pull request
- **WHEN** the review window of a pull request shows its files
- **THEN** it has the same chips, marks and keys, from the one header of the files and the one reading of the owners

### Requirement: Code owners: A chosen chip is remembered for the repository

A chip the user chooses, in this window or in the window of a pull request, MUST be remembered for the repository
(`reviewOwnerChip` of its recent project, through the writer of the settings); All forgets it. A chip is not a state
of the review: nothing else is remembered, and the approval of a pull request stays the reviewer's mark.

#### Scenario: The user chooses a chip, then All
- **WHEN** the user chooses a chip in a review window
- **THEN** it is remembered for the repository as `reviewOwnerChip` of its recent project, through the writer of the settings
- **AND** a later choice of All forgets it
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should open on the user's group that needs an approval, then on the chip the user chose, which All forgets`)

#### Scenario: Nothing else of the review
- **WHEN** the user leaves a review
- **THEN** nothing of it but the chip is remembered, and the approval of a pull request stays the reviewer's mark

### Requirement: Code owners: The window opens on the remembered chip where it marks files

The window MUST open on the remembered chip when it marks at least one file of the shown changes, else on All; a chip
the open names (Review Changes of a group of the Safe Push popover) wins.

#### Scenario: A remembered chip that marks files, one that marks none
- **GIVEN** a chip chosen last in a review window of the repository
- **WHEN** the window opens on changes where it marks at least one file
- **THEN** it opens on that chip, and on All where it marks none
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should open on the owner chip last chosen in a review window of the repository where it marks files`, `should open on All where the owner chip last chosen marks no file`)
  [@test] src/lib/review-owners.test.ts (`opens on the remembered chip where it marks files, else the user's group that still needs an approval, else All`)

#### Scenario: A chip the open names
- **WHEN** the open names a chip, as Review Changes of a group of the Safe Push popover
- **THEN** that chip wins over the remembered one

### Requirement: Code owners: A remembered owner and a group of its name are one chip

A remembered owner MUST match a group of the same name and the other way round; a group whose files nobody tells MUST
mark none.

#### Scenario: A group whose files nobody tells
- **WHEN** the remembered chip is a group whose files nobody tells
- **THEN** it marks no file, and the window does not open on it
  [@test] src/lib/review-owners.test.ts (`never opens on a group whose files Patronus doesn't tell, and opens on a branch-wide one`)

#### Scenario: The groups come after an owner chip of their name
- **GIVEN** an owner chip of the `OWNERSHIP` files holds before the reviewer groups come, remembered as the group or chosen as the owner
- **WHEN** the reviewer groups come, one of them of the owner's name
- **THEN** the chip is the group of its name: one chip, pressed, marking the group's files
  [@test] src/lib/review-owners.test.ts (`makes an owner chip held before the groups came the group of its name: one chip, pressed, with the group's files`)

### Requirement: Code owners: The first chip other than All holds until the user chooses

Until the user chooses, the first chip other than All MUST hold: the owners and the groups that come later never move
it.

#### Scenario: Owners and groups that come after the chip the window opened on
- **GIVEN** the window opened on a chip other than All, and the user chose none
- **WHEN** more owners or reviewer groups come
- **THEN** the chip stays

### Requirement: The files of each reviewer group (Safe Push popover): Each group names its files

Each group of "Needs review by the code owners", the line of the popover for a repository only on Space whose merge
request needs approvals (`safe-push-review-groups`), MUST name its files: "· 2 files", "· all files" for a group that
reviews every change, nothing for a group whose files nobody tells.

#### Scenario: The groups of the review line
- **WHEN** the Safe Push popover shows "Needs review by the code owners" for a repository only on Space whose merge request needs approvals
- **THEN** each group of `safe-push-review-groups` names its files: "· 2 files", or "· all files" for a group that reviews every change
- **AND** a group whose files nobody tells names nothing
  [@test] src/lib/patronus.test.ts (`counts the files of a group, all of them for a branch-wide one, nothing when nobody tells`)

### Requirement: The files of each reviewer group (Safe Push popover): Review Changes reviews the groups' files

"Review Changes" of that line, and of the line of the approvals a profile skips ("Starts without the approvals of
…"), MUST review the branch over the popover on the whole stack, the groups as chips (`toReviewGroups`).

#### Scenario: Review Changes of the review line
- **WHEN** the user clicks "Review Changes" of the line of "Needs review by the code owners", or of the line "Starts without the approvals of …"
- **THEN** the branch over the popover opens in its review, on the whole stack, the groups as chips (`toReviewGroups`)
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should say on Space that the approvals count in the GitHub pull request, and open Settings at its repository`)

### Requirement: The files of each reviewer group (Safe Push popover): D reviews with the groups of the last check

D on the row and "Review Changes" of the branch menu MUST review the branch with the groups of its last code-owner
check at its current head (`rememberReviewGroups`).

#### Scenario: D on the row of a branch a check answered for
- **GIVEN** a code-owner check of the branch answered at its current head
- **WHEN** the user presses D on the row, or chooses "Review Changes" of the branch menu
- **THEN** the review opens with the groups of that check (`rememberReviewGroups`)

### Requirement: The files of each reviewer group (Safe Push popover): A check that answers marks the open window

A check that answers while the window is open MUST mark its groups there too: the window's session answers it
(`knownReviewGroupsOf`, null while none answered, when the groups of the open stay).

#### Scenario: The check answers while the window is open
- **GIVEN** the review window of a branch is open before its code-owner check answers
- **WHEN** the check answers
- **THEN** the window marks the groups that need approval, from its own session (`knownReviewGroupsOf`)
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should mark the groups that need approval as the code-owner check answers while the window is open`)

#### Scenario: No check answered yet
- **WHEN** no check answered while the window is open (`knownReviewGroupsOf` is null)
- **THEN** the groups of the open stay
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should mark the groups that need approval as the code-owner check answers while the window is open`)

### Requirement: The files of each reviewer group (Safe Push popover): The files come from Patronus, else the owners

The files MUST come from Patronus when it sends them (`paths`, `branchWide` of a reviewer group); otherwise a group
gets the changed paths whose owner by the `OWNERSHIP` files has its name (trimmed, case-insensitive).

#### Scenario: Groups Patronus sent with and without their paths
- **WHEN** Patronus sends a reviewer group with its `paths` or `branchWide`, and another without them
- **THEN** the first keeps the files of Patronus
- **AND** the other gets the changed paths whose owner by the `OWNERSHIP` files has its name, trimmed, case-insensitive
  [@test] crates/sync-core/src/patronus_test.rs (`the_ownership_files_give_the_paths_to_the_groups_patronus_sent_without_them`)

### Requirement: The files of each reviewer group (Safe Push popover): A group only Patronus knows stays without files

A delegated review or other reviewers named by `review-rules.yaml` are known only to Patronus: such a group MUST stay
without files.

#### Scenario: A reviewer group of another owner
- **WHEN** a group is a delegated review, or other reviewers named by `review-rules.yaml`, and Patronus sends no paths of it
- **THEN** the group stays without files
  [@test] crates/sync-core/src/patronus_test.rs (`the_ownership_files_give_the_paths_to_the_groups_patronus_sent_without_them`)

## User Experience

- The window (`WindowPage`) is the title bar on the canvas, the review card below it (the commit review: its sidebar
  with the commits, the chips and the tree, beside the diffs), and the footer below them on the canvas: the
  `PreviewDialogFooter` of the previews without its rule (`border-t-0 px-1 py-0`), muted `text-xs`, "File 2 / 5" on
  the left, `KeyHint`s on the right. The actions are `ActionButton`s: ghost `sm`, the icon (`GitPullRequestIcon`,
  `GitPullRequestCreateIcon`, `ShieldCheckIcon`, `UploadIcon`), the label and the key; in a title bar narrower than
  48rem only the icon, the label in the hover. A disabled action is dimmed (`opacity-50`) and stays focusable for its
  tooltip.
- The robot after the stack bar (`ReviewRobotStatus`) is the `RunningRowRobot` of the Status cell of a row: `text-sm`,
  the ring `size-3.5`, the line `h-0.5` under it, its colour `destructive` for one that will fail. It fills the room the
  stack bar leaves, a `@container`: how long it runs from 5rem, the time left from 10rem, or from 20rem beside "lands
  with session", which shows from 11rem in `text-muted-foreground`, cut at its end; its hover is the popover of the
  chip of a robot (`patronus-popover`).
- A segment is an outline `xs` button, the selected ones `border-primary/50 bg-primary/10 text-primary`. A chip is a
  `FilterChip` of `FilterChips`, as the state chips of the branch filter: the stock `ToggleGroup`, the chosen one in
  its pressed state (`aria-pressed`), the count after the name in tabular numbers at 70% opacity. Above the chips, from 8 files on,
  is the search of the files ([Commit Review](../../commit/commit-review/spec.md#the-search-of-the-files)); it intersects
  the chip's files. The header shows "Files 4 of 22" while a chip narrows the range, and "Show All Files" returns to All.
  The row of a step uses two lines: its number, title and icon first, its owner count, New/Rewritten, branch and
  `+662 −58` below; ordinary titles wrap at words, only a code title breaks inside a word. The step row sticks to the
  top of the tree's scroll area on an opaque `bg-canvas`. The active step's whole group has a soft tint and an accent
  line at its indent. Its owner badge is a `secondary` `Badge` in `text-muted-foreground` (`review-owner-mark`), with
  "4 of 7" visible and "4 of 7 files, Go Engineering" in its `aria-description`; the note of a chip at 0 is a line of
  the header (`branch-review-owner-empty`). A long tree filename truncates in the middle, keeping its extension visible;
  hover and Copy Path keep the full path. "From Root" and "Whole Stack" are a `ToggleGroup` (`outline`, `sm`, `spacing={0}`), the shown scope
  pressed (`aria-pressed`). A Whole Stack that can't open is announced as unavailable (`aria-disabled`), dimmed
  (`opacity-50`) and takes no press, its reason in its tooltip, whose trigger is the item itself, so the group keeps
  its joined edges.
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should keep an unavailable Whole Stack in the scopes, announced as unavailable, its reason on hover, a press changing nothing`)
  "From Root" has a tooltip that sizes the part as the branch table does: "auth to login
  on origin/master: 2 branches · 3 commits, what the Safe Push of login lands · Shift+click a branch to extend". A
  commit of the column is its subject, truncated, and its muted `font-mono` hash on hover
  (`range-review-commit-hash`); a commit row of the tree its subject after its `GitCommitHorizontalIcon`
  (`review-commit-subject`), which gets the width: no role word, its indent under the step says it and its
  description (`aria-description`) words it, its hash in the title of the subject; its "New"/"Rewritten" an `info`
  `Badge` (`review-commit-since`), "Your review" of a pull request an `outline` `Badge` (`review-commit-review`), its
  branch muted (`review-commit-branch`); the row of a step of the commits, its commit, the same facts after its
  title, and its hash in `font-mono` in place of its line counts on hover (`review-commit-hash`), so the title keeps
  its width. A
  group chip starts with `ShieldAlertIcon` in `text-warning`; its tooltip says
  "Needs approval for a Safe Push".
- In the Safe Push popover the groups are a list below the review line, a group per line in the foreground colour,
  indented to its text: "Air Plugin Release Team (suggested: Alice) · 2 files".

## Testing / Local Run

- `cargo nextest run`: `git-ops` (`ownership_test`), `sync-core` (`review_target_test`, `patronus_test`).
- `pnpm test:unit`: `src/lib/branch-review.test.ts`, `src/lib/review-owners.test.ts`, `src/lib/patronus.test.ts`,
  `src/hooks/useBranchContextActions.test.ts`, `src/lib/review-reuse.test.ts`, `src/model/store.test.ts` (two
  windows on the fake sessions).
- `pnpm e2e`: `tests/e2e/branch-review/branch-review.spec.ts` on the `stacked_ownership` template, and the code-owner
  review of `tests/e2e/patronus/patronus-actions.spec.ts`. The mock records `open_sub_window`; the test then shows the
  route in the same page with the recorded data (`openSubWindowPage`), labelled `main`. A test of what both windows
  do opens the window in a page of its own (`openLinkedSubWindow`, labelled `review`), which subscribes to the session
  of the E2E server on its own stream: a sync of the main window reaches it as its patches, and its `review/run`
  reaches the main window's page.

## Open Questions / Risks

- Until Patronus sends `paths` per group, a group named in `review-rules.yaml` as the reviewer of another owner, or
  one Patronus delegates to, shows no files.
- A form of a row needs the main window: it switches to the repository of the review to open it.
- The tips are those of the app's session; a `brd` sync in a terminal moves the virtual branches without telling the
  window until the app syncs.
- A change of the branch prefix while the window is open leaves it on the session of the old prefix: it follows no
  sync until it opens again.

## References

- [Commit Review](../../commit/commit-review/spec.md)
- [Patronus](../patronus/spec.md) ("Code-owner review")
- [Stacked Branches](../stacked-branches/spec.md)
- [Git Operations](../../../../docs/design/git-operations.md) ("Branch review", "Code owners")
