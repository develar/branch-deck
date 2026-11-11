# Proposal

## Why

A function that moved within its file reads as a deleted block and an added block: a reviewer reads its lines twice
and compares them by eye to see whether anything changed on the way. The diff has a place for the answer
(`marks.moves`, empty until now) and the line the reading of each block starts at, which still stopped at the lines
that left.

## What Changes

- A text diff names the lines that moved within its file (`marks.moves`): rows of lines one block deletes and
  another block adds, found from the lines of the two texts alone (`text_diff::moves::find_moves`), with no grammar.
  The key of a line is its text without the whitespace at its ends; a move counts from 24 characters of exact lines.
- A move says the columns its lines were indented by, and holds at most a quarter of edited lines between exact
  ones, each with what changed in it as the ranges of a block. Where the indentation of a file has a meaning, a line
  at another indent than its move is an edited line.
- One to one, the longest first; a move counts the blank lines that moved with it.
- A move is never quiet. Its lines are in no run, its new lines are always read, and its old lines are nothing to
  read where they left: `starts` has no entry for a block of nothing but lines that moved.
- The limits are by work: the lines a move may take, the lines of a key that starts a move, the pairs of lines read
  (each read, also of what is left of a row after a move) and the pairs compared by word. A chain of many short
  moves is read once for all of them, so no file costs the square of its lines.
- The empty last line of a text, which the comparison does not count, is no line of a move.
- The `commit_marks` template has its two moves: `accounts` with the blank line that moved with it, `describe`
  indented, with an edited line.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `commit/quiet-changes`: the moves of a diff, the starts beside them, the limits of the search.

## Impact

- `crates/text-diff/src/moves.rs` (new), `model.rs` (`starts_of` reads the moves), `inline.rs` (`is_similar_pair`
  for the crate).
- `crates/file-diffs/src/marks.rs` (`marks_of` fills the moves; `pair_indented`), `service.rs` (`MODEL_VERSION` 9),
  `benches/marks_bench.rs`.
- `src/lib/bindings.ts` (the words of `DiffMarks` and `LineMove`).
- `docs/design/text-diff.md` ("Moves"), `docs/design/diff-view.md` ("Marks"), `docs/design/git-operations.md`.
