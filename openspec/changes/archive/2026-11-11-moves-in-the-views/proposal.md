# Proposal

## Why

A diff names the lines that moved within a file (`TextDiff.marks.moves`), and the stops of N and P already pass the
block they left, but no view draws a move: the lines show as deleted lines where they left and as added ones where
they stand now, with no word that they are the same lines. A reader reads a moved function twice, or compares the two
places by eye to find the one line that was edited on the way.

## What Changes

- Every layout shows a note at each end of a move: where the lines left how many moved and to which line, where they
  stand now from which line, whether they were indented again and how many were edited on the way. Both ends have
  the colour of a move, `--diff-moved`, a line edited on the way its pale form with the ranges of the move, and a
  rail down their start.
- Inline shows the lines once, where they stand now, an edited one as the row of its two lines; where they left one
  note stands for them and opens as the note of a quiet run. Unified and Split keep every line at both ends.
- The lines of no move in a block of a move show as the block does; in lines the engine pairs the lines beside a
  move keep their pairs.
- The place a note names is a button that shows the other end and marks the note there; G in a review window does it
  for one note in view, the one that shows the key (the note of the end the view stands at, else the first), and the
  menu of the code has the item on a line of a move, with the key where G takes that end. A review window keeps the
  jump for Back and Forward; the preview of a commit keeps no history.
- A block of the imports alone that a review with its plan plays down draws no end of a move: imports that were
  sorted stay pale lines to pass. A move with an end in such a block is drawn at neither end: at the other one its
  lines are lines of their block, none behind a note.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `commit/quiet-changes`: the views of a move, in the group Views.

## Impact

- `src/lib/diff/block-parts.ts` (`marksByBlock`, `MoveEnd`, the parts of the ends of a move), `rows.ts`
  (`LineRow.move`, `FileRows.moveNotes`, `rowOfPlace`), `row-cells.ts` (the cell `moved`, the ranges of a move),
  `inline-rows.ts`, `code-menu.ts`, `layout.ts` (`moveNoteOfKey`, the note the key takes).
- `src/components/diff/diff-painter.ts` (the note of an end, `land`, `moveAt`), `DiffView.tsx` (`jumpMove`,
  `DeclarationPlace.wholeLine`, `moveKey`), `CommitReviewCode.tsx`, `src/components/commit/CommitReview.tsx`,
  `src/lib/review-keys.ts` (G), `diff-view.css`, `src/index.css` (`--diff-moved`, `--diff-moved-pale`).
- `src/presenters/diff-marks.ts` (the words of a move).
- `tests/e2e/commit-files/moved-lines.spec.ts`, `tests/e2e/helpers/marks-helpers.ts`, `diff-helpers.ts`.
