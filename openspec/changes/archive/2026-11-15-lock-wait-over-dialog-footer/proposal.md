# Proposal

## Why

The audit of what comes late beside a hovered control found the lock wait of the squash dialog and of the commit
preview on a line of its own in the footer, beside the buttons: as the squash or the reword began to wait for another
holder of the repository lock, Squash and Save moved 9 px down under the pointer that had pressed them, and widened
14 px to the left as their spinner came where they had no icon.

## What Changes

- The notice comes over the lines of the footer of the two dialogs (`LinesUnderLockWait`): they keep their room,
  hidden while it shows, so the footer keeps its height. In the row of the buttons it would squeeze the lines into more
  of them.
- Squash and Save have an icon, which the spinner takes the place of: the button keeps its box.

## Capabilities

### Modified Capabilities

- `branch/move-commits`: where the surface that started a rewrite says whom it waits for, in a dialog.

## Impact

- `src/components/shared/LockWaitNotice.tsx`, `src/components/commit/SquashCommitsDialog.tsx`,
  `src/components/commit/CommitPreviewDialog.tsx`, `docs/contributing/frontend.md`.
