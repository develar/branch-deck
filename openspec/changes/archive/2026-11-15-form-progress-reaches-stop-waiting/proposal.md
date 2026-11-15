# Proposal

## Why

The rule that Stop Waiting of a wait for the lock takes the focus and gives it back was tested on the surfaces that keep
their pressed button, not on the progress row of an inline form, which takes the place of the form and of its pressed
button as the rewrite runs: there the focus could fall to the page, and WebKit keeps a focus on a hidden element.

## What Changes

- An E2E test of the amend form: its progress row says whom it waits for in its live region, its Stop Waiting has the
  focus, Enter stops the wait, and the form comes again with the focus in it; in both engines (`pnpm e2e:webkit` runs
  the test).

## Capabilities

### Modified Capabilities

- `branch/move-commits`: the progress row of a form in the requirement of a wait heard and reached by key.

## Impact

- `tests/e2e/amend-changes/amend-changes.spec.ts`, `playwright.webkit.config.ts`, `docs/contributing/frontend.md`.
