# Proposal

## Why

The review of b349be5c6 and 542fc5d0 found three gaps. Of the returns of the focus only the move by Enter and the amend
ran in WebKit, whose press of the pointer focuses no button, so the path from nowhere went untested there. The live
region read the backticks around the command line of the holder aloud ("grave"). And a busy button that keeps the focus
(`focusableWhenDisabled`: `aria-disabled`, no `disabled`) got none of the `disabled:` styles of `Button`: its hover
background came, a press shifted it by a pixel, and each caller dimmed it by hand.

## What Changes

- The six tests of a wait beside its button (the drop, the move by the pointer, the squash of the fixups, the squash
  dialog, the reword, the stop by Space) run in WebKit too.
- `presentLockWaitAnnouncement` names the holder without the backticks of the screen ("Waiting for brd pick (pid
  4242)"); the visible text keeps them.
- `Button` gives an `aria-disabled` button no hover and no press shift, and dims it unless it is busy (`aria-busy`: its
  spinner says it, as Update, Sync and the pushes did by hand), with no `pointer-events-none`, so its tooltip opens. It
  leaves out the `hover:` and `active:` classes of such a button, its variant's and its caller's: `aria-disabled:`
  variants of the hover classes would outrank the hover a caller gives in place of the variant's
  (`ghostButtonColorClass`), which `cn` merges only where both are `hover:`; a test of the push button caught that.
  The callers drop their own `opacity-50`; a button whose run its caller did not dim before says it with `aria-busy`
  (the launch in Air, the creation of a pull request, the open of a review).
- The busy pressed buttons of a lock wait are no longer dimmed: their spinner says that they wait, as on every other
  busy button of the app.

## Capabilities

### Modified Capabilities

- `branch/move-commits`: the words of the live region, the look of a busy button, the WebKit runs.

## Impact

- `src/components/ui/button.tsx` and the callers of `focusableWhenDisabled` that dimmed by hand,
  `src/presenters/lock-wait.ts`, `tests/e2e/repository/repository-lock.spec.ts`,
  `tests/e2e/amend-changes/amend-changes.spec.ts`, the HTML snapshots of a focusable disabled button,
  `docs/contributing/frontend.md`, the UX note of `repository/outgoing`.
- Newly dimmed while disabled and not busy, where no caller dimmed: the decisions of a review, the arrows of the review
  header, Clear of Activity, a `MenuItemButton` kept focusable; the buttons a creation of a pull request disables beside
  its Create; a Safe Push button of a lost start.
