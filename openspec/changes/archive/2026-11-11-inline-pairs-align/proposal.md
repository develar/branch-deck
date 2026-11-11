# Proposal

## Why

Inline is the default layout, and its row of a similar pair is the one place a diff shows two lines as one. A review
of the lane found where that row lost or misplaced what the old line holds:

- the engine paired the lines of a block by their count and their likeness alone, while its inner changes are of the
  text of the whole block: a call wrapped again, whose argument goes up a line, has a change across the line end, and
  the row of its second lines showed the new line without the argument the old line lost; the find did not find it;
- under Trim Whitespace and Ignore Whitespace the text a pair shares differs in length (another indent), and an
  offset of the old line was placed as if it did not: a hit of the old line marked the indent, a revealed old name
  selected the word beside it;
- a name in a struck-through part was read as the word of the new line that stands there: ⌘-click, the underline of
  the link and the menu of the code acted on the new name, and the keys of the code on a revealed old name too;
- a copy of the revealed old name put an empty text on the clipboard, and any key, a modifier too, let the name go;
- a line with more hits than a call takes arguments (a minified file, one letter typed) failed the find.

## What Changes

- The engine pairs the lines of a block only when one row can show all of both (`pairs_align`): each inner change
  lies in line i of both sides, and between its changes a pair is the same text but for whitespace. Any other block
  shows line by line, as in Unified, with its ranges. `MODEL_VERSION` and `MERGE_MODEL_VERSION` grow.
- The page takes a pair as the engine gives it (`inlinePair`); lines whose ranges do not align, which no engine
  gives, show whole, the old line struck through before the new one.
- An offset of the old line maps into the text the row shows from the new line by the characters that are no
  whitespace (`sharedOffset`).
- A point in a struck-through part is a point of the old line: the word there is the old name, its underline is that
  part alone, and the menu of the code acts on the name it opened on.
- The revealed old name stays selectable for as long as the selection is that name; a copy takes it, the keys of the
  code read it, a modifier alone and the copy keep it.
- The hits and the parts of a line are added in loops.
- The rows of the painter have test ids and its parts data attributes; no test finds them by a class.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `commit/commit-files`: what Inline shows as one row, and that a row hides no text of its lines.
- `commit/commit-review`: the diff search in the row of a pair under a whitespace policy, in a block of no pairs, in
  a line of very many hits.
- `commit/symbol-navigation`: a struck-through name is the name of the old line; the revealed old name is copied and
  read by the keys.

## Impact

- `crates/text-diff/src/model.rs`, `crates/file-diffs/src/service.rs`, `crates/file-diffs/src/merge.rs`.
- `src/lib/diff/line-parts.ts`, `src/lib/diff/find.ts`, `src/lib/diff/inline-rows.ts`,
  `src/components/diff/diff-painter.ts`, `src/components/diff/DiffView.tsx`.
- `tests/e2e/helpers/diff-helpers.ts` and the specs that find the rows of a diff; `docs/design/diff-view.md`,
  `docs/design/text-diff.md`, `docs/contributing/e2e-tests.md`.
