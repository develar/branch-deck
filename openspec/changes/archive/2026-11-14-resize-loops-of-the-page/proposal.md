# Proposal

## Why

The E2E tests now fail on an error the page logs. Under load, the page logged a ResizeObserver loop as an uncaught
error. Some of those loops were ours: the toasts, the measured heights of a review, the columns of the branch view
(WebKit renders a commit of React that waited in a microtask after the callback of an observer, in that same rendering
update). Others came from the observers of the libraries, which the specification times. A single loop is harmless:
its notifications come in the next frame. A real cycle repeats. Some errors the page catches still reached only its
console, never the user's BranchDeck.log.

## What Changes

- Every ResizeObserver of the page is made by `observeSize` (src/lib/observe-size.ts, `pnpm lint:arch`
  `resize-observer`). Until the next task after a report, an element that a change of its subtree touches leaves its
  observer and is observed anew. An element released so in every frame of a second is a cycle, which the log has as
  an error.
- The page logs a single ResizeObserver loop as a warning, and more than two within a second as an error.
- An error the page catches reaches the log of the app (`logError`), and a user's issue pattern that is no regex reaches
  it as a warning (`logWarning`).

## Capabilities

### Modified Capabilities

- `repository/title-bar`: what the log of the app holds of the page: the loops, the cycles, the errors it catches.

## Impact

- `src/lib/observe-size.ts`, `src/lib/page-errors.ts`, `src/lib/errors.ts`, the observers of the page,
  `scripts/lint-arch.mjs`, `tests/e2e/fixtures/base-test.ts`, `tests/e2e/resize-loops/`.
