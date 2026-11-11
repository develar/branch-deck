# Proposal

## Why

The review of the views of a rename found places where a view said less, or other, than the marks: a note that
said "and 1 more", which the app never says; a note that said it was closed while its one line showed, because
another note had opened the run the two shared; a note a screen above its declaration in a block of many lines; a
quiet line whose note was out of view, which H could not open; text of a hidden old line that the find found
nowhere, and an old name found at the first row of a run wrapped otherwise, not at the row of its new name; a name
that a wrap moved to the next line, drawn as a deletion and an insertion in the rows of Inline; "renamed" said to a
screen reader once a part of a name, and the renames a note counts told on a hover alone. The screenshots added
one: on a line of a run of a renamed name the text between two names was drawn as a change, where the word diff of
the engine had joined the names.

## What Changes

- The note of renames is worded as a cut list of the app: both renames of a note of two, the first of three or
  more with the count of the others, which a hover lists and a screen reader hears after the count.
- A quiet run opens with one note alone, the note of the first rename with a site in it; a note with no run of its
  own is plain words.
- The note stands right above the line of its site: the lines that hold the site are cut there, each side at its
  line of the site.
- H takes a note for a line it hides or shows that is in view, wherever the note stands, and hides the lines again
  from there.
- The find reads every old line of a quiet run of a renamed name and finds each hit at the text of the new lines
  that stands for it: the new name for the old one, on the row of its new line; the same code for text only the
  old lines hold in one piece; nothing twice.
- Lines the engine pairs that hold a name on two lines it does not pair show apart, both lines with the name in its
  mark.
- A line of a run of a renamed name, a label too, shows its names alone.
- A renamed name says "renamed" once to a screen reader, at the part it starts with.
- What stays open is said: a place that makes more of its name than its binding is drawn in the mark, since the
  marks do not name those places.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `commit/quiet-changes`: the Views requirements of a rename (its note, its words, its mark, the row of a pair,
  what the note opens, the find), the key of the notes, the old lines one column shows by the new ones alone, and
  what a screen reader hears.

## Impact

- `src/lib/diff/block-parts.ts` (`BlockMarks.apart`, `RenameNotes.ofRun`, `cutAt`, `LinesPart.note`,
  `partsNoteOfRow`), `src/lib/diff/rows.ts` (`notesOfRows`), `src/lib/diff/layout.ts` (`rowsInView`),
  `src/lib/diff/rename-sites.ts` (`renamedRunOf`, `RunUnits`, `lineRanges`, `pairRanges`), `src/lib/diff/find.ts`
  (`DiffFindHit.stand`, `rowOfHit`, `LineHits.stands`), `src/lib/diff/row-cells.ts`, `src/lib/diff/inline-rows.ts`.
- `src/presenters/diff-marks.ts` (`RemarkWords.renamed`, `more.said`), `src/components/diff/diff-painter.ts`,
  `src/components/diff/DiffView.tsx`, `src/components/diff/diff-view.css`.
- `tests/e2e/commit-files/renamed-names.spec.ts`, `tests/e2e/commit-files/quiet-changes.spec.ts`,
  `tests/e2e/helpers/marks-helpers.ts`, `tests/e2e/helpers/diff-helpers.ts`.
