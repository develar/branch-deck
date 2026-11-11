# Proposal

## Why

A move says "these lines stand there now, re-indented by two columns", and the Inline view shows its old lines once.
A line of a move that was indented otherwise than the others (the text of a raw string), whose tabs became spaces,
or that lost the two spaces that end a line of Markdown was called the same line: under the policy that counts every
character the diff hid a change that a plain block would have shown as a range. Where the indentation has a meaning,
the indent of a line was compared by its number of columns, so a tab that became a space was no change of a recipe.

## What Changes

- Under the policy that counts every character, a line of a move whose indent is another text and did not move by
  the shift of the move, or whose whitespace after its text is another, is an edited line with that whitespace as a
  range; a blank line too. It is still an exact line of the search where the indentation has no meaning. Under a
  policy that leaves that whitespace out it is no change of a move, where the indentation has a meaning too.
- The shift of a move is a text: the same whitespace added to the indent of each line, or taken from it, at one end.
  `find_moves` takes the whitespace policy of the diff.
- `LineMove.edited` names the edited lines of a move: a line that stayed at its indent while its move shifted,
  where the indentation has a meaning, is edited and has no range (never a range that is empty on both sides).
- A move counts the blank lines that moved with it (`blanksBefore`, `blanksAfter`); they are no entry of their own.
- The search for moves ends at 262,144 pairs of lines read (it was 4,194,304: seconds for one file), a pair of two
  lines read for their likeness counted by its length too. Two lines of other keys are compared by word only in a
  row that may be a move with them edited, and the chains grow through at most 16,384 pairs they leave unread.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `commit/quiet-changes`: the whitespace at the ends of a line of a move. The other requirements of the moves are
  those of `2026-11-11-moves-within-a-file`, which lands in the same commit and holds them as they are now.

## Impact

- `crates/text-diff/src/moves.rs`, `model.rs` (`LineMove`: `blanks_before`, `blanks_after`, `edited`).
- `crates/file-diffs/src/marks.rs` (`marks_of` takes the policy), `benches/marks_bench.rs` (the file a search costs
  most in, a subject with moved functions).
- `src/lib/bindings.ts` (`LineMove`).
- `docs/design/text-diff.md` ("Moves"), `docs/design/diff-view.md` ("Marks").
