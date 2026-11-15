# Proposal

## Why

WebKit, the engine of the app on macOS, puts the caret of a point inside an empty line after the empty text of its
cell: `caretPositionFromPoint` answers offset 1 of the empty `dv-text` element, which has no children, and the range
the painter started there to read the column (`textOffset`) threw "The index is not in the allowed range": each move of
the pointer with the modifier over an empty line threw, and the underline of the name the pointer left stayed under it.
moved-lines.spec.ts:292 failed on it in WebKit, 9 of 600 at a load of 100: the pointer rested on an empty line after
the jump, and ⌘[ held the modifier as WebKit moved the pointer over the scrolled rows. The guard of `caretAt` that
f5fdf76e0 added (a caret out of its node is none) ends the throw; its words and its test named a guess of the case
(`caretRangeFromPoint` past the end of a text over code drawn anew), and its test stands in for the engine.

## What Changes

- The comment of `caretAt` and the requirement of the pointer name the case of the engine: a point inside an empty line.
- A WebKit test on the engine as it is moves the pointer with the modifier from a name on to an empty line and back.
- The test of the stand-in engine is named for what it gives, a caret past the end of a text.

## Capabilities

### Modified Capabilities

- `commit/symbol-navigation`: the modifier over an empty line underlines nothing.

## Impact

- `src/components/diff/diff-painter.ts` (the comment of `caretAt`), `tests/e2e/commit-files/symbol-navigation.spec.ts`.
