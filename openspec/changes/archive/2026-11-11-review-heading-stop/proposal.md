# Proposal

## Why

A stop at a card (J and K at a card, a row of the tree, N and P, a hit of the find, a declaration shown) put the card
at the top of the diffs, `cardTop`, whatever stood right above it. An opening ends with a gap of 10 px above its card,
so it went wholly above the view; but the heading of a group ("Imports only (1)", "Generated (1)", "view.gui · Tests
· 1 file") stands right above its card, its words at its bottom, so their lower edge peeked out under the bar of the
step, 12 of their 16 px cut.

## What Changes

- A stop at the card of a section that starts a group with a heading (`LayoutFile.divider`, no opening) goes to the
  top of the heading (`DiffLayout.stopTop`, `fileTop`): the heading shows whole at the top, the card right below it,
  as IntelliJ shows the heading of what it scrolls to. The heading belongs to the section: the bar names its step,
  the heading names the group, and a half of it said neither.
- A stop at a card below an opening keeps the card at the top: the opening, a stop of its own that J showed before,
  goes wholly above the view, the gap at the end of its layer above the card, never a cut edge of it. The view scrolls
  to a stop up to a whole pixel: WebKit drops the fraction of a `scrollTop`, and a stop a fraction below its place
  showed a pixel of the opening above it.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `commit/commit-review`: a change, a hit and a declaration land below the heading of their group.
- `branch/review-tour`: a stop at a card shows the heading of its group whole, an opening wholly above the view.

## Impact

- `src/lib/diff/layout.ts` (`stopTop`), `src/components/diff/DiffView.tsx` (every stop at a card),
  `src/components/diff/diff-painter.ts` (`scrollTo` up to a whole pixel); the e2e of the parts of a file and of the
  tour.
