# Proposal

## Why

The review of 1d261595 found the boxes the page tells a resize by other than the engine's for a scroller whose scroll
bar takes room (WebKit on macOS with a mouse, "Always show scroll bars"): the computed style says nothing of the bar,
so an unchanged scroller (the card of a commit, the parent the painter scrolls) read as resized, a loop of a library
named it ours, an error, and the cycle check counted it. Both boxes were compared, where the engine compares the one
observed (the columns observe the border box). An element observed again in a delivery, which the engine still
observed, stayed in the engine while the page took it for waiting, so a change did not let it go. The spec and the
rules said a cycle was a change of an element's own callback, where any callback of the page counts.

## What Changes

- The boxes are read as the engine reports them (src/lib/laid-out.ts): the scroll bar in the border box, not in the
  content box; an observation compares the box it observes alone.
- An element observed again in a delivery, no deeper than its report, leaves the engine's observer until it ends.
- A cycle is told of a change of any callback of the page, as the code counts it.

## Capabilities

### Modified Capabilities

- `repository/title-bar`: the boxes of a loop, an element observed again in a delivery, the words of a cycle.

## Impact

- `src/lib/laid-out.ts`, `src/lib/observe-size.ts`, `src/lib/resize-loops.ts`, `docs/contributing/frontend.md`,
  `tests/e2e/resize-loops/resize-loops.spec.ts`.
