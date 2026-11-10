# Proposal

## Why

The groups of a menu (`MenuItems`) were keyed by their place. A change of a store while a menu shows that adds or
drops a group above another (a sync that finds a conflict puts "Fix Conflict…" on top of the branch menu) gave every
group after it the key of the one before: React mounted them again, so the open "Copy ›" closed under the pointer and
its items left the DOM under a click ("element is not stable", the backdrop of the menu "intercepts pointer events",
"element was detached", the 30 s timeouts lane K saw in `branch-row-layout.spec.ts`; there they came from its work in
progress, the menu of the test never changing on master, but the defect was of every menu).

## What Changes

- A group of a menu is known by its item that opens a submenu, else by its first item (`groupKey`), never by its
  place: a group that moves keeps its elements, its open submenu and the highlight in it.
- An E2E test opens "Copy ›" of a branch, rests the pointer on its items, and syncs a conflict into the branch: the menu
  leads with "Fix Conflict…", "Copy ›" stays open, and Copy Branch Name copies.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `branch/branch-row`: Branch menu: an open submenu stays open when the menu changes above it.

## Impact

- `src/components/shared/MenuItems.tsx` (every menu built of `MenuItems`: the right-click menus and the menus of
  buttons).
