# Proposal

## Why

The second review of the marks of a subject found two ways a rename was said to be proven, or not, by something
other than the two trees. A use of the old name on a line that a move took as it is passed as "read anyway": the
diff shows such a line as the same line at another place, so no reader asks what its names mean there, and a
function that only moved and whose call a new function of the old name answers now left its rename `confirmed`.
And the memory of the answers of the index kept an answer that accio's budget cut short, so a result of the load of
the machine stayed for the life of the app.

## What Changes

- A line a move took as it is, within its file or to another, is neither a line that is read anyway nor a place
  of a rename: the old name on it is a use left behind (`usesOutside`), the new name one that meant something else
  (`otherMeaning`). Only a line the move rewrote (an edited line that differs in more than whitespace) is a changed
  line; a pair leaves those lines beside its diff, since the pass over a subject has no text.
- A pale block, equal to itself under the policy of the diff, is the lines it was: a use on it is not read anyway.
- An answer of the index is kept only when it is final for each of its declarations: a pair that could not be
  asked, two places that are not one declaration, a complete answer with the uses of both trees. An answer its query
  cut short is used for its question and asked for again by the next.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `commit/quiet-changes`: the group Subject.
- `commit/commit-files`: the result does not depend on the load of the machine.

## Impact

- `crates/file-diffs/src/marks_subject.rs` (`Subject::is_moved`, `is_read`), `leftovers.rs` (`Leftovers::rewritten`),
  `service.rs` (`MODEL_VERSION` 13, the answers kept), `accio.rs` (`RenameAnswer::is_final`).
- `crates/text-diff/src/moves.rs` (`rewritten_lines`).
- `docs/design/diff-view.md`, `docs/design/text-diff.md`, `docs/design/git-operations.md`.
