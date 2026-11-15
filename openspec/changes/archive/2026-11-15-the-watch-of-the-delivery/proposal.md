# Proposal

## Why

The delta review of a1937e6d found the watch of the page ending two microtasks after a callback: floating-ui sets the
place of a popup with a flushSync three promises after the callback of the observer of Base UI, still in the update, and
a change there let nothing go. An element in place first observed in a callback, with no change, was observed at once
(fixed by 1d261595 for the callback and the two microtasks; here for the whole delivery). The title-bar spec linked a
test of the loop that runs in Chromium alone, where the loop showed in WebKit.

## What Changes

- The page is watched from a report to the end of its delivery. A change in a callback and the microtasks right after it
  lets go every element of the depth; a change later in the delivery lets go the elements it is in, so a render of
  React in a task (the rows of a sync) lets go none of the cells of every row.
- An element first observed anywhere in the delivery, no deeper than its report, is observed only as it ends.
- The cost of the watch on 141 rows is in docs/design/rendering-performance.md.

## Capabilities

### Modified Capabilities

- `repository/title-bar`: the watch of the delivery and what a change in it lets go.

## Impact

- `src/lib/observe-size.ts`, `src/lib/observe-size.test.ts`, `docs/contributing/frontend.md`,
  `docs/design/rendering-performance.md`.
