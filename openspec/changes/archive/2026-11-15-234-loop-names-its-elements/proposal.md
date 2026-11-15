# Proposal

## Why

`review-groups.spec.ts` "should count the files of a group the reader saw" failed under load (#234) on "A
ResizeObserver loop of an observer of the page: left div pending": the line named the element only `div`, so the
report did not say which observer it was. Named by its role and nearest named ancestor it was
`div[treeitem] in review-sidebar`, the row of a large step that `stickBelowStepRow` of the file tree observes, which
an observe in the delivery made pending (fixed by "an element first observed in the delivery of an observer waits for
its end"). The first test of `observe-size.test.ts` imported the log of the errors with the toasts and the bindings it
imports, 2 s, past the timeout of a test on a loaded machine, and its import ran on into the next test, which failed
too: 8 of 8 parallel runs at background priority.

## What Changes

- The line of a loop and of a cycle names an element without a test id or slot by its tag and role, in the nearest
  ancestor that has one.
- The unit tests of `observeSize` mock the log of the errors by default; a test of a cycle mocks it with its own spy.

## Capabilities

### Modified Capabilities

- `repository/title-bar`: the line of a loop names an element with no name of its own by its nearest named ancestor.

## Impact

- `src/lib/observe-size.ts`, `src/lib/observe-size.test.ts`, `docs/contributing/frontend.md`.
