# Proposal

## Why

The head of the Status and Actions columns is their minimum (`--bd-group-<group>`), and the items of a group stood at
its start. While no row of the view showed a push or a step, the Actions of a row were narrower than the head
"Actions": "⋯" stood 19.4 px before the end of the row. As the push of the last unpushed branch ended, its track went
from 64 px to 0, and "⋯" of every row moved from 912 to 892.6 px, under the pointer that had pressed the push.

## What Changes

- The items of Status and Actions stand at the end of their group, as the row stands at its end: an item that comes or
  goes moves only what stands before it in the row.

## Capabilities

### Modified Capabilities

- `branch/branch-row`: the items of a group stand at its end.

## Impact

- `src/lib/branch-columns.ts`, `tests/e2e/branch-row-layout/column-widths.spec.ts`, the HTML snapshots of the rows,
  `docs/design/ui-theme.md`.
