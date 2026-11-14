# Proposal

## Why

The review of 214e37a5 found the depth of a delivery blind to the observers of the libraries: a flushSync of their
callback in a frame without a report of ours, or a report of theirs deeper than ours, resized an element of ours that
the delivery kept, and the loop that followed was logged as one of ours. A cycle that resized its element in every other
frame broke the row of frames and was never logged. And a loop in every frame wrote an error line in every frame.
Under load, `issue-popover.spec.ts` "should close as the view of its link hides" failed on the console's own line of a
loop the page had weighed as a warning (WebKit reports an error event nobody cancelled), and its trace in Chromium named
the cells of the branch table resized to nothing as the view hid around them: a change of an ancestor, outside the
subtree of each cell, which the delivery did not see.

## What Changes

- The global `ResizeObserver`, as the libraries make theirs, is the engine's with one step before each callback: their
  report opens the delivery at its depth. Passive: the engine's class, prototype, methods and `instanceof`, the same
  entries in the same frames; no element of theirs is let go or observed anew.
- A cycle is an element let go in the delivery of 10 of 20 frames, resized there by a change of its own callback.
- Each kind of line of the loops comes once in 10 s at most, with the count of those it left out; the page cancels the
  engine's event of a loop, so no console reports it as an uncaught error.
- The delivery sees a change of an ancestor of an observed element (its class, style, hidden, children), not only of
  its subtree.
- `E2E_CPU_THROTTLE` works without the trace, so a test runs slow with the page's own weighing of a loop.
- A cycle that resizes its element after the delivery is documented as not caught, and why.

## Capabilities

### Modified Capabilities

- `repository/title-bar`: the delivery of the observers of the page takes the reports of the libraries; the cycles and
  the lines of the loops.

## Impact

- `src/lib/observe-size.ts`, `src/lib/resize-loops.ts`, `src/lib/page-errors.ts`, `src/main.tsx`,
  `tests/e2e/resize-loops/`, `tests/e2e/fixtures/base-test.ts`, `docs/contributing/frontend.md`,
  `docs/contributing/e2e-tests.md`.
