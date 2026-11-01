---
name: Branch Review
description: Requirements for the review of a branch in its window: its diff from its parent or from the baseline, the parts of its stack, its commits, and its files by code owner, with the files each reviewer group of the Patronus code-owner check has to review.
style: plain-1
targets:
  - ../../src/pages/ReviewPage.tsx
  - ../../src/components/review/BranchReview.tsx
  - ../../src/hooks/useBranchReview.ts
  - ../../src/components/branch/BranchPreviewActions.tsx
  - ../../src/stores/branch-preview.ts
  - ../../src/components/review/BranchReviewStackBar.tsx
  - ../../src/components/ui/toggle-group.tsx
  - ../../src/components/shared/FilterChips.tsx
  - ../../src/components/shared/ActionButton.tsx
  - ../../src/components/shared/WindowPage.tsx
  - ../../src/lib/branch-review.ts
  - ../../src/lib/review-window.ts
  - ../../src/hooks/usePreviewHotkeys.ts
  - ../../crates/app-api/src/remote/review.rs
  - ../../src/lib/review-owners.ts
  - ../../src/components/review/ReviewOwnerChips.tsx
  - ../../src/hooks/useFileOwners.ts
  - ../../src/hooks/useReviewOwnerFilter.ts
  - ../../src/stores/settings/repositorySettings.ts
  - ../../src/lib/range-review.ts
  - ../../src/components/review/RangeReview.tsx
  - ../../src/hooks/useBranchReviewLink.ts
  - ../../src/hooks/useMainWindowLink.ts
  - ../../src/lib/sub-window-link.ts
  - ../../src/model/store.ts
  - ../../src/pages/IndexPage.tsx
  - ../../src/hooks/useBranchContextActions.ts
  - ../../src/hooks/useBranchRowKeys.ts
  - ../../src/hooks/useCommitMoveList.ts
  - ../../src/hooks/useCommitCopyActions.ts
  - ../../src/stores/commit-move.ts
  - ../../src/components/branch/BranchPreviewDialog.tsx
  - ../../src/components/repository/ReviewRequestRow.tsx
  - ../../src/components/branch/SafePushPopover.tsx
  - ../../src/components/branch/StackIslandCard.tsx
  - ../../src/hooks/useWholeStackActions.ts
  - ../../src/stores/repository.ts
  - ../../src/types/sub-window.ts
  - ../../src/lib/pull-request-dialog.ts
  - ../../src/lib/pull-request-run.ts
  - ../../src/components/review/WindowCreatePullRequestDialog.tsx
  - ../../src/hooks/useCreatePullRequestContext.ts
  - ../../src/components/branch/CreatePullRequestDialog.tsx
  - ../../src/lib/sub-window.ts
  - ../../src/router.ts
  - ../../src-tauri/capabilities/sub-windows.json
  - ../../src-tauri/capabilities/desktop.json
  - ../../crates/sync-core/src/review_target.rs
  - ../../crates/git-ops/src/ownership.rs
  - ../../crates/app-api/src/reads/commits.rs
---

# Branch Review

Status: Implemented
Date: 2026-10-15

## Summary

"Review Changes" (D) opens the changes of a branch in a window of its own, the only review of a branch: what the
branch adds on top of its parent, the diff of all its files in the [commit review](../commit/commit-review.spec.md),
its commits above the files in the sidebar (`BranchReview`); the review of a pull request has the same commits
([Review Requests](../repository/review-requests.spec.md#review-window)). A review is a task of minutes beside the
branch table: the window stays beside it (the Safe Push popover too), where a dialog covered it. A
stacked branch shows its stack from the baseline as a bar of segments, one per branch; a click shows one branch on its
parent, Shift+click extends the part, "From Root" shows everything from the baseline to the branch, root..branch, which
is what its Safe Push lands and Patronus checks; "Whole Stack" after it goes up to every branch of the stack. The whole stack, every branch of a stack that forks
([Landing](../../docs/design/landing.md)), is reviewed in the same window as its stack branch: "Review Whole Stack";
each of its commits names its branch, and the segments of the chain it came from go back down.
The files are labelled with their code owners by the `OWNERSHIP` files of the baseline, and chips mark them by the
reviewer groups the code-owner check of a Safe Push asks for and by owner: the story keeps every file, the others are
muted, and the keys move through the chosen owner's files. The Safe Push popover names the files each
group has to review, and its "Review Changes" opens the review on them. The title bar of the window holds the stack, the diff view and the
actions on the branch: Open Pull Request, Safe Push (Push to master or Merge #7 in its place, by the route of the
repository, [Landing Routes](landing-routes.spec.md)) and Push Branch, which the main window runs; the footer holds the
keys.

## Goals

- Look at what a branch changes before a push, a Safe Push or a pull request, without leaving the app.
- See a stacked branch alone and with its parents: a reviewer reads the child on its parent, Patronus checks all.
- Know which files need whose approval, and look at exactly those, with the declarations and uses around them.

## Non-goals

- No comments and no approvals: the review happens in the pull request.
- No branch state in the window: the main window owns the branches and runs the actions the window asks for; the
  "Create pull request" dialog of the window shows what the main window sends and asks it to create the pull request.
- No review of the children of a branch in its window: each has its own, with this branch as a segment; "Whole Stack"
  reviews every branch of the stack, the children too.
- No group membership: "my files" needs the members of a group, which the repository doesn't hold.

## Requirements

### Opening

- "Review Changes" of the branch menu (key D, its only review item), D on a branch row, "Review Changes" of the Safe
  Push popover (on its line of the approvals a repository only on Space needs, and on its line of the approvals a
  profile skips) and D or the footer button of the branch preview must open the review window of the branch
  (`openBranchReviewWindow`); there is no review dialog, no W and no "Open in Window". The preview closes once the
  window opened, and stays (the error told) when it could not; the Safe Push popover stays open in the main window. It
  is disabled, with the reason, for a branch without commits, one that failed to sync and a blocked one (its virtual
  branch is not built on its parent).
  [@test] ../../tests/e2e/branch-review/branch-review.spec.ts (`should open the review of a branch in its window from its row, its menu and its preview`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should say on Space that the approvals count in the GitHub pull request, and open Settings at its repository`)
  [@test] ../../src/lib/branch-review.test.ts (`says why Rust refuses the review of a branch, in the words of its row`)
  [@test] ../../crates/domain-policy/src/actions_test.rs (`the_review_the_pull_request_actions_and_the_rarer_starts_follow_their_facts`)
- "Review Whole Stack" (the "⋯" of a stack island and "Whole Stack ›" of the menu of every branch of the stack) must open the same window for
  the whole stack of a stack that forks (`openWholeStackReviewWindow`, titled "Review the whole air stack"): one part,
  its stack branch `{prefix}/stack/<root>` on the baseline (the target `wholeStack`, `ReviewTarget::WholeStack`), every commit of
  the stack in the order of the history, its stack bar the main line and "Whole Stack" ([below](#the-stack-and-the-commits));
  read again when a sync moves the stack branch. Its
  title bar holds the actions of the whole stack ([below](#title-bar-footer-and-actions)). It is disabled, with the
  reason, while the stack branch has no tip. Of a straight chain, which has no stack branch, it opens the review of its
  last branch from the root (`range: "all"`).
  [@test] ../../tests/e2e/stack-island/whole-stack.spec.ts (`should review the whole stack in the window of its stack branch, from the island's menu, the root's menu and the menu of a branch of it`)
  [@test] ../../tests/e2e/stack-island/stack-island.spec.ts (`should keep the popover of a row of the island at its slot, and review the whole stack in the window of its stack branch`)
  [@test] ../../crates/sync-core/src/review_target_test.rs (`the_whole_stack_is_one_part_of_its_stack_branch`)
- D on a commit row of a branch, the "Review Changes" of its menu and the D, the footer button and Enter in the
  Files tree of its preview must open the window at the commit (`reviewBranchCommit`, `focus.commitId` the copy of
  the commit in the virtual branch), its branch's own part, titled "Review <branch> · a1b2c3d"
  ([Commit Review](../commit/commit-review.spec.md#opening-and-closing): a commit the branch review can't show opens
  the review of its list).
  [@test] ../../tests/e2e/commit-files/commit-review.spec.ts (`should review a commit of a stacked branch with its stack and its actions, and move to a commit of the chain`)
- The "Review Changes" button of each commit of the branch preview must open the window at that commit, and the
  preview closes once the window opened; D on a row of Your Turn's own pull requests with changes requested opens the
  review of its branch ([Review Requests](../repository/review-requests.spec.md)).
  [@test] ../../tests/e2e/branch-review/branch-review.spec.ts (`should review a commit of the branch preview in the review of the branch, at the commit`)
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should list the own pull requests a reviewer handed back after the requests, show the branch and review it`)
- The window is the one review window, titled "Review <branch>" ([Review Window](../review/review-window.spec.md#one-window):
  how an open goes on from the review on screen, Back; [Title Bar](../repository/title-bar.spec.md) for how it
  opens). An open that would start it over while its "Create pull request" dialog is open is refused with "Cannot
  review login" and the reason, an `info` toast: the dialog holds what the user typed. It reads the repository it was
  opened for, not the one selected later in the main window (`getDiffRepositoryPath`).
- A move to a commit of the chain on screen (`move` of the window data) keeps the part when it has the commit, else it
  is the commit's segment; the owner chip stays, it hides no file; the file of `path` shows. A move waits while the
  "Create pull request" dialog of the window is open, and the dialog keeps what was typed.
  [@test] ../../tests/e2e/branch-review/branch-review.spec.ts (`should keep the Create pull request dialog of the window while a commit is reviewed, and move to the commit once it closes`)
  [@test] ../../src/lib/branch-review.test.ts (`keeps the part that has the commit, else shows the commit's segment`, `shows all changes of the commit's branch when the segments don't have the commit`)
  [@test] ../../tests/e2e/commit-files/commit-review.spec.ts (`should review a commit of a stacked branch with its stack and its actions, and move to a commit of the chain`)
- The review gets the chain of the branch (its stack ancestors by `effectiveBase`, root first), the baseline, where
  to start (`BranchReviewFocus`: the branch itself, or the whole stack from the Safe Push popover and the island; the
  owner chip, the commit, the file), the reviewer groups of the last code-owner check of the branch at its current head.
- The app must read the review ahead as its window opens, beside the start of the window: the backend reads the
  segments and the files and the patch of the part the window shows first (the branch on its parent, or the whole
  stack) into its memory while the window starts, so its first read finds them read (`prefetch_review`). Once
  per state of the branch: its chain and their heads; an open of the branch the window shows reads nothing again. A
  hover or a focus of a branch row reads nothing: no IPC for a pointer that passes over the table.
  [@test] ../../tests/e2e/branch-review/branch-review.spec.ts (`should read the review of a branch ahead as its window opens, once per state, and nothing on a hover or a focus of its row`)
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should ask the session nothing for the hover and the menu of a row, and tell it only the details of the robot whose popover opens`)
- The window must start the read of its segments with its page, not after the chunk of its page loaded; its first read
  takes that read when it asks for the same review, a later one reads again.
  [@test] ../../src/lib/review-window-reads.test.ts (`gives the first read of the page the read its load started, once, and only for the same review`)
- In the window ↑/↓, Space and P are the review's (P is the previous change), never a push.
  [@test] ../../tests/e2e/branch-review/branch-review.spec.ts (`should open the review of a branch in its window from its row, its menu and its preview`)

### The stack and the commits

- The stack bar must show the baseline, then a segment per branch of the chain with its number of commits. The
  branch itself is selected at first. A click selects one segment, Shift+click extends the part to the clicked
  segment; "From Root" selects all of them (root..branch, not the whole stack). The diff is from the base of the first segment of the part (the head of
  its parent, the commit the root is built on) to the head of its last one.
  [@test] ../../src/lib/branch-review.test.ts (`diffs the base of the first segment to the head of the last one`, `selects one segment, or extends the range to the clicked one with Shift`)
  [@test] ../../tests/e2e/branch-review/branch-review.spec.ts (`should review a stacked branch on its parent, then the whole stack, by owner`)
- The stack bar must stay on one line, the title bar's, never running under its actions: the baseline and the segments
  outside the part give way first (cut with "…", the name in their hover), the selected segments and the last one
  last.
  [@test] ../../tests/e2e/repository/title-bar.spec.ts (`should start the row of a review window after the traffic lights, its empty parts dragging the window`)
- After "From Root", "Whole Stack" must go up to every branch of the stack of the chain's root while the stack has
  branches outside the chain (`WholeStackScope` of the window state, `wholeStackScopeOf`): one option set "Scope" of
  the two (the stock `ToggleGroup`, so they read as controls, not as two words of the title), one pressed at most; a
  press of the pressed one keeps it. Of a stack that forks it is the review of its stack branch, its hover "Every branch of the
  whole air stack, its side branches too, on origin/master: 7 branches · 14 commits, what its Safe Push lands" (what is
  left of it, `StackLanding.rest`), disabled with the reason while the stack branch is not built; of a straight chain
  it is the review of its last branch from the root ("IJPL-257337 to perf-part-2 on origin/master: 2 branches · 3
  commits, what the Safe Push of perf-part-2 lands"), and its last branch has none: From Root is the whole stack. The
  window asks the main window (`{ scope: "wholeStack" }`), which opens the review with the chain as the way back.
  [@test] ../../tests/e2e/branch-review/branch-review.spec.ts (`should move the review of a branch up to the whole stack, and down to a branch of it and to the branch of a commit`, `should offer the whole stack of a straight chain on a branch with branches built on it, as the last one from the root`)
- The stack bar of the review of the whole stack must show the chain it came from (`from` of the window data; the
  main line from the island and the menus), each segment with its number of commits, none pressed, then "Whole
  Stack", pressed; a segment reviews its branch (`{ review: "branch" }`). Each commit of the column (with the plan,
  each commit row of the tree) names its branch, muted after its subject, matched across the copies by author, author
  time and subject (`reviewCommitKey`: the stack branch has no mapping notes; `stackCommits` of the window state), and
  the heading of the shown commit ends with
  "Review side-fix", the review of its branch at the commit (`{ review: "commit" }`). With the plan a commit row of
  the tree of a part of several branches names its branch too, which the column heads by branch.
  [@test] ../../tests/e2e/branch-review/branch-review.spec.ts (`should move the review of a branch up to the whole stack, and down to a branch of it and to the branch of a commit`, `should go down from the review of the whole stack the island opens, along its main line`, `should name the branch of each commit row of the plan's tree, from the root and in the whole stack, and review the branch of a commit shown alone`)
- The stack bar of a stack of more than 6 branches must stay short: it shows the root, the ends of the selected
  part and the last two segments (the parent and the branch), and folds each run of two or more others into a ghost
  "… 14 more" button (in `text-primary` while the part covers it) with "14 branches · 120 commits · Click to show them"
  as its tooltip; a click shows every segment.
  [@test] ../../src/lib/branch-review.test.ts (`folds the middle of a long stack, keeping the root, the ends of the range and the last two segments`, `shows every segment of a short stack, or once unfolded`)
- The segments come from the virtual branches: one `for-each-ref` for the tips, one `git log --first-parent` from
  the baseline to the last tip, cut at the tips of the parents. A parent whose head is not below its child, or a
  branch without a virtual branch, is an error that asks for a sync.
  [@test] ../../crates/sync-core/src/review_target_test.rs (`splits_a_stack_at_the_heads_of_the_parents`, `a_branch_outside_a_stack_is_one_segment_from_the_baseline`, `a_child_not_built_on_the_head_of_its_parent_needs_a_sync`)
- The app must remember the segments by the tips and the baseline's refs that the `for-each-ref` reads (it lists the
  refs the baseline's name may stand for too), and run the `log` again only when a tip or the baseline moved.
  [@test] ../../crates/sync-core/src/review_target_test.rs (`a_memoised_review_runs_no_log_for_the_same_tips`, `a_moved_tip_or_baseline_is_read_again`)
- The commits (`RangeReview`, the ones of a pull request too) must be above the files in the sidebar of the commit
  review ([Commit Review](../commit/commit-review.spec.md#the-sidebar-and-the-diffs)): "All Changes" with the number
  of commits of the part, then its commits newest first, under the name of their branch when the part has several, the
  subject taking the width of the row and the hash only on hover (the heading of the shown commit has it); a commit shows
  only its own changes, its subject, author, time and message (three lines, "Show More") above them. ⌥↑/⌥↓ step
  through the commits from anywhere in the review, ↑/↓ inside them (one tab stop, a listbox), which keep the focus while
  the diffs of the next commit mount. A part of one commit has no list: it shows that commit. A sync that rebuilds the
  commits keeps the shown one by its branch, subject and author time.
  [@test] ../../src/lib/range-review.test.ts (`lists all changes, then the commits of the reviewed branch first, each branch headed by its name`, `keeps the commit by its id, else the one a sync rebuilt in its place`)
  [@test] ../../tests/e2e/branch-review/branch-review.spec.ts (`should review a stacked branch on its parent, then the whole stack, by owner`)
- Another commit, another owner chip or a sync that reads the stack again must keep the file on screen when the new
  changes have it; an owner chip keeps every file, the selection and the collapsed directories.
  [@test] ../../tests/e2e/branch-review/branch-review.spec.ts (`should review a stacked branch on its parent, then the whole stack, by owner`)
- The window must read the tips of its own subscription: it subscribes to the session of its repository itself
  (`ensureSession` of its own `src/model/store.ts`, `subscribe_view` under its label, beside the main window's
  subscription and independent of it), and takes from its view the head of each branch of the chain (the copy of its
  newest commit, else that commit), of the review of the whole stack the tip of its stack branch and the branch of
  each of its commits by `reviewCommitKey`, and the baseline (`settledReviewTable`). It takes them once the view
  settled: once per finished sync of the session, whichever window asked for it, also while the main window shows
  another repository, never during a run, and not before a sync grouped the branches. The window carries the version
  of its view (`data-view-version` of `review-window`); the link of the main window sends none of it. The read at the
  open uses the baseline of the window data, which the page starts at its load.
  [@test] ../../tests/e2e/branch-review/branch-review.spec.ts (`should read the stack again from the window's own session when a sync of the main window moves a tip of it, and keep a branch that left`)
  [@test] ../../src/model/store.test.ts (`keeps the subscription of a second window independent of the first's: its own view, its own count of the runs`)
  [@test] ../../src/lib/branch-review.test.ts (`reads the heads of the chain once the view settled, never from a running sync or a table no sync grouped`, `reads the tip of the stack branch of a whole stack and the branch of each of its commits`, `keeps the table of a view that changed nothing the review reads`)
- A sync that moves a tip of the chain must read the stack again, once per set of heads, and keep the part by the names
  of its ends (the reviewed branch alone when an end left the chain).
  [@test] ../../src/lib/branch-review.test.ts (`keeps the range by the names of its ends, or falls back to the reviewed branch`)
  [@test] ../../tests/e2e/branch-review/branch-review.spec.ts (`should read the stack again from the window's own session when a sync of the main window moves a tip of it, and keep a branch that left`)
- A branch of the chain no longer in the table of the window's session (it landed, it was renamed) must keep the last
  view with "login is no longer a branch: it landed or was renamed": an info `Alert` above the review in the window;
  its Safe Push and Push Branch are disabled with that reason, its pull request still opens, and the stack is not read again.
  [@test] ../../tests/e2e/branch-review/branch-review.spec.ts (`should read the stack again from the window's own session when a sync of the main window moves a tip of it, and keep a branch that left`)

### Title bar, footer and actions

- The title bar of the window ([Title Bar](../repository/title-bar.spec.md)) must show the stack bar, then at its other
  end the Split/Unified/Inline toggle and the actions on the target branch. The footer must show the position in the review
  ("All Changes · File 2 / 5", "Commit 1 / 3 · File 1 / 2") and the keys of the commit review (`J K Files`, `N P
  Changes`, `⌥↑ ⌥↓ Commits` with several commits), `Esc Close` (Escape closes a sub-window). The actions are the items
  of the branch's menu (`getBranchReviewItems`), their keys working in the whole window: `O Open Pull Request` when it has a pull request, `Create Pull Request…` for a
  branch on GitHub without a live one, pushed or not (its dialog opens in the window, [below](#the-create-pull-request-dialog), and the main window pushes the branch on Create or "Continue on GitHub"), `S Safe Push` where Patronus is on the repository and the branch has work, and the push for
  a branch with work, named as the push button of its row (`Push Branch (2)`, `Push Stack (2 branches)`), without a key: P is
  the previous change. A first push whose landing pushes the branch (`isFirstPushInLanding`) is a step of Create Pull
  Request… and Safe Push: the push stays in its place, disabled with why ("Safe Push and Create Pull Request… push auth
  first: "⋯" pushes it alone"), and so it is while Patronus or GitHub haven't answered whether they push it
  (`firstPushWait`), so their answers move no button of the title bar. "⋯" after them is the whole menu of the branch's row (`BranchReviewWindowState.menus`, its
  items without their submenus): the main window runs the item it asks for (`menuItemId`), comes to the front and
  shows the row (a rename or an amend is a form of the row); disabled with the reason while it shows another
  repository or the branch left.
  [@test] ../../tests/e2e/branch-review/branch-review.spec.ts (`should run an item of the branch's menu from the window's ⋯, in the main window`)
  [@test] ../../tests/e2e/branch-review/branch-review.spec.ts (`should act on the branch of the part from the title bar, through the main window`)
- The title bar of the review of the whole stack must hold the actions of the whole stack, from the one builder of
  its menus (`getWholeStackMenuItems`, `getWholeStackReviewItems`), in the order of a branch's: `O Open Pull Request
  #12` of its pull request (opened by the window) or "Create Pull Request of the Stack…" (its dialog in the window,
  the context of the whole stack, `getStackPullRequestContext`; Create pushes its stack branch alone, then creates
  it, the toast in the window), `S Safe Push Whole Stack` (its popover at the header of the island, else at the
  root's row; disabled with the reason of the next step of the whole stack) and Push with Children of the root,
  without a key; "⋯" is the whole menu of the whole stack but "Review Whole Stack". Every request says it is of the
  whole stack (`wholeStack` of `BranchReviewRequest`), and the main window runs the item of the whole stack of the
  root; while the window is open it asks for the readiness of `stack:<root>`. Opened from the Safe Push popover, the
  review gets the reviewer groups of the code-owner check of the whole stack.
  [@test] ../../tests/e2e/stack-island/whole-stack.spec.ts (`should start the Safe Push of the whole stack with S in its review, its popover at the header, and run ⋯ in the main window`, `should create the pull request of the whole stack in its review, in its window, its Safe Push disabled until then`)
- A commit of the review must have the menu of its row: its right-click in the column, on its row in the steps of the
  tree after Show Commit Alone ([Review Tour](review-tour.spec.md#in-the-review-window)), and "⋯" at the end of its
  heading (a part of one commit has no column, its heading has it), built when it opens, without key hints and
  without submenus: "Reword…", "Move to…", the way back ("Move to Unassigned", "Move Back to …"), the squash into its
  target, "Push Up to Here" (`getCommitMenuGroups`, the one builder of the menus of commits, every item by a stable
  id), which the main window sends by the id of the commit in the virtual branch (`commitMenus` of the window state)
  and runs (`commitMenuItemId`): it comes to the front, reveals the row, and a form opens at the row
  (`requestCommitForm`: Move to… and the squash in the list, Reword… in the preview of the commit). The copies
  ("Copy Commit Hash", "Copy Commit Message", "Copy Changed Paths") run in the window, of the commit as it shows it.
  The review of the whole stack has the copies only: "Review side-fix" leads to the menu.
  [@test] ../../tests/e2e/branch-review/branch-review.spec.ts (`should run the menu of a commit of the review in the main window, its forms at the row, its copies in the window`)
- Split/Unified/Inline must be the setting of every diff view, saved by the writer of the backend, which owns
  `settings.json` ([Distributed Settings](../../docs/design/distributed-settings.md)): the window sends it the change,
  and keeps the choice on its answer.
  [@test] ../../tests/e2e/branch-review/branch-review.spec.ts (`should save the diff view the window chooses through the writer of the backend`)
- The target branch is the branch of the last selected segment, the reviewed end of the part: for "From Root", the
  reviewed branch; for a parent alone, the parent.
  [@test] ../../tests/e2e/branch-review/branch-review.spec.ts (`should act on the branch of the part from the title bar, through the main window`)
- What the window offers comes from the main window, over the link of the window (`sub-window-link.ts`): the state of
  the window data at the open (`BranchReviewWindowState`: the items and the menus of each branch), then
  `branch-review/state` whenever it changes (the branches, a sync or a push, the pull requests, the robots, the inline
  action of a row, the selected repository), deduplicated, for the last opened chain. An unavailable Safe Push or
  Push Branch stays, disabled, with the reason of the menu on hover ("A Safe Push of the last commit is running", "Wait for
  the sync to finish"): one blocker for the menu, S on the row, the branch preview and the review
  (the `safe-push` and `push` actions Rust decides, `domain_policy::actions`).
  [@test] ../../tests/e2e/branch-review/branch-review.spec.ts (`should act on the branch of the part from the title bar, through the main window`)
  [@test] ../../src/hooks/useBranchContextActions.test.ts (`says why a branch can't start a Safe Push in the words of its next step, and what runs on the row over it`)
- While the main window shows another repository, the title bar keeps the last items, enabled: their hover says the
  consequence, "Switches the main window to app", and a click makes the main window select the window's repository,
  wait for its sync and run the action on its branches (`showRepository`); the user never selects it by hand. The pull
  request still opens from the window. A repository the main window no longer lists is refused with "app is no longer
  among the repositories of the main window".
  [@test] ../../tests/e2e/branch-review/branch-review.spec.ts (`should act on the branch of the part from the title bar, through the main window`)
  [@test] ../../src/lib/show-repository.test.ts (`switches the main window to another repository and waits for its first sync`, `says why a repository that is no recent one any more can't be shown`)
- O (and its button) opens the pull request from the window. Safe Push (S) and Push Branch only ask: the window sends
  `branch-review/request` (`{ id, repositoryPath, branchName, itemId: "safe-push" | "push" }`). The main window builds
  the item again and runs it as the row would: it comes to the front, reveals the row and asks its start
  (`requestSafePush`: it starts at once, or opens the popover at the row's slot, or says why in an `info` toast), or
  pushes as the push button of the row does (`pushOrConfirm`: the stack while a parent has work, a push that drops
  remote commits asks in the row). A request of another repository switches the main window to it first; one it can't
  run (the repository is gone from its list, the branch left, the item is disabled) is refused with the reason, shown in the window (`branch-review/feedback`: "Cannot push branch login", "Cannot
  start the Safe Push of login") and recorded in Activity; `branch-review/reply` answers each request by its id.
  [@test] ../../tests/e2e/branch-review/branch-review.spec.ts (`should act on the branch of the part from the title bar, through the main window`)

### The Create pull request dialog

- "Create Pull Request…" opens the dialog of [Pull Requests](pull-requests.spec.md#creating-a-pull-request) in the
  window (`WindowCreatePullRequestDialog`, loaded on the first open), the form of the main window's dialog
  (`CreatePullRequestForm`, its frame outside the review, so the keys of the
  review never see what is typed; they are off while it is open); the main window stays where it is. The window asks
  `{ dialog: "open" }` and `{ dialog: "close" }`: while it is open, the main window asks for the readiness of the branch
  and sends what the dialog shows with the state (`createPullRequest`, a `CreatePullRequestContext`: the head and the
  base, the live pull request, the GitHub availability, the code owners, "Safe Push Without Review…"); the dialog opens
  once it came, and keeps the last one while the main window shows another repository. The window reads the text of
  the branch and searches the users itself. Escape in the dialog closes the dialog, not the window.
  [@test] ../../tests/e2e/branch-review/branch-review.spec.ts (`should create the pull request in the window it was asked in, through the main window`)
- Create asks `{ dialog: "create", input }`: the main window creates the pull request as its own dialog does (the head
  and the base from its branches, Activity) and sends the toast to the window (`branch-review/feedback`, `kind:
  "toast"`, "Open Pull Request" opens it from the window); another repository shown or a branch that left refuse it
  ("Cannot create the pull request of auth"), as GitHub's error does. "Safe Push Without Review…" closes the dialog and
  asks `{ dialog: "safePushWithoutReview" }`: the main window comes to the front, reveals the row and opens the Safe
  Push popover without review at its slot ([Patronus](patronus.spec.md#safe-push-without-review)).
  [@test] ../../tests/e2e/branch-review/branch-review.spec.ts (`should create the pull request in the window it was asked in, through the main window`, `should start the Safe Push without review the window's dialog offers in the main window`)

## Code owners

- The owner of a file is the last matching rule of the `OWNERSHIP` files of the baseline in the ancestor directories
  of the file, ordered by the depth of the file, then its path, then the order in it; a `/pattern` line is relative to
  its directory with gitignore semantics, `owner 'Name'` closes a section, a section without patterns owns `/**`. The
  owners are read once per path for the window, in one `cat-file --batch`. A repository without `OWNERSHIP` files
  shows no owners and no owner chips.
  [@test] ../../crates/git-ops/src/ownership_test.rs (`patterns_match_as_jgit_fast_ignore_rule_with_path_match_false`, `the_last_match_wins_by_depth_then_file_then_order`, `reads_the_ownership_files_of_the_ancestor_directories_of_a_tree`)
- The header of a file must show its owner, muted; the tree row keeps its width for the name and shows the owner in its
  hover ("Owned by Design").
- The chips must be in the header of the files in the sidebar, above the tree they mark: "All", the reviewer groups the code-owner check asks for (a warning shield; the number of their
  files in the part, all of them for a branch-wide group, disabled when nobody tells which files), the other owners
  most files first, and "No Owner". A file of a group matches by its path or by the old path of a rename.
  [@test] ../../src/lib/review-owners.test.ts (`lists all files, the groups the check asks for, then the other owners and the files without one`, `marks the files of a group (a rename by its old path), of an owner, or without an owner`)
- A chosen chip must filter nothing: a filter cuts the story, so an owner would approve a part read without the
  declarations it uses and the uses it changes. The tree, the diffs and the story keep every file, the selection and
  the collapsed directories. The rows of the tree and the headers of the diffs of the other files are muted, a
  directory or a group of the tree without a file of the chip too; the chosen owner's keep their full weight. With
  the review plan, a step that shows files of the chip says how many on its row, a muted chip "Design 2" (its
  description "2 files of Design"), and a step without one is muted. J and K (the sections of the story, or the files
  without the plan) and N and P (the blocks) move through the chip's files only; when none is left in that direction,
  nothing moves. The line at the end of a long card names the next file of the chip, the one J shows
  ([Commit Review](../commit/commit-review.spec.md#requirements)). A link of a mark or of the card selects a file of
  another owner as any file, and the chip stays.
  [@test] ../../tests/e2e/branch-review/branch-review.spec.ts (`should mark the files of an owner chip, mute the others, and move J and K, N and P through its files only`, `should name at the end of a long card the next file of the chosen owner chip, which a click shows as J does`)
  [@test] ../../tests/e2e/branch-review/review-tour.spec.ts (`should mark the steps and the files of an owner chip in the whole story, and move N and J through its sections only`)
  [@test] ../../src/presenters/review-steps.test.ts (`counts the files of the chosen owner each step shows, 0 for a step without one`)
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should select a linked file under an owner chip, which hides none, and clear only a search that hides it`)
- While a chip is chosen, the chips must show, its own too when the shown changes have none of its files ("Design
  0"), so it can be turned off; it then mutes nothing, the keys move as without it, and the header of the files says
  "No files of Design in this range" ("Every file in this range has an owner" for No Owner). With All chosen there
  are no chips when one owner (or none) has every file and no group asks for an approval: the header of the files
  says "1 file · Auth Team".
  [@test] ../../src/lib/review-owners.test.ts (`is just All when one owner has every file, unless a group asks for an approval`, `keeps the chosen chip when the files have none of its own, so it can be turned off`, `names the chosen owner, and says when it has no files in the part`)
  [@test] ../../tests/e2e/branch-review/branch-review.spec.ts (`should keep the chip of an owner on a part without its files, muting nothing, and show no chips when one owner has them all`)
- While a chip that marks files of the shown changes is chosen, the footer must add the part of its owner after the
  file: "All Changes · File 3 / 12 · Design 2 / 5" when the file shown is the second of Design's five in the order of
  the tree, "… · Design 5" when it is another owner's. A branch-wide group's part is every file ("Design 3 / 12"),
  as its chip counts them, and it mutes nothing. A chip at 0 and a group whose files nobody tells add nothing, nor does
  All; the search of the files counts among the files it keeps.
  [@test] ../../src/lib/review-position.test.ts (`counts the part of the chosen owner chip after the file: its place on a file of its own, its files on another`)
  [@test] ../../src/lib/review-owners.test.ts (`never opens on a group whose files Patronus doesn't tell, and opens on a branch-wide one`)
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should count every file as the part of a branch-wide group, in the footer and in Approve`)
  [@test] ../../tests/e2e/branch-review/branch-review.spec.ts (`should mark the files of an owner chip, mute the others, and move J and K, N and P through its files only`, `should review a stacked branch on its parent, then the whole stack, by owner`)
- The chips, their marks and their keys must be the same in the review window of a pull request
  ([Review Requests](../repository/review-requests.spec.md#owner-chips)): one header of the files and one reading of
  the owners for both windows.
- A chip the user chooses, in this window or in the window of a pull request, must be remembered for the repository
  (`reviewOwnerChip` of its recent project, through the writer of the settings); All forgets it. A chip is not a state
  of the review: nothing else is remembered, and the approval of a pull request stays the reviewer's mark.
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should open on the user's group that needs an approval, then on the chip the user chose, which All forgets`)
- The window must open on the remembered chip when it marks at least one file of the shown changes, else on All; a
  chip the open names (Review Changes of a group of the Safe Push popover) wins. A remembered owner matches a group of
  the same name and the other way round; a group whose files nobody tells marks none. Until the user chooses, the
  first chip other than All holds: the owners and the groups that come later never move it.
  [@test] ../../tests/e2e/branch-review/branch-review.spec.ts (`should open on the owner chip last chosen in a review window of the repository where it marks files, else on All`)
  [@test] ../../src/lib/review-owners.test.ts (`opens on the remembered chip where it marks files, else the user's group that still needs an approval, else All`, `never opens on a group whose files Patronus doesn't tell, and opens on a branch-wide one`)

<a id="the-files-of-each-reviewer-group-safe-push-form"></a>

### The files of each reviewer group (Safe Push popover)

- Each group of "Needs review by the code owners", the line of the popover for a repository only on Space whose
  merge request needs approvals (`safe-push-review-groups`), must name its files: "· 2 files", "· all files" for a
  group that reviews every change, nothing for a group whose files nobody tells. "Review Changes" of that line, and of
  the line of the approvals a profile skips ("Starts without the approvals of …"), reviews the branch over the popover
  on the whole stack, the groups as chips (`toReviewGroups`). D on the row and "Review Changes" of the branch menu
  review the branch with the groups of its last code-owner check at its current head (`rememberReviewGroups`). A
  check that answers while the window is open marks its groups there too: the main window sends them with the state
  (`BranchReviewWindowState.groups`, null while none answered, when the groups of the open stay).
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should say on Space that the approvals count in the GitHub pull request, and open Settings at its repository`)
  [@test] ../../tests/e2e/branch-review/branch-review.spec.ts (`should mark the groups that need approval as the code-owner check answers while the window is open`)
- The files come from Patronus when it sends them (`paths`, `branchWide` of a reviewer group); otherwise a group gets
  the changed paths whose owner by the `OWNERSHIP` files has its name (trimmed, case-insensitive). A delegated review
  or other reviewers named by `review-rules.yaml` are known only to Patronus: such a group stays without files.
  [@test] ../../crates/sync-core/src/patronus_test.rs (`the_ownership_files_give_the_paths_to_the_groups_patronus_sent_without_them`)
  [@test] ../../src/lib/patronus.test.ts (`counts the files of a group, all of them for a branch-wide one, nothing when nobody tells`)

## User Experience

- The window (`WindowPage`) is the title bar on the canvas, the review card below it (the commit review: its sidebar
  with the commits, the chips and the tree, beside the diffs), and the footer below them on the canvas: the
  `PreviewDialogFooter` of the previews without its rule (`border-t-0 px-1 py-0`), muted `text-xs`, "File 2 / 5" on
  the left, `KeyHint`s on the right. The actions are `ActionButton`s: ghost `sm`, the icon (`GitPullRequestIcon`,
  `GitPullRequestCreateIcon`, `ShieldCheckIcon`, `UploadIcon`), the label and the key; in a title bar narrower than
  48rem only the icon, the label in the hover. A disabled action is dimmed (`opacity-50`) and stays focusable for its
  tooltip.
- A segment is an outline `xs` button, the selected ones `border-primary/50 bg-primary/10 text-primary`. A chip is a
  `FilterChip` of `FilterChips`, as the state chips of the branch filter: the stock `ToggleGroup`, the chosen one in
  its pressed state (`aria-pressed`), the count after the name in tabular numbers at 70% opacity. Above the chips, from 8 files on,
  is the search of the files ([Commit Review](../commit/commit-review.spec.md#the-search-of-the-files)); it keeps the
  files whose path has its words, the chip marks among them. A muted row or header fades its parts (`*:opacity-60`,
  `data-muted`), so the selection and the background of a sticky header keep their colour; the count of a step is a
  `secondary` `Badge` in `text-muted-foreground` after its name (`review-owner-mark`); the note of a chip at 0 is a
  line of the header (`branch-review-owner-empty`). "From Root" and "Whole Stack" are a `ToggleGroup` (`outline`, `sm`, `spacing={0}`), the shown scope
  pressed (`aria-pressed`). A Whole Stack that can't open is announced as unavailable (`aria-disabled`), dimmed
  (`opacity-50`) and takes no press, its reason in its tooltip, whose trigger is the item itself, so the group keeps
  its joined edges.
  [@test] ../../tests/e2e/branch-review/branch-review.spec.ts (`should keep an unavailable Whole Stack in the scopes, announced as unavailable, its reason on hover, a press changing nothing`)
  "From Root" has a tooltip that sizes the part as the branch table does: "auth to login
  on origin/master: 2 branches · 3 commits, what the Safe Push of login lands · Shift+click a branch to extend". A
  commit of the column is its subject, truncated, and its muted `font-mono` hash on hover
  (`range-review-commit-hash`); a commit row of the tree the same after its `GitCommitHorizontalIcon`, its role in
  `text-xs text-muted-foreground` (`review-commit-role`), its "New"/"Rewritten" an `info` `Badge`
  (`review-commit-since`), "Your review" of a pull request an `outline` `Badge` (`review-commit-review`), its branch
  muted (`review-commit-branch`), its hash on hover (`review-commit-hash`). A
  group chip starts with `ShieldAlertIcon` in `text-warning`; its tooltip says
  "Needs approval for a Safe Push".
- In the Safe Push popover the groups are a list below the review line, a group per line in the foreground colour,
  indented to its text: "Air Plugin Release Team (suggested: Alice) · 2 files".

## Testing / Local Run

- `cargo nextest run`: `git-ops` (`ownership_test`), `sync-core` (`review_target_test`, `patronus_test`).
- `pnpm test:unit`: `src/lib/branch-review.test.ts`, `src/lib/review-owners.test.ts`, `src/lib/patronus.test.ts`,
  `src/hooks/useBranchContextActions.test.ts`, `src/lib/sub-window-link.test.ts`, `src/model/store.test.ts` (two
  windows on the fake sessions).
- `pnpm e2e`: `tests/e2e/branch-review/branch-review.spec.ts` on the `stacked_ownership` template, and the code-owner
  review of `tests/e2e/patronus/patronus-actions.spec.ts`. The mock records `open_sub_window`; the test then shows the
  route in the same page with the recorded data (`openSubWindowPage`), labelled `main`. The mock records `emit`
  (`getEmittedEvents`) instead of delivering it: the footer test reads the `review/request` requests of the
  window, then shows the main window in the page again and fires them there, so the main window's listener runs them.
  A test of what both windows do at once opens the window in a page of its own (`openLinkedSubWindow`, labelled
  `review`), which subscribes to the session of the E2E server on its own stream: a sync of the main window
  reaches it as its patches.

## Open Questions / Risks

- Until Patronus sends `paths` per group, a group named in `review-rules.yaml` as the reviewer of another owner, or
  one Patronus delegates to, shows no files.
- The window acts only through the main window: closed or showing another repository, it pushes nothing.
- The tips are those of the app's session; a `bd` sync in a terminal moves the virtual branches without telling the
  window until the app syncs.
- A change of the branch prefix while the window is open leaves it on the session of the old prefix: it follows no
  sync until it opens again.

## References

- [Commit Review](../commit/commit-review.spec.md)
- [Patronus](patronus.spec.md) ("Code-owner review")
- [Stacked Branches](stacked-branches.spec.md)
- [Git Operations](../../docs/design/git-operations.md) ("Branch review", "Code owners")
