# Proposal

## Why

The review of 939ed49e found the boxes the page reads of an element (`laidOutBoxes`, the computed style and the whole
pixels of its client box) other than the engine reports in WebKit at a fractional device scale: a bar inferred a pixel
off, borders on a grid the computed style does not say. Measured further, WebKit at a zoom other than 100% (the zoom of
the app, which WebKit lays out as a zoom of the root) misses by up to 1.5 px for every element with a border or a
padding, a bar or none. An element laid out as reported read as resized: a loop of a library was taken for the page's,
an error, and a drag counted toward a cycle.

## What Changes

- A loop is told by the engine's own boxes: the computed style proposes the candidates, an observer made before the
  others observes them at the loop, its report in the next update the verdict; the page weighs the loop at its time.
- The release that would make a cycle is measured against the layout of the element as reported, one arithmetic for
  both (`sameBoxOf`), exact at any zoom and scale.
- The chain of a callback is pinned at its boundary: a change 255 promises after the callback is in it, 256 past it.

## Capabilities

### Modified Capabilities

- `repository/title-bar`: the elements of a loop by the engine's boxes, the measure of a cycle, the boundary of the chain.

## Impact

- `src/lib/laid-out.ts`, `src/lib/observe-size.ts`, `src/lib/resize-loops.ts`, `src/lib/page-errors.ts`,
  `docs/contributing/frontend.md`, `docs/contributing/e2e-tests.md`, `tests/e2e/resize-loops/resize-loops.spec.ts`.
