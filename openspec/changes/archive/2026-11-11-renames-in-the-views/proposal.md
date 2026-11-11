# Proposal

## Why

A diff names the names a change renamed within a file and where each stands on both sides
(`TextDiff.marks.renames`), and the lines that changed in such a name alone are runs of the kind `renamed`, but no
view words a rename: each run shows under the placeholder note "Renamed only", once a run, and the name at each site
is drawn as any changed range, in the colours of a change. A reader of a local
renamed at twelve places reads twelve notes that say nothing and twelve changed words, and cannot tell the rename
from a change of the code.

## What Changes

- A rename has one note in the rows of a file, "Renamed total → sum · 12 sites", before the lines of its
  declaration, or of its first site where the rows do not show the declaration (a section of a review). The other
  sites have no note, and a run of a renamed name none of its own. Renames whose notes stand in one block share one
  note: the first is worded, the others counted ("and 2 more") and told on a hover.
- The name at a site is drawn in a mark of its own on both sides, in every layout, on a quiet line, on a line that
  is read and on a line with another change: never as a changed range. The row of a pair of lines of Inline strikes
  the whole old name through before the whole new one, in that mark: nothing of a line that is read is left out.
  What the name was, or became, is its hover.
- A quiet run of a renamed name shows once in Inline, by its new line, and the one note of the rename opens the old
  lines of every such run of the rename; Unified and Split keep both lines, played down. Nothing of this is decided
  on the page: a run is quiet when Rust says so, and until the kind is switched on every run of a renamed name is a
  label, read as any line.
- The find for the old name lands on the row of a quiet line shown once and marks the new name that stands for it;
  where the old name shows, struck through or in its own line, the hit is in it.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `commit/quiet-changes`: the views of a rename, in the group Views.

## Impact

- `src/lib/diff/rename-sites.ts` (new: the sites of the renames by line, the ranges of a line with its sites),
  `block-parts.ts` (`renameNotesOf`, `RenameNote`, the note of the renames of a block), `rows.ts` (`FileRows.notes`
  by key), `row-cells.ts`, `line-parts.ts` (`RENAMED_RANGE`), `find.ts`.
- `src/presenters/diff-marks.ts`, `src/components/diff/diff-painter.ts`, `diff-view.css`, `DiffView.tsx`.
- `tests/e2e/commit-files/renamed-names.spec.ts` (new), `tests/e2e/helpers/diff-helpers.ts`,
  `tests/perf/diff-fixtures.ts`.
