# Proposal

## Why

A sync that put a group above an open submenu ("Fix Conflict…" above "Copy ›" of the branch menu) closed the submenu
when the pointer rested on its trigger: the content moved Copy from under the still pointer, the browser told Copy that
the pointer left and the item now there that it came, at the same position (Chromium and WebKit alike), and the safe
polygon of Base UI closed the submenu on that leave. Lane L's change `menu-group-keys` kept the groups mounted; this is
the pointer part, the class of content that moves under a still pointer (`src/lib/pointer-moves.ts`).

## What Changes

- `hoverClosesOnMove` of `src/lib/pointer-moves.ts`, in the submenu roots of `src/components/ui/context-menu.tsx` and
  `dropdown-menu.tsx`: a close by the hover whose event no move brought is canceled, and the next move that is not over
  the trigger gives it the leave of the pointer, at its position and as an event of the move, so Base UI decides by the
  pointer that moved: away the submenu closes, into it the submenu stays.
- E2E tests in Chromium and WebKit: the pointer rests on Copy while a sync puts "Fix Conflict…" above it; the submenu
  stays open, then closes at a move away, or stays at a move into it.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `branch/branch-row`: Hover and click: a submenu whose trigger moved from under a still pointer stays open until the
  pointer moves.

## Impact

- `src/lib/pointer-moves.ts`, `src/components/ui/context-menu.tsx`, `src/components/ui/dropdown-menu.tsx`;
  `docs/contributing/frontend.md` (Pitfalls).
