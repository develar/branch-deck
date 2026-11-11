# Proposal

## Why

Inline became the default layout, so what it showed worse than Unified and Split is what every user meets. The row of
a similar pair holds both its lines, and three things read only one of them:

- the find searched its new line alone, so a text of the old line that Unified and Split found was not found;
- the horizontal scroll reached as far as the widest line of the two texts, but the row is the new line with the
  deleted parts of the old one, longer than both, so its end could not be scrolled to;
- a name shown on the old side (a removed declaration of Possible Breakage) had no cell of the old line to be
  selected in, and a name found by its word was looked for in the text of the row, the deleted parts with it, so the
  selection stood as many columns off as the deleted part before the name was long.

## What Changes

- The find searches an Inline row in both its lines: the new line, and of the old line the hits that hold a deleted
  part or a place where the new line inserts text. Such a hit is marked in the struck-through part and on in the
  text the row shows from the new line; a hit in the text the lines share counts once.
- The one column of Inline scrolls as far as its widest row (`widestInlineRow`).
- A name of the old line is selected where its Inline row shows it: in the shared text, or in a deleted part, which
  is selectable while the name is selected. A name by its word is found in the line of the model.
- The words of the branch review name the layouts in the order of the toggle.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `commit/commit-review`: the diff search finds what each layout shows.
- `commit/commit-files`: the horizontal scroll of Inline reaches the end of its widest row.
- `commit/symbol-navigation`: a name of the old line is selected in its Inline row.
- `branch/branch-review`: the layouts by the order of the toggle.

## Impact

- `src/lib/diff/find.ts`, `src/lib/diff/line-parts.ts`, `src/lib/diff/inline-rows.ts` (new),
  `src/lib/diff/line-table.ts`, `src/components/diff/DiffView.tsx`, `src/components/diff/diff-painter.ts`,
  `src/components/diff/diff-view.css`; their unit tests and the e2e of the review.
