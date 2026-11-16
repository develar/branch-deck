# Proposal

## Why

A card over its chip hangs by its bottom edge: when its robot turned out to need review, the second row of its actions
went (Start in Air, the prompts) and the first row moved down under the pointer, about 40 px without a pull request; the
rows always stood in one order. A card beside its chip that the positioner shifted up against the bottom of the window
hangs by its bottom edge too, and its late details below the actions pushed it up. Neither had a test: the 420 px case
stands beside its chip, and the spec named a test over the chip that did not exist.

## What Changes

- While the pointer is in the details of a chip (or a press that began there lasts), they hold still where they stood:
  a virtual anchor, side and alignment place them as they were, hung by the edge they hung by, without collision
  avoidance (`useHeldAnchor` of `StatusChip`). What comes or goes in them changes their other edge only. The edge is
  `data-hang` of the popup: the chip's edge under or over it, of a card beside it the bottom edge once aligned by its end
  or shifted up against the bottom of the window.
- A card hung by its bottom edge stands its first row of actions at the bottom, the late details above (`data-hang` and
  `data-side`); the focus order stays the order of the DOM (accepted, docs/contributing/frontend.md).
- The robot of a whole stack without a pull request offers neither Restart nor Safe Push once it needs review: its first
  row goes, accepted.
- The fake Patronus answers a held problems request with what it had when asked.
- E2E: each test says where its card stands (`data-side`, `data-hang`); over its chip with and without a pull request;
  beside its chip against the bottom of the window; the pointer point stays in the control.

## Capabilities

### Modified Capabilities

- `branch/branch-row`: the details hold still by the edge they hang by; the rows of a card hung by its bottom edge.
- `branch/patronus`: the over-its-chip tests of a late reason.

## Impact

- `src/components/branch/StatusChip.tsx`, `src/components/branch/PatronusChip.tsx`,
  `crates/patronus/src/fake_patronus.rs`, `crates/patronus/src/actions_test.rs`,
  `tests/e2e/patronus/patronus-actions.spec.ts`, `docs/contributing/frontend.md`.
