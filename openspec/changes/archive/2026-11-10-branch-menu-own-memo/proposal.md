# Proposal

## Why

The main window went blank (React #185, "Maximum update depth exceeded") when two menus of one branch were mounted at
once and a store changed: the menu of a row that still faded out beside the command palette opened on the row (a menu
item pressed, then ⌘K, on a machine that paints no frame of the window for seconds) or beside the "⋯" of the preview,
and the right-click of a row the palette had listed last, during any sync. The selector of each menu read a cache by
branch that held one entry; the second subscriber, whose `setExpanded` differs, replaced it on every read, so
`useSyncExternalStore` got a new menu on every read and rendered both menus again until React gave up.

## What Changes

- A selector that builds (the menu of a branch, of an island header and of a kept row, the actions of the review, the
  read of the Safe Push popover) keeps what it built in a memo of its subscriber (`useSourceMemo`), by the states of
  the stores it reads and by its arguments, never in a cache by a key that two subscribers share.
- The command palette reads the menu of the branch at hand only while it shows; it read the menu of the branch of its
  last open again on every change of a store, also while closed.
- `useControlRef` and `useHeadingMenu` read their context with `useContext`: React Compiler compiles a hook only when it
  calls a hook named `use` and a capital, and the ref callback of each render made React detach and attach the ref of
  every trigger it rendered again (a menu item unregistered from its menu and registered again).

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `branch/branch-row`: Branch menu: two menus of one branch at once each follow the stores.

## Impact

- `src/hooks/useSourceMemo.ts` (new), `src/hooks/useBranchContextActions.ts`, `src/components/branch/StackIslandCard.tsx`,
  `src/hooks/useKeptBranchMenu.ts`, `src/hooks/useReviewActions.ts`, `src/hooks/useSafePush.ts`,
  `src/components/command-palette/CommandPalette.tsx`, `src/hooks/useControlKey.ts`, `src/hooks/useHeadingCard.ts`;
  `docs/design/rendering-performance.md` (Key Decision 3), `docs/contributing/frontend.md` (Pitfalls).
