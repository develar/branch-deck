# Proposal

## Why

The views draw the ends of a move now (the change `moves-in-the-views`), and what reads the rows of a diff has to
read them as they show: the stops of N and P, the find, a copy, the scroll position, the heights of the rows, the
heading of a change in a review and the words for a screen reader. Each had its rule for the note of a quiet run;
the note where the lines of a move left is such a note, and the lines where they stand now are new rows.

## What Changes

- N and P stop at the first line of a move where it stands now, under its note, and read a block that lost lines
  from a line that stayed: the lines a move took end what is read before them, as a quiet run does.
- The find searches the lines a move took at the note that stands for them, and the row of an edited line of a move
  by its new line alone, so a hit of its old line is one hit, where the line left; a copy takes the lines of a move
  once, where they stand now, and the lines that left too once their note is opened.
- An old line a move took is found at its note (the scroll position, a jump); the notes of a move are a row each and
  the row of an edited line of a move counts among the rows of two lines, as the pair does of a block in which a
  review with its plan draws no move.
- The words of an end of a move that starts its block join the heading of the change in a review.
- A line of a move says which end it is at to a screen reader, and the buttons of its note are named by what they do.
- The tints of a block: a line of a move is in the colour of a move, pale where it was edited on the way.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `commit/quiet-changes`: the stops, the find, the copy, the row of an old line, the layout arithmetic, the heading
  and the words for a screen reader, each with the lines of a move.
- `commit/commit-files`: the tints of a block.

## Impact

- `src/lib/diff/block-parts.ts` (`partsStopRow`, `partsRowOfLine`, `inlinePairsOf`), `rows.ts`, `row-cells.ts`,
  `inline-rows.ts`; `find.ts`, `copy-text.ts` and `layout.ts` read the rows as before.
- `src/components/diff/diff-painter.ts`, `diff-view.css`.
- `tests/e2e/commit-files/moved-lines.spec.ts`, `quiet-changes.spec.ts`.
