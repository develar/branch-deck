---
name: Commit Review
description: Requirements for the review of a commit in its window, and of the range of a pull request or a branch: a sidebar (the commits of a range, the header of the files, their directory tree) beside the diffs of all of them in one virtualized view, with keys for the files, the changes and the commits.
style: plain-1
targets:
  - ../../src/components/commit/CommitReview.tsx
  - ../../src/components/commit/CommitPreviewDialog.tsx
  - ../../src/components/diff/CommitReviewCode.tsx
  - ../../src/components/diff/DiffView.tsx
  - ../../src/components/diff/diff-painter.ts
  - ../../src/lib/sub-window.ts
  - ../../src/lib/open-timing.ts
  - ../../src/lib/review-window-reads.ts
  - ../../src-tauri/src/commands/window_management.rs
  - ../../src/components/files/FileTree.tsx
  - ../../src/components/files/FileContextMenu.tsx
  - ../../src/components/shared/WindowPage.tsx
  - ../../src/pages/CommitReviewPage.tsx
  - ../../src/lib/diff/rows.ts
  - ../../src/lib/diff/layout.ts
  - ../../src/lib/diff/view-files.ts
  - ../../src/lib/file-tree.ts
  - ../../src/lib/preview-keys.ts
  - ../../src/stores/file-diffs.ts
  - ../../src/stores/blob-texts.ts
  - ../../src/stores/commit-files.ts
  - ../../src/lib/commit-files.ts
  - ../../src/pages/BranchReviewPage.tsx
  - ../../src/pages/CommitReviewPage.tsx
  - ../../src/lib/commit-review-window.ts
  - ../../src/lib/branch-review-window.ts
  - ../../src/components/commit/CommitList.tsx
  - ../../src/hooks/useCommitReviewActions.ts
  - ../../src/hooks/usePreviewHotkeys.ts
  - ../../src/components/review/RangeReview.tsx
  - ../../src/components/review/ReviewKeyHints.tsx
  - ../../src/lib/review-position.ts
  - ../../src/router.ts
  - ../../src-tauri/capabilities/sub-windows.json
  - ../../src/components/shared/SearchField.tsx
  - ../../src/hooks/useSlashFocus.ts
  - ../../src/lib/file-filter.ts
  - ../../src/lib/text-search.ts
  - ../../src/components/files/FileHeaderParts.tsx
---

# Commit Review

Status: Implemented
Date: 2026-10-15

## Summary

The commit preview shows a commit, as Quick Look shows a file; the review reads it, in a window of its own, as ⌘O
opens a file: D on a row, the item of its menu or D in the preview opens the review. A commit lands in master with its
branch ([Landing](../../docs/design/landing.md)), so a commit of a branch is reviewed in the review window of its
branch, at the commit, with the stack, the actions and the other commits of the branch beside it
([Branch Review](../branch/branch-review.spec.md)); a commit no branch review shows (unassigned, archived, of a branch
not built on its parents, of a conflict) opens the window `commit-review`. A sidebar with
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
- Open at once: the window shows the review complete in its first frame, the first diffs highlighted, when the patch
  is read already; a slow read shows the files first.

## Non-goals

- No comments, no "viewed" marks: the review reads, it does not record.
- No review of uncommitted changes; the amend card shows them ([Amend Changes](amend.spec.md)).
- The search of the sidebar finds files by their path, not text in them; the find of the diffs finds text.

## Requirements

### Opening and closing

- The review of a commit is in a window: the review window of its branch, at the commit, or the window
  `commit-review` (`/commit-review`, one window, reused, titled "Review a1b2c3d <subject>" and retitled as it steps;
  [Title Bar](../repository/title-bar.spec.md) for how it opens). There is no review dialog, no W and no "Open in
  Window": one action, "Review Changes" (D).
- A commit of a branch must open the review window of the branch at the commit (`reviewBranchCommit`: the copy of the
  commit in the virtual branch, `focus.commitId`, and the file, `focus.path`), titled "Review login · a1b2c3d", from
  every surface below; a commit of no branch, of a branch the branch review can't show (blocked, failed, in conflict:
  `branchReviewBlocker`) or one its virtual branch doesn't have yet (no copy, a conflicting or blocked commit) opens
  the window `commit-review`. Only the main window decides: a list in a sub-window opens `commit-review`.
  [@test] ../../tests/e2e/commit-files/commit-review.spec.ts (`should review a commit of a stacked branch with its stack and its actions, and move to a commit of the chain`)
  [@test] ../../tests/e2e/commit-files/commit-review.spec.ts (`should review an unassigned commit in the review window of the commit`)
  [@test] ../../tests/e2e/commit-files/commit-review.spec.ts (`should review a commit of a branch in conflict and of a blocked one in the review window of the commit`)
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
- The close of the preview (also by D) must focus its list, and the keys of a row (Space, D) then act on the commit it
  showed last, also after ↑/↓, while the pointer is on no row; a click of a row makes it the anchor again.
  [@test] ../../tests/e2e/commit-files/commit-review.spec.ts (`should give the focus to the list after the preview, its keys going on from the commit it showed last`)
- A list in a sub-window (the missing commits of a conflict) opens the same window, with the repository of that
  window (`repositoryPath` of its data).
  [@test] ../../tests/e2e/conflict-viewer/conflict-viewer.spec.ts (`should review a missing commit in the review window from the missing commits window`)
- The window gets the list as it was (its commits don't change), its name and the commit. Another commit, the same
  one from another list, or an open at a file starts the window over; an open of the commit it shows only brings it
  to the front, where the review is.
  [@test] ../../src/lib/commit-review-window.test.ts (`keeps its review for an open of the commit it shows, and starts over for another commit, list or file`)
- The window shows the commits of the list above its files (the commits of a range review, without a scope), the
  commit above its diffs as the branch preview heads one (`CommitHeading`: the subject `text-md font-medium`, the meta
  line with the committer time on hover and the coloured line counts, the reading body, three lines until Show More), the footer "Commit 2 / 3 · File 1 / 1" and
  `Esc Close`. Its title bar names the list and the commit ("Unassigned › 11346cdf", `listLabel` of the window data: the
  branch, or "Unassigned") and ends with the Split/Unified/Inline toggle. Nothing is edited there.
- ⌥↑/⌥↓ (Alt+↑/↓) must show the previous or next commit of the list.
  [@test] ../../tests/e2e/commit-files/commit-review.spec.ts (`should switch the commit of a review with Alt and the arrows`)

- The review has a subject: a commit, or the range `base..head` of a pull request that waits for the user's review
  ([Review Requests](../repository/review-requests.spec.md#review-window)) or of a branch in its review window
  ([Branch Review](../branch/branch-review.spec.md)). The same view shows both: the loading line,
  the error ("Cannot read the changes of the commit", "… of the pull request") and the empty state are one. A range is
  keyed `<base>..<head>` in the stores of the files and the diffs and dropped when its review closes.
  [@test] ../../src/stores/commit-files.test.ts (`loads the range of a review by its id, base..head, until its review drops it`)
  [@test] ../../src/stores/file-diffs.test.ts (`keeps the last subjects only, and forgets the subjects of a closed review under every policy`)
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should preview the pull request, fetch it with progress, then review its range with the file tree`)

### The sidebar and the diffs

- The left pane must be a sidebar: the commits of a range on top (a range of several commits,
  [Branch Review](../branch/branch-review.spec.md), [Review Requests](../repository/review-requests.spec.md#review-window)), at most 40% of its height and scrolling inside; then the header of the files ("5
  files", or what the surface puts there: the owner chips of a branch review); then the tree of the Files section, in
  the order of the tree, a file row selecting the file. The selection follows the focus.
- The sidebar must be resizable (30% at first, never narrower than 16rem, at most 50%) and keep its width
  (`review-sidebar`, one width for every review). The split must be there in every state of the review: while another
  commit or filter loads, has no files or cannot be read, the diffs pane says so, and the commits and the header of
  the files stay where they are, with their focus.
  [@test] ../../tests/e2e/branch-review/branch-review.spec.ts (`should review a stacked branch on its parent, then the whole stack, by owner`)
- The right pane must show the diffs of all files in the order of the tree, in one virtualized view, each file a card
  as in IntelliJ's combined diff: 10 px apart, a 1-px border, rounded corners, a collapsed file its header alone. Its
  header is the top of the card and sticks while its rows scroll, until the end of the card pushes it up. The header
  shows the status letter, the name bold, where a rename came from and its directory muted, Expand All with its
  tooltip, its owner, its line counts, a copy button and the menu of the file
  ([Commit Files](commit-files.spec.md#the-actions-on-a-file)). The commit a range shows is above them.
  [@test] ../../src/lib/diff/layout.test.ts (`counts the gaps, the space around the cards and the bottom border of each card, so each offset stays exact`)
- A file chosen in the tree must scroll its header to the top. As the diffs scroll, the file whose header is at the
  top must become the selected file, its directories opened. A scroll the review made (a choice, J, K, N, P) changes
  the selection only through the key or the row that made it, also for a last file that cannot reach the top.
  [@test] ../../tests/e2e/commit-files/commit-review.spec.ts (`should show the tree beside the diffs of all files and follow J, K, N, P and the tree`)
- J and K must select the next and the previous file. N and P must show the next and the previous block of changed
  lines, into the next or the previous file at the end of one; collapsed files are skipped.
  [@test] ../../src/lib/diff/rows.test.ts (`starts N and P at each block of changed lines, a deleted one too, never at a block the policy ignores`)
  [@test] ../../tests/e2e/commit-files/commit-review.spec.ts (`should show the tree beside the diffs of all files and follow J, K, N, P and the tree`, `N and P skip a block the whitespace policy ignores`)
- The tree and the headers of the first files must show as soon as the files are read, while git reads the patch;
  "Loading changes…" only after 300 ms. The diffs mount with the diffs of the engine, before their texts: each file
  reads its texts by blob id as it comes into view, and its rows show plain text until the tokens of their page
  arrive. The window shows once the rows of the first screen are painted with their texts, and their tokens are there
  or 80 ms late ([Title Bar](../repository/title-bar.spec.md)). Once the diffs are read, the diffs of the commits
  before and after it in the list are read too, so ⌥↑/⌥↓ show them at once.
  [@test] ../../tests/e2e/commit-files/commit-review.spec.ts (`should show the files while the patch is read, settle with the diffs, and step to a commit read beside it`)
- The diffs must have the focus when the review shows its subject, so the arrows, PageUp, PageDown and Space scroll them;
  Shift+Tab goes to the tree. While another commit loads, or one without files or that cannot be read is shown, the
  review itself keeps the focus, so ⌥↑/⌥↓ and Escape still work. What the user works in beside the diffs (the commits,
  the owner chips) keeps the focus when the diffs of the next subject mount; J, K, N and P work from there too.
- A binary file, a file without changed lines and a file the patch left out must show only their header, which says
  so. Lock files (`pnpm-lock.yaml`, `Cargo.lock`, …), minified and snapshot files, and files with more than 1,000
  changed lines must start collapsed; the chevron of the header expands them.
- The unchanged lines between hunks must expand as in the Files section ([Commit Files](commit-files.spec.md)).
- A copy of a selection in the diffs must give the lines its rows show, from the texts, also of rows no longer on
  screen: without the line numbers, the folds and the deleted parts of an Inline row, the old and the new line of a
  changed pair in Unified, and one side in Split (a press on a side keeps the selection there). ⌘A (Ctrl+A off macOS)
  selects the side of the file at the top, and a copy then gives all its shown lines.
  [@test] ../../src/lib/diff/copy-text.test.ts (`copies the lines the rows show from the model, cut at both ends, in the order of the diffs`, `copies one side in Split, and the new line of an Inline row without its deleted parts`, `copies across files and leaves out a fold and a text not read yet`)
  [@test] ../../tests/e2e/commit-files/commit-review.spec.ts (`should copy the lines of a selection of the diffs from the model, one side in Split, without the line numbers`)
- ⌘F (Ctrl+F off macOS) in the diffs must open a find bar above them. It finds the text in the lines the rows show,
  ignoring case, from the model, not from the rows on screen: both sides of a change in Split and the new side of an
  unchanged line, and with "In folded lines" the folded lines too. It reads the texts of every file it searches. The
  bar counts the hits ("12 results", "3 of 12"); Enter and ⌘G go to the next one, Shift with them to the previous one,
  a hit in a fold opens its fold, and the current hit is marked; Escape closes the bar and gives the diffs the focus.
  [@test] ../../src/lib/diff/find.test.ts (`finds the query in the lines the rows show, ignoring case, in the order of the rows`, `searches both sides of a change in Split and the new side of an unchanged line, and the folds on request`, `groups the hits by file and line for the painter, the current one marked`)
  [@test] ../../tests/e2e/commit-files/commit-review.spec.ts (`should find a text in the diffs from the model, also in the folded lines, with ⌘F, Enter and Escape`)
- The diffs come from `get_file_diffs` and their texts from `get_blob_texts`
  ([Commit Files](commit-files.spec.md#file-diffs)): N and P step through the blocks that are not `Ignored`.
- The Split/Unified/Inline toggle must switch the layout; it is the setting of the Files section, Split by default. It is in
  the header of the block whose diffs it switches: the title bar of a review window, the toolbar of a pull request.
  Soft wraps follow the setting of Settings → Appearance ([Commit Files](commit-files.spec.md#soft-wraps)).
  [@test] ../../tests/e2e/commit-files/commit-review.spec.ts (`should show the tree beside the diffs of all files and follow J, K, N, P and the tree`)
- A review may show a part of its files (the branch review filters by code owner: by the path or the old path of a
  rename) and the owner of each file, muted in the header of its diff and in the hover of its tree row; a tree row
  keeps its width for the name. A filter that keeps nothing says why ("No files of Air Plugin in this range").
  [@test] ../../tests/e2e/branch-review/branch-review.spec.ts (`should review a stacked branch on its parent, then the whole stack, by owner`)

### The search of the files

- From 8 files on (`FILE_FILTER_MIN_FILES`, the count of the branch filter), or while it has a text, the sidebar must
  have a search of the files above their header, the search of the branch table (`SearchField`). A review of fewer
  files has none.
  [@test] ../../tests/e2e/commit-files/commit-review.spec.ts (`should show the tree beside the diffs of all files and follow J, K, N, P and the tree`)
- The tree and the diffs must show the files with every word of the text in their path or the old path of a rename,
  ignoring case, as it is typed; the header says "Files 3 of 9". The directories of what it found are open, the
  selected file stays while the search keeps it, else the first one is selected, and the diffs show it at the top.
  It applies on top of the filter of the surface (the owner chips of a branch review) and holds for the next commit
  (⌥↑/⌥↓) until the window closes.
  [@test] ../../src/lib/file-filter.test.ts (`keeps the files with every word in their path, ignoring case`, `finds a renamed file by its old path`)
  [@test] ../../tests/e2e/commit-files/commit-review.spec.ts (`should filter the files of a review by their path, with / from the diffs, Escape and Clear Filter`)
- `/` must focus the search from anywhere in the window and select its text, except in another input, a menu or a
  dialog. The letters typed into it are text, never J, K, N, P or an action of the window (S, O). Escape must clear
  the text, and in an empty search give the diffs the focus (the next Escape closes the window); ↓ must move the
  focus to the tree. A search that keeps nothing says "No files match “parser”" with Clear Filter.
  [@test] ../../tests/e2e/commit-files/commit-review.spec.ts (`should filter the files of a review by their path, with / from the diffs, Escape and Clear Filter`)

## User Experience

- The island of the review fills the window below its title bar; the panes scroll on their own. A 1px `bg-border`
  divider separates them.
- The sidebar has the commits above a `border-b`, then the search of the files (`SearchField` `w-full`: the
  `SearchIcon`, the placeholder "Filter files", `Kbd` `/` while empty, the X "Clear the filter text" once typed), then
  the header of the files ("Files 5", "Files 3 of 9" while searched, a muted `text-xs font-medium`
  `SectionLabel`, at least `h-10`), then the tree with the rows of the Files section. The selected row is `bg-primary/15`; a file row has no
  chevron, its name keeps the indent.
- A file header is `h-9` on `bg-card` with a `border-b` (`FileHeaderParts`, shared with the conflict and
  missing-commits accordions): a ghost `icon-xs` chevron (disabled without lines to show),
  the file as every list of changed files names it (`FilePathLabel`: the status letter in `font-mono text-xs` in the
  status colour, the name `font-medium` in the status colour, the directory muted after it), the copy button on hover, then on the right the notice and the owner muted and the coloured counts in `text-xs`.
- The review window ([Title Bar](../repository/title-bar.spec.md)) names the list and the commit in its title bar
  ("session › 11346cdf"), the toggle at its other end; the native title stays "Review 11346cdf <subject>".
- The footer shows "File 3 / 12" on the left, "Commit 2 / 3 · File 3 / 12" in a list of several commits, and on the
  right the hints `J K Files`, `N P Changes`, `⌥↑ ⌥↓ Commits` (`Alt+↑ Alt+↓` off macOS, not for a list of one) and
  `Esc Close`.
- The preview's footer keeps `D Review Changes` (the window opens), `F Files`, `E Edit` and `Space Close`.

## Testing / Local Run

- `pnpm test:unit`: `src/lib/diff/rows.test.ts`, `layout.test.ts`, `copy-text.test.ts`, `find.test.ts`, and the tests of [Commit Files](commit-files.spec.md) for
  the tree and the patch.
- `pnpm e2e`: `tests/e2e/commit-files/commit-review.spec.ts` on the `commit_tree`, `commit_many_files` (nine files,
  the search) and `issue_links` templates. A
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
