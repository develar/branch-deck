# Proposal

## Why

A diff says which lines of a block changed in their formatting or their comments alone and where the reading of each
block starts ([Quiet Changes](../../specs/commit/quiet-changes/spec.md)), and no view showed it: a wrapped call was
drawn as any change, and a reader could not tell what Rust had proven needs no reading.

## What Changes

- A view shows a block part by part (`src/lib/diff/block-parts.ts`): the lines between two runs as the block shows
  without marks, each run after a row of words, its note ("Formatting only · 3 lines").
- Inline shows a quiet run once, by its new lines, in the quiet colour; its note is a button that shows the old lines
  and hides them again, ⌥-click every note of the file. Unified and Split keep every line, played down.
- A run of comments is a label: a note, its lines as any change.
- In a section of a review the words of a run that starts its block join the heading of the change.
- An old line a note stands for resolves to the note: the anchor of the scroll, a reveal (which opens the note), the
  find (which lands on the note and names its hits); a copy takes the lines the view shows.
- A quiet line and a note are named to a screen reader.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `commit/quiet-changes`: the group Views, what a view draws of the runs.

## Impact

- `src/lib/diff/block-parts.ts` (new), `rows.ts`, `row-cells.ts`, `find.ts`, `layout.ts`, `inline-rows.ts`.
- `src/components/diff/diff-painter.ts`, `DiffView.tsx`, `diff-view.css`; `src/presenters/diff-marks.ts` (new).
- `tests/e2e/commit-files/quiet-changes.spec.ts` (new), `tests/e2e/helpers/diff-helpers.ts`.
- `docs/design/diff-view.md` ("Marks", "The row model"), `docs/design/ui-theme.md`.
