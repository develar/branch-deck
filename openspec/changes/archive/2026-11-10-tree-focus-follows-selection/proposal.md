# Proposal

## Why

In the review window the tree kept the focus on the row clicked before while J, K, N and P moved the selection: the
outline stayed on one row, the selection tint went to another, two rows marked. The first J of a review just opened
on its overview did nothing: the diffs had reported no stop yet, so J went from before the first, to the overview it
was at. And at the end of a short document, where J brings a section only as far up as the end lets it, nothing said
which section the bar, the footer and the tree name there.

## What Changes

- The focus of the tree follows a selection the review moves (J, K, N, P, the diffs scrolled, a file chosen from the
  card of a mark): while it is on a row of the tree it goes to the selected row, and the tab stop with it; one row is
  marked. Enter on a file of a card opened by `I` puts the focus on that file's row, as a click does.
- Until a scroll or a choice reports a stop, the diffs say what is at their top: the first J of a review that opens on
  its overview shows the opening of the first step.
- At the end of a document the bar, the footer and the tree name one section: the one J, K, a row or a place showed
  while the reader does not scroll, the last one once the reader scrolls to the end, as the selection of
  [Commit Review](../../../specs/commit/commit-review/spec.md) does.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `commit/commit-review`: the focus of the tree follows the selection.
- `branch/review-plan`: Enter on a file of a card opened by `I` focuses its row.
- `branch/review-tour`: the first J of a review just opened; the bar at the end of a document.

## Impact

- `src/components/files/FileTree.tsx` (the focus follows `selectedKey`), `src/components/diff/DiffView.tsx`
  (`visibleNow`); the e2e of the parts of a file and of the card of a mark.
