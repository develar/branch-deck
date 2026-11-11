# Proposal

## Why

The second reading of a block, with the two names of a rename read as one, made one run of the kind `renamed` of every
row of lines that were the same tokens then: a call wrapped beside a line of the name, forty lines indented inside a
new `if` with one use of the name among them. Those lines changed in their formatting alone, which the tokens of the
texts prove without the rename, and they lost their quiet: read in full under the note of a rename while the kind is
a label, and for good beside a rename that is never proven (a member, a parameter a call may name, a local of a C#
file with an `#if`). A run of comments beside a line of the name whose comment changed too was lost the same way.

## What Changes

- A group of segments that are the same tokens is cut where it may be, at each end of a segment that as many of the
  same tokens stand before on both sides: its parts that hold a place of a rename are runs of the kind `renamed`, the
  parts that hold none the runs of formatting they are without the rename, the blank lines between the two with the
  formatting. A segment that holds a place and differs in its comments ends the run of comments before it.
- The second reading adds runs and takes none: a run the first reading found lies in a run of its kind. Where the
  two readings pair the lines of a block otherwise, the run of the first stays, in place of the runs of the second
  that share a line with it or cross it.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `commit/quiet-changes`: a run of a renamed name takes no line of formatting.

## Impact

- `crates/file-diffs/src/marks_runs.rs` (`Asked`, `Rows::runs_of_group`), `marks_renames.rs` (`Renames::cut`,
  `with_the_first`), `marks.rs` (`read_blocks`, the first reading of the blocks by itself).
- `docs/design/diff-view.md` ("Marks").
