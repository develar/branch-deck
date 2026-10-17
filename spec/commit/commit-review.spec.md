---
name: Commit Review
description: Requirements for the review of a commit in its window, and of the range of a pull request or a branch: a sidebar (the commits of a range, the header of the files, their directory tree) beside the diffs of all of them in one virtualized view, with keys for the files, the changes and the commits.
style: plain-1
targets:
  - ../../src/components/commit/CommitReview.tsx
  - ../../src/components/commit/CommitPreviewDialog.tsx
  - ../../src/components/diff/CommitReviewCode.tsx
  - ../../src/components/diff/LazyDiffViews.tsx
  - ../../src/components/files/FileTree.tsx
  - ../../src/components/files/FileContextMenu.tsx
  - ../../src/components/shared/WindowPage.tsx
  - ../../src/pages/CommitReviewPage.tsx
  - ../../src/lib/diff/change-targets.ts
  - ../../src/lib/diff/commit-patch.ts
  - ../../src/lib/file-tree.ts
  - ../../src/lib/preview-keys.ts
  - ../../src/stores/commit-patch.ts
  - ../../src/stores/commit-files.ts
  - ../../src/lib/commit-files.ts
  - ../../src/components/review/PullRequestReviewDialog.tsx
  - ../../src/pages/BranchReviewPage.tsx
  - ../../src/pages/CommitReviewPage.tsx
  - ../../src/lib/commit-review-window.ts
  - ../../src/components/commit/CommitList.tsx
  - ../../src/hooks/useCommitReviewActions.ts
  - ../../src/hooks/usePreviewHotkeys.ts
  - ../../src/components/review/RangeReview.tsx
  - ../../src/components/review/ReviewKeyHints.tsx
  - ../../src/lib/review-position.ts
  - ../../src/router.ts
  - ../../src-tauri/capabilities/sub-windows.json
---

# Commit Review

Status: Implemented
Date: 2026-10-15

## Summary

The commit preview shows a commit, as Quick Look shows a file; the review reads it, in a window of its own, as ⌘O
opens a file: D on a row, the item of its menu or D in the preview opens the window `commit-review`. A sidebar with
the directory tree of the files on the left, the diffs of all of them in one scrolling view on the right, a sticky
header per file, as a code review in Space, GitHub or IntelliJ shows a change. The tree follows the diffs as they
scroll, a file chosen in the tree scrolls the diffs to it, J and K step through the files and N and P through the
changes. A review is a task of minutes beside the branch table: a window stays beside it, on another monitor too,
where a dialog would cover it.

## Goals

- Read a whole commit, file after file, without opening each file.
- Never lose the place: the tree says which file is on screen, and the keys go to the next file or change.
- Stay fast on a large commit: one git process for the patch, the diffs virtualized, large and generated files
  collapsed.

## Non-goals

- No comments, no "viewed" marks: the review reads, it does not record.
- No review of uncommitted changes; the amend card shows them ([Amend Changes](amend.spec.md)).
- No search in the diffs: the browser's find sees only the rendered part of a virtualized view.

## Requirements

### Opening and closing

- The review is only in the window `commit-review` (`/commit-review`, one window, reused, titled "Review a1b2c3d
  <subject>" and retitled as it steps; [Title Bar](../repository/title-bar.spec.md) for how it opens). There is no
  review dialog, no W and no "Open in Window": one action, "Review Changes" (D).
- D on a commit row (the focused row while the pointer hasn't moved, else the row under it, `usePreviewHotkeys`) and
  "Review Changes" (D), the first item of the row's menu, must open the window at the first file; no preview opens. A
  commit without files gets its preview, which says so; the menu item is disabled with "The commit changes no files",
  and with "Review one commit at a time" for a selection of several. An open menu keeps D and Space (they act on no
  row under it); a closing one does not, so D right after Escape reviews.
  [@test] ../../tests/e2e/commit-files/commit-review.spec.ts (`should review a commit of a row in its window with D and with the item of its menu`)
- In the preview, D and the `D Review Changes` button of its footer must open the window at the first file, Enter on
  a file of the Files tree at that file; the preview closes once the window opened, and stays (the error told) when
  it could not.
  [@test] ../../tests/e2e/commit-files/commit-review.spec.ts (`should open the review at a file with Enter in the tree, and with the footer button`, `should show the tree beside the diffs of all files and follow J, K, N, P and the tree`)
- A list in a sub-window (the missing commits of a conflict) opens the same window, with the repository of that
  window (`repositoryPath` of its data).
  [@test] ../../tests/e2e/conflict-viewer/conflict-viewer.spec.ts (`should review a missing commit in the review window from the missing commits window`)
- The window gets the list as it was (its commits don't change), its name and the commit. Another commit, the same
  one from another list, or an open at a file starts the window over; an open of the commit it shows only brings it
  to the front, where the review is.
- The window shows the commits of the list above its files (the commits of a range review, without a scope), the
  subject, meta line and message of the commit above its diffs, the footer "Commit 2 / 3 · File 1 / 1" and
  `Esc Close`. Its title bar names the list and the commit ("session › 11346cdf", `listLabel` of the window data: the
  branch, or "Unassigned") and ends with the Unified/Split toggle. Nothing is edited there.
- ⌥↑/⌥↓ (Alt+↑/↓) must show the previous or next commit of the list.
  [@test] ../../tests/e2e/commit-files/commit-review.spec.ts (`should switch the commit of a review with Alt and the arrows`)

- The review has a subject: a commit, or the range `base..head` of a pull request that waits for the user's review
  ([Review Requests](../repository/review-requests.spec.md#review-dialog)) or of a branch in its review window
  ([Branch Review](../branch/branch-review.spec.md)). The same view shows both: the loading line,
  the error ("Cannot read the changes of the commit", "… of the pull request") and the empty state are one. A range is
  keyed `<base>..<head>` in the stores of the files and the patches and dropped when its review closes.
  [@test] ../../src/stores/commit-files.test.ts (`loads the range of a review by its id, base..head, until its review drops it`)
  [@test] ../../src/stores/commit-patch.test.ts (`reads the range of a review with its own command, keyed base..head, until its review drops it`)
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should preview the pull request, fetch it with progress, then review its range with the file tree`)

### The sidebar and the diffs

- The left pane must be a sidebar: the commits of a range on top (a range of several commits,
  [Branch Review](../branch/branch-review.spec.md), [Review Requests](../repository/review-requests.spec.md#review-dialog)), at most 40% of its height and scrolling inside; then the header of the files ("5
  files", or what the surface puts there: the owner chips of a branch review); then the tree of the Files section, in
  the order of the tree, a file row selecting the file. The selection follows the focus.
- The sidebar must be resizable (30% at first, never narrower than 16rem, at most 50%) and keep its width
  (`review-sidebar`, one width for every review). The split must be there in every state of the review: while another
  commit or filter loads, has no files or cannot be read, the diffs pane says so, and the commits and the header of
  the files stay where they are, with their focus.
  [@test] ../../tests/e2e/branch-review/branch-review.spec.ts (`should review a stacked branch on its parent, then the whole stack, by owner`)
- The right pane must show the diffs of all files in the order of the tree, in one virtualized view, each under a
  sticky header with its status letter, its directory muted and its name, where a rename came from, its owner, its
  line counts, a copy button and the menu of the file ([Commit Files](commit-files.spec.md#the-actions-on-a-file)).
  The commit a range shows is above them.
- A file chosen in the tree must scroll its header to the top. As the diffs scroll, the file whose header is at the
  top must become the selected file, its directories opened. A scroll the review made (a choice, J, K, N, P) changes
  the selection only through the key or the row that made it, also for a last file that cannot reach the top.
  [@test] ../../tests/e2e/commit-files/commit-review.spec.ts (`should show the tree beside the diffs of all files and follow J, K, N, P and the tree`)
- J and K must select the next and the previous file. N and P must show the next and the previous block of changed
  lines, into the next or the previous file at the end of one; collapsed files are skipped.
  [@test] ../../src/lib/diff/change-targets.test.ts (`finds each block of changed lines on the new side, past the context and the other blocks`, `takes the old side of a block that only deletes, and counts every hunk from its own start`)
  [@test] ../../tests/e2e/commit-files/commit-review.spec.ts (`should show the tree beside the diffs of all files and follow J, K, N, P and the tree`)
- The diffs must have the focus when the review shows its subject, so the arrows, PageUp, PageDown and Space scroll them;
  Shift+Tab goes to the tree. While another commit loads, or one without files or that cannot be read is shown, the
  review itself keeps the focus, so ⌥↑/⌥↓ and Escape still work. What the user works in beside the diffs (the commits,
  the owner chips) keeps the focus when the diffs of the next subject mount; J, K, N and P work from there too.
- A binary file, a file without changed lines and a file the patch left out must show only their header, which says
  so. Lock files (`pnpm-lock.yaml`, `Cargo.lock`, …), minified and snapshot files, and files with more than 1,000
  changed lines must start collapsed; the chevron of the header expands them.
- The unchanged lines between hunks must expand as in the Files section ([Commit Files](commit-files.spec.md)).
- The Unified/Split toggle must switch the layout; it is the setting of the Files section. It is in the header of the
  block whose diffs it switches: the title bar of a review window, the toolbar of a pull request.
  [@test] ../../tests/e2e/commit-files/commit-review.spec.ts (`should show the tree beside the diffs of all files and follow J, K, N, P and the tree`)
- A review may show a part of its files (the branch review filters by code owner: by the path or the old path of a
  rename) and the owner of each file, muted in the header of its diff and in the hover of its tree row; a tree row
  keeps its width for the name. A filter that keeps nothing says why ("No files of Air Plugin in this range").
  [@test] ../../tests/e2e/branch-review/branch-review.spec.ts (`should review a stacked branch on its parent, then the whole stack, by owner`)

## User Experience

- The island of the review fills the window below its title bar; the panes scroll on their own. A 1px `bg-border`
  divider separates them.
- The sidebar has the commits above a `border-b`, then the header of the files ("5 files" in muted `text-xs`, at least
  `h-10`), then the tree with the rows of the Files section. The selected row is `bg-primary/15`; a file row has no
  chevron, its name keeps the indent.
- A file header is `h-9` on `bg-card` with a `border-b`: a ghost `icon-xs` chevron (disabled without lines to show),
  the status letter in `font-mono text-xs` in the status colour, the directory muted and the name `font-medium`, the
  copy button on hover, then on the right the notice and the owner muted and the coloured counts in `text-xs`.
- The review window ([Title Bar](../repository/title-bar.spec.md)) names the list and the commit in its title bar
  ("session › 11346cdf"), the toggle at its other end; the native title stays "Review 11346cdf <subject>".
- The footer shows "File 3 / 12" on the left, "Commit 2 / 3 · File 3 / 12" in a list of several commits, and on the
  right the hints `J K Files`, `N P Changes`, `⌥↑ ⌥↓ Commits` (`Alt+↑ Alt+↓` off macOS, not for a list of one) and
  `Esc Close`.
- The preview's footer keeps `D Review Changes` (the window opens), `F Files`, `E Edit` and `Space Close`.

## Testing / Local Run

- `pnpm test:unit`: `src/lib/diff/change-targets.test.ts`, and the tests of [Commit Files](commit-files.spec.md) for
  the tree and the patch.
- `pnpm e2e`: `tests/e2e/commit-files/commit-review.spec.ts` on the `commit_tree` and `issue_links` templates. A
  virtualized view renders what is on screen: assert on the text of files the review scrolled to.

## Open Questions / Risks

- The patch of a commit is parsed whole when the review opens; a commit of thousands of files may take a moment.
- A diff without the file contents cannot know whether lines follow its last hunk, so its last separator offers
  "More unchanged context may be available".
- The browser's find (⌘F) sees only the rendered files.

## References

- [Commit Files](commit-files.spec.md)
- [Diff View](../../docs/design/diff-view.md) ("Review")
- [Commit Row](commit-row.spec.md) (the preview)
- Space and GitHub code review: a file tree beside one scrolling list of file diffs with sticky headers.
