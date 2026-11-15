# Proposal

## Why

`repository-lock.spec.ts`, the stop of the wait of a squash of the fixups, failed once in 592 runs in WebKit at a load
of 60, and among the timeouts of a loaded Studio: after Stop Waiting the Squash of the popover was neither focused nor
there. Stop Waiting gave the focus back in the cleanup of a passive effect, after it had left the document: the focus fell
to the page meanwhile, and the popover, which takes a focus on the page for one that left it, closed whenever that cleanup
came after its check. With the return of the focus held back by 300 ms, the test fails 3 of 3 times in both engines.

## What Changes

- Stop Waiting takes the focus and gives it back in a layout effect, whose cleanup runs before it leaves the document,
  and only while it has the focus: the focus goes from it to its target at once, never through the page.
- The tests of a wait beside its button read, as the focus comes to its target, whether Stop Waiting is still in the
  document: with the passive effect each of the five fails in both engines, with the layout effect none.

## Capabilities

### Modified Capabilities

- `branch/move-commits`: the focus never passes through the page as Stop Waiting goes.

## Impact

- `src/components/shared/LockWaitNotice.tsx`, `tests/e2e/repository/repository-lock.spec.ts`,
  `docs/contributing/frontend.md`.
