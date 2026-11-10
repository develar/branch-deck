# Proposal

## Why

A popup that a hover opens did not open again when the pointer came back over its trigger soon after it closed: the
tooltip of a status after ⌘3 and ⌘1, the description of a push, the submenu Copy of the row menu reopened right after
Copy Branch Name. Every popup part of Base UI 1.9 (`TooltipPopup`, `PopoverPopup`, `MenuPopup`, `PreviewCardPopup`, by
`useHoverFloatingInteraction`) cleared the timers of its trigger's hover as it unmounted, at the end of its exit: the
open delay the pointer back over the trigger had just started was gone, and the popup opened only at the next move. A
view that hides keeps a closed popup until it shows again, and a loaded machine ends an exit late, so the pointer was
back first in most runs of four E2E tests under load (`statistics.spec.ts`, 20 of 20 at a load of 20 to 37).

## What Changes

- A patch of `@base-ui/react@1.9.0` (`patches/@base-ui__react@1.9.0.patch`, `patchedDependencies` of
  `pnpm-workspace.yaml`): only a trigger clears the timers of its hover as it unmounts, a popup leaves them to it. The
  master of Base UI still clears them (`intent.cancelPendingOpen()` on the unmount of `useHoverPopupInteraction`): an
  upgrade ports the patch.
- E2E tests that bring the pointer back over a trigger while its closed popup still exits, with the time standing
  still, and see it open after the delay of its hover: a tooltip and a hover card a hidden view closed, a submenu a
  press of its item closed.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `branch/branch-row`: Hover and click: the pointer back over a trigger opens its popup after the delay of its hover.

## Impact

- `patches/@base-ui__react@1.9.0.patch`, `pnpm-workspace.yaml`, `pnpm-lock.yaml`;
  `docs/contributing/frontend.md` (Pitfalls).
