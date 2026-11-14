# Proposal

## Why

The review of `observeSize` found three holes. Its cycle was 30 releases of an element within a second, which a window
below 30 frames a second (a loaded machine, WebKit, a window in the background) never reaches, while a drag that lets an
element go in every frame reaches it in half a second, and no test failed with the count raised to a million. The page logged more than two loops within a second as an error, so a loop every 600 ms stayed a warning forever.
And a review painted its overview over its first file for a frame, 2 of 22 openings on a CPU four times as slow: the
delivery ended at the next task only, so a render of React in a task before it let the block go, and a frame came
before the block was observed again; and the delivery let go a block the update would have delivered at its depth in
that same frame. A loop of an observer of ours was found only by the WebKit check that attributes it.

## What Changes

- The delivery of an update ends at the next task or the next frame, whichever comes first, and lets go only an element
  no deeper than the elements of the latest report; one let go comes back as soon as a report of shallower elements
  shows it deeper.
- A cycle is an element let go in the delivery of 10 frames in a row, resized there by a change of its own.
- The page logs 3 loops of a library within 5 s as an error, and a loop that left an element of ours pending as an error
  at once. The thresholds are in one module, `src/lib/resize-loops.ts`.

## Capabilities

### Modified Capabilities

- `repository/title-bar`: the loops and the cycles the log of the app holds, and the delivery of the observers of the page.

## Impact

- `src/lib/observe-size.ts`, `src/lib/resize-loops.ts`, `src/lib/page-errors.ts`, `scripts/lint-arch.mjs` (the
  exemption of the engine's event is its one comparison), `tests/e2e/resize-loops/`, the docs of the observers.
