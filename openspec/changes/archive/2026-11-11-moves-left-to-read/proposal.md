# Proposal

## Why

The views count what is left to read of a file by the starts of its blocks, and the moves take the starts of the
blocks their lines left: with both in one build, `src/Ledger.kt` of the `commit_marks` template says "8 to read", not
the "10 to read" the scenarios of the count named while the wrapped call was its one block without a start. The
requirement is the same; its scenarios named a number of the template that the moves changed.

## What Changes

- The scenarios of the count of a file name what the template shows with its moves: of the eleven changes of
  `src/Ledger.kt`, eight are left to read. The wrapped call and the two blocks its moved functions left are none of
  them: N and P pass the three, and the lines the functions left still show as deleted lines until a view draws a
  move.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `commit/quiet-changes`: Views: the count of what is left to read of the file of the template.

## Impact

- No code: `stopsOf` and `readingOf` of `src/lib/diff/block-parts.ts` read the starts as before, and the E2E reads
  the count from the page (`ledgerStops` of `tests/e2e/commit-files/quiet-changes.spec.ts`).
