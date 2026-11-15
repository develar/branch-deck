# Proposal

## Why

The audit of what comes late beside a hovered control found the lock wait of the forms of a row on a line of its own
above their buttons: as the rewrite began to wait for another holder of the repository lock, the notice pushed Drop
Commit, Move and the Squash of the fixups popover from under the pointer that had just pressed them.

## What Changes

- `LockWaitNotice` comes on the line of the buttons of the drop form and of the squash of the fixups, before them, and
  below the input and the buttons of the move to a branch; the pressed button stays where it stood.

## Capabilities

### Modified Capabilities

- `branch/move-commits`: where the surface that started a rewrite says whom it waits for.

## Impact

- `src/components/shared/InlineForm.tsx`, `src/components/commit/InlineDropConfirm.tsx`,
  `src/components/shared/InlineBranchTarget.tsx`, `src/components/commit/SquashFixupsPopover.tsx`,
  `docs/contributing/frontend.md`.
