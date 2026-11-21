# Proposal

## Why

Since schema 39 a function moved whole to another file says where it went and where it came from, but the window still
shows it twice: its old lines in red under "formatTotal — removed" and its new ones under "formatTotal — new", read twice
(decision 34 of docs/design/change-review.md: one unit of an exact move comes next). The reader should meet the move
once, at its new place, where it has other imports and another owner.

## What Changes

- The plan says which slice of the place a move left holds nothing but the move (`UnitSlice::moved_out`): the blocks of
  a unit `removed` change the old side as removing the declarations that moved would, blank lines aside, or a deleted
  file holds its moved declarations, imports and comments alone. Pure, no git process, no index.
- The place of a move names the unit that shows it (`MovedPlace::unit`): of its declaration, or the unit that takes its
  file whole.
- The window reads the move at its new place, its heading "formatTotal — moved from Orders.kt" in place of "new"; a new
  file names what moved into it; the old place, where it holds the move alone, is one row of the kind of "Shown in
  Imports only", "Moved formatTotal to Report.kt", which opens the section of the new place, another step named.
  Anything more at the old place stays read under "formatTotal — moved to Report.kt".
- Still two units: ids, fingerprints, kinds, the order, the concerns, the steps and their coverage are those without it;
  what is left to read counts as the diffs count it (decision 28).
- `brd review-plan`: `unit` of `moved_from` and `moved_to`, `moved_out` of a slice, `"schema": 40`.

Capabilities: `branch/review-units`, `branch/review-plan`, `review/range-reading`.
