# Proposal

## Why

The sticky horizontal scrollbar of a file whose lines are wider than their column lay over the lower 10 px of the
last row of its card: a fold at the end of a card, and its places ("1 change in Other changes"), took a click only in
their upper half. An e2e that clicked such a fold timed out at 30 s.

## What Changes

- The scrollbars of a file take a strip of their own below its rows, 10 px, in the layout (`LayoutFile.scrollbar`):
  at the end of the file they lie under the last row, so a control of that row takes a click anywhere on it. While
  the rows go on below the view, they stick at its bottom as before.
- The strip is kept only where a side is wider than its column, once its text was read; a file keeps it while its
  text leaves the window of the view.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `commit/commit-files`: the scrollbar of a long line never lies over a row of its file.

## Impact

- `src/lib/diff/layout.ts` (`SCROLLBAR_HEIGHT`, `scrollbarHeight`), `src/components/diff/DiffView.tsx`
  (`HorizontalBars`, `scrollerWidths`), `src/components/diff/diff-view.css` (`.dv-hscrolls`); the e2e of a click on
  the fold at the end of a card, in WebKit too.
