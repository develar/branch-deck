---
name: Commit Review
description: Requirements for the review of a commit in its window, and of the range of a pull request or a branch: a sidebar (the commits of a range, the header of the files, their directory tree) beside the diffs of all of them in one virtualized view, with keys for the files, the changes and the commits.
style: plain-1
targets:
  - src/components/commit/CommitReview.tsx
  - src/components/commit/CommitPreviewDialog.tsx
  - src/components/diff/CommitReviewCode.tsx
  - src/components/diff/DiffView.tsx
  - src/components/diff/DiffFindBar.tsx
  - src/components/diff/DiffFileMap.tsx
  - src/components/diff/diff-painter.ts
  - src/components/diff/use-measured-heights.ts
  - src/components/diff/diff-view.css
  - src/components/files/FilePathLabel.tsx
  - src/lib/sub-window.ts
  - src/lib/open-timing.ts
  - src/lib/review-window-reads.ts
  - crates/app-api/src/reads/diffs.rs
  - crates/app-api/src/app/windows.rs
  - src/components/files/FileTree.tsx
  - src/components/files/FileContextMenu.tsx
  - src/components/shared/WindowPage.tsx
  - src/pages/ReviewPage.tsx
  - src/lib/diff/rows.ts
  - src/lib/diff/find.ts
  - src/lib/diff/find-text.ts
  - src/lib/diff/layout.ts
  - src/lib/diff/file-map.ts
  - src/lib/diff/view-files.ts
  - crates/file-diffs/src/collapse.rs
  - crates/generated-files/src/names.rs
  - src/presenters/collapse.ts
  - src/presenters/review-card-end.ts
  - src/lib/file-tree.ts
  - src/lib/preview-keys.ts
  - src/stores/file-diffs.ts
  - src/stores/blob-texts.ts
  - src/stores/commit-files.ts
  - src/lib/commit-files.ts
  - src/lib/review-window.ts
  - src/components/commit/CommitList.tsx
  - src/hooks/useCommitReviewActions.ts
  - src/components/preview/CommitsPreviewBody.tsx
  - src/hooks/usePreviewHotkeys.ts
  - src/components/review/RangeReview.tsx
  - src/components/review/ChapterOpening.tsx
  - src/components/commit/CommitBodyView.tsx
  - src/components/shared/Disclosure.tsx
  - src/components/review/ReviewKeyHints.tsx
  - src/components/shared/DiffViewToggle.tsx
  - src/lib/review-position.ts
  - src/router.ts
  - src-tauri/capabilities/sub-windows.json
  - src/components/shared/SearchField.tsx
  - src/hooks/useSlashFocus.ts
  - src/lib/file-filter.ts
  - src/lib/text-search.ts
  - src/components/files/FileHeaderParts.tsx
  - crates/sync-core/src/uncommitted_changes.rs
---

# Commit Review

Status: Implemented
Date: 2026-10-15

## Purpose

The commit preview shows a commit, as Quick Look shows a file; the review reads it, in a window of its own, as ⌘O
opens a file: D on a row, the item of its menu or D in the preview opens the review. A commit lands in master with its
branch ([Landing](../../../../docs/design/landing.md)), so a commit of a branch is reviewed in the review window of its
branch, at the commit, with the stack, the actions and the other commits of the branch beside it
([Branch Review](../../branch/branch-review/spec.md)), a commit of an unapplied or archived branch in the review of that
branch ([Review Window](../../review/review-window/spec.md#kept-branches)); a commit no branch review shows (unassigned, of
a branch not built on its parents, of a conflict) opens in the same window as a list of commits
([Review Window](../../review/review-window/spec.md#one-window)). A sidebar with
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
- No review of uncommitted changes; the amend card shows them ([Amend Changes](../amend/spec.md)).
- The search of the sidebar finds files by their path, not text in them; the find in the review searches story text and diff rows.

## Requirements

### Requirement: Opening and closing: The review of a commit is in the review window

The review of a commit SHALL be in the review window ([Review Window](../../review/review-window/spec.md#one-window)):
the review of its branch, at the commit, or the review of the list it is in.

#### Scenario: The review of the list a commit is in
- **WHEN** a commit is reviewed in the review of the list it is in
- **THEN** the window is titled "Review a1b2c3d <subject>" and retitled as it steps
- **AND** [Title Bar](../../repository/title-bar/spec.md) says how it opens

### Requirement: Opening and closing: One action reviews, and there is no review dialog

There MUST be no review dialog, no W and no "Open in Window": one action, "Review Changes" (D).

#### Scenario: The menu of a commit row has one review item
- **WHEN** the user opens the menu of a commit row
- **THEN** "Review Changes" (D) is its review item, and there is no "Open in Window"
- **AND** no W and no dialog review the commit
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should review a commit of a row in its window with D and with the item of its menu`)

### Requirement: Opening and closing: A commit of a branch opens the review of its branch

A commit of a branch MUST open the review window of the branch at the commit (`reviewBranchCommit`: the copy of the
commit in the virtual branch, `focus.commitId`, and the file, `focus.path`), titled "Review login · a1b2c3d", from
every surface below.

#### Scenario: A commit of a stacked branch
- **WHEN** the user reviews a commit of a stacked branch
- **THEN** the review window of the branch opens at the copy of the commit, titled "Review login · a1b2c3d"
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should review a commit of a stacked branch with its stack and its actions, and move to a commit of the chain`)

### Requirement: Opening and closing: A commit no branch review shows opens the review of its list

A commit of no branch, of a branch the branch review can't show (blocked, failed, in conflict: `reviewBlockerOf`, the
Review Changes Rust offers) or one its virtual branch doesn't have yet (no copy, a conflicting or blocked commit) MUST
open the review of its list (`openCommitReviewWindow`).

#### Scenario: An unassigned commit
- **WHEN** the user reviews a commit of no branch
- **THEN** the review of its list opens
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should review an unassigned commit in the review window of the commit`)

#### Scenario: A commit of a branch in conflict or of a blocked one
- **WHEN** the user reviews a commit of a branch the branch review can't show (blocked, failed, in conflict)
- **THEN** the review of its list opens
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should review a commit of a branch in conflict and of a blocked one in the review window of the commit`)

#### Scenario: A commit its virtual branch doesn't have yet
- **WHEN** the user reviews a commit its virtual branch doesn't have yet (no copy, a conflicting or blocked commit)
- **THEN** the review of its list opens

### Requirement: Opening and closing: Only the main window decides on the review of a branch

Only the main window MUST decide on the review of a branch: a list in a sub-window opens the review of the list.

#### Scenario: A commit of a list in a sub-window
- **WHEN** the user reviews a commit of a list in a sub-window
- **THEN** the review of the list opens

### Requirement: Opening and closing: D and Review Changes on a row open the window at the first file

D on a commit row (the focused row while the pointer hasn't moved, else the row under it, `usePreviewHotkeys`) and
"Review Changes" (D), the first item of the row's menu, MUST open the window at the first file; no preview opens.

#### Scenario: D over a row and the item of its menu
- **WHEN** the user presses D over a commit row, or chooses "Review Changes" (D), the first item of its menu
- **THEN** the window opens at the first file
- **AND** no preview opens
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should review a commit of a row in its window with D and with the item of its menu`)

### Requirement: Opening and closing: A commit without files gets its preview

A commit without files MUST get its preview, which says so, in place of the review; the menu item MUST be disabled
with "The commit changes no files".

#### Scenario: D on a commit that changes no files
- **WHEN** the user presses D on a commit without files, or opens its menu
- **THEN** its preview opens and says so
- **AND** the menu item is disabled with "The commit changes no files"

### Requirement: Opening and closing: A selection of several commits is reviewed together

D and the item on a row of a selection of several MUST review them together: inside a branch the review of the branch
at the first of them in the order of the list (the focus of a review is one commit, and its column lists the others),
in a list no branch owns (unassigned) a review of the selection as a list of its own.

#### Scenario: A selection of commits of a branch
- **WHEN** the user presses D on a row of a selection of several commits of a branch
- **THEN** the review of the branch opens at the first of them in the order of the list
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should review a selection of commits of a branch in the review of the branch, at the first of them`)

#### Scenario: A selection of unassigned commits
- **WHEN** the user presses D on a row of a selection of several commits of a list no branch owns (unassigned)
- **THEN** a review of the selection opens as a list of its own
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should review a selection of several unassigned commits together, as a list of its own`)

### Requirement: Opening and closing: An open menu keeps D and Space, a closing one does not

An open menu MUST keep D and Space (they act on no row under it); a closing one MUST NOT, so D right after Escape
reviews.

#### Scenario: D under an open menu, then after Escape
- **GIVEN** the menu of a commit row is open
- **WHEN** the user presses D
- **THEN** nothing is reviewed
- **AND** D right after Escape reviews the row
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should review a commit of a row in its window with D and with the item of its menu`)

### Requirement: Opening and closing: The preview opens the review with D, its footer button and Enter

In the preview, D and the `D Review Changes` button of its footer MUST open the window at the first file, Enter on a
file of the Files tree at that file; the preview closes once the window opened, and stays (the error told) when it
could not.

#### Scenario: D in the preview
- **WHEN** the user presses D in the preview
- **THEN** the window opens at the first file and the preview closes
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should show the tree beside the diffs of all files and follow J, K, N, P and the tree`)

#### Scenario: Enter on a file of the tree and the footer button
- **WHEN** the user presses Enter on a file of the Files tree of the preview
- **THEN** the window opens at that file and the preview closes
- **AND** the `D Review Changes` button of its footer opens it at the first file
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should open the review at a file with Enter in the tree, and with the footer button`)

#### Scenario: The window could not open
- **WHEN** the window could not open
- **THEN** the preview stays, the error told

### Requirement: Opening and closing: The close of the preview focuses its list at the commit it showed

The close of the preview (also by D) MUST focus its list, and the keys of a row (Space, D) then act on the commit it
showed last, also after ↑/↓, while the pointer is on no row; a click of a row makes it the anchor again.

#### Scenario: The keys go on from the commit the preview showed last
- **GIVEN** the preview moved to another commit with ↑/↓ and the pointer is on no row
- **WHEN** the preview closes (also by D)
- **THEN** its list has the focus, and Space and D act on the commit it showed last
- **AND** a click of a row makes it the anchor again
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should give the focus to the list after the preview, its keys going on from the commit it showed last`)

### Requirement: Opening and closing: A list in a sub-window opens the window with its repository

A list in a sub-window (the missing commits of a conflict) MUST open the same window, with the repository of that
window (`repositoryPath` of its data).

#### Scenario: A missing commit of a conflict
- **WHEN** the user reviews a commit of the missing commits window of a conflict
- **THEN** the review window opens with the repository of that window
  [@test] tests/e2e/conflict-viewer/conflict-viewer.spec.ts (`should review a missing commit in the review window from the missing commits window`)

### Requirement: Opening and closing: The window reads nothing of the list but what it gets

The window MUST get the list as it was (its commits don't change), its name and the commit, and read nothing else of
it.

#### Scenario: The list of a review window
- **WHEN** a review of a list opens
- **THEN** the window gets the list as it was, its name and the commit
- **AND** reads nothing else of it

### Requirement: Opening and closing: An open of a list goes on by the one rule of the window

How an open of a list goes on from the review on screen SHALL be the one rule of the window
([Review Window](../../review/review-window/spec.md#one-window)).

#### Scenario: An open of a list while a review is on screen
- **WHEN** a list is opened while a review is on screen
- **THEN** another commit of the list on screen moves there
- **AND** another list or an open at a file starts it over

### Requirement: Opening and closing: The commits of the list stand above the files

The window of a list of several commits SHALL show the commits of the list above its files (the commits of a range
review, without a scope), and every review of a list the commit at the top of the document of its diffs as the branch
preview heads one, so it scrolls away with the overview of the plan below it.

#### Scenario: A list of several commits
- **WHEN** the review of a list of several commits is shown
- **THEN** the commits of the list stand above its files, the commits of a range review without a scope

#### Scenario: A list of one commit
- **WHEN** the review of a list of one commit is shown
- **THEN** no column of commits stands above its files

#### Scenario: The head of the document of the diffs
- **WHEN** the review of a list shows a commit
- **THEN** the commit heads the document of its diffs: `CommitCard` with its `heading` header (the subject `text-md font-medium`, the meta line with the committer time on hover and the coloured line counts, the reading body whole)
- **AND** it scrolls away with the overview of the plan below it

### Requirement: Opening and closing: The footer of the review of a list

The footer of the review of a list SHALL say "Commit 2 / 3 · File 1 / 1" and `Esc Close`.

#### Scenario: The footer of a list of several commits
- **WHEN** the review of a list of several commits is shown
- **THEN** its footer says "Commit 2 / 3 · File 1 / 1" and `Esc Close`

### Requirement: Opening and closing: The title bar names the list and the commit

The title bar of the review of a list SHALL name the list and the commit and end with the Split/Unified/Inline toggle;
it has no action and no "⋯". Nothing is edited there.

#### Scenario: The title bar of an unassigned commit
- **WHEN** the review of a list is shown
- **THEN** its title bar says "Unassigned › 11346cdf" (`listLabel` of the window data: the branch, or "Unassigned") and ends with the Split/Unified/Inline toggle
- **AND** it has no action and no "⋯", and nothing is edited there

### Requirement: Opening and closing: Alt and the arrows show the previous or next commit

⌥↑/⌥↓ (Alt+↑/↓) MUST show the previous or next commit of the list.

#### Scenario: Alt and an arrow
- **WHEN** the user presses ⌥↑ or ⌥↓ in the review of a commit, of its branch or of its list
- **THEN** the previous or next commit of the list is shown
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should switch the commit of a review with Alt and the arrows`)

### Requirement: Opening and closing: The subject of a review is a commit or a range

The review SHALL have a subject: a commit, or the range `base..head` of a pull request that waits for the user's
review ([Review Requests](../../repository/review-requests/spec.md#review-window)) or of a branch in its review window
([Branch Review](../../branch/branch-review/spec.md)).

#### Scenario: The range of a pull request
- **WHEN** the user reviews a pull request that waits for their review
- **THEN** the review shows its range `base..head` with the file tree
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should preview the pull request, fetch it with progress, then review its range with the file tree`)

### Requirement: Opening and closing: One view shows a commit and a range

The same view MUST show both subjects: the loading line, the error ("Cannot read the changes of the commit", "… of the
pull request") and the empty state are one.

#### Scenario: A subject that loads, cannot be read or is empty
- **WHEN** a commit or a range loads, cannot be read or has no files
- **THEN** the same view shows the loading line, the error ("Cannot read the changes of the commit", "… of the pull request") or the empty state

### Requirement: Opening and closing: A range is keyed by base..head and dropped with its review

A range MUST be keyed `<base>..<head>` in the stores of the files and the diffs and dropped when its review closes.

#### Scenario: The range in the stores of the files and the diffs
- **WHEN** a review of a range loads and then closes
- **THEN** the files and the diffs of the range are keyed `<base>..<head>`
- **AND** they are dropped when its review closes
  [@test] src/stores/commit-files.test.ts (`loads the range of a review by its id, base..head, until its review drops it`)
  [@test] src/stores/file-diffs.test.ts (`keeps the last subjects only, and forgets the subjects of a closed review under every policy`)

### Requirement: The sidebar and the diffs: The left pane is a sidebar of the commits, the header and the tree

The left pane MUST be a sidebar: the commits of a range on top, at most 40% of its height and scrolling inside; then
the header of the files; then the tree of the Files section, in the order of the tree, a file row selecting the file.
The selection follows the focus.

#### Scenario: The sidebar of a range of several commits
- **WHEN** a review of a range of several commits is shown ([Branch Review](../../branch/branch-review/spec.md), [Review Requests](../../repository/review-requests/spec.md#review-window))
- **THEN** its commits are on top of the sidebar, at most 40% of its height and scrolling inside
- **AND** below them the header of the files ("5 files", or what the surface puts there: the owner chips of a branch review), then the tree of the Files section in the order of the tree
- **AND** a file row selects the file, and the selection follows the focus

### Requirement: The sidebar and the diffs: The sidebar is resizable and keeps its width

The sidebar MUST be resizable (24rem at first, what the names of a tree need, not a share of the window; never narrower
than 16rem, at most 50%) and keep its width (`review-sidebar`, one width for every review).

#### Scenario: The width of the sidebar
- **WHEN** a review opens
- **THEN** its sidebar is 24rem wide at first, what the names of a tree need, not a share of the window
- **AND** a title of a step, task or commit in the tree wraps as many lines as needed, and a file row stays one line
- **AND** a resize keeps it between 16rem and 50%, the width (`review-sidebar`) kept for every review
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should give the sidebar the width of its names, and hide and show it with F and its button, the choice kept`)

### Requirement: The sidebar and the diffs: F and the sidebar button hide and show the sidebar

F and the sidebar button of the title bar MUST hide and show the sidebar, so the diffs take the whole window; the footer
names F, and the choice is kept for every review and never changes by itself.

#### Scenario: F hides the sidebar, and the choice is kept
- **WHEN** the user presses F or the sidebar button of the title bar
- **THEN** the sidebar hides, and the diffs take the whole window; the same shows it again
- **AND** the footer names F, and the choice is kept for every review and never changes by itself
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should give the sidebar the width of its names, and hide and show it with F and its button, the choice kept`)

### Requirement: The sidebar and the diffs: The divider of Split drags and keeps its share

In Split, the divider between the two sides MUST drag, as IntelliJ's DiffSplitter, between 15% and 85% of the row; a
double click makes the sides equal. The share is one setting of every diff view (`splitRatio`), kept, and no view
changes it by itself.

#### Scenario: A drag and a double click of the divider
- **WHEN** the user drags the divider of the sides of Split
- **THEN** the share moves between 15% and 85% of the row and is kept as `splitRatio` for every diff view
- **AND** a double click makes the sides equal
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should drag the divider of the sides of Split, keep its share, and make the sides equal with a double click`)

### Requirement: The sidebar and the diffs: The diffs say they are dragged while the divider is held

While the press holds the divider, the diffs MUST say they are dragged: the cursor stays the resize one over the whole
view, where the pointer left the hit area of the line long ago, and the run over the rows selects no code.

#### Scenario: The press holds the divider
- **WHEN** the press holds the divider and the pointer leaves the hit area of the line
- **THEN** the diffs say they are dragged, the cursor stays the resize one over the whole view
- **AND** the run over the rows selects no code
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should drag the divider of the sides of Split, keep its share, and make the sides equal with a double click`)

### Requirement: The sidebar and the diffs: The split is there in every state of the review

The split MUST be there in every state of the review: while another commit or filter loads, has no files or cannot be
read, the diffs pane says so, and the commits and the header of the files stay where they are, with their focus.

#### Scenario: Another subject loads
- **WHEN** another commit or filter loads, has no files or cannot be read
- **THEN** the diffs pane says so
- **AND** the commits and the header of the files stay where they are, with their focus
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should review a stacked branch on its parent, then the whole stack, by owner`)

### Requirement: The sidebar and the diffs: The diffs of all files are cards in one virtualized view

The right pane MUST show the diffs of all files in the order of the tree, in one virtualized view, each file a card as
in IntelliJ's combined diff: 10 px apart on the canvas, so a card stands out from it as an island does, a 1-px border,
rounded corners, a collapsed file its header alone. The commit a range shows is above them.

#### Scenario: The cards of the files
- **WHEN** the review shows its files
- **THEN** the diffs of all of them stand in the order of the tree in one virtualized view, each a card 10 px apart on the canvas with a 1-px border and rounded corners
- **AND** a collapsed file is its header alone
  [@test] src/lib/diff/layout.test.ts (`counts the gaps, the space around the cards and the bottom border of each card, so each offset stays exact`)
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should show the tree beside the diffs of all files and follow J, K, N, P and the tree`)

#### Scenario: The commit of a range
- **WHEN** the review shows a range
- **THEN** the commit it shows is above the cards

### Requirement: The sidebar and the diffs: The header of a card is a band that sticks and casts a shadow

The header of a file MUST be the top of the card, a band of its own colour between the card and the canvas, so it never
reads as one more row of code; it sticks while its rows scroll, until the end of the card pushes it up, and once a row
of them has gone under it, it casts a shadow on them.

#### Scenario: A header stuck over its rows
- **WHEN** the rows of a card scroll under its header
- **THEN** the header sticks until the end of the card pushes it up
- **AND** it casts a shadow once a row has gone under it, and none in its place
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should show the tree beside the diffs of all files and follow J, K, N, P and the tree`)

### Requirement: The sidebar and the diffs: The header of a card names the file and holds its actions

The header MUST show the status letter, the name semibold, where a rename came from and its directory muted, Expand All
with its tooltip, its owner, its line counts, a copy button and the menu of the file
([Commit Files](../commit-files/spec.md#the-actions-on-a-file)); the name of every file stands at one x, and the
directory gives way, from its start, before the name.

#### Scenario: The headers of the files
- **WHEN** the review shows its cards
- **THEN** each header shows the status letter, the name semibold, the directory muted, Expand All, the owner, the line counts, a copy button and the menu of the file
- **AND** the name of every file stands at one x, and the directory gives way, from its start, before the name
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should show the tree beside the diffs of all files and follow J, K, N, P and the tree`)

### Requirement: The sidebar and the diffs: The opening of a step stands on the canvas above its card

With the review plan, a section that starts a step MUST have its opening on the canvas between the gap above its card
and the card, as tall as its words, and one that starts a group and no step a divider there, never words in the header;
they scroll with the card, and a scroll to the section shows its opening.

#### Scenario: A section that starts a step or a group
- **GIVEN** the review plan
- **WHEN** a section starts a step, or a group and no step ([Review Tour](../../branch/review-tour/spec.md#in-the-review-window))
- **THEN** its opening, as tall as its words, or a divider stands between the gap above its card and the card, never words in the header
- **AND** it scrolls with the card, and a scroll to the section shows it
  [@test] src/lib/diff/layout.test.ts (`puts the opening of a step at its measured height and the divider of a group between the gap and its card: a scroll to the file shows it, a y in it is the file below`)

### Requirement: The sidebar and the diffs: What comes before the first section lies above it

What comes before the first section (the overview of a review) MUST lie above it, a gap of 10 px between them.

#### Scenario: The overview of a review
- **WHEN** the review has an overview before its first section
- **THEN** the overview lies above it, a gap of 10 px between them
  [@test] src/lib/diff/layout.test.ts (`puts the prologue before the first section, a gap below it, keeps a top in it from the top of the document and one in the gap at the first section`)

### Requirement: The sidebar and the diffs: A new height of an opening keeps the reader's place

A new height of an opening (its words wrap at a new width, a font loads, a part of it opens, a title arrives) MUST keep
the reader's place: an opening grows and shrinks below the top of the view, the cards below it move, and a top in a
card keeps its line.

#### Scenario: An opening grows or shrinks
- **WHEN** an opening gets a new height
- **THEN** it grows and shrinks below the top of the view, and the cards below it move
- **AND** a top in a card keeps its line
  [@test] src/lib/diff/layout.test.ts (`keeps a top in an opening from the start of the opening, so an opening that grows grows below it and one that shrinks gives the top to its card`)

### Requirement: The sidebar and the diffs: A change, a hit and a declaration land in their card

A change N and P show, a hit of the find and a declaration shown MUST land in their card, the opening of its step above
the view, so their place on screen never depends on the height of the opening.

#### Scenario: N and P, a hit of the find, a declaration
- **WHEN** N or P shows a change, the find goes to a hit or a declaration is shown
- **THEN** it lands in its card, the opening of its step above the view
  [@test] src/lib/diff/layout.test.ts (`shows a change, a hit of the find and a declaration in a card below the opening of its step: the top of the card is their least top`)

### Requirement: The sidebar and the diffs: A file chosen in the tree scrolls its header to the top

A file chosen in the tree MUST scroll its header to the top.

#### Scenario: A row of the tree
- **WHEN** the user chooses a file in the tree
- **THEN** the diffs scroll its header to the top
- **AND** the last file, whose header can never reach the top, is shown and selected all the same
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should show the tree beside the diffs of all files and follow J, K, N, P and the tree`)

### Requirement: The sidebar and the diffs: The file whose header is at the top is the selected one

As the diffs scroll, the file whose header is at the top MUST become the selected file, its directories opened;
scrolled to their end, the last file, in view entire while its header can never reach the top, is the selected one,
the position and the map following it.

#### Scenario: The diffs scroll
- **WHEN** the diffs scroll
- **THEN** the file whose header is at the top becomes the selected file, its directories opened
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should scroll the diffs with a map of the files in place of their scrollbar: a click shows the file under it, the hover names it, the thumb drags and follows`)

#### Scenario: The diffs scrolled to their end
- **WHEN** the diffs are scrolled to their end and the header of the last file can never reach the top
- **THEN** the last file, in view entire, is the selected one, the position and the map following it
  [@test] src/lib/diff/layout.test.ts (`shows the last file at the end of the document, whose header can never reach the top, else the file at the top`)

### Requirement: The sidebar and the diffs: A scroll the review made selects through its key or row

A scroll the review made (a choice, J, K, N, P) MUST change the selection only through the key or the row that made it,
also for a last file that cannot reach the top.

#### Scenario: A choice of the last file
- **WHEN** a choice, J, K, N or P scrolls the diffs, also to a last file that cannot reach the top
- **THEN** the selection is the one of the key or the row that made the scroll
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should show the tree beside the diffs of all files and follow J, K, N, P and the tree`)

### Requirement: The sidebar and the diffs: The top of the diffs keeps the gap of the first card

The top of the diffs MUST keep the gap of 10 px that the first card has above it: a file at the top and a header stuck
while its rows scroll under it sit 10 px below the header of the commit, never against its line, and no row shows in
the gap.

#### Scenario: A file at the top and a stuck header
- **WHEN** a file is at the top, or its header is stuck while its rows scroll under it
- **THEN** it sits 10 px below the header of the commit, never against its line
- **AND** no row shows in the gap
  [@test] src/lib/diff/layout.test.ts (`sticks a header below the space above the first card, so a file at the top keeps that space over its card`)
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should show the tree beside the diffs of all files and follow J, K, N, P and the tree`)

### Requirement: The sidebar and the diffs: J and K select the next and the previous file

J and K MUST select the next and the previous file.

#### Scenario: J and K in the diffs
- **WHEN** the user presses J or K
- **THEN** the next or the previous file is selected
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should show the tree beside the diffs of all files and follow J, K, N, P and the tree`)

### Requirement: The sidebar and the diffs: With the review plan J and K step through its stops

With the review plan of a branch, whose diffs are the story of its units, J and K MUST select the next and the previous
stop of the document (the overview, the opening of a step, a card), the tree and the bar of the step above the diffs
following ([Review Units](../../branch/review-units/spec.md#in-the-review-window),
[Review Tour](../../branch/review-tour/spec.md#in-the-review-window)).

#### Scenario: J and K in the story of a branch
- **GIVEN** the review plan of a branch
- **WHEN** the user presses J or K
- **THEN** the next or the previous stop of the document is shown (the overview, the opening of a step, a card)
- **AND** the tree and the bar of the step above the diffs follow
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should show a repeated edit once under its group, the other files of the edit collapsed, and a file's own change in its section with a row to the edit`)

### Requirement: The sidebar and the diffs: N and P show the next and the previous change

N and P MUST show the next and the previous block of changed lines, into the next or the previous file at the end of
one; collapsed files are skipped.

#### Scenario: N and P through the blocks and the files
- **WHEN** the user presses N or P
- **THEN** the next or the previous block of changed lines is shown, into the next or the previous file at the end of one
- **AND** collapsed files are skipped
  [@test] src/lib/diff/rows.test.ts (`starts N and P at each block of changed lines, a deleted one too, never at a block the policy ignores`)
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should show the tree beside the diffs of all files and follow J, K, N, P and the tree`, `N and P skip a block the whitespace policy ignores`)

### Requirement: The sidebar and the diffs: A long card and the last card end with what J shows next

A card of more than 30 rows, whose end is far from its header, and the last card MUST end with a line that names what J
shows next, by its rows alone, never by the viewport.

#### Scenario: A long card and a short one
- **WHEN** a card has more than 30 rows, or is the last card
- **THEN** it ends with a line one row high that names what J shows next, by its rows alone, never by the viewport
  [@test] src/lib/diff/layout.test.ts (`ends a card of more than 30 rows and the last card with a line one row high, below the rows, which the offsets count`)

### Requirement: The sidebar and the diffs: The end of a card names the next file as its header does

The line at the end of a card MUST name the next file as the header of its card names it, the name and then the
directory, which gives way from its start, so its last folder shows (ten files named SKILL.md in a row differ by it).

#### Scenario: The next file and the next section of the same file
- **WHEN** a card ends with the line of what J shows next
- **THEN** it says "Next: SKILL.md .claude/skills/setup-local-rd-flow J", the name and then the directory
- **AND** it says "Next: SKILL.md, continued" for the next section of the same file
  [@test] src/presenters/review-card-end.test.ts (`names the next file as its header does, its name and its directory, and the next part of the same file as continued`)

#### Scenario: Under an owner chip
- **WHEN** an owner chip is chosen
- **THEN** the line names the next of its files
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should navigate the long card to the next visible file of the chosen owner chip`)

#### Scenario: The opening of a step of no card
- **WHEN** the last card stands above the opening of a step of no card after it
- **THEN** the line says the words of that step ("Next: Step 2 · Close moves to b")
  [@test] src/presenters/review-card-end.test.ts (`names after the last card the opening of a step of no card below it by the words of the step`)

### Requirement: The sidebar and the diffs: The end of a card says the end of the review

With nothing left, the line at the end of a card MUST say "End of the review", and under an owner chip before the last
card "No more files of Design".

#### Scenario: Nothing left
- **WHEN** nothing is left after the card
- **THEN** its line says "End of the review"
- **AND** under an owner chip, before the last card, "No more files of Design"
  [@test] src/presenters/review-card-end.test.ts (`ends the review at the last card, and the files of an owner chip before it`)

### Requirement: The sidebar and the diffs: A click of the end of a card shows what it names as J does

A click of the line at the end of a card MUST show what it names as J does: its header at the top, the tree and the
footer following, the focus left in the diffs, so J goes on from there.

#### Scenario: A click of the end of a long card
- **WHEN** the user clicks the line at the end of a card
- **THEN** the header of what it names is at the top, the tree and the footer follow
- **AND** the focus stays in the diffs, so J goes on from there
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should end a long card with a line naming the next file, which a click shows as J does, and the last card with the end of the review`)

### Requirement: The sidebar and the diffs: The end of a card never says the end of its file

The line at the end of a card MUST NOT say "End of <file>" (a section of the story may be a part of its file); it is out
of the tab order (J does it from the keys) and never in a copy of the code.

#### Scenario: The words, the tab order and a copy
- **WHEN** a card ends with its line
- **THEN** it never says "End of <file>"
- **AND** it is out of the tab order and never in a copy of the code
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should end a long card with a line naming the next file, which a click shows as J does, and the last card with the end of the review`)

### Requirement: The sidebar and the diffs: A map of the files stands in place of the scrollbar

The diffs MUST have a map of the files in place of their scrollbar: a column at their right, beside the diffs and not
above them, the whole review scaled to its height (never shorter than the view, so a short review keeps its files where
its cards are).

#### Scenario: The scale of the map
- **WHEN** the review shows its diffs
- **THEN** a column at their right, beside them, maps the whole review scaled to its height
- **AND** a review shorter than the view is scaled as the view, so its files stay where its cards are
  [@test] src/lib/diff/file-map.test.ts (`scales the document, never shorter than the view, to the height of the map, a tick at the start of each section`)

### Requirement: The sidebar and the diffs: The map has a tick at the start of each section

The map MUST have a tick at the start of each section, one across it at the start of a step of the plan; where the
pixels are too few for every section, the ticks of several are one.

#### Scenario: The ticks of the sections and the steps
- **WHEN** the map is drawn
- **THEN** each section has a tick at its start, a step of the plan one across the map
  [@test] src/lib/diff/file-map.test.ts (`scales the document, never shorter than the view, to the height of the map, a tick at the start of each section`)
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should open the review on its overview, then each step's opening above its first card, a step per commit with its story, its rounds and the tests that reach its code`)

#### Scenario: Too few pixels for every section
- **WHEN** the pixels are too few for every section
- **THEN** the ticks of several sections are one
  [@test] src/lib/diff/file-map.test.ts (`makes one tick of the sections whose pixels are too few, a step when any of them is, muted only when all are`)

### Requirement: The sidebar and the diffs: The window of the view is the thumb of the map

The section at the top MUST be filled, and the window of the view is the thumb of the map, its strongest mark and never
thinner than a mark to grab, even for a review of 500 files.

#### Scenario: The thumb of a review of many files
- **WHEN** the map is drawn, even for a review of 500 files
- **THEN** the section at the top is filled, and the thumb stands at the window of the view, never thinner than a mark to grab
  [@test] src/lib/diff/file-map.test.ts (`draws the thumb at the window of the view, at least the least height centred on it, all of it inside the map`)

### Requirement: The sidebar and the diffs: An owner chip filters the map

An owner chip MUST filter the map to its files with the diffs.

#### Scenario: A chosen owner chip
- **WHEN** the user chooses an owner chip
- **THEN** the map shows its files, as the diffs do

### Requirement: The sidebar and the diffs: The map scrolls, shows and names the sections

A drag of the thumb and the wheel over the map MUST scroll the diffs, the tree and the footer following the file at the
top; a click shows the section under the pointer as a row of the tree does, the diffs taking the focus, so J goes on
from there; the hover names its file, the directory after the name, and its step.

#### Scenario: A drag, the wheel, a click and the hover
- **WHEN** the user drags the thumb or turns the wheel over the map
- **THEN** the diffs scroll, the tree and the footer following the file at the top
- **AND** a click shows the section under the pointer as a row of the tree does, the diffs taking the focus
- **AND** the hover names its file, the directory after the name, and its step
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should scroll the diffs with a map of the files in place of their scrollbar: a click shows the file under it, the hover names it, the thumb drags and follows`)
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should open the review on its overview, then each step's opening above its first card, a step per commit with its story, its rounds and the tests that reach its code`)

### Requirement: The sidebar and the diffs: What comes before the first section is a band of the map

What comes before the first section MUST be a band of the map without a tick, "Overview" in its hover, which a click
shows.

#### Scenario: The overview in the map
- **WHEN** the review has an overview before its first section
- **THEN** the map has a band for it without a tick, "Overview" in its hover
- **AND** a click on it shows the overview
  [@test] src/lib/diff/file-map.test.ts (`leaves the prologue above the first section a band without a tick, which the hover and a click tell from the sections and the map fills at the top`)

### Requirement: The sidebar and the diffs: The map is neutral and outside the tab order

The map MUST be neutral at any count of files: no colour of a status. It is out of the tab order, hidden from a screen
reader and without the menu of a file: the tree, the headers and the keys reach every file.

#### Scenario: The map and the keys
- **WHEN** the map is drawn
- **THEN** it has no colour of a status
- **AND** it is out of the tab order, hidden from a screen reader and without the menu of a file

### Requirement: The sidebar and the diffs: The tree and the first headers show while git reads the patch

The tree and the headers of the first files MUST show as soon as the files are read, while git reads the patch;
"Loading changes…" only after 300 ms.

#### Scenario: A patch still read
- **WHEN** the files are read and git still reads the patch
- **THEN** the tree and the headers of the first files show
- **AND** "Loading changes…" shows only after 300 ms
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should show the files while the patch is read, settle with the diffs, and step to a commit read beside it`)

### Requirement: The sidebar and the diffs: The diffs mount before their texts

The diffs MUST mount with the diffs of the engine, before their texts: each file reads its texts by blob id as it comes
into view, and its rows show plain text until the tokens of their page arrive.

#### Scenario: A file comes into view
- **WHEN** a file comes into view
- **THEN** it reads its texts by blob id
- **AND** its rows show plain text until the tokens of their page arrive

### Requirement: The sidebar and the diffs: The window shows once the first screen is painted

The window MUST show once the rows of the first screen are painted with their texts, and their tokens are there or 80 ms
late ([Title Bar](../../repository/title-bar/spec.md)).

#### Scenario: The first frame of the window
- **WHEN** the review window opens
- **THEN** it shows once the rows of the first screen are painted with their texts, their tokens there or 80 ms late

### Requirement: The sidebar and the diffs: The neighbours of the shown commit are read after it

Once the diffs are all read (the stream of the shown subject ended, not at its first headers), the diffs of the commits
before and after it in the list MUST be read too, so ⌥↑/⌥↓ show them at once; their streams never compete with the
shown one for the workers of the backend.

#### Scenario: The stream of the shown subject ended
- **WHEN** the stream of the shown subject ended
- **THEN** the diffs of the commits before and after it in the list are read
- **AND** ⌥↑/⌥↓ show them at once
  [@test] src/stores/file-diffs.test.ts (`tells the end of a subject's read only once its stream ended: the neighbours of a review are read after it`)
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should show the files while the patch is read, settle with the diffs, and step to a commit read beside it`)

### Requirement: The sidebar and the diffs: The diffs have the focus when the review shows its subject

The diffs MUST have the focus when the review shows its subject, so the arrows, PageUp, PageDown and Space scroll them;
Shift+Tab goes to the tree.

#### Scenario: A subject is shown
- **WHEN** the review shows its subject
- **THEN** the diffs have the focus, and the arrows, PageUp, PageDown and Space scroll them
- **AND** Shift+Tab goes to the tree

### Requirement: The sidebar and the diffs: The review keeps the focus while no diffs are shown

While another commit loads, or one without files or that cannot be read is shown, the review itself MUST keep the
focus, so ⌥↑/⌥↓ and Escape still work.

#### Scenario: Another commit loads
- **WHEN** another commit loads, or one without files or that cannot be read is shown
- **THEN** the review keeps the focus, and ⌥↑/⌥↓ and Escape still work
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should review a stacked branch on its parent, then the whole stack, by owner`)

### Requirement: The sidebar and the diffs: What the user works in beside the diffs keeps the focus

What the user works in beside the diffs (the commits, the owner chips) MUST keep the focus when the diffs of the next
subject mount; J, K, N and P work from there too.

#### Scenario: The diffs of the next subject mount
- **GIVEN** the focus is in the commits or the owner chips
- **WHEN** the diffs of the next subject mount
- **THEN** the focus stays there, and J, K, N and P work from there too
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should review a stacked branch on its parent, then the whole stack, by owner`)

### Requirement: The sidebar and the diffs: A file without lines to show shows only its header

A binary file, a file without changed lines and a file the patch left out MUST show only their header, which says so.

#### Scenario: A binary file, a file without changed lines, a file left out
- **WHEN** the review shows a binary file, a file without changed lines or a file the patch left out
- **THEN** only its header shows, and it says so
  [@test] src/presenters/collapse.test.ts (`names the generated file by its name, and leaves a file without text and a large file to their notice and counts`)

### Requirement: The sidebar and the diffs: Generated, large and folded files start collapsed

Lock files (`pnpm-lock.yaml`, `Cargo.lock`, …), minified files, source maps and snapshot files, files with more than
1,000 changed lines and the files the review plan folds
([Review Plan](../../branch/review-plan/spec.md#in-the-review-window)) MUST start collapsed; the chevron of the header
expands them.

#### Scenario: A lock file and a minified file
- **WHEN** the review shows a lock file and a minified file
- **THEN** they start collapsed
- **AND** the chevron of the header expands one
  [@test] crates/file-diffs/src/collapse_test.rs (`a_generated_name_no_text_and_many_changed_lines_start_a_file_collapsed`)
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should start a lock file and a minified file collapsed, their header saying why, and open one with its chevron`)

### Requirement: The sidebar and the diffs: Rust decides the collapse, and the header says why

Rust MUST decide the collapse for every view of a diff (`collapse_of`, the `collapse` of each file's content), by the
names of `generated-files`; the header of a generated file says what its name makes it, the header of a folded file
why the plan folds it.

#### Scenario: The header of a generated file
- **WHEN** a file is generated by its name
- **THEN** its header says what its name makes it ("Lock file", "Minified", "Source map", "Snapshot")
  [@test] crates/generated-files/src/names_test.rs (`a_lock_file_a_minified_file_a_source_map_and_a_snapshot_are_generated_by_their_name`)
  [@test] src/presenters/collapse.test.ts (`names the generated file by its name, and leaves a file without text and a large file to their notice and counts`)
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should start a lock file and a minified file collapsed, their header saying why, and open one with its chevron`)

#### Scenario: The header of a folded file
- **WHEN** the review plan folds a file
- **THEN** its header says why the plan folds it

### Requirement: The sidebar and the diffs: The unchanged lines between hunks expand

The unchanged lines between hunks MUST expand as in the Files section ([Commit Files](../commit-files/spec.md)).

#### Scenario: A fold between two hunks
- **WHEN** the user expands the unchanged lines between two hunks
- **THEN** they show as in the Files section

### Requirement: The sidebar and the diffs: A copy gives the lines the rows show

A copy of a selection in the diffs MUST give the lines its rows show, from the texts, also of rows no longer on screen:
without the line numbers, the folds and the deleted parts of an Inline row, the old and the new line of a changed pair
in Unified, and one side in Split (a press on a side keeps the selection there).

#### Scenario: A copy of a selection
- **WHEN** the user copies a selection in the diffs, also of rows no longer on screen
- **THEN** the copy gives the lines its rows show, from the texts, without the line numbers and the folds
- **AND** the old and the new line of a changed pair in Unified, the new line of an Inline row without its deleted parts
  [@test] src/lib/diff/copy-text.test.ts (`copies the lines the rows show from the model, cut at both ends, in the order of the diffs`, `copies across files and leaves out a fold and a text not read yet`)

#### Scenario: A copy in Split
- **WHEN** the user presses on a side in Split and copies a selection
- **THEN** the selection stays on that side, and the copy gives one side
  [@test] src/lib/diff/copy-text.test.ts (`copies one side in Split, and the new line of an Inline row without its deleted parts`)
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should copy the lines of a selection of the diffs from the model, one side in Split, without the line numbers`)

### Requirement: The sidebar and the diffs: Select All selects the side of the file at the top

⌘A (Ctrl+A off macOS) MUST select the side of the file at the top, and a copy then gives all its shown lines.

#### Scenario: Select All, then a copy
- **WHEN** the user presses ⌘A (Ctrl+A off macOS) in the diffs and copies
- **THEN** the side of the file at the top is selected, and the copy gives all its shown lines

### Requirement: The sidebar and the diffs: Find opens a find bar in the review

⌘F (Ctrl+F off macOS) in the review MUST open a find bar labelled and placeholder "Find in the review".

#### Scenario: Find in the diffs
- **WHEN** the user presses ⌘F (Ctrl+F off macOS) in the review
- **THEN** a find bar labelled and placeholder "Find in the review" opens
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should find a text in the diffs from the model, also in the folded lines, with ⌘F, Enter and Escape`)

### Requirement: The sidebar and the diffs: The find searches the story and the diff rows as one document

The find MUST search the prologue, every opening (including steps told before their section), the epilogue and the diff
rows as one document, in order: prologue, opening of file *i*, diff rows of file *i*, then epilogue.

#### Scenario: The order of the hits
- **WHEN** the query is in the overview, an opening and the diff rows of a file
- **THEN** the hits come in order: prologue, opening of file *i*, diff rows of file *i*, then epilogue
  [@test] src/lib/diff/find.test.ts (`puts text before diff hits for each file and keeps each list's order`)
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should find overview and opening text before the matching diff rows, and open a folded follow-up`)

#### Scenario: The epilogue
- **WHEN** the query is in the epilogue
- **THEN** the find goes there and scrolls to the hit
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should search story text in the epilogue and scroll to the hit`)

### Requirement: The sidebar and the diffs: Story text is matched one block at a time

Story text MUST be matched ignoring case, one block at a time (never across paragraphs or list items); `.sr-only`,
`[aria-hidden=true]` and `[data-find-skip]` are excluded.

#### Scenario: Case and blocks of the story
- **WHEN** the query is in story text in another case, several times in one block, or across two blocks
- **THEN** it matches ignoring case, every hit of the block
- **AND** it never matches across paragraphs or list items, and an empty query has no hits
  [@test] src/lib/diff/find.test.ts (`matches ignoring case and finds several hits in one segment`, `does not match across segments and returns no hits for an empty query`)

#### Scenario: Text the find skips
- **WHEN** the query is in `.sr-only`, `[aria-hidden=true]` or `[data-find-skip]` text
- **THEN** it does not match there

### Requirement: The sidebar and the diffs: Closed parts of the story are searched and open at a hit

Closed story-body parts and `Disclosure`s MUST be searched always and open when the find goes to a hit; "In folded lines"
applies to diff rows only.

#### Scenario: A hit in a closed story body
- **WHEN** the find goes to a hit in a closed story-body part or a closed `Disclosure`
- **THEN** it opens
- **AND** "In folded lines" does not decide it, as it applies to diff rows only
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should open a hit in a closed story body and keep it current`)

### Requirement: The sidebar and the diffs: The diff search reads the model

The diff search MUST remain model-based: both sides of a change in Split, the new side of an unchanged line, and with
the checkbox folded diff lines too.

#### Scenario: The rows, the sides and the folds
- **WHEN** the query is in the diff rows
- **THEN** it matches in the lines the rows show, ignoring case, in the order of the rows
- **AND** both sides of a change in Split and the new side of an unchanged line, and with the checkbox the folded lines too
  [@test] src/lib/diff/find.test.ts (`finds the query in the lines the rows show, ignoring case, in the order of the rows`, `searches both sides of a change in Split and the new side of an unchanged line, and the folds on request`)
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should find a text in the diffs from the model, also in the folded lines, with ⌘F, Enter and Escape`)

### Requirement: The sidebar and the diffs: The find counts every hit and steps through them

The count MUST combine text and diff hits ("12 results", "3 of 12"); Enter and ⌘G go to the next one, Shift with them to
the previous one.

#### Scenario: The count and the steps
- **WHEN** the find has hits in text and in diff rows
- **THEN** the count combines them ("12 results", "3 of 12")
- **AND** Enter and ⌘G go to the next one, Shift with them to the previous one
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should find a text in the diffs from the model, also in the folded lines, with ⌘F, Enter and Escape`)

### Requirement: The sidebar and the diffs: The current hit stays the same hit

The current hit MUST stay the same hit when a fold opens or story text arrives.

#### Scenario: A fold opens or story text arrives
- **WHEN** a fold opens or story text arrives while a hit is current
- **THEN** the current hit stays the same hit
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should open a hit in a closed story body and keep it current`)

### Requirement: The sidebar and the diffs: The painter and the highlights mark the hits

The painter MUST mark diff hits; the CSS Custom Highlight API marks text hits in the same colours, with scrolling still
working when that API is unavailable.

#### Scenario: The marks of the hits
- **WHEN** the find has hits
- **THEN** the painter marks the diff hits, grouped by file and line, the current one marked
- **AND** the CSS Custom Highlight API marks the text hits in the same colours, the scroll working without it
  [@test] src/lib/diff/find.test.ts (`groups the hits by file and line for the painter, the current one marked`)

### Requirement: The sidebar and the diffs: The find works anywhere in the review window

⌘F (Ctrl+F off macOS) MUST work anywhere in the review window, including the sidebar; ⌘G and Shift+⌘G step while the
bar is open, including with focus in the sidebar.

#### Scenario: Find from the file tree
- **GIVEN** the focus is in the sidebar
- **WHEN** the user presses ⌘F, then ⌘G and Shift+⌘G
- **THEN** the find bar opens and steps through the hits
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should open find from the file tree and step results there with ⌘F and ⌘G @webkit`)

### Requirement: The sidebar and the diffs: Escape closes the find bar

Escape MUST close the find bar, clear the marks and give the scroller the focus.

#### Scenario: Escape in the find bar
- **WHEN** the user presses Escape in the find bar
- **THEN** the bar closes and the marks are cleared
- **AND** the scroller has the focus
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should find a text in the diffs from the model, also in the folded lines, with ⌘F, Enter and Escape`)

### Requirement: The sidebar and the diffs: The diffs and their texts come from the engine

The diffs SHALL come from `get_file_diffs` and their texts from `get_blob_texts`
([Commit Files](../commit-files/spec.md#file-diffs)): N and P step through the blocks that are not `Ignored`.

#### Scenario: Blocks the policy ignores
- **WHEN** a file has a block of changed lines that is `Ignored`
- **THEN** N and P step past it
  [@test] src/lib/diff/rows.test.ts (`starts N and P at each block of changed lines, a deleted one too, never at a block the policy ignores`)
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`N and P skip a block the whitespace policy ignores`)

### Requirement: The sidebar and the diffs: The Split/Unified/Inline toggle switches the layout

The Split/Unified/Inline toggle MUST switch the layout; it is the setting of the Files section, Split by default.

#### Scenario: Unified in the title bar
- **WHEN** the user chooses Unified in the toggle
- **THEN** the diffs switch to Unified, the shown layout pressed
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should show the tree beside the diffs of all files and follow J, K, N, P and the tree`)

### Requirement: The sidebar and the diffs: The toggle stands in the header of the block it switches

The toggle SHALL be in the header of the block whose diffs it switches: the title bar of a review window, the end of the
tabs of a pull request review, beside Changes. It is one option set, the stock `ToggleGroup` (`outline`, `sm`,
`spacing={0}`), the shown layout in its pressed state, no colour of its own (`DiffLayoutButtons`).

#### Scenario: A review window and a pull request review
- **WHEN** a review window or a pull request review is shown
- **THEN** the toggle is in the title bar of the review window, or at the end of the tabs of a pull request review, beside Changes
- **AND** it is one option set, the stock `ToggleGroup`, the shown layout pressed, no colour of its own

### Requirement: The sidebar and the diffs: Soft wraps follow the setting of Diffs

Soft wraps MUST follow the setting of Settings → Diffs ([Commit Files](../commit-files/spec.md#soft-wraps)).

#### Scenario: Soft wraps on
- **WHEN** soft wraps are on in Settings → Diffs
- **THEN** the lines of the review wrap

### Requirement: The sidebar and the diffs: A review may show the owner of each file

Where a review shows the owner of each file, it SHALL show it muted in the header of its diff and in the hover of its
tree row, and a tree row keeps its width for the name.

#### Scenario: The owners of a branch review
- **WHEN** a review shows the owners of its files
- **THEN** the owner of each file is muted in the header of its diff and in the hover of its tree row
- **AND** a tree row keeps its width for the name
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should review a stacked branch on its parent, then the whole stack, by owner`)

### Requirement: The sidebar and the diffs: An owner chip filters the tree, the diffs and the keys

An owner chip of a branch review ([Branch Review](../../branch/branch-review/spec.md#code-owners)) MUST filter the tree
and diffs to its files, and J, K, N and P move through that same shown set; none left in that direction, nothing moves.

#### Scenario: J, K, N and P under a chip
- **WHEN** the user chooses an owner chip
- **THEN** the tree and the diffs show its files, and J, K, N and P move through them
- **AND** with none left in that direction, nothing moves
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should filter tree, diffs and navigation to the files of an owner chip`)

### Requirement: The sidebar and the diffs: A step under a chip counts its owner's files

A step with only some of its files under an owner chip MUST show the owner count out of the step total, and the footer
names the selected owner without repeating its count.

#### Scenario: A step of the story under a chip
- **WHEN** an owner chip is chosen in the story of a branch
- **THEN** a step with only some of its files shows the owner count out of the step total
- **AND** the footer names the selected owner without repeating its count
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should show only the steps and files of an owner chip, and navigate within its story`)

### Requirement: The sidebar and the diffs: A link to a hidden file clears the chip

A link to a file hidden by the chip MUST clear the chip; an owner with no files in this range does not filter.

#### Scenario: A link to a hidden file
- **WHEN** the user follows a link to a file hidden by the chip
- **THEN** the chip is cleared
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should reveal a linked file from another owner by clearing its chip and clear a search that hides the target`)

#### Scenario: An owner with no files in the range
- **WHEN** the chosen owner has no files in this range
- **THEN** it does not filter
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should keep the chip of an owner with no files in the range and show no chips when one owner has them all`)

### Requirement: The search of the files: A review of 8 files or more has a search of the files

From 8 files on (`FILE_FILTER_MIN_FILES`, the count of the branch filter), or while it has a text, the sidebar MUST have
a search of the files above their header, the search of the branch table (`SearchField`). A review of fewer files has
none.

#### Scenario: A review of fewer files
- **WHEN** a review has fewer than 8 files and the search has no text
- **THEN** the sidebar has no search of the files
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should show the tree beside the diffs of all files and follow J, K, N, P and the tree`)

#### Scenario: A review of 8 files or more
- **WHEN** a review has 8 files or more, or the search has a text
- **THEN** the sidebar has the search of the files above their header
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should filter the files of a review by their path, with / from the diffs, Escape and Clear Filter`)

### Requirement: The search of the files: The search keeps the files with every word in their path

The tree and the diffs MUST show the files with every word of the text in their path or the old path of a rename,
ignoring case, as it is typed; the header says "Files 3 of 9".

#### Scenario: A text typed into the search
- **WHEN** the user types a text into the search
- **THEN** the tree and the diffs show the files with every word of it in their path, ignoring case
- **AND** the header says "Files 3 of 9"
  [@test] src/lib/file-filter.test.ts (`keeps the files with every word in their path, ignoring case`)
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should filter the files of a review by their path, with / from the diffs, Escape and Clear Filter`)

#### Scenario: A renamed file
- **WHEN** the text is in the old path of a rename
- **THEN** the renamed file is kept
  [@test] src/lib/file-filter.test.ts (`finds a renamed file by its old path`)

### Requirement: The search of the files: The search keeps the selection or selects the first file

The directories of what the search found MUST be open, the selected file stays while the search keeps it, else the
first one is selected, and the diffs show it at the top.

#### Scenario: The search leaves out the selected file
- **WHEN** the search keeps the selected file, or leaves it out
- **THEN** the directories of what it found are open
- **AND** the selected file stays, else the first one is selected and the diffs show it at the top
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should filter the files of a review by their path, with / from the diffs, Escape and Clear Filter`)

### Requirement: The search of the files: The search intersects an owner chip and holds for the next commit

The search MUST intersect an owner chip of a branch review, and both narrow the same tree and diffs; the text filter
holds for the next commit (⌥↑/⌥↓) until the window closes.

#### Scenario: A chip and a text
- **WHEN** an owner chip is chosen and the search has a text
- **THEN** both narrow the same tree and diffs

#### Scenario: The next commit
- **WHEN** the user shows the next commit (⌥↑/⌥↓) while the search has a text
- **THEN** the text filter holds until the window closes

### Requirement: The search of the files: Slash focuses the search, and its letters are text

`/` MUST focus the search from anywhere in the window and select its text, except in another input, a menu or a dialog.
The letters typed into it are text, never J, K, N, P or an action of the window (S, O).

#### Scenario: Slash from the diffs
- **WHEN** the user presses `/` in the diffs
- **THEN** the search has the focus, its text selected
- **AND** the letters typed into it are text, never J, K, N, P or an action of the window (S, O)
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should filter the files of a review by their path, with / from the diffs, Escape and Clear Filter`)

#### Scenario: Another input, a menu or a dialog
- **WHEN** the user presses `/` in another input, a menu or a dialog
- **THEN** the search does not take the focus

### Requirement: The search of the files: Escape clears the search, then gives the diffs the focus

Escape MUST clear the text, and in an empty search give the diffs the focus (the next Escape closes the window); ↓ MUST
move the focus to the tree.

#### Scenario: Escape and the arrow in the search
- **WHEN** the user presses Escape in the search with a text
- **THEN** the text is cleared
- **AND** Escape in an empty search gives the diffs the focus, and the next Escape closes the window
- **AND** ↓ moves the focus to the tree
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should filter the files of a review by their path, with / from the diffs, Escape and Clear Filter`)

### Requirement: The search of the files: A search that keeps nothing offers Clear Filter

A search that keeps nothing MUST say "No files match “parser”" with Clear Filter.

#### Scenario: No file matches
- **WHEN** the search keeps no file
- **THEN** it says "No files match “parser”" with Clear Filter
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should filter the files of a review by their path, with / from the diffs, Escape and Clear Filter`)

## User Experience

- The island of the review fills the window below its title bar; the panes scroll on their own. A 1px `bg-border`
  divider separates them.
- The sidebar has the commits of a range above a `border-b` (with the plan only its scopes while the tree lists the
  commits in its steps), then the search of the files (`SearchField` `w-full`: the
  `SearchIcon`, the placeholder "Filter files", `Kbd` `/` while empty, the X "Clear the filter text" once typed), then
  the header of the files ("Files 5", "Files 3 of 9" while narrowed by search or owner chip, a muted `text-xs font-medium`
  `SectionLabel`, at least `h-10`), then the tree with the rows of the Files section. The selected row is `bg-primary/15`; a file row has no
  chevron, its name keeps the indent.
- A file header is 48 px, the band `--diff-file-header` (`color-mix` of `--card` and 60% `--canvas`) in the
  `--diff-card-border` ring of its card (`--foreground` 8% into `--canvas`), a line of the ring under it; stuck with
  rows under it, the shadow `0 6px 6px -6px var(--diff-header-shadow)` (`--foreground` 12% in the light theme, black
  at 55% in the dark). In it: a ghost `icon-xs` chevron (disabled
  without lines to show), the file as every list of changed files names it (`FilePathLabel` with `emphasis`: the
  status letter in `font-mono text-xs` in the status colour, the name `text-sm font-semibold` in the status colour,
  struck through when deleted, the directory `text-muted-foreground` of the same size after it, its last folder in
  `text-foreground`), the copy button on
  hover, then on the right the notice and the owner muted and the coloured counts in `text-xs`.
- The divider of a step or a group is 32 px on the canvas at the inset of the cards, after the gap of 10 px, its words
  at its bottom, 6 px above the card they head, not selectable: the step in `text-sm`, "Step 3" `font-medium
  text-muted-foreground`, then "· <title>" `font-semibold` in the text colour, a button underlined on hover, then the
  group in `text-xs text-muted-foreground` ("Same edit in 4 files"); after the words of a step a 1-px rule in
  `--diff-card-border` fills the rest of the width at their middle, `gap-2`; a group alone has none.
- The line at the end of a card is one row high below its rows, in the band of the header (`--diff-file-header`),
  a line of the ring above it unless its last row is a fold, which draws its own, not selectable; its words at the
  right in `text-xs text-muted-foreground`: one button out of the tab order, `rounded-sm px-1.5`, `bg-accent` on hover,
  "Next:", the file as its header names it (`FilePathLabel` with `emphasis`: the name `font-semibold` in the text
  colour, the directory muted, its last folder not), then `Kbd` `J`.
- The map of the files is 10 px wide on the canvas at the right of the diffs, which hide their own scrollbar, its
  track inset by `--radius-4xl` above and below. A file is a tick of 1 px in `--diff-map-tick` (`--foreground` 26% into
  the canvas), 2 px from its sides; an owner chip filters the map to the shown files. A step a tick of 2 px in `--diff-map-step`
  (`--foreground` 55% into the canvas) across it; the section at the top `--primary` at 20%; the thumb `--foreground` at
  30% with an inset 1-px edge at 50%, 38% under the pointer, 45% dragged, `--radius-sm`, at least 16 px, a thicker one
  centred on the window of the view and kept inside the track. Its hover is a popover at its left, its middle at the height of the pointer: the file as a list names it (`FilePathLabel`), its step muted under it, in `text-xs`.
- The review window ([Title Bar](../../repository/title-bar/spec.md)) names the list and the commit in its title bar
  ("session › 11346cdf"), the toggle at its other end; the native title stays "Review 11346cdf <subject>".
- The footer shows "File 3 / 12" on the left, "Commit 2 / 3 · File 3 / 12" in a list of several commits; with the
  plan "Overview" while its overview is at the top (the tree then selects no row: no file is shown, none is the file a
  review read again starts at) and the step of
  the story at the top before the file ("All Changes · Step 2 · File 3 / 12"), one hook for every window of a review
  (`useReviewPosition`) (the windows
  of a branch and of a pull request add the chosen owner chip's name, "· Design", [Branch
  Review](../../branch/branch-review/spec.md#code-owners); the window of a commit has no chips), and on the
  right the hints `J K Files`, `N P Changes`, `F Sidebar`, `I Details` while a row of the tree has the focus
  ([Review Plan](../../branch/review-plan/spec.md)), `⌥↑ ⌥↓ Commits` (`Alt+↑ Alt+↓` off macOS, not for a list of one) and
  `Esc Close`.
  [@test] src/lib/review-position.test.ts (`says Overview while the overview is at the top, and the step of the story before the file`, `shows the file of the position, none while the overview is at the top`)
- The preview's footer keeps `D Review Changes` (the window opens), `F Files`, `E Edit` and `Space Close`.

## Testing / Local Run

- `pnpm test:unit`: `src/lib/diff/rows.test.ts`, `layout.test.ts`, `file-map.test.ts`, `copy-text.test.ts`, `find.test.ts`, and the tests of [Commit Files](../commit-files/spec.md) for
  the tree and the patch.
- `pnpm e2e`: `tests/e2e/commit-files/commit-review.spec.ts` on the `commit_tree` (`RowMapper.kt` expanded is a long
  card), `commit_many_files` (nine files, the search) and `issue_links` templates. A
  virtualized view renders what is on screen: assert on the text of files the review scrolled to.

## Open Questions / Risks

- The patch of a commit is parsed whole when the review opens; a commit of thousands of files may take a moment.
- A diff without the file contents cannot know whether lines follow its last hunk, so its last separator offers
  "More unchanged context may be available".

## References

- [Commit Files](../commit-files/spec.md)
- [Diff View](../../../../docs/design/diff-view.md) ("Review")
- [Commit Row](../commit-row/spec.md) (the preview)
- Space and GitHub code review: a file tree beside one scrolling list of file diffs with sticky headers.
