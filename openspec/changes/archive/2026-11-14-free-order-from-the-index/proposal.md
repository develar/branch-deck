# Proposal

## Why

A review plan folds a declaration that moved within its file with the same code ("Moved only") where its place has
no meaning, and Branch Deck decided that by rules of its own for each language (`crates/review-plan/src/moved.rs`):
which declarations may move freely, which may be crossed, what a decorator does. accio now answers the same question
for every tool (`accio moves`, answer revision 2, `reordered`: each declaration a change put in another order among
those of its owner, with `order: "free"` or `"matters"` and the rule that decides it), so the knowledge lives once,
in the index; and the diff of a commit showed such a move as any move, never saying its order does not matter.

## What Changes

- The plan folds a pair that moved with its code the same (`moved`) only where the index says its order is free: the
  declaration the index reordered with an order that is free holds its name on both sides, and every line of code its
  blocks add is a line of such a declaration (no other code moved with it). The plan of git alone reads it, and so
  does a plan without an index; the index is asked of the moves (`accio moves`, kept by the two trees with the diffs
  of the same subject) beside its other queries only where a pair moved, and the story of the plan (its units,
  concerns, brief and steps) is told again with the fold. `crates/review-plan/src/moved.rs` is deleted.
- The marks of a subject say which moves within a file hold declarations of free order (`SubjectMarks::reordered`, by
  the two ends of each move): a move of the pair, or a declaration the index reordered in another layout, which is a
  move of the declaration to the file itself now. A label: their lines are read where they stand now.
- The note of such a move names its declarations and says the order does not matter ("Moved total from line 4 ·
  order does not matter", "Moved total to line 8 · order does not matter").
- `brd commits --mark-stats --index` counts those moves (`moves.free_order`).

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `branch/review-units`: a pair that moved is folded where the index says its order is free, not by rules of Branch
  Deck.
- `commit/quiet-changes`: a move within a file of declarations whose order is free says so; a declaration the index
  reordered in another layout is a move of the declaration to its own file.

## Impact

- `crates/review-plan/src/{units,lib,whitespace}.rs`, `moved.rs` deleted; `crates/diff-marks/src/{reorder,model,marks_subject}.rs`;
  `crates/text-diff/src/model.rs`; `crates/file-diffs/src/{service,accio}.rs` (`FileDiffs::moves`); `crates/bd-rewrite/src/mark_stats.rs`;
  `src/lib/diff/block-parts.ts`, `src/components/diff/diff-painter.ts`, `src/presenters/diff-marks.ts`; the template
  `free_reorder` and the e2e helpers.
- `docs/design/change-review.md` (item 26), `docs/design/diff-view.md` ("Marks"), `docs/contributing/code-map.md`.
