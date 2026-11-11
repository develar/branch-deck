# Proposal

## Why

The views word a rename now (the change `renames-in-the-views`), and the requirements of the views that were
written for the note of a run have to say what holds for a run of a renamed name, which has no note of its own:
where its old line is found, what N and P do at it, what a screen reader hears, and what the heading of a change
says.

## What Changes

- A block is still cut at every run; a run of a renamed name that a rename of the marks names has no note of its
  own, and the words "Renamed only" are left to a run no rename names.
- A run that is a label shows as the lines of its block; of a renamed name with the name in its mark, under the one
  note of its rename or under none.
- The note of the renames named at a block joins the heading of the change where its lines start the block.
- An old line of a quiet run of a renamed name is found at the row of its new line, and a reveal of a name in it
  opens the note of the rename.
- N and P stop at each line of a renamed name while its run is a label, at the line and never at the note, and pass
  it once it is quiet.
- A renamed name says that it is one to a screen reader, and the note of a rename that opens is a button named by
  its words and by the old lines it shows.
- The one column of Inline scrolls as far as the row that strikes the whole old name of a rename through.
- The tints of a block: a renamed name is a range of its own, in no colour of a change.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `commit/quiet-changes`: the parts of a block, a label, the heading, the row of an old line, the stops, the words
  for a screen reader and the layout arithmetic, each with a rename.
- `commit/commit-files`: the tints of a block.

## Impact

- `src/lib/diff/block-parts.ts` (`partsRowOfLine`, `partsNotes`), `rows.ts` (`noteOfLine`), `row-cells.ts`,
  `inline-rows.ts` (`widestInlineRow`).
- `src/components/diff/diff-painter.ts`, `diff-view.css`.
- `tests/e2e/commit-files/renamed-names.spec.ts`, `quiet-changes.spec.ts`.
