# Proposal

## Why

N and P stopped at every block of changed lines, also at one Rust had proven a change of formatting alone, and the
counts of a file said nothing of how much of it needs reading. The diff names the line the reading of each block
starts at (`TextDiff.marks.starts`); the views read it now.

## What Changes

- N and P stop only where the reading of a block starts, at the first row of the part that shows that line; a block
  without a start, a block of the imports alone in a review with its plan and a block another section shows are passed.
- A file says how many of its changes are left to read where it shows its counts, when fewer than all: its row in
  the tree of the files and the header of its card in a review.
- A note is one row and a line shown once the rows of its new line: the layout stays arithmetic.
- `--diff-quiet`, one colour of both themes for a line with nothing to read.
- The requirements that said N and P stop at every block, that a change of indentation always has inner ranges and
  that a whitespace-only block is always pale say what holds in a file of a grammar.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `commit/quiet-changes`: the stops, the count to read, the layout and the colour of the group Views.
- `commit/commit-files`: the tints of a block, a change of indentation, a whitespace-only block.
- `commit/commit-review`: N and P, the blocks they step through, the find.

## Impact

- `src/lib/diff/block-parts.ts` (`stopsOf`, `readingOf`, `partsStopRow`), `rows.ts` (`changeStarts`, `changes`),
  `view-files.ts` (`withEngineCounts`), `src/components/files/FileTree.tsx`, `CommitReviewCode.tsx`.
- `src/index.css`, `src/components/diff/diff-view.css`; `tests/perf/diff-fixtures.ts`.
- `tests/e2e/commit-files/commit-files.spec.ts`, `commit-review.spec.ts` (the tests on `commit_whitespace`).
