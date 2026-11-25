# Proposal

## Why

In WebKit the Landing overview stayed open after the pointer left its button (2 of 20 runs of `land-together.spec.ts`
under load, on master too): the button changed what it shows while the pointer rested on it (its count and the
segments of its bar as the units changed), the element under the pointer left the page, and WebKit told the button no
leave at the next move, so the popover its hover opened never closed.

## What Changes

- The content of a popover trigger that opens on hover takes no pointer events (`HOVER_TRIGGER` of
  `src/components/ui/popover.tsx`): the trigger alone is under the pointer, whatever its content becomes.
- The overview closes as the pointer leaves its button also after the button changed under the still pointer.

Capabilities: `repository/landing-overview`.
