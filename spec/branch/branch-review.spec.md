---
name: Branch Review
description: Requirements for the review of a branch in its window: its diff from its parent or from the baseline, the parts of its stack, its commits, and its files by code owner, with the files each reviewer group of the Patronus code-owner check has to review.
style: plain-1
targets:
  - ../../src/pages/BranchReviewPage.tsx
  - ../../src/components/review/BranchReview.tsx
  - ../../src/hooks/useBranchReview.ts
  - ../../src/components/branch/BranchPreviewActions.tsx
  - ../../src/stores/branch-preview.ts
  - ../../src/components/review/BranchReviewStackBar.tsx
  - ../../src/components/shared/ActionButton.tsx
  - ../../src/components/shared/WindowPage.tsx
  - ../../src/lib/branch-review.ts
  - ../../src/lib/branch-review-window.ts
  - ../../src/hooks/usePreviewHotkeys.ts
  - ../../src-tauri/src/commands/branch_review.rs
  - ../../src/lib/review-owners.ts
  - ../../src/lib/range-review.ts
  - ../../src/components/review/RangeReview.tsx
  - ../../src/hooks/useBranchReviewLink.ts
  - ../../src/hooks/useMainWindowLink.ts
  - ../../src/lib/sub-window-link.ts
  - ../../src/pages/IndexPage.tsx
  - ../../src/hooks/useBranchContextActions.ts
  - ../../src/hooks/useBranchRowKeys.ts
  - ../../src/components/branch/BranchPreviewDialog.tsx
  - ../../src/components/branch/PatronusReviewBlock.tsx
  - ../../src/components/branch/StackIslandCard.tsx
  - ../../src/stores/repository.ts
  - ../../src/types/sub-window.ts
  - ../../src/lib/create-pull-request-context.ts
  - ../../src/components/review/WindowCreatePullRequestDialog.tsx
  - ../../src/hooks/useCreatePullRequestContext.ts
  - ../../src/components/branch/CreatePullRequestDialog.tsx
  - ../../src/lib/sub-window.ts
  - ../../src/router.ts
  - ../../src-tauri/capabilities/sub-windows.json
  - ../../src-tauri/capabilities/desktop.json
  - ../../src-tauri/src/commands/branch_review.rs
  - ../../crates/sync-core/src/branch_review.rs
  - ../../crates/git-ops/src/ownership.rs
---

# Branch Review

Status: Implemented
Date: 2026-10-15

## Summary

"Review Changes" (D) opens the changes of a branch in a window of its own, the only review of a branch: what the
branch adds on top of its parent, the diff of all its files in the [commit review](../commit/commit-review.spec.md),
its commits above the files in the sidebar (`BranchReview`); the review of a pull request has the same commits
([Review Requests](../repository/review-requests.spec.md#review-window)). A review is a task of minutes beside the
branch table: the window stays beside it (the Safe Push form too), where a dialog covered it. A
stacked branch shows its stack from the baseline as a bar of segments, one per branch; a click shows one branch on its
parent, Shift+click extends the part, "Whole Stack" shows everything from the baseline, which is what Patronus checks.
The files are labelled with their code owners by the `OWNERSHIP` files of the baseline, and chips filter them by the
reviewer groups the code-owner check of a Safe Push asks for and by owner. The Safe Push form names the files each
group has to review; a file opens the review at it. The title bar of the window holds the stack, the diff view and the
actions on the branch: Open Pull Request, Safe Push… and Push, which the main window runs; the footer holds the keys.

## Goals

- Look at what a branch changes before a push, a Safe Push or a pull request, without leaving the app.
- See a stacked branch alone and with its parents: a reviewer reads the child on its parent, Patronus checks all.
- Know which files need whose approval, and look at exactly those.

## Non-goals

- No comments and no approvals: the review happens in the pull request.
- No branch state in the window: the main window owns the branches and runs the actions the window asks for; the
  "Create pull request" dialog of the window shows what the main window sends and asks it to create the pull request.
- No review of the children of a branch: each has its own window, with this branch as a segment.
- No group membership: "my files" needs the members of a group, which the repository doesn't hold.

## Requirements

### Opening

- "Review Changes" of the branch menu (key D, its only review item), D on a branch row, "Review Changes" of the menu
  of a stack island (the whole main line), the code-owner block of the Safe Push form and D or the footer button of
  the branch preview must open the review window of the branch (`openBranchReviewWindow`); there is no review dialog,
  no W and no "Open in Window". The preview closes once the window opened, and stays (the error told) when it could
  not; the Safe Push form stays in the main window. It is disabled, with the reason, for a branch without commits,
  one that failed to sync and a blocked one (its virtual branch is not built on its parent).
  [@test] ../../tests/e2e/branch-review/branch-review.spec.ts (`should open the review of a branch in its window from its row, its menu and its preview`)
  [@test] ../../tests/e2e/stack-island/stack-island.spec.ts (`should open the Safe Push of the main line from the header, and offer the stack actions in its menu`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should list the files of each group and review them in the window beside the form`)
  [@test] ../../src/lib/branch-review.test.ts (`allows a synced branch with commits`)
- The window is one sub-window (`branch-review`, titled "Review <branch>", [Title Bar](../repository/title-bar.spec.md)
  for how it opens), reused for the next branch: opening it again replaces what it shows, its title too. Another
  branch or an open at a place (the code-owner block) starts it over; a plain open of the branch it shows only brings
  it to the front, where the review is. It reads the repository it was opened for, not the one selected later in the
  main window (`getDiffRepositoryPath`).
  [@test] ../../src/lib/branch-review.test.ts (`keeps its review for a plain open of the branch it shows, and starts over for another branch or a place`)
- The review gets the chain of the branch (its stack ancestors by `effectiveBase`, root first), the baseline, where
  to start (`BranchReviewFocus`: the branch itself, or the whole stack from the code-owner block and the island; the
  filter, the commit, the file), the reviewer groups of the last code-owner check of the branch at its current head.
- The app must read the review ahead when the user may open it: when the pointer rests on a branch row for 200 ms,
  when the row gets the focus, and on the open, beside the start of the window. The backend reads the segments and the
  files and the patch of the part the window shows first (the branch on its parent, or the whole stack) into its
  memory, so D finds them read (`prefetch_branch_review`). Once per state of the branch: its chain and their heads;
  a branch that cannot be reviewed is not read.
  [@test] ../../tests/e2e/branch-review/branch-review.spec.ts (`should read the review of a branch ahead once the pointer rests on its row or it has the focus, once per state`)
- The window must start the read of its segments with its page, not after its route chunk loaded; its first read
  takes that read when it asks for the same review, a later one reads again.
  [@test] ../../src/lib/review-window-reads.test.ts (`gives the first read of the page the read its load started, once, and only for the same review`)
- In the window ↑/↓, Space and P are the review's (P is the previous change), never a push.
  [@test] ../../tests/e2e/branch-review/branch-review.spec.ts (`should open the review of a branch in its window from its row, its menu and its preview`)

### The stack and the commits

- The stack bar must show the baseline, then a segment per branch of the chain with its number of commits. The
  branch itself is selected at first. A click selects one segment, Shift+click extends the part to the clicked
  segment; "Whole Stack" selects all of them. The diff is from the base of the first segment of the part (the head of
  its parent, the commit the root is built on) to the head of its last one.
  [@test] ../../src/lib/branch-review.test.ts (`diffs the base of the first segment to the head of the last one`, `selects one segment, or extends the range to the clicked one with Shift`)
  [@test] ../../tests/e2e/branch-review/branch-review.spec.ts (`should review a stacked branch on its parent, then the whole stack, by owner`)
- The stack bar must stay on one line, the title bar's, never running under its actions: the baseline and the segments
  outside the part give way first (cut with "…", the name in their hover), the selected segments and the last one
  last.
  [@test] ../../tests/e2e/repository/title-bar.spec.ts (`should start the row of a review window after the traffic lights, its empty parts dragging the window`)
- The stack bar of a stack of more than 6 branches must stay short: it shows the root, the ends of the selected
  part and the last two segments (the parent and the branch), and folds each run of two or more others into a ghost
  "… 14 more" button (in `text-primary` while the part covers it) with "14 branches · 120 commits · Click to show them"
  as its tooltip; a click shows every segment.
  [@test] ../../src/lib/branch-review.test.ts (`folds the middle of a long stack, keeping the root, the ends of the range and the last two segments`, `shows every segment of a short stack, or once unfolded`)
- The segments come from the virtual branches: one `for-each-ref` for the tips, one `git log --first-parent` from
  the baseline to the last tip, cut at the tips of the parents. A parent whose head is not below its child, or a
  branch without a virtual branch, is an error that asks for a sync.
  [@test] ../../crates/sync-core/src/branch_review_test.rs (`splits_a_stack_at_the_heads_of_the_parents`, `a_branch_outside_a_stack_is_one_segment_from_the_baseline`, `a_child_not_built_on_the_head_of_its_parent_needs_a_sync`)
- The app must remember the segments by the tips and the baseline's refs that the `for-each-ref` reads (it lists the
  refs the baseline's name may stand for too), and run the `log` again only when a tip or the baseline moved.
  [@test] ../../crates/sync-core/src/branch_review_test.rs (`a_memoised_review_runs_no_log_for_the_same_tips`, `a_moved_tip_or_baseline_is_read_again`)
- The commits (`RangeReview`, the ones of a pull request too) must be above the files in the sidebar of the commit
  review ([Commit Review](../commit/commit-review.spec.md#the-sidebar-and-the-diffs)): "All Changes" with the number
  of commits of the part, then its commits newest first, under the name of their branch when the part has several; a commit shows
  only its own changes, its subject, author, time and message (three lines, "Show More") above them. ⌥↑/⌥↓ step
  through the commits from anywhere in the review, ↑/↓ inside them (one tab stop, a listbox), which keep the focus while
  the diffs of the next commit mount. A part of one commit has no list: it shows that commit. A sync that rebuilds the
  commits keeps the shown one by its branch, subject and author time.
  [@test] ../../src/lib/range-review.test.ts (`lists all changes, then the commits of the reviewed branch first, each branch headed by its name`, `keeps the commit by its id, else the one a sync rebuilt in its place`)
  [@test] ../../tests/e2e/branch-review/branch-review.spec.ts (`should review a stacked branch on its parent, then the whole stack, by owner`)
- Another commit, another owner filter or a sync that reads the stack again must keep the file on screen when the new
  changes have it.
  [@test] ../../tests/e2e/branch-review/branch-review.spec.ts (`should review a stacked branch on its parent, then the whole stack, by owner`)
- A sync that moves a tip of the chain must read the stack again, once per set of heads, and keep the part by the names
  of its ends (the reviewed branch alone when an end left the chain): the main window sends the heads the last sync
  built (`tips` of the window state, over its link `branch-review/state`).
  [@test] ../../src/lib/branch-review.test.ts (`keeps the range by the names of its ends, or falls back to the reviewed branch`)
  [@test] ../../tests/e2e/branch-review/branch-review.spec.ts (`should read the stack again when a sync moves a tip of it`)
- A branch of the chain the last sync no longer built (it landed, it was renamed) must keep the last view with "login
  is no longer a branch: it landed or was renamed": an info `Alert` above the review in the window; its Safe Push…
  and Push are disabled with that
  reason, its pull request still opens, and the stack is not read again.
  [@test] ../../tests/e2e/branch-review/branch-review.spec.ts (`should act on the branch of the part from the title bar, through the main window`)

### Title bar, footer and actions

- The title bar of the window ([Title Bar](../repository/title-bar.spec.md)) must show the stack bar, then at its other
  end the Unified/Split toggle and the actions on the target branch. The footer must show the position in the review
  ("All Changes · File 2 / 5", "Commit 1 / 3 · File 1 / 2") and the keys of the commit review (`J K Files`, `N P
  Changes`, `⌥↑ ⌥↓ Commits` with several commits), `Esc Close` (Escape closes a sub-window). The actions are the items
  of the branch's menu (`getBranchReviewItems`), their keys working in the whole window: `O Open Pull Request` when it has a pull request, `Create Pull Request…` for a
  branch on GitHub without a live one, pushed or not (its dialog opens in the window, [below](#the-create-pull-request-dialog), and the main window pushes the branch on Create or "Continue on GitHub"), `S Safe Push…` where Patronus is on the repository and the branch has work, and the push for
  a branch with work, named as the push button of its row (`Push (2)`, `Push Stack (2 branches)`), without a key: P is
  the previous change.
  [@test] ../../tests/e2e/branch-review/branch-review.spec.ts (`should act on the branch of the part from the title bar, through the main window`)
- Unified/Split must be the setting of every diff view, saved by the main window, which owns `settings.json`
  ([Distributed Settings](../../docs/design/distributed-settings.md)): the window asks it, and keeps the choice on its
  answer.
  [@test] ../../tests/e2e/branch-review/branch-review.spec.ts (`should save the diff view the window chooses through the main window`)
- The target branch is the branch of the last selected segment, the reviewed end of the part: for "Whole Stack", the
  reviewed branch; for a parent alone, the parent.
  [@test] ../../tests/e2e/branch-review/branch-review.spec.ts (`should act on the branch of the part from the title bar, through the main window`)
- What the window offers comes from the main window, over the link of the window (`sub-window-link.ts`): the state of
  the window data at the open (`BranchReviewWindowState`: the heads of the chain, the items of each branch), then
  `branch-review/state` whenever it changes (the branches, a sync or a push, the pull requests, the robots, the inline
  action of a row, the selected repository), deduplicated, for the last opened chain. An unavailable Safe Push… or
  Push stays, disabled, with the reason of the menu on hover ("A Safe Push of the last commit is running", "Wait for
  the sync to finish"): one blocker for the menu, S on the row, the branch preview and the review
  (`getSafePushBlocker`, `getPushBlocker`).
  [@test] ../../tests/e2e/branch-review/branch-review.spec.ts (`should act on the branch of the part from the title bar, through the main window`)
  [@test] ../../src/hooks/useBranchContextActions.test.ts (`says why a branch can't start a Safe Push, in the words of every surface that offers it`)
- While the main window shows another repository, the title bar keeps the last items: Safe Push… and Push are disabled
  with "Select app in the main window to push login from here", the pull request still opens.
  [@test] ../../tests/e2e/branch-review/branch-review.spec.ts (`should act on the branch of the part from the title bar, through the main window`)
- O (and its button) opens the pull request from the window. Safe Push… (S) and Push only ask: the window sends
  `branch-review/request` (`{ id, repositoryPath, branchName, itemId: "safe-push" | "push" }`). The main window builds
  the item again and runs it as the row would: it comes to the front, reveals the row and opens its Safe Push form, or
  pushes as the push button of the row does (`pushOrConfirm`: the stack while a parent has work, a push that drops
  remote commits asks in the row). A request it can't run (another repository is shown, the branch left, the item is
  disabled) is refused with the reason, shown in the window (`branch-review/feedback`: "Cannot push login", "Cannot
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
  asks `{ dialog: "safePushWithoutReview" }`: the main window comes to the front, reveals the row and opens its Safe
  Push form without review ([Patronus](patronus.spec.md#safe-push-without-review)).
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
- The chips must be in the header of the files in the sidebar, above the tree they filter: "All", the reviewer groups the code-owner check asks for (a warning shield; the number of their
  files in the part, all of them for a branch-wide group, disabled when nobody tells which files), the other owners
  most files first, and "No Owner". A chip filters the tree and the diffs; a file of a group matches by its path or by
  the old path of a rename. A filter that keeps nothing says "No files of <group> in this range".
  [@test] ../../src/lib/review-owners.test.ts (`lists all files, the groups the check asks for, then the other owners and the files without one`, `keeps the files of a group (a rename by its old path), of an owner, or without an owner`)
- While a filter is on, the chips must show, its own chip too when the shown changes have none of its files ("Design
  0"), so the filter can be turned off. Without a filter nothing filters when one owner (or none) has every file and no
  group asks for an approval: no chips, the header of the files says "1 file · Auth Team".
  [@test] ../../src/lib/review-owners.test.ts (`is just All when one owner has every file, unless a group asks for an approval`, `keeps the chip of the active filter when the files have none of its own, so it can be turned off`)
  [@test] ../../tests/e2e/branch-review/branch-review.spec.ts (`should keep the chip of the filter on a part without its files, and show no chips when one owner has them all`)

### The files of each reviewer group (Safe Push form)

- Each group of "Needs review by the code owners" must name its files: "· 2 files" with a disclosure listing their
  paths, "· all files" for a group that reviews every change, nothing for a group whose files nobody tells. A path
  reviews the branch at the file over the form, on the whole stack, filtered to the group; "Review Changes" beside the
  callout's buttons reviews it on the whole stack with the groups as chips.
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should list the files of each group and review them in the window beside the form`)
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
- A segment and a chip are outline `xs` buttons; the selected ones are `border-primary/50 bg-primary/10
  text-primary`. "Whole Stack" is a ghost button with a tooltip that sizes the stack as the branch table does: "From origin/master:
  2 branches · 3 commits, what Patronus checks · Shift+click a branch to extend". A group chip starts with `ShieldAlertIcon` in `text-warning`; its tooltip says
  "Needs approval for a Safe Push".
- In the Safe Push form a group is a disclosure (`ChevronRightIcon`, the line of the group, "· 2 files" muted), its
  paths in `text-xs` muted, underlined on hover.

## Testing / Local Run

- `cargo nextest run`: `git-ops` (`ownership_test`), `sync-core` (`branch_review_test`, `patronus_test`).
- `pnpm test:unit`: `src/lib/branch-review.test.ts`, `src/lib/review-owners.test.ts`, `src/lib/patronus.test.ts`,
  `src/hooks/useBranchContextActions.test.ts`, `src/lib/sub-window-link.test.ts`.
- `pnpm e2e`: `tests/e2e/branch-review/branch-review.spec.ts` on the `stacked_ownership` template, and the code-owner
  review of `tests/e2e/patronus/patronus-actions.spec.ts`. The mock records `open_sub_window`; the test then shows the
  route in the same page with the recorded data (`openSubWindowPage`), labelled `main`. The mock records `emit`
  (`getEmittedEvents`) instead of delivering it: the footer test reads the `branch-review/request` requests of the
  window, then shows the main window in the page again and fires them there, so the main window's listener runs them.

## Open Questions / Risks

- Until Patronus sends `paths` per group, a group named in `review-rules.yaml` as the reviewer of another owner, or
  one Patronus delegates to, shows no files.
- The window acts only through the main window: closed or showing another repository, it pushes nothing.
- The tips the main window sends are the heads it synced; a `bd` sync in a terminal moves the virtual branches
  without telling the window until the app syncs.

## References

- [Commit Review](../commit/commit-review.spec.md)
- [Patronus](patronus.spec.md) ("Code-owner review")
- [Stacked Branches](stacked-branches.spec.md)
- [Git Operations](../../docs/design/git-operations.md) ("Branch review", "Code owners")
