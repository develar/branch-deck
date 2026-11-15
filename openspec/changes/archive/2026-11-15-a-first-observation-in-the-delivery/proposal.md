# Proposal

## Why

The review of 0e309ea3 found a full `pnpm e2e:webkit` failing `review-groups.spec.ts:48` on "A ResizeObserver loop of an
observer of the page: left div pending": the step row of a group a flushSync in a callback mounted, which the ref of its
block observes at once (`stickBelowStepRow`), a first observation the update skipped. The tests never saw it: the fake
mutation observer of the unit tests gave records of any node. And a drag of the view while the blocks report counted
toward a cycle: the view, let go before its own report, measured against its report of the frame before. Since
a1937e6d, any change of the update lets the shallow elements go, which the review asked to measure on a large table.

## What Changes

- An element first observed in the rendering update, no deeper than the latest report, is observed at the end of the
  delivery, with no change of its own to tell it.
- A release counts toward a cycle only after the element's report in that delivery; the boxes a loop is told by are the
  content box and the border box, as the engine compares them.
- The fake mutation observer of the unit tests gives records only under the nodes it observes, and none while it observes
  nothing. On the 141 rows of `branch_table_large`, a sync that changes nothing lets nothing go and the menu of a row
  observes one element, as on master.

## Capabilities

### Modified Capabilities

- `repository/title-bar`: the first observation in the delivery, the releases that count toward a cycle.

## Impact

- `src/lib/observe-size.ts`, `src/lib/observe-size.test.ts`, `docs/contributing/frontend.md`,
  `tests/perf/branch-table.perf.ts` (the closing menu waits for its exit).
