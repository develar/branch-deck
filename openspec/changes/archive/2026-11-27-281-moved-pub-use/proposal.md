# Proposal

## Why

The split of a file into modules (develar/branch-deck#281) took the imports and the `pub use` lines beside the moved
code as gone with the move. A `pub use` passes names on and may be API: one the commits remove and no file holds again
made the place it left nothing to read.

## What Changes

- A `pub use` line a block removes beside the moved code goes with the move only where a file the commits add holds the
  same code (as a call of a macro does, `MoveEnds::lines_left`); else the place is read. An import of the file alone
  still goes with the move.

Capabilities: `branch/review-units`.
