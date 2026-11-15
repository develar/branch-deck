# Proposal

## Why

A wait for the repository lock that a surface started (the drop, the move to a branch, the squash of the fixups, the
squash dialog, the commit message editor, Apply Fix, the progress row of an inline form) said whom it waits for only
to the eye: the notice was no live region, so a screen reader heard only the "Loading" of the button. And the press
dropped the focus: the pressed button became disabled (a press by the pointer had focused it), and the focus fell to the
page, from where a keyboard user needed Tabs to reach Stop Waiting; Stop Waiting pressed by key dropped it again.

## What Changes

- A polite live region beside each Stop Waiting of a surface, there before the wait, says whom it waits for without how
  long (`presentLockWaitAnnouncement`: "Waiting for `brd pick` (pid 4242)"), so it speaks once per holder and not with
  the time that grows every second; the visible notice, which has the time, is hidden from it.
- One rule of the focus on every such surface: Stop Waiting takes the focus as the wait shows, where the press left it
  on the pressed button, elsewhere in its form, dialog or popover, or nowhere, so Enter or Space stops the wait without
  a Tab; as the wait ends, stopped or over, the focus goes back where it came from, else to the button that waited, else
  to the surface. A focus the user moved elsewhere stays there.
- The pressed button stays focusable while it is busy (`focusableWhenDisabled`, `aria-busy`), dimmed as before, so the
  focus a pointer gave it stays and has a place to go back to.

## Capabilities

### Modified Capabilities

- `branch/move-commits`: a wait a surface started is heard and reached by key.

## Impact

- `src/components/shared/LockWaitNotice.tsx`, `src/presenters/lock-wait.ts`, `src/stores/indicators.ts`, the pressed
  buttons of `InlineForm`, `InlineInputDialog`, `SquashFixupsPopover`, `ConflictFixPanel`, `SquashCommitsDialog` and
  `CommitPreviewDialog`, the HTML snapshots of these surfaces (the empty live region), `docs/design/operations.md`,
  `docs/contributing/frontend.md`.
