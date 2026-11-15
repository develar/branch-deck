# Proposal

## Why

The review of d8e3aa61 found a loop the page did not prevent: a change some promises after the callback of an observer,
still in the rendering update of its delivery, let go only the elements it was in, so a sibling that grew there shrank
a view of the app (the room of the repository path in a flex row) that kept its observation, which the next iteration
skipped. Floating-ui places a popup with a flushSync so, 42 to 87 microtasks after the callback of its observer in the
E2E suites of both engines; the page took only two microtasks after a callback for its update.

## What Changes

- A change in a callback or in the chain of 256 microtasks after it (`SPAN_MICROTASKS`) lets go every element no deeper
  than the latest report; a change past the chain, in a task or in a microtask of the update later still, the elements
  it is in. The chain runs in the checkpoint of the callback, before any task; a hop is an await, some nanoseconds.
- One chain at a time: a callback that begins in a running chain makes it as long again.

## Capabilities

### Modified Capabilities

- `repository/title-bar`: the chain of a callback in the scenario "No observer of the page makes a loop".

## Impact

- `src/lib/observe-size.ts`, `docs/contributing/frontend.md`, `docs/design/rendering-performance.md`,
  `tests/e2e/resize-loops/resize-loops.spec.ts`.
