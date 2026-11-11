# Proposal

## Why

What a view shows of a quiet run of a renamed name (the line once in Inline, the one note of the rename as the button
that opens every such run, N and P passing it, the find for the old name) was tested on rows built in the unit tests
alone: the engine gives such a run as a label until a measurement switches its kind on, and a view of a browser never
met one. The page reads `LineRun.quiet` and decides nothing, so the day the kind is switched on is the day a browser
shows this for the first time.

A test of the note of a rename found a defect of every note that opens: a press that came before the event of the
scroll that brought the note into view laid the rows out by the row the view had at its top before that scroll, and
the note left the view.

## What Changes

- The E2E tests of the views read the quiet form in a browser: the test server runs `get_file_diffs` on the real
  engine and the test rewrites its answer on the way (`switchRenamedOn`: the runs of a renamed local quiet, their
  blocks without a start, as `PairMarks::as_proven` answers), which is no page answer of a read. The preview in
  Inline, Unified and Split; in the review the note by a click, ⌥-click and H, N and P, the find, and the count of
  what is left to read. The helper goes with the switch of the kind; its tests stay.
- A press or a key in the view reads the row at its top where the view stands, so a note that opens, or a fold,
  stays where it was pressed also right after a scroll.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `commit/quiet-changes`: the scenarios of the quiet form of a rename in a browser, and where a pressed note stays.

## Impact

- `src/components/diff/diff-painter.ts` (`readAnchor`).
- `tests/e2e/helpers/marks-helpers.ts` (`switchRenamedOn`), `tests/e2e/commit-files/renamed-names.spec.ts`,
  `tests/e2e/commit-files/quiet-changes.spec.ts`.
