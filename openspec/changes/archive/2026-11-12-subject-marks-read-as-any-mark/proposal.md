# Proposal

## Why

The views word the marks of a subject now (the change `subject-marks-in-the-views`): the lines that moved between
two files, the name another file declares, what the index proved. Four requirements of the views of the marks of a
file say what those marks change in them, and one scenario said the opposite of what a view shows since: a file that
only uses a renamed name showed its calls as changes, with no note.

## What Changes

- **A quiet line and a note are named to a screen reader**: a line of a move between two files says that its other
  end is in another file; the places are buttons named by the file and the line they show.
- **N and P stop where the reading of a block starts**: the lines that left for another file are read, unlike the
  lines a move took within the file; the uses of a name the index proved are passed.
- **G shows the other end of a move in view**: it takes the note of a move between two files, and passes a note
  whose other end the view cannot show.
- **A renamed name is drawn in a mark of its own, never as a change**: a name another file of the subject declares
  is drawn the same at each of its uses.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `commit/quiet-changes`: four requirements of the group Views.

## Impact

- The same code as `subject-marks-in-the-views`; `tests/e2e/commit-files/moved-lines.spec.ts` and
  `renamed-names.spec.ts` (the tests of the template that read Report.kt).
